//! A viewport's header (`docs/design/workspace.md` §2): one compact row inside the viewport,
//! above its canvas. Left to right: the view type, then what applies to it (a scene's camera
//! menu, its shading as a segmented group and its overlays; a plot's "show all"), and at the
//! right a warning dot, maximise, pop out and close.
//!
//! The header is made from data. Which controls there are, in which order and for which view
//! types is [`CONTROLS`]; the view types are [`PaneContent::ALL`]; the rows of the camera menu
//! and of the overlays popover are actions of `ui/actions.ron` (their menus are named in
//! [`CAMERA_MENUS`] and [`OVERLAY_MENUS`]). Every control runs a console line on its own
//! viewport (`show periodogram @3`), so nothing here is a second path into the engine. No status
//! is drawn here: that is the status bar's, and a viewport with something to warn of shows one
//! dot whose tooltip holds the words.
//!
//! The keyboard reaches the active viewport's header as the scope `viewport/header`, whose stops
//! are [`CONTROLS`] in that order whatever the viewport shows (a stop that does not apply is
//! disabled, never left out: the order comes from data, `docs/design/keyboard.md`).

use cav_engine::{Command, PaneContent, Request, ViewCommand};
use cav_sim::{DrawStyle, Projection};
use egui::{Align2, Color32, Painter, Pos2, Rect, RichText, Sense, Stroke, pos2, vec2};

use crate::actions::{Action, table};
use crate::keyboard::Kind;
use crate::panels::Asked;
use crate::shell::{HeaderControl, Shell};
use crate::{icons, theme};

/// Which view types a header control is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applies {
    /// Every viewport.
    Any,
    /// A viewport that shows the scene (it has a camera).
    Scene,
    /// A viewport that shows a plot (it has a horizontal axis).
    Plot,
}

impl Applies {
    /// Whether a control for these applies to a viewport showing `content`.
    #[must_use]
    pub fn to(self, content: PaneContent) -> bool {
        match self {
            Self::Any => true,
            Self::Scene => content == PaneContent::Scene,
            Self::Plot => content != PaneContent::Scene,
        }
    }
}

/// One control of a viewport's header, as data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Control {
    /// Its id: the keyboard's stop is `viewport/header/<id>`.
    pub id: &'static str,
    /// What the breadcrumb calls it.
    pub name: &'static str,
    /// The view types it is drawn for.
    pub applies: Applies,
    /// The action of `ui/actions.ron` it runs; `None` for a control that opens a menu.
    pub action: Option<&'static str>,
}

/// The header's controls, left to right; the last three stand at the header's right end. The
/// view type is the first, always.
pub const CONTROLS: [Control; 8] = [
    Control { id: "type", name: "View type", applies: Applies::Any, action: None },
    Control { id: "camera", name: "Camera", applies: Applies::Scene, action: None },
    Control { id: "shading", name: "Shading", applies: Applies::Scene, action: None },
    Control { id: "overlays", name: "Overlays", applies: Applies::Scene, action: None },
    Control { id: "span", name: "Show all", applies: Applies::Plot, action: Some("span-all") },
    Control { id: "maximise", name: "Maximise", applies: Applies::Any, action: Some("maximise") },
    Control { id: "pop-out", name: "Pop out", applies: Applies::Any, action: Some("viewport-pop-out") },
    Control { id: "close", name: "Close", applies: Applies::Any, action: Some("viewport-close") },
];

/// The draw styles of the segmented group, in its order. Paper is not among them: it is a look
/// for a figure to be printed, reached from View ▸ Draw style.
pub const SHADING: [DrawStyle; 4] =
    [DrawStyle::Shaded, DrawStyle::Flat, DrawStyle::XRay, DrawStyle::Wireframe];

/// The menus of `ui/actions.ron` whose actions are the camera menu's rows, in this order, with
/// a rule between them; then the camera's mode (orbit, fly, walk), noclip, Frame and Home.
pub const CAMERA_MENUS: [&str; 2] = ["View/Projection", "View/Look from"];

/// The menus of `ui/actions.ron` whose actions are the overlays popover's rows, each under its
/// heading; then the section plane across x, y or z.
pub const OVERLAY_MENUS: [(&str, &str); 4] = [
    ("Draw bodies", "View/Draw bodies"),
    ("Colour voxels by", "View/Colour voxels by"),
    ("Overlay", "View/Overlay"),
    ("World LOD", "View/World LOD"),
];

/// The keyboard's scope for the active viewport's header.
pub const SCOPE: &str = "viewport/header";

/// The width of a drop-down's mark and of the room round a control's words, points.
const CHEVRON: f32 = 10.0;
/// The gap between two controls of the left group, points.
const GAP: f32 = 2.0;
/// The width a warning dot takes at the right group's left, points.
const DOT: f32 = 14.0;

/// What a control's tooltip says for the action `id`: its name, its key, and what it does.
///
/// # Panics
/// When `id` is not an action of the table (the header's are named in [`CONTROLS`], which
/// `every_header_control_says_what_it_does` holds to the table).
#[must_use]
pub fn tooltip(id: &str) -> String {
    let action = table().action(id).unwrap_or_else(|| panic!("`{id}` is an action of ui/actions.ron"));
    tooltip_of(&action.label, &action.shortcut_text(), &action.about)
}

