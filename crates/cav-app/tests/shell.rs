//! The shell with no GPU: layout, keys, buttons, menus and the console, driven by the same input
//! events a window would deliver.

use cav_app::headless::{context, run_frames};
use cav_app::{NoSurface, Shell, menus};
use cav_engine::{Command, Level, LogFilter, Target, ViewCommand};
use cav_sim::{NamedView, Projection};
use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect, pos2, vec2};

struct Rig {
    ctx: egui::Context,
    shell: Shell,
    size: [u32; 2],
    clock: f64,
}

impl Rig {
    fn new(size: [u32; 2]) -> Self {
        let mut rig = Self { ctx: context(), shell: Shell::new(), size, clock: 0.0 };
        // Two frames: egui sizes panels on the first and settles on the second.
        rig.frames(vec![vec![], vec![]]);
        rig
    }

    fn frames(&mut self, frames: Vec<Vec<Event>>) {
        run_frames(&self.ctx, &mut self.shell, &mut NoSurface, self.size, &mut self.clock, frames);
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        let event = |pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
        self.frames(vec![vec![event(true)], vec![event(false)]]);
    }

    fn press(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
        Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE }
    }

    fn click(&mut self, pos: Pos2) {
        self.frames(vec![
            vec![Event::PointerMoved(pos)],
            vec![Self::press(pos, PointerButton::Primary, true)],
            vec![Self::press(pos, PointerButton::Primary, false)],
            vec![],
        ]);
    }

    fn drag(&mut self, from: Pos2, to: Pos2, button: PointerButton) {
        let middle = from + (to - from) * 0.5;
        self.frames(vec![
            vec![Event::PointerMoved(from)],
            vec![Self::press(from, button, true)],
            vec![Event::PointerMoved(middle)],
            vec![Event::PointerMoved(to)],
            vec![Self::press(to, button, false)],
            vec![],
        ]);
    }

    fn click_button(&mut self, line: &str) {
        let rect = self.shell.layout().button(line).unwrap_or_else(|| panic!("no button runs `{line}`"));
        self.click(rect.center());
    }

    fn last_command(&self) -> Option<&Command> {
        self.shell.engine.journal().last()
    }
}

fn overlap(a: Rect, b: Rect) -> f32 {
    let i = a.intersect(b);
    if i.is_positive() { i.area() } else { 0.0 }
}

const PANELS: [&str; 6] = ["menu", "footer", "timeline", "outliner", "properties", "header"];

#[test]
fn every_panel_is_laid_out_and_none_covers_the_figure() {
    // The helper the test leans on can see an overlap.
    assert_eq!(
        overlap(
            Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 10.0)),
            Rect::from_min_size(pos2(5.0, 5.0), vec2(10.0, 10.0))
        ),
        25.0
    );

    for size in [[1440, 900], [1024, 700]] {
        let mut figure_area_closed = 0.0;
        for console_open in [false, true] {
            let mut rig = Rig::new(size);
            if console_open {
                rig.shell.run("console").unwrap();
                rig.frames(vec![vec![], vec![]]);
            }
            let layout = rig.shell.layout().clone();
            let screen = layout.screen;
            assert_eq!(screen.size(), vec2(size[0] as f32, size[1] as f32));
            let mut names: Vec<&str> = PANELS.to_vec();
            if console_open {
                names.push("console");
            }
            assert_eq!(layout.panels.len(), names.len(), "{:?}", layout.panels);
            let mut covered = layout.figure.area();
            for name in &names {
                let rect = layout
                    .panel(name)
                    .unwrap_or_else(|| panic!("the {name} panel was not shown at {size:?}"));
                assert!(rect.width() >= 100.0 && rect.height() >= 20.0, "{name} is {rect:?}");
                assert!(screen.contains_rect(rect), "{name} {rect:?} leaves the window {screen:?}");
                // The rule: the figure is never covered.
                assert!(
                    overlap(rect, layout.figure) < 1.0,
                    "{name} {rect:?} covers the figure {:?}",
                    layout.figure
                );
                for other in &names {
                    if other != name {
                        assert!(
                            overlap(rect, layout.panel(other).unwrap()) < 1.0,
                            "{name} and {other} overlap"
                        );
                    }
                }
                covered += rect.area();
            }
            // Panels and figure tile the window: nothing is left over and nothing is counted twice.
            assert!(
                (covered / screen.area() - 1.0).abs() < 0.005,
                "panels and figure cover {covered} of {}",
                screen.area()
            );
            // The figure keeps a useful share of the window, and one pane is the whole figure.
            let share = layout.figure.area() / screen.area();
            assert!(
                share > if console_open { 0.15 } else { 0.3 },
                "the figure has {share:.2} of the window at {size:?}"
            );
            assert_eq!(layout.panes, vec![layout.figure]);
            if console_open {
                assert!(
                    layout.figure.area() < figure_area_closed,
                    "the console takes its space from the figure"
                );
            } else {
                figure_area_closed = layout.figure.area();
            }
        }
    }
}

