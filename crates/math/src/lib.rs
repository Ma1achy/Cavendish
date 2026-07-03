//! `math` — numeric core: the `Scalar` seam and forward-mode `Dual`, plus vector/quaternion/isometry types.
//!
//! Design: `DESIGN.md` §5 (Scalar genericity), `design/compute.md` §8 (the `Dual`/`Scalar` contract).
//! Milestone: `milestones/M0-scaffolding.md`.
//!
//! The whole kernel is generic over [`Scalar`]: `f64` is the CPU reference, `f32` is re-expressed in
//! WGSL, and forward-mode [`Dual`] carries one tangent for `analysis`. Anything on the differentiable
//! path is written `fn …<S: Scalar>(…)`, so the same code differentiates without a rewrite.

use core::ops::{Add, Div, Mul, Neg, Sub};

/// The one numeric abstraction the kernel is generic over.
///
/// `f64` is the CPU reference; a forward-mode [`Dual`] threads a derivative through the identical
/// code. [`value`](Scalar::value) is the primal channel — for `Dual` it drops the tangent.
pub trait Scalar:
    Copy
    + PartialOrd
    + core::fmt::Debug
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
{
    /// Lift a plain `f64` constant (tangent zero for `Dual`).
    fn from_f64(x: f64) -> Self;
    /// The primal channel.
    fn value(self) -> f64;
    fn sqrt(self) -> Self;
    fn sin(self) -> Self;
    fn cos(self) -> Self;
    fn exp(self) -> Self;
    fn ln(self) -> Self;
    fn powi(self, n: i32) -> Self;
    fn abs(self) -> Self;
    fn max(self, o: Self) -> Self;
}

impl Scalar for f64 {
    fn from_f64(x: f64) -> Self {
        x
    }
    fn value(self) -> f64 {
        self
    }
    fn sqrt(self) -> Self {
        self.sqrt()
    }
    fn sin(self) -> Self {
        self.sin()
    }
    fn cos(self) -> Self {
        self.cos()
    }
    fn exp(self) -> Self {
        self.exp()
    }
    fn ln(self) -> Self {
        self.ln()
    }
    fn powi(self, n: i32) -> Self {
        self.powi(n)
    }
    fn abs(self) -> Self {
        self.abs()
    }
    fn max(self, o: Self) -> Self {
        self.max(o)
    }
}

/// A forward-mode dual number: value `v` plus one tangent `d` (the `ε` coefficient, `ε² = 0`).
///
/// Seeding `d = 1` on one input and `0` elsewhere makes `d` the partial derivative with respect to
/// that input — the mechanism `analysis` builds the Jacobian from.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Dual {
    pub v: f64,
    pub d: f64,
}

impl Dual {
    /// A dual with an explicit value and tangent.
    pub fn new(v: f64, d: f64) -> Self {
        Dual { v, d }
    }
    /// A seeded variable: value `v`, tangent `1` (differentiate with respect to this input).
    pub fn var(v: f64) -> Self {
        Dual { v, d: 1.0 }
    }
}

impl Add for Dual {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Dual { v: self.v + o.v, d: self.d + o.d }
    }
}

impl Sub for Dual {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Dual { v: self.v - o.v, d: self.d - o.d }
    }
}

#[allow(clippy::suspicious_arithmetic_impl)] // product rule: d(ab) = a·db + b·da
impl Mul for Dual {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        Dual { v: self.v * o.v, d: self.v * o.d + self.d * o.v }
    }
}

#[allow(clippy::suspicious_arithmetic_impl)] // quotient rule: d(a/b) = (da·b − a·db)/b²
impl Div for Dual {
    type Output = Self;
    fn div(self, o: Self) -> Self {
        Dual { v: self.v / o.v, d: (self.d * o.v - self.v * o.d) / (o.v * o.v) }
    }
}

impl Neg for Dual {
    type Output = Self;
    fn neg(self) -> Self {
        Dual { v: -self.v, d: -self.d }
    }
}

