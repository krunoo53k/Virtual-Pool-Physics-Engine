//! Ball-cushion collisions: the Han (2005) impulse model of §5.1 and the three rail
//! regimes of §11.3.
//!
//! The contact height is *derived* from the specified nose height: `eps = h_nose - R`
//! and `theta = asin(eps / R)` (§5.1). Nothing here stores a tuned epsilon.

use crate::config::TableConfig;
use crate::geometry::{CushionGeometry, RailGeometry};
use crate::math::Vec3;
use crate::quartic;
use crate::state::BallState;

/// The outcome of a cushion impact.
#[derive(Clone, Copy, Debug)]
pub struct CushionOutcome {
    /// The ball's velocity after the impact.
    pub velocity: Vec3,
    /// The ball's angular velocity after the impact.
    pub angular_velocity: Vec3,
    /// The normal impulse.
    pub jn: f64,
    /// The tangential impulse.
    pub jt: f64,
    /// The relative slip at the contact point.
    pub slip: f64,
    /// True when the contact sticks (the tangential impulse is not Coulomb limited).
    pub sticking: bool,
    /// The contact normal used.
    pub normal: Vec3,
}

/// Which surface of the rail the ball met (§11.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailRegime {
    /// The nose rubber: the ball's lowest point is at or below the nose height.
    NoseRubber,
    /// The rail structure above the nose rubber.
    RailStructure,
    /// The ball clears the rail entirely.
    ClearOfRail,
}

/// The Han impulse model in the rail's local frame (§5.1).
///
/// The local frame has `x` towards the rail, `y` along the rail and `z` up. The
/// contact face is inclined `theta` from the vertical, so the contact normal is
/// tilted by `theta` from the horizontal.
///
/// The sticking branch is written out component-wise in the document; the general
/// form below is the same thing and is used for the rail structure as well (with
/// `theta = 0`, i.e. a vertical wall).
pub fn resolve_cushion(
    ball: &BallState,
    rail_normal: Vec3,
    rail_tangent: Vec3,
    sin_theta: f64,
    cos_theta: f64,
    e: f64,
    mu: f64,
    cfg: &TableConfig,
) -> CushionOutcome {
    let m = cfg.ball.mass;
    let radius = cfg.ball.radius;
    let inertia = cfg.moment_of_inertia();

    // The contact point relative to the ball's centre, the contact tangent and the
    // contact normal.
    let contact_offset = rail_normal * (radius * cos_theta) + Vec3::Z * (radius * sin_theta);
    let tangent = rail_normal * sin_theta - Vec3::Z * cos_theta;
    let normal = rail_normal * -cos_theta - Vec3::Z * sin_theta;

    // The slip of the contact point over the (stationary) cushion.
    let contact_velocity = ball.velocity + ball.angular_velocity.cross(contact_offset);
    let s_x = contact_velocity.dot(tangent);
    let s_y = contact_velocity.dot(rail_tangent);
    let slip = (s_x * s_x + s_y * s_y).sqrt();

    // The normal approach speed, positive while the ball heads into the rail.
    let v_n = -ball.velocity.dot(normal);
    if v_n <= 0.0 || slip <= 0.0 {
        return CushionOutcome {
            velocity: ball.velocity,
            angular_velocity: ball.angular_velocity,
            jn: 0.0,
            jt: 0.0,
            slip,
            sticking: true,
            normal,
        };
    }

    let jn = (1.0 + e) * m * v_n;
    let jt_stick = (2.0 / 7.0) * m * slip;
    let jt_cap = mu * jn;
    let jt = if jt_stick <= jt_cap { jt_stick } else { jt_cap };

    // The slip direction; the friction impulse always opposes the slip.
    let slip_dir = tangent * (s_x / slip) + rail_tangent * (s_y / slip);
    let impulse = normal * jn - slip_dir * jt;

    CushionOutcome {
        velocity: ball.velocity + impulse / m,
        angular_velocity: ball.angular_velocity + contact_offset.cross(impulse) / inertia,
        jn,
        jt,
        slip,
        sticking: jt_stick <= jt_cap,
        normal,
    }
}

/// The cushion impact against the nose rubber, using the geometry derived from the
/// table configuration.
pub fn resolve_nose_cushion(
    ball: &BallState,
    rail_normal: Vec3,
    rail_tangent: Vec3,
    cfg: &TableConfig,
    geom: &CushionGeometry,
) -> CushionOutcome {
    resolve_cushion(
        ball,
        rail_normal,
        rail_tangent,
        geom.sin_theta,
        geom.cos_theta,
        cfg.cushion.e_cushion,
        cfg.cushion.mu_cushion,
        cfg,
    )
}

/// The regime a ball at height `z` meets (§11.3, and the `rail_regimes` block of the
/// `cushion_rebound` fixture).
pub fn regime_for_height(z: f64, radius: f64, cfg: &TableConfig) -> RailRegime {
    let lowest = z - radius;
    if lowest > cfg.cushion.rail_top_height {
        RailRegime::ClearOfRail
    } else if lowest > cfg.cushion.nose_height {
        RailRegime::RailStructure
    } else {
        RailRegime::NoseRubber
    }
}

