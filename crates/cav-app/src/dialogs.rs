//! The editor's windows (`docs/slices/S18.md` §1, "Start, save and open" and "See what
//! happened"): the edit history, the new-scene wizard, Open and Save. Each is a window of the
//! keyboard model (a list of sections; Esc closes it and gives the focus back) and does what it
//! does through console lines and commands: the wizard is one `Command::Load`, so one undo takes
//! the new scene away; Save is `save <file>`; Open is `load <name or file>`.

use cav_engine::Command;
use cav_sim::SceneSpec;
use egui::RichText;

use crate::keyboard::Kind;
use crate::shell::Shell;
use crate::theme;

/// The wizard's choices.
#[derive(Clone, Debug, PartialEq)]
pub struct Wizard {
    /// The array: 0 the default five towers (AION-10), 1 one tower, 2 none, 3 five on AICE.
    pub array: usize,
    /// What surrounds it: 0 nothing, 1 a floor to the horizon, 2 a room.
    pub surroundings: usize,
    /// The first body: a starter's name, or none.
    pub body: Option<String>,
    /// How long the scene runs, seconds.
    pub duration: f64,
}

impl Default for Wizard {
    fn default() -> Self {
        Self { array: 0, surroundings: 2, body: Some("point-mass".into()), duration: 12.0 }
    }
}

/// The wizard's array choices, in its order.
pub const ARRAYS: [&str; 4] =
    ["five towers, AION-10 (the default layout)", "one tower", "no towers", "five towers, AICE"];
/// The wizard's surroundings, in its order.
pub const SURROUNDINGS: [&str; 3] = ["nothing (the void)", "a floor to the horizon", "a room"];

impl Wizard {
    /// The scene the choices make, checked.
    ///
    /// # Errors
    /// When the scene would not be valid, with the spec's reason.
    pub fn scene(&self) -> Result<SceneSpec, String> {
        let mut spec = SceneSpec::room();
        spec.name = "new scene".into();
        spec.description = "Made with File > New scene.".into();
        spec.duration = self.duration;
        match self.array {
            1 => spec.array.towers.truncate(1),
            2 => spec.array.towers.clear(),
            3 => spec.array.instrument = cav_sim::presets::load("aice").map_err(|e| e.0)?,
            _ => {}
        }
        match self.surroundings {
            0 => spec.room = None,
            1 => {
                spec.room = None;
                spec.floor = Some(cav_sim::FloorSpec {
                    height: 0.0,
                    kind: cav_sim::FloorKind::HalfSpace,
                    density: 2400.0,
                });
            }
            _ => {}
        }
        if let Some(name) = &self.body {
            let lift = cav_sim::starters::find(name).and_then(|s| s.body.as_ref()).map_or(0.0, |b| {
                if b.shape.is_none() && b.radius == 0.0 { 1.0 } else { cav_sim::starters::lift(b) }
            });
            let (list, value) = cav_sim::starters::make(&spec, name, [2.0, 2.0, lift])?;
            spec = spec.inserted(list, None, &value)?.0;
        }
        spec.validate().map_err(|e| e.0)?;
        Ok(spec)
    }
}

/// Draws the editor's windows that are open.
pub fn draw(shell: &mut Shell, ctx: &egui::Context) {
    history(shell, ctx);
    wizard(shell, ctx);
    open(shell, ctx);
    save(shell, ctx);
    import(shell, ctx);
}

/// What the Import mesh window will import (S19): each field as typed.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportForm {
    /// The file: a name dropped on the window, or a path (natively).
    pub file: String,
    /// How long its widest side is made, metres; empty for the file's own units as metres.
    pub fit: String,
    /// The material's label.
    pub material: String,
    /// The voxel pitch, metres; empty for the widest side over 32.
    pub pitch: String,
    /// The file the box below was read from, and its box's sides in the file's units (or why
    /// it could not be read): shown in the window, and what sets the body's lift off the ground.
    pub measured: Option<(String, Result<[f64; 3], String>)>,
}

impl ImportForm {
    /// A form for `file`: at the file's own size (its units read as metres: S19's review,
    /// minor 9, where it was fitted to 1 m whatever it was), of the default material, the pitch
    /// worked out.
    #[must_use]
    pub fn new(file: &str) -> Self {
        Self {
            file: file.to_owned(),
            fit: String::new(),
            material: cav_sim::import::DEFAULT_MATERIAL.to_owned(),
            pitch: String::new(),
            measured: None,
        }
    }

