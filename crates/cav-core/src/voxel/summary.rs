//! What a node of the world's octree knows of its matter: the mass moments about the node's own
//! centre, the absolute moments that bound what an expansion to the octupole leaves out, and how
//! far its matter reaches (`docs/slices/S5.md` §2 "The world octree"; the upward rule is
//! `docs/research/s5-world-octree.md` §2's table).
//!
//! A [`Summary`] describes a set of signed point-like masses and uniform cubes about a fixed
//! **expansion centre** `c` (for a node, its geometric centre, never its centre of mass):
//!
//! - raw moments about `c`, `M = Σ m`, `F_a = Σ m u_a`, `S_ab = Σ m u_a u_b`,
//!   `T_abc = Σ m u_a u_b u_c` with `u = x − c`. They are **additive**: the summary of two sets is
//!   the sum of their summaries about the same centre, whatever the signs of the masses, so a
//!   negative departure (a ditch carved from a plane, TASK-S5-13) adds as a building does;
//! - absolute moments `A_k = Σ |m| s^k`, `s = |u|`, `k = 0 … 16`: upper bounds (exact for points,
//!   for a cube exact at `k = 0, 2, 4` and taken at its far corner otherwise);
//! - the reach `e`: every bit of matter is within `e` of `c`.
//!
//! The expansion of the potential about `c` to the octupole ([`Summary::difference`]) leaves out
//! the orders `l ≥ 4`; their gradient is at most `Σ_{l≥4} (l+1) A_l / R^{l+2}` at a distance `R > e`
//! from `c` ([`Summary::bound`], S4's `Multipole::bound` about a geometric centre: the dipole is
//! kept, so nothing needs the centre of mass).
//!
//! **What must not be done.** Raw moments are only ever formed about a nearby centre: about the
//! node's own centre each term is at most `|m| side^k`, while about a far origin `D` away the
//! third moment would hold `m D³` and lose the node's own octupole to rounding. Moving a summary is
//! [`Summary::shifted`], whose binomial algebra is exact but whose rounding grows as `(δ/side)^k`:
//! move by about a node's size (a child to its parent), not across the world. The `A_k` are bounds,
//! never subtracted from one another: they are only added and enlarged.
//!
//! Frames and units: whatever frame the caller's points are in (the world's, for the octree),
//! metres; masses kg; `A_k` kg m^k; the evaluator's result kg/m (`V/G`).

use crate::Vec3;

use super::tree::{THIRD, second_index, third_index};

/// How many absolute moments a [`Summary`] carries: `A_k` for `k = 0 … 16`.
pub const ABSOLUTE_ORDERS: usize = 17;

/// The first order the expansion of [`Summary::difference`] leaves out (it keeps the monopole,
/// dipole, quadrupole and octupole).
pub const FIRST_LEFT_OUT: usize = 4;

/// The mass moments and the bounds of a set of signed masses about a fixed centre. See the
/// module's documentation for what each field is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summary {
    /// The expansion centre `c`, metres, the caller's frame.
    pub centre: Vec3,
    /// `Σ m`, kg (signed).
    pub mass: f64,
    /// `Σ m u_a`, kg m, in the order x, y, z (`u = x − c`).
    pub first: [f64; 3],
    /// `Σ m u_a u_b`, kg m², in the order xx, yy, zz, xy, xz, yz (raw, about `c`).
    pub second: [f64; 6],
    /// `Σ m u_a u_b u_c`, kg m³, in the order xxx, yyy, zzz, xxy, xxz, xyy, yyz, xzz, yzz, xyz
    /// (raw, about `c`).
    pub third: [f64; 10],
    /// `absolute[k]` bounds `Σ |m| s^k` from above, kg m^k, `s` the distance from `c`;
    /// `absolute[0]` is `Σ |m|` exactly.
    pub absolute: [f64; ABSOLUTE_ORDERS],
    /// Metres: every bit of the matter is within this distance of `c` (0 when there is none).
    pub reach: f64,
}

