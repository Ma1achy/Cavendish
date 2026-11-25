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

use crate::keyboard::Kind as Focus;
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
    // A body's first part: a ball of its own material on its surface, two cells across.
    if let Some(shape) = path.strip_suffix(".parts[]") {
        let pitch = paths::get(&spec.to_value(), &format!("{shape}.pitch")).ok()?.as_f64()?;
        return Some(
            json!({"mode": "Add", "at": [0.0, 0.0, 0.0], "kind": {"Solid": {"shape": {"Sphere": {"radius": 2.0 * pitch}}}}}),
        );
    }
    match (path, variant) {
        ("room", None) => serde_json::to_value(cav_sim::SceneSpec::room().room).ok(),
        ("floor", None) => Some(json!({"height": 0.0, "kind": "HalfSpace", "density": 2400.0})),
        ("floor.kind", Some("Slab")) => Some(json!({"Slab": {"thickness": 1.0}})),
        // The parts of a schedule start as something gentle and valid for any instrument.
        ("array.schedule.jitter", None) => Some(json!({"model": "Walk", "kind": "Positive", "delta": 0.1})),
        ("array.schedule.alternate", None) => Some(json!(spec.array.instrument.cycle * 1.25)),
        ("array.schedule.gaps", None) => Some(json!({"on": 4, "period": 5})),
        ("array.schedule.window", None) => {
            Some(json!({"start": 0.0, "length": spec.duration / 4.0, "period": spec.duration / 2.0}))
        }
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

/// A number as an editor shows it: as short as says it, to six figures, with an exponent when
/// it is very small or very large (the atom's mass is 1.44316e-25 kg, not 0.000).
#[must_use]
pub fn shown(value: f64) -> String {
    let size = value.abs();
    if value == 0.0 {
        "0".to_owned()
    } else if (1e-3..1e7).contains(&size) {
        let digits = (5 - size.log10().floor() as i32).clamp(0, 9) as usize;
        let text = format!("{value:.digits$}");
        if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.').to_owned() } else { text }
    } else {
        format!("{value:.5e}")
    }
}

/// The width of the column of names, and of one number, points: together they fit the panel.
const NAME_WIDTH: f32 = 92.0;
const NUMBER_WIDTH: f32 = 88.0;
const COMPONENT_WIDTH: f32 = 44.0;

/// One number: dragged or typed, `width` points wide. Returns the new value when it changed.
fn number(
    ui: &mut egui::Ui,
    value: f64,
    entry: &Entry,
    whole: bool,
    width: f32,
) -> (Option<f64>, egui::Response) {
    let mut edited = value;
    // An atom's mass is shown and typed in unified atomic mass units (86.9092 u for ⁸⁷Sr, not
    // 1.44316e-25 kg: S18's second review, UX 8); the value stays in kilograms.
    let atomic = entry.unit.as_deref() == Some("kg") && value.abs() < 1e-20;
    let scale = if atomic { cav_core::constants::ATOMIC_MASS_UNIT } else { 1.0 };
    let unit = if atomic { Some("u") } else { entry.unit.as_deref() };
    let unit = unit.filter(|unit| *unit != "1" && width >= NUMBER_WIDTH);
    let unit = unit.map_or_else(String::new, |unit| format!(" {unit}"));
    let mut drag = egui::DragValue::new(&mut edited)
        .speed(speed(value))
        .range(range(entry))
        .suffix(unit)
        .custom_formatter(move |value, _| shown(value / scale))
        // The field shows six figures; the value has seventeen. Text that is still what the
        // field showed is the value itself, not its six figures read back: leaving a field
        // without typing must not round it (S2's review, finding 17: Tab into a field and Enter
        // issued a journalled edit of the mass to 1.23457).
        .custom_parser(move |text| {
            let text = text.trim();
            if text == shown(value / scale) {
                Some(value)
            } else {
                text.parse::<f64>().ok().map(|v| v * scale)
            }
        });
    if whole {
        drag = drag.max_decimals(0);
    }
    let response = ui.add_sized([width, 18.0], drag);
    ((edited != value).then_some(edited), response)
}

/// The keyboard's id of the editor of the field at `path`: `field/bodies[0].mass`.
#[must_use]
pub fn field_id(path: &str) -> String {
    format!("field/{path}")
}

/// `value` moved by `steps` of its step ([`crate::keyboard::step_for`]), kept to the step's
/// decimals so that 2.5 + 0.1 is 2.6 and not 2.6000000000000001, and to the field's range.
fn nudged(value: f64, steps: f64, entry: &Entry, whole: bool) -> f64 {
    let step = crate::keyboard::step_for(value, entry.unit.as_deref(), whole) * steps.abs();
    let raw = value + step * steps.signum();
    let decimals = (-step.log10().floor()).max(0.0) as usize;
    let tidy = format!("{raw:.decimals$}").parse::<f64>().unwrap_or(raw);
    let next = if (tidy - raw).abs() <= step * 1e-6 { tidy } else { raw };
    let range = range(entry);
    next.clamp(*range.start(), *range.end())
}

/// Draws the editors for everything under `prefix` and applies what they change. Each field is
/// an element of the keyboard's scope `scope` ([`field_id`]), so it is reached by Tab and the
/// arrows, adjusted with them, typed into, or chosen from, as the spec says.
pub fn show(shell: &mut Shell, ui: &mut egui::Ui, scope: &str, prefix: &str) {
    let spec = shell.engine.spec().clone();
    let entries = match paths::leaves(&spec, prefix, MAX_FIELDS) {
        Ok(entries) => entries,
        Err(why) => {
            ui.label(RichText::new(why).size(11.0).color(theme::WARN));
            return;
        }
    };
    let mut commands: Vec<Command> = Vec::new();
    // The scene as it was opened or saved: a field that differs from it is marked (gold) and
    // has a reset that puts its value back (S18 E7).
    let baseline = shell.engine.baseline().to_value();
    // Sections: the fields straight under the selection first, then one section for each part
    // of it (`shape`, `motion`, …) in the schema's order, each a scope of the keyboard's tree
    // that folds (`docs/research/editor-ux.md` §3.6).
    let section_of = |entry: &Entry| -> String {
        let below = entry.path.strip_prefix(prefix).unwrap_or(&entry.path).trim_start_matches('.');
        match below.find(['.', '[']) {
            Some(end) => below[..end].to_owned(),
            None => String::new(),
        }
    };
    let mut sections: Vec<String> = Vec::new();
    for entry in &entries {
        let section = section_of(entry);
        if !sections.contains(&section) {
            sections.push(section);
        }
    }
    for section in sections {
        let in_section: Vec<&Entry> = entries.iter().filter(|e| section_of(e) == section).collect();
        let mut grid = |shell: &mut Shell, ui: &mut egui::Ui, scope: &str| {
            egui::Grid::new(("editors", prefix, section.as_str())).num_columns(2).spacing([8.0, 3.0]).show(
                ui,
                |ui| {
                    for entry in &in_section {
                        row(shell, ui, &spec, &baseline, scope, prefix, entry, &mut commands);
                        ui.end_row();
                    }
                },
            );
        };
        if section.is_empty() {
            grid(shell, ui, scope);
            continue;
        }
        let id = format!("{scope}/{prefix}/{section}");
        // What the section is, in words: "Motion", "Shape", "Timing" (S18's review, U4).
        let title = paths::words(&section);
        // The section first, so that its fields find it; where it is, once it is drawn. From the
        // keyboard Enter on it goes in (or, folded, opens it); a click on its header folds it.
        let opened = shell.take_click(&id);
        shell.building.add(scope, &id, &title, Focus::Scope);
        let header = egui::CollapsingHeader::new(RichText::new(&title).size(12.0).color(theme::STRONG))
            .id_salt(("section", prefix, section.as_str()))
            .default_open(true)
            .open(opened.then_some(true))
            .show(ui, |ui| grid(shell, ui, &id));
        shell.building.place(&id, header.header_response.rect);
        if header.body_returned.is_none() {
            shell.building.add(scope, &id, &title, Focus::Button);
        }
    }
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

/// One row of the inspector: the field's name in words (gold, with a gold bar at the panel's edge,
/// when it differs from the scene as opened or saved), its widget, and its reset (`‹`, drawn only
/// on a changed field; always in its place in the keyboard's order, disabled when there is
/// nothing to put back). What it changes is pushed onto `commands`.
#[allow(clippy::too_many_arguments)]
fn row(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    spec: &cav_sim::SceneSpec,
    baseline: &Value,
    scope: &str,
    prefix: &str,
    entry: &Entry,
    commands: &mut Vec<Command>,
) {
    // The name under the selection, in words: `motion.Linear.frequency` reads as "Frequency", set
    // in under the choice it belongs to; `tower_height` as "Tower height", or the schema's title
    // ("Interferometer separation Δr"). S18's review, U1.
    let below = entry.path.strip_prefix(prefix).unwrap_or(&entry.path).trim_start_matches('.');
    let depth = below.matches('.').count().saturating_sub(1);
    let label = entry.label();
    let name = label.as_str();
    let saved = paths::get(baseline, &entry.path).ok().cloned();
    let differs = saved.as_ref().is_some_and(|saved| *saved != entry.value);
    // The doc string first; the console's path after it, for whoever wants to type it.
    let hover = format!(
        "{}\n\n`{}`{}",
        entry.doc,
        entry.path,
        if differs {
            "\n\nChanged since the scene was opened or saved: ‹ or Backspace puts it back."
        } else {
            ""
        }
    );
    // A vector says its unit once, beside its name; its components have no room for it.
    let unit = match (&entry.kind, entry.unit.as_deref()) {
        (Kind::Vector(_), Some(unit)) if unit != "1" => format!(", {unit}"),
        _ => String::new(),
    };
    let indent = " ".repeat(2 * depth);
    let colour = if differs { theme::GOLD } else { theme::WEAK };
    let text =
        egui::Label::new(RichText::new(format!("{indent}{name}{unit}")).color(colour).size(12.0)).truncate();
    let named = theme::fixed_left(ui, egui::vec2(NAME_WIDTH, 18.0), text).on_hover_text(&hover);
    if differs {
        // Unity's mark for an overridden field: a bar at the edge, beside the gold name.
        let x = ui.max_rect().left() - 6.0;
        ui.painter().vline(x, named.rect.y_range(), egui::Stroke::new(2.0, theme::GOLD));
    }
    let start = ui.cursor().min;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        edit(shell, ui, spec, scope, name, entry, commands);
        if matches!(entry.kind, Kind::List) {
            list_buttons(shell, ui, spec, scope, name, entry, commands);
        }
        let reset_id = format!("{}/reset", field_id(&entry.path));
        // Drawn only where there is something to put back; where there is not, its place is
        // kept (and its stop in the keyboard's order, disabled) without a mark to read.
        let reset = ui
            .add_visible(
                differs,
                egui::Button::new(RichText::new("‹").size(12.0).color(theme::GOLD)).small().frame(false),
            )
            .on_hover_text("put back the value the scene was opened or saved with (Backspace)");
        shell.register(scope, &reset_id, &format!("{name}: reset"), Focus::Button, reset.rect);
        if differs {
            // Only a reset that is drawn is in the layout's buttons (what a test can see).
            shell.layout.buttons.push((format!("reset {}", entry.path), reset.rect));
        }
        if !differs {
            shell.building.disable(&reset_id);
        }
        if differs && (reset.clicked() || shell.take_click(&reset_id)) {
            commands.push(command_for(&entry.path, saved.clone().unwrap_or(Value::Null), false));
        }
    });
    let took = egui::Rect::from_min_max(start, egui::pos2(ui.min_rect().right(), ui.min_rect().bottom()));
    shell.layout.editors.push((entry.path.clone(), took));
}

