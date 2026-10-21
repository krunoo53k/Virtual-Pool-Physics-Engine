//! The deterministic, event-driven simulation core (§11.2, §11.4).
//!
//! The loop is the one the document prescribes:
//!
//! ```text
//! loop:
//!     for each ball: compute the time until its next event
//!     take the earliest event
//!     advance every ball analytically to that time
//!     apply the event
//! ```
//!
//! Between two events every ball's state is given by the closed-form expressions of
//! §3 (cloth) and §7.1 (air), so nothing is ever integrated step by step.

use crate::ball_ball;
use crate::cloth::{self, ClothParams};
use crate::config::TableConfig;
use crate::cue;
use crate::cushion::{self, RailRegime};
use crate::geometry::TableGeometry;
use crate::math::{Vec2, Vec3};
use crate::pocket;
use crate::state::{BallState, GamePhase, GameState, MotionState, PocketState, ShotInput};

/// An error raised by the simulation. The core never panics; a fault is reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimError {
    /// What went wrong.
    pub message: String,
}

impl SimError {
    fn new(message: impl Into<String>) -> SimError {
        SimError {
            message: message.into(),
        }
    }
}

/// The kinds of event the core resolves.
///
/// The declaration order *is* the tie-break order of §11.4: events at the same
/// instant are resolved by `(time, event kind, ball a, ball b)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    /// Two balls touch.
    BallBall,
    /// A ball meets a rail.
    Cushion,
    /// A ball meets a pocket facing.
    Jaw,
    /// A ball lands on the slate.
    SlateBounce,
    /// A ball changes motion state (slide to roll, roll to rest, spin to rest).
    StateChange,
    /// A ball's centre crosses a pocket's mouth line.
    PocketEnter,
    /// A ball's centre crosses a pocket's drop line.
    PocketDrop,
    /// A ball's centre crosses back over a pocket's mouth line (a rattle out).
    PocketExit,
    /// A ball leaves the table.
    OffTable,
}

impl EventKind {
    /// The tie-break rank of this event kind.
    pub fn rank(self) -> u8 {
        match self {
            EventKind::BallBall => 0,
            EventKind::Cushion => 1,
            EventKind::Jaw => 2,
            EventKind::SlateBounce => 3,
            EventKind::StateChange => 4,
            EventKind::PocketEnter => 5,
            EventKind::PocketDrop => 6,
            EventKind::PocketExit => 7,
            EventKind::OffTable => 8,
        }
    }
}

/// A resolved event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SimEvent {
    /// The time of the event, s.
    pub time: f64,
    /// The kind of the event.
    pub kind: EventKind,
    /// The primary ball.
    pub a: u8,
    /// The secondary ball, the rail index or the pocket index.
    pub b: u8,
    /// Extra information: the rail regime for cushion events.
    pub detail: u8,
}

impl SimEvent {
    /// The total order used to pick the earliest event (§11.4).
    pub fn order_key(&self) -> (f64, u8, u8, u8) {
        (self.time, self.kind.rank(), self.a, self.b)
    }
}

/// The deterministic simulation.
pub struct Simulation<'a> {
    cfg: &'a TableConfig,
    geom: TableGeometry,
    cloth: ClothParams,
    balls: Vec<BallState>,
    time: f64,
    events: Vec<SimEvent>,
}

