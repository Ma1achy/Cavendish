# Rigid dynamics and voxel contact for S7 ("Things that fall and stack")

Research note, 25 Nov 2026. Question: which integrator, contact solver, contact generator, destruction
and determinism design lets S7 meet its exits (`plan/REBUILD.md` §9: free fall ≤ 1e-10 m, a slept pose
bit-constant, box on box rests), and how is each part validated against a known answer?
Benchmarks quoted here were written and run for this note (Python, IEEE doubles, scripts and raw output in
`research/rigid-bodies/`, one file per benchmark, B1 to B7). Python's `float` is the same binary64 as Rust's
`f64`, so the arithmetic is the arithmetic; the *code* is a model of the algorithms, not the S7 code.

---

## 0. One-page summary

**Recommendation, in one paragraph.** A CPU, `f64`, fixed-step engine in `cav-sim`, built from five
pieces. (1) **Free flight is closed-form**: a body not in contact carries `(t₀, x₀, v₀, q₀, L)` and is
*evaluated*, not stepped; torque-free rotation uses the Strang splitting already in v2 (conserves |L| to
rounding), or its 4th-order triple-jump when energy must be tighter. (2) **Contact is the "soft step"
of Catto/Box2D v3**: small substeps (K = 20 per 1/60 s frame as a start), one Gauss–Seidel sweep each, soft
contacts capped by the substep rate, warm starting, and a *relax* sweep; restitution and Coulomb
friction at velocity level; friction clamps give an *exact* stop. XPBD (Müller 2020) is the written
alternative; the stack benchmark below shows the two within a factor of about two of each other and both
far ahead of iterations-with-Baumgarte at equal budget. (3) **Contact generation walks the two octrees**
together (node-cube against node-cube, in the frame of one body), turns leaf-cube overlaps into
candidates keyed by the two Morton keys, and *reduces* them to at most four points per body pair; a
plain analytic fast path for sphere/box/plane is kept as the test oracle. (4) **Sleeping is per
island** (Box2D's design): a whole island freezes, poses are not touched, velocities set to exactly zero,
and a frozen body is skipped, so its pose is bit-constant by construction; thresholds are set from the
*measured* solver noise, not from game defaults. (5) **Determinism by construction**: IEEE-only
arithmetic (+ − × ÷ √), no `mul_add`, no platform `sin`/`cos`/`exp`/`atan2` (fixed-degree series, ≤ 1 ulp
for the small angles of a substep, B6), no `relaxed-simd`, islands solved independently and merged in
sorted order, **mass moments kept as integer sums on the lattice** so that splitting, carving and merging a
body are exact and order-free. Destruction = connected-component labelling of leaves, with children
inheriting rigid-body velocities (momentum, angular momentum and kinetic energy then add up exactly in
exact arithmetic). Motion feeds the read as a pose (`x`, `q`) of an unchanged tree: no re-voxelising.

**Numbers that decided it (B1–B7, all in §8).**
- A stepped free fall misses the 1e-10 m exit: the PBD/PGS update (`v += g h; x += h v`, symplectic Euler) is
  0.20 m off after 10 s at 1/240 s; velocity Verlet is exact for constant `g` but its rounding drifts to
  2e-8 m at 100 s. A closed-form flight segment has no drift.
- Game-style rotation (explicit Euler on Euler's equations) drifts |L| by 2.5e-3 relative in 20 s at 1/240;
  the splitting keeps |L| to 1.6e-14 and E to 1.2e-7 (hard, near-intermediate-axis case).
- At equal solver budget, a 10-box stack penetrates by 1.1 mm (PGS + Baumgarte, K = 20), 0.10 mm (XPBD) or
  0.05 mm (soft step with a stiff contact); a Box2D-default 30 Hz contact never gets below 5.0 mm. Penetration
  falls like 1/K² for the substepped methods. A 1000 kg box on five 1 kg boxes defeats the iterative method
  (319 mm, still moving at 16 m/s) and is only roughly tamed by substeps (10 to 14 mm at K = 20).
- Fixed-step "detect, then project" contact misses a bounce's impact time by 36 ms at 1/240; solving the
  time of impact inside the step gets 1e-15 s.
- Simultaneous restitution is *not* Newton's cradle: three touching equal balls with e = 1 give
  v = (−⅓, ⅔, ⅔), not (0, 0, 1), at any iteration count (B7). The cradle test must have gaps.

**Not settled by reading (resolve in the slice, §9):** the penetration/sag tolerance the instrument can
accept; contact quality of voxel normals on curved bodies (rolling sphere); the sleeping thresholds; whether
Rust or LLVM ever contracts `a*b+c` on the web target (there is no primary source I could read; a
native-versus-wasm trajectory hash settles it).

---

## 1. Integrators and solvers

### 1.1 Free flight and rotation

- **The update inside the usual contact pipeline is not exact for free fall.** Both PBD
  ([Müller et al. 2020, Algorithm 1/2](https://matthias-research.github.io/pages/publications/PBDBodies.pdf);
  predict `x + hv + h²g`) and the sequential-impulse recipe
  ([Catto 2006: "Update position last"](https://box2d.org/files/ErinCatto_SequentialImpulses_GDC2006.pdf))
  are symplectic Euler: `v += h g`, then `x += h v`. For constant `g` the position is `½ g t² + ½ g h t`, i.e. an
  error that grows linearly in time. **B1:** 0.0817 m at 1 s and 1/60 s, 0.204 m after 10 s at 1/240 s,
  against an exit of 1e-10 m. Leapfrog (kick–drift–kick) and velocity Verlet
  (`x += h v + ½h²g; v += h g`) are exact for constant acceleration in exact arithmetic; in doubles the
  error is rounding, but it is *secular*: 2e-12 m at 10 s, 2.2e-8 m at 100 s, 1.3e-5 m at 1000 s (1/256 s step).
  So even the exact scheme only meets 1e-10 m for flights up to about 30 s.
- **Therefore: closed-form flight segments.** A body with no contact within its swept bound carries its
  last-contact epoch and is *evaluated* `x(t) = x₀ + v₀ τ + ½ g τ²`, `τ = t − t₀`; the error is one rounding of
  the final expression whatever the duration. The same representation is what a recorded track needs: the
  v2 design records each body and interpolates by cubic Hermite, "exact for ballistic flight"
  (`reference/design/physics.md` §7); a flight segment *is* that quadratic.
- **Rotation.** Torque-free free rotation of a rigid body is a splitting problem: the kinetic energy splits
  into three terms (one per principal axis), each an *exact* rotation. Composing them symmetrically is
  second-order, symplectic, conserves |L| to rounding and keeps E bounded
  ([Dullweber, Leimkuhler, McLachlan 1997](https://doi.org/10.1063/1.474310) (not read; abstract-level via
  [search listing](https://www.research.ed.ac.uk/en/publications/symplectic-splitting-methods-for-rigid-body-molecular-dynamics/));
  [Lee, Leok, McClamroch 2007](https://arxiv.org/abs/math/0508365) give the Lie-group variational view, read at abstract level;
  [Van Zon, Omelyan, Schofield 2008](https://arxiv.org/abs/0710.3390), abstract only, compare splittings and exact free motion).
  **v2 already has it**: `crates/source/src/integrator.rs` at tag `v2-prototype` ("McLachlan/Reich free-rigid-body
  Strang splitting … order 1,2,3,2,1"), which I read. **B2** (inertia 1, 2, 3; spin about the unstable intermediate
  axis; 20 s; reference RK4 at 1/8192 s):

  | integrator | h | max |ΔE/E| | |Δ**L**|/|L| (world) | attitude error at 20 s |
  |---|---|---|---|---|
  | explicit Euler (game style) | 1/240 | 4.9e-3 | 2.5e-3 | 2.7e-2 rad |
  | Strang splitting | 1/60 | 1.9e-6 | 4.2e-15 | 2.6e-5 rad |
  | Strang splitting | 1/240 | 1.2e-7 | 1.6e-14 | 1.6e-6 rad |
  | Strang splitting | 1/960 | 7.6e-9 | 4.7e-14 | 1.0e-7 rad |
  | Yoshida triple jump of the splitting (4th order) | 1/240 | 2.4e-12 | not measured | 2.9e-9 rad (near the reference's own floor) |

  Second-order convergence (×16 for ×4 in h), as theory predicts. Catto's slide advice "ignore gyroscopic
  term for improved stability" ([GDC 2006](https://box2d.org/files/ErinCatto_SequentialImpulses_GDC2006.pdf))
  is a game trade that Cavendish's quadrupole read cannot take: a spinning asymmetric body's attitude *is*
  the signal.
- **Trig inside the splitting must be portable.** The splitting needs `sin`, `cos` of `ω h` (small).
  Rust documents `sin`, `cos`, `exp`, `ln`, `powf` as "non-deterministic … varies by platform, Rust version, and
  can even differ within the same execution" ([std f64](https://doc.rust-lang.org/std/primitive.f64.html)); the
  v2 integrator calls `S::cos`, `S::sin`. For |x| ≤ 0.5 rad an eight-term Taylor/Horner series (only + − ×) is
  within 0.95 ulp (sin) and 0.65 ulp (cos) of a 60-digit reference (**B6**); this machine's libm is within 0.5 ulp.
  Any series is bit-identical on every IEEE target. (An alternative with no trig at all is the Cayley transform
  per sub-rotation; not tested.)

### 1.2 Contact: impulses, positions, substeps

Candidates and what the primary sources say:

| family | what it is | evidence |
|---|---|---|
| Sequential impulses / PGS with Baumgarte | velocity-level Gauss–Seidel, warm started, position error fed back as velocity bias | [Catto 2006](https://box2d.org/files/ErinCatto_SequentialImpulses_GDC2006.pdf): "accept penetration, remember the past, apply impulses early and often, pursue the true impulse, update position last". [Solver2D](https://box2d.org/posts/2024/02/solver2d/): Baumgarte "can lead to energy creation and jitter if not tuned carefully". |
| Soft step (TGS_Soft) | substeps, one sweep each, soft constraints, warm start, relax sweep | Catto's Box2D v3 choice: "smaller sub-steps beat extra iterations because they reduce non-linear effects"; relaxation "is expensive, but it improves the simulation quality dramatically" ([Solver2D](https://box2d.org/posts/2024/02/solver2d/), via fetch summary). |
| PBD / XPBD | positions projected, velocity = Δx/h | [PBD 2007](https://doi.org/10.1016/j.jvcir.2007.01.005); [XPBD 2016](https://doi.org/10.1145/2994258.2994272) (compliance makes stiffness step- and iteration-independent); [Small steps 2019](https://doi.org/10.1145/3309486.3340247): "a single large time step with n iterations is less effective than n smaller time steps, each with a single iteration"; their capsule stack under a heavy mass "collapses" with 1 substep × 100 iterations and stays stable with 100 × 1 (Fig. 9); [Müller 2020](https://doi.org/10.1111/cgf.14105): 20 substeps × 1 iteration for every example in Table 1, and "the best choice is to replace all solver iterations by substeps". |
| Parallel / Jacobi with mass splitting | each body's mass divided by its contact count in the effective-mass term | [Tonge et al. 2012](https://doi.org/10.1145/2185520.2185601), abstract and patent summaries only (not read). Aimed at GPUs and large piles; avoids jitter at low iteration counts. |
| Non-smooth Newton, LCP/NCP | solve the complementarity problem directly | [Macklin et al. 2019](https://arxiv.org/abs/1907.04587): GPU conjugate-residual solver; abstract only. Heavier than needed at tens to thousands of bodies. |
| Gustafsson's new engine | "substepping instead of solver iteration … 'Temporal Gauss Seidel' … a parallel solver that can solve large piles … improvements to contact generation and broad phase" | [Voxagon 2024 summary](https://blog.voxagon.se/2024/12/29/year-summary.html); [80.lv](https://80.lv/articles/see-what-s-new-in-teardown-creator-s-custom-voxel-physics-engine): ~5 ms step on 32 threads (scene size not stated). No algorithm detail is public that I could find. |

**B3: a 1D stack (heights only, so every number can be checked by hand), equal constraint-sweeps per frame
(K = substeps × iterations), frame 1/60 s, g = 9.81, rest after 6 s. "pen" = worst interpenetration; "|v|" =
worst speed in the last second.** (script: `b3_stack.py`; soft contact stiffness = `min(HZ, 0.25/h)` per Box2D.)

| 10 equal boxes | K = 4 | K = 8 | K = 20 |
|---|---|---|---|
| A: PGS + Baumgarte, 1 substep (pen mm, \|v\| m/s) | 7.9, 2e-7 | 3.8, 3e-14 | 1.14, 3e-16 |
| B: soft step, stiff contact (0.25/h) | 1.39, 1e-14 | 0.35, 4e-14 | 0.050, 1e-13 |
| B′: soft step, Box2D default cap 30 Hz | 4.97 | 4.97 | 4.97 |
| C: XPBD, K substeps × 1 | 2.6, 9e-8 | 0.61, 2e-12 | 0.10, 0 |

| 1000 kg on five 1 kg boxes (mass ratio 1000:1) | K = 4 | K = 8 | K = 20 |
|---|---|---|---|
| A | 1285 mm, 49 m/s | 719 mm, 24 m/s | 319 mm, 16 m/s |
| B (stiff) | 308 mm, 0.35 | 61 mm, 0.05 | 9.8 mm, 1.5e-4 |
| C | 338 mm, 2.6 | 85 mm, 1.1 | 13.5 mm, 0.31 |

(Light-on-heavy 1:1000 is easy for all three: pen ≤ 0.55 mm for B and C at K = 4, 0.02 mm at K = 20.)
Reading: (a) substeps beat iterations at equal cost, matching Macklin 2019 and Müller 2020; (b) rest penetration
is `O(N g h²)`, never zero, so "rests" must be a stated tolerance (§6); (c) the **contact stiffness cap is the
dominant error** in B′: Box2D's 30 Hz default (chosen for visual softness) sags 5 mm in 10 boxes regardless of
K, so S7 must not copy it; (d) 1000:1 on a chain is a known-hard case, and only B and C at K = 20 get within
centimetres; this is a model, not S7's solver, and 3D contact adds friction cones and rotations.

**Which of B and C?** XPBD's contact positions come out of a *position* projection with `v = Δx/h`. Müller 2020
says plainly that this velocity "simply reflects the penetration depth" after a collision and that it needs an
extra velocity pass for restitution and dynamic friction; Catto "had trouble getting friction to work well" in
his XPBD ([Solver2D](https://box2d.org/posts/2024/02/solver2d/)). `plan/REBUILD.md` §5.3 forbids "position
projection that violates momentum". Mass-weighted PBD projections conserve linear and angular momentum by
construction ([PBD 2007](https://doi.org/10.1016/j.jvcir.2007.01.005), the section on momentum-conserving projections), so XPBD is not excluded, but it
*adds* energy through the separating velocity. **Pick B** (velocity level, restitution from the pre-solve
velocity, the relax sweep removing the bias energy); keep C as the second implementation to compare on the
same stack scene.

**Restitution, friction, sleeping thresholds**, concretely:
- Restitution at velocity level after the solve, from the *pre-solve* approach speed, with a threshold so a
  resting contact does not bounce: Müller 2020 §3.6 (Eq. 34) uses `|v̄ₙ| ≤ 2|g|h`, "two times the velocity the
  prediction step adds due to gravitational acceleration".
- Coulomb friction with an impulse clamp `|λₜ| ≤ μ λₙ`: an exact stop (the clamp cannot reverse motion).
  **B5:** a box sliding at 2 m/s, μ = 0.4: closed-form stop distance 0.509858 m; mean-velocity position update
  error 7.9e-6 m at 1/240 s (O(h²)), semi-implicit −4.2e-3 m (O(h)); final velocity exactly 0 in both.
  Stop *time* is quantised to the step (2.6 ms at 1/240); split the final step to remove it.
- A step containing an impact needs the **time of impact** (B4 below), not "detect at the step end, project".
  Müller 2020 §3.5 makes pair collection a once-per-frame step with AABBs expanded by a safety multiple of
  the motion (factor 2 in his examples), and Soft Step treats separated contacts speculatively.

### 1.3 Stability of stacks, rest and sleeping

- **Stacks.** The evidence above (Macklin 2019 Fig. 9; Müller 2020; B3) is that stack stiffness comes from
  substep count, warm starting and relaxation. Catto also notes Block solving (two points together) improves
  stacking at extra cost ([Solver2D](https://box2d.org/posts/2024/02/solver2d/)).
- **Sleeping.** Box2D v3: sleep is *per island*, "putting only one body to sleep causes shapes to overlap",
  each body has a timer reset when it exceeds a velocity threshold, the island sleeps when the *smallest* timer
  exceeds `timeToSleep`; an awake body touching a sleeper wakes the whole island at once; islands are
  connected components of the contact/joint graph (static bodies excluded); removing a constraint flags an
  island for a depth-first split, at most one per step
  ([Box2D: simulation islands](https://box2d.org/posts/2023/10/simulation-islands/), via fetch summary).
  The v2 design (single body in contact below 0.02 m/s, 0.05 rad/s for 0.5 s) is the body-level version; S7 should
  use the island version, since stacks are what S7 is for.
- **Bit-constant rest** is then trivial *if* a sleeping body is skipped and its velocity is set to zero: the
  pose bits do not change because nothing writes them. The risk is the opposite one: sleeping too early
  freezes a body that is truly moving at the threshold speed, an error that grows as `v_th t`. §5.3's rule
  ("sleeping thresholds that move bodies" are not taken) is satisfied because nothing moves the body when it
  sleeps; but the threshold must sit **above the solver's rest noise and well below any real motion**. B3's
  noise at rest was ≤ 2e-7 m/s (PGS, K = 4) and ≤ 1e-9 m/s (substepped, K ≥ 8). A threshold near 1e-4 m/s
  (a creep of 0.1 mm per second) would sit three orders above that noise; whether it is also far enough below
  any motion the read must see is the instrument's call. **Fix it by measurement in the slice**, the
  criterion written down before measuring (see §9).

---

## 2. Contact between voxel bodies and with voxel worlds

What is on the table, and what the primary sources support:

- **Point shell against voxel map** ([McNeely, Puterbaugh, Troy 1999](https://doi.org/10.1145/311535.311600); I could
  not obtain the 1999 text; read the 2006 follow-up
  [Voxel-Based 6-DOF Haptic Rendering Improvements](https://digital.lib.washington.edu/researchworks/bitstreams/54910b02-279c-481b-878b-20e30327a227/download),
  which reviews it): one body is a *point shell* (surface samples with normals), the other a *voxmap* with a
  distance field; contact is "point-voxel intersection", force along the point normal by depth, a 512-tree voxel
  hierarchy, "voxel-scale accuracy" traded for 1 kHz. Their follow-up reports an order-of-magnitude accuracy
  gain from distance fields, temporal coherence and culling redundant surface contacts, and states that
  surface-point counts grow as the inverse square of the voxel size ("easily exceed a million points", only
  about 2000 point-voxel tests per haptic frame on the hardware of the time). Lesson for Cavendish: per-surface-
  sample contact **needs reduction** and **hierarchy**.
- **Signed distance fields with local optimisation** ([Macklin et al. 2020](https://doi.org/10.1145/3384538);
  abstract-level only): robust SDF collision by finding the closest points between an SDF isosurface and mesh
  elements with a local solver (projected gradient, Frank–Wolfe, golden section). Relevant for smooth normals
  and sub-cell depth on a voxel body.
- **Teardown.** The public record is thin: destruction made deterministic with fixed-point integer maths, each
  break "a stream of deterministic commands" ([Voxagon, 2026](https://blog.voxagon.se/2026/03/13/teardown-multiplayer.html));
  the new engine's solver is sub-stepped Gauss–Seidel ([2024](https://blog.voxagon.se/2024/12/29/year-summary.html)).
  **I found no primary text describing Teardown's voxel contact generation** (the 2018 "Physics" post at the
  old URL 404s; the 2014 "Cracking destruction" post is about convex shapes, not voxels,
  [link](https://blog.voxagon.se/2014/05/13/cracking-destruction.html)). The plan's description (§5.2) of
  Teardown's method is from the owner's earlier notes, not verified here.

### Recommended contact pipeline (my design, from the above plus S3's tree)

S3's bodies are linear octrees from 30-bit Morton keys, leaves `{key, level, count, material}`, per-node mass
moments in f64 (`docs/slices/S3.md` §2; `crates/cav-core/src/voxel/tree.rs`). That is already a bounding
volume hierarchy of cubes.

1. **Broad phase.** A dynamic AABB tree or a sorted-axis sweep over body AABBs (swept by the frame's motion ×
   the safety factor, as Müller 2020), output *sorted* by `(bodyA, bodyB)`. Hundreds to a few thousand bodies
   do not need more. Not a primary-sourced choice; Box2D uses a dynamic tree, I did not read its source.
2. **Midphase: dual-tree descent.** Put body B's frame in A's: node cubes (side `h·2^level`) against node cubes
   as oriented cubes (15-axis SAT; cheap), descending only the overlapping, internal-on-either-side pairs.
   Against the static world volume (S5) the world is axis-aligned and lattice-aligned, so only the body's
   nodes are rotated.
3. **Leaf pairs → candidates.** Two overlapping leaf cubes give a candidate `(point, normal = SAT axis of least
   penetration, depth)`; the feature id is `(keyA, levelA, keyB, levelB)`, which is *stable between steps*
   (Morton keys are lattice addresses), so impulses warm-start by lookup. A cell counts as solid for contact
   when `count·2 ≥ k³` (half the supersamples inside); mass is unchanged. State the rule in the docs: the
   collision surface is within half a cell of the mass surface.
4. **Reduction.** Per body pair, keep at most four points: deepest, farthest from it, then those maximising
   the polygon area, **per normal cluster** (normals quantised to the three axes of either body plus ±,
   because voxel faces give a handful of exact normals). This is the usual contact-manifold practice (Bullet
   keeps four; I am recalling it, not citing it) and is the "reduction" McNeely's culling stands for. For
   resting bodies it yields the stable 4-point support polygon a stack needs; without it the solver sees
   hundreds of coincident constraints per face and effective mass is wrong.
5. **Normals on curved shapes.** Cube faces give axis-aligned normals; a voxel sphere rolling on a plane gets
   stair-step "ratchet" contacts. Options: (a) smooth normals from the occupancy gradient or a one-level
   distance transform (McNeely's distance field; Macklin's SDF); (b) analytic colliders for the primitive
   shapes (the specs know their `kind`). **Not decided by reading.** A rolling-sphere benchmark in the slice
   (closed form `a = (5/7) g sin θ`, tilting *gravity* rather than the floor so the floor is exact) measures the
   loss; criterion: write the acceptable deviation first, decide (a) or (b) from the number.
6. **Analytic fast path = oracle.** Sphere/box/plane (and box–box by SAT + clipping, as in
   [Catto 2006](https://box2d.org/files/ErinCatto_SequentialImpulses_GDC2006.pdf)) as a *separate* code path used
   by tests: the voxel path must agree with it on shapes where both apply, within a stated envelope (S3 already
   has analytic volume and inertia per primitive to compare against).
7. **Tunnelling.** Voxel bodies thinner than `v h` are missed by discrete tests: substep h = 1/1200 s at 10 m/s is
   8 mm per substep, so with pitches ≥ 1 cm the substepped solver with speculative contacts (separated contacts
   enter with bias `s/h`) suffices; faster bodies take a swept test. Measure, do not assume.

---

## 3. Destruction and fracture

- **Splitting = connected components of leaves** under face adjacency (a leaf of one level touches leaves of
  another across a face). Union-find with path compression is near-linear
  ([Tarjan 1975](https://doi.org/10.1145/321879.321884), not read; textbook); Box2D uses depth-first search over
  islands ([source above](https://box2d.org/posts/2023/10/simulation-islands/)); Gustafsson's 2014 post used
  repeated min-id propagation over a few dozen convex shapes
  ([post](https://blog.voxagon.se/2014/05/13/cracking-destruction.html)). For octree leaves, find neighbours by Morton arithmetic at
  the leaf's level and the two adjacent levels. Only carved bodies need relabelling; carve-free bodies are
  never re-examined. Fracture by *volumetric approximate convex decomposition*
  ([Müller et al. 2013](https://doi.org/10.1145/2461912.2461934), not read) is a different, shape-based
  approach.
- **Exact mass moments, the point of the question.** S3 computes node moments in `f64` bottom-up
  (`Moments::combine`), mass `= count·8^level·unit`, `unit = M/Σ samples`. Floating point sums are not
  associative, so "the pieces' moments add to the parent's" holds only to rounding, and depends on traversal
  order. **Recommendation:** also carry, per body, exact integer accumulators over the lattice:
  `N = Σ count·8^level` and the first, second (and, if wanted, third) moments of `count·8^level` against cell
  centres, in *integer units of half the finest cell* so that centres are integers. Mass is `N·unit` with one
  rounding; centre of mass and inertia are single divisions. Because the sums are integers (`i128` is ample:
  10⁹–10¹¹ cells × coordinates ≤ 2¹¹ squared stays far below 2¹²⁷), carving, splitting and merging are exact
  and independent of order, so `Σ N_child = N_parent` *bit for bit* and the same for the moments.
  Children must take the **parent's `unit`**, not recompute `M_c/Σ samples` (S3's `Tree::build(grid, leaves,
  mass)` takes a mass; add a constructor taking `unit`, otherwise density can differ by an ulp). Teardown does
  the same in spirit ("destruction rewritten in fixed-point integer math", [2026](https://blog.voxagon.se/2026/03/13/teardown-multiplayer.html)).
  Combined parent→children: use the parallel-axis shift in integer arithmetic where possible, f64 only for the
  final divisions.
- **Velocities at a split.** Each child takes `v_c = v + ω × (c_c − c)` and the parent's `ω` (so each particle
  keeps its velocity). Then Σp, ΣL and ΣKE are conserved in exact arithmetic (a rigid body's particles are
  unchanged); in floating point the residual is ~1e-16 relative. Test with that tolerance and a control arm
  that perturbs one child.
- **Cost bound.** A carve touches `O(leaves)` once; the integer update is `O(leaves removed)`. The
  connectivity relabel is `O(leaves α)`. A body of 10⁷ leaves relabels in tens of milliseconds, not
  per-frame; estimate, not measured.

---

## 4. Determinism and replay (native and web)

Cavendish replays journals bit-for-bit (`plan/REBUILD.md` §2.10, S2 exit). For the dynamics:

| source of divergence | evidence | rule for S7 |
|---|---|---|
| Transcendentals | Rust: `sin/cos/exp/ln/powf` "non-deterministic … can even differ within the same execution" ([std](https://doc.rust-lang.org/std/primitive.f64.html)); Gaffer: AMD and Intel differ on `sin/cos/tan` ([Floating point determinism](https://gafferongames.com/post/floating_point_determinism/)); Box2D wrote its own `sin/cos/atan2` after `atan2f` differed across platforms ([Box2D determinism](https://box2d.org/posts/2024/08/determinism/)) | only `+ − × ÷ √` in the solver; series for `sin/cos` (B6); for `atan2` of small angles, an atan series or avoid the angle altogether (quaternion normalised by one `sqrt`) |
| Fused multiply-add | `mul_add` is specified as IEEE `fusedMultiplyAdd`, rounded once, "guaranteed not to change" ([std](https://doc.rust-lang.org/std/primitive.f64.html)); Wasm `fma` likewise; but the relaxed-SIMD `madd` is "either rounded twice … or … once" per host ([relaxed-simd](https://github.com/WebAssembly/relaxed-simd/blob/main/proposals/relaxed-simd/Overview.md)); Box2D requires `-ffp-contract=off` | never call `mul_add` in the solver (on wasm without FMA it is also slow software); never enable `relaxed-simd`; native builds must not turn on contraction. **I could not find a Rust reference that says plain `a*b+c` is never contracted**; the nightly [`fmuladd`](https://doc.rust-lang.org/nightly/core/intrinsics/fn.fmuladd.html) docs say that intrinsic may or may not fuse. Settle by test (below). B1 shows the *size* of the effect: fused vs unfused Verlet differ by 2e-13 m after 10 s: invisible to physics, fatal to a bit-for-bit hash |
| IEEE basics on Wasm | operators use round-to-nearest-even; the only nondeterminism is NaN payload/sign ([Wasm numerics](https://webassembly.github.io/spec/core/exec/numerics.html)) | non-NaN arithmetic is bit-identical to native; trap or assert on NaN |
| Order of reductions | Gaffer ("order of operations"); Box2D: contact order from threads is nondeterministic and fixed by per-thread bit arrays merged on the main thread | island-parallel solve, results written to island-owned slots, merged in sorted `(bodyA, bodyB, keyA, keyB)`; no `rayon::reduce` over floats; hash maps iterated in sorted order or replaced by sorted `Vec` / `BTreeMap` |
| Thread count | Box2D tested "Falling Hinges" at several thread counts | the same test, `dynamics.thread_count_invariant` |
| Integer moments | above | exact; order-free |
| Build flags | Gaffer: replays break across compilers and optimisation settings; same binary on the same architecture is the realistic claim | pin the toolchain (`rust-toolchain.toml` already does: `channel = "1.100.0"`), no `-ffast-math`-like flags; record in the validation report |

- **Cross-target test**, copying Box2D's: a scene where bodies fall, stack and sleep; assert (a) the step at
  which every body is asleep and (b) a hash of all final poses (as raw bits), identical on native and in the
  web build. Box2D runs this in CI across MSVC, Clang, GCC on x64 and ARM and says determinism is "a brittle
  feature". Box2D also states it does **not** provide *roll-back* determinism (recreating a world may change
  results through free-list pools), which matters for Cavendish's undo and "re-run from the log": a replay
  must rebuild the world from the journal in the same insertion order, never from a snapshot of containers.
- **Fixed step.** One `h` for the whole run, `h` a power of two or a rational with exact t = n·h bookkeeping
  (B1: a dyadic step avoids a systematic `g·h` rounding drift).
- **Where this disagrees with v2:** `rigid.rs` sleeps single bodies and uses `S::cos`; both change (above).

---

## 5. CPU or GPU; scale; how motion feeds the read

- **S7's scale per the plan** is "tens of bodies" at exact rest on the CPU, and `plan/REBUILD.md` §3 puts
  "GPU only for thousands of bodies". The build machine's GPU is a 1660 SUPER, "correctness-only" (owner's
  memory note). My estimate (not measured): a contact costs ~100 ns per sweep on one core; 1000 bodies × 3
  contacts × 4 points × 20 substeps × 2 sweeps (solve + relax) ≈ 5×10⁵ point-sweeps ≈ 50 ms single-threaded,
  ~5 ms on the 12 cores with island parallelism. Gustafsson's reported ~5 ms per step on 32 threads
  ([80.lv](https://80.lv/articles/see-what-s-new-in-teardown-creator-s-custom-voxel-physics-engine)) is the one
  public number of the right order, scene unknown. **CPU `f64`, island-parallel**, is the choice for S7.
- **The GPU's actual track record:** [Harada, GPU Gems 3 ch. 29](https://developer.nvidia.com/gpugems/gpugems3/part-v-physics-simulation/chapter-29-real-time-rigid-body-simulation-gpus):
  rigid bodies as clusters of spheres on a grid, 16 384 chess pieces at 168 iterations/s on an 8800 GTX;
  the author says accuracy "could be improved" (a more precise narrow phase suggested) and stacking is not
  analysed. [Macklin 2019](https://arxiv.org/abs/1907.04587) runs a GPU conjugate-residual NCP solver for
  robotics. Both are single-precision-class, non-reproducible-by-default, and approximate the geometry:
  **the opposite of what the exact-rest requirement and the replay rule need.** The `million-bodies` demo
  (§9 of the plan) therefore needs a *different mode* (loose particles, no exact rest; ballistic or granular
  with a GPU solver) and a separate research note; it is not S7.
- **Feeding the read.** S3's body is a tree in its own frame, "the shape's centre at the body's position"
  (`docs/slices/S3.md` §2). A dynamic body is exactly `(x(t), q(t))` applied to that tree: the read transforms
  the query points (or node boxes) into the body frame, as `RecordedRigid` did in v2 (a recorded pose carrying
  a fixed cloud). **No re-voxelising**, ever, while the body is rigid; a carve or split makes new trees (§3),
  once. Interpolation of recorded tracks: position by cubic Hermite (exact for flight segments, which are
  quadratic), attitude by a *proper* spherical interpolation or by integrating the flight segment's exact
  rotation (v2 used "normalised linear blend", whose angular-rate error is O((Δθ)²) per step and is not
  within the µrad budget if poses are recorded at, say, 100 ms; **check in the slice**: closed form is a
  constant-rate rotation about a fixed axis).
- **How much pose accuracy the read needs.** For a point-like source at range r the gradient scales as r⁻³,
  so a position error δ gives a relative gradient error ≈ 3δ/r. At r = 1 m, δ = 1e-10 m is 3e-10 (negligible);
  a 0.1 mm rest penetration is 3e-4. My estimate, from the power law, not from the instrument model: take the
  tolerance for rest penetration from a sensitivity run of the S1 read (move the body by δ, see the phase),
  and set the contact stiffness (K, hence h) so the *sum* is inside it.

---

## 6. Validation

Closed forms (all with a control arm, per CLAUDE.md), each with the measurement made for this note where one
exists:

| case | known answer | S7 test (suggested id) | tolerance / control |
|---|---|---|---|
| free fall, projectile | `x = x₀+v₀t+½gt²` | `free_fall_matches_closed_form` | 1e-10 m over 100 s. **Control:** stepped symplectic Euler, which must fail (B1: 0.2 m at 10 s) |
| torque-free asymmetric top | |L| constant, E constant, Euler equations | `free_rotation_conserves_l_and_e` | |L| 1e-13 relative, E 1e-6 at h = 1/240 (B2: 1.2e-7) or 1e-10 with 4th order. **Control:** explicit Euler (B2: 2.5e-3) |
| bounce | apex after k impacts `e^{2k} h₀`; impact time `t_k = t₀(1+2e(1−e^{k−1})/(1−e))` | `bounce_height_follows_restitution` | time 1e-12 s with TOI (B4: 1e-15 s). **Control:** detect-and-project (B4: 36 ms late at 1/240 s) |
| two-body collision | `v₁′ = ((m₁−e m₂)v₁ + (1+e)m₂v₂)/(m₁+m₂)`; momentum conserved exactly; for e = 1, energy | `two_spheres_exchange_momentum` | 1e-12. Control: wrong mass ratio |
| Newton's cradle | sequential collisions of three equal balls, e = 1: `(0, 0, 1)`; simultaneous restitution gives `(−⅓, ⅔, ⅔)` (B7) | `cradle_with_gaps_passes_the_impulse` | balls with gaps ≥ `v h` so collisions are sequential; **control:** touching balls must give `(−⅓,⅔,⅔)`, documenting the model. I could not find a primary source for the cradle as a *benchmark*; the numbers above are my derivation, checked by B7 |
| sliding with friction | stop distance `v₀²/(2μg)`, stop time `v₀/(μg)` | `sliding_box_stops_where_coulomb_says` | 1e-5 m at 1/240 s (B5: 7.9e-6). Control: μ = 0 never stops |
| incline | slides when `tan θ > μ`, `a = g(sin θ − μ cos θ)`; sticks and sleeps when `tan θ < μ` | `incline_threshold_is_at_atan_mu` | **tilt gravity, not the floor** (stairs); bracket θ = atan μ ± 1% |
| rolling | solid sphere `a = (5/7) g sin θ` | `rolling_sphere_on_incline` | **measures the voxel-normal question of §2.5**; criterion set before measuring |
| stack at rest | every pose constant after sleep; penetration ≤ stated | `stack_of_ten_rests_and_sleeps` | B3's 1D numbers as the model's expectation: 0.05–0.1 mm at K = 20. Control: K = 1 must fail |
| heavy-on-light stack | none closed form; stability only | `mass_ratio_1000_stays_up` | document the limit (B3: 10 to 14 mm at K = 20); **do not** claim 1000:1 solved |
| momentum, angular momentum, energy | ΣP, ΣL conserved in any free collision; E non-increasing | `dynamics_conserve_momentum` | 1e-12 relative |
| sleep | pose bits equal over 10 s | `slept_pose_is_bit_constant` | control: sleeping disabled, bits differ |
| split | Σ N exact, ΣP, ΣL, ΣKE 1e-12 | `split_conserves_exactly` | control: perturb one child |
| determinism | hash of poses, native = wasm; thread count 1 = 12 | `dynamics_hash_matches_across_targets` | control: one `mul_add` in the solver, hash must change (this also answers whether the compiler contracts) |

**Published stack benchmarks.** I could not read a source giving numeric stacking benchmarks:
[Bender, Erleben, Trinkle 2014](https://doi.org/10.1111/cgf.12272) (a state-of-the-art report) and the earlier
Eurographics STAR ([10.2312/conf/EG2012/stars/095-134](https://doi.org/10.2312/conf/EG2012/stars/095-134)) should
list them, but the PDF copy I fetched was corrupt. The practical, published *scenes* are: Catto's large
pyramid and stack samples in Solver2D (video only; "The post gives no numeric stacking results",
[Solver2D](https://box2d.org/posts/2024/02/solver2d/)) and Macklin's heavy-box-on-capsule-stack (Fig. 9 of
[Small steps](https://doi.org/10.1145/3309486.3340247)) — qualitative. Hence the recommendation to **define the
numeric stack criterion ourselves** (B3's model) rather than to cite one.

---

## 7. Recommended design for S7, costs, tests

**Module layout** (inside `cav-sim`, dependencies only `cav-core`): `dynamics/{flight,rotation,contact,
broad,solver,island,split}.rs`. A `World` owns bodies in a `Vec` indexed by creation order; a step is a
pure function of (state, inputs).

1. **State per body**: pose `(x, q)`, `v`, `ω` (world), tree handle, integer moment accumulators, `asleep`,
   `flight: Option<Flight>`.
2. **Step (fixed h_frame = 1/60 s, K substeps, h = h_frame/K)**:
   a. flight bodies: evaluate closed form; test the swept AABB against the world (broad phase once per frame);
   b. if a candidate contact lies within the frame, locate the time of impact by the closed form (a quadratic
      in the normal direction) and cut the frame there, as B4;
   c. contact bodies: per substep: `v += h g` → warm start → solve (soft, stiffness `min(HZ, 0.25/h)`) →
      `x += h v` (rotation by the splitting) → relax; once per frame: dual-tree midphase, reduction, island build;
   d. after the last substep: restitution and friction (velocity level), sleeping timers, island sleep;
   e. a body leaving contact re-enters a flight segment with its current `(t, x, v, q, L)`.
3. **Contact softness is tuned for rigidity**, not Box2D's 30 Hz. `HZ = 0.25/h` makes the penetration scale as
   `h²`; at K = 20 expect ≈ 0.05–0.3 mm for ten boxes (B3), larger for heavy tops.
4. **Parallelism**: islands solved on a rayon pool, each writing its own slots, merged sorted. Single-island
   scenes (one big pile) stay serial in S7; Gustafsson's "parallel solver for big piles" is a later task
   (graph colouring or mass splitting, [Tonge 2012](https://doi.org/10.1145/2185520.2185601)).
5. **Recording**: per body, flight segments and contact-state samples at a fixed interval; a `RecordedRigid`
   plays the quadratic exactly and interpolates contact states by cubic Hermite plus constant-rate rotation.
6. **Costs** (estimates, to be measured in S7): contact solve O(contacts × K) per frame; midphase
   O(overlapping node pairs); relabelling only on carve. For tens of bodies, well under 1 ms a frame on one
   core; for 10³, a few ms across cores (§5).

**First three tasks, in order of risk**: (i) the flight + TOI + sleeping core with the closed-form tests
(B1, B4, B5, B2 are the specification); (ii) the stack in 3D with analytic box colliders and the solver
comparison B vs C; (iii) the voxel midphase with reduction and the rolling-sphere decision. Destruction and
the integer moments can run in parallel to (ii) as they are independent.

---

## 8. Benchmarks, in full (scripts and outputs in `research/rigid-bodies/`)

- **B1 `b1_freefall.py`.** Free fall, `x₀ = 100 m`, `v₀ = 3 m/s`, `g = −9.80665`. Position errors vs closed
  form at T = 10 s: symplectic Euler −0.204 m (h = 1/240), −0.817 m (1/60); leapfrog −1.3e-11; velocity Verlet
  −9.7e-12 (the last two limited by `g h` rounding). h = 1/256: Verlet −2.2e-12 (10 s), −2.2e-8 (100 s), +1.3e-5
  (1000 s). Fused vs unfused Verlet: 2.3e-13 m apart after 10 s. (The "closed form from epoch" lines in the
  output are tautological, the same expression as the oracle; they only report one ulp of x, 6e-14 m at
  10 s to 9e-10 m at 1000 s, where x is near 5×10⁶ m. That ulp is the true floor of any `f64` position, so a
  1e-10 m exit holds only while |x| ≲ 10⁶ m, which the plan's tower-relative frames already ensure.)
- **B2 `b2_rotation.py`.** See §1.1's table. The reference is RK4 at h = 1/8192 with an exponential-map
  attitude update, so differences below ~3e-9 rad are at the reference's own floor.
- **B3 `b3_stack.py`**, outputs `b3_hz30.out`, `b3_hzmax.out`. 1D; contacts in a chain; ground rigid. Contact
  softness parameters from Solver2D (zeta 10, push-out 3 m/s) as I recall the Box2D defaults; **I did not read
  Box2D's source**, only Catto's description. A first version without warm-start application gave nonsense and
  was fixed; the numbers above are from the fixed version. These are a 1D model of the solver *families*;
  they do not test friction, rotation, or contact generation.
- **B4 `b4_bounce.py`.** Ball from 1 m, e = 0.5: detect-and-project: impact-time error 0.140 s (1/60), 36 ms
  (1/240), 7.8 ms (1/960); apex error 17 mm, 3.1 mm, 0.5 mm. With time of impact: impact times to 1e-15 s;
  the residual apex error (1.7e-4 m to 1.1e-6 m) is my *sampling* of the peak at step ends, not the physics.
- **B5 `b5_slide.py`.** See §1.2.
- **B6 `b6_trig.py`.** Taylor/Horner series vs 60-digit decimal: ≤ 0.95 ulp sin, ≤ 0.65 ulp cos for
  |x| ≤ 0.5 with 8 terms; 6 terms is insufficient for cos above ~0.25 (6.7 ulp at 0.5).
- **B7 `b7_cradle.py`.** Three equal balls touching, simultaneous velocity-level restitution, GS iterations
  1, 2, 10, 100, 10 000: `(0, ½, ½)`, `(−¼, ⅝, ⅝)`, … → `(−⅓, ⅔, ⅔)`; momentum 1 and energy ½ conserved.

---

## 9. What I could not verify, and how each is settled

1. **Rust/LLVM never contracts `a*b+c`** (no primary source found). Settle: the cross-target hash test with a
   deliberate `mul_add` as control; check `RUSTFLAGS`/`target-cpu` in `xtask`.
2. **`libm` crate determinism** (docs.rs page gives no statement; v0.2.16). Moot if series are used.
3. **Teardown's voxel contact generation**: no primary text found. My pipeline (§2) is from McNeely 2006, S3's
   tree and general practice, not from Teardown.
4. **Box2D's soft-contact constants and restitution code** (I read Catto's descriptions, not the source; the
   web fetches were summaries by a small model, so quoted phrases are as returned, not checked against the
   pages' exact text). Settle: read `solver.c`/`contact_solver.c` of Box2D v3 when implementing.
5. **Numeric stack benchmarks in the literature** (STAR not readable). Our own criterion stands in (§6).
6. **Sleep thresholds, rest-penetration tolerance, voxel-normal quality, tunnelling limit, performance at 10³
   bodies**: need measurement in S7. Write the criteria first (CLAUDE.md): e.g. sleep when |v| < 1e-4 m/s and
   |ω| < 1e-4 rad/s for 0.5 s *and* the island's residual from a relax sweep < threshold; measure solver noise at
   rest first.
7. **3D behaviour**: B3 is 1D. Friction cones, rotation in stacks, box–box manifolds are not covered by any
   number here.
8. **Order of accuracy of attitude interpolation for recorded tracks** (the v2 normalised-lerp claim is from
   reading `physics.md`, not from a measurement).

## 10. Further questions (not chased)

- Million-body mode: what is the right model (particles, granular/MPM, ballistic only) and what does the read
  need from it? Separate note before that demo.
- Structural collapse (support graph, §5.2 table in the plan) and the voxel-FE modal track the plan gives S7.
- Continuous collision for fast thin bodies; speculative margins sized by the read.
- Rolling resistance and contact-patch physics for voxel wheels; whether to smooth normals with an SDF.
- Articulated bodies/joints (v2 had none for rigid dynamics; skinned bodies are kinematic in S4).
- Exact Jacobi-elliptic free rotation vs high-order splitting for *very* long flights.
- Cross-target determinism of the *fluid* and read kernels (separate from this note).

---

## Papers I could not read

Read in full or in part from a primary copy: Macklin et al. 2019 (text via PDF), Müller et al. 2020 (text via
PDF), XPBD and PBD (text via PDF, searched), Catto 2006 slides, McNeely et al. 2006 (follow-up), Rust and
WebAssembly documentation. Blog posts by Catto and Gustafsson, the Gaffer article and the 80.lv article were
read through a summarising fetch tool, so treat quotations as the tool's. The following I could not read
(paywalled, corrupt copy, or only an abstract/listing seen):

- McNeely, Puterbaugh, Troy 1999, *Six degree-of-freedom haptic rendering using voxel sampling*: https://doi.org/10.1145/311535.311600
- Tonge, Benevolenski, Voroshilov 2012, *Mass splitting for jitter-free parallel rigid body simulation*: https://doi.org/10.1145/2185520.2185601
- Bender, Erleben, Trinkle 2014, *Interactive simulation of rigid body dynamics in computer graphics*: https://doi.org/10.1111/cgf.12272
- Bender, Erleben, Trinkle, Coumans 2012, Eurographics STAR version: https://doi.org/10.2312/conf/EG2012/stars/095-134
- Macklin, Erleben, Müller, Chentanez, Jeschke, Corse 2020, *Local optimization for robust signed distance field collision*: https://doi.org/10.1145/3384538
- Macklin, Erleben, Müller, Chentanez, Jeschke, Makoviychuk 2019, *Non-smooth Newton methods for deformable multi-body dynamics*: https://doi.org/10.1145/3338695
- Dullweber, Leimkuhler, McLachlan 1997, *Symplectic splitting methods for rigid body molecular dynamics*: https://doi.org/10.1063/1.474310
- Lee, Leok, McClamroch 2007, *Lie group variational integrators for the full body problem*: https://doi.org/10.1016/j.cma.2007.01.017
- Van Zon, Omelyan, Schofield 2008, *Efficient algorithms for rigid body integration using optimized splitting methods and exact free rotational motion*: https://doi.org/10.1063/1.2889937
- Hairer, Lubich, Wanner 2006, *Geometric numerical integration*: https://doi.org/10.1007/3-540-30666-8
- Müller, Chentanez, Kim, Macklin 2013, *Real time dynamic fracture with volumetric approximate convex decompositions*: https://doi.org/10.1145/2461912.2461934
- Tarjan 1975, *Efficiency of a good but not linear set union algorithm*: https://doi.org/10.1145/321879.321884
- Harada 2007, *Real-time rigid body simulation on GPUs* (GPU Gems 3 ch. 29; read via fetch summary, no DOI): https://developer.nvidia.com/gpugems/gpugems3/part-v-physics-simulation/chapter-29-real-time-rigid-body-simulation-gpus

(Read via PDF text: Macklin et al. 2019 *Small steps* https://doi.org/10.1145/3309486.3340247; Müller et al. 2020
https://doi.org/10.1111/cgf.14105; Macklin et al. 2016 *XPBD* https://doi.org/10.1145/2994258.2994272; Müller et al.
2007 *PBD* https://doi.org/10.1016/j.jvcir.2007.01.005.)
