//! The tool palette and the tool's options row (`docs/design/workspace.md` §3; Photoshop's
//! toolbox and options bar, Blender's T panel).
//!
//! **The palette** floats inside the canvas of one scene viewport, its *host*: the active
//! viewport if it shows the scene, otherwise the first that does, in the labels' order
//! ([`host`]); with no scene on show there is no palette (the tools' keys still take a tool: it
//! is in hand when a scene comes back). It is icon-only buttons of [`theme::ICON`], one for each
//! tool of `ui/tools.ron`, with a grip at its head, and it never changes the canvas's size: it
//! is drawn over the picture, in the viewport's own layer (so a window or a floating viewport
//! covers it as it covers the picture).
//!
//! Where it stands is an [`Anchor`], one of the canvas's four corners and four edge middles,
//! kept for each layout name in the window's state ([`crate::Shell::tool_anchors`]; never
//! journalled). At a corner or a side it is a column (two columns when the canvas is too short
//! for one); at the middle of the top or the bottom edge it lies along the edge. It keeps clear
//! of the navigation gizmo ([`gizmo_zone`]). Dragged by its grip it follows the pointer, and let
//! go it snaps to the anchor nearest the pointer ([`nearest`]).
//!
//! **A flyout**: a tool with variants (`variants` in `ui/tools.ron`) has a corner mark, and its
//! variants open beside the palette, away from the canvas's nearer edge, on a press held
//! [`HOLD`] seconds or a right-click; a plain click only takes the tool. Choosing a variant runs
//! `tool <id> <variant>`. From the keyboard the same list is the drop-down of the stop after the
//! tool's own in the `toolbar` scope ([`register`]).
//!
//! **The options row** ([`options_row`]) is one row under the host's header, there only while
//! the tool in hand has settings ([`crate::tools::Behaviour::has_options`]) or a transform
//! session is under way; otherwise it has no height and the canvas has the room.
//!
//! Lengths are points. Nothing here edits the scene.

use cav_engine::{Command, PaneContent, ViewCommand};
use egui::{Align, Align2, Button, Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::keyboard::Kind;
use crate::panels::Asked;
use crate::shell::Shell;
use crate::tools::{Behaviour, ToolData, Variants};
use crate::{icons, theme};

/// How far the palette stands from the canvas's edges, points (the navigation gizmo's margin).
pub const MARGIN: f32 = crate::navgizmo::MARGIN;
/// The room round the buttons inside the palette, points.
pub const PAD: f32 = theme::GRID;
/// The gap between two buttons, points: with [`theme::ICON`] a pitch of 24, on the 4 pt grid.
pub const GAP: f32 = theme::ROW_GAP;
/// The grip's extent along the palette, points.
pub const GRIP: f32 = 8.0;
/// How long a press on a tool with variants is held before its flyout opens, seconds.
pub const HOLD: f64 = 0.4;
/// The keyboard's scope of the palette (a big scope of `ui/actions.ron`).
pub const SCOPE: &str = "toolbar";

/// Where the palette stands in its canvas: a corner or the middle of an edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Anchor {
    /// The top left corner: where it starts.
    #[default]
    TopLeft,
    /// The middle of the top edge (it lies along the edge).
    Top,
    /// The top right corner, under the navigation gizmo.
    TopRight,
    /// The middle of the left edge.
    Left,
    /// The middle of the right edge.
    Right,
    /// The bottom left corner.
    BottomLeft,
    /// The middle of the bottom edge (it lies along the edge).
    Bottom,
    /// The bottom right corner.
    BottomRight,
}

impl Anchor {
    /// The eight anchors, in reading order.
    pub const ALL: [Self; 8] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Left,
        Self::Right,
        Self::BottomLeft,
        Self::Bottom,
        Self::BottomRight,
    ];

    /// Its name, as `tool-palette <name>` takes it: `top-left`, `right`, `bottom`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::TopLeft => "top-left",
            Self::Top => "top",
            Self::TopRight => "top-right",
            Self::Left => "left",
            Self::Right => "right",
            Self::BottomLeft => "bottom-left",
            Self::Bottom => "bottom",
            Self::BottomRight => "bottom-right",
        }
    }

    /// The anchor called `name`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|anchor| anchor.name() == name)
    }

    /// Where it is in a canvas, as fractions across and down: 0, ½ or 1 each.
    #[must_use]
    pub const fn at(self) -> (f32, f32) {
        let across = match self {
            Self::TopLeft | Self::Left | Self::BottomLeft => 0.0,
            Self::Top | Self::Bottom => 0.5,
            Self::TopRight | Self::Right | Self::BottomRight => 1.0,
        };
        let down = match self {
            Self::TopLeft | Self::Top | Self::TopRight => 0.0,
            Self::Left | Self::Right => 0.5,
            Self::BottomLeft | Self::Bottom | Self::BottomRight => 1.0,
        };
        (across, down)
    }

    /// Whether the palette lies along the edge there (a row), not down it (a column): the
    /// middles of the top and bottom edges.
    #[must_use]
    pub const fn along(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }
}

