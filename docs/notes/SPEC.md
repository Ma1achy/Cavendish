# STALE — SUPERSEDED BY `cavendish.tex`

> **THIS DOCUMENT IS OUT OF DATE. DO NOT IMPLEMENT FROM IT.**
>
> The authoritative, maintained specification is now **`cavendish-spec/cavendish.tex`**
> (v0.5, 25 Jun 2026). This file was last current ~20 Jun 2026 and is roughly one major
> version behind: it predates the **gravitational-radar** reframing and is missing the
> **detector array**, the **ULDM common-mode channel**, the **measurement schedule**, the
> **CPU/GPU compute split**, **Lomb–Scargle** analysis, the **radar track labels**, and the
> **identifiability ladder**. In places (e.g. "no GPU kernel") it states the *opposite* of the
> current design.
>
> Everything below is retained for history only.

---

# Cavendish — Simulator Specification

Working title: **Cavendish** (provisional — the gradiometer is a Cavendish-class
device, two atom clouds reading G from a known source mass; this simulator runs that
same forward map inverted: G held known, the source inferred). This is the
component-by-component build spec —
modules, the load-bearing traits, data types, the forward-model algorithm, the
config schema, the test/oracle strategy, and the milestone slice. It is the
companion to `ARCHITECTURE.md` (which holds the shape and the rationale); where this
document gives a type or an algorithm, the architecture doc gives the reason.

Physics is pinned to G. Parish's AION GGN report (the model) and his reference code
(the oracle: constants, trajectory construction, the phase integral).

---

## 1. What it is

A Rust library that simulates the **differential phase signal of an atom-gradiometer**
(AION-10 geometry) responding to a moving external mass, and emits the **full
per-measurement state** — source kinematics, mass distribution, gravitational field,
and the noisy signal — as a time series of tensors. It runs **live as a data source**:
a training loop asks for scenarios, the library generates and streams them. The
eventual consumer is a JEPA-style model that infers the source from the signal; that
model is out of scope here, but its two interfaces (the streaming SDK, the
source-agnostic state contract) are designed in now.

The signal is **one channel** of the emitted state. The simulator produces the state
as ground truth; a model will later produce the same state shape as a prediction; the
viewer renders either. That shared bundle (§5) is the only coupling between the
engine and anything downstream.

---

## 2. Scope

### v1 — this spec
- **Source**: rigid bodies only. Arbitrary mesh → voxel cloud; analytic primitives as
  the oracle. A library of prescribed/physical **trajectories** (static, linear pass,
  1D/2D/3D oscillation, circular, pendulum).
- **Forward model**: the propagation-phase double-difference integral (George's
  eq 64–65), evaluated along the four parabolic arm trajectories.
- **Gravity**: direct element sum near-field; multipole expansion far-field;
  swappable discretisation element (point default; sphere, cube behind the trait).
- **Noise**: pluggable stack (shot noise, vibration residual); v1 can ship with the
  stack empty and add sources incrementally — the forward model validates noiseless.
- **Interfaces**: Python SDK (`run`, `stream`) + thin CLI; egui/wgpu debug viewer.
- **Output**: the state bundle as torch-ready tensors, live; optional frozen cache.

### Parked — seams kept, not built
- Non-rigid source dynamics (articulated, deforming, fluid) behind `SourceDynamics`.
- The ML model that consumes the stream.
- ULDM signal injection (the eventual *separation* target).
- Identifiability / Cramér–Rao analysis (wants a differentiable forward model).

---

## 3. Crate & module map

```
cavendish/
├── crates/
│   ├── sim-core/                 # the engine — all physics, no Python, no UI
│   │   ├── gravity/              # 6.1  potential/field/tensor; elements; multipole; oracle primitives
│   │   ├── source/               # 6.2  SourceDynamics (rigid), Trajectory library, voxel import
│   │   ├── instrument/           # 6.3  gradiometer geometry, arm-trajectory construction, PhaseModel
│   │   ├── noise/                # 6.4  NoiseSource stack
│   │   ├── scenario/             # 6.5  Scenario assembly + prior/sampler
│   │   ├── state/               # 5    StateBundle types + field selection + (de)serialise
│   │   ├── config/              # 6.6  the one typed config schema
│   │   └── generate/            #      per-scenario evaluation + rayon parallel batch
│   └── viewer/                  # 6.8  eframe/egui + wgpu (separate, one-way dep on sim-core)
├── sdk/                          # 6.7  maturin/PyO3 package: run()/stream() + CLI console entry
└── docs/                         #      ARCHITECTURE.md, SPEC.md
```

Dependency direction is one-way out of `sim-core`. SDK is the primary consumer, the
viewer secondary. Two Rust crates, one Python package.

---

## 4. The seams (load-bearing traits)

Four traits are the flex points; everything else is composition over them. Keeping
these clean is what lets the parked features land without touching the core.

```rust
/// A thing that produces a gravitational potential/field at a world point.
/// Implemented by discretisation ELEMENTS (point/sphere/cube), by the CLOUD
/// (sum over elements + multipole far-field), and by analytic ORACLE primitives.
trait GravitySource {
    fn potential(&self, r: Vec3) -> f64;     // V(r)            [J/kg]
    fn field(&self, r: Vec3) -> Vec3;        // g = -∇V         [m/s²]
    fn gradient(&self, r: Vec3) -> Mat3;     // T = ∂gᵢ/∂xⱼ     [1/s²]  (grid/diagnostic)
}

/// How the source's mass configuration evolves. v1: Rigid only.
/// A continuous function of time — the instrument samples it finely for the
/// phase integral, the state bundle samples it at the measurement cadence.
trait SourceDynamics {
    fn cloud_body(&self) -> &Cloud;          // body-frame mass elements (fixed for rigid)
    fn pose_at(&self, t: f64) -> Pose;       // COM position + orientation at time t
}

/// Turns source + instrument into the differential phase at one measurement time.
/// v1: PropagationIntegral (eq 64–65). Alt: QuasiStaticGradient (kaT², sanity only).
trait PhaseModel {
    fn delta_phi(&self, src: &dyn SourceDynamics, instr: &Instrument, t_meas: f64) -> f64;
}

/// One additive contribution to the signal. The stack is Vec<Box<dyn NoiseSource>>.
trait NoiseSource {
    fn add(&self, t: &[f64], clean: &mut Array1<f64>, rng: &mut StdRng);
}
```

