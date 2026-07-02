# Cavendish — test cases & physics from George's code (working note)

Distilled from the `atom-interferometry` repo
(`github.com/Thranduil02/atom-interferometry`, HEAD `0e8579c`, 2026-06-24).
Destined for the §9 validation tiers and the scenario/schedule additions in
`cavendish.tex`. Organised around the test cases we must recreate; the physics,
schedule layer, and open scope questions follow.

---

## 0. The reframing (sets the purpose of everything below)

George's new scripts show what the moving-mass forward model is *for*. The
scientific target of the AION work is **ultralight dark matter (ULDM)** — a
coherent phase oscillation at **f_uldm = 0.1 Hz**. The moving masses (lifts,
oscillating equipment) are **gravity-gradient noise (GGN)**: the contaminating
background to be removed, not the signal.

- **Cavendish's moving-mass forward model *is* the GGN model.**
- The pipeline (in `lift_uldm_osc_pipeline`, `thinning_pipeline`, `week_pipeline`):
  inject ULDM + GGN + realistic corruption → excise transients → Lomb–Scargle →
  least-squares notch out the GGN → recover the 0.1 Hz peak.
- Physics nicety: `m_phi = 4e-16 eV` and `f_uldm = 0.1 Hz` are not independent.
  f = m_φc²/h (4e-16 eV → 0.097 Hz). **The DM particle mass *is* the signal
  frequency**; recovering the peak frequency = measuring the DM mass.

---

## 1. Test cases to recreate

The four forward-model oracles, with exact parameters and the quantity Cavendish
must reproduce. These become named regression targets in §9.

### T1 — point-mass oscillator  (`1D_oscillator`) — Tier 1

Single point mass oscillating vertically; propagation-phase double-difference.

- Source: `V = −GM/√(D² + (z−h)²)`, `h(t) = h₀ + A cos(2πf t)`.
- Params: M = 1 kg, D₀ = 5 m, A = 1 m, f = 0.1 Hz, h₀ = 1 m.
- Two IFOs at floors z = 0 and z = 5; arms via the standard launch/kick model.
- Cadence T_cyc = 2 s; ΔΦ tagged at the **end** of each 2T window
  (t = 1.46, 3.46, … s); run T_total = 300 s.
- **Assert:** reproduce ΔΦ(t) (µrad scale) and the periodogram peak at 0.1 Hz.
- Note: implements the window integral as a sliding-trapezoid cumsum
  (`cum[L:] − cum[:−L]`). See the conditioning flag in §2.

### T2a — solid cuboid  (`cuboid`) — Tier 2 (voxel convergence)

