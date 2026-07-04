# Cavendish — `state` drill-down

> Subsystem design for the `state` crate: the **output contract** — the complete dump the engine
> hands back. Companion to `DESIGN.md` (§3.5 data contract, §6 inventory) and the spec (`tab:bundle`,
> §freq `sec:freq`). **Settles the last open question** (`DESIGN.md` §7): analysis placement.
>
> **Dependencies:** `state → math` only. It is a clean, low-dependency data-contract crate — it does
> **not** depend on the forward model. (The CRB, which *does* need the forward model, is split out,
> §7.)

---

## 1. Responsibility & boundaries

**Owns:** the `StateBundle` (the complete per-measurement record); the `FieldSet` (the cost knob);
the **torch-ready tensor layout**; the one derived field that is a pure function of the bundle's own
signal — the **Lomb–Scargle periodogram**; and the serialisation schema.

**Does not own:** the forward model or how the fields are *produced* (`generate` fills them); the
**CRB/Fisher** analysis (needs the differentiable forward model → a separate `analysis` crate, §7);
Python/​torch interop (`sdk` wraps this layout).

**The engine's job ends here** (spec §1, `DESIGN.md` §3.5): `state` is where the entire simulation
state is dumped, complete, as ground truth, and the engine stops. What is input/​label/​target/​
context is the consumer's call, made on this dump.

---

## 2. The `StateBundle` — the complete dump

Exactly the spec's `tab:bundle`, as a struct of named arrays. The **full** per-measurement record
for `T` measurements, `S` sources, `D` detectors:

```rust
pub struct StateBundle {
    // -- time, and the source's motion (per source S, over T measurements) --
    time:                    Array1<f64>,         // (T,)        measurement timestamps (non-uniform under a schedule)
    source_position:         Array3<f32>,         // (S,T,3)     COM world position -- the localisation target
    source_orientation:      Array3<f32>,         // (S,T,4)     orientation quaternion wxyz (world <- body)
    source_velocity:         Array3<f32>,         // (S,T,3)     COM linear velocity
    source_angular_velocity: Array3<f32>,         // (S,T,3)     angular velocity omega: direction = spin axis, magnitude = rate
    source_accel:            Array3<f32>,         // (S,T,3)     COM linear acceleration
    source_angular_accel:    Array3<f32>,         // (S,T,3)     angular acceleration
    // -- the source's shape (per source S, static; one second moment C) --
    source_cloud:            Array3<f32>,         // (S,N,4)     body-frame mass elements (x,y,z,m); the density (fixed)
    n_sources:               usize,               //             number of sources S
    // -- the array (per detector D, static) --
    detector_placement:      Array2<f32>,         // (D,7)       position xyz + orientation quat; static (v1 vertical)
    // -- the signal (per measurement T, detector D) --
    signal:                  Array2<f32>,         // (T,D)       gradiometer dPhi (targets + atmospheric + ULDM + noise) [rad]
    mask:                    Array1<bool>,        // (T,)        transient-contaminated cycles
    // -- optional: shape & inertia descriptors (cheap, derived from source_cloud) --
    source_mass:             Option<Array1<f32>>, // (S,)        total mass M
    source_inertia:          Option<Array3<f32>>, // (S,3,3)     inertia tensor I (body frame)
    source_moments:          Option<Array2<f32>>, // (S,3)       principal moments (I1,I2,I3)
    source_axes:             Option<Array3<f32>>, // (S,3,3)     principal axes (body frame; shared by I and Q)
    source_quadrupole:       Option<Array3<f32>>, // (S,3,3)     gravitational quadrupole Q (body frame)
    // -- optional: ground-truth signal channels (sum to `signal`) --
    signal_targets:          Option<Array2<f32>>, // (T,D)       phase from the target masses alone (clean)
    signal_atmospheric:      Option<Array2<f32>>, // (T,D)       phase from atmospheric GGN
    signal_uldm:             Option<Array1<f32>>, // (T,)        ULDM common-mode phase (identical across detectors)
    signal_noise:            Option<Array2<f32>>, // (T,D)       additive measurement noise (shot, vibration)
    signal_per_ifo:          Option<Array3<f32>>, // (T,D,2)     the two single-interferometer phases (pre double-difference)
    // -- optional: field samples (heavy) and derived spectra; n_c = 2 --
    field_at_clouds:         Option<Array4<f32>>, // (T,D,2,3)   g at each atom cloud
    gradient_at_clouds:      Option<Array5<f32>>, // (T,D,2,3,3) gradient tensor Gamma at each cloud
    field_grid:              Option<Array5<f32>>, // (T,X,Y,Z,3) g on a grid (storage-dominant)
    periodogram:             Option<Array2<f32>>, // (F,D)       Lomb-Scargle PSD of `signal`
    // -- meta --
    meta:                    Meta,                //             resolved config (per-source motion type & params) + seed
}
```

