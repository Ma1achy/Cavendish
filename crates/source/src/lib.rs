//! `source` — the `SourceDynamics` seam and the closed-form motion library.
//!
//! Design: `design/source.md` §3–6. Milestone: `milestones/M2-motion-and-shape.md`.
//!
//! A source is a fixed body cloud (from `shape`) carried by a pose. Motion factors as
//! `Placement ∘ Isometry{ Orient(t), Path(Timing(t)) }`. `Orient` is `Fixed` (identity) at M2;
//! rotational **dynamics** (the pendulum, the free-rotation Euler top) are M4. Velocity and
//! acceleration are analytic (the chain rule through `Timing`), supplied to the bundle by
//! [`SourceDynamics::motion_at`].

use gravity::Cloud;
use math::{Isometry3, Quat, Vec3};
use shape::{voxelise, MassSpec, Registry, ShapeError, Solid, VoxelParams};

const TAU: f64 = std::f64::consts::TAU;

/// Kinematic state of the source at a time: the CoM twist and its acceleration (world frame).
#[derive(Clone, Copy, Debug)]
pub struct BodyMotion {
    pub velocity: Vec3<f64>,
    pub acceleration: Vec3<f64>,
    pub angular_velocity: Vec3<f64>,
    pub angular_acceleration: Vec3<f64>,
}

impl BodyMotion {
    pub fn zero() -> Self {
        let z = Vec3::new(0.0, 0.0, 0.0);
        BodyMotion {
            velocity: z,
            acceleration: z,
            angular_velocity: z,
            angular_acceleration: z,
        }
    }
}

/// The mass configuration versus time.
///
/// # Contract (spec `sec:contracts`, `SourceDynamics`)
/// - **Post.** `body_cloud` is fixed (rigid); total mass conserved. `cloud_at(t) = pose_at(t) ⊗ body`.
/// - **Invariant.** Determinism: `pose_at`/`motion_at` are pure functions of `t` and fixed parameters.
pub trait SourceDynamics {
    /// The fixed body-frame mass cloud (built once, never moves).
    fn body_cloud(&self) -> &Cloud;
    /// The world ← body pose at time `t`.
    fn pose_at(&self, t: f64) -> Isometry3;
    /// The CoM kinematic state at time `t`, for the bundle (not needed by field evaluation).
    fn motion_at(&self, t: f64) -> BodyMotion;
}

/// A CoM space-curve as a function of progress `u`.
#[derive(Clone, Copy, Debug)]
pub enum Path {
    /// Sits at the placement origin.
    Static,
    /// Straight transit from `a` to `b` over `u ∈ [0, 1]` (also the flyby, parameterised by u).
    LinearPass { a: Vec3<f64>, b: Vec3<f64> },
    /// `A·sin(2πf·u + φ)·ê` along a single axis.
    Oscillation {
        axis: Vec3<f64>,
        amp: f64,
        freq: f64,
        phase: f64,
    },
    /// Orbital translation of the CoM in the `xy` plane.
    Circular { radius: f64, freq: f64 },
}

impl Path {
    /// Position and its first/second derivatives with respect to progress `u`.
    fn at(&self, u: f64) -> (Vec3<f64>, Vec3<f64>, Vec3<f64>) {
        let zero = Vec3::new(0.0, 0.0, 0.0);
        match *self {
            Path::Static => (zero, zero, zero),
            Path::LinearPass { a, b } => (a + (b - a).scale(u), b - a, zero),
            Path::Oscillation {
                axis,
                amp,
                freq,
                phase,
            } => {
                let w = TAU * freq;
                let theta = w * u + phase;
                (
                    axis.scale(amp * theta.sin()),
                    axis.scale(amp * w * theta.cos()),
                    axis.scale(-amp * w * w * theta.sin()),
                )
            }
            Path::Circular { radius, freq } => {
                let w = TAU * freq;
                let theta = w * u;
                (
                    Vec3::new(radius * theta.cos(), radius * theta.sin(), 0.0),
                    Vec3::new(-radius * w * theta.sin(), radius * w * theta.cos(), 0.0),
                    Vec3::new(
                        -radius * w * w * theta.cos(),
                        -radius * w * w * theta.sin(),
                        0.0,
                    ),
                )
            }
        }
    }
}

