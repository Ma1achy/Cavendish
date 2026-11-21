# Cavendish — milestone plan

> The bottom-up design (`DESIGN.md` + `design/*.md` + the spec) says *what* each subsystem is and its
> *exit requirements*. This document sequences those into an ordered **build plan**. It is the
> milestone-sequencing pass the drill-downs deferred.
>
> **Organising principle — vertical slices.** Every milestone keeps a working end-to-end path
> (`Scenario → StateBundle`) and *enriches* it. We do **not** finish one crate at a time and
> integrate at the end; from M1 there is always something runnable and validated. Risk is retired in
> order: the physics first (M1), the GPU path only once the CPU reference exists to check it against
> (M6), rotation and the channels in between.
>
> **Per-milestone implementation briefs live in `milestones/`** (M0–M10): requirements traced
> to tests, design + diagrams, equations, pseudocode, three-level tests with tolerances, and exit
> criteria — the document a Claude Code session is pointed at for that milestone.
>
> **Anchors are validated against a live reference, not remembered figures.** George's
> validation cases are ported to an independent Rust `reference` crate (`milestones/reference-port.md`);
> each anchor asserts `cavendish ≈ reference` (two independent methods — quadrature vs voxels —
> agreeing).

---

## Test taxonomy (used by every milestone)

- **Unit** — in-crate, pure functions and types, fast, no cross-crate deps. Rust `#[cfg(test)]`
  modules. (E.g. the gradient kernel is trace-free; quaternion integration preserves norm.)
- **Integration** — cross-crate within the workspace, composed behaviour. Rust `tests/` directories.
  (E.g. `source → gravity → instrument` yields a phase; `generate.run` fills a bundle.)
- **E2E** — the whole pipeline to an externally-meaningful result: the physics anchors, the
  decomposition identity, reproducibility, and (from M7) the Python→torch path. Rust `tests/` or
  Python `pytest`.

Every milestone's **Exit** row ties back to the drill-down exit requirements; a milestone is done
when its tests pass *and* CI is green.

---

## Sequence at a glance

```
M0  scaffolding & CI ─┐
M1  physics spine      │  (walking skeleton: one mass, one gradiometer, one anchor)
M2  motion library     │
M3  detector array     ├─ core engine (usable from Rust for single runs)
M4  rotation           │
M5  channels + decomp  │
M6  compute/GPU + batch ┘  (usable for dataset generation at scale)
M7  sdk (python/torch)     (usable from the intended workflow)
M8  analysis (CRB)     ─┐  (independent of each other; either order after M6)
M9  viewer             ─┘
M10 mesh import            (off the critical path; any time after M2)
M11 world seam         ─┐  v2: one source seam (a scene is a list of sources of any kind);
M12 physics layer       │  parcel bodies, rigid dynamics, convecting air — CPU. Implemented.
M13 scene viewer        │  planned: continuous-time replay, the air as a volume, the response map,
                        │           and the full interface (panels, menus, console, per-tower lanes)
M14 voxel core          │  planned: one sparse tree for the voxeliser, the gravity read, air, contact, viewer
M15 timing + scene spec─┘  planned: towers on their own clocks; one serialisable, addressable scene description
Mref reference port        (a THREAD: each of George's 13 cases lands with the milestone it
                            validates — M1/M2 geometry, M5 ULDM/lift, M6 schedules/spectra)
```

Critical path is **M0→M6→M7**. **M8** (analysis) and **M9** (viewer) branch off after M6 and may be
built in parallel or reordered. **M10** (mesh import) extends the body dictionary only — nothing
downstream depends on it — so it may be scheduled any time after M2's shape core.

---

## Workspace layout (established in M0)

```
cavendish/
├── Cargo.toml                 # workspace
├── crates/
│   ├── math/  config/                         # L0
│   ├── gravity/  reference/                   # L1  (reference = George's cases, oracle)
│   ├── shape/ compute/ source/ instrument/ uldm/ noise/   # L2
│   ├── world/ physics/                        # L2  (v2: the source seam; the scene simulator)
│   ├── scenario/ state/                       # L3
│   ├── generate/ analysis/                    # L4
│   └── sdk/ viewer/                           # L5
├── python/                    # SDK Python package + pytest
├── cavendish-spec/            # cavendish.tex (the authoritative spec)
├── .github/workflows/  .gitignore            # (the M0 dev container was removed: run natively)
└── DESIGN.md  MILESTONES.md  design/*.md
```

---

## M0 — Scaffolding & CI

