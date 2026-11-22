//! The engine's vocabulary: commands change state, queries read it, replies answer.
//!
//! Everything is plain serialisable data, so a command can come from a button, a console line, the
//! CLI, an MCP tool or a file, and be journalled and replayed.

use cav_sim::{DrawStyle, NamedView, PaneContent, Projection, SceneSpec, Target, ViewSpec};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::log::{LogEntry, LogFilter};

/// The most panes a figure is split into.
pub const MAX_PANES: usize = 4;

/// Something that changes the engine's state. Every applied command is journalled and logged.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Command {
    /// Replace the scene. An edit: it can be undone. (Boxed: a scene is far larger than any
    /// other command.)
    Load(Box<SceneSpec>),
    /// Set the value at a path of the scene spec, such as `bodies[0].mass` or
    /// `bodies[0].motion.Static.position`. The scene that results must be a valid one. An edit: it
    /// can be undone. `continues` marks a step of a drag after its first: such a set overwrites the
    /// set before it (same path), so a whole drag is one journal entry and one undo. Two sets that
    /// a person or an agent issues separately are two edits, whatever their paths.
    Set {
        path: String,
        value: serde_json::Value,
        #[serde(default)]
        continues: bool,
    },
    /// Put an item into a list of the scene spec (`bodies`, `array.towers`): before item `index`,
    /// or at the end if there is none. The scene that results must be a valid one. An edit.
    Insert {
        /// The path of the list.
        path: String,
        /// Where the item goes, from 0; at the end if absent.
        #[serde(default)]
        index: Option<usize>,
        /// The item, as JSON of the list's item type.
        value: serde_json::Value,
    },
    /// Take an item out of a list of the scene spec: the path ends in its index, `bodies[1]`. The
    /// items after it move down by one. An edit.
    Remove {
        /// The path of the item.
        path: String,
    },
    /// Write something to a file. Not an edit and not a change of state: it is a command so that
    /// the journal says what a session wrote. Replaying a journal never writes to the paths in
    /// it: the replayer names a directory for exports, or they are skipped
    /// ([`crate::Exports`]).
    Export {
        /// What is written.
        what: ExportKind,
        /// The file (or, for kinds that are several files, the directory) to write.
        to: String,
    },
    /// Compute again from a stage, dropping what was kept for it and the stages after it.
    Run { from: Stage },
    /// Move the playhead to this time, seconds; clamped to the scene's duration.
    Seek(f64),
    /// Select something, or nothing.
    Select(Option<Target>),
    /// Change how a pane looks at the scene. Never an edit.
    View(ViewCommand),
    /// Take back the last edit.
    Undo,
    /// Reapply the edit last taken back.
    Redo,
}

/// What an [`Command::Export`] writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ExportKind {
    /// The scene as a `.ron` file: what `load` reads back.
    Scene,
    /// The journal so far as JSON: a session that `cav replay` runs again.
    Journal,
}

impl ExportKind {
    /// The word the console uses.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Scene => "scene",
            Self::Journal => "journal",
        }
    }
}

/// A change to the figure's panes and their cameras.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum ViewCommand {
    /// Split the figure into this many panes: 1, 2 or 4.
    Panes(usize),
    /// Make this pane (from 0) the one that keys and unaddressed commands act on.
    Activate(usize),
    /// Look from a named direction, keeping the target and the distance.
    Look { pane: usize, view: NamedView },
    /// Perspective or orthographic.
    Project { pane: usize, projection: Projection },
    /// The draw style.
    Style { pane: usize, style: DrawStyle },
    /// What the pane shows: the scene, or a plot.
    Show { pane: usize, content: PaneContent },
    /// The stretch of a plot's horizontal axis a pane shows: seconds for the signal, hertz for the
    /// periodogram; `None` for all of it.
    Span { pane: usize, span: Option<[f64; 2]> },
    /// Orbit to these angles, degrees; pitch is clamped to ±90.
    Orbit { pane: usize, yaw: f64, pitch: f64 },
    /// Set the distance from the target to the camera, metres.
    Zoom { pane: usize, distance: f64 },
    /// Move the target, metres.
    Pan { pane: usize, target: [f64; 3] },
    /// Back to the pane's home camera for the current scene; the style is kept.
    Home { pane: usize },
}

