//! `generate` — orchestration: the `run` spine that drives a `Scenario` to a `StateBundle`.
//!
//! Design: `design/generate.md`. Milestone: `milestones/M1-physics-spine.md`.
//!
//! M1 is the walking skeleton: build the arms once, evaluate `PropagationIntegral` at each scheduled
//! time, assemble the minimal bundle. Batch dispatch and the `ComputeBackend` path arrive at M6. The
//! seam names are re-exported so every lower crate's public surface stays reachable here (M0-R1).

pub use compute::{ComputeBackend, EvalBatch, SignalBatch};
pub use gravity::FieldContribution;
pub use scenario::{
    Detector, KeyRng, NoiseSource, PhaseModel, Prescribed, Scenario, Schedule, SourceDynamics,
};
pub use state::{Dual, Isometry3, Mat3, Quat, Scalar, StateBundle, Vec3};

use gravity::Cloud;
use instrument::PropagationIntegral;
use state::Meta;

/// Drive a scenario through the propagation-integral spine to a `StateBundle`.
pub fn run(scenario: &Scenario) -> StateBundle {
    let model = PropagationIntegral::default();
    let sources: [&dyn SourceDynamics; 1] = [scenario.source.as_ref()];

    let mut time = Vec::new();
    let mut signal = Vec::new();
    let mut track = Vec::new();
    let mut mask = Vec::new();
    for &t in &scenario.schedule.times {
        let dphi = model.delta_phi(&sources, &scenario.detector, t);
        let pos = scenario.source.pose_at(t).translation;
        time.push(t);
        signal.push(vec![dphi]);
        track.push([pos.x, pos.y, pos.z]);
        mask.push(false);
    }

    StateBundle {
        time,
        signal,
        source_position: vec![track],
        mask,
        meta: Meta {
            seed: scenario.seed,
            description: "M1 propagation-integral spine".into(),
        },
    }
}

/// Ad-hoc concrete-wall lattice — **throwaway**, replaced by `shape`'s voxeliser in M2.
///
/// A regular lattice of point elements filling the `size` cuboid centred at `centre`, each carrying
/// `m = density·pitch³` (no renormalisation). Do not invest in it; it exists only to feed the M1
/// anchor before the real geometry pipeline lands.
pub fn wall_cloud(size: Vec3<f64>, centre: Vec3<f64>, density: f64, pitch: f64) -> Cloud {
    let m = density * pitch * pitch * pitch;
    let counts = [
        (size.x / pitch).round().max(1.0) as usize,
        (size.y / pitch).round().max(1.0) as usize,
        (size.z / pitch).round().max(1.0) as usize,
    ];
    let origin = [
        centre.x - size.x * 0.5,
        centre.y - size.y * 0.5,
        centre.z - size.z * 0.5,
    ];
    let mut elems = Vec::with_capacity(counts[0] * counts[1] * counts[2]);
    for ix in 0..counts[0] {
        let x = origin[0] + (ix as f64 + 0.5) * pitch;
        for iy in 0..counts[1] {
            let y = origin[1] + (iy as f64 + 0.5) * pitch;
            for iz in 0..counts[2] {
                let z = origin[2] + (iz as f64 + 0.5) * pitch;
                elems.push((x, y, z, m));
            }
        }
    }
    Cloud::from_elements(&elems)
}

#[cfg(test)]
mod edges_reachable {
    //! Naming each lower crate's public surface proves the dependency edges carry it up to L4.
    #[allow(unused_imports)]
    use super::{
        ComputeBackend, Detector, Dual, EvalBatch, FieldContribution, Isometry3, KeyRng, Mat3,
        NoiseSource, PhaseModel, Prescribed, Quat, Scalar, Scenario, Schedule, SignalBatch,
        SourceDynamics, StateBundle, Vec3,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use gravity::Cloud;
    use instrument::{InstrumentConfig, PropagationIntegral};

    fn point_source(x: f64, z: f64) -> Prescribed {
        Prescribed::fixed(
            Cloud::from_elements(&[(x, 0.0, z, 500.0)]),
            Isometry3::identity(),
        )
    }

    #[test]
    fn spine_runs() {
        // Finite, non-zero ΔΦ, and the sign flips as the source crosses the detector plane — the
        // gradiometer centre at base_z + Δr/2 = 2.5 m (the two IFOs sit at 0 and 5).
        let det = Detector::new(0.0);
        let model = PropagationIntegral::default();
        let above = point_source(1.0, 4.0); // above the centre, between the IFOs
        let below = point_source(1.0, 1.0); // below the centre, between the IFOs
        let dphi_above = model.delta_phi(&[&above], &det, 2.0);
        let dphi_below = model.delta_phi(&[&below], &det, 2.0);
        assert!(dphi_above.is_finite() && dphi_above != 0.0);
        assert!(dphi_below.is_finite() && dphi_below != 0.0);
        assert!(
            dphi_above * dphi_below < 0.0,
            "sign did not flip across the plane"
        );
    }

    #[test]
    fn bundle_shapes() {
        let scenario = Scenario::new(
            Box::new(point_source(1.0, 2.5)),
            Detector::new(0.0),
            Schedule::uniform(2.0, 5),
            42,
        );
        let bundle = run(&scenario);
        assert_eq!(bundle.time.len(), 5);
        assert_eq!(bundle.signal.len(), 5);
        assert_eq!(bundle.signal[0].len(), 1);
        assert_eq!(bundle.source_position.len(), 1);
        assert_eq!(bundle.source_position[0].len(), 5);
        assert_eq!(bundle.mask.len(), 5);
        assert!(bundle.time.iter().all(|t| t.is_finite()));
    }

    #[test]
    fn quadrature_converges() {
        // Halving fine_dt changes ΔΦ negligibly — the integrator is far finer than the signal.
        let det = Detector::new(0.0);
        let src = point_source(1.5, 2.5);
        let phi = |dt: f64| {
            let cfg = InstrumentConfig {
                fine_dt: dt,
                ..InstrumentConfig::default()
            };
            PropagationIntegral::new(cfg).delta_phi(&[&src], &det, 2.0)
        };
        let coarse = phi(0.01);
        let fine = phi(0.005);
        assert!(
            (coarse - fine).abs() / fine.abs() <= 1e-6,
            "quadrature not converged"
        );
    }
}
