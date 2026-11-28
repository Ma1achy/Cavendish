//! The potential of a uniform cube, exact to rounding, and a voxel body as a source of the
//! forward model.
//!
//! **The cube series.** Outside a uniform cube of mass `M` and side `s`, at `r = R·n` from its
//! centre,
//!
//! ```text
//! V/G = −M [ 1/R + Σ_{l = 4, 6, 8, …} (s/2)^l Q_l(n) / R^{l+1} ]
//! ```
//!
//! where `Q_l(n) = ⟨r'^l P_l(n·r̂')⟩ / (s/2)^l` is the cube's average of the Legendre term:
//! a polynomial in `n` derived here from the cube's moments (`⟨x^e⟩ = (s/2)^e/(e + 1)` for even
//! `e`). Odd `l` and `l = 2` vanish by symmetry; `Q_4 = −(7/60)(5Σnᵢ⁴ − 3)`, which is
//! `reference/design/voxel.md` §5.2's term. Since `|P_l| ≤ 1` and `|r'| ≤ a = s√3/2`, the terms past
//! order `L` are bounded by `(M/R)·q^{L'}/(1 − q)`, `q = a/R`, `L'` the next order (6 after 4, and
//! so on): [`series_bound`]. That bound is what chooses the order.
//!
//! **The kernel** ([`cube_potential`], [`cube_difference`]) takes the lowest order whose bound is
//! under the rounding of the monopole, up to [`MAX_ORDER`]; closer than that it takes the
//! closed-form prism ([`prism_potential`], Nagy's formula), which holds inside and on the cube
//! too. Between two arm points the monopole is differenced without cancellation (as
//! `forward::Mass` does), so a far cube's tiny difference keeps its digits.
//!
//! **The closed form's difference** (TASK-S5-24). Near a cube the arms' difference is the
//! difference of two closed-form potentials, whose rounding (a few `ε` of the size of their terms)
//! does not shrink with the gap. A large block (an octree's merged interior) beside an arm, at a
//! moment when the arms are close together, loses digits that way. So [`cube_difference`] takes
//! the closed form only where that rounding, estimated from the terms themselves, is within
//! [`DIFFERENCE_EPSILON`] of the difference's scale; otherwise it splits the cube into its eight
//! halves (exact) and takes each the same way, so the parts near the arms get small and the rest
//! go to the series. Measured in `docs/slices/S5.md` §7.4.

use crate::Vec3;
use crate::forward::Source;

use super::tree::VoxTree;

/// The highest order of the series the kernel uses; closer cubes are taken by the prism.
pub const MAX_ORDER: usize = 16;
/// The bound under which the series is taken, relative to the monopole `M/R`: the rounding of one
/// `f64`.
pub const SERIES_EPSILON: f64 = f64::EPSILON / 2.0;
/// The most cube evaluations one read of a voxel body may make: leaves × flight nodes × 2 points ×
/// 2 interferometers, over every rule the audited read tries. Past it the read is refused before
/// it starts (see `cav_sim::read`).
pub const MAX_KERNEL_WORK: u64 = 2_000_000_000;

/// One monomial `coef · n_x^a n_y^b n_z^c` of a `Q_l`.
#[derive(Clone, Copy, Debug)]
struct Term {
    powers: [usize; 3],
    coef: f64,
}

fn factorial(n: usize) -> f64 {
    (1..=n).fold(1.0, |f, k| f * k as f64)
}

/// `⟨u^e⟩` for `u` uniform on `[−1, 1]`.
fn mean_power(e: usize) -> f64 {
    if e % 2 == 1 { 0.0 } else { 1.0 / (e as f64 + 1.0) }
}

/// The monomials of `Q_l`, collected by their powers of `n`.
fn q_terms(l: usize) -> Vec<Term> {
    let mut terms: Vec<Term> = Vec::new();
    for j in 0..=l / 2 {
        // P_l(x) = Σ_j c_{l,j} x^{l−2j}.
        let c = if j % 2 == 0 { 1.0 } else { -1.0 } * factorial(2 * l - 2 * j)
            / (2f64.powi(l as i32) * factorial(j) * factorial(l - j) * factorial(l - 2 * j));
        let m = l - 2 * j;
        // (n·r')^m |r'|^{2j}, averaged over the cube of half-side 1.
        for a in 0..=m {
            for b in 0..=m - a {
                let cc = m - a - b;
                let multinomial = factorial(m) / (factorial(a) * factorial(b) * factorial(cc));
                let mut average = 0.0;
                for p in 0..=j {
                    for q in 0..=j - p {
                        let t = j - p - q;
                        let w = factorial(j) / (factorial(p) * factorial(q) * factorial(t));
                        average += w * mean_power(a + 2 * p) * mean_power(b + 2 * q) * mean_power(cc + 2 * t);
                    }
                }
                let coef = c * multinomial * average;
                if coef == 0.0 {
                    continue;
                }
                match terms.iter_mut().find(|t| t.powers == [a, b, cc]) {
                    Some(term) => term.coef += coef,
                    None => terms.push(Term { powers: [a, b, cc], coef }),
                }
            }
        }
    }
    terms.retain(|t| t.coef.abs() > 1e-18);
    terms
}

/// `Q_l` for every even `l` from 4 to [`MAX_ORDER`], made once.
fn table() -> &'static [Vec<Term>] {
    static TABLE: std::sync::OnceLock<Vec<Vec<Term>>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        (0..=MAX_ORDER).map(|l| if l >= 4 && l % 2 == 0 { q_terms(l) } else { Vec::new() }).collect()
    })
}

/// `Q_l(n)` at a unit vector `n`: the cube's Legendre term of order `l`, over `(s/2)^l`.
#[must_use]
pub fn q(l: usize, n: Vec3) -> f64 {
    if l > MAX_ORDER {
        return f64::NAN;
    }
    let powers = |x: f64| {
        let mut p = [1.0; MAX_ORDER + 1];
        for k in 1..=MAX_ORDER {
            p[k] = p[k - 1] * x;
        }
        p
    };
    let (px, py, pz) = (powers(n.x), powers(n.y), powers(n.z));
    table()[l].iter().map(|t| t.coef * px[t.powers[0]] * py[t.powers[1]] * pz[t.powers[2]]).sum()
}

/// The order of the first term past order `order` (the series has no `l = 2`).
const fn next_order(order: usize) -> usize {
    if order < 4 { 4 } else { order + 2 }
}

/// The bound on what the series to order `order` leaves out, relative to the monopole `M/R`:
/// `q^{L'}/(1 − q)` with `q = (s√3/2)/R` and `L'` the next order. Infinite when `q ≥ 1` (the
/// series does not converge there).
#[must_use]
pub fn series_bound(order: usize, side: f64, distance: f64) -> f64 {
    let q = side * 3f64.sqrt() / 2.0 / distance;
    if q >= 1.0 { f64::INFINITY } else { q.powi(next_order(order) as i32) / (1.0 - q) }
}

/// The order the kernel takes for a cube of side `side` at `distance` (both metres): the lowest
/// whose bound is under [`SERIES_EPSILON`], or `None` when even [`MAX_ORDER`]'s is not.
#[must_use]
pub fn order_needed(side: f64, distance: f64) -> Option<usize> {
    [0, 4, 6, 8, 10, 12, 14, 16].into_iter().find(|&l| series_bound(l, side, distance) <= SERIES_EPSILON)
}

