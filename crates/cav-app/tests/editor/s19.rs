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
