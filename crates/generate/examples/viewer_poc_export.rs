//! Records one scene for the browser proof-of-concept of the scene viewer (`milestones/M13-viewer-v2.md`):
//! a walking figure crossing a five-tower array while a radiator heats the room.
//!
//! `cargo run --release -p generate --example viewer_poc_export -- <out_dir>`
//!
//! Writes `replay.json` (layout, clock, signals by channel), `walker.bin` (parcel positions per frame,
//! i16 millimetres), `air.bin` (air density anomaly per cell per frame, u8) and `response.bin` (each
//! detector's phase per kilogram in each cell, f32). A hand-rolled stand-in for the planned `Replay`.

use generate::{
    run, Channel, Detector, DetectorArray, Eval, FieldSet, Isometry3, NoiseStack, Quat, Scenario,
    Schedule, ShotNoise, Vec3, WorldSource,
};
use gravity::Cloud;
use instrument::{build_arms, InstrumentConfig};
use physics::{flight_nodes, kernel::cube_potential, simulate_air, GridField, Room, Walker};
use std::sync::Arc;

const TOWERS: [[f64; 2]; 5] = [[0.0, 0.0], [4.0, 0.0], [-4.0, 0.0], [0.0, 4.0], [0.0, -4.0]];
const DT: f64 = 0.1;
const FRAMES: usize = 121;
const AIR_EVERY: usize = 5;

struct Shared(Arc<GridField>);
impl WorldSource for Shared {
    fn eval(&self) -> Eval<'_> {
        self.0.eval()
    }
    fn world_cloud_at(&self, t: f64) -> Cloud {
        self.0.world_cloud_at(t)
    }
    fn channel(&self) -> Channel {
        self.0.channel()
    }
    fn per_ifo_fast(&self, cfg: &InstrumentConfig, det: &Detector, t: f64) -> Option<[f64; 2]> {
        self.0.per_ifo_fast(cfg, det, t)
    }
}

fn walker() -> physics::Parcels {
    Walker {
        mass: 70.0,
        height: 1.75,
        pitch: 0.08,
        start: Vec3::new(-4.5, -2.2, 0.0),
        velocity: Vec3::new(0.75, 0.25, 0.0),
        swing: 0.55,
        cadence: 0.9,
    }
    .source()
    .unwrap()
}

