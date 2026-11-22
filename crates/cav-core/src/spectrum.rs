//! Spectra of a tower's signal: the classical periodogram, the Lomb–Scargle periodogram, Nyquist
//! folding and a least-squares notch. `f64` on the CPU (the owner's ruling: no GPU periodogram
//! until float-float arrives in S5).
//!
//! The conventions are the reference scripts' (`docs/research/s1-periodograms-and-script-fixtures.md`):
//!
//! - classical: `S_k = (Δt/N)·|FFT(y − ȳ)|²` at `f_k = k/(NΔt)`, one-sided and not doubled, so a
//!   tone of amplitude `a` on a bin has `√S = a·√T_obs/2` and white noise of amplitude spectral
//!   density `A` has mean `S = A²`;
//! - Lomb–Scargle: Scargle's τ form on data with its mean taken off, times the mean spacing
//!   `dt_eff = (t_last − t_first)/(N − 1)`, which makes it the classical periodogram on a uniform
//!   grid.
//!
//! [`lomb_scargle`] is exact and takes `N·M` steps; [`lomb_scargle_fast`] is the same estimator from
//! two non-uniform FFTs, for long series, held to the exact one (`docs/slices/S1.md` §3.2).

use std::f64::consts::{PI, TAU};

/// A complex number, as much of one as a transform needs.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    #[must_use]
    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    /// `e^{iθ}`.
    #[must_use]
    pub fn unit(theta: f64) -> Self {
        let (im, re) = theta.sin_cos();
        Self { re, im }
    }

    #[must_use]
    pub fn times(self, o: Self) -> Self {
        Self { re: self.re * o.re - self.im * o.im, im: self.re * o.im + self.im * o.re }
    }

    #[must_use]
    pub const fn conjugate(self) -> Self {
        Self { re: self.re, im: -self.im }
    }

    /// `|z|²`.
    #[must_use]
    pub fn norm_squared(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
}

/// The discrete Fourier transform of a power-of-two length, in place:
/// `X_k = Σ x_n e^{−2πi·kn/N}` (or with `+` for `inverse`, unscaled).
fn radix2(data: &mut [Complex], inverse: bool) {
    let n = data.len();
    debug_assert!(n.is_power_of_two());
    // Bit reversal.
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            data.swap(i, j);
        }
    }
    let sign = if inverse { 1.0 } else { -1.0 };
    let mut len = 2;
    while len <= n {
        // The twiddles of this stage, each from its own angle so that rounding does not build up.
        let twiddles: Vec<Complex> =
            (0..len / 2).map(|k| Complex::unit(sign * TAU * k as f64 / len as f64)).collect();
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (a, b) = (data[start + k], data[start + k + len / 2].times(twiddles[k]));
                data[start + k] = Complex::new(a.re + b.re, a.im + b.im);
                data[start + k + len / 2] = Complex::new(a.re - b.re, a.im - b.im);
            }
        }
        len <<= 1;
    }
}

/// The discrete Fourier transform of any length: `X_k = Σ x_n e^{−2πi·kn/N}`, `k = 0 … N − 1`.
/// A power of two goes straight through the radix-2 transform; any other length through
/// Bluestein's identity `kn = (k² + n² − (k − n)²)/2`, which turns it into a convolution of a
/// power-of-two length.
#[must_use]
pub fn fft(input: &[Complex]) -> Vec<Complex> {
    let n = input.len();
    if n <= 1 {
        return input.to_vec();
    }
    if n.is_power_of_two() {
        let mut data = input.to_vec();
        radix2(&mut data, false);
        return data;
    }
    // The chirp e^{−πi·n²/N}; n² is taken modulo 2N first, so the angle stays small and exact.
    let chirp: Vec<Complex> = (0..n)
        .map(|k| {
            let square = (k as u128 * k as u128 % (2 * n as u128)) as f64;
            Complex::unit(-PI * square / n as f64)
        })
        .collect();
    let size = (2 * n - 1).next_power_of_two();
    let mut a = vec![Complex::default(); size];
    let mut b = vec![Complex::default(); size];
    for k in 0..n {
        a[k] = input[k].times(chirp[k]);
        b[k] = chirp[k].conjugate();
        if k > 0 {
            b[size - k] = chirp[k].conjugate();
        }
    }
    radix2(&mut a, false);
    radix2(&mut b, false);
    for (x, y) in a.iter_mut().zip(&b) {
        *x = x.times(*y);
    }
    radix2(&mut a, true);
    let scale = 1.0 / size as f64;
    (0..n).map(|k| a[k].times(chirp[k]).times(Complex::new(scale, 0.0))).collect()
}

/// The same transform by its definition, in `N²` steps: what [`fft`] is held to.
#[must_use]
pub fn dft(input: &[Complex]) -> Vec<Complex> {
    let n = input.len();
    (0..n)
        .map(|k| {
            input.iter().enumerate().fold(Complex::default(), |sum, (j, x)| {
                let turn = (k as u128 * j as u128 % n as u128) as f64;
                let term = x.times(Complex::unit(-TAU * turn / n as f64));
                Complex::new(sum.re + term.re, sum.im + term.im)
            })
        })
        .collect()
}

/// How the classical periodogram treats the mean and the bin at zero frequency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Classical {
    /// Take the mean off the series first.
    pub subtract_mean: bool,
    /// Keep the bin at zero frequency.
    pub keep_dc: bool,
}

impl Classical {
    /// The `fig2_*` and `100m_oscillators` scripts': the mean taken off, the zero bin dropped.
    pub const SCRIPTS: Self = Self { subtract_mean: true, keep_dc: false };
    /// `1D_oscillator`'s: the series as it is, the zero bin kept.
    pub const ONE_D: Self = Self { subtract_mean: false, keep_dc: true };
}

/// A spectrum: frequencies in hertz and the power at each, in the series' unit squared per hertz.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Spectrum {
    pub frequencies: Vec<f64>,
    pub power: Vec<f64>,
}

