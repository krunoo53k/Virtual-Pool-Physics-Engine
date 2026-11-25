//! The renderer hand-off: the piecewise-analytic path of a shot (§11.2).
//!
//! The core is event-driven: between two events every ball's state is given by a
//! closed-form expression, so nothing is ever integrated. What a renderer needs is
//! therefore not a stream of frames but the *path* - and §11.2 says so explicitly:
//!
//! > you hand the renderer a list of segments ("ball 7 slides in a straight line
//! > until t=1.83 s, then rolls until t=4.1 s") and it can interpolate the exact
//! > position at any render time, including sub-step times, with no accumulated
//! > interpolation error.
//!
//! This module is that list. A [`Segment`] is one ball's motion between two events
//! that touched it, and inside a segment the motion is a parabola:
//!
//! ```text
//! p(t) = p0 + v0 * dt + a * (dt^2 / 2)
//! ```
//!
//! The acceleration `a` is constant over a segment because it is a function of the
//! motion state, and the motion state only changes at an event - the slide-to-roll
//! transition *is* an event. So the segment describes the trajectory exactly, and
//! [`Trace::sample`] evaluates it at any time the renderer wants.
//!
//! The recorder in [`crate::sim`] only ever *reads* the state of the simulation, so
//! turning it on cannot change the physics: the event list, the final state and the
//! hash are the same with and without it. The one thing the recorder does that is
//! not merely reading is *shaping* the path: it splits a segment where the
//! segment's straight-line model of the vertical spin would cross zero, because
//! §3.2 says a spin decay stops at zero and the straight line would carry it
//! past. That adds no event and touches no state, so the promise above holds.
//!
//! The path also continues where the shot before it left off: the rotation a
//! ball has made is counted from the orientation the ball carries into the shot
//! ([`BallState.rotation`](crate::state), the hand-off of §11.2 seen from the
//! input side), so a renderer can play two shots back to back without
//! integrating anything in between them.

use crate::json::Value;
use crate::math::Vec3;
use crate::sim::SimEvent;
use crate::state::{BallState, MotionState, PocketState};

/// The kind of motion a [`Segment`] describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentKind {
    /// Sliding on the bed: `u(t) != 0` (§3.3).
    Sliding,
    /// Rolling without slipping (§3.2).
    Rolling,
    /// Rotating in place (§3.4).
    Spinning,
    /// At rest.
    Stationary,
    /// Off the cloth, in flight (§7.1).
    Airborne,
}

impl SegmentKind {
    /// The name of the kind, as the renderer sees it.
    pub fn as_str(self) -> &'static str {
        match self {
            SegmentKind::Sliding => "slide",
            SegmentKind::Rolling => "roll",
            SegmentKind::Spinning => "spin",
            SegmentKind::Stationary => "rest",
            SegmentKind::Airborne => "air",
        }
    }

    /// The motion state of a ball that is in this kind of motion.
    pub fn motion_state(self) -> MotionState {
        match self {
            SegmentKind::Sliding => MotionState::Sliding,
            SegmentKind::Rolling => MotionState::Rolling,
            SegmentKind::Spinning => MotionState::Spinning,
            SegmentKind::Stationary => MotionState::Stationary,
            SegmentKind::Airborne => MotionState::Airborne,
        }
    }
}

/// One ball's motion between two events that touched it.
///
/// The state at the start of the segment is `p0`/`v0`/`w0`, and the motion is a
/// parabola with the constant acceleration `a` (and the constant angular
/// acceleration `wa`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    /// The time the segment starts, s.
    pub t0: f64,
    /// The time the segment ends, s.
    pub t1: f64,
    /// The kind of motion in the segment.
    pub kind: SegmentKind,
    /// The position at `t0`, m.
    pub p0: Vec3,
    /// The velocity at `t0`, m/s.
    pub v0: Vec3,
    /// The constant acceleration over the segment, m/s^2.
    pub a: Vec3,
    /// The angular velocity at `t0`, rad/s.
    pub w0: Vec3,
    /// The constant angular acceleration over the segment, rad/s^2.
    pub wa: Vec3,
    /// The rotation the ball has already made when the segment starts, as a
    /// rotation vector: the axis of the rotation times the angle, in radians.
    ///
    /// A renderer turns it into an orientation: the axis is the direction of the
    /// vector and the angle is its length. The angle is *not* wrapped, so a ball
    /// that has rolled through many turns keeps its phase.
    pub spin0: Vec3,
    /// Where the ball is in the pocket state machine (§9.2).
    pub pocket_state: PocketState,
}

