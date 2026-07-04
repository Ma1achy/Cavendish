//! M6a CPU≡GPU parity. `cpu_backend_matches_run` pins `CpuBackend` as the bit-exact f64 oracle (no
//! GPU — runs in the blocking gate). `cpu_equals_gpu` (Commit 3, `#[ignore]`) compares the oracle
//! against the f32 GPU path on the anchors, run only in the GPU CI job / locally on Metal.

use compute::{
    AtmoContribution, AtmoMode, ComputeBackend, CpuBackend, EvalBatch, Gpu, ScenarioBatch,
    SourceBatch, WgpuBackend, FORWARD_WGSL,
};
use generate::{
    run, AtmoConfig, AtmoField, Detector, DetectorArray, Isometry3, Orient, Path, Quat, Scenario,
    Schedule, Source, SourceDynamics, Timing, Trajectory, Vec3,
};
use gravity::FieldContribution;
use instrument::{InstrumentConfig, PhaseModelKind};
use shape::{voxelise, Cuboid, MassSpec, Sphere, VoxelParams};

fn cuboid(half: [f64; 3], mass: f64) -> gravity::Cloud {
    voxelise(
        &Cuboid { half },
        &VoxelParams::pitch(0.1),
        MassSpec::Total(mass),
    )
    .unwrap()
}

