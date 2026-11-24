//! The v1 tie: with v1's conventions the engine reads what v1's own forward model reads.
//!
//! v1 is compiled here from tag `v1-final` (`instrument`, `gravity`, `source`, `math`) and run on
//! the anchors of `cav_core::anchors`. The engine is run with `Instrument::with_v1_conventions()`:
//! no floor, v1's mass and ħ, upper minus lower, Simpson at 0.01 s on each half. Nothing else is
//! different about it: the floor, the mass, the sign and the rule are fields, not a second path.

use cav_core::anchors::{self, Anchor};
use cav_core::forward::{Instrument, Mass, Reader, Sign};
use cav_core::{Vec3, defaults};
use instrument::{Detector, PhaseModel, PropagationIntegral};
use math::{Isometry3, Quat};
use oracle::{fixture, fixtures, worst_of_peak};

/// The anchor's series as v1 computes it: a one-element cloud carried along the path.
fn v1_series(anchor: &Anchor, tower: Vec3) -> Vec<f64> {
    let place = |p: Vec3| Isometry3::new(Quat::identity(), math::Vec3::new(p.x, p.y, p.z));
    let path = anchor.path;
    let cloud = gravity::Cloud::from_elements(&[(0.0, 0.0, 0.0, anchor.mass)]);
    let source = source::Prescribed::new(cloud, move |t| place(path(t)));
    let detector = Detector::placed(place(tower));
    let model = PropagationIntegral::default();
    anchor.times().map(|t| model.delta_phi(&[&source], &[], &detector, t)).collect()
}

/// The same series from the engine's forward model with the given instrument.
fn engine_series(instrument: Instrument, anchor: &Anchor, tower: Vec3) -> Vec<f64> {
    let reader = Reader::new(instrument);
    let source = Mass::point(anchor.mass, anchor.path);
    anchor.times().map(|t| reader.phase(tower, &source, t)).collect()
}

fn v1_conventions() -> Instrument {
    Instrument::aion10().with_v1_conventions()
}

/// The engine against v1 on one anchor, with the control arms: each convention set wrong, one at
/// a time, must be seen.
fn tie(anchor: &Anchor, tower: Vec3) -> f64 {
    let v1 = v1_series(anchor, tower);
    let right = v1_conventions();
    let wrong: [(&str, Instrument); 4] = [
        ("the floor on", Instrument { floor: true, ..right }),
        ("the mass of strontium 87", Instrument { atom_mass: cav_core::constants::SR87_MASS, ..right }),
        ("lower minus upper", Instrument { sign: Sign::LowerMinusUpper, ..right }),
        ("the Gauss rule of the engine", Instrument { quadrature: Instrument::aion10().quadrature, ..right }),
    ];
    for (what, instrument) in wrong {
        let error = worst_of_peak(&engine_series(instrument, anchor, tower), &v1);
        eprintln!("{}: with {what} the engine is {error:e} of the peak from v1", anchor.name);
        assert!(error > 1e-9, "{}: with {what} the engine is still within {error:e} of v1", anchor.name);
    }
    worst_of_peak(&engine_series(right, anchor, tower), &v1)
}

cav_validate::case! {
    test: validation_v1_static_point,
    name: "oracle.v1_static_point",
    subsystem: "oracle",
    slice: "S1",
    reference: V1Oracle,
    against: "v1's PropagationIntegral at v1-final on 100 kg at rest 2 m from the tower, engine set to v1's conventions; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-10,
    measure: cav_validate::Measured::Error(tie(&anchors::STATIC, Vec3::ZERO)),
}

cav_validate::case! {
    test: validation_v1_moving_pass,
    name: "oracle.v1_moving_pass",
    subsystem: "oracle",
    slice: "S1",
    reference: V1Oracle,
    against: "v1's PropagationIntegral on the published moving anchor (10 kg, D = 10 m, 2.5 m/s), 21 shots, engine set to v1's conventions; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-10,
    measure: cav_validate::Measured::Error(tie(&anchors::PASS, Vec3::ZERO)),
}

