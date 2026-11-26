//! `pool-cli` - a headless tuning harness for the physics core.
//!
//! The harness exists so that the uncalibrated parameters of §13.3 can be fitted
//! against data instead of by feel: it fires parameterised shots and writes the
//! results as CSV.
//!
//! ```text
//! pool-cli simulate --cue-speed 2 --aim-deg 0 --object 0.9,0.635
//! pool-cli simulate --cue-speed 9 --elev-deg 35 --spin-r -0.4   # the game's jump
//! pool-cli sweep --kind speed --from 0.5 --to 6 --steps 12 --out speed.csv
//! pool-cli sweep --kind english --from 0 --to 0.5 --steps 6 --out english.csv
//! pool-cli sweep --kind cut --from 0 --to 60 --steps 13 --out throw.csv
//! pool-cli corpus --out tests/fixtures/corpus.json
//! ```

mod corpus;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::process::ExitCode;

use pool_core::math::Vec3;
use pool_core::{simulate_shot, BallState, ShotInput, TableConfig};

struct Args {
    command: String,
    flags: BTreeMap<String, String>,
    repeated: BTreeMap<String, Vec<String>>,
}

impl Args {
    fn parse() -> Result<Args, String> {
        let mut iter = std::env::args().skip(1);
        let command = iter.next().unwrap_or_else(|| "help".to_string());
        let mut flags = BTreeMap::new();
        let mut repeated: BTreeMap<String, Vec<String>> = BTreeMap::new();
        while let Some(flag) = iter.next() {
            let (name, value) = match flag.split_once('=') {
                Some((n, v)) => (n.trim_start_matches('-').to_string(), v.to_string()),
                None => {
                    let name = flag.trim_start_matches('-').to_string();
                    let value = iter.next().unwrap_or_default();
                    (name, value)
                }
            };
            repeated
                .entry(name.clone())
                .or_default()
                .push(value.clone());
            flags.insert(name, value);
        }
        Ok(Args {
            command,
            flags,
            repeated,
        })
    }

