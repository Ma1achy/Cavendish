//! Voxel bodies in figures: the legend made from what is drawn, and a view of them as a vector
//! drawing (the S3 brief, R10 and V12).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use cav_engine::Engine;
use cav_plot::drawing::Item;
use cav_plot::fonts::Outlines;
use cav_plot::style::Style;
use cav_plot::voxels::{BAR_SEGMENTS, colour_of, heading, legend, number};
use cav_plot::{figure, pdf, svg};
use cav_sim::voxel_view::{Colouring, LeafField, Overlay, VoxelFrame, voxel_frame};

const LONG: Duration = Duration::from_secs(300);

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/voxel-figures").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn word(path: &Path) -> String {
    format!("\"{}\"", path.display().to_string().replace('\\', "/"))
}

fn frame_of(scene: &cav_sim::SceneSpec, field: LeafField) -> VoxelFrame {
    let trees: Vec<_> =
        scene.bodies.iter().map(|b| b.shape.as_ref().map(|s| Arc::new(s.tree(b.mass).unwrap()))).collect();
    voxel_frame(scene, &trees, Colouring { field, tower: 0 }, Overlay::default(), 0.0)
}

/// The fills of the legend's bar, left to right: the rectangles with a fill and no stroke.
fn bar(drawing: &cav_plot::drawing::Drawing) -> Vec<cav_plot::drawing::Colour> {
    drawing
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Rect { fill: Some(fill), stroke: None, .. } => Some(*fill),
            _ => None,
        })
        .collect()
}

#[test]
fn a_legend_is_made_from_what_is_drawn() {
    let scene = cav_sim::dev::unit_cube();
    let style = Style::preset("paper-column").unwrap();
    for field in [LeafField::Height, LeafField::Phase] {
        let frame = frame_of(&scene, field);
        let (lo, hi) = frame.range.unwrap();
        let drawn = legend(&frame, &scene, &style, &Outlines).unwrap();
        let texts = drawn.texts();
        // Its heading is the field's, and its numbers are the drawn range's ends and middle; a
        // range of one value (the cube's leaves all at one height) is one swatch and its number
        // (S18's second review, UX 5: it was a bar labelled "2.5 2.5 2.5").
        assert_eq!(texts[0], heading(&frame, &scene));
        if lo == hi {
            assert_eq!(texts[1..], [format!("all {}", number(lo, field))]);
            assert!(bar(&drawn).is_empty());
            continue;
        }
        assert_eq!(texts[1..], [number(lo, field), number(0.5 * (lo + hi), field), number(hi, field)]);
        // The bar is the field's own map, segment by segment, from the least to the greatest.
        let fills = bar(&drawn);
        assert_eq!(fills.len(), BAR_SEGMENTS);
        for (k, fill) in fills.iter().enumerate() {
            let value = lo + (hi - lo) * (k as f64 + 0.5) / BAR_SEGMENTS as f64;
            assert_eq!(*fill, colour_of(&frame, value, false), "{field:?}, segment {k}");
        }
        if field.diverging() {
            assert!((lo + hi).abs() < 1e-300, "symmetric about zero: {lo} {hi}");
            assert_eq!(texts[2], "0");
        }
        // Control: the same frame with another range is another legend.
        let mut other = frame.clone();
        other.range = Some((lo - 1.0, hi + 1.0));
        assert_ne!(legend(&other, &scene, &style, &Outlines).unwrap().texts()[1..], texts[1..]);
    }
    // A sequential field over a range that is not flat, segment by segment (S18's third review,
    // minor 5: the cube's heights are one value, so only Phase, diverging, was checked so). The
    // range is widened by hand; the bar is the field's own map from the least to the greatest.
    let mut level = frame_of(&scene, LeafField::Level);
    level.range = Some((0.0, 4.0));
    let fills = bar(&legend(&level, &scene, &style, &Outlines).unwrap());
    assert_eq!(fills.len(), BAR_SEGMENTS);
    for (k, fill) in fills.iter().enumerate() {
        let value = 4.0 * (k as f64 + 0.5) / BAR_SEGMENTS as f64;
        assert_eq!(*fill, colour_of(&level, value, false), "Level, segment {k}");
    }
    // Control: the map read the other way round is not this bar.
    let reversed: Vec<_> = (0..BAR_SEGMENTS)
        .map(|k| colour_of(&level, 4.0 * (1.0 - (k as f64 + 0.5) / BAR_SEGMENTS as f64), false))
        .collect();
    assert_ne!(fills, reversed);
    // Nothing drawn, no legend.
    assert!(legend(&VoxelFrame::default(), &scene, &style, &Outlines).is_none());
}

