# Cavendish — `compute` drill-down

> Subsystem design for the `compute` crate: the execution substrate that evaluates the forward
> model over a whole batch on a device. Companion to `DESIGN.md` (§3.4 seam, §4 control flow, §5
> determinism) and the spec (§3 compute split, `nfr:cond` differential-first, `nfr` determinism).
> Consumes the two items deferred here: encoding the **two ODE motions** for on-device integration,
> and the **differential-first f32 WGSL kernel**.
>
> **Layering refinement (a drill-down finding).** `DESIGN.md` had `compute → gravity, math`. But
> `SignalBatch` is **ΔΦ** (a phase, not raw field), so `compute` executes the *whole* forward model
> — field **and** the arm phase integral — and therefore depends on what *defines* it:
> `compute → gravity, source, instrument, math`. `compute` sits just below `generate`, above
> `instrument`/`source`. Patched in `DESIGN.md` §1.

---

## 1. Responsibility & boundaries

**Owns:** the `ComputeBackend` seam; the `CpuBackend` (reference) and `WgpuBackend` (hot path); the
`EvalBatch`/`SignalBatch` types and their device layout; the WGSL re-expression of the forward
model; the counter-based RNG plumbing.

**Does not own:** the field *math* (gravity), the phase-model *math* (instrument), scenario
assembly or the `Prior` (scenario), the output bundle (state). `compute` *executes* a forward model
defined elsewhere; it adds no physics of its own — every backend must reproduce the canonical Rust
forward model to tolerance.

**The principle it exists to enforce (spec §3): upload parameters, not poses.** Body clouds upload
**once** per template; per batch we upload small per-scenario *parameters* (template index,
trajectory params, ODE initial conditions, detector placements, schedule, RNG keys), and the device
computes poses and fields. CPU-integrated poses are never shipped to the GPU — locality, not
parallelism, is the deciding factor for the one sequential axis (§5).

---

## 2. The `ComputeBackend` seam

```rust
pub trait ComputeBackend {
    fn evaluate(&self, batch: &EvalBatch) -> Result<SignalBatch, ComputeError>;
    fn info(&self) -> BackendInfo;   // device name, precision (f32/f64), limits
}
// impls: CpuBackend (rayon, f64, bit-reproducible), WgpuBackend (WGSL, f32). CUDA later.
```

`generate` picks a backend; they are interchangeable and must agree (§8). The contract: **`evaluate`
is a pure function of `batch`** — same batch (including RNG keys) ⟹ same `SignalBatch`, to the
backend's reproducibility class.

---

## 3. `EvalBatch` / `SignalBatch` — the device boundary

SoA throughout, flattened with offset tables (variable-length per scenario). The shape is the
"outer product" the GPU parallelises over.

```rust
pub struct EvalBatch {
    // ── templates: uploaded once, referenced by index ──
    elem_x: Vec<f32>, elem_y: Vec<f32>, elem_z: Vec<f32>, elem_m: Vec<f32>,  // all clouds concatenated
    templates: Vec<TemplateMeta>,        // {elem_offset, elem_count, kind, com, multipole, inertia}
    // ── per-scenario parameters ──
    sources:   Vec<SourceInstance>,      // {template_idx, Trajectory params, OdeIc}  (offsets per scenario)
    detectors: Vec<DetectorRec>,         // {placement SE3, Δr, baseline, arm geometry} (per scenario or shared)
    schedule:  Vec<f32>,                 // measurement times {t_ℓ}, concatenated (offsets per scenario)
    rng_keys:  Vec<u64>,                 // counter-based key per scenario/stream
    phase_model: PhaseModelKind,         // PropagationIntegral | QuasiStaticGradient (+ params: 2T, fine_dt)
    fields:    FieldSet,                  // which outputs to compute
    offsets:   BatchOffsets,             // scenario → (source span, detector span, schedule span)
}

pub struct SignalBatch {
    dphi:    Vec<f32>,                   // ΔΦ flattened over (scenario, detector, measurement)
    offsets: BatchOffsets,
    // optional intermediates (per FieldSet): per-arm phases, field samples
}
```

Element coordinates are `f32` on the GPU path; the CPU path lifts the same `EvalBatch` to `f64`
(or `Dual`) via the `Scalar` seam. `TemplateMeta` carries gravity's cached reductions so the far
field needs no recomputation.

---

## 4. Execution model — two passes

One field/phase work-item per `(scenario, detector, measurement)` would have to know every source's
pose at `t_ℓ`; ODE poses cannot be recomputed per detector (they are sequential in time). So
evaluation is **two passes**, which also isolates the one sequential axis:

