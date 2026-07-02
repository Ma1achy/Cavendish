# Cavendish — `uldm` drill-down

> Subsystem design for the `uldm` crate: the closed-form coherent ultralight-dark-matter phase —
> the **common-mode** channel the Gradar tracker rejects and the DM detector's signal. Companion to
> `DESIGN.md` (§6 inventory) and the spec (§ULDM `sec:uldm`, `eq:uldm`, `tab:uldm`). Small and
> self-contained.
>
> **Dependencies:** `uldm → math` (the `Scalar` seam, for the closed form and its derivatives). It
> does **not** depend on `gravity` — ULDM is **not a mass**, so it never touches the field kernel.

---

## 1. Responsibility & boundaries

**Owns:** the closed-form ULDM phase `ΔΦ_φ(t)` and its parameters.

**Does not own:** anything gravitational (no `Cloud`, no kernel), the additive measurement noise
(`noise`), the way it is mixed into the signal (`compute`/​`generate`). `uldm` produces one scalar
phase per time; it is added downstream.

**Invariant it guarantees:** the ULDM phase is **identical across every detector** in the array
(common-mode) — it carries no placement dependence (§3, `INV.4`).

---

## 2. The physics — a transition-frequency oscillation, not a mass

ULDM is a scalar dark-matter field `φ` oscillating at `f_φ = m_φc²/h ≈ 0.1 Hz` (for
`m_φ ≈ 4×10⁻¹⁶ eV`). Through dilatonic couplings `d_e, d_{m_e}` it modulates the **atomic
transition frequency** `ω_A`, and that modulation — not any gravitational pull — imprints the phase
(spec `sec:uldm`):

```
ΔΦ_φ(t) = 8 (δω_A/ω_φ)(Δr/L) · [interferometer response] · cos(ω_φ((2T+L/c)/2 + t) + θ)
δω_A    = ω_A √(4πG_N) · (coupling term in d_e, d_{m_e})
```

It is a coherent oscillation at `ω_φ = 2πf_φ`; its amplitude scales with `δω_A` (the couplings), the
arm separation `Δr`, and inversely with the baseline `L`. Parameters: `tab:uldm`. Verified against
George's `uldm_oscillators_demo` (the spec matches it line-for-line).

---

## 3. Common-mode — why the array cannot localise it

The ULDM field is coherent over astronomical scales, so every detector in the array sees the
**same** phase, in step — there is no placement dependence and hence no differential signal across
the array. This is the load-bearing asymmetry of the whole Gradar idea (spec §array): **common-mode
across the array → ULDM; differential across the array → a local mass.** The tracker rejects this
common channel to isolate moving masses; a DM search reads it directly. So `uldm_phase` is a
function of `t` (and the shared instrument geometry) **only** — not of the detector.

---

## 4. The seam — an additive phase contributor

```rust
pub fn uldm_phase<S: Scalar>(cfg: &UldmConfig, geom: &InstrumentConfig, t: S) -> S;
```

Not a `PhaseModel` and not a `NoiseSource` — a plain closed-form term added to the per-measurement
phase. Because it is identical across detectors, it is computed **once per (scenario, measurement)**
and broadcast, never recomputed per detector (the common-mode optimisation). Generic over `Scalar`
so a `Dual` tangent flows for the **DM-detection CRB** (sensitivity to `d_e`, `m_φ`), the analysis
analogue of the track CRB.

---

## 5. Connection to `compute`

Added in the forward loop (`compute.md` §4 control flow: `phi += uldm.phase(t_ℓ)`). The CPU path
calls `uldm_phase`; the `WgpuBackend` re-expresses it in WGSL — trivial, a single cosine — and
evaluates it **once per measurement** per scenario, then adds it to every detector's `ΔΦ`.

---

## 6. Errors & API

`uldm_phase` is total (no fallible path); `UldmConfig` is validated at construction (non-negative
`m_φ`, `ρ_φ`). Public surface: `UldmConfig`, `uldm_phase`. `scenario` carries the `UldmConfig`;
`compute`/​`generate` add the phase.

---

## 7. Exit requirements (definition of done)

| Requirement | Check | Tol |
|---|---|---|
| oracle | reproduces `uldm_oscillators_demo` over a run | ≥99% |
| common-mode | `ΔΦ_φ` identical across all detectors | exact (`INV.4`) |
| spectral line | Lomb–Scargle of the ULDM-only signal shows a line at `f_φ` | tol |
| coupling scaling | amplitude `∝ δω_A` (linear in the dilatonic couplings) | exact |
| autodiff | `uldm_phase` differentiable w.r.t. `d_e`, `m_φ` via `Dual` | ~1e-6 |

Needs nothing beneath it but `math`; verifiable in isolation against the demo.

---

## 8. Open sub-questions (resolve in implementation)

- **Stochastic phase `θ`.** Whether the ULDM oscillation phase is fixed per scenario or itself drawn
  (a coherence-time model) — `tab:uldm` fixes `θ`; a realistic search may randomise it per
  realisation.
- **Multiple masses.** A second ULDM field/​mass (a small spectrum of lines) is a trivial sum of
  closed forms if the science ever wants it — noted, not built.
