# S1 research: periodograms and the reference-script fixtures

> Research note for slice S1 ("a point mass"), 21 Nov 2026. Question: how should S1 implement its
> periodograms and their oracle fixtures (`plan/VALIDATION-AND-RESEARCH.md` §2.1, §2.4;
> `plan/REBUILD.md` §3.4)?
>
> Reference scripts: `github.com/Thranduil02/atom-interferometry`, read at commit
> [`d7d360a`](https://github.com/Thranduil02/atom-interferometry/tree/d7d360ad92837bec929b85091b2bdd9af81d285c)
> (26 Oct 2026). Links of the form [`file#Lx`] below point into that commit; `REF` abbreviates
> `https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c`.
>
> Measurements were made on the build PC (Ryzen 5 3600, 12 threads; Python 3.14, numpy 2.5.5,
> scipy 1.18.1, astropy 8.0.3, finufft 2.5.1; rustc 1.100) in a throwaway directory outside the
> repo. Nothing from the script repository has been copied into this tree.

## 1. Short answer

1. **All five §2.1 scripts use the classical FFT periodogram, not Lomb–Scargle.** The convention is
   `S_k = (Δt/N)·|FFT(y − ȳ)|²` at `f_k = k/(NΔt)`, one-sided, not doubled. Lomb–Scargle appears
   only in the cadence scripts (§2.4, slice S11), as Scargle's τ form on mean-subtracted data,
   multiplied by the mean spacing.
2. **Build three f64 CPU estimators in S1:** the classical periodogram; an exact O(N·M)
   Lomb–Scargle as the oracle; a NUFFT Lomb–Scargle for long series. **Do not build a GPU
   periodogram in S1:** WGSL has no f64, and a single-precision NUFFT was measured to be wrong by
   4 % of the peak at 10⁵ shots and by more than the peak at 10⁶.
3. **The scripts' phase differs from the engine's design in sign and in quadrature.** The scripts
   compute lower-minus-upper interferometer (v1 is upper-minus-lower) and pair each arm position
   with the source one time step earlier. The second point shifts the phase by 0.02–0.4 % of its
   peak, so the plan's "≤ 1e-9 relative" against the scripts can only be met by a test-only
   quadrature that copies the scripts' sampling (§5).

## 2. What the reference scripts compute (part b)

The repository is **public** and has 38 scripts in one directory and no README (I cloned only the
latest commit, so I did not read its history). All five scripts were read in full before running; they import only numpy and matplotlib,
read no files, and their only side effect is `savefig` to an absolute Desktop path in four of them.

Common to all five: `m_atom = 1.46e-25`, `hbar = 1.055e-34`, `G = 6.67e-11`, `g = 9.81`,
`v_kick = 1000·ħ·(2π/698e-9)/m = 6.504651743802706 m/s`, `dt = 0.01 s`
([`1D_oscillator#L11-L23`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/1D_oscillator#L11-L23),
[`fig2_10m#L50-L58`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/fig2_10m#L50-L58)).
These equal v1's defaults (`crates/instrument/src/lib.rs` at tag `v1-final`, lines 29–41).

| Script | Instrument | Source (point mass, horizontal distance D, height h₀ + A cos 2πf t) | Default length | Arrays worth keeping |
|---|---|---|---|---|
| [`1D_oscillator`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/1D_oscillator) | AION-10: T = 0.73, u₀ = 3.86, IFOs at 0 and 5 m, cycle 2 s | M = 1 kg, D = 5, h₀ = 1, A = 1, f = 0.1 Hz | T_total = 10⁴ s → 5000 shots | `subsample_times`, `subsample_phase_diff`, `f_k_filtered`, `S_phasediff_filtered`, the four `*_cycle` arm tables |
| [`100m_oscillators`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/100m_oscillators) | AICE: T = 2.5, u₀ = gT − v_kick/2 = 21.27267412809865, IFOs at 0 and 70 m, cycle 5 s | oscillator A only: M = 0.05, D = 1, h₀ = 1, A = 0.5, f = 0.08 (B and C disabled); plus a ULDM tone at 0.1 Hz and unseeded shot noise | 2.5×10⁶ s → 5×10⁵ shots | `t_meas`, `phases_multi` (GGN only, deterministic), arm tables |
| [`fig2_folding_map.py`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/fig2_folding_map.py) | AICE, f_N = 0.1 Hz | `osc_b` f = 0.02 (linear); `osc_d` f = 0.1425 (circle of radius A in the D–h plane); both M = 1, D = 3, h₀ = 1, A = 0.5; ULDM at f_φ = 0.085 | 3×10⁷ s → 6×10⁶ shots | raw phases per panel, `f_k`, `S`, the `fold_frequency` triangle wave |
| [`fig2_uldm_folding`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/fig2_uldm_folding) | AICE | panels a (ULDM only), b (`osc_b` 0.02), c (`osc_c` 0.131 + ULDM), d (`osc_d` 0.1425 circular + ULDM) | 3×10⁷ s | as above, four panels |
| [`fig2_10m`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/fig2_10m) | AION-10, f_N = 0.25 Hz, δr = 5, L_base = 10 | same geometry; f = 0.05, 0.3275, 0.35625 (circular); f_φ = 0.2125 | 3×10⁷ s → 1.5×10⁷ shots | as above |

Things the table hides:

- **`1D_oscillator` cannot show four separate harmonics.** With f₀ = 0.1 Hz and f_N = 0.25 Hz, 3f₀
  folds onto 0.2 Hz and 4f₀ onto 0.1 Hz. Its periodogram has exactly two peaks (measured: 0.1 and
  0.2 Hz). `comb_at_integer_harmonics` (n = 1…4) should use `fig2_10m` panel b (measured peaks at
  0.05, 0.10, 0.15, 0.20 Hz: 5.7×10⁵, 3.5×10⁴, 2.1×10³, 1.6×10² µrad/√Hz at T_total = 10⁵ s) or
  `fig2_uldm_folding` panel b.
- **`100m_oscillators` is a multi-oscillator script with ULDM and noise switched on**; the
  point-oscillator fixture is its `phases_multi`, not `phases`. Its T_total is 2.5×10⁶ s, not the
  3×10⁷ s that `VALIDATION-AND-RESEARCH.md` §1 gives for AICE (that figure belongs to the `fig2_*`
  scripts).
- **Two periodogram variants.** `1D_oscillator` uses a hand-written O(N²) DFT with **no mean
  subtraction**, keeps the DC bin, and keeps bins up to and including Nyquist
  ([`1D_oscillator#L64-L71`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/1D_oscillator#L64-L71),
  [`#L178-L189`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/1D_oscillator#L178-L189));
  measured equal to `(Δt/N)|fft(y)|²` to 2×10⁻¹⁴. The other four use
  `(T_cyc/N)·|rfft(y − ȳ)|²` and drop only DC
  ([`fig2_10m#L182-L186`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/fig2_10m#L182-L186)).
  In both, a tone of amplitude a on a bin gives √S = a√T_obs/2 (measured 1.3226×10⁻³ against
  1.3185×10⁻³ predicted for the ULDM tone; I did not chase the 0.3 % difference), and white
  noise of per-shot σ = ASD/√T_cyc gives mean S = ASD² (measured 0.981 over one draw).
- **Noise uses the legacy global stream** (`np.random.normal`, no seed in the script). Calling
  `np.random.seed(0)` first reproduces it exactly (checked), and the legacy stream is frozen across
  numpy versions by numpy's own policy, which I did not re-read today.

### Running them headless

The scripts have no CLI and their lengths are module constants, so the harness executes the source
with constants replaced. `fig2_*` guard their plotting with `if __name__ == "__main__"`; the two
oscillator scripts do not, so `plt.show` and `Figure.savefig` are replaced by no-ops. The harness I
used is in the appendix. Measured run times at reduced length:

| Script | Setting | Shots | Time |
|---|---|---|---|
| `1D_oscillator` | default (T_total = 10⁴) | 5000 | 58 s, nearly all of it the O(N²) Python DFT; T_total = 2000 should take ~3 s (estimated, not run) |
| `100m_oscillators` | T_total = 5×10⁴ | 10⁴ | 1.6 s |
| `fig2_folding_map.py`, `fig2_uldm_folding`, `fig2_10m` | T_total = 10⁵ | 2×10⁴, 2×10⁴, 5×10⁴ | 2–3 s each |

One trap: `fig2_uldm_folding` has a docstring line beginning `T_total = 3.15e7`
([`#L36`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/fig2_uldm_folding#L36)),
so a replace-first-match substitution leaves the real constant at 3×10⁷ and the run takes gigabytes.
Replace every match. Keep T_total a multiple of 1/f for every tone so the peaks stay on bins
(10⁵ s works for all the frequencies above).

### Fixture format

One directory per script, float64 `.npy` plus a `meta.json` (script name, upstream commit, overrides,
seed, numpy version, the oscillator dictionaries). Suggested contents: `t_meas`, `raw_<panel>` (GGN
phase, no ULDM, no noise), `uldm`, `noise_<panel>` (seed 0), `f_k`, `S_clean_<panel>`,
`S_noisy_<panel>`, the four arm tables, and `fold_f_true`/`fold_f_obs`. At T_total = 10⁵ s that is
1.4–5.7 MB per `fig2_*` script and 0.4 MB for `100m_oscillators`; `1D_oscillator` needs only its
5000-point series and spectrum (~0.2 MB) — the script's other module arrays add up to 131 MB and
should not be saved. Rust reads `.npy` v1 float64 with a 30-line parser or the `npyz` crate (not
evaluated).

### Licence

The repository has **no licence file**, and the GitHub API reports `license: null`. With no licence,
default copyright applies: nothing grants permission to copy, modify or redistribute the scripts.
Whether numerical arrays produced by running them count as derived works is a legal question I
cannot settle. The commit I read is authored by George Parish, the first author of the paper the
validation plan cites, so permission or a licence can be asked for directly. **For the owner to decide:** whether fixtures
(and the harness, which contains no script text) may be committed; whether to ask the author to add
a licence. Until then the builder can generate fixtures locally into an untracked directory, with
the oracle tests skipping loudly when it is absent.

## 3. The phase formula in the scripts (part c)

For shot ℓ = 0, 1, …, launched at t_ℓ = ℓ·T_cyc and tagged at t_ℓ + 2T
([`fig2_10m#L154-L177`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/fig2_10m#L154-L177);
the same code is in `100m_oscillators`, `fig2_uldm_folding`, `fig2_folding_map.py`):

```
ΔΦ_ℓ = (m_atom/ħ) · trapezoid_{j=0…N_fly−1} F_j · dt,          N_fly = round(2T/dt)
F_j  = [V(z_u1[j], h_j) − V(z_l1[j], h_j)] − [V(z_u2[j], h_j) − V(z_l2[j], h_j)]
V(z, h) = −G·M / sqrt(D² + (z − h)²)
h_j  = h₀ + A·cos(2π f (t_ℓ + j·dt))                           (circular: h₀ + A sin, D + A cos)
```

IFO 1 launches from z = 0 and IFO 2 from z = δr. The lower arm launches at u₀ and receives
+v_kick at T; the upper arm launches at u₀ + v_kick and receives −v_kick at T
([`fig2_10m#L135-L144`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/fig2_10m#L135-L144)).
The arm tables come from a stepping loop that appends the position **after** each step and stops at
the launch height
([`fig2_10m#L127-L133`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/fig2_10m#L127-L133)),
so `z[j]` is the closed-form parabola at **(j + 1)·dt** (measured: equal to 6×10⁻¹⁵ m), clamped at
the launch height.

`1D_oscillator` is the same integrand written as a sliding window over a continuous 0.01 s grid
([`#L144-L176`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/1D_oscillator#L144-L176)):
shot ℓ uses nodes j = 0…146 (147 nodes, the full 2T) from sample 200ℓ, and is tagged at
2ℓ + 1.46 s (measured: every selected index is a multiple of 200; the times are 1.46, 3.46, …).

An independent re-implementation of these formulas reproduces the scripts' phases to 2×10⁻¹³
(`1D_oscillator`) and 5×10⁻¹⁶ (`100m_oscillators`) relative, so the description above is complete.

### Differences from `reference/DESIGN.md`, `reference/design/instrument.md` and v1

| | Scripts | Reference design and v1 code | Effect |
|---|---|---|---|
| **Sign** | (IFO at 0) − (IFO at δr), i.e. δφ₁ − δφ₂ | ΔΦ = δφ₂ − δφ₁ (`instrument.md` §4; v1 `d2 - d1`) | Phase series have opposite sign. Periodograms are unaffected. Measured: script ÷ continuous(δφ₁ − δφ₂) = +1.0013 at shot 0. |
| **Arm sampling** | arm position at (j+1)·dt paired with the source at j·dt | arms and source at the same instant | The dominant numerical difference; see below. |
| **Quadrature** | trapezoid at 0.01 s; `fig2_*` and `100m_oscillators` stop at 2T − dt (146 or 500 nodes); `1D_oscillator` covers the full 2T (147 nodes) | composite Simpson on [0, T] and [T, 2T] at `fine_dt` = 0.01 (`M16-trust.md` §2.7) | Trapezoid alone: 4×10⁻⁵–1×10⁻⁴ of peak. v1's Simpson is within 1×10⁻⁸–3×10⁻⁷ of the converged integral. |
| **Floor** | the arm stops at its launch height and stays there | `instrument.md` §5 says "a reflecting floor"; v1's code has no floor at all (`Arm::z_at` is a bare parabola) | Matters only for AION-10: with u₀ = 3.86 the closed-form arms end 7.15 cm **below** launch height at 2T; the scripts clamp them from t = 1.44 s (lower) and 1.45 s (upper). Continuous integral with and without the clamp differs by 3×10⁻⁴ (`1D_oscillator`) to 7×10⁻⁴ (`fig2_10m` b) of peak. |
| **Launch rule** | AION-10: u₀ = 3.86. AICE: u₀ = gT − v_kick/2, which closes the arms exactly at launch height at 2T (last table entry 2×10⁻¹³ m) | u₀ = 3.86 default; the AICE rule is in `VALIDATION-AND-RESEARCH.md` §1 and §3 item 4 | Agree. The symmetric AION-10 value would be gT − v_kick/2 = 3.909 m/s. |
| **Times** | launch ℓ·T_cyc, window [t_ℓ, t_ℓ + 2T], tag t_ℓ + 2T; source phase zero at the first launch | measurement at t, window [t − 2T, t] | Agree if the engine's first measurement time is 2T, not 0. |
| **Constants** | 1.46e-25, 1.055e-34, 6.67e-11, 9.81 | same in v1 | Agree. Note 1.46×10⁻²⁵ kg is the mass of ⁸⁸Sr; ⁸⁷Sr (which `instrument.md` §3 names) is 1.443×10⁻²⁵ kg, a 1.2 % scale factor. |

Size of the script-versus-continuous difference, as max |Δ| over the first 40–60 shots divided by
the peak |ΔΦ| (reference: adaptive quadrature to 10⁻¹³ on each half, closed-form arms, no floor):

| Case | Peak ΔΦ | Scripts vs continuous | of which the one-sample lag |
|---|---|---|---|
| `1D_oscillator` | 3.86 mrad | 3.3×10⁻³ (12.8 µrad) | 3.3×10⁻³ |
| `fig2_10m` panel b | 4.24 mrad | 3.7×10⁻³ | 3.7×10⁻³ |
| `fig2_uldm_folding` panel b (AICE) | 9.70 mrad | 2.0×10⁻⁴ | 3.2×10⁻⁴ before the clamp |
| `100m_oscillators` A (AICE, D = 1 m) | 0.68 mrad | 1.2×10⁻³ (0.80 µrad) | not separated |

So the engine's forward model and the scripts agree to a few parts in 10³, not 10⁻⁹. This is a
property of the scripts' sampling, not an engine error, and the tolerance in
`VALIDATION-AND-RESEARCH.md` §2.1 should not be moved to hide it (§5 gives the alternative).

## 4. Lomb–Scargle (part a)

### The convention the scripts use

Two equivalent implementations, both called `ls_psd`: the explicit-τ form
([`cycle jitter#L65-L76`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/cycle%20jitter#L65-L76))
and a rotated-sums form
([`cadence_alias_ratio#L97-L119`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/cadence_alias_ratio#L97-L119)).
Measured equal to 8×10⁻¹⁴.

```
y ← y − mean(y)                          (pre-centred; no floating mean)
tan 2ωτ = Σ sin 2ωt / Σ cos 2ωt          (Scargle 1982)
S(f) = dt_eff · ½ · [ (Σ y cos ω(t−τ))² / Σ cos² ω(t−τ) + (Σ y sin ω(t−τ))² / Σ sin² ω(t−τ) ]
dt_eff = (t_last − t_first)/(N − 1)
```

- **Against astropy:** `dt_eff × LombScargle(t, y, fit_mean=False, center_data=True,
  normalization='psd').power(f)` equals the scripts' value to 5–8×10⁻¹⁴ (methods `slow`, `cython`).
  astropy's default `fit_mean=True` (the floating-mean estimator of Zechmeister & Kürster 2009) is a
  different estimator and differs by 2×10⁻⁶ of the peak on the test series: do not use it for
  parity. ([astropy API](https://docs.astropy.org/en/stable/api/astropy.timeseries.LombScargle.html))
- **Against v1:** `state::lomb_scargle` at `v1-final` is the same τ form on mean-subtracted data
  **without the `dt_eff` factor**. The rebuild needs the factor to match the scripts' units
  (rad²/Hz).
- **Against the classical periodogram:** on a uniform grid the scripts' LS and
  `(Δt/N)|rfft(y − ȳ)|²` agree at interior Fourier bins to 1×10⁻¹⁰ (measured ratio
  1 ± 1×10⁻¹⁰, N = 2000). At the Nyquist bin Σ sin² = 0 and the expression is 0/0; the
  `lomb_scargle_equals_fft_on_uniform_grid` test must exclude it or define the limit.
- **Frequency grids are not uniform.** `cycle jitter` uses `linspace(0.5/T_span, 1/median(Δt), 7500)`
  ([`#L107-L111`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/cycle%20jitter#L107-L111));
  the alias-ratio scripts concatenate a coarse window at 2 points per 1/T_span with a 300-point zoom
  on the line
  ([`cadence_alias_ratio#L121-L133`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/cadence_alias_ratio#L121-L133)).
  The engine's exact estimator must therefore accept an arbitrary frequency list. A NUFFT needs
  f₀ + k·δf; each `linspace` piece is such a grid.
- One script, `alias_suppression_factor`, already abandons exact LS above N ≈ 10⁴ for a
  gridded FFT of the classical periodogram, per its own docstring
  ([`#L8-L21`](https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/alias_suppression_factor#L8-L21)).
  I did not analyse it further.

### Candidate methods, measured

Test series: jittered cadence T_c ~ U(5, 5.3) s, a 1 mrad tone at 0.05 Hz, a 0.1 mrad tone, white
noise; frequencies δf·(1…M) up to 0.1 Hz with δf = 1/(4·T_span), so M ≈ 2.06·N. "Error" is
max |S − S_exact| / max S_exact; in brackets the worst pointwise relative error, which is dominated
by the low-power bins. For N ≥ 10⁵ the exact sum was evaluated on 200 of the frequencies.

| Method | N = 10⁴ | N = 10⁵ | N = 10⁶ | N = 10⁷ |
|---|---|---|---|---|
| Exact, numpy | 6.2 s (30 ns per sample-frequency) | — | — | — |
| Exact, Rust f64, std only, 12 threads | 0.54 s (2.7 ns) | 5.4 ns measured at M = 2×10⁴ → ~110 s at M = 2×10⁵ (extrapolated) | ~3 h (extrapolated) | ~12 days (extrapolated) |
| FINUFFT f64, ε = 10⁻¹² | 0.017 s, 2×10⁻¹² (8×10⁻⁹) | 0.18 s, 6×10⁻¹¹ (6×10⁻⁹) | 1.07 s, 5×10⁻⁹ (2×10⁻⁷) | 16.5 s, 5×10⁻⁸ (9×10⁻⁷) |
| FINUFFT **f32**, ε = 10⁻⁶ | 0.06 s, 4×10⁻⁴ (12) | 0.23 s, **4×10⁻²** (5) | 0.74 s, **5.8** (380) | 12 s, 10⁵ |
| Press–Rybicki extirpolation (astropy `algorithm='fasper'`, defaults) | 0.017 s, 3×10⁻³ (51) | 0.23 s, 1.3×10⁻² (1.6) | 3.4 s, 5.5×10⁻² (0.5) | — |
| Press–Rybicki, oversampling 20, Mfft 8 | 0.057 s, 9×10⁻¹¹ (1×10⁻⁵) | 0.52 s, 2×10⁻⁸ | 9.9 s, 7×10⁻⁹ | — |
| astropy 8 `fast` default (`'lra'`, Ruiz-Antolín & Townsend 2018) | 0.058 s, 1×10⁻¹² (4×10⁻⁹) | 1.1 s, 3×10⁻¹¹ | 11.8 s, 9×10⁻¹⁰ | — |
| Own type-1 NUFFT in numpy (ES kernel, width 14, upsampling 2) | 0.08 s, 6×10⁻¹³ (6×10⁻⁹) | 1.07 s, 4×10⁻¹¹ (6×10⁻⁹) | — | — |

Reading the table:

- **Press–Rybicki with its default settings is not accurate enough.** Its error is per cent of the
  peak, and bins far from the peak can be wrong by a factor of 50. A comb whose fourth harmonic is
  10⁻⁵ of the fundamental in amplitude (the `fig2_uldm_folding` panel b fixture: 7.6×10⁴ against
  0.67 µrad/√Hz) cannot be read through it. nifty-ls reports the same in general terms
  ("many orders of magnitude more accurate" than Press & Rybicki as implemented in astropy;
  [arXiv:2409.08090](https://arxiv.org/abs/2409.08090), abstract only read). Note that astropy
  8.0.3's `method='fast'` no longer defaults to Press–Rybicki: the installed source defaults to
  `algorithm="lra"` (`astropy/timeseries/periodograms/lombscargle/implementations/fast_impl.py`).
- **An f64 NUFFT is exact for our purposes and fast.** Lomb–Scargle needs four trigonometric sums,
  which are two type-1 transforms (Σ y e^{−iωt} and Σ e^{−2iωt}); the script's rotated-sums form
  then applies unchanged. The rounding floor grows roughly as N·ε_mach, as FINUFFT's documentation
  predicts ("one cannot demand that NUFFT relative output errors be smaller than" N_max·ε_mach;
  [FINUFFT troubleshooting](https://finufft.readthedocs.io/en/latest/trouble.html)). A direct test
  of dynamic range — a jittered comb with harmonics at 10⁻³, 10⁻⁵ and 10⁻⁷ of the fundamental,
  N = 10⁵ and 10⁶, with and without shot noise — gave NUFFT and exact values identical to the four
  digits printed at every harmonic and at two off-comb frequencies. The floor in that test
  (3–5×10⁻⁵ of the fundamental in amplitude) is the jitter's own leakage, present in the exact sum.
- **A pure-Rust NUFFT is small.** The 40-line numpy prototype (spread with the
  "exponential of semicircle" kernel of Barnett, Magland & af Klinteberg 2019, FFT, divide by the
  kernel's transform) matches the exact sum to 4×10⁻¹¹ at N = 10⁵. In Rust it needs an FFT (the
  `rustfft` crate is the usual choice; not evaluated here) and about 150 lines. I did not write or
  time the Rust version; FINUFFT's times above are the target, not a measurement of it.
  `cargo search` shows NUFFT crates (`apollo-nufft`, `ducc0` bindings); none was evaluated.
- **Single precision fails, so the GPU path as planned fails.** WGSL has f32 and f16 only
  ([WGSL spec, floating-point types](https://www.w3.org/TR/WGSL/#floating-point-types); not re-read
  today). The f32 error above is the same N_max·ε_mach rule with ε = 6×10⁻⁸: the phase k·x of mode
  k ~ 10⁶ cannot be held in 24 bits. `REBUILD.md` §3.4's row "periodograms over 10⁵–10⁷ shots: GPU
  (NUFFT)" would need float-float phases and accumulation to work; the CPU f64 route already does
  10⁷ shots in seconds, so there is nothing to gain in S1.

## 5. Recommendation

1. **Classical periodogram (S1, needed by every §2.1 test).** f64, FFT-based, one function with the
   scripts' convention: `S_k = (Δt/N)|FFT(y − ȳ)|²`, `f_k = k/(NΔt)`, k = 1…N/2, not doubled, plus
   a `subtract_mean: false, keep_dc: true` option for the `1D_oscillator` fixture. Tests: tone
   height a√T_obs/2; white-noise mean ASD²; the fixtures.
2. **Exact Lomb–Scargle (S1, the oracle).** f64, O(N·M), parallel over frequencies, arbitrary
   frequency list, the scripts' estimator including `dt_eff` and pre-centring. Offer floating mean
   only as a separately named option. Fixture: the scripts' `ls_psd` values or, equivalently and
   without licence questions, `dt_eff × astropy psd` with `fit_mean=False`. Practical to N ≈ 10⁵
   (minutes); it stays the truth for everything faster.
3. **NUFFT Lomb–Scargle (S1 if time allows, otherwise before S11).** f64 on the CPU, own type-1
   transform on a uniform grid f₀ + k·δf, checked against (2) on every run of the oracle tier at
   10⁴–10⁵ shots with tolerance 10⁻⁹ of the peak, and with a control arm (kernel width 8 gave
   8×10⁻¹⁰ at N = 2000; width 4 should fail). Skip Press–Rybicki entirely.
4. **No GPU periodogram in S1.** Record the deviation from §3.4 in the slice notes and put
   "float-float NUFFT on wgpu" in `plan/IDEAS.md`; revisit only if a measured CPU time is too slow
   for the cadence designer's live view.
5. **Phase parity with the scripts.** Give the engine's propagation integral a test-only
   quadrature that copies the scripts: nodes τ_j = j·dt, arms evaluated at τ_j + dt and clamped at
   launch height, trapezoid over 146/147/500 nodes, sign δφ₁ − δφ₂, first measurement at 2T. With
   that, `point_oscillator_phase_matches_script` can hold at ≤ 1e-9 and proves the geometry,
   constants, timing and kernel. Separately assert the engine's default model against the same
   fixture at a tolerance set from the table in §3 (written down before measuring), so the 10⁻³
   gap is recorded rather than absorbed. Periodogram tests (`comb_at_integer_harmonics`,
   `folded_comb_fingerprint`) compare peak positions to one bin, which neither difference moves.
6. **Fixtures.** Generate with the appendix harness at T_total = 10⁵ s (2000 s for
   `1D_oscillator`), seed 0, and keep them untracked until the owner rules on the licence.

## 6. What I could not verify, and how to resolve it

- **Whether the one-sample arm lag and the 2T − dt window are intended.** They look like artefacts
  of appending after the step and of `arange(N_fly)`, but only the author can say. A question for
  George (below). If they are fixed upstream, the fixtures and item 5 change.
- **Rust NUFFT speed and the choice of FFT crate.** Not built. Resolve in the slice: time the Rust
  transform at 10⁵ and 10⁶ shots against the FINUFFT numbers above.
- **Exact LS in Rust beyond M = 2×10⁴ frequencies.** The 10⁵ and larger rows are extrapolations
  from 2.7–5.4 ns per sample-frequency; the per-term cost rose with N (larger arguments to
  `sin_cos`, memory traffic) and may rise further.
- **NUFFT accuracy on the scripts' own cadence cases** (alternating cadence, lost shots, lift
  excision). The test series here was a positive-jitter cadence only. Resolve by running (3)
  against (2) on each §2.4 fixture when S11 builds them.
- **Primary papers.** Scargle 1982, Press & Rybicki 1989, Zechmeister & Kürster 2009, Barnett et al.
  2019 and Ruiz-Antolín & Townsend 2018 are named to identify the methods; I did not re-read them
  for this note. Every number above comes from the code read or from a measurement, and the two
  documentation claims quoted are linked.
- **The paper and the upgrade report** (the "paper §II.B" references in the validation plan) were
  not available to me; the suppression law `(A/√X)ⁿ` was not checked against the fixtures.
- **Licence status of derived arrays** (§2): a legal question, reported not decided.
- **GPU hardware.** Nothing was run on a GPU; the single-precision result is from FINUFFT's f32
  CPU build, which has the same arithmetic.

## 7. Further questions (not chased)

- For George: is the arm table meant to be one step ahead of the source, and should the integral
  cover the full 2T? Is the sign δφ₁ − δφ₂ (lower minus upper) the paper's convention? Is the atom
  ⁸⁷Sr or ⁸⁸Sr (1.46×10⁻²⁵ kg is ⁸⁸Sr)? Should AION-10 use u₀ = 3.86 (arms meet 7 cm below launch
  height at 2T) or gT − v_kick/2 = 3.909?
- Should the engine's arms stop at the launch height as the scripts' do? v1 has no floor and
  `instrument.md` §5 says "reflecting"; three different behaviours for the last 20 ms of an AION-10
  flight.
- v1's Simpson rule at `fine_dt` = 0.01 is 1×10⁻⁸–3×10⁻⁷ from the converged integral on these
  cases, looser than the 10⁻¹⁰ the anchors are held to against v1 itself. Worth a line in the
  precision note.
- The harmonic-height suppression law and the circular-orbit cases belong to S6; the fixtures here
  already contain panel d (circular) for both presets.
- The jitter floor did not visibly recede between N = 10⁵ and 10⁶ in the dynamic-range test
  (3–5×10⁻⁵ of the fundamental at single frequencies); `random_jitter_floor_recedes` should be
  specified as a statistic over a band, as the scripts do, before S11.

## Appendix: the harness used

Run with `python -I harness.py <script> <outdir> <seed> NAME=VALUE…` from a directory that is not
the clone. It contains no text from the scripts.

```python
import json, pathlib, re, sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.figure
import matplotlib.pyplot as plt
import numpy as np

plt.show = lambda *a, **k: None
matplotlib.figure.Figure.savefig = lambda self, *a, **k: None  # scripts save to a Desktop path


def load(script: str, seed: int, overrides: dict[str, str]) -> dict:
    """Execute a reference script with module constants replaced; `__main__` blocks do not run."""
    src = pathlib.Path(script).read_text(encoding="utf-8")
    for name, value in overrides.items():
        pattern = re.compile(rf"^(\s*){re.escape(name)}(\s*)=[^=\n;]*", re.M)
        src, n = pattern.subn(lambda m: f"{m.group(1)}{name}{m.group(2)}= {value} ", src)  # every match
        assert n >= 1, f"override {name} not applied"
    np.random.seed(seed)  # the scripts draw noise from the legacy global stream
    namespace = {"__name__": "ref"}
    exec(compile(src, script, "exec"), namespace)
    return namespace
```

For `1D_oscillator` and `100m_oscillators`, save the module arrays named in §2 from the returned
namespace. For the `fig2_*` scripts, call the script's own functions:
`raw = ns["compute_phases"](ns["osc_b"])` (add `circular=True` for `osc_d`),
`uldm = ns["uldm_phase_vec"](ns["t_meas"], ns["f_phi"], ns["m_phi"])`, then
`np.random.seed(seed)` once and, per panel in the script's order, one
`np.random.normal(0, asd/np.sqrt(T_cyc), N_cycles)` draw and `ns["periodogram"](...)`.
The benchmark scripts behind §3 and §4 (script-versus-continuous decomposition, the Lomb–Scargle
accuracy and cost tables, the dynamic-range test, the Rust exact-sum timing) were run from
`%TEMP%\cav-ref-kkmFuM` and are not in the repository; that directory is disposable.
