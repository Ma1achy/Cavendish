//! The engine: owns the scene and the views, applies commands, answers queries.
//!
//! S0's engine has no stages to run, so it is a plain value with `&mut` methods and no worker;
//! the worker and the stage cache arrive with the first thing worth waiting for (S1's forward pass).
//! What is fixed already is the shape: state changes only through [`Engine::apply`], every applied
//! command is journalled, and the journal replayed on a fresh engine gives the same state.

use cav_sim::{DrawStyle, NamedView, Projection, SceneSpec, SceneView, Target, ViewSpec};
use serde::Serialize;

use crate::command::{
    COMB_HARMONICS, Comb, Command, Harmonic, MAX_PANES, PaneContent, PeriodogramReply, Query, ReadingReply,
    Reply, SignalReply, Stats, TowerReading, ViewCommand,
};
use crate::console;
use crate::forward::{Forward, SignalState};
use crate::log::{Level, Log, Subsystem};

/// How long a line typed at a console waits for the numbers it asks for, unless a client sets
/// otherwise: a tenth of a second, which a person does not see and a long computation outlasts
/// (the answer then says `stale` or `computing`). Nothing the application does every frame waits.
pub const TYPED_LINE_PATIENCE: std::time::Duration = std::time::Duration::from_millis(100);

