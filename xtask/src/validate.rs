//! `cargo xtask validate [--report]`: every validation case that exists, run and tabulated.
//!
//! Cases are declared with `cav_validate::case!` next to the code they check. This command finds
//! the declarations by reading the source, runs the tests with a directory to record into, and
//! lays the two lists side by side. A case that is declared but recorded nothing is **missing**,
//! and the command fails: a report cannot go green by a case quietly not running.
//!
//! With `--report` it writes `docs/validation/report.json` and `report.md`, and asks
//! `scripts/validation_plots.py` to draw the plots beside them.

use std::collections::BTreeMap;
use std::path::Path;

use crate::{cargo, step};

/// Tests that are validation cases carry this in their names.
const FILTER: &str = "test(validation_)";

/// A case as the source declares it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declared {
    pub name: String,
    pub test: String,
    pub subsystem: String,
    pub slice: String,
    pub has_tolerance: bool,
    /// `file:line` of the declaration.
    pub at: String,
}

/// How a case came out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    /// Declared, but it recorded nothing: it did not run.
    Missing,
}

impl Status {
    const fn name(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Missing => "missing",
        }
    }
}

/// One row of the report.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    pub subsystem: String,
    pub slice: String,
    pub status: Status,
    /// The record's fields, as the case wrote them; empty for a missing case.
    pub fields: BTreeMap<String, String>,
}

impl Row {
    fn text(&self, key: &str) -> &str {
        self.fields.get(key).map_or("", String::as_str)
    }

    fn number(&self, key: &str) -> Option<f64> {
        self.fields.get(key)?.parse().ok()
    }

    /// The measured error as a fraction of the tolerance: at most 1 for a pass. An exact case
    /// (tolerance zero) has margin 0 when it measured zero and is infinitely over otherwise.
    pub fn margin(&self) -> Option<f64> {
        let (measured, tolerance) = (self.number("measured")?, self.number("tolerance")?);
        Some(if tolerance > 0.0 {
            measured / tolerance
        } else if measured == 0.0 {
            0.0
        } else {
            f64::INFINITY
        })
    }
}

/// The value of `key: value,` on a declaration line, without quotes or the trailing comma.
fn declared_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line.trim().strip_prefix(key)?.strip_prefix(':')?.trim();
    Some(rest.trim_end_matches(',').trim().trim_matches('"'))
}

/// The cases declared in one source file. Comment lines are not declarations.
pub fn scan_source(file: &str, text: &str) -> Vec<Declared> {
    let lines: Vec<&str> = text.lines().collect();
    let mut found = Vec::new();
    for (n, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || !(trimmed == "case! {" || trimmed.ends_with("::case! {")) {
            continue;
        }
        let mut case = Declared {
            name: String::new(),
            test: String::new(),
            subsystem: String::new(),
            slice: String::new(),
            has_tolerance: false,
            at: format!("{file}:{}", n + 1),
        };
        // The macro fixes the order of the fields and ends them with `measure:`.
        for field in lines[n + 1..].iter().take_while(|l| !l.trim().starts_with("measure:")) {
            if let Some(value) = declared_value(field, "name") {
                case.name = value.to_owned();
            } else if let Some(value) = declared_value(field, "test") {
                case.test = value.to_owned();
            } else if let Some(value) = declared_value(field, "subsystem") {
                case.subsystem = value.to_owned();
            } else if let Some(value) = declared_value(field, "slice") {
                case.slice = value.to_owned();
            } else if let Some(value) = declared_value(field, "tolerance") {
                case.has_tolerance = value.parse::<f64>().is_ok();
            }
        }
        found.push(case);
    }
    found
}

/// Where cases are declared: the workspace's crates, and the oracle crate beside them (a
/// workspace of its own, whose cases are run by a second nextest run).
pub const CASE_ROOTS: [&str; 2] = ["crates", "oracle"];

/// Every case declared under [`CASE_ROOTS`].
fn scan(root: &Path) -> Result<Vec<Declared>, String> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<Declared>) -> Result<(), String> {
        let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut paths: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                walk(&path, root, out)?;
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                let file = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                out.extend(scan_source(&file, &text));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    for directory in CASE_ROOTS {
        walk(&root.join(directory), root, &mut out)?;
    }
    Ok(out)
}

/// A record file's fields: `key<TAB>value` lines.
pub fn parse_record(text: &str) -> BTreeMap<String, String> {
    text.lines().filter_map(|line| line.split_once('\t')).map(|(k, v)| (k.to_owned(), v.to_owned())).collect()
}

