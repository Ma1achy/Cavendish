//! Drawing the shell: the menu bar, the outliner, the properties, the figure (a mosaic of
//! viewports, each with its header: [`crate::header`]), the timeline, the console and the status
//! bar.
//!
//! Principia's rules, from the mock-up: the figure is never covered (panels take space beside
//! it; over a scene's canvas float only the tool palette, [`crate::tool_palette`], and the
//! navigation gizmo); status is the
//! status bar's, never a viewport header's; rarely used things are in windows; F3 hides
//! everything but the figure.

use cav_engine::{Boundary, Command, Level, Mosaic, Subsystem, Target, ViewCommand};
use egui::{
    Align, Align2, Button, Color32, FontId, Layout, Margin, Panel, Rect, RichText, Sense, Stroke, StrokeKind,
    pos2, vec2,
};

use crate::keyboard::Kind;
use crate::panes::PaneSurface;
use crate::shell::{APP_COMMANDS, ConsoleFilter, Shell};
use crate::{theme, timeline};

/// The gap between panes, pixels; the active pane's outline is drawn in it.
const PANE_GAP: u32 = 2;

fn frame(fill: Color32, x: i8, y: i8) -> egui::Frame {
    egui::Frame::new().fill(fill).inner_margin(Margin::symmetric(x, y))
}

/// A section heading inside a side panel.
/// A number for a list, to four significant figures (281.7 for 281.7428672266191): the editor
/// shows every digit; a list needs only to tell its rows apart.
pub(crate) fn short_number(value: f64) -> String {
    if value == 0.0 || !value.is_finite() {
        return format!("{value}");
    }
    let digits = (3 - value.abs().log10().floor() as i32).max(0) as usize;
    let text = format!("{value:.digits$}");
    if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.').to_owned() } else { text }
}

/// A side panel's contents in a scroll area, so that a short window scrolls them instead of
/// letting them spill over the panel below. Returns whether there is more than fits, which is
/// when the solid scroll bar shows.
///
/// With `reserve` (every panel's own use), the bar's width is kept free whether the bar shows or
/// not, so a row is as wide with it as without and nothing jumps or is cut when it appears
/// (S19's second review, minor 10). Without it the contents are egui's: wider by the bar's width
/// when there is no bar (what the reserve's test compares with).
///
/// The "more below: scroll" badge at the foot is gone (S20): the solid bar is in sight whenever
/// there is more, and the badge covered the last row.
pub fn scrolling(ui: &mut egui::Ui, id: &str, reserve: bool, contents: impl FnOnce(&mut egui::Ui)) -> bool {
    // A solid scroll bar, seen whenever there is more than fits (S19's review, minor 16: egui's
    // floating one showed only under the pointer).
    ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
    let width = ui.available_width() - ui.spacing().scroll.allocated_width();
    let shown = egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
        .show(ui, |ui| {
            if reserve {
                ui.set_max_width(width);
            }
            contents(ui);
            ui.add_space(theme::GRID);
        });
    shown.content_size.y > shown.inner_rect.height() + 0.5
}

pub fn draw(shell: &mut Shell, ui: &mut egui::Ui, surface: &mut dyn PaneSurface) {
    let ctx = ui.ctx().clone();
    if !shell.gui_hidden {
        let menu = Panel::top("menu")
            .exact_size(28.0)
            .frame(frame(theme::PANEL, 8, 0))
            .show(ui, |ui| menu_bar(shell, ui));
        shell.layout.panels.push(("menu", menu.response.rect));
        shell.building.place("top-bar", menu.response.rect);
        let footer_panel = Panel::bottom("footer")
            .exact_size(22.0)
            .frame(frame(theme::FOOTER, 10, 0))
            .show(ui, |ui| footer(shell, ui));
        shell.layout.panels.push(("footer", footer_panel.response.rect));
        shell.building.place("footer", footer_panel.response.rect);
        if shell.console_open {
            let panel = Panel::bottom("console")
                .exact_size(200.0)
                .frame(frame(theme::FOOTER, 0, 0))
                .show(ui, |ui| console(shell, ui));
            shell.layout.panels.push(("console", panel.response.rect));
        }
    }
    // Maximised, the active pane has the window but for the menu bar, the status line and the
    // console (Ctrl+Space; Blender's "maximise area"). The big scopes keep their places in the
    // Tab order: Tab onto a hidden one restores the window first.
    if !shell.gui_hidden && !shell.maximised {
        // Up to eight lanes are shown; an array with more scrolls inside the panel.
        let lanes_shown = shell.engine.spec().array.towers.len().min(timeline::MAX_LANES_SHOWN);
        let lanes_height = timeline::height(lanes_shown);
        let panel = Panel::bottom("timeline")
            // The head (the transport's row, an icon button high), the lanes, the margins.
            .exact_size(lanes_height + theme::ICON + theme::GRID + 8.0)
            .frame(frame(theme::PANEL, 8, 4))
            .show(ui, |ui| timeline_panel(shell, ui));
        shell.layout.panels.push(("timeline", panel.response.rect));
        shell.building.place("timeline", panel.response.rect);
        // The outliner gives way to the figure on a small window; the inspector's rows need
        // their 264.
        // Its edge can be dragged (egui keeps the width it is left at); the inspector's too.
        let side = (shell.layout.screen.width() * 0.19).clamp(210.0, 262.0);
        let panel = Panel::left("outliner")
            .default_size(side)
            .size_range(180.0..=520.0)
            .resizable(true)
            .frame(frame(theme::PANEL, PANEL_MARGIN, PANEL_MARGIN))
            .show(ui, |ui| scrolling(ui, "outliner", true, |ui| crate::outliner::outliner(shell, ui)));
        shell.layout.panels.push(("outliner", panel.response.rect));
        shell.building.place("tree", panel.response.rect);
        let panel = Panel::right("properties")
            // Wide enough for the cards' one grid (a 96 pt label column and X, Y, Z boxes): S19's
            // panels; the tool strip's column it replaces gave the figure its 52 points back.
            .default_size(INSPECTOR_WIDTH)
            .size_range(INSPECTOR_WIDTH..=560.0)
            .resizable(true)
            .frame(frame(theme::PANEL, PANEL_MARGIN, PANEL_MARGIN))
            .show(ui, |ui| {
                // The panel keeps its width whatever it shows (S19's review, UX: egui widened it to
                // its widest row, so it jumped with the selection): drawn into a child of exactly
                // the room there is, which tells the panel nothing of its contents' size, and
                // clipped to it.
                let room = ui.available_rect_before_wrap();
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(room));
                child.set_clip_rect(room.intersect(ui.clip_rect()));
                let below = inspector_panel(shell, &mut child);
                ui.allocate_rect(room, Sense::hover());
                below
            });
        if panel.inner {
            // Where the solid scroll bar stands while there is more than fits: the panel's
            // right edge, inside its margin.
            let rect = panel.response.rect;
            let bar = Rect::from_min_max(
                pos2(rect.right() - f32::from(PANEL_MARGIN) - SCROLL_BAR, rect.top()),
                pos2(rect.right() - f32::from(PANEL_MARGIN), rect.bottom()),
            );
            shell.layout.buttons.push(("inspector scroll bar".to_owned(), bar));
        }
        shell.layout.panels.push(("properties", panel.response.rect));
        shell.building.place("inspector", panel.response.rect);
    }
    // The figure has all the panels leave: the tool's options are a row inside a viewport
    // (S20; they were a panel above the whole figure).
    egui::CentralPanel::no_frame().show(ui, |ui| figure(shell, ui, surface));
    windows(shell, &ctx);
    if shell.quit {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

/// The inspector's panel: its tabs (the selection's properties, the towers' readings, the
/// Probe's reading) and the tab shown, scrolling. Returns whether there is more than fits.
fn inspector_panel(shell: &mut Shell, ui: &mut egui::Ui) -> bool {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (tab, name) in crate::outliner::RightTab::ALL {
            let response = tab_title(ui, name, shell.right_tab == tab);
            let id = format!("inspector/tab/{name}");
            shell.register("inspector", &id, name, Kind::Button, response.rect);
            shell.layout.buttons.push((format!("tab {name}"), response.rect));
            if response.clicked() || shell.take_click(&id) {
                shell.right_tab = tab;
            }
        }
    });
    match shell.right_tab {
        crate::outliner::RightTab::Inspector => scrolling(ui, "properties", true, |ui| properties(shell, ui)),
        crate::outliner::RightTab::Readings => {
            scrolling(ui, "readings", true, |ui| crate::outliner::readings(shell, ui))
        }
        crate::outliner::RightTab::Probe => {
            scrolling(ui, "probe", true, |ui| crate::outliner::probe(shell, ui))
        }
    }
}

/// The search box's width when the top bar has the room, points.
const SEARCH_WIDTH: f32 = 168.0;

/// The top bar, one row (`docs/design/workspace.md` §4): the menus, the layouts as tabs with a
/// `+` that keeps the present arrangement as a new one, the command search, and at the right the
/// scene's name with its unsaved mark. Nothing else: the time is the transport's. While keyboard
/// navigation is on, the breadcrumb stands before the scene's name (`docs/design/keyboard.md`
/// §10). What is painted is recorded in the layout's `top_bar`.
fn menu_bar(shell: &mut Shell, ui: &mut egui::Ui) {
    // The menus and the scene's name share the menu bar's own row, so that the name on the
    // right is laid out in what the menus leave and cannot be pushed off the window.
    egui::MenuBar::new().ui(ui, |ui| {
        for title in crate::menus::titles() {
            let response = ui.menu_button(&title, |ui| crate::menus::draw_items(shell, ui, &title)).response;
            // From the keyboard a menu opens as the palette, filtered to it: its items, searchable.
            let id = format!("top-bar/menu/{title}");
            shell.register("top-bar", &id, &title, Kind::Button, response.rect);
            shell.layout.top_bar.push(("menu", title.clone(), response.rect));
            if shell.take_click(&id) {
                shell.palette = crate::palette::Palette {
                    open: true,
                    focus_query: true,
                    menu: Some(title.clone()),
                    ..Default::default()
                };
                shell.keyboard.open_window("palette", "palette/query", true);
            }
        }
        // The layouts (`ui/layouts.ron`, then the session's own) as tabs: each arranges the
        // figure for a task.
        ui.separator();
        let layouts: Vec<crate::layouts::LayoutData> = shell.layouts().cloned().collect();
        let built_in = crate::layouts::layouts().layouts.len();
        for (n, layout) in layouts.iter().enumerate() {
            let tab = ui
                .selectable_label(
                    shell.layout_name == layout.id,
                    RichText::new(&layout.label).size(theme::BODY_PT),
                )
                .on_hover_text(&layout.about);
            let id = format!("top-bar/layout/{}", layout.id);
            let line = format!("layout {}", layout.id);
            shell.register("top-bar", &id, &layout.label, Kind::Button, tab.rect);
            shell.layout.buttons.push((line.clone(), tab.rect));
            shell.layout.top_bar.push(("layout", layout.label.clone(), tab.rect));
            if tab.clicked() || shell.take_click(&id) {
                let _ = shell.run(&line);
            }
            if n >= built_in {
                // One of the session's own can be dropped again.
                tab.context_menu(|ui| {
                    if ui.button("Forget this layout").clicked() {
                        let _ = shell.run(&format!("layout forget {}", layout.id));
                        ui.close();
                    }
                });
            }
        }
        let tip = crate::header::tooltip("layout-save");
        let (rect, plus) = ui.allocate_exact_size(vec2(theme::HIT, theme::HIT), Sense::click());
        let plus = plus.on_hover_text(&tip);
        if plus.hovered() {
            ui.painter().rect_filled(rect, 2.0, theme::HOVER);
        }
        let colour = if plus.hovered() { Color32::WHITE } else { theme::TEXT };
        crate::icons::plus(ui.painter(), Rect::from_center_size(rect.center(), vec2(12.0, 12.0)), colour);
        shell.register("top-bar", "top-bar/layout-save", "Save as a new layout", Kind::Button, rect);
        shell.layout.buttons.push(("layout save".to_owned(), rect));
        if plus.clicked() || shell.take_click("top-bar/layout-save") {
            let _ = shell.run_action("layout-save");
        }
        ui.separator();

        // The scene's name, with `*` while it has edits not saved, at the right; the search box
        // has what is left, down to its icon alone.
        let star = if shell.unsaved() { " *" } else { "" };
        let scene = format!("{}{star}", shell.engine.spec().name);
        let font = FontId::proportional(theme::BODY_PT);
        let scene_wide = ui.painter().layout_no_wrap(scene.clone(), font, theme::STRONG).size().x;
        let room = ui.available_width() - scene_wide.min(240.0) - 3.0 * theme::GRID;
        let width = room.clamp(theme::ICON, SEARCH_WIDTH);
        let tip = crate::header::tooltip("palette");
        let (rect, search) = ui.allocate_exact_size(vec2(width, theme::HIT), Sense::click());
        let search = search.on_hover_text(&tip);
        let painter = ui.painter_at(rect);
        let edge = if search.hovered() { theme::SELECTION_EDGE } else { theme::WIDGET_EDGE };
        painter.rect(rect, 3.0, theme::BACKGROUND, Stroke::new(1.0, edge), StrokeKind::Inside);
        let glass = Rect::from_center_size(pos2(rect.left() + 11.0, rect.center().y), vec2(12.0, 12.0));
        crate::icons::search(&painter, glass, theme::WEAK_ON_WIDGET);
        if width >= 120.0 {
            let small = FontId::proportional(theme::SMALL_PT);
            let at = pos2(rect.left() + 22.0, rect.center().y);
            painter.text(at, Align2::LEFT_CENTER, "Search", small, theme::WEAK_ON_WIDGET);
            let key = FontId::monospace(theme::SMALL_PT - 1.0);
            let at = pos2(rect.right() - 6.0, rect.center().y);
            painter.text(at, Align2::RIGHT_CENTER, "Ctrl+K", key, theme::WEAK);
        }
        shell.register("top-bar", "top-bar/search", "Search", Kind::Button, rect);
        shell.layout.buttons.push(("palette".to_owned(), rect));
        shell.layout.top_bar.push(("search", "Search".to_owned(), rect));
        if search.clicked() || shell.take_click("top-bar/search") {
            let _ = shell.run("palette");
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let words = RichText::new(&scene).size(theme::BODY_PT).color(theme::STRONG);
            let name = ui.add(egui::Label::new(words).truncate()).on_hover_text(if star.is_empty() {
                "the scene; nothing is unsaved"
            } else {
                "the scene; * it has edits that are not saved (Ctrl+S)"
            });
            shell.layout.top_bar.push(("scene", scene.clone(), name.rect));
            // Where the keys go, while keyboard navigation is on: the breadcrumb (the spec's §10).
            if let Some(crumb) = shell.breadcrumb() {
                let colour = match shell.keyboard.mode {
                    crate::keyboard::Mode::Navigation => crate::shell::RING_NAVIGATION,
                    crate::keyboard::Mode::Interaction => crate::shell::RING_INTERACTION,
                };
                let words = RichText::new(&crumb).size(theme::SMALL_PT).color(colour);
                let crumbs = ui.add(egui::Label::new(words).truncate());
                shell.layout.top_bar.push(("breadcrumb", crumb, crumbs.rect));
            }
        });
    });
}

