//! The forward stage: every tower's signal, and the reading under the playhead, computed off the
//! caller's thread.
//!
//! The phase is a sum over sources, so each source's contribution is a job of its own and is kept
//! by the hash of what it depends on (the instrument, the towers, the timeline and that source).
//! Moving one body asks the worker for that body alone; the rest is already here. The signal is
//! the kept contributions summed in the scene's order, which is the order a cold run sums them
//! in, so the two are the same bits.
//!
//! Nothing here waits, and nothing here computes: [`Forward::schedule`] and
//! [`Forward::request_reading`] hand work over and return; [`Forward::poll`] takes whatever has
//! come back. Two workers, so that the one number under the playhead is not queued behind a long
//! series.
//!
//! **Nothing here grows with what is asked.** Work is handed over through a [`Slot`] that holds
//! one item: a newer request replaces one that has not been started, so no queue of superseded
//! scenes can build up behind a busy worker. The series worker checks the current epoch as it
//! goes and drops work for a scene that has since changed. The reading worker finishes the
//! reading it has begun (a drag that asks sixty times a second would otherwise never see one)
//! and then takes the latest. What is kept is counted in bytes and held to
//! [`FORWARD_STORE_BYTES`]; a scene whose own contributions would not fit is refused with the
//! numbers. Until a new result is whole, the last whole one stays in place, marked stale.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use cav_sim::SceneSpec;
use cav_sim::read::{self, Contribution, ReadingAt, Signal, SourceId};

/// The most bytes of contributions the forward stage keeps: the current scene's, and earlier
/// scenes' while there is room (a drag makes a new one at every step, and going back to an
/// earlier place finds it still here). Earlier scenes' are dropped first, oldest first; a scene
/// whose own contributions exceed this has no signal, and says why.
pub const FORWARD_STORE_BYTES: usize = 256 << 20;

/// The bytes one value of a contribution takes: three channels of `f64`.
const BYTES_PER_VALUE: usize = 3 * std::mem::size_of::<f64>();

/// Where a result of the forward stage stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalState {
    /// It is the current scene's.
    Refined,
    /// The scene has changed; what is shown is the last whole result, and work is under way.
    Stale,
    /// There is nothing to show: the scene cannot have one, or none has been asked for.
    None,
}

impl SignalState {
    /// The word used in replies.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Refined => "refined",
            Self::Stale => "stale",
            Self::None => "none",
        }
    }
}

/// A place for one item of work. Putting a second replaces the first: the latest wins, and what
/// is waiting never grows. The worker blocks in [`Slot::take`] until there is an item or the
/// slot is closed.
struct Slot<T> {
    held: Mutex<(Option<T>, bool)>,
    filled: Condvar,
}

impl<T> Slot<T> {
    fn new() -> Self {
        Self { held: Mutex::new((None, false)), filled: Condvar::new() }
    }

    fn put(&self, item: T) {
        self.held.lock().unwrap_or_else(PoisonError::into_inner).0 = Some(item);
        self.filled.notify_one();
    }

