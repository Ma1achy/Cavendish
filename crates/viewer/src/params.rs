//! The editable scenario parameters the App holds. A `Scenario` cannot be stored — its `source` is a
//! `Box<dyn SourceDynamics>`, not `Clone` — so the App keeps these plain values and `build_scenario`
//! realises a fresh `Scenario` each run. Tweak-and-rerun changes one field here and runs again.

use generate::{
    Detector, DetectorArray, FieldSet, Isometry3, Orient, Path, Quat, Scenario, Schedule, Source,
    Timing, Trajectory, Vec3,
};
use gravity::Cloud;

/// A small, legible scenario: one cuboid-cloud source at a distance, a line of detectors, a uniform
/// schedule. Enough to inspect the forward model and to tweak-and-rerun.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScenarioParams {
    pub mass: f64,
    pub distance: f64,
    pub omega0: [f64; 3],
    pub n_times: usize,
    pub rate: f64,
    pub detectors: usize,
    pub seed: u64,
}

impl Default for ScenarioParams {
    fn default() -> Self {
        ScenarioParams {
            mass: 500.0,
            distance: 3.0,
            omega0: [0.0, 0.0, 0.0],
            n_times: 64,
            rate: 2.0,
            detectors: 2,
            seed: 7,
        }
    }
}

/// Realise a fresh `Scenario` from the parameters. Signal amplitude scales with `mass`, so
/// tweak-and-rerun on `mass` is visible (and testable) in the bundle.
pub fn build_scenario(p: &ScenarioParams) -> Scenario {
    // An ASYMMETRIC box (distinct half-extents → distinct principal moments Iₓ < I_y < I_z), so a spin
    // about the intermediate axis (y) is unstable — the Dzhanibekov flip the ω₀·y control shows.
    let (hx, hy, hz) = (0.35, 0.2, 0.12);
    let each = p.mass / 8.0;
    let cloud = Cloud::from_elements(&[
        (hx, hy, hz, each),
        (-hx, hy, hz, each),
        (hx, -hy, hz, each),
        (-hx, -hy, hz, each),
        (hx, hy, -hz, each),
        (-hx, hy, -hz, each),
        (hx, -hy, -hz, each),
        (-hx, -hy, -hz, each),
    ]);
    let orient = if p.omega0 == [0.0, 0.0, 0.0] {
        Orient::Fixed(Quat::identity())
    } else {
        Orient::FreeRotation {
            omega0: Vec3::new(p.omega0[0], p.omega0[1], p.omega0[2]),
        }
    };
    let traj = Trajectory::new(
        Isometry3::new(Quat::identity(), Vec3::new(p.distance, 0.0, p.distance)),
        Path::Static,
        Timing::Uniform { rate: 0.0 },
    )
    .with_orient(orient);
    let dets = (0..p.detectors.max(1))
        .map(|i| Detector::new(i as f64))
        .collect();
    Scenario::new(
        Box::new(Source::new(cloud, traj)),
        DetectorArray::new(dets),
        Schedule::uniform(p.rate, p.n_times.max(1)),
        p.seed,
    )
    .with_field_set(FieldSet {
        periodogram: true,
        ..FieldSet::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use state::StateBundle;

    /// The clean (noise-subtracted) peak amplitude of detector 0.
    fn clean_amplitude(b: &StateBundle) -> f64 {
        b.signal
            .iter()
            .zip(&b.signal_noise)
            .map(|(s, n)| (s[0] - n[0]).abs())
            .fold(0.0_f64, f64::max)
    }

    #[test]
    fn tweak_rerun() {
        // Tweak-and-rerun (the headless half): doubling the source mass doubles the signal amplitude —
        // the potential is linear in mass, so the clean channel scales exactly. "The view updates" is
        // the review half, judged live (see tumble_visible's recipe; Run after moving the mass slider).
        let base = ScenarioParams::default();
        let heavier = ScenarioParams {
            mass: 2.0 * base.mass,
            ..base
        };
        let a1 = clean_amplitude(&generate::run(&build_scenario(&base)));
        let a2 = clean_amplitude(&generate::run(&build_scenario(&heavier)));
        assert!(a1 > 0.0, "no signal to scale");
        let ratio = a2 / a1;
        assert!(
            (ratio - 2.0).abs() < 1e-9,
            "amplitude ratio {ratio}, expected 2 (signal ∝ mass)"
        );
    }

    #[test]
    #[ignore = "review-grade: run `cargo run -p viewer`, raise ω₀·y, press Run, and scrub to watch the flip"]
    fn tumble_visible() {
        // An asymmetric top spun about its intermediate axis (y) undergoes the Dzhanibekov flip (the
        // source crate's `dzhanibekov` test validates the physics). Here we only assert the tumble is
        // PRESENT in the data — ω throughout, and the orientation genuinely evolves across ℓ — so the
        // review has real motion to watch. The visible flip itself is the human's call.
        // Cheap params — enough steps for the orientation to evolve; the visible flip is the reviewer's
        // call in the app (which can scrub a far longer window), so this need not integrate for seconds.
        let p = ScenarioParams {
            omega0: [0.0, 3.0, 0.02],
            n_times: 60,
            rate: 0.05,
            ..Default::default()
        };
        let b = generate::run(&build_scenario(&p));
        let angvel = &b.source_angular_velocity[0];
        assert!(
            angvel
                .iter()
                .all(|w| w[0] * w[0] + w[1] * w[1] + w[2] * w[2] > 0.0),
            "ω vanished — nothing spinning"
        );
        let first = b.source_orientation[0][0];
        let evolved = b.source_orientation[0]
            .iter()
            .any(|q| (0..4).map(|k| (q[k] - first[k]).abs()).sum::<f64>() > 0.1);
        assert!(evolved, "orientation did not evolve — no tumble to see");
    }
}
