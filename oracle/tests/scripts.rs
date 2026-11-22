//! Parity with George's reference scripts, through the fixtures of `fixtures/scripts/`.
//!
//! Each phase series is kept twice (`PROVENANCE.md`): as the scripts publish it, and with only
//! the one-step offset between arm and source corrected, which George has said is not intended.
//!
//! - **Published** is matched by `Quadrature::ScriptTrapezoid { lag: 1 }`, a rule that copies the
//!   scripts' sampling. It proves the geometry, the constants, the floor, the timing and the
//!   kernel, to 1e-9 of the peak.
//! - **Same-time** is what the engine's own model is held to, in three steps. The fixture is the
//!   scripts' rule with the offset gone (the tie, 1e-9). That rule on the whole flight, taken to
//!   its limit, is the engine (the limit: two Richardson steps on AICE, a step of dt/128 on
//!   AION-10, where the arms land between nodes). And the engine differs from each fixture by
//!   what the rule's own error predicts, shot by shot (the gap, two-sided, 2%).
//!
//! The gap is not a check independent of the limit. Given the tie, the engine minus the fixture
//! less the limit minus the rule is the engine minus the limit, so the gap is the limit's number
//! divided by the size of the predicted difference: the same measurement on another scale. What
//! it adds is the record of how large the scripts' own error is, and of its parts.
//!
//! The scripts' error has two parts: their window stops one step short of 2T, which is the
//! larger wherever it applies, and the trapezoid itself. On `fig2_10m` the two have opposite
//! signs. A single bound of 3e-4 on the engine-against-fixture difference failed on one series
//! (`100m_oscillators`) and passed on seven, two of them (`fig2_10m` c and d, whose window alone
//! is over the bound) only by that cancellation; the review replaced it (`docs/slices/S1.md`
//! §3.1, §7.2, §7.5).
//!
//! The engine is run with the scripts' constants (their mass, ħ and G) in both.

use std::f64::consts::TAU;

use cav_core::Vec3;
use cav_core::forward::{Instrument, Mass, PHASE_TOLERANCE, Quadrature, Reader};
use oracle::{fixture, worst_of_peak};

/// The oscillating point mass of a script: at horizontal distance `d`, about height `h0`, with
/// amplitude `amp` at `f` hertz. `circular` is the scripts' circle in the D–h plane.
#[derive(Clone, Copy, Debug)]
struct Oscillator {
    mass: f64,
    d: f64,
    h0: f64,
    amp: f64,
    f: f64,
    circular: bool,
}

impl Oscillator {
    fn at(&self, t: f64) -> Vec3 {
        let phase = TAU * self.f * t;
        if self.circular {
            Vec3::new(self.d + self.amp * phase.cos(), 0.0, self.h0 + self.amp * phase.sin())
        } else {
            Vec3::new(self.d, 0.0, self.h0 + self.amp * phase.cos())
        }
    }
}

/// One series of one script.
#[derive(Clone, Copy, Debug)]
struct Series {
    /// The fixture directory.
    dir: &'static str,
    /// The file stem before `_published` or `_same_time`.
    stem: &'static str,
    /// The 100 m instrument, or AION-10.
    aice: bool,
    /// How many nodes the script's trapezoid has.
    nodes: usize,
    oscillator: Oscillator,
}

const FIG2: Oscillator = Oscillator { mass: 1.0, d: 3.0, h0: 1.0, amp: 0.5, f: 0.0, circular: false };

