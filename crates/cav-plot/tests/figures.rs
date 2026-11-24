//! Figures: the files a `figure` command writes, their sizes, their data and that they come out
//! the same every time.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use cav_engine::{ChannelName, Engine, Figure, FigureFormat, FigureKind, Query, Reply};
use cav_plot::drawing::Item;
use cav_plot::style::{MM_PER_POINT, PRESETS, SHOT_NOISE_FLOORS, Style, ULDM_FREQUENCY};
use cav_plot::{figure, pdf, plots, svg};

const LONG: Duration = Duration::from_secs(300);

/// An empty directory of this test's own, under the build's.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/figures").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An engine that draws figures, after `script`.
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

/// A console word for a path: quoted, with forward slashes.
fn word(path: &Path) -> String {
    format!("\"{}\"", path.display().to_string().replace('\\', "/"))
}

/// The session whose figures must come out the same: one of every plot, in SVG and PDF.
fn session(dir: &Path, tower: usize, mass: &str) -> Engine {
    let mut engine = engine(&format!("load comb; set duration 400; set bodies[0].mass {mass}; seek 12"));
    for line in [
        format!("figure periodogram tower {tower} as paper-column to {}", word(&dir.join("periodogram.svg"))),
        format!(
            "figure periodogram tower {tower} as paper-full to {}",
            word(&dir.join("periodogram-full.pdf"))
        ),
        format!("figure signal as slide to {}", word(&dir.join("signal.svg"))),
        format!(
            "figure signal tower {tower} lower from 20 to 90 as paper-column to {}",
            word(&dir.join("one.pdf"))
        ),
        format!("figure cadence tower {tower} as paper-full to {}", word(&dir.join("cadence.pdf"))),
        format!("figure flight as poster size 300x120 to {}", word(&dir.join("flight.svg"))),
    ] {
        engine.run_line(&line).unwrap_or_else(|why| panic!("`{line}`: {why}"));
    }
    engine
}

/// How many files of the two directories differ in any byte, counting one that is missing.
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
    test: validation_figure_regenerates,
    name: "figure.regenerates",
    subsystem: "figure",
    slice: "S2",
    reference: Exact,
    against: "six figures of the comb (periodogram, signal, one tower's lower channel over a span, cadence window, flight diagram; SVG and PDF; four presets) written by two engines from the same commands: each figure, its .csv and its .json; files that differ in any byte (each .json's provenance, the build's revision and dirty flag, among them)",
    unit: Count,
    tolerance: 0.0,
    measure: {
        let root = scratch("regenerates");
        let dirs: Vec<PathBuf> = ["first", "second", "other tower", "other mass"].iter().map(|name| root.join(name)).collect();
        for dir in &dirs {
            std::fs::create_dir_all(dir).unwrap();
        }
        let mass = engine("load comb").spec().bodies[0].mass;
        session(&dirs[0], 2, &format!("{mass:?}"));
        session(&dirs[1], 2, &format!("{mass:?}"));
        let files = files_of(&dirs[0]);
        // Six figures, each with its data and its provenance.
        assert_eq!(files.len(), 18, "{:?}", files.keys().collect::<Vec<_>>());
        for name in ["periodogram.svg", "periodogram.csv", "periodogram.json", "periodogram-full.pdf", "one.csv", "flight.json"] {
            assert!(files.contains_key(name), "no {name}");
        }
        assert!(files.values().all(|bytes| !bytes.is_empty()));
        // The provenance in particular: every figure's .json is the same bytes from the second
        // engine, and what it says of the engine is the build's. (Two engines in one process
        // could not have caught a revision asked of git when the file is written; `cav`'s
        // `cli_provenance_is_the_builds_whatever_the_tree_or_the_directory` runs the binary
        // twice with the tree changed in between, and does.)
        let again = files_of(&dirs[1]);
        for (name, bytes) in files.iter().filter(|(name, _)| name.ends_with(".json")) {
            assert!(again[name] == *bytes, "{name} differs between the two engines");
            let said: serde_json::Value = serde_json::from_slice(bytes).unwrap();
            assert_eq!(said["engine"], cav_engine::build().json(), "{name}");
            assert_eq!(said["numerics"]["engine"], said["engine"], "{name}");
        }
        // Control: another tower changes the pictures and data of that tower's figures; one
        // bit of one number of the scene changes every figure of the signal.
        session(&dirs[2], 3, &format!("{mass:?}"));
        assert!(differing(&dirs[0], &dirs[2]) >= 9, "{}", differing(&dirs[0], &dirs[2]));
        let nudged = f64::from_bits(mass.to_bits() + 1);
        session(&dirs[3], 2, &format!("{nudged:?}"));
        let other = files_of(&dirs[3]);
        assert_ne!(other["periodogram.csv"], files["periodogram.csv"]);
        assert_ne!(other["signal.csv"], files["signal.csv"]);
        // (The flight diagram is of the instrument, not of the body: its data is the same.)
        assert_eq!(other["flight.csv"], files["flight.csv"]);
        cav_validate::Measured::Error(differing(&dirs[0], &dirs[1]) as f64)
    },
}

cav_validate::case! {
    test: validation_gpu_png_figure_regenerates,
    name: "figure.png_regenerates",
    subsystem: "figure",
    slice: "S2",
    reference: Exact,
    against: "the comb's periodogram and signal as PNG figures (paper-column at 300 pixels per inch, slide), painted by two engines on the local adapter, each with a device of its own: the .png, .csv and .json files that differ in any byte",
    unit: Count,
    tolerance: 0.0,
    measure: {
        let root = scratch("png regenerates");
        let run = |name: &str, tower: usize| {
            let dir = root.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            let mut engine = engine("load comb; set duration 400");
            engine.run_line(&format!("figure periodogram tower {tower} as paper-column dpi 300 to {}", word(&dir.join("p.png")))).unwrap();
            engine.run_line(&format!("figure signal as slide to {}", word(&dir.join("s.png")))).unwrap();
            dir
        };
        let (first, second, other) = (run("first", 2), run("second", 2), run("other", 3));
        let files = files_of(&first);
        assert_eq!(files.len(), 6);
        assert!(files["p.png"].starts_with(b"\x89PNG") && files["s.png"].len() > 10_000);
        // Control: another tower's periodogram is another picture.
        assert_ne!(files_of(&other)["p.png"], files["p.png"]);
        cav_validate::Measured::Error(differing(&first, &second) as f64)
    },
}

