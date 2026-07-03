//! `uldm` — the ULDM common-mode phase channel.
//!
//! Design: `design/uldm.md`. Milestone: `milestones/M5-channels-and-decomposition.md`.
//!
//! A coherent scalar dark-matter field oscillating at `f_φ = m_φc²/h ≈ 0.1 Hz` modulates the atomic
//! transition frequency, giving a closed-form per-measurement phase `δφ_ULDM(t) = A_φ cos(2πf_φ t + φ₀)`.
//! It is **common-mode by construction** — coherent over astronomical scales, so every detector sees
//! the same phase — hence computed once per measurement and broadcast, and rejected by the array
//! against the geometrically-varying target channel (spec `eq:uldm`, `tab:uldm`, `INV.4`).

use math::Scalar;

const TAU: f64 = std::f64::consts::TAU;

/// Parameters of the coherent ULDM phase (spec `tab:uldm`).
///
/// M5 carries the resolved `(A_φ, f_φ, φ₀)`; the full `eq:uldm` derivation of `A_φ` from the dilatonic
/// couplings and local DM density (which also needs the instrument geometry) is a later refinement.
#[derive(Clone, Copy, Debug)]
pub struct UldmConfig {
    /// Phase amplitude `A_φ` [rad].
    pub amplitude: f64,
    /// Oscillation frequency `f_φ = m_φc²/h` [Hz].
    pub frequency: f64,
    /// Initial phase `φ₀` [rad].
    pub phase: f64,
}

impl Default for UldmConfig {
    fn default() -> Self {
        UldmConfig {
            amplitude: 1e-3,
            frequency: 0.1,
            phase: core::f64::consts::FRAC_PI_2, // spec tab:uldm θ = π/2
        }
    }
}

/// The closed-form ULDM phase `δφ_ULDM(t) = A_φ cos(2πf_φ t + φ₀)`.
///
/// No detector dependence — the common-mode channel. Scalar-generic so a `Dual` tangent flows for the
/// DM-detection CRB (M8).
pub fn uldm_phase<S: Scalar>(cfg: &UldmConfig, t: S) -> S {
    let arg = t * S::from_f64(TAU * cfg.frequency) + S::from_f64(cfg.phase);
    S::from_f64(cfg.amplitude) * arg.cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uldm_common_mode() {
        // Recover amplitude A_φ (the peak) and frequency f_φ (periodicity) to machine precision; the
        // channel has no detector argument, so it is identical across the array by construction.
        let cfg = UldmConfig {
            amplitude: 2.5e-3,
            frequency: 0.1,
            phase: 0.3,
        };
        // Peak value = A_φ at the time where the cosine argument is zero.
        let t_peak = -cfg.phase / (TAU * cfg.frequency);
        assert!(
            (uldm_phase(&cfg, t_peak) - cfg.amplitude).abs() <= 1e-12,
            "amplitude"
        );
        // Periodic with 1/f_φ.
        for &t in &[0.0, 1.7, 3.3] {
            let a = uldm_phase(&cfg, t);
            let b = uldm_phase(&cfg, t + 1.0 / cfg.frequency);
            assert!((a - b).abs() <= 1e-12, "frequency/period at t={t}");
        }
        // The closed form, checked against a direct evaluation.
        let t = 0.7;
        let want = cfg.amplitude * (TAU * cfg.frequency * t + cfg.phase).cos();
        assert!((uldm_phase(&cfg, t) - want).abs() <= 1e-12, "closed form");
    }
}