/// The series' correction to order `order`: `Σ_{l=4..order} (s/2)^l Q_l(n)/R^{l+1}`, metres⁻¹
/// (multiply by `−M` for `V/G`).
fn correction(order: usize, side: f64, d: Vec3, distance: f64) -> f64 {
    if order < 4 {
        return 0.0;
    }
    let n = d * (1.0 / distance);
    let ratio = 0.5 * side / distance;
    let mut sum = 0.0;
    let mut scale = ratio.powi(4) / distance;
    for l in (4..=order).step_by(2) {
        sum += scale * q(l, n);
        scale *= ratio * ratio;
    }
    sum
}

#[cfg(test)]
mod edge_tests {
    use super::*;

    #[test]
    fn a_point_beside_an_edge_past_its_end_is_finite() {
        // The point on the planes of two faces, below the cube (S4's town: a tower's base on a
        // cell's corner line): the corner terms' `z + r` rounds to zero there.
        let side = 0.4;
        let d = Vec3::new(0.2 + 1.8e-16, 0.2 + 1.8e-16, 1.0);
        let at = cube_potential(115.2, side, d);
        assert!(at.is_finite(), "{at}");
        // Continuous with points a hair away, on either side.
        for e in [1e-9, -1e-9] {
            let near = cube_potential(115.2, side, Vec3::new(0.2 + e, 0.2 + e, 1.0));
            assert!((near - at).abs() <= 1e-7 * at.abs(), "{near} {at}");
        }
        // And with the quadrature far enough out to trust it.
        let far = Vec3::new(0.2, 0.2, 3.0);
        let q = cube_by_quadrature(115.2, side, far, 40);
        assert!((cube_potential(115.2, side, far) - q).abs() <= 1e-10 * q.abs());
    }
}

/// The series to order `order` for a cube of `mass` kg and side `side` m at offset `d` (metres,
/// from its centre): `V/G`, kg/m. For tests; the kernel chooses its own order.
#[must_use]
pub fn cube_series(order: usize, mass: f64, side: f64, d: Vec3) -> f64 {
    let r = d.length();
    -mass * (1.0 / r + correction(order, side, d, r))
}

/// Nagy's term for one corner of a prism at offset `(x, y, z)` from the point, m², and the size
/// its rounding scales with, m²: the sum of its six terms' magnitudes and `r²` (what rounding the
/// logarithm's argument `u + r` can cost where it nearly cancels, at most about `2εr²`).
fn corner(x: f64, y: f64, z: f64) -> (f64, f64) {
    let r = (x * x + y * y + z * z).sqrt();
    // A term whose coefficient is zero is zero, whatever its logarithm or angle would be.
    // `u + r` cancels when `u` is negative and nearly `−r` (the point beside an edge, past its
    // end: S4 found NaN there, the sum rounding to zero or below). Then it is formed as
    // `(r² − u²)/(r − u)`, the other two squares over a sum, which never cancels. Elsewhere the
    // plain sum, so that every other value is the same bits as before.
    let log = |c: f64, u: f64, others: f64| {
        if c == 0.0 {
            return 0.0;
        }
        let sum = u + r;
        let sum = if u < 0.0 && sum < 1e-6 * r { others / (r - u) } else { sum };
        c * sum.ln()
    };
    let angle = |p: f64, q: f64, w: f64| if p == 0.0 { 0.0 } else { 0.5 * p * p * (q * w / (p * r)).atan() };
    let (xx, yy, zz) = (x * x, y * y, z * z);
    let terms = [
        log(x * y, z, xx + yy),
        log(y * z, x, yy + zz),
        log(z * x, y, zz + xx),
        angle(x, y, z),
        angle(y, z, x),
        angle(z, x, y),
    ];
    // The same sum, in the same order, as before the magnitude was kept: the same bits.
    let value = terms[0] + terms[1] + terms[2] - terms[3] - terms[4] - terms[5];
    (value, terms.iter().map(|t| t.abs()).sum::<f64>() + xx + yy + zz)
}

/// [`prism_potential`] and the size its rounding scales with, kg/m: `density` times the sum over
/// the corners of each corner term's magnitude (see `corner`). The value's rounding is a few
/// `f64::EPSILON` of that size; the value itself may be far smaller, which is how the closed form
/// loses digits far away.
fn prism_terms(density: f64, lo: Vec3, hi: Vec3) -> (f64, f64) {
    let (mut total, mut size) = (0.0, 0.0);
    for (i, x) in [lo.x, hi.x].into_iter().enumerate() {
        for (j, y) in [lo.y, hi.y].into_iter().enumerate() {
            for (k, z) in [lo.z, hi.z].into_iter().enumerate() {
                let sign = if (i + j + k) % 2 == 0 { -1.0 } else { 1.0 };
                let (value, magnitude) = corner(x, y, z);
                total += sign * value;
                size += magnitude;
            }
        }
    }
    (-density * total, density.abs() * size)
}

/// The potential of a uniform rectangular prism of `density` kg/m³ spanning `lo` to `hi` (its
/// corners as offsets from the point asked about, metres), over G: kg/m. Exact in closed form,
/// finite inside and on the prism. Far away its terms cancel: the kernel uses it only near.
#[must_use]
pub fn prism_potential(density: f64, lo: Vec3, hi: Vec3) -> f64 {
    prism_terms(density, lo, hi).0
}

/// Closer than this many sides the prism is taken; between here and where the series converges
/// to rounding, the cube is split into its eight halves (exact: eight uniform cubes of the same
/// density are the cube), each taken the same way. The prism's own rounding grows with distance
/// (its terms cancel): measured 2.8e-14 of |V| at three sides, 4.6e-13 at 6.7
/// (`kernel_error_by_distance`), so it is kept close.
pub const PRISM_WITHIN: f64 = 3.0;

/// The eight half-cubes' centres relative to the cube's, for a cube of side `side`.
fn octants(side: f64) -> [Vec3; 8] {
    let q = 0.25 * side;
    std::array::from_fn(|i| {
        Vec3::new(
            if i & 1 == 0 { -q } else { q },
            if i & 2 == 0 { -q } else { q },
            if i & 4 == 0 { -q } else { q },
        )
    })
}

/// The potential over G (kg/m) of a uniform cube of `mass` kg and side `side` m at the point
/// `d` metres from its centre: the series to the order its bound asks; or, nearer, the cube's
/// eight halves the same way; or, nearer still (within [`PRISM_WITHIN`] sides), the prism.
#[must_use]
pub fn cube_potential(mass: f64, side: f64, d: Vec3) -> f64 {
    potential_and_size(mass, side, d).0
}

/// [`cube_potential`] (the same bits) and the size its rounding scales with, kg/m: `|M|/r` where
/// the series is taken (its monopole), the prism's terms where the prism is (`prism_terms`), the
/// sum of the halves' where the cube is split.
fn potential_and_size(mass: f64, side: f64, d: Vec3) -> (f64, f64) {
    let r = d.length();
    match order_needed(side, r) {
        Some(order) => (-mass * (1.0 / r + correction(order, side, d, r)), mass.abs() / r),
        None if r >= PRISM_WITHIN * side => {
            let (mut value, mut size) = (0.0, 0.0);
            for c in octants(side) {
                let (v, s) = potential_and_size(0.125 * mass, 0.5 * side, d - c);
                value += v;
                size += s;
            }
            (value, size)
        }
        None => {
            let h = 0.5 * side;
            let half = Vec3::new(h, h, h);
            prism_terms(mass / side.powi(3), -d - half, -d + half)
        }
    }
}

