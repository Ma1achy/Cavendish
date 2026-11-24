# Fixtures from George's reference scripts

Arrays made by running the reference scripts behind the oscillatory-noise paper. Only the arrays, the
harness that makes them (`scripts/reference_fixtures.py`) and this file are in the repository; none of
the scripts' text is.

| | |
|---|---|
| Repository | `https://github.com/Thranduil02/atom-interferometry` (public; 38 scripts, no README) |
| Commit | `d7d360ad92837bec929b85091b2bdd9af81d285c`, Mon 26 Oct 2026 17:17:57 +0000 |
| Author | George Parish |
| Run on | 21 Nov 2026, the build PC (Windows 10), Python 3.14.3, numpy 2.5.5, matplotlib 3.11.2 (Agg) |
| Seed | `np.random.seed(0)` before each script, and once more before the noise draws of the `fig2_*` cases |

## Permission and licence

The repository has **no licence file**. George has given permission to use his scripts and to commit
fixtures made by running them: `plan/PROGRESS.md`, "Owner's rulings (21 Nov 2026)", "Fixtures from the
reference scripts". The same file records his answer that the scripts' one-step offset between the arm
and the source is not intended ("For George", answer 2), which is why every phase series here is kept
twice.

## How they were made

The clone was made in a fresh temporary directory outside this repository and treated as untrusted:
every script was read before it was run (they import numpy and matplotlib only, read no files, and
their one side effect is `savefig` to a Desktop path, which the harness turns off); Python ran with
`-I` from a throwaway virtual environment holding numpy and matplotlib; the harness ran from this
repository, never from inside the clone.

```
git clone https://github.com/Thranduil02/atom-interferometry <tmp>/clone     # at d7d360a
python -m venv <tmp2>/venv  &&  <tmp2>/venv/Scripts/python -m pip install numpy matplotlib
<tmp2>/venv/Scripts/python -I scripts/reference_fixtures.py <tmp>/clone oracle/fixtures/scripts
```

It takes about six seconds. The scripts' lengths are module constants, so the harness executes each
script's source with those constants replaced (every match), `__name__ = "ref"` so that `__main__`
blocks do not run, `plt.show` and `Figure.savefig` as no-ops, and the scripts' printing discarded.

| Case | Script | Override | Shots | Instrument |
|---|---|---|---|---|
| `1d_oscillator/` | `1D_oscillator` | `T_total = 2000` (published: 10⁴) | 1000 | AION-10: T = 0.73 s, u₀ = 3.86 m/s, IFOs at 0 and 5 m, cycle 2 s |
| `100m_oscillators/` | `100m_oscillators` | `T_total = 2e4` (published: 2.5×10⁶) | 4000 | AICE: T = 2.5 s, u₀ = gT − v_kick/2, IFOs at 0 and 70 m, cycle 5 s |
| `fig2_10m/` | `fig2_10m` | `T_total = 2e4` (published: 3×10⁷) | 10 000 | AION-10, f_N = 0.25 Hz |
| `fig2_uldm_folding/` | `fig2_uldm_folding` | `T_total = 2e4` (published: 3×10⁷) | 4000 | AICE, f_N = 0.1 Hz |
| `fig2_folding_map/` | `fig2_folding_map.py` | `T_total = 1e3` (only its `fold_frequency` is used) | — | f_N = 0.1 Hz |

2×10⁴ s is a whole number of periods of every tone in the `fig2_*` scripts (0.05, 0.3275, 0.35625 and
0.2125 Hz; 0.02, 0.131, 0.1425 and 0.085 Hz), so every peak sits on a bin. Each directory's `meta.json`
holds the script's constants, the oscillator dictionaries, the overrides and the shape of every array.
All arrays are float64 `.npy` in C order.

## What the arrays are

**Published and same-time.** The scripts build each arm table by stepping and appending the position
*after* the step, so `z[j]` is the arm at (j + 1)·dt, and they pair it with the source at j·dt.

