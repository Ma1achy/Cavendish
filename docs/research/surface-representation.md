# The cheapest surface representation for the gravity read, at a given accuracy

Research note, 28 Nov 2026, for S5 (the world octree and its bounded walk) and later slices. The
owner's question: what is the cheapest way to represent a body's surface for the read at a given
accuracy, measured in accuracy, FLOPs, memory, and CPU and GPU time? Everything below is measured
unless tagged otherwise. Scripts, data, bench code and figures:
[`surface-representation/`](surface-representation/). The pass criteria were committed before any
measurement ([`CRITERIA.md`](surface-representation/CRITERIA.md), commit `2dd5f86`).

Tags: **[M]** measured here (a script and its JSON); **[Mo]** modelled (measured parts × counts, e.g.
cost per point = Σ elements × their measured ns); **[D]** derived (argument, no run).

## Answer in brief

1. **The winning boundary leaf is new to this project: the PLIC piece plus the multipole of what it
   misses.** Store the cell's exact moments (fill, centroid, second, third: 80 B in f32) and an
   8-bit-per-component plane normal (2 B); rebuild the cube ∩ half-space piece from them (its offset
   comes from the fill) and add the multipole, through l = 3, of the exact piece minus the PLIC
   piece. On all five shapes at the finest pitch this is 10⁻¹¹–2×10⁻⁹ rad at 0.5 m and
   ≤ 4×10⁻¹¹ rad at 1.5 m, 30–270 times better than the third-moment leaf at 0.5 m and at the
   reference's floor (~10⁻¹³ rad) from 5 m; its order in h is 5–6 against 4–5 for the moments
   ([`data/ladder.md`](surface-representation/data/ladder.md)) [M]. It costs one PLIC piece
   (0.5–0.8 µs per point on the CPU, at most 10 vertices, 15 edges, 7 faces) plus 20 ns. Neither
   ingredient alone is good: PLIC alone is *worse* than fill + centroid (its centroid is wrong by
   up to 10 % of h on edges), and moments alone are poor within a few cells.
2. **Exact clipped pieces are the expensive rung and buy nothing past the near field.** Real pieces
   have a median of 28–56 triangles (90 %: 36–80; max 44–190; 16–30 welded vertices) [M], so
   2–18 µs per piece per point on the CPU; the PLIC + residual leaf reaches the floor from 1.5–5 m
   at a tenth of that.
3. **Refine the surface adaptively by measured error, not uniformly.** With each cell's error known
   at build time (the voxeliser has the exact piece), choosing per cell between moments, PLIC +
   residual, exact, or refining, meets 10⁻⁸ rad at 1.5 m for 2–60 µs of surface work per point,
   against 25 ms for uniform finest exact pieces and 0.4–6 ms for the whole mesh (§2) [Mo].
4. **In the walk, the a-priori bounds hold but are pessimistic.** Every accepted node's measured
   error was within its bound (worst 0.8 of it) at every shape, tower and budget, and the control
   (one order too many) breaks it [M]. But the measured total error comes out 10²–10⁴ below the
   budget, so a bound-driven walk opens far more than an error-driven tree needs (§3). Tight
   radii (b from the contents' box, not the cell's cube) cut the walk's cost 2–3×; a BVH another
   1.4–2.4× at 5–20 m, and nothing near (§5).
5. **Free boxes with an l = 6 series win far and lose near.** Extending the box series to l = 4 and
   6 is cheap (25 and 78 flops, 6–16 ns; checked against 40 digits, C9) and makes 144 free boxes
   ~30× cheaper than 4,657 octree blocks for the CAD part's interior at 5–20 m, but elongated boxes
   (median aspect 17) still need their closed form near the body, where octree blocks stay cheaper
   (§4).
6. **GPU: float-float works and is cheap; f32 is wrong.** On the GTX 1660 SUPER the soup-triangle
   kernel runs at 3–17 ns per triangle per point in float-float (error 10⁻¹² rad) and 5–6 ns in
   native f64 (SHADER_F64 is available; 123–138 GFLOP/s against 4.3–5.0 TFLOP/s in f32), against
   75–95 ns on one CPU core. Plain f32 is wrong by 10⁻² rad. **The driver removes Dekker's
   error-free transforms unless they are guarded** with a runtime 1.0 (measured: error term 0
   without, exact with), so a WGSL float-float library must carry that guard (§6) [M].