    fn get(&self, name: &str, default: &str) -> String {
        self.flags
            .get(name)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    fn get_f64(&self, name: &str, default: f64) -> Result<f64, String> {
        match self.flags.get(name) {
            None => Ok(default),
            Some(v) => v
                .parse::<f64>()
                .map_err(|_| format!("--{name} expects a number, got '{v}'")),
        }
    }

    fn get_usize(&self, name: &str, default: usize) -> Result<usize, String> {
        match self.flags.get(name) {
            None => Ok(default),
            Some(v) => v
                .parse::<usize>()
                .map_err(|_| format!("--{name} expects a number, got '{v}'")),
        }
    }

    fn list(&self, name: &str) -> Vec<String> {
        self.repeated.get(name).cloned().unwrap_or_default()
    }
}

fn usage() {
    println!(
        "pool-cli - headless tuning harness for the pool physics core

USAGE:
    pool-cli config
    pool-cli simulate [options]
    pool-cli sweep --kind <speed|english|cut> [options]
    pool-cli corpus [--out FILE]
    pool-cli help

COMMON OPTIONS:
    --cue X,Y        the cue ball's position (default 0.635,0.635)
    --object X,Y     an object ball's position (repeatable)
    --cue-speed S    the cue speed in m/s (default 2.0)
    --aim-deg A      the aim direction in degrees (default 0)
    --elev-deg E     the cue raised E degrees above the level (default 0)
    --offset-r B     the side offset in ball radii (english, default 0)
    --spin-r B       the vertical offset in ball radii (default 0)
    --out FILE       write the CSV to FILE instead of stdout

SWEEP OPTIONS:
    --kind K         speed | english | cut | cushion | hop
    --from A --to B --steps N
    --cut-deg C      the cut angle used by the 'speed' and 'cut' sweeps
    --distance D     the distance to the object ball used by the sweeps

The sweeps follow the calibration order of the document (section 13.3): the cloth
friction first, then the spin decay, the nose height, the cushion restitution, the
pocket parameters, the squirt curve and finally the ball-ball constants.
"
    );
}

fn parse_point(text: &str) -> Result<(f64, f64), String> {
    let mut parts = text.split(',');
    let x = parts
        .next()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .ok_or_else(|| format!("expected 'x,y', got '{text}'"))?;
    let y = parts
        .next()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .ok_or_else(|| format!("expected 'x,y', got '{text}'"))?;
    Ok((x, y))
}

// The eight numbers a stroke can be asked for are the eight numbers a stroke
// is: the same handful the panel shows, and none of them a detail of the CLI
// nobody asked for.
#[allow(clippy::too_many_arguments)]
fn build_shot(
    cfg: &TableConfig,
    cue_position: (f64, f64),
    objects: &[(f64, f64)],
    aim_deg: f64,
    elev_deg: f64,
    speed: f64,
    offset_r: f64,
    spin_r: f64,
) -> ShotInput {
    let mut balls: Vec<BallState> = Vec::new();
    balls.push(BallState::at_rest(
        cfg,
        0,
        Vec3::new(cue_position.0, cue_position.1, cfg.ball.radius),
    ));
    for (i, (x, y)) in objects.iter().enumerate() {
        let number = (i + 1) as u8;
        balls.push(BallState::at_rest(
            cfg,
            number,
            Vec3::new(*x, *y, cfg.ball.radius),
        ));
    }
    let radians = aim_deg.to_radians();
    // The elevation is baked into the direction rather than passed as an angle:
    // §12.3 says the core never calls `sin`/`cos`, so the trigonometry stays on
    // this side of the deterministic boundary. The core's `z` is up, and a cue
    // raised at the butt points its axis DOWN at the ball, so the height of the
    // axis is `-sin` of the elevation - the way `pool-visual`'s jump scene builds
    // it, and the way §7.1 wants it: the elevated cue drives the ball into the
    // slate, and the slate is what launches it.
    let elevation = elev_deg.to_radians();
    let direction = Vec3::new(
        elevation.cos() * radians.cos(),
        elevation.cos() * radians.sin(),
        -elevation.sin(),
    );
    let mut input = ShotInput::new(speed, direction, balls);
    input.contact_offset_x = offset_r * cfg.ball.radius;
    input.contact_offset_y = spin_r * cfg.ball.radius;
    input
}

/// The default configuration as JSON - the authoritative numbers for the fixture
/// generator, which must not carry a second copy of the defaults.
fn command_config() -> Result<String, String> {
    Ok(TableConfig::default().to_json())
}

fn command_simulate(args: &Args) -> Result<String, String> {
    let cfg = TableConfig::default();
    let cue_position = match args.flags.get("cue") {
        Some(v) => parse_point(v)?,
        None => (cfg.table.length / 4.0, cfg.table.width / 2.0),
    };
    let mut objects: Vec<(f64, f64)> = Vec::new();
    for text in args.list("object") {
        objects.push(parse_point(&text)?);
    }
    if objects.is_empty() {
        objects.push((cue_position.0 + 0.5, cue_position.1));
    }
    let aim_deg = args.get_f64("aim-deg", 0.0)?;
    let elev_deg = args.get_f64("elev-deg", 0.0)?;
    let speed = args.get_f64("cue-speed", 2.0)?;
    let offset_r = args.get_f64("offset-r", 0.0)?;
    let spin_r = args.get_f64("spin-r", 0.0)?;

    let input = build_shot(
        &cfg,
        cue_position,
        &objects,
        aim_deg,
        elev_deg,
        speed,
        offset_r,
        spin_r,
    );
    let state = simulate_shot(input, &cfg);

    let mut out = String::new();
    out.push_str("ball,x,y,z,vx,vy,vz,wx,wy,wz,motion,pocket,pocketed\n");
    for ball in &state.balls {
        let _ = writeln!(
            out,
            "{},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:?},{:?},{}",
            ball.number,
            ball.position.x,
            ball.position.y,
            ball.position.z,
            ball.velocity.x,
            ball.velocity.y,
            ball.velocity.z,
            ball.angular_velocity.x,
            ball.angular_velocity.y,
            ball.angular_velocity.z,
            ball.motion_state,
            ball.pocket_state,
            u8::from(ball.is_pocketed)
        );
    }
    let _ = writeln!(
        out,
        "# elapsed={:.6} miscue={} hash={}",
        state.elapsed_time,
        state.miscue,
        pool_core::hash_state_hex(&state)
    );
    Ok(out)
}

/// The outcome of a shot at a given cut angle.
struct Outcome {
    object_speed: f64,
    separation_deg: f64,
    throw_deg: f64,
    cue_speed: f64,
    cue_x: f64,
    cue_y: f64,
}

fn shoot_at_cut(cfg: &TableConfig, speed: f64, cut_deg: f64, distance: f64) -> Outcome {
    let cut = cut_deg.to_radians();
    let cue_position = (cfg.table.length / 4.0, cfg.table.width / 2.0);
    // The cue ball shoots along +x. The object ball is placed so that the contact
    // happens with the cut angle: the line of centres then makes that angle with the
    // cue's path.
    let contact = 2.0 * cfg.ball.radius;
    let object = (
        cue_position.0 + distance - contact * cut.cos(),
        cue_position.1 + contact * cut.sin(),
    );
    // The cut sweeps are level shots: it is the elevation that makes a shot a
    // jump, and a jump does not land the separation the sweeps are fitting.
    let input = build_shot(cfg, cue_position, &[object], 0.0, 0.0, speed, 0.0, 0.0);

    // The separation and the throw are properties of the collision, so the shot is
    // stopped at the moment of the collision: the first pass finds when that is, the
    // second one runs the shot up to it.
    let mut simulation = pool_core::Simulation::new(&input, cfg);
    let _ = simulation.run(&input);
    let collision = simulation
        .events()
        .iter()
        .find(|event| event.kind == pool_core::EventKind::BallBall)
        .map(|event| event.time);

    let mut short = TableConfig::default();
    short.simulation.max_sim_time = match collision {
        Some(time) => time + 1e-9,
        None => cfg.simulation.max_sim_time,
    };
    let state = simulate_shot(input.clone(), &short);

    let cue = state.balls[0].velocity;
    let object_ball = state.balls[1].velocity;
    let separation = if cue.norm() > 1e-9 && object_ball.norm() > 1e-9 {
        (object_ball.y.atan2(object_ball.x) - cue.y.atan2(cue.x)).to_degrees()
    } else {
        0.0
    };
    let throw = if object_ball.norm() > 1e-9 {
        (object_ball.y.atan2(object_ball.x) - cut).to_degrees()
    } else {
        0.0
    };
    Outcome {
        cue_x: state.balls[0].position.x,
        cue_y: state.balls[0].position.y,
        object_speed: object_ball.norm(),
        separation_deg: separation,
        throw_deg: throw,
        cue_speed: cue.norm(),
    }
}

fn command_sweep(args: &Args) -> Result<String, String> {
    let cfg = TableConfig::default();
    let kind = args.get("kind", "speed");
    let from = args.get_f64("from", 0.0)?;
    let to = args.get_f64("to", 1.0)?;
    let steps = args.get_usize("steps", 11)?.max(2);
    let cut_deg = args.get_f64("cut-deg", 0.0)?;
    let distance = args.get_f64("distance", 0.5)?;
    let speed = args.get_f64("cue-speed", 2.0)?;

    let mut out = String::new();
    match kind.as_str() {
        "speed" => {
            out.push_str(
                "cue_speed,cue_speed_after,object_speed,separation_deg,throw_deg,cue_x,cue_y\n",
            );
            for i in 0..steps {
                let v = from + (to - from) * (i as f64) / ((steps - 1) as f64);
                let result = shoot_at_cut(&cfg, v, cut_deg, distance);
                let _ = writeln!(
                    out,
                    "{v:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
                    result.cue_speed,
                    result.object_speed,
                    result.separation_deg,
                    result.throw_deg,
                    result.cue_x,
                    result.cue_y
                );
            }
        }
        "english" => {
            out.push_str("offset_r,squirt_deg,shot_direction_deg\n");
            for i in 0..steps {
                let b = from + (to - from) * (i as f64) / ((steps - 1) as f64);
                let input = build_shot(
                    &cfg,
                    (cfg.table.length / 4.0, cfg.table.width / 2.0),
                    &[],
                    0.0,
                    // The squirt curve is fitted on level strokes: the elevation
                    // drives the ball into the slate, and the squirt is the tip
                    // sliding on the cloth side of the ball.
                    0.0,
                    speed,
                    b,
                    0.0,
                );
                let state = simulate_shot(input, &cfg);
                let v = state.balls[0].velocity;
                let direction = v.y.atan2(v.x).to_degrees();
                let _ = writeln!(out, "{b:.6},{direction:.6},{direction:.6}");
            }
        }
        "cut" => {
            out.push_str("cut_deg,throw_deg,separation_deg,object_speed\n");
            for i in 0..steps {
                let cut = from + (to - from) * (i as f64) / ((steps - 1) as f64);
                let result = shoot_at_cut(&cfg, speed, cut, distance);
                let _ = writeln!(
                    out,
                    "{cut:.6},{:.6},{:.6},{:.6}",
                    result.throw_deg, result.separation_deg, result.object_speed
                );
            }
        }
        "cushion" => {
            // The rebound against the incidence angle (§5.1).
            out.push_str("incidence_deg,rebound_speed,rebound_angle_deg,rebound_vz\n");
            let geometry = pool_core::geometry::TableGeometry::new(&cfg);
            for i in 0..steps {
                let incidence = from + (to - from) * (i as f64) / ((steps - 1) as f64);
                let theta = incidence.to_radians();
                let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(0.5, 0.5, cfg.ball.radius));
                ball.velocity = Vec3::new(speed * theta.cos(), speed * theta.sin(), 0.0);
                ball.angular_velocity = Vec3::new(
                    -ball.velocity.y / cfg.ball.radius,
                    ball.velocity.x / cfg.ball.radius,
                    0.0,
                );
                let outcome = pool_core::cushion::resolve_nose_cushion(
                    &ball,
                    Vec3::X,
                    Vec3::Y,
                    &cfg,
                    &geometry.cushion,
                );
                let rebound_speed = outcome.velocity.x.hypot(outcome.velocity.y);
                let rebound_angle = outcome.velocity.y.atan2(-outcome.velocity.x).to_degrees();
                let _ = writeln!(
                    out,
                    "{incidence:.6},{rebound_speed:.6},{rebound_angle:.6},{:.6}",
                    outcome.velocity.z
                );
            }
        }
        "hop" => {
            // The hop that a cushion drives into the slate, against the impact speed
            // (§8.1). The rigid model over-predicts it at break speeds.
            out.push_str("cushion_speed,rebound_vz,hop_mm\n");
            for i in 0..steps {
                let v = from + (to - from) * (i as f64) / ((steps - 1) as f64);
                let geometry = pool_core::geometry::TableGeometry::new(&cfg);
                let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(0.5, 0.5, cfg.ball.radius));
                ball.velocity = Vec3::new(v, 0.0, 0.0);
                ball.angular_velocity = Vec3::new(0.0, v / cfg.ball.radius, 0.0);
                let outcome = pool_core::cushion::resolve_nose_cushion(
                    &ball,
                    Vec3::X,
                    Vec3::Y,
                    &cfg,
                    &geometry.cushion,
                );
                let vz = outcome.velocity.z;
                let up = if vz > 0.0 {
                    vz
                } else {
                    cfg.slate.e_table * vz.abs()
                };
                let hop_mm = up * up / (2.0 * cfg.simulation.gravity) * 1000.0;
                let _ = writeln!(out, "{v:.6},{vz:.6},{hop_mm:.6}");
            }
        }
        other => return Err(format!("unknown sweep kind '{other}'")),
    }
    Ok(out)
}

