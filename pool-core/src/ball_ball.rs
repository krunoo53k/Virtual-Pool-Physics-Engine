//! Ball-ball collisions: the impulsive model of §4.4.5 and its time of impact
//! (§11.3).

use crate::config::TableConfig;
use crate::math::Vec3;
use crate::quartic;
use crate::state::BallState;

/// The outcome of a ball-ball collision.
#[derive(Clone, Copy, Debug)]
pub struct BallBallOutcome {
    /// The cue/striking ball's velocity after the collision.
    pub velocity_a: Vec3,
    /// The object ball's velocity after the collision.
    pub velocity_b: Vec3,
    /// The striking ball's angular velocity after the collision.
    pub angular_velocity_a: Vec3,
    /// The object ball's angular velocity after the collision.
    pub angular_velocity_b: Vec3,
    /// The normal impulse.
    pub jn: f64,
    /// The tangential impulse (signed: it opposes the relative slip).
    pub jt: f64,
    /// The relative tangential slip at the contact point.
    pub slip: f64,
    /// True when the contact sticks (the tangential impulse is not Coulomb limited).
    pub sticking: bool,
    /// The line of centres.
    pub normal: Vec3,
    /// The horizontal tangent used by the tangential impulse.
    pub tangent: Vec3,
    /// The vertical speed the striking ball gains from the table effect (§4.5).
    pub lift: f64,
}

/// Resolves a ball-ball collision between `a` (the striking ball) and `b`.
///
/// The model is the one of §4.4.5:
/// - the normal impulse is `J_n = ((1 + e) / 2) * m * v_n`, which conserves momentum;
/// - the tangential impulse stops the slip at `m * u_t / 7` but is capped by the
///   Coulomb limit `mu_ball * J_n`;
/// - both balls' spin is updated, which is what produces throw and spin transfer.
pub fn resolve(a: &BallState, b: &BallState, cfg: &TableConfig) -> BallBallOutcome {
    let m = cfg.ball.mass;
    let radius = cfg.ball.radius;
    let inertia = cfg.moment_of_inertia();
    let e = cfg.ball_ball.e_ball;
    let mu = cfg.ball_ball.mu_ball;

    let delta = b.position - a.position;
    // The line of centres is taken in the horizontal plane: the document's ball-ball
    // model is a planar one, and the vertical effect of the collision is the table
    // effect of §4.5, not a vertical component of the contact impulse.
    let horizontal = Vec3::new(delta.x, delta.y, 0.0);
    let normal = match horizontal.normalize() {
        Some(n) => n,
        None => match delta.normalize() {
            Some(n) => n,
            None => {
                // Degenerate: the centres coincide. Leave the balls alone (§4.5,
                // ground rule 6: never divide by zero, never produce a NaN).
                return BallBallOutcome {
                    velocity_a: a.velocity,
                    velocity_b: b.velocity,
                    angular_velocity_a: a.angular_velocity,
                    angular_velocity_b: b.angular_velocity,
                    jn: 0.0,
                    jt: 0.0,
                    slip: 0.0,
                    sticking: true,
                    normal: Vec3::X,
                    tangent: Vec3::Y,
                    lift: 0.0,
                };
            }
        },
    };
    // The tangent lies in the horizontal plane; the vertical component of the contact
    // slip is handled by the table effect of §4.5 rather than by this impulse.
    let tangent = Vec3::new(-normal.y, normal.x, 0.0);

    let r_a = normal * radius;
    let r_b = normal * -radius;

    // Relative velocity along the line of centres, positive while A approaches B.
    let v_n = (a.velocity - b.velocity).dot(normal);
    // The slip of the two contact points along the tangent.
    let contact_a = a.velocity + a.angular_velocity.cross(r_a);
    let contact_b = b.velocity + b.angular_velocity.cross(r_b);
    let u_t = (contact_a - contact_b).dot(tangent);

    let jn = ((1.0 + e) / 2.0) * m * v_n;
    let jt_stick = m * u_t.abs() / 7.0;
    let jt_cap = mu * jn;
    // The friction always opposes the relative slip, so the impulse carries the sign
    // of the slip.
    let jt = if jt_stick <= jt_cap { jt_stick } else { jt_cap };
    let jt = if u_t >= 0.0 { jt } else { -jt };

    let impulse_a = normal * (-jn) + tangent * (-jt);
    let impulse_b = normal * jn + tangent * jt;

    let velocity_a = a.velocity + impulse_a / m;
    let velocity_b = b.velocity + impulse_b / m;
    let angular_velocity_a = a.angular_velocity + r_a.cross(impulse_a) / inertia;
    let angular_velocity_b = b.angular_velocity + r_b.cross(impulse_b) / inertia;

    // The table effect of §4.5: the friction between the balls can lift the
    // striking ball off the cloth, `dv_z = mu_ball * U_i`.
    let lift = if v_n > 0.0 { mu * v_n } else { 0.0 };

    BallBallOutcome {
        velocity_a,
        velocity_b,
        angular_velocity_a,
        angular_velocity_b,
        jn,
        jt,
        slip: u_t,
        sticking: jt_stick.abs() <= jt_cap.abs(),
        normal,
        tangent,
        lift,
    }
}

