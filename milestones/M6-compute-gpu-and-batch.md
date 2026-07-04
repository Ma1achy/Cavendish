# M6 — Compute backend, GPU & batch/stream (implementation brief)

> Scale: the WGSL fast path validated against the CPU reference, reproducible batched generation,
> plus the pieces batch generation needs — `config`, the `Prior`, realistic `Schedule`s with the
> contamination `mask`, and the Lomb–Scargle periodogram (non-uniform sampling makes LS, not FFT,
> the right spectral estimator). Read with `design/compute.md`, `design/scenario.md`,
> `design/state.md` §5, and the spec (`sec:freq`, the two-pass design).
>
> **Prereq:** M5 (the full forward model exists on the CPU path). **Delivers to:** M7 (the stream
> the SDK wraps). **Crates touched:** `compute` (both backends), `config`, `scenario`
> (`Prior`, `Schedule`), `state` (LS), `generate` (`stream`).

---

## 1. Requirements

| ID | Requirement |
|---|---|
| M6-R1 | `ComputeBackend` formalised: `CpuBackend` (rayon, f64 — the bit-exact reference and the `Dual` path) and `WgpuBackend` (WGSL, f32, differential-first). |
| M6-R2 | Two-pass execution: Pass 1 generates poses on-device (closed-form direct; ODE motions integrated with `fine_dt` substeps); Pass 2 evaluates field + phase. Parameters are uploaded, poses are not. |
| M6-R3 | Counter-based RNG: stateless `value = hash(key, counter)`; a fixed key tree (scenario → {atmo, noise-source i, prior-field j}); stable under `Prior` field-extension. |
| M6-R4 | `config`: the one typed schema (+ distribution primitives); `Prior::sample(seed) → Scenario`, total on a validated `Prior`. |
| M6-R5 | Realistic `Schedule`s: gaps and jitter; the contamination `mask` populated and carried to the bundle. |
| M6-R6 | Lomb–Scargle periodogram `(F,D)` as the `state` derived field, correct on non-uniform sampling. |
| M6-R7 | `generate.stream`: memory-bounded; **batch-invariant** (a batched bundle equals the same `Scenario` run alone); `(Prior, seed)` replays an identical batch. |
| M6-R8 | CPU ≡ GPU across the anchors to tolerance. |

---

## 2. Equations

### 2.1 Lomb–Scargle (per detector; times `tᵢ`, mean-subtracted signal `yᵢ`)

```
tan(2ωτ) = Σᵢ sin(2ωtᵢ) / Σᵢ cos(2ωtᵢ)

P(ω) = ½ [ (Σᵢ yᵢ cos ω(tᵢ−τ))² / Σᵢ cos²ω(tᵢ−τ)  +  (Σᵢ yᵢ sin ω(tᵢ−τ))² / Σᵢ sin²ω(tᵢ−τ) ]
```

Frequency grid: `f ∈ [1/T_span, f_Ny,eff]`, oversampled ×4; the ULDM line at ~0.1 Hz and an
oscillating source's drive line are the planted-recovery targets.

### 2.2 Counter RNG and the key tree

```
u64 draw:  x = philox_like(key, ctr)      key = H(seed, path…)     ctr = element index
key tree:  seed ─┬─ "atmo"          → mode draws
                 ├─ "noise"/i       → the i-th stack source
                 ├─ "prior"/field_j → each Prior field (stable under extension: new fields
                 │                     get new paths; existing draws untouched)
                 └─ "scenario"/i    → batch item i  (order-independent)
```

No sequential state anywhere ⇒ CPU/GPU, ordering, and batching cannot perturb draws.

### 2.3 The f32 budget (differential-first)

The WGSL path computes *differences* of `V` along arms (never large absolute potentials), keeping
the significand spent on the signal. Target: `|ΔΦ_gpu − ΔΦ_cpu| / |ΔΦ_cpu| ≤ 1e-4` on the anchors —
set empirically here and pinned as the regression bound thereafter.

---

## 3. Design

### 3.1 Two-pass execution

```
Vec<Scenario> ──► EvalBatch (parameters only: bodies as unit clouds + mass, trajectory params,
                  placements, schedule times, keys)
      │ upload once
      ▼
GPU  Pass 1: poses[s, src, t]  = closed-form(params, t)  |  ODE substep loop (fine_dt) on-device
     Pass 2: ΔΦ[s, t, d]       = phase(model, poses, placement)   (+ per-group when decomposition)
      │ readback
      ▼
SignalBatch ──► generate assembles bundles (noise applied CPU-side, post-hoc, keyed)
```

