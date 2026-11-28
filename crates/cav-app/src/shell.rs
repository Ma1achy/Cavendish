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
    ("play", "start or stop playback (Space)"),
    ("pause", "stop playback"),
    (
        "step",
        "move the playhead by shots of the engine's own (the selected tower's, else the first tower's), each to a readout: `step 1`, `step -1`, `step 3`; by mean cadence cycles with `cycle`: `step -1 cycle`. Playback stops",
    ),
    (
        "end",
        "what playing does at the end of the timeline: `end loop` (back to the start, as it begins), `end stop` (the playhead stops at the end), `end run-on` (the scene's duration grows by its length when playing began: one edit a run, up to the longest timeline); `end menu` opens or closes the transport's list",
    ),
    (
        "speed",
        "how fast the playhead moves while playing, as a multiple of the clock's rate, 0.1 to 10: `speed 2`, `speed 0.25`; `speed menu` opens or closes the transport's list of speeds",
    ),
    ("keys", "the keyboard reference"),
    (
        "palette",
        "the command palette: search every action, starter, scene, command and field (Ctrl+K); `palette add` lists what can be added, `palette <menu>` one menu, `palette <words>` opens it with them typed",
    ),
    ("context", "open the right-click menu of the selection's row in the scene tree"),
    (
        "rename",
        "type a new name for the selected body or tower, or the focused folder, in the scene tree (F2)",
    ),
    (
        "new-folder",
        "make a folder in the scene tree (the engine's `folder new`), beside the selection or inside the folder in focus, and type its name",
    ),
    (
        "tree-fold",
        "fold or unfold a row of the scene tree, for this scene: `tree-fold <array|bodies|surroundings|folder>` (quote a folder's name with spaces)",
    ),
    ("frame", "fit the selection (or the whole scene) into the active pane (F)"),
    (
        "tool",
        "take a tool in hand: `tool select` (Q), `tool move` (W), `tool place` (A), `tool probe` (I); with one of its variants: `tool place cube`",
    ),
    (
        "tool-palette",
        "where the tool palette stands in its view, kept for the layout: `tool-palette <top-left|top|top-right|left|right|bottom-left|bottom|bottom-right>`",
    ),
    ("flyout", "open a tool's flyout of variants beside the palette: `flyout place`; `flyout off` closes it"),
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
    (
        "import-mesh",
        "the Import mesh window: an OBJ or STL file (dropped on the window, or a path natively), its size, material and pitch; `import-mesh <file>` opens it with the file named",
    ),
    ("save-scene", "save the scene to the file it came from, or ask for one (Ctrl+S)"),
    (
        "layout",
        "arrange the window for a task: `layout scene` (Ctrl+1), `layout instrument` (Ctrl+2), `layout analysis` (Ctrl+3); or the figure as a mosaic written out: `layout \"AAB;CDB\"` (as `mosaic`); `layout save [name]` keeps the present arrangement as a new layout for this session (the top bar's +), `layout forget <id>` drops one",
    ),
    (
        "viewport",
        "act on a viewport (the active one, or number N): `viewport close [N]`; `viewport swap [N]` exchanges its place with the next viewport's; `viewport pop-out [N]` floats it over the window, or docks it if it floats; `viewport dock [N]` puts it back in the mosaic (with no N, every floating one); `viewport type|camera|overlays [N]` opens that menu of its header",
    ),
    (
        "maximise",
        "the active pane fills the window but for the menu bar and the status line; again to restore (Ctrl+Space)",
    ),
    ("fullscreen", "the window fills the screen; again to restore (F11)"),
    ("snap", "snap the Move tool's moves to the step, or not (S)"),
    (
        "fly",
        "fly the active viewport's camera as a first-person game does: the pointer is captured, the mouse looks, W A S D move, Space up, Shift down, Ctrl or a double tap of W sprints, the wheel sets the speed; matter stops it unless `noclip`; again, Esc or Enter to stop (Shift+F; or hold the right button over the view)",
    ),
    (
        "walk",
        "walk the active viewport's camera through the scene: a player-sized box with gravity that the world, voxel bodies, the floor and the room stop; Space jumps, Shift sneaks, a double tap of Space flies; again, Esc or Enter to stop (Shift+G)",
    ),
    ("noclip", "whether the first-person camera passes through matter (off at first; on, it flies) (N)"),
    (
        "eye",
        "put the active viewport's eye at a point, looking along a heading (degrees anticlockwise from east) and a pitch (degrees up): `eye <x> <y> <z> [<heading> <pitch>]`; for scripts and pictures",
    ),
    (
        "walk-for",
        "hold keys down for a time in the `walk` or `fly` session under way, at 60 steps a second: `walk-for <seconds> [w a s d space shift ctrl]`; for scripts and pictures",
    ),
    (
        "camera-set",
        "the first-person camera's settings, for this session: `camera-set` lists them; `camera-set <sensitivity|fly-speed|sprint|easing|double-tap|speed-shown> <value>` sets one (defaults in `ui/camera.ron`)",
    ),
    ("tab", "the right-hand panel's tab: `tab inspector`, `tab readings` or `tab probe`"),
    (
        "probe-at",
        "the Probe tool's click at a point of the active pane, as fractions from its top left (`probe-at 0.5 0.5`): for scripts and pictures",
    ),
    (
        "pointer",
        "for a picture with no window: rest the pointer at a point of the active pane, as fractions from its top left (`pointer 0.5 0.7`)",
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
    /// `console`, `footer`. (The tool's options row is no panel: it is inside a viewport, in
    /// [`Self::options`].)
    pub panels: Vec<(&'static str, Rect)>,
    /// The tool's options row, while it is shown: one row under the header of the viewport the
    /// tool palette is in (`docs/design/workspace.md` §3). `None` while the tool in hand has no
    /// settings and no transform session is under way: the canvas has the room.
    pub options: Option<Rect>,
    /// The viewport (pane, from 0) the tool palette was drawn in; `None` when it was not drawn
    /// (no viewport shows the scene, the panels are hidden, or the canvas has no room for it).
    /// Its rectangle is the button `tool palette`.
    pub tool_host: Option<usize>,
    /// The area the panes share.
    pub figure: Rect,
    /// Each pane (viewport), in order: where it is drawn, its header and all. A floating one's is
    /// where it floats. Not for pointer arithmetic: the picture is in [`Self::canvases`].
    pub panes: Vec<Rect>,
    /// Each viewport's canvas, in order: its rectangle less its header, where the scene's picture
    /// or the plot is drawn. A pointer's place in a view is measured from this rectangle's
    /// corner. The same as its pane's while the headers are hidden (F3); nothing when the
    /// viewport is not drawn (another is maximised).
    pub canvases: Vec<Rect>,
    /// Each viewport's header that was drawn: the pane (from 0) and its row.
    pub headers: Vec<(usize, Rect)>,
    /// Every control the viewports' headers and navigation gizmos drew, in the order drawn.
    pub controls: Vec<HeaderControl>,
    /// Each header's warning dot that was lit: the pane (from 0), the dot, and the texts its
    /// tooltip holds (a stale or unsettled signal for a plot, what the renderer thinned or left
    /// out for a scene).
    pub warnings: Vec<(usize, Rect, Vec<String>)>,
    /// What the status bar says, by part, in its order: `selection`, `instrument`, `signal`
    /// (the engine's own words for the signal's state: [`cav_engine::estimate_text`] of it when
    /// it is the scene's), `forward`, `warnings`, `errors`, `frame`. Each whole, as its tooltip
    /// has it; what is painted (cut short when long) is in [`Self::chrome`] under `status`.
    pub status_bar: Vec<(&'static str, String)>,
    /// Each pane's place in the mosaic, in order: where it is drawn unless it floats (then the
    /// place is empty but for its dock button).
    pub slots: Vec<Rect>,
    /// The viewports that float over the window: the pane (from 0) and its frame, bar and all.
    pub floating: Vec<(usize, Rect)>,
    /// The splitters between the viewports: the boundary each moves and its handle.
    pub splitters: Vec<(cav_engine::Boundary, Rect)>,
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
    /// What the status bar says about the forward stage (its `forward` part).
    pub status: String,
    /// Each row the outliner laid out: what a click on it selects, and where it is.
    pub outline: Vec<(cav_engine::Target, Rect)>,
    /// The words each of those rows showed, in the same order: its label, and every text the row
    /// painted (S19's review, I6: recorded where it is painted, not from what was passed in).
    pub outline_words: Vec<(cav_engine::Target, String, String)>,
    /// Every text a tree row painted, by the row's keyboard id, in the order painted.
    pub row_texts: Vec<(String, String)>,
    /// The values the Readings tab's rows were drawn from: the tower (from 0) and its ΔΦ, δφ₁
    /// and δφ₂, radians, as the engine gave them.
    pub reading_values: Vec<(usize, [f64; 3])>,
    /// The numbers on the timeline's ruler, each with the rectangle it took, and the rectangle
    /// the timeline is drawn inside.
    pub ruler: (Vec<(String, Rect)>, Rect),
    /// Each scene pane's legend of the field that colours its voxel bodies: the pane (from 0)
    /// and where the legend was painted.
    pub legends: Vec<(usize, Rect)>,
    /// The scene tree's groups as drawn: each group's scope id and whether it is open.
    pub tree_groups: Vec<(String, bool)>,
    /// The scene tree's folders as drawn, in the order drawn.
    pub tree_folders: Vec<crate::outliner::FolderRow>,
    /// The row a dragged row of the scene tree is over, if it is over one that takes drops (a
    /// folder's label, or a group's name), and whether a drop there would be taken.
    pub tree_drop: Option<(String, bool)>,
    /// Each tower's status dot as drawn in the tree: the tower (from 0) and what it says.
    pub dots: Vec<(usize, crate::outliner::Dot)>,
    /// The Readings tab's rows as drawn: the tower (from 0) and its ΔΦ as written, with the
    /// column's unit after it (the unit is drawn once, in the column's heading).
    pub readings: Vec<(usize, String)>,
    /// The reading the Probe tab drew its rows and its towers' table from, as the engine gave
    /// it; `None` while the tab is not shown or nothing is probed.
    pub probe: Option<cav_sim::probe::Probe>,
    /// The Probe tab's rows as written: each name and its value.
    pub probe_rows: Vec<(String, String)>,
    /// Every text painted in the chrome that the design keeps free of sentences, by where it is:
    /// `options` (the tool's options row), `timeline` (the timeline's head), `tree` (what the
    /// scene tree paints besides its rows' names, which are in `row_texts`), `header` (the
    /// viewports' headers) and `status` (the status bar's parts, as painted); and what became a
    /// tooltip there, as `options tooltip`, `timeline tooltip` and `status tooltip`.
    pub chrome: Vec<(&'static str, String)>,
    /// What each material drop-down of the inspector shows closed: the path it edits and its
    /// words ("Uniform", "steel").
    pub choices: Vec<(String, String)>,
    /// The transport's controls as drawn, in the order drawn ([`crate::transport::CONTROLS`]).
    pub transport: Vec<crate::transport::Drawn>,
    /// What the top bar painted, by part, left to right: `menu` (each menu's title), `layout`
    /// (each tab), `search`, `scene` (the scene's name, with its unsaved mark) and, while
    /// keyboard navigation is on, `breadcrumb`; each with where it is.
    pub top_bar: Vec<(&'static str, String, Rect)>,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            screen: Rect::NOTHING,
            panels: Vec::new(),
            options: None,
            tool_host: None,
            figure: Rect::NOTHING,
            panes: Vec::new(),
            canvases: Vec::new(),
            headers: Vec::new(),
            controls: Vec::new(),
            warnings: Vec::new(),
            status_bar: Vec::new(),
            slots: Vec::new(),
            floating: Vec::new(),
            splitters: Vec::new(),
            buttons: Vec::new(),
            plots: Vec::new(),
            editors: Vec::new(),
            lanes: Vec::new(),
            shots: Vec::new(),
            timeline_caption: String::new(),
            status: String::new(),
            outline: Vec::new(),
            outline_words: Vec::new(),
            row_texts: Vec::new(),
            reading_values: Vec::new(),
            ruler: (Vec::new(), Rect::NOTHING),
            legends: Vec::new(),
            tree_groups: Vec::new(),
            tree_folders: Vec::new(),
            tree_drop: None,
            dots: Vec::new(),
            readings: Vec::new(),
            probe: None,
            probe_rows: Vec::new(),
            chrome: Vec::new(),
            choices: Vec::new(),
            transport: Vec::new(),
            top_bar: Vec::new(),
        }
    }
}

impl Layout {
    /// The status bar's part called `name` (`signal`, `instrument`, …), if it was drawn.
    #[must_use]
    pub fn status_part(&self, name: &str) -> Option<&str> {
        self.status_bar.iter().find(|(n, _)| *n == name).map(|(_, text)| text.as_str())
    }

    /// The control `id` (`type`, `camera`, `style shaded`, `maximise`, `gizmo/zoom`, …) of pane
    /// `pane`'s header or gizmo, if it was drawn.
    #[must_use]
    pub fn control(&self, pane: usize, id: &str) -> Option<&HeaderControl> {
        self.controls.iter().find(|c| c.pane == pane && c.id == id)
    }

    /// The panel called `name`, if it was shown. `options` is the tool's options row
    /// ([`Self::options`]), which is inside a viewport and so not among [`Self::panels`].
    #[must_use]
    pub fn panel(&self, name: &str) -> Option<Rect> {
        if name == "options" {
            return self.options;
        }
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

    /// The transport's control `id` (`start`, `step-back`, `play`, `step-forward`, `end`,
    /// `speed`, `time`, `shot`), if it was drawn.
    #[must_use]
    pub fn transport_control(&self, id: &str) -> Option<&crate::transport::Drawn> {
        self.transport.iter().rev().find(|control| control.id == id)
    }
}

/// One control of a viewport's header or of its navigation gizmo, as it was drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct HeaderControl {
    /// The pane it belongs to, from 0.
    pub pane: usize,
    /// Which control: `type`, `camera`, `style shaded`, `overlays`, `span`, `maximise`,
    /// `pop-out`, `close`, `gizmo/zoom`, `gizmo/pan`, `gizmo/projection`.
    pub id: String,
    /// Where it is, points.
    pub rect: Rect,
    /// What its tooltip says: what it does, with its action's key where it has one; for a
    /// disabled control, why.
    pub tooltip: String,
    /// The action of `ui/actions.ron` it runs, when it runs one (a menu's opener runs none).
    pub action: Option<String>,
    /// Whether it can be used now.
    pub enabled: bool,
}

/// Every step of the history in words: those done, oldest first, and those undone, the next first.
pub type HistoryWords = (Vec<String>, Vec<String>);

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
    /// How fast the playhead moves while playing, as a multiple of the clock's rate (`speed
    /// <x>`, 0.1 to 10; 1 at the start): seconds of the scene a second. Window state.
    pub speed: f64,
    /// The transport's menu that is open by a click or by its console line (`speed` or
    /// `at-end`). Window state.
    pub transport_menu: Option<String>,
    /// A viewport header's menu asked open by `viewport type|camera|overlays [N]`: the pane (from
    /// 0) and the menu's stop. Taken by the header when it next draws that viewport, which opens
    /// the menu as a click on its opener does. Window state.
    pub(crate) header_menu: Option<(usize, String)>,
    /// What playing does when the playhead reaches the end of the timeline (`end
    /// loop|stop|run-on`; it loops at the start). Window state.
    pub at_end: crate::transport::AtEnd,
    /// A run on under way (playing with [`crate::transport::AtEnd::RunOn`], the end reached).
    /// `None` while not playing.
    pub(crate) run_on: Option<crate::transport::RunOn>,
    /// The transport's step button held down by the pointer: which, when it went down (the
    /// frame's clock, seconds) and how many steps it has fired.
    pub(crate) transport_hold: Option<(&'static str, f64, u32)>,
    /// The layouts saved in this session (`layout save`, the top bar's `+`), after the built-in
    /// ones in the top bar. Window state: not saved with the scene, gone when the window closes.
    pub session_layouts: Vec<crate::layouts::LayoutData>,
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
    /// a far field cut to its budget, meshes or bricks past their caps), held in the tooltip of
    /// each scene viewport's warning dot (`docs/slices/S4.md` §7.6, the review's I5; a badge of
    /// the header strip until S20) and said once in the log.
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
    /// Whether the selection's row in the scene tree is to open its right-click menu when it is
    /// next drawn (`context`).
    pub(crate) open_context: bool,
    /// Whether the open right-click menu was opened by `context` (under its row) rather than by
    /// a right-click (at the pointer).
    pub(crate) context_at_row: bool,
    /// The item being renamed in the scene tree (`F2`), and the name typed so far.
    pub renaming: Option<(Target, String)>,
    /// How fast a flight moves, metres per second, before sprinting (the wheel changes it while
    /// flying, within [`crate::camera::FLY_SPEED_RANGE`]).
    pub fly_speed: f64,
    /// The pane being flown with the right mouse button held (Unity's and Unreal's fly: W A S D
    /// move, the mouse looks, the wheel sets the speed), while it is held. UI state.
    pub mouse_fly: Option<usize>,
    /// The first-person camera's settings (`ui/camera.ron`; `camera-set`). UI state.
    pub camera: crate::camera::Settings,
    /// What the first-person session under way holds between frames ([`crate::first_person`]).
    /// UI state.
    pub flight: crate::first_person::Flight,
    /// Whether the first-person camera passes through matter (`noclip`): off by default, the
    /// camera being physical. UI state.
    pub noclip: bool,
    /// Where a picture taken with no window rests its pointer, as fractions of the active pane
    /// from its top left (`pointer 0.5 0.7`): what shows a tool's hover (the Place tool's ghost)
    /// in the demo's pictures. UI state, read only by [`crate::headless::Headless::screenshot`].
    pub pointer_at: Option<[f32; 2]>,
    /// What the Probe tool last read, and where (S19): shown in the right panel's Probe tab, its
    /// point marked in the active pane, until another probe, another tool, or a new scene. UI state; `probe x y z` is the same reading.
    pub probed: Option<cav_sim::probe::Probe>,
    /// A probe asked for by `probe-at`, as fractions of the active pane: taken by the next frame
    /// that has the Probe tool in hand.
    pub probe_at: Option<[f32; 2]>,
    /// The bodies the scene tree's lock holds still, by label: the view's pointer moves none of
    /// them (S19, `docs/design/panels.md`). UI state, never journalled.
    pub locked: std::collections::BTreeSet<String>,
    /// The engine's count of loads and the bodies' names when the locks were last kept
    /// ([`Self::keep_locks`]).
    lock_labels: (u64, Vec<String>),
    /// The selection at the last frame: a new one shows the Inspector tab.
    seen_selection: Option<Target>,
    /// What the scene tree's filter keeps: rows whose names hold it.
    pub tree_filter: String,
    /// The scene tree's folded groups and folders, by scene name and row id (`tree/sources`,
    /// `tree/folder/<label>`): UI state, kept per scene.
    pub tree_folded: std::collections::BTreeSet<(String, String)>,
    /// The folder in focus in the scene tree, by label: the one a click on its row lit, which
    /// `rename` (F2) and `new-folder` act on. A folder is never the engine's selection (moving
    /// everything in one with the gizmo needs multi-selection: S22). UI state.
    pub tree_focus: Option<String>,
    /// The folder being renamed in the scene tree, by its label, and the name typed so far.
    pub renaming_folder: Option<(String, String)>,
    /// The row of the scene tree being dragged onto a folder or a group, while it is.
    pub(crate) tree_drag: Option<crate::outliner::TreeDrag>,
    /// The right-hand panel's tab.
    pub right_tab: crate::outliner::RightTab,
    /// The inspector's label column, points: a third of its width, at least 96 (S20's density), so
    /// that every row's value starts at the same x. Set as the panel is drawn.
    pub label_width: f32,
    /// What the inspector's ⋮ Copy took: a body's, a tower's or a part's whole value.
    pub clipboard: Option<serde_json::Value>,
    /// The tool in hand: an id of `ui/tools.ron`. UI state, never journalled.
    pub tool: String,
    /// Whether the Move tool snaps to the step.
    pub snap: bool,
    /// The starter the Place tool adds (a name of `scenes/starters.ron`): the variant of the
    /// tool chosen in its flyout (`tool place cube`).
    pub place_starter: String,
    /// Where the tool palette stands in its viewport's canvas, by layout name
    /// ([`Self::layout_name`]); a layout not here has it at the top left. Window state, never
    /// journalled.
    pub tool_anchors: std::collections::BTreeMap<String, crate::tool_palette::Anchor>,
    /// The palette in hand by its grip: how far the pointer is from the palette's corner,
    /// points. While it is held the palette follows the pointer.
    pub(crate) tool_drag: Option<egui::Vec2>,
    /// The tool (an id of `ui/tools.ron`) whose flyout of variants is open beside the palette.
    pub flyout: Option<String>,
    /// Whether the press that opened the flyout (a hold) is still down: its release takes no
    /// tool.
    pub(crate) flyout_held: bool,
    /// A drag of the Move tool's gizmo in hand.
    pub(crate) gizmo_drag: Option<GizmoDrag>,
    /// A transform session in hand (the keyboard's Move).
    pub session: Option<crate::tools::Session>,
    /// Whether the active pane is maximised: it fills the window but for the menu bar and the
    /// status line (`Ctrl+Space`). UI state.
    pub maximised: bool,
    /// The viewports that float over the window (`viewport pop-out`), by label, each with its
    /// frame in points. Window state: the mosaic keeps the viewport's place, the engine knows
    /// nothing of it, and nothing is journalled. A label that leaves the mosaic is dropped.
    /// (A frame of no size is one asked for before the window had drawn: the figure places it.)
    pub floating: std::collections::BTreeMap<char, Rect>,
    /// The viewport (pane, from 0) whose header is being dragged toward another viewport, to
    /// swap places with it when it is let go there. UI state.
    pub(crate) header_drag: Option<usize>,
    /// A splitter's drag in hand: the layout follows the pointer here until the drag ends.
    pub(crate) splitter: Option<crate::panels::SplitterDrag>,
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
    /// The Import mesh window, when it is open: what it will import.
    pub import_window: Option<crate::dialogs::ImportForm>,
    /// The file the scene came from or was last saved to, which `Ctrl+S` saves to again.
    pub file: Option<String>,
    /// The history in words ([`crate::dialogs::history_words`]) and the engine's generation they
    /// were made at: made again only when the engine has changed.
    history_said: Option<(u64, HistoryWords)>,
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

/// The unit a phase of magnitude `size` (radians) is read in: how many of the unit one radian
/// is (1, 10³, 10⁶ or 10⁹) and its name (rad, mrad, µrad, nrad). For a column of phases, `size`
/// is the column's largest magnitude, so the column has one unit (S20).
#[must_use]
pub fn phase_unit(size: f64) -> (f64, &'static str) {
    if size == 0.0 || size >= 1.0 || !size.is_finite() {
        (1.0, "rad")
    } else if size >= 1e-3 {
        (1e3, "mrad")
    } else if size >= 1e-6 {
        (1e6, "µrad")
    } else {
        (1e9, "nrad")
    }
}

/// A phase (`radians`) as a number of a unit of which one radian is `per_radian`
/// ([`phase_unit`]): signed, three decimals, no unit (the column's heading has it).
#[must_use]
pub fn phase_in(radians: f64, per_radian: f64) -> String {
    if !radians.is_finite() {
        return if radians.is_nan() { "not a number".into() } else { "unbounded".into() };
    }
    format!("{:+.3}", radians * per_radian)
}

/// A phase as people read it: in rad, mrad, µrad or nrad, with four figures.
#[must_use]
pub fn phase_text(radians: f64) -> String {
    if !radians.is_finite() {
        return phase_in(radians, 1.0);
    }
    let (per_radian, unit) = phase_unit(radians.abs());
    format!("{} {unit}", phase_in(radians, per_radian))
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
            speed: 1.0,
            transport_menu: None,
            header_menu: None,
            at_end: crate::transport::AtEnd::Loop,
            run_on: None,
            transport_hold: None,
            session_layouts: Vec::new(),
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
            open_context: false,
            context_at_row: false,
            renaming: None,
            fly_speed: crate::camera::Settings::from_data().fly_speed,
            mouse_fly: None,
            camera: crate::camera::Settings::from_data(),
            flight: crate::first_person::Flight::default(),
            noclip: false,
            pointer_at: None,
            probed: None,
            probe_at: None,
            locked: std::collections::BTreeSet::new(),
            lock_labels: (0, Vec::new()),
            seen_selection: None,
            tree_filter: String::new(),
            tree_folded: std::collections::BTreeSet::new(),
            tree_focus: None,
            renaming_folder: None,
            tree_drag: None,
            right_tab: crate::outliner::RightTab::Inspector,
            label_width: crate::inspector::LABEL_MIN,
            clipboard: None,
            tool: "select".to_owned(),
            snap: true,
            place_starter: "sphere".to_owned(),
            tool_anchors: std::collections::BTreeMap::new(),
            tool_drag: None,
            flyout: None,
            flyout_held: false,
            gizmo_drag: None,
            session: None,
            maximised: false,
            floating: std::collections::BTreeMap::new(),
            header_drag: None,
            splitter: None,
            fullscreen: false,
            fullscreen_sent: false,
            layout_name: crate::layouts::layouts().layouts[0].id.clone(),
            unfold: Vec::new(),
            history_open: false,
            wizard: None,
            open_window: None,
            save_window: None,
            import_window: None,
            file: None,
            untitled: false,
            history_said: None,
            pending_focus: None,
            title_said: String::new(),
        }
    }

    /// The window's title: the scene's name, with `*` while it has unsaved edits.
    #[must_use]
    pub fn title(&self) -> String {
        format!("Cavendish · {}{}", self.engine.spec().name, if self.unsaved() { " *" } else { "" })
    }

    /// Every step of the history in words, done then undone, cached by the engine's generation.
    pub fn history_words(&mut self) -> HistoryWords {
        let generation = self.engine.generation();
        if self.history_said.as_ref().is_none_or(|(at, _)| *at != generation) {
            self.history_said = Some((generation, crate::dialogs::history_words(&self.engine)));
        }
        self.history_said.as_ref().expect("made above").1.clone()
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

    /// Where the tool palette stands in its canvas under the layout chosen now.
    #[must_use]
    pub fn tool_anchor(&self) -> crate::tool_palette::Anchor {
        self.tool_anchors.get(&self.layout_name).copied().unwrap_or_default()
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

    /// A transform session the keyboard has left (a click on a window's control, a window opened
    /// with the mouse, anything that moved the focus) ends, its move kept, as a click outside a
    /// move keeps it in Blender: the keyboard is the one owner of "a session is open" (S18's
    /// second review, I2: the shell's session outlived the keyboard's, and a later key took the
    /// move back unasked).
    fn orphaned_session(&mut self) {
        if self.session.is_some() && self.keyboard.session != Some(crate::keyboard::Session::Transform) {
            self.end_session(true);
        }
    }

    /// The keyboard leaves a transform session it is in: back to navigation on the same element.
    fn end_keyboard_session(&mut self) {
        if self.keyboard.session == Some(crate::keyboard::Session::Transform) {
            self.keyboard.session = None;
            self.keyboard.mode = crate::keyboard::Mode::Navigation;
        }
    }

    /// A file dropped on the window (S19), by its name (natively its path): its bytes, when the
    /// platform gives them (a page), are kept by the engine under the name, and the Import mesh
    /// window opens on it. A file that is not a mesh is said in the log when it is imported.
    pub fn dropped_file(&mut self, name: &str, bytes: Option<Vec<u8>>) {
        if let Some(bytes) = bytes
            && let Err(why) = self.engine.keep_dropped(name, bytes)
        {
            self.engine.note(Level::Error, Subsystem::App, why);
            self.console_open = true;
            return;
        }
        self.import_window = Some(crate::dialogs::ImportForm::new(name));
        self.window_opened("import", "import/go", false);
    }

    /// Whether body `n` (from 0) is locked in the scene tree: the view's pointer moves it not.
    #[must_use]
    pub fn is_locked(&self, n: usize) -> bool {
        self.engine.spec().bodies.get(n).is_some_and(|b| self.locked.contains(&b.label))
    }

    /// Whether the scene tree's group `scope` is open for the scene now shown.
    #[must_use]
    pub fn tree_open(&self, scope: &str) -> bool {
        !self.tree_folded.contains(&(self.engine.spec().name.clone(), scope.to_owned()))
    }

    /// Whether a first-person session started by `fly` or `walk` is on (until the word again,
    /// Esc or Enter).
    #[must_use]
    pub fn flying(&self) -> bool {
        self.keyboard.session == Some(crate::keyboard::Session::Fly)
    }

    /// Whether the camera is in first person in either way: a session (`fly`, `walk`) or the
    /// right mouse button held over a scene pane.
    #[must_use]
    pub fn any_fly(&self) -> bool {
        self.flying() || self.mouse_fly.is_some()
    }

    /// The active pane's canvas's width over its height, from the last frame (1.6 before there
    /// is one).
    fn active_aspect(&self) -> f64 {
        self.layout
            .canvases
            .get(self.engine.active_pane())
            .filter(|r| r.height() > 0.0)
            .map_or(1.6, |r| f64::from(r.width() / r.height()))
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
            Needs::Named => {
                matches!(self.engine.selection(), Some(Target::Body(_) | Target::Tower(_)))
                    || self.tree_focus.is_some()
            }
            Needs::Body => matches!(self.engine.selection(), Some(Target::Body(_))),
            Needs::Tower(n) => n >= 1 && n <= spec.array.towers.len(),
            Needs::Shapes => spec.bodies.iter().any(|b| b.shape.is_some()),
            Needs::World => spec.world.is_some(),
            Needs::Viewports => self.engine.panes() > 1,
            Needs::AfterStart => self.engine.time() > 0.0,
            Needs::BeforeEnd => self.engine.time() < spec.duration,
            Needs::ShotBefore => crate::transport::place_now(self).is_ok_and(|p| p.earlier.is_some()),
            Needs::ShotAfter => crate::transport::place_now(self).is_ok_and(|p| p.later.is_some()),
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
            let rest: Vec<&str> = words.clone().collect();
            if matches!(rest.first(), Some(&("save" | "forget"))) {
                let outcome = self.layout_command(&rest);
                if let Err(why) = &outcome {
                    self.engine.note(Level::Error, Subsystem::App, why.clone());
                    self.console_open = true;
                }
                return outcome.map(|()| line.to_owned());
            }
            let all: Vec<crate::layouts::LayoutData> = self.layouts().cloned().collect();
            let ids: Vec<&str> = all.iter().map(|l| l.id.as_str()).collect();
            let Some(layout) = words
                .next()
                .and_then(|id| all.iter().find(|l| l.id == id))
                .filter(|_| words.next().is_none())
            else {
                // Not a named layout: a mosaic written out (`layout "AAB;CDB" cols 2 1 1`), which
                // is the engine's `mosaic` command.
                let written = line.trim_start_matches("layout").trim();
                return match cav_engine::Mosaic::parse(written) {
                    Ok(mosaic) => {
                        self.maximised = false;
                        self.run(&format!("mosaic {}", mosaic.quoted()))
                    }
                    Err(why) => {
                        let why = format!(
                            "`{line}`: layout <{}>, or a mosaic as `\"AAB;CDB\"` ({why})",
                            ids.join("|")
                        );
                        self.engine.note(Level::Error, Subsystem::App, why.clone());
                        self.console_open = true;
                        Err(why)
                    }
                };
            };
            self.maximised = false;
            self.layout_name.clone_from(&layout.id);
            self.unfold.clone_from(&layout.unfold);
            for step in &layout.lines {
                self.run(step)?;
            }
            return Ok(line.to_owned());
        }
        if name == "viewport" {
            let outcome = self.viewport_command(&words.collect::<Vec<_>>());
            if let Err(why) = &outcome {
                self.engine.note(Level::Error, Subsystem::App, why.clone());
                self.console_open = true;
            }
            return outcome.map(|()| line.to_owned());
        }
        // `step <n> [cycle]`, `speed <x>|menu` and `end <behaviour>|menu`: the transport's
        // (`crate::transport`).
        if matches!(name, "step" | "speed" | "end") {
            let rest: Vec<&str> = words.collect();
            let outcome = if name == "end" {
                use crate::transport::AtEnd;
                match rest[..] {
                    ["menu"] => {
                        let open = self.transport_menu.as_deref() == Some("at-end");
                        self.transport_menu = (!open).then(|| "at-end".to_owned());
                        Ok(())
                    }
                    [word] if AtEnd::parse(word).is_some() => {
                        self.at_end = AtEnd::parse(word).expect("just parsed");
                        // A run on in hand ends with the behaviour that made it.
                        self.run_on = None;
                        Ok(())
                    }
                    _ => {
                        let names: Vec<&str> = AtEnd::ALL.iter().map(|at_end| at_end.name()).collect();
                        Err(format!("`{line}`: end <{}>, or `end menu`", names.join("|")))
                    }
                }
            } else if name == "step" {
                match rest[..] {
                    [n] | [n, "cycle"] => match n.parse::<i64>() {
                        Ok(steps) if steps != 0 => crate::transport::step(self, steps, rest.len() == 2),
                        _ => Err(format!("`{n}` is not a number of steps; step <n> [cycle], n not 0")),
                    },
                    _ => Err(format!("`{line}`: step <n> [cycle]")),
                }
            } else {
                let (least, most) = (crate::transport::SPEEDS[0], crate::transport::SPEEDS[6]);
                match rest[..] {
                    ["menu"] => {
                        let open = self.transport_menu.as_deref() == Some("speed");
                        self.transport_menu = (!open).then(|| "speed".to_owned());
                        Ok(())
                    }
                    [x] => match x.trim_end_matches(['×', 'x']).parse::<f64>() {
                        Ok(speed) if (least..=most).contains(&speed) => {
                            self.speed = speed;
                            Ok(())
                        }
                        _ => Err(format!("`{x}` is not a speed; speed <{least} to {most}>, or `speed menu`")),
                    },
                    _ => Err(format!("`{line}`: speed <{least} to {most}>, or `speed menu`")),
                }
            };
            if let Err(why) = &outcome {
                self.engine.note(Level::Error, Subsystem::App, why.clone());
                self.console_open = true;
            }
            return outcome.map(|()| line.to_owned());
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
        // `tool <id> [variant]` takes a tool of the palette in hand, with one of its variants
        // if one is named (`tool place cube`); nothing is journalled.
        if name == "tool" {
            let tools = &crate::tools::strip().tools;
            let ids: Vec<&str> = tools.iter().map(|t| t.id.as_str()).collect();
            let (id, variant, more) = (words.next(), words.next(), words.next());
            let tool = id.and_then(|id| tools.iter().find(|t| t.id == id)).filter(|_| more.is_none());
            let outcome = match (tool, variant) {
                (Some(tool), None) => {
                    self.tool.clone_from(&tool.id);
                    Ok(line.to_owned())
                }
                (Some(tool), Some(variant)) => {
                    let names: Vec<String> = tool.variants.list().into_iter().map(|v| v.name).collect();
                    if names.iter().any(|name| name == variant) {
                        match tool.variants {
                            crate::tools::Variants::Starters => self.place_starter = variant.to_owned(),
                            crate::tools::Variants::None => {}
                        }
                        self.tool.clone_from(&tool.id);
                        Ok(line.to_owned())
                    } else if names.is_empty() {
                        Err(format!("`{line}`: the {} tool has no variants", tool.label))
                    } else {
                        Err(format!("`{line}`: tool {} <{}>", tool.id, names.join("|")))
                    }
                }
                (None, _) => Err(format!("`{line}`: tool <{}> [variant]", ids.join("|"))),
            };
            match &outcome {
                Ok(_) => self.flyout = None,
                Err(why) => {
                    self.engine.note(Level::Error, Subsystem::App, why.clone());
                    self.console_open = true;
                }
            }
            return outcome;
        }
        // `tool-palette <anchor>` stands the palette at an edge or a corner of its view, for
        // the layout chosen now; `flyout <tool>` opens a tool's variants beside it. Both are
        // the window's own state.
        if name == "tool-palette" || name == "flyout" {
            let word = words.next().filter(|_| words.next().is_none());
            let outcome = if name == "tool-palette" {
                let names: Vec<&str> =
                    crate::tool_palette::Anchor::ALL.iter().map(|anchor| anchor.name()).collect();
                match word.and_then(crate::tool_palette::Anchor::parse) {
                    Some(anchor) => {
                        self.tool_anchors.insert(self.layout_name.clone(), anchor);
                        Ok(line.to_owned())
                    }
                    None => Err(format!("`{line}`: tool-palette <{}>", names.join("|"))),
                }
            } else {
                let with: Vec<&str> = crate::tools::strip()
                    .tools
                    .iter()
                    .filter(|t| t.variants != crate::tools::Variants::None)
                    .map(|t| t.id.as_str())
                    .collect();
                match word {
                    Some("off") => {
                        self.flyout = None;
                        Ok(line.to_owned())
                    }
                    Some(id) if with.contains(&id) => {
                        self.flyout = Some(id.to_owned());
                        Ok(line.to_owned())
                    }
                    _ => Err(format!("`{line}`: flyout <{}|off>", with.join("|"))),
                }
            };
            if let Err(why) = &outcome {
                self.engine.note(Level::Error, Subsystem::App, why.clone());
                self.console_open = true;
            }
            return outcome;
        }
        // `camera-set [<setting> <value>]`: the first-person camera's settings, the window's own.
        if name == "camera-set" {
            let (setting, value, more) = (words.next(), words.next(), words.next());
            let outcome = match (setting, value.map(str::parse::<f64>), more) {
                (None, _, _) => {
                    self.console_open = true;
                    let rows: Vec<String> = crate::camera::SETTINGS
                        .iter()
                        .map(|(name, unit, _, _, about)| {
                            let value = self.camera.get(name).unwrap_or(f64::NAN);
                            format!("{name} = {value} {unit}  ({about})")
                        })
                        .collect();
                    Ok(rows.join("\n"))
                }
                (Some(setting), Some(Ok(value)), None) => {
                    self.camera.set(setting, value).map_err(|why| format!("`{line}`: {why}")).map(|()| {
                        if setting == "fly-speed" {
                            // The speed changed: it is shown, as after the wheel.
                            self.fly_speed = value;
                            self.show_fly_speed();
                        } else if setting == "walk-speed" {
                            self.show_fly_speed();
                        }
                        line.to_owned()
                    })
                }
                _ => Err(format!("`{line}`: {}", self.camera.clone().set("", 0.0).unwrap_err())),
            };
            match &outcome {
                Ok(said) if setting.is_none() => self.engine.note(Level::Info, Subsystem::App, said.clone()),
                Ok(_) => {}
                Err(why) => {
                    self.engine.note(Level::Error, Subsystem::App, why.clone());
                    self.console_open = true;
                }
            }
            return outcome;
        }
        // `eye x y z [heading pitch]` puts the active viewport's eye somewhere, looking some way;
        // `walk-for <seconds> [keys]` holds keys down in the first-person session under way.
        // Both are for scripts and pictures made with no window; neither is an edit.
        if name == "eye" || name == "walk-for" {
            let rest: Vec<&str> = words.collect();
            let outcome = if name == "eye" {
                let numbers: Result<Vec<f64>, _> = rest.iter().map(|w| w.parse::<f64>()).collect();
                match numbers.as_deref() {
                    Ok(&[x, y, z, ref angles @ ..]) if matches!(angles.len(), 0 | 2) => {
                        let pane = self.engine.active_pane();
                        let view = self.engine.view(pane).clone();
                        let mut camera = crate::camera::FirstPerson::of(&view, [0.0; 3]);
                        camera.eye = [x, y, z];
                        if let &[heading, pitch] = angles {
                            camera = crate::camera::FirstPerson::looking([x, y, z], heading, pitch);
                        }
                        self.apply(Command::View(camera.command(&view, pane)));
                        Ok(line.to_owned())
                    }
                    _ => Err(format!("`{line}`: eye <x> <y> <z> [<heading> <pitch>] (metres; degrees)")),
                }
            } else {
                let usage = || format!("`{line}`: walk-for <seconds> [w a s d space shift ctrl]");
                match rest.split_first().map(|(first, keys)| (first.parse::<f64>(), keys)) {
                    Some((Ok(seconds), keys)) if (0.0..=600.0).contains(&seconds) => {
                        crate::first_person::Held::of_words(keys.iter().copied())
                            .and_then(|held| self.walk_for(seconds, held))
                            .map(|()| line.to_owned())
                            .map_err(|why| format!("{}: {why}", usage()))
                    }
                    _ => Err(usage()),
                }
            };
            if let Err(why) = &outcome {
                self.engine.note(Level::Error, Subsystem::App, why.clone());
                self.console_open = true;
            }
            return outcome;
        }
        // `palette <menu>` opens the palette filtered to a menu (`palette add`: the starters).
        // `palette <menu>` opens the palette filtered to a menu (`palette add`: the starters);
        // `palette <words>` opens it with the words typed, as a search.
        if name == "palette"
            && let Some(first) = words.next()
        {
            let titles = crate::menus::titles();
            let rest: Vec<&str> = words.collect();
            self.palette = match titles
                .iter()
                .find(|t| t.eq_ignore_ascii_case(first))
                .filter(|_| rest.is_empty())
            {
                Some(title) => {
                    Palette { open: true, focus_query: true, menu: Some(title.clone()), ..Palette::default() }
                }
                None => {
                    let query = std::iter::once(first).chain(rest).collect::<Vec<_>>().join(" ");
                    Palette { open: true, focus_query: true, query, ..Palette::default() }
                }
            };
            self.window_opened("palette", "palette/query", true);
            return Ok(line.to_owned());
        }
        // These take their words below; every other takes none.
        let takes_words = matches!(name, "pointer" | "tab" | "probe-at" | "import-mesh" | "tree-fold");
        if !takes_words && words.clone().next().is_some() {
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
            "import-mesh" => {
                let file = words.next().map(str::to_owned).unwrap_or_default();
                self.import_window = Some(crate::dialogs::ImportForm::new(&file));
                self.window_opened("import", "import/file", true);
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
            "context" => match self.engine.selection() {
                Some(_) => self.open_context = true,
                None => {
                    let why = "select something first: `context` opens its row's right-click menu".to_owned();
                    self.engine.note(Level::Error, Subsystem::App, why.clone());
                    return Err(why);
                }
            },
            "tab" => match words.next() {
                Some("inspector") => self.right_tab = crate::outliner::RightTab::Inspector,
                Some("readings") => self.right_tab = crate::outliner::RightTab::Readings,
                Some("probe") => self.right_tab = crate::outliner::RightTab::Probe,
                _ => {
                    let why = "`tab <inspector|readings|probe>`".to_owned();
                    self.engine.note(Level::Error, Subsystem::App, why.clone());
                    return Err(why);
                }
            },
            "probe-at" => {
                let numbers: Vec<f32> = words.by_ref().filter_map(|w| w.parse().ok()).collect();
                match numbers[..] {
                    [x, y] if (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y) => {
                        self.probe_at = Some([x, y]);
                    }
                    _ => {
                        let why = "`probe-at <x> <y>`: two fractions of the pane, from 0 to 1".to_owned();
                        self.engine.note(Level::Error, Subsystem::App, why.clone());
                        return Err(why);
                    }
                }
            }
            "pointer" => {
                let numbers: Vec<f32> = words.by_ref().filter_map(|w| w.parse().ok()).collect();
                match numbers[..] {
                    [x, y] if (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y) => {
                        self.pointer_at = Some([x, y]);
                    }
                    _ => {
                        let why = "`pointer <x> <y>`: two fractions of the pane, from 0 to 1".to_owned();
                        self.engine.note(Level::Error, Subsystem::App, why.clone());
                        return Err(why);
                    }
                }
            }
            "maximise" => self.maximised = !self.maximised,
            "fullscreen" => self.fullscreen = !self.fullscreen,
            "frame" => {
                // The selection (or, with nothing selected, the whole scene) fills the active
                // pane from the present direction: one camera command.
                let pane = self.engine.active_pane();
                let (centre, radius) =
                    crate::camera::bounds_of(self.engine.spec(), self.engine.selection(), self.engine.time());
                // A voxel body's selection box stays in the pane too.
                let cage = crate::camera::cage_of(self.engine.spec(), self.engine.selection());
                let command = crate::camera::frame_caged(
                    self.engine.view(pane),
                    pane,
                    centre,
                    radius,
                    cage,
                    self.active_aspect(),
                );
                self.apply(Command::View(command));
            }
            "fly" | "walk" => {
                if let Err(why) = self.first_person(name == "fly") {
                    self.engine.note(Level::Error, Subsystem::App, why.clone());
                    self.console_open = true;
                    return Err(why);
                }
            }
            "noclip" => self.toggle_noclip(),
            "tree-fold" => {
                // The row's name is the rest of the line, quotes or not.
                let rest = line.trim_start_matches("tree-fold").trim().trim_matches('"');
                let id = match rest.to_lowercase().as_str() {
                    "array" => Some("tree/array".to_owned()),
                    "bodies" => Some("tree/sources".to_owned()),
                    "surroundings" => Some("tree/surroundings".to_owned()),
                    _ => self.engine.spec().folder(rest).map(|f| crate::outliner::folder_id(&f.label)),
                };
                let Some(id) = id else {
                    let why = format!(
                        "`tree-fold <array|bodies|surroundings|folder>`: `{rest}` is no row that folds"
                    );
                    self.engine.note(Level::Error, Subsystem::App, why.clone());
                    return Err(why);
                };
                let key = (self.engine.spec().name.clone(), id);
                if !self.tree_folded.remove(&key) {
                    self.tree_folded.insert(key);
                }
                return Ok(line.to_owned());
            }
            "new-folder" => {
                if let Err(why) = crate::outliner::new_folder(self) {
                    self.engine.note(Level::Error, Subsystem::App, why.clone());
                    return Err(why);
                }
            }
            "rename" if self.tree_focus.as_ref().is_some_and(|f| self.engine.spec().folder(f).is_some()) => {
                let label = self.tree_focus.clone().unwrap_or_default();
                self.renaming_folder = Some((label.clone(), label));
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

    /// Every layout the top bar offers: the built-in ones (`ui/layouts.ron`), then those saved in
    /// this session ([`Self::session_layouts`]).
    pub fn layouts(&self) -> impl Iterator<Item = &crate::layouts::LayoutData> {
        crate::layouts::layouts().layouts.iter().chain(&self.session_layouts)
    }

    /// `layout save [name]` and `layout forget <id>`: the session's own layouts. Saving keeps the
    /// figure's arrangement as it stands (the mosaic with its weights, what each viewport shows,
    /// which is active) as the lines that make it again, under a new id, and makes it the layout
    /// chosen now. Nothing is journalled: a layout is the window's.
    fn layout_command(&mut self, words: &[&str]) -> Result<(), String> {
        /// The most layouts a session keeps (the top bar is one row).
        const MOST: usize = 6;
        match words {
            ["save", name @ ..] => {
                if self.session_layouts.len() >= MOST {
                    return Err(format!(
                        "a session keeps {MOST} layouts of its own; `layout forget <id>` drops one"
                    ));
                }
                let taken = |id: &str| self.layouts().any(|l| l.id == id) || matches!(id, "save" | "forget");
                let number = (1..).find(|n| !taken(&format!("layout-{n}"))).expect("a free number");
                let label = if name.is_empty() { format!("Layout {number}") } else { name.join(" ") };
                let slug: String =
                    label.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
                let id = if name.is_empty() { format!("layout-{number}") } else { slug };
                if taken(&id) || cav_engine::Mosaic::parse(&id).is_ok() {
                    return Err(format!(
                        "`{id}` is taken (a layout, or a mosaic's own writing): give another name"
                    ));
                }
                let mosaic = self.engine.mosaic().quoted();
                let mut lines = vec![format!("mosaic {mosaic}")];
                for pane in 0..self.engine.panes() {
                    lines.push(format!("show {} @{}", self.engine.content(pane).name(), pane + 1));
                }
                lines.push(format!("pane {}", self.engine.active_pane() + 1));
                // The palette's place goes with the arrangement it was set for.
                if let Some(anchor) = self.tool_anchors.get(&self.layout_name).copied() {
                    self.tool_anchors.insert(id.clone(), anchor);
                }
                self.session_layouts.push(crate::layouts::LayoutData {
                    id: id.clone(),
                    label,
                    lines,
                    unfold: Vec::new(),
                    about: format!("saved in this session: {mosaic} (`layout forget {id}` drops it)"),
                });
                self.layout_name = id;
                Ok(())
            }
            ["forget", id] => {
                let Some(at) = self.session_layouts.iter().position(|l| l.id == *id) else {
                    return Err(format!("`{id}` is not a layout saved in this session"));
                };
                self.session_layouts.remove(at);
                self.tool_anchors.remove(*id);
                if self.layout_name == *id {
                    self.layout_name.clone_from(&crate::layouts::layouts().layouts[0].id);
                }
                Ok(())
            }
            _ => Err("layout save [name] · layout forget <id>".to_owned()),
        }
    }

    /// `viewport <close|swap|pop-out|dock|type|camera|overlays> [N]`: what the View menu and a
    /// viewport's right-click menu do to one viewport (the active one, or number N from 1). Close
    /// and swap are the engine's `close @N` and `swap N M` (one `Mosaic` command each,
    /// journalled); pop-out and dock are the window's own state and journal nothing; `type`,
    /// `camera` and `overlays` open that menu of the viewport's header, as a click on it does
    /// (for scripts and pictures made with no window).
    fn viewport_command(&mut self, words: &[&str]) -> Result<(), String> {
        const USAGE: &str = "viewport <close|swap|pop-out|dock|type|camera|overlays> [N]";
        let count = self.engine.panes();
        let (verb, pane, named) = match *words {
            [verb] => (verb, self.engine.active_pane(), false),
            [verb, n] => match n.parse::<usize>().ok().and_then(|n| n.checked_sub(1)) {
                Some(pane) if pane < count => (verb, pane, true),
                _ => return Err(format!("there is no viewport {n}; the figure has 1 to {count}")),
            },
            _ => return Err(USAGE.to_owned()),
        };
        let label = self.engine.mosaic().label(pane).expect("the pane is one of the mosaic's");
        match verb {
            "close" => self.run(&format!("close @{}", pane + 1)).map(|_| ()),
            "swap" if count < 2 => Err("there is no other viewport to swap with".to_owned()),
            "swap" => self.run(&format!("swap {} {}", pane + 1, (pane + 1) % count + 1)).map(|_| ()),
            "pop-out" if self.floating.remove(&label).is_some() => Ok(()),
            "pop-out" => {
                // Over its own place, a little in from the corner, at a size that leaves the
                // window in sight; before any frame has been drawn, a default.
                let frame = match (self.layout.slots.get(pane), self.layout.figure) {
                    (Some(slot), figure) if figure.is_positive() => crate::panels::float_frame(*slot, figure),
                    // No frame has been drawn (a script's first lines): the figure places it by
                    // the same rule when it first draws it.
                    _ => Rect::NOTHING,
                };
                self.maximised = false;
                self.floating.insert(label, frame);
                Ok(())
            }
            "dock" if named => {
                self.floating.remove(&label);
                Ok(())
            }
            "dock" => {
                self.floating.clear();
                Ok(())
            }
            "camera" | "overlays" if self.engine.content(pane) != cav_engine::PaneContent::Scene => {
                Err(format!("viewport {} shows a plot: its header has no {verb} menu", pane + 1))
            }
            "type" | "camera" | "overlays" => {
                self.header_menu = Some((pane, verb.to_owned()));
                Ok(())
            }
            _ => Err(USAGE.to_owned()),
        }
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
                    world: None,
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
        self.point_on_screen(pane, at, size)
    }

    /// Where the world's point `at` (metres) is drawn in pane `pane` of `size` points, from its
    /// top left; None behind the camera.
    #[must_use]
    pub fn point_on_screen(&self, pane: usize, at: [f64; 3], size: egui::Vec2) -> Option<egui::Pos2> {
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
                    Event::PointerButton { pressed: true, pos, button, .. } => {
                        clicked = Some((*pos, *button == egui::PointerButton::Primary));
                    }
                    _ => {}
                }
            }
            (presses, released, clicked)
        });
        // The right button let go or the window's focus lost: the mouse's fly ends (S18's second
        // review, minor 4). The pointer leaving the window while the button is held does not end
        // it (S18's third review, minor 3: a wide look turn reaches the edge, and winit reports
        // the cursor gone though the button is down); egui keeps the button down until its
        // release is seen, as it does for any drag.
        if self.mouse_fly.is_some() && ctx.input(|i| !i.pointer.secondary_down()) {
            self.end_fly();
        }
        // The window's focus lost ends a flight however it was started: the keys and the
        // pointer are another window's now.
        if self.any_fly() && ctx.input(|i| !i.focused) {
            self.end_fly();
        }
        // A primary press anywhere but the tool's options row ends a transform session, its move
        // kept: a click on a row of the tree, the canvas, a menu (S18's third review, minor 1; as
        // a click outside a move keeps it in Blender). The options row is the session's own (it
        // is shown while a session is under way; were it not, any press would end the session).
        if let Some((pos, true)) = clicked
            && self.session.is_some()
            && !self.layout.panel("options").is_some_and(|r| r.contains(pos))
        {
            self.end_session(true);
        }
        if let Some((pos, _)) = clicked {
            self.keyboard.pointer();
            self.repeat.clear();
            // A click on a control in a window moves the keyboard's focus onto it, in navigation
            // mode, the ring hidden (the spec's §8); anywhere else (a panel, the canvas) the focus
            // stays where it was.
            let top = ctx.layer_id_at(pos);
            if let Some(id) = self.tree.window_node_at(pos, top).map(str::to_owned) {
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
        // While the camera is flown, every key is the flight's (`first_person`): the held keys
        // are read by `fly_frame`, and no press reaches the router or a widget. W is not the Move
        // tool, A not Place, 2 selects no tower, Delete deletes nothing, Space does not play,
        // Ctrl+S (sprinting backwards) does not save (S18's reviews, U3 and minor 3; the owner's
        // ruling of 25 Nov 2026).
        let shortcuts = crate::actions::table().shortcuts();
        if self.any_fly() {
            self.fly_keys(&routed, now);
            routed.clear();
            ctx.input_mut(|i| i.events.retain(|event| !matches!(event, Event::Key { .. } | Event::Text(_))));
        }
        if self.repeat.any() {
            ctx.request_repaint();
        }
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
                } else if self.flyout.is_some() {
                    self.flyout = None;
                } else if self.transport_menu.is_some() {
                    self.transport_menu = None;
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
                "import" => self.import_window = None,
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

    /// Moves the playhead while playing, by the frame's time step (at most a tenth of a second)
    /// times the speed ([`Self::speed`]). At the end of the timeline it does what
    /// [`Self::at_end`] says: back to the start; or the playhead stops at exactly the duration
    /// and playing stops; or the run goes on ([`Self::run_on_past`]).
    pub(crate) fn advance(&mut self, ctx: &egui::Context) {
        use crate::transport::AtEnd;
        if !self.playing {
            self.run_on = None;
            return;
        }
        let step = f64::from(ctx.input(|i| i.stable_dt)).min(0.1) * self.speed;
        let duration = self.engine.spec().duration;
        let next = self.engine.time() + step;
        if next < duration {
            self.apply(Command::Seek(next));
        } else {
            match self.at_end {
                AtEnd::Loop => {
                    self.apply(Command::Seek(0.0));
                }
                AtEnd::Stop => {
                    self.apply(Command::Seek(duration));
                    self.playing = false;
                }
                AtEnd::RunOn => self.run_on_past(next),
            }
        }
        ctx.request_repaint();
    }

    /// Run on: the playhead is to go to `next` (seconds), at or past the end of the timeline.
    /// The scene's duration grows by whole blocks of its length when this run first reached the
    /// end ([`crate::transport::RunOn::extend`]), by the `set` of `duration` an editor issues,
    /// each extension continuing the one before so that the whole run is one step of undo (the
    /// seeks between are no edits and do not end it: `cav-engine`'s
    /// `a_run_of_sets_is_one_step_whatever_comes_between`). Never past the schema's longest
    /// timeline: there the playhead stops, playing stops, and the log says so.
    ///
    /// The cost is the engine's: each extension is an edit of the scene, so the signal is stale
    /// until its worker has computed it for the longer timeline (off this thread). An extension
    /// that finds the last one's signal still being computed adds twice the blocks.
    fn run_on_past(&mut self, next: f64) {
        let limit = cav_sim::spec::limits::DURATION;
        let duration = self.engine.spec().duration;
        let mut run = self.run_on.unwrap_or_else(|| crate::transport::RunOn::new(duration));
        self.run_on = Some(run);
        if duration >= limit {
            self.apply(Command::Seek(duration));
            self.playing = false;
            self.engine.note(
                Level::Warn,
                Subsystem::App,
                format!("run on stopped at {limit} s: a timeline is no longer than that"),
            );
            return;
        }
        let continues = run.extended;
        let behind = self.engine.signal().0 == cav_engine::SignalState::Stale;
        let longer = run.extend(duration, next, behind, limit);
        let set = crate::editors::command_for("duration", serde_json::json!(longer), continues);
        if self.apply(set) {
            self.run_on = Some(run);
            self.apply(Command::Seek(next.min(longer)));
        } else {
            // The engine has said why (in the log, the console open).
            self.playing = false;
        }
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

    /// The tree's locks kept with their bodies (S19's review, minor 7): a lock follows its body
    /// through a rename, and a newly loaded scene starts with none, so no lock passes to another
    /// scene's body of the same name. Run at the start of every frame, against the bodies' names
    /// at the last.
    pub fn keep_locks(&mut self) {
        let loads = self.engine.loads();
        let labels: Vec<String> = self.engine.spec().bodies.iter().map(|b| b.label.clone()).collect();
        let (seen_loads, seen) = std::mem::take(&mut self.lock_labels);
        if seen_loads != loads {
            self.locked.clear();
        } else if seen.len() == labels.len() {
            for (old, new) in seen.iter().zip(&labels) {
                if old != new && self.locked.remove(old) {
                    self.locked.insert(new.clone());
                }
            }
        }
        self.lock_labels = (loads, labels);
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
        for (name, bytes) in crate::platform::dropped(&ctx) {
            self.dropped_file(&name, bytes);
        }
        self.keep_locks();
        // A new selection brings the Inspector forward (Unity, Godot: S19's review, UX).
        let selection = self.engine.selection();
        if selection != self.seen_selection {
            if selection.is_some() {
                self.right_tab = crate::outliner::RightTab::Inspector;
                // Something selected takes the tree's light from a folder.
                self.tree_focus = None;
            }
            self.seen_selection = selection;
        }
        self.session_keys();
        self.orphaned_session();
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
        crate::first_person::shield(self, &ctx);
        self.fly_capture(&ctx);
        self.orphaned_session();
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
    /// pane's corner, in a pane of `size` points), which stays where it is on screen. (While
    /// flying the wheel is the flight's: `fly_frame` takes it and it never comes here.)
    pub(crate) fn zoom_by(&mut self, pane: usize, scroll: f32, pointer: [f64; 2], size: [f64; 2]) {
        let factor = f64::from(-scroll * 0.002).exp();
        if self.any_fly() {
            return;
        }
        let view = self.engine.view(pane).clone();
        let factor = factor.clamp(0.05 / view.distance, 1.0e5 / view.distance);
        self.apply(Command::View(crate::camera::zoom_about(&view, pane, size, pointer, factor)));
    }
}
