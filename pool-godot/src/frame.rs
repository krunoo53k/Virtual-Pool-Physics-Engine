//! The frame of the game: the core's frame, Godot's frame, and the mapping.
//!
//! The core works in a right handed frame with `x` along the table, `y` across
//! it and `z` up. Godot has `y` up. Everything that crosses the boundary goes
//! through this module, so the convention is written down in exactly one place.
//!
//! # The frame
//!
//! The core's `x` is the length of the table, its `y` the width, its `z` the
//! height. Godot's `y` is the height. The mapping is therefore
//!
//! ```text
//! core (x, y, z)  ->  Godot (x, z, -y)
//! ```
//!
//! One of the two in-plane axes has to flip, and flipping `y` rather than `x`
//! keeps the frame right handed, so the cross products - and with them the
//! sense of the spin - survive the trip.
//!
//! # The side of the ball
//!
//! The core measures the contact offset of the tip along `up x direction`
//! (§6.1). In a right handed frame that is the shooter's **left**: the
//! shooter's right is `direction x up`. So
//!
//! * a **positive** `contact_offset_x` strikes the shooter's **left**;
//! * a **negative** `contact_offset_x` strikes the shooter's **right**.
//!
//! The squirt follows: the ball leaves away from the side the tip struck, so a
//! left english leaves towards the shooter's right.
//!
//! The screen is the other way round from the core: the player drags the tip
//! marker to the **right** of the ball, which is the shooter's right, which is
//! a **negative** `contact_offset_x`. [`contact_offset`] does that flip.

use godot::builtin::Vector3;
use pool_core::math::Vec3;

/// Up, in the core's frame.
pub const UP: Vec3 = Vec3::Z;

/// The core's frame in Godot's.
///
/// The core's `z` is the height and Godot's `y` is, and one of the two
/// in-plane axes has to change sign for the frame to stay right handed.
pub fn to_godot(v: Vec3) -> Vector3 {
    Vector3::new(v.x as f32, v.z as f32, -(v.y as f32))
}

/// Godot's frame in the core's.
pub fn to_core(v: Vector3) -> Vec3 {
    Vec3::new(v.x as f64, -(v.z as f64), v.y as f64)
}

/// The side of the ball the tip strikes: the shooter's left.
///
/// This is the direction the core measures `contact_offset_x` along: the
/// vector `up x direction`.
pub fn side(direction: Vec3) -> Vec3 {
    UP.cross(direction)
}

/// The shooter's right, in the core's frame.
pub fn right(direction: Vec3) -> Vec3 {
    direction.cross(UP)
}

/// The contact offset for a tip that sits `across` metres to the shooter's
/// right of the centre of the ball and `up` metres above it.
///
/// The core wants the offset along the shooter's **left**, so the sideways part
/// changes sign. A tip dragged to the right of the ball is right english, and
/// right english is a negative `contact_offset_x`.
pub fn contact_offset(across: f64, up: f64) -> (f64, f64) {
    (-across, up)
}

/// The aim direction after a fine trim of `angle` radians about the vertical.
///
/// A positive angle turns the shot towards the shooter's left, which is the
/// same way the core rotates a direction: `+y` is to the left of `+x`.
pub fn trim(direction: Vec3, angle: f64) -> Vec3 {
    let (s, c) = (libm::sin(angle), libm::cos(angle));
    Vec3::new(
        direction.x * c - direction.y * s,
        direction.x * s + direction.y * c,
        direction.z,
    )
}

/// The angle of a direction in the core's frame, measured from `+x` towards
/// `+y`, in radians.
pub fn angle(direction: Vec3) -> f64 {
    libm::atan2(direction.y, direction.x)
}

/// The distance from the tip to the centre of the ball, in metres: the impact
/// parameter `b` of §6.1.
pub fn impact_parameter(contact_offset: (f64, f64)) -> f64 {
    let (across, up) = contact_offset;
    libm::sqrt(across * across + up * up)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shot along the length of the table.
    const FORWARD: Vec3 = Vec3::X;

    #[test]
    fn the_side_is_the_shooters_left() {
        // The shooter looks along `+x` with `z` up, so the left hand is `+y`
        // and the right hand is `-y`.
        assert_eq!(side(FORWARD), Vec3::Y);
        assert_eq!(right(FORWARD), Vec3::new(0.0, -1.0, 0.0));
        // The two are opposites, whichever way the shot points.
        let direction = Vec3::new(0.0, 1.0, 0.0);
        assert_eq!(side(direction), Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(right(direction), Vec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn a_tip_on_the_right_is_a_negative_offset() {
        // The player drags the marker to the right of the ball: that is the
        // shooter's right, so the offset the core wants is negative.
        let (x, y) = contact_offset(0.010, 0.0);
        assert!(x < 0.0, "a tip on the right gave {x}");
        assert_eq!(y, 0.0);
        // And a tip on the left is positive.
        let (x, _) = contact_offset(-0.010, 0.0);
        assert!(x > 0.0, "a tip on the left gave {x}");
    }

    #[test]
    fn a_tip_above_the_centre_is_a_positive_offset() {
        // Above the centre is follow, and the core's `contact_offset_y` is
        // positive above the centre.
        let (_, y) = contact_offset(0.0, 0.010);
        assert!(y > 0.0);
    }

    #[test]
    fn the_offset_survives_the_round_trip_through_the_side_vector() {
        // The offset the core reads is the one along `up x direction`, so the
        // tip of a right english is at `-b * side`: on the shooter's right.
        let direction = FORWARD;
        let (x, _) = contact_offset(0.010, 0.0);
        let tip = side(direction) * x;
        assert!(
            tip.dot(right(direction)) > 0.0,
            "the tip is not on the shooter's right: {tip:?}"
        );
        assert_eq!(tip.dot(side(direction)), -0.010);
    }

    #[test]
    fn the_trim_turns_the_shot_to_the_left() {
        let turned = trim(FORWARD, 0.1);
        assert!(turned.y > 0.0, "the shot did not turn to the left: {turned:?}");
        assert!(turned.y > 0.0 && turned.x > 0.0);
        // And back again.
        let back = trim(turned, -0.1);
        assert!((back.x - 1.0).abs() < 1e-12);
        assert!(back.y.abs() < 1e-12);
    }

    #[test]
    fn the_trim_leaves_the_aim_alone_when_it_is_not_asked_for() {
        let turned = trim(FORWARD, 0.0);
        assert_eq!(turned, FORWARD);
    }

    #[test]
    fn the_angle_is_measured_from_the_length_of_the_table() {
        assert!(angle(FORWARD).abs() < 1e-12);
        assert!((angle(Vec3::Y) - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }

    #[test]
    fn the_impact_parameter_is_the_distance_to_the_centre() {
        let offset = contact_offset(0.010, 0.0);
        assert!((impact_parameter(offset) - 0.010).abs() < 1e-12);
        // A tip 10 mm to the side and 10 mm above is further away than 10 mm.
        let offset = contact_offset(0.010, 0.010);
        let expected = libm::sqrt(0.010 * 0.010 + 0.010 * 0.010);
        assert!((impact_parameter(offset) - expected).abs() < 1e-12);
    }
}
