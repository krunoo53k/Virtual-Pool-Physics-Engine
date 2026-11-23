//! The rails: a ball that meets a rail bounces off it.
//!
//! A ball that is on the bed is stopped by the cushion. A ball that is in the
//! air can go over the rail - that is a ball that has left the table, and the
//! core has to say so - but it must never go *through* it. The two are easy to
//! confuse: the contact condition of a rail is a plane, and an airborne ball
//! meets that plane at a height that may be above the rail, below it, or on it.

use pool_core::config::TableConfig;
use pool_core::math::Vec3;
use pool_core::sim::EventKind;
use pool_core::state::{BallState, MotionState, PocketState, ShotInput};
use pool_core::{try_simulate_shot_traced, try_simulate_state, GamePhase};

/// The distance from the middle of the table to the rail, along `x`.
fn half_length(cfg: &TableConfig) -> f64 {
    cfg.table.length / 2.0
}

/// A ball with a velocity, ready to be shot.
fn ball(cfg: &TableConfig, position: Vec3, velocity: Vec3) -> BallState {
    let mut ball = BallState::at_rest(cfg, 0, position);
    ball.velocity = velocity;
    ball.motion_state = if velocity.z > 0.0 || position.z > cfg.ball.radius + 1e-9 {
        MotionState::Airborne
    } else {
        MotionState::Stationary
    };
    ball
}

/// Runs a ball with no cue strike, and gives back the state and the events.
fn run(cfg: &TableConfig, ball: BallState) -> (pool_core::GameState, pool_core::Trace) {
    let input = pool_core::ShotInput::new(0.0, Vec3::X, vec![ball]);
    let mut simulation = pool_core::Simulation::new(&input, cfg);
    simulation.record();
    simulation.run_without_strike().expect("the ball runs");
    let trace = simulation.trace().cloned().expect("the path was recorded");
    let state = pool_core::GameState::new(simulation.balls().to_vec());
    (state, trace)
}

#[test]
fn a_ball_on_the_bed_bounces_off_the_rail() {
    let cfg = TableConfig::default();
    let start = Vec3::new(0.4, 0.635, cfg.ball.radius);
    let ball = ball(&cfg, start, Vec3::new(-4.0, 0.0, 0.0));
    let (state, trace) = run(&cfg, ball);

    let cushions = trace
        .events
        .iter()
        .filter(|event| event.kind == EventKind::Cushion)
        .count();
    assert!(cushions > 0, "the ball never met a rail");
    assert!(
        !state.balls[0].is_off_table,
        "the ball rolled through the rail and off the table"
    );
    // The ball is still on the table: it never crossed the nose line.
    assert!(
        state.balls[0].position.x > 0.0,
        "the ball ended past the rail at {:?}",
        state.balls[0].position
    );
}

#[test]
fn an_airborne_ball_meets_the_rail_it_is_flying_at() {
    // The ball is in the air, heading for the rail at a speed that would carry
    // it off the table if the rail were not there.
    let cfg = TableConfig::default();
    let start = Vec3::new(0.4, 0.635, cfg.ball.radius);
    for lift in [0.2, 0.5, 1.0, 2.0] {
        let ball = ball(&cfg, start, Vec3::new(-6.5, 0.0, lift));
        let (state, trace) = run(&cfg, ball);
        let cushions = trace
            .events
            .iter()
            .filter(|event| event.kind == EventKind::Cushion)
            .count();
        let off = state.balls[0].is_off_table;
        if off {
            // The ball went over the rail: that is a ball off the table, and
            // the core has to say so.
            assert!(
                cushions == 0 || state.balls[0].position.x < 0.0,
                "the ball was let go in the middle of the table"
            );
        } else {
            assert!(
                cushions > 0,
                "the ball flew at the rail with a lift of {lift} and nothing happened"
            );
            assert!(
                state.balls[0].position.x > 0.0,
                "the ball ended past the rail at {:?}",
                state.balls[0].position
            );
        }
    }
}

