# Cavendish — `voxel` drill-down

> Subsystem design for the **voxel core**: one sparse hierarchical representation of where mass and
> matter are, used by the voxeliser, the gravity read, the fluid, contact and the viewer. It replaces
> the three unconnected representations M12 left behind (a flat point `Cloud` for bodies, a dense
> `Grid3` for air, analytic primitives for contact). Companion to `design/physics.md` (what it
> serves), `design/shape.md` (the voxeliser it rebuilds), `design/world.md` (the seam it sits
> behind) and `design/compute.md` (the GPU path it is laid out for).
>
> **Status: planned — nothing here is built.** Brief: `milestones/M14-voxel-engine.md`.
>
> **Position: this is the centre of v2, not an optimisation added to it** (§0). It is built first,
> and M12's physics, the GPU work, the viewer and the scene spec are built on it.
>
> **Dependencies (planned):** `voxel → gravity, math` (L1, beside `gravity`); `shape → voxel`;
> `world → voxel`; `physics → voxel`; `compute → voxel` (device encoding); `viewer → voxel`. The
> forward-model spine's numerics (`gravity`, `instrument`, `reference`) are not edited, and the v1
> point `Cloud` path stays as the oracle.

---

## 0. Central, not optional

M12 was built without a shared representation, so it has three: a flat point `Cloud` for bodies, a
dense `Grid3` for air, analytic primitives for contact. Adding a tree as a fourth, opt-in path would
be the monkey-patch the project decided against. The rule instead:

> **In v2, matter is a `VoxTree` with sidecars. Every v2 consumer reads the tree. There is one
> evaluation path, and `tol = 0` makes it exact.**

| Today (M12, as built) | With the voxel core | What happens to the old code |
|---|---|---|
| `shape::voxelise` → flat `Cloud` | the voxeliser emits a `VoxTree`; `to_cloud()` is a derived view | the raster voxeliser stays as the bit-exact oracle in tests |
| `Parcels` evaluates a flat parcel sum | bodies are trees: one per rigid body, one per bone, joint parcels as finest leaves | the maps (rigid / skinned / displaced), skeletons and mass-conservation logic are kept; the flat sum moves into tests as the oracle |
| `GridField` with a dense response `W` | the response lives on a fixed cut of the room's tree; `tol = 0` is the dense `W` | same code path; the dense case is a setting, not a second implementation |
| bodies stamped onto the air by point splats | stamped from tree leaves (exact volume fraction, geometric area) | `Grid3::splat` stays for genuinely point-like deposits |
| fluid on a bare dense grid, MIC(0)-PCG | the same dense finest level, viewed as the bottom of a pyramid that the multigrid solve uses | discretisation, advection and the equation of state unchanged |
| analytic contact for a few shape pairs; box–box refused | contact from the distance sidecar for any pair | analytic pairs kept as the oracle and as a fast path for primitive pairs |
| viewer replay stores per-frame point positions | replay stores trees once plus per-frame poses / bone transforms | — |
| GPU: nothing | device encoding of every source is the tree | — |

