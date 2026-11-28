//! The read's kernels, generic over [`Real`]. Each returns `U = ∫dV/|x − p|` of its element at
//! unit density (m², positive); the repository's `V/G` is `−ρU`.
//!
//! - [`STri`]: one triangle of a soup, Werner & Scheeres' face form with the `h·2A` numerator and
//!   `log1p` edges (`docs/research/exact-skin-and-partial-fill.md` §1.1). Three logs, one atan2,
//!   three square roots per triangle per point.
//! - [`Poly`]: a polyhedron with shared vertices and edges and polygonal faces, the same form;
//!   per point one square root per vertex, one log per edge and **one atan2 per face**: a face's
//!   solid angle is `2 arg Π_t (D_t + i h 2A_t)` over its fan triangles (each factor scaled by
//!   `1/(r₁r₂r₃)`), since the half-angles sum to the face's half solid angle, which lies in
//!   (−π, π] for a point off the face's plane.
//! - [`Mp`]: point mass + central second and third moments at the centroid.
//! - [`Bx`]: an axis-aligned box's (or cube's) series to l = 2, 4, 6, with per-box coefficients
//!   β (`series.py` makes the κ table, `kappa.rs`).

use crate::geom::{cross, dot, norm, sub, unit, Polys, V};
use crate::real::Real;

