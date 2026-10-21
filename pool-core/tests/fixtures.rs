//! The fixture-driven acceptance tests.
//!
//! Every fixture in `tests/fixtures/` is an acceptance criterion for the physics
//! core. The fixtures are the source of truth: if a test fails, the implementation
//! is wrong, not the fixture.

mod common;

use common::{
    array, assert_close, expectations, field, fixture, number, optional_number, setup_balls, text,
    tolerance,
};
use pool_core::ball_ball;
use pool_core::cloth::{self, ClothParams};
use pool_core::config::TableConfig;
use pool_core::cushion::{self, RailRegime};
use pool_core::geometry::TableGeometry;
use pool_core::math::{Vec2, Vec3};
use pool_core::state::{BallState, MotionState, ShotInput};
use pool_core::{cue, try_simulate_state};

fn cloth_params(cfg: &TableConfig) -> ClothParams {
    ClothParams {
        mu_s: cfg.cloth.mu_s,
        mu_r: cfg.cloth.mu_r,
        mu_sp: cfg.cloth.mu_sp,
        g: cfg.simulation.gravity,
        radius: cfg.ball.radius,
    }
}

/// Builds a ball from a fixture entry.
fn ball_from_fixture(cfg: &TableConfig, entry: &Value2) -> BallState {
    let position = entry.position;
    let mut ball = BallState::at_rest(cfg, 0, position);
    ball.velocity = entry.velocity;
    ball.angular_velocity = entry.angular_velocity;
    ball
}

/// A fixture ball: position, velocity and angular velocity.
struct Value2 {
    position: Vec3,
    velocity: Vec3,
    angular_velocity: Vec3,
}

fn read_ball(entry: &pool_core::json::Value, radius: f64) -> Value2 {
    let position = entry.field("position").expect("a position");
    let velocity = entry.field("velocity").expect("a velocity");
    let angular = entry
        .field("angular_velocity")
        .expect("an angular velocity");
    let pair = |value: &pool_core::json::Value| -> Vec2 {
        let items = value.as_array().expect("a pair");
        Vec2::new(
            items[0].as_f64().expect("a number"),
            items[1].as_f64().expect("a number"),
        )
    };
    let position = pair(position);
    let velocity = pair(velocity);
    let angular = pair(angular);
    Value2 {
        position: Vec3::new(position.x, position.y, radius),
        velocity: Vec3::new(velocity.x, velocity.y, 0.0),
        angular_velocity: Vec3::new(angular.x, angular.y, 0.0),
    }
}

// ---------------------------------------------------------------- slide to roll

#[test]
fn slide_to_roll() {
    let fixture = fixture("slide_to_roll.json");
    let cfg = TableConfig::default();
    let params = cloth_params(&cfg);
    let balls = setup_balls(&fixture);
    let entry = read_ball(&balls[0], cfg.ball.radius);
    let mut ball = ball_from_fixture(&cfg, &entry);

    let u0 = cloth::slip_velocity(ball.velocity, ball.angular_velocity, cfg.ball.radius).norm();
    let time = cloth::time_to_rolling(u0, &params);

    // Advance the ball to the transition.
    cloth::advance_sliding(&mut ball, time, &params);
    let residual = ball.angular_velocity.y - ball.velocity.x / cfg.ball.radius;

    let mut checked = 0;
    for expectation in expectations(&fixture) {
        let quantity = text(&expectation, "quantity");
        let measured = match quantity.as_str() {
            "time_to_rolling" => time,
            "velocity_at_transition" => ball.velocity.x,
            "omega_y_at_transition" => ball.angular_velocity.y,
            "x_at_transition" => ball.position.x,
            "rolling_condition_residual" => residual,
            other => panic!("unknown expectation '{other}'"),
        };
        assert_close(
            measured,
            number(&expectation, "expected"),
            tolerance(&expectation),
            &quantity,
        );
        checked += 1;
    }
    assert!(checked >= 5);
}

// ---------------------------------------------------------------- 90 degree rule