`signal` is the multi-channel observable (one gradiometer differential-phase series per detector,
spec §array), and `detector_placement` is the array geometry that makes it interpretable -- you
cannot localise from `signal` without knowing where the D detectors are. **Rotation is first-class:**
`source_angular_velocity` carries the instantaneous spin axis (its direction) and rate (its
magnitude), and the static shape block (`source_inertia`/`source_moments`/`source_axes`/
`source_quadrupole`) -- all reductions of the one second moment C -- describes the tumble and the
gravitational shape. The optional `signal_*` channels (`targets`/`atmospheric`/`uldm`/`noise`) are
the ground-truth decomposition that **sums to** `signal`: the engine knows each because it
generated it, so it dumps them for any consumer that wants the clean target signal, the realistic
floor, the common-mode line, or the noise on its own. `source_position`/`n_sources` are ground
truth, not a packaged supervised target (spec §1). `meta` makes every bundle self-describing and
the run reproducible.

---

## 3. `FieldSet` — the cost knob, not a task

```rust
pub struct FieldSet {
    pub shape:         bool,         // source_mass/inertia/moments/axes/quadrupole -- cheap, derived from source_cloud
    pub decomposition: bool,         // signal_{targets,atmospheric,uldm,noise,per_ifo} -- extra forward evals
    pub field:         FieldSamples, // field_at_clouds / gradient_at_clouds / field_grid -- heavy, esp. the grid
    pub periodogram:   bool,         // Lomb-Scargle -- derived from `signal`
}
impl Default for FieldSet { /* all off: the complete mandatory record only */ }
```

The mandatory fields — `time`, the full per-tick motion (`source_position`, `source_orientation`,
`source_velocity`, `source_angular_velocity`, `source_accel`, `source_angular_accel`),
`source_cloud`, `n_sources`, `detector_placement`, `signal`, `mask`, `meta` — are **always**
present; the complete record is the default. Each flag adds a group: `shape` the cheap inertia
descriptors; `decomposition` the ground-truth signal channels (which cost extra forward
evaluations, since each field-source group must be run separately to isolate its phase); `field`
the heavy per-measurement field samples (volumetric `field_grid` dominates storage); `periodogram`
the derived spectrum. Every flag is a **compute/storage** decision — "I don't need the volumetric
field this run" — never a change to what the bundle *means* or a task (spec, `DESIGN.md` §3.5).
Selection trims the dump; it does not define a dataset.

---

## 4. The tensor layout — torch-ready

The bundle *is* a set of named, contiguous tensors with fixed shapes and dtypes, laid out so `sdk`
hands them to torch with little or no copy:

- **Leading axes** are `(T, …)` or `(S, T, …)` — measurement-time first (or source-then-time), the
  natural batch/​sequence axes for a sequence model.
- **dtype policy:** `signal` and the GPU-produced fields are **f32** (the differential-first path,
  `compute.md` §6, keeps f32 above the floor); `time` is **f64** (timestamps); ground-truth kinematics
  default to f32 for tensor-native training, with an f64 option when an analysis wants it. No hidden
  precision tricks.
- **Ragged `S`** (variable source count across a batch) is handled at the batch level by `sdk`
  (padding + a count), not inside a single bundle.

---

## 5. Lomb–Scargle — the derived field that lives here

The signal under a schedule is **non-uniformly sampled** (`scenario.md` §3), so its spectrum is the
**Lomb–Scargle periodogram**, not an FFT (spec `sec:freq`: LS is the first-class spectral output; a
uniform FFT is the fallback). It is a **pure function of `signal` and `time`** — no forward model — so
it belongs in `state`, computed per detector when `FieldSet` requests `periodogram`. This is the one
piece of "analysis" cheap and self-contained enough to be a bundle field; everything heavier is §7.

---

## 6. Serialisation — the streaming form and the optional cache

The bundle is the **unit of streaming** (the live tensor stream, spec §1) and of caching. `state`
defines the canonical serialisation (field names, shapes, dtypes); the actual sinks — a frozen
on-disk cache (WebDataset for record streams; Zarr/​HDF5 for the volumetric fields, spec) — are driven
by `generate`/​`sdk`. Disk is an **optional** cache, never the path (spec §1): the default is generate
→ stream → consumer, with `meta` making any cached bundle self-describing.

