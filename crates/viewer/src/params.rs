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
    let half = 0.2;
    let each = p.mass / 8.0;
    let cloud = Cloud::from_elements(&[
        (half, half, half, each),
        (-half, half, half, each),
        (half, -half, half, each),
        (-half, -half, half, each),
        (half, half, -half, each),
        (-half, half, -half, each),
        (half, -half, -half, each),
        (-half, -half, -half, each),
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
