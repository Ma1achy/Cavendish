# Cavendish — Engineering Design

> **Companion to `cavendish-spec/cavendish.tex`** (the spec: *what* the system is, the physics,
> and the requirements). This document is the **engineering design**: the Rust-level structure,
> the contracts *between* subsystems, and the internal design of each. Where the spec says
> "the instrument holds a detector array and a `PhaseModel`," this says *here is the `Detector`
> struct, the `PhaseModel` trait signature, its invariants, and how it is called*.
>
> Built **top-down**. This file opens at the **system level** (§1–§6); each subsystem is then
> drilled into in its own section, appended as we go (§7+). Signatures here are *illustrative
> contracts* — each is finalised in that subsystem's drill-down. Implementation (Claude Code)
> works against this document; the spec remains the source of truth for physics and numbers.

---

## 1. System decomposition and layering

Cavendish is one workspace, `sim-core` (the engine), plus two consumers with **one-way**
dependencies on it (`sdk`, `viewer`). Internally `sim-core` is strictly layered: **dependencies
point up the layers only — a lower layer never names a higher one.** This is the single
structural rule the whole design rests on; it is what keeps the engine free of Python/UI and
lets the viewer and SDK be swapped without touching physics.

```
L5  interfaces     sdk (PyO3: run/stream/view)        viewer (egui/wgpu)
                        │  one-way                          │  one-way
   ─────────────────────┼────────────────────────────────────┼──────────  sim-core ↑
L4  execution       generate  (orchestration: run/stream; batch dispatch)
L3  assembly        scenario (Scenario, Prior, Schedule)   state (StateBundle, analysis)
L2  domain          shape   source   instrument   uldm   noise      compute (backends)
L1  kernel          gravity  reference   (Cloud, elements, potential/field/tensor, oracle; reference = independent oracle)
L0  foundation      math (vectors, quaternions, Isometry, Scalar trait)   config (schema, Prior)
```

Dependency edges (each points *up*):
- `gravity → math`
- `reference → math` (the independent reference oracle — George's cases by direct quadrature; see
  `milestones/reference-port.md`. `math` only, so agreement with the engine stays non-circular)
- `compute → gravity, source, instrument, math` (executes the *forward model* — field **and** the
  arm phase integral, since `SignalBatch` is ΔΦ — so it depends on what defines that model;
  `design/compute.md` §1)
- `shape → gravity, math` (produces `Cloud`; mesh parsers feature-gated);
- `source → shape, gravity, math`; `instrument → gravity, source, math`; `uldm → math`;
  `noise → gravity, math`
- `scenario → source, instrument, uldm, noise, config`; `state → math`
- `generate → scenario, instrument, gravity, compute, state, noise, uldm`
- `sdk → generate, scenario, config, state`; `viewer → generate, state, gravity`

Two placements worth stating now, because they are easy to get wrong:
- **`gravity` is device-agnostic and pure.** It defines the kernel *math* (generic over the
  scalar type, §5) and the analytic oracle. It does **not** know about GPUs.
- **`compute` owns execution.** The CPU-reference backend calls `gravity`'s functions directly
  (rayon); the GPU backend re-expresses the same math in WGSL. Both satisfy one trait
  (`ComputeBackend`, §3.4), validated against each other and the oracle to ≥99%. This is the
  only place the CPU/GPU split (spec §3.4) lives.

---

## 2. Cross-boundary types (the nouns)

The types that travel between subsystems. Owning crate in brackets; everything else depends on
the owner to name the type.

- **`Cloud`** *[gravity; built by `shape`]* — a mass distribution: arrays of element positions and
  masses (SoA, see §5), plus an element kind (point/sphere/cube). The kernel's input; what a
  body discretises to. A `CloudView` is a borrowed slice for zero-copy world-frame evaluation.
- **`Isometry3` / `BodyMotion`** *[math]* — a rigid pose (rotation+translation) and the
  twist/acceleration (linear+angular) reported per tick.
