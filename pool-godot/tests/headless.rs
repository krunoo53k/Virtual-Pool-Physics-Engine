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
    // A fresh rack: no ball carries an orientation into the break, which is
    // what the game hands `PoolCore.simulate` too.
    let input = shot::shot_input(
        table,
        8.0,
        direction,
        (0.0, 0.0),
        &core_positions,
        &numbers,
        &[],
    );
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
fn the_carry_does_not_move_the_hash() {
    // The orientation a ball carries into a shot is §11.2's hand-off seen from
    // the input side: it says where the renderer had already turned the ball,
    // and nothing in the physics reads it. It is not part of the canonical
    // bytes of a state, so the same shot whose balls were put down straight,
    // and the same shot whose balls were left turned, end in the same state
    // and hash the same pinned value.
    let table = TableConfig::default();
    let positions: Vec<Vector3> = rack(&table).iter().map(|p| frame::to_godot(*p)).collect();
    let core_positions: Vec<Vec3> = positions.iter().map(|p| frame::to_core(*p)).collect();
    let numbers: Vec<i32> = (0..core_positions.len() as i32).collect();
    let direction = frame::to_core(frame::to_godot(Vec3::Y));
    // The whole rack is put on the table already turned: every ball by an
    // eighth of a turn about the vertical, and more for each ball along.
    let turned: Vec<Vec3> = core_positions
        .iter()
        .enumerate()
        .map(|(index, _)| Vec3::new(0.0, 0.0, 0.125 * index as f64))
        .collect();
    let input = shot::shot_input(
        &table,
        8.0,
        direction,
        (0.0, 0.0),
        &core_positions,
        &numbers,
        &turned,
    );
    let (state, _) = shot::simulate(&table, &input).expect("the shot runs");
    let hash = pool_core::hash_state_hex(&state);
    assert_eq!(hash, HASH, "the carry moved the shot's hash: {}", hash);
}

/// The hash of the same break with the cue raised, pinned.
///
/// `godot-project/scripts/input_test.gd` raises the cue the same way through the
/// game and reads the ball in the air. A cue whose height only moves the
/// drawing, and never reaches the stroke, would leave the game reading an
/// ordinary level stroke here while the game still shows a raised cue.
const RAISED_HASH: &str = "1087065d38976a3e";

/// The break with the cue raised 12 degrees above the level, tip below the
/// centre: the stroke the game makes when it holds the cue and drags up.
///
/// The raise is baked into the direction rather than passed as an angle, the
/// way §12.3 wants the trigonometry kept, and the direction goes through Godot's
/// frame like every other number the game sends. The core's `z` is the height,
/// and a cue raised at the butt points its axis DOWN at the ball, so the
/// direction loses `sin` of the raise. §7.1 says what that does to the stroke:
/// the elevated cue drives the cue ball into the slate beside the object ball
/// rather than along the bed, and §7.2's rebound of the slate is what launches
/// it. The tip below the centre is what makes a cue lean rather than glide, and
/// §12.3's cue-end mass is what turns that lean into the spin that holds the
/// hop; the offset is in metres, four tenths of a ball radius under the centre.
fn the_raised_shot(table: &TableConfig) -> (pool_core::state::GameState, pool_core::Trace) {
    // The positions go through Godot's frame, because that is the way the game
    // gives them: a `Vector3` is an `f32` triple.
    let positions: Vec<Vector3> = rack(table).iter().map(|p| frame::to_godot(*p)).collect();
    let core_positions: Vec<Vec3> = positions.iter().map(|p| frame::to_core(*p)).collect();
    let numbers: Vec<i32> = (0..core_positions.len() as i32).collect();
    let elevation = 12.0f64.to_radians();
    let direction = frame::to_core(frame::to_godot(Vec3::new(
        0.0,
        elevation.cos(),
        -elevation.sin(),
    )));
    let input = shot::shot_input(
        table,
        8.0,
        direction,
        (0.0, -0.4 * table.ball.radius),
        &core_positions,
        &numbers,
        &[],
    );
    shot::simulate(table, &input).expect("the raised shot runs")
}

#[test]
fn the_elevation_reaches_the_stroke() {
    let table = TableConfig::default();
    let (state, trace) = the_raised_shot(&table);
    let hash = pool_core::hash_state_hex(&state);
    assert_eq!(hash, RAISED_HASH, "the raised shot's hash moved: {}", hash);
    // The same rack, the same speed, the same aim line: only the cue's height
    // differs, so the two strokes have to come out different. A raise that made
    // the level shot is a raise that never left the drawing.
    assert_ne!(
        hash, HASH,
        "the raised cue made the level shot: the cue's height never reached the stroke",
    );

    // The hop. The state at the end of the shot cannot say the ball flew: the
    // slate's rebound brings it back to the cloth and the cue ball rests where
    // it landed, so `airborne` is a state inside the shot rather than one it
    // ends in. The hop is read on the path instead, at the middle of the
    // airborne stretch the path itself names, which is the same analytic
    // sampling the renderer plays the path with.
    let hop = match trace
        .segments(0)
        .iter()
        .find(|segment| segment.kind == pool_core::trace::SegmentKind::Airborne)
    {
        Some(segment) => (segment.t0 + segment.t1) * 0.5,
        // No airborne stretch at all: the checks below say so.
        None => 0.0,
    };
    assert!(
        hop > 0.0,
        "the raised cue made no hop: the cue ball never left the cloth"
    );
    let cue_ball = trace.sample(hop, &table);
    assert_eq!(
        cue_ball[0].motion_state,
        pool_core::MotionState::Airborne,
        "the cue ball is not in the air at the middle of the hop the path names",
    );
    let cue_ball_height = cue_ball[0].position.z;
    assert!(
        cue_ball_height > table.ball.radius,
        "the cue ball is on the cloth at {cue_ball_height:?}, which is not a hop",
    );
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