```
Pass 1  POSE GENERATION    parallel over (scenario × source)
        → poses[s, source, ℓ]   (closed-form: evaluate directly; ODE: integrate, §5)

Pass 2  FIELD + PHASE       parallel over (scenario × detector × measurement)
        for each source in scenario: read pose[s,source,ℓ]; sum field over its cloud
        accumulate the differential phase (§6) → ΔΦ[s,d,ℓ]
```

Pass 1 computes each pose **once** and reuses it across all detectors and arms; Pass 2 is then
embarrassingly parallel reading the pose buffer. The CPU backend runs the same two phases as a
rayon loop; the GPU runs them as two compute dispatches.

---

## 5. Pass 1 — pose generation, and the two ODE motions on-device

*(Deferred item 1.)* Per `(scenario, source)`, produce the pose at each measurement time. Most
motions are **closed-form** — evaluate `pose_at(t_ℓ)` directly, no stepping. The two **ODE motions**
(spec §motion) integrate:

- **Pendulum** — timing ODE `s̈`-law (`θ̈ = −(g/L)sinθ`), producing arc-length progress.
- **Free rotation** — orientation ODE (torque-free Euler equations) for `ω(t)`, producing the
  quaternion; principal moments come from the template's `Inertia` (no extra batch params, only `ω₀`).

Both share **one symplectic integrator** (semi-implicit Euler / leapfrog, spec `nfr:symplectic`),
re-expressed in WGSL for the GPU. Each work-item walks `t` from `0` with substep `fine_dt`,
emitting the pose as it crosses each `t_ℓ`:

```
t ← 0;  state ← ic
for ℓ in measurements:
    while t < t_ℓ:  state ← symplectic_step(state, fine_dt);  t += fine_dt
    poses[s, source, ℓ] ← pose_of(state)
```

Sequential **in time within a trajectory**, parallel **across (scenario, source)** — which is why
it stays on-device (Pass 2 consumes the poses immediately; shipping them from the CPU would lose
the locality). Because the integrator is symplectic, energy stays bounded and f32 drift over a long
run is controlled (spec: energy drift is spectral drift).

---

## 6. Pass 2 — field + phase, differential-first f32

*(Deferred item 2.)* Each `(s,d,ℓ)` work-item computes the gradiometer differential phase by
looping its scenario's sources, evaluating each cloud's contribution at the arm points using
gravity's **body-frame trick** (transform the point by `pose⁻¹`, sum over elements, rotate the
result back — `design/gravity.md` §3), and accumulating the phase per the selected `PhaseModelKind`:

- **`QuasiStaticGradient`** — evaluate `Γ_zz` once at the detector midpoint, `ΔΦ ≈ Γ_zz · L · κ_eff`.
- **`PropagationIntegral`** — flight quadrature of `[V_up − V_low]` over the `2T` trajectory.

**Differential-first (spec `nfr:cond`) is the hard f32 constraint.** The kernel evaluates the
*gradient* analytically (each element contributes `−Gm/r³(𝟙 − 3ddᵀ/r²)` to `Γ`, `design/gravity.md`
§5) and accumulates the small differential quantity **directly** — it never forms two large absolute
phases and subtracts. The mrad-scale signal therefore stays well above the f32 epsilon; were the
kernel to subtract absolutes, catastrophic cancellation would sink it below the shot-noise floor.
This is a property the WGSL must preserve, validated by the cancellation test (§11).

The element loop is the inner reduction; `(s,d,ℓ)` is the grid. The gradiometer combination over
arms/IFOs happens inside the work-item (the per-arm partials reduce to one `ΔΦ`).

---

## 7. The two backends

- **`CpuBackend` (reference).** A rayon loop over the two passes, invoking the **canonical Rust
  forward model** — `instrument`'s phase kernel → `gravity`'s field kernel — generic over `Scalar`,
  so `f64` (reference) and `Dual` (forward-mode AD for the CRB Jacobians) both run here for free.
  Deterministic chunking gives **bit-reproducibility** across runs. This is the oracle the GPU is
  checked against, and the **default** home for autodiff (§8 — a choice, not a GPU limitation).
- **`WgpuBackend` (hot path).** The two passes as WGSL compute shaders, f32, differential-first. It
  **re-expresses** the gravity kernel and the phase integral in WGSL — the one unavoidable
  duplication of the math, which is exactly why it is validated against `CpuBackend` to tolerance.
  The wgpu device is created here but **shared with the viewer** (spec/`DESIGN.md` §5) rather than
  duplicated. Targets the project's real GPUs (RTX 2080 Super, 8 GB; M3 via Metal) — no datacentre
  assumptions.

---

## 8. Determinism, RNG, precision, autodiff

