//! Drawing the shell: the menu bar, the outliner, the properties, the figure with its header
//! strip, the timeline, the console and the footer.
//!
//! Principia's rules, from the mock-up: the figure is never covered (panels take space beside it,
//! nothing floats over it but Home); badges live in the header strip; rarely used things are in
//! windows; F3 hides everything but the figure.

use cav_engine::{Command, Level, Subsystem, Target, ViewCommand};
use cav_render::pane_rects;
use cav_sim::{DrawStyle, NamedView, Projection};
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
fn short_number(value: f64) -> String {
    if value == 0.0 || !value.is_finite() {
        return format!("{value}");
    }
    let digits = (3 - value.abs().log10().floor() as i32).max(0) as usize;
    let text = format!("{value:.digits$}");
    if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.').to_owned() } else { text }
}

/// A small outlined label for the header strip.
fn badge(ui: &mut egui::Ui, text: impl Into<String>, colour: Color32) {
    egui::Frame::new()
        .stroke(Stroke::new(1.0, theme::WIDGET))
        .corner_radius(9)
        .inner_margin(Margin::symmetric(7, 0))
        .show(ui, |ui| ui.label(RichText::new(text).size(11.5).color(colour)));
}

/// A side panel's contents in a scroll area, so that a short window scrolls them instead of
/// letting them spill over the panel below.
/// Where more lies below what is shown, the panel's foot fades and says so (S18's second review,
/// UX 8: a clipped inspector gave no sign that it scrolls). Returns whether it did.
fn scrolling(ui: &mut egui::Ui, id: &str, contents: impl FnOnce(&mut egui::Ui)) -> bool {
    // A solid scroll bar, seen whenever there is more than fits (S19's review, minor 16: egui's
    // floating one showed only under the pointer), and room after the last row so that the
    // "more below" badge never covers it once scrolled to the end.
    ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
    let shown = egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
        .show(ui, |ui| {
            contents(ui);
            ui.add_space(28.0);
        });
    let rect = shown.inner_rect;
    let below = shown.content_size.y - shown.state.clamped_offset().y - rect.height();
    if below > 4.0 {
        let painter = ui.painter_at(rect);
        let steps = 12;
        for k in 0..steps {
            let t = k as f32 / steps as f32;
            let band = Rect::from_min_max(
                pos2(rect.left(), rect.bottom() - 28.0 * (1.0 - t)),
                pos2(rect.right(), rect.bottom() - 28.0 * (1.0 - t) + 28.0 / steps as f32 + 0.5),
            );
            painter.rect_filled(band, 0.0, theme::PANEL.gamma_multiply(t));
        }
        // On a solid band of its own, in the text's colour: over the faded rows it was too faint
        // to read (S18's third review, minor 9).
        let galley =
            painter.layout_no_wrap("more below: scroll".to_owned(), FontId::proportional(10.5), theme::TEXT);
        let at = pos2(rect.center().x - 0.5 * galley.size().x, rect.bottom() - 3.0 - galley.size().y);
        painter.rect_filled(
            Rect::from_min_size(at, galley.size()).expand2(egui::vec2(6.0, 1.0)),
            3.0,
            theme::WIDGET,
        );
        painter.galley(at, galley, theme::TEXT);
    }
    below > 4.0
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
            .exact_size(lanes_height + 34.0)
            .frame(frame(theme::PANEL, 8, 4))
            .show(ui, |ui| timeline_panel(shell, ui));
        shell.layout.panels.push(("timeline", panel.response.rect));
        shell.building.place("timeline", panel.response.rect);
        // The outliner gives way to the figure on a small window (S18: the tool strip and the
        // options strip took 72 points from it); the inspector's rows need their 264.
        // Its edge can be dragged (egui keeps the width it is left at); the inspector's too.
        let side = (shell.layout.screen.width() * 0.19).clamp(210.0, 262.0);
        let panel = Panel::left("outliner")
            .default_size(side)
            .size_range(180.0..=520.0)
            .resizable(true)
            .frame(frame(theme::PANEL, 10, 6))
            .show(ui, |ui| scrolling(ui, "outliner", |ui| crate::outliner::outliner(shell, ui)));
        shell.layout.panels.push(("outliner", panel.response.rect));
        shell.building.place("tree", panel.response.rect);
        let panel = Panel::right("properties")
            // Wide enough for the cards' one grid (a 96 pt label column and X, Y, Z boxes): S19's
            // panels; the tool strip's column it replaces gave the figure its 52 points back.
            .default_size(INSPECTOR_WIDTH)
            .size_range(INSPECTOR_WIDTH..=560.0)
            .resizable(true)
            .frame(frame(theme::PANEL, 10, 6))
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
            shell.layout.buttons.push(("more below".to_owned(), panel.response.rect));
        }
        shell.layout.panels.push(("properties", panel.response.rect));
        shell.building.place("inspector", panel.response.rect);
    }
    egui::CentralPanel::no_frame().show(ui, |ui| {
        if !shell.gui_hidden && !shell.maximised {
            let strip = Panel::top("header")
                .exact_size(24.0)
                .frame(frame(theme::FOOTER, 10, 0))
                .show(ui, |ui| header(shell, ui));
            shell.layout.panels.push(("header", strip.response.rect));
            let options = Panel::top("options")
                .exact_size(24.0)
                .frame(frame(theme::PANEL, 10, 0))
                .show(ui, |ui| tool_options(shell, ui));
            shell.layout.panels.push(("options", options.response.rect));
        }
        egui::CentralPanel::no_frame().show(ui, |ui| figure(shell, ui, surface));
    });
    windows(shell, &ctx);
    if shell.quit {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

/// The inspector's panel: its tabs (the selection's properties, or the towers' readings) and the
/// tab shown, scrolling. Returns whether more lies below what is shown.
fn inspector_panel(shell: &mut Shell, ui: &mut egui::Ui) -> bool {
    ui.horizontal(|ui| {
        for (tab, name) in [
            (crate::outliner::RightTab::Inspector, "Inspector"),
            (crate::outliner::RightTab::Readings, "Readings"),
        ] {
            let response = tab_title(ui, name, shell.right_tab == tab);
            let id = format!("inspector/tab/{name}");
            shell.register("inspector", &id, name, Kind::Button, response.rect);
            shell.layout.buttons.push((format!("tab {name}"), response.rect));
            if response.clicked() || shell.take_click(&id) {
                shell.right_tab = tab;
            }
        }
    });
    ui.add_space(4.0);
    match shell.right_tab {
        crate::outliner::RightTab::Inspector => scrolling(ui, "properties", |ui| properties(shell, ui)),
        crate::outliner::RightTab::Readings => {
            scrolling(ui, "readings", |ui| crate::outliner::readings(shell, ui))
        }
    }
}

fn menu_bar(shell: &mut Shell, ui: &mut egui::Ui) {
    // The brand, the menus and the status share the menu bar's own row, so that the status on the
    // right is laid out in what the menus leave and cannot be pushed off the window.
    egui::MenuBar::new().ui(ui, |ui| {
        ui.label(RichText::new("cavendish · dev").color(theme::STRONG).strong());
        ui.add_space(6.0);
        for title in crate::menus::titles() {
            let response = ui.menu_button(&title, |ui| crate::menus::draw_items(shell, ui, &title)).response;
            // From the keyboard a menu opens as the palette, filtered to it: its items, searchable.
            let id = format!("top-bar/menu/{title}");
            shell.register("top-bar", &id, &title, Kind::Button, response.rect);
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
        // The layouts (`ui/layouts.ron`) as tabs: each arranges the figure for a task.
        ui.separator();
        for layout in &crate::layouts::layouts().layouts {
            let tab = ui
                .selectable_label(shell.layout_name == layout.id, RichText::new(&layout.label).size(12.0))
                .on_hover_text(&layout.about);
            let id = format!("top-bar/layout/{}", layout.id);
            let line = format!("layout {}", layout.id);
            shell.register("top-bar", &id, &layout.label, Kind::Button, tab.rect);
            shell.layout.buttons.push((line.clone(), tab.rect));
            if tab.clicked() || shell.take_click(&id) {
                let _ = shell.run(&line);
            }
        }
        ui.separator();
        let search = ui
            .add(Button::new(RichText::new("search  Ctrl+K").size(11.5).color(theme::WEAK)).frame(false))
            .on_hover_text("the command palette: every action, starter, scene, command and field");
        shell.register("top-bar", "top-bar/search", "Search", Kind::Button, search.rect);
        shell.layout.buttons.push(("palette".to_owned(), search.rect));
        if search.clicked() || shell.take_click("top-bar/search") {
            let _ = shell.run("palette");
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let spec = shell.engine.spec();
            // The scene's name, with `*` while it has edits not saved.
            let star = if shell.unsaved() { " *" } else { "" };
            let status =
                format!("{}{star} · t {:.2} / {:.0} s", spec.name, shell.engine.time(), spec.duration);
            ui.add(egui::Label::new(RichText::new(status).monospace().color(theme::TEXT)).truncate());
            // Where the keys go, while keyboard navigation is on: the breadcrumb (the spec's §10).
            if let Some(crumb) = shell.breadcrumb() {
                let colour = match shell.keyboard.mode {
                    crate::keyboard::Mode::Navigation => crate::shell::RING_NAVIGATION,
                    crate::keyboard::Mode::Interaction => crate::shell::RING_INTERACTION,
                };
                ui.add(egui::Label::new(RichText::new(crumb).size(11.5).color(colour)).truncate());
            }
        });
    });
}

fn footer(shell: &mut Shell, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        // The newest thing worth knowing: the last entry that is not routine.
        let last = shell
            .engine
            .log()
            .iter()
            .rev()
            .find(|e| e.level >= Level::Info && e.subsystem != Subsystem::App)
            .cloned();
        let (text, colour) = last.map_or(("ready".to_owned(), theme::TEXT), |entry| {
            let colour = match entry.level {
                Level::Error => theme::ERROR,
                Level::Warn => theme::WARN,
                _ => theme::TEXT,
            };
            (entry.message, colour)
        });
        let (warnings, errors) = shell.problem_counts();
        let forward = forward_status(&shell.engine);
        shell.layout.status.clone_from(&forward);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let label = if shell.console_open { "close console" } else { "console" };
            let console =
                ui.add(Button::new(RichText::new(label).size(11.5)).frame(false)).on_hover_text("`");
            shell.register("footer", "footer/console", "Console", Kind::Button, console.rect);
            if console.clicked() || shell.take_click("footer/console") {
                let _ = shell.run("console");
            }
            ui.label(
                RichText::new(if shell.playing { "Space pause" } else { "Space play" })
                    .size(11.5)
                    .color(theme::WEAK),
            );
            ui.label(RichText::new(&forward).size(11.5).color(theme::WEAK));
            if let Some(ms) = shell.frame_ms {
                ui.label(RichText::new(format!("{ms:.1} ms")).monospace().size(11.0).color(theme::WEAK));
            }
            if errors > 0 {
                ui.label(
                    RichText::new(format!("{errors} error{}", if errors == 1 { "" } else { "s" }))
                        .size(11.5)
                        .color(theme::ERROR),
                );
            }
            if warnings > 0 {
                ui.label(
                    RichText::new(format!("{warnings} warning{}", if warnings == 1 { "" } else { "s" }))
                        .size(11.5)
                        .color(theme::WARN),
                );
            }
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
                        ui.add(
                            egui::Label::new(RichText::new(words).size(11.5).color(theme::STRONG)).truncate(),
                        );
                        let (label, hover) = if back == "undo" {
                            ("Undo", "take this edit back (Ctrl+Z)")
                        } else {
                            ("Redo", "put it back (Ctrl+Shift+Z)")
                        };
                        let link = ui
                            .add(Button::new(RichText::new(label).size(11.5).color(theme::LINK)).frame(false))
                            .on_hover_text(hover);
                        shell.register("footer", "footer/undo", label, Kind::Button, link.rect);
                        shell.layout.buttons.push((format!("toast {back}"), link.rect));
                        if link.clicked() || shell.take_click("footer/undo") {
                            let _ = shell.run(back);
                        }
                    }
                    None => {
                        ui.add(egui::Label::new(RichText::new(text).size(11.5).color(colour)).truncate());
                    }
                }
            });
        });
    });
}

