//! The shell: the application's state around the engine, and the one place user input turns into
//! commands. Buttons, keys, menus and the console all end in [`Shell::run`] or [`Shell::apply`],
//! which end in the engine's API; nothing here edits the scene or the views any other way.

use cav_engine::{
    Command, Engine, Level, LogFilter, Query, ReadingReply, Reply, Request, Subsystem, Target, TowerReading,
    ViewCommand,
};
use egui::{Key, Rect};

use crate::panes::PaneSurface;

/// Console commands the shell answers itself, because they are about the window, not the engine.
pub const APP_COMMANDS: &[(&str, &str)] = &[
    ("console", "open or close the console"),
    ("gui", "hide or show the panels; the scene fills the window (F3)"),
    ("play", "start or stop playback"),
    ("pause", "stop playback"),
    ("keys", "the keyboard reference"),
    ("about", "version and credits"),
    ("clear", "empty the console's log"),
    ("quit", "close the application"),
];

/// The first `lines` lines of `text`, and how many were left out. The console shows a long
/// answer this way; `cav exec` prints all of it.
#[must_use]
pub fn abridged(text: &str, lines: usize) -> String {
    let total = text.lines().count();
    if total <= lines {
        return text.to_owned();
    }
    let head: Vec<&str> = text.lines().take(lines).collect();
    format!("{}\n… {} more lines (`cav exec` prints them all)", head.join("\n"), total - lines)
}

/// How long one step with the arrow keys is, seconds.
const STEP: f64 = 0.1;

/// Which log entries the console shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ConsoleFilter {
    /// Every entry.
    #[default]
    All,
    /// Warnings and errors.
    Warnings,
    /// Errors only.
    Errors,
}

/// Where everything was put in the last frame, in points. Tests read it; so does the screenshot
/// check that no panel covers the figure.
#[derive(Clone, Debug)]
pub struct Layout {
    /// The whole window, points.
    pub screen: Rect,
    /// Each panel that was shown, by name: `menu`, `outliner`, `properties`, `timeline`,
    /// `console`, `footer`, `header`.
    pub panels: Vec<(&'static str, Rect)>,
    /// The area the panes share.
    pub figure: Rect,
    /// Each pane, in order.
    pub panes: Vec<Rect>,
    /// Buttons worth finding from a test, by the console line they run.
    pub buttons: Vec<(String, Rect)>,
    /// What each pane that shows a plot drew.
    pub plots: Vec<crate::plots::Drawn>,
    /// Each property editor that was shown: the path it edits and where its widget is.
    pub editors: Vec<(String, Rect)>,
    /// The first shot of each timeline lane that was drawn: the tower (from 0) and its bar,
    /// from the tower's launch to its readout.
    pub lanes: Vec<(usize, Rect)>,
    /// The numbers on the timeline's ruler, each with the rectangle it took, and the rectangle
    /// the timeline is drawn inside.
    pub ruler: (Vec<(String, Rect)>, Rect),
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            screen: Rect::NOTHING,
            panels: Vec::new(),
            figure: Rect::NOTHING,
            panes: Vec::new(),
            buttons: Vec::new(),
            plots: Vec::new(),
            editors: Vec::new(),
            lanes: Vec::new(),
            ruler: (Vec::new(), Rect::NOTHING),
        }
    }
}

impl Layout {
    /// The panel called `name`, if it was shown.
    #[must_use]
    pub fn panel(&self, name: &str) -> Option<Rect> {
        self.panels.iter().find(|(n, _)| *n == name).map(|(_, r)| *r)
    }

    /// The editor of the field at `path`, if it was shown.
    #[must_use]
    pub fn editor(&self, path: &str) -> Option<Rect> {
        self.editors.iter().find(|(p, _)| p == path).map(|(_, r)| *r)
    }

    /// The button that runs `line`, if it was shown.
    #[must_use]
    pub fn button(&self, line: &str) -> Option<Rect> {
        self.buttons.iter().find(|(l, _)| l == line).map(|(_, r)| *r)
    }
}

