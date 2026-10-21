//! The edge cases of §11.4 and of the validation plan: cluster collisions,
//! simultaneous events, grazing hits at break speed and the guards against `NaN` and
//! `Inf` (§10.4).

use pool_core::math::Vec3;
use pool_core::state::{BallState, MotionState, ShotInput};
use pool_core::{simulate_shot, try_simulate_state, TableConfig};

fn ball(cfg: &TableConfig, number: u8, x: f64, y: f64) -> BallState {
    BallState::at_rest(cfg, number, Vec3::new(x, y, cfg.ball.radius))
}

/// Checks that a state carries no `NaN` and no infinity.
fn assert_finite(state: &pool_core::GameState, what: &str) {
    for b in &state.balls {
        assert!(b.position.is_finite(), "{what}: the position is not finite");
        assert!(b.velocity.is_finite(), "{what}: the velocity is not finite");
        assert!(
            b.angular_velocity.is_finite(),
            "{what}: the angular velocity is not finite"
        );
    }
    assert!(state.elapsed_time.is_finite());
}

#[test]
fn a_cluster_collision_is_resolved() {
    // Three balls in a line: the third one hits the second, which is resting against
    // the first. The pair must not be resolved twice at the same instant.
    let cfg = TableConfig::default();
    let mut balls = vec![
        ball(&cfg, 0, 0.5, 0.635),
        ball(&cfg, 1, 0.5 + 2.0 * cfg.ball.radius, 0.635),
        ball(&cfg, 2, 0.5 + 4.0 * cfg.ball.radius, 0.635),
    ];
    balls[2].velocity = Vec3::new(-1.0, 0.0, 0.0);
    let input = ShotInput::new(0.0, Vec3::X, balls);
    let state = try_simulate_state(&input, &cfg).expect("the shot runs");
    assert_finite(&state, "the cluster collision");
    // The momentum travels through the cluster: the ball at the far end is the one
    // that moves.
    assert!(
        state.balls[0].position.x < 0.5,
        "the last ball of the cluster did not move: {:?}",
        state.balls[0].position
    );
}

#[test]
fn a_cluster_of_touching_balls_settles() {
    // A rack of balls that all touch: every pair is in contact from the start.
    let cfg = TableConfig::default();
    let mut balls = Vec::new();
    for index in 0..9u8 {
        let row = index / 3;
        let column = index % 3;
        balls.push(ball(
            &cfg,
            index,
            1.0 + column as f64 * 2.0 * cfg.ball.radius,
            0.5 + row as f64 * 2.0 * cfg.ball.radius,
        ));
    }
    let input = ShotInput::new(0.0, Vec3::X, balls);
    let state = try_simulate_state(&input, &cfg).expect("the shot runs");
    assert_finite(&state, "the cluster");
    // Nothing moves: the balls are in contact and nothing pushes them.
    for b in &state.balls {
        assert!(b.velocity.norm() < 1e-9);
    }
}

#[test]
fn simultaneous_events_are_ordered_deterministically() {
    // Two balls hit by a third one at the same instant: the tie is broken by the
    // ball index, so the outcome is the same on every run.
    let cfg = TableConfig::default();
    let mut balls = vec![
        ball(&cfg, 0, 0.5, 0.635),
        ball(&cfg, 1, 0.5, 0.635 + 2.0 * cfg.ball.radius),
        ball(
            &cfg,
            2,
            0.5 + 2.0 * cfg.ball.radius,
            0.635 + 2.0 * cfg.ball.radius,
        ),
    ];
    balls[0].velocity = Vec3::new(1.0, 1.0, 0.0);
    let input = ShotInput::new(0.0, Vec3::X, balls);
    let first = try_simulate_state(&input, &cfg).expect("the shot runs");
    let second = try_simulate_state(&input, &cfg).expect("the shot runs");
    assert_eq!(first, second);
    assert_finite(&first, "the simultaneous events");
}

#[test]
fn a_grazing_hit_at_break_speed_does_not_break_the_simulation() {
    // A ball that misses another one by a hair at break speed.
    let cfg = TableConfig::default();
    for offset in [0.0, 1e-6, 1e-4, 1e-3, 0.005] {
        let mut balls = vec![
            ball(&cfg, 0, 0.5, 0.635),
            ball(&cfg, 1, 0.9, 0.635 + 2.0 * cfg.ball.radius + offset),
        ];
        balls[0].velocity = Vec3::new(13.0, 0.0, 0.0);
        let input = ShotInput::new(0.0, Vec3::X, balls);
        let state = try_simulate_state(&input, &cfg).expect("the shot runs");
        assert_finite(&state, "the grazing hit");
    }
}

