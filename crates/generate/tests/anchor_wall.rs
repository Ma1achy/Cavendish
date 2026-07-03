//! M1 concrete-wall anchor: Cavendish's ad-hoc voxel lattice vs the independent reference cuboid.
//!
//! Both compute the **static** DC gradiometer phase for the spec wall geometry (`tab:anchors`,
//! `0.225 × 6.1 × 12.2 m`). The two paths are deliberately different — Cavendish sums a uniform
//! point lattice through its closed-form arms and the spec sign `δφ₂ − δφ₁`; the reference
//! integrates the solid cuboid by Gauss–Legendre quadrature through George's stepped, floor-clamped
//! arms and his opposite sign `δφ₁ − δφ₂` — so agreement is meaningful, not circular.
//!
//! The published ~50 µrad figure is the wall **oscillating** by 1 µm (an AC response); that
//! time-series/FFT comparison is M2/M6. Here there is no motion, so the assertion is that the two
//! independent implementations agree on the static value — the sign difference is reconciled by
//! comparing magnitudes, and neither implementation's physics is bent to match the other.

use generate::{wall_cloud, Detector, Isometry3, PhaseModel, Prescribed, Vec3};
use instrument::PropagationIntegral;
use reference::{cuboid_dc_gradiometer_phase, Cuboid};

const SIZE: [f64; 3] = [0.225, 6.1, 12.2]; // spec concrete wall (thickness × width × height)
const DENSITY: f64 = 2400.0; // concrete, kg/m³
const BASE_Z: f64 = 2.5; // lower IFO height; IFOs at 2.5 and 7.5, centre at 5.0
const CENTRE: [f64; 3] = [1.0, 0.0, 5.0]; // wall centre; ~0.89 m face standoff, aligned in z
const PITCH: f64 = 0.075; // divides the 0.225 m thickness exactly (3 layers) — mass to ~0.01%
const N_QUAD: usize = 20; // Gauss–Legendre nodes per axis

fn cavendish_dc_phase() -> f64 {
    let cloud = wall_cloud(
        Vec3::new(SIZE[0], SIZE[1], SIZE[2]),
        Vec3::new(CENTRE[0], CENTRE[1], CENTRE[2]),
        DENSITY,
        PITCH,
    );
    let src = Prescribed::fixed(cloud, Isometry3::identity());
    PropagationIntegral::default().delta_phi(&[&src], &Detector::new(BASE_Z), 2.0)
}

#[test]
fn anchor_wall() {
    let cav = cavendish_dc_phase();
    let wall = Cuboid {
        size: SIZE,
        centre: CENTRE,
        density: DENSITY,
    };
    let re = cuboid_dc_gradiometer_phase(&wall, BASE_Z, N_QUAD);

    assert!(
        cav.is_finite() && cav != 0.0,
        "cavendish ΔΦ not finite/non-zero"
    );
    assert!(
        re.is_finite() && re != 0.0,
        "reference ΔΦ not finite/non-zero"
    );
    // Opposite signs are expected (spec vs George convention); compare magnitudes.
    let delta = (cav.abs() - re.abs()).abs() / re.abs();
    assert!(
        delta <= 0.10,
        "wall anchor: cavendish {cav:.4e} vs reference {re:.4e} = {:.2}% (> 10%)",
        delta * 100.0
    );
}