/// The palette laid out in a canvas.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// The whole palette.
    pub rect: Rect,
    /// Its grip: what it is dragged by.
    pub grip: Rect,
    /// Each tool's button, in the tools' order.
    pub buttons: Vec<Rect>,
    /// How many lanes of buttons it has: 1, or 2 where one does not fit.
    pub lanes: usize,
    /// Whether it lies along a top or bottom edge (the buttons run across), not down a side.
    pub along: bool,
}

impl Plan {
    /// The same palette with its corner at `min`.
    fn moved_to(&self, min: Pos2) -> Self {
        let by = min - self.rect.min;
        Self {
            rect: self.rect.translate(by),
            grip: self.grip.translate(by),
            buttons: self.buttons.iter().map(|b| b.translate(by)).collect(),
            ..*self
        }
    }
}

/// What the palette keeps clear of in a canvas: the navigation gizmo's triad and the three
/// buttons under it, with a step of the grid round them. `None` in a canvas too small for a
/// gizmo.
#[must_use]
pub fn gizmo_zone(canvas: Rect) -> Option<Rect> {
    let triad = crate::navgizmo::place(canvas)?;
    let buttons = 3.0 * crate::navgizmo::BUTTON + 4.0;
    Some(
        Rect::from_min_max(
            pos2(triad.left().min(triad.right() - buttons), triad.top()),
            pos2(triad.right(), triad.bottom() + 2.0 + crate::navgizmo::BUTTON),
        )
        .expand(theme::GRID),
    )
}

/// The palette of `count` tools anchored at `anchor` in `canvas`, clear of `keep_clear` (the
/// gizmo's zone): in one lane if that fits the canvas less [`MARGIN`] all round, else in two;
/// where it would cover `keep_clear` it stands below it (a column) or beside it (a row), or the
/// other way if that does not fit. `None` when the canvas has no room for it even so.
#[must_use]
pub fn plan(canvas: Rect, anchor: Anchor, count: usize, keep_clear: Option<Rect>) -> Option<Plan> {
    let room = canvas.shrink(MARGIN);
    let along = anchor.along();
    let (across, down) = anchor.at();
    (1..=2).find_map(|lanes| {
        let steps = count.div_ceil(lanes).max(1);
        let main = 2.0 * PAD + GRIP + GAP + steps as f32 * theme::ICON + (steps - 1) as f32 * GAP;
        let cross = 2.0 * PAD + lanes as f32 * theme::ICON + (lanes - 1) as f32 * GAP;
        let size = if along { vec2(main, cross) } else { vec2(cross, main) };
        if size.x > room.width() || size.y > room.height() {
            return None;
        }
        let min = pos2(
            (room.left() + across * (room.width() - size.x)).round(),
            (room.top() + down * (room.height() - size.y)).round(),
        );
        let rect = Rect::from_min_size(min, size);
        let rect = match keep_clear.filter(|zone| zone.intersects(rect)) {
            None => rect,
            Some(zone) => {
                let below = Rect::from_min_size(pos2(min.x, zone.bottom()), size);
                let beside = Rect::from_min_size(pos2(zone.left() - size.x, min.y), size);
                let tries = if along { [beside, below] } else { [below, beside] };
                tries.into_iter().find(|rect| room.contains_rect(*rect))?
            }
        };
        let inner = rect.shrink(PAD);
        let (grip, first) = if along {
            (Rect::from_min_size(inner.min, vec2(GRIP, inner.height())), inner.min + vec2(GRIP + GAP, 0.0))
        } else {
            (Rect::from_min_size(inner.min, vec2(inner.width(), GRIP)), inner.min + vec2(0.0, GRIP + GAP))
        };
        let pitch = theme::ICON + GAP;
        let buttons = (0..count)
            .map(|k| {
                let (lane, step) = ((k % lanes) as f32, (k / lanes) as f32);
                let offset = if along { vec2(step, lane) } else { vec2(lane, step) };
                Rect::from_min_size(first + offset * pitch, vec2(theme::ICON, theme::ICON))
            })
            .collect();
        Some(Plan { rect, grip, buttons, lanes, along })
    })
}

