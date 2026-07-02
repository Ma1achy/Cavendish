# M8 — Analysis: Fisher & CRB (implementation brief)

> Identifiability from the differentiable forward model: the Jacobian via the `Dual` path, the
> Fisher information, the Cramér–Rao floor, and array-geometry scoring. Read with
> `design/state.md` §7 (the placement decision) and the spec (§Gradar, the CRB/array story).
>
> **Prereq:** M6 (the `Dual`-capable CPU path through the full forward model). Independent of
> M7/M9. **Crates touched:** `analysis` (new).

---

## 1. Requirements

| ID | Requirement |
|---|---|
| M8-R1 | `J = ∂signal/∂θ` assembled by forward-mode `Dual` sweeps through the *whole* forward model (`gravity → source → instrument` via `CpuBackend`), for a declared parameter set θ (source position, velocity, mass; ω₀ for rotating sources). |
| M8-R2 | Fisher `F = Jᵀ Σ⁻¹ J` and `CRB = F⁻¹`, with `Σ` from the configured noise (white `σ²𝟙` in v1). |
| M8-R3 | Validation against an analytic Fisher case. |
| M8-R4 | Degeneracy is *reported*, not hidden: conditioning/near-singularity of `F` surfaces (the mass–distance degeneracy must show up as it should). |
| M8-R5 | Array-geometry scoring: `CRB(placement)` evaluable over a placement sweep; localisation precision improves with baseline as the geometry predicts. |
| M8-R6 | `analysis` stays out of `state` (dependency check): `analysis → gravity, source, instrument, compute, state`. |

---

## 2. Equations

```
J ∈ ℝ^{(T·D) × P}         J[:, j] = ∂ signal / ∂ θⱼ      (one Dual sweep per parameter: seed dⱼ = 1)
F = Jᵀ Σ⁻¹ J              white noise: F = JᵀJ / σ²
CRB = F⁻¹                 diag(CRB)ⱼ = the variance floor of any unbiased estimator of θⱼ

analytic check — amplitude of a known template s(t) = A·f(t), white σ:
   F_AA = Σᵢ f(tᵢ)² / σ²         CRB_AA = σ² / Σ f²        (mass is exactly this: signal ∝ m)

degeneracy: for θ = (m, R) of a far monopole, columns of J are nearly parallel
   (∂s/∂m ∝ s/m, ∂s/∂R ≈ −k·s/R for power-law falloff) ⇒ cond(F) large — expected and reported.

geometry: for a source at range R and array baseline b, transverse localisation scales like
   σ_x ∝ (R/b)·σ_range-ish — the *test* is monotone improvement of diag(CRB)_position with b,
   not an asserted closed form.
```

---

## 3. Design

```
θ (P params) ──► for j in 0..P: seed Dual tangent on θⱼ ──► CpuBackend::<Dual> forward ──► column j of J
                                                              (S,T,D signal in Dual: v = signal, d = ∂/∂θⱼ)
J ──► F = JᵀΣ⁻¹J ──► chol/eig ──► CRB, cond(F), resolvability report
                         │
                         └──► sweep placements ──► CRB(b) curve (the array-design knob)
```

Cost: `P` forward passes (P ≲ 10), CPU, small scenarios — not throughput-bound, which is why the
`Dual` path is CPU-only by design (`design/compute.md` §8). Values channel of the `Dual` run must
equal the plain `f64` run exactly (a free cross-check).

---

## 4. Pseudocode

```
fn jacobian(scn, θ_spec) -> DMatrix:
    for (j, θ) in θ_spec:
        scn_d = lift_to_dual(scn, seed=j)            # one tangent hot
        sig   = cpu_backend.eval::<Dual>(scn_d)
        assert sig.map(v) == baseline_f64            # value-channel identity
        J[:, j] = sig.map(d).flatten()               # (T·D,)

fn crb(J, σ) -> (DMatrix, f64):
    F = J.t() * J / σ²
    (F.cholesky_or_eig_inverse(), F.condition_number())
```

---

## 5. Tests

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `dual_vs_finite_diff` | every J column vs central finite differences of the f64 forward model | ≤1e-6 rel |
| unit | `value_channel_identity` | `Dual` value channel ≡ plain f64 run | exact |
| unit | `fisher_spd` | `F` symmetric; eigenvalues ≥ 0; Cholesky succeeds when well-conditioned | ≤1e-10 / structural |
| integration | `analytic_amplitude` | mass-only θ: `CRB_mm = σ²/Σ(∂s/∂m)²` matches the closed form | ≤1e-8 |
| integration | `degeneracy_reported` | θ = (m, R) far monopole: `cond(F)` exceeds threshold and the report flags the pair; adding a near pass breaks the degeneracy (cond drops) | structural |
| integration | `dep_hygiene` | `state` does not depend on the forward crates; `analysis` compiles against the declared edge set only | compiles |
| e2e | `crb_vs_baseline` | placement sweep b ∈ [10, 200] m at fixed R: diag(CRB) of transverse position strictly decreases with b | structural |
| e2e | `crb_regression` | a pinned scenario's full CRB matrix vs a stored reference | ≤1e-6 |

---

## 6. Exit requirements

| Requirement | Check | Tol |
|---|---|---|
| J correct | `dual_vs_finite_diff`, `value_channel_identity` | ≤1e-6 / exact |
| CRB verified | `analytic_amplitude`, `fisher_spd` | ≤1e-8 |
| degeneracy honest | `degeneracy_reported` | structural |
| geometry scored | `crb_vs_baseline` | structural |
| placement clean | `dep_hygiene` | compiles |

## 7. Traceability

M8-R1 → dual_vs_finite_diff, value_channel_identity · M8-R2 → fisher_spd, crb_regression · M8-R3 → analytic_amplitude · M8-R4 → degeneracy_reported · M8-R5 → crb_vs_baseline · M8-R6 → dep_hygiene.
