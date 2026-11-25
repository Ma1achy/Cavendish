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
    let (start, history) = (anchor(&rig, 0), rig.shell.engine.history());
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
    assert_eq!(rig.shell.engine.history(), history, "and nothing new can be undone");
    // Control: Enter keeps it, and one undo takes the whole held run back.
    session(&mut rig);
    rig.key(Key::Enter, Modifiers::NONE);
    assert!((anchor(&rig, 0)[0] - start[0] - 3.5).abs() < 1e-9);
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
    let undo_depth = rig.shell.engine.history();
    // North (y): east would take it onto tower 2's axis at (4, 0), where the read is the
    // through-the-tower case (S5's bounded walk; docs/slices/S18.md §7).
    rig.drag_arrow(0, 1, 2.0);
    let moved = anchor(&rig, 0);
    assert!((moved[1] - start[1] - 2.0).abs() <= 1e-12, "moved by {}", moved[1] - start[1]);
    assert_eq!((moved[0], moved[2]), (start[0], start[2]));
    assert_eq!(undo_depth, (true, false));
    // The reading the window shows is the forward model's of the edited scene, bit for bit.
    assert!(rig.shell.engine.settle_reading(std::time::Duration::from_secs(120)));
    let shown = rig.shell.reading().clone();
    let direct = cav_sim::read::reading(rig.shell.engine.spec(), shown.time, &|| false).expect("read");
    assert_eq!(shown.towers[0].gradiometer.to_bits(), direct.towers[0].gradiometer.to_bits(), "tower 1's ΔΦ");
    assert!(shown.towers[0].gradiometer != 0.0);
    // The event budget: recorded at the first pass (docs/slices/S18.md §7) and held.
    assert!(rig.events <= TASK_EVENTS, "{} input events", rig.events);
}

/// The input events the T1 task takes: counted at its first pass and held, so the task does not
/// get longer unnoticed.
const TASK_EVENTS: usize = 20;

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
    rig.key(Key::S, Modifiers::COMMAND);
    assert!(rig.shell.save_window.is_some(), "no file yet: Ctrl+S asks for one");
    rig.shell.save_window = Some(file.path());
    rig.frames(vec![vec![]]);
    rig.click_button("save go");
    assert!(rig.shell.save_window.is_none() && !rig.shell.engine.unsaved());
    assert_eq!(rig.shell.file.as_deref(), Some(file.path().as_str()));
    rig.shell.run("load room").unwrap();
    rig.key(Key::O, Modifiers::COMMAND);
    rig.shell.open_window = Some(file.path());
    rig.frames(vec![vec![]]);
    rig.click_button("open go");
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
            let mut seen = Vec::new();
            rig.shell.keyboard.focus = vec!["footer".into()];
            for _ in 0..7 {
                rig.key(Key::Tab, Modifiers::NONE);
                seen.push(rig.shell.keyboard.focused().to_owned());
            }
            assert_eq!(seen, BIG, "{state}: Tab's order");
        }
    }
    // The layouts arranged the figure as their lines say.
    rig.shell.run("layout analysis").unwrap();
    assert_eq!(rig.shell.engine.panes(), 4);
    assert_eq!(rig.shell.engine.content(0), cav_engine::PaneContent::Signal);
    // Control: a layout's lines are view commands, never edits.
    assert_eq!(rig.shell.engine.history(), (true, false));
}

/// What share of the pane's shorter side a sphere (`centre`, `radius` metres) spans on screen,
/// measured by projecting its edge, in pane `pane` of the last frame.
fn spans(rig: &Rig, pane: usize, centre: [f64; 3], radius: f64) -> f64 {
    let rect = rig.shell.layout().panes[pane];
    let view = rig.shell.engine.view(pane).clone();
    let camera = cav_render::Camera::new(&view, f64::from(rect.width() / rect.height()));
    let whole = [rect.width().round() as u32, rect.height().round() as u32];
    let at = |p: cav_core::Vec3| camera.pixel(p, whole).expect("in front of the camera");
    let c = cav_core::Vec3::from_array(centre);
    let [x0, y0] = at(c);
    let [x1, y1] = at(c + camera.right * radius);
    2.0 * (x1 - x0).hypot(y1 - y0) / f64::from(rect.width().min(rect.height()))
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
    assert!(matches!(
        rig.shell.engine.journal().last(),
        Some(Command::View(cav_engine::ViewCommand::Camera { .. }))
    ));
    assert_eq!(rig.shell.engine.history(), (true, false), "framing is no edit: only the load can be undone");
    // A tower frames too, from the tree.
    rig.shell.run("select tower 3").unwrap();
    rig.shell.keyboard.focus = vec!["tree".into()];
    rig.key(Key::F, Modifiers::NONE);
    let (centre, radius) = cav_app::camera::bounds_of(rig.shell.engine.spec(), Some(Target::Tower(3)), 0.0);
    let tower = spans(&rig, 0, centre, radius);
    assert!((0.4..=0.8).contains(&tower), "{tower}");
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
    let history = rig.shell.engine.history();
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
    // None of it was an edit.
    assert_eq!(rig.shell.engine.history(), history);
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
