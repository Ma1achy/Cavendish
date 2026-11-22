//! Property editors made from the scene spec's schema: one widget for every field that holds a
//! value, with its unit, its range and its doc string, under whatever path is selected.
//!
//! Nothing here holds scene state. An editor shows the value the engine has and, when it is
//! changed, issues the same `Command::Set` that the console's `set`, `cav set` and MCP's
//! `scene_set` issue ([`command_for`]); the engine checks it, journals it and can undo it. A drag
//! of a number is a run of sets that continue one another: one journal entry, one undo.

use cav_engine::Command;
use cav_sim::paths::{self, Entry, Kind};
use egui::{Align, Layout, RichText};
use serde_json::{Value, json};

use crate::shell::Shell;
use crate::theme;

/// The most fields a selection's editors show: a panel is not a list of a thousand bodies.
pub const MAX_FIELDS: usize = 200;

/// Which widget a field gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Widget {
    /// A number that can be dragged or typed, with its unit after it.
    Number,
    /// The same, in whole numbers.
    Integer,
    /// One number for each component, side by side.
    Vector(usize),
    /// A line of text.
    Text,
    /// A tick box.
    Flag,
    /// A drop-down of plain variants.
    Choice,
    /// A drop-down of variants that carry data: choosing another replaces the data.
    Variant,
    /// An optional part that is absent: a button that adds it.
    Absent,
}

/// The widget for a field, from its kind and its value. Every kind a spec can hold has one.
#[must_use]
pub fn widget_for(entry: &Entry) -> Widget {
    match (&entry.kind, &entry.value) {
        (_, Value::Null) => Widget::Absent,
        (Kind::Number, _) => Widget::Number,
        (Kind::Integer, _) => Widget::Integer,
        (Kind::Vector(n), _) => Widget::Vector(*n),
        (Kind::Text, _) => Widget::Text,
        (Kind::Flag, _) => Widget::Flag,
        (Kind::Choice(_), Value::String(_)) => Widget::Choice,
        // A list or a group is not one value: its fields are listed in its place.
        (Kind::Choice(_) | Kind::List | Kind::Group, _) => Widget::Variant,
    }
}

/// The command an editor issues when the field at `path` takes `value`: exactly what
/// `set <path> <value>` parses to. `continues` marks a step of a drag after its first.
#[must_use]
pub fn command_for(path: &str, value: Value, continues: bool) -> Command {
    Command::Set { path: path.to_owned(), value, continues }
}

/// What an absent part starts as when it is added, or a variant when it is chosen: something
/// valid to edit from. `at` is where a body is now, for motions.
fn starter(spec: &cav_sim::SceneSpec, path: &str, variant: Option<&str>) -> Option<Value> {
    if let (Some(kind), Some(body)) = (variant, path.strip_suffix(".motion")) {
        let whole = spec.to_value();
        let at = paths::get(&whole, body)
            .ok()
            .and_then(|body| serde_json::from_value::<cav_sim::BodySpec>(body.clone()).ok())
            .map_or([0.0, 0.0, 1.0], |body| body.motion.anchor().0);
        return cav_sim::Motion::starter(kind, at).and_then(|motion| serde_json::to_value(motion).ok());
    }
    match (path, variant) {
        ("room", None) => serde_json::to_value(cav_sim::SceneSpec::room().room).ok(),
        ("floor", None) => Some(json!({"height": 0.0, "kind": "HalfSpace", "density": 2400.0})),
        ("floor.kind", Some("Slab")) => Some(json!({"Slab": {"thickness": 1.0}})),
        (_, Some(name)) => paths::template(path).map(|first| match first {
            // The template is of the first variant; a plain variant is its own name.
            Value::Object(_) => first,
            _ => Value::String(name.to_owned()),
        }),
        (_, None) => paths::template(path),
    }
}

/// How fast a dragged number moves, per point of pointer travel: a hundredth of its size, and
/// never nothing.
fn speed(value: f64) -> f64 {
    (value.abs() * 0.01).max(1e-3)
}