/// A tooltip's words from a control's name, its key (empty for none) and what it does.
fn tooltip_of(label: &str, key: &str, about: &str) -> String {
    let head = if key.is_empty() { label.to_owned() } else { format!("{label} ({key})") };
    if about.is_empty() { head } else { format!("{head}\n{about}") }
}

/// `line` addressed to pane `pane` (from 0): an engine's view command takes `@N`; the window's
/// own commands act on the active viewport, which a click on a header makes its own first.
fn on_pane(line: &str, pane: usize) -> String {
    if Shell::is_app_command(line) { line.to_owned() } else { format!("{line} @{}", pane + 1) }
}

/// A view type's name as a header writes it: `Scene`, `Periodogram`.
#[must_use]
pub fn title(content: PaneContent) -> String {
    let name = content.name();
    let mut letters = name.chars();
    letters.next().map_or_else(String::new, |first| first.to_uppercase().chain(letters).collect())
}

/// One row of a header's menu: an action's line on that viewport.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row {
    /// What the row says.
    label: String,
    /// The console line it runs.
    line: String,
    /// Its key, written at the row's right; empty for none.
    key: String,
    /// What it does, for its tooltip.
    about: String,
    /// For one of a set of choices, whether it is the one chosen now (drawn as a radio button);
    /// `None` for a command.
    chosen: Option<bool>,
    /// Why it cannot run now, when it cannot.
    why_not: Option<&'static str>,
    /// A rule above it.
    rule: bool,
    /// A heading above it.
    heading: Option<&'static str>,
    /// A switch (`chosen` says whether it is on): it runs when on too, to turn off.
    switch: bool,
}

/// Whether `line`, run on pane `pane`, would leave the view as it is: the row that says it is
/// the one chosen now. `None` for a line that is a command, not a choice (`view home`, `proj
/// toggle`, `frame`).
fn chosen(shell: &Shell, pane: usize, line: &str) -> Option<bool> {
    if Shell::is_app_command(line) || line.ends_with(" toggle") {
        return None;
    }
    let view = shell.engine.view(pane);
    match shell.engine.parse(&on_pane(line, pane)).ok()? {
        Request::Do(Command::View(command)) => match command {
            ViewCommand::Look { view: named, .. } => Some(view.named() == Some(named)),
            ViewCommand::Project { projection, .. } => Some(view.projection == projection),
            ViewCommand::Style { style, .. } => Some(view.style == style),
            ViewCommand::Colour { colour, .. } => Some(view.colour == colour),
            ViewCommand::Overlay { overlay, .. } => Some(view.overlay == overlay),
            ViewCommand::Draw { draw, .. } => Some(view.body_draw == draw),
            ViewCommand::LodView { view: ref lod, .. } => Some(view.lod_view == *lod),
            ViewCommand::Section { section, .. } => {
                Some(view.section.map(|s| s.axis) == section.map(|s| s.axis))
            }
            ViewCommand::Show { content, .. } => Some(shell.engine.content(pane) == content),
            _ => None,
        },
        _ => None,
    }
}

/// The row of `action` for pane `pane`.
fn action_row(shell: &Shell, pane: usize, action: &Action) -> Row {
    Row {
        label: action.label.clone(),
        line: on_pane(&action.line, pane),
        key: action.shortcut_text(),
        about: action.about.clone(),
        chosen: chosen(shell, pane, &action.line),
        why_not: (!shell.can_run(action)).then(|| action.needs.why_not()),
        rule: false,
        heading: None,
        switch: false,
    }
}

/// The rows of the actions in the menu `menu` of the table.
fn menu_rows(shell: &Shell, pane: usize, menu: &str) -> Vec<Row> {
    table().actions.iter().filter(|a| a.menu == menu).map(|a| action_row(shell, pane, a)).collect()
}

/// The camera menu's rows for pane `pane`: the projection, the named views, the camera's mode
/// (orbit about the target, fly from the eye, or walk), the noclip switch, Frame and Home.
fn camera_rows(shell: &Shell, pane: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    for menu in CAMERA_MENUS {
        let mut group = menu_rows(shell, pane, menu);
        if let Some(first) = group.first_mut() {
            first.rule = !rows.is_empty();
        }
        rows.append(&mut group);
    }
    // The camera's mode: orbit, or one of the two first-person ones; then noclip, a switch.
    let (first_person, walking) = (shell.any_fly(), shell.walking());
    if let (Some(fly), Some(walk)) = (table().action("fly"), table().action("walk")) {
        let (fly, walk) = (action_row(shell, pane, fly), action_row(shell, pane, walk));
        rows.push(Row {
            label: "orbit".to_owned(),
            key: String::new(),
            about: "a drag turns the camera about the point it looks at".to_owned(),
            chosen: Some(!first_person),
            rule: true,
            // The word that ends the session under way.
            ..if walking { walk.clone() } else { fly.clone() }
        });
        rows.push(Row { label: "fly".to_owned(), chosen: Some(first_person && !walking), ..fly });
        rows.push(Row { label: "walk".to_owned(), chosen: Some(walking), ..walk });
    }
    if let Some(noclip) = table().action("noclip") {
        rows.push(Row {
            label: "noclip".to_owned(),
            chosen: Some(shell.noclip),
            switch: true,
            ..action_row(shell, pane, noclip)
        });
    }
    for (n, id) in ["frame", "view-home"].into_iter().enumerate() {
        if let Some(action) = table().action(id) {
            rows.push(Row { rule: n == 0, ..action_row(shell, pane, action) });
        }
    }
    rows
}

