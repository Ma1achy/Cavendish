//! `state` — the `StateBundle` output contract, the `FieldSet` cost knob, and the Lomb-Scargle periodogram.
//!
//! Design: `design/state.md`. Milestone: `milestones/M6-compute-gpu-and-batch.md`.
//!
//! M0 re-exports the `math` surface it consumes so the reachability edges hold up to `generate`.

pub use math::{Dual, Isometry3, Mat3, Quat, Scalar, Vec3};