/// The anchor of `canvas` nearest `point`: where a palette let go there snaps to.
#[must_use]
pub fn nearest(canvas: Rect, point: Pos2) -> Anchor {
    let room = canvas.shrink(MARGIN);
    let place = |anchor: Anchor| {
        let (across, down) = anchor.at();
        pos2(room.left() + across * room.width(), room.top() + down * room.height())
    };
    Anchor::ALL
        .into_iter()
        .min_by(|a, b| place(*a).distance_sq(point).total_cmp(&place(*b).distance_sq(point)))
        .unwrap_or_default()
}

/// The viewport (pane, from 0) the palette is drawn in: the active one if it shows the scene,
/// otherwise the first that does, in the labels' order; `None` when none does, or when the
/// active one is maximised and is a plot (the others are not drawn).
#[must_use]
pub fn host(engine: &cav_engine::Engine, maximised: bool) -> Option<usize> {
    let scene = |pane: usize| engine.content(pane) == PaneContent::Scene;
    let active = engine.active_pane();
    if scene(active) {
        Some(active)
    } else if maximised {
        None
    } else {
        (0..engine.panes()).find(|pane| scene(*pane))
    }
}

/// Whether the options row is shown now: a transform session is under way, or the tool in hand
/// has settings.
#[must_use]
pub fn options_apply(shell: &Shell) -> bool {
    shell.session.is_some() || shell.behaviour().has_options()
}

/// The keyboard's id of the stop that opens `tool`'s flyout.
#[must_use]
pub fn variants_stop(tool: &str) -> String {
    format!("{SCOPE}/{tool}/variants")
}

/// What the palette drew this frame, for the keyboard's stops.
#[derive(Clone, Debug)]
pub(crate) struct Drawn {
    /// The whole palette.
    pub rect: Rect,
    /// Each tool's button, in the tools' order.
    pub buttons: Vec<Rect>,
}

/// What a tool's button says on hover: its one name, its key, what it does, and how its flyout
/// opens if it has one.
fn tooltip(tool: &ToolData) -> String {
    let line = format!("tool {}", tool.id);
    let key = crate::actions::table()
        .actions
        .iter()
        .find(|a| a.line == line)
        .map(crate::actions::Action::shortcut_text)
        .unwrap_or_default();
    let head = if key.is_empty() { tool.label.clone() } else { format!("{} ({key})", tool.label) };
    format!("{head}\n{}", tool.about)
}

/// The variant of `tool` that is chosen now, by name.
fn chosen(shell: &Shell, tool: &ToolData) -> Option<String> {
    match tool.variants {
        Variants::None => None,
        Variants::Starters => Some(shell.place_starter.clone()),
    }
}

/// A variant's small picture in `rect`.
fn variant_icon(painter: &egui::Painter, rect: Rect, source: Variants, name: &str, colour: Color32) {
    match source {
        Variants::None => {}
        Variants::Starters => {
            let starter = cav_sim::starters::find(name);
            match starter.and_then(|s| s.body.as_ref()) {
                Some(body) => icons::starter(painter, rect, Some(body), colour),
                // A tower: an upright line on its foot.
                None => {
                    let middle = rect.center().x;
                    let stroke = Stroke::new(1.6, colour);
                    painter.line_segment(
                        [pos2(middle, rect.top() + 1.0), pos2(middle, rect.bottom() - 2.0)],
                        stroke,
                    );
                    painter.line_segment(
                        [pos2(middle - 3.0, rect.bottom() - 2.0), pos2(middle + 3.0, rect.bottom() - 2.0)],
                        stroke,
                    );
                }
            }
        }
    }
}

