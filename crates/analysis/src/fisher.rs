//! The Fisher information `F = JᵀΣ⁻¹J`, the Cramér–Rao floor `CRB = F⁻¹`, and **honest** conditioning.
//!
//! v1 noise is white (`Σ = σ²𝟙`), so `F = JᵀJ/σ²`. Degeneracy is reported, not hidden: when the
//! Fisher is near-singular (`cond(F)` above a threshold) the CRB is **withheld** — a covariance
//! inverted from a rank-deficient Fisher is meaningless — and the near-null direction is surfaced so
//! the degenerate parameter combination (e.g. the far-monopole mass–distance pair) is legible.

use crate::Jacobian;
use compute::ParamSeed;
use nalgebra::DMatrix;

/// The white-noise Fisher information `F = JᵀJ / σ²` (`Σ = σ²𝟙`). Symmetric, positive-semidefinite.
pub fn fisher(j: &Jacobian, sigma: f64) -> DMatrix<f64> {
    (j.matrix.transpose() * &j.matrix) / (sigma * sigma)
}

/// The Cramér–Rao report for a Fisher matrix: its spectrum, **degeneracy conditioning**, and — only
/// when the Fisher is well-conditioned — the covariance floor `CRB = F⁻¹`.
pub struct CrbReport {
    pub params: Vec<ParamSeed>,
    pub fisher: DMatrix<f64>,
    /// Eigenvalues of the raw Fisher, ascending (a positive-semidefiniteness witness).
    pub eigenvalues: Vec<f64>,
    /// Conditioning of the **correlation-normalised** Fisher, `cond(D⁻¹FD⁻¹)`, `D = diag(√Fᵢᵢ)` — a
    /// scale-free measure of parameter degeneracy (unit choices do not inflate it). `∞` if a parameter
    /// is wholly unconstrained (`Fᵢᵢ = 0`) or the correlation matrix is singular.
    pub condition: f64,
    /// `true` when `condition` exceeds the supplied threshold — the CRB is then withheld.
    pub degenerate: bool,
    /// The covariance floor `F⁻¹`; `None` when degenerate (not a meaningless pseudo-inverse).
    pub crb: Option<DMatrix<f64>>,
    /// The correlation matrix's eigenvector of its smallest eigenvalue — the (near-)null direction.
    /// Its large-magnitude entries name the degenerate parameter combination (e.g. the m–R pair).
    pub null_direction: Vec<f64>,
}

impl CrbReport {
    /// The variance floor of parameter `i` (`diag(CRB)ᵢ`); `None` when the Fisher is degenerate.
    pub fn variance(&self, i: usize) -> Option<f64> {
        self.crb.as_ref().map(|c| c[(i, i)])
    }
}

/// Form the [`CrbReport`]. `condition_threshold` is the correlation-conditioning above which the Fisher
/// is treated as degenerate: the CRB is withheld (a covariance from a rank-deficient Fisher is
/// meaningless) and the near-null direction surfaced instead.
pub fn crb_report(
    params: Vec<ParamSeed>,
    fisher: DMatrix<f64>,
    condition_threshold: f64,
) -> CrbReport {
    let n = fisher.nrows();
    let eigenvalues = ascending_eigenvalues(&fisher);

    // Correlation-normalise so conditioning measures genuine parameter degeneracy, not unit scale: a
    // fractional-mass column (derivative ~ signal) and a position column (derivative ~ signal/R) would
    // otherwise show a huge raw cond from the scale mismatch alone.
    let scale: Vec<f64> = (0..n).map(|i| fisher[(i, i)].max(0.0).sqrt()).collect();
    let unconstrained = scale.iter().any(|&s| s <= 0.0);
    let (condition, null_direction) = if unconstrained {
        (f64::INFINITY, vec![0.0; n])
    } else {
        let mut corr = DMatrix::zeros(n, n);
        for i in 0..n {
            for j in 0..n {
                corr[(i, j)] = fisher[(i, j)] / (scale[i] * scale[j]);
            }
        }
        let eig = corr.symmetric_eigen();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| eig.eigenvalues[a].partial_cmp(&eig.eigenvalues[b]).unwrap());
        let (lmin, lmax) = (eig.eigenvalues[order[0]], eig.eigenvalues[order[n - 1]]);
        let cond = if lmin > 0.0 {
            lmax / lmin
        } else {
            f64::INFINITY
        };
        let null = eig.eigenvectors.column(order[0]).iter().copied().collect();
        (cond, null)
    };

    // `condition` is finite-positive or +∞ (never NaN), so `>` also flags a fully-singular Fisher.
    let degenerate = condition > condition_threshold;
    let crb = if degenerate {
        None // a covariance from a rank-deficient Fisher is meaningless — do not fabricate one
    } else {
        fisher.clone().cholesky().map(|c| c.inverse())
    };
    CrbReport {
        params,
        fisher,
        eigenvalues,
        condition,
        degenerate,
        crb,
        null_direction,
    }
}