#[inline(always)]
fn rv<R: Real>(a: V) -> [R; 3] {
    [R::c(a[0]), R::c(a[1]), R::c(a[2])]
}
#[inline(always)]
fn rsub<R: Real>(a: [R; 3], b: [R; 3]) -> [R; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
#[inline(always)]
fn rdot<R: Real>(a: [R; 3], b: [R; 3]) -> R {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// A soup triangle with its normal, in-plane edge normals, edge lengths and twice its area.
#[derive(Clone, Copy, Debug)]
pub struct STri {
    pub v: [V; 3],
    pub n: V,
    pub ne: [V; 3],
    pub el: [f64; 3],
    pub a2: f64,
}

impl STri {
    pub fn new(v: [V; 3]) -> Self {
        let c = cross(sub(v[1], v[0]), sub(v[2], v[0]));
        let a2 = norm(c);
        let n = unit(c);
        let mut ne = [[0.0; 3]; 3];
        let mut el = [0.0; 3];
        for i in 0..3 {
            let e = sub(v[(i + 1) % 3], v[i]);
            el[i] = norm(e);
            ne[i] = unit(cross(e, n));
        }
        Self { v, n, ne, el, a2 }
    }

    /// This triangle's share of U at `p`.
    #[inline(always)]
    pub fn u<R: Real>(&self, p: [R; 3]) -> R {
        let r = [rsub(rv(self.v[0]), p), rsub(rv(self.v[1]), p), rsub(rv(self.v[2]), p)];
        let l = [rdot(r[0], r[0]).sqrt(), rdot(r[1], r[1]).sqrt(), rdot(r[2], r[2]).sqrt()];
        let h = rdot(rv(self.n), r[0]);
        let den = l[0] * l[1] * l[2] + l[0] * rdot(r[1], r[2]) + l[1] * rdot(r[2], r[0]) + l[2] * rdot(r[0], r[1]);
        let omega = R::c(2.0) * (h * R::c(self.a2)).atan2(den);
        let mut s = R::c(0.0);
        for i in 0..3 {
            let j = (i + 1) % 3;
            let el = R::c(self.el[i]);
            s = s + rdot(rv(self.ne[i]), r[i]) * (R::c(2.0) * el / (l[i] + l[j] - el)).ln_1p();
        }
        R::c(0.5) * h * (s - h * omega)
    }
}

/// One face of a [`Poly`]: its normal, its edges (index, start vertex, in-plane outward edge
/// normal) and its fan triangles (corner indices and twice the area).
#[derive(Clone, Debug)]
pub struct PFace {
    pub n: V,
    pub edges: Vec<(u16, u16, V)>,
    pub fan: Vec<([u16; 3], f64)>,
}

/// A polyhedron with shared vertices and edges, for the per-face form with one atan2 per face.
#[derive(Clone, Debug, Default)]
pub struct Poly {
    pub v: Vec<V>,
    pub e: Vec<(u16, u16, f64)>,
    pub f: Vec<PFace>,
}

impl Poly {
    /// From polygons (anticlockwise seen from outside); corners within `tol` are one vertex.
    pub fn from_polys(p: &Polys, tol: f64) -> Self {
        let mut v: Vec<V> = Vec::new();
        let mut idx = |x: V| -> u16 {
            if let Some(i) = v.iter().position(|y| (x[0] - y[0]).abs() <= tol && (x[1] - y[1]).abs() <= tol && (x[2] - y[2]).abs() <= tol) {
                return i as u16;
            }
            v.push(x);
            (v.len() - 1) as u16
        };
        let faces: Vec<Vec<u16>> = p.faces.iter().map(|f| f.iter().map(|&x| idx(x)).collect()).collect();
        Self::from_indexed(v, &faces)
    }

    pub fn from_indexed(v: Vec<V>, faces: &[Vec<u16>]) -> Self {
        let mut e: Vec<(u16, u16, f64)> = Vec::new();
        let mut f = Vec::new();
        for face in faces {
            let k = face.len();
            // Newell normal.
            let mut s = [0.0; 3];
            for t in 1..k - 1 {
                let c = cross(sub(v[face[t] as usize], v[face[0] as usize]), sub(v[face[t + 1] as usize], v[face[0] as usize]));
                s = [s[0] + c[0], s[1] + c[1], s[2] + c[2]];
            }
            if norm(s) == 0.0 {
                continue;
            }
            let n = unit(s);
            let mut edges = Vec::with_capacity(k);
            for i in 0..k {
                let (a, b) = (face[i], face[(i + 1) % k]);
                if a == b {
                    continue;
                }
                let key = (a.min(b), a.max(b));
                let id = match e.iter().position(|x| (x.0, x.1) == key) {
                    Some(id) => id,
                    None => {
                        e.push((key.0, key.1, norm(sub(v[key.1 as usize], v[key.0 as usize]))));
                        e.len() - 1
                    }
                };
                let along = sub(v[b as usize], v[a as usize]);
                edges.push((id as u16, a, unit(cross(along, n))));
            }
            let mut fan = Vec::new();
            for t in 1..k - 1 {
                let c = [face[0], face[t], face[t + 1]];
                let a2 = dot(cross(sub(v[c[1] as usize], v[c[0] as usize]), sub(v[c[2] as usize], v[c[0] as usize])), n);
                if a2 != 0.0 {
                    fan.push((c, a2));
                }
            }
            f.push(PFace { n, edges, fan });
        }
        Self { v, e, f }
    }

    /// From a triangle soup, welding corners within `tol` (each triangle one face).
    pub fn from_soup(tris: &[[V; 3]], tol: f64) -> Self {
        // Weld on a hash of the rounded coordinates (neighbours of the cell too).
        use std::collections::HashMap;
        let q = |x: f64| (x / tol).round() as i64;
        let mut map: HashMap<(i64, i64, i64), u16> = HashMap::new();
        let mut v: Vec<V> = Vec::new();
        let mut faces = Vec::new();
        for t in tris {
            let mut face = Vec::with_capacity(3);
            for &x in t {
                let k = (q(x[0]), q(x[1]), q(x[2]));
                let mut found = None;
                'n: for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            if let Some(&i) = map.get(&(k.0 + dx, k.1 + dy, k.2 + dz)) {
                                found = Some(i);
                                break 'n;
                            }
                        }
                    }
                }
                let i = found.unwrap_or_else(|| {
                    v.push(x);
                    map.insert(k, (v.len() - 1) as u16);
                    (v.len() - 1) as u16
                });
                face.push(i);
            }
            if face[0] != face[1] && face[1] != face[2] && face[0] != face[2] {
                faces.push(face);
            }
        }
        Self::from_indexed(v, &faces)
    }

    /// U at `p`.
    #[inline]
    pub fn u<R: Real>(&self, p: [R; 3], r: &mut Vec<([R; 3], R)>, logs: &mut Vec<R>) -> R {
        r.clear();
        logs.clear();
        for &x in &self.v {
            let d = rsub(rv(x), p);
            r.push((d, rdot(d, d).sqrt()));
        }
        for &(a, b, l) in &self.e {
            let (la, lb) = (r[a as usize].1, r[b as usize].1);
            let l = R::c(l);
            logs.push((R::c(2.0) * l / (la + lb - l)).ln_1p());
        }
        let mut u = R::c(0.0);
        for f in &self.f {
            let n = rv(f.n);
            let h = rdot(n, r[f.edges[0].1 as usize].0);
            let mut s = R::c(0.0);
            for &(id, start, ne) in &f.edges {
                s = s + rdot(rv(ne), r[start as usize].0) * logs[id as usize];
            }
            let (mut re, mut im) = (R::c(1.0), R::c(0.0));
            // One scale per face keeps the product near 1 (f32 would overflow at ~R⁻³ᵏ).
            let sc = {
                let c = f.fan[0].0;
                R::c(1.0) / (r[c[0] as usize].1 * r[c[1] as usize].1 * r[c[2] as usize].1)
            };
            for &(c, a2) in &f.fan {
                let (r1, l1) = r[c[0] as usize];
                let (r2, l2) = r[c[1] as usize];
                let (r3, l3) = r[c[2] as usize];
                let l123 = l1 * l2 * l3;
                let den = (l123 + l1 * rdot(r2, r3) + l2 * rdot(r3, r1) + l3 * rdot(r1, r2)) * sc;
                let num = h * R::c(a2) * sc;
                let (nr, ni) = (re * den - im * num, re * num + im * den);
                re = nr;
                im = ni;
            }
            let omega = R::c(2.0) * im.atan2(re);
            u = u + R::c(0.5) * h * (s - h * omega);
        }
        u
    }

    pub fn counts(&self) -> (usize, usize, usize, usize) {
        (self.v.len(), self.e.len(), self.f.len(), self.f.iter().map(|f| f.fan.len()).sum())
    }
}