A fifth, smaller seam sits **inside** `Rigid`: the motion is itself pluggable.

```rust
/// A rigid-body motion factors into a path, a timing law along it, an orientation law,
/// and a rigid placement. The exposed API is named conveniences over this core.
trait Trajectory { fn pose_at(&self, t: f64) -> Pose; }

trait Path   { fn point_at(&self, s: f64) -> Vec3; fn tangent_at(&self, s: f64) -> Vec3; fn len(&self) -> f64; }
//   impls: Line, Arc, Circle, Polyline, Bezier        (curve geometry only; arc-length param)
trait Timing { fn s_at(&self, t: f64) -> f64; }
//   impls: ConstSpeed, Trapezoid (accel/cruise/decel), Dwell, OscillateAlong, PendulumODE
enum Orient  { Fixed(Quat), Tangent /*Frenet*/, Spin { axis: Vec3, rate: f64 } }

struct Placed { path: Box<dyn Path>, timing: Box<dyn Timing>, orient: Orient, place: Isometry3 }
// pose_at(t):  s = timing.s_at(t);  place ⊗ (path.point_at(s), orient(path, s))

// Two distinct compositions:
struct Sum(Vec<Box<dyn Trajectory>>);              // superpose — base ∘ perturbation (oscillation on a pass)
struct Sequence(Vec<(f64, Box<dyn Trajectory>)>);  // concatenate in time, continuity at the joins
```

Factoring orientation into `place` (an SE(3)) makes "in any plane, about any point" free for
every motion — implement each in a canonical frame, place it rigidly. `Path × Timing`
separates *geometry* from *speed*, and the timing law shapes the signal as much as the path
(it is what turns a `Line` into a lift, via an accel/dwell timing). Named conveniences
(`LinearPass`, `Circle`, `Lift`, `Pendulum`, `Oscillation`) wrap the core so the config never
assembles primitives for the common cases.

---

## 5. Data model — the State Bundle

The universal interchange. Every field carries a **leading time axis `T`**, which is
the **measurement-cadence** axis (one entry per `ΔΦ` sample, spaced `cycle_time`). The
within-flight integration grid is internal to the forward model and never surfaces as
`T`.

**Conventions (fixed by fiat — cheaper than a wrong-frame bug).** World axes are
right-handed with **`z` vertical** (the tower axis), gravity along `−z`; **SI throughout**
(m, s, kg), no internal scaling. Quaternions are **scalar-first `wxyz`** in the bundle; an
imported glTF asset (scalar-last `xyzw`) is converted at the import boundary, the only place
the two meet. Physical quantities use **newtype wrappers** (`Metres(f64)`, `Seconds(f64)`,
`Radians(f64)`) so the compiler refuses to add a length to a time. Reproducibility is
`(config, root_seed)` → every tensor, via a **counter-based / splittable RNG** (one stream
per scenario index from the root seed) so a run is independent of `DataLoader` worker count
and scheduling — naive per-worker seeding would make output depend on the parallel layout.

```
StateBundle
  t              (T,)              measurement timestamps                         [s]
  source         SourceState
  field          Option<FieldState>          // computed only if requested
  signal         (T, C)            differential phase + noise — THE observable    [rad]
  meta           RunMeta           resolved config + seed (the repro record)

SourceState
  pose           (T, 7)            COM xyz + quaternion wxyz
  twist          (T, 6)            linear + angular velocity
  accel          (T, 6)            linear + angular acceleration
  cloud_body     (N, 4)            body-frame mass elements (x,y,z,m) — fixed (rigid)
  # DERIVED on request: world cloud (T,N,4); per-voxel v/a (T,N,3)

FieldState  (optional)
  g_clouds       (T, n_clouds, 3)         g vector at each atom cloud
  T_clouds       (T, n_clouds, 3, 3)      gradient tensor at each cloud
  g_grid         Option<(T, X, Y, Z, 3)>      g on a spatial grid
  T_grid         Option<(T, X, Y, Z, 3, 3)>   tensor on a grid
```

**Primary vs derived.** The irreducible state is `cloud_body` (fixed), the per-tick
`pose`/`twist`/`accel`, and the noise realisation. World cloud, velocity field, and
the whole gravity field are *derived* on demand. A run that asks only for `signal`
plus one target never computes the gravity volume — this is what lets "full state
available" coexist with "generate live every batch".

**Field selection.** The SDK selects on two axes: the scenario, and which fields come
back. Selection is a bitset/enum passed into `generate`; unrequested fields are never
allocated or computed.

**Two request modes.** `run` returns one `StateBundle` (`(T, …)`); `stream` yields
them batched to `(B, T, …)` per field. A fixed window (duration) per request gives
uniform `T` and clean batching.

**Persistence (optional).** The bundle serialises tensor-native (safetensors /
WebDataset; Zarr/HDF5 for volumetric fields) for a frozen eval set or a cache. Not in
the training loop.