/// The most characters a part of the status bar paints: a longer one is cut short, and whole in
/// its tooltip (S20: no sentences in the chrome).
const STATUS_MOST: usize = 32;

/// The status bar's parts, in its order (left to right), each with its name, its words and
/// their colour: the selection, the instrument (and the one the scene is also read on), the
/// signal's state in the engine's own words, the forward stage, and the counts of warnings and
/// errors and the frame's time when there are any.
fn status_parts(shell: &mut Shell) -> Vec<(&'static str, String, Color32)> {
    let spec = shell.engine.spec();
    let selection = match shell.engine.selection() {
        Some(Target::Tower(n)) => (spec.array.towers[n - 1].label.clone(), theme::STRONG),
        Some(Target::Body(n)) => (spec.bodies[n - 1].label.clone(), theme::STRONG),
        Some(Target::Room) => ("room".to_owned(), theme::STRONG),
        Some(Target::Floor) => ("floor".to_owned(), theme::STRONG),
        Some(Target::World) => ("world".to_owned(), theme::STRONG),
        None => ("nothing selected".to_owned(), theme::WEAK),
    };
    let mut instrument =
        cav_sim::presets::name_of(&spec.array.instrument).unwrap_or("custom instrument").to_owned();
    if let Some(other) = shell.engine.comparison() {
        let other = cav_sim::presets::name_of(other).unwrap_or("another instrument");
        instrument = format!("{instrument} vs {other}");
    }
    // Where the signal stands: a stale one is never mistaken for the current scene's, and one
    // some of whose reads did not settle never reads "within".
    let signal = match shell.engine.signal() {
        (cav_engine::SignalState::Refined, Some(signal)) => (
            cav_engine::estimate_text(signal.estimate, signal.settled, signal.truncated),
            if signal.settled { theme::TEXT } else { theme::WARN },
        ),
        (cav_engine::SignalState::Stale, _) => ("stale".to_owned(), theme::WARN),
        _ => ("none".to_owned(), theme::WEAK),
    };
    let forward = forward_status(&shell.engine);
    shell.layout.status.clone_from(&forward);
    let (warnings, errors) = shell.problem_counts();
    let plural = |n: usize, word: &str| format!("{n} {word}{}", if n == 1 { "" } else { "s" });
    let mut parts = vec![
        ("selection", selection.0, selection.1),
        ("instrument", instrument, theme::TEXT),
        ("signal", signal.0, signal.1),
        ("forward", forward, theme::WEAK),
    ];
    if warnings > 0 {
        parts.push(("warnings", plural(warnings, "warning"), theme::WARN));
    }
    if errors > 0 {
        parts.push(("errors", plural(errors, "error"), theme::ERROR));
    }
    if let Some(ms) = shell.frame_ms {
        parts.push(("frame", format!("{ms:.1} ms"), theme::WEAK));
    }
    parts
}

/// What a part of the status bar paints: its words (the signal's after the word "signal", the
/// forward stage's without the word "forward", which its place says), cut to [`STATUS_MOST`]
/// characters.
fn status_painted(name: &str, whole: &str) -> String {
    let words = match name {
        "signal" => format!("signal {whole}"),
        "forward" => whole.trim_start_matches("forward · ").to_owned(),
        _ => whole.to_owned(),
    };
    if words.chars().count() <= STATUS_MOST {
        return words;
    }
    let kept: String = words.chars().take(STATUS_MOST - 1).collect();
    format!("{}…", kept.trim_end())
}

/// The status bar: at the left the toast (what the last edit did, with the way back) or the
/// newest thing worth knowing; at the right, in a fixed order with rules between, the selection,
/// the instrument, the signal's state, the forward stage, the counts of warnings and errors, the
/// frame's time and the console's button. Each part is whole in its tooltip and in the layout's
/// `status_bar`; what is painted is in its `chrome` under `status`.
fn footer(shell: &mut Shell, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        // The newest thing worth knowing: the last entry that is not routine.
        let last = shell
            .engine
            .log()
            .iter()
            .rev()
            // (The window's own lines are routine, but for a warning of its own: a run on that
            // stopped at the longest timeline.)
            .find(|e| e.level >= Level::Info && (e.subsystem != Subsystem::App || e.level == Level::Warn))
            .cloned();
        let (text, colour) = last.map_or(("ready".to_owned(), theme::TEXT), |entry| {
            let colour = match entry.level {
                Level::Error => theme::ERROR,
                Level::Warn => theme::WARN,
                _ => theme::TEXT,
            };
            (entry.message, colour)
        });
        let parts = status_parts(shell);
        shell.layout.status_bar = parts.iter().map(|(name, whole, _)| (*name, whole.clone())).collect();
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = theme::GRID;
            let label = if shell.console_open { "close console" } else { "console" };
            let console = ui
                .add(Button::new(RichText::new(label).size(theme::SMALL_PT)).frame(false))
                .on_hover_text("the console: every command, typed (`)");
            shell.register("footer", "footer/console", "Console", Kind::Button, console.rect);
            if console.clicked() || shell.take_click("footer/console") {
                let _ = shell.run("console");
            }
            // Right to left, so the last part first; a rule before each.
            for (name, whole, colour) in parts.iter().rev() {
                ui.separator();
                let painted = status_painted(name, whole);
                let font = if *name == "frame" {
                    FontId::monospace(theme::SMALL_PT)
                } else {
                    FontId::proportional(theme::SMALL_PT)
                };
                ui.label(RichText::new(&painted).font(font).color(*colour))
                    .on_hover_text(format!("{name}: {whole}"));
                shell.layout.chrome.push(("status", painted));
                shell.layout.chrome.push(("status tooltip", format!("{name}: {whole}")));
            }
            ui.separator();
            // Whatever is left is the status line, cut short rather than wrapped. When it says an
            // edit, an Undo link stands before it (the toast: what was just done, and the way back).
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                // The toast: what the last edit did, in words (S18's review, U6), and the way
                // back. After an undo it says what was undone, and the way forward.
                // It stands until something worth more notice is logged after the edit (a
                // warning, an error); routine news (the signal recomputed) does not hide it.
                let last = shell
                    .engine
                    .log()
                    .iter()
                    .rev()
                    .find(|e| e.subsystem == Subsystem::Edit || e.level >= Level::Warn)
                    .cloned();
                let edited = last.as_ref().is_some_and(|e| e.subsystem == Subsystem::Edit);
                let undid = last.as_ref().is_some_and(|e| e.message.starts_with("undo"));
                // Opening a scene is not offered as an edit to undo (S18's second review, UX 9):
                // undo still takes it back, from the history or Ctrl+Z.
                let opened = matches!(
                    shell.engine.history_steps().0.last().map(|s| &s.redo),
                    Some(cav_engine::history::Edit::Replace(_))
                );
                let (done, undone) = shell.history_words();
                let toast = match (edited, undid) {
                    (true, true) => undone.first().map(|w| (format!("Undid: {w}"), "redo")),
                    (true, false) if !opened => done.last().map(|w| (w.clone(), "undo")),
                    _ => None,
                };
                match toast {
                    Some((words, back)) => {
                        let (label, hover) = if back == "undo" {
                            ("Undo", "take this edit back (Ctrl+Z)")
                        } else {
                            ("Redo", "put it back (Ctrl+Shift+Z)")
                        };
                        // What was done, then the way back; when the words are longer than the
                        // room, the link first, so that it is the words that are cut short.
                        let font = FontId::proportional(theme::SMALL_PT);
                        let wide = ui.painter().layout_no_wrap(words.clone(), font, theme::STRONG).size().x;
                        let fits = wide + 48.0 <= ui.available_width();
                        for link in if fits { [false, true] } else { [true, false] } {
                            if !link {
                                let words = RichText::new(&words).size(theme::SMALL_PT).color(theme::STRONG);
                                ui.add(egui::Label::new(words).truncate());
                                continue;
                            }
                            let link = ui
                                .add(
                                    Button::new(
                                        RichText::new(label).size(theme::SMALL_PT).color(theme::LINK),
                                    )
                                    .frame(false),
                                )
                                .on_hover_text(hover);
                            shell.register("footer", "footer/undo", label, Kind::Button, link.rect);
                            shell.layout.buttons.push((format!("toast {back}"), link.rect));
                            if link.clicked() || shell.take_click("footer/undo") {
                                let _ = shell.run(back);
                            }
                        }
                    }
                    None => {
                        ui.add(
                            egui::Label::new(RichText::new(text).size(theme::SMALL_PT).color(colour))
                                .truncate(),
                        );
                    }
                }
            });
        });
    });
}

/// The forward stage's state in a line: how many shots, and refined or computing. The status
/// bar's (it was the outliner's foot before S19's panels); the quadrature's error is the bar's
/// `signal` part.
fn forward_status(engine: &cav_engine::Engine) -> String {
    match engine.signal() {
        (cav_engine::SignalState::Refined, Some(signal)) => {
            let shots = signal.most();
            let (fewest, most) = taken_a_tower(engine).unwrap_or((shots, shots));
            let count = match (fewest == shots, fewest == most) {
                (true, _) => format!("{shots} shots"),
                (_, true) => format!("{shots} shots, {most} taken a tower"),
                _ => format!("{shots} shots, {fewest} to {most} taken a tower"),
            };
            format!("forward · {count} · refined")
        }
        (cav_engine::SignalState::Stale, _) => "forward · computing".to_owned(),
        _ => "forward · no signal".to_owned(),
    }
}

