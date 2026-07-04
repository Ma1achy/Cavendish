# Cavendish — `viewer` drill-down

> Subsystem design for the `viewer` crate: a desktop **inspector** (egui + wgpu) for looking at a
> scenario and its run — the moving source clouds, the detector array, the field, the signal traces,
> the periodogram — with a scrubber over measurement time. A development and insight tool, not a
> product surface. Companion to `DESIGN.md` (§6 inventory) and `state.md` (the bundle it renders).
>
> **Dependencies:** `viewer → generate, state, gravity`. It defines no physics; it *runs* the engine
> and draws the result.

---

## 1. Responsibility & boundaries

**Owns:** the interactive window — the 3D scene, the 2D panels, the controls, and the time scrubber
that animates a run.

**Does not own:** any physics or orchestration. It obtains a `StateBundle` from `generate` (or a
serialised one from `state`'s cache) and renders it; where it needs a field slice it did not request,
it samples `gravity` directly (§4).

**What it is for:** seeing whether a run is *right* — does the trajectory look sane, does the field
make sense around the masses, does the signal have the expected structure, does a free-rotation
source actually tumble through its intermediate-axis flip (spec §5.4). Eyes on the forward model.

---

## 2. What it shows

- **3D scene.** The source cloud(s) in the *world* frame at the scrubbed time — `source_cloud`
  placed by `source_position`/`source_orientation` — as a point cloud; the detector array from
  `detector_placement` as markers; optionally the gravitational field as arrows or a coloured slice
  (§4). The spin axis (`source_angular_velocity`) can be drawn as a vector on a rotating body.
- **2D panels.** The per-detector `signal` traces against `time`; the `periodogram` when present;
  the `mask` shown as shaded/excised cycles.
- **The scrubber.** A slider over the measurement axis `T`, with play/pause, driving both the 3D
  pose and a cursor on the 2D traces — so motion and signal are read together.

---

## 3. Rendering

- **wgpu for the 3D**, the *same* wgpu the compute backend uses (`compute.md`) — one graphics
  dependency, shared. The cloud is an instanced point/sprite draw; the array a handful of markers;
  the field a set of arrow instances or a textured slice. Nothing here is performance-critical at
  inspection scale.
- **egui for the UI** — an immediate-mode panel layout for the controls (scenario parameters,
  `FieldSet` toggles, the scrubber) and, via `egui_plot`, the signal and periodogram plots. Immediate
  mode suits a tool that re-renders every frame off live state.

---

## 4. The data path

Two sources, both thin:

1. **Live run.** Build a `Scenario` in-app (or load one), call `generate.run` with a `FieldSet`
   chosen for *viewing* — the motion fields always, plus whatever the panels need (`periodogram`
   for the spectrum; `field_grid` only if the volumetric field view is on) — then render the bundle.
2. **Loaded bundle.** Open a serialised `StateBundle` from `state`'s cache (`state.md` §6) and render
   it read-only.

For the **field view** specifically, `viewer` need not store the heavy `field_grid`: it can sample
`gravity` on a slice at the scrubbed time on demand (cheap for one plane, `gravity.md`), which is
lighter than requesting the whole `(T,X,Y,Z,3)` grid. So the field visualisation has two modes — the
bundle's stored grid, or an on-demand slice — and defaults to the cheap one.

**Realisation (M9).** The on-demand slice is the implemented mode; the stored-grid mode is **deferred**
(no `field_grid`/`FieldSet.field` producer exists yet — `state.md` §6). The slice is validated against
a direct `gravity::field` reference (`field_two_modes`), so it is anchored to the kernel, not to a
second cached copy.

---

## 5. Interactivity

The point of the tool is the tight loop: **scrub** to watch the motion and the signal cursor move
together; **tweak-and-rerun** — change a source parameter or the array and call `generate.run` again
to see the effect immediately; **toggle** the field view (off by default, since it is the expensive
one). Because a run is deterministic (`generate.md` §8), re-rendering the same scenario is stable,
and the only cost of a tweak is one `run`.

---

## 6. Scope

Deliberately a **development tool**, not a shipped product: single-window, local, no persistence
beyond loading/saving a scenario or bundle, and no attempt at large-`S`/large-grid scale (inspect one
scenario at a time). Keeping it in this scope is what lets it stay a thin consumer rather than
growing its own state or rendering pipeline.

---

## 7. Robustness

The viewer must not bring down a debugging session: a `run` that errors (`generate.md` §10) is caught
and shown as a message, not a crash; an absent optional field (its `FieldSet` flag was off) renders
as an empty/disabled panel rather than an error; a degenerate scenario (no sources, empty array) draws
an empty scene. It fails soft, because its whole value is being on when something has gone wrong.

---

## 8. Exit requirements (definition of done)

| Requirement | Check | Tol |
|---|---|---|
| renders a run | a `Scenario` runs and its cloud/array/signal display correctly | review |
| scrub coherent | the 3D pose and the 2D cursor track the same `time` index | exact |
| rotation visible | a free-rotation source shows its tumble and spin axis over `T` | review |
| field modes | field view works from both the stored grid and an on-demand `gravity` slice | review |
| tweak-and-rerun | changing a parameter and re-running updates the view | structural |
| fails soft | run errors and absent optional fields degrade gracefully, never crash | structural |
| loads a bundle | a serialised `StateBundle` renders read-only | structural |

`viewer` needs only `generate`, `state`, and `gravity`; it is judged by whether it makes a run
*legible*, not by throughput.

---

## 9. Open sub-questions (resolve in implementation)

- **Field representation.** Arrows vs a coloured slice vs isosurfaces for the field, and whether the
  gradient tensor `Γ` gets its own view (it is what the instrument actually senses).
- **Multi-source clarity.** Colour/scheme for telling `S` sources apart, and per-source panel
  selection when the scene is crowded.
- **Playback.** Whether the scrubber interpolates between measurement ticks for smoother motion or
  steps discretely at the true cadence.
- **Scenario editing.** How much scenario construction lives in the UI vs loading a config file
  authored elsewhere.
