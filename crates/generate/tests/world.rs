//! The unified source seam, end to end: scenes of several sources of several kinds through
//! `generate::run`, checked by superposition, by the rigid null limit, and against the propagation
//! integral evaluated directly.

use generate::{
    batch, run, write_dataset, Ambient, Detector, DetectorArray, FieldSet, Isometry3, NoiseStack,
    Path, PhaseModel, PhaseModelKind, Prescribed, Quat, Rigid, Scenario, Schedule, ShotNoise,
    Source, Timing, Trajectory, UldmConfig, Vec3, WorldSource,
};
use gravity::Cloud;
use instrument::PropagationIntegral;
use physics::{
    simulate_air, Collider, Grid3, GridField, Obstacle, Parcels, RigidBody, RigidWorld, Room,
    Walker,
};
use shape::{voxelise, Cuboid, MassSpec, Sphere, VoxelParams};

fn array() -> DetectorArray {
    DetectorArray::new(vec![
        Detector::new(0.0),
        Detector::placed(Isometry3::new(Quat::identity(), Vec3::new(5.0, 0.0, 0.0))),
    ])
}

fn ball(mass: f64) -> Cloud {
    voxelise(
        &Sphere { r: 0.25 },
        &VoxelParams::pitch(0.1),
        MassSpec::Total(mass),
    )
    .unwrap()
}

fn pass(cloud: Cloud, x: f64) -> Source {
    let place = Isometry3::new(Quat::identity(), Vec3::new(x, 0.0, 2.0));
    Source::new(
        cloud,
        Trajectory::new(
            place,
            Path::LinearPass {
                a: Vec3::new(0.0, -3.0, 0.0),
                b: Vec3::new(0.0, 3.0, 0.0),
            },
            Timing::Uniform { rate: 0.1 },
        ),
    )
}

fn decomposed() -> FieldSet {
    FieldSet {
        shape: true,
        decomposition: true,
        periodogram: false,
    }
}

/// A blob of air-density anomaly drifting upward — a stand-in for a plume, cheap to build.
fn drifting_air(t0: f64, t1: f64) -> GridField {
    let grid = Grid3::new([1.0, -0.6, 0.4], 0.3, [4, 4, 10]);
    let dt = 0.25;
    let frames = (0..=((t1 - t0) / dt).ceil() as usize)
        .map(|f| {
            let zc = 0.8 + 0.2 * (t0 + f as f64 * dt);
            let mut m = vec![0.0; grid.len()];
            for k in 0..grid.n[2] {
                for j in 0..grid.n[1] {
                    for i in 0..grid.n[0] {
                        let c = grid.centre(i, j, k);
                        let r2 = (c[0] - 1.6).powi(2) + c[1] * c[1] + (c[2] - zc).powi(2);
                        m[grid.idx(i, j, k)] = -3e-3 * (-r2 / 0.4).exp();
                    }
                }
            }
            m
        })
        .collect();
    GridField::new(&grid, t0, dt, frames, generate::Channel::Ambient)
}

#[test]
fn sources_superpose() {
    // Gravity is linear: a scene of two bodies reads as the sum of each alone.
    let schedule = Schedule::uniform(2.0, 6);
    let one =
        |x: f64, m: f64| Scenario::new(Box::new(pass(ball(m), x)), array(), schedule.clone(), 3);
    let both = Scenario::scene(
        vec![
            Box::new(Rigid(Box::new(pass(ball(300.0), 2.0)))),
            Box::new(Rigid(Box::new(pass(ball(500.0), 3.5)))),
        ],
        array(),
        schedule.clone(),
        3,
    );
    let (a, b, ab) = (run(&one(2.0, 300.0)), run(&one(3.5, 500.0)), run(&both));
    assert_eq!(
        ab.source_position.len(),
        2,
        "two tracked sources on the S axis"
    );
    for t in 0..6 {
        for d in 0..2 {
            let want = a.signal[t][d] + b.signal[t][d];
            assert!(
                (ab.signal[t][d] - want).abs() <= 1e-12 * want.abs(),
                "t={t} d={d}"
            );
        }
    }
}

