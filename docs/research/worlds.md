# Worlds: procedural and real-data world generation, consistent in mass across levels of detail

Research note for **S24 "Worlds"** (the core), **S26 "Worlds: the Earth"** (open data) and the
planet-ready parts of **S25** (the engine core), 28 Nov 2026. It cites S4's chunks and region files
([S4](../slices/S4.md) §2), S5's world octree, walk, ledger and bases ([S5](../slices/S5.md) §2;
[s5-world-octree.md](s5-world-octree.md), here [O]), and S6-07's density functions
([s6-density-and-skinning.md](s6-density-and-skinning.md), merged while this note was written). That
note agrees on the two points this one builds on: value and improved Perlin noise are polynomials
inside each lattice cell (degree ≤ 6 per axis), so Gauss–Legendre on cells split at the lattice planes
gives their moments exactly; and noise is deterministic with integer hashing and `+ − × floor` only.
§3 adds the constraint that makes a *coarse* node's moments exact without visiting the fine cells.

Markers: **[R]** primary text read (by me or by a research agent whose page and section are given);
**[S]** abstract or secondary source only; **[M]** measured here (scripts in [`worlds/`](worlds/));
**[D]** derived here, not measured. Every URL below was read on 28 Nov 2026 unless marked.

---

## 0. Summary

