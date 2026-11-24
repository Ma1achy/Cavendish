//! The plots as display lists: what a signal, a periodogram and a flight diagram draw, read from
//! the list itself.

use std::time::Duration;

use cav_engine::{Engine, PaneContent};
use cav_plot::drawing::{Colour, Drawing, Item};
use cav_plot::fonts::Outlines;
use cav_plot::plots::{self, Plot, Subject};
use cav_plot::style::Style;

const LONG: Duration = Duration::from_secs(120);

fn engine(script: &str) -> Engine {
    let mut engine = Engine::new();
    engine.set_patience(LONG);
    engine.run_script(script).unwrap();
    assert!(engine.settle(LONG), "the signal of `{script}` did not arrive");
    engine
}

fn subject(content: PaneContent) -> Subject {
    Subject::new(0, content)
}

fn build(engine: &Engine, subject: &Subject, style: &Style, size: [f32; 2]) -> Plot {
    plots::build(engine, subject, style, &Outlines, size).expect("a plot")
}

/// The series of a signal plot, in the order they are drawn: each line of more than two points,
/// with its colour.
fn series(drawing: &Drawing) -> Vec<(Colour, &[[f32; 2]])> {
    drawing
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Line { points, colour, .. } if points.len() > 2 => Some((*colour, points.as_slice())),
            _ => None,
        })
        .collect()
}

/// For each series, in the order given (later ones drawn over earlier ones): the fraction of
/// the one-point columns it crosses in which at least one point of its height is under no later
/// series. A series is one or more lines.
fn seen(series: &[(Colour, Vec<&[[f32; 2]]>)], width: usize) -> Vec<f64> {
    // Each series as the stretch of heights it covers in each column.
    let bands: Vec<Vec<Option<(f32, f32)>>> = series
        .iter()
        .map(|(_, lines)| {
            let mut band: Vec<Option<(f32, f32)>> = vec![None; width];
            for pair in lines.iter().flat_map(|points| points.windows(2)) {
                let (lo, hi) = (pair[0][1].min(pair[1][1]), pair[0][1].max(pair[1][1]));
                let (from, to) = (pair[0][0].min(pair[1][0]), pair[0][0].max(pair[1][0]));
                let (first, last) = (from.floor().max(0.0) as usize, (to.floor() as usize).min(width - 1));
                for held in band.iter_mut().take(last + 1).skip(first) {
                    *held = Some(held.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
                }
            }
            band
        })
        .collect();
    (0..series.len())
        .map(|n| {
            let (mut crossed, mut clear) = (0usize, 0usize);
            for column in 0..width {
                let Some((lo, hi)) = bands[n][column] else { continue };
                crossed += 1;
                // The part of this band that sticks out above or below everything drawn later.
                let later = bands[n + 1..].iter().filter_map(|band| band[column]);
                let (top, bottom) =
                    later.fold((f32::INFINITY, f32::NEG_INFINITY), |(t, b), (a, z)| (t.min(a), b.max(z)));
                if top > bottom || top - lo >= 1.0 || hi - bottom >= 1.0 {
                    clear += 1;
                }
            }
            if crossed == 0 { 0.0 } else { clear as f64 / crossed as f64 }
        })
        .collect()
}

/// Lines with their colours gathered by colour, in the order each colour is first met.
fn gathered<'a>(
    lines: impl IntoIterator<Item = (Colour, &'a [[f32; 2]])>,
) -> Vec<(Colour, Vec<&'a [[f32; 2]]>)> {
    let mut towers: Vec<(Colour, Vec<&[[f32; 2]]>)> = Vec::new();
    for (colour, points) in lines {
        match towers.iter_mut().find(|(c, _)| *c == colour) {
            Some((_, held)) => held.push(points),
            None => towers.push((colour, vec![points])),
        }
    }
    towers
}

/// The one-point cells of `area` a line touches at its width: sampled every quarter point
/// along it, and every quarter point upright across its width.
fn touched(points: &[[f32; 2]], width: f32, area: egui::Rect) -> Vec<(usize, usize)> {
    let (w, h) = (area.width().ceil() as usize, area.height().ceil() as usize);
    let mut out = Vec::new();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let steps = ((b[0] - a[0]).hypot(b[1] - a[1]) / 0.25).ceil().max(1.0) as usize;
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let (x, y) = (a[0] + (b[0] - a[0]) * t - area.left(), a[1] + (b[1] - a[1]) * t - area.top());
            let mut across = -width / 2.0;
            while across <= width / 2.0 {
                let (column, row) = (x.floor(), (y + across).floor());
                if column >= 0.0 && row >= 0.0 && (column as usize) < w && (row as usize) < h {
                    out.push((column as usize, row as usize));
                }
                across += 0.25;
            }
        }
    }
    out
}

/// What series drawn in turn over `area` cover, a one-point cell at a time: the fraction of the
/// cells any touches, and for each series the fraction of the columns it touches in which it
/// is the last drawn in at least one cell. Each series is its lines and their width.
fn coverage(series: &[(Vec<&[[f32; 2]]>, f32)], area: egui::Rect) -> (f64, Vec<f64>) {
    let (w, h) = (area.width().ceil() as usize, area.height().ceil() as usize);
    let mut grid: Vec<Option<usize>> = vec![None; w * h];
    let mut crossed: Vec<std::collections::BTreeSet<usize>> =
        vec![std::collections::BTreeSet::new(); series.len()];
    for (n, (lines, width)) in series.iter().enumerate() {
        for line in lines {
            for (column, row) in touched(line, *width, area) {
                grid[column * h + row] = Some(n);
                crossed[n].insert(column);
            }
        }
    }
    let covered = grid.iter().filter(|cell| cell.is_some()).count() as f64 / (w * h) as f64;
    let shown = (0..series.len())
        .map(|n| {
            let on_top =
                crossed[n].iter().filter(|column| grid[**column * h..(**column + 1) * h].contains(&Some(n)));
            on_top.count() as f64 / crossed[n].len().max(1) as f64
        })
        .collect();
    (covered, shown)
}