#[test]
fn stun_separation_90() {
    let fixture = fixture("stun_separation_90.json");
    let cfg = TableConfig::default();
    let balls = setup_balls(&fixture);
    let cue_ball = read_ball(&balls[0], cfg.ball.radius);
    let object = read_ball(&balls[1], cfg.ball.radius);

    let mut a = ball_from_fixture(&cfg, &cue_ball);
    a.number = 0;
    let mut b = ball_from_fixture(&cfg, &object);
    b.number = 1;

    // The head-on case: the object ball is placed straight ahead.
    let mut head_on = b;
    head_on.position = Vec3::new(0.3, 0.0, cfg.ball.radius);
    let head_on_outcome = ball_ball::resolve(&a, &head_on, &cfg);

    let outcome = ball_ball::resolve(&a, &b, &cfg);
    let separation = (outcome.velocity_b.y.atan2(outcome.velocity_b.x)
        - outcome.velocity_a.y.atan2(outcome.velocity_a.x))
    .to_degrees();
    let cut = b.position.y.atan2(b.position.x);
    let throw = (outcome.velocity_b.y.atan2(outcome.velocity_b.x) - cut).to_degrees();
    let momentum = (outcome.velocity_a + outcome.velocity_b - (a.velocity + b.velocity)).norm();

    for expectation in expectations(&fixture) {
        let quantity = text(&expectation, "quantity");
        let expected = number(&expectation, "expected");
        let tol = tolerance(&expectation);
        match quantity.as_str() {
            "cue_speed_after_head_on" => {
                assert_close(head_on_outcome.velocity_a.norm(), expected, tol, &quantity)
            }
            "separation_angle" => assert_close(separation, expected, tol, &quantity),
            "cue_speed_after" => assert_close(outcome.velocity_a.norm(), expected, tol, &quantity),
            "object_speed_after" => {
                assert_close(outcome.velocity_b.norm(), expected, tol, &quantity)
            }
            "object_throw_angle" => assert_close(throw, expected, tol, &quantity),
            "momentum_residual" => assert_close(momentum, expected, tol, &quantity),
            other => panic!("unknown expectation '{other}'"),
        }
    }
}

// ---------------------------------------------------------------- the throw sweep

#[test]
fn cut_throw_sweep() {
    let fixture = fixture("cut_throw_sweep.json");
    let cfg = TableConfig::default();
    let balls = setup_balls(&fixture);
    let cue_ball = read_ball(&balls[0], cfg.ball.radius);
    let object_ball = read_ball(&balls[1], cfg.ball.radius);
    let mut a = ball_from_fixture(&cfg, &cue_ball);
    a.number = 0;

    for row in expectations(&fixture) {
        let cut_deg = number(&row, "cut_angle");
        let cut = cut_deg.to_radians();
        let mut b = ball_from_fixture(&cfg, &object_ball);
        b.number = 1;
        b.position = Vec3::new(0.3 * cut.cos(), 0.3 * cut.sin(), cfg.ball.radius);

        let outcome = ball_ball::resolve(&a, &b, &cfg);
        let throw = (outcome.velocity_b.y.atan2(outcome.velocity_b.x) - cut).to_degrees();

        // The fixture records the model value rounded to four decimals.
        assert_close(
            throw,
            number(&row, "model_throw_deg"),
            1e-4,
            &format!("throw at {cut_deg} degrees"),
        );
        let regime = text(&row, "regime");
        assert_eq!(
            regime,
            if outcome.sticking {
                "sticking"
            } else {
                "sliding"
            },
            "the regime at {cut_deg} degrees"
        );
    }

    // The sticking regime matches the published formula
    // `atan(2 tan(cut) / (7 (1 + e)))` to within a hundredth of a degree.
    let cut = 10.0_f64.to_radians();
    let expected = (2.0 * cut.tan() / (7.0 * (1.0 + cfg.ball_ball.e_ball)))
        .atan()
        .to_degrees();
    let mut b = ball_from_fixture(&cfg, &object_ball);
    b.position = Vec3::new(0.3 * cut.cos(), 0.3 * cut.sin(), cfg.ball.radius);
    let outcome = ball_ball::resolve(&a, &b, &cfg);
    let throw = (outcome.velocity_b.y.atan2(outcome.velocity_b.x) - cut).to_degrees();
    // The published formula gives the magnitude of the throw; the model throws the
    // object ball towards the cue ball's post-collision direction, which is the
    // negative side for this geometry.
    assert_close(
        throw.abs(),
        expected,
        1e-9,
        "the sticking throw at 10 degrees",
    );
    // The measured throw at a 10 degree cut is 1.45 degrees (Alciatore TP A-14).
    assert_close(throw.abs(), 1.45, 0.03, "the measured 10 degree throw");
}

// ---------------------------------------------------------------- the cushion