fn console(shell: &mut Shell, ui: &mut egui::Ui) {
    ui.painter().hline(ui.max_rect().x_range(), ui.max_rect().top(), Stroke::new(1.0, theme::RULE));
    frame(theme::FOOTER, 10, 3).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Console").color(theme::STRONG).strong());
            let (warnings, errors) = shell.problem_counts();
            for (filter, label) in [
                (ConsoleFilter::All, "all".to_owned()),
                (ConsoleFilter::Warnings, format!("warnings {warnings}")),
                (ConsoleFilter::Errors, format!("errors {errors}")),
            ] {
                if ui.add(Button::new(label).selected(shell.console_filter == filter)).clicked() {
                    shell.console_filter = filter;
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("close").clicked() {
                    shell.console_open = false;
                }
                if ui.button("clear").clicked() {
                    let _ = shell.run("clear");
                }
            });
        });
    });
    // The command line is laid out first, at the bottom, so the log takes what is left.
    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        frame(theme::BACKGROUND, 10, 3).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(">").monospace().color(theme::LINK));
                let edit = egui::TextEdit::singleline(&mut shell.console_line)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .frame(egui::Frame::NONE)
                    .hint_text("help · view top · panes 4 · style paper · load void · select tower 3 · undo");
                let response = ui.add(edit);
                if shell.focus_console {
                    response.request_focus();
                    shell.focus_console = false;
                }
                if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Backtick)) {
                    // The console's own key closes it from its line (a field keeps the global
                    // keys, so the line hears this one itself), and is not typed.
                    shell.console_line.retain(|c| c != '`');
                    shell.console_open = false;
                    response.surrender_focus();
                }
                if response.has_focus() {
                    // Up and down walk the history of what was typed.
                    let (up, down) = ui
                        .input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown)));
                    if up && !shell.history.is_empty() {
                        let at = shell.history_at.map_or(shell.history.len() - 1, |i| i.saturating_sub(1));
                        shell.history_at = Some(at);
                        shell.console_line.clone_from(&shell.history[at]);
                    } else if down && let Some(at) = shell.history_at {
                        if at + 1 < shell.history.len() {
                            shell.history_at = Some(at + 1);
                            shell.console_line.clone_from(&shell.history[at + 1]);
                        } else {
                            shell.history_at = None;
                            shell.console_line.clear();
                        }
                    }
                }
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let line = std::mem::take(&mut shell.console_line);
                    if !line.trim().is_empty() {
                        shell.history.push(line.clone());
                        shell.history_at = None;
                        let _ = shell.run(&line);
                        shell.console_open = true;
                    }
                    shell.focus_console = true;
                }
            });
        });
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.spacing_mut().interact_size.y = 0.0;
                let font = FontId::monospace(theme::SMALL_PT);
                for entry in shell.console_entries(300) {
                    let colour = match entry.level {
                        Level::Error => theme::ERROR,
                        Level::Warn => theme::WARN,
                        Level::Info => theme::STRONG,
                        Level::Debug => theme::TEXT,
                    };
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        let head = format!(
                            "{:>8.2}  {:<5}  {:<6}",
                            entry.at,
                            entry.level.name(),
                            entry.subsystem.name()
                        );
                        ui.label(RichText::new(head).font(font.clone()).color(theme::WEAK));
                        // An answer is one entry however many lines it has; a long one is cut here.
                        let shown = crate::shell::abridged(&entry.message, 24);
                        ui.label(RichText::new(shown).font(font.clone()).color(colour));
                    });
                }
            });
        });
    });
}

/// The fewest and the most shots any tower of the scene takes: the engine's cadence answer,
/// which it keeps between frames. `None` when the scene has no shots to count.
fn taken_a_tower(engine: &cav_engine::Engine) -> Option<(usize, usize)> {
    match engine.query(&cav_engine::Query::Cadence) {
        Ok(cav_engine::Reply::Cadence(cadence)) => {
            cadence.kept.iter().min().copied().zip(cadence.kept.iter().max().copied())
        }
        _ => None,
    }
}

fn timeline_panel(shell: &mut Shell, ui: &mut egui::Ui) {
    let spec = shell.engine.spec().clone();
    // The shots are the engine's (the ones the signal is read at), and the counts are its
    // cadence answer, kept between frames: nothing here works out a shot's time.
    let taken = taken_a_tower(&shell.engine);
    let mean_cycle = spec.array.schedule.mean_cycle(spec.array.instrument.cycle);
    let caption = timeline::caption(shell.engine.shots(), mean_cycle, taken);
    shell.layout.timeline_caption.clone_from(&caption);
    let facts = timeline::facts(shell.engine.shots(), taken);
    ui.horizontal(|ui| {
        // What the lanes are and how they are worked is the title's tooltip (S20: no sentences
        // in the chrome). Then the transport (`crate::transport`), and at the row's right the
        // facts that are data.
        ui.label(RichText::new("Timeline").color(theme::STRONG).strong()).on_hover_text(&caption);
        shell.layout.chrome.push(("timeline", "Timeline".to_owned()));
        shell.layout.chrome.push(("timeline tooltip", caption.clone()));
        ui.add_space(theme::GRID);
        crate::transport::draw(shell, ui, &facts, &caption);
    });
    let labels: Vec<String> = spec.array.towers.iter().map(|t| t.label.clone()).collect();
    let offsets: Vec<f64> = spec.array.towers.iter().map(|t| t.timing.offset).collect();
    let (time, selected) = (shell.engine.time(), shell.selected_tower());
    let response = egui::ScrollArea::vertical()
        .id_salt("lanes")
        .auto_shrink([false, false])
        .show(ui, |ui| timeline::lanes(ui, &labels, shell.engine.shots().ok(), spec.duration, time, selected))
        .inner;
    shell.layout.lanes.clone_from(&response.bars);
    shell.layout.shots.clone_from(&response.shots);
    shell.layout.ruler = (response.ruler.clone(), response.rect.unwrap_or(Rect::NOTHING));
    // The scrubber from the keyboard: Enter, then ← → step the playhead by 0.1 s (Shift 1 s,
    // Alt 0.01 s), held keys repeating (the spec's §5 for Time).
    shell.register(
        "timeline",
        "timeline/scrubber",
        "Scrubber",
        Kind::Adjust,
        response.rect.unwrap_or(Rect::NOTHING),
    );
    for (axis, steps, _, _) in shell.take_adjust("timeline/scrubber") {
        if axis == 0 {
            shell.playing = false;
            shell.apply(Command::Seek(shell.engine.time() + steps * crate::shell::SCRUB_STEP));
        }
    }
    // A lane dragged with Shift: a `set` of that tower's offset, each step continuing the last,
    // so that the whole drag is one edit.
    if let Some((tower, seconds)) = response.offset
        && seconds != 0.0
    {
        let path = format!("array.towers[{tower}].timing.offset");
        let continues = shell.editing.as_deref() == Some(path.as_str());
        let value = serde_json::json!(offsets[tower] + seconds);
        let applied = shell.apply(Command::Set { path: path.clone(), value, continues });
        shell.editing = (applied || continues).then_some(path);
    }
    if response.offset_ended {
        shell.editing = None;
    }
    if let Some(t) = response.seek {
        shell.playing = false;
        shell.apply(Command::Seek(t));
    }
    if let Some(n) = response.select {
        shell.apply(Command::Select(Some(Target::Tower(n + 1))));
    }
}

/// The id of the outliner row of `target`: `tree/row/body 2`, `tree/row/tower 1`, `tree/row/room`.
#[must_use]
pub fn row_id(target: Target) -> String {
    let words = cav_engine::describe(&Command::Select(Some(target)), 0);
    format!("tree/row/{}", words.trim_start_matches("select "))
}

/// The right-click menu of a row of the tree: the row is selected by the right-click, and the
/// menu opens at the pointer (egui's context menu), or under the row when `context` asks for the
/// selection's (for scripts and the demo's picture of it).
pub(crate) fn row_menu(shell: &mut Shell, response: &egui::Response, target: Target, selected: bool) {
    if response.secondary_clicked() && !selected {
        shell.apply(Command::Select(Some(target)));
    }
    let by_command = selected && std::mem::take(&mut shell.open_context);
    if by_command {
        shell.context_at_row = true;
    } else if response.secondary_clicked() {
        shell.context_at_row = false;
    }
    let open = if response.secondary_clicked() || by_command {
        Some(egui::SetOpenCommand::Bool(true))
    } else if response.clicked() {
        Some(egui::SetOpenCommand::Bool(false))
    } else {
        None
    };
    let popup = egui::Popup::menu(response).open_memory(open);
    let popup = if shell.context_at_row { popup } else { popup.at_pointer_fixed() };
    popup.show(|ui| context_menu(shell, ui));
}

/// A panel's title drawn as a tab (the design's chrome, ready for docking): the active one bright
/// on a lifted ground, the others muted. Returns its response.
pub(crate) fn tab_title(ui: &mut egui::Ui, text: &str, active: bool) -> egui::Response {
    let colour = if active { theme::STRONG } else { theme::WEAK };
    let galley = ui.painter().layout_no_wrap(text.to_owned(), theme::body(), colour);
    let size = vec2(galley.size().x + 4.0 * theme::GRID, theme::HEADER);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if active {
        ui.painter().rect_filled(rect, egui::CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 }, theme::WIDGET);
        ui.painter().hline(rect.x_range(), rect.bottom(), Stroke::new(2.0, theme::SELECTION_EDGE));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 4.0, theme::HOVER);
    }
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, colour);
    response
}

/// What a scene with no bodies means for what the towers read: the tooltip of the tree's Bodies
/// group and of the buttons under it (it was a sentence in the tree until S20).
#[must_use]
pub(crate) fn empty_words(spec: &cav_sim::SceneSpec) -> &'static str {
    match (spec.world.is_some(), spec.floor.is_some()) {
        (false, false) => "Nothing for the towers to read yet.",
        _ => "No bodies: the surroundings are read.",
    }
}

/// The ways to begin an empty scene, in the tree under its Bodies: two buttons, no sentence
/// ([`empty_words`] is the group's tooltip).
pub(crate) fn empty_guidance(shell: &mut Shell, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(30.0);
        ui.spacing_mut().item_spacing.x = theme::GRID;
        for text in ["Add a mass", "Template ›"] {
            shell.layout.chrome.push(("tree", text.to_owned()));
        }
        let mass = ui.button("Add a mass").on_hover_text("a 1 kg point mass where the view looks");
        shell.register("tree/sources", "tree/sources/add-mass", "Add a mass", Kind::Button, mass.rect);
        shell.layout.buttons.push(("add point-mass".to_owned(), mass.rect));
        if mass.clicked() || shell.take_click("tree/sources/add-mass") {
            let _ = shell.add_starter("point-mass");
        }
        let templates = ui
            .menu_button("Template ›", |ui| crate::menus::draw_items(shell, ui, "File/New from template"))
            .response;
        shell.register("tree/sources", "tree/sources/templates", "Templates", Kind::Button, templates.rect);
        if shell.take_click("tree/sources/templates") {
            let _ = shell.run("palette file");
        }
    });
}

/// The right-click menu of the scene tree and the view: the table's `context` actions for the
/// selection, then the Add menu.
fn context_menu(shell: &mut Shell, ui: &mut egui::Ui) {
    let table = crate::actions::table();
    for action in table.actions.iter().filter(|a| a.context) {
        let enabled = shell.can_run(action);
        let button = Button::new(&action.label).shortcut_text(action.shortcut_text());
        let response = ui.add_enabled(enabled, button).on_hover_text(&action.about);
        shell.layout.buttons.push((format!("context {}", action.id), response.rect));
        if response.clicked() {
            let _ = shell.run_action(&action.id);
            ui.close();
        }
    }
    // A body or a tower goes into a folder of its group, or out of every folder.
    let item = match shell.engine.selection() {
        Some(Target::Body(n)) => Some(cav_sim::FolderItem::Body(n)),
        Some(Target::Tower(n)) => Some(cav_sim::FolderItem::Tower(n)),
        _ => None,
    };
    if let Some(item) = item {
        ui.menu_button("Move to", |ui| crate::outliner::move_menu(shell, ui, &item));
    }
    ui.separator();
    ui.menu_button("Add", |ui| crate::menus::add_menu(shell, ui));
}

/// The text field that renames an item in place (`F2`, a double click): Enter keeps the name
/// (a `set` of its label: one edit, undone as one), Esc leaves it as it was.
/// The name of a row being renamed (F2), typed in place.
pub(crate) fn rename_field_at(shell: &mut Shell, ui: &mut egui::Ui, target: Target, rect: Rect) {
    let Some((_, mut text)) = shell.renaming.clone() else { return };
    let response = ui.put(rect, egui::TextEdit::singleline(&mut text).font(theme::body()));
    shell.layout.editors.push(("rename".to_owned(), response.rect));
    if !response.has_focus() && !response.lost_focus() {
        response.request_focus();
    }
    shell.renaming = Some((target, text.clone()));
    if response.lost_focus() {
        shell.renaming = None;
        let keep = ui.input(|i| i.key_pressed(egui::Key::Enter)) && !text.trim().is_empty();
        let path = match target {
            Target::Body(n) => format!("bodies[{}].label", n - 1),
            Target::Tower(n) => format!("array.towers[{}].label", n - 1),
            _ => return,
        };
        if keep {
            shell.apply(Command::Set { path, value: serde_json::json!(text.trim()), continues: false });
        }
    }
}