/// The application's state.
pub struct Shell {
    /// The engine this window drives. All scene state is here: the shell sends commands and reads replies.
    pub engine: Engine,
    /// Whether the console is shown.
    pub console_open: bool,
    /// Which log entries the console shows.
    pub console_filter: ConsoleFilter,
    pub(crate) console_line: String,
    pub(crate) history: Vec<String>,
    pub(crate) history_at: Option<usize>,
    pub(crate) focus_console: bool,
    /// Whether the panels are hidden, leaving the figure alone.
    pub gui_hidden: bool,
    /// Whether the playhead is advancing with the clock.
    pub playing: bool,
    /// Whether the list of keys is shown.
    pub keys_open: bool,
    /// Whether the About box is shown.
    pub about_open: bool,
    /// Set by `quit`; the window closes at the end of the frame.
    pub quit: bool,
    /// The adapter's name, for the About box.
    pub adapter: Option<String>,
    /// How long the last frame took, for the footer; `None` in screenshots, so they are repeatable.
    pub frame_ms: Option<f32>,
    pub(crate) layout: Layout,
    /// The body being dragged, from 0, and the pane it is dragged in.
    pub(crate) dragging: Option<(usize, usize)>,
    /// Whether the drag in hand has already moved its body: its next step continues that edit.
    pub(crate) drag_under_way: bool,
    /// Every tower's reading under the playhead, and the engine's generation it was read at.
    readings: Option<(u64, ReadingReply)>,
    /// The path whose editor is being dragged, once a step of the drag has been applied: the
    /// next step continues that edit.
    pub(crate) editing: Option<String>,
    /// The text field being typed in: its path and what has been typed so far. The scene is
    /// not edited until the field is left.
    pub(crate) typing: Option<(String, String)>,
}

/// How near a body's picture the pointer must be to take hold of it, points.
pub const GRAB_RADIUS: f32 = 16.0;

