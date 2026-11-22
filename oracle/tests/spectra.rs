//! The harmonic comb and its folding (VALIDATION-AND-RESEARCH §2.1), and the engine's periodogram
//! against the reference scripts' own.
//!
//! The series here are the engine's own: its forward model with George's defaults, read at the
//! scripts' shot times. The scripts' spectra are the fixtures of `fixtures/scripts/`.

use std::f64::consts::TAU;

use cav_core::Vec3;
use cav_core::forward::{Instrument, Mass, PHASE_TOLERANCE, Reader};
use cav_core::spectrum::{Classical, Spectrum, fold_frequency, periodogram};
use oracle::fixture;

/// The engine's series for the scripts' oscillator (1 kg, 3 m from the axis, about 1 m up) of
/// amplitude `amp` at `f` hertz, at the shot times of a fixture directory.
fn engine_series(instrument: Instrument, dir: &str, amp: f64, f: f64) -> Vec<f64> {
    let reader = Reader::new(instrument);
    let source = Mass::point(1.0, move |t: f64| Vec3::new(3.0, 0.0, 1.0 + amp * (TAU * f * t).cos()));
    fixture(&format!("scripts/{dir}/t.npy"))
        .iter()
        .map(|&t| {
            reader.read_to(Vec3::ZERO, &source, t, PHASE_TOLERANCE).reading.gradiometer(instrument.sign)
        })
        .collect()
}

/// The frequencies of the `count` bins with the most power, in rising order.
fn largest(spectrum: &Spectrum, count: usize) -> Vec<f64> {
    let mut bins: Vec<usize> = (0..spectrum.power.len()).collect();
    bins.sort_by(|&a, &b| spectrum.power[b].total_cmp(&spectrum.power[a]));
    let mut top: Vec<f64> = bins[..count].iter().map(|&k| spectrum.frequencies[k]).collect();
    top.sort_by(f64::total_cmp);
    top
}

/// Whether two lists of frequencies are the same bins of a spectrum with the given bin width.
fn same_bins(a: &[f64], b: &[f64], width: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.5 * width)
}

/// A script's spectrum as a `Spectrum`.
fn script_spectrum(dir: &str, file: &str) -> Spectrum {
    Spectrum {
        frequencies: fixture(&format!("scripts/{dir}/f.npy")),
        power: fixture(&format!("scripts/{dir}/{file}.npy")),
    }
}

/// The largest difference between the engine's periodogram of a series and the script's own, as
/// a fraction of the script's peak.
fn periodogram_error(dir: &str, series: &[f64], dt: f64, options: Classical, file: &str) -> f64 {
    let (ours, theirs) = (periodogram(series, dt, options), script_spectrum(dir, file));
    assert_eq!(ours.frequencies.len(), theirs.frequencies.len(), "{dir}/{file}: the bins differ in number");
    let width = 1.0 / (series.len() as f64 * dt);
    assert!(same_bins(&ours.frequencies, &theirs.frequencies, 1e-6 * width), "{dir}/{file}: the bins differ");
    let peak = theirs.power.iter().fold(0.0f64, |m, p| m.max(*p));
    ours.power.iter().zip(&theirs.power).fold(0.0f64, |m, (a, b)| m.max((a - b).abs())) / peak
}

fn sum(parts: &[&[f64]]) -> Vec<f64> {
    (0..parts[0].len()).map(|k| parts.iter().map(|p| p[k]).sum()).collect()
}

