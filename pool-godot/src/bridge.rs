//! The f64-only bridge helpers, in the core frame.
//!
//! The old methods take Godot `Vector3`/`Vector2`, whose components are `f32`,
//! and pack rack positions into `PackedVector3Array`. That path stays for
//! hotseat. The online path takes flat `f64` arrays instead, so no number that
//! enters physics is ever quantised on the way through Godot. The conversion
//! here is pure Rust and can be tested without an engine running.
//!
//! Positions and rotations are flat triples `[x0, y0, z0, x1, ...]` in the
//! **core** frame: `x` along the table, `y` across it, `z` up. The Godot
//! display transform `(x, z, -y)` is one-way and is never applied here.

use pool_core::config::TableConfig;
use pool_core::math::Vec3;
use pool_core::sim::SimError;
use pool_core::state::GameState;
use pool_core::Trace;

/// Flat core-frame triples to vectors.
pub fn flat_triples(values: &[f64]) -> Vec<Vec3> {
    values
        .chunks_exact(3)
        .map(|triple| Vec3::new(triple[0], triple[1], triple[2]))
        .collect()
}

/// Vectors to flat core-frame triples.
pub fn triples_to_flat(vectors: &[Vec3]) -> Vec<f64> {
    let mut flat = Vec::with_capacity(vectors.len() * 3);
    for vector in vectors {
        flat.extend_from_slice(&[vector.x, vector.y, vector.z]);
    }
    flat
}

/// The whole f64 shot path, from the six intent scalars and a flat rack.
///
/// The direction and the shot input come from `pool-intent`, the same crate
/// the server uses. Nothing here casts to `f32`.
///
/// The argument count is the wire contract: six scalars plus the three flat
/// rack arrays, with no Godot types in between.
#[allow(clippy::too_many_arguments)]
pub fn simulate_from_intent_f64(
    table: &TableConfig,
    cue_speed: f64,
    level_aim_deg: f64,
    elevation_deg: f64,
    contact_x: f64,
    contact_y: f64,
    positions: &[f64],
    numbers: &[i32],
    rotations: &[f64],
) -> Result<(GameState, Trace), SimError> {
    let direction = pool_intent::cue_direction(level_aim_deg, elevation_deg);
    let positions = flat_triples(positions);
    let rotations = flat_triples(rotations);
    let input = pool_intent::shot_input(
        table,
        cue_speed,
        direction,
        (contact_x, contact_y),
        &positions,
        numbers,
        &rotations,
    );
    pool_core::try_simulate_shot_traced(&input, table)
}

/// The core-frame direction for the same degrees the bridge stores, as a flat
/// triple. The client and the server can compare this number before a shot.
pub fn cue_direction_f64(level_aim_deg: f64, elevation_deg: f64) -> Vec<f64> {
    let direction = pool_intent::cue_direction(level_aim_deg, elevation_deg);
    vec![direction.x, direction.y, direction.z]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_triples_round_trip_without_a_cast() {
        let vectors = vec![
            Vec3::new(0.1, 0.2, 0.3),
            Vec3::new(f64::from_bits(1), f64::from_bits(2), f64::from_bits(3)),
        ];
        let flat = triples_to_flat(&vectors);
        assert_eq!(flat.len(), 6);
        let back = flat_triples(&flat);
        for (a, b) in vectors.iter().zip(back.iter()) {
            assert_eq!(a.x.to_bits(), b.x.to_bits());
            assert_eq!(a.y.to_bits(), b.y.to_bits());
            assert_eq!(a.z.to_bits(), b.z.to_bits());
        }
    }

    #[test]
    fn an_incomplete_triple_is_ignored() {
        assert_eq!(flat_triples(&[1.0, 2.0, 3.0, 4.0]).len(), 1);
    }
}
