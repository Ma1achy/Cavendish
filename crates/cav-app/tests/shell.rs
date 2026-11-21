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
