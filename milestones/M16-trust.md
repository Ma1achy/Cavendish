# M16 — Trust: oracles, an error ledger, reproducible datasets (plan / implementation brief)

> **Status: planned — nothing here is built.** Depends on: M11, M12. Independent of M14 and can start
> now; its ledger gains entries as M14 lands. Parts of it change the data contract (`Meta`), so the
> spec is edited first.
>
> **Goal.** Every number Cavendish emits comes with a stated error, an independent check behind the
> path that produced it, and a recipe that regenerates it. The forward-model spine already has this
> (George's anchors). The v2 paths — fluid to phase, trees, level of detail, the GPU — do not.

---

## 1. Requirements

| ID | Requirement |
|---|---|
| M16-R1 | **An oracle for the fluid-to-phase path.** Density fields with answers known another way, pushed through `GridField` and the response, compared with that answer. |
| M16-R2 | **An error ledger in every bundle.** `Meta` gains a `numerics` block (what settings produced this) and the bundle an optional `error_estimate (D,)` in radians (what they cost), measured by re-running at tighter settings. |
| M16-R3 | **Metamorphic tests** over randomly generated scenes: properties that must hold whatever the scene is. |
| M16-R4 | **Tests that did not run are failures.** CI asserts the number of tests executed against a recorded floor, per job. |
| M16-R5 | **Dataset manifests and verification.** A dataset directory carries everything needed to regenerate it, and a command that proves it. |
| M16-R6 | **Real-device CI and benchmark gates.** The Metal and Vulkan paths run on real hardware; performance regressions fail a build. |
| M16-R7 | **A faster flight quadrature, if the measurement supports it** — a new `PhaseModel` beside the reference one, never replacing it. |

## 2. Design

### 2.1 Oracles for fluid → phase (R1)

| Case | The independent answer | What it exercises |
|---|---|---|
| **a ball of anomaly moving at constant velocity** — a uniform sphere of `δρ` rasterised onto the grid, frame by frame | a uniform sphere is a point mass outside itself: the same scene as a v1 point source through the validated spine | frames, time interpolation, the response `W`, the flight window — against George-anchored code. Converges as `h²`; assert the rate, and ≤1e-4 relative at the finest grid |
| **a static slab layer** — one horizontal layer of cells at `δρ` | the same layer as a single closed-form prism (one Nagy evaluation, not a sum of cells) through direct quadrature | partition invariance of the cell kernel and the response sum; ≤1e-10 |
| **a layer that switches on linearly in time** | the static answer times the time-weighting integral, in closed form | the `[t − 2T, t]` window and frame interpolation; ≤1e-10 |
| **a plane density wave** in a large box | the infinite-medium potential `−4πG·A/k² · cos(k·x − ωt)`; the box truncation is an error term that must fall as the box grows | trend only (monotone convergence), not a tolerance |

The first row is the important one: it routes a v2 path through the v1 oracle.

### 2.2 The ledger (R2)

```rust
pub struct Numerics {                 // in Meta — what produced this bundle
    pub engine: String,               // git SHA, dirty flag
    pub precision: Precision,         // F64 | F32 | FloatFloat
    pub phase_model: PhaseModelKind,  // and its quadrature rule and node count
    pub tree_tol: f64,                // rad; 0 = exact
    pub lod: LodMode,                 // design/voxel.md §6.1
    pub pitch: Vec<f64>,              // per source, finest
    pub fluid: Option<FluidNumerics>, // levels, solver tol, regrid tol, step
}
```

`error_estimate` is **measured, not derived**: `generate::audit(fraction)` re-runs a random fraction
of a batch at tighter settings (halved pitch, doubled quadrature nodes, `tol = 0`, `f64`) and records
the per-tower difference, split by cause. A dataset's manifest carries the audit's distribution
(median, 99th percentile, worst) next to the shot-noise level, so "is the numerical error under the
noise?" is a line in a file, not a belief.

### 2.3 Metamorphic properties (R3)

For scenes drawn from a generator (random sources, array, schedule), each to ≤1e-10 unless noted:

- **superposition** — the scene equals the sum of its sources run alone;
- **mass scaling** — every mass ×λ scales every target/ambient channel by λ;
- **time shift** — shifting all source tracks and the schedule by `Δt` leaves the signal unchanged;
- **rigid motion of the world** — rotating everything (sources and array) about the vertical, or
  translating it, leaves the signal unchanged; a rotation that tilts the towers does not, and the
  test asserts that too;
- **tower permutation** — permuting towers permutes columns;
- **refinement** — halving the pitch changes the signal by no more than the ledger's estimate;
- **decomposition** — channels sum to the signal (already tested; now over random scenes);
- **batch invariance** — any batch order, thread count, or chunking gives identical bytes.

### 2.4 Tests that ran (R4)

Each CI job writes its executed-test count; a checked-in `ci/test-floor.toml` holds the minimum per
job (workspace, GPU-marked, Python). Fewer than the floor fails the job. Raising the floor is part of
any PR that adds tests. This closes the recurring "green because it did not run" trap by
construction.

### 2.5 Manifests (R5)

```
dataset/
  manifest.json     spec (verbatim) and its hash · engine SHA · root seed · count · field set
                    numerics block · audit summary · platform and adapter · array shapes and dtypes
  *.npy
```

`cavendish verify <dir> [--n 32]` regenerates `n` random indices from the manifest alone and compares:
bitwise on the `f64` CPU path, within the manifest's stated tolerance on a GPU path. A dataset that
does not verify is not used for training. Depends on M15's `SceneSpec` for the "spec, verbatim" line;
before M15, the manifest names the builder function and its arguments.

### 2.6 Devices and benchmarks (R6)

- Self-hosted runners: the M3 (Metal) and the 2080 machine (Vulkan). The GPU-marked tests and the
  float-float parity tests run there on every PR that touches `compute`, `voxel` or `physics`.
- `criterion` benchmarks for the kernels that set throughput (cloud sum, tree walk, response dot
  product, fluid step, voxeliser); baselines stored per runner; a >10% regression fails the job.
- Nightly: the golden-scene hashes (`bundle_hash`) on all three platforms.

### 2.7 The quadrature (R7)

Cost is elements × flight nodes, so halving the nodes is worth as much as halving the elements.
Today: composite Simpson at `fine_dt` = 0.01 s over the two halves of the flight — 147 nodes. The
integrand is smooth on the scale of (range to the source) / (source speed), typically longer than a
half-flight, which suggests Gauss–Legendre on each half would converge with far fewer nodes.

That is a hypothesis. M14's measurement phase tests it (convergence of the anchors and of a close,
fast pass against node count). If it holds, `GaussFlight { n }` is added as a third `PhaseModelKind`,
checked against `PropagationIntegral` on every anchor to ≤1e-11, with `n` chosen per scene by a
doubling test and recorded in the ledger. The reference model is not edited and stays the default
for anything used as an oracle.

