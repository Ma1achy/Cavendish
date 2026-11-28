//! The far field of a tower's cut gathered into one series along the tower's line: the fast
//! multipole method's local expansion, specialised to a vertical gradiometer
//! (`docs/slices/S5.md` §2 "The local expansion"; `docs/research/s5-world-octree.md` §2.2).
//!
//! **What it is.** Every arm point a tower's atoms reach lies on one vertical line, between the
//! tower's base and its top: the **span**. The accepted nodes of the tower's cut are split in two
//! ([`Local::gather`]): the **near** ones, whose expansion's singularity (at the node's centre,
//! `z = c_z ± i ρ`, `ρ` the node's horizontal distance from the line) is too close to the span for
//! a short series (its Bernstein ellipse parameter below [`NEAR_ELLIPSE`]), are read directly as
//! [`super::walk::Far`] reads them; the rest make the far field
//! `D(z) = Σ_n (V_n(z) − V_n(z_m))/G` (`z_m` the span's middle, each term formed without
//! cancellation by [`super::walk::Rule::difference`]), which is interpolated at Chebyshev points
//! on the span. The degree is doubled from [`FIRST_DEGREE`] until the expansion's **audit** is
//! within its share of the read's tolerance, at most [`MAX_LOCAL_DEGREE`]. A read of two points on
//! the line is then `D(upper) − D(lower)` by the series, plus the near nodes.
//!
//! **The audit** ([`Series::audit`], radians). At degree `n` the function is sampled at the
//! `n + 1` Chebyshev–Lobatto points; the `n` points between them (the next doubling's new points)
//! are evaluated directly and compared with the series there. The audit is twice the largest of
//! that measured mismatch, the series' last two coefficients and a rounding allowance of
//! `8 ε Σ|a_k|` (kg/m), times the flight's radians per kg/m held at every instant (both
//! interferometers'): a read is `D(upper) − D(lower)`, so twice what either point may be off. It is
//! an **audit**, not a bound: between the points it is measured at, the series is held by its
//! smoothness only (the far field is analytic on the span, its nearest singularity outside the
//! ellipse of [`NEAR_ELLIPSE`]). It is added to the read's estimate beside the ledger.
//!
//! **What must not be done.** The series holds on its own line and span only: [`Local::at`]
//! reads any other point (another tower's line, a point outside the span) directly, node by node,
//! so it is right everywhere and fast only where it was made for. It is made for a static world:
//! nothing here follows a node that moves.
//!
//! **Where it is kept.** `cav-sim` holds no state between calls (`docs/ARCHITECTURE.md`), so a
//! [`Local`] lives as long as the read of one world contribution (`super::read_world`), and the
//! engine's cache of contributions keeps what it made: the world's contribution is read again only
//! when the scene's world, its towers or their instruments change. Keeping the series itself
//! across such changes (and for the reading under the playhead, which reads every source again) is
//! the engine's, in `plan/IDEAS.md`.
//!
//! Frames and units: the world's frame (x east, y north, z up), metres, kilograms, kg/m (`V/G`),
//! radians.

use std::f64::consts::PI;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use cav_core::Vec3;
use cav_core::forward::Source;
use cav_core::voxel::summary::Summary;

use super::walk::{Far, Rule};

/// The degree the expansion starts at; it is doubled from here.
pub const FIRST_DEGREE: usize = 16;

/// The highest degree an expansion may have (`docs/slices/S5.md` §3.1, X5): at most
/// `MAX_LOCAL_DEGREE + 1` samples and as many between them are made, each a sum over the far
/// nodes. Past it the expansion is not used and the far nodes are read directly.
pub const MAX_LOCAL_DEGREE: usize = 256;

/// The share of a world read's tolerance the expansion's audit may take; the walk's ledger is held
/// to the rest (`1 − LOCAL_SHARE` of it), so the two together are within the tolerance.
pub const LOCAL_SHARE: f64 = 1.0 / 16.0;

/// A node whose Bernstein ellipse parameter on the span is below this is read directly, not
/// through the series: a series of degree `n` converges about as `ρ^−n`, so at 1.3 a degree of
/// 256 is far below rounding (1.3^−256 ≈ 1e-29) and 128 is about 1e-15.
pub const NEAR_ELLIPSE: f64 = 1.3;