- Source: `gravitational_potential` = `nquad` over the cuboid volume
  (George's eq 121), evaluated on a precomputed (x₀, z) grid +
  `RectBivariateSpline` + `quad` over each 2T window.
- Params: M = 38 880 kg; sides a = 0.225 (x), b = 6 (y), c = 12 (z) m; centre
  (x₀ = 5, y₀ = 0, z₀ = 6); oscillation amp 1e-6 m at 0.1 Hz; N = 100 cycles.
- **Assert:** a voxelised cuboid → converges to this `nquad` reference as voxel
  size → 0.

### T2b — solid cylinder  (`cylinder`) — Tier 2 (voxel convergence)

- Source: `cylinder_potential` = `nquad` in cylindrical coords (eq 122).
- **Assert:** voxelised cylinder → converges to this `nquad` reference.

### T2c — cylinder vs point mass, swept  (`cylinder`, Parts 1–2) — Tier 2 / identifiability

The most valuable single script. Sweeps **max|ΔΦ − mean|** for a cylinder against
a point-mass reference of equal mass, over geometry and mass height.

- Part 1: M = 100 kg, mass centre x₀ = 3 (osc amp 0.1, 0.1 Hz), z₀ = 1; sweep
  R ∈ [0.05 … 2.5] m at H = 1; sweep H ∈ [0.05 … 25] m at R = 0.5.
- Part 2: M = 100 kg, R = 0.5; sweep H ∈ logspace(0.05, 30, 25) at mass heights
  h₀ ∈ {0, 0.5, 1, 2.5, 4, 5} m, each vs its own point-mass reference.
- **Use twice:** (i) the cylinder oracle; (ii) the **empirical near/far + multipole
  validation** — the R/H at which the extended body departs from the monopole is
  exactly the curve our multipole-truncation-vs-noise-floor and far-field cutoff
  must reproduce. This is the identifiability question (when does shape matter?)
  made concrete and pre-computed.

### T3 — end-to-end ULDM recovery  (`lift_uldm_osc_pipeline`, `week_pipeline`) — Tier 3 (spectral/systems)

Full chain: ULDM + GGN + corruption → excise → LS → notch → recover 0.1 Hz.

- ULDM signal: closed form, params in §2.
- GGN sources: 1–3 point-mass oscillators. `lift_uldm`: M = 1, h₀ = 1, D = 5,
  A = 1, f_sig = 0.12. `week_pipeline`: three oscillators A/B/C —
  f = 0.11 / 0.20 / 0.08 Hz, M = 1, A = 1, D = 5 / 8 / 3, h₀ = 1 / 3 / −3.
- Lift transient: M = 1000 kg, D = 10 m, travel h = −10 → +20 m (clipped) at
  v = 1–2 m/s ⇒ 30 s transit; Poisson arrivals (mean 300–3600 s); excised with
  ±10 s padding.
- Corruption: nighttime gaps (on ~18:00–08:00, ±30 min jitter), MOT failures
  (p_fail = 0.01), cycle jitter (±0.1 s, or T_c ~ U(1.9, 3.0) s).
- Recovery: excise lift cycles → Lomb–Scargle → least-squares notch at each
  oscillator's f **and 2f** → ULDM peak. Detection threshold
  `noise_floor = median(√S off-peak)`.
- **Assert:** the 0.1 Hz peak is recovered above the noise floor after the GGN
  notch, on non-uniform/gapped data.

---

## 2. Physics reference

### Confirmed (matches `cavendish.tex`)

- Forward model: per-IFO `δφ = (m/ℏ) ∫[V_up − V_low] dt` over the 2T window;
  gradiometer `ΔΦ = δφ₂ − δφ₁`. Linear in source mass.
- Geometry: two IFOs at z = 0 and z = 5; L_base = 10 m, δr = 5 m;
  v_kick = n·ℏk/m with n = 1000.
- Constants: m_atom = 1.46e-25 kg; ℏ = 1.055e-34; λ = 698e-9 (k = 2π/λ);
  T = 0.73 (2T = 1.46); g = 9.81; u₀ = 3.86; T_cyc = 2.0; fine_dt = 0.01.
  G = 6.67e-11 in the oscillator scripts, 6.67430e-11 in the geometry scripts.

### New — the ULDM phase (closed form, additive, cheap)

`uldm_phase(t)`:

    Δω_A = ω_A · √(4π G_nat) · (d_m_e + (2 + ξ) d_e) · (√(2 ρ_φ) / m_φ) · conv
    ΔΦ_uldm(t) = 8 · (Δω_A / ω_φ) · (δr / L)
                 · sin(ω_φ n L / (2c))
                 · sin(ω_φ (T − (n−1) L / c) / 2)
                 · sin(ω_φ T / 2)
                 · cos(ω_φ ((2T + L/c)/2 + t) + θ)

with ω_φ = 2π f_uldm. Parameters: G_nat = 6.708e-39 (GeV⁻²), d_e = 1, d_m_e = 1,
m_φ = 4e-16 eV, ρ_φ = 0.3 GeV/cm³, f_uldm = 0.1 Hz, θ = π/2, ξ = 0.06,
ω_A = 2.67e15 rad/s, n = 1000, L = 10, δr = 5, conv = 2.77e-12. Deterministic;
not a gravity cloud.

### Two flags

- **Species label noise.** `1D_oscillator`/ULDM say Sr-87, `cuboid` comments say
  Sr-88, ξ is commented "Sr-86." Mass is 1.46e-25 everywhere, so the number is
  consistent; do not trust the element label in any single file.
- **Validates NFR-9 (conditioning).** `1D_oscillator` forms the window integral as
  `cum[L:] − cum[:−L]`, a difference of a global cumulative sum that grows ~N over
  a long run — the catastrophic-cancellation pattern we flagged. Confirms the GPU
  kernel must integrate each 2T window **directly** (differential-first), not via a
  global cumsum.

---

## 3. New scope — the measurement-schedule / corruption layer

Six scripts model cadence and corruption. Cavendish currently assumes a uniform
Δt axis; George's data are non-uniform, gapped, and thinned.

- **Cycle jitter** — T_c ~ U(1.9, 3.0) s, or a ±0.1 s per-shot offset ⇒
  non-uniform measurement times.
- **MOT / shot failures** — each shot drops independently, p_fail ≈ 0.01.
- **Nighttime operation** — on ~18:00–08:00 (or 08:00–24:00) with ±30 min jitter
  on the switch times ⇒ daily gaps; 7-day runs.
- **Lift events** — Poisson arrivals (mean 5–60 min); 1000 kg at D = 10 m,
  −10 → +20 m at 1–2 m/s (`np.clip` to range — our `Lift` = clipped vertical
  line); excised with ±10 s padding.

**Three consequences for Cavendish:**

1. **StateBundle's leading T axis must carry explicit per-measurement
   timestamps**, not an implicit uniform Δt.
2. **Scenario layer needs a `Schedule`/cadence model** — jitter distribution, gap
   windows, failure probability, Poisson transient events — alongside the existing
   trajectory model.
3. **SDK spectral output must be Lomb–Scargle, not FFT.** `gapped_fft_vs_ls`
   exists to show FFT breaks on gapped/non-uniform data while LS handles it. The
   "eq 66 periodogram" only works for the idealised uniform case. GGN removal is a
   least-squares notch at each oscillator's f **and 2f** — the 2nd harmonic
   because large-amplitude oscillation is anharmonic (ties to the PendulumODE
   "harmonics at nf₀" note).

---

## 4. Open scope questions (decide before folding into the doc)

1. **What does the JEPA output?** GGN-source characterisation (which moving mass,
   where, on what trajectory — the historical framing), or ULDM detection directly
   (is there a 0.1 Hz peak, at what amplitude/phase, GGN as nuisance), or multi-task
   both? Decides what the label *is*.
2. **Is the corruption/schedule layer in scope for the training data?** If the JEPA
   is meant to transfer to George's real pipeline, gaps/jitter/failures are part of
   the input distribution, not an afterthought — a fair amount of new scenario
   machinery.
3. **Excise-then-feed, or learn robustness?** His pipeline excises lift transients
   before analysis. Does Cavendish hand the JEPA pre-excised clean data, or
   contaminated data plus a contamination mask so it learns to handle transients?

---

## 5. Where this lands in the doc (next actions)

- **§9 validation tiers** — add T1, T2a, T2b, T2c, T3 as named regression targets
  with the parameters above.
- **Scenario layer (+ Appendix)** — add the `Schedule`/cadence model (jitter, gaps,
  failures, Poisson transients) and the `Lift` transient; explicit per-measurement
  timestamps on the StateBundle T axis.
- **SDK** — switch the spectral output to Lomb–Scargle (Scargle 1982); note the
  least-squares harmonic notch as the GGN-removal reference.
- **(Pending Q1)** — if ULDM is a target, add the closed-form ULDM phase as an
  additive signal source and a corresponding label.
