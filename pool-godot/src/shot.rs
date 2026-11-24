//! The shot, in plain Rust.
//!
//! The Godot classes are thin: they convert what the game says into what the
//! core takes, and what the core gives back into what the game reads. The work
//! itself is here, where it can be tested without an engine running.

use pool_core::config::TableConfig;
use pool_core::geometry::TableGeometry;
use pool_core::math::{Vec2, Vec3};
use pool_core::sim::SimError;
use pool_core::state::{BallState, GamePhase, GameState, PocketState, ShotInput};
use pool_core::{cue, try_simulate_shot_traced, Trace};

/// A pocketed ball's hand-off position sits at the drop line. Positions cross
/// the bridge as f32, so the depth recovered there is only good to a few
/// nanometres; that much slack counts as the line itself.
const DROP_LINE_TOLERANCE: f64 = 1e-6;

/// The strike: the cue ball's state after the tip hits it.
pub struct Strike {
    /// The velocity of the cue ball, m/s.
    pub velocity: Vec3,
    /// The angular velocity of the cue ball, rad/s.
    pub angular_velocity: Vec3,
    /// The squirt angle, radians. Always positive; the direction is in the
    /// sign.
    pub squirt: f64,
    /// `-1` when the ball squirts towards `-u`, `+1` otherwise.
    pub squirt_sign: f64,
    /// The distance from the cue's line of travel to the ball's centre, m.
    pub impact_parameter: f64,
    /// True when the tip slipped off the ball.
    pub miscue: bool,
}

/// The shot input for a stroke.
///
/// `direction` is the direction of the cue in the core's frame: from the cue
/// ball towards the target. `contact_offset` is the tip on the cue ball's
/// face: the distance to the shooter's **right** and the distance **above** the
/// centre, both in metres.
///
/// `rotations` is the orientation every ball carries into the shot, in the
/// core's frame as rotation vectors - the axis of the orientation times the
/// angle in radians - index by index with `positions`. This is the hand-off of
/// §11.2 seen from the input side: what the renderer had already turned the
/// balls to when the shot starts. The path of the shot then counts their
/// rotation from that orientation, and a ball that is still lying where the
/// last shot left it keeps its phase; a short list leaves the balls it does
/// not name at identity, which is what a ball that was put on the table does.
///
/// The hand-off carries only positions, numbers and rotations, so a ball that
/// ended the last shot on a pocket shelf arrives here as a bare position. Its
/// pocket state is recovered from the geometry: past the mouth line it starts
/// `Entering`, past the drop line `Pocketed`, so the drop logic applies from
/// frame zero (§9.2).
pub fn shot_input(
    table: &TableConfig,
    cue_speed: f64,
    direction: Vec3,
    contact_offset: (f64, f64),
    positions: &[Vec3],
    numbers: &[i32],
    rotations: &[Vec3],
) -> ShotInput {
    let (across, up) = crate::frame::contact_offset(contact_offset.0, contact_offset.1);
    let geometry = TableGeometry::new(table);
    let mut balls: Vec<BallState> = Vec::with_capacity(positions.len());
    for (index, position) in positions.iter().enumerate() {
        let number = numbers.get(index).copied().unwrap_or(index as i32);
        let number = u8::try_from(number).unwrap_or(0);
        let mut ball = BallState::at_rest(table, number, *position);
        // The pocket state machine only announces pocket-enter on a crossing
        // from the table side (§9.2), so a ball handed over with its centre
        // already past the mouth line would stay OnTable, never get a drop
        // check and be taken out by the off-table backstop instead (§10.1).
        // Recover the state from the position. The mouth line itself counts as
        // in: a ball placed exactly on it cannot fire a crossing either.
        let plan = Vec2::new(ball.position.x, ball.position.y);
        for pocket in &geometry.pockets {
            if pocket.lateral(plan).abs() > pocket.mouth_width / 2.0 {
                continue;
            }
            let depth = pocket.mouth_distance(plan);
            if depth >= pocket.shelf_depth - DROP_LINE_TOLERANCE {
                ball.pocket_state = PocketState::Pocketed;
                ball.is_pocketed = true;
            } else if depth >= 0.0 {
                ball.pocket_state = PocketState::Entering;
            } else {
                continue;
            }
            break;
        }
        // The orientation the ball carries in: it is not physics state, and the
        // shot's own numbers do not depend on it - only the path's rotations do.
        ball.rotation = rotations.get(index).copied().unwrap_or(Vec3::ZERO);
        balls.push(ball);
    }
    let mut input = ShotInput::new(cue_speed, direction, balls);
    input.contact_offset_x = across;
    input.contact_offset_y = up;
    input
}

/// The strike the shot would make.
///
/// This is the same call the simulation makes, so the aim preview cannot
/// promise a line the shot does not take.
pub fn strike(table: &TableConfig, input: &ShotInput) -> Strike {
    let strike = cue::strike(input, table);
    Strike {
        velocity: strike.velocity,
        angular_velocity: strike.angular_velocity,
        squirt: strike.squirt,
        squirt_sign: strike.squirt_sign,
        impact_parameter: strike.impact_parameter,
        miscue: strike.miscue,
    }
}

/// The shot: the state it ends in and the path it took.
pub fn simulate(table: &TableConfig, input: &ShotInput) -> Result<(GameState, Trace), SimError> {
    try_simulate_shot_traced(input, table)
}

/// The state of a shot that faulted.
///
/// The core never panics: a fault is a state with a phase, and the detail
/// comes back with it.
pub fn faulted_state(input: &ShotInput) -> GameState {
    let mut state = GameState::new(input.ball_states.clone());
    state.phase = GamePhase::Faulted;
    state.foul = true;
    state
}
