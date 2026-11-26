//! Replays the canonical eight-ball fixtures exactly as the client's
//! `tools/eight_ball_fixture_test.gd` does: a fresh `MatchState` per scenario,
//! the same fold after every shot, and an assert on all seven `Judgement`
//! fields plus the exact announcement words.
//!
//! The JSON is embedded with `include_str!` rather than opened at runtime so
//! the same test binary runs under WASI in the determinism CI lane.

use std::fmt::Debug;

use pool_core::json::{parse, Value};
use pool_intent::outcome::{EventKind, Outcome, ShotEvent, ShotPhase};
use pool_rules::{apply_ruling, judge_and_announce, MatchState, ShotParams};

const FIXTURES: &str = include_str!("../../tests/fixtures/rules/eight_ball_fixtures.json");
const FIELDS: [&str; 7] = [
    "foul_reason",
    "next_player",
    "ball_in_hand",
    "re_rack",
    "solids_player",
    "game_over",
    "winner",
];

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value
        .get(key)
        .unwrap_or_else(|| panic!("the fixture object has no '{key}' field"))
}

fn integer(value: &Value, key: &str) -> i32 {
    field(value, key)
        .as_f64()
        .unwrap_or_else(|| panic!("'{key}' is not a number")) as i32
}

fn boolean(value: &Value, key: &str) -> bool {
    field(value, key)
        .as_bool()
        .unwrap_or_else(|| panic!("'{key}' is not a boolean"))
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    field(value, key)
        .as_str()
        .unwrap_or_else(|| panic!("'{key}' is not a string"))
}

fn shot_event(value: &Value) -> ShotEvent {
    let word = text(value, "kind");
    ShotEvent {
        time: field(value, "time")
            .as_f64()
            .unwrap_or_else(|| panic!("event time is not a number")),
        kind: EventKind::from_str(word).unwrap_or_else(|| panic!("unknown event kind '{word}'")),
        ball: integer(value, "ball"),
        other: integer(value, "other"),
        detail: integer(value, "detail") as u8,
    }
}

fn shot_params(shot: &Value) -> ShotParams {
    // Only the called pocket is carried into the judge, as in the client
    // fixture runner; every other field keeps its default.
    ShotParams {
        called_pocket: integer(field(shot, "params"), "called_pocket"),
        ..ShotParams::default()
    }
}

fn shot_outcome(shot: &Value) -> Outcome {
    let outcome = field(shot, "outcome");
    // The fixture's `outcome_of` is the core's detail word; the runner maps it
    // to the core's phase word before handing it to the rules.
    let phase = match text(outcome, "outcome_of") {
        "solved" => ShotPhase::Settled,
        "settling" => ShotPhase::Settling,
        _ => ShotPhase::Faulted,
    };
    Outcome {
        phase,
        miscue: boolean(outcome, "miscue"),
        foul: boolean(outcome, "foul"),
        ..Outcome::default()
    }
}

fn full_rack() -> Vec<i32> {
    (1..=15).collect()
}

fn record<T: Debug + PartialEq>(
    failures: &mut Vec<String>,
    scenario: &str,
    shot: &str,
    label: &str,
    got: T,
    want: T,
) {
    if got != want {
        failures.push(format!(
            "{scenario} / {shot}: {label} got {got:?}, want {want:?}"
        ));
    }
}

#[test]
fn canonical_eight_ball_fixtures() {
    let root = parse(FIXTURES).expect("the canonical fixture JSON parses");
    let fixtures = field(&root, "fixtures")
        .as_array()
        .expect("fixtures is an array");
    let mut failures = Vec::new();
    let mut shots = 0;

    for fixture in fixtures {
        let scenario = text(fixture, "name");
        let mut state = MatchState {
            player: 1,
            remaining: full_rack(),
            break_next: true,
            ..MatchState::default()
        };
        for shot in field(fixture, "shots")
            .as_array()
            .unwrap_or_else(|| panic!("'{scenario}' has no shots array"))
        {
            shots += 1;
            let shot_name = text(shot, "name");
            let params = shot_params(shot);
            let outcome = shot_outcome(shot);
            let events: Vec<ShotEvent> = field(shot, "events")
                .as_array()
                .unwrap_or_else(|| panic!("'{shot_name}' has no events array"))
                .iter()
                .map(shot_event)
                .collect();
            let expect = field(shot, "expect");
            let (ruling, announced) = judge_and_announce(&outcome, &params, &events, &state);

            for label in FIELDS {
                match label {
                    "foul_reason" => record(
                        &mut failures,
                        scenario,
                        shot_name,
                        label,
                        ruling.foul_reason.clone(),
                        text(expect, label).to_string(),
                    ),
                    "next_player" | "solids_player" | "winner" => record(
                        &mut failures,
                        scenario,
                        shot_name,
                        label,
                        match label {
                            "next_player" => ruling.next_player,
                            "solids_player" => ruling.solids_player,
                            _ => ruling.winner,
                        },
                        integer(expect, label),
                    ),
                    "ball_in_hand" | "re_rack" | "game_over" => record(
                        &mut failures,
                        scenario,
                        shot_name,
                        label,
                        match label {
                            "ball_in_hand" => ruling.ball_in_hand,
                            "re_rack" => ruling.re_rack,
                            _ => ruling.game_over,
                        },
                        boolean(expect, label),
                    ),
                    _ => unreachable!("the fixture field list is closed"),
                }
            }

            let want_signals: Vec<String> = field(expect, "signals")
                .as_array()
                .unwrap_or_else(|| panic!("'{shot_name}' has no signals array"))
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .unwrap_or_else(|| panic!("a signal is not a string"))
                        .to_string()
                })
                .collect();
            record(
                &mut failures,
                scenario,
                shot_name,
                "signals",
                announced,
                want_signals,
            );

            if ruling.game_over {
                break;
            }
            // The composer's own fold: the ruling onto state plus its
            // remaining-ball bookkeeping. The fixture runner removes pocketed
            // balls only; off-table balls are production's extra bookkeeping.
            let potted: Vec<i32> = events
                .iter()
                .filter(|event| event.kind == EventKind::PocketDrop && event.ball != 0)
                .map(|event| event.ball)
                .collect();
            apply_ruling(&mut state, &ruling, &potted);
        }
    }

    assert_eq!(shots, 54, "the canonical file holds 54 shots");
    assert!(
        failures.is_empty(),
        "{} ruling(s)/signal(s) said no:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
