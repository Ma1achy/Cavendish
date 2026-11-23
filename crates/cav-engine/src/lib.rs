//! The Cavendish engine: one command/query API that the app, the CLI and the MCP server all drive
//! (REBUILD §3.2), with a journal that replays and an event log.
//!
//! ## What this crate is for
//! Everything a person or an agent can do to a scene is a [`Command`]; everything they can ask is a
//! [`Query`] answered by a [`Reply`]. The engine applies commands (or refuses them, changing
//! nothing), keeps the journal, the undo history and the event log, and hands the forward model to
//! its workers so that no command waits for a computation.
//!
//! ## What it may depend on
//! `cav-core` and `cav-sim`, and nothing that draws: no `cav-gpu`, `cav-render`, `cav-plot` or
//! `cav-app`. A client that can draw installs what the engine cannot do itself (a figure exporter).
//! It must not block a command on a computation, and must not hold state that a journal cannot
//! reproduce.
//!
//! ```text
//! use cav_engine::{Command, Engine, Query, Reply};
//!
//! let mut engine = Engine::new();
//! engine.run_line("load void").unwrap();           // text and typed commands are the same path
//! engine.apply(Command::Seek(1.5)).unwrap();
//! let Reply::Stats(stats) = engine.query(&Query::Stats).unwrap() else { unreachable!() };
//! assert_eq!((stats.scene.as_str(), stats.time), ("void", 1.5));
//! assert_eq!(Engine::replay(engine.journal()).unwrap().digest(), engine.digest());
//! ```

#![warn(missing_docs)]

pub mod command;
pub mod console;
pub mod engine;
pub mod forward;
pub mod history;
pub mod journal;
pub mod log;

pub use cav_sim::{PaneContent, Target};
pub use command::{
    COMB_HARMONICS, ChannelName, Comb, Command, DEFAULT_FIGURE_PRESET, ExportKind, Figure, FigureFormat,
    FigureKind, Harmonic, MAX_FIGURE_DPI, MAX_FIGURE_MM, MAX_PANES, PeriodogramReply, Query, ReadingReply,
    Reply, SignalReply, Stage, Standing, Stats, TowerReading, ViewCommand, WindowReply,
};
pub use console::{COMMANDS, CommandDoc, Request, describe, help, render};
pub use engine::{Engine, Exports, FigureExporter, TYPED_LINE_PATIENCE, estimate_text};
pub use forward::{FORWARD_STORE_BYTES, SignalState};
pub use history::{HISTORY_BYTES, JOURNAL_BYTES};
pub use log::{Level, Log, LogEntry, LogFilter, Subsystem};

