//! Prints a hash of the serialised bundle for a spread of v1 scenes. Run on two revisions and diff
//! the output to prove a change to the source seam left the forward model bit-identical.
use generate::{
    run, AtmoConfig, Detector, DetectorArray, FieldSet, Isometry3, NoiseStack, Orient, Path,
    PhaseModelKind, Quat, Scenario, Schedule, ShotNoise, Source, Timing, Trajectory, UldmConfig,
    Vec3,
};
use shape::{voxelise, Cuboid, MassSpec, Sphere, VoxelParams};

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    })
}

fn main() {
    let array = DetectorArray::new(vec![
        Detector::new(0.0),
        Detector::placed(Isometry3::new(
            Quat::from_axis_angle(Vec3::new(0.2, 1.0, 0.0), 0.1),
            Vec3::new(6.0, 1.0, 0.0),
        )),
    ]);
    let all = FieldSet {
        shape: true,
        decomposition: true,
        periodogram: true,
    };
    let wall = voxelise(
        &Cuboid {
            half: [0.2, 0.5, 0.4],
        },
        &VoxelParams::pitch(0.1),
        MassSpec::Total(900.0),
    )
    .unwrap();
    let ball = voxelise(
        &Sphere { r: 0.3 },
        &VoxelParams::pitch(0.1),
        MassSpec::Total(400.0),
    )
    .unwrap();
    let place = Isometry3::new(Quat::identity(), Vec3::new(2.5, 0.3, 2.0));
    let paths = [
        (Path::Static, Orient::Fixed(Quat::identity())),
        (
            Path::Oscillation {
                axis: Vec3::new(0.0, 0.0, 1.0),
                amp: 0.4,
                freq: 0.3,
                phase: 0.2,
            },
            Orient::FreeRotation {
                omega0: Vec3::new(0.4, 0.1, 0.7),
            },
        ),
        (
            Path::LinearPass {
                a: Vec3::new(0.0, -3.0, 0.0),
                b: Vec3::new(0.0, 3.0, 0.0),
            },
            Orient::Libration {
                axis: Vec3::new(0.0, 1.0, 0.0),
                pivot_distance: 0.9,
                theta0: 0.3,
                thetadot0: 0.1,
            },
        ),
    ];
    let mut k = 0;
    for cloud in [&wall, &ball] {
        for (path, orient) in paths {
            for kind in [
                PhaseModelKind::PropagationIntegral,
                PhaseModelKind::QuasiStatic,
            ] {
                for fs in [FieldSet::default(), all] {
                    let traj = Trajectory::new(
                        place,
                        path,
                        Timing::Eased {
                            rate: 0.1,
                            accel: 0.01,
                        },
                    )
                    .with_orient(orient);
                    let scn = Scenario::new(
                        Box::new(Source::new(cloud.clone(), traj)),
                        array.clone(),
                        Schedule::jittered(1.5, 6, 0.2, 7 + k).with_contamination(0.3, 5),
                        11 + k,
                    )
                    .with_phase_model(kind)
                    .with_field_set(fs)
                    .with_atmo(AtmoConfig::default())
                    .with_uldm(UldmConfig::default())
                    .with_noise(NoiseStack(vec![Box::new(ShotNoise { sigma: 1e-3 })]));
                    let bytes = bincode::serialize(&run(&scn)).unwrap();
                    println!("{k:02} {:016x}", fnv(&bytes));
                    k += 1;
                }
            }
        }
    }
}
