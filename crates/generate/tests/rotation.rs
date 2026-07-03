//! M4 rotation bundle fields: angular velocity consistent with the orientation, and the shape
//! descriptors gated by `FieldSet.shape`.

use generate::{
    run, Detector, DetectorArray, FieldSet, Isometry3, Orient, Path, PhaseModel, Prescribed, Quat,
    Scenario, Schedule, Source, SourceDynamics, Timing, Trajectory, Vec3,
};
use instrument::PropagationIntegral;
use shape::{voxelise, Cuboid, MassSpec, Solid, Sphere, VoxelParams};

fn asym_cuboid(mass: f64) -> gravity::Cloud {
    voxelise(
        &Cuboid {
            half: [0.5, 0.8, 1.1],
        },
        &VoxelParams::pitch(0.1),
        MassSpec::Total(mass),
    )
    .unwrap()
}

fn tumbling(mass: f64) -> Source {
    let traj = Trajectory::new(
        Isometry3::identity(),
        Path::Static,
        Timing::Uniform { rate: 0.0 },
    )
    .with_orient(Orient::FreeRotation {
        omega0: Vec3::new(0.4, 0.1, 0.6),
    });
    Source::new(asym_cuboid(mass), traj)
}

#[test]
fn omega_consistency() {
    // source_angular_velocity matches the quaternion-log central finite difference of the orientation.
    // A fine integration step so q and ω are consistent to well within the tolerance (the mismatch
    // is O(fine_dt²)); the central FD step is separate.
    let src = tumbling(2.0).with_fine_dt(0.001);
    let dt = 1e-3;
    for &t in &[1.234, 2.717, 3.5] {
        let q_prev = src.pose_at(t - dt).rotation;
        let q_next = src.pose_at(t + dt).rotation;
        // Body-frame increment δq = q(t−dt)⁻¹ ⊗ q(t+dt) ≈ (1, ω_body·dt).
        let dq = q_prev.conjugate() * q_next;
        let omega_fd = Vec3::new(dq.x, dq.y, dq.z).scale(1.0 / dt);
        let m = src.motion_at(t);
        let rel = (omega_fd - m.angular_velocity).norm() / m.angular_velocity.norm();
        assert!(rel <= 1e-6, "omega mismatch at t={t}: {rel:.2e}");
    }
}

#[test]
fn shape_fields() {
    // FieldSet.shape on ⇒ descriptors filled; off ⇒ None.
    let cloud = asym_cuboid(3.0);
    let build = |on: bool| {
        let traj = Trajectory::new(
            Isometry3::identity(),
            Path::Static,
            Timing::Uniform { rate: 0.0 },
        )
        .with_orient(Orient::Fixed(Quat::identity()));
        Scenario::new(
            Box::new(Source::new(cloud.clone(), traj)),
            DetectorArray::single(Detector::new(0.0)),
            Schedule::uniform(2.0, 2),
            0,
        )
        .with_field_set(FieldSet { shape: on })
    };

    let on = run(&build(true));
    assert!(on.source_mass.is_some(), "mass");
    assert!(on.source_inertia.is_some(), "inertia");
    assert!(on.source_moments.is_some(), "moments");
    assert!(on.source_axes.is_some(), "axes");
    assert!(on.source_quadrupole.is_some(), "quadrupole");
    assert!(
        (on.source_mass.unwrap()[0] - 3.0).abs() / 3.0 <= 1e-6,
        "mass value"
    );

    let off = run(&build(false));
    assert!(off.source_mass.is_none());
    assert!(off.source_inertia.is_none());
    assert!(off.source_quadrupole.is_none());
}

const SPIN: f64 = 2.0; // rad/s about ê_z
const BODY_Z: f64 = 2.5; // at the gradiometer centre height

