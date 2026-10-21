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
//! hash are the same with and without it.

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
    /// Sliding on a pocket shelf (§9.4).
    Shelf,
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
            SegmentKind::Shelf => "shelf",
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
            // A ball on the shelf is sliding; the shelf is what tells it apart.
            SegmentKind::Shelf => MotionState::Sliding,
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
            pocket_state: self.pocket_state,
        }
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
}

impl Recorder {
    /// A recorder for a shot with the given balls.
    pub(crate) fn new(numbers: &[u8]) -> Recorder {
        let count = numbers.len();
        Recorder {
            trace: Trace {
                balls: vec![Vec::new(); count],
                numbers: numbers.to_vec(),
                events: Vec::new(),
                duration: 0.0,
            },
            open: vec![None; count],
        }
    }

    /// Opens a segment for a ball.
    pub(crate) fn open(&mut self, index: usize, segment: OpenSegment) {
        if index < self.open.len() {
            self.open[index] = Some(segment);
        }
    }

    /// Closes the open segment of a ball at the time `t1`.
    pub(crate) fn close(&mut self, index: usize, t1: f64) {
        if index >= self.open.len() {
            return;
        }
        if let Some(open) = self.open[index].take() {
            self.trace.balls[index].push(open.close(t1));
        }
    }

    /// The number of balls the recorder follows.
    pub(crate) fn len(&self) -> usize {
        self.open.len()
    }
}
