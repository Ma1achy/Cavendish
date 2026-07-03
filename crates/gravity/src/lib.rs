//! `gravity` — the gravity kernel: `Cloud`, the point-element law, and potential/field/gradient-tensor.
//!
//! Design: `design/gravity.md`. Milestone: `milestones/M1-physics-spine.md`.
//!
//! The kernel is generic over [`math::Scalar`] (the field point lifts to `S`; cloud coordinates are
//! `f64` lifted via `from_f64`), so `f64` today and a forward-mode `Dual` at M8 flow through the same
//! code. The gradient tensor is formed **analytically** per element, so `Γ = Γᵀ` and `tr Γ = 0` hold
//! by construction (spec `INV.1`) — never by finite-differencing the potential.

use math::{Isometry3, Mat3, Scalar, Vec3};

/// Newton's constant (spec `tab:params`).
pub const G: f64 = 6.674_30e-11;

/// A discretised mass distribution: point elements, structure-of-arrays, in world coordinates.
#[derive(Clone, Debug, Default)]
pub struct Cloud {
    pub xs: Vec<f64>,
    pub ys: Vec<f64>,
    pub zs: Vec<f64>,
    pub ms: Vec<f64>,
}

impl Cloud {
    /// Build a cloud from `(x, y, z, m)` elements.
    pub fn from_elements(elements: &[(f64, f64, f64, f64)]) -> Self {
        let mut c = Cloud::default();
        for &(x, y, z, m) in elements {
            c.xs.push(x);
            c.ys.push(y);
            c.zs.push(z);
            c.ms.push(m);
        }
        c
    }

    pub fn len(&self) -> usize {
        self.ms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ms.is_empty()
    }

    pub fn total_mass(&self) -> f64 {
        self.ms.iter().sum()
    }

    /// A copy with every element mapped through the rigid transform (world ← body).
    pub fn transformed(&self, iso: &Isometry3) -> Cloud {
        let mut c = Cloud {
            xs: Vec::with_capacity(self.len()),
            ys: Vec::with_capacity(self.len()),
            zs: Vec::with_capacity(self.len()),
            ms: self.ms.clone(),
        };
        for i in 0..self.len() {
            let p = iso.apply(Vec3::new(self.xs[i], self.ys[i], self.zs[i]));
            c.xs.push(p.x);
            c.ys.push(p.y);
            c.zs.push(p.z);
        }
        c
    }
}

/// The scalar potential `V(p) = −G Σ mᵢ / |p − xᵢ|` at world point `p`.
pub fn potential<S: Scalar>(cloud: &Cloud, p: Vec3<S>) -> S {
    let neg_g = S::from_f64(-G);
    let mut acc = S::from_f64(0.0);
    for i in 0..cloud.len() {
        let d = p - element(cloud, i);
        let r = d.norm();
        acc = acc + S::from_f64(cloud.ms[i]) * neg_g / r;
    }
    acc
}

/// The field `g = −∇V = −G Σ mᵢ (p − xᵢ) / |p − xᵢ|³` (attractive; points towards the mass).
pub fn field<S: Scalar>(cloud: &Cloud, p: Vec3<S>) -> Vec3<S> {
    let mut acc = Vec3::new(S::from_f64(0.0), S::from_f64(0.0), S::from_f64(0.0));
    for i in 0..cloud.len() {
        let d = p - element(cloud, i);
        let r = d.norm();
        let coeff = S::from_f64(-G) * S::from_f64(cloud.ms[i]) / (r * r * r);
        acc = acc + d.scale(coeff);
    }
    acc
}

/// The gradient tensor `Γ = ∇g = −G Σ (mᵢ / r³)(𝟙 − 3 d̂ d̂ᵀ)`.
///
/// Symmetric and trace-free by construction (`tr(𝟙 − 3 d̂ d̂ᵀ) = 0` since `|d̂| = 1`).
pub fn gradient_tensor<S: Scalar>(cloud: &Cloud, p: Vec3<S>) -> Mat3<S> {
    let zero = S::from_f64(0.0);
    let three = S::from_f64(3.0);
    let mut m = [[zero; 3]; 3];
    for i in 0..cloud.len() {
        let d = p - element(cloud, i);
        let r = d.norm();
        let inv_r = S::from_f64(1.0) / r;
        let coeff = S::from_f64(-G) * S::from_f64(cloud.ms[i]) / (r * r * r);
        // Unit separation, computed once per pair so the tensor is exactly symmetric.
        let u = [d.x * inv_r, d.y * inv_r, d.z * inv_r];
        for (j, &uj) in u.iter().enumerate() {
            for (k, &uk) in u.iter().enumerate() {
                let kron = if j == k { S::from_f64(1.0) } else { zero };
                m[j][k] = m[j][k] + coeff * (kron - three * uj * uk);
            }
        }
    }
    Mat3 { m }
}

