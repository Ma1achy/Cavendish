//! `gravity` — the gravity kernel: `Cloud`, the element law, potential/field/gradient-tensor, the second-moment reduction, and the analytic field oracle.
//!
//! Design: `design/gravity.md`. Milestone: `milestones/M1-physics-spine.md`.
//!
//! M0 declares the field seams (`FieldContribution`, `Fields`) with their contracts; the kernel
//! itself lands in M1. See `milestones/M0-scaffolding.md` §2.3.

use math::{Scalar, Vec3};

/// A term summed into the gravitational potential during the forward pass.
///
/// The atmospheric-GGN source is the canonical impl: a contribution folded into the potential in
/// the forward pass, never a post-hoc `NoiseSource`. Generic over `Scalar` so it differentiates on
/// the kernel path.
///
/// # Contract (spec `sec:contracts`, `GravitySource`)
/// - **Pre.** `p` finite; for a point mass, `p` is not the source position.
/// - **Post.** `field = −∇potential` and `gradient = −∇∇potential`, each consistent to numerical
///   tolerance; the gradient is symmetric and trace-free in vacuum.
/// - **Invariant.** Linear over a cloud (`potential = Σᵢ elementᵢ.potential`); far- and near-field
///   paths agree at the cutoff to tolerance. Purity: no observable state mutation.
pub trait FieldContribution<S: Scalar> {
    /// The scalar potential contribution at world point `p` and time `t`.
    fn potential(&self, p: Vec3<S>, t: f64) -> S;
}

/// The queryable gravitational field at a given scalar type — what a `PhaseModel` reads.
///
/// A marker in M0; the query surface (`potential`/`field`/`gradient`, generic over `Scalar`) is
/// finalised in M1. The role is fixed here so `instrument` can compile against it.
pub trait Fields<S: Scalar> {}
