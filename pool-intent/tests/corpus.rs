//! Replays the self-play corpus on every platform (schema `pool-corpus/2`).
//!
//! The fixture is canonical JSON written by `pool-cli corpus`: the six intent
//! scalars and the raw bits of each, the starting rack, the path's rotations,
//! the whole event list and its canonical digest, the final rotations, the
//! final-state hash and the duration. Nothing here recomputes those values -
//! the shot is rebuilt through the same intent hand-off the bridge uses and
//! every stored number must match bit for bit.
//!
//! The tail of the corpus covers states the self-play carry loop cannot reach:
//! pocket hand-off from the shelf, jaw rattles, a hung-up ball, a miscue,
//! frozen/near-touching balls and a shot faulted by a per-shot event limit.
//! Each of those is checked against the replayed state or event list, not
//! merely against a stored summary.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use pool_core::json::Value;
use pool_core::math::Vec3;
use pool_core::sim::{EventKind, SimEvent, Simulation};
use pool_core::state::{GamePhase, PocketState, ShotInput};
use pool_core::trace::Trace;
use pool_core::{hash_state_hex, simulate_shot, try_simulate_shot_traced, GameState, TableConfig};
use pool_intent::{cue_direction, shot_input};

fn corpus() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("corpus.json");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    pool_core::json::parse(&text)
        .unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display()))
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.field(key).unwrap_or_else(|error| panic!("{error}"))
}

fn number(value: &Value, key: &str) -> f64 {
    field(value, key)
        .as_f64()
        .unwrap_or_else(|| panic!("'{key}' is not a number"))
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    field(value, key)
        .as_str()
        .unwrap_or_else(|| panic!("'{key}' is not a string"))
}

fn array(value: &Value, key: &str) -> Vec<Value> {
    field(value, key)
        .as_array()
        .unwrap_or_else(|| panic!("'{key}' is not an array"))
        .to_vec()
}

fn flat_vec3(value: &Value, key: &str) -> Vec<Vec3> {
    let items = array(value, key);
    assert_eq!(
        items.len() % 3,
        0,
        "'{key}' is not a whole number of rotation/position triples"
    );
    items
        .chunks(3)
        .map(|triple| {
            let component = |index: usize| {
                triple[index]
                    .as_f64()
                    .unwrap_or_else(|| panic!("'{key}' has a non-number component"))
            };
            Vec3::new(component(0), component(1), component(2))
        })
        .collect()
}

fn ball_numbers(value: &Value, key: &str) -> Vec<i32> {
    array(value, key)
        .iter()
        .map(|item| {
            let raw = item
                .as_f64()
                .unwrap_or_else(|| panic!("'{key}' has a non-number entry"));
            let number = raw as i32;
            assert_eq!(
                raw,
                f64::from(number),
                "'{key}' has a fractional ball number"
            );
            number
        })
        .collect()
}

/// `(event word, ball number, other number)` - the rules' frame, not indices.
type EventSummary = (String, i32, i32);

fn event_numbers(event: &SimEvent, numbers: &[u8]) -> (i32, i32) {
    let number = |index: u8| -> i32 {
        numbers
            .get(index as usize)
            .copied()
            .map(i32::from)
            .unwrap_or(-1)
    };
    let other = if event.kind == EventKind::BallBall {
        number(event.b)
    } else {
        i32::from(event.b)
    };
    (number(event.a), other)
}

fn summary(event: &SimEvent, numbers: &[u8]) -> EventSummary {
    let (ball, other) = event_numbers(event, numbers);
    (
        pool_core::trace::event_kind_name(event.kind).to_string(),
        ball,
        other,
    )
}

fn stored_summary(value: &Value) -> Vec<EventSummary> {
    array(value, "events")
        .iter()
        .map(|event| {
            let raw_other = number(event, "other");
            let other = raw_other as i32;
            assert_eq!(
                raw_other,
                f64::from(other),
                "an event's other is fractional"
            );
            (
                text(event, "kind").to_string(),
                number(event, "ball") as i32,
                other,
            )
        })
        .collect()
}

// The event digest is `pool_intent::events_digest`, the same function that
// feeds the canary and the server's COMMIT.

fn pocket_name(state: PocketState) -> &'static str {
    match state {
        PocketState::OnTable => "on-table",
        PocketState::Entering => "entering",
        PocketState::Pocketed => "pocketed",
        PocketState::HungUp => "hung-up",
        PocketState::OffTable => "off-table",
    }
}