/// Lays the declared cases beside what was recorded.
///
/// # Errors
/// What makes the collection itself untrustworthy: a name declared twice, a declaration without
/// a tolerance or a name, a test the validation filter would not run, or a record that no
/// declaration accounts for (the scan and the macro have drifted apart).
pub fn collate(declared: &[Declared], records: &[BTreeMap<String, String>]) -> Result<Vec<Row>, String> {
    let mut problems = Vec::new();
    for (n, case) in declared.iter().enumerate() {
        if case.name.is_empty() {
            problems.push(format!("{}: a case has no name", case.at));
        }
        if !case.has_tolerance {
            problems.push(format!("{}: case `{}` has no tolerance", case.at, case.name));
        }
        if !case.test.starts_with("validation_") {
            problems.push(format!(
                "{}: case `{}`: its test `{}` must be named `validation_…` or it would never run here",
                case.at, case.name, case.test
            ));
        }
        if let Some(first) = declared[..n].iter().find(|other| other.name == case.name) {
            problems.push(format!("case `{}` is declared twice: {} and {}", case.name, first.at, case.at));
        }
    }
    for record in records {
        let name = record.get("name").map_or("", String::as_str);
        if !declared.iter().any(|case| case.name == name) {
            problems
                .push(format!("a case recorded as `{name}` is not among the declarations the scan found"));
        }
    }
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    let mut rows: Vec<Row> = declared
        .iter()
        .map(|case| {
            let record = records.iter().find(|r| r.get("name") == Some(&case.name));
            let status = match record.and_then(|r| r.get("pass")).map(String::as_str) {
                Some("true") => Status::Pass,
                Some(_) => Status::Fail,
                None => Status::Missing,
            };
            Row {
                name: case.name.clone(),
                subsystem: case.subsystem.clone(),
                slice: case.slice.clone(),
                status,
                fields: record.cloned().unwrap_or_default(),
            }
        })
        .collect();
    // By subsystem; within one, the worst margin first, with missing and failed cases on top.
    let key = |row: &Row| match row.status {
        Status::Missing => f64::INFINITY,
        _ => row.margin().unwrap_or(f64::INFINITY),
    };
    rows.sort_by(|a, b| {
        a.subsystem.cmp(&b.subsystem).then(key(b).total_cmp(&key(a))).then(a.name.cmp(&b.name))
    });
    Ok(rows)
}

/// The counts of a report: `(total, passing, failing, missing)`.
pub fn counts(rows: &[Row]) -> (usize, usize, usize, usize) {
    let of = |status| rows.iter().filter(|r| r.status == status).count();
    (rows.len(), of(Status::Pass), of(Status::Fail), of(Status::Missing))
}

/// The case that ran with the least room to spare.
fn worst(rows: &[Row]) -> Option<(&Row, f64)> {
    rows.iter().filter_map(|r| Some((r, r.margin()?))).max_by(|a, b| a.1.total_cmp(&b.1))
}

/// Whether the report is good: every declared case ran and passed.
///
/// # Errors
/// The cases that failed or are missing, by name.
pub fn verdict(rows: &[Row]) -> Result<(), String> {
    let named = |status| {
        let names: Vec<&str> = rows.iter().filter(|r| r.status == status).map(|r| r.name.as_str()).collect();
        names.join(", ")
    };
    let (total, _, failing, missing) = counts(rows);
    if total == 0 {
        return Err("no validation cases were found: the scan is broken or the cases are gone".into());
    }
    match (failing, missing) {
        (0, 0) => Ok(()),
        (_, 0) => Err(format!("{failing} case(s) failed: {}", named(Status::Fail))),
        (0, _) => Err(format!(
            "{missing} case(s) are declared but did not run (missing): {}",
            named(Status::Missing)
        )),
        _ => Err(format!(
            "{failing} failed ({}) and {missing} are declared but did not run ({})",
            named(Status::Fail),
            named(Status::Missing)
        )),
    }
}

/// One line on the whole report.
pub fn summary(rows: &[Row]) -> String {
    let (total, passing, failing, missing) = counts(rows);
    let mut text = format!("{passing} of {total} cases pass");
    if failing > 0 {
        text.push_str(&format!(", {failing} fail"));
    }
    if missing > 0 {
        text.push_str(&format!(", {missing} missing"));
    }
    if let Some((row, margin)) = worst(rows) {
        text.push_str(&format!("; worst margin: {} at {} of its tolerance", row.name, percent(margin)));
    }
    text
}

fn percent(margin: f64) -> String {
    if margin.is_finite() { format!("{:.0}%", 100.0 * margin) } else { "over".to_owned() }
}

/// A JSON string literal.
fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A JSON number, or `null` for what JSON cannot write.
fn json_number(value: Option<f64>) -> String {
    match value {
        Some(v) if v.is_finite() => format!("{v:?}"),
        _ => "null".to_owned(),
    }
}

/// The report as JSON, for scripts and for the plots. It holds no time and no commit, so that
/// regenerating it without a change in the results changes nothing.
pub fn report_json(rows: &[Row]) -> String {
    let (total, passing, failing, missing) = counts(rows);
    let worst = worst(rows).map_or("null".to_owned(), |(row, margin)| {
        format!("{{\"name\": {}, \"margin\": {}}}", json_string(&row.name), json_number(Some(margin)))
    });
    let mut out = format!(
        "{{\n  \"schema\": 1,\n  \"summary\": {{\"total\": {total}, \"passing\": {passing}, \"failing\": {failing}, \
         \"missing\": {missing}, \"worst\": {worst}}},\n  \"cases\": [\n"
    );
    for (n, row) in rows.iter().enumerate() {
        let order = match row.number("order_expected") {
            Some(expected) => format!(
                "{{\"expected\": {}, \"within\": {}, \"measured\": {}}}",
                json_number(Some(expected)),
                json_number(row.number("order_within")),
                json_number(row.number("order_measured"))
            ),
            None => "null".to_owned(),
        };
        let points: Vec<String> = row
            .text("points")
            .split(';')
            .filter_map(|p| p.split_once(':'))
            .map(|(h, e)| format!("[{}, {}]", json_number(h.parse().ok()), json_number(e.parse().ok())))
            .collect();
        let why = row.fields.get("why").map_or("null".to_owned(), |w| json_string(w));
        out.push_str(&format!(
            "    {{\"name\": {}, \"subsystem\": {}, \"slice\": {}, \"status\": {}, \"reference\": {}, \"against\": {}, \
             \"unit\": {}, \"measured\": {}, \"tolerance\": {}, \"margin\": {}, \"order\": {order}, \"points\": [{}], \
             \"why\": {why}}}{}\n",
            json_string(&row.name),
            json_string(&row.subsystem),
            json_string(&row.slice),
            json_string(row.status.name()),
            json_string(row.text("reference")),
            json_string(row.text("against")),
            json_string(row.text("unit")),
            json_number(row.number("measured")),
            json_number(row.number("tolerance")),
            json_number(row.margin()),
            points.join(", "),
            if n + 1 < rows.len() { "," } else { "" },
        ));
    }
    out.push_str("  ]\n}\n");
    out
}

