# STALE — SUPERSEDED BY `cavendish.tex`

> **THIS DOCUMENT IS OUT OF DATE. DO NOT DESIGN OR IMPLEMENT FROM IT.**
>
> The authoritative, maintained specification is now **`cavendish-spec/cavendish.tex`**
> (v0.5, 25 Jun 2026), whose §3 (System Architecture) has absorbed and superseded this
> document. This is the older Penumbra-era architecture note and predates the
> gravitational-radar reframing, the detector array, the ULDM channel, the measurement
> schedule, the CPU/GPU compute split, and the identifiability ladder.
>
> Retained for history only.

---

# Gravitational Gradiometer Simulator — Architecture

## 1. Shape

The **simulator is the product**, and it runs **live, as a data source for
training** — not a dataset you precompute. The model asks for a batch, the
simulator runs the requested scenarios, and returns the **time series over their
ticks**. No dataset on disk in the main loop; disk is at most an optional cache.

```
                         ┌─→ training loop ─→ [ML model] ─┐ prediction (state)
   Simulator ─→ SDK+CLI ─┼─→ (optional) cache              │
   live generator        └─→ Viewer ◄─────────────────────-┘
                             renders states: ground truth + predicted overlay
```

The simulator is a **Rust library**. The **Python SDK** is its control surface —
you say what to simulate and receive trajectories of tensors — and the **CLI is just
the SDK invoked from the command line**, the same package, not a separate binary. A
separate lightweight **viewer** renders *states*: ground truth from the simulator
now, and — since both speak the same state schema (§2) — predictions from the model
overlaid, eventually. The simulator depends on none of them.

The model that trains on the stream is **out of scope here**, but two interfaces to
it are designed in now: the SDK's live streaming, and the viewer's source-agnostic
state contract so a prediction can be rendered against truth the moment a model
exists.

## 2. What a request returns

A call simulates a scenario **over its requested duration** and returns the full
state as a **time series over its ticks** — every field carries a time axis `(T, …)`,
in memory, as tensors. The unit of a request is a whole **trajectory**, not a single
tick. The simulation has the complete state at every tick and returns all of it (or
the subset you ask for); the consumer decides what is input and what is target, and
the interferometer signal is **one channel** of it.

Per tick (each field below carries a leading time axis):

- **Source kinematics** — COM position, orientation, linear + angular velocity, acceleration. `(T, k)`
- **Mass density** — the voxel cloud. Rigid: a fixed body-frame cloud `(N, 4)` placed by the per-tick pose `(T, 7)`.
- **Velocity / acceleration field** — per-voxel v and a, derived from pose + v + ω. `(T, N, 3)`
- **Gravitational field** — acceleration g and gradient tensor T, at the atom clouds
  `(T, n_clouds, 3, 3)` or on a spatial grid `(T, X, Y, Z, 3, 3)`.
- **Instrument** — atom cloud trajectories, per-cloud phase.
- **Interferometer signal** — the differential phase with noise: the observable. `(T, C)`

`run` returns one trajectory; `stream` yields these batched to `(B, T, …)` per
field. Time is a feature axis, not a streaming dimension — ticks aren't independent
samples. (A genuine tick-by-tick stream is a separate online/real-time mode, §4.)

This bundle is a **universal interchange**, not just the simulator's output: the
simulator produces it as ground truth, the model (eventually) produces it as a
prediction, and the viewer renders it without caring which. That shared contract is
what makes overlaying a prediction on truth a single render path (§6).

**You choose what gets computed.** The SDK selects on two axes — the scenario (which
mass distribution, which motion, how long, which instrument) and which fields come
back. A run that needs only the signal plus one target field never computes the
gravity volume. This is what lets "full state available" coexist with "generate live
every batch": only what you request is computed.

**Primary vs derived (internal).** The irreducible state is the fixed body-frame
voxel cloud (rigid, so only the pose changes per tick), the per-tick pose +
kinematics, and the noise realisation; world positions, the velocity field and the
gravity field are derived from it on demand. Keeps per-call cost low.

