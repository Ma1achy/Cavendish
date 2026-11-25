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
    rig.shell.run("load room").unwrap();
    rig.frames(vec![vec![]]);
    let start = rig.shell.engine.view(0).clone();
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
    assert_eq!(rig.shell.engine.history(), (true, false));
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