#[test]
fn a_figure_is_the_stated_size() {
    let dir = scratch("sizes");
    let mut engine = engine("load comb; set duration 200");
    for preset in PRESETS {
        let stated = Style::preset(preset).unwrap().size_mm;
        for (format, size) in
            [("svg", None), ("pdf", None), ("svg", Some([120.0, 80.0])), ("pdf", Some([33.3, 211.7]))]
        {
            let file = dir.join(format!("{preset}.{format}"));
            let sized = size.map_or_else(String::new, |[w, h]: [f64; 2]| format!(" size {w}x{h}"));
            engine.run_line(&format!("figure periodogram as {preset}{sized} to {}", word(&file))).unwrap();
            let expected = size.unwrap_or(stated);
            let bytes = std::fs::read(&file).unwrap();
            let found = if format == "svg" {
                let checked = svg::check(&String::from_utf8(bytes).unwrap()).unwrap();
                // The viewBox is the same size in points: one unit of the file is a point.
                for side in 0..2 {
                    assert!((checked.view_box[side + 2] * MM_PER_POINT - expected[side]).abs() < 0.01);
                }
                checked.size_mm
            } else {
                let media = pdf::check(&bytes).unwrap().media_box;
                assert_eq!([media[0], media[1]], [0.0, 0.0]);
                [media[2] * MM_PER_POINT, media[3] * MM_PER_POINT]
            };
            for side in 0..2 {
                assert!(
                    (found[side] - expected[side]).abs() < 0.01,
                    "{preset} {format}: {found:?} mm, stated {expected:?}"
                );
            }
            // The provenance says the same.
            let json: serde_json::Value =
                serde_json::from_slice(&std::fs::read(file.with_extension("json")).unwrap()).unwrap();
            let name = format!("{preset}.{format}");
            let entry = picture(&json, &name);
            assert_eq!(entry["size_mm"], serde_json::json!(expected));
            assert_eq!(entry["preset"], preset);
        }
    }
    // The four light presets are the widths the plan states.
    for (preset, width) in
        [("paper-column", 85.0), ("paper-full", 178.0), ("slide", 254.0), ("poster", 400.0)]
    {
        let text = std::fs::read_to_string(dir.join(format!("{preset}.svg")));
        drop(text);
        assert_eq!(Style::preset(preset).unwrap().size_mm[0], width);
    }
    // Control: a figure of another size is found to be another size.
    let text = std::fs::read_to_string(dir.join("slide.svg")).unwrap();
    assert!((svg::check(&text).unwrap().size_mm[0] - 85.0).abs() > 30.0);
    // And a preset that does not exist is refused, by name.
    let why =
        engine.run_line(&format!("figure periodogram as a4 to {}", word(&dir.join("x.svg")))).unwrap_err();
    assert!(why.contains("`a4` is not a figure preset"), "{why}");
    assert!(!dir.join("x.svg").exists() && !dir.join("x.json").exists());
}

/// The entry of the picture `name` in a figure's record.
fn picture<'a>(record: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    record["files"]
        .as_array()
        .and_then(|files| files.iter().find(|entry| entry["file"] == name))
        .unwrap_or_else(|| panic!("the record lists no {name}: {}", record["files"]))
}

/// A CSV figure's data: the header's fields and each column's values.
fn read_csv(path: &Path) -> (Vec<String>, Vec<Vec<f64>>) {
    let text = std::fs::read_to_string(path).unwrap();
    let mut lines = text.lines();
    let header: Vec<String> = lines.next().unwrap().split(',').map(str::to_owned).collect();
    let mut columns = vec![Vec::new(); header.len()];
    for line in lines {
        for (column, field) in columns.iter_mut().zip(line.split(',')) {
            column.push(field.parse::<f64>().unwrap_or_else(|_| panic!("`{field}` is not a number")));
        }
    }
    (header, columns)
}

fn same_bits(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[test]
fn figure_data_is_the_plotted_data() {
    let dir = scratch("data");
    // The staggered array with shots lost: every tower has its own times, and some are not taken.
    let mut engine = engine("load cadence-lab; set duration 300");
    let towers = engine.spec().array.towers.len();
    engine.run_line(&format!("figure signal as paper-full to {}", word(&dir.join("signal.svg")))).unwrap();
    let (header, columns) = read_csv(&dir.join("signal.csv"));
    assert_eq!(columns.len(), 3 * towers);
    let signal = engine.signal().1.unwrap().clone();
    let gradiometer = cav_core::forward::Channel::Gradiometer;
    for tower in 0..towers {
        let label = &engine.spec().array.towers[tower].label;
        assert_eq!(header[3 * tower], format!("readout time of {label} [s]"));
        assert_eq!(header[3 * tower + 1], format!("ΔΦ of {label} [rad]"));
        assert!(same_bits(&columns[3 * tower], &signal.readouts(tower)), "{label}'s times");
        assert!(same_bits(&columns[3 * tower + 1], signal.channel(tower, gradiometer)), "{label}'s values");
        let taken: Vec<f64> = signal.taken(tower).iter().map(|t| f64::from(u8::from(*t))).collect();
        assert_eq!(columns[3 * tower + 2], taken);
    }
    assert!(columns[2].contains(&0.0) && columns[2].contains(&1.0), "the scene loses some shots");
    // The periodogram: the engine's frequencies and power, and the root that is drawn.
    engine
        .run_line(&format!("figure periodogram tower 2 as paper-column to {}", word(&dir.join("p.pdf"))))
        .unwrap();
    let (header, columns) = read_csv(&dir.join("p.csv"));
    assert_eq!(header, ["frequency [Hz]", "S [rad²/Hz]", "√S [rad/√Hz]"]);
    let asked = Query::Periodogram { tower: 2, channel: ChannelName::Gradiometer, compared: false };
    let Reply::Periodogram(spectrum) = engine.query(&asked).unwrap() else { panic!("a periodogram") };
    assert!(same_bits(&columns[0], &spectrum.frequencies) && same_bits(&columns[1], &spectrum.power));
    assert!(same_bits(&columns[2], &spectrum.power.iter().map(|s| s.sqrt()).collect::<Vec<_>>()));
    // The .json names the columns each series is, and the axes with their units and scales.
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("p.json")).unwrap()).unwrap();
    assert_eq!(json["data"]["file"], "p.csv");
    assert_eq!(json["data"]["rows"], spectrum.frequencies.len());
    assert_eq!(json["files"][0]["series"][0]["x"], "frequency");
    assert_eq!(json["files"][0]["series"][0]["y"], "√S");
    assert_eq!(json["files"][0]["axes"]["y"]["scale"], "log");
    assert_eq!(json["files"][0]["axes"]["y"]["unit"], "rad/√Hz");
    assert_eq!(json["files"][0]["axes"]["x"]["range"], serde_json::json!([0.0, spectrum.nyquist]));
    assert_eq!(json["scene"]["spec"], serde_json::to_value(engine.spec()).unwrap());
    assert_eq!(json["engine"]["revision"], cav_engine::build().revision);
    assert_eq!(json["numerics"], cav_sim::bundle::numerics(engine.spec(), cav_engine::build()));
    assert_eq!(json["files"][0]["command"]["console"], "figure periodogram tower 2 as paper-column to p.pdf");
    assert_eq!(
        json["files"][0]["command"]["cli"],
        "cav figure <a .ron file holding the scene in this file's `scene.spec`> periodogram --out p.pdf --preset paper-column --tower 2"
    );
    // Nothing in it is a time of day: no key or value speaks of one.
    let text = std::fs::read_to_string(dir.join("p.json")).unwrap();
    for word in ["date", "created", "timestamp", "time\""] {
        assert!(!text.to_lowercase().contains(&format!("\"{word}")), "the provenance has a `{word}…`");
    }
    // Control: six significant figures would not be the engine's numbers …
    let short: Vec<f64> = spectrum.power.iter().map(|s| format!("{s:.5e}").parse().unwrap()).collect();
    assert!(!same_bits(&short, &spectrum.power));
    // … and another tower's figure holds other numbers.
    engine
        .run_line(&format!("figure periodogram tower 3 as paper-column to {}", word(&dir.join("q.pdf"))))
        .unwrap();
    assert!(!same_bits(&read_csv(&dir.join("q.csv")).1[1], &spectrum.power));
    // An unedited library scene's figure says the one line that makes it again.
    let mut library = self::engine("load comb");
    library.run_line(&format!("figure cadence as slide to {}", word(&dir.join("c.svg")))).unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("c.json")).unwrap()).unwrap();
    assert_eq!(
        json["files"][0]["command"]["cli"],
        "cav figure comb cadence --out c.svg --preset slide --tower 1"
    );
}