**Realisation (M9).** The v1 bundle is `serde`-derived over its `f64`/`Vec` fields (the `f32`/`ndarray`
representation above is the target, not yet cut), with `save_bundle`/`load_bundle` providing a compact
`bincode` sink — the concrete cache the `viewer`'s loaded path reads. `source_cloud` is populated by
`generate::run` from the source's body cloud; the storage-dominant `field_grid` stays deferred (the
viewer samples `gravity` on demand for its field view — `viewer.md` §4).

---

## 7. Analysis placement — the decision (resolves `DESIGN.md` §7)

Split by what each needs:

- **Lomb–Scargle stays in `state`** — pure post-processing on `signal`, no forward model (§5).
- **The CRB/Fisher goes to a separate `analysis` crate** — because it needs the **differentiable
  forward model**: the Fisher information is `J^⊤ Σ⁻¹ J` with `J = ∂(signal)/∂(source params)`
  obtained from the **`Dual` path** through `gravity`/​`source`/​`instrument` via `compute`'s
  `CpuBackend` (`compute.md` §8), and `CRB = Fisher⁻¹`. That reaches back across the whole forward
  stack, so it must **not** live in `state` (which would drag the entire forward model into the
  data-contract crate). `analysis → gravity, source, instrument, compute, state` (it consumes the
  bundle shape and the Dual forward model), sits beside `generate`, and is exposed through `sdk`. It
  produces the identifiability characterisation (spec §Gradar) per scenario/​geometry — an analysis
  *output*, not a bundle field. Keeping `state` to `math`-only is the reason for the split.

`analysis` is small (assemble `J` via duals, form and invert Fisher, report CRB and resolvability);
it can take its own short drill-down if it grows, but the placement is the load-bearing call here.

---

## 8. Connection to `generate`

`generate` runs the forward model and **fills** the mandatory + requested fields; `state` defines
their *shape* and computes the **derived** `periodogram` (§5) when asked. `state` never runs physics
— it is the container `generate` writes into and the consumer reads out of. (`generate.run` →
`StateBundle`, `DESIGN.md` §4.)

---

## 9. Purity & determinism

A `StateBundle` is a pure function of `(Scenario, backend)`: every field is determined by the run,
and the derived `periodogram` is a deterministic transform of `signal`/​`time`. `meta` pins the config
and seed, so a bundle is reproducible and self-describing in isolation.

---

## 10. Errors & API

Construction enforces shape consistency (`Result<_, StateError>`: a field whose `T`/​`S`/​`D` disagree
with the others, a requested derived field with no `signal`). Reads are infallible. Public surface:
`StateBundle`, `FieldSet`, `Meta`, the `periodogram` computation, and the serialisation schema;
`generate` writes the bundle, `sdk` exposes it as torch tensors, `analysis` consumes its shape.

---

## 11. Exit requirements (definition of done)

| Requirement | Check | Tol |
|---|---|---|
| schema | every field matches `tab:bundle` shapes/​dtypes; mandatory always present | exact |
| complete by default | default `FieldSet` yields the full mandatory record | structural |
| rotation | `source_angular_velocity` direction/​magnitude track the spin axis/​rate; consistent with `d(orientation)/dt` | tol |
| shape descriptors | `source_inertia`/​`moments`/​`axes`/​`quadrupole` reduce correctly from `source_cloud` (one second moment C) | tol |
| decomposition | `signal_targets + signal_atmospheric + signal_uldm + signal_noise` reproduces `signal` | tol |
| cost knob | toggling any flag changes only what is computed, never the meaning of present fields | structural |
| Lomb–Scargle | correct PSD of a known non-uniformly-sampled signal; recovers a planted line | tol |
| tensor-native | the layout loads into torch with no per-field reshape on the Python side | structural |
| serialisation | a bundle round-trips through the cache; `meta` makes it self-describing | exact |
| purity | same `(Scenario, backend)` → identical bundle; `periodogram` deterministic | exact |

`state` needs only `math`; verifiable by constructing bundles and checking shapes/​round-trip without
running the forward model. (The CRB's exit lives with `analysis`.)

---

## 12. Open sub-questions (resolve in implementation)

- **`analysis` crate boundary.** Exactly what it exposes (Fisher, CRB, resolvability, the
  array-geometry optimisation that `scenario.md` §7 flagged) and whether it earns its own drill-down.
- **Cache format.** WebDataset vs Zarr/​HDF5 split for record streams vs volumetric fields; chunking
  for `field_grid` (the storage-dominant field).
- **Ragged batching.** Where variable `S`/​`N` padding lives precisely (`sdk` vs a batch helper) and
  the count convention the consumer reads.
- **Backing array type.** `ndarray` vs a thin own type for the tensors, and the cheapest hand-off to
  torch in `sdk` (zero-copy where the dtype/​layout allow).
