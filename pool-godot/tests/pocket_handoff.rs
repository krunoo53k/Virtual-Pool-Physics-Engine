//! A ball that starts a shot already inside a pocket mouth.
//!
//! The game hands each shot to the bridge as positions, numbers and rotations.
//! A ball that came to rest just past a pocket mouth line is handed over as a
//! fresh `BallState`, which used to leave it `OnTable`. The pocket state machine
//! only announces a pocket with a mouth crossing from the table side, so such a
//! ball could never drop: a strike sent it through the shelf and the off-table
//! backstop took it out of play instead. These tests pin the hand-off.

use godot::builtin::Vector3;
use pool_core::config::TableConfig;
use pool_core::geometry::TableGeometry;
use pool_core::math::Vec3;
use pool_core::state::PocketState;
use pool_godot::{frame, shot};

/// A cue ball 20 cm behind an object ball sitting 6.5 mm past the mouth line
/// of the bottom-left corner pocket, on its centreline. The object ball looks
/// over the hole but has not reached the drop line, which is 44.45 mm back.
fn mouth_shot(table: &TableConfig) -> pool_core::state::GameState {
    let pocket = &TableGeometry::new(table).pockets[0];
    let object = pocket.mouth_centre + pocket.centreline * 0.0065;
    let cue = object - pocket.centreline * 0.2;
    let core_positions = [
        Vec3::new(cue.x, cue.y, table.ball.radius),
        Vec3::new(object.x, object.y, table.ball.radius),
    ];
    // The positions and the direction go through Godot's frame, as they do
    // when the game asks for a shot.
    let positions: Vec<Vector3> = core_positions.iter().map(|p| frame::to_godot(*p)).collect();
    let positions: Vec<Vec3> = positions.iter().map(|p| frame::to_core(*p)).collect();
    let direction = frame::to_core(frame::to_godot(Vec3::new(
        pocket.centreline.x,
        pocket.centreline.y,
        0.0,
    )));
    let input = shot::shot_input(table, 3.0, direction, (0.0, 0.0), &positions, &[0, 1], &[]);
    assert_eq!(
        input.ball_states[1].pocket_state,
        PocketState::Entering,
        "a ball inside the mouth must start on the shelf"
    );
    shot::simulate(table, &input).expect("the shot runs").0
}

#[test]
fn a_ball_already_in_the_mouth_drops_instead_of_leaving_the_table() {
    let table = TableConfig::default();
    let state = mouth_shot(&table);
    let ball = &state.balls[1];
    assert!(
        ball.is_pocketed,
        "the ball should drop: phase {:?}, pocket state {:?}",
        state.phase, ball.pocket_state
    );
    assert!(
        !ball.is_off_table,
        "a ball over the hole is not off the table"
    );
}

#[test]
fn a_ball_just_outside_the_mouth_is_still_an_ordinary_table_ball() {
    let table = TableConfig::default();
    let pocket = &TableGeometry::new(&table).pockets[0];
    // The same test shot, with the object ball 6.5 mm on the table side of the
    // mouth line. It must still get its pocket-enter from the crossing.
    let object = pocket.mouth_centre - pocket.centreline * 0.0065;
    let cue = object - pocket.centreline * 0.2;
    let core_positions = [
        Vec3::new(cue.x, cue.y, table.ball.radius),
        Vec3::new(object.x, object.y, table.ball.radius),
    ];
    let positions: Vec<Vector3> = core_positions.iter().map(|p| frame::to_godot(*p)).collect();
    let positions: Vec<Vec3> = positions.iter().map(|p| frame::to_core(*p)).collect();
    let direction = frame::to_core(frame::to_godot(Vec3::new(
        pocket.centreline.x,
        pocket.centreline.y,
        0.0,
    )));
    let input = shot::shot_input(&table, 3.0, direction, (0.0, 0.0), &positions, &[0, 1], &[]);
    assert_eq!(
        input.ball_states[1].pocket_state,
        PocketState::OnTable,
        "a ball outside the mouth must start on the table"
    );
    let (state, trace) = shot::simulate(&table, &input).expect("the shot runs");
    assert!(
        state.balls[1].is_pocketed,
        "the crossing should still drop it"
    );
    assert!(
        trace
            .events
            .iter()
            .any(|event| event.kind == pool_core::EventKind::PocketEnter && event.a == 1),
        "the crossing must announce pocket-enter"
    );
}

// The positions below are built in the core's frame directly, not round-tripped
// through Godot: `Vector3` is f32, so a point exactly on a boundary would land
// on one side or the other and the test would not pin the boundary.
#[test]
fn a_ball_exactly_on_the_mouth_line_starts_on_the_shelf_and_drops() {
    let table = TableConfig::default();
    let pocket = &TableGeometry::new(&table).pockets[0];
    let object = pocket.mouth_centre;
    let cue = object - pocket.centreline * 0.2;
    let positions = [
        Vec3::new(cue.x, cue.y, table.ball.radius),
        Vec3::new(object.x, object.y, table.ball.radius),
    ];
    let input = shot::shot_input(
        &table,
        3.0,
        Vec3::new(pocket.centreline.x, pocket.centreline.y, 0.0),
        (0.0, 0.0),
        &positions,
        &[0, 1],
        &[],
    );
    assert_eq!(
        input.ball_states[1].pocket_state,
        PocketState::Entering,
        "a ball on the mouth line cannot fire a pocket-enter crossing"
    );
    let state = shot::simulate(&table, &input).expect("the shot runs").0;
    assert!(state.balls[1].is_pocketed, "the ball should drop");
}

#[test]
fn a_ball_already_past_the_drop_line_stays_pocketed() {
    let table = TableConfig::default();
    let pocket = &TableGeometry::new(&table).pockets[0];
    // Both the line itself and a point beyond it belong to the cabinet: a
    // pocketed ball's hand-off position sits at the drop line.
    for extra in [0.0, 0.001] {
        let plan = pocket.mouth_centre + pocket.centreline * (pocket.shelf_depth + extra);
        let positions = [Vec3::new(plan.x, plan.y, table.ball.radius)];
        let input = shot::shot_input(
            &table,
            3.0,
            Vec3::new(pocket.centreline.x, pocket.centreline.y, 0.0),
            (0.0, 0.0),
            &positions,
            &[0],
            &[],
        );
        let ball = &input.ball_states[0];
        assert_eq!(
            ball.pocket_state,
            PocketState::Pocketed,
            "depth past the drop line by {extra}"
        );
        assert!(ball.is_pocketed, "a ball in the cabinet is out of play");
    }
}