/// The `i`-th element position, lifted to the scalar type.
fn element<S: Scalar>(cloud: &Cloud, i: usize) -> Vec3<S> {
    Vec3::new(
        S::from_f64(cloud.xs[i]),
        S::from_f64(cloud.ys[i]),
        S::from_f64(cloud.zs[i]),
    )
}

/// A term summed into the gravitational potential during the forward pass.
///
/// The atmospheric-GGN source is the canonical impl: a contribution folded into the potential in
/// the forward pass, never a post-hoc `NoiseSource`. Generic over `Scalar` so it differentiates on
/// the kernel path. Fleshed out at M5.
///
/// # Contract (spec `sec:contracts`, `GravitySource`)
/// - **Post.** `field = −∇potential` and `gradient = −∇∇potential` to numerical tolerance; the
///   gradient is symmetric and trace-free in vacuum.
/// - **Invariant.** Linear over a cloud; pure — no observable state mutation.
pub trait FieldContribution<S: Scalar> {
    /// The scalar potential contribution at world point `p` and time `t`.
    fn potential(&self, p: Vec3<S>, t: f64) -> S;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny deterministic PRNG — reproducible without a `rand` dependency.
    struct Lcg(u64);
    impl Lcg {
        fn next_unit(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 11) as f64 / 9007199254740992.0
        }
        fn range(&mut self, lo: f64, hi: f64) -> f64 {
            lo + (hi - lo) * self.next_unit()
        }
    }

    fn frob(m: &Mat3<f64>) -> f64 {
        let mut s = 0.0;
        for row in &m.m {
            for &x in row {
                s += x * x;
            }
        }
        s.sqrt()
    }

    #[test]
    fn gamma_symmetric_tracefree() {
        let mut rng = Lcg(0x51A7_1CE5);
        for _ in 0..200 {
            let n = 1 + (rng.next_unit() * 8.0) as usize;
            let mut elems = Vec::new();
            for _ in 0..n {
                elems.push((
                    rng.range(-3.0, 3.0),
                    rng.range(-3.0, 3.0),
                    rng.range(-3.0, 3.0),
                    rng.range(0.5, 5.0),
                ));
            }
            let cloud = Cloud::from_elements(&elems);
            // A field point held clear of the elements (vacuum).
            let p = Vec3::new(
                rng.range(8.0, 12.0),
                rng.range(8.0, 12.0),
                rng.range(8.0, 12.0),
            );
            let g = gradient_tensor(&cloud, p);
            let scale = frob(&g).max(1e-300);
            for j in 0..3 {
                for k in 0..3 {
                    assert!((g.m[j][k] - g.m[k][j]).abs() / scale <= 1e-12, "asymmetry");
                }
            }
            let trace = g.m[0][0] + g.m[1][1] + g.m[2][2];
            assert!(trace.abs() / scale <= 1e-12, "trace {trace} not free");
        }
    }

    #[test]
    fn falloff() {
        // A single point mass at the origin; V ∝ 1/r, |g| ∝ 1/r², ‖Γ‖ ∝ 1/r³ over three decades.
        let m = 3.0;
        let cloud = Cloud::from_elements(&[(0.0, 0.0, 0.0, m)]);
        let mut v_r = Vec::new();
        let mut g_r2 = Vec::new();
        let mut gamma_r3 = Vec::new();
        for e in 0..4 {
            let r = 10f64.powi(e); // 1, 10, 100, 1000
            let p = Vec3::new(r, 0.0, 0.0);
            v_r.push(potential(&cloud, p) * r);
            g_r2.push(field(&cloud, p).norm() * r * r);
            gamma_r3.push(frob(&gradient_tensor(&cloud, p)) * r * r * r);
        }
        // Each invariant is r-independent: the products above must be constant across decades.
        for series in [&v_r, &g_r2, &gamma_r3] {
            let ref0 = series[0];
            for &val in series {
                assert!((val - ref0).abs() / ref0.abs() <= 1e-10, "falloff drift");
            }
        }
        // Sanity on the constants: V·r = −Gm, |g|·r² = Gm.
        assert!((v_r[0] - (-G * m)).abs() / (G * m) <= 1e-10);
        assert!((g_r2[0] - G * m).abs() / (G * m) <= 1e-10);
    }
}