**Persistence is optional.** For a frozen dataset — a reproducible eval set, or a
cache when generation is the bottleneck — the same state bundle serialises to a
tensor-native container per scenario (safetensors / WebDataset, or Zarr/HDF5 for
volumetric fields). Not part of the training loop.

## 3. Simulator core (Rust library)

The live generator. Internally:

**Forward model** (settled internals; per G. Parish's AION GGN treatment). A body is
imported by **voxelisation** (any mesh → a cloud of mass elements; the element is
swappable, below). The phase is the **propagation-phase double-difference integral**,
not the uniform-field shortcut: for each interferometer, integrate the perturbing
potential difference between its two arms over the full Mach–Zehnder sequence, then
difference the two interferometers —

    δφ_ℓ = (m_A/ℏ) ∫[V(z_u,t) − V(z_l,t)] dt ,   ΔΦ_ℓ = δφ_ℓ,2 − δφ_ℓ,1

evaluated along the four parabolic free-fall trajectories. The voxel cloud enters as
the potential, `V(r,t) = −G Σ_i m_i / |r − r_i(t)|`. The closed-form `kₑ𝒻𝒻·a·T²` form
is only the uniform-field limit and serves as a sanity check; GGN sources are
non-uniform, so the integral is the model. **Oracle — the reference implementation, frozen.** George's GGN code is the golden
master: freeze a sweep of `(scenario → ΔΦ)` pairs as committed fixtures (no Python in
CI) and test against them with **tiered** tolerance, not one flat threshold. The
point-mass forward model is identical maths on the same trajectories → it must match
*tight* (a 1% gap is a bug, not tolerance). Voxelised extended bodies must *converge*
to his analytic sheet/cuboid/cylinder integrals as resolution rises (the convergence
is the assertion). ODE-driven motion (the chaotic 3D pendulum) is compared on
periodogram features — peak positions, height ratios — not pointwise, at long
horizons. Where his code exposes intermediate quantities (potential at a point,
per-interferometer phase before the difference), test those too to localise bugs.
Compare against his *full*-simulation outputs, never his perturbative closed forms —
your full simulation beats those. A pluggable **noise stack** (shot noise, vibration
residual) is added. The pure gravity kernel is a dependency-light inner module, the
most-tested part of the code.

Two time scales, both real (and matching the config split below): the **fine
within-flight step** that integrates each δφ over the sequence 2T = 1.46 s, and the
**measurement cadence** Δt = 2 s at which the signal ΔΦ_ℓ is sampled — Δt sets the
Nyquist frequency and produces real spectral mirroring.

**Discretisation element (swappable).** What a voxel *is* — the elemental mass unit
summed to get a body's tensor — is a strategy behind the same `GravityGradient`
trait as the oracle. The body's tensor is the sum over elements regardless of type,
so the element is selectable, trading per-element cost against near-field fidelity:
**points** (cheapest, far-field-justified, singular close in — **v1 default**),
**spheres** (externally identical to points but softened near/inside), **cubes/prisms**
(the *physically exact* voxel — summing Nagy prism tensors is the exact gravity of
the voxelised density; best near the body, costlier per element but fewer needed).
The choice is orthogonal to source dynamics and to the multipole far-field path —
near-field direct sums use the element type, the far-field expansion washes it out. v1
ships points; cubes/spheres drop in behind the trait. (A cube-cloud, exact for the
voxelised geometry, is itself an oracle the point-cloud can be checked against.)

**Source dynamics — what moves the mass (v1: rigid only).** Gravity is agnostic to a
body's constitutive physics: at each tick it needs only the mass configuration
ρ(x,t), not how the body holds together. So the **point-mass cloud is the universal
interface** between any dynamics model and the gravity core, not merely a mesh-import
format. **v1 implements one source-dynamics model: `Rigid`** — a fixed body-frame
cloud moved by an evolving pose, multipole-reduced **once**. The other models
(articulated, deforming, fluid) are **parked**; the cloud-per-tick boundary is kept
as a `SourceDynamics` seam (one variant for now) so they slot in later without
touching the gravity core or the data contract. Because gravity reads only ρ, the
velocity field is derived from pose + v + ω and is optional *output*, never a signal
input.

**Scenario + prior.** A `Scenario` ties a body, a source-dynamics model, an
instrument and a noise stack to a tick schedule. A **prior** samples scenarios for
`stream` mode — which objects, distances, speeds, orientations, noise levels. The
prior *is* the experiment; its closest-approach distribution sets voxel resolution
and multipole order.

**Generation.** Evaluating a scenario yields the per-tick state. It's embarrassingly
parallel across independent scenarios, and the multipole reduction keeps per-sample
cost low — the property that matters, because **generation must keep the GPU fed**
(§4). CPU + rayon is the default; a GPU compute backend slots in behind the same
generation call if throughput demands, an internal detail either way.

## 4. Python SDK + CLI (the primary interface)

One Python package, built with **maturin** over **PyO3** bindings to `sim-core`.
Importable as a library and runnable as a command — the CLI is a thin console
entry point (declared in `pyproject.toml`) that parses args and calls the same SDK
functions. No physics in Python; it only marshals and parameterises the Rust call.

Two modes:

- **`run(mass=…, motion=…, duration=…, instrument=…, fields=…, seed=…)`** — simulate
  exactly the scenario you specify; returns one full trajectory, `(T, …)` per field.
  Explicit path: debugging, eval sets, the viewer, and the CLI's one-shot generation.
- **`stream(prior, fields=…, batch=…)`** — an endless `IterableDataset` of randomised
  scenarios from the prior; each item is a complete trajectory, and the loop pulls
  batches `(B, T, …)` per field. A fixed duration (window) per request gives uniform
  T and clean batching; variable-length needs padding or a custom collate.

Both return torch-ready tensors and select which fields are computed. (Genuine
tick-by-tick streaming — for real-time inference on a live instrument as ticks
arrive — would be a separate mode; it is not the training-data path.)

**Feeding the loop without starving the GPU** is the design constraint of live
generation. The shape: the Rust generator runs in `DataLoader` workers (separate
processes, no GIL contention), **releases the GIL** during the compute, and
**prefetches** so the next batch generates while the current one trains. Generation
speed — not storage — is the thing to watch.

The CLI (`gradsim run …` / `gradsim stream … --out cache/`) is for when you
deliberately want a frozen bundle — a shared eval set, reproducibility, caching
expensive scenarios. Same code as the SDK, just a command-line door.

**Config surface — every knob, one schema.** All parameters are a single typed
config defined once in `sim-core` (exposed through PyO3) and consumed by both
front-ends — the SDK as dataclasses, the CLI as a TOML/YAML file with flag
overrides — so no parameter list is duplicated. Grouped by concern, each with a
default so you override only what you touch:

- **discretisation** — `voxel_size`, `element` (point/sphere/cube), `dt` (timestep),
  `duration` (→ T = duration/dt), `multipole_order`, softening, far-field cutoff.
- **instrument** — `n_clouds`, `baseline` (Δr, default 5 m / 10 m tower), `k_eff`,
  `lmt`, `T` (default 0.73 s), `cycle_time` (Δt, default 2 s), launch velocity
  (default 3.86 m/s), atom/species (default Sr-87), `contrast`, `n_atoms`, axis.
- **noise** — the noise stack and levels (shot noise, vibration residual + rejection
  ratio, …), RNG seed.
- **body / motion** — library object or mesh, shape/mass/density params; trajectory
  (r₀, v, a, spin) or path.
- **prior** (stream mode) — distributions over all of the above.
- **fields** — which fields to compute/return, plus grid resolution for volumetric ones.

The resolved config + seed fully determines a run — the reproducibility record.
Validation lives here too: `voxel_size` against closest-approach and the noise floor
(the M0 convergence tolerance), `dt` against the instrument cadence. Note `dt` (the
source-motion / state timestep) and `cycle_time` (how often the gradiometer yields a
shot) are distinct — the signal's time axis runs at the instrument cadence, which
may be coarser than `dt`.

## 5. Viewer (separate, lightweight)

A debug/inspection tool, **not** the main mode. A separate crate that renders a
**state** (§2) — and, crucially, it does not care whether that state is ground truth
from the simulator or a prediction from the model. Its input contract is the state
schema, source-agnostic. Stack: **eframe/egui** with `egui_plot` for signal traces
plus a small `wgpu` viewport for a 3D scene (baseline + atom clouds + the source body
on its trajectory, optionally the tidal field as a colour slice) — lightweight and
cross-platform (macOS/Windows/Linux). Much of the "plotting" also comes free from the
SDK tensors + matplotlib, so the viewer earns its place mainly for interactive 3D
sanity-checking — and for the prediction overlay below.

**Seeing the prediction.** Because the viewer renders states regardless of origin,
`sdk.view(truth, overlay=prediction)` draws a model-produced state against the
ground-truth one — a ghost body beside the solid one, predicted field beside true
field — through the same render path. The SDK is the conduit: it pushes a state into
the viewer (IPC to the viewer process, or embedding — an implementation detail). The
model just has to express its prediction in the §2 schema. The model itself is
deferred, but this contract is designed in now so nothing has to change when it lands.

## 6. Workspace layout

```
<project>/
├── crates/
│   ├── sim-core/    forward model (voxel/multipole/oracle, gradiometer, noise),
│   │                scenario + prior, parallel generation — the live engine
│   └── viewer/      eframe/egui + wgpu debug viewer (separate, one-way dep)
├── sdk/             Python package (maturin/PyO3 → sim-core): run() + stream()
│                    + thin CLI console entry point
└── docs/            this spec
```

Two Rust crates plus one Python package. Dependency direction is one-way out of
`sim-core`; the SDK (with its CLI) is the primary consumer, the viewer secondary.
The §2 state bundle is the only coupling between the simulator and anything
downstream.

## 7. Build phasing (test-grounded)

- **M0 — sim-core forward model + oracle.** Voxel cloud, multipole evaluator,
  analytic oracles. Test: voxelised sphere → analytic sphere tensor as resolution/
  order rise; trace-free/symmetric tensor invariants.
- **M1 — instrument + noise + one signal.** Reproduce George's reference outputs from
  the frozen fixtures via the propagation-phase integral (§3) — point-mass cases held
  to a tight tolerance (e.g. the 10 kg / D=10 m / 2.5 m/s pass, ~7 mrad), extended-body
  cases asserting convergence.
- **M2 — scenario + prior + per-tick state.** Assemble the full state bundle; freeze
  the §2 field set and shapes.
- **M3 — Python SDK (+ CLI).** `run` and `stream` yield torch tensors; the CLI entry
  point wraps them; round-trip a `DataLoader` with prefetch and confirm generation
  overlaps a dummy training step without starving it.
- **M4 — viewer.** Lightweight egui/wgpu: signal plot + 3D scene; built when needed.

## Deferred (parked behind existing seams, not built in v1)

- **Non-rigid source dynamics** — `Articulated` (walking people, piecewise-rigid),
  `Deforming` (soft bodies, per-tick re-reduction), `Fluid` (compressible &
  incompressible), plus structural vibration (modal, footfall-driven) and
  airflow/convection. All slot in behind the `SourceDynamics` seam (§3) without touching
  the gravity core or the §2 contract; the prescribed versions are cheap drop-ins, the
  solver versions are cache-and-replay backends. v1 is **rigid only**. Full design —
  prescribed-vs-solved, composition, the four hooks to keep in v1 — in **SPEC Appendix A**.
- **The model** that trains on the SDK stream (the eventual point). The source
  research names this directly: ML trained on simulation outputs to decompose
  multi-source periodograms and identify the physics signal. The periodogram (peaks
  at nf₀, mass-independent height ratios, frequency combs) is the natural analysis
  space, so it's a useful derived output of the SDK.
- **ULDM signal injection** — optionally add a synthetic ultralight-dark-matter phase
  signal to the simulated GGN so the model's task becomes *separating* signal from
  noise, the actual end-goal, rather than just identifying the noise.
- **Identifiability / Cramér–Rao analysis** — a future layer over the forward model;
  belongs with the ML work since it wants a differentiable forward model.