/// The time of impact of two balls whose trajectories are quadratic over the
/// segment (§11.3).
///
/// The contact condition `|dp0 + dv t + 0.5 da t^2|^2 = (2R)^2` is a quartic in
/// `t`; the smallest positive root is the impact time.
///
/// Two balls that have just collided are still exactly `2R` apart, so the solver
/// has to tell a *new* collision from the contact it has just resolved: a ball that
/// is still touching but already separating is not a new collision.
pub fn time_of_impact(
    a: &BallState,
    b: &BallState,
    accel_a: Vec3,
    accel_b: Vec3,
    horizon: f64,
) -> Option<f64> {
    let radius = a.radius;
    // The contact test is planar, like the collision model of §4.4.5: the vertical
    // separation of two balls is small compared with their diameter, and the
    // vertical effect of a collision is the table effect of §4.5.
    let dp0 = Vec3::new(
        b.position.x - a.position.x,
        b.position.y - a.position.y,
        0.0,
    );
    let dv = Vec3::new(
        b.velocity.x - a.velocity.x,
        b.velocity.y - a.velocity.y,
        0.0,
    );
    let da = Vec3::new(accel_b.x - accel_a.x, accel_b.y - accel_a.y, 0.0);

    let dist2 = dp0.norm_squared();
    let contact2 = (2.0 * radius) * (2.0 * radius);
    // The distance within which two balls count as touching, and the closing speed
    // below which a contact is just numerical noise.
    let touch_tol = 1e-12;
    let touch_tol2 = (2.0 * radius + touch_tol) * (2.0 * radius + touch_tol);
    let closing_tol = 1e-9;

    if dist2 <= touch_tol2 {
        // The balls are touching (or a hair past it). A new collision needs them to
        // be closing on each other; two balls that are resting against each other
        // are not colliding again.
        let normal = match dp0.normalize() {
            Some(n) => n,
            None => return Some(0.0),
        };
        // `closing` is positive while the balls move apart.
        let closing = dv.dot(normal);
        if closing >= -closing_tol {
            return None;
        }
        // They are in contact and closing: the collision is immediate.
        return Some(0.0);
    }

    let mut constant = dist2 - contact2;
    // Snap the constant term to zero when the balls are touching, so that the
    // rounding noise of the positions cannot manufacture a root at `t = 0`.
    if constant.abs() <= touch_tol * (4.0 * radius) {
        constant = 0.0;
    }

    let coeffs = [
        0.25 * da.norm_squared(),
        da.dot(dv),
        dv.norm_squared() + da.dot(dp0),
        2.0 * dv.dot(dp0),
        constant,
    ];

    quartic::first_root_after(&coeffs, 0.0, horizon).filter(|t| *t > crate::math::MIN_IMPACT_TIME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TableConfig;
    use crate::math::Vec3;

    fn ball(cfg: &TableConfig, number: u8, pos: Vec3, vel: Vec3) -> BallState {
        let mut b = BallState::at_rest(cfg, number, pos);
        b.velocity = vel;
        b
    }

    #[test]
    fn head_on_collision_conserves_momentum() {
        let cfg = TableConfig::default();
        let a = ball(&cfg, 0, Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0));
        let b = ball(&cfg, 1, Vec3::new(0.3, 0.0, 0.0), Vec3::ZERO);
        let out = resolve(&a, &b, &cfg);
        // The cue ball keeps ((1 - e) / 2) * v and the object ball takes
        // ((1 + e) / 2) * v.
        assert!((out.velocity_a.x - 0.050000000000000044).abs() < 1e-12);
        assert!((out.velocity_b.x - 1.95).abs() < 1e-12);
        let p_before = 0.170 * 2.0;
        let p_after = 0.170 * (out.velocity_a.x + out.velocity_b.x);
        assert!((p_before - p_after).abs() < 1e-12);
    }

    #[test]
    fn thirty_degree_cut_reproduces_the_fixture() {
        let cfg = TableConfig::default();
        let angle = 30.0_f64.to_radians();
        let a = ball(&cfg, 0, Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0));
        let b = ball(
            &cfg,
            1,
            Vec3::new(0.3 * angle.cos(), 0.3 * angle.sin(), 0.0),
            Vec3::ZERO,
        );
        let out = resolve(&a, &b, &cfg);
        let sep = (out.velocity_b.y.atan2(out.velocity_b.x)
            - out.velocity_a.y.atan2(out.velocity_a.x))
        .to_degrees();
        assert!((sep - 84.42982481306461).abs() < 1e-9);
        let throw = (out.velocity_b.y.atan2(out.velocity_b.x) - angle).to_degrees();
        assert!((throw - -2.8624052261117545).abs() < 1e-9);
        assert!((out.velocity_a.norm() - 0.9165859118282551).abs() < 1e-12);
        assert!((out.velocity_b.norm() - 1.6908591566124014).abs() < 1e-12);
    }

    #[test]
    fn ten_degree_cut_is_in_the_sticking_regime() {
        let cfg = TableConfig::default();
        let angle = 10.0_f64.to_radians();
        let a = ball(&cfg, 0, Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0));
        let b = ball(
            &cfg,
            1,
            Vec3::new(0.3 * angle.cos(), 0.3 * angle.sin(), 0.0),
            Vec3::ZERO,
        );
        let out = resolve(&a, &b, &cfg);
        assert!(out.sticking);
        let throw = (out.velocity_b.y.atan2(out.velocity_b.x) - angle).to_degrees();
        assert!((throw - -1.4799).abs() < 1e-3);
    }
}
