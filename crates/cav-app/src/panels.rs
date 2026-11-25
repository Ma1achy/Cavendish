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

use crate::menus::{Item, MENUS};
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

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(text).color(theme::STRONG).size(12.0));
    let y = ui.cursor().top();
    ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, theme::RULE));
    ui.add_space(3.0);
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
fn scrolling(ui: &mut egui::Ui, id: &str, contents: impl FnOnce(&mut egui::Ui)) {
    egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, false]).show(ui, contents);
}

pub fn draw(shell: &mut Shell, ui: &mut egui::Ui, surface: &mut dyn PaneSurface) {
    let ctx = ui.ctx().clone();
    if !shell.gui_hidden {
        let menu = Panel::top("menu")
            .exact_size(28.0)
            .frame(frame(theme::PANEL, 8, 0))
            .show(ui, |ui| menu_bar(shell, ui));
        shell.layout.panels.push(("menu", menu.response.rect));
        let footer_panel = Panel::bottom("footer")
            .exact_size(22.0)
            .frame(frame(theme::FOOTER, 10, 0))
            .show(ui, |ui| footer(shell, ui));
        shell.layout.panels.push(("footer", footer_panel.response.rect));
        if shell.console_open {
            let panel = Panel::bottom("console")
                .exact_size(200.0)
                .frame(frame(theme::FOOTER, 0, 0))
                .show(ui, |ui| console(shell, ui));
            shell.layout.panels.push(("console", panel.response.rect));
        }
        // Up to eight lanes are shown; an array with more scrolls inside the panel.
        let lanes_shown = shell.engine.spec().array.towers.len().min(timeline::MAX_LANES_SHOWN);
        let lanes_height = timeline::height(lanes_shown);
        let panel = Panel::bottom("timeline")
            .exact_size(lanes_height + 34.0)
            .frame(frame(theme::PANEL, 8, 4))
            .show(ui, |ui| timeline_panel(shell, ui));
        shell.layout.panels.push(("timeline", panel.response.rect));
        let panel = Panel::left("outliner")
            .exact_size(262.0)
            .resizable(false)
            .frame(frame(theme::PANEL, 10, 6))
            .show(ui, |ui| scrolling(ui, "outliner", |ui| outliner(shell, ui)));
        shell.layout.panels.push(("outliner", panel.response.rect));
        let panel = Panel::right("properties")
            .exact_size(264.0)
            .resizable(false)
            .frame(frame(theme::PANEL, 10, 6))
            .show(ui, |ui| scrolling(ui, "properties", |ui| properties(shell, ui)));
        shell.layout.panels.push(("properties", panel.response.rect));
    }
    egui::CentralPanel::no_frame().show(ui, |ui| {
        if !shell.gui_hidden {
            let strip = Panel::top("header")
                .exact_size(24.0)
                .frame(frame(theme::FOOTER, 10, 0))
                .show(ui, |ui| header(shell, ui));
            shell.layout.panels.push(("header", strip.response.rect));
        }
        egui::CentralPanel::no_frame().show(ui, |ui| figure(shell, ui, surface));
    });
    windows(shell, &ctx);
    if shell.quit {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

fn menu_bar(shell: &mut Shell, ui: &mut egui::Ui) {
    fn items(shell: &mut Shell, ui: &mut egui::Ui, list: &'static [Item]) {
        let (can_undo, can_redo) = shell.engine.history();
        for item in list {
            match *item {
                Item::Separator => {
                    ui.separator();
                }
                Item::Later { label, slice } => {
                    ui.add_enabled(false, Button::new(label).shortcut_text(slice));
                }
                Item::Sub { label, items: nested } => {
                    ui.menu_button(label, |ui| items(shell, ui, nested));
                }
                Item::Run { label, line, shortcut } => {
                    let enabled = match line {
                        "undo" => can_undo,
                        "redo" => can_redo,
                        _ => crate::menus::possible(line, shell.engine.spec().array.towers.len()),
                    };
                    if ui.add_enabled(enabled, Button::new(label).shortcut_text(shortcut)).clicked() {
                        let _ = shell.run(line);
                        ui.close();
                    }
                }
            }
        }
    }
    // The brand, the menus and the status share the menu bar's own row, so that the status on the
    // right is laid out in what the menus leave and cannot be pushed off the window.
    egui::MenuBar::new().ui(ui, |ui| {
        ui.label(RichText::new("cavendish · dev").color(theme::STRONG).strong());
        ui.add_space(6.0);
        for (title, list) in MENUS {
            ui.menu_button(*title, |ui| items(shell, ui, list));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let spec = shell.engine.spec();
            let status = format!("{} · t {:.2} / {:.0} s", spec.name, shell.engine.time(), spec.duration);
            ui.add(egui::Label::new(RichText::new(status).monospace().color(theme::TEXT)).truncate());
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
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let label = if shell.console_open { "close console" } else { "console" };
            if ui.add(Button::new(RichText::new(label).size(11.5)).frame(false)).on_hover_text("`").clicked()
            {
                let _ = shell.run("console");
            }
            ui.label(
                RichText::new(if shell.playing { "Space pause" } else { "Space play" })
                    .size(11.5)
                    .color(theme::WEAK),
            );
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
            // Whatever is left is the status line, cut short rather than wrapped.
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add(egui::Label::new(RichText::new(text).size(11.5).color(colour)).truncate());
            });
        });
    });
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
        if ui
            .add(Button::new(if shell.playing { "pause" } else { "play" }).min_size(vec2(48.0, 0.0)))
            .clicked()
        {
            let _ = shell.run("play");
        }
        if ui.button("rewind").clicked() {
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

/// A row of the outliner that selects something when clicked.
fn outline_row(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    target: Target,
    swatch: Color32,
    label: &str,
    detail: &str,
) {
    let selected = shell.engine.selection() == Some(target);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 21.0), Sense::click());
    shell.layout.outline.push((target, rect));
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 2.0, Color32::from_rgb(0x10, 0x30, 0x3d));
    } else if response.hovered() {
        painter.rect_filled(rect, 2.0, Color32::from_rgb(0x24, 0x24, 0x24));
    }
    let swatch_rect = Rect::from_center_size(pos2(rect.left() + 14.0, rect.center().y), vec2(9.0, 9.0));
    painter.rect_filled(swatch_rect, 1.0, swatch);
    let text = if selected { theme::STRONG } else { theme::TEXT };
    painter.text(
        pos2(rect.left() + 26.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        text,
    );
    painter.text(
        pos2(rect.right() - 4.0, rect.center().y),
        Align2::RIGHT_CENTER,
        detail,
        FontId::monospace(11.0),
        theme::WEAK,
    );
    if response.clicked() {
        shell.apply(Command::Select(if selected { None } else { Some(target) }));
    }
}

