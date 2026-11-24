//! Extended bodies against George's scripts (`VALIDATION-AND-RESEARCH.md` §2.3): the shaky wall
//! (`cuboid`), the cylinder against the point mass (`cylinder`), and the scaffold in one mode and
//! in many (`scaffold`, `scaffold_multimode`). The criteria are the S3 brief's (`docs/slices/S3.md`
//! §3.3, R12), written before these were run; the fixtures and how they were made are in
//! `fixtures/scripts/PROVENANCE.md`.
//!
//! The scripts' own constants are used wherever a phase is compared with a script (⁸⁸Sr's mass
//! 1.46e-25 kg, ħ = 1.055e-34 J s, G = 6.6743e-11, u₀ = 3.86 m/s); the scripts' pipeline (a
//! cubic-spline table of the potential, `interp1d` arms a step ahead, `quad`) is theirs alone.

use cav_core::Vec3;
use cav_core::forward::{Instrument, Node, PHASE_TOLERANCE, Reader, Source};
use cav_core::voxel::kernel::{TreeBody, cube_potential, cube_shift, prism_potential};
use cav_core::voxel::shape::{Cuboid, Cylinder, Shape};
use cav_core::voxel::{Grid, VoxTree, voxelise};
use oracle::fixture;

/// A fixture of any shape, its values in C order (`fixture` reads one-dimensional ones).
fn table(relative: &str) -> Vec<f64> {
    let path = oracle::fixtures().join(relative);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("fixture {relative}: {e}"));
    cav_core::npy::parse(&bytes)
        .and_then(cav_core::npy::Array::into_f64)
        .unwrap_or_else(|e| panic!("{relative}: {e}"))
        .1
}

/// The scripts' G.
const G: f64 = 6.6743e-11;

/// AION-10 with the scripts' atom, ħ and G (the engine's floor, sign and same-time arms). These
/// four scripts take G = 6.6743e-11, where the `fig2_*` scripts (and so `with_script_constants`)
/// take 6.67e-11: until the review a phase read here was 6.4e-4 smaller than the scripts'.
fn script_instrument() -> Instrument {
    Instrument { launch_velocity: 3.86, big_g: G, ..Instrument::aion10().with_script_constants() }
}

/// The potential of `tree` (its frame at `at`) at `point`, J/kg: every leaf, Morton order.
fn potential(tree: &VoxTree, at: Vec3, point: Vec3) -> f64 {
    G * tree
        .leaves
        .iter()
        .map(|l| {
            cube_potential(
                tree.leaf_mass(l),
                tree.side(l.level),
                point - at - tree.block_centre(l.key, l.level),
            )
        })
        .sum::<f64>()
}

/// Runs `job(i)` for `i` in `0..n` on every core; results in order.
fn parallel<T: Send>(n: usize, job: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out: Vec<std::sync::Mutex<Option<T>>> = (0..n).map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..threads.min(n.max(1)) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    let value = job(i);
                    *out[i].lock().unwrap() = Some(value);
                }
            });
        }
    });
    out.into_iter().map(|m| m.into_inner().unwrap().unwrap()).collect()
}

/// A series less its mean.
fn ac(series: &[f64]) -> Vec<f64> {
    let mean = series.iter().sum::<f64>() / series.len() as f64;
    series.iter().map(|v| v - mean).collect()
}

/// The largest difference of two series over the largest value of the second.
fn of_peak(a: &[f64], b: &[f64]) -> f64 {
    oracle::worst_of_peak(a, b)
}

// ======================================== THE SHAKY WALL ========================================

/// The wall: mass (kg), size (m), centre height (m).
const WALL: (f64, [f64; 3], f64) = (38_880.0, [0.225, 6.0, 12.0], 6.0);

/// The wall voxelised on a grid that fits its faces: 7.5 cm, 3 × 80 × 160 cells, merged.
fn wall_tree() -> VoxTree {
    let half = Vec3::from_array(WALL.1) * 0.5;
    let shape = Cuboid { centre: Vec3::ZERO, half };
    let grid = Grid::around(shape.bounds(), 0.075, 2).unwrap();
    assert_eq!(grid.cells, [3, 80, 160]);
    voxelise(&shape, &grid, WALL.0, 0).unwrap()
}

/// The worst relative difference, over the script's 50 × 50 grid of points, between the script's
/// potential and (a) the closed-form prism, (b) the wall's voxel tree.
fn wall_potentials() -> (f64, f64) {
    let (x0s, zs, table) = (
        fixture("scripts/cuboid/x0_grid.npy"),
        fixture("scripts/cuboid/z_grid.npy"),
        table("scripts/cuboid/V_grid.npy"),
    );
    let tree = wall_tree();
    let half = Vec3::from_array(WALL.1) * 0.5;
    let density = WALL.0 / (WALL.1[0] * WALL.1[1] * WALL.1[2]);
    let rows = parallel(x0s.len(), |i| {
        let centre = Vec3::new(x0s[i], 0.0, WALL.2);
        let (mut prism, mut voxels) = (0f64, 0f64);
        for (j, &z) in zs.iter().enumerate() {
            let point = Vec3::new(0.0, 0.0, z);
            let script = table[i * zs.len() + j];
            let closed = G * prism_potential(density, centre - half - point, centre + half - point);
            prism = prism.max((closed - script).abs() / script.abs());
            voxels = voxels.max((potential(&tree, centre, point) - script).abs() / script.abs());
        }
        (prism, voxels)
    });
    rows.into_iter().fold((0.0, 0.0), |(a, b), (c, d)| (f64::max(a, c), f64::max(b, d)))
}

