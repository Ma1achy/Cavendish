//! `state` — the `StateBundle` output contract (minimal M1 subset).
//!
//! Design: `design/state.md`. Milestone: `milestones/M1-physics-spine.md`.
//!
//! M1 populates only `time`, `signal`, `source_position`, `mask`, `meta`; the full 27-field bundle
//! (shape descriptors, channel decomposition, spectra) is filled in later milestones. The `math`
//! surface is re-exported so it stays reachable up the layers.

pub use math::{Dual, Isometry3, Mat3, Quat, Scalar, Vec3};

/// Run metadata: the seed and a resolved-config summary.
#[derive(Clone, Debug, Default)]
pub struct Meta {
    pub seed: u64,
    pub description: String,
}

/// The forward model's output. Leading axis `T` is the measurement cadence.
#[derive(Clone, Debug, Default)]
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
    /// Per-detector placement: position xyz + orientation quaternion (wxyz), shape `(D, 7)`.
    pub detector_placement: Vec<[f64; 7]>,
    /// Transient-contaminated cycles, shape `(T,)`.
    pub mask: Vec<bool>,
    /// Resolved config and seed.
    pub meta: Meta,
}
