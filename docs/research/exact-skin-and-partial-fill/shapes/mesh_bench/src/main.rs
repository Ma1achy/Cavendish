//! The direct whole-mesh read, timed: g of a closed triangle mesh at a few evaluation points, summed
//! over every triangle with Werner & Scheeres' per-face closed form (the same per-triangle work as
//! the note's `kbench`: one atan2, three ln_1p, a few dot and cross products), on one thread.
//!
//!     mesh_bench <triangles.f64> [points]
//!
//! The file is the triangles as little-endian f64, 9 per triangle (metres), written by
//! `fullres.py dump`. Prints the triangle count, the time per evaluation point and per triangle per
//! point, and a checksum (g_z summed over the points, per unit Gρ). Not part of the workspace.

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

/// A triangle with its unit normal, edge normals and edge lengths precomputed.
struct Tri {
    v: [V; 3],
    n: V,
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
        Self { v, n, ne, el }
    }

    /// This triangle's share of g at `p` (per unit Gρ, pointing at the mass).
    fn g(&self, p: V) -> V {
        let r = [sub(self.v[0], p), sub(self.v[1], p), sub(self.v[2], p)];
        let l = [norm(r[0]), norm(r[1]), norm(r[2])];
        let h = dot(self.n, r[0]);
        let num = dot(r[0], cross(r[1], r[2]));
        let den = l[0] * l[1] * l[2] + l[0] * dot(r[1], r[2]) + l[1] * dot(r[2], r[0]) + l[2] * dot(r[0], r[1]);
        let omega = 2.0 * num.atan2(den);
        let mut e = 0.0;
        for i in 0..3 {
            let j = (i + 1) % 3;
            e += dot(self.ne[i], r[i]) * (2.0 * self.el[i] / (l[i] + l[j] - self.el[i])).ln_1p();
        }
        let b = e - h * omega;
        [self.n[0] * b, self.n[1] * b, self.n[2] * b]
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&args[1]).expect("read the triangle file");
    let points: usize = args.get(2).map_or(8, |s| s.parse().expect("a count of points"));
    let f: Vec<f64> = bytes.chunks_exact(8).map(|b| f64::from_le_bytes(b.try_into().unwrap())).collect();
    let tris: Vec<Tri> =
        f.chunks_exact(9).map(|t| Tri::new([[t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], t[8]]])).collect();
    drop(f);
    let pts: Vec<V> = (0..points)
        .map(|i| {
            let a = i as f64;
            [1.5 + 0.1 * a.sin(), 0.3 + 0.1 * a.cos(), -0.45 + 0.05 * a]
        })
        .collect();
    let t0 = Instant::now();
    let mut gz = 0.0;
    for &p in &pts {
        let mut g = [0.0; 3];
        for t in &tris {
            let d = t.g(p);
            g = [g[0] + d[0], g[1] + d[1], g[2] + d[2]];
        }
        gz += g[2];
    }
    let s = t0.elapsed().as_secs_f64();
    black_box(gz);
    let per_point = s / points as f64;
    println!(
        "triangles {} points {} ms_per_point {:.3} ns_per_triangle_point {:.2} checksum {:.6e}",
        tris.len(),
        points,
        per_point * 1e3,
        per_point / tris.len() as f64 * 1e9,
        gz
    );
}