**Goal.** The workspace compiles, CI is green on a skeleton, and the development environment is
reproducible. No physics yet.

**Builds.** The cargo workspace and all sixteen crate skeletons (compiling, empty); the four seam
traits as *interfaces only* (`SourceDynamics`, `PhaseModel`, `NoiseSource`, `ComputeBackend`) plus
`FieldContribution`; `math` in full (the `Scalar` trait and the forward-mode `Dual`, since everything
depends on it); the infra — `.devcontainer/`, `.gitignore`, `.github/workflows/ci.yml`.

**Plan.**
1. Create the workspace `Cargo.toml` and the sixteen crates with their dependency edges wired per
   `DESIGN.md` §1 (deps point up only) — each crate a stub that compiles.
2. Implement `math`: `Scalar` (the numeric abstraction over `f64`/`f32`/`Dual`) and forward-mode
   `Dual` (value + tangent), with the arithmetic ops.
3. Declare the seam traits with their signatures and doc-contracts (`DESIGN.md` §3), no impls.
4. Author the infra files (below) and get CI green: `fmt`, `clippy -D warnings`, `cargo test` (trivially).

**Tests.**
- *Unit:* `Dual` arithmetic (sum/product/chain rule) matches analytic derivatives; `Scalar` ops agree
  across `f64` and `Dual` on the value channel.
- *Integration:* the workspace builds; every crate's public surface is reachable from `generate`.
- *E2E:* CI pipeline runs to green (fmt + clippy + test + spec build) on the skeleton.

**Exit.** `cargo test --workspace` green; CI green on all jobs; `math`'s `Dual` verified against
analytic derivatives (the foundation `analysis` will rely on).

---

## M1 — Physics spine (the walking skeleton)

**Goal.** One mass, one gradiometer, one **validated number** — the forward model end to end on the
simplest case.

**Builds.** `gravity` (the analytic differential-first kernel, CPU `f64`, point/small-cloud);
`instrument` (a single `Detector`, the four ballistic arms built once, the `PropagationIntegral`
reference `PhaseModel`); `source` (one prescribed closure — static, or a fixed linear drift);
minimal `state` (a bundle of `time`, `signal`, `source_position`, `meta`); minimal `scenario`
(direct construction) and `generate.run` (wire source→gravity→instrument→bundle).

**Plan.**
1. `gravity`: the per-element gradient kernel `Γ = −Gm/r³(𝟙 − 3d̂d̂ᵀ)`, trace-free by construction;
   evaluate the potential/gradient at a point from a cloud.
2. `instrument`: build the four arms from launch geometry; `PropagationIntegral` over `[t−2T, t]`
   giving `δφ` per interferometer, then the gradiometer double-difference `ΔΦ`.
3. `source`: a `SourceDynamics` impl for a prescribed pose (static/linear).
4. `state` + `scenario` + `generate.run`: assemble the minimal bundle for a one-source, one-detector
   scenario.
5. Wire an anchor test.

**Tests.**
- *Unit:* `Γ` symmetric and trace-free in vacuum; `1/r³` fall-off; arm endpoints match launch
  geometry; `ΔΦ` linear in source mass (`ΔΦ(αm)=αΔΦ(m)`).
- *Integration:* `source → gravity → instrument` produces a finite `ΔΦ`; `generate.run` returns a
  bundle with correct shapes.
- *E2E:* a static/quasi-static configuration reproduces a **published anchor** (concrete wall
  ≈ 50 µrad) to tolerance (spec, `instrument.md` exit).

**Exit.** `Scenario → StateBundle` runs; one anchor reproduced; `ΔΦ` mass-linear. The spine exists.

---

## M2 — Source motion & shape core

**Goal.** Real trajectories, and geometry→mass done properly — the motion anchors run on
properly-voxelised clouds (replacing M1's ad-hoc lattice).

**Builds.** The closed-form `Path × Timing × Orient + Placement` factoring (`source.md` §3): linear,
oscillation, orbit/flyby paths; constant/eased timing; bundle kinematics filled (`source_velocity`,
`source_accel`). Plus the **`shape` crate core** (`design/shape.md`): the `Solid` seam, the lattice
voxeliser (renormalise mass exactly, recentre to CoM, deterministic order), the primitives with
their analytic moment oracles, `Union`, and the unit-mass cloud cache. Mesh import is **not** here
(M10).

**Plan.**
1. Implement the closed-form path library as `SourceDynamics` closures.
2. Derive velocity/acceleration analytically for each (fill the motion fields).
3. Extend `generate` to populate the per-tick kinematics.
4. Add the motion anchor tests.

**Tests.**
- *Unit:* voxelised sphere/cuboid: mass exact, CoM at origin, `C` converges to the analytic table
  (halving `h` halves the error); bit-identical clouds across runs; each path's position/velocity/acceleration are mutually consistent (finite-difference of
  position matches the analytic velocity); oscillation period/amplitude correct.