#[test]
fn cushion_rebound() {
    let fixture = fixture("cushion_rebound.json");
    let cfg = TableConfig::default();
    let geom = TableGeometry::new(&cfg);

    for row in expectations(&fixture) {
        let incidence = number(&row, "incidence_angle");
        let theta = incidence.to_radians();
        let speed = 2.0;
        let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(0.5, 0.5, cfg.ball.radius));
        ball.velocity = Vec3::new(speed * theta.cos(), speed * theta.sin(), 0.0);
        ball.angular_velocity = Vec3::new(
            -ball.velocity.y / cfg.ball.radius,
            ball.velocity.x / cfg.ball.radius,
            0.0,
        );

        let outcome = cushion::resolve_nose_cushion(&ball, Vec3::X, Vec3::Y, &cfg, &geom.cushion);
        let rebound_speed = outcome.velocity.x.hypot(outcome.velocity.y);
        let rebound_angle = outcome.velocity.y.atan2(-outcome.velocity.x).to_degrees();

        assert_close(
            rebound_speed,
            number(&row, "rebound_speed"),
            1e-6,
            &format!("the rebound speed at {incidence} degrees"),
        );
        assert_close(
            rebound_angle,
            number(&row, "rebound_angle"),
            1e-4,
            &format!("the rebound angle at {incidence} degrees"),
        );
        assert_close(
            outcome.velocity.z,
            number(&row, "rebound_vz"),
            1e-6,
            &format!("the rebound vz at {incidence} degrees"),
        );
    }

    // The vertical flip between the three reference nose heights (Dr. Dave).
    let flip = array(&fixture, "vertical_flip");
    assert!(!flip.is_empty());
    for row in &flip {
        let fraction = number(row, "nose_height_over_D");
        let eps_over_r = fraction * 2.0 - 1.0;
        let sin_theta = eps_over_r;
        let cos_theta = (1.0 - sin_theta * sin_theta).sqrt();
        let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(0.5, 0.5, cfg.ball.radius));
        ball.velocity = Vec3::new(2.0, 0.0, 0.0);
        ball.angular_velocity = Vec3::new(0.0, 2.0 / cfg.ball.radius, 0.0);
        let outcome = cushion::resolve_cushion(
            &ball,
            Vec3::X,
            Vec3::Y,
            sin_theta,
            cos_theta,
            cfg.cushion.e_cushion,
            cfg.cushion.mu_cushion,
            &cfg,
        );
        assert_close(
            outcome.velocity.z,
            number(row, "rebound_vz"),
            1e-6,
            &format!("the vertical impulse at {fraction} D"),
        );
        // The hop: a ball that is pressed into the slate rebounds with e_table.
        let up = if outcome.velocity.z > 0.0 {
            outcome.velocity.z
        } else {
            cfg.slate.e_table * outcome.velocity.z.abs()
        };
        let hop_mm = up * up / (2.0 * cfg.simulation.gravity) * 1000.0;
        assert_close(
            hop_mm,
            number(row, "hop_height_mm"),
            1e-3,
            &format!("the hop at {fraction} D"),
        );
        let direction = text(row, "direction");
        if number(row, "rebound_vz") < 0.0 {
            assert_eq!(direction, "down");
        } else {
            assert_eq!(direction, "up");
        }
    }

    // The three reference heights of Dr. Dave, spelled out.
    let reference = [0.500_f64, 0.635, 0.700];
    let expected = [0.49_f64, -0.455624, -0.871521];
    for (fraction, expected_vz) in reference.iter().zip(expected.iter()) {
        let sin_theta = fraction * 2.0 - 1.0;
        let cos_theta = (1.0 - sin_theta * sin_theta).sqrt();
        let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(0.5, 0.5, cfg.ball.radius));
        ball.velocity = Vec3::new(2.0, 0.0, 0.0);
        ball.angular_velocity = Vec3::new(0.0, 2.0 / cfg.ball.radius, 0.0);
        let outcome = cushion::resolve_cushion(
            &ball,
            Vec3::X,
            Vec3::Y,
            sin_theta,
            cos_theta,
            cfg.cushion.e_cushion,
            cfg.cushion.mu_cushion,
            &cfg,
        );
        assert_close(
            outcome.velocity.z,
            *expected_vz,
            1e-5,
            &format!("the vertical impulse at {fraction} D"),
        );
    }

    // The rail regimes: the solver must branch on the ball's height.
    let regimes = array(&fixture, "rail_regimes");
    for row in &regimes {
        let centre = number(row, "ball_centre_height_mm") / 1000.0;
        let regime = cushion::regime_for_height(centre, cfg.ball.radius, &cfg);
        let expected = match text(row, "regime").as_str() {
            "nose rubber" => RailRegime::NoseRubber,
            "rail structure" => RailRegime::RailStructure,
            _ => RailRegime::ClearOfRail,
        };
        assert_eq!(regime, expected, "the regime at {centre} m");
    }
}