/// The forward stage's state in a line: how many shots, refined or computing, and the
/// quadrature's error. The status bar's (it was the outliner's foot before S19's panels).
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
            format!(
                "forward · {count} · refined · {}",
                cav_engine::estimate_text(signal.estimate, signal.settled, signal.truncated)
            )
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
                let font = FontId::monospace(11.5);
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
    ui.horizontal(|ui| {
        ui.label(RichText::new("Timeline").color(theme::STRONG).strong());
        let play =
            ui.add(Button::new(if shell.playing { "pause" } else { "play" }).min_size(vec2(48.0, 0.0)));
        shell.register("timeline", "timeline/play", "Play", Kind::Button, play.rect);
        if play.clicked() || shell.take_click("timeline/play") {
            let _ = shell.run("play");
        }
        let rewind = ui.button("rewind");
        shell.register("timeline", "timeline/rewind", "Rewind", Kind::Button, rewind.rect);
        if rewind.clicked() || shell.take_click("timeline/rewind") {
            let _ = shell.run("seek 0");
        }
        ui.label(RichText::new(format!("t {:.2} s", shell.engine.time())).monospace().color(theme::STRONG));
        ui.label(RichText::new(caption).size(11.0).color(theme::WEAK));
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
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::proportional(13.0), colour);
    let (rect, response) = ui.allocate_exact_size(galley.size() + vec2(16.0, 10.0), Sense::click());
    if active {
        ui.painter().rect_filled(rect, egui::CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 }, theme::WIDGET);
        ui.painter().hline(rect.x_range(), rect.bottom(), Stroke::new(2.0, theme::SELECTION_EDGE));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 4.0, theme::HOVER);
    }
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, colour);
    response
}

