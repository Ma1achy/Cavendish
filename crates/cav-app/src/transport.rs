//! The transport: the timeline's controls, as a media player's (`docs/design/workspace.md` §5).
//!
//! ```text
//! [start] [step back] [play/pause] [step forward] [end]  [at the end ▾]  1× ▾   t [3.20] / [12.00] s   shot [3] / 6
//! ```
//!
//! Every button is an action of `ui/actions.ron` (its console line, its key, when it can run and
//! why not), so the buttons, the Scene menu, the palette and the keys are one path. The time,
//! the duration and the shot are fields: typing a time seeks there, typing a shot's number seeks
//! to that shot, and typing a duration is the `set duration` the Scene card's editor issues (one
//! edit, one undo).
//!
//! **At the end** ([`AtEnd`], `end loop|stop|run-on`) playing loops to the start (as it begins),
//! stops with the playhead at exactly the duration, or runs on: the scene's duration grows by
//! its length at the start of the run, each extension continuing the same `set` so that a whole
//! run is one undo, up to the schema's limit (`cav_sim::spec::limits::DURATION`), where playing
//! stops and the log says so ([`crate::Shell::advance`]). Each extension is an edit: the engine
//! computes the signal again for the longer timeline, off the frame's thread.
//!
//! **Which shot times.** A step lands on a time the engine reads the signal at: a *readout* of
//! the engine's own shots ([`cav_engine::Engine::shots`], `cav_sim::read::Shots::readout`), never
//! a time worked out here. Towers may have their own clocks and cycles, so the shots are one
//! tower's: the selected tower's, else the first's ([`tower`]). Every shot of its schedule
//! counts, taken or not (the lane draws both). Only readouts inside the timeline (0 to the
//! scene's duration, seconds) can be reached: the playhead cannot leave it.
//!
//! Playing, the speed and which menu is open are the window's own state ([`crate::Shell`]):
//! the buttons edit nothing; the commands issued are the engine's `seek` and, for the duration,
//! its `set`.

use cav_engine::Command;
use cav_sim::read::Shots;
use egui::{Align, Align2, Color32, Rect, RichText, Sense, pos2, vec2};

use crate::actions::table;
use crate::icons::{self, Transport};
use crate::keyboard::{Kind, REPEAT_DELAY, REPEAT_EVERY};
use crate::shell::Shell;
use crate::theme;

/// The keyboard's scope the transport's controls are in.
pub const SCOPE: &str = "timeline";

/// The speeds the transport offers, as multiples of the clock's rate (1 is a second of the
/// scene a second). `speed <x>` takes any from the least to the greatest.
pub const SPEEDS: [f64; 7] = [0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0];

/// The most steps one frame of a held step button takes (a frame that took long does not jump).
const MOST_STEPS_A_FRAME: u32 = 8;

/// One control of the transport, as data: the keyboard walks them in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Control {
    /// Its id: `timeline/<id>` in the keyboard's tree, and what [`Drawn::id`] says.
    pub id: &'static str,
    /// What the breadcrumb calls it.
    pub name: &'static str,
    /// The action of `ui/actions.ron` its click runs, for a button.
    pub action: Option<&'static str>,
}

/// The transport's controls, in the order they stand and the keyboard walks them
/// (`docs/design/keyboard.md`: the order comes from data, never from what is drawn).
pub const CONTROLS: [Control; 10] = [
    Control { id: "start", name: "To the start", action: Some("seek-start") },
    Control { id: "step-back", name: "Step back", action: Some("step-back") },
    Control { id: "play", name: "Play", action: Some("play") },
    Control { id: "step-forward", name: "Step forward", action: Some("step-forward") },
    Control { id: "end", name: "To the end", action: Some("seek-end") },
    Control { id: "at-end", name: "At the end", action: None },
    Control { id: "speed", name: "Speed", action: None },
    Control { id: "time", name: "Time", action: None },
    Control { id: "duration", name: "Duration", action: None },
    Control { id: "shot", name: "Shot", action: None },
];

/// The menu of the table (`ui/actions.ron`) that holds the speeds: the drop-down's rows.
pub const SPEED_MENU: &str = "Scene/Speed";

/// The menu of the table that holds what playing does at the end: the rows of the `at-end`
/// control's menu, one for each [`AtEnd`].
pub const END_MENU: &str = "Scene/At the end";

/// What playing does when the playhead reaches the end of the timeline. The window's state
/// ([`crate::Shell::at_end`]), chosen at the transport or by `end <name>`; never journalled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AtEnd {
    /// The playhead goes back to the start and playing goes on.
    #[default]
    Loop,
    /// The playhead stops at exactly the scene's duration and playing stops.
    Stop,
    /// The scene's duration grows by its length at the start of the run (an edit, one undo for
    /// the whole run) and playing goes on, to the schema's longest timeline.
    RunOn,
}

