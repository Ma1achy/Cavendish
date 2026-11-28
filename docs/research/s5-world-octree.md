# One world octree for drawing and for the gravity read (S5)

*Research note for slice S5 (and S14's wider world). Written 25 Nov 2026. No code was changed; two
small benchmarks were written and run ([`s5-world-octree/lod_walk.py`](s5-world-octree/lod_walk.py),
output in [`results.txt`](s5-world-octree/results.txt)). Tags: **[R]** read in a primary source this
session; **[S]** read only as a search-result summary; **[M]** measured here; **[D]** derived by me,
to be checked; **[U]** not verified (listed again in section 8).* Extended the same day with the owner's Barnes–Hut frame (2.1 to 2.5), view modes (7a) and seams (7b).

*Updated 28 Nov 2026:* Salmon's thesis (the parts on interaction counts, error bounds and opening criteria) and
Hernquist 1987 read from the owner's copies; Rathinam & Dempster 2019 added from a companion agent's full reading;
Salmon & Warren 1994 Eq. 9 = 62 and Fig. 7 re-checked by that agent (27 Nov). No recommendation changes; see the
block below and §8.

**Changed after the third batch of primary sources (28 Nov 2026).** Not copied into the repository:
* **Salmon 1991, *Parallel hierarchical N-body methods*** (Caltech PhD thesis, 334 pp., defended 6 Dec 1990; read:
  §4.6, Ch. 5, Ch. 6). **The `θ⁻³` argument** (S&W 1994 p. 137, Eq. 4) is this: the expected number of body–cell
  interactions is `⟨N_bc⟩ ≤ F_oc N D_avg ≤ F_oc N log₈(…N)` (Eq. 4.63, p. 93), where `D_avg` is the mean depth of a body
  in the tree (Ch. 2) and `F_oc = V_mid / V_γ` (Eq. 4.61) is the volume, in units of the cell's own, of the shell in
  which a cell is accepted but its parent is not (Fig. 4.2, §4.6.2–4.6.3, pp. 85–89). `F_oc` is "the only
  characteristic of the OpeningCriterion which influences the total number of interactions" (§6.5, p. 154). For
  the BH test it is `(4π/3)((2/θ)³ − (1/θ)³) = 28π/(3θ³)` (Eq. 6.7, p. 156); for a centre-of-mass test in the `L∞`
  metric `56/θ³` (Eq. 6.10). Body–body interactions are `≈ b_m F_oc N m` (Eq. 4.68–4.69, Table 4.1, p. 96). The
  derivation assumes bodies drawn independently from a smooth density (§4.6.5–4.6.6); §4.6.1 (p. 84) gives a
  contrived set where BH is `O(N²)`. For Cavendish this is the count per level of §6 B's rings: a shell `1/θ` to
  `2/θ` node sides about the target, i.e. `≈ 28π/(3θ³) ≈ 680` level-L nodes at θ = 0.35 *if space were full* [D: my
  arithmetic]; the town's sparse world reads 971 nodes in all levels (§6 A).
  **Error bounds** (§5.1.4, pp. 112–115): Eq. 5.57 bounds the potential's remainder after `n` terms by the first
  term times a function of `b/d`, "essentially the same result" as Greengard's; the acceleration bound Eq. 5.61 adds a
  factor `(1 − b/d)⁻¹` and roughly `n`, so "the fractional error in the acceleration ... [is] substantially greater"
  (p. 115; S&W's factor-`p` remark is this). Eq. 5.61 rests on a **conjecture** (the maximum of `|∇K_n|` at `μ = 1`),
  "verified empirically for n < 4" (p. 113); `mac_bound.py` found S&W's Eq. 62 never below the true remainder in 48
  cases, which is the evidence the note uses. Typical errors are far below the bound: averaged over direction the
  error vanishes, and the worst case is all mass at one boundary point (pp. 115–116). **On a uniform cube the lowest
  multipole whose expectation does not vanish is `n = 4`** (§5.2.2, Eq. 5.99, p. 136): an independent statement of
  the hexadecapole result of `mac_bound.py`. Contours of constant worst-case error about a cube "are not at all round.
  They follow, more-or-less, the shape of the cubical volume" (Figs. 5.8–5.10, p. 126): support for measuring
  acceptance from the node's box (least distance), not from its centre.
  **Opening criteria** (Ch. 6): the `θ > 1/√3 ≈ 0.58` failure is derived here first (§6.4, Fig. 6.3, pp. 153–155:
  the error grows as `(d − b)⁻¹` in the potential and `(d − b)⁻²` in the acceleration as the target nears the
  boundary); found "quite by accident" in a real two-galaxy run (§6.3, pp. 149–152). Edge-distance criteria "are not
  susceptible" at any θ; centre-distance ones are safe only below a θ that costs `F_oc > 28π√3` (Cartesian) or `> 56`
  (`L∞`, safe for θ < 1) (§6.6, Eq. 6.16–6.21, p. 162). The thesis compares criteria at equal `F_oc` on the
  hypothesis that they are "equally good" (p. 156), not by measured error; the error-budget MACs are S&W 1994's.
  **Nothing changes:** the least-distance acceptance, never accepting within `b_max`, and the Eq. 62 ledger stand.
* **Hernquist 1987, *ApJS* 64, 715–734** (read in full). The extra test is **forced subdivision of any cell that
  contains the particle**: without it the BH C code and TREECODE "can unintentionally include a self-acceleration"
  for large θ, when a particle sits at one edge of its own cell and the centre of mass at the other (§II, p. 718).
  Measured with and without it, "the typical error in the force computation from this effect was completely
  negligible for θ < 1.2" (§IIIc, p. 725). Salmon (§6.3, p. 151) names it as the fix for bodies *inside* the cell
  (his Y), not for targets just *outside* (his X): what S&W 1994 say, now first-hand. Cavendish's rule (never accept a
  node within `b_max` of the target, §2.1) is stronger than both. Error statistics (§IIIc, pp. 722–725): the error is
  the **mean absolute deviation** of each acceleration component from a direct sum, relative to the mean
  acceleration (the standard deviation is 30–50 % larger); the mean error is ~100× smaller than the deviation, of
  random sign; < 1 % for θ < 1 at N > 10⁴ (Fig. 6, Plummer models, monopole); falling as about `N^(−1/5)`; the
  quadrupole costs ~1.5× the monopole (p. 721) and helps most for θ < 1 (Fig. 7). Terms per particle at θ = 1: 130
  (N = 1,024) to 220 (N = 32,768); at θ = 0.5: 350 to 1,060 (p. 721). These are system-wide averages, not worst
  cases, which is why §5 takes the worst over positions.
* **Rathinam & Dempster 2019**, "Octree-based mascon model for small body gravity fields", *J. Guid. Control Dyn.*
  42(11), 2557–2567, doi:10.2514/1.G004008 (read in full by a companion agent, 28 Nov 2026; not re-read here). An
  octree built from the shape model's vertices as a point cloud, uniform-depth (Uni128, Uni256) or variable-depth
  leaves (§II.A–B); interior leaves found by per-layer alpha-shape slicing (§II.C); each leaf one point mass at its
  centroid, the boundary leaves' density scaled so the total volume matches the polyhedron (Eq. 1), the centre of mass
  left a few cm off; the field is a **direct sum over every occupied leaf** (Eqs. 2–9): no node moments, no
  multipoles, no opening criterion, the octree only sizes the leaves. Mean relative potential error on the surface
  10⁻³–10⁻⁴ (§V.A); ~1 % acceleration error at ~10⁵ mascons (Var256, §V.C); Uni256 (1.3 M occupied nodes, Table 2)
  runs 2–3× the polyhedron's time (Table 3); error ∝ `N^(−1/3)` after Werner & Scheeres (§V.C). **For S5:** no
  precedent there for moment-summed nodes with error-bound acceptance; it is the first-order baseline (0/1 fill, one
  point per leaf) that S3–S5's cube kernel and moments already go beyond.
* **Salmon & Warren 1994 re-checked** (companion agent, 27 Nov 2026): Eq. 9 is Eq. 62; the Fig. 7 caption (p. 143)
  gives mean actual/bound 0.22, rms 0.28. Consistent with the block below.

**Changed after the first batch of primary sources (25 Nov 2026, earlier).** The owner supplied the PDFs of Lengyel 2010,
Strugar 2010, Losasso & Hoppe 2004 and Laine & Karras 2010 (read with their figures; not copied into the
repository). What changed:
* **Seam blend: driven by continuous camera position, not by time** (§7b). All three primary sources make the
  transition weight a function of the viewer's position: the clipmap's `α` "is not time-based but instead tracks
  the continuous viewer position" (Losasso & Hoppe 2004, §6.2), CDLOD morphs "based on its own LOD metric", the
  3-D distance per vertex (Strugar 2010, pp. 4, 7–8), and Lengyel's geomorph "is a function of camera position
  only" (Lengyel 2010, §6.1, p. 86). The clipmap does this *although its regions snap* in whole grid steps, by
  measuring `α` from the viewer, not from the snapped region. So the dithered cross-fade stays, but its threshold
  is the camera-distance fraction across a band inset from the snapped ring edge, and the time-driven fade on
  snaps is dropped (it is kept only for a node that arrives late from the streamer). This also makes screenshots
  and the seam test deterministic.
* **Band width:** the sources use about a tenth of a level's extent (`w = n/10` grid units, L&H §6.2) to 15–30 %
  of each LOD range (CDLOD p. 4); the note's "1–2 coarse cells" was narrower than any of them and is now a
  measured parameter starting near 10 % of the ring.
* **Transvoxel facts corrected and sourced** (§1, §7b, §8): blocks are **16³ cells at every LOD**, the physical
  size doubling per level (Lengyel §4.2, p. 28; §4.4, p. 44): the same equal-cell-count pyramid this note
  recommends, not the "growing cell count" a forum summary had claimed. A transition cell has 9 full-resolution
  samples plus 4 duplicated corners, 2⁹ = 512 cases in 73 classes (§4.3, pp. 35–37; Table 4.8, p. 42), width
  `w(k) = 2^(k−2)` (eq. 4.2, p. 46), 2:1 only (§4.1, p. 28). It still does not apply to a blocky
  mesh-near/ray-march-far split (no isosurface mesh on either side of our seams).
* **Lengyel's "surface shifting" (§4.2.1, pp. 28–30) is our proud-halo problem**, named in a primary source:
  coarse surfaces must coincide with fine ones or objects "appear sunken" when the level changes. It supports the
  mass-weighted drawn bits (§7b).
* **ESVO read in full** (§1, §8): 64-bit child descriptor, children stored contiguously, contours as conservative
  slab pairs, beam optimisation, 5–8 bytes per voxel measured. It confirms the layout (contiguous children
  addressed by masks plus one relative pointer, the `Summary` array flat) and adds two optional ideas for the
  brick march (a coarse depth pre-pass, and filter radii clamped across level boundaries).
* **Skirts** are now sourced (Lengyel §2.1.1–2.1.2, pp. 6–8, which also says Ulrich's chunked LOD uses them).

**Changed after reading the primary sources (25 Nov 2026).** A second batch of the owner's PDFs (not copied into
the repository): Barnes & Hut 1986 (4 pages, a scan; read as page images), Salmon & Warren 1994 (20 pages, a scan;
read page by page, App. B's equations followed to Eq. 62), Ulrich 2002 (the SIGGRAPH course notes, 14 pages,
read in full), and a fourth file that is **not Ulrich**: Çetin Koca's 2012 Bilkent MSc thesis on hybrid
heightmap and octree-voxel terrain (174 pages; only its contents and its §2 account of Ulrich were looked at,
nothing is cited from it). Two benchmarks were written ([`mac_bound.py`](s5-world-octree/mac_bound.py),
[`cut_cancellation.py`](s5-world-octree/cut_cancellation.py)). What changed, in order of weight:
* **Cavendish's bound is the Salmon & Warren bound in kind, and theirs is not tighter.** Their rigorous error bound
  (Eq. 9 / 62, App. B) is built from the same ingredients as the S4 bound this note carries: absolute moments
  `B_(n) = Σ m |x − r₀|ⁿ` about the expansion centre, a node radius `b_max`, and powers of `b_max/d`. Their
  closed form for the whole tail adds a factor `1/(1 − b_max/d)²` and subtracts `(p+1)B_(p+2)/d^(p+2)` (using a
  lower bound on `B_(p+2)` from Hölder, Eq. 65–66); on a unit cube at `d/s` = 2 to 10 that subtraction is worth
  at most about 1.3×. **Measured on a uniform cube** (the shape of a voxel node; `mac_bound.out`): with *exact*
  absolute moments Eq. 62 is **109–4,800× the true remainder at p = 1, 38–350× at p = 2 and 6–65× at p = 3**
  (three target directions, `d/s` = 2 to 10; along the cube's axis, where the error is largest, 109–1,200×, 39–87× and
  6–9×).
  The reason is symmetry: a cube's dipole, quadrupole and octupole vanish, the first non-zero error is the
  hexadecapole, and an absolute-moment bound cannot know that. Their reported tightness (mean actual/bound 0.22, rms
  0.28, p. 143, Fig. 7) is for *clumpy particle sets* with a monopole-only expansion, where the first omitted term
  is not zero. **Cavendish's 27–457× is in the range these two effects predict** (my reading, not a proof):
  per node 6–9× at p = 3 along the axis and up to 65× along a face diagonal (this benchmark), times the loss from adding bounds by the triangle inequality: on the
  town cut `Σ|eᵢ| / |Σeᵢ|` is **3–27** (`cut_cancellation.out`, quadrupole cuts). So the looseness is mostly
  *intrinsic to a rigorous, absolute, per-node bound*, not a defect of S4's. The §6 C remark "a bound a hundred
  times tighter would cut this by a large factor" should be read as: not available rigorously. (The benchmark's "crude bound", all of a node's mass at distance `a`,
  is S&W's Eq. 70, the "weak bound" for treecodes that store no absolute moments, App. B.3, p. 155; they also note that
  a bound on the potential understates the acceleration's by up to a factor of about `p`, "an order of magnitude or
  more in practice".)
* **What can be tightened, and what it buys.** Spend the signed moments on the leading remainder term, which
  S&W's own Eq. 46–47 give exactly from the traceless moment tensor `Q_(n)`, and bound only the tail beyond it
  with Eq. 62. Measured on the cube (`mac_bound.out`, Part 2, target on the axis): at p = 3 the bound falls to
  **1.4–3.2× the true remainder (acceleration) and 1.5–3.1× (the gradient `∂g_z/∂z`)**, from 6–9×; at p = 2, 6–9×
  from 27–87×; at p = 1 it stays at 27–87× because the next two terms vanish too. It needs the signed **fourth**
  moments at every node (15 raw components, about 120 B at `f64`: the owner's further question 3 below, answered in
  the affirmative if a tight ledger is wanted). **But it buys little distance:** the error falls like `d^−(l+3)` with
  `l = 4` for a cube, so a bound 3.5× tighter lets a node be accepted about 3.5^(1/7) ≈ 1.2× closer: a modest
  saving, not the orders of magnitude µrad needs. The bound is not what makes µrad expensive; the expansion's
  order and the cached local expansion of §2.2–2.3 are. (The cube is the best case for this: real nodes of mixed
  material and partial fill do not have vanishing odd moments.)
* **The sum-of-squares ledger is not rigorous here.** S&W Eq. 16–17 add per-node bounds *in quadrature*, on the
  assumption that node errors are uncorrelated. On the town cut `√Σeᵢ² / |Σeᵢ|` is 0.05–0.94: it **under-reports the
  true error by up to 20×**, because node errors are correlated (the same ground slab, the same sign). S&W
  themselves write that the errors in the forces from the particles of one box "are not just correlated; they are
  nearly identical" (p. 143); the data here say the correlation reaches across boxes. Use the triangle sum for the rigorous ledger; report the quadrature
  sum, if at all, as an estimate.
* **The walk should be a priority queue on a global ledger, not a per-level threshold** (S&W §4.3, p. 145,
  Eq. 14–18). Open the node with the largest bound first, push its children, stop when `Σ bound ≤ Δ_tot`: this
  "Sum MAC" finds a near-minimal cut for a given total and needs no per-level share. The note's
  `ε_level = share × tolerance` (§2.1, §5) is the *absolute MAC*, which S&W argue wastes effort on small partials
  (partials span five orders of magnitude, Fig. 8, p. 144), so it is demoted: the per-level radii `r_L` become an
  *output* of the queue, and the cached table for the GPU fast path. S&W's numbers (Table I, p. 143): the
  monopole-only Sum Squares MAC at `f = 0.01` reaches maximum errors of 0.0046–0.0057 against 0.0135–0.0701 for the BH
  MAC at `θ = 0.65` with quadrupoles, at 0.64–1.02× the work. They also found quadrupole terms save at most ~7 % of
  the work for the Sum Squares MAC (§4.6, p. 148), a statement about *their* tolerance (a fraction of the total
  acceleration); Cavendish's µrad needs higher orders (§2.2), so I do not carry it over.
* **The opening distance must be the least distance, and a node may never be accepted near the target.** Barnes &
  Hut's test is `l/D < θ` with `D` the distance to the cell's *centre of mass* (p. 447). S&W (§2.1, App. A, Fig. 12)
  show this accepts, for `θ > 1/√3`, a cell whose far corner holds the mass while the target sits just outside its
  boundary: errors of 20 % and more on a satellite galaxy, which "detonates" (Figs. 13–14, pp. 149–150). Their
  fix is the minimum-distance MAC (`s/r` with `r` the distance to the cell *face*, §2.2), the best of the simple
  MACs "in the sense of best worst-case behavior for a given amount of CPU resources" (§5, p. 148). The note's "least
  distance" is that MAC; the bound itself needs `d > b_max` (Eq. 9 carries `1/(1 − b_max/d)²`) and S&W's footnote 3
  (p. 145) says the bound is infinite when the target is closer than `b_max`: such a node is **opened, never
  accepted**, whatever the tolerance.
* **Barnes & Hut's own claims, as they read** (Nature 324, 446–449): a rigorous worst-case error analysis is
  "possible" (two lumps in opposite corners maximise the quadrupole, p. 448) but is promised for "a more detailed
  paper", not given; `θ ~ 1` is accurate "to ~1 %" empirically, "with little dependence on N"; force error scales as
  the computing time to the −1.5; 4,096 bodies at `θ = 1` needed under 0.1 of the `½N(N−1)` direct interactions
  (Fig. 3 caption); about 10² interactions per particle at 10⁴–10⁵ bodies; the tree is rebuilt "ab initio at every
  time step" (p. 447). Quadrupole moments are listed as a future improvement (p. 448). "No bounded error" (Dehnen's
  footnote) is therefore true of the *criterion as used*, and is not something B&H claimed.