---

## 6. Component specs

### 6.1 `gravity` — the gravity kernel

The dependency-light inner module; the most-tested code in the project.

**Responsibilities**
- Evaluate `V`, `g`, `T` at a point for: single elements, a voxel cloud, and the
  analytic oracle primitives.
- Reduce a rigid cloud to **multipole moments** once; evaluate far-field cheaply.
- Route near-field to a direct element sum, far-field to the expansion.

**Features**
- **Discretisation elements** (`GravitySource` impls): `PointMass` (v1 default,
  cheapest, singular close in); `SoftSphere` (externally identical to a point,
  softened near/inside via `eps`); `UniformCube` (Nagy prism — exact gravity of a
  uniform box, best near the body). Element type is a config knob, orthogonal to
  everything else.
- **Cloud** (`GravitySource`): a `Vec` of elements. `potential/field` internally
  switch on distance: `|r − com| > far_cutoff` → multipole expansion; else → direct
  sum. Holds cached multipole moments (monopole → quadrupole, `multipole_order`).
- **Multipole reduction**: compute moments from the body-frame cloud once per body;
  re-express in world frame from the pose (rotation only — cheap per tick).
- **Analytic oracle primitives** (`GravitySource` impls, closed-form / quadrature):
  `Sphere`, `Cuboid`, `Cylinder`, `Sheet` — the exact potentials from the report
  (eq 118 sheet, 121 cuboid, 122 cylinder). These are **not** used in generation;
  they are the reference the voxel cloud is validated against (§9).
- **Tensor diagnostics**: `T` is symmetric and trace-free in vacuum — used as a
  free invariant check.

**Layout & numerics** (the kernel is the one place performance bites):
- **SoA, aligned**: store the cloud as separate `xs/ys/zs/ms` arrays in cache-line-aligned
  buffers, so the near-field sum streams sequentially and vectorises (one reciprocal-distance
  per SIMD lane). A fat element (mass + material + temperature) would drag cold bytes through
  every cache line of the hot loop — the minimal cloud (§3.4) is the *cache-optimal* one, not
  just the clean one. Keep the far-field multipole data **array-flattened**, not
  pointer-linked, so traversal stays sequential (random access is ~70% slower).
- **Generic over the scalar type** (`T: Float`) from M0, so a forward-mode autodiff scalar
  (dual numbers) instantiates without a rewrite → gradient-based inversion and Fisher /
  Cramér–Rao identifiability later. M0-or-never.
- **Deterministic reduction**: sum into per-thread, **cache-line-padded** partials combined
  in a fixed order → bit-identical regardless of worker count, *and* free of false sharing
  (independent threads writing one line is measured at several-hundred-percent overhead). One
  decision buys determinism and speed.
- **Conditioning**: `ΔΦ` is a difference of differences of large potentials — difference
  potentials *before* summation (or use compensated summation), or f64 cancellation can eat a
  ≥99% match at large source distance.

**Non-features (v1)**: no GPU kernel (rayon CPU is the default; a compute backend
slots behind `generate` later); no adaptive multipole (FMM) — fixed order is enough
at these N.

---

### 6.2 `source` — source dynamics & bodies

**Responsibilities**
- Import any mesh → voxel cloud (point-in-mesh fill at `voxel_size`).
- Hold the rigid body (body-frame cloud) and evolve its pose via a `Trajectory`.
- Expose the source as a continuous `SourceDynamics` (`pose_at(t)`, `cloud_body()`).

**Features**
- **Voxel import**: mesh → occupancy grid at `voxel_size` → element per filled cell,
  mass `= density · voxel³` (or normalised to a target total mass). Element kind from
  config. Centre-of-mass and multipole moments computed at import.
- **`Rigid` (the v1 `SourceDynamics`)**: `{ cloud: Cloud, trajectory: Box<dyn Trajectory> }`.
  Multipole-reduced once; per tick only the pose changes.
- **Motion model** (`Trajectory`, see §3) — a motion factors into a `Path` (line, arc,
  circle, polyline, bezier), a `Timing` law (constant speed, accel/cruise/decel, dwell,
  oscillate, or ODE-derived), an `Orient` law (fixed / tangent / spin), and a rigid
  `place`ment so any motion sits in any plane at any point. `Sum` superposes (oscillation
  carried along a pass); `Sequence` concatenates in time (a right-angle turn and a lift are
  *sequenced* passes, not new primitives). Named conveniences over the core cover George's
  catalogue: `Static`; `LinearPass` (his Fig 12); `Oscillation` (his Fig 15); `Circular`;
  `Lift` (line + accel/dwell timing — his `lift_excision` source); `Pendulum` (arc + ODE
  timing; large-amplitude → harmonics at $nf_0$; the spherical case is chaotic → the Tier-3
  test). Physical notes: a sharp turn is a velocity discontinuity (an unphysical signal
  transient), so turns default to a fillet arc or a stop-turn; a bezier needs arc-length
  reparameterisation for uniform speed; the `PendulumODE` timing is integrated
  **symplectically** (semi-implicit Euler / leapfrog) so energy — and thus the spectral peaks
  — don't drift over a long run; and the orientation law (fixed vs tangent) matters only
  for non-spherical bodies and is a per-scenario choice.
- **Body library**: a few parameterised primitives as bodies (sphere, cuboid,
  cylinder, sheet) plus mesh load — so a scenario can name `cylinder(R,H,ρ)` directly
  and the same shape exists as an oracle.

**Parked behind `SourceDynamics`**: `Articulated` (piecewise-rigid, per-segment pose),
`Deforming` (per-tick re-reduction), `Fluid` (SPH/grid solver). None touch the gravity
core — they just supply a cloud-per-tick.

