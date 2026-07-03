//! `instrument` — the `PhaseModel` seam: the detector, its four ballistic arms, and `PropagationIntegral`.
//!
//! Design: `design/instrument.md`. Milestone: `milestones/M1-physics-spine.md`.
//!
//! One `Detector` is one gradiometer: two vertically stacked interferometers launched `Δr` apart. Its
//! four ballistic arms (two per IFO) are built **once** — the external source never perturbs them.
//! `PropagationIntegral` is the reference phase model: quadrature of the differenced potential along
//! the arms (spec `eq:singlephi`), double-differenced across the two IFOs (spec `eq:doublediff`).

use gravity::{gradient_tensor, potential, FieldContribution};
use math::{Isometry3, Quat, Vec3};
use source::SourceDynamics;

/// Pinned instrument/physics parameters (spec `tab:params`, AION-10 defaults).
#[derive(Clone, Copy, Debug)]
pub struct InstrumentConfig {
    pub m_a: f64,     // atomic mass (⁸⁷Sr)
    pub hbar: f64,    // reduced Planck constant
    pub g: f64,       // local gravity
    pub t_half: f64,  // T (2T is the interrogation time)
    pub v_rec: f64,   // recoil / arm-separation velocity, n·ħk/m_A
    pub ifo_sep: f64, // Δr, the gradiometer baseline
    pub u0: f64,      // launch velocity
    pub fine_dt: f64, // phase-integral step
}

impl Default for InstrumentConfig {
    fn default() -> Self {
        let m_a = 1.46e-25;
        let hbar = 1.055e-34;
        let lambda = 698e-9;
        let n_kick = 1000.0;
        let k = core::f64::consts::TAU / lambda; // 2π/λ
        InstrumentConfig {
            m_a,
            hbar,
            g: 9.81,
            t_half: 0.73,
            v_rec: n_kick * hbar * k / m_a,
            ifo_sep: 5.0,
            u0: 3.86,
            fine_dt: 0.01,
        }
    }
}

/// One gradiometer, placed in the world by a rigid isometry (position + orientation).
///
/// v1 uses identity orientation (vertical sensitive axis), but the placement is a full `Isometry3`,
/// so the `(D,7)` format admits tilted detectors later without a data-model change.
#[derive(Clone, Copy, Debug)]
pub struct Detector {
    pub placement: Isometry3,
}

impl Detector {
    /// A vertical detector whose lower interferometer sits at height `base_z` (identity orientation).
    pub fn new(base_z: f64) -> Self {
        Detector {
            placement: Isometry3::new(Quat::identity(), Vec3::new(0.0, 0.0, base_z)),
        }
    }

    /// A detector at an arbitrary placement (position + orientation; admits tilt).
    pub fn placed(placement: Isometry3) -> Self {
        Detector { placement }
    }

    /// The `(D,7)` row: position xyz + orientation quaternion (wxyz).
    pub fn placement_row(&self) -> [f64; 7] {
        let t = self.placement.translation;
        let q = self.placement.rotation;
        [t.x, t.y, t.z, q.w, q.x, q.y, q.z]
    }

    /// The world tower midpoint (between the two interferometers), where `QuasiStaticGradient`
    /// evaluates the gradient.
    pub fn midpoint(&self, cfg: &InstrumentConfig) -> Vec3<f64> {
        self.placement.apply(Vec3::new(0.0, 0.0, 0.5 * cfg.ifo_sep))
    }
}

/// A `D`-gradiometer array. The apparatus is shared; only the placements differ.
#[derive(Clone, Debug, Default)]
pub struct DetectorArray {
    pub detectors: Vec<Detector>,
}

impl DetectorArray {
    pub fn new(detectors: Vec<Detector>) -> Self {
        DetectorArray { detectors }
    }

    /// The `N = 1` array — a single gradiometer on the same code path.
    pub fn single(det: Detector) -> Self {
        DetectorArray {
            detectors: vec![det],
        }
    }

    pub fn len(&self) -> usize {
        self.detectors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.detectors.is_empty()
    }

