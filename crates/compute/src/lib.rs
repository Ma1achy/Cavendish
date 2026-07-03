//! `compute` — the `ComputeBackend` seam: the CPU reference and the wgpu fast path (two-pass).
//!
//! Design: `design/compute.md`. Milestone: `milestones/M6-compute-gpu-and-batch.md`.
//!
//! M0 declares the `ComputeBackend` seam; the CPU and wgpu backends land in M6.

/// An evaluation batch — parameters (not poses) for a run. Opaque in M0; fields land in M6.
pub struct EvalBatch;

/// Per-detector ΔΦ signals produced by a backend. Opaque in M0; fields land in M6.
pub struct SignalBatch;

/// Executes the forward model — the only crate that knows about devices.
///
/// # Contract (`DESIGN.md` §3.4, `design/compute.md`)
/// - **Method.** `eval(batch) -> SignalBatch` — take an `EvalBatch` (parameters, not poses) and
///   return per-detector signals.
/// - **Post.** `CpuBackend` (rayon, f64) is bit-reproducible for a fixed reduction order;
///   `WgpuBackend` (WGSL, f32) is differential-first and reproduces the CPU path to the validation
///   tolerance, never worse.
pub trait ComputeBackend {
    /// Evaluate one batch, returning per-detector signals.
    fn eval(&self, batch: &EvalBatch) -> SignalBatch;
}
