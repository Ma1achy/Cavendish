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

    fn drag(&mut self, from: Pos2, to: Pos2) {
        let middle = from + (to - from) * 0.5;
        self.frames(vec![
            vec![Event::PointerMoved(from)],
            vec![Self::press(from, PointerButton::Primary, true)],
            vec![Event::PointerMoved(middle)],
            vec![Event::PointerMoved(to)],
            vec![Self::press(to, PointerButton::Primary, false)],
            vec![],
        ]);
    }

    /// Drags the Move tool's gizmo on body `body` along `axis` by `metres`, from a point on the
    /// arrow, as a person would (the pointer moves `metres` worth of the arrow on screen).
    fn drag_arrow(&mut self, body: usize, axis: usize, metres: f64) {
        let rect = self.shell.layout().panes[0];
        let spec = self.shell.engine.spec();
        let at = spec.bodies[body].motion.position(self.shell.engine.time(), spec.array.instrument.gravity);
        let g =
            cav_app::tools::gizmo(self.shell.engine.view(0), rect.size(), at).expect("the gizmo is in view");
        let tip = g.tips[axis].expect("the arrow lies across the view");
        let arrow = tip - g.centre;
        let per_metre = arrow.length() / g.metres as f32;
        let from = rect.min + g.centre.to_vec2() + arrow * 0.6;
        let to = from + arrow.normalized() * (metres as f32 * per_metre);
        self.drag(from, to);
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

fn anchor(rig: &Rig, body: usize) -> [f64; 3] {
    rig.shell.engine.spec().bodies[body].motion.anchor().0
}

#[test]
fn a_gizmo_drag_is_one_edit() {
    // S18 E2: a drag of an arrow is one journal entry and one undo; snapped to the step.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("select body 1").unwrap();
    rig.shell.run("tool move").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let (start, journal) = (anchor(&rig, 0), rig.shell.engine.journal().len());
    rig.drag_arrow(0, 0, 2.0);
    let moved = anchor(&rig, 0);
    assert!((moved[0] - start[0] - 2.0).abs() <= 1e-12, "{start:?} → {moved:?}");
    assert_eq!((moved[1], moved[2]), (start[1], start[2]), "only along x");
    assert_eq!(rig.shell.engine.journal().len(), journal + 1, "one journal entry");
    rig.shell.run("undo").unwrap();
    assert_eq!(anchor(&rig, 0), start, "one undo takes the whole drag back");
    // Control: with snap off the same drag is not a whole number of steps in general, and is still
    // one edit; a drag along z moves z.
    rig.shell.run("snap").unwrap();
    rig.drag_arrow(0, 2, 0.6);
    let up = anchor(&rig, 0);
    assert!((up[2] - start[2] - 0.6).abs() < 0.02 && up[0] == start[0], "{up:?}");
}

#[test]
fn the_gizmo_the_session_and_the_field_issue_the_same_edit() {
    // S18 E1: dragging the x arrow 2 m, typing 2 in a transform session, and typing x + 2 in the
    // inspector's field give the same scene; the gizmo and the session the same journal line.
    let fresh = || {
        let mut rig = Rig::new();
        rig.shell.run("load point").unwrap();
        rig.shell.run("select body 1").unwrap();
        rig.frames(vec![vec![], vec![]]);
        rig
    };
    let mut by_gizmo = fresh();
    by_gizmo.shell.run("tool move").unwrap();
    by_gizmo.frames(vec![vec![]]);
    by_gizmo.drag_arrow(0, 0, 2.0);
    let mut by_keys = fresh();
    by_keys.shell.keyboard.focus = vec!["viewport".into(), "viewport/selection".into()];
    by_keys.shell.keyboard.nav_on = true;
    by_keys.key(Key::Enter, Modifiers::NONE);
    assert!(by_keys.shell.session.is_some(), "Enter on the selection starts a session");
    by_keys.key(Key::Num2, Modifiers::NONE);
    by_keys.key(Key::Enter, Modifiers::NONE);
    assert!(by_keys.shell.session.is_none());
    let mut by_field = fresh();
    let x = anchor(&by_field, 0)[0];
    by_field.shell.run(&format!("set bodies[0].motion.Static.position[0] {}", x + 2.0)).unwrap();
    let specs = [&by_gizmo, &by_keys, &by_field].map(|rig| rig.shell.engine.spec().clone());
    assert_eq!(specs[0], specs[1], "gizmo and session");
    assert_eq!(specs[0], specs[2], "gizmo and field");
    assert_eq!(
        by_gizmo.shell.engine.journal().last(),
        by_keys.shell.engine.journal().last(),
        "the same line"
    );
    // Control: a drag of the y arrow gives another scene.
    let mut other = fresh();
    other.shell.run("tool move").unwrap();
    other.frames(vec![vec![]]);
    other.drag_arrow(0, 1, 2.0);
    assert_ne!(other.shell.engine.spec(), &specs[0]);
}

#[test]
fn a_held_nudge_is_one_undo_and_esc_cancels_it() {
    // S18 K10: in a transform session a held arrow repeats (500 ms, then every 40 ms), the whole
    // session is one undo; Esc takes it back; Enter keeps it.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("select body 1").unwrap();
    rig.frames(vec![vec![]]);
    let (start, depth) = (anchor(&rig, 0), rig.shell.engine.depth());
    let session = |rig: &mut Rig| {
        rig.shell.keyboard.focus = vec!["viewport".into(), "viewport/selection".into()];
        rig.shell.keyboard.nav_on = true;
        rig.key(Key::Enter, Modifiers::NONE);
        // Hold → for a second: 1 + 1 + 12 steps of 0.25 m.
        let down = Event::Key {
            key: Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let up = Event::Key {
            key: Key::ArrowRight,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let mut frames = vec![vec![down]];
        frames.extend(std::iter::repeat_with(Vec::new).take(59));
        frames.push(vec![up]);
        rig.frames(frames);
    };
    session(&mut rig);
    let held = anchor(&rig, 0);
    assert!((held[0] - start[0] - 14.0 * 0.25).abs() < 1e-9, "{} steps", (held[0] - start[0]) / 0.25);
    rig.key(Key::Escape, Modifiers::NONE);
    assert_eq!(anchor(&rig, 0), start, "Esc cancels: the body is where it began");
    assert_eq!(rig.shell.engine.depth(), depth, "and nothing new can be undone or redone");
    // Control: Enter keeps it, as exactly one step, and one undo takes the whole held run back.
    session(&mut rig);
    rig.key(Key::Enter, Modifiers::NONE);
    assert!((anchor(&rig, 0)[0] - start[0] - 3.5).abs() < 1e-9);
    assert_eq!(rig.shell.engine.depth(), (depth.0 + 1, 0), "Enter kept one step");
    rig.shell.run("undo").unwrap();
    assert_eq!(anchor(&rig, 0), start, "one undo for the whole session");
}

#[test]
fn the_place_tool_drops_a_starter_where_the_pointer_is() {
    // S18 E3: a click with the Place tool inserts a valid body at the ground under the pointer.
    let mut rig = Rig::new();
    rig.shell.run("load void").unwrap();
    rig.shell.run("tool place").unwrap();
    rig.shell.place_starter = "cube".into();
    rig.frames(vec![vec![], vec![]]);
    let rect = rig.shell.layout().panes[0];
    let pointer = rect.min + egui::vec2(0.62 * rect.width(), 0.7 * rect.height());
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let within = [f64::from(pointer.x - rect.min.x), f64::from(pointer.y - rect.min.y)];
    let expected = cav_app::tools::on_ground(rig.shell.engine.view(0), size, within, rig.shell.ground());
    rig.click(pointer);
    let spec = rig.shell.engine.spec();
    assert_eq!(spec.bodies.len(), 1, "one click, one body");
    let [x, y, z] = spec.bodies[0].motion.anchor().0;
    assert!((x - expected[0]).hypot(y - expected[1]) < 1e-3, "({x}, {y}) against {expected:?}");
    assert_eq!(z, rig.shell.ground() + 0.25, "the cube rests on the ground");
    assert_eq!(rig.shell.engine.selection(), Some(Target::Body(1)));
    rig.shell.run("undo").unwrap();
    assert!(rig.shell.engine.spec().bodies.is_empty(), "one undo takes it away");
    // Control: with the Select tool the same click adds nothing.
    rig.shell.run("load void").unwrap();
    rig.shell.run("tool select").unwrap();
    rig.frames(vec![vec![]]);
    rig.click(pointer);
    assert!(rig.shell.engine.spec().bodies.is_empty());
}

#[test]
fn choosing_a_tool_changes_no_scene() {
    // S18 E5: tools are UI state.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    let (digest, journal) = (rig.shell.engine.digest(), rig.shell.engine.journal().len());
    rig.shell.keyboard.focus = vec!["toolbar".into()];
    for key in [Key::W, Key::A, Key::Q, Key::S, Key::S] {
        rig.key(key, Modifiers::NONE);
    }
    rig.click_button("tool move");
    assert_eq!(rig.shell.tool, "move");
    assert_eq!((rig.shell.engine.digest(), rig.shell.engine.journal().len()), (digest.clone(), journal));
    // Control: a tool's action does change the scene (Place), and is journalled.
    rig.shell.run("tool place").unwrap();
    rig.frames(vec![vec![]]);
    let rect = rig.shell.layout().panes[0];
    rig.click(rect.center() + egui::vec2(0.0, 0.3 * rect.height()));
    assert_ne!(rig.shell.engine.digest(), digest);
}

#[test]
fn task_add_a_sphere_move_it_two_metres_read_tower_one() {
    // S18 T1, end to end through the UI: the palette adds a sphere, the Move tool's arrow takes it
    // 2 m east, and tower 1 reads what the forward model reads of the scene as edited.
    let mut rig = Rig::new();
    rig.shell.run("load room").unwrap();
    rig.frames(vec![vec![]]);
    rig.events = 0;
    rig.key(Key::K, Modifiers::COMMAND);
    rig.frames(vec![vec![]]);
    rig.typed("add sph");
    rig.key(Key::Enter, Modifiers::NONE);
    let spec = rig.shell.engine.spec();
    assert_eq!(spec.bodies.len(), 1);
    assert_eq!(rig.shell.engine.selection(), Some(Target::Body(1)), "the new sphere is selected");
    rig.key(Key::W, Modifiers::NONE);
    assert_eq!(rig.shell.tool, "move", "W: the Move tool");
    rig.frames(vec![vec![]]);
    let start = anchor(&rig, 0);
    // One undo per step: the add, so far (loading the room the window opened with changed
    // nothing, so it is no step).
    assert_eq!(rig.shell.engine.depth(), (1, 0), "the add");
    // North (y): east would take it onto tower 2's axis at (4, 0), where the read is the
    // through-the-tower case (S5's bounded walk; docs/slices/S18.md §7).
    rig.drag_arrow(0, 1, 2.0);
    let moved = anchor(&rig, 0);
    assert!((moved[1] - start[1] - 2.0).abs() <= 1e-12, "moved by {}", moved[1] - start[1]);
    assert_eq!((moved[0], moved[2]), (start[0], start[2]));
    assert_eq!(rig.shell.engine.depth(), (2, 0), "the drag is one step more");
    // The reading the window shows is the forward model's of the edited scene, bit for bit.
    assert!(rig.shell.engine.settle_reading(std::time::Duration::from_secs(120)));
    let shown = rig.shell.reading().clone();
    let direct = cav_sim::read::reading(rig.shell.engine.spec(), shown.time, &|| false).expect("read");
    assert_eq!(shown.towers[0].gradiometer.to_bits(), direct.towers[0].gradiometer.to_bits(), "tower 1's ΔΦ");
    assert!(shown.towers[0].gradiometer != 0.0);
    // The event budget: recorded at the first pass (docs/slices/S18.md §7) and held.
    assert!(rig.events <= TASK_EVENTS, "{} input events", rig.events);
}

#[test]
fn task_keyboard_only() {
    // S18 T3: with the mouse put away, select tower 2, add a body, move it 1 m, change its mass.
    let mut rig = Rig::new();
    rig.shell.run("load room").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let pointer = |frames: &[Vec<Event>]| {
        frames.iter().flatten().any(|e| matches!(e, Event::PointerButton { .. } | Event::PointerMoved(_)))
    };
    let mut sent: Vec<Vec<Event>> = Vec::new();
    let mut key = |rig: &mut Rig, key: Key, modifiers: Modifiers| {
        let event = |pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
        sent.push(vec![event(true)]);
        sent.push(vec![event(false)]);
        rig.key(key, modifiers);
    };
    // Tower 2: its digit, in the viewport where the focus starts.
    key(&mut rig, Key::Num2, Modifiers::NONE);
    assert_eq!(rig.shell.engine.selection(), Some(Target::Tower(2)));
    // A body: the palette, typed, Enter.
    key(&mut rig, Key::K, Modifiers::COMMAND);
    rig.frames(vec![vec![]]);
    rig.typed("point mass");
    key(&mut rig, Key::Enter, Modifiers::NONE);
    assert_eq!(rig.shell.engine.spec().bodies.len(), 1);
    assert_eq!(rig.shell.engine.selection(), Some(Target::Body(1)));
    // Move it 1 m: Tab round to the viewport, its Selection stop, a session, type 1, Enter.
    let start = anchor(&rig, 0);
    while rig.shell.keyboard.focus != ["viewport"] {
        key(&mut rig, Key::Tab, Modifiers::NONE);
    }
    key(&mut rig, Key::Enter, Modifiers::NONE);
    while rig.shell.keyboard.focused() != "viewport/selection" {
        key(&mut rig, Key::ArrowRight, Modifiers::NONE);
    }
    key(&mut rig, Key::Enter, Modifiers::NONE);
    key(&mut rig, Key::Num1, Modifiers::NONE);
    key(&mut rig, Key::Enter, Modifiers::NONE);
    let moved = anchor(&rig, 0);
    assert!((moved[0] - start[0] - 1.0).abs() < 1e-12, "{start:?} → {moved:?}");
    // Its mass: Tab to the inspector, in, down to the mass, adjust it up by one step.
    while rig.shell.keyboard.focus != ["inspector"] {
        key(&mut rig, Key::Tab, Modifiers::NONE);
    }
    key(&mut rig, Key::Enter, Modifiers::NONE);
    key(&mut rig, Key::Enter, Modifiers::NONE);
    while rig.shell.keyboard.focused() != "field/bodies[0].mass" {
        key(&mut rig, Key::ArrowDown, Modifiers::NONE);
    }
    let mass = rig.shell.engine.spec().bodies[0].mass;
    key(&mut rig, Key::Enter, Modifiers::NONE);
    key(&mut rig, Key::ArrowUp, Modifiers::NONE);
    key(&mut rig, Key::Escape, Modifiers::NONE);
    assert!(rig.shell.engine.spec().bodies[0].mass > mass, "the mass went up a step");
    assert!(!pointer(&sent), "no pointer event was sent");
    // The breadcrumb said where the keys were: the inspector, the section by what it is (the
    // body), the field by its name in words (S18's review, U4: it was "Inspector › mass › mass",
    // the section named after the body's label).
    assert_eq!(rig.shell.breadcrumb().as_deref(), Some("Inspector › Body › Mass"));
}

/// The input events the T1 task takes: 12, counted at its first pass (docs/slices/S18.md §7.2)
/// and held, so the task does not get longer unnoticed. (It was 20, a looser budget than the
/// count §7.2 recorded: S18's review, R5.)
const TASK_EVENTS: usize = 12;

/// A file in the system's temporary folder for one test, removed when dropped.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        Self(std::env::temp_dir().join(format!("cavendish-s18-{}-{name}", std::process::id())))
    }

    fn path(&self) -> String {
        self.0.to_string_lossy().replace('\\', "/")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn reset_puts_a_field_back_and_the_title_says_unsaved() {
    // S18 E7.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("select body 1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert!(!rig.shell.title().contains('*'), "{}", rig.shell.title());
    let mass = rig.shell.engine.spec().bodies[0].mass;
    rig.shell.run("set bodies[0].mass 5").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert!(rig.shell.title().ends_with(" *"), "an edit: unsaved");
    // Backspace on the field puts it back: one edit, and the mark goes.
    let tree = rig.shell.focus_tree().clone();
    rig.shell.keyboard.focus_on(&tree, "field/bodies[0].mass");
    assert_eq!(rig.shell.keyboard.focused(), "field/bodies[0].mass", "the field is in the keyboard's tree");
    rig.shell.keyboard.nav_on = true;
    rig.key(Key::Backspace, Modifiers::NONE);
    assert_eq!(rig.shell.engine.spec().bodies[0].mass, mass);
    assert!(!rig.shell.title().contains('*'));
    // Control: the reset of an unchanged field is disabled and does nothing.
    let journal = rig.shell.engine.journal().len();
    let reset = rig.shell.focus_tree().node("field/bodies[0].mass/reset").expect("the reset keeps its place");
    assert!(!reset.enabled);
    rig.key(Key::Backspace, Modifiers::NONE);
    assert_eq!(rig.shell.engine.journal().len(), journal + 1, "a set to the same value: journalled, no step");
    // Saving clears the mark: the saved scene is what fields reset to from now on.
    rig.shell.run("set bodies[0].mass 7").unwrap();
    let file = Scratch::new("saved.ron");
    rig.shell.run(&format!("save {}", file.path())).unwrap();
    assert!(!rig.shell.engine.unsaved());
    rig.shell.run("set bodies[0].mass 9").unwrap();
    rig.shell.run("reset bodies[0].mass").unwrap();
    assert_eq!(rig.shell.engine.spec().bodies[0].mass, 7.0, "reset to the saved value, not the loaded one");
}

#[test]
fn the_history_list_goes_to_a_row() {
    // S18 E8.
    let mut rig = Rig::new();
    for d in [13, 14, 15] {
        rig.shell.run(&format!("set duration {d}")).unwrap();
    }
    rig.key(Key::H, Modifiers::COMMAND);
    assert!(rig.shell.history_open);
    rig.frames(vec![vec![], vec![]]);
    let (done, undone) = rig.shell.engine.steps();
    assert_eq!(done, ["set duration 13", "set duration 14", "set duration 15"]);
    assert!(undone.is_empty());
    rig.click_button("history 1");
    assert_eq!(rig.shell.engine.spec().duration, 13.0, "to the first edit");
    rig.frames(vec![vec![]]);
    rig.click_button("history 3");
    assert_eq!(rig.shell.engine.spec().duration, 15.0, "and forward to the last");
    rig.frames(vec![vec![]]);
    rig.click_button("history 0");
    assert_eq!(rig.shell.engine.spec().duration, cav_sim::SceneSpec::room().duration, "to before them all");
    // Control: Esc closes it and the scene stays.
    rig.shell.keyboard.focus = vec!["history".into(), "history/start".into()];
    rig.key(Key::Escape, Modifiers::NONE);
    assert!(!rig.shell.history_open);
}

#[test]
fn task_build_a_scene_from_the_wizard() {
    // S18 T2: one tower on a floor with a voxel sphere, by clicks, one undo away.
    let mut rig = Rig::new();
    rig.key(Key::N, Modifiers::COMMAND);
    rig.frames(vec![vec![], vec![]]);
    for button in ["wizard array 1", "wizard surroundings 1", "wizard body sphere", "wizard create"] {
        rig.click_button(button);
    }
    let spec = rig.shell.engine.spec().clone();
    assert_eq!((spec.array.towers.len(), spec.room.is_some(), spec.floor.is_some()), (1, false, true));
    assert_eq!(spec.bodies.len(), 1);
    assert!(spec.bodies[0].shape.is_some());
    assert!(rig.shell.wizard.is_none(), "the wizard closed");
    rig.shell.run("undo").unwrap();
    assert_eq!(rig.shell.engine.spec(), &cav_sim::SceneSpec::room(), "one undo takes the scene away");
    // Control: cancelled, nothing is made.
    rig.key(Key::N, Modifiers::COMMAND);
    rig.frames(vec![vec![]]);
    rig.shell.keyboard.focus = vec!["wizard".into(), "wizard/array".into()];
    rig.key(Key::Escape, Modifiers::NONE);
    assert!(rig.shell.wizard.is_none());
    assert_eq!(rig.shell.engine.spec(), &cav_sim::SceneSpec::room());
}

#[test]
fn task_save_and_reopen_gives_the_same_scene() {
    // S18 T2: save through the Save window, open something else, open the file again.
    let mut rig = Rig::new();
    rig.shell.run("load comb").unwrap();
    rig.shell.run("set bodies[0].mass 3.5").unwrap();
    let saved = rig.shell.engine.spec().clone();
    let file = Scratch::new("reopen.ron");
    // By input events only (S18's review, R5): Ctrl+S, the path typed over what is offered,
    // Enter; Ctrl+O, the path typed, Enter.
    rig.key(Key::S, Modifiers::COMMAND);
    assert!(rig.shell.save_window.is_some(), "no file yet: Ctrl+S asks for one");
    rig.frames(vec![vec![]]);
    assert_eq!(rig.shell.keyboard.focused(), "save/path", "the window lands on its path");
    rig.key(Key::Enter, Modifiers::NONE);
    rig.frames(vec![vec![]]);
    rig.key(Key::A, Modifiers::COMMAND);
    rig.typed(&file.path());
    rig.key(Key::Enter, Modifiers::NONE);
    assert!(rig.shell.save_window.is_none() && !rig.shell.engine.unsaved());
    assert_eq!(rig.shell.file.as_deref(), Some(file.path().as_str()));
    rig.shell.run("load room").unwrap();
    rig.key(Key::O, Modifiers::COMMAND);
    rig.frames(vec![vec![]]);
    assert_eq!(rig.shell.keyboard.focused(), "open/path");
    rig.key(Key::Enter, Modifiers::NONE);
    rig.frames(vec![vec![]]);
    rig.typed(&file.path());
    rig.key(Key::Enter, Modifiers::NONE);
    assert_eq!(rig.shell.engine.spec(), &saved, "the same scene, to the bit");
    // Control: Ctrl+S now saves to the file without asking.
    rig.shell.run("set bodies[0].mass 4").unwrap();
    rig.key(Key::S, Modifiers::COMMAND);
    assert!(rig.shell.save_window.is_none() && !rig.shell.engine.unsaved());
    assert_eq!(cav_sim::load(&file.path()).unwrap().bodies[0].mass, 4.0);
}

fn overlap(a: egui::Rect, b: egui::Rect) -> f32 {
    let i = a.intersect(b);
    if i.is_positive() { i.area() } else { 0.0 }
}

const BIG: [&str; 7] = ["top-bar", "toolbar", "tree", "viewport", "inspector", "timeline", "footer"];

#[test]
fn panels_resize_and_maximise_restores() {
    // S18 W1.
    let mut rig = Rig::new();
    rig.frames(vec![vec![], vec![]]);
    let outliner = rig.shell.layout().panel("outliner").unwrap();
    let edge = egui::pos2(outliner.right() - 1.0, outliner.center().y);
    rig.drag(edge, edge + egui::vec2(40.0, 0.0));
    rig.frames(vec![vec![], vec![]]);
    let wider = rig.shell.layout().panel("outliner").unwrap();
    assert!(
        (wider.width() - outliner.width() - 40.0).abs() <= 1.0,
        "{} → {}",
        outliner.width(),
        wider.width()
    );
    // It stays as it was left.
    rig.frames(vec![vec![], vec![], vec![]]);
    assert_eq!(rig.shell.layout().panel("outliner").unwrap(), wider);
    // Ctrl+Space maximises the active pane: only the menu bar and the status line are left.
    let before = rig.shell.layout().clone();
    rig.key(Key::Space, Modifiers::COMMAND);
    assert!(rig.shell.maximised);
    let names: Vec<&str> = rig.shell.layout().panels.iter().map(|(n, _)| *n).collect();
    assert_eq!(names, ["menu", "footer"], "{names:?}");
    assert!(rig.shell.layout().figure.area() > 1.5 * before.figure.area());
    assert!(!rig.shell.playing, "Ctrl+Space is not Space");
    // Again: the window is as it was, to the point.
    rig.key(Key::Space, Modifiers::COMMAND);
    rig.frames(vec![vec![]]);
    assert_eq!(rig.shell.layout().panels, before.panels);
    assert_eq!(rig.shell.layout().figure, before.figure);
    // Control: maximised, Tab onto a hidden scope (the tree) restores the window first.
    rig.key(Key::Space, Modifiers::COMMAND);
    rig.shell.keyboard.focus = vec!["toolbar".into()];
    rig.key(Key::Tab, Modifiers::NONE);
    assert_eq!(rig.shell.keyboard.focus, ["tree"]);
    assert!(!rig.shell.maximised, "Tab onto the tree showed it");
}

#[test]
fn every_layout_keeps_every_scope_and_the_figure_uncovered() {
    // S18 W2 and K4: in every layout, and maximised, the figure is never covered, every big scope
    // is in the tree, and Tab walks them in the same order.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    let mut states: Vec<String> = cav_app::layouts::layouts().layouts.iter().map(|l| l.id.clone()).collect();
    states.push("maximised".into());
    for state in &states {
        if state == "maximised" {
            rig.shell.run("layout scene").unwrap();
            rig.shell.run("maximise").unwrap();
        } else {
            rig.shell.run(&format!("layout {state}")).unwrap();
        }
        rig.frames(vec![vec![], vec![]]);
        let layout = rig.shell.layout().clone();
        for (name, rect) in &layout.panels {
            assert!(overlap(*rect, layout.figure) < 1.0, "{state}: {name} covers the figure");
        }
        assert_eq!(rig.shell.focus_tree().big_scopes(), BIG, "{state}");
        if state != "maximised" {
            for scope in BIG {
                let node = rig.shell.focus_tree().node(scope).unwrap();
                assert!(node.rect.is_some(), "{state}: {scope} is on screen");
            }
        }
        // Tab walks the seven in order, twice round, whatever is selected and maximised (K4; S18's
        // review, R5: the walk was skipped maximised and never made with a selection).
        for selection in ["select none", "select body 1"] {
            if state == "maximised" {
                rig.shell.maximised = true;
            }
            rig.shell.run(selection).unwrap();
            rig.frames(vec![vec![], vec![]]);
            let mut seen = Vec::new();
            rig.shell.keyboard.focus = vec!["footer".into()];
            for _ in 0..14 {
                rig.key(Key::Tab, Modifiers::NONE);
                seen.push(rig.shell.keyboard.focused().to_owned());
            }
            assert_eq!(seen, [BIG, BIG].concat(), "{state}, `{selection}`: Tab's order");
        }
    }
    // Control: a walk that skipped what is hidden would not be the seven (the maximised window
    // hides five of them, and Tab showed each as it landed).
    assert!(!rig.shell.maximised, "Tab onto a hidden scope restored the window");
    // The layouts arranged the figure as their lines say.
    rig.shell.run("layout analysis").unwrap();
    assert_eq!(rig.shell.engine.panes(), 4);
    assert_eq!(rig.shell.engine.content(0), cav_engine::PaneContent::Signal);
    // A layout's lines are view commands, never edits: the history is as deep as the load made
    // it. (S18's review, R3: `history()` was (true, false) whatever the depth.)
    assert_eq!(rig.shell.engine.depth(), (1, 0));
    // Control: the measure sees an edit, and a layout with one in its lines is refused.
    rig.shell.run("set duration 5").unwrap();
    assert_eq!(rig.shell.engine.depth(), (2, 0));
    let edited = cav_app::layouts::LAYOUTS_RON.replace("\"pane 1\"]", "\"pane 1\", \"set duration 5\"]");
    assert!(cav_app::layouts::Layouts::parse(&edited).is_err(), "a layout may not edit the scene");
}

/// What share of the pane's shorter side a sphere (`centre`, `radius` metres) spans on screen, in
/// pane `pane` of the last frame: twice the farthest its outline reaches from its centre's
/// picture, over points all round the sphere (its silhouette, which in perspective is wider than
/// its radius at its centre's depth: S18's review, U5).
fn spans(rig: &Rig, pane: usize, centre: [f64; 3], radius: f64) -> f64 {
    let rect = rig.shell.layout().panes[pane];
    let view = rig.shell.engine.view(pane).clone();
    let camera = cav_render::Camera::new(&view, f64::from(rect.width() / rect.height()));
    let whole = [rect.width().round() as u32, rect.height().round() as u32];
    let at = |p: cav_core::Vec3| camera.pixel(p, whole).expect("in front of the camera");
    let c = cav_core::Vec3::from_array(centre);
    let [x0, y0] = at(c);
    // A Fibonacci sphere of points: the outline's farthest point within a fraction of a pixel.
    let n = 4000;
    let golden = std::f64::consts::PI * (3.0 - 5f64.sqrt());
    let reach = (0..n)
        .map(|i| {
            let z = 1.0 - 2.0 * (f64::from(i) + 0.5) / f64::from(n);
            let ring = (1.0 - z * z).sqrt();
            let a = golden * f64::from(i);
            let [x, y] = at(c + cav_core::Vec3::new(ring * a.cos(), ring * a.sin(), z) * radius);
            (x - x0).hypot(y - y0)
        })
        .fold(0.0, f64::max);
    2.0 * reach / f64::from(rect.width().min(rect.height()))
}

#[test]
fn f_frames_the_selection() {
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("select body 1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let (centre, radius) =
        cav_app::camera::bounds_of(rig.shell.engine.spec(), Some(Target::Body(1)), rig.shell.engine.time());
    // Control: before framing, the small mass is a speck in the room's view.
    let before = spans(&rig, 0, centre, radius);
    assert!(before < 0.4, "{before}");
    rig.shell.keyboard.focus = vec!["viewport".into()];
    rig.key(Key::F, Modifiers::NONE);
    let after = spans(&rig, 0, centre, radius);
    assert!((0.4..=0.8).contains(&after), "the framed body spans {after} of the pane (C3: 0.4 to 0.8)");
    // And it is what framing aims at, the outline at FILL of the shorter side, to 2% (it was
    // 0.79, the edge at the centre's depth measured where the silhouette is wider, and the
    // field of a square picture used for a wide one: S18's review, U5).
    assert!((after - cav_app::camera::FILL).abs() < 0.02, "{after}");
    assert!(matches!(
        rig.shell.engine.journal().last(),
        Some(Command::View(cav_engine::ViewCommand::Camera { .. }))
    ));
    assert_eq!(rig.shell.engine.depth(), (1, 0), "framing is no edit: only the load can be undone");
    // Control: Home, the one camera move that is an edit, is seen by the same measure.
    rig.shell.run("view home").unwrap();
    assert_eq!(rig.shell.engine.depth(), (2, 0));
    rig.shell.run("undo").unwrap();
    // A tower frames too, from the tree.
    rig.shell.run("select tower 3").unwrap();
    rig.shell.keyboard.focus = vec!["tree".into()];
    rig.key(Key::F, Modifiers::NONE);
    let (centre, radius) = cav_app::camera::bounds_of(rig.shell.engine.spec(), Some(Target::Tower(3)), 0.0);
    let tower = spans(&rig, 0, centre, radius);
    assert!((0.4..=0.8).contains(&tower), "{tower}");
    assert!((tower - cav_app::camera::FILL).abs() < 0.02, "{tower}");
    // In a narrow pane too (two side by side), and orthographic.
    rig.shell.run("panes 2").unwrap();
    rig.shell.run("proj ortho").unwrap();
    rig.frames(vec![vec![], vec![]]);
    rig.key(Key::F, Modifiers::NONE);
    let narrow = spans(&rig, 0, centre, radius);
    assert!((narrow - cav_app::camera::FILL).abs() < 0.02, "{narrow}");
}

#[test]
fn the_wheel_zooms_toward_the_pointer() {
    let mut rig = Rig::new();
    rig.shell.run("load room").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let pane = rig.shell.layout().panes[0];
    let pointer = pane.min + egui::vec2(0.8 * pane.width(), 0.25 * pane.height());
    let size = [f64::from(pane.width()), f64::from(pane.height())];
    let within = [f64::from(pointer.x - pane.min.x), f64::from(pointer.y - pane.min.y)];
    let start = rig.shell.engine.view(0).clone();
    let point = cav_app::camera::under_pointer(&start, size, within);
    let wheel = Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, 120.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: Modifiers::NONE,
    };
    rig.frames(vec![vec![Event::PointerMoved(pointer)], vec![wheel], vec![], vec![], vec![], vec![]]);
    let after = rig.shell.engine.view(0).clone();
    assert!(after.distance < start.distance, "the wheel zoomed in");
    let pixel = |view: &cav_sim::ViewSpec| {
        let camera = cav_render::Camera::new(view, size[0] / size[1]);
        camera.pixel(cav_core::Vec3::from_array(point), [pane.width() as u32, pane.height() as u32]).unwrap()
    };
    let [x0, y0] = pixel(&start);
    let [x1, y1] = pixel(&after);
    let moved = (x1 - x0).hypot(y1 - y0);
    assert!(moved < 1.0, "the point under the pointer moved {moved} px (C4: under 1)");
    // Control: the old zoom about the target, to the same distance, moves it a long way.
    let about_target = cav_sim::ViewSpec { distance: after.distance, ..start.clone() };
    let [x2, y2] = pixel(&about_target);
    assert!((x2 - x0).hypot(y2 - y0) > 20.0, "the control moved it {} px", (x2 - x0).hypot(y2 - y0));
}

#[test]
fn fly_is_a_session() {
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.frames(vec![vec![]]);
    let start = rig.shell.engine.view(0).clone();
    let depth = rig.shell.engine.depth();
    // Outside a session W does not move the camera.
    rig.shell.keyboard.focus = vec!["viewport".into()];
    rig.key(Key::W, Modifiers::NONE);
    assert_eq!(rig.shell.engine.view(0).target, start.target);
    rig.key(Key::F, Modifiers::SHIFT);
    assert!(rig.shell.flying());
    // Held, W flies forward along the view.
    let down = Event::Key {
        key: Key::W,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    let up = Event::Key {
        key: Key::W,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    rig.frames(vec![vec![down], vec![], vec![], vec![], vec![up]]);
    let flown = rig.shell.engine.view(0).clone();
    let (to_eye, _, _) = start.basis();
    let moved = [
        flown.target[0] - start.target[0],
        flown.target[1] - start.target[1],
        flown.target[2] - start.target[2],
    ];
    let along = -(moved[0] * to_eye[0] + moved[1] * to_eye[1] + moved[2] * to_eye[2]);
    assert!(along > 0.05, "flew {along} m forward");
    assert_eq!((flown.yaw, flown.pitch, flown.distance), (start.yaw, start.pitch, start.distance));
    // A viewport letter does nothing mid-flight (O would swap the projection).
    rig.key(Key::O, Modifiers::NONE);
    assert_eq!(rig.shell.engine.view(0).projection, start.projection);
    // Esc stops, the camera kept where it was flown; then O is the projection's again.
    rig.key(Key::Escape, Modifiers::NONE);
    assert!(!rig.shell.flying());
    assert_eq!(rig.shell.engine.view(0).target, flown.target);
    rig.key(Key::O, Modifiers::NONE);
    assert_ne!(rig.shell.engine.view(0).projection, start.projection);
    // None of it was an edit (S18's review, R3: counted, not `history()`'s two flags).
    assert_eq!(rig.shell.engine.depth(), depth);
    // Control: an edit is seen by the same count.
    rig.shell.run("set duration 13").unwrap();
    assert_eq!(rig.shell.engine.depth(), (depth.0 + 1, 0));
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

/// Holds a key down for `frames` frames of the rig's clock, then lets it go.
fn hold(rig: &mut Rig, key: Key, frames: usize) {
    let event =
        |pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE };
    let mut sent = vec![vec![event(true)]];
    sent.extend(std::iter::repeat_with(Vec::new).take(frames));
    sent.push(vec![event(false)]);
    rig.frames(sent);
}

/// A rig on `point` with a second mass at (3, 3, 1), body 1 selected and the keyboard on its
/// handle in the viewport.
fn two_masses() -> Rig {
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell
        .run(r#"add bodies {"label":"other","mass":5.0,"motion":{"Static":{"position":[3.0,3.0,1.0]}}}"#)
        .unwrap();
    rig.shell.run("select body 1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    rig.shell.keyboard.focus = vec!["viewport".into(), "viewport/selection".into()];
    rig.shell.keyboard.nav_on = true;
    rig
}

#[test]
fn delete_inside_a_transform_session_removes_nothing_and_esc_moves_no_other_body() {
    // S18's review, R1: in a session, Delete and Ctrl+D are the session's (eaten); Esc then
    // puts back the body it moved, and never moves another into its place.
    let mut rig = two_masses();
    let (start, other) = (anchor(&rig, 0), anchor(&rig, 1));
    rig.key(Key::Enter, Modifiers::NONE);
    assert!(rig.shell.session.is_some());
    rig.key(Key::ArrowRight, Modifiers::NONE);
    rig.key(Key::Delete, Modifiers::NONE);
    assert_eq!(rig.shell.engine.spec().bodies.len(), 2, "Delete did nothing mid-move");
    rig.key(Key::D, Modifiers::COMMAND);
    assert_eq!(rig.shell.engine.spec().bodies.len(), 2, "nor Ctrl+D");
    rig.key(Key::Escape, Modifiers::NONE);
    assert_eq!((anchor(&rig, 0), anchor(&rig, 1)), (start, other), "Esc put back the one it moved");
    // Control: the same Delete outside a session takes the selected body out.
    rig.key(Key::Delete, Modifiers::NONE);
    assert_eq!(rig.shell.engine.spec().bodies.len(), 1);
    assert_eq!(anchor(&rig, 0), other, "the other body, untouched");
}

#[test]
fn a_session_whose_body_has_gone_moves_nothing() {
    // R1's second half: the body at the session's index is checked before every move and at
    // Esc; a console `remove` mid-session (the one way left to take it) leaves the other alone.
    let mut rig = two_masses();
    let other = anchor(&rig, 1);
    rig.key(Key::Enter, Modifiers::NONE);
    rig.key(Key::ArrowRight, Modifiers::NONE);
    rig.shell.run("remove bodies[0]").unwrap();
    rig.key(Key::ArrowRight, Modifiers::NONE);
    rig.key(Key::Escape, Modifiers::NONE);
    assert_eq!(rig.shell.engine.spec().bodies.len(), 1);
    assert_eq!(anchor(&rig, 0), other, "the body now at index 0 was neither moved nor put back");
    assert!(rig.shell.session.is_none());
    // Control: with its body there, Esc does put it back.
    let mut rig = two_masses();
    let start = anchor(&rig, 0);
    rig.key(Key::Enter, Modifiers::NONE);
    rig.key(Key::ArrowRight, Modifiers::NONE);
    assert_ne!(anchor(&rig, 0), start);
    rig.key(Key::Escape, Modifiers::NONE);
    assert_eq!(anchor(&rig, 0), start);
}

#[test]
fn a_global_key_ends_a_session_first_and_undo_is_its_cancel() {
    // R1: a global (the only key that reaches an action from a session) cancels the session
    // before it runs; Ctrl+Z after a move is that cancel and takes back nothing more.
    let mut rig = two_masses();
    rig.shell.run("set duration 20").unwrap();
    let (start, depth) = (anchor(&rig, 0), rig.shell.engine.depth());
    rig.key(Key::Enter, Modifiers::NONE);
    rig.key(Key::ArrowUp, Modifiers::NONE);
    rig.key(Key::Z, Modifiers::COMMAND);
    assert!(rig.shell.session.is_none() && rig.shell.keyboard.session.is_none());
    assert_eq!(anchor(&rig, 0), start, "the move was taken back");
    assert_eq!(rig.shell.engine.spec().duration, 20.0, "and the edit before the session kept");
    assert_eq!(rig.shell.engine.depth().0, depth.0);
    // Control: Ctrl+Z outside a session undoes that edit.
    rig.key(Key::Z, Modifiers::COMMAND);
    assert_ne!(rig.shell.engine.spec().duration, 20.0);
}

#[test]
fn enter_on_the_handle_with_a_tower_selected_starts_no_session() {
    // S18's review, minor 4: the handle is disabled with no body; even reached, it starts nothing.
    let mut rig = two_masses();
    rig.shell.run("select tower 2").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert!(!rig.shell.focus_tree().node("viewport/selection").unwrap().enabled);
    rig.key(Key::Enter, Modifiers::NONE);
    assert!(rig.shell.session.is_none());
    assert_eq!(
        rig.shell.keyboard.mode,
        cav_app::keyboard::Mode::Navigation,
        "no blue ring with nothing to move"
    );
    // Control: with a body selected Enter starts one.
    rig.shell.run("select body 1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    rig.key(Key::Enter, Modifiers::NONE);
    assert!(rig.shell.session.is_some());
}

#[test]
fn the_selection_handle_keeps_its_place_whatever_the_panes_show() {
    // S18's review, minor 5: in the Analysis layout (pane 1 a plot) the viewport's stops are the
    // same, in the same order, as in the Scene layout.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("select body 1").unwrap();
    let stops = |rig: &mut Rig, layout: &str| {
        rig.shell.run(&format!("layout {layout}")).unwrap();
        rig.frames(vec![vec![], vec![]]);
        rig.shell.focus_tree().children("viewport").iter().map(|s| (*s).to_owned()).collect::<Vec<_>>()
    };
    let scene = stops(&mut rig, "scene");
    let analysis = stops(&mut rig, "analysis");
    assert!(scene.iter().any(|s| s == "viewport/selection"), "{scene:?}");
    assert_eq!(scene, analysis);
    // Control: there, it is disabled (pane 1 shows a plot: nothing to move in it).
    assert!(!rig.shell.focus_tree().node("viewport/selection").unwrap().enabled);
}

#[test]
fn a_drag_or_a_session_while_playing_is_one_undo() {
    // S18's review, R2: the scene plays (a seek every frame between the drag's steps); a gizmo
    // drag is still one step, and a session cancelled with Esc leaves none. Control: paused.
    let run = |playing: bool| {
        let mut rig = Rig::new();
        rig.shell.run("load point").unwrap();
        rig.shell.run("select body 1").unwrap();
        rig.shell.run("tool move").unwrap();
        rig.frames(vec![vec![], vec![]]);
        rig.shell.playing = playing;
        let (start, depth) = (anchor(&rig, 0), rig.shell.engine.depth());
        rig.drag_arrow(0, 0, 2.0);
        assert!(rig.shell.engine.time() > 0.0 || !playing, "it played");
        assert!((anchor(&rig, 0)[0] - start[0] - 2.0).abs() < 1e-12);
        let after_drag = rig.shell.engine.depth();
        rig.shell.run("undo").unwrap();
        assert_eq!(anchor(&rig, 0), start, "one undo took the drag back (playing: {playing})");
        rig.shell.run("redo").unwrap();
        // A session of three presses, then Esc.
        let before = rig.shell.engine.depth();
        rig.shell.keyboard.focus = vec!["viewport".into(), "viewport/selection".into()];
        rig.shell.keyboard.nav_on = true;
        rig.key(Key::Enter, Modifiers::NONE);
        for _ in 0..3 {
            rig.key(Key::ArrowRight, Modifiers::NONE);
        }
        rig.key(Key::Escape, Modifiers::NONE);
        (after_drag.0 - depth.0, rig.shell.engine.depth().0 - before.0)
    };
    assert_eq!(run(true), (1, 0), "playing: one step for the drag, none for the cancelled session");
    assert_eq!(run(false), (1, 0), "paused, the same");
}

#[test]
fn a_held_session_while_playing_is_one_undo() {
    // R2 with a held key: 14 steps over a second of play, one undo.
    let mut rig = two_masses();
    rig.shell.playing = true;
    let (start, depth) = (anchor(&rig, 0), rig.shell.engine.depth());
    rig.key(Key::Enter, Modifiers::NONE);
    hold(&mut rig, Key::ArrowRight, 59);
    rig.key(Key::Enter, Modifiers::NONE);
    assert!((anchor(&rig, 0)[0] - start[0] - 3.5).abs() < 1e-9);
    assert_eq!(rig.shell.engine.depth().0, depth.0 + 1);
    rig.shell.run("undo").unwrap();
    assert_eq!(anchor(&rig, 0), start);
}

#[test]
fn a_canvas_click_does_not_move_the_keyboard_focus() {
    // S18 K3 in the application: the keys on an inspector field; a click on the canvas that selects
    // a body hides the ring and leaves the focus where it was. Control: a click on a window's
    // control moves the focus onto it (the spec's §8).
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("select tower 1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    rig.shell.run("focus inspector").unwrap();
    rig.key(Key::Enter, Modifiers::NONE);
    let focus = rig.shell.keyboard.focus.clone();
    assert!(rig.shell.keyboard.nav_on && focus.len() > 1, "{focus:?}");
    let pane = rig.shell.layout().panes[0];
    let at = rig.shell.body_point(0, 0, pane.size()).expect("the mass is in view");
    rig.click(pane.min + at.to_vec2());
    assert_eq!(rig.shell.engine.selection(), Some(Target::Body(1)), "the click did its canvas action");
    assert!(!rig.shell.keyboard.nav_on, "and hid the ring");
    assert_eq!(rig.shell.keyboard.focus[0], focus[0], "the focus stayed in the inspector");
    // Control: a click on the wizard's radio moves the focus there.
    rig.key(Key::N, Modifiers::COMMAND);
    rig.frames(vec![vec![], vec![]]);
    rig.click_button("wizard surroundings 1");
    assert_eq!(rig.shell.keyboard.focus, ["wizard", "wizard/surroundings", "wizard/surroundings/1"]);
    assert!(!rig.shell.keyboard.nav_on, "in navigation mode, the ring hidden");
}

#[test]
fn typing_q_in_a_field_does_not_change_the_tool() {
    // S18 K9 in the application: Q typed into a text field is a letter, not the Select tool.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("tool move").unwrap();
    rig.key(Key::Backtick, Modifiers::NONE);
    rig.frames(vec![vec![], vec![]]);
    let q = |pressed| Event::Key {
        key: Key::Q,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    rig.frames(vec![vec![q(true), Event::Text("q".into())], vec![q(false)]]);
    assert_eq!(rig.shell.tool, "move", "Q was typed, not taken as the Select tool");
    assert!(rig.shell.console_line.contains('q'), "{:?}", rig.shell.console_line);
    // Control: out of the field, in the viewport, Q is the Select tool.
    rig.key(Key::Escape, Modifiers::NONE);
    rig.key(Key::Escape, Modifiers::NONE);
    rig.frames(vec![vec![], vec![]]);
    rig.shell.keyboard.focus = vec!["viewport".into()];
    rig.key(Key::Q, Modifiers::NONE);
    assert_eq!(rig.shell.tool, "select");
}

#[test]
fn every_starter_has_an_editor_for_every_field() {
    // S18 E6's second half: each starter, added and selected, has an editor for each of its
    // fields in the inspector, and each number among them has a unit from the schema.
    for starter in cav_sim::starters::starters() {
        let mut rig = Rig::new();
        rig.shell.run("load void").unwrap();
        rig.shell.add_starter(&starter.name).unwrap();
        rig.frames(vec![vec![], vec![]]);
        let prefix = Shell::resolve("{selection}", rig.shell.engine.selection());
        assert!(!prefix.is_empty(), "{}: selected", starter.name);
        let leaves =
            cav_sim::paths::leaves(rig.shell.engine.spec(), &prefix, cav_app::editors::MAX_FIELDS).unwrap();
        assert!(!leaves.is_empty());
        for leaf in &leaves {
            assert!(
                rig.shell.layout().editor(&leaf.path).is_some(),
                "{}: no editor for {}",
                starter.name,
                leaf.path
            );
            if matches!(leaf.kind, cav_sim::paths::Kind::Number | cav_sim::paths::Kind::Vector(_)) {
                assert!(leaf.unit.is_some(), "{}: {} has no unit in the schema", starter.name, leaf.path);
            }
        }
    }
    // Control: with nothing selected the body's fields have no editors.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert!(rig.shell.layout().editor("bodies[0].mass").is_none());
}

#[test]
fn fly_speed_takes_shift_and_alt_through_the_frame() {
    // S18 C5 through the window: the same frames of W held move the camera 10 times as far with
    // Shift and a tenth as far with Alt (the frame's own use of the modifiers).
    let flown = |modifiers: Modifiers| {
        let mut rig = Rig::new();
        rig.shell.run("load point").unwrap();
        rig.frames(vec![vec![]]);
        rig.shell.keyboard.focus = vec!["viewport".into()];
        rig.key(Key::F, Modifiers::SHIFT);
        assert!(rig.shell.flying());
        let start = rig.shell.engine.view(0).target;
        let w = |pressed, repeat| Event::Key { key: Key::W, physical_key: None, pressed, repeat, modifiers };
        let mut frames = vec![vec![w(true, false)]];
        // A held key repeats in the window's events, carrying the modifiers held with it.
        frames.extend((0..6).map(|_| vec![w(true, true)]));
        frames.push(vec![w(false, false)]);
        rig.frames(frames);
        let end = rig.shell.engine.view(0).target;
        (end[0] - start[0]).hypot(end[1] - start[1]).hypot(end[2] - start[2])
    };
    let (plain, fast, slow) = (flown(Modifiers::NONE), flown(Modifiers::SHIFT), flown(Modifiers::ALT));
    assert!(plain > 0.0);
    assert!((fast / plain - 10.0).abs() < 1e-9, "Shift: {}", fast / plain);
    assert!((slow / plain - 0.1).abs() < 1e-9, "Alt: {}", slow / plain);
}

#[test]
fn every_sub_scope_is_registered() {
    // The sub-scopes data may name (`actions::SUB_SCOPES`, S18's review, minor 6) are each drawn,
    // with a selection and without.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    for selection in ["select none", "select body 1"] {
        rig.shell.run(selection).unwrap();
        rig.frames(vec![vec![], vec![]]);
        for id in cav_app::actions::SUB_SCOPES {
            assert!(rig.shell.focus_tree().node(id).is_some(), "`{id}` is not drawn ({selection})");
        }
    }
}

#[test]
fn the_sheet_fits_the_window_and_scrolls_from_the_keyboard() {
    // S18's review, R6: the `?` sheet is inside the window (at 1440 × 900 and at a small 1024 × 640),
    // and its list is a stop that Enter, then Page Down and the arrows, scroll.
    for size in [[1440, 900], [1024, 640]] {
        let mut rig = Rig::new();
        rig.size = size;
        rig.shell.run("load point").unwrap();
        rig.frames(vec![vec![]]);
        rig.key(Key::Questionmark, Modifiers::NONE);
        assert!(rig.shell.keys_open);
        rig.frames(vec![vec![], vec![]]);
        let screen = rig.shell.layout().screen;
        let sheet = rig.shell.focus_tree().node("sheet").and_then(|n| n.rect).expect("the sheet is drawn");
        assert!(screen.contains_rect(sheet), "{size:?}: {sheet:?} in {screen:?}");
        assert_eq!(rig.shell.keyboard.focus, ["sheet", "sheet/list"], "the window lands on its list");
    }
    // At the small size the list is longer than the window: the keys scroll it.
    let mut rig = Rig::new();
    rig.size = [1024, 640];
    rig.shell.run("load point").unwrap();
    rig.key(Key::Questionmark, Modifiers::NONE);
    rig.frames(vec![vec![], vec![]]);
    // Control first: in navigation mode Page Down does not scroll (it is not the list's yet).
    rig.key(Key::PageDown, Modifiers::NONE);
    rig.frames(vec![vec![]]);
    assert_eq!(rig.shell.sheet_scroll, 0.0);
    rig.key(Key::Enter, Modifiers::NONE);
    assert_eq!(rig.shell.keyboard.mode, cav_app::keyboard::Mode::Interaction);
    rig.key(Key::PageDown, Modifiers::NONE);
    rig.frames(vec![vec![]]);
    let paged = rig.shell.sheet_scroll;
    assert!(paged > 100.0, "a page down scrolled {paged} points");
    rig.key(Key::ArrowUp, Modifiers::NONE);
    rig.frames(vec![vec![]]);
    assert!(
        (paged - rig.shell.sheet_scroll - 18.0).abs() < 0.5,
        "↑ is a row: {paged} → {}",
        rig.shell.sheet_scroll
    );
    rig.key(Key::Escape, Modifiers::NONE);
    rig.key(Key::Escape, Modifiers::NONE);
    assert!(!rig.shell.keys_open, "Esc, Esc: out of the list, then the window closes");
}

#[test]
fn the_inspector_reads_as_an_editor() {
    // S18's review, U1: with nothing selected the inspector is the scene's (its name, its
    // duration, a summary), the instrument and cadence folded under it; fields are named in words
    // from the schema; a reset is offered only where there is something to put back.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert!(rig.shell.layout().editor("duration").is_some());
    assert!(rig.shell.layout().editor("array.instrument.tower_height").is_none(), "the instrument is folded");
    assert!(rig.shell.layout().button("fold inspector/instrument").is_some());
    // Control: unfolded by a click on its header, every field of it has an editor.
    rig.click_button("fold inspector/instrument");
    rig.frames(vec![vec![]]);
    assert!(rig.shell.layout().editor("array.instrument.tower_height").is_some());
    // And from the keyboard: Enter on a folded section opens it.
    let cadence = rig.shell.focus_tree().node("inspector/cadence").unwrap().clone();
    assert_eq!(cadence.kind, cav_app::keyboard::Kind::Button, "folded, Enter opens it");
    rig.shell.run("focus inspector/cadence").unwrap();
    rig.key(Key::Enter, Modifiers::NONE);
    rig.frames(vec![vec![]]);
    assert!(rig.shell.layout().editor("array.schedule.loss").is_some());
    // Names in words, from the schema.
    let tree = rig.shell.focus_tree();
    assert_eq!(tree.node("field/array.instrument.tower_height").unwrap().name, "Tower height");
    assert_eq!(
        tree.node("field/array.instrument.ifo_separation").unwrap().name,
        "Interferometer separation Δr"
    );
    // The reset: disabled (and not drawn) on an unchanged field, there on a changed one.
    assert!(!tree.node("field/duration/reset").unwrap().enabled);
    rig.shell.run("set duration 21").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert!(rig.shell.focus_tree().node("field/duration/reset").unwrap().enabled);
    // A tower opens them as it did: the array's settings are a tower's to see.
    rig.shell.run("select tower 1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert!(rig.shell.layout().editor("array.instrument.launch_floor").is_some());
    // The Instrument layout opens them with nothing selected (a fresh window, so nothing is open).
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("layout instrument").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert!(rig.shell.layout().editor("array.instrument.tower_height").is_some());
}

#[test]
fn the_right_button_flies_and_holds_the_letters() {
    // S18's review, U3 (Unity's and Unreal's fly): hold the right button over the view, W moves
    // the camera forward and is not the Move tool; the mouse looks round from the eye; let go,
    // and W is the Move tool again (the control).
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("tool select").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let pane = rig.shell.layout().panes[0];
    let at = pane.center() + egui::vec2(0.0, -0.3 * pane.height());
    let start = rig.shell.engine.view(0).clone();
    let w = |pressed, repeat| Event::Key {
        key: Key::W,
        physical_key: None,
        pressed,
        repeat,
        modifiers: Modifiers::NONE,
    };
    let mut frames =
        vec![vec![Event::PointerMoved(at)], vec![Rig::press(at, PointerButton::Secondary, true)]];
    frames.push(vec![w(true, false)]);
    frames.extend((0..5).map(|_| vec![w(true, true)]));
    frames.push(vec![w(false, false)]);
    rig.frames(frames);
    assert!(rig.shell.mouse_fly.is_some(), "the right button is held: flying");
    assert_eq!(rig.shell.tool, "select", "W was the flight's, not the Move tool");
    let flown = rig.shell.engine.view(0).clone();
    let (to_eye, _, _) = start.basis();
    let moved: f64 = (0..3).map(|i| -(flown.target[i] - start.target[i]) * to_eye[i]).sum();
    assert!(moved > 0.05, "flew {moved} m forward");
    // The mouse looks: a drag with the button held turns the view, the eye kept.
    rig.frames(vec![
        vec![Event::PointerMoved(at + egui::vec2(30.0, 0.0))],
        vec![Event::PointerMoved(at + egui::vec2(60.0, 0.0))],
    ]);
    let looked = rig.shell.engine.view(0).clone();
    assert_ne!(looked.yaw, flown.yaw, "the mouse turned the view");
    let eye = |v: &cav_sim::ViewSpec| cav_app::camera::eye(v);
    assert!(eye(&looked).iter().zip(eye(&flown)).all(|(a, b)| (a - b).abs() < 1e-6), "from the eye");
    // Let go: no longer flying, and W is the Move tool again.
    rig.frames(vec![vec![Rig::press(at + egui::vec2(60.0, 0.0), PointerButton::Secondary, false)], vec![]]);
    assert!(rig.shell.mouse_fly.is_none());
    rig.shell.keyboard.focus = vec!["viewport".into()];
    rig.key(Key::W, Modifiers::NONE);
    assert_eq!(rig.shell.tool, "move");
    // None of it was an edit.
    assert_eq!(rig.shell.engine.depth(), (1, 0));
}

#[test]
fn the_tool_strip_has_one_name_a_tool_and_snap_apart() {
    // S18's review, U2: each tool has one name, its action's (Place is Place on the strip, in
    // the options row and on the sheet); snap is a toggle below the tools, not one of them.
    let mut rig = Rig::new();
    rig.shell.run("load void").unwrap();
    rig.shell.run("tool place").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let table = cav_app::actions::table();
    for tool in &cav_app::tools::strip().tools {
        let action = table.actions.iter().find(|a| a.line == format!("tool {}", tool.id)).unwrap();
        assert_eq!(tool.label, action.label);
        let node = rig.shell.focus_tree().node(&format!("toolbar/{}", tool.id)).unwrap();
        assert_eq!(node.name, tool.label);
    }
    let strip: Vec<&str> = rig.shell.focus_tree().children("toolbar");
    assert_eq!(strip.last(), Some(&"toolbar/snap"), "snap last, after the tools");
    let lowest_tool = cav_app::tools::strip()
        .tools
        .iter()
        .map(|t| rig.shell.layout().button(&format!("tool {}", t.id)).unwrap().bottom())
        .fold(0.0_f32, f32::max);
    let snap = rig.shell.layout().button("snap").unwrap();
    assert!(snap.top() >= lowest_tool + 8.0, "apart from the tools: {} after {lowest_tool}", snap.top());
    assert!(
        !cav_app::sheet::rows(table).iter().any(|r| r.label.starts_with("Tool: Snap")),
        "snap is no tool on the sheet"
    );
}

#[test]
fn maximised_there_is_a_way_back_in_sight() {
    // S18's review, U7: maximised, a Restore button stands beside Home; it restores the window.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.frames(vec![vec![]]);
    assert!(rig.shell.layout().button("maximise").is_none(), "the control: not maximised, no Restore");
    rig.key(Key::Space, Modifiers::COMMAND);
    assert!(rig.shell.maximised);
    rig.frames(vec![vec![]]);
    rig.click_button("maximise");
    assert!(!rig.shell.maximised, "the button restored it");
    assert!(rig.shell.layout().panel("outliner").is_some());
}

#[test]
fn the_session_s_arrows_go_the_way_they_point_on_screen() {
    // S18's review, minor 11: seen from the east, → moves the body right on screen (it moved it
    // along +x, which from the east is to the left).
    let screen_x = |rig: &Rig| {
        let rect = rig.shell.layout().panes[0];
        rig.shell.body_point(0, 0, rect.size()).unwrap().x
    };
    for view in ["view east", "view iso", "view north", "view top"] {
        let mut rig = two_masses();
        rig.shell.run(view).unwrap();
        rig.frames(vec![vec![], vec![]]);
        let before = screen_x(&rig);
        rig.key(Key::Enter, Modifiers::NONE);
        rig.key(Key::ArrowRight, Modifiers::NONE);
        rig.frames(vec![vec![]]);
        assert!(
            screen_x(&rig) > before + 1.0,
            "{view}: → moved it right on screen ({before} → {})",
            screen_x(&rig)
        );
        rig.key(Key::Escape, Modifiers::NONE);
    }
    // Control: from the north, +x is to the left on screen, so the old arrows (→ always +x) went
    // the wrong way. (From the east +x comes toward the eye: they did not move it across at all.)
    let mut rig = two_masses();
    rig.shell.run("view north").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let before = screen_x(&rig);
    rig.shell.run("set bodies[0].motion.Static.position [3.0, 0.0, 2.5]").unwrap();
    rig.frames(vec![vec![]]);
    assert!(screen_x(&rig) < before - 1.0);
}