/// A list's own buttons: `+` adds an item (a copy of the last, so it is valid as the others
/// are), `−` takes the last away; each one edit, refused with the reason when the list would be
/// outside its bounds (a motion's modes, one to 64).
fn list_buttons(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    spec: &cav_sim::SceneSpec,
    scope: &str,
    name: &str,
    entry: &Entry,
    commands: &mut Vec<Command>,
) {
    let items = entry.value.as_array().map_or(0, Vec::len);
    let id = field_id(&entry.path);
    let add = ui.small_button("+").on_hover_text("add an item: a copy of the last");
    shell.register(scope, &format!("{id}/add"), &format!("{name}: add"), Focus::Button, add.rect);
    // A copy of the last; for an empty list, its first item's starter where it has one (a
    // body's first part: a small ball of its own material, S19).
    let first = || starter(spec, &format!("{}[]", entry.path), None);
    if (add.clicked() || shell.take_click(&format!("{id}/add")))
        && let Some(value) = entry.value.as_array().and_then(|a| a.last()).cloned().or_else(first)
    {
        commands.push(Command::Insert { path: entry.path.clone(), index: None, value });
    }
    let remove = ui.small_button("-").on_hover_text("take the last item away");
    shell.register(scope, &format!("{id}/remove"), &format!("{name}: remove"), Focus::Button, remove.rect);
    if (remove.clicked() || shell.take_click(&format!("{id}/remove"))) && items > 0 {
        commands.push(Command::Remove { path: format!("{}[{}]", entry.path, items - 1) });
    }
}