impl AtEnd {
    /// Every behaviour, in the menu's order.
    pub const ALL: [Self; 3] = [Self::Loop, Self::Stop, Self::RunOn];

    /// Its word in the console (`end run-on`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Loop => "loop",
            Self::Stop => "stop",
            Self::RunOn => "run-on",
        }
    }

    /// The behaviour of that word.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|at_end| at_end.name() == name)
    }
}

/// A run on under way: playing with [`AtEnd::RunOn`] has reached the end of the timeline.
///
/// The timeline grows by whole blocks of the length it had when the run first reached its end.
/// Each extension is an edit, after which the engine computes the signal again for the longer
/// timeline (measured on the build PC, release: about 20 µs a shot for point masses, so 1 ms
/// for `point` at 24 s and 2 s at 10⁵ shots; 12 s for `town` whatever its length, the world's
/// share). So that a scene slow to compute is not extended faster than it can be read, an
/// extension asked for while the signal of the one before is still being computed takes twice
/// as many blocks as the last (the cost of one recompute then buys twice the time).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunOn {
    /// The scene's duration when the run first reached the end, seconds: one block.
    pub block: f64,
    /// How many blocks the next extension adds at least.
    pub blocks: f64,
    /// Whether an extension has been made: the next continues it (one step of undo a run).
    pub extended: bool,
}

impl RunOn {
    /// A run that has just reached the end of a timeline `duration` seconds long.
    #[must_use]
    pub const fn new(duration: f64) -> Self {
        Self { block: duration, blocks: 1.0, extended: false }
    }

    /// The timeline's new length, seconds, for a playhead that is to go to `next` (at or past
    /// the end of a timeline `duration` long): whole blocks added, enough to hold `next`, twice
    /// as many as last time when the engine is `behind` (still computing the signal of the
    /// extension before); never more than `limit`. Marks the run as extended.
    pub fn extend(&mut self, duration: f64, next: f64, behind: bool, limit: f64) -> f64 {
        if self.extended && behind {
            self.blocks *= 2.0;
        }
        self.extended = true;
        let needed = ((next - duration) / self.block).floor() + 1.0;
        (duration + needed.max(self.blocks) * self.block).min(limit)
    }
}

/// One control of the transport as it was drawn, for tests and the keyboard.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
    /// Which control: an id of [`CONTROLS`].
    pub id: &'static str,
    /// Where it is, points.
    pub rect: Rect,
    /// What its tooltip says: its name, its key, what it does, and why it cannot be used when it
    /// cannot.
    pub tooltip: String,
    /// The action it runs, when it runs one.
    pub action: Option<&'static str>,
    /// Whether it can be used now.
    pub enabled: bool,
    /// What it shows, for a control that shows words or a number (`1×`, `3.20`, `3`).
    pub text: String,
}

/// The tower (from 0) whose shots a step goes by and the shot counter counts: the selected
/// tower, else the first. `None` when the scene has no towers.
#[must_use]
pub fn tower(shell: &Shell) -> Option<usize> {
    let towers = shell.engine.spec().array.towers.len();
    (towers > 0).then(|| shell.selected_tower().filter(|n| *n < towers).unwrap_or(0))
}

/// Where the playhead stands among one tower's shots.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    /// How many shots the tower has on the timeline, taken or not.
    pub count: usize,
    /// How many of them are read out at or before the playhead: the number (from 1) of the last
    /// shot read out, 0 before the first. What the shot's field shows.
    pub read: usize,
    /// The readout before the playhead that a step back lands on, seconds; `None` at or before
    /// the first one inside the timeline.
    pub earlier: Option<f64>,
    /// The readout after the playhead that a step forward lands on, seconds; `None` at or after
    /// the last one inside the timeline.
    pub later: Option<f64>,
}

/// Where a playhead at `time` (seconds) stands among tower `tower`'s shots (from 0) on a
/// timeline `duration` seconds long. The times are `shots`' own readouts, bit for bit: found by
/// bisection among them, never made here.
///
/// # Panics
/// When `tower` is not one of `shots`' towers.
#[must_use]
pub fn place(shots: &Shots, tower: usize, time: f64, duration: f64) -> Place {
    let count = shots.count(tower);
    // The readouts rise, so each bound is one bisection (`Shots::readout` is the same sum).
    let before = |limit: f64, inclusive: bool| {
        shots.times(tower).partition_point(|clock| {
            let readout = clock + shots.offsets[tower];
            if inclusive { readout <= limit } else { readout < limit }
        })
    };
    // Those inside the timeline are `first..end`; the playhead is among them.
    let (first, end) = (before(0.0, false), before(duration, true));
    let (below, read) = (before(time, false), before(time, true));
    let (earlier, later) = (below.min(end), read.max(first));
    Place {
        count,
        read,
        earlier: (earlier > first).then(|| shots.readout(tower, earlier - 1)),
        later: (later < end).then(|| shots.readout(tower, later)),
    }
}

