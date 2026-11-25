//! The outliner lays out a bounded number of rows, whatever the scene holds.

use cav_app::headless::{context, run_frames};
use cav_app::{MAX_OUTLINE_ROWS, NoSurface, Shell};
use cav_engine::{Command, Target};

fn rows(shell: &mut Shell) -> Vec<Target> {
    let (ctx, mut clock) = (context(), 0.0);
    run_frames(&ctx, shell, &mut NoSurface, [1440, 900], &mut clock, vec![vec![], vec![]]);
    shell.layout().outline.iter().map(|(target, _)| *target).collect()
}

#[test]
fn the_outliner_lays_out_a_bounded_number_of_rows() {
    // S2's review, finding 11: every body and tower was laid out every frame; a scene may hold
    // a thousand of each.
    let mut scene = cav_sim::dev::point();
    for n in 1..300 {
        scene.bodies.push(cav_sim::BodySpec::point(&format!("body {n}"), 1.0, [n as f64, 0.0, 1.0]));
    }
    let mut shell = Shell::new();
    shell.engine.apply(Command::load(scene)).unwrap();
    let shown = rows(&mut shell);
    let bodies = |shown: &[Target]| shown.iter().filter(|t| matches!(t, Target::Body(_))).count();
    assert_eq!(bodies(&shown), MAX_OUTLINE_ROWS, "300 bodies, {MAX_OUTLINE_ROWS} rows");
    assert!(
        shown.contains(&Target::Body(MAX_OUTLINE_ROWS))
            && !shown.contains(&Target::Body(MAX_OUTLINE_ROWS + 1))
    );
    assert!(shown.contains(&Target::Tower(5)), "the towers are all there: five is under the cap");
    // What is selected keeps a row, wherever it is in the list.
    shell.run("select body 250").unwrap();
    let shown = rows(&mut shell);
    assert!(shown.contains(&Target::Body(250)));
    assert_eq!(bodies(&shown), MAX_OUTLINE_ROWS + 1);
    // Control: a scene under the cap is shown whole, so the cap is what cut the list.
    let mut small = Shell::new();
    small.run("load oscillator-zoo").unwrap();
    let count = small.engine.spec().bodies.len();
    assert_eq!(bodies(&rows(&mut small)), count);
}

#[test]
fn the_world_is_in_the_outliner_and_can_be_selected() {
    // S4's review, I12: a scene whose matter is a world has a row for it, under Surroundings,
    // that a click selects, as a body's does.
    let mut shell = Shell::new();
    shell.run("load world-box").unwrap();
    let shown = rows(&mut shell);
    assert!(shown.contains(&Target::World), "{shown:?}");
    let (_, rect) = *shell.layout().outline.iter().find(|(t, _)| *t == Target::World).unwrap();
    let at = rect.center();
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let (ctx, mut clock) = (context(), 0.0);
    let frames =
        vec![vec![], vec![egui::Event::PointerMoved(at)], vec![press(true)], vec![press(false)], vec![]];
    run_frames(&ctx, &mut shell, &mut NoSurface, [1440, 900], &mut clock, frames);
    assert_eq!(shell.engine.selection(), Some(Target::World));
    // The console says the same, and the journal's line for it reads back.
    shell.run("select none").unwrap();
    shell.run("select world").unwrap();
    assert_eq!(shell.engine.selection(), Some(Target::World));
    // Control: a scene without a world has no such row, and refuses to select one.
    let mut room = Shell::new();
    room.run("load point").unwrap();
    assert!(!rows(&mut room).contains(&Target::World));
    assert!(room.run("select world").unwrap_err().contains("no world"));
}