- **Counter-based RNG (philox/threefry)** keyed by `(global_seed, scenario_id, stream_id)` gives
  parallel, order-independent draws: the same value at a given index regardless of CPU/GPU or
  scheduling. Used by any in-forward-pass stochastic source (e.g. atmospheric GGN realised through
  the kernel — the source-vs-noise question is settled in the `noise` drill-down). Additive
  post-hoc noise (shot, vibration) is applied downstream in `generate`, not here.
- **Reproducibility classes:** `CpuBackend` is **bit-exact** run-to-run; `WgpuBackend` is exact to
  the **validation tolerance** (f32 + reordered reductions). Both are deterministic given the batch.
- **Precision:** f32 on the GPU is sufficient *only because* of differential-first (§6); f64 on the
  CPU. No mixed-precision tricks beyond this.
- **Autodiff is forward-mode, and lives on the CPU by *choice*, not necessity.** The CRB needs the
  Jacobian of the signal w.r.t. a *handful* of source parameters per scenario — a small fan-in, for
  which **forward-mode** (dual numbers) is the right tool: a dual is just `{value, derivative}`
  carried through the same arithmetic, and it threads through Pass 1's integrator too (differentiate
  w.r.t. `ω₀`, initial position). On the CPU it is **free** — instantiate the generic kernel at
  `S = Dual`. WGSL *can* do forward-mode equally well (a dual is a `vec2<f32>`; the GPU-hostile kind
  is **reverse-mode**'s tape, which Cavendish never needs). It is kept off the GPU only because
  (i) WGSL has no generics, so a dual kernel is a *second* shader to write and validate, not a free
  instantiation; (ii) the CRB is a small, occasional *analysis* job, not throughput-bound; and
  (iii) f32 tangents sit on the same differential-first edge as the primal, so f64 duals on the CPU
  are preferable for a quantity being pinned down precisely. **If** the CRB ever became
  throughput-bound (dense Fisher across a large geometry sweep), the path is code-generated
  forward-mode dual WGSL, validated GPU-AD-vs-CPU-AD exactly as the primal is today (§11) —
  recorded here so the option is on file, not built.

---

## 9. Errors

`evaluate` returns `Result<_, ComputeError>`: device init/allocation failure, batch exceeds device
limits (buffer size, workgroup count — relevant on the 8 GB 2080), malformed offsets. Validation is
at the boundary; the dispatch itself does not fail per work-item. Oversized batches return a clear
error so `generate` can chunk (it owns batching policy).

---

## 10. Public API surface

`ComputeBackend`, `CpuBackend`, `WgpuBackend`, `EvalBatch` (+ its builder, used by `generate`),
`SignalBatch`, `BackendInfo`, `ComputeError`. `generate` constructs the `EvalBatch` and chooses the
backend; nothing below `generate` names these.

---

## 11. Exit requirements (definition of done)

| Requirement | Check | Tol |
|---|---|---|
| backend agreement | `WgpuBackend` ≈ `CpuBackend` on a scenario suite (incl. ODE motions) | ≥99% / differential floor |
| CPU reproducibility | `CpuBackend` bit-identical run-to-run | exact |
| GPU reproducibility | `WgpuBackend` identical run-to-run | exact (same device) |
| params-not-poses | poses generated on-device; no per-tick pose upload in the batch | structural |
| pose pass | closed-form poses exact; ODE poses match the CPU integrator | integrator tol |
| differential-first | f32 `ΔΦ` stays above the noise floor; no cancellation collapse | floor |
| RNG | draws identical across CPU/GPU and across evaluation order | exact |
| throughput (goal) | batch path beats the reference loop by a large factor on a 2080 / M3 | — (not a correctness gate) |

Agreement with `CpuBackend` is the master check: it makes the GPU re-expression trustworthy and lets
everything downstream treat the backend as interchangeable.

---

## 12. Open sub-questions (resolve in implementation)

- **Pose buffer footprint.** `scenarios × sources × measurements × 7` f32 — fine for typical
  batches; very long schedules may need tiling the schedule axis. A batching-policy detail shared
  with `generate`.
- **One dispatch or two.** Whether Pass 1/Pass 2 are separate dispatches (clean) or fused for
  closed-form-only batches (saves a buffer when no ODE motion is present). Lean: always two, special-
  case later if profiling demands.
- **Quadrature order** for `PropagationIntegral` over `2T` — set by matching the CPU reference within
  the differential floor; not fixed now.
- **wgpu f64.** wgpu has no f64 in shaders; if a scenario ever needs f64 on GPU, that forces CUDA.
  Out of scope unless differential-first proves insufficient (it should not).
