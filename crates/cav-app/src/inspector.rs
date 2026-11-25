//! The inspector as component cards (S19, `docs/design/panels.md`): a header for the selection
//! (its icon, its editable name, its type, an eye, a ⋮ menu with duplicate, delete, reset all,
//! copy and paste), then a card a part of it (Body, Shape, Motion; Scene, Array, Cadence,
//! Instrument), every card on one grid whose value column starts at the same x, and an Info card
//! of what is worked out and not edited. No help paragraphs: what a field is lives on its name's
//! hover. Each control is a console line or a `Command`, through the same path as before.

use cav_engine::{Command, Target};
use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, pos2, vec2};
use serde_json::Value;

use crate::Shell;
use crate::keyboard::Kind;
use crate::theme;

/// A card's ground: a step lighter than the panel's.
pub const CARD: Color32 = Color32::from_rgb(0x22, 0x22, 0x22);
/// The least width of the label column, points (the design's 96 pt).
pub const LABEL_MIN: f32 = 96.0;
/// The label column's share of the panel's width.
pub const LABEL_SHARE: f32 = 0.4;
/// A card's inner margin, points.
const MARGIN: i8 = 8;

/// The ⋮ of a menu, painted (the fonts have no such glyph): three dots one above another.
fn dots(painter: &egui::Painter, rect: Rect, colour: Color32) {
    for k in [-1.0, 0.0, 1.0] {
        painter.circle_filled(rect.center() + vec2(0.0, 4.5 * k), 1.6, colour);
    }
}

/// What a card's ⋮ menu draws, when it has one.
pub type CardMenu<'a> = &'a dyn Fn(&mut Shell, &mut egui::Ui);

/// A card's ⋮ menu for the part of the scene at `path`: put it back as the scene was opened or
/// saved, copy it, paste what was copied of the same kind; each one edit.
pub fn part_menu(shell: &mut Shell, ui: &mut egui::Ui, path: &str) {
    let saved = cav_sim::paths::get(&shell.engine.baseline().to_value(), path).ok().cloned();
    let now = cav_sim::paths::get(&shell.engine.spec().to_value(), path).ok().cloned();
    let reset = ui.add_enabled(saved.is_some() && saved != now, egui::Button::new("Reset"));
    shell.layout.buttons.push((format!("reset card {path}"), reset.rect));
    if reset.clicked()
        && let Some(value) = saved
    {
        shell.apply(Command::set(path, value));
        ui.close();
    }
    if ui.add_enabled(now.is_some(), egui::Button::new("Copy")).clicked() {
        shell.clipboard = now.clone();
        ui.close();
    }
    let paste = shell.clipboard.clone().filter(|c| now.as_ref().is_some_and(|n| same_kind(n, c)));
    if ui.add_enabled(paste.is_some(), egui::Button::new("Paste")).clicked()
        && let Some(value) = paste
    {
        shell.apply(Command::set(path, value));
        ui.close();
    }
}

/// Whether two values are of one kind (the same keys at the top, or both not objects): what a
/// paste may put where.
fn same_kind(a: &Value, b: &Value) -> bool {
    match (a.as_object(), b.as_object()) {
        (Some(a), Some(b)) => a.keys().eq(b.keys()),
        (None, None) => true,
        _ => false,
    }
}

/// The width of the label column for a panel `width` points wide: 40 % of it, at least 96 pt.
#[must_use]
pub fn label_width(width: f32) -> f32 {
    (width * LABEL_SHARE).max(LABEL_MIN).min(width * 0.5)
}

