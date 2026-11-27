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

/// The most bytes the journal holds, counted as the file it would be written as
/// ([`weigh`]). Past it the journal stops recording, says so once in the log, and is no longer
/// offered as a session that can be replayed; `journal restart` (`Engine::restart_journal`)
/// starts a new one from the session as it stands.
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
    /// Put a pane's camera back as it was: what undoes and redoes `Home`, the one camera move
    /// that is an edit (`docs/slices/S18.md` §2). The scene is unchanged.
    View {
        /// The pane, from 0.
        pane: usize,
        /// Its whole view.
        view: Box<cav_sim::ViewSpec>,
    },
    /// Put the scene's folders, and which folder each body and tower is in, as they are here:
    /// what undoes and redoes a folder edit (`Command::Folder`), however many members it moved.
    /// A few bytes a body; nothing else of the scene is touched.
    Folders {
        /// The console line of the folder edit this is a step of: what the history list shows.
        line: String,
        /// The folders and every member's folder.
        to: Box<cav_sim::folders::Membership>,
    },
    /// Several edits as one step, applied in order (each on the scene the one before made):
    /// what undoes and redoes `folder duplicate`, which makes folders and adds bodies.
    Batch {
        /// The console line the whole step is: what the history list shows.
        line: String,
        /// The edits, in the order applied.
        edits: Vec<Self>,
    },
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
            Self::View { .. } => Ok(spec.clone()),
            Self::Folders { to, .. } => spec.with_membership(to),
            Self::Batch { edits, .. } => edits.iter().try_fold(spec.clone(), |spec, edit| edit.apply(&spec)),
        }
    }

    /// The console line that does this edit: `set <path> <value>`, `add <list> <index> <value>`,
    /// `remove <path>`, `load <name>`, `view home`. What the history list shows.
    #[must_use]
    pub fn line(&self) -> String {
        match self {
            Self::Put { path, value } => format!("set {path} {value}"),
            Self::Insert { list, index, value } => {
                let text = value.to_string();
                let short: String = text.chars().take(60).collect();
                format!("add {list} {index} {short}{}", if text.chars().count() > 60 { "…" } else { "" })
            }
            Self::Remove { path } => format!("remove {path}"),
            Self::Replace(scene) => format!("load {}", scene.name),
            Self::View { pane, .. } => format!("view home @{}", pane + 1),
            Self::Folders { line, .. } | Self::Batch { line, .. } => line.clone(),
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
            Self::View { .. } => own + std::mem::size_of::<cav_sim::ViewSpec>(),
            Self::Folders { line, to } => {
                let text = |label: &Option<String>| label.as_ref().map_or(0, String::len);
                let slot = std::mem::size_of::<Option<String>>();
                own + line.len()
                    + to.folders
                        .iter()
                        .map(|f| std::mem::size_of_val(f) + f.label.len() + text(&f.parent))
                        .sum::<usize>()
                    + to.bodies.iter().chain(&to.towers).map(|l| slot + text(l)).sum::<usize>()
            }
            Self::Batch { line, edits } => own + line.len() + edits.iter().map(Self::weight).sum::<usize>(),
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
    /// The step of a drag that has come back to where it began, while the drag may go on.
    #[serde(skip)]
    parked: Option<Step>,
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
        Self { undo: VecDeque::new(), redo: Vec::new(), bytes: 0, budget, dropped: 0, parked: None }
    }

    /// An edit has been made: it can be undone, and nothing can be redone any more. An edit that
    /// changed nothing (its undo and its redo the same: a `set` to the value already there) is
    /// no step to undo (S18: a cancelled transform session leaves none, live or replayed).
    pub fn edited(&mut self, step: Step) {
        self.parked = None;
        if step.undo == step.redo {
            // Parked, not dropped: if it is the first step of a drag, the drag's later steps
            // renew it, never the edit before it. Nothing was changed, so what can be redone
            // still can (S18's review, minor 1): a drag that goes on calls `continued`, which
            // forgets the redo once something has changed.
            self.parked = Some(step);
        } else {
            self.forget_redo();
            self.push_undo(step);
        }
    }

    /// A step of a drag after its first: the edit that takes the whole drag back is already
    /// held, and what puts it on again is now `redo`. A drag that comes back to where it began
    /// leaves no step: there is nothing to undo. (The step is parked while the drag may still
    /// go on, so that a drag passing through its start keeps its own step and never renews
    /// the edit before it.)
    pub fn continued(&mut self, redo: Edit) {
        if let Some(mut step) = self.parked.take() {
            step.redo = redo;
            if step.undo == step.redo {
                // Still back where it began: still nothing to undo, and the redo kept.
                self.parked = Some(step);
            } else {
                self.forget_redo();
                self.push_undo(step);
            }
            return;
        }
        self.forget_redo();
        if let Some(last) = self.undo.back_mut() {
            self.bytes -= last.weight();
            last.redo = redo;
            if last.undo == last.redo {
                self.parked = self.undo.pop_back();
            } else {
                self.bytes += last.weight();
            }
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
        self.parked = None;
        if let Some(step) = self.undo.pop_back() {
            self.redo.push(step);
        }
    }

    /// The edit last taken back has been put on again: it can be undone once more.
    pub fn redone(&mut self) {
        self.parked = None;
        if let Some(step) = self.redo.pop() {
            self.bytes -= step.weight();
            self.push_undo(step);
        }
    }

    /// What each step that can be undone did, oldest first, and each that can be redone, the
    /// next to redo first: a console line for each, from its redo edit ([`Edit::line`]), so that
    /// the history holds no text of its own.
    #[must_use]
    pub fn said(&self) -> (Vec<String>, Vec<String>) {
        (
            self.undo.iter().map(|s| s.redo.line()).collect(),
            self.redo.iter().rev().map(|s| s.redo.line()).collect(),
        )
    }

    /// The steps that can be undone, oldest first, and those that can be redone, the next to
    /// redo first: what the editor's history list words for a person (`said` gives the lines).
    #[must_use]
    pub fn steps(&self) -> (Vec<&Step>, Vec<&Step>) {
        (self.undo.iter().collect(), self.redo.iter().rev().collect())
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

    /// The most bytes this history holds.
    #[must_use]
    pub const fn budget(&self) -> usize {
        self.budget
    }

    /// How many of the oldest steps have been dropped for room.
    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }
}

/// How many bytes a command is counted as in the journal: the bytes it takes in a journal file
/// (its JSON, as [`crate::journal::write`] writes it) and one for the comma after it. Counted
/// this way a journal within its budget is a file within its budget, which is the most
/// [`crate::journal::read`] reads: what is offered as whole can be read back. (Counted by what
/// a command holds in memory, as it was, a figure's export weighed less than its text and a
/// "whole" journal could be written larger than it could be read.)
#[must_use]
pub fn weigh(command: &Command) -> usize {
    serde_json::to_vec(command).map_or(0, |text| text.len()) + 1
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
    fn a_change_to_nothing_is_no_step_and_never_renews_the_one_before() {
        let put = |v: f64| Edit::Put { path: "duration".into(), value: json!(v) };
        let mut history = History::default();
        history.edited(Step { undo: put(1.0), redo: put(2.0) });
        // A set to the value already there is no step.
        history.edited(Step { undo: put(2.0), redo: put(2.0) });
        assert_eq!(history.depth(), (1, 0));
        // A drag from 2 that comes back to 2 leaves no step, and the edit before it is untouched.
        history.edited(Step { undo: put(2.0), redo: put(3.0) });
        history.continued(put(2.0));
        assert_eq!(history.depth(), (1, 0));
        assert_eq!(history.last(), Some(&Step { undo: put(1.0), redo: put(2.0) }));
        // Control: the same drag going on past its start keeps a step of its own.
        history.continued(put(4.0));
        assert_eq!(history.depth(), (2, 0));
        assert_eq!(history.last(), Some(&Step { undo: put(2.0), redo: put(4.0) }));
        assert_eq!(history.bytes(), 2 * Step { undo: put(1.0), redo: put(2.0) }.weight());
        // A drag whose first step changes nothing (it starts where the value is) still has its
        // own step once it moves, and the edit before it is untouched.
        history.edited(Step { undo: put(4.0), redo: put(4.0) });
        history.continued(put(5.0));
        assert_eq!(history.depth(), (3, 0));
        assert_eq!(history.last(), Some(&Step { undo: put(4.0), redo: put(5.0) }));
        history.undone();
        assert_eq!(history.last(), Some(&Step { undo: put(2.0), redo: put(4.0) }));
    }

    #[test]
    fn a_change_to_nothing_keeps_what_can_be_redone() {
        // S18's review, minor 1: set 13, undo, set it back to the value it has: the redo stays.
        let put = |v: f64| Edit::Put { path: "duration".into(), value: json!(v) };
        let mut history = History::default();
        history.edited(Step { undo: put(12.0), redo: put(13.0) });
        history.undone();
        history.edited(Step { undo: put(12.0), redo: put(12.0) });
        assert_eq!(history.depth(), (0, 1), "the no-op left the redo");
        // A drag that starts and ends where the value is keeps it too.
        history.continued(put(12.0));
        assert_eq!(history.depth(), (0, 1));
        // Control: once something changes, the redo is gone, as after any edit.
        history.continued(put(14.0));
        assert_eq!(history.depth(), (1, 0));
        let mut edited = History::default();
        edited.edited(Step { undo: put(12.0), redo: put(13.0) });
        edited.undone();
        edited.edited(Step { undo: put(12.0), redo: put(15.0) });
        assert_eq!(edited.depth(), (1, 0));
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
