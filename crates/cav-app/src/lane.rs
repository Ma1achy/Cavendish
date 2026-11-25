//! The forward stage's worker lane in a page, as a plain state machine: which Web Worker holds
//! which job, which jobs wait, when a failed worker is started again, and when the page runs a job
//! itself. The page (`platform/web.rs`) turns each [`Action`] into a call on a `Worker`; nothing
//! here touches a browser, so the native tests hold it (S4's review, I11).
//!
//! **Rules.**
//! - Jobs go to a free worker that has said it is ready, first come first served.
//! - A worker that fails is terminated; the job it held is answered with nothing (the engine's
//!   lane is free again; that scene's numbers stay stale until an edit asks again); the failure is
//!   said, with the panic's own message when the worker sent one before it trapped. It is started
//!   again after [`FIRST_BACKOFF_MS`], then four times longer each time, at most [`MAX_RESPAWNS`]
//!   times; past that its slot stays empty, and that is said.
//! - When no worker is ready **now** and the oldest waiting job has waited [`READY_WITHIN_MS`],
//!   the page runs waiting jobs itself, at most one a frame ([`LOCAL_GAP_MS`] apart): slow, not
//!   wrong. Workers that come back (ready again) take the jobs again.
//!
//! Times are milliseconds on the page's clock, as the caller reads it.

use std::collections::{HashMap, VecDeque};

/// How many Web Workers the forward stage has: two, like the native stage's two threads (the
/// series and the reading under the playhead).
pub const WORKERS: usize = 2;
/// How long a job may wait with no worker ready before the page runs it itself, milliseconds.
pub const READY_WITHIN_MS: f64 = 15_000.0;
/// The most times one worker's slot is started again after failures.
pub const MAX_RESPAWNS: u32 = 3;
/// How long after its first failure a worker is started again, milliseconds; each later wait is
/// four times the one before.
pub const FIRST_BACKOFF_MS: f64 = 1_000.0;
/// The least time between the end of one job the page ran itself and the start of the next,
/// milliseconds: longer than a pump's calls within one frame, so at most one such job a frame.
pub const LOCAL_GAP_MS: f64 = 50.0;

/// What the page is to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Start a new worker in slot `0`, sending it the module.
    Spawn(usize),
    /// Terminate the worker in this slot.
    Terminate(usize),
    /// Hand job `ticket`'s request to the worker in `slot`.
    Post {
        /// The worker's slot.
        slot: usize,
        /// The job.
        ticket: u64,
        /// Its request (`cav_engine::forward::wire`).
        request: Vec<u8>,
    },
    /// Run job `ticket` on the page's own thread now, and report it with [`Lane::ran_here`].
    RunHere {
        /// The job.
        ticket: u64,
        /// Its request.
        request: Vec<u8>,
    },
    /// Say this on the page and in the console.
    Say(String),
}

#[derive(Clone, Debug, Default)]
struct Slot {
    /// Whether the worker now in the slot has said it is ready.
    ready: bool,
    /// The job it holds.
    busy: Option<u64>,
    /// How many times the slot has been started again.
    respawns: u32,
    /// When it is to be started again, after a failure.
    respawn_at: Option<f64>,
    /// Whether it has failed past [`MAX_RESPAWNS`] and stays empty.
    dead: bool,
    /// The panic message the worker sent before it trapped, for its failure's words.
    panic: Option<String>,
}

/// The lane's state.
#[derive(Debug)]
pub struct Lane {
    slots: Vec<Slot>,
    /// Waiting jobs: ticket, when it was sent, its request.
    queue: VecDeque<(u64, f64, Vec<u8>)>,
    answers: HashMap<u64, Vec<u8>>,
    next_ticket: u64,
    /// The page may run a job itself from this time on.
    local_after: f64,
}

impl Lane {
    /// A lane of [`WORKERS`] slots, and the actions that start their workers.
    #[must_use]
    pub fn new() -> (Self, Vec<Action>) {
        let lane = Self {
            slots: vec![Slot::default(); WORKERS],
            queue: VecDeque::new(),
            answers: HashMap::new(),
            next_ticket: 0,
            local_after: f64::NEG_INFINITY,
        };
        (lane, (0..WORKERS).map(Action::Spawn).collect())
    }

    /// Queues a job's request at `now`; returns its ticket.
    pub fn send(&mut self, request: Vec<u8>, now: f64) -> u64 {
        self.next_ticket += 1;
        self.queue.push_back((self.next_ticket, now, request));
        self.next_ticket
    }

    /// Job `ticket`'s answer, when it has one (empty: the job was lost with its worker).
    pub fn take(&mut self, ticket: u64) -> Option<Vec<u8>> {
        self.answers.remove(&ticket)
    }

