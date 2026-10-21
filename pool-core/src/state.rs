//! The state representation and the input model of §12.

use crate::config::TableConfig;
use crate::math::Vec3;

/// The motion state of a ball (§3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionState {
    /// `u(t) != 0`: the ball slides on the cloth.
    Sliding,
    /// `u(t) = 0`: the ball rolls without slipping.
    Rolling,
    /// The ball rotates in place (`v = 0`, `omega_z != 0`).
    Spinning,
    /// The ball is at rest.
    Stationary,
    /// The ball is off the cloth, in flight.
    Airborne,
}

/// Where a ball is in the pocket state machine (§9.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PocketState {
    /// Normal play on the bed.
    OnTable,
    /// The centre has crossed the mouth line: the ball is sliding on the shelf.
    Entering,
    /// The centre has crossed the drop line: the ball is in the cabinet.
    Pocketed,
    /// The ball ran out of speed on the shelf and hung up (§9.2).
    HungUp,
    /// The ball left the table (§10.1).
    OffTable,
}

/// The state of one ball (§12.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BallState {
    /// Position of the centre, m.
    pub position: Vec3,
    /// Linear velocity, m/s.
    pub velocity: Vec3,
    /// Angular velocity, rad/s.
    pub angular_velocity: Vec3,
    /// The motion state derived from the above.
    pub motion_state: MotionState,
    /// Mass, kg.
    pub mass: f64,
    /// Radius, m.
    pub radius: f64,
    /// Moment of inertia, kg m^2.
    pub moment_of_inertia: f64,
    /// 0 = cue ball, 1-15 = object balls.
    pub number: u8,
    /// The ball is in a pocket.
    pub is_pocketed: bool,
    /// The ball left the table.
    pub is_off_table: bool,
    /// The pocket state machine state (§9.2).
    pub pocket_state: PocketState,
}

impl BallState {
    /// A ball at rest at the given position.
    pub fn at_rest(cfg: &TableConfig, number: u8, position: Vec3) -> BallState {
        BallState {
            position,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            motion_state: MotionState::Stationary,
            mass: cfg.ball.mass,
            radius: cfg.ball.radius,
            moment_of_inertia: cfg.moment_of_inertia(),
            number,
            is_pocketed: false,
            is_off_table: false,
            pocket_state: PocketState::OnTable,
        }
    }

    /// True when the ball will not move again: at rest, pocketed or off the table.
    pub fn is_settled(&self) -> bool {
        self.is_pocketed
            || self.is_off_table
            || (self.velocity.norm_squared() == 0.0 && self.angular_velocity.norm_squared() == 0.0)
    }
}

/// The phase of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GamePhase {
    /// The player is aiming.
    Aiming,
    /// The shot is being simulated.
    Shooting,
    /// The balls have come to rest.
    Settled,
    /// The simulation aborted with an error; the state is the last consistent one.
    Faulted,
}

/// The cue stick, as seen by the renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CueStickState {
    /// Where the butt of the cue is.
    pub butt_position: Vec3,
    /// The unit direction the cue points in.
    pub direction: Vec3,
    /// The stick's elevation, radians.
    pub elevation: f64,
    /// How far the tip has been drawn back, m.
    pub tip_offset: f64,
}

impl Default for CueStickState {
    fn default() -> Self {
        CueStickState {
            butt_position: Vec3::ZERO,
            direction: Vec3::X,
            elevation: 0.0,
            tip_offset: 0.0,
        }
    }
}

/// The complete state of the game (§12.2).
#[derive(Clone, Debug, PartialEq)]
pub struct GameState {
    /// The sixteen balls: index 0 is the cue ball.
    pub balls: Vec<BallState>,
    /// The cue stick.
    pub cue_stick: CueStickState,
    /// The player whose turn it is.
    pub turn: u8,
    /// The phase of the game.
    pub phase: GamePhase,
    /// A foul was committed on this shot.
    pub foul: bool,
    /// The game is over.
    pub game_over: bool,
    /// The winner, if the game is over.
    pub winner: Option<u8>,
    /// The tip slipped off the ball (§6.3).
    pub miscue: bool,
    /// The simulated time of the shot, s.
    pub elapsed_time: f64,
}

impl GameState {
    /// A fresh game state from the initial ball positions.
    pub fn new(balls: Vec<BallState>) -> GameState {
        GameState {
            balls,
            cue_stick: CueStickState::default(),
            turn: 0,
            phase: GamePhase::Aiming,
            foul: false,
            game_over: false,
            winner: None,
            miscue: false,
            elapsed_time: 0.0,
        }
    }
}

/// The input to the physics simulation (§12.3).
#[derive(Clone, Debug, PartialEq)]
pub struct ShotInput {
    /// The speed of the cue at the moment of the strike, m/s.
    pub cue_speed: f64,
    /// The direction of the cue axis as a unit vector. The elevation for jump shots is
    /// baked into this vector, so the core never has to call `sin`/`cos` (§12.3).
    pub cue_direction: Vec3,
    /// The horizontal offset of the contact point on the cue ball, m.
    ///
    /// Positive is towards `k x cue_direction` - the same side the document calls
    /// "+y" in §6.2, where a hit on that side gives `omega_z < 0`.
    pub contact_offset_x: f64,
    /// The vertical offset of the contact point on the cue ball, m. Positive is above
    /// the centre, which gives topspin (`omega_y > 0` in §6.2's frame).
    pub contact_offset_y: f64,
    /// The state of all balls at the moment of the strike.
    pub ball_states: Vec<BallState>,
}

impl ShotInput {
    /// A shot with no spin on the cue ball.
    pub fn new(cue_speed: f64, cue_direction: Vec3, ball_states: Vec<BallState>) -> ShotInput {
        ShotInput {
            cue_speed,
            cue_direction,
            contact_offset_x: 0.0,
            contact_offset_y: 0.0,
            ball_states,
        }
    }
}