/// A quadrature estimate as people read it.
#[must_use]
pub fn estimate_text(estimate: f64) -> String {
    if estimate == 0.0 {
        "exact".to_owned()
    } else if estimate.is_finite() {
        format!("within {estimate:.0e} rad")
    } else {
        "not settled".to_owned()
    }
}

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
    undo: Vec<SceneSpec>,
    redo: Vec<SceneSpec>,
    journal: Vec<Command>,
    log: Log,
    /// `log.total()` just after the last command's entry: tells whether that entry is still the newest.
    log_mark: u64,
    generation: u64,
    /// The forward stage: its worker, what it has kept, the last whole signal.
    forward: Forward,
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
    undo: &'a [SceneSpec],
    redo: &'a [SceneSpec],
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
            undo: Vec::new(),
            redo: Vec::new(),
            journal: Vec::new(),
            log,
            log_mark: 0,
            generation: 0,
            forward,
            patience: TYPED_LINE_PATIENCE,
        }
    }

    /// A fresh engine with `journal` applied in order.
    ///
    /// # Errors
    /// The first command that is refused, with its position; a journal the engine wrote never fails.
    pub fn replay(journal: &[Command]) -> Result<Self, String> {
        let mut engine = Self::new();
        for (n, command) in journal.iter().enumerate() {
            engine.apply(command.clone()).map_err(|why| format!("replay: command {n}: {why}"))?;
        }
        Ok(engine)
    }

    /// Applies a command. On success the state has changed, the command is in the journal and the
    /// log says what was done. On refusal nothing has changed except the log, which has the reason.
    ///
    /// # Errors
    /// Why the command was refused.
    pub fn apply(&mut self, command: Command) -> Result<(), String> {
        let described = console::describe(&command, self.active);
        if let Err(why) = self.check(&command) {
            self.note(Level::Error, Subsystem::Engine, format!("refused `{described}`: {why}"));
            return Err(why);
        }
        let (level, subsystem) = match &command {
            Command::Load(_) | Command::Set { .. } | Command::Undo | Command::Redo => {
                (Level::Info, Subsystem::Edit)
            }
            Command::Run { .. } => (Level::Info, Subsystem::Engine),
            Command::Select(_) => (Level::Info, Subsystem::Engine),
            Command::Seek(_) => (Level::Debug, Subsystem::Engine),
            Command::View(_) => (Level::Debug, Subsystem::View),
        };
        let message = match &command {
            Command::Load(spec) => format!("{described} · {}", summary(spec)),
            _ => described,
        };
        // A run of commands that overwrite one another (a scrub, a drag) is one journal entry and
        // one log line: the last. For an edit it is also one step of undo: the first of the run.
        let overwrites = self.journal.last().is_some_and(|previous| command.overwrites(previous));
        self.perform(&command, overwrites);
        self.generation += 1;
        // Whatever changed the scene or moved the playhead changes what the towers read under it.
        if !matches!(command, Command::Select(_) | Command::View(_)) {
            self.forward.request_reading(&self.spec, self.time);
        }
        if overwrites {
            *self.journal.last_mut().expect("there is a previous command") = command;
            if self.log.total() == self.log_mark {
                self.log.amend_last(message);
            } else {
                self.log.push(level, subsystem, message);
            }
        } else {
            self.journal.push(command);
            self.log.push(level, subsystem, message);
        }
        self.log_mark = self.log.total();
        Ok(())
    }

    /// Whether the command can be applied to the current state; changes nothing.
    fn check(&self, command: &Command) -> Result<(), String> {
        match command {
            Command::Load(spec) => spec.validate().map_err(|e| e.0),
            Command::Set { path, value, .. } => self.edited(path, value).map(|_| ()),
            Command::Run { .. } => Ok(()),
            Command::Seek(t) if !t.is_finite() => Err(format!("a time must be finite, not {t}")),
            Command::Seek(_) => Ok(()),
            Command::Select(None) => Ok(()),
            Command::Select(Some(target)) => self.check_target(*target),
            Command::Undo if self.undo.is_empty() => Err("nothing to undo".into()),
            Command::Redo if self.redo.is_empty() => Err("nothing to redo".into()),
            Command::Undo | Command::Redo => Ok(()),
            Command::View(view) => self.check_view(view),
        }
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
            ViewCommand::Pan { target, .. } if target.iter().any(|c| !c.is_finite()) => {
                Err(format!("a target must be finite, not {target:?}"))
            }
            _ => Ok(()),
        }
    }

    /// The scene with the value at `path` replaced by `value`, if that is a valid scene.
    fn edited(&self, path: &str, value: &serde_json::Value) -> Result<SceneSpec, String> {
        let mut whole = serde_json::to_value(&self.spec).expect("a scene spec is JSON");
        *at_path_mut(&mut whole, path)? = value.clone();
        let spec: SceneSpec =
            serde_json::from_value(whole).map_err(|e| format!("`{path}` cannot be set to {value}: {e}"))?;
        spec.validate().map_err(|e| e.0)?;
        Ok(spec)
    }

    /// Carries out a command that [`Self::check`] has accepted. `continues` says that it overwrites
    /// the command before it: a drag in progress.
    fn perform(&mut self, command: &Command, continues: bool) {
        match command {
            Command::Set { path, value, .. } => {
                let edited = self.edited(path, value).expect("checked");
                let before = std::mem::replace(&mut self.spec, edited);
                if !continues {
                    self.undo.push(before);
                }
                self.redo.clear();
                self.settle_after_history();
                self.forward.schedule(&self.spec, false);
            }
            Command::Run { .. } => self.forward.schedule(&self.spec, true),
            Command::Load(spec) => {
                self.undo.push(std::mem::replace(&mut self.spec, (**spec).clone()));
                self.redo.clear();
                self.time = 0.0;
                self.selection = None;
                // Every pane keeps its direction, projection and style and re-frames the new scene.
                for view in &mut self.views {
                    view.frame(&self.spec);
                }
                self.forward.forget();
                self.forward.schedule(&self.spec, false);
            }
            Command::Seek(t) => self.time = t.clamp(0.0, self.spec.duration),
            Command::Select(target) => self.selection = *target,
            Command::Undo => {
                let previous = self.undo.pop().expect("checked");
                self.redo.push(std::mem::replace(&mut self.spec, previous));
                self.settle_after_history();
                self.forward.schedule(&self.spec, false);
            }
            Command::Redo => {
                let next = self.redo.pop().expect("checked");
                self.undo.push(std::mem::replace(&mut self.spec, next));
                self.settle_after_history();
                self.forward.schedule(&self.spec, false);
            }
            Command::View(view) => self.perform_view(view),
        }
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
            ViewCommand::Show { pane, content } => self.contents[pane] = content,
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
                let whole = serde_json::to_value(&self.spec).expect("a scene spec is JSON");
                match path.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
                    None => Ok(Reply::Spec(whole)),
                    Some(path) => at_path(&whole, path).map(|v| Reply::Spec(v.clone())),
                }
            }
            Query::Schema => Ok(Reply::Schema(SceneSpec::schema())),
            Query::Stats => Ok(Reply::Stats(self.stats())),
            Query::Log(filter) => Ok(Reply::Log(self.log.select(filter))),
            Query::View(pane) => self
                .views
                .get(*pane)
                .map(|v| Reply::View(v.clone()))
                .ok_or_else(|| format!("there is no pane {}; panes are 1 to {MAX_PANES}", pane + 1)),
            Query::Journal => Ok(Reply::Journal(self.journal.clone())),
            Query::Signal { tower, channel } => {
                let towers = self.spec.array.towers.len();
                if *tower == 0 || *tower > towers {
                    return Err(format!("there is no tower {tower}; the scene has towers 1 to {towers}"));
                }
                if let Some(why) = self.forward.why() {
                    return Err(why.to_owned());
                }
                let state = match (self.forward.state(), self.forward.signal()) {
                    (SignalState::Refined, Some(_)) => "refined",
                    (_, Some(_)) => "stale",
                    _ => "computing",
                };
                // A stale signal may be an earlier scene's, with another number of towers.
                let signal = self.forward.signal().filter(|signal| *tower <= signal.towers);
                Ok(Reply::Signal(SignalReply {
                    tower: *tower,
                    channel: *channel,
                    state: if signal.is_some() { state } else { "computing" }.to_owned(),
                    times: signal.map_or_else(Vec::new, |s| s.times.clone()),
                    values: signal
                        .map_or_else(Vec::new, |s| s.channel(tower - 1, channel.channel()).to_vec()),
                    estimate: signal.map_or(0.0, |s| s.estimate),
                    warnings: signal.map_or_else(Vec::new, |s| s.warnings.clone()),
                }))
            }
            Query::Periodogram { tower, channel } => {
                let Reply::Signal(signal) =
                    self.query(&Query::Signal { tower: *tower, channel: *channel })?
                else {
                    unreachable!("a signal query answers with a signal")
                };
                let instrument = &self.spec.array.instrument;
                let nyquist = 0.5 / instrument.cycle;
                let spectrum = cav_core::spectrum::periodogram(
                    &signal.values,
                    instrument.cycle,
                    cav_core::spectrum::Classical::SCRIPTS,
                );
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
                    nyquist,
                    frequencies: spectrum.frequencies,
                    power: spectrum.power,
                    combs,
                }))
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
                    (_, None) => "computing",
                    (SignalState::Refined, Some(reading)) if reading.time == time => "refined",
                    _ => "stale",
                };
                let towers = held.map_or_else(Vec::new, |reading| {
                    let labels = self.spec.array.towers.iter().map(|t| t.label.clone());
                    labels
                        .zip(&reading.towers)
                        .map(|(label, &[lower, upper, gradiometer])| TowerReading {
                            label,
                            lower,
                            upper,
                            gradiometer,
                        })
                        .collect()
                });
                Ok(Reply::Reading(ReadingReply {
                    time: held.map_or(time, |reading| reading.time),
                    state: state.to_owned(),
                    towers,
                    estimate: held.map_or(0.0, |reading| reading.estimate),
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
        self.took(arrived)
    }

    /// Waits, at most `patience`, for the signal and the reading to be the current scene's. For
    /// clients that print numbers (`cav exec`, MCP); the application polls instead. True when
    /// both are.
    pub fn settle(&mut self, patience: std::time::Duration) -> bool {
        let arrived = self.forward.settle(patience);
        self.took(arrived);
        self.forward.state() != SignalState::Stale && self.forward.reading().0 != SignalState::Stale
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

    pub(crate) const fn patience(&self) -> std::time::Duration {
        self.patience
    }

    /// Holds the forward workers before their next job, or lets them go on: for tests that must
    /// say what happened on the caller's thread without a clock.
    #[doc(hidden)]
    pub fn hold_workers(&self, held: bool) {
        self.forward.hold(held);
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
                estimate_text(signal.estimate)
            ),
        );
        for warning in &signal.warnings {
            self.log.push(Level::Warn, Subsystem::Engine, warning.clone());
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
            undo: &self.undo,
            redo: &self.redo,
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
            undo: self.undo.len(),
            redo: self.redo.len(),
            generation: self.generation,
            digest: self.digest(),
            signal: self.forward.state().name().to_owned(),
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
        Ok(SceneView { scene: &self.spec, view, highlight: self.selection, time: self.time })
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
        (!self.undo.is_empty(), !self.redo.is_empty())
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

/// The place at a path inside a JSON value, to be written to. The last step may name a field
/// that is absent only if its parent is an object that already has it: nothing is created.
fn at_path_mut<'a>(root: &'a mut serde_json::Value, path: &str) -> Result<&'a mut serde_json::Value, String> {
    // Walk read-only first, for the same refusals as a `get`; then again to borrow mutably.
    at_path(root, path)?;
    let mut here = root;
    for segment in path.split('.') {
        let (name, indices) = segment.split_once('[').map_or((segment, ""), |(n, rest)| (n, rest));
        if !name.is_empty() {
            here = here.get_mut(name).expect("walked");
        }
        for index in indices.split('[').filter(|i| !i.is_empty()) {
            let n: usize = index.trim_end_matches(']').parse().expect("walked");
            here = here.get_mut(n).expect("walked");
        }
    }
    Ok(here)
}

/// The value at a path such as `array.towers[2].position[0]` inside a JSON value.
fn at_path<'a>(root: &'a serde_json::Value, path: &str) -> Result<&'a serde_json::Value, String> {
    let mut here = root;
    let mut walked = String::new();
    for segment in path.split('.') {
        let (name, indices) = segment.split_once('[').map_or((segment, ""), |(n, rest)| (n, rest));
        if !name.is_empty() {
            here = here.get(name).ok_or_else(|| {
                let fields: Vec<&str> =
                    here.as_object().map(|o| o.keys().map(String::as_str).collect()).unwrap_or_default();
                let at = if walked.is_empty() { "the scene".to_owned() } else { format!("`{walked}`") };
                format!("{at} has no field `{name}`; it has: {}", fields.join(", "))
            })?;
            if !walked.is_empty() {
                walked.push('.');
            }
            walked.push_str(name);
        }
        for index in indices.split('[').filter(|i| !i.is_empty()) {
            let number = index.strip_suffix(']').ok_or_else(|| format!("`{segment}`: a `[` needs a `]`"))?;
            let n: usize = number.parse().map_err(|_| format!("`{segment}`: `{number}` is not an index"))?;
            let length = here.as_array().map(Vec::len);
            here = here.get(n).ok_or_else(|| match length {
                Some(len) => format!("`{walked}` has {len} items; there is no [{n}]"),
                None => format!("`{walked}` is not a list"),
            })?;
            walked.push_str(&format!("[{n}]"));
        }
    }
    Ok(here)
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
        let original = run(&session());
        let replayed = Engine::replay(original.journal()).unwrap();
        assert_eq!(replayed.digest(), original.digest());
        assert_eq!(replayed.spec(), original.spec());
        assert_eq!(replayed.query(&Query::Stats).unwrap(), original.query(&Query::Stats).unwrap());
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
