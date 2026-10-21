//! The core's JSON as Godot's `Variant`.
//!
//! The core has its own JSON type, with the exact number handling the
//! determinism rules ask for (§11.4). The game reads the table's geometry and
//! the shot's events as dictionaries, so the two are converted here - in one
//! place, so that the game never has to know how the core writes JSON.

use godot::builtin::{GString, VarArray, VarDictionary, Variant};
use godot::meta::ToGodot;
use pool_core::json::Value;

/// A core JSON value as a Godot `Variant`.
pub fn to_variant(value: &Value) -> Variant {
    match value {
        Value::Null => Variant::nil(),
        Value::Bool(value) => value.to_variant(),
        Value::Number(value) => value.to_variant(),
        Value::String(value) => GString::from(value.as_str()).to_variant(),
        Value::Array(values) => {
            let mut array = VarArray::new();
            for value in values {
                array.push(&to_variant(value));
            }
            array.to_variant()
        }
        Value::Object(entries) => {
            let mut dictionary = VarDictionary::new();
            for (key, value) in entries {
                dictionary.set(key.as_str(), &to_variant(value));
            }
            dictionary.to_variant()
        }
    }
}

/// A core JSON value as a Godot dictionary.
///
/// A value that is not an object gives an empty dictionary.
pub fn to_dictionary(value: &Value) -> VarDictionary {
    let mut dictionary = VarDictionary::new();
    if let Value::Object(entries) = value {
        for (key, entry) in entries {
            dictionary.set(key.as_str(), &to_variant(entry));
        }
    }
    dictionary
}
