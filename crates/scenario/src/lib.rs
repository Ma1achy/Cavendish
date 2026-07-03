//! `scenario` — the runnable `Scenario` and the measurement `Schedule` (minimal M1).
//!
//! Design: `design/scenario.md`. Milestone: `milestones/M1-physics-spine.md`.
//!
//! Re-exports the seam names it consumes so the reachability edges hold up to `generate`.

pub use instrument::{Detector, PhaseModel};
pub use noise::{KeyRng, NoiseSource};
pub use source::{Prescribed, SourceDynamics};

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

/// One runnable scene: a source, a detector, a schedule, and the seed.
pub struct Scenario {
    pub source: Box<dyn SourceDynamics>,
    pub detector: Detector,
    pub schedule: Schedule,
    pub seed: u64,
}

impl Scenario {
    pub fn new(
        source: Box<dyn SourceDynamics>,
        detector: Detector,
        schedule: Schedule,
        seed: u64,
    ) -> Self {
        Scenario {
            source,
            detector,
            schedule,
            seed,
        }
    }
}
