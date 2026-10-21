//! The trace: the path a renderer interpolates (§11.2).
//!
//! The trace is observational, so the first thing these tests check is that
//! turning the recorder on changes nothing about the shot. The rest checks that
//! the recorded path really is the path of the balls: continuous at every event,
//! and on the contact geometry the model says it should be.

use pool_core::geometry::TableGeometry;
use pool_core::math::Vec3;
use pool_core::sim::EventKind;
use pool_core::{try_simulate_shot, try_simulate_shot_traced, BallState, ShotInput, TableConfig};

/// A shot with a cue ball, an object ball and a third ball in the way.
fn shot() -> (TableConfig, ShotInput) {
    let cfg = TableConfig::default();
    let balls = vec![
        BallState::at_rest(&cfg, 0, Vec3::new(0.635, 0.635, cfg.ball.radius)),
        BallState::at_rest(&cfg, 1, Vec3::new(0.9, 0.635, cfg.ball.radius)),
        BallState::at_rest(&cfg, 2, Vec3::new(1.45, 0.9, cfg.ball.radius)),
    ];
    let input = ShotInput::new(2.5, Vec3::X, balls);
    (cfg, input)
}

/// The tolerance of a position comparison: the analytic path and the simulation's
/// own accumulation are algebraically identical but round differently.
const POSITION_TOLERANCE: f64 = 1e-12;

#[test]
fn the_recorder_does_not_change_the_shot() {
    let (cfg, input) = shot();
    let plain = try_simulate_shot(&input, &cfg).expect("the shot runs");
    let (traced, trace) = try_simulate_shot_traced(&input, &cfg).expect("the shot runs");
    assert_eq!(plain, traced);
    assert_eq!(
        pool_core::hash_state_hex(&plain),
        pool_core::hash_state_hex(&traced)
    );
    assert!(!trace.events.is_empty());
}

#[test]
fn the_trace_carries_the_events_of_the_shot() {
    let (cfg, input) = shot();
    let (_, trace) = try_simulate_shot_traced(&input, &cfg).expect("the shot runs");
    let mut simulation = pool_core::Simulation::new(&input, &cfg);
    simulation.record();
    simulation.run(&input).expect("the shot runs");
    let events = simulation.events().to_vec();
    assert_eq!(events.len(), trace.events.len());
    for (recorded, expected) in trace.events.iter().zip(events.iter()) {
        assert_eq!(recorded.time, expected.time);
        assert_eq!(recorded.kind, expected.kind);
        assert_eq!(recorded.a, expected.a);
        assert_eq!(recorded.b, expected.b);
    }
}

#[test]
fn the_path_ends_where_the_shot_ends() {
    let (cfg, input) = shot();
    let (state, trace) = try_simulate_shot_traced(&input, &cfg).expect("the shot runs");
    let sampled = trace.sample(trace.duration, &cfg);
    assert_eq!(sampled.len(), state.balls.len());
    for (sampled, expected) in sampled.iter().zip(state.balls.iter()) {
        assert!(
            (sampled.position.x - expected.position.x).abs() < POSITION_TOLERANCE,
            "x: {} vs {}",
            sampled.position.x,
            expected.position.x
        );
        assert!(
            (sampled.position.y - expected.position.y).abs() < POSITION_TOLERANCE,
            "y: {} vs {}",
            sampled.position.y,
            expected.position.y
        );
        assert!(
            (sampled.velocity.x - expected.velocity.x).abs() < 1e-9,
            "vx: {} vs {}",
            sampled.velocity.x,
            expected.velocity.x
        );
    }
}

#[test]
fn the_path_is_continuous_at_every_event() {
    let (cfg, input) = shot();
    let (_, trace) = try_simulate_shot_traced(&input, &cfg).expect("the shot runs");
    for (index, segments) in trace.balls.iter().enumerate() {
        for pair in segments.windows(2) {
            let (first, second) = (&pair[0], &pair[1]);
            assert!(
                first.t1 <= second.t0 + 1e-12,
                "ball {index}: the segments overlap"
            );
            let end = first.position_at(first.t1);
            assert!(
                (end.x - second.p0.x).abs() < POSITION_TOLERANCE
                    && (end.y - second.p0.y).abs() < POSITION_TOLERANCE
                    && (end.z - second.p0.z).abs() < POSITION_TOLERANCE,
                "ball {index}: the path jumps at t = {}",
                first.t1
            );
        }
    }
}

#[test]
fn the_segments_are_ordered_and_bounded() {
    let (cfg, input) = shot();
    let (_, trace) = try_simulate_shot_traced(&input, &cfg).expect("the shot runs");
    for segments in &trace.balls {
        for segment in segments {
            assert!(segment.t1 >= segment.t0);
            assert!(segment.t1 <= trace.duration + 1e-9);
        }
    }
}

