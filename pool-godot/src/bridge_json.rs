//! JSON exposure of the shared outcome mapping and the rules judge.
//!
//! The client's ruling preview used to be a second GDScript judge. It now
//! calls these functions: they run `pool-intent::outcome_from_trace` and
//! `pool-rules::judge` on the same typed values the server uses, so a preview
//! cannot disagree with the authoritative ruling. The numbers on the wire are
//! read with `pool-core`'s correctly-rounding JSON reader.
//!
//! Input (one object):
//!
//! ```json
//! { "numbers": [0, 1, ...],
//!   "events": [{"time": 0.3, "kind": "ball-ball", "a": 0, "b": 1, "detail": 0}],
//!   "state": {"phase": "settled", "foul": false, "miscue": false, "elapsed_time": 6.2},
//!   "params": {"speed": 8.0, "level_aim_deg": 90.0, "elevation_deg": 0.0,
//!              "contact_x": 0.0, "contact_y": 0.0, "called_pocket": -1},
//!   "match": {"player": 1, "solids_player": 0, "remaining": [1, ..., 15],
//!             "ball_in_hand": false, "break_next": true},
//!   "error": "" }
//! ```
//!
//! Output adds `events`, `outcome`, `judgement` and `announcements`.

use pool_core::json::{parse, Value};
use pool_core::sim::{EventKind as CoreEventKind, SimEvent};
use pool_core::state::{GamePhase, GameState};
use pool_core::trace::event_kind_name;

fn number(value: Option<&Value>) -> Result<f64, String> {
    value
        .and_then(Value::as_f64)
        .ok_or_else(|| "expected a number".to_string())
}

fn boolean(value: Option<&Value>, default: bool) -> Result<bool, String> {
    match value {
        None => Ok(default),
        Some(value) => value
            .as_bool()
            .ok_or_else(|| "expected a boolean".to_string()),
    }
}

fn integer(value: Option<&Value>, default: i32) -> Result<i32, String> {
    match value {
        None => Ok(default),
        Some(value) => Ok(number(Some(value))? as i32),
    }
}

fn array<'a>(value: Option<&'a Value>, name: &str) -> Result<&'a [Value], String> {
    value
        .and_then(Value::as_array)
        .ok_or_else(|| format!("expected an array for '{name}'"))
}

fn core_kind(word: &str) -> Option<CoreEventKind> {
    use CoreEventKind::*;
    Some(match word {
        "ball-ball" => BallBall,
        "cushion" => Cushion,
        "jaw" => Jaw,
        "slate" => SlateBounce,
        "state" => StateChange,
        "pocket-enter" => PocketEnter,
        "pocket-drop" => PocketDrop,
        "pocket-exit" => PocketExit,
        "off-table" => OffTable,
        _ => return None,
    })
}

fn parse_events(value: Option<&Value>) -> Result<Vec<SimEvent>, String> {
    array(value, "events")?
        .iter()
        .map(|entry| {
            let word = entry
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| "an event has no kind".to_string())?;
            Ok(SimEvent {
                time: number(entry.get("time"))?,
                kind: core_kind(word).ok_or_else(|| format!("unknown event kind '{word}'"))?,
                a: integer(entry.get("a"), -1)? as u8,
                b: integer(entry.get("b"), -1)? as u8,
                detail: integer(entry.get("detail"), 0)? as u8,
            })
        })
        .collect()
}

fn parse_state(value: Option<&Value>) -> Result<GameState, String> {
    let state = value.ok_or_else(|| "no 'state' object".to_string())?;
    let phase = match state
        .get("phase")
        .and_then(Value::as_str)
        .unwrap_or("settled")
    {
        "settled" => GamePhase::Settled,
        "settling" => GamePhase::Settling,
        "faulted" => GamePhase::Faulted,
        other => return Err(format!("unknown phase '{other}'")),
    };
    let mut game = GameState::new(Vec::new());
    game.phase = phase;
    game.foul = boolean(state.get("foul"), false)?;
    game.miscue = boolean(state.get("miscue"), false)?;
    game.elapsed_time = number(state.get("elapsed_time"))?;
    Ok(game)
}