/// The playhead's place among the shots of the transport's tower ([`tower`]), or why there is
/// none: no towers, or a scene whose signal is refused (the engine's reason).
///
/// # Errors
/// Why there are no shots to step by.
pub fn place_now(shell: &Shell) -> Result<Place, String> {
    let tower = tower(shell).ok_or("the scene has no towers, so no shots")?;
    let shots = shell.engine.shots().map_err(str::to_owned)?;
    if tower >= shots.towers() {
        return Err("the shots are being made".to_owned());
    }
    Ok(place(shots, tower, shell.engine.time(), shell.engine.spec().duration))
}

/// A speed as the transport writes it: `1×`, `0.25×`.
#[must_use]
pub fn speed_text(speed: f64) -> String {
    format!("{speed}×")
}

/// `step <n> [cycle]`: moves the playhead `n` shots (negative: back) of the transport's tower,
/// each step to the next readout of the engine's shots; with `cycle`, by `n` mean cadence
/// cycles of that tower's instrument (seconds, kept inside the timeline) instead. Playback
/// stops. One `seek` is issued, to the place the steps end at.
///
/// # Errors
/// When there is nowhere to step to: no shot that way, or the playhead at that end.
pub fn step(shell: &mut Shell, steps: i64, cycle: bool) -> Result<(), String> {
    let (time, duration) = (shell.engine.time(), shell.engine.spec().duration);
    let back = steps < 0;
    let target = if cycle {
        let tower = tower(shell).ok_or("the scene has no towers, so no cadence")?;
        let spec = shell.engine.spec();
        let mean = spec.array.schedule.mean_cycle(spec.array.instrument_of(tower).cycle);
        let target = (time + steps as f64 * mean).clamp(0.0, duration);
        if target == time {
            return Err(format!("the playhead is at the {}", if back { "start" } else { "end" }));
        }
        target
    } else {
        let tower = tower(shell).ok_or("the scene has no towers, so no shots")?;
        let shots = shell.engine.shots().map_err(str::to_owned)?;
        let mut at = time;
        for _ in 0..steps.unsigned_abs() {
            let here = place(shots, tower, at, duration);
            match if back { here.earlier } else { here.later } {
                Some(next) => at = next,
                None => break,
            }
        }
        if at == time {
            return Err(format!("there is no shot {} the playhead", if back { "before" } else { "after" }));
        }
        at
    };
    shell.playing = false;
    if shell.apply(Command::Seek(target)) { Ok(()) } else { Err("the seek was refused".to_owned()) }
}

/// The tooltip of the control that runs the action `id`: its name, its key and what it does,
/// and, when it cannot run now, why.
fn tooltip(shell: &Shell, id: &str) -> (String, bool) {
    let action = table().action(id).unwrap_or_else(|| panic!("`{id}` is an action of ui/actions.ron"));
    let tip = crate::header::tooltip(id);
    if shell.can_run(action) { (tip, true) } else { (format!("{tip}\n{}", action.needs.why_not()), false) }
}

/// One icon button of the transport, [`theme::ICON`] a side, drawn and recorded.
fn button(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    id: &'static str,
    glyph: Transport,
    lit: bool,
) -> (egui::Response, bool) {
    let action = CONTROLS.iter().find(|c| c.id == id).and_then(|c| c.action).expect("a button of CONTROLS");
    let (tip, enabled) = tooltip(shell, action);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(vec2(theme::ICON, theme::ICON), sense);
    let response = response.on_hover_text(&tip);
    let held = enabled && response.is_pointer_button_down_on();
    let painter = ui.painter_at(rect);
    if lit || held {
        painter.rect_filled(rect.shrink(1.0), 2.0, theme::SELECTION);
    } else if enabled && response.hovered() {
        painter.rect_filled(rect.shrink(1.0), 2.0, theme::HOVER);
    }
    let colour = if !enabled {
        theme::WEAK.gamma_multiply(0.45)
    } else if lit || held || response.hovered() {
        Color32::WHITE
    } else {
        theme::TEXT
    };
    icons::transport(&painter, Rect::from_center_size(rect.center(), vec2(14.0, 14.0)), glyph, colour);
    let line = table().action(action).map(|a| a.line.clone()).unwrap_or_default();
    shell.layout.buttons.push((line, rect));
    shell.layout.transport.push(Drawn {
        id,
        rect,
        tooltip: tip,
        action: Some(action),
        enabled,
        text: String::new(),
    });
    (response, enabled)
}

