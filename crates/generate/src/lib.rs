//! `generate` — orchestration: `run`/`stream`, the forward-model wiring, and batch dispatch.
//!
//! Design: `design/generate.md`. Milestone: `milestones/M6-compute-gpu-and-batch.md`.
//!
//! M0 names every crate up to L3 through its dependency edges — the `edges_reachable` check below
//! is the compile-time proof (M0-R1). No orchestration yet; that lands in M6.

pub use compute::{ComputeBackend, EvalBatch, SignalBatch};
pub use gravity::{FieldContribution, Fields};
pub use state::{Dual, Isometry3, Mat3, Quat, Scalar, Vec3};
// SourceDynamics, PhaseModel, Detector, NoiseSource and KeyRng arrive via scenario's re-exports.
pub use scenario::{Detector, KeyRng, NoiseSource, PhaseModel, SourceDynamics};

#[cfg(test)]
mod edges_reachable {
    //! Naming each lower crate's public surface proves the dependency edges carry it up to L4.
    //! Compilation is the assertion.
    #[allow(unused_imports)]
    use super::{
        ComputeBackend, Detector, Dual, EvalBatch, FieldContribution, Fields, Isometry3, KeyRng,
        Mat3, NoiseSource, PhaseModel, Quat, Scalar, SignalBatch, SourceDynamics, Vec3,
    };
}