/// The most rows the outliner lays out for the bodies, and for the towers: a scene of a thousand
/// of either is not a list to scroll, and every row is laid out every frame. The rest are said
/// to be there; `ls bodies` lists them all. A selected item past the cap keeps its row.
pub const MAX_OUTLINE_ROWS: usize = 100;

/// Whether a scene has nothing its towers could read: no bodies, no world, no floor.
#[must_use]
pub fn silent(spec: &cav_sim::SceneSpec) -> bool {
    spec.bodies.is_empty() && spec.world.is_none() && spec.floor.is_none()
}

/// What tower `n`'s outliner row says: its reading under the playhead, marked ⚠ when the audit
/// warns of it and ◐ when it is of part of the matter only (the world read within a radius);
/// its place when the scene is `silent` (nothing to read) or there is no reading yet.
#[must_use]
pub fn tower_detail(reply: &cav_engine::ReadingReply, n: usize, position: [f64; 3], silent: bool) -> String {
    let [x, y, _] = position;
    let warned = reply.warnings.iter().any(|w| w.tower == Some(n));
    match reply.towers.get(n) {
        Some(reading) if !silent => format!(
            "{}{}{}",
            crate::shell::phase_text(reading.gradiometer),
            if reply.truncated.is_some() { " ◐" } else { "" },
            if warned { " ⚠" } else { "" }
        ),
        _ => format!("{x:+.1} {y:+.1}"),
    }
}

/// The world's properties as rows: what it is (pitch, pieces, materials, finest cells), how it is
/// read (radius, tolerance) and what is resident now (`resident` chunks and bricks, `pending`
/// queued).
#[must_use]
pub(crate) fn world_rows(
    world: &cav_sim::world::WorldSpec,
    resident: [usize; 2],
    pending: usize,
) -> Vec<(&'static str, String)> {
    let cells = cav_sim::world::World::new(world).cells(world);
    vec![
        ("pitch", format!("{} cm", short_number(world.pitch * 100.0))),
        ("pieces", world.pieces.len().to_string()),
        ("materials", world.materials.iter().map(|m| m.label.as_str()).collect::<Vec<_>>().join(", ")),
        ("finest cells", format!("{cells:.1e}")),
        ("read within", format!("{} m of each tower", short_number(world.read.radius))),
        ("read tolerance", format!("{:.0e} rad", world.read.tolerance)),
        ("resident", format!("{} chunks, {} bricks", resident[0], resident[1])),
        ("queued", pending.to_string()),
    ]
}

/// The inspector's width at first and its least, points: room for the cards' one grid (a label
/// column of 100 pt, then X, Y and Z boxes of six figures each, or a number, its unit and its
/// reset) and the scroll bar's reserved width. 352 until S20, when targets were 24 pt and boxes
/// had their text centred. The label column is a third of this, not of the panel as dragged. The
/// panel does not grow with what it holds (its contents are drawn in a child of fixed width), so
/// a dragged width is kept whatever is selected.
const INSPECTOR_WIDTH: f32 = 320.0;
/// A side panel's margin, points: one step of the grid.
const PANEL_MARGIN: i8 = theme::GRID as i8;
/// The width a side panel keeps for its solid scroll bar, points (egui's solid bar: 6 and a
/// margin of 4).
const SCROLL_BAR: f32 = 10.0;

fn properties(shell: &mut Shell, ui: &mut egui::Ui) {
    let spec = shell.engine.spec().clone();
    // One label column for every card: a third of the panel's own room (its margins and the
    // scroll bar's width taken off), at least 96 points, so every value starts at the same x.
    shell.label_width =
        crate::inspector::label_width(INSPECTOR_WIDTH - 2.0 * f32::from(PANEL_MARGIN) - SCROLL_BAR);
    // The selection's section, as before (the breadcrumb's "Inspector › Body › …").
    let selection = shell.engine.selection();
    let (kind, path) = match selection {
        None => ("Scene", String::new()),
        Some(Target::Tower(n)) => ("Tower", format!("array.towers[{}]", n - 1)),
        Some(Target::Room) => ("Room", "room".to_owned()),
        Some(Target::Floor) => ("Floor", "floor".to_owned()),
        Some(Target::World) => ("World", "world".to_owned()),
        Some(Target::Body(n)) => ("Body", format!("bodies[{}]", n - 1)),
    };
    // Every target in the panel the theme's side (S19's review, I6; 20 pt since S20): boxes,
    // ticks and drop-downs take their height from this. The least width of a box is a target's
    // too, not egui's 40: a vector's three boxes share what the row has (the panel keeps its
    // width).
    theme::dense(ui, theme::ROW_GAP);
    crate::inspector::header(shell, ui);
    match selection {
        None => {
            let menu = |shell: &mut Shell, ui: &mut egui::Ui| {
                crate::inspector::fields_menu(shell, ui, "", &["description", "duration"]);
            };
            crate::inspector::card(
                shell,
                ui,
                "inspector",
                "inspector/selection",
                "Scene",
                "scene",
                true,
                Some(&menu),
                |shell, ui| {
                    for path in ["description", "duration"] {
                        crate::editors::show(shell, ui, "inspector/selection", path);
                    }
                },
            );
        }
        Some(Target::World) => {
            let rows = spec.world.as_ref().map(|world| {
                let streaming = shell.engine.streaming();
                let resident = streaming.frame().map_or([0, 0], |f| [f.chunks.len(), f.bricks.len()]);
                world_rows(world, resident, streaming.pending())
            });
            let menu = |shell: &mut Shell, ui: &mut egui::Ui| crate::inspector::part_menu(shell, ui, "world");
            crate::inspector::card(
                shell,
                ui,
                "inspector",
                "inspector/selection",
                "World",
                "world",
                true,
                Some(&menu),
                |shell, ui| {
                    if let Some(rows) = &rows {
                        let rows: Vec<(&str, String)> = rows.iter().map(|(n, v)| (*n, v.clone())).collect();
                        crate::inspector::info_rows(shell, ui, &rows);
                    }
                    for part in ["world.read", "world.stream", "world.lod"] {
                        crate::editors::show(shell, ui, "inspector/selection", part);
                    }
                },
            );
        }
        // A body's cards in the design's order (S19's review, I5).
        Some(Target::Body(n)) => crate::inspector::body(shell, ui, n - 1),
        Some(_) => {
            // The selection's own fields in a card named by what it is; its parts in cards of
            // their own (`editors::show`). Its name is the header's, so not repeated here.
            crate::editors::show_in_cards(shell, ui, "inspector/selection", &path, &["label"], Some(kind));
        }
    }
    // The array's own settings belong to the scene and to its towers, not to a body (S18). Info,
    // what is worked out, comes last whatever is selected (S19's review, minor 17).
    if !matches!(selection, None | Some(Target::Tower(_))) {
        crate::inspector::info(shell, ui);
        return;
    }
    let (salt, open) = if selection.is_none() { ("scene", false) } else { ("tower", true) };
    // The Array's ⋮ is over its towers (their layout): reset, copy, paste.
    let menu = |shell: &mut Shell, ui: &mut egui::Ui| crate::inspector::part_menu(shell, ui, "array.towers");
    crate::inspector::card(
        shell,
        ui,
        "inspector",
        "inspector/array",
        "Array",
        salt,
        true,
        Some(&menu),
        |shell, ui| {
            let instrument = shell.engine.spec().array.instrument.clone();
            let other = shell.engine.comparison().cloned();
            let now = preset_word(&instrument);
            preset_choice(shell, ui, "inspector/array", "Preset", "preset", now, false);
            let compared = other.as_ref().map_or("off", preset_word);
            preset_choice(shell, ui, "inspector/array", "Compare with", "compare", compared, true);
        },
    );
    let menu =
        |shell: &mut Shell, ui: &mut egui::Ui| crate::inspector::part_menu(shell, ui, "array.schedule");
    crate::inspector::card(
        shell,
        ui,
        "inspector",
        "inspector/cadence",
        "Cadence",
        salt,
        open,
        Some(&menu),
        |shell, ui| {
            crate::editors::show(shell, ui, "inspector/cadence", "array.schedule");
        },
    );
    let menu =
        |shell: &mut Shell, ui: &mut egui::Ui| crate::inspector::part_menu(shell, ui, "array.instrument");
    crate::inspector::card(
        shell,
        ui,
        "inspector",
        "inspector/instrument",
        "Instrument",
        salt,
        open,
        Some(&menu),
        |shell, ui| {
            crate::editors::show(shell, ui, "inspector/instrument", "array.instrument");
        },
    );
    crate::inspector::info(shell, ui);
}

/// The preset an instrument is, by the word `preset` takes, or "custom".
fn preset_word(instrument: &cav_sim::InstrumentSpec) -> &'static str {
    ["aion10", "aice"]
        .into_iter()
        .find(|word| cav_sim::presets::load(word).is_ok_and(|preset| preset == *instrument))
        .unwrap_or("custom")
}

/// A drop-down of the instrument presets that runs `<line> <preset>` (`preset aion10`, `compare
/// aice`), with "off" for `compare off` when `off` is offered. `now` is the word chosen now.
fn preset_choice(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    scope: &str,
    name: &str,
    line: &str,
    now: &str,
    off: bool,
) {
    let mut choices: Vec<(&str, &str)> = vec![("AION-10", "aion10"), ("AICE 140 m", "aice")];
    if off {
        choices.push(("off", "off"));
    }
    let shown = choices.iter().find(|(_, w)| *w == now).map_or("custom", |(l, _)| *l);
    ui.horizontal(|ui| {
        theme::fixed_left(
            ui,
            vec2(shell.label_width, theme::HIT),
            egui::Label::new(RichText::new(name).size(theme::BODY_PT).color(theme::WEAK)),
        );
        let mut chosen = None;
        let combo = egui::ComboBox::from_id_salt(("preset", line))
            .width(crate::editors::value_room(ui))
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for (label, word) in &choices {
                    if ui.selectable_label(*label == shown, *label).clicked() {
                        chosen = Some(*word);
                    }
                }
            });
        let id = format!("{scope}/{line}");
        let current = choices.iter().position(|(l, _)| *l == shown).unwrap_or(0);
        shell.register(scope, &id, name, Kind::Choice { count: choices.len(), current }, combo.response.rect);
        if let Some(index) = shell.take_pick(&id) {
            chosen = choices.get(index).map(|(_, w)| *w);
        }
        if let Some(word) = chosen
            && word != now
        {
            let _ = shell.run(&format!("{line} {word}"));
        }
    });
}

/// The scene at a glance, for the Info card with nothing selected (S18's review, U1): what the
/// array is, what is in the scene, what surrounds it, where the signal stands.
pub(crate) fn scene_rows(shell: &mut Shell) -> Vec<(&'static str, String)> {
    let spec = shell.engine.spec();
    let towers = spec.array.towers.len();
    let instrument = cav_sim::presets::name_of(&spec.array.instrument).unwrap_or("its own instrument");
    let plural = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let around = match (&spec.room, &spec.floor, &spec.world) {
        (Some(room), _, _) => {
            let [x, y, z] = room.size;
            format!("a room, {x:.0} × {y:.0} × {z:.0} m")
        }
        (None, Some(floor), _) => format!("a floor at z = {:.1} m", floor.height),
        (None, None, Some(world)) => format!("a world, {} piece(s)", world.pieces.len()),
        (None, None, None) => "nothing".to_owned(),
    };
    let signal = match shell.engine.signal() {
        (cav_engine::SignalState::Refined, Some(signal)) => format!("{} shots, read", signal.most()),
        (cav_engine::SignalState::Stale, _) => "computing".to_owned(),
        _ => "none".to_owned(),
    };
    vec![
        ("Array", format!("{}, {instrument}", plural(towers, "tower", "towers"))),
        (
            "Sources",
            if spec.bodies.is_empty() {
                "none yet".to_owned()
            } else {
                plural(spec.bodies.len(), "body", "bodies")
            },
        ),
        ("Around it", around),
        ("Signal", signal),
    ]
}

/// The Probe tool's point, marked in the view: a gold ring. Its reading is the right panel's
/// Probe tab ([`crate::outliner::probe`]); nothing else of it is drawn over the figure (S20; S19's
/// second review, minor 13).
fn probe_mark(shell: &Shell, ui: &egui::Ui, pane: usize, rect: Rect) {
    let Some(probed) = &shell.probed else { return };
    let painter = ui.painter_at(rect);
    if let Some(p) = shell.point_on_screen(pane, probed.point, rect.size()) {
        let p = p + rect.min.to_vec2();
        painter.circle_stroke(p, 6.0, Stroke::new(3.0, Color32::from_black_alpha(160)));
        painter.circle_stroke(p, 6.0, Stroke::new(1.5, theme::GOLD));
        painter.circle_filled(p, 1.5, theme::GOLD);
    }
}

