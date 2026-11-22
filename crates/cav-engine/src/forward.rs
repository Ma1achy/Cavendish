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
//! [`Forward::request_reading`] send jobs and return; [`Forward::poll`] takes whatever has come
//! back. Two workers, so that the one number under the playhead is not queued behind a long
//! series. Every job carries the epoch it was sent in, and the worker checks the current epoch
//! as it goes: work for a scene that has since changed is dropped, not finished. Until a new
//! result is whole, the last whole one stays in place, marked stale.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use cav_sim::SceneSpec;
use cav_sim::read::{self, Contribution, ReadingAt, Signal, SourceId};

/// How many contributions are kept. A drag makes a new one at every step; past this the store is
/// emptied of all but what the current scene needs.
const KEPT: usize = 64;

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

struct Job {
    epoch: u64,
    key: u64,
    spec: Arc<SceneSpec>,
    source: SourceId,
    readouts: Arc<Vec<f64>>,
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
        let mut closed = self.closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        while *closed {
            closed = self.opened.wait(closed).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    fn set(&self, closed: bool) {
        *self.closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = closed;
        self.opened.notify_all();
    }
}

/// The forward stage's state: the workers, the kept contributions, the last whole signal and
/// the last whole reading.
pub struct Forward {
    jobs: Sender<Job>,
    done: Receiver<Done>,
    /// The epoch of the current scene; the series worker drops any job from another.
    epoch: Arc<AtomicU64>,
    kept: HashMap<u64, Arc<Contribution>>,
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
    read_jobs: Sender<ReadJob>,
    read_done: Receiver<(u64, ReadingAt)>,
    /// The epoch of the reading wanted now; the reading worker drops any other.
    read_epoch: Arc<AtomicU64>,
    reading: Option<Arc<ReadingAt>>,
    reading_state: SignalState,
    gate: Arc<Gate>,
}

impl std::fmt::Debug for Forward {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Forward").field("state", &self.state).field("kept", &self.kept.len()).finish()
    }
}

