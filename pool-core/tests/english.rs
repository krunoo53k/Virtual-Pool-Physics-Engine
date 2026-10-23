//! The sign of the english: the one convention the whole game hangs on.
//!
//! The tip strikes the cue ball off centre. The side it strikes decides which
//! way the cue ball spins, which way it squirts, and which way it throws the
//! object ball - and every one of those is a sign on a number the game passes
//! to the core.
//!
//! The core's frame is right handed: `x` runs along the table, `y` across it and
//! `z` is up. The shooter's right hand is `direction x up`, so for a shot along
//! `+x` the shooter's right is `-y` and the shooter's left is `+y`. The core
//! measures the contact offset along `up x direction` (§6.1), which is that same
//! `+y`, so:
//!
//! * a **positive** `contact_offset_x` strikes the shooter's **left**;
//! * a **negative** `contact_offset_x` strikes the shooter's **right**.
//!
//! Right english is the case to watch, because it is the one the throw depends
//! on: the tip strikes the right of the ball, the ball leaves with a spin that
//! is counter clockwise seen from above, and the friction at the ball-ball
//! contact drags the object ball to the shooter's **left** (§4.4.2).
//!
//! These tests pin that with the physics, not with a label: they assert the
//! direction the object ball is actually thrown in.
//!
//! One trap is worth naming. The cue ball does not travel along the aim line:
//! it squirts away from the side the tip struck (§6.1), so by the time it
//! reaches the object ball it has drifted sideways and the hit is a cut, not a
//! full one. A cut throws the object ball sideways on its own, which is far
//! bigger than the english does, and it hides the english completely. The tests
//! below therefore put the object ball **on the line the cue ball actually
//! takes**, so the hit is full and the only thing that can move the object ball
//! sideways is the english.

use pool_core::ball_ball;
use pool_core::config::TableConfig;
use pool_core::math::Vec3;
use pool_core::state::BallState;
use pool_core::{cue, try_simulate_shot_traced, EventKind, ShotInput};

/// The cue ball and the object ball, straight in line along `+x`.
fn pair(cfg: &TableConfig) -> (BallState, BallState) {
    let cue_ball = BallState::at_rest(cfg, 0, Vec3::new(0.3, 0.635, cfg.ball.radius));
    let object_ball = BallState::at_rest(cfg, 1, Vec3::new(0.6, 0.635, cfg.ball.radius));
    (cue_ball, object_ball)
}

/// The shot: a straight stroke along `+x` with the tip `offset` to the side.
fn shot(cfg: &TableConfig, offset: f64) -> ShotInput {
    let (cue_ball, object_ball) = pair(cfg);
    let mut input = ShotInput::new(2.0, Vec3::X, vec![cue_ball, object_ball]);
    input.contact_offset_x = offset;
    input
}

/// The same shot, but with the object ball on the line the cue ball really
/// takes, so that the hit is full and the english is the only sideways force.
fn full_hit_shot(cfg: &TableConfig, offset: f64) -> ShotInput {
    let mut input = shot(cfg, offset);
    let strike = cue::strike(&input, cfg);
    let line = match strike.velocity.normalize() {
        Some(line) => line,
        None => return input,
    };
    let from = input.ball_states[0].position;
    input.ball_states[1].position = from + line * 0.3;
    input
}

/// The strike, and the collision that follows it.
fn collide(cfg: &TableConfig, offset: f64) -> ball_ball::BallBallOutcome {
    let input = full_hit_shot(cfg, offset);
    let strike = cue::strike(&input, cfg);
    let mut a = input.ball_states[0];
    let b = input.ball_states[1];
    a.velocity = strike.velocity;
    a.angular_velocity = strike.angular_velocity;
    ball_ball::resolve(&a, &b, cfg)
}

#[test]
fn a_positive_offset_strikes_the_shooters_left() {
    let cfg = TableConfig::default();
    let input = shot(&cfg, 0.010);
    let strike = cue::strike(&input, &cfg);
    // The tip is on the `+y` side, which is the shooter's left for a shot along
    // `+x`, so the spin about the vertical is clockwise: `omega_z < 0`.
    assert!(
        strike.angular_velocity.z < 0.0,
        "the spin was {:?}, which is not clockwise seen from above",
        strike.angular_velocity
    );
    assert!(
        strike.angular_velocity.y == 0.0,
        "a side offset must not spin the ball about the roll axis"
    );
}

