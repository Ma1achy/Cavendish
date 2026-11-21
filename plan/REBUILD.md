# Cavendish — the rebuild, from a blank slate

> **Status: plan.** Supersedes the v2 branch as the way forward. The v2 work (`v2-world-physics`) and its
> briefs (M11–M18, `design/voxel.md`) stay as the record of what was learned and as the source of most
> of the detail below; nothing from them is ported as code unless a slice says so. The validated v1
> forward model on `main` is kept as an **oracle**, consulted by tests, never extended.
>
> **To the agent building this:** this plan is the floor, not the ceiling. Where you see a way to make
> it more accurate, faster, simpler, better-looking or more useful, do it, and say so in the slice's
> notes. Where the plan is wrong or unclear, do the sensible thing and leave one line saying what you
> chose and why. Ask the owner only about money, publishing, deleting history, or decisions that
> cannot be undone.

---

## 1. What Cavendish is

A voxel physics engine and viewer built around one instrument: an array of atom-interferometer
gravity gradiometers (AION-10 geometry). Anything with mass — people, vehicles, machines, buildings,
terrain, air, water — is matter in one sparse voxel world; the engine moves it, and a forward model
turns it into what each tower would read. It makes training data for Gradar (tracking, denoising),
and it is a tool a person or an agent can explore interactively, at scales up to a small town.

## 2. Principles (each one paid for in v1 or v2)

1. **A working app from the first day, better every slice.** Every slice ends with something new you
   can see and do in the GUI, the CLI and the MCP server. No slice is "infrastructure only".
2. **One representation of matter.** A sparse voxel tree (linear octree from Morton keys, values in
   sidecar arrays) for bodies, structure, terrain, air and water. No second path beside it.
3. **One error budget, in radians.** Every approximation — tree walk, level of detail, quadrature,
   precision, adaptive grids — is bounded or audited against a phase tolerance, and the bundle records
   what it was.
4. **Oracles before features.** The v1 forward model and George's anchors are the reference; every new
   path is tied to one by a test with a stated tolerance. A test that cannot fail is not a test.
5. **One engine API.** The GUI, the CLI, the MCP server and Python all drive the same command/query
   interface. Every edit is a command; every command is logged, undoable and replayable.
6. **GPU first, f64 reference.** Production kernels run on wgpu (Metal on the M3, Vulkan on the 2080),
   with float-float where f32 is not enough. Every GPU kernel has a CPU f64 twin used as its oracle.
7. **Scale is a requirement, not an optimisation.** 10⁹–10¹¹ finest cells in a scene, interactively.
   The renderer's level of detail (what the camera needs) and the instrument's (what the phase needs)
   are separate and both first-class.
8. **Fast to build, fast to test, local first.** Seconds, not minutes. Heavy validation runs on demand.
9. **British English; simplicity over cleverness; dependencies only where they earn their place.**

## 3. Architecture

### 3.1 Crates — few, because crate count and generics drive compile time

| Crate | Holds | Depends on |
|---|---|---|
| `cav-core` | units, maths (f64 + float-float), Morton keys, `VoxTree` + sidecars, shapes and `cover`, cube and point kernels, the forward model (flight, arms, propagation integral) | — |
| `cav-sim` | bodies (rigid, skinned, displaced), paths and populations, rigid dynamics and voxel contact, fluids (uniform then adaptive), the scene spec and its library, bundles and datasets | `cav-core` |
| `cav-gpu` | wgpu device, buffers, the WGSL kernels (tree walk, response dot product, fluid, voxeliser), parity harness | `cav-core` |
| `cav-engine` | the engine: owns a scene, runs stages, caches by spec hash, exposes the command/query API, the event log | `cav-sim`, `cav-gpu` |
| `cav-render` | the big-scene renderer (§5) | `cav-core`, `cav-gpu` |
| `cav-app` | the egui application | `cav-engine`, `cav-render` |
| `cav` (bin) | CLI, headless runs, the MCP server | `cav-engine`, `cav-render` (offscreen) |
| `cav-py` | Python bindings (later slice) | `cav-engine` |
| `oracle` | test-only: v1's `gravity` + `instrument` at a pinned `main` SHA as a git dev-dependency, plus George's anchors as fixtures | `cav-core` (dev) |

