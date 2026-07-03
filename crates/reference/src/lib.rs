//! `reference` — independent oracle: a faithful Rust port of George's validation cases.
//!
//! Design & milestone: `milestones/reference-port.md`.
//!
//! Independence is the point: this crate depends on `math` **only** — never `gravity` or
//! `instrument`. It reimplements George's method (solid-cuboid potential by direct 3-D
//! Gauss–Legendre quadrature; his two-segment, floor-clamped, stepped arms; his gradiometer sign
//! `ΔΦ = δφ₁ − δφ₂`) so that agreement with the engine's voxelised, closed-form-arm path is
//! meaningful rather than circular. M1 ports the **static** cuboid → the DC gradiometer phase; the
//! oscillating time-series and its FFT are M2/M6.

use math::Vec3;

/// George's pinned constants (verbatim; see `reference-port.md` §4).
pub mod constants {
    pub const G: f64 = 6.674_30e-11;
    pub const M_ATOM: f64 = 1.46e-25; // ⁸⁷Sr calibration mass
    pub const HBAR: f64 = 1.055e-34;
    pub const WAVELENGTH: f64 = 698e-9;
    pub const N_KICK: f64 = 1000.0;
    pub const T_HALF: f64 = 0.73; // T; 2T is the interrogation time
    pub const G_ACCEL: f64 = 9.81;
    pub const U_INITIAL: f64 = 3.86;
    pub const DT: f64 = 0.01;
    pub const IFO_SEP: f64 = 5.0; // Δr

    /// The recoil / arm-separation velocity, `n·ħk/m_atom`.
    pub fn velocity_boost() -> f64 {
        let k = core::f64::consts::TAU / WAVELENGTH;
        N_KICK * HBAR * k / M_ATOM
    }
}

use constants::*;

/// Gauss–Legendre nodes and weights on `[-1, 1]` (roots of `P_n` by Newton iteration).
fn gauss_legendre(n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut nodes = vec![0.0; n];
    let mut weights = vec![0.0; n];
    for i in 0..n {
        // Chebyshev initial guess for the i-th root, then Newton-refine on P_n.
        let mut x = (core::f64::consts::PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
        loop {
            let (p, dp) = legendre_p_dp(n, x);
            let dx = -p / dp;
            x += dx;
            if dx.abs() < 1e-15 {
                break;
            }
        }
        let (_, dp) = legendre_p_dp(n, x);
        nodes[i] = x;
        weights[i] = 2.0 / ((1.0 - x * x) * dp * dp);
    }
    (nodes, weights)
}

/// The Legendre polynomial `P_n(x)` and its derivative, by the three-term recurrence.
fn legendre_p_dp(n: usize, x: f64) -> (f64, f64) {
    if n == 0 {
        return (1.0, 0.0);
    }
    let mut p_prev = 1.0;
    let mut p = x;
    for k in 2..=n {
        let kf = k as f64;
        let p_next = ((2.0 * kf - 1.0) * x * p - (kf - 1.0) * p_prev) / kf;
        p_prev = p;
        p = p_next;
    }
    let dp = n as f64 * (x * p - p_prev) / (x * x - 1.0);
    (p, dp)
}

/// A homogeneous solid cuboid, evaluated by direct 3-D quadrature.
pub struct Cuboid {
    pub size: [f64; 3],   // full extents (x, y, z)
    pub centre: [f64; 3], // centre in world coordinates
    pub density: f64,     // kg/m³
}

impl Cuboid {
    pub fn total_mass(&self) -> f64 {
        self.density * self.size[0] * self.size[1] * self.size[2]
    }
}

/// A cuboid pre-sampled onto a Gauss–Legendre grid: potential is then a weighted `1/r` sum.
pub struct QuadCuboid {
    points: Vec<Vec3<f64>>,
    weights: Vec<f64>,
    neg_g_rho_jac: f64,
}

impl QuadCuboid {
    /// Sample `wall` with an `n`-node Gauss–Legendre rule per axis.
    pub fn new(wall: &Cuboid, n: usize) -> Self {
        let (nodes, w) = gauss_legendre(n);
        let half = [wall.size[0] * 0.5, wall.size[1] * 0.5, wall.size[2] * 0.5];
        let jac = half[0] * half[1] * half[2];
        let mut points = Vec::with_capacity(n * n * n);
        let mut weights = Vec::with_capacity(n * n * n);
        for (a, &xa) in nodes.iter().enumerate() {
            for (b, &xb) in nodes.iter().enumerate() {
                for (c, &xc) in nodes.iter().enumerate() {
                    points.push(Vec3::new(
                        wall.centre[0] + half[0] * xa,
                        wall.centre[1] + half[1] * xb,
                        wall.centre[2] + half[2] * xc,
                    ));
                    weights.push(w[a] * w[b] * w[c]);
                }
            }
        }
        QuadCuboid {
            points,
            weights,
            neg_g_rho_jac: -G * wall.density * jac,
        }
    }

    /// The potential `V(p) = −Gρ ∫ dV'/|p − r'|` at world point `p`.
    pub fn potential(&self, p: Vec3<f64>) -> f64 {
        let mut s = 0.0;
        for (pt, &w) in self.points.iter().zip(&self.weights) {
            s += w / (p - *pt).norm();
        }
        self.neg_g_rho_jac * s
    }
}

/// George's stepped ballistic arm: free-fall under `−g`, one recoil kick at `T`, clamped at the
/// launch height once the atom has fallen back (`z ≤ z0` with `v < 0`). Sampled at `DT`.
fn integrate_arm(z0: f64, v0: f64, kick_at_t: f64) -> Vec<f64> {
    let mut z = z0;
    let mut v = v0;
    let n = (2.0 * T_HALF / DT).round() as usize;
    let mut path = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let t = i as f64 * DT;
        path.push(z);
        if (t - T_HALF).abs() < 0.5 * DT {
            v += kick_at_t;
        }
        z += v * DT - 0.5 * G_ACCEL * DT * DT;
        v -= G_ACCEL * DT;
        if z <= z0 && v < 0.0 {
            z = z0;
            v = 0.0;
        }
    }
    path
}

