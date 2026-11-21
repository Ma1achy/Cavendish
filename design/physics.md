# Cavendish — `physics` drill-down

> Subsystem design for the `physics` crate: the **scene simulator**. It produces mass configurations
> — bodies that move, fold, fall and collide; structure that rings; air that convects — and hands
> them to the forward model through the source seam (`design/world.md`). This is the spec's parked
> appendix (`sec:parked`) built: prescribed sources (articulated bodies, plate modes) and solved ones
> (rigid dynamics, convection), with one-way coupling between them. Companion to `design/world.md`
> (the seam), `design/shape.md` (the voxeliser it reuses), `design/source.md` §5 (the rotation
> integrator it reuses) and `design/noise.md` §6 (the analytic atmospheric field it sits beside).
>
> **Dependencies:** `physics → world, instrument, source, shape, gravity, math`. Nothing below L3
> depends on it; `generate` uses it only in tests and examples (a scene author depends on it directly).

---

## 1. The governing split — two accuracy standards

The simulator and the forward model answer different questions and are held to different standards:

- **`physics` decides where the mass is.** Its standard is *plausible, conservative, varied*: mass is
  conserved exactly, momentum and energy behave, the air obeys the equations it claims to. A bounce
  need not match a laboratory bounce.
- **The forward model decides what the gradiometer reads from that mass.** Its standard is
  *reference-exact*: George's anchors, the propagation integral, µrad on hundreds of radians.

The exactness burden sits entirely on the density → phase map, which this crate does not touch. That
is why hand-written game-style dynamics are the right tool here and not a compromise, and why the
crate is checked against analytic results and published benchmarks rather than against the reference
oracle (which has nothing to say about a plume).

One consequence is enforced throughout: **anything that would make the mass field jitter is a defect,
not a cosmetic issue** — a resting body that wobbles in a contact slop band, or a boundary voxel that
pops in and out, is a spurious high-frequency term in the differential phase.

---

## 2. Responsibility & boundaries

**Owns:** the cell kernel (§3); the world grid, cloud-in-cell deposition and the grid mass field
(§4); the fluid (§5); bodies as transported parcels (§6); rigid dynamics and contact (§7); the
coupled room (§8).

**Does not own:** the phase integral (`instrument`), the point-element kernel (`gravity`), rigid
closed-form motion (`source`), geometry → mass (`shape`), scene assembly (`scenario`).

**Module map.**

```
kernel   cube_potential, cube_gradient_tensor              (closed-form prism; point law far)
grid     Grid3::splat, GridField, flight_nodes             (lattice fields → FieldContribution + fast path)
fluid    Fluid, FluidParams, Heat, Air                     (Boussinesq on a MAC grid; equation of state)
body     Parcels, Skeleton/Bone/Pose, Walker, Plate        (transported mass parcels)
rigid    RigidWorld, RigidBody, Collider, RecordedRigid    (dynamics + contact → a rigid source)
scene    Room, Obstacle, simulate_air                      (one-way coupling: bodies stir and heat the air)
```

---

## 3. The cell kernel — a cube is not a point

A grid cell is a cube of matter. Its potential per unit mass at offset `d` from the centre is the
closed-form prism potential (Nagy 2000),

```
V(d) = −(G/h³) |||  xy·ln(z+r) + yz·ln(x+r) + zx·ln(y+r)
                   − ½x²·atan(yz/xr) − ½y²·atan(zx/yr) − ½z²·atan(xy/zr)  |||
```

evaluated over the eight corners `(x, y, z) = (±h/2 − d)` with alternating sign, and the gradient
tensor follows as `Γ_aa = −(G/h³)|||atan(bc/ar)|||`, `Γ_ab = (G/h³)|||ln(c + r)|||`.

**Why it is needed.** A cube has no dipole, quadrupole or octupole, so a point at its centre is wrong
only at the hexadecapole, `O((h/d)⁴)` — which is why v1's point-voxel bodies agree with the oracle at
range. But an air cell can sit against the atom path, or contain it, and there a point element is
singular. The prism form is finite everywhere, including inside the cube, where
`tr Γ = −4πG/h³` (Poisson) — the trace the `FieldContribution` contract already anticipates.