#[test]
fn the_balls_meet_where_the_model_says_they_meet() {
    let (cfg, input) = shot();
    let (_, trace) = try_simulate_shot_traced(&input, &cfg).expect("the shot runs");
    let contacts = trace
        .events
        .iter()
        .filter(|event| event.kind == EventKind::BallBall)
        .count();
    assert!(contacts > 0, "the shot has to have a collision");
    for event in trace
        .events
        .iter()
        .filter(|e| e.kind == EventKind::BallBall)
    {
        let state = trace.sample(event.time, &cfg);
        let a = state[event.a as usize].position;
        let b = state[event.b as usize].position;
        let distance = (a - b).norm();
        assert!(
            (distance - 2.0 * cfg.ball.radius).abs() < 1e-9,
            "the balls are {distance} apart at t = {}",
            event.time
        );
    }
}

#[test]
fn the_balls_meet_the_rails_where_the_model_says_they_meet() {
    let (cfg, input) = shot();
    let (_, trace) = try_simulate_shot_traced(&input, &cfg).expect("the shot runs");
    let geometry = TableGeometry::new(&cfg);
    let contacts = trace
        .events
        .iter()
        .filter(|event| event.kind == EventKind::Cushion)
        .count();
    assert!(contacts > 0, "the shot has to have a cushion hit");
    for event in trace.events.iter().filter(|e| e.kind == EventKind::Cushion) {
        let state = trace.sample(event.time, &cfg);
        let position = state[event.a as usize].position;
        let rail = &geometry.rails[event.b as usize];
        let along = position.x * rail.normal.x + position.y * rail.normal.y;
        let expected = rail.contact_coord(geometry.cushion.cos_theta, cfg.ball.radius);
        assert!(
            (along - expected).abs() < 1e-9,
            "the ball is {along} from the rail, expected {expected}"
        );
    }
}

#[test]
fn the_cue_ball_is_still_where_the_shot_left_it() {
    let (cfg, input) = shot();
    let (state, trace) = try_simulate_shot_traced(&input, &cfg).expect("the shot runs");
    let sampled = trace.sample(trace.duration, &cfg);
    for (index, ball) in state.balls.iter().enumerate() {
        assert!(
            (sampled[index].position.x - ball.position.x).abs() < POSITION_TOLERANCE,
            "ball {index} moved"
        );
    }
}

#[test]
fn the_sampler_gives_the_state_after_an_event() {
    let (cfg, input) = shot();
    let (_, trace) = try_simulate_shot_traced(&input, &cfg).expect("the shot runs");
    let event = trace
        .events
        .iter()
        .find(|event| event.kind == EventKind::BallBall)
        .expect("the shot has a collision");
    let before = trace.sample(event.time - 1e-9, &cfg);
    let after = trace.sample(event.time, &cfg);
    // The impulse has been applied: the balls leave the contact with new
    // velocities, but they are still touching.
    let a = event.a as usize;
    let b = event.b as usize;
    assert!(
        (before[a].velocity.x - after[a].velocity.x).abs() > 0.01
            || (before[a].velocity.y - after[a].velocity.y).abs() > 0.01,
        "the cue ball left the contact with the velocity it came with"
    );
    let distance = (after[a].position - after[b].position).norm();
    assert!(
        (distance - 2.0 * cfg.ball.radius).abs() < 1e-9,
        "the balls are {distance} apart at the moment of the contact"
    );
}

#[test]
fn two_traces_of_the_same_shot_are_identical() {
    let (cfg, input) = shot();
    let (_, first) = try_simulate_shot_traced(&input, &cfg).expect("the shot runs");
    let (_, second) = try_simulate_shot_traced(&input, &cfg).expect("the shot runs");
    assert_eq!(first, second);
}