/// How many times a step button held down fires this frame: once as it goes down, then, after
/// the keyboard's delay ([`REPEAT_DELAY`]), once every [`REPEAT_EVERY`] seconds: a held button
/// scrubs as a held key does.
fn held_steps(shell: &mut Shell, ui: &egui::Ui, id: &'static str, response: &egui::Response) -> u32 {
    if !response.is_pointer_button_down_on() {
        if shell.transport_hold.is_some_and(|(held, ..)| held == id) {
            shell.transport_hold = None;
        }
        return 0;
    }
    let now = ui.input(|i| i.time);
    ui.ctx().request_repaint();
    match &mut shell.transport_hold {
        Some((held, down, fired)) if *held == id => {
            let since = now - *down;
            let total = if since < REPEAT_DELAY {
                1
            } else {
                2 + ((since - REPEAT_DELAY + 1e-9) / REPEAT_EVERY).floor() as u32
            };
            let due = total.saturating_sub(*fired).min(MOST_STEPS_A_FRAME);
            *fired = total;
            due
        }
        hold => {
            *hold = Some((id, now, 1));
            1
        }
    }
}

/// A small muted word of the row (`t`, `s`, `/`), recorded as painted.
fn word(shell: &mut Shell, ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(theme::SMALL_PT).color(theme::WEAK));
    shell.layout.chrome.push(("timeline", text.to_owned()));
}

/// How wide `text` is in the monospace face, points.
fn mono_width(ui: &egui::Ui, text: &str) -> f32 {
    ui.painter().layout_no_wrap(text.to_owned(), theme::mono(), theme::STRONG).size().x
}

/// A compact field of the row: a number shown monospace and right-aligned in a box with room
/// for text `width` points wide, typed into. Returns what was typed when the field is left with other text than
/// it showed (Enter, Tab, a click elsewhere); Esc leaves it as it was.
fn field(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    id: &'static str,
    shown: &str,
    width: f32,
    tip: &str,
) -> Option<String> {
    let key = format!("{SCOPE}/{id}");
    let mut text = match &shell.typing {
        Some((typing, buffer)) if *typing == key => buffer.clone(),
        _ => shown.to_owned(),
    };
    let edit = egui::TextEdit::singleline(&mut text)
        .font(theme::mono())
        .text_color(theme::STRONG)
        // (A little more than the text's width: a number that fills the box is not cut.)
        .desired_width(width + 6.0)
        .margin(egui::Margin::symmetric(3, 1))
        .align(egui::Align2::RIGHT_CENTER)
        .min_size(vec2(0.0, theme::HIT));
    let response = ui.add(edit).on_hover_text(tip);
    if shell.take_type(&key) {
        response.request_focus();
    }
    if response.gained_focus() {
        // What is there is replaced by what is typed: the whole of it is selected.
        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), response.id) {
            let all = egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(text.chars().count()),
            );
            state.cursor.set_char_range(Some(all));
            state.store(ui.ctx(), response.id);
        }
    }
    if response.has_focus() {
        shell.typing = Some((key.clone(), text.clone()));
    }
    shell.layout.chrome.push(("timeline", shown.to_owned()));
    shell.layout.transport.push(Drawn {
        id,
        rect: response.rect,
        tooltip: tip.to_owned(),
        action: None,
        enabled: true,
        text: shown.to_owned(),
    });
    if response.lost_focus() {
        shell.typing = None;
        let cancelled = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if !cancelled && text.trim() != shown {
            return Some(text.trim().to_owned());
        }
    }
    None
}

/// Says why something typed at the transport was refused: in the log, with the console opened,
/// as a refused edit is.
fn refuse(shell: &mut Shell, why: String) {
    shell.engine.note(cav_engine::Level::Error, cav_engine::Subsystem::App, why);
    shell.console_open = true;
}

/// One row of a transport menu: an action of the table.
struct MenuRow {
    /// The action's id.
    action: String,
    /// What the row says.
    label: String,
    /// What it does, for its tooltip.
    about: String,
    /// Whether it is the one chosen now.
    chosen: bool,
    /// Its picture, for a row of the end's menu.
    glyph: Option<AtEnd>,
}

/// The rows of the table's menu `menu`; `chosen` says which is the one in force.
fn menu_rows(menu: &str, chosen: impl Fn(&str) -> bool) -> Vec<MenuRow> {
    table()
        .actions
        .iter()
        .filter(|a| a.menu == menu)
        .map(|a| MenuRow {
            action: a.id.clone(),
            label: a.label.clone(),
            about: a.about.clone(),
            chosen: chosen(&a.line),
            glyph: a.line.strip_prefix("end ").and_then(AtEnd::parse),
        })
        .collect()
}

