//! `world` — the unified source seam: anything massive in the world, behind one trait.
//!
//! Design: `design/world.md`. Milestone: `milestones/M11-world-seam.md`.
//!
//! v1 made the rigid body the native source (`SourceDynamics`: a fixed body cloud carried by a pose)
//! and bolted time-varying fields on beside it. Here the general case is a **world-frame mass
//! distribution that varies with time**, and the rigid body is one implementation of it. The
//! forward-model spine (`gravity`, `instrument`) is untouched: it already has two sockets — rigid
//! sources evaluated body-frame through the inverse pose (the amortised path) and world-frame
//! `FieldContribution`s — and [`Eval`] says which socket a source plugs into. `generate` routes on
//! it; a caller assembling a `Scenario` never has to.

use gravity::{Cloud, FieldContribution};
use instrument::{Detector, InstrumentConfig};
use math::Isometry3;
use source::{BodyMotion, SourceDynamics};

/// Which ground-truth channel a source's phase is recorded under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// A tracked object — recorded in `signal_targets`.
    Target,
    /// Environmental mass motion (air, structure) — recorded in `signal_atmospheric`.
    Ambient,
}

/// How the forward model evaluates a source — the socket of the phase integral it plugs into.
pub enum Eval<'a> {
    /// A fixed body cloud carried by a pose, evaluated body-frame at the back-transformed arm point.
    Rigid(&'a dyn SourceDynamics),
    /// A time-varying world-frame mass field, evaluated at `(p, t)`.
    Field(&'a dyn FieldContribution<f64>),
}

/// The frame a source is tracked by, at one time: its pose and that frame's twist/acceleration.
#[derive(Clone, Copy, Debug)]
pub struct Track {
    pub pose: Isometry3,
    pub motion: BodyMotion,
}

/// Anything massive in the world.
///
/// # Contract
/// - **Pre.** Queryable on `[t − 2T, t]` for every measurement time `t` (the arm-flight window).
/// - **Post.** [`world_cloud_at`](Self::world_cloud_at) is the mass the forward model integrates:
///   the phase from [`eval`](Self::eval) equals the phase of that cloud, to the tolerance the
///   implementation documents. Total mass is conserved in time unless the source says otherwise.
/// - **Invariant.** Pure in `t`: no observable state, the same answer in any call order.
pub trait WorldSource {
    /// The forward-model socket for this source.
    fn eval(&self) -> Eval<'_>;

    /// The world-frame mass distribution at time `t` — the general representation.
    fn world_cloud_at(&self, t: f64) -> Cloud;

    /// The decomposition channel this source is recorded under.
    fn channel(&self) -> Channel {
        Channel::Target
    }

    /// The tracked frame at `t`, if the source has one. Sources without a body frame (a fluid) return
    /// `None` and do not appear on the bundle's source axis.
    fn track_at(&self, _t: f64) -> Option<Track> {
        None
    }

    /// The rest (body-frame) geometry posed by the track, if the source has one.
    fn body_cloud(&self) -> Option<&Cloud> {
        None
    }

    /// An exact reformulation of the two single-interferometer propagation phases `[δφ₁, δφ₂]` at
    /// measurement time `t`, when the source has one cheaper than quadrature through
    /// [`eval`](Self::eval). Must agree with the propagation integral of `eval()` to rounding.
    fn per_ifo_fast(&self, _cfg: &InstrumentConfig, _det: &Detector, _t: f64) -> Option<[f64; 2]> {
        None
    }
}

/// A rigid body: any `SourceDynamics`, seen as a `WorldSource`. The amortised path — the body cloud
/// is built once and only the pose is evaluated per tick.
pub struct Rigid(pub Box<dyn SourceDynamics>);

impl WorldSource for Rigid {
    fn eval(&self) -> Eval<'_> {
        Eval::Rigid(self.0.as_ref())
    }
    fn world_cloud_at(&self, t: f64) -> Cloud {
        self.0.body_cloud().transformed(&self.0.pose_at(t))
    }
    fn track_at(&self, t: f64) -> Option<Track> {
        Some(Track {
            pose: self.0.pose_at(t),
            motion: self.0.motion_at(t),
        })
    }
    fn body_cloud(&self) -> Option<&Cloud> {
        Some(self.0.body_cloud())
    }
}

