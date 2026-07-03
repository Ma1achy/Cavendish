//! `noise` — the post-hoc `NoiseSource` stack and the atmospheric-GGN field source.
//!
//! Design: `design/noise.md`. Milestone: `milestones/M5-channels-and-decomposition.md`.
//!
//! Two kinds, one boundary: **additive measurement noise** (shot, vibration) perturbs the *measured*
//! phase and is applied post-hoc via [`NoiseSource::add`]; **atmospheric GGN** is a real gravitational
//! field (a stochastic `δρ`) and enters the forward pass as a [`gravity::FieldContribution`] (Commit
//! 3), never here. Determinism is via a counter-based keyed RNG: seeds compose through the key tree,
//! never sequential state (spec `nfr:reproducibility`).

use gravity::{FieldContribution, G};
use math::{Mat3, Scalar, Vec3};

const TAU: f64 = std::f64::consts::TAU;
const FOUR_PI_G: f64 = 4.0 * std::f64::consts::PI * G;

/// A counter-based keyed RNG — a stream keyed by `(seed, label)` whose draws are indexed by an
/// internal counter, so a draw is a pure function of `(key, index)` (parallel, order-independent).
#[derive(Clone, Debug)]
pub struct KeyRng {
    key: u64,
    counter: u64,
}

impl KeyRng {
    /// A stream keyed by the root `seed` mixed with a `label` (e.g. `"atmo"`, `"noise/0"`).
    pub fn stream(seed: u64, label: &str) -> Self {
        KeyRng {
            key: mix(seed, hash_str(label)),
            counter: 0,
        }
    }

