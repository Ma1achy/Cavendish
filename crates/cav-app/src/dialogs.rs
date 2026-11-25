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
}

/// A step of the history in a person's words (S18's review, U6: the list showed the console's
/// derived lines, `add bodies 1 {"label":…`): "Added sphere", "Moved sphere to (2, 0, 1) m",
/// "Set duration to 20 s", "Removed tower 3", "Opened “town”". Names are the scene's as it is
/// now (`spec`); the line that does the step is [`cav_engine::history::Edit::line`], for a
/// tooltip. Only the field touched is read, never a whole inserted value, so a long list costs
/// little each frame (minor 8).
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
    format!("Set {} to {}", place(path, spec), value_words(value, unit))
}

fn history(shell: &mut Shell, ctx: &egui::Context) {
    if !shell.history_open {
        return;
    }
    shell.building.window("history", "History");
    let spec = shell.engine.spec().clone();
    // The words of each step (cheap: no inserted value is written out); the console line only
    // when a row is hovered.
    let (done, undone): (Vec<String>, Vec<String>) = {
        let (done, undone) = shell.engine.history_steps();
        (
            done.iter().map(|s| step_words(s, &spec)).collect(),
            undone.iter().map(|s| step_words(s, &spec)).collect(),
        )
    };
    let mut open = true;
    let mut go: Option<(usize, &str)> = None;
    let mut hovered: Option<(bool, usize)> = None;
    // Over the outliner, never over the figure (S18's review, U6): the outliner and the tool strip
    // are the first ~310 points of the window's left.
    let left = shell.layout.panel("outliner").map_or(ctx.content_rect().left() + 8.0, |r| r.left() + 6.0);
    let top = shell.layout.panel("outliner").map_or(60.0, |r| r.top() + 30.0);
    let window = egui::Window::new("Edit history")
        .open(&mut open)
        .collapsible(false)
        .default_pos(egui::pos2(left, top))
        .default_width(292.0)
        .max_width(300.0)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(
                    "every edit, oldest first; click one to go back or forward to it (Ctrl+Z, Ctrl+Y step)",
                )
                .size(11.0)
                .color(theme::WEAK),
            );
            egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                let start =
                    ui.selectable_label(done.is_empty(), RichText::new("The scene as it was").italics());
                shell.register("history", "history/start", "the start", Kind::Button, start.rect);
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
        shell.building.place("history", window.response.rect);
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
            for (n, (name, label)) in options.iter().enumerate() {
                let r = ui.radio(choices.body == *name, label);
                let id = format!("wizard/body/{n}");
                shell.register("wizard/body", &id, label, Kind::Button, r.rect);
                shell
                    .layout
                    .buttons
                    .push((format!("wizard body {}", name.as_deref().unwrap_or("none")), r.rect));
                if r.clicked() || shell.take_click(&id) {
                    choices.body.clone_from(name);
                }
            }
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
        shell.building.place("wizard", window.response.rect);
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
        shell.building.place("open", window.response.rect);
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
        shell.building.place("save", window.response.rect);
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
