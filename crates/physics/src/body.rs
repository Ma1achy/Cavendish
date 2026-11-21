//! Bodies as transported mass parcels — one mechanism for rigid, articulated and deforming solids.
//!
//! A body is voxelised **once**, at rest, into parcels of fixed mass. Every tick each parcel is
//! carried to its current position by a map, and the forward model integrates the parcels where they
//! are. Whether the body is rigid, articulated or deforming is a property of the map, not a branch
//! in the engine: rigid is one transform, articulated is a skeleton's transforms blended by per-parcel
//! weights, deforming is a displacement field.
//!
//! Mass is conserved by construction — parcels are relocated, never re-weighed — whatever the map
//! does. Where a joint folds the body onto itself the parcels simply pile up: nothing is counted
//! twice, because there are no per-link bodies to overlap.

use std::sync::{Arc, Mutex};

use gravity::{Cloud, FieldContribution};
use math::{Isometry3, Mat3, Quat, Vec3};
use source::BodyMotion;
use world::{Channel, Eval, Track, WorldSource};

/// One bone of a skeleton in its rest pose: it pivots about `head`.
#[derive(Clone, Copy, Debug)]
pub struct Bone {
    pub parent: Option<usize>,
    pub head: Vec3<f64>,
    pub tail: Vec3<f64>,
}

/// A hierarchy of bones. A bone's parent must precede it in the list.
#[derive(Clone, Debug)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
}

/// A skeleton's configuration: where the whole body is, and each bone's rotation about its head
/// relative to its parent.
#[derive(Clone, Debug)]
pub struct Pose {
    pub root: Isometry3,
    pub joints: Vec<Quat>,
}

impl Skeleton {
    /// The rest → world transform of every bone (forward kinematics down the hierarchy).
    pub fn transforms(&self, pose: &Pose) -> Vec<Isometry3> {
        assert_eq!(
            pose.joints.len(),
            self.bones.len(),
            "one joint rotation per bone"
        );
        let mut out: Vec<Isometry3> = Vec::with_capacity(self.bones.len());
        for (bone, &q) in self.bones.iter().zip(&pose.joints) {
            // Rotation about the head: x ↦ head + q(x − head).
            let about = Isometry3::new(q, bone.head - q.rotate(bone.head));
            let parent = match bone.parent {
                Some(p) => {
                    assert!(p < out.len(), "a bone's parent must precede it");
                    out[p]
                }
                None => pose.root,
            };
            out.push(parent.compose(about));
        }
        out
    }

    /// Per-parcel bone weights from distance to each bone segment, `N × B` row-major. The nearest
    /// bone gets weight 1 before normalisation and others fall off as a Gaussian of the *extra*
    /// distance over `falloff`; every row sums to one. A partition of unity is all the transport
    /// needs — it never differentiates the weights — so this crude field is still exact for mass.
    pub fn proximity_weights(&self, cloud: &Cloud, falloff: f64) -> Vec<f64> {
        assert!(falloff > 0.0);
        let b = self.bones.len();
        let mut w = vec![0.0; cloud.len() * b];
        for i in 0..cloud.len() {
            let p = Vec3::new(cloud.xs[i], cloud.ys[i], cloud.zs[i]);
            let dist: Vec<f64> = self
                .bones
                .iter()
                .map(|bone| segment_distance(p, bone))
                .collect();
            let nearest = dist.iter().fold(f64::INFINITY, |m, &d| m.min(d));
            let row = &mut w[i * b..(i + 1) * b];
            for (wk, d) in row.iter_mut().zip(&dist) {
                *wk = (-((d - nearest) / falloff).powi(2)).exp();
            }
            let sum: f64 = row.iter().sum();
            row.iter_mut().for_each(|x| *x /= sum);
        }
        w
    }
}

