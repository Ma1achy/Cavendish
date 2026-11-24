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

/// The series to order `order` for a cube of `mass` kg and side `side` m at offset `d` (metres,
/// from its centre): `V/G`, kg/m. For tests; the kernel chooses its own order.
#[must_use]
pub fn cube_series(order: usize, mass: f64, side: f64, d: Vec3) -> f64 {
    let r = d.length();
    -mass * (1.0 / r + correction(order, side, d, r))
}

/// Nagy's term for one corner of a prism at offset `(x, y, z)` from the point, m².
fn corner(x: f64, y: f64, z: f64) -> f64 {
    let r = (x * x + y * y + z * z).sqrt();
    // A term whose coefficient is zero is zero, whatever its logarithm or angle would be.
    let log = |c: f64, u: f64| if c == 0.0 { 0.0 } else { c * (u + r).ln() };
    let angle = |p: f64, q: f64, w: f64| if p == 0.0 { 0.0 } else { 0.5 * p * p * (q * w / (p * r)).atan() };
    log(x * y, z) + log(y * z, x) + log(z * x, y) - angle(x, y, z) - angle(y, z, x) - angle(z, x, y)
}

/// The potential of a uniform rectangular prism of `density` kg/m³ spanning `lo` to `hi` (its
/// corners as offsets from the point asked about, metres), over G: kg/m. Exact in closed form,
/// finite inside and on the prism. Far away its terms cancel: the kernel uses it only near.
#[must_use]
pub fn prism_potential(density: f64, lo: Vec3, hi: Vec3) -> f64 {
    let mut total = 0.0;
    for (i, x) in [lo.x, hi.x].into_iter().enumerate() {
        for (j, y) in [lo.y, hi.y].into_iter().enumerate() {
            for (k, z) in [lo.z, hi.z].into_iter().enumerate() {
                let sign = if (i + j + k) % 2 == 0 { -1.0 } else { 1.0 };
                total += sign * corner(x, y, z);
            }
        }
    }
    -density * total
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
    let r = d.length();
    match order_needed(side, r) {
        Some(order) => -mass * (1.0 / r + correction(order, side, d, r)),
        None if r >= PRISM_WITHIN * side => {
            octants(side).iter().map(|c| cube_potential(0.125 * mass, 0.5 * side, d - *c)).sum()
        }
        None => {
            let h = 0.5 * side;
            let half = Vec3::new(h, h, h);
            prism_potential(mass / side.powi(3), -d - half, -d + half)
        }
    }
}

/// `(V(upper) − V(lower))/G` (kg/m) for a uniform cube of `mass` kg and side `side` m, the two
/// points on a vertical line `across` (horizontal offset from the cube's centre, metres) at
/// heights `below` and `above` metres over the centre. `gap` is `below − above` as the caller
/// knows it exactly (the arms' rise, lower minus upper), so the monopole's difference is formed
/// without cancellation.
#[must_use]
pub fn cube_difference(mass: f64, side: f64, across: [f64; 2], below: f64, above: f64, gap: f64) -> f64 {
    let horizontal = across[0] * across[0] + across[1] * across[1];
    let (b, a) = ((horizontal + below * below).sqrt(), (horizontal + above * above).sqrt());
    let nearer = a.min(b);
    match order_needed(side, nearer) {
        Some(order) => {
            // 1/a − 1/b = (b² − a²)/(ab(a + b)), b² − a² = (below − above)(below + above).
            let monopole = -mass * gap * (below + above) / (a * b * (a + b));
            let du = Vec3::new(across[0], across[1], above);
            let dl = Vec3::new(across[0], across[1], below);
            monopole - mass * (correction(order, side, du, a) - correction(order, side, dl, b))
        }
        None if nearer >= PRISM_WITHIN * side => octants(side)
            .iter()
            .map(|c| {
                let across = [across[0] - c.x, across[1] - c.y];
                cube_difference(0.125 * mass, 0.5 * side, across, below - c.z, above - c.z, gap)
            })
            .sum(),
        None => {
            cube_potential(mass, side, Vec3::new(across[0], across[1], above))
                - cube_potential(mass, side, Vec3::new(across[0], across[1], below))
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

/// A voxel body as the forward model sees it: every leaf of a tree a uniform cube, the tree's
/// frame carried to where the body is.
pub struct TreeBody<P: Fn(f64) -> Vec3 + Sync> {
    /// The leaves as `(centre in the body's frame, metres; mass, kg; side, metres)`, in Morton
    /// order: the order they are summed in.
    cubes: Vec<(Vec3, f64, f64)>,
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
        Self { cubes, position, exists: (f64::NEG_INFINITY, f64::INFINITY), breaks: Vec::new() }
    }

    /// One leaf of `tree` alone, at `position`, always there: what that leaf adds.
    pub fn single(tree: &VoxTree, leaf: &super::Leaf, position: P) -> Self {
        let cube = (tree.block_centre(leaf.key, leaf.level), tree.leaf_mass(leaf), tree.side(leaf.level));
        Self { cubes: vec![cube], position, exists: (f64::NEG_INFINITY, f64::INFINITY), breaks: Vec::new() }
    }

    /// How many cubes are summed at each point asked about.
    #[must_use]
    pub fn cubes(&self) -> usize {
        self.cubes.len()
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
