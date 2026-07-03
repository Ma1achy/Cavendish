//! `scenario` — the runnable `Scenario`, the `Schedule`, and the optional `Prior` (batch sugar).
//!
//! Design: `design/scenario.md`. Milestone: `milestones/M6-compute-gpu-and-batch.md`.
//!
//! M0 re-exports the seam names it consumes so the reachability edges hold up to `generate`.

pub use instrument::{Detector, PhaseModel};
pub use noise::{KeyRng, NoiseSource};
pub use source::SourceDynamics;
