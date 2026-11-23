//! Figures for papers and talks: a plot or a view written as SVG, PDF or PNG at a stated size,
//! with the data behind it and where it came from beside it.
//!
//! A figure `fig.pdf` is three files: the picture; `fig.csv`, every value plotted, in plain
//! columns under a header that names each column and its unit (`fig.npy`, a `(rows, columns)`
//! array, when there are more than [`CSV_ROWS`] rows); and `fig.json`: what was plotted, the
//! axes, the series and which columns they are, the marks that are not data, the preset and the
//! size, the scene verbatim, the engine's version and revision, the numerics, and the command
//! that makes the figure again. Nothing in any of them is the time of day or the machine's
//! name: the same command on the same build writes the same bytes.
//!
//! The line drawn is thinned to [`crate::plots::MAX_POINTS`] points a series; the data file
//! keeps every value. A PNG is at most [`MAX_PIXELS`] pixels a side.
//!
//! [`install`] gives an engine the exporter, after which `figure …` on its console, `cav
//! figure` and the MCP `figure` tool are all [`cav_engine::Command::Export`].

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use cav_engine::{ChannelName, Engine, Figure, FigureFormat, FigureKind, PaneContent, Query, Reply};
use serde_json::json;

use crate::drawing::{Drawing, Item};
use crate::fonts::Outlines;
use crate::headless::Painter;
use crate::plots::{self, Plot, Subject};
use crate::style::{MM_PER_POINT, Style};

/// The most rows a figure's data has as text; more are written as `.npy`.
pub const CSV_ROWS: usize = 20_000;

/// The most pixels a PNG figure has on a side.
pub const MAX_PIXELS: u32 = 8192;

/// The version of the layout of a figure's `.json`.
pub const FIGURE_VERSION: u32 = 1;

/// The data behind a figure: columns of equal length.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    /// Each column's name and unit.
    pub columns: Vec<(String, String)>,
    /// Each column's values, in the order of `columns`.
    pub values: Vec<Vec<f64>>,
}

impl Table {
    /// How many rows the table has.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.values.first().map_or(0, Vec::len)
    }

    /// The table as CSV: a header of `name [unit]`, then a row a line. Every number is written
    /// with the fewest digits that read back as the same `f64`; an infinity is `inf` or `-inf`.
    #[must_use]
    pub fn csv(&self) -> String {
        let quoted = |field: String| {
            if field.contains([',', '"', '\n']) {
                format!("\"{}\"", field.replace('"', "\"\""))
            } else {
                field
            }
        };
        let header: Vec<String> =
            self.columns.iter().map(|(name, unit)| quoted(format!("{name} [{unit}]"))).collect();
        let mut out = header.join(",");
        out.push('\n');
        for row in 0..self.rows() {
            for (n, column) in self.values.iter().enumerate() {
                if n > 0 {
                    out.push(',');
                }
                out.push_str(&format!("{:?}", column[row]));
            }
            out.push('\n');
        }
        out
    }

    /// The table as a `(rows, columns)` array of `f64` in `.npy`.
    #[must_use]
    pub fn npy(&self) -> Vec<u8> {
        let rows = self.rows();
        let mut values = Vec::with_capacity(rows * self.values.len());
        for row in 0..rows {
            values.extend(self.values.iter().map(|column| column[row]));
        }
        cav_core::npy::encode_f64(&[rows, self.values.len()], &values)
    }

    /// Whether the table is long enough to be written as `.npy`.
    #[must_use]
    pub fn is_long(&self) -> bool {
        self.rows() > CSV_ROWS
    }
}

/// A figure, built and not yet written.
#[derive(Clone, Debug, PartialEq)]
pub struct Built {
    /// The picture.
    pub drawing: Drawing,
    /// Its data; a view of the scene has none.
    pub table: Option<Table>,
    /// Its `.json`: axes, series, marks, preset, scene, provenance.
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
    let side = |mm: f64| (mm / 25.4 * dpi).round();
    let [w, h] = [side(size_mm[0]), side(size_mm[1])];
    let most = f64::from(MAX_PIXELS);
    if !(w >= 1.0 && h >= 1.0 && w <= most && h <= most) {
        return Err(format!(
            "a figure of {} × {} mm at {dpi} pixels per inch is {w} × {h} pixels; each side is 1 to {MAX_PIXELS}: \
             make it smaller (`size`) or coarser (`dpi`)",
            size_mm[0], size_mm[1]
        ));
    }
    Ok([w as u32, h as u32])
}

fn subject(figure: &Figure, content: PaneContent) -> Subject {
    Subject {
        pane: figure.pane,
        content,
        tower: figure.tower,
        channel: figure.channel,
        span: figure.span,
        compared: false,
    }
}

