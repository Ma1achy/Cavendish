//! Kernel costs for the box-coalescing cost model, on one thread, f64 (the CPU oracle's precision).
//!
//!     cargo run --release -- [evaluations]
//!
//! Times g (per unit Gρ) of: one polyhedron triangle (Werner & Scheeres face form, as mesh_bench);
//! one axis-aligned box by the same face form written for a box (12 edge logarithms, 12 triangle
//! solid angles with the h·2A numerator; the face diagonals cancel and are skipped); a cube's
//! monopole + l = 4 series; a point mass + quadrupole. Before timing, checks the box against the sum
//! of its 12 triangles (pass: 1e-13 relative at R/s ≤ 100). Prints JSON on stdout.

use std::hint::black_box;
use std::time::Instant;

type V = [f64; 3];

fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V, b: V) -> V {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: V) -> f64 {
    dot(a, a).sqrt()
}

struct Tri {
    v: [V; 3],
    n: V,
    twice_area: f64,
    ne: [V; 3],
    el: [f64; 3],
}

impl Tri {
    fn new(v: [V; 3]) -> Self {
        let c = cross(sub(v[1], v[0]), sub(v[2], v[0]));
        let cl = norm(c);
        let n = [c[0] / cl, c[1] / cl, c[2] / cl];
        let mut ne = [[0.0; 3]; 3];
        let mut el = [0.0; 3];
        for i in 0..3 {
            let e = sub(v[(i + 1) % 3], v[i]);
            el[i] = norm(e);
            let k = cross(e, n);
            ne[i] = [k[0] / el[i], k[1] / el[i], k[2] / el[i]];
        }
        Self { v, n, twice_area: cl, ne, el }
    }

    /// This triangle's share of g at `p` (h·2A numerator, log1p edge form).
    fn g(&self, p: V) -> V {
        let r = [sub(self.v[0], p), sub(self.v[1], p), sub(self.v[2], p)];
        let l = [norm(r[0]), norm(r[1]), norm(r[2])];
        let h = dot(self.n, r[0]);
        let num = h * self.twice_area;
        let den = l[0] * l[1] * l[2] + l[0] * dot(r[1], r[2]) + l[1] * dot(r[2], r[0]) + l[2] * dot(r[0], r[1]);
        let omega = 2.0 * num.atan2(den);
        let mut e = 0.0;
        for i in 0..3 {
            let j = (i + 1) % 3;
            e += dot(self.ne[i], r[i]) * (2.0 * self.el[i] / (l[i] + l[j] - self.el[i])).ln_1p();
        }
        let b = e - h * omega;
        [-self.n[0] * b, -self.n[1] * b, -self.n[2] * b]
    }
}

/// Edge logarithm of a segment of length `ell` whose ends are `a` and `b` from the point.
#[inline]
fn edge_log(a: f64, b: f64, ell: f64) -> f64 {
    (2.0 * ell / (a + b - ell)).ln_1p()
}

/// Van Oosterom–Strackee's denominator of a triangle with corner offsets `r` of norms `l`.
#[inline]
fn tri_den(r: [V; 3], l: [f64; 3]) -> f64 {
    l[0] * l[1] * l[2] + l[0] * dot(r[1], r[2]) + l[1] * dot(r[2], r[0]) + l[2] * dot(r[0], r[1])
}