/// The range a number may be dragged over: the schema's, or everything.
fn range(entry: &Entry) -> std::ops::RangeInclusive<f64> {
    let low = entry.minimum.map_or(f64::NEG_INFINITY, |(low, _)| low);
    low..=entry.maximum.unwrap_or(f64::INFINITY)
}

/// One number: dragged or typed. Returns the new value when it changed, and whether the change
/// is a step of a drag.
fn number(ui: &mut egui::Ui, value: f64, entry: &Entry, whole: bool) -> (Option<f64>, egui::Response) {
    let mut edited = value;
    let unit =
        entry.unit.as_deref().filter(|unit| *unit != "1").map_or_else(String::new, |unit| format!(" {unit}"));
    let mut drag = egui::DragValue::new(&mut edited).speed(speed(value)).range(range(entry)).suffix(unit);
    drag = if whole { drag.max_decimals(0) } else { drag.max_decimals(9) };
    let response = ui.add(drag);
    ((edited != value).then_some(edited), response)
}

/// Draws the editors for everything under `prefix` and applies what they change.
pub fn show(shell: &mut Shell, ui: &mut egui::Ui, prefix: &str) {
    let spec = shell.engine.spec().clone();
    let entries = match paths::leaves(&spec, prefix, MAX_FIELDS) {
        Ok(entries) => entries,
        Err(why) => {
            ui.label(RichText::new(why).size(11.0).color(theme::WARN));
            return;
        }
    };
    let mut commands: Vec<Command> = Vec::new();
    egui::Grid::new(("editors", prefix)).num_columns(2).spacing([8.0, 3.0]).show(ui, |ui| {
        for entry in &entries {
            // The name under the selection: `motion.Linear.frequency` reads as `frequency`,
            // set in under the choice it belongs to.
            let below = entry.path.strip_prefix(prefix).unwrap_or(&entry.path).trim_start_matches('.');
            let depth = below.matches('.').count();
            let name = below.rsplit('.').next().unwrap_or(below);
            let hover = format!("{}\n\n{}", entry.doc, entry.path);
            ui.horizontal(|ui| {
                ui.add_space(8.0 * depth as f32);
                ui.label(RichText::new(name).color(theme::WEAK).size(12.0)).on_hover_text(&hover);
            });
            let start = ui.cursor().min;
            ui.horizontal(|ui| edit(shell, ui, &spec, entry, &mut commands));
            let took =
                egui::Rect::from_min_max(start, egui::pos2(ui.min_rect().right(), ui.min_rect().bottom()));
            shell.layout.editors.push((entry.path.clone(), took));
            ui.end_row();
        }
    });
    if entries.len() >= MAX_FIELDS {
        ui.label(
            RichText::new(format!("the first {MAX_FIELDS} fields; `ls {prefix}` lists them all"))
                .size(11.0)
                .color(theme::WEAK),
        );
    }
    for command in commands {
        // A step that is refused continues nothing, and nothing continues it.
        if !shell.apply(command) {
            shell.editing = None;
        }
    }
}