/// The Chebyshev series of a far field along one vertical line.
#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    /// The line's `[x, y]`, metres, the world's frame.
    pub line: [f64; 2],
    /// The span's bottom and top heights, metres, the world's frame.
    pub span: [f64; 2],
    /// The coefficients `a_k` of `D(z) = Σ_k a_k T_k(t)`, `t = (z − z_m)/h` (`z_m` the span's
    /// middle, `h` its half-height), kg/m; `a.len() − 1` is the degree.
    pub coefficients: Vec<f64>,
    /// The largest mismatch between the series and the direct sum at the points between its
    /// samples, kg/m.
    pub mismatch: f64,
    /// `|a_{n−1}| + |a_n|`, kg/m: what the series' tail is estimated at.
    pub tail: f64,
    /// The rounding allowance, `8 ε Σ|a_k|`, kg/m.
    pub rounding: f64,
    /// The audit, radians: twice the largest of the three above, times the flight's radians per
    /// kg/m (the module's documentation).
    pub audit: f64,
}

impl Series {
    /// The degree.
    #[must_use]
    pub fn degree(&self) -> usize {
        self.coefficients.len().saturating_sub(1)
    }

    /// Whether the series holds for the two heights `z` on the vertical line through `at`: the
    /// line's bits the same, and both heights within the span.
    fn holds(&self, at: Vec3, z: [f64; 2]) -> bool {
        at.x.to_bits() == self.line[0].to_bits()
            && at.y.to_bits() == self.line[1].to_bits()
            && z.iter().all(|z| (self.span[0]..=self.span[1]).contains(z))
    }

    /// `D(z)`, kg/m, by Clenshaw's recurrence.
    #[must_use]
    pub fn value(&self, z: f64) -> f64 {
        let middle = 0.5 * (self.span[0] + self.span[1]);
        let half = 0.5 * (self.span[1] - self.span[0]);
        clenshaw(&self.coefficients, (z - middle) / half)
    }
}

/// `Σ_k a_k T_k(t)` by Clenshaw's recurrence.
fn clenshaw(a: &[f64], t: f64) -> f64 {
    let (mut b1, mut b2) = (0.0, 0.0);
    for &c in a.iter().skip(1).rev() {
        let b = 2.0 * t * b1 - b2 + c;
        b2 = b1;
        b1 = b;
    }
    a.first().map_or(0.0, |&a0| t * b1 - b2 + a0)
}

/// What the expansion of one tower's cut found, for the read's record and the bundle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LocalAudit {
    /// The series' degree; 0 when no series is used (no far node, or it could not meet its share).
    pub degree: usize,
    /// Accepted nodes gathered into the series.
    pub far: usize,
    /// Accepted nodes read directly beside it (and all of them when no series is used).
    pub near: usize,
    /// The audit, radians (0 with no series).
    pub audit: f64,
    /// Whether a series was made but not used: at [`MAX_LOCAL_DEGREE`] its audit was still past
    /// its share.
    pub refused: bool,
}

/// A tower's far field: its near nodes read directly and the rest through a [`Series`] on its
/// line, or every node directly when there is no series.
#[derive(Clone, Debug)]
pub struct Local {
    /// The nodes read directly at a point the series holds for.
    pub near: Far,
    /// The nodes the series stands for, kept to read a point it does not hold for.
    pub far: Far,
    /// The series, or `None` when there is none (every node then read directly).
    pub series: Option<Series>,
    /// What was found.
    pub found: LocalAudit,
}

/// How a [`Local`] is made: the tower, its flight's scale and the audit's share.
#[derive(Clone, Copy, Debug)]
pub struct Along {
    /// The tower's base, metres, the world's frame: the bottom of the span.
    pub base: Vec3,
    /// The span's height, metres (the tower's).
    pub height: f64,
    /// Radians the flight makes of 1 kg/m held at every instant, both interferometers'
    /// ([`super::walk::Aim::per`]).
    pub per: f64,
    /// The most the audit may be, radians.
    pub share: f64,
    /// Degrees tried: from [`FIRST_DEGREE`] doubling to this (at most [`MAX_LOCAL_DEGREE`]). A
    /// read uses [`MAX_LOCAL_DEGREE`]; a smaller one is for controls.
    pub most: usize,
    /// When set, the series is made at exactly this degree and used whatever its audit: a
    /// control (`the_local_expansion_is_the_far_field`), never for a read.
    pub forced: Option<usize>,
}

