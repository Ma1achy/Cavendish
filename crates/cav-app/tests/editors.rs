//! An editor never changes a value the user did not edit.
//!
//! S2's review read the number editor (finding 17) and asked: it shows six figures and parses
//! its text back, so does clicking into a field and out again issue an edit that rounds the
//! value? Driven here with the events a window would deliver, on values that six figures do not
//! say.
//!
//! It was real, by the keyboard: Tab into the mass, then Enter, issued `set bodies[0].mass 1.23457`
//! for a mass of 1.2345678901234567. The test below failed so until the editor's parser took
//! unchanged text for the value itself.

use cav_app::headless::{context, run_frames};
use cav_app::{NoSurface, Shell};
use cav_engine::Command;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, vec2};

struct Rig {
    ctx: egui::Context,
    shell: Shell,
    clock: f64,
    /// The window, points.
    size: [u32; 2],
}

impl Rig {
    fn new() -> Self {
        let mut rig = Self { ctx: context(), shell: Shell::new(), clock: 0.0, size: [1440, 900] };
        rig.frames(vec![vec![], vec![]]);
        rig
    }

    fn frames(&mut self, frames: Vec<Vec<Event>>) {
        run_frames(&self.ctx, &mut self.shell, &mut NoSurface, self.size, &mut self.clock, frames);
    }

    fn key(&mut self, key: Key) {
        let event = |pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        self.frames(vec![vec![event(true)], vec![event(false)], vec![]]);
    }

    fn click(&mut self, pos: Pos2) {
        let press = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.frames(vec![vec![Event::PointerMoved(pos)], vec![press(true)], vec![press(false)], vec![]]);
    }

    /// The scene as text that says every bit of every number.
    fn bits(&self) -> String {
        serde_json::to_string(self.shell.engine.spec()).unwrap()
    }

    fn sets(&self) -> usize {
        self.shell.engine.journal().iter().filter(|command| matches!(command, Command::Set { .. })).count()
    }
}

/// Numbers that six significant figures do not say.
/// The visits each selection makes (the counts the one test made between them, split).
/// S18 (TASK-S18-08): a body's inspector shows the body's fields only; the array's instrument and
/// cadence (24 of the 48 visits it made) moved to the scene's and the towers' inspectors, which
/// still visit every one of them (`TOWER_VISITS`, `SCENE_VISITS` unchanged). The floor is the
/// body's own fields' count, measured: 24.
const BODY_VISITS: usize = 24;
const TOWER_VISITS: usize = 48;
const SCENE_VISITS: usize = 56;

const AWKWARD: [(&str, &str); 6] = [
    ("bodies[0].mass", "1.2345678901234567"),
    ("bodies[0].radius", "0.30000000000000004"),
    ("bodies[0].motion.Static.position", "[2.0000000000000004, 0.1234567890123456, 2.5000000000000004]"),
    ("duration", "9.000000000000002"),
    ("array.towers[0].timing.offset", "0.10000000000000002"),
    ("array.instrument.gravity", "9.810000000000002"),
];

