//! The E4 determinism pins.
//!
//! The single old golden hash is kept as-is. This file adds the pins the
//! online protocol needs: an f64 round-trip, a three-shot carry, a random
//! f64-bit-pattern serialisation round-trip, the startup canary, a rotation
//! and path pin, and client/server equivalence of the two bridge paths.
//!
//! All of them are pure Rust: the wasm32 CI lane runs this file too.

use pool_core::config::TableConfig;
use pool_core::hash::{hash_state, hash_state_hex, Hasher};
use pool_core::json::{parse, Value};
use pool_core::math::Vec3;
use pool_godot::bridge;
use pool_godot::frame;

/// The old golden hash: the f32 hotseat break must not move.
const OLD_GOLDEN: &str = "c399229b25faae5a";
/// The startup canary from the three-shot reference sequence.
const CANARY: &str = "c8ee370c4195017f";

/// The three-shot reference: hashes, path hashes and carried rotation hashes.
const THREE_SHOT_HASHES: [&str; 3] = ["51817b37b13c4587", "9a1017d7cdcb2235", "2b0dedb66711845a"];
const THREE_SHOT_PATHS: [&str; 3] = ["0b00bcad82ab5257", "d14b69b2264cefeb", "c0c9b496bc03b9e0"];
const THREE_SHOT_ROTATIONS: [&str; 3] =
    ["b4a68d2f572bb938", "93d6bd0edd43cec1", "e42750c204660cc4"];

const REFERENCE: [(f64, f64, f64, f64, f64); 3] = [
    (8.0, 90.0, 0.0, 0.0, 0.0),
    (6.5, 105.0, 0.0, 0.006, 0.004),
    (7.5, 78.0, 8.0, -0.005, -0.003),
];

fn rack(table: &TableConfig) -> (Vec<Vec3>, Vec<i32>) {
    let radius = table.ball.radius;
    let mut positions = vec![Vec3::new(0.25, 0.5625, radius)];
    let mut numbers = vec![0];
    for row in 0..4 {
        for column in 0..4 {
            if row == 0 && column == 0 {
                continue;
            }
            positions.push(Vec3::new(
                0.5 + 0.0625 * column as f64,
                0.5 + 0.0625 * row as f64,
                radius,
            ));
            numbers.push(numbers.len() as i32);
        }
    }
    (positions, numbers)
}

fn path_hash(trace: &pool_core::Trace) -> String {
    let mut hasher = Hasher::new();
    for t in [0.0, trace.duration * 0.5, trace.duration] {
        for position in trace.sample_positions(t) {
            hasher.write_f64(position.x);
            hasher.write_f64(position.y);
            hasher.write_f64(position.z);
        }
    }
    format!("{:016x}", hasher.finish())
}

fn rotation_hash(rotations: &[Vec3]) -> String {
    let mut hasher = Hasher::new();
    for rotation in rotations {
        hasher.write_f64(rotation.x);
        hasher.write_f64(rotation.y);
        hasher.write_f64(rotation.z);
    }
    format!("{:016x}", hasher.finish())
}

#[test]
fn the_old_golden_hash_has_not_moved() {
    let table = TableConfig::default();
    let (positions, numbers) = rack(&table);
    // The old method's path: core -> Godot f32 -> core.
    let positions: Vec<Vec3> = positions
        .iter()
        .map(|p| frame::to_core(frame::to_godot(*p)))
        .collect();
    let direction = frame::to_core(frame::to_godot(Vec3::Y));
    let input = pool_godot::shot::shot_input(
        &table,
        8.0,
        direction,
        (0.0, 0.0),
        &positions,
        &numbers,
        &[],
    );
    let (state, _) = pool_godot::shot::simulate(&table, &input).expect("the break runs");
    assert_eq!(hash_state_hex(&state), OLD_GOLDEN);
}

#[test]
fn the_startup_canary_is_pinned() {
    assert_eq!(pool_intent::canary_hex(), CANARY);
    assert!(pool_intent::canary_of(CANARY));
    assert!(!pool_intent::canary_of("0000000000000000"));
}

#[test]
fn an_f64_round_trip_keeps_every_bit() {
    let vectors = vec![
        Vec3::new(0.1, 0.2, 0.3),
        Vec3::new(f64::from_bits(1), f64::from_bits(2), f64::from_bits(3)),
        Vec3::new(1e300, -1e-300, -0.0),
    ];
    let flat = bridge::triples_to_flat(&vectors);
    let back = bridge::flat_triples(&flat);
    assert_eq!(back.len(), vectors.len());
    for (before, after) in vectors.iter().zip(back.iter()) {
        for i in 0..3 {
            assert_eq!(before.get(i).to_bits(), after.get(i).to_bits());
        }
    }
}

#[test]
fn random_f64_bit_patterns_survive_json() {
    // The wire uses JSON.stringify(..., full_precision=true) on the client and
    // pool-core's exact JSON writer on the server. Both write the shortest
    // round-tripping decimal; parse it back and compare raw bits.
    let mut seed: u64 = 0x1234_5678_9abc_def0;
    let mut hasher = Hasher::new();
    let mut checked = 0;
    for _ in 0..256 {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let value = f64::from_bits(seed);
        if !value.is_finite() {
            continue;
        }
        let text = Value::Number(value).to_json();
        let parsed = parse(&text).expect("the number parses");
        let back = parsed.as_f64().expect("it is a number");
        assert_eq!(back.to_bits(), value.to_bits(), "lost {text}");
        hasher.write_u64(value.to_bits());
        checked += 1;
    }
    assert!(checked > 200);
    // Pin the sequence itself, so a different generator cannot quietly weaken
    // the test.
    assert_eq!(
        format!("{:016x}", hasher.finish()),
        "d9e504e33fb95cb4",
        "the random bit-pattern sequence moved"
    );
}

