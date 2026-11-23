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
}

impl Rig {
    fn new() -> Self {
        let mut rig = Self { ctx: context(), shell: Shell::new(), clock: 0.0 };
        rig.frames(vec![vec![], vec![]]);
        rig
    }

    fn frames(&mut self, frames: Vec<Vec<Event>>) {
        run_frames(&self.ctx, &mut self.shell, &mut NoSurface, [1440, 900], &mut self.clock, frames);
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
const AWKWARD: [(&str, &str); 6] = [
    ("bodies[0].mass", "1.2345678901234567"),
    ("bodies[0].radius", "0.30000000000000004"),
    ("bodies[0].motion.Static.position", "[2.0000000000000004, 0.1234567890123456, 2.5000000000000004]"),
    ("duration", "9.000000000000002"),
    ("array.towers[0].timing.offset", "0.10000000000000002"),
    ("array.instrument.gravity", "9.810000000000002"),
];

#[test]
fn focusing_and_leaving_a_number_editor_changes_nothing() {
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    for (path, value) in AWKWARD {
        rig.shell.run(&format!("set {path} {value}")).unwrap();
    }
    let mut visited = 0;
    for selection in ["select body 1", "select tower 1", "select none"] {
        rig.shell.run(selection).unwrap();
        rig.frames(vec![vec![], vec![]]);
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
                        1 => rect.left_center() + vec2(8.0, 0.0),
                        _ => rect.right_center() - vec2(8.0, 0.0),
                    };
                    let (before, sets) = (rig.bits(), rig.sets());
                    rig.click(at);
                    match leave {
                        0 => rig.click(rect.left_center() - vec2(40.0, 0.0)),
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
    }
    assert!(visited >= 80, "only {visited} visits were made");
    // The awkward values are still what was set, to the bit.
    let spec = rig.shell.engine.spec();
    assert_eq!(spec.bodies[0].mass.to_bits(), 1.234_567_890_123_456_7_f64.to_bits());
    assert_eq!(spec.array.instrument.gravity.to_bits(), 9.810_000_000_000_002_f64.to_bits());

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
