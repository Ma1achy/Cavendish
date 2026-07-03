//! Per-detector anchor: M2's moving-mass anchor on a **2-detector array** at different ranges.
//!
//! The source (a voxelised sphere, point-like) passes vertically at fixed horizontal x. Two detectors
//! sit at different x, so at different horizontal ranges. The **nearer** detector must read the
//! larger peak, and each detector's reading agrees with the single-detector reference
//! (`reference::point_mass_phase`, unchanged) at its own range. `crates/reference/` is untouched.

use generate::{Detector, Isometry3, Path, PhaseModel, Quat, Source, Trajectory, Vec3};
use instrument::PropagationIntegral;
use reference::point_mass_phase;
use shape::{voxelise, MassSpec, Sphere, VoxelParams};

const SPEED: f64 = 2.5; // vertical pass speed [m/s]
const MASS: f64 = 10.0; // kg
const SOURCE_X: f64 = 10.0; // source horizontal position

fn sphere(mass: f64) -> gravity::Cloud {
    voxelise(
        &Sphere { r: 0.1 },
        &VoxelParams::pitch(0.025),
        MassSpec::Total(mass),
    )
    .unwrap()
}

/// Cavendish peak |ΔΦ| over the pass, for a detector at some placement.
fn cav_peak(det: &Detector, times: &[f64]) -> f64 {
    let place = Isometry3::new(Quat::identity(), Vec3::new(SOURCE_X, 0.0, 0.0));
    let path = Path::LinearPass {
        a: Vec3::new(0.0, 0.0, -10.0),
        b: Vec3::new(0.0, 0.0, 10.0),
    };
    let timing = generate::Timing::Uniform { rate: SPEED / 20.0 }; // dz/dt = 20·rate = 2.5
    let model = PropagationIntegral::default();
    times
        .iter()
        .map(|&t| {
            let src = Source::new(sphere(MASS), Trajectory::new(place, path, timing));
            model.delta_phi(&[&src], det, t).abs()
        })
        .fold(0.0, f64::max)
}

/// Reference peak |ΔΦ| for a single detector at horizontal `standoff` (George's point-mass phase).
fn ref_peak(standoff: f64, times: &[f64]) -> f64 {
    let height = |t: f64| -10.0 + SPEED * t;
    times
        .iter()
        .map(|&t| point_mass_phase(MASS, standoff, 0.0, height, t).abs())
        .fold(0.0, f64::max)
}

#[test]
fn anchor_per_detector() {
    let times: Vec<f64> = (0..=20).map(|i| i as f64 * 0.5).collect(); // h = −10 → +15 m
    let det_a = Detector::new(0.0); // x = 0 → range 10 (farther)
    let det_b = Detector::placed(Isometry3::new(Quat::identity(), Vec3::new(4.0, 0.0, 0.0))); // x = 4 → range 6 (nearer)

    let (cav_a, cav_b) = (cav_peak(&det_a, &times), cav_peak(&det_b, &times));
    let (ref_a, ref_b) = (ref_peak(10.0, &times), ref_peak(6.0, &times));

    // The nearer detector reads the larger peak.
    assert!(
        cav_b > cav_a,
        "nearer detector not larger: B {cav_b:.3e} vs A {cav_a:.3e}"
    );

    // Each detector agrees with the single-detector reference at its own range.
    let da = (cav_a - ref_a).abs() / ref_a;
    let db = (cav_b - ref_b).abs() / ref_b;
    eprintln!(
        "per-detector: A(range 10) cav={cav_a:.4e} ref={ref_a:.4e} {:.2}%  |  B(range 6) cav={cav_b:.4e} ref={ref_b:.4e} {:.2}%",
        da * 100.0,
        db * 100.0
    );
    assert!(da <= 0.10, "detector A off reference: {:.2}%", da * 100.0);
    assert!(db <= 0.10, "detector B off reference: {:.2}%", db * 100.0);
}