/// The comb's signal drawn as it was before review 29: every shot joined (as `plots::runs`
/// gives them, all of 1000 shots being under its 2000 points), in the towers' own order.
fn joined_lines(engine: &Engine, plot: &Plot, style: &Style) -> Vec<(Colour, Vec<[f32; 2]>)> {
    let signal = engine.signal().1.unwrap();
    let (area, x, y) = (plot.drawn.area, plot.drawn.x, plot.drawn.y);
    let channel = cav_core::forward::Channel::Gradiometer;
    let mut out = Vec::new();
    for tower in 0..signal.towers {
        let times = signal.readouts(tower);
        for run in plots::runs(&times, signal.channel(tower, channel), signal.taken(tower)) {
            let points = run.iter().map(|q| plots::place(area, &x, &y, *q)).map(|q| [q.x, q.y]).collect();
            out.push((style.series(tower).0, points));
        }
    }
    out
}

#[test]
fn a_dense_signal_shows_every_tower() {
    // The comb over all of 2000 s: a thousand shots of five towers in 860 points, each series a
    // band. Criterion, written before measuring: drawn as the plot draws them, every tower
    // shows clear of the rest in at least a fifth of the columns it crosses; drawn in the
    // towers' own order (as S1 drew them), at least one shows in fewer than a twentieth.
    //
    // Measured: 1.0, 1.0, 0.0, 1.0, 1.0. The 0.0 is tower 4, which stands as far from the body
    // as tower 5 and reads the same signal: one line lies on the other, and no order of drawing
    // can show both. So the criterion is held for every *different* series (a tower whose
    // signal is another's, value for value, counts with it); the threshold is not moved.
    //
    // Review 29 (S2.md §7.14): a signal with more shots than points across is drawn as each
    // tower's envelope, two lines; the fifth of the columns is held to it as it was to the
    // joined lines, and `a_dense_signal_is_not_a_block` holds it to the cells it covers.
    let engine = engine("load comb");
    let style = Style::screen();
    let plot = build(&engine, &subject(PaneContent::Signal), &style, [960.0, 600.0]);
    let towers = gathered(series(&plot.drawing));
    assert_eq!(towers.len(), 5);
    for (_, lines) in &towers {
        assert!(lines.len() >= 2 && lines.len() % 2 == 0, "an upper and a lower line: {} lines", lines.len());
    }
    let width = 960;
    let fractions = seen(&towers, width);
    println!("seen, as drawn: {fractions:?}");
    let signal = engine.signal().1.unwrap().clone();
    let channel = cav_core::forward::Channel::Gradiometer;
    assert_eq!(signal.channel(3, channel), signal.channel(4, channel), "towers 4 and 5 read the same");
    let twin = |n: usize| towers.iter().enumerate().any(|(m, (_, lines))| m > n && *lines == towers[n].1);
    for (n, fraction) in fractions.iter().enumerate() {
        assert!(*fraction >= 0.2 || twin(n), "series {n} is hidden: {fractions:?}");
    }
    assert_eq!((0..5).filter(|n| twin(*n)).count(), 1, "one pair of towers coincides, no more");
    // Every series is thinner than a sparse one would be: a band of thin lines shows more.
    for item in &plot.drawing.items {
        if let Item::Line { points, width, .. } = item
            && points.len() > 2
        {
            assert!(*width < style.line, "a dense series is drawn {width} wide");
        }
    }
    // Control: the lines as S1 drew them, every shot joined, in the towers' own order.
    let old = joined_lines(&engine, &plot, &style);
    let in_order = gathered(old.iter().map(|(colour, points)| (*colour, points.as_slice())));
    let control = seen(&in_order, width);
    println!("seen, every shot joined in the towers' order: {control:?}");
    // Tower 1's band, the widest but one, lies wholly under tower 2's.
    assert!(control[0] < 0.05, "the control hides no tower: {control:?}");
    // Control: a few shots are drawn at the full width, with their dots.
    let short = build(
        &self::engine("load oscillator-zoo; set duration 60"),
        &subject(PaneContent::Signal),
        &style,
        [960.0, 600.0],
    );
    assert!(short.drawing.items.iter().any(
        |item| matches!(item, Item::Line { points, width, .. } if points.len() > 2 && *width == style.line)
    ));
    assert!(short.drawing.items.iter().any(|item| matches!(item, Item::Circle { .. })));
}