/// The menu of the control `id` (`speed`), standing above its opener while it is open: opened by
/// a click on the opener or by its console line (`speed menu`: [`Shell::transport_menu`]), or by
/// Enter on it from the keyboard (the keyboard's drop-down, the row the arrows are on lit). A
/// row clicked or picked runs its action; a press anywhere else closes it.
fn menu(shell: &mut Shell, ui: &egui::Ui, id: &'static str, opener: Rect, rows: &[MenuRow]) {
    let stop = format!("{SCOPE}/{id}");
    if let Some(row) = shell.take_pick(&stop).and_then(|index| rows.get(index)) {
        let _ = shell.run_action(&row.action);
    }
    let lit = shell.keyboard.popup.as_ref().filter(|p| p.id == stop).map(|p| p.at);
    let by_pointer = shell.transport_menu.as_deref() == Some(id);
    if !by_pointer && lit.is_none() {
        return;
    }
    let mut ran = None;
    let area = egui::Area::new(egui::Id::unique(("transport menu", id)))
        .order(egui::Order::Foreground)
        .pivot(Align2::LEFT_BOTTOM)
        .fixed_pos(opener.left_top() - vec2(0.0, 2.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                for (n, row) in rows.iter().enumerate() {
                    let mut text = RichText::new(&row.label);
                    if lit == Some(n) {
                        text = text.background_color(theme::SELECTION).color(Color32::WHITE);
                    }
                    let response = match row.glyph {
                        // A row with a picture: the picture, then its words, lit when chosen.
                        Some(glyph) => {
                            let padded = RichText::new(format!("       {}", row.label));
                            let lit_here = row.chosen || lit == Some(n);
                            let response = ui.selectable_label(lit_here, padded);
                            let square = Rect::from_center_size(
                                pos2(response.rect.left() + 13.0, response.rect.center().y),
                                vec2(14.0, 14.0),
                            );
                            let colour = if lit_here { Color32::WHITE } else { theme::TEXT };
                            icons::at_end(ui.painter(), square, glyph, colour);
                            response
                        }
                        None => ui.radio(row.chosen, text),
                    }
                    .on_hover_text(&row.about);
                    let line = table().action(&row.action).map(|a| a.line.clone()).unwrap_or_default();
                    shell.layout.buttons.push((line, response.rect));
                    if response.clicked() {
                        ran = Some(row.action.clone());
                    }
                }
            });
        });
    shell.layout.buttons.push((format!("{id} menu"), area.response.rect));
    if let Some(action) = ran {
        let _ = shell.run_action(&action);
        shell.transport_menu = None;
    } else if by_pointer
        && ui.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|at| !area.response.rect.contains(at) && !opener.contains(at))
        })
    {
        shell.transport_menu = None;
    }
}

/// A drop-down's closed face in the row: its words in the monospace face and the mark, in a box
/// `width` points wide. A click opens or closes its menu.
fn opener(shell: &mut Shell, ui: &mut egui::Ui, id: &'static str, text: &str, width: f32, tip: &str) -> Rect {
    let (rect, response) = ui.allocate_exact_size(vec2(width, theme::HIT), Sense::click());
    let response = response.on_hover_text(tip);
    let open = shell.transport_menu.as_deref() == Some(id);
    let painter = ui.painter_at(rect);
    let ground = if open || response.hovered() { theme::HOVER } else { theme::WIDGET };
    painter.rect_filled(rect, 2.0, ground);
    let colour = if open || response.hovered() { Color32::WHITE } else { theme::STRONG };
    painter.text(pos2(rect.left() + 5.0, rect.center().y), Align2::LEFT_CENTER, text, theme::mono(), colour);
    icons::chevron(&painter, pos2(rect.right() - 8.0, rect.center().y), 6.0, theme::WEAK_ON_WIDGET);
    if response.clicked() {
        shell.transport_menu = if open { None } else { Some(id.to_owned()) };
    }
    shell.layout.chrome.push(("timeline", text.to_owned()));
    shell.layout.buttons.push((id.to_owned(), rect));
    shell.layout.transport.push(Drawn {
        id,
        rect,
        tooltip: tip.to_owned(),
        action: None,
        enabled: true,
        text: text.to_owned(),
    });
    rect
}

/// The end behaviour's button: its picture and a drop-down's mark; a click opens its menu.
fn at_end_button(shell: &mut Shell, ui: &mut egui::Ui) {
    let at_end = shell.at_end;
    let rows = menu_rows(END_MENU, |line| line == format!("end {}", at_end.name()));
    let label = rows.iter().find(|row| row.chosen).map(|row| row.label.clone()).unwrap_or_default();
    let tip = format!(
        "At the end: {label}\nwhat playing does when the playhead reaches the end of the timeline: loop, stop \
         there, or run on (the duration grows)"
    );
    let (rect, response) = ui.allocate_exact_size(vec2(theme::ICON + 10.0, theme::ICON), Sense::click());
    let response = response.on_hover_text(&tip);
    let open = shell.transport_menu.as_deref() == Some("at-end");
    let painter = ui.painter_at(rect);
    if open || response.hovered() {
        painter.rect_filled(rect.shrink(1.0), 2.0, theme::HOVER);
    }
    let colour = if open || response.hovered() { Color32::WHITE } else { theme::TEXT };
    let square =
        Rect::from_center_size(pos2(rect.left() + 0.5 * theme::ICON, rect.center().y), vec2(14.0, 14.0));
    icons::at_end(&painter, square, at_end, colour);
    icons::chevron(&painter, pos2(rect.right() - 6.0, rect.center().y), 6.0, theme::WEAK);
    if response.clicked() {
        shell.transport_menu = if open { None } else { Some("at-end".to_owned()) };
    }
    shell.layout.buttons.push(("at-end".to_owned(), rect));
    shell.layout.transport.push(Drawn {
        id: "at-end",
        rect,
        tooltip: tip,
        action: None,
        enabled: true,
        text: at_end.name().to_owned(),
    });
    menu(shell, ui, "at-end", rect, &rows);
}