#[test]
fn rigid_parcels_reproduce_the_rigid_source() {
    // The null limit through the whole pipeline: a body as transported parcels under a rigid map
    // gives the v1 rigid source's signal to rounding.
    let schedule = Schedule::uniform(2.0, 5);
    let rigid = pass(ball(400.0), 2.5);
    let traj_pose = {
        let r = pass(ball(400.0), 2.5);
        move |t: f64| generate::SourceDynamics::pose_at(&r, t)
    };
    let v1 = run(&Scenario::new(
        Box::new(rigid),
        array(),
        schedule.clone(),
        1,
    ));
    let v2 = run(&Scenario::scene(
        vec![Box::new(Parcels::rigid(ball(400.0), traj_pose))],
        array(),
        schedule,
        1,
    ));
    for (ra, rb) in v1.signal.iter().zip(&v2.signal) {
        for (a, b) in ra.iter().zip(rb) {
            assert!(
                (a - b).abs() <= 1e-11 * a.abs(),
                "v1 {a:e} vs parcels {b:e}"
            );
        }
    }
    for (pa, pb) in v1.source_position[0].iter().zip(&v2.source_position[0]) {
        assert_eq!(pa, pb, "the track is the same pose");
    }
}

#[test]
fn mixed_scene_decomposes_exactly() {
    // A rigid target, a walking figure, drifting air, and a swaying wall: the recorded channels sum
    // to the signal, the air and the wall land in the ambient channel, and the grid field's
    // fast path agrees with quadrature through the propagation integral.
    let schedule = Schedule::uniform(1.5, 5);
    let (t0, t1) = (-2.0, 7.0);
    let walker = Walker {
        mass: 70.0,
        height: 1.75,
        pitch: 0.1,
        start: Vec3::new(2.5, -3.0, 0.0),
        velocity: Vec3::new(0.0, 1.0, 0.0),
        swing: 0.5,
        cadence: 0.9,
    };
    let wall = || {
        let slab = voxelise(
            &Cuboid {
                half: [0.1, 1.0, 1.2],
            },
            &VoxelParams::pitch(0.2),
            MassSpec::Density(2000.0),
        )
        .unwrap();
        Prescribed::new(slab, |t| {
            Isometry3::new(
                Quat::identity(),
                Vec3::new(4.0 + 1e-3 * (2.0 * t).sin(), 0.0, 1.2),
            )
        })
    };
    let scene = |air: bool| {
        let mut sources: Vec<Box<dyn WorldSource>> = vec![
            Box::new(Rigid(Box::new(pass(ball(300.0), 2.0)))),
            Box::new(walker.source().unwrap()),
            Box::new(Ambient(Rigid(Box::new(wall())))),
        ];
        if air {
            sources.push(Box::new(drifting_air(t0, t1)));
        }
        Scenario::scene(sources, array(), schedule.clone(), 9)
            .with_field_set(decomposed())
            .with_uldm(UldmConfig::default())
            .with_noise(NoiseStack(vec![Box::new(ShotNoise { sigma: 1e-6 })]))
    };
    let bundle = run(&scene(true));
    let (targets, ambient, uldm) = (
        bundle.signal_targets.as_ref().unwrap(),
        bundle.signal_atmospheric.as_ref().unwrap(),
        bundle.signal_uldm.as_ref().unwrap(),
    );
    assert_eq!(
        bundle.source_position.len(),
        2,
        "the rigid target and the walker are tracked"
    );
    assert_eq!(bundle.source_mass.as_ref().unwrap().len(), 2);
    for t in 0..5 {
        for d in 0..2 {
            let sum = targets[t][d] + ambient[t][d] + uldm[t] + bundle.signal_noise[t][d];
            assert!(
                (sum - bundle.signal[t][d]).abs() <= 1e-10 * bundle.signal[t][d].abs().max(1e-12)
            );
        }
    }
    // Without the air the ambient channel is the wall alone; the difference is the air alone, and
    // must equal the propagation integral of the grid field evaluated the slow way.
    let no_air = run(&scene(false));
    let field = drifting_air(t0, t1);
    let model = PropagationIntegral::default();
    for (t, &time) in bundle.time.iter().enumerate() {
        for (d, det) in array().detectors.iter().enumerate() {
            let air_fast = ambient[t][d] - no_air.signal_atmospheric.as_ref().unwrap()[t][d];
            let air_slow = model.delta_phi(&[], &[&field], det, time);
            assert!(
                (air_fast - air_slow).abs() <= 1e-6 * air_slow.abs(),
                "t={t} d={d}: {air_fast:e} vs {air_slow:e}"
            );
            assert!(
                (targets[t][d] - no_air.signal_targets.as_ref().unwrap()[t][d]).abs()
                    <= 1e-12 * targets[t][d].abs()
            );
        }
    }
    // The quasi-static model reads the same scene through the gradient tensor, without the fast path.
    let qs = run(&scene(true).with_phase_model(PhaseModelKind::QuasiStatic));
    assert!(qs.signal.iter().flatten().all(|x| x.is_finite()));
}

