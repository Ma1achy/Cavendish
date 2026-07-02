# M2 — Source motion & shape core (implementation brief)

> Real trajectories, and geometry→mass done properly: the motion anchors run on properly-voxelised
> clouds, replacing M1's ad-hoc lattice. Read with `design/source.md` §3–6 and `design/shape.md`.
>
> **Prereq:** M1. **Delivers to:** M3+ (all bodies), M10 (the mesh path plugs into this voxeliser).
> **Crates touched:** `source` (motion library), `shape` (new: core), `generate` (kinematics fill).

---

## 1. Requirements

| ID | Requirement |
|---|---|
| M2-R1 | The closed-form motion library exists as the `Path × Timing × Orient + Placement` factoring: linear, oscillation, orbit/flyby paths; constant/eased timing; analytic velocity and acceleration. |
| M2-R2 | The `shape` core exists: the `Solid` seam, one lattice voxeliser, primitives (sphere, shell, cuboid, cylinder) with analytic moment oracles, `Union`, and the unit-mass cloud cache. |
| M2-R3 | Every produced cloud has exact total mass (renormalised), CoM at the body-frame origin, and deterministic element order (`design/shape.md` §4). |
| M2-R4 | Bundle kinematics filled: `source_velocity`, `source_accel` per tick, consistent with the trajectory. |
| M2-R5 | Anchors: moving mass (10 kg, D = 10 m, 2.5 m s⁻¹ → ≈ 7 mrad) and oscillation (1 kg, D = 5 m → ≈ 2 mrad). |
| M2-R6 | The **shell theorem** holds numerically: a voxelised sphere's external field matches a point mass. |

---

## 2. Equations

### 2.1 Paths (position, with analytic derivatives)

```
linear:       r(t) = r₀ + v·t                        ṙ = v            r̈ = 0
oscillation:  r(t) = r₀ + A sin(2πf t + φ₀)·ê        ṙ = 2πfA cos(·)ê r̈ = −(2πf)² (r−r₀)
flyby:        r(t) = r_closest + v·(t − t_ca)        (a linear path parameterised by closest approach)
timing:       s(t) — monotone reparameterisation (constant: s=t; eased: C¹ ramps); r∘s, with
              d(r∘s)/dt = ṙ(s)·ṡ etc.
```

### 2.2 Voxeliser post-conditions (the two exact operations)

```
renormalise:  mᵢ ← mᵢ · M / Σⱼ mⱼ            ⇒ Σ mᵢ = M   exactly (monopole error = 0)
recentre:     xᵢ ← xᵢ − (Σ mⱼxⱼ)/M           ⇒ Σ mᵢxᵢ = 0 exactly (dipole = 0 about origin)
```

### 2.3 Analytic second moments (uniform density, mass M, about the CoM) — the moment oracle

```
solid sphere R:      C = (M R²/5)·𝟙          (I = 2MR²/5·𝟙,  Q = 0)
thin shell R:        C = (M R²/3)·𝟙          (I = 2MR²/3·𝟙,  Q = 0)
cuboid a×b×c:        C = diag(Ma², Mb², Mc²)/12
cylinder R, L ‖ ẑ:   C = diag(MR²/4, MR²/4, ML²/12)
identities:          I = tr(C)𝟙 − C          Q = 3C − tr(C)𝟙        (shared principal axes)
union (disjoint):    C = Σ parts (about the union's CoM, parallel-axis shifted)
```

---

## 3. Design

### 3.1 The shape pipeline (one path for everything)

```
Sphere/Shell/Cuboid/Cylinder ──┐
Union(Vec<Solid>)  ────────────┤   occupancy(p)∈[0,1]        lattice pitch h
(M10: MeshSolid) ──────────────┴─►  Solid  ──►  voxelise ──► renormalise ──► recentre ──► Cloud
                                              (raster order,      (M exact)      (CoM = 0)   (canonical
                                               boundary k³                                     order)
                                               sub-samples)
                                                        └──► registry cache: (geom hash, h, k) → Arc<Cloud> @ unit mass
```

`source` resolves a dictionary entry to a cached unit-mass cloud and applies the scenario mass as a
per-element multiply (linearity, `INV.2`) — a mass draw never re-voxelises.