fn segment_distance(p: Vec3<f64>, bone: &Bone) -> f64 {
    let ab = bone.tail - bone.head;
    let len2 = ab.norm_squared();
    let s = if len2 > 0.0 {
        ((p - bone.head).dot(ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p - (bone.head + ab.scale(s))).norm()
}

type PositionMap = dyn Fn(f64, &Cloud) -> Cloud + Send + Sync;
type FrameMap = dyn Fn(f64) -> Isometry3 + Send + Sync;

/// A body of fixed mass parcels carried by a time-dependent map.
pub struct Parcels {
    rest: Cloud,
    map: Box<PositionMap>,
    frame: Box<FrameMap>,
    channel: Channel,
    /// The most recently posed cloud. The phase integral evaluates both arms at each quadrature
    /// time, so the second evaluation reuses the first's posing.
    cache: Mutex<Option<(u64, Arc<Cloud>)>>,
}

impl Parcels {
    fn new(rest: Cloud, map: Box<PositionMap>, frame: Box<FrameMap>) -> Self {
        Parcels {
            rest,
            map,
            frame,
            channel: Channel::Target,
            cache: Mutex::new(None),
        }
    }

    /// A rigid body: every parcel through the one transform `pose(t)`.
    pub fn rigid(rest: Cloud, pose: impl Fn(f64) -> Isometry3 + Send + Sync + 'static) -> Self {
        let pose = Arc::new(pose);
        let p = pose.clone();
        Parcels::new(
            rest,
            Box::new(move |t, rest| rest.transformed(&p(t))),
            Box::new(move |t| pose(t)),
        )
    }

    /// An articulated or skinned body: each parcel is carried by its bones' transforms blended by
    /// its weights (linear blend skinning of the *volume*). `weights` is `N × B`, rows summing to one.
    pub fn skinned(
        rest: Cloud,
        skeleton: Skeleton,
        weights: Vec<f64>,
        pose: impl Fn(f64) -> Pose + Send + Sync + 'static,
    ) -> Self {
        let b = skeleton.bones.len();
        assert_eq!(weights.len(), rest.len() * b, "weights must be N × B");
        for row in weights.chunks(b) {
            let sum: f64 = row.iter().sum();
            assert!(
                (sum - 1.0).abs() <= 1e-9,
                "skinning weights must sum to one per parcel"
            );
        }
        let pose = Arc::new(pose);
        let p = pose.clone();
        Parcels::new(
            rest,
            Box::new(move |t, rest| {
                let bones = skeleton.transforms(&p(t));
                let mut out = rest.clone();
                for i in 0..rest.len() {
                    let x = Vec3::new(rest.xs[i], rest.ys[i], rest.zs[i]);
                    let mut acc = Vec3::new(0.0, 0.0, 0.0);
                    for (bone, &w) in bones.iter().zip(&weights[i * b..(i + 1) * b]) {
                        if w != 0.0 {
                            acc = acc + bone.apply(x).scale(w);
                        }
                    }
                    out.xs[i] = acc.x;
                    out.ys[i] = acc.y;
                    out.zs[i] = acc.z;
                }
                out
            }),
            Box::new(move |t| pose(t).root),
        )
    }

    /// A deforming body: each parcel displaced from its rest position by `displacement(rest, t)`
    /// (body frame), then placed in the world by the fixed `placement`.
    pub fn displaced(
        rest: Cloud,
        placement: Isometry3,
        displacement: impl Fn(Vec3<f64>, f64) -> Vec3<f64> + Send + Sync + 'static,
    ) -> Self {
        Parcels::new(
            rest,
            Box::new(move |t, rest| {
                let mut out = rest.clone();
                for i in 0..rest.len() {
                    let x = Vec3::new(rest.xs[i], rest.ys[i], rest.zs[i]);
                    let p = placement.apply(x + displacement(x, t));
                    out.xs[i] = p.x;
                    out.ys[i] = p.y;
                    out.zs[i] = p.z;
                }
                out
            }),
            Box::new(move |_| placement),
        )
    }

    /// Record this body under the ambient channel (structure, not a target).
    pub fn ambient(mut self) -> Self {
        self.channel = Channel::Ambient;
        self
    }

    /// Replace each parcel's mass by its mass in excess of the fluid it displaces:
    /// `m ← m − ρ_fluid·V`, with the body's volume `V = M/ρ_body` shared in proportion to mass. This
    /// is the mass that gravitates *relative to the ambient medium* — the right source when the
    /// surrounding air is modelled as an anomaly about a uniform background.
    pub fn in_fluid(mut self, body_density: f64, fluid_density: f64) -> Self {
        let k = 1.0 - fluid_density / body_density;
        self.rest.ms.iter_mut().for_each(|m| *m *= k);
        *self.cache.get_mut().unwrap() = None;
        self
    }

    /// The rest (body-frame) parcels.
    pub fn rest(&self) -> &Cloud {
        &self.rest
    }

    /// The parcels at time `t`, in the world frame. Masses are the rest masses, bit for bit.
    pub fn posed(&self, t: f64) -> Arc<Cloud> {
        let key = t.to_bits();
        let mut cache = self.cache.lock().unwrap();
        if let Some((k, cloud)) = cache.as_ref() {
            if *k == key {
                return cloud.clone();
            }
        }
        let mut cloud = (self.map)(t, &self.rest);
        cloud.ms.clone_from(&self.rest.ms);
        let cloud = Arc::new(cloud);
        *cache = Some((key, cloud.clone()));
        cloud
    }
}

impl FieldContribution<f64> for Parcels {
    fn potential(&self, p: Vec3<f64>, t: f64) -> f64 {
        gravity::potential(&self.posed(t), p)
    }
    fn gradient_tensor(&self, p: Vec3<f64>, t: f64) -> Mat3<f64> {
        gravity::gradient_tensor(&self.posed(t), p)
    }
}

/// The twist and acceleration of a frame, by central differences of the frame map.
fn frame_motion(frame: &FrameMap, t: f64) -> BodyMotion {
    let h = 1e-4;
    let (a, b, c) = (frame(t - h), frame(t), frame(t + h));
    let velocity = (c.translation - a.translation).scale(1.0 / (2.0 * h));
    let acceleration =
        (c.translation - b.translation.scale(2.0) + a.translation).scale(1.0 / (h * h));
    // ω from the relative rotation over a step: δq = q(t₂)·q(t₁)⁻¹ ≈ (1, ½ω·Δt). Body frame, to
    // match the rigid sources.
    let omega = |q0: Quat, q1: Quat, dt: f64| {
        let d = q1 * q0.conjugate();
        let s = if d.w < 0.0 { -2.0 } else { 2.0 };
        b.rotation
            .conjugate()
            .rotate(Vec3::new(d.x, d.y, d.z).scale(s / dt))
    };
    let (w0, w1) = (
        omega(a.rotation, b.rotation, h),
        omega(b.rotation, c.rotation, h),
    );
    BodyMotion {
        velocity,
        acceleration,
        angular_velocity: omega(a.rotation, c.rotation, 2.0 * h),
        angular_acceleration: (w1 - w0).scale(1.0 / h),
    }
}

impl WorldSource for Parcels {
    fn eval(&self) -> Eval<'_> {
        Eval::Field(self)
    }
    fn world_cloud_at(&self, t: f64) -> Cloud {
        (*self.posed(t)).clone()
    }
    fn channel(&self) -> Channel {
        self.channel
    }
    fn track_at(&self, t: f64) -> Option<Track> {
        Some(Track {
            pose: (self.frame)(t),
            motion: frame_motion(&self.frame, t),
        })
    }
    fn body_cloud(&self) -> Option<&Cloud> {
        Some(&self.rest)
    }
}

/// A simply supported rectangular plate (a wall panel, a floor slab) ringing in its bending modes.
///
/// Kirchhoff plate theory: mode `(m, n)` has shape `sin(mπξ) sin(nπη)` over the plate and angular
/// frequency `ω_mn = π²[(m/a)² + (n/b)²]·√(D/ρh)`, with flexural rigidity `D = E h³ / 12(1 − ν²)`.
#[derive(Clone, Debug)]
pub struct Plate {
    /// In-plane side lengths `a` (body x) and `b` (body y), and thickness `h` (body z) [m].
    pub size: [f64; 3],
    /// Young's modulus [Pa], Poisson ratio, density [kg m⁻³].
    pub young: f64,
    pub poisson: f64,
    pub density: f64,
    /// Excited modes: `(m, n, amplitude [m], phase [rad])`.
    pub modes: Vec<(u32, u32, f64, f64)>,
}

impl Plate {
    /// The angular frequency of bending mode `(m, n)` [rad s⁻¹].
    pub fn omega(&self, m: u32, n: u32) -> f64 {
        let [a, b, h] = self.size;
        let rigidity = self.young * h.powi(3) / (12.0 * (1.0 - self.poisson * self.poisson));
        let pi2 = std::f64::consts::PI.powi(2);
        pi2 * ((m as f64 / a).powi(2) + (n as f64 / b).powi(2))
            * (rigidity / (self.density * h)).sqrt()
    }

    /// The out-of-plane displacement at body point `x` and time `t`.
    pub fn deflection(&self, x: Vec3<f64>, t: f64) -> f64 {
        let [a, b, _] = self.size;
        let (xi, eta) = (x.x / a + 0.5, x.y / b + 0.5);
        let pi = std::f64::consts::PI;
        self.modes
            .iter()
            .map(|&(m, n, amp, phase)| {
                amp * (m as f64 * pi * xi).sin()
                    * (n as f64 * pi * eta).sin()
                    * (self.omega(m, n) * t + phase).cos()
            })
            .sum()
    }

    /// The plate as an ambient parcel source: voxelised at `pitch`, placed by `placement` (body z is
    /// the plate normal).
    pub fn source(self, pitch: f64, placement: Isometry3) -> Result<Parcels, shape::ShapeError> {
        let [a, b, h] = self.size;
        let solid = shape::Cuboid {
            half: [0.5 * a, 0.5 * b, 0.5 * h],
        };
        let rest = shape::voxelise(
            &solid,
            &shape::VoxelParams::pitch(pitch),
            shape::MassSpec::Density(self.density),
        )?;
        Ok(Parcels::displaced(rest, placement, move |x, t| {
            Vec3::new(0.0, 0.0, self.deflection(x, t))
        })
        .ambient())
    }
}

/// A walking figure built from primitives — a torso, a head and four limbs, the limbs swinging about
/// the shoulders and hips as it moves. A deliberately simple "complex object": several parts, a real
/// skeleton, non-rigid motion.
pub struct Walker {
    /// Total mass [kg] and standing height [m].
    pub mass: f64,
    pub height: f64,
    /// Voxel pitch [m].
    pub pitch: f64,
    /// Where it starts (feet on the ground at `start.z`), its horizontal velocity, and its stride.
    pub start: Vec3<f64>,
    pub velocity: Vec3<f64>,
    /// Peak limb swing [rad] and step frequency [Hz].
    pub swing: f64,
    pub cadence: f64,
}

impl Walker {
    pub fn source(&self) -> Result<Parcels, shape::ShapeError> {
        let s = self.height / 1.75; // proportions scale with height
        let vox = shape::VoxelParams::pitch(self.pitch);
        // (solid, centre) for each part, in a frame with the feet at z = 0.
        let limb = |r: f64, half_l: f64| shape::Cylinder {
            r: r * s,
            half_l: half_l * s,
        };
        let parts: Vec<(Box<dyn shape::Solid>, Vec3<f64>)> = vec![
            (
                Box::new(shape::Cuboid {
                    half: [0.17 * s, 0.11 * s, 0.30 * s],
                }),
                Vec3::new(0.0, 0.0, 1.15 * s),
            ),
            (
                Box::new(shape::Sphere { r: 0.11 * s }),
                Vec3::new(0.0, 0.0, 1.61 * s),
            ),
            (
                Box::new(limb(0.045, 0.30)),
                Vec3::new(-0.23 * s, 0.0, 1.13 * s),
            ),
            (
                Box::new(limb(0.045, 0.30)),
                Vec3::new(0.23 * s, 0.0, 1.13 * s),
            ),
            (
                Box::new(limb(0.07, 0.425)),
                Vec3::new(-0.09 * s, 0.0, 0.425 * s),
            ),
            (
                Box::new(limb(0.07, 0.425)),
                Vec3::new(0.09 * s, 0.0, 0.425 * s),
            ),
        ];
        // Each part is voxelised about its own centre (uniform density, mass ∝ volume) and placed.
        let mut rest = Cloud::default();
        for (solid, centre) in &parts {
            let part = shape::voxelise(solid.as_ref(), &vox, shape::MassSpec::Density(1.0))?;
            for i in 0..part.len() {
                rest.xs.push(part.xs[i] + centre.x);
                rest.ys.push(part.ys[i] + centre.y);
                rest.zs.push(part.zs[i] + centre.z);
                rest.ms.push(part.ms[i]);
            }
        }
        let rest = shape::scale_mass(&rest, self.mass);

        let joint = |x: f64, z: f64| Vec3::new(x * s, 0.0, z * s);
        let bone = |parent, head: Vec3<f64>, tail: Vec3<f64>| Bone { parent, head, tail };
        let skeleton = Skeleton {
            bones: vec![
                bone(None, joint(0.0, 0.85), joint(0.0, 1.45)),
                bone(Some(0), joint(0.0, 1.50), joint(0.0, 1.72)),
                bone(Some(0), joint(-0.23, 1.43), joint(-0.23, 0.83)),
                bone(Some(0), joint(0.23, 1.43), joint(0.23, 0.83)),
                bone(Some(0), joint(-0.09, 0.85), joint(-0.09, 0.0)),
                bone(Some(0), joint(0.09, 0.85), joint(0.09, 0.0)),
            ],
        };
        let weights = skeleton.proximity_weights(&rest, 0.03 * s);

        // Face the direction of travel (body +y forward); limbs swing about the body x axis.
        let (start, velocity, swing, cadence) =
            (self.start, self.velocity, self.swing, self.cadence);
        let heading = Quat::from_axis_angle(
            Vec3::new(0.0, 0.0, 1.0),
            if velocity.norm() > 0.0 {
                (-velocity.x).atan2(velocity.y)
            } else {
                0.0
            },
        );
        let tau = std::f64::consts::TAU;
        Ok(Parcels::skinned(rest, skeleton, weights, move |t| {
            let phase = (tau * cadence * t).sin() * swing;
            let about_x = |angle: f64| Quat::from_axis_angle(Vec3::new(1.0, 0.0, 0.0), angle);
            Pose {
                root: Isometry3::new(heading, start + velocity.scale(t)),
                joints: vec![
                    Quat::identity(),
                    Quat::identity(),
                    about_x(phase),
                    about_x(-phase),
                    about_x(-phase),
                    about_x(phase),
                ],
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use instrument::{Detector, PhaseModel, PropagationIntegral};
    use source::Prescribed;

    fn blob() -> Cloud {
        shape::voxelise(
            &shape::Cuboid {
                half: [0.15, 0.25, 0.4],
            },
            &shape::VoxelParams::pitch(0.1),
            shape::MassSpec::Total(80.0),
        )
        .unwrap()
    }

    fn tumbling(t: f64) -> Isometry3 {
        Isometry3::new(
            Quat::from_axis_angle(Vec3::new(0.2, 1.0, 0.4), 0.7 * t),
            Vec3::new(2.0 + 0.3 * t, 0.5 - 0.2 * t, 1.5),
        )
    }

    #[test]
    fn rigid_parcels_equal_the_rigid_source() {
        // Rigid is the one-transform limit of parcel transport: its phase equals the v1 rigid
        // source's (amortised, body-frame) phase to rounding.
        let parcels = Parcels::rigid(blob(), tumbling);
        let rigid = Prescribed::new(blob(), tumbling);
        let (model, det) = (PropagationIntegral::default(), Detector::new(0.0));
        for &t in &[1.5, 2.9] {
            let a = model.delta_phi(&[], &[&parcels], &det, t);
            let b = model.delta_phi(&[&rigid], &[], &det, t);
            assert!(
                (a - b).abs() <= 1e-11 * b.abs(),
                "parcels {a:e} vs rigid {b:e}"
            );
        }
    }

    #[test]
    fn one_bone_skeleton_is_rigid() {
        let skeleton = Skeleton {
            bones: vec![Bone {
                parent: None,
                head: Vec3::new(0.0, 0.0, -0.4),
                tail: Vec3::new(0.0, 0.0, 0.4),
            }],
        };
        let rest = blob();
        let weights = skeleton.proximity_weights(&rest, 0.1);
        let skinned = Parcels::skinned(rest.clone(), skeleton, weights, |t| Pose {
            root: tumbling(t),
            joints: vec![Quat::identity()],
        });
        let want = rest.transformed(&tumbling(1.3));
        let got = skinned.posed(1.3);
        for i in 0..rest.len() {
            let d = (got.xs[i] - want.xs[i]).abs()
                + (got.ys[i] - want.ys[i]).abs()
                + (got.zs[i] - want.zs[i]).abs();
            assert!(d <= 1e-13, "parcel {i} off by {d:e}");
        }
    }

    #[test]
    #[allow(clippy::float_cmp)] // conservation here is exact, not approximate
    fn mass_is_conserved_exactly_through_any_pose() {
        // The walker folding its limbs: every parcel keeps its rest mass bit for bit, so the total
        // is identical at every instant — no rescale, no Jacobian.
        let walker = Walker {
            mass: 70.0,
            height: 1.75,
            pitch: 0.06,
            start: Vec3::new(2.0, -3.0, 0.0),
            velocity: Vec3::new(0.0, 1.2, 0.0),
            swing: 0.6,
            cadence: 0.9,
        }
        .source()
        .unwrap();
        let rest_total: f64 = walker.rest().total_mass();
        assert!((rest_total - 70.0).abs() <= 1e-12);
        for &t in &[0.0, 0.31, 1.7, 4.4] {
            let posed = walker.posed(t);
            assert_eq!(posed.ms, walker.rest().ms);
            assert_eq!(posed.total_mass(), rest_total);
        }
    }

    #[test]
    fn walker_walks_and_swings() {
        let walker = Walker {
            mass: 70.0,
            height: 1.75,
            pitch: 0.06,
            start: Vec3::new(2.0, -3.0, 0.0),
            velocity: Vec3::new(0.0, 1.2, 0.0),
            swing: 0.6,
            cadence: 0.9,
        }
        .source()
        .unwrap();
        // The centre of mass advances at the walking velocity (limb swing is antisymmetric).
        let com = |t: f64| gravity::inertia(&walker.posed(t)).com;
        let (a, b) = (com(0.0), com(2.0));
        assert!(
            ((b.y - a.y) / 2.0 - 1.2).abs() <= 0.02,
            "CoM speed {}",
            (b.y - a.y) / 2.0
        );
        assert!(
            (a.x - 2.0).abs() <= 0.01 && a.z > 0.8 && a.z < 1.1,
            "CoM {a:?}"
        );
        // The limbs really move: at peak swing the body is longer fore-aft than when upright.
        let extent_y = |t: f64| {
            let c = walker.posed(t);
            let (lo, hi) =
                c.ys.iter()
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &y| {
                        (l.min(y), h.max(y))
                    });
            hi - lo
        };
        let quarter = 0.25 / 0.9;
        assert!(
            extent_y(quarter) > extent_y(0.0) + 0.3,
            "{} vs {}",
            extent_y(quarter),
            extent_y(0.0)
        );
        // And the track reports the walking velocity.
        let track = walker.track_at(1.0).unwrap();
        assert!((track.motion.velocity.y - 1.2).abs() <= 1e-6);
    }

    #[test]
    fn skinning_weights_are_a_partition_of_unity() {
        let skeleton = Skeleton {
            bones: vec![
                Bone {
                    parent: None,
                    head: Vec3::new(0.0, 0.0, -0.4),
                    tail: Vec3::new(0.0, 0.0, 0.0),
                },
                Bone {
                    parent: Some(0),
                    head: Vec3::new(0.0, 0.0, 0.0),
                    tail: Vec3::new(0.0, 0.0, 0.4),
                },
            ],
        };
        let rest = blob();
        let w = skeleton.proximity_weights(&rest, 0.05);
        for (i, row) in w.chunks(2).enumerate() {
            assert!((row[0] + row[1] - 1.0).abs() <= 1e-15);
            // Far from the joint a parcel belongs to its own bone.
            if rest.zs[i] > 0.3 {
                assert!(row[1] > 0.999, "upper parcel weight {}", row[1]);
            }
        }
    }

    #[test]
    fn hinge_moves_the_far_parcels_rigidly() {
        // Bend the upper bone 90° about x at the origin: a parcel well up the limb moves as the bone.
        let skeleton = Skeleton {
            bones: vec![
                Bone {
                    parent: None,
                    head: Vec3::new(0.0, 0.0, -0.4),
                    tail: Vec3::new(0.0, 0.0, 0.0),
                },
                Bone {
                    parent: Some(0),
                    head: Vec3::new(0.0, 0.0, 0.0),
                    tail: Vec3::new(0.0, 0.0, 0.4),
                },
            ],
        };
        let rest = blob();
        let w = skeleton.proximity_weights(&rest, 0.02);
        let quarter = Quat::from_axis_angle(Vec3::new(1.0, 0.0, 0.0), std::f64::consts::FRAC_PI_2);
        let bent = Parcels::skinned(rest.clone(), skeleton, w, move |_| Pose {
            root: Isometry3::identity(),
            joints: vec![Quat::identity(), quat_copy(quarter)],
        });
        let posed = bent.posed(0.0);
        let i = (0..rest.len()).find(|&i| rest.zs[i] > 0.3).unwrap();
        let want = quarter.rotate(Vec3::new(rest.xs[i], rest.ys[i], rest.zs[i]));
        assert!(
            (posed.xs[i] - want.x).abs()
                + (posed.ys[i] - want.y).abs()
                + (posed.zs[i] - want.z).abs()
                <= 1e-6
        );
    }

    fn quat_copy(q: Quat) -> Quat {
        q
    }

    #[test]
    fn plate_frequency_and_nodes() {
        // A 4 × 3 m, 0.2 m concrete slab: ω₁₁ from the closed form, and mode (2,1) has a node line
        // down the middle that never moves.
        let plate = Plate {
            size: [4.0, 3.0, 0.2],
            young: 30e9,
            poisson: 0.2,
            density: 2400.0,
            modes: vec![(2, 1, 1e-4, 0.3)],
        };
        let d = 30e9 * 0.2f64.powi(3) / (12.0 * (1.0 - 0.04));
        let want =
            std::f64::consts::PI.powi(2) * (1.0 / 16.0 + 1.0 / 9.0) * (d / (2400.0 * 0.2)).sqrt();
        assert!((plate.omega(1, 1) - want).abs() <= 1e-9 * want);
        assert!(
            (plate.omega(1, 1) / std::f64::consts::TAU - 56.7).abs() <= 0.5,
            "f₁₁ ≈ 57 Hz"
        );
        for &t in &[0.0, 0.004, 0.011] {
            assert!(plate.deflection(Vec3::new(0.0, 0.4, 0.0), t).abs() <= 1e-18);
        }
        assert!(plate.deflection(Vec3::new(1.0, 0.0, 0.0), 0.0).abs() > 5e-5);
        // As a source it is ambient, conserves mass, and weighs ρ·V.
        let src = plate.source(0.1, Isometry3::identity()).unwrap();
        assert_eq!(src.channel(), Channel::Ambient);
        let m = src.posed(0.003).total_mass();
        assert!((m - 2400.0 * 4.0 * 3.0 * 0.2).abs() <= 1e-6 * m);
    }

    #[test]
    fn displaced_air_reduces_the_gravitating_mass() {
        let body = Parcels::rigid(blob(), tumbling).in_fluid(1000.0, 1.2);
        assert!((body.posed(0.0).total_mass() - 80.0 * (1.0 - 1.2e-3)).abs() <= 1e-12);
    }
}
