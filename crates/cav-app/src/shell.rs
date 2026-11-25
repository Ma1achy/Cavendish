//! The shell: the application's state around the engine, and the one place user input turns into
//! commands. Buttons, keys, menus and the console all end in [`Shell::run`] or [`Shell::apply`],
//! which end in the engine's API; nothing here edits the scene or the views any other way.

use cav_engine::{
    Command, Engine, Level, LogFilter, Query, ReadingReply, Reply, Request, Subsystem, Target, TowerReading,
    ViewCommand,
};
use egui::{Event, Rect};

use crate::actions::{Action, Needs};
use crate::keyboard::{Effect, FocusTree, Keyboard, Kind, Press, Repeat};
use crate::palette::Palette;
use crate::panes::PaneSurface;

/// Console commands the shell answers itself, because they are about the window, not the engine.
pub const APP_COMMANDS: &[(&str, &str)] = &[
    ("console", "open or close the console"),
    ("gui", "hide or show the panels; the scene fills the window (F3)"),
    ("play", "start or stop playback"),
    ("pause", "stop playback"),
    ("keys", "the keyboard reference"),
    (
        "palette",
        "the command palette: search every action, starter, scene, command and field (Ctrl+K); `palette add` lists what can be added, `palette <menu>` one menu",
    ),
    ("rename", "type a new name for the selected body or tower in the scene tree (F2)"),
    ("frame", "fit the selection (or the whole scene) into the active pane (F)"),
    ("tool", "take a tool in hand: `tool select` (Q), `tool move` (W), `tool place` (A)"),
    (
        "reset",
        "put a field back as the scene was opened or saved: `reset bodies[0].mass` (Backspace on a field)",
    ),
    (
        "focus",
        "the keyboard's focus to a scope or element by its id, the ring showing: `focus inspector`, `focus field/duration`",
    ),
    ("history", "the list of edits: click one to undo or redo to it (Ctrl+H)"),
    ("wizard", "start a new scene: the array, the surroundings, a first body, the duration (Ctrl+N)"),
    ("open", "open a scene: the library, or a file by its path (Ctrl+O)"),
    ("save-as", "save the scene to a file named in a box (Ctrl+S saves to the file it came from)"),
    ("save-scene", "save the scene to the file it came from, or ask for one (Ctrl+S)"),
    (
        "layout",
        "arrange the window for a task: `layout scene` (Ctrl+1), `layout instrument` (Ctrl+2), `layout analysis` (Ctrl+3)",
    ),
    (
        "maximise",
        "the active pane fills the window but for the menu bar and the status line; again to restore (Ctrl+Space)",
    ),
    ("fullscreen", "the window fills the screen; again to restore (F11)"),
    ("snap", "snap the Move tool's moves to the step, or not (S)"),
    (
        "fly",
        "fly the camera: W A S D, E up, Q down, arrows or a drag to look; again, Enter or Esc to stop (Shift+F)",
    ),
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

/// How long one step of the timeline's scrubber is, seconds (Shift ×10, Alt ×0.1).
pub const SCRUB_STEP: f64 = 0.1;

/// The ring's colour in navigation mode (orange) and in interaction mode (blue), sRGB: the spec's
/// §10.
pub const RING_NAVIGATION: egui::Color32 = egui::Color32::from_rgb(0xf0, 0x96, 0x2e);
/// See [`RING_NAVIGATION`].
pub const RING_INTERACTION: egui::Color32 = egui::Color32::from_rgb(0x3d, 0x9b, 0xff);

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
    /// Every bar the lanes drew: at most `timeline::MAX_SHOTS_DRAWN` for each lane in view.
    pub shots: Vec<crate::timeline::Bar>,
    /// What the timeline's heading says about the shots.
    pub timeline_caption: String,
    /// What the outliner says about the forward stage.
    pub status: String,
    /// Each row the outliner laid out: what a click on it selects, and where it is.
    pub outline: Vec<(cav_engine::Target, Rect)>,
    /// The words each of those rows showed, in the same order: its label and its detail (a
    /// tower's reading, a body's mass), as drawn.
    pub outline_words: Vec<(cav_engine::Target, String, String)>,
    /// The numbers on the timeline's ruler, each with the rectangle it took, and the rectangle
    /// the timeline is drawn inside.
    pub ruler: (Vec<(String, Rect)>, Rect),
    /// Each scene pane's legend of the field that colours its voxel bodies: the pane (from 0)
    /// and where the legend was painted.
    pub legends: Vec<(usize, Rect)>,
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
            shots: Vec::new(),
            timeline_caption: String::new(),
            status: String::new(),
            outline: Vec::new(),
            outline_words: Vec::new(),
            ruler: (Vec::new(), Rect::NOTHING),
            legends: Vec::new(),
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
    /// What is typed on the console's line, not yet run.
    pub console_line: String,
    pub(crate) history: Vec<String>,
    pub(crate) history_at: Option<usize>,
    pub(crate) focus_console: bool,
    /// Whether the panels are hidden, leaving the figure alone.
    pub gui_hidden: bool,
    /// Whether the playhead is advancing with the clock.
    pub playing: bool,
    /// Whether the list of keys is shown.
    pub keys_open: bool,
    /// How far the `?` sheet's list is scrolled, points (kept so the keys can scroll it).
    pub sheet_scroll: f32,
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
    /// What the renderer could not draw in the last picture of the scene (the world's caps:
    /// a far field cut to its budget, meshes or bricks past their caps), shown as badges in the
    /// header (`docs/slices/S4.md` §7.6, the review's I5) and said once in the log.
    pub render_notes: Vec<String>,
    /// The keyboard model's state (`docs/design/keyboard.md`): UI only, never in the engine.
    pub keyboard: Keyboard,
    /// The scopes and elements the last whole frame registered: what the keys walk.
    pub(crate) tree: FocusTree,
    /// The same, being registered by this frame as it draws.
    pub(crate) building: FocusTree,
    /// Held keys, for the repeat.
    pub(crate) repeat: Repeat,
    /// What the keys asked of particular widgets this frame (a click, an adjustment, a start of
    /// typing), each with whether it came from a held key's repeat; a widget takes its own.
    pub(crate) pending: Vec<(Effect, bool)>,
    /// The command palette.
    pub palette: Palette,
    /// Whether what is being done now came from the keyboard (a window opened from it shows
    /// the ring at once).
    pub(crate) by_keyboard: bool,
    /// The item being renamed in the scene tree (`F2`), and the name typed so far.
    pub renaming: Option<(Target, String)>,
    /// How fast a fly session moves, metres per second, before Shift and Alt.
    pub fly_speed: f64,
    /// The pane being flown with the right mouse button held (Unity's and Unreal's fly: W A S D
    /// move, the mouse looks, the wheel sets the speed), while it is held. UI state.
    pub mouse_fly: Option<usize>,
    /// The tool in hand: an id of `ui/tools.ron`. UI state, never journalled.
    pub tool: String,
    /// Whether the Move tool snaps to the step.
    pub snap: bool,
    /// The starter the Place tool adds (a name of `scenes/starters.ron`).
    pub place_starter: String,
    /// A drag of the Move tool's gizmo in hand.
    pub(crate) gizmo_drag: Option<GizmoDrag>,
    /// A transform session in hand (the keyboard's Move).
    pub session: Option<crate::tools::Session>,
    /// Whether the active pane is maximised: it fills the window but for the menu bar and the
    /// status line (`Ctrl+Space`). UI state.
    pub maximised: bool,
    /// Whether the window is full screen (`F11`), and what the window was last told.
    pub fullscreen: bool,
    fullscreen_sent: bool,
    /// The layout last chosen (`ui/layouts.ron`).
    pub layout_name: String,
    /// The inspector's folding sections a layout asked to open (`unfold` in `ui/layouts.ron`),
    /// opened as they are next drawn.
    pub(crate) unfold: Vec<String>,
    /// Whether the edit history window is open (`Ctrl+H`).
    pub history_open: bool,
    /// The new-scene wizard's choices, while it is open (`Ctrl+N`).
    pub wizard: Option<crate::dialogs::Wizard>,
    /// The Open window's path box, while it is open (`Ctrl+O`).
    pub open_window: Option<String>,
    /// The Save window's path box, while it is open.
    pub save_window: Option<String>,
    /// The file the scene came from or was last saved to, which `Ctrl+S` saves to again.
    pub file: Option<String>,
    /// Whether the scene was made in the window (the wizard) and has not been saved since: it
    /// is unsaved even though nothing has been edited since it was made.
    pub untitled: bool,
    /// A `focus` asked for before the first frame, applied once a frame has drawn its element.
    pending_focus: Option<String>,
    title_said: String,
}