/// The traced entry point reports a guard fault as an error and drops the
/// state. The public non-traced entry point returns the faulted state; the
/// recorder is observational, so a second run with it yields the event list
/// and rotations the fixture pins.
fn simulate_record(
    input: &ShotInput,
    cfg: &TableConfig,
    expect_fault: bool,
) -> Result<(GameState, Trace), pool_core::sim::SimError> {
    match try_simulate_shot_traced(input, cfg) {
        Ok(result) => Ok(result),
        Err(error) => {
            let state = simulate_shot(input.clone(), cfg);
            if !expect_fault || state.phase != GamePhase::Faulted {
                return Err(error);
            }
            let mut simulation = Simulation::new(input, cfg);
            simulation.record();
            let _ = simulation.run(input);
            let trace = simulation.trace().cloned().unwrap_or_default();
            Ok((state, trace))
        }
    }
}

#[test]
fn the_corpus_has_every_category_and_the_carry_chain() {
    let document = corpus();
    assert_eq!(text(&document, "schema"), "pool-corpus/2");
    let shots = array(&document, "shots");
    assert_eq!(shots.len(), 81, "the corpus must hold 81 shots");

    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for shot in &shots {
        *counts.entry(text(shot, "category")).or_default() += 1;
    }
    for (category, expected) in [
        ("rails", 16),
        ("spin", 16),
        ("jumps", 12),
        ("pockets", 15),
        ("off_table", 10),
        ("carry", 3),
        ("shelf", 2),
        ("jaw", 2),
        ("hung_up", 1),
        ("miscue", 1),
        ("edge", 2),
        ("fault", 1),
    ] {
        assert_eq!(
            counts.get(category).copied(),
            Some(expected),
            "the '{category}' category is missing or has the wrong count"
        );
    }
    assert_eq!(counts.len(), 12, "an unexpected category appeared");

    let carry: Vec<&Value> = shots
        .iter()
        .filter(|shot| text(shot, "category") == "carry")
        .collect();
    for (index, shot) in carry.iter().enumerate() {
        assert_eq!(text(shot, "id"), format!("carry-0{}", index + 1));
    }
}

