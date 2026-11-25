//! The table configuration: every tunable parameter of the physics core lives here
//! (§12.2, §13.3). No magic numbers are allowed in the physics path, and the cushion
//! contact height `eps` is *derived* from the nose geometry rather than stored
//! (§5.1, §13.3).

use crate::json::Value;

/// Reads a number from a JSON object.
fn f64_field(value: &Value, key: &str) -> Result<f64, String> {
    value
        .field(key)?
        .as_f64()
        .ok_or_else(|| format!("'{key}' must be a number"))
}

/// Reads a list of number pairs from a JSON array.
fn pairs_field(value: &Value, key: &str) -> Result<Vec<(f64, f64)>, String> {
    let items = value
        .field(key)?
        .as_array()
        .ok_or_else(|| format!("'{key}' must be an array"))?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let pair = item
            .as_array()
            .ok_or_else(|| format!("every entry of '{key}' must be a pair"))?;
        if pair.len() != 2 {
            return Err(format!("every entry of '{key}' must be a pair"));
        }
        let a = pair[0]
            .as_f64()
            .ok_or_else(|| format!("every entry of '{key}' must be a pair of numbers"))?;
        let b = pair[1]
            .as_f64()
            .ok_or_else(|| format!("every entry of '{key}' must be a pair of numbers"))?;
        out.push((a, b));
    }
    Ok(out)
}

/// Writes a list of number pairs as a JSON array.
fn pairs_value(pairs: &[(f64, f64)]) -> Value {
    Value::Array(
        pairs
            .iter()
            .map(|(a, b)| Value::Array(vec![Value::Number(*a), Value::Number(*b)]))
            .collect(),
    )
}

/// Ball properties.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BallConfig {
    /// Ball radius, m (WPA: 2.25" diameter).
    pub radius: f64,
    /// Ball mass, kg (WPA: 6 oz).
    pub mass: f64,
    /// `I = inertia_factor * m * R^2` (solid sphere: 2/5).
    pub inertia_factor: f64,
}

impl Default for BallConfig {
    fn default() -> Self {
        BallConfig {
            radius: 0.028575,
            mass: 0.170,
            inertia_factor: 2.0 / 5.0,
        }
    }
}

/// The cloth (bed) friction model of §3.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClothConfig {
    /// Sliding (kinetic) ball-cloth friction, `mu_s`. Never the static coefficient.
    pub mu_s: f64,
    /// Rolling friction, `mu_r`.
    pub mu_r: f64,
    /// Phenomenological spin decay coefficient, `mu_sp` (§2.4).
    pub mu_sp: f64,
}

impl Default for ClothConfig {
    fn default() -> Self {
        ClothConfig {
            mu_s: 0.20,
            mu_r: 0.01,
            mu_sp: 0.0117,
        }
    }
}

/// The ball-ball collision model of §4.4.5.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BallBallConfig {
    /// Coefficient of restitution between two balls.
    pub e_ball: f64,
    /// Ball-ball sliding friction, `mu_ball`, at a sliding speed of 1 m/s.
    ///
    /// The friction between the balls falls as the contact slides faster (TP A.14),
    /// so the model scales this value with the sliding speed - see
    /// [`crate::ball_ball::friction_at_speed`].
    ///
    /// The default is TP A.14's own fit value at the reference slip speed
    /// (`0.009951 + 0.108 * exp(-0.77)` = 0.05996, rounded), so the law in force is
    /// the published fit *unscaled*. An earlier default of 0.05 rescaled the whole
    /// law by 0.83 and manufactured about 0.7 degrees of the throw deviation that
    /// §14.3 then recorded (doc revision 1.8).
    pub mu_ball: f64,
}

impl Default for BallBallConfig {
    fn default() -> Self {
        BallBallConfig {
            e_ball: 0.95,
            mu_ball: 0.06,
        }
    }
}

/// The cushion model of §5.1 together with the three rail regimes of §11.3.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CushionConfig {
    /// Effective ball-rail restitution (`e_cushion`).
    pub e_cushion: f64,
    /// Ball-rail sliding friction (`mu_cushion`).
    pub mu_cushion: f64,
    /// Height above the bed at which the cushion makes contact with a ball resting on
    /// the bed: the WPA specifies 63.5% of the ball diameter (36.3 mm).
    ///
    /// The contact height above the ball centre (`eps`) and the contact tilt (`theta`)
    /// are derived from this, never stored (§5.1, §13.3).
    pub nose_height: f64,
    /// Radius of the nose tip, m (1/4"). Shape only; it does not set the impulse
    /// direction.
    pub nose_tip_radius: f64,
    /// Height of the rail's top edge above the bed (the nose plus ~15 mm).
    pub rail_top_height: f64,
    /// Restitution of the rail structure above the nose rubber (regime 2, §11.3).
    pub e_rail_structure: f64,
    /// Friction of the rail structure above the nose rubber (regime 2).
    pub mu_rail_structure: f64,
}