/// Draws the palette over `canvas` (pane `pane`'s, the host's) and takes its input: a click
/// takes a tool (`tool <id>`, run with the frame's other lines), the grip's drag moves it, a
/// hold or a right-click opens a tool's flyout. Nothing is drawn in a canvas with no room for
/// it ([`plan`]). What it drew is left in `asked.tools` for [`register`].
pub(crate) fn draw(
    shell: &mut Shell,
    ui: &egui::Ui,
    asked: &mut Asked,
    pane: usize,
    canvas: Rect,
    active: bool,
) {
    let tools = &crate::tools::strip().tools;
    let zone = gizmo_zone(canvas);
    let Some(mut plan) = plan(canvas, shell.tool_anchor(), tools.len(), zone) else { return };
    let painter = ui.painter_at(canvas);
    let pointer = ui.input(|i| i.pointer.latest_pos());
    let mut used = false;

    // In hand: it follows the pointer, kept inside the canvas; where it would land is outlined.
    if let (Some(grab), Some(pointer)) = (shell.tool_drag, pointer) {
        let most = (canvas.max - plan.rect.size()).max(canvas.min);
        plan = plan.moved_to((pointer - grab).clamp(canvas.min, most));
        if let Some(target) = self::plan(canvas, nearest(canvas, pointer), tools.len(), zone) {
            painter.rect_stroke(
                target.rect,
                4.0,
                Stroke::new(1.0, theme::SELECTION_EDGE),
                StrokeKind::Inside,
            );
        }
    }

    // Its ground takes every press on it, so that none reaches the view under it.
    ui.interact(plan.rect, egui::Id::unique("tool palette"), Sense::click_and_drag());
    painter.rect(
        plan.rect,
        4.0,
        theme::FOOTER.gamma_multiply(0.94),
        Stroke::new(1.0, theme::WIDGET_EDGE),
        StrokeKind::Inside,
    );

    let grip = ui
        .interact(plan.grip, egui::Id::unique("tool palette grip"), Sense::drag())
        .on_hover_text("drag to another edge or corner of the view");
    if grip.drag_started()
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
    {
        shell.tool_drag = Some(origin - plan.rect.min);
        shell.flyout = None;
        used = true;
    }
    let held = shell.tool_drag.is_some();
    if grip.drag_stopped() {
        if let Some(at) = pointer {
            let anchor = nearest(canvas, at);
            shell.tool_anchors.insert(shell.layout_name.clone(), anchor);
        }
        shell.tool_drag = None;
        ui.ctx().request_repaint();
    } else if held && !ui.input(|i| i.pointer.any_down()) {
        shell.tool_drag = None;
    }
    if held {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        ui.ctx().request_repaint();
    } else if grip.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    // The grip: two rows of dots along it.
    let dots = if grip.hovered() || held { theme::STRONG } else { theme::WEAK };
    let centre = plan.grip.center();
    for k in -2..=2 {
        for side in [-1.5, 1.5] {
            let offset = if plan.along { vec2(side, 4.0 * k as f32) } else { vec2(4.0 * k as f32, side) };
            if plan.grip.expand(0.5).contains(centre + offset) {
                painter.circle_filled(centre + offset, 0.9, dots);
            }
        }
    }

    let down_for = ui.input(|i| {
        let since = i.pointer.press_start_time().map(|start| i.time - start);
        since.filter(|_| i.pointer.primary_down())
    });
    let mut open: Option<(usize, Option<usize>)> = None;
    for (k, tool) in tools.iter().enumerate() {
        let rect = plan.buttons[k];
        let line = format!("tool {}", tool.id);
        let has_variants = tool.variants != Variants::None;
        let response = ui
            .interact(rect, egui::Id::unique(("tool palette", tool.id.as_str())), Sense::click_and_drag())
            .on_hover_text(tooltip(tool));
        let in_hand = shell.tool == tool.id;
        if in_hand {
            painter.rect_filled(rect, 3.0, theme::SELECTION);
        } else if response.hovered() || response.is_pointer_button_down_on() {
            painter.rect_filled(rect, 3.0, theme::HOVER);
        }
        let colour = if in_hand || response.hovered() { Color32::WHITE } else { theme::TEXT };
        // A tool with variants keeps its corner free for the mark: its icon is a little smaller
        // and stands up and to the left.
        let picture = if has_variants {
            Rect::from_min_size(rect.min + vec2(2.0, 2.0), vec2(theme::ICON - 8.0, theme::ICON - 8.0))
        } else {
            rect.shrink(3.0)
        };
        icons::tool(&painter, picture, tool.behaviour, colour);
        if has_variants {
            // The corner mark: this tool has more under it.
            let corner = rect.right_bottom() - vec2(1.0, 1.0);
            let mark = vec![corner, corner - vec2(4.0, 0.0), corner - vec2(0.0, 4.0)];
            let tint = if in_hand { Color32::WHITE } else { theme::WEAK_ON_WIDGET };
            painter.add(egui::Shape::convex_polygon(mark, tint, Stroke::NONE));
        }
        shell.layout.buttons.push((line.clone(), rect));

        // A press held opens the flyout; the click that ends that press takes no tool.
        if has_variants && response.is_pointer_button_down_on() && !held {
            match down_for {
                Some(since) if since >= HOLD => {
                    if shell.flyout.as_deref() != Some(tool.id.as_str()) {
                        shell.flyout = Some(tool.id.clone());
                        shell.flyout_held = true;
                        used = true;
                    }
                }
                Some(_) => ui.ctx().request_repaint(),
                None => {}
            }
        }
        if has_variants && response.secondary_clicked() {
            let same = shell.flyout.as_deref() == Some(tool.id.as_str());
            shell.flyout = (!same).then(|| tool.id.clone());
            used = true;
        }
        if response.clicked() {
            if shell.flyout_held {
                shell.flyout_held = false;
            } else {
                asked.lines.push(line);
                shell.flyout = None;
                used = true;
            }
        }
        // Its variants are open: by the pointer, or as the keyboard's drop-down (the row the
        // arrows are on lit).
        let stop = variants_stop(&tool.id);
        let by_keys = shell.keyboard.popup.as_ref().filter(|p| p.id == stop).map(|p| p.at);
        if has_variants && (by_keys.is_some() || shell.flyout.as_deref() == Some(tool.id.as_str())) {
            open = Some((k, by_keys));
        }
    }
    if !ui.input(|i| i.pointer.any_down()) {
        shell.flyout_held = false;
    }
    if let Some((k, lit)) = open.filter(|_| !held) {
        used |= flyout(shell, ui, asked, canvas, &plan, k, lit);
    }

    shell.layout.buttons.push(("tool palette".to_owned(), plan.rect));
    shell.layout.buttons.push(("tool palette grip".to_owned(), plan.grip));
    shell.layout.tool_host = Some(pane);
    asked.tools = Some(Drawn { rect: plan.rect, buttons: plan.buttons });
    // Using the palette makes its viewport the active one, as using its gizmo does.
    if used && !active {
        asked.commands.insert(0, Command::View(ViewCommand::Activate(pane)));
    }
}

