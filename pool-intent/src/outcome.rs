//! The output side of a shot, shared by the bridge and the rules port.
//!
//! The core logs events against **indices** into the rack; the rules read ball
//! **numbers**. That conversion is done here, once, so the Godot bridge, the
//! server and the client's preview cannot each invent a slightly different
//! mapping.

use pool_core::hash::Hasher;
use pool_core::sim::{EventKind as CoreEventKind, SimEvent};
use pool_core::state::{GamePhase, GameState};

/// The canonical digest of a shot's complete event sequence.
///
/// The final-state hash is final-state only; the rules consume events. This
/// digest is what `COMMIT` carries and what the canary hashes, so a build that
/// resolves a different rail or pocket on the same final position is caught.
/// The bytes, in event order: the kind word, the event time's raw f64 bits,
/// the ball number, the other ball/rail/pocket number and the detail byte.
pub fn events_digest(events: &[SimEvent], numbers: &[i32]) -> String {
    let mut hasher = Hasher::new();
    for event in events {
        let kind = EventKind::from_core(event.kind);
        hasher.write_bytes(kind.as_str().as_bytes());
        hasher.write_f64(event.time);
        let ball = numbers.get(event.a as usize).copied().unwrap_or(-1);
        let other = if kind == EventKind::BallBall {
            numbers.get(event.b as usize).copied().unwrap_or(-1)
        } else {
            i32::from(event.b)
        };
        hasher.write_u8(ball as u8);
        hasher.write_u8(other as u8);
        hasher.write_u8(event.detail);
    }
    format!("{:016x}", hasher.finish())
}

/// The closed word set of a shot event, as the rules see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    BallBall,
    Cushion,
    Jaw,
    Slate,
    State,
    PocketEnter,
    PocketDrop,
    PocketExit,
    OffTable,
}

impl EventKind {
    /// The core's own event word.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BallBall => "ball-ball",
            Self::Cushion => "cushion",
            Self::Jaw => "jaw",
            Self::Slate => "slate",
            Self::State => "state",
            Self::PocketEnter => "pocket-enter",
            Self::PocketDrop => "pocket-drop",
            Self::PocketExit => "pocket-exit",
            Self::OffTable => "off-table",
        }
    }

    /// Parses the core's event word. Unknown words are malformed, not guessed.
    ///
    /// This is an infallible-word lookup returning `Option`, not the `FromStr`
    /// trait (which would force a `Result` error type on a closed enum).
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(word: &str) -> Option<Self> {
        Some(match word {
            "ball-ball" => Self::BallBall,
            "cushion" => Self::Cushion,
            "jaw" => Self::Jaw,
            "slate" => Self::Slate,
            "state" => Self::State,
            "pocket-enter" => Self::PocketEnter,
            "pocket-drop" => Self::PocketDrop,
            "pocket-exit" => Self::PocketExit,
            "off-table" => Self::OffTable,
            _ => return None,
        })
    }

    fn from_core(kind: CoreEventKind) -> Self {
        match kind {
            CoreEventKind::BallBall => Self::BallBall,
            CoreEventKind::Cushion => Self::Cushion,
            CoreEventKind::Jaw => Self::Jaw,
            CoreEventKind::SlateBounce => Self::Slate,
            CoreEventKind::StateChange => Self::State,
            CoreEventKind::PocketEnter => Self::PocketEnter,
            CoreEventKind::PocketDrop => Self::PocketDrop,
            CoreEventKind::PocketExit => Self::PocketExit,
            CoreEventKind::OffTable => Self::OffTable,
        }
    }
}

/// A typed event in the rules' frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotEvent {
    /// The event time, s.
    pub time: f64,
    pub kind: EventKind,
    /// The ball **number** at the core's `a` index (-1 if out of range).
    pub ball: i32,
    /// For ball-ball, the other ball's number; for other kinds the rail or
    /// pocket index, as the core logged it.
    pub other: i32,
    /// Extra detail from the core (the rail regime).
    pub detail: u8,
}

