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
    hash, uldm_phase, AtmoConfig, AtmoField, BodyMotion, Detector, DetectorArray, Dist, FieldSet,
    Key, KeyRng, NoiseSource, NoiseStack, Orient, Path, PhaseModel, PhaseModelKind, Prescribed,
    Prior, RunConfig, Scenario, Schedule, ShotNoise, Source, SourceDynamics, Timing, Trajectory,
    UldmConfig, VibrationResidual,
};
pub use state::{Dual, Isometry3, Mat3, Periodogram, Quat, Scalar, StateBundle, Vec3};

use instrument::{PropagationIntegral, QuasiStaticGradient};
use state::{frequency_grid, lomb_scargle, Meta};

/// Drive a scenario through the forward model and channels to a `StateBundle`.
///
/// `signal = targets + atmospheric + uldm + noise`. The atmospheric field is summed into the potential
/// in the forward pass (never post-hoc); ULDM is common-mode (once per tick, broadcast); noise is the
/// ordered post-hoc stack. With `field_set.decomposition` on, the gravitational phase is evaluated per
/// group (targets-only, atmo-only — the ≈2× cost) so each channel is recorded exactly by superposition.
pub fn run(scenario: &Scenario) -> StateBundle {
    let model: Box<dyn PhaseModel> = match scenario.phase_model {
        PhaseModelKind::PropagationIntegral => Box::new(PropagationIntegral::default()),
        PhaseModelKind::QuasiStatic => Box::new(QuasiStaticGradient::default()),
    };
    let pi = PropagationIntegral::default(); // reference single-IFO phases for signal_per_ifo
    let sources: [&dyn SourceDynamics; 1] = [scenario.source.as_ref()];
    // Atmospheric modes are drawn ONCE per scenario, then evaluated in every forward pass.
    let atmo = scenario.atmo.map(|c| AtmoField::realise(&c, scenario.seed));
    let fields: Vec<&dyn FieldContribution<f64>> = atmo
        .iter()
        .map(|a| a as &dyn FieldContribution<f64>)
        .collect();
    let decompose = scenario.field_set.decomposition;
    let d = scenario.array.detectors.len();

    let mut time = Vec::new();
    let mut signal = Vec::new();
    let (mut track, mut vel, mut acc) = (Vec::new(), Vec::new(), Vec::new());
    let (mut orient, mut angvel, mut angacc) = (Vec::new(), Vec::new(), Vec::new());
    let mut mask = Vec::new();
    // Channel accumulators — filled only when decomposing.
    let (mut targets_ch, mut atmo_ch, mut uldm_ch, mut per_ifo_ch) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());

    for (idx, &t) in scenario.schedule.times.iter().enumerate() {
        let mut row = Vec::with_capacity(d);
        let (mut trow, mut arow, mut pirow) = (Vec::new(), Vec::new(), Vec::new());
        for det in &scenario.array.detectors {
            let grav = if decompose {
                let ft = model.delta_phi(&sources, &[], det, t); // targets only
                let fa = model.delta_phi(&[], &fields, det, t); // atmo only (the 2nd pass)
                trow.push(ft);
                arow.push(fa);
                pirow.push(pi.per_ifo(&sources, &fields, det, t));
                ft + fa
            } else {
                model.delta_phi(&sources, &fields, det, t) // one combined pass
            };
            row.push(grav);
        }
        // ULDM: once per measurement, broadcast to every detector (common-mode).
        let u = scenario.uldm.map_or(0.0, |c| uldm_phase(&c, t));
        for x in row.iter_mut() {
            *x += u;
        }
        signal.push(row);
        if decompose {
            targets_ch.push(trow);
            atmo_ch.push(arow);
            uldm_ch.push(u);
            per_ifo_ch.push(pirow);
        }

        let pose = scenario.source.pose_at(t);
        let m = scenario.source.motion_at(t);
        time.push(t);
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
        mask.push(scenario.schedule.mask.get(idx).copied().unwrap_or(false));
    }

    // Post-hoc noise, realised (T, D) and added: signal = clean + noise (recoverable bit-for-bit).
    let noise = scenario
        .noise
        .realise(&scenario.schedule.times, d, scenario.seed);
    for (srow, nrow) in signal.iter_mut().zip(&noise) {
        for (s, n) in srow.iter_mut().zip(nrow) {
            *s += n;
        }
    }

    let shape = scenario
        .field_set
        .shape
        .then(|| gravity::inertia(scenario.source.body_cloud()));

    // Body-frame geometry, so the loaded path can pose and render the cloud without the scenario.
    let body = scenario.source.body_cloud();
    let source_cloud = vec![(0..body.len())
        .map(|i| [body.xs[i], body.ys[i], body.zs[i], body.ms[i]])
        .collect()];

    // Lomb–Scargle per detector on the (possibly non-uniform) measurement times — the correct
    // estimator when the schedule is gappy/jittered.
    let periodogram = scenario.field_set.periodogram.then(|| {
        let freqs = frequency_grid(&scenario.schedule.times);
        let power = (0..d)
            .map(|di| {
                let y: Vec<f64> = signal.iter().map(|row| row[di]).collect();
                lomb_scargle(&scenario.schedule.times, &y, &freqs)
            })
            .collect();
        Periodogram { freqs, power }
    });

    StateBundle {
        time,
        signal,
        source_position: vec![track],
        source_velocity: vec![vel],
        source_accel: vec![acc],
        source_orientation: vec![orient],
        source_angular_velocity: vec![angvel],
        source_angular_accel: vec![angacc],
        source_cloud,
        detector_placement: scenario.array.placements(),
        mask,
        meta: Meta {
            seed: scenario.seed,
            description: "M5 channels spine".into(),
        },
        source_mass: shape.map(|r| vec![r.mass]),
        source_inertia: shape.map(|r| vec![r.i.m]),
        source_moments: shape.map(|r| vec![r.moments]),
        source_axes: shape.map(|r| vec![r.axes.m]),
        source_quadrupole: shape.map(|r| vec![r.q.m]),
        signal_noise: noise,
        signal_uldm: decompose.then_some(uldm_ch),
        signal_targets: decompose.then_some(targets_ch),
        signal_atmospheric: decompose.then_some(atmo_ch),
        signal_per_ifo: decompose.then_some(per_ifo_ch),
        periodogram,
    }
}