#[test]
fn panes_split_the_figure() {
    let mut rig = Rig::new([1440, 900]);
    for count in [2, 4, 1] {
        rig.shell.run(&format!("panes {count}")).unwrap();
        rig.frames(vec![vec![]]);
        let layout = rig.shell.layout();
        assert_eq!(layout.panes.len(), count);
        let mut total = 0.0;
        for (n, pane) in layout.panes.iter().enumerate() {
            assert!(layout.figure.expand(0.01).contains_rect(*pane), "pane {n} leaves the figure");
            assert!(pane.width() > 200.0 && pane.height() > 150.0, "pane {n} is {pane:?}");
            for other in &layout.panes[n + 1..] {
                assert!(overlap(*pane, *other) < 0.01, "panes overlap");
            }
            total += pane.area();
        }
        // The panes fill the figure but for the thin gaps between them.
        let share = total / layout.figure.area();
        assert!(share > 0.98 && share <= 1.0001, "{count} panes cover {share} of the figure");
    }
}

#[test]
fn f3_hides_the_panels_and_the_figure_fills_the_window() {
    let mut rig = Rig::new([1440, 900]);
    assert_eq!(rig.shell.layout().panels.len(), PANELS.len(), "the control: panels are shown to begin with");
    assert!(rig.shell.layout().figure.area() < 0.7 * rig.shell.layout().screen.area());

    rig.key(Key::F3, Modifiers::NONE);
    assert!(rig.shell.gui_hidden);
    assert!(rig.shell.layout().panels.is_empty(), "{:?}", rig.shell.layout().panels);
    assert_eq!(rig.shell.layout().figure, rig.shell.layout().screen);
    assert_eq!(rig.shell.layout().button("view home"), None, "not even Home is on the figure");

    // Escape leaves it, as F3 again would.
    rig.key(Key::Escape, Modifiers::NONE);
    assert!(!rig.shell.gui_hidden);
    assert_eq!(rig.shell.layout().panels.len(), PANELS.len());
    // Hiding the GUI is not an engine command: the journal is untouched.
    assert!(rig.shell.engine.journal().is_empty());
}

#[test]
fn every_menu_item_is_a_console_command() {
    let lines = menus::lines();
    assert!(lines.len() >= 35, "the menus have {} runnable items", lines.len());
    for &line in &lines {
        // Each runs on a shell prepared so that undo and redo have something to act on.
        let mut shell = Shell::new();
        shell.run("load void").unwrap();
        if line == "redo" {
            shell.run("undo").unwrap();
        }
        let outcome = shell.run(line);
        assert!(outcome.is_ok(), "the menu item `{line}` is not a command that runs: {outcome:?}");
        // And it is either the window's own or one the engine's console parses.
        assert!(Shell::is_app_command(line) || Shell::new().engine.parse(line).is_ok(), "`{line}`");
    }
    // Every menu is reachable and none is empty.
    let titles: Vec<&str> = menus::MENUS.iter().map(|(title, _)| *title).collect();
    assert_eq!(titles, ["File", "Edit", "View", "Scene", "Array", "Dataset", "Windows", "Help"]);
    assert!(menus::MENUS.iter().all(|(_, items)| !items.is_empty()));
    // Control: a line that is not a command fails the same check, so the loop above can fail.
    let mut shell = Shell::new();
    assert!(shell.run("view sideways").is_err());
    assert!(!Shell::is_app_command("view sideways") && shell.engine.parse("view sideways").is_err());
    assert!(shell.console_open, "a refused command opens the console");
}

