//! M4 rotation bundle fields: angular velocity consistent with the orientation, and the shape
//! descriptors gated by `FieldSet.shape`.

use generate::{
    run, Detector, DetectorArray, FieldSet, Isometry3, Orient, Path, Quat, Scenario, Schedule,
    Source, SourceDynamics, Timing, Trajectory, Vec3,
};
use shape::{voxelise, Cuboid, MassSpec, VoxelParams};

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