/// A positive quantity in `unit` with the SI prefix that keeps it between 1 and 1000: `si(1.2e-7,
/// "m/s²")` is "120 nm/s²". Three significant figures.
pub(crate) fn si(value: f64, unit: &str) -> String {
    if value == 0.0 || !value.is_finite() {
        return format!("{value} {unit}");
    }
    let prefixes = [(1e-15, "f"), (1e-12, "p"), (1e-9, "n"), (1e-6, "µ"), (1e-3, "m"), (1.0, ""), (1e3, "k")];
    let (scale, prefix) =
        prefixes.iter().rev().find(|(s, _)| value.abs() >= *s).copied().unwrap_or(prefixes[0]);
    let scaled = value / scale;
    let digits = (2 - scaled.abs().log10().floor() as i32).clamp(0, 2) as usize;
    format!("{scaled:.digits$} {prefix}{unit}")
}

/// The Move tool's gizmo on body `body` in a pane at `rect`: three arrows (x red, y green, z blue,
/// as the inspector's vectors read) and a square, drawn over the body in bright colours on a dark
/// halo, so that they stand out from a body of any colour (S18's review, U5); the held handle is
/// drawn thicker. What is drawn is what the pointer can take. Returns the gizmo, in the pane's
/// own points.
fn draw_gizmo(
    shell: &Shell,
    ui: &egui::Ui,
    pane: usize,
    rect: Rect,
    body: usize,
) -> Option<crate::tools::Gizmo> {
    let spec = shell.engine.spec();
    let at = spec.bodies.get(body)?.motion.position(shell.engine.time(), spec.array.instrument.gravity);
    let gizmo = crate::tools::gizmo(shell.engine.view(pane), rect.size(), at)?;
    // Over the pane's picture (painted before it, in the same layer), under any window.
    let painter = ui.painter_at(rect);
    let colours = [
        Color32::from_rgb(0xff, 0x5a, 0x5a),
        Color32::from_rgb(0x6c, 0xe8, 0x6c),
        Color32::from_rgb(0x6c, 0xb4, 0xff),
    ];
    let halo = Color32::from_black_alpha(200);
    let held = shell.gizmo_drag.as_ref().map(|d| d.handle);
    let origin = rect.min.to_vec2();
    let centre = gizmo.centre + origin;
    for (axis, tip) in gizmo.tips.iter().enumerate() {
        let Some(tip) = tip else { continue };
        let width = if held == Some(crate::tools::Handle::Axis(axis)) { 4.0 } else { 2.5 };
        let tip = *tip + origin;
        let along = (tip - centre).normalized();
        let side = vec2(-along.y, along.x);
        let head = [tip, tip - along * 13.0 + side * 6.0, tip - along * 13.0 - side * 6.0];
        let shaft = [centre + along * 7.0, tip - along * 11.0];
        // The halo first, then the colour on it.
        painter.line_segment(shaft, Stroke::new(width + 3.0, halo));
        painter.add(egui::Shape::convex_polygon(head.to_vec(), colours[axis], Stroke::new(2.0, halo)));
        painter.line_segment(shaft, Stroke::new(width, colours[axis]));
        // The axis's letter by its tip.
        painter.text(
            tip + along * 9.0,
            Align2::CENTER_CENTER,
            ["x", "y", "z"][axis],
            theme::body(),
            colours[axis],
        );
    }
    let square = Rect::from_center_size(centre, vec2(11.0, 11.0));
    let free = held == Some(crate::tools::Handle::Centre);
    painter.rect(
        square,
        2.0,
        if free { Color32::WHITE } else { Color32::from_white_alpha(60) },
        Stroke::new(1.5, Color32::WHITE),
        StrokeKind::Middle,
    );
    painter.rect_stroke(square.expand(1.5), 2.0, Stroke::new(1.5, halo), StrokeKind::Outside);
    Some(gizmo)
}

/// The Place tool's aim: the ground's grid, a metre a line, eight metres round the point under the
/// pointer (`at`, points within pane `pane` at `rect`; with no pointer over the pane, round the
/// point under the pane's middle), and, under the pointer, a ghost of the starter: its outline on
/// the ground at its size (a point mass's a small ring) and a line up to where its middle will be.
/// The ghost's box is in the layout's buttons as `place ghost`.
fn place_preview(shell: &mut Shell, ui: &egui::Ui, pane: usize, rect: Rect, at: Option<egui::Pos2>) {
    let view = shell.engine.view(pane).clone();
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let ground = shell.ground();
    let middle_of_pane = egui::pos2(0.5 * rect.width(), 0.5 * rect.height());
    let aimed = at.unwrap_or(middle_of_pane);
    let aim = crate::tools::on_ground(&view, size, [f64::from(aimed.x), f64::from(aimed.y)], ground);
    let camera = cav_render::Camera::new(&view, size[0] / size[1]);
    let scale = 1024.0;
    let whole = [(size[0] * scale) as u32, (size[1] * scale) as u32];
    let screen = |p: [f64; 3]| {
        camera
            .pixel(cav_core::Vec3::from_array(p), whole)
            .map(|[x, y]| rect.min + vec2((x / scale) as f32, (y / scale) as f32))
    };
    let painter = ui.painter_at(rect);
    let line = Stroke::new(1.0, Color32::from_white_alpha(38));
    let (cx, cy) = (aim[0].round(), aim[1].round());
    for k in -8..=8 {
        let k = f64::from(k);
        for (a, b) in [
            ([cx + k, cy - 8.0, ground], [cx + k, cy + 8.0, ground]),
            ([cx - 8.0, cy + k, ground], [cx + 8.0, cy + k, ground]),
        ] {
            if let (Some(a), Some(b)) = (screen(a), screen(b)) {
                painter.line_segment([a, b], line);
            }
        }
    }
    if at.is_none() {
        return;
    }
    let starter = cav_sim::starters::find(&shell.place_starter);
    let body = starter.and_then(|s| s.body.as_ref());
    let reach = body.map_or(0.5, |b| {
        if b.shape.is_none() && b.radius == 0.0 { 0.15 } else { cav_sim::starters::reach(b) }
    });
    let lift = match body {
        Some(b) if b.shape.is_none() && b.radius == 0.0 => 1.0,
        Some(b) => cav_sim::starters::lift(b),
        None => 0.0,
    };
    let ghost = theme::GOLD.gamma_multiply(0.85);
    let ring: Vec<egui::Pos2> = (0..=40)
        .filter_map(|k| {
            let a = std::f64::consts::TAU * f64::from(k) / 40.0;
            screen([aim[0] + reach * a.cos(), aim[1] + reach * a.sin(), ground])
        })
        .collect();
    if ring.len() > 2 {
        painter.add(egui::Shape::line(ring.clone(), Stroke::new(1.5, ghost)));
    }
    let middle = [aim[0], aim[1], ground + lift];
    if let (Some(foot), Some(top)) = (screen(aim), screen(middle)) {
        painter.line_segment([foot, top], Stroke::new(1.0, ghost));
        painter.circle_stroke(top, 4.0, Stroke::new(1.5, ghost));
        let bounds =
            ring.iter().fold(Rect::from_min_max(foot, foot), |r, p| r.union(Rect::from_min_max(*p, *p)));
        shell
            .layout
            .buttons
            .push(("place ghost".to_owned(), bounds.union(Rect::from_center_size(top, vec2(8.0, 8.0)))));
    }
}

/// How far one keyboard step orbits the canvas, degrees (the spec's 1° for spherical views).
pub const ORBIT_STEP: f64 = 1.0;
/// How far one keyboard step pans the canvas, as a share of the camera's distance.
pub const PAN_STEP: f64 = 0.02;
/// How far one keyboard step zooms the canvas, in powers of two of the distance.
pub const ZOOM_STEP: f64 = 0.25;

/// The viewport's canvas as the keyboard reaches it: Enter, then the arrows orbit by
/// [`ORBIT_STEP`] (with Ctrl they pan by [`PAN_STEP`] of the distance) and `+` `-` zoom by
/// [`ZOOM_STEP`], Shift ×10 and Alt ×0.1. Each is the same view command a drag issues, so none is
/// an edit and none is undone (the owner's rule: camera moves are not undoable, Home apart).
fn canvas_keys(shell: &mut Shell, area: Rect) {
    shell.register("viewport", "viewport/canvas", "Canvas", Kind::Canvas, area);
    let pane = shell.engine.active_pane();
    for (axis, steps, ctrl, _) in shell.take_adjust("viewport/canvas") {
        let view = shell.engine.view(pane).clone();
        // (While flying the arrows look: the flight takes them before they come here.)
        let command = if ctrl {
            let camera = cav_render::Camera::new(&view, 1.0);
            let along = if axis == 0 { camera.right } else { camera.up };
            let moved = camera.origin + along * (steps * PAN_STEP * view.distance);
            ViewCommand::Pan { pane, target: moved.to_array() }
        } else if axis == 0 {
            ViewCommand::Orbit { pane, yaw: view.yaw - steps * ORBIT_STEP, pitch: view.pitch }
        } else {
            ViewCommand::Orbit { pane, yaw: view.yaw, pitch: view.pitch + steps * ORBIT_STEP }
        };
        shell.apply(Command::View(command));
    }
    while let Some((crate::keyboard::Effect::Zoom { steps, .. }, _)) = shell
        .take_effect(|e| matches!(e, crate::keyboard::Effect::Zoom { id, .. } if id == "viewport/canvas"))
    {
        let distance = shell.engine.view(pane).distance * 2f64.powf(-steps * ZOOM_STEP);
        shell.apply(Command::View(ViewCommand::Zoom { pane, distance }));
    }
}

/// The menu path of the actions that act on one viewport: the View menu's Viewport submenu, and
/// every viewport's right-click menu.
pub const VIEWPORT_MENU: &str = "View/Viewport";

/// The least a viewport's column is wide and its row high, points: a splitter stops there.
pub const MIN_VIEWPORT: f32 = 80.0;

/// The least a floating viewport's frame is, points (width, height).
pub const MIN_FLOAT: egui::Vec2 = vec2(200.0, 140.0);

/// A splitter being dragged: the layout follows the pointer here, in the window, and the engine
/// hears of it once, when the drag ends (one journal line a drag).
#[derive(Clone, Debug)]
pub(crate) struct SplitterDrag {
    /// The boundary it moves.
    pub boundary: Boundary,
    /// The engine's mosaic when the drag began.
    pub from: Mosaic,
    /// The mosaic as the drag has made it so far.
    pub now: Mosaic,
    /// How far the pointer was pressed from the middle of the gap, points along the drag's axis.
    pub grab: f32,
}

/// `from` with `boundary` moved so that the middle of its gap lies `wanted` pixels along a
/// figure `extent` pixels long (with `gap` pixels between tracks): the two weights either side
/// change, their sum kept, and neither track is left narrower than `least` pixels (if both
/// cannot have that, nothing moves).
#[must_use]
pub fn splitter_moved(
    from: &Mosaic,
    boundary: Boundary,
    wanted: f32,
    extent: u32,
    gap: u32,
    least: f32,
) -> Mosaic {
    let (weights, k) = match boundary {
        Boundary::Column(k) => (from.column_weights(), k),
        Boundary::Row(k) => (from.row_weights(), k),
    };
    if k + 1 >= weights.len() {
        return from.clone();
    }
    let gaps = (gap * (weights.len() as u32 - 1)).min(extent);
    let room = f64::from(extent - gaps);
    let total: f64 = weights.iter().sum();
    let pair = weights[k] + weights[k + 1];
    let least = f64::from(least) / room * total;
    if !(room > 0.0 && 2.0 * least < pair) {
        return from.clone();
    }
    // The gap's middle is half a gap past the end of track `k`, with `k` gaps before it.
    let edge = f64::from(wanted) - (k as f64 + 0.5) * f64::from(gap);
    let before: f64 = weights[..k].iter().sum();
    let weight = (edge / room * total - before).clamp(least, pair - least);
    from.resize(boundary, weight - weights[k]).unwrap_or_else(|_| from.clone())
}