/// The record beside the figures of stem `stem` in `dir`.
fn record_of(dir: &Path, stem: &str) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(dir.join(format!("{stem}.json"))).unwrap()).unwrap()
}

/// The names a record lists under `key`.
fn names(record: &serde_json::Value, key: &str) -> Vec<String> {
    let entries = record[key].as_array().unwrap_or_else(|| panic!("the record has no `{key}`"));
    entries.iter().map(|entry| entry["file"].as_str().or(entry.as_str()).unwrap().to_owned()).collect()
}

#[test]
fn a_figures_record_lists_every_format_written_of_it() {
    // Finding 26 of S2's review: x.svg, x.pdf and x.png left one x.json, describing the last.
    let (dir, other) = (scratch("record"), scratch("record other order"));
    let mut engine = engine("load comb; set duration 200");
    let line = |dir: &Path, tower: usize, preset: &str, file: &str| {
        format!("figure periodogram tower {tower} as {preset} to {}", word(&dir.join(file)))
    };
    engine.run_line(&line(&dir, 2, "paper-column", "x.svg")).unwrap();
    let alone = std::fs::read(dir.join("x.json")).unwrap();
    assert_eq!(names(&record_of(&dir, "x"), "files"), ["x.svg"]);
    engine.run_line(&line(&dir, 2, "slide", "x.pdf")).unwrap();
    // One record, both pictures, in the order of their names; each entry is its own picture's.
    let record = record_of(&dir, "x");
    assert_eq!(record["cavendish_figure"], 2);
    assert_eq!(names(&record, "files"), ["x.pdf", "x.svg"]);
    assert_eq!(names(&record, "not_this_figure"), Vec::<String>::new());
    let (pdf, svg) = (picture(&record, "x.pdf"), picture(&record, "x.svg"));
    assert_eq!(
        (&pdf["format"], &pdf["preset"], &pdf["size_mm"][0]),
        (&"pdf".into(), &"slide".into(), &254.0.into())
    );
    assert_eq!(
        (&svg["format"], &svg["preset"], &svg["size_mm"][0]),
        (&"svg".into(), &"paper-column".into(), &85.0.into())
    );
    assert_eq!(pdf["command"]["console"], "figure periodogram tower 2 as slide to x.pdf");
    assert!(pdf["pixels"].is_null() && pdf["dpi"].is_null(), "a vector picture has no pixels");
    // What is the figure's, whatever the format, is said once: the data, the scene, the build.
    assert_eq!(record["data"]["file"], "x.csv");
    assert_eq!(record["subject"]["tower"], 2);
    assert_eq!(record["scene"]["spec"], serde_json::to_value(engine.spec()).unwrap());
    assert!(pdf["data"].is_null() && pdf["scene"].is_null());
    // The same command again changes no byte of the record.
    let both = std::fs::read(dir.join("x.json")).unwrap();
    engine.run_line(&line(&dir, 2, "slide", "x.pdf")).unwrap();
    assert!(std::fs::read(dir.join("x.json")).unwrap() == both);
    // Written in the other order, somewhere else: the same record, byte for byte.
    engine.run_line(&line(&other, 2, "slide", "x.pdf")).unwrap();
    engine.run_line(&line(&other, 2, "paper-column", "x.svg")).unwrap();
    assert!(std::fs::read(other.join("x.json")).unwrap() == both, "the order of writing shows in the record");
    // Control: the record as it was written before, the last picture's alone, is not this one.
    assert!(alone != both && alone.len() < both.len());
    // A picture that is no longer there is not listed.
    std::fs::remove_file(other.join("x.pdf")).unwrap();
    engine.run_line(&line(&other, 2, "paper-column", "x.svg")).unwrap();
    assert_eq!(names(&record_of(&other, "x"), "files"), ["x.svg"]);
    assert!(std::fs::read(other.join("x.json")).unwrap() == alone);
    // Another tower's figure under the same name: the record and the data become that figure's,
    // and the picture left behind is named as not this figure, never as a file of it.
    let before = std::fs::read(dir.join("x.csv")).unwrap();
    engine.run_line(&line(&dir, 3, "slide", "x.pdf")).unwrap();
    let record = record_of(&dir, "x");
    assert_eq!(names(&record, "files"), ["x.pdf"]);
    assert_eq!(names(&record, "not_this_figure"), ["x.svg"]);
    assert_eq!(record["subject"]["tower"], 3);
    assert_ne!(std::fs::read(dir.join("x.csv")).unwrap(), before, "the data is the new figure's");
    // Written again, it is this figure's, and the list is empty: as if it had all been written afresh.
    engine.run_line(&line(&dir, 3, "paper-column", "x.svg")).unwrap();
    let record = record_of(&dir, "x");
    assert_eq!(names(&record, "files"), ["x.pdf", "x.svg"]);
    assert_eq!(names(&record, "not_this_figure"), Vec::<String>::new());
    let afresh = scratch("record afresh");
    engine.run_line(&line(&afresh, 3, "slide", "x.pdf")).unwrap();
    engine.run_line(&line(&afresh, 3, "paper-column", "x.svg")).unwrap();
    assert!(std::fs::read(afresh.join("x.json")).unwrap() == std::fs::read(dir.join("x.json")).unwrap());
    // One bit of the scene is another figure too.
    let mass = engine.spec().bodies[0].mass;
    engine.run_line(&format!("set bodies[0].mass {:?}", f64::from_bits(mass.to_bits() + 1))).unwrap();
    assert!(engine.settle(LONG));
    engine.run_line(&line(&dir, 3, "slide", "x.pdf")).unwrap();
    assert_eq!(names(&record_of(&dir, "x"), "not_this_figure"), ["x.svg"]);
    // A record that is not one (an older layout, a file of something else) is replaced; and a
    // name in it that is a path is never looked for.
    std::fs::write(
        dir.join("x.json"),
        r#"{"cavendish_figure": 1, "file": "x.svg", "files": [{"file": "../x.svg"}]}"#,
    )
    .unwrap();
    engine.run_line(&line(&dir, 3, "slide", "x.pdf")).unwrap();
    let record = record_of(&dir, "x");
    assert_eq!(
        (names(&record, "files"), names(&record, "not_this_figure")),
        (vec!["x.pdf".to_owned()], vec![])
    );
    std::fs::write(dir.join("x.json"), "not a record").unwrap();
    engine.run_line(&line(&dir, 3, "slide", "x.pdf")).unwrap();
    assert_eq!(names(&record_of(&dir, "x"), "files"), ["x.pdf"]);
}

