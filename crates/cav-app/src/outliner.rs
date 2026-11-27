//! The scene tree and the readings (S19, `docs/design/panels.md`): the outliner as one tree that
//! names things (a type icon a row, groups that fold, the scene's folders inside them (S20:
//! `cav_sim::folders`, made, nested, renamed and filled here by `folder` commands, each one
//! edit), an eye and a lock, a status dot a tower, no numbers), and the Readings tab where the numbers are (a table of each tower's phases and
//! state). Every row and button is registered in the keyboard's tree under the same scopes as
//! before (`tree/array`, `tree/sources`, `tree/surroundings`), so the Tab order is unchanged.

use cav_engine::{Command, Target};
use cav_sim::{FolderEdit, FolderGroup, FolderItem, FolderSpec};
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
    /// The Probe tool's reading of the point last probed (S20: it was a card over the figure).
    Probe,
}

impl RightTab {
    /// Every tab, in the order drawn, with its title; the title in lower case is the word `tab`
    /// takes (`tab probe`).
    pub const ALL: [(Self, &'static str); 3] =
        [(Self::Inspector, "Inspector"), (Self::Readings, "Readings"), (Self::Probe, "Probe")];
}

/// What a row is, for its icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// The scene itself, the tree's root.
    Scene,
    /// A group of rows (the array, the bodies, the surroundings).
    Group,
    /// A folder of the scene's (S20): a named group of bodies or of towers.
    Folder,
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
        RowKind::Folder => {
            // A folder: a filled tab at its top left, then its body in outline.
            let body = Rect::from_center_size(c + vec2(0.0, 1.5), vec2(2.4 * r, 1.5 * r));
            let tab = body.left_top();
            painter.add(egui::Shape::convex_polygon(
                vec![
                    tab + vec2(0.0, 0.5),
                    tab + vec2(0.0, -2.5),
                    tab + vec2(0.9 * r, -2.5),
                    tab + vec2(1.4 * r, 0.5),
                ],
                colour,
                Stroke::NONE,
            ));
            painter.rect_stroke(body, 1.0, stroke, egui::StrokeKind::Middle);
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
const INDENT: f32 = 3.0 * theme::GRID;
/// Where a row's fold triangle, its icon and its name stand from its indented left edge, points.
const FOLD_AT: f32 = 8.0;
const ICON_AT: f32 = 20.0;
const NAME_AT: f32 = 32.0;
/// A row's height, points: the theme's target, and the rows touch, so it is their pitch too.
pub const ROW: f32 = theme::HIT;

/// A row of the scene tree being dragged onto a folder or a group (S20): what `folder move`
/// would move. Window state, there while the pointer's button is held.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeDrag {
    /// What is dragged.
    pub item: FolderItem,
    /// Its name, drawn beside the pointer.
    pub label: String,
    /// The drop last asked about (a group and a folder of it, or none) and the console line
    /// that would make it, if it would be taken: asked once a row, not once a frame.
    asked: Option<(Place, Option<String>)>,
}

/// Somewhere a row can be dropped: a group, and a folder of it (`None`: directly in the group).
type Place = (FolderGroup, Option<String>);

/// What a folder's eye says of the bodies under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eye {
    /// Every body under it is drawn.
    Shown,
    /// Some are hidden and some are not.
    Mixed,
    /// Every body under it is hidden.
    Hidden,
}

/// A folder's row as the scene tree drew it: what a test, or the demo, finds it by.
#[derive(Clone, Debug, PartialEq)]
pub struct FolderRow {
    /// The folder's label.
    pub label: String,
    /// Where its row is, points.
    pub rect: Rect,
    /// Whether it is open (its rows drawn under it).
    pub open: bool,
    /// How deep its row is indented: 2 for a folder directly in a group.
    pub depth: usize,
    /// Its eye, when it holds a body at any depth; `None` for a folder of towers or an empty one.
    pub eye: Option<Eye>,
    /// Whether every body under it is locked (false when it holds none).
    pub locked: bool,
}

/// The keyboard id of a folder's row: `tree/folder/<label>`.
#[must_use]
pub fn folder_id(label: &str) -> String {
    format!("tree/folder/{label}")
}

/// A label as one console word.
fn quoted(label: &str) -> String {
    format!("\"{label}\"")
}

/// An item as `folder move` names it: `body 2`, `tower 1`, a folder's label in quotes.
fn item_words(item: &FolderItem) -> String {
    match item {
        FolderItem::Body(n) => format!("body {n}"),
        FolderItem::Tower(n) => format!("tower {n}"),
        FolderItem::Folder(label) => quoted(label),
    }
}

/// The group of the tree `item` is in, and the folder it is in now.
fn place_of<'a>(spec: &'a cav_sim::SceneSpec, item: &FolderItem) -> Option<(FolderGroup, Option<&'a str>)> {
    match item {
        FolderItem::Body(n) => {
            spec.bodies.get(n.checked_sub(1)?).map(|b| (FolderGroup::Bodies, b.folder.as_deref()))
        }
        FolderItem::Tower(n) => {
            spec.array.towers.get(n.checked_sub(1)?).map(|t| (FolderGroup::Array, t.folder.as_deref()))
        }
        FolderItem::Folder(label) => spec.folder(label).map(|f| (f.group, f.parent.as_deref())),
    }
}

/// The console line that moves `item` into the folder `to` of `group` (`None`: out of every
/// folder, directly in the group), if that is a move the engine would take and one that changes
/// something: `None` for a body onto the Array, a folder into itself, an item where it is.
#[must_use]
pub fn move_line(
    spec: &cav_sim::SceneSpec,
    item: &FolderItem,
    group: FolderGroup,
    to: Option<&str>,
) -> Option<String> {
    let (own, now) = place_of(spec, item)?;
    if own != group || now == to {
        return None;
    }
    let edit = FolderEdit::Move { items: vec![item.clone()], to: to.map(str::to_owned) };
    spec.folder_edit(&edit).ok()?;
    Some(format!("folder move {} {}", item_words(item), to.map_or_else(|| "none".to_owned(), quoted)))
}

