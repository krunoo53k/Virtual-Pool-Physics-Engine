//! A deterministic, event-driven 8-ball pool physics core.
//!
//! The model follows [`docs/virtual-pool-physics.md`](../../docs/virtual-pool-physics.md):
//!
//! * the motion between events is closed form (§3), never integrated;
//! * every number is `f64` (§11.1);
//! * the transcendental functions come from the pure-Rust `libm` crate, never from
//!   the platform's libm (§11.4);
//! * the event order is a total order, so two runs agree bit for bit (§11.4);
//! * every tunable parameter lives in [`TableConfig`]; the cushion contact height is
//!   *derived* from the nose geometry rather than tuned (§5.1, §13.3).
//!
//! ```no_run
//! use pool_core::{simulate_shot, BallState, ShotInput, TableConfig};
//! use pool_core::math::Vec3;
//!
//! let cfg = TableConfig::default();
//! let mut balls = Vec::new();
//! balls.push(BallState::at_rest(&cfg, 0, Vec3::new(0.5, 0.635, cfg.ball.radius)));
//! balls.push(BallState::at_rest(&cfg, 1, Vec3::new(0.9, 0.635, cfg.ball.radius)));
//! let input = ShotInput::new(1.5, Vec3::X, balls);
//! let state = simulate_shot(input, &cfg);
//! println!("the object ball ended at x = {}", state.balls[1].position.x);
//! ```

pub mod ball_ball;
pub mod cloth;
pub mod config;
pub mod cue;
pub mod cushion;
pub mod geometry;
pub mod hash;
pub mod json;
pub mod math;
pub mod pocket;
pub mod quartic;
pub mod sim;
pub mod state;
pub mod trace;

pub use config::TableConfig;
pub use hash::{canonical_bytes, hash_state, hash_state_hex};
pub use math::{Vec2, Vec3};
pub use sim::{
    simulate_shot, try_simulate_shot, try_simulate_shot_traced, try_simulate_state, EventKind,
    SimEvent, Simulation,
};
pub use state::{
    BallState, CueStickState, GamePhase, GameState, MotionState, PocketState, ShotInput,
    BED_TOLERANCE,
};
pub use trace::{Segment, SegmentKind, Trace};

/// The version of the physics model, following the research document.
///
/// The number changes whenever the physics changes, so that a game can say
/// which physics it is running: the extension is a library that is loaded when
/// the game starts, and a game that is left running keeps the physics it was
/// started with, however new the library on disk is.
pub const MODEL_VERSION: &str = "1.8";