/// The overlays popover's rows for pane `pane`: what colours the voxel bodies, what is outlined
/// over them, and the section plane (off, or across an axis through the middle of the voxel
/// bodies' span along it).
fn overlay_rows(shell: &Shell, pane: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    for (heading, menu) in OVERLAY_MENUS {
        let mut group = menu_rows(shell, pane, menu);
        if let Some(first) = group.first_mut() {
            first.heading = Some(heading);
        }
        rows.append(&mut group);
    }
    // The table's `section off` closes its Overlay menu; here it opens the section's own group.
    if let Some(off) = rows.iter_mut().find(|row| row.line.starts_with("section off")) {
        off.rule = false;
        off.heading = Some("Section plane");
    }
    let spec = shell.engine.spec();
    let time = shell.engine.time();
    let shapes = spec.bodies.iter().any(|b| b.shape.is_some());
    for (axis, word) in [(cav_sim::Axis::X, "x"), (cav_sim::Axis::Y, "y"), (cav_sim::Axis::Z, "z")] {
        let (lo, hi) = cav_sim::voxel_view::section_span(spec, axis, time).unwrap_or((0.0, 0.0));
        let line = format!("section {word} {}", 0.5 * (lo + hi));
        rows.push(Row {
            label: format!("across {word}"),
            chosen: chosen(shell, pane, &line),
            line: on_pane(&line, pane),
            key: String::new(),
            about: format!("cut the voxel bodies by a plane square to {word}, through their middle"),
            why_not: (!shapes).then_some("the scene has no voxel body"),
            rule: false,
            heading: None,
            switch: false,
        });
    }
    rows
}

/// The view types as rows: one for each of [`PaneContent::ALL`].
fn type_rows(shell: &Shell, pane: usize) -> Vec<Row> {
    let shown = shell.engine.content(pane);
    PaneContent::ALL
        .into_iter()
        .map(|content| {
            let line = format!("show {}", content.name());
            let about = table().actions.iter().find(|a| a.line == line).map(|a| a.about.clone());
            Row {
                label: title(content),
                line: on_pane(&line, pane),
                key: String::new(),
                about: about.unwrap_or_default(),
                chosen: Some(content == shown),
                why_not: None,
                rule: false,
                heading: None,
                switch: false,
            }
        })
        .collect()
}

/// The shading group's rows: one for each of [`SHADING`].
fn shading_rows(shell: &Shell, pane: usize) -> Vec<Row> {
    SHADING
        .into_iter()
        .filter_map(|style| table().actions.iter().find(|a| a.line == format!("style {}", style.name())))
        .map(|action| action_row(shell, pane, action))
        .collect()
}

/// A header to draw: whose it is and where.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Head {
    /// The pane, from 0.
    pub pane: usize,
    /// The header's row, points.
    pub rect: Rect,
    /// Whether its viewport is the active one.
    pub active: bool,
    /// How many viewports the figure has.
    pub count: usize,
    /// Whether its viewport floats over the window.
    pub floating: bool,
}

/// What a header's own pointer work leaves for its caller.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Outcome {
    /// How far the header's empty stretch was dragged this frame, points: a floating viewport's
    /// frame follows it.
    pub dragged: egui::Vec2,
}

/// One stop of the keyboard's `viewport/header` scope, as the active header drew it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Stop {
    /// The control's id (of [`CONTROLS`]).
    pub id: &'static str,
    /// What Enter does on it.
    pub kind: Kind,
    /// Where it is.
    pub rect: Rect,
    /// Whether it can be used now.
    pub enabled: bool,
}

/// How a control looks.
#[derive(Clone, Copy, Default)]
struct Look<'a> {
    /// Words after the icon.
    label: Option<&'a str>,
    /// A drop-down's mark at its right.
    chevron: bool,
    /// Lit: the one of a group that is on.
    selected: bool,
    /// Greyed, and not to be clicked.
    disabled: bool,
}

/// The width of a control with these words (`None`: its icon alone) and a drop-down's mark or
/// not, points.
fn width_of(label: Option<f32>, chevron: bool) -> f32 {
    theme::ICON + label.map_or(0.0, |w| w + 4.0) + if chevron { CHEVRON } else { 0.0 }
}

