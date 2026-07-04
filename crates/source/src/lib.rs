//! `source` — the `SourceDynamics` seam and the closed-form motion library.
//!
//! Design: `design/source.md` §3–6. Milestone: `milestones/M2-motion-and-shape.md`.
//!
//! A source is a fixed body cloud (from `shape`) carried by a pose. Motion factors as
//! `Placement ∘ Isometry{ Orient(t), Path(Timing(t)) }`. `Orient` is `Fixed` (identity) at M2;
//! rotational **dynamics** (the pendulum, the free-rotation Euler top) are M4. Velocity and
//! acceleration are analytic (the chain rule through `Timing`), supplied to the bundle by
//! [`SourceDynamics::motion_at`].

mod integrator;
pub use integrator::step;

use gravity::{Cloud, Inertia};
use math::{Isometry3, Mat3, Quat, Scalar, Vec3};
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

/// A CoM space-curve as a function of progress `u`. Generic over [`Scalar`], `S = f64` the default,
/// so a `Dual` trajectory can carry a tangent (the source-velocity/position Jacobian for `analysis`).
#[derive(Clone, Copy, Debug)]
pub enum Path<S: Scalar = f64> {
    /// Sits at the placement origin.
    Static,
    /// Straight transit from `a` to `b` over `u ∈ [0, 1]` (also the flyby, parameterised by u).
    LinearPass { a: Vec3<S>, b: Vec3<S> },
    /// `A·sin(2πf·u + φ)·ê` along a single axis.
    Oscillation {
        axis: Vec3<S>,
        amp: S,
        freq: S,
        phase: S,
    },
    /// Orbital translation of the CoM in the `xy` plane.
    Circular { radius: S, freq: S },
}

impl<S: Scalar> Path<S> {
    /// Position and its first/second derivatives with respect to progress `u`.
    fn at(&self, u: S) -> (Vec3<S>, Vec3<S>, Vec3<S>) {
        let zero = Vec3::new(S::from_f64(0.0), S::from_f64(0.0), S::from_f64(0.0));
        match *self {
            Path::Static => (zero, zero, zero),
            Path::LinearPass { a, b } => (a + (b - a).scale(u), b - a, zero),
            Path::Oscillation {
                axis,
                amp,
                freq,
                phase,
            } => {
                let w = S::from_f64(TAU) * freq;
                let theta = w * u + phase;
                (
                    axis.scale(amp * theta.sin()),
                    axis.scale(amp * w * theta.cos()),
                    axis.scale(-amp * w * w * theta.sin()),
                )
            }
            Path::Circular { radius, freq } => {
                let w = S::from_f64(TAU) * freq;
                let theta = w * u;
                let z = S::from_f64(0.0);
                (
                    Vec3::new(radius * theta.cos(), radius * theta.sin(), z),
                    Vec3::new(-radius * w * theta.sin(), radius * w * theta.cos(), z),
                    Vec3::new(
                        -radius * w * w * theta.cos(),
                        -radius * w * w * theta.sin(),
                        z,
                    ),
                )
            }
        }
    }
}

/// The `t → u` reparameterisation (how the path advances). Generic over [`Scalar`], `S = f64`.
#[derive(Clone, Copy, Debug)]
pub enum Timing<S: Scalar = f64> {
    /// Constant rate: `u = rate·t`.
    Uniform { rate: S },
    /// Constant-acceleration ease (C²): `u = rate·t + ½·accel·t²`.
    Eased { rate: S, accel: S },
}

impl<S: Scalar> Timing<S> {
    /// Progress and its first/second time-derivatives.
    fn at(&self, t: S) -> (S, S, S) {
        match *self {
            Timing::Uniform { rate } => (rate * t, rate, S::from_f64(0.0)),
            Timing::Eased { rate, accel } => (
                rate * t + S::from_f64(0.5) * accel * t * t,
                rate + accel * t,
                accel,
            ),
        }
    }
}

/// The `t → R` orientation. `Fixed` is closed-form; `FreeRotation` and `Libration` are the M4 ODE
/// motions, integrated by `Source` (which holds the cloud's inertia). Generic over [`Scalar`], `S = f64`.
#[derive(Clone, Copy, Debug)]
pub enum Orient<S: Scalar = f64> {
    Fixed(Quat<S>),
    /// Torque-free Euler top from an initial body angular velocity `ω₀`.
    ///
    /// **Precondition:** the body must be authored in its **principal frame** — the integrator reads
    /// the principal moments from the diagonal of the body-frame inertia and interprets `ω₀` there.
    /// This holds for v1's axis-aligned primitives; an arbitrary (M10) mesh with off-diagonal inertia
    /// would need `ω₀` and the cloud rotated into the principal frame first.
    FreeRotation {
        omega0: Vec3<S>,
    },
    /// Physical pendulum about a fixed pivot axis at distance `pivot_distance` from the CoM.
    Libration {
        axis: Vec3<S>,
        pivot_distance: S,
        theta0: S,
        thetadot0: S,
    },
}

