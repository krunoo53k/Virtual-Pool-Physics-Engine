//! The determinism tests (§11.4, §14.4).
//!
//! The whole point of the architecture: identical inputs must produce an identical
//! state, bit for bit, on every run and on every platform.

mod common;

use common::{array, field, fixture, number, optional_number};
use pool_core::hash::{canonical_bytes, hash_state_hex};
use pool_core::math::Vec3;
use pool_core::state::{BallState, ShotInput};
use pool_core::{hash_state, simulate_shot, try_simulate_shot, TableConfig};

/// Builds the golden break shot from the fixture.
fn golden_shot(cfg: &TableConfig) -> ShotInput {
    let fixture = fixture("determinism_golden.json");
    let setup = field(&fixture, "setup");
    let shot = field(setup, "shot");

    let mut balls: Vec<BallState> = Vec::new();
    for (index, entry) in array(setup, "balls").iter().enumerate() {
        let position = field(entry, "position")
            .as_array()
            .expect("a position")
            .to_vec();
        let x = position[0].as_f64().expect("a number");
        let y = position[1].as_f64().expect("a number");
        balls.push(BallState::at_rest(
            cfg,
            index as u8,
            Vec3::new(x, y, cfg.ball.radius),
        ));
    }

    let direction = field(shot, "cue_direction")
        .as_array()
        .expect("a direction")
        .to_vec();
    let dx = direction[0].as_f64().expect("a number");
    let dy = direction[1].as_f64().expect("a number");

    let mut input = ShotInput::new(number(shot, "cue_speed"), Vec3::new(dx, dy, 0.0), balls);
    let offset = field(shot, "contact_offset")
        .as_array()
        .expect("an offset")
        .to_vec();
    input.contact_offset_x = offset[0].as_f64().expect("a number");
    input.contact_offset_y = offset[1].as_f64().expect("a number");
    input
}

#[test]
fn the_golden_shot_is_reproducible() {
    let mut cfg = TableConfig::default();
    let fixture = fixture("determinism_golden.json");
    if let Some(max_sim_time) =
        optional_number(field(field(&fixture, "setup"), "params"), "max_sim_time")
    {
        cfg.simulation.max_sim_time = max_sim_time;
    }

    let input = golden_shot(&cfg);
    let first = try_simulate_shot(&input, &cfg).expect("the shot runs");
    let second = try_simulate_shot(&input, &cfg).expect("the shot runs");

    // The two runs agree in every single bit.
    assert_eq!(hash_state(&first), hash_state(&second));
    assert_eq!(hash_state_hex(&first), hash_state_hex(&second));
    assert_eq!(first, second);

    // And the hash matches the golden value in the fixture.
    let golden = field(&fixture, "expectations")
        .as_array()
        .expect("the expectations")
        .iter()
        .find(|entry| entry.get("quantity").and_then(|q| q.as_str()) == Some("final_state_hash"))
        .and_then(|entry| entry.get("expected"))
        .and_then(|value| value.as_str())
        .and_then(|text| u64::from_str_radix(text, 16).ok());

    if let Some(golden) = golden {
        assert_eq!(
            hash_state(&first),
            golden,
            "the final state hash does not match the golden value in the fixture"
        );
    }
}

#[test]
fn the_event_order_is_stable() {
    let cfg = TableConfig::default();
    let input = golden_shot(&cfg);
    let mut first = pool_core::Simulation::new(&input, &cfg);
    let _ = first.run(&input);
    let mut second = pool_core::Simulation::new(&input, &cfg);
    let _ = second.run(&input);

    assert_eq!(first.events().len(), second.events().len());
    for (a, b) in first.events().iter().zip(second.events().iter()) {
        assert_eq!(a.time.to_bits(), b.time.to_bits());
        assert_eq!(a.kind, b.kind);
        assert_eq!(a.a, b.a);
        assert_eq!(a.b, b.b);
    }
}

#[test]
fn a_repeated_run_gives_the_same_bytes() {
    let cfg = TableConfig::default();
    let input = golden_shot(&cfg);
    let first = simulate_shot(input.clone(), &cfg);
    let second = simulate_shot(input, &cfg);

    // The canonical serialisation of the two states is byte for byte identical.
    let first_bytes = canonical_bytes(&first);
    let second_bytes = canonical_bytes(&second);
    assert_eq!(first_bytes.len(), second_bytes.len());
    assert!(
        first_bytes == second_bytes,
        "the two runs differ in {} of {} bytes",
        first_bytes
            .iter()
            .zip(second_bytes.iter())
            .filter(|(a, b)| a != b)
            .count(),
        first_bytes.len()
    );
    assert_eq!(hash_state_hex(&first), hash_state_hex(&second));

    // The hash is over the raw f64 state: a single bit of difference changes it.
    let mut third = first.clone();
    third.balls[0].position.x = f64::from_bits(third.balls[0].position.x.to_bits() + 1);
    assert_ne!(hash_state(&first), hash_state(&third));
    assert_ne!(canonical_bytes(&first), canonical_bytes(&third));
}

#[test]
fn the_fixture_pins_the_hash() {
    // The golden fixture must carry a hash, otherwise the test above is vacuous.
    let fixture = fixture("determinism_golden.json");
    let expectations = array(&fixture, "expectations");
    let entry = expectations
        .iter()
        .find(|entry| entry.get("quantity").and_then(|q| q.as_str()) == Some("final_state_hash"))
        .expect("the final_state_hash expectation");
    let golden = entry
        .get("expected")
        .and_then(|value| value.as_str())
        .expect("the golden hash is filled in");
    assert_eq!(golden.len(), 16, "the golden hash is a 64 bit hex value");
    assert!(
        golden.chars().all(|c| c.is_ascii_hexdigit()),
        "the golden hash is hexadecimal"
    );
}