- *Integration:* a moving source yields a *time-varying* `ΔΦ` of the expected shape over `T`.
- *E2E:* the **shell theorem** — a voxelised sphere's external field matches a point mass to
  tolerance; the moving-mass anchor (10 kg, D = 10 m, 2.5 m/s → ≈ 7 mrad) and the oscillation anchor
  (1 kg, D = 5 m → ≈ 2 mrad) reproduced to tolerance.

**Exit.** Both motion anchors pass on voxelised clouds; the shell theorem holds; kinematic fields
consistent with the trajectory.

---

## M3 — The detector array

**Goal.** Multi-detector signal — the geometry that makes localisation possible.

**Builds.** `DetectorArray` and `detector_placement (D,7)`; `signal (T,D)`; the second `PhaseModel`
(`QuasiStaticGradient`, `ΔΦ ≈ C·Γ_zz·Δr`) validated against the reference.

**Plan.**
1. Generalise `instrument` from one detector to an array; build arms per detector.
2. Fill `signal (T,D)` and `detector_placement` in the bundle.
3. Implement `QuasiStaticGradient` and gate PhaseModel choice by config.
4. Array/geometry and PhaseModel-agreement tests.

**Tests.**
- *Unit:* `QuasiStaticGradient` agrees with `PropagationIntegral` in the uniform-field limit
  (`instrument.md` exit); `detector_placement` round-trips into the arm geometry.
- *Integration:* a two-detector array gives per-detector `ΔΦ` differing with baseline vs source
  distance (the geometric signature localisation exploits).
- *E2E:* a known two-detector geometry reproduces the differential signal magnitude expected from
  the published anchors, per detector.

**Exit.** `signal (T,D)` correct for `D>1`; the two PhaseModels agree in-limit; geometry is
first-class.

---

## M4 — Rotation

**Goal.** Tumbling bodies — the pure-quadrupole probe, and the shape/inertia descriptors.

**Builds.** The hand-written symplectic Euler + quaternion integrator, generic over `Scalar`
(`source.md`, *not* an off-the-shelf physics engine); the pendulum (chaotic) and free-rotation
(torque-free Euler top) motions; the `gravity` reduction to the shape descriptors (`source_inertia`,
`source_moments`, `source_axes`, `source_quadrupole`) from the one second moment **C**.

**Plan.**
1. Implement `step<S: Scalar>` — the generic symplectic integrator with `fine_dt` substeps, autodiff-
   compatible.
2. Add `Libration`/`FreeRotation` orientation motions driving it.
3. Implement the second-moment reduction: `C → I, Q`, principal moments/axes.
4. Fill the shape descriptors and the rotational kinematics (`source_angular_velocity` etc.).
5. Conservation and tumble tests.

**Tests.**
- *Unit:* quaternion norm preserved under integration; the free top conserves angular momentum and
  energy to tolerance over long runs; `I` and `Q` share principal axes (same **C**); `C` reductions
  correct on analytic shapes (sphere, rod, plate).
- *Integration:* a `FreeRotation` source drives `source_orientation`/`source_angular_velocity`
  consistently (ω is the analytic angular velocity, matching `d(orientation)/dt`).
- *E2E:* the intermediate-axis (Dzhanibekov) flip appears for an asymmetric top; a rotating body
  about its CoM produces a pure-quadrupole signal (monopole fixed, dipole zero).

**Exit.** The integrator conserves the invariants; the shape descriptors reduce correctly; rotation
is first-class in the bundle.

---

## M5 — Channels & decomposition

**Goal.** Everything that is not the target masses, and the ground-truth breakdown of the signal.

**Builds.** `uldm` (closed-form common-mode phase); `noise` post-hoc stack (shot, vibration); the
atmospheric GGN **field source** (`FieldContribution`, stochastic `δρ`, in-pass); the signal
decomposition by superposition (`signal_targets`/`_atmospheric`/`_uldm`/`_noise`, `generate.md` §4).

**Plan.**
1. `uldm`: the closed-form transition-frequency oscillation, broadcast identically across detectors.
2. `noise`: the additive stack with defined order; record `signal_noise`.
3. Atmospheric GGN as a `FieldContribution` summed into the potential (in the forward pass, not
   post-hoc).
