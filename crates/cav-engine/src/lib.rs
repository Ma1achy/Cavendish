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
pub mod voxels;

pub use cav_sim::{PaneContent, Target};
pub use command::{
    COMB_HARMONICS, ChannelName, Comb, Command, DEFAULT_FIGURE_PRESET, ExportKind, Figure, FigureFormat,
    FigureKind, Harmonic, MAX_FIGURE_DPI, MAX_FIGURE_MM, MAX_PANES, PeriodogramReply, Query, ReadingReply,
    Reply, SignalReply, Stage, Standing, Stats, TowerReading, ViewCommand, WindowReply,
};
pub use console::{COMMANDS, CommandDoc, Request, describe, help, render, value_of};
pub use engine::{Engine, Exports, FigureExporter, REPLAY_PATIENCE, TYPED_LINE_PATIENCE, estimate_text};
pub use forward::{FORWARD_STORE_BYTES, SignalState};
pub use history::{HISTORY_BYTES, JOURNAL_BYTES};
pub use log::{Level, Log, LogEntry, LogFilter, Subsystem};

/// The engine's version, as `cav doctor`, `stats` and the About box report it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Who is running: the engine's version, the git revision it was built from and whether its
/// sources then had changes that were not committed. A bundle's manifest and a figure's
/// provenance carry these.
///
/// All three are fixed when the engine is compiled (`build.rs`): nothing is asked of git when a
/// file is written, so the same binary says the same thing wherever it is run and whatever has
/// happened to the source tree since.
///
/// - **revision**: the commit that was checked out when the engine was compiled.
/// - **dirty**: whether the engine's sources then differed from that commit: a tracked file
///   changed, staged or not, or a file git neither tracks nor ignores. The engine's sources are
///   what makes the numbers: `cav-core`, `cav-sim` and `cav-engine` (each crate's `src` and
///   `Cargo.toml`), the built-in `scenes`, the workspace's `Cargo.toml` and `Cargo.lock`,
///   `rust-toolchain.toml` and `.cargo`. It says nothing of the crates that draw or of the
///   clients (`cav-plot`, `cav-render`, `cav-gpu`, `cav-app`, `cav`), which are at the same
///   revision.
/// - Built where there is no git, no repository or no commit, both are unknown (`unknown` and
///   `None`), and the build does not fail.
///
/// Cargo makes the stamp again whenever one of those sources changes, or git's `HEAD`, its log,
/// the branch, the packed refs or the index do: so after every commit the engine and everything
/// that depends on it are compiled again (`docs/slices/S2.md` §7.12 has what that costs).
#[must_use]
pub fn build() -> &'static cav_sim::bundle::Build {
    static BUILD: std::sync::OnceLock<cav_sim::bundle::Build> = std::sync::OnceLock::new();
    BUILD.get_or_init(|| cav_sim::bundle::Build {
        version: VERSION.to_owned(),
        revision: env!("CAVENDISH_REVISION").to_owned(),
        dirty: match env!("CAVENDISH_DIRTY") {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
    })
}

/// The decision `build.rs` makes, held to what it says: the same file, included here.
#[cfg(test)]
mod provenance {
    include!("../provenance.rs");

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn the_stamp_is_decided_from_what_git_answers() {
        let head = format!("\n{COMMIT}\n");
        // As committed: the revision, and not dirty.
        assert_eq!(decide(Some(&head), Some("")), (COMMIT.to_owned(), "false"));
        // A tracked file changed, staged or not, or a file git does not know: dirty.
        for changes in [" M crates/cav-sim/src/read.rs\n", "M  Cargo.lock\n", "?? scenes/dev/new.ron\n"] {
            assert_eq!(decide(Some(&head), Some(changes)), (COMMIT.to_owned(), "true"), "{changes}");
        }
        // The commit is known and the status is not: the flag says so; it is not `false`.
        assert_eq!(decide(Some(&head), None), (COMMIT.to_owned(), "unknown"));
        // No answer at all (no git, no repository, no commit): unknown, whatever the status says.
        assert_eq!(decide(None, Some("")), ("unknown".to_owned(), "unknown"));
        assert_eq!(decide(None, None), ("unknown".to_owned(), "unknown"));
        // The workspace is a folder of someone else's repository: that commit is not ours.
        let inside = format!("vendor/cavendish/\n{COMMIT}\n");
        assert_eq!(decide(Some(&inside), Some("")), ("unknown".to_owned(), "unknown"));
        // An answer that is not a commit is not passed on as one.
        for answer in ["\nHEAD\n", "\n\n", "", "\n0123abc\n", &format!("\n{COMMIT}\nmore\n")] {
            assert_eq!(decide(Some(answer), Some("")).0, "unknown", "{answer:?}");
        }
    }