#[test]
fn simulated_scene_runs_end_to_end() {
    // Dynamics → recorded track → air around it → forward model. A warm ball dropped onto the floor
    // of a small room: its track is ballistic until it lands, the room's air is heated by it, and
    // both reach the bundle on the right channels.
    let schedule = Schedule::uniform(1.0, 4)
        .times
        .iter()
        .map(|t| t + 2.0)
        .collect::<Vec<_>>();
    let schedule = Schedule {
        mask: vec![false; schedule.len()],
        times: schedule,
    };
    let (t0, t1) = (0.0, 5.2);
    let r = 0.2;
    let cloud = voxelise(
        &Sphere { r },
        &VoxelParams::pitch(0.1),
        MassSpec::Density(800.0),
    )
    .unwrap();
    let mut world = RigidWorld::new(Vec3::new(0.0, 0.0, -9.81), 1e-3).with_floor(0.0);
    world.time = t0;
    let mut body = RigidBody::new(cloud, Collider::Sphere { r }, Vec3::new(1.6, 0.0, 1.0));
    body.restitution = 0.0;
    world.add(body);
    let track = world.record(t1, 0.02).pop().unwrap();

    let room = Room::new([0.8, -0.8, 0.0], [1.6, 1.6, 2.0], 0.2, 293.15);
    let ball_source = Rigid(Box::new(track));
    let (_, air) = simulate_air(
        &room,
        &[],
        &[Obstacle {
            source: &ball_source,
            density: 800.0,
            heat: room.film(330.0, 8.0),
        }],
        t0,
        t1,
        0.2,
        0.8,
    );
    let bundle = run(&Scenario::scene(
        vec![Box::new(ball_source), Box::new(air)],
        array(),
        schedule,
        4,
    )
    .with_field_set(decomposed()));
    // By t = 2 s the ball has landed and is at rest on the floor at z = r.
    let z = bundle.source_position[0]
        .iter()
        .map(|p| p[2])
        .collect::<Vec<_>>();
    assert!(
        z.iter().all(|&z| (z - r).abs() <= 5e-3),
        "resting height {z:?}"
    );
    let ambient = bundle.signal_atmospheric.unwrap();
    let targets = bundle.signal_targets.unwrap();
    assert!(
        ambient.iter().flatten().all(|x| x.abs() > 0.0),
        "the heated air gravitates"
    );
    assert!(
        ambient.last().unwrap()[0].abs() > ambient[0][0].abs(),
        "and the anomaly grows as it heats"
    );
    assert!(
        targets[0][0].abs() > 100.0 * ambient[0][0].abs(),
        "the ball dominates the air"
    );
}

#[test]
fn batch_is_ordered_reproducible_and_exportable() {
    // The parallel batch returns scene i at index i with scene i's key, identically on every call,
    // and the stacked arrays have the documented shapes.
    let build = |_: usize, key: u64| {
        let x = 2.0 + (key % 1000) as f64 * 1e-3;
        Scenario::new(
            Box::new(pass(ball(300.0), x)),
            array(),
            Schedule::uniform(2.0, 4),
            key,
        )
        .with_field_set(decomposed())
    };
    let (a, b) = (batch(5, 42, build), batch(5, 42, build));
    assert_eq!(a, b);
    for (i, bundle) in a.iter().enumerate() {
        assert_eq!(bundle, &run(&build(i, generate::scenario_key(42, i))));
    }
    let dir = std::env::temp_dir().join(format!("cavendish-dataset-{}", std::process::id()));
    write_dataset(&dir, &a).unwrap();
    let size = |name: &str| std::fs::metadata(dir.join(name)).unwrap().len();
    assert_eq!(size("signal.npy") % 64, (5 * 4 * 2 * 8) % 64);
    assert_eq!(size("source_position.npy"), size("source_velocity.npy"));
    for name in [
        "signal_targets.npy",
        "signal_atmospheric.npy",
        "source_mass.npy",
        "time.npy",
        "detector_placement.npy",
    ] {
        assert!(dir.join(name).exists(), "{name} missing");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
