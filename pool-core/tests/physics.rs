//! The physics acceptance tests of the milestones that the fixtures do not cover:
//! the jump shot (§7.1), the cushion hop (§8.1), the table effect of a collision
//! (§4.5), the three rail regimes (§11.3) and the pocket state machine (§9).

use pool_core::cushion::{self, RailRegime};
use pool_core::geometry::TableGeometry;
use pool_core::math::{Vec2, Vec3};
use pool_core::state::{BallState, MotionState, PocketState, ShotInput};
use pool_core::EventKind;
use pool_core::{cue, simulate_shot, try_simulate_state, TableConfig};

fn ball(cfg: &TableConfig, number: u8, x: f64, y: f64) -> BallState {
    BallState::at_rest(cfg, number, Vec3::new(x, y, cfg.ball.radius))
}

/// The highest point a ball reaches above the bed during a simulation, m.
///
/// The events are the only sample points, so the apex of a flight is derived from
/// the ball's vertical velocity when it leaves the bed.
fn peak_height(cfg: &TableConfig, input: &ShotInput, index: usize) -> f64 {
    let mut simulation = pool_core::Simulation::new(input, cfg);
    let _ = simulation.run(input);
    let mut peak = 0.0_f64;
    // The strike itself is not an event, so the launch of the first flight has to be
    // taken from the cue model.
    if index == 0 {
        let strike = cue::strike(input, cfg);
        if strike.velocity.z < 0.0 {
            // An elevated cue drives the ball into the slate, and the slate's
            // rebound is what launches it (§7.1).
            let launch = cfg.slate.e_table * strike.velocity.z.abs();
            peak = peak.max(launch * launch / (2.0 * cfg.simulation.gravity));
        }
    }
    for event in simulation.events() {
        if event.a as usize == index || event.b as usize == index {
            let b = &simulation.balls()[index];
            peak = peak.max(b.position.z - cfg.ball.radius);
            if b.velocity.z > 0.0 {
                let apex =
                    b.position.z + b.velocity.z * b.velocity.z / (2.0 * cfg.simulation.gravity);
                peak = peak.max(apex - cfg.ball.radius);
            }
        }
    }
    peak
}

// ---------------------------------------------------------------- the jump shot

#[test]
fn an_elevated_cue_drives_the_ball_into_the_slate_and_the_slate_launches_it() {
    let cfg = TableConfig::default();

    // The cue is elevated so that the downward velocity is 1 m/s (§7.1).
    let target = 1.0_f64;
    let elevation = 20.0_f64.to_radians();
    let speed = target
        / (cfg.cue.cue_mass * (1.0 + cfg.cue.e_cue) / (cfg.ball.mass + cfg.cue.cue_mass)
            * elevation.sin());
    let direction = Vec3::new(0.0, elevation.cos(), -elevation.sin());

    let input = ShotInput::new(speed, direction, vec![ball(&cfg, 0, 1.0, 0.635)]);
    let height = peak_height(&cfg, &input, 0);
    let expected = (cfg.slate.e_table * target).powi(2) / (2.0 * cfg.simulation.gravity);
    assert!(
        (height - expected).abs() < 1e-3,
        "the jump height was {height} m, expected {expected} m"
    );
    // The document's figure: a downward velocity of 1 m/s gives about 1.8 cm.
    assert!((expected - 0.0183).abs() < 0.001);
}

#[test]
fn a_twice_as_hard_jump_shot_reaches_about_seven_centimetres() {
    let cfg = TableConfig::default();
    let target = 2.0_f64;
    let elevation = 20.0_f64.to_radians();
    let speed = target
        / (cfg.cue.cue_mass * (1.0 + cfg.cue.e_cue) / (cfg.ball.mass + cfg.cue.cue_mass)
            * elevation.sin());
    let direction = Vec3::new(0.0, elevation.cos(), -elevation.sin());
    let input = ShotInput::new(speed, direction, vec![ball(&cfg, 0, 1.0, 0.635)]);
    let height = peak_height(&cfg, &input, 0);
    let expected = (cfg.slate.e_table * target).powi(2) / (2.0 * cfg.simulation.gravity);
    assert!((height - expected).abs() < 1e-3);
    // The document's figure: 2 m/s gives about 7 cm.
    assert!((expected - 0.0735).abs() < 0.002);
}

#[test]
fn a_flat_cue_does_not_make_the_ball_jump() {
    let cfg = TableConfig::default();
    let input = ShotInput::new(2.0, Vec3::X, vec![ball(&cfg, 0, 1.0, 0.635)]);
    let height = peak_height(&cfg, &input, 0);
    assert!(height < 1e-9, "the ball left the cloth: {height} m");
}

// ---------------------------------------------------------------- the cushion hop

