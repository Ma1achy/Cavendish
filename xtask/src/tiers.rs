//! The test tiers and their guardrail: a recorded floor on how many tests each tier executes.
//!
//! A tier that silently runs fewer tests than it used to (a filter typo, a deleted file, a `#[cfg]`
//! that stopped matching) would still be green. The floor in `xtask/floors.txt` makes that a failure.
//! Floors only go up: when a tier runs more tests than its floor, xtask says so and the builder
//! raises the number in the same commit.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Stdio;
use std::time::Instant;

use crate::{cargo, step};

/// The warm budget for `quick`, seconds (S0's gate; REBUILD §8 allows 30 s in later slices).
const QUICK_BUDGET_S: f64 = 10.0;

/// Tests that need a GPU carry `gpu_` in their names; everything else is the quick tier.
const QUICK_FILTER: &str = "not test(gpu_)";
const GPU_FILTER: &str = "test(gpu_)";

pub fn quick(root: &Path, enforce_budget: bool) -> Result<(), String> {
    let started = Instant::now();
    step("fmt", cargo(root, &["fmt", "--all", "--check"]))?;
    // The oracle crate is outside the workspace. Checking its formatting compiles nothing, so it
    // is checked here on every commit; its lints and tests wait for `cargo xtask oracle`.
    step("fmt (oracle)", cargo(root, &["fmt", "--manifest-path", ORACLE_MANIFEST, "--check"]))?;
    // Clippy includes `cargo check`. Cargo's fingerprints make it a no-op for unchanged crates, which
    // is the plan's "clippy on changed crates" without a second mechanism to get wrong.
    step(
        "clippy",
        cargo(root, &["clippy", "--workspace", "--all-targets", "--quiet", "--", "-D", "warnings"]),
    )?;
    run_tier(root, "quick", QUICK_FILTER)?;
    // Documentation examples are not built here: the one there was cost two to three seconds of
    // the ten, and is a unit test now. `cargo xtask oracle` runs any that are written later.
    let took = started.elapsed().as_secs_f64();
    let verdict = if took <= QUICK_BUDGET_S { "within" } else { "OVER" };
    eprintln!("xtask: quick took {took:.1} s, {verdict} the {QUICK_BUDGET_S:.0} s warm budget");
    if enforce_budget && took > QUICK_BUDGET_S {
        return Err(format!("quick took {took:.1} s, over the {QUICK_BUDGET_S:.0} s warm budget"));
    }
    Ok(())
}

pub fn gpu(root: &Path) -> Result<(), String> {
    run_tier(root, "gpu", GPU_FILTER)
}

/// The oracle crate: a workspace of its own, so that the main one never fetches or compiles v1.
pub const ORACLE_MANIFEST: &str = "oracle/Cargo.toml";
/// Its artefacts go under the main `target/`, which is the directory excluded from virus scanning.
pub const ORACLE_TARGET: &str = "target/oracle";

/// The arguments that run the oracle crate's tests through nextest, with the main workspace's
/// nextest configuration (the watchdog that kills a test at two minutes).
pub const ORACLE_NEXTEST: [&str; 9] = [
    "nextest",
    "run",
    "--manifest-path",
    ORACLE_MANIFEST,
    "--target-dir",
    ORACLE_TARGET,
    "--config-file",
    ".config/nextest.toml",
    "--no-tests=pass",
];

/// The oracle tier: v1's forward model at tag `v1-final`, compiled from source, and George's
/// reference scripts as fixtures, each holding the engine to a stated tolerance. The crate is
/// outside the workspace, so it is formatted and linted here as well as tested.
pub fn oracle(root: &Path) -> Result<(), String> {
    step("oracle fmt", cargo(root, &["fmt", "--manifest-path", ORACLE_MANIFEST, "--check"]))?;
    let clippy = ["clippy", "--manifest-path", ORACLE_MANIFEST, "--target-dir", ORACLE_TARGET];
    let lints = ["--all-targets", "--quiet", "--", "-D", "warnings"];
    step("oracle clippy", cargo(root, &[&clippy[..], &lints[..]].concat()))?;
    // nextest does not run documentation examples, and `quick` has no time for them.
    step("doctests", cargo(root, &["test", "--doc", "--workspace", "--quiet"]))?;
    // The Python side: typed, and its tests pass on figures this build writes. Never skipped.
    crate::python::run(root)?;
    run_tests(root, "oracle", &ORACLE_NEXTEST)
}