/// A phase as people read it: in rad, mrad, µrad or nrad, with four figures.
#[must_use]
pub fn phase_text(radians: f64) -> String {
    let size = radians.abs();
    if !radians.is_finite() {
        return if radians.is_nan() { "not a number".into() } else { "unbounded".into() };
    }
    let (scaled, unit) = if size == 0.0 || size >= 1.0 {
        (radians, "rad")
    } else if size >= 1e-3 {
        (radians * 1e3, "mrad")
    } else if size >= 1e-6 {
        (radians * 1e6, "µrad")
    } else {
        (radians * 1e9, "nrad")
    };
    format!("{scaled:+.3} {unit}")
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

impl Shell {
    /// A shell on a fresh engine: the default scene, one pane, the console closed.
    #[must_use]
    pub fn new() -> Self {
        let mut engine = Engine::new();
        // The console's `figure` line is drawn by cav-plot: the engine does not draw.
        cav_plot::figure::install(&mut engine);
        Self {
            engine,
            console_open: false,
            console_filter: ConsoleFilter::All,
            console_line: String::new(),
            history: Vec::new(),
            history_at: None,
            focus_console: false,
            gui_hidden: false,
            playing: false,
            keys_open: false,
            about_open: false,
            quit: false,
            adapter: None,
            frame_ms: None,
            layout: Layout::default(),
            dragging: None,
            drag_under_way: false,
            readings: None,
            editing: None,
            typing: None,
        }
    }

    /// Where the last frame put things.
    #[must_use]
    pub const fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Whether `line`'s first word is one of the shell's own commands.
    #[must_use]
    pub fn is_app_command(line: &str) -> bool {
        line.split_whitespace().next().is_some_and(|word| APP_COMMANDS.iter().any(|(name, _)| *name == word))
    }

    /// Runs one console line: the shell's own commands here, everything else in the engine.
    /// A failure opens the console, where the engine's log has the reason (M13-R17).
    ///
    /// # Errors
    /// Why the line was not understood or was refused.
    pub fn run(&mut self, line: &str) -> Result<String, String> {
        let line = line.trim();
        if Self::is_app_command(line) {
            return self.run_app_command(line);
        }
        let is_command = matches!(self.engine.parse(line), Ok(Request::Do(_)));
        let outcome = self.engine.run_line(line);
        match &outcome {
            // An answer to a question is shown where it was asked: in the console.
            Ok(answer) if !is_command => {
                // One entry for the whole answer, so a long one cannot push the session's history
                // out of the log. `log` is not echoed at all: the console is already showing it.
                if line.split_whitespace().next() != Some("log") {
                    self.engine.note(Level::Info, Subsystem::App, answer.as_str());
                }
                self.console_open = true;
            }
            Ok(_) => {}
            Err(_) => self.console_open = true,
        }
        outcome
    }

    fn run_app_command(&mut self, line: &str) -> Result<String, String> {
        let mut words = line.split_whitespace();
        let name = words.next().unwrap_or_default();
        if words.next().is_some() {
            let why = format!("`{name}` takes no arguments");
            self.engine.note(Level::Error, Subsystem::App, why.clone());
            self.console_open = true;
            return Err(why);
        }
        match name {
            "console" => {
                self.console_open = !self.console_open;
                self.focus_console = self.console_open;
            }
            "gui" => self.gui_hidden = !self.gui_hidden,
            "play" => self.playing = !self.playing,
            "pause" => self.playing = false,
            "keys" => self.keys_open = !self.keys_open,
            "about" => self.about_open = !self.about_open,
            "clear" => self.engine.clear_log(),
            "quit" => self.quit = true,
            _ => unreachable!("is_app_command checked the name"),
        }
        Ok(name.to_owned())
    }

    /// Applies a typed command, as a drag or a scrub issues it. A refusal opens the console.
    /// True when it was applied.
    pub fn apply(&mut self, command: Command) -> bool {
        let applied = self.engine.apply(command).is_ok();
        if !applied {
            self.console_open = true;
        }
        applied
    }

    /// What every tower reads from a shot read out at the playhead, as the engine's reading worker
    /// last had it: the numbers, whether they are the playhead's now (`refined`) or an earlier
    /// answer (`stale`), the audit's estimate and its warnings. Asked again whenever the engine's
    /// state has changed; nothing is computed here.
    pub fn reading(&mut self) -> &ReadingReply {
        let generation = self.engine.generation();
        if self.readings.as_ref().is_none_or(|(at, _)| *at != generation) {
            let reply = match self.engine.query(&Query::Reading(None)) {
                Ok(Reply::Reading(reply)) => reply,
                _ => ReadingReply {
                    time: self.engine.time(),
                    state: cav_engine::Standing::Computing,
                    towers: Vec::new(),
                    estimate: 0.0,
                    settled: true,
                    warnings: Vec::new(),
                },
            };
            self.readings = Some((generation, reply));
        }
        &self.readings.as_ref().expect("just read").1
    }

    /// Each tower's reading, from [`Self::reading`].
    pub fn readings(&mut self) -> &[TowerReading] {
        &self.reading().towers
    }

    /// Where body `n` (from 0) is drawn in a pane of `size` points, from the pane's top-left corner;
    /// `None` when it is behind the camera.
    #[must_use]
    pub fn body_point(&self, pane: usize, n: usize, size: egui::Vec2) -> Option<egui::Pos2> {
        let spec = self.engine.spec();
        let body = spec.bodies.get(n)?;
        let at = body.motion.position(self.engine.time(), spec.array.instrument.gravity);
        let camera = cav_render::Camera::new(self.engine.view(pane), f64::from(size.x / size.y));
        // The camera's own arithmetic, in points instead of pixels.
        let scale = 1024.0;
        let whole = [(f64::from(size.x) * scale) as u32, (f64::from(size.y) * scale) as u32];
        let [x, y] = camera.pixel(cav_core::Vec3::from_array(at), whole)?;
        Some(egui::pos2((x / scale) as f32, (y / scale) as f32))
    }

    /// The body whose picture is nearest `point` in a pane, if one is within reach.
    #[must_use]
    pub fn body_under(&self, pane: usize, point: egui::Pos2, size: egui::Vec2) -> Option<usize> {
        let near = |n: usize| self.body_point(pane, n, size).map(|at| (n, at.distance(point)));
        let nearest =
            (0..self.engine.spec().bodies.len()).filter_map(near).min_by(|a, b| a.1.total_cmp(&b.1))?;
        (nearest.1 <= GRAB_RADIUS).then_some(nearest.0)
    }

    /// Drags body `n` by `delta` points across a pane: it moves in the plane that faces the camera
    /// through where it is, so that it stays under the pointer. A `Set` of the point its motion is
    /// anchored at; a run of them is one edit.
    pub(crate) fn drag_body_by(&mut self, pane: usize, n: usize, delta: egui::Vec2, pane_size: egui::Vec2) {
        let spec = self.engine.spec();
        let Some(body) = spec.bodies.get(n) else { return };
        let view = self.engine.view(pane);
        let camera = cav_render::Camera::new(view, f64::from(pane_size.x / pane_size.y));
        let at = cav_core::Vec3::from_array(
            body.motion.position(self.engine.time(), spec.array.instrument.gravity),
        );
        // Metres per point at the body's own depth: in perspective that grows with the distance
        // from the eye; in an orthographic view it is the same everywhere.
        let at_target = 2.0 * camera.half_height / f64::from(pane_size.y);
        let scale = match view.projection {
            cav_sim::Projection::Orthographic => at_target,
            cav_sim::Projection::Perspective => {
                at_target * (at - camera.origin - camera.eye).dot(camera.forward) / view.distance
            }
        };
        let moved = camera.right * (f64::from(delta.x) * scale) - camera.up * (f64::from(delta.y) * scale);
        let (anchor, field) = body.motion.anchor();
        let value = [anchor[0] + moved.x, anchor[1] + moved.y, anchor[2] + moved.z];
        // The first step of a drag that is applied is an edit; the steps after it continue it. A
        // step that is refused (the body dragged out of range, say) continues nothing.
        let continues = self.drag_under_way;
        let applied = self.apply(Command::Set {
            path: format!("bodies[{n}].motion.{field}"),
            value: serde_json::json!(value),
            continues,
        });
        self.drag_under_way = continues || applied;
    }

    /// The selected tower, from 0.
    #[must_use]
    pub fn selected_tower(&self) -> Option<usize> {
        match self.engine.selection() {
            Some(Target::Tower(n)) => Some(n - 1),
            _ => None,
        }
    }

    /// Turns this frame's key presses into commands. Keys go to a text field first if one has the
    /// keyboard; the function keys and Escape always work.
    pub(crate) fn keys(&mut self, ctx: &egui::Context) {
        let typing = ctx.egui_wants_keyboard_input();
        let pressed = |key: Key| ctx.input(|i| i.key_pressed(key));
        let (command, shift) = ctx.input(|i| (i.modifiers.command, i.modifiers.shift));
        let mut lines: Vec<String> = Vec::new();
        if pressed(Key::F3) {
            lines.push("gui".into());
        }
        if pressed(Key::Escape) {
            // Escape backs out one step: a window, then the console, then the hidden GUI, then
            // the selection.
            if self.keys_open || self.about_open {
                self.keys_open = false;
                self.about_open = false;
            } else if self.console_open {
                self.console_open = false;
            } else if self.gui_hidden {
                self.gui_hidden = false;
            } else if self.engine.selection().is_some() {
                lines.push("select none".into());
            }
        }
        if pressed(Key::Backtick) {
            lines.push("console".into());
            // The backtick that opened the console should not be typed into it.
            self.console_line.retain(|c| c != '`');
        }
        if !typing {
            if pressed(Key::Space) {
                lines.push("play".into());
            }
            if pressed(Key::O) {
                lines.push("proj toggle".into());
            }
            if pressed(Key::Home) {
                lines.push("view home".into());
            }
            if pressed(Key::Questionmark) {
                lines.push("keys".into());
            }
            if command && pressed(Key::Z) {
                lines.push(if shift { "redo" } else { "undo" }.into());
            }
            for (key, direction) in [(Key::ArrowLeft, -1.0), (Key::ArrowRight, 1.0)] {
                if pressed(key) {
                    self.playing = false;
                    self.apply(Command::Seek(self.engine.time() + direction * STEP));
                }
            }
            let towers = self.engine.spec().array.towers.len();
            for (n, key) in [
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
                Key::Num8,
                Key::Num9,
            ]
            .into_iter()
            .enumerate()
            {
                if !command && pressed(key) && n < towers {
                    lines.push(format!("select tower {}", n + 1));
                }
            }
        }
        for line in lines {
            // A refusal (nothing to undo, say) is already in the log and opens the console.
            let _ = self.run(&line);
        }
    }

    /// Moves the playhead while playing, by the frame's time step; loops at the end.
    pub(crate) fn advance(&mut self, ctx: &egui::Context) {
        if !self.playing {
            return;
        }
        let step = f64::from(ctx.input(|i| i.stable_dt)).min(0.1);
        let duration = self.engine.spec().duration;
        let next = self.engine.time() + step;
        self.apply(Command::Seek(if next >= duration { 0.0 } else { next }));
        ctx.request_repaint();
    }

    /// The log entries the console shows, oldest first, at most `limit`.
    pub(crate) fn console_entries(&self, limit: usize) -> Vec<cav_engine::LogEntry> {
        let min_level = match self.console_filter {
            ConsoleFilter::All => None,
            ConsoleFilter::Warnings => Some(Level::Warn),
            ConsoleFilter::Errors => Some(Level::Error),
        };
        self.engine.log().select(&LogFilter { min_level, last: Some(limit), ..LogFilter::default() })
    }

    /// How many warnings and errors the log holds.
    pub(crate) fn problem_counts(&self) -> (usize, usize) {
        let mut counts = (0, 0);
        for entry in self.engine.log().iter() {
            match entry.level {
                Level::Warn => counts.0 += 1,
                Level::Error => counts.1 += 1,
                _ => {}
            }
        }
        counts
    }

    /// Draws one frame of the application into `ui` and handles its input.
    pub fn ui(&mut self, ui: &mut egui::Ui, surface: &mut dyn PaneSurface) {
        let ctx = ui.ctx().clone();
        self.layout = Layout { screen: ui.max_rect(), ..Layout::default() };
        self.keys(&ctx);
        self.advance(&ctx);
        // Take whatever the forward worker has finished; while it is still at work, keep drawing so
        // that its result is shown when it comes. Nothing here waits.
        self.engine.poll();
        if self.engine.signal().0 == cav_engine::SignalState::Stale
            || self.reading().state == cav_engine::Standing::Stale
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
        crate::panels::draw(self, ui, surface);
    }

    /// A drag in a pane: the primary button orbits, the secondary or middle button pans.
    pub(crate) fn orbit_by(&mut self, pane: usize, delta: egui::Vec2) {
        let view = self.engine.view(pane);
        let command = ViewCommand::Orbit {
            pane,
            yaw: view.yaw - f64::from(delta.x) * 0.4,
            pitch: view.pitch + f64::from(delta.y) * 0.4,
        };
        self.apply(Command::View(command));
    }

    pub(crate) fn pan_by(&mut self, pane: usize, delta: egui::Vec2, pane_size: egui::Vec2) {
        let view = self.engine.view(pane);
        let camera = cav_render::Camera::new(view, f64::from(pane_size.x / pane_size.y));
        // Metres per point at the target's depth: the scene follows the pointer.
        let scale = 2.0 * camera.half_height / f64::from(pane_size.y);
        let moved = camera.origin - camera.right * (f64::from(delta.x) * scale)
            + camera.up * (f64::from(delta.y) * scale);
        self.apply(Command::View(ViewCommand::Pan { pane, target: moved.to_array() }));
    }

    pub(crate) fn zoom_by(&mut self, pane: usize, scroll: f32) {
        let distance = self.engine.view(pane).distance * f64::from(-scroll * 0.002).exp();
        self.apply(Command::View(ViewCommand::Zoom { pane, distance }));
    }
}
