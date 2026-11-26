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

    /// Clicks the button that runs `line`; one that is a row of a viewport header's menu (the
    /// camera's, the overlays') is reached by opening that menu first, as a person would. (The
    /// owner's ruling of 25 Nov 2026 replaced S17's one "View ›" popover by a header a viewport:
    /// a line now names its viewport, `view top @1`.)
    fn click_button(&mut self, line: &str) {
        let pane = line.rsplit_once('@').map_or("1", |(_, n)| n).to_owned();
        for menu in ["camera", "overlays", "type"] {
            if self.shell.layout().button(line).is_some() {
                break;
            }
            // (A click on the next opener shuts the menu tried before.)
            let opener = self.shell.layout().button(&format!("header {menu} {pane}")).expect("the header");
            self.click(opener.center());
        }
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

/// The panels; the tool strip is not one since S19's panels (it floats over the view's edge),
/// nor the header strip since S20 (each viewport has its own header, inside the figure), nor the
/// tool's options (S20: a row inside a viewport, under its header, shown only while the tool in
/// hand has settings; it was a panel above the figure).
const PANELS: [&str; 5] = ["menu", "footer", "timeline", "outliner", "properties"];

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
                let least = 100.0;
                assert!(rect.width() >= least && rect.height() >= 20.0, "{name} is {rect:?}");
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
    // (S20: the headers and the navigation gizmo go with the panels; it was the Home button.)
    assert!(rig.shell.layout().headers.is_empty() && rig.shell.layout().button("gizmo 1").is_none());
    assert_eq!(rig.shell.layout().canvases, vec![rig.shell.layout().screen], "nothing but the canvas");

    // Escape leaves it, as F3 again would.
    rig.key(Key::Escape, Modifiers::NONE);
    assert!(!rig.shell.gui_hidden);
    assert_eq!(rig.shell.layout().panels.len(), PANELS.len());
    // Hiding the GUI is not an engine command: the journal is untouched.
    assert!(rig.shell.engine.journal().is_empty());
}

