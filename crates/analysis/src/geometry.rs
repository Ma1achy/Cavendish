//! M8 array-geometry scoring (`crb_vs_baseline`) and the CRB regression pin (`crb_regression`).
//!
//! The CRB tells you where to put detectors: for a source at range `R`, transverse localisation
//! improves as the array baseline widens (parallax) — the Gradar array-design payoff.

use crate::testkit::{detector_at, seeds, with_detectors};
use crate::{crb_report, fisher, Jacobian};
use compute::{Axis, Param};
use instrument::InstrumentConfig;
use math::Vec3;

#[test]
fn crb_vs_baseline() {
    // Source overhead at range R; a line array of two interferometers at ±b/2 along x. The transverse
    // (x) position variance strictly decreases as the baseline b widens — wider aperture, sharper
    // localisation. Mass is marginalised (a free amplitude), so this is genuine geometric resolution.
    let cfg = InstrumentConfig::default();
    let sigma = 1e-3;
    let r = 300.0;
    let params = seeds(&[Param::Position(Axis::X), Param::Mass]);

    let mut prev = f64::INFINITY;
    for &b in &[10.0, 20.0, 50.0, 100.0, 200.0] {
        let scn = with_detectors(
            &[(0.0, 0.0, r, 500.0)],
            Vec3::new(0.0, 0.0, 0.0),
            None,
            vec![detector_at(-b / 2.0, 0.0), detector_at(b / 2.0, 0.0)],
            4,
        );
        let report = crb_report(
            params.clone(),
            fisher(&Jacobian::assemble(&scn, &cfg, &params), sigma),
            1e12,
        );
        let var_x = report
            .variance(0)
            .expect("well-conditioned at every baseline");
        eprintln!("baseline {b:>5}: CRB_xx = {var_x:e}");
        assert!(
            var_x < prev,
            "transverse CRB did not improve at baseline {b}: {var_x:e} !< {prev:e}"
        );
        prev = var_x;
    }
}

#[test]
fn crb_regression() {
    // A pinned scenario's full CRB matrix vs a stored reference, so the machinery cannot silently
    // drift. The reference is this build's own output, held to ≤1e-6 relative.
    let cfg = InstrumentConfig::default();
    let sigma = 1e-3;
    let params = seeds(&[
        Param::Position(Axis::X),
        Param::Position(Axis::Z),
        Param::Mass,
    ]);
    let scn = with_detectors(
        &[(2.0, 0.0, 30.0, 500.0), (2.4, 0.1, 30.3, 300.0)],
        Vec3::new(0.0, 0.0, 0.0),
        None,
        vec![
            detector_at(0.0, 0.0),
            detector_at(3.0, 0.0),
            detector_at(0.0, 1.0),
        ],
        5,
    );
    let report = crb_report(
        params.clone(),
        fisher(&Jacobian::assemble(&scn, &cfg, &params), sigma),
        1e12,
    );
    let crb = report.crb.as_ref().expect("well-conditioned");

    // Stored reference (upper triangle, row-major): [xx, xz, xm, zz, zm, mm].
    let reference = [REF_XX, REF_XZ, REF_XM, REF_ZZ, REF_ZM, REF_MM];
    let got = [
        crb[(0, 0)],
        crb[(0, 1)],
        crb[(0, 2)],
        crb[(1, 1)],
        crb[(1, 2)],
        crb[(2, 2)],
    ];
    let names = ["xx", "xz", "xm", "zz", "zm", "mm"];
    for ((&g, &r), name) in got.iter().zip(&reference).zip(&names) {
        let rel = (g - r).abs() / r.abs().max(1e-300);
        eprintln!("CRB_{name}: {g:e} (ref {r:e}, rel {rel:e})");
        assert!(
            rel <= 1e-6,
            "CRB_{name} drifted: {g:e} vs {r:e} (rel {rel:e})"
        );
    }
}

// Reference CRB entries for `crb_regression` (pinned build output; static no-atmo scenario uses only
// sqrt + arithmetic, so it is deterministic across platforms — the ≤1e-6 tolerance is ample margin).
const REF_XX: f64 = 0.014198114844962481;
const REF_XZ: f64 = -0.030313898693387197;
const REF_XM: f64 = -0.003488538413765649;
const REF_ZZ: f64 = 0.6291360534546462;
const REF_ZM: f64 = 0.07564842405919919;
const REF_MM: f64 = 0.00910048955830278;
