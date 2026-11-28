//! The engine's vocabulary: commands change state, queries read it, replies answer.
//!
//! Everything is plain serialisable data, so a command can come from a button, a console line, the
//! CLI, an MCP tool or a file, and be journalled and replayed.

use cav_sim::{DrawStyle, NamedView, PaneContent, Projection, SceneSpec, Target, ViewSpec};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::log::{LogEntry, LogFilter};

/// The most panes (viewports) a figure holds: the mosaic's cap, 12. A thirteenth is refused with a
/// message that says so.
pub const MAX_PANES: usize = cav_sim::mosaic::MAX_VIEWPORTS;

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
        /// The field's path in the scene spec.
        path: String,
        /// Its new value, as JSON, in the field's own unit (the schema says which).
        value: serde_json::Value,
        /// Whether this is a step of a drag after its first.
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
    /// Change the scene tree's folders (`cav_sim::folders`): make one, move bodies, towers or
    /// folders into one or out, rename one (its members follow), remove one (what it holds
    /// moves up: nothing is deleted), duplicate one. An edit: one step of undo, whatever it
    /// moved. Folders are for people: no tower's reading changes.
    Folder(cav_sim::FolderEdit),
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
    /// Read the same scene on another instrument as well, beside the scene's own (`Some`), or
    /// stop doing so (`None`). Not an edit: the scene is unchanged. While it is on, signals and
    /// periodograms can be asked for `compared`, and the plots draw both.
    Compare(Option<Box<cav_sim::InstrumentSpec>>),
    /// Compute again from a stage, dropping what was kept for it and the stages after it.
    Run {
        /// The first stage to compute again.
        from: Stage,
    },
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum ExportKind {
    /// The scene as a `.ron` file: what `load` reads back.
    Scene,
    /// The journal so far as JSON: a session that `cav replay` runs again.
    Journal,
    /// The bundle: a directory of `.npy` arrays named and shaped as v1's `StateBundle`, each
    /// tower's readout times, the numerics and the per-tower error estimate, with a manifest
    /// (`cav_sim::bundle`). Written only from a refined signal.
    Bundle,
    /// A figure: a plot or a view as an SVG, a PDF or a PNG at a stated size, with the data
    /// behind it (`.csv`, or `.npy` when it is long) and a `.json` of its axes, units and
    /// provenance beside it, under the same name. The engine does not draw: the client's
    /// exporter does ([`crate::Engine::set_figure_exporter`]; `cav-plot` has one). A figure of a
    /// signal or a periodogram is written only from a refined signal.
    Figure(Box<Figure>),
}

impl ExportKind {
    /// The word the console uses.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Scene => "scene",
            Self::Journal => "journal",
            Self::Bundle => "bundle",
            Self::Figure(_) => "figure",
        }
    }
}

/// What a figure shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum FigureKind {
    /// A channel of every tower's signal (or of one tower's) against readout time.
    Signal,
    /// The periodogram of one tower's signal, with the combs.
    Periodogram,
    /// The spectral window of one tower's shots, with the predicted lines.
    Cadence,
    /// The atoms' flights on absolute time.
    Flight,
    /// A pane's view of the scene, as the renderer draws it.
    View,
    /// The scene as a technical drawing on paper, orthographic: a plan from above or a vertical
    /// section ([`PaperView`]), with a scale bar, a north arrow, dimension lines and a title
    /// block.
    Paper,
}

impl FigureKind {
    /// Every kind, in the order the console lists them.
    pub const ALL: [Self; 6] =
        [Self::Signal, Self::Periodogram, Self::Cadence, Self::Flight, Self::View, Self::Paper];

    /// The word the console uses.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Signal => "signal",
            Self::Periodogram => "periodogram",
            Self::Cadence => "cadence",
            Self::Flight => "flight",
            Self::View => "view",
            Self::Paper => "paper",
        }
    }

    /// The kind a word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == word)
    }
}

/// What a paper drawing ([`FigureKind::Paper`]) shows: the scene from above, or a vertical cut
/// through it. Positions are metres in the world's frame (x east, y north, z up).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum PaperView {
    /// From above, north up the page.
    #[default]
    Plan,
    /// The cut through the plane x = this many metres, looking west: north to the right of the
    /// page, up up it. Finite.
    SectionX(f64),
    /// The cut through the plane y = this many metres, looking north: east to the right of the
    /// page, up up it. Finite.
    SectionY(f64),
}

