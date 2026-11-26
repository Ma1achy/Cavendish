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

/// Whether `painted` has more digits than `label` has: a number besides the name.
fn more_digits(label: &str, painted: &str) -> bool {
    let digits = |t: &str| t.chars().filter(char::is_ascii_digit).count();
    digits(painted) > digits(label)
}

#[test]
fn the_readings_tab_shows_the_engine_s_reading_bit_for_bit() {
    // docs/design/panels.md: each tower's phase in the Readings tab is the engine's reading, bit
    // for bit (the row is drawn from the values recorded, its full value under the pointer), and
    // its text is that value's. Control: another tower's value is another number, so a row drawn
    // from the wrong tower would fail.
    let mut shell = Shell::new();
    shell.run("load point").unwrap();
    assert!(shell.engine.settle(std::time::Duration::from_secs(60)));
    shell.right_tab = cav_app::outliner::RightTab::Readings;
    rows(&mut shell);
    let reply = shell.reading().clone();
    let drawn = shell.layout().reading_values.clone();
    assert_eq!(drawn.len(), reply.towers.len(), "a row a tower");
    let bits = |v: [f64; 3]| v.map(f64::to_bits);
    for (n, values) in &drawn {
        let r = &reply.towers[*n];
        assert_eq!(bits(*values), bits([r.gradiometer, r.lower, r.upper]), "tower {}", n + 1);
        let text = &shell.layout().readings.iter().find(|(t, _)| t == n).unwrap().1;
        assert_eq!(*text, cav_app::shell::phase_text(r.gradiometer), "tower {}", n + 1);
    }
    // Control: some tower reads another number than the first, so rows drawn from the wrong
    // tower would not match.
    let first = reply.towers[0].gradiometer.to_bits();
    assert!(reply.towers.iter().any(|t| t.gradiometer.to_bits() != first), "control: the towers differ");
}

/// What the Readings tab wrote for tower `n` (from 0) after a couple of frames: the numbers moved
/// there from the tree's rows (S19, `docs/design/panels.md`).
fn reading_words(shell: &mut Shell, n: usize) -> String {
    shell.right_tab = cav_app::outliner::RightTab::Readings;
    rows(shell);
    let found = shell.layout().readings.iter().find(|(t, _)| *t == n);
    found.unwrap_or_else(|| panic!("no reading for tower {}", n + 1)).1.clone()
}

#[test]
fn slow_a_world_scene_s_towers_show_what_they_read() {
    // S4's second review J5, third review K2: with no bodies, a world scene still reads
    // something, and the Readings tab says what, marked ◐ when it is of part of the matter only;
    // the tree's status dot says the same. (Until S19 the tree's row carried the number.)
    let mut shell = Shell::new();
    shell.engine.set_patience(std::time::Duration::from_secs(60));
    shell.run("load world-box").unwrap();
    shell.run("set world.read.radius 1").unwrap();
    assert!(shell.engine.settle(std::time::Duration::from_secs(60)));
    let words = reading_words(&mut shell, 0);
    assert!(words.contains("rad") && words.contains('◐'), "{words}");
    let dot = shell.layout().dots.iter().find(|(t, _)| *t == 0).map(|(_, d)| *d);
    assert_eq!(dot, Some(cav_app::outliner::Dot::Partial));
    let reply = shell.reading().clone();
    assert_eq!(reply.truncated, Some(1.0));
    assert!(!cav_app::panels_for_tests::silent(shell.engine.spec()));
    // Control: `point` with its body taken away reads nothing: no number, a silent dot.
    let mut empty = cav_sim::dev::point();
    empty.bodies.clear();
    let mut nothing = Shell::new();
    nothing.engine.apply(Command::load(empty)).unwrap();
    assert!(nothing.engine.settle(std::time::Duration::from_secs(60)));
    assert_eq!(reading_words(&mut nothing, 0), "");
    let dot = nothing.layout().dots.iter().find(|(t, _)| *t == 0).map(|(_, d)| *d);
    assert_eq!(dot, Some(cav_app::outliner::Dot::Silent));
}

#[test]
fn the_outliner_is_one_tree_of_named_rows() {
    // S19, docs/design/panels.md: the scene's groups (Array, Bodies, Surroundings) fold, each
    // row names a thing and carries no number. Control: the rows of S18 carried "100 kg ·
    // static" and the towers' phases.
    for scene in ["point", "town", "world-box"] {
        let mut shell = Shell::new();
        shell.run(&format!("load {scene}")).unwrap();
        rows(&mut shell);
        let groups: Vec<String> = shell.layout().tree_groups.iter().map(|(g, _)| g.clone()).collect();
        assert_eq!(groups, ["tree/array", "tree/sources", "tree/surroundings"], "{scene}");
        // What each row painted (recorded where it paints: S19's review, I6) carries no digit
        // beyond its label's own.
        let rows = &shell.layout().outline_words;
        assert!(!rows.is_empty());
        for (_, label, painted) in rows {
            assert!(!more_digits(label, painted), "{scene}: `{label}` painted `{painted}`");
            assert_eq!(painted, label, "{scene}: a row paints its name");
        }
        assert_eq!(shell.layout().dots.len(), shell.engine.spec().array.towers.len(), "a dot a tower");
    }
    // Control: the measure finds a number where a row paints one, as S18's tower rows did.
    let reply = Shell::new().reading().clone();
    let old = cav_app::panels_for_tests::tower_detail(&reply, 0, [1.0, 2.0, 0.0], true);
    assert!(more_digits("tower 1", &format!("tower 1 {old}")), "control: S18's rows: {old}");
    // A group folds by a click on its row, and its rows go; the fold is kept for the scene.
    let mut shell = Shell::new();
    shell.run("load point").unwrap();
    rows(&mut shell);
    let bodies_before = shell.layout().outline.iter().filter(|(t, _)| matches!(t, Target::Body(_))).count();
    assert!(bodies_before > 0);
    shell.tree_folded.insert(("point".to_owned(), "tree/sources".to_owned()));
    rows(&mut shell);
    assert!(!shell.layout().outline.iter().any(|(t, _)| matches!(t, Target::Body(_))), "folded");
    assert!(shell.layout().tree_groups.contains(&("tree/sources".to_owned(), false)));
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