    /// The sides of the mesh's box as it will be imported, metres: the file's box, scaled to
    /// the typed widest side (or 1:1). None when the file has not been read.
    #[must_use]
    pub fn imported_size(&self) -> Option<[f64; 3]> {
        let size = self.measured.as_ref()?.1.as_ref().ok()?;
        let widest = size.iter().copied().fold(0.0, f64::max);
        let scale = match self.fit.trim().parse::<f64>() {
            Ok(fit) if fit.is_finite() && fit > 0.0 && widest > 0.0 => fit / widest,
            _ => 1.0,
        };
        Some(size.map(|s| s * scale))
    }

    /// The console line it runs, the body placed at `at` (metres): `import <file> fit … material
    /// … [pitch …] at x y z`.
    #[must_use]
    pub fn line(&self, at: [f64; 3]) -> String {
        let quote = |s: &str| if s.contains(char::is_whitespace) { format!("\"{s}\"") } else { s.to_owned() };
        let mut line = format!("import {}", quote(self.file.trim()));
        if !self.fit.trim().is_empty() {
            line.push_str(&format!(" fit {}", self.fit.trim()));
        }
        line.push_str(&format!(" material {}", quote(&self.material)));
        if !self.pitch.trim().is_empty() {
            line.push_str(&format!(" pitch {}", self.pitch.trim()));
        }
        line.push_str(&format!(" at {} {} {}", at[0], at[1], at[2]));
        line
    }
}

/// The sides of the box of the mesh in `file` (a dropped file's name, or natively a path), in the
/// file's units; or why it cannot be read.
fn mesh_box(shell: &Shell, file: &str) -> Result<[f64; 3], String> {
    if file.is_empty() {
        return Err(String::new());
    }
    let bytes = match shell.engine.dropped(file) {
        Some(bytes) => bytes.to_vec(),
        None => cav_engine::files::read(std::path::Path::new(file), cav_core::voxel::mesh::MAX_MESH_BYTES)
            .map_err(|e| format!("{file}: {e}"))?,
    };
    let mesh = cav_core::voxel::mesh::Mesh::parse(file, &bytes, 1.0).map_err(|e| e.0)?;
    Ok(cav_core::voxel::Shape::bounds(&mesh).size().to_array())
}

