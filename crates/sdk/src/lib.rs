//! `sdk` — the Python/torch boundary (PyO3, DLPack, GIL release). Becomes a cdylib at M7.
//!
//! Design: `design/sdk.md`. Milestone: `milestones/M7-sdk-python-torch.md`.
//!
//! M0 names `generate`'s public surface (the `edges_reachable` check); the PyO3 layer lands in M7.

#[cfg(test)]
mod names_generate {
    #[allow(unused_imports)]
    use generate::{ComputeBackend, FieldContribution, NoiseSource, PhaseModel, SourceDynamics};
}