/// Draws the flyout of tool `k` of the palette `plan`: its variants as rows (an icon and a
/// name; the one chosen now marked, the one the keyboard is on, `lit`, lit), beside the palette
/// on the side away from the canvas's nearer edge, inside the canvas (in more columns where one
/// is too long). A click on a row runs `tool <id> <variant>`; a press anywhere else closes it.
/// Returns whether a row was chosen.
fn flyout(
    shell: &mut Shell,
    ui: &egui::Ui,
    asked: &mut Asked,
    canvas: Rect,
    plan: &Plan,
    k: usize,
    lit: Option<usize>,
) -> bool {
    let tool = &crate::tools::strip().tools[k];
    let variants = tool.variants.list();
    if variants.is_empty() {
        return false;
    }
    let opener = plan.buttons[k];
    let now = chosen(shell, tool);
    let widest = variants
        .iter()
        .map(|v| ui.painter().layout_no_wrap(v.label.clone(), theme::body(), theme::TEXT).size().x)
        .fold(0.0, f32::max);
    let row = vec2(theme::ICON + widest.ceil() + 2.0 * theme::GRID, theme::HIT);
    let room = canvas.shrink(theme::GRID);
    let most = (((room.height() - 2.0 * PAD) / row.y).floor().max(1.0) as usize).min(variants.len());
    let columns = variants.len().div_ceil(most);
    let size = vec2(
        2.0 * PAD + columns as f32 * row.x + (columns - 1) as f32 * theme::GRID,
        2.0 * PAD + most as f32 * row.y,
    );
    let min = if plan.along {
        let above = plan.rect.center().y > canvas.center().y;
        let y = if above { plan.rect.top() - theme::GRID - size.y } else { plan.rect.bottom() + theme::GRID };
        pos2(opener.left() - PAD, y)
    } else {
        let left = plan.rect.center().x > canvas.center().x;
        let x = if left { plan.rect.left() - theme::GRID - size.x } else { plan.rect.right() + theme::GRID };
        pos2(x, opener.top() - PAD)
    };
    let last = (room.max - size).max(room.min);
    let rect = Rect::from_min_size(min.clamp(room.min, last).round(), size);

    let painter = ui.painter_at(canvas);
    ui.interact(rect, egui::Id::unique(("tool flyout", tool.id.as_str())), Sense::click_and_drag());
    painter.rect(rect, 4.0, theme::FOOTER, Stroke::new(1.0, theme::WIDGET_EDGE), StrokeKind::Inside);
    let mut chose = false;
    for (n, variant) in variants.iter().enumerate() {
        let (column, line) = ((n / most) as f32, (n % most) as f32);
        let at = rect.min + vec2(PAD + column * (row.x + theme::GRID), PAD + line * row.y);
        let cell = Rect::from_min_size(at, row);
        let response = ui
            .interact(cell, egui::Id::unique(("tool flyout", tool.id.as_str(), n)), Sense::click())
            .on_hover_text(&variant.about);
        let current = now.as_deref() == Some(variant.name.as_str());
        if lit == Some(n) {
            painter.rect_filled(cell, 2.0, theme::SELECTION);
        } else if response.hovered() {
            painter.rect_filled(cell, 2.0, theme::HOVER);
        }
        let colour = if lit == Some(n) || response.hovered() {
            Color32::WHITE
        } else if current {
            theme::GOLD
        } else {
            theme::TEXT
        };
        let square = Rect::from_center_size(
            pos2(cell.left() + 0.5 * theme::ICON, cell.center().y),
            vec2(theme::ICON - 8.0, theme::ICON - 8.0),
        );
        variant_icon(&painter, square, tool.variants, &variant.name, colour);
        painter.text(
            pos2(cell.left() + theme::ICON, cell.center().y),
            Align2::LEFT_CENTER,
            &variant.label,
            theme::body(),
            colour,
        );
        let line = format!("tool {} {}", tool.id, variant.name);
        shell.layout.buttons.push((line.clone(), cell));
        if response.clicked() {
            asked.lines.push(line);
            shell.flyout = None;
            chose = true;
        }
    }
    shell.layout.buttons.push((format!("flyout {}", tool.id), rect));
    // A press anywhere but in it or on its tool closes it.
    let pressed = ui.input(|i| i.pointer.any_pressed().then(|| i.pointer.interact_pos()).flatten());
    if pressed.is_some_and(|at| !rect.contains(at) && !opener.contains(at)) {
        shell.flyout = None;
    }
    chose
}

