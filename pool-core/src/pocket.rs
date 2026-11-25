//! Ball-pocket interactions: the state machine, the jaw collisions and the shelf of
//! §9.

use crate::config::TableConfig;
use crate::geometry::{JawGeometry, PocketGeometry};
use crate::math::{Vec2, Vec3};
use crate::quartic;
use crate::state::BallState;

/// The time at which a signed distance to a line crosses a value.
///
/// The signed distance is `s(t) = (p(t) - origin) . normal - target`, and the ball's
/// plan position is quadratic in `t`, so the crossing time solves a quadratic.
///
/// `direction` selects the crossing: `+1` for a rising signed distance, `-1` for a
/// falling one.
///
/// The flat parameters mirror the quadratic being solved; bundling them would only
/// obscure the correspondence with the derivation.
#[allow(clippy::too_many_arguments)]
pub fn time_to_cross_line(
    p0: Vec2,
    v: Vec2,
    a: Vec2,
    origin: Vec2,
    normal: Vec2,
    target: f64,
    direction: f64,
    horizon: f64,
) -> Option<f64> {
    let s0 = (p0 - origin).dot(normal) - target;
    let sv = v.dot(normal);
    let sa = a.dot(normal);
    let coeffs = [0.5 * sa, sv, s0];
    let roots = quartic::quadratic_roots_abc(coeffs[0], coeffs[1], coeffs[2]);
    let mut best: Option<f64> = None;
    for t in roots {
        if !(t > 0.0 && t <= horizon) {
            continue;
        }
        // The crossing has to be in the requested direction.
        if (sv + sa * t) * direction <= 0.0 {
            continue;
        }
        best = match best {
            Some(b) if b <= t => best,
            _ => Some(t),
        };
    }
    best
}

/// The time at which the ball's centre crosses the pocket's mouth line (§9.2).
pub fn time_to_cross_mouth(
    ball: &BallState,
    accel: Vec2,
    pocket: &PocketGeometry,
    direction: f64,
    horizon: f64,
) -> Option<f64> {
    time_to_cross_line(
        Vec2::new(ball.position.x, ball.position.y),
        Vec2::new(ball.velocity.x, ball.velocity.y),
        accel,
        pocket.mouth_centre,
        pocket.centreline,
        0.0,
        direction,
        horizon,
    )
}

/// The time at which the ball's centre crosses the pocket's drop line (§9.4).
pub fn time_to_cross_drop(
    ball: &BallState,
    accel: Vec2,
    pocket: &PocketGeometry,
    horizon: f64,
) -> Option<f64> {
    time_to_cross_line(
        Vec2::new(ball.position.x, ball.position.y),
        Vec2::new(ball.velocity.x, ball.velocity.y),
        accel,
        pocket.mouth_centre,
        pocket.centreline,
        pocket.shelf_depth,
        1.0,
        horizon,
    )
}

/// The time at which a ball touches a jaw (a facing).
///
/// The facing is a vertical plane segment. The contact condition is the distance
/// from the ball's centre to the segment being `R`, which is a quartic in `t` for the
/// rounded ends and a quadratic for the flat part.
pub fn jaw_time_of_impact(
    ball: &BallState,
    accel: Vec3,
    jaw: &JawGeometry,
    horizon: f64,
) -> Option<f64> {
    let p0 = Vec3::new(ball.position.x, ball.position.y, ball.position.z);
    let v = ball.velocity;
    let a = accel;
    let radius = ball.radius;

    let tip = Vec3::new(jaw.tip.x, jaw.tip.y, ball.position.z);
    let end = Vec3::new(jaw.end.x, jaw.end.y, ball.position.z);

    let mut best: Option<f64> = None;

    // The flat part of the facing. The ball touches the facing when the distance
    // from its centre to the plane is the ball's radius, so the contact condition is
    // a quadratic in `t`.
    let normal = Vec3::new(jaw.normal.x, jaw.normal.y, 0.0);
    let tangent = jaw.direction();
    let tangent = Vec3::new(tangent.x, tangent.y, 0.0);
    let distance_to_plane = (p0 - tip).dot(normal);
    let closing_speed = v.dot(normal);

    for sign in [1.0_f64, -1.0_f64] {
        let s0 = distance_to_plane - sign * radius;
        let sv = closing_speed;
        let sa = a.dot(normal);
        let coeffs = [0.5 * sa, sv, s0];
        for t in quartic::quadratic_roots_abc(coeffs[0], coeffs[1], coeffs[2]) {
            if !(t > crate::math::MIN_IMPACT_TIME && t <= horizon) {
                continue;
            }
            // The ball has to cross the contact surface towards the facing: the
            // signed distance has to shrink to zero.
            let signed = s0 + sv * t + sa * 0.5 * t * t;
            let derivative = sv + sa * t;
            if signed * derivative >= 0.0 {
                continue;
            }
            // The contact has to be on the segment, not beyond its end.
            let p = p0 + v * t + a * (0.5 * t * t);
            let along = (p - tip).dot(tangent);
            let len = (end - tip).dot(tangent);
            if along < -1e-9 || along > len + 1e-9 {
                continue;
            }
            best = match best {
                Some(b) if b <= t => best,
                _ => Some(t),
            };
        }
    }

    // A ball that is already in contact with the facing - or inside it - and moving
    // into it has to be resolved now: it would otherwise pass through the facing.
    if distance_to_plane.abs() < radius && distance_to_plane * closing_speed < 0.0 {
        let along = (p0 - tip).dot(tangent);
        let len = (end - tip).dot(tangent);
        if along >= -1e-9 && along <= len + 1e-9 {
            best = Some(0.0);
        }
    }

    // The rounded ends of the facing. They only matter when the ball is not
    // touching the flat part already: a ball that slides along the facing is held
    // by it, and the end of the segment is not a contact of its own.
    let along = (p0 - tip).dot(tangent);
    let length = (end - tip).dot(tangent);
    let on_the_flat_part = along > 0.0 && along < length;
    if !(on_the_flat_part && distance_to_plane.abs() <= radius + 1e-6) {
        for point in [tip, end] {
            let dp0 = p0 - point;
            let dv = v;
            let da = a;
            let coeffs = [
                0.25 * da.norm_squared(),
                da.dot(dv),
                dv.norm_squared() + da.dot(dp0),
                2.0 * dv.dot(dp0),
                dp0.norm_squared() - radius * radius,
            ];
            if let Some(t) = quartic::first_root_after(&coeffs, 0.0, horizon) {
                if t > crate::math::MIN_IMPACT_TIME {
                    // The ball has to be closing on the end of the facing.
                    let gap = dp0 + dv * t + da * (0.5 * t * t);
                    let closing = gap.dot(dv + da * t);
                    if closing < 0.0 {
                        best = match best {
                            Some(b) if b <= t => best,
                            _ => Some(t),
                        };
                    }
                }
            }
        }
    }

    best
}