const ONE_D: Series = Series {
    dir: "1d_oscillator",
    stem: "phase",
    aice: false,
    nodes: 147,
    oscillator: Oscillator { mass: 1.0, d: 5.0, h0: 1.0, amp: 1.0, f: 0.1, circular: false },
};
const HUNDRED_M: Series = Series {
    dir: "100m_oscillators",
    stem: "phase",
    aice: true,
    nodes: 500,
    oscillator: Oscillator { mass: 0.05, d: 1.0, h0: 1.0, amp: 0.5, f: 0.08, circular: false },
};
const fn fig2_10m(stem: &'static str, f: f64, circular: bool) -> Series {
    Series { dir: "fig2_10m", stem, aice: false, nodes: 146, oscillator: Oscillator { f, circular, ..FIG2 } }
}
const fn fig2_aice(stem: &'static str, f: f64, circular: bool) -> Series {
    Series {
        dir: "fig2_uldm_folding",
        stem,
        aice: true,
        nodes: 500,
        oscillator: Oscillator { f, circular, ..FIG2 },
    }
}
const TEN_B: Series = fig2_10m("raw_b", 0.05, false);
const TEN_C: Series = fig2_10m("raw_c", 0.3275, false);
const TEN_D: Series = fig2_10m("raw_d", 0.35625, true);
const AICE_B: Series = fig2_aice("raw_b", 0.02, false);
const AICE_C: Series = fig2_aice("raw_c", 0.131, false);
const AICE_D: Series = fig2_aice("raw_d", 0.1425, true);

/// The instrument of a script: its geometry and its constants, the floor on, lower minus upper.
fn script_instrument(aice: bool) -> Instrument {
    if aice {
        let mut instrument = Instrument::aice().with_script_constants();
        // The scripts' launch rule, with the scripts' kick: u₀ = gT − v_kick/2.
        instrument.launch_velocity = instrument.symmetric_launch();
        instrument
    } else {
        Instrument::aion10().with_script_constants()
    }
}

impl Series {
    fn file(&self, which: &str) -> Vec<f64> {
        fixture(&format!("scripts/{}/{}_{which}.npy", self.dir, self.stem))
    }

    fn times(&self) -> Vec<f64> {
        fixture(&format!("scripts/{}/t.npy", self.dir))
    }

    /// The series with the scripts' own sampling: `lag = 1` as published, `0` with the offset gone.
    fn copied(&self, lag: usize) -> Vec<f64> {
        let quadrature = Quadrature::ScriptTrapezoid { step: 0.01, nodes: self.nodes, lag };
        let reader = Reader::new(Instrument { quadrature, ..script_instrument(self.aice) });
        let oscillator = self.oscillator;
        let source = Mass::point(oscillator.mass, move |t| oscillator.at(t));
        self.times().iter().map(|&t| reader.phase(Vec3::ZERO, &source, t)).collect()
    }

    /// The scripts' rule with the offset gone at `step`, on the window `[0, 2T − step]` as the
    /// `fig2_*` and `100m` scripts place it (`whole` false), or on the whole flight `[0, 2T]`.
    fn rule(&self, step: f64, whole: bool) -> Vec<f64> {
        let instrument = script_instrument(self.aice);
        let nodes = (instrument.flight_time() / step).round() as usize + usize::from(whole);
        let quadrature = Quadrature::ScriptTrapezoid { step, nodes, lag: 0 };
        let reader = Reader::new(Instrument { quadrature, ..instrument });
        let oscillator = self.oscillator;
        let source = Mass::point(oscillator.mass, move |t| oscillator.at(t));
        self.times().iter().map(|&t| reader.phase(Vec3::ZERO, &source, t)).collect()
    }

    /// The scripts' rule at another step, on the scripts' window.
    fn copied_at(&self, step: f64) -> Vec<f64> {
        self.rule(step, false)
    }

    /// The scripts' rule on the whole flight taken to its limit. On AICE the arms close exactly
    /// at 2T and every kink is on a node, so the trapezoid's error is a series in the step
    /// squared and two Richardson steps remove its first two terms. On AION-10 the arms land
    /// between nodes and it is not; there the rule is simply run at dt/128, where what is left of
    /// it is 1e-9 to 5e-9 of the peak and still falling (`docs/slices/S1.md` §7.5).
    fn limit(&self) -> Vec<f64> {
        const DT: f64 = 0.01;
        if !self.aice {
            return self.rule(DT / 128.0, true);
        }
        let (t1, t2, t4) = (self.rule(DT, true), self.rule(DT / 2.0, true), self.rule(DT / 4.0, true));
        (0..t1.len())
            .map(|k| {
                let (first, second) = ((4.0 * t2[k] - t1[k]) / 3.0, (4.0 * t4[k] - t2[k]) / 3.0);
                (16.0 * second - first) / 15.0
            })
            .collect()
    }