/// A number as the report's tables show it.
fn shown(value: Option<f64>) -> String {
    match value {
        None => "—".to_owned(),
        Some(0.0) => "0".to_owned(),
        Some(v) if v.is_finite() && (1e-3..1e4).contains(&v.abs()) => {
            format!("{v:.4}").trim_end_matches('0').trim_end_matches('.').to_owned()
        }
        Some(v) => format!("{v:.3e}"),
    }
}

/// The report as Markdown: a table per subsystem, worst margin first.
pub fn report_md(rows: &[Row], acknowledged: &[String]) -> String {
    let mut out = String::from(
        "# Validation report\n\n\
         Written by `cargo xtask validate --report`; do not edit by hand. Every case is declared with\n\
         `cav_validate::case!` beside the code it checks, and all of them are listed here: one that did not run\n\
         is shown as **missing**, never left out.\n\n",
    );
    out.push_str(&format!("**{}.**\n\n", summary(rows)));
    out.push_str(
        "<picture><source media=\"(prefers-color-scheme: dark)\" srcset=\"error-vs-tolerance-dark.svg\">\
         <img src=\"error-vs-tolerance-light.svg\" alt=\"Each case's measured error against its tolerance, on a log scale.\" width=\"100%\"></picture>\n\n",
    );
    let mut subsystem = "";
    for row in rows {
        if row.subsystem != subsystem {
            subsystem = &row.subsystem;
            out.push_str(&format!(
                "\n## {subsystem}\n\n| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |\n|---|---|---|---|---|---|---|---|\n"
            ));
        }
        let order =
            match (row.number("order_measured"), row.number("order_expected"), row.number("order_within")) {
                (Some(found), Some(expected), Some(within)) => {
                    format!("{found:.2} (expected {expected} ± {within})")
                }
                (None, Some(expected), _) => format!("— (expected {expected})"),
                _ => String::new(),
            };
        let result = match row.status {
            Status::Pass => "pass".to_owned(),
            Status::Fail => format!("**FAIL**: {}", row.text("why")),
            Status::Missing => "**MISSING**: declared, did not run".to_owned(),
        };
        let against = match (row.text("reference"), row.text("against")) {
            ("", _) => "—".to_owned(),
            (reference, against) => format!("{reference}: {}", against.replace('|', "/")),
        };
        out.push_str(&format!(
            "| `{}` ({}) | {against} | {} | {} | {} | {} | {order} | {result} |\n",
            row.name,
            row.slice,
            shown(row.number("measured")),
            shown(row.number("tolerance")),
            if row.text("unit").is_empty() { "—" } else { row.text("unit") },
            row.margin().map_or("—".to_owned(), percent),
        ));
    }
    if !acknowledged.is_empty() {
        out.push_str("\n## Changes against the earlier report, each with its reason\n\n");
        for change in acknowledged {
            out.push_str(&format!("- {change}\n"));
        }
    }
    out.push_str(
        "\n## Convergence\n\n\
         <picture><source media=\"(prefers-color-scheme: dark)\" srcset=\"convergence-dark.svg\">\
         <img src=\"convergence-light.svg\" alt=\"Error against step size for each convergence case, with the expected rate drawn in.\" width=\"100%\"></picture>\n\n\
         ## Over time\n\n\
         <picture><source media=\"(prefers-color-scheme: dark)\" srcset=\"history-dark.svg\">\
         <img src=\"history-light.svg\" alt=\"Each case's error as a fraction of its tolerance at every commit that changed this report.\" width=\"100%\"></picture>\n\n\
         The history is read from the earlier versions of `report.json` in git.\n",
    );
    out
}

/// A case of an earlier report: its name and its tolerance.
pub type Earlier = (String, Option<f64>);

/// The cases of an earlier `report.json`, read from the one-case-per-line form this file writes.
pub fn earlier_cases(json: &str) -> Vec<Earlier> {
    json.lines()
        .filter_map(|line| {
            let line = line.trim();
            let name = line.strip_prefix("{\"name\": \"")?.split('"').next()?;
            let tolerance =
                line.split("\"tolerance\": ").nth(1).and_then(|rest| rest.split([',', '}']).next());
            Some((name.to_owned(), tolerance.and_then(|t| t.trim().parse().ok())))
        })
        .collect()
}

/// A change to the report that someone has owned up to, in `docs/validation/acknowledged.txt`:
/// `dropped <case>: <why>` or `tolerance <case> <old> -> <new>: <why>`.
#[derive(Clone, Debug, PartialEq)]
pub enum Acknowledged {
    Dropped { case: String, why: String },
    Tolerance { case: String, old: f64, new: f64, why: String },
}