/// The static concrete-wall anchor: the DC gradiometer phase for a stationary cuboid.
///
/// Returns George's convention `ΔΦ = δφ₁ − δφ₂` (lower IFO minus upper). The opposite sign to the
/// spec's `δφ₂ − δφ₁` is deliberate; the agreement test reconciles it by magnitude.
pub fn cuboid_dc_gradiometer_phase(wall: &Cuboid, base_z: f64, n_quad: usize) -> f64 {
    let quad = QuadCuboid::new(wall, n_quad);
    let boost = velocity_boost();
    let dphi = |z0: f64| -> f64 {
        let lower = integrate_arm(z0, U_INITIAL, boost);
        let upper = integrate_arm(z0, U_INITIAL + boost, -boost);
        // δφ = (m_A/ħ) Σ [V(z_u) − V(z_l)] dt (rectangle rule over the stepped path).
        let mut acc = 0.0;
        for i in 0..lower.len() {
            acc += quad.potential(Vec3::new(0.0, 0.0, upper[i]))
                - quad.potential(Vec3::new(0.0, 0.0, lower[i]));
        }
        (M_ATOM / HBAR) * acc * DT
    };
    let dphi_1 = dphi(base_z); // lower interferometer
    let dphi_2 = dphi(base_z + IFO_SEP); // upper interferometer
    dphi_1 - dphi_2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ref_cuboid_quadrature() {
        // Independent sanity on the quadrature: far from the box the potential must approach the
        // point-mass monopole −GM/r, and the rule must be converged at n = 24. (The full Nagy
        // closed-form cross-check is the M2 `shape` oracle.)
        let wall = Cuboid {
            size: [0.225, 6.1, 12.2],
            centre: [0.0, 0.0, 0.0],
            density: 2400.0,
        };
        let m = wall.total_mass();
        let far = Vec3::new(200.0, 0.0, 0.0);
        let r = far.norm();
        let v = QuadCuboid::new(&wall, 24).potential(far);
        assert!(
            (v - (-G * m / r)).abs() / (G * m / r) <= 1e-3,
            "monopole limit"
        );

        // Convergence: n = 16 and n = 24 agree at a modest standoff.
        let near = Vec3::new(3.0, 0.0, 0.0);
        let v16 = QuadCuboid::new(&wall, 16).potential(near);
        let v24 = QuadCuboid::new(&wall, 24).potential(near);
        assert!(
            (v16 - v24).abs() / v24.abs() <= 1e-6,
            "quadrature not converged"
        );
    }
}