/// One field's widget, registered in the keyboard's `scope` as `name`. What it changes is pushed
/// onto `commands`.
fn edit(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    spec: &cav_sim::SceneSpec,
    scope: &str,
    name: &str,
    entry: &Entry,
    commands: &mut Vec<Command>,
) {
    let path = entry.path.as_str();
    let id = field_id(path);
    // Keys: each adjustment of a held key continues the edit before it (one undo for the hold);
    // a new press is a new edit. The end of the interaction ends the run.
    let keyed =
        |shell: &mut Shell, id: &str, path: &str, value: f64, whole: bool, commands: &mut Vec<Command>| {
            let mut value = value;
            for (_, steps, _, repeat) in shell.take_adjust(id) {
                value = nudged(value, steps, entry, whole);
                let continues = repeat && shell.editing.as_deref() == Some(path);
                shell.editing = Some(path.to_owned());
                let json = if whole { json!(value as i64) } else { json!(value) };
                commands.push(command_for(path, json, continues));
            }
            if shell.take_release(id).is_some() && shell.editing.as_deref() == Some(path) {
                shell.editing = None;
            }
        };
    // A step of a drag continues the edit before it when that edit was of this path and applied.
    let dragged = |response: &egui::Response,
                   path: &str,
                   value: Value,
                   shell: &mut Shell,
                   commands: &mut Vec<Command>| {
        let continues = response.dragged() && shell.editing.as_deref() == Some(path);
        if response.dragged() && !continues {
            // The first step of a drag: mark it, so that the steps after it continue it.
            shell.editing = Some(path.to_owned());
        }
        commands.push(command_for(path, value, continues));
    };
    // A voxel body's material and its parts' are chosen from the materials there are, not typed
    // (S19); a body of materials weighs what they make it, which is shown, not edited.
    if let Some(none) = material_field(path) {
        material_choice(shell, ui, spec, scope, name, entry, none, commands);
        return;
    }
    if let Some(n) = materials_set_the_mass(spec, path) {
        let mass = shell.engine.body_tree(n).map(|tree| tree.body().mass);
        let text = mass.map_or_else(
            || "from its materials".to_owned(),
            |kg| format!("{} kg, from its materials", shown(kg)),
        );
        let label = ui.label(RichText::new(text).size(12.0).color(theme::TEXT));
        shell.register(scope, &id, name, Focus::ReadOnly, label.rect);
        return;
    }
    match widget_for(entry) {
        Widget::Number | Widget::Integer => {
            let whole = entry.kind == Kind::Integer;
            let now = entry.value.as_f64().unwrap_or_default();
            let (changed, response) = number(ui, now, entry, whole, NUMBER_WIDTH);
            shell.register(scope, &id, name, Focus::Adjust, response.rect);
            if let Some(value) = changed {
                let value = if whole { json!(value as i64) } else { json!(value) };
                dragged(&response, path, value, shell, commands);
            }
            if response.drag_stopped() {
                shell.editing = None;
            }
            keyed(shell, &id, path, now, whole, commands);
            if entry.optional {
                let none = ui.small_button("×").on_hover_text("none");
                let none_id = format!("{id}/none");
                shell.register(scope, &none_id, &format!("{name}: none"), Focus::Button, none.rect);
                if none.clicked() || shell.take_click(&none_id) {
                    commands.push(command_for(path, Value::Null, false));
                }
            }
        }
        Widget::Vector(n) => {
            for axis in 0..n {
                let value = entry.value[axis].as_f64().unwrap_or_default();
                let component = format!("{path}[{axis}]");
                let component_id = field_id(&component);
                let (changed, response) = number(ui, value, entry, false, COMPONENT_WIDTH);
                let axis_name = format!("{name} {}", ["x", "y", "z", "w"].get(axis).copied().unwrap_or("·"));
                shell.register(scope, &component_id, &axis_name, Focus::Adjust, response.rect);
                if let Some(value) = changed {
                    dragged(&response, &component, json!(value), shell, commands);
                }
                if response.drag_stopped() {
                    shell.editing = None;
                }
                keyed(shell, &component_id, &component, value, false, commands);
            }
        }
        Widget::Text => {
            let current = entry.value.as_str().unwrap_or_default().to_owned();
            let mut text = match &shell.typing {
                Some((typing, buffer)) if typing == path => buffer.clone(),
                _ => current.clone(),
            };
            let response = ui.add(egui::TextEdit::singleline(&mut text).desired_width(132.0));
            shell.register(scope, &id, name, Focus::Text, response.rect);
            if shell.take_type(&id) {
                response.request_focus();
            }
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
            let response = ui.checkbox(&mut on, "");
            shell.register(scope, &id, name, Focus::Button, response.rect);
            if response.changed() {
                commands.push(command_for(path, Value::Bool(on), false));
            } else if shell.take_click(&id) {
                commands.push(command_for(path, Value::Bool(!on), false));
            }
        }
        Widget::Choice | Widget::Variant => {
            let Kind::Choice(names) = &entry.kind else {
                ui.label(RichText::new(paths::words(&entry.kind.name())).size(11.0).color(theme::WEAK));
                return;
            };
            let now = entry.variant.clone().unwrap_or_default();
            let choose = |name: &String, commands: &mut Vec<Command>| {
                let value = starter(spec, path, Some(name)).unwrap_or_else(|| Value::String(name.clone()));
                commands.push(command_for(path, value, false));
            };
            // The variants in words ("Lower minus upper"); what is set is the variant's name.
            let combo = egui::ComboBox::from_id_salt(("choice", path))
                .width(132.0)
                .selected_text(paths::words(&now))
                .show_ui(ui, |ui| {
                    for name in names {
                        if ui.selectable_label(*name == now, paths::words(name)).clicked() && *name != now {
                            choose(name, commands);
                        }
                    }
                });
            let current = names.iter().position(|n| *n == now).unwrap_or(0);
            shell.register(
                scope,
                &id,
                name,
                Focus::Choice { count: names.len(), current },
                combo.response.rect,
            );
            // Opened from the keyboard: the choices below it, the highlighted one lit (the
            // router holds ↑ ↓ Enter Esc while it is open).
            if let Some(popup) = shell.keyboard.popup.as_ref().filter(|p| p.id == id) {
                let at = popup.at;
                egui::Area::new(egui::Id::unique(("keyboard choice", path)))
                    .order(egui::Order::Foreground)
                    .fixed_pos(combo.response.rect.left_bottom())
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            for (n, choice) in names.iter().enumerate() {
                                let _ = ui.selectable_label(n == at, paths::words(choice));
                            }
                        });
                    });
            }
            if let Some(index) = shell.take_pick(&id)
                && let Some(name) = names.get(index)
                && *name != now
            {
                choose(name, commands);
            }
        }
        Widget::Absent => {
            ui.label(RichText::new("none").size(12.0).color(theme::WEAK));
            let add = ui.small_button("add");
            shell.register(scope, &id, &format!("{name}: add"), Focus::Button, add.rect);
            if add.clicked() || shell.take_click(&id) {
                let value = match (&entry.kind, starter(spec, path, None)) {
                    (_, Some(value)) if !value.is_null() && path.starts_with("array.schedule") => value,
                    // A number that may be absent starts at the bottom of its range, or zero.
                    (Kind::Number, _) => json!(entry.minimum.map_or(0.0, |(low, _)| low)),
                    (_, Some(value)) => value,
                    (_, None) => Value::Null,
                };
                commands.push(command_for(path, value, false));
            }
        }
    }
}