impl Segment {
    /// The time into the segment, clamped to the segment.
    fn dt(&self, t: f64) -> f64 {
        let dt = t - self.t0;
        let span = self.t1 - self.t0;
        if !dt.is_finite() {
            return 0.0;
        }
        if dt < 0.0 {
            0.0
        } else if dt > span {
            span
        } else {
            dt
        }
    }

    /// The position of the ball at time `t`, m.
    ///
    /// The expression is written exactly like the one in `cloth`, so that the
    /// analytic path and the stepped accumulation of the simulation agree to the
    /// last bit of the rounding noise.
    pub fn position_at(&self, t: f64) -> Vec3 {
        let dt = self.dt(t);
        self.p0 + (self.v0 * dt + self.a * (0.5 * dt * dt))
    }

    /// The velocity of the ball at time `t`, m/s.
    pub fn velocity_at(&self, t: f64) -> Vec3 {
        let dt = self.dt(t);
        self.v0 + self.a * dt
    }

    /// The angular velocity of the ball at time `t`, rad/s.
    pub fn angular_velocity_at(&self, t: f64) -> Vec3 {
        let dt = self.dt(t);
        self.w0 + self.wa * dt
    }

    /// The rotation the ball has made by the time `t`, as a rotation vector.
    ///
    /// The rotation vector is the axis of the rotation times the angle in
    /// radians, so the renderer reads the orientation off it: the axis is the
    /// direction of the vector, the angle is its length.
    ///
    /// Over a segment the angular velocity is `w0 + wa * dt`, so the rotation is
    /// the integral of it: `spin0 + w0 * dt + wa * dt^2 / 2`. That is exact for a
    /// rotation about a fixed axis - a rolling, a spinning or an airborne ball -
    /// and an approximation for a sliding ball, whose axis turns as it is spun up
    /// towards the rolling condition (§3.3).
    pub fn rotation_at(&self, t: f64) -> Vec3 {
        let dt = self.dt(t);
        self.spin0 + (self.w0 * dt + self.wa * (0.5 * dt * dt))
    }

    /// The JSON form of the segment.
    pub fn to_json_value(&self) -> Value {
        Value::Object(vec![
            ("t0".into(), Value::Number(self.t0)),
            ("t1".into(), Value::Number(self.t1)),
            ("kind".into(), Value::String(self.kind.as_str().into())),
            ("p".into(), vec3_value(self.p0)),
            ("v".into(), vec3_value(self.v0)),
            ("a".into(), vec3_value(self.a)),
            ("w".into(), vec3_value(self.w0)),
            ("wa".into(), vec3_value(self.wa)),
            ("spin".into(), vec3_value(self.spin0)),
            (
                "pocket".into(),
                Value::String(pocket_name(self.pocket_state).into()),
            ),
        ])
    }
}

/// The recorded path of a shot.
///
/// The trace is the whole shot: the path of every ball and the event list that
/// produced it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trace {
    /// The path of every ball: the segments of each ball, in time order.
    pub balls: Vec<Vec<Segment>>,
    /// The ball number of every ball, index by index.
    pub numbers: Vec<u8>,
    /// The events of the shot, in the order they were resolved.
    pub events: Vec<SimEvent>,
    /// The simulated time of the shot, s.
    pub duration: f64,
}