/// The Import mesh window (File › Import mesh…, or a file dropped on the window): the file, its
/// size, its material, its pitch; Import adds the body where the camera looks, one undo.
fn import(shell: &mut Shell, ctx: &egui::Context) {
    let Some(mut form) = shell.import_window.clone() else { return };
    shell.building.window("import", "Import mesh");
    let mut open = true;
    let mut go = false;
    let window = egui::Window::new("Import a mesh")
        .open(&mut open)
        .collapsible(false)
        .default_width(380.0)
        .show(ctx, |ui| {
            ui.label(
                RichText::new("an OBJ or STL file: drop one on the window, or (natively) type its path")
                    .size(11.0)
                    .color(theme::WEAK),
            );
            let text_row = |ui: &mut egui::Ui,
                            shell: &mut Shell,
                            id: &str,
                            name: &str,
                            value: &mut String,
                            hint: &str| {
                ui.horizontal(|ui| {
                    ui.add_sized([96.0, 18.0], egui::Label::new(RichText::new(name).color(theme::WEAK)));
                    let field =
                        ui.add(egui::TextEdit::singleline(value).hint_text(hint).desired_width(220.0));
                    shell.register("import", id, name, Kind::Text, field.rect);
                    if shell.take_type(id) {
                        field.request_focus();
                    }
                    field
                })
                .inner
            };
            text_row(ui, shell, "import/file", "File", &mut form.file, "bracket.stl");
            // The file's box, read once a file (S19's review, minor 9): so that its size is seen
            // before it is chosen.
            if form.measured.as_ref().is_none_or(|(of, _)| *of != form.file) {
                form.measured = Some((form.file.clone(), mesh_box(shell, form.file.trim())));
            }
            let size = match &form.measured {
                Some((_, Ok([x, y, z]))) => format!(
                    "its box: {} × {} × {} in the file's units",
                    crate::editors::shown(*x),
                    crate::editors::shown(*y),
                    crate::editors::shown(*z)
                ),
                Some((_, Err(why))) => why.clone(),
                None => String::new(),
            };
            ui.label(RichText::new(size).size(11.0).color(theme::WEAK));
            text_row(
                ui,
                shell,
                "import/fit",
                "Widest side, m",
                &mut form.fit,
                "empty: its own size, its units as metres",
            );
            ui.horizontal(|ui| {
                ui.add_sized([96.0, 18.0], egui::Label::new(RichText::new("Material").color(theme::WEAK)));
                let all: Vec<cav_sim::MaterialSpec> =
                    cav_sim::materials::every(&shell.engine.spec().materials).into_iter().cloned().collect();
                let combo = egui::ComboBox::from_id_salt("import material")
                    .width(132.0)
                    .selected_text(form.material.clone())
                    .show_ui(ui, |ui| {
                        for m in &all {
                            if ui.selectable_label(m.label == form.material, &m.label).clicked() {
                                form.material.clone_from(&m.label);
                            }
                        }
                    });
                let current = all.iter().position(|m| m.label == form.material).unwrap_or(0);
                shell.register(
                    "import",
                    "import/material",
                    "Material",
                    Kind::Choice { count: all.len(), current },
                    combo.response.rect,
                );
                if let Some(index) = shell.take_pick("import/material")
                    && let Some(m) = all.get(index)
                {
                    form.material.clone_from(&m.label);
                }
            });
            text_row(ui, shell, "import/pitch", "Pitch, m", &mut form.pitch, "empty: the widest side / 32");
            let button = ui.button("Import");
            shell.register("import", "import/go", "Import", Kind::Button, button.rect);
            shell.layout.buttons.push(("import go".to_owned(), button.rect));
            go = button.clicked() || shell.take_click("import/go");
        });
    if let Some(window) = window {
        shell.building.place_window("import", window.response.rect, window.response.layer_id);
    }
    shell.import_window = Some(form.clone());
    if go && !form.file.trim().is_empty() {
        // Where the camera looks, as the Add menu puts a starter, standing on the ground: lifted
        // by half its own height (S19's review, minor 8: the cube's lift sank a taller mesh).
        let mut at = shell.place_for("cube");
        if let Some([_, _, height]) = form.imported_size() {
            at[2] = shell.ground() + 0.5 * height;
        }
        if shell.run(&form.line(at)).is_ok() {
            let n = shell.engine.spec().bodies.len();
            let _ = shell.run(&format!("select body {n}"));
            open = false;
        }
    }
    if !open {
        shell.import_window = None;
        if shell.keyboard.focus.first().map(String::as_str) == Some("import") {
            shell.keyboard.close_window();
        }
    }
}

/// A step of the history in a person's words (S18's review, U6: the list showed the console's
/// derived lines, `add bodies 1 {"label":…`): "Added sphere", "Moved sphere to (2, 0, 1) m",
/// "Set duration to 20 s", "Removed tower 3", "Opened “town”". Names are those of `spec`, the
/// scene just after the step ([`history_words`] gives each step its own, so a removal later in
/// the list renames nothing: S18's second review, minor 1); the line that does the step is
/// [`cav_engine::history::Edit::line`], for a tooltip. Only the field touched is read, never a
/// whole inserted value.
#[must_use]
pub fn step_words(step: &cav_engine::history::Step, spec: &SceneSpec) -> String {
    use cav_engine::history::Edit;
    match (&step.undo, &step.redo) {
        (_, Edit::Replace(scene)) => format!("Opened “{}”", scene.name),
        (_, Edit::View { .. }) => "Reset the view (Home)".to_owned(),
        (_, Edit::Insert { list, value, .. }) => {
            match (list.as_str(), value.get("label").and_then(|l| l.as_str())) {
                ("bodies", Some(label)) => format!("Added {label}"),
                ("array.towers", Some(label)) => format!("Added the tower {label}"),
                _ => format!("Added an item to {}", place(list, spec)),
            }
        }
        (Edit::Insert { value, .. }, Edit::Remove { path }) => {
            match value.get("label").and_then(|l| l.as_str()) {
                Some(label) => format!("Removed {label}"),
                None => format!(
                    "Took an item out of {}",
                    place(path.rsplit_once('[').map_or(path.as_str(), |(list, _)| list), spec)
                ),
            }
        }
        (_, Edit::Remove { path }) => format!("Removed {}", place(path, spec)),
        (_, Edit::Put { path, value }) => put_words(path, value, spec),
        (_, Edit::Folders { line, .. } | Edit::Batch { line, .. }) => folder_words(line),
    }
}

