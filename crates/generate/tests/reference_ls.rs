//! M6b — George's Lomb–Scargle pipeline agreements. Cavendish's forward model + LS vs an INDEPENDENT
//! math-only reconstruction of George's method in `crates/reference/` (Scargle LS, a zero-filled FFT,
//! his ULDM line). This is M5's ULDM/lift channels' FIRST external validation. The robust invariant is
//! **bin-exact line recovery** on gappy/jittered sampling; heights are model-dependent (M5 chose a
//! simplified ULDM amplitude) so height agreement is only asserted where the physics is shared.

use generate::{
    run, Detector, DetectorArray, FieldSet, Isometry3, Orient, Path, Quat, Scenario, Schedule,
    Source, Timing, Trajectory, UldmConfig, Vec3,
};
use shape::{voxelise, Cuboid, MassSpec, Sphere, VoxelParams};

const TAU: f64 = std::f64::consts::TAU;

fn cuboid() -> gravity::Cloud {
    voxelise(
        &Cuboid {
            half: [0.2, 0.2, 0.2],
        },
        &VoxelParams::pitch(0.2),
        MassSpec::Total(1000.0),
    )
    .unwrap()
}

fn sphere() -> gravity::Cloud {
    voxelise(
        &Sphere { r: 0.1 },
        &VoxelParams::pitch(0.05),
        MassSpec::Total(1000.0),
    )
    .unwrap()
}

/// A static source (constant DC phase) plus a ULDM common-mode line, on the given schedule.
fn uldm_scenario(cloud: gravity::Cloud, f_uldm: f64, schedule: Schedule) -> Scenario {
    let traj = Trajectory::new(
        Isometry3::new(Quat::identity(), Vec3::new(3.0, 0.0, 0.0)),
        Path::Static,
        Timing::Uniform { rate: 0.0 },
    )
    .with_orient(Orient::Fixed(Quat::identity()));
    Scenario::new(
        Box::new(Source::new(cloud, traj)),
        DetectorArray::new(vec![Detector::new(0.0)]),
        schedule,
        3,
    )
    .with_uldm(UldmConfig {
        amplitude: 1e-3,
        frequency: f_uldm,
        phase: 0.0,
    })
    .with_field_set(FieldSet {
        periodogram: true,
        ..FieldSet::default()
    })
}

/// The signal series at detector 0 and the measurement times.
fn series(scn: &Scenario) -> (Vec<f64>, Vec<f64>) {
    let b = run(scn);
    let y = b.signal.iter().map(|row| row[0]).collect();
    (b.time, y)
}

fn excise(times: &[f64], y: &[f64], mask: &[bool]) -> (Vec<f64>, Vec<f64>) {
    times
        .iter()
        .zip(y)
        .zip(mask)
        .filter(|(_, &m)| !m)
        .map(|((&t, &v), _)| (t, v))
        .unzip()
}

#[test]
fn gapped_fft_vs_ls_agree() {
    // The textbook FFT failure: on IRREGULARLY sampled data, LS resolves a line ABOVE the mean-cadence
    // Nyquist (0.5 Hz here) uniquely, whereas the zero-filled FFT is mirror-symmetric about Nyquist and
    // cannot tell f₀ = 0.7 Hz from its alias 0.3 Hz. Cavendish's LS and reference's LS agree and both
    // break the alias; the FFT does not. (Regular sampling would alias LS too — irregularity is the key.)
    let (cadence, n, f0) = (1.0, 512usize, 0.7);
    let sched = Schedule::jittered(cadence, n, 0.4, 7); // genuinely non-uniform (dt ∈ [0.2, 1.8])
    let y: Vec<f64> = sched.times.iter().map(|&t| (TAU * f0 * t).cos()).collect();
    // A grid spanning above the mean-Nyquist (the auto grid caps at ≈0.5).
    let freqs: Vec<f64> = (4..=200).map(|k| k as f64 * 0.005).collect(); // 0.02 … 1.0 Hz

    let cav = state::lomb_scargle(&sched.times, &y, &freqs);
    let refls = reference::lomb_scargle(&sched.times, &y, &freqs);
    let fft = reference::zero_filled_fft_power(&sched.times, &y, cadence, n, &freqs);

    let target = reference::nearest_bin(&freqs, f0);
    let alias = reference::nearest_bin(&freqs, 1.0 - f0); // 0.3 Hz
    assert_eq!(
        reference::peak_bin(&cav),
        target,
        "cavendish LS misses the super-Nyquist line"
    );
    assert_eq!(
        reference::peak_bin(&refls),
        target,
        "reference LS misses it"
    );

    // The two independent LS implementations agree ≤5% on the peak height.
    let rel = (cav[target] - refls[target]).abs() / refls[target];
    assert!(
        rel <= 0.05,
        "cavendish vs reference LS peak: {:.2}%",
        rel * 100.0
    );

    // LS breaks the alias (power at 0.3 ≪ power at 0.7); the FFT cannot (mirror-symmetric ⇒ ≈equal).
    let alias_ratio = |p: &[f64]| p[alias] / p[target].max(1e-30);
    assert!(
        alias_ratio(&cav) < 0.5 && alias_ratio(&fft) > 0.8,
        "LS did not break the FFT's alias: LS {:.2} vs FFT {:.2}",
        alias_ratio(&cav),
        alias_ratio(&fft)
    );
    eprintln!(
        "gapped_fft_vs_ls: line {f0} Hz @ bin {target}; LS≈ref {:.2}%; alias 0.3/0.7 LS {:.2} vs FFT {:.2}",
        rel * 100.0,
        alias_ratio(&cav),
        alias_ratio(&fft)
    );
}

