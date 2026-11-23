//! The Python side of the oracle tier: `python/cavendish_figures` is typed (pyright, strict) and
//! its tests pass, on figures that `cav figure` has just written as well as on its own.
//!
//! Nothing here is skipped: an interpreter, a module or a checker that is not there fails the
//! step, saying what is missing.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::{cargo, step};

/// The step's name, in every message it gives.
pub const STEP: &str = "python_is_typed_and_its_tests_pass";

/// The environment variable that names the interpreter, in place of `python` on the path.
pub const INTERPRETER: &str = "CAV_PYTHON";

/// What the Python side needs of the interpreter, each as the module to import.
pub const NEEDS: [&str; 3] = ["numpy", "matplotlib", "pyright"];

/// The figures the tests read back: what to draw, of which scene, in which preset.
const FIGURES: [(&str, &str, &str, &str); 4] = [
    ("comb", "periodogram", "paper-column", "periodogram.pdf"),
    ("pass", "signal", "slide", "signal.svg"),
    ("cadence-lab", "cadence", "paper-full", "cadence.pdf"),
    ("staggered", "flight", "paper-full", "flight.svg"),
];

/// Whether `interpreter` runs `code` to a clean exit, saying nothing.
fn runs(interpreter: &str, code: &str) -> bool {
    Command::new(interpreter)
        .args(["-c", code])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// What `interpreter` lacks of what the step needs, in words; `None` when it has everything.
pub fn missing(interpreter: &str) -> Option<String> {
    if !runs(interpreter, "import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)") {
        return Some(format!(
            "{STEP}: `{interpreter}` is not a Python of 3.10 or later that can be run; install one, or name \
             it in {INTERPRETER}"
        ));
    }
    let absent: Vec<&str> =
        NEEDS.into_iter().filter(|module| !runs(interpreter, &format!("import {module}"))).collect();
    (!absent.is_empty()).then(|| {
        format!(
            "{STEP}: `{interpreter}` has no {}; install with `{interpreter} -m pip install {}`",
            absent.join(", "),
            absent.join(" ")
        )
    })
}

/// Runs pyright over `python/` and then the package's tests, with real figures to read.
pub fn run(root: &Path) -> Result<(), String> {
    let interpreter = std::env::var(INTERPRETER).unwrap_or_else(|_| "python".to_owned());
    if let Some(why) = missing(&interpreter) {
        return Err(why);
    }
    // Figures written by this build, for the tests to read back and redraw.
    let figures = root.join("target").join("tmp").join("python figures");
    let _ = std::fs::remove_dir_all(&figures);
    std::fs::create_dir_all(&figures).map_err(|e| format!("{STEP}: {}: {e}", figures.display()))?;
    for (scene, what, preset, file) in FIGURES {
        let out = figures.join(file);
        let out = out.to_string_lossy();
        let arguments =
            ["run", "--quiet", "-p", "cav", "--", "figure", scene, what, "--preset", preset, "--out"];
        let mut command = cargo(root, &arguments);
        command.arg(out.as_ref()).stdout(Stdio::null());
        step(&format!("{STEP}: cav figure {scene} {what}"), command)?;
    }
    let python = root.join("python");
    let mut pyright = Command::new(&interpreter);
    pyright.current_dir(&python).args(["-m", "pyright"]);
    step(&format!("{STEP}: pyright"), pyright)?;
    let mut tests = Command::new(&interpreter);
    tests.current_dir(&python).arg("tests/test_figures.py").env("CAVENDISH_FIGURES", &figures);
    step(&format!("{STEP}: tests"), tests)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_interpreter_or_module_fails_the_python_step_by_name() {
        // An interpreter that is not there is said to be missing, not skipped.
        let why = missing("no-such-python-anywhere").expect("a missing interpreter is found missing");
        assert!(why.starts_with(STEP) && why.contains("`no-such-python-anywhere` is not a Python"), "{why}");
        assert!(why.contains(INTERPRETER));
        // Something that runs but is not Python (cargo itself) is not taken for one.
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        assert!(missing(&cargo).is_some_and(|why| why.contains("is not a Python")));
        // The step is part of the oracle tier, under its name, and not behind a condition.
        let tiers = include_str!("tiers.rs");
        let oracle = tiers.split("pub fn oracle").nth(1).expect("the oracle tier");
        let body = oracle.split("\n}\n").next().unwrap();
        assert!(body.contains("crate::python::run(root)?"), "the oracle tier does not run the Python step");
        assert_eq!(STEP, "python_is_typed_and_its_tests_pass");
        // Control: where this machine has a Python with everything, nothing is missing. (Where
        // it has not, the oracle tier fails with the message above; this test does not need one.)
        let here = std::env::var(INTERPRETER).unwrap_or_else(|_| "python".to_owned());
        if runs(&here, "import numpy, matplotlib, pyright") {
            assert_eq!(missing(&here), None);
        }
    }
}
