//! Aim and tip conversion, in the core frame, with no Godot types.
//!
//! The game stores an aim as degrees and asks the engine for the direction,
//! because a stored direction would have been built with whatever trig the
//! platform happened to provide (§11.4). This module owns that conversion for
//! both the bridge and the server.

use pool_core::math::Vec3;

/// One degree, in radians.
const DEG: f64 = std::f64::consts::PI / 180.0;

/// Normalises a level aim into `[0, 360)`, so that an aim of `450` degrees and
/// an aim of `90` degrees are the same shot.
///
/// `f64::rem_euclid` is used rather than `%` because a negative aim (the
/// shooter's right) must land in the same circle as a positive one. Exact
/// multiples of 360 land on 0, and an already-normalised angle is unchanged,
/// bit for bit, so existing pins are not disturbed.
pub fn normalize_aim_deg(level_aim_deg: f64) -> f64 {
    if level_aim_deg.is_finite() {
        level_aim_deg.rem_euclid(360.0)
    } else {
        level_aim_deg
    }
}

/// The cue direction for a level aim and a butt raise, as a unit vector in the
/// **core** frame.
///
/// `level_aim_deg` is measured from the core's `+x` towards its `+y`, in
/// degrees, and is normalised with [`normalize_aim_deg`]. `elevation_deg` is the
/// raise at the butt: a positive raise points the cue **down** at the ball,
/// which is what a level stroke and a jump both do, so the core's `z` component
/// is `-sin(elevation)`.
///
/// The trig is `libm`'s, never the platform's, and the conversion is done here
/// so the Godot bridge and the server derive the same direction from the same
/// wire numbers.
pub fn cue_direction(level_aim_deg: f64, elevation_deg: f64) -> Vec3 {
    let aim = normalize_aim_deg(level_aim_deg) * DEG;
    let elevation = elevation_deg * DEG;
    let (s_aim, c_aim) = (libm::sin(aim), libm::cos(aim));
    let (s_elev, c_elev) = (libm::sin(elevation), libm::cos(elevation));
    Vec3::new(c_aim * c_elev, s_aim * c_elev, -s_elev)
}

/// The tip offset the core wants, from the offset a player drew.
///
/// The player drags the tip to the shooter's **right**; the core measures the
/// contact along the shooter's **left** (`up × direction`). Only the sideways
/// part changes sign, so right english is a negative `contact_offset_x`.
pub fn contact_offset(across: f64, up: f64) -> (f64, f64) {
    (-across, up)
}

/// The distance from the tip to the centre of the ball, in metres.
pub fn impact_parameter(contact_offset: (f64, f64)) -> f64 {
    libm::sqrt(contact_offset.0 * contact_offset.0 + contact_offset.1 * contact_offset.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_angle_already_in_range_is_unchanged_bit_for_bit() {
        for degrees in [0.0, 12.5, 90.0, 180.0, 270.0, 359.999] {
            assert_eq!(normalize_aim_deg(degrees).to_bits(), degrees.to_bits());
        }
    }

    #[test]
    fn a_lap_does_not_change_the_direction() {
        assert_eq!(normalize_aim_deg(450.0), 90.0);
        assert_eq!(normalize_aim_deg(-270.0), 90.0);
        assert_eq!(normalize_aim_deg(720.0), 0.0);
        let once = cue_direction(90.0, 12.0);
        let twice = cue_direction(450.0, 12.0);
        for i in 0..3 {
            assert_eq!(once.get(i).to_bits(), twice.get(i).to_bits());
        }
    }

    #[test]
    fn a_raised_cue_points_down_at_the_ball() {
        let direction = cue_direction(90.0, 12.0);
        assert!(direction.z < 0.0, "the cue points up: {direction:?}");
        assert!((direction.norm() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn right_english_is_a_negative_core_offset() {
        assert_eq!(contact_offset(0.01, 0.02), (-0.01, 0.02));
        assert!(
            (impact_parameter(contact_offset(0.01, 0.02)) - libm::sqrt(0.01 * 0.01 + 0.02 * 0.02))
                .abs()
                < 1e-15
        );
    }
}
