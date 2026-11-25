# Voxel engines: compression, meshing, GPU-driven rendering, ray marching and streaming

*Research note for slices S5 (world octree and the gravity read) and S21 (the look). Written 25 Nov 2026.
It covers only the gaps left by [`s3-sparse-voxel-structures.md`](s3-sparse-voxel-structures.md),
[`s5-world-octree.md`](s5-world-octree.md), [`voxel-lighting.md`](voxel-lighting.md), `docs/slices/S3.md`,
`docs/slices/S4.md` (§7.9 to 7.10) and `plan/REBUILD.md`; read those first. No build was run. One small
Python benchmark was (§7).*

**How sources were read.** Unlike the S3 note, the network worked this time. I downloaded and read the
full text (via `pdftotext`) of: Kämpe et al. 2013, Villanueva et al. 2016 (SSVDAG), Careil et al. 2020
(HashDAG), Museth 2013 (VDB), Laine and Karras's NVR-2010-001 report (ESVO), Amanatides and Woo 1987, and
the Aaltonen/Haar 2015 slides. Web pages (0fps, the binary-greedy-meshing README, wgpu and zfp docs,
gpuweb issues, the Aokana paper) were read through a page-summarising fetch tool, so numbers taken from
them are second-hand and marked *(page summary)*. Anything I could not open is listed in §9 and §11.

---

## 1. One-page summary

**Bottom line.** Cavendish's storage and renderer are already close to what the literature recommends for
its constraints (exact integer leaves, edits, the web build). The literature mostly *confirms* S4's
choices and offers three cheap additions and a short list of things to measure before building anything
bigger.