#[test]
fn a_dense_signal_is_not_a_block() {
    // Review 29 (S2.md §7.14), written before measuring: for the comb's five towers × 1000
    // shots, in the `paper-column` figure and in a 960 × 600 pane, every tower whose signal is
    // not another's shows clear of what is drawn over it in at least 15% of the columns it
    // crosses, and the series' lines together touch at most 50% of the one-point cells of the
    // axes. Control: the lines as they were drawn before (every shot joined) touch more than 60%.
    let engine = engine("load comb");
    let paper = Style::preset("paper-column").unwrap();
    let points = paper.size_mm.map(|mm| (mm / cav_plot::style::MM_PER_POINT) as f32);
    for (style, size) in [(paper, points), (Style::screen(), [960.0, 600.0])] {
        let plot = build(&engine, &subject(PaneContent::Signal), &style, size);
        let area = plot.drawn.area;
        let lines: Vec<(Colour, &[[f32; 2]], f32)> = plot
            .drawing
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Line { points, colour, width, .. } if points.len() > 2 => {
                    Some((*colour, points.as_slice(), *width))
                }
                _ => None,
            })
            .collect();
        let towers = gathered(lines.iter().map(|(colour, points, _)| (*colour, *points)));
        assert_eq!(towers.len(), 5, "{}", style.name);
        let width_of = |colour: Colour| lines.iter().find(|line| line.0 == colour).unwrap().2;
        let drawn: Vec<(Vec<&[[f32; 2]]>, f32)> =
            towers.iter().map(|(colour, held)| (held.clone(), width_of(*colour))).collect();
        let (covered, shown) = coverage(&drawn, area);
        println!("{}: the envelopes touch {covered:.3} of the cells; each shows in {shown:?}", style.name);
        let twin = |n: usize| towers.iter().enumerate().any(|(m, (_, held))| m > n && *held == towers[n].1);
        for (n, fraction) in shown.iter().enumerate() {
            assert!(*fraction >= 0.15 || twin(n), "{}: series {n} is hidden: {shown:?}", style.name);
        }
        assert!(covered <= 0.5, "{}: {covered}", style.name);
        // Control: every shot joined, at the width a dense series had before (0.6 of a line).
        let old = joined_lines(&engine, &plot, &style);
        let before: Vec<(Vec<&[[f32; 2]]>, f32)> =
            old.iter().map(|(_, points)| (vec![points.as_slice()], style.line * 0.6)).collect();
        let (covered_before, _) = coverage(&before, area);
        println!("{}: every shot joined touches {covered_before:.3}", style.name);
        // Measured: 0.650 in the figure, 0.192 in the pane. The pane's 860 points hold the 1000
        // shots nearly one a point, and joined they were never a block there: the control is
        // held where the block was, the figure (S2.md §7.14, as measured).
        if style.house {
            assert!(covered_before > 0.6, "{}: the control covers only {covered_before}", style.name);
        }
    }
}

/// The void with `towers` towers in a row and a slab under them: a scene with as many towers as
/// asked whose signal costs one read a tower.
fn row_of_towers(towers: usize) -> cav_sim::SceneSpec {
    let mut scene = cav_sim::dev::void();
    scene.array.towers =
        (0..towers).map(|n| cav_sim::TowerSpec::at(&format!("t{}", n + 1), [n as f64, 0.0, 0.0])).collect();
    scene.floor = Some(cav_sim::FloorSpec {
        height: -1.0,
        kind: cav_sim::FloorKind::Slab { thickness: 1.0 },
        density: 2400.0,
    });
    scene.duration = 60.0;
    assert_eq!(scene.validate(), Ok(()));
    scene
}

#[test]
fn a_signal_plot_draws_at_most_twelve_towers_and_says_so() {
    // Finding 10 of S2's review: a figure was bounded a series, not in series.
    assert_eq!(plots::MAX_SERIES, 12);
    // Which towers: all of a few; of many the first twelve, the selected one among them.
    assert_eq!(plots::towers_drawn(5, Some(3), 12), [0, 1, 2, 3, 4]);
    assert_eq!(plots::towers_drawn(12, None, 12).len(), 12);
    assert_eq!(plots::towers_drawn(1000, None, 12), (0..12).collect::<Vec<_>>());
    assert_eq!(plots::towers_drawn(1000, Some(4), 12), (0..12).collect::<Vec<_>>());
    let with = plots::towers_drawn(1000, Some(249), 12);
    assert_eq!(with, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 249]);
    assert_eq!(plots::towers_drawn(1000, Some(5000), 12).len(), 12, "a tower that is not there is not drawn");
    assert_eq!(plots::numbered(&with), "1–11 and 250");
    assert_eq!(plots::numbered(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]), "1–12");
    assert_eq!(plots::numbered(&[0, 2, 3, 7, 8, 9]), "1, 3, 4 and 8–10");
    assert_eq!(plots::numbered(&[6]), "7");
    // A scene of twenty towers, the seventeenth selected, in every preset and in a small pane.
    let mut engine = Engine::new();
    engine.set_patience(LONG);
    engine.apply(cav_engine::Command::load(row_of_towers(20))).unwrap();
    engine.run_line("select tower 17").unwrap();
    assert!(engine.settle(LONG));
    let mut sizes: Vec<(Style, [f32; 2])> = cav_plot::style::PRESETS
        .iter()
        .map(|name| {
            let style = Style::preset(name).unwrap();
            let points = style.size_mm.map(|mm| (mm / cav_plot::style::MM_PER_POINT) as f32);
            (style, points)
        })
        .collect();
    sizes.push((Style::screen(), [320.0, 180.0]));
    for (style, size) in &sizes {
        let plot = build(&engine, &subject(PaneContent::Signal), style, *size);
        assert_eq!(plot.described.towers, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 16], "{}", style.name);
        assert_eq!(plot.described.series.len(), 12);
        assert_eq!(plot.described.series[11].0, "t17");
        // Twelve lines of the data, and no more: each in its own tower's colour.
        let lines = series(&plot.drawing);
        assert_eq!(lines.len(), 12, "{}", style.name);
        assert!(lines.iter().any(|(colour, _)| *colour == style.series(16).0));
        // It says so under the axes, whatever the size.
        let said: Vec<&String> = plot
            .drawn
            .labels
            .iter()
            .map(|(text, _)| text)
            .filter(|text| text.contains("towers drawn"))
            .collect();
        assert_eq!(said.len(), 1, "{}: {:?}", style.name, plot.drawn.labels);
        assert!(said[0].contains("12 of 20 towers drawn"), "{}: {}", style.name, said[0]);
        let note = plot.described.marks["towers"]["note"].as_str().unwrap();
        assert!(note.starts_with("12 of 20 towers drawn: 1–11 and 17 "), "{note}");
        assert_eq!(plot.described.marks["towers"]["drawn"].as_array().unwrap().len(), 12);
    }
    // Where there is room the towers are named: a full-width figure has it.
    let (full, size) = &sizes[1];
    let plot = build(&engine, &subject(PaneContent::Signal), full, *size);
    assert!(
        plot.drawn.labels.iter().any(|(text, _)| text.contains("12 of 20 towers drawn: 1–11 and 17")),
        "{:?}",
        plot.drawn.labels.last()
    );
    // One tower asked for by name is drawn whichever it is, and nothing is said to be left out.
    let one = Subject { tower: Some(20), ..subject(PaneContent::Signal) };
    let plot = build(&engine, &one, full, *size);
    assert_eq!((plot.described.towers.as_slice(), series(&plot.drawing).len()), (&[19][..], 1));
    assert!(plot.described.marks["towers"]["note"].is_null());
    // Control: with the cap lifted the same plot draws all twenty and says nothing of it …
    let all = Subject { most_series: usize::MAX, ..subject(PaneContent::Signal) };
    let plot = build(&engine, &all, full, *size);
    assert_eq!(series(&plot.drawing).len(), 20);
    assert!(!plot.drawn.labels.iter().any(|(text, _)| text.contains("towers drawn")));
    // … and a scene of twelve towers is drawn whole.
    engine.apply(cav_engine::Command::load(row_of_towers(12))).unwrap();
    assert!(engine.settle(LONG));
    let plot = build(&engine, &subject(PaneContent::Signal), full, *size);
    assert_eq!(series(&plot.drawing).len(), 12);
    assert!(plot.described.marks["towers"]["note"].is_null());
}

