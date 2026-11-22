//! What a schedule does to a spectrum, said from the schedule alone: the mean rate, the alias
//! lines and how tall each is expected to be, the floor the cadence itself lays down, and the
//! loss of sensitivity from shots not taken. The cadence designer shows these live; nothing here
//! simulates.
//!
//! The closed forms are `cav_core::cadence`'s, each held to a Monte Carlo there
//! (`docs/slices/S2.md` §3.3). Heights are of the spectral window's amplitude `|W|`, relative to
//! the peak at zero frequency (1): a tone at `f` has images at `f ± line` of that relative
//! height. Floors are of `|W|²`, relative to the same peak.
//!
//! Each feature of a schedule is predicted **as if it were the only one**. Where two are on
//! together (jitter and planned gaps, say) the lines of each are listed and a note says so; the
//! realisation drawn beside them is the truth for the combination.

use cav_core::cadence;
use cav_core::spectrum::Complex;

use crate::{JitterKind, JitterModel, SceneSpec};

/// One line the cadence puts in the spectral window.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Line {
    /// Where it is, hertz: a tone at `f` is imaged at `f ± frequency`.
    pub frequency: f64,
    /// Its expected height in `|W|`, relative to the peak: 1 is as tall as the tone itself.
    pub height: f64,
    /// What makes it, and whether it recedes as shots are added.
    pub what: String,
}

/// What a scene's schedule is predicted to do, from its closed forms.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Prediction {
    /// The mean time between launches, seconds.
    pub mean_cycle: f64,
    /// Half the mean shot rate, hertz: the Nyquist frequency of an even cadence at that rate.
    pub nyquist: f64,
    /// How many shots the schedule plans (launches before the end of the timeline).
    pub shots: usize,
    /// How many of them each tower takes, in the towers' order.
    pub kept: Vec<usize>,
    /// The lines, in rising frequency.
    pub lines: Vec<Line>,
    /// The flat floor the cadence lays under a tone, in `|W|²` relative to the peak: from jitter
    /// about a grid and from lost shots. Zero for a cadence that has neither.
    pub floor: f64,
    /// By how much the smallest detectable tone grows because shots were not taken, against the
    /// same span fully sampled: `√(planned / kept)` for the tower that keeps fewest. Shot noise
    /// alone; leakage from strong lines is the floor above.
    pub noise_factor: f64,
    /// What to bear in mind when reading the numbers.
    pub notes: Vec<String>,
}

fn magnitude(z: Complex) -> f64 {
    z.re.hypot(z.im)
}

/// The prediction for `spec`'s schedule.
///
/// # Errors
/// When the scene has too many shots to make ([`crate::read::shots`]).
pub fn predict(spec: &SceneSpec) -> Result<Prediction, String> {
    let (schedule, cycle) = (&spec.array.schedule, spec.array.instrument.cycle);
    let shots = crate::read::shots(spec)?;
    let n = shots.times.len();
    let kept: Vec<usize> = (0..shots.towers())
        .map(|tower| shots.taken_all(tower).iter().filter(|taken| **taken).count())
        .collect();
    let mean_cycle = schedule.mean_cycle(cycle);
    let mut lines = Vec::new();
    let mut notes = Vec::new();
    let mut floor = 0.0;
    match (schedule.alternate, schedule.jitter) {
        (Some(other), _) => {
            let period = cycle + other;
            for k in 1..=4 {
                lines.push(Line {
                    frequency: k as f64 / period,
                    height: cadence::alternating_sideband(k, cycle, other),
                    what: format!(
                        "alternation, sideband {k} of 1/{period} s: does not recede with more shots"
                    ),
                });
            }
        }
        (None, None) => lines.push(Line {
            frequency: 1.0 / cycle,
            height: 1.0,
            what: "the alias of an even cadence: as tall as the tone, for ever".into(),
        }),
        (None, Some(jitter)) if jitter.model == JitterModel::Walk => {
            let alias = 1.0 / mean_cycle;
            let phi = match jitter.kind {
                JitterKind::Positive => cadence::phi_positive(alias, cycle, jitter.delta),
                JitterKind::Symmetric => cadence::phi_symmetric(alias, cycle, jitter.delta),
            };
            lines.push(Line {
                frequency: alias,
                height: cadence::alias_ratio(phi, n.max(1)),
                what: "cycle jitter (a random walk): the alias broadens and recedes as 1/√N".into(),
            });
        }
        (None, Some(jitter)) => {
            let alias = 1.0 / cycle;
            let psi = match jitter.kind {
                JitterKind::Symmetric => cadence::psi_uniform(alias, jitter.delta).abs(),
                JitterKind::Positive => magnitude(cadence::psi_positive(alias, jitter.delta)),
            };
            lines.push(Line {
                frequency: alias,
                height: psi,
                what: "jitter about a grid: the alias is lowered and does not recede".into(),
            });
            floor += (1.0 - psi * psi) / n.max(1) as f64;
        }
    }
    if schedule.alternate.is_some() && schedule.jitter.is_some() {
        notes.push("alternation and jitter together: the lines listed are the alternation's alone".into());
    }
    if let Some(gaps) = schedule.gaps {
        let period = f64::from(gaps.period) * mean_cycle;
        for k in 1..=4 {
            lines.push(Line {
                frequency: k as f64 / period,
                height: cadence::gapped_line(k, gaps.on as usize, gaps.period as usize),
                what: format!(
                    "planned gaps ({} of every {}), line {k}: does not recede",
                    gaps.on, gaps.period
                ),
            });
        }
    }
    if let Some(window) = schedule.window {
        let duty = window.length / window.period;
        for k in 1..=4 {
            let x = std::f64::consts::PI * k as f64 * duty;
            lines.push(Line {
                frequency: k as f64 / window.period,
                height: if duty >= 1.0 { 0.0 } else { (x.sin() / x).abs() },
                what: format!(
                    "the repeating window ({} s of every {} s), line {k}: does not recede",
                    window.length, window.period
                ),
            });
        }
    }
    if schedule.loss > 0.0 {
        floor += cadence::loss_floor(schedule.loss, n.max(1));
    }
    let features =
        [schedule.jitter.is_some(), schedule.gaps.is_some(), schedule.window.is_some(), schedule.loss > 0.0];
    if features.iter().filter(|on| **on).count() > 1 {
        notes.push(
            "several features are on: each is predicted as if alone; the realisation is the combination"
                .into(),
        );
    }
    lines.sort_by(|a, b| a.frequency.total_cmp(&b.frequency));
    let fewest = kept.iter().copied().min().unwrap_or(0);
    let noise_factor = if fewest == 0 { f64::INFINITY } else { (n as f64 / fewest as f64).sqrt() };
    Ok(Prediction {
        mean_cycle,
        nyquist: 0.5 / mean_cycle,
        shots: n,
        kept,
        lines,
        floor,
        noise_factor,
        notes,
    })
}

