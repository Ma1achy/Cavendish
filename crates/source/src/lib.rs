//! `source` — the `SourceDynamics` seam: the trajectory library, rotation, and bodies via `shape`.
//!
//! Design: `design/source.md`. Milestone: `milestones/M2-motion-and-shape.md`.
//!
//! M0 declares the `SourceDynamics` seam with its contract; the trajectory library lands in M2.

use math::Isometry3;

/// The mass configuration versus time — how a source moves and (later) deforms.
///
/// # Contract (spec `sec:contracts`, `SourceDynamics`)
/// - **Method.** `cloud_at(t) -> Cloud` — the discretised ρ(x, t) (grid cells or particles, each a
///   mass element). M0 exposes only `pose_at`; `motion_at` and `cloud_at` land in M2.
/// - **Pre.** `t` within the scenario window.
/// - **Post.** Total mass conserved: Σᵢ mᵢ = M, independent of `t` (rigid and incompressible
///   sources). A rigid source satisfies `cloud_at(t) = pose_at(t) ⊗ source_cloud`, with `pose_at`
///   continuous (and C¹ where velocity/acceleration are requested).
/// - **Invariant.** Determinism: `cloud_at` is a pure function of `t` and fixed parameters.
pub trait SourceDynamics {
    /// The world ← body pose of the (rigid) source at time `t`.
    fn pose_at(&self, t: f64) -> Isometry3;
    // motion_at(t) -> BodyMotion (twist + acceleration) added in M2.
}