#[test]
fn a_signal_plot_has_a_key() {
    let engine = engine("load pass");
    let style = Style::screen();
    let plot = build(&engine, &subject(PaneContent::Signal), &style, [960.0, 600.0]);
    let area = plot.drawn.area;
    for (n, tower) in engine.spec().array.towers.iter().enumerate() {
        // The tower's label, above the axes …
        let (_, took) = plot
            .drawn
            .labels
            .iter()
            .find(|(text, _)| text == &tower.label)
            .unwrap_or_else(|| panic!("the key does not name {}", tower.label));
        assert!(took.bottom() <= area.top() && took.left() >= area.left(), "{took:?}");
        // … with a sample of its line, in its colour, just to the left of it.
        let colour = style.series(n).0;
        let sample = plot.drawing.items.iter().any(|item| match item {
            Item::Line { points, colour: c, .. } => {
                points.len() == 2
                    && *c == colour
                    && points[0][1] == points[1][1]
                    && points[1][0] < took.left()
                    && took.left() - points[1][0] < 8.0
                    && took.y_range().contains(points[0][1])
            }
            _ => false,
        });
        assert!(sample, "no sample of {}'s line by its name", tower.label);
    }
    // The key's names do not run into each other.
    let names: Vec<_> = plot.drawn.labels.iter().filter(|(text, _)| text.starts_with("tower")).collect();
    for (k, a) in names.iter().enumerate() {
        for b in &names[k + 1..] {
            assert!(!a.1.intersects(b.1), "`{}` and `{}` overlap", a.0, b.0);
        }
    }
    // The same from a pane, which is what `render_view` and the application draw.
    let mut shown = self::engine("load pass; show signal");
    shown.settle(LONG);
    let from_pane = plots::of_pane(&shown, 0, &style, &Outlines, [960.0, 600.0]);
    assert!(from_pane[0].0.drawn.labels.iter().any(|(text, _)| text == "tower 3"));
    // Control: one tower alone needs no key, and has none.
    let alone = Subject { tower: Some(2), ..subject(PaneContent::Signal) };
    let plot = build(&engine, &alone, &style, [960.0, 600.0]);
    assert!(!plot.drawn.labels.iter().any(|(text, _)| text == "tower 2" || text == "tower 1"));
    assert_eq!(series(&plot.drawing).len(), 1);
}

#[test]
fn the_playhead_is_one_colour() {
    let mut engine = engine("load staggered; seek 3");
    engine.settle(LONG);
    let style = Style::screen();
    let colour = |c: egui::Color32| Colour([c.r(), c.g(), c.b()]);
    assert_eq!(style.playhead, colour(cav_plot::theme::PLAYHEAD));
    for content in [PaneContent::Signal, PaneContent::Flight] {
        let plot = build(&engine, &subject(content), &style, [960.0, 600.0]);
        let at = plot.drawn.playhead.expect("the playhead is in the span");
        let lines: Vec<Colour> = plot
            .drawing
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Line { points, colour, .. }
                    if points.len() == 2
                        && points[0][0] == at
                        && points[1][0] == at
                        && (points[0][1] - points[1][1]).abs() >= plot.drawn.area.height() - 0.5 =>
                {
                    Some(*colour)
                }
                _ => None,
            })
            .collect();
        assert_eq!(lines, [style.playhead], "{content:?}: the playhead's line");
    }
    // It is no series' colour and no comb's: S1 drew it in the first comb's blue.
    assert!(!style.series.contains(&style.playhead) && !style.combs.contains(&style.playhead));
    assert_ne!(style.playhead, colour(cav_plot::theme::LINK));
    // Control: a figure in the house style has no playhead at all.
    let paper = Style::preset("paper-full").unwrap();
    assert_eq!(build(&engine, &subject(PaneContent::Signal), &paper, [500.0, 250.0]).drawn.playhead, None);
}