`CpuBackend` runs the *same* two passes with rayon over `(s, src, t)` — one structure, two
executors; the WGSL kernels mirror `gravity`'s Rust functions statement-for-statement.

### 3.2 Stream

Bounded: compute batch `k+1` while yielding batch `k`; hold ≤ 2 batches. Batch size from
`RunConfig` (auto-shrunk when `FieldSet.field` requests the volumetric grid).

---

## 4. Pseudocode

```
fn stream(prior, n, root, cfg) -> impl Iterator<Item = Result<StateBundle>>:
    scenarios = (0..n).map(|i| prior.sample(H(root, "scenario", i)))
    for chunk in scenarios.chunks(cfg.batch):
        sig = backend.eval(EvalBatch::from(chunk))       # passes 1+2
        for (scn, s) in zip(chunk, sig):
            yield assemble(scn, s)                        # noise, uldm broadcast, derived, bundle

fn lomb_scargle(t, y, freqs) -> Vec<f64>:
    y -= mean(y)
    for ω in 2π·freqs:  τ = ½·atan2(Σ sin 2ωt, Σ cos 2ωt) / ω;  P[ω] = ½(C²/Σc² + S²/Σs²)
```

---

## 5. Tests

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `rng_stateless` | same (key, ctr) → same draw; distinct paths decorrelated; extension-stability (adding a `Prior` field leaves old fields' draws intact) | exact |
| unit | `wgsl_kernel_parity` | each WGSL kernel vs its Rust reference on fixed inputs | ≤1e-6 rel (f32) |
| unit | `ls_analytic` | LS of a pure sinusoid on uniform sampling matches the periodogram peak/height; flat for white noise | ≤1e-6 / statistical |
| unit | `schedule_realism` | gap/jitter schedules realisable; uniform default exact; mask fraction as configured | exact |
| integration | `prior_total` | 10⁴ samples from a validated `Prior` all construct runnable `Scenario`s | total |
| integration | `pass1_ode_on_device` | GPU-integrated free-rotation poses vs CPU integrator | ≤1e-5 |
| integration | `stream_bounded` | streaming 10⁴ scenarios holds ≤ 2 batches resident (allocator counter) | structural |
| e2e | `cpu_equals_gpu` | all M1/M2 anchors + a rotating + a decomposed scenario: CPU vs GPU `ΔΦ` | ≤1e-4 rel |
| e2e | `batch_invariant` | one `Scenario` alone vs inside a 256-batch: CPU bit-exact; GPU within the f32 budget | exact / ≤1e-6 |
| e2e | `seed_replay` | `(Prior, seed)` twice → identical tensors, any machine/order | exact |
| e2e | `ls_planted_line` | an oscillating source + ULDM on a **gappy** schedule: LS recovers both lines as global peaks within one grid bin | bin-exact |

---

## 6. Exit requirements

| Requirement | Check | Tol |
|---|---|---|
| GPU ≡ CPU | `cpu_equals_gpu`, `wgsl_kernel_parity`, `pass1_ode_on_device` | ≤1e-4 |
| reproducibility | `seed_replay`, `rng_stateless`, `batch_invariant` | exact |
| dataset path | `stream_bounded`, `prior_total` | structural/total |
| spectra & schedule | `ls_planted_line`, `ls_analytic`, `schedule_realism` | bin/exact |

**The core engine is complete at M6 exit.**

> **Follow-up (M6-R1, `Dual` path).** M6a shipped `CpuBackend` as the f64 oracle but stopped the
> `Scalar` genericity at the `gravity` kernel — the phase integral, poses, and backend stayed f64, so
> the `Dual` path R1 names was not actually realisable (`CpuBackend::<Dual>` could not be
> instantiated). It is completed in the `forward-scalar-generic` PR (the M8 prerequisite): `math`
> poses, `instrument`'s phase core, `source`'s pose/integrator, and `compute::forward::<S>` are lifted
> to `Scalar`, gated by `value_channel_identity` (f64 ≡ `Dual` value across the whole model) with the
> f64 path held bit-exact. See `design/compute.md` §7–8.

## 7. Traceability

M6-R1/R8 → cpu_equals_gpu, wgsl_kernel_parity · M6-R2 → pass1_ode_on_device · M6-R3 → rng_stateless, seed_replay · M6-R4 → prior_total · M6-R5 → schedule_realism · M6-R6 → ls_* · M6-R7 → stream_bounded, batch_invariant, seed_replay.
