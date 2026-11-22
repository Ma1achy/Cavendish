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

/// The largest picture `render_view` will draw, pixels per side.
const MAX_SIDE: u32 = 4096;

#[derive(Clone)]
pub struct Cav {
    session: Arc<Mutex<Session>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct SceneLoadArgs {
    /// A library scene name (`room`, `void`, `point`, `pass`, `oscillator-zoo` and the others `cav scenes` lists), or the path of a `.ron` scene file.
    pub scene: String,
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
    /// The JSON Schema of the scene spec: every field, its type and its meaning.
    #[tool(name = "scene_schema")]
    fn scene_schema(&self) -> Result<String, String> {
        answered("scene_schema", || self.session().engine.run_line("schema"))
    }

    /// Replace the scene with a library scene or a `.ron` file. Returns the engine's state.
    #[tool(name = "scene_load")]
    fn scene_load(&self, Parameters(args): Parameters<SceneLoadArgs>) -> Result<String, String> {
        answered("scene_load", || {
            let mut session = self.session();
            session.load(&args.scene)?;
            session.engine.run_line("stats")
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

    /// Draw the scene as a pane sees it and return the picture as a PNG.
    #[tool(name = "render_view")]
    fn render_view(&self, Parameters(args): Parameters<RenderArgs>) -> Result<CallToolResult, String> {
        answered("render_view", || self.render(&args))
    }
}

impl Cav {
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
                    S0 has the array, a room and the first dev scenes. Load a scene with scene_load, read it \
                    with scene_get (its shape is scene_schema), change the view with command (try `help`), \
                    look at it with render_view, and read what happened with log."
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
        assert_eq!(production.matches("#[tool(name = \"").count(), 6, "the scan sees the six tools");
        assert_eq!(unguarded(production), Vec::<String>::new());
        // Control: a tool whose body calls the engine bare is found.
        let bare = "    #[tool(name = \"log\")]\n    fn log(&self) -> Result<String, String> {\n        \
                    self.session().engine.run_line(\"log\")\n    }\n    #[tool(name = \"ok\")]\n    fn ok(&self) \
                    -> Result<String, String> {\n        answered(\"ok\", || Ok(String::new()))\n    }\n}\n";
        assert_eq!(unguarded(bare), ["log"]);
    }
}
