//! Shared shot intent for the Godot bridge, the server and the client preview.
//!
//! `pool-godot` is a `cdylib` and cannot be depended on by another crate, so
//! the input conversion, the tuning bounds and the output mapping live here as
//! a plain `rlib`. Both the binding and the server call the same functions on
//! the same flat `f64` numbers; a client preview can use them without Godot at
//! all.
//!
//! * [`direction`] — degrees to a unit direction, and the contact sign flip;
//! * [`input`] — rack hand-off and the inclusive `CueTuning` bounds;
//! * [`outcome`] — typed events and outcome from a trace.

pub mod canary;
pub mod direction;
pub mod input;
pub mod outcome;

pub use canary::{canary_hex, canary_of};
pub use direction::{contact_offset, cue_direction, impact_parameter, normalize_aim_deg};
pub use input::{shot_input, CueTuning};
pub use outcome::{events_digest, outcome_from_trace, Outcome, ShotEvent};
