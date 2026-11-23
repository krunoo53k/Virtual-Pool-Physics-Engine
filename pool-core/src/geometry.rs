//! The table geometry derived from [`TableConfig`].
//!
//! The playing surface spans `x in [0, length]`, `y in [0, width]`, with `z` up and
//! the origin at the corner where the two cushion nose lines of the lower-left
//! corner pocket would meet. The bed is at `z = 0`; a ball resting on it has its
//! centre at `z = R`.
//!
//! Everything here is derived: the cushion contact height and tilt come from the
//! nose geometry (§5.1) and the pocket facings come from the cut angles (§9.1).

use crate::config::TableConfig;
use crate::math::{Vec2, Vec3};

/// The kind of pocket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PocketKind {
    Corner,
    Side,
}

/// One rail of the table.
#[derive(Clone, Debug)]
pub struct RailGeometry {
    /// Unit horizontal vector pointing from the playing surface towards the rail.
    pub normal: Vec2,
    /// Unit horizontal vector running along the rail.
    pub tangent: Vec2,
    /// Coordinate of the cushion nose line along `normal`.
    pub line_coord: f64,
    /// The spans of the rail that carry a cushion nose, as coordinates along `tangent`.
    /// The gaps between them are the pocket mouths.
    pub cushion_spans: Vec<(f64, f64)>,
}

impl RailGeometry {
    /// The coordinate of the ball centre at the moment of contact with the nose
    /// rubber: the nose line inset by `R * cos(theta)`.
    pub fn contact_coord(&self, cos_theta: f64, radius: f64) -> f64 {
        self.line_coord - radius * cos_theta
    }
}

/// One pocket facing (a "jaw").
#[derive(Clone, Copy, Debug)]
pub struct JawGeometry {
    /// The jaw tip, on the cushion nose line.
    pub tip: Vec2,
    /// The far end of the facing (the throat).
    pub end: Vec2,
    /// Unit normal, pointing into the pocket's interior.
    pub normal: Vec2,
}

impl JawGeometry {
    /// The segment direction, from the tip towards the throat.
    pub fn direction(&self) -> Vec2 {
        match (self.end - self.tip).normalize() {
            Some(d) => d,
            None => Vec2::ZERO,
        }
    }

    /// The closest point of the segment to `p`, and the squared distance to it.
    pub fn closest_point(&self, p: Vec2) -> (Vec2, f64) {
        let d = self.end - self.tip;
        let len2 = d.norm_squared();
        if len2 <= 0.0 {
            let diff = p - self.tip;
            return (self.tip, diff.norm_squared());
        }
        let t = ((p - self.tip).dot(d) / len2).clamp(0.0, 1.0);
        let closest = self.tip + d * t;
        let diff = p - closest;
        (closest, diff.norm_squared())
    }
}

/// A pocket: its mouth, its facings and its shelf.
#[derive(Clone, Copy, Debug)]
pub struct PocketGeometry {
    pub kind: PocketKind,
    /// Midpoint of the mouth line (the jaw tips' midpoint).
    pub mouth_centre: Vec2,
    /// Unit vector pointing from the playing surface into the pocket.
    pub centreline: Vec2,
    /// Width of the mouth, jaw tip to jaw tip.
    pub mouth_width: f64,
    /// Angle of each facing away from the centreline.
    pub facing_angle: f64,
    /// Depth of the shelf behind the mouth line.
    pub shelf_depth: f64,
    /// The two facings.
    pub jaws: [JawGeometry; 2],
}

impl PocketGeometry {
    /// The signed distance of a point from the mouth line: negative on the table side,
    /// positive inside the pocket.
    pub fn mouth_distance(&self, p: Vec2) -> f64 {
        (p - self.mouth_centre).dot(self.centreline)
    }

    /// The signed lateral coordinate along the mouth line.
    pub fn lateral(&self, p: Vec2) -> f64 {
        let perp = Vec2::new(-self.centreline.y, self.centreline.x);
        (p - self.mouth_centre).dot(perp)
    }
}

/// The cushion contact geometry derived from the nose profile (§5.1).
#[derive(Clone, Copy, Debug)]
pub struct CushionGeometry {
    /// `eps = h_nose - R`: the contact height above the ball centre.
    pub epsilon: f64,
    /// `sin(theta)`.
    pub sin_theta: f64,
    /// `cos(theta)`.
    pub cos_theta: f64,
    /// `theta` in radians.
    pub theta: f64,
}

