//! The inspector as component cards (S19, `docs/design/panels.md`): a header for the selection
//! (the tree's icon for what it is, its editable name, its type, an eye, a ⋮ menu with
//! duplicate, delete, reset all, copy and paste), then its cards, every one with a ⋮ and on one
//! grid whose value column starts at the same x: a body's in the design's order ([`body`]:
//! Transform, Shape, Material, Parts, Motion, Lifetime), the scene's (Scene, Array, Cadence,
//! Instrument), a tower's; and last an Info card of what is worked out and not edited. No help
//! paragraphs: what a field is lives on its name's hover. Each control is a console line or a
//! `Command`, through the same path as before. Every target is [`crate::theme::HIT`] a side.

use cav_engine::{Command, Target};
use cav_sim::paths::Entry;
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
                let at = Rect::from_center_size(
                    pos2(rect.right() - 12.0, rect.center().y),
                    vec2(theme::HIT, theme::HIT),
                );
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
                vec2(width, theme::HIT),
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
        // The tree's own icon for what it is (S19's review, minor 18: a gold dot for everything).
        let (icon, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
        let kind = match target {
            None => crate::outliner::RowKind::Scene,
            Some(Target::Tower(_)) => crate::outliner::RowKind::Tower,
            Some(Target::Body(n)) => spec
                .bodies
                .get(n - 1)
                .map_or(crate::outliner::RowKind::Point, |b| crate::outliner::body_kind(b).0),
            Some(Target::Room | Target::Floor | Target::World) => crate::outliner::RowKind::Surroundings,
        };
        crate::outliner::icon(ui.painter(), icon, kind, theme::GOLD);
        let mut text = match &shell.typing {
            Some((typing, buffer)) if Some(typing) == label_path.as_ref() => buffer.clone(),
            _ => name.clone(),
        };
        let field = ui.add_enabled(
            label_path.is_some(),
            egui::TextEdit::singleline(&mut text)
                .font(FontId::proportional(14.0))
                .min_size(vec2(0.0, theme::HIT))
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
            let (at, menu) = ui.allocate_exact_size(vec2(theme::HIT, theme::HIT), Sense::click());
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
                let (eye, eye_hit) = ui.allocate_exact_size(vec2(theme::HIT, theme::HIT), Sense::click());
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
            let tree = shell.engine.body_tree(n - 1);
            // What the body weighs, by the one rule (`BodySpec::mass_with`): its materials', or
            // its `mass`.
            if let Some(kg) = body.mass_with(tree.map(std::sync::Arc::as_ref)) {
                rows.push(("Mass", format!("{} kg", crate::editors::shown(kg))));
            }
            if let Some(tree) = tree {
                let volume =
                    tree.samples as f64 * (tree.grid.pitch / f64::from(tree.grid.supersample)).powi(3);
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
    // Its menu copies what it says, as text (a ⋮ on every card: S19's review, I5).
    let text: String = rows.iter().map(|(name, value)| format!("{name}: {value}\n")).collect();
    let menu = |_: &mut Shell, ui: &mut egui::Ui| {
        if ui.button("Copy as text").clicked() {
            ui.ctx().copy_text(text.clone());
            ui.close();
        }
    };
    card(shell, ui, "inspector", "inspector/info", "Info", "info", false, Some(&menu), |shell, ui| {
        info_rows(shell, ui, &rows)
    });
}

/// A card's ⋮ menu over several fields of one part of the scene (`parent`, and the `fields`
/// below it, by their paths from it): Reset puts them back as the scene was opened or saved,
/// Copy takes them, Paste puts what was copied of the same fields; each one edit (one `Set` of
/// `parent`). What the Lifetime card and the Scene card use.
pub fn fields_menu(shell: &mut Shell, ui: &mut egui::Ui, parent: &str, fields: &[&str]) {
    let get = |root: &Value, at: &str| cav_sim::paths::get(root, &join(parent, at)).ok().cloned();
    let (now, saved) = (shell.engine.spec().to_value(), shell.engine.baseline().to_value());
    let parent_now = cav_sim::paths::get(&now, parent).ok().cloned();
    let with = |values: &[(String, Value)]| -> Option<Value> {
        let mut whole = parent_now.clone()?;
        for (field, value) in values {
            cav_sim::paths::set(&mut whole, field, value.clone()).ok()?;
        }
        Some(whole)
    };
    let saved_values: Option<Vec<(String, Value)>> =
        fields.iter().map(|f| get(&saved, f).map(|v| ((*f).to_owned(), v))).collect();
    let differs = fields.iter().any(|f| get(&saved, f) != get(&now, f));
    let reset = saved_values.filter(|_| differs).and_then(|v| with(&v));
    let button = ui.add_enabled(reset.is_some(), egui::Button::new("Reset"));
    shell.layout.buttons.push((format!("reset card {parent}"), button.rect));
    if button.clicked()
        && let Some(value) = reset
    {
        shell.apply(Command::set(parent, value));
        ui.close();
    }
    let copied: serde_json::Map<String, Value> =
        fields.iter().filter_map(|f| get(&now, f).map(|v| ((*f).to_owned(), v))).collect();
    if ui.button("Copy").clicked() {
        shell.clipboard = Some(Value::Object(copied));
        ui.close();
    }
    let paste = shell.clipboard.clone().and_then(|c| {
        let map = c.as_object()?.clone();
        (map.len() == fields.len() && fields.iter().all(|f| map.contains_key(*f)))
            .then(|| with(&map.into_iter().collect::<Vec<_>>()))
            .flatten()
    });
    if ui.add_enabled(paste.is_some(), egui::Button::new("Paste")).clicked()
        && let Some(value) = paste
    {
        shell.apply(Command::set(parent, value));
        ui.close();
    }
}

/// `parent.field`, or `field` at the top.
fn join(parent: &str, field: &str) -> String {
    if parent.is_empty() { field.to_owned() } else { format!("{parent}.{field}") }
}

/// The entries of `entries` at `path` or below it.
fn under(entries: &[Entry], path: &str) -> Vec<Entry> {
    entries
        .iter()
        .filter(|e| {
            e.path == path
                || e.path.starts_with(&format!("{path}."))
                || e.path.starts_with(&format!("{path}["))
        })
        .cloned()
        .collect()
}

/// A body's cards, in the order `docs/design/panels.md` draws them (S19's review, I5): Transform
/// (where it is), Shape (its kind: a point mass or a shape, and the shape), Material (what it is
/// made of and what that makes it weigh; Mass for a point), Parts (a body of materials: each part
/// under a header of its own), Motion (its kind and its numbers), Lifetime (when it is there).
/// Info comes after, from [`info`]. Every card has a ⋮.
pub fn body(shell: &mut Shell, ui: &mut egui::Ui, n: usize) {
    let spec = shell.engine.spec().clone();
    let Some(body) = spec.bodies.get(n) else { return };
    let at = format!("bodies[{n}]");
    let entries = match cav_sim::paths::leaves(&spec, &at, crate::editors::MAX_FIELDS) {
        Ok(entries) => entries,
        Err(why) => {
            ui.label(RichText::new(why).size(11.0).color(theme::WARN));
            return;
        }
    };
    let single = |path: &str| entries.iter().filter(|e| e.path == path).cloned().collect::<Vec<_>>();

    // Transform: the place its motion is anchored at (a static body's position).
    let anchor = format!("{at}.motion.{}", body.motion.anchor().1);
    let anchor_parent = anchor.rsplit_once('.').map_or(at.clone(), |(p, _)| p.to_owned());
    let transform = single(&anchor);
    let menu = |shell: &mut Shell, ui: &mut egui::Ui| part_menu(shell, ui, &anchor);
    card(
        shell,
        ui,
        "inspector",
        "inspector/transform",
        "Transform",
        "body",
        true,
        Some(&menu),
        |shell, ui| {
            crate::editors::draw_rows(shell, ui, "inspector/transform", &anchor_parent, &transform);
        },
    );

    // Shape: its kind, then the shape's fields (a point's radius).
    let shape_path = format!("{at}.shape");
    let shape_fields: Vec<Entry> = under(&entries, &shape_path)
        .into_iter()
        .filter(|e| {
            e.path != shape_path
                && !e.path.starts_with(&format!("{shape_path}.material"))
                && !e.path.starts_with(&format!("{shape_path}.parts"))
        })
        .map(|mut e| {
            // The card's first row is the body's Kind; the shape's own is the solid it is.
            if e.path == format!("{shape_path}.kind") {
                e.title = Some("Solid".to_owned());
            }
            e
        })
        .collect();
    let radius = single(&format!("{at}.radius"));
    let menu_path = if body.shape.is_some() { shape_path.clone() } else { format!("{at}.radius") };
    let menu = |shell: &mut Shell, ui: &mut egui::Ui| part_menu(shell, ui, &menu_path);
    card(shell, ui, "inspector", "inspector/shape", "Shape", "body", true, Some(&menu), |shell, ui| {
        kind_row(shell, ui, n);
        if body.shape.is_some() {
            crate::editors::draw_rows(shell, ui, "inspector/shape", &shape_path, &shape_fields);
        } else {
            // A point's radius (a sphere's), which a voxel body has no use for (minor 21).
            crate::editors::draw_rows(shell, ui, "inspector/shape", &at, &radius);
        }
    });

    // Material: what it is made of, and what it weighs.
    // (Each drawn under its own parent: drawn from the body, the material's row would put the
    // shape's own box before it, the "Shape" tick the review asked to be rid of.)
    let material = single(&format!("{shape_path}.material"));
    let mass = single(&format!("{at}.mass"));
    let voxels = body.shape.is_some();
    let title = if voxels { "Material" } else { "Mass" };
    let fields: &[&str] = if voxels { &["shape.material", "mass"] } else { &["mass"] };
    let menu = |shell: &mut Shell, ui: &mut egui::Ui| fields_menu(shell, ui, &at, fields);
    card(shell, ui, "inspector", "inspector/material", title, "body", true, Some(&menu), |shell, ui| {
        crate::editors::draw_rows(shell, ui, "inspector/material", &shape_path, &material);
        crate::editors::draw_rows(shell, ui, "inspector/material", &at, &mass);
    });

    // Parts: a body of materials only.
    if body.shape.as_ref().is_some_and(|s| s.material.is_some()) {
        let parts_path = format!("{shape_path}.parts");
        let menu = |shell: &mut Shell, ui: &mut egui::Ui| part_menu(shell, ui, &parts_path);
        let count = body.shape.as_ref().map_or(0, |s| s.parts.len());
        let title = if count == 0 { "Parts".to_owned() } else { format!("Parts · {count}") };
        card(shell, ui, "inspector", "inspector/parts", &title, "body", true, Some(&menu), |shell, ui| {
            parts(shell, ui, n, &entries);
        });
    }

    // Motion: its kind, then its numbers (its anchor is the Transform's).
    let motion_path = format!("{at}.motion");
    let motion: Vec<Entry> = under(&entries, &motion_path)
        .into_iter()
        .filter(|e| e.path != anchor)
        .map(|mut e| {
            if e.path == motion_path {
                e.title = Some("Kind".to_owned());
            }
            e
        })
        .collect();
    let menu = |shell: &mut Shell, ui: &mut egui::Ui| part_menu(shell, ui, &motion_path);
    card(shell, ui, "inspector", "inspector/motion", "Motion", "body", true, Some(&menu), |shell, ui| {
        crate::editors::draw_rows(shell, ui, "inspector/motion", &motion_path, &motion);
    });

    // Lifetime: when it comes and goes, each behind its box.
    let mut lifetime = single(&format!("{at}.appears"));
    lifetime.extend(single(&format!("{at}.vanishes")));
    let always = body.appears.is_none() && body.vanishes.is_none();
    let title = if always { "Lifetime · always there" } else { "Lifetime" };
    let menu = |shell: &mut Shell, ui: &mut egui::Ui| fields_menu(shell, ui, &at, &["appears", "vanishes"]);
    // Open at first, as every card is, so that each field is one look away (the design drew it
    // folded; S18's E6 asks every field to have its editor in sight).
    card(shell, ui, "inspector", "inspector/lifetime", title, "body", true, Some(&menu), |shell, ui| {
        crate::editors::draw_rows(shell, ui, "inspector/lifetime", &at, &lifetime);
    });
}

/// The Shape card's first row: whether the body is a point mass (a sphere, with a radius) or a
/// shape made of voxels, as an explicit choice (S19's review, UX: the "Shape" box that made a
/// point mass when unticked). Choosing is one edit of the whole body: a point keeps what the body
/// weighs ([`cav_sim::BodySpec::mass_in`]); a shape starts as the sphere starter's.
fn kind_row(shell: &mut Shell, ui: &mut egui::Ui, n: usize) {
    let spec = shell.engine.spec().clone();
    let Some(body) = spec.bodies.get(n) else { return };
    let choices = ["Point mass".to_owned(), "Shape (voxels)".to_owned()];
    let current = usize::from(body.shape.is_some());
    let item = |i: usize| egui::WidgetText::from(choices[i].clone());
    // The editor of whether it has a shape: `bodies[n].shape`'s own.
    let id = crate::editors::field_id(&format!("bodies[{n}].shape"));
    let hover = "a point mass (a uniform sphere when it has a radius), or a solid made of voxels";
    let Some(chosen) = crate::editors::choice_row(
        shell,
        ui,
        "inspector/shape",
        &id,
        "Kind",
        hover,
        &choices,
        current,
        &item,
    ) else {
        return;
    };
    let mut changed = body.clone();
    if chosen == 0 {
        changed.mass = body.mass_in(&spec).unwrap_or(body.mass);
        changed.shape = None;
        changed.radius = 0.0;
    } else {
        let shape =
            cav_sim::starters::find("sphere").and_then(|s| s.body.as_ref()).and_then(|b| b.shape.clone());
        changed.shape = shape;
        changed.radius = 0.0;
    }
    if let Ok(value) = serde_json::to_value(&changed) {
        shell.apply(Command::set(&format!("bodies[{n}]"), value));
    }
}

/// The Parts card's contents: each part under a header that says what it is ("Part 1 · Add lead
/// · Sphere") with a ⋮ to move it up or down or remove it (one edit each), its fields below; and
/// Add part at the foot.
fn parts(shell: &mut Shell, ui: &mut egui::Ui, n: usize, entries: &[Entry]) {
    let spec = shell.engine.spec().clone();
    let Some(shape) = spec.bodies.get(n).and_then(|b| b.shape.as_ref()) else { return };
    let list = format!("bodies[{n}].shape.parts");
    let own = shape.material.clone().unwrap_or_default();
    for (i, part) in shape.parts.iter().enumerate() {
        let path = format!("{list}[{i}]");
        let mode = format!("{:?}", part.mode);
        let what = part.material.clone().unwrap_or_else(|| own.clone());
        let solid = match &part.kind {
            cav_sim::shape::PartKind::Solid { shape } => cav_sim::paths::words(shape.name()),
            cav_sim::shape::PartKind::Stroke { .. } => "Stroke".to_owned(),
        };
        let title = if part.mode == cav_sim::shape::PartMode::Erase {
            format!("Part {} · Erase · {solid}", i + 1)
        } else {
            format!("Part {} · {mode} {what} · {solid}", i + 1)
        };
        let scope = format!("inspector/parts/{i}");
        shell.building.add("inspector/parts", &scope, &title, Kind::Scope);
        let header = ui.horizontal(|ui| {
            ui.set_min_height(theme::HIT);
            ui.label(RichText::new(&title).size(12.5).color(theme::STRONG));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // An id of its own, as the cards' ⋮ have, so that its menu stays open from frame to
                // frame whatever is laid out before it.
                let (at, _) = ui.allocate_exact_size(vec2(theme::HIT, theme::HIT), Sense::hover());
                let menu = ui.interact(at, egui::Id::unique(("part menu", n, i)), Sense::click());
                dots(ui.painter(), at, if menu.hovered() { theme::STRONG } else { theme::WEAK });
                let menu_id = format!("{scope}/menu");
                shell.register(&scope, &menu_id, &format!("{title}: menu"), Kind::Button, menu.rect);
                shell.layout.buttons.push((format!("menu {path}"), menu.rect));
                let open =
                    (menu.clicked() || shell.take_click(&menu_id)).then_some(egui::SetOpenCommand::Toggle);
                egui::Popup::menu(&menu).open_memory(open).show(|ui| part_item_menu(shell, ui, n, i));
            });
        });
        ui.painter().hline(
            header.response.rect.x_range(),
            header.response.rect.bottom(),
            Stroke::new(1.0, theme::RULE),
        );
        let fields = under(entries, &path);
        crate::editors::draw_rows(shell, ui, &scope, &path, &fields);
        let rect = Rect::from_min_max(
            header.response.rect.min,
            pos2(header.response.rect.right(), ui.cursor().top()),
        );
        shell.building.place(&scope, rect);
        ui.add_space(4.0);
    }
    let add = ui.add_sized(vec2(ui.available_width(), theme::HIT), egui::Button::new("+ Add part"));
    shell.register("inspector/parts", "inspector/parts/add", "Add part", Kind::Button, add.rect);
    shell.layout.buttons.push((format!("add part {list}"), add.rect));
    // The list's own editor is this button (each part's fields are theirs).
    shell.layout.editors.push((list.clone(), add.rect));
    // A copy of the last part (as a list's + adds), or for the first a small ball of the body's
    // own material.
    let next = || {
        shape
            .parts
            .last()
            .and_then(|p| serde_json::to_value(p).ok())
            .or_else(|| crate::editors::starter(&spec, &format!("{list}[]"), None))
    };
    if (add.clicked() || shell.take_click("inspector/parts/add"))
        && let Some(value) = next()
    {
        shell.apply(Command::Insert { path: list, index: None, value });
    }
}