/// The result of a jaw impact.
#[derive(Clone, Copy, Debug)]
pub struct JawOutcome {
    pub velocity: crate::math::Vec3,
    pub angular_velocity: crate::math::Vec3,
    pub jn: f64,
    pub jt: f64,
    pub sticking: bool,
}

/// Resolves a jaw impact.
///
/// The facings behave like short pieces of cushion: the same impulsive model as
/// §5.1, with the pocket's own restitution and friction and the facing's own normal
/// (§9.3).
pub fn resolve_jaw(ball: &BallState, jaw: &JawGeometry, cfg: &TableConfig) -> JawOutcome {
    // The contact normal depends on where the ball touches the facing: on the flat
    // part it is the facing's own normal, at the tip or the throat it points away
    // from the end of the facing.
    let position = Vec2::new(ball.position.x, ball.position.y);
    let (closest, _) = jaw.closest_point(position);
    let along = (closest - jaw.tip).dot(jaw.direction());
    let length = (jaw.end - jaw.tip).dot(jaw.direction());
    let mut towards = Vec3::new(jaw.normal.x, jaw.normal.y, 0.0);
    if along <= 0.0 {
        // The tip: the contact normal points away from the tip, i.e. from the ball
        // towards the facing.
        let radial = position - jaw.tip;
        if let Some(n) = Vec3::new(radial.x, radial.y, 0.0).normalize() {
            towards = -n;
        }
    } else if along >= length {
        // The throat end of the facing.
        let radial = position - jaw.end;
        if let Some(n) = Vec3::new(radial.x, radial.y, 0.0).normalize() {
            towards = -n;
        }
    } else {
        // The flat part of the facing: the normal has to point from the ball towards
        // the facing, whichever side of it the ball is on.
        let offset = position - jaw.tip;
        if offset.dot(jaw.normal) > 0.0 {
            towards = -towards;
        }
    }

    let tangent3 = Vec3::new(-towards.y, towards.x, 0.0);
    let outcome = crate::cushion::resolve_cushion(
        ball,
        towards,
        tangent3,
        0.0,
        1.0,
        cfg.table.e_jaw,
        cfg.table.mu_jaw,
        cfg,
    );
    JawOutcome {
        velocity: outcome.velocity,
        angular_velocity: outcome.angular_velocity,
        jn: outcome.jn,
        jt: outcome.jt,
        sticking: outcome.sticking,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TableConfig;
    use crate::geometry::TableGeometry;
    use crate::math::Vec3;

    #[test]
    fn a_ball_rolling_straight_into_a_corner_pocket_crosses_the_mouth() {
        let cfg = TableConfig::default();
        let geom = TableGeometry::new(&cfg);
        let pocket = &geom.pockets[0];
        // The corner pocket's centreline runs along the diagonal, so the ball is
        // placed on it, just outside the mouth.
        let dir = pocket.centreline;
        let start = pocket.mouth_centre - dir * 0.2;
        let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(start.x, start.y, cfg.ball.radius));
        ball.velocity = Vec3::new(dir.x * 1.5, dir.y * 1.5, 0.0);
        let t = time_to_cross_mouth(&ball, Vec2::ZERO, pocket, 1.0, 10.0).expect("crossing");
        assert!((t - 0.2 / 1.5).abs() < 1e-12);
        let drop = time_to_cross_drop(&ball, Vec2::ZERO, pocket, 10.0).expect("drop");
        let expected = (0.2 + pocket.shelf_depth) / 1.5;
        assert!((drop - expected).abs() < 1e-12);
    }
}