impl Default for CushionConfig {
    fn default() -> Self {
        CushionConfig {
            e_cushion: 0.75,
            mu_cushion: 0.14,
            nose_height: 0.0363,
            nose_tip_radius: 0.00635,
            rail_top_height: 0.0513,
            e_rail_structure: 0.50,
            mu_rail_structure: 0.20,
        }
    }
}

/// The slate (table body) model of §7.2 and §8.2.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlateConfig {
    /// Coefficient of restitution of the slate.
    pub e_table: f64,
    /// Below this vertical speed the ball settles on the bed instead of bouncing.
    pub settle_speed: f64,
    /// The largest upward velocity the slate gives back to a ball that a cushion has
    /// pressed into it, m/s.
    ///
    /// The rigid model of §8.1 over-predicts the hop at break speed - the document
    /// says so itself: "Real cushion deformation limits this: the rubber compresses,
    /// the contact is not rigid, and the ball is not launched as cleanly. The rigid
    /// model therefore over-predicts the hop at break speeds." This is the
    /// compliant-contact correction the document asks for. The default leaves the
    /// behaviour the `cushion_rebound` fixture pins untouched (a 2 m/s cushion hit
    /// rebounds at 0.27 m/s) and keeps a break-speed hit to a couple of centimetres
    /// instead of the half metre the uncorrected model would give.
    ///
    /// It also has to be small enough that the hop does not carry the ball over
    /// the rail: a ball whose centre rises above the top of the rail has left the
    /// table. The centre starts a radius above the bed, so the hop may be at most
    /// `rail_top_height - radius` = 22.7 mm, which is a rebound of
    /// `sqrt(2 g h)` = 0.67 m/s. The default is below that.
    pub max_cushion_rebound: f64,
}

impl Default for SlateConfig {
    fn default() -> Self {
        SlateConfig {
            e_table: 0.60,
            settle_speed: 0.02,
            max_cushion_rebound: 0.65,
        }
    }
}

/// The playing surface dimensions and the pocket geometry of §9.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableGeometryConfig {
    /// Playing surface length between the cushion noses, m (100").
    pub length: f64,
    /// Playing surface width between the cushion noses, m (50").
    pub width: f64,
    /// Corner pocket mouth, jaw tip to jaw tip (4.56").
    pub mouth_corner: f64,
    /// Side pocket mouth, jaw tip to jaw tip (5.06").
    pub mouth_side: f64,
    /// Corner pocket cut angle (142 deg), the angle the WPA specifies.
    pub corner_cut_angle_deg: f64,
    /// Side pocket cut angle (104 deg), the angle the WPA specifies.
    pub side_cut_angle_deg: f64,
    /// The corner pocket's facing angle from the centreline, in degrees.
    ///
    /// Zero means "derive it from the cut angle", which is the default rule of §9.1:
    /// the cut angle is read as the angle between a facing and its own rail line, and
    /// the corner's centreline is 45° from either rail, so the facing is
    /// `cut angle - 135` from the centreline — 7° for the WPA's 142°. The derivation
    /// floors at a parallel slot: below a 135° cut the reading would point the facings
    /// out of the pocket. The derivation is a *reading* of the specified cut angle,
    /// and a table that has been measured can be matched by giving the facing angle
    /// directly.
    pub corner_facing_angle_deg: f64,
    /// The side pocket's facing angle from the centreline, in degrees. Zero means
    /// "derive it from the cut angle" (see `corner_facing_angle_deg`; the side
    /// pocket's centreline is perpendicular to its rail, so the rule gives
    /// `cut angle - 90` — 14° for the WPA's 104°).
    pub side_facing_angle_deg: f64,
    /// Slate cut angle at the lip, from vertical (13.5 deg).
    pub pocket_angle_deg: f64,
    /// Shelf depth behind a corner pocket mouth (1.75").
    pub shelf_depth_corner: f64,
    /// Shelf depth behind a side pocket mouth (0.25").
    pub shelf_depth_side: f64,
    /// Radius of the slate's drop point (1/8" - 1/4").
    pub drop_radius: f64,
    /// Jaw restitution (`e_jaw`).
    pub e_jaw: f64,
    /// Jaw friction (`mu_jaw`).
    pub mu_jaw: f64,
    /// Sliding friction of the pocket shelf (`mu_slate`). Only a ball that is still
    /// sliding when it crosses the mouth uses this; a rolling ball keeps `cloth.mu_r`
    /// (§9.4).
    pub mu_slate: f64,
}