    /// R10, the limit: the scripts' rule taken to its limit against the engine's own model.
    fn limit_error(&self) -> f64 {
        let engine = self.engine();
        // Control: the rule at the scripts' own step is not the limit, and this measure sees it.
        let coarse = worst_of_peak(&self.rule(0.01, true), &engine);
        assert!(
            coarse > 1e-6,
            "{}/{}: the rule at dt is already {coarse:e} from the engine",
            self.dir,
            self.stem
        );
        worst_of_peak(&self.limit(), &engine)
    }

    /// The series from the engine's own model: its Gauss rule, read until it stops changing.
    fn engine(&self) -> Vec<f64> {
        let instrument = script_instrument(self.aice);
        let reader = Reader::new(instrument);
        let oscillator = self.oscillator;
        let source = Mass::point(oscillator.mass, move |t| oscillator.at(t));
        self.times()
            .iter()
            .map(|&t| {
                reader.read_to(Vec3::ZERO, &source, t, PHASE_TOLERANCE).reading.gradiometer(instrument.sign)
            })
            .collect()
    }

    /// R9: the script-copying rule against the script as published.
    fn published(&self) -> f64 {
        let reference = self.file("published");
        // Control: without the one-step offset the same rule is visibly not the published script,
        // so this comparison is one that can tell.
        let without = worst_of_peak(&self.copied(0), &reference);
        assert!(
            without > 1e-5,
            "{}/{}: the offset changes the series by only {without:e}",
            self.dir,
            self.stem
        );
        worst_of_peak(&self.copied(1), &reference)
    }

    /// R10, the gap: the engine minus the corrected fixture against what the scripts' rule's own
    /// error predicts for it, the limit minus the rule at the scripts' step and window. Returned as
    /// the largest difference between the two, shot by shot, as a fraction of the largest predicted
    /// gap: two-sided, so an engine that drifted either way would fail it. With the tie holding,
    /// this is `limit_error` times the peak over the largest predicted gap: the limit rescaled,
    /// not a second check.
    fn same_time(&self) -> f64 {
        let corrected = self.file("same_time");
        let ours = self.engine();
        // The tie: the fixture is the scripts' rule with the offset gone.
        let at_dt = self.copied(0);
        let tie = worst_of_peak(&at_dt, &corrected);
        assert!(
            tie < 1e-9,
            "{}/{}: the same-time fixture is not the scripts' rule with lag 0: {tie:e}",
            self.dir,
            self.stem
        );
        let limit = self.limit();
        let peak = corrected.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let (mut predicted, mut off) = (0.0f64, 0.0f64);
        for k in 0..ours.len() {
            let (measured, expected) = (ours[k] - corrected[k], limit[k] - at_dt[k]);
            predicted = predicted.max(expected.abs());
            off = off.max((measured - expected).abs());
        }
        // The parts of the scripts' error, for the record: the window that ends a step short, and
        // the trapezoid on the whole flight.
        let whole = self.rule(0.01, true);
        let truncation = worst_of_peak(&whole, &at_dt);
        let trapezoid = worst_of_peak(&whole, &limit);
        eprintln!(
            "{}/{}: engine against the fixture {:e} of peak; predicted {:e}; apart by {:e} of the prediction; \
             the window a step short {truncation:e}; the trapezoid {trapezoid:e}",
            self.dir,
            self.stem,
            worst_of_peak(&ours, &corrected),
            predicted / peak,
            off / predicted
        );
        // Control: the prediction is not nothing. An engine that simply equalled the fixture
        // would be a whole prediction away.
        assert!(predicted / peak > 1e-6, "{}/{}: nothing is predicted", self.dir, self.stem);
        off / predicted
    }
}

// Whose error is the gap on `100m_oscillators`? Its source is 1 m from the atoms' path, which
// AICE's atoms pass at up to 21 m/s: 0.05 s, five of the scripts' steps. The two cases below were
// written, with what each would show, after `oracle.script_same_time_100m_oscillators` first
// measured 1.07e-3 against its 3e-4 (docs/slices/S1.md §7.1).

