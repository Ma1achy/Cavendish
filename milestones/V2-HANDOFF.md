# Cavendish v2 — handoff brief

> For the next session. Branch `v2-world-physics` (off `main` at `56df4ab`), pushed, **not merged**.
> `main` is untouched. This brief says what the branch contains and how far to trust it, what is not
> done, and the remaining work as instructions. Read it with the repo skills
> (`.claude/skills/`): the conventions, doc-sync and milestone-workflow rules still apply.
>
> If you would rather start from a clean `main`, delete the branch — sections 3 and 4 still stand as
> the plan.

---

## 1. What the goal is

A generic simulator of mass moving around a gradiometer array — solid objects, articulated bodies,
vibrating structure, air — feeding the existing forward model, producing complete state tensors for
ML (Gradar tracking, and paired data for a gravity-gradient-noise denoiser). GPU-first for
throughput, with emulated double precision (float-float) where f32 is not enough. Runs natively on
the host, no container.

The governing decision from the design conversation: **keep v1's validated forward-model spine as the
oracle; rework only how sources enter it; one source interface, not several backends.**

---

## 2. What the branch contains

Verified on the branch head: `cargo fmt --check` clean, `cargo clippy --workspace --all-targets` with
`-D warnings` clean, `cargo test --workspace` **159 passed, 0 failed** (115 are v1's, unchanged), and
the 6 GPU-marked tests pass on the software adapter. Not run: `clippy --all-features` (needs
libpython), the Python suite, the LaTeX build, and anything on a real GPU.

The spine crates — `math`, `gravity`, `instrument`, `analysis`, `compute`, `reference` — have **no
diff against `main`**.

### 2.1 The unified source seam (`crates/world`, `scenario`, `generate`)

- `world::WorldSource` — one trait for anything massive. `eval()` returns which socket of the phase
  integral it uses: `Eval::Rigid(&dyn SourceDynamics)` (the amortised inverse-pose path) or
  `Eval::Field(&dyn FieldContribution<f64>)`. Also `world_cloud_at(t)`, `channel()` (`Target` /
  `Ambient`), optional `track_at`/`body_cloud` (which sources appear on the bundle's `S` axis), and an
  optional `per_ifo_fast` (an exact cheaper reformulation of the propagation integral).
- `world::Rigid` wraps any `SourceDynamics`; `world::Ambient` moves a source to the ambient channel.
- `Scenario` now holds `sources: Vec<Box<dyn WorldSource>>`. `Scenario::new(Box<dyn SourceDynamics>, …)`
  is unchanged for v1 callers; `Scenario::scene(vec![…], …)` and `.with_source(…)` are new.
- `generate::run` routes sources by channel and socket. `generate::batch(n, root, build)` runs
  arbitrary scene builders across cores in index order; `generate::write_dataset` stacks bundles as
  `.npy` (`state::save_npy`, no dependency).
- `StateBundle` is **unchanged** (no field added).

**The merge gate held.** `crates/generate/examples/bundle_hash.rs` serialises 24 v1 scenes (both phase
models, all field sets, free rotation, libration, atmosphere, ULDM, noise, tilted detector, jittered
schedule); the output is byte-for-byte identical on `main` and on the branch. Re-run it on both
revisions after any change to `world`, `scenario` or `generate`.

### 2.2 The physics layer (`crates/physics`, new, L2)

| Module | What it is | Checked against | Measured |
|---|---|---|---|
| `kernel` | Closed-form potential and gradient tensor of a homogeneous cube (Nagy prism); point law beyond 8 cells | brute-force quadrature; finite differences; Poisson trace inside the cube; known centre value | ≤2e-5 vs quadrature; ≤1e-5 jump at the switch; error falls ~16× per doubling of distance |
| `grid` | `Grid3`, cloud-in-cell deposition, `GridField` (frames of cell masses → `FieldContribution`), and a linear-response fast path | deposition total vs input; identity at cell centres; fast path vs `PropagationIntegral::per_ifo` | partition of unity ≤1e-14; identity **bit-exact**; fast path = reference to **≤1e-9** |
| `fluid` | 3-D Boussinesq on a MAC grid: MacCormack advection, explicit diffusion, MIC(0)-PCG projection, film / isothermal / adiabatic surfaces; `Air` equation of state | divergence after projection; hydrostatic rest; de Vahl Davis heated cavity | Nu **1.1205** (benchmark 1.118) at Ra=10³, **2.283** (2.243) at Ra=10⁴, 32² grid; rest ≤1e-9 m/s |
| `body` | `Parcels`: a rest voxelisation carried by a map — `rigid`, `skinned` (skeleton + per-parcel weights), `displaced` (displacement field). `Walker`, `Plate` (Kirchhoff modes) | rigid parcels vs v1 rigid source; one-bone skeleton vs rigid transform; mass through poses | phase ≤1e-11 vs v1; mass conserved **bit-for-bit** |
| `rigid` | Velocity-Verlet + `source::step` rotation; sequential-impulse contact (sphere/box–plane, sphere–sphere, sphere–box); sleep; `RecordedRigid` replays as a plain `SourceDynamics` | analytic ballistic; \|L\| and energy; restitution; elastic exchange | fall ≤1e-10 m; \|L\| ≤1e-10 over 20 s; rebound within 1 cm; slept pose bit-constant |
| `scene` | `simulate_air`: room air stirred and heated by any `WorldSource` (one-way), recorded as the equation-of-state mass anomaly | plume rises; anomaly negative and growing; static thermal field gravitates; isothermal stirring does not | see `crates/physics/src/scene.rs` tests |

`crates/generate/tests/world.rs` covers the whole path: superposition of sources (≤1e-12), the rigid
null limit through parcels (≤1e-11), exact decomposition of a mixed scene (≤1e-10) with the grid fast
path checked against quadrature (≤1e-6 on the difference of two runs), and dynamics → recorded track →
heated air → forward model.

### 2.3 Design choices that differ from the scratchpad, and why

- **Solids are not splatted to a grid for the gravity read.** The transported parcels *are* the world
  cloud; the forward model integrates them directly. Exact, no one-cell blur, mass conserved trivially.
  Cloud-in-cell deposition is used where it is actually needed: stamping bodies onto the fluid grid.
- **The air field is the anomaly `ρ(T) − ρ(T₀)`, not full ρ.** A finite box of full-density air would
  read as a spurious static block; uniform ambient air beyond the room contributes nothing
  time-varying. `Parcels::in_fluid` gives a body its mass in excess of displaced air when wanted.
- **The fluid response is a dot product.** Cells and detectors are both fixed, so the propagation
  integral of a grid field is a fixed linear functional of the frames. Built once per detector, then
  each measurement costs one pass over the frames. This largely removes the per-tick field evaluation
  from the GPU to-do list; the fluid *solve* is what still needs the GPU.
- **Film heat enters through `|∇φ|`** (surface-area density of the deposited occupancy) rather than
  staircase faces, so a body's heat output does not depend on grid alignment. It reads ~8% low at four
  cells per radius.
- **The dev container is removed** (`.devcontainer/`, `scripts/view.sh`); README describes the native
  path. The M0 brief still mentions it as history.

### 2.4 Throughput measured here (2 cores, CPU)

- Point-mass scenes (24 measurements × 5 detectors): 30 000 in 91 s.
- Fluid: 48 000 cells ≈ 59 ms/step (advection 26, projection 28); a 19 200-cell room ran 52 s of
  simulated time in 9.6 s.

### 2.5 Tracking proof of concept (`examples/tracking_dataset.rs`, `python/poc/track.py`)

A 3-layer MLP trained on 27 000 **point-mass** passes, tested on the *same* 300 passes replayed with
different bodies. Horizontal position error:

| Test set | Median | 90th pct | RMS |
|---|---|---|---|
| point mass | 0.022 m | 0.052 m | 0.037 m |
| sphere | 0.022 m | 0.052 m | 0.037 m |
| cuboid | 0.027 m | 0.063 m | 0.044 m |
| walking figure | 0.025 m | 0.056 m | 0.040 m |
| point mass + radiator-heated room | 0.065 m | 0.173 m | 0.121 m |

Predict-the-mean baseline: 3.62 m RMS. Scope of the claim: one object, mass known to 50–90 kg,
straight constant-velocity passes, 1 mrad shot noise, one fixed five-tower layout. It shows the
pipeline works end to end and that point-trained tracking transfers to extended and articulated
bodies in this regime; it is not evidence about realistic scenes.

One physics result worth keeping: a 70 °C radiator ~1 m from a tower produced a peak convective phase
of **2.2e-2 rad** against a 1e-3 rad shot floor, and tripled the tracking error. Convective noise from
a strong nearby heat source is not negligible for this instrument. A warm 0.4 m block gave ~1e-4 rad.
Neither figure is validated against measurement.

---

## 3. What is not done

**Nothing new runs on the GPU.** No float-float kernel was written. The fluid solver, parcel bodies
and grid fields are CPU-only; `compute::EvalBatch` and `forward.wgsl` do not know about `WorldSource`.

**Docs.** Written after the code and synced in one change: `design/world.md`, `design/physics.md`,
the retroactive briefs `milestones/M11-world-seam.md` and `M12-physics.md`, the viewer plan
`milestones/M13-viewer-v2.md`, `DESIGN.md` (§1 layering, §2, §3.1a, §6), `MILESTONES.md`, the README,
the conventions skill, `design/{generate,scenario,state,viewer}.md`, and the spec (scope, crate map,
seams, a `WorldSource` contract, and a new appendix subsection on what was built). The spec compiles
clean (`latexmk` exit 0, no undefined references, no overfull boxes). Not touched:
`design/compute.md` and `design/sdk.md`, because nothing in those crates changed — which is itself the
gap listed next.

**Not exposed or not checked:**
- Python SDK: multi-source scenes and physics are Rust-only. The Python tests were not run.
- Viewer: untested with more than one source; ambient fields are not drawn.
- CRB/`analysis`: still parametric rigid sources only (by design, but unstated in the docs).

**Physics limits:**
- Fluid is validated only in 2-D at Ra ≤ 10⁴. At room scale (Ra ~10⁹) it is an under-resolved,
  model-free simulation: plausible, not validated. Closed box only — no openings, ventilation or
  inflow. The 1.24 m/s radiator plume speed is unchecked.
- Rigid: no box–box contact (refused with a panic), no joints or constraints, no two-way coupling
  (the force/torque slot on `RigidBody` exists and is unfilled).
- Skinning weights: bone-proximity only. Laplace diffusion from authored surface weights — the method
  the design settled on — is not built, and there is no skinned-mesh import to feed it.
- Parcels use point elements (fine at range: a cube has no multipole below the hexadecapole). The
  cube kernel is used for grid fields only.
- No octree of any kind — now planned as its own milestone (`milestones/M14-voxel-engine.md`,
  `design/voxel.md`), not a deferred note.
- One measurement schedule for the whole array; no per-tower timing or instrument settings, and no
  scene file format (`milestones/M15-array-timing-and-scene-spec.md`).
- Vibration is the Kirchhoff plate and v1's rigid `Path::Oscillation`. Room acoustic modes are not built.
- None of the new physics has an independent oracle of George's kind; it is checked against analytic
  results and one published benchmark.

**v1's deferred items are untouched:** the config-seating refactor, George's `week_pipeline` /
`thinning` / `nighttime` cases, CUDA zero-copy, the ULDM amplitude axis.

---

## 4. Remaining work, in order

Each step is one branch and one PR per the milestone workflow. Write the brief (requirements traced to
named tests, exact tolerances, exit table) before the code.

### Step 1 — Review and merge the branch

The design docs and briefs now exist (above). What remains before a PR into `main`: read them against
the code; run what could not be run here — `cargo clippy --workspace --all-targets --all-features`,
the Python suite, the GPU-marked tests on a real device; re-run `bundle_hash` against `main`.
**Exit:** CI green on the PR, including the GPU and Python jobs.

### Step 1b — The voxel core, phases A–C (before any GPU source work)

`milestones/M14-voxel-engine.md`. The tree, the top-down voxeliser, the lossless merge with the cube
series, and the tolerance-bounded walk, on the CPU in `f64`. It comes before step 3 because the device
encoding of a source should be the tree; porting the flat cloud first means porting twice.
**Exit:** the brief's phase A–C lines, including the measured speed table.

### Step 1c — Per-tower timing and the scene spec

`milestones/M15-array-timing-and-scene-spec.md`. Independent of 1b; needed by the viewer's
configuration panels. A data-contract change: spec first.

### Step 2 — GPU: the fluid solve

The measured cost is the fluid step, split evenly between advection and the pressure solve. Port to
WGSL compute: advection and diffusion are embarrassingly parallel; replace MIC(0)-PCG (inherently
sequential) with red-black Gauss–Seidel or geometric multigrid. f32 throughout is expected to
suffice here — the fluid's own truncation error is ~1%.
**Exit:** GPU fluid reproduces the CPU heated-cavity Nusselt numbers within 1%; post-projection
divergence ≤1e-4 of pre-projection; plume test passes on GPU; speed-up measured on the M3 (Metal) and
the 2080 (Vulkan), not on a software adapter.

### Step 3 — GPU: sources and float-float

(M14 phase F: with the voxel core in place, the first bullet becomes "upload each rigid body's and
each bone's tree once and walk it on-device"; only blend-region parcels are streamed per tick.)

- Per-tick parcel clouds in `forward.wgsl`: stream posed positions (or bone transforms plus weights and
  skin on-device) instead of one static cloud. Differential-first per element, as the rigid kernel is.
- Grid-field response: the `W·m` dot product on-device, accumulated in float-float.
- A float-float (two-f32) library in WGSL: two-sum, two-product, accumulate. **Every kernel that uses
  it gets a parity test against the f64 CPU path**, because driver fast-math can reorder the
  error-free transforms and silently break them (Metal is the one to worry about).
- Extend `EvalBatch` with an encoding for field and parcel sources; route `generate::batch` through
  `ComputeBackend`.
**Exit:** CPU≡GPU ≤1e-4 in f32 and ≤1e-9 where float-float is used, on the anchors and on
`tests/world.rs` scenes, **on both real devices**.

### Step 3b — The scene viewer (independent of steps 2–3)

(Phases A–F as below; the full interface — spec-bound panels, per-tower lanes, console, dataset
builder — is §9 of the brief, phases G–J, and needs step 1c.)

`milestones/M13-viewer-v2.md`: a scene replay beside the bundle, one continuous clock, the air as a
slice and a ray-marched volume, each detector's response map, the decomposition panel, background runs
and a predicted-track overlay — on the existing `egui` + `wgpu` stack. Phases A, B and E make it
useful; C and D make it good.

### Step 4 — SDK

Expose `Scenario::scene`, the physics sources and `write_dataset` to Python; add bundle-fidelity tests
for a multi-source scene. Decide whether per-source target channels `(S, T, D)` belong in the
`StateBundle` (useful for multi-target training; it is a data-contract change, so spec first).

### Step 5 — Physics, as scenes demand it

In rough order of value for Gradar: open boundaries and ventilation; convergence of the cavity
benchmark at Ra=10⁵ and 10⁶ (Nu 4.519, 8.800) and a 3-D case; volumetric skinning weights by Laplace
diffusion plus skinned glTF import; room acoustic modes as an analytic grid field; box–box contact
(SAT + clipping, manifold reduction) with the rest-determinism test extended to stacks; two-way
coupling only if a water or near-buoyant scene needs it. Box–box and arbitrary-shape contact, the
per-bone trees and the hierarchical grid response are M14 phases D–E. Adaptive refinement of the air
is the last item and is gated on a measurement (`design/voxel.md` §9).

### Step 6 — Gradar datasets

Multi-target scenes, unknown mass, non-straight paths, realistic schedules and noise stacks; the
paired (signal, clean) denoiser set with convective noise in the ambient channel. Check the
convective-noise band against the tracking band before investing in a learned denoiser.

---

## 5. Running what exists

```
cargo test --workspace                                   # 159 tests
cargo run --release -p generate --example bundle_hash    # the v1 bit-identity gate (diff vs main)
cargo run --release -p physics  --example fluid_timing   # fluid profile on 48 000 cells
cargo run --release -p generate --example tracking_dataset -- <dir>   # ~3.5 min on 2 cores
python python/poc/track.py <dir>                         # needs numpy + torch; ~5 min on CPU
```

The earlier design scratchpad (`physics_engine_scratchpad.md`) is superseded by this brief and the
code; where they disagree, section 2.3 says which way it went and why.