#[test]
fn keys_issue_engine_commands() {
    let mut rig = Rig::new([1440, 900]);
    assert!(rig.shell.engine.journal().is_empty(), "the control: drawing frames issues nothing");

    rig.key(Key::O, Modifiers::NONE);
    assert_eq!(
        rig.last_command(),
        Some(&Command::View(ViewCommand::Project { pane: 0, projection: Projection::Orthographic }))
    );
    rig.key(Key::O, Modifiers::NONE);
    assert_eq!(rig.shell.engine.view(0).projection, Projection::Perspective, "O toggles");

    rig.key(Key::Num3, Modifiers::NONE);
    assert_eq!(rig.last_command(), Some(&Command::Select(Some(Target::Tower(3)))));
    rig.key(Key::Num9, Modifiers::NONE);
    assert_eq!(
        rig.shell.engine.selection(),
        Some(Target::Tower(3)),
        "there is no tower 9, so 9 does nothing"
    );
    rig.key(Key::Escape, Modifiers::NONE);
    assert_eq!(rig.last_command(), Some(&Command::Select(None)));

    rig.shell.run("view top").unwrap();
    rig.key(Key::Home, Modifiers::NONE);
    assert_eq!(rig.last_command(), Some(&Command::View(ViewCommand::Home { pane: 0 })));
    assert_eq!(rig.shell.engine.view(0).named(), Some(NamedView::Iso));

    rig.key(Key::ArrowRight, Modifiers::NONE);
    rig.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(rig.shell.engine.time(), 0.2);
    rig.key(Key::ArrowLeft, Modifiers::NONE);
    assert_eq!(rig.shell.engine.time(), 0.1);

    // Undo with nothing to undo is refused, logged, and opens the console.
    let before = rig.shell.engine.digest();
    rig.key(Key::Z, Modifiers::COMMAND);
    assert_eq!(rig.shell.engine.digest(), before);
    assert!(rig.shell.console_open);
    let errors =
        rig.shell.engine.log().select(&LogFilter { min_level: Some(Level::Error), ..LogFilter::default() });
    assert!(errors.last().unwrap().message.contains("nothing to undo"));
    rig.key(Key::Escape, Modifiers::NONE);
    assert!(!rig.shell.console_open, "Escape closes the console");
    // With an edit to take back, Ctrl+Z and Ctrl+Shift+Z work.
    rig.shell.run("load void").unwrap();
    rig.key(Key::Z, Modifiers::COMMAND);
    assert_eq!(rig.shell.engine.spec().name, "room");
    rig.key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
    assert_eq!(rig.shell.engine.spec().name, "void");

    // Keys about the window, not the engine: no journal entry.
    let journal = rig.shell.engine.journal().len();
    rig.key(Key::Space, Modifiers::NONE);
    assert!(rig.shell.playing);
    rig.key(Key::Space, Modifiers::NONE);
    assert!(!rig.shell.playing);
    rig.key(Key::Backtick, Modifiers::NONE);
    assert!(rig.shell.console_open);
    rig.key(Key::Backtick, Modifiers::NONE);
    assert!(!rig.shell.console_open);
    // (Playing for two frames moved the playhead; that is a seek, coalesced with the last one.)
    assert!(rig.shell.engine.journal().len() <= journal + 1);
}