- `…_published`: the script exactly as published.
- `…_same_time`: the same pipeline with **only** that offset corrected: each arm table is replaced by
  the arm at j·dt (the launch height prepended, the last entry dropped). The trapezoid rule, the
  window, the clamp at the launch height and the constants are as published. For `100m_oscillators`
  and the `fig2_*` scripts the four arm arrays are replaced in the executed script's namespace and
  the script's own `compute_phases` is called again. `1D_oscillator` is module-level code with no
  such function, so the harness recomputes its sliding-window series from the script's own
  `potential_difference`, constants and cycle arm tables; with the *unshifted* tables that
  recomputation gives back the script's own `subsample_phase_diff` with a largest difference of
  **0** (bit for bit; the harness refuses to continue above 10⁻¹⁵ of the peak).

The arm tables saved (`lower1`, `upper1`, `lower2`, `upper2`) are the published ones.

| Directory | Arrays |
|---|---|
| `1d_oscillator/` | `t` (`subsample_times`), `phase_published` (`subsample_phase_diff`), `phase_same_time`, `f` (`f_k_filtered`), `S_published` (`S_phasediff_filtered`: the script's own DFT, no mean subtraction, DC kept, bins up to and including Nyquist), the four `*_cycle` arm tables (200 entries: one 2 s cycle) |
| `100m_oscillators/` | `t` (`t_meas`), `phase_published` (`phases_multi`: oscillator A alone, no ULDM, no noise), `phase_same_time`, the four arm tables (500 entries) |
| `fig2_10m/`, `fig2_uldm_folding/` | `t` (`t_meas`); `raw_<p>_published` and `raw_<p>_same_time` for panels b, c (vertical oscillators) and d (a circle in the D–h plane): `compute_phases`, GGN only; `uldm` (`uldm_phase_vec(t_meas, f_phi, m_phi)`); `noise_<p>` for a, b, c, d: one seeded stream, drawn in that order, σ = ASD/√T_cyc; `f`; `S_clean_<p>` (the script's `periodogram` of the published raw series, b, c, d); `S_noisy_<p>` (the script's `periodogram` of what it plots: a = uldm + noise, b = raw + noise, c and d = raw + uldm + noise); the four arm tables (146 and 500 entries) |
| `fig2_folding_map/` | `f_true` (2001 frequencies from 0 to 5 f_N) and `f_obs` (the script's `fold_frequency` at each) |

## Sanity numbers (this run)

| Series | Peak \|ΔΦ\| | max \|published − same-time\| / peak |
|---|---|---|
| `1d_oscillator` | 3.8605 mrad | 3.29×10⁻³ |
| `100m_oscillators` | 0.68227 mrad | 2.60×10⁻⁴ |
| `fig2_10m` b (0.05 Hz) | 4.2428 mrad | 2.82×10⁻³ |
| `fig2_10m` c (0.3275 Hz) | 3.5896 mrad | 1.61×10⁻² |
| `fig2_10m` d (0.35625 Hz, circle) | 4.5270 mrad | 1.89×10⁻² |
| `fig2_uldm_folding` b (0.02 Hz) | 9.7057 mrad | 8.89×10⁻⁵ |
| `fig2_uldm_folding` c (0.131 Hz) | 9.3774 mrad | 1.94×10⁻⁴ |
| `fig2_uldm_folding` d (0.1425 Hz, circle) | 9.4953 mrad | 2.30×10⁻⁴ |

The comb of `fig2_10m` panel b (`S_clean_b`, T_obs = 2×10⁴ s), √S at the bin of each harmonic:

| 0.05 Hz | 0.10 Hz | 0.15 Hz | 0.20 Hz |
|---|---|---|---|
| 2.567×10⁵ µrad/√Hz | 1.561×10⁴ | 9.190×10² | 7.081×10¹ |

(A line's height grows as √T_obs: these are √(2×10⁴/10⁵) of the heights the research note measured at
10⁵ s.)

## What the scripts do, as checked on these arrays

- **Interferometer 1 is the lower one**: `lower1` and `upper1` launch from z = 0, `lower2` and
  `upper2` from z = δr (5 m or 70 m). The series is δφ₁ − δφ₂: **lower minus upper**.
- Every arm table equals the closed-form parabola at (j + 1)·dt, clamped at its launch height, to
  6×10⁻¹⁵ m (AION-10) and 6×10⁻¹³ m (AICE). On AION-10 the lower arms are on the floor from index 144
  and the upper arms from index 145; on AICE no entry is on the floor (the last is 2×10⁻¹³ m above it).
- Table lengths: 146 (`fig2_10m`: the trapezoid runs over j = 0…145 and stops at 2T − dt), 500 (the
  AICE scripts, likewise), 200 (`1D_oscillator`: a whole cycle, of which its window uses j = 0…146,
  147 nodes, the full 2T).
- The first measurement time `t[0]` is 2T in every case (1.46 s and 5.0 s), and the shots are one
  cycle apart; the source's phase is zero at the first launch.

## Extended bodies (S3, TASK-S3-12)

Four more scripts, for `plan/VALIDATION-AND-RESEARCH.md` §2.3: `cuboid`, `cylinder`, `scaffold`
and `scaffold_multimode`, at the same commit (`d7d360a`; none of the four has changed since, to
the head of 24 Nov 2026, `5ff8152`). Run on 24 Nov 2026 on the build PC, Python 3.14.3, numpy
2.5.5, scipy 1.18.1, matplotlib 3.11.2 (Agg), each case in a process of its own:

```
<venv>/Scripts/python -I scripts/reference_fixtures.py <clone> oracle/fixtures/scripts --only cuboid
                                                                  (likewise cylinder, scaffold, scaffold_multimode)
```

Their lengths are `N` cycles of 2 s (`scaffold_multimode`: `T_total`), and the shots they will make
(`subsample_times`) are checked against the cap before and after. Their tables of the potential
are module constants too, so each phase series is also made a second time with the table's grids
doubled: the change is the scripts' own table error, which the oracle tests take as the measure of
how far the scripts can be trusted. An override that spans several lines (`R_sweep = np.array([…`)
is replaced whole (`_replaced`), the old cases keep the old rule.

All four scripts use their own constants: `m_atom = 1.46e-25` kg (⁸⁸Sr), `hbar = 1.055e-34`,
`G = 6.67430e-11`, `u_initial = 3.86` m/s, `T = 0.73` s, 1000 kicks of 698 nm, `g = 9.81`, IFOs
launched from 0 and 5 m, a floor at each launch height, arm tables at `dt = 0.01` s interpolated
linearly (`interp1d`) with the scripts' one-step offset (the arm at `(j + 1)·dt` paired with the
source at `j·dt`), the potential tabulated and interpolated by `RectBivariateSpline`, and the
flight integral by `scipy.integrate.quad`.

| Case | Script | Overrides | Shots | Time |
|---|---|---|---|---|
| `cuboid/` | `cuboid` | `N = 20` (published 100); doubled: `n_zgrid = 100`, `n_x0grid = 100` | 20 | 67 s, doubled 278 s |
| `cylinder/` | `cylinder` | `R_sweep`, `H_sweep` (six each), `H_vals = logspace(log10 0.05, log10 30, 25)[::3]` (nine), `h0_vals = [0, 1, 2.5, 5]` | 50 | 2402 s |
| `scaffold/` | `scaffold` | `N = 50` (as published); doubled: `n_zgrid = 60`, `n_alpha = 24` | 50 | 1431 s, doubled 3890 s |
| `scaffold_multimode/` | `scaffold_multimode` | `T_total = 200` (published 3×10⁶); doubled: `n_zgrid = 60`, `n_alpha = 24` | 100 | 1407 s, doubled 3925 s |

| Directory | Arrays (all float64) |
|---|---|
| `cuboid/` | `V_grid [50, 50]`: the script's `gravitational_potential` (its `nquad`) at the atom on the tower's axis (x = y = 0) at height `z_grid[j]` with the wall's centre at `x0_grid[i]`, J/kg; `x0_grid [50]`, `z_grid [50]`, m; `subsample_times [20]`, s: shot k is integrated over `[t_k − 1, t_k − 1 + 2T]` (the script's own window); `phase [20]`: its `phase_difference` (lower minus upper), rad; `phase_doubled [20]`: the same with both grids at 100 |
| `cylinder/` | `R_sweep [6]`, `max_R [6]`: part 1, the largest AC |ΔΦ| (rad) of a cylinder of 100 kg, H = 1 m, centred at height 1 m, oscillating as `x₀ = 3 + 0.1 cos(2π·0.1 t)` m, at each radius; `H_sweep [6]`, `max_H [6]`: the same at R = 0.5 m for each height; `pm_max [1]`: the point mass of part 1; `H_vals [9]`, `h0_vals [4]`, `results [4, 9]`: part 2, R = 0.5 m, centre height `h0_vals[i]`, height `H_vals[j]`; `pm_refs [4]`: the point mass at each `h0`; `potential_points [10, 9]` (x, y, z of the point, x₀, y₀, z₀ of the centre, M, R, H) and `potential [10]`: the script's `cylinder_potential` there, J/kg |
| `scaffold/`, `scaffold_multimode/` | `V_grid [12, 30]`: the script's `scaffold_potential_mode` (48 steel members, each by `nquad`) on the axis at height `z_grid[j]` with every member displaced by `amplitudes[i]·alpha_grid[i]` in x, J/kg; `alpha_grid [12]`, `z_grid [30]`; `subsample_times`, s (readouts, the shot launched 2T earlier); `phase`: `phase_diff` (`phase_diff_raw` in the multimode script), rad; `phase_ac`: `pd_ac`, the series less its mean; `phase_ac_doubled`: the same with `n_zgrid = 60`, `n_alpha = 24`; `amplitudes [48]`, m: each member's amplitude, linear in its centre's height; `member_centre [48, 3]`, `member_mass [48]`, `member_radius [48]`, `member_length [48]`, m and kg; `member_axis [48]`: 0, 1, 2 for a cylinder along x, y, z |