**`Articulated` — the walking-human case (parked, but the natural first extension).**
A moving person is a *named* GGN source in the report (Carlton–McCabe modelled people —
as point masses), so the articulated treatment is a refinement past the published work.
It needs **no new gravity physics**: an articulated body is a small set of **rigid
segments** (head, trunk, upper/lower arms, thighs, shanks, feet — anthropometric
ellipsoids/cylinders, masses and lengths from height + total mass via standard
Winter/Dempster tables) whose **relative poses change per tick**. Each segment is rigid
→ reduced once; per tick the world cloud is the union of segment clouds, summed by the
same kernel (a union of clouds is still a cloud). The motion **factorises**: the
existing `Trajectory` seam walks the **root** across the lab; a new `Gait` provides
joint angles over the walk cycle and forward kinematics places the limbs relative to
the root —

```rust
trait Gait { fn joint_angles(&self, t: f64) -> JointAngles; }  // parametric, or mocap (BVH/CMU)
struct Articulated { skeleton: Skeleton, gait: Box<dyn Gait>, root: Box<dyn Trajectory> }
```

Crucially this is **kinematics, not dynamics** — gravity needs only where the mass is,
so plausible joint-angle trajectories suffice; no biomechanical force simulation.
**Validation is near-free**: articulation is superposition of already-validated rigid
segments, so no new oracle — only the gait's plausibility is a modelling choice. (Open
question the sim answers: far-field collapses a person to a point mass beyond a few
metres, so limb-swing only adds a distinguishable signature close in — the sim
quantifies where. The per-voxel velocity field also stops being rigid-body-trivial here
and starts carrying the gait.)

---

### 6.3 `instrument` — gradiometer & forward model

**Responsibilities**
- Hold the AION-10 gradiometer geometry and construct the four arm trajectories.
- Implement the `PhaseModel` (the propagation-phase integral) and the measurement
  sampling.

**Features**
- **`Instrument`** struct: baseline `Δr`, launch velocity, `T`, `cycle_time` (`Δt`),
  `n_kicks` (→ `velocity_boost`), atom species (`m_atom`, `wavelength`), `g`, `fine_dt`.
  Defaults = AION-10 (§7).
- **Arm construction**: builds the four parabolic free-fall arms `z_{upper,lower}` for
  the two interferometers (launch heights `0` and `Δr`), with the LMT kicks at `t=0`
  and `t=T` and floor reflection at launch height — exactly George's
  `lower_arm`/`upper_arm`. Computed once; cached as interpolable trajectories over
  `[0, 2T]`.
- **`PropagationIntegral` (`PhaseModel`)**: for each interferometer, integrate the
  source potential difference between its two arms over the flight; difference the two
  interferometers (§7 for the exact algorithm). Configurable quadrature (fixed-step
  trapezoid/Simpson on `fine_dt`, or adaptive) — must reproduce the oracle tightly.
- **`QuasiStaticGradient` (`PhaseModel`, alt)**: `kₑ𝒻𝒻·a·T²` from the tensor at the
  cloud — the uniform-field limit, kept only as a sanity check, never the default.
- **Sampling & Nyquist**: the signal is sampled at `cycle_time`; `f_Nyq = 1/(2·Δt)`.
  The instrument knows its cadence so downstream periodograms (and the real spectral
  mirroring) are correct.

**The two time scales (both real, both here)**: `fine_dt` (≈0.01 s) integrates each
`δφ` over the flight `2T`; `cycle_time` (≈2 s) is the cadence at which `ΔΦ_ℓ` is
sampled. Distinct axes — don't conflate.

---

### 6.4 `noise` — the noise stack

**Responsibilities**: add realism to the clean signal, as an ordered, seedable stack.

**Features**
- **`ShotNoise { n_atoms, contrast }`** — atom-shot-noise phase scatter
  (`σ ≈ 1/(C·√N_atoms)`), per measurement.
- **`VibrationResidual { psd, rejection }`** — vibration phase with a gradiometer
  common-mode rejection ratio (the residual after differencing).
- **Stack** = `Vec<Box<dyn NoiseSource>>`, applied in order to the clean `(T, C)`;
  one RNG, seeded from config.
- v1 may run with an **empty stack** — the forward model validates noiseless against
  the oracle, then noise is layered on. Noise is additive realism, not a blocker.

---

### 6.5 `scenario` & prior

**Responsibilities**: tie body + dynamics + instrument + noise + schedule into one
runnable unit, and sample randomised scenarios for `stream`.

**Features**
- **`Scenario`** = `{ source: Box<dyn SourceDynamics>, instrument, noise_stack,
  duration, fields }`. The unit a single `run` evaluates.
- **`Prior`** — distributions over every scenario parameter (which body, mass,
  distance/closest-approach, speed, frequency, orientation, phase, noise levels).
  Sampling a prior yields a `Scenario`. **The prior is the experiment.**
- **Resolution coupling**: the prior's closest-approach distribution sets `voxel_size`
  and `multipole_order` (validated in §6.6). Realistic envelope (masses/distances/
  frequencies of actual AION sources) is a prior-tuning input, not a blocker — George
  can advise, but defaults come from his catalogue (walls, scaffold, lifts, people).

---

### 6.6 `config` — the one typed schema

A single typed config in `sim-core`, exposed via PyO3, consumed by **both** front-ends
(SDK as dataclasses, CLI as TOML/YAML + flag overrides). No parameter list duplicated.
Grouped, every knob defaulted. **Full schema in §8.**