    /// The item, once there is one; `None` when the slot has been closed.
    fn take(&self) -> Option<T> {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if held.1 {
                return None;
            }
            if let Some(item) = held.0.take() {
                return Some(item);
            }
            held = self.filled.wait(held).unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn close(&self) {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        *held = (None, true);
        self.filled.notify_all();
    }
}

/// Everything the series worker is asked for at once: the scene, and the sources whose
/// contributions are not kept yet.
struct Batch {
    epoch: u64,
    spec: Arc<SceneSpec>,
    readouts: Arc<Vec<f64>>,
    work: Vec<(SourceId, u64)>,
}

struct Done {
    key: u64,
    contribution: Contribution,
}

struct ReadJob {
    epoch: u64,
    spec: Arc<SceneSpec>,
    time: f64,
}

/// A gate the workers pass before each job. Closed, they wait: a test can then say exactly what
/// the workers have and have not done, with no clock in it.
#[derive(Default)]
struct Gate {
    closed: Mutex<bool>,
    opened: Condvar,
}

impl Gate {
    fn pass(&self) {
        let mut closed = self.closed.lock().unwrap_or_else(PoisonError::into_inner);
        while *closed {
            closed = self.opened.wait(closed).unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn set(&self, closed: bool) {
        *self.closed.lock().unwrap_or_else(PoisonError::into_inner) = closed;
        self.opened.notify_all();
    }
}

/// One kept contribution: when it was kept (a count, not a clock), and its size.
struct Kept {
    order: u64,
    bytes: usize,
    contribution: Arc<Contribution>,
}

/// The forward stage's state: the workers, the kept contributions, the last whole signal and
/// the last whole reading.
pub struct Forward {
    batches: Arc<Slot<Batch>>,
    done: Receiver<Done>,
    /// The epoch of the current scene; the series worker drops any work from another.
    epoch: Arc<AtomicU64>,
    kept: HashMap<u64, Kept>,
    /// The bytes of everything in `kept`.
    kept_bytes: usize,
    /// How many contributions have ever been kept: the next one's `order`.
    kept_count: u64,
    /// The keys the current scene's signal is made of, in summing order.
    wanted: Vec<u64>,
    readouts: Arc<Vec<f64>>,
    towers: usize,
    signal: Option<Arc<Signal>>,
    state: SignalState,
    /// Why there is no signal, when there cannot be one.
    why: Option<String>,
    /// How many contributions the worker has been asked for since the start.
    pub computed: u64,
    /// How many it has finished and handed back.
    pub finished: u64,
    started: Instant,
    /// How long the last signal took from the edit to being whole.
    pub took: Option<Duration>,
    read_jobs: Arc<Slot<ReadJob>>,
    read_done: Receiver<(u64, ReadingAt)>,
    /// The epoch of the reading wanted now.
    read_epoch: u64,
    /// The epoch of the reading held: a later one replaces it, an earlier one is dropped.
    read_held: u64,
    /// Readings asked for at or before this epoch belong to a scene that has gone (a load, or
    /// the engine's end): the reading worker abandons one it has begun.
    read_abandon: Arc<AtomicU64>,
    reading: Option<Arc<ReadingAt>>,
    reading_state: SignalState,
    gate: Arc<Gate>,
}

impl std::fmt::Debug for Forward {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Forward")
            .field("state", &self.state)
            .field("kept", &self.kept.len())
            .field("kept_bytes", &self.kept_bytes)
            .finish()
    }
}

/// FNV-1a over text: the key a contribution is kept under.
fn hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// What every source's contribution depends on besides the source itself: the array (instrument
/// and towers), the timeline and the cap on shots, hashed once for a scene. (Written out again
/// for each of a thousand sources it was a quarter of a gigabyte of text an edit.)
fn common(spec: &SceneSpec) -> u64 {
    let json = |value: &dyn erased::Json| value.text();
    hash(&format!("{} {} {}", json(&spec.array), spec.duration, read::MAX_SHOTS))
}

/// The key of one source's contribution: everything it depends on, and nothing else. serde_json
/// writes the shortest text that reads back to the same number, so different inputs differ.
fn key(common: u64, spec: &SceneSpec, source: SourceId) -> u64 {
    let json = |value: &dyn erased::Json| value.text();
    let this = match source {
        SourceId::Floor => format!("floor {}", json(&spec.floor)),
        SourceId::Body(n) => format!("body {}", json(&spec.bodies[n])),
    };
    hash(&format!("{common:016x} {this}"))
}

mod erased {
    pub trait Json {
        fn text(&self) -> String;
    }