    /// The `(D,7)` placement rows.
    pub fn placements(&self) -> Vec<[f64; 7]> {
        self.detectors.iter().map(|d| d.placement_row()).collect()
    }
}

/// One ballistic arm: closed-form free-fall with a single ∓`v_rec` π-pulse at `T`.
#[derive(Clone, Copy, Debug)]
pub struct Arm {
    z0: f64,
    v_first: f64,
    kick: f64,
    t_half: f64,
    g: f64,
}

impl Arm {
    fn ballistic(z: f64, v: f64, tau: f64, g: f64) -> f64 {
        z + v * tau - 0.5 * g * tau * tau
    }

    /// Height at local flight time `tau ∈ [0, 2T]`.
    pub fn z_at(&self, tau: f64) -> f64 {
        if tau <= self.t_half {
            Arm::ballistic(self.z0, self.v_first, tau, self.g)
        } else {
            let z_t = Arm::ballistic(self.z0, self.v_first, self.t_half, self.g);
            let v_t = self.v_first - self.g * self.t_half + self.kick;
            Arm::ballistic(z_t, v_t, tau - self.t_half, self.g)
        }
    }
}

/// One interferometer: its upper and lower arms.
#[derive(Clone, Copy, Debug)]
pub struct Ifo {
    pub lower: Arm,
    pub upper: Arm,
}

/// Build the two interferometers (four arms) in the **detector frame** — IFOs at local `z = 0` and
/// `z = Δr`. The apparatus is shared, so this is identical for every detector: built once, then
/// placed by each detector's isometry.
pub fn build_arms(cfg: &InstrumentConfig) -> [Ifo; 2] {
    let make = |z0: f64| Ifo {
        lower: Arm {
            z0,
            v_first: cfg.u0,
            kick: cfg.v_rec,
            t_half: cfg.t_half,
            g: cfg.g,
        },
        upper: Arm {
            z0,
            v_first: cfg.u0 + cfg.v_rec,
            kick: -cfg.v_rec,
            t_half: cfg.t_half,
            g: cfg.g,
        },
    };
    [make(0.0), make(cfg.ifo_sep)]
}

/// Composite Simpson's rule over `[a, b]` at ≈`step` resolution (even interval count).
fn simpson<F: Fn(f64) -> f64>(a: f64, b: f64, step: f64, f: F) -> f64 {
    let mut n = ((b - a) / step).round() as usize;
    if n < 2 {
        n = 2;
    }
    if n % 2 == 1 {
        n += 1;
    }
    let h = (b - a) / n as f64;
    let mut s = f(a) + f(b);
    for i in 1..n {
        let w = if i % 2 == 1 { 4.0 } else { 2.0 };
        s += w * f(a + i as f64 * h);
    }
    s * h / 3.0
}

/// Maps a scene of sources to one gradiometer's differential phase at a measurement time.
///
/// # Contract (spec `sec:contracts`, `PhaseModel`)
/// - **Method.** `delta_phi(sources, fields, det, t) -> ΔΦ_ℓ` in radians. The world potential is the
///   sum over rigid-body `sources` **and** analytic `fields` (atmospheric GGN) — decomposition runs
///   with subsets of the two lists.
/// - **Pre.** each contributor queryable on `[t − 2T, t]`; the arms are built.
/// - **Post.** Returns the double-difference (spec `eq:doublediff`); linear in the potential (so the
///   channels superpose exactly); deterministic.
pub trait PhaseModel {
    fn delta_phi(
        &self,
        sources: &[&dyn SourceDynamics],
        fields: &[&dyn FieldContribution<f64>],
        det: &Detector,
        t: f64,
    ) -> f64;
}

/// The reference phase model: faithful quadrature of the propagation-phase integral (spec v1).
#[derive(Clone, Copy, Debug, Default)]
pub struct PropagationIntegral {
    pub cfg: InstrumentConfig,
}

impl PropagationIntegral {
    pub fn new(cfg: InstrumentConfig) -> Self {
        PropagationIntegral { cfg }
    }

