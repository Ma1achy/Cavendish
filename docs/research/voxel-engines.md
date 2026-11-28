# Voxel engines: compression, meshing, GPU-driven rendering, ray marching and streaming

*Research note for slices S5 (world octree and the gravity read) and S21 (the look). Written 25 Nov 2026;
revised the same day from the primary texts (see "Changed after reading the primary sources" below).
It covers only the gaps left by [`s3-sparse-voxel-structures.md`](s3-sparse-voxel-structures.md),
[`s5-world-octree.md`](s5-world-octree.md), [`voxel-lighting.md`](voxel-lighting.md), `docs/slices/S3.md`,
`docs/slices/S4.md` (§7.9 to 7.10) and `plan/REBUILD.md`; read those first. No Cavendish build was run.
Two small Python benchmarks were (§7).*

**How sources were read.** The first pass read, as full text via `pdftotext`: Kämpe et al. 2013, the SSVDAG
i3D paper, Careil et al. 2020 (HashDAG), Museth 2013 (VDB), Laine and Karras's NVR-2010-001 report, Amanatides
and Woo 1987 and the Aaltonen/Haar 2015 slides. The second pass (this revision) read the PDFs the owner
fetched, again via `pdftotext`, and cites them by **printed page, section or table**: Kämpe 2013 (ACM TOG
32(4), article 101); Villanueva, Marton and Gobbetti, JCGT 6(2) 2017 (the journal version of SSVDAG);
Careil 2020 (CGF 39(2) 111–119); Molenaar and Eisemann 2025 (the transform-aware SVDAG); Fang et al. 2025
(Aokana); van Wingerden's 2015 Utrecht MSc thesis (brickmaps); Crassin et al. 2009 (GigaVoxels); Laine and
Karras, I3D 2010 (ESVO); Museth 2013 (VDB) and 2021 (NanoVDB talk, 2 pages); Sifakis et al. 2026 (dynamic
NanoVDB); Lindstrom 2014 (ZFP); Di and Cappello 2016 (SZ); Tao et al. 2018/19 (SZ against ZFP); and the
Karis Nanite deck (155 slides: I read the slides on voxels, culling, the visibility buffer and software
rasterisation, not the build, LOD or streaming sections). Where a page number is a *printed* folio I say
"p."; where I only have the PDF's own page index I say "pdf p.". Three web pages that earlier were
"page summaries" were re-read in the original this pass (the binary-greedy-meshing README, the zfp
`modes.rst`, the `PNanoVDB.h` header) and a fourth, the third-party WGSL port of NanoVDB, was checked by
reading its README. What is still second-hand is marked *(page summary)* and listed in §9. Nothing from the
PDFs is copied into the repository.

**Updated 28 Nov 2026** from one more owner-supplied PDF: **Hoetzlein 2016, GVDB** (HPG 2016, 9 pp.; read
whole via `pdftotext`, tables by their extracted text). It is now §2.7's GVDB entry, with numbers. It changes
no recommendation: GVDB is VDB's topology (bitmask plus compacted child list per node) laid out in
fixed-width GPU memory pools with a 3-D texture brick atlas, i.e. the same pool-of-bricks shape S4 already
uses, and its tree build is on the CPU, its topology edits are not on the GPU, and it is CUDA only (§2.7).
Its one transferable measurement is the topology one: shallow N-ary trees (`<3,3,3,4>`, 8³ interior nodes,
16³ bricks) beat octrees by 30–40 % on node insertion and tilemaps on memory (§6.3, Table 2), which agrees
with S5's choice of wide nodes.

---

## 0. Changed after reading the primary sources

Each item says what the first draft claimed, what the source says, and where it lands in the note.

**Corrections (the draft was wrong or unsupported)**

1. **Kämpe 2013 does not say per-ray divergence is the cost.** The first draft (§5.1) wrote that "all
   stress that per-ray divergence is the cost, not memory (Kämpe 2013)". I cannot find that in the paper;
   its text mentions neither divergence nor memory-boundness. What *is* published is Aokana §1 and §3.1 (p.
   10:3, 10:6): deep DAGs make many dependent loads, which hurts cache behaviour, so Aokana uses many
   shallow 256³ DAGs. Rewritten in §5.1.
