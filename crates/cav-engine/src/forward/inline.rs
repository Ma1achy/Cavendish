//! The forward stage's workers in a browser page, where no thread can be started.
//!
//! **With a remote** ([`super::wire::install`]; the application installs the page's Web Workers,
//! each running this same module): [`super::Forward::poll`] hands each job over as bytes
//! ([`super::wire`]) and takes the answers that have come, and never runs one itself, so the
//! page's thread never waits on a voxel body's seconds of work (`plan/REBUILD.md` §5.1). One
//! reading and one source's contribution are out at a time, the reading's lane apart from the
//! series', as natively; the answers go through the same channels as the threads' do, so
//! everything after them is the native code. A job cannot be stopped once handed over (a worker
//! shares no memory with the page): one for a scene that has since changed runs to its end, its
//! answer is kept by what it is (a later scene may want it), and the next job is the latest
//! scene's.
//!
//! **With none** (the web equivalence under Node.js, which has no Web Workers): the same jobs,
//! through the same bytes, run by `poll` itself, a source at a time and resumed at the next
//! call; a call that ran over its budget is followed by [`REST`] in which `poll` runs nothing.
//!
//! Waiting ([`Workers::receive`], for `settle`) cannot wait for a remote's answer: it arrives
//! from the page's event loop, which is the thread that would be waiting. With a remote,
//! `receive` hands work over and says nothing more can come now (`Disconnected`), so `settle`
//! returns at once with what there is, and a client is told to try again (a figure of a signal
//! not yet whole).

use std::rc::Rc;
use std::sync::PoisonError;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::Duration;

use web_time::Instant;

use super::wire::{self, Answer, Remote};
use super::{Batch, Done, Gate, ReadJob, Shared, Slot};
use cav_sim::SceneSpec;
use cav_sim::read::{Shots, SourceId};
use std::sync::Arc;

/// How long `poll` runs nothing after a call that ran over its budget, with no remote: two
/// frames at 60 Hz and more than the shell's 30 ms between repaints while the signal is stale.
pub const REST: Duration = Duration::from_millis(50);

/// How long `poll` runs nothing after the workers start, with no remote: long enough for the
/// page's first frames.
pub const FIRST_REST: Duration = Duration::from_millis(250);

impl<T> Slot<T> {
    /// The item, if there is one now.
    fn try_take(&self) -> Option<T> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner).0.take()
    }
}