    /// The worker in `slot` says it is ready.
    pub fn ready(&mut self, slot: usize) {
        if let Some(s) = self.slots.get_mut(slot) {
            s.ready = true;
        }
    }

    /// The worker in `slot` answered job `ticket`.
    pub fn answered(&mut self, slot: usize, ticket: u64, answer: Vec<u8>) {
        if let Some(s) = self.slots.get_mut(slot)
            && s.busy == Some(ticket)
        {
            s.busy = None;
        }
        self.answers.insert(ticket, answer);
    }

    /// The worker in `slot` says it is panicking, with the panic's message (it traps next).
    pub fn panicked(&mut self, slot: usize, message: String) {
        if let Some(s) = self.slots.get_mut(slot) {
            s.panic = Some(message);
        }
    }

    /// The worker in `slot` failed at `now` with the browser's `message`: it is terminated, its
    /// job answered with nothing, the failure said, and the slot started again later or left
    /// empty past [`MAX_RESPAWNS`].
    pub fn failed(&mut self, slot: usize, message: &str, now: f64) -> Vec<Action> {
        let Some(s) = self.slots.get_mut(slot) else { return Vec::new() };
        let mut actions = vec![Action::Terminate(slot)];
        s.ready = false;
        let lost = s.busy.take();
        let why = s.panic.take().unwrap_or_else(|| message.to_owned());
        if let Some(ticket) = lost {
            self.answers.insert(ticket, Vec::new());
        }
        let what = if lost.is_some() { "the job it was computing is dropped" } else { "it held no job" };
        let then = if s.respawns < MAX_RESPAWNS {
            let wait = FIRST_BACKOFF_MS * 4f64.powi(s.respawns as i32);
            s.respawn_at = Some(now + wait);
            format!("a new worker takes its place in {:.0} s", wait / 1000.0)
        } else {
            s.dead = true;
            s.respawn_at = None;
            format!(
                "it has failed {} times and is not started again; while no worker is ready the page runs the \
                 jobs itself, slowly",
                MAX_RESPAWNS + 1
            )
        };
        actions.push(Action::Say(format!(
            "The forward stage's worker {slot} failed ({why}): {what}; {then}. That scene's numbers stay stale; an \
             edit asks for them again."
        )));
        actions
    }