cav_validate::case! {
    test: validation_cuboid_potential_matches_script,
    name: "oracle.script_cuboid_potential",
    subsystem: "oracle",
    slice: "S3",
    reference: Script,
    against: "cuboid: the script's potential (its nquad) on its 50 × 50 grid, against the closed-form prism and the wall's voxel tree (7.5 cm, a grid that fits its faces); the worse of the two",
    unit: Relative,
    tolerance: 1e-10,
    measure: {
        let (prism, voxels) = wall_potentials();
        eprintln!("cuboid potential: prism {prism:.3e}, voxels {voxels:.3e}");
        cav_validate::Measured::Error(prism.max(voxels))
    },
}

/// Where the wall's centre is at `t` seconds: x₀ = 5 + 1e-6 cos(2π·0.1 t) m, at the script's height.
fn wall_at(t: f64) -> Vec3 {
    Vec3::new(5.0 + 1e-6 * (std::f64::consts::TAU * 0.1 * t).cos(), 0.0, WALL.2)
}

/// The engine's series of the shaking wall at the script's shots (its read of the voxel wall,
/// lower minus upper, radians, with its mean).
fn wall_engine() -> &'static [f64] {
    static ONCE: std::sync::OnceLock<Vec<f64>> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        let times = fixture("scripts/cuboid/subsample_times.npy");
        let tree = wall_tree();
        let reader = Reader::new(script_instrument());
        let two_t = reader.instrument().flight_time();
        parallel(times.len(), |k| {
            let body = TreeBody::new(&tree, wall_at);
            // The script integrates shot k over [t_k − 1, t_k − 1 + 2T]: read out at its end.
            let reading = reader.read_to(Vec3::ZERO, &body, times[k] - 1.0 + two_t, PHASE_TOLERANCE).reading;
            reading.lower - reading.upper
        })
    })
}

/// One node of the scripts' sampling: seconds into the window, its weight (seconds), and the four
/// arm heights there (metres: the lower interferometer's lower and upper arm, then the upper's).
type ScriptNode = (f64, f64, [f64; 4]);

/// The scripts' sampling, written again: the four arms as tables at `j·step` holding where each
/// arm is `lag` steps later (the published scripts' `lag` is 1: `atom_path` stores the position
/// after a step under the step's start), joined linearly as `interp1d` joins them, over a window
/// of `steps` steps. Where the scripts call `quad`, each step is integrated here by Gauss–Legendre
/// at `order` points: between two nodes the arms are straight lines, so this is the integral
/// `quad` aims at, without its error (`scripts/script_pipeline.py` measures that: 2.5e-5 of the
/// cuboid's phase). The same at every shot: only the source moves.
fn script_nodes(lag: usize, step: f64, steps: usize, order: usize) -> Vec<ScriptNode> {
    let instrument = script_instrument();
    let [lower, upper] = instrument.arms();
    let rule = cav_core::forward::gauss_legendre(order);
    let (two_t, sep) = (instrument.flight_time(), instrument.ifo_separation);
    // Past 2T the scripts' tables hold the floor (`t_rem`), whether or not the arm has landed.
    let rise = |arm: &cav_core::forward::Arm, j: usize| {
        let tau = (j + lag) as f64 * step;
        if tau > two_t + 1e-12 { 0.0 } else { arm.rise(tau) }
    };
    let mut out = Vec::new();
    for j in 0..steps {
        let (l0, l1, u0, u1) = (rise(&lower, j), rise(&lower, j + 1), rise(&upper, j), rise(&upper, j + 1));
        if l0 == u0 && l1 == u1 {
            // Both arms on the floor at both ends: they are at one point for the whole step.
            continue;
        }
        for &(x, w) in &rule {
            let f = 0.5 * (x + 1.0);
            let (l, u) = (l0 + f * (l1 - l0), u0 + f * (u1 - u0));
            out.push(((j as f64 + f) * step, 0.5 * step * w, [l, u, sep + l, sep + u]));
        }
    }
    out
}

/// One shot by the scripts' sampling: `change(t, node, arm)` is what the source adds to the
/// potential at that arm's height on the tower's axis at time `t` (J/kg) beyond a part that is
/// the same at every shot. Lower minus upper, radians, of the changing part alone: a series of
/// these has the scripts' AC series, without differencing two sums of 114 rad.
fn script_shot(nodes: &[ScriptNode], launch: f64, change: &dyn Fn(f64, usize, usize) -> f64) -> f64 {
    let instrument = script_instrument();
    // Neumaier's compensated sum: up to 146·128·6 terms of both signs.
    let (mut sum, mut lost) = (0f64, 0f64);
    for (n, &(tau, w, _)) in nodes.iter().enumerate() {
        let t = launch + tau;
        let v = |arm: usize| change(t, n, arm);
        let x = w * ((v(1) - v(0)) - (v(3) - v(2)));
        let s = sum + x;
        lost += if sum.abs() >= x.abs() { (sum - s) + x } else { (x - s) + sum };
        sum = s;
    }
    instrument.atom_mass / instrument.hbar * (sum + lost)
}
/// The wall's potential change at height `z` on the axis when its centre is at `wall_at(t)`
/// rather than at rest (x₀ = 5 m), J/kg: the closed-form prism, which the script's `nquad` is
/// within 1.1e-14 of (S3.md §3.3).
fn wall_change(t: f64, z: f64) -> f64 {
    let half = Vec3::from_array(WALL.1) * 0.5;
    let density = WALL.0 / (WALL.1[0] * WALL.1[1] * WALL.1[2]);
    let point = Vec3::new(0.0, 0.0, z);
    let at = |centre: Vec3| G * prism_potential(density, centre - half - point, centre + half - point);
    at(wall_at(t)) - at(Vec3::new(5.0, 0.0, WALL.2))
}

