//! Cadence: when the shots of a run are taken, and what that does to a spectrum.
//!
//! Three things, all pure `f64` mathematics with no state:
//!
//! - **keyed random numbers** ([`keyed`], [`unit()`]): a 64-bit mix of `(seed, stream, tower, shot)`.
//!   A number depends on its key alone, so the order of evaluation and the number of threads cannot
//!   change it (`docs/research/s2-replay-spec-paths-cadence.md` §2.4);
//! - **the spectral window** of a list of shot times, `W(ν) = (1/N)·Σₙ e^{−2πiν·tₙ}`: a unit tone at
//!   frequency `f` sampled at those times has the classical periodogram `|W(ν − f)|²`. [`window`]
//!   and [`window_fast`] give `|W|²` of one realisation;
//! - **closed forms** for the *expected* window of each kind of cadence (uniform, cycle jitter as a
//!   random walk, jitter about a grid, alternation, planned gaps, lost shots), with the small
//!   schedule generators needed to check them by Monte Carlo. The forms are the note's own
//!   derivations (§4.2); the Monte Carlo cases beside the tests are their check.
//!
//! Units throughout: times and cycle times in seconds, frequencies in hertz, windows as a fraction
//! of the peak (`|W|² = 1` at `ν = 0`). `|W|²` is a power; a line's *height* is `|W|`.
//!
//! Nothing here bounds its own work: a caller that takes a count from a scene or a command checks
//! it first (`docs/slices/S2.md` §3.1, rows A4 and A13).

use crate::spectrum::{Complex, NUFFT_WIDTH, nufft1};
use std::f64::consts::{PI, TAU};

/// The stream of [`keyed`] numbers that jitter is drawn from.
pub const STREAM_JITTER: u64 = 1;

/// The stream of [`keyed`] numbers that decides which shots are lost.
pub const STREAM_LOSS: u64 = 2;

/// SplitMix64's increment: 2⁶⁴ over the golden ratio.
const GOLDEN: u64 = 0x9e37_79b9_7f4a_7c15;

/// SplitMix64's output function (Stafford's variant 13): a bijection of the 64-bit words in which
/// every input bit reaches every output bit.
const fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The random 64-bit word of a key: four rounds of SplitMix64's mix, one for each of `seed`,
/// `stream` (what the number is for: [`STREAM_JITTER`], [`STREAM_LOSS`]), `tower` and `shot`.
///
/// There is no state: the same key gives the same word whenever and on whichever thread it is
/// asked for, on every platform (integer arithmetic only). Each round is a bijection of the word
/// it takes in, so two keys that differ in one of the four words never give the same number.
///
/// The words are part of what a scene with a seed means: changing this function changes every
/// jittered schedule ever saved. Not for cryptography, and not a stream: there is no "next"
/// number, only another key.
#[must_use]
pub const fn keyed(seed: u64, stream: u64, tower: u64, shot: u64) -> u64 {
    let mut word = mix(seed.wrapping_add(GOLDEN));
    word = mix((word ^ stream).wrapping_add(GOLDEN));
    word = mix((word ^ tower).wrapping_add(GOLDEN));
    mix((word ^ shot).wrapping_add(GOLDEN))
}

/// The [`keyed`] number of a key as a real number uniform in `[0, 1)`: its top 53 bits over 2⁵³,
/// so every value is a multiple of 2⁻⁵³ and 1 is never returned. Dimensionless.
///
/// Not for a draw that must be able to return 1, nor for more than 53 bits of randomness.
#[must_use]
pub fn unit(seed: u64, stream: u64, tower: u64, shot: u64) -> f64 {
    (keyed(seed, stream, tower, shot) >> 11) as f64 / (1u64 << 53) as f64
}

/// `|W(ν)|²` of shot times (seconds) at one frequency (hertz), by the sum itself. The phase of each
/// term is taken modulo a turn before it is scaled by 2π, so a long run loses no digits to the size
/// of `ν·t`.
fn power_at(times: &[f64], frequency: f64) -> f64 {
    if times.is_empty() {
        return 0.0;
    }
    let (mut re, mut im) = (0.0, 0.0);
    for &time in times {
        let turns = frequency * time;
        let (s, c) = (TAU * (turns - turns.round())).sin_cos();
        re += c;
        im -= s;
    }
    let n = times.len() as f64;
    (re * re + im * im) / (n * n)
}

/// The spectral window of one realisation: `|W(ν)|²` with `W(ν) = (1/N)·Σₙ e^{−2πiν·tₙ}`, for shot
/// `times` in seconds (any order, need not be uniform) at each of `frequencies` in hertz. A
/// fraction of the peak: 1 at `ν = 0`. No times give zeros.
///
/// Exact, in `N·M` steps for `N` times and `M` frequencies: the caller bounds `N·M`
/// (`docs/slices/S2.md` §3.1 row A13 allows 2e7) and uses [`window_fast`] beyond that. Not the
/// *expected* window of a random cadence: that is the closed forms below.
#[must_use]
pub fn window(times: &[f64], frequencies: &[f64]) -> Vec<f64> {
    frequencies.iter().map(|&frequency| power_at(times, frequency)).collect()
}

/// [`window`] at the frequencies `k·df` (hertz), `k = 0 … count − 1`, from one non-uniform FFT
/// ([`nufft1`], kernel [`NUFFT_WIDTH`] points wide) in about `14·N + M·log M` steps. `times` in
/// seconds, in any order, need not be uniform.
///
/// Held to [`window`] to 1e-9 of the peak (the transform's own error is near 1e-13). Memory is the
/// transform's grid, `64·count` bytes rounded up to a power of two: the caller bounds `count`. Not
/// for a list of arbitrary frequencies (use [`window`]).
#[must_use]
pub fn window_fast(times: &[f64], df: f64, count: usize) -> Vec<f64> {
    let Some(&first) = times.first() else {
        return vec![0.0; count];
    };
    // Times from the first one: |W| does not depend on the origin, and small angles keep their digits.
    let x: Vec<f64> = times.iter().map(|time| TAU * df * (time - first)).collect();
    let ones = vec![Complex::new(1.0, 0.0); times.len()];
    let n = times.len() as f64;
    nufft1(&x, &ones, count, NUFFT_WIDTH).iter().map(|sum| sum.norm_squared() / (n * n)).collect()
}

/// `sin(πx)/(πx)`, 1 at `x = 0`.
fn sinc(x: f64) -> f64 {
    if x == 0.0 { 1.0 } else { (PI * x).sin() / (PI * x) }
}

/// `e^{−2πi·turns}`, with the whole turns taken off first.
fn turned(turns: f64) -> Complex {
    Complex::unit(-TAU * (turns - turns.round()))
}

fn scaled(z: Complex, by: f64) -> Complex {
    Complex::new(z.re * by, z.im * by)
}

/// The window of a **uniform** cadence, `n` shots every `t0` seconds, at `nu` hertz: the Dirichlet
/// kernel squared,
///
/// ```text
/// |W|² = | sin(N·π·ν·T₀) / (N·sin(π·ν·T₀)) |²
/// ```
///
/// with its limit 1 at the lines `ν = k/T₀` (an alias as tall as the peak, `1/(N·T₀)` wide). The
/// product `ν·T₀` is taken modulo one first, so the lines are found exactly. A power; the line's
/// height is its square root. `n = 0` gives 0.
///
/// Not for shots that are not on a grid: any jitter, gap or loss changes it.
#[must_use]
pub fn dirichlet(nu: f64, n: usize, t0: f64) -> f64 {
    if n == 0 {
        return 0.0;
    }
    let turns = nu * t0;
    let x = turns - turns.round();
    let bottom = n as f64 * (PI * x).sin();
    if bottom == 0.0 {
        return 1.0;
    }
    let ratio = (n as f64 * PI * x).sin() / bottom;
    ratio * ratio
}