#[test]
fn a_two_metre_per_second_cushion_hit_hops_about_three_point_eight_millimetres() {
    let cfg = TableConfig::default();
    let geom = TableGeometry::new(&cfg);
    let mut ball = ball(&cfg, 0, 0.5, 0.635);
    ball.velocity = Vec3::new(2.0, 0.0, 0.0);
    ball.angular_velocity = Vec3::new(0.0, 2.0 / cfg.ball.radius, 0.0);

    let outcome = cushion::resolve_nose_cushion(&ball, Vec3::X, Vec3::Y, &cfg, &geom.cushion);
    // The cushion presses the ball into the slate.
    assert!(outcome.velocity.z < 0.0);
    let rebound = -cfg.slate.e_table * outcome.velocity.z;
    let hop = rebound * rebound / (2.0 * cfg.simulation.gravity);
    assert!(
        (hop - 0.0038).abs() < 0.0002,
        "the hop was {hop} m, expected about 3.8 mm"
    );
}

#[test]
fn the_three_reference_nose_heights_reproduce_dr_daves_account() {
    // The check that pins the cushion geometry: 0.500 D launches the ball, 0.635 D
    // presses it into the slate and 0.700 D drives it down hard.
    let cfg = TableConfig::default();
    let cases = [
        (0.500_f64, 0.49_f64),
        (0.635, -0.455624),
        (0.700, -0.871521),
    ];
    for (nose_over_d, expected) in cases {
        let sin_theta = nose_over_d * 2.0 - 1.0;
        let cos_theta = (1.0 - sin_theta * sin_theta).sqrt();
        let mut ball = ball(&cfg, 0, 0.5, 0.635);
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
        assert!(
            (outcome.velocity.z - expected).abs() < 1e-5,
            "at {nose_over_d} D the vertical impulse was {}, expected {expected}",
            outcome.velocity.z
        );
    }
}

#[test]
fn the_vertical_impulse_changes_sign_at_the_flip_threshold() {
    let cfg = TableConfig::default();
    let threshold =
        cfg.cushion.mu_cushion / (1.0 + cfg.cushion.mu_cushion * cfg.cushion.mu_cushion).sqrt();
    for (eps, expected_up) in [(threshold - 0.01, true), (threshold + 0.01, false)] {
        let sin_theta = eps;
        let cos_theta = (1.0 - sin_theta * sin_theta).sqrt();
        let mut ball = ball(&cfg, 0, 0.5, 0.635);
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
        assert_eq!(outcome.velocity.z > 0.0, expected_up);
    }
}

// ---------------------------------------------------------------- the table effect

#[test]
fn a_ten_metre_per_second_collision_lifts_the_ball_about_a_centimetre_and_a_half() {
    let cfg = TableConfig::default();
    let mut a = ball(&cfg, 0, 0.0, 0.635);
    a.velocity = Vec3::new(10.0, 0.0, 0.0);
    let b = ball(&cfg, 1, 0.3, 0.635);
    let outcome = pool_core::ball_ball::resolve(&a, &b, &cfg);
    assert!((outcome.lift - cfg.ball_ball.mu_ball * 10.0).abs() < 1e-12);
    let height = outcome.lift * outcome.lift / (2.0 * cfg.simulation.gravity);
    assert!(
        (height - 0.0127).abs() < 0.0005,
        "the lift was {height} m, expected about 1.3 cm"
    );
}

#[test]
fn the_lift_only_applies_while_the_ball_is_on_the_bed() {
    let cfg = TableConfig::default();
    let mut a = ball(&cfg, 0, 0.0, 0.635);
    a.velocity = Vec3::new(10.0, 0.0, 0.0);
    let b = ball(&cfg, 1, 0.3, 0.635);
    let outcome = pool_core::ball_ball::resolve(&a, &b, &cfg);
    assert!(outcome.lift > 0.0);

    // In the simulation the lift only applies once: a ball that is already in the air
    // cannot pick up any more.
    let mut balls = vec![ball(&cfg, 0, 0.0, 0.635), ball(&cfg, 1, 0.3, 0.635)];
    balls[0].velocity = Vec3::new(10.0, 0.0, 0.0);
    let input = ShotInput::new(0.0, Vec3::X, balls);
    let state = try_simulate_state(&input, &cfg).expect("the shot runs");
    // The cue ball is in the air after the collision and it lands again.
    assert!(state.balls[0].position.z >= cfg.ball.radius - 1e-12);
}

// ---------------------------------------------------------------- the rail regimes

