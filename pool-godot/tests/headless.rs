//! The shot the Godot side plays back in its smoke test.
//!
//! The same shot is described in `godot-project/scripts/smoke_test.gd`, and
//! both sides pin the same hash: if the extension moves a number on the way
//! from Godot to the core, both sides stop agreeing, and the check fails on
//! whichever side changed.
//!
//! The positions are on sixteenths of a metre, because they have to survive
//! the trip through Godot's `f32` vectors unchanged - and the height of the
//! balls is the radius of the ball, which is what the game places them at.

use godot::builtin::Vector3;
use pool_core::config::TableConfig;
use pool_core::math::Vec3;
use pool_godot::{frame, shot};

/// The hash of the shot, pinned.
///
/// The same value is in `godot-project/scripts/smoke_test.gd`.
const HASH: &str = "c399229b25faae5a";

/// The cue ball and the fifteen object balls, in the core's frame.
///
/// The rack is a four by four grid with one ball missing, and the cue ball
/// sits a quarter of a metre to the side of it.
fn rack(table: &TableConfig) -> Vec<Vec3> {
    let radius = table.ball.radius;
    let mut balls = vec![Vec3::new(0.25, 0.5625, radius)];
    for row in 0..4 {
        for column in 0..4 {
            if row == 0 && column == 0 {
                continue;
            }
            let x = 0.5 + 0.0625 * column as f64;
            let y = 0.5 + 0.0625 * row as f64;
            balls.push(Vec3::new(x, y, radius));
        }
    }
    balls
}

/// The shot: the cue ball into the rack, at break speed, along `+y`.
fn the_shot(table: &TableConfig) -> pool_core::state::GameState {
    // The positions go through Godot's frame, because that is the way the
    // game gives them: a `Vector3` is an `f32` triple.
    let positions: Vec<Vector3> = rack(table).iter().map(|p| frame::to_godot(*p)).collect();
    let core_positions: Vec<Vec3> = positions.iter().map(|p| frame::to_core(*p)).collect();
    let numbers: Vec<i32> = (0..core_positions.len() as i32).collect();
    let direction = frame::to_core(frame::to_godot(Vec3::Y));
    let input = shot::shot_input(table, 8.0, direction, (0.0, 0.0), &core_positions, &numbers);
    let (state, _) = shot::simulate(table, &input).expect("the shot runs");
    state
}

#[test]
fn the_shot_has_the_pinned_hash() {
    let table = TableConfig::default();
    let state = the_shot(&table);
    let hash = pool_core::hash_state_hex(&state);
    assert_eq!(hash, HASH, "the shot's hash moved");
}

#[test]
fn the_shot_is_the_same_every_time() {
    let table = TableConfig::default();
    let first = pool_core::hash_state_hex(&the_shot(&table));
    let second = pool_core::hash_state_hex(&the_shot(&table));
    assert_eq!(first, second);
}

#[test]
fn the_rack_survives_the_trip_through_the_godot_frame() {
    let table = TableConfig::default();
    for ball in rack(&table) {
        let back = frame::to_core(frame::to_godot(ball));
        // The sixteenths survive; the radius does not, because it is not a
        // number an `f32` holds exactly. That is fine: the game places the
        // balls where the engine can hold them, and the physics reads that
        // position as it is.
        assert!(
            (back.x - ball.x).abs() < 1e-6 && (back.y - ball.y).abs() < 1e-6,
            "the position {ball:?} came back as {back:?}"
        );
    }
}