#[test]
fn a_negative_offset_strikes_the_shooters_right() {
    let cfg = TableConfig::default();
    let input = shot(&cfg, -0.010);
    let strike = cue::strike(&input, &cfg);
    // The other way round: the tip is on the `-y` side, the shooter's right, and
    // the spin is counter clockwise seen from above.
    assert!(
        strike.angular_velocity.z > 0.0,
        "the spin was {:?}, which is not counter clockwise seen from above",
        strike.angular_velocity
    );
}

#[test]
fn the_squirt_points_away_from_the_side_that_was_struck() {
    let cfg = TableConfig::default();
    // The tip strikes the left, so the ball leaves to the right, and the other
    // way round (§6.1).
    let left = cue::strike(&shot(&cfg, 0.010), &cfg).velocity;
    let right = cue::strike(&shot(&cfg, -0.010), &cfg).velocity;
    assert!(left.y < 0.0, "the left english squirted to {left:?}");
    assert!(right.y > 0.0, "the right english squirted to {right:?}");
}

#[test]
fn a_tip_above_or_below_the_centre_does_not_squirt() {
    // The squirt comes from the tip sliding on the ball, and the tip slides
    // *sideways*. A tip above the centre gives follow and a tip below it gives
    // draw, and neither of them slides sideways: those shots go straight down
    // the aim line.
    //
    // The impact parameter counts a tip that is above or below the centre as an
    // off-centre hit too, so the squirt used to be looked up at the wrong
    // number: a draw or a follow shot came off the line towards the shooter's
    // right, and the player saw the cue ball leave to the side the moment it
    // was struck.
    let cfg = TableConfig::default();
    for offset in [0.005, 0.010, -0.005, -0.010] {
        let mut input = shot(&cfg, 0.0);
        input.contact_offset_y = offset;
        let strike = cue::strike(&input, &cfg);
        assert!(
            strike.velocity.y.abs() < 1e-12,
            "a tip {offset} above the centre squirted to {:?}",
            strike.velocity
        );
        // And the spin is there: the tip above the centre gives topspin and
        // the tip below it gives backspin.
        assert!(
            strike.angular_velocity.y * offset > 0.0,
            "a tip {offset} from the centre spun the ball {:?}",
            strike.angular_velocity
        );
    }
}

#[test]
fn the_squirt_depends_only_on_the_sideways_offset() {
    // The same english with and without follow or draw squirts the same way:
    // the sideways offset is what makes the tip slide. The *direction* is what
    // is compared: an offset hit also leaves the ball slower than a centre one
    // (section 6.2), and a vertical offset makes it slower again, so the
    // sideways component of the velocity scales with the speed even though the
    // deflection angle does not change.
    let cfg = TableConfig::default();
    let plain = cue::strike(&shot(&cfg, 0.010), &cfg);
    let plain_tangent = plain.velocity.y / plain.velocity.x;
    for height in [0.005, -0.005, 0.010] {
        let mut input = shot(&cfg, 0.010);
        input.contact_offset_y = height;
        let strike = cue::strike(&input, &cfg);
        let tangent = strike.velocity.y / strike.velocity.x;
        assert!(
            (tangent - plain_tangent).abs() < 1e-12,
            "a follow of {height} changed the squirt: {:?} against {:?}",
            strike.velocity,
            plain.velocity
        );
    }
}

#[test]
fn right_english_throws_the_object_ball_to_the_shooters_left() {
    let cfg = TableConfig::default();
    // Right english is the tip on the shooter's right: a negative offset.
    let outcome = collide(&cfg, -0.010);
    // The cue ball spins counter clockwise seen from above, so its surface slides
    // towards `+y` at the contact, and the friction drags the object ball with
    // it: the object ball is thrown towards the shooter's left.
    assert!(
        outcome.velocity_b.y > 0.0,
        "the object ball left with {:?}, which is not towards +y",
        outcome.velocity_b
    );
    assert!(
        outcome.angular_velocity_a.z > 0.0,
        "the cue ball kept the spin {:?}, which is not counter clockwise",
        outcome.angular_velocity_a
    );
}

#[test]
fn left_english_throws_the_object_ball_to_the_shooters_right() {
    let cfg = TableConfig::default();
    let outcome = collide(&cfg, 0.010);
    assert!(
        outcome.velocity_b.y < 0.0,
        "the object ball left with {:?}, which is not towards -y",
        outcome.velocity_b
    );
    assert!(
        outcome.angular_velocity_a.z < 0.0,
        "the cue ball kept the spin {:?}, which is not clockwise",
        outcome.angular_velocity_a
    );
}