#[test]
fn buttons_and_the_pointer_issue_engine_commands() {
    let mut rig = Rig::new([1440, 900]);

    rig.click_button("view top");
    assert_eq!(rig.last_command(), Some(&Command::View(ViewCommand::Look { pane: 0, view: NamedView::Top })));
    rig.click_button("proj ortho");
    assert_eq!(rig.shell.engine.view(0).projection, Projection::Orthographic);
    rig.click_button("style paper");
    assert_eq!(rig.shell.engine.view(0).style, cav_sim::DrawStyle::Paper);
    rig.click_button("view home");
    assert_eq!(rig.shell.engine.view(0).named(), Some(NamedView::Iso), "the Home button on the figure");
    rig.click_button("panes 4");
    assert_eq!(rig.shell.engine.panes(), 4);

    // Clicking a pane makes it the active one; the view buttons then act on it.
    let panes = rig.shell.layout().panes.clone();
    rig.click(panes[2].center());
    assert_eq!(rig.last_command(), Some(&Command::View(ViewCommand::Activate(2))));
    rig.click_button("view west");
    assert_eq!(rig.shell.engine.view(2).named(), Some(NamedView::West));
    assert_eq!(rig.shell.engine.view(0).named(), Some(NamedView::Iso), "the first pane is untouched");

    // Dragging orbits: to the right turns the camera clockwise (yaw falls), down raises it.
    let before = rig.shell.engine.view(1).clone();
    let centre = panes[1].center();
    rig.drag(centre, centre + vec2(50.0, 20.0), PointerButton::Primary);
    assert_eq!(rig.shell.engine.active_pane(), 1, "a drag activates its pane");
    let after = rig.shell.engine.view(1).clone();
    assert!(
        matches!(rig.last_command(), Some(Command::View(ViewCommand::Orbit { pane: 1, .. }))),
        "{:?}",
        rig.last_command()
    );
    assert!((after.yaw - (before.yaw - 20.0)).abs() < 1e-3, "yaw {} → {}", before.yaw, after.yaw);
    // The top view starts at the pole, so the pitch stays clamped there.
    assert_eq!(after.pitch, 90.0);
    assert_eq!((after.target, after.distance), (before.target, before.distance));

    // A right-drag pans: the scene follows the pointer, so the target moves the other way.
    let before = rig.shell.engine.view(3).clone();
    let centre = panes[3].center();
    rig.drag(centre, centre + vec2(40.0, 0.0), PointerButton::Secondary);
    let after = rig.shell.engine.view(3).clone();
    assert!(
        matches!(rig.last_command(), Some(Command::View(ViewCommand::Pan { pane: 3, .. }))),
        "{:?}",
        rig.last_command()
    );
    // Pane 4 looks from the east: right on screen is north (+y). Dragging right moves the target south.
    assert!(after.target[1] < before.target[1] - 0.5, "{:?} → {:?}", before.target, after.target);
    assert_eq!((after.target[0], after.target[2]), (before.target[0], before.target[2]));
    assert_eq!((after.yaw, after.pitch, after.distance), (before.yaw, before.pitch, before.distance));

    // Scrolling over a pane zooms it: up moves the camera in.
    let before = rig.shell.engine.view(0).distance;
    let wheel = Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, 120.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: Modifiers::NONE,
    };
    rig.frames(vec![
        vec![Event::PointerMoved(panes[0].center())],
        vec![wheel],
        vec![],
        vec![],
        vec![],
        vec![],
    ]);
    assert!(
        matches!(rig.last_command(), Some(Command::View(ViewCommand::Zoom { pane: 0, .. }))),
        "{:?}",
        rig.last_command()
    );
    assert!(rig.shell.engine.view(0).distance < before);

    // Control: a click on a panel that is not a button issues nothing.
    let journal = rig.shell.engine.journal().len();
    let header = rig.shell.layout().panel("header").unwrap();
    rig.click(header.right_center() - vec2(30.0, 0.0));
    assert_eq!(rig.shell.engine.journal().len(), journal);
}