- Resolved `config + seed` fully determines a run — the reproducibility record.
- **Validation lives here**: `voxel_size` vs closest-approach and the noise floor (the
  M0 convergence tolerance); `fine_dt` vs `T`; `cycle_time ≥ 2T`; `far_cutoff` vs the
  prior's near regime.

---

### 6.7 `sdk` — Python bindings + CLI

One package, **maturin** over **PyO3** to `sim-core`. No physics in Python — it
marshals and parameterises the Rust call.

**Features**
- **`run(mass=…, motion=…, duration=…, instrument=…, fields=…, seed=…) → StateBundle`**
  — one explicit scenario; torch-ready tensors `(T, …)`. For debugging, eval sets, the
  viewer, CLI one-shots.
- **`stream(prior, fields=…, batch=…) → IterableDataset`** — endless randomised
  scenarios; each item a full trajectory; the loop pulls `(B, T, …)`.
- **GPU-feeding shape**: the Rust generator runs in `DataLoader` workers (separate
  processes, no GIL), **releases the GIL** during compute, and **prefetches** so the
  next batch builds while the current trains. Generation speed is the thing to watch,
  not storage.
- **Derived outputs**: the **periodogram** (PSD per the report's eq 66) as a first-class
  derived field — the natural analysis space, and the likely model input. `sdk.view(…)`
  pushes a state to the viewer.
- **Config**: dataclasses mirroring the typed schema; `**overrides` on `run`.
- **CLI** (`gradsim run …` / `gradsim stream … --out cache/`): a console entry point in
  `pyproject.toml` calling the same functions — for deliberately frozen bundles
  (shared eval sets, reproducibility, caching). Same code, command-line door.

**Hardware target**: M3 MacBook (MPS) as primary, an RTX 2080 Super and George's GPU
(CUDA) secondary. Tensors are torch on whichever device; **no datacentre/A100
assumptions** anywhere.

---

### 6.8 `viewer` — debug/inspection (separate, lightweight)

Not the main mode. A separate crate rendering a **state** (§5), **source-agnostic**:
it does not care whether the state is simulator truth or model prediction.

**Features**
- Stack: **eframe/egui** + `egui_plot` (signal/periodogram traces) + a small **wgpu**
  3D viewport (baseline + atom clouds + the source body on its trajectory; optional
  tidal-field colour slice). Native, cross-platform (macOS/Windows/Linux).
- **Prediction overlay**: `sdk.view(truth, overlay=prediction)` draws a model state
  against truth through the same path — ghost body beside solid, predicted field
  beside true. Designed in now; the model that fills it is deferred.

Most "plotting" also comes free from SDK tensors + matplotlib; the viewer earns its
place for interactive 3D sanity-checking and the overlay.

---

## 7. The forward model, pinned

The canonical reference, taken from George's `scaffold` script and report eq 62–65.
This is what M0/M1 must reproduce to ≥99% (point-mass cases tighter).

**Constants — AION-10 defaults**
```
G              = 6.67430e-11
launch_velocity= 3.86         m/s          (u_initial)
m_atom         = 1.46e-25     kg           (Sr-87)
hbar           = 1.055e-34
wavelength     = 698e-9       m            → wavenumber k = 2π/λ
T              = 0.73         s            (flight 2T = 1.46 s)
n_kicks        = 1000         → velocity_boost = n_kicks·ħ·k/m_atom  ≈ 6.5 m/s
g              = 9.81         m/s²
baseline Δr    = 5            m            (lower IFO launches z=0, upper z=5)
cycle_time Δt  = 2            s            (measurement cadence)
fine_dt        = 0.01         s            (within-flight integration step)
```

**Arm trajectory construction** (the four arms, per George)
- Each interferometer has a **lower** and an **upper** arm; the two interferometers
  launch at `z=0` and `z=Δr`.
- Integrate ballistic free-fall under `−g` from launch velocity, with a **floor** at
  the launch height (atom has landed → clamp, stop). Step `fine_dt`.
- **Upper arm**: launch with `+velocity_boost`, segment `[0,T]`; at `t=T` apply
  `−velocity_boost`, segment `[T,2T]`.
- **Lower arm**: launch at `launch_velocity`, segment `[0,T]`; at `t=T` apply
  `+velocity_boost`, segment `[T,2T]`.
- The two momentum swaps at `t=T` are the π-pulse; the arms separate then reconverge.
- Arms depend only on the instrument → built **once**, cached as interpolable paths
  over `[0, 2T]`.

**Phase integral** (eq 64–65)
```
for each measurement ℓ at t_ℓ  (spaced Δt):
    for each interferometer j in {lower(z0=0), upper(z0=Δr)}:
        δφ_j = (m_atom/ħ) · ∫_{t_ℓ−2T}^{t_ℓ} [ V(arm_upper_j(t), t) − V(arm_lower_j(t), t) ] dt
    ΔΦ_ℓ = δφ_lower − δφ_upper        # sign is convention
```
- `V(r,t)` = source potential at world point `r` at time `t`. Voxel cloud:
  `V = −G · Σ_i m_i / |r − r_i(t)|`. The source moves between **and during** the flight,
  so `r_i(t)` is queried at `fine_dt` resolution via `SourceDynamics::pose_at`.
- Quadrature on the inner integral: fixed-step (trapezoid/Simpson) at `fine_dt`, or
  adaptive — chosen to match the oracle. (George uses `scipy.quad` over arm/`V`
  interpolants; for extended bodies he precomputes `V` on an `(α, z)` grid and
  `RectBivariateSpline`-interpolates. We may mirror or integrate directly.)

**Sampling**: `ΔΦ_ℓ` over `ℓ` is the signal at cadence `Δt`; `f_Nyq = 1/(2Δt)`.

---

## 8. Configuration schema (full)

One schema, grouped, every field defaulted.

**discretisation** — `voxel_size`, `element` ∈ {point, sphere, cube},
`multipole_order`, `softening` (sphere eps), `far_cutoff` (multipole switch distance).

**instrument** — `baseline` (Δr, 5 m), `launch_velocity` (3.86), `T` (0.73),
`cycle_time` (Δt, 2 s), `n_kicks` (1000), `species` (Sr-87 → m_atom, wavelength),
`g` (9.81), `fine_dt` (0.01), `n_clouds` (2), `phase_model` ∈ {propagation, quasistatic}.

**noise** — ordered stack with per-source params (`shot{n_atoms, contrast}`,
`vibration{psd, rejection}`, …), `seed`.

**body / motion** — body: library primitive (`sphere/cuboid/cylinder/sheet` params)
or mesh path, `density` / target mass; motion: `Trajectory` variant + params
(`linear{r0,v}`, `oscillation{axis,A,f,phase}`, `circular{centre,radius,f,plane}`,
`pendulum{…}`, `composite[…]`).

**duration / schedule** — `duration` (window length) → `T_meas = duration/Δt`.

**prior** (stream) — distributions over all of the above.

**fields** — which fields to compute/return (`signal`, `source`, `field@clouds`,
`field@grid`, `periodogram`, …) + grid resolution for volumetric ones.

---

## 9. Validation & test strategy

George's full-simulation code is the **golden master**. Freeze a sweep of
`(scenario → outputs)` as committed fixtures — no Python in CI — and assert against
them with **tiered** tolerance, not one flat threshold.

- **Tier 1 — point-mass forward model (tight).** Identical maths on the same
  trajectories; a 1% gap is a bug, not tolerance. Anchor cases: the 10 kg / D=10 m /
  2.5 m/s vertical pass (~7 mrad peak, Fig 12); the 1 kg / D₀=5 m oscillator (~2 mrad,
  Fig 15). Metric: relative error of the series, `1 − ‖sim−oracle‖₂/‖oracle‖₂ ≥ 0.99`, and
  `max|ΔΦ|` within 1% (not "≥99%" of an unstated quantity).
- **Tier 2 — voxel convergence (the key assertion).** Voxelise a primitive and show
  it **converges** to the analytic oracle as `voxel_size → 0`: e.g. the steel column
  (R=0.0338 m) vs George's `cylinder_potential` (`nquad`) at a set of field points, and
  the resulting `ΔΦ` to ≥99% by the same relative-L₂ metric, with the per-point potential
  error decreasing monotonically in resolution. *Convergence is the proof the voxel approach
  is correct.*
- **Tier 3 — ODE motion (feature-level).** The chaotic 3D pendulum can't match
  pointwise at long horizons; the metric is **spectral** — periodogram peak positions within
  one frequency bin, peak-height ratios within a stated tolerance. Pointwise agreement is
  explicitly not required.
- **Localisation**: where the oracle exposes intermediates (potential at a point,
  per-interferometer `δφ` before differencing), test those too, so a failure points at
  a module.
- **Invariants (free)**: gradient tensor symmetric + trace-free in vacuum; signal
  scales linearly with source mass; far-field cloud → its own monopole.
- **Fixture hygiene**: capture George's numerical knobs (`fine_dt`, quadrature/`nquad`
  tolerances, grid sizes) alongside his outputs — some of the ≥99% residual is *his*
  discretisation, so compare like with like.

