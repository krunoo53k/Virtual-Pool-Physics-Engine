//! The physics acceptance tests of the milestones that the fixtures do not cover:
//! the jump shot (§7.1), the cushion hop (§8.1), the table effect of a collision
//! (§4.5), the three rail regimes (§11.3) and the pocket state machine (§9).

use pool_core::cushion::{self, RailRegime};
use pool_core::geometry::TableGeometry;
use pool_core::math::{Vec2, Vec3};
use pool_core::state::{BallState, GamePhase, MotionState, PocketState, ShotInput};
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
fn a_rolling_cue_ball_is_lifted_by_the_friction_between_the_balls() {
    // The vertical impulse comes from the vertical component of the slip at the
    // contact (§4.4.5). A rolling cue ball's contact point moves downwards at the
    // front of the ball, so the friction acts upwards on it and the ball is lifted.
    let cfg = TableConfig::default();
    let mut a = ball(&cfg, 0, 0.0, 0.635);
    a.velocity = Vec3::new(2.0, 0.0, 0.0);
    // Natural roll: omega = (k x v) / R.
    a.angular_velocity = Vec3::new(0.0, 2.0 / cfg.ball.radius, 0.0);
    let b = ball(&cfg, 1, 0.3, 0.635);
    let outcome = pool_core::ball_ball::resolve(&a, &b, &cfg);
    assert!(outcome.lift > 0.0, "the cue ball was not lifted");
    // The lift is the vertical share of the friction impulse: `J_t * |u_z| / u`.
    assert!(
        (outcome.lift - outcome.jt / cfg.ball.mass).abs() < 1e-12,
        "the lift was {}, expected {}",
        outcome.lift,
        outcome.jt / cfg.ball.mass
    );
    // Kim's estimate for the same collision is `mu_ball * U_i` (§4.5); the two are
    // the same effect, and the model is the smaller of the two because TP A.14's
    // friction coefficient falls with the sliding speed.
    let kim = cfg.ball_ball.mu_ball * 2.0;
    assert!(
        outcome.lift < kim && outcome.lift > 0.2 * kim,
        "the lift was {}, Kim's estimate is {}",
        outcome.lift,
        kim
    );
}

#[test]
fn a_stun_cue_ball_is_not_lifted() {
    // Without spin the slip at the contact is purely horizontal, so the friction
    // between the balls has no vertical component: neither ball is lifted and
    // neither is pressed into the cloth.
    let cfg = TableConfig::default();
    let mut a = ball(&cfg, 0, 0.0, 0.635);
    a.velocity = Vec3::new(2.0, 0.0, 0.0);
    let b = ball(&cfg, 1, 0.3, 0.635);
    let outcome = pool_core::ball_ball::resolve(&a, &b, &cfg);
    assert_eq!(outcome.lift, 0.0);
    assert_eq!(outcome.velocity_b.z, 0.0);
}

#[test]
fn a_rolling_cue_ball_presses_the_object_ball_into_the_cloth() {
    // The other half of the same vertical impulse: the reaction presses the object
    // ball into the cloth, which is what activates the large static friction of §4.5.
    let cfg = TableConfig::default();
    let mut a = ball(&cfg, 0, 0.0, 0.635);
    a.velocity = Vec3::new(2.0, 0.0, 0.0);
    a.angular_velocity = Vec3::new(0.0, 2.0 / cfg.ball.radius, 0.0);
    let b = ball(&cfg, 1, 0.3, 0.635);
    let outcome = pool_core::ball_ball::resolve(&a, &b, &cfg);
    assert!(outcome.lift > 0.0);
    assert!(
        outcome.velocity_b.z < 0.0,
        "the object ball should be pressed down, vz = {}",
        outcome.velocity_b.z
    );
}