impl PaperView {
    /// The words the console writes it as: `plan`, `section x 12`, `section y -40`.
    #[must_use]
    pub fn words(self) -> String {
        match self {
            Self::Plan => "plan".into(),
            Self::SectionX(at) => format!("section x {at}"),
            Self::SectionY(at) => format!("section y {at}"),
        }
    }

    /// The view some text names: `plan`, or a section as `section x 12`, `x 12`, `x=12` or
    /// `y,-40` (metres). `None` when it names none, or the place is not a finite number.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let words: Vec<&str> = text
            .split(|c: char| c.is_whitespace() || c == '=' || c == ',')
            .filter(|w| !w.is_empty())
            .collect();
        let (axis, at) = match words.as_slice() {
            ["plan"] => return Some(Self::Plan),
            ["section", axis, at] | [axis, at] => (*axis, *at),
            _ => return None,
        };
        let at = at.parse::<f64>().ok().filter(|v| v.is_finite())?;
        match axis {
            "x" => Some(Self::SectionX(at)),
            "y" => Some(Self::SectionY(at)),
            _ => None,
        }
    }

    /// The place of a section's cut, metres, if it is one and that is finite; `Some(0)` for a
    /// plan. `None` only for a section at a place that is not a number.
    #[must_use]
    pub fn finite(self) -> Option<f64> {
        match self {
            Self::Plan => Some(0.0),
            Self::SectionX(at) | Self::SectionY(at) => at.is_finite().then_some(at),
        }
    }
}

/// The format of a figure's file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum FigureFormat {
    /// SVG: vector, text as outlines.
    Svg,
    /// PDF: vector, text as outlines, one page the size of the figure.
    Pdf,
    /// PNG: pixels, at the figure's size times its resolution.
    Png,
}

impl FigureFormat {
    /// The file extension, which is also the word for the format: `svg`, `pdf`, `png`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Pdf => "pdf",
            Self::Png => "png",
        }
    }

    /// The format a file's name asks for, by its extension (in either case).
    #[must_use]
    pub fn of_file(file: &str) -> Option<Self> {
        let extension = std::path::Path::new(file).extension()?.to_str()?.to_ascii_lowercase();
        [Self::Svg, Self::Pdf, Self::Png].into_iter().find(|format| format.name() == extension)
    }
}

/// The preset a figure has when none is named.
pub const DEFAULT_FIGURE_PRESET: &str = "paper-column";

/// The largest side of a figure, millimetres, and the largest resolution, pixels per inch: a
/// size a person may write decides how much is drawn. (A PNG's pixels are bounded as well, by
/// the exporter: size × resolution, each side.)
pub const MAX_FIGURE_MM: f64 = 2000.0;
/// See [`MAX_FIGURE_MM`].
pub const MAX_FIGURE_DPI: f64 = 2400.0;

/// A figure to export: what it shows, how it looks and how big it is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Figure {
    /// What it shows.
    pub what: FigureKind,
    /// The format of the file; the file's extension must be the format's.
    pub format: FigureFormat,
    /// The preset that gives its look and its size: `paper-column`, `paper-full`, `slide`,
    /// `poster` or `screen`.
    pub preset: String,
    /// The tower, from 1: whose periodogram or spectral window it is; for a signal, the one
    /// tower drawn (every tower if absent). Not used by a flight diagram or a view.
    #[serde(default)]
    pub tower: Option<usize>,
    /// The channel of a signal or a periodogram.
    #[serde(default)]
    pub channel: ChannelName,
    /// The stretch of the horizontal axis shown: seconds for a signal or a flight diagram, hertz
    /// for a periodogram or a window; all of it if absent.
    #[serde(default)]
    pub span: Option<[f64; 2]>,
    /// For a view: the pane whose camera it is, from 0.
    #[serde(default)]
    pub pane: usize,
    /// The width and height, millimetres, in place of the preset's.
    #[serde(default)]
    pub size_mm: Option<[f64; 2]>,
    /// The resolution of a PNG, pixels per inch, in place of the preset's.
    #[serde(default)]
    pub dpi: Option<f64>,
    /// Whether the picture has a title at its top (it has unless told otherwise: `untitled` on
    /// the console, `--no-title` at the command line, `"title": false` over MCP). Without one
    /// the axes take the room, and the record's `title` is `null`; the name an SVG or a PDF
    /// carries in its metadata, which is not drawn, is kept.
    #[serde(default = "titled")]
    pub title: bool,
    /// For a paper drawing: a plan or a section; a plan if absent. Not used by any other kind
    /// (and absent from their JSON).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paper: Option<PaperView>,
}

