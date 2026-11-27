//! A summary's moments against direct sums, its shift against recomputing, and its expansion
//! against the cubes' exact kernel within its bound (`docs/slices/S5.md` §2, O1–O2 in small).

use super::*;
use crate::forward::gauss_legendre;
use crate::voxel::kernel::cube_difference;

/// A fixed scatter of numbers in [0, 1): no random crate, the same everywhere.
struct Scatter(u64);

impl Scatter {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// `n` cubes inside the box of side `size` centred on `centre`: sides from 1/16 to 1/4 of it,
/// masses of both signs (mostly positive, as departures are), placed lopsidedly so no low moment
/// vanishes.
fn cubes(n: usize, centre: Vec3, size: f64, seed: u64) -> Vec<(Vec3, f64, f64)> {
    let mut s = Scatter(seed);
    (0..n)
        .map(|_| {
            let side = size * (0.0625 + 0.1875 * s.next());
            let room = 0.5 * (size - side);
            // Squared to push the cubes towards one corner.
            let at = |s: &mut Scatter| room * (2.0 * s.next().powi(2) - 1.0);
            let offset = Vec3::new(at(&mut s), at(&mut s), at(&mut s));
            let mass = 1.0e3 * (s.next() - 0.15);
            (centre + offset, mass, side)
        })
        .collect()
}

/// Each cube as the points of the `points`-point Gauss–Legendre product rule on it, each with its
/// share of the mass (side 0).
fn as_points(cubes: &[(Vec3, f64, f64)], points: usize) -> Vec<(Vec3, f64, f64)> {
    let rule = gauss_legendre(points);
    let mut out = Vec::new();
    for &(at, mass, side) in cubes {
        let h = 0.5 * side;
        for &(x, wx) in &rule {
            for &(y, wy) in &rule {
                for &(z, wz) in &rule {
                    out.push((at + Vec3::new(x, y, z) * h, mass * wx * wy * wz / 8.0, 0.0));
                }
            }
        }
    }
    out
}

/// The worst difference of two summaries' raw moments, each order relative to `A_0 size^k`.
fn raw_error(a: &Summary, b: &Summary, size: f64) -> f64 {
    let scale = a.absolute[0].max(b.absolute[0]);
    let mut worst = (a.mass - b.mass).abs() / scale;
    let orders: [(&[f64], &[f64], i32); 3] =
        [(&a.first, &b.first, 1), (&a.second, &b.second, 2), (&a.third, &b.third, 3)];
    for (x, y, k) in orders {
        for (p, q) in x.iter().zip(y) {
            worst = worst.max((p - q).abs() / (scale * size.powi(k)));
        }
    }
    worst
}

/// The cubes' true `Σ |m| s^l` about `centre` (odd orders by Cauchy–Schwarz, never below it).
fn true_absolute(centre: Vec3, cubes: &[(Vec3, f64, f64)]) -> [f64; ABSOLUTE_ORDERS] {
    cubes_absolute(centre, cubes.iter().copied())
}

#[test]
fn a_cube_s_moments_are_those_of_its_gauss_points() {
    let centre = Vec3::new(3.0, -2.0, 1.5);
    let some = cubes(40, centre, 2.0, 7);
    let summary = Summary::of_cubes(centre, some.iter().copied());
    // Two points a side integrate cubics exactly: the raw moments to the third.
    let points = Summary::of_cubes(centre, as_points(&some, 2));
    let raw = raw_error(&summary, &points, 2.0);
    assert!(raw <= 1e-14, "raw moments {raw:e}");
    // Three a side integrate quintics: A_2 and A_4 are exact for a cube.
    let fine = Summary::of_cubes(centre, as_points(&some, 3));
    for k in [0, 2, 4] {
        let e = (summary.absolute[k] - fine.absolute[k]).abs() / summary.absolute[k];
        assert!(e <= 1e-14, "A_{k} {e:e}");
    }
    // And for every order the cube's bound is no smaller than the truth, and the reach holds it.
    let truth = true_absolute(centre, &some);
    for k in 0..ABSOLUTE_ORDERS {
        assert!(
            summary.absolute[k] >= truth[k] * (1.0 - 1e-14),
            "A_{k}: {} < {}",
            summary.absolute[k],
            truth[k]
        );
    }
    let far = some
        .iter()
        .map(|&(at, _, side)| (at - centre).length() + 0.5 * side * 3f64.sqrt())
        .fold(0.0, f64::max);
    assert_eq!(summary.reach, far);
    // Control: the cubes as points at their centres lose their own second moments.
    let lumped = Summary::of_cubes(centre, some.iter().map(|&(at, m, _)| (at, m, 0.0)));
    assert!(raw_error(&summary, &lumped, 2.0) > 1e-4);
}

#[test]
fn the_exact_even_powers_of_a_cube_are_its_quadrature() {
    // cube_even_powers against a Gauss rule exact for the degree (n + 1 points a side for 2n).
    let rule = gauss_legendre(9);
    for (u, side) in [(Vec3::new(0.3, -1.2, 0.7), 0.8), (Vec3::ZERO, 1.0), (Vec3::new(5.0, 0.0, 0.0), 0.25)] {
        let powers = cube_even_powers(u, side);
        for (n, &exact) in powers.iter().enumerate() {
            let mut sum = 0.0;
            for &(x, wx) in &rule {
                for &(y, wy) in &rule {
                    for &(z, wz) in &rule {
                        let p = u + Vec3::new(x, y, z) * (0.5 * side);
                        sum += wx * wy * wz / 8.0 * p.dot(p).powi(n as i32);
                    }
                }
            }
            assert!((exact - sum).abs() <= 1e-13 * exact, "n {n} at {u:?}: {exact} against {sum}");
        }
    }
}

#[test]
fn a_shifted_summary_is_the_summary_about_the_new_centre() {
    let (from, size) = (Vec3::new(-4.0, 6.0, 0.5), 1.6);
    let some = cubes(60, from, size, 11);
    let summary = Summary::of_cubes(from, some.iter().copied());
    // About a node's size away in each of several directions, as a child moves to its parent.
    let mut unmoved_fails = false;
    for step in [Vec3::new(0.8, 0.8, 0.8), Vec3::new(-0.8, 0.8, -0.8), Vec3::new(1.3, -0.2, 0.4)] {
        let to = from + step;
        let moved = summary.shifted(to);
        let direct = Summary::of_cubes(to, some.iter().copied());
        let raw = raw_error(&moved, &direct, size);
        assert!(raw <= 1e-13, "raw {raw:e} by {step:?}");
        let truth = true_absolute(to, &some);
        for l in 0..ABSOLUTE_ORDERS {
            assert!(moved.absolute[l] >= truth[l], "A_{l} {} < {}", moved.absolute[l], truth[l]);
        }
        assert!(moved.reach >= direct.reach);
        // Control: the absolute moments not moved at all fall below the truth.
        unmoved_fails |= (1..ABSOLUTE_ORDERS).any(|l| summary.absolute[l] < truth[l]);
    }
    assert!(unmoved_fails);
}

#[test]
fn a_sum_of_parts_is_the_summary_of_all_their_matter() {
    // Eight octants of a box of side 4, each summarised about its own centre, summed about the
    // box's: a parent from its children.
    let parent = Vec3::new(10.0, -3.0, 2.0);
    let mut all = Vec::new();
    let mut parts = Vec::new();
    for o in 0..8u64 {
        let bit = |b: u64| if o >> b & 1 == 1 { 1.0 } else { -1.0 };
        let centre = parent + Vec3::new(bit(0), bit(1), bit(2));
        let some = if o == 5 { Vec::new() } else { cubes(12, centre, 2.0, 100 + o) };
        parts.push(Summary::of_cubes(centre, some.iter().copied()));
        all.extend(some);
    }
    let sum = Summary::sum(parent, &parts);
    let direct = Summary::of_cubes(parent, all.iter().copied());
    let raw = raw_error(&sum, &direct, 4.0);
    assert!(raw <= 1e-13, "raw {raw:e}");
    let truth = true_absolute(parent, &all);
    for l in 0..ABSOLUTE_ORDERS {
        assert!(sum.absolute[l] >= truth[l], "A_{l} {} < {}", sum.absolute[l], truth[l]);
    }
    assert!(sum.reach >= direct.reach && sum.reach <= 2.0 * 3f64.sqrt() + 1e-12);
    // The empty octant changes nothing.
    let without: Vec<Summary> = parts.iter().copied().filter(|p| !p.is_empty()).collect();
    assert_eq!(Summary::sum(parent, &without), sum);
    // Control: a part's third moment zeroed is seen.
    let mut broken = parts.clone();
    broken[2].third = [0.0; 10];
    assert!(raw_error(&Summary::sum(parent, &broken), &direct, 4.0) > 1e-6);
}

/// The least distance from `p` to the vertical segment from `base` up `height`.
fn to_segment(p: Vec3, base: Vec3, height: f64) -> f64 {
    let dz = (base.z - p.z).max(p.z - base.z - height).max(0.0);
    (p.x - base.x).hypot(p.y - base.y).hypot(dz)
}

#[test]
fn the_expansion_is_the_cubes_within_its_bound() {
    // Lopsided signed cubes in a node of side 2; pairs of points on vertical lines from 1.5 to 60
    // node sides away, in many directions, gaps of a tenth to twice the node.
    let (centre, size) = (Vec3::new(1.0, 2.0, -0.5), 2.0);
    let some = cubes(30, centre, size, 3);
    let summary = Summary::of_cubes(centre, some.iter().copied());
    let mut s = Scatter(99);
    let (mut checked, mut without_octupole_fails, mut worst_ratio) = (0, 0, 0.0f64);
    for _ in 0..400 {
        let distance = size * (1.5 + 58.5 * s.next().powi(2));
        let theta = std::f64::consts::PI * s.next();
        let phi = 2.0 * std::f64::consts::PI * s.next();
        let dir = Vec3::new(theta.sin() * phi.cos(), theta.sin() * phi.sin(), theta.cos());
        let gap = size * (0.1 + 1.9 * s.next());
        let lower = centre + dir * distance;
        let r = to_segment(centre, lower, gap);
        if summary.reach >= r {
            continue;
        }
        // The arms: lower at `below` over the centre, upper `gap` above it.
        let across = [lower.x - centre.x, lower.y - centre.y];
        let below = lower.z - centre.z;
        let above = below + gap;
        let expansion = summary.difference(across, below, above, -gap);
        let exact: f64 = some
            .iter()
            .map(|&(at, m, side)| {
                let a = [lower.x - at.x, lower.y - at.y];
                let b = lower.z - at.z;
                cube_difference(m, side, a, b, b + gap, -gap)
            })
            .sum();
        let error = (expansion - exact).abs();
        let bound = summary.bound(r, gap);
        assert!(error <= bound, "at {distance} m: error {error:e} over bound {bound:e}");
        assert!(bound < exact.abs() || distance < 4.0 * size, "a vacuous bound at {distance} m");
        worst_ratio = worst_ratio.max(error / bound);
        checked += 1;
        // Control: with the octupole left out the bound on the rest no longer holds.
        let short = summary.difference_to(2, across, below, above, -gap);
        if (short - exact).abs() > bound {
            without_octupole_fails += 1;
        }
    }
    assert!(checked > 300, "{checked}");
    assert!(without_octupole_fails > checked / 2, "{without_octupole_fails} of {checked}");
    assert!(worst_ratio > 1e-4, "the bound is never approached: {worst_ratio:e}");
}

#[test]
fn the_bound_is_infinite_within_the_reach() {
    let summary = Summary::of_cubes(Vec3::ZERO, [(Vec3::new(0.5, 0.0, 0.0), 1.0, 0.2)]);
    assert_eq!(summary.bound(summary.reach, 1.0), f64::INFINITY);
    assert_eq!(summary.bound(0.0, 1.0), f64::INFINITY);
    assert!(summary.bound(2.0 * summary.reach, 1.0).is_finite());
    assert_eq!(Summary::empty(Vec3::ZERO).bound(1.0, 1.0), 0.0);
}