4. `generate`: the decomposition path — run per field-source group and sum; gate by `FieldSet`.
5. Decomposition, common-mode, and noise-recovery tests.

**Tests.**
- *Unit:* ULDM phase identical across detectors; a `NoiseSource` is additive and zero-mean
  (shot/vibration); the atmospheric field's correlation length behaves as specified.
- *Integration:* toggling `decomposition` runs the extra passes and only then; ULDM/noise appear in
  their channels, atmospheric in the field channel (not post-hoc noise).
- *E2E:* `signal_targets + signal_atmospheric + signal_uldm + signal_noise == signal` to tolerance
  (exact by linearity, `generate.md` §11); `signal − signal_noise` recovers the clean signal; ULDM
  is common-mode, atmospheric partial-common-mode across the array.

**Exit.** The decomposition superposes to `signal`; the atmospheric/noise boundary holds; the
channels are correct.

---

## M6 — Compute backend, GPU & batch/stream

**Goal.** Scale — the fast path and reproducible dataset generation, validated against the reference.

**Builds.** `ComputeBackend` formalised (`CpuBackend` = bit-exact reference; `WgpuBackend` =
differential-first `f32`, WGSL); the two-pass execution (poses, then field+phase, `compute.md`);
`config` (typed schema + distribution primitives); the `Prior` (batch sugar); realistic
`Schedule`s (gaps, jitter) with the contamination `mask`; the **Lomb–Scargle periodogram** (the
`state` derived field — non-uniform sampling makes LS, not FFT, the right estimator);
`generate.stream` (memory-bounded batching).

**Plan.**
1. Formalise `CpuBackend` (rayon, `f64`) as the oracle behind the current direct calls.
2. `WgpuBackend`: WGSL kernels for the differential-first field/phase; Pass 1 pose generation
   on-device with `fine_dt` substeps; Pass 2 field+phase.
3. `config` + `Prior` + counter-based seeding.
4. `generate.stream`: batch dispatch, bounded memory.
5. CPU≡GPU, batch-invariance, reproducibility tests.

**Tests.**
- *Unit:* the counter-based RNG is deterministic and stable under field-extension; a WGSL kernel
  matches its Rust reference on a fixed input.
- *Integration:* `Prior::sample` yields runnable scenarios; `generate.stream` holds ≪ the dataset.
- *E2E:* `CpuBackend ≡ WgpuBackend` across the anchors to tolerance; a batched bundle equals the same
  `Scenario` run alone (batch-invariance); `(Prior, seed)` replays an identical batch; Lomb–Scargle
  recovers a planted line (an oscillating source and the ULDM line) on a **gappy** schedule.

**Exit.** GPU matches the CPU reference; batching is invariant and reproducible; datasets stream at
scale. **The core engine is complete.**

---

## M7 — SDK (Python/torch)

**Goal.** The intended workflow — drive the engine from Python and receive torch tensors.

**Builds.** `sdk` (PyO3) — bindings for `run`/`stream` and `Scenario`/`Prior`/`FieldSet`; the torch
hand-off via DLPack (zero-copy on CUDA/CPU); GIL release; the streaming iterator (`sdk.md`).

**Plan.**
1. PyO3 module exposing the constructors and the two verbs.
2. DLPack hand-off from the bundle's contiguous fields to torch tensors.
3. `Python::allow_threads` around the Rust work; the Python streaming iterator.
4. Optional fields → `None`; error conversion; packaging (maturin).
5. Python fidelity/zero-copy/reproducibility tests.

**Tests.**
- *Unit (Python):* an unset `FieldSet` flag yields `None`; Rust errors surface as Python exceptions
  (no panic crosses).
- *Integration (Python):* `run`/`stream` produce tensors of the right shape/dtype; the stream is
  memory-bounded under a loop.
- *E2E (Python):* the Python tensors match the Rust bundle value-for-value; a shared-memory (CUDA/CPU)
  tensor reflects the Rust buffer without a copy; a seeded stream replays identical tensors; Python
  threads progress while a batch computes (GIL released).

**Exit.** `import cavendish`, run/stream to torch tensors, faithful and copy-free where possible,
reproducible.

---

## M8 — Analysis (CRB)

**Goal.** Identifiability — the Cramér–Rao floor from the differentiable forward model.