fn sphere(r: f64, mass: f64) -> gravity::Cloud {
    voxelise(
        &Sphere { r },
        &VoxelParams::pitch(0.05),
        MassSpec::Total(mass),
    )
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn one_source(
    cloud: gravity::Cloud,
    placement: Isometry3,
    path: Path,
    timing: Timing,
    orient: Orient,
    atmo: Vec<AtmoMode>,
    dets: Vec<Isometry3>,
    times: Vec<f64>,
) -> ScenarioBatch {
    ScenarioBatch {
        sources: vec![SourceBatch {
            cloud,
            placement,
            path,
            timing,
            orient,
        }],
        atmo,
        detectors: dets,
        times,
    }
}

#[test]
fn atmo_reexpression_matches() {
    // compute::AtmoContribution re-expresses noise::AtmoField from POD modes — bit-identical on the
    // same modes, so the GPU's CPU oracle inherits M5's atmospheric validation (no GPU needed).
    let field = AtmoField::realise(
        &AtmoConfig {
            n_modes: 16,
            correlation_length: 40.0,
            amplitude: 1.0,
            sound_speed: 343.0,
        },
        3,
    );
    let modes: Vec<AtmoMode> = field
        .mode_params()
        .into_iter()
        .map(|(k, omega, psi, coeff)| AtmoMode {
            k,
            omega,
            psi,
            coeff,
        })
        .collect();
    let contrib = AtmoContribution { modes };
    for &(x, y, z, t) in &[(1.0, 2.0, 3.0, 0.5), (-2.0, 0.5, 4.0, 1.7)] {
        let p = Vec3::new(x, y, z);
        let a = FieldContribution::<f64>::potential(&field, p, t);
        let b = FieldContribution::<f64>::potential(&contrib, p, t);
        assert!(
            (a - b).abs() <= 1e-15 * a.abs().max(1e-30),
            "atmo re-expression: {a:e} vs {b:e}"
        );
    }
}

#[test]
#[ignore = "requires a GPU device (run in the gpu CI job / locally on Metal)"]
fn cpu_equals_gpu() {
    // The f64 oracle (CpuBackend) vs the f32 GPU path (WgpuBackend) on the anchors — ΔΦ to ≤1e-4 of
    // the signal scale (a coincidentally-small ΔΦ at a zero-crossing needn't match to 1e-4 of itself).
    let gpu = WgpuBackend::new().expect("device");
    let cfg = InstrumentConfig::default();
    let anchor = |name: &str, scn: ScenarioBatch| -> f64 {
        let batch = EvalBatch {
            scenarios: vec![scn],
            instrument: cfg,
            phase_model: PhaseModelKind::PropagationIntegral,
        };
        let cpu = CpuBackend.evaluate(&batch).unwrap();
        let g = gpu.evaluate(&batch).unwrap();
        let scale = cpu.dphi[0]
            .iter()
            .flatten()
            .fold(0.0f64, |a, &x| a.max(x.abs()))
            .max(1e-30);
        let mut worst = 0.0f64;
        for (dc, dg) in cpu.dphi[0].iter().zip(&g.dphi[0]) {
            for (&a, &b) in dc.iter().zip(dg) {
                worst = worst.max((a - b).abs() / scale);
            }
        }
        eprintln!("cpu_equals_gpu: {name:<12} rel {worst:.2e}");
        worst
    };

    let d0 = vec![Isometry3::identity()];
    let ident = || Isometry3::identity();
    let at = |x: f64, z: f64| Isometry3::new(Quat::identity(), Vec3::new(x, 0.0, z));

    // wall (Static cuboid)
    let wall = anchor(
        "wall",
        one_source(
            cuboid([0.1, 0.5, 1.0], 1000.0),
            at(3.0, 2.5),
            Path::Static,
            Timing::Uniform { rate: 0.0 },
            Orient::Fixed(Quat::identity()),
            vec![],
            d0.clone(),
            vec![0.0, 2.0, 4.0],
        ),
    );
    // moving mass (LinearPass sphere, vertical pass at x = 10)
    let moving = anchor(
        "moving",
        one_source(
            sphere(0.1, 10.0),
            at(10.0, 0.0),
            Path::LinearPass {
                a: Vec3::new(0.0, 0.0, -10.0),
                b: Vec3::new(0.0, 0.0, 10.0),
            },
            Timing::Uniform { rate: 0.125 },
            Orient::Fixed(Quat::identity()),
            vec![],
            d0.clone(),
            vec![2.0, 3.0, 4.0],
        ),
    );
    // oscillation (Oscillation path)
    let oscillation = anchor(
        "oscillation",
        one_source(
            cuboid([0.3, 0.3, 0.6], 100.0),
            at(5.0, 2.5),
            Path::Oscillation {
                axis: Vec3::new(0.0, 0.0, 1.0),
                amp: 0.5,
                freq: 0.1,
                phase: 0.0,
            },
            Timing::Uniform { rate: 1.0 },
            Orient::Fixed(Quat::identity()),
            vec![],
            d0.clone(),
            vec![1.0, 2.0, 3.0],
        ),
    );
    // rotating (FreeRotation cuboid, ≤1.5 s horizon so the f32 pose parity stays tight)
    let rotating = anchor(
        "rotating",
        one_source(
            cuboid([0.3, 0.5, 0.7], 100.0),
            at(3.0, 2.5),
            Path::Static,
            Timing::Uniform { rate: 0.0 },
            Orient::FreeRotation {
                omega0: Vec3::new(0.4, 0.1, 0.6),
            },
            vec![],
            d0.clone(),
            vec![0.5, 1.0, 1.5],
        ),
    );
    // decomposed (static source + atmospheric field on a 2-detector array)
    let field = AtmoField::realise(
        &AtmoConfig {
            n_modes: 8,
            correlation_length: 50.0,
            amplitude: 1.0,
            sound_speed: 343.0,
        },
        7,
    );
    let modes: Vec<AtmoMode> = field
        .mode_params()
        .into_iter()
        .map(|(k, omega, psi, coeff)| AtmoMode {
            k,
            omega,
            psi,
            coeff,
        })
        .collect();
    let decomposed = anchor(
        "decomposed",
        one_source(
            cuboid([0.2, 0.5, 0.4], 100.0),
            at(3.0, 2.5),
            Path::Static,
            Timing::Uniform { rate: 0.0 },
            Orient::Fixed(Quat::identity()),
            modes,
            vec![ident(), at(2.0, 0.0)],
            vec![0.0, 1.0, 2.0],
        ),
    );

    for r in [wall, moving, oscillation, rotating, decomposed] {
        assert!(r <= 1e-4, "GPU off the CPU oracle: {r:.2e}");
    }
}

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

fn principal_moments(cloud: &gravity::Cloud) -> [f64; 3] {
    let m = &gravity::inertia(cloud).i.m; // body-frame inertia diagonal (principal frame at v1)
    [m[0][0], m[1][1], m[2][2]]
}

#[test]
#[ignore = "requires a GPU device (run in the gpu CI job / locally on Metal)"]
fn pass1_ode_on_device() {
    // The GPU-integrated free-rotation quaternion vs the CPU integrator (via Source::pose_at), ≤1e-5
    // over a 200-substep (2 s) horizon. The scheme is structure-preserving; the residual is pure f32
    // accumulation, which grows with step count (≈1.7e-5 by 300 steps) — that longer horizon is still
    // well within cpu_equals_gpu's ≤1e-4, the bound the whole pipeline is held to.
    let cloud = voxelise(
        &Cuboid {
            half: [0.3, 0.5, 0.7],
        },
        &VoxelParams::pitch(0.1),
        MassSpec::Total(10.0),
    )
    .unwrap();
    let moments = principal_moments(&cloud);
    let omega0 = Vec3::new(0.4, 0.1, 0.6);
    let traj = Trajectory::new(
        Isometry3::identity(),
        Path::Static,
        Timing::Uniform { rate: 0.0 },
    )
    .with_orient(Orient::FreeRotation { omega0 });
    let source = Source::new(cloud, traj);
    let gpu = Gpu::new().expect("device");
    let fine_dt = 0.01_f64;

    let mut worst = 0.0_f64;
    for &t in &[0.5, 1.0, 1.5, 2.0] {
        let q_ref = source.pose_at(t).rotation; // identity placement ⇒ the integrated quaternion
        let pp = [
            omega0.x as f32,
            omega0.y as f32,
            omega0.z as f32,
            moments[0] as f32,
            moments[1] as f32,
            moments[2] as f32,
            t as f32,
            fine_dt as f32,
        ];
        let g = gpu.run_kernel(FORWARD_WGSL, "k_free_rotation_pose", &[], &pp, 4);
        // Sign-align (quaternion double cover), then compare components.
        let dot = q_ref.w * g[0] as f64
            + q_ref.x * g[1] as f64
            + q_ref.y * g[2] as f64
            + q_ref.z * g[3] as f64;
        let s = if dot < 0.0 { -1.0 } else { 1.0 };
        let d = [
            (q_ref.w - s * g[0] as f64).abs(),
            (q_ref.x - s * g[1] as f64).abs(),
            (q_ref.y - s * g[2] as f64).abs(),
            (q_ref.z - s * g[3] as f64).abs(),
        ];
        let dt = d.into_iter().fold(0.0, f64::max);
        eprintln!(
            "pass1_ode_on_device: t={t} |Δq| {dt:.2e} ({} steps)",
            (t / fine_dt) as usize
        );
        worst = worst.max(dt);
    }
    eprintln!("pass1_ode_on_device: worst |Δq| {worst:.2e}");
    assert!(
        worst <= 1e-5,
        "GPU free-rotation pose off the CPU integrator: {worst:.2e}"
    );
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
