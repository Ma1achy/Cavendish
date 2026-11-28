//! The local expansion held to the far field it stands for (`docs/slices/S5.md` §3, W6, X5).

use super::*;
use crate::read;
use crate::world::tests::{aim, boxed, scene, world_of};
use crate::world::walk::tests::{Case, case, terrain, world_box};
use crate::world::walk::{far_field, walk};
use cav_core::forward::Reading;

/// A world whose accepted nodes come near the towers' line: a 40 m square slab of concrete
/// 0.6 m thick under the array and two blocks beside it, at 5 cm (chunks of 1.6 m), so that the
/// walk accepts chunks from a few metres off and the far field varies along the span.
fn near() -> Case {
    let mut world = world_of(
        0.05,
        vec![
            boxed([-20.0, -20.0, -0.61], [20.0, 20.0, -0.01]),
            boxed([3.0, -2.0, -0.01], [5.0, 2.0, 6.0]),
            boxed([-9.0, 4.0, -0.01], [-6.0, 9.0, 11.0]),
        ],
    );
    world.origin = [0.013, -0.029, 0.007];
    let bases = vec![[0.0, 0.0, 0.0], [1.5, 0.5, 0.0], [-4.0, 6.0, 0.0], [10.0, -10.0, 0.0]];
    case("near", scene(world), bases)
}

/// One read of W6: the expansion's audit and its share, and the worst error of the flight read
/// through it against the same nodes read directly, over each interferometer and the gradiometer,
/// less W2's rounding allowance (1e-13 of the far part's phase).
struct Measured {
    audit: f64,
    share: f64,
    error: f64,
    found: LocalAudit,
}

/// W6 at every base of world-box, the terrain world and [`near`] where the cut accepts a node, at 1e-2,
/// 1e-4 and 1e-6 rad, the degree free (`None`) or held (`Some(n)`).
fn measured(forced: Option<usize>) -> Vec<Measured> {
    let mut out = Vec::new();
    for case in [world_box(), terrain(), near()] {
        let readers = read::Readers::new(&case.spec);
        let reader = readers.of(0);
        for &base in &case.bases {
            for tolerance in [1e-2, 1e-4, 1e-6] {
                let probe = aim(&case.spec, 0, base, (1.0 - LOCAL_SHARE) * tolerance, Rule::Octupole);
                let cut = walk(&case.tree, &probe);
                if cut.accepted.is_empty() {
                    continue;
                }
                let nodes = far_field(&case.tree, &cut, Rule::Octupole).nodes;
                let along = Along {
                    base,
                    height: probe.height,
                    per: probe.per,
                    share: LOCAL_SHARE * tolerance,
                    most: MAX_LOCAL_DEGREE,
                    forced,
                };
                let local = Local::gather(&nodes, Rule::Octupole, &along);
                let direct = reader.read(base, &Far { nodes, rule: Rule::Octupole }, 0.0);
                let through = reader.read(base, &local, 0.0);
                out.push(Measured {
                    audit: local.found.audit,
                    share: along.share,
                    error: excess(direct, through),
                    found: local.found,
                });
            }
        }
    }
    out
}

/// The worst `|through − direct|` over the two interferometers and the gradiometer, each less
/// 1e-13 of the phase it is of (W2's allowance), radians.
fn excess(direct: Reading, through: Reading) -> f64 {
    let arm = |d: f64, t: f64| (t - d).abs() - 1e-13 * d.abs();
    let gradiometer = ((through.lower - through.upper) - (direct.lower - direct.upper)).abs()
        - 1e-13 * (direct.lower.abs() + direct.upper.abs());
    arm(direct.lower, through.lower).max(arm(direct.upper, through.upper)).max(gradiometer)
}

/// W6, measured once a process: the worst over its reads of the error over the audit and of the
/// audit over its share, and how many reads used a series.
fn w6() -> &'static (f64, f64, usize) {
    static FOUND: std::sync::OnceLock<(f64, f64, usize)> = std::sync::OnceLock::new();
    FOUND.get_or_init(|| {
        let reads = measured(None);
        println!("degree, far, near, audit (rad), share (rad), error (rad)");
        for m in &reads {
            println!(
                "{} {} {} {:.2e} {:.2e} {:.2e}{}",
                m.found.degree,
                m.found.far,
                m.found.near,
                m.audit,
                m.share,
                m.error,
                if m.found.refused { " refused" } else { "" }
            );
        }
        let used: Vec<&Measured> = reads.iter().filter(|m| m.found.degree > 0).collect();
        let over = used.iter().map(|m| m.error.max(0.0) / m.audit).fold(0.0, f64::max);
        let share = used.iter().map(|m| m.audit / m.share).fold(0.0, f64::max);
        (over, share, used.len())
    })
}

