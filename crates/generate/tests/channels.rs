//! M5 channels & decomposition: the exact superposition identity, noise recoverability, the per-IFO
//! reconstruction, cost gating, and the common-mode structure that distinguishes the channels.

use generate::{
    run, AtmoConfig, Detector, DetectorArray, FieldSet, Isometry3, NoiseStack, Prescribed, Quat,
    Scenario, Schedule, ShotNoise, UldmConfig, Vec3,
};

fn point() -> Prescribed {
    Prescribed::fixed(
        gravity::Cloud::from_elements(&[(3.0, 0.0, 2.5, 500.0)]),
        Isometry3::identity(),
    )
}

fn scenario(
    decompose: bool,
    atmo: bool,
    noise: bool,
    dets: Vec<Detector>,
    sched: Schedule,
) -> Scenario {
    let mut s = Scenario::new(Box::new(point()), DetectorArray::new(dets), sched, 7)
        .with_field_set(FieldSet {
            decomposition: decompose,
            ..FieldSet::default()
        })
        .with_uldm(UldmConfig {
            amplitude: 1e-3,
            frequency: 0.1,
            phase: 0.3,
        });
    if atmo {
        s = s.with_atmo(AtmoConfig {
            n_modes: 64,
            correlation_length: 50.0,
            amplitude: 1.0,
            sound_speed: 343.0,
        });
    }
    if noise {
        s = s.with_noise(NoiseStack(vec![Box::new(ShotNoise { sigma: 1e-4 })]));
    }
    s
}

fn one_det() -> Vec<Detector> {
    vec![Detector::new(0.0)]
}

fn corr(a: &[f64], b: &[f64]) -> f64 {
    let (ma, mb) = (
        a.iter().sum::<f64>() / a.len() as f64,
        b.iter().sum::<f64>() / b.len() as f64,
    );
    let mut num = 0.0;
    let (mut va, mut vb) = (0.0, 0.0);
    for (&x, &y) in a.iter().zip(b) {
        num += (x - ma) * (y - mb);
        va += (x - ma).powi(2);
        vb += (y - mb).powi(2);
    }
    num / (va.sqrt() * vb.sqrt())
}

#[test]
fn sum_identity() {
    // signal_targets + signal_atmospheric + signal_uldm + signal_noise = signal, ≤1e-10 rel.
    let b = run(&scenario(
        true,
        true,
        true,
        one_det(),
        Schedule::uniform(2.0, 6),
    ));
    let targets = b.signal_targets.unwrap();
    let atmo = b.signal_atmospheric.unwrap();
    let uldm = b.signal_uldm.unwrap();
    let mut worst = 0.0f64;
    for ti in 0..b.time.len() {
        for di in 0..b.signal[ti].len() {
            let recon = targets[ti][di] + atmo[ti][di] + uldm[ti] + b.signal_noise[ti][di];
            let sig = b.signal[ti][di];
            worst = worst.max((recon - sig).abs() / sig.abs().max(1e-30));
        }
    }
    eprintln!("sum_identity worst rel {worst:.2e}");
    assert!(worst <= 1e-10, "channels do not sum to signal: {worst:.2e}");
}

#[test]
#[allow(clippy::float_cmp)] // additive separability is exact by construction
fn noise_recoverable() {
    // signal − signal_noise is the clean forward signal, bit-for-bit (additive separability).
    let sched = Schedule::uniform(2.0, 5);
    let noisy = run(&scenario(false, true, true, one_det(), sched.clone()));
    let clean = run(&scenario(false, true, false, one_det(), sched));
    for ti in 0..noisy.time.len() {
        for di in 0..noisy.signal[ti].len() {
            // noisy.signal = clean.signal + noise, exactly (same deterministic forward pass).
            assert_eq!(
                noisy.signal[ti][di],
                clean.signal[ti][di] + noisy.signal_noise[ti][di]
            );
        }
    }
}

#[test]
fn per_ifo() {
    // signal_per_ifo[…,1] − signal_per_ifo[…,0] = the gradiometer ΔΦ (targets + atmospheric), ≤1e-12.
    let b = run(&scenario(
        true,
        true,
        false,
        one_det(),
        Schedule::uniform(2.0, 4),
    ));
    let pif = b.signal_per_ifo.unwrap();
    let targets = b.signal_targets.unwrap();
    let atmo = b.signal_atmospheric.unwrap();
    for ti in 0..b.time.len() {
        for di in 0..pif[ti].len() {
            let ddphi = pif[ti][di][1] - pif[ti][di][0];
            let grav = targets[ti][di] + atmo[ti][di];
            assert!(
                (ddphi - grav).abs() / grav.abs().max(1e-30) <= 1e-12,
                "per_ifo at ({ti},{di})"
            );
        }
    }
}

