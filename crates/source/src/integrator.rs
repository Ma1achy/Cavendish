//! The hand-written, structure-preserving rotation integrator, generic over [`Scalar`].
//!
//! The torque-free Euler top is integrated by the **McLachlan/Reich free-rigid-body Strang
//! splitting**: the kinetic energy splits into per-principal-axis terms, each an *exact* rotation of
//! the body angular momentum `Π = I∘ω`. Composing them (order 1,2,3,2,1) conserves `|L| = |Π|` to
//! machine precision by construction and keeps `E` bounded forever — no secular drift. The quaternion
//! is reconstructed by the exponential map per sub-rotation and renormalised. It is written
//! `fn step<S: Scalar>(…)`, not f64-only, so a forward-mode `Dual` flows through at M8 (`compute`
//! re-expresses the same step in WGSL at M6). The pendulum is separable and uses a plain leapfrog.

use math::{Quat, Scalar, Vec3};

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

/// Angular acceleration from Euler's equations: `ω̇ᵢ = (Iⱼ − Iₖ)/Iᵢ · ωⱼωₖ`.
pub fn euler_accel(omega: Vec3<f64>, inertia: Vec3<f64>) -> Vec3<f64> {
    Vec3::new(
        (inertia.y - inertia.z) / inertia.x * omega.y * omega.z,
        (inertia.z - inertia.x) / inertia.y * omega.z * omega.x,
        (inertia.x - inertia.y) / inertia.z * omega.x * omega.y,
    )
}

/// Integrate the torque-free Euler top from the identity orientation to time `t` on the `fine_dt`
/// grid (a pure function of `t`). Returns the body → world orientation, body angular velocity, and
/// body angular acceleration.
pub fn free_rotation_state(
    omega0: Vec3<f64>,
    inertia: Vec3<f64>,
    t: f64,
    fine_dt: f64,
) -> (Quat, Vec3<f64>, Vec3<f64>) {
    let mut q = [1.0, 0.0, 0.0, 0.0];
    let mut omega = omega0;
    let n = (t / fine_dt).max(0.0) as usize;
    for _ in 0..n {
        step::<f64>(&mut q, &mut omega, inertia, fine_dt);
    }
    let rem = t - n as f64 * fine_dt;
    if rem > 1e-15 {
        step::<f64>(&mut q, &mut omega, inertia, rem);
    }
    (
        Quat::new(q[0], q[1], q[2], q[3]),
        omega,
        euler_accel(omega, inertia),
    )
}

/// One leapfrog (Störmer–Verlet) step of the physical pendulum `θ̈ = −k·sin θ`.
fn leap_step(theta: &mut f64, thetadot: &mut f64, k: f64, h: f64) {
    *thetadot += 0.5 * h * (-k * theta.sin());
    *theta += h * *thetadot;
    *thetadot += 0.5 * h * (-k * theta.sin());
}

