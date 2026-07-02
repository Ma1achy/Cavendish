# M5 — Channels & decomposition (implementation brief)

> Everything that is not the target masses — ULDM, the additive noise stack, atmospheric GGN as a
> stochastic *field source* — and the exact ground-truth decomposition of `signal`. Read with
> `design/uldm.md`, `design/noise.md`, `design/generate.md` §4–5, and the spec (`eq:uldm`,
> `tab:uldm`, `sec:atmo`, `tab:atmo`, carlton2025).
>
> **Prereq:** M3 (multi-detector), M4 (rotating sources exist but are not required here).
> **Delivers to:** M6 (channels must survive batching), the Gradar story (common-mode rejection).
> **Crates touched:** `uldm`, `noise`, `gravity` (`FieldContribution` sum), `generate`
> (decomposition), `state` (channel fields).

---

## 1. Requirements

| ID | Requirement |
|---|---|
| M5-R1 | ULDM: the closed-form transition-frequency oscillation at `f_φ ≈ 0.1 Hz` (spec `eq:uldm`), computed once per measurement and **identical across detectors** (`signal_uldm (T,)`). |
| M5-R2 | The post-hoc `NoiseSource` stack (shot, vibration): additive, zero-mean, order-significant, seeded; the realisation recorded as `signal_noise`. |
| M5-R3 | Atmospheric GGN enters as a `FieldContribution` — a seeded stochastic `δρ` field summed into the potential **in the forward pass**, never post-hoc (spec `sec:atmo`); finite correlation length ⇒ *partial* common-mode across the array. |
| M5-R4 | The decomposition: `signal_targets + signal_atmospheric + signal_uldm + signal_noise = signal`, exact by linearity, gated by `FieldSet.decomposition`. |
| M5-R5 | `signal_per_ifo (T,D,2)`: the two single-interferometer phases before the double difference, optional. |

---

## 2. Equations

### 2.1 ULDM (coherent scalar; common-mode by construction)

```
δφ_ULDM(t) = A_φ · cos(2π f_φ t + φ₀)          f_φ = m_φ c² / h ≈ 0.1 Hz   (m_φ ≈ 4e-16 eV)
```

`A_φ`, `f_φ`, `φ₀` from `UldmConfig` (coupling, local DM density — the spec's `tab:uldm`
parameterisation). One evaluation per `t_ℓ`, broadcast to every detector — it rides *through* the
gradiometer double-difference identically, which is exactly why the array can reject it against
the geometrically-varying target channel.

### 2.2 Superposition (why the decomposition is exact)

`eq:singlephi` is linear in `V`, and `V = V_targets + V_atmo`:

```
ΔΦ_grav = (m_A/ħ)∫[ΔV_targets + ΔV_atmo]dτ = ΔΦ_targets + ΔΦ_atmo
signal  = ΔΦ_targets + ΔΦ_atmo + δφ_ULDM + n_post-hoc          (channel identity, to fp tolerance)
```

### 2.3 Atmospheric GGN realisation (Carlton 2025; spec `tab:atmo` for the spectral forms)

Two channels — infrasound (closed-form pressure→density transfer) and temperature
(Greenwood–Tarazano spatial spectrum) — realised as one stochastic density perturbation field:

```
δρ(x, t) = Σ_modes  a_m · cos(k_m·x − ω_m t + ψ_m)
   a_m² set by the channel PSD (per tab:atmo);  (k_m, ω_m, ψ_m) drawn once per scenario from the
   seeded counter RNG (key: seed ⊕ "atmo");  correlation length ℓ_c = 2π/|k|_typical.
V_atmo(p, t) = Σ_modes a_m · G_kernel(k_m; p) · cos(·)   — each mode's potential is closed-form
   (a plane-wave density has an analytic Poisson solution: ∇²V = 4πG δρ ⇒ V_m = −4πG a_m/|k_m|² · cos(·)),
   so the FieldContribution is a *sum of analytic mode potentials*: cheap, exact, differentiable.
```

Partial common-mode: detectors separated by `b ≪ ℓ_c` see nearly identical `ΔΦ_atmo`; `b ≳ ℓ_c`
decorrelates — the knob the Gradar front-end must live with.

### 2.4 Shot noise (per-shot phase floor)

