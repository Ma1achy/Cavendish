# Cavendish — `source` drill-down

> Subsystem design for the `source` crate: what a *moving mass* is — its kinematics over time and
> its mass distribution. Companion to `DESIGN.md` (§3.1 seam, §6 inventory) and the spec
> (§motion `sec:motion`, the rotation paragraph, `nfr:symplectic`, the invariants). Carries the
> rotation work specced in `rigid-body-rotation.md`.
>
> **Dependencies:** `source → shape` (bodies → clouds), `gravity` (the `Cloud` type, the `Inertia`
> reduction), `math` (`Isometry3`, quaternions, the `Scalar` seam). It does **not** depend on `instrument` or
> `compute`; rather `compute` depends on it (it calls/​re-expresses this crate's integrator, §5).

---

## 1. Responsibility & boundaries

**Owns:** the `SourceDynamics` seam; the **motion factoring** (`Path × Timing × Orient +
Placement`); the closed-form motion library; the two ODE motions and the **hand-written symplectic
integrator** they share; the body library (clouds via `shape`); single-source time-sequencing.

**Does not own:** the field kernel, the `Cloud` type, or the `Inertia` reduction (`gravity`);
voxelisation and mesh import (`shape`); the phase model (`instrument`); execution/​batching (`compute`); multi-source scene
assembly and the `Prior` (`scenario`). `source` *defines* a source; it neither evaluates its field
nor composes scenes.

**Invariants it guarantees:** a rigid body's `body_cloud` is fixed (motion is pose, not
deformation); the field is linear in mass (the cloud scales, §6); the two ODE motions are
integrated symplectically so energy/​`|L|` stay bounded (§5); a source's principal moments come
from its cloud, never from free parameters.

---

## 2. The `SourceDynamics` seam

```rust
pub trait SourceDynamics {
    fn body_cloud(&self) -> &Cloud;              // fixed body-frame mass cloud (rigid)
    fn pose_at(&self, t: f64) -> Isometry3;      // world ← body, at time t
    fn motion_at(&self, t: f64) -> BodyMotion;   // twist (lin+ang vel) + accel, for the bundle
}
```

`body_cloud` is built once (§6) and never changes — rigidity is structural. `pose_at` is
closed-form for most motions and **ODE-derived (integrated, cached)** for the pendulum's timing and
free rotation's orientation (§5). `motion_at` supplies the kinematic state the `StateBundle`
records (it is not needed by the field evaluation). The deformable extension (a `world_cloud(t)`
that varies) is the seam's growth point — noted, not built (`DESIGN.md` §3.1).

`pose_at` and the integrator (§5) are **generic-friendly**: the underlying `step` is written over
the `Scalar` seam so a `Dual` tangent flows through to the signal, which is how the CRB
differentiates w.r.t. trajectory parameters (`compute.md` §8).

---

## 3. The motion factoring — `Path × Timing × Orient + Placement`

A source's pose is four orthogonal pieces composed in one place (spec §motion):

```
pose_at(t) = Placement ∘ Isometry{ rotation: Orient(t),
                                    translation: Path( Timing(t) ) }
```

- **Placement** `P ∈ SE(3)` — the static world frame the whole motion lives in (where it is).
- **Path** — the CoM space-curve, a function of progress `u`: where the body goes.
- **Timing** — `t → u`: how the body advances along the path (the schedule of motion).
- **Orient** — `t → R`: how the body is turned, independent of where it goes.

Orthogonality is the point: any orientation composes with any path (a spinning body on a linear
pass is `Path=LinearPass, Orient=Spin`), and `Placement` relocates the lot without touching the
rest. Two motions break the closed-form `Timing`/`Orient` and are ODE-derived (§5); everything
else is analytic (§4).

```rust
struct Trajectory { placement: Isometry3, path: Path, timing: Timing, orient: Orient,
                    segments: Option<Vec<Segment>> }   // §8 time-sequencing
enum Path   { Static, LinearPass{a,b}, Oscillation{axes,amp,freq,phase},
              Circular{radius,freq}, Lift{height,profile} }
enum Timing { Uniform{speed}, Profile{trapezoid}, PendulumOde{length,theta0,thetadot0} }
enum Orient { Fixed(Quat), Tangent, Spin{axis,rate}, Libration{axis,amp,freq},
              FreeRotation{omega0} }
```

---

## 4. The closed-form motion library

Evaluated directly at any `t`, no state (so trivially parallel and re-entrant on both backends):

