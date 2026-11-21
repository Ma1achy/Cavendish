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

/// Every case declared under `crates/`.
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
    walk(&root.join("crates"), root, &mut out)?;
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
pub fn report_md(rows: &[Row]) -> String {
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

/// Runs the cases and, with `report`, writes the report.
pub fn run(root: &Path, report: bool) -> Result<(), String> {
    let declared = scan(root)?;
    let records_dir = root.join("target").join("validation");
    let _ = std::fs::remove_dir_all(&records_dir);
    std::fs::create_dir_all(&records_dir).map_err(|e| format!("{}: {e}", records_dir.display()))?;
    let mut tests =
        cargo(root, &["nextest", "run", "--workspace", "--no-fail-fast", "--no-tests=pass", "-E", FILTER]);
    tests.env("CAV_VALIDATION_DIR", &records_dir);
    // A failing case is a row in the report, so a failed run is not the end: collate first.
    let ran = step("validation cases", tests);

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
    if report {
        let out = root.join("docs").join("validation");
        std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        std::fs::write(out.join("report.json"), report_json(&rows))
            .map_err(|e| format!("report.json: {e}"))?;
        std::fs::write(out.join("report.md"), report_md(&rows)).map_err(|e| format!("report.md: {e}"))?;
        plots(root)?;
        eprintln!("xtask: validate: wrote {}", out.display());
    }
    verdict(&rows)?;
    // Every case passed its tolerance, yet the test run failed: something else is wrong with it.
    ran
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
        assert!(report_md(&rows).contains("**MISSING**: declared, did not run"));
        assert!(report_json(&rows).contains("\"status\": \"missing\""));
        assert!(summary(&rows).contains("1 of 2 cases pass, 1 missing"));
        // Control: when both ran and passed, the same code is content.
        let both = [record("alpha.first", "1e-10", "1e-9", true), record("beta.second", "0.0", "0.0", true)];
        let rows = collate(&declared, &both).unwrap();
        assert_eq!(verdict(&rows), Ok(()));
        assert!(!report_md(&rows).contains("MISSING"));
    }

    #[test]
    fn a_failed_case_is_a_row_and_fails_the_command() {
        let declared = scan_source("f.rs", SOURCE);
        let records =
            [record("alpha.first", "2e-9", "1e-9", false), record("beta.second", "0.0", "0.0", true)];
        let rows = collate(&declared, &records).unwrap();
        assert_eq!(counts(&rows), (2, 1, 1, 0));
        assert!(verdict(&rows).unwrap_err().contains("1 case(s) failed: alpha.first"));
        assert!(report_md(&rows).contains("**FAIL**"));
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
}
