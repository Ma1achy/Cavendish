//! The oracle tier's support: where the fixtures are, and how two series are compared.
//!
//! The tests are in `tests/`: `v1.rs` holds the engine to v1's forward model at tag `v1-final`,
//! compiled from source; `scripts.rs` holds it to arrays made by running George's reference
//! scripts (`fixtures/scripts/PROVENANCE.md`); `polyhedral.rs` holds the polyhedral closed form to
//! the paper's 40-digit reference and `benchmark.rs` reads the paper's benchmark bodies
//! (`fixtures/paper/PROVENANCE.md`, `docs/papers/polyhedral-gravity`).

use std::path::PathBuf;

/// The directory of fixtures, `oracle/fixtures`.
#[must_use]
pub fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// A one-dimensional fixture, by its path under `oracle/fixtures`.
///
/// # Panics
/// When the fixture is missing or is not a float64 `.npy`: an oracle without its reference has
/// checked nothing, and must not pass.
#[must_use]
pub fn fixture(relative: &str) -> Vec<f64> {
    cav_core::npy::read(&fixtures().join(relative)).unwrap_or_else(|why| panic!("fixture {relative}: {why}"))
}

/// The largest difference between two series, as a fraction of the largest value of the second.
///
/// # Panics
/// When the series differ in length or the reference is all zeros.
#[must_use]
pub fn worst_of_peak(ours: &[f64], reference: &[f64]) -> f64 {
    assert_eq!(ours.len(), reference.len(), "the series differ in length");
    let peak = reference.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    assert!(peak > 0.0, "the reference series is all zeros");
    ours.iter().zip(reference).fold(0.0f64, |m, (a, b)| m.max((a - b).abs())) / peak
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_measure_is_the_largest_difference_over_the_peak() {
        assert_eq!(worst_of_peak(&[1.0, -4.0, 2.0], &[1.0, -4.0, 2.0]), 0.0);
        assert_eq!(worst_of_peak(&[1.0, -4.0, 2.5], &[1.0, -4.0, 2.0]), 0.125);
        // Control: a sign error is as large as it gets.
        assert_eq!(worst_of_peak(&[-1.0, 4.0, -2.0], &[1.0, -4.0, 2.0]), 2.0);
    }

    #[test]
    fn a_missing_fixture_is_a_failure_not_a_skip() {
        let provenance = fixtures().join("scripts").join("PROVENANCE.md");
        assert!(provenance.is_file(), "the fixtures are where they are said to be");
        let missing = std::panic::catch_unwind(|| fixture("scripts/no-such-case/t.npy"));
        assert!(missing.is_err());
        // Control: one that is there is read.
        assert_eq!(fixture("scripts/1d_oscillator/t.npy").len(), 1000);
    }
}