`meta.json` beside each holds the script's constants, the overrides, the doubled grids, the
scaffold's modes, the run's seconds and the sizes below.

**For George** (recorded in `meta.json`, not acted on): `scaffold_multimode` sets
`A_top = 1e-4` m while its comment says "1 mm"; the fixture keeps the script's number.

### Sanity numbers (this run)

| Case | Largest AC \|ΔΦ\| | Its doubled-grid series differs by (of that) |
|---|---|---|
| `cuboid` | 29.86 µrad | 1.16% |
| `scaffold` | 1.850 µrad | 242% |
| `scaffold_multimode` | 0.934 µrad | 101% |

The cuboid's potential is −3.35×10⁻⁷ to −4.25×10⁻⁷ J/kg over its grid; its `nquad` agrees with the
prism in 40-digit arithmetic to 1.1×10⁻¹⁴ at five points (measured before the fixture was made,
docs/slices/S3.md §3.3). **The scaffolds' phase series are dominated by their tables:** doubling
`n_zgrid` and `n_alpha` changes them by more than their own size (the members are 3.4 cm rods
0.5 m from the axis, and 30 heights over 10.5 m do not resolve them), so a phase-level comparison
with these scripts can hold the engine only to within that change; their potential tables
(`V_grid`, by `nquad`) are the precise part. The cylinder's ratio of the largest AC |ΔΦ| to the
point mass's, by `h0` (rows) and `H` (columns, 0.05 to 30 m):

