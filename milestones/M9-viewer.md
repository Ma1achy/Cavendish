# M9 — Viewer (implementation brief)

> Eyes on the forward model: an egui + wgpu inspector that makes a run *legible* — the moving
> clouds, the array, the signal, the spectrum, a time scrubber — and fails soft. A development
> tool, deliberately scoped. Read with `design/viewer.md`.
>
> **Prereq:** M6 (runs to inspect; shares `compute`'s wgpu). Independent of M7/M8.
> **Crates touched:** `viewer` (new).

---

## 1. Requirements

| ID | Requirement |
|---|---|
| M9-R1 | 3D scene: world-frame source cloud(s) (`source_cloud` posed by `source_position`/`source_orientation`), array markers from `detector_placement`, optional field view, optional spin-axis vector (`source_angular_velocity`). |
| M9-R2 | 2D panels: per-detector `signal` vs `time`; `periodogram` when present; `mask` as shaded cycles. |
| M9-R3 | A scrubber over the measurement axis `T` with play/pause, driving the 3D pose and the 2D cursor coherently. |
| M9-R4 | Data paths: live (`generate.run` with a viewing `FieldSet`) and loaded (a serialised bundle, read-only). Field view from either the stored `field_grid` or an on-demand `gravity` slice (default: the cheap slice). |
| M9-R5 | Tweak-and-rerun: change a parameter, re-run, view updates. |
| M9-R6 | Fails soft: run errors → message; absent optional fields → disabled panels; degenerate scenes → empty scene. Never a crash. |

---

## 2. Design

### 2.1 Layout

```
┌────────────────────────────────────────────┬──────────────────────────┐
│  3D (wgpu):  clouds ▪ array ▲ field ↑↑     │  Scenario panel (egui)   │
│      spin axis ─►                          │   params · FieldSet ·    │
│                                            │   [Run] [Load bundle]    │
├────────────────────────────────────────────┼──────────────────────────┤
│  signal (T,D) — egui_plot, cursor ─┐       │  periodogram (F,D)       │
│  mask: shaded cycles               │       │  (disabled if None)      │
├────────────────────────────────────┴───────┴──────────────────────────┤
│  ◄◄  ▶  ►►   t-scrubber ────────────●────────────────  t = 214/450    │
└────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Rendering split

wgpu (the **same** dependency `compute` uses — one graphics stack): instanced point sprites for the
cloud, a handful of marker instances for the array, arrow instances or one textured slice for the
field. egui/`egui_plot` for everything 2D. Nothing performance-critical at inspection scale
(one scenario, `N ≲ 10⁵` points).

### 2.3 Field view — two modes

Stored `(T,X,Y,Z,3)` grid when the bundle has it; otherwise sample `gravity` on one plane at the
scrubbed `t` on demand (cheap: one slice, not a volume) — the default, so inspecting the field
never forces the storage-dominant `FieldSet.field`.

**Realisation (M9).** Only the on-demand slice is implemented — the stored-grid mode is **deferred**
(no `field_grid`/`FieldSet.field` producer exists yet, `state.md` §6). `field_two_modes` therefore
validates the slice sampler against a **direct `gravity::field` reference** independently assembled at
the same nodes (reference-independence), rather than against a second stored copy.

---

## 3. Pseudocode

```
fn frame(app):
    ui.panel(scenario_editor);  ui.panel(plots);  ui.scrubber(&mut app.ℓ, play)
    if clicked(Run):
        match generate::run(&app.scenario) { Ok(b) => app.bundle = b,
                                             Err(e) => app.toast(e) }        # fails soft
    if let Some(b) = &app.bundle:
        t = b.time[app.ℓ]
        scene.set_clouds(pose(b, app.ℓ));  scene.set_markers(b.detector_placement)
        if app.field_on: scene.set_field(slice_or_grid(b, t))
        plots.cursor_at(app.ℓ)                                               # 3D/2D coherence
```

---

## 4. Tests

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `scrub_maps_index` | slider index ℓ → `time[ℓ]` for uniform and gappy schedules | exact |
| unit | `none_disables` | `periodogram = None` ⇒ panel disabled, no error path taken | structural |
| unit | `pose_placement` | cloud vertices = `source_orientation[ℓ]`·body + `source_position[ℓ]` | ≤1e-6 |
| integration | `renders_headless` | a `Scenario` runs and one frame renders under lavapipe (offscreen target) without panic | structural |
| integration | `field_two_modes` | on-demand slice vs an independent `gravity::field` reference at the same nodes (stored grid deferred, §2.3) | ≤1e-6 |
| integration | `load_bundle` | a serialised bundle renders read-only (no re-run required) | structural |
| e2e (review) | `tumble_visible` | a free-rotation source visibly flips (Dzhanibekov) while scrubbing; spin-axis vector tracks ω | review |
| e2e (review) | `tweak_rerun` | change source mass → re-run → signal amplitude scales; view updates | review |
| e2e | `fails_soft` | an induced `RunError` and an empty scene both degrade gracefully, app alive | structural |

---

## 5. Exit requirements

| Requirement | Check | Tol |
|---|---|---|
| coherent scrub | `scrub_maps_index`, `pose_placement` | exact/1e-6 |
| renders a run | `renders_headless`, `tumble_visible` | structural/review |
| field modes | `field_two_modes` | ≤1e-6 |
| loop | `tweak_rerun`, `load_bundle` | review/structural |
| never crashes | `fails_soft`, `none_disables` | structural |

## 6. Traceability

M9-R1 → pose_placement, tumble_visible · M9-R2/R3 → scrub_maps_index, none_disables · M9-R4 → field_two_modes, load_bundle · M9-R5 → tweak_rerun · M9-R6 → fails_soft.
