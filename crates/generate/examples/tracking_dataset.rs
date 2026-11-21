//! Generates the tracking proof-of-concept datasets: one object crossing a five-gradiometer array.
//!
//! `cargo run --release -p generate --example tracking_dataset -- <out_dir> [n_train]`
//!
//! The training set is point masses. Every test set then replays the **same** passes (same keys, so
//! the same starts, velocities and masses) with a different body — a point mass, a sphere, a cuboid,
//! a walking articulated figure — and once more as a point mass crossing a room whose air is being
//! heated by a radiator. Because the passes are paired, any difference in a model's error between
//! sets is down to the body or the air, not to which passes happened to be drawn.

use generate::{
    batch, write_dataset, Detector, DetectorArray, FieldSet, Isometry3, Key, NoiseStack, Path,
    Quat, Rigid, Scenario, Schedule, ShotNoise, Source, Timing, Trajectory, Vec3, WorldSource,
};
use gravity::Cloud;
use physics::{simulate_air, Room, Walker};
use shape::{voxelise, Cuboid, MassSpec, Sphere, VoxelParams};

const TOWERS: [[f64; 2]; 5] = [[0.0, 0.0], [4.0, 0.0], [-4.0, 0.0], [0.0, 4.0], [0.0, -4.0]];
const CADENCE: f64 = 0.5;
const N_MEAS: usize = 24;
const SHOT: f64 = 1e-3;
/// Every test set replays these passes.
const TEST_ROOT: u64 = 2;
const N_TEST: usize = 300;

fn array() -> DetectorArray {
    DetectorArray::new(
        TOWERS
            .iter()
            .map(|p| Detector::placed(Isometry3::new(Quat::identity(), Vec3::new(p[0], p[1], 0.0))))
            .collect(),
    )
}

/// A pass drawn from the scene key: where it starts, its velocity, and the object's mass.
struct Pass {
    start: Vec3<f64>,
    velocity: Vec3<f64>,
    mass: f64,
}

fn draw(key: u64) -> Pass {
    let k = Key::root(key).child("pass");
    let duration = CADENCE * (N_MEAS - 1) as f64;
    // Rejection-sample until the path keeps half a metre clear of every tower.
    for attempt in 0.. {
        let u = |i: u64| k.index(attempt).unit(i);
        let start = [-5.0 + 10.0 * u(0), -5.0 + 10.0 * u(1)];
        let heading = std::f64::consts::TAU * u(2);
        let speed = 0.4 + 0.8 * u(3);
        let v = [speed * heading.cos(), speed * heading.sin()];
        let clear = TOWERS.iter().all(|tw| {
            // Closest approach of the segment (extended back over the first flight) to the tower.
            let (t_lo, t_hi) = (-1.5, duration);
            let rel = [start[0] - tw[0], start[1] - tw[1]];
            let t_star = (-(rel[0] * v[0] + rel[1] * v[1]) / (speed * speed)).clamp(t_lo, t_hi);
            let d = [rel[0] + v[0] * t_star, rel[1] + v[1] * t_star];
            (d[0] * d[0] + d[1] * d[1]).sqrt() >= 0.6
        });
        let end = [start[0] + v[0] * duration, start[1] + v[1] * duration];
        if clear && end[0].abs() <= 6.0 && end[1].abs() <= 6.0 {
            return Pass {
                start: Vec3::new(start[0], start[1], 0.0),
                velocity: Vec3::new(v[0], v[1], 0.0),
                mass: 50.0 + 40.0 * u(4),
            };
        }
    }
    unreachable!()
}

/// A rigid body of the given cloud making the pass, its centre of mass 1 m above the floor.
fn rigid_pass(cloud: Cloud, pass: &Pass) -> Box<dyn WorldSource> {
    let duration = CADENCE * (N_MEAS - 1) as f64;
    let place = Isometry3::new(Quat::identity(), pass.start + Vec3::new(0.0, 0.0, 1.0));
    let traj = Trajectory::new(
        place,
        Path::LinearPass {
            a: Vec3::new(0.0, 0.0, 0.0),
            b: pass.velocity.scale(duration),
        },
        Timing::Uniform {
            rate: 1.0 / duration,
        },
    );
    Box::new(Rigid(Box::new(Source::new(
        shape::scale_mass(&cloud, pass.mass),
        traj,
    ))))
}