#[test]
fn the_timeline_scrubs_and_playback_moves_the_playhead() {
    let mut rig = Rig::new([1440, 900]);
    let timeline = rig.shell.layout().panel("timeline").unwrap();
    // Three quarters of the way along the lanes is three quarters of the way through the scene.
    let lanes = Rect::from_min_max(
        pos2(timeline.left() + 8.0 + 76.0, timeline.top()),
        pos2(timeline.right() - 8.0 - 18.0, timeline.bottom()),
    );
    let at = pos2(lanes.left() + 0.75 * lanes.width(), timeline.bottom() - 20.0);
    rig.click(at);
    assert!(matches!(rig.last_command(), Some(Command::Seek(_))), "{:?}", rig.last_command());
    assert!((rig.shell.engine.time() - 9.0).abs() < 0.05, "scrubbed to {}", rig.shell.engine.time());
    // A drag is a run of seeks: one journal entry.
    rig.drag(at, at - vec2(200.0, 0.0), PointerButton::Primary);
    assert!(rig.shell.engine.time() < 8.0);
    assert_eq!(rig.shell.engine.journal().len(), 1);

    // Playing advances the playhead by the frames' time, and loops at the end.
    rig.shell.run("seek 0").unwrap();
    rig.shell.run("play").unwrap();
    rig.frames(vec![vec![]; 30]);
    let played = rig.shell.engine.time();
    assert!(played > 0.3 && played < 0.7, "30 frames at 60 Hz played {played} s");
    rig.shell.run("seek 11.99").unwrap();
    rig.frames(vec![vec![]; 3]);
    assert!(rig.shell.engine.time() < 1.0, "the playhead loops: {}", rig.shell.engine.time());
    // Control: paused, frames do not move it.
    rig.shell.run("pause").unwrap();
    let paused = rig.shell.engine.time();
    rig.frames(vec![vec![]; 10]);
    assert_eq!(rig.shell.engine.time(), paused);
}

#[test]
fn the_console_shows_answers_and_keeps_window_commands_out_of_the_engine() {
    let mut shell = Shell::new();
    // A question opens the console and its answer is in the log the console shows.
    let answer = shell.run("get array.instrument.cycle").unwrap();
    assert_eq!(answer, "2.0");
    assert!(shell.console_open);
    assert_eq!(shell.engine.log().iter().next_back().unwrap().message, "2.0");
    // A command does not open it.
    let mut shell = Shell::new();
    shell.run("view top").unwrap();
    assert!(!shell.console_open);
    // The window's own commands never reach the engine's journal, and take no arguments.
    for (name, _) in cav_app::APP_COMMANDS {
        let mut shell = Shell::new();
        assert!(shell.run(name).is_ok(), "{name}");
        assert!(shell.engine.journal().is_empty(), "{name} reached the engine");
        assert!(shell.run(&format!("{name} now")).unwrap_err().contains("takes no arguments"));
    }
    let mut shell = Shell::new();
    shell.run("quit").unwrap();
    assert!(shell.quit);
}

#[test]
fn an_answer_is_one_log_entry_and_log_is_not_echoed() {
    let mut shell = Shell::new();
    let before = shell.engine.log().len();
    // `schema` answers with hundreds of lines. It is one entry, not hundreds.
    let schema = shell.run("schema").unwrap();
    assert!(schema.lines().count() > 100, "the schema is long: {} lines", schema.lines().count());
    assert_eq!(shell.engine.log().len(), before + 1);
    assert_eq!(shell.engine.log().iter().next_back().unwrap().message, schema, "and it is the whole answer");
    // The console shows the start of it and says how much it left out.
    let shown = cav_app::shell::abridged(&schema, 24);
    assert_eq!(shown.lines().count(), 25);
    assert!(shown.ends_with("more lines (`cav exec` prints them all)"), "{shown}");
    assert_eq!(cav_app::shell::abridged("one\ntwo", 24), "one\ntwo", "a short answer is shown whole");
    // Control: what came before is still there. Twenty long answers do not empty a 2000-entry log.
    for _ in 0..20 {
        shell.run("schema").unwrap();
    }
    assert!(
        shell.engine.log().iter().next().unwrap().message.starts_with("cavendish"),
        "the first entry survives"
    );
    // `log` shows the log; it does not add its own output to it.
    let entries = shell.engine.log().len();
    let listed = shell.run("log 5").unwrap();
    assert_eq!(listed.lines().filter(|l| l.contains("  info ")).count(), 5);
    assert_eq!(shell.engine.log().len(), entries, "`log` left the log as it was");
    assert!(shell.console_open);
}