impl Trace {
    /// The segments of one ball.
    pub fn segments(&self, index: usize) -> &[Segment] {
        self.balls.get(index).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// The segment of a ball that covers the time `t`, if the ball was in play.
    pub fn segment_at(&self, index: usize, t: f64) -> Option<&Segment> {
        segment_at(self.segments(index), t)
    }

    /// The state of every ball at the time `t`.
    ///
    /// The positions are the analytic ones: the exact trajectory of the model,
    /// not a resampling of the simulation's own accumulation.
    pub fn sample(&self, t: f64, cfg: &crate::config::TableConfig) -> Vec<BallState> {
        let t = self.clamp(t);
        let mut out = Vec::with_capacity(self.balls.len());
        for (index, segments) in self.balls.iter().enumerate() {
            let number = self.numbers.get(index).copied().unwrap_or(index as u8);
            let mut ball = BallState::at_rest(cfg, number, Vec3::ZERO);
            match segment_at(segments, t) {
                Some(segment) => {
                    ball.position = segment.position_at(t);
                    ball.velocity = segment.velocity_at(t);
                    ball.angular_velocity = segment.angular_velocity_at(t);
                    ball.motion_state = segment.kind.motion_state();
                    ball.pocket_state = segment.pocket_state;
                    if segment.pocket_state == PocketState::Pocketed
                        || segment.pocket_state == PocketState::OffTable
                    {
                        ball.is_pocketed = segment.pocket_state == PocketState::Pocketed;
                        ball.is_off_table = segment.pocket_state == PocketState::OffTable;
                        ball.velocity = Vec3::ZERO;
                        ball.angular_velocity = Vec3::ZERO;
                    }
                }
                None => {
                    // The ball was not in play at this time: it was either pocketed
                    // before or after, or it never existed.
                    ball.position = Vec3::ZERO;
                }
            }
            out.push(ball);
        }
        out
    }

    /// The time of the shot, clamped to the shot itself.
    ///
    /// A renderer asks for the position at the time it draws, and the frame it
    /// draws is usually a little past the end of the shot: the last frame of a
    /// game loop is at the shot's duration plus a fraction of a frame. The balls
    /// are where the shot left them, so the time is clamped to the shot and the
    /// last position is given - not the origin, which is what a lookup that
    /// misses every segment would return.
    fn clamp(&self, t: f64) -> f64 {
        if !t.is_finite() {
            return 0.0;
        }
        if t < 0.0 {
            0.0
        } else if t > self.duration {
            self.duration
        } else {
            t
        }
    }

    /// The position of every ball at the time `t`, m.
    ///
    /// This is what a renderer asks for every frame: the positions only, without
    /// the state of each ball around them.
    pub fn sample_positions(&self, t: f64) -> Vec<Vec3> {
        let t = self.clamp(t);
        self.balls
            .iter()
            .map(|segments| match segment_at(segments, t) {
                Some(segment) => segment.position_at(t),
                None => Vec3::ZERO,
            })
            .collect()
    }

    /// The angular velocity of every ball at the time `t`, rad/s.
    pub fn sample_angular(&self, t: f64) -> Vec<Vec3> {
        let t = self.clamp(t);
        self.balls
            .iter()
            .map(|segments| match segment_at(segments, t) {
                Some(segment) => segment.angular_velocity_at(t),
                None => Vec3::ZERO,
            })
            .collect()
    }

    /// The rotation every ball has made by the time `t`, as a rotation vector.
    ///
    /// The rotation vector is the axis of the rotation times the angle, so the
    /// renderer reads the orientation off it: the axis is the direction and the
    /// angle is the length. The angle is not wrapped, so a ball that has rolled
    /// through many turns keeps its phase.
    pub fn sample_rotation(&self, t: f64) -> Vec<Vec3> {
        let t = self.clamp(t);
        self.balls
            .iter()
            .map(|segments| match segment_at(segments, t) {
                Some(segment) => segment.rotation_at(t),
                None => Vec3::ZERO,
            })
            .collect()
    }

    /// The JSON form of the trace.
    pub fn to_json_value(&self) -> Value {
        Value::Object(vec![
            ("duration".into(), Value::Number(self.duration)),
            (
                "events".into(),
                Value::Array(self.events.iter().map(event_value).collect()),
            ),
            (
                "balls".into(),
                Value::Array(
                    self.balls
                        .iter()
                        .enumerate()
                        .map(|(index, segments)| {
                            Value::Object(vec![
                                (
                                    "number".into(),
                                    Value::Number(
                                        self.numbers.get(index).copied().unwrap_or(0) as f64
                                    ),
                                ),
                                (
                                    "segments".into(),
                                    Value::Array(
                                        segments.iter().map(Segment::to_json_value).collect(),
                                    ),
                                ),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }
}

/// The segment of a ball's path that covers the time `t`.
///
/// The segments of a ball are ordered and do not overlap, so the lookup is a
/// binary search. At the time of an event the *later* segment wins: that is the
/// state the ball was left in by the event.
fn segment_at(segments: &[Segment], t: f64) -> Option<&Segment> {
    let mut low = 0usize;
    let mut high = segments.len();
    // The first segment that starts after `t`.
    while low < high {
        let middle = (low + high) / 2;
        if segments[middle].t0 <= t {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    if low == 0 {
        return None;
    }
    let candidate = &segments[low - 1];
    if t <= candidate.t1 + 1e-12 {
        Some(candidate)
    } else {
        None
    }
}

/// The JSON form of an event.
fn event_value(event: &SimEvent) -> Value {
    Value::Object(vec![
        ("t".into(), Value::Number(event.time)),
        (
            "kind".into(),
            Value::String(event_kind_name(event.kind).into()),
        ),
        ("a".into(), Value::Number(event.a as f64)),
        ("b".into(), Value::Number(event.b as f64)),
        ("detail".into(), Value::Number(event.detail as f64)),
    ])
}

/// The name of an event kind, as the renderer sees it.
pub fn event_kind_name(kind: crate::sim::EventKind) -> &'static str {
    use crate::sim::EventKind::*;
    match kind {
        BallBall => "ball-ball",
        Cushion => "cushion",
        Jaw => "jaw",
        SlateBounce => "slate",
        StateChange => "state",
        PocketEnter => "pocket-enter",
        PocketDrop => "pocket-drop",
        PocketExit => "pocket-exit",
        OffTable => "off-table",
    }
}

/// The name of a pocket state, as the renderer sees it.
pub fn pocket_name(state: PocketState) -> &'static str {
    match state {
        PocketState::OnTable => "on-table",
        PocketState::Entering => "entering",
        PocketState::Pocketed => "pocketed",
        PocketState::HungUp => "hung-up",
        PocketState::OffTable => "off-table",
    }
}

/// A vector as a JSON array of three numbers.
fn vec3_value(v: Vec3) -> Value {
    Value::Array(vec![
        Value::Number(v.x),
        Value::Number(v.y),
        Value::Number(v.z),
    ])
}

/// A segment that is still open: it has a start but no end yet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OpenSegment {
    /// The time the segment starts, s.
    pub t0: f64,
    /// The kind of motion in the segment.
    pub kind: SegmentKind,
    /// The position at `t0`, m.
    pub p0: Vec3,
    /// The velocity at `t0`, m/s.
    pub v0: Vec3,
    /// The constant acceleration, m/s^2.
    pub a: Vec3,
    /// The angular velocity at `t0`, rad/s.
    pub w0: Vec3,
    /// The constant angular acceleration, rad/s^2.
    pub wa: Vec3,
    /// The rotation the ball has already made when the segment starts.
    pub spin0: Vec3,
    /// The pocket state at `t0`.
    pub pocket_state: PocketState,
}

impl OpenSegment {
    /// Closes the segment at the time `t1`.
    fn close(self, t1: f64) -> Segment {
        Segment {
            t0: self.t0,
            t1,
            kind: self.kind,
            p0: self.p0,
            v0: self.v0,
            a: self.a,
            w0: self.w0,
            wa: self.wa,
            spin0: self.spin0,
            pocket_state: self.pocket_state,
        }
    }
}

/// The time into a segment at which the segment's model of the vertical spin
/// reaches zero, if it reaches it inside the segment.
///
/// The model of a segment is a straight line: `w(t) = w0 + wa * dt`. The
/// vertical spin is the only part of it that decays - the spin-up of a slide
/// and the rolling spin of a roll go to the two in-plane axes (§3.2, §3.3),
/// the air leaves the spin alone altogether (§7.1) - so a
/// segment whose `w0.z` and `wa.z` have strictly opposite signs is a segment
/// whose model crosses zero part way through it, and from there the model
/// grows a backspin instead of dying.
///
/// The stepped spin of the simulation does not: `cloth`'s decay clamps it at
/// zero (§3.2). A long roll therefore ends with the two disagreeing - and the
/// renderer, which reads the model rather than the state, turns the ball
/// backwards for the rest of the roll. With the defaults that is easy to hit:
/// the decay is `5 mu_sp g / 2R` = about 10 rad/s², a ball that leaves with
/// 20 rad/s of vertical spin dies inside a segment that lasts two seconds, and
/// rolling segments last seconds.
///
/// The crossing is read off the same straight line the samplers use, and it
/// counts only when it is strictly inside the segment: a model that touches
/// zero at the end of the segment does not cross it.
fn spin_death(segment: &OpenSegment, dt: f64) -> Option<f64> {
    let decaying =
        segment.w0.z > 0.0 && segment.wa.z < 0.0 || segment.w0.z < 0.0 && segment.wa.z > 0.0;
    if !decaying {
        return None;
    }
    let death = -(segment.w0.z / segment.wa.z);
    if death < dt {
        Some(death)
    } else {
        None
    }
}

/// The recorder: it collects the path of every ball as the simulation runs.
///
/// The recorder is observational. It never touches the state of the simulation,
/// and it is off unless it is asked for.
#[derive(Clone, Debug, Default)]
pub(crate) struct Recorder {
    /// The trace being recorded.
    pub trace: Trace,
    /// The segment every ball currently has open.
    open: Vec<Option<OpenSegment>>,
    /// The rotation every ball has made so far, as a rotation vector.
    spin: Vec<Vec3>,
}

impl Recorder {
    /// A recorder for a shot with the given balls, from the orientations they
    /// carry into it.
    ///
    /// The tally of rotation every ball has made starts at the orientation that
    /// ball was left turned to by the shot before this one -
    /// [`BallState.rotation`](crate::state), the hand-off of §11.2 seen from
    /// the input side - rather than at zero. A ball that is still lying where
    /// the last shot left it therefore keeps its phase: the path of this shot
    /// counts its rotation from that orientation, and a renderer that played
    /// the last shot back to its last frame plays this one on from there with
    /// nothing integrated in between. A ball that was put on the table carries
    /// `Vec3::ZERO` and starts over. A list that is shorter than the balls
    /// leaves the rest of them at zero.
    pub(crate) fn new(numbers: &[u8], start_rotations: &[Vec3]) -> Recorder {
        let count = numbers.len();
        let mut spin: Vec<Vec3> = Vec::with_capacity(count);
        for index in 0..count {
            spin.push(start_rotations.get(index).copied().unwrap_or(Vec3::ZERO));
        }
        Recorder {
            trace: Trace {
                balls: vec![Vec::new(); count],
                numbers: numbers.to_vec(),
                events: Vec::new(),
                duration: 0.0,
            },
            open: vec![None; count],
            spin,
        }
    }

    /// Opens a segment for a ball.
    ///
    /// The segment is stamped with the rotation the ball has made so far: that is
    /// the state the ball was left in by the event that ended its last segment.
    pub(crate) fn open(&mut self, index: usize, mut segment: OpenSegment) {
        if index < self.open.len() {
            segment.spin0 = self.spin[index];
            self.open[index] = Some(segment);
        }
    }

    /// Closes the open segment of a ball at the time `t1`, and closes it at a
    /// spin death instead if the segment has one inside it.
    ///
    /// The rotation the ball makes over a segment is the integral of a straight
    /// line, so a segment whose model of the vertical spin crosses zero
    /// ([`spin_death`]) keeps that crossing as a rotation of its own: it is
    /// closed there, and a second segment is opened from that time and closed at
    /// `t1`, with the vertical spin and its decay snapped to exactly zero. The
    /// straight line of the first segment then never goes past zero, and the
    /// second one has nothing left to die.
    ///
    /// This is the recorder telling the model what §3.2 tells the spin: a
    /// decay acts towards zero and stops there. It is the recorder and not the
    /// simulation that does it, because the split is a boundary of the *path*
    /// and not an event: the simulation's own stepping is untouched and stays
    /// bit-identical - the same events, the same state, the same hash - and a
    /// segment stays one motion state, so the sampling code stays the one
    /// straight line `w0 + wa * dt`.
    ///
    /// The two halves agree at the time they share: the state of the
    /// continuation is written with the same order of operations as the
    /// samplers of [`Segment`] write it, and the rotation it starts from is the
    /// one the first half accumulated at that time, so `sample_positions` and
    /// `sample_rotation` do not jump there any more than they do at an event.
    pub(crate) fn close(&mut self, index: usize, t1: f64) {
        if index >= self.open.len() {
            return;
        }
        if let Some(open) = self.open[index].take() {
            let dt = t1 - open.t0;
            if let Some(death) = spin_death(&open, dt) {
                // The part up to the death, and from there a segment with
                // nothing left to decay. The rotation is accumulated with the
                // expression `rotation_at` uses, so the two halves meet where
                // the renderer reads them.
                self.spin[index] =
                    self.spin[index] + open.w0 * death + open.wa * (0.5 * death * death);
                self.trace.balls[index].push(open.close(open.t0 + death));
                let w = open.w0 + open.wa * death;
                let again = OpenSegment {
                    t0: open.t0 + death,
                    kind: open.kind,
                    p0: open.p0 + (open.v0 * death + open.a * (0.5 * death * death)),
                    v0: open.v0 + open.a * death,
                    a: open.a,
                    w0: Vec3::new(w.x, w.y, 0.0),
                    wa: Vec3::new(open.wa.x, open.wa.y, 0.0),
                    spin0: Vec3::ZERO,
                    pocket_state: open.pocket_state,
                };
                // The vertical spin is snapped to zero, and its decay with it:
                // the line reaches zero at the split by construction, but not
                // to the last bit, and a hair of residue there would be a spin
                // that is not dead yet - one that the next split would send
                // turning the other way.
                self.open(index, again);
                // The continuation cannot split again: its vertical spin is
                // exactly zero, so `spin_death` does not fire for it.
                self.close(index, t1);
                return;
            }
            // The rotation the ball makes over the segment: the integral of the
            // angular velocity, which is linear over a segment.
            self.spin[index] = self.spin[index] + open.w0 * dt + open.wa * (0.5 * dt * dt);
            self.trace.balls[index].push(open.close(t1));
        }
    }

    /// The number of balls the recorder follows.
    pub(crate) fn len(&self) -> usize {
        self.open.len()
    }
}