    impl<T: serde::Serialize> Json for T {
        fn text(&self) -> String {
            serde_json::to_string(self).expect("a spec is JSON")
        }
    }
}

/// The bytes a contribution holds: its three channels and its warnings.
fn size_of(contribution: &Contribution) -> usize {
    let values =
        contribution.lower.capacity() + contribution.upper.capacity() + contribution.gradiometer.capacity();
    values * std::mem::size_of::<f64>()
        + contribution.warnings.iter().map(String::capacity).sum::<usize>()
        + std::mem::size_of::<Contribution>()
}

impl Forward {
    /// Starts the two workers. They live until this value is dropped.
    #[must_use]
    pub fn new() -> Self {
        let gate = Arc::new(Gate::default());
        let batches = Arc::new(Slot::<Batch>::new());
        let (outbox, done) = channel::<Done>();
        let epoch = Arc::new(AtomicU64::new(0));
        let (current, held, inbox) = (Arc::clone(&epoch), Arc::clone(&gate), Arc::clone(&batches));
        let series = std::thread::Builder::new().name("cav-forward".into()).spawn(move || {
            // Ends when the engine goes: the slot is closed and `take` gives nothing.
            'batches: while let Some(batch) = inbox.take() {
                held.pass();
                let stale = || current.load(Ordering::Relaxed) != batch.epoch;
                for &(source, key) in &batch.work {
                    if stale() {
                        continue 'batches;
                    }
                    if let Some(contribution) =
                        read::contribution(&batch.spec, source, &batch.readouts, &stale)
                        && outbox.send(Done { key, contribution }).is_err()
                    {
                        break 'batches;
                    }
                }
            }
        });
        series.expect("the forward worker's thread");

        let read_jobs = Arc::new(Slot::<ReadJob>::new());
        let (read_outbox, read_done) = channel::<(u64, ReadingAt)>();
        let read_abandon = Arc::new(AtomicU64::new(0));
        let (abandon, held, inbox) = (Arc::clone(&read_abandon), Arc::clone(&gate), Arc::clone(&read_jobs));
        let readings = std::thread::Builder::new().name("cav-reading".into()).spawn(move || {
            while let Some(job) = inbox.take() {
                held.pass();
                // A reading that has been begun is finished, unless its scene has gone.
                let gone = || abandon.load(Ordering::Relaxed) >= job.epoch;
                if gone() {
                    continue;
                }
                if let Some(reading) = read::reading(&job.spec, job.time, &gone)
                    && read_outbox.send((job.epoch, reading)).is_err()
                {
                    break;
                }
            }
        });
        readings.expect("the reading worker's thread");

        Self {
            batches,
            done,
            epoch,
            kept: HashMap::new(),
            kept_bytes: 0,
            kept_count: 0,
            wanted: Vec::new(),
            readouts: Arc::new(Vec::new()),
            towers: 0,
            signal: None,
            state: SignalState::None,
            why: None,
            computed: 0,
            finished: 0,
            started: Instant::now(),
            took: None,
            read_jobs,
            read_done,
            read_epoch: 0,
            read_held: 0,
            read_abandon,
            reading: None,
            reading_state: SignalState::None,
            gate,
        }
    }

    /// Holds the workers before their next job (`true`), or lets them go on. While they are held
    /// nothing is computed, whatever is asked for: for tests that must say what a caller's thread
    /// did without timing it.
    pub fn hold(&self, held: bool) {
        self.gate.set(held);
    }

    /// How many bytes of contributions are kept now: never more than [`FORWARD_STORE_BYTES`].
    #[must_use]
    pub const fn kept_bytes(&self) -> usize {
        self.kept_bytes
    }

