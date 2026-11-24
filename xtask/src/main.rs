//! Cavendish's local build and test tiers (REBUILD §8). No CI: these are the gates.
//!
//! ```text
//! cargo xtask quick [--budget]   fmt, clippy, CPU tests; under 10 s warm
//! cargo xtask gpu                tests that need the local adapter; under 2 min
//! cargo xtask oracle             the engine against v1 at v1-final and George's scripts
//! cargo xtask demo <slice>       the slice's demo, headless, with screenshots
//! cargo xtask validate [--report] every validation case, tabulated; missing cases fail
//! cargo xtask web [build|serve]  the browser build in target/web/dist; with no word, its tier
//! cargo xtask hooks              install the pre-push hook that runs `quick`
//! ```

mod demo;
#[cfg(test)]
mod json;
mod python;
#[cfg(test)]
mod recipes;
mod tiers;
mod validate;
mod web;
#[cfg(test)]
mod workspace;

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
  oracle             the oracle tier: the `oracle` crate, which compiles v1's forward model at tag
                     v1-final and holds the engine to it and to arrays made by running George's
                     reference scripts. A workspace of its own, built only by this command and by
                     validate; formatted, linted and held to a floor like the other tiers. Then the
                     Python side (python/): pyright in strict mode and the tests of cavendish_figures on
                     figures this build writes; a missing Python, numpy, matplotlib or pyright fails it.
  demo <slice>       runs the slice's demo headlessly and saves screenshots to docs/slices/<slice>/.
  validate [--report] [--against <revision>]
                     runs every validation case (tests named `validation_…`, declared with
                     `cav_validate::case!`, in the workspace and in the oracle crate) and tabulates
                     measured error against tolerance. A case that is declared but did not run is
                     reported as missing and fails the command.
                     It also holds the cases to docs/validation/report.json as committed at
                     the revision (HEAD unless --against names another, such as the last slice's
                     tag): a case that has gone, or a tolerance that has changed, fails unless
                     docs/validation/acknowledged.txt names it with a reason. A revision named
                     with --against that git cannot resolve, or that holds no report, is an error.
                     With --report it writes docs/validation/ (report.md, report.json, plots), and
                     only when every case passed and the comparison held: a failing run leaves
                     the committed report as it was.
  web [build|serve]  the application as a web page (wasm32, WebGPU): `build` writes target/web/dist
                     (the GitHub Pages workflow runs it); with no word, the tier: clippy for wasm32,
                     the build, and the engine's numbers in wasm (Node.js, wasm-bindgen-test-runner)
                     held to the native ones. Needs the wasm32 target, wasm-bindgen-cli at the lock's
                     version, binaryen's wasm-opt and Node.js. `serve` serves the built page at
                     http://127.0.0.1:8000/ until stopped.
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
        ["oracle"] => tiers::oracle(&root),
        ["demo", slice] => demo::run(&root, slice),
        ["validate"] => validate::run(&root, false, validate::Against::Head),
        ["validate", "--report"] => validate::run(&root, true, validate::Against::Head),
        ["validate", "--against", revision] => {
            validate::run(&root, false, validate::Against::Revision(revision))
        }
        ["validate", "--report", "--against", revision] | ["validate", "--against", revision, "--report"] => {
            validate::run(&root, true, validate::Against::Revision(revision))
        }
        ["web"] => web::tier(&root),
        ["web", "build"] => web::build(&root),
        ["web", "serve"] => web::serve_forever(&root),
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
    // Bytes, not text: a hook that is not UTF-8 (a binary, a script in another encoding) is still
    // a hook, and one that cannot be read at all is not ours to replace either.
    let existing = match std::fs::read(&hook) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(format!("{} exists and could not be read ({e}); it was left alone", hook.display()));
        }
    };
    if !may_write_hook(existing.as_deref(), script)? {
        eprintln!("xtask: {} is already this hook", hook.display());
        return Ok(());
    }
    std::fs::write(&hook, script).map_err(|e| format!("writing {}: {e}", hook.display()))?;
    eprintln!("xtask: wrote {}", hook.display());
    Ok(())
}

/// Whether the pre-push hook should be written: yes when there is none, no when it is already
/// ours.
///
/// # Errors
/// When a different hook is there. It is someone's, and it is not overwritten.
fn may_write_hook(existing: Option<&[u8]>, ours: &str) -> Result<bool, String> {
    // Ours, give or take the line endings a checkout tool may have changed. Anything that is not
    // text cannot be ours.
    let is_ours =
        |bytes: &[u8]| std::str::from_utf8(bytes).is_ok_and(|text| text.replace("\r\n", "\n") == ours);
    match existing {
        None => Ok(true),
        Some(bytes) if is_ours(bytes) => Ok(false),
        Some(_) => Err("a different .git/hooks/pre-push already exists; it was left alone. Move it away \
                        (or add `cargo xtask quick` to it yourself) and run this again"
            .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::may_write_hook;

    #[test]
    fn an_existing_hook_of_someone_elses_is_not_overwritten() {
        let ours = "#!/bin/sh\nexec cargo xtask quick\n";
        assert_eq!(may_write_hook(None, ours), Ok(true), "no hook: write it");
        assert_eq!(may_write_hook(Some(ours.as_bytes()), ours), Ok(false), "ours already: nothing to do");
        assert_eq!(may_write_hook(Some(b"#!/bin/sh\r\nexec cargo xtask quick\r\n"), ours), Ok(false));
        let refused = may_write_hook(Some(b"#!/bin/sh\nrun-my-linter\n"), ours).unwrap_err();
        assert!(refused.contains("left alone"), "{refused}");
    }

    #[test]
    fn a_hook_that_is_not_text_is_someone_elses_too() {
        let ours = "#!/bin/sh\nexec cargo xtask quick\n";
        // A compiled hook, and a script with one byte that is not UTF-8 (Latin-1 `é` in a comment).
        // Read as a string, each looked like "no hook" and was overwritten.
        for foreign in
            [&b"MZ\x90\x00\x03\xff\xfe\x00"[..], &b"#!/bin/sh\n# v\xe9rifier\nexec make check\n"[..]]
        {
            assert!(std::str::from_utf8(foreign).is_err(), "the case is not valid UTF-8");
            let refused = may_write_hook(Some(foreign), ours).unwrap_err();
            assert!(refused.contains("left alone"), "{refused}");
        }
        // Even ours with a stray byte is no longer ours.
        let mut damaged = ours.as_bytes().to_vec();
        damaged.push(0xff);
        assert!(may_write_hook(Some(&damaged), ours).is_err());
        // Control: the same decision still writes where there is nothing, and rests where it is ours.
        assert_eq!(may_write_hook(None, ours), Ok(true));
        assert_eq!(may_write_hook(Some(ours.as_bytes()), ours), Ok(false));
    }
}
