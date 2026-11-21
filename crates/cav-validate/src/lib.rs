//! Validation cases: tests that compare the engine with something known, and say by how much.
//!
//! A case is declared next to the code it checks, with [`case!`]. The macro makes an ordinary test
//! (so `cargo xtask quick` or `gpu` runs it and it fails like any other), and the declaration is
//! plain enough for `cargo xtask validate` to find by reading the source. When that command runs
//! the tests it sets `CAV_VALIDATION_DIR`, and each case writes what it measured there. The
//! command then compares the two lists: a case that is declared but wrote nothing is reported as
//! **missing**, never skipped.
//!
//! ```text
//! cav_validate::case! {
//!     test: validation_example_sum,
//!     name: "example.sum_of_thirds",
//!     subsystem: "example",
//!     slice: "S0",
//!     reference: ClosedForm,
//!     against: "1/3 + 1/3 + 1/3 = 1",
//!     unit: Relative,
//!     tolerance: 1e-15,
//!     measure: cav_validate::Measured::Error((1.0f64 / 3.0 * 3.0 - 1.0).abs()),
//! }
//! ```

use std::fmt::Write as _;
use std::path::PathBuf;

/// The environment variable that names the directory records are written to.
pub const RECORD_DIR: &str = "CAV_VALIDATION_DIR";

/// What a case is checked against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reference {
    /// An analytic result.
    ClosedForm,
    /// The v1 forward model at tag `v1-final`.
    V1Oracle,
    /// One of George's reference scripts.
    Script,
    /// A number or a figure from a published paper.
    Paper,
    /// Recorded reference data kept in the repository.
    Recording,
    /// The engine against itself, where the two results must be identical.
    Exact,
}

/// The unit of a case's error and tolerance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Radians,
    /// A fraction of the reference value.
    Relative,
    Metres,
    Pixels,
    /// A number of things that differ: bytes, fields, commands.
    Count,
}

impl Reference {
    /// The words used in reports.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ClosedForm => "closed form",
            Self::V1Oracle => "v1 oracle",
            Self::Script => "George's script",
            Self::Paper => "published paper",
            Self::Recording => "reference recording",
            Self::Exact => "exact self-consistency",
        }
    }
}

impl Unit {
    /// The word used in reports.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Radians => "radians",
            Self::Relative => "relative",
            Self::Metres => "metres",
            Self::Pixels => "pixels",
            Self::Count => "count",
        }
    }
}

/// A validation case, as declared.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Case {
    /// Unique, lower case, `subsystem.what`: the row's name in the report.
    pub name: &'static str,
    /// The part of the engine it belongs to; the report groups by it.
    pub subsystem: &'static str,
    /// The slice that added it.
    pub slice: &'static str,
    pub reference: Reference,
    /// What exactly it is compared with, in a few words.
    pub against: &'static str,
    pub unit: Unit,
    /// The largest error that passes. Zero means the results must be identical.
    pub tolerance: f64,
    /// For a convergence case: the expected order, and how far the measured order may be from it.
    pub order: Option<(f64, f64)>,
}

/// What a case measured.
#[derive(Clone, Debug, PartialEq)]
pub enum Measured {
    /// One error, in the case's unit.
    Error(f64),
    /// Errors at a sequence of step sizes, `(step, error)`. The case's error is the one at the
    /// smallest step; its order is the fitted slope of log error against log step.
    Convergence(Vec<(f64, f64)>),
}

/// A case with what it measured and the verdict.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub error: f64,
    pub order: Option<f64>,
    pub points: Vec<(f64, f64)>,
    pub pass: bool,
    /// Why it failed, when it did.
    pub why: Option<String>,
}

/// The slope of the least-squares line through `(ln step, ln error)`: the measured order of
/// convergence. `None` with fewer than two usable points (positive and finite).
#[must_use]
pub fn fitted_order(points: &[(f64, f64)]) -> Option<f64> {
    let logs: Vec<(f64, f64)> = points
        .iter()
        .filter(|(h, e)| h.is_finite() && e.is_finite() && *h > 0.0 && *e > 0.0)
        .map(|(h, e)| (h.ln(), e.ln()))
        .collect();
    if logs.len() < 2 {
        return None;
    }
    let n = logs.len() as f64;
    let (mean_x, mean_y) =
        (logs.iter().map(|p| p.0).sum::<f64>() / n, logs.iter().map(|p| p.1).sum::<f64>() / n);
    let spread: f64 = logs.iter().map(|p| (p.0 - mean_x).powi(2)).sum();
    if spread == 0.0 {
        return None;
    }
    Some(logs.iter().map(|p| (p.0 - mean_x) * (p.1 - mean_y)).sum::<f64>() / spread)
}

