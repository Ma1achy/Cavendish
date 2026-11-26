//! S20's editor tests (`docs/slices/S20.md` §4), each driven through the same input events a
//! window delivers, each with a control arm. Rows N1 to N5 are the density pass's (TASK-S20-12):
//! the sizes are the theme's (`docs/design/workspace.md` §0), a field fills its column, the
//! scroll bar's width is reserved, the chrome holds no sentences, a uniform body's material
//! reads "Uniform" and the Probe reads into the right panel. Rows M3 to M6 and K1 are the mosaic's
//! (TASK-S20-03): more than four viewports, the splitters, the labels' order, pop-out and dock.

use std::collections::BTreeMap;

use super::Rig;
use cav_app::headless::context;
use cav_app::{Shell, theme};
use cav_engine::{Boundary, Command, ViewCommand};
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
        // Control: the tool strip's icon buttons are larger than a row.
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
            for place in ["options", "timeline"] {
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
    let pane = rig.shell.layout().panes[0];
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
        .filter(|name| !name.starts_with("tool ") && *name != "view home")
        .collect();
    assert!(over.is_empty(), "over the figure: {over:?}");
}

// ---- The mosaic (TASK-S20-03): rows M3 to M6 and K1 ----

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
