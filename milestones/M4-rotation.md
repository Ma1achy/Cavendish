# M4 — Rotation (implementation brief)

> Tumbling bodies: the torque-free Euler top and the pendulum, on a hand-written structure-preserving
> integrator generic over `Scalar` — **not** a physics engine. Rotation is the pure-quadrupole probe.
> Read with `design/source.md` §4–5, `rigid-body-rotation.md`, and the spec (§5.4, `INV.5`, `T4.4`).
>
> **Prereq:** M2 (clouds, moments). **Delivers to:** M5+ (rotating sources everywhere), M8 (the
> integrator is on the `Dual` path). **Crates touched:** `source` (integrator + motions), `gravity`
> (reduction wiring), `state`/`generate` (angular + shape fields).

---

## 1. Requirements

| ID | Requirement |
|---|---|
| M4-R1 | A hand-written symplectic/structure-preserving integrator `step<S: Scalar>` (≈200 lines), `fine_dt` substeps, no external physics engine, autodiff-compatible. |
| M4-R2 | Two ODE orientation motions: `Libration` (the pendulum) and `FreeRotation` (the torque-free Euler top). |
| M4-R3 | Conservation: quaternion norm exact (renormalised); energy and |L| bounded (no secular drift) over long runs. |
| M4-R4 | Bundle rotation fields filled: `source_orientation`, `source_angular_velocity` (direction = spin axis, magnitude = rate), `source_angular_accel`; consistent with `d(orientation)/dt`. |
| M4-R5 | Shape descriptors filled from the one second moment: `source_mass/inertia/moments/axes/quadrupole` (`FieldSet.shape`). |
| M4-R6 | The intermediate-axis (Dzhanibekov) flip appears for an asymmetric top spun near its middle axis — integrable separatrix behaviour, not chaos. |
| M4-R7 | A body rotating about its CoM produces a **pure-quadrupole** signal: monopole term constant, dipole identically zero. |

---

## 2. Equations

### 2.1 Torque-free Euler top (body frame; principal moments I₁, I₂, I₃)

```
I₁ ω̇₁ = (I₂ − I₃) ω₂ ω₃
I₂ ω̇₂ = (I₃ − I₁) ω₃ ω₁            (Euler's equations, torque N = 0)
I₃ ω̇₃ = (I₁ − I₂) ω₁ ω₂

invariants:   E = ½ ωᵀ I ω          L² = |I ω|²        (both exactly conserved by the flow)
separatrix:   spin near the intermediate axis (I₁ < I₂ < I₃, ω ≈ ω₂ ê₂) is unstable —
              trajectories hug the separatrix and periodically flip (Dzhanibekov). Integrable
              (Jacobi elliptic solutions exist); the *pendulum* is the genuinely chaotic one when driven.
```

### 2.2 Quaternion kinematics and the exponential-map update

```
q̇ = ½ q ⊗ (0, ω_body)                        (wxyz; q maps body → world)
exact rotation about a single axis ê by angle φ (the building block of the §4 splitting):
   q ← q ⊗ ( cos(φ/2),  ê sin(φ/2) )              then renormalise |q| = 1
```

### 2.3 Libration (physical pendulum about a fixed horizontal pivot axis)

```
θ̈ = −(M g d / I_pivot) · sin θ         d = pivot→CoM distance; I_pivot = I_axis + M d² (parallel axis)
E = ½ I_pivot θ̇² − M g d cos θ          (conserved; leapfrog preserves it to O(h²), bounded)
```

### 2.4 The one second moment (already in `gravity`; wired to the bundle here)

```
C = Σ mᵢ xᵢ xᵢᵀ  (about CoM, body frame)      I = tr(C)𝟙 − C       Q = 3C − tr(C)𝟙
eig(C) → principal axes (shared by I and Q);  moments (I₁,I₂,I₃) = eigvals of I
multipoles under rotation about the CoM:  monopole M fixed;  dipole ≡ 0 (recentred cloud);
   the time-varying field is carried by Q rotating with q(t) — the pure-quadrupole probe.
```

---

## 3. Design

```
Trajectory { path, timing, orient: Libration{axis, θ₀, θ̇₀} | FreeRotation{ω₀} | … , placement }
                                  │  per measurement tick t_ℓ
                                  ▼
   substep loop (fine_dt): step(q, ω, I, h)  — momentum-splitting (§4); renormalise q
                                  │
                                  ▼
   pose_at(t_ℓ) = (translation from path, q)          motion_at → (ω, ω̇) for the bundle
```

The integrator is **generic over `Scalar`** so M8 can push `Dual` through initial conditions.
State between measurement ticks is carried (the ODE motions are stateful within a run; poses are
still a pure function of `(Scenario, t)` because the substep grid is fixed by `fine_dt`).