- **`Source`** *[source]* — a `dyn SourceDynamics` plus identity/label; the unit summed into a
  scene (gravity is linear, so a scene is a `Vec<Source>`).
- **`Detector` / `Scene`** *[instrument]* — a `Detector` is one gradiometer: geometry (`Δr`,
  baseline, arms) at a ground `Placement`. A `Scene` is the array `Vec<Detector>` (`N=1` = single
  device). *(Disambiguation: "scene of sources" vs the detector `Scene` — name the latter
  `DetectorArray` to avoid the clash; resolved in §7.)*
- **`Schedule`** *[scenario]* — the realised measurement times `{t_ℓ}` (non-uniform) plus the
  contamination mask.
- **`Scenario`** *[scenario]* — the fully-specified runnable unit: sources, detector array, ULDM
  config, noise stack, schedule, requested field set.
- **`EvalBatch` / `SignalBatch`** *[compute]* — the compute boundary. `EvalBatch` carries
  *parameters, not poses* (spec §3.4): the body clouds (uploaded once per template), per-scenario
  trajectory parameters, detector placements, RNG keys, schedule times. `SignalBatch` is
  `ΔΦ[scenario, detector, measurement]`.
- **`StateBundle`** *[state]* — the output contract (spec Table, `tab:bundle`): per-detector
  signal, time-resolved track labels, target count, mask, optional fields, Lomb–Scargle.
- **`Config` / `Prior`** *[config]* — the one typed schema and the distribution over it.

---

## 3. The seams (the contracts between subsystems)

Four load-bearing traits plus one data contract. These are the *only* abstraction points; the
spec's four seams map onto §3.1–§3.4, the data contract onto §3.5. Signatures illustrative.

### 3.1 `SourceDynamics` — the source seam
```rust
/// A source's kinematics and mass distribution over time.
/// v1: rigid bodies only. The seam admits prescribed → solved → deforming later (spec App. A).
pub trait SourceDynamics {
    /// Fixed body-frame mass cloud. Rigid: computed once at load. (Deformable overrides §3.1.)
    fn body_cloud(&self) -> &Cloud;
    /// Pose at time t (world ← body). Closed-form for most motions; ODE-derived and cached
    /// for the pendulum's timing and free rotation's orientation (§source).
    fn pose_at(&self, t: f64) -> Isometry3;
    /// Twist + acceleration at t, for the state bundle (not needed by the field eval).
    fn motion_at(&self, t: f64) -> BodyMotion;
}
```
Rigidity is encoded structurally: a fixed `body_cloud` + a time-varying `pose_at` is exactly the
"upload the cloud once, upload parameters per tick" shape the compute boundary wants. The
deformable extension (a `world_cloud(t)` that varies) is the seam's growth point and gives up
that optimisation — noted, not built.

Orientation is a source class in its own right (spec §motion): `Spin`/`Libration` are closed-form,
while **`FreeRotation`** (the torque-free Euler top) is the *second* ODE motion after the pendulum
— both behind `pose_at`, both integrated symplectically by one shared integrator (generic over
`Scalar`, §5; explicitly **not** a physics-engine dependency). `FreeRotation` needs no extra
parameters on the seam: its principal moments come from `body_cloud`'s `Inertia` reduction
(`design/gravity.md` §6), so only the initial angular velocity is per-scenario.

### 3.2 `PhaseModel` — the forward-model seam
```rust
/// Maps a scene of sources to ONE gradiometer's differential phase at a measurement time.
/// The double-difference is internal to a detector; cross-detector structure is derived later.
pub trait PhaseModel {
    fn delta_phi(&self, sources: &[&dyn SourceDynamics], det: &Detector, t: f64) -> f64;
}
```
Implementations: `PropagationIntegral` (the flight quadrature over the four arms, the reference);
`QuasiStaticGradient` (the `Γ_zz·L` fast path). Selection is a config choice (§7).

