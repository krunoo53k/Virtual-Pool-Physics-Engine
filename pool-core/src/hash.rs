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

/// The hash of a complete game state.
///
/// The balls are hashed in index order, and each ball contributes its position, its
/// velocity, its angular velocity and its discrete state.
pub fn hash_state(state: &GameState) -> u64 {
    let mut hasher = Hasher::new();
    hasher.write_u64(state.balls.len() as u64);
    for ball in &state.balls {
        hasher.write_f64(ball.position.x);
        hasher.write_f64(ball.position.y);
        hasher.write_f64(ball.position.z);
        hasher.write_f64(ball.velocity.x);
        hasher.write_f64(ball.velocity.y);
        hasher.write_f64(ball.velocity.z);
        hasher.write_f64(ball.angular_velocity.x);
        hasher.write_f64(ball.angular_velocity.y);
        hasher.write_f64(ball.angular_velocity.z);
        hasher.write_u8(ball.motion_state as u8);
        hasher.write_u8(ball.pocket_state as u8);
        hasher.write_u8(ball.number);
        hasher.write_u8(u8::from(ball.is_pocketed));
        hasher.write_u8(u8::from(ball.is_off_table));
    }
    hasher.write_f64(state.elapsed_time);
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