/// g of the box [lo, hi] at `p` (per unit Gρ, pointing at the mass), by the face form.
fn box_g(lo: V, hi: V, p: V) -> V {
    let c = [[lo[0] - p[0], hi[0] - p[0]], [lo[1] - p[1], hi[1] - p[1]], [lo[2] - p[2], hi[2] - p[2]]];
    let len = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    // Corner offsets and norms, index i + 2j + 4k.
    let mut r = [[0.0; 3]; 8];
    let mut l = [0.0; 8];
    for k in 0..2 {
        for j in 0..2 {
            for i in 0..2 {
                let q = [c[0][i], c[1][j], c[2][k]];
                r[i + 2 * j + 4 * k] = q;
                l[i + 2 * j + 4 * k] = norm(q);
            }
        }
    }
    let idx = |i: usize, j: usize, k: usize| i + 2 * j + 4 * k;
    // The 12 edge logarithms, once each: lg[axis][u][v] for the edge along `axis` at the corners
    // whose other two indices (in cyclic order axis+1, axis+2) are u, v.
    let mut lg = [[[0.0; 2]; 2]; 3];
    for (ax, lg_ax) in lg.iter_mut().enumerate() {
        let (b, cc) = ((ax + 1) % 3, (ax + 2) % 3);
        for u in 0..2 {
            for v in 0..2 {
                let mut i0 = [0; 3];
                i0[b] = u;
                i0[cc] = v;
                let mut i1 = i0;
                i1[ax] = 1;
                lg_ax[u][v] = edge_log(l[idx(i0[0], i0[1], i0[2])], l[idx(i1[0], i1[1], i1[2])], len[ax]);
            }
        }
    }
    let mut g = [0.0; 3];
    for a in 0..3 {
        let (b, cc) = ((a + 1) % 3, (a + 2) % 3);
        // Two faces normal to axis a, at s = 0 (lo, outward −) and s = 1 (hi, outward +).
        let mut bracket = [0.0; 2];
        for s in 0..2 {
            let corner = |u: usize, v: usize| {
                let mut ijk = [0; 3];
                ijk[a] = s;
                ijk[b] = u;
                ijk[cc] = v;
                idx(ijk[0], ijk[1], ijk[2])
            };
            let mut e = 0.0;
            // Edges along cc at b = u (in-plane outward normal ±b̂): lg[cc][a-index][b-index], since
            // cyclically after cc come a then b. Edges along b at cc = v: lg[b][cc-index][a-index].
            for u in 0..2 {
                e += if u == 1 { c[b][1] } else { -c[b][0] } * lg[cc][s][u];
            }
            for v in 0..2 {
                e += if v == 1 { c[cc][1] } else { -c[cc][0] } * lg[b][v][s];
            }
            let h = if s == 1 { c[a][1] } else { -c[a][0] };
            let num = h * len[b] * len[cc];
            let (p00, p10, p11, p01) = (corner(0, 0), corner(1, 0), corner(1, 1), corner(0, 1));
            // One atan2 for both triangles: arg(D1 + iN) + arg(D2 + iN) = arg of the product.
            let d1 = tri_den([r[p00], r[p10], r[p11]], [l[p00], l[p10], l[p11]]);
            let d2 = tri_den([r[p00], r[p11], r[p01]], [l[p00], l[p11], l[p01]]);
            let w = 2.0 * (num * (d1 + d2)).atan2(d1 * d2 - num * num);
            bracket[s] = e - h * w;
        }
        g[a] = -(bracket[1] - bracket[0]);
    }
    g
}

fn box_tris(lo: V, hi: V) -> Vec<Tri> {
    let mut out = Vec::new();
    for a in 0..3 {
        let (b, cc) = ((a + 1) % 3, (a + 2) % 3);
        for s in 0..2 {
            let pt = |u: usize, v: usize| {
                let mut q = [0.0; 3];
                q[a] = if s == 1 { hi[a] } else { lo[a] };
                q[b] = if u == 1 { hi[b] } else { lo[b] };
                q[cc] = if v == 1 { hi[cc] } else { lo[cc] };
                q
            };
            let quad = [pt(0, 0), pt(1, 0), pt(1, 1), pt(0, 1)];
            let mut t = [Tri::new([quad[0], quad[1], quad[2]]), Tri::new([quad[0], quad[2], quad[3]])];
            if (t[0].n[a] > 0.0) != (s == 1) {
                t = [Tri::new([quad[0], quad[2], quad[1]]), Tri::new([quad[0], quad[3], quad[2]])];
            }
            out.extend(t);
        }
    }
    out
}

/// Cube of side h at c: monopole + l = 4 series (S3's far kernel, g form).
fn cube_series_g(c: V, h: f64, mass: f64, p: V) -> V {
    let x = sub(p, c);
    let r2 = dot(x, x);
    let r = r2.sqrt();
    let inv_r = 1.0 / r;
    let inv3 = inv_r * inv_r * inv_r;
    let s4 = x[0].powi(4) + x[1].powi(4) + x[2].powi(4);
    let a = -(7.0 / 60.0) * (h / 2.0).powi(4) * mass;
    let inv7 = inv3 * inv3 * inv_r;
    let inv9 = inv7 / r2;
    let inv11 = inv9 / r2;
    let mut g = [0.0; 3];
    for k in 0..3 {
        g[k] = -mass * x[k] * inv3 + a * (20.0 * x[k].powi(3) * inv9 - 45.0 * s4 * inv11 * x[k] + 15.0 * x[k] * inv7);
    }
    g
}

