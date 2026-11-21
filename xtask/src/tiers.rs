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
    // Clippy includes `cargo check`. Cargo's fingerprints make it a no-op for unchanged crates, which
    // is the plan's "clippy on changed crates" without a second mechanism to get wrong.
    step(
        "clippy",
        cargo(root, &["clippy", "--workspace", "--all-targets", "--quiet", "--", "-D", "warnings"]),
    )?;
    run_tier(root, "quick", QUICK_FILTER)?;
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

/// The oracle tier. Honest about what it is: nothing to run until S1 brings the oracle crate.
pub fn oracle() -> Result<(), String> {
    eprintln!(
        "xtask: oracle: 0 checks. There is no physics to check in S0: the `oracle` crate (v1's forward \
         model at tag v1-final, and George's anchors) arrives with S1 (TASK-S1-02), and this tier will \
         then run it with a floor like the others. Nothing was verified by this command."
    );
    Ok(())
}

/// Runs the tests matching `filter` through cargo-nextest and holds the count to the tier's floor.
fn run_tier(root: &Path, tier: &str, filter: &str) -> Result<(), String> {
    let floor = floor_for(&read_floors(root)?, tier)?;
    let started = Instant::now();
    let mut command = cargo(root, &["nextest", "run", "--workspace", "--no-tests=pass", "-E", filter]);
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
    }
}