**The gravity-critical finding [M].** A coarse octree node made straight from a noise recipe (the
usual "band-limited summary": evaluate only the octaves the node can resolve) misses the mass of the
fine terrain it stands for by 6×10⁻⁵ to 4×10⁻² of the node's mass. That is **0.03 to 0.26 rad a node**
at 250 m to 4 km from a tower, against a µrad budget ([§3.3](#33-measured)). The fix is in the noise, not
in the summary: **make every octave integrate to zero, with zero horizontal first moments, over aligned
blocks of four of its own lattice cells.** Then every octree node is a union of whole blocks of every
octave finer than it, and those octaves add nothing to its mass or its horizontal centroid. The node's
mass and first moments come exactly, to round-off (worst 6×10⁻¹⁶ relative, measured), from the coarse
octaves alone, at O(1) work a node.

What is left is the vertical moment `ρ/2 ∫h²`, which is not linear in the octaves: 10⁻⁴ to 5×10⁻⁴ rad a
node when the summary stops at the node's own scale. It falls about fourfold for each finer octave the
summary includes, reaching 3×10⁻⁷ rad a node at four octaves finer [M]. It is bounded and carried as a
**generation line in the ledger**. The walk opens a node whose line is too large, so generation is
driven by the read's tolerance. The constraint costs a few per cent of generation time once the
lattice values are tabled per tile [M].

**Recommendation, in one paragraph.**
- **Recipe as data.** A world is a recipe of layers written as data (Minecraft's 1.18+ density
  functions are the precedent: JSON expressions, a noise router, climate parameters for biomes).
- **Determinism.** A chunk is a pure function `f(seed, recipe hash, key)` on integer cell
  coordinates. Hashes are integer only (pcg3d, Jarzynski & Olano 2020), and the arithmetic is IEEE
  basic operations only: no platform `sin`. Rust documents its transcendentals as non-deterministic;
  WebAssembly's add, mul, div and sqrt are exact-rounded. The same chunk then has the same bits on
  every thread count and in the page, and neighbouring chunks agree by construction.
- **Levels of detail.** Octaves are mass- and dipole-preserving. Level-0 columns carry their exact
  column integral (partial fill), so "merged = unmerged" holds across generation.
- **Infinite flat worlds.** S5's plane plus departures; departures with zero mass per largest block
  have a far tail that falls faster than S5's `2πGσ/R` [D].
- **Planets.** S5's planet-centred Cartesian octree for mass; a cube-sphere quadtree (S2's quadratic
  projection: basic operations only, area ratio 2.1) as the surface index for generation, tiles and
  streaming.
- **Earth.** Open data with each data cell's mean kept: Copernicus GLO-30 via a CORS-enabled mirror,
  the Environment Agency's 1 m LiDAR via its WCS, OSM and Overture buildings, WorldCover, BGS and
  Macrostrat, all credited.
- **Validation.** An independent prism sum (Nagy; Harmonica's `prism_layer`, BSD-3), Bouguer and
  Hammer checks, and bit-identity across threads and wasm.

**What I could not verify.**
- The mass-preserving construction is tested on a flat height field of value noise. It is not yet
  tested with ridged or billow shaping (nonlinear), with 3-D density caves, with strata that cross,
  or on a sphere ([§3.5](#35-what-is-not-linear-and-how-it-is-bounded)).
- Rust's speed for the generator is estimated, not measured.
- Determinism in the page is argued from the specifications, not run.

Each has a test in the slice ([§9](#9-validation)).

---

## 1. The recipe architecture

A world is a **recipe**: an ordered list of layers, each a small typed record in the scene spec (RON
and JSON, schema-driven as every spec is), plus a seed. Nothing generated is the source of truth; the
recipe and the user's edits are.

| Layer | What it holds | Mass role | Example |
|---|---|---|---|
| **base** | `Patch` (S4), `Plane { surface, density, thickness }`, `Sphere { radius, layers }` (S5) | analytic, read in closed form | `Plane { z0: 0, ρ: 2670 }` |
| **shape** | a height field `h(x, y)` and/or a 3-D density `d(x, y, z)` (`d > 0` solid), as expressions | departures from the base | fBm of 10 octaves, warped; cheese caves |
| **strata** | surfaces `b_k(x, y)` below the shape, each with a material of real density | column mass `Σ ρ_k (b_{k−1} − b_k)` | topsoil 1600, chalk 2200, granite 2700 kg m⁻³ |
| **features** | rivers, lakes, caves, aquifers: carved or filled regions | signed (water 1000, air −ρ) | a river channel along a flow path |
| **structures** | buildings, roads, bridges, from rules or data (S26); prefabs placed by rules | pieces (S4) and instances | a village on flat ground near water |
| **detail** | small mass-preserving octaves, surface colour jitter | zero mass per block by construction | 10 cm roughness |
| **edits** | the user's changes as a sparse delta over the generated chunks (S25's save format) | exact, from the edited cells | dug foundations |

**Precedent.** Minecraft made its generator data-driven in 1.18. Density functions "make up
mathematical expressions to obtain a number from a position" and are JSON files in a data pack
([Density function](https://minecraft.wiki/w/Density_function) [R]). The types include `noise`,
`add`, `mul`, `clamp`, `abs`, `spline`, `range_choice`, `interpolated` and `cache`. A **noise router**
names the functions the generator reads: `final_density` (> 0 is solid), the aquifer's floodedness
and spread, and the six climate parameters (`temperature`, `vegetation`, `continents`, `erosion`,
`depth`, `ridges`) ([Noise router](https://minecraft.wiki/w/Noise_router) [R]). Noise settings carry
sea level, the vertical range, the router and the material rules
([Noise settings](https://minecraft.wiki/w/Noise_settings) [R]).

Cavendish should take the shape of this (expressions as data, a router, a staged chunk pipeline) and
not its numbers. Two differences:
1. **Every expression declares whether it is linear in its noise.** That decides whether a coarse
   node's moments are exact or bounded (§3).
2. **Every expression declares a bound.** It gives a range over a box (interval arithmetic on the
   expression tree), so S4's top-down `cover` can say Inside, Outside or Mixed without sampling every
   cell. Today's `Terrain::cover` does this with a Lipschitz bound
   (`crates/cav-core/src/voxel/world.rs`).

**Stages, per chunk** (Minecraft's statuses are the model, [Chunk format](https://minecraft.wiki/w/Chunk_format)
[R]: `structure_starts`, `structure_references`, `biomes`, `terrain`, `features`, `initialize_light`,
`light`, `spawn`, `full`; since 26.3 `terrain` merges the older `noise`, `surface` and `carvers`):
1. **structures placed**: per region, from the seed; a structure's footprint is known before terrain;
2. **climate and biome**: 2-D fields;
3. **shape and strata**: the voxels and their materials;
4. **features**: carving and filling;
5. **structure pieces stamped**;
6. **summaries** (S5's `Summary`);
7. **edits applied**.

Stages 1 and 4 may read a halo of neighbouring regions; every other stage is pointwise.

**What each layer costs in the spec.** One recipe is a few kB. A world of 10¹¹ cells saved as recipe
plus delta is the recipe plus the edited chunks only (S25, TASK-S24-07).

## 2. Determinism: the same chunk, bit for bit, everywhere

**Requirement.** Seed + recipe + chunk key give the same chunk bytes:
- on every thread count;
- natively on Windows, Linux and macOS;
- in the wasm page.

S3 already demands this of trees (R1) and S5 of the octree (O3). Generated worlds extend it to the
generator.

**What breaks it, and the rule for each.**

| Hazard | Source | Rule |
|---|---|---|
| Transcendentals (`sin`, `exp`, `atan2`, `powf`, …) | Rust: "The precision of this function is non-deterministic. This means it varies by platform, Rust version, and can even differ within the same execution from one invocation to the next" ([f64 docs](https://doc.rust-lang.org/std/primitive.f64.html), under "Unspecified precision" on `sin`, `cos`, `exp`, `ln`, `powf`, `atan2`, `hypot`, … [R]) | **None in a generator.** Use polynomials (the quintic fade), integer hashes, `sqrt` (exact-rounded: "guaranteed to be the rounded infinite-precision result … guaranteed not to change", same page [R]), or a software implementation compiled into the crate. A clippy-style lint lists the banned calls in `cav-core::gen` |
| Fused multiply-add | `mul_add` is exact-rounded fused [R, same page]; the compiler does not contract `a*b+c` in Rust | allowed only where written; never relaxed SIMD |
| Wasm floats | "Floating-point arithmetic follows the IEEE 754 standard … round-to-nearest ties-to-even"; `fadd`, `fsub`, `fmul`, `fdiv`, `fsqrt` return the exactly rounded result; NaN payload and sign are non-deterministic; "The result of relaxed operators are implementation-dependent" ([Wasm 3.0 numerics](https://webassembly.github.io/spec/core/exec/numerics.html) [R]; [profiles](https://webassembly.github.io/spec/core/appendix/profiles.html) [R]) | no NaN may reach the output; no relaxed-simd; then native and wasm agree |
| Summation order | float addition is not associative | fixed order (Morton, then octant), as S3's R1 and S5's O3; per-thread partials merged in key order |
| Chunk-local float coordinates | `origin + i·pitch` rounds differently in each chunk, so neighbours disagree at a shared face | positions from **integer cell indices** (`i64`) times a **dyadic pitch** (2⁻ᵏ m): exact in `f64` up to 2⁵³ cells; lattice indices by integer division when octave spacings are power-of-two multiples of the pitch |
| Hash quality | weak hashes make visible lattice patterns | **pcg3d / pcg4d**: "a good default choice for multidimensional high-quality hash functions" on the quality and speed Pareto frontier, with about 0 BigCrush failures, built from 32-bit multiply, add, xor and shift only ([Jarzynski & Olano 2020, JCGT 9(3)](https://jcgt.org/published/0009/03/02/) §6–7, pp. 30–33 [R]). 64-bit cell keys are folded in as the fourth component (pcg4d) |

The current world terrain uses `f64::sin` (`Terrain::height`, `crates/cav-core/src/voxel/world.rs`
l. 754–767), so **today's world is not guaranteed the same in the page as natively**. TASK-S24-05
replaces it with an integer-hash octave and adds the control test: a generator that calls `sin` is
refused by the lint.

**The noise.** Value or gradient noise on an integer lattice, interpolated by Perlin's quintic fade
`6t⁵ − 15t⁴ + 10t³`, which "has zero first and second derivatives at both t=0 and t=1". The old cubic's
second derivative `6 − 12t` "is not zero at either", which shows as creases in shaded normals
([Perlin 2002, "Improving Noise", §2–3](https://mrl.cs.nyu.edu/~perlin/paper445.pdf),
[DOI 10.1145/566654.566636](https://doi.org/10.1145/566654.566636) [R]).

The quintic is a polynomial, so it is deterministic and its integrals are exact rationals: the
property §3 uses (`∫₀¹ f = 1/2`, `∫₀¹ u f = 5/14`). Gradient noise uses Perlin's 12 cube-edge
gradients ("avoid the axes and long diagonals", same paper §3 [R]). Noise "is completely characterized
by its power spectrum"; lattice noises are only weakly band-limited; wavelet noise (`N = R − R↓↑`) is
almost perfectly band-limited ([Lagae et al. 2010, CGF 29(8)](https://scribblethink.org/Work/LLCDDELPZ10SPNF.pdf) §2.1,
§3.2.1 [R]; Cook & DeRose 2005 [S]). §3 explains why band-limiting is the wrong tool for mass.

## 3. Levels of detail with mass consistency (the gravity-critical part)

### 3.1 The requirement

S5 builds a coarse node's `Summary` bottom-up from level-0 chunks: "a coarse node is **never built by
voxelising at coarse pitch** (it would violate 'merged = unmerged')" ([O] §3). With a generator that
rule means:
- **(a) generate the finest level and sum it**, which is exact and costs the finest cell count; or
- **(b) produce the node's moments straight from the recipe**, which costs O(1) a node but carries an
  error unless the recipe is built for it.

At planet scale (a), at 5 cm everywhere, is impossible. A kilometre square at 5 cm is 4×10⁸ columns,
about 10 s on one thread at an estimated 25 ns a column [D]; at a cm-scale bubble over 10 km rings
it does not fit at all. So (b) is needed beyond the bubble, and its error must go in the ledger.

What the node must match. S5's summary carries:
- raw moments about the node's centre to third order: mass `M₀`, first `M_x, M_y, M_z`, second and
  third;
- the absolute moments `A_k` for the bound ([S5](../slices/S5.md) §2).

The read's sensitivity, for scale [D]: a mass error `δm` at horizontal range `R` from a tower moves the
5 m gradiometer by about `k_eff T² · G L δm / R³ = 1.6 δm / R³` rad (kg, m). µrad at 1 km allows
`δm ≈ 600 kg`, against a 256 m node's ~10¹⁰ kg: 6×10⁻⁸ relative.

### 3.2 Candidate methods

| Method | Mass of the node | Cost a node | Fine data consistent? | Sources |
|---|---|---|---|---|
| **A. Generate finest, sum** (S5 today) | exact by construction | all fine columns under it | yes, to the bit | [O] §3 |
| **B. Band-limited summary**: evaluate only the octaves coarser than the node, sampled at coarse pitch (mip-mapping noise, wavelet noise) | wrong by the dropped octaves' integral over the node: random, mean zero, not bounded below its amplitude × area | O(1) | **no**: a step when the walk opens the node | Lagae 2010 §2.1 (band-limiting as spectral control) [R]; Cook & DeRose 2005 [S] |
| **C. Analytic integrals of every octave** over the node (exact for polynomial fades) | exact | O((S/s_o)²) for each octave finer than the node, i.e. as much work as A for fine octaves | yes | [D] |
| **D. Mass- and dipole-preserving octaves** (this note): each octave constrained to zero mass and zero horizontal first moments on aligned blocks of `K × K` lattice cells (K = 4) | **exact**: octaves finer than the node contribute zero | O(1) a node for mass and first moments; the vertical moment by exact quadrature of the included octaves | mass and first moments to round-off; vertical and higher moments within a stated bound | [M] §3.3; the area-to-point idea of Kyriakidis 2004 [R] and Tobler 1979 [S] (§6.4) |

**How D works.**
1. An octave's lattice values `w` come from the hash. Blocks are aligned with the octree, since
   octave spacings are dyadic.
2. In each block, the interior `(K−1)²` lattice values are corrected by the least-norm shift that
   zeroes three linear functionals of the octave over the block: its integral and its two horizontal
   first moments.
3. Boundary values are shared with neighbouring blocks and left untouched, so the surface stays
   continuous and no grid of zeros appears.
4. The functionals are exact: with the quintic fade, a node's hat integrates to 1 (interior) or ½ (an
   end), and its first moments are `i − K/2`, `1/7 − K/4` and `5/14 + (K/2 − 1)/2`
   (`docs/research/worlds/lod_mass.py`, `_constraints`). The correction is a fixed 3 × 9 matrix: basic
   operations only, so it is deterministic.
5. A point's height needs only its own block's 25 hashes. **No dependency crosses a chunk edge**, so
   seams are unaffected (§4).

### 3.3 Measured

`worlds/lod_mass.py` ([output](worlds/lod_mass.out.txt)) [M]. The setup:
- **Terrain:** a flat world's height field above `z₀ = 0`: fBm of value noise, 10 octaves of
  spacing 2048 m down to 4 m, amplitude 100 m halving per octave, SplitMix64 lattice hashes (an
  integer hash standing in for pcg3d), ρ = 2670.
- **Nodes:** sides S = 64, 256 and 1024 m at R = 250, 1000 and 4000 m (S/R = ¼), three random nodes each.
- **Truth:** every octave by exact Gauss–Legendre quadrature (6 × 6 points a 4 m cell; the
  integrands are polynomials on it).
- **Summary:** the octaves whose block is larger than S, by the same exact quadrature on their own
  lattice.
- **Phase:** the gradiometer phase of a point mass at the moments' centroid, summary against truth
  (the mass and the dipole and vertical-moment parts at monopole level; the higher moments are S5's
  own business).

Criteria written before running: C1, mass-preserving kinds, `|δm/m| ≤ 1e-12`; C2, control: standard
octaves miss mass by ≥ 1e-6; C3 recorded.

| Octaves | worst `|δm/m|` | worst `|δ(first moment)| / (m S)` | worst phase a node at 250 m / 1 km / 4 km (rad) |
|---|---|---|---|
| standard (method B) | 4.3×10⁻² (least 6.4×10⁻⁵) | 1.7×10⁻¹ | 0.22 / 0.19 / 0.029 |
| mass-preserving only | 6.3×10⁻¹⁶ | 4.5×10⁻² | 0.021 / 0.048 / 0.023 (all from the horizontal dipole) |
| **mass + dipole preserving (D)** | **3.9×10⁻¹⁶** | **7.0×10⁻¹⁵** | **5.0×10⁻⁴ / 7.5×10⁻⁵ / 9.1×10⁻⁵** (all from the vertical moment) |

C1 passes (6.3×10⁻¹⁶), C2 passes (least 6.4×10⁻⁵), and the dipole constraint makes the horizontal
part exact (7×10⁻¹⁵). A ring holds about 2πR/S ≈ 25 such nodes; their vertical residuals are mostly of
one sign (`∫d²` ≥ 0), so the ring sum is up to 25 × the node value.

**How many octaves must the summary include?** `worlds/lod_depth.py` ([output](worlds/lod_depth.out.txt))
[M] includes every octave with spacing above `S/(K·2^j)`, for `j` = 0 to 4. Expectation, written
first: about ×4 a step, the dropped octaves' squared amplitude. Worst over the three nodes, vertical
part (rad):

| R, S | j = 0 | 1 | 2 | 3 | 4 |
|---|---|---|---|---|---|
| 250 m, 64 m | 5.0×10⁻⁴ | 4.2×10⁻⁵ | 2.5×10⁻⁵ | 0 (all octaves) | 0 |
| 1 km, 256 m | 7.5×10⁻⁵ | 2.6×10⁻⁵ | 6.7×10⁻⁶ | 1.5×10⁻⁶ | 3.6×10⁻⁷ |
| 4 km, 1024 m | 9.1×10⁻⁵ | 2.4×10⁻⁵ | 4.7×10⁻⁶ | 1.2×10⁻⁶ | 3.2×10⁻⁷ |

About ×4 a step at 1 and 4 km as expected; noisier at 250 m, where single octaves dominate. The work
a node is the quadrature over the finest included octave's lattice, `(K·2^j)²` cells: 4,096 at j = 4.
That is against 2.6×10⁷ columns for a 256 m node at 5 cm, about 6,000× less [D].

**Cost of the constraint [M].** numpy, 10 octaves, lattice values tabled per 1 km tile:
- standard: 3.9 µs a column;
- mass and dipole preserving: 4.1 µs (+5 %);
- mass only, done naively: 6.9 µs (it re-hashed per point).

The constraint's cost is per block, amortised once the block table exists. Rust: an estimated 10–30
ns a column for 10 octaves (four table reads, one fade and one lerp an octave), unmeasured [D].

### 3.4 The design

1. **Octaves are mass and dipole preserving by default** (an option `preserve: Mass | MassDipole |
   None` per octave, default `MassDipole`). `None` is allowed for looks only, inside the
   generate-and-sum radius, and the ledger says so.
2. **Inside the bubble and the inner rings** (the cut's nodes at level 0 to `L_g`, set by the tier):
   method A, generate the finest and sum, exactly as S5 builds today.
3. **Beyond them, a node is generated directly at its level.**
   - Mass and horizontal first moments: exact, from the octaves coarser than its block size.
   - Vertical moment, second and third moments: by exact quadrature of the octaves down to `j`
     levels finer.
   - The rest bounded: `|δM_z| ≤ ρ Σ_dropped (a_o ‖H − H_lin‖_∞ + ½ a_o²) S²`, with `a_o` the octave's
     bound (its amplitude times the correction's norm) and `H − H_lin` the included field's departure
     from its best plane over the block [D].
   - This becomes the node's **generation line** in its bound `B` (S5's walk adds it to the
     truncation bound).
4. **The walk opens a node whose generation line is too large.** The generator then makes its eight
   children at the next level (each again direct, or summed from level 0 inside `L_g`). Generation is
   demand-driven by the read's own priority queue (GigaVoxels' "data demanded by the traversal", [O] §3).
5. **Level-0 consistency: columns carry their exact integral.** A voxelised column's mass is
   `ρ pitch² × (cells below h)`, which misses up to half a cell a column (worst over a 256 m node at 5
   cm, `ρ·A·pitch/2 ≈ 4×10⁶ kg`, about 7×10⁻³ rad at 1 km [D]). So the top cell of each column is a
   **partial fill** whose fraction is the exact column integral of the height over the cell's
   footprint, from the same polynomial quadrature. This is the "mass anti-aliasing" of
   [exact-skin-and-partial-fill.md](exact-skin-and-partial-fill.md), applied to generated columns.
   Then summed level 0 equals the direct summary to round-off, and the switch between them makes no step.
6. **Edits** are departures added on top (S5's raw-moment deltas up the path), so they never disturb
   the generated summaries.

### 3.5 What is not linear, and how it is bounded

- **Ridged and billow shaping** (`1 − |n|`, `|n|`; Musgrave's ridged multifractal squares and weights
  each octave, [§5](#5-minecraft-style-generation)). These are nonlinear in the lattice values. Use
  the correction on the **shaped** octave's output:
  - subtract three fixed bump functions (zero on the block's boundary, so continuity holds) scaled by
    the shaped octave's block integrals;
  - those integrals come from a fixed quadrature that is part of the recipe's definition.
  - Mass is then exact against that quadrature, and the quadrature's own error (|·| has a kink) is
    bounded by the octave's Lipschitz constant times the cell. Unmeasured: TASK-S24-08 measures it.
- **3-D density (overhangs, caves).** The solid region `{d > 0}` is not linear in anything. Caves are
  limited to a depth band and a volume fraction `φ_max` per recipe. A node's bound for its caves is
  `ρ φ_max V_band` in mass, with the matching moment bounds. If that is too large the walk opens the
  node, so caves near towers are always generated and summed. Minecraft's noise caves (cheese,
  spaghetti, noodle) and its carvers ([Cave](https://minecraft.wiki/w/Cave) [R]) are both this case.
- **Strata.** Column mass `Σ ρ_k (b_{k−1} − b_k)` is linear in the surfaces, so mass-preserving
  surfaces give exact mass and dipole, as long as surfaces do not cross. Where they cross (clamped)
  the clamp's contribution is bounded as for |·| above.
- **Planets.** Column mass is `ρ ∫ (r³ − R³)/3 dΩ`: linear plus `h²` and `h³` terms, with an area
  element that varies across a cube face. Take the constraints with the area element (the rows
  integrate against the face's Jacobian at fixed quadrature points). The linear part is then exact;
  the `h²/R` terms are bounded as the vertical moment is. Unmeasured: TASK-S24-11.

## 4. Seamless chunks

Seams come for free if the generator is a pure function of **global integer** coordinates (§2):
neighbours evaluate the same lattice values at the same points. Three places need care:

1. **Shared faces.** A test generates every pair of neighbouring chunks independently (different
   threads, different orders) and compares the shared face's cells, materials and fills bit for bit.
   The control computes positions chunk-locally in floats and must mismatch.
2. **Region stages with a halo** (structures, erosion). They are computed per region tile (e.g. 8³
   chunks) from the tile and a halo of fixed width, with all randomness from the tile key. A tile
   never depends on a neighbour's *output*, only on the recipe over its halo. This is the "divide and
   conquer" tiling of Šťava et al. 2008: tiles "calculated independently … synchronized at borders"
   (Fig. 8 [R]). To keep the read's summaries exact, an erosion tile's height change is made
   mass- and dipole-free per block before it is applied (§3.2 D): erosion then only moves mass inside
   blocks, and never changes a coarse node [D].
3. **Draw seams** between LOD rings are S5's (TASK-S5-21, dithered cross-fade); the read has no
   seams, only the partition test ([O] §1, W1).

## 5. Minecraft-style generation

| Element | Definition (source) | Notes for Cavendish |
|---|---|---|
| **fBm** | `t += a·noise(f·x); f *= 2; a *= G`, `G = 2^−H`; spectrum `f^−β`, `β = 2H+1`; H = 1 (−9 dB/octave) fits mountains ([Quilez, fbm](https://iquilezles.org/articles/fbm/) [R]); Musgrave's `fBm(H, lacunarity, octaves)` (Texturing & Modeling, ISBN 1-55860-848-6, ch. 14–20 [S]; code read through Blender's port `node_musgrave_texture.osl` [R]) | Quilez advises lacunarity 1.99 or 2.01 and rotating the domain each octave so that peaks do not align. Mass-preservation needs **dyadic aligned** lattices: use per-octave integer lattice offsets and axis permutations instead of non-dyadic lacunarity, and measure visible alignment in the demo [D] |
| **Ridged multifractal** | `signal = (offset − |noise|)²·weight; weight = clamp(signal·gain, 0, 1)` (Musgrave via the Blender port [R]) | nonlinear: §3.5 |
| **Hybrid multifractal / billow** | `signal = (noise + offset)·pwr; weight *= gain·signal` (same [R]); billow = `|noise|` | nonlinear: §3.5 |
| **Domain warp** | `f(p + h(p))`, e.g. `fbm(p + 4·fbm(p + 4·fbm(p)))` with arbitrary offsets ([Quilez, warp](https://iquilezles.org/articles/warp/) [R]) | the warp moves where octaves are sampled, so per-block integrals of a warped octave are no longer the lattice's. Warp only the *coarse* octaves (those summed by exact quadrature anyway), or treat as §3.5 |
| **Derivative-damped fBm ("erosion-like")** | `n = noised(p); d += ∇n; a += b·n/(1 + d·d)` with a rotation each octave ([Quilez, morenoise](https://iquilezles.org/articles/morenoise/) [R]) | cheap valleys without a simulation; nonlinear (§3.5) |
| **3-D density, overhangs and caves** | `final_density > 0` is solid; cheese caves ("largest"), spaghetti ("long, narrow, winding"), noodle ("1 to 5 blocks"), plus carvers that "frequently cut straight through surface blocks" ([Cave](https://minecraft.wiki/w/Cave), [Noise router](https://minecraft.wiki/w/Noise_router) [R]); aquifers decide "what fills every non-solid block" on 16 × 12 × 16 cells (Cave § Aquifer [R]) | caves bounded per node (§3.5); worm caves as swept capsules along hashed random walks (a structure stage with a halo) |
| **Biomes from climate noise** | six parameters (temperature, humidity, continentalness, erosion, weirdness, depth) as "a six-dimensional (6D) space"; each biome an interval box; outside all, "the closest biome interval" ([World generation](https://minecraft.wiki/w/World_generation) [R]); since 1.18 "Biomes no longer control the terrain height" | a biome picks strata, surface materials and densities. Biome *blending* must be a partition of unity in the recipe so that mass stays linear |
| **Erosion** | thermal (talus relaxation) and hydraulic with a sediment capacity (Musgrave, Kolb & Mace 1989 [S], via Galin et al. 2019 §4.1, §4.3 [R]); virtual-pipe shallow water with capacity `C = K_c sin α |v|` (Mei et al. 2007 [S], as restated by Šťava et al. 2008 §5.1 [R]: `C_k` 10⁻⁴ for rock to 0.1 for sand); the stream-power law `∂h/∂t = −k A^m s^n + u` (Cordonnier et al. 2016 [S], via Galin 2019 §4.2.1 [R]) | thermal and pipe erosion are **local** (tileable, §4) and conserve terrain + sediment; stream power is **global** (drainage area needs the whole catchment) and **not** mass-conserving (detachment-limited). Use pipe or thermal erosion per region tile, made mass-free per block (§4) |
| **Settlements and roads** | the surveyed categories: terrain, vegetation, roads, cities, buildings (Smelik et al. 2014 [S]); Galin 2019 §2.2 on volumetric landforms as open [R] | place on low slope near water; roads by least-cost paths over the coarse terrain; buildings as prefabs with real floor and wall masses (§6.5) |

## 6. The Earth from open data (S26)

The owner's ruling (28 Nov 2026): use open data freely, credit every source, and keep a credits file.
All CORS tests below sent `Origin: https://example.github.io` (28 Nov 2026).

### 6.1 Elevation

| Source | Access | Format, resolution | Datum | Licence and credit | CORS (tested) |
|---|---|---|---|---|---|
| **Copernicus DEM GLO-30 / GLO-90** | `s3://copernicus-dem-30m`, `-90m` (eu-central-1, no account); STAC `copernicus-dem-30m-stac.s3.amazonaws.com` ([AWS registry](https://registry.opendata.aws/copernicus-dem/), [readme](https://copernicus-dem-30m.s3.amazonaws.com/readme.html) [R]) | COG, DEFLATE, 1024² internal tiles; 30 / 90 m; a **DSM** (buildings, trees); absolute vertical < 4 m (90 %) ([CDSE](https://dataspace.copernicus.eu/explore-data/data-collections/copernicus-contributing-missions/collections-description/COP-DEM) [R]) | WGS84; vertical **EGM2008** | "Licence for the use of the Copernicus WorldDEM-30"; modified use credit: "produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved", and its liability notice passed on ([licence PDF](https://documentation.dataspace.copernicus.eu/APIs/SentinelHub/Data/DEM/resources/license/License-COPDEM-30.pdf) [R]) | **AWS: none** (preflight 403). **Mirror with CORS:** `https://opentopography.s3.sdsc.edu/raster/COP30/COP30_hh.vrt` (ACAO echoes origin). The AWS bucket is the 2021 release: Armenia and Azerbaijan 404 |
| **SRTM GL1 v3** | Earthdata login (LP DAAC); OpenTopography API needs a key; public mirror `opentopography.s3.sdsc.edu/raster/SRTM_GL1/` ([Earthdata](https://www.earthdata.nasa.gov/data/catalog/lpcloud-srtmgl1-003) [R]) | 1″, 60° N to 56° S, voids filled from ASTER | not stated on the page | "openly shared, without restriction" (EOSDIS) | LP DAAC: login (no). SDSC mirror: yes |
| **AWS Terrain Tiles** (Mapzen) | `elevation-tiles-prod` ([registry](https://registry.opendata.aws/terrain-tiles/), [formats](https://github.com/tilezen/joerd/blob/master/docs/formats.md) [R]) | terrarium PNG `(R·256 + G + B/256) − 32768`, z0–15, Web Mercator; Oxford at z14–15 is EA 2 m LiDAR resampled (header `x-amz-meta-x-imagery-sources`) | not stated | per-source list ([attribution.md](https://github.com/tilezen/joerd/blob/master/docs/attribution.md) [R]), e.g. "United Kingdom terrain data © Environment Agency copyright and/or database right 2015. All rights reserved;" | **yes** (`*`); the docs say the S3 tiles are "meant for use with EC2 resources only": a fallback, not a default |
| **ETOPO 2022** | NCEI direct GeoTIFF, THREDDS, OPeNDAP ([product](https://www.ncei.noaa.gov/products/etopo-global-relief-model), [user guide](https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/docs/1.2%20ETOPO%202022%20User%20Guide.pdf) [R]) | 15″ (288 tiles), 30″, 60″; surface and bed | **EGM2008** (a geoid tile is supplied) | "freely available to use for all private, academic, or commercial purposes"; cite "NOAA National Centers for Environmental Information. 2022: ETOPO 2022 15 Arc-Second Global Relief Model … https://doi.org/10.25921/fd45-gt74" | **yes** (`*`) |
| **GEBCO_2026** | download app, CEDA `dap.ceda.ac.uk/bodc/gebco/global/gebco_2026/`, OPeNDAP ([GEBCO](https://www.gebco.net/data-products/gridded-bathymetry-data), [terms](https://www.gebco.net/data-products/gridded-bathymetry/terms-of-use) [R]) | 15″ netCDF / GeoTIFF; ~7 GB global | "assumed to be referred to mean sea level" | "placed in the public domain"; acknowledge the source; cite "GEBCO Bathymetric Compilation Group 2026 (2026). The GEBCO_2026 Grid … doi:10.5285/4f68d5c7-45eb-f999-e063-7086abc036fa" | yes (`*`), but zips: subset via OPeNDAP |
| **EA LiDAR Composite DTM/DSM** | Survey Data Download portal; **WCS 2.0.1** `environment.data.gov.uk/spatialdata/lidar-composite-digital-terrain-model-dtm-1m/wcs` ([data.gov.uk DSM 1 m](https://www.data.gov.uk/dataset/cf3f1137-c12b-44a1-a835-e80fe4a60b92/lidar-composite-digital-surface-model-dsm-1m) [R]) | 1 m (and 2 m), ~99 % of England, ±15 cm RMSE, annual; a 100 × 100 m GetCoverage over Oxford returned float32 heights 56.4 to 63.8 m | **ODN** via OSTN15 | Open Government Licence; "© Environment Agency copyright and/or database right 2022. All rights reserved." (one CKAN DTM record says "No Licence Provided"; the DSM record says OGL) | **yes** (`*`) |
| **OS Terrain 50** | OS Downloads API (no key) ([overview](https://docs.os.uk/os-downloads/products/land-and-terrain-portfolio/os-terrain-50/os-terrain-50-overview.md) [R]) | 50 m, GB, 4 m RMSE | not stated | OS OpenData: "Contains OS data © Crown copyright [and database right] [year]." | yes. OS Terrain 5 is not open |
| **EGM2008** (far field, S14 and S27) | ICGEM `icgem.gfz.de` table: model 104, degree 2190, `.gfc` 252 MB ([ICGEM](https://icgem.gfz.de/tom_longtime) [R]); geoid grids via GeographicLib (1′: 470 MB, cubic error 2.2 mm max) ([GeographicLib](https://geographiclib.sourceforge.io/C++/doc/geoid.html) [R]) | coefficients; Pavlis et al. 2012, doi:10.1029/2011JB008916 | — | NGA's terms **not verified** (site timed out); ICGEM states none on the table | **no** (native only) |

**Datums.** Copernicus and ETOPO are on EGM2008, the EA LiDAR on ODN and GEBCO on mean sea level.
A world mixes them only through one stated vertical reference: ODN ↔ ETRS89 by OSGM15, then EGM2008
(not checked here; a test in TASK-S26-02). A DSM (Copernicus) carries buildings and trees, so it
must not be used under OSM buildings: the DTM (EA) or a DSM-to-DTM correction goes there, or
buildings are counted twice.

**Range reads from the page.** The Fetch standard makes a simple `Range: bytes=a-b` header
CORS-safelisted, so cross-origin COG reads need only the ACAO header
([Fetch](https://fetch.spec.whatwg.org/) [R]). Which browsers implement this was not checked. Terrain
Tiles and ETOPO do not expose `Content-Range`; OpenTopography's mirror does.

### 6.2 Buildings, roads, water

| Source | Access | Content | Licence and credit | CORS |
|---|---|---|---|---|
| **OpenStreetMap** | Overpass `overpass-api.de` (≤ 10,000 queries and 1 GB a day, one script at a time, pause 30 s on 429 or 406: [wiki](https://wiki.openstreetmap.org/wiki/Overpass_API) [R]); Geofabrik PBF (UK 2.3 GB, daily); Protomaps PMTiles (spec v3: Hilbert tile IDs, range reads, [spec](https://github.com/protomaps/PMTiles/blob/main/spec/v3/spec.md) [R]) | Simple 3D Buildings: `height` (ground to roof top), `min_height`, `building:levels` (above ground, without roof levels), `roof:shape` (flat, gabled, hipped, …), `roof:height`, `roof:levels`, `building:part` (the outline is then not rendered) ([wiki](https://wiki.openstreetmap.org/wiki/Simple_3D_Buildings) [R]); only **5.7 %** of `building=*` have levels and **3.35 %** height ([taginfo](https://taginfo.openstreetmap.org/api/4/key/combinations?key=building), queried 28 Nov 2026 [R]); `highway`, `railway`, `waterway`, `natural=water`; `lanes` on 7.5 % of highways, `width` on 1.3 % | **ODbL** ([copyright](https://www.openstreetmap.org/copyright) [R]); credit "© OpenStreetMap contributors", linked to openstreetmap.org/copyright, visible in a corner or a credits screen ([attribution guidelines](https://osmfoundation.org/wiki/Licence/Attribution_Guidelines) [R]). Share-alike for derived databases (for information) | Overpass **yes** (`*`, simple POST, no custom headers); Geofabrik no; Protomaps daily builds no, the Source Cooperative mirror `data.source.coop/protomaps/openstreetmap/v4.pmtiles` yes |
| **Overture Maps** | GeoParquet `s3://overturemaps-us-west-2/release/<release>/theme=buildings/type=building/` and `building_part`; PMTiles `overturemaps-extras-us-west-2…/tiles/<release>/buildings.pmtiles` (not for production); monthly, kept 60 days ([cloud sources](https://docs.overturemaps.org/getting-data/cloud-sources/), [calendar](https://docs.overturemaps.org/release-calendar/) [R]) | `height`, `min_height`, `num_floors`, `num_floors_underground`, `roof_shape`, `roof_height`, materials; OSM first, then Esri Community Maps and the ML footprints (Microsoft, Google) at IoU > 0.5 ([buildings guide](https://docs.overturemaps.org/guides/buildings/) [R]) | buildings, base, transportation: **ODbL**, "© OpenStreetMap contributors. Available under the Open Database License." plus Esri Community Maps (CC BY 4.0) and "Data from TomTom." ([attribution](https://docs.overturemaps.org/attribution/) [R]) | **S3 yes**, with range preflight allowed; Azure no |
| **Microsoft Global ML Building Footprints** | index `minedbuildings.z5.web.core.windows.net/global-buildings/dataset-links.csv`, gzipped GeoJSONL by quadkey ([repo](https://github.com/microsoft/GlobalMLBuildingFootprints) [R]) | 1.4 B footprints; 194 UK tiles; `height` (null where not measured since 2026-11-12) | **CDLA Permissive 2.0** (not ODbL) | no (native only) |
| **Google Open Buildings** | GCS, Earth Engine ([site](https://sites.research.google/gr/open-buildings/) [R]) | Africa, South and South-East Asia, Latin America: **not the UK** | CC BY 4.0 or ODbL, user's choice | no |

### 6.3 Land cover, geology, densities

| Source | Access | Content | Licence and credit | CORS |
|---|---|---|---|---|
| **ESA WorldCover** | `s3://esa-worldcover/v200/2021/map/` 3° COGs ([AWS registry](https://registry.opendata.aws/esa-worldcover-vito/), [access](https://esa-worldcover.org/en/data-access) [R]) | 10 m; 11 classes (10 tree, 20 shrub, 30 grass, 40 crop, 50 built-up, 60 bare, 70 snow, 80 water, 90 wetland, 95 mangrove, 100 moss) | CC BY 4.0; "© ESA WorldCover project 2021 / Contains modified Copernicus Sentinel data (2021) processed by ESA WorldCover consortium" | **no** (preflight 403). In the page use Overture's base theme (land cover derived from it, CORS yes) |
| **BGS Geology 625k** | OGC API `ogcapi.bgs.ac.uk/collections/bgsgeology625kbedrock/items?bbox=…` ([page](https://www.bgs.ac.uk/datasets/bgs-geology-625k-digmapgb/) [R]) | bedrock, superficial, dykes, faults as GeoJSON | OGL | yes |
| **BGS Geology 50k** | WMS `map.bgs.ac.uk/arcgis/services/BGS_Detailed_Geology/MapServer/WMSServer` ([page](https://www.bgs.ac.uk/datasets/bgs-geology-50k-digmapgb/) [R]) | bedrock, superficial, artificial ground; GetFeatureInfo as GeoJSON | WMS under OGL, "Contains British Geological Survey materials copyright UKRI [year]"; **vector data paid** (£0.23 per km² commercial) | yes |
| **BGS Land Gravity Survey** | CSV via NGDC deposited data ([metadata](https://metadata.bgs.ac.uk/geonetwork/srv/api/records/9df8df51-6404-37a8-e044-0003ba9b0d98/formatters/xml) [R]) | UK mainland, 1–2 stations per km², readings and reduced values | **conflicting in the record**: OGL ("Contains NERC materials ©NERC [year]") and "BGS Digital Data Licence terms" | not tested |
| **Macrostrat** | `macrostrat.org/api/v2/geologic_units/map?lat=&lng=`, `/columns`, `/defs/lithologies` | units at a point (passes BGS 625k through in the UK); no densities; columns mostly North American | CC BY 4.0; "Peters, S.E. et al. Macrostrat: a platform for geological data integration and deep-time research. https://macrostrat.org, accessed on (date)" | yes (`*`) |

**Densities.** No open table of British rock densities was found: the BGS Geophysical Properties
Databank needs permission and has not been maintained since 2004
([data.gov.uk](https://www.data.gov.uk/dataset/geophysical-properties-databank) [R]). The recipe
carries a lithology-to-density table with a range per entry. Uncertainty enters bundles as a
parameter range for Gradar (§10), not as one number. The crustal default is 2670 kg m⁻³, as
Harmonica's topographic correction uses (§9).

### 6.4 Upsampling that keeps each data cell's mean

A 30 m DEM under a 5 cm voxel world needs detail added; the detail must not move each data cell's
mass. This is exactly §3.2 D with the data as the coarsest octave.

1. **The smooth part.** Use an interpolant whose mean over every data cell equals the datum. This is
   *pycnophylactic* interpolation: Tobler 1979 [S] solves Laplace's equation under the constraint
   that each region's integral equals its datum. Area-to-point kriging is the geostatistical
   generalisation: "the areal average … of point predictions within any arbitrary area informed by an
   areal-average datum is equal to that particular datum", with a standard error at every point and
   Tobler's method as a special case ([Kyriakidis 2004, Geogr. Anal. 36(3)](https://ktisis.cut.ac.cy/bitstream/20.500.14279/14363/1/A%20geostatistical%20framework.pdf)
   abstract, §1 [R]). The histospline is the 1-D form (Boneva, Kendall & Stefanov 1971 [S]).
   - Simplest exact version [D]: the quintic-fade interpolant of the cell values, corrected per data
     cell by the same least-norm shift of §3.2 so that its integral (and first moments) over each
     cell equals the datum's.
2. **The fractal part.** Mass- and dipole-preserving octaves below the data's spacing, with amplitudes
   from the data's own spectrum (the fBm `H` fitted to the DEM's power spectrum: Quilez's
   `β = 2H + 1` [R]).
3. **Point samples or cell means?** GLO-30 and the EA composite are posted grids (samples), not
   averages. Treat each posting as its cell's mean and say so: an error of order `curvature × cell²`
   [D]. Kyriakidis's framework handles either support.

A test (TASK-S26-03): each data cell's mean ≤ 1e-12 relative after upsampling; control:
unconstrained detail moves it.

### 6.5 Buildings with plausible interiors

- **Height.** Use `height` where present. Otherwise use `building:levels × storey + roof`, with storey
  3 m: the OSM wiki's "default floor height used for 3D renderings"
  ([Key:building:levels](https://wiki.openstreetmap.org/wiki/Key:building:levels) [R]); OSM
  Buildings uses 3 m, OpenMapTiles 3.66 m and OSM2World 2.5 m. Otherwise use a regional default from
  Overture or Microsoft heights. The source of each building's height goes in the bundle.
- **Mass.** Model floors and walls, not a uniform block:
  - a slab per storey and perimeter walls, of materials whose masses match the building's **material
    intensity** per m² of gross floor area;
  - the deQo medians are residential 654 kg/m² (98–1882) and commercial 1034 (224–2317), as quoted by
    Heeren & Fishman 2019 ([Sci. Data, DOI 10.1038/s41597-019-0021-x](https://pmc.ncbi.nlm.nih.gov/articles/PMC6480936) [R]);
  - medians computed by the agent from that paper's open CSV (gross-floor-area rows) are: concrete
    multi-family 1160 (IQR 989–1352), masonry multi-family 1512, timber 680–710, masonry single-family
    2392, non-residential 1376 kg/m². These are **our computation, not published values**.
  - At 3 m a storey that is 230–800 kg m⁻³ averaged over the envelope [D]. A uniform block of that
    density has the right mass but the wrong distribution: a 2 m-tall slab gap reads differently
    within ~10 m of a tower.
- **Roofs.** Extrude by `roof:shape` and `roof:height` as polyhedra (S5's exact pieces), so the
  roof's mass is exact.
- **Roads and rail.** Use the S4 road piece along OSM ways, its width from `width`, else `lanes ×
  3.5 m`, else a class default (an assumption, recorded).

### 6.6 The tile cache, offline

- **Native.** Use a content-addressed cache outside the repo (S4's region cache is the model): a key
  of source, product, version or ETag and tile, holding the raw bytes as fetched plus the voxelised
  chunks keyed by recipe hash and voxeliser version (S4's TASK-S4-16). With the network off, the
  cache is read and what is missing is said.
- **Page.** Use the Cache API or OPFS (bounded, said on the page as S4's memory cache is). Only
  CORS-enabled sources are used: the OpenTopography COP30 mirror, the EA WCS, ETOPO, Overpass,
  Overture S3, Protomaps on Source Cooperative, BGS OGC API, Macrostrat. The others (AWS Copernicus,
  WorldCover, Microsoft footprints, Geofabrik, ICGEM) are native-only. A re-host would be publishing
  data and is not proposed.
- **Credits.** Use `CREDITS.md` (source, licence, link, the credit line verbatim, share-alike noted
  for information) plus the in-app list of the sources in use, in the view's corner and in bundles
  (TASK-S26-07).

## 7. Planets

**Keep S5's split** ([O] §4.2):
- the mass lives in a **planet-centred, axis-aligned Cartesian octree**, so the cube kernel, prism
  near-kernel and Morton keys apply unchanged, and the layered sphere is read in closed form;
- a **cube-sphere** is the right *index* for the surface: generation tiles, elevation tiles,
  streaming and the draw's far LOD. This note agrees and adds the projection choice.

| Projection | Area ratio, max/min | Cost and determinism | Source |
|---|---|---|---|
| gnomonic (TSC), linear | ~5.2 | basic operations | Calabretta & Greisen 2002 §5.6.1 ([arXiv:astro-ph/0207413](https://arxiv.org/abs/astro-ph/0207413) [R]); S2 `s2coords.h` [R] |
| tangent / equiangular (Ronchi et al. 1996 [S]) | 1.4 | needs `atan`/`tan`: **not deterministic** in Rust (§2) | S2 `s2coords.h` [R] |
| **S2 quadratic** | **~2.1** (2.09 at level 10, 2.11 at 30) | basic operations and `sqrt`: deterministic; ~3× faster than tangent | [S2 cell hierarchy](https://s2geometry.io/devguide/s2cell_hierarchy), [cell statistics](https://s2geometry.io/resources/s2cell_statistics), `src/s2/s2coords.h` [R] |
| COBE quad-cube (CSC, QSC) | ~equal area (CSC rms 1.06 %, max +13.7 %); QSC exactly equal area but "non-differentiable along the diagonals" | polynomial fit (CSC) | Calabretta & Greisen 2002 §5.6.2–3 [R]; Chan & O'Neill 1975 [S] |
| HEALPix | exactly equal area, 12 base pixels, iso-latitude rings | not a cube; 12-face quadtree | [Górski et al. 2005](https://arxiv.org/abs/astro-ph/0409513) §4–5 [R] |

**Recommendation: the S2 quadratic projection for the surface index.** It is deterministic with
basic operations, its area variation is bounded (2.1) and it has a quadtree per face. A 64-bit tile
key `(face 3 bits, level 5 bits, i, j)` addresses level 28, i.e. about 3 cm on the Earth [D]. The
plane is the one-face case of the same key space (TASK-S24-04).

Points to settle:
- **Surface noise on a sphere.** Evaluate the low octaves in 3-D at the direction vector times the
  radius (seamless across face edges by construction). Evaluate the mass-preserving detail in face
  coordinates, with lattice values on face edges hashed from their 3-D integer position so both faces
  agree. Constraints are taken with the area element (§3.5). This is the riskiest part: measured in
  TASK-S24-11 by the face-edge bit test.
- **Precision and local frames** ([O] §4.2; S23):
  - node centres from integer keys times a dyadic pitch (exact);
  - records relative to their node centre;
  - the node-to-tower vector in `f64` on the CPU (float-float on the GPU, S23);
  - a 64-bit key covers ±2⁶² cells: at 1 cm, 4.6×10¹⁶ m [D].
  - S25 makes this real on the CPU: `town` reads and draws the same at 10⁷ m (TASK-S24-03).
- **The gravity read.** As S5: the layered sphere as point masses by the shell theorem plus crust
  departures read by the walk. The far planet beyond the crust octree is S14's and S27's spherical
  harmonics, with the crust's own harmonic content removed to avoid double counting (S27's exit).

## 8. Infinite flat worlds

S5's `Plane`: an infinite slab (zero gradient, uniform `g`) plus departures. For a generated
unbounded world:
- **Tiling.** There is none. The noise is aperiodic over `i64` cell keys, and pcg4d folds the high
  key bits in (§2).
- **The far field.** S5's tail bound for unresident departures is `2πGσ/R` with `σ ≤ ρH` ([O] §4.1).
  For a generated world with **every octave mass-preserving, the coarsest included**, the departures
  have zero mass and dipole over every block of the coarsest size `B_max`. Each such block is then at
  least a quadrupole source, and a uniform ring of them beyond `R` falls as about `1/R³` rather than
  `1/R` [D]. The horizon can then be set by the tolerance instead of by `H`.
- **To measure in TASK-S24-10:** the tail against an explicit larger world, with the bound ≥ the
  measured change as the horizon grows.

## 9. Validation

| Test | Asserts | Against | Control arm |
|---|---|---|---|
| `generated_chunks_are_deterministic` | the same chunk bytes on 1, 2, 6 threads and in the page (node runs the wasm build) | itself | a generator using `f64::sin` is refused by the lint; chunk-local float coordinates differ |
| `neighbours_agree_on_shared_faces` | every shared face's cells, materials and fills bit-equal, chunks made apart and in shuffled order | the neighbour | chunk-local float coordinates mismatch |
| `generated_nodes_carry_their_children_s_mass` | a level-L node generated directly = the sum of its level-0 children: mass and first moments ≤ 1e-12 relative; vertical and higher moments within the generation line | method A | `preserve: None` octaves miss mass ≥ 1e-6 (measured here at ≥ 6.4×10⁻⁵) |
| `the_generation_line_is_sound` | the line ≥ the measured residual at every node of town-sized and 10 km worlds | method A | the line with the dropped `a_o²` term removed underestimates somewhere |
| `generated_terrain_reads_as_prisms` | the read of a generated height field = an independent prism sum of the same column heights within the ledger at 1e-6 rad, ≥ 8 tower positions | Nagy 1966 prisms (eq. 7, p. 364 [R]; Nagy, Papp & Benedek 2000 in the library) in the oracle; Harmonica `prism_layer` + `prism_gravity` (BSD-3; [docs](https://www.fatiando.org/harmonica/latest/api/generated/harmonica.prism_layer.html) [R]) as a script cross-check | a prism sum on heights shifted by half a cell fails |
| `a_plateau_is_a_bouguer_slab` | a flat plateau of height h and radius a: `g` → 2πGρh as a grows, the gradient → 0 | `bouguer_correction` = 2πGρh (Harmonica [R]); Hammer 1939 eq. 1, p. 185 [R] | — |
| `hammer_zones` | a station on a generated hill: terrain correction positive and by zone | Hammer 1939's zone table, p. 190 ("always positive", p. 186) [R] | — |
| `terrain_correction_split` | Bouguer slab + terrain correction = the full topographic read | Forsberg 1984 via Hirt 2015 (the RTM residual to the slab) ([Hirt 2015](https://ddfe.curtin.edu.au/models/ERTM2160/pdf/Hirt2015_ForwardModelling_Encyclopedia_av.pdf) [R]); Forsberg's report itself [S] | — |
| `site_terrain_beside_bgs` (S26) | Oxford's terrain effect from the DEM, beside BGS's reduced gravity at the stations within 5 km | BGS Land Gravity Survey, if its licence allows (§6.3) | recorded, not gated: BGS values carry geology the DEM does not |
| `generation_speed` | ns a column and chunks a second per stage, native and page | recorded | — |
| `the_read_is_unchanged` | every S5 scene's read the same within its ledger (validation report against the previous tag) | the report | — |

Every case runs on a town-sized world until the bigger world (the owner's cap, 28 Nov 2026).

## 10. Gradar (S11 hooks)

- **Sites from seeds.** A dataset entry is `(recipe, seed, site parameters)`. The manifest stores the
  recipe hash, so `cav verify` regenerates the world bit for bit (§2) and the scene with it.
- **Ground truth.** The recipe *is* the truth. Per site the bundle records:
  - strata densities and their ranges (§6.3);
  - the generated summaries' ledger (§3);
  - the mass and moments within each ring.
- **Variety.** Domain randomisation over biome climate parameters, octave amplitudes and `H`, strata
  densities within their ranges, structure density, and edits. The real-data site (S26) is the
  held-out test, as a real site should be.
- **Split by seed family, never by chunk** (ML-AND-API's splits that never leak): two sites from
  neighbouring seeds of the same recipe share no chunks, but sites cut from one world do.

---

## Recommendation and reasons

1. **Mass- and dipole-preserving octaves by default.** They are the only method found that gives a
   coarse node the fine data's mass at O(1) cost, and they are validated against a known answer
   (method A, the summed fine data) to 6×10⁻¹⁶ [M]. The band-limited alternative errs by up to
   0.26 rad a node [M].
2. **Generate-and-sum inside the bubble and inner rings, direct generation beyond, the gap in the
   ledger.** This keeps S5's "merged = unmerged" where it is checked. The walk decides where to
   generate.
3. **Integer hashes and basic operations only.** Rust's own documentation rules out `sin` for
   bit-identity, and wasm's IEEE rules make basic operations safe.
4. **Partial-fill top cells for generated columns.** Without them, voxel quantisation alone errs by
   up to ~7×10⁻³ rad a node at 1 km [D].
5. **A cube-sphere with S2's quadratic projection as the surface index, mass in S5's Cartesian
   octree.** Deterministic, and every kernel is unchanged.
6. **Earth data from CORS-enabled sources in the page, everything natively, all credited.**

## What I could not verify, and how to settle it

- **Nonlinear shaping (ridged, billow, warp), caves and crossing strata** under the correction:
  measured only for linear value noise. *Settle:* `the_generation_line_is_sound` in TASK-S24-08 and
  -09, on each shaping.
- **Spheres**: the area-element constraints and face-edge agreement. *Settle:* TASK-S24-11's
  face-edge bit test and the planet residual.
- **Rust speed** of the generator (estimated 10–30 ns a column) and the walk-driven generation's
  latency. *Settle:* `generation_speed`.
- **Wasm bit-identity**, argued from the specification. *Settle:* the determinism test run in node.
- **The 1/R³ tail** of mass-preserving infinite worlds (§8) [D]. *Settle:* TASK-S24-10.
- **Visual cost of dyadic, aligned octaves** (Quilez advises non-dyadic lacunarity). *Settle:* the
  demo, with per-octave offsets and permutations.
- **Licences not read in full:** NGA's EGM2008 terms (site unreachable), ICGEM's, the GLO-90 text
  (assumed as GLO-30), the BGS Land Gravity licence (conflicting), OpenTopography's bucket terms.
- **Primary texts read only second-hand:** Cook & DeRose 2005, Mei et al. 2007, Cordonnier et al.
  2016, Musgrave et al. 1989, Tobler 1979, Smelik et al. 2014, Forsberg 1984, Ronchi et al. 1996,
  Chan & O'Neill 1975. Their DOIs are in `papers-to-fetch.md`.

## Further questions (not chased)

1. Should the detail layer be wavelet noise (band-limited, Cook & DeRose) *and* mass-preserving? The
   two constraints are compatible in principle (subtract the coarse projection, then the block
   moments).
2. Can the vertical moment be made exact as well, by a constraint on `∫ H d` (linear in `d` given `H`)?
   Then only `∫d²` would remain, and that is known in expectation.
3. Erosion at planet scale: is a mass-free-per-block erosion visually convincing, or must sediment
   cross blocks?
4. ODN against EGM2008 at Oxford (OSGM15), to the centimetre.
5. Whether the BGS 50k WMS GetFeatureInfo may be used in bulk under its OGL statement.

## Files

- [`worlds/lod_mass.py`](worlds/lod_mass.py), [`worlds/lod_mass.out.txt`](worlds/lod_mass.out.txt):
  the three kinds of octave, mass, first moments and phase per node, cost.
- [`worlds/lod_depth.py`](worlds/lod_depth.py), [`worlds/lod_depth.out.txt`](worlds/lod_depth.out.txt):
  the residual against the number of included octaves.