/// Makes a folder (`new-folder`: the `+` menu, the right-click menu): inside the folder in
/// focus; else beside the selected body or tower (in its folder, or its group); else in the
/// Bodies. One edit (`folder new`). The new folder takes the focus and its name is typed in
/// place (a second edit if it is changed, as a rename is).
///
/// # Errors
/// Why the engine refused (too many folders, too deep).
pub fn new_folder(shell: &mut Shell) -> Result<String, String> {
    let spec = shell.engine.spec();
    let focus = shell.tree_focus.clone().filter(|f| spec.folder(f).is_some());
    let within = match (focus, shell.engine.selection()) {
        (Some(folder), _) => folder,
        (None, Some(Target::Body(n))) => {
            spec.bodies[n - 1].folder.clone().unwrap_or_else(|| "bodies".to_owned())
        }
        (None, Some(Target::Tower(n))) => {
            spec.array.towers[n - 1].folder.clone().unwrap_or_else(|| "array".to_owned())
        }
        _ => "bodies".to_owned(),
    };
    let name = (1..)
        .map(|n| if n == 1 { "folder".to_owned() } else { format!("folder {n}") })
        .find(|l| spec.folder(l).is_none())
        .expect("a free label");
    shell.run(&format!("folder new {} in {}", quoted(&name), quoted(&within)))?;
    // Shown: its group and the folders above it open.
    let scene = shell.engine.spec().name.clone();
    let spec = shell.engine.spec();
    let mut above = spec.folder(&name).map(|f| (f.group, f.parent.clone()));
    if let Some((group, _)) = above {
        shell.tree_folded.remove(&(scene.clone(), group_scope(group).to_owned()));
    }
    while let Some((group, Some(parent))) = above {
        shell.tree_folded.remove(&(scene.clone(), folder_id(&parent)));
        above = shell.engine.spec().folder(&parent).map(|f| (group, f.parent.clone()));
    }
    shell.tree_focus = Some(name.clone());
    shell.renaming_folder = Some((name.clone(), name.clone()));
    Ok(name)
}

/// The keyboard scope of a group of the tree.
const fn group_scope(group: FolderGroup) -> &'static str {
    match group {
        FolderGroup::Array => "tree/array",
        FolderGroup::Bodies => "tree/sources",
    }
}

/// A row's fold triangle, centred at `at`.
fn triangle(painter: &egui::Painter, at: egui::Pos2, open: bool) {
    let tri = Rect::from_center_size(at, vec2(8.0, 8.0));
    let points = if open {
        vec![tri.left_top(), tri.right_top(), tri.center_bottom()]
    } else {
        vec![tri.left_top(), tri.right_center(), tri.left_bottom()]
    };
    painter.add(egui::Shape::convex_polygon(points, theme::WEAK, Stroke::NONE));
}