#[test]
fn dragging_a_body_moves_it_under_the_pointer() {
    for projection in ["persp", "ortho"] {
        let mut rig = Rig::new([1440, 900]);
        rig.shell.run("load point").unwrap();
        rig.shell.run(&format!("proj {projection}")).unwrap();
        rig.frames(vec![vec![], vec![]]);
        let pane = rig.shell.layout().panes[0];
        let on_screen = |rig: &Rig| pane.min + rig.shell.body_point(0, 0, pane.size()).unwrap().to_vec2();
        let start = on_screen(&rig);
        assert!(pane.contains(start), "{projection}: the mass is in the picture at {start:?}");
        let journal = rig.shell.engine.journal().len();
        let before = rig.shell.engine.spec().bodies[0].clone();

        // Take hold of the mass and pull it 90 points right and 50 up.
        let end = start + vec2(90.0, -50.0);
        rig.drag(start, end, PointerButton::Primary);
        let after = on_screen(&rig);
        // It follows the pointer. egui starts a drag only once the pointer has moved a few points,
        // so the mass is that much behind it and no more.
        assert!(after.distance(end) <= 8.0, "{projection}: the mass is at {after:?}, the pointer at {end:?}");
        assert!(after.distance(start) > 80.0, "{projection}: it moved {} points", after.distance(start));
        // The drag selected it, and is a run of sets of its position: one entry after the selection.
        assert_eq!(rig.shell.engine.selection(), Some(Target::Body(1)));
        let Some(Command::Set { path, .. }) = rig.last_command() else {
            panic!("{projection}: the last command is {:?}", rig.last_command())
        };
        assert_eq!(path, "bodies[0].motion.Static.position");
        assert_eq!(rig.shell.engine.journal().len(), journal + 2, "a selection and one drag");
        assert_ne!(rig.shell.engine.spec().bodies[0], before);
        // One undo puts it back.
        rig.shell.run("undo").unwrap();
        assert_eq!(rig.shell.engine.spec().bodies[0], before);

        // Control: a drag that starts on empty picture orbits the camera and moves no body.
        let empty = pane.left_top() + vec2(40.0, 40.0);
        assert!(rig.shell.body_under(0, (empty - pane.min).to_pos2(), pane.size()).is_none());
        rig.drag(empty, empty + vec2(60.0, 20.0), PointerButton::Primary);
        assert!(matches!(rig.last_command(), Some(Command::View(ViewCommand::Orbit { .. }))));
        assert_eq!(rig.shell.engine.spec().bodies[0], before);
    }
}

#[test]
fn a_click_selects_a_body_and_the_panels_show_what_the_towers_read() {
    let mut rig = Rig::new([1440, 900]);
    rig.shell.run("load point").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let pane = rig.shell.layout().panes[0];
    let at = pane.min + rig.shell.body_point(0, 0, pane.size()).unwrap().to_vec2();
    rig.click(at);
    assert_eq!(rig.shell.engine.selection(), Some(Target::Body(1)));
    // Every tower has a reading under the playhead: the engine's, from its reading worker.
    let long = std::time::Duration::from_secs(60);
    assert!(rig.shell.engine.settle(long));
    let reply = rig.shell.reading().clone();
    let cav_engine::Reply::Reading(asked) =
        rig.shell.engine.query(&cav_engine::Query::Reading(None)).unwrap()
    else {
        panic!("readings")
    };
    assert_eq!(reply, asked);
    assert_eq!((reply.state.as_str(), reply.towers.len()), ("refined", 5));
    let readings = reply.towers;
    assert!(readings[1].gradiometer.abs() > readings[2].gradiometer.abs(), "tower 2 is the nearer");
    // Move the mass. Until the worker answers, the panels show the last reading, marked stale, and
    // never a number computed here; then the new one.
    rig.shell.engine.hold_workers(true);
    rig.shell.run("set bodies[0].motion.Static.position [-2, 0, 2.5]").unwrap();
    rig.frames(vec![vec![]]);
    let meanwhile = rig.shell.reading().clone();
    assert_eq!((meanwhile.state.as_str(), &meanwhile.towers), ("stale", &readings));
    rig.shell.engine.hold_workers(false);
    assert!(rig.shell.engine.settle(long));
    let moved = rig.shell.reading().clone();
    assert_eq!(moved.state, "refined");
    assert_ne!(moved.towers, readings);
    assert!(moved.towers[2].gradiometer.abs() > moved.towers[1].gradiometer.abs(), "now tower 3 is");
    // A reading the audit warns of says so beside the tower.
    rig.shell.run("set bodies[0].motion.Static.position [0, 0, 1]").unwrap();
    assert!(rig.shell.engine.settle(long));
    let warned = rig.shell.reading().clone();
    assert!(warned.warnings.iter().any(|w| w.contains("did not settle") && w.contains("tower 1")));
    assert!(warned.estimate > 1e-7);
    // The text a reading is shown as.
    use cav_app::shell::phase_text;
    assert_eq!(phase_text(-0.010_240_1), "-10.240 mrad");
    assert_eq!(phase_text(3.2e-6), "+3.200 µrad");
    assert_eq!(phase_text(4.8e3), "+4800.000 rad");
    assert_eq!(phase_text(2.5e-10), "+0.250 nrad");
    assert_eq!(phase_text(0.0), "+0.000 rad");
    assert_eq!(phase_text(f64::INFINITY), "unbounded");
    // Control: in a scene with nothing to read every reading is exactly zero.
    rig.shell.run("load void").unwrap();
    assert!(rig.shell.engine.settle(long));
    assert_eq!(rig.shell.readings().len(), 5);
    assert!(rig.shell.readings().iter().all(|r| r.gradiometer == 0.0 && r.lower == 0.0));
}

