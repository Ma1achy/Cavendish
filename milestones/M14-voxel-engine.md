# M14 — The voxel core (plan / implementation brief)

> **Status: planned — nothing here is built.** Design: `design/voxel.md`. Depends on: M11 (the seam),
> M2 (the voxeliser).
>
> **This is the centre of v2 and the next thing built.** M12's physics is *rebased onto it* (phases
> D–E) rather than extended beside it; the GPU work, the viewer and the scene replay then target the
> tree and nothing else. Milestone numbers are identifiers, not the build order — the order is in
> `MILESTONES.md` and `V2-HANDOFF.md` §4.
>
> **Goal.** One sparse hierarchical representation of mass and matter, built once and used
> everywhere: the voxeliser emits it, the gravity read walks it, the fluid coarsens on it, contact
> samples it, the viewer draws it. Fine bodies become affordable without giving up the reference
> standard, and the "octree" that the design conversations assumed stops being a deferred note.

---

## 1. Requirements

| ID | Requirement |
|---|---|
| M14-R1 | `voxel::VoxTree`: a linear octree from sorted Morton keys — leaf array, breadth-first nodes, child masks, value sidecars. Immutable topology; deterministic bytes for any thread count. |
| M14-R2 | Hierarchical voxeliser: `Solid::cover(aabb)` (default `Mixed`), top-down build that touches `O(surface)` cells. Expanded to finest cells it equals `shape::voxelise` **bit for bit, in raster order**; total mass exact, dipole exactly zero. |
| M14-R3 | Lossless merge: a node whose eight children are homogeneous and bitwise equal is one leaf. A merged body and its unmerged lattice of cubes give the same field to rounding. |
| M14-R4 | Cube series kernel: monopole + `l = 4, 6, 8` terms for a homogeneous cube, with a truncation bound; agrees with the closed-form prism wherever the bound says it should. |
| M14-R5 | Bounded tree evaluation: a walk that takes a phase tolerance `tol` [rad] and returns a phase whose error against the all-leaves sum is ≤ `tol`. `tol = 0` is the exact sum. No time-domain steps above `tol`. |
| M14-R6 | **One representation.** Every v2 body is a tree (`world::WorldSource` gains `tree()`); a rigid one is evaluated through the rigid socket against its static body-frame tree. The element law (`Point` / `Cube`) is a property of the body. A tree of unmerged `Point` leaves at `tol = 0` equals the spine's cloud sum to rounding. The v1 API (`Scenario::new`) is frozen on the spine path and its bundles stay byte-identical. |
| M14-R7 | Skinned bodies evaluate as one static tree per bone plus finest-level blend parcels; equal to the flat parcel sum within `tol`; mass conserved bit for bit. |
| M14-R8 | Hierarchical grid response: `GridField`'s `W` on a fixed cut of the room's tree; equal to the dense response within `tol`; the cut never changes during a run. |
| M14-R9 | Obstacle stamping from trees: volume fraction exact for merged leaves; immersed surface area within 5% of the true area at 4 cells radius (today: 8% low). |
| M14-R10 | Voxel contact: narrow-band distance sidecar, surface-sample contact generation, manifold reduction to ≤4 points with warm start. Box–box, box–sphere and arbitrary voxelised shapes collide; a settled stack is bit-constant. |
| M14-R12 | **M12 is rebased, not duplicated.** After phase E no production code in `physics` evaluates a flat parcel sum, builds a dense-only response, stamps bodies by point splat, or resolves contact only analytically. The old routines survive in `#[cfg(test)]` as oracles. Every M12 test still passes, retargeted where it named a removed path. |
| M14-R13 | **Adaptive fluids.** The air is simulated on a balanced (2:1) octree with MAC unknowns, a well-balanced buoyancy term and multigrid-preconditioned CG. Refinement is goal-oriented (tower response × density variation, plus flow resolution and solid proximity). Regridding conserves mass anomaly and puts no step in any signal above `tol`. A fully refined tree reproduces the uniform solver. |
| M14-R14 | **Liquids.** A free-surface liquid on the same solver: narrow-band level set, full-density gravity read, two-way coupling with rigid bodies. |
| M14-R15 | **Adaptive voxelisation.** A body's surface leaves are refined until their estimated phase error at closest approach to a tower is under `tol`. |
| M14-R16 | **Level of detail.** `Lod::{Exact, Bounded, Distance, Fixed}` on the one walk. Mass and centre of mass exact at every level; `Distance` blends across its band so the signal has no steps; its error is audited (M16), not assumed. Optional and off by default. |
| M14-R0 | **Measure first.** Three measurements on the existing code decide the weight of phases B–C before they are built (§2a). |
| M14-R11 | **It is faster, measured.** On a named fine body, the tree evaluation beats the flat sum in wall-clock at equal error by a stated factor; if it does not, the flat path stays the default and the brief says so. |

