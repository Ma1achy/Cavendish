//! The real-read study's field kernels, compiled: the same formulas as the shapes study's numpy
//! (`skin_bench.polyhedron_field(stable = True)`, `shapes_lib.g_points`, `g_quadrupole`,
//! `g_series4`), run on four threads so that ~10⁵ evaluation points per pass are affordable.
//!
//! Every quantity is per unit G·ρ (masses are volumes, m³): U = ∫dV/r (positive, m²) and g = ∇U
//! (m, pointing at the mass), at points in the body's frame (m). Files are raw little-endian f64.
//!
//!     kern tri    <tris N×9> <weights N×W> <W> <points M×3> <out W×M×4>
//!     kern points <pos C×3> <mass C> <second C×9 | -> <points M×3> <out 2×M×4>
//!     kern series <centres C×3> <mass C> <h> <points M×3> <out M×4>
//!
//! `out` rows are (U, g_x, g_y, g_z). `points` writes the point masses' field, then the second
//! moments' extra field (zero when `-`). `series` is a uniform cube of side h as its monopole + l = 4
//! series (Cavendish's far kernel). The threads split the points; nothing here allocates per pair.

use std::env;
use std::fs;
use std::thread;

const THREADS: usize = 4;

fn read(path: &str) -> Vec<f64> {
    let b = fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    b.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect()
}

fn write(path: &str, v: &[f64]) {
    let mut b = Vec::with_capacity(v.len() * 8);
    for x in v {
        b.extend_from_slice(&x.to_le_bytes());
    }
    fs::write(path, b).unwrap_or_else(|e| panic!("{path}: {e}"));
}

type V3 = [f64; 3];
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}

/// One triangle, prepared: vertices, unit normal, twice its area, and per edge its length and the
/// in-plane outward unit normal.
struct Tri {
    v: [V3; 3],
    n: V3,
    twice_area: f64,
    el: [f64; 3],
    ne: [V3; 3],
}

fn prepare(t: &[f64]) -> Tri {
    let v = [[t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], t[8]]];
    let n0 = cross(sub(v[1], v[0]), sub(v[2], v[0]));
    let ta = norm(n0);
    let n = [n0[0] / ta, n0[1] / ta, n0[2] / ta];
    let mut el = [0.0; 3];
    let mut ne = [[0.0; 3]; 3];
    for i in 0..3 {
        let e = sub(v[(i + 1) % 3], v[i]);
        el[i] = norm(e);
        let c = cross(e, n);
        ne[i] = [c[0] / el[i], c[1] / el[i], c[2] / el[i]];
    }
    Tri { v, n, twice_area: ta, el, ne }
}

/// (½ h·bracket, −bracket·n) of one triangle at point p: its share of U and g (Werner & Scheeres,
/// the solid angle's numerator as h·2A, as `polyhedron_field(stable = True)`).
#[inline]
fn tri_field(t: &Tri, p: V3) -> (f64, V3) {
    let r = [sub(t.v[0], p), sub(t.v[1], p), sub(t.v[2], p)];
    let l = [norm(r[0]), norm(r[1]), norm(r[2])];
    let h = dot(t.n, r[0]);
    let triple = h * t.twice_area;
    let den = l[0] * l[1] * l[2] + l[0] * dot(r[1], r[2]) + l[1] * dot(r[2], r[0]) + l[2] * dot(r[0], r[1]);
    let omega = 2.0 * triple.atan2(den);
    let mut edges = 0.0;
    for i in 0..3 {
        let j = (i + 1) % 3;
        let ab = l[i] + l[j];
        let big_l = (2.0 * t.el[i] / (ab - t.el[i])).ln_1p();
        edges += dot(t.ne[i], r[i]) * big_l;
    }
    let bracket = edges - h * omega;
    (0.5 * h * bracket, [-bracket * t.n[0], -bracket * t.n[1], -bracket * t.n[2]])
}

