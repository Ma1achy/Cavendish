//! S20's editor tests (`docs/slices/S20.md` §4), each driven through the same input events a
//! window delivers, each with a control arm. Rows N1 to N5 are the density pass's (TASK-S20-12):
//! the sizes are the theme's (`docs/design/workspace.md` §0), a field fills its column, the
//! scroll bar's width is reserved, the chrome holds no sentences, a uniform body's material
//! reads "Uniform" and the Probe reads into the right panel.

use std::collections::BTreeMap;

use super::Rig;
use cav_app::headless::context;
use cav_app::{Shell, theme};
use egui::{Rect, vec2};

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