#[test]
fn a_crowded_periodogram_labels_few_harmonics() {
    let named = |plot: &Plot| -> Vec<String> {
        plot.drawn
            .labels
            .iter()
            .filter(|(text, _)| text.ends_with("f₀") || text.ends_with("folded"))
            .map(|(text, _)| text.clone())
            .collect()
    };
    // The zoo: four bodies, eight harmonics each. Every stem stands; only each body's first two
    // harmonics are named, and the legend names the bodies.
    let zoo = engine("load oscillator-zoo; set duration 400");
    let style = Style::screen();
    let plot = build(&zoo, &subject(PaneContent::Periodogram), &style, [960.0, 600.0]);
    assert_eq!(plot.drawn.stems.len(), 4 * 8);
    let labels = named(&plot);
    assert!(labels.len() <= plots::MAX_STEM_LABELS as usize, "{labels:?}");
    let harmonics: Vec<u32> = labels
        .iter()
        .flat_map(|label| label.split(" · "))
        .map(|one| one.split('f').next().unwrap().parse().unwrap_or(1))
        .collect();
    assert!(harmonics.iter().all(|n| *n <= 2), "{labels:?}");
    assert_eq!(harmonics.iter().filter(|n| **n == 1).count(), 4, "each body's f₀ is named: {labels:?}");
    assert_eq!(plot.drawn.labels.iter().filter(|(text, _)| text.contains("· f₀ =")).count(), 4);
    // Control: one body alone has room, and its higher harmonics are named too.
    let comb = engine("load comb; set duration 400");
    let labels = named(&build(&comb, &subject(PaneContent::Periodogram), &style, [960.0, 600.0]));
    assert!(labels.iter().any(|label| label.contains("3f₀")), "{labels:?}");
}

#[test]
fn the_cadence_pane_draws_lines_where_predicted() {
    use cav_engine::{Query, Reply};
    // Planned gaps, 4 shots of every 10: the closed forms predict lines at k/(10 T₀).
    let engine = engine(
        r#"load cadence-lab; set array.schedule.jitter null; set array.schedule.loss 0; set array.schedule.gaps {"on":4,"period":10}"#,
    );
    let Ok(Reply::Cadence(predicted)) = engine.query(&Query::Cadence) else { panic!("no prediction") };
    let subject = Subject { tower: Some(1), ..subject(PaneContent::Cadence) };
    let plot = build(&engine, &subject, &Style::screen(), [640.0, 420.0]);
    let (drawn, area) = (&plot.drawn, plot.drawn.area);
    let on_axis: Vec<_> =
        predicted.lines.iter().filter(|line| (drawn.x.lo..=drawn.x.hi).contains(&line.frequency)).collect();
    assert!(on_axis.len() >= 4, "{} lines predicted on the axis", on_axis.len());
    assert_eq!(drawn.stems.len(), on_axis.len(), "a stem for each predicted line, and no other");
    // Each stem stands where its frequency is on the axis, to a point; and a vertical rule and a
    // dot are in the drawing at that x, the dot at the predicted height.
    let x_of = |f: f64| area.left() + (((f - drawn.x.lo) / (drawn.x.hi - drawn.x.lo)) as f32) * area.width();
    let y_of = |h: f64| {
        let decades = (h.max(drawn.y.lo) / drawn.y.lo).log10() / (drawn.y.hi / drawn.y.lo).log10();
        area.bottom() - (decades as f32) * area.height()
    };
    for (line, stem) in on_axis.iter().zip(&drawn.stems) {
        assert!((stem.2 - x_of(line.frequency)).abs() <= 1.0, "{} Hz at {} pt", line.frequency, stem.2);
        let dot = plot.drawing.items.iter().any(|item| match item {
            Item::Circle { centre, .. } => {
                (centre[0] - stem.2).abs() <= 0.5 && (centre[1] - y_of(line.height)).abs() <= 1.0
            }
            _ => false,
        });
        assert!(dot, "no dot at the predicted height {} of the line at {} Hz", line.height, line.frequency);
    }
    // The realisation is drawn too: one long line across the axes.
    assert!(series(&plot.drawing).iter().any(|(_, points)| points.len() > 100));
    // Control: an even cadence predicts no line but the alias, and the pane draws no more stems
    // than that; so the stems above are the gaps', not decoration.
    let even = self::engine("load cadence-lab; set array.schedule.jitter null; set array.schedule.loss 0");
    let plain = build(&even, &subject, &Style::screen(), [640.0, 420.0]);
    assert!(
        plain.drawn.stems.len() < drawn.stems.len(),
        "{} stems on an even cadence",
        plain.drawn.stems.len()
    );
}

/// The comb's oscillator on a walk of late cycles, 4 shots of every 10 planned and a fifth of
/// those lost, over a minute: a scene whose towers do not take every shot.
const LOSSY: &str = r#"load cadence-lab; set array.schedule.gaps {"on":4,"period":10}; set array.schedule.loss 0.2; set duration 60"#;

/// Whether any point of `points` is within a point's width of any of the times' places `at`.
fn touches(points: &[[f32; 2]], at: &[f32]) -> bool {
    points.iter().any(|point| at.iter().any(|x| (point[0] - x).abs() <= 1.0))
}

