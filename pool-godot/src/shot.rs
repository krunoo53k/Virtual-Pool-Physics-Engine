//! The shot, in plain Rust.
//!
//! The Godot classes are thin: they convert what the game says into what the
//! core takes, and what the core gives back into what the game reads. The work
//! itself is here, where it can be tested without an engine running.

use pool_core::config::TableConfig;
use pool_core::math::Vec3;
use pool_core::sim::SimError;
use pool_core::state::{BallState, GamePhase, GameState, ShotInput};
use pool_core::{cue, try_simulate_shot_traced, Trace};

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
pub fn shot_input(
    table: &TableConfig,
    cue_speed: f64,
    direction: Vec3,
    contact_offset: (f64, f64),
    positions: &[Vec3],
    numbers: &[i32],
) -> ShotInput {
    let (across, up) = crate::frame::contact_offset(contact_offset.0, contact_offset.1);
    let mut balls: Vec<BallState> = Vec::with_capacity(positions.len());
    for (index, position) in positions.iter().enumerate() {
        let number = numbers.get(index).copied().unwrap_or(index as i32);
        let number = u8::try_from(number).unwrap_or(0);
        balls.push(BallState::at_rest(table, number, *position));
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
