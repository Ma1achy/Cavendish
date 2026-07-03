//! `generate` — orchestration: the `run` spine that drives a `Scenario` to a `StateBundle`.
//!
//! Design: `design/generate.md`. Milestone: `milestones/M1-physics-spine.md`.
//!
//! M1 is the walking skeleton: build the arms once, evaluate `PropagationIntegral` at each scheduled
//! time, assemble the minimal bundle. Batch dispatch and the `ComputeBackend` path arrive at M6. The
//! seam names are re-exported so every lower crate's public surface stays reachable here (M0-R1).

pub use compute::{ComputeBackend, EvalBatch, SignalBatch};
pub use gravity::FieldContribution;
pub use scenario::{
    BodyMotion, Detector, KeyRng, NoiseSource, Orient, Path, PhaseModel, Prescribed, Scenario,
    Schedule, Source, SourceDynamics, Timing, Trajectory,
};
pub use state::{Dual, Isometry3, Mat3, Quat, Scalar, StateBundle, Vec3};

use gravity::Cloud;
use instrument::PropagationIntegral;
use state::Meta;

/// Drive a scenario through the propagation-integral spine to a `StateBundle`.
pub fn run(scenario: &Scenario) -> StateBundle {
    let model = PropagationIntegral::default();
    let sources: [&dyn SourceDynamics; 1] = [scenario.source.as_ref()];

    let mut time = Vec::new();
    let mut signal = Vec::new();
    let mut track = Vec::new();
    let mut vel = Vec::new();
    let mut acc = Vec::new();
    let mut mask = Vec::new();
    for &t in &scenario.schedule.times {
        let dphi = model.delta_phi(&sources, &scenario.detector, t);
        let pos = scenario.source.pose_at(t).translation;
        let m = scenario.source.motion_at(t);
        time.push(t);
        signal.push(vec![dphi]);
        track.push([pos.x, pos.y, pos.z]);
        vel.push([m.velocity.x, m.velocity.y, m.velocity.z]);
        acc.push([m.acceleration.x, m.acceleration.y, m.acceleration.z]);
        mask.push(false);
    }

    StateBundle {
        time,
        signal,
        source_position: vec![track],
        source_velocity: vec![vel],
        source_accel: vec![acc],
        mask,
        meta: Meta {
            seed: scenario.seed,
            description: "M2 propagation-integral spine".into(),
        },
    }
}

/// Ad-hoc concrete-wall lattice — **throwaway**, replaced by `shape`'s voxeliser in M2.
///
/// A regular lattice of point elements filling the `size` cuboid centred at `centre`, each carrying
/// `m = density·pitch³` (no renormalisation). Do not invest in it; it exists only to feed the M1
/// anchor before the real geometry pipeline lands.
pub fn wall_cloud(size: Vec3<f64>, centre: Vec3<f64>, density: f64, pitch: f64) -> Cloud {
    let m = density * pitch * pitch * pitch;
    let counts = [
        (size.x / pitch).round().max(1.0) as usize,
        (size.y / pitch).round().max(1.0) as usize,
        (size.z / pitch).round().max(1.0) as usize,
    ];
    let origin = [
        centre.x - size.x * 0.5,
        centre.y - size.y * 0.5,
        centre.z - size.z * 0.5,
    ];
    let mut elems = Vec::with_capacity(counts[0] * counts[1] * counts[2]);
    for ix in 0..counts[0] {
        let x = origin[0] + (ix as f64 + 0.5) * pitch;
        for iy in 0..counts[1] {
            let y = origin[1] + (iy as f64 + 0.5) * pitch;
            for iz in 0..counts[2] {
                let z = origin[2] + (iz as f64 + 0.5) * pitch;
                elems.push((x, y, z, m));
            }
        }
    }
    Cloud::from_elements(&elems)
}

