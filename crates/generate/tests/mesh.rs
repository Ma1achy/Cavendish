//! M10 end-to-end: an imported mesh is a first-class body. It voxelises through the M2 pipeline, is
//! re-expressed in its principal frame (M10-R7), tumbles about its intermediate axis (M4), and its
//! ΔΦ decomposes/streams like any primitive — a spot-check through the M5/M6 paths.

use generate::{
    run, Detector, DetectorArray, FieldSet, Isometry3, Orient, Path, Scenario, Schedule, Source,
    Timing, Trajectory, Vec3,
};
use shape::{principal_frame, voxelise_mesh, MassSpec, MeshSolid, TriSoup, VoxelParams};

/// A watertight asymmetric box mesh (half-extents 0.35/0.2/0.12 — distinct principal moments), tilted
/// out of its principal frame so the authored inertia is non-diagonal (the case M10-R7 resolves).
fn tilted_box_soup() -> TriSoup {
    let mut verts = vec![
        [-0.5, -0.5, -0.5],
        [0.5, -0.5, -0.5],
        [0.5, 0.5, -0.5],
        [-0.5, 0.5, -0.5],
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [0.5, 0.5, 0.5],
        [-0.5, 0.5, 0.5],
    ];
    let h = [0.35, 0.2, 0.12];
    let (a, b) = (0.5f64, 0.7f64); // yaw, then pitch
    let (ca, sa, cb, sb) = (a.cos(), a.sin(), b.cos(), b.sin());
    for v in &mut verts {
        let p = [v[0] * 2.0 * h[0], v[1] * 2.0 * h[1], v[2] * 2.0 * h[2]]; // ±0.5 → ±h
        let z = [ca * p[0] - sa * p[1], sa * p[0] + ca * p[1], p[2]]; // about z
        *v = [z[0], cb * z[1] - sb * z[2], sb * z[1] + cb * z[2]]; // about x
    }
    let tris = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [2, 3, 7],
        [2, 7, 6],
        [0, 4, 7],
        [0, 7, 3],
        [1, 2, 6],
        [1, 6, 5],
    ];
    TriSoup { verts, tris }
}

#[test]
fn imported_body_runs() {
    // Import → voxelise (same M2 pipeline) → principal frame → spin about the intermediate axis.
    let (mesh, report) = MeshSolid::from_soup(tilted_box_soup()).unwrap();
    assert!(report.watertight, "the box mesh is watertight");
    // A coarse pitch keeps the structural spot-check cheap (a fine cloud only slows ΔΦ, it does not
    // change what this test asserts — that a mesh runs end-to-end).
    let cloud = voxelise_mesh(&mesh, &VoxelParams::pitch(0.05), MassSpec::Total(1.0)).unwrap();
    let (principal, _r) = principal_frame(&cloud);

    // In the principal frame the moments are ascending, so the intermediate axis is index 1 (y).
    // Spin about it with a small perturbation — the classic intermediate-axis (Dzhanibekov) setup.
    let traj = Trajectory::new(
        Isometry3::identity(),
        Path::Static,
        Timing::Uniform { rate: 0.0 },
    )
    .with_orient(Orient::FreeRotation {
        omega0: Vec3::new(0.02, 3.0, 0.01),
    });
    let source = Source::new(principal, traj).with_fine_dt(0.01);

    let n = 30;
    let scenario = Scenario::new(
        Box::new(source),
        DetectorArray::single(Detector::new(0.0)),
        Schedule::uniform(0.05, n),
        0,
    )
    .with_field_set(FieldSet {
        shape: true,
        decomposition: true,
        ..FieldSet::default()
    });
    let bundle = run(&scenario);

    // Tumbles: the orientation genuinely evolves over the schedule (the precondition resolution let
    // the integrator read the right principal moments; a non-principal cloud would tumble wrongly).
    let quats = &bundle.source_orientation[0];
    let mut drift = 0.0;
    for w in quats.windows(2) {
        for (a, b) in w[0].iter().zip(w[1].iter()) {
            drift += (b - a).abs();
        }
    }
    assert!(
        drift > 0.1,
        "imported mesh tumbles (orientation drift {drift})"
    );

    // The spin is live at every step.
    assert!(bundle.source_angular_velocity[0]
        .iter()
        .all(|w| w.iter().any(|c| c.abs() > 0.0)));

    // ΔΦ decomposes and streams like any primitive: the decomposition channels are populated.
    assert!(bundle.signal_targets.is_some(), "targets channel present");
    assert!(bundle.signal_per_ifo.is_some(), "per-IFO channel present");

    // Shape descriptors filled — the mesh is a first-class body with an inertia reduction.
    assert!(bundle.source_moments.is_some(), "principal moments present");
    assert!(bundle.source_axes.is_some(), "principal axes present");
}
