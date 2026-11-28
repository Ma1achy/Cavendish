//! The cube kernel's series: where its time goes and what three bit-for-bit or near-bit
//! reformulations buy. Run: cargo run --release.
use cav_core::Vec3;
use cav_core::voxel::kernel::{MAX_ORDER, cube_difference, cube_series, order_needed, q};
use std::hint::black_box;
use std::time::Instant;

fn factorial(n: usize) -> f64 {
    (1..=n).fold(1.0, |f, k| f * k as f64)
}
fn mean_power(e: usize) -> f64 {
    if e % 2 == 1 { 0.0 } else { 1.0 / (e as f64 + 1.0) }
}
#[derive(Clone, Copy)]
struct Term {
    powers: [usize; 3],
    coef: f64,
}
/// A copy of cav-core's `q_terms`, so the table is the same in the same order.
fn q_terms(l: usize) -> Vec<Term> {
    let mut terms: Vec<Term> = Vec::new();
    for j in 0..=l / 2 {
        let c = if j % 2 == 0 { 1.0 } else { -1.0 } * factorial(2 * l - 2 * j)
            / (2f64.powi(l as i32) * factorial(j) * factorial(l - j) * factorial(l - 2 * j));
        let m = l - 2 * j;
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

/// The original correction, through cav-core's own `q`.
fn correction_orig(order: usize, side: f64, d: Vec3, distance: f64) -> f64 {
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

/// Compact table: per order, the terms' coefficients and powers as bytes (same order).
struct Compact {
    coef: Vec<Vec<f64>>,
    pw: Vec<Vec<[u8; 3]>>,
}
impl Compact {
    fn new() -> Self {
        let mut coef = vec![Vec::new(); MAX_ORDER + 1];
        let mut pw = vec![Vec::new(); MAX_ORDER + 1];
        for l in (4..=MAX_ORDER).step_by(2) {
            for t in q_terms(l) {
                coef[l].push(t.coef);
                pw[l].push([t.powers[0] as u8, t.powers[1] as u8, t.powers[2] as u8]);
            }
        }
        Self { coef, pw }
    }
    /// The same arithmetic in the same order, the powers made once per point.
    fn correction(&self, order: usize, side: f64, d: Vec3, distance: f64) -> f64 {
        if order < 4 {
            return 0.0;
        }
        let n = d * (1.0 / distance);
        let powers = |x: f64| {
            let mut p = [1.0; MAX_ORDER + 1];
            for k in 1..=MAX_ORDER {
                p[k] = p[k - 1] * x;
            }
            p
        };
        let (px, py, pz) = (powers(n.x), powers(n.y), powers(n.z));
        let ratio = 0.5 * side / distance;
        let mut sum = 0.0;
        let mut scale = ratio.powi(4) / distance;
        for l in (4..=order).step_by(2) {
            let ql: f64 = self.coef[l]
                .iter()
                .zip(&self.pw[l])
                .map(|(c, p)| c * px[p[0] as usize] * py[p[1] as usize] * pz[p[2] as usize])
                .sum();
            sum += scale * ql;
            scale *= ratio * ratio;
        }
        sum
    }
}

/// Q_l as a polynomial in the cubic invariants s2 = Î£ n_iÂ² n_jÂ² (i<j) and s3 = n_xÂ² n_yÂ² n_zÂ²
/// (n a unit vector), fitted by least squares; the monomials s2^i s3^j with 4i + 6j â‰¤ l.
struct Invariant {
    basis: Vec<Vec<(u32, u32)>>,
    coef: Vec<Vec<f64>>,
}
fn rng(state: &mut u64) -> f64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state >> 11) as f64 / (1u64 << 53) as f64
}
fn unit(state: &mut u64) -> Vec3 {
    loop {
        let v = Vec3::new(2.0 * rng(state) - 1.0, 2.0 * rng(state) - 1.0, 2.0 * rng(state) - 1.0);
        let l = v.length();
        if l > 0.1 && l <= 1.0 {
            return v * (1.0 / l);
        }
    }
}
fn invariants(n: Vec3) -> (f64, f64) {
    let (x, y, z) = (n.x * n.x, n.y * n.y, n.z * n.z);
    (x * y + y * z + z * x, x * y * z)
}
impl Invariant {
    fn new() -> Self {
        let mut basis = vec![Vec::new(); MAX_ORDER + 1];
        let mut coef = vec![Vec::new(); MAX_ORDER + 1];
        let mut st = 0x1234_5678_9abc_def1u64;
        let pts: Vec<Vec3> = (0..2000).map(|_| unit(&mut st)).collect();
        for l in (4..=MAX_ORDER).step_by(2) {
            let mut b = Vec::new();
            for i in 0..=(l / 4) as u32 {
                for j in 0..=(l / 6) as u32 {
                    if 4 * i + 6 * j <= l as u32 {
                        b.push((i, j));
                    }
                }
            }
            // Least squares by the normal equations in f64 with Cholesky (small, well scaled).
            let k = b.len();
            let mut ata = vec![0.0; k * k];
            let mut atb = vec![0.0; k];
            for &p in &pts {
                let (s2, s3) = invariants(p);
                let row: Vec<f64> = b.iter().map(|&(i, j)| s2.powi(i as i32) * s3.powi(j as i32)).collect();
                let y = q(l, p);
                for r in 0..k {
                    atb[r] += row[r] * y;
                    for c in 0..k {
                        ata[r * k + c] += row[r] * row[c];
                    }
                }
            }
            // Gaussian elimination with partial pivoting.
            let mut a = ata.clone();
            let mut x = atb.clone();
            for col in 0..k {
                let piv = (col..k).max_by(|&i, &j| a[i * k + col].abs().total_cmp(&a[j * k + col].abs())).unwrap();
                for c in 0..k {
                    a.swap(col * k + c, piv * k + c);
                }
                x.swap(col, piv);
                for r in col + 1..k {
                    let f = a[r * k + col] / a[col * k + col];
                    for c in col..k {
                        a[r * k + c] -= f * a[col * k + c];
                    }
                    x[r] -= f * x[col];
                }
            }
            for col in (0..k).rev() {
                let mut s = x[col];
                for c in col + 1..k {
                    s -= a[col * k + c] * x[c];
                }
                x[col] = s / a[col * k + col];
            }
            basis[l] = b;
            coef[l] = x;
        }
        Self { basis, coef }
    }
    fn ql(&self, l: usize, p2: &[f64; 5], p3: &[f64; 3]) -> f64 {
        let mut s = 0.0;
        for (&(i, j), c) in self.basis[l].iter().zip(&self.coef[l]) {
            s += c * p2[i as usize] * p3[j as usize];
        }
        s
    }
    fn correction(&self, order: usize, side: f64, d: Vec3, distance: f64) -> f64 {
        if order < 4 {
            return 0.0;
        }
        let n = d * (1.0 / distance);
        let (s2, s3) = invariants(n);
        let p2 = [1.0, s2, s2 * s2, s2 * s2 * s2, s2 * s2 * s2 * s2];
        let p3 = [1.0, s3, s3 * s3];
        let ratio = 0.5 * side / distance;
        let mut sum = 0.0;
        let mut scale = ratio.powi(4) / distance;
        for l in (4..=order).step_by(2) {
            sum += scale * self.ql(l, &p2, &p3);
            scale *= ratio * ratio;
        }
        sum
    }
}

fn main() {
    let side = 0.1;
    let compact = Compact::new();
    let inv = Invariant::new();
    let terms: Vec<usize> = (4..=MAX_ORDER).step_by(2).map(|l| q_terms(l).len()).collect();
    println!("terms per Q_l, l = 4..16: {terms:?}; invariant terms: {:?}", (4..=MAX_ORDER).step_by(2).map(|l| inv.basis[l].len()).collect::<Vec<_>>());
    // Check: correction_orig is the kernel's (cube_series is public).
    let mut st = 7u64;
    let mut worst_inv = 0.0f64;
    let mut bits_equal = true;
    for _ in 0..200_000 {
        let n = unit(&mut st);
        let r = side * (3.0 + 400.0 * rng(&mut st));
        let d = n * r;
        for order in (4..=MAX_ORDER).step_by(2) {
            let a = correction_orig(order, side, d, d.length());
            let ser = cube_series(order, 1.0, side, d);
            let via = -(1.0 / d.length() + a);
            assert_eq!(ser.to_bits(), via.to_bits(), "the copy is the kernel's");
            let b = compact.correction(order, side, d, d.length());
            if a.to_bits() != b.to_bits() {
                bits_equal = false;
            }
            let c = inv.correction(order, side, d, d.length());
            // Relative to the monopole 1/R: what the potential carries.
            worst_inv = worst_inv.max((c - a).abs() * d.length());
        }
    }
    println!("compact (powers once a point) bit-for-bit equal to the kernel: {bits_equal}");
    println!("invariant form: worst |difference| relative to the monopole: {worst_inv:.2e}");

    // Timing per band of distance (in sides), the order the kernel takes there.
    let n_pts = 400_000;
    for (lo, hi) in [(6.7, 10.0), (10.0, 30.0), (30.0, 100.0), (100.0, 400.0)] {
        let mut st = 99u64;
        let pts: Vec<Vec3> = (0..n_pts)
            .map(|_| unit(&mut st) * (side * (lo + (hi - lo) * rng(&mut st))))
            .filter(|d| order_needed(side, d.length()).is_some())
            .collect();
        let n_pts = pts.len();
        let orders: Vec<usize> = pts.iter().map(|d| order_needed(side, d.length()).unwrap()).collect();
        let mean_order = orders.iter().sum::<usize>() as f64 / orders.len() as f64;
        let time = |f: &dyn Fn(usize, Vec3) -> f64| {
            let mut best = f64::INFINITY;
            for _ in 0..3 {
                let t = Instant::now();
                let mut s = 0.0;
                for (d, &o) in pts.iter().zip(&orders) {
                    s += f(o, *d);
                }
                black_box(s);
                best = best.min(t.elapsed().as_secs_f64() * 1e9 / n_pts as f64);
            }
            best
        };
        let t0 = time(&|o, d| correction_orig(o, side, d, d.length()));
        let t1 = time(&|o, d| compact.correction(o, side, d, d.length()));
        let t2 = time(&|o, d| inv.correction(o, side, d, d.length()));
        // The whole difference (two points 0.01 m apart vertically), as the read calls it.
        let t3 = {
            let mut best = f64::INFINITY;
            for _ in 0..3 {
                let t = Instant::now();
                let mut s = 0.0;
                for d in &pts {
                    s += cube_difference(2.0, side, [d.x, d.y], d.z, d.z + 0.01, -0.01);
                }
                black_box(s);
                best = best.min(t.elapsed().as_secs_f64() * 1e9 / n_pts as f64);
            }
            best
        };
        println!(
            "{lo:>5}-{hi:<5} sides: mean order {mean_order:4.1}; correction ns/point: kernel {t0:6.1}, powers once {t1:6.1} ({:.1}x), invariants {t2:6.1} ({:.1}x); cube_difference {t3:6.1} ns",
            t0 / t1,
            t0 / t2
        );
    }
    // The near regimes for context: split (3 to 6.7 sides) and prism (under 3).
    for (lo, hi) in [(0.6, 3.0), (3.0, 6.7)] {
        let mut st = 5u64;
        let pts: Vec<Vec3> = (0..100_000)
            .map(|_| unit(&mut st) * (side * (lo + (hi - lo) * rng(&mut st))))
            .collect();
        let t = Instant::now();
        let mut s = 0.0;
        for d in &pts {
            s += cube_difference(2.0, side, [d.x, d.y], d.z, d.z + 0.01, -0.01);
        }
        black_box(s);
        println!("{lo:>5}-{hi:<5} sides: cube_difference {:.0} ns", t.elapsed().as_secs_f64() * 1e9 / 100_000.0);
    }
}