/// What an empty scene says, and the ways to begin, in the tree under its Bodies.
pub(crate) fn empty_guidance(shell: &mut Shell, ui: &mut egui::Ui, spec: &cav_sim::SceneSpec) {
    let text = match (spec.world.is_some(), spec.floor.is_some()) {
        (false, false) => "Nothing for the towers to read yet.",
        _ => "No bodies: the surroundings are read.",
    };
    ui.horizontal_wrapped(|ui| {
        ui.add_space(30.0);
        ui.label(RichText::new(text).size(12.0).color(theme::WEAK));
    });
    ui.horizontal_wrapped(|ui| {
        ui.add_space(30.0);
        ui.spacing_mut().item_spacing.x = 4.0;
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
    ui.separator();
    ui.menu_button("Add", |ui| crate::menus::add_menu(shell, ui));
}

/// The text field that renames an item in place (`F2`, a double click): Enter keeps the name
/// (a `set` of its label: one edit, undone as one), Esc leaves it as it was.
/// The name of a row being renamed (F2), typed in place.
pub(crate) fn rename_field_at(shell: &mut Shell, ui: &mut egui::Ui, target: Target, rect: Rect) {
    let Some((_, mut text)) = shell.renaming.clone() else { return };
    let response = ui.put(rect, egui::TextEdit::singleline(&mut text).font(FontId::proportional(13.0)));
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

/// A row of small buttons, each running a console line; the one that matches the state is lit.
/// Each is a button of the keyboard's scope `scope`, its id `scope` + its line.
fn button_row(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    scope: &str,
    label: &str,
    buttons: &[(&str, &str, bool)],
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        ui.spacing_mut().button_padding.x = 5.0;
        theme::fixed_left(
            ui,
            vec2(52.0, 18.0),
            egui::Label::new(RichText::new(label).color(theme::WEAK).size(12.0)),
        );
        for &(text, line, on) in buttons {
            let response =
                ui.add(Button::new(RichText::new(text).size(12.0)).selected(on)).on_hover_text(line);
            shell.layout.buttons.push((line.to_owned(), response.rect));
            let id = format!("{scope}/{line}");
            shell.register(scope, &id, &format!("{label} {text}"), Kind::Button, response.rect);
            if response.clicked() || shell.take_click(&id) {
                let _ = shell.run(line);
            }
        }
    });
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

/// How the active pane looks: its direction, projection, panes, draw style, what it shows, the
/// voxel colouring, overlays and section, and its camera. A popover of the header strip (`view
/// ▾`): rarely changed, so out of the scene tree (`docs/research/editor-ux.md` §3.2). Every row is
/// a console line; the View menu and the palette have the same.
fn view_controls(shell: &mut Shell, ui: &mut egui::Ui) {
    ui.set_min_width(330.0);
    let pane = shell.engine.active_pane();
    let view = shell.engine.view(pane).clone();
    let at = |named: NamedView| view.named() == Some(named);
    button_row(
        shell,
        ui,
        "top-bar/view",
        "look",
        &[
            ("iso", "view iso", at(NamedView::Iso)),
            ("top", "view top", at(NamedView::Top)),
            ("btm", "view bottom", at(NamedView::Bottom)),
            ("N", "view north", at(NamedView::North)),
            ("S", "view south", at(NamedView::South)),
            ("E", "view east", at(NamedView::East)),
            ("W", "view west", at(NamedView::West)),
        ],
    );
    button_row(
        shell,
        ui,
        "top-bar/view",
        "project",
        &[
            ("perspective", "proj persp", view.projection == Projection::Perspective),
            ("orthographic", "proj ortho", view.projection == Projection::Orthographic),
        ],
    );
    let panes = shell.engine.panes();
    button_row(
        shell,
        ui,
        "top-bar/view",
        "panes",
        &[("1", "panes 1", panes == 1), ("2", "panes 2", panes == 2), ("4", "panes 4", panes == 4)],
    );
    let style = |s: DrawStyle| view.style == s;
    button_row(
        shell,
        ui,
        "top-bar/view",
        "draw",
        &[
            ("shaded", "style shaded", style(DrawStyle::Shaded)),
            ("flat", "style flat", style(DrawStyle::Flat)),
            ("x-ray", "style xray", style(DrawStyle::XRay)),
        ],
    );
    button_row(
        shell,
        ui,
        "top-bar/view",
        "",
        &[
            ("wireframe", "style wireframe", style(DrawStyle::Wireframe)),
            ("paper", "style paper", style(DrawStyle::Paper)),
        ],
    );
    // A button for every content there is, three to a row: a content added to
    // `PaneContent::ALL` has its button with no line here.
    let shown = shell.engine.content(pane);
    let lines = cav_engine::PaneContent::ALL.map(|content| format!("show {}", content.name()));
    for (row, contents) in cav_engine::PaneContent::ALL.chunks(3).enumerate() {
        let buttons: Vec<(&str, &str, bool)> = contents
            .iter()
            .enumerate()
            .map(|(n, content)| (content.name(), lines[3 * row + n].as_str(), *content == shown))
            .collect();
        button_row(shell, ui, "top-bar/view", if row == 0 { "show" } else { "" }, &buttons);
    }
    voxel_rows(shell, ui, &view);
    ui.add_space(4.0);
    rows(
        ui,
        "camera",
        &[
            ("target", format!("{:.2}, {:.2}, {:.2} m", view.target[0], view.target[1], view.target[2])),
            ("yaw · pitch", format!("{:.1}° · {:.1}°", view.yaw, view.pitch)),
            ("distance", format!("{:.2} m", view.distance)),
        ],
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "drag to orbit · middle-drag to pan · scroll to zoom · hold the right button to fly (W A S D) · drag a body to move it · O swaps the projection · Home resets",
        )
        .size(11.0)
        .color(theme::WEAK),
    );
}

/// The View panel's rows for voxel bodies: what colours them, the overlays (the whole octree by
/// depth among them) and the section plane with a slider for its place. Each is the console's
/// command, so each is journalled.
fn voxel_rows(shell: &mut Shell, ui: &mut egui::Ui, view: &cav_sim::ViewSpec) {
    use cav_sim::voxel_view::{Fade, LeafField};
    if !shell.engine.spec().bodies.iter().any(|b| b.shape.is_some()) {
        return;
    }
    let field = view.colour.field;
    let colour = |f: LeafField| (f.name(), format!("colour {}", f.name()), field == f);
    let colours = [LeafField::Level, LeafField::Depth, LeafField::Mass, LeafField::Fill, LeafField::Material]
        .map(colour);
    let buttons: Vec<(&str, &str, bool)> = colours.iter().map(|(t, l, on)| (*t, l.as_str(), *on)).collect();
    button_row(shell, ui, "top-bar/view", "colour", &buttons);
    let tree = view.overlay.tree;
    let fade = tree.map(|t| t.fade);
    let plain = !view.overlay.leaves && view.overlay.cut.is_none();
    button_row(
        shell,
        ui,
        "top-bar/view",
        "overlay",
        &[
            ("none", "overlay none", plain && tree.is_none()),
            ("leaves", "overlay leaves", view.overlay.leaves && tree.is_none()),
            ("tree", "overlay tree", plain && fade == Some(Fade::Off)),
            ("tree, deep solid", "overlay tree fade deep", plain && fade == Some(Fade::Deep)),
        ],
    );
    let time = shell.engine.time();
    let on = view.section.map(|s| s.axis);
    let line = |axis: cav_sim::Axis, word: &str| {
        let (lo, hi) =
            cav_sim::voxel_view::section_span(shell.engine.spec(), axis, time).unwrap_or((0.0, 0.0));
        format!("section {word} {}", 0.5 * (lo + hi))
    };
    let lines = [line(cav_sim::Axis::X, "x"), line(cav_sim::Axis::Y, "y"), line(cav_sim::Axis::Z, "z")];
    button_row(
        shell,
        ui,
        "top-bar/view",
        "section",
        &[
            ("off", "section off", on.is_none()),
            ("x", &lines[0], on == Some(cav_sim::Axis::X)),
            ("y", &lines[1], on == Some(cav_sim::Axis::Y)),
            ("z", &lines[2], on == Some(cav_sim::Axis::Z)),
        ],
    );
    if let Some(section) = view.section
        && let Some((lo, hi)) = cav_sim::voxel_view::section_span(shell.engine.spec(), section.axis, time)
    {
        let mut offset = section.offset;
        let slider = egui::Slider::new(&mut offset, lo..=hi).suffix(" m").max_decimals(3).text("place");
        if ui.add(slider).changed() {
            let word = ["x", "y", "z"][section.axis as usize];
            let _ = shell.run(&format!("section {word} {offset}"));
        }
    }
}

/// A two-column table of names and values.
fn rows(ui: &mut egui::Ui, id: &str, rows: &[(&str, String)]) {
    egui::Grid::new(id).num_columns(2).spacing([10.0, 3.0]).show(ui, |ui| {
        for (name, value) in rows {
            ui.label(RichText::new(*name).color(theme::WEAK).size(12.0));
            ui.label(RichText::new(value).monospace().color(theme::STRONG));
            ui.end_row();
        }
    });
}

/// The inspector's width at first and its least, points: room for the cards' one grid with
/// 24-point targets and a solid scroll bar (S19's review: 316 grew to 352 with a body's rows, so
/// this is what a body needs). The label column is 40 % of this, not of the panel as dragged. The
/// panel no longer grows with what it holds (its contents are drawn in a child of fixed width),
/// so a dragged width is kept whatever is selected.
const INSPECTOR_WIDTH: f32 = 352.0;

fn properties(shell: &mut Shell, ui: &mut egui::Ui) {
    let spec = shell.engine.spec().clone();
    // One label column for every card: 40 % of the panel, at least 96 points (the card's own
    // margin taken off), so every value starts at the same x.
    shell.label_width = crate::inspector::label_width(INSPECTOR_WIDTH - 18.0);
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
    // Every target in the panel at least 24 points a side (docs/design/panels.md; S19's review,
    // I6): boxes, ticks and drop-downs take their height from this.
    // The least width of a box is a target's too, not egui's 40: a vector's three boxes share
    // what the row has (the panel keeps its width).
    ui.spacing_mut().interact_size = egui::vec2(theme::HIT, theme::HIT);
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
            egui::Label::new(RichText::new(name).size(12.0).color(theme::WEAK)),
        );
        let mut chosen = None;
        let combo = egui::ComboBox::from_id_salt(("preset", line))
            .width((ui.available_width() - 24.0).clamp(72.0, 160.0))
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

fn header(shell: &mut Shell, ui: &mut egui::Ui) {
    // The strip keeps its width whatever it holds: badges past its end scroll (the wheel or a
    // drag), never pushing the strip over the panels beside it.
    egui::ScrollArea::horizontal()
        .id_salt("header badges")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| header_badges(shell, ui));
}

