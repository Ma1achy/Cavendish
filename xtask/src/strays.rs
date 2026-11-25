//! `cargo xtask strays [--stop | --self-test]`: a thin wrapper around `scripts/strays.ps1`, which
//! lists processes that look like an agent's leftovers (by command-line signature) and with `--stop`
//! stops the ones over their age or CPU threshold. The script does the work; this only finds it and
//! passes the exit code on. Windows only, like the leftovers it looks for.

use std::path::{Path, PathBuf};

/// Records older than this are removed when a new one is written: a week, seconds.
const RECORD_LIFE_S: u64 = 7 * 24 * 3600;

/// How long before `main` runs the process may have been made, milliseconds: the window a
/// record gives its process's start (`scripts/strays.ps1` takes a recorded id only for a process
/// that started inside it, so a reused id is not taken for this one).
const START_SLACK_MS: u128 = 10_000;

/// This `cargo xtask`'s record of itself, `target/strays/xtask-<pid>.txt`: its id and the window
/// of its start (`<id> <earliest, Unix ms> <latest, Unix ms> <what it was asked>`), which is how
/// `strays --stop` knows a process is xtask's or was started by it (S4's review, I10). Removed
/// when dropped, at the end of a run that ends normally; a run killed part-way leaves it, and
/// then what it started is xtask's leftover. Writing it never fails the run.
pub struct Record(Option<PathBuf>);

impl Record {
    /// Writes this process's record under `root/target/strays`, and removes records over a week
    /// old there.
    pub fn start(root: &Path, args: &[&str]) -> Self {
        let folder = root.join("target").join("strays");
        let now =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
        let id = std::process::id();
        let path = folder.join(format!("xtask-{id}.txt"));
        let text = record_text(id, now, &args.join(" "));
        if std::fs::create_dir_all(&folder).is_err() || std::fs::write(&path, text).is_err() {
            return Self(None);
        }
        if let Ok(entries) = std::fs::read_dir(&folder) {
            for entry in entries.flatten() {
                let old = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age.as_secs() > RECORD_LIFE_S);
                if old && entry.path() != path {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        Self(Some(path))
    }
}

impl Drop for Record {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// A record's text: the id, the window of its start (from `now` less [`START_SLACK_MS`] to a
/// second after `now`), and what was asked.
fn record_text(id: u32, now: u128, asked: &str) -> String {
    format!("{id} {} {} {asked}\n", now.saturating_sub(START_SLACK_MS), now + 1000)
}

/// What `cargo xtask strays …` was asked to do.
#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    /// List the processes that match a signature; fail when one is a stray.
    Report,
    /// Stop the strays (by signature, re-read before each stop); fail when one is left.
    Stop,
    /// Run `scripts/strays.test.ps1`, which starts and stops its own processes.
    SelfTest,
}

/// The mode for the words after `strays`.
///
/// # Errors
/// On anything but nothing, `--stop` or `--self-test`.
pub fn parse(args: &[&str]) -> Result<Mode, String> {
    match args {
        [] => Ok(Mode::Report),
        ["--stop"] => Ok(Mode::Stop),
        ["--self-test"] => Ok(Mode::SelfTest),
        other => Err(format!("strays takes nothing, --stop or --self-test, not `{}`", other.join(" "))),
    }
}

/// The script to run and the arguments to give it, relative to the workspace root.
pub fn script_and_arguments(mode: &Mode) -> (&'static str, &'static [&'static str]) {
    match mode {
        Mode::Report => ("scripts/strays.ps1", &[]),
        Mode::Stop => ("scripts/strays.ps1", &["-Stop"]),
        Mode::SelfTest => ("scripts/strays.test.ps1", &[]),
    }
}

/// Runs the script for `args` with Windows PowerShell; its output passes through.
pub fn run(root: &Path, args: &[&str]) -> Result<(), String> {
    let mode = parse(args)?;
    if !cfg!(windows) {
        return Err("strays looks for Windows processes; there is nothing to do here".into());
    }
    let (script, arguments) = script_and_arguments(&mode);
    let mut command = crate::gentle("powershell.exe");
    command
        .current_dir(root)
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(root.join(script))
        .args(arguments);
    let status = command.status().map_err(|e| format!("powershell.exe could not start: {e}"))?;
    match (mode, status.success()) {
        (_, true) => Ok(()),
        (Mode::Report, false) => Err("strays are running (listed above); `cargo xtask strays --stop`".into()),
        (Mode::Stop, false) => Err("a stray could not be stopped (listed above)".into()),
        (Mode::SelfTest, false) => Err("the strays self-test failed".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::{Mode, Record, parse, record_text, script_and_arguments};

    #[test]
    fn a_run_records_itself_and_removes_the_record() {
        // The line the script reads: id, then a window that holds now, in Unix milliseconds.
        let text = record_text(4242, 1_700_000_000_000, "web serve");
        let words: Vec<&str> = text.split_whitespace().collect();
        assert_eq!(words[..3], ["4242", "1699999990000", "1700000001000"]);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/test-strays");
        let _ = std::fs::remove_dir_all(&root);
        let path = root.join("target/strays").join(format!("xtask-{}.txt", std::process::id()));
        {
            let _record = Record::start(&root, &["demo", "S4"]);
            let line = std::fs::read_to_string(&path).unwrap();
            assert!(
                line.starts_with(&format!("{} ", std::process::id())) && line.contains("demo S4"),
                "{line}"
            );
        }
        // Control: dropped (a run that ends normally), the record is gone.
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn strays_reports_by_default_and_stops_only_when_asked() {
        assert_eq!(parse(&[]), Ok(Mode::Report));
        assert_eq!(script_and_arguments(&Mode::Report), ("scripts/strays.ps1", &[][..]));
        assert_eq!(parse(&["--stop"]), Ok(Mode::Stop));
        assert_eq!(script_and_arguments(&Mode::Stop), ("scripts/strays.ps1", &["-Stop"][..]));
        assert_eq!(parse(&["--self-test"]), Ok(Mode::SelfTest));
        assert_eq!(script_and_arguments(&Mode::SelfTest).0, "scripts/strays.test.ps1");
        // Control: nothing else is taken, so a typo never runs a stop it did not ask for or one
        // it did, and the script's own switches are not passed through.
        for wrong in [&["--Stop"][..], &["stop"], &["-Stop"], &["--stop", "--self-test"], &["--stop", "x"]] {
            let refused = parse(wrong).unwrap_err();
            assert!(refused.contains("--stop"), "{refused}");
        }
    }

    #[test]
    fn the_scripts_the_wrapper_names_exist() {
        let root = super::super::root();
        for mode in [Mode::Report, Mode::Stop, Mode::SelfTest] {
            let script = root.join(script_and_arguments(&mode).0);
            assert!(script.is_file(), "{} is missing", script.display());
        }
        // Control: a name that is not there is seen as missing.
        assert!(!root.join("scripts/strays-none.ps1").is_file());
    }
}