#[test]
fn gpu_a_figures_record_covers_svg_pdf_and_png() {
    // The three formats of one stem, as the demo writes them: one record, three entries, the
    // PNG's with its pixels and resolution; in any order the same bytes.
    let dirs = [scratch("record three"), scratch("record three reversed")];
    let mut engine = engine("load comb; set duration 200");
    let mut formats = ["svg", "pdf", "png"];
    for dir in &dirs {
        for format in formats {
            let file = word(&dir.join(format!("fig.{format}")));
            engine
                .run_line(&format!("figure periodogram tower 2 as paper-column dpi 150 to {file}"))
                .unwrap();
        }
        formats.reverse();
    }
    let record = record_of(&dirs[0], "fig");
    assert_eq!(names(&record, "files"), ["fig.pdf", "fig.png", "fig.svg"]);
    let png = picture(&record, "fig.png");
    assert_eq!((&png["pixels"], &png["dpi"]), (&serde_json::json!([502, 354]), &serde_json::json!(150.0)));
    assert!(picture(&record, "fig.svg")["pixels"].is_null());
    let (_, size) = cav_render::decode_png(&std::fs::read(dirs[0].join("fig.png")).unwrap()).unwrap();
    assert_eq!(size, [502, 354], "the record says what the file is");
    assert_eq!(files_of(&dirs[0]).len(), 5, "three pictures, one data file, one record");
    assert_eq!(differing(&dirs[0], &dirs[1]), 0);
    // Control: the last picture's record alone, as it was before, lists one file.
    let figure = Figure { tower: Some(2), ..Figure::new(FigureKind::Periodogram, FigureFormat::Svg) };
    let alone: serde_json::Value =
        serde_json::from_str(&figure::build(&engine, &figure, "fig.svg").unwrap().json()).unwrap();
    assert_eq!(names(&alone, "files"), ["fig.svg"]);
    assert_ne!(alone, record);
}

