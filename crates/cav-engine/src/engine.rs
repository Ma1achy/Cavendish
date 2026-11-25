//! The engine: owns the scene and the views, applies commands, answers queries.
//!
//! S0's engine has no stages to run, so it is a plain value with `&mut` methods and no worker;
//! the worker and the stage cache arrive with the first thing worth waiting for (S1's forward pass).
//! What is fixed already is the shape: state changes only through [`Engine::apply`], every applied
//! command is journalled, and the journal replayed on a fresh engine gives the same state.

use cav_sim::{DrawStyle, NamedView, PaneContent, Projection, SceneSpec, SceneView, Target, ViewSpec};
use serde::Serialize;

use crate::command::{
    COMB_HARMONICS, Comb, Command, Harmonic, MAX_PANES, PeriodogramReply, Query, ReadingReply, Reply,
    SignalReply, Standing, Stats, TowerReading, ViewCommand, WindowReply,
};
use crate::console;
use crate::forward::{Forward, SignalState};
use crate::history::{Edit, History, JOURNAL_BYTES, Step, weigh};
use crate::log::{Level, Log, Subsystem};

/// How long a line typed at a console waits for the numbers it asks for, unless a client sets
/// otherwise: a tenth of a second, which a person does not see and a long computation outlasts
/// (the answer then says `stale` or `computing`). Nothing the application does every frame waits.
pub const TYPED_LINE_PATIENCE: std::time::Duration = std::time::Duration::from_millis(100);

/// How long a replay waits for the signal an export is written from, unless told otherwise
/// ([`Engine::replay_into_within`]): an hour. A replay is a batch job, and a journal may hold a
/// long series; a tenth of a second would refuse what the session itself wrote.
pub const REPLAY_PATIENCE: std::time::Duration = std::time::Duration::from_secs(3600);

/// A quadrature estimate as people read it. `settled` says whether every read reached the
/// tolerance: only then is the estimate a bound ("within"); otherwise it is how far the worst
/// read was from settling, and is said as that.
#[must_use]
pub fn estimate_text(estimate: f64, settled: bool) -> String {
    if !settled {
        if estimate.is_finite() {
            format!("did not settle, estimate {estimate:.0e} rad")
        } else {
            "did not settle".to_owned()
        }
    } else if estimate == 0.0 {
        "exact".to_owned()
    } else {
        format!("within {estimate:.0e} rad")
    }
}

/// Where an engine's exports go.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Exports {
    /// To the path each command names: a session a person or an agent is driving.
    #[default]
    AsNamed,
    /// Into this directory and never outside it: a replay, which must not write where someone
    /// else's journal says.
    ///
    /// - A relative path keeps its folders under the directory: `figures/a.svg` is written as
    ///   `<directory>/figures/a.svg`, the folders made as needed.
    /// - An absolute path, or one with `..` anywhere in it, keeps its file name alone.
    /// - `/` and `\` both separate, whatever the platform, so a journal lands the same way
    ///   wherever it is replayed.
    /// - The same journal path lands in the same place every time, as it did in the session
    ///   (a file saved twice is written twice). Two different journal paths that would land on
    ///   one file (`b.ron` and `/somewhere/b.ron`) are kept apart: the later is written as
    ///   `b (2).ron`, the next as `b (3).ron`, and the log says so. Names are compared without
    ///   regard to case, as Windows compares them.
    Into(std::path::PathBuf),
    /// Nowhere: the command is applied and journalled and nothing is written.
    Skipped,
}

/// What a command will do, worked out before anything is changed.
enum Prepared {
    /// Nothing needs working out beforehand.
    Plain,
    /// An edit: the scene it makes, and the step that undoes and redoes it.
    Edit(Box<SceneSpec>, Step),
    /// An undo or a redo: the scene it restores.
    Restore(Box<SceneSpec>),
    /// An export: the file to write and its bytes.
    Write(std::path::PathBuf, Vec<u8>),
    /// An export of a bundle: the directory to write it into.
    Bundle(std::path::PathBuf),
    /// An export of a figure: the file to write, which the client's exporter draws.
    Figure(std::path::PathBuf),
}

impl Prepared {
    /// The file or the directory an export writes, if this is one.
    fn path(&self) -> Option<&std::path::Path> {
        match self {
            Self::Write(path, _) | Self::Bundle(path) | Self::Figure(path) => Some(path),
            _ => None,
        }
    }
}

/// What the instrument of a comparison is called.
fn compared_name(instrument: &cav_sim::InstrumentSpec) -> &'static str {
    cav_sim::presets::name_of(instrument).unwrap_or("another instrument")
}

/// What draws a figure and writes its files: the figure's file at the path given, its data and
/// its provenance beside it. It answers with every file it wrote. The engine cannot draw (it
/// knows nothing of fonts or pictures), so a client that can installs one
/// ([`Engine::set_figure_exporter`]).
pub type FigureExporter =
    Box<dyn Fn(&Engine, &crate::Figure, &std::path::Path) -> Result<Vec<std::path::PathBuf>, String> + Send>;

/// The client's figure exporter, if it has installed one.
#[derive(Default)]
struct Drawer(Option<FigureExporter>);

impl std::fmt::Debug for Drawer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() { "a figure exporter" } else { "no figure exporter" })
    }
}

/// How many frequencies a spectral window is answered at.
pub const WINDOW_POINTS: usize = 1201;

/// How many log entries the engine keeps.
const LOG_CAPACITY: usize = 2000;
/// The closest and furthest a camera may stand from its target, metres.
const DISTANCE_RANGE: (f64, f64) = (0.05, 1.0e5);

/// The engine. Create one with [`Engine::new`]; it starts on the default scene.
#[derive(Debug)]
pub struct Engine {
    spec: SceneSpec,
    time: f64,
    selection: Option<Target>,
    panes: usize,
    active: usize,
    views: [ViewSpec; MAX_PANES],
    /// What each pane shows.
    contents: [PaneContent; MAX_PANES],
    /// The stretch of its plot's horizontal axis each pane shows.
    spans: [Option<[f64; 2]>; MAX_PANES],
    history: History,
    journal: Vec<Command>,
    /// The bytes the journal is counted as holding, and whether it still records every command.
    journal_bytes: usize,
    journal_whole: bool,
    journal_budget: usize,
    /// The bytes the journal's last entry is counted as.
    journal_last: usize,
    /// The last command applied since the journal stopped recording: what a run of overwrites
    /// is then compared with, in place of the journal's last entry, which no longer moves.
    unrecorded: Option<Command>,
    /// While exports go into a directory: each journal path exported so far and where under the
    /// directory it landed.
    landed: Vec<(String, std::path::PathBuf)>,
    /// Whether the last command offered was refused: a step that says it continues the edit
    /// before it then has nothing to continue.
    refused: bool,
    /// Where exports are written.
    exports: Exports,
    /// What draws figures: the client's.
    drawer: Drawer,
    /// The last spectral window answered, with what it was made from.
    window: std::sync::Mutex<Option<(String, WindowReply)>>,
    /// The last cadence prediction answered, with what it was made from.
    prediction: std::sync::Mutex<Option<(String, cav_sim::schedule::Prediction)>>,
    log: Log,
    /// `log.total()` just after the last command's entry: tells whether that entry is still the newest.
    log_mark: u64,
    generation: u64,
    /// The forward stage: its worker, what it has kept, the last whole signal.
    forward: Forward,
    /// The instrument the scene is also read on, and that reading's own forward stage.
    compare: Option<(Box<cav_sim::InstrumentSpec>, Forward)>,
    /// How long a text client waits for the signal before it asks for one (`run_line`).
    patience: std::time::Duration,
    /// The voxel bodies' trees, made when a shape or a mass changes, for drawing.
    voxels: crate::voxels::Voxels,
}