impl<S: Scalar> Orient<S> {
    /// Closed-form rotation for `Fixed`; identity for the ODE variants (their orientation is
    /// integrated by `Source`, and only the `Trajectory` translation — orient-independent — is read
    /// from the closed-form path here).
    fn rotation(&self, _t: f64) -> Quat<S> {
        match *self {
            Orient::Fixed(q) => q,
            _ => Quat::identity(),
        }
    }
}

/// A composed trajectory: `pose_at(t) = placement ∘ Isometry{ Orient(t), Path(Timing(t)) }`.
/// Generic over [`Scalar`], `S = f64` — a `Dual` trajectory is how [`world_pose`] carries the
/// source-parameter tangent for `analysis`.
#[derive(Clone, Copy, Debug)]
pub struct Trajectory<S: Scalar = f64> {
    pub placement: Isometry3<S>,
    pub path: Path<S>,
    pub timing: Timing<S>,
    pub orient: Orient<S>,
}

impl<S: Scalar> Trajectory<S> {
    pub fn new(placement: Isometry3<S>, path: Path<S>, timing: Timing<S>) -> Self {
        Trajectory {
            placement,
            path,
            timing,
            orient: Orient::Fixed(Quat::identity()),
        }
    }

    /// Set the orientation motion (builder style).
    pub fn with_orient(mut self, orient: Orient<S>) -> Self {
        self.orient = orient;
        self
    }

    /// The closed-form pose (translation and the `Fixed`/identity rotation). The integrated
    /// orientations (`FreeRotation`/`Libration`) are added by [`world_pose`]; this reads only the
    /// orient-independent translation for them.
    pub fn pose_at(&self, t: f64) -> Isometry3<S> {
        let (u, _, _) = self.timing.at(S::from_f64(t));
        let (p, _, _) = self.path.at(u);
        self.placement
            .compose(Isometry3::new(self.orient.rotation(t), p))
    }
}

impl Trajectory<f64> {
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

/// The full world←body pose at `t`, generic over [`Scalar`] — the single differentiable pose recipe.
/// Composes the closed-form translation ([`Trajectory::pose_at`]) with the (possibly ODE-integrated)
/// orientation, reading the body constants (principal moments, pivot inertia) from `inertia`. This is
/// exactly what `Source::pose_at` computes at `S = f64` (guarded by `world_pose_matches_source`), so
/// `compute` can instantiate it at `S = Dual` for the CRB Jacobian and know it differentiates the
/// canonical path.
pub fn world_pose<S: Scalar>(
    traj: &Trajectory<S>,
    inertia: &Inertia,
    fine_dt: f64,
    t: f64,
) -> Isometry3<S> {
    let translation = traj.pose_at(t).translation;
    let pr = traj.placement.rotation;
    let rotation = match traj.orient {
        Orient::Fixed(q) => pr * q,
        Orient::FreeRotation { omega0 } => {
            let m = &inertia.i.m;
            let moments = Vec3::new(
                S::from_f64(m[0][0]),
                S::from_f64(m[1][1]),
                S::from_f64(m[2][2]),
            );
            let (q, _, _) = integrator::free_rotation_state(omega0, moments, t, fine_dt);
            pr * q
        }
        Orient::Libration {
            axis,
            pivot_distance,
            theta0,
            thetadot0,
        } => {
            let axis_n = axis.scale(S::from_f64(1.0) / axis.norm());
            let i_axis = axis_n.dot(lift_mat3::<S>(&inertia.i).mul_vec(axis_n));
            let mass = S::from_f64(inertia.mass);
            let i_pivot = i_axis + mass * pivot_distance * pivot_distance;
            let k = mass * S::from_f64(G_ACCEL) * pivot_distance / i_pivot;
            let (q, _, _) = integrator::libration_state(axis_n, k, theta0, thetadot0, t, fine_dt);
            pr * q
        }
    };
    Isometry3::new(rotation, translation)
}

/// Lift an `f64` 3×3 (a body inertia — constant w.r.t. the differentiated θ) into the scalar type.
fn lift_mat3<S: Scalar>(a: &Mat3<f64>) -> Mat3<S> {
    Mat3 {
        m: core::array::from_fn(|i| core::array::from_fn(|j| S::from_f64(a.m[i][j]))),
    }
}

/// Local gravity for the physical pendulum (spec `tab:params`).
const G_ACCEL: f64 = 9.81;
/// Default rotation-integration substep.
const ROT_FINE_DT: f64 = 0.01;

/// A body cloud carried by a trajectory. Carries the cloud's inertia so the ODE orientations can be
/// integrated from the shape alone plus an initial spin.
pub struct Source {
    cloud: Cloud,
    trajectory: Trajectory,
    inertia: Inertia,
    fine_dt: f64,
}

impl Source {
    pub fn new(cloud: Cloud, trajectory: Trajectory) -> Self {
        let inertia = gravity::inertia(&cloud);
        Source {
            cloud,
            trajectory,
            inertia,
            fine_dt: ROT_FINE_DT,
        }
    }