#[test]
fn a_ball_above_the_rail_top_clears_the_rail() {
    let cfg = TableConfig::default();
    // The rail's top edge is 15 mm above the nose; a ball whose lowest point is
    // above it sails over the table (§11.3).
    let centre = cfg.cushion.rail_top_height + cfg.ball.radius + 0.001;
    assert_eq!(
        cushion::regime_for_height(centre, cfg.ball.radius, &cfg),
        RailRegime::ClearOfRail
    );
    // A ball resting on the bed meets the nose rubber.
    assert_eq!(
        cushion::regime_for_height(cfg.ball.radius, cfg.ball.radius, &cfg),
        RailRegime::NoseRubber
    );
    // A ball in the band between the nose and the rail top meets the structure.
    let middle = cfg.cushion.nose_height + cfg.ball.radius + 0.005;
    assert_eq!(
        cushion::regime_for_height(middle, cfg.ball.radius, &cfg),
        RailRegime::RailStructure
    );
}

#[test]
fn a_ball_that_flies_over_the_rail_leaves_the_table() {
    let cfg = TableConfig::default();
    let geometry = TableGeometry::new(&cfg);
    let rail = &geometry.rails[2]; // the left rail, x = 0
    let centre = cfg.cushion.rail_top_height + cfg.ball.radius + 0.01;
    let mut ball = ball(&cfg, 0, 0.2, 0.635);
    ball.position.z = centre;
    // The ball is rising, so it clears the rail's top edge.
    ball.velocity = Vec3::new(-2.0, 0.0, 2.0);
    ball.motion_state = MotionState::Airborne;

    let accel = Vec3::new(0.0, 0.0, -cfg.simulation.gravity);
    assert!(
        cushion::time_of_impact(&ball, accel, rail, &geometry.cushion, &cfg, 10.0).is_none(),
        "a ball above the rail's top edge must not meet the rail"
    );

    // In the simulation the ball sails over the table and is off the table.
    let input = ShotInput::new(0.0, Vec3::X, vec![ball]);
    let state = try_simulate_state(&input, &cfg).expect("the shot runs");
    assert!(state.balls[0].is_off_table);
}

// ---------------------------------------------------------------- the pocket

/// Fires a ball into a corner pocket's centre line at the given speed.
fn shoot_into_the_corner_pocket(
    cfg: &TableConfig,
    speed: f64,
    offset: f64,
) -> pool_core::GameState {
    shoot_into_the_corner_pocket_from(cfg, speed, offset, 0.25)
}

/// The same, but starting a given distance in front of the pocket.
fn shoot_into_the_corner_pocket_from(
    cfg: &TableConfig,
    speed: f64,
    offset: f64,
    distance: f64,
) -> pool_core::GameState {
    let geometry = TableGeometry::new(cfg);
    let pocket = &geometry.pockets[0];
    let direction = pocket.centreline;
    // `offset` shifts the aim sideways, in units of the pocket's half width.
    let sideways = Vec2::new(-direction.y, direction.x) * (offset * pocket.mouth_width / 2.0);
    let start = pocket.mouth_centre - direction * distance + sideways;
    let mut ball = ball(cfg, 0, start.x, start.y);
    ball.velocity = Vec3::new(direction.x * speed, direction.y * speed, 0.0);
    let input = ShotInput::new(0.0, Vec3::X, vec![ball]);
    try_simulate_state(&input, cfg).expect("the shot runs")
}

#[test]
fn a_ball_on_the_centre_line_of_a_corner_pocket_drops() {
    let cfg = TableConfig::default();
    let state = shoot_into_the_corner_pocket(&cfg, 1.5, 0.0);
    assert!(state.balls[0].is_pocketed);
    assert_eq!(state.balls[0].pocket_state, PocketState::Pocketed);
}

#[test]
fn a_ball_that_is_too_slow_to_reach_the_drop_hangs_up() {
    let cfg = TableConfig::default();
    let slow = cfg.minimum_speed_to_drop(cfg.table.shelf_depth_corner) * 0.8;
    // On the centre line the ball does not meet a facing, so it runs out of speed on
    // the shelf and hangs up there.
    let state = shoot_into_the_corner_pocket_from(&cfg, slow, 0.0, 0.05);
    assert!(!state.balls[0].is_pocketed, "a slow ball must not drop");
    assert_eq!(state.balls[0].pocket_state, PocketState::HungUp);
}

