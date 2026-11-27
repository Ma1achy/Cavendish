//! S20's editor tests (`docs/slices/S20.md` §4), each driven through the same input events a
//! window delivers, each with a control arm. Rows N1 to N5 are the density pass's (TASK-S20-12):
//! the sizes are the theme's (`docs/design/workspace.md` §0), a field fills its column, the
//! scroll bar's width is reserved, the chrome holds no sentences, a uniform body's material
//! reads "Uniform" and the Probe reads into the right panel. Rows M3 to M6 and K1 are the mosaic's
//! (TASK-S20-03): more than four viewports, the splitters, the labels' order, pop-out and dock.
//! Rows T1 to T3 are the transport's (TASK-S20-06): each button its command, a step on a shot's
//! readout, the speed; with them the top bar's row (the design's §4). Rows T4 and T5 are
//! TASK-S20-09's: the duration typed at the transport, and what playing does at the end. Rows C3
//! and C4 are the first-person camera's (TASK-S20-10): the keys of a flight, and the pointer's
//! capture.

use std::collections::BTreeMap;

use super::Rig;
use cav_app::headless::context;
use cav_app::{Shell, theme};
use cav_engine::{Boundary, Command, Target, ViewCommand};
use egui::{Event, Key, Modifiers, PointerButton, Rect, vec2};

/// The greatest pitch (top to top, points) of two rows that follow one another in a card: of
/// `rows` (each a row's rectangle) taken in order down the panel, every pair with no card
/// `headers` between them. The second number is how many pairs were measured.
fn greatest_pitch(rows: &[Rect], headers: &[Rect]) -> (f32, usize) {
    let mut tops: Vec<f32> = rows.iter().map(Rect::top).collect();
    tops.sort_by(f32::total_cmp);
    tops.dedup_by(|a, b| (*a - *b).abs() < 0.5);
    let mut greatest: f32 = 0.0;
    let mut pairs = 0;
    for pair in tops.windows(2) {
        if headers.iter().any(|h| h.top() > pair[0] && h.top() < pair[1]) {
            continue;
        }
        greatest = greatest.max(pair[1] - pair[0]);
        pairs += 1;
    }
    (greatest, pairs)
}

/// The inspector's one-row editors and its cards' headers, as the last frame laid them out. The
/// scene's description is left out (it is a box of lines, not a row), and a list of parts' own
/// editor (the "+ Add part" button, which stands a step of the grid below the last part).
fn inspector_rows(shell: &Shell) -> (Vec<Rect>, Vec<Rect>) {
    let layout = shell.layout();
    // A frame may be laid out twice: the last rectangle of each is the one drawn.
    let last = |list: &[(String, Rect)], keep: &dyn Fn(&str) -> bool| -> Vec<Rect> {
        let map: BTreeMap<&str, Rect> =
            list.iter().filter(|(name, _)| keep(name)).map(|(name, rect)| (name.as_str(), *rect)).collect();
        map.into_values().collect()
    };
    let rows = last(&layout.editors, &|path| {
        path != "description" && path != "name" && !path.ends_with(".label") && !path.ends_with(".parts")
    });
    let headers = last(&layout.buttons, &|name| name.starts_with("fold "));
    (rows, headers)
}

/// One frame of `add` on `ctx` with no window, `size` points: for drawing a panel's own parts
/// with sizes other than the theme's (the controls).
pub(super) fn bare_frame(ctx: &egui::Context, size: egui::Vec2, add: impl FnMut(&mut egui::Ui)) {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, add);
    // Nothing is painted here, so the font atlas has nowhere to go.
    output.textures_delta.clear();
}

/// A body's editors drawn straight into a panel 320 points wide with rows `gap` apart and
/// controls `control` a side, with no window round them: what the theme's sizes are compared
/// with. Returns the shell, whose layout holds what was drawn.
fn drawn_with(gap: f32, control: f32) -> Shell {
    let mut shell = Shell::new();
    shell.run("load point").unwrap();
    bare_frame(&context(), vec2(320.0, 900.0), |ui| {
        ui.spacing_mut().item_spacing = vec2(theme::GRID, gap);
        ui.spacing_mut().interact_size = vec2(control, control);
        cav_app::editors::show(&mut shell, ui, "inspector", "bodies[0]");
    });
    shell
}

#[test]
fn sizes_are_the_dense_design_s() {
    // N1. Criterion (written before measuring): `theme::HIT` = 20, `ICON` in 20..=22, `GRID` =
    // 4, text 11 to 12 pt; every registered target of the tree and the inspector at least
    // `HIT − 0.5` both ways; every row's pitch at most 24 pt. Controls: rows drawn 8 pt apart
    // (S19's 28 pt pitch) fail the pitch; the tool strip's icons are larger than a row.
    assert_eq!(theme::HIT, 20.0);
    assert!((20.0..=22.0).contains(&theme::ICON), "{}", theme::ICON);
    assert_eq!(theme::GRID, 4.0);
    let style = context().global_style();
    for text in
        [egui::TextStyle::Body, egui::TextStyle::Button, egui::TextStyle::Small, egui::TextStyle::Monospace]
    {
        let size = style.text_styles[&text].size;
        assert!((11.0..=12.0).contains(&size), "{text:?} is {size} pt");
    }
    assert_eq!(style.spacing.item_spacing, vec2(theme::GRID, theme::GRID), "items on the grid");
    let least = theme::HIT - 0.5;
    let mut measured = 0;
    for selection in super::s19::SELECTIONS {
        let (rig, targets) = super::s19::side_panel_targets(selection);
        assert!(targets.len() >= 12, "{selection}: {} targets", targets.len());
        for (id, rect) in &targets {
            assert!(
                rect.width() >= least && rect.height() >= least,
                "{selection}: `{id}` is {:.1} × {:.1}",
                rect.width(),
                rect.height()
            );
        }
        // The tree: every row (the scene's, the groups', the items') a target high, touching.
        let tree = rig.shell.focus_tree();
        let rows: Vec<Rect> = rig
            .shell
            .layout()
            .row_texts
            .iter()
            .filter_map(|(id, _)| tree.node(id).and_then(|n| n.rect))
            .collect();
        for row in &rows {
            assert!((row.height() - theme::HIT).abs() < 0.01, "{selection}: a tree row is {}", row.height());
        }
        let (pitch, pairs) = greatest_pitch(&rows, &[]);
        assert!(pairs >= 8, "{selection}: {pairs} pairs of tree rows");
        assert!((20.0..=22.0).contains(&pitch), "{selection}: the tree's pitch is {pitch}");
        // The inspector: rows in a card.
        let (rows, headers) = inspector_rows(&rig.shell);
        let (pitch, pairs) = greatest_pitch(&rows, &headers);
        measured += pairs;
        assert!(pitch <= 24.0, "{selection}: the inspector's pitch is {pitch}");
        for header in &headers {
            assert!((header.height() - theme::HEADER).abs() < 0.01, "{selection}: a header is {header:?}");
        }
        // And no row runs past its card: every card is one width, every row within it.
        let right = headers.iter().map(Rect::right).fold(f32::NEG_INFINITY, f32::max);
        for edge in headers.iter().chain(&rows).map(Rect::right) {
            assert!(
                edge <= right + 0.01 && right - edge < 0.5 + rows[0].width(),
                "{selection}: {edge} past {right}"
            );
        }
        for header in &headers {
            assert!((header.right() - right).abs() < 0.01, "{selection}: cards of two widths: {headers:?}");
        }
        // Control: the tool palette's icon buttons are larger than a row.
        let tool = rig.shell.layout().button("tool select").expect("the Select tool's button");
        assert!(tool.height() >= theme::ICON && tool.height() > theme::HIT, "{tool:?}");
    }
    println!("{measured} pairs of inspector rows");
    assert!(measured >= 12, "{measured} pairs of inspector rows were measured");
    // The same rows drawn with no window: at the theme's gap they are at the pitch the window
    // draws them at; 8 pt apart (S19's 28 pt pitch) the same measure fails the criterion.
    let pitch_at = |gap: f32| {
        let shell = drawn_with(gap, theme::HIT);
        let (rows, headers) = inspector_rows(&shell);
        let (pitch, pairs) = greatest_pitch(&rows, &headers);
        assert!(pairs >= 2, "{pairs} pairs");
        pitch
    };
    assert!((pitch_at(theme::ROW_GAP) - (theme::HIT + theme::ROW_GAP)).abs() < 0.01);
    let old = pitch_at(8.0);
    assert!((old - 28.0).abs() < 0.01 && old > 24.0, "control: S19's pitch is {old}");
}

#[test]
fn a_field_takes_its_column_s_width() {
    // N2. Criterion: the Description box's width is its card's inner width within 1 pt; every
    // text field and drop-down of a card ends where its column does (the reset's place, a target
    // and a gap from the card's inner edge) within 1 pt. Control: the header's name field, which
    // shares its row with the kind's words and the menu, is not as wide as its row by the same
    // measure (as S19's 120 pt Description was not).
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("layout instrument").unwrap();
    rig.frames(vec![vec![], vec![], vec![]]);
    let layout = rig.shell.layout();
    let card = layout.button("fold inspector/selection").expect("the Scene card's header");
    let description = layout.editor("description").expect("the Description box");
    assert!(
        (description.width() - card.width()).abs() <= 1.0,
        "the box is {} wide in a card {} wide",
        description.width(),
        card.width()
    );
    assert!((description.left() - card.left()).abs() <= 1.0 && description.height() >= 3.0 * 12.0);
    assert!(description.width() > 250.0, "and the card is the panel's: {}", description.width());
    let column_end = card.right() - theme::HIT - theme::ROW_GAP;
    let tree = rig.shell.focus_tree();
    let mut measured = 0;
    let mut stack = vec!["inspector".to_owned()];
    while let Some(id) = stack.pop() {
        stack.extend(tree.children(&id).into_iter().map(str::to_owned));
        let Some(node) = tree.node(&id) else { continue };
        let field =
            matches!(node.kind, cav_app::keyboard::Kind::Text | cav_app::keyboard::Kind::Choice { .. });
        let Some(rect) = node.rect.filter(|_| field && id.starts_with("field/") && id != "field/name") else {
            continue;
        };
        if id == "field/description" {
            continue;
        }
        assert!(
            (rect.right() - column_end).abs() <= 1.0,
            "`{id}` ends at {}, not {column_end}",
            rect.right()
        );
        measured += 1;
    }
    assert!(measured >= 1, "a drop-down was measured");
    let name = layout.editor("name").expect("the header's name field");
    assert!(card.width() - name.width() > 100.0, "control: the name field is {} wide", name.width());
}

#[test]
fn the_scroll_bar_s_width_is_reserved() {
    // N3 (S19's second review, minor 10). Criterion: a card is as wide in a window short enough
    // that the inspector scrolls as in one tall enough that it does not, to 0.5 pt. Control: the
    // same scroll area without the reserve is wider by the bar's width when there is no bar.
    let card = |height: u32| {
        let mut rig = Rig::new();
        rig.size = [1440, height];
        rig.shell.run("load point").unwrap();
        rig.shell.run("layout instrument").unwrap();
        rig.frames(vec![vec![], vec![], vec![]]);
        let layout = rig.shell.layout();
        let header = layout.button("fold inspector/array").expect("the Array card");
        (header.width(), layout.button("inspector scroll bar").is_some())
    };
    let (short, short_bar) = card(500);
    let (tall, tall_bar) = card(1400);
    assert!(short_bar && !tall_bar, "the short window scrolls, the tall one does not");
    assert!((short - tall).abs() <= 0.5, "a card is {short} wide with the bar and {tall} without");
    let width = |reserve: bool, rows: usize| {
        let ctx = context();
        let mut width = 0.0;
        // Frames enough for the bar to come in whole (egui eases it in over a few).
        for _ in 0..12 {
            bare_frame(&ctx, vec2(300.0, 200.0), |ui| {
                cav_app::panels_for_tests::scrolling(ui, "test", reserve, |ui| {
                    width = ui.available_width();
                    for _ in 0..rows {
                        ui.label("row");
                    }
                });
            });
        }
        width
    };
    assert!((width(true, 2) - width(true, 60)).abs() <= 0.5, "reserved: one width");
    let (fits, scrolls) = (width(false, 2), width(false, 60));
    assert!((fits - scrolls - 10.0).abs() <= 0.5, "control: {fits} without the bar, {scrolls} with it");
    assert!((width(true, 2) - scrolls).abs() <= 0.5, "the reserve is the bar's width");
}

#[test]
fn the_chrome_holds_no_sentences() {
    // N4. Criterion: no text painted in the tool's options row, the timeline's head or the tree
    // is longer than 32 characters, with each tool in hand, a transform session under way and an
    // empty scene; the hints are tooltips. Control: each hint that S19 painted along the row is
    // recorded as a tooltip and is longer than 32, so the measure would catch it painted.
    // (TASK-S20-05: the options row is shown only while it applies, so it is measured with
    // Move or Place in hand and in a session, and with Select or Probe it must paint nothing.)
    const MOST: usize = 32;
    let scripts = [
        "load point; tool select",
        "load point; select body 1; tool move",
        "load point; tool place",
        "load point; tool probe",
        "load void",
        "load world-box",
    ];
    for script in scripts {
        let mut rig = Rig::new();
        for line in script.split("; ") {
            rig.shell.run(line).unwrap();
        }
        rig.frames(vec![vec![], vec![], vec![]]);
        let check = |rig: &Rig, what: &str| {
            let layout = rig.shell.layout();
            let painted: Vec<&String> = layout
                .chrome
                .iter()
                .filter(|(place, _)| !place.ends_with("tooltip"))
                .map(|(_, text)| text)
                .chain(layout.row_texts.iter().map(|(_, text)| text))
                .collect();
            assert!(painted.len() >= 8, "{script}{what}: {painted:?}");
            for text in painted {
                assert!(text.chars().count() <= MOST, "{script}{what}: `{text}` is painted");
            }
            let has_row = rig.shell.session.is_some() || rig.shell.behaviour().has_options();
            if !has_row {
                assert!(!layout.chrome.iter().any(|(p, _)| p.starts_with("options")), "{script}{what}");
            }
            for place in ["options", "timeline"].into_iter().filter(|p| has_row || *p != "options") {
                assert!(
                    layout.chrome.iter().any(|(p, _)| *p == place),
                    "{script}{what}: {place} is measured"
                );
                let tooltip = format!("{place} tooltip");
                let hint = layout.chrome.iter().find(|(p, _)| *p == tooltip).map(|(_, t)| t.clone());
                let hint = hint.unwrap_or_else(|| panic!("{script}{what}: no {tooltip}"));
                assert!(hint.chars().count() > MOST, "control: `{hint}` would fail painted");
            }
        };
        check(&rig, "");
        if script.ends_with("tool move") {
            // A transform session, an axis locked and a distance typed: its numbers are data.
            rig.shell.keyboard.focus = vec!["viewport".into(), "viewport/selection".into()];
            rig.shell.keyboard.nav_on = true;
            rig.key(egui::Key::Enter, egui::Modifiers::NONE);
            rig.key(egui::Key::ArrowRight, egui::Modifiers::NONE);
            rig.key(egui::Key::X, egui::Modifiers::NONE);
            rig.typed("1.5");
            assert!(rig.shell.session.is_some(), "a session is under way");
            rig.frames(vec![vec![]]);
            check(&rig, " (session)");
            let options: Vec<&str> = rig
                .shell
                .layout()
                .chrome
                .iter()
                .filter(|(p, _)| *p == "options")
                .map(|(_, t)| t.as_str())
                .collect();
            assert!(options.iter().any(|t| t.starts_with('Δ')), "the session's move is said: {options:?}");
        }
    }
    // What an empty scene means is the Bodies group's tooltip, not a line of the tree.
    let mut rig = Rig::new();
    rig.shell.run("load void").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let layout = rig.shell.layout();
    assert!(layout.chrome.iter().any(|(p, t)| *p == "tree" && t == "Add a mass"), "the ways to begin stay");
    assert!(!layout.chrome.iter().any(|(_, t)| t.contains("to read yet")));
}

#[test]
fn a_uniform_body_reads_uniform() {
    // N5 (S19's second review, minor 11): a uniform voxel body's Material drop-down reads
    // "Uniform" (it read "none: uniform, of …", cut). Control: a body of steel reads "steel".
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.add_starter("cube").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let shown = |rig: &Rig| {
        let choices = &rig.shell.layout().choices;
        choices.iter().rev().find(|(path, _)| path == "bodies[1].shape.material").map(|(_, t)| t.clone())
    };
    assert_eq!(shown(&rig).as_deref(), Some("Uniform"));
    rig.shell.run("set bodies[1].shape.material steel").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert_eq!(shown(&rig).as_deref(), Some("steel"), "control");
}

