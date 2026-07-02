# Cavendish — rigid-body rotation: Spin, free rotation, and the intermediate-axis effect

Adds rotation as a source class — the pure-quadrupole complement to the (monopole-dominated)
linear motions. It slots into the **Orientation** slot of the existing motion decomposition and
the `SourceDynamics` seam, and it is **in scope**: cheap, integrable, well-posed — not a parked
dynamics seam. Destined for the motion model (spec §motion + `DESIGN.md` `source`), the gravity
reduction (spec + `DESIGN.md` `gravity`), the identifiability section (spec §5.4), and the test
matrix. Builds on `identifiability-and-shape.md`.

---

## 0. The gap, and where it slots

Rotation about an axis is genuinely separate from linear motion, and the motion model already
has the slot for it: **Path × Timing × Orientation + Placement**. Path is where the CoM goes
(translation); Orientation is how the body is turned over time — orthogonal axes. None of the
current named motions is a free spin: Pendulum is a *swing* about an external pivot,
Circular is *orbital* translation of the CoM. Steady rotation about a body axis with the CoM
fixed is missing. Add it.

---

## 1. The primitives (prescribed and solved)

- **`Spin(axis, rate, phase)`** — prescribed steady rotation (kinematic): a pose whose rotation
  advances linearly with `t`.
- **`Libration(axis, amplitude, freq)`** — prescribed oscillatory rocking about an axis.
- **`FreeRotation`** (the Euler top) — *solved*: integrate the torque-free Euler equations for
  `ω(t)`, reconstruct the quaternion. This is the dynamical generalisation of `Spin`.

`Spin`/`Libration` are the prescribed case; `FreeRotation` is the solved case — mirroring the
prescribed-vs-solved split the source seam already has (spec App. A).

---

## 2. Why rotation is the pure-quadrupole probe

Rotation about a fixed CoM with constant angular momentum leaves the **monopole fixed** (total
mass is rotation-invariant) and the **dipole identically zero** (it vanishes about the CoM at any
orientation). So the entire *time-varying* signal lives in the **quadrupole and higher** moments.
Linear motion is monopole-dominated; free rotation is its exact complement — a source that
excites *only* the channel normally hardest to reach (the quadrupole rung of the identifiability
ladder). It is almost a calibration source for shape.

- The quadrupole modulation comes out principally at **2Ω** (the quadrupole's half-turn
  symmetry), with higher harmonics from higher moments. A **tilted** spin axis tumbles `Γ_zz`
  directly and gives a richer signal than a vertical axis.
- **Only asymmetric bodies signal.** A body spun about an axis of symmetry (a cylinder about its
  long axis; a sphere about anything) presents an invariant mass distribution → static field →
  flat ΔΦ. Symmetry about the spin axis kills the signal — which is itself a clean invariant test.

---

## 3. Inertia tensor = gravitational quadrupole (the loop-closer)

Both are the same second moment of the cloud, `M_jk = Σ m r_j r_k`:
- inertia tensor `I = (tr M)·𝟙 − M`
- quadrupole `Q = 3M − (tr M)·𝟙`

Both are linear in `M`, so they have **identical principal axes** (different eigenvalues, same
eigenvectors). Therefore the body's principal **inertia** axes — which govern how it tumbles —
*are* its principal **quadrupole** axes — which the array reconstructs (§4.2). The dynamics and
the static signature are two views of one tensor.

The payoff: a tumbling asymmetric body **over-constrains its own shape**. The instantaneous
quadrupole gives `M`'s anisotropy; the *flip cadence* gives its eigenvalue ratios (the moments);
the two must be consistent. That self-consistency is information the inference can exploit and a
test it must pass.

**Architectural consequence:** the moments are *not* parameterised — they fall out of the cloud.
`I_jk` (equivalently `M_jk`) is a **load-time reduction over the voxel cloud, a sibling of the
multipole reduction** already done — the same second-moment sum, repackaged. Diagonalise once →
principal axes and moments, ready for the integrator.

---

## 4. The intermediate-axis effect (Dzhanibekov), framed accurately

Torque-free rotation of a body with three distinct moments `I₁ < I₂ < I₃` is stable about the
largest and smallest axes and **unstable about the intermediate** one. Linearising Euler's
equations about the intermediate axis gives a real growth rate

    σ ≈ Ω · √[ (I₂−I₁)(I₃−I₂) / (I₁ I₃) ],

so a tiny perturbation grows and the body undergoes periodic ~180° flips — quiet dwell, sudden
tumble, quiet dwell (the tennis-racket / Dzhanibekov effect).

**The accurate framing (don't call it chaos):** the free asymmetric top is **integrable, not
chaotic** — exactly solvable in Jacobi elliptic functions, with `ω` tracing a *closed* polhode on
the inertia ellipsoid. The drama is a **separatrix** effect: the intermediate-axis trajectory is
a separatrix, the period diverges as you approach it, and near it the motion is exquisitely
sensitive to initial conditions. This is a *different animal* from the pendulum:

- **Pendulum** (driven, 3D) → a genuinely **chaotic** source.
- **Euler top** (free) → a **deterministic-but-separatrix-sensitive** source.

Worth having both, precisely for that contrast. The flips manifest as the quadrupole tensor's
principal axes swinging through 180° — a showcase for the array's orientation recovery (large,
fast reorientations the tracker must follow).

