//! The editor of S18 (`docs/slices/S18.md`), driven through the same input events a window
//! delivers: adding, removing, duplicating and renaming; the keyboard's reach; the tools, the
//! camera, the panels; and the end-to-end tasks of §4. Each property has a control arm.

use cav_app::headless::{context, run_frames};
use cav_app::{NoSurface, Shell};
use cav_engine::{Command, Target};
use egui::{Event, Key, Modifiers, PointerButton, Pos2};

struct Rig {
    ctx: egui::Context,
    shell: Shell,
    size: [u32; 2],
    clock: f64,
    /// Input events sent, for the tasks' event budgets.
    events: usize,
}

impl Rig {
    fn new() -> Self {
        let mut rig = Self { ctx: context(), shell: Shell::new(), size: [1440, 900], clock: 0.0, events: 0 };
        rig.frames(vec![vec![], vec![]]);
        rig.events = 0;
        rig
    }

    fn frames(&mut self, frames: Vec<Vec<Event>>) {
        self.events += frames.iter().map(Vec::len).sum::<usize>();
        run_frames(&self.ctx, &mut self.shell, &mut NoSurface, self.size, &mut self.clock, frames);
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        let event = |pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
        self.frames(vec![vec![event(true)], vec![event(false)]]);
    }

    fn typed(&mut self, text: &str) {
        self.frames(vec![vec![Event::Text(text.to_owned())], vec![]]);
    }

    fn press(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
        Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE }
    }

    fn click_with(&mut self, pos: Pos2, button: PointerButton) {
        self.frames(vec![
            vec![Event::PointerMoved(pos)],
            vec![Self::press(pos, button, true)],
            vec![Self::press(pos, button, false)],
            vec![],
        ]);
    }

    fn click(&mut self, pos: Pos2) {
        self.click_with(pos, PointerButton::Primary);
    }

    fn click_button(&mut self, line: &str) {
        let rect = self.shell.layout().button(line).unwrap_or_else(|| panic!("no button `{line}`"));
        self.click(rect.center());
    }

    fn row(&self, target: Target) -> egui::Rect {
        self.shell
            .layout()
            .outline
            .iter()
            .find(|(t, _)| *t == target)
            .map(|(_, r)| *r)
            .expect("the row is laid out")
    }
}

#[test]
fn the_empty_scene_says_how_to_begin() {
    let mut rig = Rig::new();
    rig.shell.run("load void").unwrap();
    rig.frames(vec![vec![], vec![]]);
    // The guidance is in the tree, as buttons: not a line pointing at the console.
    let words: Vec<String> = rig.shell.layout().outline_words.iter().map(|(_, l, _)| l.clone()).collect();
    assert!(!words.iter().any(|w| w.contains("load point")), "{words:?}");
    rig.click_button("add point-mass");
    let spec = rig.shell.engine.spec();
    assert_eq!(spec.bodies.len(), 1, "one click added a mass");
    assert_eq!(rig.shell.engine.selection(), Some(Target::Body(1)), "and selected it");
    // It is clear of the towers and a metre above the ground.
    let [x, y, z] = spec.bodies[0].motion.anchor().0;
    assert!(spec.array.towers.iter().all(|t| (t.position[0] - x).hypot(t.position[1] - y) >= 1.0));
    assert_eq!(z, 1.0);
    // One undo takes it away.
    rig.shell.run("undo").unwrap();
    assert!(rig.shell.engine.spec().bodies.is_empty());
    // Control: with a body the "add a mass" button is gone (the Add menu stays).
    rig.shell.run("load point").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert!(rig.shell.layout().button("add point-mass").is_none());
    assert!(rig.shell.layout().button("add").is_some());
}