    /// A directory outside the repository, removed when the test ends however it ends.
    struct Outside(PathBuf);

    impl Drop for Outside {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn built_without_git_or_outside_a_repository_the_stamp_is_unknown() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        // No git on the PATH: a program that is not there. The stamp is made all the same, and
        // still says which sources to watch.
        let none = stamp("a-git-that-is-not-installed", &root);
        assert_eq!((none.revision.as_str(), none.dirty), ("unknown", "unknown"));
        assert!(none.watch.iter().any(|path| path.ends_with("crates/cav-engine/src")), "{:?}", none.watch);
        assert!(none.watch.iter().all(|path| path.exists()), "cargo would run the script every time");

        if ask("git", &root, &["--version"]).is_none() {
            eprintln!("git is not installed: the arms that ask it were not run");
            return;
        }
        // Git is there and the directory is no work tree of its own.
        let outside =
            Outside(std::env::temp_dir().join(format!("cavendish-no-repository-{}", std::process::id())));
        std::fs::create_dir_all(&outside.0).unwrap();
        let alone = stamp("git", &outside.0);
        assert_eq!((alone.revision.as_str(), alone.dirty), ("unknown", "unknown"));

        // Control: asked in this repository the same function does know, so `unknown` above is
        // the absence of git or of a repository and not a function that never answers.
        if ask("git", &root, &["rev-parse", "--show-prefix", "HEAD"])
            .is_none_or(|answer| !answer.starts_with('\n'))
        {
            eprintln!("this workspace is not the top of a git work tree: the control was not run");
            return;
        }
        let here = stamp("git", &root);
        assert!(
            here.revision.len() == 40 && here.revision.bytes().all(|b| b.is_ascii_hexdigit()),
            "{here:?}"
        );
        assert!(matches!(here.dirty, "true" | "false"), "{here:?}");
        assert!(
            here.watch.iter().any(|path| path.ends_with("HEAD")),
            "HEAD is not watched: {:?}",
            here.watch
        );
    }

    /// The paths of `watch` that are inside git's own directory, as git names them relative to it.
    fn git_paths(watch: &[PathBuf], root: &Path) -> Vec<String> {
        let sources: Vec<PathBuf> =
            ENGINE_SOURCES.iter().chain(&[".gitignore"]).map(|path| root.join(path)).collect();
        watch
            .iter()
            .filter(|path| !sources.contains(path))
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    #[test]
    fn the_build_watches_what_it_stamps() {
        // S2's second review, item 6: the stamp is the commit and whether the sources differ from
        // it, so the build is made again when a source, the commit or the branch moves, and not
        // when the index does (`git add` rewrote it and compiled five crates again).
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let made = stamp("git", &root);
        for source in ENGINE_SOURCES {
            let path = root.join(source);
            assert!(!path.exists() || made.watch.contains(&path), "{source} is not watched");
        }
        let inside = git_paths(&made.watch, &root);
        assert!(!inside.iter().any(|path| path.ends_with("/index")), "the index is watched: {inside:?}");
        if ask("git", &root, &["rev-parse", "HEAD"]).is_some() {
            assert!(inside.iter().any(|path| path.ends_with("/HEAD")), "{inside:?}");
            // On a branch, its ref is watched (unless it lives only in the packed refs).
            if let Some(branch) = ask("git", &root, &["symbolic-ref", "-q", "HEAD"]) {
                let loose = ask("git", &root, &["rev-parse", "--git-path", branch.trim()])
                    .map(|p| root.join(p.trim()));
                if loose.as_ref().is_some_and(|path| path.exists()) {
                    assert!(
                        inside.iter().any(|path| path.ends_with(branch.trim())),
                        "{branch} is not watched: {inside:?}"
                    );
                }
            }
        }
        // Control: the check sees an index when one is there.
        let mut with_index = made.watch.clone();
        with_index.push(root.join(".git/index"));
        assert!(git_paths(&with_index, &root).iter().any(|path| path.ends_with("/index")));
    }

    #[test]
    fn the_build_carries_what_the_script_decided() {
        // What is compiled in is one of the answers `decide` gives, and nothing asks git for it.
        let build = crate::build();
        let hex = build.revision.len() == 40 && build.revision.bytes().all(|b| b.is_ascii_hexdigit());
        assert!(hex || build.revision == "unknown", "{build:?}");
        assert_eq!(build.dirty.is_some(), hex && env!("CAVENDISH_DIRTY") != "unknown", "{build:?}");
        assert_eq!(build.version, crate::VERSION);
        let source = include_str!("lib.rs");
        let body = &source[source.find("pub fn build()").unwrap()..source.find("mod provenance").unwrap()];
        assert!(!body.contains("process::Command"), "build() runs a program");
    }
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