    /// What to do at `now`: start again the workers whose wait is over, hand waiting jobs to
    /// ready workers, and, when no worker is ready and the oldest job has waited
    /// [`READY_WITHIN_MS`], run one job here (at most one a frame).
    pub fn pump(&mut self, now: f64) -> Vec<Action> {
        let mut actions = Vec::new();
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.respawn_at.is_some_and(|at| now >= at) {
                slot.respawn_at = None;
                slot.respawns += 1;
                actions.push(Action::Spawn(index));
            }
        }
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if !slot.ready || slot.busy.is_some() {
                continue;
            }
            let Some((ticket, _, request)) = self.queue.pop_front() else { break };
            slot.busy = Some(ticket);
            actions.push(Action::Post { slot: index, ticket, request });
        }
        let none_ready = self.slots.iter().all(|s| !s.ready);
        let waited = self.queue.front().is_some_and(|(_, since, _)| now - since >= READY_WITHIN_MS);
        if none_ready && waited && now >= self.local_after {
            let (ticket, _, request) = self.queue.pop_front().expect("a job waits");
            // Until it is reported done, no other: a pump within the job's own frame runs none.
            self.local_after = f64::INFINITY;
            actions.push(Action::RunHere { ticket, request });
        }
        actions
    }

    /// The page ran job `ticket` itself and it ended at `now` with `answer`.
    pub fn ran_here(&mut self, ticket: u64, answer: Vec<u8>, now: f64) {
        self.answers.insert(ticket, answer);
        self.local_after = now + LOCAL_GAP_MS;
    }

    /// A post to the worker in `slot` failed: the job goes back to the front of the queue.
    pub fn not_posted(&mut self, slot: usize, ticket: u64, request: Vec<u8>, now: f64) {
        if let Some(s) = self.slots.get_mut(slot)
            && s.busy == Some(ticket)
        {
            s.busy = None;
        }
        self.queue.push_front((ticket, now, request));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the page's side of `actions` as a page would: each worker started, nothing more.
    fn spawns(actions: &[Action]) -> Vec<usize> {
        actions.iter().filter_map(|a| if let Action::Spawn(s) = a { Some(*s) } else { None }).collect()
    }

    #[test]
    fn a_failing_worker_is_started_again_a_bounded_number_of_times() {
        let (mut lane, start) = Lane::new();
        assert_eq!(spawns(&start), [0, 1]);
        // Worker 0 fails as soon as it is started (its script will not load), again and again.
        let mut now = 0.0;
        let mut started_again = Vec::new();
        for _ in 0..20 {
            let said = lane.failed(0, "Script error", now);
            assert_eq!(said[0], Action::Terminate(0));
            // The page is pumped every frame for a minute: the slot is started at most once.
            let mut spawned = Vec::new();
            for _ in 0..3600 {
                now += 1000.0 / 60.0;
                spawned.extend(spawns(&lane.pump(now)));
                if !spawned.is_empty() {
                    break;
                }
            }
            if spawned.is_empty() {
                break;
            }
            started_again.push(now);
        }
        assert_eq!(started_again.len(), MAX_RESPAWNS as usize, "{started_again:?}");
        // Each wait four times the one before (1, 4, 16 s, to a frame).
        let gaps: Vec<f64> = started_again.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(gaps.iter().zip([4000.0, 16000.0]).all(|(g, want)| (g - want).abs() < 40.0), "{gaps:?}");
        // Its last failure says it is not started again; worker 1 was never touched.
        let last = lane.failed(0, "Script error", now);
        assert!(matches!(&last[1], Action::Say(text) if text.contains("is not started again")), "{last:?}");
        assert!(spawns(&lane.pump(now + 1e7)).is_empty());
        // Control: a slot that has not failed is never started again.
        assert!(!started_again.is_empty() && lane.slots[1].respawns == 0);
    }

    #[test]
    fn the_page_runs_jobs_when_no_worker_is_ready_and_takes_them_back() {
        let (mut lane, _) = Lane::new();
        lane.ready(0);
        lane.ready(1);
        // Both workers ready: a job goes to one.
        let a = lane.send(vec![1], 0.0);
        let posted = lane.pump(0.0);
        assert!(matches!(posted[..], [Action::Post { slot: 0, ticket, .. }] if ticket == a));
        // Both fail after being ready once; a job waits.
        lane.failed(0, "RuntimeError: unreachable", 10.0);
        lane.failed(1, "RuntimeError: unreachable", 10.0);
        assert_eq!(lane.take(a), Some(Vec::new()), "the lost job is answered with nothing");
        let b = lane.send(vec![2], 20.0);
        let c = lane.send(vec![3], 20.0);
        // Control: before the wait is over, nothing runs here.
        assert!(!lane.pump(20.0 + READY_WITHIN_MS - 1.0).iter().any(|x| matches!(x, Action::RunHere { .. })));
        // Past it (the new workers were started but have not said ready): the page runs one, and
        // only one in its frame (S4's review, minor 9).
        let now = 20.0 + READY_WITHIN_MS;
        let first = lane.pump(now);
        let ran: Vec<u64> = first
            .iter()
            .filter_map(|x| if let Action::RunHere { ticket, .. } = x { Some(*ticket) } else { None })
            .collect();
        assert_eq!(ran, [b]);
        assert!(!lane.pump(now + 1.0).iter().any(|x| matches!(x, Action::RunHere { .. })), "one a frame");
        lane.ran_here(b, vec![9], now + 5.0);
        assert!(
            !lane.pump(now + 6.0).iter().any(|x| matches!(x, Action::RunHere { .. })),
            "not in the same frame"
        );
        assert_eq!(lane.take(b), Some(vec![9]));
        // A worker that comes back takes the jobs again: the page runs no more.
        lane.ready(1);
        let back = lane.pump(now + 100.0);
        assert!(matches!(back[..], [Action::Post { slot: 1, ticket, .. }] if ticket == c), "{back:?}");
    }

    #[test]
    fn a_worker_s_panic_message_is_said() {
        let (mut lane, _) = Lane::new();
        lane.ready(0);
        let job = lane.send(vec![1], 0.0);
        lane.pump(0.0);
        // The worker posts its panic's message, then traps; the trap's words are not the news.
        lane.panicked(0, "index out of bounds at crates/cav-sim/src/read.rs:12".into());
        let said = lane.failed(0, "RuntimeError: unreachable", 1.0);
        let Action::Say(text) = &said[1] else { panic!("{said:?}") };
        assert!(text.contains("index out of bounds") && !text.contains("unreachable"), "{text}");
        assert!(text.contains("the job it was computing is dropped"));
        assert_eq!(lane.take(job), Some(Vec::new()));
        // Control: with no panic sent, the browser's own message is said.
        let said = lane.failed(1, "Script error", 2.0);
        assert!(
            matches!(&said[1], Action::Say(t) if t.contains("Script error") && t.contains("it held no job"))
        );
    }
}