fn scenario(sources: Vec<Box<dyn WorldSource>>, key: u64) -> Scenario {
    Scenario::scene(sources, array(), Schedule::uniform(CADENCE, N_MEAS), key)
        .with_field_set(FieldSet {
            shape: true,
            decomposition: true,
            periodogram: false,
        })
        .with_noise(NoiseStack(vec![Box::new(ShotNoise { sigma: SHOT })]))
}

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = std::path::PathBuf::from(args.next().unwrap_or_else(|| "tracking_dataset".into()));
    let n_train: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(30_000);

    let point = Cloud::from_elements(&[(0.0, 0.0, 0.0, 1.0)]);
    let sphere = voxelise(
        &Sphere { r: 0.25 },
        &VoxelParams::pitch(0.1),
        MassSpec::Total(1.0),
    )
    .unwrap();
    let cuboid = voxelise(
        &Cuboid {
            half: [0.2, 0.3, 0.5],
        },
        &VoxelParams::pitch(0.125),
        MassSpec::Total(1.0),
    )
    .unwrap();

    let rigid_set = |name: &str, n: usize, root: u64, cloud: &Cloud| -> std::io::Result<()> {
        let start = std::time::Instant::now();
        let bundles = batch(n, root, |_, key| {
            scenario(vec![rigid_pass(cloud.clone(), &draw(key))], key)
        });
        println!(
            "{name}: {n} scenes, {} elements, {:.1} s",
            cloud.len(),
            start.elapsed().as_secs_f64()
        );
        write_dataset(out.join(name), &bundles)
    };
    rigid_set("train_point", n_train, 1, &point)?;
    rigid_set("test_point", N_TEST, TEST_ROOT, &point)?;
    rigid_set("test_sphere", N_TEST, TEST_ROOT, &sphere)?;
    rigid_set("test_cuboid", N_TEST, TEST_ROOT, &cuboid)?;

    // The walking figure: an articulated body, feet on the floor.
    let start = std::time::Instant::now();
    let bundles = batch(N_TEST, TEST_ROOT, |_, key| {
        let pass = draw(key);
        let walker = Walker {
            mass: pass.mass,
            height: 1.75,
            pitch: 0.1,
            start: pass.start,
            velocity: pass.velocity,
            swing: 0.5,
            cadence: 0.9,
        };
        scenario(vec![Box::new(walker.source().unwrap())], key)
    });
    println!(
        "test_walker: {N_TEST} scenes, {:.1} s",
        start.elapsed().as_secs_f64()
    );
    write_dataset(out.join("test_walker"), &bundles)?;

    // A point mass crossing while a radiator heats the room: convective gravity-gradient noise in
    // the ambient channel. The air is simulated once and shared (it does not depend on the pass).
    let start = std::time::Instant::now();
    let room = Room::new([-5.0, -5.0, 0.0], [10.0, 10.0, 3.0], 0.25, 293.15);
    // A 1.5 m × 0.25 m × 0.75 m radiator against the −y side of the central tower, 70 °C surface.
    let mut fixed = Vec::new();
    for k in 0..3 {
        for i in 17..23 {
            fixed.push((room.grid.idx(i, 16, k), room.film(343.15, 10.0)));
        }
    }
    let (t0, t1) = (-40.0, CADENCE * (N_MEAS - 1) as f64);
    let (fluid, air) = simulate_air(&room, &fixed, &[], t0, t1, 0.25, 0.8);
    let wmax = fluid.w.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    println!(
        "radiator room: {} cells, {:.0} s simulated in {:.1} s, plume speed {wmax:.2} m/s",
        room.grid.len(),
        t1 - t0,
        start.elapsed().as_secs_f64()
    );
    let air = std::sync::Arc::new(air);
    let bundles = batch(N_TEST, TEST_ROOT, |_, key| {
        scenario(
            vec![
                rigid_pass(point.clone(), &draw(key)),
                Box::new(Shared(air.clone())),
            ],
            key,
        )
    });
    let ggn = bundles
        .iter()
        .flat_map(|b| b.signal_atmospheric.as_ref().unwrap().iter().flatten())
        .fold(0.0f64, |m, x| m.max(x.abs()));
    println!(
        "test_point_radiator: {N_TEST} scenes, {:.1} s; peak convective phase {ggn:.2e} rad (shot noise {SHOT:.0e})",
        start.elapsed().as_secs_f64()
    );
    write_dataset(out.join("test_point_radiator"), &bundles)
}

/// One simulated field shared by many scenes.
struct Shared(std::sync::Arc<physics::GridField>);

impl WorldSource for Shared {
    fn eval(&self) -> generate::Eval<'_> {
        self.0.eval()
    }
    fn world_cloud_at(&self, t: f64) -> Cloud {
        self.0.world_cloud_at(t)
    }
    fn channel(&self) -> generate::Channel {
        self.0.channel()
    }
    fn per_ifo_fast(
        &self,
        cfg: &instrument::InstrumentConfig,
        det: &Detector,
        t: f64,
    ) -> Option<[f64; 2]> {
        self.0.per_ifo_fast(cfg, det, t)
    }
}
