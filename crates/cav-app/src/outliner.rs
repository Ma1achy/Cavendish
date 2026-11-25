//! The scene tree and the readings (S19, `docs/design/panels.md`): the outliner as one tree that
//! names things (a type icon a row, groups that fold, an eye and a lock, a status dot a tower,
//! no numbers), and the Readings tab where the numbers are (a table of each tower's phases and
//! state). Every row and button is registered in the keyboard's tree under the same scopes as
//! before (`tree/array`, `tree/sources`, `tree/surroundings`), so the Tab order is unchanged.

use cav_engine::{Command, Target};
use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, pos2, vec2};

use crate::Shell;
use crate::keyboard::Kind;
use crate::theme;

/// Which tab the right-hand panel shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RightTab {
    /// The selection's properties.
    #[default]
    Inspector,
    /// Every tower's reading.
    Readings,
}

/// What a row is, for its icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// The scene itself, the tree's root.
    Scene,
    /// A group of rows (the array, the bodies, the surroundings).
    Group,
    /// A gradiometer tower.
    Tower,
    /// A point mass or a uniform sphere.
    Point,
    /// A body made of voxels.
    Voxels,
    /// A body made of an imported mesh.
    Mesh,
    /// The room, the floor or the world.
    Surroundings,
}

/// What a tower's status dot says of its reading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dot {
    /// Read to its tolerance, of everything there is.
    Settled,
    /// Of part of the world's matter only (the read's radius cut it).
    Partial,
    /// Being computed, or out of date.
    Stale,
    /// Did not settle to its tolerance.
    Unsettled,
    /// Nothing to read.
    Silent,
}

impl Dot {
    /// Its colour.
    #[must_use]
    pub const fn colour(self) -> Color32 {
        match self {
            Self::Settled => Color32::from_rgb(0x5f, 0xb8, 0x6a),
            Self::Partial => Color32::from_rgb(0x5a, 0x9b, 0xd5),
            Self::Stale => theme::WARN,
            Self::Unsettled => theme::ERROR,
            Self::Silent => theme::WIDGET_EDGE,
        }
    }

    /// What it means, for its tooltip and the Readings tab.
    #[must_use]
    pub const fn words(self) -> &'static str {
        match self {
            Self::Settled => "settled",
            Self::Partial => "of part of the world only",
            Self::Stale => "computing",
            Self::Unsettled => "not settled to its tolerance",
            Self::Silent => "nothing to read",
        }
    }
}

/// The dot of tower `n` (from 0) from the engine's reading.
#[must_use]
pub fn dot_of(reply: &cav_engine::ReadingReply, n: usize, silent: bool) -> Dot {
    if silent {
        Dot::Silent
    } else if reply.state == cav_engine::Standing::Stale || reply.towers.get(n).is_none() {
        Dot::Stale
    } else if reply.truncated.is_some() {
        Dot::Partial
    } else if !reply.settled {
        Dot::Unsettled
    } else {
        Dot::Settled
    }
}

