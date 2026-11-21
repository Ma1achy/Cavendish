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
    hash, uldm_phase, Ambient, AtmoConfig, AtmoField, BodyMotion, Channel, Detector, DetectorArray,
    Dist, Eval, FieldSet, Key, KeyRng, NoiseSource, NoiseStack, Orient, Path, PhaseModel,
    PhaseModelKind, Prescribed, Prior, Rigid, RunConfig, Scenario, Schedule, ShotNoise, Source,
    SourceDynamics, Timing, Track, Trajectory, UldmConfig, VibrationResidual, WorldSource,
};
pub use state::{Dual, Isometry3, Mat3, Periodogram, Quat, Scalar, StateBundle, Vec3};

use instrument::{PropagationIntegral, QuasiStaticGradient};
use state::{frequency_grid, lomb_scargle, Meta};

/// The scene's sources routed to the forward model's sockets, per decomposition channel.
#[derive(Default)]
struct Routed<'a> {
    rigid: Vec<&'a dyn SourceDynamics>,
    fields: Vec<&'a dyn FieldContribution<f64>>,
    /// Sources evaluated through their exact linear-response reformulation instead of quadrature.
    fast: Vec<&'a dyn WorldSource>,
}

impl Routed<'_> {
    /// The two single-interferometer phases contributed by the fast-path sources.
    fn fast_per_ifo(&self, pi: &PropagationIntegral, det: &Detector, t: f64) -> Option<[f64; 2]> {
        if self.fast.is_empty() {
            return None; // nothing to add — keeps a v1 scene's arithmetic bit-identical
        }
        let mut acc = [0.0; 2];
        for s in &self.fast {
            let [d1, d2] = s
                .per_ifo_fast(&pi.cfg, det, t)
                .expect("a source that offers a fast path offers it at every measurement");
            acc[0] += d1;
            acc[1] += d2;
        }
        Some(acc)
    }
}