    /// The scene has changed (or `fresh` asks for everything again): works out which
    /// contributions are already here and asks the worker for the rest. Returns at once.
    pub fn schedule(&mut self, spec: &SceneSpec, fresh: bool) {
        // Whatever the worker has finished is taken now, so that results cannot pile up behind a
        // caller that edits and never asks; whatever it is doing for an earlier scene is no
        // longer wanted.
        self.take_done();
        let epoch = self.epoch.fetch_add(1, Ordering::Relaxed) + 1;
        self.started = Instant::now();
        if fresh {
            self.kept.clear();
            self.kept_bytes = 0;
        }
        let sources = read::sources(spec);
        let planned = read::readouts(spec).and_then(|readouts| {
            // The scene's own contributions must fit in the store, or it would never be whole.
            let values = spec.array.towers.len() * readouts.len();
            let need = values.saturating_mul(sources.len()).saturating_mul(BYTES_PER_VALUE);
            if need > FORWARD_STORE_BYTES {
                return Err(format!(
                    "{} source(s) × {} tower(s) × {} shot(s) is {} MiB of contributions, more than the {} MiB \
                     the engine keeps; shorten the timeline or use fewer sources",
                    sources.len(),
                    spec.array.towers.len(),
                    readouts.len(),
                    need >> 20,
                    FORWARD_STORE_BYTES >> 20
                ));
            }
            Ok(readouts)
        });
        let readouts = match planned {
            Ok(readouts) => Arc::new(readouts),
            Err(why) => {
                (self.signal, self.state, self.why) = (None, SignalState::None, Some(why));
                self.wanted.clear();
                self.readouts = Arc::new(Vec::new());
                self.trim();
                return;
            }
        };
        let common = common(spec);
        self.wanted = sources.iter().map(|&source| key(common, spec, source)).collect();
        (self.readouts, self.towers, self.why) = (readouts, spec.array.towers.len(), None);
        let work: Vec<(SourceId, u64)> = sources
            .iter()
            .zip(&self.wanted)
            .filter(|(_, key)| !self.kept.contains_key(key))
            .map(|(&source, &key)| (source, key))
            .collect();
        self.computed += work.len() as u64;
        if !work.is_empty() {
            self.batches.put(Batch {
                epoch,
                spec: Arc::new(spec.clone()),
                readouts: Arc::clone(&self.readouts),
                work,
            });
        }
        self.state = SignalState::Stale;
        self.trim();
        self.assemble();
    }

    /// Asks the reading worker for what every tower reads from a shot read out at `time`. Returns
    /// at once; the last whole reading stays, marked stale, until this one arrives. A request not
    /// yet begun is replaced by this one.
    pub fn request_reading(&mut self, spec: &SceneSpec, time: f64) {
        self.read_epoch += 1;
        self.read_jobs.put(ReadJob { epoch: self.read_epoch, spec: Arc::new(spec.clone()), time });
        self.reading_state = SignalState::Stale;
    }

    /// Forgets the last whole signal and reading: they belong to another scene altogether (a
    /// load), and showing them under the new scene's name, even marked stale, would mislead. A
    /// reading under way for that scene is abandoned.
    pub fn forget(&mut self) {
        self.signal = None;
        self.reading = None;
        self.read_abandon.store(self.read_epoch, Ordering::Relaxed);
        self.read_held = self.read_epoch;
    }

    /// Sums the kept contributions into the signal if every one the scene needs is here.
    fn assemble(&mut self) -> bool {
        if self.state != SignalState::Stale || !self.wanted.iter().all(|key| self.kept.contains_key(key)) {
            return false;
        }
        let mut signal = Signal::silent(self.towers, self.readouts.as_ref().clone());
        for key in &self.wanted {
            signal.add(&self.kept[key].contribution);
        }
        self.signal = Some(Arc::new(signal));
        self.state = SignalState::Refined;
        self.took = Some(self.started.elapsed());
        true
    }

    fn keep(&mut self, done: Done) {
        // Kept by what it is, whichever scene asked for it: a later scene may want it again.
        self.finished += 1;
        let bytes = size_of(&done.contribution);
        let kept = Kept { order: self.kept_count, bytes, contribution: Arc::new(done.contribution) };
        self.kept_count += 1;
        if let Some(replaced) = self.kept.insert(done.key, kept) {
            self.kept_bytes -= replaced.bytes;
        }
        self.kept_bytes += bytes;
        self.trim();
    }

