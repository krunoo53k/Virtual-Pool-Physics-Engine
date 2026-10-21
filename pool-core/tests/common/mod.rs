//! Shared helpers for the fixture-driven tests.
//!
//! Every test binary that includes this module only uses part of it.
#![allow(dead_code)]
//!
//! The fixtures in `tests/fixtures/` are the acceptance criteria of the physics
//! core: each file is a shot specification plus the expected outcome. They are read
//! with the engine's own JSON reader, so the numbers are exactly the ones the
//! generator wrote.

use std::fs;
use std::path::PathBuf;

use pool_core::json::Value;

/// The directory the fixtures live in.
pub fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests")
        .join("fixtures")
}

/// Reads and parses a fixture.
pub fn fixture(name: &str) -> Value {
    let path = fixture_dir().join(name);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    pool_core::json::parse(&text)
        .unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display()))
}

/// A field of an object, panicking with a readable message.
pub fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.field(key).unwrap_or_else(|error| panic!("{error}"))
}

/// An array field of an object.
pub fn array(value: &Value, key: &str) -> Vec<Value> {
    field(value, key)
        .as_array()
        .unwrap_or_else(|| panic!("'{key}' is not an array"))
        .to_vec()
}

/// The fixture's `setup.balls` entries.
pub fn setup_balls(fixture: &Value) -> Vec<Value> {
    array(field(fixture, "setup"), "balls")
}

/// The fixture's `expectations`, with any nested lists flattened.
pub fn expectations(fixture: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    for item in array(fixture, "expectations") {
        match item {
            Value::Array(inner) => {
                for nested in inner {
                    out.push(nested);
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// The value of a field of an expectation.
pub fn number(value: &Value, key: &str) -> f64 {
    field(value, key)
        .as_f64()
        .unwrap_or_else(|| panic!("'{key}' is not a number"))
}

/// The optional value of a field of an expectation.
pub fn optional_number(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|v| v.as_f64())
}

/// The string value of a field of an expectation.
pub fn text(value: &Value, key: &str) -> String {
    field(value, key)
        .as_str()
        .unwrap_or_else(|| panic!("'{key}' is not a string"))
        .to_string()
}

/// The tolerance of an expectation.
pub fn tolerance(value: &Value) -> f64 {
    value
        .get("tolerance")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
}

/// Asserts that a measured value matches an expectation within its tolerance.
pub fn assert_close(measured: f64, expected: f64, tolerance: f64, what: &str) {
    let difference = (measured - expected).abs();
    assert!(
        difference <= tolerance,
        "{what}: measured {measured}, expected {expected}, difference {difference} > {tolerance}"
    );
}