    /// The next uniform `u64` (counter-based: `splitmix64(key ⊕ counter·φ)`).
    pub fn next_u64(&mut self) -> u64 {
        let v = splitmix64(self.key ^ self.counter.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        self.counter = self.counter.wrapping_add(1);
        v
    }

    /// A uniform `f64` in `[0, 1)`.
    pub fn next_unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A standard-normal draw (Box–Muller).
    pub fn next_normal(&mut self) -> f64 {
        let u1 = self.next_unit().max(1e-300);
        let u2 = self.next_unit();
        (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
    }
}

fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn mix(a: u64, b: u64) -> u64 {
    splitmix64(a ^ splitmix64(b))
}

fn hash_str(s: &str) -> u64 {
    // FNV-1a — a stable, dependency-free string hash.
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    for &b in s.as_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// One term in the post-hoc noise stack.
///
/// # Contract (spec `sec:contracts`, `NoiseSource`)
/// - **Method.** `add(t, noise, rng)` — mutates the `(T, D)` buffer in place.
/// - **Post.** Additive and zero-mean for shot/vibration; deterministic given the key. Order in the
///   stack is significant and preserved.
pub trait NoiseSource {
    /// Add this term's realisation to the `(T, D)` `noise` buffer, sampled at times `t`.
    fn add(&self, t: &[f64], noise: &mut [Vec<f64>], rng: &mut KeyRng);
}

/// The ordered post-hoc noise stack.
#[derive(Default)]
pub struct NoiseStack(pub Vec<Box<dyn NoiseSource>>);

impl NoiseStack {
    /// Realise the `(T, D)` noise: each source in declared order, keyed by `(seed, "noise/i")`.
    pub fn realise(&self, t: &[f64], d: usize, seed: u64) -> Vec<Vec<f64>> {
        let mut noise = vec![vec![0.0; d]; t.len()];
        for (i, src) in self.0.iter().enumerate() {
            let mut rng = KeyRng::stream(seed, &format!("noise/{i}"));
            src.add(t, &mut noise, &mut rng);
        }
        noise
    }
}

/// Atom-counting shot noise: iid `N(0, σ²)` per `(measurement, detector)` — the measurement floor.
#[derive(Clone, Copy, Debug)]
pub struct ShotNoise {
    pub sigma: f64,
}

impl NoiseSource for ShotNoise {
    fn add(&self, _t: &[f64], noise: &mut [Vec<f64>], rng: &mut KeyRng) {
        for row in noise.iter_mut() {
            for x in row.iter_mut() {
                *x += self.sigma * rng.next_normal();
            }
        }
    }
}

/// Platform vibration residual: a coloured (AR(1)) time-series per detector, reduced by the
/// gradiometer common-mode rejection ratio.
#[derive(Clone, Copy, Debug)]
pub struct VibrationResidual {
    pub sigma: f64,
    /// AR(1) coefficient in `[0, 1)` — the colour (0 = white).
    pub rho: f64,
    /// Common-mode rejection factor applied to the amplitude.
    pub rejection: f64,
}

impl NoiseSource for VibrationResidual {
    fn add(&self, _t: &[f64], noise: &mut [Vec<f64>], rng: &mut KeyRng) {
        let d = noise.first().map_or(0, |r| r.len());
        let scale = self.sigma * self.rejection;
        let inno = (1.0 - self.rho * self.rho).sqrt();
        for dd in 0..d {
            let mut prev = 0.0;
            for row in noise.iter_mut() {
                let x = self.rho * prev + inno * rng.next_normal();
                row[dd] += scale * x;
                prev = x;
            }
        }
    }
}

/// Atmospheric-GGN configuration (spec `tab:atmo`). M5 realises the model *structure* — the finite
/// correlation length that drives partial common-mode; the full Bowman/ERA5 amplitude models are
/// later refinements.
#[derive(Clone, Copy, Debug)]
pub struct AtmoConfig {
    /// Number of Fourier modes in the realisation.
    pub n_modes: usize,
    /// Correlation length `ℓ_c` [m] — sets the typical wavenumber `|k| ≈ 2π/ℓ_c`.
    pub correlation_length: f64,
    /// Density-perturbation RMS scale [kg/m³].
    pub amplitude: f64,
    /// Dispersion speed for `ω = c_s|k|` [m/s] (infrasound sound speed).
    pub sound_speed: f64,
}

impl Default for AtmoConfig {
    fn default() -> Self {
        AtmoConfig {
            n_modes: 32,
            correlation_length: 50.0,
            amplitude: 1e-6,
            sound_speed: 343.0,
        }
    }
}

/// One plane-wave density mode and its precomputed analytic potential coefficient.
#[derive(Clone, Copy, Debug)]
struct AtmoMode {
    k: [f64; 3],
    omega: f64,
    psi: f64,
    amp: f64,   // a_m — the density amplitude
    coeff: f64, // −4πG a_m / |k|² — the potential amplitude (analytic Poisson solution)
}

/// A realised atmospheric density field `δρ(x,t) = Σ a_m cos(k_m·x − ω_m t + ψ_m)`, exposed as a
/// [`gravity::FieldContribution`]: each mode's potential is the **analytic** plane-wave Poisson
/// solution `V_m = −4πG a_m/|k_m|² cos(·)`, so the field is a sum of closed forms — cheap, exact,
/// differentiable, and never a numerical Poisson solve. It lives in the **field** (forward pass), not
/// the post-hoc noise stack: its finite correlation length gives geometry-dependent partial common-mode.
pub struct AtmoField {
    modes: Vec<AtmoMode>,
}

impl AtmoField {
    /// Draw the modes **once** from the counter RNG (`key = seed ⊕ "atmo"`).
    pub fn realise(cfg: &AtmoConfig, seed: u64) -> Self {
        let mut rng = KeyRng::stream(seed, "atmo");
        let k_typ = TAU / cfg.correlation_length;
        // Per-mode amplitude so the field variance is ≈ amplitude² (Σ a²/2 = amplitude²).
        let a = cfg.amplitude * (2.0 / cfg.n_modes as f64).sqrt();
        let modes = (0..cfg.n_modes)
            .map(|_| {
                let kmag = k_typ * (0.4 + 2.0 * rng.next_unit()); // broadband, in [0.4, 2.4]·k_typ
                let cos_th = 2.0 * rng.next_unit() - 1.0;
                let sin_th = (1.0 - cos_th * cos_th).max(0.0).sqrt();
                let phi = TAU * rng.next_unit();
                let k = [
                    kmag * sin_th * phi.cos(),
                    kmag * sin_th * phi.sin(),
                    kmag * cos_th,
                ];
                AtmoMode {
                    k,
                    omega: cfg.sound_speed * kmag,
                    psi: TAU * rng.next_unit(),
                    amp: a,
                    coeff: -FOUR_PI_G * a / (kmag * kmag),
                }
            })
            .collect();
        AtmoField { modes }
    }

    /// The density perturbation `δρ(p,t) = Σ a_m cos(k·p − ωt + ψ)` — for the Poisson cross-check.
    pub fn density(&self, p: Vec3<f64>, t: f64) -> f64 {
        self.modes
            .iter()
            .map(|m| {
                let kp = p.x * m.k[0] + p.y * m.k[1] + p.z * m.k[2];
                m.amp * (kp - m.omega * t + m.psi).cos()
            })
            .sum()
    }
}

impl<S: Scalar> FieldContribution<S> for AtmoField {
    fn potential(&self, p: Vec3<S>, t: f64) -> S {
        let mut acc = S::from_f64(0.0);
        for m in &self.modes {
            let kp =
                p.x * S::from_f64(m.k[0]) + p.y * S::from_f64(m.k[1]) + p.z * S::from_f64(m.k[2]);
            let arg = kp + S::from_f64(m.psi - m.omega * t); // k·p − ωt + ψ
            acc = acc + S::from_f64(m.coeff) * arg.cos();
        }
        acc
    }

    fn gradient_tensor(&self, p: Vec3<S>, t: f64) -> Mat3<S> {
        // Γ_ij = −∂²V/∂x_i∂x_j = Σ_m coeff_m · k_i k_j · cos(arg).
        let mut m = [[S::from_f64(0.0); 3]; 3];
        for mode in &self.modes {
            let kp = p.x * S::from_f64(mode.k[0])
                + p.y * S::from_f64(mode.k[1])
                + p.z * S::from_f64(mode.k[2]);
            let arg = kp + S::from_f64(mode.psi - mode.omega * t);
            let factor = S::from_f64(mode.coeff) * arg.cos();
            for (i, row) in m.iter_mut().enumerate() {
                for (j, e) in row.iter_mut().enumerate() {
                    *e = *e + factor * S::from_f64(mode.k[i] * mode.k[j]);
                }
            }
        }
        Mat3 { m }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mean(noise: &[Vec<f64>]) -> f64 {
        let (mut s, mut n) = (0.0, 0.0);
        for row in noise {
            for &x in row {
                s += x;
                n += 1.0;
            }
        }
        s / n
    }

    #[test]
    fn noise_stack_order() {
        // Two non-commuting mock sources: the stack applies them in declared order.
        struct AddOne;
        impl NoiseSource for AddOne {
            fn add(&self, _t: &[f64], noise: &mut [Vec<f64>], _rng: &mut KeyRng) {
                for row in noise.iter_mut() {
                    for x in row.iter_mut() {
                        *x += 1.0;
                    }
                }
            }
        }
        struct Double;
        impl NoiseSource for Double {
            fn add(&self, _t: &[f64], noise: &mut [Vec<f64>], _rng: &mut KeyRng) {
                for row in noise.iter_mut() {
                    for x in row.iter_mut() {
                        *x *= 2.0;
                    }
                }
            }
        }
        let t = vec![0.0, 1.0, 2.0];
        let ab = NoiseStack(vec![Box::new(AddOne), Box::new(Double)]).realise(&t, 1, 0);
        let ba = NoiseStack(vec![Box::new(Double), Box::new(AddOne)]).realise(&t, 1, 0);
        assert_eq!(ab[0][0], 2.0); // (0+1)*2
        assert_eq!(ba[0][0], 1.0); // 0*2+1
        assert!(ab != ba, "output must depend on declared order");

        // Shot and vibration are each zero-mean over 1e5 draws (≤3σ of the mean estimator).
        let t: Vec<f64> = (0..100_000).map(|i| i as f64).collect();
        for src in [
            Box::new(ShotNoise { sigma: 1.0 }) as Box<dyn NoiseSource>,
            Box::new(VibrationResidual {
                sigma: 1.0,
                rho: 0.6,
                rejection: 1.0,
            }),
        ] {
            let noise = NoiseStack(vec![src]).realise(&t, 1, 7);
            let n = t.len() as f64;
            // The mean estimator's SD ≈ σ_eff/√n; ShotNoise σ_eff=1, vibration a few — bound generously.
            assert!(mean(&noise).abs() <= 3.0 * 5.0 / n.sqrt(), "not zero-mean");
        }
    }

    #[test]
    fn mode_potential_analytic() {
        // One plane-wave mode: V matches −4πG a/|k|²·cos(·), and ∇²V = 4πG δρ (4th-order FD), ≤1e-8 rel.
        let k = [0.4, 0.6, 0.3];
        let kmag2 = k[0] * k[0] + k[1] * k[1] + k[2] * k[2];
        let (a, omega, psi) = (1.0, 4.0, 0.6);
        let coeff = -FOUR_PI_G * a / kmag2;
        let field = AtmoField {
            modes: vec![AtmoMode {
                k,
                omega,
                psi,
                amp: a,
                coeff,
            }],
        };
        let p = Vec3::new(1.1, -0.4, 2.3);
        let t = 0.9;

        // V matches the closed form.
        let arg = k[0] * p.x + k[1] * p.y + k[2] * p.z - omega * t + psi;
        let v_closed = coeff * arg.cos();
        let v = FieldContribution::<f64>::potential(&field, p, t);
        assert!(
            (v - v_closed).abs() / v_closed.abs() <= 1e-8,
            "V_m mismatch"
        );

        // ∇²V = 4πG δρ by 4th-order central finite difference.
        let h = 1e-2;
        let vp = |dx: f64, dy: f64, dz: f64| {
            FieldContribution::<f64>::potential(&field, Vec3::new(p.x + dx, p.y + dy, p.z + dz), t)
        };
        let lap = |axis: usize| {
            let e = |s: f64| match axis {
                0 => vp(s, 0.0, 0.0),
                1 => vp(0.0, s, 0.0),
                _ => vp(0.0, 0.0, s),
            };
            (-e(-2.0 * h) + 16.0 * e(-h) - 30.0 * e(0.0) + 16.0 * e(h) - e(2.0 * h))
                / (12.0 * h * h)
        };
        let laplacian = lap(0) + lap(1) + lap(2);
        let rhs = FOUR_PI_G * field.density(p, t);
        assert!(
            (laplacian - rhs).abs() / rhs.abs() <= 1e-8,
            "Poisson ∇²V = 4πG δρ"
        );
    }

    #[test]
    fn noise_seeded() {
        let t: Vec<f64> = (0..64).map(|i| i as f64).collect();
        let stack = || {
            NoiseStack(vec![
                Box::new(ShotNoise { sigma: 1.0 }) as Box<dyn NoiseSource>
            ])
        };
        // Same seed → bit-identical realisation.
        assert_eq!(stack().realise(&t, 2, 42), stack().realise(&t, 2, 42));
        // Different seed → decorrelated (low sample correlation).
        let a = stack().realise(&t, 2, 42);
        let b = stack().realise(&t, 2, 43);
        let flat = |n: &[Vec<f64>]| n.iter().flatten().copied().collect::<Vec<f64>>();
        let (fa, fb) = (flat(&a), flat(&b));
        let corr: f64 = fa.iter().zip(&fb).map(|(x, y)| x * y).sum::<f64>()
            / (fa.iter().map(|x| x * x).sum::<f64>()).sqrt()
            / (fb.iter().map(|y| y * y).sum::<f64>()).sqrt();
        assert!(
            corr.abs() <= 0.2,
            "different seeds should decorrelate: corr {corr:.3}"
        );
    }
}
