//! The shell: the application's state around the engine, and the one place user input turns into
//! commands. Buttons, keys, menus and the console all end in [`Shell::run`] or [`Shell::apply`],
//! which end in the engine's API; nothing here edits the scene or the views any other way.

use cav_engine::{Command, Engine, Level, LogFilter, Request, Subsystem, Target, ViewCommand};
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

/// How long one step with the arrow keys is, seconds.
const STEP: f64 = 0.1;

/// Which log entries the console shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ConsoleFilter {
    #[default]
    All,
    Warnings,
    Errors,
}

/// Where everything was put in the last frame, in points. Tests read it; so does the screenshot
/// check that no panel covers the figure.
#[derive(Clone, Debug)]
pub struct Layout {
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
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            screen: Rect::NOTHING,
            panels: Vec::new(),
            figure: Rect::NOTHING,
            panes: Vec::new(),
            buttons: Vec::new(),
        }
    }
}

impl Layout {
    /// The panel called `name`, if it was shown.
    #[must_use]
    pub fn panel(&self, name: &str) -> Option<Rect> {
        self.panels.iter().find(|(n, _)| *n == name).map(|(_, r)| *r)
    }

    /// The button that runs `line`, if it was shown.
    #[must_use]
    pub fn button(&self, line: &str) -> Option<Rect> {
        self.buttons.iter().find(|(l, _)| l == line).map(|(_, r)| *r)
    }
}

/// The application's state.
pub struct Shell {
    pub engine: Engine,
    pub console_open: bool,
    pub console_filter: ConsoleFilter,
    pub(crate) console_line: String,
    pub(crate) history: Vec<String>,
    pub(crate) history_at: Option<usize>,
    pub(crate) focus_console: bool,
    pub gui_hidden: bool,
    pub playing: bool,
    pub keys_open: bool,
    pub about_open: bool,
    /// Set by `quit`; the window closes at the end of the frame.
    pub quit: bool,
    /// The adapter's name, for the About box.
    pub adapter: Option<String>,
    /// How long the last frame took, for the footer; `None` in screenshots, so they are repeatable.
    pub frame_ms: Option<f32>,
    pub(crate) layout: Layout,
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
        Self {
            engine: Engine::new(),
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
                for row in answer.lines().take(400) {
                    self.engine.note(Level::Info, Subsystem::App, row);
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
    pub fn apply(&mut self, command: Command) {
        if self.engine.apply(command).is_err() {
            self.console_open = true;
        }
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
