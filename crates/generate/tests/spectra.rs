//! M6b periodogram wiring: the Lomb–Scargle field is present and correctly shaped when
//! `FieldSet.periodogram` is on, absent when off. (The bin-exact planted-line recovery is
//! `ls_planted_line` in the reference-agreement suite.)

use generate::{
    run, Detector, DetectorArray, FieldSet, Isometry3, Orient, Path, Quat, Scenario, Schedule,
    Source, Timing, Trajectory, UldmConfig, Vec3,
};
use shape::{voxelise, Cuboid, MassSpec, VoxelParams};

fn static_scenario(periodogram: bool) -> Scenario {
    let cloud = voxelise(
        &Cuboid {
            half: [0.2, 0.2, 0.2],
        },
        &VoxelParams::pitch(0.2),
        MassSpec::Total(1000.0),
    )
    .unwrap();
    let traj = Trajectory::new(
        Isometry3::new(Quat::identity(), Vec3::new(3.0, 0.0, 0.0)),
        Path::Static,
        Timing::Uniform { rate: 0.0 },
    )
    .with_orient(Orient::Fixed(Quat::identity()));
    Scenario::new(
        Box::new(Source::new(cloud, traj)),
        DetectorArray::new(vec![Detector::new(0.0), Detector::new(1.0)]),
        Schedule::uniform(2.0, 64),
        3,
    )
    .with_uldm(UldmConfig {
        amplitude: 1e-3,
        frequency: 0.1,
        phase: 0.0,
    })
    .with_field_set(FieldSet {
        periodogram,
        ..FieldSet::default()
    })
}

#[test]
fn periodogram_present() {
    let b = run(&static_scenario(true));
    let pg = b.periodogram.expect("periodogram computed when gated on");
    assert_eq!(pg.power.len(), 2, "one spectrum per detector");
    assert!(!pg.freqs.is_empty());
    assert!(pg.power.iter().all(|row| row.len() == pg.freqs.len()));

    assert!(
        run(&static_scenario(false)).periodogram.is_none(),
        "periodogram must be None when the flag is off"
    );
}