fn rows(v: &[Vec<f64>]) -> String {
    let body: Vec<String> = v
        .iter()
        .map(|r| {
            format!(
                "[{}]",
                r.iter()
                    .map(|x| format!("{x:.6e}"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect();
    format!("[{}]", body.join(","))
}

fn main() -> std::io::Result<()> {
    let out = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "viewer_poc".into()),
    );
    std::fs::create_dir_all(&out)?;
    let detectors: Vec<Detector> = TOWERS
        .iter()
        .map(|p| Detector::placed(Isometry3::new(Quat::identity(), Vec3::new(p[0], p[1], 0.0))))
        .collect();

    // The room: a 1.5 m radiator at 70 °C beside the central tower, 40 s of spin-up.
    let room = Room::new([-5.0, -5.0, 0.0], [10.0, 10.0, 3.0], 0.25, 293.15);
    let mut fixed = Vec::new();
    let mut radiator = Vec::new();
    for k in 0..3 {
        for i in 17..23 {
            fixed.push((room.grid.idx(i, 16, k), room.film(343.15, 10.0)));
            radiator.push(room.grid.centre(i, 16, k));
        }
    }
    let t_end = DT * (FRAMES - 1) as f64;
    let (_, air) = simulate_air(&room, &fixed, &[], -40.0, t_end, 0.25, 0.8);
    let air = Arc::new(air);

    let times: Vec<f64> = (0..FRAMES).map(|f| f as f64 * DT).collect();
    let scenario = Scenario::scene(
        vec![Box::new(walker()), Box::new(Shared(air.clone()))],
        DetectorArray::new(detectors.clone()),
        Schedule {
            times: times.clone(),
            mask: vec![false; FRAMES],
        },
        7,
    )
    .with_field_set(FieldSet {
        shape: true,
        decomposition: true,
        periodogram: false,
    })
    .with_noise(NoiseStack(vec![Box::new(ShotNoise { sigma: 1e-3 })]));
    let bundle = run(&scenario);

    // Walker parcels per frame, millimetres as i16.
    let body = walker();
    let n = body.rest().len();
    let mut bytes = Vec::with_capacity(FRAMES * n * 6);
    for &t in &times {
        let c = body.posed(t);
        for i in 0..n {
            for v in [c.xs[i], c.ys[i], c.zs[i]] {
                bytes.extend_from_slice(&((v * 1000.0).round() as i16).to_le_bytes());
            }
        }
    }
    std::fs::write(out.join("walker.bin"), bytes)?;

    // Air density anomaly [kg/m³]; warm air is negative, so store −δρ scaled to a byte.
    let vol = room.grid.cell_volume();
    let air_frames: Vec<Vec<f64>> = (0..FRAMES)
        .step_by(AIR_EVERY)
        .map(|f| air.masses_at(times[f]).iter().map(|m| -m / vol).collect())
        .collect();
    let scale = air_frames.iter().flatten().fold(0.0f64, |m, &x| m.max(x));
    let bytes: Vec<u8> = air_frames
        .iter()
        .flatten()
        .map(|&x| (x / scale * 255.0).round().clamp(0.0, 255.0) as u8)
        .collect();
    std::fs::write(out.join("air.bin"), bytes)?;

    // Each detector's response: the double-differenced propagation phase per kilogram held in a cell.
    let cfg = InstrumentConfig::default();
    let nodes = flight_nodes(&cfg);
    let (xs, ys, zs) = room.grid.centres();
    let mut bytes = Vec::with_capacity(detectors.len() * xs.len() * 4);
    for det in &detectors {
        for i in 0..xs.len() {
            let x = Vec3::new(xs[i], ys[i], zs[i]);
            let mut per_ifo = [0.0; 2];
            for (ifo, acc) in build_arms(&cfg).iter().zip(per_ifo.iter_mut()) {
                for &(tau, w) in &nodes {
                    let pu = det
                        .placement
                        .apply(Vec3::new(0.0, 0.0, ifo.upper.z_at(tau)));
                    let pl = det
                        .placement
                        .apply(Vec3::new(0.0, 0.0, ifo.lower.z_at(tau)));
                    *acc += w
                        * (cube_potential(pu - x, room.grid.h)
                            - cube_potential(pl - x, room.grid.h));
                }
            }
            let dd = cfg.m_a / cfg.hbar * (per_ifo[1] - per_ifo[0]);
            bytes.extend_from_slice(&(dd as f32).to_le_bytes());
        }
    }
    std::fs::write(out.join("response.bin"), bytes)?;

    let track: Vec<Vec<f64>> = bundle.source_position[0]
        .iter()
        .map(|p| p.to_vec())
        .collect();
    let json = format!(
        "{{\"dt\":{DT},\"frames\":{FRAMES},\"two_t\":{},\"shot\":1e-3,\n\"towers\":{},\n\"grid\":{{\"origin\":{:?},\"h\":{},\"n\":{:?}}},\n\"radiator\":{},\n\"walker\":{{\"n\":{n},\"pitch\":0.08,\"mass\":70.0}},\n\"air\":{{\"frames\":{},\"every\":{AIR_EVERY},\"scale\":{scale:.6e}}},\n\"track\":{},\n\"signal\":{},\n\"targets\":{},\n\"ambient\":{},\n\"noise\":{}}}\n",
        2.0 * cfg.t_half,
        rows(&TOWERS.iter().map(|t| t.to_vec()).collect::<Vec<_>>()),
        room.grid.origin,
        room.grid.h,
        room.grid.n,
        rows(&radiator.iter().map(|c| c.to_vec()).collect::<Vec<_>>()),
        air_frames.len(),
        rows(&track),
        rows(&bundle.signal),
        rows(bundle.signal_targets.as_ref().unwrap()),
        rows(bundle.signal_atmospheric.as_ref().unwrap()),
        rows(&bundle.signal_noise),
    );
    std::fs::write(out.join("replay.json"), json)?;
    println!(
        "{n} parcels, {} air frames, peak density anomaly {:.1} g/m³",
        air_frames.len(),
        scale * 1e3
    );
    Ok(())
}