#[test]
fn an_overlapping_pair_is_resolved_without_a_loop() {
    // Two balls that start inside each other: the simulation has to resolve the
    // contact and carry on.
    let cfg = TableConfig::default();
    let mut balls = vec![
        ball(&cfg, 0, 0.5, 0.635),
        ball(&cfg, 1, 0.5 + 0.5 * cfg.ball.radius, 0.635),
    ];
    balls[0].velocity = Vec3::new(2.0, 0.0, 0.0);
    let input = ShotInput::new(0.0, Vec3::X, balls);
    let state = try_simulate_state(&input, &cfg).expect("the shot runs");
    assert_finite(&state, "the overlapping pair");
}

#[test]
fn a_zero_speed_shot_does_nothing() {
    let cfg = TableConfig::default();
    let input = ShotInput::new(0.0, Vec3::X, vec![ball(&cfg, 0, 0.5, 0.635)]);
    let state = simulate_shot(input, &cfg);
    assert_finite(&state, "the zero speed shot");
    assert!(state.balls[0].velocity.norm() < 1e-12);
    assert_eq!(state.balls[0].motion_state, MotionState::Stationary);
}

#[test]
fn a_degenerate_cue_direction_is_handled() {
    let cfg = TableConfig::default();
    let input = ShotInput::new(2.0, Vec3::ZERO, vec![ball(&cfg, 0, 0.5, 0.635)]);
    let state = simulate_shot(input, &cfg);
    assert_finite(&state, "the degenerate direction");
    assert!(state.balls[0].velocity.norm() < 1e-12);
}

#[test]
fn a_ball_outside_the_table_is_handled() {
    let cfg = TableConfig::default();
    let input = ShotInput::new(0.0, Vec3::X, vec![ball(&cfg, 0, 5.0, 5.0)]);
    let state = try_simulate_state(&input, &cfg).expect("the shot runs");
    assert_finite(&state, "the ball outside the table");
}

#[test]
fn the_state_never_contains_a_nan() {
    // A spread of shots: every one of them has to end in a finite state.
    let mut cfg = TableConfig::default();
    // The shots are only about the guards, so they do not need the full half minute.
    cfg.simulation.max_sim_time = 6.0;
    for speed in [0.0, 0.1, 1.0, 6.0, 12.0] {
        for cut in [0.0_f64, 15.0, 45.0, 89.0] {
            for english in [0.0, 0.4] {
                let mut balls = vec![ball(&cfg, 0, 0.6, 0.635)];
                let angle = cut.to_radians();
                balls.push(ball(
                    &cfg,
                    1,
                    0.6 + 0.4 * angle.cos(),
                    0.635 + 0.4 * angle.sin(),
                ));
                let mut input =
                    ShotInput::new(speed, Vec3::new(angle.cos(), angle.sin(), 0.0), balls);
                input.contact_offset_x = english * cfg.ball.radius;
                let state = simulate_shot(input, &cfg);
                assert_finite(&state, "the shot sweep");
            }
        }
    }
}

#[test]
fn the_event_order_is_a_total_order() {
    // Two events at the same instant are ordered by kind and then by ball index.
    use pool_core::EventKind;
    let a = pool_core::SimEvent {
        time: 1.0,
        kind: EventKind::BallBall,
        a: 2,
        b: 5,
        detail: 0,
    };
    let b = pool_core::SimEvent {
        time: 1.0,
        kind: EventKind::BallBall,
        a: 2,
        b: 4,
        detail: 0,
    };
    let c = pool_core::SimEvent {
        time: 1.0,
        kind: EventKind::Cushion,
        a: 0,
        b: 0,
        detail: 0,
    };
    let d = pool_core::SimEvent {
        time: 1.0,
        kind: EventKind::BallBall,
        a: 2,
        b: 5,
        detail: 0,
    };
    // The same kind is ordered by the ball index, and a ball-ball collision comes
    // before a cushion hit.
    assert!(b.order_key() < a.order_key());
    assert!(a.order_key() < c.order_key());
    assert!(a.order_key() == d.order_key());
}
