//! The mapping the game sees, and what it does to the shot.
//!
//! These are the tests that catch the mistake that matters: the tip dragged to
//! the shooter's right has to be right english, and right english has to throw
//! the object ball to the shooter's left. If the frame is flipped anywhere, the
//! english is inverted, and everything else can be perfect and still feel
//! wrong.
//!
//! The tests go through the same path the game does - the game's own
//! `contact_offset`, the game's own frame - and they assert the physics: where
//! the object ball ends up.

use godot::builtin::Vector3;
use pool_core::config::TableConfig;
use pool_core::math::Vec3;
use pool_core::state::GameState;
use pool_core::Trace;
use pool_godot::{frame, shot};

/// The cue ball and the object ball, in the **Godot** frame.
///
/// The table runs along `x`; `y` is up, so the balls sit a radius above the
/// bed, and `z` is across the table.
fn pair(table: &TableConfig) -> (Vector3, Vector3) {
    let cue_ball = frame::to_godot(Vec3::new(0.3, 0.635, table.ball.radius));
    let object_ball = frame::to_godot(Vec3::new(0.6, 0.635, table.ball.radius));
    (cue_ball, object_ball)
}

/// The shot, the way the game asks for it.
///
/// `across` is the tip to the shooter's right, `up` the tip above the centre.
///
/// The object ball is put on the line the cue ball **really** takes, which is
/// what the game does with the ghost ball: the aim line is not the line the
/// ball takes, because the tip pushes it aside (the squirt), and a ball that
/// arrives on a squirted line cuts the object ball and throws it sideways on
/// its own - sideways by much more than the english does. The english is only
/// visible once that is taken out of the way.
fn game_shot(table: &TableConfig, across: f64, up: f64) -> (GameState, Trace) {
    let (cue_ball, object_ball) = pair(table);
    let mut input = shot::shot_input(
        table,
        2.0,
        frame::to_core(frame::to_godot(Vec3::X)),
        (across, up),
        &[frame::to_core(cue_ball), frame::to_core(object_ball)],
        &[0, 1],
        // The pair is put on the table for these shots, so neither of them
        // carries an orientation into one (§11.2's hand-off is a fresh start).
        &[],
    );
    // The preview line: the same strike the shot makes.
    let strike = shot::strike(table, &input);
    let line = match strike.velocity.normalize() {
        Some(line) => line,
        None => panic!("the cue ball does not leave"),
    };
    let from = frame::to_core(cue_ball);
    input.ball_states[1].position = from + line * 0.3;
    shot::simulate(table, &input).expect("the shot runs")
}

/// Where the object ball started, in the core's frame.
fn object_ball_start(table: &TableConfig) -> Vec3 {
    frame::to_core(frame::to_godot(Vec3::new(0.6, 0.635, table.ball.radius)))
}

#[test]
fn a_tip_dragged_to_the_right_throws_the_object_ball_to_the_left() {
    let table = TableConfig::default();
    // The player drags the tip 10 mm to the shooter's right: right english.
    let (state, _) = game_shot(&table, 0.010, 0.0);
    // The object ball is thrown towards the shooter's left. In the core's
    // frame a shot along `+x` has the shooter's left at `+y`, so the object
    // ball ends up at a larger `y` than it started.
    let start = object_ball_start(&table);
    assert!(
        state.balls[1].position.y > start.y,
        "the object ball ended at y = {}, which is not past {start:?}",
        state.balls[1].position.y
    );
}

#[test]
fn a_tip_dragged_to_the_left_throws_the_object_ball_to_the_right() {
    let table = TableConfig::default();
    let (state, _) = game_shot(&table, -0.010, 0.0);
    let start = object_ball_start(&table);
    assert!(
        state.balls[1].position.y < start.y,
        "the object ball ended at y = {}, which is not before {start:?}",
        state.balls[1].position.y
    );
}

#[test]
fn the_two_english_shots_are_mirror_images() {
    let table = TableConfig::default();
    let (right, _) = game_shot(&table, 0.010, 0.0);
    let (left, _) = game_shot(&table, -0.010, 0.0);
    // The object ball ends up the same distance to either side.
    let start = object_ball_start(&table);
    let to_the_right = right.balls[1].position.y - start.y;
    let to_the_left = left.balls[1].position.y - start.y;
    assert!(
        (to_the_right + to_the_left).abs() < 1e-9,
        "the shots are not mirror images: {to_the_right} and {to_the_left}"
    );
}

#[test]
fn the_tip_above_the_centre_is_follow() {
    let table = TableConfig::default();
    // A tip above the centre is follow: the cue ball goes forward past the
    // contact. A tip below is draw.
    let (follow, _) = game_shot(&table, 0.0, 0.010);
    let (draw, _) = game_shot(&table, 0.0, -0.010);
    let contact = 0.3 + 2.0 * table.ball.radius;
    assert!(
        follow.balls[0].position.x > contact,
        "the follow ended at x = {}, which is not past {contact}",
        follow.balls[0].position.x
    );
    assert!(
        draw.balls[0].position.x < contact,
        "the draw ended at x = {}, which is not before {contact}",
        draw.balls[0].position.x
    );
}

#[test]
fn the_frame_survives_the_round_trip() {
    let table = TableConfig::default();
    let core = Vec3::new(1.25, 0.635, table.ball.radius);
    let back = frame::to_core(frame::to_godot(core));
    assert!(
        (back.x - core.x).abs() < 1e-6
            && (back.y - core.y).abs() < 1e-6
            && (back.z - core.z).abs() < 1e-6,
        "the core position {core:?} came back as {back:?}"
    );
}

#[test]
fn the_godot_frame_has_the_height_in_y() {
    let core = Vec3::new(1.25, 0.635, 0.0286);
    let godot = frame::to_godot(core);
    assert!(
        (godot.y - 0.0286).abs() < 1e-6,
        "the height is in y: {godot:?}"
    );
}
