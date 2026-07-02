# Mref — Reference port (George's cases) — implementation brief

> A faithful Rust port of George Parish's validation cases as an **independent** oracle. Each
> Cavendish anchor then asserts `cavendish ≈ reference` — two independent implementations of the
> same physics agreeing — rather than `cavendish ≈ 50 µrad` against a remembered figure. The
> reference reproduces George's *method* (direct quadrature potentials, his arm construction, his
> FFT), which is deliberately **different** from Cavendish's voxelised, differential-first,
> Nagy-prism path, so agreement is meaningful.
>
> **Not one milestone — a thread.** The `reference` crate grows case-by-case *alongside* the
> milestone whose physics each case exercises (below). Source: George's repo (extensionless Python
> scripts, one per case).

---

## 1. Why port rather than copy numbers

Copying `~7 mrad`/`~50 µrad` once bakes in a transcription and a frozen assumption. Porting instead
gives: an oracle that **runs in CI**; **method independence** (quadrature vs voxels — a bug in one
is unlikely to be mirrored in the other); and the anchors become **regression tests with a live
reference**, not magic constants. It also cross-checks the *reference itself* — George's grid-spline
shortcuts and `nquad` tolerances are replaced by direct quadrature in the port, so the port validates
George too.

---

## 2. The `reference` crate