#[test]
fn a_signal_plot_breaks_at_a_shot_not_taken() {
    // I2 of S2's review: a shot not taken was a point at 0 rad, joined into the line.
    let engine = engine(LOSSY);
    let style = Style::screen();
    let one = Subject { tower: Some(1), ..subject(PaneContent::Signal) };
    let plot = build(&engine, &one, &style, [960.0, 600.0]);
    let (drawn, area) = (&plot.drawn, plot.drawn.area);
    let signal = engine.signal().1.unwrap().clone();
    let (times, taken) = (signal.readouts(0), signal.taken(0).to_vec());
    let values = signal.channel(0, cav_core::forward::Channel::Gradiometer);
    let (shots, kept) = (taken.len(), taken.iter().filter(|taken| **taken).count());
    assert!(shots >= 24 && kept >= 4 && kept < shots / 2, "{kept} of {shots} shots taken");
    let x_of = |time: f64| area.left() + area.width() * drawn.x.fraction(time) as f32;
    let lost: Vec<f32> =
        times.iter().zip(&taken).filter(|(_, taken)| !**taken).map(|(time, _)| x_of(*time)).collect();
    // The runs of shots taken: a line for each run of two or more, and nothing else in the
    // series' colour that joins points.
    let mut runs: Vec<usize> = Vec::new();
    for (k, taken) in taken.iter().enumerate() {
        match (taken, k > 0 && signal.taken(0)[k - 1]) {
            (true, true) => *runs.last_mut().unwrap() += 1,
            (true, false) => runs.push(1),
            _ => {}
        }
    }
    let colour = style.series(0).0;
    let lines: Vec<&[[f32; 2]]> = plot
        .drawing
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Line { points, colour: c, .. } if *c == colour => Some(points.as_slice()),
            _ => None,
        })
        .collect();
    let long: Vec<usize> = runs.iter().copied().filter(|run| *run >= 2).collect();
    assert!(long.len() >= 2, "{runs:?}");
    assert_eq!(lines.iter().map(|line| line.len()).collect::<Vec<_>>(), long, "one line for each run");
    // No line has a vertex at a shot not taken, and no dot stands there.
    assert!(lines.iter().all(|line| !touches(line, &lost)));
    let dots: Vec<[f32; 2]> = plot
        .drawing
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Circle { centre, fill, .. } if *fill == colour => Some(*centre),
            _ => None,
        })
        .collect();
    assert_eq!(dots.len(), kept, "a dot for each shot taken");
    assert!(!touches(&dots, &lost));
    assert_eq!(drawn.points, kept);
    // Each shot not taken has a hollow square at the foot of the axis, at its time (the last
    // shot is read out after the end of the timeline, off the axis, and has none).
    assert!(lost.iter().filter(|x| **x <= area.right()).count() >= lost.len() - 1);
    for x in lost.iter().filter(|x| **x <= area.right()) {
        let marked = plot.drawing.items.iter().any(|item| match item {
            Item::Rect { min, max, fill: None, stroke: Some((_, c)) } => {
                *c == colour
                    && ((min[0] + max[0]) / 2.0 - x).abs() <= 0.5
                    && ((min[1] + max[1]) / 2.0 - area.bottom()).abs() <= 0.5
                    && max[0] - min[0] < 8.0
            }
            _ => false,
        });
        assert!(marked, "no hollow square at the foot of the axis at {x} pt");
    }
    // The caption counts them.
    let said = format!("{kept} of {shots} shots taken");
    assert!(drawn.labels.iter().any(|(text, _)| text.contains(&said)), "{:?}", drawn.labels.last());
    assert_eq!(plot.described.marks["shots_taken"], serde_json::json!([kept]));
    // Control: the line as it was drawn before, every slot joined, has a vertex at each shot
    // not taken, at 0 rad, and the same check finds it.
    let old: Vec<[f32; 2]> = times
        .iter()
        .zip(values)
        .map(|(time, value)| {
            let at = plots::place(area, &drawn.x, &drawn.y, (*time, *value));
            [at.x, at.y]
        })
        .collect();
    assert!(touches(&old, &lost));
    let zero = plots::place(area, &drawn.x, &drawn.y, (0.0, 0.0)).y;
    let at_zero = old.iter().filter(|point| touches(&[**point], &lost) && point[1] == zero).count();
    assert_eq!(at_zero, shots - kept, "the old line passes through 0 rad at every shot not taken");
    // Control: with every shot taken there is one line, no hollow square and the plain count.
    let whole = self::engine("load comb; set duration 60");
    let plot = build(&whole, &one, &style, [960.0, 600.0]);
    assert_eq!(series(&plot.drawing).len(), 1);
    let hollow =
        |item: &Item| matches!(item, Item::Rect { fill: None, stroke: Some((_, c)), .. } if *c == colour);
    assert!(!plot.drawing.items.iter().any(hollow));
    assert!(plot.drawn.labels.iter().any(|(text, _)| text.contains("· 30 shots")));
}

#[test]
fn a_long_series_is_broken_where_no_shot_is_taken() {
    // More shots than points: the series is thinned, a stretch at a time, and a daily window
    // that leaves most of the run untaken is a break in the line, not a line along 0 rad.
    let (n, lit) = (10_000usize, 2_000usize..3_000);
    let x: Vec<f64> = (0..n).map(|k| k as f64).collect();
    let y: Vec<f64> = (0..n).map(|k| 5.0 + (k as f64 * 0.1).sin()).collect();
    let taken: Vec<bool> = (0..n).map(|k| lit.contains(&k) || k >= 9_000).collect();
    let runs = plots::runs(&x, &y, &taken);
    assert_eq!(runs.len(), 2, "two stretches of shots taken, two lines");
    let points: usize = runs.iter().map(Vec::len).sum();
    assert!(points <= plots::MAX_POINTS + 2, "{points} points");
    assert!(runs[0].iter().all(|(at, _)| lit.contains(&(*at as usize))));
    assert!(runs.iter().flatten().all(|(_, value)| *value >= 4.0), "no slot not taken is on a line");
    // The extremes of what was taken are kept.
    let top = runs.iter().flatten().fold(f64::NEG_INFINITY, |m, (_, value)| m.max(*value));
    assert!((top - 6.0).abs() < 1e-3);
    // Control: thinned as it was before, with the slots not taken at zero, the line goes there.
    let zeroed: Vec<f64> =
        y.iter().zip(&taken).map(|(value, taken)| if *taken { *value } else { 0.0 }).collect();
    assert!(plots::thinned(&x, &zeroed).iter().any(|(_, value)| *value == 0.0));
    // Control: every shot taken is one run.
    assert_eq!(plots::runs(&x, &y, &vec![true; n]).len(), 1);
}

