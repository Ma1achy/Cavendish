//! The forward stage: every tower's signal, computed off the caller's thread.
//!
//! The phase is a sum over sources, so each source's contribution is a job of its own and is kept
//! by the hash of what it depends on (the instrument, the towers, the timeline and that source).
//! Moving one body asks the worker for that body alone; the rest is already here. The signal is
//! the kept contributions summed in the scene's order, which is the order a cold run sums them
//! in, so the two are the same bits.
//!
//! Nothing here waits. [`Forward::schedule`] sends jobs and returns; [`Forward::poll`] takes
//! whatever has come back. Every job carries the epoch it was sent in, and the worker checks the
//! current epoch between shots: work for a scene that has since changed is dropped, not finished.
//! Until a new signal is whole, the last whole one stays in place, marked stale.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use cav_sim::SceneSpec;
use cav_sim::read::{self, Contribution, Signal, SourceId};

/// How many contributions are kept. A drag makes a new one at every step; past this the store is
/// emptied of all but what the current scene needs.
const KEPT: usize = 64;

/// Where the signal stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalState {
    /// The signal is the current scene's.
    Refined,
    /// The scene has changed; the signal shown is the last whole one, and work is under way.
    Stale,
    /// There is no signal to show: the scene cannot have one, and `why` says so.
    None,
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

/// The forward stage's state: the worker, the kept contributions and the last whole signal.
pub struct Forward {
    jobs: Sender<Job>,
    done: Receiver<Done>,
    /// The epoch of the current scene; the worker drops any job from another.
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
    started: Instant,
    /// How long the last signal took from the edit to being whole.
    pub took: Option<Duration>,
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
    /// Starts the worker. It lives until this value is dropped.
    #[must_use]
    pub fn new() -> Self {
        let (jobs, inbox) = channel::<Job>();
        let (outbox, done) = channel::<Done>();
        let epoch = Arc::new(AtomicU64::new(0));
        let current = Arc::clone(&epoch);
        let spawned = std::thread::Builder::new().name("cav-forward".into()).spawn(move || {
            // Ends when the engine goes: the sender is dropped and `recv` fails.
            while let Ok(job) = inbox.recv() {
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
        spawned.expect("the forward worker's thread");
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
            started: Instant::now(),
            took: None,
        }
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

    /// Forgets the last whole signal: it belongs to another scene altogether (a load), and showing
    /// it under the new scene's name, even marked stale, would mislead.
    pub fn forget(&mut self) {
        self.signal = None;
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

    /// Takes what the worker has finished. True when a new whole signal has just been made.
    pub fn poll(&mut self) -> bool {
        while let Ok(done) = self.done.try_recv() {
            // Kept by what it is, whichever scene asked for it: a later scene may want it again.
            self.kept.insert(done.key, Arc::new(done.contribution));
        }
        self.assemble()
    }

    /// Waits, at most `patience`, for the signal to be whole. For clients that print numbers; the
    /// application never calls it. True when the signal is the current scene's.
    pub fn settle(&mut self, patience: Duration) -> bool {
        let deadline = Instant::now() + patience;
        while self.state == SignalState::Stale {
            self.poll();
            if self.state != SignalState::Stale {
                break;
            }
            let Some(left) = deadline.checked_duration_since(Instant::now()) else { break };
            match self.done.recv_timeout(left.min(Duration::from_millis(50))) {
                Ok(done) => {
                    self.kept.insert(done.key, Arc::new(done.contribution));
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        self.poll();
        self.state == SignalState::Refined
    }

    /// The last whole signal, which is the current scene's only when the state is `Refined`.
    #[must_use]
    pub fn signal(&self) -> Option<&Arc<Signal>> {
        self.signal.as_ref()
    }

    #[must_use]
    pub const fn state(&self) -> SignalState {
        self.state
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
    /// The engine is going: whatever the worker is in the middle of is wanted by nobody. It sees
    /// the new epoch at its next shot, drops the job, finds its inbox closed and ends.
    fn drop(&mut self) {
        self.epoch.fetch_add(1, Ordering::Relaxed);
    }
}
