//! The Jacobian `J = ∂signal/∂θ`, assembled by forward-mode `Dual` sweeps through the whole forward
//! model (`compute::forward_dual`). One sweep per parameter; column `j` is that sweep's tangent
//! channel, flattened over the `(detector, measurement)` grid. The value channel is a free
//! cross-check — it must equal the plain `f64` forward, or the `Dual` lift is wrong before the
//! derivatives even matter.

use compute::{forward_dual, forward_f64, ParamSeed, ScenarioBatch};
use instrument::InstrumentConfig;
use nalgebra::DMatrix;

/// The Jacobian `J ∈ ℝ^{(T·D)×P}`: rows are the flattened `(detector, measurement)` signal entries,
/// columns the parameters θ (one [`ParamSeed`] each).
pub struct Jacobian {
    pub matrix: DMatrix<f64>,
    pub params: Vec<ParamSeed>,
}

impl Jacobian {
    /// Assemble `J` for `params`: one `forward_dual` sweep per parameter, its tangent channel written
    /// as column `j` (row-major over detectors then measurements). Panics if a `Dual` sweep's value
    /// channel drifts from the plain `f64` forward — the free value-channel-identity guard.
    pub fn assemble(scn: &ScenarioBatch, cfg: &InstrumentConfig, params: &[ParamSeed]) -> Self {
        let base = forward_f64(scn, cfg);
        let rows: usize = base.iter().map(|d| d.len()).sum();
        let mut matrix = DMatrix::zeros(rows, params.len());
        for (j, &seed) in params.iter().enumerate() {
            let sweep = forward_dual(scn, cfg, seed);
            let mut r = 0;
            for (di, det_row) in sweep.iter().enumerate() {
                for (ti, dual) in det_row.iter().enumerate() {
                    assert_eq!(
                        dual.v, base[di][ti],
                        "value-channel drift at seed {seed:?}, ({di},{ti}) — the Dual lift is wrong"
                    );
                    matrix[(r, j)] = dual.d;
                    r += 1;
                }
            }
        }
        Jacobian {
            matrix,
            params: params.to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{perturb, rotating_scenario, scenario};
    use compute::{Axis, Param};
    use math::Vec3;

    /// Flatten `forward_f64` row-major (detectors, then measurements), the Jacobian's row order.
    fn flat(sig: &[Vec<f64>]) -> Vec<f64> {
        sig.iter().flatten().copied().collect()
    }

    #[test]
    fn value_channel_identity() {
        // Every Dual sweep's value channel equals the plain f64 forward (assemble asserts this; here we
        // exercise a mix of parameter kinds so the guard has teeth).
        let cfg = InstrumentConfig::default();
        let scn = scenario(
            &[(3.0, 0.0, 2.5, 500.0), (2.7, 0.1, 2.6, 300.0)],
            Vec3::new(0.0, 0.0, 0.0),
            2,
            3,
        );
        let params = [
            ParamSeed {
                source: 0,
                param: Param::Position(Axis::X),
            },
            ParamSeed {
                source: 0,
                param: Param::Position(Axis::Z),
            },
            ParamSeed {
                source: 0,
                param: Param::Mass,
            },
        ];
        // assemble panics on any value-channel drift; reaching here is the pass.
        let j = Jacobian::assemble(&scn, &cfg, &params);
        assert_eq!(j.matrix.ncols(), 3);
        assert_eq!(j.matrix.nrows(), 2 * 3);
    }

    #[test]
    fn dual_vs_finite_diff() {
        // Every column of J equals central finite differences of the f64 forward to ≤1e-6 relative —
        // the spine: the autodiff derivative is the TRUE derivative of the physics. Covers position,
        // mass, and (free-rotation) ω₀ parameters.
        let cfg = InstrumentConfig::default();
        let scn = rotating_scenario(
            &[(3.0, 0.0, 2.5, 500.0), (2.7, 0.1, 2.6, 300.0)],
            Vec3::new(0.5, -0.2, 0.0),
            Some(Vec3::new(0.5, 0.2, 0.3)),
            2,
            3,
        );
        let params = [
            ParamSeed {
                source: 0,
                param: Param::Position(Axis::X),
            },
            ParamSeed {
                source: 0,
                param: Param::Position(Axis::Z),
            },
            ParamSeed {
                source: 0,
                param: Param::Mass,
            },
            ParamSeed {
                source: 0,
                param: Param::Omega0(Axis::X),
            },
        ];
        let j = Jacobian::assemble(&scn, &cfg, &params);
        let h = 1e-6;
        for (col, &seed) in params.iter().enumerate() {
            let plus = flat(&forward_f64(&perturb(&scn, seed, h), &cfg));
            let minus = flat(&forward_f64(&perturb(&scn, seed, -h), &cfg));
            let fd: Vec<f64> = plus
                .iter()
                .zip(&minus)
                .map(|(p, m)| (p - m) / (2.0 * h))
                .collect();
            let jcol: Vec<f64> = (0..j.matrix.nrows()).map(|r| j.matrix[(r, col)]).collect();
            let diff: f64 = jcol
                .iter()
                .zip(&fd)
                .map(|(a, b)| (a - b) * (a - b))
                .sum::<f64>()
                .sqrt();
            let scale: f64 = fd.iter().map(|x| x * x).sum::<f64>().sqrt();
            assert!(
                diff <= 1e-6 * scale.max(1e-30),
                "column {col} ({seed:?}): ‖J − FD‖ = {diff:e}, ‖FD‖ = {scale:e}"
            );
        }
    }
}
