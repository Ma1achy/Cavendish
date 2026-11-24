//! Figures for papers and talks: a plot or a view written as SVG, PDF or PNG at a stated size,
//! with the data behind it and where it came from beside it.
//!
//! A figure `fig.pdf` is three files: the picture; `fig.csv`, every value plotted, in plain
//! columns under a header that names each column and its unit (`fig.npy`, a `(rows, columns)`
//! array, when it is long); and `fig.json`, the figure's record.
//!
//! ## The record, and the same figure in several formats
//! `fig.json` is the record of the figure `fig`: one set of data (what is plotted, of which
//! tower, channel, span and pane, from which scene verbatim, engine build and numerics, in which
//! data file) and, under `files`, every picture written of it, each with what is its own: the
//! format, the preset and the size, a PNG's pixels and resolution, the title, the axes, the
//! series and which columns they are, the marks that are not data, the colours, and the command
//! that makes it again.
//!
//! 1. Writing `fig.pdf` when `fig.json` records the same data adds `fig.pdf` to `files`, or
//!    replaces its entry. The entries are in the order of their names. An entry whose file is no
//!    longer there is dropped.
//! 2. Writing it when `fig.json` records other data (another scene, tower, span or build), or is
//!    not a record this build reads, replaces the record and the data file with the new
//!    figure's. Pictures the old record listed that are still there are not this figure: they
//!    are named under `not_this_figure` until they are written again, and never under `files`.
//! 3. So the record depends on which figures were written and what was in the directory, not on
//!    the order they were written in, and the same command twice changes no byte.
//!
//! Nothing in any of the files is the time of day or the machine's name: the same commands on
//! the same build write the same bytes.
//!
//! The line drawn is thinned to [`crate::plots::MAX_POINTS`] points a series, and a signal draws
//! at most [`crate::plots::MAX_SERIES`] towers; the data file keeps every value of every tower,
//! and is written a row at a time from the engine's own signal ([`Table`]): no second copy of
//! it is made. A PNG is at most [`MAX_PIXELS`] pixels a side.
//!
//! [`install`] gives an engine the exporter, after which `figure …` on its console, `cav
//! figure` and the MCP `figure` tool are all [`cav_engine::Command::Export`].

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use cav_engine::{ChannelName, Engine, Figure, FigureFormat, FigureKind, PaneContent, Query, Reply};
use serde_json::json;

use crate::drawing::{Drawing, Item};
use crate::fonts::Outlines;
use crate::headless::Painter;
use crate::plots::{self, Plot, Subject};
use crate::style::{MM_PER_POINT, Style};

/// The most rows a figure's data has as text; more are written as `.npy`.
pub const CSV_ROWS: usize = 20_000;

/// The most values (rows × columns) a figure's data has as text: what [`CSV_ROWS`] rows of five
/// towers' signal are. A thousand towers' signal is `.npy` however short it is.
pub const CSV_VALUES: usize = 300_000;

/// The most pixels a PNG figure has on a side.
pub const MAX_PIXELS: u32 = 8192;

/// The size of the buffer a data file is written through, bytes.
pub const WRITE_BUFFER: usize = 64 << 10;

/// The version of the layout of a figure's `.json`.
pub const FIGURE_VERSION: u32 = 2;

/// The most bytes of an existing record that are read before a figure is added to it: a record
/// holds a scene (1 MiB at most) and a line for each column.
const RECORD_BYTES: u64 = 16 << 20;

/// Where a table's values are.
#[derive(Clone, Debug, PartialEq)]
enum Values {
    /// Held here, a column at a time: a spectrum, a window, the flights of a span. Each is
    /// bounded by its own cap (S2.md §3.1: A13, A19).
    Held(Vec<Vec<f64>>),
    /// The engine's own signal, shared and not copied: for each of `towers` (from 0) three
    /// columns, its readout times, its values on `channel`, and whether it took each shot.
    Signal { signal: Arc<cav_sim::read::Signal>, towers: Vec<usize>, channel: cav_core::forward::Channel },
}