#[allow(clippy::too_many_arguments)]
/// One row: an icon and the label, indented by `depth` with a guide a level. Registers it as
/// `id` under `scope` as a `focus` of the keyboard's tree, and records it in the layout.
/// Returns its response and the space on its right, for buttons.
fn row(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    scope: &str,
    id: &str,
    depth: usize,
    kind: RowKind,
    label: &str,
    selected: bool,
    focus: Kind,
    sense: Sense,
) -> (egui::Response, Rect) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), sense);
    shell.register(scope, id, label, focus, rect);
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 2.0, Color32::from_rgb(0x10, 0x30, 0x3d));
    } else if response.hovered() {
        painter.rect_filled(rect, 2.0, Color32::from_rgb(0x24, 0x24, 0x24));
    }
    let left = rect.left() + INDENT * depth as f32;
    // Indent guides: a thin line for each level above.
    for d in 1..=depth {
        let x = rect.left() + FOLD_AT + INDENT * (d as f32 - 1.0);
        painter.vline(x, rect.y_range(), Stroke::new(1.0, Color32::from_rgb(0x30, 0x30, 0x30)));
    }
    let colour = if selected { theme::STRONG } else { theme::TEXT };
    icon(
        painter,
        Rect::from_center_size(pos2(left + ICON_AT, rect.center().y), vec2(14.0, 14.0)),
        kind,
        colour,
    );
    let name_at = pos2(left + NAME_AT, rect.center().y);
    let trailing =
        Rect::from_min_max(pos2(rect.right() - 2.0 * theme::HIT - theme::GRID, rect.top()), rect.max);
    let renamed = shell.renaming.as_ref().is_some_and(|(t, _)| row_target(id) == Some(*t))
        || shell.renaming_folder.as_ref().is_some_and(|(f, _)| folder_id(f) == id);
    if !renamed {
        // The name stops short of the row's buttons.
        let clip = Rect::from_min_max(rect.min, pos2(trailing.left(), rect.bottom()));
        painter.with_clip_rect(clip.intersect(painter.clip_rect())).text(
            name_at,
            Align2::LEFT_CENTER,
            label,
            theme::body(),
            colour,
        );
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

/// A row that takes drops (a folder's, a group's): while a row is dragged over it, it is lit
/// if the drop would be taken, and on release the move is made (`folder move`, one edit). A
/// drop that would be refused lights nothing, shows the no-drop pointer and does nothing.
fn drop_target(shell: &mut Shell, ui: &egui::Ui, rect: Rect, group: FolderGroup, to: Option<&str>) {
    let Some(drag) = shell.tree_drag.clone() else { return };
    if !ui.input(|i| i.pointer.latest_pos()).is_some_and(|p| rect.contains(p)) {
        return;
    }
    let target = (group, to.map(str::to_owned));
    let line = match &drag.asked {
        Some((asked, line)) if *asked == target => line.clone(),
        _ => {
            let line = move_line(shell.engine.spec(), &drag.item, group, to);
            shell.tree_drag = Some(TreeDrag { asked: Some((target, line.clone())), ..drag });
            line
        }
    };
    shell.layout.tree_drop = Some((to.unwrap_or(group.name()).to_owned(), line.is_some()));
    let Some(line) = line else { return };
    ui.painter().rect_stroke(
        rect.shrink(0.5),
        2.0,
        Stroke::new(1.5, theme::SELECTION_EDGE),
        egui::StrokeKind::Inside,
    );
    if ui.input(|i| i.pointer.any_released()) {
        shell.tree_drag = None;
        if shell.run(&line).is_ok()
            && let Some(folder) = to
        {
            // What was dropped is shown where it landed.
            shell.tree_folded.remove(&(shell.engine.spec().name.clone(), folder_id(folder)));
        }
    }
}

/// Begins a drag of a row when the pointer has moved with its button down on it.
fn drag_source(shell: &mut Shell, response: &egui::Response, item: FolderItem, label: &str) {
    if response.drag_started_by(egui::PointerButton::Primary) {
        shell.tree_drag = Some(TreeDrag { item, label: label.to_owned(), asked: None });
    }
}

/// A group's header: its fold triangle, icon and name, and a `+` that adds into it; `about`,
/// when there is something to say of the group, is its row's tooltip (S20: no sentences in the
/// tree). Returns whether it is open. Its fold is remembered for each scene (UI state). A row
/// dragged onto it leaves its folder (`takes`: the group's folders, if it has them).
fn group(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    scope: &str,
    name: &str,
    add: Option<&str>,
    about: Option<&str>,
    takes: Option<FolderGroup>,
) -> bool {
    let mut open = shell.tree_open(scope);
    // Under the scene, one level in (S19's review, minor 19: the groups sat beside the root).
    // A group's row is a scope of the keyboard's tree while it is open (Enter goes in), a button
    // while it is folded (Enter opens it), as the inspector's folding sections are.
    let focus = if open { Kind::Scope } else { Kind::Button };
    let (response, trailing) =
        row(shell, ui, "tree", scope, 1, RowKind::Group, name, false, focus, Sense::click());
    let response = match about {
        Some(about) => response.on_hover_text(about),
        None => response,
    };
    let painter = ui.painter();
    triangle(painter, pos2(response.rect.left() + FOLD_AT + INDENT, response.rect.center().y), open);
    let mut add_rect = None;
    if let Some(line) = add {
        let plus = Rect::from_min_size(
            pos2(trailing.right() - theme::HIT - 0.5 * theme::GRID, trailing.top()),
            vec2(theme::HIT, theme::HIT),
        );
        let hit = ui.interact(plus, egui::Id::unique(("group add", scope)), Sense::click());
        if hit.hovered() || response.hovered() {
            painter.text(plus.center(), Align2::CENTER_CENTER, "+", FontId::proportional(14.0), theme::TEXT);
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
    if let Some(takes) = takes {
        drop_target(shell, ui, response.rect, takes, None);
    }
    shell.layout.tree_groups.push((scope.to_owned(), open));
    open
}

/// The eye and the lock at a row's right, each a full target ([`theme::HIT`]: S19's review, I6),
/// side by side: where they are.
fn eye_and_lock(trailing: Rect) -> (Rect, Rect) {
    let lock = Rect::from_min_size(
        pos2(trailing.right() - theme::HIT - 0.5 * theme::GRID, trailing.top()),
        vec2(theme::HIT, theme::HIT),
    );
    (lock.translate(vec2(-theme::HIT - 0.5 * theme::GRID, 0.0)), lock)
}

/// An item's row: selects on a click, renames on a double click, the right-click menu, an eye
/// (bodies) or a status dot (towers) on its right, a lock for bodies. Dragged onto a folder or
/// its group it moves there.
#[allow(clippy::too_many_arguments)]
fn item(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    scope: &str,
    depth: usize,
    target: Target,
    kind: RowKind,
    label: &str,
    words: &str,
    dot: Option<Dot>,
) {
    let selected = shell.engine.selection() == Some(target);
    let id = crate::panels::row_id(target);
    let sense = match target {
        Target::Body(_) | Target::Tower(_) => Sense::click_and_drag(),
        _ => Sense::click(),
    };
    let (response, trailing) = row(shell, ui, scope, &id, depth, kind, label, selected, Kind::Button, sense);
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
            pos2(response.rect.left() + NAME_AT + INDENT * depth as f32 - 2.0, response.rect.top()),
            response.rect.max,
        );
        crate::panels::rename_field_at(shell, ui, target, at);
    }
    match target {
        Target::Body(n) => drag_source(shell, &response, FolderItem::Body(n), label),
        Target::Tower(n) => drag_source(shell, &response, FolderItem::Tower(n), label),
        _ => {}
    }
    let painter = ui.painter().clone();
    let mut on_control = false;
    if let Some(dot) = dot {
        let at = pos2(trailing.right() - 0.5 * (theme::HIT + theme::GRID), trailing.center().y);
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
        let (eye, lock) = eye_and_lock(trailing);
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
        shell.tree_focus = None;
        if shell.engine.selection() != target {
            shell.apply(Command::Select(target));
        }
    } else if keyed && !selected {
        shell.tree_focus = None;
        shell.apply(Command::Select(Some(target)));
    }
    if response.double_clicked() && !on_control {
        shell.apply(Command::Select(Some(target)));
        let _ = shell.run("rename");
    }
    crate::panels::row_menu(shell, &response, target, selected);
}

/// The "Move to" list of a right-click menu: none, and the folders `item` may go into (those of
/// its group, in the tree's order, indented by their depth), each a `folder move`.
pub(crate) fn move_menu(shell: &mut Shell, ui: &mut egui::Ui, item: &FolderItem) {
    let spec = shell.engine.spec().clone();
    let Some((group, _)) = place_of(&spec, item) else { return };
    let mut places: Vec<(Option<String>, String)> = vec![(None, "none".to_owned())];
    fn walk(
        spec: &cav_sim::SceneSpec,
        group: FolderGroup,
        parent: Option<&str>,
        depth: usize,
        out: &mut Vec<(Option<String>, String)>,
    ) {
        for folder in spec.folders_in(group, parent) {
            out.push((Some(folder.label.clone()), format!("{}{}", "  ".repeat(depth), folder.label)));
            walk(spec, group, Some(&folder.label), depth + 1, out);
        }
    }
    walk(&spec, group, None, 0, &mut places);
    for (to, text) in places {
        let line = move_line(&spec, item, group, to.as_deref());
        let response = ui.add_enabled(line.is_some(), egui::Button::new(text));
        if let Some(line) = line {
            shell.layout.buttons.push((line.clone(), response.rect));
            if response.clicked() {
                let _ = shell.run(&line);
                ui.close();
            }
        }
    }
}

/// A folder's right-click menu.
fn folder_menu(shell: &mut Shell, ui: &mut egui::Ui, label: &str) {
    let entry = |shell: &mut Shell, ui: &mut egui::Ui, text: &str, about: &str, name: &str| {
        let response = ui.button(text).on_hover_text(about);
        shell.layout.buttons.push((format!("folder menu {name}"), response.rect));
        let clicked = response.clicked();
        if clicked {
            ui.close();
        }
        clicked
    };
    if entry(shell, ui, "New folder", "a folder inside this one", "new") {
        shell.tree_focus = Some(label.to_owned());
        let _ = shell.run("new-folder");
    }
    if entry(shell, ui, "Rename", "type a new name (F2); what it holds stays in it", "rename") {
        shell.tree_focus = Some(label.to_owned());
        let _ = shell.run("rename");
    }
    if entry(shell, ui, "Duplicate", "a copy of the folder and of everything in it, beside it", "duplicate") {
        let _ = shell.run(&format!("folder duplicate {}", quoted(label)));
    }
    if entry(
        shell,
        ui,
        "Remove folder",
        "take the folder away and keep its contents: they move up a level",
        "remove",
    ) {
        let _ = shell.run(&format!("folder remove {}", quoted(label)));
    }
    ui.separator();
    ui.menu_button("Move to", |ui| move_menu(shell, ui, &FolderItem::Folder(label.to_owned())));
}

/// The text field that renames a folder in place (`F2`, a double click, a new folder): Enter
/// keeps the name (`folder rename`: one edit, its members following), Esc or a click elsewhere
/// leaves it as it was. The fold and the focus follow the new name.
fn rename_folder_field(shell: &mut Shell, ui: &mut egui::Ui, label: &str, rect: Rect) {
    let Some((_, mut text)) = shell.renaming_folder.clone() else { return };
    let response = ui.put(rect, egui::TextEdit::singleline(&mut text).font(theme::body()));
    shell.layout.editors.push(("rename folder".to_owned(), response.rect));
    if !response.has_focus() && !response.lost_focus() {
        response.request_focus();
    }
    shell.renaming_folder = Some((label.to_owned(), text.clone()));
    if response.lost_focus() {
        shell.renaming_folder = None;
        let new = text.trim();
        let keep = ui.input(|i| i.key_pressed(egui::Key::Enter)) && !new.is_empty() && new != label;
        if keep && shell.run(&format!("folder rename {} {}", quoted(label), quoted(new))).is_ok() {
            let scene = shell.engine.spec().name.clone();
            if shell.tree_folded.remove(&(scene.clone(), folder_id(label))) {
                shell.tree_folded.insert((scene, folder_id(new)));
            }
            if shell.tree_focus.as_deref() == Some(label) {
                shell.tree_focus = Some(new.to_owned());
            }
        }
    }
}

/// What the tree draws of one group: the scene, the filter, and which items get rows.
struct Tree<'a> {
    spec: &'a cav_sim::SceneSpec,
    /// The filter, in lower case; empty for none.
    filter: &'a str,
    /// The items (from 0) that get a row: [`shown`].
    rows: &'a [usize],
    /// The reading, for the towers' dots.
    reply: &'a cav_engine::ReadingReply,
    /// Whether the scene has nothing to read.
    silent: bool,
}

