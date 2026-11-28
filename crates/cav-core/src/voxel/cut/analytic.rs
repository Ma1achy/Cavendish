//! The curved solids' pieces from their analytic surfaces: the z-chord exactly, `(x, y)` by
//! Gauss–Legendre on pieces split at every kink, with a cosine map at each piece's ends.
//!
//! Ported from [X] §3.1's `shapes/analytic.py` (`cell_moments`). Each solid here is, in a frame
//! where its axis is z and its centre the origin, "inside where `z ∈ chord(ρ²)`", with
//! `ρ² = x² + y²`. For fixed `(x, y)` the inside part of the cell's z range is an interval, so its
//! length and its z-moments are exact. Over `(x, y)` the integrand has kinks only where the chord
//! starts, ends, or meets the cell's floor or ceiling, which are circles `ρ = ρ_j` about the axis:
//! the x range is split where those circles have their extremes and where they cross the cell's
//! y walls, and at each x node the y range where they cross it. Near a kink the integrand goes as a
//! square root; the cosine map `u = (1 − cos πt)/2` on each piece makes that smooth.

use std::sync::OnceLock;

use crate::Vec3;

use super::super::shape::{Cylinder, Ellipsoid, Shell, Sphere};
use super::super::{Aabb, Grid, VoxelError};
use super::{Borrowed, CellClip, Clip, Raw, Stateless};

/// Gauss–Legendre points a piece, along each of x and y ([X] §3.1's 24).
const POINTS: usize = 24;

/// The work of clipping one cell, in the voxeliser's units: about the integrand's evaluations
/// for a cell crossed by three circles (up to seven x pieces of `POINTS`, each with up to seven y
/// pieces of `POINTS`), a fair upper bound.
const ANALYTIC_WORK: u64 = (7 * POINTS * 7 * POINTS) as u64;

/// The cosine-mapped rule on [0, 1]: nodes `u` and weights.
fn rule() -> &'static [(f64, f64)] {
    static RULE: OnceLock<Vec<(f64, f64)>> = OnceLock::new();
    RULE.get_or_init(|| {
        crate::forward::gauss_legendre(POINTS)
            .into_iter()
            .map(|(x, w)| {
                let t = 0.5 * (x + 1.0);
                let pi = std::f64::consts::PI;
                (0.5 * (1.0 - (pi * t).cos()), 0.5 * w * 0.5 * pi * (pi * t).sin())
            })
            .collect()
    })
}

/// A solid of revolution about z through the origin, by its chord.
trait Radial {
    /// The z interval inside at `ρ²` from the axis, if any.
    fn chord(&self, rho2: f64) -> Option<(f64, f64)>;
    /// The radii where the integrand over `(x, y)` has a kink, for a cell from `z0` to `z1`.
    fn kinks(&self, z0: f64, z1: f64, out: &mut Vec<f64>);
}

/// A ball of `0` radius about the origin.
struct Ball(f64);

impl Radial for Ball {
    fn chord(&self, rho2: f64) -> Option<(f64, f64)> {
        let c = self.0 * self.0 - rho2;
        (c > 0.0).then(|| {
            let s = c.sqrt();
            (-s, s)
        })
    }
    fn kinks(&self, z0: f64, z1: f64, out: &mut Vec<f64>) {
        let r2 = self.0 * self.0;
        out.push(self.0);
        for z in [z0, z1] {
            if z * z < r2 {
                out.push((r2 - z * z).sqrt());
            }
        }
    }
}

/// A cylinder about z: radius, half its length.
struct Rod(f64, f64);

impl Radial for Rod {
    fn chord(&self, rho2: f64) -> Option<(f64, f64)> {
        (rho2 < self.0 * self.0).then_some((-self.1, self.1))
    }
    fn kinks(&self, _z0: f64, _z1: f64, out: &mut Vec<f64>) {
        out.push(self.0);
    }
}

/// Sorted break points in `[a, b]`, its ends included.
fn pieces(a: f64, b: f64, cuts: &mut Vec<f64>) -> Vec<f64> {
    cuts.retain(|c| *c > a && *c < b);
    cuts.sort_by(f64::total_cmp);
    let mut out = Vec::with_capacity(cuts.len() + 2);
    out.push(a);
    out.extend(cuts.iter().copied());
    out.push(b);
    out
}