#[test]
fn the_local_expansion_is_the_far_field() {
    let (over, share, used) = *w6();
    assert!(used >= 10, "only {used} reads used a series");
    assert!(over <= 1.0, "the flight read through the series is off by {over} of its audit");
    assert!(share <= 1.0, "an audit is {share} of its share");
    // Every degree within the cap (X5).
    assert!(measured(None).iter().all(|m| m.found.degree <= MAX_LOCAL_DEGREE));
    // Control: held at degree 4, the series misses its share somewhere.
    let held = measured(Some(4));
    let worst = held.iter().filter(|m| m.found.degree > 0).map(|m| m.error / m.share).fold(0.0, f64::max);
    assert!(worst > 1.0, "degree 4 stayed within its share: {worst}");
}

cav_validate::case! {
    test: validation_the_local_expansion_is_the_far_field,
    name: "world.local_expansion_is_the_far_field",
    subsystem: "world",
    slice: "S5",
    reference: Exact,
    against: "each cut's accepted nodes read through the flight by the tower's local expansion (near nodes directly, the rest by the Chebyshev series on the arm span) against the same nodes read directly, on world-box, the terrain world and a 40 m slab at 5 cm with two blocks, at every base that accepts a node and 1e-2, 1e-4, 1e-6 rad: the worst error over the expansion's audit, each interferometer and the gradiometer, less 1e-13 of the phase (control: degree 4 misses its share)",
    unit: Relative,
    tolerance: 1.0,
    measure: cav_validate::Measured::Error(w6().0),
}

cav_validate::case! {
    test: validation_the_local_expansion_s_audit_is_within_its_share,
    name: "world.local_expansion_within_its_share",
    subsystem: "world",
    slice: "S5",
    reference: Exact,
    against: "the same reads: each expansion's audit over its share of the tolerance (LOCAL_SHARE of it), the worst",
    unit: Relative,
    tolerance: 1.0,
    measure: cav_validate::Measured::Error(w6().1),
}

#[test]
fn the_series_reads_what_it_interpolates() {
    // A smooth function's Lobatto interpolant is exact at its samples and close between them;
    // Clenshaw's recurrence reads T_k exactly.
    let f = |t: f64| (1.3 * t).exp() / (2.0 - t);
    let n = 32;
    let points: Vec<f64> = (0..=n).map(|j| (PI * j as f64 / n as f64).cos()).collect();
    let a = lobatto_coefficients(&points.iter().map(|&t| f(t)).collect::<Vec<_>>());
    for &t in &points {
        assert!((clenshaw(&a, t) - f(t)).abs() <= 1e-13, "{t}");
    }
    for k in 0..6 {
        let mut unit = vec![0.0; 6];
        unit[k] = 1.0;
        let t: f64 = 0.37;
        assert!((clenshaw(&unit, t) - (k as f64 * t.acos()).cos()).abs() <= 1e-15, "T_{k}");
    }
    // Control: a degree far too low misses between the samples.
    let a = lobatto_coefficients(&[f(1.0), f(0.0), f(-1.0)]);
    assert!((clenshaw(&a, 0.5) - f(0.5)).abs() > 1e-3);
}

#[test]
fn a_point_off_the_line_is_read_directly() {
    // The series holds on its line and span only; elsewhere the nodes are read one by one, so the
    // answer is the same (to rounding) wherever it is asked.
    let case = near();
    let base = case.bases[0];
    let probe = aim(&case.spec, 0, base, 1e-4, Rule::Octupole);
    let cut = walk(&case.tree, &probe);
    let nodes = far_field(&case.tree, &cut, Rule::Octupole).nodes;
    assert!(!nodes.is_empty());
    let along = Along {
        base,
        height: probe.height,
        per: probe.per,
        share: 1e-5,
        most: MAX_LOCAL_DEGREE,
        forced: None,
    };
    let local = Local::gather(&nodes, Rule::Octupole, &along);
    assert!(local.series.is_some());
    let far = Far { nodes, rule: Rule::Octupole };
    let off = base + Vec3::new(0.25, 0.0, 0.0);
    assert_eq!(
        local.at(off, 2.0, 0.0, 1.0),
        local.near.at(off, 2.0, 0.0, 1.0) + local.far.at(off, 2.0, 0.0, 1.0)
    );
    assert!(
        (local.at(off, 2.0, 0.0, 1.0) - far.at(off, 2.0, 0.0, 1.0)).abs()
            <= 1e-12 * far.at(off, 2.0, 0.0, 1.0).abs()
    );
    // Above the span too.
    let top = probe.height + 1.0;
    assert_eq!(
        local.at(base, top, 0.0, 0.5),
        local.near.at(base, top, 0.0, 0.5) + local.far.at(base, top, 0.0, 0.5)
    );
    // Control: on the line, within the span, the series is what is read (not the nodes).
    let series = local.series.as_ref().unwrap();
    let z = [base.z + 2.0, base.z + 3.0];
    assert_eq!(
        local.at(base, 2.0, 0.0, 1.0),
        local.near.at(base, 2.0, 0.0, 1.0) + (series.value(z[1]) - series.value(z[0]))
    );
}