/// A folder edit in a person's words, from the console line that made it (`folder move body 1
/// rig`): "Made the folder rig", "Moved 2 items into rig", "Removed the folder rig".
fn folder_words(line: &str) -> String {
    let words = cav_engine::console::words(line).unwrap_or_default();
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["folder", "new", name] => format!("Made the folder {name}"),
        ["folder", "new", name, "in", within] => format!("Made the folder {name} in {within}"),
        ["folder", "move", items @ .., to] => {
            // `body N` and `tower N` are two words an item; a folder's name is one.
            let count = items.iter().filter(|w| !matches!(**w, "body" | "tower")).count().max(1);
            let what = if count == 1 { "1 item".to_owned() } else { format!("{count} items") };
            if to.eq_ignore_ascii_case("none") {
                format!("Moved {what} out of its folder")
            } else {
                format!("Moved {what} into {to}")
            }
        }
        ["folder", "rename", from, to] => format!("Renamed the folder {from} to {to}"),
        ["folder", "remove", name] => format!("Removed the folder {name} (its contents kept)"),
        ["folder", "duplicate", name] => format!("Duplicated the folder {name}"),
        _ => line.to_owned(),
    }
}

/// What owns the field at `path`, as a person names it: a body or a tower by its label, "the
/// instrument", "the cadence", "the room"; `None` for the scene's own fields (`duration`).
fn owner(path: &str, spec: &SceneSpec) -> Option<String> {
    let index = |rest: &str| {
        rest.strip_prefix('[').and_then(|r| r.split(']').next()).and_then(|n| n.parse::<usize>().ok())
    };
    if let Some(n) = path.strip_prefix("bodies").and_then(index) {
        return Some(spec.bodies.get(n).map_or_else(|| format!("body {}", n + 1), |b| b.label.clone()));
    }
    if let Some(n) = path.strip_prefix("array.towers").and_then(index) {
        return Some(
            spec.array.towers.get(n).map_or_else(|| format!("tower {}", n + 1), |t| t.label.clone()),
        );
    }
    let parts = [
        ("array.instrument", "the instrument"),
        ("array.schedule", "the cadence"),
        ("room", "the room"),
        ("floor", "the floor"),
        ("world", "the world"),
    ];
    parts.iter().find(|(prefix, _)| path.starts_with(prefix)).map(|(_, name)| (*name).to_owned())
}

/// A path as a place: its owner and its field ("sphere's motion", "the room", "duration").
fn place(path: &str, spec: &SceneSpec) -> String {
    // The field's name from the schema alone, so that a step of a body since removed is still
    // named (its title, else its name in words).
    let last = path.rsplit('.').next().unwrap_or(path);
    let field = cav_sim::paths::schema_at(path)
        .ok()
        .and_then(|schema| schema.get("title").and_then(serde_json::Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| cav_sim::paths::words(last));
    let field = lower_first(&field);
    match owner(path, spec) {
        // The owner itself (`bodies[2]`, `room`): just its name.
        Some(owner) if path.ends_with(']') || !path.contains('.') => owner,
        Some(owner) => format!("{owner}'s {field}"),
        None => field,
    }
}

fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    // An initialism or a symbol keeps its case ("AION-10", "Δr").
    match (chars.next(), chars.next()) {
        (Some(first), Some(second)) if first.is_uppercase() && !second.is_uppercase() => {
            first.to_lowercase().chain(text.chars().skip(1)).collect()
        }
        _ => text.to_owned(),
    }
}

/// A value as a person reads it, with the field's unit from the schema.
fn value_words(value: &serde_json::Value, unit: Option<&str>) -> String {
    use serde_json::Value;
    let unit = unit.filter(|u| *u != "1").map_or_else(String::new, |u| format!(" {u}"));
    match value {
        Value::Number(n) => format!("{}{unit}", crate::editors::shown(n.as_f64().unwrap_or_default())),
        Value::Array(items) if items.iter().all(Value::is_number) => {
            let numbers: Vec<String> =
                items.iter().map(|v| crate::editors::shown(v.as_f64().unwrap_or_default())).collect();
            format!("({}){unit}", numbers.join(", "))
        }
        Value::String(text) => format!("“{text}”"),
        Value::Bool(on) => (if *on { "on" } else { "off" }).to_owned(),
        Value::Null => "none".to_owned(),
        // A variant with its data: its name, in words.
        Value::Object(map) if map.len() == 1 => cav_sim::paths::words(map.keys().next().expect("one key")),
        _ => "a new value".to_owned(),
    }
}

