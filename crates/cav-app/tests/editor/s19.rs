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