/// A source moved to the ambient channel (a vibrating wall is rigid, but it is not a target).
pub struct Ambient<W: WorldSource>(pub W);

impl<W: WorldSource> WorldSource for Ambient<W> {
    fn eval(&self) -> Eval<'_> {
        self.0.eval()
    }
    fn world_cloud_at(&self, t: f64) -> Cloud {
        self.0.world_cloud_at(t)
    }
    fn channel(&self) -> Channel {
        Channel::Ambient
    }
    fn per_ifo_fast(&self, cfg: &InstrumentConfig, det: &Detector, t: f64) -> Option<[f64; 2]> {
        self.0.per_ifo_fast(cfg, det, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use instrument::{PhaseModel, PropagationIntegral};
    use math::{Quat, Vec3};
    use source::Prescribed;

    /// A world cloud re-evaluated from scratch each call — the general case with no fast path.
    struct Reposed<'a>(&'a Rigid);
    impl FieldContribution<f64> for Reposed<'_> {
        fn potential(&self, p: Vec3<f64>, t: f64) -> f64 {
            gravity::potential(&self.0.world_cloud_at(t), p)
        }
        fn gradient_tensor(&self, p: Vec3<f64>, t: f64) -> math::Mat3<f64> {
            gravity::gradient_tensor(&self.0.world_cloud_at(t), p)
        }
    }

    #[test]
    fn rigid_fast_path_equals_world_cloud() {
        // The seam's post-condition for a rigid body: the amortised (inverse-pose) phase equals the
        // phase of the world cloud it reports. Different arithmetic, same physics — to rounding.
        let cloud = Cloud::from_elements(&[
            (0.2, 0.0, 0.1, 300.0),
            (-0.2, 0.1, 0.0, 200.0),
            (0.0, -0.1, -0.2, 250.0),
        ]);
        let spin = Vec3::new(0.3, 1.0, -0.2);
        let rigid = Rigid(Box::new(Prescribed::new(cloud, move |t| {
            Isometry3::new(
                Quat::from_axis_angle(spin, 0.8 * t),
                Vec3::new(2.5 + 0.4 * t, -0.3 * t, 2.0),
            )
        })));
        let det = Detector::new(0.0);
        let model = PropagationIntegral::default();
        let Eval::Rigid(src) = rigid.eval() else {
            panic!("a Rigid evaluates through the rigid socket")
        };
        for &t in &[1.5, 2.2, 3.7] {
            let fast = model.delta_phi(&[src], &[], &det, t);
            let general = model.delta_phi(&[], &[&Reposed(&rigid)], &det, t);
            let rel = (fast - general).abs() / fast.abs();
            assert!(
                rel <= 1e-11,
                "t={t}: fast {fast} vs world cloud {general} ({rel:.2e})"
            );
        }
    }

    #[test]
    fn rigid_reports_track_and_conserves_mass() {
        let cloud = Cloud::from_elements(&[(0.1, 0.0, 0.0, 4.0), (-0.1, 0.0, 0.0, 6.0)]);
        let rigid = Rigid(Box::new(Prescribed::constant_velocity(
            cloud,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.5, 0.0, 0.0),
        )));
        for &t in &[0.0, 1.0, 7.5] {
            let w = rigid.world_cloud_at(t);
            assert!((w.total_mass() - 10.0).abs() <= 1e-12);
            let track = rigid.track_at(t).expect("a rigid body has a track");
            assert!((track.pose.translation.x - (1.0 + 0.5 * t)).abs() <= 1e-12);
        }
        assert_eq!(Ambient(rigid).channel(), Channel::Ambient);
    }
}
