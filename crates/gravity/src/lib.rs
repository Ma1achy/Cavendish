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

/// The second-moment reduction of a cloud (spec `sec:contracts`; `design/gravity.md` §6).
///
/// The inertia tensor `I` and the gravitational quadrupole `Q` are linear maps of the one second
/// moment `C = Σ mᵢ(rᵢ−com)(rᵢ−com)ᵀ` — `I = tr(C)𝟙 − C`, `Q = 3C − tr(C)𝟙` — so they share principal
/// axes (the eigenvectors of `C`). `moments` are the principal moments of inertia, ascending, matching
/// the columns of `axes`.
#[derive(Clone, Copy, Debug)]
pub struct Inertia {
    pub mass: f64,
    pub com: Vec3<f64>,
    pub c: Mat3<f64>,
    pub i: Mat3<f64>,
    pub q: Mat3<f64>,
    pub axes: Mat3<f64>,
    pub moments: [f64; 3],
}

/// Reduce a cloud to its mass, centre of mass, and second-moment descriptors.
pub fn inertia(cloud: &Cloud) -> Inertia {
    let mass = cloud.total_mass();
    let (mut cx, mut cy, mut cz) = (0.0, 0.0, 0.0);
    for k in 0..cloud.len() {
        cx += cloud.ms[k] * cloud.xs[k];
        cy += cloud.ms[k] * cloud.ys[k];
        cz += cloud.ms[k] * cloud.zs[k];
    }
    cx /= mass;
    cy /= mass;
    cz /= mass;

    let mut c = [[0.0f64; 3]; 3];
    for k in 0..cloud.len() {
        let d = [cloud.xs[k] - cx, cloud.ys[k] - cy, cloud.zs[k] - cz];
        for (a, &da) in d.iter().enumerate() {
            for (b, &db) in d.iter().enumerate() {
                c[a][b] += cloud.ms[k] * da * db;
            }
        }
    }
    let tr = c[0][0] + c[1][1] + c[2][2];

    let mut i = [[0.0f64; 3]; 3];
    let mut q = [[0.0f64; 3]; 3];
    for (a, (irow, qrow)) in i.iter_mut().zip(q.iter_mut()).enumerate() {
        for (b, (ie, qe)) in irow.iter_mut().zip(qrow.iter_mut()).enumerate() {
            let kron = if a == b { 1.0 } else { 0.0 };
            *ie = tr * kron - c[a][b];
            *qe = 3.0 * c[a][b] - tr * kron;
        }
    }

    // Principal frame from eig(C); moments of I are tr(C) − eigvals(C).
    let (lambda, vecs) = jacobi(c);
    let mut idx = [0usize, 1, 2];
    idx.sort_by(|&a, &b| (tr - lambda[a]).total_cmp(&(tr - lambda[b])));
    let moments = [
        tr - lambda[idx[0]],
        tr - lambda[idx[1]],
        tr - lambda[idx[2]],
    ];
    let mut axes = [[0.0f64; 3]; 3];
    for (col, &j) in idx.iter().enumerate() {
        for (row, axrow) in axes.iter_mut().enumerate() {
            axrow[col] = vecs[row][j];
        }
    }

    Inertia {
        mass,
        com: Vec3::new(cx, cy, cz),
        c: Mat3 { m: c },
        i: Mat3 { m: i },
        q: Mat3 { m: q },
        axes: Mat3 { m: axes },
        moments,
    }
}

/// Cyclic Jacobi eigensolver for a symmetric 3×3 matrix: returns eigenvalues and eigenvectors (as
/// the columns of the returned matrix). No external linalg dependency.
fn jacobi(mut a: [[f64; 3]; 3]) -> ([f64; 3], [[f64; 3]; 3]) {
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..50 {
        let off = a[0][1].abs() + a[0][2].abs() + a[1][2].abs();
        if off <= 1e-300 {
            break;
        }
        for (p, q) in [(0, 1), (0, 2), (1, 2)] {
            if a[p][q].abs() <= 1e-300 {
                continue;
            }
            let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            let (app, aqq, apq) = (a[p][p], a[q][q], a[p][q]);
            a[p][p] = c * c * app - 2.0 * s * c * apq + s * s * aqq;
            a[q][q] = s * s * app + 2.0 * s * c * apq + c * c * aqq;
            a[p][q] = 0.0;
            a[q][p] = 0.0;
            let r = 3 - p - q;
            let (arp, arq) = (a[r][p], a[r][q]);
            a[r][p] = c * arp - s * arq;
            a[p][r] = a[r][p];
            a[r][q] = s * arp + c * arq;
            a[q][r] = a[r][q];
            for vrow in v.iter_mut() {
                let (vp, vq) = (vrow[p], vrow[q]);
                vrow[p] = c * vp - s * vq;
                vrow[q] = s * vp + c * vq;
            }
        }
    }
    ([a[0][0], a[1][1], a[2][2]], v)
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
