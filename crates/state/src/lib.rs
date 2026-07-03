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
    /// Source orientation quaternion wxyz (world ← body), shape `(S, T, 4)`.
    pub source_orientation: Vec<Vec<[f64; 4]>>,
    /// Source angular velocity ω (direction = spin axis, magnitude = rate), shape `(S, T, 3)`.
    pub source_angular_velocity: Vec<Vec<[f64; 3]>>,
    /// Source angular acceleration, shape `(S, T, 3)`.
    pub source_angular_accel: Vec<Vec<[f64; 3]>>,
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
}