    /// Holds the store to its budget: contributions the current scene does not need are dropped,
    /// oldest first, until it fits. The current scene's always fit ([`Self::schedule`] refuses a
    /// scene whose would not).
    fn trim(&mut self) {
        if self.kept_bytes <= FORWARD_STORE_BYTES {
            return;
        }
        let mut spare: Vec<(u64, u64)> = self
            .kept
            .iter()
            .filter(|(key, _)| !self.wanted.contains(key))
            .map(|(key, kept)| (kept.order, *key))
            .collect();
        spare.sort_unstable();
        for (_, key) in spare {
            if self.kept_bytes <= FORWARD_STORE_BYTES {
                break;
            }
            if let Some(dropped) = self.kept.remove(&key) {
                self.kept_bytes -= dropped.bytes;
            }
        }
    }

    /// Takes every contribution the worker has finished.
    fn take_done(&mut self) {
        while let Ok(done) = self.done.try_recv() {
            self.keep(done);
        }
    }

    /// Takes a finished reading unless a later one is already held. True when it was taken. It
    /// is the one asked for last only when its epoch is the current one; an earlier one is still
    /// shown, as stale, so that a drag sees numbers move.
    fn take_reading(&mut self, (epoch, reading): (u64, ReadingAt)) -> bool {
        if epoch <= self.read_held {
            return false;
        }
        self.read_held = epoch;
        self.reading = Some(Arc::new(reading));
        if epoch == self.read_epoch {
            self.reading_state = SignalState::Refined;
        }
        true
    }

    /// Takes what the workers have finished: `(a new whole signal has just been made, a new
    /// reading has just arrived)`.
    pub fn poll(&mut self) -> (bool, bool) {
        self.take_done();
        let mut read = false;
        while let Ok(reading) = self.read_done.try_recv() {
            read |= self.take_reading(reading);
        }
        (self.assemble(), read)
    }

    /// Waits, at most `patience`, for the signal and the reading to be whole. For clients that
    /// print numbers; the application never calls it. Returns what [`Self::poll`] returns, over
    /// the whole wait.
    pub fn settle(&mut self, patience: Duration) -> (bool, bool) {
        self.wait(patience, true)
    }

    /// Waits, at most `patience`, for the reading alone: a reading that is ready in milliseconds
    /// is not held up by a long series.
    pub fn settle_reading(&mut self, patience: Duration) -> (bool, bool) {
        self.wait(patience, false)
    }

    fn wait(&mut self, patience: Duration, for_signal: bool) -> (bool, bool) {
        let deadline = Instant::now() + patience;
        let (mut signal, mut read) = self.poll();
        while (for_signal && self.state == SignalState::Stale) || self.reading_state == SignalState::Stale {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else { break };
            let nap = left.min(Duration::from_millis(5));
            // Wait on whichever is outstanding; the other is looked at on the way round.
            if for_signal && self.state == SignalState::Stale {
                match self.done.recv_timeout(nap) {
                    Ok(done) => self.keep(done),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match self.read_done.recv_timeout(nap) {
                    Ok(reading) => read |= self.take_reading(reading),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
            let (s, r) = self.poll();
            signal |= s;
            read |= r;
        }
        (signal, read)
    }

    /// The last whole signal, which is the current scene's only when the state is `Refined`.
    #[must_use]
    pub const fn signal(&self) -> Option<&Arc<Signal>> {
        self.signal.as_ref()
    }

    /// Where the signal stands.
    #[must_use]
    pub const fn state(&self) -> SignalState {
        self.state
    }

    /// The last whole reading and where it stands: the one asked for last only when `Refined`.
    #[must_use]
    pub const fn reading(&self) -> (SignalState, Option<&Arc<ReadingAt>>) {
        (self.reading_state, self.reading.as_ref())
    }

    /// Why there is no signal, when the scene cannot have one.
    #[must_use]
    pub fn why(&self) -> Option<&str> {
        self.why.as_deref()
    }
}

impl Default for Forward {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Forward {
    /// The engine is going: whatever the workers are in the middle of is wanted by nobody. They
    /// see it at their next step, drop the work, find their slot closed and end.
    fn drop(&mut self) {
        self.epoch.fetch_add(1, Ordering::Relaxed);
        self.read_abandon.store(u64::MAX, Ordering::Relaxed);
        self.batches.close();
        self.read_jobs.close();
        self.gate.set(false);
    }
}