- **`Static`** — `p(u)=0`; the body sits at the placement origin.
- **`LinearPass{a,b}`** — `p(u)=a+u(b−a)`; a straight transit (the canonical "mass walks past").
- **`Oscillation`** — `p(t)=Σ_k A_k sin(2πf_k t+φ_k) ê_k`; 1/2/3-axis (a Lissajous figure for 2/3),
  a convenience motion fixing path and timing together.
- **`Circular`** — `p(t)=R(cos2πft, sin2πft, 0)`; orbital translation of the CoM.
- **`Lift`** — vertical transit with a trapezoidal **`Profile`** (accelerate/​cruise/​decelerate);
  the realistic building-lift source.

Orient closed-forms:
- **`Fixed`** constant; **`Tangent`** aligns a body axis to `dp/dt`; **`Spin{axis,rate}`**
  `R(t)=R₀·exp(t·rate·[axis]_×)` (steady); **`Libration{axis,amp,freq}`**
  `R(t)=R₀·rot(axis, amp·sin2πfreq t)` (rocking).

---

## 5. The two ODE motions and the shared symplectic integrator

The rotation work (`rigid-body-rotation.md`) lands here. **Pendulum** (timing) and **FreeRotation**
(orientation) are the only state-carrying motions; both step one **hand-written symplectic
integrator**, `fn step<S: Scalar>(state, dt) -> state`.

- **Pendulum (timing).** State `(θ, θ̇)`, law `θ̈ = −(g/L)sinθ`. Semi-implicit Euler
  (`θ̇ ← θ̇ + θ̈·dt; θ ← θ + θ̇·dt`) keeps energy bounded over arbitrarily long runs — for a
  periodic source whose value is its spectrum, energy drift is spectral drift (spec
  `nfr:symplectic`). Position rides the arc; the 3-D driven pendulum is the **chaotic** source.
- **FreeRotation (orientation).** State `(ω_body, q)`, **torque-free Euler equations**
  `I₁ω̇₁=(I₂−I₃)ω₂ω₃` (cyclic) with `q̇ = ½ q⊗(0,ω)`. A structure-preserving step conserves energy
  and `|L|`, so the polhode stays **closed** — essential for faithful intermediate-axis
  (Dzhanibekov) flips, the **integrable, separatrix-sensitive** source that deliberately contrasts
  with the chaotic pendulum (spec §5.4, `T4.4`).

**The moments come from the cloud, not from parameters.** `FreeRotation` reads `(I₁,I₂,I₃)` and the
principal axes from `body_cloud().inertia` (gravity's load-time reduction, `design/gravity.md` §6);
the only per-source parameter is `ω₀`. So a tumbling body is fully determined by its shape plus an
initial spin — and its quadrupole signature and its flip cadence are forced to agree (the
over-constraint of spec §5.4).

**This is an integrator, not a physics engine** (the governing scoping). What is built: rigid-body
kinematics + torque-free rotational dynamics — Euler's equations and quaternion integration, ~200
lines. What is **not**, because bodies never touch: collision detection, constraint/​contact
solving, stacking/​sleeping/​broadphase. The problem is *easier* than a game engine, not merely
smaller — it is offline and deterministic (a tiny `fine_dt` and an accurate stepper, no real-time
budget) and torque-free (`L̇=0`, integrable). **No off-the-shelf physics crate (rapier, etc.):**
they carry the collision/​constraint machinery we do not want, and — decisively — are **not generic
over the scalar type and not differentiable**, which would break the autodiff/​CRB thread. The
integrator must be `fn step<S: Scalar>(…)` so duals flow; hand-rolling it is the right call, not a
reluctant one.

**Relationship to `compute`.** This crate owns the **canonical** integrator (Rust, generic over
`Scalar`); `pose_at` wraps it with caching (integrate once, sample at the measurement cadence) for
the CPU/​reference path. `compute`'s `WgpuBackend` **re-expresses the same `step` in WGSL** (Pass 1,
`compute.md` §5) and is validated against this one — exactly the gravity-kernel pattern (one Rust
truth, one GPU re-expression).

---

## 6. The body library — clouds from `shape`

A source's `body_cloud` is built at load, then frozen — by the **`shape` crate**
(`design/shape.md`), which owns the whole geometry→mass pipeline:

- **Primitives** — sphere, shell, cuboid, cylinder, unions (a scaffold is a union of cylinders),
  each with an analytic moment oracle, so the voxeliser is convergence-testable.
- **Mesh import** — `shape` parses the file (STL/OBJ/glTF, explicit scale), classifies
  watertightness, voxelises robustly (generalised winding numbers for dirty meshes), and returns
  the same `Cloud`. `source` owns none of it; it resolves a dictionary entry to a cached
  unit-mass cloud and applies the mass.

