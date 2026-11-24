//! A figure of a scene at its caps, with the counting allocator: what the export holds beyond
//! the engine's own signal (S2.md §3.1, A17; §7.14, finding 10 of the slice's review).
//!
//! This is a test binary of its own because the allocator is the whole process's.

use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cav_engine::{Engine, Figure, FigureFormat, FigureKind, PaneContent};
use cav_memtest::{Counting, peak_during};
use cav_plot::drawing::{Drawing, Item};
use cav_plot::fonts::Outlines;
use cav_plot::{figure, plots, svg};

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const MIB: usize = 1 << 20;
const LONG: Duration = Duration::from_secs(300);

/// What the export of a figure may hold beyond the engine's own signal (§7.14: written first).
const EXPORT_BYTES: usize = 16 * MIB;

/// The most bytes the SVG of a signal may be.
const SVG_BYTES: usize = 2_000_000;

/// The void with `towers` towers, a slab under them and `shots` shots: a scene whose signal is as
/// large as asked and costs one read a tower (as `cav-memtest`'s own tests make it).
fn slab_scene(towers: usize, shots: usize) -> cav_sim::SceneSpec {
    let mut scene = cav_sim::dev::void();
    scene.array.towers =
        (0..towers).map(|n| cav_sim::TowerSpec::at(&format!("t{n}"), [n as f64, 0.0, 0.0])).collect();
    scene.array.instrument.half_sequence = 0.2;
    scene.array.instrument.cycle = 0.5;
    scene.duration = 0.5 * shots as f64;
    scene.floor = Some(cav_sim::FloorSpec {
        height: -1.0,
        kind: cav_sim::FloorKind::Slab { thickness: 1.0 },
        density: 2400.0,
    });
    assert_eq!(scene.validate(), Ok(()));
    scene
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/figures").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// How many points the series of a drawing hold: every line of more than two points.
fn points_of(drawing: &Drawing) -> (usize, usize) {
    let lines: Vec<usize> = drawing
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Line { points, .. } if points.len() > 2 => Some(points.len()),
            _ => None,
        })
        .collect();
    (lines.len(), lines.iter().sum())
}