/// The data behind a figure: columns of equal length. (A signal's towers may have different numbers of shots:\n/// its table has as many rows as the tower with the most, and NaN in a tower's columns past its last shot.) A signal's table is the engine's signal
/// itself, read as it is written out, so a thousand towers' data costs no memory here.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    /// Each column's name and unit.
    pub columns: Vec<(String, String)>,
    values: Values,
}

impl Table {
    /// How many rows the table has.
    #[must_use]
    pub fn rows(&self) -> usize {
        match &self.values {
            Values::Held(columns) => columns.first().map_or(0, Vec::len),
            Values::Signal { signal, .. } => signal.most(),
        }
    }

    /// The value in row `row` of column `column`.
    ///
    /// # Panics
    /// When the table has no such row or column.
    #[must_use]
    pub fn value(&self, row: usize, column: usize) -> f64 {
        match &self.values {
            Values::Held(columns) => columns[column][row],
            Values::Signal { signal, towers, channel } => {
                let tower = towers[column / 3];
                // Each tower has its own shots: a row past a tower's last is no shot of it.
                if row >= signal.count(tower) {
                    return f64::NAN;
                }
                match column % 3 {
                    0 => signal.readout(tower, row),
                    1 => signal.channel(tower, *channel)[row],
                    _ => f64::from(u8::from(signal.taken(tower)[row])),
                }
            }
        }
    }

    /// Column `column`, whole: for a test, or a caller that wants one series.
    ///
    /// # Panics
    /// When the table has no such column.
    #[must_use]
    pub fn column(&self, column: usize) -> Vec<f64> {
        (0..self.rows()).map(|row| self.value(row, column)).collect()
    }

    /// Writes the table as CSV: a header of `name [unit]`, then a row a line. Every number is
    /// written with the fewest digits that read back as the same `f64`; an infinity is `inf` or
    /// `-inf`. A row at a time: hand it a buffered writer.
    ///
    /// # Errors
    /// The writer's.
    pub fn write_csv(&self, writer: &mut impl Write) -> std::io::Result<()> {
        for (n, (name, unit)) in self.columns.iter().enumerate() {
            let field = format!("{name} [{unit}]");
            let comma = if n > 0 { "," } else { "" };
            if field.contains([',', '"', '\n']) {
                write!(writer, "{comma}\"{}\"", field.replace('"', "\"\""))?;
            } else {
                write!(writer, "{comma}{field}")?;
            }
        }
        writeln!(writer)?;
        for row in 0..self.rows() {
            for column in 0..self.columns.len() {
                let comma = if column > 0 { "," } else { "" };
                write!(writer, "{comma}{:?}", self.value(row, column))?;
            }
            writeln!(writer)?;
        }
        Ok(())
    }

    /// Writes the table as a `(rows, columns)` array of `f64` in `.npy`, a value at a time: hand
    /// it a buffered writer.
    ///
    /// # Errors
    /// The writer's.
    pub fn write_npy(&self, writer: &mut impl Write) -> std::io::Result<()> {
        let (rows, columns) = (self.rows(), self.columns.len());
        let values = (0..rows).flat_map(|row| (0..columns).map(move |column| self.value(row, column)));
        cav_core::npy::write_f64(writer, &[rows, columns], values)
    }

    /// Whether the table is written as `.npy`: more than [`CSV_ROWS`] rows, or more than
    /// [`CSV_VALUES`] values.
    #[must_use]
    pub fn is_long(&self) -> bool {
        self.rows() > CSV_ROWS || self.rows().saturating_mul(self.columns.len()) > CSV_VALUES
    }
}

/// A figure, built and not yet written.
#[derive(Clone, Debug, PartialEq)]
pub struct Built {
    /// The picture.
    pub drawing: Drawing,
    /// Its data; a view of the scene has none.
    pub table: Option<Table>,
    /// What the record says of the figure whatever the format: what is plotted, of what, the
    /// scene, the build, the numerics, the data file.
    pub figure: serde_json::Value,
    /// This picture's entry in the record's `files`: its file, format, preset, size, title,
    /// axes, series, marks, colours and command.
    pub provenance: serde_json::Value,
    /// The style it was drawn in.
    pub style: Style,
    /// For a PNG: its width and height, pixels.
    pub pixels: Option<[u32; 2]>,
}