/// The phase the shot reached. The rules only distinguish "settled" from
/// anything else, but the other words are kept for the wire and the canary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotPhase {
    Settled,
    Settling,
    Faulted,
}

impl ShotPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Settled => "settled",
            Self::Settling => "settling",
            Self::Faulted => "faulted",
        }
    }

    /// Parses a phase word. `solved` is kept as a legacy alias for `settled`;
    /// unknown words are malformed, not guessed.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(word: &str) -> Option<Self> {
        Some(match word {
            "settled" | "solved" => Self::Settled,
            "settling" => Self::Settling,
            "faulted" => Self::Faulted,
            _ => return None,
        })
    }
}

/// The outcome of a shot, in ball numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub elapsed_time: f64,
    pub phase: ShotPhase,
    pub foul: bool,
    pub miscue: bool,
    /// Ball numbers that dropped into a pocket, in event order.
    pub pocketed: Vec<u8>,
    /// Ball numbers that left the table, in event order.
    pub off_table: Vec<u8>,
    /// The core's fault message, empty when the shot ran.
    pub error: String,
}

impl Default for Outcome {
    fn default() -> Self {
        Self {
            elapsed_time: 0.0,
            phase: ShotPhase::Settled,
            foul: false,
            miscue: false,
            pocketed: Vec::new(),
            off_table: Vec::new(),
            error: String::new(),
        }
    }
}

/// Builds the typed events and outcome from a core trace.
///
/// `numbers` is the rack's ball numbers index by index with the balls the shot
/// simulated; `events` and `state` come from the same `try_simulate_shot_traced`
/// call, and `error` is that call's fault message (empty on success).
pub fn outcome_from_trace(
    numbers: &[i32],
    events: &[SimEvent],
    state: &GameState,
    error: &str,
) -> (Vec<ShotEvent>, Outcome) {
    let number = |index: u8| -> i32 { numbers.get(index as usize).copied().unwrap_or(-1) };
    let mut typed = Vec::with_capacity(events.len());
    let mut pocketed = Vec::new();
    let mut off_table = Vec::new();
    for event in events {
        let kind = EventKind::from_core(event.kind);
        let ball = number(event.a);
        let other = if kind == EventKind::BallBall {
            number(event.b)
        } else {
            event.b as i32
        };
        typed.push(ShotEvent {
            time: event.time,
            kind,
            ball,
            other,
            detail: event.detail,
        });
        if matches!(kind, EventKind::PocketDrop | EventKind::OffTable) && ball >= 0 {
            if kind == EventKind::PocketDrop {
                pocketed.push(ball as u8);
            } else {
                off_table.push(ball as u8);
            }
        }
    }
    let phase = match state.phase {
        GamePhase::Settled => ShotPhase::Settled,
        GamePhase::Settling => ShotPhase::Settling,
        _ => ShotPhase::Faulted,
    };
    let outcome = Outcome {
        elapsed_time: state.elapsed_time,
        phase,
        foul: state.foul,
        miscue: state.miscue,
        pocketed,
        off_table,
        error: error.to_string(),
    };
    (typed, outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_round_trip() {
        for kind in [
            EventKind::BallBall,
            EventKind::Cushion,
            EventKind::Jaw,
            EventKind::Slate,
            EventKind::State,
            EventKind::PocketEnter,
            EventKind::PocketDrop,
            EventKind::PocketExit,
            EventKind::OffTable,
        ] {
            assert_eq!(EventKind::from_str(kind.as_str()), Some(kind));
        }
        assert_eq!(EventKind::from_str("unknown"), None);
        for phase in [ShotPhase::Settled, ShotPhase::Settling, ShotPhase::Faulted] {
            assert_eq!(ShotPhase::from_str(phase.as_str()), Some(phase));
        }
        assert_eq!(ShotPhase::from_str("solved"), Some(ShotPhase::Settled));
        assert_eq!(ShotPhase::from_str("nonsense"), None);
    }
}