impl Spectrum {
    /// The bin with the most power: its index, frequency and power.
    #[must_use]
    pub fn peak(&self) -> Option<(usize, f64, f64)> {
        let best = (0..self.power.len()).max_by(|&a, &b| self.power[a].total_cmp(&self.power[b]))?;
        Some((best, self.frequencies[best], self.power[best]))
    }

    /// The bin nearest a frequency.
    #[must_use]
    pub fn bin_of(&self, frequency: f64) -> Option<usize> {
        (0..self.frequencies.len()).min_by(|&a, &b| {
            (self.frequencies[a] - frequency).abs().total_cmp(&(self.frequencies[b] - frequency).abs())
        })
    }
}

/// The classical periodogram of a series sampled every `dt` seconds:
/// `S_k = (dt/N)·|FFT(y)|²` at `f_k = k/(N·dt)` for `k` up to `N/2` (Nyquist included).
#[must_use]
pub fn periodogram(y: &[f64], dt: f64, options: Classical) -> Spectrum {
    let n = y.len();
    if n == 0 {
        return Spectrum::default();
    }
    let mean = if options.subtract_mean { y.iter().sum::<f64>() / n as f64 } else { 0.0 };
    let data: Vec<Complex> = y.iter().map(|v| Complex::new(v - mean, 0.0)).collect();
    let transform = fft(&data);
    let first = usize::from(!options.keep_dc);
    let bins = first..=n / 2;
    Spectrum {
        frequencies: bins.clone().map(|k| k as f64 / (n as f64 * dt)).collect(),
        power: bins.map(|k| dt / n as f64 * transform[k].norm_squared()).collect(),
    }
}

/// Where a frequency appears when sampled with Nyquist frequency `nyquist`: folded into
/// `[0, nyquist]` by reflection about its multiples, a triangle wave in `frequency`.
#[must_use]
pub fn fold_frequency(frequency: f64, nyquist: f64) -> f64 {
    let sampling = 2.0 * nyquist;
    let remainder = frequency.rem_euclid(sampling);
    if remainder > nyquist { sampling - remainder } else { remainder }
}

/// The Lomb–Scargle periodogram of samples `y` at times `t`, at each of `frequencies` (hertz):
/// Scargle's form on the series with its mean taken off, times the mean spacing,
///
/// ```text
/// tan 2ωτ = Σ sin 2ωt / Σ cos 2ωt
/// S(f) = dt_eff · ½ · [ (Σ y cos ω(t − τ))² / Σ cos² ω(t − τ) + (Σ y sin ω(t − τ))² / Σ sin² ω(t − τ) ]
/// ```
///
/// Exact, in `N·M` steps. A term whose denominator vanishes (the Nyquist frequency of a uniform
/// grid) contributes nothing, which is its limit.
#[must_use]
pub fn lomb_scargle(t: &[f64], y: &[f64], frequencies: &[f64]) -> Vec<f64> {
    assert_eq!(t.len(), y.len(), "a time for every sample");
    let n = t.len();
    if n < 2 {
        return vec![0.0; frequencies.len()];
    }
    let mean = y.iter().sum::<f64>() / n as f64;
    let spacing = (t[n - 1] - t[0]) / (n as f64 - 1.0);
    frequencies
        .iter()
        .map(|&frequency| {
            let omega = TAU * frequency;
            let (mut s2, mut c2) = (0.0, 0.0);
            for &time in t {
                let (s, c) = (2.0 * omega * time).sin_cos();
                s2 += s;
                c2 += c;
            }
            let tau = if omega == 0.0 { 0.0 } else { 0.5 * s2.atan2(c2) / omega };
            let (mut yc, mut ys, mut cc, mut ss) = (0.0, 0.0, 0.0, 0.0);
            for (&time, &value) in t.iter().zip(y) {
                let (s, c) = (omega * (time - tau)).sin_cos();
                yc += (value - mean) * c;
                ys += (value - mean) * s;
                cc += c * c;
                ss += s * s;
            }
            // A sum of squares that rounding alone keeps from zero is zero.
            let term =
                |top: f64, bottom: f64| if bottom > 1e-12 * n as f64 { top * top / bottom } else { 0.0 };
            spacing * 0.5 * (term(yc, cc) + term(ys, ss))
        })
        .collect()
}

/// The width of the spreading kernel of [`lomb_scargle_fast`], in points of the fine grid. Each
/// point of width buys about a decade: 14 puts the kernel's error near 1e-13 of `Σ|c|`, below the
/// rounding floor of a long series.
pub const NUFFT_WIDTH: usize = 14;

/// The fine grid of the non-uniform FFT has at least this many points per mode.
const NUFFT_UPSAMPLING: usize = 2;