### 3.3 `NoiseSource` — the noise seam
```rust
/// Adds a seeded noise realisation to the per-detector signal.
/// `scene` is passed so PHYSICALLY-MODELLED noise (atmospheric GGN) has the geometry it needs;
/// shot noise ignores it. (Whether atmospheric GGN is a NoiseSource or a stochastic Source: §7.)
pub trait NoiseSource {
    fn add(&self, t: &[f64], scene: &DetectorArray, signal: &mut SignalBatch, rng: &mut CbrngKey);
}
```

### 3.4 `ComputeBackend` — the compute seam
```rust
/// Executes the field-evaluation outer product on a device. (Spec §3.4.)
/// Contract: take an EvalBatch (parameters, not poses), return per-detector signals.
pub trait ComputeBackend {
    fn evaluate(&self, batch: &EvalBatch) -> Result<SignalBatch, ComputeError>;
}
```
Implementors: `CpuBackend` (rayon, calls `gravity` directly, f64, bit-reproducible) and
`WgpuBackend` (WGSL, f32, differential-first). A CUDA backend is a later third implementor. The
pendulum ODE-in-time runs *inside* the backend (locality, spec §3.4); how the GPU batch carries
it is a §7 question.

### 3.5 `StateBundle` — the data contract
Not a trait — the output struct (spec `tab:bundle`), and **the engine's whole job ends here**: it
dumps the *entire* simulation state as one typed bundle (torch-ready) and stops. The full
per-measurement record — source motion (position, orientation, linear/​angular velocity and
acceleration), the static shape and inertia (cloud, mass, inertia tensor, principal moments/​axes,
quadrupole), the array geometry, the multi-channel signal with its ground-truth channel
decomposition (targets/​atmospheric/​ULDM/​noise), the contamination mask, derived spectra — all as
ground truth.
Generating ML datasets at scale is a primary intended use and the engine is *built* for it
(batched, reproducible-from-a-seed, tensor-native, hence the batch path and the `Prior` sugar); but
it serves that use by emitting the **whole** record and letting the consumer decide what is input,
label, target, or context. The supervised/self-supervised/analysis structure lives entirely
downstream — a model later predicts against the bundle's shape, a viewer renders either. **`FieldSet`
is a cost knob, not a task:** the default is dump-everything; selecting fields only trims the dump
for compute/storage ("skip the volumetric field this run"), never defines what the data *is*. This
completeness is what keeps every downstream use — including unimagined ones — possible.

---

## 4. Control & data flow

One call path, two execution modes (reference loop vs batch dispatch — same physics):

```
sdk.run / sdk.stream
  └─ Prior.sample ─────────────────────────► Scenario
        └─ generate.run(scenario, fields):
             Schedule.sample()                         → t_meas {t_ℓ}, mask
             ── reference mode (CpuBackend, one scenario) ──
             for ℓ in t_meas, for d in array:
                 phi  = PhaseModel.delta_phi(sources, det_d, t_ℓ)   // §3.2
                 phi += uldm.phase(t_ℓ)                              // common-mode
             ── batch mode (any ComputeBackend, the outer product) ──
             ComputeBackend.evaluate(EvalBatch{clouds, traj params, placements, keys, t_meas})
             ───────────────────────────────────────────────────────
             record tracks / source / field into StateBundle  // per requested fields
             for ns in noise_stack: ns.add(t_meas, array, &mut signal, rng)   // §3.3
             if fields.periodogram: lomb_scargle(t_meas, signal)
        └─ StateBundle ─► (tensors) ─► consumer
```
The reference loop and the batch path must agree to the validation tolerance (§5). `generate`
chooses the backend; everything above it is backend-agnostic.

---

## 5. Cross-cutting concerns

- **Scalar genericity (autodiff).** The kernel math is generic over a `Scalar` trait
  (`fn potential<S: Scalar>(…) -> S`), so `f64`, `f32`, and dual numbers all flow through the
  *same* code. This is how NFR-differentiability (spec `nfr:diff`) is achieved structurally
  rather than bolted on, and it is what makes the Fisher/CRB analysis (spec §Gradar) possible. The
  trait lives in `math`; `gravity`, `instrument`, and the CPU backend are generic over it.