impl Default for TableGeometryConfig {
    fn default() -> Self {
        TableGeometryConfig {
            length: 2.54,
            width: 1.27,
            mouth_corner: 4.56 * 0.0254,
            mouth_side: 5.06 * 0.0254,
            corner_cut_angle_deg: 142.0,
            side_cut_angle_deg: 104.0,
            corner_facing_angle_deg: 0.0,
            side_facing_angle_deg: 0.0,
            pocket_angle_deg: 13.5,
            shelf_depth_corner: 1.75 * 0.0254,
            shelf_depth_side: 0.25 * 0.0254,
            drop_radius: 0.25 * 0.0254,
            e_jaw: 0.60,
            mu_jaw: 0.30,
            mu_slate: 0.15,
        }
    }
}

/// The cue model of §6.
#[derive(Clone, Debug, PartialEq)]
pub struct CueConfig {
    /// Tip coefficient of restitution (`e_cue`).
    pub e_cue: f64,
    /// Tip-ball friction, used for the miscue limit (§6.3).
    pub mu_cue: f64,
    /// Total cue mass, kg (16 oz). Used by the §6.2 linear-velocity formula.
    pub cue_mass: f64,
    /// Effective cue-end mass, kg (~12 g). This - never the total mass - is the mass
    /// that belongs in a squirt calculation (§6.1).
    pub cue_end_mass: f64,
    /// The length of the cue, m (>= 40"). It is only used to place the cue in the
    /// state that the renderer draws.
    pub cue_length: f64,
    /// Global multiplier on the squirt calibration curve.
    pub squirt_scale: f64,
    /// The squirt calibration curve: `(b/R, degrees)` anchors, interpolated linearly
    /// (§6.1). Nearly linear up to `b = 0.5R`, flattening beyond.
    pub squirt_curve: Vec<(f64, f64)>,
}

impl Default for CueConfig {
    fn default() -> Self {
        CueConfig {
            e_cue: 0.75,
            mu_cue: 0.60,
            cue_mass: 0.454,
            cue_end_mass: 0.012,
            cue_length: 1.45,
            squirt_scale: 1.0,
            // Nearly linear up to 0.5 R and flattening beyond, in the 1-5 deg band that
            // the published measurements show for a maximum english (§6.1).
            squirt_curve: vec![
                (0.0, 0.0),
                (0.1, 0.55),
                (0.2, 1.10),
                (0.3, 1.65),
                (0.4, 2.15),
                (0.5, 2.60),
                (0.7, 3.10),
                (0.9, 3.40),
                (1.0, 3.50),
            ],
        }
    }
}

/// Simulation-wide knobs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SimulationConfig {
    /// Gravity, m/s^2.
    pub gravity: f64,
    /// Hard limit on the simulated time, s.
    pub max_sim_time: f64,
    /// Hard limit on the number of resolved events (protects against a pathological
    /// event loop; it is not a physics parameter).
    pub max_events: u32,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        SimulationConfig {
            gravity: 9.81,
            max_sim_time: 30.0,
            max_events: 200_000,
        }
    }
}

/// Everything the physics core needs to know about the table and the balls.
///
/// Serialisable so that a table can be stored, diffed and replayed (Milestone 0
/// acceptance: the config round-trips through JSON).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableConfig {
    pub ball: BallConfig,
    pub cloth: ClothConfig,
    pub ball_ball: BallBallConfig,
    pub cushion: CushionConfig,
    pub slate: SlateConfig,
    pub table: TableGeometryConfig,
    pub cue: CueConfig,
    pub simulation: SimulationConfig,
}

impl TableConfig {
    /// The contact height above the ball centre, `eps = h_nose - R` (§5.1).
    ///
    /// This is an *output* of the geometry, never a stored parameter (§13.3).
    pub fn contact_epsilon(&self) -> f64 {
        self.cushion.nose_height - self.ball.radius
    }