/// A row of small buttons, each running a console line; the one that matches the state is lit.
fn button_row(shell: &mut Shell, ui: &mut egui::Ui, label: &str, buttons: &[(&str, &str, bool)]) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        ui.spacing_mut().button_padding.x = 5.0;
        ui.add_sized(
            [42.0, 18.0],
            egui::Label::new(RichText::new(label).color(theme::WEAK).size(12.0)).halign(Align::Min),
        );
        for &(text, line, on) in buttons {
            let response =
                ui.add(Button::new(RichText::new(text).size(12.0)).selected(on)).on_hover_text(line);
            shell.layout.buttons.push((line.to_owned(), response.rect));
            if response.clicked() {
                let _ = shell.run(line);
            }
        }
    });
}

/// The most rows the outliner lays out for the bodies, and for the towers: a scene of a thousand
/// of either is not a list to scroll, and every row is laid out every frame. The rest are said
/// to be there; `ls bodies` lists them all. A selected item past the cap keeps its row.
pub const MAX_OUTLINE_ROWS: usize = 100;

/// Which of `count` items get a row: the first [`MAX_OUTLINE_ROWS`], and the selected one.
fn outlined(count: usize, selected: Option<usize>) -> Vec<usize> {
    let mut rows: Vec<usize> = (0..count.min(MAX_OUTLINE_ROWS)).collect();
    if let Some(n) = selected.filter(|n| *n >= MAX_OUTLINE_ROWS && *n < count) {
        rows.push(n);
    }
    rows
}