fn parse_params(value: Option<&Value>) -> Result<pool_rules::ShotParams, String> {
    let params = value.ok_or_else(|| "no 'params' object".to_string())?;
    Ok(pool_rules::ShotParams {
        speed: number(params.get("speed"))?,
        level_aim_deg: number(params.get("level_aim_deg"))?,
        elevation_deg: number(params.get("elevation_deg"))?,
        contact_x: number(params.get("contact_x"))?,
        contact_y: number(params.get("contact_y"))?,
        called_pocket: integer(params.get("called_pocket"), -1)?,
    })
}

fn parse_match(value: Option<&Value>) -> Result<pool_rules::MatchState, String> {
    let state = value.ok_or_else(|| "no 'match' object".to_string())?;
    Ok(pool_rules::MatchState {
        player: integer(state.get("player"), 1)?,
        solids_player: integer(state.get("solids_player"), 0)?,
        remaining: array(state.get("remaining"), "remaining")?
            .iter()
            .map(|value| Ok(number(Some(value))? as i32))
            .collect::<Result<Vec<i32>, String>>()?,
        ball_in_hand: boolean(state.get("ball_in_hand"), false)?,
        break_next: boolean(state.get("break_next"), true)?,
    })
}

fn events_value(events: &[pool_intent::ShotEvent]) -> Value {
    Value::Array(
        events
            .iter()
            .map(|event| {
                Value::Object(vec![
                    ("time".into(), Value::Number(event.time)),
                    ("kind".into(), Value::String(event.kind.as_str().into())),
                    ("ball".into(), Value::Number(event.ball as f64)),
                    ("other".into(), Value::Number(event.other as f64)),
                    ("detail".into(), Value::Number(event.detail as f64)),
                ])
            })
            .collect(),
    )
}

fn outcome_value(outcome: &pool_intent::Outcome) -> Value {
    Value::Object(vec![
        ("elapsed_time".into(), Value::Number(outcome.elapsed_time)),
        ("phase".into(), Value::String(outcome.phase.as_str().into())),
        ("foul".into(), Value::Bool(outcome.foul)),
        ("miscue".into(), Value::Bool(outcome.miscue)),
        (
            "pocketed".into(),
            Value::Array(
                outcome
                    .pocketed
                    .iter()
                    .map(|n| Value::Number(*n as f64))
                    .collect(),
            ),
        ),
        (
            "off_table".into(),
            Value::Array(
                outcome
                    .off_table
                    .iter()
                    .map(|n| Value::Number(*n as f64))
                    .collect(),
            ),
        ),
        ("error".into(), Value::String(outcome.error.clone())),
    ])
}

fn judgement_value(ruling: &pool_rules::Judgement) -> Value {
    Value::Object(vec![
        (
            "foul_reason".into(),
            Value::String(ruling.foul_reason.clone()),
        ),
        (
            "next_player".into(),
            Value::Number(ruling.next_player as f64),
        ),
        ("ball_in_hand".into(), Value::Bool(ruling.ball_in_hand)),
        ("re_rack".into(), Value::Bool(ruling.re_rack)),
        (
            "solids_player".into(),
            Value::Number(ruling.solids_player as f64),
        ),
        ("game_over".into(), Value::Bool(ruling.game_over)),
        ("winner".into(), Value::Number(ruling.winner as f64)),
    ])
}