    /// The two single-interferometer phases `[δφ₁, δφ₂]` before the double difference — the potential
    /// summed over rigid-body `sources` and analytic `fields` along each interferometer's arms.
    pub fn per_ifo(
        &self,
        sources: &[&dyn SourceDynamics],
        fields: &[&dyn FieldContribution<f64>],
        det: &Detector,
        t: f64,
    ) -> [f64; 2] {
        let ifos = build_arms(&self.cfg);
        let two_t = 2.0 * self.cfg.t_half;
        // δφ for one interferometer: (m_A/ħ) ∫[V(z_u) − V(z_l)] dt over the flight.
        let dphi = |ifo: &Ifo| -> f64 {
            let integrand = |flight: f64| -> f64 {
                let t_abs = (t - two_t) + flight;
                // Arm points are built in the detector frame and placed by the detector isometry.
                let pu = det
                    .placement
                    .apply(Vec3::new(0.0, 0.0, ifo.upper.z_at(flight)));
                let pl = det
                    .placement
                    .apply(Vec3::new(0.0, 0.0, ifo.lower.z_at(flight)));
                let mut acc = 0.0;
                for src in sources {
                    // Body-frame evaluation: V is rigid-invariant, so evaluate the fixed body cloud
                    // at pose(t)⁻¹·p_arm rather than posing the whole cloud each tick.
                    let inv = src.pose_at(t_abs).inverse();
                    let body = src.body_cloud();
                    acc += potential(body, inv.apply(pu)) - potential(body, inv.apply(pl));
                }
                // Field contributions (atmospheric GGN) are evaluated in the world frame directly.
                for f in fields {
                    acc += f.potential(pu, t_abs) - f.potential(pl, t_abs);
                }
                acc
            };
            // Split at the π-pulse (τ = T): the arm velocity kinks there, so Simpson keeps its
            // high order only on each smooth half.
            let half = self.cfg.t_half;
            let acc = simpson(0.0, half, self.cfg.fine_dt, integrand)
                + simpson(half, two_t, self.cfg.fine_dt, integrand);
            (self.cfg.m_a / self.cfg.hbar) * acc
        };
        [dphi(&ifos[0]), dphi(&ifos[1])]
    }
}

impl PhaseModel for PropagationIntegral {
    fn delta_phi(
        &self,
        sources: &[&dyn SourceDynamics],
        fields: &[&dyn FieldContribution<f64>],
        det: &Detector,
        t: f64,
    ) -> f64 {
        let [d1, d2] = self.per_ifo(sources, fields, det, t);
        d2 - d1 // ΔΦ = δφ₂ − δφ₁ (spec sign)
    }
}

/// Which phase model a run uses. `PropagationIntegral` is the reference (default); `QuasiStatic` is
/// the fast path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PhaseModelKind {
    #[default]
    PropagationIntegral,
    QuasiStatic,
}

/// The fast phase model: the quasi-static gradient approximation (spec `Δφ ≈ k_eff·a·T²`).
///
/// `ΔΦ_qs = −k_eff·Γ_zz(p_det,t)·Δr·T²`, one `gradient_tensor` evaluation per (measurement × detector)
/// at the tower midpoint — cheap versus the propagation integral's arm quadratures. Exact under a
/// uniform gradient (an identity, the leading term of PI), approximate for a far, slow source.
#[derive(Clone, Copy, Debug, Default)]
pub struct QuasiStaticGradient {
    pub cfg: InstrumentConfig,
}

impl QuasiStaticGradient {
    pub fn new(cfg: InstrumentConfig) -> Self {
        QuasiStaticGradient { cfg }
    }

    /// The effective wavevector `k_eff = m_A·v_rec/ħ` (equivalently `n·2π/λ`).
    pub fn k_eff(&self) -> f64 {
        self.cfg.m_a * self.cfg.v_rec / self.cfg.hbar
    }
}