**Builds.** The `analysis` crate — the `Dual` path through `gravity`/`source`/`instrument` via
`CpuBackend`, `J = ∂(signal)/∂(params)`, Fisher `= JᵀΣ⁻¹J`, `CRB = Fisher⁻¹`, and the array-geometry
optimisation (`state.md` §7, `scenario.md` §7).

**Plan.**
1. Assemble `J` by running the forward model in `Dual` (per-parameter tangents).
2. Form and invert the Fisher matrix; report CRB and resolvability.
3. The array-placement objective against the CRB.
4. Validation against an analytic Fisher case.

**Tests.**
- *Unit:* `J` matches finite-difference of the forward model to tolerance; Fisher is symmetric PSD.
- *Integration:* `analysis` consumes the bundle shape + the `Dual` forward model without pulling the
  physics into `state`.
- *E2E:* the CRB matches a known-Fisher analytic case; localisation precision scales with
  baseline/source-distance as the geometry predicts.

**Exit.** CRB verified analytically; geometry can be scored/optimised against it.

---

## M9 — Viewer

**Goal.** Make a run *legible* — eyes on the forward model.

**Builds.** The `viewer` crate — egui + wgpu (shared with `compute`): the 3D scene (world-frame
clouds, array, field), the 2D panels (signal, periodogram), the time scrubber; live run + loaded
bundle; fails-soft (`viewer.md`).

**Plan.**
1. wgpu 3D scene: cloud (placed by pose), array markers, optional field arrows/slice.
2. egui panels + `egui_plot` for signal/periodogram; the `T` scrubber tying pose to the plot cursor.
3. Data path: live `generate.run` with a viewing `FieldSet`, or a loaded serialised bundle; on-demand
   field slice via `gravity`.
4. Robustness (fails-soft) and the interactivity loop.

**Tests.**
- *Unit:* the scrubber maps a slider index to the correct `time`; a missing optional field disables
  its panel rather than erroring.
- *Integration:* a `Scenario` runs and its cloud/array/signal render; both field modes (stored grid,
  on-demand slice) work.
- *E2E (review-grade):* a free-rotation source visibly tumbles through its flip; tweak-and-rerun
  updates the view; a run error degrades to a message, never a crash.

**Exit.** A run renders coherently; scrubbing is consistent; the tool fails soft.

---

## M10 — Mesh import

**Goal.** Arbitrary geometry into the body dictionary — imported meshes indistinguishable
downstream from primitives. Off the critical path; schedulable any time after M2.

**Builds.** `shape`'s mesh path (`design/shape.md` §5): STL/OBJ/glTF parsing (feature-gated),
mandatory explicit scale, the watertightness classifier, the two inside/outside strategies
(rasterise+flood for watertight; fast generalised winding numbers for open/non-manifold, with the
loud-failure diagnostic), the divergence-theorem volume cross-check, and the `MeshReport`.

**Plan.**
1. Thin parsers → indexed triangle soup; `MeshImport { scale, … }` (no unit guessing).
2. Edge-manifold watertightness check + `MeshReport` diagnostics.
3. Watertight fast path: surface rasterisation, exterior flood fill, boundary sub-sampling via
   BVH parity rays.
4. Robust path: fast generalised winding numbers (BVH + far-field approximation); ambiguity
   diagnostic; `AmbiguousInterior` on genuinely dubious meshes.
5. Volume cross-check; wire into the dictionary/cache.

**Tests.**
- *Unit:* winding number correct inside/outside/near-surface on a closed cube mesh; the
  watertightness check flags a punctured mesh; scale is mandatory (`ScaleMissing`).
- *Integration:* a sphere STL voxelises to the primitive sphere's moments at equal `h`
  (mesh ≡ primitive); a watertight mesh's voxelised volume matches its divergence-theorem volume.
- *E2E:* a dirty (open) mesh takes the robust path, emits the diagnostic, and either voxelises
  sanely or fails loudly — never silent garbage; a dictionary mesh voxelises once and serves many
  mass draws (cache + linearity).

**Exit.** Mesh ≡ primitive to tolerance; dirty meshes are handled robustly and loudly; the
dictionary caches.

---

## M11 — The world seam

**Goal.** One source seam. A scene is a list of `WorldSource`s; the rigid body is one implementation
and keeps its amortised evaluation; the forward-model spine is not edited.

**Builds.** The `world` crate (`WorldSource`, `Eval`, `Channel`, `Track`, `Rigid`, `Ambient`);
`Scenario` over a list of sources; `generate::run` routing by channel and socket; `generate::batch`
and `write_dataset`.

