//! The cell kernel: the potential and gradient tensor of a homogeneous cube, per unit mass.
//!
//! A grid cell is a cube of matter, not a point. Far away the difference is tiny — a cube has no
//! dipole, quadrupole or octupole, so a point at its centre is wrong only at the hexadecapole,
//! `O((h/d)⁴)`. Near the cell it matters, and a fluid cell can sit right against the atom path (or
//! contain it), where a point element is singular. So within [`NEAR`] cell sizes the closed-form
//! prism potential (Nagy 2000) is used, which is finite everywhere including inside the cube; beyond
//! it, the point law. The two agree to better than `1e-5` at the switch (`cube_matches_point_far`).

use gravity::G;
use math::{Mat3, Vec3};

/// Switch radius in cell sizes: prism formula inside `NEAR·h`, point law outside.
pub const NEAR: f64 = 8.0;

/// `ln(z + r)` evaluated without cancellation for `z < 0`; `None` on the line `x = y = 0, z ≤ 0`
/// where it diverges (its prefactor vanishes there).
fn ln_z_plus_r(x: f64, y: f64, z: f64, r: f64) -> Option<f64> {
    let s = if z >= 0.0 {
        z + r
    } else {
        (x * x + y * y) / (r - z)
    };
    (s > 0.0).then(|| s.ln())
}

/// One corner term of the prism potential.
fn corner_potential(x: f64, y: f64, z: f64) -> f64 {
    let r = (x * x + y * y + z * z).sqrt();
    if r == 0.0 {
        return 0.0;
    }
    let log = |a: f64, b: f64, c: f64| ln_z_plus_r(a, b, c, r).map_or(0.0, |l| a * b * l);
    let atan = |a: f64, b: f64, c: f64| {
        if a == 0.0 {
            0.0
        } else {
            0.5 * a * a * (b * c / (a * r)).atan()
        }
    };
    log(x, y, z) + log(y, z, x) + log(z, x, y) - atan(x, y, z) - atan(y, z, x) - atan(z, x, y)
}

/// The eight corner offsets of the cube relative to the field point, with the alternating sign of
/// the triple bracket `|||·|||`.
fn corners(d: Vec3<f64>, h: f64) -> [(f64, f64, f64, f64); 8] {
    let half = 0.5 * h;
    let mut out = [(0.0, 0.0, 0.0, 0.0); 8];
    let mut n = 0;
    for (sx, x) in [(-1.0, -half - d.x), (1.0, half - d.x)] {
        for (sy, y) in [(-1.0, -half - d.y), (1.0, half - d.y)] {
            for (sz, z) in [(-1.0, -half - d.z), (1.0, half - d.z)] {
                out[n] = (sx * sy * sz, x, y, z);
                n += 1;
            }
        }
    }
    out
}

/// The potential per unit mass of a homogeneous cube of side `h`, at offset `d` from its centre.
/// `V = −G/|d|` in the far field; finite and smooth everywhere, including inside the cube.
pub fn cube_potential(d: Vec3<f64>, h: f64) -> f64 {
    let r2 = d.norm_squared();
    if r2 > (NEAR * h) * (NEAR * h) {
        return -G / r2.sqrt();
    }
    let sum: f64 = corners(d, h)
        .iter()
        .map(|&(s, x, y, z)| s * corner_potential(x, y, z))
        .sum();
    -G * sum / (h * h * h)
}