#[test]
fn every_menu_item_is_an_action_that_runs() {
    // The menus are a view of the action table (docs/slices/S18.md D3): nothing greyed out
    // that works, every item a line that runs.
    let lines = menus::lines();
    assert!(lines.len() >= 50, "the menus have {} runnable items", lines.len());
    for line in &lines {
        // A scene's load is parsed, not run: loading the town here would start its world.
        if line.starts_with("load ") {
            assert!(Shell::new().engine.parse(line).is_ok(), "`{line}`");
            continue;
        }
        // Each runs on a shell prepared so that undo, redo and the selection's items have
        // something to act on: tower 2 is selected, and `{selection}` is it.
        let mut shell = Shell::new();
        shell.run("load void").unwrap();
        shell.run("select tower 2").unwrap();
        if line == "redo" {
            shell.run("undo").unwrap();
        }
        // (S20: closing a viewport or swapping two needs a second one, as redo needs an undo;
        // with one they are refused, which the control below holds.)
        if line.starts_with("viewport ") {
            let alone = shell.run(line);
            assert_eq!(
                alone.is_err(),
                matches!(line.as_str(), "viewport close" | "viewport swap"),
                "`{line}`"
            );
            shell.run("panes 2").unwrap();
        }
        // (TASK-S20-06: a step back needs somewhere before the playhead to go, as redo needs
        // an undo: at the start it is refused in words, which is held here; then it runs from
        // the middle of the timeline.)
        if line.starts_with("step -") {
            assert!(shell.run(line).is_err_and(|why| why.contains("the playhead")), "`{line}`");
            shell.run("seek 6").unwrap();
        }
        let line = Shell::resolve(line, shell.engine.selection());
        // It is either the window's own or one the engine's console parses (S18's review, R4:
        // this parsed a fixed line, so it could not fail).
        assert!(Shell::is_app_command(&line) || shell.engine.parse(&line).is_ok(), "`{line}`");
        let outcome = shell.run(&line);
        assert!(outcome.is_ok(), "the menu item `{line}` is not a command that runs: {outcome:?}");
    }
    // Every menu is reachable and none is empty.
    let titles = menus::titles();
    assert_eq!(titles, ["File", "Edit", "Add", "View", "Scene", "Array", "Help"]);
    // The Add menu is the starters; every other menu has actions.
    assert!(!cav_sim::starters::starters().is_empty());
    assert!(
        titles.iter().filter(|title| *title != "Add").all(|title| cav_app::actions::table()
            .actions
            .iter()
            .any(|a| a.menu.starts_with(title.as_str())))
    );
    // Every starter the Add menu offers is added where the camera looks, and selected.
    for starter in cav_sim::starters::starters() {
        let mut shell = Shell::new();
        shell.run("load void").unwrap();
        shell.add_starter(&starter.name).unwrap_or_else(|why| panic!("{}: {why}", starter.name));
        assert!(shell.engine.selection().is_some(), "{} is selected once added", starter.name);
    }
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

    // The arrows step the playhead in the timeline's scrubber, adjusting it (the keyboard spec;
    // before S18 they did from anywhere). From the viewport, Tab twice is the timeline.
    rig.key(Key::Tab, Modifiers::NONE);
    rig.key(Key::Tab, Modifiers::NONE);
    assert_eq!(rig.shell.keyboard.focus, ["timeline"]);
    rig.key(Key::Enter, Modifiers::NONE);
    // Control: in navigation mode the arrows move between the timeline's elements, not the time.
    while rig.shell.keyboard.focused() != "timeline/scrubber" {
        rig.key(Key::ArrowRight, Modifiers::NONE);
        assert_eq!(rig.shell.engine.time(), 0.0, "an arrow in navigation mode is not a step");
    }
    rig.key(Key::Enter, Modifiers::NONE);
    rig.key(Key::ArrowRight, Modifiers::NONE);
    rig.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(rig.shell.engine.time(), 0.2);
    rig.key(Key::ArrowLeft, Modifiers::NONE);
    assert_eq!(rig.shell.engine.time(), 0.1);
    rig.key(Key::ArrowRight, Modifiers::SHIFT);
    assert!((rig.shell.engine.time() - 1.1).abs() < 1e-12, "Shift ×10");
    rig.key(Key::Escape, Modifiers::NONE);
    rig.key(Key::Escape, Modifiers::NONE);
    assert_eq!(rig.shell.keyboard.focus, ["timeline"]);

    // Home is the one camera move undo takes back (the owner's rule, S18): Ctrl+Z puts the top
    // view back.
    rig.key(Key::Z, Modifiers::COMMAND);
    assert_eq!(rig.shell.engine.view(0).named(), Some(NamedView::Top), "undo after Home");
    assert!(!rig.shell.console_open);
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

    // (Rewritten for S20's viewport header, the owner's ruling: the rows are the header's camera
    // menu and shading group; paper is the View menu's, Home the camera menu's, and the count of
    // panes is the mosaic's, split from the View menu or a right-click.)
    rig.click_button("view top @1");
    assert_eq!(rig.last_command(), Some(&Command::View(ViewCommand::Look { pane: 0, view: NamedView::Top })));
    rig.click_button("proj ortho @1");
    assert_eq!(rig.shell.engine.view(0).projection, Projection::Orthographic);
    rig.click_button("style flat @1");
    assert_eq!(rig.shell.engine.view(0).style, cav_sim::DrawStyle::Flat, "the shading group");
    rig.shell.run_action("style-paper").unwrap();
    assert_eq!(rig.shell.engine.view(0).style, cav_sim::DrawStyle::Paper);
    rig.click_button("view home @1");
    assert_eq!(rig.shell.engine.view(0).named(), Some(NamedView::Iso), "Home in the camera menu");
    rig.shell.run_action("panes-4").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert_eq!(rig.shell.engine.panes(), 4);

    // Clicking a pane makes it the active one; the view buttons then act on it.
    let panes = rig.shell.layout().panes.clone();
    rig.click(panes[2].center());
    assert_eq!(rig.last_command(), Some(&Command::View(ViewCommand::Activate(2))));
    rig.click_button("view west @3");
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

    // A middle-drag pans: the scene follows the pointer, so the target moves the other way.
    // (Since S18's review, U3, the right button flies, as in Unity and Unreal: `editor.rs`'s
    // `the_right_button_flies_and_holds_the_letters`.)
    let before = rig.shell.engine.view(3).clone();
    let centre = panes[3].center();
    rig.drag(centre, centre + vec2(40.0, 0.0), PointerButton::Middle);
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
    // (Since S18 the wheel zooms toward the pointer: one camera command, target and distance.)
    assert!(
        matches!(rig.last_command(), Some(Command::View(ViewCommand::Camera { pane: 0, .. }))),
        "{:?}",
        rig.last_command()
    );
    assert!(rig.shell.engine.view(0).distance < before);

    // Control: a click on a panel that is not a button issues nothing.
    let journal = rig.shell.engine.journal().len();
    // (S20: the menu bar's right end, where the scene's name is written; it was the options
    // panel above the figure, which is gone.)
    let menu = rig.shell.layout().panel("menu").unwrap();
    rig.click(menu.right_center() - vec2(30.0, 0.0));
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
        let outcome = shell.run(name);
        // `rename` and `context` need a selection, which a fresh shell has not, and `tool` a tool's name: each
        // says so and changes nothing.
        assert!(
            outcome.is_ok()
                || (matches!(*name, "rename" | "context")
                    && outcome.as_ref().unwrap_err().contains("select"))
                || (*name == "tool" && outcome.as_ref().unwrap_err().contains("tool <"))
                // (S20: the palette's two commands take a word each, and say which.)
                || (*name == "tool-palette" && outcome.as_ref().unwrap_err().contains("tool-palette <"))
                || (*name == "flyout" && outcome.as_ref().unwrap_err().contains("flyout <"))
                || (*name == "layout" && outcome.as_ref().unwrap_err().contains("layout <"))
                // (TASK-S20-06: the transport's two commands take a number each, and say so.)
                || (*name == "step" && outcome.as_ref().unwrap_err().contains("step <"))
                || (*name == "speed" && outcome.as_ref().unwrap_err().contains("speed <"))
                || (*name == "reset" && outcome.as_ref().unwrap_err().contains("reset <"))
                || (*name == "focus" && outcome.as_ref().unwrap_err().contains("focus <"))
                || (*name == "pointer" && outcome.as_ref().unwrap_err().contains("pointer <"))
                || (*name == "probe-at" && outcome.as_ref().unwrap_err().contains("probe-at <"))
                || (*name == "tab" && outcome.as_ref().unwrap_err().contains("tab <"))
                || (*name == "viewport" && outcome.as_ref().unwrap_err().contains("viewport <")),
            "{name}: {outcome:?}"
        );
        // `frame` works out a camera and issues it: the journal holds that one camera command
        // (so the session replays), never the window's own word.
        let journal = shell.engine.journal();
        if *name == "frame" {
            assert!(
                matches!(journal, [Command::View(ViewCommand::Camera { .. })]),
                "frame issues one camera command: {journal:?}"
            );
        } else {
            assert!(journal.is_empty(), "{name} reached the engine");
        }
        // The others take nothing. (`focus` takes an id, which before the first frame it waits
        // for; `palette` takes a menu's name, `palette add`, or words to search for, `palette
        // sph`, since the review fixes of S18: any word is one.)
        // `import-mesh` takes the file to import (S19): any word is one.
        if matches!(*name, "focus" | "palette" | "import-mesh") {
            continue;
        }
        let wrong = shell.run(&format!("{name} now")).unwrap_err();
        assert!(
            wrong.contains("takes no arguments")
                || (*name == "palette" && wrong.contains("not a menu"))
                || (*name == "tool" && wrong.contains("tool <"))
                || (*name == "tool-palette" && wrong.contains("tool-palette <"))
                || (*name == "flyout" && wrong.contains("flyout <"))
                || (*name == "layout" && wrong.contains("layout <"))
                || (*name == "step" && wrong.contains("step <"))
                || (*name == "speed" && wrong.contains("speed <"))
                || (*name == "tab" && wrong.contains("tab <"))
                || (*name == "pointer" && wrong.contains("pointer <"))
                || (*name == "probe-at" && wrong.contains("probe-at <"))
                || (*name == "viewport" && wrong.contains("viewport <"))
                || *name == "reset"
                || *name == "focus",
            "{name}: {wrong}"
        );
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
        // The canvas, not the viewport: the pointer's place is measured from under the header (S20).
        let pane = rig.shell.layout().canvases[0];
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
        // (Clear of the tool palette, which floats at the canvas's top left.)
        let empty = pane.left_top() + vec2(120.0, 40.0);
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
    let pane = rig.shell.layout().canvases[0];
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
fn four_plots_in_a_small_window_keep_their_text_in_their_panes() {
    // Review 29 (S2.md §7.14), written first: every line of text a plot writes is the first of
    // its forms that fits, the last cut short with an ellipsis. At 1280 × 720 with four panes,
    // each a plot (cadence among them), every text's rectangle is inside its pane. Control: the
    // cadence legend's old line, measured the same way, is wider than the pane's axes.
    use cav_engine::{PaneContent, Query, Reply};
    let mut rig = Rig::new([1280, 720]);
    rig.shell.run("load comb").unwrap();
    rig.shell.run("set array.schedule.loss 0.05").unwrap();
    rig.shell.run("panes 4").unwrap();
    rig.shell.run("show signal @1").unwrap();
    rig.shell.run("show periodogram @2").unwrap();
    rig.shell.run("show cadence @3").unwrap();
    rig.shell.run("show flight @4").unwrap();
    assert!(rig.shell.engine.settle(std::time::Duration::from_secs(60)));
    rig.frames(vec![vec![], vec![]]);
    let layout = rig.shell.layout().clone();
    let kinds: Vec<PaneContent> = layout.plots.iter().map(|d| d.content).collect();
    assert_eq!(
        kinds,
        [PaneContent::Signal, PaneContent::Periodogram, PaneContent::Cadence, PaneContent::Flight]
    );
    for drawn in &layout.plots {
        let pane = layout.panes[drawn.pane].expand(0.5);
        for (text, took) in &drawn.labels {
            assert!(
                pane.contains_rect(*took),
                "{:?}: `{text}` at {took:?} leaves pane {pane:?}",
                drawn.content
            );
        }
    }
    // Control: the legend's line as it was written before, whole, in the same face and size.
    let cadence = &layout.plots[2];
    let Ok(Reply::Cadence(predicted)) = rig.shell.engine.query(&Query::Cadence) else {
        panic!("a prediction")
    };
    let fewest = predicted.kept.iter().min().unwrap();
    let most = predicted.kept.iter().max().unwrap();
    let old = format!(
        "mean cycle {} s · Nyquist {} Hz · {} shots planned, {fewest} to {most} taken a tower · smallest detectable tone × {:.3}",
        predicted.mean_cycle,
        predicted.nyquist,
        predicted.shots,
        predicted.noise_factor.unwrap()
    );
    let style = cav_plot::style::Style::screen();
    let wide = cav_plot::drawing::Measure::size(
        &cav_plot::fonts::Outlines,
        &old,
        cav_plot::drawing::Face::Sans,
        style.text_size,
    )[0];
    assert!(
        wide > cadence.area.width(),
        "the old line, {wide} points, fits the axes' {}",
        cadence.area.width()
    );
    assert!(
        cadence.labels.iter().any(|(text, _)| text.starts_with("mean cycle")),
        "the line is written, in a form that fits"
    );
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
    // No stem's label is written over another's, of its own comb or of another body's; a label
    // with nowhere to go is left out, but each body's f₀ is always named.
    let stems: Vec<&(String, egui::Rect)> =
        spectrum.labels.iter().filter(|l| l.0.ends_with("f₀") || l.0.ends_with("folded")).collect();
    for (k, a) in stems.iter().enumerate() {
        for b in &stems[k + 1..] {
            assert!(!a.1.intersects(b.1), "`{}` and `{}` are written over each other", a.0, b.0);
        }
    }
    assert!(stems.len() >= 8, "only {} stems are named", stems.len());
    assert_eq!(stems.iter().filter(|l| l.0 == "f₀" || l.0 == "f₀ folded").count(), 4);
    // Both axes are marked between their ends: frequencies every 0.05 Hz, and the decades.
    let said = |text: &str| spectrum.labels.iter().any(|l| l.0 == text);
    assert!(said("0.05") && said("0.1") && said("0.2") && said("0.25"), "{:?}", spectrum.labels);
    // Decades are written as powers: `10` with the exponent raised (review 29).
    assert!(spectrum.tick_labels[1].iter().filter(|said| said.starts_with("10^-")).count() >= 3);
    assert!(spectrum.labels.iter().filter(|l| l.0 == "10").count() >= 3);
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

#[test]
fn every_schema_field_has_a_widget() {
    use cav_app::editors::{Widget, widget_for};
    use cav_sim::paths::{self, Kind};
    // Every field that holds a value, in a scene with every kind of motion, a slab and a room.
    let mut spec = cav_sim::dev::oscillator_zoo();
    spec.bodies.extend(cav_sim::dev::pass().bodies);
    spec.room = cav_sim::SceneSpec::room().room;
    spec.floor = Some(cav_sim::FloorSpec {
        height: -1.0,
        kind: cav_sim::FloorKind::Slab { thickness: 0.5 },
        density: 2400.0,
    });
    let leaves = paths::leaves(&spec, "", 10_000).unwrap();
    assert!(leaves.len() > 80, "{} fields", leaves.len());
    let mut seen = std::collections::BTreeSet::new();
    for entry in &leaves {
        let widget = widget_for(entry);
        // The widget is the one for the field's kind: nothing falls through to a default.
        let fits = match (&entry.kind, &entry.value, widget) {
            (_, serde_json::Value::Null, Widget::Absent) => true,
            (Kind::Number, _, Widget::Number) | (Kind::Integer, _, Widget::Integer) => true,
            (Kind::Vector(n), _, Widget::Vector(m)) => *n == m,
            (Kind::Text, _, Widget::Text) | (Kind::Flag, _, Widget::Flag) => true,
            (Kind::Choice(_), value, Widget::Choice) => value.is_string(),
            (Kind::Choice(_), value, Widget::Variant) => value.is_object(),
            _ => false,
        };
        assert!(fits, "{}: a {:?} got {widget:?}", entry.path, entry.kind);
        seen.insert(format!("{widget:?}").split('(').next().unwrap().to_owned());
    }
    // Numbers are shown as short as says them, and never as nothing.
    use cav_app::editors::shown;
    for (value, text) in [
        (1.4431558652473234e-25, "1.44316e-25"),
        (3.86, "3.86"),
        (0.73, "0.73"),
        (1000.0, "1000"),
        (0.0, "0"),
        (-2.5, "-2.5"),
        (0.30000000000000004, "0.3"),
        (123456.789, "123457"),
        (5e-324, "4.94066e-324"),
    ] {
        assert_eq!(shown(value), text);
    }
    // Control: the scene does exercise every widget there is.
    let all = ["Absent", "Choice", "Flag", "Integer", "Number", "Text", "Variant", "Vector"];
    assert_eq!(seen.iter().map(String::as_str).collect::<Vec<_>>(), all);

    // On screen: select a body and every field under it has an editor, in `ls` order; select
    // nothing and every field of the instrument and the schedule has (since S18 the array's own
    // settings are the scene's, not each body's). Nothing the schema has is left out of the panel.
    let mut rig = Rig::new([1440, 900]);
    rig.shell.run("load oscillator-zoo").unwrap();
    let spec = rig.shell.engine.spec().clone();
    for (selection, prefixes) in
        [("select body 1", &["bodies[0]"][..]), ("select none", &["array.instrument", "array.schedule"][..])]
    {
        rig.shell.run(selection).unwrap();
        rig.frames(vec![vec![], vec![]]);
        // Nothing selected, the instrument and cadence are folded (S18's review, U1): opened by
        // a click on each header, as a person would.
        if selection == "select none" {
            for fold in ["fold inspector/instrument", "fold inspector/cadence"] {
                let header = rig.shell.layout().button(fold).expect("the folded section's header");
                rig.click(header.center());
                rig.frames(vec![vec![], vec![]]);
            }
        }
        // (S20: a body that is always there has its Lifetime card folded at first; opened the
        // same way, so that the panel is asked for every field as before.)
        if let Some(header) = rig.shell.layout().button("fold inspector/lifetime") {
            rig.click(header.center());
            rig.frames(vec![vec![], vec![]]);
        }
        let shown: Vec<String> = rig.shell.layout().editors.iter().map(|(path, _)| path.clone()).collect();
        for prefix in prefixes {
            for entry in paths::leaves(&spec, prefix, 1000).unwrap() {
                assert!(shown.contains(&entry.path), "{selection}: no editor for {}: {shown:?}", entry.path);
            }
        }
    }
    rig.shell.run("select body 1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let shown: Vec<String> = rig.shell.layout().editors.iter().map(|(path, _)| path.clone()).collect();
    assert!(!shown.iter().any(|path| path.starts_with("bodies[1]")), "only the selection's");
    assert!(!shown.iter().any(|path| path.starts_with("array.")), "a body's inspector is the body's");
    // Control: another selection, other editors.
    rig.shell.run("select tower 2").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let layout = rig.shell.layout();
    assert!(layout.editor("array.towers[1].position").is_some() && layout.editor("bodies[0].mass").is_none());
    assert!(layout.editor("array.towers[1].label").is_some());
}

#[test]
fn a_dragged_editor_is_one_edit() {
    let mut rig = Rig::new([1440, 900]);
    rig.shell.run("load point").unwrap();
    rig.shell.run("select body 1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let before = rig.shell.engine.spec().clone();
    let journal = rig.shell.engine.journal().len();
    let editor = rig.shell.layout().editor("bodies[0].mass").expect("the mass has an editor");
    // Drag the number to the right, in several steps: it grows, and the whole drag is one entry
    // in the journal and one step of undo.
    let from = editor.left_center() + vec2(12.0, 0.0);
    rig.frames(vec![
        vec![Event::PointerMoved(from)],
        vec![Rig::press(from, PointerButton::Primary, true)],
        vec![Event::PointerMoved(from + vec2(20.0, 0.0))],
        vec![Event::PointerMoved(from + vec2(45.0, 0.0))],
        vec![Event::PointerMoved(from + vec2(70.0, 0.0))],
        vec![Rig::press(from + vec2(70.0, 0.0), PointerButton::Primary, false)],
        vec![],
    ]);
    let mass = rig.shell.engine.spec().bodies[0].mass;
    assert!(mass > before.bodies[0].mass, "the mass grew: {mass}");
    assert_eq!(rig.shell.engine.journal().len(), journal + 1, "one entry for the whole drag");
    let Some(Command::Set { path, continues, .. }) = rig.last_command() else { panic!("a set") };
    assert_eq!((path.as_str(), *continues), ("bodies[0].mass", false), "an edit of its own, as replayed");
    rig.shell.run("undo").unwrap();
    assert_eq!(rig.shell.engine.spec(), &before, "one undo takes the whole drag back");
    // The range is the schema's: dragged far to the left the mass stops at the bottom of its
    // range and the engine refuses nothing silently (zero itself is not a mass).
    let editor = rig.shell.layout().editor("bodies[0].mass").unwrap();
    let from = editor.left_center() + vec2(12.0, 0.0);
    rig.drag(from, from - vec2(900.0, 0.0), PointerButton::Primary);
    assert!(rig.shell.engine.spec().bodies[0].mass > 0.0);
    // Control: a second drag after the first is a second edit.
    let journal = rig.shell.engine.journal().len();
    let editor = rig.shell.layout().editor("bodies[0].radius").unwrap();
    let from = editor.left_center() + vec2(12.0, 0.0);
    rig.drag(from, from + vec2(60.0, 0.0), PointerButton::Primary);
    assert_eq!(rig.shell.engine.journal().len(), journal + 1);
    assert!(rig.shell.engine.spec().bodies[0].radius > 0.0);
}

#[test]
fn all_entry_points_issue_the_same_edit() {
    use cav_app::editors::command_for;
    use serde_json::json;
    // The editor, by its own function: the command is the one the console's line parses to.
    let engine = cav_engine::Engine::new();
    for (path, value, line) in [
        ("array.instrument.launch_floor", json!(false), "set array.instrument.launch_floor false"),
        ("duration", json!(30.5), "set duration 30.5"),
        ("name", json!("mine"), "set name mine"),
        ("array.towers[1].position[0]", json!(4.25), "set array.towers[1].position[0] 4.25"),
        ("room", json!(null), "set room null"),
    ] {
        let typed = engine.parse(line).unwrap();
        assert_eq!(typed, cav_engine::Request::Do(command_for(path, value, false)), "{line}");
    }
    // The editor, on screen: a click on the launch floor's tick box puts in the journal the
    // command that the line puts there (and MCP's scene_set and `cav set` put there: their
    // tests compare their journals with the same `Command::set`). The window is tall enough
    // for the whole instrument to be on screen: since the kick number became a field, the flag
    // is one row lower, and in a window 900 high it is reached by scrolling the panel. (1200
    // since S19's panels: cards with headers and 24 pt rows take more height.)
    let mut rig = Rig::new([1440, 1200]);
    rig.shell.run("select tower 1").unwrap();
    // The reading is waited for before anything is clicked: when it arrives the properties
    // panel gains its rows and every editor below them moves. (Not waiting, the click landed
    // where the tick box had been, once in three runs on a busy machine.)
    assert!(rig.shell.engine.settle(std::time::Duration::from_secs(60)));
    rig.frames(vec![vec![], vec![]]);
    let tick = rig.shell.layout().editor("array.instrument.launch_floor").expect("the flag has an editor");
    let panel =
        rig.shell.layout().panels.iter().find(|(name, _)| *name == "properties").expect("the panel").1;
    assert!(panel.contains_rect(tick), "the tick box is on screen: {tick:?} in {panel:?}");
    rig.click(tick.left_center() + vec2(8.0, 0.0));
    let by_click = rig.last_command().cloned();
    assert_eq!(by_click, Some(Command::set("array.instrument.launch_floor", json!(false))));
    assert!(!rig.shell.engine.spec().array.instrument.launch_floor);
    let mut typed = cav_engine::Engine::new();
    typed.run_line("select tower 1").unwrap();
    typed.run_line("set array.instrument.launch_floor false").unwrap();
    assert_eq!(typed.journal().last().cloned(), by_click);
    assert_eq!(typed.spec(), rig.shell.engine.spec());
    // Control: clicking again issues the other value, so the click is the flag's and not a
    // constant.
    rig.frames(vec![vec![], vec![]]);
    let tick = rig.shell.layout().editor("array.instrument.launch_floor").unwrap();
    rig.click(tick.left_center() + vec2(8.0, 0.0));
    assert_eq!(rig.last_command(), Some(&Command::set("array.instrument.launch_floor", json!(true))));
    // The buttons that change a list are commands as well: since S19's panels, Delete is in the
    // header's ⋮ menu (`docs/design/panels.md`), not a loose button among the fields.
    rig.shell.run("load point").unwrap();
    rig.shell.run("select body 1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    rig.click_button("inspector menu");
    rig.click_button("remove bodies[0]");
    assert_eq!(rig.last_command(), Some(&Command::Remove { path: "bodies[0]".into() }));
    assert!(rig.shell.engine.spec().bodies.is_empty());
}

/// The staggered scene with a timeline short enough for its shots to be drawn one by one.
fn staggered_rig() -> Rig {
    let mut rig = Rig::new([1440, 900]);
    rig.shell.run("load staggered").unwrap();
    rig.shell.run("set duration 20").unwrap();
    rig.frames(vec![vec![], vec![]]);
    rig
}

#[test]
fn lanes_place_shots_at_detector_time() {
    let rig = staggered_rig();
    let lanes = rig.shell.layout().lanes.clone();
    assert_eq!(lanes.len(), 5);
    // A bar runs from the tower's launch to its readout: 2T = 1.46 s wide, which gives the
    // timeline's scale, and each tower's first bar starts its own offset after tower 1's.
    let points_per_second = lanes[0].1.width() / 1.46;
    assert!(points_per_second > 20.0, "{points_per_second} points a second");
    for (n, (tower, bar)) in lanes.iter().enumerate() {
        assert_eq!(*tower, n);
        assert!((bar.width() - 1.46 * points_per_second).abs() <= 1.0, "tower {}: {bar:?}", n + 1);
        let expected = lanes[0].1.left() + 0.292 * n as f32 * points_per_second;
        assert!((bar.left() - expected).abs() <= 1.0, "tower {}: {} against {expected}", n + 1, bar.left());
    }
    // Control: towers that fire together have their bars in one place.
    let mut together = Rig::new([1440, 900]);
    together.shell.run("load comb").unwrap();
    together.shell.run("set duration 20").unwrap();
    together.frames(vec![vec![], vec![]]);
    let lanes = &together.shell.layout().lanes;
    assert!(lanes.iter().all(|(_, bar)| bar.left() == lanes[0].1.left()));
}

#[test]
fn lane_drag_edits_offset() {
    let mut rig = staggered_rig();
    let lanes = rig.shell.layout().lanes.clone();
    let points_per_second = f64::from(lanes[0].1.width()) / 1.46;
    let before = rig.shell.engine.spec().clone();
    let journal = rig.shell.engine.journal().len();
    // Shift held, tower 4's lane dragged 60 points to the right.
    let shift = Modifiers { shift: true, ..Modifiers::NONE };
    // A key with no meaning here, sent each frame so that the frame knows Shift is down.
    let held =
        Event::Key { key: Key::F20, physical_key: None, pressed: true, repeat: true, modifiers: shift };
    let from = lanes[3].1.center();
    let press = |pos: Pos2, pressed: bool| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: shift,
    };
    rig.frames(vec![
        vec![held.clone(), Event::PointerMoved(from)],
        vec![press(from, true)],
        vec![held.clone(), Event::PointerMoved(from + vec2(20.0, 0.0))],
        vec![held.clone(), Event::PointerMoved(from + vec2(40.0, 0.0))],
        vec![held.clone(), Event::PointerMoved(from + vec2(60.0, 0.0))],
        vec![press(from + vec2(60.0, 0.0), false)],
        vec![],
    ]);
    // The offset grew by the drag over the timeline's scale. (egui starts a drag only once the
    // pointer has moved a few points, so the lane trails the pointer by up to that much.)
    let offset = rig.shell.engine.spec().array.towers[3].timing.offset;
    let moved = (offset - before.array.towers[3].timing.offset) * points_per_second;
    assert!((moved - 60.0).abs() <= 8.0, "the lane moved {moved} points for a drag of 60");
    // One edit: one journal entry, the right path, one undo; the playhead did not move.
    assert_eq!(rig.shell.engine.journal().len(), journal + 1);
    let Some(Command::Set { path, continues, .. }) = rig.last_command() else { panic!("a set") };
    assert_eq!((path.as_str(), *continues), ("array.towers[3].timing.offset", false));
    assert_eq!(rig.shell.engine.time(), 0.0);
    // The bar is drawn where the tower now fires.
    rig.frames(vec![vec![]]);
    let bar = rig.shell.layout().lanes[3].1;
    assert!((f64::from(bar.left() - lanes[3].1.left()) - moved).abs() <= 1.0);
    rig.shell.run("undo").unwrap();
    assert_eq!(rig.shell.engine.spec(), &before);
    // Control: the same drag without Shift scrubs, and edits nothing.
    rig.frames(vec![vec![]]);
    rig.drag(from, from + vec2(60.0, 0.0), PointerButton::Primary);
    assert_eq!(rig.shell.engine.spec(), &before);
    assert!(rig.shell.engine.time() > 0.0);
}

const JITTERED: &[&str] = &[
    r#"set array.schedule.jitter {"model":"Walk","kind":"Positive","delta":0.5}"#,
    "set array.schedule.seed 3",
];
const GAPPED: &[&str] = &[r#"set array.schedule.gaps {"on":4,"period":10}"#];
const LOSSY: &[&str] = &["set array.schedule.loss 0.3"];
const SCHEDULES: [(&str, &[&str]); 3] = [("jittered", JITTERED), ("gapped", GAPPED), ("lossy", LOSSY)];

/// The comb over a minute (thirty shots on the plain schedule) with a schedule set.
fn scheduled_rig(lines: &[&str]) -> Rig {
    let mut rig = Rig::new([1440, 900]);
    rig.shell.run("load comb").unwrap();
    rig.shell.run("set duration 60").unwrap();
    for line in lines {
        rig.shell.run(line).unwrap();
    }
    rig.frames(vec![vec![], vec![]]);
    rig
}

/// Where time zero is on the lanes and how many points a second is, from the plain schedule,
/// whose first bar runs from 0 to 2T: the same window, so the same scale.
fn lane_scale() -> (f32, f32) {
    let plain = scheduled_rig(&[]);
    let bar = plain.shell.layout().lanes[0].1;
    (bar.left(), bar.width() / 1.46)
}

#[test]
fn lanes_draw_the_schedule_s_shots() {
    // I1 of S2's review: the lanes drew k·cycle, all taken, whatever the schedule.
    let (zero, per_second) = lane_scale();
    for (name, lines) in SCHEDULES {
        let rig = scheduled_rig(lines);
        let shots = cav_sim::read::shots(rig.shell.engine.spec()).unwrap();
        let drawn = &rig.shell.layout().shots;
        for tower in 0..5 {
            let bars: Vec<_> = drawn.iter().filter(|bar| bar.tower == tower).collect();
            assert_eq!(bars.len(), shots.count(tower), "{name}, tower {}: as many bars as shots", tower + 1);
            for (k, bar) in bars.iter().enumerate() {
                let (launch, readout) = (shots.launch(tower, k), shots.readout(tower, k));
                let expected = [zero + launch as f32 * per_second, zero + readout as f32 * per_second];
                assert!(
                    bar.k == k
                        && (bar.rect.left() - expected[0]).abs() <= 1.0
                        && (bar.rect.right() - expected[1]).abs() <= 1.0,
                    "{name}, tower {}, shot {k}: {:?} against {expected:?}",
                    tower + 1,
                    bar.rect
                );
            }
        }
        // Control: the old arithmetic (shot k launched at k·cycle, ⌈duration/cycle⌉ of them, all
        // taken) does not describe this scene: by more than a point, or in what is taken.
        let furthest = (0..shots.count(0))
            .map(|k| (shots.launch(0, k) - 2.0 * k as f64).abs() as f32 * per_second)
            .fold(0.0f32, f32::max);
        let all_taken = shots.valid.iter().all(|taken| *taken);
        assert!(furthest > 1.0 || !all_taken, "{name}: the plain schedule would do");
        if name == "jittered" {
            assert!(furthest > 20.0 && shots.count(0) != 30, "{furthest} points, {}", shots.count(0));
        }
    }
}

#[test]
fn a_shot_not_taken_is_a_hollow_bar() {
    for (name, lines) in [("gapped", GAPPED), ("lossy", LOSSY)] {
        let rig = scheduled_rig(lines);
        let shots = cav_sim::read::shots(rig.shell.engine.spec()).unwrap();
        let drawn = &rig.shell.layout().shots;
        assert_eq!(drawn.len(), 5 * 30);
        for bar in drawn {
            assert_eq!(
                bar.filled,
                shots.taken(bar.tower, bar.k),
                "{name}: tower {}, shot {}",
                bar.tower + 1,
                bar.k
            );
        }
        let hollow = drawn.iter().filter(|bar| !bar.filled).count();
        let not_taken = shots.valid.iter().filter(|taken| !**taken).count();
        assert!(
            hollow == not_taken && hollow > 20,
            "{name}: {hollow} hollow bars, {not_taken} shots not taken"
        );
    }
    // The gaps are the same for every tower; each tower loses its own shots.
    let lossy = scheduled_rig(LOSSY);
    let of = |tower: usize| -> Vec<bool> {
        lossy.shell.layout().shots.iter().filter(|bar| bar.tower == tower).map(|bar| bar.filled).collect()
    };
    assert_ne!(of(0), of(1));
    // Control: the plain schedule, which is all the lanes used to draw, has no hollow bar.
    let plain = scheduled_rig(&[]);
    assert!(
        plain.shell.layout().shots.len() == 150 && plain.shell.layout().shots.iter().all(|bar| bar.filled)
    );
}

#[test]
fn the_captions_count_the_shots_taken() {
    let mut rig = scheduled_rig(&[GAPPED[0], LOSSY[0]]);
    assert!(rig.shell.engine.settle(std::time::Duration::from_secs(60)));
    rig.frames(vec![vec![], vec![]]);
    let shots = cav_sim::read::shots(rig.shell.engine.spec()).unwrap();
    let taken: Vec<usize> =
        (0..5).map(|tower| shots.taken_all(tower).iter().filter(|taken| **taken).count()).collect();
    let (fewest, most) = (*taken.iter().min().unwrap(), *taken.iter().max().unwrap());
    assert!(fewest < most && most < 12, "{taken:?}: 4 of every 10, less those lost");
    let layout = rig.shell.layout();
    let said = format!("30 shots planned, {fewest} to {most} taken a tower");
    assert!(layout.timeline_caption.contains(&said), "{}", layout.timeline_caption);
    assert!(layout.timeline_caption.contains("an outline: not taken"));
    let said = format!("forward · 30 shots, {fewest} to {most} taken a tower · refined");
    assert!(layout.status.starts_with(&said), "{}", layout.status);
    // Control: the plain schedule takes every shot, and says only how many there are.
    let mut plain = scheduled_rig(&[]);
    assert!(plain.shell.engine.settle(std::time::Duration::from_secs(60)));
    plain.frames(vec![vec![], vec![]]);
    let layout = plain.shell.layout();
    assert!(layout.timeline_caption.contains("· 30 shots a tower ·"), "{}", layout.timeline_caption);
    assert!(
        !layout.timeline_caption.contains("taken")
            && layout.status.starts_with("forward · 30 shots · refined")
    );
}

#[test]
fn a_lane_drags_on_an_uneven_schedule() {
    let (_, per_second) = lane_scale();
    let mut rig = scheduled_rig(&[JITTERED[0], JITTERED[1], LOSSY[0]]);
    let bars = |rig: &Rig| -> Vec<cav_app::timeline::Bar> {
        rig.shell.layout().shots.iter().filter(|bar| bar.tower == 3).copied().collect()
    };
    let before = bars(&rig);
    // Control: this is not the plain schedule: the tenth bar is nowhere near ten cycles in.
    let tenth = (before[10].rect.left() - before[0].rect.left()) / per_second;
    assert!((tenth - 20.0).abs() > 1.0, "the tenth shot is launched {tenth} s after the first");
    let journal = rig.shell.engine.journal().len();
    let shift = Modifiers { shift: true, ..Modifiers::NONE };
    let held =
        Event::Key { key: Key::F20, physical_key: None, pressed: true, repeat: true, modifiers: shift };
    let from = before[4].rect.center();
    let press = |pos: Pos2, pressed: bool| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: shift,
    };
    rig.frames(vec![
        vec![held.clone(), Event::PointerMoved(from)],
        vec![press(from, true)],
        vec![held.clone(), Event::PointerMoved(from + vec2(20.0, 0.0))],
        vec![held.clone(), Event::PointerMoved(from + vec2(40.0, 0.0))],
        vec![held.clone(), Event::PointerMoved(from + vec2(60.0, 0.0))],
        vec![press(from + vec2(60.0, 0.0), false)],
        vec![],
        vec![],
    ]);
    let offset = rig.shell.engine.spec().array.towers[3].timing.offset;
    let moved = offset as f32 * per_second;
    assert!((moved - 60.0).abs() <= 8.0, "the lane moved {moved} points for a drag of 60");
    assert_eq!(rig.shell.engine.journal().len(), journal + 1, "one edit");
    // Every bar of that lane followed, taken or not, and no other lane moved.
    let after = bars(&rig);
    assert_eq!(after.len(), before.len());
    for (now, then) in after.iter().zip(&before) {
        assert!((now.rect.left() - then.rect.left() - moved).abs() <= 1.0 && now.filled == then.filled);
    }
    assert!(after.iter().any(|bar| !bar.filled), "the scene loses shots");
    let other = rig.shell.layout().shots.iter().find(|bar| bar.tower == 2).unwrap();
    assert!((other.rect.left() - before[0].rect.left()).abs() <= 1.0);
}

#[test]
fn the_flight_diagram_draws_the_schedule_s_shots() {
    // I1 of S2's review: the diagram drew (first + k)·cycle + offset, all taken.
    for (name, lines) in SCHEDULES {
        let mut rig = scheduled_rig(lines);
        rig.shell.run("show flight").unwrap();
        rig.shell.run("seek 30").unwrap();
        rig.frames(vec![vec![], vec![]]);
        let drawn = rig.shell.layout().plots[0].clone();
        assert_eq!((drawn.x.lo, drawn.x.hi), (26.0, 34.0));
        let per_second = drawn.area.width() / 8.0;
        let x_of = |seconds: f64| drawn.area.left() + (seconds - 26.0) as f32 * per_second;
        let shots = cav_sim::read::shots(rig.shell.engine.spec()).unwrap();
        let mut off_the_grid = 0.0f32;
        for tower in 0..5 {
            let row: Vec<_> = drawn.flights.iter().filter(|flight| flight.0 == tower).collect();
            let touching: Vec<usize> = shots.touching(tower, [26.0, 34.0]).collect();
            assert_eq!(row.len(), touching.len(), "{name}, tower {}", tower + 1);
            assert!(row.len() >= 3, "{name}: {} shots in eight seconds", row.len());
            for (flight, k) in row.iter().zip(touching) {
                let (launch, readout) = (shots.launch(tower, k), shots.readout(tower, k));
                assert!((flight.1 - x_of(launch)).abs() <= 1.0 && (flight.2 - x_of(readout)).abs() <= 1.0);
                assert_eq!(flight.3, shots.taken(tower, k), "{name}: tower {}, shot {k}", tower + 1);
                // How far the launch is from the nearest whole cycle, where the old diagram drew it.
                let cycles = launch / 2.0;
                off_the_grid = off_the_grid.max((cycles - cycles.round()).abs() as f32 * 2.0 * per_second);
            }
        }
        // Control: the old arithmetic is not this scene's: a shot is off the even grid by more
        // than a point, or some shot drawn is not taken.
        let dashed = drawn.flights.iter().filter(|flight| !flight.3).count();
        assert!(off_the_grid > 1.0 || dashed > 0, "{name}: the plain schedule would do");
        match name {
            "jittered" => assert!(off_the_grid > 10.0 && dashed == 0, "{off_the_grid} points"),
            _ => assert!(dashed > 0 && off_the_grid <= 1.0, "{name}: {dashed} dashed"),
        }
        let caption = drawn.labels.iter().any(|(text, _)| text.contains("shot(s) not taken"));
        assert_eq!(caption, dashed > 0, "{name}: the caption says what dashed means");
    }
}

#[test]
fn flight_rows_share_absolute_time() {
    let mut rig = staggered_rig();
    rig.shell.run("show flight").unwrap();
    rig.shell.run("seek 6").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let drawn = rig.shell.layout().plots[0].clone();
    assert_eq!(drawn.content, cav_engine::PaneContent::Flight);
    // The pane follows the playhead: two cycles either side, on one time axis for every row.
    assert_eq!((drawn.x.lo, drawn.x.hi), (2.0, 10.0));
    let points_per_second = drawn.area.width() / 8.0;
    // Each row's first shot launched inside the span: the same shot of the schedule in every
    // row (the one the array's clock launches at 2 s). A shot launched before the span is drawn
    // only where its flight reaches into it, which depends on the row's offset.
    let first = |tower: usize| {
        let inside = |(row, launch, _, _): &&(usize, f32, f32, bool)| {
            *row == tower && *launch >= drawn.area.left() - 0.5
        };
        drawn.flights.iter().find(inside).copied().expect("a shot")
    };
    for tower in 0..5 {
        let (_, launch, readout, taken) = first(tower);
        assert!(taken, "the staggered scene takes every shot");
        // A shot is 2T wide, and tower n's is drawn n·0.292 s to the right of tower 1's.
        assert!((readout - launch - 1.46 * points_per_second).abs() <= 1.0);
        let expected = first(0).1 + 0.292 * tower as f32 * points_per_second;
        assert!((launch - expected).abs() <= 1.0, "tower {}: {launch} against {expected}", tower + 1);
    }
    // The playhead crosses every row at its own time.
    let playhead = drawn.playhead.expect("the playhead is in the span");
    assert!((playhead - (drawn.area.left() + 4.0 * points_per_second)).abs() <= 1.0);
    // Control: fired together, every row's shots stand in one place.
    rig.shell.run("load comb").unwrap();
    rig.shell.run("seek 6").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let drawn = rig.shell.layout().plots[0].clone();
    let x_of = |tower: usize| drawn.flights.iter().find(|(row, _, _, _)| *row == tower).unwrap().1;
    assert!((0..5).all(|tower| x_of(tower) == x_of(0)));
}

#[test]
fn the_flight_diagram_and_the_lanes_are_bounded() {
    use cav_app::plots::{MAX_FLIGHT_ROWS, MAX_FLIGHT_SHOTS};
    // Twelve towers and a span of a thousand shots: eight rows of sixty-four shots are drawn,
    // and the pane says what it left out.
    let mut rig = Rig::new([1440, 900]);
    let mut scene = cav_sim::dev::comb();
    scene.array.towers = (0..12)
        .map(|n| cav_sim::TowerSpec::at(&format!("tower {}", n + 1), [f64::from(n) * 3.0, 8.0, 0.0]))
        .collect();
    rig.shell.apply(Command::load(scene));
    rig.shell.run("show flight").unwrap();
    rig.shell.run("span 0 2000").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let drawn = rig.shell.layout().plots[0].clone();
    let rows: std::collections::BTreeSet<usize> = drawn.flights.iter().map(|(row, _, _, _)| *row).collect();
    assert_eq!(rows.len(), MAX_FLIGHT_ROWS);
    assert_eq!(drawn.flights.len(), MAX_FLIGHT_ROWS * MAX_FLIGHT_SHOTS);
    let note = drawn.note.expect("it says what it left out");
    assert!(note.starts_with("4 tower(s) and "), "{note}");
    // The lanes: twelve towers, eight lanes' height, and only the lanes in view are drawn.
    assert!(rig.shell.layout().lanes.is_empty(), "a thousand shots a lane are a band, not bars");
    rig.shell.run("set duration 20").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let lanes = rig.shell.layout().lanes.len();
    assert!((8..=9).contains(&lanes), "{lanes} lanes drawn for twelve towers");
    // Control: a scene inside the bounds is drawn whole and says nothing.
    let mut whole = staggered_rig();
    whole.shell.run("show flight").unwrap();
    whole.frames(vec![vec![], vec![]]);
    let drawn = whole.shell.layout().plots[0].clone();
    assert!(drawn.note.is_none() && whole.shell.layout().lanes.len() == 5);
}

#[test]
fn the_last_tick_label_fits() {
    // S1's second review: a timeline of 2000 s ended in a mark labelled "200", the last digit
    // cut off by the panel's edge.
    for (scene, last) in [("comb", "2000"), ("pass", "12"), ("oscillator-zoo", "2000")] {
        let mut rig = Rig::new([1440, 900]);
        rig.shell.run(&format!("load {scene}")).unwrap();
        rig.frames(vec![vec![], vec![]]);
        let (ruler, within) = rig.shell.layout().ruler.clone();
        assert!(ruler.len() >= 3, "{scene}: {ruler:?}");
        assert_eq!(ruler.last().unwrap().0, last, "{scene}: the last mark is the end of the run");
        // Every number is whole inside the rectangle the timeline is cut to …
        for (text, took) in &ruler {
            assert!(within.contains_rect(*took), "{scene}: `{text}` at {took:?} leaves {within:?}");
            assert!(took.width() > 4.0 * text.len() as f32, "{scene}: `{text}` is {} wide", took.width());
        }
        // … and no two of them touch.
        for pair in ruler.windows(2) {
            assert!(
                pair[0].1.right() < pair[1].1.left(),
                "{scene}: `{}` runs into `{}`",
                pair[0].0,
                pair[1].0
            );
        }
    }
    // Control: the number is as wide as its digits, so a label left where S1 put it (starting
    // two points right of the last mark) would not fit: the mark is 18 points from the edge.
    let mut rig = Rig::new([1440, 900]);
    rig.shell.run("load comb").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let (ruler, within) = rig.shell.layout().ruler.clone();
    let (_, took) = ruler.last().unwrap();
    let mark = took.right() + 2.0;
    assert!((within.right() - mark - 18.0).abs() < 0.5, "the last mark is at {mark}");
    assert!(mark + 2.0 + took.width() > within.right(), "to the right of its mark it would be cut off");
}

#[test]
fn nothing_on_the_figure_covers_a_plot_s_title() {
    // S1's review held the Home button clear of a plot's title. S20 (the owner's ruling) took
    // Home off the figure: a scene's canvas has the navigation gizmo in its corner, a plot's has
    // nothing over it, and each viewport's header is above its canvas, never on it.
    use cav_engine::PaneContent;
    let gizmo = |rig: &Rig, pane: usize| rig.shell.layout().button(&format!("gizmo {}", pane + 1));
    let mut rig = Rig::new([1440, 900]);
    rig.shell.run("load oscillator-zoo").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let before = gizmo(&rig, 0).expect("with the scene shown, the gizmo is in its corner");
    assert_eq!(rig.shell.layout().button("view home"), None, "no Home button on the figure");
    rig.shell.run("show periodogram").unwrap();
    assert!(rig.shell.engine.settle(std::time::Duration::from_secs(60)));
    rig.frames(vec![vec![], vec![]]);
    assert_eq!(gizmo(&rig, 0), None, "a plot has no camera and no gizmo");
    let layout = rig.shell.layout().clone();
    let (text, took) = layout.plots[0].labels[0].clone();
    assert!(text.starts_with("Periodogram ·"), "{text}");
    // Control: where the gizmo stood, it would have been on the title's line.
    assert!(before.y_range().intersects(took.y_range()), "{before:?} against {took:?}");
    // The header is above the canvas, and every word of the plot is in the canvas.
    let (_, header) = layout.headers[0];
    assert!((header.bottom() - layout.canvases[0].top()).abs() < 0.01);
    for (text, took) in &layout.plots[0].labels {
        assert!(layout.canvases[0].expand(0.5).contains_rect(*took), "`{text}` leaves the canvas");
        assert!(!took.intersects(header), "`{text}` is under the header");
    }
    // Two viewports, the plot on the right: the gizmo is in the scene's canvas only.
    rig.shell.run("show scene").unwrap();
    rig.shell.run("panes 2").unwrap();
    rig.shell.run("show signal @2").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let layout = rig.shell.layout().clone();
    let button = gizmo(&rig, 0).expect("a pane shows the scene");
    assert!(layout.canvases[0].contains_rect(button), "{button:?} in {:?}", layout.canvases[0]);
    assert_eq!(gizmo(&rig, 1), None);
    for drawn in &layout.plots {
        assert_eq!(drawn.content, PaneContent::Signal);
        for (text, took) in &drawn.labels {
            assert!(!took.intersects(button), "the gizmo is over `{text}`");
        }
    }
    // The Home key still resets the camera of the active pane.
    rig.shell.run("orbit 10 80").unwrap();
    rig.frames(vec![vec![]]);
    rig.key(Key::Home, Modifiers::NONE);
    assert_eq!(rig.shell.engine.view(0).named(), Some(cav_sim::NamedView::Iso));
}

#[test]
fn a_scene_with_no_towers_draws_every_pane_and_the_timeline() {
    let mut rig = Rig::new([1280, 720]);
    rig.shell.run("load no-towers").unwrap();
    rig.shell.run("panes 4").unwrap();
    rig.shell.run("show signal @1").unwrap();
    rig.shell.run("show periodogram @2").unwrap();
    rig.shell.run("show cadence @3").unwrap();
    rig.shell.run("show flight @4").unwrap();
    assert!(rig.shell.engine.settle(std::time::Duration::from_secs(60)));
    rig.frames(vec![vec![], vec![]]);
    let layout = rig.shell.layout().clone();
    assert_eq!(layout.plots.len(), 4, "every plot pane is drawn");
    assert!(layout.lanes.is_empty(), "no lanes");
    assert!(layout.timeline_caption.contains("no towers"), "{}", layout.timeline_caption);
    // Back to the scene, with the selection and the menus: nothing to select, and the menus' tower
    // items are not offered.
    rig.shell.run("show scene @1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert!(rig.shell.run("select tower 1").unwrap_err().contains("the scene has no towers"));
    let table = cav_app::actions::table();
    let towers: Vec<&cav_app::actions::Action> =
        table.actions.iter().filter(|a| a.line.starts_with("select tower")).collect();
    assert!(!towers.is_empty() && towers.iter().all(|a| !rig.shell.can_run(a)));
    // Control: with five towers the first five are, and one past the fifth is not.
    rig.shell.run("load room").unwrap();
    let can: Vec<bool> = towers.iter().map(|a| rig.shell.can_run(a)).collect();
    assert!(
        can.iter().filter(|c| **c).count() >= 5
            && towers.iter().any(|a| a.line == "select tower 6" && !rig.shell.can_run(a))
    );
    // (TASK-S20-06: this was `rewind`, which could always run; the transport's `seek-start`
    // that replaced it cannot at the start, so the action that needs nothing is `play`.)
    assert!(rig.shell.can_run(table.action("play").unwrap()));
    // The last tower of a scene can be taken away, and the application goes on.
    rig.shell.run("load point").unwrap();
    for _ in 0..5 {
        rig.shell.run("remove array.towers[0]").unwrap();
    }
    assert!(rig.shell.engine.spec().array.towers.is_empty());
    assert!(rig.shell.engine.settle(std::time::Duration::from_secs(60)));
    rig.frames(vec![vec![], vec![]]);
    assert!(rig.shell.layout().lanes.is_empty());
}

#[test]
fn a_mixed_array_has_lanes_of_each_tower_s_own_shots() {
    let mut rig = Rig::new([1440, 900]);
    rig.shell.run("load mixed-array").unwrap();
    rig.shell.run("set duration 20").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let lanes = rig.shell.layout().lanes.clone();
    assert_eq!(lanes.len(), 3);
    // Each bar is its own tower's shot: 2T of 1.46 s, 5 s and 1.8 s, on one time axis.
    let per_second = lanes[0].1.width() / 1.46;
    for (n, flight) in [1.46, 5.0, 1.8].into_iter().enumerate() {
        assert!((lanes[n].1.width() - flight * per_second).abs() <= 1.0, "tower {}: {:?}", n + 1, lanes[n].1);
    }
    let caption = rig.shell.layout().timeline_caption.clone();
    assert!(
        caption.contains("2T = 1.46 to 5.00 s") && caption.contains("4 to 10 shots a tower"),
        "{caption}"
    );
}