#[test]
fn cushion_rebound_angle_does_not_depend_on_the_speed() {
    // Mathavan Fig. 8: the rebound angle is the same at 1, 2 and 4 m/s.
    let cfg = TableConfig::default();
    let geom = TableGeometry::new(&cfg);
    let mut angles = Vec::new();
    for speed in [1.0, 2.0, 4.0] {
        let theta = 30.0_f64.to_radians();
        let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(0.5, 0.5, cfg.ball.radius));
        ball.velocity = Vec3::new(speed * theta.cos(), speed * theta.sin(), 0.0);
        ball.angular_velocity = Vec3::new(
            -ball.velocity.y / cfg.ball.radius,
            ball.velocity.x / cfg.ball.radius,
            0.0,
        );
        let outcome = cushion::resolve_nose_cushion(&ball, Vec3::X, Vec3::Y, &cfg, &geom.cushion);
        angles.push(outcome.velocity.y.atan2(-outcome.velocity.x).to_degrees());
    }
    assert!((angles[0] - angles[1]).abs() < 1e-9);
    assert!((angles[1] - angles[2]).abs() < 1e-9);
}

// ---------------------------------------------------------------- draw and stop

#[test]
fn draw_stop_shot() {
    let fixture = fixture("draw_stop_shot.json");
    let cfg = TableConfig::default();
    let params = cloth_params(&cfg);

    for row in expectations(&fixture) {
        let omega_y = number(&row, "omega_y_at_impact");
        let velocity = number(&row, "velocity_at_impact");
        let mut ball = BallState::at_rest(&cfg, 0, Vec3::ZERO);
        ball.velocity = Vec3::new(velocity, 0.0, 0.0);
        ball.angular_velocity = Vec3::new(0.0, omega_y, 0.0);
        ball.motion_state = MotionState::Sliding;

        let slip = cloth::slip_velocity(ball.velocity, ball.angular_velocity, cfg.ball.radius).x;
        assert_close(
            slip,
            number(&row, "slip_at_impact"),
            1e-12,
            "the slip at the impact",
        );

        let time = cloth::time_to_rolling(slip.abs(), &params);
        assert_close(
            time,
            number(&row, "time_sliding_ends"),
            1e-12,
            "the end of the sliding phase",
        );

        cloth::advance_sliding(&mut ball, time, &params);
        assert_close(
            ball.velocity.x,
            number(&row, "velocity_when_sliding_ends"),
            1e-12,
            "the velocity when the sliding phase ends",
        );
        let direction = if ball.velocity.x < 0.0 {
            "backward"
        } else {
            "forward"
        };
        assert_eq!(direction, text(&row, "direction"));
    }

    // The analytic reversal threshold: the cue ball reverses iff
    // `R |omega_y| > 2.5 v`.
    for invariant in array(&fixture, "invariants") {
        if text(&invariant, "quantity") == "reversal_threshold_R_omega_y" {
            let velocity = 0.025 * 2.0;
            assert_close(
                2.5 * velocity,
                number(&invariant, "expected"),
                tolerance(&invariant),
                "the reversal threshold",
            );
            // The threshold really is the boundary: just below it the ball goes
            // forward, just above it the ball comes back.
            let limit = 2.5 * velocity;
            for (omega, expected_back) in [
                (limit / cfg.ball.radius * 0.99, false),
                (limit / cfg.ball.radius * 1.01, true),
            ] {
                let mut ball = BallState::at_rest(&cfg, 0, Vec3::ZERO);
                ball.velocity = Vec3::new(velocity, 0.0, 0.0);
                ball.angular_velocity = Vec3::new(0.0, -omega, 0.0);
                let slip =
                    cloth::slip_velocity(ball.velocity, ball.angular_velocity, cfg.ball.radius).x;
                let time = cloth::time_to_rolling(slip.abs(), &params);
                cloth::advance_sliding(&mut ball, time, &params);
                assert_eq!(ball.velocity.x < 0.0, expected_back);
            }
        }
    }
}

// ---------------------------------------------------------------- the pocket

