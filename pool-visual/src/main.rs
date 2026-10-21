//! `pool-visual` - the visual simulation of the physics core.
//!
//! The core is event-driven and closed-form: between two events every ball's
//! state is given by an analytic expression. This tool takes that path and puts
//! it on the screen.
//!
//! ```text
//! pool-visual list                       # the demo scenes
//! pool-visual shot --scene break         # writes break.html
//! pool-visual trace --scene cut          # writes the raw trace as JSON
//! ```
//!
//! The page is a single file with no external reference: it opens from the disk.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::process::ExitCode;

use pool_core::math::Vec3;
use pool_core::{try_simulate_shot_traced, BallState, ShotInput, TableConfig};
use pool_visual::scenes;
use pool_visual::viewer::{self, ShotView};

/// The parsed command line.
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

    fn list(&self, name: &str) -> Vec<String> {
        self.repeated.get(name).cloned().unwrap_or_default()
    }
}

fn usage() {
    println!(
        "pool-visual - the visual simulation of the pool physics core

USAGE:
    pool-visual shot [options]
    pool-visual trace [options]
    pool-visual demos [options]
    pool-visual list
    pool-visual help

COMMON OPTIONS:
    --scene NAME       the demo scene: {}
    --out FILE         where to write (default: <scene>.html / <scene>.json)
    --title TEXT       the title of the page

DEMOS OPTIONS:
    --out DIR          where to write the gallery (default: demos)

CUSTOM SHOT OPTIONS (used when --scene is 'custom'):
    --cue X,Y          the cue ball's position
    --object X,Y       an object ball's position (repeatable)
    --cue-speed S      the cue speed in m/s
    --aim-deg A        the aim direction in degrees
    --offset-r B       the side offset in ball radii (english)
    --spin-r B         the vertical offset in ball radii (top and bottom spin)
",
        scenes::NAMES.join(", ")
    );
}

/// A point of the form `x,y`.
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

/// Builds a custom shot from the command line.
fn custom_input(cfg: &TableConfig, args: &Args) -> Result<ShotInput, String> {
    let cue = match args.flags.get("cue") {
        Some(v) => parse_point(v)?,
        None => (cfg.table.length / 4.0, cfg.table.width / 2.0),
    };
    let mut objects: Vec<(f64, f64)> = Vec::new();
    for text in args.list("object") {
        objects.push(parse_point(&text)?);
    }
    if objects.is_empty() {
        objects.push((cue.0 + 0.5, cue.1));
    }
    let aim_deg = args.get_f64("aim-deg", 0.0)?;
    let speed = args.get_f64("cue-speed", 2.0)?;
    let offset_r = args.get_f64("offset-r", 0.0)?;
    let spin_r = args.get_f64("spin-r", 0.0)?;

    let mut balls: Vec<BallState> = Vec::new();
    balls.push(BallState::at_rest(
        cfg,
        0,
        Vec3::new(cue.0, cue.1, cfg.ball.radius),
    ));
    for (index, (x, y)) in objects.iter().enumerate() {
        balls.push(BallState::at_rest(
            cfg,
            (index + 1) as u8,
            Vec3::new(*x, *y, cfg.ball.radius),
        ));
    }
    let mut input = ShotInput::new(speed, scenes::aim_direction(aim_deg), balls);
    input.contact_offset_x = offset_r * cfg.ball.radius;
    input.contact_offset_y = spin_r * cfg.ball.radius;
    Ok(input)
}

/// The scene the command line asks for.
fn build_scene(cfg: &TableConfig, args: &Args) -> Result<scenes::Scene, String> {
    let name = args.get("scene", "break");
    if name == "custom" {
        let input = custom_input(cfg, args)?;
        let note = format!(
            "cue speed {:.2} m/s, aim {:.1}\u{b0}",
            args.get_f64("cue-speed", 2.0)?,
            args.get_f64("aim-deg", 0.0)?
        );
        let title = args.get("title", "Custom shot");
        return Ok(scenes::custom_shot(&title, input, note));
    }
    scenes::by_name(cfg, &name)
        .ok_or_else(|| format!("unknown scene '{name}'; try 'pool-visual list'"))
}

