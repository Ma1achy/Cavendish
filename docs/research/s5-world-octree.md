# One world octree for drawing and for the gravity read (S5)

*Research note for slice S5 (and S14's wider world). Written 25 Nov 2026. No code was changed; one
small benchmark was written and run ([`s5-world-octree/lod_walk.py`](s5-world-octree/lod_walk.py),
output in [`results.txt`](s5-world-octree/results.txt)). Tags: **[R]** read in a primary source this
session; **[S]** read only as a search-result summary; **[M]** measured here; **[D]** derived by me,
to be checked; **[U]** not verified (listed again in section 9).*

## 0. One-page summary

**Recommendation.** Keep S4's chunk (32³ cells, palette, `VoxTree` below it) as the level-0 node of a
**pyramid of equal-sized nodes**: node `(level L, ix, iy, iz)` spans 32 · 2^L · pitch on a side, its
parent is `(L+1, ix>>1, …)`, and 8 children share it ([S4 §2](../slices/S4.md) already has the level
bricks, the 32-cell brick and the `2^k · pitch` rule; this gives them a key and a gravity record).
Every node owns a small **`Summary`** (mass, centre, second to fourth moments, the `Σ m s^l` terms the
S4 bound needs, a kind: empty, uniform, mixed). Drawing and reading are **two cuts through the same
tree**, each with its own resident set:

```
            shared tree (keys, Summary)         drawing adds            reading adds
level 12     S                                  -                       -
   ...       S S S                              -                       -
level  1     S S S S S S S S                    Brick (bits+material)   -
level  0     S S S S S S S S S S S S S S        Brick                   Body (palette + VoxTree,
                                                                         moments at every inner node)
```

* **Drawing** opens nodes by screen-space error: S4's clipmap rings, unchanged, now addressed by
  `(level, key)` (a ring is a set of level-L nodes). Levels 1 and up are ray-marched bricks, so there
  are no cracks to stitch and no Transvoxel ([section 1](#1-how-others-structure-big-voxel-worlds-with-lod)).
* **Reading** opens nodes by phase error at each tower: a node is accepted when its rigorous moment
  bound (S4's, extended to carry `Σ m s^l` upwards by the triangle inequality, [D]) is below that
  level's share of the tolerance, otherwise its eight children are opened, down to level-0 bodies
  and then the leaf cube kernel. The *radius of each level* is a measured table (data, per detail tier),
  the *bound* is what the ledger reports. Error-based opening is what the FMM literature uses to
  avoid the long error tails of a fixed opening angle ([Dehnen 2014](https://arxiv.org/abs/1405.2255) [R]).
* **Edits** add and subtract raw moments up the ancestor path (about 5 + log₂(world/32) records per
  edited leaf), then recompute dirty summaries from their children in Morton order at commit, so the
  bytes are the same whatever order the edits came in (S3's R1 property).
* **Infinite flat plane = analytic slab + signed departures.** An infinite uniform slab has no
  gradient (benchmark D: a finite 64 m disc gives 2.3×10³ rad over 5 m; 6.4 km gives 25 rad; the
  scaling is 1/R, [M]), so the tree holds only departures from the plane (hills positive, ditches
  negative mass), and the read says what lies beyond its horizon, bounded by 2πGρH/R [D].
* **Sphere world = analytic layered sphere + voxel crust**, in a planet-centred, axis-aligned 3-D
  octree (the same code and the same cube kernel), *not* a cube-sphere; the cube-sphere stays a way to
  tile elevation data. Records are stored relative to their node centre in `f32`; only the
  node-to-tower vector is `f64` (or float-float on the GPU) ([section 4](#4-infinite-plane-and-sphere)).

**What the benchmark said** (a 128³-cell town, quadrupole only, so pessimistic against Cavendish's
octupole): with the geometric criterion the nearest accepted node of each level sits **2.6–5.7 node
sides** away (levels 1–4, θ = 0.35); the floor is `√3/2 / θ = 2.5` sides, so rings are about three
nodes wide at every level and the table is nearly level-independent. A crude bound (`l ≥ 3` terms, all
mass at distance `a`) is **10⁴–10⁵ × the measured error**, against 175–457× for S4's bound
([S4 §7.3](../slices/S4.md)): the bound's looseness is what sets the node count at µrad, not the
structure. Error-driven and θ-driven opening cost about the same nodes at the same error here
(§6, C): the gain of error-driven opening is a ledger and a controlled tail, not fewer nodes.

**What I could not verify:** that the S4 `Σ m s^l` terms can be carried upwards with a bound close
to the leaf-built one (the S4 code was not read, only its slice note); the sizes of any planet-scale
case; ESVO, GigaVoxels and NanoVDB internals beyond abstracts; that float-float node offsets are
enough at 6.4×10⁶ m. Each has a measurement in the slice ([section 8](#8-what-to-test-and-what-could-not-be-verified)).

---

## 1. How others structure big voxel worlds with LOD

| System | Structure | LOD chosen by | Edit cost | Memory | Fit for Cavendish |
|---|---|---|---|---|---|
| **Fixed-size chunks per level** ("LOD chunk octree") | Every level's chunk has the same cell count (here 32³), cells double per level, 8 children per node | Distance or screen error per node; split when too coarse, merge when far | One chunk per level touched: re-aggregate the 8-children union | 1 + 1/8 + 1/64 … = **8/7** of the finest level (13 % extra) [D] | **The shape to take.** S4 already has it for bricks |
| **Geometry clipmaps** ([Losasso & Hoppe 2004](https://hhoppe.com/geomclipmap.pdf) [R]; [GPU Gems 2 ch. 2](https://developer.nvidia.com/gpugems/gpugems2/part-i-geometric-complexity/chapter-2-terrain-rendering-using-gpu-based-geometry) [R]) | Nested n × n grids (n = 255) of a mip pyramid, centred on the viewer, toroidal addressing | By ring: levels fixed by distance from the viewer; levels dropped when the viewer is high (extent < 2.5 h); a blend band at each level's outer edge | Moving the viewer refills an L-shaped strip per level; coarse levels barely move (relative motion halves per level) | One float height map + normals per level; 33 % more than the finest in 2-D | S4's rings are this in 3-D. A clipmap is a *cut*, not a store: S4's rings are the draw cut |
| **Transvoxel** ([Lengyel](https://transvoxel.org/) [R]) | Marching-cubes blocks of equal cell count at different resolution, joined by "transition cells" | Chosen by the caller (any block pyramid); the algorithm only seals seams | Local: a transition cell needs only its 9 neighbouring samples | Lookup tables only (73 classes from 512 cases) | Not needed: S4 does not mesh levels ≥ 1 (they are ray-marched bricks). Only 2:1 ratios across a seam are handled [R] |
| **OpenVDB** ([overview](https://www.openvdb.org/documentation/doxygen/overview.html) [R]) | B+tree, default 5-4-3: leaf 8³, internal 16³ and 32³, a root map; constant *tiles* at any level | None built in (a data structure, not an LOD scheme) | Cheap; accessors must be cleared when nodes are deleted | Tiles make uniform regions free; leaves dense | The tile idea is S3's lossless merge. 5-4-3 is a very shallow tree: Cavendish's 32³ chunks are close to a 5-level VDB leaf |
| **NanoVDB** ([NVIDIA research page](https://research.nvidia.com/labs/prl/publication/nanovdb) [S]; [follow-up on editing](https://arxiv.org/pdf/2611.08665) [S]) | A pointer-free linear snapshot of a VDB, copyable to the GPU by `memcpy` | n/a | **Read-only**: topology edits need a rebuild (the 2026 follow-up adds GPU topology edits) [S] | Compact; optional quantisation | Argues for a flat, pointer-free layout of the cut's `Summary` array, which is what S3's note already chose |
| **Sparse voxel octrees** ([Laine & Karras, ESVO](https://research.nvidia.com/publication/2010-02_efficient-sparse-voxel-octrees) [R, abstract only]) | Compact octree with contours (a plane per node for smooth surfaces) | Not stated in the abstract | Not addressed; built offline | "Compact" | Contours are the same idea as the exact-skin pieces for drawing; the note's S3 conclusion (shallow chunks over a linear octree) stands |
| **GigaVoxels** ([Crassin et al. 2009](https://hal.archives-ouvertes.fr/inria-00345899) [S]) | Octree of node pool + brick pool, streamed *by the ray traversal itself* | By what rays touch: data loaded on demand, detail clusters at the boundary of empty and full space | On-demand, view-driven; not an editing structure | Several billion voxels in a limited GPU budget, 20–90 fps (2009 hardware) [S] | The model for "resident set = cut + ancestors, filled by demand". For the read, demand is the *tower's* opening test, not a ray |
| **Voxel planets, cube-sphere** ([acko.net, Making Worlds](https://acko.net/blog/making-worlds-4-the-devils-in-the-details/) [R]; [gamedev threads](https://gamedev.net/forums/topic/621554-imprecision-problem-on-planet/) [S]) | A subdivided cube projected to a sphere, a chunked quadtree per face, tiles split and merge around the camera | Quadtree split on screen need; tiles carry their own 1-pixel border so neighbours match | Per-tile regeneration | Two-D height data (16-bit heights banded beyond a few zooms [R]) | Fits height-field terrain; wrong for gravity (cells are not cubes, so the cube kernel and prism kernel do not apply) |
| **Infinite terrain** ([Minecraft region files](https://minecraft.wiki/w/Region_file_format) [R]) | Chunks keyed by integers, grouped 32 × 32 per file with a sector table (offset, size), a timestamp table | By distance, streamed | A chunk rewritten into its sector slot | Files are sparse; unvisited regions do not exist | S4's region files (8³ chunks, 512 slots) are the same design, one file per level |

Three things follow.

1. **For the read, a "seam" is only a partition.** A gravity cut is correct if every cell is covered
   exactly once by some accepted node (mass conserved); there is no surface to crack. S4's R12 ("clipmap
   covers once", with an unsnapped control that double-covers, [S4 §7.3](../slices/S4.md)) is the
   invariant, and the same test must hold for the read cut.
2. **The draw cut and the read cut differ in shape, not in structure.** A clipmap ring is a cube shell
   about the *camera*; the read cut is a set of shells about each *tower* with radii set by tolerance.
   Both are antichains in the same tree. A node can be in both resident sets (a tower beside the camera).
3. **Equal cell count per level is what makes streaming cost a function of the number of levels.** Each
   ring needs about the same number of chunks per level, so the work to move is constant per level, as
   in the clipmap ("coarse levels update rarely, the relative motion shrinks exponentially", [GPU Gems 2](https://developer.nvidia.com/gpugems/gpugems2/part-i-geometric-complexity/chapter-2-terrain-rendering-using-gpu-based-geometry) [R]).

## 2. One tree, two criteria

**Opening.** The gravity criterion is the Barnes–Hut / FMM one with Cavendish's bound. The original
opening-angle rule has no bounded error, and an error-based rule beats it: with a fixed angle
Dehnen's 10⁷-particle runs have tails reaching 1000 × the median error, while with criteria built on
his improved error estimates the maximum error is about 10 × the target and "more than 2 orders of
magnitude smaller" than the geometric criterion's ([Dehnen 2014, §5](https://arxiv.org/abs/1405.2255) [R],
read from the PDF text; footnote 3 on the original Barnes–Hut definition). His own rigorous bound is
"typically 10–100 times larger than the actual largest error" (his eq. 10) [R], so a loose bound is
normal: S4's 175–457× is looser than that, the crude bound in my benchmark far looser (10⁴–10⁵ ×).
[Salmon & Warren 1994](https://doi.org/10.1006/jcph.1994.1050) [S: abstract only] derived worst-case
bounds from "additional, easily computed moments of the mass distribution" and four criteria that bound
the error per evaluation. The Cavendish bound has the same character: S4 uses `Σ m s^l` per order,
summed from the leaves.

**One hierarchy, shared parts.** Shared: the topology (keys, parent and child), the `Summary` per node,
the cache files, the dirty flags. Separate: the *resident sets* (a camera cut and a cut per tower, each
a set of nodes plus their ancestors) and the *payload* held for a node.

**What each node must store.**

| Field | Used by | Size | Upward rule |
|---|---|---|---|
| kind (empty, uniform m, mixed) | both | 1 B + material | empty if all eight empty; uniform if eight equal uniforms (S3 merge) |
| mass | read | 8 B (float-float on the GPU; the S3 note shows an `f32` mass alone gives 3×10⁻⁷ rad) | sum |
| centre of mass, second and third central moments (the octupole expansion) | read | 20 numbers `f64` on the CPU; ~84–88 B on the GPU with centre in `f32` relative to the node centre ([S3 note §5](s3-sparse-voxel-structures.md)) | parallel-axis rule from the children, as S3 does within one tree |
| `S_l = Σ m s^l`, l = 2…4 (about the node's own centre) | read, for the bound | 3 × 8 B | `S_l(parent) ≤ Σ_children Σ_k C(l,k) S_k(child) d^(l−k)`, with `d` the child-centre to parent-centre distance (triangle inequality on `s`) [D]. Looser than a leaf-built value, never smaller. **Not checked against S4's code** |
| exact-skin boundary moments (S5-09) | read | nothing new above the leaf | the boundary leaf's mass, centroid and second moment are the *leaf data*; every node moment is already a sum of leaf moments, so exactness propagates with no extra field ([exact-skin note §5](exact-skin-and-partial-fill.md)). The note's 8–20 bytes per boundary leaf sit in a side table at level 0 only |
| `Brick`: 32³ occupancy bits, 4³ sub-block mask, a material per sub-block | draw | ≈ 4.2 KiB | OR of the eight children's bits, then 2 × 2 × 2 down-sampling; the material of a sub-block is the **mass-weighted dominant** child (not the first), so a thin roof does not paint a hill |
| optional mean colour per sub-block | draw | 3 B × 64 | mass-weighted mean; the colour view's field is data, so keep it a field id, not a colour |
| `Body` (palette + `VoxTree` of S3 with inner-node moments) | read and near draw | as today | level 0 only. A level-L ≥ 1 node has **no body**: opening it goes straight to its eight children's `Summary`, five tree levels at a stride |

A level-L node's brick is not a coarsened *body*: the read never needs the brick's cells, only the
`Summary`. That is why the shared structure is a pyramid of `Summary` records and the draw payload
hangs on it: the two uses do not fight over a layout.

**Why not one cut.** The draw cut and the read cut disagree: a tower inside a building 3 km from the
camera needs level-0 data there to read, and nothing to draw; the camera over a distant city needs
level-3 bricks and no gravity data at all. Separate resident sets keep each cheap; the shared tree
makes an edit land once.

## 3. Where chunks sit in the tree; files, streaming and edits

* **Key.** `NodeKey { level: u8, ix, iy, iz: i32 }`. Level 0 is today's `ChunkKey`, which means the S4
  region format only needs `level` in its header and a file per level. The world's extent bounds the
  top level; 10¹¹ cells is a world of ~4,600³ finest cells, 144³ chunks, so ~8 chunk levels above 0 [D]. The S4 `MAX_LEVELS = 12` stays plenty for a
  patch; a planet needs ~25 ([section 4](#4-infinite-plane-and-sphere)).
* **Regions.** One file per `(level, region)` with the region being 8³ nodes of that level (S4's
  header: magic, version, world hash, 512 slots). A level-L region covers 8× the span of a level-0
  one, so the number of regions needed is about constant per level, as the number of chunks is. Storage
  of all levels is 8/7 of level 0 [D]. Coarse nodes are *derived* data: their file records the hash of
  the level-0 data they were made from, so a stale coarse file is rebuilt rather than trusted (S4's
  cache is already keyed by the voxeliser's version, TASK-S4-16).
* **Streaming.** Two queues feed one cache, ordered by need: the draw queue (screen-space priority,
  S4-05) and the read queue (tower-distance priority, by the next node the opening test wants). A
  node is built from cache, else from its eight children if those are resident, else voxelised; a
  coarse node is **never built by voxelising at coarse pitch** (it would violate "merged = unmerged"),
  it is built bottom-up from level-0 (top-down `cover` as today for the finest, aggregate upward). The
  GigaVoxels loop (data demanded by the traversal, [S]) is the model, bounded per poll as S4 does.
* **Edits** (paint, terrain change, a moving fluid later). Let a leaf change by `Δ` in raw moments
  (mass, first and second raw sums, and the third: each is additive about a fixed origin).
  1. *Interactive:* subtract the old leaf, add the new, up every ancestor: 5 in-chunk levels + L_top
     chunk levels, so ~13 records of ~20 numbers for a 10¹¹-cell world. Cost per edited leaf is tens of
     flops per record [D]. This drifts in `f64` (sums are not bit-reproducible by order), so:
  2. *At commit (end of a stroke, or per frame budget):* recompute each **dirty** summary from its eight
     children in Morton order. Dirty bits propagate to the root; cost is eight records per dirty node,
     and a stroke touching `c` chunks dirties about `c + c/8 + …` nodes. This restores the "same bytes
     whatever the order" property.
  3. *Bricks:* OR-and-downsample the dirty ancestors' bricks, coalesced and budgeted per frame; a
     brick is 32768 cells and up to 8 children, so a few ms for a stroke is plausible but **unmeasured**.
  4. *Exact skin:* only the touched boundary cells are clipped again (exact-skin note §5, "moving
     bodies").
  A **moving fluid** changes mass per cell every step: that is not a tree-edit pattern but a field
  coupling, and belongs to S8/S9's adaptive grid, which should *produce* nodes in this tree (the owner's
  S9 balanced octree is the same shape). Leave it out of S5.

## 4. Infinite plane and sphere

### 4.1 The plane

An infinite uniform slab of thickness `t` has `g_z = 2πGρt` independent of height, so its gradient
is zero. A *finite* disc of radius `L` has on-axis `g_z(h) = 2πGρt (1 − h/√(h²+L²))` (the standard
disc result, [D] from Newton's integral), whose gradient over 5 m is ~`2πGρt · Δz · L²/(h²+L²)^{3/2}`.
**[M]** (benchmark D, ρ = 2000, t = 8 m, h = 12.5 m, k_eff T² = 4.8×10⁹ rad per m s⁻²):

| patch radius | phase over a 5 m baseline |
|---|---|
| 64 m | −2.32×10³ rad |
| 640 m | −2.51×10² rad |
| 6.4 km | −25.2 rad |

The 1/R fall is slow: tiling a plane and stopping at the last tile leaves a phase of tens of radians,
and the error is a **step at the tile edge**. So the plane is *not* tiled as matter:

```
world = infinite slab (analytic: zero gradient, constant g)  +  departures D(x, y, z)
        D = +density where mass is above the reference surface z0 (hills, buildings)
            -density where the slab is carved out (ditches, a cut)
```

Tiles are the usual chunks, but of signed departures, with the reference surface `z0` and the slab's
density as scene data. Only departures are read. Beyond the **horizon** `R_h` (the farthest resident
ring) the read says what it assumed. Bounding the neglected tail: a layer of surface density `σ` beyond
`R` has gradient at most `2πGσ/R` [D: `∫ G σ 2πr dr / r³`, the same 1/R seen in the table above]; with
`σ ≤ ρ H`, `H` the largest departure *thickness left after averaging at the coarsest resident level*, that is
the tail bound. Worked: ρ = 2000, H = 1 m, R = 6.4 km gives 1.3×10⁻¹³ s⁻², about 3×10⁻³ rad over a 5 m baseline
at k_eff T², and 10 m of departure gives 3×10⁻² rad (far above µrad): the horizon must be set by `H`,
which only the data can say. This is where the rings'
means matter: each coarse ring carries the *mean* of the departures in it, so the unresident tail is
what the coarsest level's variance leaves.

Hills and trees outside the horizon are unknowable from a bound alone; the tiers (preview, standard,
reference) name `R_h` and `H`, and the badge states them. The test is the analytic one: a plane with a
single box (a prism with a closed form, S4's R5) on it reads as the prism to ≤ 1e-9, and a plane with
nothing on it reads exactly zero gradient.

### 4.2 The sphere

* **Analytic base.** A radially layered Earth model is exact outside each shell (Newton's shell theorem:
  a layer below the tower acts as a point at the centre, a layer above contributes nothing), so
  the base needs no cells. The standard free-air gradient `−2g/R ≈ −3.1×10⁻⁶ s⁻²` is its main term
  (a known figure, not measured here); the instrument's response to it is a one-line check of the
  sphere world.
* **Voxel crust as departures.** Exactly as the plane: signed mass relative to the reference sphere
  (topography, ocean, ice, a mine). The tree is a **planet-centred, axis-aligned 3-D octree** in which
  only crust chunks exist. This reuses the Morton keys, the cube kernel, the prism near-kernel and the
  clipmap rings without change. A **cube-sphere** (a quadtree per cube face, as in acko's planet [R]) is
  the right *index* for elevation tiles (a DEM arrives in tiles), and the voxeliser can sample it, but
  its cells are not cubes, so every kernel in `cav-core` would need a frustum-cell variant. I would
  not do that for S5 [D, a judgement].
* **Sizes.** Earth's radius 6.4×10⁶ m, a 1 m crust pitch, chunk 32 m: ~2×10⁵ chunks across, so
  ~18 levels over the planet at 1 m (≥ 25 at 1 cm) [D]; chunk keys in `i32` are enough. Only crust
  chunks near a bubble are at level 0; the rest of the surface is level ≥ 6, whose count is small.
* **Precision.** `f32` keeps 24 bits: at 6.4×10⁶ m its spacing is 0.5 m, useless as an absolute
  position (and the web posts say as much, [S]); so no absolute position is ever `f32`. Records are
  stored relative to the node's own centre (`f32` offsets, the S3 note's tiering), node centres come
  from integer keys times a power of two times the pitch (exact), and only the node-to-tower vector
  is formed in `f64` on the CPU, or as float-float on the GPU (S5-01/02), for the nodes in the
  tower's cut. The read's frame is the **tower-local frame** (S5-02): rotate the planet-centred
  positions once, subtract in `f64`, hand the GPU small numbers. The draw does the same
  camera-relative ([S4 §3.4](../slices/S4.md): origin formed in `f64` on the CPU).
  Float-float's ~10⁻¹⁴ relative on 6.4×10⁶ m is ~10⁻⁷ m: fine for a node 100 m away, which the cut
  makes the typical case [D; not measured].

## 5. Measuring the LOD radii (experiment design, criteria first)

**Aim.** For each level L and tier tolerance τ (rad), the smallest radius `r_L(τ)`, in node sides of
level L, outside which a level-L node may be accepted by its moments. The table is data in the tier.

**Reference answers.**
1. *world-box* (S4): the prism closed form, to rounding; the leaf-exact sum is already held to it at
   ≤ 1e-9 (R5). The cleanest case: a uniform box has no surface error.
2. *town* (S4's scaled stress-1e10): the leaf-exact read (the cube kernel on every leaf) is the
   reference. The cost is the whole reason for the tree, so use town at a pitch where it is feasible
   (the benchmark below: 3.8×10⁵ nodes in 3 s of numpy for a 128³ world).
3. *voxel bodies* (S3): sphere, icosahedron, a cylinder, with the polyhedral closed form ([S5-08](../../plan/tasks.yaml))
   as the **independent** reference once it exists. A sphere checks the mass-weighted material rule
   for the coarse nodes, since it has curved surfaces.
4. *A layered sphere world*: the shell theorem value, with the voxel crust of a smooth bump (a
   spherical cap, which has a closed form) for the departure path.

**Protocol (one level at a time).** For each scene, ≥ 8 tower positions (a yard, a roof, a corridor
between buildings, beside a wall, in a tube; **worst over positions, never the mean**), for each L:
1. Build the cut "leaf-exact everywhere except that every level-L node whose least distance to the
   arm is ≥ ρ sides is taken by its moments", with everything farther than that also at level L (no
   coarser level). Error `e_L(ρ)` = |phase − reference|.
2. `r_L(τ_L)` = the smallest ρ with `max over positions e_L(ρ) ≤ τ_L / safety`, where `τ_L` is the
   level's share of τ (equal shares, then geometric as a second try) and `safety` ≥ 10 (Dehnen's tails:
   the maximum is ~10 × the median error, [R]).
3. *Compose:* run the full cut with all `r_L`; measure the total.

**Pass criteria (written before measuring).**
* **Sound:** the reported bound ≥ the measured error in every case (hard fail otherwise); the same
  property S4's R7 holds, now at every level.
* **Within tolerance:** the composed error ≤ τ at all positions, at four tolerances spanning 10⁻² to
  10⁻⁶ rad.
* **Not vacuous:** bound ≤ 1000 × measured error (S4's R7 criterion). The benchmark's crude bound would
  fail this at 10⁴–10⁵; S4's tightened one passes (175–457×).
* **Monotone:** `r_L` non-increasing in τ; cut nodes non-decreasing as τ falls.
* **Self-similar or tabled:** report `r_L / side_L`. If it is within ±30 % across levels 2…top, store one
  `ρ(τ)`; otherwise store the table. (The benchmark's radii, in sides: 5.7, 3.3, 2.6, 2.8 at levels
  1–4.)
* **Partition:** every leaf is covered exactly once by the cut (mass conserved to the bit), with an
  unsnapped control that fails (S4's R12 pattern).
* **Control arm:** radii halved must fail `τ` on at least one scene, or the test could not fail.
* **Convergence:** on the sphere world, the read converges to the shell-theorem value as `τ` falls
  (the S14 plan's stated test); and with the **octupole off** the radii must grow, showing the order
  matters (a check that the measurement is sensitive).
* *Not gating, recorded:* nodes per tower, microseconds per read, per tier.

**The gate for the owner's "measured LOD radii".** A tier is **valid** only if its stored table
reproduces its stated `τ` on the reference scenes at that commit; a changed table or tolerance is
recorded in the slice notes, never moved to make the report green (CLAUDE.md).

## 6. Benchmark: what the walk does at a small scale [M]

[`lod_walk.py`](s5-world-octree/lod_walk.py), numpy, one thread, 3 s. A 128³-cell world of 1 m cells at
2000 kg/m³: an 8 m ground slab, 60-ish random box buildings 4–20 m tall, a ditch, the tower's yard kept
clear; 162,259 occupied cells. A gradiometer reads `g_z` at two points 5 m apart (z = 20.5 m and 25.5 m)
and the phase is 4.8×10⁹ rad per m s⁻² of difference (746.67 rad leaf-exact). The octree is built by
2×2×2 reduction of mass, first moment and second raw moment (the Cavendish upward rule); a node's
radius `a` is its farthest corner from its centre of mass (conservative). **The reference is the
point-mass sum over cells**, so this measures *the walk's* error, not the cube-kernel error; and the
expansion stops at the **quadrupole**, where Cavendish uses the octupole, so errors here are larger
than Cavendish's at the same angle.

**A. θ-driven.** (`error` = |phase − 746.67| rad)

| θ | nodes | error, monopole | error, quadrupole | crude bound |
|---|---|---|---|---|
| 0.90 | 104 | 2.1×10² | 1.4×10² | 1.7×10⁶ |
| 0.50 | 470 | 6.4×10¹ | 2.2×10¹ | 7.5×10⁴ |
| 0.35 | 971 | 1.8 | 8.3 | 2.3×10⁴ |
| 0.25 | 2,020 | 0.20 | 0.45 | 7.2×10³ |
| 0.15 | 6,166 | 0.54 | 4.7×10⁻² | 1.5×10³ |
| 0.10 | 17,542 | 0.19 | 8.8×10⁻³ | 4.0×10² |

Read this table honestly: the error is not monotone at large θ (the quadrupole is *worse* than the
monopole at θ = 0.35, 0.25) because the error is a signed sum over many nodes that cancels in places;
a single run is not a trend. That is the reason the experiment in §5 takes the **worst over positions**
and a safety factor, not one number. The crude bound is 10⁴–10⁵ × the error (here, the first dropped
order summed geometrically with all of the node's mass at the bounding radius).

**B. LOD radius per level** (θ = 0.35, quadrupole; nearest accepted node, in sides):

| level (side) | nearest accepted R |
|---|---|
| 1 (2 m) | 5.7 sides = 11.5 m |
| 2 (4 m) | 3.3 sides = 13.1 m |
| 3 (8 m) | 2.6 sides = 20.6 m |
| 4 (16 m) | 2.8 sides = 45.1 m |

For a cube with its centre of mass at its centre, `a = (√3/2) side` and `a/R ≤ θ` gives `R ≥ 2.47 sides` at
θ = 0.35: the measured radii sit on that floor at levels 3–4 and above it at the fine levels (centres of
mass offset by buildings). So "about three nodes wide at every level" is a *geometry* result, the
heterogeneity makes it larger at fine levels, and the tolerance rule (§5) is what can tighten it.

**C. Error-driven opening** (accept when `G M/R² Σ_{l≥3}(l+1)(a/R)^l ≤ ε`, quadrupole):

| ε (m s⁻²) | nodes | error (rad) | summed bound (rad) |
|---|---|---|---|
| 10⁻⁹ | 3,110 | 0.18 | 3.4×10³ |
| 10⁻¹⁰ | 8,972 | 4.4×10⁻² | 8.8×10² |
| 10⁻¹¹ | 24,519 | 2.8×10⁻³ | 2.6×10² |
| 10⁻¹² | 75,010 | 2.5×10⁻³ | 52 |
| 10⁻¹³ | 171,629 | 5.4×10⁻⁵ | 8.8 |

At equal node counts the error-driven cut is about as good as the θ-driven one (A, θ = 0.15: 6,166 nodes,
0.047 rad; C, ε = 10⁻¹⁰: 8,972 nodes, 0.044 rad). Its point is not fewer nodes but a ledger: the sum of
bounds is a rigorous ceiling and the cut stops at the ceiling asked for. **The finding that matters:**
at 5×10⁻⁵ rad this 2-million-cell world needs 171,629 nodes (46 % of its 375,156 leaf cells); a
*bound* a hundred times tighter would cut this by a large factor. µrad reads at 10⁹–10¹¹ cells live or
die on the bound's looseness and on the order of the expansion, not on how the tree is laid out. This
is why §5 tests soundness *and* non-vacuity.

**D. Slab**: see §4.1. The 128 m × 128 m square patch alone (no buildings) has a leaf-exact phase of
2.04×10³ rad and the walk reads 2.03×10³ with 394 nodes: the cut is cheap, but the *physical* answer is
wrong by thousands of radians if the slab ends, which is the argument for the analytic slab.

**Limitations.** One world, one seed, quadrupole only, two points; point-mass leaves; a crude bound
that is not S4's. It supports the shape of the argument (rings of ~3 sides; the bound dominates cost),
not a table to copy.

## 7. Recommended design for S5

```
                      tower A bubble      tower B bubble
                         ________            ________
  level 6 ring  [   ..   |  L0  |   ........ |  L0  |    ..    ]   read cut (one per tower):
  level 3 ring  [  .. ...| ring |...         | ring |... ..   ]   level-0 bodies in the bubble,
  level 1 ring  [ ...... | L1   |........... | L1   |.........]   rings of ~3 nodes per level out
                                                                   to the measured r_L(tau); anything
        camera cut (S4's clipmap) is centred on the camera, rings of n nodes   past the horizon is
        per level from the screen error; ring 0 = meshes, ring >= 1 = bricks    "not read", said

       shared: NodeKey (level, ix, iy, iz), Summary, region files per level, dirty bits
```

1. **Add `level` to the chunk key** and a `Summary` to each node: the S4 chunk is level 0.
2. **Build the pyramid bottom-up** at voxelise time (aggregate moments in Morton order; the lossless
   merge for uniform nodes), stored as one file per `(level, region)`, derived and hash-checked.
3. **The read walks**: per-tower stack over `Summary` from the top; accept by
   `bound(node, R_to_arm) ≤ ε_level` with `ε_level = share × tolerance`; open to children otherwise;
   at level 0 open the body (S3 `VoxTree`, the existing cube kernel and prism near kernel, then S5-10's
   exact-piece near kernel). Move the whole walk to the GPU as S5-01 plans, reading the cut as one flat
   array of `Summary`.
4. **Tier data holds `{ρ(τ) or r_L, safety, horizon R_h, H}`.** The measured table (§5) populates it;
   the S5-03 error badge shows the bound, the not-read part, and the tier.
5. **Base worlds**: `patch` (finite; today's), `plane` (analytic slab + departures, §4.1) and `sphere`
   (analytic layered sphere + crust departures in a planet-centred frame, §4.2): one more field in the
   world spec, the same tree.
6. **Draw** stays S4's clipmap, now with node keys; mass-weighted dominant material per sub-block.

**Costs** [D unless noted]. Pyramid: 8/7 of level 0. Summary: ~190 B on the CPU (20 `f64` moments + 3 `S_l` + kind),
~112 B on the GPU. A 10¹¹-cell world is ≤ 3×10⁶ level-0 chunks if completely mixed (it is not: uniform
chunks are tiles, S3's merge); at 190 B a summary that is ≤ 0.6 GB for all levels' summaries *non-resident*,
and a tower's cut is thousands to a few hundred thousand nodes (§6: ≈ 10³ at θ = 0.35, ≈ 10⁵ at 10⁻⁴ rad
for a 2×10⁶-cell town) so ≤ 20–50 MB a tower. The walk is a few hundred microseconds to
milliseconds at 10³–10⁴ nodes (no measurement of Cavendish's own walk was made).

**What I would drop or defer.** Transvoxel (no meshes above level 0); a cube-sphere kernel set;
GigaVoxels-style ray-driven demand for the read (the tower's opening test is the demand); a DAG for
bricks (the S3 note's conclusion); moving fluids in the tree (S8/S9).

## 8. What to test, and what could not be verified

**Tests** (each with a control arm that fails when the property is broken):
1. *Partition.* The read cut and the draw cut each cover every leaf once; control: an unsnapped ring
   (S4 R12).
2. *Pyramid = sum.* Every node's mass and moments are the sums over its leaves to ≤ 1e-13 relative
   (S3 R2), with a control of one zeroed third moment.
3. *Sound bound.* Over scenes, positions and τ, error ≤ bound; control: a bound with `l+1` replaced by 1.
4. *Carried `S_l`.* The upward-carried `S_l` ≥ the leaf-built one, within a stated factor (measure the
   factor; this is the open item of §0).
5. *Radii* as in §5, including the sphere's shell-theorem convergence.
6. *Plane.* Empty plane = zero gradient to rounding; plane + box = prism to 1e-9; control: the same
   plane tiled and ended at a finite patch fails by the §4.1 table.
7. *Edits.* Incremental update vs full recompute within 1e-12; after commit, byte-equal in any order of
   edits.
8. *GPU = CPU.* The cut's phase in float-float vs `f64` ≤ 1e-9 (the S5 exit), at planet offsets (a
   tower at 6.4×10⁶ m from the origin) with an `f32`-only control that fails.
9. *Draw.* Level-≥1 bricks equal the OR-and-downsample of their children (control: the first-child
   material rule makes a roof paint a hill).

**Could not verify**
* **ESVO, GigaVoxels, NanoVDB internals**: only the abstract (ESVO) or search summaries (GigaVoxels,
  NanoVDB) were read; the papers are not opened here. Their rows in §1 are what those sources state, not more.
* **Lengyel's dissertation and the Transvoxel block sizes**: the Transvoxel page does not describe chunk sizes
  or how equal-cell-count chunks are arranged. The "16³ at finest, 32³, 64³ coarser" remark came from
  a forum post in a search summary, not a primary source, and it describes chunks of *growing* cell
  count, the opposite of this note's choice.
* **Salmon & Warren 1994 and Barnes & Hut 1986**: only abstracts and Dehnen's account. Dehnen's PDF
  was read as text; its figures were not seen.
* **Cesium, "camera-relative" patents, Voxel Farm, Outerra**: not opened; the floating-origin claims
  here rest on S4's own `f64`-on-the-CPU rule and the arithmetic of `f32`.
* **S4's code**: I read the slice note, not `cav-sim/world.rs`. How its `Σ m s^l` is held (per chunk, per
  node) decides whether §2's carried `S_l` is a small change or a new field.
* **Costs at 10⁹–10¹¹ cells** are derived, not measured; the town benchmark is 2×10⁶ cells in numpy.
* **The tail bound of §4.1** (`2πGρH/R`) is my derivation; its use as a horizon rule needs the data's
  departure statistics.

**How to resolve each, in the slice:** the first slice task is the §5 harness on `world-box` and
`town` (it settles the bound, the radii and the carried `S_l` in one run); the second is a 10¹⁰-cell
walk timing with the S4 stress scene; the planet-offset parity test settles the precision.

## 9. Further questions (not chased)

1. Mass-weighted dominant material versus a per-brick palette for colour when levels mix many materials
   (a colour/appearance question for S19's materials).
2. Time-varying coarse cells (tides, groundwater, weather) written into the tree by S14: they change
   `Summary` records of existing nodes without any leaf; does the dirty-path rule hold (it should)?
3. Whether `Summary` should carry a 4th raw moment so the bound's `Σ m s^4` is exact at every node (a few
   bytes) rather than carried by inequality.
4. Whether error-driven opening should be *per tower* or *per arm point*: the S4 read uses the least
   distance to the arm path; a differential evaluation (S5-02) may justify a per-pair bound.
5. A hole-aware tile bound for the plane when the departures are not zero-mean (a quarry, a lake).
6. Using the shared tree for S9's adaptive fluid grid, to avoid a second octree there.

## Files

* [`s5-world-octree/lod_walk.py`](s5-world-octree/lod_walk.py): the benchmark (`python lod_walk.py`, numpy only, ~3 s).
* [`s5-world-octree/results.txt`](s5-world-octree/results.txt): its output, as quoted in §6.
