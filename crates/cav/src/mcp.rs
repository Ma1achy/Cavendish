//! `cav mcp`: the engine as an MCP server on stdin and stdout, with the official Rust SDK.
//!
//! Each tool maps onto the engine API (REBUILD §7): a tool is a command or a query, nothing more.
//! One engine and one device live for the whole session. Standard output is the protocol, so
//! nothing here prints; diagnostics go to the engine's log, which the `log` tool reads.

use std::sync::{Arc, Mutex, MutexGuard};

use base64::Engine as _;
use cav_engine::{Level, LogFilter, Query, Subsystem};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::session::Session;

/// The largest picture a tool of this server draws, pixels per side: `render_view`'s, and a
/// `figure` that is made of pixels (a PNG, or a view of the scene in any format). `cav figure`
/// and the console take `cav_plot::figure::MAX_PIXELS`; a picture handed back in a message is
/// held to less (S2.md §3.1, A16).
const MAX_SIDE: u32 = 4096;

/// Refuses a figure whose picture would have a side over [`MAX_SIDE`], with the numbers. A plot
/// written as SVG or PDF has no pixels and is never refused here.
fn within_this_server(figure: &cav_engine::Figure) -> Result<(), String> {
    match cav_plot::figure::raster(figure)? {
        None => Ok(()),
        Some((size_mm, dpi)) => cav_plot::figure::pixels_within(size_mm, dpi, MAX_SIDE)
            .map(|_| ())
            .map_err(|why| format!("{why} (over MCP; `cav figure` writes up to 8192 a side)")),
    }
}