* **Ulrich 2002 confirms the draw-side design and its limits.** A chunk tree of static meshes; per-chunk error `δ`,
  halved per level (Eq. 1); screen error `ρ = δK/D`, `D` the distance to the *closest point of the bounding volume*
  (Eq. 2–3, pp. 6–7): the same test shape as the read's, with the same least-distance choice. Skirts are his
  preferred crack fill (§11, Fig. 4, pp. 8–9: the note's "Ulrich uses skirts" is now first-hand). His morph weight
  `t = clamp(2ρ/τ − 1, 0, 1)` (Eq. 4, p. 10) is a function of viewpoint distance only: another primary source that
  agrees with §7b. Paging (§15, p. 12): draw a node if its children are not all resident *or* `ρ ≤ τ`, and request
  the children: the note's "draw the nearest resident ancestor". **Limits he states (§9, p. 5): the data set must be
  static, preprocessing is non-trivial, and it uses more triangles than a primitive-level algorithm for the same
  screen error.** Nothing in Ulrich supports edits; the dirty-ancestor recompute of §3 stays our own design.
* **Not changed:** one tree with two cuts, equal-size nodes, error-driven opening with a ledger, shared cut and
  cached local expansion, analytic slab and sphere, dithered position-driven cross-fade.

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
  (After reading Salmon & Warren §4.3: the primary walk is a priority queue on one global ledger, worst bound
  first, and the per-level shares are its cached output; a node nearer than its radius is always opened.)
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

**Frame (owner's suggestion, section 2).** The read is a **Barnes–Hut walk opened by Cavendish's phase-error
bound** (not a fixed θ), taken **once per tower for its whole cluster of arm points and time samples** (a shared
cut, Dehnen's cell–cell criterion), with the far field gathered into a **local (Taylor) expansion** about the
tower and **cached** until an edit's path touches it. On the town, per tower per shot (40 points): leaf-exact
6.5×10⁶ interactions; Barnes–Hut per point 1.9×10⁴ (335× fewer); one shared cut 1.7×10⁴; shared cut + local
expansion ≈ 3×10³ (2,200× fewer); the cached expansion ≈ 40–100 (10⁵× fewer) plus ~10 node updates per edit.
At µrad the town's cut is as big as its cell count and the expansion needs degree ~10 (286 terms, 1.6×10⁻⁶ rad,
measured): the gain is real as the world grows and the bound tightens. **The baseline's help (2.4) is modest:**
the difference bound allows a node at about 0.7× the distance at 100 m and 0.5× at 1 km (one level coarser
beyond a kilometre); on the town it gains nothing (same nodes at the same measured error), because a 5 m
baseline does not cancel the error of nodes within ~10 baselines. **View modes and seams (7a, 7b):**
`lod_view: Camera | Tower(n) | Towers([..])` draws the tower's cached read cut (the union is the common
refinement) with a level, tower or error overlay; seams are hidden by dithered cross-fade in the brick march,
weighted by the continuous camera distance across a band inset from each snapped ring edge, as the clipmap and
CDLOD do (not time-driven, and not Transvoxel, skirts or geomorphing).

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
case; GigaVoxels and NanoVDB internals beyond abstracts (ESVO is now read in full); that float-float node offsets are
enough at 6.4×10⁶ m. Each has a measurement in the slice ([section 8](#8-what-to-test-and-what-could-not-be-verified)).

---

## 1. How others structure big voxel worlds with LOD

| System | Structure | LOD chosen by | Edit cost | Memory | Fit for Cavendish |
|---|---|---|---|---|---|
| **Fixed-size chunks per level** ("LOD chunk octree") | Every level's chunk has the same cell count (here 32³), cells double per level, 8 children per node | Distance or screen error per node; split when too coarse, merge when far | One chunk per level touched: re-aggregate the 8-children union | 1 + 1/8 + 1/64 … = **8/7** of the finest level (13 % extra) [D] | **The shape to take.** S4 already has it for bricks |
| **Geometry clipmaps** ([Losasso & Hoppe 2004](https://hhoppe.com/geomclipmap.pdf) [R]; [GPU Gems 2 ch. 2](https://developer.nvidia.com/gpugems/gpugems2/part-i-geometric-complexity/chapter-2-terrain-rendering-using-gpu-based-geometry) [R]) | Nested n × n grids (n = 255) of a mip pyramid, centred on the viewer, toroidal addressing | By ring: levels fixed by distance from the viewer; a level's active region is set empty when the viewer's height exceeds `0.4 n g_l` (§4), i.e. extent < 2.5 h; a transition band of width `w = n/10` grid units at each level's outer edge (§6.2) | Moving the viewer refills an L-shaped strip per level; coarse levels barely move (relative motion halves per level) | One float height map + normals per level; 33 % more than the finest in 2-D | S4's rings are this in 3-D. A clipmap is a *cut*, not a store: S4's rings are the draw cut |
| **Transvoxel** ([Lengyel 2010, *Voxel-Based Terrain for Real-Time Virtual Simulations*, PhD dissertation, UC Davis](https://transvoxel.org/Lengyel-VoxelTerrain.pdf) [R]) | Marching-cubes blocks of **16³ cells at every LOD**, the block's physical size doubling per level, eight blocks per parent: an octree of blocks (§4.2, p. 28; §4.4, p. 44). Seams sealed by "transition cells" | Octree traversal; a visible block is drawn when its projected size falls below a threshold (§4.4, p. 44) | Local: a transition cell reads 9 full-resolution samples on one face (§4.3, pp. 35–36) | Lookup tables only (512 cases in 73 classes, Table 4.8, p. 42); each block stores up to seven meshes (its own plus one transition mesh per face) and a secondary position for boundary vertices (p. 46) | Not needed: S4 does not mesh levels ≥ 1 (they are ray-marched bricks). Only 2:1 ratios across a seam are handled (§4.1, p. 28) [R]. The block pyramid itself is the same shape as this note's |
| **OpenVDB** ([overview](https://www.openvdb.org/documentation/doxygen/overview.html) [R]) | B+tree, default 5-4-3: leaf 8³, internal 16³ and 32³, a root map; constant *tiles* at any level | None built in (a data structure, not an LOD scheme) | Cheap; accessors must be cleared when nodes are deleted | Tiles make uniform regions free; leaves dense | The tile idea is S3's lossless merge. 5-4-3 is a very shallow tree: Cavendish's 32³ chunks are close to a 5-level VDB leaf |
| **NanoVDB** ([NVIDIA research page](https://research.nvidia.com/labs/prl/publication/nanovdb) [S]; [follow-up on editing](https://arxiv.org/pdf/2611.08665) [S]) | A pointer-free linear snapshot of a VDB, copyable to the GPU by `memcpy` | n/a | **Read-only**: topology edits need a rebuild (the 2026 follow-up adds GPU topology edits) [S] | Compact; optional quantisation | Argues for a flat, pointer-free layout of the cut's `Summary` array, which is what S3's note already chose |
| **Sparse voxel octrees** ([Laine & Karras 2010, "Efficient Sparse Voxel Octrees", I3D](https://research.nvidia.com/publication/2010-02_efficient-sparse-voxel-octrees) [R]) | Octree split into *blocks* of memory with block-relative pointers and a page header every 8 KB (§3). Each non-leaf voxel has a **64-bit child descriptor**: 15-bit child pointer, a far bit, 8-bit *valid mask*, 8-bit *leaf mask*, 24-bit contour pointer, 8-bit contour mask (Fig. 2, §3.1). A node's children are stored **contiguously**, so one pointer plus the masks addresses all eight; leaves have no descriptor. **Contours** (32 bits: a normal in 3 × 6 bits and two 7-bit plane positions) bound a voxel between two parallel planes; a voxel's shape is its cube intersected with its own and all its ancestors' contours, built greedily top-down so the original surface is always **enclosed** (§3.2–3.4) | Per ray: traversal stops when the voxel's projected size is small enough, `exp2(scale)` against a linear function of `t` (Fig. 8 line 11, §4.1); on-demand streaming "based on distance to camera" is mentioned, not described (§6) | Not addressed: the paper assumes "static geometry" (§2) and builds offline (Table 1: seconds to tens of minutes) | Topology ≈ 2 B per voxel at a mean branching factor of four; 1 B colour + 2 B normal (DXT-like blocks) gives **≈ 5 B per voxel** in theory, **5.16–8.10 B measured** (§3.5, Table 1) | Confirms a flat, pointer-light layout (contiguous children, masks, relative offsets) for GPU traversal; that is the S3 note's VoxTree-on-GPU shape, not a reason to change the world pyramid, which is key-addressed and edited. Contours are a *conservative* tightening of coarse voxels, the same idea as the exact-skin pieces for drawing, and an option for the far-field halo (§7b) |
| **GigaVoxels** ([Crassin et al. 2009](https://hal.archives-ouvertes.fr/inria-00345899) [S]; described in Laine & Karras 2010 §2 [R]) | Octree of node pool + brick pool, streamed *by the ray traversal itself*; leaves are bricks "typically 16³ or 32³" sampled along the ray, kd-restart traversal (Laine & Karras §2) | By what rays touch: the renderer "detects when data is missing in GPU memory and signals this to the CPU, which then streams the needed data in" (Laine & Karras §2) | On-demand, view-driven; not an editing structure | Several billion voxels in a limited GPU budget, 20–90 fps (2009 hardware) [S] | The model for "resident set = cut + ancestors, filled by demand". For the read, demand is the *tower's* opening test, not a ray |
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
[Salmon & Warren 1994](https://doi.org/10.1006/jcph.1994.1050) [R], *J. Comput. Phys.* 111, 136–155,
derived worst-case bounds from absolute moments `B_(n) = Σ m |x − r₀|ⁿ` (Eq. 9, 61, 62; App. B) and, on top of
them, four criteria that bound the error per evaluation: the *Absolute* MAC (each partial's bound under a fixed
`Δ_partial`, Eq. 10), the *Relative* MAC (Eq. 11), the *Sum* MAC (the bounds add to at most `Δ_tot`, Eq. 15) and the
*Sum Squares* MAC (they add in quadrature, Eq. 17, which assumes uncorrelated errors). The Cavendish bound has the
same character: S4 uses `Σ m s^l` per order, summed from the leaves. They show the BH criterion has unbounded
error for `θ > 1/√3` (their Appendix A) and that the minimum-distance MAC does not; the Sum MAC is walked with a
priority queue, worst partial first (§4.3). See the first block above for what that changes, and for how loose
this family of bounds is on a cube.

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

### 2.1 The frame: Barnes–Hut per target point, opened by phase error

The owner's suggestion is the frame. The read is a **Barnes–Hut walk** ([Barnes & Hut 1986](https://doi.org/10.1038/324446a0) [R], *Nature* 324,
446–449: monopole pseudo-particle at the cell's centre of mass, open if `l/D ≥ θ`, tree rebuilt every step):
for each target point, descend the tree from the root; a node whose moments are good enough
stands for all its mass, otherwise its eight children are tried. What Cavendish changes is the
*test*. Barnes–Hut's fixed opening angle θ has no bounded error (Dehnen's footnote 3, [R]); the
test here is Cavendish's rigorous bound turned into a threshold:

```
accept node n for target x   when   B(n, x)  <=  eps_level
B(n, x) = G * k_eff T^2 * L_base * sum_{l > p} (l+1)(l+2) S_l(n) / R^(l+3)      (a phase, rad)   [D; see 2.4]
```

with `S_l = Σ m s^l` from the node's `Summary` (§2 table), `R` the least distance from the node's
centre to the target (S4 uses the least distance to the atom path), and `eps_level` the level's
share of the tolerance (§5 measures the radii this implies). The ledger is `Σ B` over the cut. **After reading
S&W §4.3 this is demoted to the fast path:** the primary walk is a priority queue on one global ledger (open the
largest `B` first until `Σ B ≤ τ`), and `eps_level` falls out of it as a table; a node within `b_max` of the
target is opened whatever `B` says.
Section 6 C and F are this test, and A is the fixed-θ test it replaces.

### 2.2 Shared cut and local expansions (the dual-tree step)

A tower's arm points and time samples lie within a few metres of each other, so most of the world
is "far" for **all** of them. Treat the points as one *target cluster* (centre `c`, radius `ρ`):
accept a node for the whole cluster when `a / (|c − centre| − ρ) ≤ θ` (or the bound with `R = |c − centre| − ρ`).
This is Dehnen's cell–cell criterion, `(ρ_A + ρ_B) / r < θ_crit` ([Dehnen 2014](https://arxiv.org/abs/1405.2255) [R]).
Then:

* **Option 1, one shared cut, every point evaluates every node.** Cost `N_pts × |cut|`.
* **Option 2, a local expansion.** Gather the far nodes' field into one Taylor series about `c`
  (an M2L translation per node), evaluate it at each point: cost `|far| × K_M2L + N_pts × terms`.
  The near leaves (the cut's rejected cells) are summed directly per point.

**[M]** (benchmark E, town, quadrupole, 40 points on the 5 m arm axis, ρ = 2.6 m; the expansion
is made by *fitting* a degree-q polynomial to the far field's `g_z`, which stands in for M2L and has
the same truncation behaviour):

| θ_c | far nodes | shared-cut error | **extra** error of the local expansion (q = 4): max over points / spread between points |
|---|---|---|---|
| 0.50 | 232 | 13.4 rad | 1.9×10⁻² / 2.2×10⁻² rad |
| 0.35 | 423 | 4.9 rad | 3.8×10⁻² / 6.4×10⁻² rad |
| 0.25 | 942 | 0.50 rad | 5.2×10⁻² / 6.0×10⁻² rad |

The expansion's own error is far below the cut's at q = 4. For µrad it is not. On the θ = 0.25 cut
(nearest far node 11.5 m, ρ = 2.6 m), **[M]** ([`poly_degree.py`](s5-world-octree/poly_degree.py), [`poly_results.txt`](s5-world-octree/poly_results.txt)):

| degree q | terms | max error | spread between points |
|---|---|---|---|
| 2 | 10 | 1.2 rad | 1.3 rad |
| 4 | 35 | 0.11 | 0.14 |
| 6 | 84 | 5.2×10⁻³ | 6.1×10⁻³ |
| 8 | 165 | 5.0×10⁻⁵ | 1.0×10⁻⁴ |
| 10 | 286 | 1.6×10⁻⁶ | 2.1×10⁻⁶ |

Each two orders buy ~20–100× (`(ρ/R_min)² ≈ 0.05`), so **µrad needs q ≈ 10–12 (about 300–450 terms)**
when the nearest far node is 4–5 ρ away; pushing the near boundary out (a larger θ-floor at fine
levels) lets q fall. Evaluating 300 terms at a point is ~600 flops, against ~60 flops for a node-point
interaction, so the expansion wins as soon as the cut has more than ~10 nodes. Whether to form it by
M2L recurrence (Dehnen's route) or by a fitted polynomial is the builder's call; the fit is simple
and testable against direct evaluation, the recurrence is cheaper to form. For the GPU the natural
split is: form the expansion on the CPU in `f64` (tens of thousands of nodes, once), evaluate the
polynomial on the GPU at every point and time sample, a few hundred flops each [D].

A shared cut is also not worse in error: independent cuts per point (benchmark F2, θ = 0.35) read 971
nodes for two points with a phase error of 8.3 rad; one shared cut reads 423 nodes with 4.9 rad
(one sample, a hint and not a rate). Both points then use the same nodes, which the differential
bound of §2.4 needs.

### 2.3 The static far field is computed once and cached

For a static world the far field at each tower is fixed. Cache, per tower: the cut (node keys with the
`Summary` hash each was read from) and the local-expansion coefficients. An edit changes the
`Summary` of its ancestors only (§3). The far field is **linear in the node moments**, so the update is
`local += M2L(new summary) − M2L(old summary)` for each cut node on the edit's path, and the cut test is
repeated on those nodes only (their `S_l` and centre moved).

**[M]** (benchmark H): a 6 × 6 × 10 block added 40 m from the tower moves the leaf-exact phase by
−14.4191 rad; the cached cut, re-read from the new pyramid, moves by −14.4193 rad (a 2×10⁻⁴ rad
difference, the cut's own error on the changed nodes). The cut *topology* changed (a building's centre
of mass moved, so a node opened or closed): the update must repeat the acceptance test along the edit's
path, not just add a delta. One leaf's path is 8 records here (levels 0–7), about 13 for a 10¹¹-cell
world. Per edit and tower the cost is those ~10 nodes, not the cut. A tower **inside** an edit's
footprint rebuilds its near field (the bubble), which is the leaf-exact part anyway.

Time-varying coarse cells (S14: tides, weather) are edits to `Summary` records of existing nodes; the
same rule applies, so "drive the planet's coarse cells from a model" costs ~10 node updates per tower
per step, not a read.

### 2.4 The gradiometer reads a difference: how much coarser may the far field be?

The phase is the difference of `g_z` at two points `L` apart (benchmark: `L = 5 m`, factor 4.8×10⁹ rad per m s⁻²).
A truncated multipole of order `l` has a remainder whose `g` is at most `(l+1) m s^l / R^(l+2)`, the bound S4
already uses, and whose **difference of `g` across the baseline** is at most `L` times the next derivative,
`(l+1)(l+2) m s^l / R^(l+3)` [D: the `n`-th derivative of an `l`-pole carries `(l+1)…(l+n)`; I believe it but
found no proof in a source]. The differential bound is smaller than the sum of the two `g` bounds by
`(l+2) L / (2R)`. Equating the two bounds at their own radii (`R_g` for the `g` bound, `R_d` for the
difference) gives `R_d^(l+3) = R_g^(l+2) · (l+2) L / 2`. Worked, `l = 3`, `L = 5 m`:

| `R_g` (m) | `R_d` (m) | closer by |
|---|---|---|
| 25 | 26 | none: the baseline is not small against R |
| 100 | 71 | 29 % |
| 1,000 | 481 | 52 % |
| 10,000 | 5,100 | 49 % |

So **beyond a hundred metres a node of a given size may be accepted at about half to three-quarters of the
distance: about one level coarser from ~1 km outwards.** The gain is modest because the exponent is
`1/(l+3)` and the baseline is short: the `1/d³` fall of the gradient is already in the monopole, so this
does not remove rings, it moves each ring's inner edge in.

**Measured** (benchmark F, shared cut, town):
* At the *same bound* ε the differential criterion reads ~30 % fewer nodes than the `g` criterion (4,416 → 3,072
  at ε = 1 rad; 36,802 → 24,542 at 10⁻² rad; 153,887 → 106,421 at 10⁻⁴ rad), **but its measured error is larger
  at the same ε** (at ε = 10⁻⁴ rad: 2.5×10⁻⁵ against 4.9×10⁻⁷ rad), and at the *same measured error* the two read
  the same number of nodes (4.3×10⁻⁴ rad: 56,428 against 55,776 nodes). So the differential bound is a valid,
  smaller *certificate*, not a measured gain at 100 m scale.
* The cancellation needs the error to be common to both points, which it is not for nodes within ~10 baselines:
  F2 shows the `g` errors at the two points as 5.3 and 0.43 (times k_eff T²), and G (matter beyond 30 m only, θ = 0.5)
  shows +2.6 and −9.7 with the difference at −12.3, *larger* than either, because a 5 m baseline resolves a
  node's own multipole pattern at R ≈ 55 m.
* **Where it should pay:** `R ≳ 10 L` and more, i.e. 50–100 m and out for a 5 m baseline, where
  `(l+2)L/(2R)` is 0.125 or below: the far field of S14 (kilometres, the planet). **I could not measure that
  here** (the town is 128 m across); the experiment of §5 should include a far-field case (a distant box,
  with a prism closed form) to check the `R_d` column.

The far field's *physical* contribution falls the same way: a uniform shell beyond `R` has no gradient inside
it (shell theorem), so only **departures** from symmetry matter at the far rings, which is why the plane and
sphere worlds of §4 carry departures only.

### 2.5 Cost per tower per shot, against today's leaf-exact read

`N_pts` = 40 points a tower a shot (the exact-skin note counts ~200 points across five towers,
[§4](exact-skin-and-partial-fill.md) [D: my division]); one interaction is one node or cell against one point,
~10–25 ns (the exact-skin note measures 8.5 ns for a cube's series on one thread; a quadrupole node costs more) [D].
Town: 162,259 cells, 128 m (benchmark E, F); errors are the town's, with a quadrupole.

| Option | One-off per tower | Per shot, static world | Per shot after one edit | Town interactions a shot | Error (rad) |
|---|---|---|---|---|---|
| **0. Leaf-exact (today's S4 read, no merging)** | none | `N_pts × N_leaf` | same | **6.5×10⁶** | 0 (its own) |
| 1a. Barnes–Hut, a cut per point, θ = 0.35 | none | sum of the per-point cuts | same | 1.9×10⁴ (≈ 335× fewer) | 8.3 |
| 1b. Barnes–Hut, one shared cut, θ = 0.35 | the cut | `N_pts × cut size` | same, plus path tests | 1.7×10⁴ (≈ 380× fewer) | 4.9 |
| 2. Shared cut + local expansion (q = 4) | cut + 423 M2L | 40 evaluations of 35 terms + near leaves | same | **≈ 3×10³** (≈ 2,200× fewer) | 4.9 (+ 0.04) |
| 3. Option 2, **cached** (static world) | cut + M2L | evaluations + near leaves only | ~10 node M2L updates + the near field | **≈ 40–100** (≈ 10⁵× fewer) | as 2 |
| 2′. as 2 at µrad: `g` criterion ε = 10⁻⁴ rad, q = 10 | a 1.5×10⁵-node cut, M2L at ~10⁴ flops each | 40 × 286 terms ≈ 2×10⁴ flops | ~10 nodes | the cut itself is **about the number of cells** here | 5×10⁻⁷ (+ 1.6×10⁻⁶ at q = 10) |

At the town's scale µrad **costs as many nodes as there are cells** (row 2′: 153,887 nodes for 162,259 cells):
the crude bound is too loose (§6 C), the order is a quadrupole, and the world is small. The tree earns its
keep as the world grows, not at 10⁵ cells. Scaled (derived, not measured): leaf-exact at 10⁹ cells is
`40 × 10⁹ × 8.5 ns ≈ 340 s` a tower a shot, at 10¹¹ cells ≈ 3×10⁴ s; a cut that grows by the level count
(7 → ~13) and at S4's bound looseness (175–457×) might be 10⁵–10⁶ nodes, so 0.1–1 s a shot for option 1b, and
~10⁻⁴ s a shot for option 3 after a one-off of 0.3–3 s a tower [D]. The decision that matters is **a cached
expansion and a tighter bound**, not the choice between 1a and 1b.

**Recommended:** option 3 for the static world (a cached expansion about each tower, updated by node-path
deltas), with option 1b as the fallback when a tower moves its centre (a mobile tower, or a tower inside an
edited bubble), and the exact leaf kernel only in the bubble. Everything that moves (bodies, populations, S6
onwards) stays on its own trees through its own near kernels, so the cached field is the *static world only*; that
matches S4's "the world is static, so each tower's reading is made once" and adds the updates S4 lacks.


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
  camera-relative ([S4 §3.4](../slices/S4.md): origin formed in `f64` on the CPU). The clipmap paper gives
  the same remedy for many levels: "transform the viewpoint and view matrix into the local coordinate system
  of each clipmap level (using double precision)" ([Losasso & Hoppe 2004](https://hhoppe.com/geomclipmap.pdf),
  §9 "Precision" [R]).
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
  fail this at 10⁴–10⁵; S4's tightened one passes (175–457×). (After S&W: on a cube at p = 3 the best per-node
  absolute-moment bound is 6–65× the true remainder and the triangle sum over a cut adds 3–27×, so ~100× is about
  the floor for a rigorous ledger of this family, 10× would need the signed fourth moments. Record the ratio per
  scene; do not tighten the gate on this evidence alone.)
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
is why §5 tests soundness *and* non-vacuity. **(Revised after S&W: the second half stands; the first does not
survive: a rigorous bound this much tighter is not available. Per node the floor is ~6× at p = 3 (3× with the
signed fourth moment), and bound ∝ d^−7 turns a 3.5× gain into 1.2× in distance. The order of the expansion, the
cached local expansion and a world big enough for the cut to be small are what matter. See the top block.)**

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

## 7a. LOD view modes (as data)

The owner asks to see the world at the level of detail a reader uses. Three modes, one record, no new renderer:

```ron
lod_view: (
  source:  Camera                  // today's: S4's rings about the camera (screen-space error)
         | Tower(2)                // the world as tower 2's read sees it: its bubble and rings
         | Towers([1, 2, 5]),      // together: each region at the finest level any chosen tower reads it
  overlay: None | Level | Tower | Error | NotRead,   // by level, by which tower sets it, by share of the bound
  legend:  Auto,                   // made from what is drawn (S3's rule)
)
```

**How the renderer draws a read cut.** A cut, for drawing or for reading, is the same thing: a list of
`(level, key)` forming an antichain. The camera cut comes from the screen-space rule; a tower's read cut
comes from the opening test (§2.1) and is already held as the tower's cached node list (§2.3), so the
draw list **is that list**, with no new walk. For each node: level 0 gets the near mesh (as ring 0 today,
with the same caps), level ≥ 1 gets its `Brick` ray-marched. S4's rule "a level draws only bricks outside
the next finer level's box" becomes "draw the list"; a flat list needs no nesting, and S4's
`MAX_BRICK_OVERDRAW` and `MAX_MARCH_STEPS` still apply. Shared nodes make it cheap: the bricks are the same
objects the camera cut draws, so a view of tower 2 changes only **which** nodes are in the draw list and asks
the streamer to keep their bricks resident (the read needs summaries, not bricks, so the mode pins the
bricks of its cut, with the caps said).

* *A cut node with no brick resident:* draw its nearest resident ancestor and mark it in the overlay, with a count in the badge, as S4 does for caps.
* **`Towers([…])`, the net LOD.** The finest level any chosen tower reads each region at is the common
  refinement of their cuts: mark every strict ancestor of any cut node as *open*, traverse from the root,
  recurse into open nodes and emit the rest. O(Σ cut × depth) with a hash set, a few ×10⁴ nodes at most.
  Overlay `Tower` colours each emitted node by the tower that forced its level (ties to the lowest id).
* **Overlays.** `Level` uses S3's level palette (S3 has `overlay leaves` and `overlay cut <level>` for bodies;
  this is the world version, one colour per level, a legend of exactly the levels drawn). `Error` colours by each
  node's share of `Σ B` (§2.1; free, the bound is stored), so it shows where the ledger is spent. `NotRead`
  hatches matter outside the read radius or past the cap, which is the read's honest "not read" (S4 §2).
* **It is the read-LOD debug view.** (I did not find the plan's own wording for that view; S3's `overlay cut <level>` is the
  nearest thing in the repository.) One difference from "show the cut": the camera may be far from the tower,
  so screen-space error is not respected. Far rings are coarse near a distant camera; fine cells near the tower
  may be many pixels wide. That is the picture. The caps and the badge keep it bounded.
* The record lives with the scene's view settings (principle 11); the same record is a CLI flag
  (`--lod-view tower:2`) and an MCP query. A test: the draw list of `Tower(n)` equals the engine's cut for
  tower `n` (same node set, by key); `Towers([a,b])` is a common refinement of both and minimal (control: a naive
  union that is not a partition double-covers, S4's R12 test).

## 7b. Hiding LOD seams

Setting: S4 draws ring 0 as greedy meshes and levels ≥ 1 as ray-marched bricks, each a **conservative superset**
of its children (S4 §2: "a coarse brick holds every finer brick's cells, and may add some"), with the rings
exactly nested by snapping (R12). So there are **no holes** at a seam. What shows is a *step* (the coarse surface
stands proud of the fine one by up to `2^L − 1` fine cells at the box edge) and a *pop* when the camera crosses
a snap. The step is what Lengyel calls **surface shifting**: a coarse surface that does not coincide with the fine
one makes "structures placed on the terrain ... suddenly appear sunken into the ground when the detail level
changed, which is not acceptable"; his fix places every coarse vertex exactly on a vertex of the finest mesh
(Lengyel 2010, §4.2.1, pp. 28–30, Figs. 4.2–4.3). Binary occupancy has no vertex to move, so our analogue is the
choice of drawn bits (row "Matching coarse cells" below). The survey, with sources:

| Technique | Source | What it does | Fit here |
|---|---|---|---|
| **Transvoxel** | [Lengyel 2010](https://transvoxel.org/Lengyel-VoxelTerrain.pdf) [R] | A cell of the half-resolution block on the seam is split in two (Fig. 4.7): a thin *transition cell* triangulated from the 9 samples of the full-resolution face (its 4 corners duplicated on the half-resolution face), 2⁹ = 512 cases in 73 classes (§4.3, pp. 35–42), and an ordinary cell from the 8 half-resolution samples. The half-resolution block's boundary vertices move inward by eq. 4.2 to make room, width `w(k) = 2^(k−2)`, half the adjacent full-resolution cell (p. 46, Fig. 4.11); transition meshes are drawn only where neighbours differ (p. 46) | **No.** Both sides must be Marching Cubes isosurface meshes of one scalar field; ours are a greedy blocky mesh beside a ray-marched brick, with no isosurface on either side, so there is nothing to triangulate between. Only 2:1 (§4.1, p. 28). Even if levels ≥ 1 were meshed blockily, transition cells solve cracks, which superset bricks do not have. Its §4.2.1 surface-shifting rule is the part that transfers (above) |
| **Skirts** | Lengyel 2010 §2.1.1, Fig. 2.1(e)–(f), pp. 6–7 [R]: skirts and "flanges" hang extra triangles from the higher-detail mesh's edge downward; skirts "exhibit the same shading problems" as a vertical crack fill, flanges penetrate the surface and cause depth artefacts in additive lighting passes. Ulrich's chunked LOD (2002) uses skirts (Lengyel §2.1.2, p. 8 [R]; Ulrich's notes §11, Fig. 4, pp. 8–9 [R]: a vertical
skirt round each chunk's perimeter, "as short as possible to conserve fill rate", preferred to flanges and ribbons) | Extend each tile's edge downward so a crack shows skirt, not sky | **Low value.** We have no cracks (superset bricks); a skirt only adds walls under the near mesh's outer edge |
| **Geomorphing** (CDLOD) | [Strugar 2010, "Continuous Distance-Dependent Level of Detail for Rendering Heightmaps"](https://raw.githubusercontent.com/fstrugar/CDLOD/master/cdlod_paper_latest.pdf) [R]; Lengyel 2010 §6.1, p. 86 [R] | Each vertex morphs toward its coarse neighbour by `morphK` ∈ [0, 1], computed per vertex from the 3-D distance to the observer, over the **last 15–30 % of each LOD range**; ranges double per level (p. 4, Fig. 2), so a node at full morph "exactly matches the one from the lower LOD layer" and swaps without seams or T-junctions (pp. 7–8, Fig. 6). The distance function must be identical on both sides of a node edge or seams return (p. 8); a node can transition between **only two** adjacent layers, so ranges too short for the morph area produce seams, which the demo's debug build detects (p. 9). Lengyel lists geomorphing as future work for Transvoxel: it costs a secondary position (and normal) per vertex, ~12 B (p. 86) | **Meshes only.** A blocky mesh's vertices sit on the lattice, so a snap toward the coarse lattice is possible, but occupancy is binary: a morphing face collapses a thin wall, which is not conservative. The ray-marched far field has no vertices. Skip the morph; **keep its two rules**: a blend weight from continuous distance, and a ring wide enough that one band never overlaps the next |
| **Dithered (screen-door) cross-fade** | Unity's LOD Group cross-fade ([manual](https://docs.unity.com/en-us/engine/6000.0/manual/analysis/graphics-performance-profiling/simplifying-distant-meshes-with-level-of-detail-lod/level-of-detail); the page I could read does not describe cross-fade, so my account of dithering and "animate cross-fading" is from memory of Unity, not read here [U]). No primary paper read for the dither itself | During a fade both levels are drawn and each pixel keeps one by a dither threshold, so opaque rendering and depth still work | **The best fit.** In the brick ray-march, over an overlap band the finer level also draws its outer cells and each pixel takes one of the two by a Bayer or blue-noise threshold on the distance fraction. The cost is the band drawn twice (bounded by `MAX_BRICK_OVERDRAW`). Opaque, no sorting. It is the opaque, per-pixel form of the clipmap's `α` (next row) |
| **Overlapping rings with a blend weight** | The clipmap's transition region, [Losasso & Hoppe 2004, "Geometry Clipmaps: Terrain Rendering Using Nested Regular Grids"](https://hhoppe.com/geomclipmap.pdf) §6.2–6.3, Fig. 4–5 [R] | Near each level's outer boundary the geometry morphs toward the coarser level, `z' = (1 − α) z + α z_c`, with `α = max(α_x, α_y)`, `α_x = clamp((|x − v_x| − ((x_max − x_min)/2 − w − 1)) / w, 0, 1)`, `v` the **continuous** viewer position and `w = n/10` grid units (capped by the room to the finer region); "the transition is not time-based but instead tracks the continuous viewer position" (§6.2). Textures (normal maps) of levels `l` and `l−1` are blended in the pixel shader with the same `α` (§6.3). T-junctions are closed with zero-area triangles. The regions themselves move in whole even-grid steps (constraints 1–4, §5), yet `α` is continuous because it is measured from the viewer, not from the region | **Not as alpha** (opaque voxels, no order-independent transparency); the dither is its opaque form. **Its weight is the one to copy:** measured from the camera, inset from the snapped edge, so a snap happens where the coarse level is already fully shown |
| **Fog / haze** | No primary source read | Hides far steps and pops by distance | Free beyond the last ring, and at planet scale. S21's plan lists fog among "the look" (branch `plan-look`, commit `979a144`; I read only its subject line [U]) |
| **Matching coarse cells to the near mesh's faces** | None; my design | Rings nest exactly (S4's snapping puts level boundaries on multiples of `2^L`), so a coarse cell boundary is always a fine lattice line. A **mass-weighted** occupancy for the *drawn* coarse brick (≥ 50 % of the mass) cuts the proud halo, while the **conservative** superset stays for occlusion and the read | **Yes, per payload:** draw bits mass-weighted, occlusion bits superset. Two bit-planes per brick, ~4 KiB more. This is the blocky analogue of Lengyel's surface-shifting fix (§4.2.1): the drawn coarse surface should sit on the fine one, not outside it |
| **Conservative contours** | Laine & Karras 2010 §3.2–3.4, Fig. 4–5 [R] | A pair of parallel planes per voxel (32 bits), intersected with the ancestors' planes, shrinks a coarse voxel toward the surface while still enclosing it | **Optional, later.** Tightens the halo without giving up conservativeness, but the far field then stops looking blocky; mass-weighted bits are simpler and keep the look |
| **Filter radius clamped across levels** | Laine & Karras 2010 §4.3, Fig. 10–11 [R] | Their post-process blur takes its radius from each pixel's voxel size; neighbours on different levels made "seams ... visible at hierarchy level changes", fixed by clamping each sample's radius to `min(r, r')` with weights tapering over the last pixel | **Only if** the far field gets a size-dependent screen filter (e.g. S21's look): then clamp across the level boundary the same way |
| **Temporal blend** | Unity's "animate cross-fading" [U]; TAA in general [U]. None of the primary sources uses time: L&H, CDLOD and Lengyel all drive the blend from camera position (rows above) | A level switch fades over a few frames, driven by time rather than distance | **Only for late arrivals:** a node whose brick becomes resident after the camera has passed its band has no position to blend over, so fade it in over ≈ 0.2–0.3 s. Ring snaps do **not** need it once the band is inset from the snapped edge (row "Overlapping rings") |

**Recommended for the mesh-near, ray-march-far split** [D, a design judgement built on the sources above; nothing
here is tested]:
1. Keep exact nesting and superset bricks (no cracks by construction; R12 is the test).
2. **Dithered cross-fade in the brick ray-march**, the weight a function of the **continuous camera position**,
   as in the clipmap (L&H §6.2) and CDLOD (pp. 4, 8): over a band at the outer edge of each finer ring, inset from
   that ring's snapped boundary by at least one snap step, each pixel draws the finer or coarser level by a Bayer
   or blue-noise threshold on `α`, the camera-distance fraction across the band (L&H's `α_x, α_y` with the
   distance in the ring's own cells). Because `α` reaches 1 before the snapped edge, a snap changes nothing on
   screen. **Band width is a measured parameter**: the sources use `n/10` grid units (L&H) to 15–30 % of a range
   (CDLOD); start at ~10 % of the ring's half-width and not below 2 coarse cells, and keep each ring at least
   band + snap step wide (CDLOD's two-layer limit, p. 9).
3. **Mass-weighted drawn bits, superset occlusion bits**, to lower the halo (Lengyel's surface shifting).
4. A time-driven fade only for bricks that arrive late from the streamer.
5. Haze beyond the last ring (S21).
6. Not Transvoxel, skirts or geomorphing: they fix cracks and vertex pops the brick-and-mesh split does not have.
7. Optional, measured first: ESVO's **beam optimisation** for the brick march (Laine & Karras §4.2): a coarse pass
   per 4 × 4 or 8 × 8 pixel block finds a conservative start distance (traversal stops at a voxel too small to
   certainly cover a ray of the coarse grid; contours off), and each pixel's ray starts there. Their gain over
   contours alone was 2–25 % at 1024 × 768 (Hairball 27.9 → 28.4, Sibenik 94.1 → 103.6, City 89.1 → 106.0,
   Sibenik-D 38.7 → 48.3 Mrays/s; Table 2): modest, so only worth it if the march is step-bound.

A test with a control arm: render a camera path across 50 ring snaps; the seam metric is the mean absolute
frame difference in a band around each ring boundary, normalised by the frame difference elsewhere. Because the
blend depends on position only, the path is a fixed list of camera poses and the frames are reproducible. Pass:
below a threshold stated before measuring; controls: the blend switched off must exceed it, and the band placed
*on* the snapped edge (not inset) must exceed it at the snap frames.


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
* **GigaVoxels, NanoVDB internals**: only search summaries and Laine & Karras's §2 account (GigaVoxels) were
  read; the papers are not opened here. Their rows in §1 are what those sources state, not more. **ESVO** is now
  read in full (owner's copy of the I3D paper).
* **Lengyel's dissertation: resolved.** Read from the owner's copy of
  `https://transvoxel.org/Lengyel-VoxelTerrain.pdf` (Chapters 2.1 and 4, §6.1). Blocks are **16³ cells at every
  LOD**, doubling in physical size per level (§4.2, p. 28): the equal-cell-count pyramid this note recommends. The
  earlier forum claim of "16³ at finest, 32³, 64³ coarser" (growing cell count) is **wrong** about Lengyel.
* **The dithered cross-fade itself** has no primary source read here (Unity's page did not describe it); the
  position-driven weight it uses is sourced (L&H §6.2, CDLOD), the dither is my design [D].
* **Salmon's thesis and Hernquist 1987: now read** (28 Nov 2026, third-batch block). The thesis's acceleration bound
  (Eq. 5.61) is a conjecture checked for `n < 4`; the note relies on S&W's Eq. 62 and `mac_bound.py`'s checks instead.
* **Salmon & Warren 1994 and Barnes & Hut 1986: now read** (owner's scans, 25 Nov 2026; see the top block). Not
  followed: S&W's Appendix B derivation of Eq. 52–57 (the kernel and Legendre-derivative bounds) beyond checking that
  Eq. 60/62 is what `mac_bound.py` evaluates and that it is never below the true remainder in any comparison made (36 acceleration and 12 gradient cases);
  their §4.5 test models (the 8.8-million-body simulation) are described, not reproduced. Dehnen's PDF was read
  as text; its figures were not seen.
* **Cesium, "camera-relative" patents, Voxel Farm, Outerra**: not opened; the floating-origin claims
  here rest on S4's own `f64`-on-the-CPU rule and the arithmetic of `f32`.
* **Sources I could not read, with their exact URLs** (to fetch by hand):
  * `https://maverick.inria.fr/Publications/2009/CNLE09/` (GigaVoxels; connection refused). The HAL page
    `https://hal.archives-ouvertes.fr/inria-00345899` gave a search summary only.
  * `https://developer.nvidia.com/blog/nanovdb-sparse-volume-gpu-rendering/` (404). The paper
    `https://research.nvidia.com/labs/prl/nanovdb/nanovdb2021.pdf` was not opened.
  * `https://docs.unity3d.com/Manual/lod/lod-cross-fade.html` (404). The replacement page returned only the LOD
    basics. To try: `https://docs.unity.com/en-us/engine/6000.0/manual/analysis/graphics-performance-profiling/simplifying-distant-meshes-with-level-of-detail-lod/class-lodgroup` (not fetched).
  * **Read in the second batch (25 Nov 2026):** Barnes & Hut 1986, `https://doi.org/10.1038/324446a0`; Salmon & Warren
    1994, `https://doi.org/10.1006/jcph.1994.1050`; Ulrich 2002, "Rendering massive terrains using chunked level of
    detail control" (draft of 14 April 2002, *Super-size It! Scaling up to Massive Virtual Worlds*, SIGGRAPH '02 course
    notes no. 35; the owner's copy, `sig-notes.pdf`; no URL found). Also supplied: Koca 2012, *Representation, editing
    and real-time visualization of complex 3D terrains* (Bilkent MSc thesis, 174 pp.), **not read** beyond its
    contents and §2; its §5.2 (editing the terrain) and §4.4 (level of detail, pp. 105–123) are the parts that
    might bear on S5's edits and seams.
  * **Read in the third batch (28 Nov 2026):** Salmon's Caltech PhD thesis (S&W's Ref. 11; title page dated 1991,
    defended 6 Dec 1990; §4.6, Ch. 5 and Ch. 6 read, Chs. 1–3, 7–8 and the appendices not); Hernquist 1987, *Ap. J.
    Suppl.* 64, 715–734 (S&W's Ref. 13, in full). See the third-batch block at the top. Rathinam & Dempster 2019 was
    read by a companion agent, not here. **Still unread:** Dehnen's figures; Greengard & Rokhlin 1987 is not cited by
    this note (Salmon, p. 113, says his Eq. 5.57 is "essentially the same result" as Greengard's, his Ref. 13, which I
    did not identify) and was not opened.
  * Read in full with figures (owner's copies, 25 Nov 2026): Strugar's CDLOD paper
    (`https://raw.githubusercontent.com/fstrugar/CDLOD/master/cdlod_paper_latest.pdf`), Losasso & Hoppe
    (`https://hhoppe.com/geomclipmap.pdf`), Laine & Karras
    (`https://research.nvidia.com/publication/2010-02_efficient-sparse-voxel-octrees`, file `laine2010i3d_paper.pdf`),
    and the LOD chapters of Lengyel (`https://transvoxel.org/Lengyel-VoxelTerrain.pdf`). Dehnen's PDF was read as
    text (the figures were not seen).
* **S4's code**: I read the slice note, not `cav-sim/world.rs`. How its `Σ m s^l` is held (per chunk, per
  node) decides whether §2's carried `S_l` is a small change or a new field.
* **Costs at 10⁹–10¹¹ cells** are derived, not measured; the town benchmark is 2×10⁶ cells in numpy.
* **The tail bound of §4.1** (`2πGρH/R`) is my derivation; its use as a horizon rule needs the data's
  departure statistics.

**How to resolve each, in the slice:** the first slice task is the §5 harness on `world-box` and
`town` (it settles the bound, the radii and the carried `S_l` in one run); the second is a 10¹⁰-cell
walk timing with the S4 stress scene; the planet-offset parity test settles the precision.

## 9. Further questions (not chased)

0. The far-field cases for §2.4's `R_d` column (a distant prism, 100 m to 10 km), and a world big enough, with an octupole and S4's tighter bound, that the cut is much smaller than the cell count at µrad.

1. Mass-weighted dominant material versus a per-brick palette for colour when levels mix many materials
   (a colour/appearance question for S19's materials).
2. Time-varying coarse cells (tides, groundwater, weather) written into the tree by S14: they change
   `Summary` records of existing nodes without any leaf; does the dirty-path rule hold (it should)?
3. Whether `Summary` should carry the **signed fourth moments** (15 raw components, ~120 B at `f64`) so that the
   leading remainder term of a cube-like node is exact and only the tail is bounded: measured to tighten the
   per-node bound from 6–9× to 1.4–3.2× at p = 3 on a cube, for about 1.2× in accepted distance (top block). Worth it only if a
   tight ledger is itself wanted; the gate (§5) does not require it.
4. Whether error-driven opening should be *per tower* or *per arm point*: the S4 read uses the least
   distance to the arm path; a differential evaluation (S5-02) may justify a per-pair bound.
5. A hole-aware tile bound for the plane when the departures are not zero-mean (a quarry, a lake).
6. Using the shared tree for S9's adaptive fluid grid, to avoid a second octree there.

## Files

* [`s5-world-octree/lod_walk.py`](s5-world-octree/lod_walk.py): the benchmark (`python lod_walk.py`, numpy only, ~3 s).
* [`s5-world-octree/results.txt`](s5-world-octree/results.txt): its output, as quoted in §2 and §6.
* [`s5-world-octree/poly_degree.py`](s5-world-octree/poly_degree.py) and [`poly_results.txt`](s5-world-octree/poly_results.txt): the local-expansion degree sweep (§2.2).
* [`s5-world-octree/mac_bound.py`](s5-world-octree/mac_bound.py) and [`mac_bound.out`](s5-world-octree/mac_bound.out) (numpy, ~4 s): Salmon & Warren's bound against the true remainder on a uniform cube, with and without the exact leading term (top block).
* [`s5-world-octree/cut_cancellation.py`](s5-world-octree/cut_cancellation.py) and [`cut_cancellation.out`](s5-world-octree/cut_cancellation.out) (numpy, imports `lod_walk.py`, ~6 s): per-node errors of the town cuts, the triangle sum against the true error and the quadrature sum.