/// A row's icon, in a square `rect`.
pub(crate) fn icon(painter: &egui::Painter, rect: Rect, kind: RowKind, colour: Color32) {
    let c = rect.center();
    let r = rect.width() * 0.32;
    let stroke = Stroke::new(1.4, colour);
    match kind {
        RowKind::Scene => {
            painter.add(egui::Shape::convex_polygon(
                vec![c + vec2(0.0, -r), c + vec2(r, 0.0), c + vec2(0.0, r), c + vec2(-r, 0.0)],
                Color32::TRANSPARENT,
                stroke,
            ));
            painter.circle_filled(c, 1.6, colour);
        }
        RowKind::Group => {
            painter.rect_stroke(
                Rect::from_center_size(c, vec2(2.0 * r, 1.6 * r)),
                1.0,
                stroke,
                egui::StrokeKind::Middle,
            );
        }
        RowKind::Tower => {
            painter.line_segment([c + vec2(0.0, -r - 1.0), c + vec2(0.0, r + 1.0)], Stroke::new(2.0, colour));
            painter.circle_filled(c + vec2(0.0, -r * 0.5), 1.8, colour);
            painter.circle_filled(c + vec2(0.0, r * 0.5), 1.8, colour);
        }
        RowKind::Point => {
            painter.circle_filled(c, r * 0.75, colour);
        }
        RowKind::Voxels => {
            let h = r * 0.55;
            for (dx, dy) in [(-h, -h), (h, -h), (-h, h), (h, h)] {
                painter.rect_filled(
                    Rect::from_center_size(c + vec2(dx, dy), vec2(h * 1.7, h * 1.7)),
                    0.5,
                    colour,
                );
            }
        }
        RowKind::Mesh => {
            painter.add(egui::Shape::convex_polygon(
                vec![c + vec2(0.0, -r), c + vec2(r, r * 0.8), c + vec2(-r, r * 0.8)],
                Color32::TRANSPARENT,
                stroke,
            ));
        }
        RowKind::Surroundings => {
            painter.line_segment([c + vec2(-r, r * 0.6), c + vec2(r, r * 0.6)], stroke);
            painter.add(egui::Shape::line(
                vec![
                    c + vec2(-r, r * 0.6),
                    c + vec2(-r * 0.3, -r * 0.4),
                    c + vec2(r * 0.2, r * 0.2),
                    c + vec2(r, -r * 0.6),
                ],
                stroke,
            ));
        }
    }
}

/// The kind of body `body` is, for its icon and its row's words.
pub(crate) fn body_kind(body: &cav_sim::BodySpec) -> (RowKind, String) {
    match &body.shape {
        None => (RowKind::Point, if body.radius > 0.0 { "sphere" } else { "point mass" }.to_owned()),
        Some(shape) => match shape.kind {
            cav_sim::ShapeKind::Mesh { .. } | cav_sim::ShapeKind::Triangles { .. } => {
                (RowKind::Mesh, "mesh".to_owned())
            }
            ref kind => (RowKind::Voxels, cav_sim::paths::words(kind.name()).to_lowercase()),
        },
    }
}

/// How far a row's depth indents it, points.
const INDENT: f32 = 14.0;
/// A row's height, points (the design's least hit target).
pub const ROW: f32 = 24.0;

#[allow(clippy::too_many_arguments)]
/// One row: a fold triangle for a group, an icon, the label; on its right what is given
/// (`trailing`). Registers it as `id` under `scope`, and records it in the layout. Returns its
/// response and the space on its right, for buttons.
fn row(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    scope: &str,
    id: &str,
    depth: usize,
    kind: RowKind,
    label: &str,
    selected: bool,
) -> (egui::Response, Rect) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
    // A group's row is a scope of the keyboard's tree while it is open (Enter goes in), a button
    // while it is folded (Enter opens it), as the inspector's folding sections are.
    let focus = if kind == RowKind::Group && shell.tree_open(id) { Kind::Scope } else { Kind::Button };
    shell.register(scope, id, label, focus, rect);
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 2.0, Color32::from_rgb(0x10, 0x30, 0x3d));
    } else if response.hovered() {
        painter.rect_filled(rect, 2.0, Color32::from_rgb(0x24, 0x24, 0x24));
    }
    let left = rect.left() + 4.0 + INDENT * depth as f32;
    // Indent guides: a thin line for each level above.
    for d in 1..=depth {
        let x = rect.left() + 4.0 + INDENT * d as f32 - INDENT * 0.5;
        painter.vline(x, rect.y_range(), Stroke::new(1.0, Color32::from_rgb(0x30, 0x30, 0x30)));
    }
    let colour = if selected { theme::STRONG } else { theme::TEXT };
    icon(painter, Rect::from_center_size(pos2(left + 18.0, rect.center().y), vec2(14.0, 14.0)), kind, colour);
    let name_at = pos2(left + 30.0, rect.center().y);
    let trailing = Rect::from_min_max(pos2(rect.right() - 64.0, rect.top()), rect.max);
    if !shell.renaming.as_ref().is_some_and(|(t, _)| row_target(id) == Some(*t)) {
        painter.text(name_at, Align2::LEFT_CENTER, label, FontId::proportional(13.0), colour);
        shell.layout.row_texts.push((id.to_owned(), label.to_owned()));
    }
    (response, trailing)
}