/// Runs `f(point index) -> row` over all points on THREADS threads; rows of `width` values.
fn par_points<F>(m: usize, width: usize, f: F) -> Vec<f64>
where
    F: Fn(usize, &mut [f64]) + Sync,
{
    let mut out = vec![0.0; m * width];
    // KERN_THREADS overrides the default (several study processes at once share the cap).
    let threads = env::var("KERN_THREADS").ok().and_then(|s| s.parse().ok()).unwrap_or(THREADS).max(1);
    let per = m.div_ceil(threads).max(1);
    thread::scope(|s| {
        for (k, chunk) in out.chunks_mut(per * width).enumerate() {
            let f = &f;
            s.spawn(move || {
                for (i, row) in chunk.chunks_mut(width).enumerate() {
                    f(k * per + i, row);
                }
            });
        }
    });
    out
}

fn point(p: &[f64], i: usize) -> V3 {
    [p[3 * i], p[3 * i + 1], p[3 * i + 2]]
}

fn main() {
    let a: Vec<String> = env::args().collect();
    match a[1].as_str() {
        "tri" => {
            let tris = read(&a[2]);
            let w = read(&a[3]);
            let nw: usize = a[4].parse().unwrap();
            let pts = read(&a[5]);
            let n = tris.len() / 9;
            let m = pts.len() / 3;
            assert_eq!(w.len(), n * nw);
            let prepared: Vec<Tri> = tris.chunks_exact(9).map(prepare).collect();
            // Rows of (U, g) for every weighting at one point: W×4 values.
            let rows = par_points(m, 4 * nw, |i, row| {
                let p = point(&pts, i);
                for (k, t) in prepared.iter().enumerate() {
                    let (u, g) = tri_field(t, p);
                    for v in 0..nw {
                        let wv = w[k * nw + v];
                        if wv != 0.0 {
                            row[4 * v] += wv * u;
                            row[4 * v + 1] += wv * g[0];
                            row[4 * v + 2] += wv * g[1];
                            row[4 * v + 3] += wv * g[2];
                        }
                    }
                }
            });
            // Reorder to W×M×4.
            let mut out = vec![0.0; nw * m * 4];
            for i in 0..m {
                for v in 0..nw {
                    for c in 0..4 {
                        out[(v * m + i) * 4 + c] = rows[i * 4 * nw + 4 * v + c];
                    }
                }
            }
            write(&a[6], &out);
        }
        "points" => {
            let pos = read(&a[2]);
            let mass = read(&a[3]);
            let second = if a[4] == "-" { Vec::new() } else { read(&a[4]) };
            let pts = read(&a[5]);
            let c_n = mass.len();
            let m = pts.len() / 3;
            let rows = par_points(m, 8, |i, row| {
                let p = point(&pts, i);
                for c in 0..c_n {
                    let r = sub(point(&pos, c), p); // centroid − point
                    let r2 = dot(r, r);
                    let rl = r2.sqrt();
                    let inv3 = 1.0 / (r2 * rl);
                    row[0] += mass[c] / rl;
                    for k in 0..3 {
                        row[1 + k] += mass[c] * r[k] * inv3;
                    }
                    if !second.is_empty() {
                        let q = &second[9 * c..9 * c + 9];
                        let tr = q[0] + q[4] + q[8];
                        let ir = [
                            q[0] * r[0] + q[1] * r[1] + q[2] * r[2],
                            q[3] * r[0] + q[4] * r[1] + q[5] * r[2],
                            q[6] * r[0] + q[7] * r[1] + q[8] * r[2],
                        ];
                        let rir = dot(r, ir);
                        let inv5 = inv3 / r2;
                        let inv7 = inv5 / r2;
                        let s = 3.0 * rir - r2 * tr;
                        row[4] += 0.5 * s * inv5;
                        for k in 0..3 {
                            let du = 0.5 * (6.0 * ir[k] * inv5 - 2.0 * tr * r[k] * inv5 - 5.0 * s * inv7 * r[k]);
                            row[5 + k] -= du;
                        }
                    }
                }
            });
            let mut out = vec![0.0; 2 * m * 4];
            for i in 0..m {
                for c in 0..4 {
                    out[i * 4 + c] = rows[i * 8 + c];
                    out[(m + i) * 4 + c] = rows[i * 8 + 4 + c];
                }
            }
            write(&a[6], &out);
        }
        "series" => {
            let cen = read(&a[2]);
            let mass = read(&a[3]);
            let h: f64 = a[4].parse().unwrap();
            let pts = read(&a[5]);
            let c_n = mass.len();
            let m = pts.len() / 3;
            let q = (h / 2.0).powi(4) * (7.0 / 60.0);
            let out = par_points(m, 4, |i, row| {
                let p = point(&pts, i);
                for c in 0..c_n {
                    let x = sub(p, point(&cen, c)); // point − centre
                    let r2 = dot(x, x);
                    let rl = r2.sqrt();
                    let inv3 = 1.0 / (r2 * rl);
                    let inv5 = inv3 / r2;
                    let inv7 = inv5 / r2;
                    let inv9 = inv7 / r2;
                    let inv11 = inv9 / r2;
                    let big_s = x[0].powi(4) + x[1].powi(4) + x[2].powi(4);
                    let aa = -q * mass[c];
                    row[0] += mass[c] / rl + aa * (5.0 * big_s * inv9 - 3.0 * inv5);
                    for k in 0..3 {
                        row[1 + k] += -mass[c] * x[k] * inv3
                            + aa * (20.0 * x[k].powi(3) * inv9 - 45.0 * big_s * inv11 * x[k] + 15.0 * x[k] * inv7);
                    }
                }
            });
            write(&a[6], &out);
        }
        "mpole" => {
            // <centroids C×3> <coef C×46> <points M×3> <out M×3>: U_3, U_4, U_5 summed over cells,
            // U_l = N_l(R)/R^(2l+1), N_l's coefficients on the monomials R^α of degree l, in the
            // order a = l..0, b = l−a..0 (moments.py), already divided by 2, 8, 8.
            let cen = read(&a[2]);
            let coef = read(&a[3]);
            let pts = read(&a[4]);
            let c_n = cen.len() / 3;
            let m = pts.len() / 3;
            assert_eq!(coef.len(), c_n * 46);
            let out = par_points(m, 3, |i, row| {
                let p = point(&pts, i);
                let mut mono = [0.0f64; 46];
                for c in 0..c_n {
                    let r = sub(p, point(&cen, c)); // point − centroid
                    let px = [1.0, r[0], r[0] * r[0], r[0].powi(3), r[0].powi(4), r[0].powi(5)];
                    let py = [1.0, r[1], r[1] * r[1], r[1].powi(3), r[1].powi(4), r[1].powi(5)];
                    let pz = [1.0, r[2], r[2] * r[2], r[2].powi(3), r[2].powi(4), r[2].powi(5)];
                    let mut k = 0;
                    for l in 3..=5usize {
                        for aa in (0..=l).rev() {
                            for bb in (0..=(l - aa)).rev() {
                                mono[k] = px[aa] * py[bb] * pz[l - aa - bb];
                                k += 1;
                            }
                        }
                    }
                    let r2 = dot(r, r);
                    let rl = r2.sqrt();
                    let inv7 = 1.0 / (r2 * r2 * r2 * rl);
                    let inv9 = inv7 / r2;
                    let inv11 = inv9 / r2;
                    let q = &coef[46 * c..46 * c + 46];
                    let mut n3 = 0.0;
                    let mut n4 = 0.0;
                    let mut n5 = 0.0;
                    for j in 0..10 {
                        n3 += q[j] * mono[j];
                    }
                    for j in 10..25 {
                        n4 += q[j] * mono[j];
                    }
                    for j in 25..46 {
                        n5 += q[j] * mono[j];
                    }
                    row[0] += n3 * inv7;
                    row[1] += n4 * inv9;
                    row[2] += n5 * inv11;
                }
            });
            write(&a[5], &out);
        }
        other => panic!("unknown mode {other}"),
    }
}