fn outliner(shell: &mut Shell, ui: &mut egui::Ui) {
    ui.label(RichText::new("Outliner").color(theme::STRONG).strong());
    // Only what is shown is copied out of the scene: a row's words, not the scene.
    let (selected_body, selected_tower) = match shell.engine.selection() {
        Some(Target::Body(n)) => (Some(n - 1), None),
        Some(Target::Tower(n)) => (None, Some(n - 1)),
        _ => (None, None),
    };
    let (bodies, towers, room, floor) = {
        let spec = shell.engine.spec();
        let bodies: Vec<(usize, String, String)> = outlined(spec.bodies.len(), selected_body)
            .into_iter()
            .map(|n| {
                let body = &spec.bodies[n];
                (n, body.label.clone(), format!("{} kg · {}", short_number(body.mass), body.motion.kind()))
            })
            .collect();
        let towers: Vec<(usize, String, [f64; 3])> = outlined(spec.array.towers.len(), selected_tower)
            .into_iter()
            .map(|n| (n, spec.array.towers[n].label.clone(), spec.array.towers[n].position))
            .collect();
        (
            (bodies, spec.bodies.len()),
            (towers, spec.array.towers.len()),
            spec.room.as_ref().map(|room| room.size),
            spec.floor.as_ref().map(|floor| floor.height),
        )
    };
    let left_out = |ui: &mut egui::Ui, shown: usize, count: usize, list: &str| {
        if shown < count {
            let text = format!("… and {} more: `ls {list}` lists them all", count - shown);
            ui.label(RichText::new(text).size(11.0).color(theme::WEAK));
        }
    };

    heading(ui, "Sources");
    for (n, label, detail) in &bodies.0 {
        outline_row(shell, ui, Target::Body(n + 1), theme::GOLD, label, detail);
    }
    left_out(ui, bodies.0.len(), bodies.1, "bodies");
    if bodies.1 == 0 {
        ui.label(RichText::new("none · `load point` has one").size(11.5).color(theme::WEAK));
    }
    // Adding is a command too: a point mass at rest beside the array, to be edited from there.
    button_row(shell, ui, "", &[("+ mass", "mass 1 at 2 2 1", false)]);

    heading(ui, "Array");
    // With something to read, each tower's row shows what it reads under the playhead.
    let reply = shell.reading().clone();
    let silent = bodies.1 == 0;
    for (n, label, position) in &towers.0 {
        let (n, [x, y, _]) = (*n, *position);
        // A number the audit warns of carries a mark; the warning is in the tower's properties.
        let warned = reply.warnings.iter().any(|w| w.tower == Some(n));
        let detail = match reply.towers.get(n) {
            Some(reading) if !silent => {
                format!("{}{}", crate::shell::phase_text(reading.gradiometer), if warned { " ⚠" } else { "" })
            }
            _ => format!("{x:+.1} {y:+.1}"),
        };
        outline_row(shell, ui, Target::Tower(n + 1), theme::tower(n), label, &detail);
    }
    left_out(ui, towers.0.len(), towers.1, "array.towers");
    if !silent && !reply.warnings.is_empty() {
        let text = format!("{} reading(s) warned of: select the tower, or `reading`", reply.warnings.len());
        ui.label(RichText::new(text).size(11.0).color(theme::WARN));
    }

    heading(ui, "Surroundings");
    if let Some([x, y, z]) = room {
        outline_row(
            shell,
            ui,
            Target::Room,
            theme::WIDGET_EDGE,
            "room",
            &format!("{x:.0} × {y:.0} × {z:.0} m"),
        );
    }
    if let Some(height) = floor {
        outline_row(
            shell,
            ui,
            Target::Floor,
            theme::WIDGET_EDGE,
            "floor",
            &format!("z = {height:.1} m · to the horizon"),
        );
    }
    if room.is_none() && floor.is_none() {
        ui.label(RichText::new("nothing: no room, no floor, no air").size(11.5).color(theme::WEAK));
    }

    heading(ui, "Run");
    let forward = match shell.engine.signal() {
        (cav_engine::SignalState::Refined, Some(signal)) => {
            // A refined signal is the scene's as it is: its shots are the schedule's, and how
            // many each tower takes is the engine's count (kept between frames, not made here).
            // The most shots any tower has (towers of their own instrument may have fewer).
            let shots = signal.most();
            let (fewest, most) = taken_a_tower(&shell.engine).unwrap_or((shots, shots));
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
    };
    shell.layout.status.clone_from(&forward);
    ui.label(RichText::new(forward).size(11.5).color(theme::WEAK));

    heading(ui, "View");
    let pane = shell.engine.active_pane();
    let view = shell.engine.view(pane).clone();
    let at = |named: NamedView| view.named() == Some(named);
    button_row(
        shell,
        ui,
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
        "panes",
        &[("1", "panes 1", panes == 1), ("2", "panes 2", panes == 2), ("4", "panes 4", panes == 4)],
    );
    let style = |s: DrawStyle| view.style == s;
    button_row(
        shell,
        ui,
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
        button_row(shell, ui, if row == 0 { "show" } else { "" }, &buttons);
    }
    voxel_rows(shell, ui, &view);
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "drag to orbit · right-drag to pan · scroll to zoom · drag a body to move it · O swaps the projection · Home resets",
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
    let colours = [LeafField::Level, LeafField::Depth, LeafField::Mass, LeafField::Fill].map(colour);
    let buttons: Vec<(&str, &str, bool)> = colours.iter().map(|(t, l, on)| (*t, l.as_str(), *on)).collect();
    button_row(shell, ui, "colour", &buttons);
    let tree = view.overlay.tree;
    let fade = tree.map(|t| t.fade);
    let plain = !view.overlay.leaves && view.overlay.cut.is_none();
    button_row(
        shell,
        ui,
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

fn properties(shell: &mut Shell, ui: &mut egui::Ui) {
    let spec = shell.engine.spec().clone();
    let (title, path) = match shell.engine.selection() {
        None => ("Scene".to_owned(), String::new()),
        Some(Target::Tower(n)) => {
            (spec.array.towers[n - 1].label.clone(), format!("array.towers[{}]", n - 1))
        }
        Some(Target::Room) => ("Room".to_owned(), "room".to_owned()),
        Some(Target::Floor) => ("Floor".to_owned(), "floor".to_owned()),
        Some(Target::Body(n)) => (spec.bodies[n - 1].label.clone(), format!("bodies[{}]", n - 1)),
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new("Properties").color(theme::STRONG).strong());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(&path).monospace().size(11.0).color(theme::WEAK));
        });
    });
    heading(ui, &title);
    // Every field under the selection has an editor, made from the schema: its unit, its range,
    // its doc string on hover. Each change is a `set`, as the console would issue it.
    match shell.engine.selection() {
        None => {
            for path in ["name", "description", "duration"] {
                crate::editors::show(shell, ui, path);
            }
        }
        Some(Target::Tower(n)) => {
            crate::editors::show(shell, ui, &path);
            crate::editors::structure(shell, ui, &path);
            let reply = shell.reading().clone();
            heading(ui, &format!("Reading · a shot read out at {:.2} s", reply.time));
            if let Some(reading) = reply.towers.get(n - 1) {
                rows(
                    ui,
                    "reading",
                    &[
                        ("ΔΦ", crate::shell::phase_text(reading.gradiometer)),
                        ("δφ₁ lower", crate::shell::phase_text(reading.lower)),
                        ("δφ₂ upper", crate::shell::phase_text(reading.upper)),
                        (
                            "quadrature",
                            cav_engine::estimate_text(reply.estimate, reply.settled, reply.truncated),
                        ),
                        ("state", reply.state.to_string()),
                    ],
                );
            } else {
                ui.label(RichText::new("computing").size(11.5).color(theme::WEAK));
            }
            // What the audit warns of about this tower's number is said beside it.
            for warning in reply.warnings.iter().filter(|w| w.concerns(n - 1)) {
                ui.label(RichText::new(&warning.text).size(11.0).color(theme::WARN));
            }
        }
        Some(Target::Room | Target::Floor) => {
            crate::editors::show(shell, ui, &path);
            crate::editors::structure(shell, ui, &path);
        }
        Some(Target::Body(n)) => {
            crate::editors::show(shell, ui, &path);
            crate::editors::structure(shell, ui, &path);
            let body = &spec.bodies[n - 1];
            let [x, y, z] = body.motion.position(shell.engine.time(), spec.array.instrument.gravity);
            ui.add_space(3.0);
            rows(ui, "body", &[("now at", format!("{x:.3}, {y:.3}, {z:.3} m"))]);
        }
    }

    heading(ui, "Cadence · when the array fires");
    crate::editors::show(shell, ui, "array.schedule");

    heading(ui, "Instrument · shared by every tower");
    let instrument = shell.engine.spec().array.instrument.clone();
    let is = |name: &str| cav_sim::presets::load(name).is_ok_and(|preset| preset == instrument);
    button_row(
        shell,
        ui,
        "preset",
        &[("AION-10", "preset aion10", is("aion10")), ("AICE 140 m", "preset aice", is("aice"))],
    );
    let other = shell.engine.comparison().cloned();
    let with = |name: &str| cav_sim::presets::load(name).is_ok_and(|preset| Some(preset) == other);
    button_row(
        shell,
        ui,
        "compare",
        &[
            ("AION-10", "compare aion10", with("aion10")),
            ("AICE", "compare aice", with("aice")),
            ("off", "compare off", other.is_none()),
        ],
    );
    crate::editors::show(shell, ui, "array.instrument");

    let pane = shell.engine.active_pane();
    let view = shell.engine.view(pane).clone();
    heading(ui, &format!("Camera · pane {}", pane + 1));
    rows(
        ui,
        "camera",
        &[
            ("target", format!("{:.2}, {:.2}, {:.2} m", view.target[0], view.target[1], view.target[2])),
            ("yaw · pitch", format!("{:.1}° · {:.1}°", view.yaw, view.pitch)),
            ("distance", format!("{:.2} m", view.distance)),
        ],
    );
    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "drag a number or type one · every change is a `set <path> <value>` that undo takes back",
        )
        .size(11.0)
        .color(theme::WEAK),
    );
}