#[test]
fn the_paper_periodogram_is_the_house_style() {
    let engine = engine("load comb; set duration 400");
    let figure = Figure { tower: Some(2), ..Figure::new(FigureKind::Periodogram, FigureFormat::Svg) };
    let built = figure::build(&engine, &figure, "p.svg").unwrap();
    let style = &built.style;
    assert_eq!(style.name, "paper-column");
    let points = [(85.0 / MM_PER_POINT) as f32, (60.0 / MM_PER_POINT) as f32];
    let subject =
        plots::Subject { tower: Some(2), ..plots::Subject::new(0, cav_engine::PaneContent::Periodogram) };
    let plot = plots::build(&engine, &subject, style, &cav_plot::fonts::Outlines, points).unwrap();
    assert_eq!(plot.drawing, built.drawing, "the figure is the plot, in the preset's style and size");
    let (area, x, y) = (plot.drawn.area, plot.drawn.x, plot.drawn.y);
    // White paper, a logarithmic vertical axis.
    assert!(
        matches!(&built.drawing.items[0], Item::Rect { fill: Some(fill), .. } if fill.css() == "#ffffff")
    );
    assert!(y.log && !x.log);
    // A grey level at each shot-noise floor, right across the axes, each named.
    for floor in SHOT_NOISE_FLOORS {
        let level = plots::place(area, &x, &y, (0.0, floor)).y;
        let drawn = built.drawing.items.iter().any(|item| match item {
            Item::Line { points, colour, dash, .. } => {
                points.len() == 2
                    && *colour == style.reference
                    && dash.is_empty()
                    && (points[0][1] - level).abs() < 0.01
                    && (points[1][1] - level).abs() < 0.01
                    && (points[0][0] - area.left()).abs() < 0.01
                    && (points[1][0] - area.right()).abs() < 0.01
            }
            _ => false,
        });
        assert!(drawn, "no level at {floor} rad/√Hz");
    }
    let texts = built.drawing.texts();
    // They are named in the key above the axes, beside a sample of their grey.
    assert!(texts.contains(&"shot-noise floors, 100 and 10 µrad/√Hz"), "{texts:?}");
    let (_, named) =
        plot.drawn.labels.iter().find(|(text, _)| text.starts_with("shot-noise floors")).unwrap();
    assert!(named.bottom() <= area.top(), "{named:?}");
    assert!(texts.contains(&"√S, rad/√Hz") && texts.iter().any(|t| t.starts_with("frequency, Hz")));
    // A stem for every harmonic of the comb, from the axis up: solid, or dashed if it is folded.
    let Reply::Periodogram(spectrum) = engine
        .query(&Query::Periodogram { tower: 2, channel: ChannelName::Gradiometer, compared: false })
        .unwrap()
    else {
        panic!("a periodogram")
    };
    let harmonics = &spectrum.combs[0].harmonics;
    assert!(harmonics.iter().any(|h| h.folded) && harmonics.iter().any(|h| !h.folded));
    for harmonic in harmonics {
        let at = plots::place(area, &x, &y, (harmonic.appears_at, y.lo)).x;
        let stems: Vec<bool> = built
            .drawing
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Line { points, colour, dash, .. }
                    if points.len() == 2
                        && *colour == style.comb(0)
                        && (points[0][0] - at).abs() < 0.01
                        && (points[1][0] - at).abs() < 0.01
                        && (points[0][1] - area.bottom()).abs() < 0.01 =>
                {
                    Some(!dash.is_empty())
                }
                _ => None,
            })
            .collect();
        // (Two harmonics may stand in one place: the sixth folds onto the fourth.)
        assert!(
            stems.contains(&harmonic.folded),
            "harmonic {} at {} Hz: {stems:?}",
            harmonic.n,
            harmonic.appears_at
        );
    }
    // A diamond at the ULDM benchmark: at m c²/h, as high as a tone of 1e-8 rad stands over
    // this run (400 s: 1e-8 · √400 / 2 = 1e-7 rad/√Hz), and named.
    let height = plots::uldm_height(spectrum.shots, spectrum.nyquist);
    assert!((height / 1e-7 - 1.0).abs() < 1e-12, "{height}");
    let expected = plots::place(area, &x, &y, (ULDM_FREQUENCY, height));
    let diamonds: Vec<[f32; 2]> = built
        .drawing
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Diamond { centre, .. } => Some(*centre),
            _ => None,
        })
        .collect();
    // Two diamonds: the mark on the axes, and its sample in the key above them, beside its name.
    assert_eq!(diamonds.len(), 2);
    assert!((diamonds[0][0] - expected.x).abs() < 0.01 && (diamonds[0][1] - expected.y).abs() < 0.01);
    assert!(diamonds[1][1] < area.top(), "the key's diamond is above the axes");
    // Named in the key as a reference, as the Python redraw names it (S2's second review, item 9).
    let named =
        built.provenance["marks"]["reference"]["uldm_benchmark"]["named_as"].as_str().unwrap().to_owned();
    assert!(named.starts_with("ULDM benchmark (reference"), "{named}");
    assert!(area.contains(expected) && texts.contains(&named.as_str()), "{texts:?}");
    assert_eq!(built.provenance["marks"]["reference"]["uldm_benchmark"]["named_in_the_key"], true);
    // Review 29: no label by a mark meets a stem, a dot or a diamond but its own.
    assert!(!plot.drawn.marked.is_empty(), "the harmonics are labelled");
    for (text, took, own) in &plot.drawn.marked {
        for (n, mark) in plot.drawn.marks.iter().enumerate() {
            assert!(
                own.contains(&n) || !took.intersects(*mark),
                "`{text}` at {took:?} meets mark {n} at {mark:?}"
            );
        }
    }
    // Control: where the rule before put "ULDM benchmark" (to the right of its mark, a line
    // above it), the rectangle meets the stem of the comb's second harmonic.
    let u = style.unit();
    let [w, h] = cav_plot::drawing::Measure::size(
        &cav_plot::fonts::Outlines,
        "ULDM benchmark",
        cav_plot::drawing::Face::Sans,
        style.label_size,
    );
    let before = egui::Rect::from_min_size(
        egui::pos2(expected.x + 3.0 * u, expected.y - 3.0 * u - h),
        egui::vec2(w, h),
    );
    let second = plot.drawn.stems.iter().find(|stem| stem.1 == 2).unwrap().2;
    let stem = plot
        .drawn
        .marks
        .iter()
        .find(|mark| (mark.center().x - second).abs() < 0.5 && mark.height() > 5.0)
        .unwrap();
    assert!(before.intersects(*stem), "the old place {before:?} is clear of the 2f₀ stem {stem:?}");
    // The provenance says the mark is the plan's reference, not something simulated.
    let mark = &built.provenance["marks"]["reference"]["uldm_benchmark"];
    assert!(
        mark["note"].as_str().unwrap().starts_with("a reference mark from the plan, not a simulated signal")
    );
    assert_eq!(mark["frequency_hz"], ULDM_FREQUENCY);
    assert_eq!(mark["height_rad_per_sqrt_hz"], height);
    assert_eq!(built.provenance["marks"]["combs"][0]["stems"].as_array().unwrap().len(), harmonics.len());
    // Nothing the figure writes leaves the page or lies on another label.
    let page = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(points[0], points[1]));
    for (k, (text, took)) in plot.drawn.labels.iter().enumerate() {
        assert!(page.contains_rect(*took), "`{text}` at {took:?} leaves the page");
        for (other, there) in &plot.drawn.labels[k + 1..] {
            assert!(!took.intersects(*there), "`{text}` and `{other}` are written over each other");
        }
    }
    // Control: off the axis the benchmark is not drawn, and the provenance does not claim it …
    let zoomed = Figure { span: Some([0.1, 0.25]), ..figure.clone() };
    let built = figure::build(&engine, &zoomed, "z.svg").unwrap();
    assert!(!built.drawing.items.iter().any(|item| matches!(item, Item::Diamond { .. })));
    assert!(built.provenance["marks"]["reference"]["uldm_benchmark"].is_null());
    // … and the application's own style has neither the floors nor the mark.
    let screen = Figure { preset: "screen".into(), ..figure };
    let built = figure::build(&engine, &screen, "s.svg").unwrap();
    assert!(!built.drawing.items.iter().any(|item| matches!(item, Item::Diamond { .. })));
    assert!(!built.drawing.texts().iter().any(|text| text.starts_with("shot-noise")));
}

