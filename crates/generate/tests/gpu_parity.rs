//! M6a CPU≡GPU parity. `cpu_backend_matches_run` pins `CpuBackend` as the bit-exact f64 oracle (no
//! GPU — runs in the blocking gate). `cpu_equals_gpu` (Commit 3, `#[ignore]`) compares the oracle
//! against the f32 GPU path on the anchors, run only in the GPU CI job / locally on Metal.

use compute::{ComputeBackend, CpuBackend, EvalBatch, ScenarioBatch, SourceBatch};
use generate::{
    run, Detector, DetectorArray, Isometry3, Orient, Path, Quat, Scenario, Schedule, Source,
    Timing, Trajectory, Vec3,
};
use instrument::{InstrumentConfig, PhaseModelKind};
use shape::{voxelise, Cuboid, MassSpec, VoxelParams};

fn wall() -> gravity::Cloud {
    voxelise(
        &Cuboid {
            half: [0.1, 0.5, 1.0],
        },
        &VoxelParams::pitch(0.1),
        MassSpec::Total(1000.0),
    )
    .unwrap()
}

#[test]
fn cpu_backend_matches_run() {
    // A cloud-only anchor (no atmo/uldm/noise, decomposition off): CpuBackend's ΔΦ equals
    // generate::run's ΔΦ bit-for-bit — the oracle inherits M1–M5's validation.
    let cloud = wall();
    let placement = Isometry3::new(Quat::identity(), Vec3::new(3.0, 0.0, 2.5));
    let path = Path::Static;
    let timing = Timing::Uniform { rate: 0.0 };
    let orient = Orient::Fixed(Quat::identity());
    let dets = vec![
        Detector::new(0.0),
        Detector::placed(Isometry3::new(Quat::identity(), Vec3::new(2.0, 0.0, 0.0))),
    ];
    let sched = Schedule::uniform(2.0, 4);

    // generate::run path (decomposition off by default → combined ΔΦ, no channels).
    let traj = Trajectory::new(placement, path, timing).with_orient(orient);
    let scn = Scenario::new(
        Box::new(Source::new(cloud.clone(), traj)),
        DetectorArray::new(dets.clone()),
        sched.clone(),
        0,
    );
    let bundle = run(&scn);

    // CpuBackend path from the same parameters.
    let batch = EvalBatch {
        scenarios: vec![ScenarioBatch {
            sources: vec![SourceBatch {
                cloud,
                placement,
                path,
                timing,
                orient,
            }],
            atmo: Vec::new(),
            detectors: dets.iter().map(|d| d.placement).collect(),
            times: sched.times.clone(),
        }],
        instrument: InstrumentConfig::default(),
        phase_model: PhaseModelKind::PropagationIntegral,
    };
    let sig = CpuBackend.evaluate(&batch).unwrap();

    // bundle.signal is [t][d]; sig.dphi[0] is [d][t].
    for ti in 0..sched.times.len() {
        for di in 0..dets.len() {
            assert_eq!(
                bundle.signal[ti][di], sig.dphi[0][di][ti],
                "CpuBackend != run at (t={ti}, d={di})"
            );
        }
    }
}