/// Runs the tests matching `filter` through cargo-nextest and holds the count to the tier's floor.
fn run_tier(root: &Path, tier: &str, filter: &str) -> Result<(), String> {
    run_tests(root, tier, &["nextest", "run", "--workspace", "--no-tests=pass", "-E", filter])
}

/// Runs cargo-nextest with `arguments` and holds the number of tests it ran to the tier's floor.
fn run_tests(root: &Path, tier: &str, arguments: &[&str]) -> Result<(), String> {
    let floor = floor_for(&read_floors(root)?, tier)?;
    let started = Instant::now();
    let mut command = cargo(root, arguments);
    // nextest reports on stderr; pass it through line by line and keep it for the count.
    let mut child = command
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{tier}: could not start cargo-nextest (is it installed?): {e}"))?;
    let mut report = String::new();
    for line in BufReader::new(child.stderr.take().expect("stderr is piped")).lines() {
        let line = line.map_err(|e| format!("{tier}: reading nextest's output: {e}"))?;
        eprintln!("{line}");
        report.push_str(&line);
        report.push('\n');
    }
    let status = child.wait().map_err(|e| format!("{tier}: waiting for nextest: {e}"))?;
    eprintln!("xtask: {tier} tests: {:.1} s", started.elapsed().as_secs_f64());
    if !status.success() {
        return Err(format!("{tier}: tests failed ({status})"));
    }
    let ran = tests_run(&report).ok_or_else(|| format!("{tier}: no test count in nextest's output"))?;
    check_floor(tier, ran, floor)
}

/// The number of tests nextest executed, from its summary line
/// (`Summary [ 0.4s] 12 tests run: 12 passed, 3 skipped`). `None` if there is no such line; zero when
/// nextest says no tests were run.
pub fn tests_run(report: &str) -> Option<usize> {
    for line in report.lines().rev() {
        if let Some(head) = line.split(" run: ").next().filter(|_| line.contains(" run: ")) {
            let mut words = head.split_whitespace().rev();
            let noun = words.next()?;
            if noun == "test" || noun == "tests" {
                return words.next()?.parse().ok();
            }
        }
        if line.contains("no tests to run") {
            return Some(0);
        }
    }
    None
}

/// Fewer tests than the floor is a failure; more is a reminder to raise it.
pub fn check_floor(tier: &str, ran: usize, floor: usize) -> Result<(), String> {
    if ran < floor {
        return Err(format!(
            "{tier}: {ran} tests ran, below the floor of {floor} in xtask/floors.txt. A test was lost: \
             find it; do not lower the floor."
        ));
    }
    if ran > floor {
        eprintln!(
            "xtask: {tier}: {ran} tests ran, floor {floor}: raise the floor in xtask/floors.txt to {ran}"
        );
    } else {
        eprintln!("xtask: {tier}: {ran} tests ran, at the floor");
    }
    Ok(())
}

fn read_floors(root: &Path) -> Result<String, String> {
    let path = root.join("xtask").join("floors.txt");
    std::fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))
}

