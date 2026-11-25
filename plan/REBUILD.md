# Cavendish — the rebuild, from a blank slate

> **Status: plan.** This branch starts from a blank slate. The v1 code is tagged `v1-final` (`56df4ab`)
> and the v2 prototype `v2-prototype` (branch `v2-world-physics`); their design documents are copied into
> `reference/` as the record of what was learned. Nothing from them is ported as code unless a slice says
> so. The validated v1 forward model is kept as an **oracle** at the `v1-final` tag, consulted by tests,
> never extended.
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
   sidecar arrays) for bodies, structure, terrain, air and water. No second path beside it. Arranged as
   Teardown arranges it: a volume per object in its own frame, plus one world volume (§5.2).
3. **One error budget, in radians.** Every approximation — tree walk, level of detail, quadrature,
   precision, adaptive grids — is bounded or audited against a phase tolerance, and the bundle records
   what it was.
4. **Oracles before features.** The v1 forward model and George's anchors are the reference; every new
   path is tied to one by a test with a stated tolerance. A test that cannot fail is not a test.
5. **One engine API.** The GUI, the CLI, the MCP server and Python all drive the same command/query
   interface. Every edit is a command; every command is logged, undoable and replayable.
6. **GPU first, f64 reference.** Production kernels run on wgpu: Vulkan (or DX12 on Windows) on the build PC's NVIDIA GPU first,
   Metal on a Mac as a second platform when one is available,
   with float-float where f32 is not enough. Every GPU kernel has a CPU f64 twin used as its oracle.
7. **Scale is a requirement, not an optimisation.** 10⁹–10¹¹ finest cells in a scene, interactively.
   The renderer's level of detail (what the camera needs) and the instrument's (what the phase needs)
   are separate and both first-class.
8. **Fast to build, fast to test, local first.** Seconds, not minutes. Heavy validation runs on demand.
9. **British English; simplicity over cleverness; dependencies only where they earn their place.**
10. **Built for other people.** Someone new can read the code, find where a thing lives, and add a
    body, a motion, a source, a field, a command or a validation case by following a written recipe
    and one existing example. Every public item is documented; every crate says what it is for and
    what it must not depend on; `docs/ARCHITECTURE.md` is kept true. A slice that adds a kind of
    thing adds or updates its recipe.
11. **Everything is data** (owner, 25 Nov 2026). What a scene contains, how the simulation is
    configured, and how the GUI is laid out — panels, layouts, toolbars, tools, shortcuts — are
    described by data with a schema, loaded and saved like scenes, and driven through the one
    engine API. **Detail tiers** (preview, standard, reference, and user-defined) are data too: a
    tier names the quadrature, tree tolerance, LOD, pitches and solver settings, and every bundle
    records the tier it was made at. Code holds the mechanisms; data holds the choices.

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
| `oracle` | test-only: v1's `gravity` + `instrument` at tag `v1-final` (`56df4ab`) as a git dev-dependency, plus George's anchors as fixtures | `cav-core` (dev) |

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
regenerates it; `cav verify` proves it. Detail: `reference/milestones/M15-…` and `M16-trust.md`.

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
stage on the NVIDIA build PC (and a Mac when available), before the split above is treated as settled.

## 4. The instrument and the physics — what is carried over as knowledge

