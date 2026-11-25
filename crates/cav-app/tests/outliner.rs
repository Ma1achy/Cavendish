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

#[test]
fn a_world_scene_s_towers_show_what_they_read() {
    // S4's second review, J5: with no bodies, a world scene still reads something, and the
    // towers' rows say what, marked when it is of part of the matter only.
    let mut shell = Shell::new();
    shell.engine.set_patience(std::time::Duration::from_secs(60));
    shell.run("load world-box").unwrap();
    shell.run("set world.read.radius 1").unwrap();
    assert!(shell.engine.settle(std::time::Duration::from_secs(60)));
    let reply = shell.reading().clone();
    assert_eq!(reply.truncated, Some(1.0));
    let position = shell.engine.spec().array.towers[0].position;
    let detail = cav_app::panels_for_tests::tower_detail(&reply, 0, position, false);
    assert!(detail.contains("rad") && detail.contains('◐'), "{detail}");
    assert!(!cav_app::panels_for_tests::silent(shell.engine.spec()));
    // Control: `point` with its body taken away reads nothing: the row shows where the tower is.
    let mut empty = cav_sim::dev::point();
    empty.bodies.clear();
    assert!(cav_app::panels_for_tests::silent(&empty));
    assert_eq!(cav_app::panels_for_tests::tower_detail(&reply, 0, [1.0, -2.0, 0.0], true), "+1.0 -2.0");
}

/// A surface that draws nothing and says what it was told to.
struct Saying(Vec<String>);

impl cav_app::panes::PaneSurface for Saying {
    fn texture(
        &mut self,
        _: usize,
        _: [u32; 2],
        _: u64,
        _: cav_render::Frame<'_>,
    ) -> Option<egui::TextureId> {
        None
    }

    fn notes(&self) -> Vec<String> {
        self.0.clone()
    }
}

#[test]
fn what_the_renderer_left_out_is_shown_and_logged_once_a_kind() {
    // S4's second review, minor 5: the renderer's notes become the header's badges, and each
    // kind (the words before the colon) is logged once, the near field's two caps apart.
    let notes = vec![
        "near field vertices: meshes past 10 vertices are not drawn (the nearest are)".to_owned(),
        "near field chunks: chunks past 5 are not drawn (the nearest are)".to_owned(),
        "far field cut: its 3 farthest visible bricks are not drawn".to_owned(),
    ];
    let mut shell = Shell::new();
    shell.run("load world-box").unwrap();
    let logged =
        |shell: &Shell| shell.engine.log().iter().filter(|e| e.message.contains("are not drawn")).count();
    let (ctx, mut clock) = (context(), 0.0);
    let mut surface = Saying(notes.clone());
    run_frames(&ctx, &mut shell, &mut surface, [1440, 900], &mut clock, vec![vec![], vec![]]);
    assert_eq!(shell.render_notes, notes);
    assert_eq!(logged(&shell), 3, "each kind once");
    // The counts in a note change from frame to frame: the same kinds are not logged again.
    surface.0[2] = "far field cut: its 4 farthest visible bricks are not drawn".to_owned();
    run_frames(&ctx, &mut shell, &mut surface, [1440, 900], &mut clock, vec![vec![], vec![]]);
    assert_eq!(logged(&shell), 3);
    // Control: a surface that leaves nothing out leaves no badge.
    run_frames(&ctx, &mut shell, &mut Saying(Vec::new()), [1440, 900], &mut clock, vec![vec![]]);
    assert!(shell.render_notes.is_empty());
}