    /// The tilt of the contact normal, `theta = asin(eps / R)` (§5.1).
    pub fn contact_tilt(&self) -> f64 {
        let s = self.contact_epsilon() / self.ball.radius;
        // `asin` is only defined for |s| <= 1; a nose height outside that range is a
        // misconfiguration and is clamped rather than producing a NaN (ground rule 6).
        let s = s.clamp(-1.0, 1.0);
        libm::asin(s)
    }

    /// The moment of inertia of a ball, `I = (2/5) m R^2`.
    pub fn moment_of_inertia(&self) -> f64 {
        self.ball.inertia_factor * self.ball.mass * self.ball.radius * self.ball.radius
    }

    /// The minimum speed a *rolling* ball needs to cross a pocket shelf of the given
    /// depth (§9.4): `sqrt(2 * mu_r * g * shelf_depth)`. A ball that is still sliding
    /// when it crosses the mouth uses its sliding coefficient instead.
    pub fn minimum_speed_to_drop(&self, shelf_depth: f64) -> f64 {
        let v2 = 2.0 * self.cloth.mu_r * self.simulation.gravity * shelf_depth;
        if v2 <= 0.0 {
            0.0
        } else {
            v2.sqrt()
        }
    }

    /// The largest impact parameter the tip can hold without miscueing (§6.3):
    /// `b <= R * mu_cue / sqrt(1 + mu_cue^2)`.
    pub fn max_offset_without_miscue(&self) -> f64 {
        let mu = self.cue.mu_cue;
        self.ball.radius * mu / (1.0 + mu * mu).sqrt()
    }

    /// The configuration as JSON.
    ///
    /// The round trip is exact: the numbers are written in the shortest form that
    /// round-trips and read back with a correctly rounded parser (§11.4).
    pub fn to_json(&self) -> String {
        self.to_json_value().to_json()
    }

    /// Reads a configuration from JSON.
    pub fn from_json(text: &str) -> Result<TableConfig, String> {
        let value = crate::json::parse(text).map_err(|error| error.to_string())?;
        TableConfig::from_json_value(&value)
    }

    /// The configuration as a JSON value.
    pub fn to_json_value(&self) -> Value {
        Value::Object(vec![
            ("ball".into(), self.ball.to_json_value()),
            ("cloth".into(), self.cloth.to_json_value()),
            ("ball_ball".into(), self.ball_ball.to_json_value()),
            ("cushion".into(), self.cushion.to_json_value()),
            ("slate".into(), self.slate.to_json_value()),
            ("table".into(), self.table.to_json_value()),
            ("cue".into(), self.cue.to_json_value()),
            ("simulation".into(), self.simulation.to_json_value()),
        ])
    }

    /// Reads a configuration from a JSON value.
    pub fn from_json_value(value: &Value) -> Result<TableConfig, String> {
        Ok(TableConfig {
            ball: BallConfig::from_json_value(value.field("ball")?)?,
            cloth: ClothConfig::from_json_value(value.field("cloth")?)?,
            ball_ball: BallBallConfig::from_json_value(value.field("ball_ball")?)?,
            cushion: CushionConfig::from_json_value(value.field("cushion")?)?,
            slate: SlateConfig::from_json_value(value.field("slate")?)?,
            table: TableGeometryConfig::from_json_value(value.field("table")?)?,
            cue: CueConfig::from_json_value(value.field("cue")?)?,
            simulation: SimulationConfig::from_json_value(value.field("simulation")?)?,
        })
    }
}

impl BallConfig {
    /// The ball configuration as a JSON value.
    pub fn to_json_value(&self) -> Value {
        Value::Object(vec![
            ("radius".into(), Value::Number(self.radius)),
            ("mass".into(), Value::Number(self.mass)),
            ("inertia_factor".into(), Value::Number(self.inertia_factor)),
        ])
    }

    /// Reads the ball configuration from a JSON value.
    pub fn from_json_value(value: &Value) -> Result<BallConfig, String> {
        Ok(BallConfig {
            radius: f64_field(value, "radius")?,
            mass: f64_field(value, "mass")?,
            inertia_factor: f64_field(value, "inertia_factor")?,
        })
    }
}

impl ClothConfig {
    /// The cloth configuration as a JSON value.
    pub fn to_json_value(&self) -> Value {
        Value::Object(vec![
            ("mu_s".into(), Value::Number(self.mu_s)),
            ("mu_r".into(), Value::Number(self.mu_r)),
            ("mu_sp".into(), Value::Number(self.mu_sp)),
        ])
    }

