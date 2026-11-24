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
        // Its heading is the field's, and its numbers are the drawn range's ends and middle.
        assert_eq!(texts[0], heading(&frame, &scene));
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
    // Nothing drawn, no legend.
    assert!(legend(&VoxelFrame::default(), &scene, &style, &Outlines).is_none());
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
                Item::Text { at, anchor, text, face, size, turned, .. } if text.chars().next().is_some_and(|c| c == '-' || c.is_ascii_digit()) => {
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
    assert!(!dir.join("view.csv").exists());
    // Control: a scene with no voxel bodies has no voxel frame, and its view stays the
    // renderer's picture (`gpu_a_view_figure_is_the_renderers_picture`).
    assert!(engine("load comb").frame(0).unwrap().voxels.is_none());
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
        assert_eq!(differing(&dirs[0], &dirs[2]), 3);
        cav_validate::Measured::Error(differing(&dirs[0], &dirs[1]) as f64)
    },
}