/// How much of the difference's scale the closed form's rounding may cost before the cube is
/// split instead: the closed form `V(upper) − V(lower)` of [`cube_difference`] is taken only where
/// its estimated rounding is at most this fraction of the scale `S = |M|·min(|gap|/ρ², 1/ρ)`,
/// `ρ = max(r, side)`, `r` the nearer point's distance from the cube's centre (`S` is the size of
/// the difference when nothing cancels by symmetry). TASK-S5-24.
pub const DIFFERENCE_EPSILON: f64 = 1e-12;

/// The closed form's estimated rounding over `f64::EPSILON` times the size of its terms
/// (`prism_terms`, at both points): the ratio measured, with room (`closed_form_rounding` in the
/// tests; docs/slices/S5.md §7.4).
pub const CLOSED_FORM_ROUNDING: f64 = 1.0;

/// The most times [`cube_difference`] halves a cube to keep the closed form's difference within
/// [`DIFFERENCE_EPSILON`]: past it (a gap of nanometres beside a large block) the closed form is
/// taken as it is. Splitting stops long before for any gap the flight has (§7.4).
pub const MAX_SPLIT_DEPTH: u32 = 16;

/// `(V(upper) − V(lower))/G` (kg/m) for a uniform cube of `mass` kg and side `side` m, the two
/// points on a vertical line `across` (horizontal offset from the cube's centre, metres) at
/// heights `below` and `above` metres over the centre. `gap` is `below − above` as the caller
/// knows it exactly (the arms' rise, lower minus upper), so the monopole's difference is formed
/// without cancellation.
///
/// Where the series converges to rounding at the nearer point it is taken, differenced without
/// cancellation. Nearer, the cube is split into its eight halves (exact) until each half's series
/// converges, or, within [`PRISM_WITHIN`] sides, until the difference of the two closed-form
/// potentials keeps its digits: its rounding, estimated from the size of its terms, within
/// [`DIFFERENCE_EPSILON`] of the difference's scale (at most [`MAX_SPLIT_DEPTH`] halvings).
#[must_use]
pub fn cube_difference(mass: f64, side: f64, across: [f64; 2], below: f64, above: f64, gap: f64) -> f64 {
    difference_within(mass, side, across, below, above, gap, DIFFERENCE_EPSILON, 0)
}

/// [`cube_difference`] with the closed form's allowance `epsilon` (a fraction of the scale;
/// infinite for S3's fixed switch, the closed form for every cube within [`PRISM_WITHIN`] sides),
/// `depth` the halvings so far.
#[allow(clippy::too_many_arguments)]
fn difference_within(
    mass: f64,
    side: f64,
    across: [f64; 2],
    below: f64,
    above: f64,
    gap: f64,
    epsilon: f64,
    depth: u32,
) -> f64 {
    let horizontal = across[0] * across[0] + across[1] * across[1];
    let (b, a) = ((horizontal + below * below).sqrt(), (horizontal + above * above).sqrt());
    let nearer = a.min(b);
    let halves = || -> f64 {
        octants(side)
            .iter()
            .map(|c| {
                let across = [across[0] - c.x, across[1] - c.y];
                difference_within(
                    0.125 * mass,
                    0.5 * side,
                    across,
                    below - c.z,
                    above - c.z,
                    gap,
                    epsilon,
                    depth + 1,
                )
            })
            .sum()
    };
    match order_needed(side, nearer) {
        Some(order) => {
            // 1/a − 1/b = (b² − a²)/(ab(a + b)), b² − a² = (below − above)(below + above).
            let monopole = -mass * gap * (below + above) / (a * b * (a + b));
            let du = Vec3::new(across[0], across[1], above);
            let dl = Vec3::new(across[0], across[1], below);
            monopole - mass * (correction(order, side, du, a) - correction(order, side, dl, b))
        }
        None if nearer >= PRISM_WITHIN * side => halves(),
        None => {
            let (upper, upper_size) = potential_and_size(mass, side, Vec3::new(across[0], across[1], above));
            let (lower, lower_size) = potential_and_size(mass, side, Vec3::new(across[0], across[1], below));
            let rho = nearer.max(side);
            let scale = mass.abs() * (gap.abs() / (rho * rho)).min(1.0 / rho);
            let rounding = CLOSED_FORM_ROUNDING * f64::EPSILON * (upper_size + lower_size);
            if epsilon.is_infinite() || gap == 0.0 || depth >= MAX_SPLIT_DEPTH || rounding <= epsilon * scale
            {
                upper - lower
            } else {
                halves()
            }
        }
    }
}

/// `(V(d − shift) − V(d))/G` (kg/m): how much the potential at offset `d` (metres, from the
/// cube's centre) changes when a uniform cube of `mass` kg and side `side` m moves by `shift`
/// metres. Formed without the cancellation of two nearly equal potentials, as the arms' difference
/// is: for a cube moved by micrometres the change is a millionth of the potential, and a body of
/// many leaves whose first-order changes cancel (a scaffold vibrating about its axis) keeps its
/// digits only this way.
#[must_use]
pub fn cube_shift(mass: f64, side: f64, d: Vec3, shift: Vec3) -> f64 {
    let moved = d - shift;
    let (r, s) = (d.length(), moved.length());
    match (order_needed(side, r), order_needed(side, s)) {
        (Some(a), Some(b)) => {
            let order = a.max(b);
            // 1/s − 1/r = (r² − s²)/(rs(r + s)), r² − s² = shift·(2d − shift).
            let monopole = -mass * shift.dot(d * 2.0 - shift) / (r * s * (r + s));
            monopole - mass * (correction(order, side, moved, s) - correction(order, side, d, r))
        }
        _ => cube_potential(mass, side, moved) - cube_potential(mass, side, d),
    }
}

/// [`cube_shift`] for each of `shifts` at one offset `d`: the same bits as calling it once a
/// shift, with the part that depends on `d` alone (its distance, its order, its series correction
/// or its closed-form potential) worked out once rather than once a shift. Units and frames as
/// [`cube_shift`]. For a table of one point's change at several displacements (the oracle's
/// scaffold, five a point); it is not faster for one shift.
#[must_use]
pub fn cube_shifts<const N: usize>(mass: f64, side: f64, d: Vec3, shifts: &[Vec3; N]) -> [f64; N] {
    let r = d.length();
    let at_d = order_needed(side, r);
    // The correction at `d` for the last order asked, and the potential at `d`, once each.
    let mut series: Option<(usize, f64)> = None;
    let mut whole: Option<f64> = None;
    shifts.map(|shift| {
        let moved = d - shift;
        let s = moved.length();
        match (at_d, order_needed(side, s)) {
            (Some(a), Some(b)) => {
                let order = a.max(b);
                let monopole = -mass * shift.dot(d * 2.0 - shift) / (r * s * (r + s));
                let here = match series {
                    Some((o, c)) if o == order => c,
                    _ => {
                        let c = correction(order, side, d, r);
                        series = Some((order, c));
                        c
                    }
                };
                monopole - mass * (correction(order, side, moved, s) - here)
            }
            _ => {
                cube_potential(mass, side, moved)
                    - *whole.get_or_insert_with(|| cube_potential(mass, side, d))
            }
        }
    })
}

