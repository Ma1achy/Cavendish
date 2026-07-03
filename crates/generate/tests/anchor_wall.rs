//! Concrete-wall anchor: Cavendish's **voxelised** cuboid vs the independent reference cuboid.
//!
//! M2 retires M1's ad-hoc lattice: the Cavendish side is now a properly voxelised `shape::Cuboid`
//! (renormalised, recentred, boundary-subsampled), posed at the wall centre and evaluated through
//! the propagation integral with the spec sign `δφ₂ − δφ₁`. The reference side is **unchanged** —
//! George's Gauss–Legendre quadrature cuboid through his stepped arms and his opposite sign
//! `δφ₁ − δφ₂` — so agreement stays meaningful, not circular.
//!
//! Both compute the **static** DC gradiometer phase for the spec wall geometry (`tab:anchors`,
//! `0.225 × 6.1 × 12.2 m`). The published ~50 µrad figure is the wall oscillating by 1 µm (an AC
//! response, M6); here there is no motion, so the assertion is that the two independent methods
//! agree on the static value, reconciled by magnitude.

use generate::{Detector, Isometry3, PhaseModel, Prescribed, Quat, Vec3};
use instrument::PropagationIntegral;
use reference::{cuboid_dc_gradiometer_phase, Cuboid as RefCuboid};
use shape::{voxelise, Cuboid, MassSpec, VoxelParams};

const SIZE: [f64; 3] = [0.225, 6.1, 12.2]; // spec concrete wall (thickness × width × height)
const DENSITY: f64 = 2400.0; // concrete, kg/m³
const BASE_Z: f64 = 2.5; // lower IFO height; IFOs at 2.5 and 7.5, centre at 5.0
const CENTRE: [f64; 3] = [1.0, 0.0, 5.0]; // wall centre; ~0.89 m face standoff, aligned in z
const PITCH: f64 = 0.075; // divides the 0.225 m thickness exactly (3 layers)
const N_QUAD: usize = 20; // Gauss–Legendre nodes per axis

fn cavendish_dc_phase() -> f64 {
    // Voxelise the cuboid (recentred to the origin), then pose it at the wall centre.
    let solid = Cuboid {
        half: [SIZE[0] * 0.5, SIZE[1] * 0.5, SIZE[2] * 0.5],
    };
    let cloud = voxelise(
        &solid,
        &VoxelParams::pitch(PITCH),
        MassSpec::Density(DENSITY),
    )
    .unwrap();
    let pose = Isometry3::new(Quat::identity(), Vec3::new(CENTRE[0], CENTRE[1], CENTRE[2]));
    let src = Prescribed::fixed(cloud, pose);
    PropagationIntegral::default().delta_phi(&[&src], &Detector::new(BASE_Z), 2.0)
}

#[test]
fn anchor_wall() {
    let cav = cavendish_dc_phase();
    let wall = RefCuboid {
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
    eprintln!(
        "wall: cavendish={cav:+.5e} reference={re:+.5e} delta={:.3}%",
        delta * 100.0
    );
    assert!(
        delta <= 0.10,
        "wall anchor: cavendish {cav:.4e} vs reference {re:.4e} = {:.2}% (> 10%)",
        delta * 100.0
    );
}