/// What the viewports of one frame asked for, carried out once all of them are drawn.
pub(crate) struct Asked {
    /// Typed commands, applied first and in order.
    pub commands: Vec<Command>,
    /// Camera drags: the pane, the pointer's move, whether it pans, the canvas's size.
    pub drags: Vec<(usize, egui::Vec2, bool, egui::Vec2)>,
    /// Drags of a body: the pane, the body, the pointer's move, the canvas's size.
    pub body_drags: Vec<(usize, usize, egui::Vec2, egui::Vec2)>,
    /// The wheel: the pane, the scroll, the pointer in the canvas, the canvas's size.
    pub zooms: Vec<(usize, f32, [f64; 2], [f64; 2])>,
    /// Where the selection's keyboard handle is drawn, if the active pane shows the scene.
    pub handle_rect: Rect,
    /// Actions chosen from a viewport's right-click menu, by id.
    pub actions: Vec<String>,
    /// Console lines the viewports' headers, gizmos and dock buttons run, in order.
    pub lines: Vec<String>,
    /// A header let go after a drag: the pane it is of and where the pointer was, points. Let
    /// go over another viewport, the two swap places.
    pub dropped: Option<(usize, egui::Pos2)>,
    /// The active viewport's header controls, as the keyboard's stops.
    pub stops: Vec<crate::header::Stop>,
    /// The tool palette as it was drawn, if it was: the keyboard's stops in the `toolbar` scope.
    pub tools: Option<crate::tool_palette::Drawn>,
    /// The snap switch in the tool's options row, where it was drawn (nothing when it was not).
    pub snap: Rect,
}

/// A viewport's right-click menu on `response` (its canvas, or its header's empty stretch): what
/// can be done to this viewport (the View menu's Viewport items, from the action table). A
/// click, not a hold: holding the right button over a scene flies the camera.
pub(crate) fn viewport_menu(shell: &mut Shell, asked: &mut Asked, response: &egui::Response) {
    response.context_menu(|ui| {
        for action in crate::actions::table().actions.iter().filter(|a| a.menu == VIEWPORT_MENU) {
            if action.rule {
                ui.separator();
            }
            let button = Button::new(&action.label).shortcut_text(action.shortcut_text());
            let item = ui.add_enabled(shell.can_run(action), button).on_hover_text(&action.about);
            shell.layout.buttons.push((format!("viewport menu {}", action.id), item.rect));
            if item.clicked() {
                asked.actions.push(action.id.clone());
                ui.close();
            }
        }
    });
}

/// One viewport to draw: which, where, and what the figure round it is.
#[derive(Clone, Copy)]
struct Place {
    /// The pane, from 0.
    pane: usize,
    /// Where it is drawn, points: the whole viewport, or, for [`canvas`], its canvas.
    rect: Rect,
    /// That rectangle's size in pixels.
    pixels: [u32; 2],
    /// How many viewports the figure has, and which is active.
    count: usize,
    active: usize,
    /// The engine's generation this frame.
    generation: u64,
    /// Whether it floats over the window (its header is the bar it is dragged by).
    floating: bool,
    /// Whether it is the tool palette's host ([`crate::tool_palette::host`]): the palette is
    /// drawn over its canvas and the tool's options row under its header.
    tools: bool,
}

/// Draws one viewport at its place and takes its input: its header ([`crate::header`]) along
/// its top, its canvas under it ([`canvas`]), a scene's navigation gizmo in the canvas's corner
/// ([`crate::navgizmo`]) and the active one's edge; and, in the tool palette's host, the tool's
/// options row between the header and the canvas while it applies, and the palette over the
/// canvas ([`crate::tool_palette`]). With the panels hidden (F3) there is no header, no gizmo
/// and no palette: the canvas is the viewport. Returns what the header's own pointer work leaves
/// for a floating viewport's frame.
fn viewport(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    surface: &mut dyn PaneSurface,
    asked: &mut Asked,
    place: Place,
) -> crate::header::Outcome {
    let Place { pane, rect, pixels, count, active, floating, tools, .. } = place;
    let scale = ui.ctx().pixels_per_point();
    let mut outcome = crate::header::Outcome::default();
    // The header's height in whole pixels, so that the canvas is whole pixels too; and the
    // options row's, which is there only while the tool in hand has something to set.
    let row = (theme::HEADER * scale).round() as u32;
    let head = if shell.gui_hidden { 0 } else { row.min(pixels[1]) };
    let tools = tools && head > 0;
    let options =
        if tools && crate::tool_palette::options_apply(shell) { row.min(pixels[1] - head) } else { 0 };
    let split = rect.top() + head as f32 / scale;
    let top = split + options as f32 / scale;
    let inner = Rect::from_min_max(pos2(rect.left(), top), rect.max);
    shell.layout.canvases[pane] = inner;
    if head > 0 {
        let row = Rect::from_min_max(rect.min, pos2(rect.right(), split));
        let head = crate::header::Head { pane, rect: row, active: pane == active, count, floating };
        outcome = crate::header::draw(shell, ui, asked, head);
    }
    if options > 0 {
        let row = Rect::from_min_max(pos2(rect.left(), split), pos2(rect.right(), top));
        crate::tool_palette::options_row(shell, ui, asked, row);
    }
    let left = [pixels[0], pixels[1] - head - options];
    if left[1] > 0 {
        canvas(shell, ui, surface, asked, Place { rect: inner, pixels: left, ..place });
        if head > 0 && shell.engine.content(pane) == cav_engine::PaneContent::Scene {
            crate::navgizmo::draw(shell, ui, asked, pane, inner, pane == active);
        }
        if tools {
            crate::tool_palette::draw(shell, ui, asked, pane, inner, pane == active);
        }
    }
    // The active viewport is marked by its edge (and its header's ground), not by words.
    if count > 1 {
        let (colour, width) = if pane == active { (theme::SELECTION_EDGE, 1.5) } else { (theme::RULE, 1.0) };
        ui.painter().rect_stroke(rect, 0.0, Stroke::new(width, colour), StrokeKind::Inside);
    }
    outcome
}

/// Draws one viewport's canvas (`place.rect`, `place.pixels`) and takes its input: what it
/// shows (the scene's picture or a plot), its right-click menu, and the pointer's work in it
/// (select, drag, orbit, pan, zoom, the tool in hand), every place of the pointer measured from
/// the canvas's corner. What it asks of the engine goes into `asked`.
fn canvas(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    surface: &mut dyn PaneSurface,
    asked: &mut Asked,
    place: Place,
) {
    let Place { pane, rect, pixels, active, generation, .. } = place;
    let content = shell.engine.content(pane);
    if content == cav_engine::PaneContent::Scene {
        let frame = shell.engine.frame(pane).expect("the mosaic has no more panes than the engine has");
        match surface.texture(pane, pixels, generation, frame) {
            Some(texture) => {
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                ui.painter().image(texture, rect, uv, Color32::WHITE);
                // The legend of what colours the voxel bodies, made from what is drawn, in
                // the pane's bottom right corner: never over the middle of the figure.
                if let Some(voxels) = frame.voxels {
                    let painter = ui.painter_at(rect);
                    let measure = cav_plot::paint::EguiMeasure(&painter);
                    let style = cav_plot::style::Style::screen();
                    // The field's legend, and the tree overlay's above it.
                    let keys = [
                        cav_plot::voxels::legend(voxels, frame.scene, &style, &measure),
                        cav_plot::voxels::tree_legend(voxels, &style, &measure),
                    ];
                    let mut bottom = rect.bottom();
                    for key in keys.into_iter().flatten() {
                        let size = vec2(key.size[0], key.size[1]);
                        let at = pos2(rect.right(), bottom) - size - vec2(8.0, 8.0);
                        cav_plot::paint::paint(&painter, at, &key);
                        shell.layout.legends.push((pane, Rect::from_min_size(at, size)));
                        bottom = at.y;
                    }
                }
            }
            None => {
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    surface.problem(pane).unwrap_or_else(|| "no GPU surface".to_owned()),
                    theme::body(),
                    theme::WEAK,
                );
            }
        }
    } else {
        let drawn = crate::plots::paint(&shell.engine, &ui.painter_at(rect), pane, rect);
        shell.layout.plots.extend(drawn);
    }
    let response = ui.interact(rect, egui::Id::unique(("pane", pane)), Sense::click_and_drag());
    if (response.clicked() || response.drag_started() || response.secondary_clicked()) && pane != active {
        asked.commands.push(Command::View(ViewCommand::Activate(pane)));
    }
    viewport_menu(shell, asked, &response);
    if content != cav_engine::PaneContent::Scene {
        // A plot has no camera. On the signal plot the pointer is the playhead; on either,
        // the wheel zooms the horizontal axis about the pointer and a double click shows all of it.
        if let Some(drawn) = shell.layout.plots.iter().find(|d| d.pane == pane).cloned() {
            let value = |point: egui::Pos2| {
                let along = f64::from((point.x - drawn.area.left()) / drawn.area.width()).clamp(0.0, 1.0);
                drawn.x.lo + along * (drawn.x.hi - drawn.x.lo)
            };
            let spec = shell.engine.spec();
            let whole = match content {
                cav_engine::PaneContent::Signal | cav_engine::PaneContent::Flight => spec.duration,
                cav_engine::PaneContent::Cadence => {
                    2.5 / spec.array.schedule.mean_cycle(spec.array.instrument.cycle)
                }
                _ => 0.5 / spec.array.schedule.mean_cycle(spec.array.instrument.cycle),
            };
            let scroll = if response.hovered() { ui.input(|i| i.smooth_scroll_delta.y) } else { 0.0 };
            if response.double_clicked() {
                asked.commands.push(Command::View(ViewCommand::Span { pane, span: None }));
            } else {
                // The wheel's movement is smoothed over several frames, so a click may land
                // while it is still arriving: the two are not alternatives.
                if scroll != 0.0
                    && let Some(point) = ui.input(|i| i.pointer.hover_pos())
                {
                    let span = crate::plots::zoomed([drawn.x.lo, drawn.x.hi], value(point), scroll, whole);
                    asked.commands.push(Command::View(ViewCommand::Span { pane, span }));
                }
                if matches!(content, cav_engine::PaneContent::Signal | cav_engine::PaneContent::Flight)
                    && (response.clicked() || response.dragged())
                    && let Some(point) = response.interact_pointer_pos()
                {
                    asked.commands.push(Command::Seek(value(point)));
                }
            }
        }
        return;
    }
    crate::first_person::read_out(shell, ui, pane, rect);
    // The right button pressed over the scene flies its camera while it is held (Unity's and
    // Unreal's: W A S D move, the mouse looks). S18's review, U3.
    if response.hovered()
        && !shell.any_fly()
        && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary))
    {
        shell.mouse_fly = Some(pane);
        if pane != active {
            asked.commands.push(Command::View(ViewCommand::Activate(pane)));
        }
    }
    // A click on a body selects it; a drag that starts on one takes hold of it.
    let within = |point: egui::Pos2| (point - rect.min).to_pos2();
    let behaviour = shell.behaviour();
    // The Move tool's gizmo on the selected body, in the active pane: its handles are taken
    // before anything else under the pointer.
    let selected_body = match shell.engine.selection() {
        Some(Target::Body(n)) => Some(n - 1),
        _ => None,
    };
    let gizmo = match (behaviour, selected_body) {
        (crate::tools::Behaviour::Translate, Some(body)) if pane == active && !shell.is_locked(body) => {
            draw_gizmo(shell, ui, pane, rect, body).map(|g| (body, g))
        }
        _ => None,
    };
    if pane == active {
        asked.handle_rect = match gizmo {
            Some((_, g)) => Rect::from_center_size(g.centre + rect.min.to_vec2(), vec2(24.0, 24.0)),
            None => selected_body
                .and_then(|n| shell.body_point(pane, n, rect.size()))
                .map_or(Rect::NOTHING, |p| Rect::from_center_size(p + rect.min.to_vec2(), vec2(24.0, 24.0))),
        };
    }
    // The Place tool in hand over the scene: a grid on the ground round the pointer and a ghost
    // of the starter where a click would put it (S18's second review, UX 2: in the void there
    // was nothing to aim at).
    if behaviour == crate::tools::Behaviour::Place && pane == active {
        place_preview(shell, ui, pane, rect, response.hover_pos().map(within));
    }
    if behaviour == crate::tools::Behaviour::Probe && pane == active {
        let asked = shell.probe_at.take().map(|[x, y]| rect.min + vec2(x * rect.width(), y * rect.height()));
        let clicked = response.clicked().then(|| response.interact_pointer_pos()).flatten();
        if let Some(point) = clicked.or(asked) {
            let at = within(point);
            let size = [f64::from(rect.width()), f64::from(rect.height())];
            let picked = crate::tools::pick(
                shell.engine.view(pane),
                size,
                [f64::from(at.x), f64::from(at.y)],
                &shell.engine,
                shell.ground(),
            );
            shell.probed = Some(shell.engine.probe(picked.point));
            // The reading is the right panel's Probe tab, which comes forward (S20).
            shell.right_tab = crate::outliner::RightTab::Probe;
        }
        probe_mark(shell, ui, pane, rect);
        return;
    }
    if behaviour == crate::tools::Behaviour::Place
        && response.clicked()
        && let Some(point) = response.interact_pointer_pos()
    {
        // The Place tool: the starter goes on the ground under the pointer.
        let at = within(point);
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        let ground = crate::tools::on_ground(
            shell.engine.view(pane),
            size,
            [f64::from(at.x), f64::from(at.y)],
            shell.ground(),
        );
        let _ = shell.place_at(ground);
        return;
    }
    // A drag in the view ends a transform session first, its move kept, so the drag starts
    // from where the body is (S18's second review, I2).
    if response.drag_started_by(egui::PointerButton::Primary) && shell.session.is_some() {
        shell.end_session(true);
    }
    if response.drag_started_by(egui::PointerButton::Primary)
        && let Some((body, g)) = gizmo
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        && let Some(handle) = crate::tools::hit(&g, within(origin))
        && let Some(spec) = shell.engine.spec().bodies.get(body)
    {
        shell.gizmo_drag = Some(crate::shell::GizmoDrag {
            pane,
            body,
            handle,
            gizmo: g,
            start: spec.motion.anchor().0,
            moved: egui::Vec2::ZERO,
            edited: false,
        });
        shell.drag_under_way = false;
    }
    if let Some(drag) = shell.gizmo_drag.clone().filter(|d| d.pane == pane) {
        if response.dragged() {
            let delta = response.drag_delta();
            match drag.handle {
                crate::tools::Handle::Axis(axis) => {
                    let moved = drag.moved + delta;
                    let metres = crate::tools::snapped(
                        crate::tools::along(&drag.gizmo, axis, moved),
                        shell.snap_step(),
                    );
                    let mut anchor = drag.start;
                    anchor[axis] += metres;
                    let applied = shell.move_anchor(drag.body, anchor, drag.edited);
                    shell.gizmo_drag =
                        Some(crate::shell::GizmoDrag { moved, edited: drag.edited || applied, ..drag });
                }
                crate::tools::Handle::Centre => asked.body_drags.push((pane, drag.body, delta, rect.size())),
            }
        }
        if response.drag_stopped() {
            shell.gizmo_drag = None;
            shell.drag_under_way = false;
        }
        return;
    }
    if response.clicked()
        && let Some(point) = response.interact_pointer_pos()
        && let Some(n) = shell.body_under(pane, within(point), rect.size())
    {
        asked.commands.push(Command::Select(Some(Target::Body(n + 1))));
    }
    if response.drag_started_by(egui::PointerButton::Primary)
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        && let Some(n) = shell.body_under(pane, within(origin), rect.size())
        && !shell.is_locked(n)
    {
        shell.dragging = Some((pane, n));
        shell.drag_under_way = false;
        if shell.engine.selection() != Some(Target::Body(n + 1)) {
            asked.commands.push(Command::Select(Some(Target::Body(n + 1))));
        }
    }
    if response.dragged() {
        match shell.dragging {
            Some((held_in, n)) if held_in == pane => {
                asked.body_drags.push((pane, n, response.drag_delta(), rect.size()));
            }
            _ => {
                // The middle button pans; the right looks round from the eye (it flies).
                let panning = response.dragged_by(egui::PointerButton::Middle);
                asked.drags.push((pane, response.drag_delta(), panning, rect.size()));
            }
        }
    }
    if response.drag_stopped() {
        shell.dragging = None;
        shell.drag_under_way = false;
    }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            // Toward the point under the pointer (the pane's middle if there is none).
            let at = ui.input(|i| i.pointer.hover_pos()).unwrap_or(rect.center()) - rect.min;
            let size = [f64::from(rect.width()), f64::from(rect.height())];
            asked.zooms.push((pane, scroll, [f64::from(at.x), f64::from(at.y)], size));
        }
    }
}