/// Draws one control of pane `pane`'s header at `rect`, cut to `clip` (a narrow viewport's left
/// group ends where its right group begins), and records it. `None` when nothing of it is left
/// to aim at.
#[allow(clippy::too_many_arguments)]
fn control(
    shell: &mut Shell,
    ui: &egui::Ui,
    pane: usize,
    id: &str,
    rect: Rect,
    clip: Rect,
    tooltip: &str,
    action: Option<&str>,
    look: Look<'_>,
    icon: impl FnOnce(&Painter, Rect, Color32),
) -> Option<egui::Response> {
    let seen = rect.intersect(clip);
    if seen.width() < 0.5 * theme::ICON || seen.height() <= 0.0 {
        return None;
    }
    let sense = if look.disabled { Sense::hover() } else { Sense::click() };
    let response = ui
        .interact(seen, egui::Id::unique(("viewport header", pane, id.to_owned())), sense)
        .on_hover_text(tooltip);
    let painter = ui.painter_at(seen);
    let inner = rect.shrink2(vec2(0.0, 1.0));
    if look.selected {
        painter.rect_filled(inner, 2.0, theme::SELECTION);
    } else if response.hovered() && !look.disabled {
        painter.rect_filled(inner, 2.0, theme::HOVER);
    }
    let colour = if look.disabled {
        theme::WEAK.gamma_multiply(0.45)
    } else if look.selected || response.hovered() {
        Color32::WHITE
    } else {
        theme::TEXT
    };
    let square = Rect::from_center_size(
        pos2(rect.left() + 0.5 * theme::ICON, rect.center().y),
        vec2(theme::ICON - 8.0, theme::ICON - 8.0),
    );
    icon(&painter, square, colour);
    if let Some(label) = look.label {
        painter.text(
            pos2(rect.left() + theme::ICON, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            theme::body(),
            colour,
        );
        shell.layout.chrome.push(("header", label.to_owned()));
    }
    if look.chevron {
        let at = pos2(rect.right() - 0.5 * CHEVRON, rect.center().y);
        icons::chevron(&painter, at, 6.0, theme::WEAK);
    }
    shell.layout.controls.push(HeaderControl {
        pane,
        id: id.to_owned(),
        rect: seen,
        tooltip: tooltip.to_owned(),
        action: action.map(str::to_owned),
        enabled: !look.disabled,
    });
    Some(response)
}

/// Records and draws one more control of a gizmo or a header that is not laid out by
/// [`draw`] (the navigation gizmo's buttons): the same look, tooltip and record.
#[allow(clippy::too_many_arguments)]
pub(crate) fn extra_control(
    shell: &mut Shell,
    ui: &egui::Ui,
    pane: usize,
    id: &str,
    rect: Rect,
    tooltip: &str,
    action: Option<&str>,
    sense: Sense,
    icon: impl FnOnce(&Painter, Rect, Color32),
) -> egui::Response {
    let response = ui
        .interact(rect, egui::Id::unique(("viewport gizmo", pane, id.to_owned())), sense)
        .on_hover_text(tooltip);
    let painter = ui.painter_at(rect);
    let held = response.dragged() || response.is_pointer_button_down_on();
    let ground = if held {
        theme::SELECTION
    } else if response.hovered() {
        theme::HOVER
    } else {
        theme::FOOTER.gamma_multiply(0.8)
    };
    painter.rect_filled(rect, 3.0, ground);
    let colour = if held || response.hovered() { Color32::WHITE } else { theme::TEXT };
    icon(&painter, rect.shrink(3.0), colour);
    shell.layout.controls.push(HeaderControl {
        pane,
        id: id.to_owned(),
        rect,
        tooltip: tooltip.to_owned(),
        action: action.map(str::to_owned),
        enabled: true,
    });
    response
}

/// Runs a menu's row on its viewport, which becomes the active one; a row that cannot run, or
/// is already the one chosen, runs nothing.
fn run_row(asked: &mut Asked, head: Head, row: &Row) {
    activate(asked, head);
    if row.why_not.is_none() && (row.switch || row.chosen != Some(true)) {
        asked.lines.push(row.line.clone());
    }
}

/// A click on a header makes its viewport the active one, as a click in its canvas does.
fn activate(asked: &mut Asked, head: Head) {
    if !head.active {
        asked.commands.push(Command::View(ViewCommand::Activate(head.pane)));
    }
}

/// Draws a menu's rows into an open menu or popover; `at` lights the row the keyboard is on.
/// A row clicked is run ([`run_row`]); returns whether one was.
fn rows_ui(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    asked: &mut Asked,
    head: Head,
    rows: &[Row],
    at: Option<usize>,
) -> bool {
    let mut ran = false;
    for (n, row) in rows.iter().enumerate() {
        if row.rule {
            ui.separator();
        }
        if let Some(heading) = row.heading {
            if n > 0 {
                ui.add_space(theme::GRID);
            }
            ui.label(RichText::new(heading).size(theme::SMALL_PT).color(theme::WEAK));
        }
        let lit = at == Some(n);
        let response = match row.chosen {
            Some(on) => {
                let words = if row.key.is_empty() {
                    row.label.clone()
                } else {
                    format!("{}   {}", row.label, row.key)
                };
                let mut text = RichText::new(words);
                if lit {
                    text = text.background_color(theme::SELECTION).color(Color32::WHITE);
                }
                if row.switch {
                    let mut on = on;
                    ui.add_enabled(row.why_not.is_none(), egui::Checkbox::new(&mut on, text))
                } else {
                    ui.add_enabled(row.why_not.is_none(), egui::RadioButton::new(on, text))
                }
            }
            None => {
                let button = egui::Button::new(&row.label).shortcut_text(&row.key).selected(lit);
                ui.add_enabled(row.why_not.is_none(), button)
            }
        };
        let response = match row.why_not {
            Some(why) => response.on_disabled_hover_text(format!("{}\n{why}", row.about)),
            None => response.on_hover_text(&row.about),
        };
        shell.layout.buttons.push((row.line.clone(), response.rect));
        if response.clicked() {
            run_row(asked, head, row);
            ran = true;
        }
    }
    ran
}

/// The menu of `rows` of the control `stop`: by the pointer, under `opener` when it is clicked
/// (a control with no opener, the shading's four buttons, has none); and, for the active
/// viewport, by the keyboard: while its drop-down is open the same rows stand at `anchor` with
/// the one the arrows are on lit, and Enter runs it. `stay` keeps a popover open after a click
/// in it (the overlays: several things are set at once). `foot` draws what follows the rows.
#[allow(clippy::too_many_arguments)]
fn menu(
    shell: &mut Shell,
    ui: &egui::Ui,
    asked: &mut Asked,
    head: Head,
    stop: &str,
    opener: Option<&egui::Response>,
    anchor: Pos2,
    rows: &[Row],
    stay: bool,
    foot: &dyn Fn(&mut Shell, &mut egui::Ui, &mut Asked),
) {
    if let Some(opener) = opener {
        // Asked open by `viewport <menu> [N]` (a script, a picture): once, as a click opens it.
        let asked_open =
            shell.header_menu.as_ref().is_some_and(|(pane, menu)| *pane == head.pane && menu == stop);
        if asked_open {
            shell.header_menu = None;
        }
        let toggle = if asked_open {
            Some(egui::SetOpenCommand::Bool(true))
        } else {
            opener.clicked().then_some(egui::SetOpenCommand::Toggle)
        };
        if opener.clicked() {
            activate(asked, head);
        }
        let behaviour = if stay {
            egui::PopupCloseBehavior::CloseOnClickOutside
        } else {
            egui::PopupCloseBehavior::CloseOnClick
        };
        egui::Popup::menu(opener).open_memory(toggle).close_behavior(behaviour).show(|ui| {
            rows_ui(shell, ui, asked, head, rows, None);
            foot(shell, ui, asked);
        });
    }
    if !head.active {
        return;
    }
    let id = format!("{SCOPE}/{stop}");
    if let Some(at) = shell.keyboard.popup.as_ref().filter(|p| p.id == id).map(|p| p.at) {
        egui::Area::new(egui::Id::unique(("keyboard header menu", stop.to_owned())))
            .order(egui::Order::Foreground)
            .fixed_pos(anchor)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    rows_ui(shell, ui, asked, head, rows, Some(at));
                });
            });
    }
    if let Some(row) = shell.take_pick(&id).and_then(|index| rows.get(index)) {
        run_row(asked, head, row);
    }
}