impl Tree<'_> {
    fn matches(&self, label: &str) -> bool {
        self.filter.is_empty() || label.to_lowercase().contains(self.filter)
    }

    /// The label and the folder of item `n` of `group`.
    fn member(&self, group: FolderGroup, n: usize) -> (&str, Option<&str>) {
        match group {
            FolderGroup::Bodies => (&self.spec.bodies[n].label, self.spec.bodies[n].folder.as_deref()),
            FolderGroup::Array => {
                (&self.spec.array.towers[n].label, self.spec.array.towers[n].folder.as_deref())
            }
        }
    }

    /// Whether the filter keeps the folder: its own name matches, or a folder or an item under
    /// it at any depth does.
    fn keeps(&self, folder: &FolderSpec) -> bool {
        self.matches(&folder.label)
            || self
                .spec
                .folders
                .iter()
                .any(|f| self.spec.folder_is_under(&f.label, &folder.label) && self.matches(&f.label))
            || self.rows.iter().any(|&n| {
                let (label, within) = self.member(folder.group, n);
                within.is_some_and(|f| self.spec.folder_is_under(f, &folder.label)) && self.matches(label)
            })
    }

    /// The rows of `group` inside `parent` (`None`: directly in the group): its folders, each
    /// with what it holds under it when open, then its items, in the scene's order. `scope` is
    /// the keyboard scope they are registered under; `named`, that a folder above matched the
    /// filter (everything under it is then kept).
    #[allow(clippy::too_many_arguments)]
    fn rows(
        &self,
        shell: &mut Shell,
        ui: &mut egui::Ui,
        group: FolderGroup,
        parent: Option<&str>,
        scope: &str,
        depth: usize,
        named: bool,
    ) {
        for folder in self.spec.folders_in(group, parent) {
            if !(named || self.keeps(folder)) {
                continue;
            }
            // While filtering, a folder is drawn open: what matched is under it.
            let holds = self.spec.folders_in(group, Some(&folder.label)).next().is_some()
                || self.rows.iter().any(|&n| self.member(group, n).1 == Some(folder.label.as_str()));
            let open = folder_row(shell, ui, self.spec, scope, folder, depth, holds, !self.filter.is_empty());
            if open {
                let named = named || (!self.filter.is_empty() && self.matches(&folder.label));
                self.rows(shell, ui, group, Some(&folder.label), &folder_id(&folder.label), depth + 1, named);
            }
        }
        for &n in self.rows {
            let (label, within) = self.member(group, n);
            if within != parent || !(named || self.matches(label)) {
                continue;
            }
            match group {
                FolderGroup::Array => {
                    let dot = dot_of(self.reply, n, self.silent);
                    let target = Target::Tower(n + 1);
                    item(shell, ui, scope, depth, target, RowKind::Tower, label, "tower", Some(dot));
                }
                FolderGroup::Bodies => {
                    let (kind, words) = body_kind(&self.spec.bodies[n]);
                    item(shell, ui, scope, depth, Target::Body(n + 1), kind, label, &words, None);
                }
            }
        }
    }
}