/// The time of impact of a ball with a rail.
///
/// The rail is made of three surfaces (§11.3):
/// 1. the nose rubber: an inclined plane, so the contact condition is
///    `x(t) = x_block(z(t))` with `x_block` linear in `z`;
/// 2. the rail structure: a vertical face at the nose line;
/// 3. above the rail top the ball sails over the table.
///
/// Both candidate conditions are quadratic in `t` (the ball's horizontal position
/// is quadratic because of the cloth friction, and so is its height when it is
/// airborne), so the impact time is found with the shared root finder.
pub fn time_of_impact(
    ball: &BallState,
    accel: Vec3,
    rail: &RailGeometry,
    geom: &CushionGeometry,
    cfg: &TableConfig,
    horizon: f64,
) -> Option<(f64, RailRegime)> {
    let normal = Vec3::new(rail.normal.x, rail.normal.y, 0.0);
    let radius = ball.radius;

    let x0 = ball.position.x * normal.x + ball.position.y * normal.y;
    let vx = ball.velocity.x * normal.x + ball.velocity.y * normal.y;
    let ax = accel.x * normal.x + accel.y * normal.y;
    let z0 = ball.position.z;
    let vz = ball.velocity.z;
    let az = accel.z;

    let z_of_t = |t: f64| z0 + vz * t + 0.5 * az * t * t;

    let h_nose = cfg.cushion.nose_height;

    // Branch 1: the inclined nose rubber face. The contact surface is the plane
    //
    //   x_block(z) = line_coord - (R + (z - h_nose) * sin(theta)) / cos(theta)
    //
    // and the contact condition `x(t) = x_block(z(t))` is quadratic in `t`.
    let k = geom.sin_theta / geom.cos_theta;
    let branch_nose = (
        0.5 * (ax + az * k),
        vx + vz * k,
        x0 - rail.line_coord + radius / geom.cos_theta + (z0 - h_nose) * k,
    );

    // Branch 2: the vertical face of the rail structure, at the nose line.
    let branch_structure = (0.5 * ax, vx, x0 - (rail.line_coord - radius));

    let mut best: Option<(f64, RailRegime)> = None;

    for (branch, is_nose) in [(branch_nose, true), (branch_structure, false)] {
        let (quad, lin, const_term) = branch;
        let coeffs = [quad, lin, const_term];
        let roots = quartic::quadratic_roots_abc(coeffs[0], coeffs[1], coeffs[2]);
        for t in roots {
            if !(t > crate::math::MIN_IMPACT_TIME && t <= horizon) {
                continue;
            }
            // The crossing must enter the obstacle: the condition goes from negative
            // (clear of the face) to zero (in contact), i.e. it is increasing.
            let derivative = 2.0 * quad * t + lin;
            if derivative <= 0.0 {
                continue;
            }
            let z = z_of_t(t);
            let regime = regime_for_height(z, radius, cfg);
            // The nose rubber is the inclined face and the rail structure is the
            // vertical one; the rubber model is never extrapolated upwards (§11.3).
            let valid = matches!(
                (regime, is_nose),
                (RailRegime::NoseRubber, true) | (RailRegime::RailStructure, false)
            );
            if !valid {
                continue;
            }
            // The contact has to be on the face, not on the extension of the plane
            // below the nose.
            if is_nose && z + radius * geom.sin_theta < h_nose - 1e-12 {
                continue;
            }
            let candidate = (t, regime);
            best = match best {
                Some((bt, _)) if bt <= t => best,
                _ => Some(candidate),
            };
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TableConfig;
    use crate::geometry::TableGeometry;
    use crate::math::Vec3;

    #[test]
    fn square_hit_at_the_spec_height_presses_the_ball_into_the_slate() {
        let cfg = TableConfig::default();
        let geom = TableGeometry::new(&cfg);
        let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(0.5, 0.5, cfg.ball.radius));
        ball.velocity = Vec3::new(2.0, 0.0, 0.0);
        ball.angular_velocity = Vec3::new(0.0, 2.0 / cfg.ball.radius, 0.0);
        let out = resolve_nose_cushion(&ball, Vec3::X, Vec3::Y, &cfg, &geom.cushion);
        assert!(
            (out.velocity.z - -0.4567736244323732).abs() < 1e-12,
            "vz = {}",
            out.velocity.z
        );
        assert!(out.velocity.z < 0.0);
    }

    #[test]
    fn low_nose_launches_the_ball_upward() {
        let cfg = TableConfig::default();
        let geom = TableGeometry::new(&cfg);
        // 0.500 D: the nose sits at the ball's centre, so eps = 0 and theta = 0.
        let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(0.5, 0.5, cfg.ball.radius));
        ball.velocity = Vec3::new(2.0, 0.0, 0.0);
        ball.angular_velocity = Vec3::new(0.0, 2.0 / cfg.ball.radius, 0.0);
        let out = resolve_cushion(&ball, Vec3::X, Vec3::Y, 0.0, 1.0, 0.75, 0.14, &cfg);
        assert!(
            (out.velocity.z - 0.49).abs() < 1e-12,
            "vz = {}",
            out.velocity.z
        );
    }

    #[test]
    fn rebound_angle_does_not_depend_on_the_incident_speed() {
        let cfg = TableConfig::default();
        let geom = TableGeometry::new(&cfg);
        let _ = &geom;
        let mut angles = Vec::new();
        for speed in [1.0, 2.0, 4.0] {
            let inc = 30.0_f64.to_radians();
            let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(0.5, 0.5, cfg.ball.radius));
            ball.velocity = Vec3::new(speed * inc.cos(), speed * inc.sin(), 0.0);
            ball.angular_velocity = Vec3::new(
                -ball.velocity.y / cfg.ball.radius,
                ball.velocity.x / cfg.ball.radius,
                0.0,
            );
            let out = resolve_nose_cushion(&ball, Vec3::X, Vec3::Y, &cfg, &geom.cushion);
            let angle = out.velocity.y.atan2(-out.velocity.x).to_degrees();
            angles.push(angle);
        }
        assert!((angles[0] - angles[1]).abs() < 1e-9);
        assert!((angles[1] - angles[2]).abs() < 1e-9);
    }
}