/// The place of a floating viewport in the mosaic: an empty slot with a quiet button that docks
/// it back.
fn empty_slot(shell: &mut Shell, ui: &mut egui::Ui, asked: &mut Asked, pane: usize, label: char, slot: Rect) {
    ui.painter().rect_filled(slot, 0.0, theme::FOOTER);
    ui.painter().rect_stroke(slot, 0.0, Stroke::new(1.0, theme::RULE), StrokeKind::Inside);
    let size = vec2(132.0, theme::HIT).min(slot.size());
    let button = Rect::from_center_size(slot.center(), size);
    let response = ui
        .put(button, Button::new(RichText::new("floating — dock").size(theme::SMALL_PT).color(theme::WEAK)))
        .on_hover_text(format!("viewport {} ({label}) floats over the window: put it back here", pane + 1));
    shell.layout.buttons.push((format!("viewport dock {}", pane + 1), response.rect));
    if response.clicked() {
        asked.lines.push(format!("viewport dock {}", pane + 1));
    }
}

/// A viewport floating over the window (`viewport pop-out`): a frame whose header is the bar it
/// is dragged by (its pop-out button docks it) and a corner to resize it, drawn above the
/// panels. Where it is and how big is the window's state ([`Shell::floating`]); the engine
/// knows nothing of it.
fn floating_viewport(
    shell: &mut Shell,
    ctx: &egui::Context,
    surface: &mut dyn PaneSurface,
    asked: &mut Asked,
    place: Place,
    label: char,
) {
    let frame = place.rect;
    let screen = shell.layout.screen;
    let scale = ctx.pixels_per_point();
    egui::Area::new(egui::Id::unique(("floating viewport", label)))
        .order(egui::Order::Middle)
        .fixed_pos(frame.min)
        .constrain(false)
        .movable(false)
        .fade_in(false)
        .sizing_pass(false)
        .show(ctx, |ui| {
            ui.set_clip_rect(screen);
            ui.allocate_rect(frame, Sense::hover());
            // Inside the frame's edge, whole pixels, as the docked viewports are.
            let inside = frame.shrink(1.0);
            let pixels = [
                (inside.width() * scale).round().max(1.0) as u32,
                (inside.height() * scale).round().max(1.0) as u32,
            ];
            let inside = Rect::from_min_size(inside.min, vec2(pixels[0] as f32, pixels[1] as f32) / scale);
            ui.painter().rect_filled(frame, 2.0, theme::BACKGROUND);
            let pane = place.pane;
            shell.layout.panes[pane] = inside;
            let outcome = viewport(shell, ui, surface, asked, Place { rect: inside, pixels, ..place });
            // The header moves the frame; it stays where a pointer can reach it.
            let mut next = frame;
            if outcome.dragged != egui::Vec2::ZERO {
                next = next.translate(outcome.dragged);
                let reach = 48.0;
                let x = next.left().clamp(screen.left() + reach - next.width(), screen.right() - reach);
                let y = next.top().clamp(screen.top(), screen.bottom() - theme::HEADER);
                next = Rect::from_min_size(pos2(x, y), next.size());
            }
            // The corner resizes it (drawn last: it is above the picture).
            let corner = Rect::from_min_max(frame.max - vec2(14.0, 14.0), frame.max);
            let resized = ui
                .interact(corner, egui::Id::unique(("float corner", label)), Sense::drag())
                .on_hover_cursor(egui::CursorIcon::ResizeNwSe);
            shell.layout.buttons.push((format!("float corner {}", pane + 1), corner));
            if resized.dragged() {
                let size = (next.size() + resized.drag_delta()).max(MIN_FLOAT);
                next = Rect::from_min_size(next.min, size);
            }
            let grip = if resized.hovered() || resized.dragged() { theme::STRONG } else { theme::WEAK };
            for inset in [4.0, 8.0] {
                ui.painter().line_segment(
                    [frame.max - vec2(inset + 2.0, 3.0), frame.max - vec2(3.0, inset + 2.0)],
                    Stroke::new(1.0, grip),
                );
            }
            let edge = if pane == place.active { theme::SELECTION_EDGE } else { theme::WIDGET_EDGE };
            ui.painter().rect_stroke(frame, 2.0, Stroke::new(1.0, edge), StrokeKind::Inside);
            if next != frame {
                shell.floating.insert(label, next);
                ctx.request_repaint();
            }
        });
}

fn figure(shell: &mut Shell, ui: &mut egui::Ui, surface: &mut dyn PaneSurface) {
    // A whole number of pixels across and down, as the panes are made: the side panels' widths
    // may leave a fraction of a point (the inspector's is fixed since S19's review), and the one
    // pane must be the figure to the point.
    let ppp = ui.ctx().pixels_per_point();
    let raw = ui.max_rect();
    let area = Rect::from_min_size(
        raw.min,
        // Down, so that it never runs under a panel.
        vec2((raw.width() * ppp + 1e-3).floor() / ppp, (raw.height() * ppp + 1e-3).floor() / ppp),
    );
    ui.painter().rect_filled(area, 0.0, theme::BACKGROUND);
    shell.layout.figure = area;
    shell.building.place("viewport", area);
    canvas_keys(shell, area);
    let scale = ui.ctx().pixels_per_point();
    let size =
        [(area.width() * scale).round().max(1.0) as u32, (area.height() * scale).round().max(1.0) as u32];
    // The mosaic drawn is the engine's; while a splitter is dragged, the one the drag has made
    // so far (the engine hears of it once, when the drag ends). A drag the engine's mosaic has
    // changed under (a command from elsewhere) is dropped.
    if shell.splitter.as_ref().is_some_and(|drag| &drag.from != shell.engine.mosaic()) {
        shell.splitter = None;
    }
    let mosaic =
        shell.splitter.as_ref().map_or_else(|| shell.engine.mosaic().clone(), |drag| drag.now.clone());
    let labels = mosaic.labels();
    let count = labels.len();
    let active = shell.engine.active_pane();
    let generation = shell.engine.generation();
    shell.floating.retain(|label, _| labels.contains(label));
    let mut asked = Asked {
        commands: Vec::new(),
        drags: Vec::new(),
        body_drags: Vec::new(),
        zooms: Vec::new(),
        handle_rect: Rect::NOTHING,
        actions: Vec::new(),
        lines: Vec::new(),
        dropped: None,
        stops: Vec::new(),
        tools: None,
        snap: Rect::NOTHING,
    };
    // The viewport the tool palette is in, and the tool's options row.
    let host = crate::tool_palette::host(&shell.engine, shell.maximised);
    let points = |[x, y, width, height]: [u32; 4]| {
        Rect::from_min_size(
            area.min + vec2(x as f32, y as f32) / scale,
            vec2(width as f32, height as f32) / scale,
        )
    };

    // Maximised, the active pane is the whole figure and the others are not drawn (their
    // cameras and contents are kept: it is the window's state, not the engine's).
    let mut rects = mosaic.rects(size, PANE_GAP);
    if shell.maximised {
        for (pane, rect) in rects.iter_mut().enumerate() {
            *rect = if pane == active { [0, 0, size[0], size[1]] } else { [0, 0, 0, 0] };
        }
    }
    // The viewports in their places, in the labels' order; those that float are drawn after,
    // above the window.
    let mut floats: Vec<(Place, char)> = Vec::new();
    for (pane, pixels) in rects.into_iter().enumerate() {
        let slot = points(pixels);
        let place = Place {
            pane,
            rect: slot,
            pixels: [pixels[2], pixels[3]],
            count,
            active,
            generation,
            floating: false,
            tools: host == Some(pane),
        };
        shell.layout.slots.push(slot);
        shell.layout.canvases.push(Rect::NOTHING);
        match shell.floating.get(&labels[pane]).copied().filter(|_| !shell.maximised) {
            Some(frame) => {
                shell.layout.panes.push(frame);
                shell.layout.floating.push((pane, frame));
                empty_slot(shell, ui, &mut asked, pane, labels[pane], slot);
                floats.push((Place { rect: frame, floating: true, ..place }, labels[pane]));
            }
            None => {
                shell.layout.panes.push(slot);
                if pixels[2] > 0 && pixels[3] > 0 {
                    viewport(shell, ui, surface, &mut asked, place);
                }
            }
        }
    }
    if !shell.maximised {
        splitters(shell, ui, &mosaic, area, size);
    }
    for (place, label) in floats {
        floating_viewport(shell, ui.ctx(), surface, &mut asked, place, label);
    }
    let Asked {
        commands,
        drags,
        body_drags,
        zooms,
        handle_rect,
        actions,
        mut lines,
        dropped,
        stops,
        tools,
        snap,
    } = asked;
    // A header dragged over another viewport: while it is held the one under the pointer is
    // marked, and let go there the two swap places (one `swap`, the engine's).
    let other_under = |shell: &Shell, from: usize, at: egui::Pos2| {
        let floating = shell.layout.floating.iter().map(|(pane, frame)| (*pane, *frame));
        let docked = shell.layout.slots.iter().copied().enumerate();
        let floats = |pane: usize| shell.layout.floating.iter().any(|(floating, _)| *floating == pane);
        floating
            .chain(docked.filter(|(pane, _)| !floats(*pane)))
            .find(|(_, rect)| rect.contains(at))
            .filter(|(pane, _)| *pane != from)
    };
    if let Some(from) = shell.header_drag
        && let Some(at) = ui.input(|i| i.pointer.latest_pos())
        && let Some((_, rect)) = other_under(shell, from, at)
    {
        ui.painter().rect_filled(rect, 0.0, theme::SELECTION.gamma_multiply(0.25));
        ui.painter().rect_stroke(rect, 0.0, Stroke::new(2.0, theme::SELECTION_EDGE), StrokeKind::Inside);
    }
    if let Some((from, at)) = dropped
        && let Some((onto, _)) = other_under(shell, from, at)
    {
        lines.push(format!("swap {} {}", from + 1, onto + 1));
    }
    if !ui.input(|i| i.pointer.any_down()) {
        shell.header_drag = None;
    }
    // The tool palette's stops, in the order of `ui/tools.ron` whatever was drawn.
    crate::tool_palette::register(shell, tools.as_ref(), snap);
    // The selection's keyboard handle (Enter starts a transform session): a stop of the
    // viewport whatever is selected and whatever the panes show (order never depends on state:
    // S18's review, minor 5), disabled when there is no body in a scene pane to move.
    shell.register("viewport", "viewport/selection", "Selection", Kind::Session, handle_rect);
    // The mosaic as the keyboard walks it: one scope whose stops are the viewports in the
    // labels' order, wherever each stands and whether or not it floats (the order comes from
    // data, never from the layout: `docs/design/keyboard.md` §9). Enter on one makes it the
    // active viewport, as a click on it does.
    shell.register("viewport", "viewport/mosaic", "Viewports", Kind::Scope, area);
    for (pane, label) in labels.iter().enumerate() {
        let id = format!("viewport/mosaic/{}", pane + 1);
        let rect = shell.layout.panes[pane];
        shell.register(
            "viewport/mosaic",
            &id,
            &format!("Viewport {} ({label})", pane + 1),
            Kind::Button,
            rect,
        );
        if shell.take_click(&id) && pane != shell.engine.active_pane() {
            shell.apply(Command::View(ViewCommand::Activate(pane)));
        }
    }
    // The active viewport's header, as the keyboard reaches it: its controls in the order of
    // `header::CONTROLS`, whatever the viewport shows.
    let header = shell.layout.headers.iter().find(|(pane, _)| *pane == active).map_or(Rect::NOTHING, |h| h.1);
    crate::header::register(shell, &stops, header);
    let movable = matches!(shell.engine.selection(), Some(Target::Body(_)))
        && shell.engine.content(active) == cav_engine::PaneContent::Scene;
    if !movable {
        shell.building.disable("viewport/selection");
    }
    // What the renderer left out of the last picture: in the tooltip of each scene viewport's
    // warning dot, and each kind said once in the log when it first appears (the counts in it
    // change from frame to frame).
    let notes = surface.notes();
    let kind = |note: &String| note.split(':').next().unwrap_or_default().to_owned();
    for note in &notes {
        if !shell.render_notes.iter().any(|old| kind(old) == kind(note)) {
            shell.engine.note(cav_engine::Level::Warn, cav_engine::Subsystem::Render, note.clone());
        }
    }
    shell.render_notes = notes;

    for command in commands {
        shell.apply(command);
    }
    // What a right-click menu chose runs on the viewport it was opened on, now the active one.
    for action in actions {
        let _ = shell.run_action(&action);
    }
    for line in lines {
        let _ = shell.run(&line);
    }
    for (pane, delta, panning, size) in drags {
        if delta != egui::Vec2::ZERO {
            if shell.any_fly() {
                // Flying, the mouse looks by its raw motion (`Shell::fly_frame`), not by a drag.
            } else if panning {
                shell.pan_by(pane, delta, size);
            } else {
                shell.orbit_by(pane, delta);
            }
        }
    }
    for (pane, n, delta, size) in body_drags {
        if delta != egui::Vec2::ZERO {
            shell.drag_body_by(pane, n, delta, size);
        }
    }
    for (pane, scroll, pointer, size) in zooms {
        shell.zoom_by(pane, scroll, pointer, size);
    }
}

