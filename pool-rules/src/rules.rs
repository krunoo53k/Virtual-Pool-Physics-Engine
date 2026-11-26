//! The BCA eight-ball ruling table, verbatim from `EightBallRules.gd`.
//!
//! Events are ground truth. The core's advisory `Outcome.foul` is never read;
//! only `Outcome.miscue` and the phase word are. The ruling is pure: it does
//! not mutate the [`MatchState`] or the rack.

use pool_intent::outcome::{EventKind, Outcome, ShotEvent, ShotPhase};

use crate::{Judgement, MatchState, ShotParams};

/// BCA: object balls that must reach a rail on the break unless a ball drops.
pub const MIN_BREAK_RAILS: usize = 4;

/// 1 solids (1..=7), 2 stripes (9..=15), 0 for the 8 or an out-of-range number.
pub fn group_of(n: i32) -> i32 {
    if n == 8 {
        0
    } else if (1..=7).contains(&n) {
        1
    } else {
        2
    }
}

/// The event reduction shared by every branch of the ruling.
///
/// Only `ball-ball`, `cushion`, `pocket-drop` and `off-table` are read; the
/// other core words are noise here. `first` is the cue's first touch and only
/// updates while it is still negative, so object-object contacts before it do
/// not count. A rail only counts as *after* contact when a first contact has
/// already happened and its time is strictly later than the contact's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    /// The other ball's number on the cue's first contact, -1 if none.
    pub first: i32,
    /// Shot clock at that first contact.
    pub contact_t: f64,
    /// Whether any object ball reached a rail strictly after contact.
    pub rail_after: bool,
    /// Deduped object-ball numbers driven to a rail, in event order.
    pub driven: Vec<i32>,
    /// Pocketed object-ball numbers, in event order, with no dedup.
    pub pots: Vec<i32>,
    /// The cue scratched (drop or off table).
    pub scratch: bool,
    /// Pocket index of the 8's drop, -1 if it did not drop.
    pub eight_pocket: i32,
    /// Object balls driven off the table, in event order, with no dedup.
    pub off: Vec<i32>,
}

/// Reduces the shot's events to the facts the ruling table reads.
pub fn facts(events: &[ShotEvent]) -> Facts {
    let mut found = Facts {
        first: -1,
        eight_pocket: -1,
        ..Facts::default()
    };
    for event in events {
        match event.kind {
            EventKind::BallBall => {
                // Object-object contact before the cue's touch is noise.
                if found.first < 0 && (event.ball == 0 || event.other == 0) {
                    found.first = if event.ball == 0 {
                        event.other
                    } else {
                        event.ball
                    };
                    found.contact_t = event.time;
                }
            }
            EventKind::Cushion => {
                if event.ball != 0 && !found.driven.contains(&event.ball) {
                    found.driven.push(event.ball);
                }
                if found.first >= 0 && event.time > found.contact_t {
                    found.rail_after = true;
                }
            }
            EventKind::PocketDrop => {
                // A pocket-drop is the only pot: a rattle never counts.
                if event.ball == 0 {
                    found.scratch = true;
                } else {
                    found.pots.push(event.ball);
                    if event.ball == 8 {
                        found.eight_pocket = event.other;
                    }
                }
            }
            EventKind::OffTable => {
                // A cue off the bed is a scratch; object balls keep their own
                // reason.
                if event.ball == 0 {
                    found.scratch = true;
                } else {
                    found.off.push(event.ball);
                }
            }
            EventKind::Jaw
            | EventKind::Slate
            | EventKind::State
            | EventKind::PocketEnter
            | EventKind::PocketExit => {}
        }
    }
    found
}

/// BCA: a pot legalizes the break; otherwise at least four object balls must
/// reach a rail.
fn break_is_legal(found: &Facts) -> bool {
    !found.pots.is_empty() || found.driven.len() >= MIN_BREAK_RAILS
}