/// A folder's row: a fold triangle, the folder icon, its name; for a folder of bodies an eye
/// and a lock that act on every body under it. A click lights it (the tree's focus: what F2 and
/// New folder act on), a click on its triangle folds it, a double click renames it, a
/// right-click opens its menu; it is dragged into another folder, and takes rows dropped on it.
/// Returns whether what it holds is to be drawn.
#[allow(clippy::too_many_arguments)]
fn folder_row(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    spec: &cav_sim::SceneSpec,
    scope: &str,
    folder: &FolderSpec,
    depth: usize,
    holds: bool,
    filtering: bool,
) -> bool {
    let label = folder.label.as_str();
    let id = folder_id(label);
    let mut folded = !shell.tree_open(&id);
    let open = filtering || !folded;
    // As a group: a scope of the keyboard's tree while it is open with rows in it (Enter goes
    // in), a button otherwise (Enter opens it).
    let focus = if open && holds { Kind::Scope } else { Kind::Button };
    if shell.keyboard.nav_on && shell.keyboard.focused() == id {
        shell.tree_focus = Some(label.to_owned());
    }
    let lit = shell.tree_focus.as_deref() == Some(label);
    let (response, trailing) =
        row(shell, ui, scope, &id, depth, RowKind::Folder, label, lit, focus, Sense::click_and_drag());
    let rect = response.rect;
    let left = rect.left() + INDENT * depth as f32;
    let painter = ui.painter().clone();
    triangle(&painter, pos2(left + FOLD_AT, rect.center().y), open);
    let fold = Rect::from_min_max(pos2(left, rect.top()), pos2(left + ICON_AT - 7.0, rect.bottom()));
    shell.layout.buttons.push((format!("fold folder {label}"), fold));
    if shell.renaming_folder.as_ref().is_some_and(|(f, _)| f == label) {
        let at = Rect::from_min_max(pos2(left + NAME_AT - 2.0, rect.top()), rect.max);
        rename_folder_field(shell, ui, label, at);
    }
    drag_source(shell, &response, FolderItem::Folder(label.to_owned()), label);
    // The eye and the lock of a folder of bodies act on every body under it, at any depth.
    let mut on_control = false;
    let mut eye_state = None;
    let mut all_locked = false;
    let under: Vec<&str> = match folder.group {
        FolderGroup::Bodies => {
            spec.bodies_under(label).into_iter().map(|n| spec.bodies[n].label.as_str()).collect()
        }
        FolderGroup::Array => Vec::new(),
    };
    if !under.is_empty() {
        let hidden = under.iter().filter(|l| shell.engine.view(0).hidden.iter().any(|h| h == *l)).count();
        let locked = under.iter().filter(|l| shell.locked.contains(**l)).count();
        let state = match hidden {
            0 => Eye::Shown,
            n if n == under.len() => Eye::Hidden,
            _ => Eye::Mixed,
        };
        eye_state = Some(state);
        all_locked = locked == under.len();
        let (eye, lock) = eye_and_lock(trailing);
        let eye_hit = ui.interact(eye, egui::Id::unique(("folder eye", label)), Sense::click());
        let lock_hit = ui.interact(lock, egui::Id::unique(("folder lock", label)), Sense::click());
        let show = response.hovered() || eye_hit.hovered() || lock_hit.hovered();
        if show || state != Eye::Shown {
            // Off when all are hidden; when only some are, the open eye dimmed with a dot.
            let colour = if state == Eye::Shown { theme::TEXT } else { theme::WEAK };
            crate::icons::eye(&painter, eye, state != Eye::Hidden, colour);
            if state == Eye::Mixed {
                painter.circle_filled(eye.right_top() + vec2(-4.0, 5.0), 2.0, theme::WARN);
            }
        }
        if show || locked > 0 {
            let colour = if all_locked { theme::GOLD } else { theme::WEAK };
            crate::icons::lock(&painter, lock, all_locked, colour);
        }
        let eye_hit = eye_hit.on_hover_text(match state {
            Eye::Shown => "hide every body in the folder",
            Eye::Mixed => "some are hidden: hide every body in the folder",
            Eye::Hidden => "show every body in the folder",
        });
        let lock_hit = lock_hit.on_hover_text(if all_locked {
            "unlock every body in the folder"
        } else {
            "lock every body in the folder: the view's pointer moves none of them"
        });
        shell.layout.buttons.push((format!("eye folder {label}"), eye));
        shell.layout.buttons.push((format!("lock folder {label}"), lock));
        let eye_id = format!("{id}/eye");
        let lock_id = format!("{id}/lock");
        shell.register(scope, &eye_id, &format!("{label}: show or hide all"), Kind::Button, eye);
        shell.register(scope, &lock_id, &format!("{label}: lock all"), Kind::Button, lock);
        if eye_hit.clicked() || shell.take_click(&eye_id) {
            // One `hide` (or `unhide`) a body: the view's command, journalled, never undone.
            let hide = state != Eye::Hidden;
            let word = if hide { "hide" } else { "unhide" };
            for body in &under {
                let is_hidden = shell.engine.view(0).hidden.iter().any(|h| h == body);
                if is_hidden != hide {
                    let _ = shell.run(&format!("{word} {}", quoted(body)));
                }
            }
        }
        if lock_hit.clicked() || shell.take_click(&lock_id) {
            for body in &under {
                if all_locked {
                    shell.locked.remove(*body);
                } else {
                    shell.locked.insert((*body).to_owned());
                }
            }
        }
        on_control = eye_hit.clicked() || lock_hit.clicked() || eye_hit.hovered() || lock_hit.hovered();
    }
    let on_fold = response.interact_pointer_pos().is_some_and(|p| fold.contains(p));
    let keyed = shell.take_click(&id);
    if (response.clicked() && on_fold && !filtering) || keyed {
        folded = !folded;
        let key = (spec.name.clone(), id.clone());
        if folded {
            shell.tree_folded.insert(key);
        } else {
            shell.tree_folded.remove(&key);
        }
    } else if (response.clicked() || response.secondary_clicked()) && !on_control {
        // The folder is lit, and nothing of the scene is selected with it.
        shell.tree_focus = Some(label.to_owned());
        if shell.engine.selection().is_some() {
            shell.apply(Command::Select(None));
        }
    }
    if response.double_clicked() && !on_control && !on_fold {
        shell.tree_focus = Some(label.to_owned());
        shell.renaming_folder = Some((label.to_owned(), label.to_owned()));
    }
    let menu = if response.secondary_clicked() {
        Some(egui::SetOpenCommand::Bool(true))
    } else if response.clicked() {
        Some(egui::SetOpenCommand::Bool(false))
    } else {
        None
    };
    egui::Popup::menu(&response)
        .open_memory(menu)
        .at_pointer_fixed()
        .show(|ui| folder_menu(shell, ui, label));
    drop_target(shell, ui, rect, folder.group, Some(label));
    let open = filtering || !folded;
    shell.layout.tree_folders.push(FolderRow {
        label: label.to_owned(),
        rect,
        open,
        depth,
        eye: eye_state,
        locked: all_locked,
    });
    open
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
    // Every target the theme's side (S19's review, I6; S20: 20 pt), the rows touching.
    theme::dense(ui, 0.0);
    // The header: the panel's title as a tab, `+`, and the filter.
    ui.horizontal(|ui| {
        crate::panels::tab_title(ui, "Scene", true);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // A full target by the theme's least size of a control, not by padding (S20).
            let add = ui
                .menu_button(RichText::new("+").size(14.0), |ui| crate::menus::add_menu(shell, ui))
                .response;
            let add = add.on_hover_text("add a body, a shape, a tower or a folder");
            shell.register("tree", "tree/add", "Add", Kind::Button, add.rect);
            shell.layout.buttons.push(("add".to_owned(), add.rect));
            if shell.take_click("tree/add") {
                let _ = shell.run("palette add");
            }
            let mut filter = shell.tree_filter.clone();
            let field = ui.add(
                egui::TextEdit::singleline(&mut filter)
                    .hint_text("filter")
                    .desired_width(ui.available_width() - theme::GRID),
            );
            shell.register("tree", "tree/filter", "Filter", Kind::Text, field.rect);
            if shell.take_type("tree/filter") {
                field.request_focus();
            }
            shell.tree_filter = filter;
        });
    });
    ui.add_space(theme::GRID);
    let filter = shell.tree_filter.trim().to_lowercase();
    let wanted = |label: &str| filter.is_empty() || label.to_lowercase().contains(&filter);
    let spec = shell.engine.spec().clone();
    let reply = shell.reading().clone();
    let silent = crate::panels::silent(&spec);
    // The folder in focus, and the one being renamed, are folders of this scene.
    if shell.tree_focus.as_ref().is_some_and(|f| spec.folder(f).is_none()) {
        shell.tree_focus = None;
    }
    if shell.renaming_folder.as_ref().is_some_and(|(f, _)| spec.folder(f).is_none()) {
        shell.renaming_folder = None;
    }
    let (scene_row, _) = row(
        shell,
        ui,
        "tree",
        "tree/scene",
        0,
        RowKind::Scene,
        &spec.name,
        shell.engine.selection().is_none() && shell.tree_focus.is_none(),
        Kind::Button,
        Sense::click(),
    );
    if scene_row.clicked() || shell.take_click("tree/scene") {
        shell.tree_focus = None;
        shell.apply(Command::Select(None));
    }
    if group(shell, ui, "tree/array", "Array", Some("add tower"), None, Some(FolderGroup::Array)) {
        let selected = match shell.engine.selection() {
            Some(Target::Tower(n)) => Some(n - 1),
            _ => None,
        };
        let rows = shown(spec.array.towers.len(), selected);
        let tree = Tree { spec: &spec, filter: &filter, rows: &rows, reply: &reply, silent };
        tree.rows(shell, ui, FolderGroup::Array, None, "tree/array", 2, false);
    }
    // What an empty group means is its tooltip, not a sentence in the tree (S20).
    let about = spec.bodies.is_empty().then(|| crate::panels::empty_words(&spec));
    if group(shell, ui, "tree/sources", "Bodies", Some("palette add"), about, Some(FolderGroup::Bodies)) {
        let selected = match shell.engine.selection() {
            Some(Target::Body(n)) => Some(n - 1),
            _ => None,
        };
        let rows = shown(spec.bodies.len(), selected);
        let tree = Tree { spec: &spec, filter: &filter, rows: &rows, reply: &reply, silent };
        tree.rows(shell, ui, FolderGroup::Bodies, None, "tree/sources", 2, false);
        if spec.bodies.is_empty() {
            crate::panels::empty_guidance(shell, ui);
        }
    }
    if group(shell, ui, "tree/surroundings", "Surroundings", None, None, None) {
        let rows = [
            (spec.room.is_some(), Target::Room, "room"),
            (spec.floor.is_some(), Target::Floor, "floor"),
            (spec.world.is_some(), Target::World, "world"),
        ];
        for (present, target, label) in rows {
            if present && wanted(label) {
                item(shell, ui, "tree/surroundings", 2, target, RowKind::Surroundings, label, label, None);
            }
        }
    }
    // With the ring on a row that is no folder's, no folder is lit.
    if shell.keyboard.nav_on
        && shell.keyboard.within("tree")
        && !shell.keyboard.focused().starts_with("tree/folder/")
    {
        shell.tree_focus = None;
    }
    // A row in a drag: its name beside the pointer, and the pointer says whether a drop here
    // would be taken. The drag ends with the button's release, wherever that is.
    if let Some(drag) = &shell.tree_drag {
        let refused = shell.layout.tree_drop.as_ref().is_some_and(|(_, ok)| !ok);
        ui.ctx().set_cursor_icon(if refused { egui::CursorIcon::NoDrop } else { egui::CursorIcon::Grabbing });
        if let Some(at) = ui.input(|i| i.pointer.latest_pos()) {
            let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("tree drag"));
            let painter = ui.ctx().layer_painter(layer);
            let colour = if refused { theme::WEAK } else { theme::STRONG };
            let galley = painter.layout_no_wrap(drag.label.clone(), theme::body(), colour);
            let ghost = Rect::from_min_size(
                at + vec2(12.0, 4.0),
                galley.size() + vec2(2.0 * theme::GRID, theme::GRID),
            );
            painter.rect_filled(ghost, 2.0, theme::WIDGET);
            painter.galley(ghost.min + vec2(theme::GRID, 0.5 * theme::GRID), galley, colour);
        }
        if !ui.input(|i| i.pointer.primary_down()) {
            shell.tree_drag = None;
        }
    }
}