impl Scalar for Dual {
    fn from_f64(x: f64) -> Self {
        Dual { v: x, d: 0.0 }
    }
    fn value(self) -> f64 {
        self.v
    }
    fn sqrt(self) -> Self {
        let s = self.v.sqrt();
        Dual { v: s, d: self.d / (2.0 * s) }
    }
    fn sin(self) -> Self {
        Dual { v: self.v.sin(), d: self.d * self.v.cos() }
    }
    fn cos(self) -> Self {
        Dual { v: self.v.cos(), d: -self.d * self.v.sin() }
    }
    fn exp(self) -> Self {
        let e = self.v.exp();
        Dual { v: e, d: self.d * e }
    }
    fn ln(self) -> Self {
        Dual { v: self.v.ln(), d: self.d / self.v }
    }
    fn powi(self, n: i32) -> Self {
        Dual { v: self.v.powi(n), d: (n as f64) * self.v.powi(n - 1) * self.d }
    }
    fn abs(self) -> Self {
        Dual { v: self.v.abs(), d: self.d * self.v.signum() }
    }
    fn max(self, o: Self) -> Self {
        if self.v >= o.v {
            self
        } else {
            o
        }
    }
}

/// A 3-vector on the kernel path, generic over [`Scalar`].
#[derive(Clone, Copy, Debug)]
pub struct Vec3<S: Scalar> {
    pub x: S,
    pub y: S,
    pub z: S,
}

impl<S: Scalar> Vec3<S> {
    pub fn new(x: S, y: S, z: S) -> Self {
        Vec3 { x, y, z }
    }
    pub fn scale(self, s: S) -> Self {
        Vec3 { x: self.x * s, y: self.y * s, z: self.z * s }
    }
    pub fn dot(self, o: Self) -> S {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Self) -> Self {
        Vec3 {
            x: self.y * o.z - self.z * o.y,
            y: self.z * o.x - self.x * o.z,
            z: self.x * o.y - self.y * o.x,
        }
    }
    pub fn norm_squared(self) -> S {
        self.dot(self)
    }
    pub fn norm(self) -> S {
        self.norm_squared().sqrt()
    }
}

impl<S: Scalar> Add for Vec3<S> {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Vec3 { x: self.x + o.x, y: self.y + o.y, z: self.z + o.z }
    }
}

impl<S: Scalar> Sub for Vec3<S> {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Vec3 { x: self.x - o.x, y: self.y - o.y, z: self.z - o.z }
    }
}

impl<S: Scalar> Neg for Vec3<S> {
    type Output = Self;
    fn neg(self) -> Self {
        Vec3 { x: -self.x, y: -self.y, z: -self.z }
    }
}

/// A 3×3 matrix on the kernel path (row-major), generic over [`Scalar`].
#[derive(Clone, Copy, Debug)]
pub struct Mat3<S: Scalar> {
    pub m: [[S; 3]; 3],
}

impl<S: Scalar> Mat3<S> {
    pub fn identity() -> Self {
        let o = S::from_f64(1.0);
        let z = S::from_f64(0.0);
        Mat3 { m: [[o, z, z], [z, o, z], [z, z, o]] }
    }
    pub fn mul(self, o: Self) -> Self {
        Mat3 {
            m: core::array::from_fn(|i| {
                core::array::from_fn(|j| {
                    self.m[i][0] * o.m[0][j] + self.m[i][1] * o.m[1][j] + self.m[i][2] * o.m[2][j]
                })
            }),
        }
    }
    pub fn mul_vec(self, v: Vec3<S>) -> Vec3<S> {
        Vec3 {
            x: self.m[0][0] * v.x + self.m[0][1] * v.y + self.m[0][2] * v.z,
            y: self.m[1][0] * v.x + self.m[1][1] * v.y + self.m[1][2] * v.z,
            z: self.m[2][0] * v.x + self.m[2][1] * v.y + self.m[2][2] * v.z,
        }
    }
    pub fn transpose(self) -> Self {
        Mat3 { m: core::array::from_fn(|i| core::array::from_fn(|j| self.m[j][i])) }
    }
}