cav_validate::case! {
    test: validation_script_rule_converges_to_the_engine,
    name: "oracle.script_rule_converges_100m",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "the scripts' trapezoid on the 100m_oscillators scene, offset gone, on the scripts' own window (which ends one step short of 2T), at steps of 0.01, 0.005, 0.0025, 0.00125 s (the first is the fixture's rule, held to it to 1e-9) against the engine's own model: both the missing step and the trapezoid shrink as the step does; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 3e-5,
    order: (2.0, 0.2),
    measure: {
        let engine = HUNDRED_M.engine();
        // The first point is the rule that made the fixture: it reproduces it.
        assert!(worst_of_peak(&HUNDRED_M.copied_at(0.01), &HUNDRED_M.file("same_time")) < 1e-9);
        let points = [0.01, 0.005, 0.0025, 0.00125]
            .iter()
            .map(|&step| (step, worst_of_peak(&HUNDRED_M.copied_at(step), &engine)))
            .collect();
        cav_validate::Measured::Convergence(points)
    },
}

cav_validate::case! {
    test: validation_engine_rule_is_converged_on_the_100m_scene,
    name: "oracle.engine_rule_converged_100m",
    subsystem: "oracle",
    slice: "S1",
    reference: Exact,
    against: "the engine's audited read on the 100m_oscillators scene (a source 1 m from the atoms) against 12 nodes on 3 ms panels, 4000 shots; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-9,
    measure: {
        let fine = Quadrature::Gauss { order: 12, panel: 0.1 / 32.0 };
        let reader = Reader::new(Instrument { quadrature: fine, ..script_instrument(true) });
        let oscillator = HUNDRED_M.oscillator;
        let source = Mass::point(oscillator.mass, move |t| oscillator.at(t));
        let reference: Vec<f64> =
            HUNDRED_M.times().iter().map(|&t| reader.phase(Vec3::ZERO, &source, t)).collect();
        // Control: the engine's fixed rule, without the audit, is not converged this close.
        let fixed = Reader::new(script_instrument(true));
        let unaudited: Vec<f64> =
            HUNDRED_M.times().iter().map(|&t| fixed.phase(Vec3::ZERO, &source, t)).collect();
        let without = worst_of_peak(&unaudited, &reference);
        eprintln!("100m scene: the fixed rule without the audit is {without:e} of the peak from the fine rule");
        assert!(without > 1e-6, "the fixed rule is already within {without:e}: the audit is not being tested");
        cav_validate::Measured::Error(worst_of_peak(&HUNDRED_M.engine(), &reference))
    },
}

#[test]
fn point_oscillator_phase_matches_script() {
    // VALIDATION-AND-RESEARCH §2.1: the phase series of a 1-D vertical point-mass oscillator, on
    // AION-10 (`1D_oscillator`) and on AICE (`100m_oscillators`), to 1e-9 relative. Held by the
    // rule that copies the scripts' sampling, as the owner ruled (plan/PROGRESS.md).
    for series in [ONE_D, HUNDRED_M] {
        let error = series.published();
        assert!(error <= 1e-9, "{}: {error:e} of the peak", series.dir);
    }
    // Control: the wrong oscillator (a tenth of a hertz out) is nowhere near.
    let detuned = Series { oscillator: Oscillator { f: 0.2, ..ONE_D.oscillator }, ..ONE_D };
    assert!(worst_of_peak(&detuned.copied(1), &ONE_D.file("published")) > 0.1);
}

/// The largest difference, in metres, between a script's four arm tables and the closed-form
/// arms at (j + 1)·dt.
fn arm_tables(dir: &str, aice: bool) -> f64 {
    let instrument = script_instrument(aice);
    let [lower, upper] = instrument.arms();
    let mut worst = 0.0f64;
    for (file, arm, height) in [
        ("lower1", lower, 0.0),
        ("upper1", upper, 0.0),
        ("lower2", lower, instrument.ifo_separation),
        ("upper2", upper, instrument.ifo_separation),
    ] {
        let table = fixture(&format!("scripts/{dir}/{file}.npy"));
        assert!(table.len() >= 146, "{dir}/{file} has {} entries", table.len());
        for (j, z) in table.iter().enumerate() {
            worst = worst.max((height + arm.rise((j + 1) as f64 * 0.01) - z).abs());
        }
    }
    worst
}