/// Whether `path` is a voxel body's material or a part's, and if so what choosing none means:
/// a body with none is uniform; a part with none is of its body's own material.
#[must_use]
pub fn material_field(path: &str) -> Option<&'static str> {
    let before = path.strip_suffix(".material")?;
    if before.ends_with(".shape") {
        Some("none: uniform, of its mass")
    } else if before.rsplit_once(".parts[").is_some_and(|(owner, _)| owner.ends_with(".shape")) {
        Some("the body's own")
    } else {
        None
    }
}

/// The body whose `mass` field `path` is, when its materials set its mass (so the field is not
/// used).
fn materials_set_the_mass(spec: &cav_sim::SceneSpec, path: &str) -> Option<usize> {
    let n: usize = path.strip_prefix("bodies[")?.strip_suffix("].mass")?.parse().ok()?;
    spec.bodies.get(n)?.shape.as_ref()?.material.as_ref().map(|_| n)
}

/// A drop-down of the materials a scene can name (its own, then the library's), and none: what a
/// choice sets is the material's label, or nothing.
#[allow(clippy::too_many_arguments)]
fn material_choice(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    spec: &cav_sim::SceneSpec,
    scope: &str,
    name: &str,
    entry: &Entry,
    none: &str,
    commands: &mut Vec<Command>,
) {
    let path = entry.path.as_str();
    let id = field_id(path);
    let now = entry.value.as_str().map(str::to_owned);
    let mut choices: Vec<Option<String>> = vec![None];
    choices.extend(cav_sim::materials::every(&spec.materials).into_iter().map(|m| Some(m.label.clone())));
    let words = |choice: &Option<String>| choice.clone().unwrap_or_else(|| none.to_owned());
    let mut chosen: Option<Option<String>> = None;
    let combo = egui::ComboBox::from_id_salt(("material", path))
        .width(132.0)
        .selected_text(words(&now))
        .show_ui(ui, |ui| {
            for choice in &choices {
                if ui.selectable_label(*choice == now, words(choice)).clicked() && *choice != now {
                    chosen = Some(choice.clone());
                }
            }
        });
    let current = choices.iter().position(|c| *c == now).unwrap_or(0);
    shell.register(scope, &id, name, Focus::Choice { count: choices.len(), current }, combo.response.rect);
    if let Some(popup) = shell.keyboard.popup.as_ref().filter(|p| p.id == id) {
        let at = popup.at;
        egui::Area::new(egui::Id::unique(("keyboard material", path)))
            .order(egui::Order::Foreground)
            .fixed_pos(combo.response.rect.left_bottom())
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    for (n, choice) in choices.iter().enumerate() {
                        let _ = ui.selectable_label(n == at, words(choice));
                    }
                });
            });
    }
    if let Some(index) = shell.take_pick(&id)
        && let Some(choice) = choices.get(index)
        && *choice != now
    {
        chosen = Some(choice.clone());
    }
    if let Some(choice) = chosen {
        commands.push(command_for(path, choice.map_or(Value::Null, Value::String), false));
    }
}

