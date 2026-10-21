//! The demo scenes.
//!
//! A scene is a small set of shots that answer one question about the model:
//! what a break looks like, what spin does to the cue ball, how much the object
//! ball throws, where the squirt points, how the cushion answers the incidence
//! angle and what a hop off the slate looks like.
//!
//! Every scene is deterministic: the rack is a fixed order, not a random one, so
//! two runs of `pool-visual` produce the same file.

use pool_core::math::Vec3;
use pool_core::{BallState, ShotInput, TableConfig};

/// One shot of a scene.
pub struct Shot {
    /// The name of the shot, as it appears in the viewer.
    pub name: String,
    /// A one line note, shown under the name.
    pub note: String,
    /// What the shot is meant to show.
    pub description: String,
    /// The shot itself.
    pub input: ShotInput,
}

/// A scene: the shots that answer one question.
pub struct Scene {
    /// The name of the scene.
    pub name: &'static str,
    /// The title of the scene.
    pub title: String,
    /// The subtitle of the scene.
    pub subtitle: String,
    /// The shots of the scene.
    pub shots: Vec<Shot>,
}

/// The names of the scenes, in the order they are listed.
pub const NAMES: [&str; 6] = ["break", "trio", "cut", "english", "cushion", "jump"];

/// The scene with the given name, if there is one.
pub fn by_name(cfg: &TableConfig, name: &str) -> Option<Scene> {
    match name {
        "break" => Some(break_scene(cfg)),
        "trio" => Some(trio_scene(cfg)),
        "cut" => Some(cut_scene(cfg)),
        "english" => Some(english_scene(cfg)),
        "cushion" => Some(cushion_scene(cfg)),
        "jump" => Some(jump_scene(cfg)),
        _ => None,
    }
}

/// A cue ball at rest.
fn cue_ball(cfg: &TableConfig, x: f64, y: f64) -> BallState {
    BallState::at_rest(cfg, 0, Vec3::new(x, y, cfg.ball.radius))
}

/// An object ball at rest.
fn object_ball(cfg: &TableConfig, number: u8, x: f64, y: f64) -> BallState {
    BallState::at_rest(cfg, number, Vec3::new(x, y, cfg.ball.radius))
}

/// The direction of a shot at `degrees`, measured from `+x`.
fn direction(degrees: f64) -> Vec3 {
    let radians = degrees.to_radians();
    Vec3::new(radians.cos(), radians.sin(), 0.0)
}

/// The aim that makes up for the model's squirt.
///
/// The core applies the squirt as a *horizontal* deflection of `ψ` whatever the
/// direction of the contact offset is (§6.1 of the model), so a shot with a
/// vertical offset - a draw or a follow - leaves the line by `ψ`. A player aims
/// off to make up for it, and so do these scenes: what they are meant to show is
/// the spin, not the squirt. The squirt has a scene of its own (`english`).
fn squirt_corrected(cfg: &TableConfig, degrees: f64, impact_r: f64) -> Vec3 {
    let psi = pool_core::cue::squirt_angle(impact_r, cfg).to_radians();
    let radians = degrees.to_radians() + psi;
    Vec3::new(radians.cos(), radians.sin(), 0.0)
}

/// The break: a full rack, the cue ball into the head ball.
///
/// The rack is the standard triangle with the 1 at the apex and the 8 in the
/// middle of the third row, and a hair of clearance between the balls so that the
/// rack is not already touching.
fn break_scene(cfg: &TableConfig) -> Scene {
    let length = cfg.table.length;
    let width = cfg.table.width;
    let radius = cfg.ball.radius;
    let clearance = 0.0004;
    let spacing = 2.0 * radius + clearance;
    let row_step = spacing * (30.0f64).to_radians().cos();

    // The standard order: the 1 at the apex, the 8 in the middle, the corners of
    // the back row a solid and a stripe.
    let rows: [&[u8]; 5] = [
        &[1],
        &[2, 3],
        &[11, 8, 12],
        &[4, 5, 13, 6],
        &[7, 14, 15, 9, 10],
    ];

    let mut balls = vec![cue_ball(cfg, length / 4.0, width / 2.0)];
    let apex_x = length * 0.75;
    for (row, numbers) in rows.iter().enumerate() {
        let x = apex_x + row as f64 * row_step;
        for (slot, number) in numbers.iter().enumerate() {
            let y = width / 2.0 + (slot as f64 - row as f64 / 2.0) * spacing;
            balls.push(object_ball(cfg, *number, x, y));
        }
    }

    let mut input = ShotInput::new(8.0, squirt_corrected(cfg, 0.0, 0.1), balls);
    input.contact_offset_y = 0.1 * radius;

    Scene {
        name: "break",
        title: "The break".into(),
        subtitle: "a full rack, the cue ball into the head ball".into(),
        shots: vec![Shot {
            name: "break".into(),
            note: "cue speed 6.0 m/s".into(),
            description: "The cue ball strikes the apex of the rack head on. The \
                          burst, the cushions and the drops are all events of the \
                          core, resolved in their total order."
                .into(),
            input,
        }],
    }
}