/// The target a row's id names, if it is an item's row.
fn row_target(id: &str) -> Option<Target> {
    let words = id.strip_prefix("tree/row/")?;
    let mut parts = words.split_whitespace();
    let n = |w: Option<&str>| w.and_then(|w| w.parse::<usize>().ok());
    match parts.next()? {
        "body" => n(parts.next()).map(Target::Body),
        "tower" => n(parts.next()).map(Target::Tower),
        _ => None,
    }
}

/// A group's header: its fold triangle, icon and name, and a `+` that adds into it. Returns
/// whether it is open. Its fold is remembered for each scene (egui's memory: UI state).
fn group(shell: &mut Shell, ui: &mut egui::Ui, scope: &str, name: &str, add: Option<&str>) -> bool {
    let mut open = shell.tree_open(scope);
    // Under the scene, one level in (S19's review, minor 19: the groups sat beside the root).
    let (response, trailing) = row(shell, ui, "tree", scope, 1, RowKind::Group, name, false);
    let painter = ui.painter();
    let tri = Rect::from_center_size(
        pos2(response.rect.left() + 9.0 + INDENT, response.rect.center().y),
        vec2(8.0, 8.0),
    );
    let points = if open {
        vec![tri.left_top(), tri.right_top(), tri.center_bottom()]
    } else {
        vec![tri.left_top(), tri.right_center(), tri.left_bottom()]
    };
    painter.add(egui::Shape::convex_polygon(points, theme::WEAK, Stroke::NONE));
    let mut add_rect = None;
    if let Some(line) = add {
        let plus = Rect::from_center_size(
            pos2(trailing.right() - 12.0, trailing.center().y),
            vec2(theme::HIT, theme::HIT),
        );
        let hit = ui.interact(plus, egui::Id::unique(("group add", scope)), Sense::click());
        if hit.hovered() || response.hovered() {
            painter.text(plus.center(), Align2::CENTER_CENTER, "+", FontId::proportional(15.0), theme::TEXT);
        }
        let id = format!("{scope}/add");
        shell.register(scope, &id, &format!("{name}: add"), Kind::Button, plus);
        shell.layout.buttons.push((format!("tree add {scope}"), plus));
        if hit.clicked() || shell.take_click(&id) {
            let _ = shell.run(line);
        }
        add_rect = Some(hit.rect);
    }
    let on_plus =
        add_rect.is_some_and(|r| ui.input(|i| i.pointer.interact_pos().is_some_and(|p| r.contains(p))));
    if (response.clicked() && !on_plus) || shell.take_click(scope) {
        open = !open;
        let key = (shell.engine.spec().name.clone(), scope.to_owned());
        if open {
            shell.tree_folded.remove(&key);
        } else {
            shell.tree_folded.insert(key);
        }
    }
    shell.layout.tree_groups.push((scope.to_owned(), open));
    open
}

