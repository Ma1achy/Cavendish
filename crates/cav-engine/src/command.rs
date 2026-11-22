//! The engine's vocabulary: commands change state, queries read it, replies answer.
//!
//! Everything is plain serialisable data, so a command can come from a button, a console line, the
//! CLI, an MCP tool or a file, and be journalled and replayed.

use cav_sim::{DrawStyle, NamedView, Projection, SceneSpec, Target, ViewSpec};
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
            | Self::Orbit { pane, .. }
            | Self::Zoom { pane, .. }
            | Self::Pan { pane, .. }
            | Self::Home { pane } => Some(pane),
        }
    }
}

impl Command {
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
            (Self::View(Orbit { pane: a, .. }), Self::View(Orbit { pane: b, .. }))
            | (Self::View(Zoom { pane: a, .. }), Self::View(Zoom { pane: b, .. }))
            | (Self::View(Pan { pane: a, .. }), Self::View(Pan { pane: b, .. })) => a == b,
            _ => false,
        }
    }
}

/// Something that reads the engine's state and changes nothing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Query {
    /// The scene spec, or the part of it at a path such as `array.towers[2].position`.
    Spec(Option<String>),
    /// The JSON Schema of the scene spec.
    Schema,
    /// Counts and state at a glance.
    Stats,
    /// Entries of the event log.
    Log(LogFilter),
    /// One pane's view.
    View(usize),
    /// The commands applied so far, as journalled.
    Journal,
}

/// The answer to a query.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Reply {
    /// A scene spec, or part of one, as JSON.
    Spec(serde_json::Value),
    /// A JSON Schema.
    Schema(serde_json::Value),
    /// See [`Stats`].
    Stats(Stats),
    /// Log entries, oldest first.
    Log(Vec<LogEntry>),
    /// A pane's view.
    View(ViewSpec),
    /// The journal, oldest first.
    Journal(Vec<Command>),
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
    /// Edits that can be undone.
    pub undo: usize,
    /// Edits that can be redone.
    pub redo: usize,
    /// How many commands have been applied; it rises with every change of state.
    pub generation: u64,
    /// A digest of the whole state; equal digests mean equal states.
    pub digest: String,
}
