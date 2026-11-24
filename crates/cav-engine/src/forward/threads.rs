//! The forward stage's workers on a native process: two threads, each blocked on its slot until
//! there is work, so that [`super::Forward::poll`] only takes what they send.

use std::sync::Arc;
use std::sync::PoisonError;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use std::sync::atomic::Ordering;

use cav_sim::read::{self, SourceId};

use super::{Batch, Done, Gate, ReadJob, Shared, Slot};

impl<T> Slot<T> {
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
}

impl Gate {
    /// Waits while the gate is closed.
    fn pass(&self) {
        let mut closed = self.closed.lock().unwrap_or_else(PoisonError::into_inner);
        while *closed {
            closed = self.opened.wait(closed).unwrap_or_else(PoisonError::into_inner);
        }
    }
}

impl Shared {
    /// One source of a batch: its contribution, sent back. False when nobody is listening any
    /// more (the engine has gone).
    fn contribute(&self, batch: &Batch, (source, key): (SourceId, u64)) -> bool {
        let stale = || self.stale(batch);
        match read::contribution(&batch.spec, source, &batch.shots, &stale) {
            Some(contribution) => self.outbox.send(Done { key, contribution }).is_ok(),
            None => true,
        }
    }

    /// One reading, sent back. A reading that has been begun is finished, unless its scene has
    /// gone. False when nobody is listening any more.
    fn read(&self, job: &ReadJob) -> bool {
        let gone = || self.read_abandon.load(Ordering::Relaxed) >= job.epoch;
        if gone() {
            return true;
        }
        match read::reading(&job.spec, job.time, &gone) {
            Some(reading) => self.read_outbox.send((job.epoch, reading)).is_ok(),
            None => true,
        }
    }
}

/// The two threads. They hold nothing the caller needs: they end when their slots are closed.
pub(super) struct Workers;

impl Workers {
    /// Starts the series worker and the reading worker.
    pub(super) fn start(shared: Shared) -> Self {
        let shared = Arc::new(shared);
        let series = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("cav-forward".into())
            .spawn(move || {
                // Ends when the engine goes: the slot is closed and `take` gives nothing.
                'batches: while let Some(batch) = series.batches.take() {
                    series.gate.pass();
                    series.series_gate.pass();
                    for &item in &batch.work {
                        if series.stale(&batch) {
                            continue 'batches;
                        }
                        if !series.contribute(&batch, item) {
                            break 'batches;
                        }
                    }
                }
            })
            .expect("the forward worker's thread");
        std::thread::Builder::new()
            .name("cav-reading".into())
            .spawn(move || {
                while let Some(job) = shared.read_jobs.take() {
                    shared.gate.pass();
                    if !shared.read(&job) {
                        break;
                    }
                }
            })
            .expect("the reading worker's thread");
        Self
    }

    /// Nothing: the threads run the jobs.
    #[allow(clippy::unused_self)]
    pub(super) const fn run(&mut self, _budget: Duration) {}

    /// Waits at most `nap` for what a worker sends.
    #[allow(clippy::unused_self)]
    pub(super) fn receive<T>(&mut self, results: &Receiver<T>, nap: Duration) -> Result<T, RecvTimeoutError> {
        results.recv_timeout(nap)
    }
}
