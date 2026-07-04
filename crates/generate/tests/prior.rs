//! M6b `prior_total`: 10⁴ samples from a validated `Prior` all construct **runnable** `Scenario`s
//! (each `generate::run`s to a finite signal). The `Prior` is optional batch sugar — direct
//! `Scenario` construction stays the primary path.

use generate::run;
use scenario::{Detector, DetectorArray, Dist, Prior, Schedule};
use shape::{voxelise, Cuboid, MassSpec, VoxelParams};

fn template() -> gravity::Cloud {
    // Coarse on purpose — 10⁴ full runs must stay cheap (a handful of voxels, one measurement).
    voxelise(
        &Cuboid {
            half: [0.15, 0.15, 0.15],
        },
        &VoxelParams::pitch(0.15),
        MassSpec::Total(1000.0),
    )
    .unwrap()
}

fn prior() -> Prior {
    Prior {
        cloud: template(),
        fields: vec![
            (
                "mass".into(),
                Dist::Uniform {
                    lo: 500.0,
                    hi: 1500.0,
                },
            ),
            ("standoff".into(), Dist::Uniform { lo: 2.0, hi: 5.0 }),
            ("uldm_amp".into(), Dist::Uniform { lo: 0.0, hi: 1e-3 }),
            ("uldm_freq".into(), Dist::LogUniform { lo: 0.05, hi: 0.2 }),
        ],
        array: DetectorArray::new(vec![Detector::new(0.0)]),
        schedule: Schedule::uniform(2.0, 1),
        field_set: Default::default(),
        atmo: None,
    }
}

#[test]
fn prior_total() {
    let p = prior();
    p.validate().expect("prior validates");
    for i in 0..10_000u64 {
        let scn = p.sample(i);
        let bundle = run(&scn);
        assert!(
            bundle.signal.iter().flatten().all(|x| x.is_finite()),
            "sample {i} produced a non-finite signal"
        );
    }
}