#[test]
fn a_figure_is_bounded_in_points_and_pixels() {
    let dir = scratch("bounds");
    // 21,000 shots of five towers: more rows than a text file is given.
    let mut engine = engine("load comb; set duration 42000");
    let shots = engine.signal().1.unwrap().most();
    assert_eq!(shots, 21_000);
    // A picture's pixels are its size times its resolution, and no side may pass 8192: the
    // refusal has the numbers, and comes before any device is asked for.
    for (line, pixels) in [
        ("figure signal as poster dpi 600 to big.png", "9449 × 5906 pixels"),
        ("figure signal as paper-column size 2000x40 dpi 300 to wide.png", "23622 × 472 pixels"),
    ] {
        let why = engine.run_line(line).unwrap_err();
        assert!(why.contains(pixels) && why.contains("each side is 1 to 8192"), "`{line}`: {why}");
    }
    assert_eq!(figure::pixels([85.0, 60.0], 300.0), Ok([1004, 709]));
    assert_eq!(figure::pixels([693.6, 693.6], 300.0).unwrap(), [8192, 8192]);
    assert!(figure::pixels([693.7, 10.0], 300.0).is_err() && figure::pixels([0.01, 10.0], 72.0).is_err());
    // The same size as a vector figure is fine: it has no pixels.
    engine.run_line(&format!("figure signal as poster to {}", word(&dir.join("big.svg")))).unwrap();
    // The drawing holds at most 2000 points a series (and the two ends of a stretch) …
    let built = figure::build(&engine, &Figure::new(FigureKind::Signal, FigureFormat::Svg), "s.svg").unwrap();
    let series: Vec<usize> = built
        .drawing
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Line { points, .. } if points.len() > 2 => Some(points.len()),
            _ => None,
        })
        .collect();
    // Five towers of 21,000 shots, each drawn as its envelope: two lines (review 29).
    assert_eq!(series.len(), 2 * 5);
    assert!(series.iter().all(|points| *points <= plots::MAX_POINTS + 2), "{series:?}");
    // … while the data beside it keeps every value, as .npy because it is long.
    let table = built.table.as_ref().unwrap();
    assert!(table.is_long() && table.rows() == shots);
    assert!(dir.join("big.npy").exists() && !dir.join("big.csv").exists());
    let array = cav_core::npy::parse(&std::fs::read(dir.join("big.npy")).unwrap()).unwrap();
    let (shape, values) = array.into_f64().unwrap();
    assert_eq!(shape, [shots, 15]);
    let signal = engine.signal().1.unwrap().clone();
    let third: Vec<f64> = (0..shots).map(|row| values[row * 15 + 7]).collect();
    assert!(same_bits(&third, signal.channel(2, cav_core::forward::Channel::Gradiometer)));
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("big.json")).unwrap()).unwrap();
    assert_eq!(json["data"]["format"], "npy");
    assert_eq!(json["data"]["columns"].as_array().unwrap().len(), 15);
    // The periodogram of it: 10,500 bins drawn as at most 2002 points, every bin in the file.
    let periodogram = Figure { tower: Some(1), ..Figure::new(FigureKind::Periodogram, FigureFormat::Pdf) };
    let built = figure::build(&engine, &periodogram, "p.pdf").unwrap();
    let longest = built
        .drawing
        .items
        .iter()
        .map(|item| match item {
            Item::Line { points, .. } => points.len(),
            _ => 0,
        })
        .max()
        .unwrap();
    assert!(longest > 1000 && longest <= plots::MAX_POINTS + 2, "{longest}");
    assert_eq!(built.table.as_ref().unwrap().rows(), shots / 2);
    assert!(!built.table.as_ref().unwrap().is_long(), "10,500 rows are still text");
    // Control: a short run is drawn point for point.
    let short = self::engine("load comb; set duration 400");
    let built = figure::build(&short, &Figure::new(FigureKind::Signal, FigureFormat::Svg), "s.svg").unwrap();
    assert!(
        built
            .drawing
            .items
            .iter()
            .any(|item| matches!(item, Item::Line { points, .. } if points.len() == 200))
    );
}

#[test]
fn the_documented_figure_line_writes_a_figure_and_a_replay_writes_it_elsewhere() {
    let (dir, elsewhere) = (scratch("documented"), scratch("documented replayed"));
    let example = cav_engine::COMMANDS.iter().find(|doc| doc.name == "figure").unwrap().example;
    let mut engine = Engine::new();
    engine.set_patience(LONG);
    figure::install(&mut engine);
    engine.set_exports(cav_engine::Exports::Into(dir.clone()));
    engine.run_line("load comb").unwrap();
    assert_eq!(engine.run_line(example).unwrap(), example, "the line is its own description");
    let files = files_of(&dir);
    assert_eq!(files.keys().collect::<Vec<_>>(), ["fig.csv", "fig.json", "fig.pdf"]);
    pdf::check(&files["fig.pdf"]).unwrap();
    // The journal replayed into another directory writes the same three files there, and
    // nothing where the journal says (which is the working directory).
    let before = std::fs::read(dir.join("fig.pdf")).unwrap();
    let mut replayed = Engine::new();
    replayed.set_patience(LONG);
    figure::install(&mut replayed);
    replayed.set_exports(cav_engine::Exports::Into(elsewhere.clone()));
    for command in engine.journal() {
        replayed.apply(command.clone()).unwrap();
    }
    assert_eq!(files_of(&elsewhere), files);
    assert_eq!(std::fs::read(dir.join("fig.pdf")).unwrap(), before);
    assert!(!Path::new("fig.pdf").exists(), "nothing was written where the line says");
    // Control: without the exporter the same journal is refused at the figure.
    let mut bare = Engine::new();
    bare.set_exports(cav_engine::Exports::Into(elsewhere));
    let refused = engine.journal().iter().map(|command| bare.apply(command.clone())).find_map(Result::err);
    assert!(refused.unwrap().starts_with("no figure exporter is installed"));
}

