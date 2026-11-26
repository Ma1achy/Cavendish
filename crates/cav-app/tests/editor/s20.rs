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
