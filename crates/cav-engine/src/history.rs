//! The undo history and the journal's budget: both grow with the session, so both are counted in
//! bytes and held to a stated size.

use std::collections::VecDeque;

use cav_sim::SceneSpec;
use serde::Serialize;

use crate::command::Command;

/// The most bytes of scenes the undo history holds, undo and redo together. Past it the oldest
/// edits can no longer be undone. A scene of a thousand bodies is about 0.15 MiB as JSON, so this
/// is some two hundred steps of the largest scenes and tens of thousands of ordinary ones.
pub const HISTORY_BYTES: usize = 32 << 20;

/// The most bytes of commands the journal holds. Past it the journal stops recording, says so
/// once in the log, and is no longer offered as a session that can be replayed.
pub const JOURNAL_BYTES: usize = 64 << 20;

/// One step of the history: the scene to go back (or forward) to, and what it weighs.
#[derive(Clone, Debug, Serialize)]
struct Step {
    spec: SceneSpec,
    #[serde(skip)]
    bytes: usize,
}

/// How many bytes a scene is counted as: the length of its JSON, which is within a small factor
/// of what it holds in memory and is the same on every run.
fn weigh_spec(spec: &SceneSpec) -> usize {
    serde_json::to_vec(spec).map_or(0, |text| text.len())
}

/// The scenes an edit can be taken back to, and those an undo can be taken forward to again.
#[derive(Clone, Debug, Serialize)]
pub struct History {
    undo: VecDeque<Step>,
    redo: Vec<Step>,
    #[serde(skip)]
    bytes: usize,
    /// The most bytes held: [`HISTORY_BYTES`] unless the engine was made with another.
    #[serde(skip)]
    budget: usize,
    /// How many steps have been dropped for room since the start.
    dropped: u64,
}

impl Default for History {
    fn default() -> Self {
        Self::with_budget(HISTORY_BYTES)
    }
}

impl History {
    /// An empty history that holds at most `budget` bytes of scenes.
    #[must_use]
    pub fn with_budget(budget: usize) -> Self {
        Self { undo: VecDeque::new(), redo: Vec::new(), bytes: 0, budget, dropped: 0 }
    }

    /// An edit has replaced `before`: it can be undone, and nothing can be redone any more.
    pub fn edited(&mut self, before: SceneSpec) {
        self.forget_redo();
        self.push_undo(before);
    }

    /// A step of a drag after its first: the scene to go back to is already held.
    pub fn continued(&mut self) {
        self.forget_redo();
    }

    fn forget_redo(&mut self) {
        self.bytes -= self.redo.iter().map(|step| step.bytes).sum::<usize>();
        self.redo.clear();
    }

    fn push_undo(&mut self, spec: SceneSpec) {
        let bytes = weigh_spec(&spec);
        self.bytes += bytes;
        self.undo.push_back(Step { spec, bytes });
        // The oldest go first. The newest is kept whatever it weighs: one scene is bounded by
        // the spec's own limits, far below the budget.
        while self.bytes > self.budget && self.undo.len() > 1 {
            let oldest = self.undo.pop_front().expect("more than one");
            self.bytes -= oldest.bytes;
            self.dropped += 1;
        }
    }

    /// Takes back the last edit: gives the scene before it and keeps `now` to redo to.
    pub fn undo(&mut self, now: SceneSpec) -> Option<SceneSpec> {
        let step = self.undo.pop_back()?;
        self.bytes -= step.bytes;
        let bytes = weigh_spec(&now);
        self.bytes += bytes;
        self.redo.push(Step { spec: now, bytes });
        Some(step.spec)
    }

    /// Reapplies the edit last taken back: gives the scene after it and keeps `now` to undo to.
    pub fn redo(&mut self, now: SceneSpec) -> Option<SceneSpec> {
        let step = self.redo.pop()?;
        self.bytes -= step.bytes;
        self.push_undo(now);
        Some(step.spec)
    }

    /// How many edits can be undone, and how many redone.
    #[must_use]
    pub fn depth(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }

    /// The bytes the history is counted as holding: never more than its budget plus one scene.
    #[must_use]
    pub const fn bytes(&self) -> usize {
        self.bytes
    }

    /// How many of the oldest steps have been dropped for room.
    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }
}

/// How many bytes a command is counted as in the journal.
#[must_use]
pub fn weigh(command: &Command) -> usize {
    let own = std::mem::size_of::<Command>();
    match command {
        Command::Load(spec) => own + weigh_spec(spec),
        Command::Set { path, value, .. } => own + path.len() + weigh_value(value),
        _ => own,
    }
}

/// The bytes of a JSON value, counted without writing it out.
fn weigh_value(value: &serde_json::Value) -> usize {
    use serde_json::Value;
    let own = std::mem::size_of::<Value>();
    match value {
        Value::String(text) => own + text.len(),
        Value::Array(items) => own + items.iter().map(weigh_value).sum::<usize>(),
        Value::Object(map) => {
            own + map.iter().map(|(key, item)| key.len() + weigh_value(item)).sum::<usize>()
        }
        _ => own,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_history_drops_its_oldest_steps_for_room() {
        let scene = cav_sim::dev::oscillator_zoo();
        // Every step weighs the same: the durations all have four digits.
        let step = |n: usize| SceneSpec { duration: 1000.0 + n as f64, ..scene.clone() };
        let each = weigh_spec(&step(0));
        let fits = 100;
        let mut history = History::with_budget(fits * each + each / 2);
        for n in 0..fits + 50 {
            history.edited(step(n));
        }
        assert!(history.bytes() <= fits * each + each / 2, "{} bytes", history.bytes());
        assert_eq!(history.depth(), (fits, 0));
        assert_eq!(history.dropped(), 50);
        // The newest are the ones kept: undoing gives the last scene pushed.
        let back = history.undo(scene.clone()).unwrap();
        assert_eq!(back.duration, 1000.0 + (fits + 49) as f64);
        assert_eq!(history.depth(), (fits - 1, 1));
        // Control: a short history drops nothing and gives back every step.
        let mut short = History::default();
        short.edited(scene.clone());
        short.edited(scene.clone());
        assert_eq!((short.depth(), short.dropped(), short.bytes()), ((2, 0), 0, 2 * weigh_spec(&scene)));
        assert!(short.undo(scene.clone()).is_some() && short.undo(scene.clone()).is_some());
        assert!(short.undo(scene.clone()).is_none());
        assert_eq!(short.depth(), (0, 2));
        assert!(short.redo(scene).is_some());
    }

    #[test]
    fn commands_are_weighed_by_what_they_carry() {
        let small = weigh(&Command::Seek(1.0));
        let load = weigh(&Command::load(cav_sim::dev::oscillator_zoo()));
        assert!(load > small + 1000, "{load} against {small}");
        let long = "x".repeat(10_000);
        let set = weigh(&Command::set("name", serde_json::json!(long)));
        assert!(set >= small + 10_000);
        let nested = weigh(&Command::set("bodies", serde_json::json!([[long, 1.0], {"key": long}])));
        assert!(nested >= small + 20_000);
    }
}