/// Simulates every shot of the scene and collects the paths.
fn build_shots(cfg: &TableConfig, scene: &scenes::Scene) -> Result<Vec<ShotView>, String> {
    let mut shots = Vec::new();
    for shot in &scene.shots {
        let (state, trace) = try_simulate_shot_traced(&shot.input, cfg)
            .map_err(|error| format!("the shot '{}' failed: {}", shot.name, error.message))?;
        shots.push(ShotView {
            name: shot.name.clone(),
            note: shot.note.clone(),
            description: shot.description.clone(),
            trace,
            state,
        });
    }
    Ok(shots)
}

/// Writes a string to a file, or to the standard output.
fn write_out(args: &Args, default_name: &str, text: &str) -> Result<(), String> {
    match args.flags.get("out") {
        Some(path) => {
            std::fs::write(path, text).map_err(|error| format!("could not write {path}: {error}"))
        }
        None => {
            if default_name.is_empty() {
                print!("{text}");
                Ok(())
            } else {
                std::fs::write(default_name, text)
                    .map_err(|error| format!("could not write {default_name}: {error}"))
            }
        }
    }
}

fn command_shot(args: &Args) -> Result<String, String> {
    let cfg = TableConfig::default();
    let scene = build_scene(&cfg, args)?;
    let shots = build_shots(&cfg, &scene)?;
    let title = args.get("title", &scene.title);
    let page = viewer::html(&cfg, &title, &scene.subtitle, &shots);
    write_out(args, &format!("{}.html", scene.name), &page)?;
    Ok(format!(
        "wrote {} shot(s) of '{}' to {}",
        shots.len(),
        scene.name,
        args.flags
            .get("out")
            .cloned()
            .unwrap_or_else(|| format!("{}.html", scene.name))
    ))
}

fn command_trace(args: &Args) -> Result<String, String> {
    let cfg = TableConfig::default();
    let scene = build_scene(&cfg, args)?;
    let shots = build_shots(&cfg, &scene)?;
    let document = viewer::document(&cfg, &scene.title, &scene.subtitle, &shots);
    let text = document.to_json();
    write_out(args, &format!("{}.json", scene.name), &text)?;
    Ok(format!(
        "wrote {} shot(s) of '{}' to {}",
        shots.len(),
        scene.name,
        args.flags
            .get("out")
            .cloned()
            .unwrap_or_else(|| format!("{}.json", scene.name))
    ))
}

/// Writes the whole gallery: one page per scene, and an index that links them.
fn command_demos(args: &Args) -> Result<String, String> {
    let cfg = TableConfig::default();
    let directory = args.get("out", "demos");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create {directory}: {error}"))?;

    let mut entries = Vec::new();
    let mut written = Vec::new();
    for name in scenes::NAMES {
        let scene = scenes::by_name(&cfg, name).ok_or_else(|| format!("no scene '{name}'"))?;
        let shots = build_shots(&cfg, &scene)?;
        let page = viewer::html(&cfg, &scene.title, &scene.subtitle, &shots);
        let path = format!("{directory}/{name}.html");
        std::fs::write(&path, page).map_err(|error| format!("could not write {path}: {error}"))?;
        written.push(path);
        entries.push(viewer::IndexEntry {
            file: format!("{name}.html"),
            title: scene.title.clone(),
            subtitle: scene.subtitle.clone(),
            shots: scene
                .shots
                .iter()
                .map(|shot| viewer::IndexShot {
                    name: shot.name.clone(),
                    note: shot.note.clone(),
                })
                .collect(),
        });
    }

    let index = format!("{directory}/index.html");
    std::fs::write(&index, viewer::index(&entries))
        .map_err(|error| format!("could not write {index}: {error}"))?;
    written.push(index);

    Ok(format!("wrote {} page(s) to {directory}/", written.len()))
}

fn command_list() -> Result<String, String> {
    let cfg = TableConfig::default();
    let mut out = String::new();
    for name in scenes::NAMES {
        let scene = scenes::by_name(&cfg, name).ok_or_else(|| format!("no scene '{name}'"))?;
        let _ = writeln!(out, "{name:10} {}", scene.subtitle);
        for shot in &scene.shots {
            let _ = writeln!(out, "           - {}: {}", shot.name, shot.note);
        }
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
    let result = match args.command.as_str() {
        "shot" => command_shot(&args),
        "trace" => command_trace(&args),
        "demos" => command_demos(&args),
        "list" => command_list(),
        "help" | "--help" | "-h" => {
            usage();
            return ExitCode::SUCCESS;
        }
        other => Err(format!("unknown command '{other}'")),
    };
    match result {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