#[test]
fn the_probe_reads_into_the_right_panel() {
    // N5 (S19's second review, minor 13). Criterion: after a probe the figure holds no card (the
    // only thing registered over it is the tool strip and Home), the right panel's Probe tab is
    // forward, and what it drew is the engine's reading of the point, bit for bit. Control: with
    // the Probe in hand and nothing probed the tab has no rows; `tab probe` brings it forward.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("add sphere at 2 2 1").unwrap();
    rig.shell.run("tool probe").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert!(rig.shell.layout().button("tab Probe").is_some(), "the tab is there");
    assert_eq!(rig.shell.right_tab, cav_app::outliner::RightTab::Inspector);
    rig.shell.run("tab probe").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert_eq!(rig.shell.right_tab, cav_app::outliner::RightTab::Probe, "`tab probe`");
    assert!(rig.shell.layout().probe.is_none() && rig.shell.layout().probe_rows.is_empty(), "control");
    rig.shell.run("tab inspector").unwrap();
    rig.frames(vec![vec![]]);
    let pane = rig.shell.layout().canvases[0];
    let at = rig.shell.body_point(0, 1, pane.size()).expect("the sphere is in view") + pane.min.to_vec2();
    rig.click(at);
    rig.frames(vec![vec![], vec![]]);
    assert_eq!(rig.shell.right_tab, cav_app::outliner::RightTab::Probe, "the tab came forward");
    let layout = rig.shell.layout();
    let drawn = layout.probe.clone().expect("the tab drew a reading");
    let asked = rig.shell.engine.probe(drawn.point);
    assert_eq!(drawn, asked, "the engine's reading of that point");
    assert_eq!(drawn.g.map(f64::to_bits), asked.g.map(f64::to_bits));
    assert!(drawn.cell.is_some() && drawn.towers.len() == 5);
    for (tower, other) in drawn.towers.iter().zip(&asked.towers) {
        assert_eq!(tower.distance.to_bits(), other.distance.to_bits(), "{}", tower.label);
        assert_eq!(tower.phase.map(f64::to_bits), other.phase.map(f64::to_bits), "{}", tower.label);
        assert!(tower.phase.is_some_and(|p| p != 0.0));
    }
    let row = |name: &str| layout.probe_rows.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str());
    assert_eq!(row("Cell of"), Some("sphere"));
    assert!(row("Gravity").is_some_and(|g| g.ends_with("m/s²")), "{:?}", layout.probe_rows);
    // Nothing of the reading is over the figure: no card, and nothing registered there but the
    // tool strip, its buttons and Home.
    assert!(layout.button("probe card").is_none());
    let over: Vec<&str> = layout
        .buttons
        .iter()
        .filter(|(_, rect)| layout.figure.contains_rect(*rect) && rect.is_positive())
        .map(|(name, _)| name.as_str())
        // (S20's header and gizmo are over the figure by design; Home is gone.)
        .filter(|name| !name.starts_with("tool ") && !over_by_design(name))
        .collect();
    assert!(over.is_empty(), "over the figure: {over:?}");
}

// ---- The mosaic (TASK-S20-03): rows M3 to M6 and K1 ----

/// Whether a registered button is one of a viewport's own (its header's controls, its
/// navigation gizmo), which stand over the figure by design.
fn over_by_design(name: &str) -> bool {
    let line = ["style ", "proj toggle", "span all"].iter().any(|start| name.starts_with(start));
    line || ["header ", "viewport ", "gizmo "].iter().any(|start| name.starts_with(start))
}

/// How much of `a` lies inside `b`, points squared.
fn overlap(a: Rect, b: Rect) -> f32 {
    let both = a.intersect(b);
    if both.is_positive() { both.area() } else { 0.0 }
}

/// A rig on `point` with the figure arranged as `mosaic`, two frames drawn.
fn mosaic_rig(mosaic: &str) -> Rig {
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run(&format!("mosaic \"{mosaic}\"")).unwrap();
    rig.frames(vec![vec![], vec![]]);
    rig
}

/// The handle of the splitter that moves `boundary`.
fn splitter(rig: &Rig, boundary: Boundary) -> Rect {
    let found = rig.shell.layout().splitters.iter().find(|(b, _)| *b == boundary);
    found.unwrap_or_else(|| panic!("no splitter {boundary:?}")).1
}

#[test]
fn six_viewports_draw_and_the_thirteenth_is_refused() {
    // M3. Criterion (written before measuring): `mosaic "ABC;DEF"` lays out six pane rectangles,
    // disjoint, inside the figure and covering it but for the splitters' gaps, and each draws
    // what it shows (a plot in the fifth and the sixth is drawn inside its rectangle); a 13th
    // viewport is refused with "12" in the message, nothing changed. Control: the same measure
    // counts four with `panes 4`.
    let mut rig = mosaic_rig("ABC;DEF");
    rig.shell.run("show signal @5").unwrap();
    rig.shell.run("show flight @6").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let layout = rig.shell.layout().clone();
    assert_eq!((layout.panes.len(), layout.slots.len()), (6, 6));
    assert_eq!(layout.panes, layout.slots, "none floats: each is drawn in its place");
    let mut total = 0.0;
    for (n, pane) in layout.panes.iter().enumerate() {
        assert!(layout.figure.expand(0.01).contains_rect(*pane), "viewport {} leaves the figure", n + 1);
        assert!(pane.width() >= 200.0 && pane.height() >= 150.0, "viewport {} is {pane:?}", n + 1);
        for other in &layout.panes[n + 1..] {
            assert!(overlap(*pane, *other) < 0.01, "viewports overlap");
        }
        total += pane.area();
    }
    // Three columns and two rows: two gaps across and one down, each 2 px.
    let gap = 2.0;
    let expected = (layout.figure.width() - 2.0 * gap) * (layout.figure.height() - gap);
    assert!((total - expected).abs() <= 1.0, "the six cover {total}; the figure less the gaps is {expected}");
    // Reading order of `ABC;DEF`: the fourth is under the first.
    assert!((layout.panes[3].left() - layout.panes[0].left()).abs() < 0.01);
    assert!(layout.panes[3].top() > layout.panes[0].bottom());
    // The fifth and sixth drew their plots, inside their own rectangles.
    for pane in [4, 5] {
        let drawn: Vec<_> = layout.plots.iter().filter(|d| d.pane == pane).collect();
        assert!(!drawn.is_empty(), "viewport {} drew no plot", pane + 1);
        assert!(drawn.iter().all(|d| layout.panes[pane].expand(0.5).contains_rect(d.area)));
    }
    assert!(rig.shell.engine.frame(5).is_ok());
    assert_eq!(layout.splitters.len(), 3 + 4, "three stretches of the row's gap, two of each column's");
    // The window's own drawing of six viewports with no GPU surface (recorded, not gated: §6).
    let started = std::time::Instant::now();
    let frames = 30;
    rig.frames(vec![vec![]; frames]);
    println!(
        "six viewports, no surface: {:.2} ms a frame",
        started.elapsed().as_secs_f64() * 1e3 / frames as f64
    );

    // Twelve are held and drawn; the thirteenth is refused, by a split and by a mosaic.
    rig.shell.run("mosaic \"ABCD;EFGH;IJKL\"").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert_eq!(rig.shell.layout().panes.len(), 12);
    let before = (rig.shell.engine.digest(), rig.shell.engine.journal().len());
    for line in ["split right", "split down @7", "mosaic \"ABCD;EFGH;IJKL;MMMM\""] {
        let refused = rig.shell.run(line).unwrap_err();
        assert!(refused.contains("12"), "`{line}`: {refused}");
    }
    assert_eq!(before, (rig.shell.engine.digest(), rig.shell.engine.journal().len()));
    rig.frames(vec![vec![], vec![]]);
    assert_eq!(rig.shell.layout().panes.len(), 12);
    // Control: the count is the mosaic's, not a constant of the measure.
    rig.shell.run("panes 4").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert_eq!(rig.shell.layout().panes.len(), 4);
}

#[test]
fn a_splitter_drag_keeps_the_sum_and_is_one_line() {
    // M4. Criterion: a drag of the splitter between two columns changes those two weights and
    // keeps their sum to 1e-12; the boundary ends under the pointer within 1 pt; the journal
    // grows by one line, issued when the drag ends (the layout follows the pointer before
    // that, in the window alone); a double click evens the two sides. Controls: a drag that
    // starts in a viewport, away from any splitter, leaves the mosaic as it was; a drag past
    // the far side stops at the least width.
    let mut rig = mosaic_rig("ABC;DEF");
    let weights = |rig: &Rig| rig.shell.engine.mosaic().column_weights().to_vec();
    let start = weights(&rig);
    let rows = rig.shell.engine.mosaic().row_weights().to_vec();
    let handle = splitter(&rig, Boundary::Column(0));
    assert!((handle.width() - theme::GRID).abs() < 0.01, "the handle is the grid wide: {handle:?}");
    let from = handle.center();
    let to = from + vec2(137.0, 9.0);
    let journal = rig.shell.engine.journal().len();
    // Pressed and moved, not yet let go: the layout has followed, the engine has heard nothing.
    rig.frames(vec![
        vec![Event::PointerMoved(from)],
        vec![Rig::press(from, PointerButton::Primary, true)],
        vec![Event::PointerMoved(from + vec2(60.0, 0.0))],
        vec![Event::PointerMoved(to)],
        vec![],
    ]);
    assert_eq!((weights(&rig), rig.shell.engine.journal().len()), (start.clone(), journal));
    let following = splitter(&rig, Boundary::Column(0));
    assert!((following.center().x - to.x).abs() <= 1.0, "mid-drag the handle is at {following:?}");
    rig.frames(vec![vec![Rig::press(to, PointerButton::Primary, false)], vec![], vec![]]);
    let after = weights(&rig);
    assert_eq!(rig.shell.engine.journal().len(), journal + 1, "one journal line a drag");
    let last = rig.shell.engine.journal().last().unwrap();
    assert!(matches!(last, Command::View(ViewCommand::Mosaic { from: None, .. })), "{last:?}");
    assert!(((after[0] + after[1]) - (start[0] + start[1])).abs() <= 1e-12, "{start:?} → {after:?}");
    assert!(after[0] > start[0] && after[1] < start[1], "the left grew, the right shrank");
    assert_eq!((after[2], rig.shell.engine.mosaic().row_weights()), (start[2], rows.as_slice()));
    let moved = splitter(&rig, Boundary::Column(0));
    assert!((moved.center().x - to.x).abs() <= 1.0, "the boundary is at {moved:?}, the pointer at {to:?}");
    let panes = rig.shell.layout().panes.clone();
    assert!((panes[0].right() - (to.x - 1.0)).abs() <= 1.0 && (panes[1].left() - (to.x + 1.0)).abs() <= 1.0);
    // Nothing of it is an edit.
    assert_eq!(rig.shell.engine.depth(), (1, 0));

    // A double click evens the two sides: one more line.
    let at = moved.center();
    rig.frames(vec![
        vec![Event::PointerMoved(at)],
        vec![Rig::press(at, PointerButton::Primary, true)],
        vec![Rig::press(at, PointerButton::Primary, false)],
        vec![Rig::press(at, PointerButton::Primary, true)],
        vec![Rig::press(at, PointerButton::Primary, false)],
        vec![],
    ]);
    let evened = weights(&rig);
    assert_eq!(rig.shell.engine.journal().len(), journal + 2);
    assert_eq!(evened[0].to_bits(), evened[1].to_bits(), "{evened:?}");
    assert!(((evened[0] + evened[1]) - (start[0] + start[1])).abs() <= 1e-12);

    // Control: a drag of the same length that starts inside a viewport moves no boundary (it
    // orbits that viewport's camera, which is its own journal line).
    let mosaic = rig.shell.engine.mosaic().clone();
    let inside = rig.shell.layout().panes[0].center();
    rig.drag(inside, inside + vec2(137.0, 9.0));
    assert_eq!(rig.shell.engine.mosaic(), &mosaic);
    assert!(matches!(rig.shell.engine.journal().last(), Some(Command::View(ViewCommand::Orbit { .. }))));
    // Control: dragged past everything, the column stops at the least width and no further.
    let handle = splitter(&rig, Boundary::Column(1)).center();
    rig.drag(handle, handle + vec2(3000.0, 0.0));
    let squeezed = rig.shell.layout().panes[2];
    assert!((squeezed.width() - cav_app::MIN_VIEWPORT).abs() <= 1.0, "the third column is {squeezed:?}");
    let last = weights(&rig);
    assert!((last.iter().sum::<f64>() - start.iter().sum::<f64>()).abs() <= 1e-12);
    // The same arithmetic with no window: the gap's middle put at 301 of 802 px of two rows.
    let two = cav_engine::Mosaic::parse("A;B").unwrap();
    let pure = cav_app::splitter_moved(&two, Boundary::Row(0), 301.0, 802, 2, 80.0);
    assert_eq!(pure.row_weights(), [0.75, 1.25], "{pure}");
}

/// The mosaic's stops as the arrows walk them from the first: each stop's id, its name and
/// where it is, in the order visited (one more arrow than there are stops, to show the end).
fn walked(rig: &mut Rig) -> Vec<(String, String, Rect)> {
    rig.shell.keyboard.focus = vec!["viewport".into(), "viewport/mosaic".into()];
    rig.shell.keyboard.nav_on = true;
    rig.key(Key::Enter, Modifiers::NONE);
    let mut stops: Vec<(String, String, Rect)> = Vec::new();
    for _ in 0..=rig.shell.engine.panes() {
        let id = rig.shell.keyboard.focused().to_owned();
        let node = rig.shell.focus_tree().node(&id).expect("the stop is registered").clone();
        if stops.last().is_none_or(|(last, ..)| *last != id) {
            stops.push((id, node.name, node.rect.expect("it is on screen")));
        }
        rig.key(Key::ArrowRight, Modifiers::NONE);
    }
    stops
}

/// The stops' ids, and the same ids in the order of their places on screen (top to bottom,
/// then left to right): what an order taken from the layout would be.
fn orders(stops: &[(String, String, Rect)]) -> (Vec<String>, Vec<String>) {
    let ids: Vec<String> = stops.iter().map(|(id, ..)| id.clone()).collect();
    let mut placed: Vec<&(String, String, Rect)> = stops.iter().collect();
    placed.sort_by(|a, b| a.2.top().total_cmp(&b.2.top()).then(a.2.left().total_cmp(&b.2.left())));
    (ids, placed.into_iter().map(|(id, ..)| id.clone()).collect())
}

#[test]
fn tab_and_arrows_follow_the_labels_whatever_floats() {
    // M5. Criterion: the arrows walk the viewports in the labels' order (viewport 1, 2, 3, 4:
    // A, B, C, D), each stop where its viewport is drawn; the same after one is popped out and
    // after two have swapped places; Enter on a stop makes it the active viewport. Controls:
    // in the plain grid the order by place on screen is the same order (the measure agrees
    // when it should); after the pop-out, and after the swap, the order by place differs.
    let mut rig = mosaic_rig("AB;CD");
    let by_label: Vec<String> = (1..=4).map(|n| format!("viewport/mosaic/{n}")).collect();
    let names = ["Viewport 1 (A)", "Viewport 2 (B)", "Viewport 3 (C)", "Viewport 4 (D)"];
    let check = |rig: &mut Rig, what: &str| -> Vec<String> {
        let stops = walked(rig);
        let (ids, placed) = orders(&stops);
        assert_eq!(ids, by_label, "{what}: the arrows' order");
        assert_eq!(stops.iter().map(|(_, name, _)| name.as_str()).collect::<Vec<_>>(), names, "{what}");
        for (n, (_, _, rect)) in stops.iter().enumerate() {
            assert_eq!(*rect, rig.shell.layout().panes[n], "{what}: stop {} is where its viewport is", n + 1);
        }
        placed
    };
    assert_eq!(check(&mut rig, "the grid"), by_label, "control: in the plain grid place and label agree");
    // Popped out: viewport 1 floats lower than viewport 2's top, so by place it comes after it.
    let cameras: Vec<_> = (0..4).map(|pane| rig.shell.engine.view(pane).clone()).collect();
    rig.shell.run("viewport pop-out 1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert_eq!(rig.shell.layout().floating.len(), 1);
    let placed = check(&mut rig, "popped out");
    assert_ne!(placed, by_label, "control: an order from the places differs once one floats");
    assert_eq!(placed[0], "viewport/mosaic/2");
    // Tab from inside the mosaic goes on to the next big scope, as from anywhere in the viewport.
    rig.key(Key::Tab, Modifiers::NONE);
    assert_eq!(rig.shell.keyboard.focused(), "inspector");
    // Enter on a stop makes its viewport the active one.
    assert_eq!(rig.shell.engine.active_pane(), 0);
    rig.shell.keyboard.focus = vec!["viewport".into(), "viewport/mosaic".into(), "viewport/mosaic/3".into()];
    rig.key(Key::Enter, Modifiers::NONE);
    assert_eq!(rig.shell.engine.active_pane(), 2);
    // Docked again and two swapped: A and D change places, and keep their numbers and cameras.
    rig.shell.run("viewport dock").unwrap();
    rig.shell.run("swap 1 4").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert_eq!(rig.shell.engine.mosaic().to_string(), "DB;CA");
    let placed = check(&mut rig, "swapped");
    assert_eq!(placed, ["viewport/mosaic/4", "viewport/mosaic/2", "viewport/mosaic/3", "viewport/mosaic/1"]);
    for (pane, camera) in cameras.iter().enumerate() {
        assert_eq!(rig.shell.engine.view(pane), camera, "viewport {} kept its camera", pane + 1);
    }
}

#[test]
fn pop_out_and_dock_return_the_layout() {
    // M6. Criterion: after a viewport is popped out (moved, resized) and docked, the mosaic's
    // text, every viewport's camera (the engine's digest: bit for bit) and every viewport's
    // rectangle are what they were; neither adds a journal line. Control: a split, which is
    // the engine's, does add one and changes the digest.
    let mut rig = mosaic_rig("AAB;CDB");
    rig.shell.run("orbit 33 12 @2").unwrap();
    rig.shell.run("show flight @3").unwrap();
    rig.shell.run("pane 2").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let state = |rig: &Rig| {
        (
            rig.shell.engine.mosaic().to_string(),
            rig.shell.engine.digest(),
            rig.shell.engine.journal().len(),
            (0..4).map(|pane| rig.shell.engine.view(pane).clone()).collect::<Vec<_>>(),
        )
    };
    let (before, panes) = (state(&rig), rig.shell.layout().panes.clone());
    // Popped out from the right-click menu of viewport 2 (the active one: no activation).
    rig.click_with(panes[1].center(), PointerButton::Secondary);
    let item = rig.shell.layout().button("viewport menu viewport-pop-out").expect("the menu's item");
    rig.click(item.center());
    rig.frames(vec![vec![], vec![]]);
    let layout = rig.shell.layout().clone();
    assert_eq!(layout.floating.len(), 1, "viewport 2 floats");
    let (pane, frame) = layout.floating[0];
    assert_eq!(pane, 1);
    assert!(frame.contains_rect(layout.panes[1]) && layout.panes[1] != panes[1], "it is drawn in its frame");
    assert_eq!(layout.slots, panes, "the mosaic keeps every place");
    let place = layout.button("viewport dock 2").expect("its place has the dock button");
    assert!(panes[1].contains_rect(place));
    assert_eq!(state(&rig), before, "popping out changes nothing of the engine's");
    // Dragged by its bar and resized by its corner: the window's state, still no journal line.
    let bar = layout.button("float bar 2").unwrap();
    rig.drag(bar.center(), bar.center() + vec2(-300.0, 150.0));
    let moved = rig.shell.layout().floating[0].1;
    assert!((moved.min - frame.min - vec2(-300.0, 150.0)).length() <= 1.0, "{frame:?} → {moved:?}");
    assert!((moved.size() - frame.size()).length() <= 0.01);
    let corner = rig.shell.layout().button("float corner 2").unwrap();
    rig.drag(corner.center(), corner.center() + vec2(80.0, 40.0));
    let resized = rig.shell.layout().floating[0].1;
    assert!((resized.size() - moved.size() - vec2(80.0, 40.0)).length() <= 1.0, "{moved:?} → {resized:?}");
    assert_eq!(resized.min, moved.min);
    assert_eq!(state(&rig), before);
    // Docked from the button in its empty place.
    let dock = rig.shell.layout().button("viewport dock 2").unwrap();
    assert!(!resized.contains(dock.center()), "the frame was moved off its place");
    rig.click(dock.center());
    rig.frames(vec![vec![], vec![]]);
    assert!(rig.shell.layout().floating.is_empty());
    assert_eq!(rig.shell.layout().panes, panes, "every viewport is where it was");
    assert_eq!(state(&rig), before, "the mosaic, the cameras and the journal are as they were");
    // Popped out by its action and docked by the button in its bar: the same.
    rig.shell.run_action("viewport-pop-out").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let dock = rig.shell.layout().button("float dock 2").expect("the bar's dock button");
    rig.click(dock.center());
    rig.frames(vec![vec![], vec![]]);
    assert!(rig.shell.floating.is_empty());
    assert_eq!((rig.shell.layout().panes.clone(), state(&rig)), (panes, before.clone()));
    // Control: the engine's own change of the layout is seen by the same measure.
    rig.shell.run("split right").unwrap();
    let split = state(&rig);
    assert_eq!(split.2, before.2 + 1);
    assert!(split.0 != before.0 && split.1 != before.1);
}

#[test]
fn the_big_scopes_are_unchanged_by_the_mosaic() {
    // K1. Criterion: under every mosaic, with a viewport popped out or not, the seven big
    // scopes are in the tree in their order, Tab walks them in it twice round, and the
    // viewport's own stops are the same list. Control: the same scopes ordered by where they
    // are on screen are not in that order.
    const BIG: [&str; 7] = ["top-bar", "toolbar", "tree", "viewport", "inspector", "timeline", "footer"];
    let mut stops: Option<Vec<String>> = None;
    for mosaic in ["A", "AB", "AB;CD", "AAB;CDB", "BA;DC", "ABC;DEF", "ABCD;EFGH;IJKL"] {
        for popped in [false, true] {
            let mut rig = mosaic_rig(mosaic);
            if popped {
                rig.shell.run("viewport pop-out 1").unwrap();
                rig.frames(vec![vec![], vec![]]);
                assert_eq!(rig.shell.layout().floating.len(), 1);
            }
            let what = format!("{mosaic}{}", if popped { ", one popped out" } else { "" });
            assert_eq!(rig.shell.focus_tree().big_scopes(), BIG, "{what}");
            let own: Vec<String> =
                rig.shell.focus_tree().children("viewport").iter().map(|s| (*s).to_owned()).collect();
            assert!(own.iter().any(|s| s == "viewport/mosaic"), "{what}: {own:?}");
            assert_eq!(stops.get_or_insert_with(|| own.clone()), &own, "{what}: the viewport's stops");
            let count = rig.shell.engine.panes();
            assert_eq!(rig.shell.focus_tree().children("viewport/mosaic").len(), count, "{what}");
            let mut seen = Vec::new();
            rig.shell.keyboard.focus = vec!["footer".into()];
            for _ in 0..14 {
                rig.key(Key::Tab, Modifiers::NONE);
                seen.push(rig.shell.keyboard.focused().to_owned());
            }
            assert_eq!(seen, [BIG, BIG].concat(), "{what}: Tab's order");
            assert_eq!(rig.shell.floating.len(), usize::from(popped), "{what}: Tab docks nothing");
            // Control: by place on screen (top to bottom, then left to right) the order is another.
            let tree = rig.shell.focus_tree();
            let mut placed: Vec<(&str, Rect)> =
                BIG.iter().map(|id| (*id, tree.node(id).and_then(|n| n.rect).expect("on screen"))).collect();
            placed.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()).then(a.1.left().total_cmp(&b.1.left())));
            let placed: Vec<&str> = placed.into_iter().map(|(id, _)| id).collect();
            assert_ne!(placed, BIG, "{what}: control");
        }
    }
}