/// A `set` in words.
fn put_words(path: &str, value: &serde_json::Value, spec: &SceneSpec) -> String {
    let schema = cav_sim::paths::schema_at(path).ok();
    let unit = schema.as_ref().and_then(|s| s.get("x-unit")).and_then(serde_json::Value::as_str);
    let last = path.rsplit('.').next().unwrap_or(path);
    let who = owner(path, spec);
    // Moving a body or a tower: its anchor or its position.
    let moved = (path.starts_with("bodies[")
        && path.contains(".motion.")
        && matches!(last, "position" | "start" | "centre" | "pivot"))
        || (path.starts_with("array.towers[") && last == "position");
    if let (true, Some(who)) = (moved, &who) {
        return format!("Moved {who} to {}", value_words(value, unit));
    }
    if last == "label"
        && let Some(text) = value.as_str()
    {
        return format!("Renamed {} “{text}”", if path.starts_with("bodies") { "a body" } else { "a tower" });
    }
    if value.is_null() {
        return format!("Took away {}", place(path, spec));
    }
    // A field of a body or a tower: "Set mass to 5 kg on “ball”" (S18's second review, UX 4: it
    // read "Set ball's mass to 5 kg", and "Set mass's mass" for a body called mass).
    if let Some(who) =
        owner(path, spec).filter(|_| path.starts_with("bodies[") || path.starts_with("array.towers["))
    {
        let field = lower_first(&field_name(path));
        return format!("Set {field} to {} on “{who}”", value_words(value, unit));
    }
    format!("Set {} to {}", place(path, spec), value_words(value, unit))
}