| h0 | 0.05 | 0.111 | 0.247 | 0.551 | 1.22 | 2.72 | 6.06 | 13.5 | 30 |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1.0193 | 1.0190 | 1.0177 | 1.0114 | 0.9806 | 0.8416 | 0.3734 | 0.2638 | 0.0066 |
| 1 | 1.0298 | 1.0294 | 1.0274 | 1.0176 | 0.9699 | 0.7567 | 0.0620 | 0.4639 | 0.0081 |
| 2.5 | 1.0075 | 1.0074 | 1.0069 | 1.0044 | 0.9924 | 0.9421 | 0.8090 | 0.2735 | 0.0044 |
| 5 | 1.0120 | 1.0119 | 1.0111 | 1.0071 | 0.9879 | 0.8997 | 0.5811 | 0.0718 | 0.0014 |

At small H the ratio is 1.007 to 1.030, not 1: the cylinder's 0.5 m radius at 3 m, as well as its
height. It leaves 1 by 10% between H = 1.22 and 2.72 m at every h0.

## The scaffolds' potential, integrated tightly (`tight_*.npy`)

The scaffold scripts integrate each tube's potential with `nquad` at scipy's default tolerances,
an absolute 1.49e-8 on values of about 1e-9 J/kg, so each value is good to about 2e-7 of the
potential, while the change of the potential as the scaffold moves is about 1e-15 J/kg. Measured
on 24 Nov 2026: at α = −0.636, z = 7.57 m the scaffold's table changes by −1.80e-14 J/kg from
α = −0.091, and the same integrand integrated with `epsabs = 0`, `epsrel = 1e-12` changes by
+9.7e-16. The tables, and the phase series made from them, are mostly that error (which is why
doubling their grids moves the series by 242% and 101% of its peak).