impl Gate {
    fn is_closed(&self) -> bool {
        *self.closed.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The jobs in hand: what both workers share, the batch under way with the index of its next
/// source, and the jobs out at a remote.
pub(super) struct Workers {
    shared: Shared,
    batch: Option<(Batch, usize)>,
    /// Until when `poll` runs nothing, so that the page can show a frame (no remote).
    rest_until: Instant,
    /// The contribution out at the remote: its ticket, the key it is kept under and the shots
    /// it must be of.
    series_out: Option<(u64, u64, Arc<Shots>)>,
    /// The reading out at the remote: its ticket and the request's epoch.
    reading_out: Option<(u64, u64)>,
}

impl Workers {
    /// Starts nothing: the jobs are run by [`Self::run`].
    pub(super) fn start(shared: Shared) -> Self {
        Self {
            shared,
            batch: None,
            rest_until: Instant::now() + FIRST_REST,
            series_out: None,
            reading_out: None,
        }
    }

    /// From `poll`, once a frame: with a remote, hands jobs over and takes answers (no work
    /// here); with none, runs jobs for about `budget` unless resting.
    pub(super) fn run(&mut self, budget: Duration) {
        if let Some(remote) = wire::remote() {
            self.exchange(&remote);
            return;
        }
        let started = Instant::now();
        if started < self.rest_until {
            return;
        }
        self.work(budget);
        let now = Instant::now();
        if now.duration_since(started) > budget {
            self.rest_until = now + REST;
        }
    }

    /// Takes the remote's answers, and hands over the next reading and the next source if their
    /// lanes are free.
    fn exchange(&mut self, remote: &Rc<dyn Remote>) {
        if let Some((ticket, epoch)) = self.reading_out
            && let Some(bytes) = remote.take(ticket)
        {
            self.reading_out = None;
            match wire::answer(&bytes) {
                Ok(Answer::Reading(reading)) if !self.abandoned(epoch) => {
                    let _ = self.shared.read_outbox.send((epoch, reading));
                }
                // Nothing, or bytes that are not an answer: neither comes from the same module.
                _ => {}
            }
        }
        if let Some((ticket, key, shots)) = self.series_out.take() {
            match remote.take(ticket) {
                Some(bytes) => self.deliver(&bytes, key, shots),
                None => self.series_out = Some((ticket, key, shots)),
            }
        }
        if self.shared.gate.is_closed() {
            return;
        }
        if self.reading_out.is_none()
            && let Some(job) = self.next_reading()
        {
            let ticket = remote.send(wire::reading_request(&job.spec, job.time));
            self.reading_out = Some((ticket, job.epoch));
        }
        if self.series_out.is_none()
            && !self.shared.series_gate.is_closed()
            && let Some((spec, shots, source, key)) = self.next_source()
        {
            let ticket = remote.send(wire::contribution_request(&spec, source));
            self.series_out = Some((ticket, key, shots));
        }
    }

    /// Sends a contribution's answer on as the threads send theirs, if it is one and has a
    /// value for every shot the engine scheduled (the runner works the shots out again from the
    /// scene: the same function of the same scene, so always; the count is checked all the same,
    /// since a signal summed from a contribution of other shots would be wrong, not short).
    fn deliver(&self, bytes: &[u8], key: u64, shots: Arc<Shots>) {
        if let Ok(Answer::Contribution(contribution)) = wire::answer(bytes)
            && contribution.lower.len() == shots.total()
        {
            let _ = self.shared.outbox.send(Done { key, contribution });
        }
    }

    /// Whether readings asked for at `epoch` belong to a scene that has gone.
    fn abandoned(&self, epoch: u64) -> bool {
        self.shared.read_abandon.load(Ordering::Relaxed) >= epoch
    }

    /// The reading wanted next, if one is waiting and its scene is still here.
    fn next_reading(&self) -> Option<ReadJob> {
        self.shared.read_jobs.try_take().filter(|job| !self.abandoned(job.epoch))
    }

    /// The next source of the latest batch, skipping a batch whose scene has changed.
    fn next_source(&mut self) -> Option<(Arc<SceneSpec>, Arc<Shots>, SourceId, u64)> {
        // The latest batch replaces one under way: it is the current scene's.
        if let Some(batch) = self.shared.batches.try_take() {
            self.batch = Some((batch, 0));
        }
        let (batch, next) = self.batch.as_mut()?;
        if self.shared.stale(batch) || *next >= batch.work.len() {
            self.batch = None;
            return None;
        }
        let (source, key) = batch.work[*next];
        *next += 1;
        Some((Arc::clone(&batch.spec), Arc::clone(&batch.shots), source, key))
    }

    /// Runs jobs for about `budget` (no remote): at least one if there is one, and none once
    /// `budget` has passed. A reading first, then the series a source at a time.
    fn work(&mut self, budget: Duration) {
        let started = Instant::now();
        while !self.shared.gate.is_closed() && self.step() {
            if started.elapsed() >= budget {
                break;
            }
        }
    }

    /// One job, run here through the same bytes a remote would get. False when there was none.
    fn step(&mut self) -> bool {
        if let Some(job) = self.shared.read_jobs.try_take() {
            if !self.abandoned(job.epoch)
                && let Ok(Answer::Reading(reading)) =
                    wire::answer(&wire::serve(&wire::reading_request(&job.spec, job.time)))
            {
                let _ = self.shared.read_outbox.send((job.epoch, reading));
            }
            return true;
        }
        if self.shared.series_gate.is_closed() {
            return false;
        }
        let Some((spec, shots, source, key)) = self.next_source() else { return false };
        self.deliver(&wire::serve(&wire::contribution_request(&spec, source)), key, shots);
        true
    }

    /// For `settle`: with a remote, hands work over and takes what has come, then says nothing
    /// more can arrive while this thread waits (`Disconnected`); with none, runs jobs for at
    /// most `nap`. Then takes what a worker sent, if anything.
    pub(super) fn receive<T>(&mut self, results: &Receiver<T>, nap: Duration) -> Result<T, RecvTimeoutError> {
        let remote = wire::remote();
        match &remote {
            Some(remote) => self.exchange(remote),
            None => self.work(nap),
        }
        results.try_recv().map_err(|error| match error {
            TryRecvError::Empty if remote.is_none() => RecvTimeoutError::Timeout,
            _ => RecvTimeoutError::Disconnected,
        })
    }
}