#[test]
fn a_figure_is_bounded_in_series_and_its_data_is_streamed() {
    let dir = scratch("bounded");
    let (towers, shots) = (1000, 4000);
    let mut engine = Engine::new();
    engine.set_patience(LONG);
    figure::install(&mut engine);
    engine.apply(cav_engine::Command::load(slab_scene(towers, shots))).unwrap();
    engine.run_line("select tower 250").unwrap();
    assert!(engine.settle(LONG), "the signal did not arrive");
    let signal = std::sync::Arc::clone(engine.signal().1.unwrap());
    assert_eq!((signal.towers, signal.times.len()), (towers, shots));
    // The whole export of an SVG: the drawing, the file, the data and the record.
    let file = dir.join("sig.svg");
    let line = format!("figure signal to \"{}\"", file.display().to_string().replace('\\', "/"));
    let started = Instant::now();
    let (answer, peak) = peak_during(|| engine.run_line(&line));
    let took = started.elapsed();
    answer.unwrap();
    let svg_bytes = std::fs::metadata(&file).unwrap().len() as usize;
    let data_bytes = std::fs::metadata(dir.join("sig.npy")).unwrap().len() as usize;
    println!(
        "1000 towers × 4000 shots: the export held {:.2} MiB at most beyond the signal and took {:.2} s; \
         the SVG is {svg_bytes} bytes, the data {data_bytes}",
        peak as f64 / MIB as f64,
        took.as_secs_f64()
    );
    assert!(peak <= EXPORT_BYTES, "the export held {peak} bytes");
    assert!(svg_bytes < SVG_BYTES, "the SVG is {svg_bytes} bytes");
    // The picture: twelve towers' lines, the selected one among them, and it says so.
    let text = std::fs::read_to_string(&file).unwrap();
    let checked = svg::check(&text).unwrap();
    assert!(
        checked.labels.iter().any(|label| label.contains("12 of 1000 towers drawn")),
        "{:?}",
        checked.labels
    );
    let record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("sig.json")).unwrap()).unwrap();
    let drawn: Vec<u64> = (1..=11).chain([250]).collect();
    assert_eq!(record["files"][0]["marks"]["towers"]["drawn"], serde_json::json!(drawn));
    assert_eq!(record["files"][0]["marks"]["towers"]["in_the_scene"], 1000);
    let note = record["files"][0]["marks"]["towers"]["note"].as_str().unwrap();
    assert!(note.starts_with("12 of 1000 towers drawn: 1–11 and 250 "), "{note}");
    let series = record["files"][0]["series"].as_array().unwrap();
    assert_eq!(series.len(), 12);
    assert_eq!(series[11]["name"], "t249");
    assert_eq!(
        (&series[11]["x_column"], &series[11]["y_column"], &series[11]["taken_column"]),
        (&serde_json::json!(747), &serde_json::json!(748), &serde_json::json!(749))
    );
    assert_eq!(series[11]["y"], record["data"]["columns"][748]["name"]);
    // The data: every tower, every shot, as .npy, read here a value at a time.
    assert_eq!(record["data"]["format"], "npy");
    assert_eq!(record["data"]["rows"], shots);
    assert_eq!(record["data"]["columns"].as_array().unwrap().len(), 3 * towers);
    assert!(!dir.join("sig.csv").exists());
    let mut data = std::fs::File::open(dir.join("sig.npy")).unwrap();
    let header = data_bytes - 8 * shots * 3 * towers;
    assert!((64..=256).contains(&header), "a header of {header} bytes");
    let mut at = |row: usize, column: usize| {
        data.seek(SeekFrom::Start((header + 8 * (row * 3 * towers + column)) as u64)).unwrap();
        let mut bytes = [0u8; 8];
        data.read_exact(&mut bytes).unwrap();
        f64::from_le_bytes(bytes)
    };
    let channel = cav_core::forward::Channel::Gradiometer;
    for (row, tower) in [(0, 0), (1, 999), (3999, 249), (2500, 613)] {
        assert_eq!(at(row, 3 * tower).to_bits(), signal.readout(tower, row).to_bits());
        assert_eq!(at(row, 3 * tower + 1).to_bits(), signal.channel(tower, channel)[row].to_bits());
        assert_eq!(at(row, 3 * tower + 2), 1.0);
    }
    // The same figure built here, with no file: twelve series of at most 2002 points.
    let asked = Figure::new(FigureKind::Signal, FigureFormat::Svg);
    let built = figure::build(&engine, &asked, "sig.svg").unwrap();
    let (lines, points) = points_of(&built.drawing);
    assert_eq!(lines, 12);
    assert!(points <= 12 * (plots::MAX_POINTS + 2), "{points} points");
    let table = built.table.as_ref().unwrap();
    assert_eq!((table.rows(), table.columns.len()), (shots, 3 * towers));

    // Control, the cap: the same plot with the cap lifted is every tower's line. Built here, it
    // holds more than the export may, and more points than 2 MB of SVG can say (a point is at
    // least four characters there: two digits, a comma and a space).
    let style = cav_plot::style::Style::preset("paper-column").unwrap();
    let size = style.size_mm.map(|mm| (mm / cav_plot::style::MM_PER_POINT) as f32);
    let lifted = plots::Subject { most_series: usize::MAX, ..plots::Subject::new(0, PaneContent::Signal) };
    let (plot, uncapped) = peak_during(|| plots::build(&engine, &lifted, &style, &Outlines, size).unwrap());
    let (lines, points) = points_of(&plot.drawing);
    println!(
        "with the cap lifted: {lines} series, {points} points, {:.1} MiB to build",
        uncapped as f64 / MIB as f64
    );
    assert_eq!(lines, towers);
    assert!(points * 4 > SVG_BYTES, "{points} points");
    assert!(uncapped > EXPORT_BYTES, "building every series held only {uncapped} bytes");
    drop(plot);
    // Control, the streaming: the table as it was held before, every column in memory, is more
    // than the export may hold by itself.
    let (held, unstreamed) =
        peak_during(|| (0..table.columns.len()).map(|column| table.column(column)).collect::<Vec<_>>());
    println!("the table held in memory: {:.1} MiB", unstreamed as f64 / MIB as f64);
    assert_eq!(held.len(), 3 * towers);
    assert!(unstreamed >= 8 * shots * 3 * towers && unstreamed > EXPORT_BYTES);
    drop(held);
    // (The data file is 96 MB: it is not left behind.)
    std::fs::remove_file(dir.join("sig.npy")).unwrap();
}