#[test]
fn uldm_ls_agree() {
    // Cavendish's ULDM channel and George's ULDM line, both LS'd, recover the same 0.1 Hz line bin-exact.
    let f_uldm = 0.1;
    let sched = Schedule::uniform(1.0, 512);
    let (t, y) = series(&uldm_scenario(cuboid(), f_uldm, sched));
    let freqs = state::frequency_grid(&t);
    let cav = state::lomb_scargle(&t, &y, &freqs);

    let ref_y = reference::uldm_series(&t, 1e-3, f_uldm);
    let refls = reference::lomb_scargle(&t, &ref_y, &freqs);

    let target = reference::nearest_bin(&freqs, f_uldm);
    assert_eq!(
        reference::peak_bin(&cav),
        target,
        "cavendish ULDM line off bin"
    );
    assert_eq!(
        reference::peak_bin(&refls),
        target,
        "reference ULDM line off bin"
    );
    eprintln!(
        "uldm_ls_agree: ULDM line recovered at {:.4} Hz (bin {target}) by both pipelines",
        freqs[target]
    );
}

#[test]
fn cycle_jitter_agree() {
    // A jittered (non-uniform) schedule: both pipelines still recover the ULDM line bin-exact.
    let f_uldm = 0.1;
    let sched = Schedule::jittered(1.0, 512, 0.3, 5);
    let (t, y) = series(&uldm_scenario(cuboid(), f_uldm, sched));
    let freqs = state::frequency_grid(&t);
    let cav = state::lomb_scargle(&t, &y, &freqs);
    let refls = reference::lomb_scargle(&t, &reference::uldm_series(&t, 1e-3, f_uldm), &freqs);

    let target = reference::nearest_bin(&freqs, f_uldm);
    assert_eq!(
        reference::peak_bin(&cav),
        target,
        "cavendish jittered line off bin"
    );
    assert_eq!(
        reference::peak_bin(&refls),
        target,
        "reference jittered line off bin"
    );
}