/// Reads the acknowledgements. Blank lines and `#` comments are skipped.
///
/// # Errors
/// A line that is neither form, or that gives no reason.
pub fn parse_acknowledged(text: &str) -> Result<Vec<Acknowledged>, String> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let wrong = || {
            format!(
                "acknowledged.txt line {}: `{line}` is neither `dropped <case>: <why>` nor \
                 `tolerance <case> <old> -> <new>: <why>`",
                n + 1
            )
        };
        let (what, why) = line.split_once(':').ok_or_else(wrong)?;
        let why = why.trim().to_owned();
        if why.is_empty() {
            return Err(format!("acknowledged.txt line {}: no reason is given", n + 1));
        }
        let words: Vec<&str> = what.split_whitespace().collect();
        out.push(match words.as_slice() {
            ["dropped", case] => Acknowledged::Dropped { case: (*case).to_owned(), why },
            ["tolerance", case, old, "->", new] => Acknowledged::Tolerance {
                case: (*case).to_owned(),
                old: old.parse().map_err(|_| wrong())?,
                new: new.parse().map_err(|_| wrong())?,
                why,
            },
            _ => return Err(wrong()),
        });
    }
    Ok(out)
}

/// Holds this report to an earlier one: no case may have gone, and no tolerance may have changed,
/// unless `acknowledged` names the case and says why.
///
/// Returns the changes that were acknowledged, for the report to print.
///
/// # Errors
/// Each dropped case and each changed tolerance that nobody acknowledged.
pub fn changes(
    earlier: &[Earlier],
    rows: &[Row],
    acknowledged: &[Acknowledged],
) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    let mut owned = Vec::new();
    for (name, old) in earlier {
        let Some(row) = rows.iter().find(|row| &row.name == name) else {
            let reason = acknowledged.iter().find_map(|a| match a {
                Acknowledged::Dropped { case, why } if case == name => Some(why),
                _ => None,
            });
            match reason {
                Some(why) => owned.push(format!("`{name}` was dropped: {why}")),
                None => problems.push(format!(
                    "case `{name}` is in the earlier report and has gone. Never drop a case to make the report \
                     green; if what it checked has gone, say so in docs/validation/acknowledged.txt: \
                     `dropped {name}: <why>`"
                )),
            }
            continue;
        };
        let (Some(old), Some(new)) = (*old, row.number("tolerance")) else { continue };
        if old.to_bits() == new.to_bits() {
            continue;
        }
        let reason = acknowledged.iter().find_map(|a| match a {
            Acknowledged::Tolerance { case, old: from, new: to, why }
                if case == name && *from == old && *to == new =>
            {
                Some(why)
            }
            _ => None,
        });
        match reason {
            Some(why) => owned.push(format!("`{name}`: tolerance {old:?} → {new:?}: {why}")),
            None => problems.push(format!(
                "case `{name}`: its tolerance changed from {old:?} to {new:?}. A tolerance changes only with a \
                 reason: add `tolerance {name} {old:?} -> {new:?}: <why>` to docs/validation/acknowledged.txt \
                 and a line to the slice notes"
            )),
        }
    }
    if problems.is_empty() { Ok(owned) } else { Err(problems.join("\n")) }
}

/// Where the earlier report comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Against<'a> {
    /// `HEAD`, because nothing else was asked for. No report there is no baseline (the first
    /// report, or no git), and the run says so.
    Head,
    /// `--against <revision>`: the caller named the baseline, so it must be there.
    Revision(&'a str),
}

impl Against<'_> {
    const fn revision(&self) -> &str {
        match self {
            Self::Head => "HEAD",
            Self::Revision(revision) => revision,
        }
    }
}

/// What git said about the baseline: whether the revision names a commit, and the report
/// committed there, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub resolves: bool,
    pub report: Option<String>,
}

/// The cases to hold this report to, or `None` when there is no baseline.
///
/// # Errors
/// A baseline that was asked for by name and cannot be read: the revision does not resolve (a
/// mistyped tag must not switch the guard off), it holds no report, or its report lists no cases.
pub fn baseline(against: Against<'_>, found: &Found) -> Result<Option<Vec<Earlier>>, String> {
    let Against::Revision(revision) = against else {
        return Ok(found.report.as_deref().map(earlier_cases));
    };
    if !found.resolves {
        return Err(format!(
            "--against {revision}: git knows no such revision, so there is nothing to hold the report to. \
             Check the tag or commit (`git tag --list`); nothing was run"
        ));
    }
    let Some(report) = &found.report else {
        return Err(format!(
            "--against {revision}: there is no docs/validation/report.json at that revision; nothing was run"
        ));
    };
    let earlier = earlier_cases(report);
    if earlier.is_empty() {
        return Err(format!(
            "--against {revision}: the report at that revision lists no cases that can be read; nothing was run"
        ));
    }
    Ok(Some(earlier))
}

/// Asks git for the baseline: `docs/validation/report.json` as committed at the revision.
fn find(root: &Path, revision: &str) -> Found {
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git").current_dir(root).args(args).output().ok()?;
        output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    };
    Found {
        resolves: git(&["rev-parse", "--verify", "--quiet", &format!("{revision}^{{commit}}")]).is_some(),
        report: git(&["show", &format!("{revision}:docs/validation/report.json")]),
    }
}

/// Writes a report, given the acknowledged changes to print in it.
pub type Write<'a> = &'a mut dyn FnMut(&[String]) -> Result<(), String>;