impl ViewCommand {
    /// The pane the command addresses, if it addresses one.
    #[must_use]
    pub const fn pane(&self) -> Option<usize> {
        match *self {
            Self::Panes(_) => None,
            Self::Activate(pane)
            | Self::Look { pane, .. }
            | Self::Project { pane, .. }
            | Self::Style { pane, .. }
            | Self::Show { pane, .. }
            | Self::Span { pane, .. }
            | Self::Orbit { pane, .. }
            | Self::Zoom { pane, .. }
            | Self::Pan { pane, .. }
            | Self::Home { pane } => Some(pane),
        }
    }
}

impl Command {
    /// The command that sets the value at `path`: an edit of its own.
    #[must_use]
    pub fn set(path: &str, value: serde_json::Value) -> Self {
        Self::Set { path: path.to_owned(), value, continues: false }
    }

    /// The command that replaces the scene with `spec`.
    #[must_use]
    pub fn load(spec: SceneSpec) -> Self {
        Self::Load(Box::new(spec))
    }

    /// Whether this command, arriving straight after `previous`, simply overwrites what `previous`
    /// set. Such runs (a scrub, a drag) are kept as one journal entry: the last.
    #[must_use]
    pub fn overwrites(&self, previous: &Self) -> bool {
        use ViewCommand::{Orbit, Pan, Zoom};
        match (self, previous) {
            (Self::Seek(_), Self::Seek(_)) => true,
            (Self::Set { path: a, continues: true, .. }, Self::Set { path: b, .. }) => a == b,
            (Self::View(Orbit { pane: a, .. }), Self::View(Orbit { pane: b, .. }))
            | (Self::View(Zoom { pane: a, .. }), Self::View(Zoom { pane: b, .. }))
            | (Self::View(Pan { pane: a, .. }), Self::View(Pan { pane: b, .. }))
            | (
                Self::View(ViewCommand::Span { pane: a, .. }),
                Self::View(ViewCommand::Span { pane: b, .. }),
            ) => a == b,
            _ => false,
        }
    }
}

/// Something that reads the engine's state and changes nothing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Query {
    /// The scene spec, or the part of it at a path such as `array.towers[2].position`.
    Spec(Option<String>),
    /// The JSON Schema of the scene spec, or of the field at a path: its type, range, unit
    /// (`x-unit`), the stage an edit of it invalidates (`x-stage`) and its doc string.
    Schema(Option<String>),
    /// What is inside the field at a path (the whole spec if none): each child with its kind,
    /// unit, range, doc string and value. A field that holds one value lists itself.
    Ls(Option<String>),
    /// Counts and state at a glance.
    Stats,
    /// Entries of the event log.
    Log(LogFilter),
    /// One pane's view.
    View(usize),
    /// The commands applied so far, as journalled.
    Journal,
    /// One channel of one tower's signal over the timeline: a value for every shot. The answer
    /// says whether it is the current scene's (`refined`) or the last whole one (`stale`).
    Signal {
        /// The tower, counted from 1 as on screen.
        tower: usize,
        /// Which of the tower's three channels.
        channel: ChannelName,
        /// Only the shots read out in this stretch of the timeline, seconds: from the first
        /// number up to and including the second. All of them if absent.
        #[serde(default)]
        span: Option<[f64; 2]>,
    },
    /// What every tower would read from a shot read out at this time, seconds (the playhead if
    /// none is given). The engine's reading worker computes it; this answers with the last whole
    /// reading and says whether it is the one asked for (`refined`), an earlier one (`stale`) or
    /// none yet (`computing`). Nothing is computed on the caller's thread.
    Reading(Option<f64>),
    /// The classical periodogram of one channel of one tower's signal, with the combs of the
    /// scene's oscillating bodies. Computed when asked, from the signal as it stands.
    Periodogram {
        /// The tower, counted from 1 as on screen.
        tower: usize,
        channel: ChannelName,
    },
}