/// The mean cycle time `T̄` in seconds of a jittered cycle: `t0 + delta/2` when the jitter is
/// positive only (a cycle uniform in `[t0, t0 + delta]`), `t0` when it is symmetric (uniform in
/// `[t0 − delta, t0 + delta]`). The mean-rate alias of a cadence is at `1/T̄` hertz.
#[must_use]
pub fn mean_cycle(t0: f64, delta: f64, symmetric: bool) -> f64 {
    if symmetric { t0 } else { t0 + 0.5 * delta }
}

/// The characteristic function `φ(ν) = E e^{−2πiν·T}` of a cycle time `T` uniform in
/// `[t0, t0 + delta]` seconds (**positive-only** jitter: a shot can be late, never early), at `nu`
/// hertz:
///
/// ```text
/// φ = e^{−2πiν(T₀ + δ/2)} · sinc(ν·δ),      sinc x = sin(πx)/(πx)
/// ```
///
/// Complex and dimensionless, `|φ| ≤ 1`; real at the mean-rate alias `ν = 1/(T₀ + δ/2)`. For
/// [`cycle_jitter_window`]. Not the characteristic function of a shot's *time* about a grid: that
/// is [`psi_positive`].
#[must_use]
pub fn phi_positive(nu: f64, t0: f64, delta: f64) -> Complex {
    scaled(turned(nu * (t0 + 0.5 * delta)), sinc(nu * delta))
}

/// The characteristic function `φ(ν) = E e^{−2πiν·T}` of a cycle time `T` uniform in
/// `[t0 − delta, t0 + delta]` seconds (**symmetric** jitter), at `nu` hertz:
///
/// ```text
/// φ = e^{−2πiν·T₀} · sinc(2ν·δ)
/// ```
///
/// Complex and dimensionless; real at the alias `ν = 1/T₀`. For [`cycle_jitter_window`]. Not the
/// characteristic function of a shot's time about a grid: that is [`psi_uniform`].
#[must_use]
pub fn phi_symmetric(nu: f64, t0: f64, delta: f64) -> Complex {
    scaled(turned(nu * t0), sinc(2.0 * nu * delta))
}

/// `zⁿ` by repeated squaring: multiplications only, so the same bits on every platform.
fn power(z: Complex, mut n: usize) -> Complex {
    let (mut result, mut square) = (Complex::new(1.0, 0.0), z);
    while n > 0 {
        if n & 1 == 1 {
            result = result.times(square);
        }
        square = square.times(square);
        n >>= 1;
    }
    result
}

/// `Σₖ₌₁ᴺ⁻¹ (N − k)·φᵏ`, the sum inside [`cycle_jitter_window`].
///
/// Away from `φ = 1` it is the closed form `φ·(N(1 − φ) − (1 − φᴺ)) / (1 − φ)²`. In that form the
/// bracket is the difference of two numbers that agree to a part in `N|1 − φ|/2`, so near `φ = 1`
/// (low frequencies, or a line of a cadence with no jitter) it would lose every digit. There, for
/// `N|1 − φ| < 0.1`, the whole is summed as its binomial series instead,
/// `φ·Σⱼ₌₂ᴺ C(N, j)·(φ − 1)ʲ⁻²`, whose terms fall by at least thirty at each step and do not
/// cancel (twenty of them are more than enough). At the changeover the closed form has lost a factor of twenty, about 1e-15.
fn walk_sum(phi: Complex, n: usize) -> Complex {
    let count = n as f64;
    // 1 − φ is exact for any φ near 1 (Sterbenz).
    let gap = Complex::new(1.0 - phi.re, -phi.im);
    let size = gap.norm_squared();
    if count * size.sqrt() < 0.1 {
        let mut term = Complex::new(0.5 * count * (count - 1.0), 0.0);
        let mut sum = term;
        for j in 2..n.min(20) {
            term = scaled(term.times(gap), -(count - j as f64) / (j as f64 + 1.0));
            sum = Complex::new(sum.re + term.re, sum.im + term.im);
        }
        return phi.times(sum);
    }
    let high = power(phi, n);
    let bracket = Complex::new(count * gap.re - (1.0 - high.re), count * gap.im + high.im);
    // Divided by (1 − φ)²: multiplied by its conjugate squared over |1 − φ|⁴.
    let inverse = scaled(gap.conjugate().times(gap.conjugate()), 1.0 / (size * size));
    phi.times(bracket).times(inverse)
}

/// The expected window `E|W(ν)|²` of `n` shots whose **cycle times are independent and alike**, so
/// that the shot times are a random walk (a late shot delays every later one). `phi` is the cycle
/// time's characteristic function at `ν` ([`phi_positive`], [`phi_symmetric`]):
///
/// ```text
/// E|W|² = (1/N²)·[ N + 2·Re Σₖ₌₁ᴺ⁻¹ (N − k)·φᵏ ]
///       = (1/N²)·[ N + 2·Re( φ·(N(1 − φ) − (1 − φᴺ)) / (1 − φ)² ) ]
/// ```
///
/// since the time between shots `m` and `m + k` is a sum of `k` cycles. Evaluated without the
/// closed form's cancellation near `φ = 1` (its binomial series there), to about `N·1e-16`, which
/// is all that a `φ` known to 1e-16 determines. At the mean-rate alias `φ` is real and just below
/// 1, and for large `N(1 − φ)` the line has broadened to `E|W|² ≈ 2/(N(1 − φ))`: its height falls
/// as `1/√N`. A power, as a fraction of the peak; `n = 0` gives 0.
///
/// An expectation over cadences, not any one realisation (whose `|W|²` at a broadened line
/// scatters by as much as its mean), and not for jitter about a grid, where a late shot delays
/// nothing: that is [`grid_jitter_window`].
#[must_use]
pub fn cycle_jitter_window(phi: Complex, n: usize) -> f64 {
    if n == 0 {
        return 0.0;
    }
    let count = n as f64;
    ((count + 2.0 * walk_sum(phi, n).re) / (count * count)).max(0.0)
}

/// The floor of [`cycle_jitter_window`]: its part that falls as `1/N`,
///
/// ```text
/// (1 − |φ|²) / (N·|1 − φ|²)
/// ```
///
/// the whole of `E|W|²` but for the remainder `−2·Re(φ(1 − φᴺ)/(1 − φ)²)/N²`, which is at most
/// `4/(N|1 − φ|)²`. A power, as a fraction of the peak. Not for use on or near a line (`φ → 1`),
/// where the remainder is the larger part and this grows past 1: use [`cycle_jitter_window`]
/// there.
#[must_use]
pub fn cycle_jitter_floor(phi: Complex, n: usize) -> f64 {
    let gap = Complex::new(1.0 - phi.re, -phi.im).norm_squared();
    (1.0 - phi.norm_squared()) / (n as f64 * gap)
}