/// Loads the point, sets the awkward numbers, selects `selection`, and focuses and leaves every
/// field editor it shows in every way (a click elsewhere, Tab, Enter, Escape), from its middle
/// and from near each end of a vector, asserting that nothing changes. Returns the rig and how
/// many visits were made. One test a selection, so that the three run side by side (S2's second
/// review, item 8: as one test it was four seconds on `quick`'s critical path).
fn leave_every_editor(selection: &str) -> (Rig, usize) {
    let mut rig = Rig::new();
    // A window tall enough that every field is in sight, so that each click lands on its field:
    // 1000 since S20's rows of 20 points (1400 while they were 24: in 900 the instrument's last
    // ran under the timeline, where a click seeks or selects).
    rig.size = [1440, 1000];
    rig.shell.run("load point").unwrap();
    for (path, value) in AWKWARD {
        rig.shell.run(&format!("set {path} {value}")).unwrap();
    }
    let mut visited = 0;
    rig.shell.run(selection).unwrap();
    // The reading arrives before anything is clicked: its rows move the editors below them.
    assert!(rig.shell.engine.settle(std::time::Duration::from_secs(60)));
    rig.frames(vec![vec![], vec![]]);
    // With nothing selected the instrument and the cadence are folded (S18's review, U1): open
    // both, so that every field they hold is visited as before.
    for fold in ["fold inspector/instrument", "fold inspector/cadence"] {
        if let Some(header) = rig.shell.layout().button(fold).filter(|_| selection == "select none") {
            rig.click(header.center());
            rig.frames(vec![vec![], vec![]]);
        }
    }
    let editors: Vec<(String, egui::Rect)> = rig.shell.layout().editors.clone();
    assert!(editors.len() >= 5, "`{selection}` shows {} editors", editors.len());
    for (path, _) in editors {
        // Fields that hold a number, a vector or text; a tick box, a choice and an absent
        // option's `add` are buttons, and pressing a button is an edit.
        let whole = rig.shell.engine.spec().to_value();
        let value = cav_sim::paths::get(&whole, &path).unwrap().clone();
        if !(value.is_number()
            || value.is_string()
            || value.as_array().is_some_and(|v| v.iter().all(serde_json::Value::is_number)))
        {
            continue;
        }
        // Where a click lands in a field: its middle, and near each end (a vector has a
        // number at each).
        for place in 0..if value.is_array() { 3 } else { 1 } {
            // The four ways out of a field: a click elsewhere, Tab, Enter, Escape.
            for leave in 0..4 {
                // Where the field is now: Tab can scroll the panel to the widget it lands on.
                rig.frames(vec![vec![]]);
                let Some(rect) = rig.shell.layout().editor(&path) else { continue };
                let at = match place {
                    0 => rect.center(),
                    // Past a vector's axis letter, and short of the reset's place (S19's panels
                    // put X, Y, Z before the boxes).
                    1 => rect.left_center() + vec2(20.0, 0.0),
                    _ => rect.right_center() - vec2(26.0, 0.0),
                };
                let (before, sets) = (rig.bits(), rig.sets());
                rig.click(at);
                match leave {
                    // Elsewhere: the inspector's own ground beside its tabs (S19's panels: the
                    // name is edited in the header, with no label column to its left to click).
                    0 => {
                        let panel = rig.shell.layout().panel("properties").unwrap();
                        rig.click(panel.right_top() + vec2(-14.0, 14.0));
                    }
                    1 => rig.key(Key::Tab),
                    2 => rig.key(Key::Enter),
                    _ => rig.key(Key::Escape),
                }
                rig.frames(vec![vec![], vec![]]);
                assert_eq!(
                    rig.sets(),
                    sets,
                    "`{path}`: a set was issued by focusing and leaving ({leave}): {:?}",
                    rig.shell.engine.journal().last()
                );
                assert_eq!(
                    rig.bits(),
                    before,
                    "`{path}`: the scene changed by focusing and leaving ({leave})"
                );
                visited += 1;
            }
        }
    }
    // The awkward values are still what was set, to the bit.
    let spec = rig.shell.engine.spec();
    assert_eq!(spec.bodies[0].mass.to_bits(), 1.234_567_890_123_456_7_f64.to_bits());
    assert_eq!(spec.array.instrument.gravity.to_bits(), 9.810_000_000_000_002_f64.to_bits());
    println!("{selection}: {visited} visits");
    (rig, visited)
}

// The three selections together made at least 80 visits when they were one test; each is held
// here to the visits it made when the test was split (24 Nov 2026), which sum to more than 80.

#[test]
fn slow_focusing_and_leaving_a_tower_s_editors_changes_nothing() {
    let (_, visited) = leave_every_editor("select tower 1");
    assert!(visited >= TOWER_VISITS, "only {visited} visits were made");
}

#[test]
fn slow_focusing_and_leaving_the_scene_s_editors_changes_nothing() {
    let (_, visited) = leave_every_editor("select none");
    assert!(visited >= SCENE_VISITS, "only {visited} visits were made");
}

#[test]
fn focusing_and_leaving_a_body_s_editors_changes_nothing() {
    let (mut rig, visited) = leave_every_editor("select body 1");
    assert!(visited >= BODY_VISITS, "only {visited} visits were made");
    // Control: the same click does put the field into editing, and typing there is an edit; so
    // the visits above were of fields that could have changed. Select all, type, Enter.
    rig.shell.run("select body 1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let mass = rig.shell.layout().editor("bodies[0].mass").expect("the mass has an editor");
    let sets = rig.sets();
    rig.click(mass.center());
    let select_all = |pressed| Event::Key {
        key: Key::A,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    };
    rig.frames(vec![
        vec![select_all(true)],
        vec![select_all(false)],
        vec![Event::Text("7.5".into())],
        vec![],
    ]);
    rig.key(Key::Enter);
    assert_eq!(rig.shell.engine.spec().bodies[0].mass, 7.5, "typing in the field edits it");
    assert!(rig.sets() > sets, "and the edit is a set in the journal");
}
