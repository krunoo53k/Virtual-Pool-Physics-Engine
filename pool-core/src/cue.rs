//! Ball-cue interactions: the strike model of §6.
//!
//! The cue's direction arrives as a unit vector, so the core never has to call
//! `sin`/`cos` for the aim (§12.3).

use crate::config::TableConfig;
use crate::math::Vec3;
use crate::state::ShotInput;

/// The result of a cue strike.
#[derive(Clone, Copy, Debug)]
pub struct CueStrike {
    /// The cue ball's linear velocity after the strike.
    pub velocity: Vec3,
    /// The cue ball's angular velocity after the strike.
    pub angular_velocity: Vec3,
    /// The squirt angle, radians (always positive; the direction is in `squirt_sign`).
    pub squirt: f64,
    /// `-1` when the ball squirts towards `-u` (away from the side that was struck),
    /// `+1` otherwise.
    pub squirt_sign: f64,
    /// The impact parameter: the distance from the cue's line of travel to the
    /// ball's centre.
    pub impact_parameter: f64,
    /// True when the tip slipped off the ball (§6.3).
    pub miscue: bool,
}

/// Applies a cue strike to the cue ball (§6.2).
pub fn strike(input: &ShotInput, cfg: &TableConfig) -> CueStrike {
    let radius = cfg.ball.radius;
    let mass = cfg.ball.mass;

    let direction = match input.cue_direction.normalize() {
        Some(d) => d,
        None => {
            // A degenerate aim direction: nothing happens.
            return CueStrike {
                velocity: Vec3::ZERO,
                angular_velocity: Vec3::ZERO,
                squirt: 0.0,
                squirt_sign: 1.0,
                impact_parameter: 0.0,
                miscue: false,
            };
        }
    };

    // The horizontal direction of the side offset: `u = k x direction`, which is the
    // document's "+y" in its §6.2 frame.
    let side = Vec3::Z.cross(direction);
    let side = match side.normalize() {
        Some(s) => s,
        None => Vec3::X,
    };

    // The contact point on the ball, relative to its centre.
    let mut offset = side * input.contact_offset_x + Vec3::Z * input.contact_offset_y;

    // The impact parameter: the distance from the cue's axis to the ball's centre.
    let impact_parameter = offset.cross(direction).norm();

    // The miscue condition of §6.3: `b <= R * mu_cue / sqrt(1 + mu_cue^2)`.
    let limit = cfg.max_offset_without_miscue();
    let mut miscue = false;
    if impact_parameter > limit && impact_parameter > 0.0 {
        miscue = true;
        // The tip slips off at the largest offset it can hold.
        let scale = limit / impact_parameter;
        offset = offset * scale;
    }

    // The linear velocity of the ball (§6.2).
    //
    // The impulse responds against the ball's effective mass at the contact
    // point, `1/m_eff = 1/m + b²/I = (1 + 5b²/(2R²))/m`, so
    //
    //     v_ball = (1 + e)·V / (1 + m/M + 5b²/(2R²))
    //
    // and an off-centre hit leaves the ball slower than a centre one (at
    // b = 0.5R it is 31% slower for a standard cue and ball). At `b = 0` this
    // is the centre-hit formula `(1 + e)·M·V/(m + M)`. The impact parameter is
    // the clamped one: the tip slips off at the miscue limit, so that is where
    // it strikes.
    let impact = offset.cross(direction).norm();
    let impact_ratio = if radius > 0.0 { impact / radius } else { 0.0 };
    let speed = (1.0 + cfg.cue.e_cue) * input.cue_speed
        / (1.0 + mass / cfg.cue.cue_mass + 2.5 * impact_ratio * impact_ratio);
    let speed = if speed.is_finite() { speed } else { 0.0 };

    // The squirt (§6.1): the calibration curve is looked up at the impact
    // parameter and the ball is deflected away from the side that was struck.
    //
    // The squirt comes from the tip *sliding* on the ball, and the tip slides
    // around the ball: only a **sideways** offset makes it slide sideways. A
    // tip above or below the centre gives follow or draw, and that shot goes
    // straight - the tip does not slide sideways at all. So the squirt is
    // looked up at the sideways part of the contact, and not at the whole
    // impact parameter, which counts a tip that is above or below the centre
    // as an off-centre hit as well.
    let lateral = offset.dot(side).abs();
    let b_over_r = if radius > 0.0 {
        lateral / radius
    } else {
        0.0
    };
    let squirt_deg = squirt_angle(b_over_r, cfg);
    let squirt_sign = if input.contact_offset_x >= 0.0 {
        -1.0
    } else {
        1.0
    };

    // The deflected direction: rotating the cue axis about the vertical by
    // `-squirt * sign` moves it away from the struck side.
    let angle = squirt_deg.to_radians() * squirt_sign;
    let (s, c) = (libm::sin(angle), libm::cos(angle));
    let deflected = Vec3::new(
        direction.x * c - direction.y * s,
        direction.x * s + direction.y * c,
        direction.z,
    );

    let velocity = deflected * speed;

    // The spin (§6.2): `omega = (5/2) * (b / R) * (v_ball / R)`, about the axis
    // `r_c x direction`.
    let arm = offset.norm();
    let axis = offset.cross(direction);
    let angular_velocity = match axis.normalize() {
        Some(unit) => {
            let omega = 2.5 * (arm / radius) * (speed / radius);
            unit * omega
        }
        None => Vec3::ZERO,
    };

    CueStrike {
        velocity,
        angular_velocity,
        squirt: squirt_deg.to_radians(),
        squirt_sign,
        impact_parameter,
        miscue,
    }
}