/// Drive a scenario through the forward model and channels to a `StateBundle`.
///
/// `signal = targets + atmospheric + uldm + noise`. Every source enters through the one
/// `WorldSource` seam and is routed to the phase integral's rigid or field socket; ambient sources
/// (air, structure) and the analytic atmospheric field are summed in the forward pass (never
/// post-hoc); ULDM is common-mode (once per tick, broadcast); noise is the ordered post-hoc stack.
/// With `field_set.decomposition` on, the gravitational phase is evaluated per channel
/// (targets-only, ambient-only — the ≈2× cost) so each is recorded exactly by superposition.
pub fn run(scenario: &Scenario) -> StateBundle {
    let model: Box<dyn PhaseModel> = match scenario.phase_model {
        PhaseModelKind::PropagationIntegral => Box::new(PropagationIntegral::default()),
        PhaseModelKind::QuasiStatic => Box::new(QuasiStaticGradient::default()),
    };
    let pi = PropagationIntegral::default(); // reference single-IFO phases for signal_per_ifo
                                             // Atmospheric modes are drawn ONCE per scenario, then evaluated in every forward pass.
    let atmo = scenario.atmo.map(|c| AtmoField::realise(&c, scenario.seed));
    let decompose = scenario.field_set.decomposition;
    let d = scenario.array.detectors.len();

    // Route each source to its socket. A linear-response fast path reformulates the propagation
    // integral, so it is only taken when that is the phase model in use.
    let fast_ok = scenario.phase_model == PhaseModelKind::PropagationIntegral;
    let probe = scenario
        .array
        .detectors
        .first()
        .zip(scenario.schedule.times.first());
    let (mut targets, mut ambient) = (Routed::default(), Routed::default());
    for src in &scenario.sources {
        let src: &dyn WorldSource = src.as_ref();
        let group = match src.channel() {
            Channel::Target => &mut targets,
            Channel::Ambient => &mut ambient,
        };
        match src.eval() {
            Eval::Rigid(r) => group.rigid.push(r),
            Eval::Field(f) => {
                let fast = fast_ok
                    && probe.is_some_and(|(det, &t)| src.per_ifo_fast(&pi.cfg, det, t).is_some());
                if fast {
                    group.fast.push(src);
                } else {
                    group.fields.push(f);
                }
            }
        }
    }
    if let Some(a) = &atmo {
        ambient.fields.insert(0, a);
    }
    // The combined pass: every rigid source, then every field — one forward evaluation.
    let all_rigid: Vec<&dyn SourceDynamics> = targets
        .rigid
        .iter()
        .chain(&ambient.rigid)
        .copied()
        .collect();
    let all_fields: Vec<&dyn FieldContribution<f64>> = targets
        .fields
        .iter()
        .chain(&ambient.fields)
        .copied()
        .collect();
    let double = |x: Option<[f64; 2]>| x.map_or(0.0, |[d1, d2]| d2 - d1);

    let tracked: Vec<&dyn WorldSource> = scenario
        .sources
        .iter()
        .map(|s| s.as_ref())
        .filter(|s| s.body_cloud().is_some())
        .collect();
    let n_src = tracked.len();

    let mut time = Vec::new();
    let mut signal = Vec::new();
    let mut track = vec![Vec::new(); n_src];
    let (mut vel, mut acc) = (vec![Vec::new(); n_src], vec![Vec::new(); n_src]);
    let (mut orient, mut angvel, mut angacc) = (
        vec![Vec::new(); n_src],
        vec![Vec::new(); n_src],
        vec![Vec::new(); n_src],
    );
    let mut mask = Vec::new();
    // Channel accumulators — filled only when decomposing.
    let (mut targets_ch, mut atmo_ch, mut uldm_ch, mut per_ifo_ch) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());

    for (idx, &t) in scenario.schedule.times.iter().enumerate() {
        let mut row = Vec::with_capacity(d);
        let (mut trow, mut arow, mut pirow) = (Vec::new(), Vec::new(), Vec::new());
        for det in &scenario.array.detectors {
            let fast_t = targets.fast_per_ifo(&pi, det, t);
            let fast_a = ambient.fast_per_ifo(&pi, det, t);
            let grav = if decompose {
                let mut ft = model.delta_phi(&targets.rigid, &targets.fields, det, t);
                let mut fa = model.delta_phi(&ambient.rigid, &ambient.fields, det, t); // the 2nd pass
                let mut both = pi.per_ifo(&all_rigid, &all_fields, det, t);
                if fast_t.is_some() {
                    ft += double(fast_t);
                }
                if fast_a.is_some() {
                    fa += double(fast_a);
                }
                for fast in [fast_t, fast_a].into_iter().flatten() {
                    both[0] += fast[0];
                    both[1] += fast[1];
                }
                trow.push(ft);
                arow.push(fa);
                pirow.push(both);
                ft + fa
            } else {
                let mut g = model.delta_phi(&all_rigid, &all_fields, det, t); // one combined pass
                if fast_t.is_some() || fast_a.is_some() {
                    g += double(fast_t) + double(fast_a);
                }
                g
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

        time.push(t);
        for (s, src) in tracked.iter().enumerate() {
            let tr = src
                .track_at(t)
                .expect("a source with a body cloud reports a track");
            let (pose, m) = (tr.pose, tr.motion);
            track[s].push([pose.translation.x, pose.translation.y, pose.translation.z]);
            vel[s].push([m.velocity.x, m.velocity.y, m.velocity.z]);
            acc[s].push([m.acceleration.x, m.acceleration.y, m.acceleration.z]);
            let q = pose.rotation;
            orient[s].push([q.w, q.x, q.y, q.z]);
            angvel[s].push([
                m.angular_velocity.x,
                m.angular_velocity.y,
                m.angular_velocity.z,
            ]);
            angacc[s].push([
                m.angular_acceleration.x,
                m.angular_acceleration.y,
                m.angular_acceleration.z,
            ]);
        }
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

    // Body-frame geometry per tracked source, so the loaded path can pose and render each cloud
    // without the scenario; and (optionally) its second-moment descriptors.
    let bodies: Vec<&gravity::Cloud> = tracked.iter().filter_map(|s| s.body_cloud()).collect();
    let shape: Option<Vec<gravity::Inertia>> = scenario
        .field_set
        .shape
        .then(|| bodies.iter().map(|b| gravity::inertia(b)).collect());
    let source_cloud = bodies
        .iter()
        .map(|body| {
            (0..body.len())
                .map(|i| [body.xs[i], body.ys[i], body.zs[i], body.ms[i]])
                .collect()
        })
        .collect();

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
        source_position: track,
        source_velocity: vel,
        source_accel: acc,
        source_orientation: orient,
        source_angular_velocity: angvel,
        source_angular_accel: angacc,
        source_cloud,
        detector_placement: scenario.array.placements(),
        mask,
        meta: Meta {
            seed: scenario.seed,
            description: "M5 channels spine".into(),
        },
        source_mass: shape.as_ref().map(|v| v.iter().map(|r| r.mass).collect()),
        source_inertia: shape.as_ref().map(|v| v.iter().map(|r| r.i.m).collect()),
        source_moments: shape
            .as_ref()
            .map(|v| v.iter().map(|r| r.moments).collect()),
        source_axes: shape.as_ref().map(|v| v.iter().map(|r| r.axes.m).collect()),
        source_quadrupole: shape.as_ref().map(|v| v.iter().map(|r| r.q.m).collect()),
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

/// Run `n` scenes built by `build(i, key)` (item `i` keyed `scenario[i]` off `root`), across every
/// available core, returning the bundles in index order. Unlike [`stream`] the scenes need not come
/// from a `Prior` — `build` may assemble anything, including a physics simulation. Each scene is
/// built and run entirely on one thread and depends only on its own key, so the result is identical
/// for any thread count.
pub fn batch<F>(n: usize, root: u64, build: F) -> Vec<StateBundle>
where
    F: Fn(usize, u64) -> Scenario + Sync,
{
    let threads = std::thread::available_parallelism()
        .map_or(1, |t| t.get())
        .min(n.max(1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut out: Vec<Option<StateBundle>> = (0..n).map(|_| None).collect();
    let done: Vec<Vec<(usize, StateBundle)>> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if i >= n {
                            break mine;
                        }
                        mine.push((i, run(&build(i, scenario_key(root, i)))));
                    }
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|w| w.join().expect("a scene panicked"))
            .collect()
    });
    for (i, bundle) in done.into_iter().flatten() {
        out[i] = Some(bundle);
    }
    out.into_iter()
        .map(|b| b.expect("every index was run"))
        .collect()
}

/// Write a batch of same-shaped bundles as stacked `.npy` arrays under `dir` (created if absent),
/// leading axis the batch `B`: `signal`, `signal_noise` `(B, T, D)`; `source_position`,
/// `source_velocity` `(B, S, T, 3)`; `time` `(B, T)`; `detector_placement` `(D, 7)`; and, where the
/// bundles carry them, `signal_targets`, `signal_atmospheric` `(B, T, D)` and `source_mass` `(B, S)`.
/// The whole record, no input/label split — what is an input and what is a target is the consumer's
/// choice.
pub fn write_dataset(
    dir: impl AsRef<std::path::Path>,
    bundles: &[StateBundle],
) -> std::io::Result<()> {
    let dir = dir.as_ref();
    let first = bundles
        .first()
        .ok_or_else(|| std::io::Error::other("no bundles to write"))?;
    let (b, t, d) = (
        bundles.len(),
        first.time.len(),
        first.detector_placement.len(),
    );
    let s = first.source_position.len();
    let same = |x: &StateBundle| {
        x.time.len() == t && x.detector_placement.len() == d && x.source_position.len() == s
    };
    if !bundles.iter().all(same) {
        return Err(std::io::Error::other("bundles differ in shape (T, D or S)"));
    }
    std::fs::create_dir_all(dir)?;
    let td = |get: &dyn Fn(&StateBundle) -> &Vec<Vec<f64>>| -> Vec<f64> {
        bundles
            .iter()
            .flat_map(|x| get(x).iter().flatten().copied())
            .collect()
    };
    let st3 = |get: &dyn Fn(&StateBundle) -> &Vec<Vec<[f64; 3]>>| -> Vec<f64> {
        bundles
            .iter()
            .flat_map(|x| get(x).iter().flatten().flatten().copied())
            .collect()
    };
    let save = |name: &str, shape: &[usize], data: Vec<f64>| {
        state::save_npy(dir.join(format!("{name}.npy")), shape, &data)
    };
    save("signal", &[b, t, d], td(&|x| &x.signal))?;
    save("signal_noise", &[b, t, d], td(&|x| &x.signal_noise))?;
    save(
        "time",
        &[b, t],
        bundles
            .iter()
            .flat_map(|x| x.time.iter().copied())
            .collect(),
    )?;
    save(
        "source_position",
        &[b, s, t, 3],
        st3(&|x| &x.source_position),
    )?;
    save(
        "source_velocity",
        &[b, s, t, 3],
        st3(&|x| &x.source_velocity),
    )?;
    save(
        "detector_placement",
        &[d, 7],
        first.detector_placement.iter().flatten().copied().collect(),
    )?;
    if bundles
        .iter()
        .all(|x| x.signal_targets.is_some() && x.signal_atmospheric.is_some())
    {
        save(
            "signal_targets",
            &[b, t, d],
            td(&|x| x.signal_targets.as_ref().unwrap()),
        )?;
        save(
            "signal_atmospheric",
            &[b, t, d],
            td(&|x| x.signal_atmospheric.as_ref().unwrap()),
        )?;
    }
    if bundles.iter().all(|x| x.source_mass.is_some()) {
        let mass = bundles
            .iter()
            .flat_map(|x| x.source_mass.clone().unwrap())
            .collect();
        save("source_mass", &[b, s], mass)?;
    }
    Ok(())
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