/// The predicted height of the mean-rate alias of a random-walk cadence against the peak: the
/// root of [`cycle_jitter_window`] with `phi_at_alias` the cycle's characteristic function at
/// `ν = 1/T̄` hertz. Dimensionless; it falls towards `√(2/(N(1 − φ)))`, a slope of −½ in `N`.
///
/// The root of the mean power: not the mean of `|W|`, and not the largest `√S` in a window about
/// the alias, which for a noisy line sits above it (the reference scripts take that maximum).
#[must_use]
pub fn alias_ratio(phi_at_alias: Complex, n: usize) -> f64 {
    cycle_jitter_window(phi_at_alias, n).sqrt()
}

/// The expected window `E|W(ν)|²` of `n` shots **jittered about a grid**, `tₙ = n·T₀ + εₙ` with
/// the `εₙ` independent and alike (each shot is late on its own; the clock keeps time):
///
/// ```text
/// E|W|² = |ψ|²·|D_N|² + (1 − |ψ|²)/N
/// ```
///
/// with `psi_abs_sq = |ψ(ν)|²` the squared size of the jitter's characteristic function
/// ([`psi_uniform`], [`psi_positive`]) and `dirichlet_sq = |D_N(ν)|²` the grid's own window
/// ([`dirichlet`]). The lines keep the height `|ψ(k/T₀)|` for any `N` (a Debye–Waller factor);
/// only the floor falls. A power, as a fraction of the peak.
///
/// Not for cycle jitter, where the lines broaden and recede: that is [`cycle_jitter_window`].
#[must_use]
pub fn grid_jitter_window(psi_abs_sq: f64, dirichlet_sq: f64, n: usize) -> f64 {
    psi_abs_sq * dirichlet_sq + (1.0 - psi_abs_sq) / n as f64
}

/// The characteristic function `ψ(ν) = E e^{−2πiν·ε}` of a shot's lateness `ε` uniform in
/// `[−delta, delta]` seconds, at `nu` hertz: `ψ = sinc(2ν·δ)`, real. Dimensionless. For
/// [`grid_jitter_window`], squared. Not a cycle time's characteristic function ([`phi_symmetric`]).
#[must_use]
pub fn psi_uniform(nu: f64, delta: f64) -> f64 {
    sinc(2.0 * nu * delta)
}

/// The characteristic function `ψ(ν) = E e^{−2πiν·ε}` of a shot's lateness `ε` uniform in
/// `[0, delta]` seconds (late, never early), at `nu` hertz: `ψ = e^{−iπν·δ}·sinc(ν·δ)`, complex.
/// Dimensionless. For [`grid_jitter_window`] as `ψ.norm_squared()`. Not a cycle time's
/// characteristic function ([`phi_positive`]).
#[must_use]
pub fn psi_positive(nu: f64, delta: f64) -> Complex {
    scaled(turned(0.5 * nu * delta), sinc(nu * delta))
}

/// The height `|W|` of the `k`-th sideband of an **alternating** cadence, cycles of `t_a` and
/// `t_b` seconds in turn, at `ν = k/T_s` hertz with `T_s = T_A + T_B`:
///
/// ```text
/// |W(k/T_s)| = |cos(π·k·T_A / T_s)|
/// ```
///
/// (two shots a period, at 0 and `T_A`). It does not depend on the number of shots: these lines
/// never recede. For `T_A ≈ T_B` the odd `k` are small and the even ones near 1. A height (an
/// amplitude against the peak), not a power; exact for a whole number of periods, and not the
/// window between the lines.
#[must_use]
pub fn alternating_sideband(k: usize, t_a: f64, t_b: f64) -> f64 {
    (PI * k as f64 * t_a / (t_a + t_b)).cos().abs()
}

/// The height `|W|` of the `k`-th line of a cadence with **planned gaps**, `on` shots taken of
/// every `period` slots of a grid of `T₀` seconds, at `ν = k/(P·T₀)` hertz:
///
/// ```text
/// |W| = | sin(π·k·L/P) / (L·sin(π·k/P)) |
/// ```
///
/// with the limit 1 where `k` is a multiple of `P` (the grid's own lines), tending to
/// `|sinc(k·L/P)|` as `P` grows with `L/P` fixed. It does not depend on the number of shots. A
/// height, not a power; exact for a whole number of periods; 0 when `on` or `period` is 0. Not for
/// gaps that do not repeat (a daily window on a jittered cadence).
#[must_use]
pub fn gapped_line(k: usize, on: usize, period: usize) -> f64 {
    if on == 0 || period == 0 {
        return 0.0;
    }
    if k.is_multiple_of(period) {
        return 1.0;
    }
    // Whole turns taken off in integers: only k·L and k modulo P matter.
    let sine = |m: u128| (PI * (m % period as u128) as f64 / period as f64).sin();
    (sine(k as u128 * on as u128) / (on as f64 * sine(k as u128))).abs()
}

/// The flat floor that **lost shots** add to a window: each of `n` planned shots is lost
/// independently with probability `p`, and
///
/// ```text
/// E|W|² = |W₀|² + p / ((1 − p)·N)
/// ```
///
/// where `W₀` is the window of the plan with nothing lost. This function is the second term; it is
/// white (the same at every frequency) and falls only as `1/N`.
///
/// **Which `N`.** With `bₙ` = 1 for a kept shot, `E|Σ bₙeₙ|² = (1 − p)²·|Σ eₙ|² + p(1 − p)·N`;
/// dividing by the square of the kept count `K ≈ (1 − p)·N` gives the form above. So `n` is the
/// number of shots *planned* (kept and lost together), and the window is normalised by the shots
/// *kept*, as [`window`] of the surviving times is; in terms of the kept count the floor is `p/K`.
/// Normalised by the planned count instead it would be `p(1 − p)/N`, smaller by `(1 − p)²`.
///
/// `p` in `[0, 1)`. A power, as a fraction of the peak. Not the whole window: between the lines of
/// a uniform plan `|W₀|²` is its sidelobes, about `1/(2N²·sin²(πν·T₀))`, which at `N` = 4000 and
/// `p` = 0.01 are still a few per cent of this floor.
#[must_use]
pub fn loss_floor(p: f64, n: usize) -> f64 {
    p / ((1.0 - p) * n as f64)
}

/// A **uniform** cadence: `n` shot times in seconds, `k·t0` for `k = 0 … n − 1`.
///
/// Allocates `n` values: the caller bounds `n`. Launch times on the array's clock, with no tower's
/// offset or flight time added.
#[must_use]
pub fn uniform_times(n: usize, t0: f64) -> Vec<f64> {
    (0..n).map(|k| k as f64 * t0).collect()
}

/// **Cycle jitter**: `n` shot times in seconds that random-walk. The first is 0 and the cycle after
/// shot `k` lasts `t0 + delta·u` (positive only) or `t0 + delta·(2u − 1)` (`symmetric`), with
/// `u = unit(seed, STREAM_JITTER, 0, k)`, so a late shot delays every later one. The mean cycle is
/// [`mean_cycle`]; the expected window is [`cycle_jitter_window`].
///
/// Allocates `n` values: the caller bounds `n`, and keeps `delta < t0` if `symmetric` (a cycle
/// must not be negative; whether it is long enough for a shot is the scene's to check). The times
/// are summed in shot order, so they are the same bits however they are asked for. Not for jitter
/// about a grid ([`grid_times`]).
#[must_use]
pub fn walk_times(n: usize, t0: f64, delta: f64, symmetric: bool, seed: u64) -> Vec<f64> {
    let mut now = 0.0;
    (0..n as u64)
        .map(|k| {
            let time = now;
            now += t0 + delta * centred(unit(seed, STREAM_JITTER, 0, k), symmetric);
            time
        })
        .collect()
}

