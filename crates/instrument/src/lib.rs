//! `instrument` — the `PhaseModel` seam: the detector, its four ballistic arms, and `PropagationIntegral`.
//!
//! Design: `design/instrument.md`. Milestone: `milestones/M1-physics-spine.md`.
//!
//! One `Detector` is one gradiometer: two vertically stacked interferometers launched `Δr` apart. Its
//! four ballistic arms (two per IFO) are built **once** — the external source never perturbs them.
//! `PropagationIntegral` is the reference phase model: quadrature of the differenced potential along
//! the arms (spec `eq:singlephi`), double-differenced across the two IFOs (spec `eq:doublediff`).

use gravity::potential;
use math::Vec3;
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

/// One gradiometer, placed by the height of its lower interferometer.
#[derive(Clone, Copy, Debug)]
pub struct Detector {
    pub base_z: f64,
}

impl Detector {
    pub fn new(base_z: f64) -> Self {
        Detector { base_z }
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

/// Build a detector's two interferometers (four arms) from the config. Done once per detector.
pub fn build_arms(det: &Detector, cfg: &InstrumentConfig) -> [Ifo; 2] {
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
    [make(det.base_z), make(det.base_z + cfg.ifo_sep)]
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
/// - **Method.** `delta_phi(sources, det, t) -> ΔΦ_ℓ` in radians.
/// - **Pre.** each source queryable on `[t − 2T, t]`; the arms are built.
/// - **Post.** Returns the double-difference (spec `eq:doublediff`); linear in source mass
///   (`delta_phi(α·m) = α·delta_phi(m)` to tolerance); deterministic.
pub trait PhaseModel {
    fn delta_phi(&self, sources: &[&dyn SourceDynamics], det: &Detector, t: f64) -> f64;
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
}

impl PhaseModel for PropagationIntegral {
    fn delta_phi(&self, sources: &[&dyn SourceDynamics], det: &Detector, t: f64) -> f64 {
        let ifos = build_arms(det, &self.cfg);
        let two_t = 2.0 * self.cfg.t_half;
        // δφ for one interferometer: (m_A/ħ) ∫[V(z_u) − V(z_l)] dt over the flight (external source only).
        let dphi = |ifo: &Ifo| -> f64 {
            let integrand = |flight: f64| -> f64 {
                let t_abs = (t - two_t) + flight;
                let pu = Vec3::new(0.0, 0.0, ifo.upper.z_at(flight));
                let pl = Vec3::new(0.0, 0.0, ifo.lower.z_at(flight));
                let mut acc = 0.0;
                for src in sources {
                    // Body-frame evaluation: V is rigid-invariant, so evaluate the fixed body cloud
                    // at pose(t)⁻¹·p_arm rather than posing the whole cloud each tick.
                    let inv = src.pose_at(t_abs).inverse();
                    let body = src.body_cloud();
                    acc += potential(body, inv.apply(pu)) - potential(body, inv.apply(pl));
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
        dphi(&ifos[1]) - dphi(&ifos[0]) // ΔΦ = δφ₂ − δφ₁ (spec sign)
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
        let det = Detector::new(0.0);
        let two_t = 2.0 * cfg.t_half;
        for ifo in build_arms(&det, &cfg) {
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
        let det = Detector::new(0.0);
        let expected = cfg.v_rec * cfg.t_half;
        let two_t = 2.0 * cfg.t_half;
        for ifo in build_arms(&det, &cfg) {
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
        let dphi1 = model.delta_phi(&[&src1], &det, 2.0);
        for alpha in [0.1, 2.0, 10.0] {
            let src = wall(alpha);
            let dphi = model.delta_phi(&[&src], &det, 2.0);
            assert!((dphi - alpha * dphi1).abs() / (alpha * dphi1).abs() <= 1e-12);
        }
    }
}