Compare against his **full**-simulation outputs, never his perturbative closed forms —
our full simulation beats those.

---

## 10. Milestones (test-grounded)

- **M0 — gravity kernel + oracle.** Voxel cloud, element types, multipole evaluator,
  analytic primitives. Exit: voxelised sphere/cylinder → analytic value as resolution/
  order rise (Tier 2); tensor invariants hold (Tier-? invariants). **No dependency on
  George's runtime** — buildable today.
- **M1 — instrument + forward model + first signal.** Arm construction + propagation
  integral; reproduce the frozen point-mass fixtures tight (Tier 1) and the
  extended-body convergence (Tier 2). Exit: the 10 kg pass at ~7 mrad, asserted.
- **M2 — scenario + prior + state bundle.** Assemble the full `StateBundle`; freeze
  the §5 field set and shapes; field selection works (unrequested → uncomputed).
- **M3 — SDK (+ CLI).** `run`/`stream` yield torch tensors; CLI wraps them; a
  `DataLoader` with prefetch overlaps a dummy training step without starving it
  (GIL released, workers parallel). Periodogram derived output.
- **M4 — viewer.** egui/wgpu: signal/periodogram plot + 3D scene; built when needed.
- **M5+ (parked)** — noise realism depth, non-rigid dynamics, ULDM injection, the model.

---

## 11. Open decisions (your call)

1. **Inner-integral quadrature.** Mirror George (interpolate arms + `V`, `scipy.quad`)
   for max fidelity, or integrate directly with fixed-step Simpson on `fine_dt`?
   Affects how cleanly Tier-1 hits ≥99% and how fast generation is. *Lean: direct
   fixed-step, validate against his, fall back to adaptive only if needed.*
2. **Extended-body `V` evaluation in generation.** Always direct element sum, or adopt
   George's precompute-grid-+-spline trick for moving extended bodies (much faster when
   the same body is queried many times per flight)? *Lean: direct sum in v1 (simpler,
   exact); add a cached-grid path if profiling demands.*
3. **Model input representation.** Does the eventual model consume raw `ΔΦ(t)`, the
   periodogram, or both? This decides which derived fields the SDK must emit as
   first-class. *Doesn't block the engine, but settles what M3 ships. George works in
   the frequency domain → periodogram is likely load-bearing.*