#[test]
fn three_shots_carry_rotations_and_path() {
    let table = TableConfig::default();
    let (mut positions, mut numbers) = rack(&table);
    let mut rotations: Vec<Vec3> = Vec::new();
    for (index, (speed, aim, elevation, cx, cy)) in REFERENCE.iter().enumerate() {
        let (state, trace) = bridge::simulate_from_intent_f64(
            &table,
            *speed,
            *aim,
            *elevation,
            *cx,
            *cy,
            &bridge::triples_to_flat(&positions),
            &numbers,
            &bridge::triples_to_flat(&rotations),
        )
        .expect("the reference shot runs");
        assert_eq!(hash_state_hex(&state), THREE_SHOT_HASHES[index]);
        assert_eq!(path_hash(&trace), THREE_SHOT_PATHS[index]);
        let carried = trace.sample_rotation(trace.duration);
        assert_eq!(rotation_hash(&carried), THREE_SHOT_ROTATIONS[index]);
        let mut next_positions = Vec::new();
        let mut next_numbers = Vec::new();
        let mut next_rotations = Vec::new();
        for (ball_index, ball) in state.balls.iter().enumerate() {
            if ball.is_pocketed || ball.is_off_table {
                continue;
            }
            let mut position = ball.position;
            position.z = table.ball.radius;
            next_positions.push(position);
            next_numbers.push(ball.number as i32);
            next_rotations.push(carried[ball_index]);
        }
        if !next_numbers.contains(&0) {
            next_positions.insert(0, positions[0]);
            next_numbers.insert(0, 0);
            next_rotations.insert(0, Vec3::ZERO);
        }
        positions = next_positions;
        numbers = next_numbers;
        rotations = next_rotations;
    }
}

#[test]
fn the_state_hash_excludes_rotations_but_includes_pocket_state() {
    let table = TableConfig::default();
    let state = pool_intent::canary::rack_state(&table);
    let base = hash_state(&state);
    let mut turned = state.clone();
    turned.balls[1].rotation = Vec3::new(1.0, 2.0, 3.0);
    assert_eq!(hash_state(&turned), base, "a rendering fact moved the hash");

    let mut pocketed = state.clone();
    pocketed.balls[1].pocket_state = pool_core::state::PocketState::Pocketed;
    pocketed.balls[1].is_pocketed = true;
    assert_ne!(hash_state(&pocketed), base, "a pot did not move the hash");

    let mut off = state;
    off.balls[1].pocket_state = pool_core::state::PocketState::OffTable;
    off.balls[1].is_off_table = true;
    assert_ne!(
        hash_state(&off),
        base,
        "an off-table ball did not move the hash"
    );
}

#[test]
fn client_and_server_derive_the_same_shot() {
    let table = TableConfig::default();
    let (positions, numbers) = rack(&table);
    let flat = bridge::triples_to_flat(&positions);

    // The client simulates the values it put on the wire, not its locals.
    // Serialise through the same exact JSON the engine uses, parse it back and
    // simulate on both sides from those numbers.
    let wire = Value::Object(vec![
        (
            "positions".to_string(),
            Value::Array(flat.iter().copied().map(Value::Number).collect()),
        ),
        (
            "numbers".to_string(),
            Value::Array(numbers.iter().map(|n| Value::Number(*n as f64)).collect()),
        ),
        ("speed".to_string(), Value::Number(8.0)),
        ("aim".to_string(), Value::Number(90.0)),
    ])
    .to_json();
    let parsed = parse(&wire).expect("the shot round-trips");
    let client_positions: Vec<f64> = parsed
        .field("positions")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_f64().unwrap())
        .collect();
    let client_numbers: Vec<i32> = parsed
        .field("numbers")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_f64().unwrap() as i32)
        .collect();
    let speed = parsed.field("speed").unwrap().as_f64().unwrap();
    let aim = parsed.field("aim").unwrap().as_f64().unwrap();

    let (client_state, _) = bridge::simulate_from_intent_f64(
        &table,
        speed,
        aim,
        0.0,
        0.0,
        0.0,
        &client_positions,
        &client_numbers,
        &[],
    )
    .expect("client shot runs");
    let (server_state, _) =
        bridge::simulate_from_intent_f64(&table, speed, aim, 0.0, 0.0, 0.0, &flat, &numbers, &[])
            .expect("server shot runs");
    assert_eq!(hash_state(&client_state), hash_state(&server_state));
    assert_eq!(hash_state_hex(&server_state), THREE_SHOT_HASHES[0]);

    // The legacy hotseat path is a different shot by design: its direction
    // arrives as an f32 Vector3 and its rack as f32 triples. It keeps its own
    // golden hash and is not expected to match the f64 path.
    let godot_positions: Vec<godot::builtin::Vector3> =
        positions.iter().map(|p| frame::to_godot(*p)).collect();
    let core_positions: Vec<Vec3> = godot_positions.iter().map(|p| frame::to_core(*p)).collect();
    let f32_shot = pool_godot::shot::shot_input(
        &table,
        8.0,
        frame::to_core(frame::to_godot(Vec3::Y)),
        (0.0, 0.0),
        &core_positions,
        &numbers,
        &[],
    );
    let (f32_state, _) = pool_godot::shot::simulate(&table, &f32_shot).expect("f32 shot runs");
    assert_eq!(hash_state_hex(&f32_state), OLD_GOLDEN);
}

#[test]
fn direction_is_the_same_in_both_bridges() {
    let a = pool_intent::cue_direction(450.0, 12.0);
    let b = frame::to_core(frame::cue_direction(450.0, 12.0));
    for i in 0..3 {
        assert!((a.get(i) - b.get(i)).abs() < 1e-7);
    }
}