#[test]
fn a_legend_of_one_value_is_one_swatch() {
    // S18's second review, UX 5: every leaf of one level read "3 3 3" under a bar of one colour.
    let scene = cav_sim::dev::unit_cube();
    let style = Style::preset("screen").unwrap();
    let mut frame = frame_of(&scene, LeafField::Level);
    frame.range = Some((3.0, 3.0));
    let drawn = legend(&frame, &scene, &style, &Outlines).unwrap();
    let texts = drawn.texts();
    assert_eq!(texts[1..], ["all 3".to_owned()], "{texts:?}");
    assert!(bar(&drawn).is_empty(), "a swatch, no bar");
    // Control: a range of two values is a bar with its three numbers.
    frame.range = Some((2.0, 3.0));
    let drawn = legend(&frame, &scene, &style, &Outlines).unwrap();
    assert_eq!(drawn.texts().len(), 4);
    assert_eq!(bar(&drawn).len(), BAR_SEGMENTS);
}

#[test]
fn a_legend_says_each_word_once_and_each_number_as_it_is() {
    // Found in the S3 demo's `shapes.png`: the heading read "level, level", and the middle of
    // levels 0 to 3 read "2" where the bar's middle is 1.5.
    let scene = cav_sim::dev::unit_cube();
    let repeats = |text: &str| {
        let words: Vec<&str> = text.split([',', ' ']).filter(|w| !w.is_empty()).collect();
        words.iter().enumerate().any(|(i, w)| words[..i].contains(w))
    };
    for field in [LeafField::Level, LeafField::Mass, LeafField::Fill, LeafField::Height, LeafField::Distance]
    {
        let title = heading(&frame_of(&scene, field), &scene);
        assert!(!repeats(&title), "{field:?}: {title}");
    }
    // Control: the old heading is caught.
    assert!(repeats("level, level"));
    assert_eq!(number(1.5, LeafField::Level), "1.5");
    assert_eq!([number(0.0, LeafField::Level), number(3.0, LeafField::Level)], ["0", "3"]);
}
#[test]
fn a_legend_s_numbers_do_not_run_into_each_other() {
    use cav_plot::drawing::{Item, Measure, text_box};
    let scene = cav_sim::dev::unit_cube();
    let style = Style::preset("screen").unwrap();
    // The ranges the demo's first legends wrote over each other ("2.36e-414.8"): a tiny least
    // with a wide greatest, and three-figure decimals.
    for range in [(2.36e-4, 29.6), (0.0312, 1.0), (-0.498, 0.498), (1.4, 2.8)] {
        let mut frame = frame_of(&scene, LeafField::Mass);
        frame.range = Some(range);
        let drawn = legend(&frame, &scene, &style, &Outlines).unwrap();
        let boxes: Vec<([f32; 2], [f32; 2])> = drawn
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Text { at, anchor, text, face, size, turned, .. }
                    if text.chars().next().is_some_and(|c| c == '-' || c.is_ascii_digit()) =>
                {
                    Some(text_box(*at, *anchor, Outlines.size(text, *face, *size), *turned))
                }
                _ => None,
            })
            .collect();
        assert_eq!(boxes.len(), 3, "{range:?}");
        for pair in boxes.windows(2) {
            assert!(pair[0].1[0] < pair[1].0[0], "{range:?}: {pair:?} overlap");
        }
    }
}

fn engine(script: &str) -> Engine {
    let mut engine = Engine::new();
    engine.set_patience(LONG);
    figure::install(&mut engine);
    engine.run_script(script).unwrap();
    assert!(engine.settle(LONG), "the signal of `{script}` did not arrive");
    engine
}

fn files_of(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (path.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read(&path).unwrap())
        })
        .collect()
}

/// The `unit-cube` scene coloured by `field`, its view written as SVG and PDF into `dir`.
fn write_view(dir: &Path, field: &str) {
    let mut engine = engine(&format!("load unit-cube; view iso; colour {field}"));
    for name in ["view.svg", "view.pdf"] {
        let line = format!("figure view as paper-full to {}", word(&dir.join(name)));
        engine.run_line(&line).unwrap_or_else(|why| panic!("`{line}`: {why}"));
    }
}