    /// Set the rotation-integration substep (builder style; used by the Dzhanibekov step-stability check).
    pub fn with_fine_dt(mut self, fine_dt: f64) -> Self {
        self.fine_dt = fine_dt;
        self
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
        Ok(Source::new(shape::scale_mass(&unit, mass), trajectory))
    }

    /// Principal moments in the body frame (the body is authored in its principal frame at v1).
    fn principal_moments(&self) -> Vec3<f64> {
        let m = &self.inertia.i.m;
        Vec3::new(m[0][0], m[1][1], m[2][2])
    }

    /// World orientation, body angular velocity, and body angular acceleration at `t`.
    fn rotation_state(&self, t: f64) -> (Quat, Vec3<f64>, Vec3<f64>) {
        let pr = self.trajectory.placement.rotation;
        let zero = Vec3::new(0.0, 0.0, 0.0);
        match self.trajectory.orient {
            Orient::Fixed(q) => (pr * q, zero, zero),
            Orient::FreeRotation { omega0 } => {
                let (q, w, wd) = integrator::free_rotation_state(
                    omega0,
                    self.principal_moments(),
                    t,
                    self.fine_dt,
                );
                (pr * q, w, wd)
            }
            Orient::Libration {
                axis,
                pivot_distance,
                theta0,
                thetadot0,
            } => {
                let axis_n = axis.scale(1.0 / axis.norm());
                let i_axis = axis_n.dot(self.inertia.i.mul_vec(axis_n)); // moment about the pivot axis
                let i_pivot = i_axis + self.inertia.mass * pivot_distance * pivot_distance;
                let k = self.inertia.mass * G_ACCEL * pivot_distance / i_pivot;
                let (q, w, wd) =
                    integrator::libration_state(axis_n, k, theta0, thetadot0, t, self.fine_dt);
                (pr * q, w, wd)
            }
        }
    }
}

impl SourceDynamics for Source {
    fn body_cloud(&self) -> &Cloud {
        &self.cloud
    }
    fn pose_at(&self, t: f64) -> Isometry3 {
        // Translation is orient-independent; the rotation comes from the (possibly integrated) orient.
        let translation = self.trajectory.pose_at(t).translation;
        let (rotation, _, _) = self.rotation_state(t);
        Isometry3::new(rotation, translation)
    }
    fn motion_at(&self, t: f64) -> BodyMotion {
        let m = self.trajectory.motion_at(t); // CoM linear velocity/acceleration
        let (_, angular_velocity, angular_acceleration) = self.rotation_state(t);
        BodyMotion {
            velocity: m.velocity,
            acceleration: m.acceleration,
            angular_velocity,
            angular_acceleration,
        }
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
    #[allow(clippy::float_cmp)] // world_pose must reproduce Source::pose_at bit-for-bit
    fn world_pose_matches_source() {
        // world_pose::<f64> is the same recipe Source::pose_at uses — assert bit-for-bit agreement
        // for static, free-rotation, and libration sources, so the Dual sweep differentiates the
        // canonical forward path (not a parallel one that merely happens to be close).
        let cloud = Cloud::from_elements(&[
            (0.5, 0.0, 0.0, 10.0),
            (-0.5, 0.0, 0.0, 10.0),
            (0.0, 0.7, 0.0, 8.0),
            (0.0, 0.0, 0.3, 6.0),
        ]);
        let place = Isometry3::new(
            Quat::from_axis_angle(Vec3::new(0.1, 0.8, -0.2), 0.4),
            Vec3::new(1.5, -0.5, 2.0),
        );
        let orients = [
            Orient::Fixed(Quat::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), 0.6)),
            Orient::FreeRotation {
                omega0: Vec3::new(0.4, 0.1, 0.7),
            },
            Orient::Libration {
                axis: Vec3::new(0.0, 1.0, 0.0),
                pivot_distance: 0.9,
                theta0: 0.3,
                thetadot0: 0.1,
            },
        ];
        for orient in orients {
            let traj = Trajectory::new(
                place,
                Path::LinearPass {
                    a: Vec3::new(0.0, 0.0, 0.0),
                    b: Vec3::new(1.0, -2.0, 0.5),
                },
                Timing::Uniform { rate: 0.5 },
            )
            .with_orient(orient);
            let src = Source::new(cloud.clone(), traj);
            for &t in &[0.0, 0.37, 1.9] {
                let a = world_pose(&src.trajectory, &src.inertia, src.fine_dt, t);
                let b = src.pose_at(t);
                assert_eq!(a.translation.x, b.translation.x, "tx");
                assert_eq!(a.translation.y, b.translation.y, "ty");
                assert_eq!(a.translation.z, b.translation.z, "tz");
                assert_eq!(a.rotation.w, b.rotation.w, "qw");
                assert_eq!(a.rotation.x, b.rotation.x, "qx");
                assert_eq!(a.rotation.y, b.rotation.y, "qy");
                assert_eq!(a.rotation.z, b.rotation.z, "qz");
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
