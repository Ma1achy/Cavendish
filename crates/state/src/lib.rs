//! `state` — the `StateBundle` output contract (minimal M1 subset).
//!
//! Design: `design/state.md`. Milestone: `milestones/M1-physics-spine.md`.
//!
//! M1 populates only `time`, `signal`, `source_position`, `mask`, `meta`; the full 27-field bundle
//! (shape descriptors, channel decomposition, spectra) is filled in later milestones. The `math`
//! surface is re-exported so it stays reachable up the layers.

pub use math::{Dual, Isometry3, Mat3, Quat, Scalar, Vec3};

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

/// A source body-frame point cloud: `[x, y, z, m]` per element, shape `(N, 4)`. The static geometry the
/// viewer poses by `source_position`/`source_orientation` at render time.
pub type SourceCloud = Vec<[f64; 4]>;

/// Run metadata: the seed and a resolved-config summary.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub seed: u64,
    pub description: String,
}

/// The forward model's output. Leading axis `T` is the measurement cadence.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StateBundle {
    /// Measurement timestamps, shape `(T,)`.
    pub time: Vec<f64>,
    /// Gradiometer phase per measurement, per detector, shape `(T, D)`.
    pub signal: Vec<Vec<f64>>,
    /// Source COM world position, shape `(S, T, 3)`.
    pub source_position: Vec<Vec<[f64; 3]>>,
    /// Source COM linear velocity, shape `(S, T, 3)`.
    pub source_velocity: Vec<Vec<[f64; 3]>>,
    /// Source COM linear acceleration, shape `(S, T, 3)`.
    pub source_accel: Vec<Vec<[f64; 3]>>,
    /// Source orientation quaternion wxyz (world ← body), shape `(S, T, 4)`.
    pub source_orientation: Vec<Vec<[f64; 4]>>,
    /// Source angular velocity ω (direction = spin axis, magnitude = rate), shape `(S, T, 3)`.
    pub source_angular_velocity: Vec<Vec<[f64; 3]>>,
    /// Source angular acceleration, shape `(S, T, 3)`.
    pub source_angular_accel: Vec<Vec<[f64; 3]>>,
    /// Source body-frame geometry `[x, y, z, m]` per element, shape `(S, N, 4)` — posed at render time.
    pub source_cloud: Vec<SourceCloud>,
    /// Per-detector placement: position xyz + orientation quaternion (wxyz), shape `(D, 7)`.
    pub detector_placement: Vec<[f64; 7]>,
    /// Transient-contaminated cycles, shape `(T,)`.
    pub mask: Vec<bool>,
    /// Resolved config and seed.
    pub meta: Meta,

    // Shape descriptors — optional, computed from the body cloud iff `FieldSet.shape` is on.
    /// Total mass `M`, shape `(S,)`.
    pub source_mass: Option<Vec<f64>>,
    /// Inertia tensor `I` (body frame), shape `(S, 3, 3)`.
    pub source_inertia: Option<Vec<[[f64; 3]; 3]>>,
    /// Principal moments `(I₁, I₂, I₃)`, shape `(S, 3)`.
    pub source_moments: Option<Vec<[f64; 3]>>,
    /// Principal axes (body frame; shared by `I` and `Q`), shape `(S, 3, 3)`.
    pub source_axes: Option<Vec<[[f64; 3]; 3]>>,
    /// Gravitational quadrupole `Q` (body frame), shape `(S, 3, 3)`.
    pub source_quadrupole: Option<Vec<[[f64; 3]; 3]>>,

    // Channel decomposition (spec `sec:contracts`) — `signal` = targets + atmospheric + uldm + noise.
    /// The post-hoc noise realisation, shape `(T, D)` — always recorded (`signal − signal_noise` = clean).
    pub signal_noise: Vec<Vec<f64>>,
    /// ULDM common-mode phase, shape `(T,)` — optional; identical across detectors by construction.
    pub signal_uldm: Option<Vec<f64>>,
    /// Targets-only gravitational channel, shape `(T, D)` — optional (gated by `FieldSet.decomposition`).
    pub signal_targets: Option<Vec<Vec<f64>>>,
    /// Atmospheric-only gravitational channel, shape `(T, D)` — optional (gated by decomposition).
    pub signal_atmospheric: Option<Vec<Vec<f64>>>,
    /// The two single-interferometer phases before the double difference, shape `(T, D, 2)` — optional.
    pub signal_per_ifo: Option<Vec<Vec<[f64; 2]>>>,

    /// The Lomb–Scargle periodogram per detector — optional (gated by `FieldSet.periodogram`).
    pub periodogram: Option<Periodogram>,
}

/// A Lomb–Scargle periodogram: `power[d][k]` is the spectral power of detector `d`'s signal at
/// `freqs[k]`. The correct estimator for non-uniform (gappy/jittered) sampling, where the FFT is not.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Periodogram {
    /// The frequency grid `(F,)` [Hz].
    pub freqs: Vec<f64>,
    /// Power per detector, shape `(D, F)`.
    pub power: Vec<Vec<f64>>,
}

/// Serialise a bundle to a compact binary sink (bincode) — the read-back path the viewer loads.
pub fn save_bundle(bundle: &StateBundle, path: impl AsRef<Path>) -> io::Result<()> {
    let bytes = bincode::serialize(bundle).map_err(io::Error::other)?;
    std::fs::write(path, bytes)
}

/// Read a bundle serialised by [`save_bundle`]. Round-trips the whole contract byte-for-byte.
pub fn load_bundle(path: impl AsRef<Path>) -> io::Result<StateBundle> {
    let bytes = std::fs::read(path)?;
    bincode::deserialize(&bytes).map_err(io::Error::other)
}

