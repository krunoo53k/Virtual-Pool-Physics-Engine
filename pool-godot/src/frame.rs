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

/// The cue direction for a level aim and a cue raised at the butt, as a unit
/// vector in the Godot frame, with the elevation baked in the way §12.3 wants
/// ("the elevation for jump shots is baked into this vector").
///
/// The game, its tests and the AIs inside it all ask the engine for their
/// directions rather than calling whatever math library GDScript happens to
/// link, so an aim stored as degrees replays the same on every platform (§11.4:
/// the trig comes from the same `libm` the core uses).
///
/// `level_aim_deg` is the level aim measured the way [`angle`] measures, in
/// degrees rather than radians: from the core's own `+x` (Godot's) along the
/// length of the table towards its own `+y` (Godot's `-y`, the shooter's left).
/// `elevation_deg` is the raise at the butt: a POSITIVE raise points the cue
/// DOWN at the ball, which is what a level stroke and §7.1's jump both do;
/// the `elevation_deg` of the preview (sim.rs:1209 reads `asin` of the
/// direction's height) signs a raised cue NEGATIVE, so the two are each
/// other's negation - the sign is the engine's, never the game's.
pub fn cue_direction(level_aim_deg: f64, elevation_deg: f64) -> Vector3 {
    let degrees = std::f64::consts::PI / 180.0;
    let aim = level_aim_deg * degrees;
    let elevation = elevation_deg * degrees;
    let (s_aim, c_aim) = (libm::sin(aim), libm::cos(aim));
    let (s_elev, c_elev) = (libm::sin(elevation), libm::cos(elevation));
    to_godot(pool_core::math::Vec3::new(
        c_aim * c_elev,
        s_aim * c_elev,
        -s_elev,
    ))
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
        assert!(
            turned.y > 0.0,
            "the shot did not turn to the left: {turned:?}"
        );
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

    #[test]
    fn the_baked_level_direction_is_a_unit_vector_along_the_aim() {
        // Along the shooter's left (the core's +y): Godot's -z.
        let level = cue_direction(90.0, 0.0);
        assert!((level.length_squared() - 1.0) < 1e-6);
        assert!(
            level.z < 0.0,
            "the shot points away from the shooter: {level:?}"
        );
        // Along the core's +x: Godot's +x.
        let along = cue_direction(0.0, 0.0);
        assert!((along.x - 1.0) < 1e-9);
        assert!(along.z.abs() < 1e-9);
        assert!(along.y.abs() < 1e-9);
    }

    #[test]
    fn the_baked_raise_points_the_cue_down_at_the_ball() {
        // The same 12 degrees the headless pin raises the cue by, in the
        // engine's own terms: the core's direction is (0, cos, -sin) (the
        // one tests/headless.rs's the_raised_shot puts into ShotInput), so
        // Godot's is (0, -sin, -cos) - the y is negative because Godot's y
        // is up and the cue points down at the ball.
        let raised = cue_direction(90.0, 12.0);
        let sine = libm::sin(12.0 * (std::f64::consts::PI / 180.0));
        let cosine = libm::cos(12.0 * (std::f64::consts::PI / 180.0));
        assert!(
            ((raised.y as f64) + sine).abs() < 1e-7,
            "raised: {raised:?}"
        );
        assert!(
            ((raised.z as f64) + cosine).abs() < 1e-7,
            "raised: {raised:?}"
        );
        assert!(
            raised.y < 0.0,
            "the raised cue does not point down: {raised:?}"
        );
        // And it is still unit, so the elevation is baked in, not bolted on.
        assert!((raised.length_squared() - 1.0) < 1e-6);
        // The level aim of a raised shot is its horizontal part: the aim of
        // the raised shot is still along the shooter's left.
        assert!(cue_direction(90.0, 12.0).x.abs() < 1e-7);
    }

    #[test]
    fn a_level_stroke_is_the_elevation_of_nothing_raised() {
        let level = cue_direction(90.0, 0.0);
        assert_eq!(level.y, 0.0);
        assert!(cue_direction(0.0, 0.0).y == 0.0);
    }
}