Read these in `reference/` before the slices that need them; they are the design, re-derive the code
(v1 docs not copied here, such as `reference/design/instrument.md`'s siblings, are at tag `v1-final`):

- forward model, flight, arms, Simpson split, `fine_dt`, the channels: `reference/DESIGN.md`, `reference/design/instrument.md`
- the voxel tree, lossless merge, cube series, bounded walk, level of detail, adaptive fluids, contact,
  the error budget: `reference/design/voxel.md`, `reference/milestones/M14-voxel-engine.md`
- per-tower timing and the scene spec: `reference/milestones/M15-array-timing-and-scene-spec.md`
- oracles, error ledger, metamorphic tests, manifests: `reference/milestones/M16-trust.md`
- the Beecroft stairwell populations, seismic noise, the baseline tracker: `reference/milestones/M17-site-and-baseline.md`
- the scene catalogue, signal-size guide, water physics: `reference/milestones/M18-scene-library.md`
- the fluid (Boussinesq, MAC, equation-of-state read, film heat via |∇φ|, de Vahl Davis numbers),
  parcels and skinning, rigid dynamics with exact rest: `reference/design/physics.md`
- the oscillatory-noise validation suite, analysis features and the physics research programme:
  `plan/VALIDATION-AND-RESEARCH.md` (from George's paper, upgrade report and reference scripts)
- the ML side and the Python API (simulate while you train, typed tables with roles, splits that
  never leak, manifests, the tasks and their baselines): `plan/ML-AND-API.md`
- the viewer's interaction model (one edit path, stale-not-blocked, per-tower lanes, console):
  `reference/milestones/M13-viewer-v2.md` §9, and the browser mock-up (§6)

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
- **Targets (measure, then commit):** 60 fps at 1440p on the NVIDIA build PC for a 10¹⁰-cell scene (1 km² terrain
  at 5 cm near the array, a town's buildings, a road with traffic); scene load under 5 s from cache.
- **Offscreen** rendering through the same renderer for the CLI and MCP (`render` returns a PNG).

### 5.1 Live or baked — how the app talks to the simulation, and how it stays responsive

Neither alone. The engine runs on its own threads and the GPU; the app only ever **reads finished
results** and **sends commands**; nothing the user does waits on a solver.

- **Live** (computed as you watch, at playback rate): kinematic and rigid bodies, paths, populations,
  dynamics for up to thousands of bodies, and the gravity read itself. The read is cheap enough on the
  GPU — per shot about 3,000 evaluation points (147 flight nodes × 2 arms × 2 interferometers × 5
  towers) against the scene's elements: ~10⁸ kernel evaluations for 40,000 leaves, a few ms; beyond
  that the instrument LOD (§5, `reference/design/voxel.md`) keeps it in budget.
- **Baked** (simulated ahead, cached, scrubbed): air and liquids at scale, big batches, anything over
  budget. The fluid runs as a background simulation that **streams frames**; the timeline shows a
  "simulated up to" front, the playhead can move freely behind it, and the read of a baked field is
  the fixed response dot product (instant once the frame exists).
- **Cached by spec hash** at every stage (shape, dynamics, air, forward, readout): an edit re-runs
  only the stages it invalidates (M15's `diff`), and a scene opened twice is instant the second time.

What makes it *feel* responsive:

1. **Superposition is incremental.** The phase is a sum over sources, so each source's contribution
   is cached separately; dragging one body recomputes that body only, and the rest is a stored array.
2. **Preview, then refine.** While something is being dragged or scrubbed, the engine answers at a
   coarse LOD and fewer flight nodes within the frame budget; when the input stops it refines to the
   requested tolerance in the background and swaps the result in. A badge says which you are seeing
   (`preview · ±3e-4 rad` → `refined · ±1e-10 rad`), so a preview is never mistaken for an answer.
3. **Frame-budget scheduler.** Work is split into chunks with a time budget per frame (render first,
   then the read, then refinement, then baking); nothing long runs on the UI thread.
4. **Speculation ahead of the playhead.** During playback the engine computes the next few seconds
   before they are needed; scrubbing backwards hits the cache.
5. **Latest result wins.** Every command carries a generation number; stale work is cancelled, not
   finished; results are double-buffered so the screen never shows a half-updated state.
6. **Stale, not blocked** (from M13): an edit marks dependent views stale, keeps the old result on
   screen with a badge, and the new one replaces it when ready.
7. **Budgets are visible.** The profiler shows the frame budget, the read's share, the preview/refine
   state and the bake front; the footer shows when the engine is behind.

Exit tests for this (S2 onwards): input-to-photon latency under 50 ms while dragging a body in a
40,000-leaf scene on the build PC; the refined result equals a cold run bit for bit; a cancelled run leaves
no partial result visible.

### 5.2 The world model — Teardown's, made exact for gravity

Like Teardown: **every object owns its own voxel volume in its own frame**, moving and rotating
freely; the world adds **one global, coarser volume** over the whole scene for what needs a shared
grid. Teardown uses the global volume for shadows and lighting; Cavendish uses it for air and water,
contact broad-phase, lighting, and as the stamp target of moving bodies. Static matter (terrain,
buildings, slabs) lives directly in the world volume, in chunks.

- **Per-object volumes** are sparse trees (§3.1), one per rigid body and one per bone, with a material
  palette per voxel (density, colour, elastic properties). The gravity read uses them directly, so a
  moving body is never snapped to a world lattice. Snapping a 70 kg body 2 m from a tower to 1 cm cells
  would put ~20 mrad steps in a ~3 rad signal, 20–80× the shot noise.
- **Destruction and change** as Teardown: removing voxels edits the object's tree; when it falls apart,
  connected components are split into new objects (each a new body with exact mass and inertia from
  its voxels); repainting changes materials. Mass is conserved through every split, to the bit.
- **Rendering** as Teardown at heart: each object's volume is ray-marched inside its oriented box
  using the tree's coarse levels to skip empty space; the world volume uses the chunked renderer of
  §5; one global coarse occupancy volume drives shadows and ambient occlusion.
- **Coupling**: each tick, moving objects are stamped into the world volume conservatively (mass and
  volume fraction exact) for the air, water and contact; the stamp is never used for the gravity read.

### 5.3 Ideas taken from other voxel engines

Owner's decision (21 Nov 2026): the world model is §5.2's — per-object volumes plus one world
volume, as Teardown. What else is worth taking, and what it becomes here:

| From | Idea | In Cavendish |
|---|---|---|
| **Teardown** | object volumes with a material palette; ray-marched in oriented boxes with coarse levels to skip space; a global coarse volume for shadows; destruction by splitting connected components into new bodies | §5.2, as written |
| **Gustafsson's new engine** | sub-stepping instead of solver iterations ("temporal Gauss–Seidel"); a parallel solver for big piles; better contact generation and broad phase | the rigid solver of S7: small substeps with one Gauss–Seidel pass each, parallel islands, exact rest kept as the test |
| **Minecraft** | fixed-size chunks with a block palette per chunk; region files; lighting propagated through the grid; chunk load radius around the player | world-volume chunks with per-chunk material palettes; a region-file scene cache on disk; streaming radius around the camera **and** around each tower (the instrument always has its neighbourhood resident) |
| **Noita** | every cell simulated (falling sand); rigid bodies made from cells, broken back into cells | granular matter (sand, gravel, rubble) as a cellular or MPM layer on the world volume; debris from destruction can settle into it |
| **Space Engineers / 7 Days to Die** | structural integrity: load paths through blocks; unsupported structure collapses | a cheap structural pass on the world volume (support graph) before the voxel-FE modal solve of S7 — decides what falls when something is removed |
| **Atomontage / sparse voxel DAGs / brickmaps** | billions of voxels by deduplicating identical subtrees (DAG) or bricking; streaming LOD | static world chunks stored as a DAG where it compresses (terrain, repeated buildings); bricks of 8³ for GPU traversal; the 64-tree re-pack for ray marching |
| **NanoVDB / OpenVDB / SPGrid** | topology separate from values; sparse levels for simulation; GPU-friendly flat layouts | the tree and sidecars of §3.1; the fluid's sparse pyramid (S8–S9) |
| **Enshrouded / dual contouring** | smooth surfaces extracted from voxels for rendering while simulation stays voxel | an optional smooth-surface draw style for terrain and liquids (marching cubes / dual contouring on the GPU), purely visual |
| **John Lin's and other voxel GI demos** | per-voxel global illumination by tracing the voxel structure | lighting in the viewer by tracing the global coarse volume (soft shadows, ambient occlusion, bounce), with a budget toggle |

What is **not** taken: snapping moving objects to the world lattice for anything the instrument reads
(§5.2), and game-style approximations (sleeping thresholds that move bodies, position projection that
violates momentum) anywhere the error budget would see them. Visual-only shortcuts are fine; the
viewer labels them.

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
- **Figures for papers and talks** — any plot or view exported as SVG, PDF or PNG at a stated size
  and resolution, with a house style (fonts, line weights, colour maps safe for colour-blind readers
  and for print) and named presets (paper column, paper full width, slide, poster); the data behind
  every figure exported beside it (`.csv` or `.npy` plus a small JSON of axes, units and provenance:
  scene, engine SHA, numerics) so it can be redrawn elsewhere; the same from the CLI
  (`cav figure …`), MCP and Python (a matplotlib style sheet and helpers that redraw a figure from
  its exported data). A figure regenerates bit for bit from its command.

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
- **Linker:** the platform's fast linker — `rust-lld` on Windows (MSVC target), mold or wild on Linux,
  the default on macOS.
- **Tests in tiers:**
  - `cargo xtask quick` — check, clippy on changed crates, unit tests via cargo-nextest, small sizes,
    CPU only: **under 30 s warm**. A pre-push hook runs it.
  - `cargo xtask gpu` — parity tests on the local adapter (the NVIDIA GPU): under 2 min.
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
| S0 | **The shell** | Open the app: the room, five towers, the panels, the console, the timeline, the view controls (projections, named views, panes, styles). `cav doctor`, `cav render` of an empty scene; `cav mcp` answering `scene_schema` | builds in under a minute cold on the build PC; `quick` under 10 s |
| S1 | **A point mass** | Drag a mass around; see each tower's phase update live; play a straight pass; plot the signal | phase vs v1 rigid source ≤1e-10 on the anchors (`oracle`) |
| S2 | **Scenes as files, the API everywhere** | Load and save `.ron` scenes; edit any path from a panel, the console, the CLI or an agent over MCP; undo; per-tower timing lanes; headless `run` writes a bundle | a session replays from its log bit-identically; `.npy` round-trips |
| S3 | **Voxel bodies** | Shapes library (primitives, Platonic solids, meshes); bodies drawn as trees; colour by any field with generated legends; debug overlays; towers of different heights and sequences in one array, and a scene with no towers at all | tree expanded = raster voxeliser bit-exact; merged = unmerged ≤1e-12; mass and inertia vs analytic; an array of unequal towers = each tower simulated alone, bit for bit |
| S4 | **Big scenes** | A 10¹⁰-cell scene (terrain, buildings, a road) at 60 fps with streaming and view LOD; paper style export to SVG | frame-time budget met on the build PC, measured |
| S5 | **Gravity on the GPU, at scale** | Real-time phase for big scenes; the world as one LOD octree (chunks merged upward into coarser nodes carrying mass moments) and the gravity read walking it with LODs: full precision in a bubble around each tower, coarser rings outward, every level's switch-over radius measured, the whole world read (no truncation); world shapes: a finite patch, an infinite flat plane, a sphere; the bounded walk and instrument LOD with the error badge; the error ledger in bundles | GPU = CPU f64 ≤1e-9 with float-float; walk error ≤ tol; no steps in time |
| S6 | **Things that move** | Paths, populations (traffic, birds, drones, aircraft), articulated walkers on per-bone trees, the outdoor library scenes | mass conserved bit-exact through poses; signal-size guide holds within ×10 |
| S7 | **Things that fall and stack** | Rigid dynamics with voxel contact; drop, bounce, stack; recorded tracks | free fall ≤1e-10 m; slept pose bit-constant; box on box rests |
| S8 | **Fluids: air** | One fluid solver, first on a uniform GPU grid, with air and the radiator scene; slices, volumes, streamlines; the response and contribution maps | de Vahl Davis Nu within 1%; moving-ball oracle through the grid path |
| S9 | **Fluids: the adaptive grid** | The same solver on a balanced octree refined by what the towers see, for any fluid; the adaptive grid visible | fully refined = uniform; at-rest on a graded grid; no step on regrid |
| S10 | **Fluids: water** | Liquids with a free surface on that adaptive grid, refined at the surface and by what the towers see: tanks, a river's level changing, boats, a waterfall | floating box at Archimedes draught; steady river silent; fully refined = uniform and no step on regrid, for water as for air |
| S11 | **Datasets and Gradar** | Dataset builder in the GUI and CLI, manifests, `verify`, Python and torch; the non-learned baseline beside a trained model | manifests regenerate; CRB ≤ baseline error |
| S12 | **The site** | The Beecroft stairwell scene with its populations, seismic noise, the published ordering reproduced | published ordering; masked fraction within ×2 |
| S13 | **Heat everywhere** | Anything can be hot: people, machines, radiators, sunlit walls. Heat conducts through solids, radiates between surfaces, is carried by air and by water, and warms structure so that it expands; every one of those shows in the towers' readings, with temperature and heat flux drawn in the viewer | energy conserved in a closed box; the analytic and published cases of `VALIDATION-AND-RESEARCH.md` §2.5; a uniformly heated sphere gives no signal |
| S14 | **The wider world** | Put the site at a real latitude and longitude on a rotating Earth with the Moon and the Sun: tides, weather and clouds, ground water, ocean microseisms, earthquakes from anywhere, a pressure wave from the far side of the planet. Replay a real day from open data | tides against standard tidal prediction; the same local scene reads the same phase at the origin and on the globe; each far source against its closed form or published record |
| S15 | **Instruments anywhere** | A gradiometer on any axis, horizontal included; local g and its gradient from the Earth's gravity field at the site; the same instrument on the Moon or Mars, and in orbit; a full budget of instrument systematics | vertical case unchanged to the bit; a horizontal baseline against the closed-form gradient tensor; an orbiting gradiometer against the gravity model's tensor and published GOCE-class gradients |
| S16 | **The science case** | Inject a gravitational wave or a dark-matter signal and try to find it through a full simulated day; two sites on one planet, correlated; choose a site anywhere on Earth and get its noise budget; run beside the real instrument as its twin | injected signals recovered at the published sensitivity in a quiet world; two-site correlation rejects local noise by the predicted factor |
| S17 | **In the browser** | The whole app as a web page on GitHub Pages, built to WebAssembly with WebGPU by CI on each push to main; single-threaded on the web, the engine's workers run in frame-sized slices | the web build runs the S0–S3 demos' scenes and reads the same phases as the native build, bit for bit where the arithmetic is the same |
| S18 | **The editor** | A toolbar of tools (Select, Move, Place), an interactive viewport (gizmo, transform session, fly, framing), a schema-driven inspector, resizable panels, maximise and full screen, the keyboard model of `docs/design/keyboard.md`, the GUI's actions, tools and layouts as data (docking, pop-out, materials, mesh import and detail tiers moved to S19 by the S18 brief) | scripted tasks run end to end through the UI tests ("add a sphere, move it 2 m, read tower 1's phase"); the keyboard state machine tested with control arms; every edit by viewport or panel is one undoable command |
| S19 | **The editor's second half** | Materials (a library as data, bodies of several materials, a brush), mesh import from the editor, Probe, Measure, Slice, Paint and Scale tools, units typed with values, detail tiers as data, camera bookmarks, the browser's reserved keys, docking with tabs and floating panels (pop-out windows, follow, pointer lock, multi-selection and Rotate moved to S20 by the S19 brief) | as S18's exits for each new tool; a tier's phase within its stated bound of the reference tier; mass conserved to the bit through every material edit; a two-material body reads as the sum of its parts |
| S20 | **The editor's workspace** | Pop-out OS windows, follow, pointer lock and the trackball, multi-selection and grouped edits, Rotate (bodies with an orientation), a user keymap file, phones and tablets in the web build (touch, drawers, a bottom tool bar) | as S18's exits; a turned body reads as the same body unturned with the arm points turned, bit for bit |

| S21 | **The look** | Per-vertex ambient occlusion in the near meshes and the same AO at the far march's hit; sun and sky light in linear light, a neutral tone curve, distance fog; ray-traced sun shadows through a coarse occupancy volume; an analytic sky now and a scattering atmosphere with the planet; per-voxel colour variation (cell-hash jitter, height and slope blends); a beauty look and a data look as data, the data look leaving colour maps exact | AO exact on known configurations; merging keeps AO; near and far agree; golden images with each layer's off-arm; data-look pixels equal the colour map; every layer within its per-frame cap on this card and the web |
Order is a default. Slices that do not depend on each other (S6 and S8, say) can run in parallel.
S18 was added by the owner on 25 Nov 2026 and runs straight after S4 (research: `docs/research/editor-ux.md`).
S19 was split from S18 by its brief (`docs/slices/S18.md` §3): S18 builds the note's "A1" and "A2",
S19 its "B", the owner's materials and mesh import, and the detail tiers. S20 was split from S19 by
its brief (`docs/slices/S19.md` §3).
S17 was added by the owner on 24 Nov 2026 and runs next, after S3; its brief is
`docs/slices/S17.md`, its research `docs/research/web-build-and-pages.md`.
S14–S16 were added by the owner on 22 Nov 2026. What they hold is in §9.3; each starts with a
research note that sizes every effect in radians, and what a note shows to be invisible is built as
a null test and no further.
S13 was added by the owner on 22 Nov 2026 and is numbered by when it was added, not by when it runs:
it follows S10 (it needs air, adaptive air and water) and comes before S11 and S12, whose datasets
and site scenes should include heat. What it holds is in §9.2.

### 9.1 Dev and debug scenes (from S0, grown with every slice)

Scenes whose answer is known, for building, testing and looking at one thing at a time. Each is a
spec file in `scenes/dev/`, loads in one keystroke from the library, and most double as tests.

| Scene | What it is | What it checks |
|---|---|---|
| `void` | the array and nothing else: no room, no air, no ground | every channel exactly zero; the renderer with an empty world |
| `infinite-floor` | an analytic half-space or slab below the array, drawn as a grid to the horizon | a uniform infinite slab gives uniform g and no gradient: its contribution to the differenced phase is zero. A **finite** floor growing in size must converge to that, with edge terms falling as the size grows |
| `point` · `unit-cube` · `sphere` | one mass at a known place | the v1 anchors; cube kernel vs point far away |
| `probe-grid` | a lattice of 1 kg probes, one at a time | the response map of each tower, cell by cell, against the closed form |
| `platonic-row` | the five Platonic solids at equal range | their missing low multipoles (`M18`) |
| `oscillator-zoo` | linear, circular, pendulum, sphere oscillators at the paper's parameters | the comb, folding and tilt-ratio tests of `VALIDATION-AND-RESEARCH.md` §2 |
| `lod-ladder` | the same body at 1, 3, 10, 30, 100 m | instrument LOD error vs distance; renderer LOD transitions without popping |
| `precision-far` | a body 10 km from the origin, towers beside it | the local-frame rules of §3.4: same phase as at the origin |
| `stress-1e10` | procedural terrain and buildings, 10¹⁰ finest cells | the big-scene budgets of §5 |
| `fuzz` | random scenes from a seed | the metamorphic properties of M16 |
| `superposition` | A, then B, then A + B | the sum is exact, channel by channel |
| `mirror-pair` | two equal masses mirrored about a vertical plane through the array | towers on the plane read the sum, mirrored towers swap readings |
| `spinning-sphere` | a uniform sphere rotating in place | zero time-varying signal (its field never changes) |
| `shell` | a thin spherical shell around a tower, then a mass inside it | shell theorem: no gradient inside from the shell |
| `ring-orbit` | a mass circling a tower at fixed height and radius | constant reading; any ripple is quadrature or interpolation error |
| `flash` | a mass that exists for one millisecond at a chosen time inside a flight | the impulse response of the flight window; sweep the time to draw it |
| `switch-on` | a mass that appears and stays | the step response; ramp over `[t − 2T, t]` |
| `teleport` | a body that jumps between frames | no interpolation artefacts across a discontinuity |
| `time-reversal` | a pass played forwards and backwards | mirrored signal (the sequence is symmetric in time) |
| `through-the-tower` | a body passing through a tower's atom path | finite everywhere (cube kernel), no NaN, the warning logged |
| `cadence-lab` | no physics: a known tone sampled with jitter, alternation, gaps, loss | the analysis tests of `VALIDATION-AND-RESEARCH.md` §2.4 |
| `contact-gallery` | every shape pair resting on every other; a Newton's cradle; a stack | exact rest; momentum through the cradle |
| `cantilever` · `plate-modes` | a beam and a plate ringing | Euler–Bernoulli and Kirchhoff frequencies and mode shapes (voxel FE) |
| `fluid-benchmarks` | heated cavity, lid-driven cavity, Rayleigh–Bénard onset, dam break | published numbers, uniform and adaptive |
| `regrid-stress` | a plume crossing level boundaries on purpose | mass conserved, no step in any signal |
| `million-bodies` | 10⁶ small rigid bodies falling into a box | dynamics and read throughput; budget behaviour |
| `flythrough` | a scripted camera over `stress-1e10` | streaming with no stalls; frame-time trace |
| `gpu-vs-cpu` | any scene, both paths | a difference overlay in the viewer and a number in the footer |

Outdoor and world scenes — aircraft over a field, traffic on a road, birds, drones, boats, rivers,
the town — are the scene library (`reference/milestones/M18-scene-library.md`) and arrive with S6, S9 and S10;
many are near-null by design (an airliner at 1 km is well under shot noise), which is a test too.

### 9.1a Fluids (S8–S10) — air and water are one thing

Owner's decision (22 Nov 2026): air and water are both **fluids**, one solver and one grid, and water
is adaptive as the air is. A fluid is a material with a density, a viscosity and an equation of
state; air and water differ in those numbers and in whether there is a free surface, not in code
path. So:

- **One solver** (S8): the MAC projection solver on the world volume, written for a fluid, not for
  air. Air is its first user because its benchmarks are the simplest.
- **One adaptive grid** (S9): the balanced octree, its level transfers, the well-balanced buoyancy and
  the goal-oriented refinement by tower response belong to the solver, so every fluid gets them.
- **Water** (S10) adds what a liquid needs and nothing else: the free surface (a level set on the same
  octree, refined at the surface), the full-density read, two-way coupling with bodies. Air above
  the water is the same solver's other phase where it matters, and still air otherwise.
- The viewer, the spec and the commands say `fluid` with a material, not `air` and `water` as
  separate kinds; the debug views, the regrid events and the response maps are shared.
- Every property proved for air on the adaptive grid is proved again for water: fully refined equals
  uniform, at rest on a graded grid, no step in any signal on regrid, mass conserved to the bit.

### 9.2 Heat (S13) — one temperature field through solids, air and water

S8–S10 give air and water that move and a wall that can be held warm. S13 makes heat a property of
the whole world, because the instrument reads every way heat moves mass: warm air and warm water are
lighter, and warm structure expands. `reference/design/physics.md` records a 70 °C radiator a metre
from a tower at 2.2e-2 rad, twenty times the shot noise, so none of this is decoration.

- **One temperature field, one energy budget.** Temperature lives on the same voxels as matter. Each
  material in the palette carries conductivity, heat capacity, emissivity and expansion coefficient.
  Energy is conserved to round-off across solid, air and water in every step; the bundle records the
  balance (in, out, stored) as it records the phase error.
- **Sources.** Any body or surface can be a source: fixed temperature, fixed power (a person at about
  100 W, a machine, a radiator), a schedule in time, or sunlight on outside surfaces.
- **Conduction in solids** on the voxel tree (the fluid's multigrid, reused), with contact between
  touching bodies.
- **Convection in air and in water**, coupled to solids at their surfaces (conjugate heat transfer):
  the wall temperature is solved, not set. Water uses its real equation of state, with the density
  maximum near 4 °C; air keeps Boussinesq where the temperature range allows and the bundle says
  when it does not.
- **Radiation between surfaces** (grey, diffuse) by tracing the world's coarse volume, as the lighting
  does; view factors are checked against closed forms. Participating air is out of scope.
- **Thermal expansion.** Warm solids expand: displacement from the voxel finite elements of S7, played
  back through the displaced-body read, mass conserved to the bit.
- **The read.** Three contributions, separable in the viewer and in bundles: the density change of
  air, of water, and the displacement of solids. Each is first order in ΔT, so each has a response
  map: "which warm cell does this tower hear".
- **Not in S13:** phase change (boiling, freezing, condensation), combustion, humidity. They go in
  `plan/IDEAS.md` until someone needs them.

Method choices (radiation by ray-traced view factors or discrete ordinates; implicit or explicit
conduction; how air beyond Boussinesq is handled) are open until the research note
`docs/research/s13-heat.md` is written and checked.

### 9.3 Beyond the scene (S14–S16) — the planet, other instruments, the science case

The voxel scene is at most a small town. Everything beyond it is **never voxelised**: it enters as
analytic and tabulated sources evaluated at the site, through the same forward model and the same
error budget in radians. A gradiometer's signal from a mass falls as 1/r³, so what arrives from far
away is tides and waves, not attraction.

**S14, the wider world.**
- **Frames, as planet-scale engines do it.** Sun-centred → Earth-centred inertial → Earth-fixed and
  rotating → local east-north-up at the site → each tower's frame (§3.4). Positions stay in the frame
  they belong to and are converted in f64 only where frames meet; time is kept as a two-part date.
  The GPU still sees only local offsets (split doubles where a far position must reach it); the
  viewer gains a depth range from a room to a planet. The dev scene `precision-far` grows to
  `precision-globe`.
- **Never difference big numbers** applies here most of all: the Moon enters as a tidal gradient at
  the tower, not as two attractions 5 m apart.
- **Far sources**, each a small model with a stated error: lunar and solar tides with the solid
  Earth's and the oceans' response; pressure systems, fronts, clouds, rain and ground water;
  atmospheric and seismic Newtonian noise as spectra; ocean microseisms; teleseismic waves and the
  prompt gravity signal of great earthquakes; a global pressure wave; tsunamis and other cases
  expected to read nothing.
- **Earth's rotation**: Coriolis and centrifugal terms on the atoms' flight and the Sagnac phase, as
  instrument terms; Coriolis on air and water where the scene is large enough to care.
- **Real data, cached outside the repo**: an Earth interior model, topography and bathymetry, a global
  gravity field, ocean-tide models, weather reanalysis, earthquake catalogues, ephemerides. Each
  dataset's licence is checked before it is used and recorded in the bundle; none is committed.
- **Replay a real day**: the Moon, the weather and the earthquakes of a chosen date at a chosen site.
- **The solar system and things in space** (owner, 24 Nov 2026): any body of the solar system can host
  the hierarchy below (the Moon, Mars and others, each with its own LOD tree from elevation data,
  placed by the ephemeris; a detector's world gets the bubble). Satellites from public element sets
  (SGP4), asteroids and comets from the orbit catalogues, debris and user-placed spacecraft, propagated
  by Kepler, SGP4 or an n-body integrator for close approaches; mostly nulls on Earth.
- **The whole planet as one level-of-detail hierarchy** (owner, 24 Nov 2026). Full-precision bubbles
  around each detector; outside them the world coarsens in rings until it is a cube-sphere of the
  whole Earth, then the Moon and the Sun. One hierarchy serves both readers:
  - *drawn*: S4's clipmap rings extended outwards, real terrain and bathymetry tiles streamed from a
    cache outside the repo, logarithmic or reversed depth so a tower and a continent sort in one
    view, the Moon and the Sun where the ephemeris puts them;
  - *read*: every coarse node carries its mass moments, so the instrument reads the far planet
    through S5's bounded walk with its error in radians, as it reads the scene. Static mass (terrain,
    ocean, ice, crust and mantle from a layered Earth model) is read this way; the error budget
    decides how coarse each ring may be, and the test is that the read converges to the
    gravity-field model's value at the site as the rings refine.
  - *moving far away*: tides, weather, ground water and seismic waves are not simulated cell by cell
    across the planet. They are driven from data and models (ephemeris, tidal models, reanalysis,
    travel-time tables) into the coarse cells, so the rings carry the right mass at the right time.
    Simulating the planet's atmosphere and oceans at the resolution that matters is a weather model's
    job, not this engine's; what the instrument needs from them is their mass, which the data give.
  The view says, ring by ring, what is read, at what error, and what is only drawn.
- **The boundary**: a far source hands over to the voxel scene at its edge without double counting;
  a test moves a mass across the boundary and demands no step.

**S15, instruments anywhere.**
- **Per-tower instruments** came forward to S3 (owner, 23 Nov 2026): height, separation, sequence and
  launch per tower as an override on the array's default, readers shared by instrument, readout
  times per tower, so bundles are ragged (`docs/research/s15-instruments-anywhere.md` §3). What stays
  here is mixing across axes and gravity environments.
- **Any axis**: the baseline is a direction, not "up". A horizontal gradiometer reads a different
  component of the gradient tensor and its atoms fall across the beam; the flight, the arms and the
  propagation integral are generalised, with the vertical case unchanged to the bit.
- **Any gravity**: local g and its gradient from latitude, height and the gravity field model at the
  site, feeding the launch rule and the flight; other bodies (Moon, Mars) as presets.
- **In orbit**: free fall, long interrogation times, the Earth's field along the orbit as the signal;
  one satellite, or two with a long baseline as the space proposals have it.
- **Systematics budget**: laser phase noise, mirror vibration, magnetic fields, the cloud's size and
  temperature, launch jitter, rotation, and the blackbody shift from warm surfaces (from S13's
  temperature field).

**S16, the science case.**
- **Signal injection and recovery**: gravitational-wave and ultralight-dark-matter signals through the
  full world, with the analysis layer finding them or not.
- **Gravitational waves from cosmic events** (owner, 22 Nov 2026): a source library, not one test
  tone. Inspiralling binaries (black holes, neutron stars, intermediate-mass black holes, white
  dwarfs) with their chirp through the instrument's band, a stochastic background, bursts; each with
  a sky position, distance, inclination and polarisation, read through the antenna pattern of the
  baseline as the Earth turns (S14's frames). Known events can be replayed: where a catalogued
  merger would have been in this band in the days before it reached the ground detectors' band.
  The strain is drawn beside the phase it produces, with the instrument's sensitivity curve.
- **Theoretical and speculative sources** (owner, 22 Nov 2026), each an optional source with its
  parameters, the paper it comes from, and a label saying it is hypothetical:
  - *as gravitational waves*: mergers of intermediate-mass and primordial black holes, bursts from
    cosmic strings, a stochastic background from a phase transition in the early universe;
  - *as fields the atoms feel*: ultralight dark matter of several kinds (scalar, vector), a
    transient from a dark-matter clump or a domain wall passing through the Earth, slow drifts of
    the fundamental constants;
  - *as changes to gravity itself*: a fifth force of Yukawa form added to Newton's law for every
    mass in the scene, so that a moving mass in the room becomes a test of it; a violation of the
    equivalence principle between two isotopes;
  - *as masses passing by*: a small primordial black hole or other dark compact object crossing near
    the Earth, read by the far-field layer of S14 as any other moving mass.
  Each answers one question: what would it look like here, and could it be told from the world's
  own noise.
- **Networks**: two or more sites on one planet sharing the far field, with independent local worlds.
- **Site selection**: a noise budget for any place on Earth from the data of S14.
- **A twin**: the simulation run on the same inputs as a real instrument, and the difference drawn.

## 10. How the work runs — builders and reviewers

- **Builder agent** per slice: writes the slice brief (requirements traced to tests, tolerances,
  demo), then the code, then the demo and screenshots.
- **Reviewer agent**, fresh context, after each slice: reads the diff against the brief and this plan;
  checks the physics and the oracle tolerances, looks for tests that cannot fail, runs the demo, and
  reviews the screenshots for UX against §6. Findings are fixed before the slice is merged.
- **Parallel work in cloud sessions** (`claude --cloud "…"` from the CLI, one task per session):
  independent pieces such as the shapes library, MCP tool coverage, the scene catalogue, docs and the
  paper-style exporter. GPU work stays on the build PC.
- **Cloud sessions run without prompts.** They run in isolated VMs, so give them full permission:
  commit `.claude/settings.json` with the permission default mode set to auto (or bypass permissions)
  and broad allow rules for cargo, git and the toolchain, and pick that mode when starting a session.
  Check on the first run that no prompts appear; adjust the file if they do.
- **Owner checkpoints:** at the end of each slice, a short note: what is new (with screenshots), what
  was decided and why, what to look at. Anything that cannot be undone waits for the owner.

How to start it and keep it running unattended: `plan/KICKOFF.md`. Agent definitions:
`.claude/agents/{builder,reviewer,researcher}.md`. Progress between runs: `plan/PROGRESS.md`.

## 11. Open questions

- Whether to keep `oracle` as a git dev-dependency on v1 crates or to freeze v1's anchor outputs as
  fixtures and drop the dependency (faster builds; loses the ability to run new scenes through v1).
- The array geometry and firing rules for an outdoor or multi-tower deployment — George.
- Licence and publishing of the engine (owner).