/// What a transform session's row says of its keys: the tooltip of its numbers.
const SESSION_HINT: &str =
    "arrows move · X Y Z lock · type a distance, Tab: next axis · Enter keeps · Esc cancels";

/// Draws the options row in `rect` (one row under the host's header): for Move, the snap switch
/// with its step; for Place, what it places (a drop-down of the starters) and the ground's
/// height; in a transform session, its move, its axis lock and the distance typed, as data.
/// Every text it paints is recorded in the layout's `chrome` under `options`, and none is a
/// sentence (those are tooltips, recorded as `options tooltip`). The row's rectangle is the
/// layout's `options`; a tool whose behaviour gains settings gets its arm here and in
/// [`Behaviour::has_options`].
pub(crate) fn options_row(shell: &mut Shell, ui: &mut egui::Ui, asked: &mut Asked, rect: Rect) {
    ui.painter().rect_filled(rect, 0.0, theme::PANEL);
    shell.layout.options = Some(rect);
    let inside = rect.shrink2(vec2(theme::GRID, 0.0));
    let builder = egui::UiBuilder::new()
        .id_salt("tool options")
        .max_rect(inside)
        .layout(egui::Layout::left_to_right(Align::Center));
    let mut row = ui.new_child(builder);
    row.set_clip_rect(rect.intersect(ui.clip_rect()));
    theme::dense(&mut row, 0.0);
    let ui = &mut row;
    let behaviour = shell.behaviour();
    let say = |shell: &mut Shell, ui: &mut egui::Ui, text: String, colour: Color32, hint: &str| {
        ui.label(RichText::new(&text).font(theme::mono()).color(colour)).on_hover_text(hint);
        shell.layout.chrome.push(("options", text));
    };
    if behaviour == Behaviour::Translate {
        let step = crate::tools::strip().snap_step;
        let on = shell.snap;
        let text = if on { format!("Snap {step} m") } else { "Snap off".to_owned() };
        let colour = if on { theme::GOLD } else { theme::WEAK };
        let hint = crate::header::tooltip("snap");
        shell.layout.chrome.push(("options", text.clone()));
        shell.layout.chrome.push(("options tooltip", hint.clone()));
        let snap = ui
            .add(Button::new(RichText::new(text).size(theme::SMALL_PT).color(colour)).selected(on))
            .on_hover_text(hint);
        shell.layout.buttons.push(("snap".to_owned(), snap.rect));
        asked.snap = snap.rect;
        if snap.clicked() {
            asked.lines.push("snap".to_owned());
        }
    }
    if let Some(session) = shell.session.clone() {
        shell.layout.chrome.push(("options tooltip", SESSION_HINT.to_owned()));
        let colour = crate::shell::RING_INTERACTION;
        let [dx, dy, dz] = session.offset;
        say(shell, ui, format!("Δ {dx:+.3} {dy:+.3} {dz:+.3} m"), colour, SESSION_HINT);
        if let Some(axis) = session.lock {
            say(shell, ui, format!("lock {}", ["x", "y", "z"][axis]), colour, SESSION_HINT);
        }
        if !session.typed.is_empty() {
            let axis = ["x", "y", "z"][session.lock.unwrap_or(session.typed_axis)];
            say(shell, ui, format!("{axis} = {} m", session.typed), colour, SESSION_HINT);
        }
        return;
    }
    if behaviour == Behaviour::Place {
        let starters = cav_sim::starters::starters();
        let current =
            starters.iter().find(|s| s.name == shell.place_starter).map_or("?", |s| s.label.as_str());
        let mut picked = None;
        let combo = egui::ComboBox::from_id_salt("place starter")
            .selected_text(current)
            .width(150.0)
            .show_ui(ui, |ui| {
                for starter in starters {
                    let row = ui
                        .selectable_label(starter.name == shell.place_starter, &starter.label)
                        .on_hover_text(&starter.about);
                    if row.clicked() {
                        picked = Some(starter.name.clone());
                    }
                }
            });
        let opener = combo
            .response
            .on_hover_text("what a click on the ground adds: the Place tool's flyout lists them too");
        shell.layout.chrome.push(("options", current.to_owned()));
        shell.layout.buttons.push(("place starter".to_owned(), opener.rect));
        if let Some(name) = picked {
            asked.lines.push(format!("tool place {name}"));
        }
        let ground = shell.ground();
        let hint = format!("the ground's height: a click on the ground (z = {ground} m) adds one there");
        shell.layout.chrome.push(("options tooltip", hint.clone()));
        say(shell, ui, format!("z {} m", crate::panels::short_number(ground)), theme::TEXT, &hint);
    }
}