1. **Lossless only for anything that feeds mass.** Palette + bit-packing, run-length coding and
   content-hash deduplication are exact, cheap to edit and trivially decodable in WGSL. Lossy coders
   (ZFP, SZ) and DAG-with-lossy-LOD are out for the leaf/mass path: ZFP's lossless "reversible" mode
   exists, but it is the one mode with no random-access guarantee and no CUDA/GPU path
   ([zfp docs](https://zfp.readthedocs.io/en/latest/modes.html), *page summary*), and categorical
   material data compresses better with a palette anyway.
2. **A DAG is a measurement away, not a decision.** Published ratios (28× to 576× fewer nodes,
   [Kämpe 2013](https://www.cse.chalmers.se/~uffe/HighResolutionSparseVoxelDAGs.pdf); 0.123 bit/voxel,
   [SSVDAG](https://www.crs4.it/vic/data/papers/i3d2016-symmetry-dags.pdf)) are for *binary occupancy*
   of static scanned or modelled scenes. Cavendish's leaves carry a coverage count and material, and its
   world is edited. **One pass that hashes every node and counts unique ones per level** settles whether
   a DAG would pay (S5, ten lines; §2.5). Until then: no DAG.
3. **Moments dedupe by content, even without a DAG.** Because a node's summary relative to its own
   centre depends only on its contents, two identical subtrees must have bit-identical summaries
   (§2.6, my derivation). Computing summaries once per distinct content hash cuts S5's build cost and
   gives a free test of determinism (equal content, equal bytes; control: one cell changed).
4. **Meshing is not the bottleneck; draw cost and memory are.** The bit-row mesher already makes a 32³
   chunk in about 0.08 ms ([S4 §7.10](../slices/S4.md)), the same order as the 50–200 µs the
   [binary-greedy-meshing](https://github.com/cgerikj/binary-greedy-meshing) author reports for 64³
   chunks *(page summary)*. A GPU mesher or mesh shaders buy nothing here. What is wasteful is the
   **vertex format**: 4 × 12 B vertices + 6 × 4 B indices = **72 B per quad** now, against **8 B per quad**
   with vertex pulling (§3.2): 9× less mesh memory and upload, and room for the AO byte S21 needs.
5. **Web limits shape the GPU-driven design.** WebGPU has `drawIndirect` and `drawIndexedIndirect` but
   no multi-draw and no draw count ([gpuweb #1354](https://github.com/gpuweb/gpuweb/issues/1354),
   open since 2021); no sparse/virtual resources ([#455](https://github.com/gpuweb/gpuweb/issues/455));
   no mesh shaders, bindless arrays, or 64-bit integers
   ([wgpu `Features`](https://docs.rs/wgpu-types/latest/wgpu_types/struct.Features.html), *page summary*).
   Aaltonen and Haar shipped a GPU-driven renderer with **two `DrawInstancedIndirect` calls and no
   multi-draw** ([slides](https://advances.realtimerendering.com/s2015/aaltonenhaar_siggraph2015_combined_final_footer_220dpi.pdf)),
   so the missing features are not blockers; and they report Hi-Z/cluster culling gave **under 10 % of
   geometry cost** back, so Hi-Z is not a first move for the meshes.
6. **The far-field march is the thing to attack for S21**, not the meshes. S4's own measurement says
   the bricks' `frag_depth` is what the 99th percentile pays for. The two levers in the literature are
   tile/beam pre-passes ([ESVO beam optimisation](https://nvidia.com/docs/io/88972/nvr-2010-001.pdf);
   [Aokana](https://arxiv.org/html/2505.02017v1)'s 8×8 tiles with previous-frame Hi-Z) and ordering
   front to back. First measure brick fragment overdraw (§8, S21-2).

**Ranked recommendations** (value for effort; detail and measurements in §8):

| # | For | Do | Effort | Why |
|---|---|---|---|---|
| 1 | S5 | Count unique nodes per level by content hash over `town`, `stress-1e10`, the planet | hours | Settles DAG/dedup for good, with a known answer |
| 2 | S5 | Compute node summaries once per content hash; assert equal content gives equal bytes | days | Build cost ÷ duplication; a determinism test with a control arm |
| 3 | S21 | Measure brick overdraw (fragments marched ÷ pixels) and front-to-back order | hours | Finds the actual cost of the 99th percentile |
| 4 | S21 | 8-byte quads by vertex pulling, AO in the same word | days | 9× mesh memory/upload; AO needs a byte per quad |
| 5 | S21 | Request `indirect-first-instance` to replace per-chunk dynamic-uniform offsets (if browsers expose it) | hours | Fewer binds; verify support first |
| 6 | S21 | Hi-Z (previous-frame pyramid) for **brick boxes**, not meshes | days | Gains are in the march, not the meshes |
| 7 | later | Tile/beam compute march for the far field | weeks | Largest change; only if 3 and 6 are not enough |
| – | reject | GPU meshing, mesh shaders, multi-draw-indirect-count as a dependency, NanoVDB, lossy coders on mass | | §2, §3, §4 |

---

## 2. Voxel data compression

### 2.1 What Cavendish must keep

From the plan and S3: a leaf is a coverage count (≤ 255) and a palette index; mass is count × unit mass
of the material; "expanded tree = raster, bit for bit"; moments are exact up to the stated bound
(`s3-sparse-voxel-structures.md` §1). So the test for any codec is: **decode(encode(x)) = x**, byte for
byte, for the leaf layer, and any summary derived from decoded data equals the one derived from the
original. That excludes anything lossy on the leaf layer, and makes the "lossless merge" of S3
(equality of integers) the natural dedup key.

### 2.2 Per-chunk palette and bit-packing

Cavendish chunks already carry a palette (S4 §1). The literature adds nothing new except the usual
detail: pick bits per cell = ⌈log₂(palette size)⌉, pack into 32-bit words, repack when the palette
grows past a power of two. Edit cost is O(1) per cell except at a repack (O(chunk)); reading on the GPU is
a shift and a mask on a `u32` (WGSL has no `u8`, and no 64-bit integers on the web, see §4.3).
Measured on synthetic worlds in §7: **6–8× against one byte per cell** (more for chunks with ≤ 2
materials), uniform chunks 0 bytes beyond the table entry.

### 2.3 Run-length coding

Along a Morton or x-fastest order, runs of one material are long in terrain and buildings: §7 measures
**23× (noise terrain) to 64× (boxy town)** against raw bytes, beating palette packing by 3–8×. The cost
is edits: changing one cell splits a run and shifts every later byte (O(chunk)), and a GPU reader needs a
prefix sum or per-row index. VDB makes the same trade: its leaf nodes are 8³ dense blocks with a bit
mask for activity, and compression (codecs, bit-quantisation) is applied only at file/stream level, never
in the traversed structure ([Museth 2013](https://www.museth.org/Ken/Publications_files/Museth_TOG13.pdf),
read). Same recommendation for Cavendish: **RLE for region files on disk; palette-packed in memory and on
the GPU.** (This is how S4's region cache can shrink without touching hot paths; whether S4's region
format already does this I did not check.)

### 2.4 Brick dictionaries and chunk dedup

Replace each 8³ brick (or whole chunk) by an index into a table of distinct bricks, keyed by a content
hash. It is the "DAG without the tree": exact, editable (an edit looks up or inserts the new content and
swaps an index; garbage-collect when the reference count reaches zero), and trivially GPU-readable (one
indirection). On the synthetic town, 65 536 bricks reduce to 17 distinct ones; on noise terrain, 13 604
distinct, a **22×** reduction against raw (§7). Real worlds sit between those. The point to carry is the
measurable one: the distinct fraction is a number one can compute in a pass (§2.5), and it predicts a
DAG's payoff because a DAG is the same idea applied recursively.

### 2.5 Sparse voxel DAGs, SSVDAGs and HashDAGs

| Source | Idea | Numbers (from the paper text I read) | Limit |
|---|---|---|---|
| [Kämpe, Sintorn, Assarsson 2013](https://www.cse.chalmers.se/~uffe/HighResolutionSparseVoxelDAGs.pdf) | Merge identical subtrees bottom-up: sort each level's nodes by their child pointers (256-bit key), compact, patch the parents' pointers; 4³ leaf subtrees held as a 64-bit integer | Epic Citadel at 128K³, 19 billion voxels, **945 MB** against 5.1 GB for a pointerless SVO; node count reduced 28× (HAIRBALL) to 576× (CRYSPONZA, 64K³); 8 to 36 B per node; up to 38× less memory than ESVO | "Our focus has not been on dynamic scenes"; geometry only, no materials |
| [Villanueva, Marton, Gobbetti 2016 (SSVDAG)](https://www.crs4.it/vic/data/papers/i3d2016-symmetry-dags.pdf) | Also merge subtrees equal up to a mirror in x, y or z (3-bit tag in each pointer), per-level variable-width pointers, 64-bit leaf bricks | PowerPlant 64K³, ~6 billion voxels in **< 86 MB** (0.123 bit/voxel) against 167 MB for the SVDAG and 16.2 GB for a SVO; reflections cost 14–16 % tracing time over a plain SVDAG | Static; geometry only |
| [Careil, Billeter, Eisemann 2020 (HashDAG)](https://raw.githubusercontent.com/Phyronnaz/HashDAG/master/ModifyingCompressedVoxels-main.pdf) | Interactive edits on a DAG: nodes in per-level hash buckets, paged virtual memory over a 32-bit word address space, path-copy on edit, mark-and-sweep garbage collection, a "full subtree" shortcut so large edits scale sub-linearly; colour as a separate compressed array | Epic Citadel (64K)³ = 380 MB of data plus 105 MB (128-word pages) to 550 MB (512-word pages) of paging overhead; edits up to 350 M voxels at (64K)³ and 3 G at (128K)³; software virtual memory costs **1.5× to 2×** in tracing time; measured on an RTX 2080, CUDA | Needs CUDA-style atomics and a manual allocator; I did not find edit latencies as a single number |
| [Fang et al. 2025 (Aokana)](https://arxiv.org/html/2505.02017v1) *(page summary)* | Per-chunk SVDAG (256³-voxel chunks), LOD by merging 8 chunks to one (voxel kept if ≥ 2 of 8 children are), implicit-octree streaming | San Miguel 64K³ in 424 MB VRAM against HashDAG's 3.5 GB; "only about 5 %" of the scene resident; ~6 ms a frame at 64K³ on an RTX 3060 Ti | **No runtime editing**, no transparency, colour LOD artefacts; the authors avoid lossy compression "because voxel values may carry gameplay meaning" |

What follows for Cavendish:

* **Scale is not the issue.** 10¹¹ cells is 4.6K³; these papers reach 64K³ to 128K³ in under a gigabyte.
  The question is entirely about *what is stored* and *whether it is edited*.
* **The published ratios are for binary geometry of authored scenes.** A voxel's coverage count (0 to 255
  of k³ samples) is what makes the boundary leaves of Cavendish unique: partial-fill surface cells dedupe
  far less than occupancy bits. Interior and uniform regions already cost nothing (kind: uniform). So the
  expected win is on repeated structures (a tiled town, repeated floors) and in the far levels.
* **Editing** is the DAG's weak point: HashDAG shows it can be done, with path-copy, a hash table, a
  garbage collector, and 1.5–2× slower reads from the software paging. Aokana, the most recent
  large-scale GPU renderer, simply does not edit. Cavendish edits constantly (bodies stamped into the
  world each step), so a DAG could only hold the *static, quiescent* world (terrain, buildings that
  never move), as the plan already suggests (`REBUILD.md`, "Atomontage / sparse voxel DAGs" row).
* **Cheap, decisive measurement (S5, recommended):** build the world octree as now; then, per level,
  hash each node as `H(node kind, children's hashes or leaf bytes)` and count distinct hashes against
  node count. The ratio at each level is the DAG's reduction, computed on Cavendish's real data with
  Cavendish's real leaves, in one pass, no new structure. If distinct/total at levels ≥ 1 is above
  ~0.3, stop; if it is below ~0.05 on a scene the owner cares about, a far-field DAG is worth a slice.
  (The thresholds are my judgement; the node-size arithmetic is in §2.6.)

### 2.6 What a DAG does to moments (my derivation; needs the S5 test)

S3 flagged that moments depend on position (`s3-sparse-voxel-structures.md` §4.3). Stored **relative to
the node's own centre and in units of its own cell pitch**, a node's mass, centre-of-mass offset and
higher moments are a pure function of its contents. Two nodes with identical contents at the same level
therefore have identical summaries, bit for bit, *provided the summary is computed by the same
operations in the same order* (the Morton-order commit of S5 already fixes this). Consequences:

* A summary can be computed once per **distinct content hash** and shared, with or without a DAG.
  Build cost and sidecar memory fall by the duplication factor. For a DAG, the 88 B of moments per
  internal node (`s3-sparse-voxel-structures.md` §5) are shared exactly like the topology, so the
  topology saving is not swamped by moment storage.
* The world position enters only at read time, as the node-to-tower vector, which the walk already
  has.
* If moments are held as **integer sums** (count × integer unit mass × integer cell offset to a power
  ≤ 4, which fits easily in `i64`), they are exact and order-independent, so "bit for bit" holds without
  depending on summation order. This needs per-material integer unit masses, which is a design choice
  for S5 (otherwise sum per material, which is heavier); on the GPU, WGSL has no `i64`, so the GPU would
  use float-float as now. I have not tested any of this.
* Test with a control: build two identical blocks at different positions and check equal summaries
  bytes; change one cell and require a difference.

### 2.7 OpenVDB, NanoVDB and GVDB

* **OpenVDB** ([Museth 2013](https://www.museth.org/Ken/Publications_files/Museth_TOG13.pdf), read): a
  B+tree-like hierarchy of large, fixed-depth, power-of-two branching factors (the paper's tests use
  configurations such as [Hash,5,4,3]: branches of 32 × 16 × 8 = 4096 cells), average O(1) random access
  by direct-access tables with bit masks, dynamic topology, leaf nodes dense. It is the right tool for
  simulation grids and was designed for them; the fluid slices (S8, S9) are the natural place to
  borrow its ideas (bit-masked dense leaves, tiles for uniform regions).
* **NanoVDB** ([FAQ](https://www.openvdb.org/documentation/doxygen/NanoVDB_FAQ.html), *page summary*):
  a linearised, pointer-free, contiguous copy of an OpenVDB tree with min/max and active-bounding-box
  per node, "assumes the topology ... to be static", values can change; GPU-ready in CUDA, GLSL, HLSL,
  with WebGL "in progress". No WGSL path, and the per-node statistics are min/max, not multipoles. The
  S3 note's verdict (right library, wrong shape) stands.
* **GVDB / GigaVoxels / brickmaps:** I did not obtain primary text for GVDB or the van Wingerden thesis
  (§9). From the S3 note and the S4 implementation, a brick pool plus an indirection table is what
  Cavendish already has; the literature agrees with it ([Aokana](https://arxiv.org/html/2505.02017v1)'s
  chunk pool is the same shape).

### 2.8 Lossy compression (ZFP, SZ) for density and material fields

* **ZFP** works on 4^d blocks; **fixed-rate** mode gives random access but is lossy; **fixed-accuracy**
  is floats only and lossy; **reversible** gives bit-for-bit reconstruction for integers and floats,
  not yet in CUDA ([zfp modes](https://zfp.readthedocs.io/en/latest/modes.html), *page summary*; paper:
  [Lindstrom 2014](https://doi.org/10.1109/TVCG.2014.2346458), not read). **SZ** is error-bounded
  prediction-based lossy compression of floating-point arrays; the comparison paper
  [Tao et al. 2018](https://arxiv.org/abs/1806.08901) (read at abstract/method level only) describes
  both as lossy coders for HPC floating-point data.
* **Verdict.** Mass comes from integer counts and a palette; there is nothing floating-point to
  compress there, and the lossless route (palette, RLE, dedup) already does well on categorical data.
  The place a lossy coder could earn its keep is **derived floating-point fields stored per cell**
  (temperature, fluid velocity and pressure in S8–S13), where an error bound can be set below the
  field's own discretisation error and recorded in the ledger. That is a different question, for the
  slice that stores such a field; note only that fixed-rate ZFP is the one that supports random access.
  Do not apply to the density field that the gravity read sums.

---

## 3. Meshing

### 3.1 Greedy meshing and its binary variant

* [Lysenko, "Meshing in a Minecraft game" (0fps, 2012)](https://0fps.net/2012/06/30/meshing-in-a-minecraft-game/)
  *(page summary)*: culling gives 8× fewer quads than one cube per voxel on a solid 8³ block; greedy
  merging reaches 6 quads for it; on his examples 4 770 quads (naive culled) to 2 100 for a sphere and
  2 198 to 1 670 for noise terrain. The greedy mesh is proved at most 8× the optimal size (conjectured
  4×); it assumes one block type, so per-voxel attributes (textures, AO) complicate merging; the open
  problem he states is maintaining a greedy mesh dynamically.
* **Binary greedy meshing** ([cgerikj README](https://github.com/cgerikj/binary-greedy-meshing),
  *page summary*): a 64×64 array of `u64` occupancy; face culling with bit operations does 64 faces at
  once; merging also runs in bit operations, with the voxel type looked up to allow a merge; **50–200 µs
  per chunk, 74 µs average single-threaded on a Ryzen 3800X**; chunk ≤ 64³ including neighbour
  padding; **8 bytes per quad** (6-bit x, y, z, width, height, plus 8 bits of type), drawn by vertex
  pulling; quads are drawn about 1 px larger to hide T-junction cracks.
* **Cavendish already has the equivalent.** S4 §7.10: faces found "34 cells at a time as bits", 0.08 ms
  per mesh against 0.67 ms before, 32³ chunks, and the same rectangles as the plain mesher (tested).
  Nothing to gain on the CPU side by switching algorithm. S4's remaining tail was the *rate* (96
  meshes in a frame on one thread), which `MESH_MILLISECONDS` already bounds.
* **Merge keys.** For S21's AO, merging must require equal AO on all four corners
  (`voxel-lighting.md` §3.1). That lowers merge rates; measure the quad-count change on `town` before
  and after (S21 note already lists it).

### 3.2 Compact vertex formats (vertex pulling)

Today `ChunkVertex` is `[u16;4]` + `[u8;4]` = 12 B, four per quad, six `u32` indices per quad:
**72 B per quad** (`crates/cav-render/src/world.rs:168`). A quad record in a storage buffer, expanded by
`vertex_index` in the shader, is 8 B (cgerikj's layout above; the same in [Bevy's vertex-pulling
experiment](https://www.github.com/superdump/bevy-vertex-pulling), *page summary*). For Cavendish a
chunk is 32³, so each of x, y, z, w, h needs 6 bits (0..32 inclusive is 33 values: 6 bits suffice):

| field | bits |
|---|---|
| x, y, z (corner within chunk) | 3 × 6 = 18 |
| width, height (cells) | 2 × 6 = 12 |
| face | 3 |
| AO, four corners × 2 bits | 8 |
| material | 8 to 16 |
| chunk slot (for the camera-relative origin) | 0 if one draw per chunk, else 12 to 16 |

That is 49 to 69 bits: fits 64 with the slot kept out of band (per chunk draw), or in two `u32` words
plus a shared chunk table. **Expected effect:** 9× less mesh memory and upload, no index buffer, AO
included; time effect uncertain, because S4 attributes the draw cost mostly to the bricks, not mesh
vertices (§1 point 6). So rank it for memory and S21's AO packing, not for frame time.

Requirements on the web: read-only storage buffers in the vertex stage (WebGPU defaults allow 8 storage
buffers per stage, 4 bind groups; [W3C WebGPU spec](https://www.w3.org/TR/webgpu/), limits section
*page summary, partly truncated*); not available on the GL-backend downlevel limits, which Cavendish
does not target for the page. **Pitfall:** triangle order and winding come from `vertex_index`, so
backface and AO diagonal flip must be done in the shader (the AO diagonal rule of `voxel-lighting.md`
applies unchanged).

### 3.3 GPU meshing in compute, and mesh shaders

* **Compute meshing.** I found only demos (a Unity GPU voxel mesher that reports ~650 FPS on an RTX 4090
  with counting and writing passes and `DrawProceduralIndirect`, no CPU readback;
  [unityvoxelmeshgpu](https://github.com/unitycoder/unityvoxelmeshgpu), *search-result summary*), and
  no peer-reviewed numbers. Given 0.08 ms CPU meshes, the benefit is only to free the web's single
  thread. The costs are an upload of chunk cells and a compaction pass; **defer** (`plan/IDEAS.md`).
* **Mesh shaders.** wgpu exposes them as `EXPERIMENTAL_MESH_SHADER`: Vulkan (`VK_EXT_mesh_shader`),
  DX12 and Metal, **native only**, with Naga compiling them only for Vulkan
  ([wgpu docs](https://docs.rs/wgpu-types/latest/wgpu_types/struct.Features.html), *page summary*).
  Not available to the page; the equivalent on both is compute culling plus indirect draw (§4).
  **Reject** for Cavendish.

---

## 4. GPU-driven rendering

### 4.1 The pattern

Move culling and draw-argument generation to compute: per-object data in big GPU buffers, a compute
pass tests frustum (and optionally occlusion) and writes indirect arguments or a compacted list, then
a few indirect draws consume it ([vkguide: GPU-driven engines](https://vkguide.dev/docs/gpudriven/gpu_driven_engines/),
*page summary*). Aaltonen and Haar
([SIGGRAPH 2015](https://advances.realtimerendering.com/s2015/aaltonenhaar_siggraph2015_combined_final_footer_220dpi.pdf),
read) add **64-triangle clusters** with per-cluster frustum/occlusion/backface culling and index-buffer
compaction, then draw.

Measured in that talk:

* A "torture" scene of 250 000 moving objects, 1 GB of mesh data, **DirectX 11, no ExecuteIndirect or
  multi-draw, only two `DrawInstancedIndirect` calls**: 2.3 ms GPU on Xbox One at 1080p, 0.2 ms of one
  CPU core. Object culling 0.28 ms, cluster culling 0.09 ms, pyramid 0.06 ms.
* Backface and bounds culling removed 20–40 % of triangles but gave "only small overall gain: <10 % of
  geometry rendering".

Cavendish today: one compute pass writes each chunk's indirect args (count 0 or 1), one
`draw_indexed_indirect` per resident chunk (1.1 to 1.5 thousand resident, S4 §7.9), the chunk origin
by dynamic uniform offset. That is the "one indirect per possible draw, zero the culled ones"
workaround that gpuweb itself describes as the web's answer to missing multi-draw
([#1354](https://github.com/gpuweb/gpuweb/issues/1354), *page summary*).

### 4.2 Occlusion culling (Hi-Z)

[Aaltonen/Haar](https://advances.realtimerendering.com/s2015/aaltonenhaar_siggraph2015_combined_final_footer_220dpi.pdf)
use a **two-phase** scheme: cull everything against *last frame's* depth pyramid, draw what passes,
rebuild the pyramid, retest what was culled and draw the false negatives. Their depth source was 300
best occluders (~600 µs on PS4), downsampled to 512×256, combined with reprojected last-frame depth.
For Cavendish the interesting object is not a mesh chunk (terrain and towns occlude little of
each other at flight altitude and the S4 note already rates Hi-Z low) but the **brick boxes**: each is
a whole ray-marched fragment workload. Aokana does exactly this at 8×8-pixel tile granularity with the
previous frame's Hi-Z and re-tests wrongly culled tiles after rebuilding it
([paper](https://arxiv.org/html/2505.02017v1), *page summary*). A Cavendish version would cull a brick
box when its nearest depth is behind the pyramid's farthest value in its screen rectangle. Expected
gain depends on how much brick fragment work is occluded; **measure that first** (§8, S21-2).

### 4.3 WebGPU against native wgpu

| Capability | Web (WebGPU) | Native wgpu | Source |
|---|---|---|---|
| `draw(_indexed)_indirect`, one draw | yes | yes | [gpuweb #1354](https://github.com/gpuweb/gpuweb/issues/1354) |
| `multi_draw_indirect`, `…_count` | **no**; "chromium-experimental-multi-draw-indirect" behind a flag in Chrome | `MULTI_DRAW_INDIRECT_COUNT`: DX12, Vulkan 1.2+ | [wgpu Features](https://docs.rs/wgpu-types/latest/wgpu_types/struct.Features.html); [Chrome 131 notes](https://developer.chrome.com/blog/new-in-webgpu-131) (search summary) |
| `indirect-first-instance` (instance base from the indirect buffer) | optional feature in the spec's feature list; **browser support not verified** | Vulkan, DX12, Metal | [wgpu docs](https://docs.rs/wgpu-types/latest/wgpu_types/struct.Features.html) |
| Mesh shaders | no | `EXPERIMENTAL_MESH_SHADER`, Vulkan/DX12/Metal | as above |
| Bindless / binding arrays | no | `TEXTURE_BINDING_ARRAY` (DX12, Metal, Vulkan), `BUFFER_BINDING_ARRAY` (Vulkan), partially bound (Vulkan, DX12) | as above |
| 64-bit integers in shaders | no | `SHADER_INT64` (Vulkan; DX12 with DXC; Metal 2.3+) | as above |
| Subgroups | WebGPU has a subgroups feature, wgpu's differs, open issue | Vulkan, DX12, Metal | as above |
| Sparse / virtual resources | no ([#455](https://github.com/gpuweb/gpuweb/issues/455), open, Milestone 4+, "no updates" May 2025) | not in wgpu | issue summary |
| Timestamp queries | yes | yes | wgpu docs |

Consequences for the design: (a) write the GPU-driven path against **single indirect draws and compute
compaction**, which works on both; optionally enable `MULTI_DRAW_INDIRECT_COUNT` natively as a pure
speed-up behind a capability check, never as a dependency (the owner wants the page to run the same
code, `web-build-pages` memory). (b) The one new thing worth checking is `indirect-first-instance`:
with it, the instance index can carry the chunk slot, removing per-chunk dynamic-uniform offsets
(S4 §3.4). S4 says "no `first_instance`: neither is core in WebGPU", which is true; *optional* is not
the same as absent, but I did not verify which browsers expose it. (c) No 64-bit atomics: Aokana's
64-bit visibility buffer with `InterlockedMax` cannot be copied literally; it needs a two-`u32`
scheme (e.g. atomicMax on depth, second pass writes the id where depth matches; mine, untested).

### 4.4 Meshlets / clusters for voxels

Cluster hierarchies (Nanite, [Karis 2021](https://advances.realtimerendering.com/s2021/Karis_Nanite_SIGGRAPH_Advances_2021_final.pdf);
skimmed: its HZB section describes the same two-pass scheme, build the HZB from last frame's visible set, draw, rebuild, retest what was culled; the rest not read) solve the problem of *continuous LOD of arbitrary
triangles*. Voxel worlds get LOD for free from the clipmap (S4) and the octree (S5). The one idea
that transfers is Aaltonen's **fixed-size cluster of 64 quads** as the culling and draw unit, which makes
per-chunk vertex budgets uniform and a single `draw_indirect` over a compacted cluster list possible
(instance = cluster, 6 × 64 vertices per instance, tail quads degenerate). Worth doing only if the CPU
cost of ~1.5 thousand draws is measured to matter on the page (§8, S21-4).

---

## 5. Ray-marched voxels

### 5.1 DDA and empty-space skipping

* **DDA.** [Amanatides and Woo 1987](http://www.cse.yorku.ca/~amana/research/grid.pdf) (read): stepping
  to the next voxel costs "two floating point comparisons and one floating point addition". It is the
  basis of every grid march. Floating-point robustness at large coordinates is Cavendish's issue, not the
  paper's: S4 already marches in camera-relative space.
* **Hierarchical skipping.** S4 uses a 32³ brick with a 4³ mask over 8³ sub-blocks (a 64-tree layout)
  and two-level DDA, with `MAX_MARCH_STEPS`. The DAG papers use ESVO-style octree descent with a
  64-bit leaf, and all stress that *per-ray divergence* is the cost, not memory
  ([Kämpe 2013](https://www.cse.chalmers.se/~uffe/HighResolutionSparseVoxelDAGs.pdf)).
* **Beam optimisation** ([Laine and Karras, NVR-2010-001](https://nvidia.com/docs/io/88972/nvr-2010-001.pdf),
  §6.2, read): render a coarse, conservative distance image first (8×8 pixel blocks), start each ray at
  that distance, skipping the empty space at the beginning. It works because voxels are cubes, so a beam
  can be bounded conservatively; the paper shows iteration counts per pixel falling (its Figure 16).
  It is for *primary* rays; the follow-up for shadows (shoot the beam to the light and back) is in
  Kämpe 2013. Cavendish's far field is rasterised brick boxes with a per-fragment march, which
  already starts each ray at the box's entry; a beam pass would help where **many boxes stack behind each
  other along a ray** (overdraw), not inside a single box.
* **Brickmaps** (van Wingerden 2015, Utrecht MSc): not read (§9). The S3 note already gives the shape
  (two-level grid, dense 8³ bricks, cheap edits, no merge); Cavendish's brick pool is that.

### 5.2 Hybrid: raster near, trace far (what Cavendish does) and its pitfalls

[Aokana](https://arxiv.org/html/2505.02017v1) (*page summary*) does the same: raster/mesh pipeline for
near content, voxel passes inserted between the opaque and transparent passes, composed through depth.
The pitfalls I can support from sources, and those that are standard but unsourced here:

1. **Early-Z loss.** Writing depth from the fragment shader (`frag_depth`) disables early depth
   rejection for those fragments; S4 §7.7 already says it is the 99th percentile's cost. I did not
   find *conservative depth* output (depth-greater-equal) in WGSL; if absent, the cheap fix is to
   rasterise brick boxes **after** the near meshes and front to back, so the box's own depth test
   rejects hidden boxes before the fragment shader runs (box depth as the interpolated position,
   fragment depth only for what survives). Unverified for WGSL/wgpu; measure.
2. **Overdraw of boxes.** Every box a ray crosses runs the march. Front-to-back ordering plus Hi-Z on
   boxes (§4.2) or a tile pre-pass (§5.1) are the two published mitigations.
3. **Seams between representations.** S4's nested level boxes and S5's dithered cross-fade address
   this; nothing new from the sources.
4. **Consistency of shading** between mesh and march (AO, shadows): S21's own premise ("near and far
   agree", `voxel-lighting.md`).
5. **Depth precision** over a 5 km far plane with a 1 m near plane: reversed-Z with a float depth buffer
   is the standard remedy (not verified in a source here; `Depth32Float` availability on the web is
   standard but I did not check S4's current depth format).
6. **Colour/LOD averaging** is what Aokana reports as its LOD artefact (colours averaged across 8
   children); Cavendish's LOD is a measured table with a recorded error, so this applies to *colour*
   only (S21's data look keeps map colours exact).

---

## 6. Streaming and memory

* **Uploads.** wgpu's `Queue::write_buffer` copies into a *new* staging allocation per call, scheduled
  at the next `submit`, freed after it completes; `write_buffer_with` writes straight into that staging
  memory (saves a copy); and `StagingBelt` recycles a ring of staging buffers and is "most useful for
  writing many small pieces of data" ([Queue docs](https://docs.rs/wgpu/latest/wgpu/struct.Queue.html),
  [StagingBelt docs](https://docs.rs/wgpu/latest/wgpu/util/struct.StagingBelt.html), both *page summary*).
  S4 §7.10 says uploads are "a few hundred vertices" and 4 KB bricks, not measured to matter: a belt
  adds nothing until a flythrough trace shows upload time, which the trace can record.
* **Residency.** Aokana's rule is a screen-space error on an implicit octree per chunk,
  `(ChunkSize × StreamingFactor) − distance`: positive means subdivide, negative means load; higher LODs
  load first so their children can be freed; only ~5 % of the scene resident. S4's clipmap rings with
  stand-ins (§7.9) already do this and fixed the black-chunk bug; there is nothing to add except
  that S5's pyramid will want *the tower's cut resident too* (already planned).
* **Page tables, virtual textures for voxels.** GPU-driven engines use virtual texturing with a page
  cache and indirection (Aaltonen and Haar: a 256K² virtual atlas, 128² pages, 8K² cache, with
  [slides](https://advances.realtimerendering.com/s2015/aaltonenhaar_siggraph2015_combined_final_footer_220dpi.pdf)).
  Hardware sparse resources are not on the web (§4.3) and one gpuweb discussant argues indirection
  tables "perform better" anyway ([#455](https://github.com/gpuweb/gpuweb/issues/455), *page summary*).
  HashDAG, forced to implement its own paging because CUDA lacked sparse resources, pays **1.5× to 2×**
  traversal time for software paging and hopes hardware sparse resources would cut it
  ([HashDAG](https://raw.githubusercontent.com/Phyronnaz/HashDAG/master/ModifyingCompressedVoxels-main.pdf)).
  **For Cavendish:** keep the brick pool with an explicit slot indirection (as S4 does); do not build
  a second paging layer.
* **Disk.** Aokana's disk footprint is *larger* than HashDAG's because of per-LOD storage (23 GB for San
  Miguel 64K³ against 424 MB of VRAM): LOD levels on disk are cheap compared with VRAM, which supports
  S5's choice to store levels explicitly.

---

## 7. A small benchmark: lossless chunk storage on two synthetic worlds [M]

File: `C:\Users\Georg\AppData\Local\Temp\claude\06e6b27cf58b2ff3\29333d60-b123-4fc9-a131-a04033e79cc8\scratchpad\bench\chunk_storage.py`
(Python 3.14, NumPy; ~10 s). **Pass criterion written first:** every encoding decodes to the identical
array (asserted for the palette packer; RLE and dedup count bytes only and are exact by construction).
Synthetic: 512×512×128 cells = 33.6 M cells in 32³ chunks, material 0 = air. **"town"**: soil layers and
24×24 hollow boxes on a 56-cell grid with 3 to 5 storeys and a window band (very regular: an upper
bound on dedup). **"terrain"**: six random plane waves, layered materials (soil, topsoil, rock). Neither
has partial-fill coverage counts, so real worlds will dedupe less at surfaces. A uniform chunk is counted
as 0 bytes beyond its table entry (in RLE, as 1 byte).

| Encoding (lossless) | town | terrain |
|---|---|---|
| raw `u8` per cell | 33.55 MB | 33.55 MB |
| uniform chunks (share) | 56 % | 49 % |
| palette + bit-pack per chunk | 4.23 MB (7.9×) | 5.24 MB (6.4×) |
| run-length, best of RLE and palette per chunk | 0.52 MB (64×) | 1.44 MB (23×) |
| palette + exact dedup of whole chunks | 1.16 MB (29×) | 5.24 MB (6.4×) |
| 8³ brick dictionary (index 2 B per brick slot) | 0.13 MB (253×), 17 distinct of 65 536 | 1.49 MB (22.5×), 13 604 distinct |

**Reading it.** Palette packing is a safe 6–8× at zero edit cost; RLE and dedup are very scene
dependent (regular buildings versus smooth terrain). The decision-relevant number is the *distinct
fraction*, and it is a pass over real data to obtain. What it does **not** show: edit cost, GPU read
cost, or anything about the coverage-count leaves. Extend it by pointing it at an exported chunk dump of
`town` (no exporter exists that I know of; the engine's region files are the natural input).

---

## 8. Recommendations for S5 and S21, with what to measure

### S5 (the world octree and the gravity read)

**S5-1. Count distinct nodes per level (hours).** Hash `(kind, child hashes | leaf bytes)` for every node
of `town`, `world-box`, `stress-1e10` and the planet world; report distinct/total per level and the
bytes a DAG with S5's real node record (topology 8 B + summary ~88 B) would save. *Pass:* a number per
level, recorded in the slice note with the decision rule of §2.5. *Control:* a world filled with
random leaves must show distinct/total ≈ 1.

**S5-2. Content-hash the summaries (days).** Compute each node's summary once per distinct content hash
(relative to the node centre, in cell-pitch units, Morton order). *Tests:* identical blocks at two
positions give byte-identical summaries; one changed cell changes the bytes (control); the incremental
edit path equals a full rebuild bit for bit (S3's R1). *Measure:* summary-build time and sidecar bytes
before and after on `town`.

**S5-3. Keep leaves lossless; lossy coders only for derived float fields, later.** Nothing to build now.

**S5-4. Region files: RLE or palette+RLE on disk, palette-packed in memory.** Check what S4's region
format does; if it stores raw palettes, §7 says 6–64× is available. *Measure:* region file bytes per
chunk and decode time for `town`.

**S5-5. Do not store the dynamic world as a DAG.** If S5-1 says a far-field DAG pays, it is a
read-only derived structure for static levels ≥ 1, rebuilt on edit of its inputs (Aokana-style, no
editing), not the world's store. HashDAG shows editing is possible but costs a hash table, a garbage
collector and 1.5–2× in reads.

### S21 (the look)

**S21-1. Merge key and quad budget.** Before and after adding AO to the greedy merge: quads per chunk on
`town`, and mesh time (S4's 0.08 ms is the baseline). Uses existing tests (`a_greedy_mesh_covers_the_exposed_faces`).

**S21-2. Brick overdraw (hours).** Count, per frame, brick-box fragments entering the march versus
pixels covered, and the fraction of march steps whose result is hidden by nearer geometry (a debug
counter buffer in the existing kernel twin). *Decision:* if mean overdraw is below ~2 and hidden
fractions below ~20 %, stop; otherwise do S21-5/6. (Thresholds are mine.)

**S21-3. 8-byte quads (days).** Pack as §3.2 including 4×2-bit AO. *Tests:* decode(encode(quad)) equals
the quad for all legal values (control: width 33 rejected); the pulled mesh draws pixel-identical to the
indexed one on `town` (golden image). *Measure:* mesh bytes per chunk (expect 9×), upload bytes, draw
time (may not move).

**S21-4. Draw-call cost on the page.** Measure CPU encode time of ~1.5 K `draw_indexed_indirect` calls
in a render pass on the web build (timestamp queries do not see CPU). Only if it exceeds ~1 ms, move to
the 64-quad-cluster draw of §4.4.

**S21-5. `indirect-first-instance`.** Query the feature in the adapter on Chrome, Firefox, Safari-TP;
if present, use instance base = chunk slot and drop the dynamic offsets (S4 §3.4). Keep the current path
as the fallback.

**S21-6. Hi-Z for brick boxes (days).** Previous-frame depth pyramid, test each box's screen rectangle,
two-phase retest of culled boxes after the pyramid rebuild. *Measure:* pyramid pass time (Aaltonen:
0.06 ms at 1080p on Xbox One for the whole pyramid), boxes culled, march time saved, false-negative
rate. *Control:* camera cut (pyramid invalid) must not lose any brick (reuse S4's
`a_moving_camera_never_loses_what_it_drew`).

**S21-7. Tile/beam march (weeks).** Only if S21-2 shows heavy overdraw that S21-6 cannot cure.

**Rejected, with reasons:** GPU meshing (CPU mesh 0.08 ms; benefit only on the web's single thread);
mesh shaders (native only); multi-draw-count as a dependency (web lacks it); bindless (native only,
unneeded: the brick pool is one buffer); NanoVDB (static topology, min/max not multipoles, no WGSL);
lossy compression on mass.

---

## 9. What I could not verify

* **Browser support for `indirect-first-instance`** and for WebGPU subgroups; only the wgpu docs'
  statement that WebGPU lists the former.
* **Most of the Nanite talk** (only its occlusion section was skimmed), **Hi-Z details
  beyond the 2015 slides and Nanite's**, and **mesh-shader meshlet performance**; I used only the slides above.
* **GVDB, GigaVoxels, brickmap (van Wingerden) and the Utrecht thesis**: the PDF URL I guessed
  returned an HTML page; the S3 note's descriptions are the only source for these.
* **WebGPU default limits beyond three:** the spec fetch truncated before the buffer-size limits; the
  S3 note §6 covers flat-storage limits and should be taken as the reference.
* **Numbers from "page summary" sources** (0fps, binary-greedy-meshing, Aokana, wgpu and zfp docs,
  gpuweb issues, NanoVDB FAQ) were extracted by a summarising fetch, not read by me in full.
  Aokana's table in particular should be checked against the paper.
* **Whether S4's region format already compresses**, and what `ChunkCells` stores exactly (palette
  width); I read only the slice notes and `ChunkVertex`.
* **Edit latencies of HashDAG as a single number**: the paper presents them as plots (its Figures 7 to
  9) which `pdftotext` does not extract.
* **My own derivations** (§2.6 moment dedup, integer moments, the 64-bit visibility-buffer workaround,
  the decision thresholds in §8) are untested.
* **The §7 benchmark is synthetic**; it supports only relative statements.

## 10. Further questions (not chased)

1. Do the coverage-count boundary leaves dedupe at all? (S5-1 answers it on real data.)
2. A far-field-only DAG with float-float moments on the GPU: node record size and walk cost.
3. Compute-shader mesher for the single-threaded page: worth it only after S21-4.
4. Conservative depth in WGSL/wgpu (`depth_greater`-style qualifiers): does it exist, per backend?
5. Reversed-Z and the 5 km far plane: current depth format and precision at the horizon.
6. Time-varying voxel data coding ([Kämpe et al.](https://doi.org/10.1145/2461912.2462024) cites related work), for replayed scenes.
7. Whether ZFP fixed-rate is right for the S8 to S13 float fields, with an error bound in the ledger.

## 11. Papers I could not read

Each is listed with the reason, as a DOI link where one exists. All DOIs except the ones marked
"search result" are from my memory of the citation and **should be checked when fetched**.

| Paper | DOI / link | Why it matters |
|---|---|---|
| Lindstrom, "Fixed-Rate Compressed Floating-Point Arrays", IEEE TVCG 2014 | [10.1109/TVCG.2014.2346458](https://doi.org/10.1109/TVCG.2014.2346458) (from a search result) | ZFP's design and error behaviour (§2.8) |
| Museth, "NanoVDB: A GPU-Friendly and Portable VDB Data Structure for Real-Time Rendering and Simulation", SIGGRAPH Talks 2021 | [10.1145/3450623.3464653](https://doi.org/10.1145/3450623.3464653) | Node layout and sizes; only the FAQ was read (§2.7) |
| Crassin, Neyret, Lefebvre, Eisemann, "GigaVoxels", I3D 2009 | [10.1145/1507149.1507152](https://doi.org/10.1145/1507149.1507152) | Brick pool and streaming by ray demand (§2.7, §6) |
| Laine and Karras, "Efficient Sparse Voxel Octrees", I3D 2010 (the technical report was read, not the I3D version) | [10.1145/1730804.1730814](https://doi.org/10.1145/1730804.1730814) | Published numbers on beam optimisation and contours |
| Molenaar and Eisemann, "Transform-Aware Sparse Voxel Directed Acyclic Graphs", ACM TOG 2025 | [10.1145/3728301](https://doi.org/10.1145/3728301) (from a search result) | Generalises SSVDAG symmetries to more transforms; could raise dedup on rotated/translated repeats |
| Fang, Wang, Wang, "Aokana: A GPU-Driven Voxel Rendering Framework for Open World Games", PACMCGIT 2025 (read as a web page summary only) | [10.48550/arXiv.2505.02017](https://doi.org/10.48550/arXiv.2505.02017) (arXiv DOI; the journal DOI I do not have) | The closest published system to S4/S21; check Table 1 and the pipeline |
| Villanueva, Marton, Gobbetti, "Symmetry-aware Sparse Voxel DAGs (SSVDAGs) for compression-domain tracing of high-resolution geometric scenes", JCGT 6(2), 2017 (the i3D 2016 paper was read) | no DOI; <https://jcgt.org/published/0006/02/01/paper.pdf> | Longer treatment: build algorithm and out-of-core details |
| Kämpe, Sintorn, Assarsson, "High resolution sparse voxel DAGs", SIGGRAPH 2013 (read from the author's preprint; the published version may differ) | [10.1145/2461912.2462024](https://doi.org/10.1145/2461912.2462024) | Confirm numbers against the ACM version |
| Careil, Billeter, Eisemann, "Interactively Modifying Compressed Sparse Voxel Representations", CGF 2020 (read from the authors' repository copy; figures not extracted) | [10.1111/cgf.13916](https://doi.org/10.1111/cgf.13916) | Edit timings in Figures 7 to 9 |
| Museth, "VDB: High-Resolution Sparse Volumes with Dynamic Topology", ACM TOG 2013 (read from the author's PDF) | [10.1145/2487228.2487235](https://doi.org/10.1145/2487228.2487235) | Confirm the default node configuration |
| Tao, Di, Liang, Chen, Cappello, "Optimizing Lossy Compression Rate-Distortion from Automatic Online Selection between SZ and ZFP", IEEE TPDS 2018 (read at method level only) | [10.48550/arXiv.1806.08901](https://doi.org/10.48550/arXiv.1806.08901) | SZ vs ZFP behaviour; journal DOI not known to me |
| Di and Cappello, "Fast Error-bounded Lossy HPC Data Compression with SZ", IPDPS 2016 | [10.1109/IPDPS.2016.11](https://doi.org/10.1109/IPDPS.2016.11) | The SZ primary source |
| Sifakis et al., dynamic-topology NanoVDB grids, 2026 (cited in the S3 note) | no DOI found: <https://research.nvidia.com/labs/prl/sifakis2026dynamic/dynamic_grids.pdf> | GPU-native topology updates |
| van Wingerden, "Real-time Ray Tracing and Editing of Large Voxel Scenes", MSc thesis, Utrecht 2015 | no DOI: <https://hdl.handle.net/20.500.12932/20460> | Brickmap editing and streaming |
| Karis, "Nanite: A Deep Dive", SIGGRAPH 2021 Advances (skimmed, not read through) | no DOI: <https://advances.realtimerendering.com/s2021/Karis_Nanite_SIGGRAPH_Advances_2021_final.pdf> | Cluster hierarchy, visibility buffer and two-pass HZB at scale |
| Amanatides and Woo, "A Fast Voxel Traversal Algorithm for Ray Tracing", Eurographics 1987 (read) | no DOI known to me | included for completeness |