/// All geometry derived from a [`TableConfig`].
#[derive(Clone, Debug)]
pub struct TableGeometry {
    pub cushion: CushionGeometry,
    pub rails: [RailGeometry; 4],
    pub pockets: Vec<PocketGeometry>,
    /// Height of the rail's top edge above the bed.
    pub rail_top_height: f64,
}

/// The corner pockets, described by their corner point, the centreline that runs into
/// the pocket and the two rail directions that meet there.
struct CornerSpec {
    corner: Vec2,
    centreline: Vec2,
    rail_x: Vec2,
    rail_y: Vec2,
}

impl TableGeometry {
    /// Builds the geometry from the configuration.
    pub fn new(cfg: &TableConfig) -> TableGeometry {
        let theta = cfg.contact_tilt();
        let cushion = CushionGeometry {
            epsilon: cfg.contact_epsilon(),
            sin_theta: libm::sin(theta),
            cos_theta: libm::cos(theta),
            theta,
        };

        let l = cfg.table.length;
        let w = cfg.table.width;

        // The corner pocket's jaw tips sit on the two cushion nose lines that meet at
        // the corner, at a distance `mouth / sqrt(2)` from it (§9.1).
        let a_corner = cfg.table.mouth_corner / 2.0_f64.sqrt();
        let s_half = cfg.table.mouth_side / 2.0;

        let corner_facing = facing_angle(
            cfg.table.corner_facing_angle_deg,
            cfg.table.corner_cut_angle_deg,
            45.0, // the corner's centreline is 45 degrees from either rail
        );
        let side_facing = facing_angle(
            cfg.table.side_facing_angle_deg,
            cfg.table.side_cut_angle_deg,
            90.0, // the side pocket's centreline is perpendicular to its rail
        );

        let corner_specs = [
            CornerSpec {
                corner: Vec2::new(0.0, 0.0),
                centreline: Vec2::new(-1.0, -1.0),
                rail_x: Vec2::new(1.0, 0.0),
                rail_y: Vec2::new(0.0, 1.0),
            },
            CornerSpec {
                corner: Vec2::new(l, 0.0),
                centreline: Vec2::new(1.0, -1.0),
                rail_x: Vec2::new(-1.0, 0.0),
                rail_y: Vec2::new(0.0, 1.0),
            },
            CornerSpec {
                corner: Vec2::new(0.0, w),
                centreline: Vec2::new(-1.0, 1.0),
                rail_x: Vec2::new(1.0, 0.0),
                rail_y: Vec2::new(0.0, -1.0),
            },
            CornerSpec {
                corner: Vec2::new(l, w),
                centreline: Vec2::new(1.0, 1.0),
                rail_x: Vec2::new(-1.0, 0.0),
                rail_y: Vec2::new(0.0, -1.0),
            },
        ];

        let mut pockets = Vec::with_capacity(6);
        for spec in &corner_specs {
            let centreline = unit(spec.centreline);
            let tip_x = spec.corner + spec.rail_x * a_corner;
            let tip_y = spec.corner + spec.rail_y * a_corner;
            let dir_x = facing_dir(centreline, corner_facing, tip_x, spec.corner);
            let dir_y = facing_dir(centreline, corner_facing, tip_y, spec.corner);
            // The usual funnel has the two facings converge on a single throat. With a
            // floored (or otherwise zero) facing they run parallel and there is none:
            // then each facing runs its own fixed length instead of both sharing a
            // point that lies on only one of their two lines.
            let (end_x, end_y) = match intersect_lines(tip_x, dir_x, tip_y, dir_y) {
                Some(throat) => (throat, throat),
                None => (tip_x + dir_x * 0.05, tip_y + dir_y * 0.05),
            };
            let mouth_centre = (tip_x + tip_y) * 0.5;
            pockets.push(PocketGeometry {
                kind: PocketKind::Corner,
                mouth_centre,
                centreline,
                mouth_width: cfg.table.mouth_corner,
                facing_angle: corner_facing,
                shelf_depth: cfg.table.shelf_depth_corner,
                jaws: [
                    JawGeometry {
                        tip: tip_x,
                        end: end_x,
                        normal: facing_normal(dir_x),
                    },
                    JawGeometry {
                        tip: tip_y,
                        end: end_y,
                        normal: facing_normal(dir_y),
                    },
                ],
            });
        }

        // Side pockets: the centreline is perpendicular to the rail and the mouth line
        // lies on the rail.
        let side_specs = [
            (Vec2::new(l / 2.0, 0.0), Vec2::new(0.0, -1.0)),
            (Vec2::new(l / 2.0, w), Vec2::new(0.0, 1.0)),
        ];
        for (centre, centreline) in side_specs {
            let centreline = unit(centreline);
            let perp = Vec2::new(-centreline.y, centreline.x);
            let tip_a = centre + perp * s_half;
            let tip_b = centre - perp * s_half;
            let dir_a = facing_dir(centreline, side_facing, tip_a, centre);
            let dir_b = facing_dir(centreline, side_facing, tip_b, centre);
            // As at the corners: with no throat (a floored or zero facing) each facing
            // runs its own fixed length.
            let (end_a, end_b) = match intersect_lines(tip_a, dir_a, tip_b, dir_b) {
                Some(throat) => (throat, throat),
                None => (tip_a + dir_a * 0.05, tip_b + dir_b * 0.05),
            };
            pockets.push(PocketGeometry {
                kind: PocketKind::Side,
                mouth_centre: centre,
                centreline,
                mouth_width: cfg.table.mouth_side,
                facing_angle: side_facing,
                shelf_depth: cfg.table.shelf_depth_side,
                jaws: [
                    JawGeometry {
                        tip: tip_a,
                        end: end_a,
                        normal: facing_normal(dir_a),
                    },
                    JawGeometry {
                        tip: tip_b,
                        end: end_b,
                        normal: facing_normal(dir_b),
                    },
                ],
            });
        }

        let bottom_spans = vec![
            (a_corner, l / 2.0 - s_half),
            (l / 2.0 + s_half, l - a_corner),
        ];
        let side_spans = vec![(a_corner, w - a_corner)];

        let rails = [
            // bottom rail, y = 0; the rail is towards -y
            RailGeometry {
                normal: Vec2::new(0.0, -1.0),
                tangent: Vec2::new(1.0, 0.0),
                line_coord: 0.0,
                cushion_spans: bottom_spans.clone(),
            },
            // top rail, y = width
            RailGeometry {
                normal: Vec2::new(0.0, 1.0),
                tangent: Vec2::new(1.0, 0.0),
                line_coord: w,
                cushion_spans: bottom_spans,
            },
            // left rail, x = 0
            RailGeometry {
                normal: Vec2::new(-1.0, 0.0),
                tangent: Vec2::new(0.0, 1.0),
                line_coord: 0.0,
                cushion_spans: side_spans.clone(),
            },
            // right rail, x = length
            RailGeometry {
                normal: Vec2::new(1.0, 0.0),
                tangent: Vec2::new(0.0, 1.0),
                line_coord: l,
                cushion_spans: side_spans,
            },
        ];

        TableGeometry {
            cushion,
            rails,
            pockets,
            rail_top_height: cfg.cushion.rail_top_height,
        }
    }