/// Every file under `dir`, folders included, by its path within it, with its bytes.
fn files_under(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let name = path.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(name, std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

#[test]
fn a_session_with_figures_and_a_bundle_replays_to_the_same_files() {
    // The exit test with figures in it, which `cav-engine` cannot draw: a session that edits,
    // compares, exports figures and a bundle and saves its scene and its journal; the journal
    // read back from disk and replayed into another directory writes the same files.
    let (first, again, other) = (scratch("session"), scratch("session replayed"), scratch("session nudged"));
    let at = |name: &str| word(&first.join(name));
    let mut session = engine("load comb; set duration 300; set array.towers[1].timing.offset 0.25");
    for line in [
        "compare aice".to_owned(),
        format!("figure periodogram tower 2 as paper-column to {}", at("periodogram.svg")),
        format!("export bundle {}", at("bundle")),
        "compare off".to_owned(),
        "set bodies[0].mass 30".to_owned(),
        "seek 40".to_owned(),
        format!("figure signal from 0 to 120 as slide to {}", at("signal.pdf")),
        format!("figure cadence tower 2 as paper-full to {}", at("cadence.svg")),
        format!("save {}", at("scene.ron")),
        format!("journal save {}", at("session.json")),
    ] {
        session.run_line(&line).unwrap_or_else(|why| panic!("`{line}`: {why}"));
    }
    let journal = cav_engine::journal::read(first.join("session.json").to_str().unwrap()).unwrap();
    let replay = |journal: &[cav_engine::Command], into: &Path| {
        let mut replayed = Engine::new();
        replayed.set_patience(LONG);
        figure::install(&mut replayed);
        replayed.set_exports(cav_engine::Exports::Into(into.to_path_buf()));
        for command in journal {
            replayed.apply(command.clone()).unwrap();
        }
        files_under(into)
    };
    // Three figures of three files, a bundle of eighteen and a scene; the journal's own file is
    // the one thing the session wrote that its journal does not say.
    let mut wrote = files_under(&first);
    assert!(wrote.remove("session.json").is_some());
    assert_eq!(wrote.len(), 28, "{:?}", wrote.keys().collect::<Vec<_>>());
    for name in ["periodogram.json", "signal.csv", "cadence.svg", "bundle/manifest.json", "scene.ron"] {
        assert!(wrote.contains_key(name), "no {name}");
    }
    let replayed = replay(&journal, &again);
    assert_eq!(replayed.keys().collect::<Vec<_>>(), wrote.keys().collect::<Vec<_>>());
    for (name, bytes) in &wrote {
        assert!(replayed[name] == *bytes, "{name} differs between the session and its replay");
    }
    // Control: one bit of one number of the journal, and the replay's files are not the session's.
    let mut nudged = journal.clone();
    let mass = nudged
        .iter()
        .position(
            |command| matches!(command, cav_engine::Command::Set { path, .. } if path == "bodies[0].mass"),
        )
        .unwrap();
    nudged[mass] =
        cav_engine::Command::set("bodies[0].mass", serde_json::json!(f64::from_bits(30.0f64.to_bits() + 1)));
    let changed = replay(&nudged, &other);
    let differing = wrote.iter().filter(|(name, bytes)| changed[*name] != **bytes).count();
    // (What was written before the edit, and the cadence, which is of the schedule, are the same.)
    assert!(differing >= 4, "{differing} files differ");
    assert_eq!(changed["bundle/signal.npy"], wrote["bundle/signal.npy"]);
    assert_ne!(changed["signal.csv"], wrote["signal.csv"]);
}

#[test]
fn gpu_a_png_figure_has_the_stated_pixels() {
    let dir = scratch("pixels");
    let mut engine = engine("load comb; set duration 400");
    for (options, expected) in [
        ("as paper-column", [2008, 1417]),
        ("as paper-column dpi 150", [502, 354]),
        ("as slide", [1500, 844]),
        ("as screen", [960, 600]),
        ("as paper-full size 100x50 dpi 254", [1000, 500]),
    ] {
        let file = dir.join("p.png");
        engine.run_line(&format!("figure periodogram tower 2 {options} to {}", word(&file))).unwrap();
        let (rgba, size) = cav_render::decode_png(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(size, expected, "{options}");
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("p.json")).unwrap()).unwrap();
        assert_eq!(json["files"][0]["pixels"], serde_json::json!(expected));
        // The picture is the figure: its corner is the style's paper, and the spectrum's grey
        // and the first comb's colour are both in it.
        let style = Style::preset(options.split(' ').nth(1).unwrap()).unwrap();
        let has = |colour: cav_plot::drawing::Colour| {
            rgba.chunks(4).filter(|p| (0..3).all(|c| p[c].abs_diff(colour.0[c]) <= 6)).count()
        };
        assert_eq!(&rgba[..3], &style.background.0, "{options}");
        assert!(
            has(style.spectrum) > 50 && has(style.comb(0)) > 50,
            "{options}: {} {}",
            has(style.spectrum),
            has(style.comb(0))
        );
        // Control: a colour the figure does not use is not in it.
        assert_eq!(has(cav_plot::drawing::Colour::hex(0x00_ff00)), 0);
    }
}

#[test]
fn gpu_a_view_figure_is_the_renderers_picture() {
    let dir = scratch("view");
    let mut engine = engine("load comb; view top");
    // As a PNG it is the renderer's own picture, at the figure's pixels.
    engine
        .run_line(&format!("figure view as paper-column dpi 100 to {}", word(&dir.join("plan.png"))))
        .unwrap();
    let (rgba, size) = cav_render::decode_png(&std::fs::read(dir.join("plan.png")).unwrap()).unwrap();
    assert_eq!(size, [335, 236]);
    let gpu = cav_gpu::Gpu::new().unwrap();
    let direct = cav_render::Renderer::new(&gpu).render_rgba(&gpu, engine.frame(0).unwrap(), size).unwrap();
    assert!(rgba == direct, "the figure is not the renderer's picture");
    // As an SVG and as a PDF it is that picture inside a page of the stated size; no data file.
    engine
        .run_line(&format!("figure view as paper-column dpi 100 to {}", word(&dir.join("plan.svg"))))
        .unwrap();
    engine
        .run_line(&format!("figure view as paper-column dpi 100 to {}", word(&dir.join("plan.pdf"))))
        .unwrap();
    let text = std::fs::read_to_string(dir.join("plan.svg")).unwrap();
    assert_eq!(svg::check(&text).unwrap().size_mm, [85.0, 60.0]);
    assert!(text.contains("<image ") && text.contains("data:image/png;base64,"));
    let bytes = std::fs::read(dir.join("plan.pdf")).unwrap();
    let checked = pdf::check(&bytes).unwrap();
    assert_eq!(checked.objects, 6);
    let needle = format!(
        "/Width 335 /Height 236 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length {}",
        335 * 236 * 3
    );
    assert!(bytes.windows(needle.len()).any(|window| window == needle.as_bytes()));
    assert!(!dir.join("plan.csv").exists() && !dir.join("plan.npy").exists());
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("plan.json")).unwrap()).unwrap();
    assert_eq!(json["camera"], serde_json::to_value(engine.view(0)).unwrap());
    // Control: another view is another picture.
    engine.run_line("view north").unwrap();
    engine
        .run_line(&format!("figure view as paper-column dpi 100 to {}", word(&dir.join("north.png"))))
        .unwrap();
    assert_ne!(std::fs::read(dir.join("north.png")).unwrap(), std::fs::read(dir.join("plan.png")).unwrap());
}

/// A scene whose towers do not take every shot (as `plots.rs` has it).
const LOSSY: &str = r#"load cadence-lab; set array.schedule.gaps {"on":4,"period":10}; set array.schedule.loss 0.2; set duration 60"#;