/// A part's ⋮: move it up, move it down, remove it; each one edit of the list.
fn part_item_menu(shell: &mut Shell, ui: &mut egui::Ui, n: usize, i: usize) {
    let spec = shell.engine.spec().clone();
    let Some(parts) = spec.bodies.get(n).and_then(|b| b.shape.as_ref()).map(|s| s.parts.clone()) else {
        return;
    };
    let list = format!("bodies[{n}].shape.parts");
    let moved = |to: usize| -> Option<Value> {
        let mut order = parts.clone();
        let part = order.remove(i);
        order.insert(to, part);
        serde_json::to_value(order).ok()
    };
    let up = ui.add_enabled(i > 0, egui::Button::new("Move up"));
    shell.layout.buttons.push((format!("move up {list}[{i}]"), up.rect));
    if up.clicked()
        && let Some(value) = moved(i - 1)
    {
        shell.apply(Command::set(&list, value));
        ui.close();
    }
    let down = ui.add_enabled(i + 1 < parts.len(), egui::Button::new("Move down"));
    shell.layout.buttons.push((format!("move down {list}[{i}]"), down.rect));
    if down.clicked()
        && let Some(value) = moved(i + 1)
    {
        shell.apply(Command::set(&list, value));
        ui.close();
    }
    let remove = ui.button("Remove");
    shell.layout.buttons.push((format!("remove {list}[{i}]"), remove.rect));
    if remove.clicked() {
        shell.apply(Command::Remove { path: format!("{list}[{i}]") });
        ui.close();
    }
}
