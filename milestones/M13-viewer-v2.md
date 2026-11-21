# M13 — The scene viewer (plan / implementation brief)

> **Status: planned — nothing here is built.** Design: `design/viewer.md` (the M9 viewer this
> extends, and its new §10). Depends on: M11 (scenes of many sources), M12 (bodies that deform, air
> that moves). Independent of the GPU compute work; can be built in parallel with it.
>
> **Order:** built after the voxel core (M14) — the replay stores each body's tree once plus per-frame
> poses or bone transforms, and the body pass draws merged leaves, so §3.1's per-frame point positions
> are the fallback for displaced bodies only. Read §3.1–3.2 with that substitution.
>
> **Extended (§9):** the full interface — panels, menus, the configuration editor, the console, and a
> timeline and flight view for towers that fire on their own clocks. Those parts depend on **M15**
> (per-tower timing and the `SceneSpec`); phases A–D below do not. The look is the one in the browser
> mock-up (egui dark, Ubuntu Light / Ubuntu Mono, 28 px menu bar, side panels with collapsing headers,
> a bottom strip, a footer that opens into the console).
>
> **Goal.** See a v2 scene the way the instrument and a person would both want to: bodies walking,
> falling and folding in continuous time; the air's plume as a volume; what each gradiometer is
> listening to; and the signal, split by channel, moving under the same cursor.

---

## 0. Toolkit — stay on egui + wgpu

The viewer already runs on **`eframe`/`egui` 0.29 for the interface and `wgpu` 22 for the 3-D**, the
same wgpu the compute backend uses. Keep it. `egui` is the de facto immediate-mode GUI in Rust (the
role Dear ImGui plays in C++); Dear ImGui's Rust bindings exist but would add a second UI stack and a
C++ build for no gain. Native on the host: Metal on macOS, Vulkan on Linux — no container.

Additions within that stack: `egui_plot` (already present) for the traces; `egui_dock` for dockable
panels is the one new dependency worth considering, and is optional (R9).

---

## 1. Why the M9 viewer is not enough

The M9 viewer draws a `StateBundle`. That is the right contract for a rigid scene and it cannot show
a v2 one, for three specific reasons:

1. **A bundle poses a rest cloud by a track.** An articulated figure's track is its root; drawn from
   the bundle its limbs would never move. Parcel bodies have to be drawn from `world_cloud_at(t)`.
2. **A bundle is sampled at measurement times** — every 0.5–2 s. Motion and convection need continuous
   playback, and the instrument itself integrates over the 1.46 s *before* each measurement, which a
   tick-by-tick view hides.
3. **The air is not in the bundle at all**, and `simulate_air` currently keeps only the mass anomaly,
   discarding temperature and velocity.

So the viewer needs a second data source alongside the bundle: a **scene replay**.

---

## 2. Requirements

