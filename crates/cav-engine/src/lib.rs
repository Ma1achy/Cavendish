//! The Cavendish engine: one command/query API that the app, the CLI and the MCP server all drive
//! (REBUILD §3.2), with a journal that replays and an event log.
//!
//! ```
//! use cav_engine::{Command, Engine, Query, Reply};
//!
//! let mut engine = Engine::new();
//! engine.run_line("load void").unwrap();           // text and typed commands are the same path
//! engine.apply(Command::Seek(1.5)).unwrap();
//! let Reply::Stats(stats) = engine.query(&Query::Stats).unwrap() else { unreachable!() };
//! assert_eq!((stats.scene.as_str(), stats.time), ("void", 1.5));
//! assert_eq!(Engine::replay(engine.journal()).unwrap().digest(), engine.digest());
//! ```

pub mod command;
pub mod console;
pub mod engine;
pub mod log;

pub use cav_sim::Target;
pub use command::{Command, MAX_PANES, Query, Reply, Stats, ViewCommand};
pub use console::{COMMANDS, CommandDoc, Request, describe, help, render};
pub use engine::Engine;
pub use log::{Level, Log, LogEntry, LogFilter, Subsystem};

/// The engine's version, as `cav doctor`, `stats` and the About box report it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;
    use cav_sim::{DrawStyle, NamedView, Projection, SceneSpec};

    #[test]
    fn commands_and_queries_round_trip_through_json() {
        let commands = vec![
            Command::Load(SceneSpec::room()),
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
        ];
        for command in &commands {
            let text = serde_json::to_string(command).unwrap();
            assert_eq!(&serde_json::from_str::<Command>(&text).unwrap(), command, "{text}");
        }
        let queries = vec![
            Query::Spec(None),
            Query::Spec(Some("room.size".into())),
            Query::Schema,
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