/// Point mass + quadrupole (second moment `m2`, symmetric) at c.
fn point_quad_g(c: V, mass: f64, m2: [f64; 6], p: V) -> V {
    let r = sub(c, p);
    let r2 = dot(r, r);
    let rl = r2.sqrt();
    let i5 = 1.0 / (r2 * r2 * rl);
    let i7 = i5 / r2;
    let tr = m2[0] + m2[1] + m2[2];
    let ir = [
        m2[0] * r[0] + m2[3] * r[1] + m2[4] * r[2],
        m2[3] * r[0] + m2[1] * r[1] + m2[5] * r[2],
        m2[4] * r[0] + m2[5] * r[1] + m2[2] * r[2],
    ];
    let rir = dot(r, ir);
    let mono = mass * i5 * r2; // mass / r³
    let mut g = [0.0; 3];
    for k in 0..3 {
        let du = 0.5 * (6.0 * ir[k] * i5 - 2.0 * tr * r[k] * i5 - 5.0 * (3.0 * rir - r2 * tr) * i7 * r[k]);
        g[k] = mono * r[k] - du;
    }
    g
}

fn time<F: FnMut(usize) -> V>(n: usize, mut f: F) -> f64 {
    // Best of five runs, ns per evaluation.
    let mut best = f64::INFINITY;
    for _ in 0..5 {
        let t = Instant::now();
        let mut acc = 0.0;
        for i in 0..n {
            let g = f(i);
            acc += g[0] + g[1] + g[2];
        }
        black_box(acc);
        best = best.min(t.elapsed().as_secs_f64() * 1e9 / n as f64);
    }
    best
}

fn main() {
    let n: usize = std::env::args().nth(1).map_or(2_000_000, |s| s.parse().unwrap());
    // Field points: a fixed pseudo-random cloud 0.5–3 m from the origin (no RNG crate).
    let mut s: u64 = 0x2545_f491_4f6c_dd1d;
    let mut rnd = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f64 / (1u64 << 53) as f64
    };
    let pts: Vec<V> = (0..1024)
        .map(|_| {
            let d = [rnd() - 0.5, rnd() - 0.5, rnd() - 0.5];
            let k = (0.5 + 2.5 * rnd()) / norm(d);
            [d[0] * k, d[1] * k, d[2] * k]
        })
        .collect();
    let lo = [-0.137, -0.051, -0.023];
    let hi = [0.163, 0.049, 0.211];
    // Check: box face form against its 12 triangles, near and far.
    let tris = box_tris(lo, hi);
    let mut worst: f64 = 0.0;
    for (k, p0) in pts.iter().enumerate().take(200) {
        let scale = [1.0, 3.0, 10.0][k % 3];
        let p = [p0[0] * scale, p0[1] * scale, p0[2] * scale];
        let gb = box_g(lo, hi, p);
        let mut gt = [0.0; 3];
        for t in &tris {
            let g = t.g(p);
            gt = [gt[0] + g[0], gt[1] + g[1], gt[2] + g[2]];
        }
        worst = worst.max(norm(sub(gb, gt)) / norm(gt));
    }
    let tri = Tri::new([[0.1, 0.02, -0.03], [-0.05, 0.12, 0.01], [0.02, -0.04, 0.09]]);
    let m = |i: usize| pts[i & 1023];
    let t_tri = time(n, |i| tri.g(m(i)));
    let t_box = time(n, |i| box_g(lo, hi, m(i)));
    let t_series = time(n, |i| cube_series_g([0.01, -0.02, 0.03], 0.25, 0.015625, m(i)));
    let m2 = [1e-4, 2e-4, 1.5e-4, 1e-5, -2e-5, 3e-6];
    let t_quad = time(n, |i| point_quad_g([0.01, -0.02, 0.03], 0.015, m2, m(i)));
    let t_atan2 = time(n, |i| {
        let p = m(i);
        [p[0].atan2(p[1]), 0.0, 0.0]
    });
    let t_log1p = time(n, |i| {
        let p = m(i);
        [p[0].abs().ln_1p(), 0.0, 0.0]
    });
    println!(
        "{{\"box_vs_12_triangles_max_rel\": {worst:e}, \"ns_triangle\": {t_tri:.2}, \"ns_box\": {t_box:.2}, \
         \"ns_cube_series_l4\": {t_series:.2}, \"ns_point_quadrupole\": {t_quad:.2}, \"ns_atan2\": {t_atan2:.2}, \
         \"ns_ln_1p\": {t_log1p:.2}, \"evaluations\": {n}}}"
    );
}