---

## 2. Phases

Each phase is one branch and one PR, leaves the workspace green, and has its own exit line.

### 2a. Phase 0 — measurements (days, on the code that exists)

| Measurement | How | What it decides |
|---|---|---|
| **Does body resolution reach the signal?** | the walker and a cuboid at 8, 4, 2, 1 cm pitch, passing 0.5–3 m from a tower; difference each from the finest | if the change is far under shot noise (250 µrad–1 mrad) at every range that occurs, fine bodies are for contact and the viewer, and phases B–C drop in priority behind D–E |
| **The differenced bound.** | derive the truncation bound on the upper-minus-lower, IFO-minus-IFO phase (one or two more powers of `Δ/r` than §2 of the design); test it is a bound on random nodes | whether a reference-grade walk opens to the leaves or stops early; and what `theta` is safe for `Lod::Distance` |
| **Quadrature convergence.** | anchors and a fast close pass against node count, Simpson vs Gauss–Legendre per half-flight | whether M16-R7's faster phase model is worth building — a saving that multiplies everything else |

Write the three results into `design/voxel.md` §2 and §13 before starting phase B.

| Phase | Delivers | Requirements | Exit |
|---|---|---|---|
| **A — the tree and the voxeliser** | `voxel` crate; `VoxTree`, keys, build, sidecars, moments; `Solid::cover` for every primitive and for meshes; top-down voxeliser | R1, R2 | `tree_expands_to_the_v1_cloud` bit-exact for every primitive and the M10 mesh fixtures; voxelising a 256-cell sphere is ≥10× faster than raster |
| **B — the lossless merge and the series** | merge pass; cube series with bound; implicit-child splitting; `TreeBody` | R3, R4, R6 | `merged_equals_unmerged` ≤1e-12 rel; series vs prism within its bound; v1 `bundle_hash` byte-identical |
| **C — bounded evaluation** | general moments to octupole; the tolerance walk; differential-first accumulation; the jitter test | R5, R11 | error ≤ `tol` for `tol` ∈ {1e-6, 1e-8, 1e-10}; no steps; the speed table in §5 filled in from a run |
| **D — rebase bodies and fields** | `Parcels` stores per-bone trees and a blend shell; `GridField` builds its response on the tree cut (`tol = 0` = today's dense `W`); flat sums move into tests | R7, R8, R12 | walker by trees = walker by flat sum within `tol`; hierarchical response = dense within `tol`; all M12 `body`, `grid` and `generate/tests/world` tests green |
| **E — rebase air coupling and contact** | tree stamping onto the air grid; the pyramid exposed for the solver; distance sidecar; voxel contact and manifold cache; analytic pairs demoted to oracle / primitive fast path | R9, R10, R12 | area test; `box_box_contact_is_refused` replaced by `box_rests_on_box_exactly`; stack of three sleeps bit-constant; all M12 `scene`, `rigid` and `fluid` tests green |
| **G — adaptive air** | balanced octree grid, level transfers, well-balanced buoyancy, flux-form Poisson with multigrid-PCG, goal-oriented regridding; adaptive voxelisation of bodies | R13, R15 | the seven checks of `design/voxel.md` §9.5; cells and wall-clock against the uniform run at equal phase error, recorded |
| **H — liquids** | level set in a narrow band, full-density read, two-way coupling | R14 | a tank at rest stays at rest; a dam break matches the standard front-position curve within 5%; a floating box sits at its Archimedes draught within one cell; sloshing in a driven tank conserves mass to rounding |
| **F — device** | tree upload, WGSL walk, float-float accumulation; merged into the GPU steps of the handoff | (handoff steps 3–4) | CPU≡GPU ≤1e-9 with float-float on both real devices |

A–C are the engine. D and E are the rebase: they are what make it *the* representation, and they are
not optional — stopping after C would leave the tree as a fourth representation beside M12's three.
F is not a separate effort: it is how the handoff's GPU steps are done once A–E exist. M12's tests
are the safety net throughout: they assert physics (Nusselt numbers, conservation, exact rest), not
storage, so they should pass unchanged across the rebase.

---

## 3. Algorithms

**Keys.** `key(i, j, k)` interleaves three ≤10-bit integers (`u32`); a node at level `ℓ` is the key
with its low `3ℓ` bits cleared, plus `ℓ`.

**Build (bottom-up, no recursion).**

```
keys ← sort(unique(leaf keys))                    # canonical order
for ℓ in 1..=depth:
    parents ← unique(keys_at(ℓ−1) >> 3)           # compaction
    child_mask[p] |= 1 << (child & 7)             # for each child of p
    first_child[p] ← lower_bound(keys_at(ℓ−1), p << 3)
merge: a parent with mask 0xFF whose children are leaves with UNIFORM and bitwise-equal density
       becomes a leaf at level ℓ; repeat upward
moments: leaves → parents, f64, in level order
```

**Voxelise (top-down).**

```
visit(node): match solid.cover(node.aabb)
    Outside → return
    Inside  → emit uniform leaf(node)
    Mixed   → if node.level == 0 { emit leaf with k³-supersampled mass } else { for c in children: visit(c) }
then renormalise Σm = M exactly; shift origin so the dipole is exactly zero
```

**Walk (per query pair `p_u`, `p_l`).**

```
stack ← [root]
while node ← pop:
    b ← bound(node, min(|p_u − com|, |p_l − com|))
    if b ≤ tol·mass(node)/M_total:  acc += kernel(node, p_u) − kernel(node, p_l)      # series or multipole
    else if node is a finest leaf:   acc += exact(node, p_u) − exact(node, p_l)        # point, or prism if near
    else:                            push children (stored, or implicit for UNIFORM)
```

---

## 4. Tests

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `morton_roundtrip_and_order` | encode/decode; sort order is Z-order | exact |
| unit | `build_is_deterministic` | same keys, shuffled input, 1 vs N threads → identical bytes | exact |
| unit | `sidecar_indexing_by_popcount` | every leaf reachable from the root at its ordinal | exact |
| unit | `tree_expands_to_the_v1_cloud` | `to_cloud()` = `shape::voxelise` for each primitive and mesh fixture, incl. element order | **bit-exact** |
| unit | `cover_is_conservative` | random boxes: `Inside` ⇒ all samples inside; `Outside` ⇒ none | exact |
| unit | `mass_and_dipole_postconditions` | `Σm = M`; dipole zero | exact / ≤1e-15 |
| unit | `merge_counts_match_the_design_table` | sphere 64/128, slab 1 cm: leaf counts as `design/voxel.md` §5.1 | exact |
| unit | `merged_equals_unmerged` | field of the merged tree vs the same body as finest cubes (prism kernel), near and far | ≤1e-12 rel |
| unit | `cube_series_matches_prism` | series vs closed form over `s/R` ∈ [0.02, 0.3], on and off axes | within the stated bound |
| unit | `cube_series_bound_is_a_bound` | actual error ≤ bound everywhere sampled | — |
| unit | `point_tree_equals_the_spine_sum` | unmerged tree, `Element::Point`, `tol = 0` vs `gravity::Cloud` through the rigid socket, tumbling body | ≤1e-12 rel |
| unit | `no_production_flat_paths` | a CI check: the flat parcel sum, dense-only response and point-splat stamp are referenced only from tests | structural |
| unit | `tree_vs_point_lattice_differs_as_h4` | tree body vs v1 point cloud: difference scales as `(h/r)⁴` with coefficient 0.015 on axis | 10% |
| unit | `moments_match_direct_sums` | node mass, CoM, quadrupole, octupole vs summing its leaves | ≤1e-13 rel |
| unit | `walk_error_is_within_tol` | phase by walk vs all-leaves sum, for three tolerances, three ranges, tilted tower | ≤ `tol` |
| unit | `walk_with_zero_tol_is_the_exact_sum` | identical to the flat sum up to summation order | ≤1e-13 rel |
| unit | `no_steps_in_time` | body translating at 1 mm per sample past a tower: second difference of the phase has no jump above `tol` | ≤ `tol` |
| unit | `lod_conserves_mass_and_com` | any cut: total mass and centre of mass equal the leaves' | ≤1e-14 rel |
| unit | `lod_distance_has_no_steps` | body translating past a tower under `Distance`: first and second differences of the phase show no jump beyond the smooth trend | ≤1e-9 rad |
| unit | `lod_fixed_cut_never_changes` | static source: identical accepted node set at every measurement | exact |
| unit | `lod_exact_equals_tol_zero` | `Exact` ≡ `Bounded{0}` ≡ the flat sum | ≤1e-13 rel |
| unit | `bone_trees_equal_parcels` | walker: per-bone trees + blend parcels vs flat parcel sum, several poses | ≤ `tol` |
| unit | `bone_partition_conserves_mass` | Σ over bone trees and blend parcels = rest mass | **exact** |
| unit | `hierarchical_response_equals_dense` | `W` on the cut vs dense `W`, random mass frames at the plume's amplitude | ≤ `tol` |
| unit | `response_cut_is_fixed` | the cut built for a tower is identical across measurements | exact |
| unit | `tree_stamp_volume_and_area` | sphere of radius 4 and 8 cells: volume fraction; `∫|∇φ|` vs `4πr²` | ≤1e-12 / 5% |
| unit | `distance_sidecar_matches_analytic` | sphere and box: narrow-band distance and normal | ≤0.5 h / 5° |
| unit | `box_rests_on_box_exactly` | pose bit-constant after sleep; rests face to face | **bit-constant** / 2 mm |
| unit | `stack_of_three_sleeps` | three boxes settle and sleep; no drift over 10 s | **bit-constant** |
| unit | `voxel_contact_matches_analytic_pairs` | sphere–plane, sphere–sphere, sphere–box impulses vs M12's analytic contacts | 2% |
| integration | `tree_body_through_generate` | a `TreeBody` scene through `generate::run` vs the same body as a v1 source | `(h/r)⁴` tolerance |
| integration | `fine_body_speed` | wall-clock, tree vs flat, at equal error — recorded, and asserted ≥ the factor in §5 | — |
| e2e | `bundle_hash` | 24 v1 scenes | **byte-identical** |
| e2e | `mixed_scene_decomposes_exactly` (extended) | tree body + articulated tree figure + hierarchical air: channels sum to the signal | ≤1e-10 |

---

## 5. Exit requirements

| Requirement | Check | Tol |
|---|---|---|
| the tree is the voxeliser's output (R1, R2) | `tree_expands_to_the_v1_cloud`, `build_is_deterministic` | bit-exact |
| the merge loses nothing (R3) | `merged_equals_unmerged` | ≤1e-12 rel |
| approximation is bounded and stated (R4, R5) | `cube_series_bound_is_a_bound`, `walk_error_is_within_tol`, `no_steps_in_time` | ≤ `tol` |
| v1 unchanged, and the tree agrees with it (R6) | `bundle_hash`; spine numerics' diff empty; `point_tree_equals_the_spine_sum` | byte-identical / ≤1e-12 |
| one representation (R12) | `no_production_flat_paths`; the full M12 suite green after phases D and E | — |
| articulated and air use it (R7, R8, R9) | `bone_trees_equal_parcels`, `hierarchical_response_equals_dense`, `tree_stamp_volume_and_area` | ≤ `tol` / 5% |
| any shape collides, and rests (R10) | `box_rests_on_box_exactly`, `stack_of_three_sleeps` | bit-constant |
| **it pays (R11)** | `fine_body_speed` on the M3, CPU | see below |

**The speed table — to be filled from a run, not from this document:**

| body | finest cells | leaves | flat sum [ms/measurement] | tree, `tol`=1e-10 | tree, `tol`=1e-6 |
|---|---|---|---|---|---|
| sphere, 128 cells across, 2 m from the nearest tower | 1 143 368 | 138 504 | — | — | — |
| walker at 1 cm pitch | — | — | — | — | — |
| 4 × 3 × 0.2 m wall at 2 cm, 4 m away | — | — | — | — | — |

Pass: ≥4× at `tol` = 1e-10 on the sphere and ≥10× at `tol` = 1e-6 on the wall. If the first fails,
the merge ships as a memory optimisation only and the default evaluation stays flat; record why.

## 6. Traceability

R1 → `morton_*`, `build_is_deterministic`, `sidecar_*` · R2 → `tree_expands_to_the_v1_cloud`,
`cover_is_conservative`, `mass_and_dipole_postconditions` · R3 → `merge_counts_*`,
`merged_equals_unmerged` · R4 → `cube_series_*` · R5 → `walk_*`, `no_steps_in_time`,
`moments_match_direct_sums` · R6 → `point_tree_equals_the_spine_sum`, `tree_body_through_generate`, `tree_vs_point_lattice_differs_as_h4`,
`bundle_hash` · R7 → `bone_*` · R8 → `hierarchical_response_*`, `response_cut_is_fixed` · R9 →
`tree_stamp_volume_and_area` · R10 → `distance_sidecar_*`, `box_rests_on_box_exactly`,
`stack_of_three_sleeps`, `voxel_contact_matches_analytic_pairs` · R16 → `lod_*` · R0 → the three results recorded in `design/voxel.md` · R11 → `fine_body_speed` · R12 → `no_production_flat_paths`, the M12 suite.

## 7. Not in this milestone

A world-level broad-phase tree; destruction or topology change of a body at run time; a
tighter bound on the differenced phase (an open question the brief does not depend on); 4³ re-packing.

## 8. Risks, stated

- **Adaptive incompressible flow is the hardest thing in this plan.** Its characteristic failure —
  spurious currents at level jumps under stratification — would appear as spurious gravity-gradient
  noise, i.e. in the signal. The defences are the well-balanced buoyancy term, the fully-refined ≡
  uniform test, the at-rest test on a graded grid, and the no-step-on-regrid test. Phase G is not
  done until all four pass; the uniform solver remains available as a setting.
- **Phase order within fluids:** the uniform GPU solver comes first (it is the oracle and the
  fully-refined case), then G, then H.

- **The rebase touches working code.** M12 is implemented and tested on the branch; phases D–E rework
  how its bodies, response, stamping and contact store and read matter. The mitigation is that M12's
  tests check physics rather than storage and the old routines stay as in-test oracles — but it is
  real rework, roughly `body.rs`, `grid.rs`, `scene.rs` and the contact half of `rigid.rs`.

- **The merge may not pay at the pitches Gradar scenes use.** At 5–8 cm the leaf ratio is under 2.
  The value is in making fine bodies affordable; if scenes stay coarse, phases B–C are worth less
  than D–E.
- **The bound is pessimistic** (it ignores arm cancellation), so reference-grade walks will open
  more than they need. The fix is derivation, not tuning.
- **Voxel contact is rougher than analytic contact** for primitives: normals come from a sampled
  distance field. M12's analytic pairs stay as the fast path where both shapes are primitives, and
  as the oracle for the voxel path.
