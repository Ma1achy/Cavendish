//! Shared test scenario builders (poses, clouds, detector lines) — the one place the tests name `math`.

use compute::{AtmoMode, Axis, Param, ParamSeed, ScenarioBatch, SourceBatch};
use gravity::Cloud;
use math::{Isometry3, Quat, Vec3};
use source::{Orient, Path, Timing};

/// A static source at `placement` with the given `(x,y,z,mass)` elements, a line of `d` detectors at
/// unit `x`-spacing from the origin, and `t` uniformly-spaced measurement times.
pub fn scenario(
    elements: &[(f64, f64, f64, f64)],
    placement: Vec3<f64>,
    d: usize,
    t: usize,
) -> ScenarioBatch {
    rotating_scenario(elements, placement, None, d, t)
}

/// As [`scenario`], optionally with a free-rotation `ω₀` (to exercise the ω₀ Jacobian).
pub fn rotating_scenario(
    elements: &[(f64, f64, f64, f64)],
    placement: Vec3<f64>,
    omega0: Option<Vec3<f64>>,
    d: usize,
    t: usize,
) -> ScenarioBatch {
    let detectors = (0..d)
        .map(|i| Isometry3::new(Quat::identity(), Vec3::new(i as f64, 0.0, 0.0)))
        .collect();
    with_detectors(elements, placement, omega0, detectors, t)
}

/// As [`rotating_scenario`] but with explicit detector placements (for the array-geometry sweeps).
pub fn with_detectors(
    elements: &[(f64, f64, f64, f64)],
    placement: Vec3<f64>,
    omega0: Option<Vec3<f64>>,
    detectors: Vec<Isometry3>,
    t: usize,
) -> ScenarioBatch {
    let orient = match omega0 {
        Some(w) => Orient::FreeRotation { omega0: w },
        None => Orient::Fixed(Quat::identity()),
    };
    ScenarioBatch {
        sources: vec![SourceBatch {
            cloud: Cloud::from_elements(elements),
            placement: Isometry3::new(Quat::identity(), placement),
            path: Path::Static,
            timing: Timing::Uniform { rate: 0.0 },
            orient,
        }],
        atmo: Vec::<AtmoMode>::new(),
        detectors,
        times: (0..t).map(|i| 1.0 + i as f64).collect(),
    }
}

/// A detector at world position `(x, 0, z)`, identity orientation.
pub fn detector_at(x: f64, z: f64) -> Isometry3 {
    Isometry3::new(Quat::identity(), Vec3::new(x, 0.0, z))
}

/// Perturb the scalar parameter `seed` picks out by `delta` (for a finite-difference reference).
/// `Mass` scales every element mass by `1 + delta` (the fractional-mass parameterisation).
pub fn perturb(scn: &ScenarioBatch, seed: ParamSeed, delta: f64) -> ScenarioBatch {
    let mut out = scn.clone();
    let s = &mut out.sources[seed.source];
    match seed.param {
        Param::Position(ax) => {
            let v = &mut s.placement.translation;
            match ax {
                Axis::X => v.x += delta,
                Axis::Y => v.y += delta,
                Axis::Z => v.z += delta,
            }
        }
        Param::Omega0(ax) => {
            if let Orient::FreeRotation { omega0 } = &mut s.orient {
                match ax {
                    Axis::X => omega0.x += delta,
                    Axis::Y => omega0.y += delta,
                    Axis::Z => omega0.z += delta,
                }
            }
        }
        Param::Velocity(ax) => {
            if let Path::LinearPass { b, .. } = &mut s.path {
                match ax {
                    Axis::X => b.x += delta,
                    Axis::Y => b.y += delta,
                    Axis::Z => b.z += delta,
                }
            }
        }
        Param::Mass => {
            for m in s.cloud.ms.iter_mut() {
                *m *= 1.0 + delta;
            }
        }
    }
    out
}
