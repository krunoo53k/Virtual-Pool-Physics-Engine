//! BCA eight-ball rules: judgement and state/rack folding.
//!
//! This crate is the Rust port of the client's `EightBallRules`, `MatchJudge`,
//! `RulingApply` and `RackModel`. It is a plain `rlib`, so the server, the
//! Godot bridge's shared helper crate and the client can all call the very
//! same table over the very same event words. It never simulates anything:
//! events are ground truth, `Outcome.foul` is advisory, and only
//! `Outcome.miscue` and the phase word are read.
//!
//! * [`judge`] / [`judge_and_announce`] — the ruling table;
//! * [`Judgement::announcements`] — the stable `name:payload` signal words;
//! * [`apply_ruling`] — folding a ruling into a [`MatchState`];
//! * [`RackState`] — folding a shot's final frame into the rack.
//!
//! The canonical fixtures live in this repository at
//! `tests/fixtures/rules/eight_ball_fixtures.json` and are consumed by the
//! client's parity suite, so neither side can claim the other's copy is stale.

pub mod apply;
pub mod judgement;
pub mod rules;
pub mod state;

pub use apply::{
    apply_ruling, full_rack, head_spot, respot_cue, RackState, TableLayout, HEAD_SPOT,
};
pub use judgement::Judgement;
pub use rules::{facts, group_of, judge, judge_and_announce, Facts, MIN_BREAK_RAILS};
pub use state::{MatchState, ShotParams};

/// Version of this rules table, separate from the physics model version.
pub const RULES_VERSION: &str = "1.0";