/// A drag of the gizmo: the pane, the body, the handle, the gizmo as it was when the drag began
/// (so the arrow's scale does not change under the pointer), the anchor then, how far the pointer
/// has gone, points, and whether an edit has been made.
#[derive(Clone, Debug)]
pub(crate) struct GizmoDrag {
    pub pane: usize,
    pub body: usize,
    pub handle: crate::tools::Handle,
    pub gizmo: crate::tools::Gizmo,
    pub start: [f64; 3],
    pub moved: egui::Vec2,
    pub edited: bool,
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
            sheet_scroll: 0.0,
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
            render_notes: Vec::new(),
            keyboard: Keyboard::default(),
            tree: FocusTree::new(&crate::actions::table().big_scopes()),
            building: FocusTree::new(&crate::actions::table().big_scopes()),
            repeat: Repeat::default(),
            pending: Vec::new(),
            palette: Palette::default(),
            by_keyboard: false,
            renaming: None,
            fly_speed: crate::camera::FLY_SPEED,
            mouse_fly: None,
            tool: "select".to_owned(),
            snap: true,
            place_starter: "sphere".to_owned(),
            gizmo_drag: None,
            session: None,
            maximised: false,
            fullscreen: false,
            fullscreen_sent: false,
            layout_name: crate::layouts::layouts().layouts[0].id.clone(),
            unfold: Vec::new(),
            history_open: false,
            wizard: None,
            open_window: None,
            save_window: None,
            file: None,
            untitled: false,
            pending_focus: None,
            title_said: String::new(),
        }
    }

    /// The window's title: the scene's name, with `*` while it has unsaved edits.
    #[must_use]
    pub fn title(&self) -> String {
        format!("Cavendish · {}{}", self.engine.spec().name, if self.unsaved() { " *" } else { "" })
    }

    /// Whether the scene has something not saved: an edit since it was opened or saved, or a
    /// scene made here (the wizard) that has never been saved.
    #[must_use]
    pub fn unsaved(&self) -> bool {
        self.engine.unsaved() || self.untitled
    }

    /// What the tool in hand does.
    #[must_use]
    pub fn behaviour(&self) -> crate::tools::Behaviour {
        crate::tools::strip().tool(&self.tool).map_or(crate::tools::Behaviour::Select, |t| t.behaviour)
    }

    /// The step a move snaps to, metres, when snapping is on.
    #[must_use]
    pub fn snap_step(&self) -> Option<f64> {
        self.snap.then_some(crate::tools::strip().snap_step)
    }

    /// Moves body `body`'s anchor to `anchor` (metres): the `Set` of `bodies[i].motion.<anchor>`
    /// that the gizmo, the transform session and a drag all issue. `continues`: a step of a run
    /// after its first. True when it was applied.
    pub(crate) fn move_anchor(&mut self, body: usize, anchor: [f64; 3], continues: bool) -> bool {
        let Some(spec) = self.engine.spec().bodies.get(body) else { return false };
        let (_, field) = spec.motion.anchor();
        self.apply(Command::Set {
            path: format!("bodies[{body}].motion.{field}"),
            value: serde_json::json!(anchor),
            continues,
        })
    }

    /// Adds the Place tool's starter on the ground at `ground` (metres: a voxel shape resting on
    /// it, a point mass a metre above it), and selects it.
    ///
    /// # Errors
    /// Why it was refused.
    pub fn place_at(&mut self, ground: [f64; 3]) -> Result<String, String> {
        let name = self.place_starter.clone();
        let lift = match cav_sim::starters::find(&name).and_then(|s| s.body.as_ref()) {
            Some(body) if body.shape.is_none() && body.radius == 0.0 => 1.0,
            Some(body) => cav_sim::starters::lift(body),
            None => 0.0,
        };
        let [x, y, z] = ground;
        let outcome = self.run(&format!("add {name} at {x} {y} {}", z + lift))?;
        let spec = self.engine.spec();
        let select = match cav_sim::starters::find(&name).map(cav_sim::starters::Starter::list) {
            Some("array.towers") => format!("select tower {}", spec.array.towers.len()),
            _ => format!("select body {}", spec.bodies.len()),
        };
        let _ = self.run(&select);
        Ok(outcome)
    }

    /// A key for the transform session (`docs/slices/S18.md` K10): starts it on the selected
    /// body, moves it by the snap step (or 0.25 m) along an axis, types a distance, locks an axis,
    /// keeps it (Enter) or cancels it (Esc: the move is undone, as if it had not been made).
    /// Every move is the same `Set` of the anchor a gizmo drag issues, a run of them one edit.
    /// With no body selected (a tower, nothing) Enter starts no session: the keys go back to
    /// navigation at once (S18's review, minor 4).
    pub(crate) fn session_keys(&mut self) {
        use crate::keyboard::{Effect, SessionKey};
        const ID: &str = "viewport/selection";
        if self.take(|e| matches!(e, Effect::Interact(i) if i == ID)).is_some() {
            let body = match self.engine.selection() {
                Some(Target::Body(n)) => self.engine.spec().bodies.get(n - 1).map(|b| (n - 1, b)),
                _ => None,
            };
            match body {
                Some((n, body)) => {
                    self.session = Some(crate::tools::Session::new(n, &body.label, body.motion.anchor().0));
                }
                None => self.end_keyboard_session(),
            }
        }
        let step = self.snap_step().unwrap_or(crate::tools::strip().snap_step);
        let mut changed = false;
        while let Some((effect, _)) = self
            .take(|e| matches!(e, Effect::Adjust { id, .. } if id == ID) || matches!(e, Effect::Session(_)))
        {
            let view = self.engine.view(self.engine.active_pane()).clone();
            let Some(session) = self.session.as_mut() else { continue };
            match effect {
                // The arrows go the way they point on screen (S18's review, minor 11); an axis
                // lock takes them along the locked axis, → and ↑ its plus way.
                Effect::Adjust { axis, steps, .. } if session.lock.is_none() => {
                    let (world, sign) = crate::camera::arrow_axis(&view, axis);
                    session.nudge(world, sign * steps, step);
                }
                Effect::Adjust { axis, steps, .. } => session.nudge(axis, steps, step),
                Effect::Session(SessionKey::Char(c)) => session.type_char(c),
                Effect::Session(SessionKey::Lock(axis)) => session.toggle_lock(axis),
                Effect::Session(SessionKey::NextAxis) => session.next_axis(),
                _ => {}
            }
            changed = true;
        }
        if changed && let Some(session) = self.session.clone() {
            if session.holds(self.engine.spec()) {
                let applied = self.move_anchor(session.body, session.anchor(), session.edited);
                if let Some(s) = self.session.as_mut() {
                    s.edited |= applied;
                }
            } else {
                // The body is gone or another is in its place: the session ends, touching nothing.
                self.session = None;
                self.end_keyboard_session();
            }
        }
        if let Some((Effect::Release { keep, .. }, _)) =
            self.take(|e| matches!(e, Effect::Release { id, .. } if id == ID))
        {
            self.end_session(keep);
        }
    }

    /// Ends the transform session in hand, if there is one: `keep` leaves the body where it was
    /// moved (Enter); otherwise it goes back where it began, as the run's last step, so the run
    /// changed nothing and leaves no step to undo or redo (Esc: the owner's rule; the engine's
    /// history drops a run that ends where it began). Nothing is moved unless the body at the
    /// session's index is still the one it began on (S18's review, R1). True when the session
    /// had moved its body.
    pub(crate) fn end_session(&mut self, keep: bool) -> bool {
        let Some(session) = self.session.take() else { return false };
        self.end_keyboard_session();
        if !keep && session.edited && session.holds(self.engine.spec()) {
            self.move_anchor(session.body, session.start, true);
        }
        session.edited
    }

    /// The keyboard leaves a transform session it is in: back to navigation on the same element.
    fn end_keyboard_session(&mut self) {
        if self.keyboard.session == Some(crate::keyboard::Session::Transform) {
            self.keyboard.session = None;
            self.keyboard.mode = crate::keyboard::Mode::Navigation;
        }
    }

    /// Whether a fly session is on: the viewport's canvas holds the keys W A S D E Q.
    #[must_use]
    pub fn flying(&self) -> bool {
        self.keyboard.session == Some(crate::keyboard::Session::Fly)
    }

    /// Whether the camera is flown in either way: a fly session (Shift+F) or the right mouse
    /// button held over a scene pane.
    #[must_use]
    pub fn any_fly(&self) -> bool {
        self.flying() || self.mouse_fly.is_some()
    }

    /// The active pane's width over its height, from the last frame (1.6 before there is one).
    fn active_aspect(&self) -> f64 {
        self.layout
            .panes
            .get(self.engine.active_pane())
            .filter(|r| r.height() > 0.0)
            .map_or(1.6, |r| f64::from(r.width() / r.height()))
    }

    /// One frame of a fly session: the held keys push the camera ([`crate::camera::fly`]) at
    /// [`Self::fly_speed`], Shift ×10 and Alt ×0.1, for the frame's time.
    pub(crate) fn fly_frame(&mut self, ctx: &egui::Context) {
        if !self.any_fly() || self.keyboard.typing {
            return;
        }
        let (push, modifiers, dt) = ctx.input(|i| {
            let axis = |plus: egui::Key, minus: egui::Key| {
                f64::from(u8::from(i.key_down(plus))) - f64::from(u8::from(i.key_down(minus)))
            };
            let push = crate::camera::Push {
                forward: axis(egui::Key::W, egui::Key::S),
                right: axis(egui::Key::D, egui::Key::A),
                up: axis(egui::Key::E, egui::Key::Q),
            };
            (push, i.modifiers, f64::from(i.stable_dt).min(0.1))
        });
        if push == crate::camera::Push::default() {
            return;
        }
        let speed = self.fly_speed * Press::with(egui::Key::W, modifiers).multiplier();
        let pane = self.mouse_fly.unwrap_or_else(|| self.engine.active_pane());
        let command = crate::camera::fly(self.engine.view(pane), pane, push, speed, dt);
        self.apply(Command::View(command));
        ctx.request_repaint();
    }

    /// The height of the ground under the scene, metres: the floor's top, else the room's floor,
    /// else 0 (the towers' base level in the default array).
    #[must_use]
    pub fn ground(&self) -> f64 {
        let spec = self.engine.spec();
        spec.floor
            .as_ref()
            .map(|f| f.height)
            .or_else(|| spec.room.as_ref().map(|r| r.origin[2]))
            .unwrap_or(0.0)
    }

    /// Where a starter is put when no place is said: under where the active pane's camera
    /// looks, on the ground (a voxel shape resting on it, a point mass a metre above it), and
    /// not on a tower's axis (moved 2 m east until it is clear of every tower by a metre).
    /// Metres, the world's frame.
    #[must_use]
    pub fn place_for(&self, starter: &str) -> [f64; 3] {
        let view = self.engine.view(self.engine.active_pane());
        let body = cav_sim::starters::find(starter).and_then(|s| s.body.as_ref());
        let lift = match body {
            Some(body) if body.shape.is_none() && body.radius == 0.0 => 1.0,
            Some(body) => cav_sim::starters::lift(body),
            None => 0.0,
        };
        // A tower goes at the ground; a body clear of the towers' axes.
        let (mut x, y) = ((view.target[0] * 4.0).round() / 4.0, (view.target[1] * 4.0).round() / 4.0);
        if body.is_some() {
            let towers = &self.engine.spec().array.towers;
            let near = |x: f64| towers.iter().any(|t| (t.position[0] - x).hypot(t.position[1] - y) < 1.0);
            while near(x) {
                x += 2.0;
            }
        }
        [x, y, self.ground() + lift]
    }

    /// The scopes and elements of the last whole frame, as the keyboard walks them.
    #[must_use]
    pub const fn focus_tree(&self) -> &FocusTree {
        &self.tree
    }

    /// An action's console line with its placeholders filled from `selection`: `{selection}` is
    /// its path (`bodies[0]`, `array.towers[2]`, `room`), `{body}` and `{tower}` its number from 1.
    #[must_use]
    pub fn resolve(line: &str, selection: Option<Target>) -> String {
        let (path, number) = match selection {
            Some(Target::Body(n)) => (format!("bodies[{}]", n - 1), n),
            Some(Target::Tower(n)) => (format!("array.towers[{}]", n - 1), n),
            Some(Target::Room) => ("room".to_owned(), 0),
            Some(Target::Floor) => ("floor".to_owned(), 0),
            Some(Target::World) => ("world".to_owned(), 0),
            None => (String::new(), 0),
        };
        line.replace("{selection}", &path)
            .replace("{body}", &number.to_string())
            .replace("{tower}", &number.to_string())
    }

    /// Whether `action` can run now (its `needs`).
    #[must_use]
    pub fn can_run(&self, action: &Action) -> bool {
        let (can_undo, can_redo) = self.engine.history();
        let spec = self.engine.spec();
        match action.needs {
            Needs::Always => true,
            Needs::Undo => can_undo,
            Needs::Redo => can_redo,
            Needs::Selection => self.engine.selection().is_some(),
            Needs::Item => matches!(self.engine.selection(), Some(Target::Body(_) | Target::Tower(_))),
            Needs::Body => matches!(self.engine.selection(), Some(Target::Body(_))),
            Needs::Tower(n) => n >= 1 && n <= spec.array.towers.len(),
            Needs::Shapes => spec.bodies.iter().any(|b| b.shape.is_some()),
        }
    }

    /// Runs the action called `id` from the table: its line, its placeholders filled. An action
    /// that cannot run now does nothing, but undo and redo, which the engine refuses with the
    /// reason (in the log).
    ///
    /// # Errors
    /// Why it did not run.
    pub fn run_action(&mut self, id: &str) -> Result<String, String> {
        let action = crate::actions::table().action(id).ok_or_else(|| format!("no action `{id}`"))?;
        if !self.can_run(action) && !matches!(action.needs, Needs::Undo | Needs::Redo) {
            return Err(format!("`{}` cannot run now", action.label));
        }
        let mut line = Self::resolve(&action.line, self.engine.selection());
        if line.contains("{field}") {
            // The field the keyboard is on, in the inspector.
            let Some(path) = self.keyboard.focused().strip_prefix("field/").map(str::to_owned) else {
                return Err("no field has the keyboard's focus".into());
            };
            line = line.replace("{field}", &path);
        }
        self.run(&line)
    }

    /// Adds the starter `name` where [`Self::place_for`] puts it, and selects it: the Add menu,
    /// the empty scene's buttons and the palette's Add entries.
    ///
    /// # Errors
    /// Why it was refused.
    pub fn add_starter(&mut self, name: &str) -> Result<String, String> {
        let [x, y, z] = self.place_for(name);
        let outcome = self.run(&format!("add {name} at {x} {y} {z}"))?;
        // What was added is selected, so that the next thing done (a move, a field) is to it.
        let spec = self.engine.spec();
        let select = match cav_sim::starters::find(name).map(cav_sim::starters::Starter::list) {
            Some("array.towers") => format!("select tower {}", spec.array.towers.len()),
            _ => format!("select body {}", spec.bodies.len()),
        };
        let _ = self.run(&select);
        Ok(outcome)
    }

    /// The paths of the fields the inspector shows for the selection, for the palette.
    #[must_use]
    pub fn inspected_fields(&self) -> Vec<String> {
        let prefix = Self::resolve("{selection}", self.engine.selection());
        let prefix = if prefix.is_empty() { "duration" } else { prefix.as_str() };
        cav_sim::paths::leaves(self.engine.spec(), prefix, crate::editors::MAX_FIELDS)
            .map(|entries| entries.into_iter().map(|e| e.path).collect())
            .unwrap_or_default()
    }

    /// Registers a node of the keyboard's tree for this frame, drawn at `rect`.
    pub(crate) fn register(&mut self, parent: &str, id: &str, name: &str, kind: Kind, rect: Rect) {
        self.building.add(parent, id, name, kind);
        self.building.place(id, rect);
    }

    /// Whether the keys asked for a click of `id` this frame (taken: asked once).
    pub(crate) fn take_click(&mut self, id: &str) -> bool {
        self.take(|e| matches!(e, Effect::Click(i) if i == id)).is_some()
    }

    /// Whether the keys asked to start typing in `id` this frame.
    pub(crate) fn take_type(&mut self, id: &str) -> bool {
        self.take(|e| matches!(e, Effect::Type(i) if i == id)).is_some()
    }

    /// Every adjustment the keys asked of `id` this frame: `(axis, steps, ctrl, repeat)`.
    pub(crate) fn take_adjust(&mut self, id: &str) -> Vec<(usize, f64, bool, bool)> {
        let mut out = Vec::new();
        while let Some((effect, repeat)) = self.take(|e| matches!(e, Effect::Adjust { id: i, .. } if i == id))
        {
            if let Effect::Adjust { axis, steps, ctrl, .. } = effect {
                out.push((axis, steps, ctrl, repeat));
            }
        }
        out
    }

    /// The choice the keys picked in the drop-down `id` this frame, from 0.
    pub(crate) fn take_pick(&mut self, id: &str) -> Option<usize> {
        match self.take(|e| matches!(e, Effect::Pick { id: i, .. } if i == id)) {
            Some((Effect::Pick { index, .. }, _)) => Some(index),
            _ => None,
        }
    }

    /// Whether interaction with `id` ended this frame, and whether what it did is kept.
    pub(crate) fn take_release(&mut self, id: &str) -> Option<bool> {
        match self.take(|e| matches!(e, Effect::Release { id: i, .. } if i == id)) {
            Some((Effect::Release { keep, .. }, _)) => Some(keep),
            _ => None,
        }
    }

    /// The first effect this frame that `wanted` accepts, taken, with whether it came from a
    /// held key's repeat.
    pub(crate) fn take_effect(&mut self, wanted: impl Fn(&Effect) -> bool) -> Option<(Effect, bool)> {
        self.take(wanted)
    }

    fn take(&mut self, wanted: impl Fn(&Effect) -> bool) -> Option<(Effect, bool)> {
        let at = self.pending.iter().position(|(e, _)| wanted(e))?;
        Some(self.pending.remove(at))
    }

    /// Closes the palette; the focus goes back to where it was.
    pub(crate) fn palette_close(&mut self) {
        self.palette.open = false;
        if self.keyboard.focus.first().map(String::as_str) == Some("palette") {
            self.keyboard.close_window();
        }
    }

    /// Runs what the palette chose: an action, a line, or a move of the focus to a field.
    pub(crate) fn palette_run(&mut self, entry: &crate::palette::Entry) {
        if let Some(action) = &entry.action {
            let _ = self.run_action(action);
        } else if let Some(field) = &entry.field {
            let id = format!("field/{field}");
            self.keyboard.focus_on(&self.tree, &id);
            self.keyboard.nav_on = true;
        } else if let Some(line) = &entry.line {
            let takes_words = cav_engine::console::COMMANDS
                .iter()
                .find(|c| c.name == line)
                .is_some_and(|c| c.usage.trim() != c.name);
            if let Some(starter) = line.strip_prefix("add ").filter(|s| cav_sim::starters::find(s).is_some())
            {
                // A starter goes where the camera looks, and is selected.
                let _ = self.add_starter(starter);
            } else if takes_words {
                // A command that needs words: the console opens with its name typed.
                self.console_open = true;
                self.focus_console = true;
                self.console_line = format!("{line} ");
            } else {
                let _ = self.run(line);
            }
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
        // A scene saved, or another opened, is no longer the wizard's unsaved one.
        if outcome.is_ok() && matches!(line.split_whitespace().next(), Some("save" | "load")) {
            self.untitled = false;
        }
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
        // `layout <id>` runs a layout's lines (view commands): the figure arranged for a task.
        if name == "layout" {
            let all = crate::layouts::layouts();
            let ids: Vec<&str> = all.layouts.iter().map(|l| l.id.as_str()).collect();
            let Some(layout) = words.next().and_then(|id| all.layout(id)).filter(|_| words.next().is_none())
            else {
                let why = format!("`{line}`: layout <{}>", ids.join("|"));
                self.engine.note(Level::Error, Subsystem::App, why.clone());
                self.console_open = true;
                return Err(why);
            };
            self.maximised = false;
            self.layout_name.clone_from(&layout.id);
            self.unfold.clone_from(&layout.unfold);
            for step in &layout.lines {
                self.run(step)?;
            }
            return Ok(line.to_owned());
        }
        // `focus <id>` puts the keyboard's focus on a scope or an element of the last frame's tree,
        // with the ring showing: for scripts, the demo and a person who knows the id.
        if name == "focus" {
            let outcome = match (words.next(), words.next()) {
                (Some(id), None) if self.tree.node(id).is_some() => {
                    let tree = self.tree.clone();
                    self.keyboard.focus_on(&tree, id);
                    self.keyboard.nav_on = true;
                    Ok(line.to_owned())
                }
                // Before the first frame nothing is registered (a script run before the window
                // draws): the focus goes there once the frame has drawn it.
                (Some(id), None) if self.layout.screen == Rect::NOTHING => {
                    self.pending_focus = Some(id.to_owned());
                    Ok(line.to_owned())
                }
                (Some(id), None) => {
                    Err(format!("`{id}` is not on screen: `focus` takes a scope or an element's id"))
                }
                _ => Err(format!("`{line}`: focus <id>")),
            };
            if let Err(why) = &outcome {
                self.engine.note(Level::Error, Subsystem::App, why.clone());
            }
            return outcome;
        }
        // `reset <path>` puts a field back as the scene was opened or saved: a `set` (an edit).
        if name == "reset" {
            let outcome = match (words.next(), words.next()) {
                (Some(path), None) => {
                    let saved = cav_sim::paths::get(&self.engine.baseline().to_value(), path).cloned();
                    match saved {
                        Ok(value) => {
                            self.apply(Command::Set { path: path.to_owned(), value, continues: false });
                            Ok(line.to_owned())
                        }
                        Err(why) => Err(why),
                    }
                }
                _ => Err(format!("`{line}`: reset <path>")),
            };
            if let Err(why) = &outcome {
                self.engine.note(Level::Error, Subsystem::App, why.clone());
                self.console_open = true;
            }
            return outcome;
        }
        // `tool <id>` takes a tool of the strip in hand; nothing is journalled.
        if name == "tool" {
            let ids: Vec<&str> = crate::tools::strip().tools.iter().map(|t| t.id.as_str()).collect();
            return match (words.next(), words.next()) {
                (Some(id), None) if ids.contains(&id) => {
                    self.tool = id.to_owned();
                    Ok(line.to_owned())
                }
                _ => {
                    let why = format!("`{line}`: tool <{}>", ids.join("|"));
                    self.engine.note(Level::Error, Subsystem::App, why.clone());
                    self.console_open = true;
                    Err(why)
                }
            };
        }
        // `palette <menu>` opens the palette filtered to a menu (`palette add`: the starters).
        if name == "palette"
            && let Some(menu) = words.next()
        {
            let titles = crate::menus::titles();
            let Some(title) = titles.iter().find(|t| t.eq_ignore_ascii_case(menu)) else {
                let why = format!("`{menu}` is not a menu: {}", titles.join(", "));
                self.engine.note(Level::Error, Subsystem::App, why.clone());
                self.console_open = true;
                return Err(why);
            };
            self.palette =
                Palette { open: true, focus_query: true, menu: Some(title.clone()), ..Palette::default() };
            self.window_opened("palette", "palette/query", true);
            return Ok(line.to_owned());
        }
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
            "keys" => {
                self.keys_open = !self.keys_open;
                self.window_opened("sheet", "sheet/list", self.keys_open);
            }
            "about" => {
                self.about_open = !self.about_open;
                self.window_opened("about", "about/text", self.about_open);
            }
            "palette" => {
                self.palette = Palette { open: true, focus_query: true, ..Palette::default() };
                self.window_opened("palette", "palette/query", true);
            }
            "history" => {
                self.history_open = !self.history_open;
                self.window_opened("history", "history/start", self.history_open);
            }
            "wizard" => {
                self.wizard = Some(crate::dialogs::Wizard::default());
                self.window_opened("wizard", "wizard/array", true);
            }
            "open" => {
                self.open_window = Some(String::new());
                self.window_opened("open", "open/path", true);
            }
            "save-as" => {
                self.save_window =
                    Some(self.file.clone().unwrap_or_else(|| format!("{}.ron", self.engine.spec().name)));
                self.window_opened("save", "save/path", true);
            }
            "save-scene" => match self.file.clone() {
                Some(file) => {
                    let quoted =
                        if file.contains(char::is_whitespace) { format!("\"{file}\"") } else { file };
                    return self.run(&format!("save {quoted}"));
                }
                None => {
                    self.save_window = Some(format!("{}.ron", self.engine.spec().name));
                    self.window_opened("save", "save/path", true);
                }
            },
            "snap" => self.snap = !self.snap,
            "maximise" => self.maximised = !self.maximised,
            "fullscreen" => self.fullscreen = !self.fullscreen,
            "frame" => {
                // The selection (or, with nothing selected, the whole scene) fills the active
                // pane from the present direction: one camera command.
                let pane = self.engine.active_pane();
                let (centre, radius) =
                    crate::camera::bounds_of(self.engine.spec(), self.engine.selection(), self.engine.time());
                let command =
                    crate::camera::frame(self.engine.view(pane), pane, centre, radius, self.active_aspect());
                self.apply(Command::View(command));
            }
            "fly" => {
                if self.flying() {
                    self.keyboard.session = None;
                    self.keyboard.mode = crate::keyboard::Mode::Navigation;
                } else {
                    // The canvas takes the keys, in interaction mode: W A S D E Q fly, the
                    // arrows look, Enter or Esc stop (the camera kept where it was flown).
                    self.keyboard.focus = vec!["viewport".into(), "viewport/canvas".into()];
                    self.keyboard.mode = crate::keyboard::Mode::Interaction;
                    self.keyboard.session = Some(crate::keyboard::Session::Fly);
                    self.keyboard.nav_on |= self.by_keyboard;
                }
            }
            "rename" => match self.engine.selection() {
                Some(target @ (Target::Body(n) | Target::Tower(n))) => {
                    let spec = self.engine.spec();
                    let label = match target {
                        Target::Body(_) => spec.bodies[n - 1].label.clone(),
                        _ => spec.array.towers[n - 1].label.clone(),
                    };
                    self.renaming = Some((target, label));
                }
                _ => {
                    let why = "select a body or a tower to rename".to_owned();
                    self.engine.note(Level::Error, Subsystem::App, why.clone());
                    return Err(why);
                }
            },
            "clear" => self.engine.clear_log(),
            "quit" => self.quit = true,
            _ => unreachable!("is_app_command checked the name"),
        }
        Ok(name.to_owned())
    }

    /// A window has opened (`open`) or closed: on opening it takes the keyboard's focus at its
    /// first section, with the ring if it was opened from the keyboard; on closing the focus
    /// goes back to where it was, if it was in the window.
    fn window_opened(&mut self, window: &str, first: &str, open: bool) {
        if open {
            self.keyboard.open_window(window, first, self.by_keyboard);
        } else if self.keyboard.focus.first().map(String::as_str) == Some(window) {
            self.keyboard.close_window();
        }
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
                    truncated: None,
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

    /// Turns this frame's keys into effects through the keyboard model's router
    /// ([`Keyboard::route`]): the fixed priority of `docs/design/keyboard.md` §11 over the
    /// scopes and elements the last frame registered. A key the router uses is taken out of
    /// egui's input, so no widget acts on it twice; a key it leaves (what a text field types)
    /// stays. A mouse press turns keyboard navigation off.
    pub(crate) fn keys(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        // Typing is a text field with egui's focus. (egui lets go of a field on Esc before the
        // frame starts; that Esc is still the field's, so the router hears it as typing.)
        let typing = ctx.text_edit_focused();
        let esc_from_field = self.keyboard.typing
            && ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter));
        if self.keyboard.typing && !typing && !esc_from_field {
            // A click elsewhere ended the typing.
            self.keyboard.mode = crate::keyboard::Mode::Navigation;
        }
        if typing {
            self.keyboard.mode = crate::keyboard::Mode::Interaction;
        }
        self.keyboard.typing = typing || esc_from_field;
        // egui would move its own focus between widgets on Tab and the arrows; the keyboard model
        // owns those keys (`docs/design/keyboard.md`), so egui's move is cancelled every frame.
        ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
        let (presses, released, clicked) = ctx.input(|i| {
            let mut presses = Vec::new();
            let mut released = Vec::new();
            let mut clicked = None;
            for event in &i.events {
                match event {
                    Event::Key { key, pressed: true, repeat: false, modifiers, .. } => {
                        presses.push(Press::with(*key, *modifiers));
                    }
                    Event::Key { key, pressed: false, .. } => released.push(*key),
                    Event::PointerButton { pressed: true, pos, .. } => clicked = Some(*pos),
                    _ => {}
                }
            }
            (presses, released, clicked)
        });
        // The right button let go: the mouse's fly ends.
        if self.mouse_fly.is_some() && !ctx.input(|i| i.pointer.secondary_down()) {
            self.mouse_fly = None;
        }
        if let Some(pos) = clicked {
            self.keyboard.pointer();
            self.repeat.clear();
            // A click on a control in a window moves the keyboard's focus onto it, in navigation
            // mode, the ring hidden (the spec's §8); anywhere else (a panel, the canvas) the focus
            // stays where it was.
            if let Some(id) = self.tree.window_node_at(pos).map(str::to_owned) {
                let tree = self.tree.clone();
                self.keyboard.focus_on(&tree, &id);
            }
        }
        for key in released {
            self.repeat.release(key);
        }
        let mut routed: Vec<(Press, bool)> = Vec::new();
        for press in presses {
            self.repeat.press(press, now);
            routed.push((press, false));
        }
        routed.extend(self.repeat.due(now).into_iter().map(|press| (press, true)));
        // While the right button flies the camera, letters are the flight's (W A S D E Q, read
        // as held keys by `fly_frame`) and no letter is a shortcut: W is not the Move tool, A not
        // Place (S18's review, U3).
        if self.mouse_fly.is_some() {
            let letter =
                |key: egui::Key| key.name().len() == 1 && key.name().chars().all(|c| c.is_ascii_alphabetic());
            routed.retain(|(press, _)| !(letter(press.key) && !press.modifiers.command));
        }
        if self.repeat.any() {
            ctx.request_repaint();
        }
        let shortcuts = crate::actions::table().shortcuts();
        let mut used = Vec::new();
        for (press, repeat) in routed {
            let (effects, took) = self.keyboard.route(&self.tree, &shortcuts, press);
            if took {
                used.push(press);
            }
            if press.key == egui::Key::Backtick {
                // The backtick that opened the console should not be typed into it.
                self.console_line.retain(|c| c != '`');
            }
            for effect in effects {
                self.effect(ctx, effect, repeat);
            }
        }
        if !used.is_empty() {
            // The keys the router used are not seen again by a widget this frame.
            ctx.input_mut(|i| {
                i.events.retain(|event| {
                    !matches!(event, Event::Key { key, pressed: true, .. } if used.iter().any(|p| p.key == *key))
                });
            });
        }
    }

    /// Carries out one effect of a key: an action at once; the frame's own ladder for Esc; what
    /// a particular widget must do is left for it to take as it is drawn.
    fn effect(&mut self, ctx: &egui::Context, effect: Effect, repeat: bool) {
        match effect {
            Effect::Action(id) => {
                // A global key in a transform session (the only keys that reach an action from
                // one) first ends it, cancelled, so that no other action runs on a body half
                // moved (S18's review, R1). Undo in a session that has moved its body *is* that
                // cancel, and takes back nothing more.
                if self.session.is_some() {
                    self.session_keys();
                    let moved = self.end_session(false);
                    if moved && id == "undo" {
                        return;
                    }
                }
                self.by_keyboard = true;
                // A refusal (nothing to undo, say) is in the log and opens the console.
                let _ = self.run_action(&id);
                self.by_keyboard = false;
            }
            Effect::Escape => {
                // Esc at a big scope's top level: a window, then the console, then the hidden
                // GUI, then the selection.
                if self.keys_open || self.about_open || self.history_open {
                    self.keys_open = false;
                    self.about_open = false;
                    self.history_open = false;
                } else if self.console_open {
                    self.console_open = false;
                } else if self.gui_hidden {
                    self.gui_hidden = false;
                } else if self.engine.selection().is_some() {
                    let _ = self.run("select none");
                }
            }
            Effect::Close(window) => match window.as_str() {
                "sheet" => self.keys_open = false,
                "about" => self.about_open = false,
                "palette" => self.palette.open = false,
                "console" => self.console_open = false,
                "history" => self.history_open = false,
                "wizard" => self.wizard = None,
                "open" => self.open_window = None,
                "save" => self.save_window = None,
                _ => {}
            },
            Effect::StopTyping => {
                // Tab left a field: egui lets it go, so the field commits what was typed.
                if let Some(id) = ctx.memory(egui::Memory::focused) {
                    ctx.memory_mut(|m| m.surrender_focus(id));
                }
            }
            Effect::Reveal(scope) => {
                // Tab onto a scope that is not on screen shows it first: nothing happens unseen
                // (`docs/research/editor-ux.md` §3.11). The top bar, the view and the status line
                // are shown even maximised.
                if self.maximised && !matches!(scope.as_str(), "top-bar" | "viewport" | "footer") {
                    self.maximised = false;
                }
                if self.gui_hidden && scope != "viewport" {
                    self.gui_hidden = false;
                }
            }
            Effect::Interact(_) | Effect::Session(_) => self.pending.push((effect, repeat)),
            Effect::Click(_)
            | Effect::Adjust { .. }
            | Effect::Zoom { .. }
            | Effect::Type(_)
            | Effect::Pick { .. } => {
                self.pending.push((effect, repeat));
            }
            Effect::Release { .. } => self.pending.push((effect, repeat)),
        }
    }

    /// The ring and the breadcrumb, once the frame has drawn: a 2-point outline round the focused
    /// node (a border round a whole big scope), orange in navigation mode and blue in
    /// interaction mode, only while keyboard navigation is on (the spec's §10). Never a fill, so
    /// it never covers the figure.
    pub(crate) fn paint_ring(&self, ctx: &egui::Context) {
        if !self.keyboard.nav_on {
            return;
        }
        let colour = match self.keyboard.mode {
            crate::keyboard::Mode::Navigation => RING_NAVIGATION,
            crate::keyboard::Mode::Interaction => RING_INTERACTION,
        };
        // The innermost node of the path that was drawn.
        let rect =
            self.keyboard.focus.iter().rev().find_map(|id| self.building.node(id).and_then(|n| n.rect));
        if let Some(rect) = rect {
            let painter =
                ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("ring")));
            painter.rect_stroke(
                rect.expand(1.0),
                2.0,
                egui::Stroke::new(2.0, colour),
                egui::StrokeKind::Outside,
            );
        }
    }

    /// The breadcrumb of the focus path (`Inspector › Body › mass`), while navigation is on.
    #[must_use]
    pub fn breadcrumb(&self) -> Option<String> {
        // The last whole frame's names: this frame's tree is still being registered.
        self.keyboard.nav_on.then(|| self.tree.breadcrumb(&self.keyboard.focus)).filter(|t| !t.is_empty())
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
        // The keys walk what the last frame registered and laid out (F frames the active pane as
        // it was last drawn: S18's review, U5, found it framing for no pane at all); this frame
        // registers and lays out anew as it draws.
        self.building = FocusTree::new(&crate::actions::table().big_scopes());
        self.keys(&ctx);
        self.layout = Layout { screen: ui.max_rect(), ..Layout::default() };
        // The window's title says the scene and whether it has unsaved edits.
        let title = self.title();
        if title != self.title_said {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title_said = title;
        }
        if self.fullscreen != self.fullscreen_sent {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
            self.fullscreen_sent = self.fullscreen;
        }
        self.session_keys();
        self.fly_frame(&ctx);
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
        crate::palette::draw(self, &ctx);
        crate::dialogs::draw(self, &ctx);
        self.paint_ring(&ctx);
        // What no widget took this frame (a click of something no longer drawn) is dropped; so is
        // a layout's unfolding, once a frame has drawn what it opens.
        self.pending.clear();
        self.unfold.clear();
        self.tree = std::mem::take(&mut self.building);
        if let Some(id) = self.pending_focus.take() {
            if self.tree.node(&id).is_some() {
                let tree = self.tree.clone();
                self.keyboard.focus_on(&tree, &id);
                self.keyboard.nav_on = true;
            } else {
                self.engine.note(
                    Level::Warn,
                    Subsystem::App,
                    format!("`focus {id}`: nothing of that id was drawn"),
                );
            }
        }
        // The world still streaming, or its meshes still being made: another frame soon.
        if self.engine.streaming().pending() > 0 || surface.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
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

    /// The wheel over a pane: zoom toward the point under the pointer (`pointer`, points from the
    /// pane's corner, in a pane of `size` points), which stays where it is on screen; in a fly
    /// session the wheel sets the flying speed instead.
    pub(crate) fn zoom_by(&mut self, pane: usize, scroll: f32, pointer: [f64; 2], size: [f64; 2]) {
        let factor = f64::from(-scroll * 0.002).exp();
        if self.any_fly() {
            self.fly_speed = (self.fly_speed / factor).clamp(0.01, 1000.0);
            return;
        }
        let view = self.engine.view(pane).clone();
        let factor = factor.clamp(0.05 / view.distance, 1.0e5 / view.distance);
        self.apply(Command::View(crate::camera::zoom_about(&view, pane, size, pointer, factor)));
    }
}