`crates/reference/` — pure Rust, no dependency on `shape`/`compute`/`generate` (independence is the
point). It re-implements, per case: the atom arms (George's exact two-segment construction, §4), the
source potential by **direct quadrature or closed form** (not voxels), the per-cycle phase integral,
and his spectral step (FFT or LS). Feature-gated (`--features reference`) so the core engine builds
without it. Public surface: one function per case returning the time series `ΔΦ(t_ℓ)` and, where
relevant, the periodogram — the exact arrays a Cavendish run must match.

Dependencies: `reference → math` only (for constants/quadrature helpers). It is a sibling oracle,
consumed by the anchor tests in `tests/`.

---

## 3. The case catalogue (13 cases → milestones)

| Case (file) | Physics | Validates in | Cavendish maps to |
|---|---|---|---|
| `cuboid` | solid-cuboid potential (3-D quadrature), oscillating concrete wall, FFT | M1/M2 | `shape` cuboid + `gravity` + `instrument` (**concrete-wall anchor ~50 µrad**) |
| `1D_oscillator` | point-mass `−GM/√(D²+(z−h)²)`, 1-D oscillation | M2 | point source + `Oscillation` path (**~2 mrad**) |
| `cylinder` | cylinder vs point-mass, swept over R, H, z₀ | M2 | `shape` cylinder; near/far convergence |
| `scaffold` | **union** of cylinder members, amplitude ramp 1 µm→1 mm in z | M2 | `shape` `Union`; multi-member body (**scaffold anchor ~10 µrad**) |
| `uldm_oscillators_demo` | ULDM line + up to 3 GGN oscillators, continuous, LS | M5/M6 | `uldm` channel + Lomb–Scargle |
| `lift_uldm_osc_pipeline` | ULDM + moving 1000 kg lift (transients), excision, LS recovery | M5/M6 | moving mass + ULDM + `mask` excision + LS |
| `lift_excision_demo` | lift transient identification + excision | M5/M6 | contamination `mask` |
| `gapped_fft_vs_ls` | uniform cadence, random shot failures, **FFT vs LS** | M6 | gapped `Schedule`; LS-not-FFT |
| `gapped_fft_vs_ls_averaged` | as above, averaged over realisations | M6 | gapped schedule statistics |
| `cycle jitter` | jittered cycle time `T_c`, non-uniform `t_m` | M6 | jittered `Schedule` |
| `thinning_pipeline` | data thinning / down-selection | M6 | schedule down-selection |
| `nighttime_gapped_series` | day/night gap structure | M6 | realistic gapped `Schedule` |
| `week_pipeline` | week-long end-to-end (the capstone) | M6 | full stream integration |

Ordering: `cuboid`,`1D_oscillator` first (they gate M1/M2), then `cylinder`,`scaffold`, then the
M5 ULDM/lift cases, then the M6 schedule/spectral cases, `week_pipeline` last.

---

## 4. Pinned from George (adopt verbatim — the reference *is* the convention)

```
G = 6.67430e-11    m_atom = 1.46e-25 kg (⁸⁷Sr; George's comment says "Sr-88" but the value and the
                   physics are ⁸⁷Sr per AION/Carlton Table I — spec is correct, do not change)
hbar = 1.055e-34   wavelength = 698e-9   wavenumber = 2π/λ ≈ 9.002e6 m⁻¹
n = 1000           velocity_boost = n·ħ·k/m_atom ≈ 6.5 m/s
T = 0.73 (2T=1.46) g = 9.81   u_initial = 3.86 m/s   dt = 0.01   cadence = 2 s   Δr = 5 m
```

**Arm construction (George's exact form, §cuboid):** two segments per interferometer over `[0,T]`
then `[T,2T]`, floor at the launch height, with the recoil applied as:
`lower = launch(u) then (v+boost)`, `upper = launch(u+boost) then (v−boost)`; a floor clamp
(`z≤floor & v<0 ⇒ stop`). **Sign:** George forms `ΔΦ = δφ₁ − δφ₂` (lower minus upper) — the port
matches this; Cavendish's spec uses `δφ₂ − δφ₁`, so the agreement test aligns the global sign (or
compares magnitudes).

---

## 5. Independent-method design (what the port does *differently* from Cavendish, on purpose)

| Ingredient | `reference` (George) | `cavendish` |
|---|---|---|
| cuboid/cylinder potential | direct 3-D/2-D adaptive quadrature | voxelised `Cloud` sum + Nagy prism near-field |
| arm/phase differencing | `V(upper) − V(lower)` evaluated separately | differential-first (never forms large `V`) |
| spectrum | `np.fft.rfft` (uniform) / LS (gapped) | Lomb–Scargle first-class |
| precision | f64 throughout | f64 (CPU ref) / f32 (GPU) |

Agreement across *these* differences is the signal. (The port drops George's grid-spline
interpolation of the cuboid `V` — a speed hack — for direct quadrature, so it is the cleaner oracle.)

---

## 6. Tests (the agreement suite)

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `ref_arms_match_george` | ported arm trajectories vs a stored sample of George's arm arrays | ≤1e-9 |
| unit | `ref_cuboid_quadrature` | ported cuboid `V` vs the closed-form Nagy prism potential at sample points | ≤1e-6 rel |
| integration | `cuboid_agree` | `cavendish` voxelised wall vs `reference` `cuboid` `ΔΦ(t)` series | ≤2% (voxel `h`) |
| integration | `oscillator_agree` | `cavendish` point-oscillation vs `reference` `1D_oscillator` | ≤1e-3 rel |
| integration | `cylinder_agree` | swept R/H/z₀: `cavendish` vs `reference` `cylinder` | ≤2% |
| integration | `scaffold_agree` | union-of-members vs `reference` `scaffold` (~10 µrad) | ≤5% |
| integration | `uldm_ls_agree` | ULDM+oscillator periodogram peaks vs `reference` `uldm_oscillators_demo` | bin-exact / ≤5% height |
| integration | `lift_excision_agree` | contaminated-cycle mask + LS recovery vs `reference` `lift_uldm_osc_pipeline` | recovered f bin-exact |
| e2e | `gapped_fft_vs_ls_agree` | on the same gappy series: `cavendish` LS ≈ `reference` LS; both beat zero-filled FFT | structural + ≤5% |
| e2e | `week_pipeline_agree` | the capstone series vs `reference` `week_pipeline` (spectral peaks + noise floor) | ≤10% |

Tolerances are looser than the internal invariants (≤1e-12) because the two methods genuinely differ
(quadrature vs voxels); the voxel term dominates and shrinks with `h` (M2's convergence test).

---

## 7. Exit requirements

| Requirement | Check | Tol |
|---|---|---|
| arms faithful | `ref_arms_match_george` | ≤1e-9 |
| reference sound | `ref_cuboid_quadrature` vs Nagy closed form | ≤1e-6 |
| M1/M2 anchors via reference | `cuboid_agree`, `oscillator_agree`, `cylinder_agree`, `scaffold_agree` | ≤2–5% |
| M5/M6 via reference | `uldm_ls_agree`, `lift_excision_agree`, `gapped_fft_vs_ls_agree` | bin/≤5% |
| capstone | `week_pipeline_agree` | ≤10% |

Each Cavendish milestone's anchor rows (M1 `anchor_wall`, M2 `anchor_moving`/`anchor_oscillation`,
M5 `sum_identity` scenarios, M6 `ls_planted_line`) are **re-expressed against the reference** rather
than against remembered magnitudes.

---

## 8. Wiring into the plan

- **M1/M2** anchor tests import `reference::cuboid`, `::one_d_oscillator`, `::cylinder`,
  `::scaffold` and assert agreement (§6) — replacing the "≤10% of a remembered figure" wording.
- **M5/M6** import the ULDM/lift/gapped/jitter cases as their spectral and schedule oracles.
- The `reference` crate is built incrementally: each case lands with the milestone it validates, so
  no milestone waits on the whole port.
- CI: a `reference` job (or a feature in the rust job) runs the agreement suite; it is a **gate**
  (unlike the software-GPU job), because it is CPU-deterministic.

## 9. Open sub-questions

- **Quadrature choice.** Adaptive (matching `nquad`) vs fixed Gauss–Legendre for the cuboid/cylinder
  — accuracy vs determinism of the oracle.
- **Storing George's outputs.** Whether to also snapshot his Python outputs as fixtures (belt-and-
  braces) or rely solely on the Rust port + Nagy cross-check.
- **Scaffold members.** Confirm the member geometry/count against `scaffold` before the `Union` map.
