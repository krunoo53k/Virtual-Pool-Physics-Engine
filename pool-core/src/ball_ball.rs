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
    /// The tangential impulse (a magnitude: it acts along the slip).
    pub jt: f64,
    /// The relative slip speed at the contact point.
    pub slip: f64,
    /// True when the contact sticks (the tangential impulse is not Coulomb limited).
    pub sticking: bool,
    /// The line of centres.
    pub normal: Vec3,
    /// The horizontal tangent of the tangent plane.
    pub tangent: Vec3,
    /// The vertical speed the striking ball gains from the impulse, m/s. It is
    /// positive when the ball is lifted.
    ///
    /// For a contact between balls at the same height this is the friction-only lift
    /// of §4.5: the normal is horizontal, so `impulse_a.z` carries only the tangential
    /// impulse. At different heights the normal has a vertical component as well, and
    /// this is the vertical speed from the whole impulse, normal and friction together.
    pub lift: f64,
}

/// The friction coefficient between two balls, as a function of the sliding speed at
/// the contact.
///
/// Alciatore's TP A.14 fits Marlow's data with
///
/// ```text
/// mu(v_rel) = 0.009951 + 0.108 * exp(-0.77 * v_rel)
/// ```
///
/// so the friction between the balls *falls* as the contact slides faster. That is why
/// the measured throw is larger on a slow shot than on a fast one (section 4.4.5).
///
/// The configuration's `mu_ball` is the coefficient at the reference slip speed of
/// 1 m/s, so the law is normalised there: at that speed the model uses `mu_ball`
/// itself, and the *shape* of the curve is TP A.14's. The default `mu_ball` is the
/// fit's own value at that speed (0.05996, used as 0.06), so the normalisation is
/// the rounding of the config value and the law in force is TP A.14's, unscaled.
pub fn friction_at_speed(mu_ball: f64, slip: f64) -> f64 {
    /// TP A.14's fit: `mu(v) = 0.009951 + 0.108 * exp(-0.77 * v)`.
    const A: f64 = 0.009951;
    const B: f64 = 0.108;
    const K: f64 = 0.77;
    /// `exp(-0.77)`: the exponential factor at the reference slip speed.
    const EXP_REFERENCE: f64 = 0.46301306831122807;
    /// The fit at the reference slip speed of 1 m/s.
    const REFERENCE_MU: f64 = A + B * EXP_REFERENCE;
    let slip = if slip > 0.0 { slip } else { 0.0 };
    mu_ball * (A + B * libm::exp(-K * slip)) / REFERENCE_MU
}

