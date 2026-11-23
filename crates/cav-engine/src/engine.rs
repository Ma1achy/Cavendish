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
    /// Into this directory, under each path's file name alone: a replay, which must never write
    /// where someone else's journal says.
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
    /// Whether the last command offered was refused: a step that says it continues the edit
    /// before it then has nothing to continue.
    refused: bool,
    /// Where exports are written.
    exports: Exports,
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
            journal_bytes: 0,
            journal_whole: true,
            journal_budget: JOURNAL_BYTES,
            refused: false,
            exports: Exports::AsNamed,
            log,
            log_mark: 0,
            generation: 0,
            forward,
            compare: None,
            patience: TYPED_LINE_PATIENCE,
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
        engine.exports = Exports::AsNamed;
        engine.patience = TYPED_LINE_PATIENCE;
        Ok(engine)
    }

    /// Says where this engine's exports go from now on.
    pub fn set_exports(&mut self, exports: Exports) {
        self.exports = exports;
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
        if matches!(command, Command::Export { what: crate::ExportKind::Bundle, .. })
            && self.exports != Exports::Skipped
        {
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
        // A file is written before anything is recorded: an export that fails is a refusal.
        let mut wrote = None;
        if let Prepared::Write(path, bytes) = &prepared {
            if let Err(why) = std::fs::write(path, bytes) {
                let why = format!("{}: not written: {why}", path.display());
                self.note(Level::Error, Subsystem::Engine, format!("refused `{described}`: {why}"));
                self.refused = true;
                return Err(why);
            }
            wrote = Some(format!("{} ({} bytes)", path.display(), bytes.len()));
        }
        if let Prepared::Bundle(directory) = &prepared {
            let signal = self.forward.signal().expect("prepare saw a refined signal");
            match cav_sim::bundle::write(directory, &self.spec, signal, crate::build()) {
                Ok(files) => wrote = Some(format!("{} ({} files)", directory.display(), files.len())),
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
        let message = match (&command, &prepared) {
            (Command::Load(spec), _) => format!("{described} · {}", summary(spec)),
            (Command::Export { .. }, _) => match (&wrote, &self.exports) {
                (Some(wrote), _) => format!("{described} · wrote {wrote}"),
                _ => format!("{described} · not written (a replay with nowhere for exports to go)"),
            },
            _ => described,
        };
        // A run of commands that overwrite one another (a scrub, a drag) is one journal entry and
        // one log line: the last. For an edit it is also one step of undo: the first of the run.
        let overwrites = self.journal.last().is_some_and(|previous| command.overwrites(previous));
        self.perform(&command, prepared, overwrites);
        self.generation += 1;
        // Whatever changed the scene or moved the playhead changes what the towers read under it.
        if !matches!(command, Command::Select(_) | Command::View(_)) {
            self.forward.request_reading(&self.spec, self.time);
        }
        let weight = weigh(&command);
        if overwrites {
            let last = self.journal.last_mut().expect("there is a previous command");
            self.journal_bytes = self.journal_bytes - weigh(last) + weight;
            // A run of sets is journalled as its last step, standing where its first stood: it
            // continues what the first continued, which for a drag is nothing. (Kept as the last
            // step's own flag, a drag replayed after an edit of the same path merged into it.)
            let mut command = command;
            if let (Command::Set { continues: first, .. }, Command::Set { continues, .. }) =
                (&*last, &mut command)
            {
                *continues = *first;
            }
            *last = command;
            if self.log.total() == self.log_mark {
                self.log.amend_last(message);
            } else {
                self.log.push(level, subsystem, message);
            }
        } else if self.journal_whole && self.journal_bytes + weight > self.journal_budget {
            // The journal grows with the session, so it has a size. Past it the command is still
            // applied; what stops is the record, and it says so rather than replay as if whole.
            self.journal_whole = false;
            self.log.push(level, subsystem, message);
            self.log.push(
                Level::Error,
                Subsystem::Engine,
                format!(
                    "the journal has reached its {} KiB and records no further commands: this session can \
                     no longer be replayed from it",
                    self.journal_budget >> 10
                ),
            );
        } else {
            if self.journal_whole {
                self.journal_bytes += weight;
                self.journal.push(command);
            }
            self.log.push(level, subsystem, message);
        }
        self.log_mark = self.log.total();
        Ok(())
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
            Command::Export { what, to } => self.prepare_export(*what, to),
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
    fn prepare_export(&self, what: crate::ExportKind, to: &str) -> Result<Prepared, String> {
        if to.trim().is_empty() {
            return Err("an export needs a file to write".into());
        }
        let path = match &self.exports {
            Exports::Skipped => return Ok(Prepared::Plain),
            Exports::AsNamed => std::path::PathBuf::from(to),
            Exports::Into(directory) => {
                let name =
                    std::path::Path::new(to).file_name().ok_or_else(|| format!("`{to}` names no file"))?;
                directory.join(name)
            }
        };
        let bytes = match what {
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
            crate::ExportKind::Scene => self.spec.to_ron().into_bytes(),
            crate::ExportKind::Journal => {
                if !self.journal_whole {
                    return Err("the journal stopped recording: it is not the whole session and is not written as one".into());
                }
                crate::journal::write(&self.journal).into_bytes()
            }
        };
        Ok(Prepared::Write(path, bytes))
    }

    fn check_target(&self, target: Target) -> Result<(), String> {
        let towers = self.spec.array.towers.len();
        match target {
            Target::Tower(n) if n == 0 || n > towers => {
                Err(format!("there is no tower {n}; the scene has towers 1 to {towers}"))
            }
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
            // An export changed nothing here; the file was written before this was reached.
            _ => {}
        }
    }

    /// The scene as it is read on `instrument`: everything the same but the instrument.
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
                self.views[pane] = home_view(&self.spec, pane, self.views[pane].style);
            }
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
                "the journal stopped recording at {} KiB: it is not the whole session and is not offered as one",
                self.journal_budget >> 10
            )),
            Query::Journal => Ok(Reply::Journal(self.journal.clone())),
            Query::Signal { tower, channel, span, compared } => {
                let towers = self.spec.array.towers.len();
                if *tower == 0 || *tower > towers {
                    return Err(format!("there is no tower {tower}; the scene has towers 1 to {towers}"));
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
                    None => 0..signal.times.len(),
                    Some([from, to]) => {
                        signal.times.partition_point(|t| t + offset < *from)
                            ..signal.times.partition_point(|t| t + offset <= *to)
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
                let instrument = match (&self.compare, compared) {
                    (Some((instrument, _)), true) => &**instrument,
                    _ => &self.spec.array.instrument,
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
            Query::Cadence => cav_sim::schedule::predict(&self.spec).map(Reply::Cadence),
            Query::Window { tower, to } => {
                let prediction = cav_sim::schedule::predict(&self.spec)?;
                if *tower == 0 {
                    return Err("towers are counted from 1".into());
                }
                let to = to.unwrap_or(2.5 / prediction.mean_cycle);
                let (frequencies, window) =
                    cav_sim::schedule::window(&self.spec, tower - 1, to, WINDOW_POINTS)?;
                Ok(Reply::Window(WindowReply { tower: *tower, frequencies, window }))
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
        let started = std::time::Instant::now();
        let arrived = self.forward.settle(patience);
        self.took(arrived);
        // The comparison has what is left of the patience. (Its reading is never asked for:
        // only its signal is waited on.)
        let mut compared = true;
        if let Some((_, forward)) = &mut self.compare {
            let left = patience.saturating_sub(started.elapsed());
            let deadline = std::time::Instant::now() + left;
            while forward.state() == SignalState::Stale && std::time::Instant::now() < deadline {
                forward.poll();
                if forward.state() == SignalState::Stale {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            }
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
        self.log.push(
            Level::Info,
            Subsystem::Engine,
            format!(
                "signal: {} tower(s) × {} shot(s), quadrature {}, whole {took:.0} ms after the edit",
                signal.towers,
                signal.times.len(),
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
            compare: self.comparison().map(|instrument| {
                cav_sim::presets::name_of(instrument).unwrap_or("another instrument").to_owned()
            }),
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
        let mut nudged = original.journal().to_vec();
        nudged[11] = Command::View(ViewCommand::Orbit {
            pane: 0,
            yaw: 200.0,
            pitch: f64::from_bits(17.5f64.to_bits() + 1),
        });
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
        assert_eq!(errors.len(), 12);
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
