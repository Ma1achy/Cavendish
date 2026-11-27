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
    let at = rig.shell.layout().canvases[0].center();
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
    // opens the Import window on it; Import adds the body, at its own size, where the camera looks,
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
    assert!((shape.kind.bounds().unwrap().size().x - 1.0).abs() < 1e-12, "its own size, 1 m");
    // It stands on the ground, lifted by half its own height (S19's review, minor 8); the
    // cube starter's lift, which it had, is another height.
    let z = spec.bodies[0].motion.anchor().0[2];
    assert!((z - (rig.shell.ground() + 0.125)).abs() < 1e-12, "{z} m");
    assert!((rig.shell.place_for("cube")[2] - z).abs() > 0.1, "the control: the cube's lift differs");
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
    // reading is drawn, and is `probe x y z`'s), and changes no scene. Control: the same click
    // with Select in hand reads nothing.
    // (S20 changed where it is drawn: the right panel's Probe tab, not a card over the figure,
    // which S19's second review called out as minor 13. So this looks for the tab's reading where
    // it looked for the button "probe card"; `s20::the_probe_reads_into_the_right_panel` holds
    // the rest.)
    let click_body = |tool: &str| {
        let mut rig = Rig::new();
        rig.shell.run("load point").unwrap();
        rig.shell.run("add sphere at 2 2 1").unwrap();
        rig.shell.run(&format!("tool {tool}")).unwrap();
        rig.frames(vec![vec![], vec![]]);
        let pane = rig.shell.layout().canvases[0];
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
    assert_eq!(rig.shell.layout().probe.as_ref(), Some(&probed), "drawn in the Probe tab");
    assert!(rig.shell.layout().button("probe card").is_none());
    assert!(click_body("select").shell.probed.is_none(), "control: Select reads nothing");
}

#[test]
fn the_tool_strip_floats_over_the_view_s_edge() {
    // S19's panels (docs/design/panels.md): the tools are a bar over the view, not a column
    // beside the tree, so the figure is as wide as the panels leave it. Control: the tree's panel
    // ends where the figure begins (no column between).
    // (S20, `docs/design/workspace.md` §3: the expectation changed. The strip was an area fixed
    // at the figure's top left corner, 30 pt buttons; it is now the palette inside the active
    // scene viewport's canvas, under that viewport's header, `theme::ICON` buttons, at the
    // canvas's top left until it is dragged elsewhere. P1 in `s20.rs` holds the rest.)
    let rig = Rig::new();
    let layout = rig.shell.layout();
    let strip = layout.button("tool palette").expect("drawn");
    let canvas = layout.canvases[0];
    assert!(layout.figure.contains_rect(strip), "{strip:?} over the figure {:?}", layout.figure);
    assert!(canvas.contains_rect(strip), "{strip:?} inside the canvas {canvas:?}, under the header");
    assert!(strip.left() - canvas.left() < 16.0 && strip.top() - canvas.top() < 16.0, "at its corner");
    assert!(strip.width() < 40.0, "one column of icon buttons: {}", strip.width());
    let tree = layout.panel("outliner").unwrap();
    assert!((layout.figure.left() - tree.right()).abs() < 2.0, "{tree:?} against {:?}", layout.figure);
}

/// Every editable row's value rectangle's left edge in the inspector, by path.
fn value_lefts(rig: &Rig) -> Vec<(String, f32)> {
    let panel = rig.shell.layout().panel("properties").unwrap();
    rig.shell
        .layout()
        .editors
        .iter()
        // (S20: but for the scene's description, which is a box as wide as its card under its
        // name, not a value in the column: `docs/design/workspace.md` §0.)
        .filter(|(path, rect)| {
            panel.contains_rect(*rect) && !path.ends_with(".label") && path != "name" && path != "description"
        })
        .map(|(path, rect)| (path.clone(), rect.left()))
        .collect()
}