**Near/far switch.** Prism form within `NEAR = 8` cell sizes; point law beyond. The prism form loses
digits to cancellation far away (eight large terms leaving `h³/d`), and the point law is by then
accurate to better than `1e-5`. `ln(z + r)` is evaluated as `ln((x²+y²)/(r − z))` for `z < 0` to avoid
its own cancellation; degenerate corner terms (`x = 0`, `r = 0`) are their limits, zero.

**Scope.** The cube kernel is used for grid fields. Parcel bodies use `gravity`'s point elements:
they are metres from the detector and the hexadecapole argument applies.

---

## 4. The world grid and the grid mass field

### 4.1 Cloud-in-cell deposition

`Grid3::splat` deposits point quantities onto cell centres with trilinear weights. Each point's eight
weights sum to one — a **partition of unity** — so the deposited total equals the input total
*whatever the positions are*: conservation does not depend on the quality of the map that moved the
points. A point on a cell centre deposits wholly into that cell, so depositing a lattice's own points
is the identity; a lattice coordinate within `1e-9` of an integer is snapped to it so that the
rounding in `origin + (i+½)h` cannot leak a part in 10¹⁵ into a neighbour and the null limit is exact
to the bit. `splat` returns what fell outside the grid — the only way anything can go missing.

Deposition is used to stamp bodies onto the fluid grid (§8). It is **not** used to read a body's
gravity: the transported parcels are already a world cloud and are integrated where they are (§6).

### 4.2 `GridField`

Frames of cell masses [kg] on a fixed lattice, sampled every `dt`, interpolated linearly in time. It
implements `FieldContribution<f64>` (potential and gradient tensor through the cube kernel) — the
reference path, quadrature through the propagation integral like any other field — and `WorldSource`
with a channel of the author's choosing.

### 4.3 The linear response — the propagation integral as a dot product

The cells do not move and neither does the detector, so the arm points `p_u(τ)`, `p_l(τ)` and
therefore every kernel value in the integrand are independent of the measurement time. With
quadrature nodes `(τ_n, w_n)` and linear time interpolation of the masses,

```
δφ(t) = (m_A/ħ) Σ_n w_n Σ_i m_i(t − 2T + τ_n) · [K(p_u(τ_n) − x_i) − K(p_l(τ_n) − x_i)]
      = Σ_j Σ_i  W[j][i] · m[k₀ + j][i]
```

where `k₀` is the frame containing the start of the flight and `W[j][i]` collects the node weights
that interpolate onto frame `k₀ + j`. `W` depends only on the detector, the instrument configuration
and the *phase* of the flight against the frame grid — so it is built once and each measurement
is one pass over a handful of frames.

- **Same quadrature.** `flight_nodes` reproduces `instrument`'s two composite-Simpson halves node for
  node; the kernel is the same; only the order of summation differs. Agreement with
  `PropagationIntegral::per_ifo` is to rounding (`fast_path_equals_reference`, ≤1e-9, upright and
  tilted detectors, on and off the frame grid).
- **Cache.** Keyed by detector placement, configuration and phase (quantised to 2⁻³⁰ of a frame; the
  flight start is snapped so an aligned schedule lands on one phase despite rounding). A uniform
  schedule builds one response per detector; a jittered one builds a fresh response per measurement —
  the same cost as direct quadrature, never worse. At most 64 are kept.
- **Cost.** Building `W` is one propagation integral per cell; evaluating is `(J+2)·N` multiplies with
  `J = 2T/dt`. Measured end to end: 300 scenes sharing one 19 200-cell room and five detectors,
  responses included, ran in 15 s on two cores.

This is the reason the per-tick *evaluation* of a fluid is not a GPU problem: it is already a
matrix–vector product. The fluid *solve* is where the time goes (§5.6).

### 4.4 Loud at the edges