#[test]
fn pocket_drop() {
    let fixture = fixture("pocket_drop.json");
    let cfg = TableConfig::default();

    for expectation in expectations(&fixture) {
        let quantity = text(&expectation, "quantity");
        match quantity.as_str() {
            "minimum_speed_to_reach_drop" => {
                let expected = number(&expectation, "expected");
                let measured = cfg.minimum_speed_to_drop(cfg.table.shelf_depth_corner);
                assert_close(measured, expected, tolerance(&expectation), &quantity);
            }
            "outcome" => {
                assert_eq!(text(&expectation, "expected"), "POCKETED");
                // A ball rolling straight into the centre line of a corner pocket at
                // 1.5 m/s has to drop.
                let geom = TableGeometry::new(&cfg);
                let pocket = &geom.pockets[0];
                let direction = pocket.centreline;
                let start = pocket.mouth_centre - direction * 0.3;
                let mut balls = Vec::new();
                let mut ball =
                    BallState::at_rest(&cfg, 0, Vec3::new(start.x, start.y, cfg.ball.radius));
                ball.velocity = Vec3::new(direction.x * 1.5, direction.y * 1.5, 0.0);
                ball.angular_velocity = Vec3::new(
                    -ball.velocity.y / cfg.ball.radius,
                    ball.velocity.x / cfg.ball.radius,
                    0.0,
                );
                balls.push(ball);
                let input = ShotInput::new(0.0, Vec3::X, balls);
                let state = try_simulate_state(&input, &cfg).expect("the shot runs");
                assert!(state.balls[0].is_pocketed, "the ball did not drop");
            }
            other => panic!("unknown expectation '{other}'"),
        }
    }

    // A ball that is too slow to cross the shelf hangs up instead of dropping.
    let geom = TableGeometry::new(&cfg);
    let pocket = &geom.pockets[0];
    let direction = pocket.centreline;
    let start = pocket.mouth_centre - direction * 0.3;
    let slow = cfg.minimum_speed_to_drop(cfg.table.shelf_depth_corner) * 0.8;
    let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(start.x, start.y, cfg.ball.radius));
    ball.velocity = Vec3::new(direction.x * slow, direction.y * slow, 0.0);
    let input = ShotInput::new(0.0, Vec3::X, vec![ball]);
    let state = try_simulate_state(&input, &cfg).expect("the shot runs");
    assert!(!state.balls[0].is_pocketed, "a slow ball must not drop");
}

// ---------------------------------------------------------------- the squirt curve

#[test]
fn squirt_curve() {
    let fixture = fixture("squirt_curve.json");
    let cfg = TableConfig::default();

    let mut previous: Option<(f64, f64)> = None;
    for row in expectations(&fixture) {
        let b_over_r = number(&row, "b_over_R");
        let angle = cue::squirt_angle(b_over_r, &cfg);
        match optional_number(&row, "squirt_deg") {
            Some(expected) => {
                assert_close(
                    angle,
                    expected,
                    tolerance(&row),
                    &format!("the squirt at {b_over_r} R"),
                );
            }
            None => {
                // The anchors are still to be calibrated; the shape is what matters.
                assert!(angle >= 0.0);
            }
        }
        if let Some((previous_b, previous_angle)) = previous {
            assert!(
                angle >= previous_angle - 1e-12,
                "the squirt must increase with the impact parameter ({previous_b} -> {b_over_r})"
            );
        }
        previous = Some((b_over_r, angle));
    }

    // A centre-ball hit has no squirt at all.
    assert!(cue::squirt_angle(0.0, &cfg).abs() < 1e-12);

    // The curve is nearly linear up to 0.5 R: the second differences are small.
    let mut angles = Vec::new();
    for step in 0..=10 {
        let b = 0.05 * step as f64;
        angles.push(cue::squirt_angle(b, &cfg));
    }
    let mut curvature = 0.0_f64;
    for index in 1..angles.len() - 1 {
        let second = angles[index + 1] - 2.0 * angles[index] + angles[index - 1];
        curvature = curvature.max(second.abs());
    }
    assert!(
        curvature < 0.2,
        "the squirt curve is not nearly linear: {curvature}"
    );

    // The ball squirts away from the side that was struck.
    let mut balls = Vec::new();
    balls.push(BallState::at_rest(
        &cfg,
        0,
        Vec3::new(0.5, 0.635, cfg.ball.radius),
    ));
    let mut input = ShotInput::new(2.0, Vec3::X, balls.clone());
    input.contact_offset_x = 0.3 * cfg.ball.radius;
    let strike = cue::strike(&input, &cfg);
    assert!(
        strike.velocity.y < 0.0,
        "the ball must squirt away from the side that was struck (vy = {})",
        strike.velocity.y
    );
    // The other side squirts the other way.
    let mut input = ShotInput::new(2.0, Vec3::X, balls);
    input.contact_offset_x = -0.3 * cfg.ball.radius;
    let strike = cue::strike(&input, &cfg);
    assert!(strike.velocity.y > 0.0);
}
