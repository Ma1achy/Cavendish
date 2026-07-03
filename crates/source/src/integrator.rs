//! The hand-written, structure-preserving rotation integrator, generic over [`Scalar`].
//!
//! The torque-free Euler top is integrated by the **McLachlan/Reich free-rigid-body Strang
//! splitting**: the kinetic energy splits into per-principal-axis terms, each an *exact* rotation of
//! the body angular momentum `Π = I∘ω`. Composing them (order 1,2,3,2,1) conserves `|L| = |Π|` to
//! machine precision by construction and keeps `E` bounded forever — no secular drift. The quaternion
//! is reconstructed by the exponential map per sub-rotation and renormalised. It is written
//! `fn step<S: Scalar>(…)`, not f64-only, so a forward-mode `Dual` flows through at M8 (`compute`
//! re-expresses the same step in WGSL at M6). The pendulum is separable and uses a plain leapfrog.

use math::{Scalar, Vec3};

/// A Scalar-generic quaternion as `[w, x, y, z]` (wxyz) — local to the integrator; `math::Quat`
/// stays f64. Hamilton product `a ⊗ b`.
fn quat_mul<S: Scalar>(a: [S; 4], b: [S; 4]) -> [S; 4] {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
        a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ]
}

/// The exponential-map quaternion for a rotation of angle `phi` about principal axis `axis` (0,1,2).
fn quat_axis<S: Scalar>(axis: usize, phi: S) -> [S; 4] {
    let half = phi * S::from_f64(0.5);
    let zero = S::from_f64(0.0);
    let mut q = [half.cos(), zero, zero, zero];
    q[axis + 1] = half.sin();
    q
}

fn quat_normalise<S: Scalar>(q: [S; 4]) -> [S; 4] {
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    [q[0] / n, q[1] / n, q[2] / n, q[3] / n]
}

/// Rotate the body angular momentum `pi` about principal axis `k` by angle `phi` (the exact sub-flow
/// of the axis-`k` kinetic term: `Π̇ = Π × ωₖêₖ`).
fn rotate_pi<S: Scalar>(pi: &mut [S; 3], k: usize, phi: S) {
    let c = phi.cos();
    let s = phi.sin();
    match k {
        0 => {
            let (y, z) = (pi[1], pi[2]);
            pi[1] = y * c + z * s;
            pi[2] = z * c - y * s;
        }
        1 => {
            let (z, x) = (pi[2], pi[0]);
            pi[2] = z * c + x * s;
            pi[0] = x * c - z * s;
        }
        _ => {
            let (x, y) = (pi[0], pi[1]);
            pi[0] = x * c + y * s;
            pi[1] = y * c - x * s;
        }
    }
}

/// One structure-preserving substep of the torque-free Euler top over time `h`.
///
/// Updates the quaternion `q` (wxyz, body → world) and the body angular velocity `omega` in place,
/// given the principal moments `inertia = (I₁, I₂, I₃)`.
pub fn step<S: Scalar>(q: &mut [S; 4], omega: &mut Vec3<S>, inertia: Vec3<S>, h: S) {
    let half = h * S::from_f64(0.5);
    let inv = [
        S::from_f64(1.0) / inertia.x,
        S::from_f64(1.0) / inertia.y,
        S::from_f64(1.0) / inertia.z,
    ];
    let mut pi = [
        inertia.x * omega.x,
        inertia.y * omega.y,
        inertia.z * omega.z,
    ];
    // Strang composition of the three per-axis exact rotations.
    for (k, tau) in [(0, half), (1, half), (2, h), (1, half), (0, half)] {
        let phi = pi[k] * inv[k] * tau; // ωₖ·τ, ωₖ = Πₖ/Iₖ
        rotate_pi(&mut pi, k, phi);
        *q = quat_mul(*q, quat_axis(k, phi));
    }
    *q = quat_normalise(*q);
    *omega = Vec3::new(pi[0] * inv[0], pi[1] * inv[1], pi[2] * inv[2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use math::Dual;

    #[test]
    #[allow(clippy::float_cmp)] // the value channel must match the f64 computation bit-for-bit
    fn scalar_generic() {
        // step::<f64> and step::<Dual>'s value channel agree exactly — the guard on the M8 autodiff path.
        let (i1, i2, i3) = (2.0, 3.0, 5.0);
        let (w0, w1, w2) = (0.3, 0.5, 0.2);
        let h = 0.01;

        let mut qf = [1.0, 0.0, 0.0, 0.0];
        let mut wf = Vec3::new(w0, w1, w2);

        // Seed non-trivial tangents so the Dual is genuinely differentiated — the value channel must
        // be unaffected.
        let mut qd = [
            Dual::new(1.0, 0.1),
            Dual::new(0.0, 0.2),
            Dual::new(0.0, -0.3),
            Dual::new(0.0, 0.05),
        ];
        let mut wd = Vec3::new(Dual::var(w0), Dual::new(w1, 0.7), Dual::new(w2, -0.4));

        for _ in 0..1000 {
            step::<f64>(&mut qf, &mut wf, Vec3::new(i1, i2, i3), h);
            step::<Dual>(
                &mut qd,
                &mut wd,
                Vec3::new(Dual::from_f64(i1), Dual::from_f64(i2), Dual::from_f64(i3)),
                Dual::from_f64(h),
            );
        }
        for k in 0..4 {
            assert_eq!(qf[k], qd[k].v, "quaternion value channel drift at {k}");
        }
        assert_eq!(wf.x, wd.x.v);
        assert_eq!(wf.y, wd.y.v);
        assert_eq!(wf.z, wd.z.v);
    }

    #[test]
    fn free_top_no_drift_smoke() {
        // A quick guard that |L| and E stay put over a modest run (the full 1e4-period test is in lib).
        let inertia = Vec3::new(2.0, 3.0, 5.0);
        let mut q = [1.0, 0.0, 0.0, 0.0];
        let mut omega = Vec3::new(0.4, 0.1, 0.6);
        let l0 = ang_mom(&omega, &inertia);
        let e0 = energy(&omega, &inertia);
        for _ in 0..100_000 {
            step::<f64>(&mut q, &mut omega, inertia, 0.005);
        }
        let l = ang_mom(&omega, &inertia);
        let e = energy(&omega, &inertia);
        assert!((l - l0).abs() / l0 <= 1e-9, "|L| drift {l} vs {l0}");
        assert!((e - e0).abs() / e0 <= 1e-6, "E drift {e} vs {e0}");
    }

    fn ang_mom(w: &Vec3<f64>, i: &Vec3<f64>) -> f64 {
        let l = Vec3::new(i.x * w.x, i.y * w.y, i.z * w.z);
        l.norm()
    }

    fn energy(w: &Vec3<f64>, i: &Vec3<f64>) -> f64 {
        0.5 * (i.x * w.x * w.x + i.y * w.y * w.y + i.z * w.z * w.z)
    }
}