/// `C(n, k)` as a float, for the small `n` used here (at most 32: exact in `f64`).
fn binomial(n: usize, k: usize) -> f64 {
    let mut out = 1.0;
    for i in 0..k {
        out = out * (n - i) as f64 / (i + 1) as f64;
    }
    out
}

impl Summary {
    /// No matter, about `centre`.
    #[must_use]
    pub const fn empty(centre: Vec3) -> Self {
        Self {
            centre,
            mass: 0.0,
            first: [0.0; 3],
            second: [0.0; 6],
            third: [0.0; 10],
            absolute: [0.0; ABSOLUTE_ORDERS],
            reach: 0.0,
        }
    }

    /// Whether it holds no matter: `Σ |m| = 0`. (A summary whose signed masses sum to zero is not
    /// empty.)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.absolute[0] == 0.0
    }

    /// The summary about `centre` of uniform cubes `(centre, metres; mass, kg, signed; side, m)`,
    /// added in the order given. A side of 0 is a point mass.
    #[must_use]
    pub fn of_cubes(centre: Vec3, cubes: impl IntoIterator<Item = (Vec3, f64, f64)>) -> Self {
        let mut out = Self::empty(centre);
        for (at, mass, side) in cubes {
            out.add_cube(at, mass, side);
        }
        out
    }

    /// Adds a uniform cube of `mass` kg (signed) and side `side` m centred at `at`: its raw
    /// moments exactly (`S_ab = m(u_a u_b + δ_ab h²/12)`, `T_abc = m u_a u_b u_c + m h²/12 (δ_ab
    /// u_c + δ_ac u_b + δ_bc u_a)`), `A_2 = |m|(d² + h²/4)` and `A_4 = |m|(d⁴ + 5d²h²/6 +
    /// 19h⁴/240)` exactly, and the other `A_k` as all its mass at its far corner, `d + h√3/2`.
    /// Nothing is added for a mass of 0.
    pub fn add_cube(&mut self, at: Vec3, mass: f64, side: f64) {
        if mass == 0.0 {
            return;
        }
        let u = at - self.centre;
        let u = [u.x, u.y, u.z];
        let own = mass * side * side / 12.0;
        self.mass += mass;
        for a in 0..3 {
            self.first[a] += mass * u[a];
            for b in a..3 {
                let delta = if a == b { own } else { 0.0 };
                self.second[second_index(a, b)] += mass * u[a] * u[b] + delta;
            }
        }
        for (a, b, c) in THIRD {
            let spread = |p: usize, q: usize, r: usize| if p == q { own * u[r] } else { 0.0 };
            self.third[third_index(a, b, c)] +=
                mass * u[a] * u[b] * u[c] + spread(a, b, c) + spread(a, c, b) + spread(b, c, a);
        }
        let weight = mass.abs();
        let d2 = u[0] * u[0] + u[1] * u[1] + u[2] * u[2];
        let far = d2.sqrt() + 0.5 * side * 3f64.sqrt();
        let h2 = side * side;
        let mut power = weight;
        for (k, a) in self.absolute.iter_mut().enumerate() {
            *a += match k {
                2 => weight * (d2 + 0.25 * h2),
                4 => weight * (d2 * d2 + 5.0 * d2 * h2 / 6.0 + 19.0 * h2 * h2 / 240.0),
                _ => power,
            };
            power *= far;
        }
        self.reach = self.reach.max(far);
    }

    /// The second moment `ab` (axes 0, 1, 2 for x, y, z), raw, about the centre.
    #[must_use]
    pub const fn second_at(&self, a: usize, b: usize) -> f64 {
        self.second[second_index(a, b)]
    }

    /// The third moment `abc`, raw, about the centre.
    #[must_use]
    pub const fn third_at(&self, a: usize, b: usize, c: usize) -> f64 {
        self.third[third_index(a, b, c)]
    }

    /// The same matter about `to`: the raw moments moved by the binomial rules (exact algebra:
    /// with `δ = to − c`, `F' = F − Mδ`, `S'_ab = S_ab − F_a δ_b − F_b δ_a + M δ_a δ_b`, and the
    /// third moments likewise); the reach `e + |δ|`; each `A_l` the smaller of the triangle
    /// inequality's `Σ_k C(l,k) A_k |δ|^{l−k}` and `A_0 e'^l`. Move by about the summary's own size
    /// at most (the module's documentation says why).
    #[must_use]
    pub fn shifted(&self, to: Vec3) -> Self {
        let step = to - self.centre;
        let d = [step.x, step.y, step.z];
        let m = self.mass;
        let mut out = Self::empty(to);
        out.mass = m;
        for a in 0..3 {
            out.first[a] = self.first[a] - m * d[a];
            for b in a..3 {
                out.second[second_index(a, b)] =
                    self.second_at(a, b) - self.first[a] * d[b] - self.first[b] * d[a] + m * d[a] * d[b];
            }
        }
        for (a, b, c) in THIRD {
            out.third[third_index(a, b, c)] = self.third_at(a, b, c)
                - (self.second_at(a, b) * d[c] + self.second_at(a, c) * d[b] + self.second_at(b, c) * d[a])
                + (self.first[a] * d[b] * d[c] + self.first[b] * d[a] * d[c] + self.first[c] * d[a] * d[b])
                - m * d[a] * d[b] * d[c];
        }
        if self.is_empty() {
            return out;
        }
        let delta = step.length();
        out.reach = self.reach + delta;
        out.absolute = self.carried(delta, out.reach);
        out
    }

    /// The absolute moments about a centre `delta` metres away whose reach is `reach`.
    fn carried(&self, delta: f64, reach: f64) -> [f64; ABSOLUTE_ORDERS] {
        let mut out = [0.0; ABSOLUTE_ORDERS];
        out[0] = self.absolute[0];
        let mut cap = self.absolute[0];
        for (l, slot) in out.iter_mut().enumerate().skip(1) {
            cap *= reach;
            let mut sum = 0.0;
            let mut power = 1.0;
            for k in (0..=l).rev() {
                sum += binomial(l, k) * self.absolute[k] * power;
                power *= delta;
            }
            *slot = sum.min(cap);
        }
        out
    }

    /// The summary about `centre` of `parts` together: each moved to `centre` ([`Self::shifted`])
    /// and added in the order given (the octree gives its children in octant order, so the bits
    /// do not depend on how the parts were made). The `A_l` are then also capped by `A_0 e^l`
    /// with the whole's reach `e`. Empty parts are skipped.
    #[must_use]
    pub fn sum(centre: Vec3, parts: &[Self]) -> Self {
        let mut out = Self::empty(centre);
        for part in parts.iter().filter(|p| !p.is_empty()) {
            let moved = part.shifted(centre);
            out.mass += moved.mass;
            for (o, p) in out.first.iter_mut().zip(moved.first) {
                *o += p;
            }
            for (o, p) in out.second.iter_mut().zip(moved.second) {
                *o += p;
            }
            for (o, p) in out.third.iter_mut().zip(moved.third) {
                *o += p;
            }
            for (o, p) in out.absolute.iter_mut().zip(moved.absolute) {
                *o += p;
            }
            out.reach = out.reach.max(moved.reach);
        }
        let mut cap = out.absolute[0];
        for a in &mut out.absolute[1..] {
            cap *= out.reach;
            *a = a.min(cap);
        }
        out
    }

    /// `(V(upper) − V(lower))/G`, kg/m, at two points on a vertical line, by the expansion about
    /// the centre to the octupole: `across` is the line's horizontal offset from the centre,
    /// `below` and `above` the two points' heights over it, metres, and `gap = below − above` as
    /// the caller knows it exactly (as [`super::kernel::cube_difference`] takes them). The
    /// monopole's and the dipole's differences are formed without cancellation (`1/a − 1/b =
    /// gap (below + above)/(ab(a + b))`, and `1/a³ − 1/b³` from it); the quadrupole and octupole
    /// as the difference of their values. Good to the bound [`Self::bound`] when both points are
    /// farther than the reach.
    #[must_use]
    pub fn difference(&self, across: [f64; 2], below: f64, above: f64, gap: f64) -> f64 {
        self.difference_to(3, across, below, above, gap)
    }

    /// [`Self::difference`] with the expansion kept to `order` (0 the monopole … 3 the octupole).
    fn difference_to(&self, order: usize, across: [f64; 2], below: f64, above: f64, gap: f64) -> f64 {
        let horizontal = across[0] * across[0] + across[1] * across[1];
        let (b, a) = ((horizontal + below * below).sqrt(), (horizontal + above * above).sqrt());
        // 1/a − 1/b, from b² − a² = (below − above)(below + above).
        let k1 = gap * (below + above) / (a * b * (a + b));
        let mut sum = self.mass * k1;
        if order >= 1 {
            // 1/a³ − 1/b³ = (1/a − 1/b)(1/a² + 1/(ab) + 1/b²); and above/a³ − below/b³ =
            // (above − below)/a³ + below (1/a³ − 1/b³).
            let k3 = k1 * (1.0 / (a * a) + 1.0 / (a * b) + 1.0 / (b * b));
            sum += (self.first[0] * across[0] + self.first[1] * across[1]) * k3
                + self.first[2] * (-gap / (a * a * a) + below * k3);
        }
        if order >= 2 {
            let du = [across[0], across[1], above];
            let dl = [across[0], across[1], below];
            sum += self.higher(order, du) - self.higher(order, dl);
        }
        -sum
    }

    /// `−V/G`'s quadrupole and (for `order` 3) octupole terms at offset `d` from the centre, kg/m:
    /// `Σ S_ab (3 d_a d_b − δ_ab R²)/(2R⁵) + [5 Σ T_abc d_a d_b d_c − 3R² Σ T_aac d_c]/(2R⁷)`.
    fn higher(&self, order: usize, d: [f64; 3]) -> f64 {
        let r2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        let r = r2.sqrt();
        let (mut quad, mut cube, mut trace) = (0.0, 0.0, 0.0);
        for a in 0..3 {
            for b in 0..3 {
                let delta = if a == b { r2 } else { 0.0 };
                quad += self.second_at(a, b) * (3.0 * d[a] * d[b] - delta);
                for c in 0..3 {
                    cube += self.third_at(a, b, c) * d[a] * d[b] * d[c];
                }
                trace += self.third_at(a, a, b) * d[b];
            }
        }
        let mut out = quad / (2.0 * r2 * r2 * r);
        if order >= 3 {
            out += (5.0 * cube - 3.0 * r2 * trace) / (2.0 * r2 * r2 * r2 * r);
        }
        out
    }

    /// The most the orders the expansion leaves out (`l ≥ 4`) can change `V/G` between two points
    /// `separation` metres apart on a line every point of which is at least `distance` (`R`)
    /// metres from the centre, kg/m: `separation × Σ_{l≥4} (l+1) A_l / R^{l+2}`, the `A_l` to
    /// `l = 16` and past it `A_16 e^{l−16}` (since `s ≤ e`), summed until its terms are below
    /// 1e-17 of the first. `(l+1)/R^{l+2}` is the most the gradient of an order-`l` term per unit
    /// `s^l` can be (S4's `the_legendre_gradient_bound_holds`). Infinite when `e/R ≥ 1` (the
    /// series does not converge) or `R` is not positive.
    #[must_use]
    pub fn bound(&self, distance: f64, separation: f64) -> f64 {
        self.bound_from(FIRST_LEFT_OUT, distance, separation)
    }

    /// [`Self::bound`] for the orders from `first` on.
    fn bound_from(&self, first: usize, distance: f64, separation: f64) -> f64 {
        if self.is_empty() {
            return 0.0;
        }
        if distance.is_nan() || distance <= 0.0 || self.reach >= distance {
            return f64::INFINITY;
        }
        let q = self.reach / distance;
        let mut sum = 0.0;
        let mut scale = distance.powi(-(first as i32 + 2));
        for (l, a) in self.absolute.iter().enumerate().skip(first) {
            sum += (l + 1) as f64 * a * scale;
            scale /= distance;
        }
        // The tail: (l + 1) A_16 e^{l−16} / R^{l+2} for l > 16, that is A_16/R^18 (l + 1) q^{l−16}.
        let last = self.absolute[ABSOLUTE_ORDERS - 1] * distance.powi(-(ABSOLUTE_ORDERS as i32 + 1));
        let (mut power, mut l) = (q, ABSOLUTE_ORDERS);
        while power > 1e-17 && l < 4000 {
            sum += (l + 1) as f64 * last * power;
            power *= q;
            l += 1;
        }
        separation.abs() * sum
    }
}