| ID | Requirement |
|---|---|
| M13-R1 | **Scene replay.** A `Replay` is what the viewer draws: per-source world clouds sampled on a playback time axis, optional air frames (temperature, velocity, mass anomaly) on their grid, and the `StateBundle`. Built live from a `Scenario` (query `world_cloud_at(t)`), or loaded from a file. `StateBundle` is not changed. |
| M13-R2 | **Continuous-time playback.** One clock `t` drives everything: play/pause, speed, scrub, step. Measurement times are ticks on the timeline; the flight window `[t_ℓ − 2T, t_ℓ]` of the nearest measurement is shaded. |
| M13-R3 | **Bodies.** Every source drawn from its world cloud at `t` as instanced, shaded cubes sized to its pitch; one colour per source, ambient sources muted; articulated and deforming bodies animate. Trails (the track so far) and velocity arrows are toggles. |
| M13-R4 | **Air.** Three views of the air field, each a toggle: a movable **slice plane** (temperature, density anomaly or speed, with a colour bar); **velocity glyphs** on that slice; and a **volume render** of the temperature anomaly (ray-marched 3-D texture) so a plume reads as a plume. |
| M13-R5 | **What the instrument hears.** For a selected detector: its **response map** — the phase per unit mass in each cell (`GridField`'s `W`, summed over the flight) — as a slice or volume; and the M9 gravity-gradient slice, now over the whole scene. |
| M13-R6 | **Signal panels.** Per-detector traces with the time cursor; the **channel decomposition** stacked (targets / ambient / ULDM / noise) when present; the periodogram; masked cycles shaded. Clicking a trace selects that detector in 3-D. |
| M13-R7 | **Scene library and background runs.** A picker of ready scenes (a pass, the walker, a dropped object, a radiator room, a mixed scene) with their parameters; **Run** simulates on a worker thread with a progress line and a cancel button — the interface never blocks on a fluid simulation. |
| M13-R8 | **Model overlay.** Load a predicted track (`.npy`, `(T, 2)` or `(T, 3)`) and draw it against the true one, with the error per measurement plotted — the proof-of-concept tracker, visible. |
| M13-R9 | **Layout and capture.** Panels that can be shown, hidden and resized; a screenshot and a numbered frame-sequence export of the 3-D view. |
| M13-R10 | **Fails soft, stays coherent** — the M9 spines carry over: a panic in a run is a toast; an absent field is a disabled control; every view reads the one clock. |

---

## 3. Design

### 3.1 The replay — the one new data type

```rust
pub struct Replay {
    pub times: Vec<f64>,                 // playback axis, uniform, e.g. 30 Hz over the scene span
    pub sources: Vec<SourceReplay>,      // one per WorldSource, tracked or not
    pub air: Option<AirReplay>,
    pub bundle: StateBundle,             // signals, tracks, placements — unchanged contract
}
pub struct SourceReplay {
    pub channel: Channel,
    pub pitch: f32,
    pub masses: Vec<f32>,                // (N)        constant — parcels are never re-weighed
    pub positions: Vec<Vec<[f32; 3]>>,   // (F, N, 3)  world frame; a rigid source stores (rest, pose[F]) instead
}
pub struct AirReplay {
    pub grid: Grid3, pub t0: f64, pub dt: f64,
    pub temperature: Vec<Vec<f32>>,      // (F_air, cells)
    pub velocity: Option<Vec<Vec<[f32; 3]>>>,   // cell-centred, optional (3× the storage)
    pub solid: Vec<Vec<u8>>,             // which cells a body fills, per frame (bit-packed)
}
```

- It lives in `viewer` (it is a rendering artefact, `f32`, lossy) — not in `state`, whose contract
  stays the bundle.
- **Live path:** `Replay::record(&Scenario, playback_dt)` queries each source. **Loaded path:** one
  file, `bincode` like the bundle. A rigid source is stored as rest cloud plus poses, so rigid scenes
  stay small.
- **`physics` change needed:** `simulate_air` gains an optional recorder for temperature and
  cell-centred velocity (today it keeps only `mass_anomaly`). Additive; default off.
- **Size.** A 70 kg figure at 0.05 m pitch is ~5 000 parcels: 60 s at 30 Hz is 108 MB as raw
  `[f32; 3]` — store skinned sources at 10 Hz and interpolate, or store bone transforms plus weights
  and skin in the vertex shader (the better answer; R3's stretch goal). Air at 19 200 cells × 240
  frames × 4 B is 18 MB.

### 3.2 Rendering

All within the existing two-pipeline renderer (`render/mod.rs`), plus two passes:

- **Bodies** — the existing instanced-cube pipeline; per-source instance buffers rewritten each frame
  from the replay (interpolating between stored frames). 10⁵ cubes is comfortable.
- **Slice** — one textured quad; the texture is a 2-D cut through the selected field, uploaded when the
  plane or the frame changes. Colour maps: a perceptually uniform sequential map for temperature and
  speed, a diverging one centred on zero for density anomaly and response.
- **Glyphs** — the existing line list, arrows subsampled to at most ~40 per side.
- **Volume** — a new fragment pass: the room's bounding box rasterised, a 3-D `R16Float` texture of
  `T − T₀` ray-marched front to back with a transfer function (opacity ∝ |anomaly|), composited over
  the scene with depth so bodies occlude the plume. 64–128 steps is ample for rooms of this size.
- **Labels and colour bars** — egui, as now.

### 3.3 Interface

```
┌─ scene ─────────┬──────────────── 3-D view ─────────────────┬─ inspector ──────┐
│ library ▾       │                                            │ source: walker   │
│ parameters      │      bodies · air slice/volume · array     │ mass 70.0 kg     │
│ [Run] ▓▓▓░ 42%  │      response map · trails · gizmos        │ parcels 5 120    │
│ layers:         │                                            │ detector: 2      │
│ ☑ bodies ☑ air  ├─────────────── timeline ───────────────────┤ ΔΦ −0.184 rad    │
│ ☐ response …    │ ▶ ×1  ├──┼──┼──▓▓│──┼──┼──┤  t = 6.42 s     │ channels …       │
├─────────────────┴────────────────────────────────────────────┴──────────────────┤
│ signal (per detector, cursor)  │  decomposition (stacked)  │  periodogram       │
└───────────────────────────────────────────────────────────────────────────────────┘
```

### 3.4 Module plan

New pure modules (tested headlessly, like M9's): `replay` (record, load/save, interpolate),
`clock` (time ↔ playback frame ↔ nearest measurement, flight window), `colourmap`, `slice` (plane →
texture), `response` (detector → per-cell weights; reuses `physics::GridField`), `library` (named
scenes → `Scenario` + parameters), `worker` (thread, progress, cancel), `overlay` (predicted tracks).
Renderer additions in `render/`: `slice.wgsl`, `volume.wgsl`.

Dependency edge added: `viewer → physics, world` (it must build scenes and read the response).

---

## 4. Build order

Each phase leaves a working viewer.

| Phase | Delivers | Requirements |
|---|---|---|
| **A — replay and clock** | `Replay`, live recording, continuous playback, bodies animated from world clouds, timeline with ticks and flight window. The walker walks. | R1, R2, R3, R10 |
| **B — signals** | decomposition panel, detector selection, cursor on the continuous clock | R6 |
| **C — air** | air recorder in `physics`; slice plane, glyphs; then the volume pass | R4 |
| **D — the instrument's view** | response map; scene-wide gradient slice | R5 |
| **E — workflow** | scene library, background runs with progress and cancel, save/load replay | R7, R1 (loaded path) |
| **F — overlay and capture** | predicted-track overlay and error plot; screenshot and frame export; dockable layout | R8, R9 |

A, B and E make it a useful tool; C and D make it a good one.

---

## 5. Tests

The eye is not the check (M9's rule): everything asserted lives in pure modules or the headless
renderer.

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `replay_matches_world_cloud` | recorded positions at a stored frame = `world_cloud_at(t)` for rigid, skinned and displaced sources | ≤1e-6 m (f32) |
| unit | `replay_conserves_mass` | every frame of every source sums to the source's mass | ≤1e-6 rel (f32) |
| unit | `replay_roundtrip` | save → load is identical | exact |
| unit | `clock_maps_consistently` | time → frame → time; nearest measurement; flight window `[t_ℓ − 2T, t_ℓ]` | exact / ≤1e-12 |
| unit | `rigid_replay_equals_bundle_pose` | a rigid source's replay agrees with the M9 pose path | ≤1e-6 m |
| unit | `slice_samples_the_field` | a plane through known cells returns those cells' values; out-of-room is transparent | exact |
| unit | `colourmap_endpoints_and_monotone` | sequential map monotone in lightness; diverging map neutral at zero | structural |
| unit | `response_map_equals_phase` | Σ response × a test mass frame = `per_ifo_fast` for that static field | ≤1e-9 |
| unit | `overlay_error_is_distance` | plotted error = distance between predicted and true track | ≤1e-12 |
| integration | `worker_reports_and_cancels` | progress is monotone; cancel stops the run; the UI thread never joins it synchronously | structural |
| integration | `library_scenes_run` | every library scene builds, runs and records a replay | structural |
| integration | `fails_soft_on_panic` | a panicking source becomes a toast (extends M9's test) | structural |
| e2e | `headless_frame_per_layer` | each layer (bodies, slice, glyphs, volume, response) renders a non-empty frame off-screen | structural |
| e2e | `walker_limbs_move_on_screen` | projected extent of the walker changes between two playback times | structural |

## 6. Exit requirements

| Requirement | Check |
|---|---|
| a v2 scene is legible | the walker's limbs animate; the plume is visible as a volume and on a slice; review on the M3 |
| one clock | `clock_maps_consistently`; 3-D, timeline and traces agree at any `t` |
| the replay is faithful | `replay_matches_world_cloud`, `replay_conserves_mass`, `rigid_replay_equals_bundle_pose` |
| the instrument's view is the instrument's | `response_map_equals_phase` |
| never blocks, never crashes | `worker_reports_and_cancels`, `fails_soft_on_panic` |
| M9 still holds | the existing viewer tests pass; a plain bundle still loads and renders |
| performance | 60 fps on the M3 with a 10⁵-cube scene and a 64³ volume (measured, not assumed) |

## 7. Traceability

R1 → `replay_*` · R2 → `clock_maps_consistently` · R3 → `replay_matches_world_cloud`,
`walker_limbs_move_on_screen` · R4 → `slice_samples_the_field`, `colourmap_*`,
`headless_frame_per_layer` · R5 → `response_map_equals_phase` · R6 → `clock_maps_consistently` (cursor),
review · R7 → `worker_reports_and_cancels`, `library_scenes_run` · R8 → `overlay_error_is_distance` ·
R9 → `headless_frame_per_layer` (capture path) · R10 → `fails_soft_on_panic`.

## 8. Open questions

- **Skinning on the GPU.** Storing bone transforms and weights and skinning in the vertex shader makes
  replays tiny and playback exact at any `t`; it needs `Parcels` to expose its weights and transforms
  (today the map is an opaque closure). Worth doing in phase A if `Parcels` is being touched anyway.
- **Where the replay is recorded** when datasets are generated in bulk: not at all by default — it is
  a viewing artefact, recorded on demand for one scene.
- **Streamlines** instead of glyphs for the velocity field; isosurfaces instead of ray-marching.
- **The spec.** The viewer section (`viewer` under System Components) should gain a sentence on the
  replay when phase A lands; it is not a data-contract change.

---

## 9. The interface, in full

§3.3 is a player. This section is the tool: every panel and menu, how a scene is configured from the
GUI, the console, and what changes when towers are not synchronised.

### 9.1 Additional requirements

| ID | Requirement |
|---|---|
| M13-R11 | **Everything editable is the `SceneSpec`** (M15). Property panels are generated from its schema (label, unit, range, doc string as hover text); no panel holds scene state of its own. |
| M13-R12 | **One edit path.** A slider, a text field, a console command, a loaded file and an undo all issue the same `Edit::Set { path, value }`. Edits are logged, undoable, and tagged with the stage they invalidate. |
| M13-R13 | **Stale, not blocked.** After an edit the affected stages show a *stale* badge and the old result stays on screen; **Run** re-runs from the earliest stale stage only (M15's `diff`). View edits never trigger a run. |
| M13-R14 | **Per-tower timeline.** One lane per tower; each shot is a bar from launch (`t − 2T_d`) to readout (`t`); offsets, jitter and masked shots are visible; the cursor crosses all lanes. Dragging a lane sideways edits that tower's offset. |
| M13-R15 | **Per-tower flight view.** Space–time diagrams on a shared *absolute* time axis, one row per tower (selected tower enlarged), each with its own `T`, launch velocity and baseline, and a marker where its clouds are at the cursor. |
| M13-R16 | **Signals without a shared sample index.** Traces are plotted against each tower's own readout times; the "all towers at the cursor" table shows each tower's latest reading and its age; cross-tower quantities are labelled as interpolated when they are. |
| M13-R17 | **Console.** The footer opens into a log stream (time, level, subsystem, message; level filters, text filter, copy, clear) with a command line bound to the same edit path. Errors open it. |
| M13-R18 | **Menus and keyboard** as §9.3 and §9.7; every menu action is also a console command and appears in the command palette. |
| M13-R19 | **Dataset builder.** A scene spec plus parameter distributions → a batch run on the worker with progress, throughput and an estimate of size on disk. |
| M13-R20 | **Profiler.** Per-stage wall-clock for the last run (shape, dynamics, air, forward by source, noise), frame time, device and adapter. |

### 9.2 Layout

```
┌ cavendish · dev   File  Edit  View  Scene  Array  Dataset  Windows  Help      Overlays ▾   ⌘K   t 9.10 · 60 fps ┐
├─ Outliner ───────┬──────────────────────── 3-D view ───────────────────────────┬─ Properties ───────────────────┤
│ ▾ Sources        │                                                             │ (of the selection)              │
│   walker    ●    │   bodies · air (slice / dots / volume) · towers and clouds  │ Tower 3                         │
│   radiator  ●    │   response map · trails · tree cells · measure tool         │  placement  x  y  z  tilt       │
│ ▾ Array          │                                                             │  timing     offset 0.292 s      │
│   tower 1 … 5    │                                                             │             jitter 0 ms         │
│ ▾ Air            │                                                             │  instrument T  u₀  Δr  (inherit)│
│ ▾ Noise · ULDM   ├─ Timeline ──────────────────────────────────────────────────┤  reading · channels at cursor   │
│ ▾ Run            │ T1 ▕██████▏   ▕██████▏   ▕██████▏                            │                                 │
│  shape ✓ air ⟳   │ T2    ▕██████▏   ▕██████▏   ▕██████▏         one lane/tower  ├─ Signals ──────────────────────┤
│  forward stale   │ T3       ▕██████▏   ▕██████▏   ▕██░░░▏ masked                │ per tower · by cause · spectrum │
├──────────────────┴────────────────────────────────────────────────────────────┴─────────────────────────────────┤
│ Flight (per tower, absolute time)          │ Decomposition / Response / Legend (tabs)                            │
├─────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ready · forward stale (tower 3 offset) · ⚠ 1                                             console ▴   Space play │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

Panels dock, tab, float and hide (`egui_dock`); layouts are saved by name; "Reset layout" is one
click. Three presets ship: **Explore** (above), **Configure** (outliner and properties wide, timeline
tall), **Dataset** (builder and run queue; small preview).

### 9.3 Menus

| Menu | Items |
|---|---|
| **File** | New scene from library ▸ · Open scene… (`.ron`) · Open replay / bundle… · Recent ▸ · Save scene · Save scene as… · Save replay… · Export ▸ (bundle `.npy` set, screenshot, frame sequence, signal CSV) · Quit |
| **Edit** | Undo · Redo · Edit history… · Copy value · Copy path · Paste value · Revert selection to default · Preferences… |
| **View** | Camera ▸ (orbit, plan, side, follow selection, frame selection, save view…) · Layers ▸ (the Overlays list) · Slice ▸ (off, air, response, gradient) · Colour maps… · UI scale ▸ · Layout ▸ (presets, save, reset) · Full-screen view |
| **Scene** | Add source ▸ (point, primitive, mesh…, walker, plate, dropped object, radiator) · Duplicate · Remove · Air… · Noise stack… · ULDM… · Seed ▸ (set, reroll) · Run (stale stages) · Run all · Cancel · Record replay |
| **Array** | Add tower · Remove tower · Layout ▸ (line, cross, ring, grid, from file…) · Timing ▸ (synchronise all, stagger evenly over 2T, stagger by…, add jitter…, reset) · Instrument defaults… · Copy settings from tower ▸ |
| **Dataset** | Builder… · Parameter distributions… · Run batch · Queue · Open output folder · Load predicted track… |
| **Windows** | one toggle per panel · Console · Profiler · Edit history · Keyboard · Legend |
| **Help** | Keyboard reference · Scene spec reference (generated from the schema) · About (version, adapter, wgpu) |

### 9.4 Panels

| Panel | Shows | Edits |
|---|---|---|
| **Outliner** | the spec as a tree: sources (colour swatch, channel, visibility eye, solo), array, air, noise, ULDM; the run's stages with state (fresh / stale / running / failed) | select, reorder, rename, hide, solo; context menu = the Scene and Array menus for that row |
| **Properties** | the selection's schema fields, grouped; inherited values greyed with the source of the value; derived read-outs (mass, parcels or leaves, Ra, flight apex) below | every field; drag-to-scrub numbers; reset-to-default per field; a field edited since the last run is marked |
| **3-D view** | §3.2's passes; tower tags with live reading; gizmos for the selection | move / rotate a source's start pose or a tower's placement with a gizmo (issues the same `Edit`); measure tool (distance, and range to each tower) |
| **Timeline** | lanes per tower (R14); source events (contact, sleep, wake) as ticks; air span; the cursor | scrub; drag a lane to change an offset; click a shot to select `(tower, ℓ)`; box-select shots to mask |
| **Signals** | per-tower traces on own readout times; split by cause on separate scales; periodogram tab; predicted-track error tab | channel visibility; which towers are overlaid; y-scale lock |
| **Flight** | R15 | none (read-out); hover gives `τ`, arm separation, height |
| **Response** | the selected tower's response on the slice or as a volume, for the shot under the cursor | plane, log scale, range |
| **Decomposition** | at the cursor: targets / ambient / ULDM / noise per tower as a table and bars; residual of the identity | none |
| **Legend** | generated from what is on screen: colours ↔ sources and towers, dot size and colour bars with units | none |
| **Console** | R17 | commands |
| **Profiler** | R20 | none |
| **Dataset builder** | R19 | distributions per spec path (fixed / uniform / normal / choice), count, root seed, output folder, fields |
| **Edit history** | every `Edit` since load, with stage tags | click to revert to that point |

### 9.5 Configuration — how an edit flows

```
slider / field / gizmo / console / file ──► Edit::Set{path, value} ──► SceneSpec ──► diff ──► stale stages
                                                   │                                              │
                                             history (undo)                              Run ──► worker: Shape → Dynamics → Air → Forward → Readout
                                                   │                                              │
                                              console log                             replay + bundle swapped in when complete
```

- The viewer keeps the **last complete result** and the **current spec** separately. The 3-D view and
  plots draw the result; the panels edit the spec. They differ exactly when something is stale, and
  the footer says what and why (`forward stale (tower 3 offset)`).
- Cached stage outputs are keyed by the hash of the spec fields that stage reads, so changing a tower
  offset re-runs the forward pass over a recorded air field in seconds, not the fluid simulation.
- **Auto-run** is a toggle, on only for stages under a time budget measured from the last run (the
  forward pass, typically); never for the fluid.
- Timing edits get a dedicated dialog as well (**Array ▸ Timing**): a table of towers × (offset,
  jitter, `T`, `u₀`), a preview of the lanes, and the three bulk actions.

### 9.6 Towers on their own clocks — what changes on screen

- **Timeline:** the single row of measurement ticks becomes lanes. The shaded "flight band" under the
  cursor becomes one band per tower, at different places.
- **3-D view:** each tower's atom clouds are drawn where *that* tower's current shot has them — or
  absent, between shots. A tower mid-flight has its arms drawn; one that has just read out pulses.
- **Flight view:** rows share the absolute time axis so the stagger is visible as a horizontal shift,
  and a shorter `T` as a narrower, lower diagram.
- **Tower tags and the table:** each shows its latest reading and how old it is (`−0.884 rad · 0.31 s
  ago`); "at the cursor" no longer means "the same sample".
- **Response panel:** the response is per `(tower, shot)` because the air it weighs is the air during
  *that* tower's flight; with a uniform per-tower cadence it is one map per tower, shifted in time.
- **Decomposition:** per tower at its own latest shot; any cross-tower combination is computed on
  interpolated values and labelled so.

### 9.7 Console

- **Stream.** Rows of `time · level · subsystem · message`; subsystems are `app`, `spec`, `edit`,
  `shape`, `dynamics`, `air`, `forward`, `noise`, `render`, `worker`. Filters by level and text; copy;
  clear. The engine logs through the `log` facade into a ring buffer (one small dependency); the same
  buffer feeds the profiler's event list.
- **What gets logged without being asked:** every edit (`edit  Set array.tower[2].timing.offset 0 →
  0.292 s · forward stale`); stage start/finish with wall-clock; solver health (pressure iterations,
  residual, CFL, mass lost at the grid edge); anything the engine refuses (a query outside a field's
  span, an unsupported contact pair) as an error with the offending values.
- **Command line.** `get <path>` · `set <path> <value>` · `reset <path>` · `ls [path]` · `run [stage]`
  · `cancel` · `select <path>` · `cam <preset>` · `seek <t>` · `export <kind> <file>` · `help [cmd]`.
  Tab-completion from the schema; history with ↑; a failed command prints the schema's expected type
  and range. Every menu item is one of these, so a session can be replayed from its log.
- **Command palette** (`⌘K` / `Ctrl-K`): fuzzy search over the same commands and over spec paths.

### 9.8 Keyboard

Space play/pause · ←/→ step one playback frame · ⇧←/→ step to the previous/next shot of the selected
tower · 1–9 select tower · Tab cycle sources · F frame selection · S cycle slice · O orbit · P plan ·
`⌘Z`/`⇧⌘Z` undo/redo · `⌘R` run · `⌘.` cancel · `` ` `` console · `⌘K` palette · `⌘S` save scene.
Keys act on the panel under the pointer first, then globally; text fields swallow everything.

### 9.9 Additional modules, phases and tests

Modules: `edit` (the edit type, history, undo), `stages` (cache keys, stale tracking), `panels::*`
(one file per panel), `schema_ui` (schema → widgets), `console` (ring buffer, parser, completion),
`timeline` (lanes, hit-testing), `flight` (per-tower diagrams), `palette`.

| Phase | Delivers | Requirements |
|---|---|---|
| **G — spec-bound panels** | outliner, properties from the schema, edit path, history and undo, stale badges, staged re-run | R11, R12, R13 |
| **H — towers on their own clocks** | lane timeline, per-tower flight view, signals on own times, the timing dialog | R14, R15, R16 |
| **I — console and commands** | log stream, command line, palette, menus wired to commands, keyboard map | R17, R18 |
| **J — dataset and profiler** | dataset builder, run queue, profiler | R19, R20 |

G needs M15; H needs M15's timing; I and J need neither beyond the spec's paths.

| Level | Test | Asserts |
|---|---|---|
| unit | `every_schema_field_has_a_widget` | each schema type maps to a widget; no field is silently dropped |
| unit | `edit_undo_redo_roundtrip` | N random edits, undo all → the original spec, bit for bit |
| unit | `all_entry_points_issue_the_same_edit` | a widget change, a console `set` and a file load of the same value produce identical `Edit`s |
| unit | `stale_stage_matches_spec_diff` | the badge shown = M15's `diff` for a table of edits |
| unit | `cached_air_survives_a_timing_edit` | after a tower-offset edit, the air stage is not re-run and the forward result equals a full run |
| unit | `lanes_place_shots_at_detector_time` | bar start/end = `detector_time − 2T_d`, `detector_time`; hit-testing returns `(tower, ℓ)` |
| unit | `lane_drag_edits_offset` | a drag of `δ` pixels issues `Set …timing.offset` by `δ / pixels_per_second` |
| unit | `flight_rows_share_absolute_time` | a tower with offset `δ` is drawn shifted by `δ`; one with half the `T` spans half the width |
| unit | `age_of_reading_is_cursor_minus_readout` | the table's age column |
| unit | `console_parses_and_completes` | each command parses; completion lists schema children; bad input returns the expected type |
| unit | `menu_items_are_commands` | every menu entry has a command string that the parser accepts |
| unit | `log_ring_buffer_bounds` | capacity respected; filters compose |
| integration | `session_replays_from_its_log` | the commands logged in a session, replayed on a fresh app, give the same spec |
| integration | `dataset_builder_matches_batch` | the builder's output equals `generate::batch` called directly with the same root seed |
| e2e | `headless_layout_presets_render` | each layout preset renders every visible panel non-empty at 1440×900 and 1024×700 |

Exit, in addition to §6: a scene can be built, configured, run, re-run after a timing edit and
exported **without leaving the GUI or touching Rust**; the same scene saved to `.ron` and run from the
command line gives a bit-identical bundle.

### 9.10 Open questions

- **Gizmo edits of trajectories** (dragging a path's waypoints in 3-D) — wanted, but each `Path` kind
  needs its own handles; start with start-pose and tower placement only.
- **Text editing of the spec in-app** (a RON pane with live validation) versus properties only.
- **Remote runs** (viewer on the Mac, engine on the 2080 box): the worker boundary is already a
  message channel; a socket is the same shape. Not planned until local runs are the bottleneck.