cav_validate::case! {
    test: validation_v1_oscillation,
    name: "oracle.v1_oscillation",
    subsystem: "oracle",
    slice: "S1",
    reference: V1Oracle,
    against: "v1's PropagationIntegral on the published oscillation anchor (1 kg, D = 5 m, 0.1 Hz), 41 shots, engine set to v1's conventions; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-10,
    measure: cav_validate::Measured::Error(tie(&anchors::OSCILLATION, Vec3::ZERO)),
}

/// The default array's five tower bases.
fn towers() -> Vec<Vec3> {
    defaults::TOWER_POSITIONS.iter().map(|p| Vec3::new(p[0], p[1], 0.0)).collect()
}

cav_validate::case! {
    test: validation_v1_five_towers,
    name: "oracle.v1_five_towers",
    subsystem: "oracle",
    slice: "S1",
    reference: V1Oracle,
    against: "v1's PropagationIntegral on the oscillation anchor read by each of the default array's five towers, engine set to v1's conventions; largest difference as a fraction of that tower's peak",
    unit: Relative,
    tolerance: 1e-10,
    measure: {
        let worst = towers().into_iter().fold(0.0f64, |worst, tower| worst.max(tie(&anchors::OSCILLATION, tower)));
        cav_validate::Measured::Error(worst)
    },
}

/// Each frozen series: its file under `fixtures/v1`, and what v1 computes for it now.
fn frozen() -> Vec<(String, Vec<f64>)> {
    let mut all: Vec<(String, Vec<f64>)> =
        anchors::ALL.iter().map(|a| (format!("v1/{}.npy", a.name), v1_series(a, Vec3::ZERO))).collect();
    let five: Vec<f64> = towers().into_iter().flat_map(|t| v1_series(&anchors::OSCILLATION, t)).collect();
    all.push(("v1/five_towers.npy".to_owned(), five));
    all
}

#[test]
fn the_frozen_v1_fixtures_are_what_v1_computes() {
    // `quick` holds the engine to these files without compiling v1; this test is what makes them
    // the numbers of v1 and not a copy of the engine's. They are written once, with
    // CAV_ORACLE_FREEZE set and only where no file exists yet, and never to make a test pass: a
    // difference here is a change in v1 or in the anchors.
    let freeze = std::env::var_os("CAV_ORACLE_FREEZE").is_some();
    for (file, series) in frozen() {
        let path = fixtures().join(&file);
        if freeze && !path.exists() {
            std::fs::create_dir_all(path.parent().expect("a directory")).expect("fixtures/v1");
            cav_core::npy::write(&path, &series).expect("writing the fixture");
        }
        let kept = fixture(&file);
        let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&kept), bits(&series), "{file} is not what v1 computes");
    }
    // Control: the comparison is to the bit, and the series of a tower a nanometre away differs.
    let moved = v1_series(&anchors::PASS, Vec3::new(1e-9, 0.0, 0.0));
    assert_ne!(moved, fixture("v1/pass.npy"));
}

/// Half the peak to peak of a series, and the peak to peak itself, in milliradians.
fn amplitude_mrad(series: &[f64]) -> (f64, f64) {
    let swing =
        series.iter().fold(f64::MIN, |m, &x| m.max(x)) - series.iter().fold(f64::MAX, |m, &x| m.min(x));
    (0.5e3 * swing, 1e3 * swing)
}

/// One reading of a published anchor's size: the engine's amplitude (half the peak to peak,
/// George's answer of 23 Nov 2026), its peak to peak and its largest |ΔΦ|, with v1's amplitude,
/// all in milliradians. The engine is run with its defaults at the anchor's own readouts.
struct Sizes {
    amplitude: f64,
    swing: f64,
    largest: f64,
    v1_amplitude: f64,
}