/// Draws the transport into the timeline's head row (after its title) and takes its input:
/// the five buttons, what playing does at the end, the speed, the time, the duration and the
/// shot. `facts` (the shots' count and 2T, each a
/// few words) stand at the row's right while there is room for them; `caption` is their tooltip.
pub(crate) fn draw(shell: &mut Shell, ui: &mut egui::Ui, facts: &[String], caption: &str) {
    ui.spacing_mut().item_spacing.x = theme::ROW_GAP;
    let shift = ui.input(|i| i.modifiers.shift);

    // The buttons. A step button fires as it goes down and repeats while it is held; with Shift
    // it moves a cadence cycle. The rest act on a click.
    let (start, _) = button(shell, ui, "start", Transport::Start, false);
    if start.clicked() || shell.take_click("timeline/start") {
        let _ = shell.run_action("seek-start");
    }
    let stepping = |shell: &mut Shell, ui: &mut egui::Ui, id: &'static str, glyph: Transport| {
        let (response, enabled) = button(shell, ui, id, glyph, false);
        let mut fire = if enabled { held_steps(shell, ui, id, &response) } else { 0 };
        if shell.take_click(&format!("{SCOPE}/{id}")) {
            fire += 1;
        }
        for _ in 0..fire {
            let action = if shift { format!("{id}-cycle") } else { id.to_owned() };
            if shell.run_action(&action).is_err() {
                break;
            }
        }
    };
    stepping(shell, ui, "step-back", Transport::StepBack);
    let glyph = if shell.playing { Transport::Pause } else { Transport::Play };
    let (play, _) = button(shell, ui, "play", glyph, shell.playing);
    if play.clicked() || shell.take_click("timeline/play") {
        let _ = shell.run_action("play");
    }
    stepping(shell, ui, "step-forward", Transport::StepForward);
    let (end, _) = button(shell, ui, "end", Transport::End, false);
    if end.clicked() || shell.take_click("timeline/end") {
        let _ = shell.run_action("seek-end");
    }
    at_end_button(shell, ui);
    ui.add_space(theme::GRID);

    // The speed: a drop-down of the table's speeds.
    let speed = shell.speed;
    let rows = menu_rows(SPEED_MENU, |line| line == format!("speed {speed}"));
    let widest = SPEEDS.iter().map(|s| mono_width(ui, &speed_text(*s))).fold(0.0, f32::max);
    let tip = "Speed\nhow fast the playhead moves while playing, as a multiple of the clock's rate";
    let speed_rect = opener(shell, ui, "speed", &speed_text(speed), widest + 24.0, tip);
    menu(shell, ui, "speed", speed_rect, &rows);
    ui.spacing_mut().item_spacing.x = theme::GRID;
    ui.add_space(2.0 * theme::GRID);

    // The time, typed to seek; the duration beside it, typed to set it.
    let (time, duration) = (shell.engine.time(), shell.engine.spec().duration);
    let (time_text, duration_text) = (format!("{time:.2}"), format!("{duration:.2}"));
    let digits = mono_width(ui, &duration_text).max(mono_width(ui, "00.00"));
    word(shell, ui, "t");
    let tip = "Time\nthe playhead, seconds; type a time to go to it";
    if let Some(typed) = field(shell, ui, "time", &time_text, digits, tip) {
        match typed.trim_end_matches('s').trim().parse::<f64>() {
            Ok(to) if to.is_finite() => {
                let _ = shell.run(&format!("seek {to}"));
            }
            _ => refuse(shell, format!("`{typed}` is not a time in seconds")),
        }
    }
    word(shell, ui, "/");
    let tip = "Duration\nhow long the timeline is, seconds; type a length to set it (an edit of the scene, \
               as the Scene card's Duration: Ctrl+Z takes it back)";
    if let Some(typed) = field(shell, ui, "duration", &duration_text, digits, tip) {
        match typed.trim_end_matches('s').trim().parse::<f64>() {
            Ok(length) if length.is_finite() => {
                // The same `set` the Scene card's editor issues; a length the scene cannot
                // have is refused by the engine, which says why. A run on in hand starts
                // again from the length typed.
                shell.run_on = None;
                shell.apply(crate::editors::command_for("duration", serde_json::json!(length), false));
            }
            _ => refuse(shell, format!("`{typed}` is not a duration in seconds")),
        }
    }
    word(shell, ui, "s");
    ui.add_space(2.0 * theme::GRID);

    // The shot: the last one read out, typed to go to a shot's readout.
    let place = place_now(shell);
    let (read, count) = place.as_ref().map_or((0, 0), |p| (p.read, p.count));
    let shot_text = read.to_string();
    let which = tower(shell)
        .and_then(|n| shell.engine.spec().array.towers.get(n).map(|t| t.label.clone()))
        .unwrap_or_else(|| "no tower".to_owned());
    let tip = format!(
        "Shot\nthe last shot read out at or before the playhead, of {which}'s {count} (the selected tower's \
         shots, else the first tower's); type a number to go to that shot's readout"
    );
    word(shell, ui, "shot");
    let digits = mono_width(ui, &count.max(10).to_string());
    if let Some(typed) = field(shell, ui, "shot", &shot_text, digits, &tip) {
        seek_shot(shell, &typed);
    }
    let of = format!("/ {count}");
    ui.label(RichText::new(&of).font(theme::mono()).color(theme::STRONG)).on_hover_text(&tip);
    shell.layout.chrome.push(("timeline", of));

    // The shots' facts at the row's right, while there is room: they are the title's tooltip too.
    let font = egui::FontId::monospace(theme::SMALL_PT);
    let wide: f32 = facts
        .iter()
        .map(|fact| ui.painter().layout_no_wrap(fact.clone(), font.clone(), theme::WEAK).size().x + 12.0)
        .sum();
    if wide + 12.0 <= ui.available_width() {
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            ui.add_space(theme::GRID);
            for fact in facts.iter().rev() {
                ui.label(RichText::new(fact).font(font.clone()).color(theme::WEAK)).on_hover_text(caption);
                shell.layout.chrome.push(("timeline", fact.clone()));
            }
        });
    }
    register(shell);
}