/// `(V(upper) − V(lower))/G` (kg/m) for a point of `mass` kg with the second central moments
/// `second` (kg m², xx, yy, zz, xy, xz, yz) about it, the two points as [`cube_difference`] takes
/// them (`across`, `below`, `above` from the point, metres; `gap` = `below − above`). To the
/// quadrupole: `V/G = −M/R − (3 d·S·d − R² tr S)/(2R⁵)`; the monopole's difference is formed
/// without cancellation, the quadrupole's (smaller by `(s/R)²`) directly.
#[must_use]
pub fn quadrupole_difference(
    mass: f64,
    second: &[f64; 6],
    across: [f64; 2],
    below: f64,
    above: f64,
    gap: f64,
) -> f64 {
    if gap == 0.0 {
        return 0.0;
    }
    let horizontal = across[0] * across[0] + across[1] * across[1];
    let (b2, a2) = (horizontal + below * below, horizontal + above * above);
    let (b, a) = (b2.sqrt(), a2.sqrt());
    let monopole = -mass * gap * (below + above) / (a * b * (a + b));
    let trace = second[0] + second[1] + second[2];
    let quadrupole = |d: [f64; 3], r2: f64| {
        let s = &second;
        let dsd = s[0] * d[0] * d[0]
            + s[1] * d[1] * d[1]
            + s[2] * d[2] * d[2]
            + 2.0 * (s[3] * d[0] * d[1] + s[4] * d[0] * d[2] + s[5] * d[1] * d[2]);
        (3.0 * dsd - r2 * trace) / (2.0 * r2 * r2 * r2.sqrt())
    };
    let upper = quadrupole([across[0], across[1], above], a2);
    let lower = quadrupole([across[0], across[1], below], b2);
    monopole - (upper - lower)
}

/// `(V_3(upper) − V_3(lower))/G` (kg/m): the octupole term of a mass distribution with the third
/// central moments `third` (kg m³, in `Moments::third`'s order) about its centre of mass, at the
/// two points as [`cube_difference`] takes them:
/// `V_3/G = −(5 Σ T_abc d_a d_b d_c − 3R² Σ T_aac d_c)/(2R⁷)`. Formed directly: it is smaller than
/// the monopole by `(s/R)³`.
#[must_use]
pub fn octupole_difference(third: &[f64; 10], across: [f64; 2], below: f64, above: f64) -> f64 {
    use super::tree::third_index;
    let term = |d: [f64; 3]| {
        let r2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        let (mut cubic, mut trace) = (0.0, 0.0);
        for a in 0..3 {
            for b in 0..3 {
                for c in 0..3 {
                    cubic += third[third_index(a, b, c)] * d[a] * d[b] * d[c];
                }
            }
            for c in 0..3 {
                trace += third[third_index(a, a, c)] * d[c];
            }
        }
        -(5.0 * cubic - 3.0 * r2 * trace) / (2.0 * r2.powi(3) * r2.sqrt())
    };
    term([across[0], across[1], above]) - term([across[0], across[1], below])
}

/// A cut cell read as a point with its piece's quadrupole ([`super::cut::Representation::Second`]).
#[derive(Clone, Copy, Debug)]
struct Quad {
    /// The piece's centroid, metres, the body's frame.
    centre: Vec3,
    /// Kilograms.
    mass: f64,
    /// Its second central moments, kg m².
    second: [f64; 6],
    /// Its third central moments, kg m³ (read only by `Pieces` with its octupole, a measurement).
    third: [f64; 10],
    /// The cell's side, metres: within [`PRISM_WITHIN`] of it the piece is read as a cube there.
    side: f64,
}

/// A voxel body as the forward model sees it: every leaf of a tree a uniform cube, the tree's
/// frame carried to where the body is.
pub struct TreeBody<P: Fn(f64) -> Vec3 + Sync> {
    /// The leaves as `(centre in the body's frame, metres; mass, kg; side, metres)`, in Morton
    /// order: the order they are summed in.
    cubes: Vec<(Vec3, f64, f64)>,
    /// The cut cells read as points with quadrupoles (`Second`), after the cubes; empty
    /// otherwise.
    quads: Vec<Quad>,
    /// The cut cells read as their exact pieces near an arm and as `quads` are beyond (`Pieces`),
    /// at the tree's density, after the quads; empty otherwise.
    solids: Vec<(super::cut::PieceSolid, Quad)>,
    /// Kilograms per cubic metre of the solids.
    density: f64,
    /// Within this many cells of an arm point a solid is read exactly.
    within: f64,
    /// Whether a solid beyond `within` adds its piece's octupole to `Second`'s read.
    octupole: bool,
    /// Where the body's frame is at a time: seconds on the scene's clock in, metres in the world's
    /// frame out. The body does not turn.
    pub position: P,
    /// `(from, until)`, seconds on the scene's clock: it exists from the first up to the second.
    pub exists: (f64, f64),
    /// Times at which its motion is not smooth, seconds on the scene's clock.
    pub breaks: Vec<f64>,
}

impl<P: Fn(f64) -> Vec3 + Sync> TreeBody<P> {
    /// The body of `tree` (its leaves are copied: the tree need not outlive it), at `position`,
    /// always there.
    pub fn new(tree: &VoxTree, position: P) -> Self {
        let cubes = tree
            .leaves
            .iter()
            .map(|leaf| {
                (tree.block_centre(leaf.key, leaf.level), tree.leaf_mass(leaf), tree.side(leaf.level))
            })
            .collect();
        Self {
            cubes,
            quads: Vec::new(),
            solids: Vec::new(),
            density: 0.0,
            within: 0.0,
            octupole: false,
            position,
            exists: (f64::NEG_INFINITY, f64::INFINITY),
            breaks: Vec::new(),
        }
    }

    /// The body of `tree` read as `representation` says for its cut cells
    /// ([`super::cut::Representation`]); its other leaves are cubes, as [`Self::new`]. For a tree
    /// with no cuts, or `Sampled` or `Fill`, it is [`Self::new`] exactly (a cut leaf already
    /// weighs its piece and sits at its cell's centre). `Centroid` moves each cut cell's cube to
    /// its piece's centroid; `Second` reads it as a point with the piece's quadrupole there, or the
    /// `Centroid` cube within [`PRISM_WITHIN`] cells of the point asked about.
    /// `Pieces` needs its solids ([`Self::with_pieces`]); given only the tree it reads as `Second`.
    pub fn read_as(tree: &VoxTree, representation: super::cut::Representation, position: P) -> Self {
        use super::cut::Representation;
        let mut body = Self::new(tree, position);
        if tree.cuts.is_empty() || matches!(representation, Representation::Sampled | Representation::Fill) {
            return body;
        }
        let mut cubes = Vec::with_capacity(body.cubes.len());
        for (leaf, cube) in tree.leaves.iter().zip(&body.cubes) {
            let Some(cut) = tree.cut_of(leaf) else {
                cubes.push(*cube);
                continue;
            };
            let mass = tree.leaf_mass(leaf);
            let side = tree.side(0);
            match representation {
                Representation::Second | Representation::Pieces => body.quads.push(Quad {
                    centre: cut.piece.centroid,
                    mass,
                    second: cut.piece.second.map(|s| tree.density * s),
                    third: cut.piece.third.map(|t| tree.density * t),
                    side,
                }),
                _ => cubes.push((cut.piece.centroid, mass, side)),
            }
        }
        body.cubes = cubes;
        body
    }

    /// The body of `tree` read as [`super::cut::Representation::Pieces`]: each cut cell within
    /// [`super::cut::PIECE_WITHIN`] cells of the point asked about (from the cell's centre) as its
    /// exact piece, `solids` (from [`super::cut::piece_solids`], in the order of the tree's
    /// cuts), by the polyhedral closed form; beyond, as `Second` with the piece's octupole added
    /// (its third moments are kept anyway: 4 to 13 times nearer, `docs/slices/S5.md` §7.7). A tree with no cuts reads as
    /// [`Self::new`].
    ///
    /// # Panics
    /// When `solids` are not one for each cut cell of `tree`.
    pub fn with_pieces(tree: &VoxTree, solids: Vec<super::cut::PieceSolid>, position: P) -> Self {
        Self::with_pieces_within(tree, solids, super::cut::PIECE_WITHIN, true, position)
    }