/// The Bernstein ellipse parameter of a node centred at `centre` on the span `[z0, z1]` of the
/// vertical line through `line`: the sum of the semi-axes of the ellipse with foci at the span's
/// ends through the node's singularity, over the span's half-height. Infinite for a span of no
/// height.
fn ellipse(centre: Vec3, line: [f64; 2], span: [f64; 2]) -> f64 {
    let half = 0.5 * (span[1] - span[0]);
    if half <= 0.0 {
        return f64::INFINITY;
    }
    let rho = (centre.x - line[0]).hypot(centre.y - line[1]);
    let dz = [centre.z - span[0], centre.z - span[1]];
    // The distances from the singularity `z = c_z + iρ` to the two foci; their sum is the
    // ellipse's major axis.
    let sum = dz[0].hypot(rho) + dz[1].hypot(rho);
    let a = 0.5 * sum / half;
    a + (a * a - 1.0).max(0.0).sqrt()
}

impl Local {
    /// The far field of `nodes` (a cut's accepted nodes, read by `rule`) along the tower `along`
    /// describes: split into near and far by [`NEAR_ELLIPSE`], the far ones gathered into a series
    /// by the module's rule, on [`crate::read::threads`] threads (each sample a sum over the nodes
    /// in their order: the same bits on any number of threads).
    #[must_use]
    pub fn gather(nodes: &[Summary], rule: Rule, along: &Along) -> Self {
        let line = [along.base.x, along.base.y];
        let span = [along.base.z, along.base.z + along.height];
        let (near, far): (Vec<Summary>, Vec<Summary>) =
            nodes.iter().partition(|n| ellipse(n.centre, line, span) < NEAR_ELLIPSE);
        let (near, far) = (Far { nodes: near, rule }, Far { nodes: far, rule });
        let direct = |near: Far, far: Far, refused: bool| {
            let found =
                LocalAudit { near: near.nodes.len() + far.nodes.len(), refused, ..LocalAudit::default() };
            Self { near, far, series: None, found }
        };
        if far.nodes.is_empty() || along.height <= 0.0 {
            return direct(near, far, false);
        }
        let most = along.most.clamp(1, MAX_LOCAL_DEGREE);
        let middle = 0.5 * (span[0] + span[1]);
        let half = 0.5 * along.height;
        let nodes = &far.nodes;
        // D(t): every far node's (V(z) − V(z_m))/G, each without cancellation.
        let d = |t: f64| -> f64 {
            let z = middle + half * t;
            nodes
                .iter()
                .map(|n| {
                    let across = [line[0] - n.centre.x, line[1] - n.centre.y];
                    rule.difference(n, across, middle - n.centre.z, z - n.centre.z, middle - z)
                })
                .sum()
        };
        let lobatto = |n: usize| -> Vec<f64> { (0..=n).map(|j| (PI * j as f64 / n as f64).cos()).collect() };
        let between = |n: usize| -> Vec<f64> {
            (0..n).map(|j| (PI * (2 * j + 1) as f64 / (2 * n) as f64).cos()).collect()
        };
        let mut degree = along.forced.unwrap_or(FIRST_DEGREE).clamp(1, MAX_LOCAL_DEGREE);
        // f[j] at t = cos(π j / n), kept as n doubles.
        let mut samples = sample(&lobatto(degree), &d);
        loop {
            let coefficients = lobatto_coefficients(&samples);
            let points = between(degree);
            let midway = sample(&points, &d);
            let mismatch = points
                .iter()
                .zip(&midway)
                .map(|(&t, &f)| (clenshaw(&coefficients, t) - f).abs())
                .fold(0.0, f64::max);
            let tail = coefficients[degree].abs() + coefficients[degree - 1].abs();
            let rounding = 8.0 * f64::EPSILON * coefficients.iter().map(|a| a.abs()).sum::<f64>();
            let audit = 2.0 * mismatch.max(tail).max(rounding) * along.per;
            let fits = along.forced.is_some() || audit <= along.share;
            if fits || degree * 2 > most {
                if !fits {
                    return direct(near, far, true);
                }
                let series = Series { line, span, coefficients, mismatch, tail, rounding, audit };
                let found = LocalAudit {
                    degree: series.degree(),
                    far: far.nodes.len(),
                    near: near.nodes.len(),
                    audit,
                    refused: false,
                };
                return Self { near, far, series: Some(series), found };
            }
            // The next degree's samples: the old ones in its even places, the points between in
            // its odd places.
            let mut next = Vec::with_capacity(2 * degree + 1);
            for j in 0..degree {
                next.push(samples[j]);
                next.push(midway[j]);
            }
            next.push(samples[degree]);
            samples = next;
            degree *= 2;
        }
    }