/// The cuboid script's rule over its 20 shots: `lag` steps of offset, `step` seconds, on its
/// window (2T from `t_k − 1`). The AC series, radians.
fn wall_rule(lag: usize, step: f64) -> Vec<f64> {
    let times = fixture("scripts/cuboid/subsample_times.npy");
    let steps = (script_instrument().flight_time() / step).round() as usize;
    let nodes = script_nodes(lag, step, steps, 6);
    let change = |t: f64, n: usize, arm: usize| wall_change(t, nodes[n].2[arm]);
    ac(&parallel(times.len(), |k| script_shot(&nodes, times[k] - 1.0, &change)))
}

/// The scripts' step, seconds.
const DT: f64 = 0.01;

/// The cuboid script's rule taken to its limit: the offset gone, the whole flight, dt/128.
fn wall_limit() -> &'static [f64] {
    static ONCE: std::sync::OnceLock<Vec<f64>> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| wall_rule(0, DT / 128.0))
}

/// The copy: the script's rule as published (the offset, dt).
fn wall_copy() -> &'static [f64] {
    static ONCE: std::sync::OnceLock<Vec<f64>> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| wall_rule(1, DT))
}

cav_validate::case! {
    test: validation_cuboid_rule_copies_the_script,
    name: "oracle.script_cuboid_copy",
    subsystem: "oracle",
    slice: "S3",
    reference: Script,
    against: "cuboid: the script's rule copied (arms a step ahead of the wall, linear between steps, its window, each step integrated exactly, the closed-form prism for its spline) against its own table and rule with its quad replaced by exact integration (scripts/script_pipeline.py; its quad is 2.5e-5 of the phase off that); AC series, 20 shots; tolerance three times the script's own AC change when its table is doubled (3.57e-8 of the peak)",
    unit: Relative,
    tolerance: 1.1e-7,
    measure: {
        let published = ac(&fixture("scripts/cuboid/phase.npy"));
        let doubled = ac(&fixture("scripts/cuboid/phase_doubled.npy"));
        let script = ac(&fixture("scripts/cuboid/phase_exact.npy"));
        // The table's own error, as the brief defined the script's: held so that a changed
        // fixture is seen.
        let own = of_peak(&doubled, &published);
        assert!((own - 3.57e-8).abs() < 0.05e-8, "the fixture's doubling changed: {own:e}");
        // The script's quad, which the brief's measure of its error did not see (measured after
        // the copy first missed the published series by 2.54e-5: S3.md §8.2).
        let quad = of_peak(&published, &script);
        eprintln!(
            "cuboid: the copy against the published series {:.3e}; the script's quad against exact integration {quad:.3e}",
            of_peak(wall_copy(), &published)
        );
        assert!((2e-5..3e-5).contains(&quad), "the script's quad error changed: {quad:e}");
        // Control: without the offset the same rule is visibly not the script.
        let same_time = of_peak(&wall_rule(0, DT), &script);
        eprintln!("cuboid: the rule without the offset is {same_time:.3e} of the AC peak from the script");
        assert!(same_time > 1e-4, "the offset moves the series by only {same_time:e}");
        cav_validate::Measured::Error(of_peak(wall_copy(), &script))
    },
}

cav_validate::case! {
    test: validation_cuboid_rule_tends_to_the_engine,
    name: "oracle.script_cuboid_limit",
    subsystem: "oracle",
    slice: "S3",
    reference: Script,
    against: "cuboid: the script's rule with the offset gone, on the whole flight at dt/128, against the engine's read of the voxel wall; AC series, 20 shots; tolerance from the read's own audit (2.3e-10 rad of 2.86e-5)",
    unit: Relative,
    tolerance: 1e-5,
    measure: {
        let engine = ac(wall_engine());
        // Control: the published rule is not the limit, and this measure sees it.
        let published = of_peak(wall_copy(), &engine);
        assert!(published > 1e-3, "the published rule is already {published:e} from the engine");
        cav_validate::Measured::Error(of_peak(wall_limit(), &engine))
    },
}

cav_validate::case! {
    test: validation_cuboid_series_matches_script,
    name: "oracle.script_cuboid_series",
    subsystem: "oracle",
    slice: "S3",
    reference: Script,
    against: "cuboid: the engine's AC series minus the script's against what the script's rule predicts for it (its limit minus its copy, plus its quad's own error), shot by shot, over the largest prediction, two-sided; the limit rescaled, given the copy",
    unit: Relative,
    tolerance: 0.02,
    measure: {
        let (engine, script) = (ac(wall_engine()), ac(&fixture("scripts/cuboid/phase.npy")));
        let quad: Vec<f64> = script.iter().zip(ac(&fixture("scripts/cuboid/phase_exact.npy"))).map(|(p, e)| p - e).collect();
        let (limit, copy) = (wall_limit(), wall_copy());
        let (mut predicted, mut off) = (0f64, 0f64);
        for k in 0..engine.len() {
            let (measured, expected) = (engine[k] - script[k], limit[k] - copy[k] - quad[k]);
            predicted = predicted.max(expected.abs());
            off = off.max((measured - expected).abs());
        }
        let peak = script.iter().fold(0f64, |m, v| m.max(v.abs()));
        eprintln!(
            "cuboid series: engine against the script {:.3e} of the AC peak; predicted {:.3e}; apart by {:.3e} of the prediction",
            of_peak(&engine, &script),
            predicted / peak,
            off / predicted
        );
        // Control: the prediction is not nothing; an engine equal to the script is a whole
        // prediction away.
        assert!(predicted / peak > 1e-4, "nothing is predicted: {predicted:e}");
        cav_validate::Measured::Error(off / predicted)
    },
}

