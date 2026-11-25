//! `cargo xtask strays [--stop | --self-test]`: a thin wrapper around `scripts/strays.ps1`, which
//! lists processes that look like an agent's leftovers (by command-line signature) and with `--stop`
//! stops the ones over their age or CPU threshold. The script does the work; this only finds it and
//! passes the exit code on. Windows only, like the leftovers it looks for.

use std::path::Path;

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
    use super::{Mode, parse, script_and_arguments};

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