#[test]
fn every_value_in_the_inspector_starts_at_one_x() {
    // docs/design/panels.md: one grid in every card, the value column at the same x on every
    // row of the panel, a body's and the scene's alike. Control: the header's name field is not
    // in a card, and the same measure sees it elsewhere.
    for selection in ["select body 1", "select none", "select tower 1"] {
        let mut rig = Rig::new();
        rig.shell.run("load point").unwrap();
        rig.shell.run(selection).unwrap();
        if selection == "select none" {
            // The instrument's and the cadence's cards open, so their rows are measured too.
            rig.shell.run("layout instrument").unwrap();
        }
        rig.frames(vec![vec![], vec![], vec![]]);
        let lefts = value_lefts(&rig);
        assert!(lefts.len() >= 3, "{selection}: {lefts:?}");
        let x = lefts[0].1;
        for (path, left) in &lefts {
            assert!((left - x).abs() < 0.5, "{selection}: {path} at {left}, not {x}");
        }
        let name = rig.shell.layout().editors.iter().find(|(p, _)| p.ends_with("label") || p == "name");
        if let Some((_, rect)) = name {
            assert!((rect.left() - x).abs() > 10.0, "control: the header's field is at {}", rect.left());
        }
    }
}

#[test]
fn the_header_menu_holds_delete_and_duplicate() {
    // docs/design/panels.md: no loose "remove this body" among the fields; the header's ⋮ menu
    // has Duplicate and Delete, each one edit. Control: closed, the menu shows neither.
    let mut rig = two_masses();
    rig.shell.run("select body 2").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert!(rig.shell.layout().button("remove bodies[1]").is_none(), "control: closed");
    rig.click_button("inspector menu");
    assert!(rig.shell.layout().button("remove bodies[1]").is_some());
    rig.click_button("duplicate bodies[1]");
    assert_eq!(rig.shell.engine.spec().bodies.len(), 3, "Duplicate made a copy");
    rig.shell.run("undo").unwrap();
    rig.frames(vec![vec![], vec![]]);
    rig.click_button("inspector menu");
    rig.click_button("remove bodies[1]");
    assert_eq!(rig.shell.engine.spec().bodies.len(), 1, "Delete took it out");
}

#[test]
fn optional_properties_are_behind_checkboxes() {
    // docs/design/panels.md: "Appears none add" became a box. Ticking it puts the time there,
    // one edit; unticking takes it away. Control: a body's mass, which is not optional, has none.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("select body 1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    // (S20: the Lifetime card is folded at first on a body that is always there, so it is opened
    // here by a click on its header before its boxes are looked for.)
    rig.click_button("fold inspector/lifetime");
    rig.frames(vec![vec![]]);
    assert!(rig.shell.layout().button("gate bodies[0].mass").is_none(), "control");
    rig.click_button("gate bodies[0].appears");
    rig.frames(vec![vec![]]);
    assert!(rig.shell.engine.spec().bodies[0].appears.is_some(), "ticked: there");
    rig.click_button("gate bodies[0].appears");
    rig.frames(vec![vec![]]);
    assert!(rig.shell.engine.spec().bodies[0].appears.is_none(), "unticked: gone");
}

#[test]
fn every_text_reads_on_its_ground() {
    // docs/design/panels.md and S19's review, I6: every (text, ground) pair the panels draw is at
    // least 4.5:1, the tool strip's key letters and a field's hint among them. Control: the weak
    // grey on a button's ground, as the key letters were drawn, fails (3.3:1).
    use cav_app::theme::{WEAK, WIDGET, contrast, text_pairs};
    let pairs = text_pairs();
    assert!(pairs.len() >= 20, "{} pairs", pairs.len());
    for (text, colour, ground, behind) in &pairs {
        let ratio = contrast(*colour, *behind);
        assert!(ratio >= 4.5, "{text} on {ground}: {ratio:.2}:1");
    }
    assert!(contrast(WEAK, WIDGET) < 4.5, "control: {:.2}", contrast(WEAK, WIDGET));
}

/// Every element of the keyboard's tree under `scope` that was drawn, with its rect.
fn drawn_targets(rig: &Rig, scope: &str) -> Vec<(String, egui::Rect)> {
    let tree = rig.shell.focus_tree();
    let mut out = Vec::new();
    let mut stack = vec![scope.to_owned()];
    while let Some(id) = stack.pop() {
        let Some(node) = tree.node(&id) else { continue };
        let children = tree.children(&id);
        if children.is_empty()
            && node.kind != cav_app::keyboard::Kind::Scope
            && node.kind != cav_app::keyboard::Kind::ReadOnly
            && let Some(rect) = node.rect.filter(|r| r.is_positive())
        {
            out.push((id.clone(), rect));
        }
        stack.extend(children.into_iter().map(str::to_owned));
    }
    out
}