/// Spin a solid about ê_z at a CoM-fixed horizontal `standoff`; return the mean and peak-to-peak of
/// ΔΦ(t) over two quadrupole periods.
fn spin_phi(solid: &dyn Solid, mass: f64, standoff: f64) -> (f64, f64) {
    let cloud = voxelise(solid, &VoxelParams::pitch(0.1), MassSpec::Total(mass)).unwrap();
    let traj = Trajectory::new(
        Isometry3::new(Quat::identity(), Vec3::new(standoff, 0.0, BODY_Z)),
        Path::Static,
        Timing::Uniform { rate: 0.0 },
    )
    .with_orient(Orient::FreeRotation {
        omega0: Vec3::new(0.0, 0.0, SPIN),
    });
    let src = Source::new(cloud, traj).with_fine_dt(0.002);
    let model = PropagationIntegral::default();
    let det = Detector::new(0.0);
    let period = std::f64::consts::PI; // two quadrupole periods (Q has 2-fold symmetry)
    let n = 24;
    let phis: Vec<f64> = (0..n)
        .map(|k| model.delta_phi(&[&src], &det, k as f64 / n as f64 * period))
        .collect();
    let mean = phis.iter().sum::<f64>() / n as f64;
    let p2p = phis.iter().cloned().fold(f64::MIN, f64::max)
        - phis.iter().cloned().fold(f64::MAX, f64::min);
    (mean, p2p)
}

#[test]
fn pure_quadrupole() {
    // Dipole ≡ 0: the recentred cloud has no dipole about its CoM.
    let cloud = voxelise(
        &Cuboid {
            half: [0.2, 0.5, 0.4],
        },
        &VoxelParams::pitch(0.05),
        MassSpec::Total(100.0),
    )
    .unwrap();
    let m: f64 = cloud.ms.iter().sum();
    let com = gravity::inertia(&cloud).com;
    let mut dip = [0.0f64; 3];
    for k in 0..cloud.len() {
        dip[0] += cloud.ms[k] * (cloud.xs[k] - com.x);
        dip[1] += cloud.ms[k] * (cloud.ys[k] - com.y);
        dip[2] += cloud.ms[k] * (cloud.zs[k] - com.z);
    }
    let dipole = (dip[0] * dip[0] + dip[1] * dip[1] + dip[2] * dip[2]).sqrt() / m;
    assert!(dipole <= 1e-12, "dipole not zero: {dipole:.2e}");

    // The varying part vanishes for a sphere (Q = 0) and for a body spun about its symmetry axis (INV.5).
    let (sph_mean, sph_p2p) = spin_phi(&Sphere { r: 0.4 }, 100.0, 3.0);
    let (sym_mean, sym_p2p) = spin_phi(
        &Cuboid {
            half: [0.3, 0.3, 1.0],
        },
        100.0,
        3.0,
    );
    // An asymmetric body (Q_xx ≠ Q_yy) varies; a more asymmetric one varies more (scales with ‖Q‖).
    let (asym_mean, asym_p2p) = spin_phi(
        &Cuboid {
            half: [0.15, 0.6, 0.4],
        },
        100.0,
        3.0,
    );
    let (near_mean, near_p2p) = spin_phi(
        &Cuboid {
            half: [0.28, 0.32, 0.4],
        },
        100.0,
        3.0,
    );
    eprintln!(
        "pure_quadrupole: sphere {:.2e} sym {:.2e} asym {:.2e} near {:.2e} (p2p/|mean|)",
        sph_p2p / sph_mean.abs(),
        sym_p2p / sym_mean.abs(),
        asym_p2p / asym_mean.abs(),
        near_p2p / near_mean.abs()
    );
    assert!(sph_p2p / sph_mean.abs() <= 1e-3, "sphere should not vary");
    assert!(
        sym_p2p / sym_mean.abs() <= 1e-3,
        "symmetric-axis spin should be flat (INV.5)"
    );
    assert!(
        asym_p2p / asym_mean.abs() > 1e-3,
        "asymmetric body should vary"
    );
    assert!(asym_p2p > near_p2p * 3.0, "variation should scale with ‖Q‖");

    // The mean over a rotation is the static-monopole value (a point mass M at the CoM). Checked at a
    // larger standoff, where the body's static quadrupole offset (∝ 1/D³) is negligible.
    let far = 8.0;
    let point = Prescribed::fixed(
        gravity::Cloud::from_elements(&[(far, 0.0, BODY_Z, 100.0)]),
        Isometry3::identity(),
    );
    let mono = PropagationIntegral::default().delta_phi(&[&point], &Detector::new(0.0), 0.0);
    let (far_mean, _) = spin_phi(
        &Cuboid {
            half: [0.2, 0.5, 0.4],
        },
        100.0,
        far,
    );
    eprintln!("pure_quadrupole: mean {far_mean:.6e} vs monopole {mono:.6e}");
    assert!(
        (far_mean - mono).abs() / mono.abs() <= 1e-3,
        "mean ≠ monopole"
    );
}