/// The moments of `solid` inside the box from `lo` to `hi` (the solid's frame: its axis z, its
/// centre the origin), about the box's centre.
fn radial_raw(solid: &dyn Radial, lo: Vec3, hi: Vec3) -> Raw {
    let c = (lo + hi) * 0.5;
    let mut radii = Vec::with_capacity(3);
    solid.kinks(lo.z, hi.z, &mut radii);
    let mut xcuts = Vec::with_capacity(radii.len() * 6);
    for &r in &radii {
        xcuts.push(r);
        xcuts.push(-r);
        for y in [lo.y, hi.y] {
            let d = r * r - y * y;
            if d > 0.0 {
                xcuts.push(d.sqrt());
                xcuts.push(-d.sqrt());
            }
        }
    }
    let xs = pieces(lo.x, hi.x, &mut xcuts);
    let rule = rule();
    // Σ w x'^p y'^q ∫ z'^r, by p·16 + q·4 + r.
    let mut table = [0.0f64; 64];
    let mut ycuts = Vec::with_capacity(radii.len() * 2);
    for pair in xs.windows(2) {
        let (xa, xb) = (pair[0], pair[1]);
        for &(u, wu) in rule {
            let x = xa + (xb - xa) * u;
            let wx = (xb - xa) * wu;
            ycuts.clear();
            for &r in &radii {
                let d = r * r - x * x;
                if d > 0.0 {
                    ycuts.push(d.sqrt());
                    ycuts.push(-d.sqrt());
                }
            }
            let ys = pieces(lo.y, hi.y, &mut ycuts);
            let dx = x - c.x;
            let xp = [1.0, dx, dx * dx, dx * dx * dx];
            for ypair in ys.windows(2) {
                let (ya, yb) = (ypair[0], ypair[1]);
                for &(v, wv) in rule {
                    let y = ya + (yb - ya) * v;
                    let Some((zlo, zhi)) = solid.chord(x * x + y * y) else { continue };
                    let (a, b) = (zlo.max(lo.z) - c.z, zhi.min(hi.z) - c.z);
                    if b <= a {
                        continue;
                    }
                    let w = wx * (yb - ya) * wv;
                    let dy = y - c.y;
                    let yp = [1.0, dy, dy * dy, dy * dy * dy];
                    let (a2, b2) = (a * a, b * b);
                    let zp = [b - a, 0.5 * (b2 - a2), (b2 * b - a2 * a) / 3.0, 0.25 * (b2 * b2 - a2 * a2)];
                    for p in 0..4 {
                        for q in 0..4 - p {
                            let wxy = w * xp[p] * yp[q];
                            for r in 0..4 - p - q {
                                table[p * 16 + q * 4 + r] += wxy * zp[r];
                            }
                        }
                    }
                }
            }
        }
    }
    Raw::from_monomials(|e| table[e[0] * 16 + e[1] * 4 + e[2]])
}

/// `raw` (moments in a frame whose axes are the body's `axes[0]`, `axes[1]`, `axes[2]`, each
/// scaled by `scale`) in the body's frame: monomial `x^e` of the body's frame is the frame's
/// monomial with each exponent moved to its axis, times the scales' powers and their product (the
/// volume's).
fn unmap(raw: &Raw, axes: [usize; 3], scale: [f64; 3]) -> Raw {
    let det = scale[0] * scale[1] * scale[2];
    // The frame's axis that is the body's axis a.
    let mut of = [0; 3];
    for (i, &a) in axes.iter().enumerate() {
        of[a] = i;
    }
    let get = |e: [usize; 3]| -> f64 {
        // e is in the frame's axes: find its value from `raw`'s parts.
        let order = e[0] + e[1] + e[2];
        let mut list = Vec::with_capacity(3);
        for (axis, &n) in e.iter().enumerate() {
            list.extend(std::iter::repeat_n(axis, n));
        }
        match order {
            0 => raw.volume,
            1 => raw.first[list[0]],
            2 => raw.second[super::second_index(list[0], list[1])],
            _ => raw.third[super::third_index(list[0], list[1], list[2])],
        }
    };
    Raw::from_monomials(|e| {
        let mut frame = [0; 3];
        let mut factor = det;
        for a in 0..3 {
            frame[of[a]] = e[a];
            factor *= scale[of[a]].powi(e[a] as i32);
        }
        factor * get(frame)
    })
}

impl Stateless for Sphere {
    fn work(&self, _boxed: &Aabb) -> u64 {
        ANALYTIC_WORK
    }
    fn clip(&self, b: &Aabb) -> Raw {
        radial_raw(&Ball(self.radius), b.lo - self.centre, b.hi - self.centre)
    }
}

impl Stateless for Shell {
    fn work(&self, _boxed: &Aabb) -> u64 {
        2 * ANALYTIC_WORK
    }
    /// The outer ball's piece less the inner's.
    fn clip(&self, b: &Aabb) -> Raw {
        let (lo, hi) = (b.lo - self.centre, b.hi - self.centre);
        let outer = radial_raw(&Ball(self.outer), lo, hi);
        if self.inner > 0.0 { outer.plus(-1.0, &radial_raw(&Ball(self.inner), lo, hi)) } else { outer }
    }
}

impl Stateless for Ellipsoid {
    fn work(&self, _boxed: &Aabb) -> u64 {
        ANALYTIC_WORK
    }
    /// The unit ball in the frame scaled by the semi-axes.
    fn clip(&self, b: &Aabb) -> Raw {
        let r = self.radii;
        let scaled = |p: Vec3| {
            let d = p - self.centre;
            Vec3::new(d.x / r.x, d.y / r.y, d.z / r.z)
        };
        let raw = radial_raw(&Ball(1.0), scaled(b.lo), scaled(b.hi));
        unmap(&raw, [0, 1, 2], [r.x, r.y, r.z])
    }
}

impl Stateless for Cylinder {
    fn work(&self, _boxed: &Aabb) -> u64 {
        ANALYTIC_WORK
    }
    /// The rod about z in the frame whose third axis is the cylinder's.
    fn clip(&self, b: &Aabb) -> Raw {
        let axes = match self.axis {
            0 => [1, 2, 0],
            1 => [0, 2, 1],
            _ => [0, 1, 2],
        };
        let permute = |p: Vec3| {
            let d = (p - self.centre).to_array();
            Vec3::new(d[axes[0]], d[axes[1]], d[axes[2]])
        };
        let raw = radial_raw(&Rod(self.radius, self.half_length), permute(b.lo), permute(b.hi));
        unmap(&raw, axes, [1.0; 3])
    }
}

macro_rules! stateless_clip {
    ($($t:ty),*) => {$(
        impl Clip for $t {
            fn prepare(&self, _grid: &Grid) -> Result<Box<dyn CellClip + '_>, VoxelError> {
                Ok(Box::new(Borrowed(self)))
            }
        }
    )*};
}

stateless_clip!(Sphere, Shell, Ellipsoid, Cylinder);
