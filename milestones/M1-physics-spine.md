# M1 — Physics spine (implementation brief)

> The walking skeleton: **one mass, one gradiometer, one validated number.** Read with
> `design/gravity.md`, `design/instrument.md`, and the spec (`eq:singlephi`, `eq:doublediff`,
> `tab:params`, `sec:oracle`).
>
> **Prereq:** M0. **Delivers to:** M2+ (everything runs through this spine).
> **Crates touched:** `gravity`, `instrument`, `source` (minimal), `state` (minimal),
> `scenario` (minimal), `generate` (minimal `run`).

---

## 1. Requirements

| ID | Requirement |
|---|---|
| M1-R1 | `gravity` evaluates `V`, `g`, `Γ` at a point from a `Cloud`, with `Γ` symmetric and trace-free in vacuum (spec `INV.1`). |
| M1-R2 | `instrument` builds one `Detector`'s four ballistic arms once, and `PropagationIntegral` produces `δφ` per interferometer and `ΔΦ` per gradiometer (spec `eq:singlephi`, `eq:doublediff`). |
| M1-R3 | `ΔΦ` is linear in source mass (spec `INV.2`): `ΔΦ(αm) = α·ΔΦ(m)`. |
| M1-R4 | A hand-built `Scenario` (one static/slow source, one detector, uniform schedule) runs via `generate.run` to a minimal `StateBundle` (`time`, `signal`, `source_position`, `mask`, `meta`). |
| M1-R5 | The **concrete-wall anchor** (~50 µrad) is reproduced. |

---

## 2. Constants (pinned; spec `tab:params`)

```
G        = 6.674 30e-11  m³ kg⁻¹ s⁻²        g      = 9.81 m s⁻²
m_A      = 1.46e-25 kg   (⁸⁷Sr, calibration) λ      = 698e-9 m      n = 1000 (LMT)
v_rec    = n·ħ·k/m_A ≈ 6.50 m s⁻¹  (k = 2π/λ; k_eff = m_A·v_rec/ħ ≈ 9.0e9 m⁻¹)
T        = 0.73 s   (2T = 1.46 s)            Δt     = 2 s (cadence)   fine_dt = 0.01 s
Δr       = 5 m      (gradiometer baseline in a 10 m tower)
```

---

## 3. Equations

### 3.1 The gravity kernel (per element, mass `m` at `x`; field point `p`; `d = p − x`, `r = |d|`, `d̂ = d/r`)

```
V(p)  = − G m / r
g(p)  = −∇V = − G m d̂ / r²                        (attractive: points from p towards the element)
Γ(p)  =  ∇g = − (G m / r³) · ( 𝟙 − 3 d̂ d̂ᵀ )      (symmetric; tr Γ = 0 exactly)
```

A cloud is the sum over elements. **Differential-first:** all evaluations of interest are
differences/​gradients of `V`, never large absolute potentials — the design rule that later keeps
f32 above the floor (M6).

### 3.2 The four ballistic arms (one gradiometer = two vertically stacked Mach–Zehnder IFOs, launched Δr apart, common lasers; pulses at τ = 0, T, 2T)

For an interferometer launched at height `z₀` with launch velocity `v₀` (chosen so the trajectory
fits the tower), the two arms over a cycle starting at `t₀` (τ = t − t₀ ∈ [0, 2T]):

```
ballistic(z, v, τ)   = z + v·τ − ½ g τ²
upper arm: z_u(τ) =  ballistic(z₀, v₀ + v_rec, τ)                          τ ∈ [0, T]
                     ballistic(z_u(T), ż_u(T) − v_rec, τ − T)              τ ∈ (T, 2T]
lower arm: z_l(τ) =  ballistic(z₀, v₀, τ)                                  τ ∈ [0, T]
                     ballistic(z_l(T), ż_l(T) + v_rec, τ − T)              τ ∈ (T, 2T]
```

The arms separate to `v_rec·T ≈ 4.7 m` at τ = T and re-close at 2T (closure is a unit test). Built
**once** per detector; the external source's field never perturbs the arms (perturbative regime).

### 3.3 Phase (the observable)

```
δφ_i(t)  = (m_A / ħ) ∫_{t−2T}^{t} [ V(z_u(τ), τ) − V(z_l(τ), τ) ] dτ        (eq:singlephi)
ΔΦ(t)    = δφ_2(t) − δφ_1(t)                                                 (eq:doublediff)
```

`V` here is the **external-source** potential only (the perturbation); Earth's uniform `g` is in the
arm trajectories, not the integrand. Linearity in `m` (M1-R3) is inherited directly: `V ∝ m`.