/// What pane `pane`'s viewport has to warn of, in words: for a plot of the signal, a signal
/// that is stale or did not settle; for the scene, what the renderer thinned to keep it
/// drawable, what it left out of the last picture, and what the voxel view drew in place of the
/// leaves.
#[must_use]
pub fn warnings(shell: &Shell, pane: usize) -> Vec<String> {
    let mut notes = Vec::new();
    match shell.engine.content(pane) {
        PaneContent::Scene => {
            notes.extend(cav_render::decimation(shell.engine.spec()));
            notes.extend(shell.render_notes.iter().cloned());
            if let Ok(frame) = shell.engine.frame(pane)
                && let Some(voxels) = frame.voxels
            {
                notes.extend(voxels.notes.iter().cloned());
            }
        }
        PaneContent::Signal | PaneContent::Periodogram => match shell.engine.signal() {
            (cav_engine::SignalState::Stale, _) => {
                notes.push("the signal is stale: it is being computed for the scene as it is now".to_owned());
            }
            (cav_engine::SignalState::Refined, Some(signal)) if !signal.settled => {
                notes.push(format!(
                    "the signal {}",
                    cav_engine::estimate_text(signal.estimate, signal.settled, signal.truncated)
                ));
            }
            _ => {}
        },
        PaneContent::Flight | PaneContent::Cadence => {}
    }
    notes
}

