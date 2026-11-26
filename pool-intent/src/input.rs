//! The input side of a stroke: rack hand-off and the tuning bounds.
//!
//! A hand-off carries only positions, numbers and rotations. A ball that ended
//! the last shot on a pocket shelf arrives here as a bare position, so its
//! pocket state has to be recovered from the geometry before the drop logic
//! can see it (§9.2). This is the same conversion the Godot bridge has always
//! made; it lives here so the server makes it identically.

use pool_core::config::TableConfig;
use pool_core::geometry::TableGeometry;
use pool_core::math::{Vec2, Vec3};
use pool_core::state::{BallState, PocketState, ShotInput};

/// A ball handed over at the drop line carries only a position. Godot's `f32`
/// crossing is good to a few nanometres, so the depth recovered here gets that
/// much slack.
const DROP_LINE_TOLERANCE: f64 = 1e-6;

/// The shot input for a stroke.
///
/// `direction` is the cue axis in the **core** frame, from the cue ball towards
/// the target, with the elevation already baked in. `contact_offset` is the
/// tip on the cue ball's face in the player's terms: distance to the shooter's
/// **right** and distance **above** centre, both in metres; the sideways sign
/// flip happens in [`crate::direction::contact_offset`].
///
/// `rotations` is the orientation every ball carries in, index by index with
/// `positions`. A short list leaves unnamed balls at identity.
pub fn shot_input(
    table: &TableConfig,
    cue_speed: f64,
    direction: Vec3,
    contact_offset: (f64, f64),
    positions: &[Vec3],
    numbers: &[i32],
    rotations: &[Vec3],
) -> ShotInput {
    let (across, up) = crate::direction::contact_offset(contact_offset.0, contact_offset.1);
    let geometry = TableGeometry::new(table);
    let mut balls: Vec<BallState> = Vec::with_capacity(positions.len());
    for (index, position) in positions.iter().enumerate() {
        let number = numbers.get(index).copied().unwrap_or(index as i32);
        let number = u8::try_from(number).unwrap_or(0);
        let mut ball = BallState::at_rest(table, number, *position);
        // The pocket state machine announces `pocket-enter` only on a crossing
        // from the table side, so a ball handed over past the mouth line has to
        // start in `Entering` (or `Pocketed` past the drop line) or it would be
        // taken out by the off-table backstop instead.
        let plan = Vec2::new(ball.position.x, ball.position.y);
        for pocket in &geometry.pockets {
            if pocket.lateral(plan).abs() > pocket.mouth_width / 2.0 {
                continue;
            }
            let depth = pocket.mouth_distance(plan);
            if depth >= pocket.shelf_depth - DROP_LINE_TOLERANCE {
                ball.pocket_state = PocketState::Pocketed;
                ball.is_pocketed = true;
            } else if depth >= 0.0 {
                ball.pocket_state = PocketState::Entering;
            } else {
                continue;
            }
            break;
        }
        ball.rotation = rotations.get(index).copied().unwrap_or(Vec3::ZERO);
        balls.push(ball);
    }
    let mut input = ShotInput::new(cue_speed, direction, balls);
    input.contact_offset_x = across;
    input.contact_offset_y = up;
    input
}

/// The legal range of a stroke, shared by the validation on the server and the
/// aim UI on the client. All bounds are **inclusive**: the exact edge is legal.
pub struct CueTuning;

impl CueTuning {
    /// Slowest legal stroke, m/s.
    pub const MIN_SPEED: f64 = 0.15;
    /// Fastest legal stroke, m/s.
    pub const MAX_SPEED: f64 = 9.0;
    /// Lowest legal butt raise, degrees. A level stroke is legal.
    pub const MIN_ELEVATION_DEG: f64 = 0.0;
    /// Highest legal butt raise, degrees.
    pub const MAX_ELEVATION_DEG: f64 = 60.0;

    /// The largest radial tip offset that holds, for a given table, in metres.
    ///
    /// This is the core's miscue limit, not a second hand-maintained number:
    /// an offset outside the disc is legal on the wire and becomes a foul the
    /// core reports.
    pub fn tip_radius(table: &TableConfig) -> f64 {
        table.max_offset_without_miscue()
    }

    /// The largest radial tip offset for the default table, in metres.
    pub fn default_tip_radius() -> f64 {
        Self::tip_radius(&TableConfig::default())
    }

    /// Speed is in `[0.15, 9.0]`, both ends included.
    pub fn speed_is_legal(speed: f64) -> bool {
        speed.is_finite() && (Self::MIN_SPEED..=Self::MAX_SPEED).contains(&speed)
    }

