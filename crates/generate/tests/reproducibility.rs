//! M6b reproducibility spine. `batch_invariant`: a scenario run alone equals the same scenario inside
//! a 256-batch, bit-exact — because its RNG-keyed channels (atmospheric draws, keyed off the
//! scenario's seed) hang off its INDEX in the key tree, not its batch position. `seed_replay`:
//! `(Prior, seed)` twice → identical tensors, independent of batch size.

use generate::{
    run, scenario_key, stream, AtmoConfig, Detector, DetectorArray, Dist, Prior, RunConfig,
    Schedule,
};
use shape::{voxelise, Cuboid, MassSpec, VoxelParams};

fn prior() -> Prior {
    Prior {
        cloud: voxelise(
            &Cuboid {
                half: [0.2, 0.2, 0.2],
            },
            &VoxelParams::pitch(0.2),
            MassSpec::Total(1000.0),
        )
        .unwrap(),
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
        array: DetectorArray::new(vec![Detector::new(0.0), Detector::new(1.0)]),
        schedule: Schedule::uniform(2.0, 8),
        field_set: Default::default(),
        // An RNG-keyed channel, so batch-invariance is a real check, not just run() determinism.
        atmo: Some(AtmoConfig {
            n_modes: 8,
            correlation_length: 50.0,
            amplitude: 1.0,
            sound_speed: 343.0,
        }),
    }
}

#[test]
fn batch_invariant() {
    let p = prior();
    let root = 12_345;
    let batch: Vec<_> = stream(&p, 256, root, RunConfig { batch: 32 }).collect();
    for &i in &[0usize, 1, 7, 63, 128, 255] {
        let alone = run(&p.sample(scenario_key(root, i)));
        let in_batch = &batch[i];
        assert_eq!(alone.signal, in_batch.signal, "signal differs at i={i}");
        assert_eq!(
            alone.signal_noise, in_batch.signal_noise,
            "noise channel differs at i={i}"
        );
        assert_eq!(alone.mask, in_batch.mask, "mask differs at i={i}");
    }
}

#[test]
fn seed_replay() {
    let p = prior();
    let root = 999;
    // Two runs, DIFFERENT batch sizes — identical results (replay is seed-determined, not batch-shaped).
    let a: Vec<_> = stream(&p, 16, root, RunConfig { batch: 4 })
        .map(|b| b.signal)
        .collect();
    let b: Vec<_> = stream(&p, 16, root, RunConfig { batch: 16 })
        .map(|b| b.signal)
        .collect();
    assert_eq!(a, b, "replay differs across batch sizes");
}