A field queried outside its span panics with the time and the span. The forward model reaches back
`2T` before each measurement; a simulation that does not cover that is an error to be fixed, not a
value to be guessed.

---

## 5. The fluid

### 5.1 Equations

Incompressible Boussinesq:

```
∂u/∂t + (u·∇)u = −∇p/ρ₀ + ν∇²u + gβ(T − T₀) ẑ
∂T/∂t + (u·∇)T = α∇²T + S
∇·u = 0
```

Velocities on cell faces, scalars at centres (a MAC grid), so the projection is exact to the solver
tolerance and has no checkerboard mode. Closed box; each pair of walls is no-slip or free-slip.

### 5.2 The step

1. **Advect** `u, v, w, T` by the start-of-step velocity: semi-Lagrangian with a **MacCormack**
   correction (a forward step, the same step run backwards to estimate its error, half that error
   removed). Where the correction would overshoot the values the forward step interpolated between,
   it is dropped — the limiter that keeps it monotone. Traces are single Euler steps; the pair is
   second-order in space and time.
2. **Diffuse**, explicit, sub-stepped to `dt ≤ 0.9·h²/(6·max(ν, α))`.
3. **Buoyancy** on free vertical faces from the mean temperature of the two cells the face joins.
4. **Project**: solve `A q = −h ∇·u` over fluid cells with Neumann conditions at every solid face,
   by MIC(0)-preconditioned conjugate gradients, warm-started from the previous solution; subtract
   `∇q` from the free faces.

**Why MacCormack and not plain semi-Lagrangian.** The usual argument ("stable fluids is fine") is a
graphics argument. Here the temperature field *is* the density signal: numerical diffusion smears the
plume and so smears exactly the spatial structure a denoiser would be trained on.

### 5.3 The equation of state — the Boussinesq guard

Boussinesq holds the solver's working density constant; reading it gives **exactly zero** gravity
signal. The signal is reconstructed from temperature:

```
ρ(T) = p₀ / (R_s T)                 (dry air, constant thermodynamic pressure; β = 1/T₀ follows)
δm_i = (ρ(T_i) − ρ(T₀)) · h³        in fluid cells; 0 in solid cells      → Fluid::mass_anomaly
```

**The anomaly, not the full density.** A finite box of full-density air would read as a static block
of matter that does not exist; uniform ambient air extends beyond the room and contributes nothing
time-varying. Where a body's displaced air matters, `Parcels::in_fluid` gives the body its mass in
excess of the air it displaces (`m ← m(1 − ρ_air/ρ_body)`), which is the consistent counterpart.

### 5.4 Surfaces and heat

`Heat` is the thermal condition on a domain wall or a solid surface:

- `Adiabatic`.
- `Isothermal(T_s)` — conduction through the half-cell next to the surface. Correct when the thermal
  boundary layer is resolved (the benchmark, §5.5).
- `Film { t_surface, h_over_rho_cp }` — `∂T/∂t = (h_c/ρc_p)(T_s − T)·a`, with `a` the surface area per
  unit volume. The model for room scale, where the real boundary layer is millimetres thick and the
  cell is not; `h_c` is a natural-convection coefficient of a few W m⁻² K⁻¹.

For a *moving* body the area density is `a = |∇φ|`, the gradient of its deposited volume fraction
(§8): `∫|∇φ| dV` is the body's surface area, so its heat output tracks its true area rather than the
staircase it happens to make on the grid, and a body smaller than a cell still has a surface. It
reads low: measured 8% under for a sphere four cells in radius (1.853 m² against 2.011), 28% under for
one a single cell in radius (0.360 against 0.503). The film coefficient it multiplies is itself known
only to tens of percent.

### 5.5 Validation

| Check | Result |
|---|---|
| Divergence after projection, random field around a solid block | ≤1e-8 of before (`tol = 1e-12`) |
| Stable stratification stays at rest (buoyancy is a pure gradient) | max speed ≤1e-9 m/s over 20 steps |
| Differentially heated cavity, de Vahl Davis 1983, Pr = 0.71, 32² | Nu = **1.1205** vs 1.118 (Ra = 10³); **2.283** vs 2.243 (Ra = 10⁴); hot = cold wall |