fn header_badges(shell: &mut Shell, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        // How the pane looks, in a popover: the header strip is the mock-up's home for it.
        let popover = ui
            .menu_button(RichText::new("View ›").color(theme::STRONG), |ui| view_controls(shell, ui))
            .response
            .on_hover_text(
                "look, projection, panes, draw style, what the pane shows, voxel colouring, overlays",
            );
        shell.layout.buttons.push(("view menu".to_owned(), popover.rect));
        shell.register("viewport", "viewport/view-menu", "View", Kind::Button, popover.rect);
        if shell.take_click("viewport/view-menu") {
            let _ = shell.run("palette view");
        }
        // The camera's mode: orbit about the target (the default), or fly from the eye.
        let flying = shell.any_fly();
        let mode = ui
            .menu_button(
                RichText::new(if flying { "fly ›" } else { "orbit ›" }).color(theme::STRONG),
                |ui| {
                    if ui.selectable_label(!flying, "Orbit: drag turns about the target").clicked() && flying
                    {
                        let _ = shell.run("fly");
                        ui.close();
                    }
                    if ui
                        .selectable_label(
                            flying,
                            "Fly: W A S D, E Q, drag to look (Shift+F; or hold the right button)",
                        )
                        .clicked()
                        && !flying
                    {
                        let _ = shell.run("fly");
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Frame the selection (F)").clicked() {
                        let _ = shell.run("frame");
                        ui.close();
                    }
                },
            )
            .response
            .on_hover_text("the camera's mode");
        shell.layout.buttons.push(("camera mode".to_owned(), mode.rect));
        shell.register("viewport", "viewport/camera-mode", "Camera mode", Kind::Button, mode.rect);
        if shell.take_click("viewport/camera-mode") {
            let _ = shell.run("fly");
        }
        let pane = shell.engine.active_pane();
        let view = shell.engine.view(pane);
        badge(ui, shell.engine.spec().name.clone(), theme::GOLD);
        let direction = view.named().map_or("free", NamedView::name);
        let projection =
            if view.projection == Projection::Perspective { "perspective" } else { "orthographic" };
        match shell.engine.content(pane) {
            cav_engine::PaneContent::Scene => {
                badge(ui, format!("{direction} · {projection}"), theme::STRONG);
                badge(ui, view.style.name(), theme::STRONG);
            }
            plot => badge(ui, plot.name(), theme::STRONG),
        }
        if shell.engine.panes() > 1 {
            badge(ui, format!("pane {} of {}", pane + 1, shell.engine.panes()), theme::LINK);
        }
        // The instrument, when it is a preset; and the one the scene is also being read on.
        if let Some(name) = cav_sim::presets::name_of(&shell.engine.spec().array.instrument) {
            badge(ui, name, theme::STRONG);
        }
        if let Some(other) = shell.engine.comparison() {
            let name = cav_sim::presets::name_of(other).unwrap_or("another instrument");
            badge(ui, format!("compared with {name}"), theme::LINK);
        }
        // Where the signal stands: a stale one is never mistaken for the current scene's.
        match shell.engine.signal() {
            (cav_engine::SignalState::Refined, Some(signal)) => {
                // A signal some of whose reads did not settle is the scene's, and is not good to
                // the tolerance: the badge says which, and never "within" for the second.
                let text = format!(
                    "signal · {}",
                    cav_engine::estimate_text(signal.estimate, signal.settled, signal.truncated)
                );
                badge(ui, text, if signal.settled { theme::STRONG } else { theme::WARN });
            }
            (cav_engine::SignalState::Stale, _) => badge(ui, "signal · stale", theme::WARN),
            _ => {}
        }
        // What the renderer thinned to keep a huge scene drawable is said here, not passed over.
        for note in cav_render::decimation(shell.engine.spec()) {
            badge(ui, note, theme::WARN);
        }
        // What the world's renderer left out of the last picture (a cut far field, a cap).
        for note in &shell.render_notes {
            badge(ui, note.clone(), theme::WARN);
        }
        // What the voxel view drew in place of the leaves (a coarser cut), or could not colour.
        if let Ok(frame) = shell.engine.frame(pane)
            && let Some(voxels) = frame.voxels
        {
            for note in &voxels.notes {
                badge(ui, note.clone(), theme::WARN);
            }
        }
        if let Some(target) = shell.engine.selection() {
            let text = match target {
                Target::Tower(n) => shell.engine.spec().array.towers[n - 1].label.clone(),
                Target::Room => "room".into(),
                Target::Floor => "floor".into(),
                Target::World => "world".into(),
                Target::Body(n) => shell.engine.spec().bodies[n - 1].label.clone(),
            };
            badge(ui, format!("selected: {text}"), theme::STRONG);
        }
    });
}