/// The most frequencies a spectral window or an uneven periodogram is computed at.
pub const MAX_FREQUENCIES: usize = 1 << 20;

/// The spectral window of the shots tower `tower` (from 0) takes: `|W(ν)|` at `count` frequencies
/// from 0 up to `to` hertz, with the frequencies. One realisation, for the scene's seed: the
/// closed forms of [`predict`] are what it is expected to be.
///
/// The sum is direct while shots × frequencies is at most 2×10⁶, and by the non-uniform FFT
/// beyond; `count` is held to [`MAX_FREQUENCIES`].
///
/// # Errors
/// When the scene has too many shots, there is no such tower, or `to` is not a positive number.
pub fn window(spec: &SceneSpec, tower: usize, to: f64, count: usize) -> Result<(Vec<f64>, Vec<f64>), String> {
    if !(to.is_finite() && to > 0.0) {
        return Err(format!("a window runs up to a frequency above zero, not {to}"));
    }
    let shots = crate::read::shots(spec)?;
    if tower >= shots.towers() {
        return Err(format!("there is no tower {}; the scene has towers 1 to {}", tower + 1, shots.towers()));
    }
    let count = count.clamp(2, MAX_FREQUENCIES);
    let times: Vec<f64> =
        (0..shots.times.len()).filter(|&k| shots.taken(tower, k)).map(|k| shots.readout(tower, k)).collect();
    let df = to / (count - 1) as f64;
    let frequencies: Vec<f64> = (0..count).map(|k| k as f64 * df).collect();
    let power = if times.len().saturating_mul(count) <= 2_000_000 {
        cadence::window(&times, &frequencies)
    } else {
        cadence::window_fast(&times, df, count)
    };
    Ok((frequencies, power.into_iter().map(f64::sqrt).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GapSpec, JitterSpec, ScheduleSpec, dev};

    fn comb_with(edit: impl FnOnce(&mut ScheduleSpec)) -> SceneSpec {
        let mut scene = dev::comb();
        scene.duration = 2000.0;
        edit(&mut scene.array.schedule);
        assert_eq!(scene.validate(), Ok(()));
        scene
    }

    /// The realised window's height at `frequency`, by direct sum over tower 1's shots.
    fn realised(scene: &SceneSpec, frequency: f64) -> f64 {
        let shots = crate::read::shots(scene).unwrap();
        let times: Vec<f64> =
            (0..shots.times.len()).filter(|&k| shots.taken(0, k)).map(|k| shots.times[k]).collect();
        cadence::window(&times, &[frequency])[0].sqrt()
    }

    #[test]
    fn the_prediction_is_the_closed_forms() {
        // Even: one alias, as tall as the tone; nothing lost.
        let even = predict(&comb_with(|_| {})).unwrap();
        assert_eq!((even.mean_cycle, even.nyquist, even.shots), (2.0, 0.25, 1000));
        assert_eq!(even.kept, [1000; 5]);
        assert_eq!((even.lines.len(), even.lines[0].frequency, even.lines[0].height), (1, 0.5, 1.0));
        assert_eq!((even.floor, even.noise_factor), (0.0, 1.0));
        // Alternating 2 s and 2.5 s: four sidebands of 1/4.5 s at |cos(πk·2/4.5)|, and the
        // realisation has them there to three decimals (the form does not depend on N).
        let scene = comb_with(|s| s.alternate = Some(2.5));
        let alternating = predict(&scene).unwrap();
        assert_eq!(alternating.mean_cycle, 2.25);
        for (k, line) in alternating.lines.iter().enumerate() {
            let k = k + 1;
            assert!((line.frequency - k as f64 / 4.5).abs() < 1e-12);
            assert_eq!(line.height, cadence::alternating_sideband(k, 2.0, 2.5));
            assert!(
                (realised(&scene, line.frequency) - line.height).abs() < 2e-3,
                "sideband {k}: {} against {}",
                realised(&scene, line.frequency),
                line.height
            );
        }
        // Planned gaps, 3 of every 5: lines at k/(5·2 s), and the realisation agrees.
        let scene = comb_with(|s| s.gaps = Some(GapSpec { on: 3, period: 5 }));
        let gapped = predict(&scene).unwrap();
        assert_eq!(gapped.kept, [600; 5]);
        assert!((gapped.noise_factor - (1000.0f64 / 600.0).sqrt()).abs() < 1e-12);
        let first = gapped.lines.iter().find(|line| line.what.starts_with("planned gaps")).unwrap();
        assert_eq!((first.frequency, first.height), (0.1, cadence::gapped_line(1, 3, 5)));
        assert!((realised(&scene, 0.1) - first.height).abs() < 1e-9);
        // Cycle jitter: the alias at the mean rate, at the walk's expected height; it recedes.
        let walk = |n: f64| {
            let mut scene = comb_with(|s| {
                s.jitter =
                    Some(JitterSpec { model: JitterModel::Walk, kind: JitterKind::Positive, delta: 0.3 });
            });
            scene.duration = n * 2.15;
            predict(&scene).unwrap()
        };
        let (short, long) = (walk(500.0), walk(5000.0));
        assert_eq!(short.mean_cycle, 2.15);
        assert!((short.lines[0].frequency - 1.0 / 2.15).abs() < 1e-12);
        let ratio = short.lines[0].height / long.lines[0].height;
        assert!(ratio > 2.5 && ratio < 3.4, "ten times the shots: the alias fell by {ratio}, about √10");
        // Jitter about a grid: lowered, not receding, with a floor that does.
        let grid = predict(&comb_with(|s| {
            s.jitter =
                Some(JitterSpec { model: JitterModel::Grid, kind: JitterKind::Symmetric, delta: 0.25 });
        }))
        .unwrap();
        let psi = cadence::psi_uniform(0.5, 0.25).abs();
        assert_eq!(grid.lines[0].height, psi);
        assert!((grid.floor - (1.0 - psi * psi) / 1000.0).abs() < 1e-15);
        // Loss: a floor, and fewer shots; the notes say when features are combined.
        let lossy =
            predict(&comb_with(|s| (s.loss, s.gaps) = (0.05, Some(GapSpec { on: 3, period: 5 })))).unwrap();
        assert_eq!(lossy.floor, cadence::loss_floor(0.05, 1000));
        assert!(lossy.kept.iter().all(|kept| (520..=600).contains(kept)), "{:?}", lossy.kept);
        assert!(lossy.notes.iter().any(|note| note.contains("as if alone")));
        // Control: the realisation off a line is nowhere near a line's height, so agreement on
        // the lines above is agreement about where they are.
        assert!(realised(&comb_with(|s| s.gaps = Some(GapSpec { on: 3, period: 5 })), 0.137) < 0.02);
    }

    #[test]
    fn the_window_is_one_realisation() {
        let scene = comb_with(|s| s.alternate = Some(2.5));
        let (frequencies, window) = window(&scene, 0, 1.0, 901).unwrap();
        assert_eq!((frequencies.len(), frequencies[0], frequencies[900]), (901, 0.0, 1.0));
        assert!((window[0] - 1.0).abs() < 1e-12, "the peak is 1 at zero frequency");
        // The sidebands stand where the prediction says: 2/4.5 Hz is the 400th of 900 steps.
        let second = cadence::alternating_sideband(2, 2.0, 2.5);
        assert!((window[400] - second).abs() < 2e-3, "{} against {second}", window[400]);
        // A long run goes by the non-uniform FFT and agrees with the direct sum.
        let mut long = scene.clone();
        long.duration = 22_500.0;
        let (_, fast) = super::window(&long, 0, 1.0, 901).unwrap();
        assert!((fast[400] - second).abs() < 1e-3);
        // Refusals, and the bound on frequencies.
        assert!(super::window(&scene, 7, 1.0, 100).unwrap_err().contains("no tower 8"));
        assert!(super::window(&scene, 0, 0.0, 100).is_err());
        assert_eq!(super::window(&scene, 0, 1.0, 1).unwrap().0.len(), 2);
    }
}