- **Determinism & seeding.** A counter-based RNG (philox/threefry) keyed by `(global_seed,
  scenario_id, stream_id)` gives parallel, reproducible draws with no sequential state — the
  same value at index *i* regardless of evaluation order, on CPU or GPU. The CPU path is
  bit-reproducible; the GPU path is reproducible to the validation tolerance (reductions
  reorder), per spec `nfr` determinism.
- **Differential-first f32.** The WGSL kernel evaluates `Γ_zz·L` directly, never subtracting two
  large absolute phases (spec `nfr:cond`), keeping f32 above the mrad shot-noise floor. A design
  constraint on the GPU backend, not a runtime option.
- **Error handling.** Construction/config validation returns `Result` (a `ConfigError` /
  `ComputeError`); the hot evaluation path does not allocate errors per element — it validates at
  the boundary and then runs infallibly. (Finalised per subsystem.)

---

## 6. Subsystem inventory (the drill-down agenda)

Each row is a subsystem's **contract surface**: what it owns, what it promises, what must hold.
The drill-down fills each into a full section (types, internal structure, invariants, tests).

| Subsystem | Owns / implements | Inputs → Outputs | Key invariants |
|---|---|---|---|
| `math` | `Scalar`, vectors, `Isometry3` | — | autodiff-clean; no panics in hot ops |
| `gravity` | `Cloud`, elements, multipole, **oracle** | (`Cloud`,point) → `V,g,Γ` | `Γ=Γᵀ`, `trΓ=0` in vacuum; near/far continuous; generic over `Scalar` |
| `shape` | `Solid`, voxeliser, primitives + moment oracle, mesh import | geometry → `Cloud` (unit mass, cached) | mass exact; CoM at origin; deterministic order; robust winding-number path for dirty meshes (`design/shape.md`) |
| `compute` | **`ComputeBackend`**; CPU + wgpu backends | `EvalBatch` → `SignalBatch` | CPU bit-exact; GPU ≈ CPU to tol; params-not-poses |
| `source` | **`SourceDynamics`**; bodies (via `shape`), `Trajectory`, rotation | params → poses + `Cloud` | rigid: cloud fixed; linear in mass; 2 ODE motions (pendulum, free rotation) symplectic; moments from cloud |
| `instrument` | **`PhaseModel`**; `Detector`, `DetectorArray`, arms | sources, det, t → `ΔΦ` | double-difference internal; `N=1` = single device |
| `uldm` | closed-form `ΔΦ_φ(t)` | t → phase | identical across the array (common-mode) |
| `noise` | **`NoiseSource`** stack; shot/vibration/atmospheric | t, scene, signal → signal′ | seeded/deterministic; additive after validation |
| `scenario` | `Scenario`, `Schedule`, optional `Prior` | `Scenario` → run; (`Prior` → `Scenario`) | `Scenario`→bundle is the contract; `Prior` is optional batch sugar; schedule default = uniform |
| `state` | `StateBundle`; analysis (LS, CRB) | filled by `generate` | field selection; shapes per `tab:bundle` |
| `generate` | run/stream; batch dispatch | `Scenario` → `StateBundle` | reference ≡ batch to tol; chooses backend |
| `sdk` | PyO3 run/stream/view, CLI | Python ↔ engine | GIL released on eval; torch-ready tensors |
| `viewer` | egui/wgpu inspector | `StateBundle` → pixels | one-way dep; reuses wgpu device |

---

## 7. Open design questions (resolve during drill-down)

