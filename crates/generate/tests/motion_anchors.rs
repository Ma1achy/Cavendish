//! Motion anchors on **voxelised** clouds: a moving mass and an oscillating mass, each agreeing with
//! the independent reference point-mass phase to ≤10%.
//!
//! The Cavendish side is a voxelised `shape::Sphere` (point-like: R ≪ D, and the shell theorem holds)
//! carried by the closed-form motion library, evaluated through `PropagationIntegral`. The reference
//! side is George's stepped-arm point-mass phase (`reference::point_mass_phase`) — a different method,
//! opposite sign — so magnitudes are compared. Parameters are spec `tab:anchors`.

use generate::{Detector, Isometry3, PhaseModel, Quat, Source, Trajectory, Vec3};
use generate::{Path, Timing};
use instrument::PropagationIntegral;
use reference::point_mass_phase;
use shape::{voxelise, MassSpec, Sphere, VoxelParams};

const TAU: f64 = std::f64::consts::TAU;
const BASE_Z: f64 = 0.0; // IFOs at 0 and 5

fn point_sphere(mass: f64) -> gravity::Cloud {
    // R ≪ D, so the sphere reads as a point mass externally (shell theorem).
    voxelise(
        &Sphere { r: 0.1 },
        &VoxelParams::pitch(0.025),
        MassSpec::Total(mass),
    )
    .unwrap()
}

/// The Cavendish gradiometer phase at `t` for a source at horizontal `standoff` moving vertically.
fn cav_phase(cloud: gravity::Cloud, standoff: f64, path: Path, timing: Timing, t: f64) -> f64 {
    let place = Isometry3::new(Quat::identity(), Vec3::new(standoff, 0.0, 0.0));
    let src = Source::new(cloud, Trajectory::new(place, path, timing));
    PropagationIntegral::default().delta_phi(&[&src], &[], &Detector::new(BASE_Z), t)
}

#[test]
fn anchor_moving() {
    // 10 kg at D = 10 m, vertical pass at 2.5 m/s → ≈ 7 mrad peak.
    let (mass, d, speed) = (10.0, 10.0, 2.5);
    let times: Vec<f64> = (0..=20).map(|i| i as f64 * 0.5).collect(); // h = −10 → +15 m
    let path = Path::LinearPass {
        a: Vec3::new(0.0, 0.0, -10.0),
        b: Vec3::new(0.0, 0.0, 10.0),
    };
    let timing = Timing::Uniform { rate: speed / 20.0 }; // dz/dt = 20·rate = 2.5
    let height = |t: f64| -10.0 + speed * t;

    let cav_peak = times
        .iter()
        .map(|&t| cav_phase(point_sphere(mass), d, path, timing, t).abs())
        .fold(0.0, f64::max);
    let ref_peak = times
        .iter()
        .map(|&t| point_mass_phase(mass, d, BASE_Z, height, t).abs())
        .fold(0.0, f64::max);

    let delta = (cav_peak - ref_peak).abs() / ref_peak;
    eprintln!(
        "moving: cav={cav_peak:.4e} ref={ref_peak:.4e} (~{:.2} mrad) delta={:.3}%",
        ref_peak * 1e3,
        delta * 100.0
    );
    assert!(delta <= 0.10, "anchor_moving: {:.2}% > 10%", delta * 100.0);
}

#[test]
fn anchor_oscillation() {
    // 1 kg at D = 5 m, h(t) = 1 + cos(2π·0.1 t) → ≈ 2 mrad peak-to-peak.
    let (mass, d, freq) = (1.0, 5.0, 0.1);
    let times: Vec<f64> = (0..=40).map(|i| i as f64 * 0.25).collect(); // one 10 s period
    let path = Path::Oscillation {
        axis: Vec3::new(0.0, 0.0, 1.0),
        amp: 1.0,
        freq,
        phase: std::f64::consts::FRAC_PI_2, // sin(·+π/2) = cos
    };
    let timing = Timing::Uniform { rate: 1.0 };
    let placement_offset = 1.0; // the static "+1" in h = 1 + cos
    let height = |t: f64| placement_offset + (TAU * freq * t).cos();

    // Cavendish: the "+1" offset goes into the placement height, the oscillation into the path.
    let cav: Vec<f64> = times
        .iter()
        .map(|&t| {
            let place = Isometry3::new(Quat::identity(), Vec3::new(d, 0.0, placement_offset));
            let src = Source::new(point_sphere(mass), Trajectory::new(place, path, timing));
            PropagationIntegral::default().delta_phi(&[&src], &[], &Detector::new(BASE_Z), t)
        })
        .collect();
    let reference: Vec<f64> = times
        .iter()
        .map(|&t| point_mass_phase(mass, d, BASE_Z, height, t))
        .collect();

    let p2p = |v: &[f64]| {
        v.iter().cloned().fold(f64::MIN, f64::max) - v.iter().cloned().fold(f64::MAX, f64::min)
    };
    let (cav_pp, ref_pp) = (p2p(&cav), p2p(&reference));
    let delta = (cav_pp - ref_pp).abs() / ref_pp;
    eprintln!(
        "oscillation: cav_pp={cav_pp:.4e} ref_pp={ref_pp:.4e} (~{:.2} mrad) delta={:.3}%",
        ref_pp * 1e3,
        delta * 100.0
    );
    assert!(
        delta <= 0.10,
        "anchor_oscillation: {:.2}% > 10%",
        delta * 100.0
    );
}