/// Stream `n` scenarios sampled from `prior` (scenario `i` keyed `scenario[i]` off `root`), yielding
/// one `StateBundle` at a time. **Memory-bounded**: scenarios are processed in `cfg.batch` chunks and
/// each bundle is dropped once yielded, so at most one batch is resident — never the whole run.
/// `stream(prior, n, root, cfg)` and running `prior.sample(scenario_key(root, i))` directly agree
/// bundle-for-bundle (batch-invariance), because a scenario's key hangs off its index, not its batch.
pub fn stream(
    prior: &Prior,
    n: usize,
    root: u64,
    cfg: RunConfig,
) -> impl Iterator<Item = StateBundle> + '_ {
    let batch = cfg.batch.max(1);
    (0..n).step_by(batch).flat_map(move |start| {
        let end = (start + batch).min(n);
        (start..end)
            .map(|i| run(&prior.sample(scenario_key(root, i))))
            .collect::<Vec<_>>()
            .into_iter()
    })
}

/// The seed for batch item `i`: the `scenario[i]` node of the key tree off `root`. The single source
/// of truth for how a batch position maps to a scenario seed, so "alone" and "in a batch" agree.
pub fn scenario_key(root: u64, i: usize) -> u64 {
    Key::root(root).child("scenario").index(i as u64).bits()
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
        let dphi_above = model.delta_phi(&[&above], &[], &det, 2.0);
        let dphi_below = model.delta_phi(&[&below], &[], &det, 2.0);
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
    #[allow(clippy::float_cmp)] // source_cloud is copied verbatim from the body cloud — exact by construction
    fn source_cloud_matches_body() {
        // The bundle carries the source's body-frame geometry verbatim, so the loaded path can pose
        // and render the cloud without the scenario in hand.
        let elements = [
            (0.1, 0.2, 0.3, 500.0),
            (-0.1, -0.2, -0.3, 250.0),
            (0.5, 0.0, 1.0, 100.0),
        ];
        let traj = Trajectory::new(
            Isometry3::identity(),
            Path::Static,
            Timing::Uniform { rate: 0.0 },
        );
        let scn = Scenario::new(
            Box::new(Source::new(Cloud::from_elements(&elements), traj)),
            DetectorArray::single(Detector::new(0.0)),
            Schedule::uniform(2.0, 3),
            1,
        );
        let cloud = run(&scn).source_cloud;
        assert_eq!(cloud.len(), 1, "one source at v1");
        assert_eq!(cloud[0].len(), elements.len());
        for (got, e) in cloud[0].iter().zip(&elements) {
            assert_eq!(*got, [e.0, e.1, e.2, e.3]);
        }
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
            let phi_a = model.delta_phi(&[&src], &[], &det_a, 2.0);
            let phi_b = model.delta_phi(&[&src], &[], &det_b, 2.0);
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
            PropagationIntegral::new(cfg).delta_phi(&[&src], &[], &det, 2.0)
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