// ======================================== THE CYLINDER ========================================

/// The cylinder's and the point's largest AC |ΔΦ| for height `h` and centre height `h0` (metres),
/// R = 0.5 m, 100 kg, moving as x0 = 3 + 0.1 cos(2π·0.1 t): over one period of the oscillation
/// (five shots; the script's fifty are ten periods of the same five).
fn cylinder_and_point(h: f64, h0: f64) -> (f64, f64) {
    let reader = Reader::new(script_instrument());
    let two_t = reader.instrument().flight_time();
    let x0 = |t: f64| 3.0 + 0.1 * (std::f64::consts::TAU * 0.1 * t).cos();
    let times: Vec<f64> = (0..5).map(|k| two_t + 2.0 * f64::from(k)).collect();
    let radius: f64 = 0.5;
    let pitch = (radius / 16.0).min(h / 4.0).max(h / 400.0);
    let shape = Cylinder { centre: Vec3::ZERO, radius, half_length: 0.5 * h, axis: 2 };
    let tree = voxelise(&shape, &Grid::around(shape.bounds(), pitch, 2).unwrap(), 100.0, 0).unwrap();
    // The shots are read side by side: a crossover's bisection is otherwise one read at a time.
    let largest = |source: &(dyn Source + Sync)| -> f64 {
        let series = parallel(times.len(), |k| {
            let r = reader.read_to(Vec3::ZERO, source, times[k], PHASE_TOLERANCE).reading;
            r.lower - r.upper
        });
        ac(&series).iter().fold(0f64, |m, v| m.max(v.abs()))
    };
    let body = TreeBody::new(&tree, |t| Vec3::new(x0(t), 0.0, h0));
    let point = cav_core::forward::Mass::point(100.0, |t| Vec3::new(x0(t), 0.0, h0));
    (largest(&body), largest(&point))
}

/// Every (h0, H) of the script's sweep: `(h0, H, engine's ratio, script's ratio)`, and the worst
/// relative difference of the ratios.
fn cylinder_ratios() -> (Vec<(f64, f64, f64, f64)>, f64) {
    let (hs, h0s, results, points) = (
        fixture("scripts/cylinder/H_vals.npy"),
        fixture("scripts/cylinder/h0_vals.npy"),
        table("scripts/cylinder/results.npy"),
        fixture("scripts/cylinder/pm_refs.npy"),
    );
    let cases: Vec<(usize, usize)> =
        (0..h0s.len()).flat_map(|i| (0..hs.len()).map(move |j| (i, j))).collect();
    let ours = parallel(cases.len(), |c| cylinder_and_point(hs[cases[c].1], h0s[cases[c].0]));
    let mut rows = Vec::new();
    let mut worst = 0f64;
    for (c, &(i, j)) in cases.iter().enumerate() {
        let engine = ours[c].0 / ours[c].1;
        let script = results[i * hs.len() + j] / points[i];
        // "Within 5e-3 of each other" (S3.md §3.3): the ratios' difference, not a relative one,
        // which a ratio near nought (a cylinder whose field nearly cancels) would blow up.
        worst = worst.max((engine - script).abs());
        rows.push((h0s[i], hs[j], engine, script));
    }
    (rows, worst)
}

cav_validate::case! {
    test: validation_cylinder_vs_point_mass,
    name: "oracle.script_cylinder_ratio",
    subsystem: "oracle",
    slice: "S3",
    reference: Script,
    against: "cylinder: the ratio of the cylinder's largest AC |ΔΦ| to the point mass's, 4 heights h0 × 9 heights H (R = 0.5 m, 100 kg, x0 = 3 + 0.1 cos 2π·0.1t), engine (voxel cylinder) against script: the ratios' largest difference",
    unit: Relative,
    tolerance: 5e-3,
    measure: {
        let (rows, worst) = cylinder_ratios();
        for (h0, h, engine, script) in &rows {
            eprintln!("h0 {h0:>4} H {h:>7.3}: engine {engine:.5} script {script:.5}");
        }
        cav_validate::Measured::Error(worst)
    },
}