/// The size of a figure in `style`, millimetres: its own if it names one.
fn size_mm(figure: &Figure, style: &Style) -> [f64; 2] {
    figure.size_mm.unwrap_or(style.size_mm)
}

/// The pixels of a picture of `size_mm` millimetres at `dpi` pixels per inch.
///
/// # Errors
/// When a side would be over [`MAX_PIXELS`] or under one pixel, with the numbers.
pub fn pixels(size_mm: [f64; 2], dpi: f64) -> Result<[u32; 2], String> {
    pixels_within(size_mm, dpi, MAX_PIXELS)
}

/// As [`pixels`], with no side over `most`: a caller that hands the picture on in a message (the
/// MCP server) takes fewer than a file may hold.
///
/// # Errors
/// When a side would be over `most` or under one pixel, with the numbers.
pub fn pixels_within(size_mm: [f64; 2], dpi: f64, most: u32) -> Result<[u32; 2], String> {
    let side = |mm: f64| (mm / 25.4 * dpi).round();
    let [w, h] = [side(size_mm[0]), side(size_mm[1])];
    let limit = f64::from(most);
    if !(w >= 1.0 && h >= 1.0 && w <= limit && h <= limit) {
        return Err(format!(
            "a figure of {} × {} mm at {dpi} pixels per inch is {w} × {h} pixels; each side is 1 to {most}: \
             make it smaller (`size`) or coarser (`dpi`)",
            size_mm[0], size_mm[1]
        ));
    }
    Ok([w as u32, h as u32])
}

/// The size and the resolution `figure` has if it is made of pixels (a PNG, or a view of the
/// scene in any format): millimetres and pixels per inch, its own or its preset's. `None` for a
/// plot written as SVG or PDF, which has no pixels.
///
/// # Errors
/// A preset that does not exist.
pub fn raster(figure: &Figure) -> Result<Option<([f64; 2], f64)>, String> {
    if figure.format != FigureFormat::Png && figure.what != FigureKind::View {
        return Ok(None);
    }
    let style = Style::preset(&figure.preset)?;
    Ok(Some((size_mm(figure, &style), figure.dpi.unwrap_or(style.dpi))))
}

fn subject(figure: &Figure, content: PaneContent) -> Subject {
    Subject {
        tower: figure.tower,
        channel: figure.channel,
        span: figure.span,
        title: figure.title,
        ..Subject::new(figure.pane, content)
    }
}