#[test]
fn every_shot_replays_bit_for_bit() {
    let cfg = TableConfig::default();
    let shots = array(&corpus(), "shots");
    assert_eq!(shots.len(), 81, "the corpus must hold 81 shots");

    for shot in &shots {
        let id = text(shot, "id");
        let intent = field(shot, "intent");
        let speed = number(intent, "speed");
        let aim_deg = number(intent, "level_aim_deg");
        let elevation_deg = number(intent, "elevation_deg");
        let contact_x = number(intent, "contact_x");
        let contact_y = number(intent, "contact_y");
        let called_pocket = number(intent, "called_pocket") as i32;
        assert!(
            called_pocket == -1 || (0..6).contains(&called_pocket),
            "{id}: called_pocket {called_pocket} is outside the core's pocket set"
        );

        // Every intent scalar is pinned twice: once as its decimal value and
        // once as the 16 lowercase hex digits of that value's f64 bits.
        let hex = field(shot, "intent_hex");
        for name in [
            "speed",
            "level_aim_deg",
            "elevation_deg",
            "contact_x",
            "contact_y",
            "called_pocket",
        ] {
            let digits = text(hex, name);
            assert_eq!(
                digits.len(),
                16,
                "{id}: {name} is not a 16-digit hex string"
            );
            assert!(
                digits
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "{id}: {name} hex is not lowercase"
            );
            let from_hex = u64::from_str_radix(digits, 16)
                .unwrap_or_else(|error| panic!("{id}: {name} hex: {error}"));
            assert_eq!(
                from_hex,
                number(intent, name).to_bits(),
                "{id}: {name} hex and decimal disagree on the f64 bits"
            );
        }

        let positions = flat_vec3(shot, "positions");
        let numbers = ball_numbers(shot, "numbers");
        let rotations = flat_vec3(shot, "rotations");
        assert_eq!(positions.len(), numbers.len(), "{id}: rack length mismatch");
        assert_eq!(
            rotations.len(),
            numbers.len(),
            "{id}: rotation length mismatch"
        );

        // A per-shot config override is part of the shot: the fault record
        // trips the guard with a much smaller event limit than the default.
        let mut shot_cfg = cfg.clone();
        let faulted = shot
            .get("faulted")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if let Some(limit) = shot.get("max_events") {
            let raw = limit
                .as_f64()
                .unwrap_or_else(|| panic!("{id}: max_events is not a number"));
            let limit = raw as u32;
            assert_eq!(raw, f64::from(limit), "{id}: max_events is integral");
            shot_cfg.simulation.max_events = limit;
        }

        let direction = cue_direction(aim_deg, elevation_deg);
        let input = shot_input(
            &cfg,
            speed,
            direction,
            (contact_x, contact_y),
            &positions,
            &numbers,
            &rotations,
        );
        let (state, trace) = simulate_record(&input, &shot_cfg, faulted)
            .unwrap_or_else(|error| panic!("{id} faulted unexpectedly: {}", error.message));

        let hash = hash_state_hex(&state);
        assert_eq!(
            text(shot, "final_state_hash"),
            hash,
            "{id}: final-state hash mismatch"
        );

        let duration = number(shot, "duration");
        assert_eq!(
            duration.to_bits(),
            state.elapsed_time.to_bits(),
            "{id}: duration mismatch"
        );
        if faulted {
            // The faulted state is the input rack with `GamePhase::Faulted`,
            // while the recorder still holds the path up to the guard.
            assert_eq!(state.phase, GamePhase::Faulted, "{id}: not faulted");
            assert!(state.foul, "{id}: a fault must be a foul");
            assert!(
                trace.duration > 0.0,
                "{id}: the guard tripped before the shot ran"
            );
        } else {
            assert_eq!(
                duration.to_bits(),
                trace.duration.to_bits(),
                "{id}: the trace and state disagree on duration"
            );
        }

        let expected = stored_summary(shot);
        let actual: Vec<EventSummary> = trace
            .events
            .iter()
            .map(|event| summary(event, &trace.numbers))
            .collect();
        assert_eq!(actual, expected, "{id}: event summary mismatch");

        // The digest covers the complete sequence; the summary above only
        // checks what a human reads. Equal summaries alone cannot detect a
        // wrong time or detail byte.
        assert_eq!(
            text(shot, "events_digest"),
            pool_intent::events_digest(&trace.events, &numbers),
            "{id}: events digest mismatch"
        );

        // Rotations are not in the canonical state hash: the flat core-frame
        // triples are the renderer's carry and are pinned bit for bit.
        let expected_rotation = flat_vec3(shot, "final_rotations");
        let actual_rotation = trace.sample_rotation(trace.duration);
        assert_eq!(
            expected_rotation.len(),
            actual_rotation.len(),
            "{id}: final rotation count mismatch"
        );
        for (index, (stored, replayed)) in expected_rotation
            .iter()
            .zip(actual_rotation.iter())
            .enumerate()
        {
            for component in 0..3 {
                assert_eq!(
                    stored.get(component).to_bits(),
                    replayed.get(component).to_bits(),
                    "{id}: ball {index} rotation component {component} mismatch"
                );
            }
        }

        let kinds: Vec<&str> = trace
            .events
            .iter()
            .map(|event| pool_core::trace::event_kind_name(event.kind))
            .collect();
        match text(shot, "category") {
            "shelf" => {
                let handoff_values = array(shot, "handoff_pocket_states");
                let handoff: Vec<&str> = handoff_values
                    .iter()
                    .map(|value| value.as_str().expect("a pocket state name"))
                    .collect();
                let recovered: Vec<&str> = input
                    .ball_states
                    .iter()
                    .map(|ball| pocket_name(ball.pocket_state))
                    .collect();
                assert_eq!(
                    handoff, recovered,
                    "{id}: shot_input did not recover the handed-over pocket state"
                );
                assert_eq!(
                    state.balls[1].pocket_state,
                    if id == "shelf-01" {
                        PocketState::Entering
                    } else {
                        PocketState::Pocketed
                    },
                    "{id}: the shelf ball ended in the wrong state"
                );
            }
            "jaw" => assert!(
                kinds.contains(&"jaw") || kinds.contains(&"pocket-exit"),
                "{id}: no jaw/rattle event in {kinds:?}"
            ),
            "hung_up" => assert!(
                state
                    .balls
                    .iter()
                    .any(|ball| ball.pocket_state == PocketState::HungUp),
                "{id}: no ball hung up on the shelf"
            ),
            "miscue" => assert!(state.miscue, "{id}: the tip did not miscue"),
            "edge" => {
                let gap = positions[1].x - positions[0].x;
                let contact = 2.0 * cfg.ball.radius;
                assert!(
                    gap >= contact && gap - contact <= 1e-11,
                    "{id}: the balls are not at (or a hair short of) contact"
                );
                assert!(
                    kinds.contains(&"ball-ball"),
                    "{id}: the near-touching balls never met"
                );
            }
            "fault" => {
                assert!(faulted, "{id}: the fault record lacks the faulted flag");
                assert_eq!(
                    trace.events.len(),
                    shot_cfg.simulation.max_events as usize,
                    "{id}: the event guard did not stop the shot at the limit"
                );
            }
            _ => {}
        }
    }
}

