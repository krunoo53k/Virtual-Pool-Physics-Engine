//! The deterministic self-play shot corpus (E0/E4), schema `pool-corpus/2`.
//!
//! Every shot in the self-play part starts from the previous shot's final ball
//! states. Rotations are carried from the recorded path - the canonical state
//! hash deliberately excludes them - while pocketed and off-table balls are
//! removed and a cue ball that did not survive is put back on the head spot.
//! The tail of the corpus adds hand-placed edge cases (pocket hand-off, jaws,
//! a hung-up ball, a miscue, near-touching balls and an event-limit fault) that
//! the carry chain cannot reach.
//!
//! There is no RNG and no clock: the plan is a fixed table of f64 scalars and
//! the direction comes from `pool-intent`'s `libm` conversion. The result is a
//! canonical JSON document written by the core's own writer, so the numbers
//! round-trip bit for bit into the replay test.

use pool_core::json::Value;
use pool_core::math::{Vec2, Vec3};
use pool_core::sim::{EventKind, SimError, SimEvent, Simulation};
use pool_core::state::{GamePhase, GameState, PocketState, ShotInput};
use pool_core::trace::Trace;
use pool_core::{
    geometry::TableGeometry, hash_state_hex, simulate_shot, try_simulate_shot_traced, TableConfig,
};
use pool_intent::{cue_direction, shot_input};

/// One stroke of the corpus. The six scalars are the wire intent: the two
/// contact offsets are the player's (across/right, up), which `shot_input`
/// converts to the core frame.
#[derive(Clone, Copy)]
struct Plan {
    id: &'static str,
    category: &'static str,
    speed: f64,
    level_aim_deg: f64,
    elevation_deg: f64,
    contact_x: f64,
    contact_y: f64,
    called_pocket: i32,
    rack: Rack,
    /// A per-shot event-limit override, used by the fault case.
    max_events: Option<u32>,
}

/// Where the balls of a shot come from. The first 72 shots carry the previous
/// shot's balls; the special tail is placed from the table's own geometry.
#[derive(Clone, Copy)]
enum Rack {
    Carry,
    Initial,
    CueAt(Vec3),
    Pair {
        cue: Vec3,
        object: Vec3,
    },
    /// Ball 1 is handed over on the pocket shelf, at this fraction of its depth.
    Shelf {
        depth: f64,
    },
    /// The cue ball is handed over on the shelf with too little speed to drop.
    HungUp {
        depth: f64,
    },
    /// The cue ball is placed off the centreline and aimed at a jaw.
    Jaw {
        distance: f64,
        lateral: f64,
    },
}