/// A card that folds (`id` under the keyboard's `parent`, titled `title`), open at first when
/// `open`: a scope of the keyboard's tree while open, a button while folded (Enter opens it),
/// its header's rect in the layout's buttons as `fold <id>`; a ⋮ menu on its header when `menu`
/// gives one. `salt` keeps one card's fold apart for each kind of selection.
#[allow(clippy::too_many_arguments)]
pub fn card(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    parent: &str,
    id: &str,
    title: &str,
    salt: &str,
    open: bool,
    menu: Option<CardMenu<'_>>,
    contents: impl FnOnce(&mut Shell, &mut egui::Ui),
) {
    let opened = shell.take_click(id) || shell.unfold.iter().any(|u| u == id);
    shell.building.add(parent, id, title, Kind::Scope);
    let state_id = egui::Id::unique(("card", id.to_owned(), salt.to_owned()));
    let mut state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), state_id, open);
    if opened {
        state.set_open(true);
    }
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, theme::WIDGET_EDGE.gamma_multiply(0.6)))
        .corner_radius(5.0)
        .inner_margin(egui::Margin { left: MARGIN, right: MARGIN, top: 2, bottom: 4 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let width = ui.available_width();
            let (rect, response) = ui.allocate_exact_size(vec2(width, 26.0), Sense::click());
            let painter = ui.painter();
            let tri = Rect::from_center_size(pos2(rect.left() + 5.0, rect.center().y), vec2(8.0, 8.0));
            let points = if state.is_open() {
                vec![tri.left_top(), tri.right_top(), tri.center_bottom()]
            } else {
                vec![tri.left_top(), tri.right_center(), tri.left_bottom()]
            };
            painter.add(egui::Shape::convex_polygon(points, theme::WEAK, Stroke::NONE));
            painter.text(
                pos2(rect.left() + 16.0, rect.center().y),
                Align2::LEFT_CENTER,
                title,
                FontId::proportional(13.0),
                theme::STRONG,
            );
            let mut on_menu = false;
            if let Some(menu) = menu {
                let at = Rect::from_center_size(pos2(rect.right() - 10.0, rect.center().y), vec2(22.0, 22.0));
                let button = ui.interact(at, egui::Id::unique(("card menu", id.to_owned())), Sense::click());
                dots(ui.painter(), at, if button.hovered() { theme::STRONG } else { theme::WEAK });
                let menu_id = format!("{id}/menu");
                shell.register(id, &menu_id, &format!("{title}: menu"), Kind::Button, button.rect);
                shell.layout.buttons.push((format!("menu {id}"), button.rect));
                on_menu = button.hovered();
                let open_menu =
                    (button.clicked() || shell.take_click(&menu_id)).then_some(egui::SetOpenCommand::Toggle);
                egui::Popup::menu(&button).open_memory(open_menu).show(|ui| menu(shell, ui));
            }
            if response.clicked() && !on_menu {
                state.toggle(ui);
            }
            shell.building.place(id, rect);
            shell.layout.buttons.push((format!("fold {id}"), rect));
            if state.is_open() {
                ui.painter().hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, theme::RULE));
                ui.add_space(3.0);
                contents(shell, ui);
            }
        });
    if !state.is_open() {
        shell.building.add(parent, id, title, Kind::Button);
    }
    state.store(ui.ctx());
    ui.add_space(6.0);
}

/// Read-only rows of a card: a muted name in the label column, the value after it.
pub fn info_rows(shell: &Shell, ui: &mut egui::Ui, rows: &[(&str, String)]) {
    let width = shell.label_width;
    for (name, value) in rows {
        ui.horizontal(|ui| {
            theme::fixed_left(
                ui,
                vec2(width, 20.0),
                egui::Label::new(RichText::new(*name).size(12.0).color(theme::WEAK)).truncate(),
            );
            ui.add(egui::Label::new(RichText::new(value).size(12.0).color(theme::TEXT)).wrap());
        });
    }
}

/// What the selection is, in words for its header: its kind, and for a body its shape and motion
/// ("Sphere · static").
fn kind_words(spec: &cav_sim::SceneSpec, target: Option<Target>) -> String {
    match target {
        None => "Scene".to_owned(),
        Some(Target::Tower(_)) => "Tower".to_owned(),
        Some(Target::Room) => "Room".to_owned(),
        Some(Target::Floor) => "Floor".to_owned(),
        Some(Target::World) => "World".to_owned(),
        Some(Target::Body(n)) => {
            let body = &spec.bodies[n - 1];
            let shape = body.shape.as_ref().map_or_else(
                || if body.radius > 0.0 { "Sphere".to_owned() } else { "Point mass".to_owned() },
                |s| cav_sim::paths::words(s.kind.name()),
            );
            format!("{shape} · {}", body.motion.kind().to_lowercase())
        }
    }
}