/// The squirt angle, in degrees, for an impact parameter of `b_over_r` radii.
///
/// The curve is a calibration, not a closed form (§6.1): nearly linear up to
/// `b = 0.5R` and flattening beyond.
pub fn squirt_angle(b_over_r: f64, cfg: &TableConfig) -> f64 {
    let curve = &cfg.cue.squirt_curve;
    if curve.is_empty() {
        return 0.0;
    }
    let x = b_over_r.clamp(curve[0].0, curve[curve.len() - 1].0);
    let mut result = curve[curve.len() - 1].1;
    for pair in curve.windows(2) {
        let (x0, y0) = pair[0];
        let (x1, y1) = pair[1];
        if x >= x0 && x <= x1 {
            let span = x1 - x0;
            let t = if span.abs() > 0.0 {
                (x - x0) / span
            } else {
                0.0
            };
            result = y0 + t * (y1 - y0);
            break;
        }
        if x < x0 {
            result = y0;
            break;
        }
    }
    result * cfg.cue.squirt_scale
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::BallState;

    fn ball(cfg: &TableConfig) -> BallState {
        BallState::at_rest(cfg, 0, Vec3::ZERO)
    }

    #[test]
    fn centre_ball_hit_has_no_squirt_and_no_spin() {
        let cfg = TableConfig::default();
        let input = ShotInput::new(2.0, Vec3::X, vec![ball(&cfg)]);
        let out = strike(&input, &cfg);
        assert!((out.squirt_deg_for_test()).abs() < 1e-12);
        assert!(out.angular_velocity.norm() < 1e-12);
        assert!(!out.miscue);
        // The velocity is along the cue axis.
        assert!((out.velocity.y).abs() < 1e-15);
    }

    #[test]
    fn side_english_gives_negative_omega_z_and_squirts_away() {
        let cfg = TableConfig::default();
        let mut input = ShotInput::new(2.0, Vec3::X, vec![ball(&cfg)]);
        input.contact_offset_x = 0.01;
        let out = strike(&input, &cfg);
        assert!(
            out.angular_velocity.z < 0.0,
            "omega_z = {}",
            out.angular_velocity.z
        );
        assert!(
            out.velocity.y < 0.0,
            "the ball squirts away from the struck side"
        );
    }

    #[test]
    fn top_spin_gives_positive_omega_y() {
        let cfg = TableConfig::default();
        let mut input = ShotInput::new(2.0, Vec3::X, vec![ball(&cfg)]);
        input.contact_offset_y = 0.01;
        let out = strike(&input, &cfg);
        assert!(out.angular_velocity.y > 0.0);
    }

    #[test]
    fn miscue_is_detected_beyond_51_percent_of_the_radius() {
        let cfg = TableConfig::default();
        let mut input = ShotInput::new(2.0, Vec3::X, vec![ball(&cfg)]);
        input.contact_offset_x = 0.6 * cfg.ball.radius;
        let out = strike(&input, &cfg);
        assert!(out.miscue);
        // The tip slipped off: the attempt was beyond the limit.
        assert!(out.impact_parameter > cfg.max_offset_without_miscue());
    }

    #[test]
    fn an_off_centre_hit_leaves_the_ball_slower() {
        // The impulse responds against the ball's effective mass at the contact
        // point (section 6.2): `v_ball = (1 + e)·V / (1 + m/M + 5b²/2R²)`, so a
        // tip at `b = 0.5R` leaves the ball about 31% slower than a centre hit,
        // and the centre-hit formula is the `b = 0` case of it.
        let cfg = TableConfig::default();

        let centre = ShotInput::new(2.0, Vec3::X, vec![ball(&cfg)]);
        let centre_speed = strike(&centre, &cfg).velocity.norm();
        let centre_formula = (1.0 + cfg.cue.e_cue) * 2.0 * cfg.cue.cue_mass
            / (cfg.ball.mass + cfg.cue.cue_mass);
        assert!((centre_speed - centre_formula).abs() < 1e-12);

        let mut tipped = ShotInput::new(2.0, Vec3::X, vec![ball(&cfg)]);
        tipped.contact_offset_y = 0.5 * cfg.ball.radius;
        let out = strike(&tipped, &cfg);
        assert!(!out.miscue);
        let expected = (1.0 + cfg.cue.e_cue) * 2.0
            / (1.0 + cfg.ball.mass / cfg.cue.cue_mass + 2.5 * 0.25);
        assert!((out.velocity.norm() - expected).abs() < 1e-12);

        let ratio = out.velocity.norm() / centre_speed;
        assert!(
            (ratio - 0.6874).abs() < 1e-3,
            "a tip at half a radius leaves the ball at {ratio} of the centre-hit speed"
        );
    }

    impl CueStrike {
        fn squirt_deg_for_test(&self) -> f64 {
            self.squirt.to_degrees()
        }
    }
}