### 3.2 Types

`shape`: `Solid` trait (`occupancy`, `bbox`), the four primitives + `Union`, `VoxelParams { pitch |
target_n, supersample k (default 2) }`, `MassSpec { Density | Total }`, `voxelise(...) ->
Result<Cloud, ShapeError>`, `analytic_moments()` per primitive, the registry.
`source`: `Path`, `Timing`, `Orient` (identity/fixed for M2), `Placement`; `Trajectory` composing
them into `SourceDynamics`; `motion_at(t) -> (pose, vel, acc)`.

---

## 4. Pseudocode

```
fn voxelise(solid, vox, mass) -> Cloud:
    h    = vox.pitch or refine_from(target_n, solid.bbox())   # one refinement pass: N within ×2
    elems = []
    for cell in raster(solid.bbox(), h):                      # fixed x-fastest order (canonical)
        occ_c = solid.occupancy(cell.centre)
        if boundary(cell):                                    # any corner disagrees with centre
            occ = mean(solid.occupancy(p) for p in sublattice(cell, k))   # k³ fixed offsets, seed-free
        else:
            occ = occ_c                                       # 0 or 1
        if occ > 0: elems.push((cell.centre, occ·h³))         # volume-weighted
    if elems.empty: return Err(EmptySolid)
    if len(elems) > CAP: return Err(TooManyElements)
    ρ = mass.density or mass.total / Σ occ·h³
    m = [ρ·vol for vol in vols];  m *= M / Σm                 # renormalise (exact)
    x -= Σ mᵢxᵢ / M                                           # recentre   (exact)
    Cloud::from_elements(x, m)
```

---

## 5. Tests

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `path_consistency` | central finite-difference of `r(t)` matches analytic `ṙ`, `r̈` for every path×timing | ≤1e-8 rel |
| unit | `oscillation_params` | period `1/f`, amplitude `A`, axis `ê` recovered from samples | ≤1e-10 |
| unit | `moments_table` | voxelised primitive `C` → analytic table values (h = R/20, k = 2) | ≤2% |
| unit | `mass_com_exact` | every voxelised cloud: `Σm = M`, `|CoM| = 0` | ≤1e-12 (abs, scaled) |
| unit | `deterministic_cloud` | two runs, same params → bit-identical elements incl. order | exact |
| unit | `union_no_doublecount` | two overlapping spheres: voxelised mass < sum of parts; = analytic union volume·ρ | ≤2% |
| integration | `convergence_halving` | halving `h` halves the `C` error (sphere, cuboid); `k`: 1→4 shrinks the constant | ratio 0.5 ±20% |
| integration | `kinematics_filled` | `source_velocity/accel` in the bundle match `motion_at` | exact |
| integration | `cache_hits` | second resolution of a dictionary entry returns the same `Arc`; mass draw rescales only | structural |
| e2e | `shell_theorem` | voxelised sphere (h = R/20): external `g` vs point mass at d ∈ [2R, 10R] | ≤1e-3 rel |
| e2e | `anchor_moving` | 10 kg, D = 10 m, 2.5 m s⁻¹ → ≈ 7 mrad | ≤10% of reference |
| e2e | `anchor_oscillation` | 1 kg, D = 5 m, oscillating → ≈ 2 mrad; spectral line at the drive f (checked in M6 with LS; here: peak-to-peak) | ≤10% of reference |

---

## 6. Exit requirements

| Requirement | Check | Tol |
|---|---|---|
| motion anchors | `anchor_moving`, `anchor_oscillation` on voxelised clouds | ≤10% |
| shell theorem | `shell_theorem` | ≤1e-3 |
| exact post-conditions | `mass_com_exact`, `deterministic_cloud` | exact |
| convergence | `convergence_halving` | ratio |
| kinematics | `kinematics_filled` | exact |

## 7. Traceability

M2-R1 → path_*, oscillation_params · M2-R2 → moments_table, union_*, cache_hits · M2-R3 → mass_com_exact, deterministic_cloud · M2-R4 → kinematics_filled · M2-R5 → anchor_* · M2-R6 → shell_theorem.