#[test]
fn a_flight_figure_says_which_shots_were_taken() {
    let dir = scratch("flight-taken");
    let mut engine = engine(LOSSY);
    engine
        .run_line(&format!("figure flight from 0 to 40 as paper-full to {}", word(&dir.join("f.svg"))))
        .unwrap();
    let (header, columns) = read_csv(&dir.join("f.csv"));
    assert_eq!(
        header,
        ["tower [its number from 1]", "launch [s]", "readout [s]", "taken [1 if taken or 0 if not]"]
    );
    // The rows are the schedule's shots that touch the span, tower by tower, with its flags.
    let shots = cav_sim::read::shots(engine.spec()).unwrap();
    let mut row = 0;
    for tower in 0..shots.towers() {
        for k in shots.touching(tower, [0.0, 40.0]) {
            let taken = f64::from(u8::from(shots.taken(tower, k)));
            let expected = [(tower + 1) as f64, shots.launch(tower, k), shots.readout(tower, k), taken];
            let found = [columns[0][row], columns[1][row], columns[2][row], columns[3][row]];
            assert!(same_bits(&found, &expected), "row {row}: {found:?} against {expected:?}");
            row += 1;
        }
    }
    assert_eq!(row, columns[0].len());
    let lost = columns[3].iter().filter(|taken| **taken == 0.0).count();
    assert!(lost > row / 2, "{lost} of {row} shots not taken");
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("f.json")).unwrap()).unwrap();
    assert_eq!(json["files"][0]["marks"]["not_taken"]["shots"], serde_json::json!(lost));
    // A signal's series name the column that says which shots were taken.
    engine.run_line(&format!("figure signal as paper-full to {}", word(&dir.join("s.svg")))).unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("s.json")).unwrap()).unwrap();
    assert_eq!(json["files"][0]["series"][1]["taken_column"], serde_json::json!(5));
    assert_eq!(json["files"][0]["marks"]["shots_taken"].as_array().unwrap().len(), 5);
    // Control: the plain schedule takes every shot, and the column is all ones.
    let mut plain = self::engine("load comb; set duration 60");
    plain
        .run_line(&format!("figure flight from 0 to 40 as paper-full to {}", word(&dir.join("p.svg"))))
        .unwrap();
    let (_, columns) = read_csv(&dir.join("p.csv"));
    assert!(columns[3].len() >= 100 && columns[3].iter().all(|taken| *taken == 1.0));
}

#[test]
fn gpu_a_lost_shot_is_a_gap_in_the_png() {
    let dir = scratch("gap");
    let mut engine = engine(LOSSY);
    let file = dir.join("lost.png");
    engine.run_line(&format!("figure signal tower 1 as paper-full dpi 150 to {}", word(&file))).unwrap();
    let (rgba, [width, height]) = cav_render::decode_png(&std::fs::read(&file).unwrap()).unwrap();
    // The same figure, built here, says where things are in points; the picture is that, scaled.
    let figure = Figure {
        tower: Some(1),
        preset: "paper-full".into(),
        dpi: Some(150.0),
        ..Figure::new(FigureKind::Signal, FigureFormat::Png)
    };
    let built = figure::build(&engine, &figure, "lost.png").unwrap();
    let style = Style::preset("paper-full").unwrap();
    let scale = width as f32 / built.drawing.size[0];
    let frame = |item: &Item| match item {
        Item::Rect { min, max, fill: None, stroke: Some(_) } => Some((*min, *max)),
        _ => None,
    };
    let (min, max) = built.drawing.items.iter().find_map(frame).expect("the frame of the axes");
    let range = &built.provenance["axes"]["x"]["range"];
    let (from, to) = (range[0].as_f64().unwrap(), range[1].as_f64().unwrap());
    let column_of = |time: f64| {
        ((min[0] + (max[0] - min[0]) * ((time - from) / (to - from)) as f32) * scale).round() as u32
    };
    // The series' colour in a column of pixels, from the top of the axes down to just above
    // the foot (where a hollow square marks a shot not taken).
    let colour = style.series(0).0;
    let (top, foot) = ((min[1] * scale) as u32, (((max[1] - 6.0) * scale) as u32).min(height));
    let inked = |column: u32| {
        (top..foot)
            .filter(|row| {
                let at = ((row * width + column) * 4) as usize;
                (0..3).all(|c| rgba[at + c].abs_diff(colour.0[c]) <= 40)
            })
            .count()
    };
    let signal = engine.signal().1.unwrap().clone();
    let (times, taken) = (signal.readouts(0), signal.taken(0));
    let mut lost = 0;
    for k in 1..times.len() - 1 {
        // A shot not taken with one not taken either side: nothing of the series is near it.
        if !taken[k] && !taken[k - 1] && !taken[k + 1] {
            assert_eq!(inked(column_of(times[k])), 0, "shot {k} at {} s is drawn", times[k]);
            lost += 1;
        }
        // The neighbours are the control: a shot taken has its dot in its column.
        if taken[k] {
            assert!(inked(column_of(times[k])) >= 2, "shot {k} at {} s is not drawn", times[k]);
        }
    }
    assert!(lost >= 5, "{lost} lost shots looked at");
}

#[test]
fn a_figure_export_goes_through_the_files_seam() {
    // Every file a figure export writes (the picture, its data and its record), and every read
    // it makes, goes through `cav_engine::files`, which in a page makes them downloads. Natively
    // the seam's test double (`files::capture`) holds them as a page would: a file written
    // around the seam would land on disc here and be missing from the list.
    let dir = scratch("through-the-seam");
    let mut engine = engine("load comb; seek 12");
    let made_on_disc = |name: &str| dir.join(name).exists();
    for (format, data) in [("svg", "csv"), ("pdf", "csv")] {
        let file = dir.join(format!("comb.{format}"));
        let (said, downloads) = cav_engine::files::capture(|| {
            engine.run_script(&format!("figure periodogram to {}", word(&file)))
        });
        said.unwrap();
        let names: Vec<&str> = downloads.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, [format!("comb.{format}"), format!("comb.{data}"), "comb.json".to_owned()]);
        assert!(
            !made_on_disc(&format!("comb.{format}"))
                && !made_on_disc("comb.json")
                && !made_on_disc("comb.csv"),
            "a file was written around the seam"
        );
        // The bytes are the ones written natively, outside the double.
        engine.run_script(&format!("figure periodogram to {}", word(&file))).unwrap();
        for download in &downloads {
            let native = std::fs::read(dir.join(&download.name)).unwrap();
            assert_eq!(download.bytes, native, "{} differs from the file written natively", download.name);
        }
        for download in &downloads {
            std::fs::remove_file(dir.join(&download.name)).unwrap();
        }
    }
}