/// A draw in `[0, 1)` as it is, or stretched to `[−1, 1)`.
fn centred(u: f64, symmetric: bool) -> f64 {
    if symmetric { 2.0 * u - 1.0 } else { u }
}

/// **Jitter about a grid**: `n` shot times in seconds, `k·t0 + ε_k` with `ε_k = delta·u` (late
/// only) or `delta·(2u − 1)` (`symmetric`), `u = unit(seed, STREAM_JITTER, 0, k)`. Each shot is
/// late on its own; the grid keeps time. The expected window is [`grid_jitter_window`].
///
/// Allocates `n` values: the caller bounds `n`. The times are in rising order only while the
/// jitter is less than half a cycle (`symmetric`) or a cycle. Not for cycle jitter
/// ([`walk_times`]).
#[must_use]
pub fn grid_times(n: usize, t0: f64, delta: f64, symmetric: bool, seed: u64) -> Vec<f64> {
    (0..n as u64)
        .map(|k| k as f64 * t0 + delta * centred(unit(seed, STREAM_JITTER, 0, k), symmetric))
        .collect()
}

/// An **alternating** cadence: `n` shot times in seconds, cycles of `t_a` then `t_b` in turn from
/// 0: `0, T_A, T_s, T_s + T_A, …` with `T_s = T_A + T_B`. Each time is a product, not a running
/// sum. The sidebands are [`alternating_sideband`].
///
/// Allocates `n` values: the caller bounds `n`.
#[must_use]
pub fn alternating_times(n: usize, t_a: f64, t_b: f64) -> Vec<f64> {
    (0..n).map(|k| (k / 2) as f64 * (t_a + t_b) + (k % 2) as f64 * t_a).collect()
}

/// **Planned gaps**: of `slots` slots every `t0` seconds, the times in seconds of those taken, the
/// first `on` of every `period` (slot `k` is taken when `k mod period < on`). The lines are
/// [`gapped_line`]. `period = 0` takes nothing.
///
/// Allocates up to `slots` values: the caller bounds `slots`. The closed form is exact when
/// `slots` is a multiple of `period`.
#[must_use]
pub fn gapped_times(slots: usize, t0: f64, on: usize, period: usize) -> Vec<f64> {
    if period == 0 {
        return Vec::new();
    }
    (0..slots).filter(|k| k % period < on).map(|k| k as f64 * t0).collect()
}