fn header(shell: &mut Shell, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        ui.label(RichText::new("Scene view").color(theme::STRONG));
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
                Target::Body(n) => shell.engine.spec().bodies[n - 1].label.clone(),
            };
            badge(ui, format!("selected: {text}"), theme::STRONG);
        }
    });
}

fn figure(shell: &mut Shell, ui: &mut egui::Ui, surface: &mut dyn PaneSurface) {
    let area = ui.max_rect();
    ui.painter().rect_filled(area, 0.0, theme::BACKGROUND);
    shell.layout.figure = area;
    let scale = ui.ctx().pixels_per_point();
    let size =
        [(area.width() * scale).round().max(1.0) as u32, (area.height() * scale).round().max(1.0) as u32];
    let count = shell.engine.panes();
    let active = shell.engine.active_pane();
    let generation = shell.engine.generation();
    let mut commands: Vec<Command> = Vec::new();
    let mut drags: Vec<(usize, egui::Vec2, bool, egui::Vec2)> = Vec::new();
    let mut body_drags: Vec<(usize, usize, egui::Vec2, egui::Vec2)> = Vec::new();
    let mut zooms: Vec<(usize, f32)> = Vec::new();

    for (pane, pixels) in pane_rects(count, size, PANE_GAP).into_iter().enumerate() {
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
                ui.painter().text(
                    rect.left_top() + vec2(6.0, 4.0),
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
        // A click on a body selects it; a drag that starts on one takes hold of it.
        let within = |point: egui::Pos2| (point - rect.min).to_pos2();
        if response.clicked()
            && let Some(point) = response.interact_pointer_pos()
            && let Some(n) = shell.body_under(pane, within(point), rect.size())
        {
            commands.push(Command::Select(Some(Target::Body(n + 1))));
        }
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
            && let Some(n) = shell.body_under(pane, within(origin), rect.size())
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
                    let panning = response.dragged_by(egui::PointerButton::Secondary)
                        || response.dragged_by(egui::PointerButton::Middle);
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
                zooms.push((pane, scroll));
            }
        }
    }

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
    }

    for command in commands {
        shell.apply(command);
    }
    for (pane, delta, panning, size) in drags {
        if delta != egui::Vec2::ZERO {
            if panning { shell.pan_by(pane, delta, size) } else { shell.orbit_by(pane, delta) }
        }
    }
    for (pane, n, delta, size) in body_drags {
        if delta != egui::Vec2::ZERO {
            shell.drag_body_by(pane, n, delta, size);
        }
    }
    for (pane, scroll) in zooms {
        shell.zoom_by(pane, scroll);
    }
}

