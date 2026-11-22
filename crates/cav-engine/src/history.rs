//! The undo history and the journal's budget: both grow with the session, so both are counted in
//! bytes and held to a stated size.
//!
//! An edit is remembered as the pair of small edits that take it back and put it on again (the
//! old value at a path, the item that was removed), not as a copy of the scene: a `Set` is a few
//! bytes however large the scene. Only a `Load` keeps whole scenes, since nothing smaller undoes
//! it.

use std::collections::VecDeque;

use cav_sim::SceneSpec;
use serde::Serialize;
use serde_json::Value;

use crate::command::Command;

/// The most bytes of edits the undo history holds, undo and redo together. Past it the oldest
/// edits can no longer be undone.
pub const HISTORY_BYTES: usize = 32 << 20;

/// The most bytes of commands the journal holds. Past it the journal stops recording, says so
/// once in the log, and is no longer offered as a session that can be replayed.
pub const JOURNAL_BYTES: usize = 64 << 20;

/// One change to a scene, as the history replays it in either direction.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Edit {
    /// Put this value at this path.
    Put {
        /// The path of the field.
        path: String,
        /// The value it takes.
        value: Value,
    },
    /// Put this item into the list at `list`, at `index`.
    Insert {
        /// The path of the list.
        list: String,
        /// Where the item goes, from 0.
        index: usize,
        /// The item.
        value: Value,
    },
    /// Take the item at this path (which ends in an index) out of its list.
    Remove {
        /// The path of the item.
        path: String,
    },
    /// Replace the whole scene: what undoes and redoes a load.
    Replace(Box<SceneSpec>),
}

impl Edit {
    /// The scene after this edit.
    ///
    /// # Errors
    /// When the edit does not apply to `spec`. An edit the history made applies to the scene it
    /// was made for; an error here means the history and the scene have parted, and it is
    /// reported rather than hidden.
    pub fn apply(&self, spec: &SceneSpec) -> Result<SceneSpec, String> {
        match self {
            Self::Put { path, value } => spec.edited(path, value).map(|(spec, _)| spec),
            Self::Insert { list, index, value } => {
                spec.inserted(list, Some(*index), value).map(|(spec, _)| spec)
            }
            Self::Remove { path } => spec.removed(path).map(|(spec, _)| spec),
            Self::Replace(scene) => Ok((**scene).clone()),
        }
    }

    /// The bytes the edit is counted as holding.
    #[must_use]
    pub fn weight(&self) -> usize {
        let own = std::mem::size_of::<Self>();
        match self {
            Self::Put { path, value } | Self::Insert { list: path, value, .. } => {
                own + path.len() + weigh_value(value)
            }
            Self::Remove { path } => own + path.len(),
            Self::Replace(scene) => own + weigh_spec(scene),
        }
    }
}

/// One step of the history: the edit that takes it back and the edit that puts it on again.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Step {
    /// What undoing it does.
    pub undo: Edit,
    /// What redoing it does.
    pub redo: Edit,
}

impl Step {
    fn weight(&self) -> usize {
        self.undo.weight() + self.redo.weight()
    }
}

/// How many bytes a scene is counted as: the length of its JSON, which is within a small factor
/// of what it holds in memory and is the same on every run.
fn weigh_spec(spec: &SceneSpec) -> usize {
    serde_json::to_vec(spec).map_or(0, |text| text.len())
}