    /// Reads the cloth configuration from a JSON value.
    pub fn from_json_value(value: &Value) -> Result<ClothConfig, String> {
        Ok(ClothConfig {
            mu_s: f64_field(value, "mu_s")?,
            mu_r: f64_field(value, "mu_r")?,
            mu_sp: f64_field(value, "mu_sp")?,
        })
    }
}

impl BallBallConfig {
    /// The ball-ball configuration as a JSON value.
    pub fn to_json_value(&self) -> Value {
        Value::Object(vec![
            ("e_ball".into(), Value::Number(self.e_ball)),
            ("mu_ball".into(), Value::Number(self.mu_ball)),
        ])
    }

    /// Reads the ball-ball configuration from a JSON value.
    pub fn from_json_value(value: &Value) -> Result<BallBallConfig, String> {
        Ok(BallBallConfig {
            e_ball: f64_field(value, "e_ball")?,
            mu_ball: f64_field(value, "mu_ball")?,
        })
    }
}

impl CushionConfig {
    /// The cushion configuration as a JSON value.
    pub fn to_json_value(&self) -> Value {
        Value::Object(vec![
            ("e_cushion".into(), Value::Number(self.e_cushion)),
            ("mu_cushion".into(), Value::Number(self.mu_cushion)),
            ("nose_height".into(), Value::Number(self.nose_height)),
            (
                "nose_tip_radius".into(),
                Value::Number(self.nose_tip_radius),
            ),
            (
                "rail_top_height".into(),
                Value::Number(self.rail_top_height),
            ),
            (
                "e_rail_structure".into(),
                Value::Number(self.e_rail_structure),
            ),
            (
                "mu_rail_structure".into(),
                Value::Number(self.mu_rail_structure),
            ),
        ])
    }

    /// Reads the cushion configuration from a JSON value.
    pub fn from_json_value(value: &Value) -> Result<CushionConfig, String> {
        Ok(CushionConfig {
            e_cushion: f64_field(value, "e_cushion")?,
            mu_cushion: f64_field(value, "mu_cushion")?,
            nose_height: f64_field(value, "nose_height")?,
            nose_tip_radius: f64_field(value, "nose_tip_radius")?,
            rail_top_height: f64_field(value, "rail_top_height")?,
            e_rail_structure: f64_field(value, "e_rail_structure")?,
            mu_rail_structure: f64_field(value, "mu_rail_structure")?,
        })
    }
}

impl SlateConfig {
    /// The slate configuration as a JSON value.
    pub fn to_json_value(&self) -> Value {
        Value::Object(vec![
            ("e_table".into(), Value::Number(self.e_table)),
            ("settle_speed".into(), Value::Number(self.settle_speed)),
            (
                "max_cushion_rebound".into(),
                Value::Number(self.max_cushion_rebound),
            ),
        ])
    }

    /// Reads the slate configuration from a JSON value.
    pub fn from_json_value(value: &Value) -> Result<SlateConfig, String> {
        Ok(SlateConfig {
            e_table: f64_field(value, "e_table")?,
            settle_speed: f64_field(value, "settle_speed")?,
            max_cushion_rebound: f64_field(value, "max_cushion_rebound")?,
        })
    }
}

impl TableGeometryConfig {
    /// The table geometry configuration as a JSON value.
    pub fn to_json_value(&self) -> Value {
        Value::Object(vec![
            ("length".into(), Value::Number(self.length)),
            ("width".into(), Value::Number(self.width)),
            ("mouth_corner".into(), Value::Number(self.mouth_corner)),
            ("mouth_side".into(), Value::Number(self.mouth_side)),
            (
                "corner_cut_angle_deg".into(),
                Value::Number(self.corner_cut_angle_deg),
            ),
            (
                "side_cut_angle_deg".into(),
                Value::Number(self.side_cut_angle_deg),
            ),
            (
                "corner_facing_angle_deg".into(),
                Value::Number(self.corner_facing_angle_deg),
            ),
            (
                "side_facing_angle_deg".into(),
                Value::Number(self.side_facing_angle_deg),
            ),
            (
                "pocket_angle_deg".into(),
                Value::Number(self.pocket_angle_deg),
            ),
            (
                "shelf_depth_corner".into(),
                Value::Number(self.shelf_depth_corner),
            ),
            (
                "shelf_depth_side".into(),
                Value::Number(self.shelf_depth_side),
            ),
            ("drop_radius".into(), Value::Number(self.drop_radius)),
            ("e_jaw".into(), Value::Number(self.e_jaw)),
            ("mu_jaw".into(), Value::Number(self.mu_jaw)),
            ("mu_slate".into(), Value::Number(self.mu_slate)),
        ])
    }