/// The tool strip (`ui/tools.ron`): one button a tool, an icon with its key in the corner, the
/// one in hand lit; then, apart and drawn as a switch, the snap toggle (S18's review, U2: the
/// strip was abbreviated text, and snap looked like a fourth tool). Each is a button of the
/// keyboard's Tools scope; a click and Enter both run its line. The tooltip says the tool's one
/// name, its key and what it does.
fn tool_strip(shell: &mut Shell, ui: &mut egui::Ui) {
    let table = crate::actions::table();
    ui.spacing_mut().item_spacing.y = 4.0;
    for tool in &crate::tools::strip().tools {
        let line = format!("tool {}", tool.id);
        let key = table
            .actions
            .iter()
            .find(|a| a.line == line)
            .map(crate::actions::Action::shortcut_text)
            .unwrap_or_default();
        let held = shell.tool == tool.id;
        let response = icon_button(ui, held, &key, |painter, rect, colour| {
            crate::icons::tool(painter, rect, tool.behaviour, colour);
        })
        .on_hover_text(format!("{} ({key})\n{}", tool.label, tool.about));
        let id = format!("toolbar/{}", tool.id);
        shell.register("toolbar", &id, &tool.label, Kind::Button, response.rect);
        shell.layout.buttons.push((line.clone(), response.rect));
        if response.clicked() || shell.take_click(&id) {
            let _ = shell.run(&line);
        }
    }
}

/// A square button of the tool strip: an icon drawn by `icon` (in the colour the button's state
/// gives) and `key` small in its bottom-right corner; lit when `selected`.
fn icon_button(
    ui: &mut egui::Ui,
    selected: bool,
    key: &str,
    icon: impl FnOnce(&egui::Painter, Rect, Color32),
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
    let visuals = ui.style().interact_selectable(&response, selected);
    let painter = ui.painter_at(rect);
    painter.rect(rect, 4.0, visuals.weak_bg_fill, visuals.bg_stroke, StrokeKind::Inside);
    let colour = if selected { Color32::WHITE } else { theme::STRONG };
    icon(&painter, Rect::from_center_size(rect.center() - vec2(3.0, 2.0), vec2(20.0, 20.0)), colour);
    painter.text(
        rect.right_bottom() - vec2(4.0, 3.0),
        Align2::RIGHT_BOTTOM,
        key,
        FontId::monospace(9.5),
        // On the button's ground, not the panel's: the lighter grey (S19's review, I6: the weak
        // grey there read at 3.3:1).
        if selected { theme::STRONG } else { theme::WEAK_ON_WIDGET },
    );
    response
}

/// The tool's options, in a strip above the figure (Algodoo's and Blender's): what the tool does
/// and its settings, and a transform session's state while one is under way.
fn tool_options(shell: &mut Shell, ui: &mut egui::Ui) {
    // As the header: a strip of fixed width; what is past its end scrolls.
    egui::ScrollArea::horizontal()
        .id_salt("tool options")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| tool_options_row(shell, ui));
}

/// The snap switch, in the tool's options row (S19's panels: beside the hint, not in the strip):
/// a setting of the Move tool and the transform session, not a tool.
fn snap_switch(shell: &mut Shell, ui: &mut egui::Ui) {
    let step = crate::tools::strip().snap_step;
    let on = shell.snap;
    let text = if on { format!("Snap {step} m") } else { "Snap off".to_owned() };
    let colour = if on { theme::GOLD } else { theme::WEAK };
    let snap = ui
        .add(Button::new(RichText::new(text).size(11.5).color(colour)).selected(on))
        .on_hover_text("snap moves to the step, or not (S)");
    shell.register("toolbar", "toolbar/snap", "Snap", Kind::Button, snap.rect);
    shell.layout.buttons.push(("snap".to_owned(), snap.rect));
    if snap.clicked() || shell.take_click("toolbar/snap") {
        let _ = shell.run("snap");
    }
}

