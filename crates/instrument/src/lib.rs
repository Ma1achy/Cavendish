//! `instrument` — the `PhaseModel` seam: the detector array, ballistic arms, and the two phase models.
//!
//! Design: `design/instrument.md`. Milestone: `milestones/M1-physics-spine.md`.
//!
//! M0 declares the `PhaseModel` seam with its contract; the arms and phase models land in M1.

use gravity::Fields;
use math::Scalar;

/// One gradiometer in the array. Opaque in M0; its geometry (position, arms) is built in M1.
pub struct Detector;

/// Maps a scene of sources to one gradiometer's differential phase at a measurement time.
///
/// # Contract (spec `sec:contracts`, `PhaseModel`)
/// - **Method.** `delta_phi(src, instr, t) -> ΔΦ_ℓ` in radians.
/// - **Pre.** `src` queryable on `[t − 2T, t]`; the instrument arms are built.
/// - **Post.** Returns the double-difference (spec `eq:doublediff`); linear in source mass
///   (`delta_phi(α·m) = α·delta_phi(m)` to tolerance); deterministic. `QuasiStaticGradient` agrees
///   with `PropagationIntegral` in the uniform-field limit.
pub trait PhaseModel {
    /// The differential phase ΔΦ_ℓ for detector `det`, generic over the scalar type.
    fn delta_phi<S: Scalar>(&self, src: &dyn Fields<S>, det: &Detector, t: f64) -> S;
}
