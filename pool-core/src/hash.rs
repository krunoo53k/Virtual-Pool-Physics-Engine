//! A canonical, byte-exact hash of the simulation state (§14.4).
//!
//! The hash is computed over the *raw* `f64` values - never over a formatted or
//! rounded representation - so that it detects a single-bit difference between two
//! runs. It deliberately contains nothing that depends on the wall clock, on hash
//! map iteration order or on thread scheduling.

use crate::state::GameState;

/// The FNV-1a 64 bit offset basis.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/// The FNV-1a 64 bit prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// A running FNV-1a 64 bit hash.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hasher {
    state: u64,
}

impl Hasher {
    /// A fresh hasher.
    pub fn new() -> Hasher {
        Hasher { state: FNV_OFFSET }
    }

    /// Mixes in raw bytes.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.state ^= *b as u64;
            self.state = self.state.wrapping_mul(FNV_PRIME);
        }
    }

    /// Mixes in a `u64`.
    pub fn write_u64(&mut self, value: u64) {
        self.write_bytes(&value.to_le_bytes());
    }

    /// Mixes in an `f64` by its raw bit pattern.
    pub fn write_f64(&mut self, value: f64) {
        self.write_u64(value.to_bits());
    }

    /// Mixes in a `u8`.
    pub fn write_u8(&mut self, value: u8) {
        self.write_bytes(&[value]);
    }

    /// The hash value.
    pub fn finish(&self) -> u64 {
        self.state
    }
}

/// The canonical byte form of a game state.
///
/// This is the serialisation the determinism check compares (§14.4): the raw `f64`
/// values in a fixed order, never a formatted or rounded representation. Two runs
/// that agree byte for byte here agree on every bit of the state.
pub fn canonical_bytes(state: &GameState) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::with_capacity(16 * (9 * 8 + 4) + 128);
    bytes.extend_from_slice(&(state.balls.len() as u64).to_le_bytes());
    for ball in &state.balls {
        for value in [
            ball.position.x,
            ball.position.y,
            ball.position.z,
            ball.velocity.x,
            ball.velocity.y,
            ball.velocity.z,
            ball.angular_velocity.x,
            ball.angular_velocity.y,
            ball.angular_velocity.z,
        ] {
            bytes.extend_from_slice(&value.to_bits().to_le_bytes());
        }
        bytes.push(ball.motion_state as u8);
        bytes.push(ball.pocket_state as u8);
        bytes.push(ball.number);
        bytes.push(u8::from(ball.is_pocketed));
        bytes.push(u8::from(ball.is_off_table));
    }
    // The rest of the game state: the phase, the flags and the cue stick.
    bytes.push(state.phase as u8);
    bytes.push(state.turn);
    bytes.push(u8::from(state.foul));
    bytes.push(u8::from(state.game_over));
    bytes.push(u8::from(state.miscue));
    bytes.push(state.winner.unwrap_or(0xff));
    for value in [
        state.cue_stick.butt_position.x,
        state.cue_stick.butt_position.y,
        state.cue_stick.butt_position.z,
        state.cue_stick.direction.x,
        state.cue_stick.direction.y,
        state.cue_stick.direction.z,
        state.cue_stick.elevation,
        state.cue_stick.tip_offset,
    ] {
        bytes.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    bytes.extend_from_slice(&state.elapsed_time.to_bits().to_le_bytes());
    bytes
}

/// The hash of a complete game state.
///
/// The balls are hashed in index order, and each ball contributes its position, its
/// velocity, its angular velocity and its discrete state.
pub fn hash_state(state: &GameState) -> u64 {
    let mut hasher = Hasher::new();
    hasher.write_bytes(&canonical_bytes(state));
    hasher.finish()
}

/// The hash of a game state, as a lower-case hex string.
pub fn hash_state_hex(state: &GameState) -> String {
    format!("{:016x}", hash_state(state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TableConfig;
    use crate::state::BallState;

    #[test]
    fn the_hash_reacts_to_a_single_bit() {
        let cfg = TableConfig::default();
        let mut state = GameState::new(vec![BallState::at_rest(
            &cfg,
            0,
            crate::math::Vec3::new(0.5, 0.5, cfg.ball.radius),
        )]);
        let a = hash_state(&state);
        state.balls[0].position.x = f64::from_bits(state.balls[0].position.x.to_bits() + 1);
        let b = hash_state(&state);
        assert_ne!(a, b);
    }

    #[test]
    fn the_hash_is_stable() {
        let cfg = TableConfig::default();
        let state = GameState::new(vec![BallState::at_rest(
            &cfg,
            0,
            crate::math::Vec3::new(0.5, 0.5, cfg.ball.radius),
        )]);
        assert_eq!(hash_state(&state), hash_state(&state));
        assert_eq!(hash_state_hex(&state).len(), 16);
    }
}