#[allow(clippy::too_many_arguments)]
fn plan(
    id: &'static str,
    category: &'static str,
    speed: f64,
    level_aim_deg: f64,
    elevation_deg: f64,
    contact_x: f64,
    contact_y: f64,
    called_pocket: i32,
) -> Plan {
    Plan {
        id,
        category,
        speed,
        level_aim_deg,
        elevation_deg,
        contact_x,
        contact_y,
        called_pocket,
        rack: Rack::Carry,
        max_events: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn special(
    id: &'static str,
    category: &'static str,
    speed: f64,
    level_aim_deg: f64,
    elevation_deg: f64,
    contact_x: f64,
    contact_y: f64,
    called_pocket: i32,
    rack: Rack,
    max_events: Option<u32>,
) -> Plan {
    Plan {
        id,
        category,
        speed,
        level_aim_deg,
        elevation_deg,
        contact_x,
        contact_y,
        called_pocket,
        rack,
        max_events,
    }
}

/// A point on a pocket's shelf, expressed from the pocket's own mouth line.
fn shelf_point(pocket: &pool_core::geometry::PocketGeometry, depth: f64) -> Vec3 {
    let plan = pocket.mouth_centre + pocket.centreline * depth;
    Vec3::new(plan.x, plan.y, 0.0)
}

fn plans(cfg: &TableConfig) -> Vec<Plan> {
    let geometry = TableGeometry::new(cfg);
    let corner = &geometry.pockets[0];
    // The 12-degree rattle-out of the physics acceptance test, expressed as an
    // aim in degrees so it goes over the wire like any other shot.
    let along = Vec2::new(-corner.centreline.y, corner.centreline.x);
    let angle = 12.0_f64.to_radians();
    let rattle_direction = (-along * angle.cos() + corner.centreline * angle.sin())
        .normalize()
        .expect("a non-zero rattle direction");
    let rattle_aim = rattle_direction.y.atan2(rattle_direction.x).to_degrees();
    // A centre-hit stroke's speed on the cloth, from the cue model's own numbers.
    let speed_factor = (1.0 + cfg.ball.mass / cfg.cue.cue_mass) / (1.0 + cfg.cue.e_cue);
    let slow_drop = cfg.minimum_speed_to_drop(cfg.table.shelf_depth_corner) * 0.8 * speed_factor;
    let tiny_gap = 1e-12;

    vec![
        // rails: four cushions, four cuts into each. -1 is "no called pocket".
        plan("rails-01", "rails", 1.2, 180.0, 0.0, 0.0, 0.0, -1),
        plan("rails-02", "rails", 1.6, 195.0, 0.0, 0.0, 0.0, -1),
        plan("rails-03", "rails", 2.0, 165.0, 0.0, 0.0, 0.0, -1),
        plan("rails-04", "rails", 2.4, 210.0, 0.0, 0.0, 0.0, -1),
        plan("rails-05", "rails", 1.3, 0.0, 0.0, 0.0, 0.0, -1),
        plan("rails-06", "rails", 1.7, 15.0, 0.0, 0.0, 0.0, -1),
        plan("rails-07", "rails", 2.1, 345.0, 0.0, 0.0, 0.0, -1),
        plan("rails-08", "rails", 2.5, 30.0, 0.0, 0.0, 0.0, -1),
        plan("rails-09", "rails", 1.4, 270.0, 0.0, 0.0, 0.0, -1),
        plan("rails-10", "rails", 1.8, 285.0, 0.0, 0.0, 0.0, -1),
        plan("rails-11", "rails", 2.2, 255.0, 0.0, 0.0, 0.0, -1),
        plan("rails-12", "rails", 2.6, 300.0, 0.0, 0.0, 0.0, -1),
        plan("rails-13", "rails", 1.5, 90.0, 0.0, 0.0, 0.0, -1),
        plan("rails-14", "rails", 1.9, 105.0, 0.0, 0.0, 0.0, -1),
        plan("rails-15", "rails", 2.3, 75.0, 0.0, 0.0, 0.0, -1),
        plan("rails-16", "rails", 2.7, 120.0, 0.0, 0.0, 0.0, -1),
        // spin: right english, left english, follow and draw at exact offsets.
        plan("spin-01", "spin", 2.0, 0.0, 0.0, 0.008, 0.0, -1),
        plan("spin-02", "spin", 2.2, 10.0, 0.0, 0.008, 0.0, -1),
        plan("spin-03", "spin", 2.4, 20.0, 0.0, 0.008, 0.0, -1),
        plan("spin-04", "spin", 2.6, 30.0, 0.0, 0.008, 0.0, -1),
        plan("spin-05", "spin", 2.1, 0.0, 0.0, -0.008, 0.0, -1),
        plan("spin-06", "spin", 2.3, 350.0, 0.0, -0.008, 0.0, -1),
        plan("spin-07", "spin", 2.5, 340.0, 0.0, -0.008, 0.0, -1),
        plan("spin-08", "spin", 2.7, 330.0, 0.0, -0.008, 0.0, -1),
        plan("spin-09", "spin", 2.0, 0.0, 0.0, 0.0, 0.008, -1),
        plan("spin-10", "spin", 2.5, 45.0, 0.0, 0.0, 0.008, -1),
        plan("spin-11", "spin", 3.0, 90.0, 0.0, 0.0, 0.008, -1),
        plan("spin-12", "spin", 3.5, 135.0, 0.0, 0.0, 0.008, -1),
        plan("spin-13", "spin", 2.2, 0.0, 0.0, 0.0, -0.008, -1),
        plan("spin-14", "spin", 2.7, 45.0, 0.0, 0.0, -0.008, -1),
        plan("spin-15", "spin", 3.2, 90.0, 0.0, 0.0, -0.008, -1),
        plan("spin-16", "spin", 3.7, 135.0, 0.0, 0.0, -0.008, -1),
        // jumps: the butt raise is baked into the direction by `cue_direction`.
        plan("jumps-01", "jumps", 3.0, 0.0, 12.0, 0.0, 0.0, -1),
        plan("jumps-02", "jumps", 3.5, 45.0, 18.0, 0.0, 0.0, -1),
        plan("jumps-03", "jumps", 4.0, 90.0, 24.0, 0.0, 0.0, -1),
        plan("jumps-04", "jumps", 4.5, 135.0, 30.0, 0.0, 0.0, -1),
        plan("jumps-05", "jumps", 5.0, 180.0, 12.0, 0.0, 0.0, -1),
        plan("jumps-06", "jumps", 5.5, 225.0, 18.0, 0.0, 0.0, -1),
        plan("jumps-07", "jumps", 6.0, 270.0, 24.0, 0.0, 0.0, -1),
        plan("jumps-08", "jumps", 6.5, 315.0, 30.0, 0.0, 0.0, -1),
        plan("jumps-09", "jumps", 4.2, 20.0, 15.0, 0.0, 0.0, -1),
        plan("jumps-10", "jumps", 4.8, 60.0, 21.0, 0.0, 0.0, -1),
        plan("jumps-11", "jumps", 5.4, 110.0, 27.0, 0.0, 0.0, -1),
        plan("jumps-12", "jumps", 6.0, 150.0, 33.0, 0.0, 0.0, -1),
        // pockets: straight and cut attempts, with the core's pocket index called.
        // The aims are the head-spot sight lines to pockets 0..5 and cuts off them.
        plan("pockets-01", "pockets", 2.2, 225.0, 0.0, 0.0, 0.0, 0),
        plan("pockets-02", "pockets", 2.4, 341.565, 0.0, 0.0, 0.0, 1),
        plan("pockets-03", "pockets", 2.6, 135.0, 0.0, 0.0, 0.0, 2),
        plan("pockets-04", "pockets", 2.8, 18.435, 0.0, 0.0, 0.0, 3),
        plan("pockets-05", "pockets", 3.0, 315.0, 0.0, 0.0, 0.0, 4),
        plan("pockets-06", "pockets", 3.2, 45.0, 0.0, 0.0, 0.0, 5),
        plan("pockets-07", "pockets", 2.3, 231.0, 0.0, 0.0, 0.0, 0),
        plan("pockets-08", "pockets", 2.5, 335.0, 0.0, 0.0, 0.0, 1),
        plan("pockets-09", "pockets", 2.7, 129.0, 0.0, 0.0, 0.0, 2),
        plan("pockets-10", "pockets", 2.9, 24.0, 0.0, 0.0, 0.0, 3),
        plan("pockets-11", "pockets", 3.1, 309.0, 0.0, 0.0, 0.0, 4),
        plan("pockets-12", "pockets", 3.3, 51.0, 0.0, 0.0, 0.0, 5),
        plan("pockets-13", "pockets", 2.35, 219.0, 0.0, 0.0, 0.0, 0),
        plan("pockets-14", "pockets", 2.85, 13.0, 0.0, 0.0, 0.0, 3),
        plan("pockets-15", "pockets", 3.35, 39.0, 0.0, 0.0, 0.0, 5),
        // off_table: full-power shots at the rails and corners.
        plan("off_table-01", "off_table", 8.5, 0.0, 0.0, 0.0, 0.0, -1),
        plan("off_table-02", "off_table", 8.6, 45.0, 0.0, 0.0, 0.0, -1),
        plan("off_table-03", "off_table", 8.7, 90.0, 4.0, 0.0, 0.0, -1),
        plan("off_table-04", "off_table", 8.8, 135.0, 0.0, 0.005, 0.0, -1),
        plan("off_table-05", "off_table", 8.9, 180.0, 8.0, 0.0, 0.0, -1),
        plan(
            "off_table-06",
            "off_table",
            9.0,
            225.0,
            0.0,
            -0.005,
            0.0,
            -1,
        ),
        plan(
            "off_table-07",
            "off_table",
            8.55,
            270.0,
            4.0,
            0.0,
            0.005,
            -1,
        ),
        plan("off_table-08", "off_table", 8.65, 315.0, 0.0, 0.0, 0.0, -1),
        plan("off_table-09", "off_table", 8.75, 30.0, 8.0, 0.004, 0.0, -1),
        plan(
            "off_table-10",
            "off_table",
            8.95,
            330.0,
            0.0,
            -0.004,
            0.0,
            -1,
        ),
        // carry: the last three shots of the self-play chain pin the rotation
        // hand-off (the state hash excludes rotations).
        plan("carry-01", "carry", 2.0, 180.0, 0.0, 0.006, 0.003, -1),
        plan("carry-02", "carry", 2.5, 180.0, 0.0, -0.005, 0.004, -1),
        plan("carry-03", "carry", 3.0, 90.0, 0.0, 0.0, -0.006, -1),
        // shelf: a hand-off with the ball's centre already past the mouth line
        // and another past the drop line. Neither state is reachable by playing
        // the previous shot to its end - only `shot_input` can recover it.
        special(
            "shelf-01",
            "shelf",
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            -1,
            Rack::Shelf { depth: 0.5 },
            None,
        ),
        special(
            "shelf-02",
            "shelf",
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            -1,
            Rack::Shelf { depth: 1.5 },
            None,
        ),
        // jaw: one ball rattles in off a facing, one rattles back out.
        special(
            "jaw-01",
            "jaw",
            2.5,
            225.0,
            0.0,
            0.0,
            0.0,
            0,
            Rack::Jaw {
                distance: 0.25,
                lateral: 0.5,
            },
            None,
        ),
        special(
            "jaw-02",
            "jaw",
            0.9,
            rattle_aim,
            0.0,
            0.0,
            0.0,
            0,
            Rack::Jaw {
                distance: 0.015,
                lateral: 0.045 / (corner.mouth_width / 2.0),
            },
            None,
        ),
        // hung_up: a ball handed over on the shelf with too little energy to
        // reach the drop line.
        special(
            "hung_up-01",
            "hung_up",
            slow_drop,
            225.0,
            0.0,
            0.0,
            0.0,
            -1,
            Rack::HungUp { depth: 0.1 },
            None,
        ),
        // miscue: a tip well outside the core's own miscue disc.
        special(
            "miscue-01",
            "miscue",
            2.0,
            0.0,
            0.0,
            0.02,
            0.0,
            -1,
            Rack::CueAt(head_spot(cfg)),
            None,
        ),
        // edge: a frozen contact and a contact a hair short of touching.
        special(
            "edge-01",
            "edge",
            2.0,
            0.0,
            0.0,
            0.0,
            0.0,
            -1,
            Rack::Pair {
                cue: head_spot(cfg),
                object: Vec3::new(
                    cfg.table.length / 4.0 + 2.0 * cfg.ball.radius,
                    cfg.table.width / 2.0,
                    cfg.ball.radius,
                ),
            },
            None,
        ),
        special(
            "edge-02",
            "edge",
            2.0,
            0.0,
            0.0,
            0.0,
            0.0,
            -1,
            Rack::Pair {
                cue: head_spot(cfg),
                object: Vec3::new(
                    cfg.table.length / 4.0 + 2.0 * cfg.ball.radius + tiny_gap,
                    cfg.table.width / 2.0,
                    cfg.ball.radius,
                ),
            },
            None,
        ),
        // fault: a full spread, with the event guard lowered to eight.
        special(
            "fault-01",
            "fault",
            9.0,
            0.0,
            0.0,
            0.0,
            0.0,
            -1,
            Rack::Initial,
            Some(8),
        ),
    ]
}

/// A ball as the next shot sees it: number, position and the rotation the
/// trace left it with. Nothing about the motion state crosses the boundary.
#[derive(Clone, Copy)]
struct TableBall {
    number: i32,
    position: Vec3,
    rotation: Vec3,
}

fn head_spot(cfg: &TableConfig) -> Vec3 {
    Vec3::new(
        cfg.table.length / 4.0,
        cfg.table.width / 2.0,
        cfg.ball.radius,
    )
}

/// A spread rack: fifteen object balls on a fixed 5x3 grid, with the cue ball
/// on the head spot. The spacing comes from the core's own numbers.
fn initial_rack(cfg: &TableConfig) -> Vec<TableBall> {
    let mut balls = Vec::with_capacity(16);
    balls.push(TableBall {
        number: 0,
        position: head_spot(cfg),
        rotation: Vec3::ZERO,
    });
    let x0 = cfg.table.length * 0.45;
    let step_x = cfg.ball.radius * 8.0;
    let y0 = cfg.table.width * 0.35;
    let step_y = cfg.table.width * 0.15;
    let mut number = 1;
    for row in 0..3 {
        for column in 0..5 {
            balls.push(TableBall {
                number,
                position: Vec3::new(
                    x0 + step_x * column as f64,
                    y0 + step_y * row as f64,
                    cfg.ball.radius,
                ),
                rotation: Vec3::ZERO,
            });
            number += 1;
        }
    }
    balls
}

/// The rack a shot starts from. The special cases carry no previous state:
/// they are placed from the same geometry `shot_input` uses to recover the
/// pocket state, with the cue ball on the head spot wherever it is irrelevant.
fn rack_for(cfg: &TableConfig, shot: &Plan, carried: &[TableBall]) -> Vec<TableBall> {
    let ball = |number: i32, position: Vec3| TableBall {
        number,
        position,
        rotation: Vec3::ZERO,
    };
    match shot.rack {
        Rack::Carry => carried.to_vec(),
        Rack::Initial => initial_rack(cfg),
        Rack::CueAt(position) => vec![ball(0, position)],
        Rack::Pair { cue, object } => vec![ball(0, cue), ball(1, object)],
        Rack::Shelf { depth } => {
            let geometry = TableGeometry::new(cfg);
            let pocket = &geometry.pockets[0];
            let mut shelf = shelf_point(pocket, depth * pocket.shelf_depth);
            shelf.z = cfg.ball.radius;
            vec![ball(0, head_spot(cfg)), ball(1, shelf)]
        }
        Rack::HungUp { depth } => {
            let geometry = TableGeometry::new(cfg);
            let pocket = &geometry.pockets[0];
            let mut shelf = shelf_point(pocket, depth * pocket.shelf_depth);
            shelf.z = cfg.ball.radius;
            vec![ball(0, shelf)]
        }
        Rack::Jaw { distance, lateral } => {
            let geometry = TableGeometry::new(cfg);
            let pocket = &geometry.pockets[0];
            let sideways = Vec2::new(-pocket.centreline.y, pocket.centreline.x);
            let plan = pocket.mouth_centre - pocket.centreline * distance
                + sideways * (lateral * pocket.mouth_width / 2.0);
            vec![ball(0, Vec3::new(plan.x, plan.y, cfg.ball.radius))]
        }
    }
}

fn flat_vec3(values: &[Vec3]) -> Value {
    let mut numbers = Vec::with_capacity(values.len() * 3);
    for value in values {
        numbers.push(Value::Number(value.x));
        numbers.push(Value::Number(value.y));
        numbers.push(Value::Number(value.z));
    }
    Value::Array(numbers)
}

/// The event summary: the core's event word plus the ball **numbers** it
/// touched (the rules' frame, not the rack indices).
fn event_value(event: &SimEvent, numbers: &[u8]) -> Value {
    let (ball, other) = event_numbers(event, numbers);
    Value::Object(vec![
        (
            "kind".into(),
            Value::String(pool_core::trace::event_kind_name(event.kind).into()),
        ),
        ("ball".into(), Value::Number(f64::from(ball))),
        ("other".into(), Value::Number(f64::from(other))),
    ])
}

/// The ball numbers an event names, in the rules' frame: `other` is the other
/// ball for a ball-ball contact and the rail/pocket index otherwise.
fn event_numbers(event: &SimEvent, numbers: &[u8]) -> (i32, i32) {
    let number = |index: u8| -> i32 {
        numbers
            .get(index as usize)
            .copied()
            .map(i32::from)
            .unwrap_or(-1)
    };
    let other = if event.kind == EventKind::BallBall {
        number(event.b)
    } else {
        i32::from(event.b)
    };
    (number(event.a), other)
}

// The canonical event digest lives in `pool-intent`: the same recipe is used
// by the corpus, the canary and the `COMMIT` the server sends.

/// The six wire scalars as the raw bits of their f64 representation.
fn intent_hex(intent: &Value) -> Value {
    let fields = [
        "speed",
        "level_aim_deg",
        "elevation_deg",
        "contact_x",
        "contact_y",
        "called_pocket",
    ];
    Value::Object(
        fields
            .iter()
            .map(|name| {
                let value = field(intent, name).as_f64().unwrap_or(0.0);
                (
                    (*name).into(),
                    Value::String(format!("{:016x}", value.to_bits())),
                )
            })
            .collect(),
    )
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value
        .field(key)
        .unwrap_or_else(|error| panic!("generator: {error}"))
}

/// The traced entry point turns a fault into an error and drops the state.
/// The non-traced entry point is the public contract for a faulted shot: it
/// returns the input rack with `GamePhase::Faulted`. The recorder is
/// observational, so a second run with it yields the same event list and the
/// same final rotations for the pinned record.
fn simulate_record(
    input: &ShotInput,
    cfg: &TableConfig,
    expect_fault: bool,
) -> Result<(GameState, Trace), SimError> {
    match try_simulate_shot_traced(input, cfg) {
        Ok(result) => Ok(result),
        Err(error) => {
            let state = simulate_shot(input.clone(), cfg);
            if !expect_fault || state.phase != GamePhase::Faulted {
                return Err(error);
            }
            let mut simulation = Simulation::new(input, cfg);
            simulation.record();
            let _ = simulation.run(input);
            let trace = simulation.trace().cloned().unwrap_or_default();
            Ok((state, trace))
        }
    }
}

fn fault(plan: &Plan, error: &SimError) -> String {
    format!("shot {} faulted unexpectedly: {}", plan.id, error.message)
}

/// Generates the corpus and returns its canonical JSON plus the shot count.
pub(crate) fn command_corpus() -> Result<(String, usize), String> {
    let cfg = TableConfig::default();
    let plans = plans(&cfg);
    let mut rack = initial_rack(&cfg);
    let mut shots = Vec::with_capacity(plans.len());

    for shot in &plans {
        let shot_rack = rack_for(&cfg, shot, &rack);
        let positions: Vec<Vec3> = shot_rack.iter().map(|ball| ball.position).collect();
        let numbers: Vec<i32> = shot_rack.iter().map(|ball| ball.number).collect();
        let rotations: Vec<Vec3> = shot_rack.iter().map(|ball| ball.rotation).collect();

        let mut shot_cfg = cfg.clone();
        if let Some(max_events) = shot.max_events {
            shot_cfg.simulation.max_events = max_events;
        }

        let direction = cue_direction(shot.level_aim_deg, shot.elevation_deg);
        let input = shot_input(
            &cfg,
            shot.speed,
            direction,
            (shot.contact_x, shot.contact_y),
            &positions,
            &numbers,
            &rotations,
        );
        let (state, trace) = simulate_record(&input, &shot_cfg, shot.max_events.is_some())
            .map_err(|error| fault(shot, &error))?;

        // The hand-off. The rotation comes from the trace, not the state: the
        // canonical hash excludes it, so a chain of shots is the only way to
        // pin the renderer's carry.
        let final_rotations = trace.sample_rotation(trace.duration);
        let mut next = Vec::with_capacity(shot_rack.len());
        for (index, ball) in state.balls.iter().enumerate() {
            if ball.is_pocketed || ball.is_off_table {
                continue;
            }
            let rotation = final_rotations.get(index).copied().unwrap_or(Vec3::ZERO);
            // A cue that survived keeps the spot the previous shot left it on.
            // Only a cue that was pocketed or driven off the table is put back
            // on the head spot, below.
            next.push(TableBall {
                number: i32::from(ball.number),
                position: ball.position,
                rotation,
            });
        }
        if !next.iter().any(|ball| ball.number == 0) {
            next.insert(
                0,
                TableBall {
                    number: 0,
                    position: head_spot(&cfg),
                    rotation: Vec3::ZERO,
                },
            );
        }
        rack = next;

        let events: Vec<Value> = trace
            .events
            .iter()
            .map(|event| event_value(event, &trace.numbers))
            .collect();
        let intent = Value::Object(vec![
            ("speed".into(), Value::Number(shot.speed)),
            ("level_aim_deg".into(), Value::Number(shot.level_aim_deg)),
            ("elevation_deg".into(), Value::Number(shot.elevation_deg)),
            ("contact_x".into(), Value::Number(shot.contact_x)),
            ("contact_y".into(), Value::Number(shot.contact_y)),
            (
                "called_pocket".into(),
                Value::Number(f64::from(shot.called_pocket)),
            ),
        ]);
        let mut record = vec![
            ("id".into(), Value::String(shot.id.into())),
            ("category".into(), Value::String(shot.category.into())),
            ("intent".into(), intent.clone()),
            ("positions".into(), flat_vec3(&positions)),
            (
                "numbers".into(),
                Value::Array(
                    numbers
                        .iter()
                        .map(|number| Value::Number(f64::from(*number)))
                        .collect(),
                ),
            ),
            ("rotations".into(), flat_vec3(&rotations)),
            ("events".into(), Value::Array(events)),
            (
                "final_state_hash".into(),
                Value::String(hash_state_hex(&state)),
            ),
            ("duration".into(), Value::Number(state.elapsed_time)),
        ];
        // The new pins, on every shot.
        record.insert(2, ("intent_hex".into(), intent_hex(&intent)));
        record.push((
            "events_digest".into(),
            Value::String(pool_intent::events_digest(&trace.events, &numbers)),
        ));
        record.push(("final_rotations".into(), flat_vec3(&final_rotations)));

        // Per-shot config override and the outcome the override must produce.
        if let Some(max_events) = shot.max_events {
            record.push(("max_events".into(), Value::Number(f64::from(max_events))));
        }
        if shot.category == "fault" {
            record.push(("faulted".into(), Value::Bool(true)));
        }
        if shot.category == "shelf" {
            let states: Vec<&str> = input
                .ball_states
                .iter()
                .map(|ball| match ball.pocket_state {
                    PocketState::OnTable => "on-table",
                    PocketState::Entering => "entering",
                    PocketState::Pocketed => "pocketed",
                    PocketState::HungUp => "hung-up",
                    PocketState::OffTable => "off-table",
                })
                .collect();
            record.push((
                "handoff_pocket_states".into(),
                Value::Array(
                    states
                        .into_iter()
                        .map(|name| Value::String(name.into()))
                        .collect(),
                ),
            ));
        }
        shots.push(Value::Object(record));
    }

    let document = Value::Object(vec![
        ("schema".into(), Value::String("pool-corpus/2".to_string())),
        ("shots".into(), Value::Array(shots)),
    ]);
    Ok((format!("{}\n", document.to_json()), plans.len()))
}