cav_validate::case! {
    test: validation_arm_tables_match_the_scripts,
    name: "oracle.script_arm_tables",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "the four arm tables of 1D_oscillator, fig2_10m, 100m_oscillators and fig2_uldm_folding against the closed-form arms with the floor, at (j + 1)·dt; largest difference",
    unit: Metres,
    tolerance: 1e-12,
    measure: {
        // Control: with the floor off the AION-10 arms go 7 cm below what the scripts hold.
        let [bare, _] = Instrument { floor: false, ..script_instrument(false) }.arms();
        let table = fixture("scripts/fig2_10m/lower1.npy");
        assert!((bare.rise(1.46) - table[145]).abs() > 0.05);
        let worst = [("1d_oscillator", false), ("fig2_10m", false), ("100m_oscillators", true), ("fig2_uldm_folding", true)]
            .into_iter()
            .fold(0.0f64, |worst, (dir, aice)| worst.max(arm_tables(dir, aice)));
        cav_validate::Measured::Error(worst)
    },
}

cav_validate::case! {
    test: validation_script_published_1d_oscillator,
    name: "oracle.script_published_1d_oscillator",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "1D_oscillator as published (AION-10, 1 kg, D = 5 m, 0.1 Hz, 1000 shots), by the rule that copies its sampling; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-9,
    measure: cav_validate::Measured::Error(ONE_D.published()),
}

cav_validate::case! {
    test: validation_script_published_100m_oscillators,
    name: "oracle.script_published_100m_oscillators",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "100m_oscillators as published (AICE, 0.05 kg, D = 1 m, 0.08 Hz, 4000 shots), by the rule that copies its sampling; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-9,
    measure: cav_validate::Measured::Error(HUNDRED_M.published()),
}

cav_validate::case! {
    test: validation_script_published_fig2_10m_b,
    name: "oracle.script_published_fig2_10m_b",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_10m panel b as published (AION-10, 0.05 Hz, 10000 shots), by the rule that copies its sampling; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-9,
    measure: cav_validate::Measured::Error(TEN_B.published()),
}

cav_validate::case! {
    test: validation_script_published_fig2_10m_c,
    name: "oracle.script_published_fig2_10m_c",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_10m panel c as published (AION-10, 0.3275 Hz, above Nyquist), by the rule that copies its sampling; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-9,
    measure: cav_validate::Measured::Error(TEN_C.published()),
}

cav_validate::case! {
    test: validation_script_published_fig2_10m_d,
    name: "oracle.script_published_fig2_10m_d",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_10m panel d as published (AION-10, a circle in the D-h plane at 0.35625 Hz), by the rule that copies its sampling; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-9,
    measure: cav_validate::Measured::Error(TEN_D.published()),
}

cav_validate::case! {
    test: validation_script_published_fig2_uldm_folding_b,
    name: "oracle.script_published_fig2_uldm_folding_b",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_uldm_folding panel b as published (AICE, 0.02 Hz, 4000 shots), by the rule that copies its sampling; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-9,
    measure: cav_validate::Measured::Error(AICE_B.published()),
}

cav_validate::case! {
    test: validation_script_published_fig2_uldm_folding_c,
    name: "oracle.script_published_fig2_uldm_folding_c",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_uldm_folding panel c as published (AICE, 0.131 Hz, above Nyquist), by the rule that copies its sampling; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-9,
    measure: cav_validate::Measured::Error(AICE_C.published()),
}

cav_validate::case! {
    test: validation_script_published_fig2_uldm_folding_d,
    name: "oracle.script_published_fig2_uldm_folding_d",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_uldm_folding panel d as published (AICE, a circle in the D-h plane at 0.1425 Hz), by the rule that copies its sampling; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-9,
    measure: cav_validate::Measured::Error(AICE_D.published()),
}