**What this does and does not establish.** The solver integrates the Boussinesq equations correctly
in a laminar, resolved, two-dimensional regime. At room scale the Rayleigh number is ~10⁹ and the grid
is centimetres to decimetres: the simulation is under-resolved with no turbulence model, the scheme's
own dissipation standing in for one. Plumes form, rise at plausible speeds and stratify under the
ceiling; none of that is validated against measurement. Benchmark-pass is not a claim that a simulated
room's convective *statistics* match a real room's — and those statistics (spectra, correlation
lengths) are what would decide whether a denoiser trained here transfers.

### 5.6 Cost

On two CPU cores: 48 000 cells ≈ 59 ms per step, split between advection (threaded over the lattice;
each site independent, so results are identical for any thread count) and the pressure solve
(~24 iterations at the default `tol = 1e-6`; MIC(0) is sequential). A 19 200-cell room ran 5× faster
than real time. This is the component that needs the GPU; the planned port replaces MIC(0)-PCG with a
parallel smoother or multigrid (`milestones/V2-HANDOFF.md` step 2).

---

## 6. Bodies as transported parcels

### 6.1 The mechanism

A body is voxelised **once**, at rest, into parcels of fixed mass (`shape::voxelise`, inheriting its
exact total mass and zero dipole). Each tick every parcel is carried to its current position by a map
and the forward model integrates the parcels where they are. Whether the body is rigid, articulated or
deforming is a property of the map, not a branch in the engine:

| Constructor | The map | Use |
|---|---|---|
| `Parcels::rigid(rest, pose)` | one isometry | the null limit; equals a v1 rigid source to rounding |
| `Parcels::skinned(rest, skeleton, weights, pose)` | bone transforms blended by per-parcel weights (linear blend skinning of the *volume*) | articulated and skinned figures |
| `Parcels::displaced(rest, placement, displacement)` | a displacement field in the body frame | vibration modes, any analytic deformation |

**Conservation by construction.** Parcels are relocated, never re-weighed: the posed cloud's masses
are the rest masses bit for bit (`mass_is_conserved_exactly_through_any_pose`). No rescale, no
Jacobian. Compression is implicit — parcels crowd together.

**Joints.** Where a joint folds a body onto itself the parcels pile up and nothing is counted twice,
because there are no per-link bodies to overlap. This is why the per-link amortised design was
dropped: its double-counted mass in the crook of a joint would have been a spurious differential
signal.