/// A field's name from the schema alone (its title, else its name in words).
fn field_name(path: &str) -> String {
    let last = path.rsplit('.').next().unwrap_or(path);
    cav_sim::paths::schema_at(path)
        .ok()
        .and_then(|schema| schema.get("title").and_then(serde_json::Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| cav_sim::paths::words(last))
}

/// Every step of the engine's history in words, each named from the scene as it was just after
/// that step (walked back from the scene now by the steps' own undo edits, and forward by the
/// redo edits): the done ones oldest first, then the undone ones, the next to redo first.
#[must_use]
pub fn history_words(engine: &cav_engine::Engine) -> (Vec<String>, Vec<String>) {
    let (done, undone) = engine.history_steps();
    let now = engine.spec().clone();
    let mut after = now.clone();
    let mut done_words = Vec::with_capacity(done.len());
    for step in done.iter().rev() {
        done_words.push(step_words(step, &after));
        // A step the walk cannot take back (never, for the history's own steps) names from now on.
        after = step.undo.apply(&after).unwrap_or_else(|_| now.clone());
    }
    done_words.reverse();
    let mut after = now.clone();
    let mut undone_words = Vec::with_capacity(undone.len());
    for step in &undone {
        after = step.redo.apply(&after).unwrap_or_else(|_| now.clone());
        undone_words.push(step_words(step, &after));
    }
    (done_words, undone_words)
}

/// What the history list's first row says: what undoing every step goes back to.
fn start_words(engine: &cav_engine::Engine) -> String {
    use cav_engine::history::Edit;
    match engine.history_steps().0.first().map(|s| &s.undo) {
        Some(Edit::Replace(scene)) => format!("Before opening: “{}”", scene.name),
        Some(_) => "Before the first edit".to_owned(),
        None => "Nothing to undo yet".to_owned(),
    }
}

fn history(shell: &mut Shell, ctx: &egui::Context) {
    if !shell.history_open {
        return;
    }
    shell.building.window("history", "History");
    // The words of each step, made once a change of the engine (the console line only when a
    // row is hovered).
    let (done, undone) = shell.history_words();
    let start_label = start_words(&shell.engine);
    let mut open = true;
    let mut go: Option<(usize, &str)> = None;
    let mut hovered: Option<(bool, usize)> = None;
    // Inside the outliner, over neither the figure nor the tool palette (S18's reviews, U6 and UX 4).
    let outliner = shell.layout.panel("outliner");
    let left = outliner.map_or(ctx.content_rect().left() + 8.0, |r| r.left() + 4.0);
    let top = outliner.map_or(60.0, |r| r.top() + 30.0);
    let width = outliner.map_or(260.0, |r| r.width() - 22.0);
    let window = egui::Window::new("Edit history")
        .open(&mut open)
        .collapsible(false)
        .default_pos(egui::pos2(left, top))
        .min_width(width)
        .max_width(width)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(
                    "every edit, oldest first; click one to go back or forward to it (Ctrl+Z, Ctrl+Y step)",
                )
                .size(11.0)
                .color(theme::WEAK),
            );
            egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                let start = ui.selectable_label(done.is_empty(), RichText::new(&start_label).italics());
                // Named as it is drawn, so a screen reader says what the eye reads (S18's third
                // review, minor 6).
                shell.register("history", "history/start", &start_label, Kind::Button, start.rect);
                shell.layout.buttons.push(("history 0".to_owned(), start.rect));
                if (start.clicked() || shell.take_click("history/start")) && !done.is_empty() {
                    go = Some((done.len(), "undo"));
                }
                for (n, said) in done.iter().enumerate() {
                    let now = n + 1 == done.len();
                    let text = RichText::new(format!("{}{said}", if now { "› " } else { "" })).size(12.0);
                    let row = ui.selectable_label(now, text);
                    if row.hovered() {
                        hovered = Some((true, n));
                    }
                    let id = format!("history/done/{n}");
                    shell.register("history", &id, said, Kind::Button, row.rect);
                    shell.layout.buttons.push((format!("history {}", n + 1), row.rect));
                    if (row.clicked() || shell.take_click(&id)) && !now {
                        go = Some((done.len() - 1 - n, "undo"));
                    }
                }
                for (n, said) in undone.iter().enumerate() {
                    let row = ui.selectable_label(false, RichText::new(said).size(12.0).color(theme::WEAK));
                    if row.hovered() {
                        hovered = Some((false, n));
                    }
                    let id = format!("history/undone/{n}");
                    shell.register("history", &id, said, Kind::Button, row.rect);
                    shell.layout.buttons.push((format!("history {}", done.len() + n + 1), row.rect));
                    if row.clicked() || shell.take_click(&id) {
                        go = Some((n + 1, "redo"));
                    }
                }
            });
            // The console line of the row under the pointer: what an agent or a script would type.
            let (done_steps, undone_steps) = shell.engine.history_steps();
            let line = match hovered {
                Some((true, n)) => done_steps.get(n).map(|s| s.redo.line()),
                Some((false, n)) => undone_steps.get(n).map(|s| s.redo.line()),
                None => None,
            };
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(
                    RichText::new(
                        line.map_or_else(
                            || "hover a row for its console line".to_owned(),
                            |l| format!("> {l}"),
                        ),
                    )
                    .monospace()
                    .size(10.5)
                    .color(theme::WEAK),
                )
                .truncate(),
            );
        });
    if let Some(window) = window {
        shell.building.place_window("history", window.response.rect, window.response.layer_id);
    }
    if let Some((times, line)) = go {
        for _ in 0..times {
            if shell.run(line).is_err() {
                break;
            }
        }
    }
    if !open {
        shell.history_open = false;
    }
}

