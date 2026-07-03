//! `scenario` — the runnable `Scenario` and the measurement `Schedule` (minimal M1).
//!
//! Design: `design/scenario.md`. Milestone: `milestones/M1-physics-spine.md`.
//!
//! Re-exports the seam names it consumes so the reachability edges hold up to `generate`.

pub use config::FieldSet;
pub use instrument::{Detector, DetectorArray, PhaseModel, PhaseModelKind};
pub use noise::{KeyRng, NoiseSource};
pub use source::{
    BodyMotion, Orient, Path, Prescribed, Source, SourceDynamics, Timing, Trajectory,
};

/// The measurement times. Uniform in M1; jitter/gaps arrive with M6's schedules.
#[derive(Clone, Debug, Default)]
pub struct Schedule {
    pub times: Vec<f64>,
}

impl Schedule {
    /// `n` measurements spaced `cadence` seconds apart, starting at `t = 0`.
    pub fn uniform(cadence: f64, n: usize) -> Self {
        Schedule {
            times: (0..n).map(|i| i as f64 * cadence).collect(),
        }
    }
}

/// One runnable scene: a source, a detector array, a schedule, the seed, and the phase model.
pub struct Scenario {
    pub source: Box<dyn SourceDynamics>,
    pub array: DetectorArray,
    pub schedule: Schedule,
    pub seed: u64,
    /// Which `PhaseModel` `generate` uses (default `PropagationIntegral`, the reference). The config
    /// crate stands up at M6, so the selector rides on the scenario for now.
    pub phase_model: PhaseModelKind,
    /// Which optional bundle field groups to compute (default: none).
    pub field_set: FieldSet,
}

impl Scenario {
    pub fn new(
        source: Box<dyn SourceDynamics>,
        array: DetectorArray,
        schedule: Schedule,
        seed: u64,
    ) -> Self {
        Scenario {
            source,
            array,
            schedule,
            seed,
            phase_model: PhaseModelKind::default(),
            field_set: FieldSet::default(),
        }
    }

    /// Select the phase model (builder style).
    pub fn with_phase_model(mut self, kind: PhaseModelKind) -> Self {
        self.phase_model = kind;
        self
    }

    /// Select which optional field groups to compute (builder style).
    pub fn with_field_set(mut self, field_set: FieldSet) -> Self {
        self.field_set = field_set;
        self
    }
}