/// A figure has a title unless told otherwise.
const fn titled() -> bool {
    true
}

impl Figure {
    /// A figure of `what` in `format` with the default preset and everything else as the
    /// preset has it.
    #[must_use]
    pub fn new(what: FigureKind, format: FigureFormat) -> Self {
        Self {
            what,
            format,
            preset: DEFAULT_FIGURE_PRESET.to_owned(),
            tower: None,
            channel: ChannelName::default(),
            span: None,
            pane: 0,
            size_mm: None,
            dpi: None,
            title: true,
            paper: None,
        }
    }

    /// Whether the figure is of the signal, and so is written only from a refined one.
    #[must_use]
    pub const fn needs_signal(&self) -> bool {
        matches!(self.what, FigureKind::Signal | FigureKind::Periodogram)
    }
}

/// A change to the figure's panes and their cameras.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum ViewCommand {
    /// Split the figure into this many panes: 1, 2 or 4. Short for the mosaics `A`, `AB` and
    /// `AB;CD` ([`Self::Mosaic`], with no `from`).
    Panes(usize),
    /// Arrange the figure as this mosaic of viewports (`cav_sim::mosaic`). What a split, a
    /// close, a swap and a splitter's drag each end as: one command, journalled and replayed,
    /// never an edit, never undone.
    ///
    /// A viewport is known by its label. One whose label is in the figure already keeps its
    /// camera, its style and what it shows, wherever the new mosaic puts it and whatever its
    /// number becomes (a pane's number is its label's rank in the alphabet among those present).
    /// A label new to the figure starts as a copy of `from`'s viewport when `from` is given (a
    /// split), and otherwise takes the camera and content last left in a pane that is not shown
    /// (as `panes 4` after `panes 1` brings the three others back as they were). The active
    /// pane stays with its label; if that has gone, the first viewport is active.
    Mosaic {
        /// The mosaic.
        layout: cav_sim::mosaic::Mosaic,
        /// The label of a viewport of the figure as it is now, which the labels new to the
        /// figure start as copies of; `None` for none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<char>,
    },
    /// Make this pane (from 0) the one that keys and unaddressed commands act on.
    Activate(usize),
    /// Look from a named direction, keeping the target and the distance.
    Look {
        /// The pane, from 0.
        pane: usize,
        /// The direction to look from.
        view: NamedView,
    },
    /// Perspective or orthographic.
    Project {
        /// The pane, from 0.
        pane: usize,
        /// The projection.
        projection: Projection,
    },
    /// The draw style.
    Style {
        /// The pane, from 0.
        pane: usize,
        /// How the scene is drawn.
        style: DrawStyle,
    },
    /// What the pane shows: the scene, or a plot.
    Show {
        /// The pane, from 0.
        pane: usize,
        /// What it shows.
        content: PaneContent,
    },
    /// The stretch of a plot's horizontal axis a pane shows: seconds for the signal, hertz for the
    /// periodogram; `None` for all of it.
    Span {
        /// The pane, from 0.
        pane: usize,
        /// `[from, to]` on the plot's horizontal axis, in that axis's unit; `None` for all of it.
        span: Option<[f64; 2]>,
    },
    /// Orbit to these angles, degrees; pitch is clamped to ±90.
    Orbit {
        /// The pane, from 0.
        pane: usize,
        /// The angle about the world's z axis (up), degrees.
        yaw: f64,
        /// The angle above the horizontal, degrees.
        pitch: f64,
    },
    /// Set the distance from the target to the camera, metres.
    Zoom {
        /// The pane, from 0.
        pane: usize,
        /// From the target to the camera, metres.
        distance: f64,
    },
    /// Move the target, metres.
    Pan {
        /// The pane, from 0.
        pane: usize,
        /// The point looked at: `[x, y, z]` in the world's frame (x east, y north, z up), metres.
        target: [f64; 3],
    },
    /// Put the camera exactly here in one step: the target, the angles and the distance
    /// together (fly, frame the selection, zoom toward the pointer). Angles in degrees (pitch
    /// clamped to ±90, yaw wrapped into (−180, 180]); the target in metres in the world's frame
    /// (x east, y north, z up); the distance in metres, clamped as `Zoom`'s. Never an edit.
    Camera {
        /// The pane, from 0.
        pane: usize,
        /// The point looked at, metres.
        target: [f64; 3],
        /// The angle about the world's z axis (up), degrees.
        yaw: f64,
        /// The angle above the horizontal, degrees.
        pitch: f64,
        /// From the target to the camera, metres.
        distance: f64,
    },
    /// Back to the pane's home camera for the current scene; the style is kept. The one camera
    /// move that undo takes back (the owner's rule, `docs/slices/S18.md` §2): an edit of the
    /// view, not of the scene.
    Home {
        /// The pane, from 0.
        pane: usize,
    },
    /// What colours the leaves of voxel bodies in the pane.
    Colour {
        /// The pane, from 0.
        pane: usize,
        /// The field, and for `phase` the tower.
        colour: cav_sim::voxel_view::Colouring,
    },
    /// What the pane draws over voxel bodies: leaf bounds, a cut.
    Overlay {
        /// The pane, from 0.
        pane: usize,
        /// The overlays.
        overlay: cav_sim::voxel_view::Overlay,
    },
    /// A section plane through the pane's voxel bodies, or none.
    Section {
        /// The pane, from 0.
        pane: usize,
        /// The plane; None for no section.
        section: Option<cav_sim::voxel_view::Section>,
    },
    /// Hide the body of this label from every pane, or show it again (the scene tree's eye, S19).
    /// A view's: the towers still read it, and it is never undone.
    Hide {
        /// The body's label.
        label: String,
        /// True to hide, false to show.
        hidden: bool,
    },
}