---

## 4. Pseudocode

```
fn step<S: Scalar>(q: &mut Quat<S>, ω: &mut Vec3<S>, I: Vec3<S>, h: S):
    # McLachlan/Reich free-rigid-body Strang splitting: exact rotations of the body angular
    # momentum Π = I∘ω about each principal axis, composed 1(½),2(½),3(1),2(½),1(½).
    Π = I ∘ ω                                          # body angular momentum
    for (k, τ) in [(0,h/2),(1,h/2),(2,h),(1,h/2),(0,h/2)]:
        φ = (Π[k] / I[k]) · τ                          # exact sub-rotation angle (ωₖ = Πₖ/Iₖ)
        rotate Π about êₖ by φ                          # Πₖ fixed, the other two rotate — |Π| exact
        q = q ⊗ Quat(cos(φ/2), êₖ·sin(φ/2))            # exponential-map reconstruction
    q = q / |q|                                        # renormalise
    ω = Π ∘ (1/I)

fn libration_step<S>(θ, θ̇, h):          # leapfrog (the pendulum is separable — this sketch is correct)
    θ̇ += h/2 · (−k·sin θ);  θ += h·θ̇;  θ̇ += h/2 · (−k·sin θ)
```

> **Why the splitting, not an explicit half-kick.** The free rigid body is **non-separable**: an
> explicit half-kick/exact-map/half-kick is a midpoint-class scheme that does *not* conserve the
> quadratic invariants `E`, `|L|`, so it drifts secularly — contradicting `free_top_invariants`.
> Splitting the kinetic energy into per-axis terms makes each sub-flow an *exact* rotation of `Π`,
> so `|L|` is conserved to machine precision by construction and `E` stays bounded forever. (The
> pendulum is separable, so its leapfrog is correct as written.) This is the integrator `compute`
> re-expresses in WGSL (M6) and `analysis` differentiates through (M8), so the doc describes it exactly.

---

## 5. Tests

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `quat_norm_preserved` | |q| = 1 after 10⁶ substeps | ≤1e-12 |
| unit | `free_top_invariants` | E and |L| bounded, **no secular drift**, over 10⁴ tumble periods at h = period/100 | rel ≤1e-6, oscillatory |
| unit | `pendulum_energy` | libration E bounded over 10⁴ periods; small-angle limit period = 2π√(I_p/Mgd) | ≤1e-6 / ≤1e-4 |
| unit | `axisymmetric_analytic` | symmetric top (I₁=I₂): ω precesses at the closed-form rate (Ω = (I₃−I₁)/I₁·ω₃) | ≤1e-8 |
| unit | `moment_identities` | `I = trC𝟙−C`, `Q = 3C−trC𝟙`; shared eigvectors; analytic values for rod/cuboid | ≤1e-10 |
| unit | `scalar_generic` | `step::<f64>` and `step::<Dual>`'s value channel agree exactly | exact |
| integration | `omega_consistency` | `source_angular_velocity` matches finite-difference of `source_orientation` (quaternion log) | ≤1e-6 |
| integration | `shape_fields` | `FieldSet.shape` fills mass/inertia/moments/axes/quadrupole from the cloud; off ⇒ `None` | exact/structural |
| e2e | `dzhanibekov` | asymmetric cuboid (I₁<I₂<I₃), ω₀ = ω₂ê₂ + 1e-3 perturbation: ω₂ changes sign periodically; the flip period is stable across `fine_dt` halving | structural + ≤1% |
| e2e | `pure_quadrupole` | a rotating (CoM-fixed) cuboid's `ΔΦ(t)`: mean equals the static-monopole value; the varying part vanishes for a sphere (Q=0) and scales with ‖Q‖ across aspect ratios | ≤1e-3 / structural |

---

## 6. Exit requirements

| Requirement | Check | Tol |
|---|---|---|
| integrator invariants | quat_norm, free_top_invariants, pendulum_energy | bounded, ≤1e-6 |
| analytic cross-checks | axisymmetric_analytic, moment_identities | ≤1e-8/1e-10 |
| rotation first-class | omega_consistency, shape_fields | ≤1e-6 / exact |
| the flip | dzhanibekov | structural |
| quadrupole probe | pure_quadrupole (spec `INV.5`, `T4.4`) | ≤1e-3 |

## 7. Traceability

M4-R1 → scalar_generic, invariant rows · M4-R2 → pendulum/free-top rows · M4-R3 → quat_norm, free_top_invariants, pendulum_energy · M4-R4 → omega_consistency · M4-R5 → shape_fields · M4-R6 → dzhanibekov · M4-R7 → pure_quadrupole.
