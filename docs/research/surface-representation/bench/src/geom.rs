//! Cell geometry in f64: exact pieces' moments, the cube clipped by one or two planes (the PLIC
//! and two-plane pieces), their fits to the exact piece, and the polyhedron layout the kernels read.
//!
//! Coordinates are metres. A cell is `lo + [0, h]³`; plane offsets are in the cell's local frame,
//! about its centre: the material is `n·(x − centre) ≤ α`, with `n` the unit outward normal.

pub type V = [f64; 3];

#[inline(always)]
pub fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
#[inline(always)]
pub fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
#[inline(always)]
pub fn scale(a: V, s: f64) -> V {
    [a[0] * s, a[1] * s, a[2] * s]
}
#[inline(always)]
pub fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
#[inline(always)]
pub fn cross(a: V, b: V) -> V {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
#[inline(always)]
pub fn norm(a: V) -> f64 {
    dot(a, a).sqrt()
}
pub fn unit(a: V) -> V {
    scale(a, 1.0 / norm(a))
}

/// Mass moments of a uniform solid at unit density: volume (m³), centroid (m), central second
/// moments ∫yᵢyⱼ (xx, yy, zz, xy, xz, yz; m⁵) and central third moments ∫yᵢyⱼyₖ (xxx, yyy, zzz,
/// xxy, xxz, xyy, yyz, xzz, yzz, xyz; m⁶).
#[derive(Clone, Copy, Debug, Default)]
pub struct Mom {
    pub vol: f64,
    pub cen: V,
    pub m2: [f64; 6],
    pub m3: [f64; 10],
}

pub const I2: [(usize, usize); 6] = [(0, 0), (1, 1), (2, 2), (0, 1), (0, 2), (1, 2)];
pub const I3: [(usize, usize, usize); 10] =
    [(0, 0, 0), (1, 1, 1), (2, 2, 2), (0, 0, 1), (0, 0, 2), (0, 1, 1), (1, 1, 2), (0, 2, 2), (1, 2, 2), (0, 1, 2)];

/// Index of (i, j) in `I2` order.
pub fn i2(i: usize, j: usize) -> usize {
    let (a, b) = if i <= j { (i, j) } else { (j, i) };
    I2.iter().position(|&p| p == (a, b)).unwrap()
}

/// Moments of the solid bounded by `tris` (closed, outward), by signed tetrahedra from `o`, then
/// shifted to the centroid.
pub fn moments(tris: &[[V; 3]], o: V) -> Mom {
    let (mut vol, mut m1) = (0.0, [0.0; 3]);
    let mut m2 = [[0.0; 3]; 3];
    let mut m3 = [[[0.0; 3]; 3]; 3];
    for t in tris {
        let q = [[0.0; 3], sub(t[0], o), sub(t[1], o), sub(t[2], o)];
        let v = dot(q[1], cross(q[2], q[3])) / 6.0;
        let s = add(add(q[1], q[2]), q[3]);
        vol += v;
        for i in 0..3 {
            m1[i] += v / 4.0 * s[i];
            for j in 0..3 {
                let mut qq = 0.0;
                for c in &q {
                    qq += c[i] * c[j];
                }
                m2[i][j] += v / 20.0 * (qq + s[i] * s[j]);
                for k in 0..3 {
                    let mut a = 0.0;
                    let mut b = 0.0;
                    for c in &q {
                        a += c[i] * c[j] * s[k] + c[i] * c[k] * s[j] + c[j] * c[k] * s[i];
                        b += c[i] * c[j] * c[k];
                    }
                    m3[i][j][k] += v / 120.0 * (s[i] * s[j] * s[k] + a + 2.0 * b);
                }
            }
        }
    }
    let c = scale(m1, 1.0 / vol);
    let mut out = Mom { vol, cen: add(o, c), ..Default::default() };
    for (n, &(i, j)) in I2.iter().enumerate() {
        out.m2[n] = m2[i][j] - vol * c[i] * c[j];
    }
    for (n, &(i, j, k)) in I3.iter().enumerate() {
        out.m3[n] = m3[i][j][k] - c[i] * m2[j][k] - c[j] * m2[i][k] - c[k] * m2[i][j] + 2.0 * vol * c[i] * c[j] * c[k];
    }
    out
}

/// A convex or signed polyhedron as polygons (each anticlockwise seen from outside), metres.
#[derive(Clone, Debug, Default)]
pub struct Polys {
    pub faces: Vec<Vec<V>>,
}

impl Polys {
    /// The cell `lo + [0, h]³`.
    pub fn cube(lo: V, h: f64) -> Self {
        let p = |i: usize, j: usize, k: usize| [lo[0] + i as f64 * h, lo[1] + j as f64 * h, lo[2] + k as f64 * h];
        let f = |a: [usize; 3], b: [usize; 3], c: [usize; 3], d: [usize; 3]| {
            vec![p(a[0], a[1], a[2]), p(b[0], b[1], b[2]), p(c[0], c[1], c[2]), p(d[0], d[1], d[2])]
        };
        Self {
            faces: vec![
                f([0, 0, 0], [0, 1, 0], [1, 1, 0], [1, 0, 0]), // z = lo
                f([0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]), // z = hi
                f([0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]), // y = lo
                f([0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0]), // y = hi
                f([0, 0, 0], [0, 0, 1], [0, 1, 1], [0, 1, 0]), // x = lo
                f([1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]), // x = hi
            ],
        }
    }

    /// Cut to `n·x ≤ d` (n unit, world frame), closing the cut with a cap polygon.
    pub fn clip(&self, n: V, d: f64, tol: f64) -> Self {
        let mut faces = Vec::new();
        let mut cut: Vec<V> = Vec::new();
        for f in &self.faces {
            let mut out = Vec::new();
            let k = f.len();
            for i in 0..k {
                let (a, b) = (f[i], f[(i + 1) % k]);
                let (da, db) = (dot(n, a) - d, dot(n, b) - d);
                if da <= 0.0 {
                    out.push(a);
                }
                if (da < 0.0 && db > 0.0) || (da > 0.0 && db < 0.0) {
                    let t = da / (da - db);
                    let x = add(a, scale(sub(b, a), t));
                    out.push(x);
                    cut.push(x);
                } else if da == 0.0 {
                    cut.push(a);
                }
            }
            let out = dedupe(out, tol);
            if out.len() >= 3 {
                faces.push(out);
            }
        }
        let cut = dedupe_set(cut, tol);
        if cut.len() >= 3 {
            // Order the cap's corners anticlockwise about +n.
            let c = scale(cut.iter().fold([0.0; 3], |s, &x| add(s, x)), 1.0 / cut.len() as f64);
            let u = unit(sub(cut[0], c));
            let w = cross(n, u);
            let mut pts: Vec<(f64, V)> = cut.iter().map(|&x| (dot(sub(x, c), w).atan2(dot(sub(x, c), u)), x)).collect();
            pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            faces.push(pts.into_iter().map(|p| p.1).collect());
        }
        Self { faces }
    }

    /// Volume and centroid (divergence theorem, fans from each face's first corner, tetrahedra
    /// from `o`).
    pub fn vol_cen(&self, o: V) -> (f64, V) {
        let (mut vol, mut m1) = (0.0, [0.0; 3]);
        for f in &self.faces {
            for t in 1..f.len() - 1 {
                let (a, b, c) = (sub(f[0], o), sub(f[t], o), sub(f[t + 1], o));
                let v = dot(a, cross(b, c)) / 6.0;
                vol += v;
                m1 = add(m1, scale(add(add(a, b), c), v / 4.0));
            }
        }
        (vol, add(o, scale(m1, 1.0 / vol.max(1e-300))))
    }

    pub fn tris(&self) -> Vec<[V; 3]> {
        let mut out = Vec::new();
        for f in &self.faces {
            for t in 1..f.len() - 1 {
                out.push([f[0], f[t], f[t + 1]]);
            }
        }
        out
    }

    pub fn vertex_count(&self, tol: f64) -> usize {
        dedupe_set(self.faces.iter().flatten().copied().collect(), tol).len()
    }
}

fn close(a: V, b: V, tol: f64) -> bool {
    (a[0] - b[0]).abs() <= tol && (a[1] - b[1]).abs() <= tol && (a[2] - b[2]).abs() <= tol
}

fn dedupe(p: Vec<V>, tol: f64) -> Vec<V> {
    let mut out: Vec<V> = Vec::with_capacity(p.len());
    for x in p {
        if out.last().is_none_or(|&l| !close(l, x, tol)) {
            out.push(x);
        }
    }
    while out.len() > 1 && close(out[0], *out.last().unwrap(), tol) {
        out.pop();
    }
    out
}

fn dedupe_set(p: Vec<V>, tol: f64) -> Vec<V> {
    let mut out: Vec<V> = Vec::new();
    for x in p {
        if !out.iter().any(|&y| close(x, y, tol)) {
            out.push(x);
        }
    }
    out
}

/// The cell `lo + [0, h]³` cut by planes `(n, α)` (α about the cell centre): their intersection.
pub fn cut_cell(lo: V, h: f64, planes: &[(V, f64)]) -> Polys {
    let centre = add(lo, [h / 2.0; 3]);
    let mut p = Polys::cube(lo, h);
    for &(n, a) in planes {
        p = p.clip(n, a + dot(n, centre), 1e-13 * h);
        if p.faces.is_empty() {
            break;
        }
    }
    p
}

/// The offset α (about the cell centre) with `|cube ∩ {n·y ≤ α}| = target` (m³), by safeguarded
/// Newton with the cap's area as the derivative, and the piece.
pub fn plic_offset(lo: V, h: f64, n: V, target: f64) -> f64 {
    let reach = 0.5 * h * (n[0].abs() + n[1].abs() + n[2].abs());
    let (mut a, mut b) = (-reach, reach);
    let mut x = 0.0;
    for _ in 0..100 {
        let v = cut_cell(lo, h, &[(n, x)]).vol_cen(lo).0;
        let r = v - target;
        if r.abs() <= 1e-15 * h * h * h {
            break;
        }
        if r > 0.0 {
            b = x;
        } else {
            a = x;
        }
        let area = cap_area(lo, h, n, x);
        let nx = if area > 0.0 { x - r / area } else { 0.5 * (a + b) };
        x = if nx > a && nx < b { nx } else { 0.5 * (a + b) };
        if b - a <= 1e-15 * h {
            break;
        }
    }
    x
}

fn cap_area(lo: V, h: f64, n: V, alpha: f64) -> f64 {
    let p = cut_cell(lo, h, &[(n, alpha)]);
    // The cap is the last face when there is a cut.
    p.faces.last().map_or(0.0, |f| {
        let mut s = [0.0; 3];
        for t in 1..f.len().saturating_sub(1) {
            s = add(s, cross(sub(f[t], f[0]), sub(f[t + 1], f[0])));
        }
        0.5 * dot(s, n).abs()
    })
}

/// The unit vector of spherical angles (θ, φ).
pub fn sph(t: f64, p: f64) -> V {
    [t.sin() * p.cos(), t.sin() * p.sin(), t.cos()]
}
pub fn angles(n: V) -> (f64, f64) {
    (n[2].clamp(-1.0, 1.0).acos(), n[1].atan2(n[0]))
}

/// Moment of fluid: the normal whose volume-matched PLIC piece's centroid is nearest `cen`,
/// Levenberg–Marquardt in (θ, φ) from `start`. Returns the normal and the centroid residual (m).
pub fn mof(lo: V, h: f64, target: f64, cen: V, start: V) -> (V, f64) {
    let resid = |n: V| -> V {
        let a = plic_offset(lo, h, n, target);
        let (_, c) = cut_cell(lo, h, &[(n, a)]).vol_cen(lo);
        sub(c, cen)
    };
    let (mut t, mut p) = angles(start);
    let mut r = resid(start);
    let mut e = dot(r, r);
    let mut lam = 1e-3;
    for _ in 0..30 {
        if e.sqrt() < 1e-13 * h {
            break;
        }
        let d = 1e-7;
        let rt = scale(sub(resid(sph(t + d, p)), r), 1.0 / d);
        let rp = scale(sub(resid(sph(t, p + d)), r), 1.0 / d);
        let (a11, a12, a22) = (dot(rt, rt), dot(rt, rp), dot(rp, rp));
        let (g1, g2) = (dot(rt, r), dot(rp, r));
        let mut improved = false;
        for _ in 0..8 {
            let (b11, b22) = (a11 * (1.0 + lam) + 1e-30, a22 * (1.0 + lam) + 1e-30);
            let det = b11 * b22 - a12 * a12;
            let dt = -(b22 * g1 - a12 * g2) / det;
            let dp = -(b11 * g2 - a12 * g1) / det;
            let n2 = sph(t + dt, p + dp);
            let r2 = resid(n2);
            let e2 = dot(r2, r2);
            if e2 < e {
                t += dt;
                p += dp;
                r = r2;
                e = e2;
                lam *= 0.3;
                improved = true;
                break;
            }
            lam *= 10.0;
        }
        if !improved {
            break;
        }
    }
    (sph(t, p), e.sqrt())
}

/// Octahedral encoding of a unit normal into [−1, 1]².
pub fn oct_encode(n: V) -> (f64, f64) {
    let s = n[0].abs() + n[1].abs() + n[2].abs();
    let (x, y, z) = (n[0] / s, n[1] / s, n[2] / s);
    if z >= 0.0 {
        (x, y)
    } else {
        ((1.0 - y.abs()) * x.signum(), (1.0 - x.abs()) * y.signum())
    }
}
pub fn oct_decode(u: f64, v: f64) -> V {
    let z = 1.0 - u.abs() - v.abs();
    let (x, y) = if z >= 0.0 { (u, v) } else { ((1.0 - v.abs()) * u.signum(), (1.0 - u.abs()) * v.signum()) };
    unit([x, y, z])
}
/// Round x ∈ [lo, hi] to one of 2^bits levels.
pub fn quant(x: f64, lo: f64, hi: f64, bits: u32) -> f64 {
    let m = ((1u64 << bits) - 1) as f64;
    let q = ((x - lo) / (hi - lo) * m).round().clamp(0.0, m);
    lo + q / m * (hi - lo)
}

/// A plane stored in `bits` per value (32 = f32 storage, else fixed point): the decoded normal and
/// offset. `ubits`, `abits` let the normal and offset take different widths (the 32-bit packing).
pub fn store_plane(n: V, alpha: f64, h: f64, ubits: u32, abits: u32) -> (V, f64) {
    let (u, v) = oct_encode(n);
    let (uq, vq) = if ubits == 32 { (u as f32 as f64, v as f32 as f64) } else { (quant(u, -1.0, 1.0, ubits), quant(v, -1.0, 1.0, ubits)) };
    let nq = oct_decode(uq, vq);
    let reach = 0.5 * h * (nq[0].abs() + nq[1].abs() + nq[2].abs());
    let aq = if abits == 32 { (alpha / h) as f32 as f64 * h } else { quant(alpha, -reach, reach, abits) };
    (nq, aq)
}

/// Split the surface triangles' unit normals into two area-weighted clusters (2-means). Returns
/// (normal, area-weighted surface centroid, area) per cluster, or one cluster.
pub fn two_clusters(surf: &[[V; 3]]) -> Vec<(V, V, f64)> {
    let info: Vec<(V, V, f64)> = surf
        .iter()
        .map(|t| {
            let c = cross(sub(t[1], t[0]), sub(t[2], t[0]));
            let a = 0.5 * norm(c);
            (scale(c, 0.5 / a.max(1e-300)), scale(add(add(t[0], t[1]), t[2]), 1.0 / 3.0), a)
        })
        .filter(|x| x.2 > 0.0)
        .collect();
    let total: f64 = info.iter().map(|x| x.2).sum();
    let mean = |sel: &dyn Fn(usize) -> bool| -> (V, V, f64) {
        let (mut n, mut c, mut a) = ([0.0; 3], [0.0; 3], 0.0);
        for (k, x) in info.iter().enumerate() {
            if sel(k) {
                n = add(n, scale(x.0, x.2));
                c = add(c, scale(x.1, x.2));
                a += x.2;
            }
        }
        (if norm(n) > 0.0 { unit(n) } else { [0.0, 0.0, 1.0] }, scale(c, 1.0 / a.max(1e-300)), a)
    };
    let one = mean(&|_| true);
    if info.len() < 2 {
        return vec![one];
    }
    let big = info.iter().enumerate().max_by(|a, b| a.1 .2.partial_cmp(&b.1 .2).unwrap()).unwrap().0;
    let mut na = info[big].0;
    let far = info.iter().enumerate().min_by(|a, b| dot(a.1 .0, na).partial_cmp(&dot(b.1 .0, na)).unwrap()).unwrap().0;
    let mut nb = info[far].0;
    let mut side = vec![false; info.len()];
    for _ in 0..12 {
        for (k, x) in info.iter().enumerate() {
            side[k] = dot(x.0, nb) > dot(x.0, na);
        }
        na = mean(&|k| !side[k]).0;
        nb = mean(&|k| side[k]).0;
    }
    let a = mean(&|k| !side[k]);
    let b = mean(&|k| side[k]);
    if a.2 < 0.1 * total || b.2 < 0.1 * total || dot(a.0, b.0) > (20f64).to_radians().cos() {
        return vec![one];
    }
    vec![a, b]
}

/// The two-plane piece from two clusters: convex (cube ∩ H₁ ∩ H₂) or concave
/// (cube ∩ (H₁ ∪ H₂) = cube − cube ∩ H₁ᶜ ∩ H₂ᶜ). Both offsets shift together until the volume is
/// `target`. Returns (signed pieces, concave).
pub fn two_plane(lo: V, h: f64, cl: &[(V, V, f64)], target: f64) -> (Vec<(f64, Polys)>, bool) {
    let centre = add(lo, [h / 2.0; 3]);
    let (n1, q1) = (cl[0].0, sub(cl[0].1, centre));
    let (n2, q2) = (cl[1].0, sub(cl[1].1, centre));
    let concave = dot(n1, sub(q2, q1)) + dot(n2, sub(q1, q2)) > 0.0;
    let (a1, a2) = (dot(n1, q1), dot(n2, q2));
    let build = |d: f64| -> Vec<(f64, Polys)> {
        if concave {
            vec![(1.0, Polys::cube(lo, h)), (-1.0, cut_cell(lo, h, &[(scale(n1, -1.0), -(a1 + d)), (scale(n2, -1.0), -(a2 + d))]))]
        } else {
            vec![(1.0, cut_cell(lo, h, &[(n1, a1 + d), (n2, a2 + d)]))]
        }
    };
    let vol = |p: &[(f64, Polys)]| -> f64 { p.iter().map(|(s, q)| if q.faces.is_empty() { 0.0 } else { s * q.vol_cen(lo).0 }).sum() };
    let (mut a, mut b) = (-2.0 * h, 2.0 * h);
    for _ in 0..80 {
        let m = 0.5 * (a + b);
        if vol(&build(m)) > target {
            b = m;
        } else {
            a = m;
        }
    }
    (build(0.5 * (a + b)), concave)
}