#[test]
fn a_ball_that_hops_over_a_rail_is_off_the_table() {
    // The ball is in the air, high enough to clear the rail, and it crosses the
    // nose line. The core has to notice: a ball past the rail is off the table
    // (§10.1), not a ball that is still rolling on the cloth.
    let cfg = TableConfig::default();
    let start = Vec3::new(0.4, 0.635, cfg.ball.radius);
    let ball = ball(&cfg, start, Vec3::new(-6.5, 0.0, 2.0));
    let (state, trace) = run(&cfg, ball);
    let off = trace
        .events
        .iter()
        .filter(|event| event.kind == EventKind::OffTable)
        .count();
    assert!(
        state.balls[0].is_off_table || off > 0,
        "the ball cleared the rail and nobody noticed"
    );
}

#[test]
fn a_ball_that_squeezes_past_a_rail_is_marked_off_the_table() {
    // A ball that clears the rail between the two collision regimes can pass the
    // nose line without raising either a cushion or an off-table crossing event.
    // It used to roll on the cloth outside the table until the time limit, with
    // the shot left in `Settling` for good. Position has to catch it (§10.1).
    let cfg = TableConfig::default();
    let ball = ball(&cfg, Vec3::new(0.3, 0.635, cfg.ball.radius), Vec3::ZERO);
    let mut input = ShotInput::new(
        4.0,
        Vec3::new(1.0, 0.0, 50.0_f64.to_radians().tan()),
        vec![ball],
    );
    input.contact_offset_y = 0.8 * cfg.ball.radius;
    let (state, trace) = try_simulate_shot_traced(&input, &cfg).expect("the shot runs");

    assert!(state.balls[0].is_off_table, "the ball left the table");
    assert_eq!(state.balls[0].pocket_state, PocketState::OffTable);
    assert_eq!(state.balls[0].velocity, Vec3::ZERO);
    assert_eq!(state.balls[0].angular_velocity, Vec3::ZERO);
    assert_eq!(state.phase, GamePhase::Settled);
    assert!(
        trace.duration < 5.0,
        "the shot should end when the ball leaves, not at {} s",
        trace.duration
    );
    assert!(
        trace.events.iter().any(|e| e.kind == EventKind::OffTable),
        "the departure has to be in the event log"
    );
    // The ball is taken out of play, not removed from the state vector: its slot
    // stays where the renderer and the hash expect it.
    assert_eq!(state.balls.len(), 1);
}

#[test]
fn a_ball_already_past_a_rail_is_marked_off_the_table() {
    // The backstop runs before the settled check: a ball that has stopped outside
    // the table has still left it and must be marked even though it is at rest.
    let cfg = TableConfig::default();
    let ball = BallState::at_rest(
        &cfg,
        0,
        Vec3::new(
            cfg.table.length + 0.1,
            cfg.table.width / 2.0,
            cfg.ball.radius,
        ),
    );
    let input = ShotInput::new(0.0, Vec3::X, vec![ball]);
    let state = try_simulate_state(&input, &cfg).expect("the state runs");
    assert!(state.balls[0].is_off_table);
    assert_eq!(state.balls[0].pocket_state, PocketState::OffTable);
    assert_eq!(state.phase, GamePhase::Settled);
}

#[test]
fn no_ball_ever_ends_up_past_a_rail_it_did_not_bounce_off() {
    // The whole table, every direction, a spread of speeds: no ball may cross
    // a nose line without a cushion event or an off-table event first.
    let cfg = TableConfig::default();
    let half = half_length(&cfg);
    for speed in [0.5, 1.0, 2.0, 4.0, 6.0, 8.0, 10.0] {
        for lift in [-1.0, -0.3, 0.0, 0.3, 1.0, 2.0] {
            for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                let start = Vec3::new(half - 0.6, cfg.table.width / 2.0, cfg.ball.radius);
                let velocity = Vec3::new(dx * speed, dy * speed, lift);
                let ball = ball(&cfg, start, velocity);
                let (state, _) = run(&cfg, ball);
                let ball_state = &state.balls[0];
                if ball_state.is_off_table || ball_state.is_pocketed {
                    continue;
                }
                let inside = ball_state.position.x > -1e-9
                    && ball_state.position.x < cfg.table.length + 1e-9
                    && ball_state.position.y > -1e-9
                    && ball_state.position.y < cfg.table.width + 1e-9;
                assert!(
                    inside,
                    "a ball at {velocity:?} ended outside the table at {:?}",
                    ball_state.position
                );
            }
        }
    }
}