/// The selections whose side panels' targets are measured: a point mass, the scene, a tower and
/// a body of materials with a part.
pub(super) const SELECTIONS: [&str; 4] = ["select body 1", "select none", "select tower 1", "select body 2"];

/// `point` with a steel cube that has a lead part, `selection` made, and every target the
/// keyboard's tree has under the scene tree and the inspector, as drawn.
pub(super) fn side_panel_targets(selection: &str) -> (Rig, Vec<(String, egui::Rect)>) {
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.add_starter("cube").unwrap();
    rig.shell.run("set bodies[1].shape.material steel").unwrap();
    rig.shell
        .run(r#"add bodies[1].shape.parts {"mode":"Add","material":"lead","at":[0.45,0,0],"kind":{"Solid":{"shape":{"Sphere":{"radius":0.3}}}}}"#)
        .unwrap();
    rig.shell.run(selection).unwrap();
    rig.frames(vec![vec![], vec![], vec![]]);
    let mut targets = drawn_targets(&rig, "tree");
    targets.extend(drawn_targets(&rig, "inspector"));
    (rig, targets)
}

#[test]
fn every_target_in_the_side_panels_is_big_enough() {
    // docs/design/panels.md and S19's review, I6: every control in the tree and the inspector
    // (rows, boxes, ticks, drop-downs, eye, lock, +, ⋮) is at least `theme::HIT` a side, for a
    // point mass, the scene, a tower and a body of materials with a part (its Parts card's ⋮,
    // "+ Add part" and the material drop-downs: the second review, minor 7), the body's eye and
    // lock among them.
    //
    // Rewritten for S20 (the owner's ruling of 25 Nov 2026 withdrew 24 pt on desktop:
    // `docs/design/workspace.md` §0). `theme::HIT` is 20, read from the theme as before. The
    // control was "the menu bar's targets are smaller", which is not so at 20; it is now a tick
    // box of the same editors drawn with controls of half the theme's side, which the same
    // measure finds too small: so the size is the theme's doing and the measure can fail. And
    // the window is 900 high again, not the 1600 that rows of 24 needed.
    let least = cav_app::theme::HIT - 0.5;
    for selection in SELECTIONS {
        let (_, targets) = side_panel_targets(selection);
        assert!(targets.len() >= 12, "{selection}: {} targets", targets.len());
        assert!(targets.iter().any(|(id, _)| id.ends_with("/eye")), "the eye is measured");
        if selection == "select body 2" {
            for part in ["field/bodies[1].shape.material", "field/bodies[1].shape.parts[0].material", "parts"]
            {
                assert!(targets.iter().any(|(id, _)| id.contains(part)), "`{part}` is measured: {targets:?}");
            }
        }
        for (id, rect) in &targets {
            assert!(
                rect.width() >= least && rect.height() >= least,
                "{selection}: `{id}` is {:.1} × {:.1}",
                rect.width(),
                rect.height()
            );
        }
    }
    // Control: a body's Lifetime boxes, drawn with no window at the theme's side and at half of
    // it. The first passes the measure; the second fails it.
    let tick = |side: f32| {
        let mut shell = cav_app::Shell::new();
        shell.run("load point").unwrap();
        super::s20::bare_frame(&cav_app::headless::context(), egui::vec2(320.0, 900.0), |ui| {
            ui.spacing_mut().interact_size = egui::vec2(side, side);
            cav_app::editors::show(&mut shell, ui, "inspector", "bodies[0]");
        });
        let found = shell.layout().buttons.iter().rev().find(|(name, _)| name == "gate bodies[0].appears");
        found.map(|(_, rect)| *rect).expect("the Appears box is drawn")
    };
    let (full, half) = (tick(cav_app::theme::HIT), tick(0.5 * cav_app::theme::HIT));
    assert!(full.width() >= least && full.height() >= least, "at the theme's side: {full:?}");
    assert!(half.width() < least, "control: at half the side the box is {half:?}");
}

/// The ids the keyboard walks inside the inspector, in order, depth first.
fn inspector_stops(rig: &Rig) -> Vec<String> {
    let tree = rig.shell.focus_tree();
    let mut out = Vec::new();
    let mut stack = vec!["inspector".to_owned()];
    while let Some(id) = stack.pop() {
        let children = tree.children(&id);
        if id != "inspector" {
            out.push(id.clone());
        }
        stack.extend(children.into_iter().rev().map(str::to_owned));
    }
    out
}

#[test]
fn folding_a_card_keeps_it_a_stop_in_its_place() {
    // keyboard.md §9 and S19's review, I6: a folded card is still a stop, where it was, and the
    // stops inside the inspector keep their order: folded, the walk is the open walk without the
    // card's own rows. The control is in the equality itself: a folded card that stopped
    // registering would be missing from `folded` and present in `expected` (S19's second review,
    // minor 8: the walk compared with itself less the card was true by arithmetic, and is gone).
    let walk = |fold: bool| {
        let mut rig = Rig::new();
        rig.shell.run("load point").unwrap();
        rig.shell.run("select body 1").unwrap();
        rig.frames(vec![vec![], vec![]]);
        if fold {
            let header = rig.shell.layout().button("fold inspector/motion").expect("the Motion card");
            rig.click(header.center());
            rig.frames(vec![vec![]]);
        }
        inspector_stops(&rig)
    };
    let (open, folded) = (walk(false), walk(true));
    assert!(open.iter().any(|id| id == "inspector/motion") && open.len() > folded.len());
    let inside = |id: &String| {
        id.starts_with("field/bodies[0].motion") && !id.starts_with("field/bodies[0].motion.Static.position")
            || (id.starts_with("inspector/motion/") && *id != "inspector/motion/menu")
    };
    let expected: Vec<String> = open.iter().filter(|id| !inside(id)).cloned().collect();
    assert_eq!(folded, expected, "the folded card in its place, the rest in order");
}

#[test]
fn a_body_s_cards_come_in_the_design_s_order_each_with_a_menu() {
    // docs/design/panels.md and S19's review, I5: Transform first (Position), then Shape (its
    // kind), Material, Motion (its kind), Lifetime, Info; each card with a ⋮. A body of materials
    // has Parts after Material. Control: a point mass has no Parts card, and its Mass card is
    // titled for what it holds.
    let order = |rig: &Rig| -> Vec<String> {
        rig.shell
            .focus_tree()
            .children("inspector")
            .into_iter()
            .filter(|id| !id.starts_with("inspector/tab") && *id != "inspector/header")
            .map(str::to_owned)
            .collect()
    };
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("select body 1").unwrap();
    rig.frames(vec![vec![], vec![], vec![]]);
    let cards = order(&rig);
    let expected =
        ["transform", "shape", "material", "motion", "lifetime", "info"].map(|c| format!("inspector/{c}"));
    assert_eq!(cards, expected);
    for card in &expected {
        assert!(rig.shell.layout().button(&format!("menu {card}")).is_some(), "{card} has a ⋮");
    }
    assert!(rig.shell.focus_tree().node("field/bodies[0].motion.Static.position[0]").is_some(), "Position");
    assert_eq!(rig.shell.focus_tree().node("inspector/material").map(|n| n.name.as_str()), Some("Mass"));
    let mut rig = Rig::new();
    rig.shell.run("load void").unwrap();
    rig.shell.add_starter("cube").unwrap();
    rig.shell.run("set bodies[0].shape.material steel").unwrap();
    rig.frames(vec![vec![], vec![], vec![]]);
    let cards = order(&rig);
    let parts = cards.iter().position(|c| c == "inspector/parts").expect("a Parts card");
    assert_eq!(cards[parts - 1], "inspector/material");
    assert_eq!(cards[parts + 1], "inspector/motion");
    assert_eq!(rig.shell.focus_tree().node("inspector/material").map(|n| n.name.as_str()), Some("Material"));
    // No box anywhere that takes the shape away (the Kind row does that, as a choice); the
    // lifetime's boxes are there (the control: the measure sees a box).
    assert!(rig.shell.layout().button("gate bodies[0].shape").is_none(), "no hidden point-mass switch");
    // (S20: the Lifetime card is folded at first, so opened here for its boxes to be seen.)
    rig.click_button("fold inspector/lifetime");
    rig.frames(vec![vec![]]);
    assert!(rig.shell.layout().button("gate bodies[0].appears").is_some());
}

#[test]
fn a_body_s_parts_are_a_list_each_under_its_own_header() {
    // S19's review, I5: each part under a header that says what it is, with a ⋮ that moves it
    // or removes it, one edit each; "+ Add part" adds one. Control: before the move the order is
    // the one added in. (A window 1100 high, so that both parts' ⋮ are in sight to be clicked;
    // 1600 until S20, when rows were 24 pt.)
    let mut rig = Rig::new();
    rig.size = [1440, 1100];
    rig.shell.run("load void").unwrap();
    rig.shell.add_starter("cube").unwrap();
    rig.shell.run("set bodies[0].shape.material steel").unwrap();
    rig.frames(vec![vec![], vec![]]);
    rig.click_button("add part bodies[0].shape.parts");
    rig.frames(vec![vec![]]);
    rig.shell.run("set bodies[0].shape.parts[0].material lead").unwrap();
    rig.click_button("add part bodies[0].shape.parts");
    rig.frames(vec![vec![], vec![]]);
    let materials = |rig: &Rig| -> Vec<Option<String>> {
        rig.shell.engine.spec().bodies[0]
            .shape
            .as_ref()
            .unwrap()
            .parts
            .iter()
            .map(|p| p.material.clone())
            .collect()
    };
    assert_eq!(materials(&rig), [Some("lead".to_owned()), Some("lead".to_owned())], "a copy of the last");
    rig.shell.run("set bodies[0].shape.parts[1].material copper").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let header = rig.shell.focus_tree().node("inspector/parts/0").map(|n| n.name.clone()).unwrap();
    assert_eq!(header, "Part 1 · Add lead · Sphere");
    assert_eq!(materials(&rig), [Some("lead".to_owned()), Some("copper".to_owned())], "control: as added");
    let depth = rig.shell.engine.depth().0;
    rig.click_button("menu bodies[0].shape.parts[0]");
    rig.click_button("move down bodies[0].shape.parts[0]");
    assert_eq!(materials(&rig), [Some("copper".to_owned()), Some("lead".to_owned())]);
    assert_eq!(rig.shell.engine.depth().0, depth + 1, "one edit");
    rig.frames(vec![vec![]]);
    rig.click_button("menu bodies[0].shape.parts[1]");
    rig.click_button("remove bodies[0].shape.parts[1]");
    assert_eq!(materials(&rig), [Some("copper".to_owned())]);
}

#[test]
fn the_kind_is_a_choice_of_a_point_or_a_shape_and_keeps_the_weight() {
    // S19's review, UX: the "Shape" box that made a point mass when unticked is a Kind drop-down.
    // A steel cube made a point mass keeps what it weighed (ρV), one edit; made a shape again it
    // is the sphere starter's shape. Control: nothing chosen, nothing changes.
    let mut rig = Rig::new();
    rig.shell.run("load void").unwrap();
    rig.shell.add_starter("cube").unwrap();
    rig.shell.run("set bodies[0].shape.material steel").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let spec = rig.shell.engine.spec().clone();
    let weighs = spec.bodies[0].mass_in(&spec).unwrap();
    let id = "field/bodies[0].shape";
    assert!(rig.shell.focus_tree().node(id).is_some(), "the Kind row");
    let pick = |rig: &mut Rig, down: bool| {
        rig.shell.keyboard.focus = vec!["inspector".into(), "inspector/shape".into(), id.into()];
        rig.shell.keyboard.nav_on = true;
        rig.key(Key::Enter, Modifiers::NONE);
        rig.key(if down { Key::ArrowDown } else { Key::ArrowUp }, Modifiers::NONE);
        rig.key(Key::Enter, Modifiers::NONE);
        rig.frames(vec![vec![], vec![]]);
    };
    let depth = rig.shell.engine.depth().0;
    pick(&mut rig, false);
    let body = rig.shell.engine.spec().bodies[0].clone();
    assert!(body.shape.is_none() && body.radius == 0.0, "a point mass");
    assert_eq!(body.mass.to_bits(), weighs.to_bits(), "it weighs what it weighed");
    assert_eq!(rig.shell.engine.depth().0, depth + 1, "one edit");
    pick(&mut rig, true);
    assert!(rig.shell.engine.spec().bodies[0].shape.is_some(), "a shape again");
    let before = rig.shell.engine.spec().clone();
    rig.frames(vec![vec![], vec![]]);
    assert_eq!(rig.shell.engine.spec(), &before, "control: nothing chosen, nothing changes");
}

#[test]
fn the_inspector_keeps_its_width_whatever_is_shown() {
    // S19's review, UX: the inspector was 352 pt for a body, 317 for the scene, 325 for a tower
    // and 390 for the Readings, so the view re-centred at each click in the tree. Now one width;
    // and a body with the longest material name and a long label does not widen it.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.add_starter("cube").unwrap();
    rig.shell.run("set bodies[1].shape.material \"stainless steel\"").unwrap();
    rig.shell.run("set bodies[1].label \"a body with a long name that goes on and on and on\"").unwrap();
    // Parts too: a solid and a stroke, each under its header.
    rig.shell
        .run(r#"add bodies[1].shape.parts {"mode":"Add","material":"lead","at":[0.45,0,0],"kind":{"Solid":{"shape":{"Sphere":{"radius":0.3}}}}}"#)
        .unwrap();
    rig.shell
        .run(r#"add bodies[1].shape.parts {"mode":"Paint","material":"copper","at":[0,0,0.3],"kind":{"Stroke":{"radius":0.1,"points":[[-0.3,-0.3,0],[0.3,0.3,0]]}}}"#)
        .unwrap();
    let mut widths = Vec::new();
    for selection in ["select body 1", "select body 2", "select none", "select tower 1", "readings"] {
        if selection == "readings" {
            rig.shell.right_tab = cav_app::outliner::RightTab::Readings;
        } else {
            rig.shell.run(selection).unwrap();
        }
        rig.frames(vec![vec![], vec![], vec![]]);
        let panel = rig.shell.layout().panel("properties").unwrap();
        widths.push((selection, panel.width()));
        // Nothing in it runs past its margin (4 points since S20; the 10 asked here is inside
        // it, where the scroll bar's width is kept), and every card is as wide as the first:
        // a row too wide for its card widened every card after it (S19's pictures: the cards'
        // menus were cut at the panel's edge).
        let layout = rig.shell.layout();
        let edge = panel.right() - 10.0;
        let over: Vec<String> = layout
            .editors
            .iter()
            .chain(&layout.buttons)
            // (The scroll bar's own place is in the margin's reserve: S20 names it, where the
            // "more below" badge was.)
            .filter(|(what, r)| {
                what != "inspector scroll bar" && panel.intersects(*r) && r.right() > edge + 0.5
            })
            .map(|(what, r)| format!("{what} to {}", r.right()))
            .collect();
        assert!(over.is_empty(), "{selection}: past the panel's edge at {edge}: {over:?}");
        let cards: Vec<(String, f32)> = layout
            .buttons
            .iter()
            .filter(|(b, r)| b.starts_with("fold inspector/") && panel.intersects(*r))
            .map(|(b, r)| (b.clone(), r.right()))
            .collect();
        assert!(cards.len() >= 2 || selection == "readings", "{selection}: {cards:?}");
        assert!(cards.iter().all(|(_, right)| (right - cards[0].1).abs() < 0.5), "{selection}: {cards:?}");
    }
    let first = widths[0].1;
    for (selection, width) in &widths {
        assert!((width - first).abs() < 0.5, "{selection}: {width} against {first}: {widths:?}");
    }
}

#[test]
fn selecting_brings_the_inspector_forward_and_a_click_keeps_the_selection() {
    // S19's review, UX, as in Unity, Godot and Blender: with Readings shown, selecting a row shows
    // the Inspector; a click on the selected row keeps it selected; Ctrl+click toggles it off.
    // Control: selecting nothing leaves the Readings tab shown.
    let mut rig = two_masses();
    rig.shell.run("select none").unwrap();
    rig.shell.right_tab = cav_app::outliner::RightTab::Readings;
    rig.frames(vec![vec![], vec![]]);
    rig.shell.run("select none").unwrap();
    rig.frames(vec![vec![]]);
    assert_eq!(rig.shell.right_tab, cav_app::outliner::RightTab::Readings, "control: nothing selected");
    let row = rig.row(Target::Body(1));
    rig.click(row.center());
    assert_eq!(rig.shell.engine.selection(), Some(Target::Body(1)));
    assert_eq!(rig.shell.right_tab, cav_app::outliner::RightTab::Inspector, "the Inspector forward");
    // (Time enough between clicks that none is a double click, which renames.)
    rig.frames(vec![vec![]; 40]);
    rig.click(row.center());
    assert_eq!(rig.shell.engine.selection(), Some(Target::Body(1)), "a click keeps it");
    rig.frames(vec![vec![]; 40]);
    let ctrl = |pressed| Event::PointerButton {
        pos: row.center(),
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::COMMAND,
    };
    rig.frames(vec![vec![Event::PointerMoved(row.center())], vec![ctrl(true)], vec![ctrl(false)], vec![]]);
    assert_eq!(rig.shell.engine.selection(), None, "Ctrl+click toggles it off");
}
#[test]
fn a_locked_body_is_not_moved_by_the_gizmo_and_a_hidden_one_is_not_drawn() {
    // docs/design/panels.md: the tree's lock and eye. Locked, the Move tool has no gizmo on the
    // body; hidden, the view does not draw it but the towers still read it. Controls: unlocked,
    // the drag moves it; shown, it is in the view's frame.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("select body 1").unwrap();
    rig.shell.run("tool move").unwrap();
    rig.frames(vec![vec![], vec![]]);
    rig.click_button("lock mass");
    assert!(rig.shell.is_locked(0));
    rig.frames(vec![vec![]]);
    let rect = rig.shell.layout().canvases[0];
    let spec = rig.shell.engine.spec().clone();
    let at = spec.bodies[0].motion.position(0.0, spec.array.instrument.gravity);
    let g = cav_app::tools::gizmo(rig.shell.engine.view(0), rect.size(), at).unwrap();
    let tip = g.tips[0].unwrap();
    let from = rect.min + g.centre.to_vec2() + (tip - g.centre) * 0.6;
    let start = anchor(&rig, 0);
    rig.drag(from, from + (tip - g.centre));
    assert_eq!(anchor(&rig, 0), start, "locked: not moved");
    rig.click_button("lock mass");
    rig.drag_arrow(0, 0, 1.0);
    assert_ne!(anchor(&rig, 0), start, "control: unlocked, moved");
    // The eye.
    rig.click_button("eye mass");
    assert_eq!(rig.shell.engine.view(0).hidden, ["mass".to_owned()]);
    assert!(rig.shell.engine.settle(std::time::Duration::from_secs(60)));
    let reading = rig.shell.reading().clone();
    assert!(reading.towers.first().is_some_and(|t| t.gradiometer != 0.0), "still read");
    rig.click_button("eye mass");
    assert!(rig.shell.engine.view(0).hidden.is_empty(), "control: shown again");
}

#[test]
fn a_body_made_uniform_keeps_what_it_weighed() {
    // S19's second review, minor 6: a body of materials may be saved with mass 0 (the field is
    // not read), and "none" in its Material drop-down then asked a uniform body to weigh 0, which
    // was refused. Now choosing none sets the mass to what the body weighed (ρV), one edit, as the
    // Kind switch does. Control: the plain edit of the material alone is still refused at mass 0.
    let mut rig = Rig::new();
    rig.shell.run("load void").unwrap();
    rig.shell.add_starter("cube").unwrap();
    rig.shell.run("set bodies[0].shape.material steel").unwrap();
    rig.shell.run("set bodies[0].mass 0").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let spec = rig.shell.engine.spec().clone();
    assert_eq!(spec.bodies[0].mass, 0.0);
    let weighs = spec.bodies[0].mass_in(&spec).unwrap();
    assert!(weighs > 1.0, "{weighs} kg of steel");
    assert!(rig.shell.run("set bodies[0].shape.material null").is_err(), "control: mass 0, uniform, refused");
    let id = "field/bodies[0].shape.material";
    assert!(rig.shell.focus_tree().node(id).is_some(), "the Material row");
    rig.shell.keyboard.focus = vec!["inspector".into(), "inspector/material".into(), id.into()];
    rig.shell.keyboard.nav_on = true;
    let depth = rig.shell.engine.depth().0;
    rig.key(Key::Enter, Modifiers::NONE);
    // Up to the first choice (none), wherever steel is in the list: the arrows stop at the end.
    for _ in 0..40 {
        rig.key(Key::ArrowUp, Modifiers::NONE);
    }
    rig.key(Key::Enter, Modifiers::NONE);
    rig.frames(vec![vec![], vec![]]);
    let body = rig.shell.engine.spec().bodies[0].clone();
    assert!(body.shape.as_ref().is_some_and(|s| s.material.is_none()), "uniform now");
    assert_eq!(body.mass.to_bits(), weighs.to_bits(), "it weighs what it weighed");
    assert_eq!(rig.shell.engine.depth().0, depth + 1, "one edit");
}