fn wizard(shell: &mut Shell, ctx: &egui::Context) {
    let Some(mut choices) = shell.wizard.clone() else { return };
    shell.building.window("wizard", "New scene");
    let mut open = true;
    let mut create = false;
    let mut cancel = false;
    let window = egui::Window::new("New scene").open(&mut open).collapsible(false).default_width(380.0).show(
        ctx,
        |ui| {
            let section = |shell: &mut Shell, ui: &mut egui::Ui, id: &str, title: &str| {
                let response = ui.label(RichText::new(title).color(theme::STRONG));
                shell.register("wizard", id, title, Kind::Scope, response.rect);
            };
            section(shell, ui, "wizard/array", "The array");
            for (n, text) in ARRAYS.iter().enumerate() {
                let r = ui.radio(choices.array == n, *text);
                let id = format!("wizard/array/{n}");
                shell.register("wizard/array", &id, text, Kind::Button, r.rect);
                shell.layout.buttons.push((format!("wizard array {n}"), r.rect));
                if r.clicked() || shell.take_click(&id) {
                    choices.array = n;
                }
            }
            section(shell, ui, "wizard/surroundings", "Around it");
            for (n, text) in SURROUNDINGS.iter().enumerate() {
                let r = ui.radio(choices.surroundings == n, *text);
                let id = format!("wizard/surroundings/{n}");
                shell.register("wizard/surroundings", &id, text, Kind::Button, r.rect);
                shell.layout.buttons.push((format!("wizard surroundings {n}"), r.rect));
                if r.clicked() || shell.take_click(&id) {
                    choices.surroundings = n;
                }
            }
            section(shell, ui, "wizard/body", "A first body");
            let mut options: Vec<(Option<String>, String)> = vec![(None, "none".into())];
            options.extend(
                cav_sim::starters::starters()
                    .iter()
                    .filter(|s| s.body.is_some())
                    .map(|s| (Some(s.name.clone()), s.label.clone())),
            );
            // A grid of tiles, each with a small picture of what it adds, three to a row: fifteen
            // radios in a column did not fit the window or scan (S18's second review, UX 7).
            egui::Grid::new("wizard bodies").num_columns(3).spacing([4.0, 4.0]).show(ui, |ui| {
                for (n, (name, label)) in options.iter().enumerate() {
                    let starter = name.as_deref().and_then(cav_sim::starters::find);
                    let on = choices.body == *name;
                    let (rect, r) = ui.allocate_exact_size(egui::vec2(118.0, 26.0), egui::Sense::click());
                    let visuals = ui.style().interact_selectable(&r, on);
                    let painter = ui.painter_at(rect);
                    painter.rect(
                        rect,
                        3.0,
                        visuals.weak_bg_fill,
                        visuals.bg_stroke,
                        egui::StrokeKind::Inside,
                    );
                    let icon = egui::Rect::from_center_size(
                        rect.left_center() + egui::vec2(13.0, 0.0),
                        egui::vec2(16.0, 16.0),
                    );
                    crate::icons::starter(
                        &painter,
                        icon,
                        starter.and_then(|s| s.body.as_ref()),
                        visuals.fg_stroke.color,
                    );
                    painter.text(
                        rect.left_center() + egui::vec2(26.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        label,
                        egui::FontId::proportional(11.5),
                        visuals.fg_stroke.color,
                    );
                    let r = match starter {
                        Some(s) => r.on_hover_text(&s.about),
                        None => r.on_hover_text("no body: begin with the array alone"),
                    };
                    let id = format!("wizard/body/{n}");
                    shell.register("wizard/body", &id, label, Kind::Button, r.rect);
                    shell
                        .layout
                        .buttons
                        .push((format!("wizard body {}", name.as_deref().unwrap_or("none")), r.rect));
                    if r.clicked() || shell.take_click(&id) {
                        choices.body.clone_from(name);
                    }
                    if n % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
            section(shell, ui, "wizard/duration", "How long, seconds");
            let r = ui
                .add(egui::DragValue::new(&mut choices.duration).range(1.0..=1.0e5).speed(0.5).suffix(" s"));
            shell.register("wizard/duration", "wizard/duration/value", "duration", Kind::Adjust, r.rect);
            for (_, steps, _, _) in shell.take_adjust("wizard/duration/value") {
                choices.duration = (choices.duration + steps).clamp(1.0, 1.0e5);
            }
            ui.add_space(6.0);
            let problem = choices.scene().err();
            if let Some(why) = &problem {
                ui.label(RichText::new(why).size(11.0).color(theme::WARN));
            }
            let response = ui
                .horizontal(|ui| {
                    let make = ui.add_enabled(problem.is_none(), egui::Button::new("Create"));
                    shell.layout.buttons.push(("wizard create".to_owned(), make.rect));
                    let stop = ui.button("Cancel");
                    (make, stop)
                })
                .inner;
            shell.register(
                "wizard",
                "wizard/buttons",
                "Create or cancel",
                Kind::Scope,
                response.0.rect.union(response.1.rect),
            );
            shell.register("wizard/buttons", "wizard/create", "Create", Kind::Button, response.0.rect);
            shell.register("wizard/buttons", "wizard/cancel", "Cancel", Kind::Button, response.1.rect);
            create = response.0.clicked() || shell.take_click("wizard/create");
            cancel = response.1.clicked() || shell.take_click("wizard/cancel");
        },
    );
    if let Some(window) = window {
        shell.building.place_window("wizard", window.response.rect, window.response.layer_id);
    }
    shell.wizard = Some(choices.clone());
    if create && let Ok(spec) = choices.scene() {
        // One load: one undo takes the new scene away. It has never been saved: the title says so
        // until it is (S18's review, minor 10).
        shell.apply(Command::load(spec));
        shell.file = None;
        shell.untitled = true;
        cancel = true;
    }
    if cancel || !open {
        shell.wizard = None;
        if shell.keyboard.focus.first().map(String::as_str) == Some("wizard") {
            shell.keyboard.close_window();
        }
    }
}

fn open(shell: &mut Shell, ctx: &egui::Context) {
    let Some(mut path) = shell.open_window.clone() else { return };
    shell.building.window("open", "Open");
    let mut open = true;
    let mut load: Option<String> = None;
    let window = egui::Window::new("Open a scene")
        .open(&mut open)
        .collapsible(false)
        .default_width(380.0)
        .show(ctx, |ui| {
            ui.label(RichText::new("a library scene").color(theme::STRONG));
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for scene in cav_sim::LIBRARY {
                    if scene.name == "stress-1e10" {
                        // The full 10¹⁰-cell scene is opened by name only (`load stress-1e10`): it is
                        // a measurement, not a place to begin.
                        continue;
                    }
                    let r = ui.selectable_label(false, scene.name);
                    let id = format!("open/library/{}", scene.name);
                    shell.register("open", &id, scene.name, Kind::Button, r.rect);
                    if r.clicked() || shell.take_click(&id) {
                        load = Some(scene.name.to_owned());
                    }
                }
            });
            ui.add_space(6.0);
            ui.label(RichText::new("or a file, by its path").color(theme::STRONG));
            let field = ui.add(
                egui::TextEdit::singleline(&mut path).hint_text("my-scene.ron").desired_width(f32::INFINITY),
            );
            shell.register("open", "open/path", "path", Kind::Text, field.rect);
            if shell.take_type("open/path") {
                field.request_focus();
            }
            let go = ui.button("Open the file");
            shell.register("open", "open/go", "Open the file", Kind::Button, go.rect);
            shell.layout.buttons.push(("open go".to_owned(), go.rect));
            if go.clicked()
                || shell.take_click("open/go")
                || (field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            {
                load = Some(path.clone());
            }
        });
    if let Some(window) = window {
        shell.building.place_window("open", window.response.rect, window.response.layer_id);
    }
    shell.open_window = Some(path);
    if let Some(what) = load {
        let quoted = if what.contains(char::is_whitespace) { format!("\"{what}\"") } else { what.clone() };
        if shell.run(&format!("load {quoted}")).is_ok() {
            shell.file = cav_sim::LIBRARY.iter().all(|s| s.name != what).then_some(what);
            open = false;
        }
    }
    if !open {
        shell.open_window = None;
        if shell.keyboard.focus.first().map(String::as_str) == Some("open") {
            shell.keyboard.close_window();
        }
    }
}

fn save(shell: &mut Shell, ctx: &egui::Context) {
    let Some(mut path) = shell.save_window.clone() else { return };
    shell.building.window("save", "Save");
    let mut open = true;
    let mut write = false;
    let window = egui::Window::new("Save the scene")
        .open(&mut open)
        .collapsible(false)
        .default_width(380.0)
        .show(ctx, |ui| {
            ui.label(RichText::new("to a file (in the page: a download)").color(theme::STRONG));
            let field = ui.add(egui::TextEdit::singleline(&mut path).desired_width(f32::INFINITY));
            shell.register("save", "save/path", "path", Kind::Text, field.rect);
            if shell.take_type("save/path") {
                field.request_focus();
            }
            let go = ui.button("Save");
            shell.register("save", "save/go", "Save", Kind::Button, go.rect);
            shell.layout.buttons.push(("save go".to_owned(), go.rect));
            write = go.clicked()
                || shell.take_click("save/go")
                || (field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
        });
    if let Some(window) = window {
        shell.building.place_window("save", window.response.rect, window.response.layer_id);
    }
    shell.save_window = Some(path.clone());
    if write && !path.trim().is_empty() {
        let quoted = if path.contains(char::is_whitespace) { format!("\"{path}\"") } else { path.clone() };
        if shell.run(&format!("save {quoted}")).is_ok() {
            shell.file = Some(path);
            open = false;
        }
    }
    if !open {
        shell.save_window = None;
        if shell.keyboard.focus.first().map(String::as_str) == Some("save") {
            shell.keyboard.close_window();
        }
    }
}