/// Integrate the physical pendulum (leapfrog) about `axis` to time `t`, with `k = Mgd/I_pivot`.
/// Returns the world orientation, angular velocity, and angular acceleration.
pub fn libration_state(
    axis: Vec3<f64>,
    k: f64,
    theta0: f64,
    thetadot0: f64,
    t: f64,
    fine_dt: f64,
) -> (Quat, Vec3<f64>, Vec3<f64>) {
    let mut theta = theta0;
    let mut thetadot = thetadot0;
    let n = (t / fine_dt).max(0.0) as usize;
    for _ in 0..n {
        leap_step(&mut theta, &mut thetadot, k, fine_dt);
    }
    let rem = t - n as f64 * fine_dt;
    if rem > 1e-15 {
        leap_step(&mut theta, &mut thetadot, k, rem);
    }
    let axis_n = axis.scale(1.0 / axis.norm());
    (
        Quat::from_axis_angle(axis_n, theta),
        axis_n.scale(thetadot),
        axis_n.scale(-k * theta.sin()),
    )
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
    fn quat_norm_preserved() {
        // |q| = 1 after 10⁶ substeps (renormalised each step).
        let inertia = Vec3::new(2.0, 3.0, 5.0);
        let mut q = [1.0, 0.0, 0.0, 0.0];
        let mut omega = Vec3::new(0.4, 0.7, 0.2);
        for _ in 0..1_000_000 {
            step::<f64>(&mut q, &mut omega, inertia, 0.001);
        }
        let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
        assert!((n - 1.0).abs() <= 1e-12, "|q| = {n}");
    }

    #[test]
    fn free_top_invariants() {
        // E and |L| bounded with NO secular drift over ~10⁴ tumble periods at h = period/100.
        let inertia = Vec3::new(2.0, 3.0, 5.0);
        let omega0 = Vec3::new(0.4, 0.1, 0.6);
        let period = std::f64::consts::TAU / omega0.norm();
        let h = period / 100.0;
        let steps = 1_000_000usize;
        let mut q = [1.0, 0.0, 0.0, 0.0];
        let mut omega = omega0;
        let (l0, e0) = (ang_mom(&omega, &inertia), energy(&omega, &inertia));
        let (mut l_dev, mut e_dev) = (0.0f64, 0.0f64);
        // Half-run means of E: a secular drift moves the mean; a bounded oscillation does not.
        let (mut e_first, mut e_last) = (0.0f64, 0.0f64);
        for s in 0..steps {
            step::<f64>(&mut q, &mut omega, inertia, h);
            let e = energy(&omega, &inertia);
            l_dev = l_dev.max((ang_mom(&omega, &inertia) - l0).abs() / l0);
            e_dev = e_dev.max((e - e0).abs() / e0);
            if s < steps / 2 {
                e_first += e;
            } else {
                e_last += e;
            }
        }
        let mean_first = e_first / (steps / 2) as f64;
        let mean_last = e_last / (steps - steps / 2) as f64;
        let secular = (mean_last - mean_first).abs() / e0;
        eprintln!("free_top: |L| dev {l_dev:.2e}  E osc {e_dev:.2e}  E secular {secular:.2e}");
        // |L| is conserved to machine precision by construction (every substep is a rotation of Π).
        assert!(l_dev <= 1e-6, "|L| drift {l_dev:.2e}");
        // E oscillates (bounded) but does not drift: the half-run means agree to ≤1e-6.
        assert!(secular <= 1e-6, "E secular drift {secular:.2e}");
    }

    #[test]
    fn pendulum_energy() {
        let k = 3.0; // Mgd/I_pivot
        let expected = std::f64::consts::TAU / k.sqrt();

        // Small-angle period = 2π√(I_p/Mgd) = 2π/√k, to ≤1e-4.
        {
            let h = expected / 2000.0;
            let (mut theta, mut thetadot) = (0.001, 0.0);
            let (mut t, mut prev) = (0.0, theta);
            let quarter = loop {
                leap_step(&mut theta, &mut thetadot, k, h);
                t += h;
                if theta <= 0.0 {
                    let frac = prev / (prev - theta); // linear interpolation of the zero-crossing
                    break t - h + frac * h;
                }
                prev = theta;
            };
            let period = 4.0 * quarter;
            assert!(
                (period - expected).abs() / expected <= 1e-4,
                "period {period} vs {expected}"
            );
        }

        // Energy bounded (no secular drift) over ~10⁴ periods, at a larger swing.
        {
            let h = expected / 200.0;
            let steps = 2_000_000usize;
            let (mut theta, mut thetadot) = (0.6, 0.0);
            let e = |th: f64, thd: f64| 0.5 * thd * thd - k * th.cos();
            let e0 = e(theta, thetadot);
            let (mut e_first, mut e_last) = (0.0f64, 0.0f64);
            for s in 0..steps {
                leap_step(&mut theta, &mut thetadot, k, h);
                let en = e(theta, thetadot);
                if s < steps / 2 {
                    e_first += en;
                } else {
                    e_last += en;
                }
            }
            let secular =
                (e_last / (steps / 2) as f64 - e_first / (steps / 2) as f64).abs() / e0.abs();
            assert!(secular <= 1e-6, "pendulum E secular drift {secular:.2e}");
        }
    }

    #[test]
    fn axisymmetric_analytic() {
        // Symmetric top (I₁ = I₂): ω precesses about ê₃ at Ω = (I₃−I₁)/I₁·ω₃.
        let inertia = Vec3::new(2.0, 2.0, 5.0);
        let omega3 = 0.6;
        let big_omega = (inertia.z - inertia.x) / inertia.x * omega3;
        let mut q = [1.0, 0.0, 0.0, 0.0];
        let mut omega = Vec3::new(0.3, 0.0, omega3);
        let h = 1e-4;
        let tau = 1.0; // Ω·τ ≈ 0.9 < π, so the phase does not wrap
        let steps = (tau / h) as usize;
        for _ in 0..steps {
            step::<f64>(&mut q, &mut omega, inertia, h);
        }
        let t = steps as f64 * h;
        let measured = omega.y.atan2(omega.x) / t; // phase of (ω₁, ω₂) accumulated over t
        eprintln!("axisymmetric: measured Ω {measured:.9} vs {big_omega:.9}");
        assert!(
            (measured - big_omega).abs() / big_omega.abs() <= 1e-8,
            "precession {measured} vs {big_omega}"
        );
    }

    #[test]
    fn dzhanibekov() {
        // An asymmetric top spun near its intermediate axis flips sign periodically (the tennis-racket
        // / Dzhanibekov effect). It is integrable, not chaotic — the flip period is well-defined and
        // must be STABLE under fine_dt halving; a wandering period would mean the integrator is wrong.
        fn flip_period(h: f64) -> f64 {
            let inertia = Vec3::new(2.0, 3.0, 5.0); // I₁ < I₂ < I₃; ê₂ (index 1) is the intermediate axis
            let mut q = [1.0, 0.0, 0.0, 0.0];
            let mut omega = Vec3::new(0.001, 1.0, 0.0); // ω ≈ ω₂ê₂ + 1e-3 perturbation
            let mut prev = omega.y;
            let mut t = 0.0;
            let mut flips = Vec::new();
            while flips.len() < 7 && t < 5000.0 {
                step::<f64>(&mut q, &mut omega, inertia, h);
                t += h;
                if (prev > 0.0) != (omega.y > 0.0) {
                    flips.push(t); // ω₂ changed sign — a flip
                }
                prev = omega.y;
            }
            let iv: Vec<f64> = flips.windows(2).map(|w| w[1] - w[0]).collect();
            iv.iter().sum::<f64>() / iv.len().max(1) as f64
        }
        let coarse = flip_period(0.01);
        let fine = flip_period(0.005);
        let rel = (coarse - fine).abs() / fine;
        eprintln!("dzhanibekov: coarse {coarse:.4} fine {fine:.4}  rel {rel:.2e}");
        assert!(coarse > 0.0 && fine > 0.0, "no flips detected");
        assert!(
            rel <= 0.01,
            "flip period unstable under refinement: {rel:.3}"
        );
    }

    fn ang_mom(w: &Vec3<f64>, i: &Vec3<f64>) -> f64 {
        let l = Vec3::new(i.x * w.x, i.y * w.y, i.z * w.z);
        l.norm()
    }

    fn energy(w: &Vec3<f64>, i: &Vec3<f64>) -> f64 {
        0.5 * (i.x * w.x * w.x + i.y * w.y * w.y + i.z * w.z * w.z)
    }
}
