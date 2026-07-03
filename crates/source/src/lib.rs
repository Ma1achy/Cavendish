//! `source` — the `SourceDynamics` seam and the minimal `Prescribed` dynamics.
//!
//! Design: `design/source.md`. Milestone: `milestones/M1-physics-spine.md`.
//!
//! M1 provides only `Prescribed`: a fixed body cloud carried by an explicit pose closure (static or
//! constant-velocity). The full trajectory library — oscillation, spin, the shape voxeliser — is M2.

use gravity::Cloud;
use math::{Isometry3, Quat, Vec3};

/// The mass configuration versus time — how a source moves and (later) deforms.
///
/// # Contract (spec `sec:contracts`, `SourceDynamics`)
/// - **Post.** Total mass conserved: `cloud_at(t)` is a rigid map of a fixed body cloud, so
///   `Σᵢ mᵢ = M` independent of `t`. A rigid source satisfies `cloud_at(t) = pose_at(t) ⊗ body`.
/// - **Invariant.** Determinism: `cloud_at` is a pure function of `t` and fixed parameters.
pub trait SourceDynamics {
    /// The world ← body pose at time `t`.
    fn pose_at(&self, t: f64) -> Isometry3;
    /// The posed world cloud at time `t`.
    fn cloud_at(&self, t: f64) -> Cloud;
}

/// A fixed body cloud carried by an explicit pose closure.
pub struct Prescribed {
    body: Cloud,
    pose: Box<dyn Fn(f64) -> Isometry3>,
}

impl Prescribed {
    pub fn new(body: Cloud, pose: impl Fn(f64) -> Isometry3 + 'static) -> Self {
        Prescribed {
            body,
            pose: Box::new(pose),
        }
    }

    /// A static source at a fixed pose.
    pub fn fixed(body: Cloud, iso: Isometry3) -> Self {
        Prescribed::new(body, move |_| iso)
    }

    /// A source translating at constant velocity from `x0`.
    pub fn constant_velocity(body: Cloud, x0: Vec3<f64>, v: Vec3<f64>) -> Self {
        Prescribed::new(body, move |t| {
            Isometry3::new(Quat::identity(), x0 + v.scale(t))
        })
    }
}

impl SourceDynamics for Prescribed {
    fn pose_at(&self, t: f64) -> Isometry3 {
        (self.pose)(t)
    }
    fn cloud_at(&self, t: f64) -> Cloud {
        self.body.transformed(&(self.pose)(t))
    }
}