cav_validate::case! {
    test: validation_periodogram_matches_the_scripts,
    name: "oracle.periodogram_vs_scripts",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "the engine's classical periodogram of each script's own series against the script's own spectrum: 1D_oscillator (no mean taken off, zero bin kept), and panels a to d of fig2_10m and fig2_uldm_folding, clean and with the script's noise; largest difference as a fraction of each spectrum's peak",
    unit: Relative,
    tolerance: 1e-9,
    measure: {
        let one_d = fixture("scripts/1d_oscillator/phase_published.npy");
        let mut worst = periodogram_error("1d_oscillator", &one_d, 2.0, Classical::ONE_D, "S_published");
        // Control: the other convention is not this script's.
        let other = periodogram(&one_d, 2.0, Classical::SCRIPTS);
        assert_ne!(other.frequencies.len(), script_spectrum("1d_oscillator", "S_published").frequencies.len());
        for (dir, dt) in [("fig2_10m", 2.0), ("fig2_uldm_folding", 5.0)] {
            let file = |name: &str| fixture(&format!("scripts/{dir}/{name}.npy"));
            let uldm = file("uldm");
            worst = worst.max(periodogram_error(dir, &sum(&[&uldm, &file("noise_a")]), dt, Classical::SCRIPTS, "S_noisy_a"));
            for panel in ["b", "c", "d"] {
                let raw = file(&format!("raw_{panel}_published"));
                worst = worst.max(periodogram_error(dir, &raw, dt, Classical::SCRIPTS, &format!("S_clean_{panel}")));
                let noise = file(&format!("noise_{panel}"));
                // What the script plots: panel b is the oscillator and noise; c and d add the ULDM tone.
                let plotted = if panel == "b" { sum(&[&raw, &noise]) } else { sum(&[&raw, &uldm, &noise]) };
                worst = worst.max(periodogram_error(dir, &plotted, dt, Classical::SCRIPTS, &format!("S_noisy_{panel}")));
            }
        }
        cav_validate::Measured::Error(worst)
    },
}

cav_validate::case! {
    test: validation_fold_matches_the_script,
    name: "oracle.fold_vs_script",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fold_frequency against fig2_folding_map.py's own, at 2001 frequencies from 0 to five times its Nyquist frequency of 0.1 Hz; largest difference",
    unit: Relative,
    tolerance: 1e-12,
    measure: {
        let (truth, observed) = (fixture("scripts/fig2_folding_map/f_true.npy"), fixture("scripts/fig2_folding_map/f_obs.npy"));
        assert_eq!(truth.len(), 2001);
        // Control: without the fold, most of the map is wrong.
        assert!(truth.iter().zip(&observed).filter(|(f, o)| (*f - *o).abs() > 1e-9).count() > 1500);
        let worst = truth.iter().zip(&observed).fold(0.0f64, |m, (f, o)| m.max((fold_frequency(*f, 0.1) - o).abs()));
        cav_validate::Measured::Error(worst / 0.1)
    },
}

#[test]
fn comb_at_integer_harmonics() {
    // fig2_10m panel b: 0.05 Hz, sampled every 2 s for 20,000 s. Nyquist is 0.25 Hz.
    let (dt, f0) = (2.0, 0.05);
    let series = engine_series(Instrument::aion10(), "fig2_10m", 0.5, f0);
    let width = 1.0 / (series.len() as f64 * dt);
    let harmonics: Vec<f64> = (1..=4).map(|n| f64::from(n) * f0).collect();
    // Without noise: the four largest bins are the first four harmonics, in the engine's series
    // and in the script's own spectrum.
    let clean = periodogram(&series, dt, Classical::SCRIPTS);
    assert!(
        same_bins(&largest(&clean, 4), &harmonics, width),
        "the engine's peaks: {:?}",
        largest(&clean, 4)
    );
    let script = script_spectrum("fig2_10m", "S_clean_b");
    assert!(
        same_bins(&largest(&script, 4), &harmonics, width),
        "the script's peaks: {:?}",
        largest(&script, 4)
    );
    let height = |spectrum: &Spectrum, f: f64| spectrum.power[spectrum.bin_of(f).unwrap()].sqrt() * 1e6;
    for (n, f) in harmonics.iter().enumerate() {
        eprintln!(
            "comb: {}f0 = {f:.2} Hz: engine {:.4e}, script {:.4e} urad/rtHz",
            n + 1,
            height(&clean, *f),
            height(&script, *f)
        );
        // The engine's defaults and the script's constants and sampling differ by a few per cent at most.
        assert!((height(&clean, *f) / height(&script, *f) - 1.0).abs() < 0.1);
    }
    // With the script's shot noise: the bins that stand more than six times above the median are
    // the first three harmonics and nothing else. The fourth is under the floor at this length.
    let noisy =
        periodogram(&sum(&[&series, &fixture("scripts/fig2_10m/noise_b.npy")]), dt, Classical::SCRIPTS);
    let mut sorted: Vec<f64> = noisy.power.iter().map(|p| p.sqrt()).collect();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    let above: Vec<f64> = (0..noisy.power.len())
        .filter(|&k| noisy.power[k].sqrt() > 6.0 * median)
        .map(|k| noisy.frequencies[k])
        .collect();
    eprintln!("comb: median floor {:.1} urad/rtHz; above six times it: {above:?}", median * 1e6);
    assert!(same_bins(&above, &harmonics[..3], width), "{above:?}");
    assert!(height(&clean, 0.20) < 6.0 * median * 1e6, "the fourth harmonic is under the threshold here");
    // Control: a tone with no comb has one bin, not four.
    let tone: Vec<f64> = (0..series.len()).map(|k| 1e-3 * (TAU * f0 * 2.0 * k as f64).cos()).collect();
    let single = periodogram(&tone, dt, Classical::SCRIPTS);
    assert!(height(&single, 0.10) < 1e-9 * height(&single, 0.05));
}