**What is not replaced: the v1 spine.** `gravity::Cloud` and the rigid socket are the validated
forward model. They cannot be removed without editing the spine, and they should not be: they are the
oracle. Their role becomes exactly two things — (1) the frozen v1 API (`Scenario::new`), whose
bundles stay byte-identical, and (2) the reference every tree evaluation is tested against (a tree
of unmerged point leaves at `tol = 0` must equal the spine's sum to rounding). No new feature is
built on that path. This is one production representation plus an oracle, not two backends.

**What centrality does not change:** the numbers in §2 and §5.1. At coarse pitch the merge gains
little. The case for making the tree central is that contact, stamping, the viewer, the device
encoding and the scene replay each need *a* hierarchical voxel structure, and building one is cheaper
and safer than building four.

**Element law.** A leaf is evaluated as a point or as a homogeneous cube; that is a property of the
body (`Element::Point | Element::Cube`), not of the engine. `Point` is what v1 does and is what the
bridge test uses; `Cube` is the default for new v2 bodies because it is the more accurate reading of
a voxel (§5.3).

---

## 1. What is borrowed, and from where

Voxel engines and tree codes are mature. Nothing below is invented here; the work is choosing the
parts that survive Cavendish's accuracy standard (§2).

| Technique | Source | Taken | Not taken, and why |
|---|---|---|---|
| Linear octree from sorted Morton keys; leaf array + breadth-first internal nodes; rebuild instead of edit | Bonsai (Bédorf et al. 2012), Cornerstone (Keller et al. 2023) | the whole build: key → radix sort → levels by masking → link by binary search. One algorithm for CPU and WGSL | Hilbert keys (better locality across ranks; irrelevant on one device) |
| Child bitmask + popcount indexing into packed child arrays | ESVO; sparse 64-trees | 8-bit child masks, packed children | 4³ nodes: fewer levels and faster ray-marching, but a node collapses only when all 64 children agree, which costs the lossless merge (§5) its yield. The key format makes a later re-pack to 4³ a storage change only |
| Topology separate from values; values in flat "sidecar" arrays indexed by active-voxel ordinal | NanoVDB index grids, fVDB | exactly this: one topology, many sidecars (mass, density, temperature, SDF, colour) | VDB's fixed 5-4-3 depth: built for 10⁹-voxel film volumes; our bodies are 10⁴–10⁷ |
| Bulk-synchronous topology change: mark, prefix-sum, allocate once, rebuild | NanoVDB dynamic grids (Sifakis et al. 2026) | the model — no pointer edits, ever | their operators (dilation, pruning); not needed until adaptive air (§9) |
| Multipole acceptance with an explicit error bound, not a bare opening angle | Barnes–Hut 1986; Salmon & Warren 1994; Dehnen 2002 | bound-driven opening (§6) | fixed θ ≈ 0.5–0.75: gives 10⁻³ force error, six orders short of what is needed here |
| Pyramid of uniform levels for multigrid and 2:1 adaptive grids | SPGrid (Setaluri et al. 2014); Losasso et al. 2004 | the pyramid as multigrid levels for the pressure solve (§9) | adaptive refinement of the air: planned last, gated on measurement |
| Any shape collides with any shape by testing one body's surface voxels against the other's occupancy / distance field | voxel game engines; SDF contact generation | voxel contact with manifold reduction (§10), replacing the refused box–box case | per-voxel destruction, fracture |
| DDA / hierarchical ray-marching of the same structure that is simulated | ESVO, 64-tree marching | the viewer draws the tree it is given (§11) | DAG compression: rendering-only, and bodies here carry per-voxel mass |

## 2. The accuracy gap — why this is not a game engine with the constants changed

The forward model is held to ≤10⁻⁹–10⁻¹¹ rad on the anchors. Shot noise is 10⁻³ rad. A 70 kg body 2 m
from a tower contributes a raw (undifferenced) potential phase of

```
(m_A/ħ) · (GM/r) · 2T  =  1.37e9 s/m² · 2.3e-9 m²/s² · 1.46 s  ≈  4.7 rad    per arm.
```

A multipole expansion of order `p` for a node of radius `a` at distance `r` has potential error
bounded by `(GM/r)·θ^(p+1)/(1−θ)`, `θ = a/r`. Summed over nodes the masses add to `M`, so the phase
error bound is `≈ 4 · 4.7 rad · θ^(p+1)` (four arm evaluations). The opening angle that meets a phase
tolerance:

| tolerance | quadrupole `p=2` | octupole `p=3` | `p=5` | `p=7` |
|---|---|---|---|---|
| 10⁻¹⁰ rad (reference-grade) | 0.0002 | 0.0015 | 0.013 | 0.039 |
| 10⁻⁶ rad (10³× under shot noise) | 0.004 | 0.015 | 0.061 | 0.12 |

Astrophysical tree codes run at θ = 0.5–0.75. At reference grade, θ = 0.0015 means a node 2 m away
may be 3 mm across — smaller than a voxel. **A conventional Barnes–Hut far field buys nothing for a
body near the array at the reference standard.** That is the honest reading of the numbers, and it
shapes the design:

1. The main gain is a **lossless** merge (§5), which has no θ.
2. Approximation is **bound-driven and explicit** (§6): the caller states a phase tolerance; the walk
   opens whatever it must. At reference grade it degenerates gracefully toward the leaves; for
   distant mass (walls, far air, other rooms) and for training data at a stated tolerance it saves a
   great deal.
3. The uniform cube has a special, cheap, high-order series (§5.2), so merged interior cells reach
   reference grade at far larger θ than a general node can.

## 3. `VoxTree` — the one structure

```rust
pub struct VoxTree {
    pub origin: [f64; 3],        // world or body frame corner of the root cell
    pub h: f64,                  // finest pitch; the root is h·2^depth across
    pub depth: u8,               // ≤ 10 per axis (30-bit keys; 21-bit/axis u64 on the CPU)
    pub leaves: Vec<Leaf>,       // sorted by Morton key; variable size — THE evaluation list
    pub nodes: Vec<Node>,        // internal nodes, breadth-first, children contiguous
    pub level_offsets: Vec<u32>,
}
pub struct Leaf { pub key: u32, pub level: u8 }                  // a cube of side h·2^level
pub struct Node { pub child_mask: u8, pub first_child: u32, pub flags: u8 /* UNIFORM, LEAF_CHILDREN */ }

pub struct Sidecar<T> { pub values: Vec<T> }                     // indexed by leaf ordinal (or node ordinal)
pub struct Moments { pub mass: f64, pub com: [f64; 3], pub radius: f64, pub quad: [f64; 6], pub oct: [f64; 10] }
```

- **Topology is immutable.** A change of shape is a rebuild from keys. For a body that is voxelised
  once at rest this never happens at run time; for the stamp of moving bodies onto the air grid it is
  a mark-and-compact per step.
- **Values are sidecars.** Mass and density for the gravity read; temperature and velocity for the
  air; a narrow-band signed distance for contact; colour for the viewer. Different consumers share one
  topology and never each other's arrays.
- **Build.** Keys from integer cell coordinates → sort → per level, mask keys to that level and
  compact to unique → link children by binary search. Linear in the leaf count, no recursion, no
  pointers; the same passes are compute shaders on the GPU. WGSL has no 64-bit integers: keys are
  30-bit (`depth ≤ 10`, 1024³ finest cells) in one `u32` on the device; a larger tree is a grid of
  trees under a top-level table.
- **Determinism.** Morton order is canonical; summation order is fixed by it. Same inputs → identical
  bytes, any thread count.
- **Dense is a degenerate tree.** A fully occupied level needs no masks: the air keeps its plain
  `Grid3` arithmetic and is *viewed* as the finest level of a tree when something hierarchical is
  wanted (§8, §9).

## 4. The hierarchical voxeliser

Today `shape::voxelise` classifies every cell of the bounding box: `O(volume)`. With one addition to
the `Solid` seam it becomes `O(surface)` and emits a tree directly:

```rust
pub enum Cover { Inside, Outside, Mixed }
pub trait Solid {
    fn occupancy(&self, p: [f64; 3]) -> f64;
    fn bbox(&self) -> Aabb;
    /// Conservative classification of a box. `Mixed` is always a correct answer.
    fn cover(&self, b: &Aabb) -> Cover { Cover::Mixed }
}
```

Descend from the root: `Outside` → drop; `Inside` → emit one uniform leaf at this level; `Mixed` →
split, down to the finest level, where the existing `k³` supersample gives the fractional mass.
Primitives answer `cover` exactly; a mesh answers from its acceleration structure (a box that
intersects no triangle is wholly in or out — one winding-number query decides which); the default
(`Mixed`) reproduces today's behaviour cell for cell.

The two exact post-conditions carry over unchanged: masses renormalised so `Σm = M` exactly, and the
cloud recentred so the dipole is exactly zero (the recentring moves `origin`, not the keys).
`VoxTree::to_cloud()` expands every leaf to finest cells and must reproduce `shape::voxelise`'s
`Cloud` **bit for bit, in raster order** — the test that the new voxeliser is the old one.

## 5. The lossless merge

### 5.1 Partition invariance

Eight homogeneous cubes of side `h` and equal density are one homogeneous cube of side `2h`. Evaluated
with the cube kernel the two are the same field — not approximately. So every `Inside` node is stored
as a single leaf, and the number of elements a field evaluation touches falls from the body's volume
to roughly its surface. Counted on the voxeliser's own cell classification:

| body | finest cells | leaves after merge | ratio |
|---|---|---|---|
| sphere, 32 cells across | 19 952 | 7 968 | 2.5 |
| sphere, 64 | 148 640 | 33 504 | 4.4 |
| sphere, 128 | 1 143 368 | 138 504 | 8.3 |
| sphere, 256 | 8 965 552 | 558 776 | 16 |
| 0.45 × 0.25 × 1.75 m slab, h = 2 cm | 28 336 | 9 086 | 3.1 |
| same, h = 1 cm | 210 496 | 33 543 | 6.3 |
| same, h = 5 mm | 1 628 991 | 179 179 | 9.1 |

The ratio grows as `1/h`, as it must (volume over surface). **At the pitches used today (5–8 cm, a few
hundred to a few thousand parcels) it is under 2 and not worth having.** It matters when bodies are
voxelised finely — which is what a resolved surface near a tower needs.

### 5.2 The catch: the kernel, and the cube series

A merged leaf is a large cube, and a large cube near a tower is not a point. The closed-form prism
kernel (`physics::kernel`) is exact but costs tens of point-kernel evaluations. Counting elements is
therefore not the measure; **wall-clock at equal error is**, and the brief makes that the exit test.

What makes the merge pay is that a homogeneous cube has no dipole, no quadrupole and no octupole. Its
exterior potential, for mass `M`, side `s`, at offset `(x, y, z)`, `R = |·|`, is

```
V = −GM/R  +  (7GM s⁴/960) · (5(x⁴ + y⁴ + z⁴) − 3R⁴) / R⁹  +  O(GM s⁶/R⁷)
```

(from `⟨x⁴⟩ = s⁴/80`, `⟨x²y²⟩ = s⁴/144`; checked against 60³ quadrature: the residual after the
hexadecapole term is 10⁻⁷ relative at `R = 3.2 s`). The correction is `0.015 (s/R)⁴` on axis. With
the `l = 6` and `l = 8` terms the remainder scales as `(s/R)¹⁰`, which by the size of the leading
coefficient should put the series near 10⁻¹² relative at `s/R ≈ 0.1` for the cost of a few point
kernels — an estimate from the scaling, to be measured in phase B, not a result. So a merged leaf is evaluated by

- the **series** when its truncation bound is under the tolerance,
- **splitting into its eight implicit children** otherwise (exact, by §5.1 — no storage, no kernel
  change), recursing,
- the **closed-form prism** only at the finest level when a query point is within a few cells — the
  case the kernel was written for.

### 5.3 What the merge is not

- It is not the v1 point lattice. v1 treats each finest cell as a point, an error of `0.015 (h/r)⁴`
  per cell (10⁻⁷ relative at `h` = 5 cm, `r` = 1 m). A tree body is *more* accurate than the cloud it
  came from, so the two do not agree to rounding and must not be asserted to. The tree is checked
  against a uniform lattice of cubes (rounding) and against v1 with an `(h/r)⁴` tolerance.
- So the **element law is explicit** (§0). With `Element::Point` and no merge, a tree equals the
  spine's sum to rounding — the bridge test. With `Element::Cube` (the v2 default) it differs from
  the point lattice by the `(h/r)⁴` term, by design. The v1 API does not go through the tree at all,
  so v1 bundles stay byte-identical.
- It does not apply to heterogeneous cells: boundary cells with fractional mass, and bodies with
  per-voxel density, stay at the finest level unless eight siblings are bitwise equal.

## 6. Bounded tree evaluation

One walk serves every consumer. For a query point and a node:

```
bound = (m_A/ħ)·2T · G·mass · θ^(p+1)/(1−θ) / r          θ = radius/r,  r = distance to the node's CoM
if node is UNIFORM:   accept by cube series if its bound ≤ share(tol), else descend (implicit children)
else:                 accept by multipole (order p) if bound ≤ share(tol), else descend; leaves are exact
```

- **`tol` is a phase, in radians, per source per measurement** — the unit the rest of the project
  thinks in. `share(tol)` apportions it by mass so the node bounds sum to `tol`. Default
  `tol = 1e-10`; `tol = 0` means every leaf, exactly.
- **No jitter.** An opening decision that flips as a body moves changes the result by at most the
  bound. `design/physics.md` §1 calls anything that makes the mass field flicker a defect; the rule
  here is that `tol` must sit below the reader's noise floor by a stated margin, and that margin is
  tested: the phase of a slowly translating body is differenced in time and its second difference
  must show no steps above `tol`.
- **Differential-first.** The walk evaluates the upper and lower arm points together against the same
  node and accumulates the difference, as the rigid kernel does, so the cancelling monopole never
  enters a sum.
- **Moments** (mass, CoM, quadrupole, octupole) are computed bottom-up in `f64`. Bonsai measured ~1%
  moment error from single precision; on the GPU they are computed on the CPU and uploaded, or
  accumulated in float-float.

## 7. Bodies

| Body | Tree | Evaluation |
|---|---|---|
| **Rigid** | one tree in the body frame, built once | the existing rigid socket: back-transform the query point, walk the static tree. Nothing is rebuilt per tick |
| **Skinned** | parcels whose weight is 1 on a single bone move rigidly with it: **one tree per bone**, in the bone's frame, built once. Parcels with blended weights (the joints) stay finest-level points | per bone: back-transform, walk. Blend parcels: summed directly (they are few — a shell around each joint), or a per-tick tree by re-sorting keys if a profile asks for it |
| **Displaced** (vibration) | no merge is valid — neighbours move differently | finest-level parcels; a far-field multipole per mode is the natural shortcut, not built until needed |

The skinned row is the earlier "rigid is an internal fast path" idea made concrete: an articulated
body is mostly rigid pieces, and each piece gets the amortised evaluation; only the joints pay the
general price. Mass conservation is unaffected — the parcels are the same parcels, grouped.

## 8. Grid fields — a hierarchical response

`GridField` evaluates the propagation integral as `Σ W·m` over every cell (`design/physics.md` §4.3).
`W` is smooth far from the tower. Build it on a tree over the static grid: descend from the root and
stop where the restriction error bound (same bound as §6, with the cell's maximum plausible mass
anomaly in place of `mass`) is under `tol`; a coarse node's weight multiplies the *sum* of its cells'
masses (and, one order up, their first moment).

Because neither the cells nor the tower move, **the cut is fixed for the life of the response**: there
is no opening decision to flip, hence no jitter by construction. A room at 5 cm pitch is 4.8 M cells;
the fine region around five towers is a small fraction of that. The restriction of each frame to the
cut is one pass over the cells and belongs on the GPU with the solve.

## 9. The air

- **Finest level stays dense.** Air fills a closed room; sparsity saves nothing there. The MAC grid
  and its arithmetic are unchanged.
- **The pyramid is the multigrid.** The GPU port already has to replace MIC(0)-PCG (sequential) with
  a parallel solver; geometric multigrid on the coarsened levels of the room's tree is that solver.
  Solid cells at each level come from the obstacle trees (§10's occupancy), restricted conservatively.
- **Obstacles** are stamped from body trees, not from point splats: a leaf of level `ℓ` covers its
  `8^ℓ` cells exactly, so the volume fraction and the surface-area density `|∇φ|` used for film heat
  come from geometry. This addresses the measured 8–28% area shortfall on small bodies.
- **Adaptive air (last, gated).** Refinement around plumes and boundary layers on an SPGrid-style
  pyramid with 2:1 grading. Built only if the uniform GPU solver, measured on the M3, cannot reach
  the resolution a validated benchmark needs (Ra 10⁵–10⁶, then room scale). Adaptive incompressible
  solvers are a substantial piece of work with their own failure modes (spurious flow at level
  jumps); it is not committed to until the measurement asks for it.

## 10. Contact from voxels

M12 refuses box–box contact. Rather than add SAT for boxes and then again for every new shape, use
the representation every body already has:

- **Narrow-band signed distance** sidecar per body (distance to the surface for leaves within a few
  cells of it; sign from occupancy), computed at voxelisation.
- **Contact generation:** transform body A's surface leaves into B's frame; sample B's distance; a
  negative sample is a penetration with depth `−d` and normal `∇d`. Swap roles; merge.
- **Manifold reduction** to at most four points per pair (deepest, then maximal area), cached across
  steps for warm starting — standard practice, and what keeps a resting stack still.
- **Solver, sleep and the recorded track are unchanged** (sequential impulses, exact rest).
- **Broad phase:** bounding-box sweep. A world-level tree over bodies is added only when a scene has
  enough bodies for the sweep to show in a profile.

The standard that governs it is M12's: a settled body's pose is bit-constant. The test extends to a
box on a box and a stack of three.

## 11. The viewer

The scene viewer (M13) draws what the engine holds: merged leaves as instanced cubes of their true
size (far fewer instances than finest cells), an optional overlay of the tree's cell outlines by
level, the opening cut a tower used (which nodes it accepted, coloured by level — the "what did the
evaluation actually touch" view), and the air's pyramid levels on the slice.

## 12. GPU

- Trees are flat arrays of fixed-size records: uploaded once per rigid body or bone.
- The walk is an iterative descent with a small fixed stack per query; queries are batched over
  (measurement × tower × arm × flight node).
- Accumulation is differential-first, in float-float where `f32` is not enough; every such kernel has
  a parity test against the `f64` CPU walk, on real devices (Metal reorders error-free transforms
  under fast-math; that is the failure to look for).
- Builds that happen at run time (the obstacle stamp; a per-tick joint tree if ever needed) are key
  generation, radix sort and compaction — measured at well under a millisecond for 10⁶ keys on
  current GPUs (Cornerstone: 0.5 ms sort, 0.3 ms build on an A100).

## 13. Open questions

- **Order of the general multipole.** Octupole is where Cartesian tensors stay cheap; the table in §2
  says higher orders buy real θ at tight tolerances. Decide from a measured cost-versus-θ curve.
- **Bound tightness.** The bound in §6 ignores the cancellation between arms and between
  interferometers, so it is pessimistic — possibly by orders of magnitude for distant nodes, where
  the reading sees only the gradient. A bound on the *differenced* quantity (one more power of
  `Δr/r`) would open far fewer nodes. Derive it and test it before relying on it.
- **Per-voxel density.** Imported bodies with tissue-like density variation merge poorly. Whether to
  quantise density into a few materials (merge within each) is a modelling choice for the scene
  author, not the engine.
- **4³ re-pack** for the viewer if ray-marching the tree shows in a frame profile.