// ---- The viewport header, the status bar and the navigation gizmo (TASK-S20-04): H1 to H4 ----

/// Every text the viewports' headers painted in the last frame.
fn header_texts(rig: &Rig) -> Vec<String> {
    let chrome = &rig.shell.layout().chrome;
    chrome.iter().filter(|(place, _)| *place == "header").map(|(_, text)| text.clone()).collect()
}

#[test]
fn the_header_holds_no_status_and_the_bar_says_the_engine_s() {
    // H1. Criterion (written before measuring): no text a header paints names the instrument,
    // the comparison, the signal's state, the selection or the scene; the status bar's parts say
    // them, in a fixed order, and its `signal` part is `estimate_text` of the engine's signal to
    // the character. Control: a stale signal changes the bar's part and lights the dot of the
    // viewport that plots the signal (and of no other), the header still holding no words of it.
    let mut rig = Rig::new();
    for line in ["load point", "compare aice", "select body 1", "panes 2", "show signal @2"] {
        rig.shell.run(line).unwrap();
    }
    assert!(rig.shell.engine.settle(std::time::Duration::from_secs(60)));
    rig.frames(vec![vec![], vec![]]);
    let spec = rig.shell.engine.spec().clone();
    let instrument = cav_sim::presets::name_of(&spec.array.instrument).expect("a preset");
    let other = cav_sim::presets::name_of(rig.shell.engine.comparison().unwrap()).expect("a preset");
    let estimate = |rig: &Rig| match rig.shell.engine.signal() {
        (cav_engine::SignalState::Refined, Some(signal)) => {
            cav_engine::estimate_text(signal.estimate, signal.settled, signal.truncated)
        }
        other => panic!("the signal is {:?}", other.0),
    };
    let said = estimate(&rig);
    let no_status = |rig: &Rig, what: &str| {
        let texts = header_texts(rig);
        assert_eq!(rig.shell.layout().headers.len(), 2, "{what}: a header a viewport");
        assert!(texts.len() >= 4, "{what}: the headers paint their controls' words: {texts:?}");
        let banned =
            [instrument, other, said.as_str(), "stale", " rad", "pane ", &spec.name, &spec.bodies[0].label];
        for text in &texts {
            for word in banned {
                assert!(!text.contains(word), "{what}: a header says `{text}`, which holds `{word}`");
            }
        }
    };
    no_status(&rig, "settled");
    let layout = rig.shell.layout().clone();
    let names: Vec<&str> = layout.status_bar.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, ["selection", "instrument", "signal", "forward"], "the bar's order");
    assert_eq!(layout.status_part("signal"), Some(said.as_str()), "the engine's words, to the character");
    assert_eq!(layout.status_part("selection"), Some(spec.bodies[0].label.as_str()));
    assert_eq!(layout.status_part("instrument"), Some(format!("{instrument} vs {other}").as_str()));
    assert!(layout.status_part("forward").is_some_and(|f| f.ends_with("refined")), "{:?}", layout.status_bar);
    // What is painted is those words (a part of 32 characters or fewer is painted whole).
    let painted: Vec<&str> =
        layout.chrome.iter().filter(|(place, _)| *place == "status").map(|(_, t)| t.as_str()).collect();
    assert!(said.chars().count() <= 25, "`{said}` fits");
    assert!(painted.contains(&format!("signal {said}").as_str()), "{painted:?}");
    assert!(painted.contains(&format!("{instrument} vs {other}").as_str()), "{painted:?}");
    assert!(!layout.warnings.iter().any(|(pane, ..)| *pane == 1), "settled: no dot on the plot");

    // Control: the scene moves on and the worker is held, so the signal is stale.
    rig.shell.engine.hold_workers(true);
    rig.shell.run("set bodies[0].motion.Static.position [-2, 0, 2.5]").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let layout = rig.shell.layout().clone();
    assert_eq!(layout.status_part("signal"), Some("stale"), "the bar changed");
    let dots: Vec<_> = layout.warnings.iter().filter(|(pane, ..)| *pane == 1).collect();
    assert_eq!(dots.len(), 1, "one dot on the signal's viewport: {:?}", layout.warnings);
    let (_, dot, texts) = dots[0];
    let header = layout.headers.iter().find(|(pane, _)| *pane == 1).unwrap().1;
    assert!(header.contains_rect(*dot), "{dot:?} in its header {header:?}");
    assert!(texts.iter().any(|text| text.contains("stale")), "{texts:?}");
    let stale = |texts: &Vec<String>| texts.iter().any(|text| text.contains("stale"));
    assert!(!layout.warnings.iter().any(|(pane, _, texts)| *pane == 0 && stale(texts)), "not the scene's");
    no_status(&rig, "stale");
    // And back: the worker answers, the dot goes out and the bar says the new estimate.
    rig.shell.engine.hold_workers(false);
    assert!(rig.shell.engine.settle(std::time::Duration::from_secs(60)));
    rig.frames(vec![vec![], vec![]]);
    let layout = rig.shell.layout().clone();
    assert_eq!(layout.status_part("signal"), Some(estimate(&rig).as_str()));
    assert!(!layout.warnings.iter().any(|(pane, ..)| *pane == 1));
    // A long part is cut short where it is painted and whole in the layout (and its tooltip).
    rig.shell.run("select none").unwrap();
    rig.frames(vec![vec![]]);
    assert_eq!(rig.shell.layout().status_part("selection"), Some("nothing selected"));
}

/// What H2 asks of one control: a tooltip, and in it the key of the action it runs when that
/// action has one.
fn says_what_it_does(control: &cav_app::HeaderControl) -> Result<(), String> {
    if control.tooltip.trim().is_empty() {
        return Err(format!("`{}` of viewport {} has no tooltip", control.id, control.pane + 1));
    }
    let Some(id) = &control.action else { return Ok(()) };
    let action = cav_app::actions::table().action(id).ok_or_else(|| format!("`{id}` is not an action"))?;
    match action.keys.first() {
        Some(key) if !control.tooltip.contains(key.as_str()) => {
            Err(format!("`{}`: its tooltip `{}` lacks its key {key}", control.id, control.tooltip))
        }
        _ => Ok(()),
    }
}