/// A unit quaternion, **wxyz** storage (matches the state bundle's `source_orientation`).
///
/// Off the kernel path: plain `f64`. Rotations assume the quaternion is normalised.
#[derive(Clone, Copy, Debug)]
pub struct Quat {
    pub w: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Quat {
    pub fn new(w: f64, x: f64, y: f64, z: f64) -> Self {
        Quat { w, x, y, z }
    }
    pub fn identity() -> Self {
        Quat { w: 1.0, x: 0.0, y: 0.0, z: 0.0 }
    }
    /// A rotation of `angle` radians about `axis` (need not be unit).
    pub fn from_axis_angle(axis: Vec3<f64>, angle: f64) -> Self {
        let n = axis.norm();
        let half = angle * 0.5;
        let k = if n > 0.0 { half.sin() / n } else { 0.0 };
        Quat { w: half.cos(), x: axis.x * k, y: axis.y * k, z: axis.z * k }
    }
    /// Hamilton product `self ⊗ o`.
    pub fn mul(self, o: Self) -> Self {
        Quat {
            w: self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
            x: self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            y: self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            z: self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
        }
    }
    pub fn norm(self) -> f64 {
        (self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
    pub fn normalise(self) -> Self {
        let n = self.norm();
        Quat { w: self.w / n, x: self.x / n, y: self.y / n, z: self.z / n }
    }
    /// The conjugate — the inverse for a unit quaternion.
    pub fn conjugate(self) -> Self {
        Quat { w: self.w, x: -self.x, y: -self.y, z: -self.z }
    }
    /// Rotate a vector: `self ⊗ (0, v) ⊗ self*` (assumes `self` is unit).
    pub fn rotate(self, v: Vec3<f64>) -> Vec3<f64> {
        let p = Quat { w: 0.0, x: v.x, y: v.y, z: v.z };
        let r = self.mul(p).mul(self.conjugate());
        Vec3 { x: r.x, y: r.y, z: r.z }
    }
}

/// A rigid transform: rotation then translation (world ← body). Off the kernel path: plain `f64`.
#[derive(Clone, Copy, Debug)]
pub struct Isometry3 {
    pub rotation: Quat,
    pub translation: Vec3<f64>,
}

impl Isometry3 {
    pub fn new(rotation: Quat, translation: Vec3<f64>) -> Self {
        Isometry3 { rotation, translation }
    }
    pub fn identity() -> Self {
        Isometry3 { rotation: Quat::identity(), translation: Vec3::new(0.0, 0.0, 0.0) }
    }
    /// Map a point through the transform.
    pub fn apply(self, p: Vec3<f64>) -> Vec3<f64> {
        self.rotation.rotate(p) + self.translation
    }
    /// Compose `self ∘ o` — apply `o`, then `self`.
    pub fn compose(self, o: Self) -> Self {
        Isometry3 {
            rotation: self.rotation.mul(o.rotation),
            translation: self.rotation.rotate(o.translation) + self.translation,
        }
    }
    /// The inverse transform.
    pub fn inverse(self) -> Self {
        let r = self.rotation.conjugate();
        Isometry3 { rotation: r, translation: -r.rotate(self.translation) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny deterministic PRNG — keeps the suite reproducible without a `rand` dependency.
    struct Lcg(u64);
    impl Lcg {
        fn next_unit(&mut self) -> f64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 11) as f64 / 9007199254740992.0
        }
        /// A sample in `[lo, hi)`.
        fn range(&mut self, lo: f64, hi: f64) -> f64 {
            lo + (hi - lo) * self.next_unit()
        }
    }

    /// Assert `a` and `b` agree to relative tolerance `tol` (with a unit floor for near-zero `b`).
    fn close_rel(a: f64, b: f64, tol: f64) {
        assert!(
            (a - b).abs() <= tol * b.abs().max(1.0),
            "rel mismatch: {a} vs {b} (tol {tol})"
        );
    }

    #[test]
    fn dual_arithmetic() {
        let mut rng = Lcg(0x1234_5678);
        for _ in 0..200 {
            let (av, ad) = (rng.range(-5.0, 5.0), rng.range(-5.0, 5.0));
            let (bv, bd) = (rng.range(1.0, 3.0), rng.range(-5.0, 5.0)); // bv ≥ 1: division safe
            let a = Dual::new(av, ad);
            let b = Dual::new(bv, bd);

            let s = a + b;
            close_rel(s.v, av + bv, 1e-12);
            close_rel(s.d, ad + bd, 1e-12);

            let d = a - b;
            close_rel(d.v, av - bv, 1e-12);
            close_rel(d.d, ad - bd, 1e-12);

            let m = a * b;
            close_rel(m.v, av * bv, 1e-12);
            close_rel(m.d, av * bd + ad * bv, 1e-12);

            let q = a / b;
            close_rel(q.v, av / bv, 1e-12);
            close_rel(q.d, (ad * bv - av * bd) / (bv * bv), 1e-12);
        }
    }

    #[test]
    fn dual_chain() {
        // 50 points, x > 0 (ln is sampled). Compare the tangent to the closed-form derivative.
        for i in 0..50 {
            let x0 = 0.1 + (i as f64) * 0.1;
            let x = Dual::var(x0);
            let one = Dual::from_f64(1.0);

            // f1 = sin(x²)·exp(−x);  f1' = e^{−x}(2x·cos(x²) − sin(x²))
            let f1 = (x * x).sin() * (-x).exp();
            let f1_prime = (-x0).exp() * (2.0 * x0 * (x0 * x0).cos() - (x0 * x0).sin());
            close_rel(f1.v, (x0 * x0).sin() * (-x0).exp(), 1e-12);
            close_rel(f1.d, f1_prime, 1e-12);

            // f2 = 1/√(1+x²);  f2' = −x·(1+x²)^{−3/2}
            let f2 = one / (one + x * x).sqrt();
            let f2_prime = -x0 * (1.0 + x0 * x0).powf(-1.5);
            close_rel(f2.v, 1.0 / (1.0 + x0 * x0).sqrt(), 1e-12);
            close_rel(f2.d, f2_prime, 1e-12);

            // f3 = ln(x)·x³;  f3' = x²·(1 + 3·ln x)
            let f3 = x.ln() * x.powi(3);
            let f3_prime = x0 * x0 * (1.0 + 3.0 * x0.ln());
            close_rel(f3.v, x0.ln() * x0.powi(3), 1e-12);
            close_rel(f3.d, f3_prime, 1e-12);
        }
    }

    #[test]
    #[allow(clippy::float_cmp)] // the value channel must match the f64 computation bit-for-bit
    fn dual_value_channel() {
        let mut rng = Lcg(0xC0FF_EE00);
        for _ in 0..200 {
            let (av, ad) = (rng.range(0.5, 5.0), rng.range(-5.0, 5.0));
            let (bv, bd) = (rng.range(1.0, 3.0), rng.range(-5.0, 5.0));
            let a = Dual::new(av, ad);
            let b = Dual::new(bv, bd);

            assert_eq!((a + b).v, av + bv);
            assert_eq!((a - b).v, av - bv);
            assert_eq!((a * b).v, av * bv);
            assert_eq!((a / b).v, av / bv);
            assert_eq!((-a).v, -av);
            assert_eq!(a.sqrt().v, av.sqrt());
            assert_eq!(a.sin().v, av.sin());
            assert_eq!(a.cos().v, av.cos());
            assert_eq!(a.exp().v, av.exp());
            assert_eq!(a.ln().v, av.ln());
            assert_eq!(a.powi(3).v, av.powi(3));
            assert_eq!(a.abs().v, av.abs());
            assert_eq!(a.max(b).v, av.max(bv));
        }
    }

    #[test]
    fn quat_norm_ops() {
        let mut rng = Lcg(0x9E37_79B9);
        for _ in 0..100 {
            // Two unit quaternions; their product stays unit.
            let axis1 = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
            let axis2 = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
            let q1 = Quat::from_axis_angle(axis1, rng.range(-3.0, 3.0));
            let q2 = Quat::from_axis_angle(axis2, rng.range(-3.0, 3.0));
            close_rel(q1.mul(q2).norm(), 1.0, 1e-14);

            // Normalise brings an arbitrary quaternion onto the unit sphere.
            let arb = Quat::new(
                rng.range(-4.0, 4.0),
                rng.range(-4.0, 4.0),
                rng.range(-4.0, 4.0),
                rng.range(1.0, 4.0),
            );
            close_rel(arb.normalise().norm(), 1.0, 1e-14);

            // Isometry compose/inverse round-trip: the inverse undoes apply, to 1e-14.
            let t = Vec3::new(rng.range(-5.0, 5.0), rng.range(-5.0, 5.0), rng.range(-5.0, 5.0));
            let iso = Isometry3::new(q1, t);
            let p = Vec3::new(rng.range(-5.0, 5.0), rng.range(-5.0, 5.0), rng.range(-5.0, 5.0));
            let back = iso.inverse().apply(iso.apply(p));
            close_rel(back.x, p.x, 1e-14);
            close_rel(back.y, p.y, 1e-14);
            close_rel(back.z, p.z, 1e-14);

            // compose(self, inverse) is the identity: zero translation, unit rotation.
            let id = iso.compose(iso.inverse());
            close_rel(id.translation.norm(), 0.0, 1e-14);
            close_rel(id.rotation.norm(), 1.0, 1e-14);
        }
    }
}
