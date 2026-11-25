//! S19's editor tests (`docs/slices/S19.md` §4): the third review's minors of S18, and the new
//! tools, each driven through the same input events a window delivers, each with a control arm.

use super::{Rig, anchor, two_masses};
use cav_engine::Target;
use egui::{Event, Key, Modifiers, PointerButton};

#[test]
fn a_click_on_a_row_ends_a_transform_session_keeping_its_move() {
    // S18's third review, minor 1: Enter, →, then a click on the other body's row in the tree. The
    // session ends with its move kept, so the next → is no session's and moves neither body.
    // Control: with no session the same click and arrow move nothing.
    let run = |session: bool| {
        let mut rig = two_masses();
        let (first, second, depth) = (anchor(&rig, 0), anchor(&rig, 1), rig.shell.engine.depth().0);
        if session {
            rig.key(Key::Enter, Modifiers::NONE);
            rig.key(Key::ArrowRight, Modifiers::NONE);
            assert!(rig.shell.session.is_some());
        }
        let row = rig.row(Target::Body(2));
        rig.click(row.center());
        assert!(rig.shell.session.is_none(), "the click ended any session");
        assert_eq!(rig.shell.engine.selection(), Some(Target::Body(2)), "and selected the row");
        rig.key(Key::ArrowRight, Modifiers::NONE);
        (first, anchor(&rig, 0), second, anchor(&rig, 1), rig.shell.engine.depth().0 - depth)
    };
    let (start, moved, other, other_now, steps) = run(true);
    let step = (0..3).map(|i| (moved[i] - start[i]).abs()).fold(0.0, f64::max);
    assert_eq!(step, 0.25, "the first body kept its one step: {start:?} -> {moved:?}");
    assert_eq!(other_now, other, "the later arrow moved the other body nowhere");
    assert_eq!(steps, 1, "one step of undo");
    let (start, moved, other, other_now, steps) = run(false);
    assert_eq!((moved, other_now, steps), (start, other, 0), "control: nothing moves");
}

#[test]
fn the_mouse_fly_outlives_the_pointer_leaving_while_held() {
    // S18's third review, minor 3: a wide look turn reaches the window's edge and the pointer
    // leaves it while the right button is held; the fly goes on. The release ends it (the control:
    // the same release with the pointer in the window ends it too).
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let at = rig.shell.layout().panes[0].center();
    rig.frames(vec![vec![Event::PointerMoved(at)], vec![Rig::press(at, PointerButton::Secondary, true)]]);
    assert!(rig.shell.mouse_fly.is_some(), "flying");
    rig.frames(vec![vec![Event::PointerGone], vec![], vec![]]);
    assert!(rig.shell.mouse_fly.is_some(), "the pointer left with the button held: still flying");
    rig.frames(vec![
        vec![Event::PointerMoved(at)],
        vec![Rig::press(at, PointerButton::Secondary, false)],
        vec![],
    ]);
    assert!(rig.shell.mouse_fly.is_none(), "the release ends it");
}

#[test]
fn loss_and_seed_say_what_they_are() {
    // S18's third review, minor 9: "Loss 0" and "Seed 0" had no unit or hint in the Instrument
    // layout. Their names now say what they are.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("layout instrument").unwrap();
    rig.frames(vec![vec![], vec![], vec![]]);
    let name =
        |rig: &Rig, path: &str| rig.shell.focus_tree().node(&format!("field/{path}")).map(|n| n.name.clone());
    assert_eq!(name(&rig, "array.schedule.loss").as_deref(), Some("Shots lost, fraction"));
    assert_eq!(name(&rig, "array.schedule.seed").as_deref(), Some("Random seed"));
}

