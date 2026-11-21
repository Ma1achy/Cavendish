# M13 — The scene viewer (plan / implementation brief)

> **Status: planned — nothing here is built.** Design: `design/viewer.md` (the M9 viewer this
> extends, and its new §10). Depends on: M11 (scenes of many sources), M12 (bodies that deform, air
> that moves). Independent of the GPU compute work; can be built in parallel with it.
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