/// **Lost shots**: the `times` (seconds) that survive when each is lost independently with
/// probability `p`. Shot `k` of the list is kept when `unit(seed, STREAM_LOSS, 0, k) ≥ p`, so
/// whether a shot is lost depends on its own index alone. The floor this adds is [`loss_floor`].
///
/// Allocates up to `times.len()` values. `p ≤ 0` keeps every shot and `p ≥ 1` none. Not for
/// planned gaps ([`gapped_times`]).
#[must_use]
pub fn kept_times(times: &[f64], p: f64, seed: u64) -> Vec<f64> {
    times
        .iter()
        .zip(0u64..)
        .filter(|&(_, k)| unit(seed, STREAM_LOSS, 0, k) >= p)
        .map(|(&time, _)| time)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyed_numbers_do_not_depend_on_order() {
        // The mix is SplitMix64's: its first output from a seed of zero is the published one.
        assert_eq!(mix(GOLDEN), 0xe220_a839_7b1d_cdaf);
        let keys: Vec<(u64, u64, u64, u64)> = (0..4000u64)
            .map(|i| (i % 5, STREAM_JITTER + i % 2, i % 7, i))
            .chain([(u64::MAX, u64::MAX, u64::MAX, u64::MAX), (0, 0, 0, 0)])
            .collect();
        let of = |&(seed, stream, tower, shot): &(u64, u64, u64, u64)| keyed(seed, stream, tower, shot);
        let forwards: Vec<u64> = keys.iter().map(of).collect();
        // Backwards, and on four threads each taking every fourth key.
        let mut backwards: Vec<u64> = keys.iter().rev().map(of).collect();
        backwards.reverse();
        assert_eq!(forwards, backwards);
        let mut threaded = vec![0u64; keys.len()];
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..4)
                .map(|lane| {
                    let keys = &keys;
                    scope.spawn(move || {
                        keys.iter()
                            .enumerate()
                            .skip(lane)
                            .step_by(4)
                            .map(|(i, key)| (i, of(key)))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            for handle in handles {
                for (i, word) in handle.join().unwrap() {
                    threaded[i] = word;
                }
            }
        });
        assert_eq!(forwards, threaded);
        // Different keys, different numbers.
        let mut sorted = forwards.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), keys.len());
        // Changing any one of the four words changes the number, by a bit or by all of them.
        for &(seed, stream, tower, shot) in &keys[..500] {
            let here = keyed(seed, stream, tower, shot);
            for change in [1u64, 1 << 17, 1 << 63, u64::MAX] {
                assert_ne!(here, keyed(seed ^ change, stream, tower, shot));
                assert_ne!(here, keyed(seed, stream ^ change, tower, shot));
                assert_ne!(here, keyed(seed, stream, tower ^ change, shot));
                assert_ne!(here, keyed(seed, stream, tower, shot ^ change));
            }
        }
        // The words do not stand in for one another.
        assert_ne!(keyed(1, 2, 3, 4), keyed(4, 3, 2, 1));
        assert_ne!(keyed(0, 1, 0, 0), keyed(0, 0, 1, 0));
        // unit is the top 53 bits: in [0, 1), and the largest word is just below 1.
        assert!(keys.iter().all(|&(a, b, c, d)| (0.0..1.0).contains(&unit(a, b, c, d))));
        assert_eq!((u64::MAX >> 11) as f64 / (1u64 << 53) as f64, 1.0 - f64::EPSILON / 2.0);
        // Control: a generator with state gives a key a different number when the order changes.
        let stream = |order: &[usize]| {
            let mut state = 7u64;
            let mut words = vec![0u64; order.len()];
            for &i in order {
                state = state.wrapping_add(GOLDEN);
                words[i] = mix(state);
            }
            words
        };
        assert_ne!(stream(&[0, 1, 2, 3]), stream(&[3, 2, 1, 0]));
    }

    /// How far 10⁵ draws are from uniform and independent, each in its own standard errors: the
    /// mean from 1/2, the variance (about 1/2) from 1/12, and the correlation of each draw with the
    /// next from 0.
    fn uniformity(draw: impl Fn(u64) -> f64) -> [f64; 3] {
        let n = 100_000u64;
        let centred: Vec<f64> = (0..n).map(|k| draw(k) - 0.5).collect();
        let count = n as f64;
        let mean = centred.iter().sum::<f64>() / count;
        let variance = centred.iter().map(|u| u * u).sum::<f64>() / count;
        let lagged = centred.windows(2).map(|pair| pair[0] * pair[1]).sum::<f64>() / (count - 1.0);
        [
            mean / (1.0 / 12.0 / count).sqrt(),
            // (u − ½)² has mean 1/12 and variance 1/80 − 1/144 = 1/180.
            (variance - 1.0 / 12.0) / (1.0 / 180.0 / count).sqrt(),
            // The correlation coefficient of independent draws has standard error 1/√n.
            12.0 * lagged * (count - 1.0).sqrt(),
        ]
    }

    #[test]
    fn keyed_numbers_are_uniform() {
        let within = |z: [f64; 3]| z.iter().all(|z| z.abs() <= 4.0);
        // Along each of the four words in turn, the others held.
        let along: [(&str, [f64; 3]); 4] = [
            ("shot", uniformity(|k| unit(2026, STREAM_JITTER, 0, k))),
            ("tower", uniformity(|k| unit(2026, STREAM_JITTER, k, 17))),
            ("stream", uniformity(|k| unit(2026, k, 3, 17))),
            ("seed", uniformity(|k| unit(k, STREAM_LOSS, 3, 17))),
        ];
        for (word, z) in along {
            eprintln!(
                "keyed numbers along {word}: mean {:+.2}, variance {:+.2}, lag-one correlation {:+.2} standard errors",
                z[0], z[1], z[2]
            );
            assert!(within(z), "along {word}: {z:?} standard errors");
        }
        // Controls: a mix that is no mix fails on its mean, and a Weyl sequence (the counter times
        // the golden ratio, unmixed) has the right mean and variance but each draw gives away the next.
        let lazy = uniformity(|k| (k % 7) as f64 / 7.0);
        assert!(lazy[0].abs() > 4.0, "{lazy:?}");
        let weyl = uniformity(|k| (k.wrapping_mul(GOLDEN) >> 11) as f64 / (1u64 << 53) as f64);
        assert!(weyl[0].abs() <= 4.0 && weyl[2].abs() > 4.0, "{weyl:?}");
        assert!(!within(lazy) && !within(weyl));
    }

    /// The mean of samples and the standard error of that mean, measured from their scatter.
    fn mean_and_error(samples: &[f64]) -> (f64, f64) {
        let n = samples.len() as f64;
        let mean = samples.iter().sum::<f64>() / n;
        let variance = samples.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / (n - 1.0);
        (mean, (variance / n).sqrt())
    }

    #[test]
    fn the_fast_window_is_the_direct_one() {
        let (df, count) = (0.5 / 700.0, 701);
        let frequencies: Vec<f64> = (0..count).map(|k| k as f64 * df).collect();
        let apart = |times: &[f64]| {
            window(times, &frequencies)
                .iter()
                .zip(window_fast(times, df, count))
                .map(|(a, b)| (a - b).abs())
                .fold(0.0f64, f64::max)
        };
        // A walk, shots about a grid out of order, a gapped and thinned grid, far from the origin.
        let mut shuffled = grid_times(1500, 5.0, 4.0, true, 3);
        shuffled.reverse();
        let gapped: Vec<f64> =
            kept_times(&gapped_times(2000, 5.0, 4, 10), 0.05, 9).iter().map(|t| t + 1e6).collect();
        for (what, times) in
            [("walk", walk_times(1500, 5.0, 0.3, false, 1)), ("shuffled", shuffled), ("gapped", gapped)]
        {
            let worst = apart(&times);
            eprintln!("window_fast against window, {what}: {worst:e}");
            assert!(worst <= 1e-9, "{what}: {worst:e}");
        }
        assert_eq!(window(&[], &[0.1, 0.2]), vec![0.0; 2]);
        assert_eq!(window_fast(&[], 0.1, 3), vec![0.0; 3]);
        assert_eq!(window(&[3.0], &[0.0, 0.123]), vec![1.0; 2]);
        // Control: the window of other times (stretched by a thousandth) is another window.
        let times = walk_times(1500, 5.0, 0.3, false, 1);
        let stretched: Vec<f64> = times.iter().map(|t| t * 1.001).collect();
        let other = window_fast(&stretched, df, count);
        let worst =
            window(&times, &frequencies).iter().zip(other).map(|(a, b)| (a - b).abs()).fold(0.0f64, f64::max);
        assert!(worst > 1e-3, "{worst:e}");
    }

    /// The largest difference between `|W|` of a uniform cadence and the Dirichlet kernel: 200
    /// shots every 5 s, 257 frequencies from 0 to 0.8 Hz, the lines at 0, 0.2, … 0.8 Hz among them.
    fn uniform_against_dirichlet() -> f64 {
        let (n, t0) = (200, 5.0);
        let frequencies: Vec<f64> = (0..=256).map(|j| f64::from(j) / 320.0).collect();
        let worst = |times: &[f64]| {
            window(times, &frequencies)
                .iter()
                .zip(&frequencies)
                .map(|(w, &nu)| (w.sqrt() - dirichlet(nu, n, t0).sqrt()).abs())
                .fold(0.0f64, f64::max)
        };
        // The lines are among the frequencies and are 1 there.
        for line in [0.0, 0.2, 0.4, 0.6, 0.8] {
            assert!(frequencies.contains(&line));
            assert_eq!(dirichlet(line, n, t0), 1.0);
        }
        // Control: a twentieth of a second of cycle jitter, or one shot more, is not this kernel.
        assert!(worst(&walk_times(n, t0, 0.05, false, 1)) > 1e-2);
        assert!(worst(&uniform_times(n + 1, t0)) > 1e-3);
        let error = worst(&uniform_times(n, t0));
        eprintln!("uniform window against the Dirichlet kernel: {error:e}");
        error
    }

    cav_validate::case! {
        test: validation_window_uniform_is_dirichlet,
        name: "cadence.window_uniform",
        subsystem: "cadence",
        slice: "S2",
        reference: ClosedForm,
        against: "the spectral window |W| of 200 shots every 5 s by direct sum against the Dirichlet kernel |sin(N pi nu T0)/(N sin(pi nu T0))| at 257 frequencies from 0 to 0.8 Hz, the five lines among them; largest absolute difference of |W| (the peak is 1)",
        unit: Relative,
        tolerance: 1e-12,
        measure: cav_validate::Measured::Error(uniform_against_dirichlet()),
    }

    /// The root of the mean of `|W|²` at one frequency over the first `seeds` seeds of a cadence.
    fn rms_height(seeds: u64, nu: f64, times: impl Fn(u64) -> Vec<f64>) -> f64 {
        ((0..seeds).map(|seed| power_at(&times(seed), nu)).sum::<f64>() / seeds as f64).sqrt()
    }

    /// Cycles of 5.5 s and 6.0 s in turn: the heights of the sidebands `k` = 1 … 4 at 2000 shots
    /// against `|cos(πkT_A/T_s)|`, and against themselves at 20 000 shots; the larger difference.
    fn alternating_sidebands() -> f64 {
        let (t_a, t_b, n) = (5.5, 6.0, 2000);
        let (short, long) = (alternating_times(n, t_a, t_b), alternating_times(10 * n, t_a, t_b));
        let mut worst = 0.0f64;
        for k in 1..=4 {
            let nu = k as f64 / (t_a + t_b);
            let (here, there) = (power_at(&short, nu).sqrt(), power_at(&long, nu).sqrt());
            let predicted = alternating_sideband(k, t_a, t_b);
            eprintln!(
                "alternating 5.5/6.0, k = {k}: {here:.6} at N, {there:.6} at 10 N, predicted {predicted:.6}"
            );
            worst = worst.max((here - predicted).abs()).max((here - there).abs());
        }
        // Control: cycles drawn from the same span, U(5.5, 6.0), instead of alternating. The alias
        // at the mean rate recedes: its height at N and at 10 N differs by more than 30%.
        let alias = 1.0 / mean_cycle(t_a, t_b - t_a, false);
        let jittered =
            |shots: usize| rms_height(32, alias, |seed| walk_times(shots, t_a, t_b - t_a, false, seed));
        let (here, there) = (jittered(n), jittered(10 * n));
        eprintln!("control, cycle jitter U(5.5, 6.0): alias height {here:.4} at N, {there:.4} at 10 N");
        assert!((there / here - 1.0).abs() > 0.3, "{here} and {there}");
        worst
    }

    cav_validate::case! {
        test: validation_window_alternating_sidebands,
        name: "cadence.window_alternating",
        subsystem: "cadence",
        slice: "S2",
        reference: ClosedForm,
        against: "cycles of 5.5 s and 6.0 s in turn: the window's height at k/T_s, k = 1..4, by direct sum at 2000 shots against |cos(pi k T_A/T_s)|, and against the same at 20 000 shots; the larger absolute difference of heights (the peak is 1)",
        unit: Relative,
        tolerance: 1e-3,
        measure: cav_validate::Measured::Error(alternating_sidebands()),
    }

    /// The lines `k` = 1 … 4 of a cadence with `on` of every `period` slots taken, over `slots`
    /// slots of 5 s: the largest difference of height from the closed form.
    fn gapped_error(slots: usize, on: usize, period: usize) -> f64 {
        let t0 = 5.0;
        let times = gapped_times(slots, t0, on, period);
        (1..=4)
            .map(|k| {
                let height = power_at(&times, k as f64 / (period as f64 * t0)).sqrt();
                (height - gapped_line(k, on, period)).abs()
            })
            .fold(0.0f64, f64::max)
    }

    /// L/P = 0.4 as 4 of 10 and as 40 of 100, fifty periods of each; the larger error.
    fn gapped_lines() -> f64 {
        for (on, period) in [(4, 10), (40, 100)] {
            let heights: Vec<String> =
                (1..=4).map(|k| format!("{:.4}", gapped_line(k, on, period))).collect();
            eprintln!("gapped {on} of {period}: lines k = 1..4 predicted {}", heights.join(", "));
        }
        // The grid's own lines are 1.
        assert_eq!((gapped_line(10, 4, 10), gapped_line(7, 7, 7), gapped_line(20, 4, 10)), (1.0, 1.0, 1.0));
        // With no gaps there is no line between the grid's own.
        assert_eq!(gapped_line(3, 7, 7), 0.0);
        assert_eq!(gapped_times(20, 5.0, 4, 10), vec![0.0, 5.0, 10.0, 15.0, 50.0, 55.0, 60.0, 65.0]);
        // Control: three slots past a whole number of periods and the form is no longer exact.
        assert!(gapped_error(503, 4, 10) > 1e-4);
        let error = gapped_error(500, 4, 10).max(gapped_error(5000, 40, 100));
        eprintln!("gapped lines against the closed form: {error:e}");
        error
    }

    cav_validate::case! {
        test: validation_window_gapped_lines,
        name: "cadence.window_gapped",
        subsystem: "cadence",
        slice: "S2",
        reference: ClosedForm,
        against: "planned gaps, L of every P slots of 5 s taken, L/P = 0.4 as 4 of 10 and as 40 of 100, fifty whole periods: the window's height at k/(P T0), k = 1..4, by direct sum against |sin(pi k L/P)/(L sin(pi k/P))|; largest absolute difference of heights (the peak is 1)",
        unit: Relative,
        tolerance: 1e-9,
        measure: cav_validate::Measured::Error(gapped_lines()),
    }

    /// Cycle jitter as a random walk, positive and symmetric, δ = 0.1 and 0.3 s about 5 s, 500
    /// shots, seeds 0 … 1999: the mean of `|W|²` at the alias `1/T̄`, a line-width and a half above
    /// it, and at 1.37 times it, against [`cycle_jitter_window`]. Returns the worst `|ratio − 1|`
    /// and the worst distance in standard errors of the mean.
    fn cycle_jitter() -> (f64, f64) {
        let (n, t0, seeds) = (500usize, 5.0, 2000u64);
        let (mut worst, mut furthest, mut wrongly_passed) = (0.0f64, 0.0f64, 0);
        for symmetric in [false, true] {
            for delta in [0.1, 0.3] {
                let cycle = mean_cycle(t0, delta, symmetric);
                let alias = 1.0 / cycle;
                let frequencies = [alias, alias + 1.5 / (n as f64 * cycle), 1.37 * alias];
                let mut samples = [const { Vec::new() }; 3];
                for seed in 0..seeds {
                    let times = walk_times(n, t0, delta, symmetric, seed);
                    for (list, &nu) in samples.iter_mut().zip(&frequencies) {
                        list.push(power_at(&times, nu));
                    }
                }
                for (list, &nu) in samples.iter().zip(&frequencies) {
                    let phi = |symmetric: bool| {
                        if symmetric { phi_symmetric(nu, t0, delta) } else { phi_positive(nu, t0, delta) }
                    };
                    let predicted = cycle_jitter_window(phi(symmetric), n);
                    let (mean, error) = mean_and_error(list);
                    let (ratio, z) = (mean / predicted, (mean - predicted) / error);
                    eprintln!(
                        "cycle jitter {} δ = {delta}: ν = {nu:.6} Hz: mean |W|² {mean:.6e}, predicted {predicted:.6e}, ratio {ratio:.4}, {z:+.2} standard errors",
                        if symmetric { "symmetric" } else { "positive" }
                    );
                    worst = worst.max((ratio - 1.0).abs());
                    furthest = furthest.max(z.abs());
                    // The control: the other kind's characteristic function for the same draws.
                    let other = cycle_jitter_window(phi(!symmetric), n);
                    if ((mean - other) / error).abs() <= 3.0 && (mean / other - 1.0).abs() <= 0.08 {
                        wrongly_passed += 1;
                    }
                }
            }
        }
        // Control: held to the other kind of jitter's form, not one of the twelve passes.
        assert_eq!(wrongly_passed, 0, "the wrong form passed");
        (worst, furthest)
    }

    cav_validate::case! {
        test: validation_window_cycle_jitter,
        name: "cadence.window_cycle_jitter",
        subsystem: "cadence",
        slice: "S2",
        reference: ClosedForm,
        against: "cycle jitter as a random walk, positive U(T0, T0+d) and symmetric U(T0-d, T0+d), d = 0.1 and 0.3 s, T0 = 5 s, 500 shots: the mean of |W|^2 over 2000 keyed seeds at the alias 1/Tbar, 1.5 line-widths above it and at 1.37 times it, against (1/N^2)[N + 2 Re sum (N-k) phi^k]; the worst |measured/predicted - 1| of the twelve (each is also held to 3 standard errors of its mean)",
        unit: Relative,
        tolerance: 0.08,
        measure: cav_validate::Measured::Error({
            let (worst, furthest) = cycle_jitter();
            assert!(furthest <= 3.0, "a mean is {furthest:.2} standard errors from its closed form");
            worst
        }),
    }

    /// Jitter about a grid, ε ~ U(−0.3, 0.3) s on a grid of 5 s, 500 shots, seeds 0 … 1999: the
    /// mean of `|W|²` at the first alias `1/T₀` against [`grid_jitter_window`]. Returns
    /// `|ratio − 1|` and the distance in standard errors.
    fn grid_jitter() -> (f64, f64) {
        let (n, t0, delta, seeds) = (500usize, 5.0, 0.3, 2000u64);
        let nu = 1.0 / t0;
        let samples: Vec<f64> =
            (0..seeds).map(|seed| power_at(&grid_times(n, t0, delta, true, seed), nu)).collect();
        let (mean, error) = mean_and_error(&samples);
        let psi = psi_uniform(nu, delta);
        let predicted = grid_jitter_window(psi * psi, dirichlet(nu, n, t0), n);
        let z = (mean - predicted) / error;
        eprintln!(
            "grid jitter U(−0.3, 0.3): |ψ| = {psi:.4}; mean |W|² {mean:.6}, predicted {predicted:.6}, ratio {:.5}, {z:+.2} standard errors",
            mean / predicted
        );
        // The line does not recede: its height at N and at 10 N (200 seeds there) is the same to 3%.
        let there = rms_height(200, nu, |seed| grid_times(10 * n, t0, delta, true, seed));
        eprintln!("grid jitter: line height {:.5} at N, {there:.5} at 10 N", mean.sqrt());
        assert!((there / mean.sqrt() - 1.0).abs() <= 0.03, "{} and {there}", mean.sqrt());
        // Off the line (1.37/T₀) the floor is nearly all there is: held to the same criterion.
        let off = 1.37 / t0;
        let samples: Vec<f64> =
            (0..seeds).map(|seed| power_at(&grid_times(n, t0, delta, true, seed), off)).collect();
        let (floor, floor_error) = mean_and_error(&samples);
        let psi_off = psi_uniform(off, delta).powi(2);
        let lobes = psi_off * dirichlet(off, n, t0);
        let expected = grid_jitter_window(psi_off, dirichlet(off, n, t0), n);
        let z_off = (floor - expected) / floor_error;
        eprintln!(
            "grid jitter off the line: mean |W|² {floor:.6e}, predicted {expected:.6e}, ratio {:.4}, {z_off:+.2} standard errors",
            floor / expected
        );
        assert!(z_off.abs() <= 3.0 && (floor / expected - 1.0).abs() <= 0.08);
        // Controls: on the line, without the Debye–Waller factor (a line of height 1) the form is
        // outside three standard errors; off it, so is the form without its floor; and the same
        // jitter on the cycle recedes.
        assert!(((mean - 1.0) / error).abs() > 3.0);
        assert!(((floor - lobes) / floor_error).abs() > 3.0);
        let walk = |shots: usize| rms_height(32, nu, |seed| walk_times(shots, t0, delta, true, seed));
        assert!((walk(10 * n) / walk(n) - 1.0).abs() > 0.3);
        // The late-only jitter has the same size of ψ at half the spread.
        assert!((psi_positive(nu, 2.0 * delta).norm_squared() - psi * psi).abs() < 1e-15);
        ((mean / predicted - 1.0).abs(), z.abs())
    }

    cav_validate::case! {
        test: validation_window_grid_jitter,
        name: "cadence.window_grid_jitter",
        subsystem: "cadence",
        slice: "S2",
        reference: ClosedForm,
        against: "jitter about a grid, shot times n T0 + U(-0.3, 0.3) s, T0 = 5 s, 500 shots: the mean of |W|^2 over 2000 keyed seeds at the first alias 1/T0 against |psi|^2 |D_N|^2 + (1 - |psi|^2)/N; |measured/predicted - 1| (also held to 3 standard errors of the mean, and the line's height at N and 10 N to 3%)",
        unit: Relative,
        tolerance: 0.08,
        measure: cav_validate::Measured::Error({
            let (ratio, z) = grid_jitter();
            assert!(z <= 3.0, "the mean is {z:.2} standard errors from its closed form");
            ratio
        }),
    }

    /// Lost shots on a uniform plan of 4000 shots every 5 s, seeds 0 … 399: the mean over the
    /// seeds and over 64 frequencies between the first two lines (`ν·T₀` from 33/128 to 96/128) of
    /// `|W|² − |W₀|²`, as a ratio to [`loss_floor`]. Returns that ratio; the same with the window
    /// normalised by the planned count; the same with `|W₀|²` left in; and the standard error of
    /// the first.
    fn loss(p: f64) -> [f64; 4] {
        loss_over(p, 400)
    }

    fn loss_over(p: f64, seeds: u64) -> [f64; 4] {
        let (n, t0) = (4000usize, 5.0);
        let (df, count) = (1.0 / (128.0 * t0), 97);
        let bins = 33..=96usize;
        let plan = uniform_times(n, t0);
        let base: Vec<f64> = (0..count).map(|k| dirichlet(k as f64 * df, n, t0)).collect();
        let sidelobes = bins.clone().map(|k| base[k]).sum::<f64>() / 64.0;
        let (mut by_kept, mut by_planned) = (Vec::new(), Vec::new());
        for seed in 0..seeds {
            let kept = kept_times(&plan, p, seed);
            let window = window_fast(&kept, df, count);
            let floor = bins.clone().map(|k| window[k]).sum::<f64>() / 64.0 - sidelobes;
            by_kept.push(floor);
            by_planned.push(floor * (kept.len() as f64 / n as f64).powi(2));
        }
        let predicted = loss_floor(p, n);
        let (mean, error) = mean_and_error(&by_kept);
        let planned = mean_and_error(&by_planned).0;
        [mean / predicted, planned / predicted, (mean + sidelobes) / predicted, error / predicted]
    }

    /// The worse of the two loss probabilities' `|ratio − 1|`.
    fn loss_floor_error() -> f64 {
        let (mut worst, mut by_planned) = (0.0f64, 0.0);
        for p in [0.01, 0.05] {
            let [ratio, planned, uncorrected, error] = loss(p);
            by_planned = planned;
            eprintln!(
                "loss p = {p}: floor/predicted {ratio:.4} ± {error:.4} (normalised by the kept count, |W₀|² taken off); {planned:.4} normalised by the planned count; {uncorrected:.4} with the plan's sidelobes left in"
            );
            worst = worst.max((ratio - 1.0).abs());
        }
        // Control: normalised by the planned count the floor is (1 − p)² of the form, which at
        // p = 0.05 is outside the 5%.
        assert!((by_planned - 1.0).abs() > 0.05);
        // Losing nothing keeps everything; losing everything keeps nothing.
        let plan = uniform_times(100, 5.0);
        assert_eq!(kept_times(&plan, 0.0, 1), plan);
        assert!(kept_times(&plan, 1.0, 1).is_empty());
        worst
    }

    #[test]
    fn the_loss_floor_has_its_one_minus_p() {
        // S2's review, finding 5: with `p / n` in place of `p / ((1 − p) n)` every test passed,
        // because at p = 0.01 and 0.05 the factor is within the case's 5%. Criterion, written
        // before measuring: at p = 0.3, 4000 shots, 200 keyed seeds, the measured floor is within
        // 5% of the closed form; without the (1 − p) the form would be 30% low, far outside.
        let p = 0.3;
        let [ratio, _, _, error] = loss_over(p, 200);
        eprintln!("loss p = {p}: floor/predicted {ratio:.4} ± {error:.4}");
        assert!((ratio - 1.0).abs() < 0.05, "floor/predicted = {ratio}");
        // Control: the same measurement against the form without its (1 − p) is far outside.
        let without = ratio * loss_floor(p, 4000) / (p / 4000.0);
        assert!((without - 1.0).abs() > 0.25, "against p/N: {without}");
    }

    cav_validate::case! {
        test: validation_window_loss_floor,
        name: "cadence.window_loss_floor",
        subsystem: "cadence",
        slice: "S2",
        reference: ClosedForm,
        against: "shots lost independently with probability p = 0.01 and 0.05 from 4000 shots every 5 s: the mean over 400 keyed seeds and 64 frequencies between the first two lines of |W|^2 - |W0|^2 (W normalised by the kept count, W0 the Dirichlet kernel of the plan) against p/((1-p)N); the worse |measured/predicted - 1|",
        unit: Relative,
        tolerance: 0.05,
        measure: cav_validate::Measured::Error(loss_floor_error()),
    }

    #[test]
    fn alias_ratio_falls_as_root_n() {
        let t0 = 5.0;
        let ratio = |delta: f64, n: usize| {
            alias_ratio(phi_positive(1.0 / mean_cycle(t0, delta, false), t0, delta), n)
        };
        // The note's table (§4.2), to three figures.
        let note = [(0.1, 0.660, 0.175), (0.2, 0.382, 0.089), (0.3, 0.263, 0.060)];
        for (delta, few, many) in note {
            let (here, there) = (ratio(delta, 5000), ratio(delta, 100_000));
            let slope = (there / here).ln() / 20f64.ln();
            eprintln!("alias ratio, δ = {delta}: {here:.5} at N = 5e3, {there:.5} at 1e5, slope {slope:.4}");
            assert!((here - few).abs() < 5e-4, "δ = {delta}, N = 5e3: {here} against {few}");
            assert!((there - many).abs() < 5e-4, "δ = {delta}, N = 1e5: {there} against {many}");
            if delta != 0.2 {
                assert!((-0.50..=-0.40).contains(&slope), "δ = {delta}: slope {slope}");
            }
        }
        // Control: with no jitter the alias is a line of height 1 at any N and has no slope; and
        // jitter about a grid of the same size does not fall either.
        assert_eq!(alias_ratio(phi_positive(0.2, t0, 0.0), 5000), 1.0);
        assert_eq!(alias_ratio(phi_positive(0.2, t0, 0.0), 100_000), 1.0);
        let psi = psi_uniform(0.2, 0.3).powi(2);
        let grid = |n: usize| grid_jitter_window(psi, dirichlet(0.2, n, t0), n).sqrt();
        assert!(((grid(100_000) / grid(5000)).ln() / 20f64.ln()).abs() < 1e-3);
    }

    /// `(1/N²)[N + 2 Re Σ (N − k) φᵏ]` by the sum itself, in `N` steps.
    fn walk_window_direct(phi: Complex, n: usize) -> f64 {
        let (mut sum, mut power) = (0.0, Complex::new(1.0, 0.0));
        for k in 1..n {
            power = power.times(phi);
            sum += (n - k) as f64 * power.re;
        }
        (n as f64 + 2.0 * sum) / (n as f64 * n as f64)
    }

    /// The closed form as the note writes it, with nothing done about its cancellation.
    fn walk_window_naive(phi: Complex, n: usize) -> f64 {
        let count = n as f64;
        let gap = Complex::new(1.0 - phi.re, -phi.im);
        let high = power(phi, n);
        let bracket = Complex::new(count * gap.re - (1.0 - high.re), count * gap.im + high.im);
        let inverse = scaled(gap.conjugate().times(gap.conjugate()), 1.0 / gap.norm_squared().powi(2));
        (count + 2.0 * phi.times(bracket).times(inverse).re) / (count * count)
    }

    #[test]
    fn the_cycle_jitter_sum_is_stable() {
        // φ everywhere it can be: near 1 from inside and along the circle, on the circle, small,
        // negative, and each side of the changeover at N|1 − φ| = 0.1.
        let mut cases = vec![
            Complex::new(1.0, 0.0),
            Complex::new(0.0, 0.0),
            Complex::new(-1.0, 0.0),
            Complex::new(0.3, -0.4),
            Complex::new(-0.7, 0.2),
            Complex::new(0.999_355, 0.0),
        ];
        for exponent in 1..=14 {
            let small = 10f64.powi(-exponent);
            cases.push(Complex::new(1.0 - small, 0.0));
            cases.push(Complex::unit(small));
            cases.push(scaled(Complex::unit(-small), 1.0 - 0.5 * small));
        }
        let mut worst = 0.0f64;
        for &phi in &cases {
            for n in [1usize, 2, 3, 19, 20, 21, 500, 5000] {
                let (closed, direct) = (cycle_jitter_window(phi, n), walk_window_direct(phi, n));
                assert!(closed.is_finite(), "φ = {phi:?}, N = {n}");
                worst = worst.max((closed - direct).abs());
            }
        }
        eprintln!("cycle-jitter closed form against its sum: {worst:e}");
        assert!(worst <= 1e-11, "{worst:e}");
        // The ends: one shot, no jitter on a line, and independent phases.
        assert_eq!(cycle_jitter_window(Complex::new(0.3, 0.1), 1), 1.0);
        assert_eq!(cycle_jitter_window(Complex::new(1.0, 0.0), 777), 1.0);
        assert_eq!(cycle_jitter_window(Complex::new(0.0, 0.0), 50), 1.0 / 50.0);
        assert_eq!(cycle_jitter_window(Complex::new(0.5, 0.0), 0), 0.0);
        // With no jitter the walk is the uniform grid, and the form is the Dirichlet kernel.
        let apart = (0..=256)
            .map(|j| {
                let nu = f64::from(j) / 320.0;
                (cycle_jitter_window(phi_positive(nu, 5.0, 0.0), 200) - dirichlet(nu, 200, 5.0)).abs()
            })
            .fold(0.0f64, f64::max);
        assert!(apart <= 1e-11, "{apart:e}");
        // The floor is the whole of it but for a remainder of at most 4/(N|1 − φ|)² …
        let phi = phi_positive(0.271, 5.0, 0.3);
        let (whole, floor) = (cycle_jitter_window(phi, 5000), cycle_jitter_floor(phi, 5000));
        let bound = 4.0 / (5000f64.powi(2) * Complex::new(1.0 - phi.re, -phi.im).norm_squared());
        eprintln!("cycle-jitter floor {floor:e}, the whole {whole:e}, the bound on the rest {bound:e}");
        assert!((floor - whole).abs() <= bound && bound < 0.05 * whole);
        // … and (control) on a line it is not the window at all: it is above the peak.
        let alias = phi_positive(1.0 / 5.05, 5.0, 0.1);
        assert!(cycle_jitter_floor(alias, 500) > 1.0 && cycle_jitter_window(alias, 500) < 1.0);
        // Control: the closed form as written loses its digits near φ = 1, where the sum does not.
        let phi = Complex::new(1.0 - 1e-9, 0.0);
        let direct = walk_window_direct(phi, 500);
        let naive = (walk_window_naive(phi, 500) - direct).abs();
        eprintln!("the naive closed form at 1 − φ = 1e-9, N = 500: off by {naive:e}");
        assert!(naive > 1e-9, "{naive:e}");
        assert!((cycle_jitter_window(phi, 500) - direct).abs() <= 1e-12);
    }
}