/// The type-1 non-uniform FFT: `F_k = Σ_j c_j e^{i·k·x_j}` for `k = 0 … modes − 1`, with `x` in
/// radians (any real value; only its value modulo 2π matters).
///
/// Each point is spread onto a fine uniform grid with the "exponential of semicircle" kernel
/// `φ(z) = e^{β(√(1 − z²) − 1)}`, `β = 2.3·width` (Barnett, Magland & af Klinteberg 2019), the grid
/// is transformed, and each mode is divided by the kernel's own transform. The modes are centred
/// first (`c_j` is turned by `e^{i·x_j·modes/2}`), so the grid is `2·modes` points and not `4·modes`.
///
/// The error is about `10^(1 − width)` of `Σ|c_j|` until rounding takes over, near `N·ε`. Memory is
/// the fine grid: `32·modes` bytes rounded up to a power of two.
///
/// # Panics
/// When `x` and `c` differ in length, or `width` is less than 2.
#[must_use]
pub fn nufft1(x: &[f64], c: &[Complex], modes: usize, width: usize) -> Vec<Complex> {
    assert_eq!(x.len(), c.len(), "a weight for every point");
    assert!(width >= 2, "a kernel is at least two grid points wide");
    if modes == 0 {
        return Vec::new();
    }
    let half = modes.div_ceil(2);
    // At least `width` points, so that a kernel never wraps onto itself.
    let size = (NUFFT_UPSAMPLING * 2 * half).max(2 * width).next_power_of_two();
    let step = TAU / size as f64;
    let beta = 2.3 * width as f64;
    let reach = width as f64 / 2.0;
    let kernel = |z: f64| if z.abs() < 1.0 { (beta * ((1.0 - z * z).sqrt() - 1.0)).exp() } else { 0.0 };
    let mut grid = vec![Complex::default(); size];
    for (&at, &weight) in x.iter().zip(c) {
        // The point's place on the grid, in grid steps, within one turn.
        let turns = at / TAU;
        let place = (turns - turns.floor()) * size as f64;
        let angle = place * step;
        let turned = weight.times(Complex::unit(angle * half as f64));
        let first = (place - reach).ceil() as i64;
        for l in first..first + width as i64 {
            let value = kernel((l as f64 - place) / reach);
            let cell = &mut grid[l.rem_euclid(size as i64) as usize];
            cell.re += turned.re * value;
            cell.im += turned.im * value;
        }
    }
    // Σ_l b_l e^{+i·k·l·step} is the conjugate-direction transform: the forward one at −k.
    radix2(&mut grid, false);
    // The kernel's transform at mode k: reach·∫φ(z)·cos(k·step·reach·z) dz over [−1, 1], by a
    // Gauss–Legendre rule on eight panels of [0, 1] (the integrand is smooth and turns at most a
    // few times there).
    let rule = crate::forward::gauss_legendre(8);
    const PANELS: usize = 8;
    let nodes: Vec<(f64, f64)> = (0..PANELS)
        .flat_map(|panel| {
            let rule = &rule;
            rule.iter().map(move |(node, weight)| {
                let z = (panel as f64 + 0.5 * (node + 1.0)) / PANELS as f64;
                (z, 0.5 * weight / PANELS as f64)
            })
        })
        .map(|(z, weight)| (z, weight * kernel(z)))
        .collect();
    let transform = |k: i64| {
        let rate = k as f64 * step * reach;
        2.0 * reach * nodes.iter().map(|(z, weight)| weight * (rate * z).cos()).sum::<f64>()
    };
    (0..modes)
        .map(|k| {
            let centred = k as i64 - half as i64;
            let cell = grid[(-centred).rem_euclid(size as i64) as usize];
            let scale = 1.0 / transform(centred);
            Complex::new(cell.re * scale, cell.im * scale)
        })
        .collect()
}

/// [`lomb_scargle`] at the frequencies `k·df`, `k = 0 … count − 1`, from two non-uniform FFTs in
/// about `N·width + M·log M` steps: the same estimator, held to 1e-9 of its largest value and
/// measured at a few 1e-15 for 2000 and for 100 000 shots (`docs/slices/S1.md` §7.3).
///
/// Units: `t` in seconds, in rising order; `y` in any unit (radians for a phase); `df` in hertz.
/// The result is in `y`'s unit squared per hertz, in the scripts' convention: the mean spacing
/// of the samples times Scargle's power, one-sided and not doubled, so that a tone of amplitude
/// `a` on a bin of a run of length `T_obs` has `√S = a·√T_obs/2`.
///
/// The four sums of Scargle's form come from `A(ω) = Σ(y − ȳ)e^{iωt}` and `B(ω) = Σe^{2iωt}`:
/// `2ωτ = arg B`, `Σcos²ω(t − τ) = (N + |B|)/2`, `Σsin²ω(t − τ) = (N − |B|)/2`, and the two
/// numerators are the real and imaginary parts of `A·e^{−iωτ}`.
///
/// Not for a list of arbitrary frequencies (use [`lomb_scargle`]) and not for `f32`: in single
/// precision the result is wrong by per cent at 1e5 shots (the research note measured it).
///
/// # Panics
/// When `t` and `y` differ in length.
#[must_use]
pub fn lomb_scargle_fast(t: &[f64], y: &[f64], df: f64, count: usize) -> Vec<f64> {
    lomb_scargle_nufft(t, y, df, count, NUFFT_WIDTH)
}

/// [`lomb_scargle_fast`] with a kernel of `width` grid points: narrower is faster and less
/// accurate, by about a decade a point.
///
/// # Panics
/// When `t` and `y` differ in length, or `width` is less than 2.
#[must_use]
pub fn lomb_scargle_nufft(t: &[f64], y: &[f64], df: f64, count: usize, width: usize) -> Vec<f64> {
    assert_eq!(t.len(), y.len(), "a time for every sample");
    let n = t.len();
    if n < 2 || count == 0 {
        return vec![0.0; count];
    }
    let mean = y.iter().sum::<f64>() / n as f64;
    let spacing = (t[n - 1] - t[0]) / (n as f64 - 1.0);
    // Times from the first one: the estimator does not depend on the origin, and small angles
    // keep their digits.
    let x: Vec<f64> = t.iter().map(|time| TAU * df * (time - t[0])).collect();
    let centred: Vec<Complex> = y.iter().map(|value| Complex::new(value - mean, 0.0)).collect();
    let ones = vec![Complex::new(1.0, 0.0); n];
    let a = nufft1(&x, &centred, count, width);
    // B at ω_k is the sum of e^{i·(2k)·x}: every second mode of a transform twice as long.
    let b = nufft1(&x, &ones, 2 * count - 1, width);
    (0..count)
        .map(|k| {
            let b = b[2 * k];
            let size = b.norm_squared().sqrt();
            // e^{−iωτ} = e^{−i·arg(B)/2}.
            let turn = Complex::unit(-0.5 * b.im.atan2(b.re));
            let numerator = a[k].times(turn);
            let (cc, ss) = (0.5 * (n as f64 + size), 0.5 * (n as f64 - size));
            // A denominator the transforms' error alone keeps from zero is zero (the Nyquist
            // frequency of a uniform grid); the exact form's 1e-12 would sit inside that error.
            let term =
                |top: f64, bottom: f64| if bottom > 1e-10 * n as f64 { top * top / bottom } else { 0.0 };
            spacing * 0.5 * (term(numerator.re, cc) + term(numerator.im, ss))
        })
        .collect()
}