/// Where an answer stands against what was asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Standing {
    /// It is the current scene's, for the time asked.
    Refined,
    /// The scene (or the time) has changed and this is the last whole answer; ask again.
    Stale,
    /// There is nothing to show yet.
    Computing,
}

impl Standing {
    /// The word used in text and in JSON: `refined`, `stale` or `computing`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Refined => "refined",
            Self::Stale => "stale",
            Self::Computing => "computing",
        }
    }
}

impl std::fmt::Display for Standing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq<&str> for Standing {
    fn eq(&self, word: &&str) -> bool {
        self.as_str() == *word
    }
}

/// One harmonic of a body's comb.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Harmonic {
    /// Which harmonic: 1 is the fundamental.
    pub n: u32,
    /// Its true frequency, `n·f₀`, hertz.
    pub frequency: f64,
    /// Where it appears in the periodogram, hertz: itself below Nyquist, its fold above.
    pub appears_at: f64,
    /// Whether it is above Nyquist and so appears folded.
    pub folded: bool,
}

/// The comb of one oscillating body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Comb {
    /// The body's label.
    pub label: String,
    /// Its fundamental, hertz.
    pub fundamental: f64,
    pub harmonics: Vec<Harmonic>,
}

/// The periodogram of one channel of one tower's signal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PeriodogramReply {
    /// The tower, from 1.
    pub tower: usize,
    pub channel: ChannelName,
    /// As for a signal: whether it is the current scene's.
    pub state: Standing,
    /// The Nyquist frequency, hertz: half the shot rate.
    pub nyquist: f64,
    /// The bins' frequencies, hertz, from one over the timeline's length to Nyquist.
    pub frequencies: Vec<f64>,
    /// `S_k = (Δt/N)·|FFT(y − ȳ)|²` at each bin, rad² per hertz: one-sided, not doubled.
    pub power: Vec<f64>,
    /// Each oscillating body's first harmonics and where they fold to.
    pub combs: Vec<Comb>,
}

/// How many harmonics of each body a comb lists.
pub const COMB_HARMONICS: u32 = 8;

/// A stage of the simulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Stage {
    /// The forward model: every tower's signal.
    Forward,
}

/// A channel of a tower's signal. Interferometer 1 is the lower one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub enum ChannelName {
    /// The gradiometer's differential phase, ΔΦ.
    #[default]
    Gradiometer,
    /// The lower interferometer alone, δφ₁.
    Lower,
    /// The upper interferometer alone, δφ₂.
    Upper,
}

impl ChannelName {
    /// The word the console uses.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Gradiometer => "gradiometer",
            Self::Lower => "lower",
            Self::Upper => "upper",
        }
    }

    /// The channel a word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        [Self::Gradiometer, Self::Lower, Self::Upper].into_iter().find(|c| c.name() == word)
    }

    /// The forward model's own name for it.
    #[must_use]
    pub const fn channel(self) -> cav_core::forward::Channel {
        match self {
            Self::Gradiometer => cav_core::forward::Channel::Gradiometer,
            Self::Lower => cav_core::forward::Channel::Lower,
            Self::Upper => cav_core::forward::Channel::Upper,
        }
    }
}

/// One channel of one tower's signal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SignalReply {
    /// The tower, from 1.
    pub tower: usize,
    pub channel: ChannelName,
    /// `refined`: the current scene's signal. `stale`: the scene has changed and this is the last
    /// whole signal; ask again. `computing`: there is none to show yet.
    pub state: Standing,
    /// When each shot is read out, seconds.
    pub times: Vec<f64>,
    /// The channel's value at each shot, radians. Over a half-space a single interferometer's
    /// phase is infinite: in JSON that is the string `"inf"` (`cav_sim::json_f64`).
    #[serde(with = "cav_sim::json_f64::list")]
    #[schemars(schema_with = "cav_sim::json_f64::list_schema")]
    pub values: Vec<f64>,
    /// The largest quadrature estimate of any shot of any source, radians.
    #[serde(with = "cav_sim::json_f64")]
    #[schemars(schema_with = "cav_sim::json_f64::schema")]
    pub estimate: f64,
    /// Whether every read settled to the tolerance; when false the estimate is how far the worst
    /// was from settling, not a bound that was met.
    pub settled: bool,
    /// What a person should know about these readings, each with the tower it is about (from 0).
    pub warnings: Vec<cav_sim::read::Warning>,
}

