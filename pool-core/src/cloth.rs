//! Ball-cloth interactions: the closed-form motion states of §3.
//!
//! Nothing here integrates anything. Between two events a ball's position,
//! velocity and angular velocity are given by closed-form expressions, and this
//! module only ever evaluates those expressions (§11.2).

use crate::math::Vec3;
use crate::state::{BallState, MotionState};

/// The parameters of the cloth model.
#[derive(Clone, Copy, Debug)]
pub struct ClothParams {
    /// Sliding friction, `mu_s`.
    pub mu_s: f64,
    /// Rolling friction, `mu_r`.
    pub mu_r: f64,
    /// Spin decay coefficient, `mu_sp`.
    pub mu_sp: f64,
    /// Gravity, m/s^2.
    pub g: f64,
    /// Ball radius, m.
    pub radius: f64,
}

/// Below this magnitude a velocity or an angular velocity counts as zero.
pub const VELOCITY_EPS: f64 = 1e-9;

/// The relative velocity at the point of contact, `u = v + R (k x w)` (§3.1).
pub fn slip_velocity(velocity: Vec3, angular_velocity: Vec3, radius: f64) -> Vec3 {
    velocity + Vec3::Z.cross(angular_velocity) * radius
}

/// True when the ball rolls without slipping (`u = 0`).
pub fn is_rolling(velocity: Vec3, angular_velocity: Vec3, radius: f64) -> bool {
    slip_velocity(velocity, angular_velocity, radius).norm_squared() <= VELOCITY_EPS * VELOCITY_EPS
}

/// The in-plane angular velocity of a rolling ball, `k x v / R` (§3.2).
pub fn rolling_angular_velocity(velocity: Vec3, radius: f64) -> Vec3 {
    Vec3::Z.cross(velocity) / radius
}

/// The time a sliding ball needs to reach the rolling condition,
/// `t = 2 u0 / (7 mu_s g)` (§3.3).
pub fn time_to_rolling(u0: f64, p: &ClothParams) -> f64 {
    if p.mu_s <= 0.0 {
        return f64::INFINITY;
    }
    2.0 * u0 / (7.0 * p.mu_s * p.g)
}

/// The time a rolling ball needs to stop, `|v0| / (mu_r g)` (§3.2).
pub fn time_to_rest(speed: f64, p: &ClothParams) -> f64 {
    if p.mu_r <= 0.0 {
        return f64::INFINITY;
    }
    speed / (p.mu_r * p.g)
}

/// The time a spinning ball needs to stop, `2 R |w0| / (5 mu_sp g)` (§3.4).
pub fn time_to_rest_spinning(omega_z: f64, p: &ClothParams) -> f64 {
    if p.mu_sp <= 0.0 {
        return f64::INFINITY;
    }
    2.0 * p.radius * omega_z.abs() / (5.0 * p.mu_sp * p.g)
}

/// The time an airborne ball needs to come back down to the bed (§7.1).
pub fn time_to_land(z: f64, v_z: f64, bed_height: f64, g: f64) -> f64 {
    let disc = v_z * v_z + 2.0 * g * (z - bed_height);
    if disc < 0.0 {
        return f64::INFINITY;
    }
    (v_z + disc.sqrt()) / g
}

/// Advances a sliding ball by `dt` (§3.3).
///
/// The slip direction is constant while the ball slides on a flat bed, so the
/// acceleration is constant and the trajectory is a parabola.
pub fn advance_sliding(ball: &mut BallState, dt: f64, p: &ClothParams) {
    let u = slip_velocity(ball.velocity, ball.angular_velocity, p.radius);
    let u_len = u.norm();
    if u_len <= VELOCITY_EPS {
        return;
    }
    let u_hat = u / u_len;
    let accel = u_hat * (-p.mu_s * p.g);
    ball.position += ball.velocity * dt + accel * (0.5 * dt * dt);
    ball.velocity += accel * dt;
    // The friction torque spins the ball up towards forward roll. The sign is the
    // one derived from the torque balance in §3.3.
    let spin_up = Vec3::Z.cross(u_hat) * (5.0 * p.mu_s * p.g / (2.0 * p.radius));
    ball.angular_velocity += spin_up * dt;
    decay_spin(ball, dt, p);
}

/// Advances a rolling ball by `dt` (§3.2).
pub fn advance_rolling(ball: &mut BallState, dt: f64, p: &ClothParams) {
    let speed = libm::hypot(ball.velocity.x, ball.velocity.y);
    if speed <= VELOCITY_EPS {
        return;
    }
    let v_hat = Vec3::new(ball.velocity.x / speed, ball.velocity.y / speed, 0.0);
    let accel = v_hat * (-p.mu_r * p.g);
    ball.position += ball.velocity * dt + accel * (0.5 * dt * dt);
    ball.velocity += accel * dt;
    let w = rolling_angular_velocity(ball.velocity, p.radius);
    ball.angular_velocity.x = w.x;
    ball.angular_velocity.y = w.y;
    decay_spin(ball, dt, p);
}

/// Advances a spinning ball by `dt` (§3.4).
pub fn advance_spinning(ball: &mut BallState, dt: f64, p: &ClothParams) {
    decay_spin(ball, dt, p);
}

