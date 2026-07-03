//! `noise` — the `NoiseSource` stack and the atmospheric-GGN field source.
//!
//! Design: `design/noise.md`. Milestone: `milestones/M5-channels-and-decomposition.md`.
//!
//! M0 declares the `NoiseSource` seam with its contract; the post-hoc stack lands in M5.

/// A counter-based RNG key — deterministic streams keyed by `(seed, scenario, stream)`.
///
/// Opaque in M0; the counter-based generator is finalised in M5. Held here so the seam's
/// determinism guarantee is expressible now.
pub struct KeyRng;

/// One additive term in the post-hoc noise stack.
///
/// # Contract (spec `sec:contracts`, `NoiseSource`)
/// - **Method.** `add(t, clean, rng)` — additive, in place.
/// - **Pre.** `clean` of length N; `rng` seeded.
/// - **Post.** Additive (`out = clean + noise`); deterministic given the seed; zero-mean for shot
///   and vibration. Order in the stack is significant and preserved.
pub trait NoiseSource {
    /// Add this term's noise to `clean` in place, sampled at times `t`.
    fn add(&self, t: &[f64], clean: &mut [f64], rng: &mut KeyRng);
}