/// The short marks a drawing has at the left edge of `area`: the heights of the lines that run
/// from the frame inwards no further than five points.
fn short_marks(drawing: &Drawing, area: egui::Rect) -> Vec<f32> {
    drawing
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Line { points, .. }
                if points.len() == 2
                    && (points[0][0] - area.left()).abs() < 0.01
                    && points[1][0] > area.left()
                    && points[1][0] - area.left() < 5.0
                    && (points[0][1] - points[1][1]).abs() < 0.01 =>
            {
                Some(points[0][1])
            }
            _ => None,
        })
        .collect()
}

#[test]
fn a_logarithmic_axis_is_written_in_powers_with_marks_between() {
    // Review 29 (S2.md §7.14), written first: a decade is written as 10 with its exponent raised
    // and smaller; between labelled decades, short marks at 2 to 9 of each decade when a decade
    // is at least 16·u high, at the unlabelled decades when only some are labelled, none when
    // there is no room. Control: a linear axis has none; a cramped one has none.
    let engine = engine("load comb; set duration 400");
    let style = Style::screen();
    let periodogram = Subject { tower: Some(2), ..subject(PaneContent::Periodogram) };
    let plot = build(&engine, &periodogram, &style, [960.0, 600.0]);
    let (area, y) = (plot.drawn.area, plot.drawn.y);
    // The numbers: each labelled decade is `10` and its exponent, the exponent smaller and above
    // the middle of the `10`.
    let ticks = &plot.drawn.ticks[1];
    assert!(ticks.len() >= 3);
    for (value, said) in ticks.iter().zip(&plot.drawn.tick_labels[1]) {
        assert_eq!(*said, format!("10^{}", value.log10().round() as i32));
    }
    let tens: Vec<&(String, egui::Rect)> =
        plot.drawn.labels.iter().filter(|(text, _)| text == "10").collect();
    assert_eq!(tens.len(), ticks.len());
    for (ten, value) in tens.iter().zip(ticks) {
        let n = value.log10().round() as i32;
        let exponent = if n < 0 { format!("\u{2212}{}", -n) } else { n.to_string() };
        let (_, raised) = plot
            .drawn
            .labels
            .iter()
            .find(|(text, took)| *text == exponent && (took.left() - ten.1.right()).abs() < 1.0)
            .unwrap();
        assert!(
            raised.height() < ten.1.height() && raised.center().y < ten.1.center().y,
            "{raised:?} against {:?}",
            ten.1
        );
    }
    assert!(!plot.drawn.labels.iter().any(|(text, _)| text.starts_with("1e")), "no `1e-5` is written");
    // The short marks: 2 to 9 of each decade, where the axis puts them.
    let decade = area.height() / (y.hi.log10() - y.lo.log10()) as f32;
    assert!(decade >= 16.0, "{decade} points a decade");
    let minor = &plot.drawn.minor_ticks[1];
    let expected: usize = 8 * (y.hi.log10() - y.lo.log10()).round() as usize;
    assert!(minor.len() >= expected, "{} marks, at least {expected}", minor.len());
    let marks = short_marks(&plot.drawing, area);
    for value in minor {
        let at = plots::place(area, &plot.drawn.x, &y, (0.0, *value)).y;
        assert!(marks.iter().any(|m| (m - at).abs() < 0.01), "no mark at {value}");
    }
    for m in [2.0, 5.0, 9.0] {
        let value = m * 10f64.powi(y.lo.log10().round() as i32);
        assert!(minor.iter().any(|v| (v / value - 1.0).abs() < 1e-9), "{value} is not marked");
    }
    // A cramped axis has none; one a little less cramped marks the decades it does not label.
    let axis = plots::Axis { lo: 1e-12, hi: 1.0, log: true };
    let labelled = axis.ticks(6);
    assert!(axis.minor_ticks(&labelled, 2.0, 1.0).is_empty());
    let skipped = axis.minor_ticks(&labelled, 8.0, 1.0);
    assert_eq!(skipped.len(), 13 - labelled.len());
    assert!(skipped.iter().all(|v| !labelled.contains(v)));
    // Control: a linear axis has no short marks, in the axis or in the drawing.
    assert!(
        plots::Axis { lo: 0.0, hi: 1.0, log: false }.minor_ticks(&[0.0, 0.5, 1.0], 100.0, 1.0).is_empty()
    );
    let signal = build(&engine, &subject(PaneContent::Signal), &style, [960.0, 600.0]);
    assert!(
        signal.drawn.minor_ticks[1].is_empty() && short_marks(&signal.drawing, signal.drawn.area).is_empty()
    );
}