    /// Reads the table geometry configuration from a JSON value.
    pub fn from_json_value(value: &Value) -> Result<TableGeometryConfig, String> {
        Ok(TableGeometryConfig {
            length: f64_field(value, "length")?,
            width: f64_field(value, "width")?,
            mouth_corner: f64_field(value, "mouth_corner")?,
            mouth_side: f64_field(value, "mouth_side")?,
            corner_cut_angle_deg: f64_field(value, "corner_cut_angle_deg")?,
            side_cut_angle_deg: f64_field(value, "side_cut_angle_deg")?,
            corner_facing_angle_deg: f64_field(value, "corner_facing_angle_deg")?,
            side_facing_angle_deg: f64_field(value, "side_facing_angle_deg")?,
            pocket_angle_deg: f64_field(value, "pocket_angle_deg")?,
            shelf_depth_corner: f64_field(value, "shelf_depth_corner")?,
            shelf_depth_side: f64_field(value, "shelf_depth_side")?,
            drop_radius: f64_field(value, "drop_radius")?,
            e_jaw: f64_field(value, "e_jaw")?,
            mu_jaw: f64_field(value, "mu_jaw")?,
            mu_slate: f64_field(value, "mu_slate")?,
        })
    }
}

impl CueConfig {
    /// The cue configuration as a JSON value.
    pub fn to_json_value(&self) -> Value {
        Value::Object(vec![
            ("e_cue".into(), Value::Number(self.e_cue)),
            ("mu_cue".into(), Value::Number(self.mu_cue)),
            ("cue_mass".into(), Value::Number(self.cue_mass)),
            ("cue_end_mass".into(), Value::Number(self.cue_end_mass)),
            ("cue_length".into(), Value::Number(self.cue_length)),
            ("squirt_scale".into(), Value::Number(self.squirt_scale)),
            ("squirt_curve".into(), pairs_value(&self.squirt_curve)),
        ])
    }

    /// Reads the cue configuration from a JSON value.
    pub fn from_json_value(value: &Value) -> Result<CueConfig, String> {
        Ok(CueConfig {
            e_cue: f64_field(value, "e_cue")?,
            mu_cue: f64_field(value, "mu_cue")?,
            cue_mass: f64_field(value, "cue_mass")?,
            cue_end_mass: f64_field(value, "cue_end_mass")?,
            cue_length: f64_field(value, "cue_length")?,
            squirt_scale: f64_field(value, "squirt_scale")?,
            squirt_curve: pairs_field(value, "squirt_curve")?,
        })
    }
}

impl SimulationConfig {
    /// The simulation configuration as a JSON value.
    pub fn to_json_value(&self) -> Value {
        Value::Object(vec![
            ("gravity".into(), Value::Number(self.gravity)),
            ("max_sim_time".into(), Value::Number(self.max_sim_time)),
            ("max_events".into(), Value::Number(self.max_events as f64)),
        ])
    }

    /// Reads the simulation configuration from a JSON value.
    pub fn from_json_value(value: &Value) -> Result<SimulationConfig, String> {
        Ok(SimulationConfig {
            gravity: f64_field(value, "gravity")?,
            max_sim_time: f64_field(value, "max_sim_time")?,
            max_events: f64_field(value, "max_events")? as u32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trips_through_json() {
        let cfg = TableConfig::default();
        let json = cfg.to_json();
        let back = TableConfig::from_json(&json).expect("deserialize");
        assert_eq!(cfg, back);
        // The round trip is exact, bit for bit.
        assert_eq!(back.to_json(), json);
        assert_eq!(
            back.table.shelf_depth_corner.to_bits(),
            cfg.table.shelf_depth_corner.to_bits()
        );
    }

    #[test]
    fn contact_geometry_is_derived_not_stored() {
        let cfg = TableConfig::default();
        let eps = cfg.contact_epsilon();
        assert!((eps - 0.007725).abs() < 1e-12);
        assert!((eps / cfg.ball.radius - 0.27034120734908135).abs() < 1e-12);
        let theta_deg = cfg.contact_tilt().to_degrees();
        assert!((theta_deg - 15.684571679006801).abs() < 1e-9);
    }

    #[test]
    fn miscue_limit_is_51_percent_of_the_radius() {
        let cfg = TableConfig::default();
        let b = cfg.max_offset_without_miscue();
        assert!((b / cfg.ball.radius - 0.5144957554275265).abs() < 1e-12);
    }
}
