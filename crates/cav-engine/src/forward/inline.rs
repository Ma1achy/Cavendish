//! The forward stage's workers in a browser page, where no thread can be started: the same two
//! jobs, run by [`super::Forward::poll`] itself for a bounded time a call. A reading is run before
//! the series (it is the one number under the playhead); a batch is run one source at a time and
//! resumed at the next call. The results go through the same channels as the threads' do, so
//! everything after them is the native code.

use std::sync::PoisonError;
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::Duration;

use web_time::Instant;

use super::{Batch, Gate, Shared, Slot};

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

/// The jobs in hand: what both workers share, and the batch under way with the index of its
/// next source.
pub(super) struct Workers {
    shared: Shared,
    batch: Option<(Batch, usize)>,
}

impl Workers {
    /// Starts nothing: the jobs are run by [`Self::run`].
    pub(super) const fn start(shared: Shared) -> Self {
        Self { shared, batch: None }
    }

    /// Runs jobs for about `budget`: at least one if there is one, and none once `budget` has
    /// passed. A reading first, then the series a source at a time.
    pub(super) fn run(&mut self, budget: Duration) {
        let started = Instant::now();
        while !self.shared.gate.is_closed() && self.step() {
            if started.elapsed() >= budget {
                break;
            }
        }
    }

    /// One job. False when there was none to run.
    fn step(&mut self) -> bool {
        if let Some(job) = self.shared.read_jobs.try_take() {
            self.shared.read(&job);
            return true;
        }
        if self.shared.series_gate.is_closed() {
            return false;
        }
        // The latest batch replaces one under way: it is the current scene's, and the one under
        // way would be dropped as stale at its next source anyway.
        if let Some(batch) = self.shared.batches.try_take() {
            self.batch = Some((batch, 0));
        }
        let Some((batch, next)) = &mut self.batch else { return false };
        if self.shared.stale(batch) || *next >= batch.work.len() {
            self.batch = None;
            return true;
        }
        let item = batch.work[*next];
        *next += 1;
        self.shared.contribute(batch, item);
        true
    }

    /// Runs jobs for at most `nap`, then takes what a worker sent, if anything.
    pub(super) fn receive<T>(&mut self, results: &Receiver<T>, nap: Duration) -> Result<T, RecvTimeoutError> {
        self.run(nap);
        results.try_recv().map_err(|error| match error {
            TryRecvError::Empty => RecvTimeoutError::Timeout,
            TryRecvError::Disconnected => RecvTimeoutError::Disconnected,
        })
    }
}