/// The gradient tensor `Γ = ∇g = −∇∇V` per unit mass of the same cube. Symmetric; trace-free outside
/// the cube and `tr Γ = −4πG/h³` inside it (Poisson, unit mass spread over `h³`).
pub fn cube_gradient_tensor(d: Vec3<f64>, h: f64) -> Mat3<f64> {
    let r2 = d.norm_squared();
    if r2 > (NEAR * h) * (NEAR * h) {
        let r = r2.sqrt();
        let c = -G / (r2 * r);
        let u = [d.x / r, d.y / r, d.z / r];
        let mut m = [[0.0; 3]; 3];
        for (j, row) in m.iter_mut().enumerate() {
            for (k, e) in row.iter_mut().enumerate() {
                let kron = if j == k { 1.0 } else { 0.0 };
                *e = c * (kron - 3.0 * u[j] * u[k]);
            }
        }
        return Mat3 { m };
    }
    // Γ_aa = −(G/h³) |||atan(bc/(a r))|||,  Γ_ab = (G/h³) |||ln(c + r)|||.
    let diag = |a: f64, b: f64, c: f64, r: f64| {
        if a == 0.0 {
            0.0
        } else {
            (b * c / (a * r)).atan()
        }
    };
    let mut m = [[0.0; 3]; 3];
    for &(s, x, y, z) in &corners(d, h) {
        let r = (x * x + y * y + z * z).sqrt();
        if r == 0.0 {
            continue;
        }
        m[0][0] -= s * diag(x, y, z, r);
        m[1][1] -= s * diag(y, z, x, r);
        m[2][2] -= s * diag(z, x, y, r);
        m[0][1] += s * ln_z_plus_r(x, y, z, r).unwrap_or(0.0);
        m[1][2] += s * ln_z_plus_r(y, z, x, r).unwrap_or(0.0);
        m[0][2] += s * ln_z_plus_r(z, x, y, r).unwrap_or(0.0);
    }
    let k = G / (h * h * h);
    m[1][0] = m[0][1];
    m[2][1] = m[1][2];
    m[2][0] = m[0][2];
    for row in &mut m {
        for e in row.iter_mut() {
            *e *= k;
        }
    }
    Mat3 { m }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cube potential by brute-force midpoint quadrature over `n³` sub-cubes (an independent
    /// check of the closed form; converges as `1/n²`).
    fn cube_potential_quadrature(d: Vec3<f64>, h: f64, n: usize) -> f64 {
        let mut acc = 0.0;
        for i in 0..n {
            for j in 0..n {
                for k in 0..n {
                    let q = Vec3::new(
                        ((i as f64 + 0.5) / n as f64 - 0.5) * h,
                        ((j as f64 + 0.5) / n as f64 - 0.5) * h,
                        ((k as f64 + 0.5) / n as f64 - 0.5) * h,
                    );
                    acc += -G / (d - q).norm();
                }
            }
        }
        acc / (n * n * n) as f64
    }

    #[test]
    fn cube_matches_quadrature_near() {
        // Outside the cube but close: the closed form against direct quadrature, at offsets on and
        // off the symmetry planes (the degenerate cases of the corner terms).
        let h = 0.2;
        for d in [
            Vec3::new(0.25, 0.0, 0.0),
            Vec3::new(0.2, 0.2, 0.0),
            Vec3::new(0.17, -0.21, 0.33),
            Vec3::new(0.0, 0.0, -0.4),
            Vec3::new(-0.1, 0.1, 0.6),
        ] {
            let exact = cube_potential(d, h);
            let quad = cube_potential_quadrature(d, h, 60);
            let rel = (exact - quad).abs() / quad.abs();
            assert!(rel <= 2e-5, "d={d:?}: closed {exact:e} vs quadrature {quad:e} ({rel:.1e})");
        }
    }

    #[test]
    fn cube_matches_point_far() {
        // At the switch radius the prism formula and the point law agree, so the kernel is
        // continuous there; and the disagreement falls as (h/d)⁴ (the hexadecapole).
        let h = 0.1;
        let dir = Vec3::new(0.3, -0.5, 0.81);
        let dir = dir.scale(1.0 / dir.norm());
        let mut prev = f64::INFINITY;
        for mult in [2.0, 4.0, 7.99] {
            let d = dir.scale(mult * h);
            let cube = cube_potential(d, h);
            let point = -G / d.norm();
            let rel = (cube - point).abs() / point.abs();
            assert!(rel < prev / 8.0, "error should fall ~16× per doubling: {rel:.2e} after {prev:.2e}");
            prev = rel;
        }
        assert!(prev <= 1e-5, "discontinuity at the switch: {prev:.2e}");
    }

    #[test]
    fn cube_finite_inside_and_on_boundary() {
        // Finite at the centre, on a face, on an edge and at a corner; the centre value is the known
        // closed form for a unit cube, V(0) = −G·M/h · 2.3800772…
        let h = 0.5;
        let centre = cube_potential(Vec3::new(0.0, 0.0, 0.0), h);
        assert!((centre / (-G / h) - 2.380_077_2).abs() <= 1e-6, "centre {centre:e}");
        for d in [
            Vec3::new(0.25, 0.0, 0.0),
            Vec3::new(0.25, 0.25, 0.0),
            Vec3::new(0.25, 0.25, 0.25),
            Vec3::new(0.1, -0.05, 0.2),
        ] {
            assert!(cube_potential(d, h).is_finite(), "non-finite at {d:?}");
        }
    }

    #[test]
    fn gradient_tensor_matches_finite_differences() {
        // Γ = −∇∇V, checked against central second differences of the potential, outside the cube.
        let h = 0.2;
        let e = 1e-3;
        for d in [Vec3::new(0.31, -0.12, 0.27), Vec3::new(0.05, 0.4, -0.3)] {
            let g = cube_gradient_tensor(d, h);
            let v = |dx: f64, dy: f64, dz: f64| cube_potential(d + Vec3::new(dx, dy, dz), h);
            let step = |a: usize, s: f64| {
                let mut o = [0.0; 3];
                o[a] = s;
                o
            };
            let scale = G / d.norm().powi(3);
            for a in 0..3 {
                for b in 0..3 {
                    let (pa, na, pb, nb) = (step(a, e), step(a, -e), step(b, e), step(b, -e));
                    let f = |u: [f64; 3], w: [f64; 3]| v(u[0] + w[0], u[1] + w[1], u[2] + w[2]);
                    let second = (f(pa, pb) - f(pa, nb) - f(na, pb) + f(na, nb)) / (4.0 * e * e);
                    assert!(
                        (g.m[a][b] + second).abs() / scale <= 1e-4,
                        "Γ[{a}][{b}] = {:e}, −∂²V = {:e}",
                        g.m[a][b],
                        -second
                    );
                }
            }
            let trace = g.m[0][0] + g.m[1][1] + g.m[2][2];
            assert!(trace.abs() / scale <= 1e-10, "trace outside the cube: {trace:e}");
        }
    }

    #[test]
    fn gradient_trace_is_poisson_inside() {
        // Inside the cube the trace is −4πGρ with ρ = 1/h³ (unit mass) — the FieldContribution contract.
        let h = 0.3;
        let g = cube_gradient_tensor(Vec3::new(0.04, -0.07, 0.1), h);
        let trace = g.m[0][0] + g.m[1][1] + g.m[2][2];
        let want = -4.0 * std::f64::consts::PI * G / (h * h * h);
        assert!((trace - want).abs() / want.abs() <= 1e-10, "trace {trace:e} vs {want:e}");
    }
}
