//! `config` — the typed configuration schema and distribution primitives.
//!
//! Design: `DESIGN.md`. Milestone: `milestones/M6-compute-gpu-and-batch.md`.
//!
//! Mostly stood up at M6; M4 adds the one field it needs, `FieldSet`, so the bundle's optional
//! descriptor groups can be gated per run.

/// Which optional bundle field groups to compute — a cost knob, not a task (spec `FieldSet`).
#[derive(Clone, Copy, Debug, Default)]
pub struct FieldSet {
    /// Fill the static shape descriptors (mass, inertia, moments, axes, quadrupole).
    pub shape: bool,
    /// Decompose `signal` into its channels (targets, atmospheric, uldm, per-IFO) — the ≈2× cost of a
    /// second gravitational pass. Off ⇒ one combined pass, channel fields `None`.
    pub decomposition: bool,
}