/// How many even powers [`cube_even_powers`] gives: `s^0, s^2, …, s^16`.
pub const EVEN_POWERS: usize = ABSOLUTE_ORDERS / 2 + 1;

/// The means of `|u + v|^{2n}` for `n = 0 … 8` over `v` uniform in a cube of side `side` metres
/// centred on the origin, m^{2n}: so `mass × cube_even_powers(u, side)[n]` is a uniform cube's
/// exact `Σ m s^{2n}` about a point `u` from its centre. By the multinomial theorem and the
/// independence of the three axes, `Σ_{i+j+k=n} n!/(i!j!k!) X_i Y_j Z_k` with `X_i = E(u_x +
/// v_x)^{2i} = Σ_{m even} C(2i, m) u_x^{2i−m} (h/2)^m/(m + 1)`. For checking the absolute
/// moments' bounds (and for tightening them: `plan/IDEAS.md`).
#[must_use]
pub fn cube_even_powers(u: Vec3, side: f64) -> [f64; EVEN_POWERS] {
    const TOP: usize = 2 * (EVEN_POWERS - 1);
    // Pascal's triangle to row 16, and (h/2)^m/(m + 1).
    let mut choose = [[0.0f64; TOP + 1]; TOP + 1];
    for n in 0..=TOP {
        choose[n][0] = 1.0;
        for k in 1..=n {
            choose[n][k] = choose[n - 1][k - 1] + if k < n { choose[n - 1][k] } else { 0.0 };
        }
    }
    let mut spread = [0.0f64; TOP + 1];
    let mut power = 1.0;
    for (m, s) in spread.iter_mut().enumerate() {
        *s = power / (m + 1) as f64;
        power *= 0.5 * side;
    }
    let axis = |x: f64| -> [f64; EVEN_POWERS] {
        let mut powers = [1.0f64; TOP + 1];
        for k in 1..=TOP {
            powers[k] = powers[k - 1] * x;
        }
        std::array::from_fn(|i| {
            let p = 2 * i;
            (0..=p).step_by(2).map(|m| choose[p][m] * powers[p - m] * spread[m]).sum()
        })
    };
    let (xs, ys, zs) = (axis(u.x), axis(u.y), axis(u.z));
    std::array::from_fn(|n| {
        let mut sum = 0.0;
        for i in 0..=n {
            for j in 0..=n - i {
                sum += choose[n][i] * choose[n - i][j] * xs[i] * ys[j] * zs[n - i - j];
            }
        }
        sum
    })
}

/// The true `Σ |m| s^l` of uniform cubes `(centre, mass, side)` about `centre` for even `l`
/// (exact, [`cube_even_powers`]), and for odd `l` the Cauchy–Schwarz bound `√(A_{l−1} A_{l+1})`
/// of the exact even ones, which is never below the truth: what a [`Summary`]'s `absolute` must
/// not be below. For tests of the bounds and for measuring how loose they are.
#[must_use]
pub fn cubes_absolute(
    centre: Vec3,
    cubes: impl IntoIterator<Item = (Vec3, f64, f64)>,
) -> [f64; ABSOLUTE_ORDERS] {
    // ABSOLUTE_ORDERS is odd: its last order, 16, is even.
    let mut out = [0.0; ABSOLUTE_ORDERS];
    for (at, mass, side) in cubes {
        for (n, p) in cube_even_powers(at - centre, side).into_iter().enumerate() {
            out[2 * n] += mass.abs() * p;
        }
    }
    for l in (1..ABSOLUTE_ORDERS).step_by(2) {
        out[l] = (out[l - 1] * out[l + 1]).sqrt();
    }
    out
}

#[cfg(test)]
mod tests;