#[test]
fn delete_and_duplicate_act_on_the_selection_not_in_a_text_field() {
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("select body 1").unwrap();
    rig.frames(vec![vec![]]);
    // Control first: typing in a field, Delete and Ctrl+D are the field's.
    rig.key(Key::Backtick, Modifiers::NONE);
    assert!(rig.shell.console_open);
    rig.frames(vec![vec![], vec![]]);
    rig.key(Key::Delete, Modifiers::NONE);
    rig.key(Key::D, Modifiers::COMMAND);
    assert_eq!(rig.shell.engine.spec().bodies.len(), 1, "a Delete typed in a field removes nothing");
    rig.key(Key::Escape, Modifiers::NONE);
    rig.key(Key::Escape, Modifiers::NONE);
    rig.frames(vec![vec![], vec![]]);
    // In the viewport (where the focus starts) Ctrl+D duplicates the selection, Delete takes it out.
    rig.shell.keyboard.focus = vec!["viewport".into()];
    rig.key(Key::D, Modifiers::COMMAND);
    let spec = rig.shell.engine.spec().clone();
    assert_eq!(spec.bodies.len(), 2, "Ctrl+D duplicated it");
    assert_ne!(spec.bodies[0].label, spec.bodies[1].label);
    assert!(matches!(rig.shell.engine.journal().last(), Some(Command::Insert { .. })));
    rig.key(Key::Delete, Modifiers::NONE);
    assert_eq!(rig.shell.engine.spec().bodies.len(), 1, "Delete took the selected one out");
    assert_eq!(rig.shell.engine.journal().last(), Some(&Command::Remove { path: "bodies[0]".into() }));
    // Each was one edit: two undos bring both back.
    rig.shell.run("undo").unwrap();
    rig.shell.run("undo").unwrap();
    assert_eq!(rig.shell.engine.spec(), &cav_sim::load("point").unwrap());
}

#[test]
fn a_row_s_right_click_menu_acts_on_its_item() {
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let row = rig.row(Target::Tower(2));
    rig.click_with(row.center(), PointerButton::Secondary);
    assert_eq!(rig.shell.engine.selection(), Some(Target::Tower(2)), "a right-click selects its row");
    rig.click_button("context duplicate");
    assert_eq!(rig.shell.engine.spec().array.towers.len(), 6, "the menu's Duplicate copied the tower");
    // Control: the menu's Delete on a body row takes that body, not the tower.
    rig.frames(vec![vec![], vec![]]);
    let row = rig.row(Target::Body(1));
    rig.click_with(row.center(), PointerButton::Secondary);
    rig.click_button("context delete");
    let spec = rig.shell.engine.spec();
    assert!(spec.bodies.is_empty());
    assert_eq!(spec.array.towers.len(), 6);
}

#[test]
fn f2_renames_in_place_and_esc_leaves_the_name() {
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("select body 1").unwrap();
    rig.frames(vec![vec![]]);
    rig.shell.keyboard.focus = vec!["tree".into()];
    rig.key(Key::F2, Modifiers::NONE);
    assert!(rig.shell.renaming.is_some());
    rig.frames(vec![vec![], vec![]]);
    rig.key(Key::A, Modifiers::COMMAND);
    rig.typed("ball");
    rig.key(Key::Enter, Modifiers::NONE);
    assert_eq!(rig.shell.engine.spec().bodies[0].label, "ball");
    assert_eq!(
        rig.shell.engine.journal().last(),
        Some(&Command::set("bodies[0].label", serde_json::json!("ball")))
    );
    // Control: Esc leaves the name as it was and issues nothing.
    let journal = rig.shell.engine.journal().len();
    rig.key(Key::F2, Modifiers::NONE);
    rig.frames(vec![vec![], vec![]]);
    rig.key(Key::A, Modifiers::COMMAND);
    rig.typed("other");
    rig.key(Key::Escape, Modifiers::NONE);
    assert_eq!(rig.shell.engine.spec().bodies[0].label, "ball");
    assert_eq!(rig.shell.engine.journal().len(), journal);
}

#[test]
fn the_palette_adds_and_runs_what_is_typed() {
    let mut rig = Rig::new();
    rig.shell.run("load void").unwrap();
    rig.frames(vec![vec![]]);
    rig.key(Key::K, Modifiers::COMMAND);
    assert!(rig.shell.palette.open);
    rig.frames(vec![vec![], vec![]]);
    rig.typed("add sph");
    rig.key(Key::Enter, Modifiers::NONE);
    assert!(!rig.shell.palette.open, "running closes it");
    let spec = rig.shell.engine.spec();
    assert_eq!(spec.bodies.len(), 1);
    assert!(spec.bodies[0].shape.is_some(), "the voxel sphere was added: {:?}", spec.bodies[0].label);
    assert_eq!(rig.shell.engine.selection(), Some(Target::Body(1)));
    // Control: Esc closes it and runs nothing.
    let journal = rig.shell.engine.journal().len();
    rig.key(Key::K, Modifiers::COMMAND);
    rig.frames(vec![vec![], vec![]]);
    rig.typed("undo");
    rig.key(Key::Escape, Modifiers::NONE);
    assert!(!rig.shell.palette.open);
    assert_eq!(rig.shell.engine.journal().len(), journal);
}