/// An item's row: selects on a click, renames on a double click, the right-click menu, an eye
/// (bodies) or a status dot (towers) on its right, a lock for bodies.
#[allow(clippy::too_many_arguments)]
fn item(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    scope: &str,
    target: Target,
    kind: RowKind,
    label: &str,
    words: &str,
    dot: Option<Dot>,
) {
    let selected = shell.engine.selection() == Some(target);
    let id = crate::panels::row_id(target);
    let (response, trailing) = row(shell, ui, scope, &id, 2, kind, label, selected);
    // What it is ("point mass", "tower") is the row's tooltip, not painted on it.
    let response = response.on_hover_text(words);
    shell.layout.outline.push((target, response.rect));
    // (The last text painted for this row: a frame may be laid out twice.)
    let painted = shell
        .layout
        .row_texts
        .iter()
        .rev()
        .find(|(row, _)| *row == id)
        .map_or(String::new(), |(_, t)| t.clone());
    shell.layout.outline_words.push((target, label.to_owned(), painted));
    let keyed = shell.take_click(&id);
    let renaming = shell.renaming.as_ref().is_some_and(|(t, _)| *t == target);
    if renaming {
        let at = Rect::from_min_max(
            pos2(response.rect.left() + 4.0 + 2.0 * INDENT + 30.0, response.rect.top()),
            response.rect.max,
        );
        crate::panels::rename_field_at(shell, ui, target, at);
    }
    let painter = ui.painter().clone();
    let mut on_control = false;
    if let Some(dot) = dot {
        let at = pos2(trailing.right() - 10.0, trailing.center().y);
        painter.circle_filled(at, 4.0, dot.colour());
        let hit = ui.interact(
            Rect::from_center_size(at, vec2(16.0, 16.0)),
            egui::Id::unique(("dot", label)),
            Sense::hover(),
        );
        let _ = hit.on_hover_text(format!("{label}: {}", dot.words()));
        if let Target::Tower(n) = target {
            shell.layout.dots.push((n - 1, dot));
        }
    }
    if let Target::Body(n) = target {
        // The eye (the view's `hide`) and the lock (UI state): shown on hover, or when set.
        let hidden = shell.engine.view(0).hidden.iter().any(|l| l == label);
        let locked = shell.locked.contains(label);
        // Each a full target (24 pt: S19's review, I6), side by side at the row's right.
        let eye = Rect::from_center_size(
            pos2(trailing.right() - 14.0 - theme::HIT, trailing.center().y),
            vec2(theme::HIT, theme::HIT),
        );
        let lock = Rect::from_center_size(
            pos2(trailing.right() - 12.0, trailing.center().y),
            vec2(theme::HIT, theme::HIT),
        );
        let eye_hit = ui.interact(eye, egui::Id::unique(("eye", n)), Sense::click());
        let lock_hit = ui.interact(lock, egui::Id::unique(("lock", n)), Sense::click());
        let show = response.hovered() || eye_hit.hovered() || lock_hit.hovered();
        if show || hidden {
            crate::icons::eye(&painter, eye, !hidden, if hidden { theme::WEAK } else { theme::TEXT });
        }
        if show || locked {
            crate::icons::lock(&painter, lock, locked, if locked { theme::GOLD } else { theme::WEAK });
        }
        shell.layout.buttons.push((format!("eye {label}"), eye));
        shell.layout.buttons.push((format!("lock {label}"), lock));
        let eye_id = format!("{id}/eye");
        let lock_id = format!("{id}/lock");
        shell.register(scope, &eye_id, &format!("{label}: show or hide"), Kind::Button, eye);
        shell.register(scope, &lock_id, &format!("{label}: lock"), Kind::Button, lock);
        if eye_hit.clicked() || shell.take_click(&eye_id) {
            let word = if hidden { "unhide" } else { "hide" };
            let _ = shell.run(&format!("{word} \"{label}\""));
        }
        if lock_hit.clicked() || shell.take_click(&lock_id) {
            if locked {
                shell.locked.remove(label);
            } else {
                shell.locked.insert(label.to_owned());
            }
        }
        on_control = eye_hit.clicked() || lock_hit.clicked() || eye_hit.hovered() || lock_hit.hovered();
    }
    // A click selects, and a click on what is selected keeps it so (Unity, Godot, Blender: S19's
    // review, UX); Ctrl+click toggles it.
    let toggle = ui.input(|i| i.modifiers.command);
    if response.clicked() && !on_control {
        let target = if selected && toggle { None } else { Some(target) };
        if shell.engine.selection() != target {
            shell.apply(Command::Select(target));
        }
    } else if keyed && !selected {
        shell.apply(Command::Select(Some(target)));
    }
    if response.double_clicked() && !on_control {
        shell.apply(Command::Select(Some(target)));
        let _ = shell.run("rename");
    }
    crate::panels::row_menu(shell, &response, target, selected);
}