/// The width of a column of numbers in the right panel's tables, points: a sign, four digits, a
/// point and three more in the monospace face.
const NUMBER_COLUMN: f32 = 62.0;

/// One cell of a table's row: `text` in a box `width` wide and a row high, at the box's right
/// when `right` (a number) or its left (a name).
fn cell(ui: &mut egui::Ui, width: f32, right: bool, text: RichText) -> egui::Response {
    let size = vec2(width, theme::HIT);
    let label = egui::Label::new(text).truncate();
    if right { theme::fixed_right(ui, size, label) } else { theme::fixed_left(ui, size, label) }
}

/// A table's row `n` (from 0; the heading is row 0): a faint band behind every other row, then
/// `cells` laid out along it.
fn table_row(ui: &mut egui::Ui, n: usize, cells: impl FnOnce(&mut egui::Ui)) {
    if n > 0 && n.is_multiple_of(2) {
        let band = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), theme::HIT));
        ui.painter().rect_filled(band, 0.0, ui.visuals().faint_bg_color);
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme::GRID;
        cells(ui);
    });
}

/// A number of a table, in the monospace face.
fn number(text: String, colour: Color32) -> RichText {
    RichText::new(text).font(theme::mono()).color(colour)
}

/// A table's heading, small and muted.
fn heading(text: impl Into<String>) -> RichText {
    RichText::new(text).size(theme::SMALL_PT).color(theme::WEAK)
}