    /// `(V(upper) − V(lower))/G`, kg/m, of every node at the vertical line through `tower`
    /// between heights `height + lower` and `height + upper` (as [`Far::at`]): through the series
    /// where it holds, node by node elsewhere.
    #[must_use]
    pub fn at(&self, tower: Vec3, height: f64, lower: f64, upper: f64) -> f64 {
        let near = self.near.at(tower, height, lower, upper);
        let z = [tower.z + height + lower, tower.z + height + upper];
        match &self.series {
            Some(series) if series.holds(tower, z) => near + (series.value(z[1]) - series.value(z[0])),
            _ => near + self.far.at(tower, height, lower, upper),
        }
    }

    /// How many kernel evaluations' worth one point pair costs: each node read directly four
    /// (as the read counts a node by its moments), the series one for each 16 of its terms.
    #[must_use]
    pub fn work(&self) -> u64 {
        let direct = self.near.nodes.len() + if self.series.is_some() { 0 } else { self.far.nodes.len() };
        let series = self.series.as_ref().map_or(0, |s| s.coefficients.len().div_ceil(16));
        4 * direct as u64 + series as u64
    }
}

impl From<Far> for Local {
    /// Every node of `far` read directly, with no series.
    fn from(far: Far) -> Self {
        let found = LocalAudit { near: far.nodes.len(), ..LocalAudit::default() };
        Self { near: far, far: Far { nodes: Vec::new(), rule: Rule::Octupole }, series: None, found }
    }
}

impl Source for Local {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, _time: f64) -> [f64; 2] {
        heights.map(|height| self.at(tower, height, lower, upper))
    }
}

/// `f(t)` at each of `points`, on [`crate::read::threads`] threads, in order.
fn sample(points: &[f64], f: &(dyn Fn(f64) -> f64 + Sync)) -> Vec<f64> {
    let next = AtomicUsize::new(0);
    let out: Vec<Mutex<f64>> = points.iter().map(|_| Mutex::new(0.0)).collect();
    let work = || {
        loop {
            let at = next.fetch_add(1, Ordering::Relaxed);
            let Some(&t) = points.get(at) else { break };
            *out[at].lock().unwrap_or_else(PoisonError::into_inner) = f(t);
        }
    };
    let workers = crate::read::threads().clamp(1, points.len().max(1));
    std::thread::scope(|scope| {
        for _ in 1..workers {
            scope.spawn(work);
        }
        work();
    });
    out.into_iter().map(|m| m.into_inner().unwrap_or_else(PoisonError::into_inner)).collect()
}

/// The coefficients of the polynomial of degree `n` through `f[j]` at `t_j = cos(π j / n)`,
/// `j = 0 … n`, as `Σ_k a_k T_k(t)`: `a_k = (2/n) Σ''_j f_j cos(π j k / n)`, with the first and
/// last terms of the sum halved and `a_0`, `a_n` halved again (the discrete cosine transform of
/// the first kind).
fn lobatto_coefficients(f: &[f64]) -> Vec<f64> {
    let n = f.len() - 1;
    (0..=n)
        .map(|k| {
            let mut sum = 0.0;
            for (j, &v) in f.iter().enumerate() {
                let weight = if j == 0 || j == n { 0.5 } else { 1.0 };
                // cos(π j k / n) by the index's residue, so that the table is exact at the
                // quarter turns.
                let angle = PI * ((j * k) % (2 * n)) as f64 / n as f64;
                sum += weight * v * angle.cos();
            }
            let edge = if k == 0 || k == n { 0.5 } else { 1.0 };
            edge * 2.0 * sum / n as f64
        })
        .collect()
}

#[cfg(test)]
mod tests;