/// FNV-1a over text: the key a contribution is kept under.
fn hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// The key of one source's contribution: everything it depends on, and nothing else. serde_json
/// writes the shortest text that reads back to the same number, so different inputs differ.
fn key(spec: &SceneSpec, source: SourceId) -> u64 {
    let json = |value: &dyn erased::Json| value.text();
    let this = match source {
        SourceId::Floor => format!("floor {}", json(&spec.floor)),
        SourceId::Body(n) => format!("body {}", json(&spec.bodies[n])),
    };
    hash(&format!("{} {} {} {this}", json(&spec.array), spec.duration, read::MAX_SHOTS))
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

impl Forward {
    /// Starts the two workers. They live until this value is dropped.
    #[must_use]
    pub fn new() -> Self {
        let gate = Arc::new(Gate::default());
        let (jobs, inbox) = channel::<Job>();
        let (outbox, done) = channel::<Done>();
        let epoch = Arc::new(AtomicU64::new(0));
        let (current, held) = (Arc::clone(&epoch), Arc::clone(&gate));
        let series = std::thread::Builder::new().name("cav-forward".into()).spawn(move || {
            // Ends when the engine goes: the sender is dropped and `recv` fails.
            while let Ok(job) = inbox.recv() {
                held.pass();
                let stale = || current.load(Ordering::Relaxed) != job.epoch;
                if stale() {
                    continue;
                }
                if let Some(contribution) = read::contribution(&job.spec, job.source, &job.readouts, &stale)
                    && outbox.send(Done { key: job.key, contribution }).is_err()
                {
                    break;
                }
            }
        });
        series.expect("the forward worker's thread");

        let (read_jobs, read_inbox) = channel::<ReadJob>();
        let (read_outbox, read_done) = channel::<(u64, ReadingAt)>();
        let read_epoch = Arc::new(AtomicU64::new(0));
        let (current, held) = (Arc::clone(&read_epoch), Arc::clone(&gate));
        let readings = std::thread::Builder::new().name("cav-reading".into()).spawn(move || {
            while let Ok(job) = read_inbox.recv() {
                held.pass();
                let stale = || current.load(Ordering::Relaxed) != job.epoch;
                if stale() {
                    continue;
                }
                if let Some(reading) = read::reading(&job.spec, job.time, &stale)
                    && read_outbox.send((job.epoch, reading)).is_err()
                {
                    break;
                }
            }
        });
        readings.expect("the reading worker's thread");

        Self {
            jobs,
            done,
            epoch,
            kept: HashMap::new(),
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
            read_epoch,
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

    /// The scene has changed (or `fresh` asks for everything again): works out which
    /// contributions are already here and asks the worker for the rest. Returns at once.
    pub fn schedule(&mut self, spec: &SceneSpec, fresh: bool) {
        // Whatever the worker is doing for an earlier scene is no longer wanted.
        let epoch = self.epoch.fetch_add(1, Ordering::Relaxed) + 1;
        self.started = Instant::now();
        if fresh {
            self.kept.clear();
        }
        let readouts = match read::readouts(spec) {
            Ok(readouts) => Arc::new(readouts),
            Err(why) => {
                (self.signal, self.state, self.why) = (None, SignalState::None, Some(why));
                self.wanted.clear();
                return;
            }
        };
        let sources = read::sources(spec);
        self.wanted = sources.iter().map(|&source| key(spec, source)).collect();
        (self.readouts, self.towers, self.why) = (readouts, spec.array.towers.len(), None);
        if self.kept.len() > KEPT {
            let wanted = &self.wanted;
            self.kept.retain(|key, _| wanted.contains(key));
        }
        let shared = Arc::new(spec.clone());
        for (&source, &key) in sources.iter().zip(&self.wanted) {
            if !self.kept.contains_key(&key) {
                self.computed += 1;
                let job = Job {
                    epoch,
                    key,
                    spec: Arc::clone(&shared),
                    source,
                    readouts: Arc::clone(&self.readouts),
                };
                // The worker cannot have gone while this value lives; if it has, the signal stays stale.
                let _ = self.jobs.send(job);
            }
        }
        self.state = SignalState::Stale;
        self.assemble();
    }

    /// Asks the reading worker for what every tower reads from a shot read out at `time`. Returns
    /// at once; the last whole reading stays, marked stale, until this one arrives.
    pub fn request_reading(&mut self, spec: &SceneSpec, time: f64) {
        let epoch = self.read_epoch.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.read_jobs.send(ReadJob { epoch, spec: Arc::new(spec.clone()), time });
        self.reading_state = SignalState::Stale;
    }

    /// Forgets the last whole signal and reading: they belong to another scene altogether (a
    /// load), and showing them under the new scene's name, even marked stale, would mislead.
    pub fn forget(&mut self) {
        self.signal = None;
        self.reading = None;
    }

    /// Sums the kept contributions into the signal if every one the scene needs is here.
    fn assemble(&mut self) -> bool {
        if self.state != SignalState::Stale || !self.wanted.iter().all(|key| self.kept.contains_key(key)) {
            return false;
        }
        let mut signal = Signal::silent(self.towers, self.readouts.as_ref().clone());
        for key in &self.wanted {
            signal.add(&self.kept[key]);
        }
        self.signal = Some(Arc::new(signal));
        self.state = SignalState::Refined;
        self.took = Some(self.started.elapsed());
        true
    }

    fn keep(&mut self, done: Done) {
        // Kept by what it is, whichever scene asked for it: a later scene may want it again.
        self.finished += 1;
        self.kept.insert(done.key, Arc::new(done.contribution));
    }

    /// Takes a finished reading if it is the one wanted now. True when it was.
    fn take_reading(&mut self, (epoch, reading): (u64, ReadingAt)) -> bool {
        if epoch != self.read_epoch.load(Ordering::Relaxed) {
            return false;
        }
        self.reading = Some(Arc::new(reading));
        self.reading_state = SignalState::Refined;
        true
    }

    /// Takes what the workers have finished: `(a new whole signal has just been made, a new
    /// reading has just arrived)`.
    pub fn poll(&mut self) -> (bool, bool) {
        while let Ok(done) = self.done.try_recv() {
            self.keep(done);
        }
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
        let deadline = Instant::now() + patience;
        let (mut signal, mut read) = self.poll();
        while self.state == SignalState::Stale || self.reading_state == SignalState::Stale {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else { break };
            let nap = left.min(Duration::from_millis(5));
            // Wait on whichever is outstanding; the other is looked at on the way round.
            if self.state == SignalState::Stale {
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
    /// see the new epoch at their next step, drop the job, find their inbox closed and end.
    fn drop(&mut self) {
        self.epoch.fetch_add(1, Ordering::Relaxed);
        self.read_epoch.fetch_add(1, Ordering::Relaxed);
        self.gate.set(false);
    }
}