    /// [`Self::with_pieces`] with the exact pieces read within `cells` cells of an arm point in
    /// place of [`super::cut::PIECE_WITHIN`], and beyond them, with `octupole`, `Second` and the
    /// piece's octupole: for measuring how far they must reach (infinite: every cut cell exact)
    /// and what the far read costs the near one.
    ///
    /// # Panics
    /// As [`Self::with_pieces`].
    pub fn with_pieces_within(
        tree: &VoxTree,
        solids: Vec<super::cut::PieceSolid>,
        cells: f64,
        octupole: bool,
        position: P,
    ) -> Self {
        let mut body = Self::read_as(tree, super::cut::Representation::Pieces, position);
        body.within = cells;
        body.octupole = octupole;
        assert_eq!(solids.len(), body.quads.len(), "one solid a cut cell");
        body.density = tree.density;
        body.solids = solids.into_iter().zip(std::mem::take(&mut body.quads)).collect();
        body
    }

    /// One leaf of `tree` alone, at `position`, always there: what that leaf adds.
    pub fn single(tree: &VoxTree, leaf: &super::Leaf, position: P) -> Self {
        let cube = (tree.block_centre(leaf.key, leaf.level), tree.leaf_mass(leaf), tree.side(leaf.level));
        Self {
            cubes: vec![cube],
            quads: Vec::new(),
            solids: Vec::new(),
            density: 0.0,
            within: 0.0,
            octupole: false,
            position,
            exists: (f64::NEG_INFINITY, f64::INFINITY),
            breaks: Vec::new(),
        }
    }

    /// How many cubes and points are summed at each point asked about.
    #[must_use]
    pub fn cubes(&self) -> usize {
        self.cubes.len() + self.quads.len() + self.solids.len()
    }
}

impl<P: Fn(f64) -> Vec3 + Sync> Source for TreeBody<P> {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, time: f64) -> [f64; 2] {
        if !(time >= self.exists.0 && time < self.exists.1) {
            return [0.0; 2];
        }
        // The local frame first: the body's frame relative to the tower's base, then each leaf.
        let offset = (self.position)(time) - tower;
        heights.map(|height| {
            let mut sum = 0.0;
            for &(centre, mass, side) in &self.cubes {
                let at = offset + centre;
                let (below, above) = (height + lower - at.z, height + upper - at.z);
                sum += cube_difference(mass, side, [-at.x, -at.y], below, above, lower - upper);
            }
            for q in &self.quads {
                let at = offset + q.centre;
                let (below, above) = (height + lower - at.z, height + upper - at.z);
                let across = [-at.x, -at.y];
                let horizontal = across[0] * across[0] + across[1] * across[1];
                let nearer = (horizontal + below * below).min(horizontal + above * above).sqrt();
                sum += if nearer < PRISM_WITHIN * q.side {
                    cube_difference(q.mass, q.side, across, below, above, lower - upper)
                } else {
                    quadrupole_difference(q.mass, &q.second, across, below, above, lower - upper)
                };
            }
            for (solid, q) in &self.solids {
                let at = offset + solid.centre;
                let (below, above) = (height + lower - at.z, height + upper - at.z);
                let across = [-at.x, -at.y];
                let horizontal = across[0] * across[0] + across[1] * across[1];
                let nearer = (horizontal + below * below).min(horizontal + above * above).sqrt();
                if nearer < self.within * q.side {
                    for (sign, part) in &solid.parts {
                        sum += part.difference(sign * self.density, across, below, above, lower - upper);
                    }
                } else {
                    let at = offset + q.centre;
                    let (below, above) = (height + lower - at.z, height + upper - at.z);
                    sum +=
                        quadrupole_difference(q.mass, &q.second, [-at.x, -at.y], below, above, lower - upper);
                    if self.octupole {
                        sum += octupole_difference(&q.third, [-at.x, -at.y], below, above);
                    }
                }
            }
            sum
        })
    }

    fn breaks(&self, from: f64, to: f64, out: &mut Vec<f64>) {
        let ends = [self.exists.0, self.exists.1];
        out.extend(self.breaks.iter().chain(&ends).copied().filter(|t| *t > from && *t < to));
    }
}