#[test]
fn folded_comb_fingerprint() {
    // Panel c on both instruments: the oscillator is above Nyquist and every harmonic folds.
    for (dir, instrument, dt, f0) in [
        ("fig2_10m", Instrument::aion10(), 2.0, 0.3275),
        ("fig2_uldm_folding", Instrument::aice(), 5.0, 0.131),
    ] {
        let nyquist = 0.5 / dt;
        let series = engine_series(instrument, dir, 0.5, f0);
        let width = 1.0 / (series.len() as f64 * dt);
        let mut predicted: Vec<f64> = (1..=4).map(|n| fold_frequency(f64::from(n) * f0, nyquist)).collect();
        predicted.sort_by(f64::total_cmp);
        let ours = largest(&periodogram(&series, dt, Classical::SCRIPTS), 4);
        let theirs = largest(&script_spectrum(dir, "S_clean_c"), 4);
        eprintln!("{dir}: folds predicted {predicted:?}; engine {ours:?}; script {theirs:?}");
        assert!(
            same_bins(&ours, &predicted, width),
            "{dir}: the engine's peaks {ours:?}, predicted {predicted:?}"
        );
        assert!(same_bins(&theirs, &predicted, width), "{dir}: the script's peaks {theirs:?}");
        // Control: unfolded, none of the harmonics is below Nyquist to be a bin at all.
        assert!((1..=4).all(|n| f64::from(n) * f0 > nyquist));
    }
}

#[test]
fn harmonic_heights_fall_as_amplitude_ratio() {
    // The paper's law is (A/√X)ⁿ with X its squared distance scale; the paper is not in the
    // repository, so X is not checked. What is: the n-th harmonic over the fundamental grows as
    // A^(n−1), for the oscillator of fig2_10m panel b at four amplitudes.
    let amplitudes = [0.05, 0.1, 0.2, 0.4];
    let ratios: Vec<[f64; 2]> = amplitudes
        .iter()
        .map(|&amp| {
            let spectrum = periodogram(
                &engine_series(Instrument::aion10(), "fig2_10m", amp, 0.05),
                2.0,
                Classical::SCRIPTS,
            );
            let height = |f: f64| spectrum.power[spectrum.bin_of(f).unwrap()].sqrt();
            [height(0.10) / height(0.05), height(0.15) / height(0.05)]
        })
        .collect();
    for (n, expected) in [(0, 1.0), (1, 2.0)] {
        // The slope of log(ratio) against log(A), by least squares.
        let points: Vec<(f64, f64)> = amplitudes.iter().zip(&ratios).map(|(a, r)| (*a, r[n])).collect();
        let slope = cav_validate::fitted_order(&points).unwrap();
        eprintln!("harmonic {}: ratio to the fundamental {:?}; slope {slope:.4}", n + 2, points);
        assert!(
            (slope / expected - 1.0).abs() < 0.10,
            "harmonic {}: slope {slope}, expected {expected}",
            n + 2
        );
    }
    // Control: the fundamental itself grows as A, not as A² or A⁰.
    let fundamental: Vec<(f64, f64)> = amplitudes
        .iter()
        .map(|&amp| {
            let spectrum = periodogram(
                &engine_series(Instrument::aion10(), "fig2_10m", amp, 0.05),
                2.0,
                Classical::SCRIPTS,
            );
            (amp, spectrum.power[spectrum.bin_of(0.05).unwrap()].sqrt())
        })
        .collect();
    assert!((cav_validate::fitted_order(&fundamental).unwrap() - 1.0).abs() < 0.05);
}
