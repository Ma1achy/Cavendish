# M12 — The physics layer (implementation brief)

> **Status: implemented on the CPU** on `v2-world-physics` (commits `23dd8dd`, `21b2cee`, `922b8b3`);
> written up after the fact from the code. Design: `design/physics.md`. Depends on: M11 (the seam),
> M2 (the voxeliser), M4 (the rotation integrator), M5 (the field socket).
>
> **Goal.** Scenes with mass that is not a single prescribed rigid body: articulated and deforming
> bodies, bodies that fall and collide, vibrating structure, and air that convects around all of it —
> each entering the forward model through the M11 seam, each checked against its own physics.

---

## 1. Requirements

| ID | Requirement |
|---|---|
| M12-R1 | A cell kernel — the closed-form potential and gradient tensor of a homogeneous cube — finite everywhere including inside the cell, continuous with the point law at the switch radius. |
| M12-R2 | Cloud-in-cell deposition that is a partition of unity (total conserved for any positions) and exactly the identity at cell centres. |
| M12-R3 | `GridField`: frames of cell masses as a `FieldContribution`, plus a linear-response evaluation of the propagation integral equal to the reference quadrature to rounding. Out-of-span queries fail loudly. |
| M12-R4 | An incompressible Boussinesq solver on a MAC grid whose projection is exact to its tolerance, which holds hydrostatic balance, and which reproduces a published natural-convection benchmark. |
| M12-R5 | The gravity read uses density from the **equation of state**, never the solver's working density; a static temperature anomaly gravitates. |
| M12-R6 | Bodies as transported parcels — rigid, skinned, displaced — with mass conserved bit-for-bit through any pose, and the rigid case equal to a v1 rigid source. |
| M12-R7 | Rigid-body dynamics: ballistic flight exact, `|L|` conserved, impulse contact with restitution and friction, and **exact rest** (a settled body's pose is bit-constant). Unsupported contact pairs are refused, not ignored. |
| M12-R8 | One-way coupling: any `WorldSource` stirs and heats the air; the recorded mass anomaly is an ambient source. Stirring isothermal air produces no density anomaly. |
| M12-R9 | The whole chain — dynamics → recorded track → air → forward model → bundle — runs end to end with each part on the right channel. |

---

## 2. Equations

**Cube kernel** (per unit mass, corners `(x, y, z) = (±h/2 − d)`, alternating sign):

```
V(d)   = −(G/h³) ||| xy·ln(z+r) + yz·ln(x+r) + zx·ln(y+r) − ½x²·atan(yz/xr) − ½y²·atan(zx/yr) − ½z²·atan(xy/zr) |||
Γ_aa   = −(G/h³) ||| atan(bc/(a r)) |||         Γ_ab = (G/h³) ||| ln(c + r) |||
tr Γ   = 0 outside the cube,  −4πG/h³ inside (Poisson)
```

**Grid response.** With flight nodes `(τ_n, w_n)` and masses linear in time between frames,

```
δφ(t) = (m_A/ħ) Σ_n w_n Σ_i m_i(t − 2T + τ_n) [K(p_u(τ_n) − x_i) − K(p_l(τ_n) − x_i)]  =  Σ_j Σ_i W[j][i]·m[k₀+j][i]
```

**Fluid.**

```
∂u/∂t + (u·∇)u = −∇p/ρ₀ + ν∇²u + gβ(T − T₀)ẑ ,   ∂T/∂t + (u·∇)T = α∇²T + S ,   ∇·u = 0
ρ(T) = p₀/(R_s T) ,   δm_i = (ρ(T_i) − ρ(T₀))·h³                      (the gravity read)
film:  ∂T/∂t = (h_c/ρc_p)(T_s − T)·a ,   a = |∇φ|  for an immersed body of volume fraction φ
```

**Parcels.** `x_j(t) = Σ_b w_jb T_b(t) x_j^rest`, `Σ_b w_jb = 1`; masses are the rest masses.

**Plate.** `ω_mn = π²[(m/a)² + (n/b)²]·√(D/ρh)`, `D = Eh³/12(1 − ν²)`.

**Contact impulse** along direction `d` at arms `r_a`, `r_b`:
`j = (v_target − v_rel·d) / k`, `k = Σ_bodies [1/m + d·((I⁻¹(r × d)) × r)]`; normal impulse accumulated
and clamped ≥ 0, friction bounded by `μ` times it.

---

## 3. Design

```
shape::voxelise ─► rest cloud ─┬─► Parcels (rigid | skinned | displaced) ───────────────┐
                               └─► RigidBody ─► RigidWorld::record ─► RecordedRigid ─────┤  WorldSource
                                                                                         ├──────────────► Scenario
        obstacles (any WorldSource) ─► simulate_air ─► Fluid ─► mass_anomaly ─► GridField ┘
```

Staging is strictly one-way: dynamics first (they do not depend on the air), then the air around the
recorded motion, then everything into one `Scenario`. The accuracy standard is §1 of the design
drill-down: these layers decide where the mass is; the forward model, downstream and untouched,
decides what it reads.

---

## 4. Tests

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `cube_matches_quadrature_near` | closed form vs 60³ quadrature, on and off symmetry planes | ≤2e-5 |
| unit | `cube_matches_point_far` | continuity at the switch; error falls ≥8× per doubling of distance | ≤1e-5 |
| unit | `cube_finite_inside_and_on_boundary` | finite at centre, face, edge, corner; centre value `2.3800772·(−G/h)` | ≤1e-6 |
| unit | `gradient_tensor_matches_finite_differences` | `Γ = −∇∇V`; trace-free outside | ≤1e-4 / ≤1e-10 |
| unit | `gradient_trace_is_poisson_inside` | `tr Γ = −4πG/h³` | ≤1e-10 |
| unit | `splat_is_a_partition_of_unity` | deposited total = input total, nothing lost | ≤1e-14 |
| unit | `splat_at_cell_centres_is_the_identity` | a lattice's own points deposit to themselves | **exact** |
| unit | `splat_reports_what_leaves_the_grid` | lost + deposited = input | ≤1e-14 |
| unit | `fast_path_equals_reference` | response dot product vs `PropagationIntegral::per_ifo`, upright and tilted, on and off the frame grid | ≤1e-9 |
| unit | `uniform_schedule_reuses_one_response` | ten aligned measurements build one response | structural |
| unit | `static_field_is_time_independent` | identical frames ⇒ identical phase | ≤1e-12 |
| unit | `query_outside_the_span_is_loud` | panics | — |
| unit | `projection_removes_divergence` | random field around a solid block | ≤1e-8 of before |
| unit | `stratified_fluid_at_rest_stays_at_rest` | stable stratification, 20 steps | ≤1e-9 m/s |
| unit | `equation_of_state_reads_temperature` | warm cell lighter by the ideal-gas amount; ρ(20 °C) ≈ 1.204 | ≤1e-15 kg |
| unit | `rigid_parcels_equal_the_rigid_source` | parcel phase vs v1 rigid source, tumbling | ≤1e-11 |
| unit | `one_bone_skeleton_is_rigid` | skinned with one bone = the rigid transform | ≤1e-13 m |
| unit | `mass_is_conserved_exactly_through_any_pose` | posed masses = rest masses | **exact** |
| unit | `skinning_weights_are_a_partition_of_unity` | rows sum to one; far parcels belong to their bone | ≤1e-15 |
| unit | `hinge_moves_the_far_parcels_rigidly` | 90° hinge moves the distal limb as the bone | ≤1e-6 m |
| unit | `walker_walks_and_swings` | CoM advances at the walking speed; limbs extend; track reports velocity | structural |
| unit | `plate_frequency_and_nodes` | `ω₁₁` closed form; node line stationary; mass = ρV; ambient | ≤1e-9 |
| unit | `displaced_air_reduces_the_gravitating_mass` | `m(1 − ρ_air/ρ_body)` | ≤1e-12 |
| unit | `free_fall_is_ballistic` | 2 s of flight vs the parabola | ≤1e-10 m |
| unit | `free_rotation_conserves_angular_momentum_and_energy` | 20 s tumbling about the unstable axis | ≤1e-10 / ≤1e-5 |
| unit | `bounce_height_follows_restitution` | rebound `e²·h` for `e = 0.6` | 1 cm |
| unit | `equal_spheres_exchange_velocity_and_conserve_momentum` | elastic head-on | ≤1e-9 |
| unit | `sphere_rests_on_a_box` | sphere–box contact holds | 5 mm |
| unit | `dropped_box_settles_and_sleeps_exactly` | pose constant after sleep; rests on a face | **bit-constant** / 2 mm |
| unit | `box_box_contact_is_refused` | panics | — |
| unit | `recording_reproduces_ballistic_flight_between_samples` | Hermite replay between coarse frames | ≤1e-10 m |
| integration | `heated_cavity_matches_de_vahl_davis` | Nu at Ra = 10³, 10⁴ (1.118, 2.243), 32², hot = cold | within 2% / 1% |
| integration | `warm_body_sheds_a_plume_that_gravitates` | rising air; anomaly negative and growing; phase non-zero and changing | structural |
| integration | `static_thermal_field_has_a_gravity_signal` | no flow, warm cell ⇒ non-zero potential | sign |
| integration | `moving_body_stirs_isothermal_air` | air moves; no density anomaly | ≤1e-12 of cell air mass |
| integration | `immersed_surface_area_is_the_true_area` | `∫|∇φ|dV` vs `4πr²` at two resolutions | 10% / 35% |
| e2e | `generate/tests/world::rigid_parcels_reproduce_the_rigid_source` | parcels through `generate::run` vs v1 | ≤1e-11 |
| e2e | `generate/tests/world::mixed_scene_decomposes_exactly` | grid fast path vs slow quadrature inside a full scene | ≤1e-6 |
| e2e | `generate/tests/world::simulated_scene_runs_end_to_end` | dropped warm ball: rests at `z = r`; air on the ambient channel, growing; ball dominates | structural / 5 mm |

---

## 5. Exit requirements

| Requirement | Check | Tol |
|---|---|---|
| kernel (R1) | the five `cube_*`/`gradient_*` tests | as §4 |
| deposition (R2) | the three `splat_*` tests | exact / ≤1e-14 |
| grid field (R3) | `fast_path_equals_reference`, `query_outside_the_span_is_loud` | ≤1e-9 |
| fluid (R4) | `projection_removes_divergence`, `stratified_…`, `heated_cavity_matches_de_vahl_davis` | Nu within 2% |
| Boussinesq guard (R5) | `equation_of_state_reads_temperature`, `static_thermal_field_has_a_gravity_signal` | — |
| parcels (R6) | `rigid_parcels_equal_the_rigid_source`, `mass_is_conserved_exactly_through_any_pose` | ≤1e-11 / exact |
| dynamics (R7) | `free_fall_is_ballistic`, `free_rotation_…`, `dropped_box_settles_and_sleeps_exactly`, `box_box_contact_is_refused` | as §4 |
| coupling (R8) | `warm_body_sheds_a_plume_that_gravitates`, `moving_body_stirs_isothermal_air` | — |
| whole chain (R9) | `simulated_scene_runs_end_to_end` | — |
| M11 still holds | `bundle_hash` identical to `main` | byte-identical |

Measured benchmark values at the time of writing: Nu = 1.1205 (Ra = 10³) and 2.283 (Ra = 10⁴).

## 6. Traceability

R1 → `kernel` tests · R2 → `splat_*` · R3 → `fast_path_equals_reference`,
`uniform_schedule_reuses_one_response`, `static_field_is_time_independent`,
`query_outside_the_span_is_loud` · R4 → `fluid` tests · R5 → `equation_of_state_reads_temperature`,
`static_thermal_field_has_a_gravity_signal` · R6 → `body` tests · R7 → `rigid` tests · R8 → `scene`
tests · R9 → `generate/tests/world.rs`.

## 7. Not in this milestone

GPU execution of anything here; fluid validation beyond Ra = 10⁴, in 3-D or at room scale; open
boundaries; box–box contact, joints, two-way coupling; Laplace-diffused skinning weights and skinned
mesh import; octrees; room acoustics. See `design/physics.md` §10 and `milestones/V2-HANDOFF.md`.