4. **Per-measurement source pose convention.** In `SourceState`, report the pose at
   `t_ℓ` (the measurement instant) or at the flight midpoint `t_ℓ − T`? Minor, but pick
   one and document it.
5. **Body/motion naming in config.** Flat enums (as in §8) vs a small composable DSL
   for `Composite` trajectories (needed for the scaffold's many members). *Lean: flat
   for v1, DSL when the scaffold lands.*

---

## Appendix A — Parked source dynamics: the extension sketch

Everything past `Rigid` — articulated bodies, fluids (compressible & incompressible),
structural vibration, airflow/convection — is addable **without touching the gravity
core, the forward model, or the state contract**, because they all reduce to one
currency: a **mass-density field ρ(x,t)**, discretised to a cloud per tick. What differs
is only *how each produces ρ(x,t)* — and that splits every extension into two families
with very different costs.

### A.1 The distinction that organises everything: prescribed vs solved

- **Prescribed (Category A)** — ρ(x,t) is a known function of parameters; you *evaluate*
  it. Rigid, Articulated, analytic sloshing, a parameterised plume, modal vibration with
  precomputed modes. Cheap, stateless per tick, embarrassingly parallel → fits the
  live-generation path.
- **Solved (Category B)** — ρ(x,t) is the *output of integrating a physical system
  forward in time* (a PDE / particle sim). Free-surface CFD, elastodynamics, convection.
  Stateful (t+dt depends on t), solver-sized per scenario, **too slow to run inside the
  training `DataLoader`** (and on the actual hardware — M3 / RTX 2080, no datacentre —
  not close) → wants precompute-and-replay (A.3). Loses George's code as an oracle (A.5).

Design goal: make the **prescribed** version of each phenomenon a first-class drop-in,
and treat **solvers** as pluggable backends behind the same interface, solved once and
cached.

### A.2 The seam, generalised + made composable

The general `SourceDynamics` method is the *whole cloud at time t* — `Rigid`'s
`{cloud_body, pose_at}` is a specialisation of it (fixed cloud, multipole-reduced once);
non-rigid sources return a time-varying cloud directly.

```rust
trait SourceDynamics {
    fn cloud_at(&self, t: f64) -> Cloud;   // discretised ρ(x,t): grid cells OR SPH particles, each a mass element
}
// Rigid:        cloud_at(t) = pose_at(t) ⊗ cloud_body()   (the multipole-once path)
// Non-rigid:    returns the time-varying cloud directly

// Sources superpose — gravity is linear, so a scene's ρ is the sum of contributions.
struct Composite(Vec<Box<dyn SourceDynamics>>);          // sum the clouds

// One-way coupling: source A emits a forcing that drives source B (footfall → floor modes).
trait Driver { fn forcing(&self, t: f64) -> Forcing; }   // A produces, B consumes

// The solver integration point: replay a stored ρ(x,t) trajectory as a cheap source.
struct CachedField { /* stored ρ(x,t) time series */ }   // impl SourceDynamics by replay/interp
```

`Composite` is the general "several independent moving things" — and is what the
**scaffold** needs in v1 (many cylinders, each oscillating), so it earns its place early.
`Driver` couples a walking person to a floor (A.4) without the two being monolithic.
`CachedField` is where every Category-B backend plugs in. The gravity core still only
ever sees a cloud.

#### A.2.1 The two representations — keep the gravity cloud minimal

The single most expensive thing to get wrong. There are **two** voxel representations and
they must stay separate:

- **The gravity input** (what the kernel consumes) is `Cloud` — elements of
  `(position, mass)` and at most an element-shape tag, *nothing else*. This is the
  universal currency; it stays minimal forever, and the kernel never learns about material,
  temperature, velocity, pressure or phase.
- **The solver's own state** (what each backend evolves) is where material type,
  temperature, velocity, pressure and compressibility live — a convection solver carries a
  temperature/velocity/density grid; a rigid body carries a pose; a vibration solver carries
  modal amplitudes. Each backend holds whatever rich per-cell state its physics needs and
  **emits a minimal `Cloud`** as its output via `cloud_at(t)`.

So `fluid_compressible` / `fluid_incompressible` / `solid` are the *solver's* cell types
(Category B), **not** new fields on the gravity voxel. The rule: rich per-voxel properties
belong to the source backend, behind the seam; the cloud handed to gravity is always just
mass and position. A single universal mega-voxel carrying
`{mass, material, temperature, velocity, …}` would make the kernel depend on the union of
every physical property, so adding any new physics would touch the core — destroying the
"ρ is the only currency" decoupling that the whole architecture rests on. This boundary is
the one v1 decision that is genuinely expensive to change later, so it is the one to fix now;
everything behind it (fluid solver, temperature field, skinning) can land years later
without touching the core.

### A.3 Generation-mode consequence: cache-and-replay for solved sources

Category-A sources run live in the `DataLoader` workers. Category-B solvers cannot.
So they integrate via `CachedField`: solve a **modest library** of fields offline (a few
sloshing runs, plume runs, convection runs), cache the ρ(x,t) time series, and the live
path **places, time-shifts, scales, and superposes** cached fields as cheap sources.
This is the only place the "generate live every batch" model bends — by design, not by
accident. (Caching is not an optimisation here; on the available hardware it is the only
way Category B is tractable at training scale.)

### A.4 Per-phenomenon fidelity ladders

**Articulated bodies** — see §6.2. Prescribed kinematics (gait → forward kinematics);
Category A. Concretely this is **consuming a rigged asset, not building an animation
engine**: import a rigged glTF (skeleton + skin weights + clips), sample a clip at time `t`
(SLERP the keyframes), and obtain the bone transforms. The engine work — rigging, authoring
animations, IK, blending, rendering — happens upstream in Blender/Mixamo and arrives baked
in; the project only needs glTF import + clip sampling + linear-blend skinning (~tens of
lines) + voxelisation. Pre-made walking clips (Mixamo, the CMU mocap database, any BVH/glTF
humanoid) are therefore a direct shortcut. Efficiency: skinning is *almost* piecewise-rigid
(each vertex follows its dominant bone, blending only at the joints), and gravity at
metre-scale distances cannot resolve cm-scale joint blending, so **voxelise each bone once
in bone-local space and apply its transform per frame** (a rigid transform of a fixed cloud;
the per-bone multipole reduction stays valid) rather than re-voxelising the deformed mesh
every frame. Re-voxelising every frame is the brute-force path — exactly the parked
`Deforming` source — reserved for genuinely squishy bodies a walking human does not need.

**Fluids — incompressible.** The signal comes from a **moving interface**, not the bulk:
a fully-filled rigid container of uniform incompressible fluid is gravitationally
*static* → no signal. What matters is a free surface / density interface moving
(sloshing, waves), or advection of pre-existing density inhomogeneities. Ladder:
- *Prescribed (A):* small-amplitude **sloshing modes** — the free surface as a sum of the
  tank's natural modes oscillating at known frequencies (closed-form for rectangular /
  cylindrical tanks). Cheap; checkable against the analytic modes.