fn windows(shell: &mut Shell, ctx: &egui::Context) {
    let mut keys_open = shell.keys_open;
    egui::Window::new("Keyboard")
        .open(&mut keys_open)
        .collapsible(false)
        .resizable(false)
        .default_width(430.0)
        .show(ctx, |ui| {
            let keys: &[(&str, &str)] = &[
                ("Space", "play / pause"),
                ("Left · Right", "step the playhead by 0.1 s"),
                ("1 – 9", "select a tower"),
                ("Esc", "close a window, the console, leave F3, clear the selection"),
                ("Home", "reset the active pane's camera"),
                ("O", "swap perspective and orthographic"),
                ("Ctrl+Z · Ctrl+Shift+Z", "undo · redo: every edit, whatever made it"),
                ("`", "console"),
                ("F3", "hide the GUI; the scene fills the window"),
                ("?", "this window"),
            ];
            egui::Grid::new("keys").num_columns(2).spacing([16.0, 5.0]).show(ui, |ui| {
                for (key, what) in keys {
                    ui.label(RichText::new(*key).monospace().color(theme::STRONG));
                    ui.label(*what);
                    ui.end_row();
                }
            });
            ui.add_space(6.0);
            ui.label(
                RichText::new("every key is a console command; `help` lists them all")
                    .size(11.0)
                    .color(theme::WEAK),
            );
        });
    shell.keys_open = keys_open;

    let mut about_open = shell.about_open;
    egui::Window::new("About Cavendish")
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
    shell.about_open = about_open;
}