/// `point` with a 0.4 m steel box beside it as body 2, selected.
fn steel_box() -> Rig {
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run(r#"add bodies {"label":"box","mass":1.0,"shape":{"kind":{"Cuboid":{"size":[0.4,0.4,0.4]}},"pitch":0.05,"supersample":2,"material":"steel"},"motion":{"Static":{"position":[2.0,2.0,1.0]}}}"#).unwrap();
    rig.shell.run("select body 2").unwrap();
    rig.frames(vec![vec![], vec![]]);
    rig
}

fn tree_mass(rig: &Rig, body: usize) -> f64 {
    rig.shell.engine.body_tree(body).expect("drawn as voxels").body().mass
}

#[test]
fn a_material_is_chosen_in_the_inspector_and_its_density_sets_the_mass() {
    // S19 §1: a voxel body's material is a drop-down of the materials there are; choosing lead
    // makes it weigh lead's density times its volume, which the mass row then says, and one undo
    // takes it back. Control: "none" keeps it uniform, weighing its `mass`.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.add_starter("cube").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let depth = rig.shell.engine.depth().0;
    let uniform = tree_mass(&rig, 1);
    assert_eq!(uniform, rig.shell.engine.spec().bodies[1].mass, "uniform: its mass");
    rig.shell.run("focus field/bodies[1].shape.material").unwrap();
    rig.frames(vec![vec![]]);
    rig.key(Key::Enter, Modifiers::NONE);
    // The choices: none, then the library's in its order (tungsten, lead, …).
    rig.key(Key::ArrowDown, Modifiers::NONE);
    rig.key(Key::ArrowDown, Modifiers::NONE);
    rig.key(Key::Enter, Modifiers::NONE);
    rig.frames(vec![vec![], vec![]]);
    let shape = rig.shell.engine.spec().bodies[1].shape.clone().unwrap();
    assert_eq!(shape.material.as_deref(), Some("lead"));
    let tree = rig.shell.engine.body_tree(1).unwrap().clone();
    let volume = tree.samples as f64 * (shape.pitch / f64::from(shape.supersample)).powi(3);
    assert!((tree.body().mass - 11340.0 * volume).abs() < 1e-12 * tree.body().mass);
    assert_eq!(rig.shell.engine.depth().0, depth + 1, "one step");
    let mass_row = rig.shell.focus_tree().node("field/bodies[1].mass").expect("the mass row is drawn");
    assert_eq!(mass_row.kind, cav_app::keyboard::Kind::ReadOnly, "shown, not edited");
    rig.shell.run("undo").unwrap();
    rig.frames(vec![vec![]]);
    assert_eq!(rig.shell.engine.spec().bodies[1].shape.as_ref().unwrap().material, None);
    assert_eq!(tree_mass(&rig, 1).to_bits(), uniform.to_bits(), "control: uniform again, its mass");
}

#[test]
fn mass_is_kept_to_the_bit_through_material_edits() {
    // S19 M5 (REBUILD §9's exit for S19): the tree is made again from the scene on every edit, so
    // no edit can drift a mass. Paint, add and erase, each undone, give the start's bits back;
    // painting a material over itself leaves them. Control: an add changes the mass.
    let mut rig = steel_box();
    let start = tree_mass(&rig, 1);
    let parts = "bodies[1].shape.parts";
    let edits = [
        r#"{"mode":"Paint","material":"lead","at":[0,0,0],"kind":{"Stroke":{"radius":0.12,"points":[[-0.2,0,0],[0.2,0.1,0]]}}}"#,
        r#"{"mode":"Add","material":"copper","at":[0.3,0,0],"kind":{"Solid":{"shape":{"Sphere":{"radius":0.15}}}}}"#,
        r#"{"mode":"Erase","at":[0,0,0.2],"kind":{"Solid":{"shape":{"Sphere":{"radius":0.1}}}}}"#,
    ];
    for part in edits {
        rig.shell.run(&format!("add {parts} {part}")).unwrap();
        let edited = tree_mass(&rig, 1);
        assert_ne!(edited.to_bits(), start.to_bits(), "{part} changed the mass");
        rig.shell.run("undo").unwrap();
        assert_eq!(tree_mass(&rig, 1).to_bits(), start.to_bits(), "{part}, undone: the same bits");
        rig.shell.run("redo").unwrap();
        assert_eq!(tree_mass(&rig, 1).to_bits(), edited.to_bits(), "{part}, redone: the same bits again");
        rig.shell.run("undo").unwrap();
    }
    rig.shell
        .run(&format!(
            r#"add {parts} {{"mode":"Paint","material":"steel","at":[0,0,0],"kind":{{"Stroke":{{"radius":0.3,"points":[[0,0,0]]}}}}}}"#
        ))
        .unwrap();
    assert_eq!(tree_mass(&rig, 1).to_bits(), start.to_bits(), "steel painted over steel");
}

fn stl_box(half: [f64; 3]) -> Vec<u8> {
    let h = cav_core::Vec3::from_array(half);
    let (v, t) = cav_core::voxel::mesh::box_mesh(-h, h);
    cav_core::voxel::mesh::to_stl(&v, &t)
}

#[test]
fn a_dropped_file_adds_a_body() {
    // S19 I1 through the window: a file dropped on it (its bytes, as a page hands them over)
    // opens the Import window on it; Import adds the body, fitted to 1 m, where the camera looks,
    // selected; its corners are one row, not one each; one undo takes it out. Control: a dropped
    // file that is not a mesh adds nothing, and the console says why.
    let mut rig = Rig::new();
    rig.shell.run("load void").unwrap();
    rig.frames(vec![vec![], vec![]]);
    rig.shell.dropped_file("crate.stl", Some(stl_box([0.5, 0.25, 0.125])));
    rig.frames(vec![vec![], vec![]]);
    assert_eq!(rig.shell.import_window.as_ref().map(|f| f.file.as_str()), Some("crate.stl"));
    rig.click_button("import go");
    rig.frames(vec![vec![], vec![]]);
    let spec = rig.shell.engine.spec().clone();
    assert_eq!(spec.bodies.len(), 1);
    assert_eq!(spec.bodies[0].label, "crate");
    assert_eq!(rig.shell.engine.selection(), Some(Target::Body(1)));
    assert!(rig.shell.import_window.is_none(), "the window closed");
    let shape = spec.bodies[0].shape.as_ref().unwrap();
    assert!((shape.kind.bounds().unwrap().size().x - 1.0).abs() < 1e-12, "fitted to 1 m");
    let corners =
        rig.shell.focus_tree().node("field/bodies[0].shape.kind.Triangles.vertices").expect("one row");
    assert_eq!(corners.kind, cav_app::keyboard::Kind::ReadOnly);
    assert!(rig.shell.focus_tree().node("field/bodies[0].shape.kind.Triangles.vertices[0]").is_none());
    rig.shell.run("undo").unwrap();
    assert!(rig.shell.engine.spec().bodies.is_empty(), "one undo");
    rig.shell.dropped_file("notes.txt", Some(b"hello".to_vec()));
    rig.frames(vec![vec![], vec![]]);
    rig.click_button("import go");
    rig.frames(vec![vec![]]);
    assert!(rig.shell.engine.spec().bodies.is_empty(), "control: nothing added");
    assert!(rig.shell.console_open, "and the console says why");
}

#[test]
fn the_probe_tool_reads_where_it_is_clicked() {
    // S19 T1 through the window: with Probe in hand a click on a voxel body reads its cell (the
    // card is drawn, the reading is `probe x y z`'s), and changes no scene. Control: the same
    // click with Select in hand reads nothing.
    let click_body = |tool: &str| {
        let mut rig = Rig::new();
        rig.shell.run("load point").unwrap();
        rig.shell.run("add sphere at 2 2 1").unwrap();
        rig.shell.run(&format!("tool {tool}")).unwrap();
        rig.frames(vec![vec![], vec![]]);
        let pane = rig.shell.layout().panes[0];
        let at = rig.shell.body_point(0, 1, pane.size()).expect("the sphere is in view") + pane.min.to_vec2();
        let (spec, depth) = (rig.shell.engine.spec().clone(), rig.shell.engine.depth());
        rig.click(at);
        rig.frames(vec![vec![]]);
        assert_eq!((rig.shell.engine.spec(), rig.shell.engine.depth()), (&spec, depth), "{tool}: no edit");
        rig
    };
    let rig = click_body("probe");
    let probed = rig.shell.probed.clone().expect("a reading");
    let cell = probed.cell.clone().expect("the click is on the sphere's cells");
    assert_eq!(cell.body, 1);
    assert!(probed.towers[0].phase.is_some_and(|p| p != 0.0), "its share of tower 1");
    assert_eq!(rig.shell.engine.probe(probed.point), probed, "the reading is the console's");
    assert!(rig.shell.layout().button("probe card").is_some());
    assert!(click_body("select").shell.probed.is_none(), "control: Select reads nothing");
}

#[test]
fn the_tool_strip_floats_over_the_view_s_edge() {
    // S19's panels (docs/design/panels.md): the tool strip is a bar over the view's left edge,
    // not a column beside the tree, so the figure is as wide as the panels leave it. Control: the
    // tree's panel ends where the figure begins (no column between).
    let rig = Rig::new();
    let layout = rig.shell.layout();
    let strip = layout.button("tool strip").expect("drawn");
    assert!(layout.figure.contains_rect(strip), "{strip:?} over the figure {:?}", layout.figure);
    assert!(strip.left() - layout.figure.left() < 16.0, "at its left edge");
    assert!(strip.width() < 64.0);
    let tree = layout.panel("outliner").unwrap();
    assert!((layout.figure.left() - tree.right()).abs() < 2.0, "{tree:?} against {:?}", layout.figure);
}