/// The end of a run: the report is written only when there is nothing wrong with it. A failed or
/// missing case, a dropped case, a changed tolerance or a broken test run each stop it first, so
/// the committed `report.json`, which the next run is held to, is never replaced by a report that
/// did not pass.
///
/// # Errors
/// The first of: the verdict on the rows, the comparison with the earlier report, the test run,
/// the writing itself.
pub fn conclude(
    rows: &[Row],
    held: Result<Vec<String>, String>,
    ran: Result<(), String>,
    write: Option<Write<'_>>,
) -> Result<(), String> {
    let asked = write.is_some();
    let not_written = |why: String| {
        if asked { format!("{why}\nthe report was not written: docs/validation/ is as it was") } else { why }
    };
    verdict(rows).map_err(not_written)?;
    let owned = held.map_err(not_written)?;
    // Every case passed its tolerance, yet the test run failed: something else is wrong with it.
    ran.map_err(not_written)?;
    match write {
        Some(write) => write(&owned),
        None => Ok(()),
    }
}

/// Runs the cases, holds them to the report committed at `against`, and, with `report`, writes
/// the report if all of that held.
pub fn run(root: &Path, report: bool, against: Against<'_>) -> Result<(), String> {
    // First, so that a mistyped revision costs nothing and cannot be missed.
    let revision = against.revision();
    let earlier = baseline(against, &find(root, revision))?;
    let declared = scan(root)?;
    let records_dir = root.join("target").join("validation");
    let _ = std::fs::remove_dir_all(&records_dir);
    std::fs::create_dir_all(&records_dir).map_err(|e| format!("{}: {e}", records_dir.display()))?;
    let mut tests =
        cargo(root, &["nextest", "run", "--workspace", "--no-fail-fast", "--no-tests=pass", "-E", FILTER]);
    tests.env("CAV_VALIDATION_DIR", &records_dir);
    // A failing case is a row in the report, so a failed run is not the end: collate first.
    let ran = step("validation cases", tests);
    // The oracle crate's cases (v1 at v1-final, George's scripts) record into the same directory.
    let mut oracle =
        cargo(root, &[&crate::tiers::ORACLE_NEXTEST[..], &["--no-fail-fast", "-E", FILTER]].concat());
    oracle.env("CAV_VALIDATION_DIR", &records_dir);
    let ran = ran.and(step("validation cases of the oracle crate", oracle));

    let mut records = Vec::new();
    let entries = std::fs::read_dir(&records_dir).map_err(|e| format!("{}: {e}", records_dir.display()))?;
    for entry in entries.filter_map(Result::ok) {
        let text =
            std::fs::read_to_string(entry.path()).map_err(|e| format!("{}: {e}", entry.path().display()))?;
        records.push(parse_record(&text));
    }
    let rows = collate(&declared, &records)?;
    for row in &rows {
        eprintln!(
            "  {:<8} {:<34} {:>11} / {:<9} {:<9} {}",
            row.status.name(),
            row.name,
            shown(row.number("measured")),
            shown(row.number("tolerance")),
            row.text("unit"),
            row.text("why")
        );
    }
    eprintln!("xtask: validate: {}", summary(&rows));
    // The committed report is the memory of what was checked and how tightly.
    let acknowledged_path = root.join("docs").join("validation").join("acknowledged.txt");
    let acknowledged = parse_acknowledged(&std::fs::read_to_string(&acknowledged_path).unwrap_or_default())?;
    let held = match &earlier {
        Some(earlier) => {
            eprintln!("xtask: validate: held to the {} cases of the report at {revision}", earlier.len());
            changes(earlier, &rows, &acknowledged)
        }
        None => {
            eprintln!("xtask: validate: no report at {revision} to hold this one to");
            Ok(Vec::new())
        }
    };
    let mut write = |owned: &[String]| {
        let out = root.join("docs").join("validation");
        std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        std::fs::write(out.join("report.json"), report_json(&rows))
            .map_err(|e| format!("report.json: {e}"))?;
        std::fs::write(out.join("report.md"), report_md(&rows, owned))
            .map_err(|e| format!("report.md: {e}"))?;
        plots(root)?;
        eprintln!("xtask: validate: wrote {}", out.display());
        Ok(())
    };
    conclude(&rows, held, ran, if report { Some(&mut write) } else { None })
}