/// The Readings tab: each tower's reading under the playhead as a table (its ΔΦ, the two
/// interferometers' phases, its state), and what the audit warns of. The numbers the tree's
/// rows no longer carry. One unit a column, in its heading, chosen from the column's largest
/// magnitude ([`crate::shell::phase_unit`]); the numbers monospace and right-aligned, so their
/// points stand under one another (S20, `docs/design/workspace.md` §0).
pub fn readings(shell: &mut Shell, ui: &mut egui::Ui) {
    theme::dense(ui, 0.0);
    let reply = shell.reading().clone();
    let spec = shell.engine.spec().clone();
    let silent = crate::panels::silent(&spec);
    let towers = spec.array.towers.len().min(crate::panels::MAX_OUTLINE_ROWS);
    let values =
        |n: usize| reply.towers.get(n).filter(|_| !silent).map(|r| [r.gradiometer, r.lower, r.upper]);
    let units: [(f64, &str); 3] = std::array::from_fn(|k| {
        let largest =
            (0..towers).filter_map(values).map(|v| v[k].abs()).filter(|v| v.is_finite()).fold(0.0, f64::max);
        crate::shell::phase_unit(largest)
    });
    let dot_width = 14.0;
    let name_width =
        (ui.available_width() - 3.0 * (NUMBER_COLUMN + theme::GRID) - dot_width - theme::GRID).max(40.0);
    table_row(ui, 0, |ui| {
        cell(ui, name_width, false, heading(format!("t = {:.2} s", reply.time)))
            .on_hover_text("the shot read out at this time");
        for (name, (_, unit)) in ["ΔΦ", "δφ₁", "δφ₂"].into_iter().zip(units) {
            cell(ui, NUMBER_COLUMN, true, heading(format!("{name}, {unit}")));
        }
    });
    for (n, tower) in spec.array.towers.iter().enumerate().take(towers) {
        let dot = dot_of(&reply, n, silent);
        table_row(ui, n + 1, |ui| {
            cell(ui, name_width, false, RichText::new(&tower.label).size(theme::BODY_PT).color(theme::TEXT));
            match values(n) {
                Some(v) => {
                    let mark = if reply.truncated.is_some() { " ◐" } else { "" };
                    let text = crate::shell::phase_in(v[0], units[0].0);
                    shell.layout.readings.push((n, format!("{text} {}{mark}", units[0].1)));
                    shell.layout.reading_values.push((n, v));
                    for k in 0..3 {
                        let text = crate::shell::phase_in(v[k], units[k].0);
                        let colour = if k == 0 { theme::STRONG } else { theme::TEXT };
                        // Each number's every digit under the pointer: the value the engine gave.
                        cell(ui, NUMBER_COLUMN, true, number(text, colour))
                            .on_hover_text(format!("{:.17e} rad", v[k]));
                    }
                }
                None => {
                    shell.layout.readings.push((n, String::new()));
                    for _ in 0..3 {
                        cell(ui, NUMBER_COLUMN, true, number("—".to_owned(), theme::WEAK));
                    }
                }
            }
            // The state as the tree's dot, its words under the pointer: the table fits the
            // panel's fixed width (S19's review: the words ran past its edge).
            let (rect, response) = ui.allocate_exact_size(vec2(dot_width, theme::HIT), Sense::hover());
            ui.painter().circle_filled(rect.center(), 4.0, dot.colour());
            let mark = if reply.truncated.is_some() { " (◐: of part of the world's matter)" } else { "" };
            let _ = response.on_hover_text(format!("{}{mark}", dot.words()));
        });
    }
    ui.add_space(theme::GRID);
    ui.label(heading(format!(
        "quadrature {}",
        cav_engine::estimate_text(reply.estimate, reply.settled, reply.truncated)
    )));
    for warning in &reply.warnings {
        ui.add(
            egui::Label::new(RichText::new(&warning.text).size(theme::SMALL_PT).color(theme::WARN)).wrap(),
        );
    }
}