```
n_shot[ℓ, d] ~ N(0, σ_shot²)  i.i.d.        σ_shot from config (spec noise table)
vibration: coloured via the configured PSD, realised per-detector, seeded.
```

---

## 3. Design

```
                        FieldContribution sum (forward pass)
   targets (Vec<Source>) ─┐
                          ├─►  V(p,t) = ΣV_c ──► PhaseModel ──► ΔΦ_grav ─┐
   AtmoField (seeded)    ─┘                                              ├─ + δφ_ULDM(t) ─ + noise stack ─► signal
                                                                          │      (broadcast)      (post-hoc, ordered)
   decomposition on:  run PhaseModel per group {targets-only, atmo-only} — record each channel
   decomposition off: one combined pass (default; the 2× cost saved)
```

`noise` exposes a thin `NoiseStack` (ordered `Vec<Box<dyn NoiseSource>>`) plus the `AtmoField`
config-adaptor that *constructs* the `FieldContribution` — atmospheric is configured with the
noise, but lives in the field (the boundary `design/noise.md` settled).

---

## 4. Pseudocode

```
fn run(scenario):
    atmo = scenario.noise.atmo.map(|c| AtmoField::realise(c, key(seed,"atmo")))   # modes drawn once
    for t in schedule:
        if fields.decomposition:
            φ_t[t,d] = ΔΦ(targets_only, d, t)
            φ_a[t,d] = ΔΦ(atmo_only,    d, t)        # second pass — the ≈2× cost
            grav     = φ_t + φ_a
        else:
            grav[t,d] = ΔΦ(targets + atmo, d, t)     # one pass
        u[t] = uldm.phase(t)                          # once, broadcast over d
    clean = grav + u
    noise = zeros; for src in stack (in order): src.add(times, &mut noise, key(seed, src.id))
    signal = clean + noise
    if decomposition: store φ_t, φ_a, u, noise       # sums to signal by construction
```

---

## 5. Tests

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `uldm_common_mode` | `δφ_ULDM` identical across `D`; frequency = `f_φ`; amplitude = `A_φ` | exact / ≤1e-12 |
| unit | `mode_potential_analytic` | one plane-wave `δρ` mode: `V_m` matches `−4πG a/|k|²·cos(·)`; `∇²V = 4πG δρ` by finite difference | ≤1e-8 |
| unit | `noise_stack_order` | two non-commuting mock sources: output depends on order as declared; each zero-mean over 10⁵ draws | ≤3σ statistical |
| unit | `noise_seeded` | same seed → identical realisation; different key → decorrelated | exact / structural |
| integration | `atmo_in_field_channel` | with decomposition on: atmospheric appears in `signal_atmospheric`, is absent from `signal_noise` | structural |
| integration | `per_ifo` | `signal_per_ifo[…,1] − signal_per_ifo[…,0]` = the gradiometer `ΔΦ` per detector | ≤1e-12 |
| integration | `fieldset_gating` | decomposition off ⇒ channel fields `None` and only one gravitational pass ran (instrument call count) | structural |
| e2e | `sum_identity` | `φ_t + φ_a + u + n = signal` over a full run | ≤1e-10 rel (f64) |
| e2e | `noise_recoverable` | `signal − signal_noise` = the clean forward signal, bit-for-bit | exact |
| e2e | `common_mode_structure` | across a 2-detector array: corr(uldm ch.) = 1; corr(atmo ch.) ∈ (0,1), decreasing as baseline grows past `ℓ_c`; corr(targets ch.) varies with geometry | structural |

---

## 6. Exit requirements

| Requirement | Check | Tol |
|---|---|---|
| decomposition superposes | `sum_identity` | ≤1e-10 |
| atmospheric boundary | `atmo_in_field_channel`, `mode_potential_analytic` | structural / ≤1e-8 |
| noise invertible | `noise_recoverable`, `noise_stack_order` | exact |
| common-mode structure | `uldm_common_mode`, `common_mode_structure` | exact / structural |
| cost gate | `fieldset_gating` | structural |

## 7. Traceability

M5-R1 → uldm_common_mode, common_mode_structure · M5-R2 → noise_* · M5-R3 → mode_potential_analytic, atmo_in_field_channel, common_mode_structure · M5-R4 → sum_identity, fieldset_gating · M5-R5 → per_ifo.