impl ViewCommand {
    /// Whether this view command is an edit (a step of undo): only [`Self::Home`], by the
    /// owner's rule. Every other is journalled and never undone.
    #[must_use]
    pub const fn is_edit(&self) -> bool {
        matches!(self, Self::Home { .. })
    }

    /// The pane the command addresses, if it addresses one.
    #[must_use]
    pub const fn pane(&self) -> Option<usize> {
        match *self {
            Self::Panes(_) | Self::Mosaic { .. } | Self::Hide { .. } => None,
            Self::Activate(pane)
            | Self::Look { pane, .. }
            | Self::Project { pane, .. }
            | Self::Style { pane, .. }
            | Self::Show { pane, .. }
            | Self::Span { pane, .. }
            | Self::Orbit { pane, .. }
            | Self::Zoom { pane, .. }
            | Self::Pan { pane, .. }
            | Self::Camera { pane, .. }
            | Self::Home { pane }
            | Self::Colour { pane, .. }
            | Self::Overlay { pane, .. }
            | Self::Section { pane, .. } => Some(pane),
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
        use ViewCommand::{Camera, Orbit, Pan, Zoom};
        match (self, previous) {
            (Self::Seek(_), Self::Seek(_)) => true,
            (Self::Set { path: a, continues: true, .. }, Self::Set { path: b, .. }) => a == b,
            (Self::View(Camera { pane: a, .. }), Self::View(Camera { pane: b, .. }))
            | (Self::View(Orbit { pane: a, .. }), Self::View(Orbit { pane: b, .. }))
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
        /// The scene as read on the instrument being compared (`Command::Compare`), in place
        /// of its own.
        #[serde(default)]
        compared: bool,
    },
    /// What the scene's schedule is predicted to do to a spectrum, from its closed forms: the
    /// mean rate, the alias lines and their heights, the floor, the shots kept.
    Cadence,
    /// The spectral window of the shots one tower takes: `|W(ν)|` from 0 to a frequency, one
    /// realisation for the scene's seed.
    Window {
        /// The tower, counted from 1 as on screen.
        tower: usize,
        /// The highest frequency, hertz; 2.5 times the mean shot rate if absent.
        #[serde(default)]
        to: Option<f64>,
    },
    /// What every tower would read from a shot read out at this time, seconds (the playhead if
    /// none is given). The engine's reading worker computes it; this answers with the last whole
    /// reading and says whether it is the one asked for (`refined`), an earlier one (`stale`) or
    /// none yet (`computing`). Nothing is computed on the caller's thread.
    Reading(Option<f64>),
    /// The periodogram of one channel of one tower's signal, with the combs of the scene's
    /// oscillating bodies: the classical periodogram when the tower's shots are evenly spaced and
    /// all taken, the Lomb–Scargle periodogram (by non-uniform FFT) otherwise; the reply's
    /// `method` says which. Computed when asked, from the signal as it stands.
    Periodogram {
        /// The tower, counted from 1 as on screen.
        tower: usize,
        /// Which of the tower's three channels.
        channel: ChannelName,
        /// The scene as read on the instrument being compared, in place of its own.
        #[serde(default)]
        compared: bool,
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
    /// Its harmonics on the axis, from the fundamental up, at most [`COMB_HARMONICS`].
    pub harmonics: Vec<Harmonic>,
}

/// The periodogram of one channel of one tower's signal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PeriodogramReply {
    /// The tower, from 1.
    pub tower: usize,
    /// Which of the tower's channels: an interferometer's phase or the gradiometer's, radians.
    pub channel: ChannelName,
    /// As for a signal: whether it is the current scene's.
    pub state: Standing,
    /// The Nyquist frequency, hertz: half the shot rate.
    pub nyquist: f64,
    /// How it was made: the classical periodogram (an FFT) when the tower's shots are evenly
    /// spaced and all taken, the Lomb–Scargle periodogram (by non-uniform FFT) when they are not.
    pub method: String,
    /// How many shots went into it: those the tower took.
    pub shots: usize,
    /// The bins' frequencies, hertz, from one over the timeline's length to Nyquist.
    pub frequencies: Vec<f64>,
    /// `S_k = (Δt/N)·|FFT(y − ȳ)|²` at each bin, rad² per hertz: one-sided, not doubled.
    pub power: Vec<f64>,
    /// Each oscillating body's first harmonics and where they fold to.
    pub combs: Vec<Comb>,
    /// Present when the signal it is made from left matter out: the world was read only within
    /// this many metres of each tower (as [`SignalReply::truncated`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncated: Option<f64>,
}

/// The spectral window of one tower's shots.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WindowReply {
    /// The tower, from 1.
    pub tower: usize,
    /// The frequencies, hertz, from 0.
    pub frequencies: Vec<f64>,
    /// `|W(ν)|` at each: 1 at zero frequency; a tone at `f` is imaged at `f ± ν` at this height.
    pub window: Vec<f64>,
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
    /// Which of the tower's channels: an interferometer's phase or the gradiometer's, radians.
    pub channel: ChannelName,
    /// `refined`: the current scene's signal. `stale`: the scene has changed and this is the last
    /// whole signal; ask again. `computing`: there is none to show yet.
    pub state: Standing,
    /// When this tower read each shot out, seconds: the array's clock and the tower's own offset.
    pub times: Vec<f64>,
    /// Whether the tower took each shot. A shot not taken (a planned gap, a lost shot) keeps its
    /// place, with a value of zero that is not a measurement.
    #[serde(default)]
    pub taken: Vec<bool>,
    /// The channel's value at each shot, radians. Over a half-space a single interferometer's
    /// phase is infinite: in JSON that is the string `"inf"` (`cav_sim::json_f64`).
    #[serde(with = "cav_sim::json_f64::list")]
    #[schemars(schema_with = "cav_sim::json_f64::list_schema")]
    pub values: Vec<f64>,
    /// The largest quadrature estimate of any shot of any source, radians.
    #[serde(with = "cav_sim::json_f64")]
    #[schemars(schema_with = "cav_sim::json_f64::schema")]
    pub estimate: f64,
    /// Whether every read settled to the tolerance and read all the scene's matter; when false
    /// the estimate is how far the worst was from settling, or (see `truncated`) of part of the
    /// matter only: not a bound that was met.
    pub settled: bool,
    /// Present when matter was left out: the world was read only within this many metres of each
    /// tower's atoms' path, and these values are of that matter alone (0: the world was refused,
    /// past its octree's caps).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncated: Option<f64>,
    /// Present when the scene has a world and it was read: each tower's ledger and cut.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world: Option<cav_sim::read::WorldLedger>,
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
    /// Whether every read settled to the tolerance and read all the scene's matter; when false
    /// the estimate is how far the worst was from settling, or (see `truncated`) of part of the
    /// matter only: not a bound that was met.
    pub settled: bool,
    /// Present when matter was left out: the world was read only within this many metres of each
    /// tower's atoms' path, and these numbers are of that matter alone (0: the world was refused,
    /// past its octree's caps).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncated: Option<f64>,
    /// Present when the scene has a world and it was read: each tower's ledger and cut.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world: Option<cav_sim::read::WorldLedger>,
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
    /// What the schedule is predicted to do.
    Cadence(cav_sim::schedule::Prediction),
    /// See [`WindowReply`].
    Window(WindowReply),
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
    /// How the figure is divided into those panes: the mosaic as written (`AAB;CDB cols 2 1 1`).
    pub mosaic: String,
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
    /// The instrument the scene is also being read on, by its preset's name if it is one
    /// (`AICE 140 m`), else `another instrument`; `None` when nothing is being compared.
    pub compare: Option<String>,
}
