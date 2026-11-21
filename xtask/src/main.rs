//! Cavendish's local build and test tiers (REBUILD §8). No CI: these are the gates.
//!
//! ```text
//! cargo xtask quick [--budget]   fmt, clippy, CPU tests; under 10 s warm
//! cargo xtask gpu                tests that need the local adapter; under 2 min
//! cargo xtask oracle             the v1 anchors (the oracle crate arrives in S1)
//! cargo xtask demo <slice>       the slice's demo, headless, with screenshots
//! cargo xtask validate [--report] every validation case, tabulated; missing cases fail
//! cargo xtask hooks              install the pre-push hook that runs `quick`
//! ```

mod demo;
mod tiers;
mod validate;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

const HELP: &str = "\
cargo xtask <tier>

  quick [--budget]   cargo fmt --check, clippy -D warnings, then every test whose name does not
                     contain `gpu_`, through cargo-nextest, then the doc tests. Fails if fewer tests ran
                     than the floor in xtask/floors.txt. With --budget it also fails when it took over 10 s
                     (meaningful on a warm build only).
  gpu                the tests whose names contain `gpu_`: they need the local adapter. Same floor rule.
  oracle             the oracle tier. Before S1 there is no oracle crate, and it says so.
  demo <slice>       runs the slice's demo headlessly and saves screenshots to docs/slices/<slice>/.
  validate [--report] [--against <revision>]
                     runs every validation case (tests named `validation_…`, declared with
                     `cav_validate::case!`) and tabulates measured error against tolerance. A case
                     that is declared but did not run is reported as missing and fails the command.
                     It also holds the cases to docs/validation/report.json as committed at
                     the revision (HEAD unless --against names another, such as the last slice's
                     tag): a case that has gone, or a tolerance that has changed, fails unless
                     docs/validation/acknowledged.txt names it with a reason.
                     With --report it writes docs/validation/ (report.md, report.json, plots).
  hooks              installs .git/hooks/pre-push, which runs `cargo xtask quick`.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let root = root();
    let started = Instant::now();
    let outcome = match args.as_slice() {
        ["quick"] => tiers::quick(&root, false),
        ["quick", "--budget"] => tiers::quick(&root, true),
        ["gpu"] => tiers::gpu(&root),
        ["oracle"] => tiers::oracle(),
        ["demo", slice] => demo::run(&root, slice),
        ["validate"] => validate::run(&root, false, "HEAD"),
        ["validate", "--report"] => validate::run(&root, true, "HEAD"),
        ["validate", "--against", revision] => validate::run(&root, false, revision),
        ["validate", "--report", "--against", revision] => validate::run(&root, true, revision),
        ["hooks"] => hooks(&root),
        [] | ["help" | "--help" | "-h"] => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        other => Err(format!("unknown arguments: {}\n\n{HELP}", other.join(" "))),
    };
    match outcome {
        Ok(()) => {
            eprintln!("xtask: ok in {:.1} s", started.elapsed().as_secs_f64());
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("xtask: FAILED after {:.1} s: {why}", started.elapsed().as_secs_f64());
            ExitCode::FAILURE
        }
    }
}

/// The workspace root: xtask's manifest directory is `<root>/xtask`.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the root")
        .to_path_buf()
}

/// `cargo` with the given arguments, run in the workspace root.
fn cargo(root: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    command.current_dir(root).args(args);
    command
}

/// Runs a command with its output passed through; an error names the step that failed.
fn step(name: &str, mut command: Command) -> Result<(), String> {
    let started = Instant::now();
    let status = command.status().map_err(|e| format!("{name}: could not start: {e}"))?;
    eprintln!("xtask: {name}: {:.1} s", started.elapsed().as_secs_f64());
    if status.success() { Ok(()) } else { Err(format!("{name} failed ({status})")) }
}

/// Writes the pre-push hook. It lives in `.git/hooks`, which git does not version, so every clone
/// runs this once.
fn hooks(root: &Path) -> Result<(), String> {
    let hook = root.join(".git").join("hooks").join("pre-push");
    let script = "#!/bin/sh\n# Installed by `cargo xtask hooks`: the quick tier must pass before a push.\n\
                  PATH=\"$HOME/.cargo/bin:$PATH\" exec cargo xtask quick\n";
    std::fs::write(&hook, script).map_err(|e| format!("writing {}: {e}", hook.display()))?;
    eprintln!("xtask: wrote {}", hook.display());
    Ok(())
}