fn sizes(anchor: &Anchor) -> Sizes {
    let engine = engine_series(Instrument::aion10(), anchor, Vec3::ZERO);
    let (amplitude, swing) = amplitude_mrad(&engine);
    let largest = 1e3 * engine.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let (v1_amplitude, v1_swing) = amplitude_mrad(&v1_series(anchor, Vec3::ZERO));
    eprintln!(
        "{}: the engine: amplitude (half the peak to peak) {amplitude:.4} mrad, peak to peak {swing:.4}, \
         largest |ΔΦ| {largest:.4}; v1: amplitude {v1_amplitude:.4}, peak to peak {v1_swing:.4}",
        anchor.name
    );
    Sizes { amplitude, swing, largest, v1_amplitude }
}

cav_validate::case! {
    test: validation_published_oscillation_amplitude,
    name: "oracle.published_anchor_oscillation_amplitude",
    subsystem: "oracle",
    slice: "S2",
    reference: Paper,
    against: "the published size of the oscillation anchor, about 2 mrad (1 kg at D = 5 m, 0.1 Hz; reference/design/instrument.md §12, Fig 15), read as an amplitude, half the peak to peak (George, 23 Nov 2026): the engine's half peak to peak with its defaults over one period, 41 shots; difference as a fraction of 2 mrad",
    unit: Relative,
    tolerance: 0.1,
    measure: {
        let off = |value: f64| (value / 2.0 - 1.0).abs();
        let sizes = sizes(&anchors::OSCILLATION);
        // Controls: the peak to peak and the largest |ΔΦ| are not 2 mrad by this tolerance, so
        // the case tells the readings apart; and v1 reads the same amplitude.
        assert!(off(sizes.swing) > 0.1 && off(sizes.largest) > 0.1, "{} {}", sizes.swing, sizes.largest);
        assert!(off(sizes.v1_amplitude) <= 0.1, "v1's amplitude is {} mrad", sizes.v1_amplitude);
        cav_validate::Measured::Error(off(sizes.amplitude))
    },
}

cav_validate::case! {
    test: validation_published_pass_size,
    name: "oracle.published_anchor_pass_half_largest",
    subsystem: "oracle",
    slice: "S3",
    reference: Paper,
    against: "the published size of the moving anchor, about 7 mrad (10 kg at D = 10 m rising at 2.5 m/s; reference/design/instrument.md §12, Fig 12): George (24 Nov 2026) says it is the same pass; read, by the coordinator, as half the largest |ΔΦ| (to be confirmed by George); the engine with its defaults, as a fraction of 7 mrad; tolerance the published sizes' 10% (S2)",
    unit: Relative,
    tolerance: 0.1,
    measure: {
        let off = |value: f64| (value / 7.0 - 1.0).abs();
        let pass = sizes(&anchors::PASS);
        // Controls: read as half the peak to peak (George's first reading of the published sizes,
        // which holds for the oscillator) the same pass is 19% over, and fails; v1 reads as the
        // engine does.
        assert!(off(pass.amplitude) > 0.1, "half the peak to peak is {} mrad", pass.amplitude);
        let v1_largest = 1e3 * v1_series(&anchors::PASS, Vec3::ZERO).iter().fold(0.0f64, |m, x| m.max(x.abs()));
        assert!(off(0.5 * v1_largest) <= 0.1, "v1's half largest is {} mrad", 0.5 * v1_largest);
        cav_validate::Measured::Error(off(0.5 * pass.largest))
    },
}

