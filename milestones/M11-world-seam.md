# M11 — The world seam (implementation brief)

> **Status: implemented** on `v2-world-physics` (commit `6a8b770`); written up after the fact from the
> code. Design: `design/world.md`. Depends on: M1–M6 (the forward model, the channels, the batch
> path). Unblocks: M12 (physics sources), M13 (viewer), multi-target scenes.
>
> **Goal.** A scene is a list of sources of any kind behind one trait. The rigid body becomes one
> implementation and keeps its amortised evaluation. The forward-model spine is not edited, and every
> v1 scene produces a byte-identical bundle.

---

## 1. Requirements

| ID | Requirement |
|---|---|
| M11-R1 | One trait, `world::WorldSource`, for anything massive: `eval()` (the forward-model socket), `world_cloud_at(t)`, `channel()`, optional `track_at`/`body_cloud`, optional `per_ifo_fast`. |
| M11-R2 | `world::Rigid` wraps any `SourceDynamics` and evaluates through the rigid (inverse-pose) socket — the amortised path is preserved, not replaced. |
| M11-R3 | `Scenario` holds `Vec<Box<dyn WorldSource>>`. `Scenario::new(Box<dyn SourceDynamics>, …)` keeps its signature; `Scenario::scene` and `with_source` build general scenes. |
| M11-R4 | `generate::run` routes each source by channel and socket; ambient sources are recorded with the atmospheric channel; the decomposition identity still holds exactly. |
| M11-R5 | The bundle's source axis `S` carries every source that has a body frame. `StateBundle` gains no field. |
| M11-R6 | **Every v1 scene serialises byte-for-byte as on `main`.** `math`, `gravity`, `instrument`, `analysis`, `compute`, `reference` have an empty diff. |
| M11-R7 | `generate::batch` runs arbitrary scene builders across cores, in index order, identically for any thread count; `generate::write_dataset` stacks bundles as `.npy`. |

---

## 2. Equations

Nothing new. The seam rests on two facts already in the spec:

**Linearity** (`eq:doublediff`): the phase is linear in the potential, so a scene's phase is the sum of
its sources' phases and the per-channel passes superpose exactly,

```
ΔΦ = Σ_targets ΔΦ_s + Σ_ambient ΔΦ_s + ΔΦ_uldm + noise.
```

**Rigid invariance** (`design/gravity.md` §3): for a rigid source the potential at a world point is the
body-frame potential at the back-transformed point, `V(p, t) = V_body(pose(t)⁻¹ p)`. This is the
amortised path; `world_cloud_at(t) = pose(t) ⊗ body` is the same mass seen the general way, and the
two must give the same phase (R2's test).

---

## 3. Design

```
scenario.sources ──► generate::run
                       │  for each source:  channel() → {targets, ambient}
                       │                    eval()    → Rigid(r)  → group.rigid
                       │                                Field(f)  → group.fast   (PI in force, per_ifo_fast offered)
                       │                                          → group.fields (otherwise)
                       ▼
        model.delta_phi(rigid, fields, det, t)  [+ Σ fast double-differences]
```

- A v1 scene yields the same argument lists in the same order as before; with no fast-path source
  nothing is added to the result. That, not a tolerance, is what makes R6 hold.
- Whether a source offers a fast path is probed once, at the first (detector, measurement); a source
  that offers it must offer it at every measurement.
- Under `QuasiStaticGradient` the fast path is not taken (it reformulates the propagation integral).
- `batch` builds and runs each scene wholly on one worker thread, keyed by `scenario_key(root, i)`, so
  a `Scenario` need not be `Send` and the output depends only on the index.

---

## 4. Tests

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `world::rigid_fast_path_equals_world_cloud` | inverse-pose phase = phase of the reposed world cloud, tumbling body | ≤1e-11 |
| unit | `world::rigid_reports_track_and_conserves_mass` | track = pose; total mass constant; `Ambient` changes the channel | ≤1e-12 |
| unit | `state::npy_header_is_aligned_and_checked` | `.npy` header 64-byte aligned; shape/length mismatch rejected | exact |
| integration | `generate/tests/world::sources_superpose` | two-body scene = sum of each alone; `S = 2` | ≤1e-12 |
| integration | `generate/tests/world::mixed_scene_decomposes_exactly` | rigid target + articulated figure + ambient wall + grid field: channels sum to the signal; QS model runs | ≤1e-10 |
| integration | `generate/tests/world::batch_is_ordered_reproducible_and_exportable` | `batch` = per-index `run`, twice identical; dataset files present with consistent sizes | exact |
| e2e | `generate/examples/bundle_hash.rs` on `main` vs branch | 24 v1 scenes: serialised bundles equal | **byte-identical** |
| e2e | the v1 suite | all 115 pre-existing tests pass unmodified | — |

---

## 5. Exit requirements

| Requirement | Check | Tol |
|---|---|---|
| v1 unchanged (R6) | `bundle_hash` diff; `git diff main -- <spine crates>` empty | byte-identical / exact |
| rigid fast path is the same physics (R2) | `rigid_fast_path_equals_world_cloud` | ≤1e-11 |
| scenes of many sources (R3, R4, R5) | `sources_superpose`, `mixed_scene_decomposes_exactly` | ≤1e-12 / ≤1e-10 |
| batch and export (R7) | `batch_is_ordered_reproducible_and_exportable` | exact |
| CI | fmt, clippy `-D warnings`, `cargo test --workspace` | green |

## 6. Traceability

R1 → `world` trait and its two unit tests · R2 → `rigid_fast_path_equals_world_cloud` · R3, R5 →
`sources_superpose`, `mixed_scene_decomposes_exactly` · R4 → `mixed_scene_decomposes_exactly` ·
R6 → `bundle_hash`, the spine diff, the v1 suite · R7 → `batch_is_ordered_reproducible_and_exportable`,
`npy_header_is_aligned_and_checked`.

## 7. Not in this milestone

`EvalBatch`/GPU encoding of non-rigid sources; per-source target channels `(S, T, D)`; Python exposure
of `Scenario::scene`. See `design/world.md` §10.