    /// The playing surface half-extents, used by the off-table detection (§10.1).
    pub fn bounds(&self, cfg: &TableConfig) -> (f64, f64) {
        (cfg.table.length, cfg.table.width)
    }

    /// True when the ball's centre is past a cushion nose line and outside every
    /// pocket mouth: the ball has left the playing surface (§10.1).
    ///
    /// The crossing test in the simulation notices a ball at the instant it
    /// passes a rail, but a ball can end up past the rail without ever raising
    /// that event - at the seam between the nose rubber and the rail structure,
    /// for instance. Position is the backstop: once the centre is past a line,
    /// the ball is off the table unless it is inside a pocket mouth, which the
    /// pocket state machine owns (§9.2). The mouth's lateral allowance grows by
    /// a radius so that a ball whose edge still overlaps a jaw is given to the
    /// pocket rather than declared off the table.
    pub(crate) fn is_off_surface(&self, p: Vec2, radius: f64) -> bool {
        let past_a_line = self
            .rails
            .iter()
            .any(|rail| p.dot(rail.normal) > rail.line_coord);
        if !past_a_line {
            return false;
        }
        !self.pockets.iter().any(|pocket| {
            let depth = pocket.mouth_distance(p);
            depth > -radius
                && depth <= pocket.shelf_depth + radius
                && pocket.lateral(p).abs() <= pocket.mouth_width / 2.0 + radius
        })
    }