/// The rate at which the vertical spin decays, rad/s^2: `5 mu_sp g / (2R)`.
pub fn spin_decay_rate(p: &ClothParams) -> f64 {
    5.0 * p.mu_sp * p.g / (2.0 * p.radius)
}

/// Decays the vertical spin: `w_z(t) = w0 - (5 mu_sp g / 2R) t` (§3.2, §3.4).
///
/// The spin decays *towards* zero and stops there. A decay that simply
/// subtracted would go on past zero, and a backspin would then grow instead
/// of dying - and a ball whose spin never died would never come to rest.
pub fn decay_spin(ball: &mut BallState, dt: f64, p: &ClothParams) {
    let rate = spin_decay_rate(p) * dt;
    if ball.angular_velocity.z > 0.0 {
        ball.angular_velocity.z = (ball.angular_velocity.z - rate).max(0.0);
    } else if ball.angular_velocity.z < 0.0 {
        ball.angular_velocity.z = (ball.angular_velocity.z + rate).min(0.0);
    }
}

/// Advances an airborne ball by `dt` under gravity (§7.1).
///
/// Air drag and the Magnus effect are omitted: both are negligible for pool ball
/// sizes and speeds (§7.1, §13.4).
pub fn advance_airborne(ball: &mut BallState, dt: f64, g: f64) {
    let accel = Vec3::new(0.0, 0.0, -g);
    ball.position += ball.velocity * dt + accel * (0.5 * dt * dt);
    ball.velocity += accel * dt;
}

/// The constant acceleration of a ball over the current segment.
///
/// The event loop needs this to build the quadratic trajectories used by the
/// time-of-impact solvers (§11.3).
pub fn segment_acceleration(ball: &BallState, p: &ClothParams) -> Vec3 {
    match ball.motion_state {
        MotionState::Sliding => {
            let u = slip_velocity(ball.velocity, ball.angular_velocity, p.radius);
            let u_len = u.norm();
            if u_len <= VELOCITY_EPS {
                Vec3::ZERO
            } else {
                (u / u_len) * (-p.mu_s * p.g)
            }
        }
        MotionState::Rolling => {
            let speed = libm::hypot(ball.velocity.x, ball.velocity.y);
            if speed <= VELOCITY_EPS {
                Vec3::ZERO
            } else {
                let v_hat = Vec3::new(ball.velocity.x / speed, ball.velocity.y / speed, 0.0);
                v_hat * (-p.mu_r * p.g)
            }
        }
        MotionState::Spinning | MotionState::Stationary => Vec3::ZERO,
        MotionState::Airborne => Vec3::new(0.0, 0.0, -p.g),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TableConfig;

    fn params(cfg: &TableConfig) -> ClothParams {
        ClothParams {
            mu_s: cfg.cloth.mu_s,
            mu_r: cfg.cloth.mu_r,
            mu_sp: cfg.cloth.mu_sp,
            g: cfg.simulation.gravity,
            radius: cfg.ball.radius,
        }
    }

    #[test]
    fn sliding_ball_reaches_the_rolling_condition_exactly() {
        let cfg = TableConfig::default();
        let p = params(&cfg);
        let mut ball = BallState::at_rest(&cfg, 0, Vec3::ZERO);
        ball.velocity = Vec3::new(1.0, 0.0, 0.0);
        ball.motion_state = MotionState::Sliding;

        let u0 = slip_velocity(ball.velocity, ball.angular_velocity, p.radius).norm();
        let t = time_to_rolling(u0, &p);
        assert!((t - 0.14562399883500798).abs() < 1e-15);

        advance_sliding(&mut ball, t, &p);

        // The velocity has decayed by mu_s * g * t.
        assert!((ball.velocity.x - 0.7142857142857143).abs() < 1e-12);
        // The spin is positive: the ball spins up towards forward roll (§3.3).
        assert!((ball.angular_velocity.y - 24.996875390576175).abs() < 1e-9);
        assert!(ball.angular_velocity.y > 0.0);
        // The rolling condition holds: omega_y = v / R.
        assert!((ball.angular_velocity.y - ball.velocity.x / p.radius).abs() < 1e-9);
        // The displacement follows the closed form.
        let expected_x = 1.0 * t - 0.5 * p.mu_s * p.g * t * t;
        assert!((ball.position.x - expected_x).abs() < 1e-12);
        assert!((expected_x - 0.12482057043000684).abs() < 1e-12);
    }

    #[test]
    fn rolling_ball_stops_after_the_analytic_time() {
        let cfg = TableConfig::default();
        let p = params(&cfg);
        let mut ball = BallState::at_rest(&cfg, 0, Vec3::ZERO);
        ball.velocity = Vec3::new(2.0, 0.0, 0.0);
        ball.angular_velocity = rolling_angular_velocity(ball.velocity, p.radius);
        ball.motion_state = MotionState::Rolling;
        let t = time_to_rest(2.0, &p);
        assert!((t - 2.0 / (0.01 * 9.81)).abs() < 1e-15);
        advance_rolling(&mut ball, t, &p);
        assert!(ball.velocity.x.abs() < 1e-12);
        assert!(ball.angular_velocity.x.abs() < 1e-9);
        assert!(ball.angular_velocity.y.abs() < 1e-9);
    }
}
