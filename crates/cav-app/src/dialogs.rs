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

fn history(shell: &mut Shell, ctx: &egui::Context) {
    if !shell.history_open {
        return;
    }
    shell.building.window("history", "History");
    let (done, undone) = shell.engine.steps();
    let mut open = true;
    let mut go: Option<(usize, &str)> = None;
    let window = egui::Window::new("Edit history")
        .open(&mut open)
        .collapsible(false)
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.label(
                RichText::new("every edit, oldest first; click one to go back or forward to it")
                    .size(11.0)
                    .color(theme::WEAK),
            );
            egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                let start = ui.selectable_label(false, RichText::new("the scene as it was").italics());
                shell.register("history", "history/start", "the start", Kind::Button, start.rect);
                shell.layout.buttons.push(("history 0".to_owned(), start.rect));
                if (start.clicked() || shell.take_click("history/start")) && !done.is_empty() {
                    go = Some((done.len(), "undo"));
                }
                for (n, said) in done.iter().enumerate() {
                    let now = n + 1 == done.len();
                    let text = RichText::new(format!("{}{said}", if now { "▸ " } else { "" }))
                        .monospace()
                        .size(11.5);
                    let row = ui.selectable_label(now, text);
                    let id = format!("history/done/{n}");
                    shell.register("history", &id, said, Kind::Button, row.rect);
                    shell.layout.buttons.push((format!("history {}", n + 1), row.rect));
                    if (row.clicked() || shell.take_click(&id)) && !now {
                        go = Some((done.len() - 1 - n, "undo"));
                    }
                }
                for (n, said) in undone.iter().enumerate() {
                    let row = ui.selectable_label(
                        false,
                        RichText::new(said).monospace().size(11.5).color(theme::WEAK),
                    );
                    let id = format!("history/undone/{n}");
                    shell.register("history", &id, said, Kind::Button, row.rect);
                    shell.layout.buttons.push((format!("history {}", done.len() + n + 1), row.rect));
                    if row.clicked() || shell.take_click(&id) {
                        go = Some((n + 1, "redo"));
                    }
                }
            });
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
        // One load: one undo takes the new scene away.
        shell.apply(Command::load(spec));
        shell.file = None;
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