/// The buttons under a selection that change a list or take an optional part away: remove this
/// body, remove this tower, remove the room. Each is a command like any other.
pub fn structure(shell: &mut Shell, ui: &mut egui::Ui, scope: &str, prefix: &str) {
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
            let response = ui.small_button(&label);
            shell.layout.buttons.push((cav_engine::describe(&command, 0), response.rect));
            let id = format!("{scope}/remove");
            shell.register(scope, &id, &label, Focus::Button, response.rect);
            if response.clicked() || shell.take_click(&id) {
                shell.apply(command);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The editors shown for everything under `prefix`, drawn once with no window.
    fn drawn(shell: &mut Shell, prefix: &str) -> usize {
        let ctx = crate::headless::context();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 400.0))),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| show(shell, ui, "inspector", prefix));
        });
        // Nothing is painted here, so the font atlas has nowhere to go.
        output.textures_delta.clear();
        // egui may lay a frame out twice; the shell clears its record each frame, and here the
        // fields are counted by their paths instead.
        let paths: std::collections::BTreeSet<&str> =
            shell.layout().editors.iter().map(|(path, _)| path.as_str()).collect();
        let count = paths.len();
        shell.layout.editors.clear();
        count
    }

    #[test]
    fn a_long_list_is_shown_to_its_cap_and_no_further() {
        // A22 of S2's brief: a list shows at most MAX_FIELDS fields. (S2's review, finding 7:
        // no test noticed the cap being changed, because no selection the panels make is that
        // long. This asks for one that is: every body of a scene with a hundred of them.)
        let mut shell = Shell::new();
        let mut scene = cav_sim::dev::point();
        for n in 1..100 {
            scene.bodies.push(cav_sim::BodySpec::point(&format!("body {n}"), 1.0, [n as f64, 0.0, 1.0]));
        }
        shell.engine.apply(Command::load(scene)).unwrap();
        let all = paths::leaves(shell.engine.spec(), "bodies", usize::MAX).unwrap().len();
        assert!(all > 2 * MAX_FIELDS, "the scene has {all} fields under `bodies`");
        assert_eq!(drawn(&mut shell, "bodies"), MAX_FIELDS, "the cap is what is drawn");
        // The number itself is the brief's.
        assert_eq!(MAX_FIELDS, 200);
        // Control: one body is under the cap and is shown whole, so the cap is what cut the list.
        let one = paths::leaves(shell.engine.spec(), "bodies[0]", usize::MAX).unwrap().len();
        assert!(one < MAX_FIELDS);
        assert_eq!(drawn(&mut shell, "bodies[0]"), one);
    }
}