/// Moments at the centroid: mass (volume at unit density), centroid, central second (xx yy zz xy xz
/// yz) and third (xxx yyy zzz xxy xxz xyy yyz xzz yzz xyz) moments.
#[derive(Clone, Copy, Debug)]
pub struct Mp {
    pub m: f64,
    /// Dipole ∫ρy about `c` (zero about the centroid; set for raw moments about another centre).
    pub dp: V,
    pub c: V,
    pub i: [f64; 6],
    pub t: [f64; 10],
}

impl Mp {
    /// U through order `order` (0 point, 2 + second moment, 3 + third).
    #[inline(always)]
    pub fn u<R: Real>(&self, p: [R; 3], order: u8) -> R {
        let d = rsub(p, rv(self.c));
        let r2 = rdot(d, d);
        let ir = R::c(1.0) / r2.sqrt();
        let ir2 = ir * ir;
        let mut u = R::c(self.m) * ir;
        if self.dp != [0.0; 3] {
            u = u + rdot(rv(self.dp), d) * ir2 * ir;
        }
        if order >= 2 {
            let i = self.i.map(R::c);
            let tr = i[0] + i[1] + i[2];
            let rir = i[0] * d[0] * d[0] + i[1] * d[1] * d[1] + i[2] * d[2] * d[2]
                + R::c(2.0) * (i[3] * d[0] * d[1] + i[4] * d[0] * d[2] + i[5] * d[1] * d[2]);
            let ir3 = ir2 * ir;
            u = u + R::c(0.5) * (R::c(3.0) * rir * ir2 - tr) * ir3;
            if order >= 3 {
                let t = self.t.map(R::c);
                let (x, y, z) = (d[0], d[1], d[2]);
                // T:RRR with multiplicities, and t_i = Σ_j T_ijj.
                let trrr = t[0] * x * x * x + t[1] * y * y * y + t[2] * z * z * z
                    + R::c(3.0) * (t[3] * x * x * y + t[4] * x * x * z + t[5] * x * y * y + t[6] * y * y * z + t[7] * x * z * z + t[8] * y * z * z)
                    + R::c(6.0) * t[9] * x * y * z;
                let tx = t[0] + t[5] + t[7];
                let ty = t[3] + t[1] + t[8];
                let tz = t[4] + t[6] + t[2];
                let rt = x * tx + y * ty + z * tz;
                // (1/6)(15 T:RRR − 9 R² R·t)/R⁷
                u = u + (R::c(2.5) * trrr * ir2 - R::c(1.5) * rt) * ir3 * ir2;
            }
        }
        u
    }
}

/// An axis-aligned box's series: mass, centre, per-box coefficients β of `Σ_f β_f R^f / R^(2l+1)`
/// for l = 2 (3), 4 (6), 6 (10), over the even monomials in `kappa::MONO`.
#[derive(Clone, Copy, Debug)]
pub struct Bx {
    pub m: f64,
    pub c: V,
    pub b2: [f64; 3],
    pub b4: [f64; 6],
    pub b6: [f64; 10],
}

impl Bx {
    pub fn new(m: f64, c: V, half: V) -> Self {
        let (b2, b4, b6) = crate::kappa::betas(half);
        Self { m, c, b2, b4, b6 }
    }

    /// U through order `lmax` (0, 2, 4 or 6). The l = 4 and 6 monomials reuse x², y², z².
    #[inline(always)]
    pub fn u<R: Real>(&self, p: [R; 3], lmax: u8) -> R {
        let d = rsub(p, rv(self.c));
        let (x2, y2, z2) = (d[0] * d[0], d[1] * d[1], d[2] * d[2]);
        let r2 = x2 + y2 + z2;
        let ir = R::c(1.0) / r2.sqrt();
        let ir2 = ir * ir;
        let mut s = R::c(0.0);
        let mut irl = ir2 * ir2 * ir; // 1/R^5
        if lmax >= 2 {
            let b = self.b2.map(R::c);
            s = s + (b[0] * x2 + b[1] * y2 + b[2] * z2) * irl;
        }
        if lmax >= 4 {
            irl = irl * ir2 * ir2; // 1/R^9
            let b = self.b4.map(R::c);
            let p4 = b[0] * x2 * x2 + b[1] * x2 * y2 + b[2] * x2 * z2 + b[3] * y2 * y2 + b[4] * y2 * z2 + b[5] * z2 * z2;
            s = s + p4 * irl;
        }
        if lmax >= 6 {
            irl = irl * ir2 * ir2; // 1/R^13
            let b = self.b6.map(R::c);
            let (x4, y4, z4) = (x2 * x2, y2 * y2, z2 * z2);
            let p6 = b[0] * x4 * x2 + b[1] * x4 * y2 + b[2] * x4 * z2 + b[3] * x2 * y4 + b[4] * x2 * y2 * z2 + b[5] * x2 * z4
                + b[6] * y4 * y2 + b[7] * y4 * z2 + b[8] * y2 * z4 + b[9] * z4 * z2;
            s = s + p6 * irl;
        }
        R::c(self.m) * (ir + s)
    }
}