#[test]
fn lift_excision_agree() {
    // A moving lift contaminates cycles as it passes the detector; excising them (the mask) and LS-ing
    // the survivors recovers the underlying ULDM line — bin-exact, matching a reference excision.
    let f_uldm = 0.1;
    let n = 512;
    // A lift passing vertically at close range: build the schedule, then mark the transit cycles.
    let mut sched = Schedule::uniform(1.0, n);
    let lift = Trajectory::new(
        Isometry3::new(Quat::identity(), Vec3::new(2.0, 0.0, 0.0)),
        Path::LinearPass {
            a: Vec3::new(0.0, 0.0, -40.0),
            b: Vec3::new(0.0, 0.0, 40.0),
        },
        Timing::Uniform { rate: 0.16 }, // dz/dt = 80·rate ≈ 12.8 m/s → transit near mid-run
    )
    .with_orient(Orient::Fixed(Quat::identity()));
    // The lift enters at z = −40 and crosses z = 0 at t ≈ 3 s (dz/dt ≈ 12.8 m/s); mark the early
    // approach-and-transit cycles as contaminated. Beyond ~cycle 20 the lift is >200 m away (negligible).
    for (i, m) in sched.mask.iter_mut().enumerate() {
        *m = i < 20;
    }
    let scn = Scenario::new(
        Box::new(Source::new(sphere(), lift)),
        DetectorArray::new(vec![Detector::new(0.0)]),
        sched.clone(),
        3,
    )
    .with_uldm(UldmConfig {
        amplitude: 1e-3,
        frequency: f_uldm,
        phase: 0.0,
    });
    let (t, y) = series_no_pg(&scn);

    let freqs = state::frequency_grid(&t);
    let target = reference::nearest_bin(&freqs, f_uldm);

    // Excise the contaminated cycles, then LS the survivors — the ULDM line becomes the global peak.
    let (te, ye) = excise(&t, &y, &sched.mask);
    let cav = state::lomb_scargle(&te, &ye, &freqs);
    assert_eq!(
        reference::peak_bin(&cav),
        target,
        "cavendish: ULDM line not recovered after excision"
    );

    // Reference: George's lift transient + ULDM line, same excision, same recovery.
    let ref_uldm = reference::uldm_series(&t, 1e-3, f_uldm);
    let ref_full: Vec<f64> = t
        .iter()
        .zip(&ref_uldm)
        .map(|(&ti, &u)| {
            let h = |tt: f64| -40.0 + 12.8 * tt;
            reference::point_mass_phase(1000.0, 2.0, 0.0, h, ti) + u
        })
        .collect();
    let (tr, yr) = excise(&t, &ref_full, &sched.mask);
    let refls = reference::lomb_scargle(&tr, &yr, &freqs);
    assert_eq!(
        reference::peak_bin(&refls),
        target,
        "reference: ULDM line not recovered after excision"
    );
}

#[test]
fn ls_planted_line() {
    // An oscillating source (drive line) + ULDM, on a GAPPY schedule: Cavendish's LS recovers BOTH
    // lines as peaks within one grid bin.
    let (f_drive, f_uldm) = (0.05, 0.1);
    let sched = Schedule::gappy(1.0, 512, 0.2, 11);
    let osc = Trajectory::new(
        Isometry3::new(Quat::identity(), Vec3::new(3.0, 0.0, 0.0)),
        Path::Oscillation {
            axis: Vec3::new(0.0, 0.0, 1.0),
            amp: 1.0,
            freq: f_drive,
            phase: 0.0,
        },
        Timing::Uniform { rate: 1.0 },
    )
    .with_orient(Orient::Fixed(Quat::identity()));
    let scn = Scenario::new(
        Box::new(Source::new(sphere(), osc)),
        DetectorArray::new(vec![Detector::new(0.0)]),
        sched,
        3,
    )
    .with_uldm(UldmConfig {
        amplitude: 5e-3,
        frequency: f_uldm,
        phase: 0.0,
    })
    .with_field_set(FieldSet {
        periodogram: true,
        ..FieldSet::default()
    });
    let b = run(&scn);
    let pg = b.periodogram.unwrap();
    let (freqs, power) = (&pg.freqs, &pg.power[0]);

    // The two strongest peaks are within one bin of the two planted lines.
    let bin = |f: f64| reference::nearest_bin(freqs, f);
    let df = freqs[1] - freqs[0];
    let near = |k: usize, f: f64| (freqs[k] - f).abs() <= 1.5 * df;

    // Rank bins by power; take the top two well-separated peaks.
    let mut order: Vec<usize> = (0..power.len()).collect();
    order.sort_by(|&a, &b| power[b].partial_cmp(&power[a]).unwrap());
    let top1 = order[0];
    let top2 = *order
        .iter()
        .find(|&&k| (k as isize - top1 as isize).unsigned_abs() > 3)
        .unwrap();

    let hits = [top1, top2];
    assert!(
        hits.iter().any(|&k| near(k, f_drive)),
        "drive line {f_drive} not among the top peaks (bins {:?}, want {})",
        hits,
        bin(f_drive)
    );
    assert!(
        hits.iter().any(|&k| near(k, f_uldm)),
        "ULDM line {f_uldm} not among the top peaks (bins {:?}, want {})",
        hits,
        bin(f_uldm)
    );
}

/// The signal series without needing the periodogram field (for the excision test, which LS's a subset).
fn series_no_pg(scn: &Scenario) -> (Vec<f64>, Vec<f64>) {
    let b = run(scn);
    let y = b.signal.iter().map(|row| row[0]).collect();
    (b.time, y)
}