/// The floor for `tier` in the text of `floors.txt`: lines of `<tier> <count>`, `#` starts a comment.
pub fn floor_for(floors: &str, tier: &str) -> Result<usize, String> {
    for line in floors.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let mut words = line.split_whitespace();
        if words.next() == Some(tier) {
            let count = words.next().ok_or_else(|| format!("floors.txt: `{tier}` has no count"))?;
            return count.parse().map_err(|e| format!("floors.txt: `{tier} {count}`: {e}"));
        }
    }
    Err(format!("floors.txt has no floor for the `{tier}` tier"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = "\
    Starting 12 tests across 3 binaries (2 tests skipped)
        PASS [   0.012s] cav-core maths::tests::cross_product_is_right_handed
------------
     Summary [   0.412s] 12 tests run: 12 passed, 2 skipped
";

    #[test]
    fn nextest_summary_gives_the_number_of_tests_run() {
        assert_eq!(tests_run(REPORT), Some(12));
        assert_eq!(tests_run("     Summary [   0.1s] 1 test run: 1 passed, 0 skipped\n"), Some(1));
        // A failing run still reports how many ran.
        assert_eq!(tests_run("     Summary [   0.1s] 7 tests run: 6 passed, 1 failed, 0 skipped\n"), Some(7));
        assert_eq!(tests_run("    Starting 0 tests across 2 binaries\nerror: no tests to run\n"), Some(0));
        // Control: output with no summary gives no count, so a changed format fails the tier
        // instead of passing it with a made-up number.
        assert_eq!(tests_run("   Compiling cav-core v0.0.0\nerror: could not compile\n"), None);
        assert_eq!(tests_run("12 tests were found: none run"), None);
    }

    #[test]
    fn fewer_tests_than_the_floor_fails_the_tier() {
        assert!(check_floor("quick", 12, 12).is_ok());
        assert!(check_floor("quick", 13, 12).is_ok());
        // The property being guarded: a lost test is a failure.
        let lost = check_floor("quick", 11, 12).unwrap_err();
        assert!(lost.contains("11 tests ran") && lost.contains("floor of 12"), "{lost}");
        assert!(check_floor("gpu", 0, 1).is_err());
    }

    #[test]
    fn floors_file_is_read_per_tier_and_a_missing_tier_is_an_error() {
        let floors = "# tier count\nquick 12 # CPU\ngpu 3\n";
        assert_eq!(floor_for(floors, "quick"), Ok(12));
        assert_eq!(floor_for(floors, "gpu"), Ok(3));
        // Control: no silent default of zero, which would make the floor vacuous.
        assert!(floor_for(floors, "oracle").is_err());
        assert!(floor_for("quick\n", "quick").is_err());
        assert!(floor_for("quick many\n", "quick").is_err());
    }

    #[test]
    fn the_committed_floors_file_names_both_tiers() {
        let floors = include_str!("../floors.txt");
        assert!(floor_for(floors, "quick").unwrap() > 0, "quick must run something");
        assert!(floor_for(floors, "gpu").is_ok());
        // From S1 the oracle tier is a tier like the others: it has a floor, and not an empty one.
        assert!(floor_for(floors, "oracle").unwrap() > 0, "the oracle tier must check something");
    }

    #[test]
    fn the_oracle_tier_builds_its_own_crate_outside_the_workspace() {
        // The crate is where the tier says it is, and it is a workspace of its own …
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let manifest = std::fs::read_to_string(root.join(ORACLE_MANIFEST)).unwrap();
        assert!(manifest.lines().any(|line| line.trim() == "[workspace]"), "its own workspace");
        assert!(
            manifest.contains("56df4ab2126dce30fa0f26cb9321885511a105cc"),
            "v1 pinned to v1-final's commit"
        );
        // … which the main workspace leaves out, so `quick` never fetches or compiles v1.
        let main = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
        assert!(main.contains("exclude = [\"oracle\"]"));
        // Control: the tier's arguments name that manifest and no other.
        assert!(ORACLE_NEXTEST.contains(&ORACLE_MANIFEST) && !ORACLE_NEXTEST.contains(&"--workspace"));
        assert!(ORACLE_TARGET.starts_with("target/"), "artefacts stay under the excluded directory");
    }
}