/// The eigenvalues of a symmetric matrix, ascending.
fn ascending_eigenvalues(m: &DMatrix<f64>) -> Vec<f64> {
    let mut v: Vec<f64> = m
        .clone()
        .symmetric_eigen()
        .eigenvalues
        .iter()
        .copied()
        .collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{detector_at, scenario, seeds, with_detectors};
    use compute::{forward_f64, Axis, Param};
    use instrument::InstrumentConfig;
    use math::Vec3;

    #[test]
    fn fisher_spd() {
        // F = JᵀJ/σ² is symmetric (≤1e-10), positive-semidefinite (eigenvalues ≥ 0), and Cholesky
        // succeeds for a well-conditioned scenario (nearby source, three position parameters).
        let cfg = InstrumentConfig::default();
        let scn = scenario(&[(1.5, 0.3, 2.0, 400.0)], Vec3::new(0.0, 0.0, 0.0), 3, 4);
        let params = seeds(&[
            Param::Position(Axis::X),
            Param::Position(Axis::Y),
            Param::Position(Axis::Z),
        ]);
        let f = fisher(&Jacobian::assemble(&scn, &cfg, &params), 1e-3);

        let asym = (f.clone() - f.transpose()).abs().max();
        assert!(asym <= 1e-10, "Fisher not symmetric: {asym:e}");
        let report = crb_report(params, f, 1e12);
        assert!(
            report.eigenvalues[0] >= -1e-10 * report.eigenvalues.last().unwrap().abs(),
            "negative eigenvalue {:e}",
            report.eigenvalues[0]
        );
        assert!(
            !report.degenerate,
            "well-conditioned scenario flagged degenerate"
        );
        assert!(
            report.crb.is_some(),
            "Cholesky inverse failed on an SPD Fisher"
        );
    }

    #[test]
    fn analytic_amplitude() {
        // Mass is exactly the amplitude case (signal ∝ m), so the single-parameter CRB matches the
        // closed form CRB = σ²/Σ(∂s/∂θ)². With the fractional-mass seed, ∂s/∂θ = signal, so the Fisher
        // is Σs²/σ² and CRB = σ²/Σs² — computed here independently from the raw f64 signal.
        let cfg = InstrumentConfig::default();
        let sigma = 2e-3;
        let scn = scenario(&[(3.0, 0.0, 2.5, 500.0)], Vec3::new(0.0, 0.0, 0.0), 2, 4);
        let params = seeds(&[Param::Mass]);
        let f = fisher(&Jacobian::assemble(&scn, &cfg, &params), sigma);
        let report = crb_report(params, f, 1e12);
        let crb_mm = report.variance(0).unwrap();

        let sum_s2: f64 = forward_f64(&scn, &cfg)
            .iter()
            .flatten()
            .map(|s| s * s)
            .sum();
        let analytic = sigma * sigma / sum_s2;
        let rel = (crb_mm - analytic).abs() / analytic;
        assert!(
            rel <= 1e-8,
            "CRB_mm {crb_mm:e} vs analytic {analytic:e} (rel {rel:e})"
        );
    }

    #[test]
    fn degeneracy_reported() {
        // A far monopole: ∂s/∂m ∝ s/m and ∂s/∂R ∝ −k·s/R make the (m, R) Jacobian columns nearly
        // parallel, so cond(F) is huge — and that MUST surface (degenerate flag + the near-null
        // direction weighting both parameters), never be silently inverted into a confident CRB.
        // A wide-baseline array (detectors spanning a large fraction of R) resolves the range and
        // breaks the degeneracy — the same parallax the Gradar array-design payoff rests on.
        let cfg = InstrumentConfig::default();
        let sigma = 1e-3;
        let source = [(60.0, 0.0, 0.0, 500.0)];
        let origin = Vec3::new(0.0, 0.0, 0.0);
        let params = seeds(&[Param::Mass, Param::Position(Axis::X)]);

        // Far source (R ≈ 60) seen by a tight near-origin pair — all at essentially the same range.
        let far_f = fisher(
            &Jacobian::assemble(
                &with_detectors(
                    &source,
                    origin,
                    None,
                    vec![detector_at(0.0, 0.0), detector_at(0.5, 0.0)],
                    4,
                ),
                &cfg,
                &params,
            ),
            sigma,
        );
        // A wide array reaching towards the source (R ≈ 60, 40, 20): comparable-weight measurements at
        // genuinely different ranges, so the m:R sensitivity ratio varies and the degeneracy lifts.
        let broken_f = fisher(
            &Jacobian::assemble(
                &with_detectors(
                    &source,
                    origin,
                    None,
                    vec![
                        detector_at(0.0, 0.0),
                        detector_at(20.0, 0.0),
                        detector_at(40.0, 0.0),
                    ],
                    4,
                ),
                &cfg,
                &params,
            ),
            sigma,
        );

        // Conditioning is threshold-independent; measure both, then set the threshold between them.
        let far_cond = crb_report(params.clone(), far_f.clone(), f64::INFINITY).condition;
        let broken_cond = crb_report(params.clone(), broken_f.clone(), f64::INFINITY).condition;
        eprintln!("degeneracy: far cond(F) = {far_cond:e}, broken = {broken_cond:e}");
        assert!(
            far_cond > 100.0 * broken_cond,
            "near pass did not break the degeneracy: far {far_cond:e} vs broken {broken_cond:e}"
        );
        let threshold = (far_cond * broken_cond).sqrt();

        let far = crb_report(params.clone(), far_f, threshold);
        assert!(far.degenerate, "far monopole not flagged degenerate");
        assert!(
            far.crb.is_none(),
            "a degenerate Fisher must not yield a CRB"
        );
        // The near-null direction mixes BOTH parameters (the m–R degeneracy), not one alone.
        let (nm, nr) = (far.null_direction[0].abs(), far.null_direction[1].abs());
        assert!(
            nm > 0.1 && nr > 0.1,
            "null direction does not flag the (m,R) pair: {nm:.3},{nr:.3}"
        );

        let broken = crb_report(params, broken_f, threshold);
        assert!(!broken.degenerate, "near pass left it degenerate");
        assert!(broken.crb.is_some());
    }
}