**Linearity in mass** is a property of construction: clouds are cached at unit total mass and
`source` scales per-element mass at assembly (one pass), so the field and `ΔΦ` scale linearly
(`INV.2`) and a `Prior` drawing masses never re-voxelises. Once built, the cloud (and its cached
`Multipole`/​`Inertia`) is immutable; the pose moves it.

---

## 7. `motion_at` — twist and acceleration

For the bundle's kinematic channels: linear velocity/​acceleration of the CoM (analytic derivative
of `Path∘Timing` for closed-form motions; from the integrator state for the pendulum) and angular
velocity/​acceleration (`ω`, `ω̇` — analytic for `Spin`/​`Libration`, the Euler state for
`FreeRotation`). `motion_at` must agree with the finite-difference of `pose_at` (an exit check, §11)
— the guard that the reported kinematics are the ones actually driving the pose.

---

## 8. Composition — time-sequencing (single source)

A `Trajectory` may be a **sequence of segments** (`Some(Vec<Segment>)`): move, then hold, then
move; or transit then begin a free tumble. Each segment is a `(Path,Timing,Orient)` over a
sub-interval, with **state continuity** at the joins (position, velocity, and — for ODE segments —
`θ̇`/​`ω` carried across). `source` owns this single-source sequencing; **multi-source** composition
(a scene of independent sources, the multi-target case) is `scenario`'s, since gravity is linear and
a scene is just `Vec<Source>` (`DESIGN.md` §2).

---

## 9. Errors

Construction validates and returns `Result<_, SourceError>`: degenerate trajectory params
(`LinearPass` with `a=b`, non-positive pendulum length, zero-mass body), an un-importable/​non-
watertight mesh (surfaced from `gravity`), or discontinuous segment joins. After construction
`pose_at`/​`motion_at` are infallible.

---

## 10. Public API surface

`SourceDynamics`, `Source` (= `Trajectory` + a built `Cloud` + identity), the body builders
(`Source::primitive`, `Source::from_mesh` — thin wrappers over `shape`), the `Path`/​`Timing`/​`Orient` constructors, and the
**integrator** `step` (exposed so `compute` can call it / re-express it). `scenario` composes
`Source`s; `compute` consumes `body_cloud` + `pose_at`/​`step`; `instrument` consumes the poses via
the forward model.

---

## 11. Exit requirements (definition of done)

| Requirement | Check | Tol |
|---|---|---|
| closed-form poses | linear/​circular/​oscillation/​lift produce the intended curves | exact |
| pose composition | `Placement∘Path∘Timing` + `Orient` compose correctly (order, frames) | exact |
| pendulum energy | symplectic step keeps pendulum energy bounded over long runs | bounded |
| free-rotation conservation | energy and `|L|` conserved; polhode closed; intermediate-axis flips occur | tol |
| moments-from-cloud | `FreeRotation` reads `(I₁,I₂,I₃)`/​axes from `body_cloud.inertia` | structural |
| symmetry-spin | a symmetric body spun about its symmetry axis yields an invariant world cloud | exact (`INV.5`) |
| mass linearity | scaling density scales the cloud (hence `ΔΦ`) linearly | exact (`INV.2`) |
| `motion_at` | matches central-difference of `pose_at` | ~1e-6 |
| autodiff | `pose_at` differentiable w.r.t. trajectory params via `Dual` | ~1e-6 |

`source` needs only `gravity` beneath it; with the integrator and the body library it is verifiable
without `instrument`/​`compute`.

---

## 12. Open sub-questions (resolve in implementation)

- **ODE segment joins.** Sequencing a closed-form segment into an ODE segment (e.g. transit →
  free tumble): how the initial `ω`/​`θ̇` is set at the join, and whether mixed sequences need a
  uniform state representation.
- **Path parameterisation.** Arc-length vs a normalised phase for `Timing` — arc-length is cleaner
  for `Uniform{speed}` but costs a length integral for curved paths; pick per path type.
- **`Tangent` under ODE timing.** `Tangent` orient needs `dp/dt`; well-defined for closed-form
  paths, needs the integrator's velocity for an ODE-timed path — a small coupling to nail down.
- **Rotation integrator choice.** Which structure-preserving scheme (a Lie-group/​`SO(3)` step vs a
  symplectic splitting) best conserves `|L|`/​energy for **near-degenerate** moments — exactly the
  Dzhanibekov regime that stresses it (cf. gravity's eigensolve sub-question).