/// The Probe tab (S20; S19's second review, minor 13: the reading was a card over the figure's
/// top left): the reading of the point last probed, in rows with units, then each tower's
/// distance and the cell's share of its phase as a table, one unit a column. Only the point's
/// mark is drawn in the view. With nothing probed it says so in three words. What is drawn is
/// recorded in the layout ([`crate::Layout::probe`]), the engine's own numbers.
pub fn probe(shell: &mut Shell, ui: &mut egui::Ui) {
    theme::dense(ui, 0.0);
    let Some(probed) = shell.probed.clone() else {
        ui.label(heading("no point probed"))
            .on_hover_text("with the Probe tool in hand, click a point of the view");
        return;
    };
    let spec = shell.engine.spec().clone();
    let [x, y, z] = probed.point;
    let short = crate::panels::short_number;
    let g = cav_core::Vec3::from_array(probed.g);
    let size = g.length();
    // Nothing counted where something is left out: not known, not zero (S19's second review,
    // minor 12).
    let unknown = probed.reads_nothing();
    let mut rows: Vec<(&str, String)> = vec![
        ("At", format!("{}, {}, {} m", short(x), short(y), short(z))),
        ("Gravity", if unknown { "—".to_owned() } else { crate::panels::si(size, "m/s²") }),
    ];
    if size > 0.0 {
        let d = g * (1.0 / size);
        rows.push(("Towards", format!("{:+.2}, {:+.2}, {:+.2}", d.x, d.y, d.z)));
    }
    if !unknown {
        rows.push(("Of", probed.counted().to_owned()));
    }
    match &probed.cell {
        Some(c) => {
            let body = spec.bodies.get(c.body).map_or("", |b| b.label.as_str());
            rows.push(("Cell of", body.to_owned()));
            rows.push(("Material", c.material.clone().unwrap_or_else(|| "uniform".to_owned())));
            rows.push(("Cell mass", format!("{} kg", crate::editors::shown(c.mass))));
            rows.push(("Cell side", format!("{} m", crate::editors::shown(c.side))));
            if c.block.1 > c.side {
                rows.push(("In", format!("a block of {}³ cells alike", (c.block.1 / c.side).round())));
            }
        }
        None => rows.push(("Cell", "no body here".to_owned())),
    }
    if !probed.not_counted.is_empty() {
        rows.push(("Not counted", probed.not_counted.join("; ")));
    }
    shell.layout.probe_rows = rows.iter().map(|(name, value)| ((*name).to_owned(), value.clone())).collect();
    crate::inspector::info_rows(shell, ui, &rows);
    ui.add_space(theme::GRID);
    let shown = probed.towers.len().min(crate::panels::MAX_OUTLINE_ROWS);
    let largest = probed.towers[..shown]
        .iter()
        .filter_map(|t| t.phase)
        .map(f64::abs)
        .filter(|v| v.is_finite())
        .fold(0.0, f64::max);
    let (scale, unit) = crate::shell::phase_unit(largest);
    // Room for its heading ("share of ΔΦ, µrad").
    let share_width = 100.0;
    let name_width = (ui.available_width() - NUMBER_COLUMN - share_width - 2.0 * theme::GRID).max(40.0);
    table_row(ui, 0, |ui| {
        cell(ui, name_width, false, heading("tower"));
        cell(ui, NUMBER_COLUMN, true, heading("distance, m"));
        cell(ui, share_width, true, heading(format!("share of ΔΦ, {unit}")))
            .on_hover_text("the probed cell's share of each tower's phase");
    });
    for (n, t) in probed.towers.iter().take(shown).enumerate() {
        table_row(ui, n + 1, |ui| {
            cell(ui, name_width, false, RichText::new(&t.label).size(theme::BODY_PT).color(theme::TEXT));
            cell(ui, NUMBER_COLUMN, true, number(format!("{:.2}", t.distance), theme::TEXT))
                .on_hover_text(format!("{:.17e} m", t.distance));
            let (share, colour) = match (&t.phase, &t.why) {
                (Some(phase), _) => (crate::shell::phase_in(*phase, scale), theme::STRONG),
                (None, Some(_)) => ("not read".to_owned(), theme::WEAK),
                (None, None) => ("—".to_owned(), theme::WEAK),
            };
            let label = cell(ui, share_width, true, number(share, colour));
            match (&t.phase, &t.why) {
                (Some(phase), _) => label.on_hover_text(format!("{phase:.17e} rad")),
                (None, Some(why)) => label.on_hover_text(why),
                (None, None) => label,
            };
        });
    }
    shell.layout.probe = Some(probed);
}