#[test]
fn a_ball_that_hits_a_jaw_can_rattle_out_or_rattle_in() {
    let cfg = TableConfig::default();
    // A ball that enters the pocket off the centre line hits a facing. Depending on
    // the angle and the speed it either comes back out or still drops; what matters
    // is that the outcome is one of the three documented states and that it is the
    // same on every run.
    let mut outcomes = Vec::new();
    for offset in [-0.6, -0.3, 0.3, 0.6] {
        for speed in [0.8, 1.2, 1.8, 2.6] {
            let state = shoot_into_the_corner_pocket(&cfg, speed, offset);
            let outcome = state.balls[0].pocket_state;
            assert!(
                matches!(
                    outcome,
                    PocketState::OnTable
                        | PocketState::Entering
                        | PocketState::HungUp
                        | PocketState::Pocketed
                ),
                "unexpected pocket state {outcome:?}"
            );
            // The same shot has to give the same outcome every time.
            let again = shoot_into_the_corner_pocket(&cfg, speed, offset);
            assert_eq!(again.balls[0].pocket_state, outcome);
            outcomes.push(outcome);
        }
    }
    // The set of outcomes has to include more than one kind: the jaw angles matter.
    assert!(
        outcomes.iter().any(|o| *o == PocketState::Pocketed),
        "no ball dropped at all"
    );
}

#[test]
fn a_ball_can_rattle_out_of_the_pocket() {
    // The third failure mode of §9.2: the ball crosses the mouth, hits a facing and
    // comes back out onto the table.
    let cfg = TableConfig::default();
    let geometry = TableGeometry::new(&cfg);
    let pocket = &geometry.pockets[0];
    let inward = pocket.centreline;
    let along = Vec2::new(-inward.y, inward.x);
    let angle = 12.0_f64.to_radians();
    let direction = (-along * angle.cos() + inward * angle.sin())
        .normalize()
        .expect("a direction");
    let start = pocket.mouth_centre + along * 0.045 - inward * 0.015;
    let mut ball = ball(&cfg, 0, start.x, start.y);
    ball.velocity = Vec3::new(direction.x * 0.9, direction.y * 0.9, 0.0);
    let input = ShotInput::new(0.0, Vec3::X, vec![ball]);

    let mut simulation = pool_core::Simulation::new(&input, &cfg);
    let _ = simulation.run_without_strike();
    let kinds: Vec<EventKind> = simulation.events().iter().map(|e| e.kind).collect();
    assert!(
        kinds.contains(&EventKind::PocketEnter),
        "the ball did not enter the pocket: {kinds:?}"
    );
    assert!(
        kinds.contains(&EventKind::Jaw),
        "the ball did not hit a facing: {kinds:?}"
    );
    assert!(
        kinds.contains(&EventKind::PocketExit),
        "the ball did not rattle out: {kinds:?}"
    );
    // And it is back on the table.
    let state = try_simulate_state(&input, &cfg).expect("the shot runs");
    assert!(!state.balls[0].is_pocketed);
    assert_eq!(state.balls[0].pocket_state, PocketState::OnTable);
}

#[test]
fn a_ball_whose_centre_only_crosses_the_mouth_is_not_pocketed() {
    let cfg = TableConfig::default();
    let geometry = TableGeometry::new(&cfg);
    let pocket = &geometry.pockets[0];
    // A ball that passes the mouth far to the side of the pocket is not captured.
    let sideways = Vec2::new(-pocket.centreline.y, pocket.centreline.x) * 0.25;
    let start = pocket.mouth_centre + sideways - pocket.centreline * 0.2;
    let mut ball = ball(&cfg, 0, start.x, start.y);
    ball.velocity = Vec3::new(pocket.centreline.x, pocket.centreline.y, 0.0);
    let input = ShotInput::new(0.0, Vec3::X, vec![ball]);
    let state = try_simulate_state(&input, &cfg).expect("the shot runs");
    assert!(!state.balls[0].is_pocketed);
}

// ---------------------------------------------------------------- the cue

#[test]
fn an_offset_hit_beyond_the_miscue_limit_is_flagged() {
    let cfg = TableConfig::default();
    let mut input = ShotInput::new(2.0, Vec3::X, vec![ball(&cfg, 0, 1.0, 0.635)]);
    input.contact_offset_x = 0.6 * cfg.ball.radius;
    let state = simulate_shot(input, &cfg);
    assert!(state.miscue);
    assert!(state.foul);
}

#[test]
fn a_hit_inside_the_miscue_limit_is_not_flagged() {
    let cfg = TableConfig::default();
    let mut input = ShotInput::new(2.0, Vec3::X, vec![ball(&cfg, 0, 1.0, 0.635)]);
    input.contact_offset_x = 0.45 * cfg.ball.radius;
    let state = simulate_shot(input, &cfg);
    assert!(!state.miscue);
}

#[test]
fn the_squirt_grows_with_the_impact_parameter() {
    let cfg = TableConfig::default();
    let mut previous = 0.0;
    for step in 0..=10 {
        let b = 0.05 * step as f64;
        let angle = cue::squirt_angle(b, &cfg);
        assert!(angle >= previous - 1e-12);
        previous = angle;
    }
}