/// The same shot with three different spins.
///
/// This is the clearest picture of what the spin does: the object ball goes the
/// same way in all three, and the cue ball ends up somewhere else entirely.
fn trio_scene(cfg: &TableConfig) -> Scene {
    let radius = cfg.ball.radius;
    let cue = (0.95, cfg.table.width / 2.0);
    let object = (1.5, cfg.table.width / 2.0);
    let variants: [(&str, &str, f64); 3] = [
        ("draw", "bottom spin, -0.45 R", -0.45),
        ("stun", "centre ball, no spin", 0.0),
        ("follow", "top spin, +0.45 R", 0.45),
    ];
    let mut shots = Vec::new();
    for (name, note, spin) in variants {
        let mut input = ShotInput::new(
            2.2,
            squirt_corrected(cfg, 0.0, spin.abs()),
            vec![
                cue_ball(cfg, cue.0, cue.1),
                object_ball(cfg, 1, object.0, object.1),
            ],
        );
        input.contact_offset_y = spin * radius;
        shots.push(Shot {
            name: name.into(),
            note: note.into(),
            description: match name {
                "draw" => {
                    "The cue ball comes back towards the shooter: the bottom \
                           spin survives the collision, and the friction of the \
                           cloth turns it into a backwards roll."
                }
                "stun" => {
                    "The cue ball has picked up a little natural roll on the \
                           way to the object ball, so it stops at the contact and \
                           then creeps forward: a stun shot that is not quite dead \
                           in the pocket."
                }
                _ => {
                    "The cue ball follows the object ball forward: the top spin \
                      survives the collision and drives the ball on."
                }
            }
            .into(),
            input,
        });
    }
    Scene {
        name: "trio",
        title: "Draw, stun, follow".into(),
        subtitle: "one shot, three spins - the cue ball goes three different ways \
                   (the aim makes up for the model's squirt)"
            .into(),
        shots,
    }
}

/// Three cut angles, the same distance.
///
/// The object ball is thrown off the line of centres by the friction of the
/// collision, and the throw grows with the cut angle.
fn cut_scene(cfg: &TableConfig) -> Scene {
    let cue = (0.7, 0.45);
    let distance = 0.5;
    let mut shots = Vec::new();
    for angle in [15.0f64, 30.0, 45.0] {
        let radians = angle.to_radians();
        let object = (
            cue.0 + distance * radians.cos(),
            cue.1 + distance * radians.sin(),
        );
        let input = ShotInput::new(
            2.5,
            Vec3::X,
            vec![
                cue_ball(cfg, cue.0, cue.1),
                object_ball(cfg, 1, object.0, object.1),
            ],
        );
        shots.push(Shot {
            name: format!("cut {angle:.0}\u{b0}"),
            note: format!("cut angle {angle:.0}\u{b0}, 0.5 m"),
            description: format!(
                "A {angle:.0} degree cut. The object ball leaves along the line of \
                 centres, but the friction of the collision throws it a little to \
                 one side - and the throw is a property of the contact, not of the \
                 aim."
            ),
            input,
        });
    }
    Scene {
        name: "cut",
        title: "The cut and the throw".into(),
        subtitle: "three cut angles, the same shot - watch the object ball's path".into(),
        shots,
    }
}