fn tool_options_row(shell: &mut Shell, ui: &mut egui::Ui) {
    use crate::tools::Behaviour;
    ui.horizontal_centered(|ui| {
        snap_switch(shell, ui);
        let tool = crate::tools::strip().tool(&shell.tool).cloned();
        if let Some(tool) = &tool {
            ui.label(RichText::new(&tool.label).color(theme::GOLD).size(12.0));
        }
        let weak = |text: String| RichText::new(text).size(11.5).color(theme::WEAK);
        if let Some(session) = shell.session.clone() {
            let [dx, dy, dz] = session.offset;
            let lock = session.lock.map_or(String::new(), |a| format!(" · locked to {}", ["x", "y", "z"][a]));
            let typed = if session.typed.is_empty() {
                String::new()
            } else {
                format!(" · typing {} = {} m", ["x", "y", "z"][session.lock.unwrap_or(session.typed_axis)], session.typed)
            };
            ui.label(
                RichText::new(format!("moving by ({dx:+.3}, {dy:+.3}, {dz:+.3}) m{lock}{typed}"))
                    .size(12.0)
                    .color(crate::shell::RING_INTERACTION),
            );
            ui.label(weak("arrows move · X Y Z lock · type a distance, Tab: next axis · Enter keeps · Esc cancels".into()));
            return;
        }
        match tool.map(|t| t.behaviour) {
            Some(Behaviour::Translate) => {
                ui.label(weak(
                    "drag an arrow to move along it, the square to move freely · Enter on the selection moves it by keys".into(),
                ));
            }
            Some(Behaviour::Place) => {
                let starters = cav_sim::starters::starters();
                let current = starters.iter().find(|s| s.name == shell.place_starter).map_or("?", |s| s.label.as_str());
                let mut chosen = None;
                let combo = egui::ComboBox::from_id_salt("place starter").selected_text(current).width(150.0).show_ui(ui, |ui| {
                    for starter in starters {
                        if ui.selectable_label(starter.name == shell.place_starter, &starter.label).clicked() {
                            chosen = Some(starter.name.clone());
                        }
                    }
                });
                shell.layout.buttons.push(("place starter".to_owned(), combo.response.rect));
                if let Some(name) = chosen {
                    shell.place_starter = name;
                }
                ui.label(weak(format!("click the ground (z = {} m) to add one there", shell.ground())));
            }
            Some(Behaviour::Probe) => {
                ui.label(weak("click a point: the gravity there of the bodies (and a slab floor), the cell under it and its share of each tower's phase".into()));
            }
            _ => {
                ui.label(weak("click selects · drag a body to move it in the view's plane · middle-drag pans · hold the right button and W A S D to fly".into()));
            }
        }
    });
}

/// The Probe tool's reading as a card in the pane's top left corner, and its point marked in the
/// view: what is there, in words, a line a fact.
fn probe_card(shell: &mut Shell, ui: &mut egui::Ui, pane: usize, rect: Rect) {
    let Some(probed) = shell.probed.clone() else { return };
    let painter = ui.painter_at(rect);
    if let Some(p) = shell.point_on_screen(pane, probed.point, rect.size()) {
        let p = p + rect.min.to_vec2();
        painter.circle_stroke(p, 6.0, Stroke::new(3.0, Color32::from_black_alpha(160)));
        painter.circle_stroke(p, 6.0, Stroke::new(1.5, theme::GOLD));
        painter.circle_filled(p, 1.5, theme::GOLD);
    }
    // A card as the inspector's are (S19's review, UX: it was a block of monospace): the reading
    // in rows with units, then each tower's distance and the cell's share of its phase as a
    // table. Right of the floating tool strip, below the pane's top edge.
    let spec = shell.engine.spec().clone();
    let at = rect.left_top() + vec2(72.0, 14.0);
    let room = Rect::from_min_max(at, pos2(rect.right() - 8.0, rect.bottom() - 8.0));
    if room.width() < 120.0 || room.height() < 60.0 {
        return;
    }
    let mut child =
        ui.new_child(egui::UiBuilder::new().max_rect(room).layout(egui::Layout::top_down(egui::Align::Min)));
    let card = egui::Frame::new()
        .fill(crate::inspector::CARD.gamma_multiply(0.96))
        .stroke(Stroke::new(1.0, theme::WIDGET_EDGE))
        .corner_radius(5.0)
        .inner_margin(Margin::same(8))
        .show(&mut child, |ui| {
            ui.set_max_width(330.0f32.min(room.width() - 16.0));
            let [x, y, z] = probed.point;
            ui.horizontal(|ui| {
                ui.label(RichText::new("Probe").size(13.0).color(theme::STRONG));
                ui.label(
                    RichText::new(format!(
                        "at {}, {}, {} m",
                        short_number(x),
                        short_number(y),
                        short_number(z)
                    ))
                    .size(12.0)
                    .color(theme::WEAK),
                );
            });
            let g = cav_core::Vec3::from_array(probed.g);
            let size = g.length();
            // Nothing counted where something is left out: not known, not zero (S19's second
            // review, minor 12).
            let unknown = probed.reads_nothing();
            let gravity = if unknown { "—".to_owned() } else { si(size, "m/s²") };
            let mut rows: Vec<(&str, String)> = vec![("Gravity", gravity)];
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
                    rows.push((
                        "Cell",
                        format!(
                            "{} · {} kg · {} m a side",
                            c.material.as_deref().unwrap_or("uniform"),
                            crate::editors::shown(c.mass),
                            crate::editors::shown(c.side)
                        ),
                    ));
                    if c.block.1 > c.side {
                        rows.push((
                            "In",
                            format!("a block of {}³ cells alike", (c.block.1 / c.side).round()),
                        ));
                    }
                }
                None => rows.push(("Cell", "no body here".to_owned())),
            }
            egui::Grid::new(("probe rows", pane)).num_columns(2).spacing([10.0, 2.0]).show(ui, |ui| {
                for (name, value) in &rows {
                    ui.label(RichText::new(*name).size(12.0).color(theme::WEAK));
                    ui.label(RichText::new(value).size(12.0).color(theme::TEXT));
                    ui.end_row();
                }
            });
            if !probed.not_counted.is_empty() {
                ui.add(
                    egui::Label::new(
                        RichText::new(format!("not counted: {}", probed.not_counted.join("; ")))
                            .size(11.0)
                            .color(theme::WEAK),
                    )
                    .wrap(),
                );
            }
            ui.add_space(4.0);
            egui::Grid::new(("probe towers", pane)).num_columns(3).spacing([12.0, 2.0]).striped(true).show(
                ui,
                |ui| {
                    for heading in ["tower", "distance", "cell's share of ΔΦ"] {
                        ui.label(RichText::new(heading).size(11.5).color(theme::WEAK));
                    }
                    ui.end_row();
                    for t in probed.towers.iter().take(MAX_OUTLINE_ROWS) {
                        ui.label(RichText::new(&t.label).size(12.0).color(theme::TEXT));
                        ui.label(RichText::new(format!("{:.2} m", t.distance)).size(12.0).color(theme::TEXT));
                        let share = match (&t.phase, &t.why) {
                            (Some(phase), _) => crate::shell::phase_text(*phase),
                            (None, Some(_)) => "not read".to_owned(),
                            (None, None) => "—".to_owned(),
                        };
                        let label = ui.label(RichText::new(share).size(12.0).color(theme::STRONG));
                        if let Some(why) = &t.why {
                            label.on_hover_text(why);
                        }
                        ui.end_row();
                    }
                },
            );
        });
    shell.layout.buttons.push(("probe card".to_owned(), card.response.rect));
}

