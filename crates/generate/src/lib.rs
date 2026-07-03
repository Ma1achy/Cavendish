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
    BodyMotion, Detector, DetectorArray, FieldSet, KeyRng, NoiseSource, Orient, Path, PhaseModel,
    PhaseModelKind, Prescribed, Scenario, Schedule, Source, SourceDynamics, Timing, Trajectory,
};
pub use state::{Dual, Isometry3, Mat3, Quat, Scalar, StateBundle, Vec3};

use instrument::{PropagationIntegral, QuasiStaticGradient};
use state::Meta;

/// Drive a scenario through the selected phase model to a `StateBundle`.
pub fn run(scenario: &Scenario) -> StateBundle {
    let model: Box<dyn PhaseModel> = match scenario.phase_model {
        PhaseModelKind::PropagationIntegral => Box::new(PropagationIntegral::default()),
        PhaseModelKind::QuasiStatic => Box::new(QuasiStaticGradient::default()),
    };
    let sources: [&dyn SourceDynamics; 1] = [scenario.source.as_ref()];

    let mut time = Vec::new();
    let mut signal = Vec::new();
    let mut track = Vec::new();
    let mut vel = Vec::new();
    let mut acc = Vec::new();
    let mut orient = Vec::new();
    let mut angvel = Vec::new();
    let mut angacc = Vec::new();
    let mut mask = Vec::new();
    for &t in &scenario.schedule.times {
        // One phase per detector — the (T, D) signal row.
        let row: Vec<f64> = scenario
            .array
            .detectors
            .iter()
            .map(|det| model.delta_phi(&sources, det, t))
            .collect();
        let pose = scenario.source.pose_at(t);
        let m = scenario.source.motion_at(t);
        time.push(t);
        signal.push(row);
        track.push([pose.translation.x, pose.translation.y, pose.translation.z]);
        vel.push([m.velocity.x, m.velocity.y, m.velocity.z]);
        acc.push([m.acceleration.x, m.acceleration.y, m.acceleration.z]);
        let q = pose.rotation;
        orient.push([q.w, q.x, q.y, q.z]);
        angvel.push([
            m.angular_velocity.x,
            m.angular_velocity.y,
            m.angular_velocity.z,
        ]);
        angacc.push([
            m.angular_acceleration.x,
            m.angular_acceleration.y,
            m.angular_acceleration.z,
        ]);
        mask.push(false);
    }

    // Static shape descriptors, computed once from the body cloud iff requested.
    let shape = scenario
        .field_set
        .shape
        .then(|| gravity::inertia(scenario.source.body_cloud()));

    StateBundle {
        time,
        signal,
        source_position: vec![track],
        source_velocity: vec![vel],
        source_accel: vec![acc],
        source_orientation: vec![orient],
        source_angular_velocity: vec![angvel],
        source_angular_accel: vec![angacc],
        detector_placement: scenario.array.placements(),
        mask,
        meta: Meta {
            seed: scenario.seed,
            description: "M4 rotation spine".into(),
        },
        source_mass: shape.map(|r| vec![r.mass]),
        source_inertia: shape.map(|r| vec![r.i.m]),
        source_moments: shape.map(|r| vec![r.moments]),
        source_axes: shape.map(|r| vec![r.axes.m]),
        source_quadrupole: shape.map(|r| vec![r.q.m]),
    }
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
            DetectorArray::single(Detector::new(0.0)),
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
            DetectorArray::single(Detector::new(0.0)),
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
    #[allow(clippy::float_cmp)] // placement values and per-detector channels are exact/distinct by construction
    fn signal_td() {
        // signal is (T, D); detector_placement is (D, 7); per-detector channels are independent.
        let array = DetectorArray::new(vec![
            Detector::placed(Isometry3::new(Quat::identity(), Vec3::new(0.0, 0.0, 0.0))),
            Detector::placed(Isometry3::new(Quat::identity(), Vec3::new(8.0, 0.0, 0.0))),
            Detector::placed(Isometry3::new(Quat::identity(), Vec3::new(20.0, 0.0, 0.0))),
        ]);
        let scenario = Scenario::new(
            Box::new(point_source(1.0, 2.5)),
            array,
            Schedule::uniform(2.0, 4),
            1,
        );
        let bundle = run(&scenario);
        assert_eq!(bundle.signal.len(), 4); // T
        for row in &bundle.signal {
            assert_eq!(row.len(), 3); // D
        }
        assert_eq!(bundle.detector_placement.len(), 3); // (D, 7)
        assert_eq!(
            bundle.detector_placement[1],
            [8.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]
        );
        // Channels are independent: the three detectors read distinct phases (different ranges).
        let r0 = &bundle.signal[0];
        assert!(
            r0[0] != r0[1] && r0[1] != r0[2],
            "detector channels not independent"
        );
    }

    #[test]
    #[allow(clippy::float_cmp)] // distinct detectors read distinct phases — an exact inequality
    fn baseline_differential() {
        // Two detectors baseline b apart along x; the source approaches from +x (always beyond both).
        // ΔΦ_a ≠ ΔΦ_b, and |ΔΦ_a − ΔΦ_b| grows as the source nears the array — localisation's signature.
        let model = PropagationIntegral::default();
        let det_a = Detector::new(0.0); // at x = 0
        let det_b = Detector::placed(Isometry3::new(Quat::identity(), Vec3::new(2.0, 0.0, 0.0)));
        let mut prev = -1.0;
        for &range in &[40.0, 20.0, 10.0, 5.0] {
            // Source on the x-axis at `range` (> baseline), height at the gradiometer centre.
            let src = point_source(range, 2.5);
            let phi_a = model.delta_phi(&[&src], &det_a, 2.0);
            let phi_b = model.delta_phi(&[&src], &det_b, 2.0);
            assert!(phi_a != phi_b, "detectors read the same at range {range}");
            let diff = (phi_a - phi_b).abs();
            assert!(diff > prev, "differential did not grow at range {range}");
            prev = diff;
        }
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

    #[test]
    #[allow(clippy::float_cmp)] // non-zero check on an exact value
    fn phase_model_selected() {
        // The selector threads through run: QuasiStatic yields a finite signal, close to PI far-field.
        let build = |kind| {
            Scenario::new(
                Box::new(point_source(50.0, 2.5)), // far source (validity regime)
                DetectorArray::single(Detector::new(0.0)),
                Schedule::uniform(2.0, 1),
                0,
            )
            .with_phase_model(kind)
        };
        let pi = run(&build(PhaseModelKind::PropagationIntegral)).signal[0][0];
        let qs = run(&build(PhaseModelKind::QuasiStatic)).signal[0][0];
        assert!(qs.is_finite() && qs != 0.0);
        assert!(
            (pi - qs).abs() / pi.abs() <= 0.02,
            "selector: PI {pi} vs QS {qs}"
        );
    }
}