- **Atmospheric GGN: `NoiseSource` or stochastic `Source`?** It is realised *through* the gravity
  kernel (spec §4.14), which argues it is a source evaluated in the forward pass, not a post-hoc
  additive noise. But it is conceptually "noise." Decides whether `NoiseSource::add` needs scene
  access (§3.3) or whether a `StochasticSource` variant feeds the forward model.
  **Resolved:** a **stochastic field source** evaluated in the forward pass (correct double-
  difference + placement-dependent partial common-mode), with a thin config-level `NoiseSource`
  adaptor; additive noise (shot, vibration) stays post-hoc (`design/noise.md` §1, §6–§7).
- **`PhaseModel` selection.** Per-scenario config flag vs a property of the run. Quasi-static is
  the fast path; the integral is the reference. **Resolved:** a per-run `InstrumentConfig` flag,
  default `PropagationIntegral` (`design/instrument.md` §7).
- **Name clash `Scene`.** Detector array vs "scene of sources." **Resolved:** `DetectorArray` for
  the instrument side, `Vec<Source>` (no wrapper) for sources (`design/instrument.md` §3).
- **GPU ODE motions.** The one sequential-in-time axis (now *two* motions — the pendulum's timing
  and free rotation's orientation, sharing one integrator) lives in the backend; how `EvalBatch`
  encodes "integrate these trajectory params on-device" cleanly is a `compute` drill-down item.
  **Resolved:** a two-pass design — Pass 1 generates poses on-device (closed-form direct, ODE
  integrated, parallel over scenario×source), Pass 2 consumes them (`design/compute.md` §4–§5).
- **Cloud layout.** SoA (separate position/mass arrays) for GPU coalescing and SIMD — confirm the
  exact layout in the `gravity` drill-down (it constrains every downstream type). **Resolved:**
  SoA, body-frame, fixed; body-frame evaluation transforms the point not the cloud
  (`design/gravity.md` §3).
- **Analysis placement.** Lomb–Scargle and the CRB — in `state`, or a sibling `analysis` crate?
  CRB needs the differentiable kernel, so it reaches back to `gravity`; that may argue for its own
  crate above `gravity`. **Resolved:** split by need — Lomb–Scargle stays in `state` (pure on the
  signal); the CRB/Fisher goes to a separate `analysis` crate (`→ gravity, source, instrument,
  compute, state`), since it needs the `Dual` forward model and must not drag it into the
  data-contract crate (`design/state.md` §7).

---

## 8. Proposed drill-down order

Bottom of the dependency graph first, so each subsystem is designed against fixed foundations.
Each drill-down is its own file under `design/<subsystem>.md`.

1. **`gravity`** — the kernel + oracle + `Cloud` layout. Foundational; everything depends on it,
   and it builds and verifies in isolation, so its design unblocks the first build.
   → **`design/gravity.md` (done)**
2. **`compute`** — the `ComputeBackend` boundary and the two backends.
   → **`design/compute.md` (done)**
3. **`source`** — `SourceDynamics`, the body library, the trajectory/ODE design.
   → **`design/source.md` (done)**
4. **`instrument`** — `DetectorArray`, arm construction, the two `PhaseModel`s.
   → **`design/instrument.md` (done)**
5. **`uldm` + `noise`** — the two signal/background channels (and the atmospheric-GGN question).
   → **`design/uldm.md`, `design/noise.md` (done)**
6. **`scenario`** — assembly, the `Prior`, the `Schedule`.
   → **`design/scenario.md` (done)**
7. **`state`** — the `StateBundle` and analysis.
   → **`design/state.md` (done; CRB split into a sibling `analysis` crate, §7)**
8. **`generate` → `sdk` → `viewer`** — orchestration and the interfaces.
   → **`design/generate.md`, `design/sdk.md`, `design/viewer.md` (all done)**

**Bottom-up drill-down phase complete.** All subsystems designed: `gravity`, `compute`, `shape`
(`design/shape.md` — geometry → mass, added after the phase closed), `source`, `instrument`,
`uldm`, `noise`, `scenario`, `state` (+ the `analysis` split), `generate`, `sdk`, `viewer`. Remaining passes (separate): the **milestone-sequencing** pass, and the **M0 implementation
brief** for Claude Code.

Start with `gravity`.
