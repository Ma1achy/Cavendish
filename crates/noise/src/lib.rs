//! `noise` — the post-hoc `NoiseSource` stack and the atmospheric-GGN field source.
//!
//! Design: `design/noise.md`. Milestone: `milestones/M5-channels-and-decomposition.md`.
//!
//! Two kinds, one boundary: **additive measurement noise** (shot, vibration) perturbs the *measured*
//! phase and is applied post-hoc via [`NoiseSource::add`]; **atmospheric GGN** is a real gravitational
//! field (a stochastic `δρ`) and enters the forward pass as a [`gravity::FieldContribution`] (Commit
//! 3), never here. Determinism is via a counter-based keyed RNG: seeds compose through the key tree,
//! never sequential state (spec `nfr:reproducibility`).

const TAU: f64 = std::f64::consts::TAU;

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
