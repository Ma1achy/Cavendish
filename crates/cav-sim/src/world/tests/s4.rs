//! S4's chunk read through its moments about its centre of mass, kept as S4 wrote it for R7
//! (`world.moments_within_their_bound`, `world.moments_bound_is_not_vacuous`), whose numbers
//! stay S4's. Since S5 the read takes each chunk through its octree summary about the chunk's
//! centre (`walk`); that read's own per-chunk numbers, and why its looseness is not held by R7's
//! pointwise ratio (a root of the error at q ≈ 0.057), are in `docs/slices/S5.md` §7.3.

use cav_core::Vec3;
use cav_core::forward::Source;
use cav_core::voxel::tree::Moments;
use cav_core::voxel::world::{Chunk, WorldGrid};

use crate::world::to_segment;
/// The orders `l` from 4 whose `Σ m s^l` a [`Multipole`] keeps one by one; past them the bound
/// goes geometrically from the last (`Σ m s^l ≤ a^{l−L} Σ m s^L`).
pub(super) const BOUND_ORDERS: usize = 13;

/// A chunk read through its moments: mass, centre of mass, second and third central moments,
/// its radius `a` about the centre (everything of it is within `a`) and bounds on its moments of
/// order 4 to `3 + BOUND_ORDERS`, which together bound what the octupole expansion leaves out.
#[derive(Clone, Copy, Debug)]
pub(super) struct Multipole {
    /// The moments, in the world's frame.
    pub(super) moments: Moments,
    /// The radius about the centre of mass holding all the chunk's matter, metres.
    pub(super) radius: f64,
    /// `powers[i]` bounds `Σ m s^(4+i)` over the chunk's matter from above, `s` the distance from
    /// the centre of mass, kg m^(4+i): for `l = 4` each leaf's exact `∫ s⁴ dm` (a uniform cube of
    /// side `h` whose centre is `d` from the centre of mass: `m (d⁴ + 5d²h²/6 + 19h⁴/240)`); for
    /// higher orders each leaf's mass at its farthest point, `d + h√3/2`.
    pub(super) powers: [f64; BOUND_ORDERS],
}

/// `l + 1` for `l` from 4: the most the gradient of `P_l(cos γ)/R^{l+1}` can be, in units of
/// `1/R^{l+2}`. Its square is `(l+1)² P_l² + (1 − x²) P_l'²`, and since
/// `P_l² + (1 − x²) P_l'²/(l(l+1)) ≤ 1` on `[−1, 1]` (checked in
/// `the_legendre_gradient_bound_holds`) that is at most `l(l+1) + (l+1) P_l² ≤ (l+1)²`; it is
/// reached on the axis.
fn gradient_factor(l: u32) -> f64 {
    f64::from(l + 1)
}

impl Multipole {
    /// The chunk's multipole: its moments, and its radius and moment bounds from its leaves.
    #[must_use]
    pub(super) fn of(chunk: &Chunk, grid: &WorldGrid, densities: &[f64]) -> Self {
        let moments = chunk.moments(grid, densities);
        let (mut radius, mut powers) = (0f64, [0f64; BOUND_ORDERS]);
        for (centre, mass, side) in chunk.cubes(grid, densities) {
            let d = (centre - moments.centre).length();
            let far = d + 0.5 * side * 3f64.sqrt();
            radius = radius.max(far);
            let (d2, h2) = (d * d, side * side);
            powers[0] += mass * (d2 * d2 + 5.0 * d2 * h2 / 6.0 + 19.0 * h2 * h2 / 240.0);
            let mut power = mass * far.powi(4);
            for p in &mut powers[1..] {
                power *= far;
                *p += power;
            }
        }
        Self { moments, radius, powers }
    }

