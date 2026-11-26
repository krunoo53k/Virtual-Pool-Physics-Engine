//! The Godot 4 binding of the pool physics core.
//!
//! The core is the only physics in the game: Godot never simulates the balls.
//! The extension hands the game the path of a shot and the state it ends in,
//! and the game plays the path back.
//!
//! * [`frame`] is the frame of the game: the core's frame, Godot's frame, and
//!   the mapping between them.
//! * [`classes::PoolCore`] is the entry point: the table, the aim preview and
//!   the shot.
//! * [`classes::PoolShot`] is a resolved shot: the path to play back and the
//!   state it ends in.
//!
//! The core is the single source of truth: the aim preview comes from the same
//! strike the shot uses, the table the player sees is built from the same
//! geometry the balls roll on, and the shot comes back as the same path the
//! simulation took.

pub mod bridge;
pub mod bridge_json;
pub mod classes;
pub mod frame;
pub mod json;
pub mod shot;

use godot::prelude::*;

/// The extension itself.
struct PoolExtension;

#[gdextension]
unsafe impl ExtensionLibrary for PoolExtension {}
