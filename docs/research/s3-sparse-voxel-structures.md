# Sparse voxel structures and GPU layout for S3–S5

*Research note for slices S3 (voxel bodies), S4 (big scenes) and S5 (gravity on the GPU). GPU-free: no
shader was run. Written 21 Nov 2026.*

## 1. Answer in brief

**Keep the plan's default.** One tree type, the linear octree from Morton keys with sidecar arrays
(`plan/REBUILD.md` §2), serves all three jobs. What differs between the jobs is only what sits above
the tree: a body's volume is one tree in its own frame; the world is a sparse table of chunk trees under
a small pyramid; the instrument walks the pyramid, then the chunk or body trees, with the same code.
Four refinements to the default, each argued below:

1. **Store leaf values as small integers, not floats.** A leaf carries a coverage count (the voxeliser's
   `k³` samples, ≤ 255) and a palette index, both `u8`; mass is `count × unit(material) × body scale`.
   Equality for the lossless merge becomes integer equality, "expanded tree = raster, bit for bit" is
   testable on the GPU as well as the CPU, and a DAG becomes possible later (§4.3).
2. **Moments cost more than the topology, so budget them.** Per internal node, mass in float-float,
   centre of mass relative to the node centre in `f32`, quadrupole and octupole in `f32`: about 88 B,
   against 8 B of topology (§5). Derivation in §5; it is mine and needs the S5 measurement.
3. **Do not choose the packing by the ray marcher's preference.** For the opening-criterion walk, a
   4³ node tested per child does 1.2–3× more kernel evaluations and 2.3–4.8× more criterion tests than an
   8-way node on both test scenes (§4.2). Keep octree granularity for the walk; a 64-tree re-pack is for
   the renderer in S4, and only if a frame profile asks for it (as `reference/design/voxel.md` §13 says).
4. **Reject NanoVDB/OpenVDB, SPGrid and DAGs as the core.** Reasons in §3. A DAG is the one worth
   revisiting, for the renderer's far field only.