/// The Lomb–Scargle periodogram of an unevenly-sampled series `y(t)` on a frequency grid (spec §2.1).
/// Mean-subtracts `y`; for each `ω = 2πf` uses the time offset `τ` that diagonalises the fit
/// (`tan 2ωτ = Σsin2ωt / Σcos2ωt`) so `P(ω) = ½[C²/Σc² + S²/Σs²]` is phase-invariant. Pure `f64`.
pub fn lomb_scargle(t: &[f64], y: &[f64], freqs: &[f64]) -> Vec<f64> {
    let n = y.len() as f64;
    let mean = if n > 0.0 {
        y.iter().sum::<f64>() / n
    } else {
        0.0
    };
    freqs
        .iter()
        .map(|&f| {
            let w = std::f64::consts::TAU * f;
            let two_w = 2.0 * w;
            let (mut ss2, mut sc2) = (0.0, 0.0);
            for &ti in t {
                ss2 += (two_w * ti).sin();
                sc2 += (two_w * ti).cos();
            }
            let tau = 0.5 * ss2.atan2(sc2) / w;
            let (mut c, mut s, mut cc, mut sss) = (0.0, 0.0, 0.0, 0.0);
            for (&ti, &yi) in t.iter().zip(y) {
                let arg = w * (ti - tau);
                let (co, si) = (arg.cos(), arg.sin());
                let dy = yi - mean;
                c += dy * co;
                s += dy * si;
                cc += co * co;
                sss += si * si;
            }
            let cterm = if cc > 1e-300 { c * c / cc } else { 0.0 };
            let sterm = if sss > 1e-300 { s * s / sss } else { 0.0 };
            0.5 * (cterm + sterm)
        })
        .collect()
}

/// The standard LS frequency grid for sorted `times`: `f ∈ [1/T_span, f_Ny,eff]` oversampled ×4,
/// where the effective Nyquist `f_Ny,eff = 0.5 / median(Δt)` stands in for the Nyquist limit on
/// non-uniform sampling. Returns an empty grid for fewer than two samples.
pub fn frequency_grid(times: &[f64]) -> Vec<f64> {
    if times.len() < 2 {
        return Vec::new();
    }
    let t_span = times[times.len() - 1] - times[0];
    if t_span <= 0.0 {
        return Vec::new();
    }
    let mut dt: Vec<f64> = times.windows(2).map(|w| w[1] - w[0]).collect();
    dt.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = dt[dt.len() / 2];
    let f_min = 1.0 / t_span;
    let f_ny = 0.5 / median;
    let df = 1.0 / (4.0 * t_span); // ×4 oversampling
    let mut freqs = Vec::new();
    let mut f = f_min;
    while f <= f_ny {
        freqs.push(f);
        f += df;
    }
    freqs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_roundtrip() {
        // A bundle with populated geometry, optional periodogram, and the source cloud survives a
        // save/load byte-for-byte — the loaded path renders from a real file, not an in-memory clone.
        let bundle = StateBundle {
            time: vec![1.0, 2.5, 4.0],
            signal: vec![vec![0.1, -0.2], vec![0.3, 0.4], vec![-0.5, 0.6]],
            source_position: vec![vec![[1.0, 0.0, 2.0], [1.1, 0.0, 2.0], [1.2, 0.0, 2.0]]],
            source_orientation: vec![vec![[1.0, 0.0, 0.0, 0.0]; 3]],
            source_cloud: vec![vec![[0.1, 0.2, 0.3, 500.0], [-0.1, -0.2, -0.3, 500.0]]],
            detector_placement: vec![[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]],
            mask: vec![false, true, false],
            meta: Meta {
                seed: 7,
                description: "roundtrip".into(),
            },
            source_mass: Some(vec![1000.0]),
            periodogram: Some(Periodogram {
                freqs: vec![0.1, 0.2],
                power: vec![vec![3.0, 1.0]],
            }),
            ..Default::default()
        };
        let path = std::env::temp_dir().join("cavendish_bundle_roundtrip.bin");
        save_bundle(&bundle, &path).expect("save");
        let back = load_bundle(&path).expect("load");
        std::fs::remove_file(&path).ok();
        assert_eq!(bundle, back, "bundle did not round-trip through bincode");
    }

    #[test]
    fn ls_analytic() {
        // A pure sinusoid on uniform sampling: the periodogram peaks at the planted line, and the
        // peak's height dominates the off-peak floor.
        let f0 = 0.1;
        let times: Vec<f64> = (0..512).map(|i| i as f64 * 0.5).collect(); // 256 s, dt = 0.5
        let y: Vec<f64> = times
            .iter()
            .map(|&t| (std::f64::consts::TAU * f0 * t).cos())
            .collect();
        let freqs = frequency_grid(&times);
        let p = lomb_scargle(&times, &y, &freqs);

        let (peak_k, &peak) = p
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap();
        assert!(
            (freqs[peak_k] - f0).abs() <= freqs[1] - freqs[0],
            "peak at {} Hz, planted {f0}",
            freqs[peak_k]
        );
        // The peak carries essentially all the power; the median bin is negligible.
        let mut sorted = p.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = sorted[sorted.len() / 2];
        assert!(
            peak > 1e4 * median.max(1e-30),
            "peak {peak:e} vs median {median:e}"
        );

        // White-ish (a linear ramp is broadband): no single bin dominates the way a line does.
        let flat: Vec<f64> = times.clone();
        let pf = lomb_scargle(&times, &flat, &freqs);
        let peak_f = pf.iter().cloned().fold(0.0_f64, f64::max);
        let sum_f: f64 = pf.iter().sum();
        assert!(
            peak_f < 0.9 * sum_f,
            "a ramp should not concentrate in one bin"
        );
    }
}