impl<'a> Simulation<'a> {
    /// Builds a simulation from the shot input and the table configuration.
    pub fn new(input: &ShotInput, cfg: &'a TableConfig) -> Simulation<'a> {
        let geom = TableGeometry::new(cfg);
        let cloth = ClothParams {
            mu_s: cfg.cloth.mu_s,
            mu_r: cfg.cloth.mu_r,
            mu_sp: cfg.cloth.mu_sp,
            g: cfg.simulation.gravity,
            radius: cfg.ball.radius,
        };
        let mut balls: Vec<BallState> = input.ball_states.clone();
        // The physical properties come from the configuration, not from the input.
        for ball in balls.iter_mut() {
            ball.mass = cfg.ball.mass;
            ball.radius = cfg.ball.radius;
            ball.moment_of_inertia = cfg.moment_of_inertia();
        }
        Simulation {
            cfg,
            geom,
            cloth,
            balls,
            time: 0.0,
            events: Vec::new(),
        }
    }
    /// The balls of the simulation.
    pub fn balls(&self) -> &[BallState] {
        &self.balls
    }

    /// Derives the motion state of every ball from its velocity and spin.
    ///
    /// The input carries the physical state of the balls; the motion state is a
    /// derived quantity (§3.1), so it is worked out here rather than taken on
    /// trust.
    fn refresh_all_states(&mut self) {
        for index in 0..self.balls.len() {
            self.refresh_bed_state(index);
        }
    }

    /// The event log.
    pub fn events(&self) -> &[SimEvent] {
        &self.events
    }

    /// The simulated time.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Runs the shot to completion.
    pub fn run(&mut self, input: &ShotInput) -> Result<(), SimError> {
        self.refresh_all_states();
        self.apply_cue_strike(input);
        self.run_events()
    }

    /// Runs the simulation from the current state, without applying a cue strike.
    ///
    /// This is what a fixture needs: the setup is the complete initial state of the
    /// shot, so the balls are already moving.
    pub fn run_without_strike(&mut self) -> Result<(), SimError> {
        self.refresh_all_states();
        self.run_events()
    }

    /// The event loop.
    fn run_events(&mut self) -> Result<(), SimError> {
        let mut guard: u32 = 0;
        while self.time < self.cfg.simulation.max_sim_time {
            guard += 1;
            if guard > self.cfg.simulation.max_events {
                return Err(SimError::new("the event limit was reached"));
            }
            if self.all_settled() {
                break;
            }
            let candidates = self.collect_candidates();
            let event = match select_earliest(candidates) {
                Some(e) => e,
                None => break,
            };
            // The simulation stops at the time limit: an event beyond it is not
            // applied, so the state is always the one at `max_sim_time`.
            if event.time > self.cfg.simulation.max_sim_time {
                self.time = self.cfg.simulation.max_sim_time;
                break;
            }
            let dt = event.time - self.time;
            // The negated comparison is deliberate: it also catches a NaN.
            #[allow(clippy::neg_cmp_op_on_partial_ord)]
            if !(dt >= 0.0) || !dt.is_finite() {
                return Err(SimError::new("a non-finite time step"));
            }
            self.advance_all(dt);
            self.time = event.time;
            self.apply(event)?;

            self.events.push(event);
        }
        Ok(())
    }

    /// True when no ball can move any more.
    fn all_settled(&self) -> bool {
        self.balls.iter().all(|b| b.is_settled())
    }

    /// Applies the cue strike to the cue ball (§6.2).
    fn apply_cue_strike(&mut self, input: &ShotInput) {
        let strike = cue::strike(input, self.cfg);
        if self.balls.is_empty() {
            return;
        }
        {
            let ball = &mut self.balls[0];
            ball.velocity = strike.velocity;
            ball.angular_velocity = strike.angular_velocity;
            ball.position.z = self.cfg.ball.radius;
        }
        if strike.velocity.z < 0.0 {
            // An elevated cue drives the ball into the slate, and the slate is what
            // launches it (§7.1).
            let rebound = -self.cfg.slate.e_table * strike.velocity.z;
            let ball = &mut self.balls[0];
            ball.velocity.z = rebound;
            ball.motion_state = MotionState::Airborne;
        } else {
            self.refresh_bed_state(0);
        }
    }

    /// Recomputes a ball's motion state from its velocity and spin.
    fn refresh_bed_state(&mut self, index: usize) {
        let radius = self.cfg.ball.radius;
        let ball = &mut self.balls[index];
        if ball.pocket_state != PocketState::OnTable {
            return;
        }
        if ball.velocity.z > 0.0 || ball.position.z > radius + 1e-9 {
            ball.motion_state = MotionState::Airborne;
            return;
        }
        let speed = ball.velocity.x.hypot(ball.velocity.y);
        if speed <= cloth::VELOCITY_EPS {
            let spin = ball.angular_velocity.z.abs();
            let in_plane = ball.angular_velocity.x.abs() + ball.angular_velocity.y.abs();
            ball.motion_state = if spin > cloth::VELOCITY_EPS {
                MotionState::Spinning
            } else if in_plane > cloth::VELOCITY_EPS {
                MotionState::Sliding
            } else {
                MotionState::Stationary
            };
            return;
        }
        if cloth::is_rolling(ball.velocity, ball.angular_velocity, radius) {
            ball.motion_state = MotionState::Rolling;
        } else {
            ball.motion_state = MotionState::Sliding;
        }
    }

    /// Advances every ball analytically by `dt`.
    fn advance_all(&mut self, dt: f64) {
        for ball in self.balls.iter_mut() {
            if ball.is_pocketed || ball.is_off_table {
                continue;
            }
            if ball.pocket_state == PocketState::Entering {
                cloth::advance_shelf(
                    ball,
                    dt,
                    self.cfg.simulation.gravity,
                    self.cfg.table.mu_slate,
                );
                continue;
            }
            match ball.motion_state {
                MotionState::Sliding => cloth::advance_sliding(ball, dt, &self.cloth),
                MotionState::Rolling => cloth::advance_rolling(ball, dt, &self.cloth),
                MotionState::Spinning => cloth::advance_spinning(ball, dt, &self.cloth),
                MotionState::Stationary => {}
                MotionState::Airborne => {
                    cloth::advance_airborne(ball, dt, self.cfg.simulation.gravity)
                }
            }
        }
    }

    /// The time until the ball's next motion-state change, if any.
    fn next_state_change(&self, index: usize) -> Option<(f64, EventKind)> {
        let ball = &self.balls[index];
        if ball.pocket_state == PocketState::Entering {
            // On the shelf the ball only loses speed: it stops, and a ball that stops
            // before the drop line has hung up (§9.4).
            let speed = ball.velocity.x.hypot(ball.velocity.y);
            if speed <= cloth::VELOCITY_EPS {
                return None;
            }
            let stop = speed / (self.cfg.table.mu_slate * self.cfg.simulation.gravity);
            return Some((stop, EventKind::StateChange));
        }
        match ball.motion_state {
            MotionState::Sliding => {
                let u = cloth::slip_velocity(ball.velocity, ball.angular_velocity, ball.radius);
                Some((
                    cloth::time_to_rolling(u.norm(), &self.cloth),
                    EventKind::StateChange,
                ))
            }
            MotionState::Rolling => {
                let speed = ball.velocity.x.hypot(ball.velocity.y);
                Some((
                    cloth::time_to_rest(speed, &self.cloth),
                    EventKind::StateChange,
                ))
            }
            MotionState::Spinning => Some((
                cloth::time_to_rest_spinning(ball.angular_velocity.z, &self.cloth),
                EventKind::StateChange,
            )),
            MotionState::Airborne => Some((
                cloth::time_to_land(
                    ball.position.z,
                    ball.velocity.z,
                    self.cfg.ball.radius,
                    self.cfg.simulation.gravity,
                ),
                EventKind::SlateBounce,
            )),
            MotionState::Stationary => None,
        }
    }

    /// The horizon for a ball's own events: the time until its state changes.
    fn horizon(&self, index: usize) -> f64 {
        match self.next_state_change(index) {
            Some((dt, _)) => dt,
            None => f64::INFINITY,
        }
    }

    /// Collects every candidate event.
    fn collect_candidates(&self) -> Vec<SimEvent> {
        let mut out: Vec<SimEvent> = Vec::new();
        let count = self.balls.len();
        let mut horizons = vec![f64::INFINITY; count];

        // First the events of the individual balls. The earliest of them bounds how
        // far the ball-ball collisions have to be looked for: a collision that would
        // happen later is found in the next round, after the earlier event has been
        // resolved.
        for index in 0..count {
            let ball = &self.balls[index];
            if ball.is_pocketed || ball.is_off_table {
                continue;
            }
            let horizon = self.horizon(index);

            if let Some((dt, kind)) = self.next_state_change(index) {
                out.push(SimEvent {
                    time: self.time + dt,
                    kind,
                    a: index as u8,
                    b: 0,
                    detail: 0,
                });
                horizons[index] = horizons[index].min(self.time + dt);
            }

            let before = out.len();
            self.collect_cushion_events(index, horizon, &mut out);
            self.collect_pocket_events(index, horizon, &mut out);
            self.collect_jaw_events(index, horizon, &mut out);
            self.collect_off_table_events(index, horizon, &mut out);
            for event in &out[before..] {
                horizons[index] = horizons[index].min(event.time);
            }
        }

        // Then the ball-ball collisions.
        for i in 0..count {
            for j in (i + 1)..count {
                let a = &self.balls[i];
                let b = &self.balls[j];
                if a.is_pocketed || a.is_off_table || b.is_pocketed || b.is_off_table {
                    continue;
                }
                let horizon = horizons[i].min(horizons[j]) - self.time;
                if !(horizon > 0.0) {
                    continue;
                }
                let accel_a = self.segment_acceleration(i);
                let accel_b = self.segment_acceleration(j);
                // A cheap broad phase: the distance between the two balls cannot
                // shrink to 2R within the segment if the relative motion is not
                // enough.
                let relative_speed = (b.velocity - a.velocity).norm();
                let relative_acceleration = (accel_b - accel_a).norm();
                let reach =
                    relative_speed * horizon + 0.5 * relative_acceleration * horizon * horizon;
                if (b.position - a.position).norm() - reach > 2.0 * a.radius {
                    continue;
                }
                if let Some(t) = ball_ball::time_of_impact(a, b, accel_a, accel_b, horizon) {
                    out.push(SimEvent {
                        time: self.time + t,
                        kind: EventKind::BallBall,
                        a: i as u8,
                        b: j as u8,
                        detail: 0,
                    });
                }
            }
        }

        out
    }

    /// The constant acceleration of a ball over the current segment.
    fn segment_acceleration(&self, index: usize) -> Vec3 {
        let ball = &self.balls[index];
        if ball.pocket_state == PocketState::Entering {
            cloth::shelf_acceleration(ball, self.cfg.simulation.gravity, self.cfg.table.mu_slate)
        } else {
            cloth::segment_acceleration(ball, &self.cloth)
        }
    }

    /// Collects the rail events for one ball.
    fn collect_cushion_events(&self, index: usize, horizon: f64, out: &mut Vec<SimEvent>) {
        let ball = &self.balls[index];
        if ball.pocket_state == PocketState::Entering {
            return;
        }
        let accel = self.segment_acceleration(index);
        for (rail_index, rail) in self.geom.rails.iter().enumerate() {
            // The ball has to head towards the rail.
            let approach = ball.velocity.x * rail.normal.x + ball.velocity.y * rail.normal.y;
            if approach <= 0.0 {
                continue;
            }
            let (dt, regime) = match cushion::time_of_impact(
                ball,
                accel,
                rail,
                &self.geom.cushion,
                self.cfg,
                horizon,
            ) {
                Some(v) => v,
                None => continue,
            };
            // The contact has to be on a cushion nose, not in a pocket mouth.
            let p = ball.position + ball.velocity * dt + accel * (0.5 * dt * dt);
            let along = p.x * rail.tangent.x + p.y * rail.tangent.y;
            let on_cushion = rail
                .cushion_spans
                .iter()
                .any(|(lo, hi)| along >= *lo - 1e-9 && along <= *hi + 1e-9);
            if !on_cushion {
                continue;
            }
            out.push(SimEvent {
                time: self.time + dt,
                kind: EventKind::Cushion,
                a: index as u8,
                b: rail_index as u8,
                detail: match regime {
                    RailRegime::NoseRubber => 0,
                    RailRegime::RailStructure => 1,
                    RailRegime::ClearOfRail => 2,
                },
            });
        }
    }

    /// Collects the pocket events for one ball.
    fn collect_pocket_events(&self, index: usize, horizon: f64, out: &mut Vec<SimEvent>) {
        let ball = &self.balls[index];
        let accel = self.segment_acceleration(index);
        let plan_accel = Vec2::new(accel.x, accel.y);
        for (pocket_index, pocket_geometry) in self.geom.pockets.iter().enumerate() {
            match ball.pocket_state {
                PocketState::Entering => {
                    if let Some(t) =
                        pocket::time_to_cross_drop(ball, plan_accel, pocket_geometry, horizon)
                    {
                        out.push(SimEvent {
                            time: self.time + t,
                            kind: EventKind::PocketDrop,
                            a: index as u8,
                            b: pocket_index as u8,
                            detail: 0,
                        });
                    }
                    // A rattle out: the centre crosses the mouth line again, this
                    // time in the other direction.
                    if let Some(t) = pocket::time_to_cross_mouth(
                        ball,
                        plan_accel,
                        pocket_geometry,
                        -1.0,
                        horizon,
                    ) {
                        out.push(SimEvent {
                            time: self.time + t,
                            kind: EventKind::PocketExit,
                            a: index as u8,
                            b: pocket_index as u8,
                            detail: 0,
                        });
                    }
                }
                _ => {
                    if let Some(t) =
                        pocket::time_to_cross_mouth(ball, plan_accel, pocket_geometry, 1.0, horizon)
                    {
                        // The ball has to cross the mouth line between the jaw tips.
                        let p = ball.position + ball.velocity * t + accel * (0.5 * t * t);
                        let lateral = pocket_geometry.lateral(Vec2::new(p.x, p.y));
                        if lateral.abs() > pocket_geometry.mouth_width / 2.0 {
                            continue;
                        }
                        out.push(SimEvent {
                            time: self.time + t,
                            kind: EventKind::PocketEnter,
                            a: index as u8,
                            b: pocket_index as u8,
                            detail: 0,
                        });
                    }
                }
            }
        }
    }

    /// Collects the jaw (facing) events for one ball.
    fn collect_jaw_events(&self, index: usize, horizon: f64, out: &mut Vec<SimEvent>) {
        let ball = &self.balls[index];
        if ball.pocket_state == PocketState::OnTable
            && ball.position.z > self.cfg.ball.radius + 1e-9
        {
            // A ball in flight over a facing is handled by the rail regimes.
            return;
        }
        let accel = self.segment_acceleration(index);
        for (pocket_index, pocket_geometry) in self.geom.pockets.iter().enumerate() {
            for (jaw_index, jaw) in pocket_geometry.jaws.iter().enumerate() {
                if let Some(t) = pocket::jaw_time_of_impact(ball, accel, jaw, horizon) {
                    out.push(SimEvent {
                        time: self.time + t,
                        kind: EventKind::Jaw,
                        a: index as u8,
                        b: pocket_index as u8,
                        detail: jaw_index as u8,
                    });
                }
            }
        }
    }

    /// Collects the off-table events for one ball (§10.1).
    fn collect_off_table_events(&self, index: usize, horizon: f64, out: &mut Vec<SimEvent>) {
        let ball = &self.balls[index];
        if ball.pocket_state != PocketState::OnTable {
            return;
        }
        let accel = self.segment_acceleration(index);
        for rail in self.geom.rails.iter() {
            let x0 = ball.position.x * rail.normal.x + ball.position.y * rail.normal.y;
            let vx = ball.velocity.x * rail.normal.x + ball.velocity.y * rail.normal.y;
            if vx <= 0.0 {
                continue;
            }
            let ax = accel.x * rail.normal.x + accel.y * rail.normal.y;
            // The crossing of the nose line.
            let coeffs = [0.5 * ax, vx, x0 - rail.line_coord];
            let roots = crate::quartic::quadratic_roots_abc(coeffs[0], coeffs[1], coeffs[2]);
            for t in roots {
                if !(t > 0.0 && t <= horizon) {
                    continue;
                }
                let z = ball.position.z + ball.velocity.z * t + accel.z * (0.5 * t * t);
                // Only a ball that clears the rail leaves the table.
                if z - ball.radius <= self.cfg.cushion.rail_top_height {
                    continue;
                }
                out.push(SimEvent {
                    time: self.time + t,
                    kind: EventKind::OffTable,
                    a: index as u8,
                    b: 0,
                    detail: 0,
                });
            }
        }
    }

    /// Applies an event.
    fn apply(&mut self, event: SimEvent) -> Result<(), SimError> {
        match event.kind {
            EventKind::BallBall => self.apply_ball_ball(event.a as usize, event.b as usize),
            EventKind::Cushion => self.apply_cushion(
                event.a as usize,
                event.b as usize,
                match event.detail {
                    1 => RailRegime::RailStructure,
                    _ => RailRegime::NoseRubber,
                },
            ),
            EventKind::Jaw => {
                self.apply_jaw(event.a as usize, event.b as usize, event.detail as usize)
            }
            EventKind::SlateBounce => self.apply_slate_bounce(event.a as usize),
            EventKind::StateChange => self.apply_state_change(event.a as usize),
            EventKind::PocketEnter => self.apply_pocket_enter(event.a as usize),
            EventKind::PocketDrop => self.apply_pocket_drop(event.a as usize),
            EventKind::PocketExit => self.apply_pocket_exit(event.a as usize),
            EventKind::OffTable => self.apply_off_table(event.a as usize),
        }
        Ok(())
    }

    /// Applies a ball-ball collision (§4.4.5, §4.5).
    fn apply_ball_ball(&mut self, a: usize, b: usize) {
        let outcome = ball_ball::resolve(&self.balls[a], &self.balls[b], self.cfg);
        {
            let ball = &mut self.balls[a];
            ball.velocity = outcome.velocity_a;
            ball.angular_velocity = outcome.angular_velocity_a;
        }
        {
            let ball = &mut self.balls[b];
            ball.velocity = outcome.velocity_b;
            ball.angular_velocity = outcome.angular_velocity_b;
        }
        // The table can lift the striking ball off the cloth (§4.5). The lift comes
        // from the cloth's friction during the collision, so it only applies while
        // the ball is still in contact with the bed: a ball that is already in the
        // air cannot pick up any more.
        if outcome.lift > 0.0 {
            let on_the_bed = self.balls[a].motion_state != MotionState::Airborne
                && self.balls[a].position.z <= self.cfg.ball.radius + 1e-9;
            if on_the_bed {
                let ball = &mut self.balls[a];
                ball.velocity.z += outcome.lift;
                ball.position.z = self.cfg.ball.radius;
                ball.motion_state = MotionState::Airborne;
            }
        } else {
            self.refresh_bed_state(a);
        }
        self.refresh_bed_state(b);
    }

    /// Applies a cushion impact (§5.1, §8.1).
    fn apply_cushion(&mut self, index: usize, rail_index: usize, regime: RailRegime) {
        let rail = &self.geom.rails[rail_index];
        let normal = Vec3::new(rail.normal.x, rail.normal.y, 0.0);
        let tangent = Vec3::new(rail.tangent.x, rail.tangent.y, 0.0);
        let outcome = match regime {
            RailRegime::NoseRubber => cushion::resolve_nose_cushion(
                &self.balls[index],
                normal,
                tangent,
                self.cfg,
                &self.geom.cushion,
            ),
            RailRegime::RailStructure => cushion::resolve_cushion(
                &self.balls[index],
                normal,
                tangent,
                0.0,
                1.0,
                self.cfg.cushion.e_rail_structure,
                self.cfg.cushion.mu_rail_structure,
                self.cfg,
            ),
            RailRegime::ClearOfRail => return,
        };
        let ball = &mut self.balls[index];
        ball.velocity = outcome.velocity;
        ball.angular_velocity = outcome.angular_velocity;
        match regime {
            RailRegime::NoseRubber => {
                let vz = ball.velocity.z;
                if vz < 0.0 {
                    // The cushion presses the ball into the slate; the slate's rebound
                    // is what makes it hop (§8.1).
                    let mut rebound = -self.cfg.slate.e_table * vz;
                    // The compliant-contact correction of §8.1: the rigid model
                    // over-predicts the hop at break speed.
                    if rebound > self.cfg.slate.max_cushion_rebound {
                        rebound = self.cfg.slate.max_cushion_rebound;
                    }
                    ball.velocity.z = rebound;
                    ball.position.z = self.cfg.ball.radius;
                    ball.motion_state = MotionState::Airborne;
                } else if vz > self.cfg.slate.settle_speed {
                    ball.position.z = self.cfg.ball.radius;
                    ball.motion_state = MotionState::Airborne;
                } else {
                    ball.velocity.z = 0.0;
                    self.refresh_bed_state(index);
                }
            }
            _ => {
                // The rail structure is a vertical wall: the vertical velocity is
                // unchanged, the ball keeps falling.
                if ball.velocity.z > 0.0 || ball.position.z > self.cfg.ball.radius + 1e-9 {
                    ball.motion_state = MotionState::Airborne;
                } else {
                    ball.velocity.z = 0.0;
                    self.refresh_bed_state(index);
                }
            }
        }
    }

    /// Applies a jaw impact (§9.3).
    fn apply_jaw(&mut self, index: usize, pocket_index: usize, jaw_index: usize) {
        let jaw = self.geom.pockets[pocket_index].jaws[jaw_index];
        let outcome = pocket::resolve_jaw(&self.balls[index], &jaw, self.cfg);
        let ball = &mut self.balls[index];
        ball.velocity = outcome.velocity;
        ball.angular_velocity = outcome.angular_velocity;
        if ball.velocity.z > 0.0 || ball.position.z > self.cfg.ball.radius + 1e-9 {
            ball.motion_state = MotionState::Airborne;
        } else {
            ball.velocity.z = 0.0;
            self.refresh_bed_state(index);
        }
    }

    /// Applies the ball-slate bounce (§7.2, §8.2).
    fn apply_slate_bounce(&mut self, index: usize) {
        let ball = &mut self.balls[index];
        ball.position.z = self.cfg.ball.radius;
        let rebound = -self.cfg.slate.e_table * ball.velocity.z;
        if rebound.abs() < self.cfg.slate.settle_speed {
            ball.velocity.z = 0.0;
            self.refresh_bed_state(index);
        } else {
            ball.velocity.z = rebound;
            ball.motion_state = MotionState::Airborne;
        }
    }

    /// Applies a motion-state change (§3.5).
    fn apply_state_change(&mut self, index: usize) {
        let radius = self.cfg.ball.radius;
        let ball = &mut self.balls[index];
        if ball.pocket_state == PocketState::Entering {
            // The ball ran out of speed on the shelf: it hung up (§9.4). It stays on
            // the shelf as a stopped ball until the rules resolve it (§9.7).
            ball.velocity = Vec3::ZERO;
            ball.angular_velocity = Vec3::ZERO;
            ball.motion_state = MotionState::Stationary;
            ball.pocket_state = PocketState::HungUp;
            return;
        }
        match ball.motion_state {
            MotionState::Sliding => {
                // The slip has vanished: the ball rolls from now on.
                let w = cloth::rolling_angular_velocity(ball.velocity, radius);
                ball.angular_velocity.x = w.x;
                ball.angular_velocity.y = w.y;
                ball.motion_state = MotionState::Rolling;
            }
            MotionState::Rolling => {
                ball.velocity.x = 0.0;
                ball.velocity.y = 0.0;
                ball.velocity.z = 0.0;
                ball.angular_velocity.x = 0.0;
                ball.angular_velocity.y = 0.0;
                ball.motion_state = if ball.angular_velocity.z.abs() > cloth::VELOCITY_EPS {
                    MotionState::Spinning
                } else {
                    MotionState::Stationary
                };
            }
            MotionState::Spinning => {
                ball.angular_velocity.z = 0.0;
                ball.motion_state = MotionState::Stationary;
            }
            MotionState::Airborne | MotionState::Stationary => {}
        }
    }

    /// The ball's centre has crossed the mouth line: it is on the shelf (§9.2).
    fn apply_pocket_enter(&mut self, index: usize) {
        self.balls[index].pocket_state = PocketState::Entering;
    }

    /// The ball's centre has crossed the drop line: it is in the cabinet (§9.4).
    fn apply_pocket_drop(&mut self, index: usize) {
        let ball = &mut self.balls[index];
        ball.pocket_state = PocketState::Pocketed;
        ball.is_pocketed = true;
        ball.velocity = Vec3::ZERO;
        ball.angular_velocity = Vec3::ZERO;
        ball.motion_state = MotionState::Stationary;
    }

    /// The ball has come back out of the pocket: a rattle out (§9.2).
    fn apply_pocket_exit(&mut self, index: usize) {
        self.balls[index].pocket_state = PocketState::OnTable;
        self.refresh_bed_state(index);
    }

    /// The ball has left the table (§10.1).
    fn apply_off_table(&mut self, index: usize) {
        let ball = &mut self.balls[index];
        ball.is_off_table = true;
        ball.pocket_state = PocketState::OffTable;
        ball.velocity = Vec3::ZERO;
        ball.angular_velocity = Vec3::ZERO;
        ball.motion_state = MotionState::Stationary;
    }
}

/// Picks the earliest event using the total order of §11.4.
fn select_earliest(candidates: Vec<SimEvent>) -> Option<SimEvent> {
    let mut best: Option<SimEvent> = None;
    for event in candidates {
        if !event.time.is_finite() {
            continue;
        }
        best = match best {
            None => Some(event),
            Some(current) => {
                if event.order_key() < current.order_key() {
                    Some(event)
                } else {
                    Some(current)
                }
            }
        };
    }
    best
}

/// Simulates a shot and returns the resulting game state.
///
/// This is the entry point of §12.4. It never panics: a fault in the model is
/// reported through the phase of the returned state.
pub fn simulate_shot(input: ShotInput, table: &TableConfig) -> GameState {
    match try_simulate_shot(&input, table) {
        Ok(state) => state,
        Err(error) => {
            let mut state = GameState::new(input.ball_states.clone());
            state.phase = GamePhase::Faulted;
            state.foul = true;
            state
                .balls
                .iter_mut()
                .for_each(|ball| ball.motion_state = MotionState::Stationary);
            // The error text is kept out of the state so that the state stays a pure
            // function of the physics; `try_simulate_shot` carries the detail.
            let _ = error;
            state
        }
    }
}

/// Simulates a shot, reporting any fault.
pub fn try_simulate_shot(input: &ShotInput, table: &TableConfig) -> Result<GameState, SimError> {
    let mut simulation = Simulation::new(input, table);
    let result = simulation.run(input);
    finish(simulation, result, input, table)
}

/// Simulates a game state that is already complete, without a cue strike.
///
/// The fixtures describe a complete initial state, so this is the entry point they
/// use: nothing is struck, the balls simply evolve.
pub fn try_simulate_state(input: &ShotInput, table: &TableConfig) -> Result<GameState, SimError> {
    let mut simulation = Simulation::new(input, table);
    let result = simulation.run_without_strike();
    finish(simulation, result, input, table)
}

fn finish(
    simulation: Simulation<'_>,
    result: Result<(), SimError>,
    input: &ShotInput,
    table: &TableConfig,
) -> Result<GameState, SimError> {
    let mut state = GameState::new(simulation.balls().to_vec());
    state.elapsed_time = simulation.time();
    // The phase says whether the shot is over: a shot that ran into the time limit
    // is still going (§12.2).
    state.phase = if simulation.balls().iter().all(|b| b.is_settled()) {
        GamePhase::Settled
    } else {
        GamePhase::Settling
    };
    // The cue stick, as the renderer wants it (§12.2).
    state.cue_stick = cue_stick_state(input, table, &simulation.balls());
    // The miscue flag comes from the cue model (§6.3).
    let strike = cue::strike(input, table);
    state.miscue = strike.miscue;
    if strike.miscue {
        state.foul = true;
    }
    // A ball that left the table is a foul (§10.1).
    if simulation.balls().iter().any(|b| b.is_off_table) {
        state.foul = true;
    }
    match result {
        Ok(()) => Ok(state),
        Err(error) => {
            state.phase = GamePhase::Faulted;
            Err(error)
        }
    }
}

/// Where the cue was when the ball was struck.
fn cue_stick_state(
    input: &ShotInput,
    table: &TableConfig,
    balls: &[BallState],
) -> crate::state::CueStickState {
    let direction = match input.cue_direction.normalize() {
        Some(d) => d,
        None => return crate::state::CueStickState::default(),
    };
    let tip = match balls.first() {
        Some(ball) => ball.position - direction * ball.radius,
        None => return crate::state::CueStickState::default(),
    };
    crate::state::CueStickState {
        butt_position: tip - direction * table.cue.cue_length,
        direction,
        elevation: libm::asin(direction.z.clamp(-1.0, 1.0)),
        tip_offset: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::BallState;

    fn table() -> TableConfig {
        TableConfig::default()
    }

    #[test]
    fn a_straight_shot_into_the_ball_settles() {
        let cfg = table();
        let mut balls = Vec::new();
        balls.push(BallState::at_rest(
            &cfg,
            0,
            Vec3::new(0.5, 0.635, cfg.ball.radius),
        ));
        balls.push(BallState::at_rest(
            &cfg,
            1,
            Vec3::new(0.9, 0.635, cfg.ball.radius),
        ));
        let input = ShotInput::new(1.5, Vec3::X, balls);
        let state = try_simulate_shot(&input, &cfg).expect("the shot runs");
        assert_eq!(state.phase, GamePhase::Settled);
        // The object ball has moved.
        assert!(state.balls[1].position.x > 0.9);
    }

    #[test]
    fn the_same_shot_gives_the_same_state() {
        let cfg = table();
        let mut balls = Vec::new();
        balls.push(BallState::at_rest(
            &cfg,
            0,
            Vec3::new(0.5, 0.635, cfg.ball.radius),
        ));
        balls.push(BallState::at_rest(
            &cfg,
            1,
            Vec3::new(0.9, 0.635, cfg.ball.radius),
        ));
        let input = ShotInput::new(1.5, Vec3::X, balls);
        let a = try_simulate_shot(&input, &cfg).expect("the shot runs");
        let b = try_simulate_shot(&input, &cfg).expect("the shot runs");
        assert_eq!(a, b);
    }
}