/// The same shot with three amounts of side spin.
///
/// The cue ball does not leave along the cue's line: it squirts away from the
/// side the tip struck, and the cloth then curves it the rest of the way.
fn english_scene(cfg: &TableConfig) -> Scene {
    let radius = cfg.ball.radius;
    let mut shots = Vec::new();
    for (name, note, offset) in [
        ("right english", "the tip on the shooter's right, -0.4 R", -0.4),
        ("centre", "no side spin", 0.0),
        ("left english", "the tip on the shooter's left, +0.4 R", 0.4),
    ] {
        let mut input = ShotInput::new(
            3.0,
            Vec3::X,
            vec![cue_ball(cfg, 0.5, cfg.table.width / 2.0)],
        );
        input.contact_offset_x = offset * radius;
        shots.push(Shot {
            name: name.into(),
            note: note.into(),
            description: "The cue ball leaves along the cue's axis, but the tip \
                          pushes it to one side on the strike (the squirt, §6.1). \
                          The spin that is left over then curves it on the cloth."
                .into(),
            input,
        });
    }
    Scene {
        name: "english",
        title: "The squirt".into(),
        subtitle: "three amounts of side spin - the launch line is not the aim line".into(),
        shots,
    }
}

/// Four incidence angles into the same rail.
///
/// The Han model of §5.1 is not symmetric: the rebound angle depends on how
/// squarely the ball hits the rubber.
fn cushion_scene(cfg: &TableConfig) -> Scene {
    let mut shots = Vec::new();
    for incidence in [20.0f64, 35.0, 50.0, 65.0] {
        let radians = incidence.to_radians();
        // The angle is measured from the rail's normal, and the ball goes down
        // towards the bottom rail: it has to meet the rubber clear of the side
        // pocket in the middle of that rail.
        let dir = Vec3::new(-radians.sin(), -radians.cos(), 0.0);
        let input = ShotInput::new(3.0, dir, vec![cue_ball(cfg, 1.9, 0.22)]);
        shots.push(Shot {
            name: format!("{incidence:.0}\u{b0} incidence"),
            note: format!("{incidence:.0}\u{b0} from the normal"),
            description: format!(
                "The ball meets the bottom rail at {incidence:.0} degrees from the \
                 normal. The contact happens above the centre of the ball, so the \
                 rebound is shorter than the arrival and it comes off at a different \
                 angle."
            ),
            input,
        });
    }
    Scene {
        name: "cushion",
        title: "The cushion".into(),
        subtitle: "four incidence angles into the same rail".into(),
        shots,
    }
}

/// An elevated cue: the ball is driven into the slate and the slate launches it.
fn jump_scene(cfg: &TableConfig) -> Scene {
    let elevation = 10.0f64.to_radians();
    let direction = Vec3::new(elevation.cos(), 0.0, -elevation.sin());
    let input = ShotInput::new(
        4.0,
        direction,
        vec![
            cue_ball(cfg, 0.7, cfg.table.width / 2.0),
            object_ball(cfg, 1, 1.7, cfg.table.width / 2.0),
        ],
    );
    Scene {
        name: "jump",
        title: "The hop".into(),
        subtitle: "an elevated cue drives the ball into the slate".into(),
        shots: vec![Shot {
            name: "jump".into(),
            note: "10\u{b0} elevation, cue speed 4.0 m/s".into(),
            description: "The cue is elevated, so the ball is driven into the slate \
                          instead of along the bed. The slate's rebound launches it \
                          (§7.2): the ball is airborne, and it lands further on. \
                          Watch its shadow: it separates from the ball while the \
                          ball is in the air."
                .into(),
            input,
        }],
    }
}

/// A custom shot, built from the command line.
pub fn custom_shot(name: &str, input: ShotInput, note: String) -> Scene {
    Scene {
        name: "custom",
        title: name.to_string(),
        subtitle: note.clone(),
        shots: vec![Shot {
            name: name.to_string(),
            note,
            description: "A shot built from the command line.".into(),
            input,
        }],
    }
}

/// The direction of a shot, for the command line.
pub fn aim_direction(degrees: f64) -> Vec3 {
    direction(degrees)
}