**Recommendation for S5** (details in §7): boundary leaves store exact moments to third order plus
an 8-bit octahedral normal; the read uses moments far, PLIC + residual near, exact pieces only
inside a measured bubble (≲ 0.5 m at 10⁻¹⁰ rad); refinement is by measured per-cell error at build
time; node radii are tight; the walk's budget split needs work before the bound-driven walk is
competitive with the measured-error tree.

## 0. Set-up

- **Read.** The AION-10 flight integral with a static source, exactly as
  [`paper-flight-phase/flight.py`](paper-flight-phase/flight.py) (which matches `cav` to 2×10⁻¹⁵ rad,
  that note's C1), at the engine's quadrature (Gauss order 6, panels ≤ 0.1 s): 102 nodes × 4 arm
  points = 408 potential evaluations per tower, phase = Σ cᵢ V/G(pᵢ)
  ([`common.py`](surface-representation/common.py) `tower_points`). Towers stand with their base 0.5,
  1.5, 5 and 20 m from the surface along (1, 0.2, −0.3) from the bounding-box centre (the shapes
  study's direction). ρ = 2700 kg/m³.
- **Shapes** (the shapes study's meshes, downloaded and hashed there, not committed): the
  5,120-triangle icosphere (radius 1 m), the 64 × 32 torus (R = 1, r = 0.4 m), the Stanford bunny
  (69,664 triangles), the mk7 CAD part (5,280) and the decimated dragon (49,138, the thin shape),
  the last three 1 m across.
- **Lattices.** Four nested pitches per shape, every finer lattice inside the coarsest
  (1/8–1/64 m for the 1 m bodies, 1/4–1/32 m for the sphere and torus), voxelised with exact pieces
  by `shapes_lib.voxelise` ([`prep.py`](surface-representation/prep.py)); 9,187–23,086 boundary cells
  at the finest.
- **Errors** are the sum over boundary cells of (representation − exact piece), so the interior
  cancels. The reference floor is the same sum taken with two different exact kernels (triangle
  soup and welded polyhedron): 2×10⁻¹⁵–2.5×10⁻¹³ rad.
- **CPU bench** [`bench/`](surface-representation/bench/) (Rust, outside the workspace, depends on
  `cav-core`): every representation of every cell read at every tower (`srbench contrib`), kernel
  times (`srbench time`, one thread, below-normal priority, the S5 builder's tests running alongside,
  so times are upper bounds) and **FLOPs counted by running each kernel once on a counting number
  type** (`real.rs`, `Ct`), not by hand.

**Checks** (criteria written first):
- **C1, the bench's kernel is Cavendish's** (`data/c1-check.json`): against `Faceted::potential`,
  3×10⁻¹⁵ on a cube and **1.06×10⁻¹³ on the 5,120-triangle icosphere, against a pre-written 10⁻¹³:
  failed narrowly**. Two summation orders over 5,120 triangles at 20 m differ by this much; the
  control (one vertex moved 1 µm) differs by 6×10⁻⁸. Not moved.
- **C2, the pieces partition the body** (`data/c2-partition.json`, CAD part, h = 1/32 m): pieces +
  interior cubes = whole mesh to 7×10⁻¹³ rad (pass). The control, one piece dropped, misses by
  2.7×10⁻⁸ rad: it fails the 10⁻¹⁰ test as it must, but the threshold I wrote for it (> 10⁻⁶ rad)
  was too high for the smallest piece.
- **C3, the PLIC piece is right**: on cells cut by a single flat face (250–10,386 per shape) PLIC
  with the mesh normal equals the exact piece to ≤ 4×10⁻¹⁴ rad per cell (pass); the control
  (normal turned 1 mrad) moves them by 3×10⁻⁹–4×10⁻⁶ rad on 14 of the 16 levels that have such
  cells, but by only 2×10⁻¹⁴–5×10⁻¹³ on three bunny and dragon levels whose flat cells are slivers
  (`data/ladder.json`, `c3_*`).
- **C4, monotone ladder at 20 m: fails on the sphere.** There + second moment (5×10⁻¹²) is worse than
  + centroid (3×10⁻¹²), the sphere anomaly the shapes study already recorded; elsewhere at 20 m the
  last rungs sit on the floor (≤ 3×10⁻¹³) and their order there is noise.

## 1. The representation ladder for a boundary cell

### 1.1 Accuracy [M]

Phase error (rad) of the boundary at the finest pitch (h = 1/64 m for the 1 m bodies, 1/32 m for the
2 m sphere and 2.8 m torus), towers at 0.5 / 1.5 / 5 / 20 m. Full tables with orders in h:
[`data/ladder.md`](surface-representation/data/ladder.md); figure
[`figures/ladder.png`](surface-representation/figures/ladder.png).

| boundary cell carries | sphere | torus | bunny | CAD part | dragon |
|---|---|---|---|---|---|
| 1. fill only (mass at the centre) | 3e-4 / 2e-5 / 1e-6 / 3e-8 | 1e-3 / 3e-4 / 3e-6 / 1e-7 | 3e-5 / 4e-6 / 2e-6 / 5e-9 | 7e-4 / 2e-4 / 3e-5 / 7e-8 | 3e-5 / 2e-6 / 5e-7 / 2e-9 |
| 2. + centroid | 3e-6 / 1e-7 / 1e-9 / 3e-12 | 1e-4 / 3e-5 / 2e-7 / 1e-8 | 3e-6 / 5e-7 / 3e-9 / 1e-10 | 5e-6 / 3e-7 / 3e-9 / 7e-11 | 8e-6 / 7e-7 / 2e-8 / 9e-11 |
| 3. + second moment | 5e-6 / 4e-7 / 2e-9 / 5e-12 | 6e-6 / 3e-7 / 6e-10 / 2e-12 | 1e-7 / 4e-9 / 2e-11 / floor | 2e-8 / 4e-10 / 1e-11 / floor | 5e-9 / 1e-9 / 1e-11 / floor |
| 3+. + third moment | 8e-8 / 7e-9 / 3e-11 / 1e-13 | 2e-7 / 8e-9 / 2e-11 / 9e-13 | 5e-9 / 2e-10 / floor / floor | 6e-10 / 1e-10 / floor / floor | 2e-9 / 1e-10 / floor / floor |
| 4. PLIC, MOF normal (f64) | 6e-7 / 5e-8 / 2e-10 / 8e-13 | 8e-7 / 1e-7 / 3e-9 / 1e-10 | 2e-6 / 5e-7 / 1e-7 / 5e-10 | 2e-5 / 6e-7 / 4e-7 / 2e-9 | 7e-6 / 1e-6 / 2e-8 / 1e-9 |
| 4. PLIC, normal 2 × 8 bit, offset from the fill | 8e-6 / 3e-6 / 4e-7 / 9e-10 | 4e-6 / 6e-6 / 1e-6 / 4e-9 | 2e-6 / 4e-7 / 1e-7 / 6e-10 | 2e-5 / 3e-6 / 9e-8 / 4e-9 | 7e-6 / 1e-6 / 4e-8 / 1e-9 |
| 4. PLIC, normal and offset stored, 8 bit each | 5e-4 / 2e-4 / 4e-5 / 3e-6 | 3e-5 / 1e-5 / 2e-5 / 2e-6 | 3e-2 / 4e-2 / 2e-2 / 2e-4 | 2e-3 / 4e-4 / 5e-5 / 4e-6 | 1e-2 / 2e-2 / 1e-2 / 2e-4 |
| 5. two planes (where the surface has two directions) | 3e-6 / 3e-7 / 4e-9 / 7e-11 | 7e-6 / 6e-7 / 2e-7 / 6e-11 | 5e-6 / 1e-7 / 2e-7 / 2e-10 | 6e-6 / 1e-6 / 3e-8 / 9e-10 | 1e-5 / 7e-7 / 9e-8 / 7e-10 |
| **4 + residual: PLIC (8-bit normal) + l ≤ 3 multipole of P − Q** | **3e-10 / 2e-11 / 9e-14 / 2e-13** | **2e-9 / 2e-11 / 6e-13 / 1e-12** | **2e-11 / 3e-13 / 3e-13 / 2e-13** | **2e-11 / 6e-13 / 5e-13 / 1e-13** | **1e-11 / 4e-13 / 8e-14 / 2e-13** |
| 6. exact clipped piece | reference | | | | |
| *reference floor* | 6e-15 / 1e-13 / 4e-14 / 5e-14 | 9e-15 / 1e-14 / 2e-13 / 3e-13 | 2e-15 / 2e-14 / 2e-14 / 5e-14 | 2e-14 / 3e-14 / 6e-14 / 2e-14 | 4e-14 / 2e-14 / 2e-14 / 9e-14 |
| *the boundary cells' own phase* | 26 / 6.2 / 2.2 / 0.15 | 28 / 5.6 / 2.6 / 0.17 | 5.2 / 1.4 / 0.19 / 0.014 | 3.3 / 0.80 / 0.11 / 0.008 | 3.7 / 0.96 / 0.13 / 0.010 |

What the table says:
- **The moment rungs behave as the skin note's §2.1 count says**, now under the real read: fill only
  is first to second order, + centroid third, + second moment fourth, + third moment fifth (fitted
  orders 4.5–5.0 at 0.5–5 m where above the floor). The third moment is worth its 40 bytes: 30–60×
  over the second at 0.5 m on four shapes, 2.5× on the dragon.
- **A plane alone (PLIC) is not a better leaf than moments.** It has the right mass but its
  centroid is off wherever the cell holds an edge, a corner or curvature (moment-of-fluid residual,
  90th percentile, 0.1–6 % of h at the finest pitch, up to 22 % at coarse ones), so its error is a
  dipole layer like fill-only's, between rungs 1 and 2. The normal choice matters little for that:
  centroid-offset, mesh area-weighted and moment-of-fluid normals are within 10× of each other.
  This is the interface-reconstruction literature's experience too: a volume fraction and a normal
  fix the interface to first order; the centroid (moment of fluid) to second (Dyadechko &
  Shashkov 2008, not read: `papers-to-fetch.md`).
- **The residual is what makes PLIC worth having.** PLIC carries the near-field *shape* (the part
  that moments, being an expansion, get wrong within a few cells); the multipole of P − Q carries
  the far field to l = 3. The remaining error is the l ≥ 4 part of P − Q, whose absolute mass is
  |P Δ Q| ≪ |P|. Two ways of adding the residual were measured: about each piece's own centroid
  (`mof_fill8_residual`, the row above) and about the cell centre (`..._common`, which has the
  a-priori bound the walk uses); they agree within 2–5× (ladder.md).
- **Quantisation: store the normal, not the offset.** With the offset re-derived from the fill
  (which the leaf stores anyway), the mass stays exact and an 8-bit-per-component octahedral
  normal costs at most 10× over f64 for PLIC alone and *nothing measurable* once the residual is
  added (it absorbs the normal's error into its moments). Storing the offset quantised breaks the
  mass: 8 bits each puts 3–34 % mass errors on the 90th-percentile cell and is 100–10⁵× worse; 16
  bits each 0.01–0.2 %; the 32-bit word (11 + 11 + 10 bits) is worse than 2 × 8 bits with the
  offset from the fill.
- **Two planes do not pay.** A second plane where the surface has two directions (2-means on the
  normals; 10–93 % of cells qualify, most at coarse pitch, the dragon most) improves PLIC by ≤ 3×
  and sometimes not at all, and its residual version is within 2× of the one-plane residual. The
  convex/concave cases, the latter as cube minus a convex piece, are both handled; the gain is
  small because the residual already carries what the second plane would.

### 1.2 Cost [M]

Per cell per evaluation point, one CPU thread (bunny, finest pitch; the other shapes within ±40 %,
`data/time-*.json`). FLOPs are counted by the `Ct` type (add, mul, div, sqrt; transcendentals
separately). Stored bytes are the format proposed, f32 unless stated; kernel bytes are what the
prepared kernel reads.

| representation | ns per cell per point | add + mul + div + sqrt | atan2 / log1p | stored bytes | kernel bytes |
|---|---|---|---|---|---|
| fill only (point) | 3.4–5.6 | 12 | 0 / 0 | 4 (+ the cell's key) | 16 |
| + centroid (point at the centroid) | 3.4–5.6 | 12 | 0 / 0 | 16 | 16 |
| + second moment | 5.8–11 | 39 | 0 / 0 | 40 | 40 |
| + third moment | 12–21 | 98 | 0 / 0 | 80 | 80 |
| PLIC piece, prebuilt polyhedron | 490–850 | 738 | 5.9 / 11.7 | 2 (8-bit normal; offset from the fill) | 1,290 |
| PLIC rebuilt from its plane at every point | 6,000–8,800 | — | — | 2 | — |
| PLIC built once (per walk, shared by the tower's 408 points) | 7,400–9,300 once | — | — | — | — |
| two-plane piece | 620–1,200 | — | — | 4 | — |
| **PLIC + residual (l ≤ 3)** | **≈ 510–870** (PLIC + 2 multipoles) | ≈ 930 | 5.9 / 11.7 | **82** | ≈ 1,450 |
| exact piece as a triangle soup | 2,200–18,000 (median 26–232 triangles × 75–95 ns) | 4,636 (bunny, 51 triangles) | 51 / 153 | 36 per triangle | 200 per triangle |
| exact piece, welded (shared vertices and edges) | 2,700–30,000 | 3,939 | 51 / 76 | 12 V + 6 F | ≈ 7,800 |
| one soup triangle (C5: 75–107 ns expected) | 75–95 (**pass**) | 94 | 1 / 3 | 36 | 200 |

- **The PLIC piece's closed form.** Cube ∩ half-space has at most 10 vertices, 15 edges and 7 faces
  (measured maximum over all 75,949 finest cells: 10 and 7). It is read with Werner & Scheeres'
  face form over polygonal faces with shared vertices and edges: one sqrt per vertex, one log per
  edge, and **one atan2 per face**: a face's solid angle is 2 arg Π_t (D_t + i h 2A_t) over its fan
  triangles, because the half-angles sum to half the face's solid angle, which lies in (−π, π] off
  the face's plane (`kern.rs`, `Poly::u`) [D]; checked against the soup kernel on every piece
  (`exact_welded` against `exact`: the reference floor row). That brings the per-point cost to
  ~6 atan2 + 12 log1p + 8 sqrt, against 51 / 153 / 153 for a median bunny piece as soup.
- **Building it costs 7–9 µs** in this unoptimised Rust (allocating clipper), about 12–18 PLIC
  evaluations, so the plane is a storage format and the piece is built once per walk and shared
  by the tower's points (or by the GPU shader itself, §6). A case table over the 256 corner sign
  patterns would make the build cheaper; not done, since it is amortised.
- **Symmetry.** Six of the seven faces lie in the cell's walls: their normals are ±eₐ, so h_f is one
  coordinate and the wall edges' in-plane normals are axis vectors. Exploiting that saves
  arithmetic but not transcendentals, which dominate; not implemented.
- **The exact pieces are far from the textbook 7–14 triangles.** Median 28–56 soup triangles per
  piece at the finest pitch (90 %: 36–80, max 44–190); the clipper fans every cap segment, and the
  scans put tens of mesh triangles in each cell. Welding shares vertices (median 16–30) and edges
  (42–84) but my welded kernel was not faster (product-trick divisions on single-triangle faces).

## 2. Adaptive surface refinement

[TABLES-2]

## 3. Representation chosen per node in the tree walk

[TABLES-3]

## 4. Free-size boxes with the l = 4 and 6 series

[TABLES-4]

## 5. BVH against the implicit octree

[TABLES-5]

## 6. GPU kernels (indicative: the 1660 SUPER is correctness-only)

Code: [`bench/src/shaders.wgsl`](surface-representation/bench/src/shaders.wgsl) (each kernel written
once in "N" arithmetic, with a prelude per precision) and
[`bench/src/bin/gpu.rs`](surface-representation/bench/src/bin/gpu.rs): wgpu 30.0.2 on Vulkan (driver
NVIDIA 581.29), one thread per field point, every thread looping over the same elements, dispatches
capped at ~8 ms and waited for one at a time; times are the wall clock of the whole run less the
pauses, best of three. Data: `data/gpu-bracket-L2.json` (748 cells, 35,196 soup triangles) and
`data/gpu-bunny-L1.json` (836 cells, 175,936 triangles); 16,320 points for timing, the four towers'
1,632 points for accuracy against the CPU f64 kernels.

**SHADER_F64 is offered** by this adapter on Vulkan. Peak rates by an unrolled FMA loop: **f32
4.3–5.0 TFLOP/s, f64 123–138 GFLOP/s** (1/36) [M]. WebGPU has no f64, so the web build needs
float-float.

| kernel | precision | ns per element per point | max error of U, relative | worst phase error per tower (rad) |
|---|---|---|---|---|
| soup triangle (exact pieces) | f32 | 0.06–0.08 | 0.3–1.8 | 6×10⁻² |
| | float-float | 2.7–17 | 1–2×10⁻¹⁰ | 6×10⁻¹² |
| | f64 | 5.2–6.1 | 0.7–2.5×10⁻¹¹ | 1×10⁻¹² |
| PLIC piece built in the shader from (lo, h, n, α) | f32 | 20–21 | 0.1–1.0 | 3×10⁻² |
| | float-float | 150–230 | 1–2×10⁻¹⁰ | 2×10⁻¹¹ |
| | f64 | 72–103 | 0.3–2×10⁻¹¹ | 5×10⁻¹³ |
| moments (point + second + third) | f32 | 0.12–0.14 | 6–9×10⁻⁷ | 5×10⁻⁷ |
| | f32 terms, float-float offsets and sum | 0.28–0.42 | 2–4×10⁻⁸ | 9×10⁻⁸ |
| | float-float | 1.0 | 4×10⁻¹⁴ | 5×10⁻¹⁴ |
| | f64 | 2.2–2.5 | 4×10⁻¹⁵ | 1×10⁻¹⁴ |

For comparison, one CPU core in f64 (`srbench time`): soup triangle 75–95 ns, PLIC prebuilt
490–850 ns, moments 12–21 ns.

- **C6 passes**: float-float is within 2.4×10⁻¹⁰ relative of the CPU f64 kernel for polyhedra and
  PLIC pieces (criterion 10⁻⁹), f32 moments within 9×10⁻⁷ (criterion 10⁻⁶). The control, the f32
  polyhedron kernel, is wrong by 30–180 % per point: with body-frame coordinates in f32 and the
  GPU's own atan2 and log, the edge terms' cancellation leaves nothing. That is 100× worse than the
  precision note's numpy-f32 estimate (10⁻⁴ rad), as that note warned for WGSL's permitted errors.
- **The driver removes the error-free transforms.** The first float-float run was no better than
  f32 (moments 1.4×10⁻⁶ relative). A probe (`gpu probe`, `data/gpu-ff-probe.json`) shows why:
  two_sum(1, 2⁻³⁰) returned an error term of 0 instead of 2⁻³⁰, and Dekker's two_prod 0 instead of
  6×10⁻⁸. Passing each intermediate through a multiplication by a runtime 1.0 (from a uniform)
  restores both exactly. WGSL permits this reassociation (§15.7.5, the precision note's caution)
  and this driver does it. Every float-float routine in Cavendish's WGSL must be guarded this way,
  and the GPU tier needs this probe as a test.
- **Speed.** Float-float soup triangles run at 2.7–17 ns per triangle per point, 5–30× one CPU core;
  native f64 at 5–6 ns, so on this card f64 is as fast as float-float for the closed form (the
  transcendentals dominate, and both emulate them). In the CPU's counted operations (94 flops + 4
  transcendentals per triangle) that is 6–35 G triangle-operations per second [Mo]. Each workgroup
  reads every element once (200 B per triangle in float-float), about 1 GB/s from cache over a run:
  the kernels are compute-bound [Mo].
- **The in-shader PLIC kernel is slow** (150–230 ns in float-float, as much as 10–60 soup triangles)
  because its corner and polygon arrays are indexed local arrays, which spill to memory, and it
  computes each edge's log on both faces. It is a correctness kernel, not a tuned one; it reads a
  cell from 32 bytes (lo, h, n, α) against 200 B × 28–56 triangles for the exact piece.
- **Moments as f32 terms with float-float offsets and sum** cost 2–3× pure f32 and are good to
  ~10⁻⁸ relative: the precision note's rule (f32 is safe for contributions below ~10⁻³ rad),
  measured on hardware.

## 7. Recommendation

[REC]

## 8. Sources

Read for this note (the owner's library; not stored in the repo):
- **Salmon, J. K. & Warren, M. S. (1994).** "Skeletons from the treecode closet." *J. Comput. Phys.*
  111, 136–155. [doi:10.1006/jcph.1994.1050](https://doi.org/10.1006/jcph.1994.1050). Read pp.
  139–143: the worst-case analysis (§3.1, Figs. 4–5); the "Absolute MAC", which accepts a cell when
  an analytic bound on its partial error is below a user tolerance (eqs. 9–10, p. 142); and the
  measured ratio of error to bound (mean 0.22, rms 0.28; Fig. 7, p. 143). The walk here is that MAC
  for the potential: the Taylor remainder of 1/|x − y| after order p is at most
  M b^(p+1)/(d^(p+1)(d − b)), the potential's analogue of their eq. (9) with B₍ₙ₎ ≤ M bⁿ [D; their
  Appendix B was not re-read]. Measured worst error/bound here: 0.2–0.8.
- **Weymouth, G. D. & Yue, D. K.-P. (2010).** "Conservative Volume-of-Fluid method for free-surface
  simulations on Cartesian grids." *J. Comput. Phys.* 229, 2853–2865.
  [doi:10.1016/j.jcp.2009.12.018](https://doi.org/10.1016/j.jcp.2009.12.018). The PLIC form
  m·x = α (their eq. 8); "a set of analytic equations … which relate the intercept of a linear
  interface with the volume fraction once the normal is known" (their ref. [7], Scardovelli &
  Zaleski, p. 2854); and that the normal is the hard part (LVIRA/ELVIRA, their ref. [6]). Here the
  offset is solved numerically (safeguarded Newton on the clipped volume, `geom.rs plic_offset`),
  not by Scardovelli & Zaleski's closed form, which was not read.
- **Rathinam, A. & Dempster, A. G. (2019).** "Octree-based mascon model for small body gravity
  fields." *J. Guid. Control Dyn.*, Engineering Note.
  [doi:10.2514/1.G004008](https://doi.org/10.2514/1.G004008). A variable-depth octree refined at the
  occupied (surface) nodes to a set depth, mascons at the leaves, the outer nodes' densities scaled
  to keep the total mass (Sec. II). That is geometric refinement to a fixed depth; §2 refines by
  measured error at the tower, and carries moments, not point masses.
- **Joldes, Muller & Popescu (2017)**, read for the precision note: the double-word algorithms
  (two_sum, Dekker's split, the accurate addition) in the shaders.
- The notes this builds on: [`exact-skin-and-partial-fill.md`](exact-skin-and-partial-fill.md)
  (§§1.1, 2.1, 3.1–3.3), [`paper-boxes-and-precision.md`](paper-boxes-and-precision.md),
  [`paper-benchmarks.md`](paper-benchmarks.md), [`paper-flight-phase.md`](paper-flight-phase.md),
  [`s5-world-octree.md`](s5-world-octree.md) and [`fluids.md`](fluids.md).

Not read, added to `papers-to-fetch.md`: Scardovelli & Zaleski 2000 (the analytic cube ∩ half-space
volume), Dyadechko & Shashkov 2008 (moment of fluid), Pilliod & Puckett 2004 (LVIRA/ELVIRA), Wald
2007 (binned SAH) and MacDonald & Booth 1990 (the SAH). The BVH is built from the common description
of binned SAH (16 bins on the longest centroid axis, cost Σ area × count), not from Wald's text.

## 9. What I could not verify

[UNVERIFIED]

## 10. Further questions

[FURTHER]

## Reproduce

In [`surface-representation/`](surface-representation/), every process at below-normal priority
with at most four threads (`low.ps1`):

```
python prep.py                        # voxelise five shapes at four pitches (.work/, ~10 min)
python series.py gen                  # the box series' κ tables (bench/src/kappa.rs)
python series.py check                # C9
cd bench; cargo build --release       # stand-alone crate; builds srbench and gpu
powershell -File run_cpu.ps1 all      # C1, C2, contrib (every rep, cell, tower), time, boxes
python ladder.py                      # data/ladder.{json,md}
python tree.py run                    # parts 2-5: data/tree-<shape>.json (~15 min per shape)
python summary.py                     # data/tables.md
bench/target/release/gpu .work/bracket 2 data/gpu-bracket-L2.json   # ~2 min of GPU
bench/target/release/gpu probe        # the float-float probe
python figures.py                     # figures/*.{pdf,png}
```