/// Draws pane `head.pane`'s header in `head.rect` and takes its input. What its controls ask of
/// the engine goes into `asked` (a console line each, run once every viewport is drawn); the
/// active viewport's also leaves the keyboard's stops there.
#[allow(clippy::too_many_lines)]
pub(crate) fn draw(shell: &mut Shell, ui: &egui::Ui, asked: &mut Asked, head: Head) -> Outcome {
    let Head { pane, rect, active, count, floating } = head;
    let n = pane + 1;
    let content = shell.engine.content(pane);
    let scene = content == PaneContent::Scene;
    let ground = if active && count > 1 { theme::HEADER_ACTIVE } else { theme::FOOTER };
    ui.painter().rect_filled(rect, 0.0, ground);
    shell.layout.headers.push((pane, rect));
    let mut outcome = Outcome::default();
    let mut stops: Vec<Stop> = Vec::new();

    // The empty stretch of the row, under its controls: a click makes the viewport the active
    // one, a double click maximises it, a drag moves a floating viewport's frame or carries a
    // docked one to the viewport it is to swap places with.
    let bar = ui.interact(rect, egui::Id::unique(("viewport header bar", pane)), Sense::click_and_drag());
    if bar.clicked() || bar.drag_started() {
        activate(asked, head);
    }
    if bar.double_clicked() {
        asked.lines.push("maximise".to_owned());
    }
    if bar.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        if floating {
            outcome.dragged = bar.drag_delta();
        } else if count > 1 && !shell.maximised {
            shell.header_drag = Some(pane);
        }
    } else if bar.hovered() && (floating || count > 1) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    if bar.drag_stopped() && shell.header_drag.take() == Some(pane) {
        asked.dropped =
            bar.interact_pointer_pos().or_else(|| ui.input(|i| i.pointer.latest_pos())).map(|at| (pane, at));
    }
    crate::panels::viewport_menu(shell, asked, &bar);

    // The right group: close, pop out or dock, maximise or restore, and the warning dot.
    let side = theme::ICON;
    let slot =
        |k: f32| Rect::from_min_size(pos2(rect.right() - k * side, rect.top()), vec2(side, rect.height()));
    let notes = warnings(shell, pane);
    let right = rect.right() - 3.0 * side - if notes.is_empty() { 0.0 } else { DOT };
    let whole = rect;

    let only = count < 2;
    let close_tip = if only {
        format!("{}\nthe last viewport cannot be closed", tooltip("viewport-close"))
    } else {
        tooltip("viewport-close")
    };
    let look = Look { disabled: only, ..Look::default() };
    let close = control(
        shell,
        ui,
        pane,
        "close",
        slot(1.0),
        whole,
        &close_tip,
        Some("viewport-close"),
        look,
        icons::close,
    );
    let close_rect = close.as_ref().map_or(Rect::NOTHING, |r| r.rect);
    shell.layout.buttons.push((format!("viewport close {n}"), close_rect));
    stops.push(Stop { id: "close", kind: Kind::Button, rect: close_rect, enabled: !only });
    if close.is_some_and(|r| r.clicked()) || (active && shell.take_click(&format!("{SCOPE}/close")) && !only)
    {
        asked.lines.push(format!("viewport close {n}"));
    }

    let (pop_line, pop_name, pop_tip) = if floating {
        (
            format!("viewport dock {n}"),
            format!("float dock {n}"),
            tooltip_of("Dock", "", "put this viewport back in its place in the mosaic"),
        )
    } else {
        (format!("viewport pop-out {n}"), format!("viewport pop-out {n}"), tooltip("viewport-pop-out"))
    };
    let pop = control(
        shell,
        ui,
        pane,
        "pop-out",
        slot(2.0),
        whole,
        &pop_tip,
        Some("viewport-pop-out"),
        Look::default(),
        |painter, rect, colour| icons::pop_out(painter, rect, floating, colour),
    );
    let pop_rect = pop.as_ref().map_or(Rect::NOTHING, |r| r.rect);
    shell.layout.buttons.push((pop_name, pop_rect));
    stops.push(Stop { id: "pop-out", kind: Kind::Button, rect: pop_rect, enabled: true });
    if pop.is_some_and(|r| r.clicked()) || (active && shell.take_click(&format!("{SCOPE}/pop-out"))) {
        asked.lines.push(pop_line);
    }

    let restore = shell.maximised;
    let max_tip = if restore {
        let key = table().action("maximise").map(Action::shortcut_text).unwrap_or_default();
        tooltip_of("Restore", &key, "bring back the panels and the other viewports")
    } else {
        tooltip("maximise")
    };
    let max = control(
        shell,
        ui,
        pane,
        "maximise",
        slot(3.0),
        whole,
        &max_tip,
        Some("maximise"),
        Look { disabled: floating, ..Look::default() },
        |painter, rect, colour| icons::maximise(painter, rect, restore, colour),
    );
    let max_rect = max.as_ref().map_or(Rect::NOTHING, |r| r.rect);
    shell.layout.buttons.push((format!("viewport maximise {n}"), max_rect));
    stops.push(Stop { id: "maximise", kind: Kind::Button, rect: max_rect, enabled: !floating });
    if max.is_some_and(|r| r.clicked())
        || (active && shell.take_click(&format!("{SCOPE}/maximise")) && !floating)
    {
        activate(asked, head);
        asked.lines.push("maximise".to_owned());
    }

    if !notes.is_empty() {
        let dot = Rect::from_min_size(pos2(right, rect.top()), vec2(DOT, rect.height()));
        ui.interact(dot, egui::Id::unique(("viewport header dot", pane)), Sense::hover())
            .on_hover_text(notes.join("\n"));
        ui.painter().circle_filled(dot.center(), 4.0, theme::WARN);
        shell.layout.warnings.push((pane, dot, notes));
    }

    // The left group, cut where the right group begins. Words go first, then the shading's four
    // buttons fold into one drop-down, as the viewport narrows.
    let clip = Rect::from_min_max(rect.min, pos2((right - GAP).max(rect.left()), rect.bottom()));
    let measure =
        |text: &str| ui.painter().layout_no_wrap(text.to_owned(), theme::body(), theme::TEXT).size().x;
    let type_name = title(content);
    let projection =
        if shell.engine.view(pane).projection == Projection::Perspective { "Persp" } else { "Ortho" };
    let widths = |level: usize| -> [f32; 4] {
        let kind = width_of((level < 2).then(|| measure(&type_name)), true);
        if scene {
            [
                kind,
                width_of((level < 1).then(|| measure(projection)), true),
                if level < 3 { SHADING.len() as f32 * side } else { width_of(None, true) },
                width_of((level < 1).then(|| measure("Overlays")), true),
            ]
        } else {
            [kind, side, 0.0, 0.0]
        }
    };
    let fits = |level: usize| widths(level).iter().sum::<f32>() + 4.0 * GAP <= clip.width();
    let level = (0..3).find(|&level| fits(level)).unwrap_or(3);
    let [type_w, second_w, shading_w, overlays_w] = widths(level);
    let mut x = rect.left() + GAP;
    let mut next = |width: f32| {
        let at = Rect::from_min_size(pos2(x, rect.top()), vec2(width, rect.height()));
        x += width + GAP;
        at
    };

    // The view type: the header's first control.
    let rows = type_rows(shell, pane);
    let at = next(type_w);
    let look = Look { label: (level < 2).then_some(type_name.as_str()), chevron: true, ..Look::default() };
    let tip = tooltip_of("View type", "", "what this viewport shows: the scene, or a plot of it");
    let opener = control(shell, ui, pane, "type", at, clip, &tip, None, look, |painter, rect, colour| {
        icons::content(painter, rect, content, colour);
    });
    let shown = opener.as_ref().map_or(Rect::NOTHING, |r| r.rect);
    shell.layout.buttons.push((format!("header type {n}"), shown));
    let current = PaneContent::ALL.iter().position(|c| *c == content).unwrap_or(0);
    stops.push(Stop {
        id: "type",
        kind: Kind::Choice { count: rows.len(), current },
        rect: shown,
        enabled: true,
    });
    menu(shell, ui, asked, head, "type", opener.as_ref(), at.left_bottom(), &rows, false, &|_, _, _| {});

    if scene {
        let view = shell.engine.view(pane).clone();
        // The camera menu.
        let rows = camera_rows(shell, pane);
        let at = next(second_w);
        let look = Look { label: (level < 1).then_some(projection), chevron: true, ..Look::default() };
        let key = |id: &str| table().action(id).map(Action::shortcut_text).unwrap_or_default();
        let tip = tooltip_of(
            "Camera",
            "",
            &format!(
                "projection ({}), named views, orbit or fly ({}), frame the selection ({}), home ({})",
                key("proj-toggle"),
                key("fly"),
                key("frame"),
                key("view-home")
            ),
        );
        let opener = control(shell, ui, pane, "camera", at, clip, &tip, None, look, icons::camera);
        let shown = opener.as_ref().map_or(Rect::NOTHING, |r| r.rect);
        shell.layout.buttons.push((format!("header camera {n}"), shown));
        stops.push(Stop {
            id: "camera",
            kind: Kind::Choice { count: rows.len(), current: 0 },
            rect: shown,
            enabled: true,
        });
        let foot = |_: &mut Shell, ui: &mut egui::Ui, _: &mut Asked| {
            // Where the camera is: read only (it is moved in the view), at the menu's foot.
            ui.separator();
            let [x, y, z] = view.target;
            for line in [
                format!("target   {x:.2}, {y:.2}, {z:.2} m"),
                format!("yaw      {:.1}°   pitch {:.1}°", view.yaw, view.pitch),
                format!("distance {:.2} m", view.distance),
            ] {
                ui.label(
                    RichText::new(line).font(egui::FontId::monospace(theme::SMALL_PT)).color(theme::WEAK),
                );
            }
        };
        menu(shell, ui, asked, head, "camera", opener.as_ref(), at.left_bottom(), &rows, false, &foot);

        // The shading: four buttons, one lit; in a narrow viewport, one drop-down of the four.
        let rows = shading_rows(shell, pane);
        let at = next(shading_w);
        let current = SHADING.iter().position(|s| *s == view.style);
        let mut shown = Rect::NOTHING;
        if level < 3 {
            for (k, (style, row)) in SHADING.into_iter().zip(&rows).enumerate() {
                let cell =
                    Rect::from_min_size(pos2(at.left() + k as f32 * side, at.top()), vec2(side, at.height()));
                let id = format!("style {}", style.name());
                let action = format!("style-{}", style.name());
                let tip = tooltip_of(&format!("Shading: {}", row.label), &row.key, &row.about);
                let look = Look { selected: current == Some(k), ..Look::default() };
                let button = control(
                    shell,
                    ui,
                    pane,
                    &id,
                    cell,
                    clip,
                    &tip,
                    Some(&action),
                    look,
                    |painter, rect, colour| {
                        icons::shading(painter, rect, style, colour);
                    },
                );
                if let Some(button) = button {
                    shown = shown.union(button.rect);
                    shell.layout.buttons.push((row.line.clone(), button.rect));
                    if button.clicked() {
                        run_row(asked, head, row);
                    }
                }
            }
            if shown.is_positive() {
                ui.painter_at(clip).rect_stroke(
                    at.shrink2(vec2(0.0, 1.0)),
                    2.0,
                    Stroke::new(1.0, theme::WIDGET_EDGE),
                    egui::StrokeKind::Inside,
                );
            }
            menu(shell, ui, asked, head, "shading", None, at.left_bottom(), &rows, false, &|_, _, _| {});
        } else {
            let style = view.style;
            let tip = tooltip_of("Shading", "", "how surfaces are drawn: shaded, flat, x-ray or wireframe");
            let look = Look { chevron: true, ..Look::default() };
            let opener =
                control(shell, ui, pane, "shading", at, clip, &tip, None, look, |painter, rect, colour| {
                    icons::shading(painter, rect, style, colour);
                });
            shown = opener.as_ref().map_or(Rect::NOTHING, |r| r.rect);
            shell.layout.buttons.push((format!("header shading {n}"), shown));
            menu(
                shell,
                ui,
                asked,
                head,
                "shading",
                opener.as_ref(),
                at.left_bottom(),
                &rows,
                false,
                &|_, _, _| {},
            );
        }
        stops.push(Stop {
            id: "shading",
            kind: Kind::Choice { count: rows.len(), current: current.unwrap_or(0) },
            rect: shown,
            enabled: true,
        });

        // The overlays popover.
        let rows = overlay_rows(shell, pane);
        let at = next(overlays_w);
        let look = Look { label: (level < 1).then_some("Overlays"), chevron: true, ..Look::default() };
        let tip = tooltip_of(
            "Overlays",
            "",
            "what is drawn over the scene: the voxels' colour field, leaf bounds, the octree, a section plane",
        );
        let opener = control(shell, ui, pane, "overlays", at, clip, &tip, None, look, icons::overlays);
        let shown = opener.as_ref().map_or(Rect::NOTHING, |r| r.rect);
        shell.layout.buttons.push((format!("header overlays {n}"), shown));
        stops.push(Stop {
            id: "overlays",
            kind: Kind::Choice { count: rows.len(), current: 0 },
            rect: shown,
            enabled: true,
        });
        let foot = |shell: &mut Shell, ui: &mut egui::Ui, asked: &mut Asked| {
            // The section plane's place, while there is one: a slider over the voxel bodies' span.
            let Some(section) = view.section else { return };
            let time = shell.engine.time();
            let Some((lo, hi)) = cav_sim::voxel_view::section_span(shell.engine.spec(), section.axis, time)
            else {
                return;
            };
            let mut offset = section.offset;
            let slider = egui::Slider::new(&mut offset, lo..=hi).suffix(" m").max_decimals(3).text("place");
            let response = ui.add(slider);
            shell.layout.buttons.push((format!("section place {n}"), response.rect));
            if response.changed() {
                let word = ["x", "y", "z"][section.axis as usize];
                asked.lines.push(format!("section {word} {offset} @{n}"));
            }
        };
        menu(shell, ui, asked, head, "overlays", opener.as_ref(), at.left_bottom(), &rows, true, &foot);
    } else {
        // A plot: the whole of its horizontal axis, when it has been zoomed.
        let at = next(second_w);
        let zoomed = shell.engine.frame(pane).is_ok_and(|frame| frame.span.is_some());
        let tip = if zoomed {
            tooltip("span-all")
        } else {
            format!("{}\nall of it is shown already (scroll over the plot to zoom)", tooltip("span-all"))
        };
        let look = Look { disabled: !zoomed, ..Look::default() };
        let button =
            control(shell, ui, pane, "span", at, clip, &tip, Some("span-all"), look, icons::show_all);
        let shown = button.as_ref().map_or(Rect::NOTHING, |r| r.rect);
        shell.layout.buttons.push((format!("span all @{n}"), shown));
        stops.push(Stop { id: "span", kind: Kind::Button, rect: shown, enabled: zoomed });
        if button.is_some_and(|r| r.clicked())
            || (active && shell.take_click(&format!("{SCOPE}/span")) && zoomed)
        {
            activate(asked, head);
            asked.lines.push(format!("span all @{n}"));
        }
    }

    // Where the row is empty: what a test (and a hand) takes hold of to drag it.
    let free = Rect::from_min_max(pos2(x.min(right), rect.top()), pos2(right, rect.bottom()));
    shell.layout.buttons.push((format!("{} {n}", if floating { "float bar" } else { "header bar" }), free));
    if active {
        asked.stops = stops;
    }
    outcome
}

/// Registers the header's stops with the keyboard, under [`SCOPE`]: every control of
/// [`CONTROLS`], in that order, whatever the active viewport shows. `stops` are those its
/// header drew; one that was not drawn (it does not apply to what the viewport shows, or the
/// headers are hidden) is there all the same, disabled.
pub(crate) fn register(shell: &mut Shell, stops: &[Stop], area: Rect) {
    shell.register("viewport", SCOPE, "Header", Kind::Scope, area);
    for control in &CONTROLS {
        let id = format!("{SCOPE}/{}", control.id);
        match stops.iter().find(|stop| stop.id == control.id) {
            Some(stop) => {
                shell.register(SCOPE, &id, control.name, stop.kind, stop.rect);
                if !stop.enabled {
                    shell.building.disable(&id);
                }
            }
            None => {
                shell.register(SCOPE, &id, control.name, Kind::Button, Rect::NOTHING);
                shell.building.disable(&id);
            }
        }
    }
}