/// The priority chain, top wins. The returned words are the fixture contract.
fn foul_reason(
    found: &Facts,
    outcome: &Outcome,
    state: &MatchState,
    group: i32,
    on_eight: bool,
) -> String {
    if found.scratch {
        return "scratch".to_string();
    }
    if !found.off.is_empty() {
        return "ball_off_table".to_string();
    }
    if found.first >= 0 && !state.break_next {
        if state.solids_player == 0 && found.first == 8 {
            return "wrong_group_first_contact".to_string();
        }
        if state.solids_player != 0
            && group_of(found.first) != group
            && !(found.first == 8 && on_eight)
        {
            return "wrong_group_first_contact".to_string();
        }
    }
    if found.first < 0 && !state.break_next {
        return "no_contact".to_string();
    }
    if found.first >= 0 && !found.rail_after && found.pots.is_empty() && !state.break_next {
        return "no_rail_after_contact".to_string();
    }
    if state.break_next && !break_is_legal(found) {
        return "illegal_break".to_string();
    }
    if outcome.miscue {
        return "miscue".to_string();
    }
    if outcome.phase != ShotPhase::Settled {
        return "incomplete_shot".to_string();
    }
    String::new()
}

/// The 8 ball: off the bed loses, on the break it re-racks, else the call
/// decides.
fn eight_ruling(
    mut ruling: Judgement,
    found: &Facts,
    params: &ShotParams,
    state: &MatchState,
    on_eight: bool,
) -> Judgement {
    let shooter = state.player;
    let other = 3 - shooter;
    if found.off.contains(&8) {
        ruling.game_over = true;
        ruling.winner = other;
        ruling.next_player = 0;
    } else if state.break_next {
        ruling.re_rack = true;
        ruling.ball_in_hand = !ruling.foul_reason.is_empty();
        ruling.next_player = if ruling.foul_reason.is_empty() {
            shooter
        } else {
            other
        };
    } else {
        ruling.game_over = true;
        let called_wins = on_eight
            && !found.scratch
            && params.called_pocket >= 0
            && found.eight_pocket == params.called_pocket
            && ruling.foul_reason.is_empty();
        ruling.winner = if called_wins { shooter } else { other };
        ruling.next_player = 0;
    }
    ruling
}

fn rule(
    outcome: &Outcome,
    params: &ShotParams,
    events: &[ShotEvent],
    state: &MatchState,
) -> Judgement {
    let found = facts(events);
    let shooter = state.player;
    let other = 3 - shooter;
    // The shooter's legal group BEFORE the shot: 1 solids, 2 stripes, 0 open.
    let mut group = 0;
    if state.solids_player == shooter {
        group = 1;
    } else if state.solids_player != 0 {
        group = 2;
    }
    // "On the 8" reads the pre-shot ball list exactly as BCA has it.
    let mut on_eight = group != 0 && state.remaining.contains(&8);
    for &number in &state.remaining {
        if number != 8 && group_of(number) == group {
            on_eight = false;
        }
    }

    let reason = foul_reason(&found, outcome, state, group, on_eight);
    let mut ruling = Judgement {
        foul_reason: reason,
        ..Judgement::default()
    };
    if found.pots.contains(&8) || found.off.contains(&8) {
        return eight_ruling(ruling, &found, params, state, on_eight);
    }
    if state.break_next && !break_is_legal(&found) {
        // An illegal break re-racks; a foul on a legal break only grants
        // ball-in-hand and leaves the table where it lies.
        ruling.re_rack = true;
    }
    ruling.ball_in_hand = !ruling.foul_reason.is_empty();
    if state.solids_player == 0
        && !state.break_next
        && ruling.foul_reason.is_empty()
        && !found.pots.is_empty()
    {
        // Open table: the first ball potted names the shooter's group.
        ruling.solids_player = if group_of(found.pots[0]) == 1 {
            shooter
        } else {
            other
        };
    }
    let mut keep = ruling.foul_reason.is_empty() && !found.pots.is_empty();
    if keep && !state.break_next && state.solids_player != 0 {
        keep = false;
        for &number in &found.pots {
            if group_of(number) == group {
                keep = true;
            }
        }
    }
    // An own-group pot keeps the shooter; anything else passes the turn.
    ruling.next_player = if keep { shooter } else { other };
    ruling
}