/// Judges a measurement against its case. A tolerance that is negative or not a number, an error
/// that is not a number, and a convergence case without an expected order all fail: a case that
/// cannot fail is not a case.
#[must_use]
pub fn judge(case: &Case, measured: &Measured) -> Outcome {
    let (error, points) = match measured {
        Measured::Error(error) => (*error, Vec::new()),
        Measured::Convergence(points) => {
            let finest = points.iter().copied().min_by(|a, b| a.0.total_cmp(&b.0));
            (finest.map_or(f64::NAN, |p| p.1), points.clone())
        }
    };
    let order = if points.is_empty() { None } else { fitted_order(&points) };
    let mut why = None;
    if !(case.tolerance >= 0.0 && case.tolerance.is_finite()) {
        why = Some(format!("the tolerance {} is not a finite, non-negative number", case.tolerance));
    } else if error.is_nan() || error < 0.0 {
        why = Some(format!("the measured error {error} is not a non-negative number"));
    } else if error > case.tolerance {
        why =
            Some(format!("error {error:e} {} exceeds the tolerance {:e}", case.unit.name(), case.tolerance));
    } else if let Measured::Convergence(_) = measured {
        match (case.order, order) {
            (None, _) => why = Some("a convergence case must declare `order: (expected, within)`".into()),
            (Some(_), None) => why = Some("too few points to fit an order of convergence".into()),
            (Some((expected, within)), Some(found)) if (found - expected).abs() > within => {
                why = Some(format!("order {found:.3}, expected {expected} ± {within}"));
            }
            _ => {}
        }
    } else if case.order.is_some() {
        why = Some("the case declares an order but measured a single error".into());
    }
    Outcome { error, order, points, pass: why.is_none(), why }
}

/// The record a case writes for `cargo xtask validate`: one `key<TAB>value` per line.
#[must_use]
pub fn record(case: &Case, outcome: &Outcome) -> String {
    let mut text = String::new();
    let mut line = |key: &str, value: &str| {
        // Values are single lines; a tab or a line break inside one would break the format.
        let _ = writeln!(text, "{key}\t{}", value.replace(['\t', '\n', '\r'], " "));
    };
    line("name", case.name);
    line("subsystem", case.subsystem);
    line("slice", case.slice);
    line("reference", case.reference.name());
    line("against", case.against);
    line("unit", case.unit.name());
    line("tolerance", &format!("{:?}", case.tolerance));
    line("measured", &format!("{:?}", outcome.error));
    line("pass", if outcome.pass { "true" } else { "false" });
    if let Some((expected, within)) = case.order {
        line("order_expected", &format!("{expected:?}"));
        line("order_within", &format!("{within:?}"));
    }
    if let Some(order) = outcome.order {
        line("order_measured", &format!("{order:?}"));
    }
    if !outcome.points.is_empty() {
        let points: Vec<String> = outcome.points.iter().map(|(h, e)| format!("{h:?}:{e:?}")).collect();
        line("points", &points.join(";"));
    }
    if let Some(why) = &outcome.why {
        line("why", why);
    }
    text
}

/// Whether `name` is a well-formed case name: lower-case words joined by `.`, `_` or `-`.
#[must_use]
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.contains('.')
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