---

## 4. Design

### 4.1 Dataflow

```
Scenario{source, detector, schedule, seed}
   │ generate.run
   ▼
source.pose_at(t) ──► gravity: V(p, t) = Σ_elements −Gmᵢ/|p−xᵢ(t)|
   │                                   ▲
   ▼                                   │ arm sample points
instrument: arms (built once) ── PropagationIntegral: δφ₁, δφ₂ → ΔΦ(t)
   │
   ▼
StateBundle{ time (T,), signal (T,1), source_position (1,T,3), mask (T,), meta }
```

### 4.2 Types introduced

`gravity`: `Cloud` (SoA: `xs: Vec<f64> ×3`, `ms: Vec<f64>`), `Cloud::from_elements`,
`potential/field/gradient_tensor<S: Scalar>`.
`instrument`: `Detector { base_z, launch: [Launch; 2] }`, `Arms` (piecewise ballistic, §3.2),
`PropagationIntegral` implementing `PhaseModel`.
`source`: `Prescribed(Box<dyn Fn(f64) -> Isometry3<f64>>)` implementing `SourceDynamics`
(static and constant-velocity closures only).
`state/scenario/generate`: the minimal structs to satisfy M1-R4.

---

## 5. Pseudocode

```
fn delta_phi(cloud_at, arms, t) -> f64:            # one IFO, one measurement
    acc = 0
    for τ in linspace(t−2T, t, step=fine_dt):      # 2T/fine_dt = 146 samples
        cloud = cloud_at(τ)                        # posed world cloud
        acc  += V(cloud, p=(0,0,z_u(τ−(t−2T)))) − V(cloud, p=(0,0,z_l(τ−(t−2T))))
    return (m_A/ħ) · acc · fine_dt                 # rectangle rule; midpoint acceptable

fn run(scenario) -> StateBundle:
    arms = build_arms(scenario.detector)           # once
    for (ℓ, t) in scenario.schedule.times:
        ΔΦ[ℓ] = δφ(ifo₂) − δφ(ifo₁)
        track[ℓ] = scenario.source.pose_at(t).translation
    assemble bundle (mask = all false; meta = resolved config + seed)

# ad-hoc wall (pre-`shape`, replaced in M2):
fn wall_cloud(w, h, d, density, pitch) -> Cloud:
    regular lattice of centres inside the cuboid; m = density·pitch³   # no renormalise yet
```

Cost sanity: `N × 146 samples × 2 arms × 2 IFOs` kernel evaluations per measurement — trivially
CPU-fine at M1's `N ≲ 10⁴`.

---

## 6. Tests

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `gamma_symmetric_tracefree` | random clouds/points: `Γ = Γᵀ`, `tr Γ = 0` | ≤1e-12 |
| unit | `falloff` | point mass: `V ∝ 1/r`, `|g| ∝ 1/r²`, `‖Γ‖ ∝ 1/r³` over 3 decades | ≤1e-10 rel |
| unit | `mass_linearity` | `ΔΦ(αm) = α ΔΦ(m)`, α ∈ {0.1, 2, 10} | ≤1e-12 rel |
| unit | `arms_close` | `z_u(2T) = z_l(2T)` and both IFOs stay within the 10 m tower | ≤1e-12 m |
| unit | `arm_separation` | max separation `= v_rec·T` at τ = T | ≤1e-12 |
| integration | `spine_runs` | source→gravity→instrument yields finite, non-zero `ΔΦ`; sign flips when the source moves from above to below | structural |
| integration | `bundle_shapes` | `run` fills `time/signal/source_position/mask/meta` with the declared shapes | exact |
| integration | `quadrature_converges` | halving `fine_dt` changes `ΔΦ` ≤1e-6 rel (integrator resolution ≫ signal) | ≤1e-6 |
| e2e | `anchor_wall` | the concrete wall (spec geometry) gives `ΔΦ` ≈ 50 µrad: within 10% of the reference implementation's value; order-of-magnitude vs the published figure | ≤10% |

---

## 7. Exit requirements

| Requirement | Check | Tol |
|---|---|---|
| kernel invariants | `INV.1` unit rows | ≤1e-12 |
| spine | a hand-built `Scenario` runs to a bundle | total |
| linearity | `INV.2` | ≤1e-12 |
| anchor | `anchor_wall` | ≤10% of reference |

## 8. Traceability

M1-R1 → gamma_*, falloff · M1-R2 → arms_*, spine_runs, quadrature_converges · M1-R3 → mass_linearity · M1-R4 → bundle_shapes · M1-R5 → anchor_wall.
