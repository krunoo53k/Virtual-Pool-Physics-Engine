//! The robustness of the simulation: the shots a game actually plays.
//!
//! The core is asked for a shot with the balls where the game put them, and a
//! game puts them where `f32` can hold them. The difference between an exact
//! position and one that has been through an `f32` is a hundredth of a
//! millimetre, and it must not change whether the game hangs.
//!
//! These tests are the ones that would have caught the two bugs the game
//! found: the vertical spin that ran away and never died, and the ball that
//! went through a rail and rolled off into the dark.
//!
//! A ball that is *put* off the table is the game's business, not the core's:
//! the pocket mouths reach past the nose line, so a ball past the line is not
//! by itself off the table.

use pool_core::config::TableConfig;
use pool_core::math::Vec3;
use pool_core::state::{BallState, ShotInput};
use pool_core::try_simulate_shot_traced;

/// The core's frame in Godot's.
fn to_godot(v: Vec3) -> Vec3 {
    Vec3::new(v.x as f32 as f64, v.z as f32 as f64, -(v.y as f32 as f64))
}

/// Godot's frame in the core's.
fn to_core(v: Vec3) -> Vec3 {
    Vec3::new(v.x, -v.z, v.y)
}

/// A position on its way through Godot's `f32` vectors and back.
fn through_f32(v: Vec3) -> Vec3 {
    to_core(to_godot(v))
}

/// The eight-ball rack, the way the game puts it up.
fn rack(cfg: &TableConfig, round: bool) -> Vec<BallState> {
    let radius = cfg.ball.radius;
    let foot = Vec3::new(cfg.table.length / 4.0, cfg.table.width / 2.0, radius);
    let cue_ball = Vec3::new(3.0 * cfg.table.length / 4.0, cfg.table.width / 2.0, radius);
    let mut balls = vec![BallState::at_rest(cfg, 0, cue_ball)];
    let mut number = 1;
    for row in 0..5 {
        for column in 0..=row {
            let y = foot.y + (2.0 * column as f64 - row as f64) * radius;
            let x = foot.x - libm::sqrt(3.0) * radius * row as f64;
            let position = Vec3::new(x, y, radius);
            let position = if round {
                through_f32(position)
            } else {
                position
            };
            balls.push(BallState::at_rest(cfg, number, position));
            number += 1;
        }
    }
    if round {
        balls[0].position = through_f32(balls[0].position);
    }
    balls
}

#[test]
fn a_break_settles() {
    let cfg = TableConfig::default();
    let input = pool_core::ShotInput::new(8.0, Vec3::new(-1.0, 0.0, 0.0), rack(&cfg, false));
    let (state, trace) = try_simulate_shot_traced(&input, &cfg).expect("the break runs");
    assert!(
        trace.duration < 20.0,
        "the break took {} s, which is not a break",
        trace.duration
    );
    for (index, ball) in state.balls.iter().enumerate() {
        assert!(
            ball.is_settled() || ball.is_pocketed || ball.is_off_table,
            "ball {index} is still moving: {:?}",
            ball.velocity
        );
    }
}

#[test]
fn a_break_settles_with_the_positions_a_game_can_hold() {
    // The same break, with the positions on their way through Godot's `f32`.
    // A hundredth of a millimetre of rounding must not change the outcome.
    let cfg = TableConfig::default();
    let input = pool_core::ShotInput::new(8.0, Vec3::new(-1.0, 0.0, 0.0), rack(&cfg, true));
    let (state, trace) = try_simulate_shot_traced(&input, &cfg).expect("the break runs");
    assert!(
        trace.duration < 20.0,
        "the break took {} s, which is not a break",
        trace.duration
    );
    for (index, ball) in state.balls.iter().enumerate() {
        assert!(
            ball.is_settled() || ball.is_pocketed || ball.is_off_table,
            "ball {index} is still moving: {:?} at {:?}",
            ball.velocity,
            ball.position
        );
    }
    let off: Vec<u8> = state
        .balls
        .iter()
        .filter(|b| b.is_off_table)
        .map(|b| b.number)
        .collect();
    assert!(
        off.is_empty(),
        "the balls left the table: {:?} - a ball that rolls into a rail has to bounce",
        off
    );
}

#[test]
fn a_ball_a_nanometre_below_the_bed_does_not_fall_through_the_table() {
    // The bed is at exactly one radius. A game that keeps its balls in `f32` puts
    // the centre a fraction of a nanometre below that, and a ball that is a hair
    // below the bed and a hair above the ground is in no state at all: it cannot
    // land, because the landing time of a ball whose centre is below the bed does
    // not exist, and it falls through the table and off the world.
    let cfg = TableConfig::default();
    let mut balls = rack(&cfg, true);
    for ball in balls.iter_mut() {
        ball.position.z -= 5.0e-10;
    }
    let input = ShotInput::new(8.0, Vec3::new(-1.0, 0.0, 0.0), balls);
    let (state, _trace) = try_simulate_shot_traced(&input, &cfg).expect("the break runs");
    for (index, ball) in state.balls.iter().enumerate() {
        assert!(
            ball.is_settled() || ball.is_pocketed || ball.is_off_table,
            "ball {index} is still moving: {:?} at {:?}",
            ball.velocity,
            ball.position
        );
        assert!(
            ball.position.z >= cfg.ball.radius - 1e-9,
            "ball {index} is below the bed: {:?}",
            ball.position
        );
    }
}

#[test]
fn a_ball_with_a_tiny_vertical_speed_does_not_bounce_for_ever() {
    // The bed tolerance and the settle threshold of §8.2 have to agree. A ball on the
    // bed with a vertical speed far below the settle threshold must be absorbed on
    // its first bounce, not turn into a chain of near-zero events.
    let cfg = TableConfig::default();
    for speed in [1e-9_f64, 1e-6, 1e-3, 0.019] {
        let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(0.5, 0.635, cfg.ball.radius));
        ball.velocity = Vec3::new(0.5, 0.0, speed);
        let input = ShotInput::new(0.0, Vec3::X, vec![ball]);
        let mut simulation = pool_core::Simulation::new(&input, &cfg);
        let _ = simulation.run_without_strike();
        let events = simulation.events().len();
        assert!(
            events <= 4,
            "a ball with a vertical speed of {speed} m/s took {events} events"
        );
    }
}

#[test]
fn the_spin_of_a_ball_dies_and_does_not_grow() {
    // The vertical spin decays towards zero and stops there. A spin that grew
    // instead of dying would keep the ball alive for ever, and the game would
    // wait for a shot that never ends.
    let cfg = TableConfig::default();
    let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(0.5, 0.635, cfg.ball.radius));
    ball.velocity = Vec3::new(1.0, 0.0, 0.0);
    ball.angular_velocity = Vec3::new(0.0, 1.0 / cfg.ball.radius, 5.0);
    let input = pool_core::ShotInput::new(0.0, Vec3::X, vec![ball]);
    let (state, trace) = try_simulate_shot_traced(&input, &cfg).expect("the ball rolls");
    assert!(
        trace.duration < 20.0,
        "the roll took {} s, which is not a roll",
        trace.duration
    );
    assert!(
        state.balls[0].angular_velocity.z.abs() < 1e-9,
        "the ball ended with a vertical spin of {:?}",
        state.balls[0].angular_velocity
    );
}
