//! `config` — the typed configuration schema and distribution primitives.
//!
//! Design: `DESIGN.md`. Milestone: `milestones/M6-compute-gpu-and-batch.md`.
//!
//! A dependency-free schema crate: `FieldSet` (the per-run cost knob), the `Dist` distribution
//! primitives a `Prior` samples scenario parameters from, and `RunConfig` (batch/stream knobs).
//! `Dist::sample` takes a draw closure so `config` stays a leaf — the counter RNG that feeds it lives
//! with the `Prior` (in `scenario`), keyed through the RNG key tree.

/// Which optional bundle field groups to compute — a cost knob, not a task (spec `FieldSet`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FieldSet {
    /// Fill the static shape descriptors (mass, inertia, moments, axes, quadrupole).
    pub shape: bool,
    /// Decompose `signal` into its channels (targets, atmospheric, uldm, per-IFO) — the ≈2× cost of a
    /// second gravitational pass. Off ⇒ one combined pass, channel fields `None`.
    pub decomposition: bool,
    /// Compute the Lomb–Scargle periodogram per detector as a derived bundle field.
    pub periodogram: bool,
}

/// Batch and stream execution knobs — not physics. Kept small; grows as `stream` consumers arrive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunConfig {
    /// Scenarios evaluated per batch in `generate::stream`.
    pub batch: usize,
}

impl Default for RunConfig {
    fn default() -> Self {
        RunConfig { batch: 64 }
    }
}

/// A validation failure on a `Dist` (checked once, at construction, so `sample` is total).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// A range with `lo > hi`.
    InvertedRange,
    /// A `LogUniform` bound `≤ 0` (the logarithm is undefined).
    NonPositiveLogBound,
    /// A `Normal` with `sigma < 0`.
    NegativeSigma,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::InvertedRange => write!(f, "distribution range has lo > hi"),
            ConfigError::NonPositiveLogBound => write!(f, "log-uniform bound must be > 0"),
            ConfigError::NegativeSigma => write!(f, "normal sigma must be >= 0"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// A distribution primitive a `Prior` samples one scenario parameter from. Validate once at
/// construction (`validate`); `sample` is then total.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dist {
    /// A fixed value (draws nothing).
    Const(f64),
    /// Uniform on `[lo, hi)`.
    Uniform { lo: f64, hi: f64 },
    /// Log-uniform on `[lo, hi)` (`0 < lo ≤ hi`) — for scale parameters spanning decades.
    LogUniform { lo: f64, hi: f64 },
    /// Gaussian `N(mean, sigma²)` (`sigma ≥ 0`).
    Normal { mean: f64, sigma: f64 },
}

impl Dist {
    /// Reject inverted ranges, non-positive log bounds, and negative sigma. Total thereafter.
    pub fn validate(&self) -> Result<(), ConfigError> {
        match *self {
            Dist::Const(_) => Ok(()),
            Dist::Uniform { lo, hi } => {
                if lo <= hi {
                    Ok(())
                } else {
                    Err(ConfigError::InvertedRange)
                }
            }
            Dist::LogUniform { lo, hi } => {
                if lo <= 0.0 || hi <= 0.0 {
                    Err(ConfigError::NonPositiveLogBound)
                } else if lo <= hi {
                    Ok(())
                } else {
                    Err(ConfigError::InvertedRange)
                }
            }
            Dist::Normal { sigma, .. } => {
                if sigma >= 0.0 {
                    Ok(())
                } else {
                    Err(ConfigError::NegativeSigma)
                }
            }
        }
    }

    /// Map uniform draws from `draw` (each in `[0, 1)`) to a value. `Normal` consumes two draws; the
    /// others consume one (`Const` none). Kept dependency-free — the caller supplies the RNG.
    pub fn sample(&self, draw: &mut dyn FnMut() -> f64) -> f64 {
        match *self {
            Dist::Const(c) => c,
            Dist::Uniform { lo, hi } => lo + (hi - lo) * draw(),
            Dist::LogUniform { lo, hi } => (lo.ln() + (hi.ln() - lo.ln()) * draw()).exp(),
            Dist::Normal { mean, sigma } => {
                let u1 = draw().max(1e-300);
                let u2 = draw();
                mean + sigma * (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dist_validation() {
        assert!(Dist::Uniform { lo: 1.0, hi: 2.0 }.validate().is_ok());
        assert_eq!(
            Dist::Uniform { lo: 2.0, hi: 1.0 }.validate(),
            Err(ConfigError::InvertedRange)
        );
        assert_eq!(
            Dist::LogUniform { lo: 0.0, hi: 1.0 }.validate(),
            Err(ConfigError::NonPositiveLogBound)
        );
        assert_eq!(
            Dist::Normal {
                mean: 0.0,
                sigma: -1.0
            }
            .validate(),
            Err(ConfigError::NegativeSigma)
        );
        assert!(Dist::Const(3.0).validate().is_ok());
    }

    #[test]
    fn dist_sample_deterministic_and_in_range() {
        // Same draws → same value (the mapping is pure); Uniform stays within [lo, hi).
        let d = Dist::Uniform { lo: 5.0, hi: 9.0 };
        let mut seq = [0.25_f64, 0.75].into_iter().cycle();
        let a = d.sample(&mut || seq.next().unwrap());
        let mut seq2 = [0.25_f64, 0.75].into_iter().cycle();
        let b = d.sample(&mut || seq2.next().unwrap());
        assert_eq!(a, b);
        assert_eq!(a, 6.0);
        // LogUniform endpoints.
        let lg = Dist::LogUniform { lo: 1.0, hi: 100.0 };
        assert!((lg.sample(&mut || 0.0) - 1.0).abs() < 1e-12);
        assert!((lg.sample(&mut || 0.5) - 10.0).abs() < 1e-9);
    }
}