cav_validate::case! {
    test: validation_script_limit_1d_oscillator,
    name: "oracle.script_limit_1d_oscillator",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "1D_oscillator: the scripts' trapezoid with the offset gone, on the whole flight, taken to its limit by a step of dt/128, against the engine's own model; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-8,
    measure: cav_validate::Measured::Error(ONE_D.limit_error()),
}

cav_validate::case! {
    test: validation_script_same_time_1d_oscillator,
    name: "oracle.script_same_time_1d_oscillator",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "1D_oscillator with only the one-step offset corrected: the engine minus the fixture against what the scripts' rule predicts (its limit minus the rule at the scripts' step and window), shot by shot; largest difference as a fraction of the largest predicted gap. Two-sided: it replaced a bound on the difference itself (acknowledged.txt)",
    unit: Relative,
    tolerance: 0.02,
    measure: cav_validate::Measured::Error(ONE_D.same_time()),
}

cav_validate::case! {
    test: validation_script_limit_100m_oscillators,
    name: "oracle.script_limit_100m_oscillators",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "100m_oscillators: the scripts' trapezoid with the offset gone, on the whole flight, taken to its limit by two Richardson steps (dt, dt/2, dt/4), against the engine's own model; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-9,
    measure: cav_validate::Measured::Error(HUNDRED_M.limit_error()),
}

cav_validate::case! {
    test: validation_script_same_time_100m_oscillators,
    name: "oracle.script_same_time_100m_oscillators",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "100m_oscillators with only the one-step offset corrected: the engine minus the fixture against what the scripts' rule predicts (its limit minus the rule at the scripts' step and window), shot by shot; largest difference as a fraction of the largest predicted gap. Two-sided: it replaced a bound on the difference itself (acknowledged.txt)",
    unit: Relative,
    tolerance: 0.02,
    measure: cav_validate::Measured::Error(HUNDRED_M.same_time()),
}

cav_validate::case! {
    test: validation_script_limit_fig2_10m_b,
    name: "oracle.script_limit_fig2_10m_b",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_10m panel b: the scripts' trapezoid with the offset gone, on the whole flight, taken to its limit by a step of dt/128, against the engine's own model; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-8,
    measure: cav_validate::Measured::Error(TEN_B.limit_error()),
}

cav_validate::case! {
    test: validation_script_same_time_fig2_10m_b,
    name: "oracle.script_same_time_fig2_10m_b",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_10m panel b with only the one-step offset corrected: the engine minus the fixture against what the scripts' rule predicts (its limit minus the rule at the scripts' step and window), shot by shot; largest difference as a fraction of the largest predicted gap. Two-sided: it replaced a bound on the difference itself (acknowledged.txt)",
    unit: Relative,
    tolerance: 0.02,
    measure: cav_validate::Measured::Error(TEN_B.same_time()),
}

cav_validate::case! {
    test: validation_script_limit_fig2_10m_c,
    name: "oracle.script_limit_fig2_10m_c",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_10m panel c (0.3275 Hz): the scripts' trapezoid with the offset gone, on the whole flight, taken to its limit by a step of dt/128, against the engine's own model; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-8,
    measure: cav_validate::Measured::Error(TEN_C.limit_error()),
}

cav_validate::case! {
    test: validation_script_same_time_fig2_10m_c,
    name: "oracle.script_same_time_fig2_10m_c",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_10m panel c (0.3275 Hz) with only the one-step offset corrected: the engine minus the fixture against what the scripts' rule predicts (its limit minus the rule at the scripts' step and window), shot by shot; largest difference as a fraction of the largest predicted gap. Two-sided: it replaced a bound on the difference itself (acknowledged.txt)",
    unit: Relative,
    tolerance: 0.02,
    measure: cav_validate::Measured::Error(TEN_C.same_time()),
}