/// Registers the palette's stops with the keyboard, under [`SCOPE`], and carries out what the
/// keys asked of them. The stops are the tools of `ui/tools.ron` in its order, each tool with
/// variants followed by a drop-down of them (Enter opens the flyout, the arrows choose, Enter
/// takes the tool with that variant, Esc closes it), and last the snap switch: the same stops
/// whatever is drawn (the order comes from data, `docs/design/keyboard.md`). A stop that is not
/// drawn now (no scene viewport has the palette; the options row does not show snap) is there
/// all the same, disabled.
pub(crate) fn register(shell: &mut Shell, drawn: Option<&Drawn>, snap: Rect) {
    if let Some(drawn) = drawn {
        shell.building.place(SCOPE, drawn.rect);
    }
    for (k, tool) in crate::tools::strip().tools.iter().enumerate() {
        let rect = drawn.map_or(Rect::NOTHING, |d| d.buttons[k]);
        let id = format!("{SCOPE}/{}", tool.id);
        shell.register(SCOPE, &id, &tool.label, Kind::Button, rect);
        if drawn.is_none() {
            shell.building.disable(&id);
        }
        if shell.take_click(&id) {
            let _ = shell.run(&format!("tool {}", tool.id));
        }
        if tool.variants == Variants::None {
            continue;
        }
        let variants = tool.variants.list();
        let now = chosen(shell, tool);
        let current = variants.iter().position(|v| Some(&v.name) == now.as_ref()).unwrap_or(0);
        let stop = variants_stop(&tool.id);
        let kind = Kind::Choice { count: variants.len(), current };
        shell.register(SCOPE, &stop, &format!("{} variants", tool.label), kind, rect);
        if drawn.is_none() {
            shell.building.disable(&stop);
        }
        if let Some(variant) = shell.take_pick(&stop).and_then(|index| variants.get(index)) {
            let _ = shell.run(&format!("tool {} {}", tool.id, variant.name));
        }
    }
    let id = format!("{SCOPE}/snap");
    shell.register(SCOPE, &id, "Snap", Kind::Button, snap);
    if !snap.is_positive() {
        shell.building.disable(&id);
    }
    if shell.take_click(&id) {
        let _ = shell.run("snap");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(width: f32, height: f32) -> Rect {
        Rect::from_min_size(pos2(100.0, 50.0), vec2(width, height))
    }

    #[test]
    fn the_palette_stands_at_each_anchor_inside_the_canvas_and_clear_of_the_gizmo() {
        let canvas = canvas(800.0, 600.0);
        let zone = gizmo_zone(canvas).expect("a canvas this size has a gizmo");
        let room = canvas.shrink(MARGIN);
        for anchor in Anchor::ALL {
            let plan = plan(canvas, anchor, 4, Some(zone)).expect("it fits");
            assert!(room.contains_rect(plan.rect), "{anchor:?}: {:?}", plan.rect);
            // (Touching it is not covering it: the zone has its own step of the grid round it.)
            assert!(!plan.rect.intersects(zone.shrink(0.01)), "{anchor:?} covers the gizmo");
            assert_eq!((plan.lanes, plan.along), (1, anchor.along()), "{anchor:?}");
            assert!(plan.rect.contains_rect(plan.grip));
            for (k, button) in plan.buttons.iter().enumerate() {
                assert_eq!(button.size(), vec2(theme::ICON, theme::ICON));
                assert!(plan.rect.shrink(PAD - 0.01).contains_rect(*button), "{anchor:?}: button {k}");
                assert!(!button.intersects(plan.grip), "{anchor:?}: button {k} under the grip");
                for other in &plan.buttons[..k] {
                    assert!(!button.intersects(*other), "{anchor:?}: two buttons overlap");
                }
            }
            assert_eq!(Anchor::parse(anchor.name()), Some(anchor));
            // The anchor nearest the palette's own middle is the one it stands at, but under
            // the gizmo, where it was moved down.
            if anchor != Anchor::TopRight {
                assert_eq!(nearest(canvas, plan.rect.center()), anchor);
            }
        }
        // At the top right it stands under the gizmo, at the right edge.
        let under = plan(canvas, Anchor::TopRight, 4, Some(zone)).unwrap();
        assert!(under.rect.top() >= zone.bottom() && (under.rect.right() - room.right()).abs() < 1.0);
        // Control: with nothing to keep clear of it is in the corner, where the gizmo is.
        let corner = plan(canvas, Anchor::TopRight, 4, None).unwrap();
        assert!(corner.rect.intersects(zone) && (corner.rect.top() - room.top()).abs() < 1.0);
    }

    #[test]
    fn a_short_canvas_takes_two_lanes_and_a_tiny_one_none() {
        let column = plan(canvas(400.0, 600.0), Anchor::Left, 4, None).unwrap();
        let tall = column.rect.height();
        assert_eq!(column.lanes, 1);
        // A canvas just too short for the column: two columns of two.
        let short = canvas(400.0, tall + 2.0 * MARGIN - 1.0);
        let two = plan(short, Anchor::Left, 4, None).unwrap();
        assert_eq!(two.lanes, 2);
        assert!(short.shrink(MARGIN).contains_rect(two.rect) && two.rect.height() < tall);
        assert!((two.buttons[0].top() - two.buttons[1].top()).abs() < 0.01, "the first two side by side");
        assert!(two.buttons[2].top() > two.buttons[0].bottom());
        // Control: a point taller, one column.
        assert_eq!(plan(canvas(400.0, tall + 2.0 * MARGIN), Anchor::Left, 4, None).unwrap().lanes, 1);
        // No room at all: no palette.
        assert!(plan(canvas(400.0, 40.0), Anchor::Left, 4, None).is_none());
        assert!(plan(canvas(30.0, 400.0), Anchor::Left, 4, None).is_none());
    }
}