#[test]
fn the_two_throws_are_mirror_images() {
    let cfg = TableConfig::default();
    let right = collide(&cfg, -0.010);
    let left = collide(&cfg, 0.010);
    assert!(
        (right.velocity_b.y + left.velocity_b.y).abs() < 1e-12,
        "the throws are not symmetric: {:?} and {:?}",
        right.velocity_b,
        left.velocity_b
    );
    assert!(
        (right.velocity_b.x - left.velocity_b.x).abs() < 1e-12,
        "the english changed the speed along the line of centres: {:?} and {:?}",
        right.velocity_b,
        left.velocity_b
    );
}

#[test]
fn the_throw_is_capped_by_the_ball_friction() {
    let cfg = TableConfig::default();
    // The tangential impulse is limited to `mu_ball(u) * J_n`, so the throw cannot
    // exceed `atan(mu_ball(u))` however much english there is (§4.4.5). The
    // coefficient falls with the sliding speed, so the cap is the *largest* one,
    // at the slowest slip: `mu_ball` is the coefficient at a 1 m/s slip, and the
    // law gives a bigger one below that.
    for offset in [-0.002, -0.005, -0.010] {
        let outcome = collide(&cfg, offset);
        // The throw is measured against the line of centres, not against the
        // table: the cue ball arrives on a squirted line.
        let along = outcome.velocity_b.dot(outcome.normal);
        let across = outcome.velocity_b.dot(outcome.tangent);
        let throw = across.atan2(along).to_degrees();
        // The cap is the friction law's own limit at the slip speed of this
        // collision, and the throw may not pass it.
        let limit = ball_ball::friction_at_speed(cfg.ball_ball.mu_ball, outcome.slip)
            .atan()
            .to_degrees();
        assert!(
            throw <= limit + 1e-9,
            "the throw was {throw} degrees, which is past the {limit} degree cap"
        );
        // The limit is the reference value only at the reference slip speed; below
        // it the law gives a bigger coefficient and a bigger cap.
        assert!(limit > 0.0, "the cap is {limit} degrees");
        assert!(
            throw > 0.0,
            "the throw was {throw} degrees, which is no throw"
        );
    }
}

#[test]
fn a_shot_with_right_english_throws_the_object_ball_to_the_left_end_to_end() {
    let cfg = TableConfig::default();
    let input = full_hit_shot(&cfg, -0.010);
    let (state, trace) = try_simulate_shot_traced(&input, &cfg).expect("the shot runs");
    // The collision, as the simulation resolved it.
    let collision = trace
        .events
        .iter()
        .find(|event| event.kind == EventKind::BallBall)
        .expect("the cue ball hit the object ball");
    // Just after the collision the object ball is on its way.
    let after = trace.sample(collision.time + 1e-9, &cfg);
    assert!(
        after[1].velocity.y > 0.0,
        "the object ball left with {:?}, which is not towards +y",
        after[1].velocity
    );
    // And it ends up on that side of the table.
    let start = input.ball_states[1].position.y;
    assert!(
        state.balls[1].position.y > start,
        "the object ball ended at y = {}, which is not past {start}",
        state.balls[1].position.y
    );
}

#[test]
fn a_shot_with_left_english_throws_the_object_ball_to_the_right_end_to_end() {
    let cfg = TableConfig::default();
    let input = full_hit_shot(&cfg, 0.010);
    let (state, trace) = try_simulate_shot_traced(&input, &cfg).expect("the shot runs");
    let collision = trace
        .events
        .iter()
        .find(|event| event.kind == EventKind::BallBall)
        .expect("the cue ball hit the object ball");
    let after = trace.sample(collision.time + 1e-9, &cfg);
    assert!(
        after[1].velocity.y < 0.0,
        "the object ball left with {:?}, which is not towards -y",
        after[1].velocity
    );
    let start = input.ball_states[1].position.y;
    assert!(
        state.balls[1].position.y < start,
        "the object ball ended at y = {}, which is not before {start}",
        state.balls[1].position.y
    );
}

#[test]
fn the_english_stays_inside_the_miscue_limit() {
    let cfg = TableConfig::default();
    // The limit is 14.7 mm on a 28.575 mm ball, so the offsets above are well
    // inside it.
    let limit = cfg.max_offset_without_miscue();
    assert!((limit - 0.0147).abs() < 0.0002, "the limit is {limit}");
    assert!(0.010 < limit);
    // Inside it the tip holds.
    let strike = cue::strike(&shot(&cfg, 0.9 * limit), &cfg);
    assert!(!strike.miscue);
    // Past it, the tip slips off.
    let strike = cue::strike(&shot(&cfg, 1.2 * limit), &cfg);
    assert!(strike.miscue, "the tip should have slipped off");
}