- *Solved (B):* free-surface CFD (SPH, or level-set / VOF grid) for large amplitude /
  breaking. Pressure-projection enforces ∇·v = 0; density advects.

**Fluids — compressible.** Density itself is dynamical (compression/rarefaction), so the
bulk density field moves mass directly — continuity `∂ρ/∂t + ∇·(ρv) = 0` plus an equation
of state. This is the air case (convection, below). Prescribed: advecting density
anomalies. Solved: compressible / low-Mach Navier–Stokes.

**Structural vibration (footfall → floor).** George models walls/scaffold as **rigid
coherent oscillation** (a whole wall at 0.1 Hz, 1 µm) and explicitly flags it as an
*overestimate* — the real thing is a *spatially-varying* vibration field. The refinement
is **modal**:
- A structure has eigenmodes φₙ(x) at frequencies ωₙ; displacement u(x,t) = Σ aₙ(t) φₙ(x);
  the mass redistribution δρ = −∇·(ρ₀ u) is **linear in the modal amplitudes**.
- Modes are precomputed **once** (FE eigenproblem) — or **analytic** for simple shapes (a
  simply-supported rectangular floor-plate has φ_mn = sin(mπx/Lx)·sin(nπy/Ly) in closed
  form, no solver). At runtime: prescribed-shape × time-amplitude → Category A.
- Excitation: a **footfall** is a point force impulse; projecting it onto the modes rings
  them. A walking person (the articulated source) is a `Driver` emitting footfall forces
  at its stepping points/times; the floor source consumes them. So *person + floor* is a
  `Composite` of two sources with one-way coupling — and **both** the body mass and the
  vibrating-floor mass contribute to the signal. The realistic refinement George flags,
  expressed as composition.
- Ladder: F0 rigid coherent (already supported — `Rigid` + `Oscillation`); F1 modal
  (precomputed/analytic modes + footfall forcing); F2 full elastodynamics (solver, rarely
  needed).

**Airflow / convection.** Air-density perturbations move mass; weak (ρ_air ≈ 1.2 kg/m³,
~1000× lighter than water) but a *named* AION source (atmospheric GGN, Carlton et al.
2025). The hardest family:
- *Prescribed (A):* a **thermal plume** as a buoyantly-rising parameterised density
  anomaly (advecting Gaussian); HVAC / draft as a prescribed advecting/oscillating density
  field. Cheap, plausible first cut.
- *Solved (B):* **Boussinesq convection** — Navier–Stokes with buoyancy + a temperature
  field (Rayleigh–Bénard is canonical). Genuinely a CFD sub-project, **turbulent and
  chaotic**, so the signal is a **weak broadband stochastic field** — exactly the regime
  where notch filters fail and George flags learned regression as the way forward.
  Scientifically the strongest motivation for the ML, and the deep end: validating a
  convection solver is a discipline of its own.

### A.5 Validation reality

- **Prescribed sources** inherit correctness from the validated gravity kernel (a fluid
  cloud / vibrating cloud is still a cloud the kernel already handles); the *new* thing is
  the prescribed field's plausibility — a modelling choice — and analytic cases (sloshing
  modes, plate modes) are checkable against closed forms.
- **Solved sources lose George's code as an oracle entirely** (he used rigid coherent
  motion). Validation moves to the solver's *own* standard benchmarks (lid-driven cavity,
  Rayleigh–Bénard, analytic beam/plate response) — real work, a separate track. The main
  reason solvers are deep-parked.

### A.6 What to put in v1 now (so these are drop-ins, not rewrites)

Four small seams — none is the phenomenon itself:
1. **`Composite` `SourceDynamics`** (sum of clouds) — needed for the scaffold anyway; the
   hook for "person + floor + air".
2. **`cloud_at(t)` accepting grid-cell *or* particle clouds** (a density grid and SPH both
   reduce to "elements with mass") — so no solver backend is ever blocked by representation.
3. **`CachedField`** (replay a stored ρ(x,t)) reserved as the solver integration point — a
   thin replay/interp source.
4. **`Driver`** hook on sources for one-way coupling (footfall → modes).

These four turn each phenomenon into an additive backend rather than a core change.