/// Runs the shared outcome mapping and the shared judge over one JSON object.
///
/// Returns a JSON object, or a JSON object with an `error` field; it never
/// throws, because a preview must not take the game down.
pub fn preview_json(text: &str) -> String {
    let result = (|| -> Result<Value, String> {
        let input = parse(text).map_err(|error| error.to_string())?;
        let numbers = array(input.get("numbers"), "numbers")?
            .iter()
            .map(|value| Ok(number(Some(value))? as i32))
            .collect::<Result<Vec<i32>, String>>()?;
        let events = parse_events(input.get("events"))?;
        let state = parse_state(input.get("state"))?;
        let error = input
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let (typed, outcome) = pool_intent::outcome_from_trace(&numbers, &events, &state, &error);
        let params = parse_params(input.get("params"))?;
        let match_state = parse_match(input.get("match"))?;
        let (ruling, announcements) =
            pool_rules::judge_and_announce(&outcome, &params, &typed, &match_state);
        Ok(Value::Object(vec![
            (
                "event_digest".into(),
                Value::String(pool_intent::events_digest(&events, &numbers)),
            ),
            ("events".into(), events_value(&typed)),
            ("outcome".into(), outcome_value(&outcome)),
            ("judgement".into(), judgement_value(&ruling)),
            (
                "announcements".into(),
                Value::Array(announcements.into_iter().map(Value::String).collect()),
            ),
        ]))
    })();
    match result {
        Ok(value) => value.to_json(),
        Err(error) => Value::Object(vec![("error".into(), Value::String(error))]).to_json(),
    }
}

/// The judge alone, for a caller that already has typed events and an outcome.
pub fn judge_json(text: &str) -> String {
    subset(text, &["judgement", "announcements"])
}

/// The outcome mapping alone, for a caller that has no match state to judge.
pub fn outcome_mapping_json(text: &str) -> String {
    subset(text, &["events", "outcome", "event_digest"])
}

fn subset(text: &str, keys: &[&str]) -> String {
    let full = preview_json(text);
    let value = match parse(&full) {
        Ok(value) => value,
        Err(_) => return full,
    };
    if value.get("error").is_some() {
        return full;
    }
    let entries: Vec<(String, Value)> = keys
        .iter()
        .map(|key| {
            (
                (*key).to_string(),
                value.get(key).cloned().unwrap_or(Value::Null),
            )
        })
        .collect();
    Value::Object(entries).to_json()
}

/// The typed outcome alone, from already-parsed core values.
pub fn outcome_json(
    numbers: &[i32],
    events: &[SimEvent],
    state: &GameState,
    error: &str,
) -> String {
    let (typed, outcome) = pool_intent::outcome_from_trace(numbers, events, state, error);
    Value::Object(vec![
        ("events".into(), events_value(&typed)),
        ("outcome".into(), outcome_value(&outcome)),
    ])
    .to_json()
}

/// The core's own name for an event kind, for fixture round-trips.
pub fn event_word(kind: CoreEventKind) -> &'static str {
    event_kind_name(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_judge_reports_the_same_reason_word_as_the_client_fixture() {
        let input = r#"{
            "numbers": [0, 1],
            "events": [{"time": 0.3, "kind": "ball-ball", "a": 0, "b": 1, "detail": 0},
                       {"time": 0.6, "kind": "pocket-drop", "a": 0, "b": 1, "detail": 0}],
            "state": {"phase": "settled", "foul": true, "miscue": false, "elapsed_time": 1.0},
            "params": {"speed": 8.0, "level_aim_deg": 90.0, "elevation_deg": 0.0,
                       "contact_x": 0.0, "contact_y": 0.0, "called_pocket": -1},
            "match": {"player": 1, "solids_player": 0, "remaining": [1],
                      "ball_in_hand": false, "break_next": false},
            "error": ""
        }"#;
        let output = preview_json(input);
        let value = parse(&output).expect("the output is JSON");
        assert_eq!(
            value
                .get("judgement")
                .and_then(|judgement| judgement.get("foul_reason"))
                .and_then(Value::as_str),
            Some("scratch")
        );
        let announcements = value
            .get("announcements")
            .and_then(Value::as_array)
            .expect("the announcements");
        assert_eq!(announcements[0].as_str(), Some("foul_committed:scratch"));
    }

    #[test]
    fn a_malformed_preview_returns_an_error_object() {
        let output = preview_json("not json");
        let value = parse(&output).expect("even the error is JSON");
        assert!(value.get("error").and_then(Value::as_str).is_some());
    }
}
