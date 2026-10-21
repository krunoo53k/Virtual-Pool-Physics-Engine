//! The viewer: the trace as JSON, and the trace as a page.
//!
//! The document the viewer reads holds the table geometry and the path of every
//! shot. The page evaluates the same closed-form segments the core does, so what
//! is on the screen is the model's trajectory and not a resampling of it.

use pool_core::geometry::TableGeometry;
use pool_core::json::Value;
use pool_core::{GameState, TableConfig, Trace};

/// The template of the page, with the document placeholder in it.
const TEMPLATE: &str = include_str!("../assets/viewer.html");

/// The placeholder the document is written into.
const PLACEHOLDER: &str = "__POOL_DOCUMENT__";

/// One shot, ready to be shown.
pub struct ShotView {
    /// The name of the shot.
    pub name: String,
    /// A one line note.
    pub note: String,
    /// What the shot shows.
    pub description: String,
    /// The path of the shot.
    pub trace: Trace,
    /// The state the shot ended in.
    pub state: GameState,
}

impl ShotView {
    /// The shot as a JSON value.
    fn to_json_value(&self) -> Value {
        let value = self.trace.to_json_value();
        let entries = match value {
            Value::Object(entries) => entries,
            _ => Vec::new(),
        };
        let mut entries = entries;
        entries.insert(0, ("name".into(), Value::String(self.name.clone())));
        entries.insert(1, ("note".into(), Value::String(self.note.clone())));
        entries.insert(
            2,
            (
                "description".into(),
                Value::String(self.description.clone()),
            ),
        );
        entries.push((
            "final".into(),
            Value::Array(
                self.state
                    .balls
                    .iter()
                    .map(|ball| {
                        Value::Object(vec![
                            ("number".into(), Value::Number(ball.number as f64)),
                            ("x".into(), Value::Number(ball.position.x)),
                            ("y".into(), Value::Number(ball.position.y)),
                            ("pocketed".into(), Value::Bool(ball.is_pocketed)),
                        ])
                    })
                    .collect(),
            ),
        ));
        Value::Object(entries)
    }
}

/// The whole document: the table and the shots.
pub fn document(cfg: &TableConfig, title: &str, subtitle: &str, shots: &[ShotView]) -> Value {
    let geometry = TableGeometry::new(cfg);
    Value::Object(vec![
        ("title".into(), Value::String(title.to_string())),
        ("subtitle".into(), Value::String(subtitle.to_string())),
        (
            "model_version".into(),
            Value::String(pool_core::MODEL_VERSION.to_string()),
        ),
        ("config".into(), cfg.to_json_value()),
        ("geometry".into(), geometry.to_json_value(cfg)),
        ("shot_count".into(), Value::Number(shots.len() as f64)),
        (
            "shots".into(),
            Value::Array(shots.iter().map(ShotView::to_json_value).collect()),
        ),
    ])
}

/// The page: the document, embedded in the template.
///
/// The result is a single file with no external reference at all: it opens from
/// the disk, with no server and no network.
pub fn html(cfg: &TableConfig, title: &str, subtitle: &str, shots: &[ShotView]) -> String {
    let document = document(cfg, title, subtitle, shots).to_json();
    TEMPLATE.replace(PLACEHOLDER, &document)
}

/// The index of a gallery: one page that links to the shots.
///
/// The pages of a gallery are independent files, so the index is the thing that
/// ties them together.
pub fn index(entries: &[IndexEntry]) -> String {
    let mut body = String::new();
    for entry in entries {
        body.push_str(&format!(
            "    <a class=\"card\" href=\"{file}\">\n      <h2>{title}</h2>\n      \
             <p class=\"sub\">{subtitle}</p>\n      <ul>\n{shots}      </ul>\n    </a>\n",
            file = entry.file,
            title = entry.title,
            subtitle = entry.subtitle,
            shots = entry
                .shots
                .iter()
                .map(|shot| format!(
                    "        <li><b>{}</b> &mdash; {}</li>\n",
                    shot.name, shot.note
                ))
                .collect::<String>(),
        ));
    }
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>Virtual Pool &mdash; the demos</title>\n<style>\n\
         :root {{ --bg:#0f1319; --panel:#181e26; --line:#2a3340; --text:#e7edf4; --dim:#8a97a5; \
         --accent:#4ea1ff; }}\n\
         * {{ box-sizing:border-box; }}\n\
         body {{ margin:0; background:var(--bg); color:var(--text); padding:28px;\n\
         font:13px/1.5 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace; }}\n\
         h1 {{ font-size:16px; margin:0 0 4px; }}\n\
         p.lead {{ color:var(--dim); margin:0 0 22px; }}\n\
         .grid {{ display:grid; grid-template-columns:repeat(auto-fill,minmax(300px,1fr)); gap:14px; }}\n\
         .card {{ display:block; text-decoration:none; color:inherit; background:var(--panel);\n\
         border:1px solid var(--line); border-radius:8px; padding:14px; }}\n\
         .card:hover {{ border-color:var(--accent); }}\n\
         .card h2 {{ font-size:13px; margin:0 0 4px; color:var(--accent); }}\n\
         .card .sub {{ color:var(--dim); font-size:11px; margin:0 0 10px; }}\n\
         .card ul {{ margin:0; padding-left:16px; color:var(--dim); font-size:11px; }}\n\
         .card li b {{ color:var(--text); font-weight:600; }}\n\
         </style>\n</head>\n<body>\n  <h1>Virtual Pool &mdash; the demos</h1>\n\
         <p class=\"lead\">Every page is one shot of the physics core, replayed from \
         the path it recorded.</p>\n  <div class=\"grid\">\n{body}  </div>\n</body>\n</html>\n",
        body = body
    )
}

/// One scene of the index.
pub struct IndexEntry {
    /// The file of the scene's page.
    pub file: String,
    /// The title of the scene.
    pub title: String,
    /// The subtitle of the scene.
    pub subtitle: String,
    /// The shots of the scene.
    pub shots: Vec<IndexShot>,
}

/// One shot of an index entry.
pub struct IndexShot {
    /// The name of the shot.
    pub name: String,
    /// The note of the shot.
    pub note: String,
}