/// The data behind a plot: the engine's own numbers, every one.
fn table(engine: &Engine, figure: &Figure) -> Result<Table, String> {
    let spec = engine.spec();
    let mut table = Table { columns: Vec::new(), values: Vec::new() };
    let mut column = |name: String, unit: &str, values: Vec<f64>| {
        table.columns.push((name, unit.to_owned()));
        table.values.push(values);
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
            for tower in towers {
                let label = &spec.array.towers[tower].label;
                column(format!("readout time of {label}"), "s", signal.readouts(tower));
                column(
                    format!("{channel} of {label}"),
                    "rad",
                    signal.channel(tower, figure.channel.channel()).to_vec(),
                );
                column(
                    format!("shot taken by {label}"),
                    "1 if taken or 0 if not",
                    signal.taken(tower).iter().map(|taken| f64::from(u8::from(*taken))).collect(),
                );
            }
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
    Ok(table)
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
    line
}

/// What every figure's `.json` says of where it came from.
fn provenance(engine: &Engine, figure: &Figure, file: &str, style: &Style) -> serde_json::Value {
    let spec = engine.spec();
    let command = cav_engine::Command::Export {
        what: cav_engine::ExportKind::Figure(Box::new(figure.clone())),
        to: file.to_owned(),
    };
    let size = size_mm(figure, style);
    json!({
        "cavendish_figure": FIGURE_VERSION,
        "what": figure.what.name(),
        "file": file,
        "format": figure.format.name(),
        "preset": style.name,
        "size_mm": size,
        "size_points": [size[0] / MM_PER_POINT, size[1] / MM_PER_POINT],
        "subject": {
            "tower": figure.tower,
            "channel": figure.channel.name(),
            "span": figure.span,
            "pane": figure.pane + 1,
            "playhead_s": engine.time(),
        },
        "command": {
            "console": cav_engine::describe(&command, figure.pane),
            "cli": cli_line(engine, figure, file),
            "note": "the console line is run on an engine holding this scene; the playhead and, for a view, the pane's camera are the session's",
        },
        "engine": {
            "version": cav_engine::build().version,
            "revision": cav_engine::build().revision,
            "dirty": cav_engine::build().dirty,
        },
        "numerics": cav_sim::bundle::numerics(spec, cav_engine::build()),
        "fonts": "Ubuntu Light and Ubuntu Mono (Ubuntu Font Licence 1.0); in SVG and PDF the text is the glyphs' outlines",
        "scene": { "name": spec.name, "spec": spec },
    })
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
    let axis = |name: &(String, String), axis: &plots::Axis| {
        json!({
            "name": name.0,
            "unit": name.1,
            "scale": if axis.log { "log" } else { "linear" },
            "range": [axis.lo, axis.hi],
        })
    };
    // Which columns each series is: the table's are laid out a series at a time.
    let per_series = if figure.what == FigureKind::Signal { 3 } else { 0 };
    let series: Vec<serde_json::Value> = described
        .series
        .iter()
        .enumerate()
        .map(|(n, (name, colour))| {
            let (x, y) = match figure.what {
                FigureKind::Signal => (per_series * n, per_series * n + 1),
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
                "taken_column": (figure.what == FigureKind::Signal).then_some(per_series * n + 2),
            })
        })
        .collect();
    let mut provenance = provenance(engine, figure, file, &style);
    let extra = json!({
        "title": drawing.title,
        "pixels": pixels,
        "dpi": pixels.map(|_| figure.dpi.unwrap_or(style.dpi)),
        "axes": { "x": axis(&described.x, &drawn.x), "y": axis(&described.y, &drawn.y) },
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
        "data": {
            "file": data_file,
            "format": if table.is_long() { "npy" } else { "csv" },
            "rows": table.rows(),
            "columns": table.columns.iter().map(|(name, unit)| json!({ "name": name, "unit": unit })).collect::<Vec<_>>(),
            "note": format!(
                "every value, to the bit; the line drawn is thinned to at most {} points a series (the least and the greatest of each stretch)",
                plots::MAX_POINTS
            ),
        },
    });
    merge(&mut provenance, extra);
    Ok(Built { drawing, table: Some(table), provenance, style, pixels })
}

/// Puts the fields of `more` into `into`, after its own.
fn merge(into: &mut serde_json::Value, more: serde_json::Value) {
    if let (Some(into), serde_json::Value::Object(more)) = (into.as_object_mut(), more) {
        into.extend(more);
    }
}

impl Built {
    /// The text of the figure's `.json`.
    #[must_use]
    pub fn json(&self) -> String {
        let mut text = serde_json::to_string_pretty(&self.provenance).expect("a figure's provenance is JSON");
        text.push('\n');
        text
    }

    /// The data file's name and bytes, if the figure has data.
    #[must_use]
    pub fn data(&self) -> Option<(String, Vec<u8>)> {
        let table = self.table.as_ref()?;
        let name = self.provenance["data"]["file"].as_str()?.to_owned();
        Some((name, if table.is_long() { table.npy() } else { table.csv().into_bytes() }))
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

/// A figure of a pane's view: the renderer's picture, alone (PNG) or held by a drawing.
fn view(
    engine: &Engine,
    figure: &Figure,
    file: &str,
    studio: &mut Studio,
) -> Result<(Built, Vec<u8>), String> {
    let style = Style::preset(&figure.preset)?;
    let size = size_mm(figure, &style);
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
    let mut provenance = provenance(engine, figure, file, &style);
    merge(
        &mut provenance,
        json!({
            "title": drawing.title,
            "pixels": pixels,
            "dpi": figure.dpi.unwrap_or(style.dpi),
            "camera": engine.view(figure.pane),
            "note": "the scene as the renderer draws it: a picture, also inside an SVG or a PDF; it has no data file",
        }),
    );
    let png = cav_render::encode_png(&rgba, pixels)?;
    Ok((Built { drawing, table: None, provenance, style, pixels: Some(pixels) }, png))
}

/// Draws `figure` from `engine` and writes it to `path`, with its data and its `.json` beside
/// it. Answers with the files written, the figure first.
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
        let (built, png) = view(engine, figure, &file, studio)?;
        (built, Some(png))
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
    let mut files = vec![(path.to_path_buf(), bytes)];
    if let Some((name, data)) = built.data() {
        files.push((directory.join(name), data));
    }
    files.push((directory.join(beside(&file, "json")), built.json().into_bytes()));
    for (path, bytes) in &files {
        std::fs::write(path, bytes).map_err(|why| format!("{}: not written: {why}", path.display()))?;
    }
    Ok(files.into_iter().map(|(path, _)| path).collect())
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