/// The least-squares fit of `a·cos(2πft) + b·sin(2πft)` to the samples: `(a, b)`.
#[must_use]
pub fn fit_tone(t: &[f64], y: &[f64], frequency: f64) -> (f64, f64) {
    let (mut cc, mut cs, mut ss, mut yc, mut ys) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (&time, &value) in t.iter().zip(y) {
        let (s, c) = (TAU * frequency * time).sin_cos();
        cc += c * c;
        cs += c * s;
        ss += s * s;
        yc += value * c;
        ys += value * s;
    }
    let determinant = cc * ss - cs * cs;
    if determinant.abs() <= 1e-12 * (cc + ss).max(1.0) {
        // One of the two columns is (nearly) all zeros: fit the other alone.
        return if cc >= ss { (if cc > 0.0 { yc / cc } else { 0.0 }, 0.0) } else { (0.0, ys / ss) };
    }
    ((yc * ss - ys * cs) / determinant, (ys * cc - yc * cs) / determinant)
}

/// A notch: the series with the least-squares tone at `frequency` taken out.
#[must_use]
pub fn notch(t: &[f64], y: &[f64], frequency: f64) -> Vec<f64> {
    let (a, b) = fit_tone(t, y, frequency);
    t.iter()
        .zip(y)
        .map(|(&time, &value)| {
            let (s, c) = (TAU * frequency * time).sin_cos();
            value - a * c - b * s
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small deterministic generator (SplitMix64) with Box–Muller on top: noise for tests.
    struct Noise(u64);

    impl Noise {
        fn uniform(&mut self) -> f64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
        }

        fn normal(&mut self) -> f64 {
            (-2.0 * (1.0 - self.uniform()).ln()).sqrt() * (TAU * self.uniform()).cos()
        }
    }

    fn tone(n: usize, dt: f64, amplitude: f64, frequency: f64, phase: f64) -> Vec<f64> {
        (0..n).map(|k| amplitude * (TAU * frequency * k as f64 * dt + phase).cos()).collect()
    }

    #[test]
    fn the_fft_equals_the_direct_transform() {
        let mut noise = Noise(7);
        for n in [1, 2, 3, 8, 12, 100, 127, 128, 1000] {
            let input: Vec<Complex> = (0..n).map(|_| Complex::new(noise.normal(), noise.normal())).collect();
            let (fast, slow) = (fft(&input), dft(&input));
            let size = slow.iter().map(|z| z.norm_squared()).fold(0.0f64, f64::max).sqrt();
            let worst = fast
                .iter()
                .zip(&slow)
                .map(|(a, b)| Complex::new(a.re - b.re, a.im - b.im).norm_squared().sqrt())
                .fold(0.0f64, f64::max);
            assert!(worst <= 1e-12 * size, "length {n}: off by {worst:e} of {size:e}");
        }
        // A delta transforms to ones; a constant to a single bin.
        let delta = fft(&[Complex::new(1.0, 0.0), Complex::default(), Complex::default()]);
        assert!(delta.iter().all(|z| (z.re - 1.0).abs() < 1e-15 && z.im.abs() < 1e-15));
        // Control: a transform of the wrong sign is not the transform (unless the input is real and even).
        let input =
            [Complex::new(0.0, 0.0), Complex::new(1.0, 0.0), Complex::new(0.0, 0.0), Complex::new(0.0, 0.0)];
        let mut wrong = input.to_vec();
        radix2(&mut wrong, true);
        assert!((wrong[1].im - fft(&input)[1].im).abs() > 1.0);
    }

    #[test]
    fn a_tone_on_a_bin_has_the_stated_height() {
        // 1 mrad at 0.05 Hz, sampled every 2 s for 2000 s: on bin 100.
        let (n, dt, a) = (1000, 2.0, 1e-3);
        let spectrum = periodogram(&tone(n, dt, a, 0.05, 0.3), dt, Classical::SCRIPTS);
        assert_eq!(spectrum.frequencies.len(), n / 2);
        assert_eq!((spectrum.frequencies[0], *spectrum.frequencies.last().unwrap()), (1.0 / 2000.0, 0.25));
        let (bin, frequency, power) = spectrum.peak().unwrap();
        assert_eq!((bin, frequency), (99, 0.05));
        let expected = a * (n as f64 * dt).sqrt() / 2.0;
        assert!((power.sqrt() / expected - 1.0).abs() < 1e-12, "{} against {expected}", power.sqrt());
        // Nothing leaks: every other bin is rounding.
        assert!(spectrum.power.iter().enumerate().all(|(k, p)| k == bin || *p < 1e-24 * power));
        // The other convention keeps the zero bin, where a constant shows.
        let offset: Vec<f64> = tone(n, dt, a, 0.05, 0.0).iter().map(|v| v + 5e-3).collect();
        let kept = periodogram(&offset, dt, Classical::ONE_D);
        assert_eq!((kept.frequencies.len(), kept.frequencies[0]), (n / 2 + 1, 0.0));
        assert!((kept.power[0].sqrt() / (5e-3 * (n as f64 * dt).sqrt()) - 1.0).abs() < 1e-12);
        // Control: off a bin the peak is lower and its power is spread.
        let off = periodogram(&tone(n, dt, a, 0.05025, 0.3), dt, Classical::SCRIPTS);
        assert!(off.peak().unwrap().2.sqrt() < 0.7 * expected);
    }

    #[test]
    fn white_noise_has_the_stated_floor() {
        // Shot noise of 1e-4 rad/√Hz, sampled every 2 s: a standard deviation of ASD/√dt a shot.
        let (n, dt, asd) = (8192, 2.0, 1e-4);
        let mut noise = Noise(2026);
        let series: Vec<f64> = (0..n).map(|_| asd / f64::sqrt(dt) * noise.normal()).collect();
        let spectrum = periodogram(&series, dt, Classical::SCRIPTS);
        let mean = spectrum.power.iter().sum::<f64>() / spectrum.power.len() as f64;
        assert!((mean / (asd * asd) - 1.0).abs() < 0.05, "mean S is {:e}, ASD² is {:e}", mean, asd * asd);
        // Control: twice the noise is four times the floor.
        let louder: Vec<f64> = series.iter().map(|v| 2.0 * v).collect();
        let doubled = periodogram(&louder, dt, Classical::SCRIPTS);
        assert!((doubled.power.iter().sum::<f64>() / doubled.power.len() as f64 / mean - 4.0).abs() < 1e-9);
    }

    /// A series with a comb, a second tone and noise: something for two estimators to agree on.
    fn busy(n: usize, dt: f64) -> Vec<f64> {
        let mut noise = Noise(11);
        (0..n)
            .map(|k| {
                let t = k as f64 * dt;
                1e-3 * (TAU * 0.05 * t).cos()
                    + 2e-4 * (TAU * 0.1 * t + 1.0).sin()
                    + 3e-5 * (TAU * 0.17 * t).cos()
                    + 1e-5 * noise.normal()
                    + 4e-4
            })
            .collect()
    }

    /// The largest difference between Lomb–Scargle and the classical periodogram at the interior
    /// bins of a uniform grid, as a fraction of the classical value there.
    fn lomb_scargle_against_fft(n: usize, shift: f64) -> f64 {
        let dt = 2.0;
        let y = busy(n, dt);
        let t: Vec<f64> = (0..n).map(|k| 1.46 + k as f64 * dt + shift * (k % 2) as f64).collect();
        let classical = periodogram(&y, dt, Classical::SCRIPTS);
        // Every bin below Nyquist; at Nyquist itself Σ sin² vanishes and the form is 0/0.
        let interior = classical.frequencies.len() - 1;
        let scargle = lomb_scargle(&t, &y, &classical.frequencies[..interior]);
        (0..interior).map(|k| (scargle[k] / classical.power[k] - 1.0).abs()).fold(0.0f64, f64::max)
    }

    #[test]
    fn lomb_scargle_equals_fft_on_uniform_grid() {
        let worst = lomb_scargle_against_fft(2000, 0.0);
        assert!(worst <= 1e-9, "Lomb–Scargle is {worst:e} from the classical periodogram");
        // Control: on a grid that is not uniform (every other shot 0.3 s late) they are not the same thing.
        assert!(lomb_scargle_against_fft(2000, 0.3) > 1e-3);
    }

    cav_validate::case! {
        test: validation_lomb_scargle_equals_fft,
        name: "spectrum.lomb_scargle_equals_fft",
        subsystem: "spectrum",
        slice: "S1",
        reference: ClosedForm,
        against: "on a uniform, gap-free grid Lomb-Scargle (Scargle's tau, mean taken off, times dt_eff) is the classical periodogram: 2000 shots of a comb, a tone and noise, every bin below Nyquist; largest relative difference",
        unit: Relative,
        tolerance: 1e-9,
        measure: cav_validate::Measured::Error(lomb_scargle_against_fft(2000, 0.0)),
    }

    /// `n` shots at a cycle of 2 s, each up to `jitter` seconds late: a comb at 0.05 Hz, a tone
    /// above Nyquist and noise.
    fn jittered(n: usize, jitter: f64, seed: u64) -> (Vec<f64>, Vec<f64>) {
        let mut noise = Noise(seed);
        let t: Vec<f64> = (0..n).map(|k| 2.0 * k as f64 + jitter * noise.uniform()).collect();
        let y = t
            .iter()
            .map(|&time| {
                1e-3 * (TAU * 0.05 * time + 0.3).cos()
                    + 2e-4 * (TAU * 0.10 * time).cos()
                    + 5e-5 * (TAU * 0.3275 * time).sin()
                    + 1e-5 * noise.normal()
            })
            .collect();
        (t, y)
    }

    /// The largest difference between the fast periodogram and the exact one over `bins` (indices
    /// of the fast one's frequencies `k·df`), as a fraction of the exact one's largest value there.
    fn fast_against_exact(t: &[f64], y: &[f64], df: f64, count: usize, width: usize, bins: &[usize]) -> f64 {
        let fast = lomb_scargle_nufft(t, y, df, count, width);
        let frequencies: Vec<f64> = bins.iter().map(|&k| k as f64 * df).collect();
        let exact = lomb_scargle(t, y, &frequencies);
        let peak = exact.iter().copied().fold(0.0f64, f64::max);
        bins.iter().zip(&exact).map(|(&k, e)| (fast[k] - e).abs()).fold(0.0f64, f64::max) / peak
    }

    fn nufft_error(width: usize) -> f64 {
        let (t, y) = jittered(2000, 0.3, 21);
        let all: Vec<usize> = (0..1001).collect();
        fast_against_exact(&t, &y, 0.25 / 1000.0, 1001, width, &all)
    }

    #[test]
    fn nufft_is_the_sum_it_stands_for() {
        // The transform itself against its definition, complex weights, any x.
        let mut noise = Noise(3);
        let x: Vec<f64> = (0..300).map(|_| 40.0 * (noise.uniform() - 0.5)).collect();
        let c: Vec<Complex> = (0..300).map(|_| Complex::new(noise.normal(), noise.normal())).collect();
        let size: f64 = c.iter().map(|z| z.norm_squared().sqrt()).sum();
        for modes in [1, 2, 7, 64, 201] {
            let fast = nufft1(&x, &c, modes, NUFFT_WIDTH);
            assert_eq!(fast.len(), modes);
            for (k, value) in fast.iter().enumerate() {
                let direct = x.iter().zip(&c).fold(Complex::default(), |sum, (&at, &weight)| {
                    let term = weight.times(Complex::unit(k as f64 * at));
                    Complex::new(sum.re + term.re, sum.im + term.im)
                });
                let off = Complex::new(value.re - direct.re, value.im - direct.im).norm_squared().sqrt();
                assert!(off <= 1e-12 * size, "{modes} modes, mode {k}: off by {off:e} of {size:e}");
            }
        }
        assert!(nufft1(&x, &c, 0, NUFFT_WIDTH).is_empty());
        // Control: a transform of the other sign is not this one.
        let fast = nufft1(&x, &c, 8, NUFFT_WIDTH);
        let other = x.iter().zip(&c).fold(Complex::default(), |sum, (&at, &weight)| {
            let term = weight.times(Complex::unit(-3.0 * at));
            Complex::new(sum.re + term.re, sum.im + term.im)
        });
        assert!((fast[3].re - other.re).abs() + (fast[3].im - other.im).abs() > 1e-3 * size);
    }

    #[test]
    fn nufft_lomb_scargle_equals_exact() {
        let worst = nufft_error(NUFFT_WIDTH);
        eprintln!("NUFFT Lomb–Scargle, 2000 jittered shots: {worst:e} of the peak");
        assert!(worst <= 1e-9, "{worst:e} of the peak");
        // A uniform grid, its Nyquist bin among the frequencies, where a denominator vanishes.
        let (t, y) = jittered(2000, 0.0, 22);
        let all: Vec<usize> = (0..1001).collect();
        let uniform = fast_against_exact(&t, &y, 0.25 / 1000.0, 1001, NUFFT_WIDTH, &all);
        eprintln!("NUFFT Lomb–Scargle, uniform grid with its Nyquist bin: {uniform:e} of the peak");
        assert!(uniform <= 1e-9, "{uniform:e} of the peak on a uniform grid");
        // The origin of time does not matter; the scale of time does.
        let (t, y) = jittered(2000, 0.3, 21);
        let fast = lomb_scargle_fast(&t, &y, 0.25 / 1000.0, 1001);
        let peak = fast.iter().copied().fold(0.0f64, f64::max);
        let apart =
            |other: &[f64]| fast.iter().zip(other).map(|(a, b)| (a - b).abs()).fold(0.0f64, f64::max) / peak;
        let moved: Vec<f64> = t.iter().map(|time| time + 1.0).collect();
        assert!(apart(&lomb_scargle_fast(&moved, &y, 0.25 / 1000.0, 1001)) <= 1e-9);
        // Controls: stretched times are another series, and a kernel four points wide is not
        // accurate enough to pass.
        let stretched: Vec<f64> = t.iter().map(|time| time * 1.001).collect();
        assert!(apart(&lomb_scargle_fast(&stretched, &y, 0.25 / 1000.0, 1001)) > 1e-3);
        assert!(nufft_error(4) > 1e-9, "a kernel four points wide passed: {:e}", nufft_error(4));
    }

    cav_validate::case! {
        test: validation_nufft_lomb_scargle_equals_exact,
        name: "spectrum.nufft_lomb_scargle_equals_exact",
        subsystem: "spectrum",
        slice: "S1",
        reference: Exact,
        against: "the NUFFT Lomb-Scargle (two type-1 transforms, exponential-of-semicircle kernel 14 points wide, upsampling 2) against the exact N*M sum: 2000 shots at 2 s with up to 0.3 s of jitter, 1001 frequencies from 0 to 0.25 Hz; largest difference as a fraction of the peak",
        unit: Relative,
        tolerance: 1e-9,
        measure: cav_validate::Measured::Error(nufft_error(NUFFT_WIDTH)),
    }

    /// The error at each kernel width of the convergence case, and the worst of
    /// `error(w)/10^(2−w)` over them: at most 1 when every width is within its prediction.
    fn kernel_convergence() -> (Vec<(usize, f64)>, f64) {
        let errors: Vec<(usize, f64)> = [4, 6, 8, 10, 12].into_iter().map(|w| (w, nufft_error(w))).collect();
        let worst = errors.iter().map(|(w, e)| e / 10f64.powi(2 - *w as i32)).fold(0.0f64, f64::max);
        (errors, worst)
    }

    #[test]
    fn the_fast_periodogram_survives_a_vanishing_denominator() {
        // On a uniform grid, at zero frequency and at the Nyquist frequency, every sample has the
        // same phase: Σsin²ω(t − τ) is nothing, and so is the numerator over it. Scargle's form
        // is 0/0 there; the fast one must give a number, not NaN, and never a negative power.
        let n = 256;
        let dt = 2.0;
        let t: Vec<f64> = (0..n).map(|k| f64::from(k) * dt).collect();
        let df = 1.0 / (f64::from(n) * dt);
        // A tone at Nyquist (alternating signs), a tone inside on a bin of its own (off a bin it
        // would leak into the Nyquist bin and the height below would not be the Nyquist tone's
        // alone), and an offset.
        let inside = 26.0 * df;
        let y: Vec<f64> = (0..n)
            .map(|k| 0.7 * f64::from(1 - 2 * (k % 2)) + (TAU * inside * f64::from(k) * dt).cos() + 3.0)
            .collect();
        let count = n as usize / 2 + 1;
        let fast = lomb_scargle_fast(&t, &y, df, count);
        assert!(fast.iter().all(|s| s.is_finite() && *s >= 0.0), "a bin is not a power: {fast:?}");
        // The denominators themselves, from the same transform the estimator uses: at the two
        // ends they are below the guard's line, and at an ordinary bin far above it.
        let x: Vec<f64> = t.iter().map(|time| TAU * df * time).collect();
        let ones = vec![Complex::new(1.0, 0.0); t.len()];
        let b = nufft1(&x, &ones, 2 * count - 1, NUFFT_WIDTH);
        let sines = |k: usize| 0.5 * (f64::from(n) - b[2 * k].norm_squared().sqrt());
        let line = 1e-10 * f64::from(n);
        assert!(sines(0).abs() < line && sines(count - 1).abs() < line, "{} {}", sines(0), sines(count - 1));
        assert!(sines(count / 2) > 0.4 * f64::from(n));
        // Where the exact estimator is defined, the two agree; at the ends the fast one is what
        // the cosine term alone gives, which for the Nyquist tone is its whole power.
        let frequencies: Vec<f64> = (1..count - 1).map(|k| k as f64 * df).collect();
        let exact = lomb_scargle(&t, &y, &frequencies);
        let peak = exact.iter().fold(0.0f64, |m, s| m.max(*s));
        let worst = exact.iter().zip(&fast[1..]).fold(0.0f64, |m, (a, b)| m.max((a - b).abs()));
        assert!(worst <= 1e-9 * peak, "{worst:e} of {peak:e}");
        let nyquist = fast[count - 1];
        let expected = dt * 0.5 * (0.7 * f64::from(n)).powi(2) / f64::from(n);
        assert!((nyquist / expected - 1.0).abs() < 1e-9, "{nyquist} against {expected}");
        // Control: without the guard the same sums divide by those vanishing denominators. What
        // comes out is not a power: it is whatever rounding left on top over whatever it left
        // underneath, and it need not even be positive.
        let unguarded = |top: f64, k: usize| top * top / sines(k);
        let wild = [unguarded(1e-9, 0), unguarded(1e-9, count - 1)];
        assert!(wild.iter().any(|v| !v.is_finite() || v.abs() > 1e-9 || *v < 0.0), "{wild:?}");
    }

    #[test]
    fn nufft_kernel_converges() {
        let (errors, worst) = kernel_convergence();
        for (width, error) in &errors {
            eprintln!("NUFFT kernel width {width}: {error:e} of the peak");
        }
        assert!(worst <= 1.0, "a width is outside 10^(2-w): {errors:?}");
        for pair in errors.windows(2) {
            assert!(pair[1].1 <= pair[0].1 / 10.0, "two more points gained less than a decade: {pair:?}");
        }
        // Control: the errors are not all at the floor already, or nothing would be converging.
        assert!(errors[0].1 > 1e4 * errors[4].1);
    }

    cav_validate::case! {
        test: validation_nufft_kernel_converges,
        name: "spectrum.nufft_kernel_converges",
        subsystem: "spectrum",
        slice: "S1",
        reference: Exact,
        against: "the NUFFT Lomb-Scargle's error against the exact sum at kernel widths 4, 6, 8, 10, 12, each as a fraction of its predicted ceiling 10^(2-w) of the peak (a decade a grid point, one decade in hand); the largest of the five",
        unit: Relative,
        tolerance: 1.0,
        measure: cav_validate::Measured::Error(kernel_convergence().1),
    }

    /// 100 000 jittered shots, 50 000 frequencies, compared with the exact sum at 48 of them: the
    /// peak's bin, the two beside it, and the rest spread evenly.
    fn nufft_long() -> f64 {
        let (t, y) = jittered(100_000, 0.3, 23);
        let (count, df) = (50_000usize, 0.25f64 / 50_000.0);
        let peak = (0.05 / df).round() as usize;
        let mut bins: Vec<usize> = (0..45).map(|k| 1 + k * (count - 2) / 44).collect();
        bins.extend([peak - 1, peak, peak + 1]);
        fast_against_exact(&t, &y, df, count, NUFFT_WIDTH, &bins)
    }

    #[test]
    fn nufft_lomb_scargle_long() {
        let started = std::time::Instant::now();
        let worst = nufft_long();
        eprintln!(
            "NUFFT Lomb–Scargle, 100 000 shots: {worst:e} of the peak, {:?} with the check",
            started.elapsed()
        );
        assert!(worst <= 1e-9, "{worst:e} of the peak");
    }

    cav_validate::case! {
        test: validation_nufft_lomb_scargle_long,
        name: "spectrum.nufft_lomb_scargle_long",
        subsystem: "spectrum",
        slice: "S1",
        reference: Exact,
        against: "the NUFFT Lomb-Scargle on 100 000 jittered shots, 50 000 frequencies to 0.25 Hz, against the exact sum at 48 of them (the peak's bin, its neighbours, the rest spread evenly); largest difference as a fraction of the peak; the rounding floor is predicted near N*eps = 2e-11",
        unit: Relative,
        tolerance: 1e-9,
        measure: cav_validate::Measured::Error(nufft_long()),
    }

    #[test]
    fn ls_is_least_squares_sinusoid() {
        // Uneven times: a cycle of 5 s with up to 0.3 s of jitter.
        let mut noise = Noise(5);
        let mut t = Vec::new();
        let mut now = 0.0;
        for _ in 0..400 {
            t.push(now);
            now += 5.0 + 0.3 * noise.uniform();
        }
        let y: Vec<f64> = t
            .iter()
            .map(|&time| {
                1e-3 * (TAU * 0.031 * time + 0.4).cos()
                    + 3e-4 * (TAU * 0.077 * time).sin()
                    + 1e-4 * noise.normal()
            })
            .collect();
        let n = t.len() as f64;
        let mean = y.iter().sum::<f64>() / n;
        let centred: Vec<f64> = y.iter().map(|v| v - mean).collect();
        let total: f64 = centred.iter().map(|v| v * v).sum();
        let spacing = (t[t.len() - 1] - t[0]) / (n - 1.0);
        for frequency in [0.011, 0.031, 0.05, 0.077, 0.0931] {
            // The best a·cos + b·sin by the normal equations, with no τ anywhere, and what it leaves.
            let residual: f64 = notch(&t, &centred, frequency).iter().map(|v| v * v).sum();
            let explained = total - residual;
            let power = lomb_scargle(&t, &y, &[frequency])[0];
            // S = dt_eff · ½ · (the sum of squares the best sinusoid explains).
            assert!(
                (power / (spacing * 0.5 * explained) - 1.0).abs() < 1e-10,
                "{frequency} Hz: {power:e} against {:e}",
                spacing * 0.5 * explained
            );
        }
        // Control: at the tone's own frequency the fit explains most of the series; elsewhere, little.
        let at = |frequency: f64| lomb_scargle(&t, &y, &[frequency])[0];
        assert!(at(0.031) > 50.0 * at(0.05));
    }

    #[test]
    fn notch_filter_removes_known_tone() {
        let (n, dt) = (2000, 2.0);
        let t: Vec<f64> = (0..n).map(|k| k as f64 * dt).collect();
        let (wanted, unwanted) = (tone(n, dt, 2e-4, 0.085, 0.7), tone(n, dt, 1e-3, 0.05, 0.2));
        let both: Vec<f64> = wanted.iter().zip(&unwanted).map(|(a, b)| a + b).collect();
        let cleaned = notch(&t, &both, 0.05);
        let height = |series: &[f64], frequency: f64| {
            let spectrum = periodogram(series, dt, Classical::SCRIPTS);
            spectrum.power[spectrum.bin_of(frequency).unwrap()].sqrt()
        };
        // The tone is gone, to rounding …
        assert!(height(&cleaned, 0.05) < 1e-12 * height(&both, 0.05));
        // … and its neighbour is untouched.
        assert!((height(&cleaned, 0.085) / height(&wanted, 0.085) - 1.0).abs() < 1e-3);
        let (a, b) = fit_tone(&t, &both, 0.05);
        assert!((a.hypot(b) / 1e-3 - 1.0).abs() < 1e-9, "the fitted amplitude is the tone's");
        // Control: a notch somewhere else leaves the tone where it was.
        assert!((height(&notch(&t, &both, 0.07), 0.05) / height(&both, 0.05) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn nyquist_reflection() {
        let nyquist = 0.25;
        // Below Nyquist a frequency is itself; between f_N and 2 f_N it appears at 2 f_N − f.
        assert_eq!(fold_frequency(0.1, nyquist), 0.1);
        assert!((fold_frequency(0.3275, nyquist) - 0.1725).abs() < 1e-12);
        assert!((fold_frequency(0.35625, nyquist) - 0.14375).abs() < 1e-12);
        // The general fold is a triangle wave of period 2 f_N: continuous, between 0 and f_N.
        assert!(
            (fold_frequency(0.6, nyquist) - 0.1).abs() < 1e-12,
            "past the sampling frequency it starts again"
        );
        assert!((fold_frequency(0.7125, nyquist) - 0.2125).abs() < 1e-12, "2 × 0.35625 folds onto 0.2125");
        for step in 0..2000 {
            let f = 0.001 * f64::from(step);
            let folded = fold_frequency(f, nyquist);
            assert!((0.0..=nyquist).contains(&folded));
            assert!(
                (fold_frequency(f + 0.0005, nyquist) - folded).abs() <= 0.0005 + 1e-12,
                "no jumps at {f}"
            );
            assert!((fold_frequency(f + 2.0 * nyquist, nyquist) - folded).abs() < 1e-12);
            assert!(
                (fold_frequency(2.0 * nyquist - f.rem_euclid(0.5), nyquist) - folded).abs() < 1e-12,
                "mirror"
            );
        }
        // And it is where a sampled tone really appears: 0.3275 Hz sampled every 2 s for 20,000 s.
        let spectrum = periodogram(&tone(10_000, 2.0, 1e-3, 0.3275, 0.0), 2.0, Classical::SCRIPTS);
        let (_, at, _) = spectrum.peak().unwrap();
        assert!((at - fold_frequency(0.3275, nyquist)).abs() < 1e-12, "the peak is at {at} Hz");
        // Control: the unfolded frequency is not a bin of this spectrum at all.
        assert!(spectrum.frequencies.iter().all(|f| *f <= nyquist + 1e-12));
    }

    #[test]
    fn uldm_has_no_comb() {
        use crate::forward::{Instrument, Mass, Reader};
        // A tone at f_φ = 0.2125 Hz, as the ULDM signal is: one peak, and nothing at its multiples.
        let (n, dt, f_phi) = (10_000, 2.0, 0.2125);
        let spectrum = periodogram(&tone(n, dt, 1e-8, f_phi, 1.1), dt, Classical::SCRIPTS);
        let (bin, at, peak) = spectrum.peak().unwrap();
        assert!((at - f_phi).abs() < 1e-12);
        let others =
            spectrum.power.iter().enumerate().filter(|(k, p)| *k != bin && **p > 1e-18 * peak).count();
        assert_eq!(others, 0, "a tone has one bin");
        // Control: a mass oscillating at 0.05 Hz, read by the forward model, has a comb.
        let reader = Reader::new(Instrument::aion10());
        let source =
            Mass::point(1.0, |t: f64| crate::Vec3::new(3.0, 0.0, 1.0 + 0.5 * (TAU * 0.05 * t).cos()));
        let series: Vec<f64> =
            (0..2000).map(|k| reader.phase(crate::Vec3::ZERO, &source, 1.46 + 2.0 * f64::from(k))).collect();
        let comb = periodogram(&series, dt, Classical::SCRIPTS);
        let height = |f: f64| comb.power[comb.bin_of(f).unwrap()];
        assert!(
            height(0.10) > 1e-4 * height(0.05) && height(0.15) > 1e-7 * height(0.05),
            "harmonics at 2f₀ and 3f₀"
        );
    }
}