/// Which of `count` items get a row: the first [`crate::panels::MAX_OUTLINE_ROWS`], and the
/// selected one wherever it is.
fn shown(count: usize, selected: Option<usize>) -> Vec<usize> {
    let mut rows: Vec<usize> = (0..count.min(crate::panels::MAX_OUTLINE_ROWS)).collect();
    if let Some(n) = selected.filter(|n| *n >= crate::panels::MAX_OUTLINE_ROWS && *n < count) {
        rows.push(n);
    }
    rows
}

/// The outliner: a header (the scene's name, `+` to add, a filter), then the tree.
pub fn outliner(shell: &mut Shell, ui: &mut egui::Ui) {
    // Every target at least 24 points a side (S19's review, I6).
    ui.spacing_mut().interact_size = vec2(theme::HIT, theme::HIT);
    // The header: the panel's title as a tab, `+`, and the filter.
    ui.horizontal(|ui| {
        crate::panels::tab_title(ui, "Scene", true);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let add = ui
                // Spaced so that the button is a full target (S19's review, I6).
                .menu_button(RichText::new(" + ").size(15.0), |ui| crate::menus::add_menu(shell, ui))
                .response;
            let add = add.on_hover_text("add a body, a shape or a tower");
            shell.register("tree", "tree/add", "Add", Kind::Button, add.rect);
            shell.layout.buttons.push(("add".to_owned(), add.rect));
            if shell.take_click("tree/add") {
                let _ = shell.run("palette add");
            }
            let mut filter = shell.tree_filter.clone();
            let field = ui.add(
                egui::TextEdit::singleline(&mut filter)
                    .hint_text("filter")
                    .desired_width(ui.available_width() - 6.0),
            );
            shell.register("tree", "tree/filter", "Filter", Kind::Text, field.rect);
            if shell.take_type("tree/filter") {
                field.request_focus();
            }
            shell.tree_filter = filter;
        });
    });
    ui.add_space(4.0);
    let filter = shell.tree_filter.trim().to_lowercase();
    let wanted = |label: &str| filter.is_empty() || label.to_lowercase().contains(&filter);
    let spec = shell.engine.spec().clone();
    let reply = shell.reading().clone();
    let silent = crate::panels::silent(&spec);
    let (scene_row, _) = row(
        shell,
        ui,
        "tree",
        "tree/scene",
        0,
        RowKind::Scene,
        &spec.name,
        shell.engine.selection().is_none(),
    );
    if scene_row.clicked() || shell.take_click("tree/scene") {
        shell.apply(Command::Select(None));
    }
    if group(shell, ui, "tree/array", "Array", Some("add tower")) {
        let selected = match shell.engine.selection() {
            Some(Target::Tower(n)) => Some(n - 1),
            _ => None,
        };
        for n in shown(spec.array.towers.len(), selected) {
            let tower = &spec.array.towers[n];
            if wanted(&tower.label) {
                let dot = dot_of(&reply, n, silent);
                item(
                    shell,
                    ui,
                    "tree/array",
                    Target::Tower(n + 1),
                    RowKind::Tower,
                    &tower.label,
                    "tower",
                    Some(dot),
                );
            }
        }
    }
    if group(shell, ui, "tree/sources", "Bodies", Some("palette add")) {
        let selected = match shell.engine.selection() {
            Some(Target::Body(n)) => Some(n - 1),
            _ => None,
        };
        for n in shown(spec.bodies.len(), selected) {
            let body = &spec.bodies[n];
            if wanted(&body.label) {
                let (kind, words) = body_kind(body);
                item(shell, ui, "tree/sources", Target::Body(n + 1), kind, &body.label, &words, None);
            }
        }
        if spec.bodies.is_empty() {
            crate::panels::empty_guidance(shell, ui, &spec);
        }
    }
    if group(shell, ui, "tree/surroundings", "Surroundings", None) {
        let rows = [
            (spec.room.is_some(), Target::Room, "room"),
            (spec.floor.is_some(), Target::Floor, "floor"),
            (spec.world.is_some(), Target::World, "world"),
        ];
        for (present, target, label) in rows {
            if present && wanted(label) {
                item(shell, ui, "tree/surroundings", target, RowKind::Surroundings, label, label, None);
            }
        }
    }
}