Nothing found argues for changing the plan's storage; the measurements say the layout is worth ≤ 3× in
memory and ≤ 3× in walk work, while the tolerance (S5's level of detail) is worth orders of magnitude.

## 2. How to read the evidence

The network policy in this session let only GitHub and PyPI resolve, so **I could not open any paper**.
Claims about papers come from web-search summaries of those pages; each such link is marked *(summary)*.
Where I could not even get that, I say so in §9. Tags: **[M]** measured here, **[D]** from the v1/v2 design
documents in `reference/`, **[mine]** my own derivation or judgement, to be checked.

## 3. What Cavendish needs from the structure

| Job | Scale | Edits | Reads |
|---|---|---|---|
| (1) per-object volume, material palette | 10⁴–10⁷ cells per body **[D]** (`design/voxel.md` §3) | destruction splits components; repaint; rare | gravity walk, ray march in an oriented box, contact |
| (2) chunked world volume | 10⁹–10¹¹ finest cells (`plan/REBUILD.md` §2.7) | streaming in and out; stamps of moving bodies for air, water and contact; terrain damage | ray march, fluid stencils, broad phase, gravity for static matter |
| (3) instrument walk | ~3 000 evaluation points per shot **[D]** (`plan/REBUILD.md` §5.1) | none (reads) | top-down, opening criterion bounded in radians, per-node moments |

Two facts from the design shape everything. Reference-grade phase tolerances force the walk down toward
the leaves for nearby mass (`reference/design/voxel.md` §2 **[D]**), so the **leaf list near a tower is
the cost**, whatever the layout. And the gravity read needs fractional boundary mass and per-voxel
density, which is information a DAG and a palette-only chunk do not carry cheaply.

## 4. The candidates

### 4.1 What the literature says (qualitative)

| Structure | Core idea | Edit | Per-voxel values | GPU fit | Source |
|---|---|---|---|---|---|
| **Linear octree, Morton keys, sidecars** | sorted keys, nodes by level, values in side arrays | rebuild (bulk, sort-based) | any sidecar | flat arrays, no pointers | Karras builds octrees from Morton-sorted keys in parallel ([HPG 2012](https://doi.org/10.2312/EGGH/HPG12/033-037), *summary*); Cornerstone builds linear octrees for Barnes–Hut on GPU up to 8 trillion particles ([arXiv 2307.06345](https://arxiv.org/abs/2307.06345), *summary*) |
| **NanoVDB / OpenVDB** | B+tree-like, shallow, wide nodes; NanoVDB is the linearised, pointer-free read-only copy | OpenVDB dynamic; NanoVDB topology fixed | per node in tiles and leaves | designed for it | [VDB, Museth 2013](https://research.dreamworks.com/wp-content/uploads/2018/08/Museth_TOG13-Edited.pdf) *(summary)*; [NanoVDB](https://research.nvidia.com/labs/prl/publication/nanovdb), [FAQ](https://www.openvdb.org/documentation/doxygen/NanoVDB_FAQ.html) *(summary: 32-byte aligned, breadth-first, topology fixed)*; GPU-native topology updates are a 2026 proposal ([Sifakis et al.](https://research.nvidia.com/labs/prl/sifakis2026dynamic/dynamic_grids.pdf)) |
| **SPGrid** | a pyramid of sparse uniform grids addressed by Morton offsets inside a huge virtual array, relying on the CPU's paging hardware | write the page | any | **no**: it relies on x86 virtual memory ([project page](https://pages.cs.wisc.edu/~sifakis/project_pages/SPGrid.html), *summary*) | [paper](https://pages.cs.wisc.edu/~sifakis/papers/SPGrid.pdf) |
| **Sparse voxel DAG** | identical subtrees shared | needs a dedicated structure ([Careil et al. 2020](https://diglib.eg.org/handle/10.1111/cgf13916), *summary*) | colour handled by that paper; geometry is the natural payload | yes, read-only is easy | [Kämpe et al. 2013](https://www.cse.chalmers.se/~uffe/HighResolutionSparseVoxelDAGs.pdf): Epic Citadel at 128K³, 19 billion voxels, 945 MB against 5.1 GB for an SVO without pointers *(summary)* |
| **Brickmap, 8³ bricks** | two-level grid; dense bricks | O(1) per voxel (Utrecht thesis is about editing: [van Wingerden 2015](https://studenttheses.uu.nl/handle/20.500.12932/20460), *summary*) | dense | simple | GigaVoxels stores bricks in an octree with a brick pool, and bricks of 16³–32³ waste memory except for volumetric data ([Crassin et al. 2009](https://publications.graphics.tudelft.nl/papers/423); [Laine & Karras tech. report](https://research.nvidia.com/sites/default/files/publications/laine2010tr1_paper.pdf), *summaries*) |
| **4³ "64-tree"** | 64-bit child mask per node, popcount indexing, uniform sub-partitions implicit | rebuild | sidecar by popcount | built for ray marching ([guide](https://dubiousconst282.github.io/2024/10/03/voxel-ray-tracing), *summary*) | [Laine & Karras ESVO](https://www.nvidia.com/docs/IO/88889/laine2010i3d_paper.pdf) is the 2-level, 64-bit-descriptor ancestor |

Hybrids exist and win: composing formats per level reaches a better storage/ray-speed frontier than any
single one ([Hybrid Voxel Formats, arXiv 2410.14128](https://arxiv.org/abs/2410.14128), *summary*), and
Aokana splits an open world into chunks, each an SVDAG, with LOD and streaming so only about 5% of the
data is in VRAM ([arXiv 2505.02017](https://arxiv.org/abs/2505.02017), *summary*). Teardown, the model
for §5.2 of the plan, keeps a one-byte palette index per voxel with up to 255 materials per volume
([Voxagon](https://blog.voxagon.se/2020/12/03/spraycan.html), *summary*).

### 4.2 What I measured

`storage_model.py` builds a conservative top-down octree of two analytic scenes, merges eight equal
children bottom-up (the lossless merge), then counts what each structure would store. Scenes: a **sphere**
(radius 0.4 of the box; boundary cells fractional) and **terrain**: a rolling height field plus 48
random buildings on integer cells. Depth 10 is 1.07×10⁹ cells per box, depth 12 is 6.9×10¹⁰ (named 1024³ and 4096³ below). Byte sizes
are assumptions, stated in the script's `BYTES` table. Results are in `s3-sparse-voxel-structures/results/`.
Both scenes are friendly (smooth, one material, no interiors); real buildings will have more leaves.

**Storage** (MB, and bytes per occupied finest cell in brackets; a sidecar value is 4 B):

| structure | sphere 1024³: MB (B/cell) | sphere 4096³: MB (B/cell) | terrain 1024³: MB (B/cell) | terrain 4096³: MB (B/cell) |
|---|---|---|---|---|
| linear octree, topology only | 40.9 (0.1417) | 655.4 (0.0355) | 28.6 (0.1043) | 445.6 (0.0256) |
| linear octree + f32 mass sidecar | 57.8 (0.2000) | 925.2 (0.0502) | 40.1 (0.1465) | 626.1 (0.0360) |
|   + moments, float-float mass, f32 rest | 137.4 (0.4756) | 2,198.6 (0.1192) | 100.3 (0.3663) | 1,559.0 (0.0897) |
| 64-tree, topology only (12 B/node) | 2.3 (0.0081) | 37.3 (0.0020) | 1.9 (0.0069) | 29.6 (0.0017) |
| 64-tree + f32 slot sidecar | 30.9 (0.1070) | 495.7 (0.0269) | 23.6 (0.0861) | 370.5 (0.0213) |
| brickmap 8^3, 1 B palette | 29.0 (0.1005) | 679.1 (0.0368) | 24.9 (0.0908) | 598.3 (0.0344) |
| brickmap 8^3, 4 B per cell | 102.1 (0.3533) | 1,843.7 (0.1000) | 86.2 (0.3148) | 1,571.5 (0.0904) |
| chunks 32^3, 1 B palette | 97.9 (0.3387) | 1,597.5 (0.0866) | 78.7 (0.2873) | 1,371.0 (0.0789) |
| VDB 5-4-3 (dense node tables, estimate) | 109.8 (0.3801) | 1,729.3 (0.0938) | 89.8 (0.3281) | 1,438.1 (0.0828) |
| occupied cells / leaves / internal | 2.89e+08 / 4.21e+06 / 9.05e+05 | 1.84e+10 / 6.75e+07 / 1.45e+07 | 2.74e+08 / 2.89e+06 / 6.84e+05 | 1.74e+10 / 4.51e+07 / 1.06e+07 |

Reading it:

- **Leaves scale as the surface**, not the volume: the sphere goes from 4.2 M to 67.5 M leaves between
  depths 10 and 12, ×16.0, as 2 doublings of area predict. At 7×10¹⁰ cells the whole octree with values is
  about 0.9 GB. Storage is not the constraint at the plan's scale; **moments are** (last row of the first
  block, ~2.2 GB at depth 12).
- Every sparse option lands within about 3.5× of the others once the same per-cell value is carried. The
  64-tree's tiny topology is mostly an accounting effect: its leaf values move into the sidecar, which
  then dominates. (My slot count also over-charges uniform cells at odd levels; treat that row as an
  upper bound.)
- Dense chunks and 8³ bricks at 1 B per cell are the same order as the octree with 4 B values, but
  cannot carry fractional mass without doubling, and they have no merge: they pay for interior cells the
  octree gets for free in solid bodies.

**DAG** (unique subtrees, bottom-up hashing; node counts):

| scene | octree nodes + leaves | DAG, binary occupancy | DAG, 8-bit fractional values |
|---|---|---|---|
| sphere 1024³ | 5.1 M | 89.7 k | 701 k |
| sphere 2048³ | 20.5 M | 255 k | 2.75 M |
| terrain 1024³ | 3.6 M | 42.6 k | 411 k |
| terrain 2048³ | 14.0 M | 128 k | 1.58 M |

A DAG is 57–110× smaller than the octree on occupancy alone, and about 7–9× smaller with quantised fractions. Both are
large wins, but they are bought with quantised values, which the plan's bit-exact tree-equals-raster
test forbids unless the values are integers by design (refinement 1).

**Walk cost.** A top-down walk from one query point accepts a node when `size/distance < θ`, with the
octree testing 8 children per opened node and the 64-tree 64. `evals` is the number of kernel
evaluations that follow (accepted nodes plus leaves reached); `tests` the child records examined. Query
0.08 box-widths from the sphere's surface, 0.12 above the terrain.

| θ | sphere 1024³ evals (oct / 64) | tests ratio | terrain 1024³ evals (oct / 64) | tests ratio |
|---|---|---|---|---|
| 0.5 | 192 / 572 | 4.4× | 235 / 604 | 4.8× |
| 0.1 | 5 752 / 16 340 | 4.2× | 6 849 / 15 255 | 3.4× |
| 0.02 | 153 k / 398 k | 4.3× | 173 k / 336 k | 3.3× |
| 0.004 | 2.11 M / 4.21 M | 3.5× | 2.32 M / 2.89 M | 2.3× |

The 64-tree can only accept at every second level, so it opens more and evaluates more. Depth 12 gives the same picture (sphere 2.4–3.0×, terrain 2.0–2.5× evals). Two cautions: this counts work, not GPU time (a 64-way
node halves the dependent loads, which I could not measure); and a 64-tree *packing* with a criterion
tested at both octree levels (a 64-bit mask indexing 8+8 children) would recover octree granularity.
That is a storage change only, as `reference/design/voxel.md` §1 already says, and is the form to try if
S5 shows memory latency, not arithmetic, as the limit.

Note what the table does *not* say: at θ = 0.004, the sphere still needs 2.1 M evaluations for one query
point. At 3 000 points per shot that is 6×10⁹ kernel calls. The walk's cost is set by the tolerance and
the body's near surface; no layout in this note changes the exponent. S5's budget comes from the
instrument LOD and the bound, as the plan already says.

### 4.3 Per structure, in a sentence

- **Linear octree + sidecars:** the cheapest to validate and to evolve; moments, merge, LOD cut, DAG and
  64-tree re-packs are all derived from it. Edit cost is a rebuild; the design cites 0.5 ms sort and
  0.3 ms build for 10⁶ keys on an A100 **[D]**. I did not measure CPU rebuild times; a body of 10⁷ cells
  with 10⁶ leaves rebuilt on every edit would be the number to measure in S3.
- **NanoVDB/OpenVDB:** a good *library*, wrong *shape*. The design reached the same view (`voxel.md` §1)
  and I found nothing to change it: NanoVDB's topology is fixed once built *(summary)*, per-node statistics
  are not the multipole moments we need, and its 3-level fixed depth with wide nodes (my recollection of
  the default 5-4-3 layout; I could not confirm it) wastes memory on bodies of 10⁴–10⁷ cells (last row of
  the storage table is my estimate from that recollection). fVDB confirms the GPU pattern of one index grid with feature
  arrays beside it ([fVDB](https://arxiv.org/abs/2407.01781), *summary*): that is exactly our "topology
  plus sidecars", without taking the dependency.
- **SPGrid:** its pyramid of uniform grids is the right idea for the fluid (S8–S9), not for storage of
  matter, and its speed comes from CPU paging hardware, which wgpu does not have.
- **DAG:** wins on repetition (a tiled wall) and loses on per-voxel unique values; reading is easy,
  editing needs a purpose-built structure *(summary)*, and a moment per node is a sidecar per **unique**
  node, which is wrong for moments that depend on position (two identical subtrees at different places
  have different centres of mass in world frame; relative to node centre they are equal, so it can be
  done, **[mine]**, but only if moments are stored node-relative and that cost is accepted).
- **Brickmap 8³:** simple, editable, cheap to upload; no merge and no moments. A good *leaf* layer under
  an octree if S5 shows per-leaf overhead dominates, not a structure for the top.
- **64-tree:** best for ray marching; see the walk table.

## 5. Moments per node

The walk needs mass, centre of mass, quadrupole, octupole per internal node (`voxel.md` §6). In `f64`
that is 20 × 8 = 160 B per node; on the GPU, with no `f64` in WGSL, that does not fit anything.

**[mine]** Only the monopole needs high precision. A node accepted at tolerance `tol` has
`θ^(p+1) ≲ tol/4.7 rad` (the table in `voxel.md` §2), i.e. `θ ≈ 2×10⁻⁴` at `10⁻¹⁰` for the quadrupole. Then:

- an `f32` coefficient error of 6×10⁻⁸ in the quadrupole contributes `≈ 6×10⁻⁸ · θ² · 4.7 rad ≈ 10⁻¹⁴ rad`;
- an `f32` centre-of-mass error of 6×10⁻⁸ of the node size (stored relative to the node centre) moves the
  potential by `≈ 4.7 rad · 6×10⁻⁸ · θ ≈ 6×10⁻¹⁴ rad`;
- an `f32` **mass** (relative error 6×10⁻⁸) alone is `≈ 3×10⁻⁷ rad`, three orders over the tolerance. Mass
  needs float-float (8 B).

That gives 8 (mass) + 12 (COM) + 24 (quadrupole, 6 coefficients) + 40 (octupole, 10) = **84 B, 88 with
padding**, against 8 B of topology. For a 10⁹-cell body (905 k internal nodes) that is 80 MB. It is the
largest single array, so the unmeasured question is how many internal nodes ever get accepted: nodes
whose children are all leaves save at most a factor of 8 and may not earn their record. Measure on the S5
walk before fixing the cut. The `f32` argument must be tested against the `f64` twin on the anchors (it
is a bound, not a measurement).

## 6. Flat storage in wgpu, within WebGPU limits

- **Defaults:** `maxStorageBufferBindingSize` 128 MiB and `maxBufferSize` 256 MiB
  ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/GPUSupportedLimits), *summary*; wgpu's defaults
  match: [wgpu-types](https://docs.rs/wgpu-types/latest/src/wgpu_types/limits.rs.html), *summary*).
  Higher values must be requested on device creation and are capped by the adapter *(summary)*. I could
  not confirm `maxStorageBuffersPerShaderStage` (8 in the spec as I remember) or `maxBindGroups` (4);
  confirm both before fixing a binding layout.
- **What 128 MiB holds:** 16.7 M octree nodes of 8 B; 1.5 M moment records of 88 B (padded to 96: 1.4 M).
  A 10⁹-cell body fits one binding per array; the 7×10¹⁰-cell sphere does not (14.5 M × 88 B = 1.3 GB).
  The build PC is a 6 GB GTX 1660 SUPER (`plan/PROGRESS.md`), so the arrays *fit in memory* but **must use
  raised native limits**, queried from `adapter.limits()`; the default limits are for a browser, which
  Cavendish is not.
- **Layout (structure of arrays, element indices not byte offsets, `u32`):** `topology` (child mask,
  first child, flags), `leaf` (key, level), `leaf_value` (`u8` count, `u8` palette, packed in a `u32`),
  `moments_a` (mass float-float, COM), `moments_b` (quadrupole, octupole), `chunk_table` (hash of chunk
  coordinates to tree offsets), `params`: 7 bindings, under 8 if that limit is right. Element indices
  address 4.29×10⁹ entries, enough for every array here; byte offsets would overflow at 4 GiB.
- **No 64-bit integers:** WGSL has none in the core language; wgpu gates `i64`/`u64` behind a feature
  ([wgpu PR 6616](https://github.com/gfx-rs/wgpu/pull/6616), *summary*), and I do not know its status on
  Metal. Keys are therefore 30-bit (`depth ≤ 10`) in a `u32`, with a table above them, as the design
  already says (`voxel.md` §3). A world of 4096³ is a 4×4×4 grid of 1024³ trees, or 128³ chunks of 32³.
- **Do not rely on binding arrays of buffers.** wgpu supports them only behind native features *(I did not
  verify this)*; the pool-plus-table layout above needs none.
- **Beyond one GPU's memory,** residency is the answer, not clever packing: stream chunks around the camera
  **and** each tower (`plan/REBUILD.md` §5.3), as Aokana does *(summary)*.

## 7. What can be validated against a known answer

The plan's oracle is "tree expanded = raster voxeliser, bit for bit". What carries it:

| Choice | Bit-exact expansion to the raster? | Control arm that must fail |
|---|---|---|
| linear octree, integer leaf values | **yes**, on CPU and GPU | change one count in a leaf: the expansion must differ |
| 64-tree or 8³-brick re-pack of that octree | **yes**, a pure re-layout: round trip octree → pack → octree must give identical bytes | swap two children |
| DAG of integer values | **yes**: expand and compare | merge two subtrees that differ in one voxel |
| any float-valued or quantised mass sidecar | no: bit-exactness is lost by quantisation, and unique floats defeat sharing | n/a |
| NanoVDB, SPGrid | expansion yes if values are stored exactly; not tested here | n/a |
| moments | no, numerical: node moments vs the sum over its leaves ≤ 1e-13 on the CPU in `f64`; float-float vs `f64` ≤ 1e-9 on the GPU | zero a moment term |
| merge | numerical: merged = unmerged ≤ 1e-12 | merge across a material boundary |

The integer value is the enabling decision: it makes the strongest test in the plan apply to every
re-layout, and gives the DAG, the 64-tree and the brickmap a free correctness proof against the octree.
It is **[mine]** and it changes one line of S3's brief: the raster voxeliser (the new one, in `cav-core`)
must define mass as `count × unit`, and "`Σm = M` exactly" can then only mean the canonical Morton-order
`f64` sum equals `M` to rounding, since a float total cannot be bit-exact after a rescale. Record the
choice in the slice notes.

## 8. Recommendation

| Job | Structure | Notes |
|---|---|---|
| (1) per-object volume | linear octree, `u32` Morton keys (≤ 1024³ per body), integer leaf values + palette, node moments | lossless merge; edit = bulk rebuild; split components into new bodies as the plan says. Ray marching of a body in its oriented box starts from the octree; do not add a second structure yet |
| (2) world volume | sparse hash of **chunks of 32³**, each *the same tree type*, plus a pyramid of chunk-level nodes carrying moments | static terrain: one tree per chunk, rebuilt when it changes. A dense 32³ array is a cache derived for the fluid and the renderer, never the source of truth. 128³ chunks cover 4096³ cells; chunk table is a `u32`-keyed hash |
| (3) instrument walk | the above, walked top-down from the pyramid through chunk trees, octree granularity, differential-first, stack in registers | the bound and LOD from S5 decide the cost; layout is a constant factor (≤ 3×) |

Order of work: **S3** builds the tree, the integer leaf values, the merge and the raster oracle; no GPU.
**S4** adds the chunk table, the chunk pyramid and the renderer, with a measured decision on a 4³ re-pack
(memory-latency vs the 2–3× arithmetic penalty above). **S5** uploads the arrays, decides the moment cut,
and measures the walk. Revisit DAGs only for S4's far-field render cache if streaming bandwidth is the
limit.

Deviations from the plan, for the slice notes: integer leaf values (§1.1); moment record size and
precision tiering (§5); chunk = same tree type, with dense 32³ demoted to a derived cache (§8).

## 9. What I could not verify

- **Every paper's content.** Only summaries. Specifically unconfirmed: NanoVDB's node sizes and 5-4-3
  layout; ESVO's descriptor layout and bytes per voxel; Kämpe's node format; SPGrid's page size and
  addressing limit; 64-tree traversal speed; the Aokana and Hybrid Voxel Formats numbers. Re-run with
  `arxiv.org`, `research.nvidia.com`, `www.openvdb.org`, `dl.acm.org`, `gpuweb.github.io`, `docs.rs`,
  `www.cse.chalmers.se` and `dubiousconst282.github.io` allowed.
- **All GPU performance:** ray-march and walk times, divergence cost, the effect of 64-way nodes on
  latency. Needs the build PC. Counts in §4.2 are work, not time.
- **Spec limits,** the default `maxStorageBuffersPerShaderStage` and `maxBindGroups`, and the real
  adapter limits on the GTX 1660 SUPER. `cav doctor` should print `adapter.limits()` in S0.
- **CPU build and rebuild times** of the tree for 10⁶–10⁷ leaves; the design's numbers are from Cornerstone
  on an A100.
- **Realistic scenes:** bodies with many materials, rooms, thin shells and air-filled interiors will have
  more leaves than the two analytic scenes; no mesh voxeliser was involved. My storage model's 64-tree
  value-slot count and VDB sizes are approximate.
- **The moment-precision argument** (§5) is derived from the bound, not measured.
- **Odd depths** (11) are in `results/` but not in the tables: the 64-tree accounting needs even depth.

## 10. Further questions (not chased)

1. How many internal nodes are ever accepted at `tol` ∈ {10⁻⁶, 10⁻⁸, 10⁻¹⁰} on a real scene? Decides the
   moment cut (§5). A CPU measurement in S3–S5.
2. Does a 64-bit-mask packing with octree-granularity acceptance beat plain octree on the GPU? S5 benchmark.
3. Cost of the bulk rebuild on CPU for the stamp of moving bodies onto the world volume (S4, S7).
4. Do per-voxel materials inside bodies (tissue, steel) fragment the merge enough to matter? Needs a
   mesh voxeliser with real materials (S3).
5. A bound on the differenced phase (`voxel.md` §13) would shrink the walk far more than any layout.

## Files

- `s3-sparse-voxel-structures/storage_model.py`: the accounting model (run `python3 -I storage_model.py sphere 10 --dag --walk`).
- `s3-sparse-voxel-structures/make_tables.py`: builds the storage table from `results/`.
- `s3-sparse-voxel-structures/results/`: raw output for sphere and terrain at depths 10–12.