/// Resolves a ball-ball collision between `a` (the striking ball) and `b`.
///
/// The model is the one of §4.4.5:
/// - the line of centres is the full 3D one, so a ball in flight above another can
///   pass over it (two balls only touch when their centres are `2R` apart in 3D);
/// - the normal impulse is `J_n = ((1 + e) / 2) * m * v_n`, which conserves momentum;
/// - the tangential impulse stops the slip at `m * u / 7` but is capped by the
///   Coulomb limit `mu_ball(u) * J_n`, with the friction coefficient falling with the
///   sliding speed (TP A.14);
/// - the slip is a vector in the *tangent plane* of that 3D normal - horizontal along
///   the rail and vertical when both balls lie on the bed - and the friction acts
///   along it, which is what carries the vertical effects of §4.5;
/// - both balls' spin is updated, which is what produces throw and spin transfer.
pub fn resolve(a: &BallState, b: &BallState, cfg: &TableConfig) -> BallBallOutcome {
    let m = cfg.ball.mass;
    let radius = cfg.ball.radius;
    let inertia = cfg.moment_of_inertia();
    let e = cfg.ball_ball.e_ball;

    let delta = b.position - a.position;
    // Degenerate: the centres coincide. Leave the balls alone (§4.5, ground rule 6:
    // never divide by zero, never produce a NaN).
    let untouched = || BallBallOutcome {
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
    // The line of centres is the full 3D one. A ball in flight above another passes
    // over it: the two only touch when the *3D* separation is 2R.
    //
    // When the centres are at the same height the bed-level arithmetic is kept
    // exactly as it was. Besides being the physical model for the overwhelming
    // majority of shots, this keeps every on-the-cloth shot bit-for-bit the same.
    let (normal, tangent, bitangent) = if delta.z == 0.0 {
        let normal = match Vec3::new(delta.x, delta.y, 0.0).normalize() {
            Some(n) => n,
            None => return untouched(),
        };
        // The tangent plane is spanned by the horizontal tangent and the vertical.
        (normal, Vec3::new(-normal.y, normal.x, 0.0), Vec3::Z)
    } else {
        let normal = match delta.normalize() {
            Some(n) => n,
            None => return untouched(),
        };
        // Two orthonormal directions spanning the plane perpendicular to the
        // normal. The first is horizontal (it is the old tangent when the normal
        // is horizontal); the second is the "vertical" of that plane.
        let tangent = match Vec3::Z.cross(normal).normalize() {
            Some(t) => t,
            None => Vec3::X, // a vertical normal: any horizontal tangent will do
        };
        let bitangent = normal.cross(tangent);
        (normal, tangent, bitangent)
    };

    let r_a = normal * radius;
    let r_b = normal * -radius;

    // Relative velocity along the line of centres, positive while A approaches B.
    let v_n = (a.velocity - b.velocity).dot(normal);
    // The slip of the two contact points, in the tangent plane (§4.4.5).
    let contact_a = a.velocity + a.angular_velocity.cross(r_a);
    let contact_b = b.velocity + b.angular_velocity.cross(r_b);
    let slip_vector = contact_a - contact_b;
    let u_t = slip_vector.dot(tangent);
    // The second in-plane component: the vertical one when both balls lie on the
    // bed, and its tilted counterpart when one of them is in the air.
    let u_v = slip_vector.dot(bitangent);
    let slip = (u_t * u_t + u_v * u_v).sqrt();

    let jn = ((1.0 + e) / 2.0) * m * v_n;
    // The friction coefficient falls with the sliding speed (TP A.14).
    let mu = friction_at_speed(cfg.ball_ball.mu_ball, slip);
    let jt_stick = m * slip / 7.0;
    let jt_cap = mu * jn;
    let jt = if slip > 0.0 {
        jt_stick.min(jt_cap)
    } else {
        0.0
    };

    // The friction acts along the slip, in the tangent plane. The slip direction is
    // what carries the vertical part: for a rolling cue ball the contact point at the
    // front of the ball moves downwards, so the friction lifts the cue ball and presses
    // the object ball into the cloth (§4.5).
    let slip_dir = if slip > 0.0 {
        tangent * (u_t / slip) + bitangent * (u_v / slip)
    } else {
        Vec3::ZERO
    };

    let impulse_a = normal * (-jn) - slip_dir * jt;
    let impulse_b = normal * jn + slip_dir * jt;

    let velocity_a = a.velocity + impulse_a / m;
    let velocity_b = b.velocity + impulse_b / m;
    let angular_velocity_a = a.angular_velocity + r_a.cross(impulse_a) / inertia;
    let angular_velocity_b = b.angular_velocity + r_b.cross(impulse_b) / inertia;

    BallBallOutcome {
        velocity_a,
        velocity_b,
        angular_velocity_a,
        angular_velocity_b,
        jn,
        jt,
        slip,
        sticking: slip > 0.0 && jt_stick <= jt_cap,
        normal,
        tangent,
        // The vertical speed from the whole impulse, as documented on the field.
        lift: impulse_a.z / m,
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
    // The contact condition of §11.3 is the 3D sphere-sphere one: two balls touch
    // when their centres are 2R apart in space, height included. Dropping the z
    // term would make a ball flying above another collide with it.
    let dp0 = b.position - a.position;
    let dv = b.velocity - a.velocity;
    let da = accel_b - accel_a;

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
        assert!((sep - 83.80779436009911).abs() < 1e-9);
        let throw = (out.velocity_b.y.atan2(out.velocity_b.x) - angle).to_degrees();
        assert!((throw - -3.4336303624505273).abs() < 1e-9);
        assert!((out.velocity_a.norm() - 0.8997176254328026).abs() < 1e-12);
        assert!((out.velocity_b.norm() - 1.6917865556860299).abs() < 1e-12);
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

    #[test]
    fn a_ball_flying_above_another_does_not_touch_it() {
        let cfg = TableConfig::default();
        let r = cfg.ball.radius;
        // Directly over the other ball, 4R above it, moving horizontally. A planar
        // contact test sees the centres coincide and reports an instant collision.
        let a = ball(
            &cfg,
            0,
            Vec3::new(0.0, 0.0, r + 4.0 * r),
            Vec3::new(2.0, 0.0, 0.0),
        );
        let b = ball(&cfg, 1, Vec3::new(0.0, 0.0, r), Vec3::ZERO);
        assert!(time_of_impact(&a, &b, Vec3::ZERO, Vec3::ZERO, 1.0).is_none());
    }

    #[test]
    fn a_ball_falling_from_above_touches_it_in_three_dimensions() {
        let cfg = TableConfig::default();
        let r = cfg.ball.radius;
        // The centres are 3R apart and closing at 1 m/s: contact after R seconds.
        let a = ball(
            &cfg,
            0,
            Vec3::new(0.0, 0.0, r + 3.0 * r),
            Vec3::new(0.0, 0.0, -1.0),
        );
        let b = ball(&cfg, 1, Vec3::new(0.0, 0.0, r), Vec3::ZERO);
        let t = time_of_impact(&a, &b, Vec3::ZERO, Vec3::ZERO, 1.0).expect("the balls meet");
        assert!((t - r).abs() < 1e-9, "expected {r}, got {t}");
    }

    #[test]
    fn a_vertical_contact_resolves_along_the_three_dimensional_normal() {
        let cfg = TableConfig::default();
        let r = cfg.ball.radius;
        let a = ball(
            &cfg,
            0,
            Vec3::new(0.0, 0.0, r + 3.0 * r),
            Vec3::new(0.0, 0.0, -1.0),
        );
        let b = ball(&cfg, 1, Vec3::new(0.0, 0.0, r), Vec3::ZERO);
        let out = resolve(&a, &b, &cfg);
        assert!((out.normal.z + 1.0).abs() < 1e-12);
        // The equal-mass impulse leaves the striker with (1-e)/2 of its speed and
        // drives the other ball down at (1+e)/2 of it.
        let e = cfg.ball_ball.e_ball;
        assert!((out.velocity_a.z - (e - 1.0) / 2.0).abs() < 1e-12);
        assert!((out.velocity_b.z + (1.0 + e) / 2.0).abs() < 1e-12);
    }
}