#[test]
fn panes_show_plots_and_the_scene() {
    use cav_app::plots::place;
    use cav_engine::PaneContent;
    let mut rig = Rig::new([1440, 900]);
    rig.shell.run("load oscillator-zoo").unwrap();
    rig.shell.run("set duration 400").unwrap();
    rig.shell.run("panes 4").unwrap();
    rig.shell.run("show signal @2").unwrap();
    rig.shell.run("show periodogram @3").unwrap();
    assert!(rig.shell.engine.settle(std::time::Duration::from_secs(60)));
    rig.shell.run("seek 100").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let layout = rig.shell.layout().clone();
    assert_eq!(layout.panes.len(), 4);
    // Two panes drew plots, each inside its own pane; the other two are the scene.
    let kinds: Vec<(usize, PaneContent)> = layout.plots.iter().map(|d| (d.pane, d.content)).collect();
    assert_eq!(kinds, [(1, PaneContent::Signal), (2, PaneContent::Periodogram)]);
    for drawn in &layout.plots {
        assert!(layout.panes[drawn.pane].contains_rect(drawn.area), "pane {}", drawn.pane + 1);
        assert_eq!(drawn.note, None);
    }
    // The signal: five towers of 200 shots, and the playhead a quarter of the way along.
    let signal = &layout.plots[0];
    assert_eq!(signal.points, 5 * 200);
    let quarter = signal.area.left() + 0.25 * signal.area.width();
    assert!((signal.playhead.unwrap() - quarter).abs() < 1.0);
    // Every value is inside the axes: the largest of the series is drawn within the area.
    let series = rig.shell.engine.signal().1.unwrap().clone();
    let peak = series.gradiometer.iter().fold(f64::MIN, |m, v| m.max(*v));
    let top = place(signal.area, &signal.x, &signal.y, (0.0, peak));
    assert!(signal.area.contains(top), "{top:?} in {:?}", signal.area);
    // The periodogram: 100 bins to Nyquist, and a stem for five harmonics of each of four bodies.
    let spectrum = &layout.plots[1];
    assert_eq!(spectrum.points, 100);
    assert_eq!(spectrum.stems.len(), 4 * 8, "eight harmonics of each of four bodies");
    let stem = |label: &str, n: u32| {
        spectrum
            .stems
            .iter()
            .find(|s| s.0 == label && s.1 == n)
            .unwrap_or_else(|| panic!("no stem {label} {n}"))
    };
    let at = |f: f64| spectrum.area.left() + spectrum.area.width() * (f / 0.25) as f32;
    // The linear oscillator at 0.05 Hz: harmonics where they are, and not folded …
    for n in 1..=4 {
        let (_, _, x, folded) = stem("linear", n);
        assert!((x - at(0.05 * f64::from(n))).abs() < 1.0 && !folded, "{n}f₀ at {x}");
    }
    // … the circular one at 0.35625 Hz: every harmonic folded, the second onto 0.2125 Hz.
    let (_, _, x, folded) = stem("circular", 1);
    assert!((x - at(0.143_75)).abs() < 1.0 && *folded);
    let (_, _, x, folded) = stem("circular", 2);
    assert!((x - at(0.2125)).abs() < 1.0 && *folded);
    // Nothing a plot writes is cut off by its pane, and the legend is clear of every stem's label.
    for drawn in &layout.plots {
        let pane = layout.panes[drawn.pane].expand(0.5);
        for (text, took) in &drawn.labels {
            assert!(pane.contains_rect(*took), "`{text}` at {took:?} leaves pane {pane:?}");
        }
    }
    let legend: Vec<&(String, egui::Rect)> =
        spectrum.labels.iter().filter(|l| l.0.contains("· f₀ =")).collect();
    assert_eq!(legend.len(), 4);
    for (text, took) in spectrum.labels.iter().filter(|l| l.0.ends_with("f₀") || l.0.ends_with("folded")) {
        assert!(legend.iter().all(|l| !l.1.intersects(*took)), "`{text}` is under the legend");
        assert!(took.top() >= spectrum.area.top() - 1.0, "`{text}` is above the plot");
    }
    // Both axes are marked between their ends: frequencies every 0.05 Hz, and the decades.
    let said = |text: &str| spectrum.labels.iter().any(|l| l.0 == text);
    assert!(said("0.05") && said("0.1") && said("0.2") && said("0.25"), "{:?}", spectrum.labels);
    assert!(spectrum.labels.iter().filter(|l| l.0.starts_with("1e-")).count() >= 3);
    assert!(
        (spectrum.y.lo.log10() - spectrum.y.lo.log10().round()).abs() < 1e-9,
        "the axis ends on a decade"
    );
    // A span shows a stretch of the axis: here the first 40 s of the signal, twenty shots a tower.
    rig.shell.run("span 0 40 @2").unwrap();
    rig.frames(vec![vec![]]);
    let zoomed = rig.shell.layout().plots[0].clone();
    assert_eq!((zoomed.x.lo, zoomed.x.hi), (0.0, 40.0));
    assert!(zoomed.points >= 5 * 20 && zoomed.points <= 5 * 22, "{} points", zoomed.points);
    assert_eq!(zoomed.playhead, None, "the playhead, at 100 s, is outside it");
    // The wheel zooms about the pointer, and `span all` shows all of it again.
    let centre = zoomed.area.center();
    let wheel = Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, 200.0),
        modifiers: Modifiers::NONE,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
    };
    rig.frames(vec![vec![Event::PointerMoved(centre)], vec![wheel]]);
    // The wheel is smoothed over some frames: let all of it arrive before reading the span.
    rig.frames(vec![vec![]; 40]);
    let closer = rig.shell.layout().plots[0].clone();
    assert!(closer.x.hi - closer.x.lo < 39.0, "{:?}", closer.x);
    assert!(((closer.x.lo + closer.x.hi) / 2.0 - 20.0).abs() < 0.5, "about the pointer: {:?}", closer.x);
    rig.shell.run("span all @2").unwrap();
    rig.frames(vec![vec![]]);
    let signal = &rig.shell.layout().plots[0].clone();
    assert_eq!((signal.x.lo, signal.x.hi), (0.0, 400.0));
    // Dragging on the signal plot moves the playhead to where the pointer is.
    let point = pos2(signal.area.left() + 0.6 * signal.area.width(), signal.area.center().y);
    rig.click(point);
    assert!((rig.shell.engine.time() - 240.0).abs() < 1.0, "the playhead is at {}", rig.shell.engine.time());
    // Control: back to the scene, and nothing is plotted in that pane.
    rig.shell.run("show scene @2").unwrap();
    rig.frames(vec![vec![]]);
    assert_eq!(rig.shell.layout().plots.len(), 1);
    // A scene with nothing to resolve says so instead of drawing an empty axis.
    rig.shell.run("load void").unwrap();
    rig.frames(vec![vec![]]);
    assert!(rig.shell.layout().plots[0].note.as_deref().unwrap().contains("constant"));
}