/// The Readings tab: each tower's reading under the playhead as a table (its ΔΦ, the two
/// interferometers' phases, its state), and what the audit warns of. The numbers the tree's
/// rows no longer carry.
pub fn readings(shell: &mut Shell, ui: &mut egui::Ui) {
    let reply = shell.reading().clone();
    let spec = shell.engine.spec().clone();
    let silent = crate::panels::silent(&spec);
    ui.label(RichText::new(format!("a shot read out at {:.2} s", reply.time)).size(11.5).color(theme::WEAK));
    ui.add_space(4.0);
    egui::Grid::new("readings").num_columns(5).spacing([10.0, 4.0]).striped(true).show(ui, |ui| {
        for heading in ["tower", "ΔΦ", "δφ₁ lower", "δφ₂ upper", "state"] {
            ui.label(RichText::new(heading).size(11.5).color(theme::WEAK));
        }
        ui.end_row();
        for (n, tower) in spec.array.towers.iter().enumerate().take(crate::panels::MAX_OUTLINE_ROWS) {
            let dot = dot_of(&reply, n, silent);
            ui.label(RichText::new(&tower.label).size(12.5));
            let number = |v: f64| RichText::new(crate::shell::phase_text(v)).monospace().size(12.0);
            match reply.towers.get(n).filter(|_| !silent) {
                Some(r) => {
                    let mark = if reply.truncated.is_some() { " ◐" } else { "" };
                    let text = format!("{}{mark}", crate::shell::phase_text(r.gradiometer));
                    shell.layout.readings.push((n, text.clone()));
                    shell.layout.reading_values.push((n, [r.gradiometer, r.lower, r.upper]));
                    // Each number's every digit under the pointer: the value the engine gave.
                    let exact = |v: f64| format!("{v:.17e} rad");
                    ui.label(RichText::new(text).monospace().size(12.0).color(theme::STRONG))
                        .on_hover_text(exact(r.gradiometer));
                    ui.label(number(r.lower)).on_hover_text(exact(r.lower));
                    ui.label(number(r.upper)).on_hover_text(exact(r.upper));
                }
                None => {
                    shell.layout.readings.push((n, String::new()));
                    for _ in 0..3 {
                        ui.label(RichText::new("—").color(theme::WEAK));
                    }
                }
            }
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                ui.painter().circle_filled(rect.center(), 4.0, dot.colour());
                ui.label(RichText::new(dot.words()).size(11.5).color(theme::WEAK));
            });
            ui.end_row();
        }
    });
    ui.add_space(4.0);
    ui.label(
        RichText::new(format!(
            "quadrature: {}",
            cav_engine::estimate_text(reply.estimate, reply.settled, reply.truncated)
        ))
        .size(11.5)
        .color(theme::WEAK),
    );
    for warning in &reply.warnings {
        ui.label(RichText::new(&warning.text).size(11.0).color(theme::WARN));
    }
}