/// The selection's path in the scene, and the label path its name is edited at.
fn paths_of(target: Option<Target>) -> (String, Option<String>) {
    match target {
        None => (String::new(), Some("name".to_owned())),
        Some(Target::Tower(n)) => {
            (format!("array.towers[{}]", n - 1), Some(format!("array.towers[{}].label", n - 1)))
        }
        Some(Target::Body(n)) => (format!("bodies[{}]", n - 1), Some(format!("bodies[{}].label", n - 1))),
        Some(Target::Room) => ("room".to_owned(), None),
        Some(Target::Floor) => ("floor".to_owned(), None),
        Some(Target::World) => ("world".to_owned(), None),
    }
}

/// The header: the selection's name (typed in place), what it is, an eye for a body, and the ⋮
/// menu of what can be done to the whole of it.
pub fn header(shell: &mut Shell, ui: &mut egui::Ui) {
    let spec = shell.engine.spec().clone();
    let target = shell.engine.selection();
    let (path, label_path) = paths_of(target);
    let name = label_path.as_deref().and_then(|p| cav_sim::paths::get(&spec.to_value(), p).ok().cloned());
    let name = name.and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_else(|| kind_words(&spec, target));
    shell.building.add("inspector", "inspector/header", "Header", Kind::Scope);
    let top = ui.cursor().min;
    ui.horizontal(|ui| {
        ui.set_min_height(30.0);
        let (icon, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
        ui.painter().circle_filled(icon.center(), 5.0, theme::GOLD);
        let mut text = match &shell.typing {
            Some((typing, buffer)) if Some(typing) == label_path.as_ref() => buffer.clone(),
            _ => name.clone(),
        };
        let field = ui.add_enabled(
            label_path.is_some(),
            egui::TextEdit::singleline(&mut text)
                .font(FontId::proportional(14.0))
                .desired_width(ui.available_width() * 0.5),
        );
        // The name's field is the label's editor (its id and its place in the layout are the
        // field's: `field/bodies[0].label`), drawn in the header rather than in a card.
        let name_id =
            label_path.as_deref().map_or_else(|| "inspector/name".to_owned(), crate::editors::field_id);
        shell.register("inspector/header", &name_id, "Name", Kind::Text, field.rect);
        if let Some(path) = &label_path {
            shell.layout.editors.push((path.clone(), field.rect));
        }
        if shell.take_type(&name_id) {
            field.request_focus();
        }
        if let Some(label_path) = &label_path {
            if field.has_focus() {
                shell.typing = Some((label_path.clone(), text.clone()));
            }
            if field.lost_focus() {
                shell.typing = None;
                if text.trim() != name && !text.trim().is_empty() {
                    shell.apply(Command::set(label_path, Value::String(text.trim().to_owned())));
                }
            }
        }
        ui.label(RichText::new(kind_words(&spec, target)).size(12.0).color(theme::WEAK));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (at, menu) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
            dots(ui.painter(), at, if menu.hovered() { theme::STRONG } else { theme::TEXT });
            let menu = menu.on_hover_text("what can be done to it");
            shell.register("inspector/header", "inspector/menu", "Menu", Kind::Button, menu.rect);
            shell.layout.buttons.push(("inspector menu".to_owned(), menu.rect));
            let open = (menu.clicked() || shell.take_click("inspector/menu"))
                .then_some(egui::SetOpenCommand::Toggle);
            egui::Popup::menu(&menu).open_memory(open).show(|ui| header_menu(shell, ui, target, &path));
            if let Some(Target::Body(n)) = target {
                let label = spec.bodies[n - 1].label.clone();
                let hidden = shell.engine.view(0).hidden.contains(&label);
                let (eye, eye_hit) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                crate::icons::eye(ui.painter(), eye, !hidden, if hidden { theme::WEAK } else { theme::TEXT });
                shell.register("inspector/header", "inspector/eye", "Show or hide", Kind::Button, eye);
                if eye_hit.clicked() || shell.take_click("inspector/eye") {
                    let word = if hidden { "unhide" } else { "hide" };
                    let _ = shell.run(&format!("{word} \"{label}\""));
                }
            }
        });
    });
    let rect = Rect::from_min_max(top, pos2(ui.max_rect().right(), ui.cursor().top()));
    shell.building.place("inspector/header", rect);
    ui.add_space(4.0);
}

