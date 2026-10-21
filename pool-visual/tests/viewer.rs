//! The viewer: the page is self-contained, and the document is the core's data.
//!
//! The page is the deliverable, so the tests are about it: it must not need a
//! server, it must not reach out to the network, and the numbers in it must be
//! the numbers the core produced.

use pool_core::json;
use pool_core::{try_simulate_shot_traced, TableConfig};
use pool_visual::scenes;
use pool_visual::viewer::{self, ShotView};

/// The page of a scene, and the document it is built from.
fn page(name: &str) -> (String, json::Value) {
    let cfg = TableConfig::default();
    let scene = scenes::by_name(&cfg, name).expect("the scene exists");
    let shots: Vec<ShotView> = scene
        .shots
        .iter()
        .map(|shot| {
            let (state, trace) =
                try_simulate_shot_traced(&shot.input, &cfg).expect("the shot runs");
            ShotView {
                name: shot.name.clone(),
                note: shot.note.clone(),
                description: shot.description.clone(),
                trace,
                state,
            }
        })
        .collect();
    let html = viewer::html(&cfg, &scene.title, &scene.subtitle, &shots);
    let document = viewer::document(&cfg, &scene.title, &scene.subtitle, &shots);
    (html, document)
}

/// The document embedded in the page.
fn embedded(html: &str) -> json::Value {
    let start = html
        .find("<script id=\"pool-document\" type=\"application/json\">")
        .expect("the page has the document");
    let start = start + "<script id=\"pool-document\" type=\"application/json\">".len();
    let end = html[start..]
        .find("</script>")
        .expect("the document is closed")
        + start;
    let text = &html[start..end];
    json::parse(text).expect("the document is valid JSON")
}

#[test]
fn every_scene_produces_a_page() {
    for name in scenes::NAMES {
        let (html, document) = page(name);
        assert!(
            html.len() > 10_000,
            "{name}: the page is suspiciously small"
        );
        assert!(
            !html.contains("__POOL_DOCUMENT__"),
            "{name}: the document was not written into the page"
        );
        assert_eq!(
            document.get("shot_count").and_then(|v| v.as_f64()),
            Some(
                scenes::by_name(&TableConfig::default(), name)
                    .expect("the scene exists")
                    .shots
                    .len() as f64
            ),
            "{name}: the shot count"
        );
    }
}

#[test]
fn the_page_needs_nothing_outside_itself() {
    for name in scenes::NAMES {
        let (html, _) = page(name);
        assert!(
            !html.contains("http://") && !html.contains("https://"),
            "{name}: the page reaches out to the network"
        );
        assert!(
            !html.contains("<link") && !html.contains("src="),
            "{name}: the page loads something from outside"
        );
    }
}

#[test]
fn the_document_is_the_core_s_own_data() {
    let cfg = TableConfig::default();
    let (html, _) = page("trio");
    let document = embedded(&html);
    let shots = document
        .get("shots")
        .and_then(|v| v.as_array())
        .expect("the document has the shots");
    assert_eq!(shots.len(), 3);

    // The geometry in the document is the geometry of the table.
    let geometry = document
        .get("geometry")
        .expect("the document has the geometry");
    let length = geometry
        .get("length")
        .and_then(|v| v.as_f64())
        .expect("the table has a length");
    assert_eq!(length, cfg.table.length);
    let pockets = geometry
        .get("pockets")
        .and_then(|v| v.as_array())
        .expect("the table has pockets");
    assert_eq!(pockets.len(), 6);

    // The last state of a shot is the state the core ended in.
    for shot in shots {
        let name = shot.get("name").and_then(|v| v.as_str()).expect("named");
        let final_positions = shot
            .get("final")
            .and_then(|v| v.as_array())
            .expect("the shot has a final state");
        for ball in final_positions {
            let number = ball
                .get("number")
                .and_then(|v| v.as_f64())
                .expect("numbered");
            let x = ball.get("x").and_then(|v| v.as_f64()).expect("placed");
            assert!(
                x >= -0.2 && x <= cfg.table.length + 0.2,
                "{name}: ball {number} is at {x}, which is not on the table"
            );
        }
    }
}