    /// Butt raise is in `[0, 60]` degrees, both ends included.
    pub fn elevation_is_legal(elevation_deg: f64) -> bool {
        elevation_deg.is_finite()
            && (Self::MIN_ELEVATION_DEG..=Self::MAX_ELEVATION_DEG).contains(&elevation_deg)
    }

    /// The tip must lie inside the disc of radius `radius`.
    ///
    /// This is a **radial** check (`hypot ≤ R`), never two per-axis checks: a
    /// tip near the edge on both axes is outside even though each axis alone is
    /// inside.
    pub fn tip_is_legal(across: f64, up: f64, radius: f64) -> bool {
        across.is_finite()
            && up.is_finite()
            && radius.is_finite()
            && libm::hypot(across, up) <= radius
    }

    /// Tip check against a table's own miscue disc.
    pub fn contact_is_legal(table: &TableConfig, across: f64, up: f64) -> bool {
        Self::tip_is_legal(across, up, Self::tip_radius(table))
    }

    /// All six shot scalars at once. An aim is legal for any finite value —
    /// it is normalised mod 360 by [`crate::direction::cue_direction`].
    pub fn params_are_legal(
        speed: f64,
        level_aim_deg: f64,
        elevation_deg: f64,
        contact_x: f64,
        contact_y: f64,
        table: &TableConfig,
    ) -> bool {
        level_aim_deg.is_finite()
            && Self::speed_is_legal(speed)
            && Self::elevation_is_legal(elevation_deg)
            && Self::contact_is_legal(table, contact_x, contact_y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_bounds_are_inclusive() {
        assert!(CueTuning::speed_is_legal(0.15));
        assert!(CueTuning::speed_is_legal(9.0));
        assert!(CueTuning::speed_is_legal(4.5));
        assert!(!CueTuning::speed_is_legal(f64::from_bits(
            0.15f64.to_bits() - 1
        )));
        assert!(!CueTuning::speed_is_legal(f64::from_bits(
            9.0f64.to_bits() + 1
        )));
        assert!(!CueTuning::speed_is_legal(f64::NAN));
        assert!(!CueTuning::speed_is_legal(f64::INFINITY));
    }

    #[test]
    fn elevation_bounds_are_inclusive() {
        assert!(CueTuning::elevation_is_legal(0.0));
        assert!(CueTuning::elevation_is_legal(60.0));
        assert!(CueTuning::elevation_is_legal(12.0));
        assert!(!CueTuning::elevation_is_legal(-1e-12));
        assert!(!CueTuning::elevation_is_legal(f64::from_bits(
            60.0f64.to_bits() + 1
        )));
        assert!(!CueTuning::elevation_is_legal(f64::NAN));
    }

    #[test]
    fn the_tip_check_is_radial_and_inclusive() {
        let radius = CueTuning::default_tip_radius();
        assert!(radius > 0.0);
        // The exact edge holds: RHS is computed the same way as the check.
        assert!(CueTuning::tip_is_legal(radius, 0.0, radius));
        assert!(CueTuning::tip_is_legal(-radius, 0.0, radius));
        assert!(CueTuning::tip_is_legal(0.0, radius, radius));
        assert!(!CueTuning::tip_is_legal(
            f64::from_bits(radius.to_bits() + 1),
            0.0,
            radius
        ));
        // Both axes are inside the disc by themselves, but the corner of the
        // square is outside the circle. A per-axis check would let this through.
        let across = 0.011;
        let up = 0.011;
        assert!(across < radius && up < radius);
        assert!(!CueTuning::tip_is_legal(across, up, radius));
        assert!(!CueTuning::tip_is_legal(f64::NAN, 0.0, radius));
    }

    #[test]
    fn a_handed_over_ball_recovers_its_pocket_state() {
        let table = TableConfig::default();
        let geometry = pool_core::geometry::TableGeometry::new(&table);
        let pocket = &geometry.pockets[0];
        // A point halfway down the pocket's centreline: inside the mouth, lateral=0.
        let depth = 0.5 * pocket.shelf_depth;
        let plan = pocket.mouth_centre + pocket.centreline * depth;
        let position = Vec3::new(plan.x, plan.y, table.ball.radius);
        let input = shot_input(&table, 1.0, Vec3::X, (0.0, 0.0), &[position], &[0], &[]);
        assert_eq!(input.ball_states[0].pocket_state, PocketState::Entering);
    }
}