/// What every tower reads from one shot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReadingReply {
    /// When the shot these numbers are for is read out, seconds.
    pub time: f64,
    /// `refined`: the reading asked for. `stale`: the last whole reading, for an earlier time or
    /// scene; ask again. `computing`: there is none to show yet.
    pub state: Standing,
    /// Each tower's reading, in the towers' order; empty while `computing`.
    pub towers: Vec<TowerReading>,
    /// The largest quadrature estimate of any source at any tower, radians: how far these numbers
    /// can be trusted.
    #[serde(with = "cav_sim::json_f64")]
    #[schemars(schema_with = "cav_sim::json_f64::schema")]
    pub estimate: f64,
    /// Whether every read settled to the tolerance; when false the estimate is how far the worst
    /// was from settling, not a bound that was met.
    pub settled: bool,
    /// What a person should know about these numbers: a reading that did not settle, a body in
    /// the atoms' path, an unbounded floor. Each with the tower it is about (from 0).
    pub warnings: Vec<cav_sim::read::Warning>,
}

/// What one tower reads from one shot, radians.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TowerReading {
    /// The tower's label.
    pub label: String,
    /// δφ₁, the lower interferometer.
    #[serde(with = "cav_sim::json_f64")]
    #[schemars(schema_with = "cav_sim::json_f64::schema")]
    pub lower: f64,
    /// δφ₂, the upper interferometer.
    #[serde(with = "cav_sim::json_f64")]
    #[schemars(schema_with = "cav_sim::json_f64::schema")]
    pub upper: f64,
    /// ΔΦ, with the instrument's sign.
    #[serde(with = "cav_sim::json_f64")]
    #[schemars(schema_with = "cav_sim::json_f64::schema")]
    pub gradiometer: f64,
}

/// The answer to a query.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Reply {
    /// A scene spec, or part of one, as JSON.
    Spec(serde_json::Value),
    /// A JSON Schema.
    Schema(serde_json::Value),
    /// The fields inside a path, in the order they are declared.
    Entries(Vec<cav_sim::paths::Entry>),
    /// See [`Stats`].
    Stats(Stats),
    /// Log entries, oldest first.
    Log(Vec<LogEntry>),
    /// A pane's view.
    View(ViewSpec),
    /// The journal, oldest first.
    Journal(Vec<Command>),
    /// See [`SignalReply`].
    Signal(SignalReply),
    /// See [`ReadingReply`].
    Reading(ReadingReply),
    /// See [`PeriodogramReply`].
    Periodogram(PeriodogramReply),
}

/// The engine's state at a glance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Stats {
    /// The engine's version.
    pub engine: String,
    /// The scene's name.
    pub scene: String,
    /// Number of towers.
    pub towers: usize,
    /// Number of bodies.
    pub bodies: usize,
    /// Whether the scene has a room.
    pub room: bool,
    /// Whether the scene has a floor.
    pub floor: bool,
    /// The playhead, seconds.
    pub time: f64,
    /// The timeline's length, seconds.
    pub duration: f64,
    /// What is selected.
    pub selection: Option<Target>,
    /// How many panes the figure shows.
    pub panes: usize,
    /// The active pane, from 0.
    pub active_pane: usize,
    /// Commands in the journal.
    pub journal: usize,
    /// Whether the journal holds every command applied: false once it has reached its size
    /// and stopped recording.
    pub journal_whole: bool,
    /// Edits that can be undone.
    pub undo: usize,
    /// Edits that can be redone.
    pub redo: usize,
    /// How many commands have been applied; it rises with every change of state.
    pub generation: u64,
    /// A digest of the whole state; equal digests mean equal states.
    pub digest: String,
    /// Where the signal stands: `refined`, `stale` or `none`.
    pub signal: String,
}