/// The engine's version, as `cav doctor`, `stats` and the About box report it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Who is running: the engine's version, and the git revision of the source tree it was built
/// from with whether that tree has uncommitted changes. The revision is asked of git, in the
/// source tree, the first time it is wanted (a bundle's manifest, a figure's provenance); where
/// there is no git or no tree (a binary copied elsewhere) it is `unknown`.
///
/// This is the tree as it stands when asked, not as it stood when the binary was compiled: a
/// binary run through `cargo run` is always built from the tree it is asked about; one run by
/// hand after the tree has moved on is not, and nothing here can tell.
#[must_use]
pub fn build() -> &'static cav_sim::bundle::Build {
    static BUILD: std::sync::OnceLock<cav_sim::bundle::Build> = std::sync::OnceLock::new();
    BUILD.get_or_init(|| {
        let git = |arguments: &[&str]| {
            std::process::Command::new("git")
                .args(arguments)
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .output()
                .ok()
                .filter(|output| output.status.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        };
        let revision = git(&["rev-parse", "HEAD"]).filter(|sha| !sha.is_empty());
        let dirty = revision
            .as_ref()
            .and_then(|_| git(&["status", "--porcelain"]))
            .map(|changes| !changes.is_empty());
        cav_sim::bundle::Build {
            version: VERSION.to_owned(),
            revision: revision.unwrap_or_else(|| "unknown".to_owned()),
            dirty,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cav_sim::{DrawStyle, NamedView, Projection, SceneSpec};

    #[test]
    fn the_example_at_the_top_of_the_crate_runs() {
        // The example in this crate's documentation, as a test: `quick` does not build doctests
        // (one cost it two to three seconds of its ten), so the example is held here.
        let mut engine = Engine::new();
        engine.run_line("load void").unwrap();
        engine.apply(Command::Seek(1.5)).unwrap();
        let Reply::Stats(stats) = engine.query(&Query::Stats).unwrap() else { unreachable!() };
        assert_eq!((stats.scene.as_str(), stats.time), ("void", 1.5));
        assert_eq!(Engine::replay(engine.journal()).unwrap().digest(), engine.digest());
        // Control: the text in the documentation is this code. If either changes, this fails.
        let documented = include_str!("lib.rs");
        for line in ["engine.run_line(\"load void\").unwrap();", "engine.apply(Command::Seek(1.5)).unwrap();"]
        {
            assert!(documented.contains(&format!("//! {line}")), "the example no longer says `{line}`");
        }
    }

    #[test]
    fn commands_and_queries_round_trip_through_json() {
        let commands = vec![
            Command::load(SceneSpec::room()),
            Command::Seek(0.1 + 0.2),
            Command::Select(Some(Target::Tower(4))),
            Command::Select(None),
            Command::Undo,
            Command::Redo,
            Command::View(ViewCommand::Panes(2)),
            Command::View(ViewCommand::Activate(1)),
            Command::View(ViewCommand::Look { pane: 1, view: NamedView::Bottom }),
            Command::View(ViewCommand::Project { pane: 0, projection: Projection::Orthographic }),
            Command::View(ViewCommand::Style { pane: 0, style: DrawStyle::Wireframe }),
            Command::View(ViewCommand::Orbit { pane: 0, yaw: 1.0 / 3.0, pitch: -1e-9 }),
            Command::View(ViewCommand::Zoom { pane: 3, distance: 28.7 }),
            Command::View(ViewCommand::Pan { pane: 2, target: [0.1, 0.2, 0.3] }),
            Command::View(ViewCommand::Home { pane: 0 }),
            Command::Export { what: ExportKind::Bundle, to: "a bundle".into() },
            Command::Export {
                what: ExportKind::Figure(Box::new(Figure {
                    tower: Some(2),
                    span: Some([0.1 + 0.2, 7.0]),
                    size_mm: Some([85.0, 60.0]),
                    dpi: Some(300.0),
                    ..Figure::new(FigureKind::Periodogram, FigureFormat::Png)
                })),
                to: "fig.png".into(),
            },
        ];
        for command in &commands {
            let text = serde_json::to_string(command).unwrap();
            assert_eq!(&serde_json::from_str::<Command>(&text).unwrap(), command, "{text}");
        }
        let queries = vec![
            Query::Spec(None),
            Query::Spec(Some("room.size".into())),
            Query::Schema(None),
            Query::Schema(Some("array.instrument.cycle".into())),
            Query::Ls(Some("bodies".into())),
            Query::Stats,
            Query::Log(LogFilter { min_level: Some(Level::Warn), last: Some(3), ..LogFilter::default() }),
            Query::View(2),
            Query::Journal,
        ];
        let mut engine = Engine::new();
        engine.run_script("panes 4; seek 2; select room").unwrap();
        for query in &queries {
            let text = serde_json::to_string(query).unwrap();
            assert_eq!(&serde_json::from_str::<Query>(&text).unwrap(), query, "{text}");
            // Every reply survives the trip too: what MCP sends is what the engine answered.
            let reply = engine.query(query).unwrap();
            let sent = serde_json::to_string(&reply).unwrap();
            assert_eq!(serde_json::from_str::<Reply>(&sent).unwrap(), reply);
        }
        // Control: the wire form is tagged, so a command is not mistaken for another.
        assert_eq!(serde_json::to_string(&Command::Seek(1.5)).unwrap(), r#"{"Seek":1.5}"#);
        assert!(serde_json::from_str::<Command>(r#"{"Seek":"soon"}"#).is_err());
        assert!(serde_json::from_str::<Command>(r#"{"Explode":1}"#).is_err());
    }
}