/// Draws the plots with the script the progress workflow also uses.
fn plots(root: &Path) -> Result<(), String> {
    let script = "scripts/validation_plots.py";
    let args = [script, "--report", "docs/validation/report.json", "--out", "docs/validation"];
    for python in ["python", "python3", "py"] {
        let mut command = std::process::Command::new(python);
        command.current_dir(root).args(args);
        match command.status() {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => return Err(format!("{script} failed ({status})")),
            Err(_) => {}
        }
    }
    Err(format!("no Python was found to run {script}; the plots were not drawn"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"
    // cav_validate::case! {
    //     name: "comment.not_a_case",
    cav_validate::case! {
        test: validation_first,
        name: "alpha.first",
        subsystem: "alpha",
        slice: "S0",
        reference: ClosedForm,
        against: "something",
        unit: Radians,
        tolerance: 1e-9,
        measure: Measured::Error(0.0),
    }

    case! {
        #[ignore]
        test: validation_gpu_second,
        name: "beta.second",
        subsystem: "beta",
        slice: "S1",
        reference: Exact,
        against: "itself",
        unit: Count,
        tolerance: 0.0,
        order: (2.0, 0.2),
        measure: {
            let name = "not.a_declaration";
            Measured::Error(0.0)
        },
    }
"#;

    fn record(name: &str, measured: &str, tolerance: &str, pass: bool) -> BTreeMap<String, String> {
        parse_record(&format!(
            "name\t{name}\nsubsystem\tx\nreference\tclosed form\nagainst\tsomething\nunit\tradians\n\
             tolerance\t{tolerance}\nmeasured\t{measured}\npass\t{pass}\n"
        ))
    }

    #[test]
    fn the_scan_finds_declarations_and_only_declarations() {
        let found = scan_source("crates/x/src/lib.rs", SOURCE);
        let names: Vec<&str> = found.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            ["alpha.first", "beta.second"],
            "the comment and the string inside `measure` are not cases"
        );
        assert_eq!(found[0].test, "validation_first");
        assert_eq!((found[1].subsystem.as_str(), found[1].slice.as_str()), ("beta", "S1"));
        assert!(found.iter().all(|c| c.has_tolerance));
        assert_eq!(found[0].at, "crates/x/src/lib.rs:4");
        // Control: a declaration with its tolerance taken out is seen to lack one.
        let without = SOURCE.replace("        tolerance: 1e-9,\n", "");
        assert!(!scan_source("f.rs", &without)[0].has_tolerance);
    }

    #[test]
    fn the_scan_reads_the_oracle_crate() {
        // The oracle crate is outside the workspace; its cases must still be rows of the report.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let declared = scan(root).unwrap();
        let under = |directory: &str| declared.iter().filter(|c| c.at.starts_with(directory)).count();
        assert!(under("crates/") >= 11, "the workspace has its cases: {}", under("crates/"));
        assert!(under("oracle/") >= 4, "the v1 tie is declared in the oracle crate: {}", under("oracle/"));
        assert!(
            declared
                .iter()
                .any(|c| c.name == "oracle.v1_oscillation" && c.at.starts_with("oracle/tests/v1.rs"))
        );
        // Control: every declaration is under one of the two roots, and under no other.
        assert_eq!(under("crates/") + under("oracle/"), declared.len());
    }

    #[test]
    fn a_declared_case_that_did_not_run_is_missing_and_fails_the_command() {
        let declared = scan_source("f.rs", SOURCE);
        // Only the first case recorded anything.
        let rows = collate(&declared, &[record("alpha.first", "1e-10", "1e-9", true)]).unwrap();
        assert_eq!(rows.len(), 2, "the case that did not run is still a row");
        let second = rows.iter().find(|r| r.name == "beta.second").unwrap();
        assert_eq!(second.status, Status::Missing);
        assert_eq!(counts(&rows), (2, 1, 0, 1));
        let failure = verdict(&rows).unwrap_err();
        assert!(failure.contains("declared but did not run (missing): beta.second"), "{failure}");
        assert!(report_md(&rows, &[]).contains("**MISSING**: declared, did not run"));
        assert!(report_json(&rows).contains("\"status\": \"missing\""));
        assert!(summary(&rows).contains("1 of 2 cases pass, 1 missing"));
        // Control: when both ran and passed, the same code is content.
        let both = [record("alpha.first", "1e-10", "1e-9", true), record("beta.second", "0.0", "0.0", true)];
        let rows = collate(&declared, &both).unwrap();
        assert_eq!(verdict(&rows), Ok(()));
        assert!(!report_md(&rows, &[]).contains("MISSING"));
    }

    #[test]
    fn a_failed_case_is_a_row_and_fails_the_command() {
        let declared = scan_source("f.rs", SOURCE);
        let records =
            [record("alpha.first", "2e-9", "1e-9", false), record("beta.second", "0.0", "0.0", true)];
        let rows = collate(&declared, &records).unwrap();
        assert_eq!(counts(&rows), (2, 1, 1, 0));
        assert!(verdict(&rows).unwrap_err().contains("1 case(s) failed: alpha.first"));
        assert!(report_md(&rows, &[]).contains("**FAIL**"));
        // No cases at all is not a pass either.
        assert!(verdict(&[]).is_err());
    }

    #[test]
    fn a_collection_that_cannot_be_trusted_is_an_error() {
        let declared = scan_source("f.rs", SOURCE);
        let ok = [record("alpha.first", "0.0", "1e-9", true), record("beta.second", "0.0", "0.0", true)];
        assert!(collate(&declared, &ok).is_ok(), "the control: these declarations and records are fine");
        // Registered twice.
        let mut twice = declared.clone();
        twice.push(declared[0].clone());
        assert!(collate(&twice, &ok).unwrap_err().contains("`alpha.first` is declared twice"));
        // No tolerance.
        let mut bare = declared.clone();
        bare[0].has_tolerance = false;
        assert!(collate(&bare, &ok).unwrap_err().contains("`alpha.first` has no tolerance"));
        // A test the filter would never run.
        let mut misnamed = declared.clone();
        misnamed[1].test = "second_case".into();
        assert!(collate(&misnamed, &ok).unwrap_err().contains("must be named `validation_…`"));
        // A record nobody declared: the scan missed a case.
        let mut extra = ok.to_vec();
        extra.push(record("gamma.third", "0.0", "1.0", true));
        assert!(
            collate(&declared, &extra).unwrap_err().contains("`gamma.third` is not among the declarations")
        );
    }

    #[test]
    fn rows_are_grouped_by_subsystem_with_the_worst_margin_first() {
        let source = SOURCE.replace("subsystem: \"beta\"", "subsystem: \"alpha\"");
        let declared = scan_source("f.rs", &source);
        let records =
            [record("alpha.first", "1e-10", "1e-9", true), record("beta.second", "9e-10", "1e-9", true)];
        let rows = collate(&declared, &records).unwrap();
        assert_eq!(rows[0].name, "beta.second", "90% of tolerance comes before 10%");
        assert!((rows[0].margin().unwrap() - 0.9).abs() < 1e-12);
        assert!(
            summary(&rows).contains("worst margin: beta.second at 90% of its tolerance"),
            "{}",
            summary(&rows)
        );
        // An exact case: zero is no margin used; anything else is over.
        assert_eq!(
            collate(&declared, &[record("alpha.first", "0.0", "0.0", true)]).unwrap()[1].margin(),
            Some(0.0)
        );
        let over = collate(&declared, &[record("alpha.first", "1e-300", "0.0", false)]).unwrap();
        assert_eq!(over.iter().find(|r| r.name == "alpha.first").unwrap().margin(), Some(f64::INFINITY));
    }

    #[test]
    fn the_json_is_well_formed_for_awkward_values() {
        assert_eq!(
            json_string("a \"quoted\" \\ path\tand a tab"),
            r#""a \"quoted\" \\ path\u0009and a tab""#
        );
        assert_eq!(json_number(Some(1e-12)), "1e-12");
        assert_eq!(json_number(Some(f64::INFINITY)), "null");
        assert_eq!(json_number(Some(f64::NAN)), "null");
        assert_eq!(json_number(None), "null");
        let declared = scan_source("f.rs", SOURCE);
        let mut second = record("beta.second", "2.5e-11", "1e-9", true);
        second.extend(parse_record(
            "order_expected\t2.0\norder_within\t0.2\norder_measured\t1.98\npoints\t0.1:1e-10;0.05:2.5e-11\n",
        ));
        let json =
            report_json(&collate(&declared, &[record("alpha.first", "0.0", "1e-9", true), second]).unwrap());
        assert!(
            json.contains("\"order\": {\"expected\": 2.0, \"within\": 0.2, \"measured\": 1.98}"),
            "{json}"
        );
        assert!(json.contains("\"points\": [[0.1, 1e-10], [0.05, 2.5e-11]]"), "{json}");
        assert!(
            json.contains("\"summary\": {\"total\": 2, \"passing\": 2, \"failing\": 0, \"missing\": 0,"),
            "{json}"
        );
        // Braces and brackets balance, and there is no trailing comma before a closing bracket.
        let balance = |open: char, close: char| json.matches(open).count() == json.matches(close).count();
        assert!(balance('{', '}') && balance('[', ']'));
        assert!(!json.contains(",\n  ]"));
    }

    #[test]
    fn a_dropped_case_or_a_changed_tolerance_fails_unless_it_is_acknowledged() {
        let declared = scan_source("f.rs", SOURCE);
        let records =
            [record("alpha.first", "1e-10", "1e-9", true), record("beta.second", "0.0", "0.0", true)];
        let rows = collate(&declared, &records).unwrap();
        // The earlier report, as this module writes it and reads it back.
        let earlier = earlier_cases(&report_json(&rows));
        assert_eq!(earlier, [("alpha.first".to_owned(), Some(1e-9)), ("beta.second".to_owned(), Some(0.0))]);
        // Control: the same report again is no change at all.
        assert_eq!(changes(&earlier, &rows, &[]), Ok(vec![]));

        // A case deleted from the source: "1 of 1 pass" would be green. It is not.
        let fewer = collate(&declared[..1], &records[..1]).unwrap();
        assert_eq!(verdict(&fewer), Ok(()), "the report alone cannot see what is no longer in it");
        let why = changes(&earlier, &fewer, &[]).unwrap_err();
        assert!(why.contains("case `beta.second` is in the earlier report and has gone"), "{why}");
        let owned =
            parse_acknowledged("# why\ndropped beta.second: the subsystem it checked was removed in S3\n")
                .unwrap();
        let notes = changes(&earlier, &fewer, &owned).unwrap();
        assert_eq!(notes, ["`beta.second` was dropped: the subsystem it checked was removed in S3"]);
        assert!(report_md(&fewer, &notes).contains("- `beta.second` was dropped: the subsystem"));

        // A tolerance loosened a hundredfold: every case still passes. It is not accepted.
        let loose = [record("alpha.first", "1e-10", "1e-7", true), records[1].clone()];
        let loosened = collate(&declared, &loose).unwrap();
        assert_eq!(verdict(&loosened), Ok(()));
        let why = changes(&earlier, &loosened, &[]).unwrap_err();
        assert!(why.contains("`alpha.first`: its tolerance changed from 1e-9 to 1e-7"), "{why}");
        // Tightening is a change too, and needs its line.
        let tight = [record("alpha.first", "1e-10", "1e-10", true), records[1].clone()];
        assert!(changes(&earlier, &collate(&declared, &tight).unwrap(), &[]).is_err());
        // An acknowledgement must name this case and these numbers.
        let wrong_numbers = parse_acknowledged("tolerance alpha.first 1e-9 -> 1e-6: because").unwrap();
        assert!(changes(&earlier, &loosened, &wrong_numbers).is_err());
        let wrong_case = parse_acknowledged("tolerance beta.second 1e-9 -> 1e-7: because").unwrap();
        assert!(changes(&earlier, &loosened, &wrong_case).is_err());
        let right =
            parse_acknowledged("tolerance alpha.first 1e-9 -> 1e-7: the reference is only known to 1e-7")
                .unwrap();
        let notes = changes(&earlier, &loosened, &right).unwrap();
        assert_eq!(notes, ["`alpha.first`: tolerance 1e-9 → 1e-7: the reference is only known to 1e-7"]);
        // A new case is not a change to an old one.
        assert_eq!(changes(&earlier[..1], &rows, &[]), Ok(vec![]));
    }

    #[test]
    fn a_named_baseline_that_cannot_be_read_is_an_error() {
        let declared = scan_source("f.rs", SOURCE);
        let records =
            [record("alpha.first", "1e-10", "1e-9", true), record("beta.second", "0.0", "0.0", true)];
        let json = report_json(&collate(&declared, &records).unwrap());
        let there = Found { resolves: true, report: Some(json.clone()) };
        let no_report = Found { resolves: true, report: None };
        let no_revision = Found { resolves: false, report: None };
        // Control: a named revision with a report is the baseline, the same one HEAD would give.
        let named = baseline(Against::Revision("s0-shell"), &there).unwrap().expect("a baseline");
        assert_eq!(named.len(), 2);
        assert_eq!(baseline(Against::Head, &there), Ok(Some(named)));
        // A mistyped tag: it used to mean "no report to hold this one to", and the run passed.
        let why = baseline(Against::Revision("s0-shel"), &no_revision).unwrap_err();
        assert!(why.contains("--against s0-shel") && why.contains("no such revision"), "{why}");
        // A real revision from before there was a report, or with one that cannot be read.
        let why = baseline(Against::Revision("v1-final"), &no_report).unwrap_err();
        assert!(
            why.contains("--against v1-final") && why.contains("no docs/validation/report.json"),
            "{why}"
        );
        let empty = Found { resolves: true, report: Some("{}\n".into()) };
        assert!(baseline(Against::Revision("abc123"), &empty).unwrap_err().contains("lists no cases"));
        // The default is not a named baseline: the first report ever has nothing to be held to.
        assert_eq!(baseline(Against::Head, &no_report), Ok(None));
        assert_eq!(baseline(Against::Head, &no_revision), Ok(None));
    }

    #[test]
    fn the_real_repository_resolves_head_and_not_a_made_up_tag() {
        // `find` against git itself: this crate lives in a repository.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        if !find(root, "HEAD").resolves {
            eprintln!("not a git checkout: nothing to ask");
            return;
        }
        let made_up = find(root, "no-such-tag-as-this");
        assert_eq!(made_up, Found { resolves: false, report: None });
        assert!(baseline(Against::Revision("no-such-tag-as-this"), &made_up).is_err());
    }

    #[test]
    fn a_report_that_did_not_pass_is_not_written() {
        let declared = scan_source("f.rs", SOURCE);
        let good = [record("alpha.first", "1e-10", "1e-9", true), record("beta.second", "0.0", "0.0", true)];
        let passing = collate(&declared, &good).unwrap();
        let failing = collate(
            &declared,
            &[record("alpha.first", "2e-9", "1e-9", false), record("beta.second", "0.0", "0.0", true)],
        )
        .unwrap();
        let missing = collate(&declared, &good[..1]).unwrap();
        // Runs `conclude` and says whether the report was written, and with which notes.
        let attempt = |rows: &[Row], held: Result<Vec<String>, String>, ran: Result<(), String>| {
            let mut written: Option<Vec<String>> = None;
            let mut write = |owned: &[String]| {
                written = Some(owned.to_vec());
                Ok(())
            };
            let outcome = conclude(rows, held, ran, Some(&mut write));
            (outcome, written)
        };
        // Control: a report with nothing wrong is written, with its acknowledged changes.
        let notes = vec!["`gamma.old` was dropped: gone with S3".to_owned()];
        assert_eq!(attempt(&passing, Ok(notes.clone()), Ok(())), (Ok(()), Some(notes)));
        // Each thing that fails the command stops the writing first.
        let dropped = || Err("case `gamma.old` is in the earlier report and has gone".to_owned());
        let cases = [
            (&failing, Ok(vec![]), Ok(()), "1 case(s) failed: alpha.first"),
            (&missing, Ok(vec![]), Ok(()), "did not run (missing): beta.second"),
            (&passing, dropped(), Ok(()), "has gone"),
            (&passing, Ok(vec![]), Err("validation cases failed (exit code: 100)".to_owned()), "exit code"),
        ];
        for (rows, held, ran, expected) in cases {
            let (outcome, written) = attempt(rows, held, ran);
            let why = outcome.unwrap_err();
            assert!(why.contains(expected), "{why}");
            assert!(why.contains("the report was not written"), "{why}");
            assert_eq!(written, None, "{expected}: the report must not be written");
        }
        // Without `--report` the verdict is the same and nothing is said about writing.
        assert_eq!(conclude(&passing, Ok(vec![]), Ok(()), None), Ok(()));
        let why = conclude(&failing, Ok(vec![]), Ok(()), None).unwrap_err();
        assert!(why.contains("alpha.first") && !why.contains("not written"), "{why}");
        // A failure of the writing itself is the command's failure.
        let mut broken = |_: &[String]| Err("report.json: disk full".to_owned());
        assert_eq!(
            conclude(&passing, Ok(vec![]), Ok(()), Some(&mut broken)),
            Err("report.json: disk full".into())
        );
    }

    #[test]
    fn acknowledgements_need_a_form_and_a_reason() {
        assert_eq!(parse_acknowledged("\n# nothing yet\n"), Ok(vec![]));
        assert!(parse_acknowledged("dropped a.b:").unwrap_err().contains("no reason"));
        assert!(parse_acknowledged("dropped a.b").unwrap_err().contains("neither"));
        assert!(parse_acknowledged("tolerance a.b 1 to 2: why").unwrap_err().contains("neither"));
        assert!(parse_acknowledged("tolerance a.b one -> 2: why").unwrap_err().contains("neither"));
        assert!(parse_acknowledged("loosened a.b: why").is_err());
    }
}