/// A positive quantity in `unit` with the SI prefix that keeps it between 1 and 1000: `si(1.2e-7,
/// "m/s²")` is "120 nm/s²". Three significant figures.
fn si(value: f64, unit: &str) -> String {
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
            FontId::proportional(12.0),
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
        let command = if shell.flying() {
            // Flying, the arrows look round from the eye.
            let (yaw, pitch) = if axis == 0 { (-steps * ORBIT_STEP, 0.0) } else { (0.0, steps * ORBIT_STEP) };
            crate::camera::look(&view, pane, yaw, pitch)
        } else if ctrl {
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
    let count = shell.engine.panes();
    let active = shell.engine.active_pane();
    let generation = shell.engine.generation();
    let mut commands: Vec<Command> = Vec::new();
    let mut drags: Vec<(usize, egui::Vec2, bool, egui::Vec2)> = Vec::new();
    let mut body_drags: Vec<(usize, usize, egui::Vec2, egui::Vec2)> = Vec::new();
    let mut zooms: Vec<(usize, f32, [f64; 2], [f64; 2])> = Vec::new();
    // Where the selection's keyboard handle is drawn, if the active pane shows the scene.
    let mut handle_rect = Rect::NOTHING;

    // Maximised, the active pane is the whole figure and the others are not drawn (their
    // cameras and contents are kept: it is the window's state, not the engine's).
    let mut rects = pane_rects(count, size, PANE_GAP);
    if shell.maximised {
        let whole = pane_rects(1, size, PANE_GAP)[0];
        for (pane, rect) in rects.iter_mut().enumerate() {
            *rect =
                if pane == active { whole } else { cav_render::PaneRect { width: 0, height: 0, ..whole } };
        }
    }
    for (pane, pixels) in rects.into_iter().enumerate() {
        let rect = Rect::from_min_size(
            area.min + vec2(pixels.x as f32, pixels.y as f32) / scale,
            vec2(pixels.width as f32, pixels.height as f32) / scale,
        );
        shell.layout.panes.push(rect);
        if pixels.width == 0 || pixels.height == 0 {
            continue;
        }
        let content = shell.engine.content(pane);
        if content == cav_engine::PaneContent::Scene {
            let frame = shell.engine.frame(pane).expect("pane_rects gives no more panes than the engine has");
            match surface.texture(pane, [pixels.width, pixels.height], generation, frame) {
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
                        FontId::proportional(12.0),
                        theme::WEAK,
                    );
                }
            }
        } else {
            let drawn = crate::plots::paint(&shell.engine, &ui.painter_at(rect), pane, rect);
            shell.layout.plots.extend(drawn);
        }
        if count > 1 {
            let (colour, width) =
                if pane == active { (theme::SELECTION_EDGE, 1.5) } else { (theme::RULE, 1.0) };
            ui.painter().rect_stroke(rect, 0.0, Stroke::new(width, colour), StrokeKind::Inside);
            // A plot says what it is in its own title.
            if content == cav_engine::PaneContent::Scene {
                let view = shell.engine.view(pane);
                let name = view.named().map_or("free", NamedView::name);
                // Clear of the floating tool strip on the figure's left edge (S19's review,
                // minor 15: it covered the left pane's "iso").
                let x = if rect.left() - area.left() < 60.0 { 64.0 } else { 6.0 };
                ui.painter().text(
                    rect.left_top() + vec2(x, 4.0),
                    Align2::LEFT_TOP,
                    name,
                    FontId::proportional(11.0),
                    theme::WEAK,
                );
            }
        }
        let response = ui.interact(rect, egui::Id::unique(("pane", pane)), Sense::click_and_drag());
        if (response.clicked() || response.drag_started()) && pane != active {
            commands.push(Command::View(ViewCommand::Activate(pane)));
        }
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
                    commands.push(Command::View(ViewCommand::Span { pane, span: None }));
                } else {
                    // The wheel's movement is smoothed over several frames, so a click may land
                    // while it is still arriving: the two are not alternatives.
                    if scroll != 0.0
                        && let Some(point) = ui.input(|i| i.pointer.hover_pos())
                    {
                        let span =
                            crate::plots::zoomed([drawn.x.lo, drawn.x.hi], value(point), scroll, whole);
                        commands.push(Command::View(ViewCommand::Span { pane, span }));
                    }
                    if matches!(content, cav_engine::PaneContent::Signal | cav_engine::PaneContent::Flight)
                        && (response.clicked() || response.dragged())
                        && let Some(point) = response.interact_pointer_pos()
                    {
                        commands.push(Command::Seek(value(point)));
                    }
                }
            }
            continue;
        }
        // The right button pressed over the scene flies its camera while it is held (Unity's and
        // Unreal's: W A S D move, the mouse looks). S18's review, U3.
        if response.hovered() && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary)) {
            shell.mouse_fly = Some(pane);
            if pane != active {
                commands.push(Command::View(ViewCommand::Activate(pane)));
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
            handle_rect = match gizmo {
                Some((_, g)) => Rect::from_center_size(g.centre + rect.min.to_vec2(), vec2(24.0, 24.0)),
                None => selected_body
                    .and_then(|n| shell.body_point(pane, n, rect.size()))
                    .map_or(Rect::NOTHING, |p| {
                        Rect::from_center_size(p + rect.min.to_vec2(), vec2(24.0, 24.0))
                    }),
            };
        }
        // The Place tool in hand over the scene: a grid on the ground round the pointer and a ghost
        // of the starter where a click would put it (S18's second review, UX 2: in the void there
        // was nothing to aim at).
        if behaviour == crate::tools::Behaviour::Place && pane == active {
            place_preview(shell, ui, pane, rect, response.hover_pos().map(within));
        }
        if behaviour == crate::tools::Behaviour::Probe && pane == active {
            let asked =
                shell.probe_at.take().map(|[x, y]| rect.min + vec2(x * rect.width(), y * rect.height()));
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
            }
            probe_card(shell, ui, pane, rect);
            continue;
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
            continue;
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
                    crate::tools::Handle::Centre => body_drags.push((pane, drag.body, delta, rect.size())),
                }
            }
            if response.drag_stopped() {
                shell.gizmo_drag = None;
                shell.drag_under_way = false;
            }
            continue;
        }
        if response.clicked()
            && let Some(point) = response.interact_pointer_pos()
            && let Some(n) = shell.body_under(pane, within(point), rect.size())
        {
            commands.push(Command::Select(Some(Target::Body(n + 1))));
        }
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
            && let Some(n) = shell.body_under(pane, within(origin), rect.size())
            && !shell.is_locked(n)
        {
            shell.dragging = Some((pane, n));
            shell.drag_under_way = false;
            if shell.engine.selection() != Some(Target::Body(n + 1)) {
                commands.push(Command::Select(Some(Target::Body(n + 1))));
            }
        }
        if response.dragged() {
            match shell.dragging {
                Some((held_in, n)) if held_in == pane => {
                    body_drags.push((pane, n, response.drag_delta(), rect.size()));
                }
                _ => {
                    // The middle button pans; the right looks round from the eye (it flies).
                    let panning = response.dragged_by(egui::PointerButton::Middle);
                    drags.push((pane, response.drag_delta(), panning, rect.size()));
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
                zooms.push((pane, scroll, [f64::from(at.x), f64::from(at.y)], size));
            }
        }
    }
    // The tool strip: a floating bar over the view's left edge, not a column of its own
    // (S19, `docs/design/panels.md`: Blender's and Unity's scene toolbars).
    if !shell.gui_hidden && !shell.maximised {
        // No fade-in: an unchanged frame must be the same picture (the GPU tests hold it).
        let strip = egui::Area::new(egui::Id::unique("tool strip"))
            .fade_in(false)
            .order(egui::Order::Foreground)
            .fixed_pos(area.min + vec2(8.0, 8.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(theme::FOOTER.gamma_multiply(0.94))
                    .stroke(Stroke::new(1.0, theme::WIDGET_EDGE))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::same(4))
                    .show(ui, |ui| {
                        ui.set_width(40.0);
                        tool_strip(shell, ui);
                    });
            });
        shell.building.place("toolbar", strip.response.rect);
        shell.layout.buttons.push(("tool strip".to_owned(), strip.response.rect));
    }
    // The selection's keyboard handle (Enter starts a transform session): a stop of the
    // viewport whatever is selected and whatever the panes show (order never depends on state:
    // S18's review, minor 5), disabled when there is no body in a scene pane to move.
    shell.register("viewport", "viewport/selection", "Selection", Kind::Session, handle_rect);
    let movable = matches!(shell.engine.selection(), Some(Target::Body(_)))
        && shell.engine.content(active) == cav_engine::PaneContent::Scene;
    if !movable {
        shell.building.disable("viewport/selection");
    }
    // What the renderer left out of the last picture: badges in the header, and each kind said
    // once in the log when it first appears (the counts in it change from frame to frame).
    let notes = surface.notes();
    let kind = |note: &String| note.split(':').next().unwrap_or_default().to_owned();
    for note in &notes {
        if !shell.render_notes.iter().any(|old| kind(old) == kind(note)) {
            shell.engine.note(cav_engine::Level::Warn, cav_engine::Subsystem::Render, note.clone());
        }
    }
    shell.render_notes = notes;

    // Home: the one control on the figure. It resets a camera, so it stands in the corner of a
    // pane that has one: the pane in the figure's top right corner if that shows the scene, else
    // the active pane, else the first that does. A plot has its title along its top and no
    // camera; with no scene in any pane there is no Home.
    let shows_scene = |pane: &usize| shell.engine.content(*pane) == cav_engine::PaneContent::Scene;
    let corner = pos2(area.right() - 4.0, area.top() + 4.0);
    let under = (0..count).find(|pane| shell.layout.panes[*pane].contains(corner));
    let home_pane = under
        .into_iter()
        .chain([active])
        .chain(0..count)
        .find(shows_scene)
        .map(|pane| shell.layout.panes[pane]);
    if let (false, Some(pane)) = (shell.gui_hidden, home_pane) {
        let home = Rect::from_min_size(pos2(pane.right() - 62.0, pane.top() + 8.0), vec2(54.0, 20.0));
        let response = ui
            .put(home, Button::new(RichText::new("Home").size(12.0)))
            .on_hover_text("reset the camera (Home)");
        shell.layout.buttons.push(("view home".to_owned(), response.rect));
        if response.clicked() {
            let _ = shell.run("view home");
        }
        // Maximised, the way back is in sight beside Home (S18's review, U7: only Ctrl+Space
        // brought the window back, and nothing said so).
        if shell.maximised {
            let restore = Rect::from_min_size(home.left_top() - vec2(150.0, 0.0), vec2(142.0, 20.0));
            let response = ui
                .put(restore, Button::new(RichText::new("Restore  Ctrl+Space").size(12.0)))
                .on_hover_text("bring back the panels: the window as it was before the pane filled it");
            shell.layout.buttons.push(("maximise".to_owned(), response.rect));
            if response.clicked() {
                let _ = shell.run("maximise");
            }
        }
    }

    for command in commands {
        shell.apply(command);
    }
    for (pane, delta, panning, size) in drags {
        if delta != egui::Vec2::ZERO {
            if panning {
                shell.pan_by(pane, delta, size);
            } else if shell.any_fly() {
                // Flying, a drag looks round from the eye, as the arrows do.
                let command = crate::camera::look(
                    shell.engine.view(pane),
                    pane,
                    -f64::from(delta.x) * 0.2,
                    -f64::from(delta.y) * 0.2,
                );
                shell.apply(Command::View(command));
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
                            ui.label(RichText::new(&row.scope).color(theme::GOLD).size(12.0));
                        }
                        ui.horizontal_top(|ui| {
                            theme::fixed_left(
                                ui,
                                vec2(136.0, 16.0),
                                egui::Label::new(
                                    RichText::new(row.keys_text()).monospace().size(11.5).color(theme::STRONG),
                                )
                                .wrap(),
                            );
                            let label = ui.add(egui::Label::new(RichText::new(&row.label).size(12.0)).wrap());
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
                .size(11.0)
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
                ui.label(RichText::new(adapter).monospace().size(11.0).color(theme::WEAK));
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Numbers are set in Ubuntu Mono, © Canonical Ltd, used unmodified under the Ubuntu Font \
                     Licence 1.0 (hover for the licence; `cav licences` prints it).",
                )
                .size(11.0)
                .color(theme::WEAK),
            )
            .on_hover_text(RichText::new(theme::MONO_FONT_LICENCE).size(10.0));
            ui.add_space(4.0);
            ui.label(RichText::new("The window's own commands:").size(11.5));
            for (name, about) in APP_COMMANDS {
                ui.label(
                    RichText::new(format!("{name:<8} {about}")).monospace().size(11.0).color(theme::WEAK),
                );
            }
        });
    if let Some(window) = about {
        shell.building.place_window("about", window.response.rect, window.response.layer_id);
        shell.register("about", "about/text", "About", Kind::ReadOnly, window.response.rect);
    }
    shell.about_open = about_open;
}