/// Everything a replay must reproduce, in one serialisable piece. The journal and the log are not
/// part of it: they are the record, not the state.
#[derive(Serialize)]
struct State<'a> {
    spec: &'a SceneSpec,
    time: f64,
    selection: Option<Target>,
    panes: usize,
    active: usize,
    views: &'a [ViewSpec; MAX_PANES],
    contents: [PaneContent; MAX_PANES],
    spans: [Option<[f64; 2]>; MAX_PANES],
    history: &'a History,
    compare: Option<&'a cav_sim::InstrumentSpec>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// A fresh engine on the default scene (the five towers in their room), one pane, time zero.
    #[must_use]
    pub fn new() -> Self {
        let spec = SceneSpec::room();
        let views = std::array::from_fn(|pane| home_view(&spec, pane, DrawStyle::Shaded));
        let mut log = Log::new(LOG_CAPACITY);
        log.push(
            Level::Info,
            Subsystem::Engine,
            format!("cavendish {} · scene `{}`", crate::VERSION, spec.name),
        );
        let mut forward = Forward::new();
        forward.schedule(&spec, false);
        forward.request_reading(&spec, 0.0);
        Self {
            spec,
            time: 0.0,
            selection: None,
            panes: 1,
            active: 0,
            views,
            contents: [PaneContent::Scene; MAX_PANES],
            spans: [None; MAX_PANES],
            history: History::default(),
            journal: Vec::new(),
            journal_bytes: crate::journal::envelope(),
            journal_whole: true,
            journal_budget: JOURNAL_BYTES,
            journal_last: 0,
            unrecorded: None,
            landed: Vec::new(),
            refused: false,
            exports: Exports::AsNamed,
            drawer: Drawer::default(),
            window: std::sync::Mutex::new(None),
            prediction: std::sync::Mutex::new(None),
            log,
            log_mark: 0,
            generation: 0,
            forward,
            compare: None,
            patience: TYPED_LINE_PATIENCE,
            voxels: crate::voxels::Voxels::default(),
        }
    }

    /// A fresh engine whose undo history and journal hold at most the given bytes, in place of
    /// [`crate::HISTORY_BYTES`] and [`crate::JOURNAL_BYTES`]: for tests that fill them, which at
    /// the real sizes would take a hundred megabytes of scenes to do.
    #[doc(hidden)]
    #[must_use]
    pub fn with_budgets(history: usize, journal: usize) -> Self {
        let mut engine = Self::new();
        engine.history = History::with_budget(history);
        engine.journal_budget = journal;
        engine
    }

    /// A fresh engine with `journal` applied in order. Exports in the journal are applied and
    /// written nowhere: see [`Self::replay_into`].
    ///
    /// # Errors
    /// The first command that is refused, with its position; a journal the engine wrote never fails.
    pub fn replay(journal: &[Command]) -> Result<Self, String> {
        Self::replay_with(journal, Exports::Skipped, TYPED_LINE_PATIENCE)
    }

    /// A fresh engine with `journal` applied in order, its exports written into `directory`
    /// under their file names alone. A journal is someone's record of where they wrote; replaying
    /// it must not write there.
    ///
    /// # Errors
    /// As [`Self::replay`], and when an export cannot be written.
    pub fn replay_into(journal: &[Command], directory: &std::path::Path) -> Result<Self, String> {
        Self::replay_into_within(journal, directory, REPLAY_PATIENCE)
    }

    /// [`Self::replay_into`], waiting at most `patience` for the signal each export is written
    /// from. A replay is a batch: nobody is typing, so it waits far longer than a typed line.
    ///
    /// # Errors
    /// As [`Self::replay_into`]; an export whose signal is not refined within `patience` is a
    /// refusal, and nothing of it is written.
    pub fn replay_into_within(
        journal: &[Command],
        directory: &std::path::Path,
        patience: std::time::Duration,
    ) -> Result<Self, String> {
        Self::replay_with(journal, Exports::Into(directory.to_path_buf()), patience)
    }

    fn replay_with(
        journal: &[Command],
        exports: Exports,
        patience: std::time::Duration,
    ) -> Result<Self, String> {
        let mut engine = Self::new();
        engine.exports = exports;
        engine.patience = patience;
        for (n, command) in journal.iter().enumerate() {
            engine.apply(command.clone()).map_err(|why| format!("replay: command {n}: {why}"))?;
        }
        // The replayed engine goes on as a session of its own would.
        engine.set_exports(Exports::AsNamed);
        engine.patience = TYPED_LINE_PATIENCE;
        Ok(engine)
    }

    /// Says where this engine's exports go from now on. Which paths have landed where
    /// ([`Exports::Into`]) starts again.
    pub fn set_exports(&mut self, exports: Exports) {
        self.exports = exports;
        self.landed.clear();
    }

    /// Installs what draws this engine's figures. Without one, a figure export is refused.
    pub fn set_figure_exporter(&mut self, exporter: FigureExporter) {
        self.drawer = Drawer(Some(exporter));
    }

    /// Whether a figure exporter is installed.
    #[must_use]
    pub const fn draws_figures(&self) -> bool {
        self.drawer.0.is_some()
    }

    /// Applies a command. On success the state has changed, the command is in the journal and the
    /// log says what was done. On refusal nothing has changed except the log, which has the reason.
    ///
    /// # Errors
    /// Why the command was refused.
    pub fn apply(&mut self, command: Command) -> Result<(), String> {
        let described = console::describe(&command, self.active);
        // A bundle is of finished numbers: the signal is waited for, as long as this client
        // waits for anything (a tenth of a second in the application, which is then told to
        // try again; a minute or more from the command line).
        let of_the_signal = match &command {
            Command::Export { what: crate::ExportKind::Bundle, .. } => true,
            Command::Export { what: crate::ExportKind::Figure(figure), .. } => {
                figure.needs_signal() && self.draws_figures()
            }
            _ => false,
        };
        if of_the_signal && self.exports != Exports::Skipped {
            self.settle(self.patience);
        }
        let prepared = match self.prepare(&command) {
            Ok(prepared) => prepared,
            Err(why) => {
                self.note(Level::Error, Subsystem::Engine, format!("refused `{described}`: {why}"));
                self.refused = true;
                return Err(why);
            }
        };
        // A replay's export may land in a folder the journal names: it is made under the
        // replay's directory, where the session had it already.
        if matches!(self.exports, Exports::Into(_))
            && let Some(folder) = prepared.path().and_then(std::path::Path::parent)
            && let Err(why) = crate::files::create_dir_all(folder)
        {
            let why = format!("{}: not made: {why}", folder.display());
            self.note(Level::Error, Subsystem::Engine, format!("refused `{described}`: {why}"));
            self.refused = true;
            return Err(why);
        }
        // A file is written before anything is recorded: an export that fails is a refusal.
        let mut wrote = None;
        if let Prepared::Write(path, bytes) = &prepared {
            if let Err(why) = crate::files::write(path, bytes) {
                let why = format!("{}: not written: {why}", path.display());
                self.note(Level::Error, Subsystem::Engine, format!("refused `{described}`: {why}"));
                self.refused = true;
                return Err(why);
            }
            wrote = Some(format!("{} ({} bytes)", path.display(), bytes.len()));
        }
        if let Prepared::Bundle(directory) = &prepared {
            let signal = self.forward.signal().expect("prepare saw a refined signal");
            // A bundle is of the scene's own instrument. With a comparison on, the manifest and
            // the log say that the other reading is not in it.
            let compared = self.comparison().map(compared_name);
            match cav_sim::bundle::write(directory, &self.spec, signal, crate::build(), compared) {
                Ok(files) => {
                    let also = compared.map_or_else(String::new, |name| {
                        format!(", of the scene's own instrument: the comparison with {name} is not in it")
                    });
                    wrote = Some(format!("{} ({} files{also})", directory.display(), files.len()));
                }
                Err(why) => {
                    self.note(Level::Error, Subsystem::Engine, format!("refused `{described}`: {why}"));
                    self.refused = true;
                    return Err(why);
                }
            }
        }
        if let (Prepared::Figure(path), Command::Export { what: crate::ExportKind::Figure(figure), .. }) =
            (&prepared, &command)
        {
            let exporter = self.drawer.0.as_ref().expect("prepare saw an exporter");
            match exporter(self, figure, path) {
                Ok(files) => wrote = Some(format!("{} ({} files)", path.display(), files.len())),
                Err(why) => {
                    self.note(Level::Error, Subsystem::Engine, format!("refused `{described}`: {why}"));
                    self.refused = true;
                    return Err(why);
                }
            }
        }
        // A step of a drag continues the step before it. If that step was refused there is
        // nothing to continue, and this one is an edit of its own: it must not be folded into
        // whatever earlier set of the same path happens to be the journal's last entry.
        // It is journalled as one, too, so that a replay (which never sees the refusal) does
        // the same.
        let mut command = command;
        if std::mem::take(&mut self.refused)
            && let Command::Set { continues, .. } = &mut command
        {
            *continues = false;
        }
        let (level, subsystem) = match &command {
            Command::Load(_)
            | Command::Set { .. }
            | Command::Insert { .. }
            | Command::Remove { .. }
            | Command::Undo
            | Command::Redo => (Level::Info, Subsystem::Edit),
            Command::Run { .. } | Command::Export { .. } | Command::Compare(_) => {
                (Level::Info, Subsystem::Engine)
            }
            Command::Select(_) => (Level::Info, Subsystem::Engine),
            Command::Seek(_) => (Level::Debug, Subsystem::Engine),
            Command::View(_) => (Level::Debug, Subsystem::View),
        };
        // A replay's export: where it landed is kept, so that the same journal path lands there
        // again and another path does not; and the log says when it was moved aside for one.
        let mut aside = String::new();
        if let (Exports::Into(_), Command::Export { to, .. }, Some(_)) = (&self.exports, &command, &wrote)
            && let Ok((landing, moved)) = self.landing(to)
        {
            if moved {
                aside = format!(
                    " · as `{}`: an earlier export of this replay, of another path, has the plain name",
                    landing.display()
                );
            }
            if !self.landed.iter().any(|(named, _)| named == to) {
                self.landed.push((to.clone(), landing));
            }
        }
        let message = match (&command, &prepared) {
            (Command::Load(spec), _) => format!("{described} · {}", summary(spec)),
            (Command::Export { .. }, _) => match (&wrote, &self.exports) {
                (Some(wrote), _) => format!("{described} · wrote {wrote}{aside}"),
                _ => format!("{described} · not written (a replay with nowhere for exports to go)"),
            },
            _ => described,
        };
        // A run of commands that overwrite one another (a scrub, a drag) is one journal entry and
        // one log line: the last. For an edit it is also one step of undo: the first of the run.
        // The run is told by the command before this one: the journal's last entry while the
        // journal records, and the last command applied once it has stopped. (The journal's last
        // entry then no longer moves; compared with it, a seek long after the journal stopped
        // was taken for a step of a scrub that ended there, and rewrote the entry.)
        let previous = if self.journal_whole { self.journal.last() } else { self.unrecorded.as_ref() };
        let overwrites = previous.is_some_and(|previous| command.overwrites(previous));
        // A run of sets is journalled as its last step, standing where its first stood: it
        // continues what the first continued, which for a drag is nothing. (Kept as the last
        // step's own flag, a drag replayed after an edit of the same path merged into it.)
        let mut command = command;
        if overwrites
            && let (Some(Command::Set { continues: first, .. }), Command::Set { continues, .. }) =
                (previous, &mut command)
        {
            *continues = *first;
        }
        self.perform(&command, prepared, overwrites);
        self.generation += 1;
        // What the panes draw of voxel bodies follows the scene, the playhead and the views. What
        // it could not draw is said once, for the scene that has it (a refusal of a tree).
        let voxel_notes = self.voxels.refresh(&self.spec, self.time, &self.views[..self.panes]);
        // Whatever changed the scene or moved the playhead changes what the towers read under it.
        if !matches!(command, Command::Select(_) | Command::View(_)) {
            self.forward.request_reading(&self.spec, self.time);
        }
        if overwrites && self.log.total() == self.log_mark {
            self.log.amend_last(message);
        } else {
            self.log.push(level, subsystem, message);
        }
        for note in voxel_notes {
            self.log.push(Level::Warn, Subsystem::Render, note);
        }
        // The journal grows with the session, so it has a size: the bytes of the file it would
        // be written as. A command that would take it past that is still applied; what stops is
        // the record, and it says so rather than replay as if whole.
        let weight = weigh(&command);
        let replaced = if overwrites { self.journal_last } else { 0 };
        if !self.journal_whole {
            self.unrecorded = Some(command);
        } else if self.journal_bytes - replaced + weight > self.journal_budget {
            self.journal_whole = false;
            self.unrecorded = Some(command);
            self.log.push(
                Level::Error,
                Subsystem::Engine,
                format!(
                    "the journal has reached its {} KiB and records no further commands: this session can \
                     no longer be replayed from it. `journal restart` starts a new journal from the session \
                     as it stands",
                    self.journal_budget >> 10
                ),
            );
        } else {
            self.journal_bytes = self.journal_bytes - replaced + weight;
            self.journal_last = weight;
            if overwrites {
                *self.journal.last_mut().expect("there is a previous command") = command;
            } else {
                self.journal.push(command);
            }
        }
        self.log_mark = self.log.total();
        Ok(())
    }

    /// Starts the journal again from the session as it stands, and answers with how many
    /// commands the new journal opens with. The way on when the journal has stopped recording
    /// ([`crate::JOURNAL_BYTES`]), and allowed at any time.
    ///
    /// The new journal opens with the commands that make this state on a fresh engine: a load of
    /// the scene as it is, the comparison if one is on, the panes with what each shows and its
    /// camera, the playhead and the selection. The engine then *is* a fresh engine with those
    /// commands applied, so the new journal replays to it exactly. What that costs: the undo
    /// history starts again (the edits before this can no longer be undone), and the signal is
    /// computed again. The log, the figure exporter and where exports go are kept.
    ///
    /// # Errors
    /// When the scene alone is larger than the journal's budget; nothing has changed.
    pub fn restart_journal(&mut self) -> Result<usize, String> {
        let mut fresh = Self::with_budgets(self.history.budget(), self.journal_budget);
        for command in self.standing() {
            fresh.apply(command).map_err(|why| format!("the journal was not started again: {why}"))?;
        }
        if !fresh.journal_whole {
            return Err(format!(
                "the journal was not started again: the session as it stands is larger than the journal's {} KiB",
                self.journal_budget >> 10
            ));
        }
        // What is not state goes with the engine: its log, what draws its figures, where its
        // exports go and how long it waits.
        std::mem::swap(&mut fresh.log, &mut self.log);
        fresh.drawer = std::mem::take(&mut self.drawer);
        fresh.exports = self.exports.clone();
        fresh.landed = std::mem::take(&mut self.landed);
        fresh.patience = self.patience;
        fresh.generation = self.generation + 1;
        let commands = fresh.journal.len();
        *self = fresh;
        self.log.push(
            Level::Info,
            Subsystem::Engine,
            format!(
                "journal restart · a new journal of {commands} commands that make the session as it stands; \
                 the undo history starts again"
            ),
        );
        self.log_mark = self.log.total();
        Ok(commands)
    }

    /// The commands that make this engine's state on a fresh engine, the undo history apart.
    fn standing(&self) -> Vec<Command> {
        let mut commands = vec![Command::load(self.spec.clone())];
        if let Some(instrument) = self.comparison() {
            commands.push(Command::Compare(Some(Box::new(instrument.clone()))));
        }
        let view = |change: ViewCommand| Command::View(change);
        commands.push(view(ViewCommand::Panes(self.panes)));
        for (pane, camera) in self.views.iter().enumerate() {
            commands.extend([
                // The projection and what is shown first: each may move the camera or forget
                // the span, which are then put where they were.
                view(ViewCommand::Project { pane, projection: camera.projection }),
                view(ViewCommand::Style { pane, style: camera.style }),
                view(ViewCommand::Show { pane, content: self.contents[pane] }),
                view(ViewCommand::Span { pane, span: self.spans[pane] }),
                view(ViewCommand::Orbit { pane, yaw: camera.yaw, pitch: camera.pitch }),
                view(ViewCommand::Zoom { pane, distance: camera.distance }),
                view(ViewCommand::Pan { pane, target: camera.target }),
            ]);
        }
        commands.push(view(ViewCommand::Activate(self.active)));
        commands.push(Command::Seek(self.time));
        commands.push(Command::Select(self.selection));
        commands
    }

    /// Where under a replay's directory the export that a journal names `to` lands
    /// ([`Exports::Into`]), and whether it was moved aside for an earlier export of another path.
    fn landing(&self, to: &str) -> Result<(std::path::PathBuf, bool), String> {
        if let Some((_, landing)) = self.landed.iter().find(|(named, _)| named == to) {
            return Ok((landing.clone(), false));
        }
        // Both separators, on every platform: a journal lands the same way wherever it is replayed.
        let parts: Vec<&str> = to.split(['/', '\\']).collect();
        let outside = to.starts_with(['/', '\\'])
            || parts.first().is_some_and(|first| first.contains(':'))
            || parts.contains(&"..");
        let mut kept: Vec<&str> =
            parts.iter().copied().filter(|part| !part.is_empty() && *part != "." && *part != "..").collect();
        if outside {
            // A path that is absolute or climbs: its file name alone.
            kept.drain(..kept.len().saturating_sub(1));
        }
        let Some((name, folders)) = kept.split_last() else {
            return Err(format!("`{to}` names no file"));
        };
        let folder: std::path::PathBuf = folders.iter().collect();
        // Names are compared as Windows compares them, on every platform.
        let taken = |candidate: &std::path::Path| {
            let key = candidate.to_string_lossy().to_lowercase().replace('\\', "/");
            self.landed
                .iter()
                .any(|(_, landing)| landing.to_string_lossy().to_lowercase().replace('\\', "/") == key)
        };
        let plain = folder.join(name);
        if !taken(&plain) {
            return Ok((plain, false));
        }
        // `b.ron` is another export's: `b (2).ron`, `b (3).ron`, … the first that is free. There
        // is one, since only as many are taken as there have been exports.
        let (stem, extension) = match name.rsplit_once('.') {
            Some((stem, extension)) if !stem.is_empty() => (stem, format!(".{extension}")),
            _ => (*name, String::new()),
        };
        let free = (2..)
            .map(|n| folder.join(format!("{stem} ({n}){extension}")))
            .find(|candidate| !taken(candidate))
            .expect("fewer names are taken than there are numbers");
        Ok((free, true))
    }

    /// Works out what the command will do to the current state, or why it cannot be applied;
    /// changes nothing. An edit's scene is made here, once, and handed to [`Self::perform`].
    fn prepare(&self, command: &Command) -> Result<Prepared, String> {
        let edit =
            |spec: SceneSpec, undo: Edit, redo: Edit| Ok(Prepared::Edit(Box::new(spec), Step { undo, redo }));
        match command {
            Command::Load(spec) => {
                spec.validate().map_err(|e| e.0)?;
                edit(
                    (**spec).clone(),
                    Edit::Replace(Box::new(self.spec.clone())),
                    Edit::Replace(spec.clone()),
                )
            }
            Command::Set { path, value, .. } => {
                let (spec, before) = self.spec.edited(path, value)?;
                edit(
                    spec,
                    Edit::Put { path: path.clone(), value: before },
                    Edit::Put { path: path.clone(), value: value.clone() },
                )
            }
            Command::Insert { path, index, value } => {
                let (spec, at) = self.spec.inserted(path, *index, value)?;
                edit(
                    spec,
                    Edit::Remove { path: format!("{path}[{at}]") },
                    Edit::Insert { list: path.clone(), index: at, value: value.clone() },
                )
            }
            Command::Remove { path } => {
                let (spec, item) = self.spec.removed(path)?;
                // `removed` has checked that the path ends in an index of a list.
                let (list, index) = path.trim().rsplit_once('[').ok_or("not an item of a list")?;
                let index = index.trim_end_matches(']').parse().map_err(|_| "not an item of a list")?;
                edit(
                    spec,
                    Edit::Insert { list: list.to_owned(), index, value: item },
                    Edit::Remove { path: path.clone() },
                )
            }
            Command::Export { what, to } => self.prepare_export(what, to),
            Command::Run { .. } | Command::Compare(None) => Ok(Prepared::Plain),
            Command::Compare(Some(instrument)) => {
                // The scene must be one on that instrument too.
                self.compared_spec(instrument)
                    .validate()
                    .map(|()| Prepared::Plain)
                    .map_err(|e| format!("the scene cannot be read on that instrument: {}", e.0))
            }
            Command::Seek(t) if !t.is_finite() => Err(format!("a time must be finite, not {t}")),
            Command::Seek(_) => Ok(Prepared::Plain),
            Command::Select(None) => Ok(Prepared::Plain),
            Command::Select(Some(target)) => self.check_target(*target).map(|()| Prepared::Plain),
            Command::Undo => match self.history.last() {
                None => Err("nothing to undo".into()),
                Some(step) => step
                    .undo
                    .apply(&self.spec)
                    .map(|spec| Prepared::Restore(Box::new(spec)))
                    .map_err(|why| format!("the history no longer fits the scene: {why}")),
            },
            Command::Redo => match self.history.next() {
                None => Err("nothing to redo".into()),
                Some(step) => step
                    .redo
                    .apply(&self.spec)
                    .map(|spec| Prepared::Restore(Box::new(spec)))
                    .map_err(|why| format!("the history no longer fits the scene: {why}")),
            },
            Command::View(view) => self.check_view(view).map(|()| Prepared::Plain),
        }
    }

    /// The file an export writes and its bytes; `Plain` when exports are skipped.
    fn prepare_export(&self, what: &crate::ExportKind, to: &str) -> Result<Prepared, String> {
        if to.trim().is_empty() {
            return Err("an export needs a file to write".into());
        }
        if let crate::ExportKind::Figure(figure) = what {
            self.check_figure(figure, to)?;
        }
        let path = match &self.exports {
            Exports::Skipped => return Ok(Prepared::Plain),
            Exports::AsNamed => std::path::PathBuf::from(to),
            Exports::Into(directory) => directory.join(self.landing(to)?.0),
        };
        let bytes = match what {
            crate::ExportKind::Bundle if !crate::files::FOLDERS => {
                return Err(
                    "a bundle is a folder of files, which a browser page cannot write: export it from \
                            the native application or with `cav run`"
                        .into(),
                );
            }
            crate::ExportKind::Bundle => {
                // Never from a stale signal: the numbers would be another scene's.
                return match (self.forward.why(), self.forward.state(), self.forward.signal()) {
                    (Some(why), _, _) => Err(format!("the scene has no signal to bundle: {why}")),
                    (None, SignalState::Refined, Some(_)) => Ok(Prepared::Bundle(path)),
                    _ => Err("the signal is still being computed: a bundle is written only from a refined signal; \
                              try again when it is (the header says `signal · …`)"
                        .into()),
                };
            }
            crate::ExportKind::Figure(figure) => {
                if self.drawer.0.is_none() {
                    return Err("no figure exporter is installed in this client: the engine does not draw; \
                                `cav`, the MCP server and the application install cav-plot's"
                        .into());
                }
                if !figure.needs_signal() {
                    return Ok(Prepared::Figure(path));
                }
                // As a bundle: never from a stale signal.
                return match (self.forward.why(), self.forward.state(), self.forward.signal()) {
                    (Some(why), _, _) => Err(format!("the scene has no signal to draw: {why}")),
                    (None, SignalState::Refined, Some(_)) => Ok(Prepared::Figure(path)),
                    _ => Err(
                        "the signal is still being computed: a figure of it is written only from a refined \
                              signal; try again when it is (the header says `signal · …`)"
                            .into(),
                    ),
                };
            }
            crate::ExportKind::Scene => self.spec.to_ron().into_bytes(),
            crate::ExportKind::Journal => {
                if !self.journal_whole {
                    return Err("the journal stopped recording: it is not the whole session and is not written \
                                as one. `journal restart` starts a new journal from the session as it stands, \
                                which can then be saved"
                        .into());
                }
                crate::journal::write(&self.journal).into_bytes()
            }
        };
        Ok(Prepared::Write(path, bytes))
    }

    /// Whether a figure can be asked for at all: what the engine can say without drawing.
    fn check_figure(&self, figure: &crate::Figure, to: &str) -> Result<(), String> {
        use crate::{FigureKind, MAX_FIGURE_DPI, MAX_FIGURE_MM};
        if crate::FigureFormat::of_file(to) != Some(figure.format) {
            return Err(format!(
                "`{to}` is not the name of a .{} file: a figure's file ends in .svg, .pdf or .png, and is that format",
                figure.format.name()
            ));
        }
        if let Some(size) = figure.size_mm
            && !size.iter().all(|side| side.is_finite() && *side >= 10.0 && *side <= MAX_FIGURE_MM)
        {
            return Err(format!(
                "a figure of {} × {} mm cannot be made: each side is from 10 to {MAX_FIGURE_MM} mm",
                size[0], size[1]
            ));
        }
        if let Some(dpi) = figure.dpi
            && !(dpi.is_finite() && (10.0..=MAX_FIGURE_DPI).contains(&dpi))
        {
            return Err(format!("{dpi} is not a resolution: pixels per inch, from 10 to {MAX_FIGURE_DPI}"));
        }
        if let Some([from, to]) = figure.span
            && !(from.is_finite() && to.is_finite() && from < to)
        {
            return Err(format!("a span runs from a number to a larger one, not from {from} to {to}"));
        }
        let towers = self.spec.array.towers.len();
        if let Some(n) = figure.tower
            && (n == 0 || n > towers)
        {
            return Err(no_tower(n, towers));
        }
        if let Some(view) = figure.paper
            && view.finite().is_none()
        {
            return Err(format!("a section is cut at a place in metres, not at {}", view.words()));
        }
        if figure.what == FigureKind::View && figure.pane >= self.panes {
            return Err(format!("there is no pane {}; the figure has {}", figure.pane + 1, self.panes));
        }
        Ok(())
    }

    /// Everything a tower's spectral window is made from, as text: the timeline's length, the
    /// instrument, the schedule, the tower and its own timing, and where the window ends. Two
    /// scenes with the same key have the same shots at that tower.
    fn window_key(&self, tower: usize, to: f64) -> String {
        // The tower itself: its timing, and its own instrument if it has one.
        let own = self.spec.array.towers.get(tower.wrapping_sub(1));
        serde_json::json!([
            self.spec.duration,
            self.spec.array.instrument,
            self.spec.array.schedule,
            tower,
            own,
            to.to_bits(),
        ])
        .to_string()
    }

    /// Everything a cadence prediction is made from, as text: the timeline's length, the
    /// instrument, the schedule and how many towers there are (each loses its own shots). Not
    /// the towers' offsets: they move shots and neither add nor take any.
    fn prediction_key(&self) -> String {
        serde_json::json!([
            self.spec.duration,
            self.spec.array.instrument,
            self.spec.array.schedule,
            self.spec.array.towers.len(),
            // Towers of their own instruments have their own counts of shots.
            self.spec.array.towers.iter().map(|tower| &tower.instrument).collect::<Vec<_>>(),
        ])
        .to_string()
    }

    /// The shots of the scene as it is now: when each tower launches and reads out each one,
    /// and which it takes. They are made once an edit (`cav_sim::read::shots`) and kept, so
    /// asking costs nothing; whatever draws or counts shots goes by these.
    ///
    /// # Errors
    /// When the scene's signal is refused (too many shots, or more than the engine keeps), with
    /// the reason.
    pub fn shots(&self) -> Result<&cav_sim::read::Shots, &str> {
        self.forward.why().map_or_else(|| Ok(self.forward.shots()), Err)
    }

    fn check_target(&self, target: Target) -> Result<(), String> {
        let towers = self.spec.array.towers.len();
        match target {
            Target::Tower(n) if n == 0 || n > towers => Err(no_tower(n, towers)),
            Target::Room if self.spec.room.is_none() => Err("the scene has no room".into()),
            Target::Floor if self.spec.floor.is_none() => Err("the scene has no floor".into()),
            Target::Body(_) if self.spec.bodies.is_empty() => Err("the scene has no bodies".into()),
            Target::Body(n) if n == 0 || n > self.spec.bodies.len() => {
                Err(format!("there is no body {n}; the scene has bodies 1 to {}", self.spec.bodies.len()))
            }
            _ => Ok(()),
        }
    }

    fn check_view(&self, view: &ViewCommand) -> Result<(), String> {
        if let Some(pane) = view.pane()
            && pane >= MAX_PANES
        {
            return Err(format!("there is no pane {}; panes are 1 to {MAX_PANES}", pane + 1));
        }
        match *view {
            ViewCommand::Panes(n) if !matches!(n, 1 | 2 | 4) => {
                Err(format!("the figure splits into 1, 2 or 4 panes, not {n}"))
            }
            ViewCommand::Activate(pane) if pane >= self.panes => {
                Err(format!("pane {} is not shown; the figure has {} pane(s)", pane + 1, self.panes))
            }
            ViewCommand::Orbit { yaw, pitch, .. } if !(yaw.is_finite() && pitch.is_finite()) => {
                Err(format!("orbit angles must be finite, not {yaw} and {pitch}"))
            }
            ViewCommand::Zoom { distance, .. } if !(distance.is_finite() && distance > 0.0) => {
                Err(format!("a distance must be positive and finite, not {distance}"))
            }
            ViewCommand::Span { span: Some([from, to]), .. }
                if !(from.is_finite() && to.is_finite() && to > from) =>
            {
                Err(format!("a span runs from one value up to a larger one, not from {from} to {to}"))
            }
            ViewCommand::Pan { target, .. } if target.iter().any(|c| !c.is_finite()) => {
                Err(format!("a target must be finite, not {target:?}"))
            }
            _ => Ok(()),
        }
    }

    /// Carries out a command that [`Self::prepare`] has accepted. `continues` says that it
    /// overwrites the command before it: a drag in progress.
    fn perform(&mut self, command: &Command, prepared: Prepared, continues: bool) {
        match (command, prepared) {
            (Command::Load(_), Prepared::Edit(spec, step)) => {
                self.spec = *spec;
                self.history.edited(step);
                self.time = 0.0;
                self.selection = None;
                // Every pane keeps its direction, projection and style and re-frames the new scene.
                for view in &mut self.views {
                    view.frame(&self.spec);
                }
                self.forward.forget();
                if let Some((_, compared)) = &mut self.compare {
                    compared.forget();
                }
                self.schedule(false);
            }
            (_, Prepared::Edit(spec, step)) => {
                self.spec = *spec;
                if continues {
                    self.history.continued(step.redo);
                } else {
                    self.history.edited(step);
                }
                self.settle_after_history();
                self.schedule(false);
            }
            (Command::Undo, Prepared::Restore(spec)) => {
                self.spec = *spec;
                self.history.undone();
                self.settle_after_history();
                self.schedule(false);
            }
            (Command::Redo, Prepared::Restore(spec)) => {
                self.spec = *spec;
                self.history.redone();
                self.settle_after_history();
                self.schedule(false);
            }
            (Command::Run { .. }, _) => self.schedule(true),
            (Command::Compare(None), _) => self.compare = None,
            (Command::Compare(Some(instrument)), _) => {
                self.compare = Some((instrument.clone(), Forward::new()));
                self.schedule(false);
            }
            (Command::Seek(t), _) => self.time = t.clamp(0.0, self.spec.duration),
            (Command::Select(target), _) => self.selection = *target,
            (Command::View(view), _) => self.perform_view(view),
            // What is left changes nothing here, and is named so that a new command is a
            // compile error in this match until it is given its arm: an export wrote its file in
            // `apply`; an edit, an undo or a redo that prepared nothing to do has nothing to do.
            (
                Command::Load(_)
                | Command::Set { .. }
                | Command::Insert { .. }
                | Command::Remove { .. }
                | Command::Export { .. }
                | Command::Undo
                | Command::Redo,
                _,
            ) => {} // An export changed nothing here; the file was written before this was reached.
        }
    }

    /// The scene as it is read on `instrument`: everything the same but the array's instrument.
    /// A tower's own override stays in place on top of it: `compare aice` reads every tower that
    /// flies the array's instrument on AICE, and a tower with an instrument of its own on AICE
    /// with its own differences.
    fn compared_spec(&self, instrument: &cav_sim::InstrumentSpec) -> SceneSpec {
        let mut spec = self.spec.clone();
        spec.array.instrument = instrument.clone();
        spec
    }

    /// The scene has changed (or `fresh` asks for everything again): the forward stage, and the
    /// comparison's if there is one, work out what they need.
    fn schedule(&mut self, fresh: bool) {
        self.forward.schedule(&self.spec, fresh);
        if let Some((instrument, _)) = &self.compare {
            let spec = self.compared_spec(instrument);
            if let Some((_, compared)) = &mut self.compare {
                compared.schedule(&spec, fresh);
            }
        }
    }

    /// The instrument the scene is also being read on, if one is.
    #[must_use]
    pub fn comparison(&self) -> Option<&cav_sim::InstrumentSpec> {
        self.compare.as_ref().map(|(instrument, _)| &**instrument)
    }

    /// Where the comparison's signal stands, and the signal itself if there is a whole one.
    #[must_use]
    pub fn compared_signal(&self) -> (SignalState, Option<&std::sync::Arc<cav_sim::read::Signal>>) {
        self.compare
            .as_ref()
            .map_or((SignalState::None, None), |(_, forward)| (forward.state(), forward.signal()))
    }

    /// After undo or redo the cameras stay where they are; the playhead and the selection are kept
    /// only as far as the restored scene allows.
    fn settle_after_history(&mut self) {
        self.time = self.time.clamp(0.0, self.spec.duration);
        if self.selection.is_some_and(|target| self.check_target(target).is_err()) {
            self.selection = None;
        }
    }

    fn perform_view(&mut self, view: &ViewCommand) {
        match *view {
            ViewCommand::Panes(n) => {
                self.panes = n;
                if self.active >= n {
                    self.active = 0;
                }
            }
            ViewCommand::Activate(pane) => self.active = pane,
            ViewCommand::Look { pane, view } => {
                // A named view is a view of the scene: it looks from there and frames what it sees.
                // (Keeping the last distance left a plan view half the height of the picture.)
                (self.views[pane].yaw, self.views[pane].pitch) = view.angles();
                self.views[pane].frame(&self.spec);
            }
            ViewCommand::Project { pane, projection } => {
                // A view that frames the scene goes on framing it in the other projection, which
                // needs another distance; a camera the user has moved is left where it is.
                let view = &mut self.views[pane];
                let mut framed = view.clone();
                framed.frame(&self.spec);
                let was_framed = framed == *view;
                view.projection = projection;
                if was_framed {
                    view.frame(&self.spec);
                }
            }
            ViewCommand::Style { pane, style } => self.views[pane].style = style,
            ViewCommand::Show { pane, content } => {
                // Another plot has another axis: the span does not carry over.
                if self.contents[pane] != content {
                    self.spans[pane] = None;
                }
                self.contents[pane] = content;
            }
            ViewCommand::Span { pane, span } => self.spans[pane] = span,
            ViewCommand::Orbit { pane, yaw, pitch } => {
                // Yaw is kept in (−180, 180]; pitch cannot pass the poles.
                let wrapped = 180.0 - (180.0 - yaw).rem_euclid(360.0);
                self.views[pane].yaw = wrapped;
                self.views[pane].pitch = pitch.clamp(-90.0, 90.0);
            }
            ViewCommand::Zoom { pane, distance } => {
                self.views[pane].distance = distance.clamp(DISTANCE_RANGE.0, DISTANCE_RANGE.1);
            }
            ViewCommand::Pan { pane, target } => self.views[pane].target = target,
            ViewCommand::Home { pane } => {
                // Home is the camera's: the style, the colouring, the overlays and the section
                // are kept.
                let view = &self.views[pane];
                let (colour, overlay, section) = (view.colour, view.overlay, view.section);
                self.views[pane] = ViewSpec {
                    colour,
                    overlay,
                    section,
                    ..home_view(&self.spec, pane, self.views[pane].style)
                };
            }
            ViewCommand::Colour { pane, colour } => self.views[pane].colour = colour,
            ViewCommand::Overlay { pane, overlay } => self.views[pane].overlay = overlay,
            ViewCommand::Section { pane, section } => self.views[pane].section = section,
        }
    }

    /// Answers a query. Queries change nothing and are not journalled.
    ///
    /// # Errors
    /// When the query asks for something that does not exist (a path, a pane).
    pub fn query(&self, query: &Query) -> Result<Reply, String> {
        match query {
            Query::Spec(path) => {
                let whole = self.spec.to_value();
                cav_sim::paths::get(&whole, path.as_deref().unwrap_or_default())
                    .map(|v| Reply::Spec(v.clone()))
            }
            Query::Schema(path) => {
                cav_sim::paths::schema_at(path.as_deref().unwrap_or_default()).map(Reply::Schema)
            }
            Query::Ls(path) => {
                cav_sim::paths::ls(&self.spec, path.as_deref().unwrap_or_default()).map(Reply::Entries)
            }
            Query::Stats => Ok(Reply::Stats(self.stats())),
            Query::Log(filter) => Ok(Reply::Log(self.log.select(filter))),
            Query::View(pane) => self
                .views
                .get(*pane)
                .map(|v| Reply::View(v.clone()))
                .ok_or_else(|| format!("there is no pane {}; panes are 1 to {MAX_PANES}", pane + 1)),
            Query::Journal if !self.journal_whole => Err(format!(
                "the journal stopped recording at {} KiB: it is not the whole session and is not offered as \
                 one. `journal restart` starts a new journal from the session as it stands",
                self.journal_budget >> 10
            )),
            Query::Journal => Ok(Reply::Journal(self.journal.clone())),
            Query::Signal { tower, channel, span, compared } => {
                let towers = self.spec.array.towers.len();
                if *tower == 0 || *tower > towers {
                    return Err(no_tower(*tower, towers));
                }
                // The scene's own reading, or the comparison's.
                let forward = match (&self.compare, compared) {
                    (_, false) => &self.forward,
                    (Some((_, forward)), true) => forward,
                    (None, true) => {
                        return Err(
                            "nothing is being compared: `compare aice` reads the scene on AICE as well"
                                .into(),
                        );
                    }
                };
                if let Some(why) = forward.why() {
                    return Err(why.to_owned());
                }
                let state = match (forward.state(), forward.signal()) {
                    (SignalState::Refined, Some(_)) => Standing::Refined,
                    (_, Some(_)) => Standing::Stale,
                    _ => Standing::Computing,
                };
                if let Some([from, to]) = span
                    && !(from.is_finite() && to.is_finite() && to >= from)
                {
                    return Err(format!(
                        "a span runs from one time up to a later one, not from {from} to {to}"
                    ));
                }
                // A stale signal may be an earlier scene's, with another number of towers.
                let signal = forward.signal().filter(|signal| *tower <= signal.towers);
                // The shots inside the span, by this tower's own readout times: they rise, so
                // the shots are one stretch.
                let offset = signal.map_or(0.0, |s| s.offsets[tower - 1]);
                let stretch = |signal: &cav_sim::read::Signal| match span {
                    None => 0..signal.count(tower - 1),
                    Some([from, to]) => {
                        let times = signal.times(tower - 1);
                        times.partition_point(|t| t + offset < *from)
                            ..times.partition_point(|t| t + offset <= *to)
                    }
                };
                Ok(Reply::Signal(SignalReply {
                    tower: *tower,
                    channel: *channel,
                    state: if signal.is_some() { state } else { Standing::Computing },
                    times: signal.map_or_else(Vec::new, |s| s.readouts(tower - 1)[stretch(s)].to_vec()),
                    taken: signal.map_or_else(Vec::new, |s| s.taken(tower - 1)[stretch(s)].to_vec()),
                    values: signal.map_or_else(Vec::new, |s| {
                        s.channel(tower - 1, channel.channel())[stretch(s)].to_vec()
                    }),
                    estimate: signal.map_or(0.0, |s| s.estimate),
                    settled: signal.is_none_or(|s| s.settled),
                    warnings: signal.map_or_else(Vec::new, |s| s.warnings.clone()),
                }))
            }
            Query::Periodogram { tower, channel, compared } => {
                let asked =
                    Query::Signal { tower: *tower, channel: *channel, span: None, compared: *compared };
                let Reply::Signal(signal) = self.query(&asked)? else {
                    unreachable!("a signal query answers with a signal")
                };
                // The tower's own instrument: on the comparison's, the compared instrument with
                // the tower's own override on top, as `compare` reads it.
                let instrument = match (&self.compare, compared) {
                    (Some((instrument, _)), true) => {
                        self.compared_spec(instrument).array.instrument_of(tower - 1)
                    }
                    _ => self.spec.array.instrument_of(tower - 1),
                };
                let schedule = &self.spec.array.schedule;
                let nyquist = 0.5 / schedule.mean_cycle(instrument.cycle);
                // The shots the tower took, at its own times.
                let (times, values): (Vec<f64>, Vec<f64>) = signal
                    .times
                    .iter()
                    .zip(&signal.values)
                    .zip(&signal.taken)
                    .filter(|(_, taken)| **taken)
                    .map(|((time, value), _)| (*time, *value))
                    .unzip();
                let even = schedule.is_uniform() && times.len() == signal.times.len();
                let (method, spectrum) = if even {
                    // Evenly spaced and all there: the classical periodogram, as in S1.
                    let spectrum = cav_core::spectrum::periodogram(
                        &values,
                        instrument.cycle,
                        cav_core::spectrum::Classical::SCRIPTS,
                    );
                    ("classical periodogram (FFT)", spectrum)
                } else if times.len() < 2 {
                    (
                        "too few shots",
                        cav_core::spectrum::Spectrum { frequencies: Vec::new(), power: Vec::new() },
                    )
                } else {
                    // Uneven or gapped: Lomb–Scargle on the shots there are, at multiples of one
                    // over the span up to the Nyquist frequency of the mean rate.
                    let span = times[times.len() - 1] - times[0] + schedule.mean_cycle(instrument.cycle);
                    let df = 1.0 / span;
                    let count =
                        ((nyquist / df).floor() as usize).clamp(1, cav_sim::schedule::MAX_FREQUENCIES - 1);
                    // The upper clamp cannot bind for a scene that is valid: the frequencies are
                    // half the shots, and the shots are held to `MAX_SHOTS`. It is there for the
                    // day one of the two constants moves, and this fails the build on that day
                    // so that the bound is looked at (S2's review, finding 8: no test could see
                    // the clamp, because no valid scene reaches it).
                    const _: () = assert!(cav_sim::read::MAX_SHOTS / 2 < cav_sim::schedule::MAX_FREQUENCIES);
                    let power = cav_core::spectrum::lomb_scargle_fast(&times, &values, df, count + 1);
                    let frequencies = (1..=count).map(|k| k as f64 * df).collect();
                    (
                        "Lomb–Scargle (non-uniform FFT)",
                        cav_core::spectrum::Spectrum { frequencies, power: power[1..].to_vec() },
                    )
                };
                let combs = self
                    .spec
                    .bodies
                    .iter()
                    .filter_map(|body| {
                        let fundamental = body.motion.frequency(instrument.gravity)?;
                        let harmonics = (1..=COMB_HARMONICS)
                            .map(|n| {
                                let frequency = f64::from(n) * fundamental;
                                Harmonic {
                                    n,
                                    frequency,
                                    appears_at: cav_core::spectrum::fold_frequency(frequency, nyquist),
                                    folded: frequency > nyquist,
                                }
                            })
                            .collect();
                        Some(Comb { label: body.label.clone(), fundamental, harmonics })
                    })
                    .collect();
                Ok(Reply::Periodogram(PeriodogramReply {
                    tower: *tower,
                    channel: *channel,
                    state: signal.state,
                    method: method.to_owned(),
                    shots: times.len(),
                    nyquist,
                    frequencies: spectrum.frequencies,
                    power: spectrum.power,
                    combs,
                }))
            }
            Query::Cadence => {
                // A pane and the timeline's caption ask every frame, and a prediction counts
                // every shot of every tower: the last answer is kept, under everything it was
                // made from (one answer: a count for each tower and a dozen lines).
                let key = self.prediction_key();
                let mut kept = self.prediction.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some((made_from, prediction)) = kept.as_ref()
                    && *made_from == key
                {
                    return Ok(Reply::Cadence(prediction.clone()));
                }
                let prediction = cav_sim::schedule::predict(&self.spec)?;
                *kept = Some((key, prediction.clone()));
                Ok(Reply::Cadence(prediction))
            }
            Query::Window { tower, to } => {
                let towers = self.spec.array.towers.len();
                if *tower == 0 || *tower > towers {
                    return Err(no_tower(*tower, towers));
                }
                // The mean cycle is the schedule's own arithmetic at the tower's own cycle: no
                // prediction is made for it.
                let cycle = self.spec.array.instrument_of(tower - 1).cycle;
                let mean_cycle = self.spec.array.schedule.mean_cycle(cycle);
                let to = to.unwrap_or(2.5 / mean_cycle);
                // A pane asks for the same window every frame: the last answer is kept, under
                // everything it was made from (one answer, about 20 kB).
                let key = self.window_key(*tower, to);
                let mut kept = self.window.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some((made_from, reply)) = kept.as_ref()
                    && *made_from == key
                {
                    return Ok(Reply::Window(reply.clone()));
                }
                let (frequencies, window) =
                    cav_sim::schedule::window(&self.spec, tower - 1, to, WINDOW_POINTS)?;
                let reply = WindowReply { tower: *tower, frequencies, window };
                *kept = Some((key, reply.clone()));
                Ok(Reply::Window(reply))
            }
            Query::Reading(time) => {
                let time = time.unwrap_or(self.time);
                if !time.is_finite() {
                    return Err(format!("a time must be finite, not {time}"));
                }
                let (state, held) = self.forward.reading();
                // The reading held is for the time it was asked for; one for another time is an
                // earlier answer, to be shown as such.
                let state = match (state, held) {
                    (_, None) => Standing::Computing,
                    (SignalState::Refined, Some(reading)) if reading.time == time => Standing::Refined,
                    _ => Standing::Stale,
                };
                let towers = held.map_or_else(Vec::new, |reading| {
                    let labels = self.spec.array.towers.iter().map(|t| t.label.clone());
                    labels
                        .zip(&reading.towers)
                        .map(|(label, phases)| TowerReading {
                            label,
                            lower: phases.lower,
                            upper: phases.upper,
                            gradiometer: phases.gradiometer,
                        })
                        .collect()
                });
                Ok(Reply::Reading(ReadingReply {
                    time: held.map_or(time, |reading| reading.time),
                    state,
                    towers,
                    estimate: held.map_or(0.0, |reading| reading.estimate),
                    settled: held.is_none_or(|reading| reading.settled),
                    warnings: held.map_or_else(Vec::new, |reading| reading.warnings.clone()),
                }))
            }
        }
    }

    /// Takes what the forward workers have finished. True when a new whole signal or a new reading
    /// has just arrived; a signal's arrival is said in the log, with anything it warns of. The
    /// application calls this every frame; nothing in it waits and nothing in it computes.
    pub fn poll(&mut self) -> bool {
        let arrived = self.forward.poll();
        let compared = self.compare.as_mut().is_some_and(|(_, forward)| forward.poll().0);
        if compared {
            self.generation += 1;
        }
        self.took(arrived) || compared
    }

    /// Waits, at most `patience`, for the signal and the reading to be the current scene's. For
    /// clients that print numbers (`cav exec`, MCP); the application polls instead. True when
    /// both are.
    pub fn settle(&mut self, patience: std::time::Duration) -> bool {
        let started = web_time::Instant::now();
        let arrived = self.forward.settle(patience);
        self.took(arrived);
        // The comparison has what is left of the patience. (Its reading is never asked for:
        // only its signal is waited on.)
        let mut compared = true;
        if let Some((_, forward)) = &mut self.compare {
            // Its own `settle` waits on its results as they come, with no fixed nap (and runs the
            // work itself where there are no threads, in a browser).
            forward.settle(patience.saturating_sub(started.elapsed()));
            compared = forward.state() != SignalState::Stale;
            self.generation += 1;
        }
        compared
            && self.forward.state() != SignalState::Stale
            && self.forward.reading().0 != SignalState::Stale
    }

    /// Waits, at most `patience`, for the reading alone to be the one asked for: a reading that
    /// is ready in milliseconds is not held up by a series that takes a minute. True when it is.
    pub fn settle_reading(&mut self, patience: std::time::Duration) -> bool {
        let arrived = self.forward.settle_reading(patience);
        self.took(arrived);
        self.forward.reading().0 != SignalState::Stale
    }

    /// Answers a query, first waiting (at most the engine's patience) for the numbers it asks
    /// for: the signal for a signal or a periodogram, the reading alone for a reading. A reading
    /// at a time other than the playhead is asked of the worker here, and the playhead's again
    /// afterwards. [`Self::query`] never waits and never asks: it answers with what there is.
    ///
    /// # Errors
    /// As [`Self::query`].
    pub fn ask(&mut self, query: &Query) -> Result<Reply, String> {
        match query {
            Query::Signal { .. } | Query::Periodogram { .. } => {
                self.settle(self.patience);
                self.query(query)
            }
            Query::Reading(None) => {
                self.settle_reading(self.patience);
                self.query(query)
            }
            Query::Reading(Some(time)) => {
                if !time.is_finite() {
                    return self.query(query);
                }
                self.ask_reading(*time);
                self.settle_reading(self.patience);
                let answer = self.query(query);
                self.ask_reading(self.time);
                answer
            }
            _ => self.query(query),
        }
    }

    /// Records what has arrived: a signal is announced, and either is a change of what is shown.
    fn took(&mut self, (signal, reading): (bool, bool)) -> bool {
        if signal {
            self.announce();
        }
        if reading {
            self.generation += 1;
        }
        signal || reading
    }

    /// Asks the reading worker for the reading of a shot read out at `time`, which need not be the
    /// playhead. The answer arrives by [`Self::poll`] or [`Self::settle`]; the next command that
    /// moves the playhead or edits the scene asks for the playhead's again.
    pub fn ask_reading(&mut self, time: f64) {
        self.forward.request_reading(&self.spec, time);
    }

    /// How long `run_line` waits for the numbers a line asks for ([`TYPED_LINE_PATIENCE`] unless
    /// set). `cav exec` and MCP set a minute.
    pub const fn set_patience(&mut self, patience: std::time::Duration) {
        self.patience = patience;
    }

    /// Holds the series worker alone before its next job, or lets it go on; readings still
    /// arrive. For tests that must say a reading did not wait for the series, without a clock.
    #[doc(hidden)]
    pub fn hold_series(&self, held: bool) {
        self.forward.hold_series(held);
    }

    /// Holds the forward workers before their next job, or lets them go on: for tests that must
    /// say what happened on the caller's thread without a clock.
    #[doc(hidden)]
    pub fn hold_workers(&self, held: bool) {
        self.forward.hold(held);
        if let Some((_, forward)) = &self.compare {
            forward.hold(held);
        }
    }

    /// Says in the log that a signal has arrived.
    fn announce(&mut self) {
        let Some(signal) = self.forward.signal().cloned() else { return };
        self.generation += 1;
        let took = self.forward.took.map_or(0.0, |d| d.as_secs_f64() * 1e3);
        // How long it took is this machine's business and changes from run to run: it is a
        // debug entry of its own, written first, so that the line a person reads (and a
        // screenshot shows in the footer) says the same thing every time.
        self.log.push(
            Level::Debug,
            Subsystem::Engine,
            format!("the signal was whole {took:.0} ms after the edit"),
        );
        self.log.push(
            Level::Info,
            Subsystem::Engine,
            format!(
                "signal: {} tower(s), {} shot(s) in all, quadrature {}",
                signal.towers,
                signal.clock.len(),
                estimate_text(signal.estimate, signal.settled)
            ),
        );
        for warning in &signal.warnings {
            self.log.push(Level::Warn, Subsystem::Engine, warning.text.clone());
        }
    }

    /// Where the signal stands, and the signal itself if there is a whole one to show.
    #[must_use]
    pub fn signal(&self) -> (SignalState, Option<&std::sync::Arc<cav_sim::read::Signal>>) {
        (self.forward.state(), self.forward.signal())
    }

    /// How many contributions the forward worker has finished since the engine started: work
    /// that was dropped because the scene moved on is asked for and never finished.
    #[must_use]
    pub const fn contributions_finished(&self) -> u64 {
        self.forward.finished
    }

    /// How many contributions the forward worker has been asked for since the engine started.
    #[must_use]
    pub const fn contributions_computed(&self) -> u64 {
        self.forward.computed
    }

    /// Adds a line to the log on behalf of a client (the app, the renderer, the MCP server).
    pub fn note(&mut self, level: Level, subsystem: Subsystem, message: impl Into<String>) {
        self.log.push(level, subsystem, message);
    }

    /// Empties the log. The journal is untouched.
    pub fn clear_log(&mut self) {
        self.log.clear();
    }

    /// A digest of the whole state. Two engines with equal digests are in the same state: same
    /// scene, playhead, selection, panes, views and undo history, to the last bit of every number.
    #[must_use]
    pub fn digest(&self) -> String {
        let state = State {
            spec: &self.spec,
            time: self.time,
            selection: self.selection,
            panes: self.panes,
            active: self.active,
            views: &self.views,
            contents: self.contents,
            spans: self.spans,
            history: &self.history,
            compare: self.comparison(),
        };
        // serde_json writes the shortest decimal that reads back to the same f64, so distinct
        // values give distinct text. FNV-1a over that text is the digest.
        let text = serde_json::to_string(&state).expect("the state is JSON");
        let hash = text
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
        format!("{hash:016x}")
    }

    fn stats(&self) -> Stats {
        Stats {
            engine: crate::VERSION.into(),
            scene: self.spec.name.clone(),
            towers: self.spec.array.towers.len(),
            bodies: self.spec.bodies.len(),
            room: self.spec.room.is_some(),
            floor: self.spec.floor.is_some(),
            time: self.time,
            duration: self.spec.duration,
            selection: self.selection,
            panes: self.panes,
            active_pane: self.active,
            journal: self.journal.len(),
            journal_whole: self.journal_whole,
            undo: self.history.depth().0,
            redo: self.history.depth().1,
            generation: self.generation,
            digest: self.digest(),
            signal: self.forward.state().name().to_owned(),
            compare: self.comparison().map(|instrument| compared_name(instrument).to_owned()),
        }
    }

    /// The current scene.
    #[must_use]
    pub const fn spec(&self) -> &SceneSpec {
        &self.spec
    }

    /// The playhead, seconds.
    #[must_use]
    pub const fn time(&self) -> f64 {
        self.time
    }

    /// What is selected.
    #[must_use]
    pub const fn selection(&self) -> Option<Target> {
        self.selection
    }

    /// How many panes the figure shows.
    #[must_use]
    pub const fn panes(&self) -> usize {
        self.panes
    }

    /// The pane that keys and unaddressed commands act on, from 0.
    #[must_use]
    pub const fn active_pane(&self) -> usize {
        self.active
    }

    /// A pane's view. Panes beyond the shown count keep their views for when they return.
    ///
    /// # Panics
    /// When `pane` is not below [`MAX_PANES`].
    #[must_use]
    pub fn view(&self, pane: usize) -> &ViewSpec {
        &self.views[pane]
    }

    /// Whether pane `pane` shows the scene or a plot.
    ///
    /// # Panics
    /// When `pane` is not below [`MAX_PANES`].
    #[must_use]
    pub const fn content(&self, pane: usize) -> PaneContent {
        self.contents[pane]
    }

    /// What pane `pane` shows, for a renderer to draw: the scene, the pane's view, the selection
    /// and the playhead, together. Every client that draws (the app's panes, `cav render`, MCP
    /// `render_view`) asks here, so their pictures are of the same thing.
    ///
    /// # Errors
    /// When there is no such pane.
    pub fn frame(&self, pane: usize) -> Result<SceneView<'_>, String> {
        let view = self
            .views
            .get(pane)
            .ok_or_else(|| format!("there is no pane {}; panes are 1 to {MAX_PANES}", pane + 1))?;
        Ok(SceneView {
            scene: &self.spec,
            view,
            highlight: self.selection,
            time: self.time,
            content: self.contents[pane],
            span: self.spans[pane],
            voxels: self.voxels.frame(pane),
        })
    }

    /// The journal: every applied command, oldest first, runs of overwrites kept as their last.
    #[must_use]
    pub fn journal(&self) -> &[Command] {
        &self.journal
    }

    /// The event log.
    #[must_use]
    pub const fn log(&self) -> &Log {
        &self.log
    }

    /// Rises by one with every applied command: a cheap "has anything changed?".
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Whether there is an edit to undo, and one to redo.
    #[must_use]
    pub fn history(&self) -> (bool, bool) {
        let (undo, redo) = self.history.depth();
        (undo > 0, redo > 0)
    }

    /// The bytes the undo history and the journal are counted as holding, and whether the journal
    /// still records every command: each is held to its budget ([`crate::history`]).
    #[must_use]
    pub const fn held(&self) -> (usize, usize, bool) {
        (self.history.bytes(), self.journal_bytes, self.journal_whole)
    }

    /// The bytes of contributions the forward stage keeps.
    #[must_use]
    pub const fn forward_bytes(&self) -> usize {
        self.forward.kept_bytes()
    }
}