#[test]
fn the_index_links_to_every_scene() {
    let cfg = TableConfig::default();
    let mut entries = Vec::new();
    for name in scenes::NAMES {
        let scene = scenes::by_name(&cfg, name).expect("the scene exists");
        entries.push(pool_visual::viewer::IndexEntry {
            file: format!("{name}.html"),
            title: scene.title.clone(),
            subtitle: scene.subtitle.clone(),
            shots: scene
                .shots
                .iter()
                .map(|shot| pool_visual::viewer::IndexShot {
                    name: shot.name.clone(),
                    note: shot.note.clone(),
                })
                .collect(),
        });
    }
    let page = pool_visual::viewer::index(&entries);
    for name in scenes::NAMES {
        assert!(
            page.contains(&format!("href=\"{name}.html\"")),
            "{name}: the index does not link to the scene"
        );
    }
    assert!(!page.contains("http://") && !page.contains("https://"));
}

#[test]
fn the_draw_shot_draws_back_and_the_follow_shot_follows() {
    let cfg = TableConfig::default();
    let scene = scenes::by_name(&cfg, "trio").expect("the scene exists");
    let contact = 1.5 - 2.0 * cfg.ball.radius;
    let mut draw = None;
    let mut follow = None;
    for shot in &scene.shots {
        let (_, trace) = try_simulate_shot_traced(&shot.input, &cfg).expect("the shot runs");
        // A second after the contact the cue ball has gone its own way.
        let state = trace.sample(1.0, &cfg);
        let x = state[0].position.x;
        match shot.name.as_str() {
            "draw" => draw = Some(x),
            "follow" => follow = Some(x),
            _ => {}
        }
    }
    let draw = draw.expect("the draw shot");
    let follow = follow.expect("the follow shot");
    assert!(
        draw < contact - 0.05,
        "the draw shot ended at {draw}, which is not behind the contact"
    );
    assert!(
        follow > contact + 0.05,
        "the follow shot ended at {follow}, which is not past the contact"
    );
}

#[test]
fn the_squirt_points_away_from_the_side_that_was_struck() {
    let cfg = TableConfig::default();
    let scene = scenes::by_name(&cfg, "english").expect("the scene exists");
    let mut left = None;
    let mut right = None;
    for shot in &scene.shots {
        let (_, trace) = try_simulate_shot_traced(&shot.input, &cfg).expect("the shot runs");
        let state = trace.sample(0.15, &cfg);
        let cue = state[0];
        match shot.name.as_str() {
            "left english" => left = Some(cue.velocity.y),
            "right english" => right = Some(cue.velocity.y),
            _ => {}
        }
    }
    // The left side was struck, so the ball leaves to the right, and the other
    // way round: the ball squirts away from the side the tip hit (§6.1).
    assert!(left.expect("the left english shot") > 0.0);
    assert!(right.expect("the right english shot") < 0.0);
}

#[test]
fn the_cushion_scene_hits_the_rail_at_the_angles_it_asks_for() {
    let cfg = TableConfig::default();
    let scene = scenes::by_name(&cfg, "cushion").expect("the scene exists");
    for shot in &scene.shots {
        let (_, trace) = try_simulate_shot_traced(&shot.input, &cfg).expect("the shot runs");
        let contact = trace
            .events
            .iter()
            .find(|event| event.kind == pool_core::EventKind::Cushion)
            .unwrap_or_else(|| panic!("{}: the shot never reaches a rail", shot.name));
        let before = trace.sample(contact.time - 1e-7, &cfg)[0].velocity;
        // The bottom rail's normal is -y, so the angle from the normal is the
        // angle the ball makes with the y axis.
        let incidence = before.x.atan2(before.y.abs()).to_degrees().abs();
        let asked: f64 = shot
            .name
            .split_whitespace()
            .next()
            .map(|v| v.trim_end_matches('\u{b0}'))
            .and_then(|v| v.parse::<f64>().ok())
            .expect("the name carries the angle");
        assert!(
            (incidence - asked).abs() < 0.5,
            "{}: the ball met the rail at {incidence} degrees, not {asked}",
            shot.name
        );
    }
}

#[test]
fn the_jump_shot_leaves_the_cloth() {
    let cfg = TableConfig::default();
    let scene = scenes::by_name(&cfg, "jump").expect("the scene exists");
    let shot = &scene.shots[0];
    let (_, trace) = try_simulate_shot_traced(&shot.input, &cfg).expect("the shot runs");
    let mut highest = 0.0;
    let mut airborne = false;
    for step in 0..2000 {
        let t = trace.duration * step as f64 / 2000.0;
        let state = trace.sample(t, &cfg);
        let height = state[0].position.z - cfg.ball.radius;
        if height > highest {
            highest = height;
        }
        if state[0].motion_state == pool_core::MotionState::Airborne {
            airborne = true;
        }
    }
    assert!(airborne, "the ball never left the cloth");
    assert!(
        highest > 0.005,
        "the hop is {highest} m high, which is not a hop"
    );
}