/// The edits that can be taken back, and those that can be put on again.
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
    /// An empty history that holds at most `budget` bytes of edits.
    #[must_use]
    pub fn with_budget(budget: usize) -> Self {
        Self { undo: VecDeque::new(), redo: Vec::new(), bytes: 0, budget, dropped: 0 }
    }

    /// An edit has been made: it can be undone, and nothing can be redone any more.
    pub fn edited(&mut self, step: Step) {
        self.forget_redo();
        self.push_undo(step);
    }

    /// A step of a drag after its first: the edit that takes the whole drag back is already
    /// held, and what puts it on again is now `redo`.
    pub fn continued(&mut self, redo: Edit) {
        self.forget_redo();
        if let Some(last) = self.undo.back_mut() {
            self.bytes -= last.weight();
            last.redo = redo;
            self.bytes += last.weight();
        }
    }

    fn forget_redo(&mut self) {
        self.bytes -= self.redo.iter().map(Step::weight).sum::<usize>();
        self.redo.clear();
    }

    fn push_undo(&mut self, step: Step) {
        self.bytes += step.weight();
        self.undo.push_back(step);
        // The oldest go first. The newest is kept whatever it weighs: one step is bounded by
        // the spec's own limits, far below the budget.
        while self.bytes > self.budget && self.undo.len() > 1 {
            let oldest = self.undo.pop_front().expect("more than one");
            self.bytes -= oldest.weight();
            self.dropped += 1;
        }
    }

    /// The step an undo would take back, if there is one.
    #[must_use]
    pub fn last(&self) -> Option<&Step> {
        self.undo.back()
    }

    /// The step a redo would put on again, if there is one.
    #[must_use]
    pub fn next(&self) -> Option<&Step> {
        self.redo.last()
    }

    /// The last edit has been taken back: it can now be redone.
    pub fn undone(&mut self) {
        if let Some(step) = self.undo.pop_back() {
            self.redo.push(step);
        }
    }

    /// The edit last taken back has been put on again: it can be undone once more.
    pub fn redone(&mut self) {
        if let Some(step) = self.redo.pop() {
            self.bytes -= step.weight();
            self.push_undo(step);
        }
    }

    /// How many edits can be undone, and how many redone.
    #[must_use]
    pub fn depth(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }

    /// The bytes the history is counted as holding: never more than its budget plus one step.
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
        Command::Set { path, value, .. } | Command::Insert { path, value, .. } => {
            own + path.len() + weigh_value(value)
        }
        Command::Remove { path } => own + path.len(),
        Command::Export { to, .. } => own + to.len(),
        Command::Compare(Some(_)) => own + std::mem::size_of::<cav_sim::InstrumentSpec>(),
        _ => own,
    }
}

/// The bytes of a JSON value, counted without writing it out.
fn weigh_value(value: &Value) -> usize {
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
    use serde_json::json;

    fn put(n: usize) -> Step {
        Step {
            undo: Edit::Put { path: "duration".into(), value: json!(1000.0 + n as f64) },
            redo: Edit::Put { path: "duration".into(), value: json!(2000.0 + n as f64) },
        }
    }

    #[test]
    fn the_history_drops_its_oldest_steps_for_room() {
        let each = put(0).weight();
        let fits = 100;
        let mut history = History::with_budget(fits * each + each / 2);
        for n in 0..fits + 50 {
            history.edited(put(n));
        }
        assert!(history.bytes() <= fits * each + each / 2, "{} bytes", history.bytes());
        assert_eq!(history.depth(), (fits, 0));
        assert_eq!(history.dropped(), 50);
        // The newest are the ones kept: the step to undo is the last one made.
        assert_eq!(history.last(), Some(&put(fits + 49)));
        history.undone();
        assert_eq!((history.depth(), history.next()), ((fits - 1, 1), Some(&put(fits + 49))));
        history.redone();
        assert_eq!(history.depth(), (fits, 0));
        // A new edit after an undo forgets what could be redone, and its bytes.
        history.undone();
        history.edited(put(7));
        assert_eq!((history.depth(), history.bytes()), ((fits, 0), fits * each));
        // Control: a short history drops nothing.
        let mut short = History::default();
        short.edited(put(1));
        short.edited(put(2));
        assert_eq!((short.depth(), short.dropped(), short.bytes()), ((2, 0), 0, 2 * each));
        // A drag keeps the edit that takes it all back and only renews what puts it on again.
        short.continued(Edit::Put { path: "duration".into(), value: json!(9.0) });
        assert_eq!(short.last().unwrap().undo, put(2).undo);
        assert_eq!(short.last().unwrap().redo, Edit::Put { path: "duration".into(), value: json!(9.0) });
    }

    #[test]
    fn edits_and_commands_are_weighed_by_what_they_carry() {
        let small = weigh(&Command::Seek(1.0));
        let load = weigh(&Command::load(cav_sim::dev::oscillator_zoo()));
        assert!(load > small + 1000, "{load} against {small}");
        let long = "x".repeat(10_000);
        assert!(weigh(&Command::set("name", json!(long))) >= small + 10_000);
        assert!(weigh(&Command::set("bodies", json!([[long, 1.0], {"key": long}]))) >= small + 20_000);
        // A set is small however large the scene; a load is the scene, twice (back and forth).
        assert!(put(0).weight() < 400, "{}", put(0).weight());
        let scene = Box::new(cav_sim::dev::oscillator_zoo());
        let replace = Step { undo: Edit::Replace(scene.clone()), redo: Edit::Replace(scene) };
        assert!(replace.weight() > 2000);
        // Edits apply as they say.
        let room = SceneSpec::room();
        let longer = Edit::Put { path: "duration".into(), value: json!(30.0) }.apply(&room).unwrap();
        assert_eq!(longer.duration, 30.0);
        assert!(Edit::Remove { path: "bodies[0]".into() }.apply(&room).is_err(), "nothing to remove");
    }
}