/// Judges the measurement, writes the record when `cargo xtask validate` asked for one, and fails
/// the test if the case did not pass. The record is written before the failure, so a failing case
/// is a row in the report and not a hole in it.
///
/// # Panics
/// When the case fails, when its name is malformed, or when a record with its name already exists
/// (two cases share a name, or one ran twice).
pub fn run(case: &Case, measured: &Measured) {
    assert!(valid_name(case.name), "`{}` is not a case name: use lower-case `subsystem.what`", case.name);
    let outcome = judge(case, measured);
    if let Some(dir) = std::env::var_os(RECORD_DIR) {
        let path = PathBuf::from(dir).join(format!("{}.case", case.name));
        // `create_new`: a second record under the same name is an error, not an overwrite.
        let file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path);
        let mut file = file.unwrap_or_else(|e| {
            panic!("case `{}`: cannot record to {}: {e} (is the name used twice?)", case.name, path.display())
        });
        std::io::Write::write_all(&mut file, record(case, &outcome).as_bytes()).expect("writing the record");
    }
    if let Some(why) = &outcome.why {
        panic!("validation case `{}` ({} · {}): {why}", case.name, case.reference.name(), case.against);
    }
}

/// Declares a validation case and the test that runs it. The fields come in this order, one per
/// line, each a literal (so the declaration can be read from the source without compiling it):
///
/// ```text
/// cav_validate::case! {
///     test: validation_<what>,          // `validation_gpu_<what>` if it needs the adapter
///     name: "subsystem.what",
///     subsystem: "subsystem",
///     slice: "S0",
///     reference: ClosedForm,            // or V1Oracle, Script, Paper, Recording, Exact
///     against: "what exactly it is compared with",
///     unit: Metres,                     // or Radians, Relative, Pixels, Count
///     tolerance: 1e-5,
///     order: (2.0, 0.2),                // convergence cases only: expected order, ± allowed
///     measure: <an expression of type cav_validate::Measured>,
/// }
/// ```
#[macro_export]
macro_rules! case {
    (
        $(#[$meta:meta])*
        test: $test:ident,
        name: $name:literal,
        subsystem: $subsystem:literal,
        slice: $slice:literal,
        reference: $reference:ident,
        against: $against:literal,
        unit: $unit:ident,
        tolerance: $tolerance:literal,
        $(order: ($expected:literal, $within:literal),)?
        measure: $measure:expr $(,)?
    ) => {
        $(#[$meta])*
        #[test]
        fn $test() {
            #[allow(unused_mut, unused_assignments)]
            let mut order: Option<(f64, f64)> = None;
            $(order = Some(($expected, $within));)?
            let case = $crate::Case {
                name: $name,
                subsystem: $subsystem,
                slice: $slice,
                reference: $crate::Reference::$reference,
                against: $against,
                unit: $crate::Unit::$unit,
                tolerance: $tolerance,
                order,
            };
            $crate::run(&case, &$measure);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    const CASE: Case = Case {
        name: "test.example",
        subsystem: "test",
        slice: "S0",
        reference: Reference::ClosedForm,
        against: "nothing",
        unit: Unit::Radians,
        tolerance: 1e-9,
        order: None,
    };

    #[test]
    fn a_case_passes_within_tolerance_and_fails_beyond_it() {
        assert!(judge(&CASE, &Measured::Error(1e-10)).pass);
        assert!(judge(&CASE, &Measured::Error(1e-9)).pass, "the tolerance itself passes");
        assert!(judge(&CASE, &Measured::Error(0.0)).pass);
        // The control arms: what must fail does.
        let over = judge(&CASE, &Measured::Error(2e-9));
        assert!(!over.pass && over.why.unwrap().contains("exceeds the tolerance"));
        assert!(!judge(&CASE, &Measured::Error(f64::NAN)).pass, "an error that is not a number fails");
        assert!(!judge(&CASE, &Measured::Error(-1.0)).pass);
        assert!(!judge(&CASE, &Measured::Error(f64::INFINITY)).pass);
        // An exact case passes only at zero.
        let exact = Case { tolerance: 0.0, ..CASE };
        assert!(judge(&exact, &Measured::Error(0.0)).pass);
        assert!(!judge(&exact, &Measured::Error(5e-324)).pass);
    }

    #[test]
    fn a_case_without_a_usable_tolerance_fails() {
        for tolerance in [f64::NAN, f64::INFINITY, -1.0] {
            let case = Case { tolerance, ..CASE };
            let outcome = judge(&case, &Measured::Error(0.0));
            assert!(!outcome.pass, "a tolerance of {tolerance} cannot fail, so it must not pass");
            assert!(outcome.why.unwrap().contains("tolerance"));
        }
    }

    #[test]
    fn convergence_is_judged_by_its_order_as_well_as_its_error() {
        // Error = 3 h²: second order.
        let second: Vec<(f64, f64)> = [0.1, 0.05, 0.025, 0.0125].iter().map(|&h| (h, 3.0 * h * h)).collect();
        assert!((fitted_order(&second).unwrap() - 2.0).abs() < 1e-12);
        let case = Case { tolerance: 1e-3, order: Some((2.0, 0.1)), ..CASE };
        let outcome = judge(&case, &Measured::Convergence(second.clone()));
        assert!(outcome.pass, "{:?}", outcome.why);
        assert_eq!(outcome.error, 3.0 * 0.0125 * 0.0125, "the error is the one at the finest step");
        // Control arms: first-order data fails a second-order case even though its error is small.
        let first: Vec<(f64, f64)> = [0.1, 0.05, 0.025, 0.0125].iter().map(|&h| (h, 1e-4 * h)).collect();
        assert!((fitted_order(&first).unwrap() - 1.0).abs() < 1e-12);
        let wrong = judge(&case, &Measured::Convergence(first));
        assert!(!wrong.pass && wrong.why.unwrap().contains("order 1.000, expected 2"));
        // A convergence measurement needs a declared order, and a declared order needs points.
        assert!(!judge(&CASE, &Measured::Convergence(second)).pass);
        assert!(!judge(&case, &Measured::Error(0.0)).pass);
        assert!(!judge(&case, &Measured::Convergence(vec![(0.1, 1e-4)])).pass, "one point has no slope");
        assert_eq!(fitted_order(&[(0.1, 0.0), (0.05, 0.0)]), None, "zero errors have no logarithm");
    }

    #[test]
    fn records_carry_every_field_on_its_own_line() {
        let case = Case { order: Some((2.0, 0.2)), against: "a\ttab and\na break", ..CASE };
        let outcome = judge(&case, &Measured::Convergence(vec![(0.1, 1e-10), (0.05, 2.5e-11)]));
        let text = record(&case, &outcome);
        let field =
            |key: &str| text.lines().find_map(|l| l.strip_prefix(&format!("{key}\t"))).map(str::to_owned);
        assert_eq!(field("name").as_deref(), Some("test.example"));
        assert_eq!(field("reference").as_deref(), Some("closed form"));
        assert_eq!(field("unit").as_deref(), Some("radians"));
        assert_eq!(field("tolerance").as_deref(), Some("1e-9"));
        assert_eq!(field("measured").as_deref(), Some("2.5e-11"));
        assert_eq!(field("pass").as_deref(), Some("true"));
        assert_eq!(field("against").as_deref(), Some("a tab and a break"), "one line per field");
        assert_eq!(field("points").as_deref(), Some("0.1:1e-10;0.05:2.5e-11"));
        assert!((field("order_measured").unwrap().parse::<f64>().unwrap() - 2.0).abs() < 1e-9);
        assert_eq!(field("why"), None);
        // Numbers are written so that they read back exactly.
        assert_eq!(field("measured").unwrap().parse::<f64>().unwrap().to_bits(), 2.5e-11f64.to_bits());
        // A failing case records its reason.
        let failed = judge(&CASE, &Measured::Error(1.0));
        assert!(record(&CASE, &failed).contains("pass\tfalse\nwhy\t"));
    }

    #[test]
    fn names_are_checked() {
        assert!(valid_name("render.determinism") && valid_name("spec.ron_round-trip2"));
        for bad in ["", "render", "Render.x", "render determinism", "render.x\ty"] {
            assert!(!valid_name(bad), "`{bad}`");
        }
    }

    #[test]
    #[should_panic(expected = "exceeds the tolerance")]
    fn a_failing_case_fails_its_test() {
        run(&CASE, &Measured::Error(1.0));
    }

    // The macro makes a test that runs like any other: this one is a case of its own.
    case! {
        test: validation_fitted_order_of_a_quadratic,
        name: "validate.fitted_order",
        subsystem: "validate",
        slice: "S0",
        reference: ClosedForm,
        against: "the slope of log(3h²) against log h is 2",
        unit: Relative,
        tolerance: 1e-12,
        measure: Measured::Error((fitted_order(&[(0.1, 0.03), (0.01, 0.0003), (0.001, 0.000003)]).unwrap() / 2.0 - 1.0).abs()),
    }
}