/// The `t → u` reparameterisation (how the path advances).
#[derive(Clone, Copy, Debug)]
pub enum Timing {
    /// Constant rate: `u = rate·t`.
    Uniform { rate: f64 },
    /// Constant-acceleration ease (C²): `u = rate·t + ½·accel·t²`.
    Eased { rate: f64, accel: f64 },
}

impl Timing {
    /// Progress and its first/second time-derivatives.
    fn at(&self, t: f64) -> (f64, f64, f64) {
        match *self {
            Timing::Uniform { rate } => (rate * t, rate, 0.0),
            Timing::Eased { rate, accel } => {
                (rate * t + 0.5 * accel * t * t, rate + accel * t, accel)
            }
        }
    }
}

/// The `t → R` orientation. `Fixed` (identity) at M2; dynamic rotations are M4.
#[derive(Clone, Copy, Debug)]
pub enum Orient {
    Fixed(Quat),
}

impl Orient {
    fn rotation(&self, _t: f64) -> Quat {
        match *self {
            Orient::Fixed(q) => q,
        }
    }
}

/// A composed trajectory: `pose_at(t) = placement ∘ Isometry{ Orient(t), Path(Timing(t)) }`.
#[derive(Clone, Copy, Debug)]
pub struct Trajectory {
    pub placement: Isometry3,
    pub path: Path,
    pub timing: Timing,
    pub orient: Orient,
}

impl Trajectory {
    pub fn new(placement: Isometry3, path: Path, timing: Timing) -> Self {
        Trajectory {
            placement,
            path,
            timing,
            orient: Orient::Fixed(Quat::identity()),
        }
    }

    pub fn pose_at(&self, t: f64) -> Isometry3 {
        let (u, _, _) = self.timing.at(t);
        let (p, _, _) = self.path.at(u);
        self.placement
            .compose(Isometry3::new(self.orient.rotation(t), p))
    }

    pub fn motion_at(&self, t: f64) -> BodyMotion {
        let (u, du, d2u) = self.timing.at(t);
        let (_, dp, d2p) = self.path.at(u);
        // Chain rule: d/dt Path(u(t)) = Path'(u)·u̇, d²/dt² = Path''(u)·u̇² + Path'(u)·ü.
        let vel_local = dp.scale(du);
        let acc_local = d2p.scale(du * du) + dp.scale(d2u);
        let rot = self.placement.rotation; // placement is static, so world = R_placement · local
        BodyMotion {
            velocity: rot.rotate(vel_local),
            acceleration: rot.rotate(acc_local),
            angular_velocity: Vec3::new(0.0, 0.0, 0.0),
            angular_acceleration: Vec3::new(0.0, 0.0, 0.0),
        }
    }
}

/// A body cloud carried by a trajectory.
pub struct Source {
    cloud: Cloud,
    trajectory: Trajectory,
}

impl Source {
    pub fn new(cloud: Cloud, trajectory: Trajectory) -> Self {
        Source { cloud, trajectory }
    }

    /// Voxelise a primitive (cached at unit mass) and scale to `mass`, then attach the trajectory.
    #[allow(clippy::too_many_arguments)]
    pub fn primitive(
        solid: &dyn Solid,
        voxel: &VoxelParams,
        mass: f64,
        trajectory: Trajectory,
        registry: &Registry,
        key: u64,
    ) -> Result<Self, ShapeError> {
        let unit = registry.resolve(key, || voxelise(solid, voxel, MassSpec::Total(1.0)))?;
        Ok(Source {
            cloud: shape::scale_mass(&unit, mass),
            trajectory,
        })
    }
}

impl SourceDynamics for Source {
    fn body_cloud(&self) -> &Cloud {
        &self.cloud
    }
    fn pose_at(&self, t: f64) -> Isometry3 {
        self.trajectory.pose_at(t)
    }
    fn motion_at(&self, t: f64) -> BodyMotion {
        self.trajectory.motion_at(t)
    }
}