fn main() -> ExitCode {
    let args = match Args::parse() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    // The corpus has a default destination (the checked-in fixture), so it
    // owns its write instead of going through the generic `--out` handler.
    if args.command == "corpus" {
        let (text, count) = match corpus::command_corpus() {
            Ok(result) => result,
            Err(error) => {
                eprintln!("error: {error}");
                return ExitCode::FAILURE;
            }
        };
        let path = args.flags.get("out").cloned().unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("tests")
                .join("fixtures")
                .join("corpus.json")
                .to_string_lossy()
                .into_owned()
        });
        return match std::fs::write(&path, text) {
            Ok(()) => {
                println!("wrote {count} shots to {path}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: could not write {path}: {error}");
                ExitCode::FAILURE
            }
        };
    }
    let result = match args.command.as_str() {
        "config" => command_config(),
        "simulate" => command_simulate(&args),
        "sweep" => command_sweep(&args),
        "help" | "--help" | "-h" => {
            usage();
            return ExitCode::SUCCESS;
        }
        other => Err(format!("unknown command '{other}'")),
    };
    match result {
        Ok(text) => match args.flags.get("out") {
            Some(path) => match std::fs::write(path, text) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("error: could not write {path}: {error}");
                    ExitCode::FAILURE
                }
            },
            None => {
                print!("{text}");
                ExitCode::SUCCESS
            }
        },
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