#[test]
fn a_voxel_view_is_a_vector_figure() {
    let dir = scratch("vector");
    write_view(&dir, "height");
    let text = std::fs::read_to_string(dir.join("view.svg")).unwrap();
    svg::check(&text).unwrap();
    let polygons = text.matches("<polygon").count();
    assert!(polygons >= 3, "{polygons} polygons");
    assert!(!text.contains("<image"), "a vector view holds no picture");
    pdf::check(&std::fs::read(dir.join("view.pdf")).unwrap()).unwrap();
    // The record says what is drawn and how: the field, its unit, the drawn range, the cut.
    let record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("view.json")).unwrap()).unwrap();
    let svg_entry = record["files"].as_array().unwrap().iter().find(|f| f["file"] == "view.svg").unwrap();
    assert_eq!(svg_entry["vector"], true);
    assert_eq!(svg_entry["voxels"]["field"], "height");
    assert_eq!(svg_entry["voxels"]["unit"], "m");
    assert_eq!(svg_entry["voxels"]["cut"], 0);
    let faces = svg_entry["voxels"]["faces"].as_u64().unwrap();
    assert_eq!(faces as usize, polygons, "a polygon a face, and nothing else drawn as one");
    let range = svg_entry["voxels"]["range"].as_array().unwrap();
    let scene = cav_sim::dev::unit_cube();
    let (lo, hi) = frame_of(&scene, LeafField::Height).range.unwrap();
    assert_eq!((range[0].as_f64().unwrap(), range[1].as_f64().unwrap()), (lo, hi));
    // It is a figure as the presets promise one (REBUILD §6, R10; S3's review, finding 6): on the
    // preset's paper, with its title, the legend's heading and numbers, a scale, and the cubes it
    // draws as its data file.
    let paper = Style::preset("paper-full").unwrap().background;
    assert!(text.contains(&format!("fill=\"{}\"", paper.css())), "the paper's colour");
    let labels: Vec<&str> = text.split("aria-label=\"").skip(1).map(|t| &t[..t.find('"').unwrap()]).collect();
    // (The cube's leaves are all at one height: its legend is one swatch, "all 2.5", since S18's
    // second review, UX 5.)
    let number = if lo == hi {
        format!("all {}", number(lo, LeafField::Height))
    } else {
        number(lo, LeafField::Height)
    };
    for wanted in ["View · unit-cube", "height, m", &number] {
        assert!(labels.contains(&wanted), "{wanted:?} not in {labels:?}");
    }
    let scale = svg_entry["scale_m"].as_f64().unwrap();
    assert!(labels.iter().any(|l| l.ends_with(" m") && l.starts_with(&format!("{scale}"))), "{labels:?}");
    assert_eq!(record["data"]["file"], "view.csv");
    let data = std::fs::read_to_string(dir.join("view.csv")).unwrap();
    let mut lines = data.lines();
    assert_eq!(
        lines.next().unwrap(),
        "body [its number from 1],x [m],y [m],z [m],side [m],level [levels above the finest],height [m]"
    );
    let cubes = svg_entry["voxels"]["cubes"].as_u64().unwrap() as usize;
    assert_eq!(lines.count(), cubes, "a row a cube drawn");
    // Control: a scene with no voxel bodies has no voxel frame, and its view stays the
    // renderer's picture (`gpu_a_view_figure_is_the_renderers_picture`).
    assert!(engine("load comb").frame(0).unwrap().voxels.is_none());
}

#[test]
fn gpu_a_voxel_view_as_png_is_the_same_figure() {
    // A PNG of a view with voxel bodies is the drawing, rasterised: the preset's paper, not the
    // window's dark picture with no key (S3's review, finding 6); with the same data file.
    let dir = scratch("png");
    let mut cube = engine("load unit-cube; view iso; colour height");
    cube.run_line(&format!("figure view as paper-full dpi 100 to {}", word(&dir.join("view.png")))).unwrap();
    let (rgba, _) = cav_render::decode_png(&std::fs::read(dir.join("view.png")).unwrap()).unwrap();
    let paper = Style::preset("paper-full").unwrap().background;
    assert_eq!(&rgba[..3], &paper.0);
    assert!(dir.join("view.csv").is_file());
    // Control: a scene with no voxel bodies is still the renderer's picture, on its own ground.
    let mut plain = engine("load comb; view top");
    plain.run_line(&format!("figure view as paper-full dpi 100 to {}", word(&dir.join("comb.png")))).unwrap();
    let (rgba, _) = cav_render::decode_png(&std::fs::read(dir.join("comb.png")).unwrap()).unwrap();
    assert_ne!(&rgba[..3], &paper.0);
}

