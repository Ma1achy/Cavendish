//! The anchors: the point-mass scenes the forward model is held to.
//!
//! Two are the published ones of `reference/design/instrument.md` §12 (spec `tab:anchors`), with
//! the parameters v1's own anchor tests use (`crates/generate/tests/motion_anchors.rs` at
//! `v1-final`); the static one is the scene of v1's `mass_linearity` test. The oracle tier runs
//! them through v1 itself; `quick` holds the engine to v1's frozen outputs for the same scenes
//! (`oracle/fixtures/v1/`).

use std::f64::consts::TAU;

use crate::Vec3;

/// One anchor: a point mass on a path, read at a list of times.
#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    /// The name of its fixture and of its validation case.
    pub name: &'static str,
    /// Kilograms.
    pub mass: f64,
    /// Where the mass is at a time: seconds on the scene's clock in, metres in the world's frame
    /// (x east, y north, z up) out. The anchors are read by a tower at the origin (and, for the
    /// five-tower fixture, by the default array), so for the first the world's frame is also the
    /// tower's.
    pub path: fn(f64) -> Vec3,
    /// `(first, step, count)`: the first readout time and the step between readouts, both in
    /// seconds on the scene's clock, and how many readouts there are. A readout time is when
    /// the shot is read out (2T after its launch), not when it is launched.
    pub readouts: (f64, f64, usize),
}

impl Anchor {
    /// The readout times, seconds on the scene's clock, in rising order.
    pub fn times(&self) -> impl Iterator<Item = f64> + use<> {
        let (first, step, count) = self.readouts;
        (0..count).map(move |n| first + n as f64 * step)
    }
}

fn still(_: f64) -> Vec3 {
    Vec3::new(2.0, 0.0, 2.5)
}

fn pass(t: f64) -> Vec3 {
    Vec3::new(10.0, 0.0, -10.0 + 2.5 * t)
}

fn oscillation(t: f64) -> Vec3 {
    Vec3::new(5.0, 0.0, 1.0 + (TAU * 0.1 * t).cos())
}

/// 100 kg at rest at (2, 0, 2.5) m: 2 m east of the tower, half-way up the lower
/// interferometer's reach. Read out at 2, 3 and 4 s.
pub const STATIC: Anchor = Anchor { name: "static", mass: 100.0, path: still, readouts: (2.0, 1.0, 3) };

/// 10 kg at D = 10 m passing vertically at 2.5 m/s, from 10 m below the base. The reference
/// design says "~7 mrad"; v1 and the engine give a peak of 13.7 mrad, and what the figure measures
/// is a question for George (`plan/PROGRESS.md`).
pub const PASS: Anchor = Anchor { name: "pass", mass: 10.0, path: pass, readouts: (0.0, 0.5, 21) };

/// 1 kg at D = 5 m, at height 1 + cos(2π·0.1·t) m: about 2 mrad in amplitude, 4 mrad peak to
/// peak (George's `1D_oscillator`: 4.06).
pub const OSCILLATION: Anchor =
    Anchor { name: "oscillation", mass: 1.0, path: oscillation, readouts: (0.0, 0.25, 41) };

/// The three anchors read by a single tower at the origin: `[STATIC, PASS, OSCILLATION]`.
pub const ALL: [Anchor; 3] = [STATIC, PASS, OSCILLATION];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_anchors_are_the_published_scenes() {
        assert_eq!(PASS.times().count(), 21);
        assert_eq!(PASS.times().last(), Some(10.0));
        assert_eq!((PASS.path)(4.0), Vec3::new(10.0, 0.0, 0.0), "level with the base at 4 s");
        assert_eq!((OSCILLATION.path)(0.0), Vec3::new(5.0, 0.0, 2.0));
        assert!(((OSCILLATION.path)(5.0).z - 0.0).abs() < 1e-12, "half a period later it is at the bottom");
        assert_eq!(OSCILLATION.times().nth(40), Some(10.0), "one period of 10 s");
        assert_eq!((STATIC.path)(7.0), (STATIC.path)(0.0));
        // Control: the names are distinct, since fixtures and cases are filed under them.
        assert!(ALL.iter().enumerate().all(|(n, a)| ALL[..n].iter().all(|b| b.name != a.name)));
    }

    use crate::forward::{Instrument, Mass, Reader};

    /// A series v1 computed, frozen under `oracle/fixtures/v1/`. The oracle tier checks, against
    /// v1 compiled from source, that these files are still what v1 computes.
    fn frozen(name: &str) -> Vec<f64> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../oracle/fixtures/v1")
            .join(format!("{name}.npy"));
        crate::npy::read(&path).unwrap_or_else(|why| panic!("{why}"))
    }

    /// The engine's series for an anchor at a tower, and its largest distance from `reference` as
    /// a fraction of the reference's peak.
    fn against(instrument: Instrument, anchor: &Anchor, tower: Vec3, reference: &[f64]) -> f64 {
        let reader = Reader::new(instrument);
        let source = Mass::point(anchor.mass, anchor.path);
        let ours: Vec<f64> = anchor.times().map(|t| reader.phase(tower, &source, t)).collect();
        assert_eq!(ours.len(), reference.len(), "{}: the fixture has another length", anchor.name);
        let peak = reference.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        ours.iter().zip(reference).fold(0.0f64, |m, (a, b)| m.max((a - b).abs())) / peak
    }

    cav_validate::case! {
        test: validation_v1_anchor_fixture,
        name: "forward.v1_anchor_fixture",
        subsystem: "forward",
        slice: "S1",
        reference: V1Oracle,
        against: "v1's outputs on the three anchors and on the oscillation read by the five towers, frozen in oracle/fixtures/v1 (the oracle tier checks them against v1 itself); engine set to v1's conventions; largest difference as a fraction of each series' peak",
        unit: Relative,
        tolerance: 1e-10,
        measure: {
            let v1 = Instrument::aion10().with_v1_conventions();
            let mut worst = 0.0f64;
            for anchor in &ALL {
                worst = worst.max(against(v1, anchor, Vec3::ZERO, &frozen(anchor.name)));
            }
            let five = frozen("five_towers");
            let shots = OSCILLATION.readouts.2;
            assert_eq!(five.len(), 5 * shots);
            for (n, position) in crate::defaults::TOWER_POSITIONS.iter().enumerate() {
                let tower = Vec3::new(position[0], position[1], 0.0);
                worst = worst.max(against(v1, &OSCILLATION, tower, &five[n * shots..(n + 1) * shots]));
            }
            // Control: the engine's own conventions are not v1's, and these files can tell.
            assert!(against(Instrument::aion10(), &PASS, Vec3::ZERO, &frozen("pass")) > 1.0);
            cav_validate::Measured::Error(worst)
        },
    }
}