/// The first 72 shots are self-play: shot n+1 starts from shot n's surviving
/// balls, with rotations carried from n's recorded path. Replaying the chain
/// here pins the hand-off itself, not just each shot in isolation. The tail
/// uses placed racks instead, so only consecutive carried shots are compared.
#[test]
fn carried_racks_match_the_preceding_shot() {
    let cfg = TableConfig::default();
    let shots = array(&corpus(), "shots");
    assert_eq!(shots.len(), 81, "the corpus must hold 81 shots");

    let is_carry = |category: &str| {
        matches!(
            category,
            "rails" | "spin" | "jumps" | "pockets" | "off_table" | "carry"
        )
    };
    let assert_bits = |got: &[Vec3], want: &[Vec3], id: &str, next_id: &str, what: &str| {
        assert_eq!(
            got.len(),
            want.len(),
            "{id} -> {next_id}: carried {what} count mismatch"
        );
        for (index, (stored, carried)) in got.iter().zip(want.iter()).enumerate() {
            for component in 0..3 {
                assert_eq!(
                    stored.get(component).to_bits(),
                    carried.get(component).to_bits(),
                    "{id} -> {next_id}: ball {index} {what} component {component} mismatch"
                );
            }
        }
    };

    let mut checked = 0;
    for pair in shots.windows(2) {
        let (shot, next) = (&pair[0], &pair[1]);
        if !is_carry(text(shot, "category")) || !is_carry(text(next, "category")) {
            continue;
        }
        let id = text(shot, "id");
        let next_id = text(next, "id");

        let intent = field(shot, "intent");
        let speed = number(intent, "speed");
        let aim_deg = number(intent, "level_aim_deg");
        let elevation_deg = number(intent, "elevation_deg");
        let contact_x = number(intent, "contact_x");
        let contact_y = number(intent, "contact_y");
        let positions = flat_vec3(shot, "positions");
        let numbers = ball_numbers(shot, "numbers");
        let rotations = flat_vec3(shot, "rotations");

        // Mirror the recorder's per-shot setup so the replayed path is the
        // same one the generator took when it wrote shot n+1's rack.
        let mut shot_cfg = cfg.clone();
        if let Some(limit) = shot.get("max_events") {
            let raw = limit
                .as_f64()
                .unwrap_or_else(|| panic!("{id}: max_events is not a number"));
            let limit = raw as u32;
            assert_eq!(raw, f64::from(limit), "{id}: max_events is integral");
            shot_cfg.simulation.max_events = limit;
        }
        let faulted = shot
            .get("faulted")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        assert!(!faulted, "{id}: a carried shot cannot be faulted");

        let direction = cue_direction(aim_deg, elevation_deg);
        let input = shot_input(
            &cfg,
            speed,
            direction,
            (contact_x, contact_y),
            &positions,
            &numbers,
            &rotations,
        );
        let (state, trace) = simulate_record(&input, &shot_cfg, faulted)
            .unwrap_or_else(|error| panic!("{id} faulted unexpectedly: {}", error.message));

        // The generator's hand-off: drop pocketed/off-table balls, keep each
        // survivor's position, carry its final rotation from the trace, and
        // re-spot the cue on the head spot if it did not survive.
        let final_rotations = trace.sample_rotation(trace.duration);
        let mut want_numbers: Vec<i32> = Vec::with_capacity(state.balls.len());
        let mut want_positions: Vec<Vec3> = Vec::with_capacity(state.balls.len());
        let mut want_rotations: Vec<Vec3> = Vec::with_capacity(state.balls.len());
        for (index, ball) in state.balls.iter().enumerate() {
            if ball.is_pocketed || ball.is_off_table {
                continue;
            }
            want_numbers.push(i32::from(ball.number));
            want_positions.push(ball.position);
            want_rotations.push(final_rotations.get(index).copied().unwrap_or(Vec3::ZERO));
        }
        if !want_numbers.contains(&0) {
            want_numbers.insert(0, 0);
            want_positions.insert(
                0,
                Vec3::new(
                    cfg.table.length / 4.0,
                    cfg.table.width / 2.0,
                    cfg.ball.radius,
                ),
            );
            want_rotations.insert(0, Vec3::ZERO);
        }

        assert_eq!(
            ball_numbers(next, "numbers"),
            want_numbers,
            "{id} -> {next_id}: carried ball numbers mismatch"
        );
        assert_bits(
            &flat_vec3(next, "positions"),
            &want_positions,
            id,
            next_id,
            "positions",
        );
        assert_bits(
            &flat_vec3(next, "rotations"),
            &want_rotations,
            id,
            next_id,
            "rotations",
        );
        checked += 1;
    }
    assert_eq!(
        checked, 71,
        "the carry chain must hold 71 consecutive pairs"
    );
}