## 3. Tests

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `moving_ball_matches_point_source` | rasterised ball through `GridField` vs v1 point source; error falls as `h²` | rate 1.8–2.2; ≤1e-4 rel at finest |
| unit | `slab_layer_matches_single_prism` | layer of cells vs one prism | ≤1e-10 |
| unit | `ramped_layer_matches_closed_form` | time-weighting of the flight window | ≤1e-10 |
| unit | `plane_wave_converges_with_box` | error falls monotonically as the box doubles | trend |
| unit | `numerics_block_roundtrips` | every setting that affects a number appears in `Meta` and survives serialisation | exact |
| unit | `audit_estimate_bounds_the_refinement` | the recorded estimate ≥ the actual change on a further refinement, for 95% of samples | — |
| property | the eight properties of §2.3 | over 200 random scenes each | ≤1e-10 / exact |
| integration | `manifest_regenerates` | `verify` passes on a fresh dataset; fails when one byte, the seed, or the engine SHA is altered | exact |
| integration | `gauss_flight_matches_reference` | (if built) every anchor and a fast close pass | ≤1e-11 |
| CI | `test_floor` | executed counts ≥ floor in every job | exact |

## 4. Exit requirements

| Requirement | Check |
|---|---|
| a v2 path is tied to the v1 oracle (R1) | `moving_ball_matches_point_source` |
| every bundle states its numerics and its error (R2) | `numerics_block_roundtrips`, `audit_estimate_bounds_the_refinement` |
| invariants hold on scenes nobody wrote by hand (R3) | the property suite, green |
| nothing passes by not running (R4) | `test_floor` in all jobs |
| datasets regenerate (R5) | `manifest_regenerates` |
| real devices, and no silent slowdowns (R6) | both runners green on a PR; one deliberate slowdown caught |
| docs agree | spec (`Meta`, the bundle table), `design/{state,generate,sdk,compute}.md` in the same PR |

## 5. Open questions

- Where `error_estimate` lives when the audit covers only a fraction: per-bundle (present on audited
  indices only) or per-dataset (in the manifest only). The brief assumes both.
- Whether the audit's "tighter settings" for the fluid can be afforded at all (a halved fluid grid is
  16× the work); it may have to be a one-off convergence study per scene family, recorded as such.