#[test]
fn every_header_control_says_what_it_does() {
    // H2. Criterion: in a mosaic of six with mixed view types (one floating), every control a
    // header or a gizmo drew has a tooltip, holding its action's key where the action has one;
    // each viewport has the controls its view type calls for; a disabled one says why; and the
    // keyboard's stops are `header::CONTROLS` in that order. Controls: a control with no tooltip,
    // and one whose tooltip lacks its action's key, fail the same walk.
    use cav_app::header::{CONTROLS, SCOPE};
    let mut rig = mosaic_rig("ABC;DEF");
    for line in ["show signal @2", "show periodogram @3", "show flight @5", "viewport pop-out 6"] {
        rig.shell.run(line).unwrap();
    }
    rig.frames(vec![vec![], vec![]]);
    let layout = rig.shell.layout().clone();
    assert_eq!(layout.headers.len(), 6);
    for control in &layout.controls {
        says_what_it_does(control).unwrap();
        assert!(control.rect.is_positive(), "{control:?}");
    }
    let ids = |pane: usize| -> Vec<&str> {
        layout.controls.iter().filter(|c| c.pane == pane).map(|c| c.id.as_str()).collect()
    };
    let scene = [
        "type",
        "camera",
        "style shaded",
        "style flat",
        "style xray",
        "style wireframe",
        "overlays",
        "maximise",
        "pop-out",
        "close",
        "gizmo/zoom",
        "gizmo/pan",
        "gizmo/projection",
    ];
    let plot = ["type", "span", "maximise", "pop-out", "close"];
    for (pane, wanted) in [(0, &scene[..]), (1, &plot[..]), (2, &plot[..]), (3, &scene[..]), (4, &plot[..])] {
        let mut drawn = ids(pane);
        let mut wanted = wanted.to_vec();
        drawn.sort_unstable();
        wanted.sort_unstable();
        assert_eq!(drawn, wanted, "viewport {}", pane + 1);
    }
    // The keys the design names are in the tooltips.
    let tip = |pane: usize, id: &str| layout.control(pane, id).unwrap().tooltip.clone();
    assert!(tip(0, "maximise").contains("Ctrl+Space"));
    assert!(tip(0, "gizmo/projection").contains("(O)"), "{}", tip(0, "gizmo/projection"));
    assert!(tip(0, "camera").contains("(F)") && tip(0, "camera").contains("Shift+F"), "{}", tip(0, "camera"));
    // Disabled, with why: a plot that shows all of its axis; maximise on the one that floats.
    let span = layout.control(1, "span").unwrap();
    assert!(!span.enabled && span.tooltip.contains("all of it is shown"), "{span:?}");
    assert!(!layout.control(5, "maximise").unwrap().enabled);
    let mut one = Rig::new();
    one.frames(vec![vec![]]);
    let close = one.shell.layout().control(0, "close").unwrap().clone();
    assert!(!close.enabled && close.tooltip.contains("the last viewport cannot be closed"), "{close:?}");
    says_what_it_does(&close).unwrap();
    // The keyboard: the header's stops are the data's, in its order, whatever the viewport shows.
    let wanted: Vec<String> = CONTROLS.iter().map(|c| format!("{SCOPE}/{}", c.id)).collect();
    for pane in [1, 2] {
        rig.shell.run(&format!("pane {pane}")).unwrap();
        rig.frames(vec![vec![], vec![]]);
        let tree = rig.shell.focus_tree();
        assert_eq!(tree.children(SCOPE), wanted, "active viewport {pane}");
        for control in &CONTROLS {
            let node = tree.node(&format!("{SCOPE}/{}", control.id)).unwrap();
            let applies = control.applies.to(rig.shell.engine.content(pane - 1));
            assert!(applies || !node.enabled, "`{}` does not apply to viewport {pane}", control.id);
            if let Some(action) = control.action {
                assert!(cav_app::actions::table().action(action).is_some(), "{action}");
            }
        }
    }
    assert!(!tree_enabled(&rig, "camera") && !tree_enabled(&rig, "span"), "a plot, all of it shown");
    rig.shell.run("pane 1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert!(tree_enabled(&rig, "camera") && tree_enabled(&rig, "type"), "the scene's viewport");
    // Controls: the walk fails a control with no tooltip, and one that hides its key.
    let mut mute = layout.control(0, "maximise").unwrap().clone();
    mute.tooltip = "Maximise".to_owned();
    assert!(says_what_it_does(&mute).unwrap_err().contains("Ctrl+Space"));
    mute.tooltip = " ".to_owned();
    assert!(says_what_it_does(&mute).unwrap_err().contains("no tooltip"));
}

/// Whether the header's stop `id` is enabled in the keyboard's tree.
fn tree_enabled(rig: &Rig, id: &str) -> bool {
    rig.shell.focus_tree().node(&format!("{}/{id}", cav_app::header::SCOPE)).is_some_and(|n| n.enabled)
}

#[test]
fn the_view_type_is_chosen_in_the_header() {
    // H3. Criterion: the view type is the first control of every header (the leftmost, and the
    // first of the data and of the keyboard's stops); choosing Periodogram in the header of
    // viewport 3 of six makes viewport 3 show the periodogram, by one `show` command addressed
    // to it. Control: the other five show what they showed. The same from the keyboard.
    use cav_engine::PaneContent;
    let mut rig = mosaic_rig("ABC;DEF");
    let contents = |rig: &Rig| (0..6).map(|pane| rig.shell.engine.content(pane)).collect::<Vec<_>>();
    assert_eq!(contents(&rig), [PaneContent::Scene; 6]);
    let layout = rig.shell.layout().clone();
    assert_eq!(cav_app::header::CONTROLS[0].id, "type");
    for pane in 0..6 {
        let header = layout.headers.iter().find(|(p, _)| *p == pane).unwrap().1;
        let in_header: Vec<_> =
            layout.controls.iter().filter(|c| c.pane == pane && header.contains_rect(c.rect)).collect();
        let first = in_header.iter().min_by(|a, b| a.rect.left().total_cmp(&b.rect.left())).unwrap();
        assert_eq!(first.id, "type", "viewport {}", pane + 1);
        assert!(first.rect.left() - header.left() <= 4.0);
    }
    // By the pointer: open viewport 3's drop-down, choose Periodogram.
    let journal = rig.shell.engine.journal().len();
    rig.click_button("header type 3");
    rig.click_button("show periodogram @3");
    rig.frames(vec![vec![], vec![]]);
    let mut wanted = [PaneContent::Scene; 6];
    wanted[2] = PaneContent::Periodogram;
    assert_eq!(contents(&rig), wanted, "viewport 3 only");
    let said: Vec<_> = rig.shell.engine.journal()[journal..].to_vec();
    assert!(
        said.contains(&Command::View(ViewCommand::Show { pane: 2, content: PaneContent::Periodogram })),
        "{said:?}"
    );
    assert_eq!(rig.shell.engine.active_pane(), 2, "a click on a header makes its viewport the active one");
    assert!(header_texts(&rig).iter().any(|text| text == "Periodogram"), "the header says what it shows");
    // By the keyboard, on the active viewport's header: Enter opens the list at what is shown,
    // the arrow moves to the next (the flights), Enter chooses it.
    let scope = cav_app::header::SCOPE;
    assert_eq!(rig.shell.focus_tree().children(scope)[0], format!("{scope}/type"));
    rig.shell.keyboard.focus = vec!["viewport".into(), scope.into(), format!("{scope}/type")];
    rig.shell.keyboard.nav_on = true;
    rig.key(Key::Enter, Modifiers::NONE);
    rig.key(Key::ArrowDown, Modifiers::NONE);
    rig.key(Key::Enter, Modifiers::NONE);
    rig.frames(vec![vec![], vec![]]);
    wanted[2] = PaneContent::Flight;
    assert_eq!(contents(&rig), wanted, "the keyboard changed viewport 3 only");
}

#[test]
fn the_gizmo_snaps_to_an_axis_and_orbits() {
    // H4. Criterion: a click on the triad's +Z end gives the top view's yaw and pitch exactly
    // (one `Look`); a drag of d points on the triad orbits by 0.4° a point, to the bit what the
    // same drag in the view gives; no Home or Restore button is laid out and the Home key still
    // resets the camera; the gizmo is inside its canvas and at most a quarter of its shorter
    // side. Control: a click beside the gizmo changes no view.
    use cav_sim::NamedView;
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let layout = rig.shell.layout().clone();
    let gizmo = layout.button("gizmo 1").expect("a scene viewport has a gizmo");
    let canvas = layout.canvases[0];
    assert!(canvas.contains_rect(gizmo), "{gizmo:?} in {canvas:?}");
    assert!(gizmo.width() <= 64.0 && gizmo.width() <= 0.25 * canvas.width().min(canvas.height()));
    assert!(
        (canvas.right() - gizmo.right() - 8.0).abs() < 0.01
            && (gizmo.top() - canvas.top() - 8.0).abs() < 0.01
    );
    assert!(
        layout.button("view home").is_none() && layout.button("maximise").is_none(),
        "no Home, no Restore"
    );
    let angles = |rig: &Rig| (rig.shell.engine.view(0).yaw, rig.shell.engine.view(0).pitch);
    let iso = angles(&rig);
    assert_eq!(iso, NamedView::Iso.angles());
    // Control: a click beside the gizmo is the view's (it selects nothing here) and snaps nothing.
    let journal = rig.shell.engine.journal().len();
    rig.click(gizmo.left_center() - vec2(6.0, 0.0));
    assert_eq!((angles(&rig), rig.shell.engine.journal().len()), (iso, journal), "beside it: nothing");
    // The +Z end: the top view.
    let end = layout.button("gizmo 1 +z").expect("the end is laid out");
    assert!(gizmo.contains_rect(end));
    rig.click(end.center());
    assert_eq!(angles(&rig), NamedView::Top.angles(), "exactly the top view");
    let last = rig.shell.engine.journal().last().cloned();
    assert_eq!(last, Some(Command::View(ViewCommand::Look { pane: 0, view: NamedView::Top })));
    // The end faced, clicked again, looks from the other side; an end at the rim snaps to it.
    let end = rig.shell.layout().button("gizmo 1 +z").unwrap();
    rig.click(end.center());
    assert_eq!(angles(&rig), NamedView::Bottom.angles());
    let end = rig.shell.layout().button("gizmo 1 -x").unwrap();
    rig.click(end.center());
    assert_eq!(angles(&rig), NamedView::West.angles());
    // The Home key still resets.
    rig.key(Key::Home, Modifiers::NONE);
    assert_eq!(angles(&rig), iso, "Home");
    // A drag of d on the triad orbits by 0.4° a point, as the same drag in the view does.
    let d = vec2(30.0, 12.0);
    let from = gizmo.center() + vec2(-12.0, 9.0);
    rig.drag(from, from + d);
    let by_gizmo = angles(&rig);
    assert!(
        (by_gizmo.0 - (iso.0 - 0.4 * 30.0)).abs() < 1e-9 && (by_gizmo.1 - (iso.1 + 0.4 * 12.0)).abs() < 1e-9
    );
    assert!(matches!(
        rig.shell.engine.journal().last(),
        Some(Command::View(ViewCommand::Orbit { pane: 0, .. }))
    ));
    let mut plain = Rig::new();
    plain.shell.run("load point").unwrap();
    plain.frames(vec![vec![], vec![]]);
    let middle = plain.shell.layout().canvases[0].center() + vec2(0.0, 120.0);
    plain.drag(middle, middle + d);
    let by_view = angles(&plain);
    assert_eq!((by_gizmo.0.to_bits(), by_gizmo.1.to_bits()), (by_view.0.to_bits(), by_view.1.to_bits()));
    // The buttons under it: the projection's toggles it, a click on zoom steps nearer.
    let before = rig.shell.engine.view(0).clone();
    rig.click_button("proj toggle @1");
    assert_ne!(rig.shell.engine.view(0).projection, before.projection);
    rig.click_button("gizmo 1 zoom");
    assert!(rig.shell.engine.view(0).distance < before.distance);
    // In a small viewport it is a quarter of the canvas's shorter side (too small a canvas has
    // none: `navgizmo::tests`); with the panels hidden (F3) there is none.
    rig.shell.run("mosaic \"ABCD;EFGH;IJKL\"").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let small = rig.shell.layout().canvases[0];
    let side = rig.shell.layout().button("gizmo 1").map(|g| g.width());
    assert!(small.width().min(small.height()) < 256.0, "{small:?}");
    assert_eq!(side, cav_app::navgizmo::place(small).map(|g| g.width()));
    assert!(side.is_none_or(|side| (side - 0.25 * small.width().min(small.height())).abs() < 0.01));
    rig.shell.run("gui").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert!(rig.shell.layout().button("gizmo 1").is_none());
}

// ---- The tool palette, its flyouts and the options row (TASK-S20-05): P1 and P2 ----

/// What the last frame painted in the tool's options row.
fn options_texts(rig: &Rig) -> Vec<String> {
    let chrome = &rig.shell.layout().chrome;
    chrome.iter().filter(|(place, _)| *place == "options").map(|(_, text)| text.clone()).collect()
}

#[test]
fn the_palette_floats_in_the_canvas_and_moves() {
    // P1. Criterion (written before measuring): the palette's rectangle lies inside the active
    // scene viewport's canvas, clear of the header and the gizmo, its buttons `theme::ICON` a
    // side; a viewport's canvas is the same rectangle with the palette in it as without (bit for
    // bit); after a drag of its grip to the right edge it is anchored there, and still there
    // after `layout instrument` then `layout scene`. Controls: the same measure sees a docked
    // thing (the options row takes `theme::HEADER` off the canvas's height); a drag that starts
    // on a tool's button moves nothing; another layout keeps its own anchor.
    use cav_app::tool_palette::{Anchor, MARGIN};
    let mut rig = mosaic_rig("AB");
    rig.shell.run("pane 1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let with = rig.shell.layout().clone();
    assert_eq!(with.tool_host, Some(0), "the active viewport shows the scene: the palette is its");
    let palette = with.button("tool palette").expect("drawn");
    let canvas = with.canvases[0];
    assert!(canvas.contains_rect(palette), "{palette:?} inside the canvas {canvas:?}");
    assert!(palette.left() - canvas.left() <= MARGIN + 0.5 && palette.top() - canvas.top() <= MARGIN + 0.5);
    let (_, header) = with.headers.iter().find(|(pane, _)| *pane == 0).unwrap();
    assert!(!palette.intersects(*header) && !palette.intersects(with.button("gizmo 1").unwrap()));
    for tool in &cav_app::tools::strip().tools {
        let button = with.button(&format!("tool {}", tool.id)).expect("a button a tool");
        assert_eq!(button.size(), vec2(theme::ICON, theme::ICON), "{}", tool.id);
        assert!(palette.contains_rect(button));
    }
    assert!((palette.width() - (theme::ICON + 2.0 * theme::GRID)).abs() < 0.01, "one column: {palette:?}");
    // The other viewport active: the palette is in it, and both canvases are as they were.
    rig.shell.run("pane 2").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let without = rig.shell.layout().clone();
    assert_eq!(without.tool_host, Some(1));
    assert!(without.canvases[1].contains_rect(without.button("tool palette").unwrap()));
    assert_eq!(with.canvases, without.canvases, "the palette changes no canvas");
    assert_eq!(with.canvases[0].width(), with.panes[0].width(), "the canvas is the viewport's width");
    // Control: a docked row is seen by the same measure (Move's options row, under the header).
    rig.shell.run("tool move").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let docked = rig.shell.layout().canvases.clone();
    assert_eq!(docked[0], without.canvases[0], "not the host: unchanged");
    assert!((without.canvases[1].height() - docked[1].height() - theme::HEADER).abs() < 0.01);
    assert_eq!(docked[1].width(), without.canvases[1].width());
    rig.shell.run("tool select").unwrap();
    // A plot active: the palette is in the first viewport that shows the scene; none: no palette.
    rig.shell.run("show signal @2").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert_eq!(rig.shell.layout().tool_host, Some(0), "viewport 2 is a plot");
    assert!(rig.shell.layout().canvases[0].contains_rect(rig.shell.layout().button("tool palette").unwrap()));
    rig.shell.run("show periodogram @1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert!(rig.shell.layout().tool_host.is_none() && rig.shell.layout().button("tool palette").is_none());
    assert!(!rig.shell.focus_tree().node("toolbar/move").unwrap().enabled, "its stop stays, disabled");
    rig.key(Key::W, Modifiers::NONE);
    assert_eq!(rig.shell.tool, "move", "the tools' keys still take a tool");

    // The drag, in the Scene layout's one viewport.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let journal = rig.shell.engine.journal().len();
    let view = rig.shell.engine.view(0).clone();
    let canvas = rig.shell.layout().canvases[0];
    let palette = rig.shell.layout().button("tool palette").unwrap();
    // Control: a drag from a tool's button moves no palette, and no camera under it.
    let button = rig.shell.layout().button("tool probe").unwrap().center();
    rig.drag(button, button + vec2(300.0, 120.0));
    rig.frames(vec![vec![]]);
    assert_eq!(rig.shell.layout().button("tool palette"), Some(palette));
    assert_eq!(rig.shell.tool_anchor(), Anchor::TopLeft);
    assert_eq!(rig.shell.engine.view(0), &view, "the palette takes the press: the view does not orbit");
    // By the grip: it follows the pointer while held, and let go snaps to the nearest anchor.
    let grip = rig.shell.layout().button("tool palette grip").unwrap().center();
    let to = egui::pos2(canvas.right() - 30.0, canvas.center().y + 20.0);
    let midway = grip + (to - grip) * 0.5;
    rig.frames(vec![
        vec![Event::PointerMoved(grip)],
        vec![Rig::press(grip, PointerButton::Primary, true)],
        vec![Event::PointerMoved(midway)],
        vec![Event::PointerMoved(midway)],
    ]);
    let held = rig.shell.layout().button("tool palette").unwrap();
    let follows = (held.min - palette.min) - (midway - grip);
    assert!(follows.length() < 0.01, "held, it follows the pointer: off by {follows:?}");
    rig.frames(vec![
        vec![Event::PointerMoved(to)],
        vec![Rig::press(to, PointerButton::Primary, false)],
        vec![],
        vec![],
    ]);
    assert_eq!(rig.shell.tool_anchor(), Anchor::Right);
    let at_right = |rig: &Rig| {
        let canvas = rig.shell.layout().canvases[0];
        let palette = rig.shell.layout().button("tool palette").unwrap();
        let off = (canvas.right() - MARGIN - palette.right()).abs();
        off < 1.0 && (palette.center().y - canvas.center().y).abs() < 1.0 && canvas.contains_rect(palette)
    };
    assert!(at_right(&rig), "{:?} in {canvas:?}", rig.shell.layout().button("tool palette"));
    assert_eq!(rig.shell.layout().canvases[0], canvas, "and the canvas is as it was");
    assert_eq!(rig.shell.engine.journal().len(), journal, "the palette's place is never journalled");
    // Another layout has its own place for it (the control); back in this one it is where it was left.
    rig.shell.run("layout instrument").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert_eq!(rig.shell.tool_anchor(), Anchor::TopLeft);
    assert!(!at_right(&rig), "control: the Instrument layout's palette is at its own corner");
    rig.shell.run("layout scene").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert_eq!(rig.shell.tool_anchor(), Anchor::Right);
    assert!(at_right(&rig), "remembered for the layout");
    // At every anchor it is inside the canvas and clear of the gizmo (`tool-palette <anchor>`).
    for anchor in Anchor::ALL {
        rig.shell.run(&format!("tool-palette {}", anchor.name())).unwrap();
        rig.frames(vec![vec![], vec![]]);
        let layout = rig.shell.layout();
        let palette = layout.button("tool palette").unwrap();
        assert!(layout.canvases[0].contains_rect(palette), "{anchor:?}: {palette:?}");
        for (name, rect) in &layout.buttons {
            let gizmo = name.starts_with("gizmo ") || name.starts_with("proj toggle");
            assert!(!(gizmo && rect.intersects(palette)), "{anchor:?}: the palette covers `{name}`");
        }
    }
    assert!(rig.shell.run("tool-palette middle").is_err());
}

#[test]
fn place_opens_a_flyout_and_options_show_only_when_they_apply() {
    // P2. Criterion (written before measuring): holding Place 0.4 s opens a flyout that lists
    // every starter, inside the canvas, and choosing one sets it and takes the tool; the options
    // row has no height with Select or Probe in hand and is one row (`theme::HEADER`) under the
    // header with Move or Place. Controls: a click, not a hold, opens no flyout; a press held
    // 0.2 s has not opened it yet.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let digest = rig.shell.engine.digest();
    let journal = rig.shell.engine.journal().len();
    let plain = rig.shell.layout().canvases[0];
    let header = rig.shell.layout().headers[0].1;
    assert!(rig.shell.layout().options.is_none() && rig.shell.layout().panel("options").is_none());
    assert_eq!(plain.top(), header.bottom(), "Select: the canvas begins under the header");
    let flyout = |rig: &Rig| rig.shell.layout().button("flyout place");

    // Control: a click takes the tool and opens nothing.
    rig.click_button("tool place");
    rig.frames(vec![vec![]]);
    assert_eq!(rig.shell.tool, "place");
    assert!(rig.shell.flyout.is_none() && flyout(&rig).is_none(), "a click opens no flyout");
    // Place in hand: one row under the header, the canvas under it; the starter and the ground.
    let row = rig.shell.layout().options.expect("Place has options");
    assert_eq!(rig.shell.layout().panel("options"), Some(row));
    assert!((row.height() - theme::HEADER).abs() < 0.01 && row.top() == header.bottom());
    assert_eq!((row.left(), row.right()), (header.left(), header.right()), "inside that viewport");
    let canvas = rig.shell.layout().canvases[0];
    assert_eq!(canvas.top(), row.bottom());
    assert!((plain.height() - canvas.height() - theme::HEADER).abs() < 0.01);
    assert_eq!(options_texts(&rig), ["Sphere", "z 0 m"]);
    assert!(row.contains_rect(rig.shell.layout().button("place starter").unwrap()));
    // Move: the snap switch with its step. Probe and Select: no row at all.
    rig.click_button("tool move");
    rig.frames(vec![vec![]]);
    assert_eq!(options_texts(&rig), ["Snap 0.25 m"]);
    assert!((rig.shell.layout().options.unwrap().height() - theme::HEADER).abs() < 0.01);
    rig.click_button("snap");
    rig.frames(vec![vec![]]);
    assert!(!rig.shell.snap);
    assert_eq!(options_texts(&rig), ["Snap off"]);
    rig.shell.run("snap").unwrap();
    for tool in ["probe", "select"] {
        rig.click_button(&format!("tool {tool}"));
        rig.frames(vec![vec![]]);
        assert_eq!(rig.shell.tool, tool);
        assert!(rig.shell.layout().options.is_none() && options_texts(&rig).is_empty(), "{tool}");
        assert_eq!(rig.shell.layout().canvases[0], plain, "{tool}: the canvas has the room");
    }

    // A hold: 0.2 s is not yet enough (the control); at 0.4 s the flyout opens, the button
    // still down, and letting go takes no tool.
    let place = rig.shell.layout().button("tool place").unwrap().center();
    let mut frames =
        vec![vec![Event::PointerMoved(place)], vec![Rig::press(place, PointerButton::Primary, true)]];
    frames.extend(std::iter::repeat_with(Vec::new).take(12));
    rig.frames(frames);
    assert!(rig.shell.flyout.is_none(), "control: 0.2 s is no hold");
    rig.frames(std::iter::repeat_with(Vec::new).take(14).collect());
    assert_eq!(rig.shell.flyout.as_deref(), Some("place"), "held 0.4 s");
    rig.frames(vec![vec![Rig::press(place, PointerButton::Primary, false)], vec![], vec![]]);
    assert_eq!(rig.shell.tool, "select", "the hold's release takes no tool");
    let list = flyout(&rig).expect("the flyout stays open");
    let canvas = rig.shell.layout().canvases[0];
    let palette = rig.shell.layout().button("tool palette").unwrap();
    assert!(canvas.contains_rect(list), "{list:?} inside the canvas {canvas:?}");
    assert!(
        !list.intersects(palette) && list.left() >= palette.right(),
        "beside the palette, toward the middle"
    );
    let starters = cav_sim::starters::starters();
    assert!(starters.len() >= 10);
    for starter in starters {
        let row = rig.shell.layout().button(&format!("tool place {}", starter.name));
        let row = row.unwrap_or_else(|| panic!("no row for {}", starter.name));
        assert!(list.contains_rect(row) && (row.height() - theme::HIT).abs() < 0.01, "{}", starter.name);
    }
    // Choosing one sets it and takes the tool; the flyout closes.
    rig.click_button("tool place cube");
    rig.frames(vec![vec![], vec![]]);
    assert_eq!((rig.shell.tool.as_str(), rig.shell.place_starter.as_str()), ("place", "cube"));
    assert!(rig.shell.flyout.is_none() && flyout(&rig).is_none());
    assert_eq!(options_texts(&rig), ["Cube", "z 0 m"]);

    // A right-click opens it too (and flies no camera); a press elsewhere closes it.
    rig.shell.run("tool select").unwrap();
    rig.frames(vec![vec![]]);
    rig.click_with(place, PointerButton::Secondary);
    rig.frames(vec![vec![]]);
    assert!(flyout(&rig).is_some() && rig.shell.mouse_fly.is_none());
    assert_eq!(rig.shell.tool, "select");
    rig.click(rig.shell.layout().canvases[0].center() + vec2(0.0, 200.0));
    rig.frames(vec![vec![]]);
    assert!(rig.shell.flyout.is_none() && flyout(&rig).is_none(), "a press elsewhere closes it");
    // With the palette at the right edge the flyout opens to its left, still inside the canvas.
    rig.shell.run("tool-palette right").unwrap();
    rig.shell.run("flyout place").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let (list, palette) = (flyout(&rig).unwrap(), rig.shell.layout().button("tool palette").unwrap());
    assert!(rig.shell.layout().canvases[0].contains_rect(list) && list.right() <= palette.left());
    rig.shell.run("flyout off").unwrap();
    rig.shell.run("tool-palette top-left").unwrap();

    // From the keyboard: the stop after Place's is its variants, a drop-down (Enter opens the
    // flyout, the arrows choose, Enter takes the tool with that one).
    let stops: Vec<&str> = rig.shell.focus_tree().children("toolbar");
    assert_eq!(
        stops,
        [
            "toolbar/select",
            "toolbar/move",
            "toolbar/place",
            "toolbar/place/variants",
            "toolbar/probe",
            "toolbar/snap"
        ],
        "the order is the data's"
    );
    rig.shell.keyboard.focus = vec!["toolbar".into(), "toolbar/place".into()];
    rig.shell.keyboard.nav_on = true;
    rig.key(Key::Enter, Modifiers::NONE);
    assert!(rig.shell.tool == "place" && flyout(&rig).is_none(), "Enter on the tool takes it, no flyout");
    rig.shell.run("tool select").unwrap();
    rig.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(rig.shell.keyboard.focused(), "toolbar/place/variants");
    rig.key(Key::Enter, Modifiers::NONE);
    rig.frames(vec![vec![]]);
    assert!(flyout(&rig).is_some(), "Enter on its variants opens the flyout");
    rig.key(Key::ArrowDown, Modifiers::NONE);
    rig.key(Key::Enter, Modifiers::NONE);
    rig.frames(vec![vec![]]);
    let cube = starters.iter().position(|s| s.name == "cube").unwrap();
    assert_eq!(rig.shell.place_starter, starters[cube + 1].name, "the one after the one chosen");
    assert_eq!(rig.shell.tool, "place");
    assert!(flyout(&rig).is_none());
    // The console's line is the same; a variant that is none of the tool's is refused in words.
    rig.shell.run("tool place sphere").unwrap();
    assert_eq!(rig.shell.place_starter, "sphere");
    assert!(rig.shell.run("tool place brick").unwrap_err().contains("sphere|"));
    assert!(rig.shell.run("tool move cube").unwrap_err().contains("no variants"));
    assert_eq!(rig.shell.place_starter, "sphere");
    // None of it touched the scene.
    assert_eq!((rig.shell.engine.digest(), rig.shell.engine.journal().len()), (digest, journal));
}

#[test]
fn a_press_in_the_session_s_row_keeps_the_session() {
    // P2, the shell's use of the row (`Shell::keys`): a primary press outside the options row
    // ends a transform session, its move kept; a press in the row does not (the control).
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("select body 1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert!(rig.shell.layout().options.is_none(), "Select in hand, no session: no row");
    rig.shell.keyboard.focus = vec!["viewport".into(), "viewport/selection".into()];
    rig.shell.keyboard.nav_on = true;
    rig.key(Key::Enter, Modifiers::NONE);
    rig.key(Key::ArrowRight, Modifiers::NONE);
    rig.frames(vec![vec![]]);
    assert!(rig.shell.session.is_some());
    let row = rig.shell.layout().panel("options").expect("a session under way shows its row");
    assert!(options_texts(&rig).iter().any(|text| text.starts_with('Δ') && text.ends_with(" m")));
    rig.click(row.right_center() - vec2(40.0, 0.0));
    assert!(rig.shell.session.is_some(), "control: a press in its own row keeps the session");
    let outliner = rig.shell.layout().panel("outliner").unwrap();
    rig.click(outliner.center_bottom() - vec2(0.0, 12.0));
    assert!(rig.shell.session.is_none(), "a press elsewhere ends it");
    rig.frames(vec![vec![]]);
    assert!(rig.shell.layout().options.is_none(), "and the row goes");
}

/// The transport's control `id` as the last frame drew it.
fn transport(rig: &Rig, id: &str) -> cav_app::transport::Drawn {
    rig.shell.layout().transport_control(id).unwrap_or_else(|| panic!("no transport control `{id}`")).clone()
}

/// A key press (and its release) with the keyboard's focus in the timeline.
fn timeline_key(rig: &mut Rig, key: Key, modifiers: Modifiers) {
    rig.shell.keyboard.focus = vec!["timeline".into()];
    rig.key(key, modifiers);
}

#[test]
fn each_transport_button_issues_its_command() {
    // T1. Criterion (written before measuring): the five buttons stand left to right in the
    // timeline's head, `theme::ICON` a side, each an action of `ui/actions.ron` whose tooltip
    // holds its key; a click on end leaves t = duration (bits) by one `seek`, on start t = 0, on
    // play toggles playing; the keys do the same in the timeline; the keyboard's stops are the
    // data's order; a time typed in the time field seeks there and a shot's number goes to its
    // readout. Controls: at the start, step back and start are disabled and say why, and a click
    // on them changes nothing (no journal line); letters typed as a time are refused, the
    // playhead where it was.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let duration = rig.shell.engine.spec().duration;
    assert_eq!(duration, 12.0);
    let table = cav_app::actions::table();
    let head = rig.shell.layout().panel("timeline").unwrap();
    let mut right = head.left();
    for control in &cav_app::transport::CONTROLS {
        let drawn = transport(&rig, control.id);
        assert!(head.contains_rect(drawn.rect), "{}: {:?} in {head:?}", control.id, drawn.rect);
        assert!(drawn.rect.left() >= right - 0.01, "{}: left to right, none over another", control.id);
        right = drawn.rect.right();
        let Some(action) = control.action else { continue };
        let action = table.action(action).unwrap();
        assert_eq!(drawn.rect.size(), vec2(theme::ICON, theme::ICON), "{}", control.id);
        let key = action.shortcut_text();
        assert!(!key.is_empty() && drawn.tooltip.contains(&format!("({key})")), "{}", drawn.tooltip);
        assert!(drawn.tooltip.contains(&action.about), "{}", drawn.tooltip);
        assert_eq!(rig.shell.layout().button(&action.line), Some(drawn.rect), "found by its line");
    }
    let stops: Vec<&str> = rig.shell.focus_tree().children("timeline");
    assert_eq!(
        stops,
        [
            "timeline/start",
            "timeline/step-back",
            "timeline/play",
            "timeline/step-forward",
            "timeline/end",
            "timeline/at-end",
            "timeline/speed",
            "timeline/time",
            "timeline/duration",
            "timeline/shot",
            "timeline/scrubber"
        ],
        "the order is the data's"
    );

    // Control: at the start, start and step back cannot be used and say why; a click is nothing.
    let journal = rig.shell.engine.journal().len();
    for (id, why) in [("start", "the playhead is at the start"), ("step-back", "there is no shot before")] {
        let drawn = transport(&rig, id);
        assert!(!drawn.enabled && drawn.tooltip.contains(why), "{id}: {}", drawn.tooltip);
        assert!(!rig.shell.focus_tree().node(&format!("timeline/{id}")).unwrap().enabled);
        rig.click(drawn.rect.center());
    }
    assert!(transport(&rig, "end").enabled && transport(&rig, "step-forward").enabled);
    assert_eq!((rig.shell.engine.time(), rig.shell.engine.journal().len()), (0.0, journal));
    assert!(!rig.shell.playing && !rig.shell.console_open);

    // End, then start: one seek each, to the bit.
    rig.click_button("seek end");
    assert_eq!(rig.shell.engine.time().to_bits(), duration.to_bits());
    assert_eq!(rig.shell.engine.journal().last(), Some(&Command::Seek(duration)));
    rig.frames(vec![vec![]]);
    let end = transport(&rig, "end");
    assert!(!end.enabled && end.tooltip.contains("the playhead is at the end"), "{}", end.tooltip);
    assert!(!transport(&rig, "step-forward").enabled);
    rig.click_button("seek start");
    assert_eq!(rig.shell.engine.time(), 0.0);
    assert_eq!(rig.shell.engine.journal().last(), Some(&Command::Seek(0.0)));
    // Play toggles, and the playhead moves while it plays.
    rig.click_button("play");
    assert!(rig.shell.playing);
    rig.frames(vec![vec![]; 6]);
    assert!(rig.shell.engine.time() > 0.05);
    rig.click_button("play");
    assert!(!rig.shell.playing);
    let paused = rig.shell.engine.time();
    rig.frames(vec![vec![]; 6]);
    assert_eq!(rig.shell.engine.time(), paused);
    // The step buttons: on to the first readout and back to the start's side of it.
    let first = rig.shell.engine.shots().unwrap().readout(0, 0);
    rig.click_button("step 1");
    assert_eq!(rig.shell.engine.time().to_bits(), first.to_bits());
    rig.click_button("step 1");
    rig.click_button("step -1");
    assert_eq!(rig.shell.engine.time().to_bits(), first.to_bits());

    // The keys, in the timeline: Alt+] and Alt+[ the ends, ] and [ a shot, Space plays.
    timeline_key(&mut rig, Key::CloseBracket, Modifiers::ALT);
    assert_eq!(rig.shell.engine.time(), duration);
    timeline_key(&mut rig, Key::OpenBracket, Modifiers::ALT);
    assert_eq!(rig.shell.engine.time(), 0.0);
    timeline_key(&mut rig, Key::CloseBracket, Modifiers::NONE);
    assert_eq!(rig.shell.engine.time().to_bits(), first.to_bits());
    timeline_key(&mut rig, Key::OpenBracket, Modifiers::NONE);
    assert_eq!(rig.shell.engine.time(), first, "control: no shot before the first, so [ does nothing");
    timeline_key(&mut rig, Key::Space, Modifiers::NONE);
    assert!(rig.shell.playing);
    timeline_key(&mut rig, Key::Space, Modifiers::NONE);
    assert!(!rig.shell.playing);
    // Enter on a stop is its click.
    rig.shell.keyboard.focus = vec!["timeline".into(), "timeline/end".into()];
    rig.shell.keyboard.nav_on = true;
    rig.key(Key::Enter, Modifiers::NONE);
    assert_eq!(rig.shell.engine.time(), duration);

    // The time is typed where it is read: Enter on the field, a time, Enter.
    let typed = |rig: &mut Rig, field: &str, text: &str| {
        rig.shell.keyboard.focus = vec!["timeline".into(), format!("timeline/{field}")];
        rig.shell.keyboard.nav_on = true;
        rig.key(Key::Enter, Modifiers::NONE);
        rig.frames(vec![vec![]]);
        rig.typed(text);
        rig.key(Key::Enter, Modifiers::NONE);
        rig.frames(vec![vec![]]);
    };
    assert_eq!(transport(&rig, "time").text, "12.00");
    typed(&mut rig, "time", "3.2");
    assert_eq!(rig.shell.engine.time(), 3.2);
    assert_eq!(rig.shell.engine.journal().last(), Some(&Command::Seek(3.2)));
    assert_eq!(transport(&rig, "time").text, "3.20");
    assert_eq!(transport(&rig, "shot").text, "1", "one shot is read out by 3.2 s");
    // The shot's number goes to that shot's readout.
    typed(&mut rig, "shot", "3");
    let third = rig.shell.engine.shots().unwrap().readout(0, 2);
    assert_eq!(rig.shell.engine.time().to_bits(), third.to_bits());
    assert_eq!(transport(&rig, "shot").text, "3");
    // Control: letters are no time and 9 is no shot of six; each is refused in words.
    assert!(!rig.shell.console_open);
    typed(&mut rig, "time", "soon");
    assert_eq!(rig.shell.engine.time().to_bits(), third.to_bits());
    assert!(rig.shell.console_open, "the refusal is said");
    let said = |rig: &Rig| rig.shell.engine.log().iter().next_back().unwrap().message.clone();
    assert!(said(&rig).contains("`soon` is not a time in seconds"), "{}", said(&rig));
    typed(&mut rig, "shot", "9");
    assert_eq!(rig.shell.engine.time().to_bits(), third.to_bits());
    assert!(said(&rig).contains("shots 1 to 6"), "{}", said(&rig));
    // Esc leaves a field as it was.
    rig.shell.keyboard.focus = vec!["timeline".into(), "timeline/time".into()];
    rig.key(Key::Enter, Modifiers::NONE);
    rig.frames(vec![vec![]]);
    rig.typed("7");
    rig.key(Key::Escape, Modifiers::NONE);
    rig.frames(vec![vec![]]);
    assert_eq!(rig.shell.engine.time().to_bits(), third.to_bits());
    // None of it is an edit of the scene.
    assert_eq!(rig.shell.engine.depth(), (1, 0), "the load alone");
}

#[test]
fn a_step_lands_on_a_shot_time() {
    // T2. Criterion (written before measuring): after `step 1` the time is the next of the
    // engine's own readouts, bit for bit, and `step -1` the one before; with Shift (or `cycle`)
    // a step is one mean cadence cycle; the shots are the selected tower's, else the first's; a
    // step stops playback; a held button repeats on the keyboard's clock (0.5 s, then every
    // 40 ms). Controls: from between two shots a step back lands on the shot before, not a cycle
    // back; a tower with its own offset gives other times; a press held 0.3 s is one step.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let readouts: Vec<f64> = {
        let shots = rig.shell.engine.shots().unwrap();
        (0..shots.count(0)).map(|k| shots.readout(0, k)).collect()
    };
    assert_eq!(readouts.len(), 6);
    let bits = |rig: &Rig| rig.shell.engine.time().to_bits();
    rig.shell.run("step 1").unwrap();
    assert_eq!(bits(&rig), readouts[0].to_bits());
    rig.shell.run("step 1").unwrap();
    assert_eq!(bits(&rig), readouts[1].to_bits());
    rig.shell.run("step 3").unwrap();
    assert_eq!(bits(&rig), readouts[4].to_bits(), "three shots on");
    rig.shell.run("step -2").unwrap();
    assert_eq!(bits(&rig), readouts[2].to_bits());
    // Between two shots (3.46 s and 5.46 s): back is the one before, on is the one after.
    let cycle = {
        let spec = rig.shell.engine.spec();
        spec.array.schedule.mean_cycle(spec.array.instrument.cycle)
    };
    assert_eq!(cycle, 2.0);
    rig.shell.run("seek 4").unwrap();
    rig.shell.run("step -1").unwrap();
    assert_eq!(bits(&rig), readouts[1].to_bits());
    assert!((rig.shell.engine.time() - (4.0 - cycle)).abs() > 1.0, "control: not one cycle back");
    rig.shell.run("seek 4").unwrap();
    rig.shell.run("step 1").unwrap();
    assert_eq!(bits(&rig), readouts[2].to_bits());
    // A cycle step is the mean cycle, from wherever the playhead is.
    rig.shell.run("seek 4").unwrap();
    rig.shell.run("step -1 cycle").unwrap();
    assert!((rig.shell.engine.time() - (4.0 - cycle)).abs() <= 1e-12);
    rig.shell.run("step 2 cycle").unwrap();
    assert!((rig.shell.engine.time() - (4.0 + cycle)).abs() <= 1e-12);
    // Shift with the button, and with the key, is the cycle step.
    rig.frames(vec![vec![]]);
    let back = transport(&rig, "step-back").rect.center();
    let shift = |pressed| Event::PointerButton {
        pos: back,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::SHIFT,
    };
    rig.frames(vec![vec![Event::PointerMoved(back)], vec![shift(true)], vec![shift(false)], vec![]]);
    assert!((rig.shell.engine.time() - 4.0).abs() <= 1e-12, "Shift+click: {}", rig.shell.engine.time());
    timeline_key(&mut rig, Key::CloseBracket, Modifiers::SHIFT);
    assert!((rig.shell.engine.time() - (4.0 + cycle)).abs() <= 1e-12);
    // (A US keyboard sends `{` for Shift+[: the same step.)
    timeline_key(&mut rig, Key::OpenCurlyBracket, Modifiers::SHIFT);
    assert!((rig.shell.engine.time() - 4.0).abs() <= 1e-12);

    // The ends: no shot after the last, and the refusal says so; the playhead stays.
    rig.shell.run("seek end").unwrap();
    assert!(rig.shell.run("step 1").unwrap_err().contains("no shot after the playhead"));
    assert_eq!(rig.shell.engine.time(), 12.0);
    assert!(rig.shell.run("step 1 cycle").unwrap_err().contains("at the end"));
    assert!(rig.shell.run("step 0").is_err() && rig.shell.run("step").is_err());
    rig.shell.run("step -1").unwrap();
    assert_eq!(bits(&rig), readouts[5].to_bits());
    // A step stops playback.
    rig.shell.run("seek 0").unwrap();
    rig.shell.run("play").unwrap();
    rig.shell.run("step 1").unwrap();
    assert!(!rig.shell.playing);
    assert_eq!(bits(&rig), readouts[0].to_bits());

    // A held button scrubs on the keyboard's clock: one step as it goes down, a second after
    // 0.5 s, then one every 40 ms. Held 0.6 s: 1 + 1 + 2. Control: held 0.3 s, one.
    rig.shell.run("seek 0").unwrap();
    rig.frames(vec![vec![]]);
    let on = transport(&rig, "step-forward").rect.center();
    let hold = |rig: &mut Rig, frames: usize| {
        let mut all = vec![vec![Event::PointerMoved(on)], vec![Rig::press(on, PointerButton::Primary, true)]];
        all.extend(std::iter::repeat_with(Vec::new).take(frames));
        all.push(vec![Rig::press(on, PointerButton::Primary, false)]);
        all.push(vec![]);
        rig.frames(all);
    };
    hold(&mut rig, 18);
    assert_eq!(bits(&rig), readouts[0].to_bits(), "control: 0.3 s is one step");
    rig.shell.run("seek 0").unwrap();
    hold(&mut rig, 36);
    assert_eq!(bits(&rig), readouts[3].to_bits(), "held 0.6 s: four steps");
    let (delay, every) = (cav_app::keyboard::REPEAT_DELAY, cav_app::keyboard::REPEAT_EVERY);
    assert_eq!(2 + ((0.6 - delay + 1e-9) / every).floor() as usize, 4, "the keyboard's clock gives four");

    // The shots are the selected tower's: tower 2 with its clock 0.3 s late has other readouts.
    rig.shell.run("set array.towers[1].timing.offset 0.3").unwrap();
    assert!(rig.shell.engine.settle(std::time::Duration::from_secs(30)));
    rig.shell.run("seek 0").unwrap();
    rig.shell.run("select tower 2").unwrap();
    rig.shell.run("step 1").unwrap();
    let late = rig.shell.engine.shots().unwrap().readout(1, 0);
    assert_eq!(bits(&rig), late.to_bits());
    assert!((late - readouts[0] - 0.3).abs() < 1e-12 && late != readouts[0], "control: another time");
    rig.shell.run("select none").unwrap();
    rig.shell.run("seek 0").unwrap();
    rig.shell.run("step 1").unwrap();
    assert_eq!(bits(&rig), readouts[0].to_bits(), "nothing selected: the first tower's");
}

#[test]
fn speed_scales_playback() {
    // T3. Criterion (written before measuring): N frames of dt at 2× advance the playhead
    // 2·N·dt and at 0.1× a tenth of N·dt, each to 1e-12 of the 1× advance scaled (the control
    // arm, itself N·dt of the frames' sixtieth of a second to 1e-6); the speed is chosen from
    // the transport's drop-down, whose rows are the table's; a speed outside 0.1 to 10 is
    // refused.
    const N: usize = 30;
    let played = |speed: &str| {
        let mut rig = Rig::new();
        rig.shell.run("load point").unwrap();
        rig.shell.run(&format!("speed {speed}")).unwrap();
        rig.shell.run("play").unwrap();
        rig.frames(vec![vec![]; N]);
        rig.shell.engine.time()
    };
    let once = played("1");
    assert!((once - N as f64 / 60.0).abs() < 1e-6, "control: 1× is N·dt, {once}");
    assert!((played("2") - 2.0 * once).abs() <= 1e-12, "{} at 2×", played("2"));
    assert!((played("0.1") - 0.1 * once).abs() <= 1e-12, "{} at 0.1×", played("0.1"));
    assert!((played("10") - 10.0 * once).abs() <= 1e-12);

    // The drop-down: its face says the speed; a click opens the list, a row chooses.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.frames(vec![vec![], vec![]]);
    assert_eq!((rig.shell.speed, transport(&rig, "speed").text.as_str()), (1.0, "1×"));
    assert!(rig.shell.layout().button("speed menu").is_none(), "closed");
    rig.click(transport(&rig, "speed").rect.center());
    rig.frames(vec![vec![]]);
    let list = rig.shell.layout().button("speed menu").expect("the list is open");
    let head = transport(&rig, "speed").rect;
    assert!(list.bottom() <= head.top() + 0.01, "above its opener: {list:?} over {head:?}");
    for speed in cav_app::transport::SPEEDS {
        assert!(list.contains_rect(rig.shell.layout().button(&format!("speed {speed}")).unwrap()), "{speed}");
    }
    rig.click_button("speed 2");
    rig.frames(vec![vec![]]);
    assert_eq!((rig.shell.speed, transport(&rig, "speed").text.as_str()), (2.0, "2×"));
    assert!(rig.shell.layout().button("speed menu").is_none(), "choosing closes it");
    // A press elsewhere closes it unchanged; so does its console line.
    rig.shell.run("speed menu").unwrap();
    rig.frames(vec![vec![]]);
    assert!(rig.shell.layout().button("speed menu").is_some());
    rig.click(rig.shell.layout().canvases[0].center());
    rig.frames(vec![vec![]]);
    assert!(rig.shell.layout().button("speed menu").is_none() && rig.shell.speed == 2.0);
    // From the keyboard: Enter opens it on the speed in force, ↓ and Enter choose the next.
    rig.shell.keyboard.focus = vec!["timeline".into(), "timeline/speed".into()];
    rig.shell.keyboard.nav_on = true;
    rig.key(Key::Enter, Modifiers::NONE);
    rig.key(Key::ArrowDown, Modifiers::NONE);
    rig.key(Key::Enter, Modifiers::NONE);
    rig.frames(vec![vec![]]);
    assert_eq!(rig.shell.speed, 5.0);
    // Refused in words, the speed as it was.
    for line in ["speed 11", "speed 0", "speed fast", "speed"] {
        assert!(rig.shell.run(line).unwrap_err().contains("0.1 to 10"), "{line}");
    }
    assert_eq!(rig.shell.speed, 5.0);
    assert_eq!(rig.shell.engine.journal().len(), 1, "the speed is the window's: nothing journalled");
}

/// Whether any two of `rects` overlap by more than half a point.
fn overlapping(rects: &[(String, Rect)]) -> Option<(String, String)> {
    for (n, (a, first)) in rects.iter().enumerate() {
        for (b, second) in &rects[n + 1..] {
            let common = first.intersect(*second);
            if common.width() > 0.5 && common.height() > 0.5 {
                return Some((a.clone(), b.clone()));
            }
        }
    }
    None
}

#[test]
fn the_top_bar_is_menus_tabs_search_and_the_scene() {
    // The design's §4 (TASK-S20-06). Criterion (written before measuring): the top bar paints, in
    // one row and in this order, the table's menus, the layouts' tabs, the search box and, at
    // the right, the scene's name (with ` *` once it has an unsaved edit), and nothing else: no
    // time. `+` keeps the present arrangement as a new tab for the session, which brings it
    // back; at 900 points wide nothing of the top bar or of the transport lies over anything
    // else or leaves the window. Controls: the breadcrumb is there only while keyboard
    // navigation is on; a layout that was forgotten is no tab.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let parts = |rig: &Rig, kind: &str| -> Vec<String> {
        let bar = &rig.shell.layout().top_bar;
        bar.iter().filter(|(k, ..)| *k == kind).map(|(_, text, _)| text.clone()).collect()
    };
    let kinds = |rig: &Rig| -> Vec<&'static str> {
        let mut kinds: Vec<&str> = rig.shell.layout().top_bar.iter().map(|(k, ..)| *k).collect();
        kinds.dedup();
        kinds
    };
    assert_eq!(kinds(&rig), ["menu", "layout", "search", "scene"]);
    assert_eq!(parts(&rig, "menu"), cav_app::actions::table().menus);
    assert_eq!(parts(&rig, "layout"), ["Scene", "Instrument", "Analysis"]);
    assert_eq!(parts(&rig, "scene"), ["point"]);
    let fits = |rig: &Rig, what: &str| {
        let layout = rig.shell.layout();
        let (bar, screen) = (layout.panel("menu").unwrap(), layout.screen);
        let mut rects: Vec<(String, Rect)> =
            layout.top_bar.iter().map(|(kind, text, rect)| (format!("{kind} {text}"), *rect)).collect();
        rects.push(("+".to_owned(), layout.button("layout save").unwrap()));
        for (name, rect) in &rects {
            assert!(bar.contains_rect(*rect) && screen.contains_rect(*rect), "{what}: `{name}` at {rect:?}");
        }
        assert_eq!(overlapping(&rects), None, "{what}: the top bar");
        let timeline = layout.panel("timeline").unwrap();
        let controls: Vec<(String, Rect)> =
            layout.transport.iter().map(|drawn| (drawn.id.to_owned(), drawn.rect)).collect();
        assert_eq!(controls.len(), cav_app::transport::CONTROLS.len(), "{what}: each once");
        for (name, rect) in &controls {
            assert!(timeline.contains_rect(*rect), "{what}: `{name}` at {rect:?} in {timeline:?}");
        }
        assert_eq!(overlapping(&controls), None, "{what}: the transport");
    };
    fits(&rig, "1440");
    let scene = rig.shell.layout().top_bar.iter().find(|(k, ..)| *k == "scene").unwrap().2;
    let search = rig.shell.layout().button("palette").unwrap();
    assert!(scene.right() > 1440.0 - 24.0, "the scene's name is at the right: {scene:?}");
    assert!((search.width() - 168.0).abs() < 0.01 && search.height() == theme::HIT, "{search:?}");
    // An edit: the unsaved mark.
    rig.shell.run("set duration 13").unwrap();
    rig.frames(vec![vec![]]);
    assert_eq!(parts(&rig, "scene"), ["point *"]);
    // The breadcrumb, only while keyboard navigation is on.
    rig.shell.keyboard.nav_on = true;
    rig.frames(vec![vec![], vec![]]);
    assert_eq!(kinds(&rig), ["menu", "layout", "search", "scene", "breadcrumb"]);
    rig.shell.keyboard.nav_on = false;
    rig.frames(vec![vec![]]);
    assert_eq!(kinds(&rig).len(), 4, "control: no breadcrumb with navigation off");
    // The search box opens the palette.
    rig.click_button("palette");
    assert!(rig.shell.palette.open);
    rig.key(Key::Escape, Modifiers::NONE);
    rig.frames(vec![vec![]]);

    // `+`: the present arrangement becomes a tab of this session, and choosing it brings the
    // arrangement back.
    let journal = rig.shell.engine.journal().len();
    rig.shell.run("mosaic \"AB;CB\" cols 2 1").unwrap();
    rig.shell.run("show periodogram @3").unwrap();
    let saved = rig.shell.engine.mosaic().clone();
    rig.frames(vec![vec![]]);
    rig.click_button("layout save");
    rig.frames(vec![vec![]]);
    assert_eq!(parts(&rig, "layout"), ["Scene", "Instrument", "Analysis", "Layout 1"]);
    assert_eq!(rig.shell.layout_name, "layout-1");
    assert_eq!(rig.shell.engine.journal().len(), journal + 2, "saving a layout journals nothing");
    rig.click_button("layout scene");
    assert_eq!(rig.shell.engine.panes(), 1);
    rig.frames(vec![vec![]]);
    rig.click_button("layout layout-1");
    assert_eq!(rig.shell.engine.mosaic(), &saved, "the mosaic, weights and all");
    assert_eq!(rig.shell.engine.content(2), cav_engine::PaneContent::Periodogram);
    // A name can be given; the same name twice is refused; a seventh is refused with the count.
    rig.shell.run("layout save Two plots").unwrap();
    assert_eq!(rig.shell.layout_name, "two-plots");
    assert!(rig.shell.run("layout save Two plots").unwrap_err().contains("taken"));
    assert!(rig.shell.run("layout save Scene").unwrap_err().contains("taken"));
    // At 900 points wide, with five tabs: nothing over anything, nothing off the window.
    rig.size = [900, 700];
    rig.frames(vec![vec![], vec![], vec![]]);
    fits(&rig, "900");
    assert!(rig.shell.layout().button("palette").unwrap().width() >= theme::ICON);
    rig.size = [1440, 900];
    // Control: forgotten, it is no tab, and the window is on a built-in layout again.
    rig.shell.run("layout forget two-plots").unwrap();
    rig.shell.run("layout forget layout-1").unwrap();
    rig.frames(vec![vec![]]);
    assert_eq!(parts(&rig, "layout"), ["Scene", "Instrument", "Analysis"]);
    assert_eq!(rig.shell.layout_name, "scene");
    assert!(rig.shell.run("layout forget scene").unwrap_err().contains("not a layout saved"));
    assert!(rig.shell.run("layout layout-1").is_err(), "and its name is no layout");
    for _ in 0..6 {
        rig.shell.run("layout save").unwrap();
    }
    assert!(rig.shell.run("layout save").unwrap_err().contains("keeps 6 layouts"));
}

/// Types `text` into the transport's field `field` from the keyboard: Enter on it, the text
/// (which replaces what was there), Enter.
fn type_at_transport(rig: &mut Rig, field: &str, text: &str) {
    rig.shell.keyboard.focus = vec!["timeline".into(), format!("timeline/{field}")];
    rig.shell.keyboard.nav_on = true;
    rig.key(Key::Enter, Modifiers::NONE);
    rig.frames(vec![vec![]]);
    rig.typed(text);
    rig.key(Key::Enter, Modifiers::NONE);
    rig.frames(vec![vec![]]);
}

#[test]
fn duration_is_typed_at_the_transport() {
    // T4. Criterion (written before measuring): 20 typed in the transport's duration box is
    // the `set duration` the Scene card's editor issues: the same command in the journal (path,
    // value and all), the same line in words, one step of undo, which takes it back. Controls: a
    // letter is refused in words and the scene is as it was (no journal line, no step); so is a
    // length a scene cannot have (the engine's refusal, the console opened).
    let fresh = || {
        let mut rig = Rig::new();
        rig.shell.run("load point").unwrap();
        rig.frames(vec![vec![], vec![]]);
        rig
    };
    let mut rig = fresh();
    let (depth, journal) = (rig.shell.engine.depth(), rig.shell.engine.journal().len());
    assert_eq!(transport(&rig, "duration").text, "12.00");
    let box_ = transport(&rig, "duration").rect;
    let time = transport(&rig, "time").rect;
    assert!(box_.left() > time.right() && (box_.width() - time.width()).abs() < 0.01, "`t [..] / [..] s`");
    type_at_transport(&mut rig, "duration", "20");
    assert_eq!(rig.shell.engine.spec().duration, 20.0);
    assert_eq!(transport(&rig, "duration").text, "20.00");
    assert_eq!(rig.shell.engine.depth(), (depth.0 + 1, 0), "one step");
    assert_eq!(rig.shell.engine.journal().len(), journal + 1, "one line");
    let typed = rig.shell.engine.journal().last().unwrap().clone();

    // The Scene card's own editor, with nothing selected: its box typed into (a double click
    // makes a dragged number a typed one), 20, Enter.
    let mut card = fresh();
    let editor = card.shell.layout().editor("duration").expect("the Scene card's Duration").center();
    card.frames(vec![
        vec![Event::PointerMoved(editor)],
        vec![Rig::press(editor, PointerButton::Primary, true)],
        vec![Rig::press(editor, PointerButton::Primary, false)],
        vec![Rig::press(editor, PointerButton::Primary, true)],
        vec![Rig::press(editor, PointerButton::Primary, false)],
        vec![],
    ]);
    card.frames(vec![vec![Event::Key {
        key: Key::A,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    }]]);
    card.typed("20");
    card.key(Key::Enter, Modifiers::NONE);
    card.frames(vec![vec![]]);
    assert_eq!(card.shell.engine.spec().duration, 20.0, "the card's editor set it");
    let from_card = card.shell.engine.journal().last().unwrap().clone();
    assert_eq!(typed, from_card, "the same command, to the value's type");
    assert_eq!(typed, cav_app::editors::command_for("duration", serde_json::json!(20.0), false));
    assert_eq!(cav_engine::describe(&typed, 0), cav_engine::describe(&from_card, 0), "the same line");
    assert_eq!(rig.shell.engine.spec(), card.shell.engine.spec());
    assert_eq!(rig.shell.engine.digest(), card.shell.engine.digest());
    // One undo takes it back, and the box says so.
    rig.shell.run("undo").unwrap();
    rig.frames(vec![vec![]]);
    assert_eq!((rig.shell.engine.spec().duration, rig.shell.engine.depth()), (12.0, (depth.0, 1)));
    assert_eq!(transport(&rig, "duration").text, "12.00");

    // Control: a letter is no duration; nothing of the scene changes, and the log says why.
    let mut rig = fresh();
    let (digest, depth, journal) =
        (rig.shell.engine.digest(), rig.shell.engine.depth(), rig.shell.engine.journal().len());
    let said = |rig: &Rig| rig.shell.engine.log().iter().next_back().unwrap().message.clone();
    assert!(!rig.shell.console_open);
    type_at_transport(&mut rig, "duration", "long");
    assert!(said(&rig).contains("`long` is not a duration in seconds"), "{}", said(&rig));
    assert!(rig.shell.console_open, "the refusal is shown");
    // And a length no scene has is the engine's to refuse, as it refuses the card's.
    for wrong in ["-5", "0", "2000000"] {
        rig.shell.console_open = false;
        type_at_transport(&mut rig, "duration", wrong);
        assert!(said(&rig).contains("refused") && said(&rig).contains("duration"), "{wrong}: {}", said(&rig));
        assert!(rig.shell.console_open, "{wrong}");
    }
    assert_eq!(
        (rig.shell.engine.digest(), rig.shell.engine.depth(), rig.shell.engine.journal().len()),
        (digest, depth, journal),
        "the scene, the history and the journal are as they were"
    );
    assert_eq!(transport(&rig, "duration").text, "12.00");
    // A duration shorter than where the playhead is brings the playhead in with it.
    rig.shell.run("seek 10").unwrap();
    type_at_transport(&mut rig, "duration", "5");
    assert_eq!((rig.shell.engine.spec().duration, rig.shell.engine.time()), (5.0, 5.0));
    assert_eq!(
        (transport(&rig, "time").text.as_str(), transport(&rig, "duration").text.as_str()),
        ("5.00", "5.00")
    );
}

#[test]
fn the_end_loops_stops_or_runs_on() {
    // T5. Criterion (written before measuring): at the end of the timeline, playing with
    // `loop` wraps to 0 and goes on; with `stop` leaves t = duration (bits) and playing false;
    // with `run on` the duration grows by its length at the start of the run each time the end
    // is reached (12 → 24 → 36, not doubling), playing goes on, the whole run is one step of
    // undo live and replayed (the seeks between do not end it), and at the schema's longest
    // timeline playing stops with a message. Controls: with `stop` and with `loop` the duration
    // never changes and nothing is journalled but seeks; a second run is a step of its own.
    let fresh = |at_end: &str| {
        let mut rig = Rig::new();
        rig.shell.run("load point").unwrap();
        rig.shell.run(&format!("end {at_end}")).unwrap();
        rig.frames(vec![vec![], vec![]]);
        rig
    };
    let near_end = |rig: &mut Rig| {
        // The signal of the scene as it stands is computed first: a run that finds the engine
        // behind grows by more (`transport::tests::a_run_on_grows_by_whole_blocks_…`), and
        // these frames take no real time.
        assert!(rig.shell.engine.settle(std::time::Duration::from_secs(30)));
        let end = rig.shell.engine.spec().duration;
        rig.shell.run(&format!("seek {}", end - 0.01)).unwrap();
        rig.shell.playing = true;
        rig.frames(vec![vec![]; 3]);
    };
    let edits = |rig: &Rig| {
        rig.shell
            .engine
            .journal()
            .iter()
            .filter(|c| !matches!(c, Command::Seek(_) | Command::Load(_)))
            .count()
    };

    // Loop (how it starts): back to the start, still playing.
    let mut rig = Rig::new();
    assert_eq!(rig.shell.at_end, cav_app::transport::AtEnd::Loop, "a window starts looping");
    rig.shell.run("load point").unwrap();
    let depth = rig.shell.engine.depth();
    near_end(&mut rig);
    assert!(rig.shell.playing && rig.shell.engine.time() < 0.1, "wrapped: {}", rig.shell.engine.time());
    assert_eq!((rig.shell.engine.spec().duration, edits(&rig), rig.shell.engine.depth()), (12.0, 0, depth));

    // Stop: exactly the end, and playing stops there.
    let mut rig = fresh("stop");
    near_end(&mut rig);
    assert_eq!(rig.shell.engine.time().to_bits(), 12.0f64.to_bits());
    assert!(!rig.shell.playing);
    rig.frames(vec![vec![]; 3]);
    assert_eq!(rig.shell.engine.time(), 12.0, "and stays");
    assert_eq!(
        (rig.shell.engine.spec().duration, edits(&rig), rig.shell.engine.depth()),
        (12.0, 0, depth),
        "control: stopping changes no duration"
    );
    // Play from the end, stopped there: it stops again at once rather than run past it.
    rig.shell.run("play").unwrap();
    rig.frames(vec![vec![]; 2]);
    assert!(!rig.shell.playing && rig.shell.engine.time() == 12.0);

    // Run on: the duration grows by the length it had when the run began.
    let mut rig = fresh("run-on");
    near_end(&mut rig);
    assert_eq!(rig.shell.engine.spec().duration.to_bits(), 24.0f64.to_bits(), "12 and its own length");
    let time = rig.shell.engine.time();
    assert!(rig.shell.playing && time > 12.0 && time < 12.1, "the playhead ran on: {time}");
    assert_eq!(rig.shell.engine.depth(), (depth.0 + 1, 0));
    rig.frames(vec![vec![]; 30]);
    assert!(rig.shell.engine.time() > time + 0.4 && rig.shell.engine.spec().duration == 24.0);
    near_end(&mut rig);
    assert_eq!(rig.shell.engine.spec().duration, 36.0, "by the starting length again, not doubled");
    near_end(&mut rig);
    assert_eq!(rig.shell.engine.spec().duration, 48.0);
    assert_eq!(rig.shell.engine.depth(), (depth.0 + 1, 0), "one step for the whole run, seeks between");
    // The journal replays to the same scene and the same one step.
    let replayed = cav_engine::Engine::replay(rig.shell.engine.journal()).unwrap();
    assert_eq!((replayed.spec().duration, replayed.depth()), (48.0, (depth.0 + 1, 0)));
    // Control: a second run (paused, then played again) is its own step, by its own length.
    rig.shell.run("pause").unwrap();
    rig.frames(vec![vec![]]);
    near_end(&mut rig);
    assert_eq!(rig.shell.engine.spec().duration, 96.0, "48 and the length this run began with");
    assert_eq!(rig.shell.engine.depth(), (depth.0 + 2, 0));
    // One undo a run.
    rig.shell.run("pause").unwrap();
    rig.shell.run("undo").unwrap();
    assert_eq!(rig.shell.engine.spec().duration, 48.0);
    rig.shell.run("undo").unwrap();
    assert_eq!(rig.shell.engine.spec().duration, 12.0, "one undo takes the whole first run back");
    assert_eq!(rig.shell.engine.depth().0, depth.0);

    // A frame's step longer than the starting length (0.1 s at 10×): whole blocks, enough to
    // hold the playhead; still one step.
    let mut rig = fresh("run-on");
    rig.shell.run("set duration 0.1").unwrap();
    rig.shell.run("speed 10").unwrap();
    let depth = rig.shell.engine.depth();
    rig.shell.playing = true;
    rig.frames(vec![vec![]; 6]);
    let (time, duration) = (rig.shell.engine.time(), rig.shell.engine.spec().duration);
    assert!((time - 1.0).abs() < 1e-6, "six frames at 10×: {time}");
    assert!(duration >= time, "{duration} holds {time}");
    assert!(((duration / 0.1).round() * 0.1 - duration).abs() < 1e-9, "whole blocks: {duration}");
    assert_eq!(rig.shell.engine.depth(), (depth.0 + 1, 0));

    // The longest timeline: the run goes up to it, then stops there and says so.
    let limit = cav_sim::spec::limits::DURATION;
    let mut rig = fresh("run-on");
    rig.shell.run("set array.instrument.cycle 10000").unwrap();
    rig.shell.run("set duration 600000").unwrap();
    near_end(&mut rig);
    assert_eq!(rig.shell.engine.spec().duration, limit, "600 000 and its length would pass the limit");
    assert!(rig.shell.playing);
    near_end(&mut rig);
    assert_eq!((rig.shell.engine.time(), rig.shell.engine.spec().duration), (limit, limit));
    assert!(!rig.shell.playing, "at the limit playing stops");
    let log: Vec<String> = rig.shell.engine.log().iter().map(|e| e.message.clone()).collect();
    assert!(log.iter().any(|m| m.contains("run on stopped at 1000000 s")), "{log:?}");
    rig.frames(vec![vec![]]);
    assert!(rig.shell.layout().status_part("warnings").is_some(), "and the status bar counts it");

    // The behaviour is chosen at the transport: its button opens the list, a row chooses.
    let mut rig = fresh("loop");
    let journal = rig.shell.engine.journal().len();
    let button = transport(&rig, "at-end");
    assert!(button.tooltip.starts_with("At the end: Loop") && button.text == "loop", "{}", button.tooltip);
    assert!(button.rect.left() >= transport(&rig, "end").rect.right(), "after the end button");
    assert!(button.rect.right() <= transport(&rig, "speed").rect.left());
    rig.click(button.rect.center());
    rig.frames(vec![vec![]]);
    let list = rig.shell.layout().button("at-end menu").expect("the list is open");
    for name in ["loop", "stop", "run-on"] {
        assert!(list.contains_rect(rig.shell.layout().button(&format!("end {name}")).unwrap()), "{name}");
    }
    rig.click_button("end run-on");
    rig.frames(vec![vec![]]);
    assert_eq!(rig.shell.at_end, cav_app::transport::AtEnd::RunOn);
    assert!(rig.shell.layout().button("at-end menu").is_none(), "choosing closes it");
    assert!(transport(&rig, "at-end").tooltip.starts_with("At the end: Run on"));
    // Its console line opens it too (for a picture); Esc closes it.
    rig.shell.run("end menu").unwrap();
    rig.frames(vec![vec![]]);
    assert!(rig.shell.layout().button("at-end menu").is_some());
    rig.shell.run("end menu").unwrap();
    rig.frames(vec![vec![]]);
    assert!(rig.shell.layout().button("at-end menu").is_none());
    // From the keyboard: Enter opens it on the one in force, ↑ and Enter choose the one before.
    rig.shell.keyboard.focus = vec!["timeline".into(), "timeline/at-end".into()];
    rig.shell.keyboard.nav_on = true;
    rig.key(Key::Enter, Modifiers::NONE);
    rig.key(Key::ArrowUp, Modifiers::NONE);
    rig.key(Key::Enter, Modifiers::NONE);
    rig.frames(vec![vec![]]);
    assert_eq!(rig.shell.at_end, cav_app::transport::AtEnd::Stop);
    assert!(rig.shell.run("end sideways").unwrap_err().contains("end <loop|stop|run-on>"));
    assert_eq!(rig.shell.engine.journal().len(), journal, "the behaviour is the window's: no journal line");
}

/// The row the tree last drew for the folder `label`.
fn folder_row(rig: &Rig, label: &str) -> cav_app::outliner::FolderRow {
    let rows = &rig.shell.layout().tree_folders;
    rows.iter().rev().find(|f| f.label == label).cloned().unwrap_or_else(|| panic!("no row for `{label}`"))
}

/// The folders the tree last drew, each once, in the order drawn: label and depth.
fn folder_rows(shell: &Shell) -> Vec<(String, usize)> {
    let mut rows: Vec<(String, usize)> = Vec::new();
    for row in &shell.layout().tree_folders {
        if !rows.iter().any(|(label, _)| *label == row.label) {
            rows.push((row.label.clone(), row.depth));
        }
    }
    rows
}

/// Presses on `from`, moves to `to` and stays there with the button down: a drag not yet let go.
fn drag_over(rig: &mut Rig, from: egui::Pos2, to: egui::Pos2) {
    rig.frames(vec![
        vec![Event::PointerMoved(from)],
        vec![Rig::press(from, PointerButton::Primary, true)],
        vec![Event::PointerMoved(from + (to - from) * 0.5)],
        vec![Event::PointerMoved(to)],
    ]);
}

#[test]
fn folders_end_to_end() {
    // F2 (`docs/slices/S20.md` §4), by input events. Criterion, written before measuring: a
    // folder is made from the `+` menu and named in place; another is nested in it from its
    // right-click menu; a body's row dragged onto a folder moves it in (one edit a drag); a drop
    // that would be refused (a tower onto a folder of bodies) shows no-drop and changes nothing;
    // folding hides its rows and is no edit and no journal line; the folder's eye hides every
    // body under it (one `hide` a body: view commands, no edit); the scene saved and opened
    // again has the same folders and draws the same tree; each step is one undo, back to the
    // start. Control: removing a folder keeps its bodies.
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("mass 5 at 1 1 1").unwrap();
    rig.shell.run("mass 6 at -1 1 1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let start = rig.shell.engine.spec().clone();
    let (depth, _) = rig.shell.engine.depth();
    let mut scenes = vec![start.clone()];
    // Each edit: exactly one more step of undo, and the scene after it kept.
    let edited = |rig: &Rig, scenes: &mut Vec<cav_sim::SceneSpec>, what: &str| {
        assert_eq!(rig.shell.engine.depth(), (depth + scenes.len(), 0), "{what}: one edit");
        scenes.push(rig.shell.engine.spec().clone());
    };

    // The `+` menu's New folder: a folder in the Bodies, its name typed in place.
    rig.click_button("add");
    rig.click_button("new-folder");
    edited(&rig, &mut scenes, "new folder");
    assert_eq!(rig.shell.renaming_folder.as_ref().map(|(f, _)| f.as_str()), Some("folder"));
    rig.key(Key::A, Modifiers::COMMAND);
    rig.typed("rig");
    rig.key(Key::Enter, Modifiers::NONE);
    edited(&rig, &mut scenes, "its name");
    let spec = rig.shell.engine.spec();
    assert_eq!(spec.folders.len(), 1);
    assert_eq!(
        (spec.folders[0].label.as_str(), spec.folders[0].group),
        ("rig", cav_sim::FolderGroup::Bodies)
    );
    assert_eq!(rig.shell.tree_focus.as_deref(), Some("rig"), "the light followed the name");

    // Nested: New folder from its right-click menu, named `inner`.
    rig.frames(vec![vec![]]);
    rig.click_with(folder_row(&rig, "rig").rect.center(), PointerButton::Secondary);
    rig.click_button("folder menu new");
    edited(&rig, &mut scenes, "a folder inside");
    rig.key(Key::A, Modifiers::COMMAND);
    rig.typed("inner");
    rig.key(Key::Enter, Modifiers::NONE);
    edited(&rig, &mut scenes, "its name");
    assert_eq!(rig.shell.engine.spec().folder("inner").unwrap().parent.as_deref(), Some("rig"));
    rig.frames(vec![vec![]]);
    assert_eq!(folder_rows(&rig.shell), [("rig".to_owned(), 2), ("inner".to_owned(), 3)]);

    // A body's row dragged onto a folder: `folder move`, one journal line, one edit.
    let journal = rig.shell.engine.journal().len();
    let from = rig.row(Target::Body(2)).center();
    rig.drag(from, folder_row(&rig, "inner").rect.center());
    edited(&rig, &mut scenes, "a body dragged in");
    assert_eq!(rig.shell.engine.spec().bodies[1].folder.as_deref(), Some("inner"));
    assert_eq!(rig.shell.engine.journal().len(), journal + 1);
    assert_eq!(
        cav_engine::describe(rig.shell.engine.journal().last().unwrap(), 0),
        "folder move body 2 inner"
    );
    rig.frames(vec![vec![]]);
    let from = rig.row(Target::Body(3)).center();
    rig.drag(from, folder_row(&rig, "rig").rect.center());
    edited(&rig, &mut scenes, "another");
    assert_eq!(rig.shell.engine.spec().bodies[2].folder.as_deref(), Some("rig"));
    rig.frames(vec![vec![]]);
    // The tree's order is the data's: a folder's folders, then its bodies; then the unfiled.
    let drawn: Vec<Target> = rig.shell.layout().outline.iter().map(|(t, _)| *t).collect();
    let bodies: Vec<Target> = drawn.iter().copied().filter(|t| matches!(t, Target::Body(_))).collect();
    assert_eq!(bodies[..3], [Target::Body(2), Target::Body(3), Target::Body(1)], "{drawn:?}");
    // Its rows are stops of the keyboard's tree, under the folder, in that order.
    let tree = rig.shell.focus_tree();
    let stops = tree.children("tree/sources");
    let rows: Vec<&str> = stops.iter().copied().filter(|id| !id.ends_with("/eye")).collect();
    let rows: Vec<&str> = rows.into_iter().filter(|id| !id.ends_with("/lock")).collect();
    assert_eq!(rows, ["tree/sources/add", "tree/folder/rig", "tree/row/body 1"], "{stops:?}");
    let inside = tree.children("tree/folder/rig");
    assert_eq!(inside[0], "tree/folder/inner", "{inside:?}");
    assert!(inside.contains(&"tree/row/body 3"), "{inside:?}");
    assert_eq!(tree.children("tree/folder/inner")[0], "tree/row/body 2");

    // Control: a tower onto a folder of bodies is refused: no-drop while over it, nothing done.
    let (journal, scene) = (rig.shell.engine.journal().len(), rig.shell.engine.spec().clone());
    let (from, to) = (rig.row(Target::Tower(1)).center(), folder_row(&rig, "rig").rect.center());
    drag_over(&mut rig, from, to);
    assert_eq!(rig.shell.layout().tree_drop, Some(("rig".to_owned(), false)), "no drop here");
    rig.frames(vec![vec![Rig::press(to, PointerButton::Primary, false)], vec![]]);
    assert_eq!((rig.shell.engine.spec(), rig.shell.engine.journal().len()), (&scene, journal));
    assert_eq!(rig.shell.engine.depth(), (depth + scenes.len() - 1, 0));
    // And a body over a folder it is not in is offered the drop (the same gesture, lit).
    let (from, to) = (rig.row(Target::Body(1)).center(), folder_row(&rig, "inner").rect.center());
    drag_over(&mut rig, from, to);
    assert_eq!(rig.shell.layout().tree_drop, Some(("inner".to_owned(), true)));
    // (Let go off the tree: no drop.)
    let away = rig.shell.layout().canvases[0].center();
    rig.frames(vec![
        vec![Event::PointerMoved(away)],
        vec![Rig::press(away, PointerButton::Primary, false)],
        vec![],
    ]);
    assert_eq!(rig.shell.engine.spec(), &scene);

    // Folding: the rows under it are not laid out; window state, no edit, no journal line.
    let journal = rig.shell.engine.journal().len();
    rig.click_button("fold folder rig");
    rig.frames(vec![vec![]]);
    assert!(!folder_row(&rig, "rig").open);
    assert_eq!(folder_rows(&rig.shell), [("rig".to_owned(), 2)]);
    let rows: Vec<Target> = rig.shell.layout().outline.iter().map(|(t, _)| *t).collect();
    assert!(!rows.contains(&Target::Body(2)) && rows.contains(&Target::Body(1)), "{rows:?}");
    assert_eq!((rig.shell.engine.spec(), rig.shell.engine.journal().len()), (&scene, journal));
    let tree = rig.shell.focus_tree();
    assert!(tree.node("tree/folder/inner").is_none(), "a folded folder's rows are no stops");
    assert!(tree.node("tree/folder/rig").is_some(), "the folder itself is one");
    rig.click_button("fold folder rig");
    rig.frames(vec![vec![]]);
    assert!(folder_row(&rig, "rig").open);

    // The eye: every body under it hidden, one `hide` each; no edit.
    use cav_app::outliner::Eye;
    assert_eq!(folder_row(&rig, "rig").eye, Some(Eye::Shown));
    let journal = rig.shell.engine.journal().len();
    rig.click_button("eye folder rig");
    rig.frames(vec![vec![]]);
    let hidden = rig.shell.engine.view(0).hidden.clone();
    assert_eq!(hidden.len(), 2, "{hidden:?}");
    assert!(hidden.contains(&scene.bodies[1].label) && hidden.contains(&scene.bodies[2].label));
    assert_eq!(rig.shell.engine.journal().len(), journal + 2, "a `hide` a body");
    assert_eq!((rig.shell.engine.spec(), rig.shell.engine.depth().0), (&scene, depth + scenes.len() - 1));
    assert_eq!(folder_row(&rig, "rig").eye, Some(Eye::Hidden));
    assert_eq!(folder_row(&rig, "inner").eye, Some(Eye::Hidden));
    // One of them shown by its own eye: the folder says mixed, and a click hides the rest.
    rig.shell.run(&format!("unhide \"{}\"", scene.bodies[2].label)).unwrap();
    rig.frames(vec![vec![]]);
    assert_eq!(folder_row(&rig, "rig").eye, Some(Eye::Mixed));
    rig.click_button("eye folder rig");
    rig.frames(vec![vec![]]);
    assert_eq!(folder_row(&rig, "rig").eye, Some(Eye::Hidden));
    // The lock: every body under it, window state.
    rig.click_button("lock folder rig");
    rig.frames(vec![vec![]]);
    assert!(rig.shell.is_locked(1) && rig.shell.is_locked(2) && !rig.shell.is_locked(0));
    assert!(folder_row(&rig, "rig").locked);

    // The filter keeps a folder whose name matches (and what it holds), and one above a match.
    rig.shell.tree_filter = "inn".into();
    rig.frames(vec![vec![], vec![]]);
    assert_eq!(folder_rows(&rig.shell), [("rig".to_owned(), 2), ("inner".to_owned(), 3)]);
    let rows: Vec<Target> = rig.shell.layout().outline.iter().map(|(t, _)| *t).collect();
    assert!(rows.contains(&Target::Body(2)) && !rows.contains(&Target::Body(3)), "{rows:?}");
    rig.shell.tree_filter = "tower".into();
    rig.frames(vec![vec![], vec![]]);
    assert!(folder_rows(&rig.shell).is_empty(), "control: no folder matches");
    rig.shell.tree_filter.clear();
    rig.frames(vec![vec![], vec![]]);

    // Saved and opened again: the same folders, the same tree.
    let file = std::env::temp_dir().join(format!("cavendish-folders-{}.ron", std::process::id()));
    let path = file.to_string_lossy().replace('\\', "/");
    rig.shell.run(&format!("save \"{path}\"")).unwrap();
    let mut again = Rig::new();
    again.shell.run(&format!("load \"{path}\"")).unwrap();
    again.frames(vec![vec![], vec![]]);
    let _ = std::fs::remove_file(&file);
    assert_eq!(again.shell.engine.spec(), &scene);
    assert_eq!(folder_rows(&again.shell), folder_rows(&rig.shell));
    let outline = |shell: &Shell| -> Vec<(Target, String)> {
        shell.layout().outline_words.iter().map(|(target, label, _)| (*target, label.clone())).collect()
    };
    assert_eq!(outline(&again.shell), outline(&rig.shell), "the same rows, in the same order");

    // Control: removing a folder (its menu) keeps its bodies: they move up to `rig`.
    rig.click_with(folder_row(&rig, "inner").rect.center(), PointerButton::Secondary);
    rig.click_button("folder menu remove");
    edited(&rig, &mut scenes, "the folder removed");
    let spec = rig.shell.engine.spec();
    assert_eq!(spec.bodies.len(), start.bodies.len(), "its bodies are kept");
    assert!(spec.folder("inner").is_none());
    assert_eq!(spec.bodies[1].folder.as_deref(), Some("rig"));

    // One undo a step, back to the start.
    assert_eq!(scenes.len(), 8);
    for n in (0..scenes.len() - 1).rev() {
        rig.shell.run("undo").unwrap();
        assert_eq!(rig.shell.engine.spec(), &scenes[n], "undo {n}");
    }
    assert_eq!(rig.shell.engine.spec(), &start);
    assert_eq!(rig.shell.engine.depth().0, depth);
    rig.frames(vec![vec![]]);
    assert!(rig.shell.layout().tree_folders.is_empty() && rig.shell.tree_focus.is_none());
}

// ---- The first-person fly camera (TASK-S20-10): C3 and C4 ----

/// A frame's event that says `modifiers` are held and does nothing else (a window reports the
/// modifiers with every frame; here they ride on a key's repeat, which no one acts on).
fn holding(modifiers: Modifiers) -> Event {
    Event::Key { key: Key::F20, physical_key: None, pressed: true, repeat: true, modifiers }
}

fn key_event(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: None, pressed, repeat: false, modifiers }
}

/// Holds `keys` with `modifiers` for `frames` frames, lets them go, and lets the camera come to
/// rest. Returns how far viewport 0's eye moved, metres in the world's frame.
fn fly_holding(rig: &mut Rig, keys: &[Key], modifiers: Modifiers, frames: usize) -> [f64; 3] {
    let eye = |rig: &Rig| cav_app::camera::eye(rig.shell.engine.view(0));
    let start = eye(rig);
    let mut list = vec![keys.iter().map(|key| key_event(*key, true, modifiers)).collect::<Vec<_>>()];
    list[0].push(holding(modifiers));
    list.extend((1..frames).map(|_| vec![holding(modifiers)]));
    list.push(keys.iter().map(|key| key_event(*key, false, Modifiers::NONE)).collect());
    rig.frames(list);
    rig.settle();
    let end = eye(rig);
    [end[0] - start[0], end[1] - start[1], end[2] - start[2]]
}

fn wheel(points: f32) -> Event {
    Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, points),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: Modifiers::NONE,
    }
}