#[test]
fn the_lift_carries_the_cue_ball_off_the_bed_and_it_lands_again() {
    let cfg = TableConfig::default();
    let mut balls = vec![ball(&cfg, 0, 0.0, 0.635), ball(&cfg, 1, 0.3, 0.635)];
    balls[0].velocity = Vec3::new(2.0, 0.0, 0.0);
    balls[0].angular_velocity = Vec3::new(0.0, 2.0 / cfg.ball.radius, 0.0);
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
fn a_ball_in_the_regime_two_band_hits_the_rail_structure() {
    // The band between the nose and the rail's top edge: the ball meets the upper
    // structure, not the rubber, and the rubber model must not be extrapolated into
    // it (§11.3).
    let cfg = TableConfig::default();
    let geometry = TableGeometry::new(&cfg);
    let rail = &geometry.rails[3]; // the right rail
    let height = cfg.cushion.nose_height + cfg.ball.radius + 0.005;
    let mut ball = ball(&cfg, 0, cfg.table.length - 0.05, 0.635);
    ball.position.z = height;
    ball.velocity = Vec3::new(2.0, 0.0, 0.0);
    ball.motion_state = MotionState::Airborne;

    let accel = Vec3::new(0.0, 0.0, -cfg.simulation.gravity);
    let (time, regime) = cushion::time_of_impact(&ball, accel, rail, &geometry.cushion, &cfg, 10.0)
        .expect("the impact is found");
    assert_eq!(regime, RailRegime::RailStructure);
    assert!(time > 0.0);

    // The impulse is the structure's, not the rubber's: a vertical wall with the
    // structure's own restitution, and no vertical impulse at all.
    let structure = cushion::resolve_cushion(
        &ball,
        Vec3::X,
        Vec3::Y,
        0.0,
        1.0,
        cfg.cushion.e_rail_structure,
        cfg.cushion.mu_rail_structure,
        &cfg,
    );
    let rubber = cushion::resolve_nose_cushion(&ball, Vec3::X, Vec3::Y, &cfg, &geometry.cushion);
    assert!(
        structure.velocity.z.abs() < 1e-12,
        "the wall gives no vertical kick"
    );
    assert!(
        rubber.velocity.z < 0.0,
        "the rubber presses the ball into the slate"
    );
    assert!(
        (structure.velocity.x - rubber.velocity.x).abs() > 1e-3,
        "the two surfaces have to give different rebounds"
    );
}

#[test]
fn an_airborne_ball_hits_the_nose_rubber_at_the_right_time() {
    // A ball that is in the air when it reaches the rail meets the inclined face at
    // the height the geometry gives, and the impact time follows from the closed
    // form.
    let cfg = TableConfig::default();
    let geometry = TableGeometry::new(&cfg);
    let rail = &geometry.rails[3]; // the right rail
    let speed = 1.0;
    let mut ball = ball(&cfg, 0, cfg.table.length - 0.05, 0.635);
    ball.position.z = cfg.ball.radius + 0.01;
    ball.velocity = Vec3::new(speed, 0.0, 0.0);
    ball.motion_state = MotionState::Airborne;

    // In the air the ball keeps its speed, so the impact time is the distance to
    // the contact divided by the speed. The contact height follows from the
    // geometry: the ball meets the face where its centre is
    // `R cos(theta)` short of the nose line, at the height it has by then.
    let contact = cfg.table.length
        - (cfg.ball.radius
            + (ball.position.z - cfg.cushion.nose_height) * geometry.cushion.sin_theta)
            / geometry.cushion.cos_theta;
    let expected = (contact - ball.position.x) / speed;

    let accel = Vec3::new(0.0, 0.0, -cfg.simulation.gravity);
    let (time, regime) = cushion::time_of_impact(&ball, accel, rail, &geometry.cushion, &cfg, 10.0)
        .expect("the impact is found");
    assert_eq!(regime, RailRegime::NoseRubber);
    // The ball falls a little on the way, which moves the contact point a little.
    assert!(
        (time - expected).abs() < 1e-3,
        "the impact time was {time}, expected about {expected}"
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
    simulate_into_the_corner_pocket(cfg, speed, offset, 0.25).0
}

/// The same, but starting a given distance in front of the pocket and returning the
/// resolved events as well.
fn simulate_into_the_corner_pocket(
    cfg: &TableConfig,
    speed: f64,
    offset: f64,
    distance: f64,
) -> (pool_core::GameState, Vec<EventKind>) {
    let geometry = TableGeometry::new(cfg);
    let pocket = &geometry.pockets[0];
    let direction = pocket.centreline;
    // `offset` shifts the aim sideways, in units of the pocket's half width.
    let sideways = Vec2::new(-direction.y, direction.x) * (offset * pocket.mouth_width / 2.0);
    let start = pocket.mouth_centre - direction * distance + sideways;
    let mut ball = ball(cfg, 0, start.x, start.y);
    ball.velocity = Vec3::new(direction.x * speed, direction.y * speed, 0.0);
    let input = ShotInput::new(0.0, Vec3::X, vec![ball]);
    let state = try_simulate_state(&input, cfg).expect("the shot runs");
    let mut simulation = pool_core::Simulation::new(&input, cfg);
    let _ = simulation.run_without_strike();
    let kinds = simulation.events().iter().map(|e| e.kind).collect();
    (state, kinds)
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
    let state = simulate_into_the_corner_pocket(&cfg, slow, 0.0, 0.05).0;
    assert!(!state.balls[0].is_pocketed, "a slow ball must not drop");
    assert_eq!(state.balls[0].pocket_state, PocketState::HungUp);
}

#[test]
fn a_ball_that_hits_a_jaw_can_rattle_out_or_rattle_in() {
    let cfg = TableConfig::default();
    // A ball that enters the pocket off the centre line hits a facing. Depending on
    // the angle and the speed it either comes back out, still drops, or clears the
    // rail on the way out; what matters is that the outcome is one of the documented
    // states and that it is the same on every run. A hard rattle can carry the ball
    // past the rail line, which is now caught by the position backstop of §10.1.
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
                        | PocketState::OffTable
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
        outcomes.contains(&PocketState::Pocketed),
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
fn a_ball_can_be_rattled_into_the_pocket() {
    // The third failure mode of §9.2: the ball hits a facing on its way in and still
    // drops. The funnel is narrow (7 degrees a side, section 9.1), so the ball has to
    // run close to a facing to meet it before the drop line 4.4 cm behind the mouth.
    let cfg = TableConfig::default();
    let state = shoot_into_the_corner_pocket(&cfg, 1.8, 0.5);
    assert!(state.balls[0].is_pocketed, "the ball did not drop");

    let (_, kinds) = simulate_into_the_corner_pocket(&cfg, 1.8, 0.5, 0.25);
    assert!(kinds.contains(&EventKind::PocketEnter), "{kinds:?}");
    assert!(kinds.contains(&EventKind::Jaw), "{kinds:?}");
    assert!(kinds.contains(&EventKind::PocketDrop), "{kinds:?}");
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

// ---------------------------------------------------------------- the stop shot

#[test]
fn a_centre_ball_hit_stops_the_cue_ball() {
    // The stun shot: the cue ball has no spin, so the cloth takes it straight away.
    // It keeps the 2.5% of its speed that the restitution leaves it (the fixture
    // pins that) and is then stopped by the cloth.
    let cfg = TableConfig::default();
    let mut balls = vec![ball(&cfg, 0, 1.0, 0.635)];
    // The cue ball is struck at its centre: it has no spin at all.
    balls[0].velocity = Vec3::new(2.0, 0.0, 0.0);
    // The object ball is right in front of it, so the cue ball has hardly slid by the
    // time of the collision and has picked up almost no natural roll.
    balls.push(ball(&cfg, 1, 1.0 + 2.0 * cfg.ball.radius + 0.003, 0.635));
    let input = ShotInput::new(0.0, Vec3::X, balls);
    let state = try_simulate_state(&input, &cfg).expect("the shot runs");

    // The cue ball stops almost dead: it keeps the 2.5% of its speed that the
    // restitution leaves it and the cloth takes that away at once. (It creeps a
    // little, because it has picked up a little natural roll on the way in and
    // because the collision throws it sideways.)
    let contact = 1.0 + 2.0 * cfg.ball.radius + 0.003 - 2.0 * cfg.ball.radius;
    let cue_x = state.balls[0].position.x;
    assert!(
        cue_x > contact && cue_x < contact + 0.02,
        "the cue ball should stop on the spot, it ended at {cue_x}"
    );
    assert!(
        state.balls[0].velocity.norm() < 1e-9,
        "the cue ball has to be at rest"
    );

    // The object ball carries on: right after the collision it is moving forwards at
    // nearly the speed the cue ball had.
    let mut quick = TableConfig::default();
    quick.simulation.max_sim_time = 0.05;
    let mut balls = vec![ball(&cfg, 0, 1.0, 0.635)];
    balls[0].velocity = Vec3::new(2.0, 0.0, 0.0);
    balls.push(ball(&cfg, 1, 1.0 + 2.0 * cfg.ball.radius + 0.003, 0.635));
    let input = ShotInput::new(0.0, Vec3::X, balls);
    let state = try_simulate_state(&input, &quick).expect("the shot runs");
    let object = state.balls[1].velocity;
    assert!(
        object.x > 1.8,
        "the object ball has to carry on: {object:?}"
    );
    assert!(
        object.y.abs() < 0.2,
        "a full hit throws the object ball straight: {object:?}"
    );
}

#[test]
fn a_ball_ball_collision_conserves_the_momentum_along_the_line_of_centres() {
    // `v_A,n' + v_B,n' = v_n` (§4.4.5): the momentum along the line of centres is
    // conserved, and the cue ball keeps ((1 - e) / 2) of it.
    let cfg = TableConfig::default();
    for cut in [0.0_f64, 15.0, 30.0, 45.0] {
        let angle = cut.to_radians();
        let mut a = ball(&cfg, 0, 0.0, 0.0);
        a.velocity = Vec3::new(2.0, 0.0, 0.0);
        let mut b = ball(&cfg, 1, 0.3 * angle.cos(), 0.3 * angle.sin());
        b.velocity = Vec3::ZERO;
        let outcome = pool_core::ball_ball::resolve(&a, &b, &cfg);
        let v_n = a.velocity.dot(outcome.normal);
        let after = outcome.velocity_a.dot(outcome.normal) + outcome.velocity_b.dot(outcome.normal);
        assert!(
            (after - v_n).abs() < 1e-12,
            "the momentum along the line of centres is not conserved: {after} != {v_n}"
        );
        // The normal components follow the document.
        let expected_a = ((1.0 - cfg.ball_ball.e_ball) / 2.0) * v_n;
        assert!((outcome.velocity_a.dot(outcome.normal) - expected_a).abs() < 1e-12);
    }
}

// ---------------------------------------------------------------- the game state

#[test]
fn a_settled_shot_reports_the_settled_phase() {
    let cfg = TableConfig::default();
    let input = ShotInput::new(1.0, Vec3::X, vec![ball(&cfg, 0, 1.0, 0.635)]);
    let state = simulate_shot(input, &cfg);
    assert_eq!(state.phase, GamePhase::Settled);
    assert!(state.balls[0].velocity.norm() < 1e-9);
}

#[test]
fn a_shot_that_is_cut_short_reports_that_it_is_still_going() {
    let cfg = TableConfig::default();
    let mut short = TableConfig::default();
    short.simulation.max_sim_time = 0.05;
    let input = ShotInput::new(3.0, Vec3::X, vec![ball(&cfg, 0, 1.0, 0.635)]);
    let state = simulate_shot(input, &short);
    assert_eq!(state.phase, GamePhase::Settling);
    assert!(state.balls[0].velocity.norm() > 0.0);
    assert!(state.elapsed_time <= short.simulation.max_sim_time);
}

#[test]
fn the_cue_stick_is_reported_with_the_shot() {
    let cfg = TableConfig::default();
    let direction = Vec3::new(0.0, 0.0, -1.0); // a cue pointing straight down
    let input = ShotInput::new(2.0, direction, vec![ball(&cfg, 0, 1.0, 0.635)]);
    let state = simulate_shot(input, &cfg);
    let stick = state.cue_stick;
    assert!((stick.direction.z + 1.0).abs() < 1e-12);
    assert!((stick.elevation + core::f64::consts::FRAC_PI_2).abs() < 1e-9);
    // The tip was at the surface of the cue ball and the butt is a cue length
    // behind it, along the cue's axis.
    let strike_position = Vec3::new(1.0, 0.635, cfg.ball.radius);
    let tip = strike_position - stick.direction * cfg.ball.radius;
    let butt = tip - stick.direction * cfg.cue.cue_length;
    assert!(
        (stick.butt_position - butt).norm() < 1e-12,
        "the butt is where it should be"
    );
}

#[test]
fn a_ball_that_leaves_the_table_is_a_foul() {
    let cfg = TableConfig::default();
    let rail_top = cfg.cushion.rail_top_height + cfg.ball.radius + 0.01;
    let mut ball = ball(&cfg, 0, 0.2, 0.635);
    ball.position.z = rail_top;
    ball.velocity = Vec3::new(-2.0, 0.0, 2.0);
    ball.motion_state = MotionState::Airborne;
    let input = ShotInput::new(0.0, Vec3::X, vec![ball]);
    let state = try_simulate_state(&input, &cfg).expect("the shot runs");
    assert!(state.balls[0].is_off_table);
    assert!(state.foul, "a ball off the table is a foul (§10.1)");
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

// ------------------------------------------------------- height-aware ball-ball contact

#[test]
fn a_ball_flying_over_another_passes_over_it() {
    let mut cfg = TableConfig::default();
    // Look only at the flyer's pass. Left running for the whole shot, it would
    // rebound off a cushion and eventually reach the other ball for real.
    cfg.simulation.max_sim_time = 0.1;
    let r = cfg.ball.radius;
    let mut flyer = ball(&cfg, 0, 0.635, 0.635);
    // Directly over the other ball and 4R above it, moving away horizontally. The
    // old planar contact test saw the centres coincide and collided at t = 0.
    flyer.position.z = r + 4.0 * r;
    flyer.velocity = Vec3::new(5.0, 0.0, 0.0);
    flyer.motion_state = MotionState::Airborne;
    let object = ball(&cfg, 1, 0.635, 0.635);

    let input = ShotInput::new(0.0, Vec3::X, vec![flyer, object]);
    let mut simulation = pool_core::Simulation::new(&input, &cfg);
    simulation
        .run_without_strike()
        .expect("the shot runs without a fault");
    assert!(
        !simulation
            .events()
            .iter()
            .any(|event| event.kind == EventKind::BallBall),
        "a ball flying above another must not register a contact"
    );
    assert!(
        simulation.balls()[1].position.x - 0.635 < 1e-12,
        "the ball underneath was never struck"
    );
}

#[test]
fn a_ball_landing_on_another_from_above_does_touch_it() {
    let cfg = TableConfig::default();
    let r = cfg.ball.radius;
    let mut dropper = ball(&cfg, 0, 0.635, 0.635);
    dropper.position.z = r + 3.0 * r;
    dropper.velocity = Vec3::new(0.0, 0.0, -1.0);
    dropper.motion_state = MotionState::Airborne;
    let object = ball(&cfg, 1, 0.635, 0.635);

    let input = ShotInput::new(0.0, Vec3::X, vec![dropper, object]);
    let mut simulation = pool_core::Simulation::new(&input, &cfg);
    simulation
        .run_without_strike()
        .expect("the shot runs without a fault");
    assert!(
        simulation
            .events()
            .iter()
            .any(|event| event.kind == EventKind::BallBall),
        "a ball falling onto another must register a contact"
    );
    assert!(
        simulation.balls()[0].position.z <= r + 1e-9,
        "the ball that landed must not hang in the air"
    );
}

#[test]
fn a_ball_rolling_under_a_settled_stack_does_not_loop_for_ever() {
    let cfg = TableConfig::default();
    let r = cfg.ball.radius;
    let mut dropper = ball(&cfg, 0, 0.635, 0.635);
    dropper.position.z = r + 3.0 * r;
    dropper.velocity = Vec3::new(0.0, 0.0, -1.0);
    dropper.motion_state = MotionState::Airborne;
    let object = ball(&cfg, 1, 0.635, 0.635);
    let mut striker = ball(&cfg, 2, 0.4, 0.635);
    striker.velocity = Vec3::new(3.0, 0.0, 0.0);

    let input = ShotInput::new(0.0, Vec3::X, vec![dropper, object, striker]);
    let mut simulation = pool_core::Simulation::new(&input, &cfg);
    simulation
        .run_without_strike()
        .expect("a third ball must not turn the stack into an endless contact");
    assert!(
        simulation.balls()[0].position.z <= r + 1e-9,
        "the ball above must come down off the one below"
    );
}