#[test]
fn the_published_anchor_sizes_are_what_the_references_give() {
    // reference/design/instrument.md §12: "10 kg @ D = 10 m, 2.5 m/s → ~7 mrad; 1 kg @ 5 m osc →
    // ~2 mrad", and v1's own test comments call the second "peak-to-peak". This test first asked
    // for those two numbers within 10% and failed, for v1 as for the engine: v1 gives 13.66 mrad
    // peak and 4.34 mrad peak to peak (docs/slices/S1.md §7.1). George has since said (23 Nov
    // 2026) that the published sizes are amplitudes, half the peak to peak. For the oscillator
    // that holds and is a validation case above. For the moving anchor it does not, as measured
    // below (docs/slices/S2.md §7.13): the question has gone back to him.
    let peak = |v: &[f64]| v.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let swing =
        |v: &[f64]| v.iter().fold(f64::MIN, |m, &x| m.max(x)) - v.iter().fold(f64::MAX, |m, &x| m.min(x));
    let within = |value: f64, of: f64, fraction: f64| (value / of - 1.0).abs() < fraction;
    let mrad = 1e3;

    // The oscillator. George's `1D_oscillator` is this scene; its series is the reference.
    let script = fixture("scripts/1d_oscillator/phase_published.npy");
    let script_swing = swing(&script) * mrad;
    eprintln!(
        "oscillation, George's script: {script_swing:.4} mrad peak to peak, {:.4} mrad amplitude",
        0.5 * script_swing
    );
    assert!(within(0.5 * script_swing, 2.0, 0.10), "the script's amplitude is {} mrad", 0.5 * script_swing);
    // Control: read as peak to peak, the published figure does not fit the script that made it.
    assert!(!within(script_swing, 2.0, 0.10), "the script's peak to peak is {script_swing} mrad");
    // The engine with the defaults, read at the script's own shot times.
    let times = fixture("scripts/1d_oscillator/t.npy");
    let engine_swing = |mass: f64| {
        let reader = Reader::new(Instrument::aion10());
        let source = Mass::point(mass, anchors::OSCILLATION.path);
        let series: Vec<f64> = times.iter().map(|&t| reader.phase(Vec3::ZERO, &source, t)).collect();
        swing(&series) * mrad
    };
    let ours = 0.5 * engine_swing(anchors::OSCILLATION.mass);
    eprintln!("oscillation, the engine with the defaults: {ours:.4} mrad amplitude");
    assert!(within(ours, 2.0, 0.10), "the engine's amplitude is {ours} mrad");
    // Control: an engine a fifth too strong would fail this.
    assert!(!within(0.5 * engine_swing(1.2 * anchors::OSCILLATION.mass), 2.0, 0.10));

    // The moving anchor. Its series changes sign as the mass passes: +3.02 mrad before, −13.65
    // at the peak. Read as George said, half the peak to peak, the engine's size is 8.34 mrad,
    // which is not "~7 mrad" within the 10% that was written before it was measured; half the
    // largest |ΔΦ| is 6.83 mrad, which is. The tolerance was not moved. George has since said
    // (24 Nov 2026) that Fig 12 is this same pass, so the published figure is read as half the
    // largest |ΔΦ| (a case above, `oracle.published_anchor_pass_half_largest`; the reading is
    // the coordinator's, for George to confirm). Both facts stay asserted, and v1 says the same.
    let pass = sizes(&anchors::PASS);
    assert!(!within(pass.amplitude, 7.0, 0.10), "half the peak to peak is {} mrad", pass.amplitude);
    assert!(!within(pass.v1_amplitude, 7.0, 0.10), "v1's is {} mrad", pass.v1_amplitude);
    assert!(within(0.5 * pass.largest, 7.0, 0.10), "half the largest |ΔΦ| is {} mrad", 0.5 * pass.largest);
    // Control: the two readings differ because the series changes sign, by a lobe of 3 mrad.
    assert!(pass.swing - pass.largest > 2.5, "{} {}", pass.swing, pass.largest);
    // The engine with the defaults is held to v1's peak, allowing for the conventions that
    // differ.
    let v1 = peak(&v1_series(&anchors::PASS, Vec3::ZERO)) * mrad;
    let engine_peak = |mass: f64| {
        peak(&engine_series(Instrument::aion10(), &Anchor { mass, ..anchors::PASS }, Vec3::ZERO)) * mrad
    };
    let engine = engine_peak(anchors::PASS.mass);
    eprintln!("moving anchor: v1 peaks at {v1:.4} mrad, the engine with the defaults at {engine:.4} mrad");
    assert!(within(engine, v1, 0.05), "the engine peaks at {engine} mrad against v1's {v1}");
    // Control: an engine a tenth too strong would fail this.
    assert!(!within(engine_peak(1.1 * anchors::PASS.mass), v1, 0.05));
}