/// The header's ⋮ menu: duplicate, delete, reset all, copy, paste, for what can have them.
fn header_menu(shell: &mut Shell, ui: &mut egui::Ui, target: Option<Target>, path: &str) {
    let item = |shell: &mut Shell, ui: &mut egui::Ui, label: &str, line: Option<String>| {
        let enabled = line.is_some();
        let response = ui.add_enabled(enabled, egui::Button::new(label));
        if let Some(line) = line {
            shell.layout.buttons.push((line.clone(), response.rect));
            if response.clicked() {
                let _ = shell.run(&line);
                ui.close();
            }
        }
    };
    let list_item = matches!(target, Some(Target::Body(_) | Target::Tower(_)));
    let removable = list_item || matches!(target, Some(Target::Room | Target::Floor));
    item(shell, ui, "Duplicate", list_item.then(|| format!("duplicate {path}")));
    item(
        shell,
        ui,
        "Delete",
        removable.then(|| if list_item { format!("remove {path}") } else { format!("set {path} null") }),
    );
    ui.separator();
    // Reset all: the whole of it as the scene was opened or saved, one edit.
    let saved = (!path.is_empty())
        .then(|| cav_sim::paths::get(&shell.engine.baseline().to_value(), path).ok().cloned())
        .flatten();
    item(shell, ui, "Reset all", saved.map(|v| format!("set {path} {v}")));
    let now = (!path.is_empty())
        .then(|| cav_sim::paths::get(&shell.engine.spec().to_value(), path).ok().cloned())
        .flatten();
    let copy = ui.add_enabled(now.is_some(), egui::Button::new("Copy"));
    shell.layout.buttons.push(("copy".to_owned(), copy.rect));
    if copy.clicked() {
        shell.clipboard = now.clone();
        ui.close();
    }
    // Paste: what was copied, its label kept (a label is the selection's own).
    let pasted = shell.clipboard.clone().filter(|_| !path.is_empty()).map(|mut value| {
        if let (Some(old), Some(map)) =
            (now.as_ref().and_then(|n| n.get("label")).cloned(), value.as_object_mut())
        {
            map.insert("label".to_owned(), old);
        }
        format!("set {path} {value}")
    });
    item(shell, ui, "Paste", pasted);
}

/// The Info card: what is worked out from the selection, read only.
pub fn info(shell: &mut Shell, ui: &mut egui::Ui) {
    let spec = shell.engine.spec().clone();
    let gravity = spec.array.instrument.gravity;
    let rows: Vec<(&str, String)> = match shell.engine.selection() {
        Some(Target::Body(n)) => {
            let body = &spec.bodies[n - 1];
            let [x, y, z] = body.motion.position(shell.engine.time(), gravity);
            let mut rows = vec![("Now at", format!("{x:.3}, {y:.3}, {z:.3} m"))];
            if let Some(tree) = shell.engine.body_tree(n - 1) {
                let volume =
                    tree.samples as f64 * (tree.grid.pitch / f64::from(tree.grid.supersample)).powi(3);
                rows.push(("Mass", format!("{} kg", crate::editors::shown(tree.body().mass))));
                rows.push(("Volume", format!("{} m³", crate::editors::shown(volume))));
                rows.push(("Cells", format!("{} leaves", tree.leaves.len())));
            }
            rows
        }
        Some(Target::Tower(n)) => {
            let reply = shell.reading().clone();
            match reply.towers.get(n - 1) {
                Some(r) => vec![
                    ("ΔΦ", crate::shell::phase_text(r.gradiometer)),
                    ("δφ₁ lower", crate::shell::phase_text(r.lower)),
                    ("δφ₂ upper", crate::shell::phase_text(r.upper)),
                    ("Quadrature", cav_engine::estimate_text(reply.estimate, reply.settled, reply.truncated)),
                    ("Read at", format!("{:.2} s", reply.time)),
                ],
                None => vec![("Reading", "computing".to_owned())],
            }
        }
        None => crate::panels::scene_rows(shell),
        _ => Vec::new(),
    };
    if rows.is_empty() {
        return;
    }
    card(shell, ui, "inspector", "inspector/info", "Info", "info", false, None, |shell, ui| {
        info_rows(shell, ui, &rows)
    });
}