2. **NanoVDB (§2.7) was judged from the FAQ page; the paper says otherwise on three points.** (a) Per-node
   statistics are minimum, maximum, **average and standard deviation** plus the bounding box of active
   values, not just min/max (Museth 2021, p. 2). (b) The talk lists WebGL and GLSL among the APIs it was
   *tested* with (p. 1, p. 2), not "in progress". (c) It offers optional in-core lossy bit-quantisation
   (2, 4, 8, 16 bits, or adaptive), 4 to 6× smaller and 10 to 30 % faster to read on the GPU (p. 2). The
   "static topology" limit is also out of date: Sifakis et al. 2026 (ACM TOG 45(6), art. 225,
   [10.1145/3842564](https://doi.org/10.1145/3842564)) add GPU-native topology updates, **in CUDA**. And a
   WGSL port of `PNanoVDB.h` exists (§2.7), so "no WGSL path" is false.
3. **ZFP: what the paper says against what the docs say.** The first draft's lossless claim rested on the
   docs. Lindstrom 2014 says its fixed-rate mode
   loses low-order mantissa bits "preventing fully lossless compression" (p. 2680), that it "cannot easily
   bound the maximum error" (p. 2682), and measures what happens when it is applied inside a time-stepping
   solver (§2.8: 16 bits per double gives a 0.06 % error in shock position, 8 bits gives 7.38 %). The
   reversible mode is a later addition, confirmed in the zfp docs' `modes.rst` (§2.8).
4. **Aokana's headline needs its table beside it.** The draft quoted 424 MB of VRAM against HashDAG's
   3.5 GB. Table 1 (p. 10:13) shows what is being compared: for San Miguel at 64K³ Aokana's *geometry* is
   2,217 MB (all LODs) against HashDAG's 640 MB and SVDAG's 2,484 MB; the win is residency (about 2 % of
   the disk size is resident), not compression. Colour is **21,016 of 23,233 MB (90 %)** on Aokana's disk
   and 2,835 of 3,475 MB (82 %) in the HashDAG file. The "avoid lossy compression" paraphrase is also
   too strong: §4.2 (p. 10:12) says complex lossy schemes are unsuitable for attributes, "especially for LOD
   0", but its own colour coder is lossy (a tolerance of 0.05 on colour, §3.3, p. 10:7) and its LOD
   averages colours (§3.4).
5. **Venues and DOIs.** The transform-aware SVDAG is *Proc. ACM Comput. Graph. Interact. Tech.* 8(1),
   article 11 (not "ACM TOG"). Aokana has a journal DOI,
   [10.1145/3728299](https://doi.org/10.1145/3728299) (same journal, article 10). Tao et al. is IEEE TPDS
   30(8):1857–1871, 2019. The Sifakis paper has a DOI. All DOIs in §11 were re-resolved against Crossref
   or DataCite on 25 Nov 2026 (`voxel-engines/doi_check.txt`).
6. **ESVO section numbers.** The draft cited NVR-2010-001 "§6.2" for beam optimisation; in the I3D paper
   it is §4.2 (p. 60 of the proceedings; pdf p. 6), with 8 × 8 pixel blocks.

7. **Two SSVDAG numbers were misread.** The first draft gave "16.2 GB for a SVO" at (64K)³; that figure is the
   16K³ column of the ESVDAG row. The pointerless SVO is **1,660 MB** (2.39 bit/voxel) against 85.8 MB for
   the SSVDAG (Table 2, JCGT pdf p. 21): a 19× ratio, not 190×. And "reflections cost 14–16 %" was the
   *variable-width pointer* encoding; mirroring itself costs 1 to 2 % (pdf p. 24). I found this by
   re-extracting the table with `pdftotext -raw`.

**New findings that bear on the recommendations**

8. **What the DAG papers store.** Every DAG paper stores binary occupancy and keeps attributes in a
   separate array indexed by depth-first leaf order: Kämpe §1 (p. 101:2: "we do not attempt to compress
   material"), SSVDAG limitations (JCGT pdf p. 26), HashDAG §3.3 (p. 114), Aokana §3.3. Molenaar and Eisemann say
   why in §2.3 (p. 11:3): putting density or colour in the voxels "significantly reduces" the number of
   equivalent subtrees, "as subtrees will only be equivalent if they match in attributes and geometry".
   §2.5 and §7 now put numbers on that for a coverage count (new benchmark: on a smooth height field the
   share of distinct level-1 nodes goes from 0.8 % to 82 % when the count is part of the key).
9. **HashDAG's edit costs (§2.5).** The first draft had no edit timings because they are plots; I read them
   off the figures (Figs. 7 and 8, p. 116 to 117, ±20 %): about 2 ms for a 4×10⁵-voxel geometry edit and
   about 100 ms for a 3×10⁸-voxel one at (64K)³, single thread, 1.1× to 4× more with colour. Garbage collection
   of Epic Citadel at (64K)³ took **25 s** (18 s of it in
   the lower levels), unoptimised (p. 118); sending the modified data to the GPU took "tens of
   milliseconds" in the prototype (p. 118); "compute time in the order of milliseconds" for edits of several
   million voxels in a 10¹⁵-voxel scene (p. 118); memory grows with every edit until a collection because
   old versions stay valid (p. 118, Fig. 11); colour chunks (one per subtree at a fixed depth) are rebuilt
   whole on an edit (p. 114) and the initial 1.2 GB of colour could not be freed (p. 118).
10. **Transform-aware ratios (§2.5).** TSVDAG is 20 to 30 % smaller than SSVDAG (Table 4: −17 % to −39 %
   across six scenes), at about +12 % frame time for the pointer encoding and about +30 % more with
   translations (p. 11:14), static and binary only. Its naive translation search "would take weeks" at (64K)³
   (p. 11:8).
11. **Brickmaps and GigaVoxels were read (§2.7, §5.1, §6).** The brickmap thesis is a flat brick grid over the
    draw distance, 8³ bricks as a 512-bit mask, edits by invalidating a grid pointer, and GPU-to-CPU
    feedback streaming; it removes occluded interior voxels and compresses colour lossily, so it is a
    rendering structure, not a mass store. GigaVoxels adds ray-guided production into a brick pool.
12. **Aokana §4.4 (p. 10:14) gives a recipe for editing**: dense arrays and meshes for the editable near field,
    DAG for LOD ≥ 1 rebuilt in idle time. It is the shape S5-5 already proposes.

**Recommendations: what moved, what did not.** No headline recommendation reverses. Three are sharpened
(the S5-1 measurement, the NanoVDB verdict, the advice on lossy float fields); none was weakened.

* **DAG:** unchanged in direction (no DAG for the dynamic store; at most a far-field derived structure), but
  the evidence is firmer and the S5-1 measurement is sharpened: run it **twice**, once on occupancy and once
  on the full leaf content, and report attribute bytes next to topology bytes (§8, S5-1).
* **NanoVDB:** still rejected as the world store, on changed grounds. "No WGSL path" is gone (a port
  exists). "Static topology" is gone only on CUDA (no wgpu or web path for the dynamic operators). "Min/max,
  not multipoles" stands, softened (average and standard deviation exist, still no higher moments, and no
  slot for an ~88 B node summary). A new reason is a second fixed-shape structure beside S4's bricks. It
  becomes a candidate for S8 to S13 field storage and for interchange, not for the mass octree.
* **Far field:** S4's brick pool is confirmed as the shape of the brickmap and GigaVoxels designs. New,
  cheap and sourced: Aokana's 8 × 8 tile pre-pass with last-frame Hi-Z, and the brickmap thesis's trick of
  storing a LOD colour in the pointer of an unloaded brick (S4's stand-ins already do the equivalent).
  Ranking in §8 is unchanged.
* **ZFP and SZ:** unchanged ("not on mass"), sharpened for derived fields: prefer an *error-bounded* mode
  (ZFP fixed-accuracy, SZ absolute bound) for snapshots; avoid fixed-rate for state that a solver rewrites
  every step (§2.8).

---

## 1. One-page summary

**Bottom line.** Cavendish's storage and renderer are already close to what the literature recommends for
its constraints (exact integer leaves, edits, the web build). The literature mostly *confirms* S4's
choices and offers three cheap additions and a short list of things to measure before building anything
bigger.

1. **Lossless only for anything that feeds mass.** Palette + bit-packing, run-length coding and
   content-hash deduplication are exact, cheap to edit and trivially decodable in WGSL. Lossy coders
   (ZFP, SZ) and DAG-with-lossy-LOD are out for the leaf/mass path: the ZFP paper's own mode is lossy
   (Lindstrom 2014, p. 2680), the lossless "reversible" mode is a later addition that is not in the CUDA
   build and has no fixed rate, hence no cheap random access
   ([zfp `modes.rst`](https://github.com/LLNL/zfp/blob/develop/docs/source/modes.rst), read), and
   categorical material data compresses better with a palette anyway.
2. **A DAG is a measurement away, not a decision, and the sources lean against it.** Published ratios
   (node counts 28× to 576× lower, [Kämpe 2013](https://doi.org/10.1145/2461912.2462024), p. 101:7;
   0.123 bit/voxel at (64K)³ and 0.048 at (256K)³, [SSVDAG](https://jcgt.org/published/0006/02/01/paper.pdf),
   Table 2 and Fig. 1; 20 to 30 % smaller again with
   [transform-aware DAGs](https://doi.org/10.1145/3728301), Table 4) are for *binary occupancy* of
   *static* scanned or modelled scenes, and the authors say attributes in the voxel "significantly
   reduce" the sharing (p. 11:3). Cavendish's leaves carry a coverage count and a material, and its world
   is edited. Editing a DAG is possible ([HashDAG](https://doi.org/10.1111/cgf.13916): path-copy,
   a hash table, 1.5 to 2× slower reads, a 25 s unoptimised garbage collection) but I found no paper that edits
   a DAG whose leaves carry a per-voxel number rewritten every step. **One pass that hashes every node and counts unique ones per
   level, on occupancy and on full content**, settles whether a DAG would pay (S5-1; §2.5; first
   numbers on synthetic worlds in §7.2). Until then: no DAG for the dynamic store.
3. **Moments dedupe by content, even without a DAG.** Because a node's summary relative to its own
   centre depends only on its contents, two identical subtrees must have bit-identical summaries
   (§2.6, my derivation). Computing summaries once per distinct content hash cuts S5's build cost and
   gives a free test of determinism (equal content, equal bytes; control: one cell changed).
4. **Meshing is not the bottleneck; draw cost and memory are.** The bit-row mesher already makes a 32³
   chunk in about 0.08 ms ([S4 §7.10](../slices/S4.md)), the same order as the 50–200 µs the
   [binary-greedy-meshing](https://github.com/cgerikj/binary-greedy-meshing) author reports for 64³
   chunks (README read: 50–200 µs, 74 µs average, Ryzen 3800X). A GPU mesher or mesh shaders buy nothing here. What is wasteful is the
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
   tile/beam pre-passes ([ESVO beam optimisation](https://doi.org/10.1145/1730804.1730814), §4.2;
   [Aokana](https://doi.org/10.1145/3728299)'s 8×8 tiles with previous-frame Hi-Z, §3.5) and ordering
   front to back. First measure brick fragment overdraw (§8, S21-2).

**Ranked recommendations** (value for effort; detail and measurements in §8):

| # | For | Do | Effort | Why |
|---|---|---|---|---|
| 1 | S5 | Count unique nodes per level by content hash over `town`, `stress-1e10`, the planet, **twice** (occupancy; full leaf content), with attribute bytes beside topology bytes | hours | Settles DAG/dedup for good, with a known answer; §7.2 shows why both keys |
| 2 | S5 | Compute node summaries once per content hash; assert equal content gives equal bytes | days | Build cost ÷ duplication; a determinism test with a control arm |
| 3 | S21 | Measure brick overdraw (fragments marched ÷ pixels) and front-to-back order | hours | Finds the actual cost of the 99th percentile |
| 4 | S21 | 8-byte quads by vertex pulling, AO in the same word | days | 9× mesh memory/upload; AO needs a byte per quad |
| 5 | S21 | Request `indirect-first-instance` to replace per-chunk dynamic-uniform offsets (if browsers expose it) | hours | Fewer binds; verify support first |
| 6 | S21 | Hi-Z (previous-frame pyramid) for **brick boxes**, not meshes | days | Gains are in the march, not the meshes |
| 7 | later | Tile/beam compute march for the far field | weeks | Largest change; only if 3 and 6 are not enough |
| – | reject | GPU meshing, mesh shaders, multi-draw-indirect-count as a dependency, NanoVDB as the world store, lossy coders on mass | | §2, §3, §4 |

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

### 2.5 Sparse voxel DAGs, SSVDAGs, HashDAGs and transform-aware DAGs

All numbers below are from the papers' own text and tables (printed page or table given). "(64K)³" means a
65 536-cell side; the text extraction of several PDFs drops the K.

| Source | Idea | Numbers | Limit |
|---|---|---|---|
| [Kämpe, Sintorn, Assarsson 2013](https://doi.org/10.1145/2461912.2462024) | Merge identical subtrees bottom-up: sort each level's nodes by their child pointers, compact, patch the parents (§3, p. 101:3–4); leaves are 4³ cells as a 64-bit integer; nodes cost 8 to 36 B (p. 101:4) | Epic Citadel at (128K)³: 19 billion voxels in **945 MB** against 5.1 GB for a pointerless SVO (Fig. 1, p. 101:1); node count cut **28×** (HAIRBALL, "no obvious regularities") to **576×** (CRYSPONZA) (§5.2, p. 101:5); memory up to **38×** below ESVO and 26× below a minimal pointerless SVO (conclusion, p. 101:7) | "We do not attempt to compress material" (p. 101:2); "Our focus has not been on dynamic scenes" (p. 101:4) |
| [Villanueva, Marton, Gobbetti, JCGT 2017](https://jcgt.org/published/0006/02/01/paper.pdf) (journal version of the [i3D 2016 paper](https://doi.org/10.1145/2856400.2856420)) | Also merge subtrees equal under a mirror in x, y or z (3-bit tag in the pointer), nodes re-ordered by reference count, variable-width pointers | PowerPlant (64K)³, about 5.6 billion voxels (derived from the bits/voxel column): **85.8 MB** = 0.123 bit/voxel against 167.3 MB for the SVDAG and 1,660 MB for the pointerless SVO (Table 2, pdf p. 21); at (256K)³, nearly 100 billion voxels in **< 575 MB**, 0.048 bit/voxel, against 31.1 GB for the SVO and 1.0 GB for the SVDAG (Fig. 1, pdf p. 1); mirroring alone costs 1 to 2 % of tracing time, the variable-rate pointers a further 14 to 16 % (pdf p. 24); averaged over the scenes, SSVDAG storage is 52.4 % of the SVDAG's (pdf p. 21) | Geometry only: general use "would require to map non-geometric properties to voxels" (pdf p. 26) |
| [Careil, Billeter, Eisemann 2020 (HashDAG)](https://doi.org/10.1111/cgf.13916) | Edits in the compressed domain: nodes in per-level hash buckets, 32-bit pointers into a paged virtual address space of 16 GB, **path-copy** on edit, old roots stay valid (undo/redo), mark-and-compact garbage collection, one "full subtree" node per level so large fills scale sub-linearly (§3.2 to §3.5, pp. 112–115) | Epic Citadel (64K)³ = 380 MB plus 105 / 260 / 550 MB of paging overhead at 128 / 256 / 512-word pages (p. 117); software paging makes tracing **1.5× slower on average, 2× worst case** (Fig. 6, p. 116); edits of up to 350 M voxels at (64K)³ and 3 G at (128K)³ (Fig. 7, p. 116); **read off the plots of Figs. 7 and 8, single thread, ±20 %**: geometry-only edits of ≤ 4×10⁵ voxels take ≤ 2.3 ms and of 3×10⁸ voxels about 100 ms at (64K)³, ≤ 8 ms for 3.5×10⁶ and about 500 ms for 3×10⁹ at (128K)³, sub-linear in size because of the full-subtree shortcut; with colour 1.1× to 4× that; six threads about 3× faster on large edits; "compute time in the order of milliseconds" for several-million-voxel edits at 10¹⁵ voxels (p. 118) | The edits are solid fills, carves and copies, not partially covered surfaces (§4, p. 116); copies of sparse irregular templates are CPU-bound, about 600 ms for 8×10⁷ template voxels single-threaded at (64K)³ (Fig. 9); garbage collection **25 s** at (64K)³, unoptimised, 18 s of it in the lower levels; CPU to GPU transfer of the changes "tens of milliseconds" (p. 118); memory grows with every edit until a collection (Fig. 11); colour rebuilt per chunk, 1.2 GB of initial colour never freed (p. 118); measured on an RTX 2080, CUDA |
| [Molenaar, Eisemann 2025 (TSVDAG)](https://doi.org/10.1145/3728301), PACMCGIT 8(1) art. 11 | Subtrees equal under any child permutation (in practice: mirrors plus axis swaps, i.e. 90° rotations, §4.1), plus "destructive" translations of up to 16³ cells (§4.2), plus look-up-table and Huffman-coded pointers (§5) | Six scenes at (64K)³, 6 to 39 billion voxels: **17 % to 39 % smaller** than SSVDAG (Table 4, p. 11:13; e.g. Citadel 932 → 597 MB, Bunny 1,080 → 663 MB), typical 20 to 30 % (p. 11:14); element counts −27 % to −50 % with translations (Table 2, p. 11:12); an SVO is 6 to 16× larger than the SSVDAG there (Table 4) | Frame time +12 % for the pointer tables and Huffman, **about +30 % more** with translations (RTX 4070, path-traced, p. 11:14); their naive translation search would need "weeks" at (64K)³, their O(n) search makes it a pre-process (p. 11:8); "static" and "binary voxel scenes" only (p. 11:2, 11:3) |
| [Fang, Wang, Wang 2025 (Aokana)](https://doi.org/10.1145/3728299), PACMCGIT 8(1) art. 10 | One SVDAG **per 256³ chunk**, geometry and colour separate, LOD by merging 8 chunks into one (a voxel survives if ≥ 2 of its 8 sources do, §3.4, p. 10:7), implicit-octree streaming, compute-shader tile march with a 64-bit visibility buffer and previous-frame Hi-Z (§3.5, p. 10:8–10) | San Miguel (64K)³: **424 MB VRAM** against 3,475 MB for the HashDAG file and 2,484 MB for the SVDAG, but geometry on disk is 2,217 MB (all LODs) against HashDAG's 640 MB (Table 1, p. 10:13); about 6 ms a frame on an RTX 3060 Ti (p. 10:11); "2 to 4 times" faster than HashDAG above 32K (p. 10:11) | **No runtime editing** (§4.4, p. 10:14), no transparency, colour averaged in the LOD (p. 10:14); colour is 90 % of the disk size (Table 1) |

What follows for Cavendish:

* **Scale is not the issue.** 10¹¹ cells is 4.6K³; these papers reach (64K)³ to (256K)³ in under a gigabyte.
  The questions are *what is stored* and *whether it is edited*.
* **The sources agree that a DAG compresses topology, not payload.** All five store binary occupancy. The
  attribute (colour, density, a coverage count) goes in a separate array in depth-first leaf order,
  compressed on its own (Dolonius et al., via HashDAG p. 114 and Aokana §3.3). The numbers show which part
  is large: at (64K)³ San Miguel, colour is 2,835 MB against 640 MB of geometry in the HashDAG file
  (Aokana Table 1, p. 10:13). For Cavendish the payload is the coverage count of every non-uniform leaf
  cell, so the saving a DAG offers (topology) is the *smaller* term, and the larger term (one byte of count
  and a material per boundary cell) must be stored whatever the topology does.
* **Putting the count into the DAG key instead** makes equivalence need equal counts, and then sharing
  collapses wherever a surface is not cell-aligned. §7.2 measures this on synthetic worlds: on a smooth
  height field the share of distinct 2³ nodes is 0.8 % for occupancy and 82 % with a 4³-sample coverage
  count; the cell-aligned town (counts only 0 or max) is unchanged.
* **Editing** is the DAG's weak point. HashDAG shows it can be done, with path-copy, a hash table, a
  garbage collector and slower reads, and reports millisecond-scale compute for edits of millions of
  voxels. But its edits are interactive and rare, not a body re-stamped every step; each edit adds nodes
  that live until a 25 s collection; and the colour chunks of every touched subtree are rebuilt whole.
  Aokana, the most recent large GPU renderer, does not edit, and says what to do instead (§4.4): dense
  arrays and meshes for the editable region, SVDAG for LOD ≥ 1 rebuilt when the player leaves. A DAG could
  only hold the static, quiescent world (terrain, buildings that never move), as the plan already suggests
  (`REBUILD.md`, "Atomontage / sparse voxel DAGs" row).
* **Transform-aware sharing (TSVDAG) does not rescue it.** Its extra 20 to 30 % is on top of a binary DAG;
  it costs 12 % to about 40 % in frame time on a GPU; it is built offline. Rotations and translations of a
  *body* are exactly what Cavendish's world does not keep still.
* **Cheap, decisive measurement (S5-1):** build the world octree as now; then, per level, hash each node as
  `H(node kind, children's hashes or leaf bytes)` and count distinct hashes against the node count of the
  non-uniform nodes, **once with occupancy only and once with the full leaf content**; report the bytes of
  the payload array next to the bytes of topology. If distinct/total at levels ≥ 1 is above ~0.3 for the full
  content, stop; if it is below ~0.05 on a scene the owner cares about, a far-field DAG is worth a slice.
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

### 2.7 OpenVDB, NanoVDB, GigaVoxels and brickmaps

* **OpenVDB** ([Museth 2013](https://doi.org/10.1145/2487228.2487235), read). A short, wide, height-balanced
  tree with small leaves. The recommended shape (§2.4, pdf p. 8) is a hash-map root over internal nodes of
  32³ and 16³ children and **8³ dense leaves with a bit mask of active cells**, so each root child spans
  4,096³ cells (Table II, §5.1, calls it [5,4,3]); average O(1) random access through direct-access tables.
  The paper explains why *not* to use one big flat grid: with 8³ leaves a flat table for an 8,192³ domain is
  "a massive 8 GB, even before voxel values are inserted" (§2.4). Compression (codecs, bit-quantisation) is
  applied when a leaf is written to a stream, not in the traversed structure. It is the right tool for
  simulation grids and was designed for them; the fluid slices (S8, S9) are the natural place to borrow its
  ideas (bit-masked dense leaves, tiles for uniform regions).
* **NanoVDB** ([Museth 2021](https://doi.org/10.1145/3450623.3464653), the 2-page talk, read; the dynamic
  extension [Sifakis et al. 2026](https://doi.org/10.1145/3842564), read).
  * *Layout.* A linearised snapshot of an OpenVDB tree: pointer-free, nodes **32-byte aligned** in one
    contiguous buffer, breadth-first by level, usable as its own file format (p. 2). Fixed 4096/128/8
    node progression, i.e. the [5,4,3] shape with 4,096³ root tiles (Sifakis §2, p. 3). Per-node metadata:
    **min, max, average, standard deviation and the bounding box of active values** (Museth 2021, p. 2).
    The header constants in `PNanoVDB.h` (read from the OpenVDB repository, `master`) give the fixed
    part of each node: leaf 80 B (512 values follow), lower node 1,056 B with 4,096 table entries, upper node
    8,224 B with 32,768 table entries, grid 672 B, tree 64 B. So an isolated occupied cell costs at least
    80 + 1,056 + 8,224 = 9.4 kB of headers before any table or value: the structure is made for volumes with
    dense-ish leaves, not for a speck of dust a kilometre away.
  * *Topology.* The 2021 talk names static topology as the biggest limitation: it "does not include tools
    to modify the topology of the tree" (p. 2); values can change, and a tool recomputes the statistics. Sifakis et al. 2026 remove the limit with
    bulk-synchronous GPU operators (dilation, merge, prune, coarsen, refine) that "reconstruct grids
    containing hundreds of millions of voxels in just a few milliseconds" (abstract) and voxelise a
    2.8 M-triangle mesh to 9.88 billion active voxels in 3.24 s (Fig. 1, Fig. 12). They are **CUDA
    and CUB** (prefix sums, `atomicOr`, 512-thread blocks, §3 and §5) and need about 16 MB of scratch per
    4,096³ root tile touched (§6, p. 13). They work on *Index-Grids*, where a voxel holds an integer index
    into a separate "sidecar" array of payload, allocated only for active voxels (p. 4). No wgpu or web path.
  * *Compression.* Optional in-core lossy bit-quantisation (2, 4, 8, 16 bits or adaptive per block): 4 to 6×
    smaller, 10 to 30 % *faster* to read on the GPU because rendering is memory bound (Museth 2021, p. 2).
    Not applicable to mass.
  * *Is a WGSL read path feasible?* **Yes, and one exists.** `PNanoVDB.h` is the portable C99/GLSL/HLSL
    port; in HLSL and GLSL it represents a 64-bit integer as a pair of 32-bit words (`uint2`, `uvec2`), reads
    the buffer in 32-bit words, and has a `PNANOVDB_ADDRESS_32` mode, so nothing in the layout needs a native
    64-bit type. The talk lists WebGL and GLSL among the tested targets (p. 1, p. 2). A third-party WGSL
    translation, [`emcfarlane/webgpu-nanovdb`](https://github.com/emcfarlane/webgpu-nanovdb) (`pnanovdb.wgsl`,
    55 kB, a compute-shader demo, last pushed Dec 2025, 13 stars, one author), and a WebGPU-native repack of
    the same layout, [PicoVDB](https://github.com/emcfarlane/picovdb) (32-bit offsets; its README claims
    50 % smaller than NanoVDB on the Stanford bunny, 64 → 28 MB, and warns the format may change), were found
    by a code search and their READMEs read. I did not build or run either. The licence of the first is
    stated as Apache-2.0 in the README but GitHub's detector reports none.
  * *Verdict for Cavendish.* Still not the world store: (i) the node summary S5 needs (mass, centre of mass
    and higher moments, ~88 B per internal node, S3 §5) has no home: NanoVDB's per-node fields are
    min/max/average/standard deviation, and the sidecar is per *active voxel* (Sifakis p. 4); (ii) topology
    edits exist only as CUDA kernels, so the page could read a NanoVDB but not edit one; (iii) a fixed
    4096/128/8 shape against S4's 32³ bricks and S5's own levels is a second structure to keep consistent.
    The old third reason ("no WGSL") is gone. It stays a candidate for **derived fields in S8 to S13** (a
    sparse velocity or temperature grid with a bit mask and an index-grid sidecar) and for importing and
    exporting volumes, where the WGSL reader would be a spike of days.
* **GigaVoxels** ([Crassin et al., I3D 2009](https://doi.org/10.1145/1507149.1507152), read). An N³-tree
  whose nodes are in a node pool (a 3D texture) and point either to constant data or to an M³ **brick** in a
  brick pool; bricks are filtered, mip-mapped voxel data used for cone-traced, anti-aliased rendering (§3 to §5).
  The distinctive idea is **ray-guided production**: the renderer itself records which nodes and bricks the
  rays needed, a compaction pass shrinks that to a list, and the CPU subdivides, fuses or loads under an LRU
  rule (§6). Measured: an 8,192³ scan at 20 to 90 fps on a GeForce 8800 GTS 512 (abstract), a 4 MB node
  pool for 64³ entries, 16³ bricks and a 430 MB brick pool "giving room for 42³ bricks" (§7, the extraction reads 423). It shares the
  structure of brickmaps but is "dynamic" in the sense of view-dependent content, not edits.
* **Brickmaps** ([van Wingerden, MSc thesis, Utrecht, 25 June 2015](https://hdl.handle.net/20.500.12932/20460),
  link from the first draft, unchecked; the PDF read is the owner's copy, supervisor J. Bikker). A flat
  **brickgrid** of pointers covering the whole draw distance; each pointer is a loaded brickmap, an unloaded
  one or empty; a brickmap is **8³ cells as a 512-bit solid mask with a pointer to its colour array**, 71 B
  without colours (§3.1); a 1-bit-per-cell grid of 4³ brickgrid cells on top skips open air (§3.4);
  DDA at each level (§3.3). Streaming is **occlusion-based**: the tracer appends the bricks it hit but did
  not find to a feedback buffer of 256 per frame; an unloaded pointer carries a 3-byte LOD colour and a flag
  so distant or missing bricks still draw (§3.2.1, p. 12–13). **Editing** is the point of the design: the CPU
  edits raw data, sets the grid pointers "unloaded", and the streaming path re-uploads (§3.2.2, p. 13); it
  can simulate snow and erosion in real time. What it does *before upload* matters for Cavendish: it
  **removes voxels completely surrounded by opaque voxels** and compresses colour with DXT1 (4 bits per
  voxel, lossy) (§3.1.3 and §4.2, p. 11 and p. 16); on disk, LZ4 per chunk gave about 1:10 on landscapes
  (§3.2.3). Results (Radeon HD 7970, OpenCL): 40 to 164 MRays/s; GPU memory 64 to 112 MB in all three scenes,
  of which a fixed 64 MB is the brickgrid and its top layer (§5.3: stated to be the same in all three scenes); about 27 % lower ray rate
  while streaming (§5.5). Limits: the brickgrid size *is* the draw distance, and a flat grid does not
  scale: at Cavendish's 10¹¹ cells (4,642³) and 8³ bricks it would be 580³ = 195 M pointers, 0.78 GB at
  4 B each (my arithmetic; compare VDB's 8 GB flat-table warning above), so S4's clipmap rings and S5's
  tree are the right outer structure and the brick pool the right inner one.
* **GVDB** ([Hoetzlein 2016](https://doi.org/10.2312/hpg.20161197), HPG 2016, read whole, 28 Nov). VDB's
  topology on the GPU: "GVDB expresses an identical topological layout in virtual address space as OpenVDB"
  (§4.1), configured by a log2 vector, e.g. `<5,4,3>` (32³, 16³ then 8³ leaf bricks), which also expresses an
  octree (`<1,1,…>`) or a two-level tilemap (`<9,4>`) (§4.1). Differences from OpenVDB (§4.1): one node type
  for all levels, **integer indices instead of pointers** so the CPU and GPU copies need no address
  translation, and brick voxels in a **3-D texture atlas** for hardware trilinear filtering. Memory (§4.2,
  Fig. 3): two pools per level, `P0ᵢ` for fixed-width node records (a 40-byte header: level, position, atlas
  value, parent index, child-list index, then the `res(i)³/8`-byte bitmask) and `P1ᵢ` for child lists, sized
  at a fraction `K` (½ or ⅓) of the maximum child count and reallocated or moved on overflow; a child is found
  by 64-bit bit counting on the mask. The topology is "less than 1% (average 5MB) of memory compared to the
  atlas"; 500k leaf bricks need ~20 MB of topology against ~500 MB for a 512³ indirection table (§4.2). The
  whole tree is `2L` pools, uploaded or written to disk one call per pool (§4.2). **Apron voxels** are stored
  in the atlas and refilled by a kernel from neighbours (§4.3). **Ray tracing** (§5, Algorithm 1): a
  branchless hierarchical 3-D DDA after Amanatides and Woo, re-initialised per level so one set of DDA
  registers serves all levels, with a short stack of exit `t` values (after Horn et al.'s kd-tree short
  stack); it never returns to the root mid-ray. **Results** (§6.1, Table 1; Quadro M6000 against OpenVDB on
  a 4-core i7-3770K, same images, Figs. 5–6): 11.6–43.6 ms per frame on five OpenVDB scenes, 100–200× one
  CPU core and ~60× four cores for ray-sampled volumes, 25–30× and 5–6× for level sets. **Topology study**
  (§6.3, Table 2, 1280×960, one SPH frame resampled at 512 to 2048 voxels): ray-cast 11.5–125.6 ms; octrees
  were 30–40 % slower than 8³-interior N-ary trees on node insertion, a 128³ tilemap was slow because of bit
  counting over its 2-million-bit mask, and `<3,3,3,4>` with 16³ or 32³ bricks is "a good balance" (Fig. 7);
  beyond 2048³, 64³ bricks build in ≤ 3 ms for ~10 % more occupancy. **Limits it states**: tree rebuild is
  CPU-based (§6.3), "GVDB does not yet implement simulation" (§1), level of detail and out-of-core are not
  implemented (§7), CUDA/OptiX only. *For Cavendish:* no change. It confirms the pool-of-fixed-bricks plus
  wide-node index shape (S4, S5) and the integer-index layout that crosses CPU and GPU unchanged (which S4's
  slot indirection already is); the leaf payload is a float scalar for rendering, with no slot for a node's
  mass moments, and nothing in it bears on the gravity read.
* **What this says about S4.** A pool of fixed bricks behind a slot indirection, an empty-space mask above it,
  edits by invalidating a slot, and LOD stand-ins for unloaded bricks are all in the brickmap thesis and
  GigaVoxels, i.e. S4 is on well-trodden ground. Two differences are Cavendish's own: its leaf is a count and
  a material (the thesis stores one bit and a colour), and nothing may be removed as "invisible" because
  interior cells carry mass; the visibility cull and DXT1 are rendering-only steps and must stay out of the
  mass path.

### 2.8 Lossy compression (ZFP, SZ) for density and material fields

* **ZFP** ([Lindstrom 2014](https://doi.org/10.1109/TVCG.2014.2346458), read). Blocks of 4ᵈ doubles are mapped
  to a **fixed user-chosen number of bits per block**, which is what allows read *and write* random access
  at block granularity (abstract); an orthogonal block transform and embedded bit-plane coding, so a block's
  stream can be truncated anywhere; a software write-back cache of decoded blocks hides the cost. Measured:
  about 400 MB/s decode and 280 MB/s encode at 1 bit per double on a 2014 CPU (p. 2680); a 1.7× constant
  slowdown on array access against raw (p. 2680). Three statements matter here. (i) It is **lossy by design**;
  even at its highest rate it loses low-order mantissa bits when a block's exponents differ by more than
  eight, "preventing fully lossless compression" (p. 2680). (ii) "As designed, our scheme cannot easily
  bound the maximum error incurred"; it minimises RMS error, and a fixed-quality mode with an absolute or
  relative tolerance is available by *relaxing* the fixed-rate constraint (p. 2682). (iii) It was tested
  **inside a solver**: in LULESH (a Sedov blast) each block was re-compressed 1.18 times per step with a
  two-layer cache (20.9 with half the cache), and over 1,000 steps the final shock position was off by
  **0.06 % at 16 bits per double, 0.80 % at 12, 7.38 % at 8**; the authors conclude that 4× is tolerable
  "in simulations that demand repeated state updates", 16× or more in visualisation and analysis (p. 2682, conclusion). The transform "can be implemented using only integer addition and shifting"
  (p. 2682), so a WGSL decoder looks possible; the zfp library itself has CUDA, not wgpu, and I did not
  look for a port. The **reversible** mode that makes zfp lossless for integers and floats is a later
  addition ([`modes.rst`](https://github.com/LLNL/zfp/blob/develop/docs/source/modes.rst), read): each block
  is independent, but there is no fixed rate and it is "not yet implemented in CUDA".
* **SZ** ([Di and Cappello 2016](https://doi.org/10.1109/IPDPS.2016.11), read). Linearise the snapshot,
  predict each value from its already *decompressed* predecessors with the best-fitting of several
  curve-fitting models, and store a code when the prediction is within the user's **absolute or relative
  error bound**; unpredictable values are stored by a truncated binary representation (abstract, §II, §III).
  Compression ratios 3.3:1 to 436:1 over 13 applications, "higher than the second-best solution ZFP by as
  little as 2× and as much as an order of magnitude for most cases"; the bound is strict. It is a
  whole-snapshot coder; I found no claim of random access in the paper.
* **SZ against ZFP** ([Tao et al.](https://doi.org/10.1109/TPDS.2019.2894404), IEEE TPDS 30(8) 2019;
  arXiv v2, read). Neither wins everywhere: at equal PSNR SZ gave the better ratio on 72.8 % of the
  fields in the climate (ATM) data and ZFP on 27.2 % (§1, p. 1); an online per-field selection improves the
  ratio by up to 70 % with under 7 % overhead. Their abstract calls both "libraries for compressed
  floating-point arrays that support high throughput read and write random access"; that is ZFP's fixed-rate
  mode, and I could not verify it for SZ.
* **Verdict.** Unchanged for mass: it comes from integer counts and a palette, nothing there is
  floating point, and the lossless route (palette, RLE, dedup) already does well on categorical data. For
  **derived floating-point fields stored per cell** (temperature, fluid velocity and pressure in S8 to S13)
  the sources sharpen the advice: (a) for *snapshots and replay files* use an error-bounded mode (SZ with an
  absolute bound, or ZFP fixed-accuracy), record the bound in the ledger, and set it below the field's own
  discretisation error; (b) do **not** use fixed-rate ZFP for state that a solver rewrites every step: the
  only in-solver test in the literature I read shows errors of 0.06 % to 7 % in a derived quantity at 4× to
  8× compression, and Cavendish's phase budget is µrad, so any in-loop use needs its own measurement
  against an uncompressed run; (c) never on the density field that the gravity read sums.

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
  re-read in the original on 25 Nov 2026): a 64×64 array of `u64` occupancy; face culling with bit operations does 64 faces at
  once; merging also runs in bit operations, with the voxel type looked up to allow a merge; **50–200 µs
  per chunk, 74 µs average single-threaded (108 µs in a thread pool) on a Ryzen 3800X**; chunk ≤ 64³
  including neighbour padding (the masks are 62 × 62); **8 bytes per quad** (6-bit x, y, z, width, height in
  the first word, 8 bits of type in the second), drawn by vertex pulling; quads are drawn about 1 px larger
  to hide T-junction cracks. Its demo draws all chunks with one `glMultiDrawElementsIndirect`, which the web
  does not have (§4.3).
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
([paper](https://doi.org/10.1145/3728299), §3.5, p. 10:8–10, read). Two details are worth copying: for a
chunk the camera is *outside* of, the tile test uses the depth of the chunk's bounding box, which is
never farther than the true surface, so the cull stays conservative; for the chunk the camera is *inside*
it must trace the structure itself, because the box depth would then be too far and cull wrongly. Nanite
does the same two-pass scheme for clusters: draw what was visible last frame, build the HZB, draw what is
newly visible, and the second pass "is usually a tiny fraction of the main pass"
([Karis 2021](https://advances.realtimerendering.com/s2021/Karis_Nanite_SIGGRAPH_Advances_2021_final.pdf),
slides 17 to 19 and 75 to 76; it tests against the mip where the screen rectangle is at most 4×4 pixels,
and found cone back-face culling "not typically worthwhile"). A Cavendish version would cull a brick
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
64-bit visibility buffer with `InterlockedMax` (24 bits of depth, 3 of normal, 13 of chunk id, 24 of
voxel position; Fig. 7, p. 10:10) and Nanite's (30 bits of depth, 27 of cluster, 7 of triangle; slide 84)
cannot be copied literally; it needs a two-`u32` scheme (e.g. atomicMax on depth, second pass writes the
id where depth matches; mine, untested). Cavendish's fragment-march bricks do not need a visibility buffer
at all unless S21 moves to a compute march.

### 4.4 Meshlets / clusters for voxels

Cluster hierarchies (Nanite, [Karis 2021](https://advances.realtimerendering.com/s2021/Karis_Nanite_SIGGRAPH_Advances_2021_final.pdf);
slides on culling, the visibility buffer and rasterisation read, not the build or streaming sections) solve
the problem of *continuous LOD of arbitrary triangles*: clusters of 128 triangles (slide 85), a software
rasteriser for micro-triangles at about 3× the hardware's speed (slide 81), hardware for big ones. Karis
also argues against voxels for authored meshes: resampling a 2 M-polygon bust to 13 M narrow-band SDF
voxels is "6 times the amount of data" and "looks blobby" because "uniform resampling means loss"
(slides 6 and 7), which is a point about authored art, not about a world that is voxels by definition. Voxel worlds get LOD for free from the clipmap (S4) and the octree (S5). The one idea
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
  64-bit leaf. What the sources say about cost is about *depth*, not divergence: Aokana (§1 and §3.1,
  p. 10:3 and 10:6) argues that a deep pointer-linked DAG makes many dependent, cache-unfriendly loads,
  and that many shallow DAGs (one per 256³ chunk) trace faster than one deep one, slightly better than a
  single DAG with beam optimisation at high resolution (Fig. 10, pp. 10:11 to 10:12). S4's two-level brick march is the
  shallow case. (An earlier draft attributed a divergence claim to Kämpe 2013; it is not in the paper.)
* **Beam optimisation** ([Laine and Karras, I3D 2010](https://doi.org/10.1145/1730804.1730814), §4.2, p. 60,
  read): render a coarse, conservative distance image first (8×8 pixel blocks), start each ray at
  that distance, skipping the empty space at the beginning. It works because voxels are cubes, so a beam
  can be bounded conservatively; the paper shows iteration counts per pixel falling (its Figure 9).
  It is for *primary* rays; the extension to shadows (shoot four rays per pixel to the light, then trace
  the same beam back from the light) is in Kämpe 2013 (p. 101:4). The same paper (§3.5) puts the optimum
  at about **5 bytes per voxel including shading attributes** (2 bytes of child descriptors per voxel at a
  branching factor of 4); measured 5.2 to 8.1 B per voxel in its Table 1. Cavendish's far field is rasterised brick boxes with a per-fragment march, which
  already starts each ray at the box's entry; a beam pass would help where **many boxes stack behind each
  other along a ray** (overdraw), not inside a single box.
* **Brickmaps** (van Wingerden 2015, read; §2.7): two grid levels and a 1-bit open-air layer, DDA at each,
  a 512-bit mask per 8³ brick and no stack. The thesis argues (§5.4.1) that avoiding the traversal stack
  that ESVO needs is an advantage on the GPU, and that a beam pass "can be developed for our solution". It
  compares only against numbers read off the ESVO paper, on different hardware, so I take that as
  plausible, not measured.

### 5.2 Hybrid: raster near, trace far (what Cavendish does) and its pitfalls

[Aokana](https://doi.org/10.1145/3728299) (§3, §3.5, p. 10:5 and 10:8; Fig. 12 shows mesh objects in a
voxel scene) does the same: raster/mesh pipeline for near content, voxel compute passes inserted between
the opaque and transparent passes, composed through the depth buffer, with the opaque colour and depth
blitted to copies first (p. 10:8 to 10:9).
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
   children, and the six faces of a voxel given one colour, p. 10:14); its SSIM against the all-LOD-0
   image at its default StreamingFactor of 2.0 is **0.89 (San Miguel), 0.80 (Sponza), 0.87 (Greenfield)**
   (Table 5, p. 10:14), so the LOD error is not small. Cavendish's LOD is a measured table with a recorded
   error, so this applies to *colour* only (S21's data look keeps map colours exact).

---

## 6. Streaming and memory

* **Uploads.** wgpu's `Queue::write_buffer` copies into a *new* staging allocation per call, scheduled
  at the next `submit`, freed after it completes; `write_buffer_with` writes straight into that staging
  memory (saves a copy); and `StagingBelt` recycles a ring of staging buffers and is "most useful for
  writing many small pieces of data" ([Queue docs](https://docs.rs/wgpu/latest/wgpu/struct.Queue.html),
  [StagingBelt docs](https://docs.rs/wgpu/latest/wgpu/util/struct.StagingBelt.html), both *page summary*).
  S4 §7.10 says uploads are "a few hundred vertices" and 4 KB bricks, not measured to matter: a belt
  adds nothing until a flythrough trace shows upload time, which the trace can record.
* **Residency.** Aokana's rule (§3.4, pp. 10:7 to 10:8, Eq. 1) is a screen-space error on an implicit octree per chunk,
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
  ([HashDAG](https://doi.org/10.1111/cgf.13916), Fig. 6, p. 116 and the text on sparse resources).
  **For Cavendish:** keep the brick pool with an explicit slot indirection (as S4 does); do not build
  a second paging layer.
* **Disk.** Aokana's disk footprint is *larger* than HashDAG's because of per-LOD storage (23 GB for San
  Miguel 64K³ against 424 MB of VRAM): LOD levels on disk are cheap compared with VRAM, which supports
  S5's choice to store levels explicitly (Table 1, p. 10:13: 23,233 MB on disk against 424 MB resident).

---

## 7. Two small benchmarks [M]

### 7.1 Lossless chunk storage on two synthetic worlds

File: [`voxel-engines/chunk_storage.py`](voxel-engines/chunk_storage.py) (Python 3.14, NumPy; ~10 s). **Pass criterion written first:** every encoding decodes to the identical
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
cost, or anything about the coverage-count leaves (§7.2 starts on that). Extend it by pointing it at an
exported chunk dump of `town` (no exporter exists that I know of; the engine's region files are the natural
input).

### 7.2 Do coverage counts defeat dedup? A second benchmark [M]

File: [`voxel-engines/dag_dedup.py`](voxel-engines/dag_dedup.py), output
[`dag_dedup.out.txt`](voxel-engines/dag_dedup.out.txt) (Python 3.14, NumPy; 7 s). It is S5-1 on synthetic
worlds: Kämpe's bottom-up merge counted per level, once on **occupancy** (what every DAG paper stores) and
once on the **full coverage count** (what Cavendish stores). Only *mixed* nodes are counted (not all empty,
not all full), because uniform nodes cost nothing in Cavendish already.

**Pass criteria, written before running** (they are also in the file): P1 (control) a world of random
counts must give distinct/total > 0.99; P2 (control) a cell-aligned town, where every count is 0 or the
maximum, must give *identical* ratios for the two keys, and P2's own control arm (the offset town) must
*not*; P3 (the question) on a sub-cell-offset town and on terrain the full-content count of distinct nodes
should be much larger than the occupancy count. No threshold for P3 was set. All three controls passed.

Worlds: 128 × 128 × 64 cells, 4 × 4 × 4 samples per cell (counts 0 to 64). "Town": 16 boxes of
16 × 16 cells on a 32-cell pitch, four heights; cell-aligned, or each shifted by a random fraction of a cell.
"Terrain": five random plane waves as a height field, exact sample-count coverage.

| world | key | mixed nodes (all levels) | distinct | distinct / mixed | level 1 (2³ cells) | level 2 (4³) |
|---|---|---|---|---|---|---|
| town, cell-aligned | occupancy = count | 500 | 34 | 0.068 | none mixed | 1 of 144 |
| town, sub-cell offsets | occupancy | 1,796 | 90 | 0.050 | 6 of 948 | 13 of 417 |
| town, sub-cell offsets | **count** | 2,470 | **288** | **0.117** | 38 of 1,488 | 77 of 522 |
| terrain | occupancy | 5,184 | 1,320 | 0.255 | 27 of 3,424 (0.8 %) | 809 of 1,270 (64 %) |
| terrain | **count** | 8,090 | **6,952** | **0.859** | 4,898 of 5,976 (**82 %**) | 1,529 of 1,588 (96 %) |
| random counts (control) | count | 149,796 | 149,796 | 1.000 | 131,072 of 131,072 | 16,384 of 16,384 |

(Cells are "distinct of mixed". The cell-aligned town has no mixed node below its 4³ blocks, so its first
row is at level 2.) **Reading it.** With the count in the key, a smooth surface that is not cell-aligned
defeats sharing almost completely at the lowest levels (0.8 % to 82 % distinct), and the number of
mixed nodes itself rises by about half, because nodes that are fully non-empty but partly covered now
count as mixed. Higher levels share little in either case, since a node there contains whole building
corners or terrain patches. Translated copies of one shape share if their sub-cell offsets repeat and fall
apart if they do not (the town rows). What it does **not** show: real worlds (these are 16 boxes and a
smooth height field); the effect of coverage resolution (a larger *k* can only make it worse); the bytes
saved (this counts nodes, not the 8 to 36 B records); or edit cost. It supports the direction of §2.5:
the count belongs beside the topology, not inside the DAG key; S5-1 on real data replaces it.

---

## 8. Recommendations for S5 and S21, with what to measure

### S5 (the world octree and the gravity read)

**S5-1. Count distinct nodes per level (hours), on two keys.** Hash `(kind, child hashes | leaf bytes)`
for every non-uniform node of `town`, `world-box`, `stress-1e10` and the planet world, **once with
occupancy only (count > 0) and once with the full leaf content**; report distinct/total per level for
both, the bytes of the leaf payload array next to the bytes of topology, and the bytes a DAG with S5's real
node record (topology 8 B + summary ~88 B) would save. *Pass:* a number per level and key, recorded in the
slice note with the decision rule of §2.5. *Controls:* a world filled with random leaves must show
distinct/total ≈ 1 on the full key; a cell-aligned world must give equal ratios on both keys (both are
in `voxel-engines/dag_dedup.py`, which can be pointed at real data by replacing its world builders).

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
read-only derived structure for static levels ≥ 1, rebuilt on edit of its inputs (Aokana §4.4: dense near
field, DAG for LOD ≥ 1 rebuilt when the region goes quiet), not the world's store. HashDAG shows editing
is possible but costs a hash table, a garbage collector (25 s at (64K)³, unoptimised) and 1.5–2× in
reads, and its edits leave dead nodes until that collection. Keep the coverage count in a payload array
beside the topology, as every DAG paper does with attributes.

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
* **Parts of the Nanite deck.** I read the extracted text (slides and speaker notes) of the slides on voxels
  (6 to 8), culling (17 to 19, 75 to 77), the visibility buffer (21 to 24, 84, 99), software rasterisation
  (81 to 93) and the timing slide (110), and found the rest by keyword. I did not read the cluster-build,
  LOD, streaming, material or shadow sections, so no claim here rests on them.
* **The Aaltonen and Haar 2015 slides** were read in the first pass and not re-opened this pass (they were
  not among the files supplied); their numbers are as the first draft had them.
* **GVDB** (Hoetzlein 2016): read on 28 Nov (§2.7). Not verified: its numbers against any WebGPU or wgpu
  implementation (none was built), and Table 2's columns, which were matched to values from the extracted
  text rather than from the page image.
* **The WGSL NanoVDB port and PicoVDB** were found by a code search and their READMEs read; I did not build,
  run or benchmark either, and the licence of the first is ambiguous (README says Apache-2.0, GitHub's
  detector reports none). Whether a WGSL reader is fast enough for Cavendish's march is open.
* **HashDAG edit timings** are read by eye from scatter plots (Figs. 7 and 8) and are good to about 20 %.
  Nothing in the paper measures a body re-stamped every step, or partially covered cells.
* **Whether any DAG paper edits per-voxel payloads at a high rate.** I found none among the ones supplied; I
  did not search beyond them.
* **SZ random access.** Tao et al.'s abstract says SZ and ZFP both support random access; Di and Cappello's
  method is a sequential prediction over a linearised snapshot and I found no random-access claim in it.
* **ZFP in WGSL.** The paper says the transform needs only integer addition and shifts (p. 2682); I did
  not look for a WGSL or wgpu decoder and did not measure one.
* **Second-hand sources that remain** (page summaries from the first pass, not re-read in the original):
  0fps meshing, the gpuweb issues (#1354, #455), the Chrome 131 note, the wgpu `Features`, `Queue` and
  `StagingBelt` docs, the W3C spec limits, vkguide, Bevy's vertex-pulling repository and the Unity GPU
  mesher repository. The three that were second-hand and are now primary (binary-greedy-meshing README, zfp
  `modes.rst`, the Aokana paper) are marked as read.
* **WebGPU default limits beyond three:** the spec fetch truncated before the buffer-size limits; the S3
  note §6 covers flat-storage limits and should be taken as the reference.
* **Whether S4's region format already compresses**, and what `ChunkCells` stores exactly (palette
  width); I read only the slice notes and `ChunkVertex`.
* **Printed page numbers** are given for the ACM, Eurographics, IEEE and PACMCGIT papers, whose PDFs carry
  folios. The JCGT paper and the PDFs without clear folios (Aokana's, Museth 2013, Crassin 2009, Di and
  Cappello 2016) are cited by section or "pdf p."; the extraction of tables in several PDFs was scrambled
  and I decoded Villanueva et al.'s Table 2 with `pdftotext -raw` (which is how the first draft's "16.2 GB
  for an SVO" was found to be a different column: the pointerless SVO is 1,660 MB at (64K)³).
* **My own derivations** (§2.6 moment dedup, integer moments, the 64-bit visibility-buffer workaround, the
  flat-brickgrid arithmetic in §2.7, the decision thresholds in §8) are untested.
* **The §7 benchmarks are synthetic**; they support only relative statements.

## 10. Further questions (not chased)

1. Do the coverage-count boundary leaves dedupe on real worlds? §7.2 gives the direction on synthetic ones;
   S5-1 answers it on `town`, `stress-1e10` and the planet.
2. A far-field-only DAG with float-float moments on the GPU: node record size and walk cost, and the
   rebuild cost when a region goes quiet (Aokana §4.4).
3. Compute-shader mesher for the single-threaded page: worth it only after S21-4.
4. Conservative depth in WGSL/wgpu (`depth_greater`-style qualifiers): does it exist, per backend?
5. Reversed-Z and the 5 km far plane: current depth format and precision at the horizon.
6. Time-varying voxel data coding (Kämpe et al. 2016, cited by HashDAG p. 112), for replayed scenes.
7. ZFP or SZ on the S8 to S13 float fields: error-bounded snapshots first; an in-loop use needs a run
   against an uncompressed solver (Lindstrom's LULESH test is the model).
8. A WGSL NanoVDB read spike for imported volumes and S8 to S13 fields (days): does `pnanovdb.wgsl` run
   at useful speed in wgpu on the 1660 SUPER and in the browser?
9. Ray-guided brick production (GigaVoxels, the brickmap thesis) for the far field: would an S4 brick
   request list written by the fragment march beat clipmap rings?
10. HashDAG's persistence (old roots stay valid) as a cheap undo for editor edits, independent of whether
    the world is a DAG: a different question for the editor slices.

## 11. Sources, and the DOI check

DOIs were re-resolved on 25 Nov 2026 against Crossref (ACM, IEEE, Wiley) and DataCite (arXiv); the output is
in [`voxel-engines/doi_check.txt`](voxel-engines/doi_check.txt). "Read" means the full text of the PDF the
owner supplied, extracted with `pdftotext`, unless stated.

| Source | DOI / link | Read as | Check |
|---|---|---|---|
| Kämpe, Sintorn, Assarsson, "High resolution sparse voxel DAGs", ACM TOG 32(4), art. 101, 2013 | [10.1145/2461912.2462024](https://doi.org/10.1145/2461912.2462024) | ACM version, 8 pp. | resolves; matches |
| Villanueva, Marton, Gobbetti, "Symmetry-aware sparse voxel DAGs (SSVDAGs) for compression-domain tracing of high-resolution geometric scenes", JCGT 6(2), 2017 | no DOI: <https://jcgt.org/published/0006/02/01/paper.pdf> (HTTP 200) | JCGT version, 30 pp.; its i3D 2016 ancestor is [10.1145/2856400.2856420](https://doi.org/10.1145/2856400.2856420) | i3D DOI resolves; the JCGT paper says it extends it |
| Careil, Billeter, Eisemann, "Interactively modifying compressed sparse voxel representations", CGF 39(2):111–119, 2020 | [10.1111/cgf.13916](https://doi.org/10.1111/cgf.13916) | publisher version, 9 pp., figures viewed | resolves; matches |
| Molenaar, Eisemann, "Transform-aware sparse voxel directed acyclic graphs", PACMCGIT 8(1), art. 11, 2025 | [10.1145/3728301](https://doi.org/10.1145/3728301) | publisher version, 16 pp. | resolves; first draft's "ACM TOG" was wrong |
| Fang, Wang, Wang, "Aokana", PACMCGIT 8(1), art. 10, 2025 | [10.1145/3728299](https://doi.org/10.1145/3728299); preprint [10.48550/arXiv.2505.02017](https://doi.org/10.48550/arXiv.2505.02017) | arXiv v1, 17 pp. (the PDF carries the ACM DOI) | both resolve; the journal DOI was missing from the first draft |
| Museth, "VDB: high-resolution sparse volumes with dynamic topology", ACM TOG 32(3), art. 27, 2013 | [10.1145/2487228.2487235](https://doi.org/10.1145/2487228.2487235) | 22 pp. | resolves |
| Museth, "NanoVDB", SIGGRAPH 2021 Talks | [10.1145/3450623.3464653](https://doi.org/10.1145/3450623.3464653) | the 2-page talk | resolves |
| Sifakis, Williams, Horvath, Sawhney, Harris, Swartz, Museth, "Dynamic GPU sparse grids and voxel-sequential processing in NanoVDB", ACM TOG 45(6), art. 225, Dec 2026 | [10.1145/3842564](https://doi.org/10.1145/3842564) (printed in the PDF) | 21 pp. | resolves; Crossref's title says "Dynamic GPU *Voxel* Grids", the PDF says "Sparse Grids"; first draft said "no DOI found" |
| Hoetzlein, "GVDB: Raytracing sparse voxel database structures on the GPU", High Performance Graphics 2016 (Eurographics/ACM SIGGRAPH Symposium) | [10.2312/hpg.20161197](https://doi.org/10.2312/hpg.20161197) | 9 pp., read whole 28 Nov 2026 | not in Crossref; DataCite has it (title, author, 2016, the HPG series), checked 28 Nov 2026 |
| Crassin, Neyret, Lefebvre, Eisemann, "GigaVoxels", I3D 2009, pp. 15–22 | [10.1145/1507149.1507152](https://doi.org/10.1145/1507149.1507152) | 8 pp. | resolves; the PDF prints no DOI |
| Laine, Karras, "Efficient sparse voxel octrees", I3D 2010, pp. 55–63 | [10.1145/1730804.1730814](https://doi.org/10.1145/1730804.1730814) | the I3D paper (not the technical report read in the first pass) | resolves |
| Lindstrom, "Fixed-rate compressed floating-point arrays", IEEE TVCG 20(12):2674–2683, 2014 | [10.1109/TVCG.2014.2346458](https://doi.org/10.1109/TVCG.2014.2346458) | 10 pp. | resolves |
| Di, Cappello, "Fast error-bounded lossy HPC data compression with SZ", IPDPS 2016, pp. 730–739 | [10.1109/IPDPS.2016.11](https://doi.org/10.1109/IPDPS.2016.11) | 10 pp. | resolves |
| Tao, Di, Liang, Chen, Cappello, "Optimizing lossy compression rate-distortion from automatic online selection between SZ and ZFP", IEEE TPDS 30(8):1857–1871, 2019 | [10.1109/TPDS.2019.2894404](https://doi.org/10.1109/TPDS.2019.2894404); preprint [10.48550/arXiv.1806.08901](https://doi.org/10.48550/arXiv.1806.08901) | arXiv v2 (Jan 2019), 14 pp. | journal DOI found by Crossref title search; both resolve |
| van Wingerden, "Real-time ray tracing and editing of large voxel scenes", MSc thesis, Utrecht University, 25 June 2015 (supervisor J. Bikker) | [handle 20.500.12932/20460](https://hdl.handle.net/20.500.12932/20460) (resolves to Utrecht's student-theses page with this title and author) | 36 pp. | no DOI |
| Karis, Stubbe, Wihlidal, "Nanite: a deep dive", SIGGRAPH 2021 Advances course | no DOI: <https://advances.realtimerendering.com/s2021/Karis_Nanite_SIGGRAPH_Advances_2021_final.pdf> (HTTP 200) | 155 slides, partly (§9) | |
| Amanatides, Woo, "A fast voxel traversal algorithm for ray tracing", Eurographics 1987 | no DOI known | first pass | |
| `PNanoVDB.h` in the OpenVDB repository, `master`, 25 Nov 2026 | <https://github.com/AcademySoftwareFoundation/openvdb/blob/master/nanovdb/nanovdb/PNanoVDB.h> | read: configuration, buffer reads, node constants | a file on a moving branch |
| `emcfarlane/webgpu-nanovdb`, `emcfarlane/picovdb` | GitHub | READMEs only | §9 |
| binary-greedy-meshing README; zfp `docs/source/modes.rst` | GitHub | read in the original | confirmed the first draft's summaries |

Not cited above but in the first draft and not re-read: the Aaltonen and Haar 2015 slides and the
second-hand web pages listed in §9.