/// The splitters between the viewports: each stretch of gap between two of them is a handle
/// [`theme::GRID`] wide. A drag moves its boundary with the pointer (the layout follows here, in
/// the window) and ends as one `Mosaic` command; a double click evens its two sides.
fn splitters(shell: &mut Shell, ui: &mut egui::Ui, mosaic: &Mosaic, area: Rect, size: [u32; 2]) {
    let scale = ui.ctx().pixels_per_point();
    for (n, (boundary, [x, y, width, height])) in mosaic.splitters(size, PANE_GAP).into_iter().enumerate() {
        let gap = Rect::from_min_size(
            area.min + vec2(x as f32, y as f32) / scale,
            vec2(width as f32, height as f32) / scale,
        );
        let across = matches!(boundary, Boundary::Column(_));
        // The handle: the gap widened to the grid, so that a pointer finds it.
        let handle = if across {
            gap.expand2(vec2(((theme::GRID - gap.width()) / 2.0).max(0.0), 0.0))
        } else {
            gap.expand2(vec2(0.0, ((theme::GRID - gap.height()) / 2.0).max(0.0)))
        };
        shell.layout.splitters.push((boundary, handle));
        let response =
            ui.interact(handle, egui::Id::unique(("splitter", n, across)), Sense::click_and_drag());
        let held = shell.splitter.as_ref().is_some_and(|drag| drag.boundary == boundary);
        if response.hovered() || response.dragged() || held {
            ui.ctx().set_cursor_icon(if across {
                egui::CursorIcon::ResizeHorizontal
            } else {
                egui::CursorIcon::ResizeVertical
            });
            ui.painter().rect_filled(gap, 0.0, theme::SELECTION_EDGE);
        }
        let along = |point: egui::Pos2| if across { point.x - area.left() } else { point.y - area.top() };
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        {
            let from = shell.engine.mosaic().clone();
            let grab = along(origin) - along(gap.center());
            shell.splitter = Some(SplitterDrag { boundary, now: from.clone(), from, grab });
        }
        if response.dragged()
            && let Some(point) = response.interact_pointer_pos()
            && let Some(drag) = shell.splitter.as_mut().filter(|drag| drag.boundary == boundary)
        {
            let (extent, wanted) = (size[usize::from(!across)], (along(point) - drag.grab) * scale);
            drag.now = splitter_moved(&drag.from, boundary, wanted, extent, PANE_GAP, MIN_VIEWPORT * scale);
            ui.ctx().request_repaint();
        }
        if response.drag_stopped()
            && let Some(drag) = shell.splitter.take()
            && drag.now != drag.from
        {
            // The one command a drag is: one journal line.
            shell.apply(Command::View(ViewCommand::Mosaic { layout: drag.now, from: None }));
        }
        if response.double_clicked()
            && let Ok(evened) = shell.engine.mosaic().even_at(boundary)
            && &evened != shell.engine.mosaic()
        {
            shell.splitter = None;
            shell.apply(Command::View(ViewCommand::Mosaic { layout: evened, from: None }));
        }
    }
}

fn windows(shell: &mut Shell, ctx: &egui::Context) {
    let mut keys_open = shell.keys_open;
    if keys_open {
        shell.building.window("sheet", "Keyboard");
    }
    // The sheet fits the window (S18's review, R6): as wide as the window allows less a margin,
    // three columns when there is room, the descriptions wrapped, and what does not fit scrolls,
    // with the mouse or from the keyboard (Enter on the list, then the arrows and Page Up/Down).
    let room = ctx.content_rect();
    let width = (room.width() - 32.0).clamp(320.0, 1180.0);
    let columns = if width >= 1000.0 {
        3
    } else if width >= 640.0 {
        2
    } else {
        1
    };
    let sheet = egui::Window::new("Keyboard")
        .open(&mut keys_open)
        .collapsible(false)
        .resizable(false)
        .min_width(width)
        .max_width(width)
        .max_height((room.height() - 120.0).max(200.0))
        .pivot(Align2::CENTER_TOP)
        .default_pos(pos2(room.center().x, room.top() + 40.0))
        .show(ctx, |ui| {
            // Made from the action table and the model's own keys: a key added to the table is
            // here with no line of this code.
            let rows = crate::sheet::rows(crate::actions::table());
            // Each step of the keys is a row's height; a page is ten.
            let step = 18.0;
            let mut area = egui::ScrollArea::vertical().id_salt("sheet list").auto_shrink([false, true]);
            let scrolled: f32 =
                shell.take_adjust("sheet/list").iter().filter(|(axis, ..)| *axis >= 1).map(|(_, steps, ..)| *steps as f32).sum();
            if scrolled != 0.0 {
                area = area.vertical_scroll_offset((shell.sheet_scroll - scrolled * step).max(0.0));
            }
            let shown = area.max_height((room.height() - 190.0).max(120.0)).show(ui, |ui| {
                ui.columns(columns, |cols| {
                    // Whole groups to a column, as evenly as their rows allow.
                    let per = rows.len().div_ceil(columns);
                    let mut column = 0;
                    let mut in_column = 0;
                    let mut scope = String::new();
                    for row in &rows {
                        if row.scope != scope && in_column >= per && column + 1 < columns {
                            column += 1;
                            in_column = 0;
                        }
                        let ui = &mut cols[column];
                        if row.scope != scope {
                            scope.clone_from(&row.scope);
                            ui.add_space(if in_column == 0 { 0.0 } else { 6.0 });
                            ui.label(RichText::new(&row.scope).color(theme::GOLD).size(theme::BODY_PT));
                        }
                        ui.horizontal_top(|ui| {
                            theme::fixed_left(
                                ui,
                                vec2(136.0, 16.0),
                                egui::Label::new(
                                    RichText::new(row.keys_text()).monospace().size(theme::SMALL_PT).color(theme::STRONG),
                                )
                                .wrap(),
                            );
                            let label = ui.add(egui::Label::new(RichText::new(&row.label).size(theme::BODY_PT)).wrap());
                            if !row.about.is_empty() {
                                label.on_hover_text(&row.about);
                            }
                        });
                        in_column += 1;
                    }
                });
            });
            shell.sheet_scroll = shown.state.clamped_offset().y;
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "every key is an action, every action a console line; Ctrl+K finds them by name · Enter, then the arrows, scrolls this list",
                )
                .size(theme::SMALL_PT)
                .color(theme::WEAK),
            );
            shown.inner_rect
        });
    if let Some(window) = sheet {
        // The list is a stop that scrolls (the spec's §6 for lists: the whole list is reachable).
        let list = window.inner.unwrap_or(window.response.rect);
        shell.building.place_window("sheet", window.response.rect, window.response.layer_id);
        shell.register("sheet", "sheet/list", "Keys", Kind::Adjust, list);
    }
    shell.keys_open = keys_open;

    let mut about_open = shell.about_open;
    if about_open {
        shell.building.window("about", "About");
    }
    let about = egui::Window::new("About Cavendish")
        .open(&mut about_open)
        .collapsible(false)
        .resizable(false)
        .default_width(380.0)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(format!("Cavendish {}", cav_engine::VERSION)).color(theme::STRONG).strong(),
            );
            ui.label("A voxel world around an array of atom-interferometer gravity gradiometers.");
            ui.add_space(4.0);
            // Where this build came from: the revision stamped when the engine was compiled.
            let build = cav_engine::build();
            let dirty = match build.dirty {
                Some(true) => ", with uncommitted changes",
                Some(false) => "",
                None => ", changes unknown",
            };
            ui.label(RichText::new(format!("built from {}{dirty}", build.revision)).color(theme::WEAK));
            if let Some(adapter) = &shell.adapter {
                ui.label(RichText::new(adapter).monospace().size(theme::SMALL_PT).color(theme::WEAK));
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Numbers are set in Ubuntu Mono, © Canonical Ltd, used unmodified under the Ubuntu Font \
                     Licence 1.0 (hover for the licence; `cav licences` prints it).",
                )
                .size(theme::SMALL_PT)
                .color(theme::WEAK),
            )
            .on_hover_text(RichText::new(theme::MONO_FONT_LICENCE).size(10.0));
            ui.add_space(4.0);
            ui.label(RichText::new("The window's own commands:").size(theme::SMALL_PT));
            for (name, about) in APP_COMMANDS {
                ui.label(
                    RichText::new(format!("{name:<8} {about}"))
                        .monospace()
                        .size(theme::SMALL_PT)
                        .color(theme::WEAK),
                );
            }
        });
    if let Some(window) = about {
        shell.building.place_window("about", window.response.rect, window.response.layer_id);
        shell.register("about", "about/text", "About", Kind::ReadOnly, window.response.rect);
    }
    shell.about_open = about_open;
}
