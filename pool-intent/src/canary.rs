//! The three-shot determinism canary.
//!
//! A version string is hand-maintained and misses build flags, architecture
//! and wasm drift. `HELLO` therefore carries the hash of a fixed three-shot
//! reference sequence, computed at startup from the same pure-Rust pipeline on
//! both sides. It covers the final state hashes, the carried rotations and a
//! path sample, so it catches changes the state hash deliberately ignores.

use pool_core::config::TableConfig;
use pool_core::hash::{hash_state, Hasher};
use pool_core::math::Vec3;
use pool_core::state::{BallState, GameState};

/// The rack the engine's smoke break uses: a 4×4 grid with one ball missing,
/// and the cue a quarter of a metre to the side.
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

/// One reference stroke: speed, level aim, butt raise, right tip, up tip.
const REFERENCE: [(f64, f64, f64, f64, f64); 3] = [
    (8.0, 90.0, 0.0, 0.0, 0.0),
    (6.5, 105.0, 0.0, 0.006, 0.004),
    (7.5, 78.0, 8.0, -0.005, -0.003),
];

/// The rack after a shot: potted and off-table balls are gone, survivors are
/// laid back on the bed with their carried rotation, and a scratched cue is
/// respotted at the head spot with zero rotation.
fn carry(
    state: &GameState,
    rotations: &[Vec3],
    table: &TableConfig,
) -> (Vec<Vec3>, Vec<i32>, Vec<Vec3>) {
    let (head, _) = {
        let (positions, numbers) = rack(table);
        (positions[0], numbers[0])
    };
    let mut positions = Vec::new();
    let mut numbers = Vec::new();
    let mut carried = Vec::new();
    for (index, ball) in state.balls.iter().enumerate() {
        if ball.is_pocketed || ball.is_off_table {
            continue;
        }
        let mut position = ball.position;
        position.z = table.ball.radius;
        positions.push(position);
        numbers.push(ball.number as i32);
        carried.push(rotations.get(index).copied().unwrap_or(Vec3::ZERO));
    }
    if !numbers.contains(&0) {
        positions.insert(0, head);
        numbers.insert(0, 0);
        carried.insert(0, Vec3::ZERO);
    }
    (positions, numbers, carried)
}

/// The canary of the reference sequence, as 16 lower-case hex digits.
///
/// The same function runs in the server and in the client preview; a mismatch
/// means a build-flag, architecture or implementation difference, regardless
/// of what the version strings claim.
pub fn canary_hex() -> String {
    let table = TableConfig::default();
    let (mut positions, mut numbers) = rack(&table);
    let mut rotations: Vec<Vec3> = Vec::new();
    let mut hasher = Hasher::new();
    for (speed, aim, elevation, contact_x, contact_y) in REFERENCE {
        let input = crate::shot_input(
            &table,
            speed,
            crate::cue_direction(aim, elevation),
            (contact_x, contact_y),
            &positions,
            &numbers,
            &rotations,
        );
        let (state, trace) = pool_core::try_simulate_shot_traced(&input, &table)
            .expect("the canary reference shot runs");
        hasher.write_u64(hash_state(&state));
        // The shared protocol hashes the event digest as well: the rules read
        // events, so a build that agrees on the final state but resolves a
        // different rail or pocket must fail the canary.
        hasher.write_bytes(crate::events_digest(&trace.events, &numbers).as_bytes());
        for rotation in trace.sample_rotation(trace.duration) {
            hasher.write_f64(rotation.x);
            hasher.write_f64(rotation.y);
            hasher.write_f64(rotation.z);
        }
        for position in trace.sample_positions(trace.duration * 0.5) {
            hasher.write_f64(position.x);
            hasher.write_f64(position.y);
            hasher.write_f64(position.z);
        }
        let next = carry(&state, &trace.sample_rotation(trace.duration), &table);
        positions = next.0;
        numbers = next.1;
        rotations = next.2;
    }
    format!("{:016x}", hasher.finish())
}

/// The canary of an arbitrary sequence, for a client that wants to check a
/// custom rack through the same fold. The reference value is [`canary_hex`].
pub fn canary_of(hex: &str) -> bool {
    canary_hex() == hex
}

/// A head-spot ball at rest, for the folding tests.
pub fn head_spot(table: &TableConfig) -> Vec3 {
    let (positions, _) = rack(table);
    positions[0]
}

/// A fresh 16-ball rack at rest, for tests that need a starting state.
pub fn rack_state(table: &TableConfig) -> GameState {
    let (positions, numbers) = rack(table);
    let balls: Vec<BallState> = positions
        .into_iter()
        .zip(numbers)
        .map(|(position, number)| BallState::at_rest(table, number as u8, position))
        .collect();
    GameState::new(balls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_canary_is_repeatable() {
        assert_eq!(canary_hex(), canary_hex());
        assert_eq!(canary_hex().len(), 16);
    }
}
