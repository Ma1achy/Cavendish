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