#[test]
fn fieldset_gating() {
    // Decomposition off ⇒ channel fields None (only one combined gravitational pass ran).
    let off = run(&scenario(
        false,
        true,
        true,
        one_det(),
        Schedule::uniform(2.0, 3),
    ));
    assert!(off.signal_targets.is_none());
    assert!(off.signal_atmospheric.is_none());
    assert!(off.signal_uldm.is_none());
    assert!(off.signal_per_ifo.is_none());
    // On ⇒ each channel is recorded.
    let on = run(&scenario(
        true,
        true,
        true,
        one_det(),
        Schedule::uniform(2.0, 3),
    ));
    assert!(on.signal_targets.is_some());
    assert!(on.signal_atmospheric.is_some());
    assert!(on.signal_uldm.is_some());
    assert!(on.signal_per_ifo.is_some());
}

#[test]
fn atmo_in_field_channel() {
    // Atmospheric appears in signal_atmospheric and is ABSENT from signal_noise (it's in the field).
    let b = run(&scenario(
        true,
        true,
        false,
        one_det(),
        Schedule::uniform(0.1, 20),
    ));
    let atmo = b.signal_atmospheric.unwrap();
    let atmo_energy: f64 = atmo.iter().flatten().map(|x| x * x).sum();
    assert!(atmo_energy > 0.0, "atmospheric channel is empty");
    // With an empty noise stack, signal_noise is exactly zero — atmo is not in the noise.
    let noise_energy: f64 = b.signal_noise.iter().flatten().map(|x| x * x).sum();
    assert!(
        noise_energy == 0.0,
        "atmospheric leaked into the noise stack"
    );
}

#[test]
fn common_mode_structure() {
    // ULDM is fully common-mode (a (T,) channel, identical across D). Atmospheric is partially
    // common-mode: correlation between two detectors decreases as their baseline grows past ℓ_c.
    let sched = Schedule::uniform(0.05, 60);
    // ULDM: the channel has no detector axis — common-mode by construction.
    let b = run(&scenario(
        true,
        true,
        false,
        vec![
            Detector::new(0.0),
            Detector::placed(Isometry3::new(Quat::identity(), Vec3::new(5.0, 0.0, 0.0))),
        ],
        sched.clone(),
    ));
    assert_eq!(b.signal_uldm.as_ref().unwrap().len(), b.time.len()); // (T,), not (T,D)

    // Atmospheric correlation across the baseline, at increasing separations (ℓ_c = 50 m).
    let atmo_corr = |baseline: f64| {
        let dets = vec![
            Detector::new(0.0),
            Detector::placed(Isometry3::new(
                Quat::identity(),
                Vec3::new(baseline, 0.0, 0.0),
            )),
        ];
        let bundle = run(&scenario(true, true, false, dets, sched.clone()));
        let atmo = bundle.signal_atmospheric.unwrap();
        let a: Vec<f64> = atmo.iter().map(|r| r[0]).collect();
        let c: Vec<f64> = atmo.iter().map(|r| r[1]).collect();
        corr(&a, &c)
    };
    let (near, mid, far) = (atmo_corr(2.0), atmo_corr(50.0), atmo_corr(400.0));
    eprintln!("common_mode: atmo corr near(2m) {near:.3} mid(50m) {mid:.3} far(400m) {far:.3}");
    // b ≪ ℓ_c: nearly common-mode. Past ℓ_c: decorrelated (well below the near value) — the isotropic
    // sinc correlation oscillates around zero past ℓ_c, so we check it has fallen, not strict monotonicity.
    assert!(
        near > 0.9,
        "b ≪ ℓ_c should be nearly common-mode: {near:.3}"
    );
    assert!(
        mid.abs() < 0.5 && far.abs() < 0.5,
        "b ≳ ℓ_c should decorrelate: mid {mid:.3} far {far:.3}"
    );
    assert!(
        near > mid && near > far,
        "atmo must decorrelate past ℓ_c relative to the common-mode near value"
    );
}