/// Where the cylinder's ratio leaves 1 by 10% for centre height `h0`, metres: bisection on H.
/// Bisects between `lo` and `hi` (metres; the ratio within 10% at `lo` and not at `hi`), five
/// times in the logarithm: to a twenty-fifth of the bracket's ratio.
fn crossover(h0: f64, (mut lo, mut hi): (f64, f64)) -> f64 {
    let leaves = |h: f64| {
        let (c, p) = cylinder_and_point(h, h0);
        (c / p - 1.0).abs() > 0.1
    };
    let ends = parallel(2, |k| leaves([lo, hi][k]));
    assert!(!ends[0] && ends[1], "h0 {h0}: the ratio does not cross 10% between {lo} and {hi} m");
    for _ in 0..5 {
        let mid = (lo * hi).sqrt();
        if leaves(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    (lo * hi).sqrt()
}

#[test]
fn the_point_mass_fails_where_the_script_says() {
    // The plan's reading of George's result (VALIDATION §2.3): the error passes 10% when the
    // body's vertical extent passes about 10% of the 10 m baseline. The brief held it as a
    // crossover between 0.5 and 2 m at every h0 (S3.md §3.3), and said that if the scripts said
    // otherwise it was a finding for George, not a test to loosen. They do: on the script's own
    // grid the ratio crosses 0.9 between H = 2.725 and 6.06 m at h0 = 2.5 m. Measured
    // (24 Nov 2026): the engine's crossovers are 2.20, 1.90, 3.63 and 2.76 m (S3.md §7). What
    // is held here is the oracle's relation: the engine crosses within one step of the script's
    // grid of the bracket where the script's own ratio crosses (at h0 = 5 m the script is 0.8997
    // at 2.725 m and the engine 0.9002, within the ratio case's 5e-3, so the crossing may fall
    // either side of a grid point). The ratios themselves are held by
    // `validation_cylinder_vs_point_mass`; this records where the point mass fails.
    let (hs, h0s, results, points) = (
        fixture("scripts/cylinder/H_vals.npy"),
        fixture("scripts/cylinder/h0_vals.npy"),
        table("scripts/cylinder/results.npy"),
        fixture("scripts/cylinder/pm_refs.npy"),
    );
    let bracket = |i: usize| {
        let ratio = |j: usize| results[i * hs.len() + j] / points[i];
        (0..hs.len() - 1)
            .find(|&j| (ratio(j) - 1.0).abs() <= 0.1 && (ratio(j + 1) - 1.0).abs() > 0.1)
            .expect("the script's ratio crosses 10%")
    };
    // The engine is bisected between the script's grid points a step either side of its bracket.
    // What is checked is the engine's ratio at the two ends (`crossover` asserts that it is within
    // 10% at the lower and not at the upper): a bisection always returns a point inside its
    // interval, so the point found is reported, not asserted.
    let found = parallel(h0s.len(), |i| {
        let j = bracket(i);
        crossover(h0s[i], (hs[j.saturating_sub(1)], hs[(j + 2).min(hs.len() - 1)]))
    });
    for (i, (h0, h)) in h0s.iter().zip(&found).enumerate() {
        let j = bracket(i);
        eprintln!(
            "h0 {h0}: the engine's ratio leaves 1 by 10% at H = {h:.3} m; the script's between {} and {} m",
            hs[j],
            hs[j + 1]
        );
    }
    // The plan's reading, said rather than asserted: how far it is from what both show.
    eprintln!(
        "within 0.5 to 2 m (a tenth of the baseline, give or take two): {:?}",
        found.iter().map(|h| (0.5..=2.0).contains(h)).collect::<Vec<_>>()
    );
}

// ======================================== THE SCAFFOLD ========================================

/// One member: its tree, the centre of its tree at rest (metres), its amplitude (metres for α = 1).
type Member = (VoxTree, Vec3, f64);

/// George's scaffold as in a fixture: each member a voxel cylinder at `pitch`.
fn scaffold(case: &str, pitch: f64) -> Vec<Member> {
    let dir = format!("scripts/{case}");
    let centres = table(&format!("{dir}/member_centre.npy"));
    let (masses, radii, lengths, axes, amplitudes) = (
        fixture(&format!("{dir}/member_mass.npy")),
        fixture(&format!("{dir}/member_radius.npy")),
        fixture(&format!("{dir}/member_length.npy")),
        fixture(&format!("{dir}/member_axis.npy")),
        fixture(&format!("{dir}/amplitudes.npy")),
    );
    parallel(masses.len(), |i| {
        let axis = axes[i] as usize;
        let shape = Cylinder { centre: Vec3::ZERO, radius: radii[i], half_length: 0.5 * lengths[i], axis };
        let tree = voxelise(&shape, &Grid::around(shape.bounds(), pitch, 4).unwrap(), masses[i], 0).unwrap();
        (tree, Vec3::new(centres[3 * i], centres[3 * i + 1], centres[3 * i + 2]), amplitudes[i])
    })
}

/// The scaffold's potential at `point` with every member moved by its amplitude × `alpha` in x, J/kg.
fn scaffold_potential(members: &[Member], alpha: f64, point: Vec3) -> f64 {
    members.iter().map(|(tree, centre, a)| potential(tree, *centre + Vec3::X * (a * alpha), point)).sum()
}

/// How the scaffold's potential at `point` changes, J/kg, when every member moves by its
/// amplitude × `alpha` in x: leaf by leaf, without differencing two potentials (`cube_shift`).
/// The scaffold vibrates about the tower's axis, so the members' first-order changes cancel in
/// pairs and what is left is about 1e-8 of the potential. Two sums of whole potentials
/// (8e-8 J/kg, each rounded to about 1e-21 J/kg over its leaves) differenced afterwards keep that
/// change to about 1e-5 of itself: enough for case (a), held to 1e-2 of the largest change, which
/// does it that way (`scaffold_potential`); not for (b), held to 1e-6, where differencing gave
/// 4.5e-5 and this was written.
fn scaffold_change(members: &[Member], alpha: f64, point: Vec3) -> f64 {
    members
        .iter()
        .map(|(tree, centre, a)| {
            let shift = Vec3::X * (a * alpha);
            G * tree
                .leaves
                .iter()
                .map(|l| {
                    let d = point - *centre - tree.block_centre(l.key, l.level);
                    cube_shift(tree.leaf_mass(l), tree.side(l.level), d, shift)
                })
                .sum::<f64>()
        })
        .sum()
}

/// (a): the change of the potential with α, `V(α_i, z_j) − V(α_ref, z_j)` (`α_ref` the scripts'
/// grid's α nearest zero), engine against the scripts' own integrand integrated tightly
/// (`tight_V.npy`, `scripts/scaffold_reference.py`), over the largest change.
///
/// The brief's reference was the scripts' table (`V_grid`). Measured first (24 Nov 2026), the
/// table is not accurate enough to be one: its `nquad` stops at an absolute tolerance far above
/// these values, and its change at α = −0.636, z = 7.57 m is −1.80e-14 J/kg where the same
/// integrand integrated tightly gives +9.7e-16. The criterion (1e-2 of the largest change) is
/// unchanged; the reference is the scripts' integrand done well, and the table's own error is
/// printed beside it.
fn scaffold_changes(case: &str, pitch: f64) -> f64 {
    let dir = format!("scripts/{case}");
    let (alphas, zs, tight) = (
        fixture(&format!("{dir}/tight_alpha.npy")),
        fixture(&format!("{dir}/tight_z.npy")),
        table(&format!("{dir}/tight_V.npy")),
    );
    let members = scaffold(case, pitch);
    let ours: Vec<f64> = parallel(alphas.len() * zs.len(), |c| {
        scaffold_potential(&members, alphas[c / zs.len()], Vec3::new(0.0, 0.0, zs[c % zs.len()]))
    });
    // Row 0 is α_ref.
    let change = |v: &[f64], i: usize, j: usize| v[i * zs.len() + j] - v[j];
    let (mut worst, mut largest) = (0f64, 0f64);
    for i in 1..alphas.len() {
        for j in 0..zs.len() {
            let reference = change(&tight, i, j);
            largest = largest.max(reference.abs());
            worst = worst.max((change(&ours, i, j) - reference).abs());
        }
    }
    // The scripts' table at the same points, against the same reference.
    let (grid_alphas, grid_zs, grid) = (
        fixture(&format!("{dir}/alpha_grid.npy")),
        fixture(&format!("{dir}/z_grid.npy")),
        table(&format!("{dir}/V_grid.npy")),
    );
    let at =
        |values: &[f64], x: f64| values.iter().position(|v| *v == x).expect("a point of the scripts' grid");
    let mut script = 0f64;
    for i in 1..alphas.len() {
        for (j, &z) in zs.iter().enumerate() {
            let (gi, g0, gj) = (at(&grid_alphas, alphas[i]), at(&grid_alphas, alphas[0]), at(&grid_zs, z));
            let theirs = grid[gi * grid_zs.len() + gj] - grid[g0 * grid_zs.len() + gj];
            script = script.max((theirs - change(&tight, i, j)).abs());
        }
    }
    eprintln!(
        "{case}: largest change of V with α {largest:.3e} J/kg; engine against the tight reference {worst:.3e}; the \
         scripts' table against it {script:.3e}"
    );
    worst / largest
}

/// The flight's nodes at `level` halvings, for both paths alike.
fn nodes(reader: &Reader, level: u32) -> Vec<Node> {
    let mut out = Vec::new();
    reader.visit(level, &[], &mut |n| out.push(n));
    out
}

/// The modes of a fixture's α(t) as `(frequency, scale)`, and the largest |α|.
fn modes_of(case: &str) -> (Vec<(f64, f64)>, f64) {
    let modes: Vec<(f64, f64)> = if case == "scaffold" {
        vec![(22.4, 1.0)]
    } else {
        [2.30, 2.34, 12.7, 35.1, 37.0, 38.7, 39.4, 40.5].map(|f| (f, 1.0)).to_vec()
    };
    let largest = modes.iter().map(|m| m.1.abs()).sum();
    (modes, largest)
}

fn alpha(modes: &[(f64, f64)], t: f64) -> f64 {
    modes.iter().map(|(f, s)| s * (std::f64::consts::TAU * f * t).cos()).sum()
}

/// The gradiometer's phase read out at `t` by the displaced-body read: every member moved to
/// α(τ) at every node, every leaf read.
fn direct(reader: &Reader, members: &[Member], modes: &[(f64, f64)], nodes: &[Node], t: f64) -> f64 {
    let i = reader.instrument();
    let launch = t - i.flight_time();
    let mut sum = 0.0;
    for node in nodes {
        let a = alpha(modes, launch + node.tau);
        let v = |z: f64| scaffold_change(members, a, Vec3::new(0.0, 0.0, z));
        let lower = v(node.upper) - v(node.lower);
        let upper = v(i.ifo_separation + node.upper) - v(i.ifo_separation + node.lower);
        sum += node.weight * (lower - upper);
    }
    i.atom_mass / i.hbar * sum
}

/// The table method, the scripts' scalar trick done with the engine's own potential: for each
/// node, V at its four arm points through Chebyshev points in α spanning ±`largest`; each shot is
/// then the interpolating polynomial at α(τ).
struct Table {
    largest: f64,
    /// For each node, its four arm points' values at the Chebyshev points (lower and upper arm of
    /// the lower interferometer, then of the upper).
    values: Vec<[Vec<f64>; 4]>,
}

/// Chebyshev points of the table in α.
const CHEBYSHEV: usize = 5;

fn chebyshev(largest: f64, k: usize) -> f64 {
    largest * (std::f64::consts::PI * (k as f64 + 0.5) / CHEBYSHEV as f64).cos()
}

impl Table {
    fn new(reader: &Reader, members: &[Member], largest: f64, nodes: &[Node]) -> Self {
        let i = reader.instrument();
        let values = parallel(nodes.len(), |n| {
            let node = nodes[n];
            let heights =
                [node.lower, node.upper, i.ifo_separation + node.lower, i.ifo_separation + node.upper];
            heights.map(|z| {
                (0..CHEBYSHEV)
                    .map(|k| scaffold_change(members, chebyshev(largest, k), Vec3::new(0.0, 0.0, z)))
                    .collect()
            })
        });
        Self { largest, values }
    }

    /// The interpolating polynomial through the Chebyshev points at `a` (barycentric form).
    fn at(&self, values: &[f64], a: f64) -> f64 {
        let (mut num, mut den) = (0.0, 0.0);
        for (k, &v) in values.iter().enumerate() {
            let x = chebyshev(self.largest, k);
            if a == x {
                return v;
            }
            let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
            let c = sign * (std::f64::consts::PI * (k as f64 + 0.5) / CHEBYSHEV as f64).sin() / (a - x);
            num += c * v;
            den += c;
        }
        num / den
    }

    fn phase(&self, reader: &Reader, modes: &[(f64, f64)], nodes: &[Node], t: f64) -> f64 {
        let i = reader.instrument();
        let launch = t - i.flight_time();
        let mut sum = 0.0;
        for (node, values) in nodes.iter().zip(&self.values) {
            let a = alpha(modes, launch + node.tau);
            let v: Vec<f64> = values.iter().map(|vals| self.at(vals, a)).collect();
            sum += node.weight * ((v[1] - v[0]) - (v[3] - v[2]));
        }
        i.atom_mass / i.hbar * sum
    }
}

/// (b) the displaced-body read against the table method at the same nodes, three shots, over the
/// largest AC phase of those. Members at 4 cm: both paths read the same bodies.
fn scaffold_paths(case: &str, level: u32) -> f64 {
    let reader = Reader::new(script_instrument());
    let (modes, largest) = modes_of(case);
    let members = scaffold(case, 0.04);
    let coarse = nodes(&reader, level);
    let table = Table::new(&reader, &members, largest, &coarse);
    let times = fixture(&format!("scripts/{case}/subsample_times.npy"));
    let few: Vec<f64> = times.iter().take(3).copied().collect();
    let by_table: Vec<f64> = few.iter().map(|&t| table.phase(&reader, &modes, &coarse, t)).collect();
    let by_body = parallel(few.len(), |k| direct(&reader, &members, &modes, &coarse, few[k]));
    let mean = by_body.iter().sum::<f64>() / by_body.len() as f64;
    let peak = by_body.iter().fold(0f64, |m, v| m.max((v - mean).abs()));
    let paths = by_body.iter().zip(&by_table).fold(0f64, |m, (a, b)| m.max((a - b).abs())) / peak;
    eprintln!("{case}: the two paths {paths:.2e} of the AC peak");
    paths
}

/// What (c) compares, for one scaffold case: the engine's series by the scripts' sampling (the
/// copy: arms a step ahead, linear between steps, the scripts' window, each step integrated
/// exactly) on its 48 voxel members at 1 cm, the potential's change through the table method's
/// Chebyshev polynomial in α at every node; and the scripts' own pipeline on their integrand
/// integrated tightly (`scripts/script_pipeline.py`) at its finest grid and the grid before it.
struct ScaffoldSeries {
    /// The engine's AC series, radians.
    ours: Vec<f64>,
    /// The pipeline's AC series at 24 α × 480 heights, radians.
    pipeline: Vec<f64>,
    /// The same at 240 heights: the last doubling.
    half: Vec<f64>,
    /// The same at 48 α.
    alphas: Vec<f64>,
    /// The scripts' published AC series (their `nquad` table), radians.
    published: Vec<f64>,
}

/// The members' pitch for (c), metres.
const MEMBER_PITCH: f64 = 0.01;

fn scaffold_series(case: &str) -> ScaffoldSeries {
    let (modes, largest) = modes_of(case);
    let members = scaffold(case, MEMBER_PITCH);
    let steps = (script_instrument().flight_time() / DT).round() as usize;
    let script = script_nodes(1, DT, steps, 8);
    // The change of the potential at each node's four arm heights, at the Chebyshev points in α.
    let values: Vec<[Vec<f64>; 4]> = parallel(script.len(), |n| {
        script[n].2.map(|z| {
            (0..CHEBYSHEV)
                .map(|k| scaffold_change(&members, chebyshev(largest, k), Vec3::new(0.0, 0.0, z)))
                .collect()
        })
    });
    let table = Table { largest, values };
    let change = |t: f64, n: usize, arm: usize| table.at(&table.values[n][arm], alpha(&modes, t));
    let times = fixture(&format!("scripts/{case}/subsample_times.npy"));
    let two_t = script_instrument().flight_time();
    let ours = ac(&times.iter().map(|&t| script_shot(&script, t - two_t, &change)).collect::<Vec<_>>());
    let dir = format!("scripts/{case}");
    ScaffoldSeries {
        ours,
        pipeline: fixture(&format!("{dir}/phase_ac_pipeline.npy")),
        half: fixture(&format!("{dir}/phase_ac_pipeline_half.npy")),
        alphas: fixture(&format!("{dir}/phase_ac_pipeline_alpha_doubled.npy")),
        published: fixture(&format!("{dir}/phase_ac.npy")),
    }
}

/// (c) for one case: the engine's copy against the pipeline, over the pipeline's AC peak, after
/// checking the fixture's own convergence against the number the tolerance was made from
/// (`doubling`, its change at the last doubling of heights, as a fraction of its peak).
fn scaffold_series_error(case: &str, doubling: f64) -> f64 {
    // The allowance of 1e-3 for the members stands on case (a): at this pitch their potential
    // change is within 1e-4 of its largest against the tight reference (2.3e-5 at 1 cm; at 2 cm
    // it is 1.3e-4, which is why the members are not coarser).
    let members = scaffold_changes(case, MEMBER_PITCH);
    assert!(
        members <= 1e-4,
        "{case}: the members at {MEMBER_PITCH} m are {members:e} off the potential change"
    );
    let s = scaffold_series(case);
    let (own, alphas) = (of_peak(&s.half, &s.pipeline), of_peak(&s.alphas, &s.pipeline));
    let peak = |v: &[f64]| v.iter().fold(0f64, |m, x| m.max(x.abs()));
    eprintln!(
        "{case}: engine peak AC {:.4e} rad, the pipeline's {:.4e}, the published series' {:.4e} (its nquad noise); \
         engine against the pipeline {:.3e}; the pipeline's last doubling {own:.3e}, α doubled {alphas:.3e}; \
         engine against the published series {:.3}",
        peak(&s.ours),
        peak(&s.pipeline),
        peak(&s.published),
        of_peak(&s.ours, &s.pipeline),
        of_peak(&s.ours, &s.published)
    );
    assert!((own - doubling).abs() <= 0.05 * doubling, "{case}: the pipeline's convergence changed: {own:e}");
    assert!(alphas <= own, "{case}: α is not resolved: {alphas:e}");
    // Control: the published series is noise, and the engine is nowhere near it.
    assert!(of_peak(&s.ours, &s.published) > 0.5, "{case}: the engine is near the published series");
    of_peak(&s.ours, &s.pipeline)
}

cav_validate::case! {
    test: validation_scaffold_potential_change_matches_script,
    name: "oracle.script_scaffold_potential",
    subsystem: "oracle",
    slice: "S3",
    reference: Script,
    against: "scaffold: V(α, z) − V(α₀, z) at 12 of the script's points, its own integrand integrated tightly (its table's nquad is far coarser than the change), against the engine's 48 voxel members (1 cm), over the largest change",
    unit: Relative,
    tolerance: 1e-2,
    measure: cav_validate::Measured::Error(scaffold_changes("scaffold", 0.01)),
}

cav_validate::case! {
    test: validation_scaffold_multimode_potential_change_matches_script,
    name: "oracle.script_scaffold_multimode_potential",
    subsystem: "oracle",
    slice: "S3",
    reference: Script,
    against: "scaffold_multimode: V(α, z) − V(α₀, z) at 12 of the script's points (α to ±8), its integrand integrated tightly, against the engine's voxel members (1 cm), over the largest change",
    unit: Relative,
    tolerance: 1e-2,
    measure: cav_validate::Measured::Error(scaffold_changes("scaffold_multimode", 0.01)),
}

cav_validate::case! {
    test: validation_scaffold_single_mode,
    name: "oracle.scaffold_single_mode_paths",
    subsystem: "oracle",
    slice: "S3",
    reference: Exact,
    against: "scaffold_single_mode: the displaced-body read (every leaf of every member at α(τ)) against the table method (V(α) per node, Chebyshev in α), the same nodes, three shots, over the largest AC phase",
    unit: Relative,
    tolerance: 1e-6,
    measure: cav_validate::Measured::Error(scaffold_paths("scaffold", 3)),
}

cav_validate::case! {
    test: validation_scaffold_multimode_scalar_trick,
    name: "oracle.scaffold_multimode_paths",
    subsystem: "oracle",
    slice: "S3",
    reference: Exact,
    against: "scaffold_multimode_scalar_trick: the displaced-body read against the table method, the script's eight modes sharing one α, three shots, over the largest AC phase",
    unit: Relative,
    tolerance: 1e-6,
    measure: cav_validate::Measured::Error(scaffold_paths("scaffold_multimode", 4)),
}

cav_validate::case! {
    test: validation_scaffold_series_against_script,
    name: "oracle.script_scaffold_series",
    subsystem: "oracle",
    slice: "S3",
    reference: Script,
    against: "scaffold: the engine's AC series by the script's sampling (arms a step ahead, its window; 48 voxel members at 1 cm, Chebyshev in α), 50 shots, against the script's pipeline on its integrand integrated tightly (scripts/script_pipeline.py: its spline at 24 α × 480 heights, its interp1d arms, quad replaced by exact integration); tolerance three times the pipeline's last doubling (7.39e-4) plus 1e-3 for the members. Its published series is its nquad noise (3600 times the signal) and is not compared",
    unit: Relative,
    tolerance: 3.22e-3,
    measure: cav_validate::Measured::Error(scaffold_series_error("scaffold", 7.389e-4)),
}

cav_validate::case! {
    test: validation_scaffold_multimode_series_against_script,
    name: "oracle.script_scaffold_multimode_series",
    subsystem: "oracle",
    slice: "S3",
    reference: Script,
    against: "scaffold_multimode: the engine's AC series by the script's sampling, 100 shots, against the script's pipeline on its integrand integrated tightly (24 α × 480 heights, quad replaced by exact integration); tolerance three times the pipeline's last doubling (8.44e-6) plus 1e-3 for the members. Its published series is its nquad noise and is not compared",
    unit: Relative,
    tolerance: 1.03e-3,
    measure: cav_validate::Measured::Error(scaffold_series_error("scaffold_multimode", 8.441e-6)),
}