**Evaluation.** `Parcels` is a `FieldContribution` over `gravity`'s point kernel, posed per query time
with a one-entry cache (the phase integral evaluates both arms at each quadrature time). It reports a
track (the map's frame; twist by central differences) and its rest cloud, so it sits on the bundle's
source axis like a rigid body.

### 6.2 Skeletons and weights

`Skeleton::transforms(pose)` is forward kinematics: each bone rotates about its head relative to its
parent. `Skeleton::proximity_weights` gives per-parcel weights from distance to each bone segment
(Gaussian in the *excess* distance over the nearest bone, normalised).

The transport needs weights that exist at interior points and sum to one; it never differentiates
them. So this crude field is exact for mass, and only the *placement* of mass near a joint is rough.
The method the design settled on for authored rigs — diffusing surface weights inward by Laplace's
equation, partition of unity preserved by the maximum principle — is not built, and neither is the
skinned-mesh import that would feed it (§10).

**Skinning is not tissue mechanics.** Linear blend skinning loses volume at a twisting joint. The
transport reproduces whatever the map does; total mass stays exact, the local distribution is
skinning-grade.

### 6.3 `Walker` and `Plate`

`Walker` — a torso, head and four limbs from primitives, a six-bone skeleton, limbs swinging about
shoulders and hips as it translates. A deliberately simple complex object: several parts, a real
skeleton, non-rigid motion.

`Plate` — a simply supported Kirchhoff plate: mode `(m, n)` has shape `sin(mπξ) sin(nπη)` and
`ω_mn = π²[(m/a)² + (n/b)²]·√(D/ρh)`, `D = Eh³/12(1−ν²)`. A 4 × 3 × 0.2 m concrete slab has
`f₁₁ ≈ 57 Hz` — far above the gradiometer's band (flight time 1.46 s), where it is strongly averaged.
Low-frequency structural motion — a wall swaying as a body — is v1's `Path::Oscillation` on a rigid
slab, wrapped `Ambient`.

---

## 7. Rigid dynamics and contact

**Free flight.** Translation by velocity-Verlet (exact under constant acceleration). Rotation by
`source::step`, the engine's momentum-splitting free-rigid-body integrator, in the body's principal
frame (from the cloud's own second moment): `|L|` conserved to rounding, energy bounded, no secular
drift — the same guarantee the prescribed `FreeRotation` motion carries.

**Contact.** Sequential impulses with restitution and Coulomb friction (after Szauer, *Game Physics
Cookbook*, ch. 15 — used as a reference for structure, not a dependency). Pairs: sphere–plane,
box–plane (corners), sphere–sphere, sphere–box. **Box–box is not implemented and is refused with a
panic** rather than passed through. Two changes from the game formulation, both for §1's reason:

- **Position correction, not velocity bias.** Penetration is removed by moving bodies apart along the
  normal; no energy is injected.
- **Sleep.** A body in contact and below `0.02 m/s`, `0.05 rad/s` for `0.5 s` is frozen: velocities
  zeroed, excluded from integration, immovable to others until struck. Its pose is then exactly
  constant — a settled object contributes no jitter to the mass field.

**Recording.** `RigidWorld::record` samples every body at a fixed interval and returns
`RecordedRigid`s: each a fixed body cloud carried by the recorded pose (cubic Hermite in position —
exact for ballistic flight — and normalised linear blend in orientation). A `RecordedRigid` *is* a
`SourceDynamics`, so a simulated body rides the amortised rigid path of the forward model.

**The two-way seam.** `RigidBody::force` / `::torque` are applied each step and are zero unless
filled. A fluid's back-reaction would be written there; nothing currently does (§8).

---

## 8. The coupled room

`simulate_air(room, fixed, obstacles, t0, t1, frame_dt, cfl)` runs the air around the bodies and
returns the recorded mass anomaly as an ambient `GridField`.

**One-way, and why that is physics rather than convenience.** Bodies move as they move; the air
responds and does not push back. Air against a solid body is a density ratio of about 1000:1, so the
back-reaction on a body is a part in a thousand of its weight. Two-way coupling earns its place for
water or near-buoyant bodies, not here.

**Each fluid step**, every obstacle (any `WorldSource`) is deposited onto the fluid grid: parcel
volumes (mass over bulk density) give the occupancy; cells at least half occupied become solid and
carry the body's local velocity (deposited momentum over deposited mass, from a finite difference of
the world cloud) as a moving no-slip boundary; a `Film` surface heats the air through `|∇φ|` (§5.4),
an `Isothermal` one through its solid faces. `fixed` cells are permanent solids — furniture, a
radiator. The step is the frame interval subdivided to hold the Courant number at `cfl`.

**The staging.** Dynamics do not depend on the air, so a scene is built in order: rigid dynamics →
recorded tracks → air around them → both into the `Scenario`. Nothing is co-simulated.

---

## 9. Exit requirements (definition of done)

| Requirement | Check | Tol |
|---|---|---|
| cube kernel correct near, far and inside | `cube_matches_quadrature_near`, `cube_matches_point_far`, `cube_finite_inside_and_on_boundary`, `gradient_tensor_matches_finite_differences`, `gradient_trace_is_poisson_inside` | 2e-5 / 1e-5 / 1e-6 / 1e-4 / 1e-10 |
| deposition conserves; null limit exact | `splat_is_a_partition_of_unity`, `splat_at_cell_centres_is_the_identity`, `splat_reports_what_leaves_the_grid` | 1e-14 / **exact** / 1e-14 |
| grid fast path ≡ propagation integral | `fast_path_equals_reference`, `uniform_schedule_reuses_one_response`, `static_field_is_time_independent` | 1e-9 / structural / 1e-12 |
| out-of-span query is loud | `query_outside_the_span_is_loud` | panics |
| fluid: projection, rest, benchmark | `projection_removes_divergence`, `stratified_fluid_at_rest_stays_at_rest`, `heated_cavity_matches_de_vahl_davis` | 1e-8 / 1e-9 m/s / Nu within 2%, hot = cold within 1% |
| Boussinesq guard | `equation_of_state_reads_temperature`, `static_thermal_field_has_a_gravity_signal` | 1e-15 kg / sign |
| parcels: null limit, conservation, kinematics | `rigid_parcels_equal_the_rigid_source`, `one_bone_skeleton_is_rigid`, `mass_is_conserved_exactly_through_any_pose`, `skinning_weights_are_a_partition_of_unity`, `hinge_moves_the_far_parcels_rigidly`, `walker_walks_and_swings` | 1e-11 / 1e-13 m / **exact** / 1e-15 / 1e-6 m / structural |
| plate | `plate_frequency_and_nodes` | 1e-9 rel |
| rigid: flight, rotation, contact, rest | `free_fall_is_ballistic`, `free_rotation_conserves_angular_momentum_and_energy`, `bounce_height_follows_restitution`, `equal_spheres_exchange_velocity_and_conserve_momentum`, `sphere_rests_on_a_box`, `dropped_box_settles_and_sleeps_exactly`, `box_box_contact_is_refused` | 1e-10 m / 1e-10, 1e-5 / 1 cm / 1e-9 / 5 mm / **bit-constant** / panics |
| recording | `recording_reproduces_ballistic_flight_between_samples` | 1e-10 m |
| coupled room | `warm_body_sheds_a_plume_that_gravitates`, `moving_body_stirs_isothermal_air`, `immersed_surface_area_is_the_true_area` | structural / 1e-12 of cell air mass / 10%, 35% |
| whole path | `generate/tests/world::{rigid_parcels_reproduce_the_rigid_source, simulated_scene_runs_end_to_end}` | 1e-11 / structural |

---

## 10. Open sub-questions and known limits

- **GPU.** Nothing here runs on the GPU. The fluid solve is the measured bottleneck and the first
  port; per-tick parcel clouds in the forward kernel are the second.
- **Fluid validity** beyond Ra = 10⁴, in three dimensions, and at room scale (§5.5). No open
  boundaries, ventilation or inflow. No turbulence model.
- **Convective-noise band.** Whether convective gravity-gradient noise overlaps the tracking band
  decides whether a learned denoiser is needed or a spectral filter suffices. One data point exists: a
  70 °C radiator about a metre from a tower gave a peak ambient phase of 2.2e-2 rad against a 1e-3 rad
  shot floor; a warm 0.4 m block gave ~1e-4 rad. Neither is validated.
- **Contact:** box–box (SAT with clipping and manifold reduction), joints and constraints, stacks under
  the rest-determinism test.
- **Two-way coupling** — the slot exists (§7); fill it only for a scene that needs it.
- **Volumetric skinning weights** by Laplace diffusion from authored surface weights, and skinned
  glTF import (§6.2).
- **Octrees.** None is built. The mass octree (interior collapse for the parcel sum) is worth building
  only if profiling shows the parcel sum dominating; the collision broad-phase only with many bodies;
  fluid AMR only after the GPU port.
- **Room acoustics** — standing-wave density modes as an analytic grid field.
- **Differentiability.** The crate is `f64`, not `Scalar`-generic. Nothing in it is on the CRB's
  differentiable path (`design/world.md` §7); if a simulated source ever needs a Jacobian, that
  changes.