#[cfg(test)]
mod edges_reachable {
    //! Naming each lower crate's public surface proves the dependency edges carry it up to L4.
    #[allow(unused_imports)]
    use super::{
        ComputeBackend, Detector, Dual, EvalBatch, FieldContribution, Isometry3, KeyRng, Mat3,
        NoiseSource, PhaseModel, Prescribed, Quat, Scalar, Scenario, Schedule, SignalBatch,
        SourceDynamics, StateBundle, Vec3,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use gravity::Cloud;
    use instrument::{InstrumentConfig, PropagationIntegral};

    fn point_source(x: f64, z: f64) -> Prescribed {
        Prescribed::fixed(
            Cloud::from_elements(&[(x, 0.0, z, 500.0)]),
            Isometry3::identity(),
        )
    }

    #[test]
    #[allow(clippy::float_cmp)] // kinematics are copied verbatim from motion_at — exact by construction
    fn kinematics_filled() {
        // source_velocity/accel in the bundle match motion_at exactly.
        let traj = Trajectory::new(
            Isometry3::identity(),
            Path::LinearPass {
                a: Vec3::new(1.0, 0.0, 2.5),
                b: Vec3::new(1.0, 0.0, 8.5),
            },
            Timing::Uniform { rate: 0.4 },
        );
        let cloud = Cloud::from_elements(&[(1.0, 0.0, 2.5, 10.0)]);
        let sched = Schedule::uniform(2.0, 4);
        let expected: Vec<_> = sched.times.iter().map(|&t| traj.motion_at(t)).collect();
        let scn = Scenario::new(
            Box::new(Source::new(cloud, traj)),
            Detector::new(0.0),
            sched,
            7,
        );
        let bundle = run(&scn);
        for (i, m) in expected.iter().enumerate() {
            assert_eq!(
                bundle.source_velocity[0][i],
                [m.velocity.x, m.velocity.y, m.velocity.z]
            );
            assert_eq!(
                bundle.source_accel[0][i],
                [m.acceleration.x, m.acceleration.y, m.acceleration.z]
            );
        }
    }

    #[test]
    fn spine_runs() {
        // Finite, non-zero ΔΦ, and the sign flips as the source crosses the detector plane — the
        // gradiometer centre at base_z + Δr/2 = 2.5 m (the two IFOs sit at 0 and 5).
        let det = Detector::new(0.0);
        let model = PropagationIntegral::default();
        let above = point_source(1.0, 4.0); // above the centre, between the IFOs
        let below = point_source(1.0, 1.0); // below the centre, between the IFOs
        let dphi_above = model.delta_phi(&[&above], &det, 2.0);
        let dphi_below = model.delta_phi(&[&below], &det, 2.0);
        assert!(dphi_above.is_finite() && dphi_above != 0.0);
        assert!(dphi_below.is_finite() && dphi_below != 0.0);
        assert!(
            dphi_above * dphi_below < 0.0,
            "sign did not flip across the plane"
        );
    }

    #[test]
    fn bundle_shapes() {
        let scenario = Scenario::new(
            Box::new(point_source(1.0, 2.5)),
            Detector::new(0.0),
            Schedule::uniform(2.0, 5),
            42,
        );
        let bundle = run(&scenario);
        assert_eq!(bundle.time.len(), 5);
        assert_eq!(bundle.signal.len(), 5);
        assert_eq!(bundle.signal[0].len(), 1);
        assert_eq!(bundle.source_position.len(), 1);
        assert_eq!(bundle.source_position[0].len(), 5);
        assert_eq!(bundle.mask.len(), 5);
        assert!(bundle.time.iter().all(|t| t.is_finite()));
    }

    #[test]
    fn quadrature_converges() {
        // Halving fine_dt changes ΔΦ negligibly — the integrator is far finer than the signal.
        let det = Detector::new(0.0);
        let src = point_source(1.5, 2.5);
        let phi = |dt: f64| {
            let cfg = InstrumentConfig {
                fine_dt: dt,
                ..InstrumentConfig::default()
            };
            PropagationIntegral::new(cfg).delta_phi(&[&src], &det, 2.0)
        };
        let coarse = phi(0.01);
        let fine = phi(0.005);
        assert!(
            (coarse - fine).abs() / fine.abs() <= 1e-6,
            "quadrature not converged"
        );
    }
}