`scripts/scaffold_reference.py` evaluates the scripts' integrand (the same arithmetic, written
again, with the members and amplitudes of these fixtures) tightly at 3 × 6 of each table's points
and writes, for `scaffold/` and `scaffold_multimode/`:

| Array | Shape | What |
|---|---|---|
| `tight_alpha.npy` | (3,) | α of each row: the table's α nearest zero, then its 1st and 4th |
| `tight_z.npy` | (6,) | heights on the tower's axis, metres: every 5th of the table's `z_grid` |
| `tight_V.npy` | (3, 6) | the potential, J/kg |

Run 24 Nov 2026, Python 3.14.3, numpy 2.5.5, scipy 1.18.1: 223 s and 239 s.
## The scripts' pipeline, written again (`phase_exact.npy`, `phase_ac_pipeline*.npy`)

Added after S3's review (`docs/slices/S3.md` §8). `scripts/script_pipeline.py` writes again the
pipeline the `cuboid`, `scaffold` and `scaffold_multimode` scripts share (their arm tables a step
ahead of the source, `interp1d`, `RectBivariateSpline`, `quad` over the window), and is tied to
them before it writes anything: run with `quad` on each script's own table it gives the published
`phase.npy` to 1.2e-16 (cuboid) and 1.7e-16 (both scaffolds) of its peak. Run 24 Nov 2026,
Python 3.14.3, numpy 2.5.5, scipy 1.18.1, 9 minutes:

    <venv>/Scripts/python -I scripts/script_pipeline.py oracle/fixtures/scripts

| File | What |
|---|---|
| `cuboid/phase_exact.npy` (20,) | the script's own table and rule with `quad` replaced by exact integration (Gauss–Legendre, 8 points a step, between whose ends the arms are straight): its `quad` is 2.475e-5 of the phase off it, in the DC and the AC alike |
| `<scaffold>/phase_ac_pipeline.npy` | the pipeline (exact integration) on the scripts' integrand integrated tightly, on 24 α × 480 heights (the scripts': 12 × 30): AC series, radians |
| `<scaffold>/phase_ac_pipeline_half.npy` | the same at 240 heights: the last doubling (7.39e-4 and 8.44e-6 of the peak) |
| `<scaffold>/phase_ac_pipeline_alpha_doubled.npy` | the same at 48 α (1.8e-5 and 5.1e-7) |
| `<scaffold>/phase_ac_pipeline_script_grid.npy`, `..._quad.npy` | on the scripts' own grid, exactly and by their `quad`: 45% and 18% of the peak from the finest; their `quad` 800 and 1.06 times the peak off |

The tight potential of a tube here is the scripts' integrand integrated along the tube in closed
form (`asinh`) and over its section by Gauss–Legendre (24) in radius and the trapezoid rule (64)
in angle; against `tight_V.npy` (`nquad` at `epsrel = 1e-12`) it is within 7.8e-16 of V and
2.1e-8 of the largest change, checked by the harness before it writes.