fn grabs(asked: &[egui::ViewportCommand]) -> Vec<&egui::ViewportCommand> {
    asked
        .iter()
        .filter(|c| {
            matches!(c, egui::ViewportCommand::CursorGrab(_) | egui::ViewportCommand::CursorVisible(_))
        })
        .collect()
}

#[test]
fn flying_takes_the_keys_like_a_game() {
    // C3. Criterion (written before measuring): with the right button held over the scene,
    // Space rises and Shift sinks (straight along z); Ctrl and a double tap of W multiply the
    // speed by the sprint factor; the wheel changes `fly_speed` and the canvas shows it for
    // 1.5 s; W, A, Q, 2, Delete, Space and Ctrl+S run no action; the release or Esc ends it and
    // asks for the pointer back. Control: with the button up, W takes the Move tool.
    use egui::ViewportCommand::{CursorGrab, CursorVisible};
    let settings = cav_app::camera::Settings::from_data();
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.shell.run("tool select").unwrap();
    rig.shell.run("select body 1").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let pane = rig.shell.layout().canvases[0];
    let at = pane.center() + vec2(0.0, -0.3 * pane.height());
    // The control first: not flying, W is the Move tool's key.
    rig.shell.keyboard.focus = vec!["viewport".into()];
    rig.key(Key::W, Modifiers::NONE);
    assert_eq!(rig.shell.tool, "move", "control: with the button up, W takes the Move tool");
    rig.shell.run("tool select").unwrap();
    let start = rig.shell.engine.view(0).clone();
    rig.frames(vec![vec![Event::PointerMoved(at)], vec![Rig::press(at, PointerButton::Secondary, true)]]);
    assert_eq!(rig.shell.fly_pane(), Some(0));

    // Space rises, Shift sinks: straight up and down, the same distance.
    let up = fly_holding(&mut rig, &[Key::Space], Modifiers::NONE, 12);
    assert!(up[2] > 0.3 && up[0].abs() < 1e-9 && up[1].abs() < 1e-9, "Space rises: {up:?}");
    assert!(!rig.shell.playing, "Space did not play");
    let down = fly_holding(&mut rig, &[], Modifiers::SHIFT, 12);
    assert!((down[2] + up[2]).abs() < 1e-4 && down[0].abs() < 1e-9, "Shift sinks as far: {down:?}");
    // W flies level whatever the pitch; Ctrl sprints by the factor, exactly.
    let level = |d: [f64; 3]| d[0].hypot(d[1]);
    let plain = fly_holding(&mut rig, &[Key::W], Modifiers::NONE, 12);
    assert!(level(plain) > 0.3 && plain[2].abs() < 1e-9, "W flies level: {plain:?}");
    assert!(start.pitch.abs() > 10.0, "though the view looks down ({})", start.pitch);
    let fast = fly_holding(&mut rig, &[Key::W], Modifiers::CTRL, 12);
    // (To 1e-4: the glide's last 0.1 mm/s is cut off as rest, a few micrometres either way.)
    assert!((level(fast) / level(plain) - settings.sprint).abs() < 1e-4, "{}", level(fast) / level(plain));
    // A double tap of W sprints until W is let go; two presses half a second apart do not.
    let tap = |rig: &mut Rig, gap: usize| {
        let mut frames = vec![
            vec![key_event(Key::W, true, Modifiers::NONE)],
            vec![key_event(Key::W, false, Modifiers::NONE)],
        ];
        frames.extend(vec![vec![]; gap]);
        frames.push(vec![key_event(Key::W, true, Modifiers::NONE)]);
        frames.extend(vec![vec![]; 5]);
        rig.frames(frames);
        let sprinting = rig.shell.flight.sprinting;
        rig.frames(vec![vec![key_event(Key::W, false, Modifiers::NONE)], vec![]]);
        assert!(!rig.shell.flight.sprinting, "letting W go ends the sprint");
        sprinting
    };
    assert!(tap(&mut rig, 4), "two presses of W 0.1 s apart sprint");
    assert!(!tap(&mut rig, 40), "control: two presses 0.7 s apart do not");

    // The wheel changes the speed, bounded, and the canvas shows it for 1.5 s; it zooms nothing.
    let (speed, distance) = (rig.shell.fly_speed, rig.shell.engine.view(0).distance);
    assert!((speed - settings.fly_speed).abs() < 1e-12);
    assert!(rig.shell.layout().button("fly speed").is_none(), "nothing shown before the wheel");
    rig.frames(vec![vec![wheel(120.0)], vec![], vec![], vec![], vec![], vec![]]);
    let faster = rig.shell.fly_speed;
    assert!(faster > speed * 1.1 && faster < speed * 2.0, "the wheel raised the speed: {faster}");
    assert_eq!(rig.shell.engine.view(0).distance, distance, "and zoomed nothing");
    let shown = rig.shell.layout().button("fly speed").expect("the speed is shown");
    assert!(pane.contains_rect(shown), "in the canvas: {shown:?}");
    assert_eq!(rig.shell.fly_speed_shown(0, rig.clock), Some(format!("{faster:.1} m/s")));
    rig.frames(vec![vec![]; 80]);
    assert!(rig.shell.layout().button("fly speed").is_some(), "still shown 1.4 s after");
    rig.frames(vec![vec![]; 20]);
    assert!(rig.shell.layout().button("fly speed").is_none(), "gone after 1.5 s");
    rig.frames(vec![vec![wheel(-1.0e6)], vec![], vec![], vec![], vec![], vec![], vec![], vec![]]);
    assert!((rig.shell.fly_speed - 0.05).abs() < 1e-12, "bounded below: {}", rig.shell.fly_speed);
    rig.frames(vec![vec![wheel(1.0e6)], vec![], vec![], vec![], vec![], vec![], vec![], vec![]]);
    assert!((rig.shell.fly_speed - 1000.0).abs() < 1e-9, "and above: {}", rig.shell.fly_speed);
    rig.shell.run("camera-set fly-speed 4").unwrap();

    // No tool, scope, selection, transport or file key fires.
    let (journal, depth) = (rig.shell.engine.journal().len(), rig.shell.engine.depth());
    for (key, modifiers) in [
        (Key::W, Modifiers::NONE),
        (Key::A, Modifiers::NONE),
        (Key::Q, Modifiers::NONE),
        (Key::Num2, Modifiers::NONE),
        (Key::Delete, Modifiers::NONE),
        (Key::Space, Modifiers::NONE),
        (Key::S, Modifiers::CTRL),
        (Key::Z, Modifiers::CTRL),
        (Key::Space, Modifiers::CTRL),
        (Key::Backtick, Modifiers::NONE),
    ] {
        rig.key(key, modifiers);
    }
    assert_eq!(rig.shell.tool, "select", "W, A and Q took no tool");
    assert_eq!(rig.shell.engine.selection(), Some(Target::Body(1)), "2 selected no tower");
    assert_eq!(rig.shell.engine.spec().bodies.len(), 1, "Delete deleted nothing");
    assert!(!rig.shell.playing && !rig.shell.maximised && !rig.shell.console_open, "Space, Ctrl+Space, `");
    assert!(rig.shell.save_window.is_none(), "Ctrl+S saved nothing");
    assert_eq!(rig.shell.engine.depth(), depth, "no edit, no undo");
    // (The journal holds camera moves only: W, A, Space moved the camera.)
    assert!(
        rig.shell.engine.journal()[journal..]
            .iter()
            .all(|c| matches!(c, Command::View(ViewCommand::Camera { .. }))),
        "only camera moves were journalled"
    );
    // F3 is one of the few that still act (`first_person::WHILE_FLYING`).
    rig.key(Key::F3, Modifiers::NONE);
    assert!(rig.shell.gui_hidden, "F3 hides the panels in flight too");
    rig.key(Key::F3, Modifiers::NONE);

    // The release ends it and asks for the pointer back.
    let asked =
        rig.frames_asking(vec![vec![Rig::press(at, PointerButton::Secondary, false)], vec![], vec![]]);
    assert!(rig.shell.fly_pane().is_none(), "the release ends it");
    assert_eq!(grabs(&asked), [&CursorGrab(egui::CursorGrab::None), &CursorVisible(true)]);
    // So does Esc, with the button still held; the press must be made again to fly again.
    rig.frames(vec![vec![Rig::press(at, PointerButton::Secondary, true)], vec![]]);
    assert!(rig.shell.fly_pane().is_some());
    let asked = rig.frames_asking(vec![vec![key_event(Key::Escape, true, Modifiers::NONE)], vec![], vec![]]);
    assert!(rig.shell.fly_pane().is_none(), "Esc ends it");
    assert_eq!(grabs(&asked), [&CursorGrab(egui::CursorGrab::None), &CursorVisible(true)]);
    assert_eq!(rig.shell.engine.selection(), Some(Target::Body(1)), "that Esc was the flight's alone");
    rig.frames(vec![vec![Rig::press(at, PointerButton::Secondary, false)], vec![]]);
    // Control: not flying, W is the Move tool's again, and Space plays.
    rig.shell.keyboard.focus = vec!["viewport".into()];
    rig.key(Key::W, Modifiers::NONE);
    assert_eq!(rig.shell.tool, "move");
    rig.key(Key::Space, Modifiers::NONE);
    assert!(rig.shell.playing, "control: Space plays when not flying");
}