impl PhaseModel for QuasiStaticGradient {
    fn delta_phi(
        &self,
        sources: &[&dyn SourceDynamics],
        fields: &[&dyn FieldContribution<f64>],
        det: &Detector,
        t: f64,
    ) -> f64 {
        let p_det = det.midpoint(&self.cfg);
        // World-frame vertical gradient at the tower midpoint, summed over sources and fields.
        let mut gamma_zz = 0.0;
        for src in sources {
            let world = src.body_cloud().transformed(&src.pose_at(t));
            gamma_zz += gradient_tensor(&world, p_det).m[2][2];
        }
        for f in fields {
            gamma_zz += f.gradient_tensor(p_det, t).m[2][2];
        }
        // Leading minus matches PI's δφ₂ − δφ₁ sign in the uniform-gradient limit (∫(z_u−z_l)dt = v_rec·T²).
        -self.k_eff() * gamma_zz * self.cfg.ifo_sep * self.cfg.t_half * self.cfg.t_half
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gravity::Cloud;
    use math::Isometry3;
    use source::Prescribed;

    #[test]
    fn arms_close() {
        let cfg = InstrumentConfig::default();
        let two_t = 2.0 * cfg.t_half;
        for ifo in build_arms(&cfg) {
            // The two arms of each IFO re-close at 2T.
            assert!((ifo.upper.z_at(two_t) - ifo.lower.z_at(two_t)).abs() <= 1e-12);
            // Both stay within the 10 m tower over the whole flight.
            for k in 0..=146 {
                let tau = two_t * k as f64 / 146.0;
                for z in [ifo.upper.z_at(tau), ifo.lower.z_at(tau)] {
                    assert!((-0.5..=10.0).contains(&z), "arm left the tower: {z}");
                }
            }
        }
    }

    #[test]
    fn arm_separation() {
        let cfg = InstrumentConfig::default();
        let expected = cfg.v_rec * cfg.t_half;
        let two_t = 2.0 * cfg.t_half;
        for ifo in build_arms(&cfg) {
            // Separation peaks at τ = T, equal to v_rec·T.
            assert!(
                (ifo.upper.z_at(cfg.t_half) - ifo.lower.z_at(cfg.t_half) - expected).abs() <= 1e-12
            );
            let mut max_sep = 0.0f64;
            for k in 0..=200 {
                let tau = two_t * k as f64 / 200.0;
                max_sep = max_sep.max((ifo.upper.z_at(tau) - ifo.lower.z_at(tau)).abs());
            }
            assert!((max_sep - expected).abs() <= 1e-12);
        }
    }

    #[test]
    fn placement_roundtrip() {
        // A tilted detector (non-identity orientation): placement → placed arm bases → recovered
        // position and sensitive axis, to ≤1e-12. Proves the (D,7) format admits tilt.
        let cfg = InstrumentConfig::default();
        let pos = Vec3::new(3.0, -2.0, 1.5);
        let rot = Quat::from_axis_angle(Vec3::new(0.3, 1.0, -0.2), 0.4);
        let det = Detector::placed(Isometry3::new(rot, pos));

        let lower_base = det.placement.apply(Vec3::new(0.0, 0.0, 0.0));
        let upper_base = det.placement.apply(Vec3::new(0.0, 0.0, cfg.ifo_sep));
        assert!((lower_base - pos).norm() <= 1e-12, "position");
        let axis = (upper_base - lower_base).scale(1.0 / cfg.ifo_sep);
        let expected = rot.rotate(Vec3::new(0.0, 0.0, 1.0));
        assert!((axis - expected).norm() <= 1e-12, "sensitive axis");

        let row = det.placement_row();
        for (got, want) in row
            .iter()
            .zip([pos.x, pos.y, pos.z, rot.w, rot.x, rot.y, rot.z])
        {
            assert!((got - want).abs() <= 1e-12, "placement row");
        }
    }

    #[test]
    fn mass_linearity() {
        // ΔΦ is linear in source mass (spec INV.2): ΔΦ(αm) = α·ΔΦ(m).
        let det = Detector::new(0.0);
        let model = PropagationIntegral::default();
        let base = 100.0;
        let wall = |scale: f64| {
            Prescribed::fixed(
                Cloud::from_elements(&[(2.0, 0.0, 2.5, base * scale)]),
                Isometry3::identity(),
            )
        };
        let src1 = wall(1.0);
        let dphi1 = model.delta_phi(&[&src1], &[], &det, 2.0);
        for alpha in [0.1, 2.0, 10.0] {
            let src = wall(alpha);
            let dphi = model.delta_phi(&[&src], &[], &det, 2.0);
            assert!((dphi - alpha * dphi1).abs() / (alpha * dphi1).abs() <= 1e-12);
        }
    }

    #[test]
    fn qs_uniform_identity() {
        // Impose an exactly linear field V(z) = −½γz² (constant Γ_zz = γ). Then the propagation
        // integral of the analytic potential equals QS's −k_eff·γ·Δr·T² — an identity, not an
        // approximation; the residual is only integrator resolution.
        let cfg = InstrumentConfig::default();
        let gamma = 3.0e-6; // an arbitrary constant vertical gradient
        let v = |z: f64| -0.5 * gamma * z * z;
        let ifos = build_arms(&cfg);
        let two_t = 2.0 * cfg.t_half;
        let dphi = |ifo: &Ifo| -> f64 {
            let integ = |flight: f64| v(ifo.upper.z_at(flight)) - v(ifo.lower.z_at(flight));
            let half = cfg.t_half;
            (cfg.m_a / cfg.hbar)
                * (simpson(0.0, half, cfg.fine_dt, integ)
                    + simpson(half, two_t, cfg.fine_dt, integ))
        };
        let pi = dphi(&ifos[1]) - dphi(&ifos[0]);
        let k_eff = QuasiStaticGradient::default().k_eff();
        let qs = -k_eff * gamma * cfg.ifo_sep * cfg.t_half * cfg.t_half;
        let residual = (pi - qs).abs() / qs.abs();
        eprintln!("qs_uniform_identity: PI={pi:.6e} QS={qs:.6e} residual={residual:.2e}");
        assert!(residual <= 1e-6, "PI {pi} ≠ QS {qs}");
    }

    #[test]
    fn qs_scaling() {
        // k_eff is derived from the constants and equals n·2π/λ; ΔΦ_qs is linear in Γ_zz, Δr, T².
        let cfg = InstrumentConfig::default();
        let k_eff = QuasiStaticGradient::default().k_eff();
        let k_eff_expected = 1000.0 * core::f64::consts::TAU / 698e-9; // n·2π/λ
        assert!(
            (k_eff - k_eff_expected).abs() / k_eff_expected <= 1e-12,
            "k_eff drift"
        );

        let qs = |g: f64, dr: f64, t: f64| -k_eff * g * dr * t * t;
        let (g0, dr0, t0) = (2.0e-6, cfg.ifo_sep, cfg.t_half);
        let base = qs(g0, dr0, t0);
        for a in [0.1, 3.0, 7.0] {
            assert!((qs(a * g0, dr0, t0) - a * base).abs() / (a * base).abs() <= 1e-12);
            assert!((qs(g0, a * dr0, t0) - a * base).abs() / (a * base).abs() <= 1e-12);
            assert!((qs(g0, dr0, a.sqrt() * t0) - a * base).abs() / (a * base).abs() <= 1e-12);
        }
    }

    #[test]
    fn qs_vs_pi_far() {
        // A far, static source — the validity regime (standoff ≫ arm extent): QS approximates PI ≤1%.
        let det = Detector::new(0.0);
        let cloud = Cloud::from_elements(&[(100.0, 0.0, 2.5, 1.0e6)]);
        let src = Prescribed::fixed(cloud, Isometry3::identity());
        let pi = PropagationIntegral::default().delta_phi(&[&src], &[], &det, 2.0);
        let qs = QuasiStaticGradient::default().delta_phi(&[&src], &[], &det, 2.0);
        let delta = (pi - qs).abs() / pi.abs();
        eprintln!(
            "qs_vs_pi_far: PI={pi:.4e} QS={qs:.4e} delta={:.3}%",
            delta * 100.0
        );
        assert!(delta <= 0.01, "far-field: PI {pi} vs QS {qs}");
    }
}