    /// The geometry as JSON, for a renderer (§12.2).
    ///
    /// The renderer draws the table from *this*, not from its own idea of what a
    /// pool table looks like, so that the picture and the physics cannot drift
    /// apart: the rails, the pocket mouths and the facings the model uses are the
    /// ones on the screen.
    pub fn to_json_value(&self, cfg: &TableConfig) -> crate::json::Value {
        use crate::json::Value;
        fn pair(v: Vec2) -> Value {
            Value::Array(vec![Value::Number(v.x), Value::Number(v.y)])
        }
        Value::Object(vec![
            ("length".into(), Value::Number(cfg.table.length)),
            ("width".into(), Value::Number(cfg.table.width)),
            ("ball_radius".into(), Value::Number(cfg.ball.radius)),
            (
                "cushion".into(),
                Value::Object(vec![
                    ("epsilon".into(), Value::Number(self.cushion.epsilon)),
                    ("theta".into(), Value::Number(self.cushion.theta)),
                    ("cos_theta".into(), Value::Number(self.cushion.cos_theta)),
                    (
                        "rail_top_height".into(),
                        Value::Number(self.rail_top_height),
                    ),
                ]),
            ),
            (
                "rails".into(),
                Value::Array(
                    self.rails
                        .iter()
                        .enumerate()
                        .map(|(index, rail)| {
                            Value::Object(vec![
                                ("label".into(), Value::String(rail_label(index).into())),
                                ("normal".into(), pair(rail.normal)),
                                ("tangent".into(), pair(rail.tangent)),
                                ("line".into(), Value::Number(rail.line_coord)),
                                (
                                    "spans".into(),
                                    Value::Array(
                                        rail.cushion_spans
                                            .iter()
                                            .map(|(lo, hi)| {
                                                Value::Array(vec![
                                                    Value::Number(*lo),
                                                    Value::Number(*hi),
                                                ])
                                            })
                                            .collect(),
                                    ),
                                ),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "pockets".into(),
                Value::Array(
                    self.pockets
                        .iter()
                        .enumerate()
                        .map(|(index, pocket)| {
                            Value::Object(vec![
                                ("label".into(), Value::String(pocket_label(index).into())),
                                (
                                    "kind".into(),
                                    Value::String(
                                        match pocket.kind {
                                            PocketKind::Corner => "corner",
                                            PocketKind::Side => "side",
                                        }
                                        .into(),
                                    ),
                                ),
                                ("mouth_centre".into(), pair(pocket.mouth_centre)),
                                ("centreline".into(), pair(pocket.centreline)),
                                ("width".into(), Value::Number(pocket.mouth_width)),
                                ("facing_angle".into(), Value::Number(pocket.facing_angle)),
                                ("shelf_depth".into(), Value::Number(pocket.shelf_depth)),
                                (
                                    "jaws".into(),
                                    Value::Array(
                                        pocket
                                            .jaws
                                            .iter()
                                            .map(|jaw| {
                                                Value::Object(vec![
                                                    ("tip".into(), pair(jaw.tip)),
                                                    ("end".into(), pair(jaw.end)),
                                                    ("normal".into(), pair(jaw.normal)),
                                                ])
                                            })
                                            .collect(),
                                    ),
                                ),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }
}

/// The unit vector of `v`, or the zero vector when `v` has no direction.
fn unit(v: Vec2) -> Vec2 {
    v.normalize().unwrap_or(Vec2::ZERO)
}

/// The name of a rail, in the order the core keeps them.
fn rail_label(index: usize) -> &'static str {
    match index {
        0 => "bottom",
        1 => "top",
        2 => "left",
        _ => "right",
    }
}

/// The name of a pocket, in the order the core keeps them: the corners first,
/// then the sides.
fn pocket_label(index: usize) -> &'static str {
    match index {
        0 => "bottom-left corner",
        1 => "bottom-right corner",
        2 => "top-left corner",
        3 => "top-right corner",
        4 => "bottom side",
        _ => "top side",
    }
}

/// The facing angle of a pocket, in radians.
///
/// The WPA specifies the pocket's *cut angle*, and the step from the cut angle to the
/// direction of the facing is a reading rather than a specification (§9.1). The default
/// reading is that the cut angle is the angle between a facing and its own rail line: the
/// facing is then `180 - cut` from that rail line measured into the pocket, and
/// `rail_angle - (180 - cut)` from the pocket's centreline, where `rail_angle` is the
/// angle between the rail and the centreline — 45° at a corner, 90° at a side pocket. (The
/// other reading, where the two facings meet at the throat at the cut angle, gives a 76° V
/// at a side pocket, which is not how jaws look; §9.1 rejects it as the default.) The
/// derived angle floors at zero: below `180 - rail_angle` the reading would point the
/// facings out of the pocket, and a floor keeps them parallel instead (a cut angle that
/// low is not a pocket the WPA rules describe). A measured table can be matched by giving
/// the facing angle directly, which is what a non-zero `override` does.
fn facing_angle(override_deg: f64, cut_angle_deg: f64, rail_angle_deg: f64) -> f64 {
    if override_deg > 0.0 {
        override_deg.to_radians()
    } else {
        // The reading as such is only meaningful while the facing still converges on
        // the throat: below `180 - rail_angle` (135° at a corner, 90° at a side) the
        // facing would point back out of the pocket, its normal towards the table.
        // There is no pocket to model there (§9.1), so the derivation floors at a
        // parallel slot instead of building backwards jaws.
        (rail_angle_deg - (180.0 - cut_angle_deg))
            .max(0.0)
            .to_radians()
    }
}

/// The direction of a facing: the centreline tilted by the facing angle towards the
/// pocket's centre line, so that the two facings converge on the throat.
fn facing_dir(centreline: Vec2, facing: f64, tip: Vec2, centre: Vec2) -> Vec2 {
    let perp = Vec2::new(-centreline.y, centreline.x);
    let lateral = (tip - centre).dot(perp);
    // `rotate(centreline, +facing)` has a positive lateral component, so a tip on the
    // positive side needs the negative rotation to converge.
    let sign = if lateral >= 0.0 { -1.0 } else { 1.0 };
    unit(centreline.rotated(sign * facing))
}

/// The inward normal of a facing: perpendicular to the facing, pointing towards the
/// pocket's interior.
fn facing_normal(dir: Vec2) -> Vec2 {
    Vec2::new(dir.y, -dir.x)
}

/// Intersection of two infinite lines, `p1 + s*d1` and `p2 + t*d2`. Parallel
/// lines have no intersection; the caller decides what a facing pair without a
/// throat looks like.
fn intersect_lines(p1: Vec2, d1: Vec2, p2: Vec2, d2: Vec2) -> Option<Vec2> {
    let denom = d1.cross(d2);
    if denom.abs() < 1e-12 {
        return None;
    }
    let diff = p2 - p1;
    let s = diff.cross(d2) / denom;
    Some(p1 + d1 * s)
}

/// Convenience: the 3D position of a point on the bed.
pub fn bed_position(p: Vec2, radius: f64) -> Vec3 {
    Vec3::new(p.x, p.y, radius)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corner_pocket_tips_lie_on_the_rails() {
        let cfg = TableConfig::default();
        let geom = TableGeometry::new(&cfg);
        let a = cfg.table.mouth_corner / 2.0_f64.sqrt();
        let pocket = &geom.pockets[0];
        assert!((pocket.jaws[0].tip.x - a).abs() < 1e-12);
        assert!(pocket.jaws[0].tip.y.abs() < 1e-12);
        assert!(pocket.jaws[1].tip.x.abs() < 1e-12);
        assert!((pocket.jaws[1].tip.y - a).abs() < 1e-12);
        let width = (pocket.jaws[0].tip - pocket.jaws[1].tip).norm();
        assert!((width - cfg.table.mouth_corner).abs() < 1e-12);
    }

    #[test]
    fn corner_pocket_facings_converge_at_the_throat() {
        let cfg = TableConfig::default();
        let geom = TableGeometry::new(&cfg);
        let pocket = &geom.pockets[0];
        assert!((pocket.jaws[0].end - pocket.jaws[1].end).norm() < 1e-9);
        // The throat is well behind the mouth line.
        assert!(pocket.mouth_distance(pocket.jaws[0].end) > cfg.table.shelf_depth_corner);
        // The facings are tilted by the facing angle away from the centreline:
        // 45 - (180 - 142) = 7 degrees (the default reading of section 9.1).
        let d = pocket.jaws[0].direction();
        let c = pocket.centreline;
        let cos = d.dot(c);
        let facing = (cfg.table.corner_cut_angle_deg - 135.0).to_radians();
        assert!((cos - facing.cos()).abs() < 1e-12);
    }

    #[test]
    fn the_facing_angle_can_be_given_directly_for_a_measured_table() {
        // The facing angle is derived from the specified cut angle by default (§9.1),
        // and a measured table is matched by giving the angle itself.
        let cfg = TableConfig::default();
        let geom = TableGeometry::new(&cfg);
        let pocket = &geom.pockets[0];
        let c = pocket.centreline;
        let d = pocket.jaws[0].direction();
        let derived = (cfg.table.corner_cut_angle_deg - 135.0).to_radians();
        assert!((d.dot(c) - derived.cos()).abs() < 1e-12);

        let mut measured = TableConfig::default();
        measured.table.corner_facing_angle_deg = 19.0;
        let geom = TableGeometry::new(&measured);
        let pocket = &geom.pockets[0];
        let c = pocket.centreline;
        let d = pocket.jaws[0].direction();
        assert!((d.dot(c) - 19.0_f64.to_radians().cos()).abs() < 1e-12);
    }

    #[test]
    fn a_cut_angle_below_the_reading_floor_leaves_a_parallel_slot() {
        // Below 135 degrees (corner) / 90 degrees (side) the rail-line reading would
        // point the facings back out of the pocket (section 9.1). The derivation
        // floors there, so the facings run along the centreline instead of diverging.
        let mut cfg = TableConfig::default();
        cfg.table.corner_cut_angle_deg = 110.0;
        cfg.table.side_cut_angle_deg = 80.0;
        let geom = TableGeometry::new(&cfg);
        let corner = &geom.pockets[0];
        for jaw in &corner.jaws {
            let d = jaw.direction();
            assert!(
                (d.dot(corner.centreline) - 1.0).abs() < 1e-12,
                "a corner facing diverged from the centreline: {d:?} against {:?}",
                corner.centreline
            );
        }
        let side = &geom.pockets[4];
        for jaw in &side.jaws {
            let d = jaw.direction();
            assert!(
                (d.dot(side.centreline) - 1.0).abs() < 1e-12,
                "a side facing diverged from the centreline: {d:?} against {:?}",
                side.centreline
            );
        }
    }

    #[test]
    fn a_ball_past_a_rail_is_off_the_surface() {
        let cfg = TableConfig::default();
        let geom = TableGeometry::new(&cfg);
        let radius = cfg.ball.radius;
        let middle = Vec2::new(cfg.table.length / 2.0, cfg.table.width / 2.0);
        assert!(!geom.is_off_surface(middle, radius));
        assert!(geom.is_off_surface(Vec2::new(cfg.table.length + radius, middle.y), radius));
        // A point off the middle of the bottom rail, away from the side pocket.
        assert!(geom.is_off_surface(Vec2::new(cfg.table.length / 4.0, -radius), radius));
    }

    #[test]
    fn a_ball_in_a_pocket_mouth_is_not_off_the_surface() {
        // The mouths reach past the nose lines: a ball in one belongs to the
        // pocket state machine, not to the off-table rule (§10.1).
        let cfg = TableConfig::default();
        let geom = TableGeometry::new(&cfg);
        let side = &geom.pockets[4];
        let in_mouth = side.mouth_centre + side.centreline * (side.shelf_depth / 2.0);
        assert!(!geom.is_off_surface(in_mouth, cfg.ball.radius));
        // Far enough beyond the shelf to be in the cabinet rather than a mouth.
        let past_mouth =
            side.mouth_centre + side.centreline * (side.shelf_depth + 2.0 * cfg.ball.radius);
        assert!(geom.is_off_surface(past_mouth, cfg.ball.radius));
    }

    #[test]
    fn side_pocket_mouth_is_on_the_rail() {
        let cfg = TableConfig::default();
        let geom = TableGeometry::new(&cfg);
        let pocket = &geom.pockets[4];
        assert!(pocket.jaws[0].tip.y.abs() < 1e-12);
        assert!(pocket.jaws[1].tip.y.abs() < 1e-12);
        let width = (pocket.jaws[0].tip - pocket.jaws[1].tip).norm();
        assert!((width - cfg.table.mouth_side).abs() < 1e-12);
        assert!((pocket.jaws[0].end - pocket.jaws[1].end).norm() < 1e-9);
    }
}