#[derive(Clone)]
pub struct Cav {
    session: Arc<Mutex<Session>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct SceneLoadArgs {
    /// A library scene name (`room`, `void`, `point`, `pass`, `oscillator-zoo` and the others `cav scenes` lists), or the path of a `.ron` scene file. Give this or `spec`.
    pub scene: Option<String>,
    /// A whole scene, inline, as the JSON object `scene_get` returns and `scene_schema` describes. Give this or `scene`. At most 1 MiB.
    pub spec: Option<serde_json::Value>,
}

#[derive(Deserialize, JsonSchema)]
pub struct SceneSetArgs {
    /// A path into the scene spec, such as `bodies[0].mass`, `array.towers[2].timing.offset` or `bodies[0].motion` (an enum is addressed through its variant: `bodies[0].motion.Static.position`).
    pub path: String,
    /// The new value, as JSON of the field's own type: a number, a string, `[x, y, z]`, an object, or `null` to remove an optional part.
    pub value: serde_json::Value,
}

#[derive(Deserialize, JsonSchema)]
pub struct ScenePathArgs {
    /// A path into the scene spec. Omit it for the top of the spec.
    pub path: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct SceneGetArgs {
    /// A path into the scene spec, such as `array.towers[2].position`. Omit it for the whole spec.
    pub path: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct CommandArgs {
    /// One or more console commands separated by `;`, for example `panes 4; view top; select tower 3`.
    /// `help` lists them.
    pub line: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct LogArgs {
    /// How many of the most recent entries to return (default 50).
    pub last: Option<usize>,
    /// Only entries at this level or above: `debug`, `info`, `warn` or `error`.
    pub level: Option<String>,
    /// Only entries whose message contains this text.
    pub text: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct RenderArgs {
    /// Width in pixels, 1 to 4096 (default 960).
    pub width: Option<u32>,
    /// Height in pixels, 1 to 4096 (default 600).
    pub height: Option<u32>,
    /// Which pane's camera to draw, from 1 (default: the active pane).
    pub pane: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
pub struct FigureArgs {
    /// What the figure shows: `signal` (every tower's, or one's), `periodogram`, `cadence` (the spectral window), `flight` (the atoms' flights) or `view` (a pane's view of the scene).
    pub what: String,
    /// The file to write: its extension is the format, `.svg`, `.pdf` or `.png`. The data (`.csv`, or `.npy` when long) and a `.json` of axes, units and provenance are written beside it under the same name.
    pub file: String,
    /// The look and size: `paper-column` (85 mm wide, the default), `paper-full` (178 mm), `slide` (254 × 143 mm), `poster` (400 mm) or `screen` (the application's dark theme).
    pub preset: Option<String>,
    /// The tower, from 1: whose periodogram or window (default 1); for a signal, the one tower drawn (default: all).
    pub tower: Option<usize>,
    /// The channel of a signal or a periodogram: `gradiometer` (default), `lower` or `upper`.
    pub channel: Option<String>,
    /// The stretch of the horizontal axis, `[from, to]`: seconds for a signal or flights, hertz for a periodogram or a window. Default: all of it.
    pub span: Option<[f64; 2]>,
    /// Width and height in millimetres, in place of the preset's.
    pub size_mm: Option<[f64; 2]>,
    /// For a PNG: pixels per inch, in place of the preset's. Each side of the picture is at most 4096 pixels here (`cav figure` writes up to 8192).
    pub dpi: Option<f64>,
    /// For a view: which pane's camera, from 1 (default: the active pane).
    pub pane: Option<usize>,
    /// Whether the picture has a title at its top (default true). Without one the axes take the room and the record's `title` is null.
    pub title: Option<bool>,
}

impl Cav {
    fn new() -> Self {
        Self { session: Arc::new(Mutex::new(Session::new())) }
    }

    /// The session. A tool that panicked while holding it has not left it half-changed (the engine
    /// applies a command or refuses it), so a poisoned lock is still usable.
    fn session(&self) -> MutexGuard<'_, Session> {
        self.session.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Runs a tool's body so that the request is answered whatever happens inside it: a panic (in
/// the renderer, in wgpu) becomes a tool error with its message, where it would otherwise unwind
/// through the server and leave the client waiting for ever.
fn answered<T>(tool: &str, body: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(outcome) => outcome,
        Err(panic) => {
            let why = panic
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| panic.downcast_ref::<&str>().copied())
                .unwrap_or("no message");
            Err(format!("{tool} failed inside the engine: {why}"))
        }
    }
}

#[tool_router]
impl Cav {
    /// The JSON Schema of the scene spec, or of the field at a path: its type, range, unit
    /// (`x-unit`), doc string and the stage an edit of it invalidates (`x-stage`).
    #[tool(name = "scene_schema")]
    fn scene_schema(&self, Parameters(args): Parameters<ScenePathArgs>) -> Result<String, String> {
        answered("scene_schema", || {
            let query = Query::Schema(args.path);
            self.session().engine.query(&query).map(|reply| cav_engine::render(&reply))
        })
    }

    /// Replace the scene with a library scene, a `.ron` file, or a whole scene given inline as
    /// JSON. Returns the engine's state. An edit: `undo` takes it back.
    #[tool(name = "scene_load")]
    fn scene_load(&self, Parameters(args): Parameters<SceneLoadArgs>) -> Result<String, String> {
        answered("scene_load", || {
            let mut session = self.session();
            match (&args.scene, &args.spec) {
                (Some(scene), None) => session.load(scene)?,
                (None, Some(spec)) => {
                    let scene = cav_sim::inline(&spec.to_string()).map_err(|why| {
                        session.engine.note(Level::Error, Subsystem::Mcp, format!("scene_load: {}", why.0));
                        why.0
                    })?;
                    session.engine.apply(cav_engine::Command::load(scene))?;
                }
                _ => return Err(
                    "scene_load takes `scene` (a name or a path) or `spec` (a scene, inline): one of them"
                        .into(),
                ),
            }
            session.engine.run_line("stats")
        })
    }

    /// Set the value at a path of the scene: the same edit a panel or the console's `set` makes,
    /// checked the same way and undone by `undo`. Returns the line that makes the edit and what
    /// it invalidates. Indices are positions: removing an item renumbers the ones after it.
    #[tool(name = "scene_set")]
    fn scene_set(&self, Parameters(args): Parameters<SceneSetArgs>) -> Result<String, String> {
        answered("scene_set", || {
            let mut session = self.session();
            let before = session.engine.spec().clone();
            let command = cav_engine::Command::set(&args.path, args.value);
            let described = cav_engine::describe(&command, session.engine.active_pane());
            session.engine.apply(command)?;
            let stage = cav_sim::paths::diff(&before, session.engine.spec());
            Ok(format!("{described}\ninvalidates: {}", stage.name()))
        })
    }

    /// List what is inside a path of the scene: each field with its value, unit, kind, range and
    /// doc string, as JSON. A field that holds one value lists itself.
    #[tool(name = "scene_ls")]
    fn scene_ls(&self, Parameters(args): Parameters<ScenePathArgs>) -> Result<String, String> {
        answered("scene_ls", || {
            let entries =
                cav_sim::paths::ls(self.session().engine.spec(), args.path.as_deref().unwrap_or_default())?;
            serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())
        })
    }

    /// Read the scene spec, or the part of it at a path, as JSON.
    #[tool(name = "scene_get")]
    fn scene_get(&self, Parameters(args): Parameters<SceneGetArgs>) -> Result<String, String> {
        answered("scene_get", || {
            let query = Query::Spec(args.path);
            self.session().engine.query(&query).map(|reply| cav_engine::render(&reply))
        })
    }

    /// Run console commands: everything the application's buttons and keys do, as text.
    /// Returns what each command answered, one per line.
    #[tool(name = "command")]
    fn command(&self, Parameters(args): Parameters<CommandArgs>) -> Result<String, String> {
        answered("command", || self.session().engine.run_script(&args.line).map(|answers| answers.join("\n")))
    }

    /// The engine's event log: what was done and what was refused, oldest first.
    #[tool(name = "log")]
    fn log(&self, Parameters(args): Parameters<LogArgs>) -> Result<String, String> {
        let min_level = match args.level.as_deref() {
            None => None,
            Some("debug") => Some(Level::Debug),
            Some("info") => Some(Level::Info),
            Some("warn") => Some(Level::Warn),
            Some("error") => Some(Level::Error),
            Some(other) => return Err(format!("`{other}` is not a level; use debug, info, warn or error")),
        };
        let filter =
            LogFilter { min_level, subsystem: None, text: args.text, last: Some(args.last.unwrap_or(50)) };
        answered("log", || {
            self.session().engine.query(&Query::Log(filter)).map(|reply| cav_engine::render(&reply))
        })
    }

    /// Write a figure for a paper or a talk: a plot or a view as SVG, PDF or PNG at a stated
    /// size, with the data behind it and a JSON of its axes, units and provenance beside it.
    /// Returns the files written, and the picture itself when it is a PNG. A figure of a signal
    /// or a periodogram waits for the signal.
    #[tool(name = "figure")]
    fn figure(&self, Parameters(args): Parameters<FigureArgs>) -> Result<CallToolResult, String> {
        answered("figure", || self.draw_figure(args))
    }

    /// Draw the scene as a pane sees it and return the picture as a PNG.
    #[tool(name = "render_view")]
    fn render_view(&self, Parameters(args): Parameters<RenderArgs>) -> Result<CallToolResult, String> {
        answered("render_view", || self.render(&args))
    }
}

impl Cav {
    fn draw_figure(&self, args: FigureArgs) -> Result<CallToolResult, String> {
        use cav_engine::{ChannelName, Figure, FigureFormat, FigureKind};
        let what = FigureKind::parse(&args.what).ok_or_else(|| {
            format!(
                "`{}` is not something a figure shows: signal, periodogram, cadence, flight or view",
                args.what
            )
        })?;
        let format = FigureFormat::of_file(&args.file).ok_or_else(|| {
            format!("`{}` does not end in .svg, .pdf or .png, so it names no format", args.file)
        })?;
        let channel = match &args.channel {
            None => ChannelName::default(),
            Some(word) => ChannelName::parse(word)
                .ok_or_else(|| format!("`{word}` is not a channel: gradiometer, lower or upper"))?,
        };
        let mut session = self.session();
        let pane = match args.pane {
            None => session.engine.active_pane(),
            Some(0) => return Err("panes are counted from 1".into()),
            Some(n) => n - 1,
        };
        let mut figure = Figure::new(what, format);
        figure.preset = args.preset.unwrap_or(figure.preset);
        figure.tower =
            args.tower.or(matches!(what, FigureKind::Periodogram | FigureKind::Cadence).then_some(1));
        figure.channel = channel;
        (figure.span, figure.size_mm, figure.dpi) = (args.span, args.size_mm, args.dpi);
        figure.title = args.title.unwrap_or(true);
        figure.pane = if what == FigureKind::View { pane } else { 0 };
        within_this_server(&figure)?;
        let command = cav_engine::Command::Export {
            what: cav_engine::ExportKind::Figure(Box::new(figure)),
            to: args.file.clone(),
        };
        let described = cav_engine::describe(&command, session.engine.active_pane());
        session.engine.apply(command)?;
        let path = std::path::Path::new(&args.file);
        let mut files = vec![args.file.clone()];
        files.extend(
            ["csv", "npy", "json"]
                .iter()
                .map(|extension| path.with_extension(extension))
                .filter(|file| file.exists())
                .map(|file| file.display().to_string()),
        );
        let mut blocks = vec![ContentBlock::text(format!("{described}\nwrote: {}", files.join(", ")))];
        if format == FigureFormat::Png {
            let png = std::fs::read(path).map_err(|e| format!("{}: {e}", args.file))?;
            blocks.push(ContentBlock::image(
                base64::engine::general_purpose::STANDARD.encode(png),
                "image/png",
            ));
        }
        Ok(CallToolResult::success(blocks))
    }

    fn render(&self, args: &RenderArgs) -> Result<CallToolResult, String> {
        let size = [args.width.unwrap_or(960), args.height.unwrap_or(600)];
        if size.iter().any(|side| !(1..=MAX_SIDE).contains(side)) {
            return Err(format!(
                "{} × {} is not a size this tool draws; each side is 1 to {MAX_SIDE}",
                size[0], size[1]
            ));
        }
        let mut session = self.session();
        let pane = match args.pane {
            None => session.engine.active_pane(),
            Some(0) => return Err("panes are counted from 1".into()),
            Some(n) => n - 1,
        };
        let png = session.render_png(pane, size).inspect_err(|why| {
            session.engine.note(Level::Error, Subsystem::Mcp, format!("render_view: {why}"));
        })?;
        session.engine.note(
            Level::Info,
            Subsystem::Mcp,
            format!("render_view: pane {}, {} × {} px, {} bytes", pane + 1, size[0], size[1], png.len()),
        );
        let encoded = base64::engine::general_purpose::STANDARD.encode(png);
        Ok(CallToolResult::success(vec![ContentBlock::image(encoded, "image/png")]))
    }
}

#[tool_handler(
    name = "cavendish",
    version = "0.0.0",
    instructions = "Cavendish: a voxel world around an array of atom-interferometer gravity gradiometers. \
                    Scenes hold the array, a room or a floor, and point masses on paths. Load one with \
                    scene_load (`cav scenes` names them; or give a whole scene inline), read it with scene_get \
                    and scene_ls (its shape is scene_schema), edit any value with scene_set, and \
                    do everything else with command (try `help`): \
                    `reading`, `signal` and `periodogram` give what the towers read, `show signal` puts a plot \
                    in a pane. Look at a pane with render_view, write a figure with its data and provenance \
                    with figure, and read what happened with log."
)]
impl ServerHandler for Cav {}

/// Serves until standard input closes.
pub fn serve() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("starting the runtime: {e}"))?;
    runtime.block_on(async {
        let service =
            Cav::new().serve(rmcp::transport::stdio()).await.map_err(|e| format!("MCP handshake: {e}"))?;
        service.waiting().await.map_err(|e| format!("MCP session: {e}"))?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_inside_a_tool_is_an_answer() {
        let fine: Result<u32, String> = answered("tool", || Ok(7));
        assert_eq!(fine, Ok(7));
        let refused: Result<u32, String> = answered("tool", || Err("no".into()));
        assert_eq!(refused, Err("no".into()), "an ordinary refusal passes through unchanged");
        // The property: a panic does not escape. (The default hook still prints it to stderr.)
        let panicked: Result<u32, String> =
            answered("render_view", || panic!("buffer of {} bytes", 1_440_002_352u64));
        assert_eq!(panicked, Err("render_view failed inside the engine: buffer of 1440002352 bytes".into()));
        let literal: Result<u32, String> = answered("command", || panic!("plain"));
        assert_eq!(literal, Err("command failed inside the engine: plain".into()));
    }

    #[test]
    fn a_figure_over_mcp_is_at_most_4096_pixels_a_side() {
        use cav_engine::{Figure, FigureFormat, FigureKind};
        // 25.4 mm at N pixels per inch is N pixels: the side is the resolution.
        let png = |dpi: f64| Figure {
            size_mm: Some([25.4, 25.4]),
            dpi: Some(dpi),
            ..Figure::new(FigureKind::Signal, FigureFormat::Png)
        };
        assert_eq!(within_this_server(&png(4096.0)), Ok(()));
        let why = within_this_server(&png(4097.0)).unwrap_err();
        assert!(
            why.contains("4097 × 4097 pixels; each side is 1 to 4096") && why.contains("over MCP"),
            "{why}"
        );
        // A view is a picture whatever the file: held to the same.
        let view = Figure { what: FigureKind::View, format: FigureFormat::Pdf, ..png(4097.0) };
        assert!(within_this_server(&view).is_err());
        assert_eq!(within_this_server(&Figure { dpi: Some(4096.0), ..view }), Ok(()));
        // A plot as SVG has no pixels: its `dpi` is not looked at.
        assert_eq!(within_this_server(&Figure { format: FigureFormat::Svg, ..png(1e6) }), Ok(()));
        // Control: the command line's limit is another number, and takes what this refuses.
        assert_eq!(cav_plot::figure::pixels([25.4, 25.4], 4097.0), Ok([4097, 4097]));
        assert_eq!(cav_plot::figure::pixels([25.4, 25.4], 8192.0), Ok([8192, 8192]));
        assert!(cav_plot::figure::pixels([25.4, 25.4], 8193.0).unwrap_err().contains("1 to 8192"));
        // A preset that does not exist is refused by name, as the engine would.
        let odd = Figure { preset: "a4".into(), ..png(100.0) };
        assert!(within_this_server(&odd).unwrap_err().contains("`a4` is not a figure preset"));
    }

    /// The tools of `source` whose bodies do not run under `answered("<their name>", …)`.
    fn unguarded(source: &str) -> Vec<String> {
        let mut out = Vec::new();
        for (_, rest) in source.match_indices("#[tool(name = \"").map(|(at, m)| (at, &source[at + m.len()..]))
        {
            let name = rest.split('"').next().unwrap_or_default();
            // The tool's function runs to the next attribute or the end of the impl block.
            let body = rest.split("\n    #[tool(").next().unwrap_or(rest);
            let body = body.split("\n}\n").next().unwrap_or(body);
            if !body.contains(&format!("answered(\"{name}\"")) {
                out.push(name.to_owned());
            }
        }
        out
    }

    #[test]
    fn every_tool_runs_where_a_panic_is_an_answer() {
        let source = include_str!("mcp.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert_eq!(production.matches("#[tool(name = \"").count(), 9, "the scan sees the nine tools");
        assert_eq!(unguarded(production), Vec::<String>::new());
        // Control: a tool whose body calls the engine bare is found.
        let bare = "    #[tool(name = \"log\")]\n    fn log(&self) -> Result<String, String> {\n        \
                    self.session().engine.run_line(\"log\")\n    }\n    #[tool(name = \"ok\")]\n    fn ok(&self) \
                    -> Result<String, String> {\n        answered(\"ok\", || Ok(String::new()))\n    }\n}\n";
        assert_eq!(unguarded(bare), ["log"]);
    }
}