    /// `−V/G` at offset `d` from the centre of mass, kg/m, to the octupole, without the monopole
    /// (which the caller forms differentially): `Σ S_ab (3 d_a d_b − δ_ab R²)/(2R⁵) +
    /// [5 Σ T_abc d_a d_b d_c − 3R² Σ T_aac d_c]/(2R⁷)`.
    fn higher(&self, d: Vec3) -> f64 {
        let m = &self.moments;
        let d = [d.x, d.y, d.z];
        let r2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        let r = r2.sqrt();
        let mut quad = 0.0;
        let mut cube = 0.0;
        let mut trace = 0.0;
        for a in 0..3 {
            for b in 0..3 {
                let delta = if a == b { r2 } else { 0.0 };
                quad += m.second_at(a, b) * (3.0 * d[a] * d[b] - delta);
                for c in 0..3 {
                    cube += m.third_at(a, b, c) * d[a] * d[b] * d[c];
                }
                trace += m.third_at(a, a, b) * d[b];
            }
        }
        quad / (2.0 * r2 * r2 * r) + (5.0 * cube - 3.0 * r2 * trace) / (2.0 * r2 * r2 * r2 * r)
    }

    /// `(V(upper) − V(lower))/G`, kg/m, at two points on a vertical line: `across` the horizontal
    /// offset from the centre of mass, `below` and `above` the heights over it, `gap = below −
    /// above` as the caller knows it exactly (as [`cube_difference`] takes them).
    #[must_use]
    pub(super) fn difference(&self, across: [f64; 2], below: f64, above: f64, gap: f64) -> f64 {
        let horizontal = across[0] * across[0] + across[1] * across[1];
        let (b, a) = ((horizontal + below * below).sqrt(), (horizontal + above * above).sqrt());
        let monopole = -self.moments.mass * gap * (below + above) / (a * b * (a + b));
        let du = Vec3::new(across[0], across[1], above);
        let dl = Vec3::new(across[0], across[1], below);
        monopole - (self.higher(du) - self.higher(dl))
    }

    /// The most the remainder after the octupole can change `V/G` between two points `separation`
    /// metres apart on a line all of whose points are at least `distance` (`R`) from the centre of
    /// mass, kg/m. The remainder's gradient is at most `Σ_{l≥4} (l+1) Σ m s^l / R^{l+2}`, each
    /// `Σ m s^l` from [`Multipole::powers`] to order `3 + BOUND_ORDERS` and past it
    /// `powers[last] · a^{l−L}`; times the separation. Infinite when `q = a/R ≥ 1`.
    #[must_use]
    pub(super) fn bound(&self, distance: f64, separation: f64) -> f64 {
        let q = self.radius / distance;
        if q >= 1.0 {
            return f64::INFINITY;
        }
        let mut sum = 0.0;
        let mut scale = 1.0 / distance.powi(6);
        let mut l = 4u32;
        for p in &self.powers {
            sum += gradient_factor(l) * p * scale;
            scale /= distance;
            l += 1;
        }
        // The tail, from the last kept order on geometrically: Σ_{l>L} (l+1) P_L a^{l−L}/R^{l+2},
        // to where its terms are below 1e-17 of its first.
        let last = self.powers[BOUND_ORDERS - 1] / distance.powi(BOUND_ORDERS as i32 + 5);
        let mut power = q;
        while power > 1e-17 && l < 4000 {
            sum += gradient_factor(l) * last * power;
            power *= q;
            l += 1;
        }
        separation.abs() * sum
    }
}

/// S4's chunks read through their moments, as a source.
pub(super) struct Moments4(pub(super) Vec<Multipole>);

impl Source for Moments4 {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, _time: f64) -> [f64; 2] {
        heights.map(|height| {
            let mut sum = 0.0;
            for far in &self.0 {
                let at = far.moments.centre - tower;
                let (below, above) = (height + lower - at.z, height + upper - at.z);
                sum += far.difference([-at.x, -at.y], below, above, lower - upper);
            }
            sum
        })
    }
}

/// S4's bound on what reading chunks through their moments leaves out, as a source.
pub(super) struct Bound4<'a>(pub(super) &'a [Multipole]);

impl Source for Bound4<'_> {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, _time: f64) -> [f64; 2] {
        heights.map(|height| {
            let (z0, z1) = (tower.z + height + lower.min(upper), (upper - lower).abs());
            let base = Vec3::new(tower.x, tower.y, z0);
            self.0.iter().map(|far| far.bound(to_segment(far.moments.centre, base, z1), z1)).sum()
        })
    }
}