/// Decide one shot from its outcome, params, events and the *pre-shot* match
/// state. The ruling never mutates anything.
pub fn judge(
    outcome: &Outcome,
    params: &ShotParams,
    events: &[ShotEvent],
    state: &MatchState,
) -> Judgement {
    rule(outcome, params, events, state)
}

/// [`judge`] plus the announcement words, in fixture order. Returning the
/// words keeps the pure ruling separate from any signal bus.
pub fn judge_and_announce(
    outcome: &Outcome,
    params: &ShotParams,
    events: &[ShotEvent],
    state: &MatchState,
) -> (Judgement, Vec<String>) {
    let ruling = rule(outcome, params, events, state);
    let announcements = ruling.announcements();
    (ruling, announcements)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: EventKind, ball: i32, other: i32, time: f64) -> ShotEvent {
        ShotEvent {
            time,
            kind,
            ball,
            other,
            detail: 0,
        }
    }

    #[test]
    fn first_contact_is_only_the_cues_first_touch() {
        let found = facts(&[
            event(EventKind::BallBall, 3, 9, 0.1),
            event(EventKind::BallBall, 0, 4, 0.3),
            event(EventKind::BallBall, 0, 5, 0.4),
        ]);
        assert_eq!(found.first, 4);
        assert_eq!(found.contact_t, 0.3);
    }

    #[test]
    fn rail_after_contact_is_strict_and_needs_a_contact() {
        let early = facts(&[
            event(EventKind::Cushion, 3, 1, 0.2),
            event(EventKind::BallBall, 0, 4, 0.4),
        ]);
        assert!(!early.rail_after);
        let equal = facts(&[
            event(EventKind::BallBall, 0, 4, 0.4),
            event(EventKind::Cushion, 4, 1, 0.4),
        ]);
        assert!(!equal.rail_after);
        let late = facts(&[
            event(EventKind::BallBall, 0, 4, 0.4),
            event(EventKind::Cushion, 4, 1, 0.5),
        ]);
        assert!(late.rail_after);
    }

    #[test]
    fn driven_is_deduped_but_pots_and_off_are_not() {
        let found = facts(&[
            event(EventKind::Cushion, 3, 1, 0.2),
            event(EventKind::Cushion, 3, 2, 0.3),
            event(EventKind::Cushion, 0, 1, 0.4),
            event(EventKind::PocketDrop, 5, 0, 0.5),
            event(EventKind::PocketDrop, 5, 1, 0.6),
            event(EventKind::OffTable, 7, -1, 0.7),
            event(EventKind::OffTable, 7, -1, 0.8),
            event(EventKind::PocketDrop, 0, 3, 0.9),
            event(EventKind::OffTable, 0, -1, 1.0),
        ]);
        assert_eq!(found.driven, vec![3]);
        assert_eq!(found.pots, vec![5, 5]);
        assert_eq!(found.off, vec![7, 7]);
        assert!(found.scratch);
        assert_eq!(found.eight_pocket, -1);
    }

    #[test]
    fn eight_pocket_remembers_the_last_drop() {
        let found = facts(&[
            event(EventKind::PocketDrop, 8, 0, 0.5),
            event(EventKind::PocketDrop, 8, 3, 0.6),
        ]);
        assert_eq!(found.pots, vec![8, 8]);
        assert_eq!(found.eight_pocket, 3);
    }

    #[test]
    fn group_words() {
        assert_eq!(group_of(0), 2);
        assert_eq!(group_of(1), 1);
        assert_eq!(group_of(7), 1);
        assert_eq!(group_of(8), 0);
        assert_eq!(group_of(9), 2);
        assert_eq!(group_of(15), 2);
    }

    #[test]
    fn game_over_suppresses_the_turn_announcement() {
        let ruling = Judgement {
            foul_reason: "scratch".to_string(),
            next_player: 0,
            ball_in_hand: false,
            re_rack: false,
            solids_player: 2,
            game_over: true,
            winner: 1,
        };
        assert_eq!(
            ruling.announcements(),
            vec![
                "foul_committed:scratch".to_string(),
                "groups_assigned:2".to_string(),
                "game_over:1".to_string(),
            ]
        );
    }
}