cav_validate::case! {
    test: validation_script_limit_fig2_10m_d,
    name: "oracle.script_limit_fig2_10m_d",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_10m panel d (a circle at 0.35625 Hz): the scripts' trapezoid with the offset gone, on the whole flight, taken to its limit by a step of dt/128, against the engine's own model; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-8,
    measure: cav_validate::Measured::Error(TEN_D.limit_error()),
}

cav_validate::case! {
    test: validation_script_same_time_fig2_10m_d,
    name: "oracle.script_same_time_fig2_10m_d",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_10m panel d (a circle at 0.35625 Hz) with only the one-step offset corrected: the engine minus the fixture against what the scripts' rule predicts (its limit minus the rule at the scripts' step and window), shot by shot; largest difference as a fraction of the largest predicted gap. Two-sided: it replaced a bound on the difference itself (acknowledged.txt)",
    unit: Relative,
    tolerance: 0.02,
    measure: cav_validate::Measured::Error(TEN_D.same_time()),
}

cav_validate::case! {
    test: validation_script_limit_fig2_uldm_folding_b,
    name: "oracle.script_limit_fig2_uldm_folding_b",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_uldm_folding panel b: the scripts' trapezoid with the offset gone, on the whole flight, taken to its limit by two Richardson steps (dt, dt/2, dt/4), against the engine's own model; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-9,
    measure: cav_validate::Measured::Error(AICE_B.limit_error()),
}

cav_validate::case! {
    test: validation_script_same_time_fig2_uldm_folding_b,
    name: "oracle.script_same_time_fig2_uldm_folding_b",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_uldm_folding panel b with only the one-step offset corrected: the engine minus the fixture against what the scripts' rule predicts (its limit minus the rule at the scripts' step and window), shot by shot; largest difference as a fraction of the largest predicted gap. Two-sided: it replaced a bound on the difference itself (acknowledged.txt)",
    unit: Relative,
    tolerance: 0.02,
    measure: cav_validate::Measured::Error(AICE_B.same_time()),
}

cav_validate::case! {
    test: validation_script_limit_fig2_uldm_folding_c,
    name: "oracle.script_limit_fig2_uldm_folding_c",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_uldm_folding panel c (0.131 Hz): the scripts' trapezoid with the offset gone, on the whole flight, taken to its limit by two Richardson steps (dt, dt/2, dt/4), against the engine's own model; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-9,
    measure: cav_validate::Measured::Error(AICE_C.limit_error()),
}

cav_validate::case! {
    test: validation_script_same_time_fig2_uldm_folding_c,
    name: "oracle.script_same_time_fig2_uldm_folding_c",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_uldm_folding panel c (0.131 Hz) with only the one-step offset corrected: the engine minus the fixture against what the scripts' rule predicts (its limit minus the rule at the scripts' step and window), shot by shot; largest difference as a fraction of the largest predicted gap. Two-sided: it replaced a bound on the difference itself (acknowledged.txt)",
    unit: Relative,
    tolerance: 0.02,
    measure: cav_validate::Measured::Error(AICE_C.same_time()),
}

cav_validate::case! {
    test: validation_script_limit_fig2_uldm_folding_d,
    name: "oracle.script_limit_fig2_uldm_folding_d",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_uldm_folding panel d (a circle at 0.1425 Hz): the scripts' trapezoid with the offset gone, on the whole flight, taken to its limit by two Richardson steps (dt, dt/2, dt/4), against the engine's own model; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-9,
    measure: cav_validate::Measured::Error(AICE_D.limit_error()),
}

cav_validate::case! {
    test: validation_script_same_time_fig2_uldm_folding_d,
    name: "oracle.script_same_time_fig2_uldm_folding_d",
    subsystem: "oracle",
    slice: "S1",
    reference: Script,
    against: "fig2_uldm_folding panel d (a circle at 0.1425 Hz) with only the one-step offset corrected: the engine minus the fixture against what the scripts' rule predicts (its limit minus the rule at the scripts' step and window), shot by shot; largest difference as a fraction of the largest predicted gap. Two-sided: it replaced a bound on the difference itself (acknowledged.txt)",
    unit: Relative,
    tolerance: 0.02,
    measure: cav_validate::Measured::Error(AICE_D.same_time()),
}