#[test]
fn a_rolling_ball_turns_as_far_as_it_rolls() {
    let cfg = TableConfig::default();
    // A ball that is already rolling: the rolling condition of §3.2, so it
    // turns about the lateral axis and turns as many radians as it has rolled
    // radii. It rolls slowly enough to stop on the bed, so no rail is in the
    // way and the turn is only the roll. No cue strike either, so the state is
    // exactly what is set here.
    let speed = 0.3;
    let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(0.5, 0.635, cfg.ball.radius));
    ball.velocity = Vec3::new(speed, 0.0, 0.0);
    ball.angular_velocity = Vec3::new(0.0, speed / cfg.ball.radius, 0.0);
    let input = ShotInput::new(0.0, Vec3::X, vec![ball]);
    let mut simulation = pool_core::Simulation::new(&input, &cfg);
    simulation.record();
    simulation.run_without_strike().expect("the ball rolls");
    let trace = simulation.trace().cloned().expect("the path was recorded");

    let travelled = trace.sample_positions(trace.duration)[0].x - 0.5;
    let rotation = trace.sample_rotation(trace.duration)[0];
    let expected = travelled / cfg.ball.radius;
    assert!(
        (rotation.y - expected).abs() < 1e-9,
        "the ball turned {} radians, which is not the {expected} of the roll",
        rotation.y
    );
    // The roll is about the lateral axis, not about the direction of travel.
    assert!(
        rotation.x.abs() < 1e-9,
        "the ball turned about the direction of travel: {rotation:?}"
    );
}

#[test]
fn the_vertical_spin_dies_at_zero_and_stays_there() {
    // The spin decay of §3.2 is linear: `w_z(t) = w0 - (5 mu_sp g / 2R) t`. A
    // linear decay has to be told where zero is, or it goes on past it: a
    // backspin would then grow instead of dying, and a ball whose spin never
    // died would never come to rest. The decay therefore acts towards zero
    // and stops there.
    //
    // This test pins that: the spin of a ball that is rolling dies and stays
    // dead, and a backspin dies rather than growing.
    let cfg = TableConfig::default();
    let speed = 0.3;
    let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(0.5, 0.635, cfg.ball.radius));
    ball.velocity = Vec3::new(speed, 0.0, 0.0);
    ball.angular_velocity = Vec3::new(0.0, speed / cfg.ball.radius, 0.0);
    let input = ShotInput::new(0.0, Vec3::X, vec![ball]);
    let mut simulation = pool_core::Simulation::new(&input, &cfg);
    simulation.record();
    simulation.run_without_strike().expect("the ball rolls");
    let trace = simulation.trace().cloned().expect("the path was recorded");

    // The ball has no vertical spin to begin with, so it gets none.
    assert_eq!(simulation.balls()[0].angular_velocity.z, 0.0);
    let rotation = trace.sample_rotation(trace.duration)[0];
    assert!(
        rotation.z.abs() < 1e-9,
        "the ball turned {} about the vertical axis, which is not nothing",
        rotation.z
    );

    // A backspin dies too: it does not grow.
    let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(0.5, 0.635, cfg.ball.radius));
    ball.velocity = Vec3::new(speed, 0.0, 0.0);
    ball.angular_velocity = Vec3::new(0.0, speed / cfg.ball.radius, -20.0);
    let input = ShotInput::new(0.0, Vec3::X, vec![ball]);
    let mut simulation = pool_core::Simulation::new(&input, &cfg);
    simulation.record();
    simulation.run_without_strike().expect("the ball rolls");
    let trace = simulation.trace().cloned().expect("the path was recorded");
    let spin = simulation.balls()[0].angular_velocity.z;
    assert!(
        (-20.0..=0.0).contains(&spin),
        "the backspin ended at {spin}, which is not a backspin dying"
    );
    let rotation = trace.sample_rotation(trace.duration)[0];
    assert!(
        rotation.z < 0.0,
        "the ball turned {} about the vertical axis, which is not the backspin",
        rotation.z
    );
}

#[test]
fn the_rotation_is_continuous_at_every_event() {
    let (cfg, input) = shot();
    let (_, trace) = try_simulate_shot_traced(&input, &cfg).expect("the shot runs");
    for (index, segments) in trace.balls.iter().enumerate() {
        for pair in segments.windows(2) {
            let (first, second) = (&pair[0], &pair[1]);
            let end = first.rotation_at(first.t1);
            let start = second.rotation_at(second.t0);
            assert!(
                (end - start).norm() < 1e-9,
                "ball {index}: the rotation jumps at t = {}: {end:?} vs {start:?}",
                first.t1
            );
        }
    }
}

#[test]
fn the_samplers_agree_with_the_state() {
    let (cfg, input) = shot();
    let (state, trace) = try_simulate_shot_traced(&input, &cfg).expect("the shot runs");
    let positions = trace.sample_positions(trace.duration);
    let angular = trace.sample_angular(trace.duration);
    assert_eq!(positions.len(), state.balls.len());
    for (index, ball) in state.balls.iter().enumerate() {
        assert!(
            (positions[index].x - ball.position.x).abs() < POSITION_TOLERANCE,
            "ball {index} is not where the state says"
        );
        assert!(
            (angular[index].x - ball.angular_velocity.x).abs() < 1e-9,
            "ball {index} does not spin the way the state says"
        );
    }
}