/// A pane's home view: the first pane is the isometric perspective; the others are the plan, the
/// front and the side, square-on and orthographic, as a drawing's three views.
fn home_view(spec: &SceneSpec, pane: usize, style: DrawStyle) -> ViewSpec {
    let (named, projection) = match pane {
        0 => (NamedView::Iso, Projection::Perspective),
        1 => (NamedView::Top, Projection::Orthographic),
        2 => (NamedView::South, Projection::Orthographic),
        _ => (NamedView::East, Projection::Orthographic),
    };
    ViewSpec::framing(spec, named, projection, style)
}

/// One line on what a scene holds.
fn summary(spec: &SceneSpec) -> String {
    let towers = spec.array.towers.len();
    let room = if spec.room.is_some() { "a room" } else { "no room" };
    let floor = if spec.floor.is_some() { "a floor" } else { "no floor" };
    let bodies = match spec.bodies.len() {
        0 => "no bodies".to_owned(),
        1 => "1 body".to_owned(),
        n => format!("{n} bodies"),
    };
    format!("{towers} tower{}, {room}, {floor}, {bodies}", if towers == 1 { "" } else { "s" })
}

/// Why tower `n` (counted from 1) is not one of the scene's `towers`: said for a scene with none
/// as well as for one with too few.
fn no_tower(n: usize, towers: usize) -> String {
    if towers == 0 {
        format!("there is no tower {n}: the scene has no towers")
    } else {
        format!("there is no tower {n}; the scene has towers 1 to {towers}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::LogFilter;
    use cav_sim::dev;

    /// A session in which every command matters to the final state, so that dropping any one of
    /// them must change the digest. (`Home` is left out because it overwrites what came before it;
    /// `loading_reframes_every_pane_and_home_restores_the_camera` covers it.)
    fn session() -> Vec<Command> {
        vec![
            Command::View(ViewCommand::Panes(4)),
            Command::View(ViewCommand::Activate(2)),
            Command::View(ViewCommand::Look { pane: 2, view: NamedView::West }),
            Command::load(dev::infinite_floor()),
            Command::View(ViewCommand::Project { pane: 0, projection: Projection::Orthographic }),
            Command::View(ViewCommand::Style { pane: 3, style: DrawStyle::Paper }),
            Command::load(dev::void()),
            Command::Undo,
            Command::Seek(7.0),
            Command::Undo,
            Command::Redo,
            Command::View(ViewCommand::Orbit { pane: 0, yaw: 200.0, pitch: 17.5 }),
            Command::View(ViewCommand::Zoom { pane: 0, distance: 0.1 + 0.2 }),
            Command::View(ViewCommand::Pan { pane: 1, target: [1.0 / 3.0, -2.0, 0.5] }),
            Command::Select(Some(Target::Floor)),
        ]
    }

    fn run(commands: &[Command]) -> Engine {
        let mut engine = Engine::new();
        for command in commands {
            engine.apply(command.clone()).unwrap_or_else(|why| panic!("{command:?}: {why}"));
        }
        engine
    }

    #[test]
    fn every_applied_command_is_journalled_and_logged() {
        let commands = session();
        let engine = run(&commands);
        assert_eq!(engine.journal(), commands.as_slice(), "nothing in this session coalesces");
        assert_eq!(engine.generation(), commands.len() as u64);
        // One log line per command, after the start-up line, each the command's console text.
        let lines: Vec<String> = engine.log().iter().skip(1).map(|e| e.message.clone()).collect();
        assert_eq!(lines.len(), commands.len());
        assert_eq!(lines[0], "panes 4");
        assert_eq!(lines[8], "seek 7");
        assert_eq!(lines[3], "load infinite-floor · 5 towers, no room, a floor, no bodies");
        // Control: a query is not a command; it leaves no trace.
        let before = (engine.journal().len(), engine.log().total(), engine.generation(), engine.digest());
        engine.query(&Query::Stats).unwrap();
        engine.query(&Query::Spec(Some("array.towers[0]".into()))).unwrap();
        assert_eq!(
            before,
            (engine.journal().len(), engine.log().total(), engine.generation(), engine.digest())
        );
    }

    #[test]
    fn session_replays_from_its_journal() {
        let mut original = run(&session());
        let mut replayed = Engine::replay(original.journal()).unwrap();
        // Where the signal stands is a matter of when one looks, not of the state: both are let
        // finish before the two are laid side by side.
        let patience = std::time::Duration::from_secs(60);
        assert!(original.settle(patience) && replayed.settle(patience));
        assert_eq!(replayed.digest(), original.digest());
        assert_eq!(replayed.spec(), original.spec());
        // The generation counts arrivals from the workers as well as commands (a reading that
        // comes in is a change of what is shown), so it is a clock of sorts and not state.
        let stats = |engine: &Engine| {
            let Ok(Reply::Stats(stats)) = engine.query(&Query::Stats) else { panic!("stats") };
            Stats { generation: 0, ..stats }
        };
        assert_eq!(stats(&replayed), stats(&original));
        // The journal survives a trip through JSON, as a saved session would.
        let saved = serde_json::to_string(original.journal()).unwrap();
        let loaded: Vec<Command> = serde_json::from_str(&saved).unwrap();
        assert_eq!(Engine::replay(&loaded).unwrap().digest(), original.digest());

        // Control arms: the digest sees every part of the state. Drop any one command from the
        // journal and the replay must differ (each command in the session matters to the end state).
        for skip in 0..original.journal().len() {
            let mut shorter = original.journal().to_vec();
            let dropped = shorter.remove(skip);
            let differs = Engine::replay(&shorter).map_or(true, |e| e.digest() != original.digest());
            assert!(differs, "dropping command {skip} ({dropped:?}) went unnoticed");
        }
        // And one unit in the last place of one camera angle is seen.
        // (The orbit is found in the journal, wherever a session with more commands puts it.)
        let mut nudged = original.journal().to_vec();
        let mut orbits = 0;
        for command in &mut nudged {
            if let Command::View(ViewCommand::Orbit { pitch, .. }) = command {
                *pitch = f64::from_bits(pitch.to_bits() + 1);
                orbits += 1;
                break;
            }
        }
        assert_eq!(orbits, 1, "the session has an orbit to nudge");
        assert_ne!(Engine::replay(&nudged).unwrap().digest(), original.digest());
    }

    #[test]
    fn repeated_seeks_and_orbits_coalesce_without_changing_the_replay() {
        let mut engine = Engine::new();
        let mut every_command = Vec::new();
        let mut apply = |engine: &mut Engine, command: Command| {
            every_command.push(command.clone());
            engine.apply(command).unwrap();
        };
        apply(&mut engine, Command::View(ViewCommand::Panes(2)));
        for step in 0..50 {
            apply(&mut engine, Command::Seek(f64::from(step) * 0.1));
        }
        for step in 0..50 {
            apply(
                &mut engine,
                Command::View(ViewCommand::Orbit { pane: 0, yaw: f64::from(step), pitch: 20.0 }),
            );
        }
        // A different pane does not coalesce with the run before it.
        apply(&mut engine, Command::View(ViewCommand::Orbit { pane: 1, yaw: 5.0, pitch: 5.0 }));
        apply(&mut engine, Command::Seek(1.0));
        assert_eq!(every_command.len(), 103);
        assert_eq!(engine.journal().len(), 5, "panes, one seek, one orbit per pane, one seek");
        assert_eq!(engine.generation(), 103, "every command still counted as a change");
        assert_eq!(engine.log().len(), 1 + 5, "and the log has one line per run");
        // The short journal and the full list of commands lead to the same state.
        let from_journal = Engine::replay(engine.journal()).unwrap();
        let from_everything = Engine::replay(&every_command).unwrap();
        assert_eq!(from_journal.digest(), engine.digest());
        assert_eq!(from_everything.digest(), engine.digest());
        // Control: coalescing is not blanket deduplication. A command that does not merely
        // overwrite (a relative one would not; here, one separated by another command) is kept.
        let separated = run(&[Command::Seek(1.0), Command::Select(None), Command::Seek(2.0)]);
        assert_eq!(separated.journal().len(), 3);
    }

    #[test]
    fn undo_and_redo_restore_the_spec() {
        let mut engine = Engine::new();
        let room = engine.spec().clone();
        engine.apply(Command::load(dev::void())).unwrap();
        engine.apply(Command::load(dev::infinite_floor())).unwrap();
        engine.apply(Command::Select(Some(Target::Floor))).unwrap();
        assert_eq!(engine.history(), (true, false));

        engine.apply(Command::Undo).unwrap();
        assert_eq!(engine.spec(), &dev::void());
        assert_eq!(engine.selection(), None, "the void has no floor to keep selected");
        engine.apply(Command::Undo).unwrap();
        assert_eq!(engine.spec(), &room);
        assert_eq!(engine.history(), (false, true));
        engine.apply(Command::Redo).unwrap();
        engine.apply(Command::Redo).unwrap();
        assert_eq!(engine.spec(), &dev::infinite_floor());

        // A new edit after an undo discards the redo branch.
        engine.apply(Command::Undo).unwrap();
        engine.apply(Command::load(room.clone())).unwrap();
        assert_eq!(engine.history(), (true, false));
        assert_eq!(engine.apply(Command::Redo), Err("nothing to redo".into()));

        // Control: undo is not a no-op that happens to pass. With nothing to undo it is refused.
        let mut fresh = Engine::new();
        assert_eq!(fresh.apply(Command::Undo), Err("nothing to undo".into()));
    }

    #[test]
    fn view_commands_do_not_touch_the_undo_history() {
        let mut engine = Engine::new();
        engine.apply(Command::load(dev::void())).unwrap();
        engine.apply(Command::View(ViewCommand::Look { pane: 0, view: NamedView::Top })).unwrap();
        engine.apply(Command::Seek(3.0)).unwrap();
        engine.apply(Command::Select(Some(Target::Tower(2)))).unwrap();
        let looked = engine.view(0).clone();
        // Undo takes back the load, not the look, the seek or the selection.
        engine.apply(Command::Undo).unwrap();
        assert_eq!(engine.spec(), &SceneSpec::room());
        assert_eq!(engine.view(0), &looked);
        assert_eq!(engine.time(), 3.0);
        assert_eq!(engine.selection(), Some(Target::Tower(2)));
        assert_eq!(engine.history(), (false, true), "one edit was made, so one undo was available");
    }

    #[test]
    fn a_refused_command_changes_nothing_and_logs_an_error() {
        let refused = [
            (Command::Select(Some(Target::Tower(6))), "there is no tower 6"),
            (Command::Select(Some(Target::Tower(0))), "there is no tower 0"),
            (Command::Select(Some(Target::Floor)), "the scene has no floor"),
            (Command::Select(Some(Target::Body(1))), "the scene has no bodies"),
            (Command::Seek(f64::NAN), "must be finite"),
            (Command::View(ViewCommand::Panes(3)), "1, 2 or 4"),
            (Command::View(ViewCommand::Activate(1)), "not shown"),
            (Command::View(ViewCommand::Home { pane: 4 }), "no pane 5"),
            (Command::View(ViewCommand::Zoom { pane: 0, distance: -1.0 }), "positive"),
            (Command::View(ViewCommand::Orbit { pane: 0, yaw: f64::INFINITY, pitch: 0.0 }), "finite"),
            (Command::load(SceneSpec { duration: -1.0, ..SceneSpec::room() }), "duration"),
            (Command::Redo, "nothing to redo"),
        ];
        let mut engine = run(&[Command::Seek(2.0), Command::Select(Some(Target::Room))]);
        let count = refused.len();
        for (command, expected) in refused {
            let before = (engine.digest(), engine.journal().len(), engine.generation());
            let why = engine.apply(command.clone()).expect_err("should be refused");
            assert!(why.contains(expected), "{command:?}: `{why}` should mention `{expected}`");
            assert_eq!(before, (engine.digest(), engine.journal().len(), engine.generation()), "{command:?}");
            let last = engine.log().iter().next_back().unwrap();
            assert_eq!(last.level, Level::Error);
            assert!(last.message.contains(&why), "{}", last.message);
        }
        let errors =
            engine.log().select(&LogFilter { min_level: Some(Level::Error), ..LogFilter::default() });
        // One error a refusal, however many refusals the list above has.
        assert_eq!(errors.len(), count);
        // Control: the same kinds of command with acceptable values are applied.
        assert!(engine.apply(Command::Select(Some(Target::Tower(5)))).is_ok());
        assert!(engine.apply(Command::View(ViewCommand::Zoom { pane: 0, distance: 1.0 })).is_ok());
    }

    #[test]
    fn values_are_clamped_where_the_command_says_so() {
        let mut engine = Engine::new();
        engine.apply(Command::Seek(1e9)).unwrap();
        assert_eq!(engine.time(), engine.spec().duration);
        engine.apply(Command::Seek(-3.0)).unwrap();
        assert_eq!(engine.time(), 0.0);
        engine.apply(Command::View(ViewCommand::Orbit { pane: 0, yaw: 270.0, pitch: 120.0 })).unwrap();
        assert_eq!((engine.view(0).yaw, engine.view(0).pitch), (-90.0, 90.0));
        engine.apply(Command::View(ViewCommand::Orbit { pane: 0, yaw: 180.0, pitch: -95.0 })).unwrap();
        assert_eq!((engine.view(0).yaw, engine.view(0).pitch), (180.0, -90.0));
        engine.apply(Command::View(ViewCommand::Zoom { pane: 0, distance: 1e-9 })).unwrap();
        assert_eq!(engine.view(0).distance, DISTANCE_RANGE.0);
    }

    #[test]
    fn loading_reframes_every_pane_and_home_restores_the_camera() {
        let mut engine = Engine::new();
        engine.apply(Command::View(ViewCommand::Style { pane: 0, style: DrawStyle::Paper })).unwrap();
        engine.apply(Command::View(ViewCommand::Look { pane: 0, view: NamedView::North })).unwrap();
        engine.apply(Command::View(ViewCommand::Zoom { pane: 0, distance: 3.0 })).unwrap();
        engine.apply(Command::load(dev::void())).unwrap();
        let view = engine.view(0);
        // The void is smaller than the room, so the framing distance shrinks; direction and style stay.
        assert_eq!(view.named(), Some(NamedView::North));
        assert_eq!(view.style, DrawStyle::Paper);
        let framed = |scene: &SceneSpec| {
            ViewSpec::framing(scene, NamedView::North, Projection::Perspective, DrawStyle::Paper)
        };
        assert_eq!(view, &framed(&dev::void()));
        assert!(view.distance < framed(&SceneSpec::room()).distance);
        engine.apply(Command::View(ViewCommand::Home { pane: 0 })).unwrap();
        assert_eq!(engine.view(0).named(), Some(NamedView::Iso));
        assert_eq!(engine.view(0).style, DrawStyle::Paper, "Home resets the camera, not the style");
        // The other panes' homes are the drawing's three views.
        assert_eq!(engine.view(1).named(), Some(NamedView::Top));
        assert_eq!(engine.view(2).named(), Some(NamedView::South));
        assert_eq!(engine.view(3).named(), Some(NamedView::East));
        assert_eq!(engine.view(3).projection, Projection::Orthographic);
    }

    #[test]
    fn spec_paths_reach_into_the_scene() {
        let engine = Engine::new();
        let get = |path: &str| engine.query(&Query::Spec(Some(path.into())));
        assert_eq!(get("array.towers[1].position[0]"), Ok(Reply::Spec(serde_json::json!(4.0))));
        assert_eq!(get("room.size"), Ok(Reply::Spec(serde_json::json!([10.0, 10.0, 3.0]))));
        assert_eq!(get("name"), Ok(Reply::Spec(serde_json::json!("room"))));
        assert_eq!(get("floor"), Ok(Reply::Spec(serde_json::Value::Null)));
        // Refusals say what is there instead.
        assert!(get("array.tower").unwrap_err().contains("it has: instrument, towers"));
        assert!(get("array.towers[5]").unwrap_err().contains("has 5 items"));
        assert!(get("name[0]").unwrap_err().contains("not a list"));
        assert!(get("array.towers[x]").unwrap_err().contains("not an index"));
        // `ls` and the schema at a path are the same walk, answered by the engine.
        let Ok(Reply::Entries(inside)) = engine.query(&Query::Ls(Some("array.instrument".into()))) else {
            panic!("entries")
        };
        assert_eq!(inside[0].path, "array.instrument.tower_height");
        assert_eq!(inside[0].unit.as_deref(), Some("m"));
        let Ok(Reply::Schema(schema)) = engine.query(&Query::Schema(Some("duration".into()))) else {
            panic!("a schema")
        };
        assert_eq!(schema["x-unit"], "s");
        assert!(engine.query(&Query::Ls(Some("array.nothing".into()))).is_err());
    }

    cav_validate::case! {
        test: validation_replay_reproduces_the_session,
        name: "engine.replay",
        subsystem: "engine",
        slice: "S0",
        reference: Exact,
        against: "a session replayed from its journal, and from its journal saved as JSON; replays whose state digest differs",
        unit: Count,
        tolerance: 0.0,
        measure: {
            let original = run(&session());
            let saved = serde_json::to_string(original.journal()).unwrap();
            let loaded: Vec<Command> = serde_json::from_str(&saved).unwrap();
            let replays = [Engine::replay(original.journal()).unwrap(), Engine::replay(&loaded).unwrap()];
            let differing = replays.iter().filter(|replay| replay.digest() != original.digest()).count();
            cav_validate::Measured::Error(differing as f64)
        },
    }

    #[test]
    fn the_frame_is_the_scene_the_view_the_selection_and_the_time() {
        let mut engine = Engine::new();
        engine.run_script("load infinite-floor; panes 2; view west @2; select tower 4; seek 3.25").unwrap();
        let frame = engine.frame(1).unwrap();
        assert_eq!(frame.scene, engine.spec());
        assert_eq!(frame.view, engine.view(1));
        assert_eq!(frame.view.named(), Some(NamedView::West));
        assert_eq!((frame.highlight, frame.time), (Some(Target::Tower(4)), 3.25));
        // The other pane has the same scene, selection and time, and its own camera.
        let other = engine.frame(0).unwrap();
        assert_eq!((other.scene, other.highlight, other.time), (frame.scene, frame.highlight, frame.time));
        assert_ne!(other.view, frame.view);
        // Control: it follows the engine. Change each part and the frame changes with it.
        engine.run_script("select none; seek 1; load void").unwrap();
        let after = engine.frame(1).unwrap();
        assert_eq!((after.scene.name.as_str(), after.highlight, after.time), ("void", None, 0.0));
        assert!(engine.frame(MAX_PANES).unwrap_err().contains("no pane 5"));
    }

    #[test]
    fn a_named_view_frames_the_scene() {
        let mut engine = Engine::new();
        // Zoomed in and panned away, then asked for the plan.
        engine.run_script("zoom 3; pan 40 40 0; view top; proj ortho").unwrap();
        // Framed as a plan, and still framed after the change of projection, whichever came first.
        let framed =
            ViewSpec::framing(engine.spec(), NamedView::Top, Projection::Orthographic, DrawStyle::Shaded);
        assert_eq!(engine.view(0), &framed, "the plan is framed, not left at the last zoom");
        let mut other_order = Engine::new();
        other_order.run_script("proj ortho; view top").unwrap();
        assert_eq!(other_order.view(0), &framed);
        // Control: commands that are not named views leave the camera where it is.
        engine.run_script("zoom 3; style flat; proj persp").unwrap();
        assert_eq!(engine.view(0).distance, 3.0);
        // And it frames for the pane's own projection.
        engine.run_script("proj ortho; view south").unwrap();
        let south =
            ViewSpec::framing(engine.spec(), NamedView::South, Projection::Orthographic, DrawStyle::Flat);
        assert_eq!(engine.view(0), &south);
    }
}
