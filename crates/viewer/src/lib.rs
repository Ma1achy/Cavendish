//! `viewer` — the egui/wgpu inspector. Becomes a binary at M9.
//!
//! Design: `design/viewer.md`. Milestone: `milestones/M9-viewer.md`.
//!
//! M0 names `generate`'s public surface (the `edges_reachable` check); the egui/wgpu app lands in M9.

#[cfg(test)]
mod names_generate {
    #[allow(unused_imports)]
    use generate::{ComputeBackend, FieldContribution, PhaseModel, SourceDynamics, Vec3};
}