Keep `cav-core` generic-light (concrete `f64`/`f32` types and a small `Precision` enum over macros
or traits that monomorphise across the workspace). `oracle` is only compiled by `cargo xtask oracle`.

### 3.2 The engine API

```rust
pub enum Command { Load(SceneSpec), Set { path: SpecPath, value: Value }, Run { from: Stage },
                   Seek(f64), Select(Target), View(ViewCommand), Export(ExportKind, PathBuf), Undo, Redo, … }
pub enum Query   { Spec(SpecPath), Schema(Option<SpecPath>), Signal { tower: usize, channel: Channel, span: Span },
                   Field { field: FieldKind, region: Region, lod: Lod }, Stats, Log(Filter), Render(ViewSpec), … }
```

Serialisable (serde), versioned, documented from the schema. The app is a client of the engine on the
same process; the CLI and MCP server are clients over stdio; Python is a client in-process. Runs happen
on a worker; the API never blocks on a fluid solve. The log is the replay: a session can be re-run
from its commands and must produce identical bundles.

### 3.3 The data contract

`StateBundle` as v1 (fields by tensor shape, `.npy` export), plus: per-tower readout times `(T, D)`,
the numerics block (precision, quadrature, tree tolerance, LOD mode, pitches, fluid settings, engine
SHA) and an optional per-tower `error_estimate`. A dataset directory carries a manifest that
regenerates it; `cav verify` proves it. Detail: `milestones/M15-…` and `M16-trust.md` on the v2 branch.

### 3.4 Precision, coordinates, and what runs where

Big worlds and µrad phases pull in opposite directions; the answer is frames, not wider floats.

- **World in f64 on the CPU; every GPU number is local.** Positions handed to the GPU are offsets in
  a frame near where they are used: each tower's frame for the gravity read (origin at its lower launch
  point), each chunk's origin for voxel storage, the camera for rendering (camera-relative rendering, as
  large-world game engines do). Offsets are formed in f64 on the CPU, then rounded. At 1 km, f32 world
  coordinates resolve ~60 µm; tower-relative within 20 m, ~1 µm.
- **Never difference two large numbers on the GPU.** The read is upper-minus-lower and IFO-minus-IFO;
  compute it differentially: 1/|a| − 1/|b| = (b − a)·(b + a) / (|a||b|(|a| + |b|)), with a, b the
  offsets from the two arm points, so the cancelling monopole never appears. Same for the cube series
  and multipoles.
- **Accumulate in float-float** (two-sum, two-product) where a sum of many small terms must hold
  10⁻⁹ relative; pairwise reduction order fixed for determinism. Every such kernel has an f64 twin.
- **Scale units to order one** inside kernels (metres from the tower, kg, rad, seconds from the
  measurement), and keep `G·m_A/ħ` out of the inner loop.
- **The split, to be measured before it is fixed:**

| Work | Where | Why |
|---|---|---|
| scene graph, commands, spec, scheduling | CPU | branchy, small |
| rigid dynamics, contact for tens of bodies | CPU (f64) | sequential, needs exact rest; GPU only for thousands of bodies |
| voxelising one body, building its tree | CPU, parallel (rayon) | once per body; GPU for many or for streaming terrain |
| tree walk / LOD read over (measurement × tower × arm × flight node) | GPU, float-float accumulation | embarrassingly parallel, the main cost |
| response dot products, grid fields | GPU | one pass over cells per frame |
| fluid solve, adaptive regrid, voxel FE modal solves | GPU | bandwidth-bound stencils and multigrid |
| periodograms over 10⁵–10⁷ shots | GPU (NUFFT) with a CPU f64 check | the cadence studies need millions of points |
| rendering, culling, streaming | GPU | — |
| every oracle and parity reference | CPU f64 | the truth |

The builder's research agents own a note on this (`docs/research/precision-and-placement.md`):
measure f32 vs float-float vs f64 error on the anchors in each frame choice, and CPU vs GPU time per
stage on the M3 and the 2080, before the split above is treated as settled.