/// The data behind a plot: the engine's own numbers, every one.
fn table(engine: &Engine, figure: &Figure) -> Result<Table, String> {
    let spec = engine.spec();
    let mut names = Vec::new();
    let mut held = Vec::new();
    let mut column = |name: String, unit: &str, values: Vec<f64>| {
        names.push((name, unit.to_owned()));
        held.push(values);
    };
    match figure.what {
        FigureKind::Signal => {
            let signal = engine.signal().1.ok_or("there is no signal")?;
            let towers: Vec<usize> = match figure.tower {
                Some(n) => vec![n - 1],
                None => (0..signal.towers.min(spec.array.towers.len())).collect(),
            };
            let channel = match figure.channel {
                ChannelName::Gradiometer => "ΔΦ",
                ChannelName::Lower => "δφ₁",
                ChannelName::Upper => "δφ₂",
            };
            let mut columns = Vec::with_capacity(3 * towers.len());
            for tower in &towers {
                let label = &spec.array.towers[*tower].label;
                columns.push((format!("readout time of {label}"), "s".to_owned()));
                columns.push((format!("{channel} of {label}"), "rad".to_owned()));
                columns.push((format!("shot taken by {label}"), "1 if taken or 0 if not".to_owned()));
            }
            // The signal itself, shared: its values are read as the file is written.
            let values =
                Values::Signal { signal: Arc::clone(signal), towers, channel: figure.channel.channel() };
            return Ok(Table { columns, values });
        }
        FigureKind::Periodogram => {
            let query = Query::Periodogram {
                tower: figure.tower.unwrap_or(1),
                channel: figure.channel,
                compared: false,
            };
            let Reply::Periodogram(spectrum) = engine.query(&query)? else {
                return Err("the engine did not answer with a periodogram".into());
            };
            let amplitude = spectrum.power.iter().map(|power| power.sqrt()).collect();
            column("frequency".into(), "Hz", spectrum.frequencies);
            column("S".into(), "rad²/Hz", spectrum.power);
            column("√S".into(), "rad/√Hz", amplitude);
        }
        FigureKind::Cadence => {
            let Reply::Cadence(predicted) = engine.query(&Query::Cadence)? else {
                return Err("the engine did not answer with a prediction".into());
            };
            let to = figure.span.map_or(2.5 / predicted.mean_cycle, |span| span[1]);
            let query = Query::Window { tower: figure.tower.unwrap_or(1), to: Some(to) };
            let Reply::Window(window) = engine.query(&query)? else {
                return Err("the engine did not answer with a window".into());
            };
            column("frequency offset from a tone".into(), "Hz", window.frequencies);
            column("|W| relative to the tone".into(), "1", window.window);
        }
        FigureKind::Flight => {
            let span = plots::flight_span(engine, figure.span);
            // The scene's shots, as the diagram draws them: the schedule's, at each tower's times.
            let shots = engine.shots()?;
            let (mut tower, mut launch, mut readout, mut taken) =
                (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            for n in 0..shots.towers().min(plots::MAX_FLIGHT_ROWS) {
                for shot in plots::flights_drawn(shots, n, span).1 {
                    tower.push((n + 1) as f64);
                    launch.push(shot.launch);
                    readout.push(shot.readout);
                    taken.push(f64::from(u8::from(shot.taken)));
                }
            }
            column("tower".into(), "its number from 1", tower);
            column("launch".into(), "s", launch);
            column("readout".into(), "s", readout);
            column("taken".into(), "1 if taken or 0 if not", taken);
        }
        FigureKind::View => {}
    }
    Ok(Table { columns: names, values: Values::Held(held) })
}

/// The `cav figure` line that makes `figure` again from the scene of `engine`, to `file`.
fn cli_line(engine: &Engine, figure: &Figure, file: &str) -> String {
    let spec = engine.spec();
    let library = cav_sim::LIBRARY
        .iter()
        .find(|scene| scene.name == spec.name)
        .and_then(|scene| cav_sim::SceneSpec::from_ron(scene.ron).ok());
    let scene = if library.as_ref() == Some(spec) {
        spec.name.clone()
    } else {
        "<a .ron file holding the scene in this file's `scene.spec`>".to_owned()
    };
    let mut line =
        format!("cav figure {scene} {} --out {file} --preset {}", figure.what.name(), figure.preset);
    if let Some(tower) = figure.tower {
        line.push_str(&format!(" --tower {tower}"));
    }
    if figure.channel != ChannelName::Gradiometer {
        line.push_str(&format!(" --channel {}", figure.channel.name()));
    }
    if let Some([from, to]) = figure.span {
        line.push_str(&format!(" --span {from},{to}"));
    }
    if let Some([width, height]) = figure.size_mm {
        line.push_str(&format!(" --size {width}x{height}"));
    }
    if let Some(dpi) = figure.dpi {
        line.push_str(&format!(" --dpi {dpi}"));
    }
    if !figure.title {
        line.push_str(" --no-title");
    }
    line
}

/// What a figure's record says of it whatever its format, and the start of this picture's entry.
fn provenance(
    engine: &Engine,
    figure: &Figure,
    file: &str,
    style: &Style,
) -> (serde_json::Value, serde_json::Value) {
    let spec = engine.spec();
    let command = cav_engine::Command::Export {
        what: cav_engine::ExportKind::Figure(Box::new(figure.clone())),
        to: file.to_owned(),
    };
    let size = size_mm(figure, style);
    let shared = json!({
        "cavendish_figure": FIGURE_VERSION,
        "what": figure.what.name(),
        "subject": {
            "tower": figure.tower,
            "channel": figure.channel.name(),
            "span": figure.span,
            "pane": figure.pane + 1,
            "playhead_s": engine.time(),
        },
        "engine": cav_engine::build().json(),
        "numerics": cav_sim::bundle::numerics(spec, cav_engine::build()),
        "scene": { "name": spec.name, "spec": spec },
    });
    let entry = json!({
        "file": file,
        "format": figure.format.name(),
        "preset": style.name,
        "size_mm": size,
        "size_points": [size[0] / MM_PER_POINT, size[1] / MM_PER_POINT],
        "command": {
            "console": cav_engine::describe(&command, figure.pane),
            "cli": cli_line(engine, figure, file),
            "note": "the console line is run on an engine holding this scene; the playhead and, for a view, the pane's camera are the session's",
        },
        "fonts": "Ubuntu Light and Ubuntu Mono (Ubuntu Font Licence 1.0); in SVG and PDF the text is the glyphs' outlines",
    });
    (shared, entry)
}

/// The file a figure's data or provenance goes in: the figure's own name with another extension.
fn beside(file: &str, extension: &str) -> String {
    Path::new(file)
        .with_extension(extension)
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}

/// Builds a figure of a plot from `engine`: the drawing, its data and its `.json`. `file` is the
/// figure's file name, which the `.json` records (its directory is not recorded). Nothing is
/// written and no device is needed. A view of the scene is a picture, which
/// [`export`] makes; here it is refused.
///
/// # Errors
/// A preset that does not exist, a PNG too large, a plot with nothing to draw (with the plot's
/// own reason), a view.
pub fn build(engine: &Engine, figure: &Figure, file: &str) -> Result<Built, String> {
    let style = Style::preset(&figure.preset)?;
    let size = size_mm(figure, &style);
    let pixels = match figure.format {
        FigureFormat::Png => Some(pixels(size, figure.dpi.unwrap_or(style.dpi))?),
        _ => None,
    };
    let content = match figure.what {
        FigureKind::Signal => PaneContent::Signal,
        FigureKind::Periodogram => PaneContent::Periodogram,
        FigureKind::Cadence => PaneContent::Cadence,
        FigureKind::Flight => PaneContent::Flight,
        FigureKind::View => return Err("a view of the scene is drawn by the renderer: export it".into()),
    };
    let points = [(size[0] / MM_PER_POINT) as f32, (size[1] / MM_PER_POINT) as f32];
    let Plot { drawing, drawn, described } =
        plots::build(engine, &subject(figure, content), &style, &Outlines, points).expect("not the scene");
    if let (0, Some(why)) = (drawn.points, &drawn.note) {
        return Err(format!("there is nothing to draw: {why}"));
    }
    let table = table(engine, figure)?;
    let data_file = beside(file, if table.is_long() { "npy" } else { "csv" });
    // Each axis as it was drawn: its marks, what is written at them, the short marks between,
    // and its label, so that a redraw can be the same figure.
    let axis = |name: &(String, String), axis: &plots::Axis, n: usize| {
        json!({
            "name": name.0,
            "unit": name.1,
            "scale": if axis.log { "log" } else { "linear" },
            "range": [axis.lo, axis.hi],
            "label": drawn.axis_labels[n],
            "ticks": drawn.ticks[n],
            "tick_labels": drawn.tick_labels[n],
            "minor_ticks": drawn.minor_ticks[n],
        })
    };
    // Which columns each series is: a signal's table has three for each of its towers, and a
    // series drawn is one of those towers (all of them, unless there are more than a plot draws).
    let first = match (&table.values, figure.what) {
        (Values::Signal { towers, .. }, FigureKind::Signal) => described
            .towers
            .iter()
            .map(|tower| {
                3 * towers.iter().position(|held| held == tower).expect("a tower drawn is in the table")
            })
            .collect(),
        _ => Vec::new(),
    };
    let series: Vec<serde_json::Value> = described
        .series
        .iter()
        .enumerate()
        .map(|(n, (name, colour))| {
            let (x, y) = match figure.what {
                FigureKind::Signal => (first[n], first[n] + 1),
                FigureKind::Periodogram => (0, 2),
                FigureKind::Cadence => (0, 1),
                _ => (1, 0),
            };
            json!({
                "name": name,
                "colour": colour.css(),
                "x": table.columns.get(x).map(|c| &c.0),
                "y": table.columns.get(y).map(|c| &c.0),
                "x_column": x,
                "y_column": y,
                // A signal's third column says which of its shots the tower took: a slot not
                // taken holds 0 and is not a measurement.
                "taken_column": (figure.what == FigureKind::Signal).then(|| first[n] + 2),
            })
        })
        .collect();
    let (mut shared, mut provenance) = provenance(engine, figure, file, &style);
    let extra = json!({
        // The title drawn, or none; the drawing keeps its name for the file's metadata.
        "title": figure.title.then_some(&drawing.title),
        "pixels": pixels,
        "dpi": pixels.map(|_| figure.dpi.unwrap_or(style.dpi)),
        "axes": { "x": axis(&described.x, &drawn.x, 0), "y": axis(&described.y, &drawn.y, 1) },
        "series": series,
        "marks": described.marks,
        "colours": {
            "background": style.background.css(),
            "ink": style.ink.css(),
            "rule": style.rule.css(),
            "spectrum": style.spectrum.css(),
            "reference": style.reference.css(),
            "series": style.series.iter().map(|c| c.css()).collect::<Vec<_>>(),
        },
    });
    let data = json!({
        "data": {
            "file": data_file,
            "format": if table.is_long() { "npy" } else { "csv" },
            "rows": table.rows(),
            "columns": table.columns.iter().map(|(name, unit)| json!({ "name": name, "unit": unit })).collect::<Vec<_>>(),
            "note": format!(
                "every value of every tower, to the bit; the line drawn is thinned to at most {} points a series (the least and the greatest of each stretch), and a signal draws at most {} towers",
                plots::MAX_POINTS,
                plots::MAX_SERIES
            ),
        },
    });
    merge(&mut provenance, extra);
    merge(&mut shared, data);
    Ok(Built { drawing, table: Some(table), figure: shared, provenance, style, pixels })
}

/// A record as the text of its file.
fn text_of(record: &serde_json::Value) -> String {
    let mut text = serde_json::to_string_pretty(record).expect("a figure's record is JSON");
    text.push('\n');
    text
}

/// The record at `path`, if there is one that is JSON and no larger than a record is.
fn read_record(path: &Path) -> Option<serde_json::Value> {
    let length = std::fs::metadata(path).ok()?.len();
    if length > RECORD_BYTES {
        return None;
    }
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// Puts the fields of `more` into `into`, after its own.
fn merge(into: &mut serde_json::Value, more: serde_json::Value) {
    if let (Some(into), serde_json::Value::Object(more)) = (into.as_object_mut(), more) {
        into.extend(more);
    }
}

/// A name a record may give a picture beside it: a file's name alone, never a path.
fn plain_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\', ':']) && name != "." && name != ".."
}

impl Built {
    /// The figure's record with this picture in it, given the record already beside it (`old`,
    /// if one could be read) and which of the pictures that one names are still there
    /// (`there`): the rule at the top of this module.
    #[must_use]
    pub fn record(&self, old: Option<&serde_json::Value>, there: &dyn Fn(&str) -> bool) -> serde_json::Value {
        let own = self.provenance["file"].as_str().unwrap_or_default();
        let mut files = vec![self.provenance.clone()];
        let mut strangers: Vec<String> = Vec::new();
        if let Some(old) = old.and_then(serde_json::Value::as_object) {
            // The same figure: the old record, less its lists, is this one's.
            let mut rest = old.clone();
            let listed = rest.remove("files");
            let apart = rest.remove("not_this_figure");
            let same = serde_json::Value::Object(rest) == self.figure;
            let kept = |name: &str| plain_name(name) && name != own && there(name);
            for entry in listed.as_ref().and_then(serde_json::Value::as_array).into_iter().flatten() {
                match entry["file"].as_str().filter(|name| kept(name)) {
                    Some(_) if same => files.push(entry.clone()),
                    Some(name) => strangers.push(name.to_owned()),
                    None => {}
                }
            }
            for name in apart.as_ref().and_then(serde_json::Value::as_array).into_iter().flatten() {
                strangers.extend(name.as_str().filter(|name| kept(name)).map(str::to_owned));
            }
        }
        files.sort_by(|a, b| a["file"].as_str().cmp(&b["file"].as_str()));
        strangers.sort();
        strangers.dedup();
        let mut record = self.figure.clone();
        merge(&mut record, json!({ "files": files, "not_this_figure": strangers }));
        record
    }

    /// The text of the figure's `.json` when this picture is the only one written of it.
    #[must_use]
    pub fn json(&self) -> String {
        text_of(&self.record(None, &|_| false))
    }

    /// The data file's name, if the figure has data.
    #[must_use]
    pub fn data_file(&self) -> Option<&str> {
        self.table.as_ref()?;
        self.figure["data"]["file"].as_str()
    }

    /// Writes the figure's data to `path`, a row at a time through a buffer of [`WRITE_BUFFER`]
    /// bytes: as `.npy` when the table is long, as CSV otherwise. Nothing when the figure has no
    /// data.
    ///
    /// # Errors
    /// When the file cannot be made or written, with its path.
    pub fn write_data(&self, path: &Path) -> Result<(), String> {
        let Some(table) = &self.table else { return Ok(()) };
        let wrong = |why: std::io::Error| format!("{}: not written: {why}", path.display());
        let file = std::fs::File::create(path).map_err(wrong)?;
        let mut writer = std::io::BufWriter::with_capacity(WRITE_BUFFER, file);
        let written =
            if table.is_long() { table.write_npy(&mut writer) } else { table.write_csv(&mut writer) };
        written.and_then(|()| writer.flush()).map_err(wrong)
    }
}

/// What an exporter keeps between figures: the device and the renderers, made the first time a
/// figure needs pixels.
#[derive(Default)]
pub struct Studio {
    painter: Option<Painter>,
    renderer: Option<cav_render::Renderer>,
}

impl Studio {
    fn painter(&mut self) -> Result<&mut Painter, String> {
        if self.painter.is_none() {
            self.painter = Some(Painter::new(crate::headless::context())?);
        }
        Ok(self.painter.as_mut().expect("just made"))
    }

    /// The scene as pane `pane` of `engine` sees it, `size` pixels, as RGBA.
    fn view(&mut self, engine: &Engine, pane: usize, size: [u32; 2]) -> Result<Vec<u8>, String> {
        let gpu = self.painter()?.gpu.clone();
        let renderer = self.renderer.get_or_insert_with(|| cav_render::Renderer::new(&gpu));
        renderer.render_rgba(&gpu, engine.frame(pane)?, size)
    }
}

/// A figure of a pane's view: the renderer's picture, alone (PNG) or held by a drawing; or, for
/// an SVG or a PDF of a pane with voxel bodies, a vector drawing of them
/// ([`crate::voxels::view`]), which needs no device. The PNG's bytes come back beside it.
fn view(
    engine: &Engine,
    figure: &Figure,
    file: &str,
    studio: &mut Studio,
) -> Result<(Built, Option<Vec<u8>>), String> {
    let style = Style::preset(&figure.preset)?;
    let size = size_mm(figure, &style);
    if figure.format != FigureFormat::Png
        && let Some(voxels) = engine.frame(figure.pane)?.voxels
    {
        let spec = engine.spec();
        let points = [(size[0] / MM_PER_POINT) as f32, (size[1] / MM_PER_POINT) as f32];
        let name = format!("View · {}", spec.name);
        let title = figure.title.then_some(name.as_str());
        let camera = engine.view(figure.pane);
        let (drawing, drawn) = crate::voxels::view(spec, camera, voxels, &style, points, title, &Outlines);
        let (mut shared, mut provenance) = provenance(engine, figure, file, &style);
        let field = voxels.colouring.field;
        merge(
            &mut provenance,
            json!({
                "title": title,
                "vector": true,
                "projection": "orthographic",
                "note": if drawn.was_perspective {
                    "the pane is in perspective; the drawing is orthographic along its direction, faces far to near"
                } else {
                    "orthographic along the pane's direction, faces far to near"
                },
                "voxels": {
                    "field": field.name(),
                    "unit": field.unit(),
                    "tower": (field == cav_sim::voxel_view::LeafField::Phase).then_some(voxels.colouring.tower + 1),
                    "range": voxels.range.map(|(lo, hi)| [lo, hi]),
                    "legend": crate::voxels::heading(voxels, spec),
                    "cut": voxels.bodies.iter().map(|b| b.cut).max(),
                    "cubes": drawn.cubes,
                    "faces": drawn.faces,
                    "notes": voxels.notes,
                },
            }),
        );
        merge(&mut shared, json!({ "camera": camera, "data": null }));
        return Ok((Built { drawing, table: None, figure: shared, provenance, style, pixels: None }, None));
    }
    let pixels = pixels(size, figure.dpi.unwrap_or(style.dpi))?;
    let rgba = studio.view(engine, figure.pane, pixels)?;
    let points = [(size[0] / MM_PER_POINT) as f32, (size[1] / MM_PER_POINT) as f32];
    let spec = engine.spec();
    let mut drawing = Drawing::new(points, format!("View · {}", spec.name));
    drawing.items.push(Item::Image {
        min: [0.0, 0.0],
        max: points,
        width: pixels[0],
        height: pixels[1],
        rgb: std::sync::Arc::new(rgba.chunks(4).flat_map(|pixel| [pixel[0], pixel[1], pixel[2]]).collect()),
    });
    let (mut shared, mut provenance) = provenance(engine, figure, file, &style);
    merge(
        &mut provenance,
        json!({
            "title": drawing.title,
            "pixels": pixels,
            "dpi": figure.dpi.unwrap_or(style.dpi),
            "note": "the scene as the renderer draws it: a picture, also inside an SVG or a PDF; it has no data file",
        }),
    );
    merge(&mut shared, json!({ "camera": engine.view(figure.pane), "data": null }));
    let png = cav_render::encode_png(&rgba, pixels)?;
    Ok((Built { drawing, table: None, figure: shared, provenance, style, pixels: Some(pixels) }, Some(png)))
}

/// Draws `figure` from `engine` and writes it to `path`, with its data beside it, and adds it to
/// the figure's record there (or starts the record again, if the one there is of another
/// figure: the rule at the top of this module). Answers with the files written, the figure first.
///
/// # Errors
/// As [`build`]; when a PNG or a view needs a device and there is none; when a file cannot be
/// written.
pub fn export(
    engine: &Engine,
    figure: &Figure,
    path: &Path,
    studio: &mut Studio,
) -> Result<Vec<PathBuf>, String> {
    let file = path.file_name().ok_or_else(|| format!("`{}` names no file", path.display()))?;
    let file = file.to_string_lossy().into_owned();
    let (built, picture) = if figure.what == FigureKind::View {
        view(engine, figure, &file, studio)?
    } else {
        (build(engine, figure, &file)?, None)
    };
    let bytes = match (figure.format, picture) {
        (FigureFormat::Svg, _) => crate::svg::write(&built.drawing)?.into_bytes(),
        (FigureFormat::Pdf, _) => crate::pdf::write(&built.drawing),
        (FigureFormat::Png, Some(png)) => png,
        (FigureFormat::Png, None) => {
            let pixels = built.pixels.expect("a PNG figure has its pixels");
            studio.painter()?.drawing_png(&built.drawing, pixels, built.style.background.egui())?
        }
    };
    let directory = path.parent().unwrap_or_else(|| Path::new(""));
    let write = |path: &Path, bytes: &[u8]| {
        std::fs::write(path, bytes).map_err(|why| format!("{}: not written: {why}", path.display()))
    };
    let mut files = vec![path.to_path_buf()];
    write(path, &bytes)?;
    if let Some(name) = built.data_file() {
        let data = directory.join(name);
        built.write_data(&data)?;
        files.push(data);
    }
    // The record: this picture added to the one beside it, if that is of the same figure.
    let record = directory.join(beside(&file, "json"));
    let old = read_record(&record);
    let said = built.record(old.as_ref(), &|name| directory.join(name).is_file());
    write(&record, text_of(&said).as_bytes())?;
    files.push(record);
    Ok(files)
}

/// Gives `engine` this crate's figure exporter: from then on a figure export
/// (`figure … to fig.pdf`) is drawn and written. The device a PNG or a view needs is made the
/// first time one is asked for, and kept.
pub fn install(engine: &mut Engine) {
    let studio = Mutex::new(Studio::default());
    engine.set_figure_exporter(Box::new(move |engine, figure, path| {
        // A panic while drawing leaves nothing half-made in the studio worth refusing over.
        let mut studio = studio.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        export(engine, figure, path, &mut studio)
    }));
}
