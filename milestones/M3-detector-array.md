# M3 — Detector array (implementation brief)

> Multi-detector signal — the geometry that makes localisation possible — and the fast
> `PhaseModel`. Read with `design/instrument.md` and the spec (§array `sec:array`).
>
> **Prereq:** M2. **Delivers to:** M5 (per-channel decomposition is per-detector), M8 (baseline vs
> CRB). **Crates touched:** `instrument` (array + `QuasiStaticGradient`), `state`/`generate`
> (`signal (T,D)`, `detector_placement`).

---

## 1. Requirements

| ID | Requirement |
|---|---|
| M3-R1 | `DetectorArray` of `D` gradiometers; per-detector arms built once from `detector_placement (D,7)` (position xyz + orientation quaternion; v1 vertical). |
| M3-R2 | The bundle carries `signal (T,D)` and `detector_placement (D,7)` as first-class tensors. |
| M3-R3 | The second `PhaseModel`, `QuasiStaticGradient`, exists and is selectable by config. |
| M3-R4 | `QuasiStaticGradient` agrees with `PropagationIntegral` in the uniform-gradient limit. |
| M3-R5 | Baseline geometry produces the differential signature localisation exploits: per-detector `ΔΦ` differs with source distance. |

---

## 2. Equations

### 2.1 The quasi-static gradient model

For a source field varying slowly over the cycle (2T) and smoothly over the arm separation, the
gradiometer phase reduces to the vertical gravity gradient at the detector:

```
ΔΦ_qs(t) ≈ k_eff · Γ_zz(p_det, t) · Δr · T²          k_eff = m_A·v_rec/ħ = n·2π/λ ≈ 9.0e9 m⁻¹
```

Validity: source standoff ≫ arm extent, and signal timescales ≫ 2T. Its cost is one `Γ` evaluation
per (measurement × detector) instead of 146 arm samples — the fast path.

### 2.2 The uniform-gradient limit (the agreement construction)

Impose an exactly linear external field, `V(z) = −γ·z·z_ref` (constant `Γ_zz = γ`, no higher
structure). Then `eq:singlephi` integrates in closed form and both models must give the *same*
number; residual disagreement measures only integrator resolution.

```
uniform limit:  ΔΦ_PI(γ) = ΔΦ_QS(γ) = k_eff·γ·Δr·T²        (identity, not approximation)
```

---

## 3. Design

### 3.1 Array geometry (v1: horizontally separated vertical gradiometers)

```
        z ▲    IFO₂ ─┐            IFO₂ ─┐            IFO₂ ─┐
          │          │ Δr = 5 m         │                  │        …  D towers
          │    IFO₁ ─┘            IFO₁ ─┘            IFO₁ ─┘
          └───●──────────●──────────●──────────► x   (ground positions from detector_placement)
             d₁         d₂         d₃            baseline b = span of the ●s
   source ✦ at range R: per-detector Γ_zz differs by O(b/R) — the localisation signature
```

`detector_placement[d] = [x, y, z, qw, qx, qy, qz]`; v1 orientation is the identity (vertical
sensitive axis). Arms are built in the *detector frame* once and placed by the isometry.

### 3.2 Model selection

`PhaseModel` chosen per run from config: `propagation_integral` (reference; default for validation)
| `quasi_static` (fast path). `generate` threads the choice; nothing else changes — the seam
earning its keep.

---

## 4. Pseudocode

```
fn build_array(placement: &[(Vec3, Quat); D]) -> DetectorArray:
    for (p, q) in placement:
        det = Detector { iso: Isometry3(p, q), arms: build_arms_local() }   # once
    DetectorArray { dets }

fn signal_row(t, models, array, world) -> [f64; D]:
    for (d, det) in array:
        ΔΦ[d] = model.delta_phi(world, det, t)      # PI: 146-sample integral; QS: one Γ_zz
```

---

## 5. Tests

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `placement_roundtrip` | `detector_placement` → arms → recovered positions/orientation | ≤1e-12 |
| unit | `qs_uniform_identity` | the §2.2 construction: PI ≡ QS under an exactly linear field | ≤1e-6 rel |
| unit | `qs_scaling` | `ΔΦ_qs` linear in each of `Γ_zz`, `Δr`, and `T²` | ≤1e-12 |
| integration | `signal_TD` | bundle `signal` is `(T,D)`; `detector_placement` `(D,7)`; per-detector channels independent | exact |
| integration | `baseline_differential` | two detectors, source at range R: `ΔΦ₁ ≠ ΔΦ₂`; the difference grows as the source nears the array (monotone in 1/R over a sweep) | structural |
| integration | `qs_vs_pi_far` | far, slow source (validity regime): QS matches PI | ≤1% rel |
| e2e | `anchor_per_detector` | the M2 moving-mass anchor run on a 2-detector array: the *nearer* detector reads the larger peak; both within tolerance of single-detector references at their own ranges | ≤10% |

---

## 6. Exit requirements

| Requirement | Check | Tol |
|---|---|---|
| array first-class | `signal_TD`, `placement_roundtrip` | exact |
| models agree in-limit | `qs_uniform_identity` | ≤1e-6 |
| fast path valid | `qs_vs_pi_far` | ≤1% |
| geometry signature | `baseline_differential`, `anchor_per_detector` | structural / ≤10% |

## 7. Traceability

M3-R1 → placement_roundtrip, build path · M3-R2 → signal_TD · M3-R3/R4 → qs_* · M3-R5 → baseline_differential, anchor_per_detector.