/// One field's widget. What it changes is pushed onto `commands`.
fn edit(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    spec: &cav_sim::SceneSpec,
    entry: &Entry,
    commands: &mut Vec<Command>,
) {
    let path = entry.path.as_str();
    // A step of a drag continues the edit before it when that edit was of this path and applied.
    let mut dragged = |response: &egui::Response, path: &str, value: Value, shell: &mut Shell| {
        let continues = response.dragged() && shell.editing.as_deref() == Some(path);
        if response.dragged() && !continues {
            // The first step of a drag: mark it, so that the steps after it continue it.
            shell.editing = Some(path.to_owned());
        }
        commands.push(command_for(path, value, continues));
    };
    match widget_for(entry) {
        Widget::Number | Widget::Integer => {
            let whole = entry.kind == Kind::Integer;
            let (changed, response) = number(ui, entry.value.as_f64().unwrap_or_default(), entry, whole);
            if let Some(value) = changed {
                let value = if whole { json!(value as i64) } else { json!(value) };
                dragged(&response, path, value, shell);
            }
            if response.drag_stopped() {
                shell.editing = None;
            }
            if entry.optional && ui.small_button("×").on_hover_text("none").clicked() {
                commands.push(command_for(path, Value::Null, false));
            }
        }
        Widget::Vector(n) => {
            for axis in 0..n {
                let value = entry.value[axis].as_f64().unwrap_or_default();
                let component = format!("{path}[{axis}]");
                let (changed, response) = number(ui, value, entry, false);
                if let Some(value) = changed {
                    dragged(&response, &component, json!(value), shell);
                }
                if response.drag_stopped() {
                    shell.editing = None;
                }
            }
        }
        Widget::Text => {
            let current = entry.value.as_str().unwrap_or_default().to_owned();
            let mut text = match &shell.typing {
                Some((typing, buffer)) if typing == path => buffer.clone(),
                _ => current.clone(),
            };
            let response = ui.add(egui::TextEdit::singleline(&mut text).desired_width(150.0));
            if response.has_focus() {
                shell.typing = Some((path.to_owned(), text.clone()));
            }
            if response.lost_focus() {
                shell.typing = None;
                if text != current {
                    commands.push(command_for(path, Value::String(text), false));
                }
            }
        }
        Widget::Flag => {
            let mut on = entry.value.as_bool().unwrap_or_default();
            if ui.checkbox(&mut on, "").changed() {
                commands.push(command_for(path, Value::Bool(on), false));
            }
        }
        Widget::Choice | Widget::Variant => {
            let Kind::Choice(names) = &entry.kind else {
                ui.label(RichText::new(entry.kind.name()).size(11.0).color(theme::WEAK));
                return;
            };
            let now = entry.variant.clone().unwrap_or_default();
            egui::ComboBox::from_id_salt(("choice", path)).selected_text(now.as_str()).show_ui(ui, |ui| {
                for name in names {
                    if ui.selectable_label(*name == now, name).clicked() && *name != now {
                        let value =
                            starter(spec, path, Some(name)).unwrap_or_else(|| Value::String(name.clone()));
                        commands.push(command_for(path, value, false));
                    }
                }
            });
        }
        Widget::Absent => {
            ui.label(RichText::new("none").size(12.0).color(theme::WEAK));
            if ui.small_button("add").clicked() {
                let value = match (&entry.kind, starter(spec, path, None)) {
                    // A number that may be absent starts at the playhead's side of sense: zero.
                    (Kind::Number, _) => json!(entry.minimum.map_or(0.0, |(low, _)| low)),
                    (_, Some(value)) => value,
                    (_, None) => Value::Null,
                };
                commands.push(command_for(path, value, false));
            }
        }
    }
}

/// The buttons under a selection that change a list or take an optional part away: remove this
/// body, remove this tower, remove the room. Each is a command like any other.
pub fn structure(shell: &mut Shell, ui: &mut egui::Ui, prefix: &str) {
    let spec = shell.engine.spec();
    let command = match prefix {
        "room" | "floor" => Some((format!("remove the {prefix}"), command_for(prefix, Value::Null, false))),
        _ if prefix.starts_with("bodies[") => {
            Some(("remove this body".to_owned(), Command::Remove { path: prefix.to_owned() }))
        }
        _ if prefix.starts_with("array.towers[") && spec.array.towers.len() > 1 => {
            Some(("remove this tower".to_owned(), Command::Remove { path: prefix.to_owned() }))
        }
        _ => None,
    };
    if let Some((label, command)) = command {
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            let response = ui.small_button(label);
            shell.layout.buttons.push((cav_engine::describe(&command, 0), response.rect));
            if response.clicked() {
                shell.apply(command);
            }
        });
    }
}