#[test]
fn the_unit_is_the_vertical_axis_s_label() {
    // Review 29, written first: the vertical axis has a label written up the page beside its
    // numbers, centred on the axis; no unit stands at the top left. Control: the same text not
    // turned is wider than tall.
    let engine = engine("load comb; set duration 400");
    let paper = Style::preset("paper-column").unwrap();
    let points = paper.size_mm.map(|mm| (mm / cav_plot::style::MM_PER_POINT) as f32);
    for (content, label) in [
        (PaneContent::Signal, "ΔΦ, mrad"),
        (PaneContent::Periodogram, "√S, rad/√Hz"),
        (PaneContent::Cadence, "|W|, relative to the tone"),
    ] {
        let plot = build(&engine, &subject(content), &paper, points);
        assert_eq!(plot.drawn.axis_labels[1].as_deref(), Some(label), "{content:?}");
        let (_, took) = plot.drawn.labels.iter().find(|(text, _)| text == label).unwrap();
        let page = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(points[0], points[1]));
        assert!(took.height() > took.width() && page.contains_rect(*took), "{content:?}: {took:?}");
        assert!((took.center().y - plot.drawn.area.center().y).abs() < 0.5, "centred on the axis");
        // Left of every number of the axis.
        let numbers = plot
            .drawn
            .labels
            .iter()
            .filter(|(_, r)| r.right() <= plot.drawn.area.left() && r.center().y > plot.drawn.area.top());
        for (text, number) in numbers.filter(|(text, _)| text != label) {
            assert!(took.right() <= number.left(), "{content:?}: `{label}` is not left of `{text}`");
        }
        // It is the one turned text, and nothing stands at the top left above the numbers.
        let turned: Vec<&str> = plot
            .drawing
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Text { text, turned: true, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(turned, [label]);
        assert!(!plot.drawing.texts().iter().any(|t| ["rad/√Hz", "mrad", "µrad"].contains(t)));
        // Control: the same text as it would be written flat is wider than tall.
        let [w, h] = cav_plot::drawing::Measure::size(
            &Outlines,
            label,
            cav_plot::drawing::Face::Sans,
            paper.text_size,
        );
        assert!(w > h);
        // In the SVG and the PDF it is well formed, and its outline lies in its rectangle.
        let svg = cav_plot::svg::write(&plot.drawing).unwrap();
        cav_plot::svg::check(&svg).unwrap();
        cav_plot::pdf::check(&cav_plot::pdf::write(&plot.drawing)).unwrap();
        let outline = cav_plot::fonts::outline(label, cav_plot::drawing::Face::Sans, paper.text_size);
        let at = plot.drawing.items.iter().find_map(|item| match item {
            Item::Text { at, anchor, turned: true, .. } => Some((*at, *anchor)),
            _ => None,
        });
        let (at, anchor) = at.unwrap();
        for segment in &outline.segments {
            let end = match segment {
                cav_plot::fonts::Segment::Move(q)
                | cav_plot::fonts::Segment::Line(q)
                | cav_plot::fonts::Segment::Cubic(_, _, q) => *q,
                cav_plot::fonts::Segment::Close => continue,
            };
            let [x, y] = cav_plot::drawing::text_point(at, anchor, outline.size, true, end);
            assert!(took.expand(0.6).contains(egui::pos2(x, y)), "{content:?}: ({x}, {y}) outside {took:?}");
        }
    }
}

#[test]
fn a_figure_may_have_no_title() {
    // Review 29, written first: without its title the picture has no title text and the axes
    // take the room; the drawing keeps its name. Control: with it, the title is there.
    let engine = engine("load comb; set duration 400");
    let paper = Style::preset("paper-column").unwrap();
    let points = paper.size_mm.map(|mm| (mm / cav_plot::style::MM_PER_POINT) as f32);
    for content in [PaneContent::Signal, PaneContent::Periodogram, PaneContent::Cadence, PaneContent::Flight]
    {
        let titled = build(&engine, &subject(content), &paper, points);
        let untitled = build(&engine, &Subject { title: false, ..subject(content) }, &paper, points);
        let title = titled.drawing.title.clone();
        assert!(titled.drawn.titled && titled.drawing.texts().contains(&title.as_str()), "{content:?}");
        assert!(!untitled.drawn.titled && !untitled.drawing.texts().contains(&title.as_str()), "{content:?}");
        assert_eq!(untitled.drawing.title, title, "the name is kept for the file's metadata");
        assert!(
            untitled.drawn.area.top() < titled.drawn.area.top() - 10.0,
            "{content:?}: the axes take the room"
        );
        assert_eq!(untitled.drawn.area.bottom(), titled.drawn.area.bottom());
    }
}

#[test]
fn the_flight_rows_have_a_height_scale() {
    // Review 29, written first: at the right of the rows a bar for each row from the tower's base
    // to the top of the scale, and on the first row its two ends named in metres. Control: the
    // bar is not the row's whole height (the row has a margin).
    let engine = engine("load comb");
    let paper = Style::preset("paper-column").unwrap();
    let points = paper.size_mm.map(|mm| (mm / cav_plot::style::MM_PER_POINT) as f32);
    let plot = build(&engine, &subject(PaneContent::Flight), &paper, points);
    let (area, y) = (plot.drawn.area, plot.drawn.y);
    let rows = engine.spec().array.towers.len().min(plots::MAX_FLIGHT_ROWS);
    assert_eq!(plot.drawn.scale_bars.len(), rows);
    let row_height = area.height() / rows as f32;
    let margin = 3.0 * paper.unit();
    for (n, [x, base, top]) in plot.drawn.scale_bars.iter().enumerate() {
        let row_top = area.top() + row_height * n as f32;
        let inner = (row_top + margin, row_top + row_height - margin);
        let at = |v: f64| inner.1 - (inner.1 - inner.0) * y.fraction(v) as f32;
        assert!(
            *x > area.right() && (base - at(0.0)).abs() < 0.01 && (top - at(y.hi)).abs() < 0.01,
            "row {n}"
        );
        // Control: a bar from the row's own top and bottom would not be this one.
        assert!((top - row_top).abs() > 1.0 && (base - (row_top + row_height)).abs() > 1.0);
    }
    let page = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(points[0], points[1]));
    let named = format!("{} m", y.hi);
    for name in [named.as_str(), "0"] {
        let (_, took) =
            plot.drawn.labels.iter().find(|(text, took)| text == name && took.left() > area.right()).unwrap();
        assert!(page.contains_rect(*took), "`{name}` at {took:?}");
    }
    assert_eq!(named, "10.1 m");
}