/// A fixed body cloud carried by an explicit pose closure (the M1 stopgap; kept for direct scenes).
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
    fn body_cloud(&self) -> &Cloud {
        &self.body
    }
    fn pose_at(&self, t: f64) -> Isometry3 {
        (self.pose)(t)
    }
    fn motion_at(&self, t: f64) -> BodyMotion {
        // Central finite difference of the pose translation (the closure has no analytic form).
        let h = 1e-4;
        let p = |tt: f64| (self.pose)(tt).translation;
        let vel = (p(t + h) - p(t - h)).scale(1.0 / (2.0 * h));
        let acc = (p(t + h) - p(t).scale(2.0) + p(t - h)).scale(1.0 / (h * h));
        BodyMotion {
            velocity: vel,
            acceleration: acc,
            angular_velocity: Vec3::new(0.0, 0.0, 0.0),
            angular_acceleration: Vec3::new(0.0, 0.0, 0.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combine(p: impl Fn(f64) -> Vec3<f64>, t: f64, h: f64, w: [f64; 5]) -> Vec3<f64> {
        p(t - 2.0 * h).scale(w[0])
            + p(t - h).scale(w[1])
            + p(t).scale(w[2])
            + p(t + h).scale(w[3])
            + p(t + 2.0 * h).scale(w[4])
    }

    fn rel(a: Vec3<f64>, b: Vec3<f64>) -> f64 {
        (a - b).norm() / b.norm().max(1.0)
    }

    #[test]
    fn path_consistency() {
        // Central 4th-order finite differences of pose(t) match analytic ṙ, r̈ for every path×timing.
        let place = Isometry3::new(
            Quat::from_axis_angle(Vec3::new(0.2, 1.0, -0.3), 0.5),
            Vec3::new(1.0, 2.0, 3.0),
        );
        let trajectories = [
            Trajectory::new(
                place,
                Path::LinearPass {
                    a: Vec3::new(0.0, 0.0, 0.0),
                    b: Vec3::new(3.0, -1.0, 2.0),
                },
                Timing::Uniform { rate: 0.7 },
            ),
            Trajectory::new(
                place,
                Path::Oscillation {
                    axis: Vec3::new(0.0, 0.0, 1.0),
                    amp: 0.5,
                    freq: 0.2,
                    phase: 0.3,
                },
                Timing::Eased {
                    rate: 0.8,
                    accel: 0.1,
                },
            ),
            Trajectory::new(
                Isometry3::identity(),
                Path::Circular {
                    radius: 2.0,
                    freq: 0.15,
                },
                Timing::Uniform { rate: 1.0 },
            ),
        ];
        let h = 1e-3;
        let v_stencil = [1.0, -8.0, 0.0, 8.0, -1.0];
        let a_stencil = [-1.0, 16.0, -30.0, 16.0, -1.0];
        for traj in &trajectories {
            for &t in &[0.4, 1.3, 2.7] {
                let pos = |tt: f64| traj.pose_at(tt).translation;
                let fd_v = combine(pos, t, h, v_stencil).scale(1.0 / (12.0 * h));
                let fd_a = combine(pos, t, h, a_stencil).scale(1.0 / (12.0 * h * h));
                let m = traj.motion_at(t);
                assert!(rel(fd_v, m.velocity) <= 1e-8, "velocity mismatch");
                assert!(rel(fd_a, m.acceleration) <= 1e-8, "acceleration mismatch");
            }
        }
    }

    #[test]
    fn oscillation_params() {
        // Amplitude, axis and period recovered from the trajectory to machine precision.
        let axis = Vec3::new(0.0, 0.0, 1.0);
        let (amp, freq, phase) = (0.7, 0.2, 0.35);
        let traj = Trajectory::new(
            Isometry3::identity(),
            Path::Oscillation {
                axis,
                amp,
                freq,
                phase,
            },
            Timing::Uniform { rate: 1.0 },
        );
        // At the peak time 2πf·t + φ = π/2, displacement = A·ê exactly.
        let t_peak = (std::f64::consts::FRAC_PI_2 - phase) / (TAU * freq);
        let peak = traj.pose_at(t_peak).translation;
        assert!((peak.norm() - amp).abs() <= 1e-10, "amplitude");
        assert!(rel(peak.scale(1.0 / amp), axis) <= 1e-10, "axis");
        // Periodic with period 1/f.
        let later = traj.pose_at(t_peak + 1.0 / freq).translation;
        assert!((later - peak).norm() <= 1e-10, "period");
    }
}