**Exit.** Twenty-four v1 scenes serialise **byte-for-byte** as on `main`
(`generate/examples/bundle_hash.rs`); the spine crates have an empty diff; sources superpose (≤1e-12);
a mixed scene decomposes exactly (≤1e-10). Brief: `milestones/M11-world-seam.md`. **Implemented.**

---

## M12 — The physics layer

**Goal.** Mass that is not one prescribed rigid body — articulated and deforming bodies, bodies that
fall and collide, vibrating structure, convecting air — entering through the M11 seam, each layer
checked against its own physics.

**Builds.** The `physics` crate: the cube cell kernel; the world grid, cloud-in-cell deposition and
`GridField` with its linear-response evaluation; the Boussinesq fluid and the equation-of-state
density read; `Parcels` (rigid / skinned / displaced), `Walker`, `Plate`; rigid dynamics with impulse
contact, sleep and recorded tracks; `simulate_air` (one-way coupling).

**Exit.** Grid fast path equals the propagation integral (≤1e-9); heated-cavity Nusselt numbers within
2% of de Vahl Davis; a static temperature anomaly gravitates (the Boussinesq guard); parcel mass
conserved bit-for-bit and the rigid case equal to v1 (≤1e-11); a settled body's pose bit-constant;
dynamics → air → forward model runs end to end. Brief: `milestones/M12-physics.md`.
**Implemented on the CPU**; GPU execution is not part of it.

---

## M13 — The scene viewer

**Goal.** Make a v2 scene legible: bodies animating in continuous time, the air as a slice and a
volume, what each gradiometer is listening to, and the signal split by channel under one cursor.

**Builds.** A scene replay (world clouds and air frames on a playback axis, beside the unchanged
`StateBundle`); one continuous clock; slice, glyph and ray-marched volume passes; the detector response
map; background runs; a predicted-track overlay. Still `egui` + `wgpu`.

**Exit.** Brief: `milestones/M13-viewer-v2.md`. **Planned — not built.**

---

## M14 — The voxel core

**Goal.** One sparse hierarchical representation of mass and matter — the voxeliser emits it, the
gravity read walks it, the fluid coarsens on it, contact samples it, the viewer draws it.

**Builds.** The `voxel` crate (`VoxTree`: linear octree from Morton keys, value sidecars, moments); a
top-down voxeliser through `Solid::cover`; the lossless merge of homogeneous interior cells and the
cube series kernel; a tree walk bounded by a stated phase tolerance; `TreeBody`; per-bone trees for
articulated bodies; a hierarchical grid response; obstacle stamping and contact from voxels.

**Exit.** The tree expanded to finest cells equals `shape::voxelise` bit for bit; merged equals
unmerged (≤1e-12); walk error ≤ the stated tolerance with no steps in time; v1 bundles byte-identical;
a box rests on a box bit-constant; and a measured speed-up at equal error, or the flat path stays the
default. Brief: `milestones/M14-voxel-engine.md`. Design: `design/voxel.md`. **Planned — not built.**

---

## M15 — Per-tower timing and the scene spec

**Goal.** Towers that fire on their own clocks, and one description of a scene that a file, the GUI,
the console and Python all read and write.

**Builds.** Per-tower `Timing` and instrument overrides on `Scenario`; `StateBundle.detector_time`
`(T, D)` (a data-contract change — spec first); the `spec` crate: `SceneSpec`, RON files, paths,
schema, and a stage-level `diff` for minimal re-runs.

**Exit.** Synchronous scenes bit-identical field for field; a staggered array runs and decomposes
exactly; every library scene built from its spec equals the hand-written one. Brief:
`milestones/M15-array-timing-and-scene-spec.md`. **Planned — not built.**

---

## What follows

`milestones/V2-HANDOFF.md` carries the state of the v2 branch and the ordered remainder: GPU execution
of the fluid and of non-rigid sources (with float-float accumulation where f32 is not enough), the
Python surface for scenes, further physics, and the Gradar datasets.

---

## The implementation briefs

Each milestone above has a **self-contained implementation brief** in `milestones/`
(`M0-scaffolding.md` … `M15-array-timing-and-scene-spec.md`): requirements traced to tests, the design and its
diagrams, the governing equations, pseudocode for the core algorithms, unit/integration/e2e tests
with concrete tolerances, and the exit table. A Claude Code session is pointed at one brief (plus
the relevant `design/*.md` and the spec) and works it to green. The cross-cutting `reference-port.md` brief
defines the independent oracle the anchors are checked against.