/// Goes to the readout of the shot whose number (from 1) was typed in the shot's field, or says
/// why not.
fn seek_shot(shell: &mut Shell, typed: &str) {
    let Some(tower) = tower(shell) else {
        return refuse(shell, "the scene has no towers, so no shots".to_owned());
    };
    let duration = shell.engine.spec().duration;
    let found = match shell.engine.shots() {
        Err(why) => Err(why.to_owned()),
        Ok(shots) => match typed.parse::<usize>() {
            Ok(n) if n >= 1 && n <= shots.count(tower) => {
                let readout = shots.readout(tower, n - 1);
                if (0.0..=duration).contains(&readout) {
                    Ok(readout)
                } else {
                    Err(format!(
                        "shot {n} is read out at {readout} s, outside the timeline (0 to {duration} s)"
                    ))
                }
            }
            _ => Err(format!("`{typed}` is not a shot; the tower has shots 1 to {}", shots.count(tower))),
        },
    };
    match found {
        Ok(readout) => {
            shell.playing = false;
            shell.apply(Command::Seek(readout));
        }
        Err(why) => refuse(shell, why),
    }
}

/// Registers the transport's stops with the keyboard, under [`SCOPE`]: every control of
/// [`CONTROLS`], in that order (the order is the data's, whatever was drawn). A button that
/// cannot be used now is there all the same, disabled.
fn register(shell: &mut Shell) {
    for control in &CONTROLS {
        let id = format!("{SCOPE}/{}", control.id);
        let drawn = shell.layout.transport.iter().rev().find(|d| d.id == control.id).cloned();
        let kind = match control.id {
            "speed" => {
                let speed = shell.speed;
                let rows = menu_rows(SPEED_MENU, |line| line == format!("speed {speed}"));
                Kind::Choice { count: rows.len(), current: rows.iter().position(|r| r.chosen).unwrap_or(0) }
            }
            "at-end" => {
                let current = AtEnd::ALL.iter().position(|a| *a == shell.at_end).unwrap_or(0);
                Kind::Choice { count: AtEnd::ALL.len(), current }
            }
            "time" | "duration" | "shot" => Kind::Text,
            _ => Kind::Button,
        };
        shell.register(SCOPE, &id, control.name, kind, drawn.as_ref().map_or(Rect::NOTHING, |d| d.rect));
        if !drawn.is_some_and(|d| d.enabled) {
            shell.building.disable(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cav_sim::{InstrumentSpec, SceneSpec};

    fn shots(duration: f64, offset: f64) -> Shots {
        let mut scene = SceneSpec::room();
        (scene.array.instrument, scene.duration) = (InstrumentSpec::default(), duration);
        scene.array.towers[0].timing.offset = offset;
        cav_sim::read::shots(&scene).unwrap()
    }

    #[test]
    fn a_place_is_among_the_shots_own_readouts() {
        // Six shots, read out at 1.46, 3.46, …, 11.46 s on a 12 s timeline.
        let all = shots(12.0, 0.0);
        let readout = |k: usize| all.readout(0, k);
        let at = |time: f64| place(&all, 0, time, 12.0);
        assert_eq!(at(0.0), Place { count: 6, read: 0, earlier: None, later: Some(readout(0)) });
        // On a shot: it is read out; the steps are its neighbours, not itself.
        assert_eq!(
            at(readout(2)),
            Place { count: 6, read: 3, earlier: Some(readout(1)), later: Some(readout(3)) }
        );
        // Between two shots: the one before and the one after.
        assert_eq!(at(4.0), Place { count: 6, read: 2, earlier: Some(readout(1)), later: Some(readout(2)) });
        assert_eq!(at(12.0), Place { count: 6, read: 6, earlier: Some(readout(5)), later: None });
        // A shot launched before the end and read out after it is counted and cannot be reached.
        let cut = shots(10.5, 0.0);
        assert_eq!(cut.count(0), 6);
        assert!(cut.readout(0, 5) > 10.5);
        let last = place(&cut, 0, cut.readout(0, 4), 10.5);
        assert_eq!((last.read, last.later), (5, None), "the sixth is read out after the end");
        // An offset that puts the first readout before zero: it cannot be stepped back to.
        let early = shots(12.0, -2.0);
        assert!(early.readout(0, 0) < 0.0);
        let first = place(&early, 0, early.readout(0, 1), 12.0);
        assert_eq!((first.read, first.earlier), (2, None));
        assert_eq!(place(&early, 0, 0.0, 12.0).later, Some(early.readout(0, 1)));
        // Control: every answer is one of the shots' own times, to the bit.
        for time in [0.0, 0.3, 1.46, 5.0, 7.777, 11.9] {
            let here = at(time);
            for found in [here.earlier, here.later].into_iter().flatten() {
                assert!((0..6).any(|k| readout(k).to_bits() == found.to_bits()), "{found} at {time}");
            }
        }
    }

    #[test]
    fn a_run_on_grows_by_whole_blocks_and_by_more_when_the_engine_is_behind() {
        let limit = 1.0e6;
        // The engine keeping up: one block of the starting length each time.
        let mut run = RunOn::new(12.0);
        assert_eq!(run.extend(12.0, 12.01, false, limit), 24.0);
        assert_eq!(run.extend(24.0, 24.01, false, limit), 36.0, "not doubled");
        assert_eq!(run.extend(36.0, 36.01, false, limit), 48.0);
        // Control: behind (the last extension's signal still being computed), twice as many
        // blocks as the time before; the first extension is one block whatever the signal is.
        let mut slow = RunOn::new(12.0);
        assert_eq!(slow.extend(12.0, 12.01, true, limit), 24.0);
        assert_eq!(slow.extend(24.0, 24.01, true, limit), 48.0);
        assert_eq!(slow.extend(48.0, 48.01, true, limit), 96.0);
        assert_eq!(slow.extend(96.0, 96.01, false, limit), 144.0, "caught up: four blocks, as last time");
        // A step past several blocks: enough of them to hold the playhead.
        let mut fast = RunOn::new(0.1);
        let longer = fast.extend(0.1, 0.47, false, limit);
        assert!((longer - 0.5).abs() < 1e-12 && longer >= 0.47, "{longer}");
        // Never past the limit.
        let mut long = RunOn::new(600_000.0);
        assert_eq!(long.extend(600_000.0, 600_000.01, false, limit), limit);
        assert!(long.extended && !RunOn::new(1.0).extended);
    }

    #[test]
    fn the_controls_name_actions_and_the_speeds_are_the_table_s() {
        for control in &CONTROLS {
            if let Some(action) = control.action {
                assert!(table().action(action).is_some(), "{action}");
            }
        }
        for id in ["step-back-cycle", "step-forward-cycle"] {
            assert!(table().action(id).is_some(), "{id}");
        }
        let lines: Vec<String> =
            table().actions.iter().filter(|a| a.menu == SPEED_MENU).map(|a| a.line.clone()).collect();
        let wanted: Vec<String> = SPEEDS.iter().map(|s| format!("speed {s}")).collect();
        assert_eq!(lines, wanted, "the drop-down's rows are the speeds, in order");
        let ends: Vec<String> =
            table().actions.iter().filter(|a| a.menu == END_MENU).map(|a| a.line.clone()).collect();
        let wanted: Vec<String> = AtEnd::ALL.iter().map(|a| format!("end {}", a.name())).collect();
        assert_eq!(ends, wanted, "the end's menu is every behaviour, in order");
        assert_eq!(AtEnd::ALL.map(|a| AtEnd::parse(a.name())), AtEnd::ALL.map(Some));
        assert_eq!((AtEnd::default(), AtEnd::parse("sideways")), (AtEnd::Loop, None));
        assert_eq!(speed_text(0.25), "0.25×");
        assert_eq!(speed_text(1.0), "1×");
    }
}