#[test]
fn flying_captures_the_pointer() {
    // C4. Criterion (written before measuring): the frame a flight begins asks the window to
    // grab the cursor and hide it, the frame it ends to release and show it, and no frame
    // between asks anything of the cursor; the look then follows the device's raw motion, not
    // the pointer's. Control: an orbit drag asks for neither.
    use egui::ViewportCommand::{CursorGrab, CursorVisible};
    let settings = cav_app::camera::Settings::from_data();
    let mut rig = Rig::new();
    rig.shell.run("load point").unwrap();
    rig.frames(vec![vec![], vec![]]);
    let pane = rig.shell.layout().canvases[0];
    let at = pane.center() + vec2(0.0, -0.3 * pane.height());
    let taken = [
        &CursorGrab(egui::CursorGrab::Confined),
        &CursorGrab(egui::CursorGrab::Locked),
        &CursorVisible(false),
    ];
    let given = [&CursorGrab(egui::CursorGrab::None), &CursorVisible(true)];
    // Control: an orbit drag (the primary button) asks nothing of the cursor.
    let yaw = rig.shell.engine.view(0).yaw;
    let asked = rig.frames_asking(vec![
        vec![Event::PointerMoved(at)],
        vec![Rig::press(at, PointerButton::Primary, true)],
        vec![Event::PointerMoved(at + vec2(40.0, 0.0))],
        vec![Rig::press(at + vec2(40.0, 0.0), PointerButton::Primary, false)],
        vec![],
    ]);
    assert_ne!(rig.shell.engine.view(0).yaw, yaw, "the drag orbited");
    assert!(grabs(&asked).is_empty(), "control: an orbit drag grabs nothing: {asked:?}");
    // The right button held: taken on the frame of the press, kept, given back on the release.
    let asked = rig.frames_asking(vec![
        vec![Event::PointerMoved(at)],
        vec![Rig::press(at, PointerButton::Secondary, true)],
    ]);
    assert_eq!(grabs(&asked), taken, "the press takes the pointer");
    assert!(rig.shell.flight.captured);
    // Raw motion looks, a count at a time, and the pointer's own motion is not added to it.
    let before = rig.shell.engine.view(0).clone();
    let asked = rig.frames_asking(vec![
        vec![Event::MouseMoved(vec2(50.0, 0.0)), Event::PointerMoved(at + vec2(7.0, 3.0))],
        vec![Event::MouseMoved(vec2(0.0, -20.0))],
        vec![],
    ]);
    assert!(grabs(&asked).is_empty(), "nothing more is asked while flying");
    let after = rig.shell.engine.view(0).clone();
    assert!((before.yaw - after.yaw - 50.0 * settings.sensitivity).abs() < 1e-9, "50 counts right");
    assert!((before.pitch - after.pitch - 20.0 * settings.sensitivity).abs() < 1e-9, "20 counts up");
    let (a, b) = (cav_app::camera::eye(&before), cav_app::camera::eye(&after));
    assert!(a.iter().zip(b).all(|(p, q)| (p - q).abs() < 1e-9), "the eye stayed: {a:?} {b:?}");
    let asked = rig.frames_asking(vec![vec![Rig::press(at, PointerButton::Secondary, false)], vec![]]);
    assert_eq!(grabs(&asked), given, "the release gives it back");
    assert!(!rig.shell.flight.captured);
    // A click of the right button (no hold) still opens the viewport's menu, the pointer back.
    // The toggle (Shift+F) takes and gives it the same way, and the window losing its focus
    // gives it back too.
    rig.key(Key::Escape, Modifiers::NONE);
    rig.shell.keyboard.focus = vec!["viewport".into()];
    let shift_f = |pressed| key_event(Key::F, pressed, Modifiers::SHIFT);
    let asked = rig.frames_asking(vec![vec![shift_f(true)], vec![shift_f(false)]]);
    assert!(rig.shell.flying(), "Shift+F flies");
    assert_eq!(grabs(&asked), taken);
    // While it is on, a click acts on nothing under the unseen pointer (here a row of the tree).
    rig.shell.run("select none").unwrap();
    let row = rig.row(Target::Body(1));
    rig.click(row.center());
    assert_eq!(rig.shell.engine.selection(), None, "a click in flight selects nothing");
    let asked = rig.frames_asking(vec![vec![shift_f(true)], vec![shift_f(false)]]);
    assert!(!rig.shell.flying(), "Shift+F again stops");
    assert_eq!(grabs(&asked), given);
    rig.click(row.center());
    assert_eq!(rig.shell.engine.selection(), Some(Target::Body(1)), "control: not flying, the click selects");
    rig.shell.run("fly").unwrap();
    rig.frames(vec![vec![]]);
    let asked = rig.frames_asking(vec![vec![Event::WindowFocused(false)], vec![]]);
    assert!(!rig.shell.any_fly(), "the focus lost ends the flight");
    assert_eq!(grabs(&asked), given);
}