---

## 5. Architecture — nearly free, and explicitly NOT a physics engine

### It slots into the existing seam
A rigid body is already a fixed body-frame cloud plus a time-varying pose (`body_cloud` +
`pose_at`). So:
- **`Spin`** is a pose whose rotation advances with `t`, parameterised by `(axis, rate, phase)` —
  a tiny set, so "upload parameters, not poses" still holds; the kernel transforms the fixed
  cloud per tick exactly as for any motion.
- **`FreeRotation`** integrates Euler's equations for `ω(t)` — an ODE-in-time of the *same
  complexity class as the pendulum* (already in v1), so it lives on the GPU's one sequential
  axis. Upload `(I₁,I₂,I₃, ω₀)`, integrate on-device, reconstruct the quaternion.

Unlike genuinely-parked dynamics (fluids, deformation), this is cheap, integrable and well-posed
— a clean in-scope addition, not a new parked seam.

### Not a game physics engine (the scoping that governs how it's built)
What this is: **rigid-body kinematics + torque-free rotational dynamics** (Euler's equations,
quaternion integration, an inertia tensor from the mass distribution). That is the
rotational-dynamics *core* of a game engine — and nothing else from one.

What it is **not** — the 90% that makes an engine an engine, all absent because **the bodies
never touch**:
- collision detection (broad/narrow phase, GJK/EPA, contact manifolds),
- constraint / contact solving (sequential impulse, LCP, friction cones),
- stacking, sleeping, broadphase partitioning, continuous collision.

Two ways the problem is genuinely *easier* than a game engine, not merely smaller:
- **Offline and deterministic, not real-time interactive.** No 16 ms frame budget; use a tiny
  `fine_dt` and an expensive accurate integrator because nothing waits on a frame. The adversarial
  real-time robustness that makes game physics hard simply isn't present.
- **Torque-free.** `L̇ = 0` — no contact forces, joints, or motors; the cleanest case Euler's
  equations have, and integrable.

Only **Pendulum** and **FreeRotation** are actual ODE integration — the *same* small symplectic
integrator wearing two hats. The other motions (LinearPass, Oscillation, Circular, Lift, Spin,
Libration) are parameterised paths, not physics.

**Hard constraint — do not pull in an off-the-shelf physics crate (rapier, etc.).** They are
built around the collision/constraint machinery we don't want, would drag the whole engine in,
and — decisively — are **not differentiable and not generic over the scalar type**, which breaks
the autodiff/CRB thread that is load-bearing in the spec (`nfr:diff`, §5/M5). The integrator must
be `fn step<S: Scalar>(…)` so duals flow through it. Writing the ~200 lines of Euler +
quaternion ourselves is the *right* call, not a reluctant one: it is smaller than the existing
pendulum integration work and keeps the differentiability we actually need.

---

## 6. ML payoff

- **Confusion-pair generator.** Near the separatrix, two bodies with nearly-identical initial `ω`
  diverge into completely different flip cadences — instantaneously similar, temporally divergent.
  A natural family for the Gradar prior's degenerate-pair test (§5.3).
- **A sharp world-model test.** "Track orientation through a flip" tests whether the temporal
  model actually follows the kinematic state rather than smoothing across the tumble.

---

## 7. Honest caveats (don't oversell)

- **None of this beats `(a/r)²`.** A distant tumbler still has a tiny quadrupole signal; the
  dynamics make the signal *richer and more structured*, not *louder*. Given a detectable
  quadrupole, free rotation adds the moment-ratio information on top; below the noise floor it
  adds nothing.
- **Asymmetry required.** Spin a symmetric body about its symmetry axis → static field → flat
  ΔΦ. (This is the invariant test, not a bug.)

---

## 8. Where this lands in the doc (next actions)

- **Motion model** (spec §motion + `SourceDynamics` seam in `DESIGN.md` `source`): add `Spin`,
  `Libration` (prescribed) and `FreeRotation` / Euler-top (solved); place the latter alongside
  the pendulum as the second ODE motion.
- **`gravity`** (spec + `DESIGN.md` `gravity`): add the inertia/second-moment tensor as a
  load-time reduction beside the multipole reduction, and state the `Q` and `I` share `M` identity.
- **Identifiability** (spec §5.4): rotation is the pure-quadrupole probe; a tumbling body
  over-constrains its own shape (instantaneous `Q` vs flip cadence).
- **Test matrix**: the intermediate-axis flipper joins the confusion-pair family; the
  symmetry-spin flat-ΔΦ check joins the invariants (sibling of trace-free and mass-linearity).
- **`DESIGN.md`**: note the symplectic integrator is shared by `Pendulum` and `FreeRotation`, is
  generic over `Scalar`, and is explicitly **not** a physics-engine dependency.
- **Scope**: in-scope (cheap, integrable, well-posed) — not parked.