## 4. The instrument and the physics — what is carried over as knowledge

Read these on the v2 branch before the slices that need them; they are the design, re-derive the code:

- forward model, flight, arms, Simpson split, `fine_dt`, the channels: v1 `DESIGN.md`, `design/instrument.md`
- the voxel tree, lossless merge, cube series, bounded walk, level of detail, adaptive fluids, contact,
  the error budget: `design/voxel.md`, `milestones/M14-voxel-engine.md`
- per-tower timing and the scene spec: `milestones/M15-array-timing-and-scene-spec.md`
- oracles, error ledger, metamorphic tests, manifests: `milestones/M16-trust.md`
- the Beecroft stairwell populations, seismic noise, the baseline tracker: `milestones/M17-site-and-baseline.md`
- the scene catalogue, signal-size guide, water physics: `milestones/M18-scene-library.md`
- the fluid (Boussinesq, MAC, equation-of-state read, film heat via |∇φ|, de Vahl Davis numbers),
  parcels and skinning, rigid dynamics with exact rest: `design/physics.md`
- the oscillatory-noise validation suite, analysis features and the physics research programme:
  `plan/VALIDATION-AND-RESEARCH.md` (from George's paper, upgrade report and reference scripts)
- the viewer's interaction model (one edit path, stale-not-blocked, per-tower lanes, console):
  `milestones/M13-viewer-v2.md` §9, and the browser mock-up (§6)

## 5. Big scenes — rendering like a voxel game engine

The instrument's level of detail answers "what does the phase need"; the renderer's answers "what
does this pixel need". Both read the same tree.

- **Storage.** World split into chunks (e.g. 32³ finest cells per brick, bricks in a sparse top-level
  hash keyed by chunk coordinate). Bodies keep their own trees in their own frames; terrain, structure
  and fluids are world-space chunk sets. Out-of-core: chunks memory-mapped from a scene cache on disk,
  streamed by priority.
- **Near field: rasterise.** Uniform leaves are meshed into exposed faces only (greedy meshing per
  chunk, rebuilt on change), drawn with GPU-driven indirect draws after compute-shader frustum and
  hierarchical-Z occlusion culling.
- **Far field: ray-march.** Beyond a distance, chunks are drawn by marching their tree (a 4³
  "64-tree" re-pack suits the GPU), so cost follows pixels, not voxels.
- **Clipmap LOD around the camera** with screen-space error as the criterion; coarse nodes carry mean
  colour / field value and their mass moments, so a far view of a field is still faithful.
- **Budgets, not hopes:** frame time, GPU memory and streaming bandwidth have stated budgets; the
  profiler shows them; the renderer degrades level of detail before it misses a frame.
- **Targets (measure, then commit):** 60 fps at 1440p on the M3 for a 10¹⁰-cell scene (1 km² terrain
  at 5 cm near the array, a town's buildings, a road with traffic); scene load under 5 s from cache.
- **Offscreen** rendering through the same renderer for the CLI and MCP (`render` returns a PNG).

## 6. The viewer

The browser mock-up (`claude.ai/artifact/WLmoWvg6dPkBL3fZUDR2fw`) is the reference for look and
interaction: egui dark, Ubuntu Light / Mono, Principia's rules (the figure is never covered; badges in
a header strip; Home is the one control on the figure; rarely changed settings in windows; Tab walks
scopes with a focus ring and breadcrumb; F3 hides the GUI; the footer opens into the console).

**Must have, beyond the mock-up's current features:**

- **Colour by any field** — tree level, LOD cut, distance, height, mass, the tower response, the phase
  contribution of each cell, temperature, density anomaly, velocity, pressure, contact state, error
  estimate — with colour maps by field kind (sequential, diverging, categorical), robust or locked
  ranges, log scales, and **legends generated from what is drawn**.
- **Projections and views** — perspective and orthographic; named square-on views (top, bottom,
  north, south, east, west) and iso; saved views; follow and orbit-about a body; a view cube.
- **Multiple viewports** — 1, 2, 4 or a custom grid, each with its own camera, projection, style and
  field; linked cursors and selection across them; a viewport can show a 2-D plot instead.
- **Draw styles** — shaded, flat, x-ray (additive, depth off), wireframe/bounds, cut-away and section
  planes, and **paper**: an orthographic technical drawing on cream paper with ink outlines, hatching
  for sections, a scale bar, a north arrow, dimension lines and a title block, exportable as SVG/PDF.
- **Fields in space** — slices, isosurfaces, ray-marched volumes, streamlines and particles for flows,
  glyphs, the response map of a tower, the contribution map ("what this tower hears from where").
- **Time** — per-tower firing lanes, flight diagrams on absolute time, onion-skin / ghosting of moving
  bodies, trails coloured by any field, a time–space cube (x, y, t), event markers (contact, sleep,
  regrid), bookmarks.
- **Comparison** — two runs side by side or as a difference (e.g. pitch 2 cm vs 1 cm; LOD on vs exact);
  small multiples per tower; spectrograms of each channel.
- **Debug views** — tree bounds by level, the walk's accepted nodes per tower, LOD bands, the fluid's
  adaptive grid, regrid events, contact manifolds, GPU vs CPU difference maps, the error ledger.
- **Capture** — screenshots, frame sequences, video, and every view reproducible from a command.

## 7. Interfaces

- **GUI** (`cav-app`): everything above.
- **Headless / CLI** (`cav`): `cav run scene.ron --out dir/`, `cav batch scene.ron --n 10000 --seed 7`,
  `cav verify dir/`, `cav render scene.ron --view top --style paper --out plan.svg`,
  `cav get|set|ls <path>`, `cav bench`, `cav doctor` (device, adapter, precision self-test).
- **MCP server** (`cav mcp`, stdio, using the official Rust MCP SDK): tools that map one-to-one onto the
  engine API — `scene_load`, `scene_get`, `scene_set`, `scene_schema`, `run`, `signal`, `field_stats`,
  `render_view` (returns an image), `log`, `dataset_build`, `verify`; resources for the scene spec, the
  library index and the last bundle. An agent can build a scene, run it, look at it and judge it.
- **Python** (`cav-py`, later): the same API, NumPy arrays out, a torch `Dataset`.

## 8. Build, test and speed — local first

No CI at the start. Everything runs on the owner's machines; a slice is done when the local gates pass.

- **Toolchain:** pinned stable for releases; an optional nightly dev profile with the Cranelift
  backend and the parallel front end for faster debug builds (measure; keep only if it helps).
- **Profiles** (Cargo's own guidance): `debug = "line-tables-only"` in dev, `debug = false` for
  dependencies, `opt-level = 3` for the hot numerical crates even in dev, a `debugging` profile for
  full debug info on demand.
- **Linker:** the platform's fast linker — mold or wild on Linux, the default ld-prime on macOS.
- **Tests in tiers:**
  - `cargo xtask quick` — check, clippy on changed crates, unit tests via cargo-nextest, small sizes,
    CPU only: **under 30 s warm**. A pre-push hook runs it.
  - `cargo xtask gpu` — parity tests on the local adapter (Metal on the Mac): under 2 min.
  - `cargo xtask oracle` — the v1 anchors and fluid benchmarks: on demand and before a slice is
    declared done.
  - `cargo xtask bench` — criterion benchmarks with stored baselines; a regression over 10% is
    reported in the slice notes.
- **Guardrails without CI:** a recorded floor on the number of tests each tier executes (fewer = fail);
  golden scenes whose bundle hashes are checked by `quick`; `cargo xtask demo <slice>` runs the slice's
  demo headlessly and saves screenshots.
- **Later, if wanted:** one fast hosted job running `quick` only. Nothing heavier.

## 9. The slices

Each slice: a branch, a demo (what the owner can do in the app that they could not before), its tests,
its oracle checks, a performance note, screenshots in `docs/slices/`. Main always builds and runs.

| # | Slice | The new thing you can do | Oracle / exit |
|---|---|---|---|
| S0 | **The shell** | Open the app: the room, five towers, the panels, the console, the timeline, the view controls (projections, named views, panes, styles). `cav doctor`, `cav render` of an empty scene; `cav mcp` answering `scene_schema` | builds in under a minute cold on the M3; `quick` under 10 s |
| S1 | **A point mass** | Drag a mass around; see each tower's phase update live; play a straight pass; plot the signal | phase vs v1 rigid source ≤1e-10 on the anchors (`oracle`) |
| S2 | **Scenes as files, the API everywhere** | Load and save `.ron` scenes; edit any path from a panel, the console, the CLI or an agent over MCP; undo; per-tower timing lanes; headless `run` writes a bundle | a session replays from its log bit-identically; `.npy` round-trips |
| S3 | **Voxel bodies** | Shapes library (primitives, Platonic solids, meshes); bodies drawn as trees; colour by any field with generated legends; debug overlays | tree expanded = raster voxeliser bit-exact; merged = unmerged ≤1e-12; mass and inertia vs analytic |
| S4 | **Big scenes** | A 10¹⁰-cell scene (terrain, buildings, a road) at 60 fps with streaming and view LOD; paper style export to SVG | frame-time budget met on the M3, measured |
| S5 | **Gravity on the GPU, at scale** | Real-time phase for big scenes; the bounded walk and instrument LOD with the error badge; the error ledger in bundles | GPU = CPU f64 ≤1e-9 with float-float; walk error ≤ tol; no steps in time |
| S6 | **Things that move** | Paths, populations (traffic, birds, drones, aircraft), articulated walkers on per-bone trees, the outdoor library scenes | mass conserved bit-exact through poses; signal-size guide holds within ×10 |
| S7 | **Things that fall and stack** | Rigid dynamics with voxel contact; drop, bounce, stack; recorded tracks | free fall ≤1e-10 m; slept pose bit-constant; box on box rests |
| S8 | **Air** | Uniform GPU fluid with the radiator scene; slices, volumes, streamlines; the response and contribution maps | de Vahl Davis Nu within 1%; moving-ball oracle through the grid path |
| S9 | **Adaptive air** | The air on a balanced octree refined by what the towers see; the adaptive grid visible | fully refined = uniform; at-rest on a graded grid; no step on regrid |
| S10 | **Water** | Liquids with a free surface: tanks, a river's level changing, boats, a waterfall | floating box at Archimedes draught; steady river silent |
| S11 | **Datasets and Gradar** | Dataset builder in the GUI and CLI, manifests, `verify`, Python and torch; the non-learned baseline beside a trained model | manifests regenerate; CRB ≤ baseline error |
| S12 | **The site** | The Beecroft stairwell scene with its populations, seismic noise, the published ordering reproduced | published ordering; masked fraction within ×2 |

Order is a default. Slices that do not depend on each other (S6 and S8, say) can run in parallel.

## 10. How the work runs — builders and reviewers

- **Builder agent** per slice: writes the slice brief (requirements traced to tests, tolerances,
  demo), then the code, then the demo and screenshots.
- **Reviewer agent**, fresh context, after each slice: reads the diff against the brief and this plan;
  checks the physics and the oracle tolerances, looks for tests that cannot fail, runs the demo, and
  reviews the screenshots for UX against §6. Findings are fixed before the slice is merged.
- **Parallel work in cloud sessions** (`claude --cloud "…"` from the CLI, one task per session):
  independent pieces such as the shapes library, MCP tool coverage, the scene catalogue, docs and the
  paper-style exporter. GPU and Metal work stays on the owner's machine.
- **Cloud sessions run without prompts.** They run in isolated VMs, so give them full permission:
  commit `.claude/settings.json` with the permission default mode set to auto (or bypass permissions)
  and broad allow rules for cargo, git and the toolchain, and pick that mode when starting a session.
  Check on the first run that no prompts appear; adjust the file if they do.
- **Owner checkpoints:** at the end of each slice, a short note: what is new (with screenshots), what
  was decided and why, what to look at. Anything that cannot be undone waits for the owner.

## 11. Open questions

- Whether to keep `oracle` as a git dev-dependency on v1 crates or to freeze v1's anchor outputs as
  fixtures and drop the dependency (faster builds; loses the ability to run new scenes through v1).
- The array geometry and firing rules for an outdoor or multi-tower deployment — George.
- Licence and publishing of the engine (owner).