/// Files that differ in any byte between two directories, a missing file counting.
fn differing(a: &Path, b: &Path) -> usize {
    let (a, b) = (files_of(a), files_of(b));
    a.keys()
        .chain(b.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|name| a.get(*name) != b.get(*name))
        .count()
}

cav_validate::case! {
    test: validation_a_view_figure_regenerates,
    name: "figure.view_regenerates",
    subsystem: "figure",
    slice: "S3",
    reference: Exact,
    against: "the unit cube's view coloured by height, as vector SVG and PDF with its record, written by two engines from the same commands: files that differ in any byte",
    unit: Count,
    tolerance: 0.0,
    measure: {
        let root = scratch("regenerates");
        let dirs: Vec<PathBuf> = ["first", "second", "other"].iter().map(|n| root.join(n)).collect();
        for dir in &dirs {
            std::fs::create_dir_all(dir).unwrap();
        }
        write_view(&dirs[0], "height");
        write_view(&dirs[1], "height");
        // Control: another field is another figure, every file of it.
        write_view(&dirs[2], "level");
        assert_eq!(differing(&dirs[0], &dirs[2]), 4);
        cav_validate::Measured::Error(differing(&dirs[0], &dirs[1]) as f64)
    },
}

#[test]
fn the_tree_legend_is_made_from_the_depths_drawn() {
    use cav_plot::voxels::{tree_colour_of, tree_legend};
    use cav_sim::voxel_view::{Fade, TreeOverlay};
    let scene = cav_sim::dev::sphere();
    let trees: Vec<_> =
        scene.bodies.iter().map(|b| b.shape.as_ref().map(|s| Arc::new(s.tree(b.mass).unwrap()))).collect();
    let style = Style::screen();
    let frame_with = |tree: TreeOverlay| {
        voxel_frame(
            &scene,
            &trees,
            Colouring::default(),
            Overlay { tree: Some(tree), ..Overlay::default() },
            0.0,
        )
    };
    // The swatches: rectangles with a fill and a stroke, after the panel.
    let swatches = |d: &cav_plot::drawing::Drawing| -> Vec<cav_plot::drawing::Colour> {
        d.items
            .iter()
            .skip(1)
            .filter_map(|item| match item {
                Item::Rect { fill: Some(fill), stroke: Some(_), .. } => Some(*fill),
                _ => None,
            })
            .collect()
    };
    let frame = frame_with(TreeOverlay { from: 1, to: Some(4), ..TreeOverlay::default() });
    assert_eq!(frame.depths, Some((1, 4)));
    let drawn = tree_legend(&frame, &style, &Outlines).unwrap();
    // One swatch a depth drawn, labelled with it, in the map's colour for it, end to end.
    assert_eq!(drawn.texts()[1..], ["1", "2", "3", "4"]);
    let colours = swatches(&drawn);
    assert_eq!(colours, (1..=4).map(|d| tree_colour_of(&frame, d, &style)).collect::<Vec<_>>());
    let ends = |dark| cav_render::colour::of_value(1.0, (1.0, 4.0), false, dark);
    assert_eq!(colours[0].0, ends(true));
    // Fading: the shallowest swatch is nearer the background than with no fade; the deepest is
    // as it was. Control: the other way round for `shallow`.
    let off = |c: cav_plot::drawing::Colour| {
        c.0.iter().zip(style.background.0).map(|(a, b)| a.abs_diff(b) as u32).sum::<u32>()
    };
    let deep = swatches(
        &tree_legend(
            &frame_with(TreeOverlay { from: 1, to: Some(4), fade: Fade::Deep, faces: false, alone: false }),
            &style,
            &Outlines,
        )
        .unwrap(),
    );
    assert!(off(deep[0]) < off(colours[0]) && deep[3] == colours[3]);
    let shallow = swatches(
        &tree_legend(
            &frame_with(TreeOverlay {
                from: 1,
                to: Some(4),
                fade: Fade::Shallow,
                faces: false,
                alone: false,
            }),
            &style,
            &Outlines,
        )
        .unwrap(),
    );
    assert!(off(shallow[3]) < off(colours[3]) && shallow[0] == colours[0]);
    // No tree overlay, no legend.
    assert!(tree_legend(&frame_of(&scene, LeafField::Level), &style, &Outlines).is_none());
}