/// The potential over G of a uniform cube by a Gauss–Legendre product rule of `points` per axis,
/// summed with compensation: the reference the kernel is tested against (good where the point is
/// well outside the cube).
#[must_use]
pub fn cube_by_quadrature(mass: f64, side: f64, d: Vec3, points: usize) -> f64 {
    let rule = crate::forward::gauss_legendre(points);
    let h = 0.5 * side;
    let (mut sum, mut carry) = (0.0f64, 0.0f64);
    for &(x, wx) in &rule {
        for &(y, wy) in &rule {
            for &(z, wz) in &rule {
                let p = Vec3::new(h * x, h * y, h * z);
                let term = wx * wy * wz / (d - p).length();
                // Neumaier's sum.
                let t = sum + term;
                carry += if sum.abs() >= term.abs() { (sum - t) + term } else { (term - t) + sum };
                sum = t;
            }
        }
    }
    -mass * (sum + carry) / 8.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 64 directions spread over the sphere, with the axes and diagonals among them.
    fn directions() -> Vec<Vec3> {
        let mut out = vec![
            Vec3::X,
            Vec3::Y,
            Vec3::Z,
            Vec3::new(1.0, 1.0, 1.0).normalised(),
            Vec3::new(1.0, 1.0, 0.0).normalised(),
        ];
        let golden = std::f64::consts::PI * (3.0 - 5f64.sqrt());
        for k in 0..59 {
            let z = 1.0 - 2.0 * (k as f64 + 0.5) / 59.0;
            let r = (1.0 - z * z).sqrt();
            out.push(Vec3::new(r * (golden * k as f64).cos(), r * (golden * k as f64).sin(), z));
        }
        out
    }

    #[test]
    fn the_fourth_order_term_is_the_design_s() {
        for n in directions() {
            let sum4 = n.x.powi(4) + n.y.powi(4) + n.z.powi(4);
            let want = -(7.0 / 60.0) * (5.0 * sum4 - 3.0);
            assert!((q(4, n) - want).abs() < 1e-14, "{n:?}: {} against {want}", q(4, n));
        }
        // The terms a cube does not have: l = 2 (and odd l) are not in the table at all.
        assert!(table()[2].is_empty() && table()[5].is_empty());
        // Control: Q_6 is not Q_4.
        assert!((q(6, Vec3::Z) - q(4, Vec3::Z)).abs() > 1e-3);
    }

    /// The worst error over the sweep of the series to `order` against the quadrature, over its
    /// bound (≤ 1 means within it).
    fn series_against_bound(order: usize) -> f64 {
        let mut worst = 0f64;
        for ratio in [0.02, 0.05, 0.1, 0.2, 0.3] {
            for n in directions() {
                let d = n * (1.0 / ratio);
                let exact = cube_by_quadrature(1.0, 1.0, d, 24);
                let err = (cube_series(order, 1.0, 1.0, d) - exact).abs() / exact.abs();
                // A floor of the reference's own rounding.
                let bound = series_bound(order, 1.0, d.length()) + 1e-15;
                worst = worst.max(err / bound);
            }
        }
        worst
    }

    cav_validate::case! {
        test: validation_cube_series_is_within_its_bound,
        name: "voxel.cube_series_within_bound",
        subsystem: "voxel",
        slice: "S3",
        reference: ClosedForm,
        against: "Gauss–Legendre 24³ quadrature of the cube; s/R from 0.02 to 0.3, 64 directions; orders 0, 4, 8, 12: the worst error as a fraction of the series' own bound",
        unit: Relative,
        tolerance: 1.0,
        measure: cav_validate::Measured::Error([0, 4, 8, 12].into_iter().map(series_against_bound).fold(0.0, f64::max)),
    }

    /// The worst error of the kernel relative to |V|, against the quadrature from s/R = 1e-4 to
    /// 0.8 (where 24 points still converge), on and off the axes.
    fn kernel_error() -> f64 {
        let mut worst = 0f64;
        for ratio in [1e-4, 1e-3, 0.01, 0.05, 0.1, 0.15, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8] {
            for n in directions() {
                let d = n * (1.0 / ratio);
                let exact = cube_by_quadrature(2.5, 1.0, d, if ratio > 0.5 { 48 } else { 24 });
                worst = worst.max((cube_potential(2.5, 1.0, d) - exact).abs() / exact.abs());
            }
        }
        worst
    }

    cav_validate::case! {
        test: validation_the_cube_kernel_is_exact,
        name: "voxel.cube_kernel_exact",
        subsystem: "voxel",
        slice: "S3",
        reference: ClosedForm,
        against: "Gauss–Legendre quadrature of the cube (24³, 48³ close) from s/R = 1e-4 to 0.8 in 64 directions: series or prism, as the kernel chooses",
        unit: Relative,
        tolerance: 1e-13,
        measure: cav_validate::Measured::Error(kernel_error()),
    }

    /// `(V_cube − V_point)/V_point` on the axis at `s/R = ratio`, by the kernel.
    fn deviation(ratio: f64) -> f64 {
        let d = Vec3::Z * (1.0 / ratio);
        let point = -1.0 / d.length();
        (cube_potential(1.0, 1.0, d) - point) / point
    }

    /// The log–log slope and the coefficient of the deviation over s/R from 0.01 to 0.1.
    fn approach_to_the_point() -> (f64, f64) {
        let (a, b) = (0.01, 0.1);
        let slope = (deviation(b) / deviation(a)).ln() / (b / a).ln();
        (slope, deviation(a) / a.powi(4))
    }

    cav_validate::case! {
        test: validation_the_cube_converges_to_the_point_mass,
        name: "voxel.cube_to_point_order",
        subsystem: "voxel",
        slice: "S3",
        reference: ClosedForm,
        against: "|V_cube − V_point|/|V_point| on the axis at s/R from 0.1 to 0.01: order 4 ± 0.02, and at 0.01 within 1% above the leading term 7/480·(s/R)⁴",
        unit: Relative,
        tolerance: 1.473e-10,
        order: (4.0, 0.02),
        measure: cav_validate::Measured::Convergence(
            [0.1, 0.05, 0.02, 0.01].into_iter().map(|r| (r, deviation(r).abs())).collect()
        ),
    }

    #[test]
    fn the_cube_approaches_the_point_as_the_fourth_power() {
        let (slope, coefficient) = approach_to_the_point();
        assert!((slope - 4.0).abs() <= 0.02, "{slope}");
        // On the axis the leading coefficient is 7/480 (−Q_4(ẑ)/16 = (7/60)·2/16), and the
        // cube is deeper than the point there.
        assert!((coefficient / (-7.0 / 480.0) - 1.0).abs() <= 0.01, "{coefficient}");
        // Control: a monopole-only "cube" has no fourth-order term at all.
        let d = Vec3::Z * 10.0;
        assert_eq!(cube_series(0, 1.0, 1.0, d), -1.0 / 10.0);
    }

    #[test]
    #[ignore = "a diagnostic: the kernel's error by distance"]
    fn kernel_error_by_distance() {
        for ratio in [1e-3, 0.05, 0.1, 0.12, 0.15, 0.17, 0.2, 0.25, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8] {
            let (mut worst, mut reference) = (0f64, 0f64);
            for n in directions() {
                let d = n * (1.0 / ratio);
                let exact = cube_by_quadrature(2.5, 1.0, d, 48);
                let other = cube_by_quadrature(2.5, 1.0, d, 64);
                worst = worst.max((cube_potential(2.5, 1.0, d) - exact).abs() / exact.abs());
                reference = reference.max((other - exact).abs() / exact.abs());
            }
            println!(
                "{ratio}: {worst:.3e} reference {reference:.1e} order {:?}",
                order_needed(1.0, 1.0 / ratio)
            );
        }
    }

    #[test]
    fn a_shifted_cube_keeps_its_digits() {
        let (mass, side) = (3.0, 0.05);
        let mut plain_worst = 0f64;
        for n in directions().into_iter().take(16) {
            let d = n * 0.6;
            // Where the plain difference is still good (a millimetre: 1.7e-3 of the potential),
            // the two agree.
            let shift = Vec3::new(1e-3, -4e-4, 2e-4);
            let plain = cube_potential(mass, side, d - shift) - cube_potential(mass, side, d);
            let kept = cube_shift(mass, side, d, shift);
            assert!((plain - kept).abs() <= 1e-11 * kept.abs(), "{n:?}: {plain} {kept}");
            // Antisymmetry: moving back undoes it.
            assert!((cube_shift(mass, side, d - shift, -shift) + kept).abs() <= 1e-14 * kept.abs());
            // A nanometre: linear in the shift to its own second order (|shift|/R ≈ 2e-9, and a
            // factor for the field's curvature), and to rounding: within 1e-7.
            let tiny = shift * 1e-6;
            let (one, two) = (cube_shift(mass, side, d, tiny), cube_shift(mass, side, d, tiny * 2.0));
            assert!((two - 2.0 * one).abs() <= 1e-7 * one.abs(), "{n:?}: {one} {two}");
            // Control: the plain difference of a nanometre has lost its digits (rounding of a
            // potential 2e9 times the change).
            let plain = cube_potential(mass, side, d - tiny) - cube_potential(mass, side, d);
            plain_worst = plain_worst.max((two - 2.0 * plain).abs() / one.abs());
        }
        assert!(plain_worst > 1e-7, "the plain difference kept its digits everywhere: {plain_worst}");
    }

    #[test]
    fn shifts_at_one_point_are_the_single_shift_s_bits() {
        let (mass, side) = (3.0, 0.05);
        // Shifts large and small along and across: a large one first, so that the correction at
        // `d` is first worked out for a higher order than the small ones that follow need (a
        // stale one is then wrong in the small shifts' digits), and large ones that take the
        // moved point into the closed form part way through.
        let shifts = [
            Vec3::new(0.25, 0.0, 0.1),
            Vec3::new(2e-6, 0.0, 0.0),
            Vec3::new(-1e-3, 4e-4, 0.0),
            Vec3::new(0.0, -0.45, 0.0),
            Vec3::new(3e-6, -1e-6, 2e-6),
        ];
        let (mut closed, mut reused, mut changed) = (0, 0, 0);
        for n in directions().into_iter().take(16) {
            for distance in [0.03, 0.12, 0.3, 0.6, 2.0] {
                let d = n * distance;
                let together = cube_shifts(mass, side, d, &shifts);
                let mut last = None;
                for (shift, got) in shifts.iter().zip(together) {
                    let one = cube_shift(mass, side, d, *shift);
                    assert_eq!(got.to_bits(), one.to_bits(), "{d:?} {shift:?}: {got} {one}");
                    // Which way it went, so that every way is seen.
                    match (order_needed(side, d.length()), order_needed(side, (d - *shift).length())) {
                        (Some(a), Some(b)) => {
                            let order = a.max(b);
                            if last == Some(order) {
                                reused += 1
                            } else if last.is_some() {
                                changed += 1
                            }
                            last = Some(order);
                        }
                        _ => closed += 1,
                    }
                }
            }
        }
        // Control: the cases reach the closed form, reuse the correction at `d`, and change order
        // after one was kept (where a stale correction would be wrong).
        assert!(closed > 0 && reused > 0 && changed > 0, "{closed} {reused} {changed}");
    }

    /// Neumaier's compensated sum.
    #[derive(Default)]
    struct Compensated {
        sum: f64,
        carry: f64,
    }

    impl Compensated {
        fn add(&mut self, term: f64) {
            let t = self.sum + term;
            self.carry +=
                if self.sum.abs() >= term.abs() { (self.sum - t) + term } else { (term - t) + self.sum };
            self.sum = t;
        }
        fn value(&self) -> f64 {
            self.sum + self.carry
        }
    }

    /// The reference difference for K1 (S5 §7.4): the cube split exactly into octants,
    /// recursively, until every part's series converges to rounding at the nearer point, each
    /// part's difference then the series' (no cancellation in the monopole), summed with
    /// compensation. No closed form anywhere, so the points must be outside the cube.
    fn series_reference(mass: f64, side: f64, across: [f64; 2], below: f64, above: f64, gap: f64) -> f64 {
        fn add(
            sum: &mut Compensated,
            mass: f64,
            side: f64,
            across: [f64; 2],
            below: f64,
            above: f64,
            gap: f64,
        ) {
            let horizontal = across[0] * across[0] + across[1] * across[1];
            let nearer = (horizontal + below * below).sqrt().min((horizontal + above * above).sqrt());
            if order_needed(side, nearer).is_some() {
                // The kernel's series branch (nothing else is taken where the series converges).
                sum.add(difference_within(mass, side, across, below, above, gap, 0.0, 0));
            } else {
                assert!(side > 1e-6, "the reference needs the points outside the cube");
                for c in octants(side) {
                    let across = [across[0] - c.x, across[1] - c.y];
                    add(sum, 0.125 * mass, 0.5 * side, across, below - c.z, above - c.z, gap);
                }
            }
        }
        let mut sum = Compensated::default();
        add(&mut sum, mass, side, across, below, above, gap);
        sum.value()
    }

    /// The difference's scale `S` of §7.4 (and of [`DIFFERENCE_EPSILON`]).
    fn difference_scale(mass: f64, side: f64, across: [f64; 2], below: f64, above: f64, gap: f64) -> f64 {
        let horizontal = across[0] * across[0] + across[1] * across[1];
        let nearer = (horizontal + below * below).sqrt().min((horizontal + above * above).sqrt());
        let rho = nearer.max(side);
        mass.abs() * (gap.abs() / (rho * rho)).min(1.0 / rho)
    }

    /// K1's cases: `(mass, side, across, below, above, gap)` for blocks of side 1, 4, 16 m at
    /// 2,000 kg/m³, the line 0.1 m and 1.5 m from a face, five heights over the block's span, gaps
    /// of 1 mm to 3 m (the upper point above the lower).
    fn large_block_cases() -> Vec<(f64, f64, [f64; 2], f64, f64, f64)> {
        let mut out = Vec::new();
        for side in [1.0, 4.0, 16.0] {
            let mass = 2000.0 * side * side * side;
            for face in [0.1, 1.5] {
                // Non-dyadic offsets, so that rounding is seen.
                let across = [-(0.5 * side + face), 0.137 * side];
                for height in [-0.61, -0.27, 0.031, 0.29, 0.58] {
                    for rise in [1e-3, 1e-2, 0.1, 1.0, 3.0] {
                        let below = height * side;
                        out.push((mass, side, across, below, below + rise, -rise));
                    }
                }
            }
        }
        out
    }

    /// The worst `|difference − reference| / S` over K1's cases with the closed form's allowance
    /// `epsilon` (infinite: S3's fixed switch).
    fn large_block_error(epsilon: f64) -> f64 {
        // The reference, once a process (the control asks twice).
        static REFERENCE: std::sync::OnceLock<Vec<f64>> = std::sync::OnceLock::new();
        let reference = REFERENCE.get_or_init(|| {
            large_block_cases()
                .into_iter()
                .map(|(m, s, across, below, above, gap)| series_reference(m, s, across, below, above, gap))
                .collect()
        });
        large_block_cases()
            .into_iter()
            .zip(reference)
            .map(|((m, s, across, below, above, gap), want)| {
                let got = difference_within(m, s, across, below, above, gap, epsilon, 0);
                (got - want).abs() / difference_scale(m, s, across, below, above, gap)
            })
            .fold(0.0, f64::max)
    }

    cav_validate::case! {
        test: validation_the_difference_near_a_large_block_keeps_its_digits,
        name: "kernel.difference_near_a_large_block",
        subsystem: "voxel",
        slice: "S5",
        reference: ClosedForm,
        against: "blocks of side 1, 4, 16 m 0.1 and 1.5 m from the line, five heights, gaps 1 mm to 3 m: cube_difference against the block split into octants until every part's series converges (no closed form), relative to the difference's scale S = M·min(|gap|/ρ², 1/ρ)",
        unit: Relative,
        tolerance: 1e-12,
        measure: cav_validate::Measured::Error(large_block_error(DIFFERENCE_EPSILON)),
    }

    #[test]
    fn s3_s_fixed_switch_loses_the_difference_s_digits() {
        // Control for K1: the closed form for every cube within PRISM_WITHIN sides fails it.
        let s3 = large_block_error(f64::INFINITY);
        assert!(s3 > 1e-12, "S3's switch kept the digits: {s3:.3e}");
        // And the kernel's own allowance does better than S3's on the same cases.
        let now = large_block_error(DIFFERENCE_EPSILON);
        assert!(now < s3, "{now:.3e} {s3:.3e}");
    }

    #[test]
    #[ignore = "a diagnostic: K1's error and its cost by the closed form's allowance"]
    fn large_block_error_by_allowance() {
        for epsilon in [f64::INFINITY, 1e-11, 3e-12, 1e-12, 3e-13, 1e-13] {
            let started = std::time::Instant::now();
            let error = large_block_error(epsilon);
            let reference = std::time::Instant::now();
            for (m, s, across, below, above, gap) in large_block_cases() {
                std::hint::black_box(difference_within(m, s, across, below, above, gap, epsilon, 0));
            }
            println!(
                "allowance {epsilon:.0e}: K1 error {error:.3e} of S; the kernel's {} cases in {:.3} ms (with the reference {:.3} s)",
                large_block_cases().len(),
                reference.elapsed().as_secs_f64() * 1e3,
                (reference - started).as_secs_f64()
            );
        }
    }

    #[test]
    #[ignore = "a diagnostic: the closed form's difference error over the size of its terms"]
    fn closed_form_rounding() {
        // For cubes within PRISM_WITHIN sides (the closed form taken whole), the error of the
        // closed-form difference against the series reference over ε × (the two points' term
        // sizes): what CLOSED_FORM_ROUNDING must be at least.
        let mut worst = 0f64;
        let mut by_case = Vec::new();
        for side in [0.05, 0.4, 1.0, 4.0, 16.0, 64.0] {
            let mass = 2000.0 * side * side * side;
            for face in [0.02, 0.1, 0.5, 1.0, 1.5] {
                for lateral in [0.137, 0.41] {
                    let across = [-(0.5 * side + face * side), lateral * side];
                    for height in [-0.61, -0.27, 0.031, 0.29, 0.58] {
                        for rise in [1e-3, 1e-2, 0.1, 1.0, 3.0] {
                            let below = height * side;
                            let above = below + rise;
                            let du = Vec3::new(across[0], across[1], above);
                            let dl = Vec3::new(across[0], across[1], below);
                            if order_needed(side, du.length().min(dl.length())).is_some() {
                                continue;
                            }
                            let (u, us) = potential_and_size(mass, side, du);
                            let (l, ls) = potential_and_size(mass, side, dl);
                            let want = series_reference(mass, side, across, below, above, -rise);
                            let ratio = ((u - l) - want).abs() / (f64::EPSILON * (us + ls));
                            worst = worst.max(ratio);
                            by_case.push((ratio, side, face, rise));
                        }
                    }
                }
            }
        }
        by_case.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (ratio, side, face, rise) in by_case.iter().take(12) {
            println!("ratio {ratio:.3} side {side} face {face}·s rise {rise}");
        }
        let mean = by_case.iter().map(|c| c.0).sum::<f64>() / by_case.len() as f64;
        println!("worst {worst:.3}, mean {mean:.3} over {} cases", by_case.len());
    }

    /// A block as a source of the forward model, by a difference rule.
    struct Block {
        mass: f64,
        side: f64,
        centre: Vec3,
        rule: Rule,
    }

    type Rule = fn(f64, f64, [f64; 2], f64, f64, f64) -> f64;

    impl Source for Block {
        fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, _: f64) -> [f64; 2] {
            let at = self.centre - tower;
            heights.map(|height| {
                let (below, above) = (height + lower - at.z, height + upper - at.z);
                (self.rule)(self.mass, self.side, [-at.x, -at.y], below, above, lower - upper)
            })
        }
    }

    /// A voxel tree's leaves as a source, by a difference rule.
    struct Leaves {
        cubes: Vec<(Vec3, f64, f64)>,
        rule: Rule,
    }

    impl Source for Leaves {
        fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, _: f64) -> [f64; 2] {
            heights.map(|height| {
                let mut sum = 0.0;
                for &(centre, mass, side) in &self.cubes {
                    let at = centre - tower;
                    let (below, above) = (height + lower - at.z, height + upper - at.z);
                    sum += (self.rule)(mass, side, [-at.x, -at.y], below, above, lower - upper);
                }
                sum
            })
        }
    }

    #[test]
    #[ignore = "a diagnostic: K3, the time of a voxel sphere's read by S3's switch and the new one (release)"]
    fn sphere_read_time() {
        use crate::forward::{Instrument, PHASE_TOLERANCE, Reader};
        use crate::voxel::shape::{Shape, Sphere};
        use crate::voxel::{Grid, voxelise};
        let reader = Reader::new(Instrument::aion10());
        let s3: Rule = |m, s, a, b, u, g| difference_within(m, s, a, b, u, g, f64::INFINITY, 0);
        for (cells, gap) in [(16.0, 0.1), (32.0, 0.1), (32.0, 1.0), (64.0, 0.1)] {
            // A 0.5 m sphere of 100 kg, its surface `gap` from the tower's line, its centre 2.2 m up.
            let sphere = Sphere { centre: Vec3::new(0.5 + gap, 0.0, 2.2), radius: 0.5 };
            let grid = Grid::around(sphere.bounds(), 0.5 / cells, 4).unwrap();
            let tree = voxelise(&sphere, &grid, 100.0, 0).unwrap();
            let cubes: Vec<_> = tree
                .leaves
                .iter()
                .map(|leaf| {
                    (tree.block_centre(leaf.key, leaf.level), tree.leaf_mass(leaf), tree.side(leaf.level))
                })
                .collect();
            let time = |rule: Rule| {
                let source = Leaves { cubes: cubes.clone(), rule };
                let started = std::time::Instant::now();
                let audited = reader.read_to(Vec3::ZERO, &source, 1.46, PHASE_TOLERANCE);
                (started.elapsed().as_secs_f64(), audited.reading)
            };
            // Interleaved, the best of three each: the PC is shared.
            let (mut old, mut new) = (f64::INFINITY, f64::INFINITY);
            let (mut a, mut b) = (None, None);
            for _ in 0..3 {
                let (t, r) = time(s3);
                old = old.min(t);
                a = Some(r);
                let (t, r) = time(cube_difference);
                new = new.min(t);
                b = Some(r);
            }
            let (a, b) = (a.unwrap(), b.unwrap());
            println!(
                "r/{cells}, surface {gap} m: {} leaves; S3 {old:.3} s, now {new:.3} s ({:+.0} %); gradiometer {:.15e} against {:.15e} rad",
                cubes.len(),
                100.0 * (new / old - 1.0),
                b.lower - b.upper,
                a.lower - a.upper
            );
        }
    }

    #[test]
    #[ignore = "a diagnostic: K2, the AION-10 read of a large block by S3's switch and the new one"]
    fn large_block_by_flight() {
        use crate::forward::{Channel, Instrument, Reader};
        let reader = Reader::new(Instrument::aion10());
        let s3: Rule = |m, s, a, b, u, g| difference_within(m, s, a, b, u, g, f64::INFINITY, 0);
        for side in [1.0, 4.0, 16.0] {
            for face in [0.1, 1.5] {
                // The block's near face `face` from the tower's line, its centre at 3.3 m.
                let centre = Vec3::new(0.5 * side + face, 0.137 * side, 3.3);
                let block = |rule: Rule| Block { mass: 2000.0 * side * side * side, side, centre, rule };
                let read = |rule: Rule| {
                    let started = std::time::Instant::now();
                    let reading = reader.read(Vec3::ZERO, &block(rule), 1.46);
                    (reading, started.elapsed().as_secs_f64())
                };
                let (reference, _) = read(series_reference);
                let (old, old_time) = read(s3);
                let (new, new_time) = read(cube_difference);
                let sign = reader.instrument().sign;
                for channel in Channel::ALL {
                    let r = reference.channel(channel, sign);
                    println!(
                        "side {side} face {face} {channel:?}: phase {r:.6e} rad; S3 {:.2e} rad, now {:.2e} rad (times {old_time:.3} s, {new_time:.3} s)",
                        (old.channel(channel, sign) - r).abs(),
                        (new.channel(channel, sign) - r).abs()
                    );
                }
            }
        }
    }

    #[test]
    fn the_kernel_holds_inside_and_on_the_cube() {
        // The prism at points inside and on the cube against a fine quadrature by octants (each
        // octant's own Gauss rule, the point at a corner of each).
        for d in [
            Vec3::ZERO,
            Vec3::new(0.2, -0.1, 0.3),
            Vec3::new(0.5, 0.0, 0.0),
            Vec3::new(0.5, 0.5, 0.5),
            Vec3::new(0.7, 0.1, -0.2),
        ] {
            let v = cube_potential(1.0, 1.0, d);
            assert!(v.is_finite() && v < 0.0, "{d:?}: {v}");
        }
        // The centre of a unit cube of unit density: −∫ dV/r = −3·(a known constant) ≈ −2.380077 (twice the
        // potential at a corner of the cube of side 1/2 summed over 8 octants: here as a check of sign and size).
        let centre = cube_potential(1.0, 1.0, Vec3::ZERO);
        // 2·(3/2·ln(2 + √3) − π/4): the corner integral of the unit cube, eight octants of side ½.
        let want = 2.0 * (1.5 * (2.0 + 3f64.sqrt()).ln() - std::f64::consts::FRAC_PI_4);
        assert!((centre + want).abs() < 1e-14, "{centre} against {want}");
        // Control: the series is not used where it does not converge.
        assert_eq!(order_needed(1.0, 0.5), None);
    }
}
