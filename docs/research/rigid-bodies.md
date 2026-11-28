# Rigid dynamics and voxel contact for S7 ("Things that fall and stack")

Research note, 25 Nov 2026. Question: which integrator, contact solver, contact generator, destruction
and determinism design lets S7 meet its exits (`plan/REBUILD.md` §9: free fall ≤ 1e-10 m, a slept pose
bit-constant, box on box rests), and how is each part validated against a known answer?
Benchmarks quoted here were written and run for this note (Python, IEEE doubles, scripts and raw output in
`docs/research/rigid-bodies/`, one file per benchmark, B1 to B9; B8 and B9 were added on 25 Nov after reading the primary sources). Python's `float` is the same binary64 as Rust's
`f64`, so the arithmetic is the arithmetic; the *code* is a model of the algorithms, not the S7 code.

**Updated 27 Nov 2026** from two more owner-fetched PDFs. `tonge2012.pdf` is the main paper again (ACM TOG 31(4),
Article 105, 8 pp., p. 105:1 reference block), *not* the convergence supplement: the supplement is still missing (§9
7b now lists exactly what it holds). `vanzon2008.pdf` is the 2008 *J. Chem. Phys.* note already read (doi:10.1063/1.2889937),
*not* either van Zon & Schofield 2007 paper: both are still missing (§9 7a). Its reference list confirms their
venues (Refs. 5, 8). Small corrections: the 2008 note is two printed pages (136102-1–2), not three (the PDF's
first page is the publisher's cover sheet); Tonge's card house is 48 cards (§7), and the 58 is Table 1's body count.

**Updated 28 Nov 2026** from two more: **van Zon & Schofield 2007, *Phys. Rev. E* 75, 056701** (read whole, Eq. (5)–(11)
from the page image) and **Erleben 2007, ACM TOG 26(2), Article 12** (§1–5 read, §6 skimmed). The PRE paper gives the
exact free-rotation step as a composition `P(h) = R₁(L̃*) R₂(ψ) R₁ᵀ(L̃)` with a recursive, special-function-free
ψ (Eq. (9)–(11)), but takes the body-frame `L̃*` from the elliptic-function Euler solution of the JCP paper, which
is still missing; on a free water-like top it holds attitude to 10 % for ~16 ms where the 1,2,3,2,1 splitting
(their "MRDL") lasts ~117 ps at the same step (§V A). So §9 7a narrows to the JCP paper. Erleben 2007 is the
"dense structured stacks" source Bender cites: it is a method (velocity-based shock propagation) and a set of
scenes judged visually and by wall-clock time, not a numeric benchmark; §6's "define the criterion ourselves"
stands. Silcowitz et al. 2009 and Tonge's supplement are still missing. The JCP paper's DOI given from memory
(10.1016/j.jcp.2006.11.020) was wrong; it is 10.1016/j.jcp.2006.11.019 (Papers, below).

**Updated 28 Nov 2026 (third batch)** from three more owner-supplied PDFs, all read whole: **van Zon & Schofield
2007, *J. Comput. Phys.* 225, 145–164** (the exact free asymmetric top), **Tonge, Benevolenski & Voroshilov 2012,
the supplement** ("Proofs for 'Mass Splitting for Jitter-Free Parallel Rigid Body Simulation'", 2 pp.), and
**Silcowitz, Niebe & Erleben 2009**, VRIPHYS (the Fischer–Newton contact solver). §9 7a and 7b are closed and a
benchmark was added (**B10**, `b10_exact_rotor.py`, `b10.out`). What changed: (i) the exact free-rotation solution
is now implemented from the JCP paper's §3.3 and §4.2 and agrees with B2's reference to 2.9e-9 rad (the
reference's own floor) and with the fourth-order splitting at h = 1/240 to 3.5e-11 rad, so the Yoshida row of
B2's table was limited by the RK4 reference, not by the splitting; (ii) the paper says nothing about the
separatrix, and B10 finds its algorithm loses accuracy in `f64` as `2E → L²/I₂`: 1.5e-8 rad at 20 s for a
relative distance of 1e-8, 2.2e-5 rad at 1e-12, while the splitting stays self-consistent to 1e-11 there; (iii)
the supplement's proofs hold for frictionless contact only and do not touch mass ratios. **No decision moves**:
the portable 1,2,3,2,1 splitting stays the flight integrator (the exact form needs `sn`, `cn`, `dn`, an
incomplete elliptic integral, `exp`, `cosh`, `sinh`, `sin`, `cos` and `atan2`, none of them bit-portable, §1.1),
and the exact solution becomes the free-top *oracle* in the S7 tests, away from the separatrix (§6). B8's
conclusion on mass splitting stands, and Silcowitz et al. support §7's exact island solve for heavy-on-light
stacks while warning that their Newton method struggles once friction is large (§6).

**Changed after reading the primary sources (25 Nov 2026).** The owner supplied twelve PDFs the first draft
could not open (not copied into the repository). Read from the PDF text or page images: McNeely et al. 1999
(all 8 pages), Tonge et al. 2012 (all 8; the supplement with the convergence proofs is not in the PDF), Macklin
et al. 2019 (abstract, §1–2, §8.1, §10 read; §3–9 searched by heading and keyword) and 2020 (§1–2, §6–10 read;
§3–5 by heading), Müller et al. 2013 (§1–2, §3.1–3.2, §5, §6), Dullweber et al. 1997 (§III, App. A–C), van Zon
et al. 2008 (both printed pages), Lee et al. 2007 (abstract, §4.3, §4.4, §5; the derivations not followed),
Hairer–Lubich–Wanner 2006 (§VII.5.3, pp. 284–285, and the Triple Jump, Example II.4.2), Bender et al. 2012 STAR
(§4.2, §7.3) and 2014 (§4.1–4.2), Tarjan 1975 (abstract, introduction, Theorems 4, 13, 16). Two benchmarks were
added (B8, B9, §8). What changed, in order of weight:
* **The 1000:1 stack has a known remedy, and it is not mass splitting.** Tonge's method (Eq. 15, §4) is a
  Jacobi iteration with each body's mass divided by its contact count *in the effective mass only*; it exists for
  order-independent parallel solves without jitter; its one large-mass-ratio scene (the fracture demo, Fig. 1
  caption, §7) is scored by kinetic energy, never by penetration; and it says its convergence "is worse than PGS
  in all the tests" and shows up as compliance (§7, "Limitations"; the card house "visibly bounces"). **B8** puts
  it next to PGS at equal sweeps: no better (worse at 1000:1 and 4096:1). What the sources say works is a *direct or
  Krylov solve*: Macklin 2019 §10.1.5 and Fig. 14 (a 4,096:1 stack of boxes: "relaxation methods struggle to reduce
  error, while Krylov methods form stable structures"; §10.4, Fig. 18: Jacobi and Gauss–Seidel stagnate), Bender
  et al. 2012 §4.2 ("direct methods … ideal to handle problems such as large mass ratios") and §7.3 ("most
  iterative methods … converge so badly that even infinite amount of iterations won't help"). **B8: an exact
  solve of the 1D contact LCP each substep holds 1000:1 and 4096:1 stacks at zero penetration even at K = 4.**
  The B3 soft-step sag (10 mm at 1000:1) is *compliance*, not a convergence shortfall. **Recommendation moved:**
  S7 gets an exact island solve for heavy-on-light islands (§1.2, §7); the "do not claim 1000:1 solved" line of
  §6 now reads "solved in 1D, to be shown in 3D with friction".
* **McNeely's voxmap point-shell does not fit octree-against-octree contact as published**, and two things the
  first draft attributed to it are from the 2006 follow-up, not the 1999 paper (§2): there is no distance field
  (a 2-bit type map, Table 1; depth is the distance to a tangent plane through the voxel centre, §3), and the
  "reduction" is force averaging over 10 or more simultaneous point-voxel hits (§5.2), not contact culling. It is
  one dynamic object against a *merged, static* world map (§4.4), with a fixed three-level 512-tree on purpose
  (§4.3), a penalty force, and a two-voxel offset force layer so exact surfaces never touch (§4.2): gaps of a voxel
  or two, which an exact-rest requirement cannot take. The recommended dual-tree descent stands; the tunnelling
  limit (no point faster than `s/Δt`, §5.3) is adopted.
* **The splitting choice stands, and its two open items are closed.** The note's `1,2,3,2,1` scheme *is* Dullweber
  et al. Eq. (7), also HLW Eq. (5.31); each sub-flow is an exact rotation so `|L|` and orthogonality hold by
  construction (§III), second order by time symmetry (App. B). The trig-free alternative the note left untested
  is the paper's own implementation (App. A: `cos φ → (1−φ²/4)/(1+φ²/4)`, `sin φ → φ/(1+φ²/4)`, the Cayley
  transform). **B9:** same energy error, attitude error ×4 at second order, indistinguishable at fourth order
  (Yoshida; both at the reference's 3e-9 rad floor), `|L|` to 1e-13: so the sin/cos series of B6 is optional. van Zon et al. (2008) is a two-page note that
  *uses* the exact free-rotation propagator (elliptic functions) inside Verlet-type splittings; it gives no
  formula for it (their Refs. 5 and 8, unread). For Cavendish's torque-free flights that exact propagator is not a
  splitting at all but a closed form like the translation: the better flight evaluator if it can be had, and the
  oracle the splitting should be measured against. Lee–Leok–McClamroch is implicit (Newton, 3–4 iterations to
  1e-15 per step, §4.4 Eq. 130) and aimed at *coupled* position–attitude potentials; not needed here.
* **Numeric stack benchmarks: confirmed that none exist.** Bender et al. 2012 §7.3: "No benchmark databases
  exist with both simulation setups and ground truth data", temporal convergence is "rarely" studied. The survey
  does list scenes worth adding to §6 (rattleback and tippe top for gyroscopic terms, heavy box on a light box,
  dense structured stacks, card house, brick wall, Newton's cradle as a test of simultaneous versus sequential
  contact, which is what B7 found).
* **Smaller corrections.** Macklin 2020 generates contacts from mesh *features* against an SDF and lists
  SDF-against-SDF contact as future work (§10): our case is open ground there. Müller 2013 is geometric
  (pre-defined fracture patterns, no stress, §6), uses flood fill for island detection (§3.1) and Tonge's solver
  (§5), and does not make bodies below a size threshold rigid at all (§5). Tarjan 1975: path compression alone is
  *not* near-linear (Theorem 4, Eq. 3: `O(m log n)` at `m = n`); union by weight *and* path compression is
  `Θ(m α(m,n))` (abstract; upper bound Theorem 13, lower bound Corollary 17), which is what §3 means.
* **Not changed:** CPU `f64`, closed-form flight, soft-step substeps as the default contact solver, per-island
  sleeping, integer moments, the determinism rules, dual-tree contact descent with reduction.

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
far ahead of iterations-with-Baumgarte at equal budget. **Heavy-on-light islands** (mass ratio in the hundreds
and up) go to an *exact* solve of the island's contact problem instead (a direct factorisation at S7's scale):
the soft step's sag there is compliance, and no iteration count removes it (B8; Macklin 2019 Fig. 14; Bender
2012 §4.2, §7.3). (3) **Contact generation walks the two octrees**
together (node-cube against node-cube, in the frame of one body), turns leaf-cube overlaps into
candidates keyed by the two Morton keys, and *reduces* them to at most four points per body pair; a
plain analytic fast path for sphere/box/plane is kept as the test oracle. (4) **Sleeping is per
island** (Box2D's design): a whole island freezes, poses are not touched, velocities set to exactly zero,
and a frozen body is skipped, so its pose is bit-constant by construction; thresholds are set from the
*measured* solver noise, not from game defaults. (5) **Determinism by construction**: IEEE-only
arithmetic (+ − × ÷ √), no `mul_add`, no platform `sin`/`cos`/`exp`/`atan2` (fixed-degree series, ≤ 1 ulp
for the small angles of a substep, B6; or the Cayley rational rotation of Dullweber et al. App. A, B9), no `relaxed-simd`, islands solved independently and merged in
sorted order, **mass moments kept as integer sums on the lattice** so that splitting, carving and merging a
body are exact and order-free. Destruction = connected-component labelling of leaves, with children
inheriting rigid-body velocities (momentum, angular momentum and kinetic energy then add up exactly in
exact arithmetic). Motion feeds the read as a pose (`x`, `q`) of an unchanged tree: no re-voxelising.

**Numbers that decided it (B1–B9, all in §8).**
- A stepped free fall misses the 1e-10 m exit: the PBD/PGS update (`v += g h; x += h v`, symplectic Euler) is
  0.20 m off after 10 s at 1/240 s; velocity Verlet is exact for constant `g` but its rounding drifts to
  2e-8 m at 100 s. A closed-form flight segment has no drift.
- Game-style rotation (explicit Euler on Euler's equations) drifts |L| by 2.5e-3 relative in 20 s at 1/240;
  the splitting keeps |L| to 1.6e-14 and E to 1.2e-7 (hard, near-intermediate-axis case).
- At equal solver budget, a 10-box stack penetrates by 1.1 mm (PGS + Baumgarte, K = 20), 0.10 mm (XPBD) or
  0.05 mm (soft step with a stiff contact); a Box2D-default 30 Hz contact never gets below 5.0 mm. Penetration
  falls like 1/K² for the substepped methods. A 1000 kg box on five 1 kg boxes defeats the iterative method
  (319 mm, still moving at 16 m/s) and is only roughly tamed by substeps (10 to 14 mm at K = 20). **B8** (added
  after reading the primary sources): an exact per-substep solve gives zero penetration at K = 4 for 1000:1 and
  4096:1; mass-splitting Jacobi (Tonge) is no better than PGS at equal sweeps.
- Fixed-step "detect, then project" contact misses a bounce's impact time by 36 ms at 1/240; solving the
  time of impact inside the step gets 1e-15 s.
- Simultaneous restitution is *not* Newton's cradle: three touching equal balls with e = 1 give
  v = (−⅓, ⅔, ⅔), not (0, 0, 1), at any iteration count (B7). The cradle test must have gaps.

**Not settled by reading (resolve in the slice, §9):** whether the exact island solve survives 3D friction
(B8 is frictionless and 1D) and what mass ratio should trigger it; the penetration/sag tolerance the instrument can
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
  second-order, symplectic, conserves |L| to rounding and keeps E bounded.
  What the primary texts say (all read, 25 Nov 2026):
  * [Dullweber, Leimkuhler, McLachlan 1997](https://doi.org/10.1063/1.474310), *J. Chem. Phys.* 107, 5840–5851:
    the kinetic energy splits into one term per principal axis, each with an exact flow that is a plain rotation
    (§III, Eq. 6); the composition `G_x(Δt/2) G_y(Δt/2) G_z(Δt) G_y(Δt/2) G_x(Δt/2)` (Eq. 7, five rotations; App. A
    lists `R₁…R₅`) is time-symmetric and therefore second order (App. B, Eq. B1, "the time-symmetry property");
    `QᵀQ = 1` "never needs to be artificially enforced" because `Q` is only ever multiplied by orthogonal
    matrices (§III), and angular momentum is preserved by each sub-flow. Against quaternions with an adaptive
    Gragg–Bulirsch–Stoer integrator on TIP4P water the splitting keeps a stable energy and `|P_ang|` while the
    quaternion run drifts (Figs. 9–14; the quaternion run needed up to 10× the force evaluations for the same
    accuracy, §IV). **Appendix C**: for `I₁ = I₂` the kinetic flow is exact in one rotation (Eq. B2), and a
    near-symmetric body is better split as symmetric top + one planar rotation (three rotations instead of five). The
    same Eq. 7 splitting is HLW 2006 Eq. (5.31) ("Three Rotations Splitting", p. 284; with the centre rotation on
    axis 1 rather than 3, which is immaterial), and the symmetric-plus-rotation variant is on p. 285; Fassò (2003),
    HLW's pointer for "a careful study and comparison of various ways of splitting the kinetic energy", was not read.
  * [Van Zon, Omelyan, Schofield 2008](https://doi.org/10.1063/1.2889937), *J. Chem. Phys.* 128, 136102 (two
    printed pages): the *exact* free rotational motion of an asymmetric top "involves elliptic functions, elliptic integrals,
    and theta functions" and can be computed efficiently (their Refs. 5, 7, 8); with the exact kinetic propagator "any
    splitting scheme for … point particles can be transferred to rigid systems" (p. 136102-1). They combine it with
    optimised second-order (`ξ = 0.1932`, "HOA2") and fourth-order ("GIER4") splittings; on 512 TIP4P water
    molecules HOA2 is about 1.5× more efficient than velocity Verlet for energy fluctuation `R ≲ 1.5 %`, and
    GIER4 is best for `R ≲ 0.4 %` (Fig. 1). That is a *force-dominated* test; they give no free-top accuracy, and no
    formulas (those are in Refs. 5 and 8, not read). Re-read 27 Nov: the attitude propagator `Pᵢ(h)` of their
    Eq. (2) is "see Ref. 5 for specific forms" (p. 136102-1); the elliptic integrals and theta functions "can be
    implemented efficiently (Ref. 8) or avoided altogether using a recursive method (Ref. 5)", and the elliptic
    functions come from GSL (Ref. 7). So Ref. 5 (*Phys. Rev. E* 75, 056701, 2007) is the one to read first.
  * [Van Zon, Schofield 2007](https://doi.org/10.1103/PhysRevE.75.056701), *Symplectic algorithms for simulations of
    rigid-body systems using the exact solution of free motion*, *Phys. Rev. E* 75, 056701 (five pages; read 28 Nov,
    whole). Verlet-type splitting into an exact free step `φ⁽⁰⁾` (`q += hp/m`, `A ← P(h)A`) and a kick `φ⁽¹⁾`
    (`p += hF`, `L += hτ`), Eq. (5)–(6); "SIER2" is `φ⁽¹⁾_{h/2} φ⁽⁰⁾_h φ⁽¹⁾_{h/2}` (Eq. (7)) and "SIER4" Omelyan's
    nine-stage Forest–Ruth-like composition (Eq. (8)). The free step is `P(h) = R₁(L̃*) R₂(ψ) R₁ᵀ(L̃)` (Eq. (9)):
    `R₁` is an explicit matrix built from the body-frame angular momentum `L̃` (printed under Eq. (9)), `R₂(ψ)` a
    rotation about body z by `ψ = αh + αc ∫ w(s) ds`, `α = L/I_z`, `w = L²/L⊥²`, `c = 2I_zE/L² − 1` (Eq. (10)), and the
    integral is done without elliptic integrals or theta functions by two-point Hermite quadrature to `O(h^{2n+1})`
    with the derivatives of `w` from the Euler equations by a polynomial recursion `S_j(w)` (Eq. (11), p. 056701-2),
    raised in `n` "until ψ has converged". **What it does not give**: `L̃*` itself, which "can be computed from the
    free solution of the Euler equations [8,10], which involve Jacobi elliptic functions" (p. 056701-2), Ref. 10
    being the JCP paper ("to be published") and Ref. 8 Celledoni & Säfström, *J. Phys. A* 39, 5463 (2006). The
    printed equations hold for `2E > I_y L²`; otherwise one first rotates the principal frame, swapping x and z and
    reversing y (p. 056701-2, citing Ref. 10). Spherical and symmetric tops need no special functions (`ψ = hL/I_x`,
    note 17). **Numbers (§V A, free top with water's moments, 297 K):** error `δ = ⟨⅙ Tr[(A−A_ex)(A−A_ex)ᵀ]⟩^{1/2}`
    grows as `t/(10τ)`; at h = 1.66 fs, τ ≈ 117 ps for MRDL (McLachlan–Reich–Dullweber–Leimkuhler, i.e. our 1,2,3,2,1
    splitting), 0.91 µs for SEJ4, 16 ms for SIER2; at 8 fs, 7.6 ps, 2.1 ns and 48 ms. SIER2 costs 70–100 % more than
    MRDL per step. With forces (§V B–C) the gain shrinks to energy conservation like SEJ4's with MRDL's exact
    angular momentum, and in liquid water all methods are alike. Nothing is said about behaviour at the separatrix
    `2E = I_y L²`.
  * [Lee, Leok, McClamroch 2007](https://doi.org/10.1016/j.cma.2007.01.017), *Comput. Methods Appl. Mech. Engrg.*
    196, 2907–2924: Lie-group variational integrators for rigid bodies under a *mutual potential that couples
    position and attitude*. Implicit: each step solves for the incremental rotation `F ∈ SO(3)` by Newton iteration
    (§4.4, Eq. 130: "3 or 4 iterations are sufficient to achieve a tolerance of 10⁻¹⁵"); symplectic, momentum-
    preserving, self-adjoint and so second order (§4.3), orthogonality loss "grows linearly with the number of
    timesteps" (§4.3); Yoshida composition for fourth order is suggested, not tested (§4.3). One test (two dumbbells,
    §5): energy deviation 2.7×10⁻⁷, `‖I − RᵀR‖` 2.9×10⁻¹³, with a Runge–Kutta run failing to resolve the close
    encounter. No comparison with a splitting, and no free-top numbers.
  What this means for the choice. Cavendish's flights are torque-free, so the potential-coupling that makes Lee et
  al. necessary is absent; the explicit splitting is the right tool, and it is the one already written. If the exact
  propagator is obtained (Refs. 5 and 8 of van Zon et al.), a flight segment needs no integrator at all for
  attitude, like the quadratic for position, and the splitting becomes the test oracle's opponent rather than the
  implementation; that is the one thing in this section worth a follow-up read.
  **Read 28 Nov (third batch): the exact propagator.** [van Zon, Schofield 2007](https://doi.org/10.1016/j.jcp.2006.11.019),
  *J. Comput. Phys.* 225, 145–164. The attitude matrix `A` (lab → body) is `A = P A(0)` with
  `P = T₁′ T₂′ T₁′ᵀ(0)` (Eq. (3.15)): `T₁′` is built from the body-frame angular momentum `L̃(t)` alone (Eq. (3.13)),
  `T₂′` is a rotation about z by `ψ`, `ψ̇ = Ω = L(2E − I₃ω̃₃²)/(L² − I₃²ω̃₃²)` (Eq. (3.20)). Spherical top `P = U(−ωt)`
  (Eq. (3.25)); symmetric top a product of two constant-rate rotations (Eq. (3.31)). Asymmetric top (§3.3): with the
  "Jacobi ordering" (`I₁ > I₂ > I₃` if `2E > L²/I₂`, else `I₁ < I₂ < I₃`, Eq. (3.32); otherwise relabel axes by the
  fixed matrix of Eq. (4.10), x ↔ z and y reversed), `ω̃₁ = ω₁ₘ cn`, `ω̃₂ = ω₂ₘ sn`, `ω̃₃ = ω₃ₘ dn` of `(ω_p t + ε | m)`
  (Eqs. (3.33)–(3.35)), amplitudes, precession rate, parameter `m` and phase `ε` in Eqs. (3.36)–(3.42). The angle is
  closed: `ψ = A₁ + A₂t − arg ϑ₁(π(ω_p t + ε − iη)/(2K) | m)` (Eq. (3.54)), `ϑ₁` the first Jacobi theta function,
  evaluated by its `q`-series in real arithmetic (Eqs. (4.3)–(4.4)), `A₁` by Eq. (4.5), `A₂` by a second series
  (Eqs. (4.8)–(4.9)). §4.2 gives the whole algorithm as pseudo-code, an initialisation (pre-computes `m`, `K`, `K′`,
  `q`, `η`, `A₁`, `A₂`, the series coefficients) and an evaluation at any `t` (pp. 159–161). Accuracy claimed:
  "rarely needs more than three or four terms to get to machine precision" (p. 158), "up to machine precision, the
  results in Fig. 3 are exact" (§5). **Validation in the paper: one example only** (§5, Fig. 3: six unit point
  masses at ±3, ±2, ±1 on the axes, `I = (10, 20, 26)`, `ω(0) = (1, 15, 1)`), shown as plots with no error numbers,
  no comparison against an integrator, no cost figure, and **nothing on the separatrix** `2E = L²/I₂`, where
  `m → 1`, `K → ∞` and `q → 1`. **B10** (`b10_exact_rotor.py`, my port of §4.2 using SciPy's `ellipj`, `ellipkinc`,
  `ellipk`; criteria written first): on B2's case (`m = 0.998`) it agrees with B2's RK4 reference to **2.9e-9 rad** at
  20 s (that reference's floor) and with the Yoshida splitting to **3.5e-11** (h = 1/240), **1.6e-12** (1/960) and
  6.0e-12 rad (1/1920, rounding); over 10⁴ s `|L|` and `E` hold to 1.9e-14 and 3.8e-14 and `‖AAᵀ − 1‖` to 2.7e-14;
  the residual of `Ȧ + W(ω̃)A` by central difference is 1e-10. The paper's term count `N_T = log ε/log q` is very
  conservative (33 terms at `q = 0.33`; `q^{n(n+1)}` needs about five). **Towards the separatrix** (relative
  distance δ of `2E` from `L²/I₂`, 20 s, against Yoshida at h = 1/1920, which agrees with h = 1/3840 to ≤ 4e-11
  throughout): 3e-12 rad at δ = 1e-2, 4.5e-12 at 1e-4, 4.7e-11 at 1e-6, **1.5e-8 at 1e-8, 2.1e-8 at 1e-10 and
  2.2e-5 at 1e-12**, where the exact form's own ODE residual has grown to 1.3e-3; so the error is the exact form's
  (cancellation in `L² − 2EI₁` and in the theta arguments as `K` grows), not the splitting's. Cost: ~50–80 µs per
  evaluation in Python/NumPy, not indicative of Rust. **Decision:** unchanged for the implementation, the
  splitting stays (it is portable and the exact form needs a dozen platform transcendentals, see the next item);
  the exact form joins the S7 tests as the asymmetric-top oracle, used only for δ ≥ 1e-6 (where B10 puts it
  within 5e-11 rad of a converged splitting). The Celledoni & Säfström 2006 integrator, which "uses the exact
  solution of the Euler equations but uses an approximate expression for the attitude matrix" (p. 161, their
  Ref. [12]), is still unread.
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
  Any series is bit-identical on every IEEE target. **The trig-free alternative is the paper's own: Dullweber et
  al. App. A replace `Rₓ(φ)` by the Cayley form `cos φ → (1 − φ²/4)/(1 + φ²/4)`, `sin φ → φ/(1 + φ²/4)`, which is
  orthogonal in exact arithmetic and needs only `+ − × ÷`. B9:**

  | integrator (same case as B2) | h | max \|ΔE/E\| | \|Δ**L**\|/\|L\| | attitude error at 20 s | `‖RᵀR − I‖` |
  |---|---|---|---|---|---|
  | Strang, trig | 1/60 | 1.9e-6 | 4.2e-15 | 2.6e-5 rad | 1.1e-14 |
  | Strang, Cayley | 1/60 | 1.9e-6 | 5.4e-15 | 1.1e-4 rad | 1.2e-14 |
  | Strang, trig | 1/240 | 1.2e-7 | 1.6e-14 | 1.6e-6 rad | 3.3e-14 |
  | Strang, Cayley | 1/240 | 1.2e-7 | 1.2e-13 | 6.6e-6 rad | 1.7e-13 |
  | Strang, trig | 1/960 | 7.6e-9 | 4.7e-14 | 1.0e-7 rad | 2.8e-14 |
  | Strang, Cayley | 1/960 | 7.5e-9 | 2.5e-13 | 4.1e-7 rad | 3.3e-13 |
  | Yoshida 4th order, trig | 1/240 | 2.4e-12 | not measured | 2.9e-9 rad | not measured |
  | Yoshida 4th order, Cayley | 1/240 | 3.2e-12 | not measured | 2.9e-9 rad | not measured |

  The Cayley angle is `2 atan(φ/2) = φ − φ³/12`, a relative frequency error of `φ²/12`: same order, a factor of
  about 4 in attitude at second order, nothing visible at fourth order (both are at the reference's 3×10⁻⁹ rad
  floor), `|L|` and orthogonality an order of magnitude worse but at 1e-13 over 4,800 steps. So the trig series is
  optional: **Cayley with the fourth-order Yoshida composition is the simplest portable choice** (no series to
  validate, no `sin`/`cos`); the series with Strang is the one with the smaller second-order error.

### 1.2 Contact: impulses, positions, substeps

Candidates and what the primary sources say:

| family | what it is | evidence |
|---|---|---|
| Sequential impulses / PGS with Baumgarte | velocity-level Gauss–Seidel, warm started, position error fed back as velocity bias | [Catto 2006](https://box2d.org/files/ErinCatto_SequentialImpulses_GDC2006.pdf): "accept penetration, remember the past, apply impulses early and often, pursue the true impulse, update position last". [Solver2D](https://box2d.org/posts/2024/02/solver2d/): Baumgarte "can lead to energy creation and jitter if not tuned carefully". |
| Soft step (TGS_Soft) | substeps, one sweep each, soft constraints, warm start, relax sweep | Catto's Box2D v3 choice: "smaller sub-steps beat extra iterations because they reduce non-linear effects"; relaxation "is expensive, but it improves the simulation quality dramatically" ([Solver2D](https://box2d.org/posts/2024/02/solver2d/), via fetch summary). |
| PBD / XPBD | positions projected, velocity = Δx/h | [PBD 2007](https://doi.org/10.1016/j.jvcir.2007.01.005); [XPBD 2016](https://doi.org/10.1145/2994258.2994272) (compliance makes stiffness step- and iteration-independent); [Small steps 2019](https://doi.org/10.1145/3309486.3340247): "a single large time step with n iterations is less effective than n smaller time steps, each with a single iteration"; their capsule stack under a heavy mass "collapses" with 1 substep × 100 iterations and stays stable with 100 × 1 (Fig. 9); [Müller 2020](https://doi.org/10.1111/cgf.14105): 20 substeps × 1 iteration for every example in Table 1, and "the best choice is to replace all solver iterations by substeps". |
| Parallel / Jacobi with mass splitting | each body's mass divided by its contact count *in the effective mass only* (Eq. 15), full mass to apply the impulse; blocks by body pair solved by PGS inside, Jacobi between (Eq. 16, §4.1) | [Tonge et al. 2012](https://doi.org/10.1145/2185520.2185601), *ACM Trans. Graph.* 31(4), 105 (read). Purpose: a parallel solve whose residual does not depend on contact order, so near-rest bodies do not jitter at low iteration counts (abstract, §1). Convergence proved by showing the method is PGS on a *split system* (each body cut into one sub-body per contact block, mass shared equally, fixed joints rejoining them whose exact solve is a velocity average; §5, Fig. 4, Eq. 38); the scalar-method proof, the exact fixed-joint solve and the proof that the split system's solution solves the original LCP are in the [supplement](https://doi.org/10.1145/2185520.2185601) (read 28 Nov; §9 7b: all three rest on Murty's 1988 iterative-LCP conditions applied to `JM⁻¹Jᵀ`, so they cover the frictionless normal problem only and prove convergence, not a rate, with nothing on mass ratios). Tests (§7, Table 1, Fig. 6): 5,160 boxes on a mesh (15.7 ms/frame, 64 fps, GTX 580, against 17.7 ms for parallel PGS), 25-link chain, 1,000 chess pieces, card house (48 cards in five levels, §7; 58 bodies in Table 1), 277-piece fracture (665 contacts, greedy colouring needs 15 colours, Fig. 3); jitter judged by kinetic energy with 30 mass-split iterations against 5 PGS iterations. **Its own limitation (§7):** "convergence of our method is worse than PGS in all the tests", showing as extra contact compliance; Jacobi with line search converges better beyond ~50 iterations. No mass-ratio or stack-penetration measurement. |
| Non-smooth Newton, NCP | Newton on a minimum-map or Fischer–Burmeister form of the complementarity conditions; each Newton step is a symmetric linear system | [Macklin et al. 2019](https://doi.org/10.1145/3338695), *ACM Trans. Graph.* 38(5), 140 (read in part). Smooth isotropic Coulomb friction as NCP functions (§4.3), a complementarity preconditioner scaled by step and effective mass (§7), line search or damped Newton (§8.1, damped used for frictional runs), preconditioned conjugate *residual* for the linear solves (§8.2: monotone residual, usable with early termination). Table 2: 4–8 Newton iterations per step (40 PCR iterations per Newton iteration in the beam test, §10.2; 40 linear iterations in §10.4), `h = 0.0083 s`, **fp32 on a GTX 1070**, zero initial iterate and no warm start. **Stacks with extreme mass ratios**: a 4,096:1 stack (8 … 32,768 kg, μ = 0.5) is stable and does not interpenetrate with Krylov or direct solves, while relaxation methods "struggle to reduce error" and interpenetrate significantly on it (§10.1.5, Fig. 14 middle), and "eventually collapse even with hundreds of iterations" on the arch (Fig. 14 top); Jacobi and Gauss–Seidel stagnate on ill-conditioned cases (§10.4, Fig. 18). Its fp32 accuracy floor follows Tisseur's bound (Eq. 91, §10.5), so `f64` would lower it. Heavier per step than one soft-step sweep; the only family in the table with a demonstrated answer at 4,096:1. |
| Direct LCP solves | pivoting (Lemke, Keller) or incremental pivoting on the island's LCP | Bender, Erleben, Trinkle, Coumans 2012 STAR §4.2 ([10.2312/conf/EG2012/stars/095-134](https://doi.org/10.2312/conf/EG2012/stars/095-134)): "computational heavy … Therefore … not preferred for interactive simulation. However, their ability to deliver accurate solutions make them ideal to handle problems such as large mass ratios. Thus, for some applications direct methods are the only option." The 2014 CGF version ([10.1111/cgf.12272](https://doi.org/10.1111/cgf.12272)) §4.1 says the same; its §4.2 says PGS "is very robust … can deal gracefully with ill-conditioned problems (due to many redundant constraints)". |
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

**B8 (added after reading Tonge 2012 and Macklin 2019): the same stacks with the remedies the sources name.**
Same 1D model and conventions as B3 (script `b8_massratio.py`, output `b8.out`). A is B3's PGS. **E** is Tonge's
scalar mass splitting (Eq. 15): one substep, K Jacobi sweeps, effective mass built from `m/nᵢ`, full mass to apply
impulses, same Baumgarte as A. **D** is a hard-contact velocity-level LCP solved *exactly* by active set and a
dense solve every substep (standing in for a direct or converged Krylov solve), Baumgarte bias `β/h`, K substeps.
Worst interpenetration in mm (|v| over the last second in m/s) after 6 s:

| scene | A, K=20 | A, K=1000 | E (mass split), K=20 | E, K=1000 | D (exact), K=4 | D, K=20 |
|---|---|---|---|---|---|---|
| 10 equal boxes | 1.14 | 0.000 | 6.43 | 0.001 | 0.000 | 0.000 |
| 1000 kg on five 1 kg boxes | 319 (16) | 3.70 | 1067 (42) | 74.3 (6.3) | 0.000 (4e-15) | 0.000 |
| five 1000 kg boxes under 1 kg | 0.19 | 0.000 | 2.17 | 0.000 | 0.000 | 0.000 |
| 8, 64, 512, 4096, 32768 kg (Macklin Fig. 14) | 1409 (15) | 15.1 | 2721 (44) | 209 (9.9) | 0.000 | 0.000 |

(The same 1000 kg stack dropped from 2 mm gaps: A 324 mm at K=20, 4.6 mm at K=1000; D 0.000 mm and 9e-16 m/s at
K=20.) Reading: (a) mass splitting is *not* a fix for the heavy stack at any iteration count tried, matching Tonge's
own §7 admission that its convergence is slower than PGS: it buys order independence, not accuracy; (b) a relaxation
method needs ~1000 sweeps to hold a 1000:1 stack to millimetres, an exact solve needs one linear solve per substep;
(c) so the B3 "B (stiff)" sag of 9.8 mm at 1000:1 is **compliance** (the soft contact's stiffness scales with the
pair's effective mass, not with the load above it: the penetration grew 196× between the equal-box and heavy cases
while load per unit effective mass grew 100×, same order [D, a consistency check, not a derivation]), and no
sweep count removes it; (d) D is 1D and frictionless: it shows the conditioning problem is solvable by an exact solve,
not that a 3D friction-cone solve is as easy (Macklin 2019 needs Newton and a preconditioner for that; B8 does not
test it).
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

- **Point shell against voxel map** ([McNeely, Puterbaugh, Troy 1999](https://doi.org/10.1145/311535.311600),
  *SIGGRAPH 99*, pp. 401–408; read in full from the owner's copy; the 2006 follow-up
  [Voxel-Based 6-DOF Haptic Rendering Improvements](https://digital.lib.washington.edu/researchworks/bitstreams/54910b02-279c-481b-878b-20e30327a227/download)
  was read earlier). What the 1999 text says, section by section:
  * *Representation* (§3, §4.1): the dynamic object is a **point shell**, the centres of its surface voxels with
    precomputed inward normals; the environment is a **voxmap** with a **2-bit type per voxel**: free, interior,
    surface, proximity (Table 1, Fig. 4). **There is no distance field**; the first draft's "voxmap with a distance
    field" came from the 2006 paper.
  * *Force* (§3, Fig. 2): when a point enters a surface voxel, depth `d` is its distance below a *tangent plane*
    through the voxel centre with the point's own normal; force `K_ff d` along that normal (a "half-voxel-deep force
    field"); the net is the sum. Direction comes from the precomputed normal, which is why it is fast.
    Magnitude is discontinuous as a point crosses a voxel boundary (§3).
  * *No exact-surface contact by design* (§4.2, Fig. 5): the force layer is offset **two voxel layers outward** so
    exact surfaces never interpenetrate; the price is a minimum separation of about a voxel ("voxel-scale accuracy",
    §1; a 0.5-inch clearance rule is their justification). Resting contact therefore sits a voxel or two off the
    surface. **For Cavendish, whose rest must be at the mass surface, that is the wrong trade.**
  * *Structure* (§4.3, Fig. 6): a `2^{3N}`-tree limited to **three levels** above the leaves, because tree
    traversal time varies and they want a constant refresh rate (§2 criticises hierarchies for "indeterminacy in
    the cycle rate"); memory was minimal at `N = 3`, a "512-tree", for their geometry.
  * *Scene* (§4.4, §7): **one dynamic object against all static objects merged into one voxmap**, each voxelised in
    its final world pose (§4.1); "limited to the case of a single dynamic rigid object". Cost is linear in the
    shell's point count (teapot 380 points at 5 mm, 600 at most on the two-CPU machine, §6.3).
  * *Stability* (§5): penalty forces are averaged over the number of simultaneous point-voxel hits once there are 10
    or more (`F_net = F_total/(N/10)`, §5.2), a virtual spring–damper coupling to the user (§5.1), and a
    pre-contact braking force in the proximity layer (§5.3); tunnelling is named as a weakness ("a point may skip over the proximity voxel in a single haptic cycle") and
    they suggest limiting every point's speed to `s/Δt` (§5.3). Without braking 70 of 69,000 contact frames registered a possible exact-surface penetration
    (`10⁻³`), with braking 6 of 108,000 (`6×10⁻⁵`; Table 2).
  * *From the 2006 follow-up (read earlier, not re-read now):* distance fields, temporal coherence and culling of
    redundant surface contacts together give an order-of-magnitude accuracy gain; surface-point counts grow as the
    inverse square of the voxel size ("easily exceed a million points"), with only about 2000 point-voxel tests per
    haptic frame on the hardware of the time. The lesson the first draft drew stands: per-surface-sample contact
    **needs reduction and hierarchy**.
  *Does it fit octree-against-octree contact?* **Not as published.** (1) It is one-sided and one-to-many: shell
  against a world map, not two bodies that both move and both carry mass. Probing could be made
  symmetric (my inference: transform one body's points into the other's frame), but the merged static map, the
  world-pose voxelisation and the fixed-depth tree are what make it fast, and none carries over to a moving, edited,
  adaptively deep octree. (2) A penalty force is not a velocity-level impulse; the contact points would be right
  and the model wrong for a solver that needs `λ ≥ 0` constraints and warm starting. (3) The two-layer offset
  contradicts exact rest. What *does* transfer: the tunnelling bound `|v_point| ≤ s/Δt` (it is §2.7's rule,
  now sourced), the idea of normalising by the number of simultaneous hits (the same instinct as Tonge's `1/nᵢ`
  mass division and as contact reduction to four points), and probing one body's boundary points into the other's
  tree as a *cheap first stage* before leaf-pair descent. The 1999 text does **not** contain the "culling of redundant
  surface contacts" the first draft credited it with (that is the 2006 follow-up). Recommendation unchanged:
  dual-tree descent below.
- **Signed distance fields with local optimisation** ([Macklin et al. 2020](https://doi.org/10.1145/3384538),
  *Proc. ACM Comput. Graph. Interact. Tech.* 3(1), Art. 8; read §1–2, §6–10, headings of §3–5). Contacts are
  generated from **mesh elements** (vertices, faces, edges) against an SDF isosurface: each face is minimised over
  its barycentric coordinates to the closest point on the surface, by projected gradient descent (§3.1),
  Frank–Wolfe (§3.2) or, for edges, golden-section search (§4), with sphere culling and a fixed 32 iterations in
  Table 1. Reported: 0.4 ms per step for rigid shells of 129k triangles each (Fig. 7, §8.2) and about 15 ms for
  surface-to-surface mesh collision on the same scene against under 0.5 ms (§8.4), on a GTX 2080 Ti; 64 low-
  discrepancy samples per face could not remove interpenetration at sharp features, while optimised contacts
  did (§8.3). Limits it states: contacts are local (no global minimum translation distance, deep penetrations
  resolve "within a few time-steps"), **one contact per element** so an element resting on several minima can
  interpenetrate (§9), a discrete SDF needs "a sufficient margin" between the shape and the volume's extent (§6),
  and the next step the authors name is "contact generation between two solids both represented purely as SDFs,
  without the requirement for any mesh-based surface representation" (§10): **exactly our case is open ground in
  this paper.** What it offers Cavendish: a way to get *sub-cell depth and smooth normals* for a voxel body against
  an SDF (a distance transform of the other body's octree), with the boundary faces of one body playing the "mesh"
  (the exact-skin pieces are such a surface). Not needed for blocky contact; relevant to the rolling-sphere decision
  (item 5 of the pipeline below), which stays unmeasured.
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
   keeps four; I am recalling it, not citing it). McNeely 1999 has the same instinct in another form (it averages
   the penalty force over the number of simultaneous hits above ten, §5.2, and the 2006 follow-up culls redundant
   surface contacts), and Tonge's `1/nᵢ` mass division is the solver-side version. For
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
  another across a face). Union-find is near-linear *only with both rules*:
  [Tarjan 1975](https://doi.org/10.1145/321879.321884) (*J. ACM* 22(2), 215–225; abstract, introduction and
  theorems read) shows that path compression alone costs `O(m log(n²/m)/log(2m/n))` (Theorem 4, Eq. 3: `O(m log n)`
  at `m = n`), while union by weight *plus* compression costs `Θ(m α(m, n))` with `α` an inverse Ackermann function
  (abstract; upper bound Theorem 13, matching lower bound Theorem 16 and Corollary 17), "not linear" in the title
  and conjectured to be optimal (Conclusion). The first draft's "path compression is near-linear" was loose;
  with union by weight `α ≤ 4` for any `n` we will meet. Box2D uses depth-first search over
  islands ([source above](https://box2d.org/posts/2023/10/simulation-islands/)); Gustafsson's 2014 post used
  repeated min-id propagation over a few dozen convex shapes
  ([post](https://blog.voxagon.se/2014/05/13/cracking-destruction.html)). For octree leaves, find neighbours by Morton arithmetic at
  the leaf's level and the two adjacent levels. Only carved bodies need relabelling; carve-free bodies are
  never re-examined. Fracture by *volumetric approximate convex decomposition*
  ([Müller, Chentanez, Kim 2013](https://doi.org/10.1145/2461912.2461934), *ACM Trans. Graph.* 32(4), 115; read
  §1–3, §5–6) is a different, shape-based approach, and what it says is limited: (a) fracture is **geometric**,
  fracture patterns (decompositions of space into convex cells) are aligned to the impact and intersected with
  convex pieces; there is no stress model and "structures do not fracture under their own weight" yet (§6);
  (b) it **needs island detection** after every fracture and calls it "one of our core contributions": a flood fill
  over pieces whose neighbours are found by sorting all faces by `|d|` of their plane equation and testing pairs
  with equal values and opposite normals (§3.1, step 5) — for octree leaves Morton-key arithmetic is the better
  neighbour finder, so the *step*, not the method, carries over; (c) timings: typically under 10 ms, always under 50
  ms for the 20,000-piece arena on an i7 and a GTX 680 (§5, Fig. 14); (d) its rigid solver is Tonge et al. 2012
  (§5), and "to stabilize the solver, we do not create true rigid bodies below a certain size": pieces below a
  threshold become debris particles (spheres with high angular damping). That last rule is worth copying for the
  `million-bodies` mode and for S7's smallest fragments (below some leaf count, hand the piece to a particle path
  rather than a rigid body). No validation against a known answer, and momentum conservation at fracture is not
  discussed; §3's velocity rule is ours.
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
| torque-free asymmetric top | |L| constant, E constant, Euler equations; attitude against the exact solution of [van Zon & Schofield 2007](https://doi.org/10.1016/j.jcp.2006.11.019) (Eqs. (3.33)–(3.54), §4.2; B10) | `free_rotation_conserves_l_and_e`, `free_rotation_matches_exact_top` | |L| 1e-13 relative, E 1e-6 at h = 1/240 (B2: 1.2e-7) or 1e-10 with 4th order; attitude ≤ 1e-9 rad at 20 s for 4th order at h = 1/240 (B10: 3.5e-11), cases kept to δ ≥ 1e-6 from the separatrix. **Control:** explicit Euler (B2: 2.5e-3; attitude 2.7e-2 rad) |
| bounce | apex after k impacts `e^{2k} h₀`; impact time `t_k = t₀(1+2e(1−e^{k−1})/(1−e))` | `bounce_height_follows_restitution` | time 1e-12 s with TOI (B4: 1e-15 s). **Control:** detect-and-project (B4: 36 ms late at 1/240 s) |
| two-body collision | `v₁′ = ((m₁−e m₂)v₁ + (1+e)m₂v₂)/(m₁+m₂)`; momentum conserved exactly; for e = 1, energy | `two_spheres_exchange_momentum` | 1e-12. Control: wrong mass ratio |
| Newton's cradle | sequential collisions of three equal balls, e = 1: `(0, 0, 1)`; simultaneous restitution gives `(−⅓, ⅔, ⅔)` (B7) | `cradle_with_gaps_passes_the_impulse` | balls with gaps ≥ `v h` so collisions are sequential; **control:** touching balls must give `(−⅓,⅔,⅔)`, documenting the model. I could not find a primary source for the cradle as a *benchmark*; the numbers above are my derivation, checked by B7 |
| sliding with friction | stop distance `v₀²/(2μg)`, stop time `v₀/(μg)` | `sliding_box_stops_where_coulomb_says` | 1e-5 m at 1/240 s (B5: 7.9e-6). Control: μ = 0 never stops |
| incline | slides when `tan θ > μ`, `a = g(sin θ − μ cos θ)`; sticks and sleeps when `tan θ < μ` | `incline_threshold_is_at_atan_mu` | **tilt gravity, not the floor** (stairs); bracket θ = atan μ ± 1% |
| rolling | solid sphere `a = (5/7) g sin θ` | `rolling_sphere_on_incline` | **measures the voxel-normal question of §2.5**; criterion set before measuring |
| stack at rest | every pose constant after sleep; penetration ≤ stated | `stack_of_ten_rests_and_sleeps` | B3's 1D numbers as the model's expectation: 0.05–0.1 mm at K = 20. Control: K = 1 must fail |
| heavy-on-light stack | none closed form; at rest every contact carries the weight above it, so the rest state is known: zero penetration, zero velocity, contact impulses equal to the weight times `h` | `mass_ratio_1000_stays_up` | soft-step path: document the limit (B3: 10 to 14 mm at K = 20). **Exact-solve path: zero penetration at 1000:1 and 4096:1 (B8, 1D); the 3D, friction version is the test to write.** Control: the PGS path at K = 20 (B8: 319 mm) must fail it |
| gyroscopic terms | rattleback and tippe top reverse or flip as the physics says (Bender et al. 2012 §7.3, Fig. 18); our own closed forms are `|L|` and `E` of the free asymmetric top (B2) | `free_rotation_conserves_l_and_e` plus a rattleback *qualitative* case | the survey gives no numbers; a rattleback needs a curved-bottom body, so it is a voxel-normal test as well as an integrator test |
| dense structured stack, card house | static friction holds the structure (Bender et al. 2012 §7.3: "static structures like the card house or dry stone masonry … excellent for testing … static friction") | `card_house_stands` (later) | qualitative; Tonge's 48-card house (§7, Fig. 6; 58 bodies in Table 1) is the published instance |
| momentum, angular momentum, energy | ΣP, ΣL conserved in any free collision; E non-increasing | `dynamics_conserve_momentum` | 1e-12 relative |
| sleep | pose bits equal over 10 s | `slept_pose_is_bit_constant` | control: sleeping disabled, bits differ |
| split | Σ N exact, ΣP, ΣL, ΣKE 1e-12 | `split_conserves_exactly` | control: perturb one child |
| determinism | hash of poses, native = wasm; thread count 1 = 12 | `dynamics_hash_matches_across_targets` | control: one `mul_add` in the solver, hash must change (this also answers whether the compiler contracts) |

**Published stack benchmarks.** *There are none with numbers, and the survey says so.* Bender, Erleben,
Trinkle, Coumans 2012 ([STAR, §7.3 "Benchmarking and Validation of Simulators", pp. 127–129](https://doi.org/10.2312/conf/EG2012/stars/095-134);
the 2014 CGF version, [10.1111/cgf.12272](https://doi.org/10.1111/cgf.12272), drops this section) write: "Few
examples of benchmark tests and simulator comparison tests exist … These limit themselves to case-by-case studies
comparing simulated results against analytical results or by feature list comparisons. **No benchmark databases
exist with both simulation setups and ground truth data.**" and "computer graphics rarely perform temporal
convergence studies". What it lists instead are *scene types*: towers and card houses for stacking accuracy ("dense
structured stacks are very good for verifying the accuracy of contact force computations", citing Erleben 2007),
a block on an inclined plane for the friction law, dominoes, the Newton's cradle ("to determine whether a
simulator's underlying model is a simultaneous contact model or a sequential contact model", Fig. 16: B7 found
exactly that split), a see-saw, a brick wall for sleeping that is too aggressive (Fig. 17), "a heavy box placed on
top of a light box [SNE09]" for large mass ratios ("most iterative methods … converge so badly that even infinite
amount of iterations won't help"), rattleback and tippe top for gyroscopic terms (Fig. 18), plus energy,
momentum and constraint-error plots, and "compare against a noninteractive but very high-fidelity simulator".
SNE09 is Silcowitz, Niebe, Erleben, *Nonsmooth Newton method for Fischer function reformulation of contact force
problems for interactive rigid body simulation*, VRIPHYS 2009, and "dense structured stacks" cites Erleben,
*Velocity-based shock propagation for multibody dynamics animation*, ACM Trans. Graph. 26(2), 2007. **SNE09 was
read on 28 Nov (third batch)**, [Silcowitz, Niebe, Erleben 2009](https://doi.org/10.2312/PE/vriphys/vriphys09/105-114),
VRIPHYS 2009, pp. 105–114 (10 pp.). Method: the velocity-level contact problem with a friction box whose bounds
are `±μλₙ` (Coulomb's law per tangent direction, Eqs. (12)–(19)), written as roots of the Fischer–Burmeister
function `φ(a, b) = √(a² + b²) − a − b` (Eq. (21)) in a bounded form with variable bounds (Eqs. (23)–(29)) and
solved by a generalised (nonsmooth) Newton method, `H_k Δλ = −Φ(λ_k)` with `H_k` from the B-subdifferential
(Eqs. (30)–(32)), quadratic near a solution by Qi and Sun's theorem (Thm 3.1); the Newton system is solved
approximately by a finite-difference nonlinear CG, globalised by an Armijo line search on `½‖Φ‖²`
(Eqs. (35)–(39)), and warm-started "semi-staggered": normal forces first with friction off, then the full problem
(§3, Fig. 4). Its tests are **the three box scenes**, judged by eye and by the merit function (§4, Fig. 5):
a heavy box on a light box on a floor, a sparse stack of equal boxes, and a dense structured stack. Results:
quadratic convergence to ~1e-25 in 7 iterations on a 6 × 6 problem in 3.3 ms (Fig. 2, §3); on the heavy-on-light
case the merit function falls "several orders of magnitude" further than PGS at equal cost (Fig. 8(a), costs in
PGS-iteration units), and PGS "fails to support the upper heavy box" "even with a 150 iterations per frame"
while FN with 10 Newton iterations holds it (Fig. 10); on the sparse stack PGS almost catches up given enough
iterations (Fig. 8(b)); on the dense stack both stall at local minima and FN's is *less* accurate (Fig. 8(c)); a
~280-contact scene runs at 6 ms per frame in Java on one 2.1 GHz core (Fig. 9). **No mass ratio is given as a
number** anywhere in the paper, and no penetration is measured. Its own limits (§4.1, §5): `A_ncp` has ~60 % zero
eigenvalues in all three scenes, so solutions are non-unique; the method "proved to have convergence problems"
with large friction forces, attributed to over-determinacy; damping cures it only by damping the solution.
*For this note:* it is the published heavy-on-light case Bender cites, and it supports §7's rule that such
islands need a solve that converges (Newton or direct), not more PGS sweeps; it also warns that the frictional
NCP is where a Newton method breaks, which is exactly the open item §9 7 leaves to S7's contact task. **Erleben 2007 was read on 28 Nov** (ACM TOG 26(2), Article 12, 20 pp., doi:10.1145/1243980.1243986): its
"dense structured stacks" are *scenes*, scored by eye ("an observer would immediately notice any misalignment",
§1) and by wall-clock time (Fig. 27), with no penetration or force numbers: a 25-box stack on which plain PGS "not
even 100,000 iterations will work" (§4, Fig. 9, citing Erleben's 2005 thesis), a 200-brick wall, a 320-brick tower,
a 125-ball grid, and the comparison scene, a 40-layer, 800-object tower hit by a heavy block and a cannonball,
against Maya 6.5, Guendelman's impulse-based shock propagation, Novodex 2.1.2 and ODE 0.30 (§5). The method is
shock propagation on velocities: bodies get a stack height by breadth-first search from fixed bodies, and layers
are solved bottom-up with the lower bodies of each layer *fixed* (infinite mass), ≤ 10 PGS iterations for dynamics
and 5 for projection error correction (§4, Fig. 12); a weight `0 < a ≤ 0.01` mixes ordinary dynamics back in
because with `a = 0` "bodies … cannot feel the weight of each other" (§4, Fig. 14). It is a mass-ratio *workaround*
that is wrong by construction for loads that should propagate upwards, so it does not change §7's choice of an
exact island solve. On mass ratios (§3.2): diagonal scaling "lowers the convergence rate by a constant but does
not change the asymptotic behavior", and damping `A + εI` (ODE's constraint-force mixing) speeds convergence by
weakening the forces (Eq. (41)–(44), Figs. 6–7); Jacobi "had terrible convergence" and SOR "suffered from energy
gain" (§3, opening). The first draft's "PDF copy … corrupt" is resolved: both
files are now read. The practical, published *scenes* are: Catto's large
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
   `h²`; at K = 20 expect ≈ 0.05–0.3 mm for ten boxes (B3), larger for heavy tops (compliance: it scales with the
   load per unit effective mass, B8). **Heavy-on-light islands take the exact path:** an island whose largest
   mass ratio across a contact exceeds a threshold (to be measured; start at 100:1), or whose relax-sweep residual
   does not fall, is solved by a direct factorisation of its contact system each substep instead of one Gauss–Seidel
   sweep. S7's islands are tens of bodies, so a dense or banded solve is microseconds to a millisecond; on the CPU
   in `f64` with a fixed elimination order it is deterministic. In 3D the friction cone makes it an NCP, not an
   LCP: the options the sources give are Newton on a Fischer–Burmeister form with a preconditioned Krylov or direct
   solve (Macklin 2019 §4.2, §7, §8.2) or a staggered normal/friction pair of exact solves (Bender et al. §4,
   "staggering"). Which one, and the mass-ratio trigger, are the measurement of S7's third task (B8's 3D
   counterpart, with a PGS control that must fail).
4. **Parallelism**: islands solved on a rayon pool, each writing its own slots, merged sorted. Single-island
   scenes (one big pile) stay serial in S7; Gustafsson's "parallel solver for big piles" is a later task. Two
   routes, now read: greedy contact colouring (at most `2Δ − 1` colours, `Δ` the largest contact count of a body, so
   cost multiplies by the colour count; Tonge §3.4.1) or mass splitting ([Tonge 2012](https://doi.org/10.1145/2185520.2185601)).
   Mass splitting is *order-independent by construction*, which suits the replay rule (§4) provided its
   per-body velocity average (Alg. 1 lines 22–27) is summed in a fixed order; but it converges slower than PGS
   (§7), so it is a determinism and parallelism tool, not an accuracy one, and not the answer to heavy stacks.
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
- **B8 `b8_massratio.py`** (output `b8.out`, ~40 s). Added after reading the primary sources. 1D stack as B3 with
  A (PGS + Baumgarte), E (Tonge mass-splitting Jacobi, scalar form, Eq. 15) and D (exact active-set LCP solve per
  substep, hard contacts, Baumgarte `β/h`): four scenes (10 equal boxes; 1000 kg on five 1 kg; five 1000 kg under 1
  kg; 8…32768 kg geometric), K = 4, 20, 100, 1000 for A and E, K = 4, 8, 20 for D, plus a 2 mm drop. Not tested:
  friction, rotation, 3D, the block version of mass splitting (one contact per body pair in 1D, so blocks are
  trivial), and any solver at a realistic frame time (D's cost is a dense solve of ≤ 10×10 here).
- **B9 `b9_cayley.py`** (output `b9.out`). B2's case and RK4 reference with the Cayley rotation of Dullweber et al.
  App. A in place of `sin`/`cos`; Strang at three step sizes and Yoshida at two. Same floor as B2 (3e-9 rad).
- **B10 `b10_exact_rotor.py`** (output `b10.out`, ~1 min). Added 28 Nov (third batch). A port of van Zon & Schofield
  2007 §4.2 (initialisation and evolution) with SciPy's elliptic functions; B2's case (`I = (1, 2, 3)`,
  `L̃(0) = (0.05, 2, 0.04)`, so `m = 0.998` and the axes are relabelled by Eq. (4.10)). Criteria written first:
  (a) ≤ 1e-8 rad against B2's RK4 reference at 20 s: **2.9e-9**, pass; (b) ≤ 1e-9 rad against Yoshida-4 at
  h = 1/1920: **6.0e-12**, pass (3.5e-11 at 1/240, 1.6e-12 at 1/960); (c) `|L|`, `E` drift ≤ 1e-13 over 10⁴ s:
  **1.9e-14, 3.8e-14**, pass, orthogonality 2.7e-14; (d) towards the separatrix, a measurement: within 5e-11 rad of
  Yoshida for δ ≥ 1e-6, then 1.5e-8 (δ = 1e-8), 2.1e-8 (1e-10), 2.2e-5 (1e-12); (e) Yoshida at 1/1920 and 1/3840
  agree to ≤ 4e-11 rad at δ = 1e-8 and 1e-12, while the exact form's `Ȧ + W(ω̃)A` residual grows from 1.7e-10 to
  1.3e-3, so the separatrix error is the exact form's. Also corrects B2's reading: its Yoshida row (2.9e-9 rad)
  was the RK4 reference's error; the splitting itself is at ~3e-11 rad at h = 1/240.

---

## 9. What I could not verify, and how each is settled

1. **Rust/LLVM never contracts `a*b+c`** (no primary source found). Settle: the cross-target hash test with a
   deliberate `mul_add` as control; check `RUSTFLAGS`/`target-cpu` in `xtask`.
2. **`libm` crate determinism** (docs.rs page gives no statement; v0.2.16). Moot if series are used.
3. **Teardown's voxel contact generation**: no primary text found. My pipeline (§2) is from S3's tree, Müller's
   island step and general practice, not from Teardown; McNeely 1999 and 2006 supply the point-probe idea, the
   tunnelling bound and the cautions only.
4. **Box2D's soft-contact constants and restitution code** (I read Catto's descriptions, not the source; the
   web fetches were summaries by a small model, so quoted phrases are as returned, not checked against the
   pages' exact text). Settle: read `solver.c`/`contact_solver.c` of Box2D v3 when implementing.
5. **Numeric stack benchmarks in the literature**: resolved, there are none (Bender et al. 2012 §7.3). Our own
   criterion stands in (§6).
6. **Sleep thresholds, rest-penetration tolerance, voxel-normal quality, tunnelling limit, performance at 10³
   bodies**: need measurement in S7. Write the criteria first (CLAUDE.md): e.g. sleep when |v| < 1e-4 m/s and
   |ω| < 1e-4 rad/s for 0.5 s *and* the island's residual from a relax sweep < threshold; measure solver noise at
   rest first.
7. **3D behaviour**: B3 and B8 are 1D. Friction cones, rotation in stacks, box–box manifolds are not covered by any
   number here. In particular **whether an exact island solve stays exact and cheap with a friction cone** (Macklin
   2019 does it with Newton, a preconditioner and a Krylov solve in fp32 on a GPU; no CPU `f64` numbers) is the
   open question the S7 contact task must settle, with a mass-ratio sweep and a PGS control.
7a. **The exact free-rotation formulas** (van Zon & Schofield 2007; the 2008 note cites but does not give them), and
   whether they are robust near the unstable intermediate axis. Settle by reading the two papers, then testing
   against B2's reference; if good, flights need no attitude integrator. Still unread (27 Nov): the file fetched as
   `vanzon2008.pdf` is the 2008 *J. Chem. Phys.* note again, whose reference list confirms Ref. 5 = *Phys. Rev. E*
   75, 056701 (2007) and Ref. 8 = *J. Comput. Phys.* 225, 145 (2007), titles not printed; Ref. 5 holds the
   propagator's forms (p. 136102-1, after Eq. (3)). **Narrowed 28 Nov**: the PRE paper is read (§1.1). It gives
   the composition `P(h) = R₁(L̃*) R₂(ψ) R₁ᵀ(L̃)` and a special-function-free ψ, so the remaining unknown is the
   body-frame `L̃(t)` of the free Euler equations in Jacobi elliptic functions (its Ref. 10, the JCP paper, and
   Ref. 8, Celledoni & Säfström 2006), plus the separatrix case `2E = I_y L²`, where the elliptic modulus goes to
   1 and the PRE says nothing. The ~10⁸ gain in attitude-error time over the 1,2,3,2,1 splitting at equal step
   (§V A) makes this worth finishing. `L̃(t)` is also textbook (Landau–Lifshitz §37), so B2's test can be built
   without the JCP paper if it stays missing; the JCP paper would add the numerically robust form.
   **Closed 28 Nov (third batch).** The JCP paper is read whole and ported (§1.1, B10). It gives `L̃(t)` in
   `cn`, `sn`, `dn` (Eqs. (3.33)–(3.42)) and the attitude in closed form through a theta function (Eq. (3.54)),
   with an algorithm (§4.2). It is accurate to the reference's floor on B2's case and to ~1e-11 rad against a
   converged splitting, and it has **no separatrix treatment**: in `f64` it degrades from δ ≈ 1e-8 (1.5e-8 rad at
   20 s) to 2.2e-5 rad at δ = 1e-12 (B10 (d), (e)). Settled: oracle, not implementation, and only for δ ≥ 1e-6.
   Not settled: whether a reformulation (e.g. evaluating `L² − 2EI₁` and the theta arguments with compensated
   sums, or switching to the separatrix limit, where the functions become `tanh` and `sech`) recovers accuracy
   near δ = 0. That matters only if the oracle has to cover a body set spinning about its middle axis, which a
   test can simply avoid. Celledoni & Säfström 2006 remains unread and is no longer needed for this item.
7b. **Tonge's supplement: read 28 Nov (third batch)**, Tonge, Benevolenski, Voroshilov, "Proofs for 'Mass
   Splitting for Jitter-Free Parallel Rigid Body Simulation'" (2 pp., NVIDIA; no DOI of its own, it supplements
   [10.1145/2185520.2185601](https://doi.org/10.1145/2185520.2185601)). It holds four results, all for the
   frictionless LCP: (§2) a **lemma**, `(Σᵢ vᵢ)·(Σᵢ vᵢ) ≤ n Σᵢ vᵢ·vᵢ` for `vᵢ ∈ ℝᵐ` (Eq. (1), by induction from
   `2xy ≤ x² + y²`, Eq. (2)), and the **theorem** that the scalar mass-splitting iteration
   `z^{r+1} = (z^r − E(q + Nz^r))₊` with `Eᵢᵢ = (Σⱼ Jᵢⱼ (n_j M_j⁻¹) Jᵢⱼᵀ)⁻¹` (Eq. (3), i.e. the paper's Eq. 15)
   converges. Proof: Murty 1988's conditions (Eqs. 9.8, 9.10 of his ch. 9) reduce to `2E⁻¹ − N` positive definite;
   it suffices that `E⁻¹ − N` is positive semi-definite, which follows by factoring each `Mᵢ⁻¹ = QᵢQᵢᵀ`, grouping
   the Jacobian blocks by body (`C(k)`, Eq. (5), `|C(k)| = n_k`) and applying the lemma term by term (Eqs. (6)–(9)).
   Assumptions stated: each `Mᵢ⁻¹` positive definite, each row touches two bodies, every split count `n > 0`
   (p. 1); a static body (zero inverse mass) is not discussed. (§3) **Convergence of the block method with exact
   joints**: Murty's iteration (9.9) with unit relaxation, `E = diag(D_CC⁻¹, N_FF⁻¹)` and the projection applied to
   the contact part `z_C` only (Eqs. (10)–(12)) is the paper's block iteration; it converges because
   `(z_P − z)ᵀE⁻¹(z_P − y) ≥ 0` for all `y ≥ 0` (Eqs. (13)–(14), Murty's Theorem 9.8). (§4) **The exact fixed-joint
   solve**: with `F = [S(n−1, 1)  −I(n−1)]` (Eq. (15)), `I − Fᵀ(FWFᵀ)⁻¹FW = n⁻¹S(n)` (Eq. (17), proof Eqs. (18)–(20)),
   so solving a body's joints sets every sub-body velocity to their mean (Eq. (21)), the paper's Eq. 38. (§5) **The
   split solution solves the original problem**: if `(z_C, z_F)` solves the split MLCP, then `z_C =
   LCP(JM⁻¹Jᵀ, q)` (Eq. (22)), because the impulse `z_C` applied to the unsplit bodies gives the first sub-body's
   velocity (Eqs. (24)–(26)) and those velocities satisfy the unsplit constraints (Eqs. (27)–(28)). **What it does
   not give**: a rate, any bound in terms of mass ratio or contact count, or anything on friction (the paper's
   friction is one staggered BLCP sweep, §6, and is not claimed to converge). **Effect on this note:** none on the
   decisions. It confirms mass splitting as a *convergent, order-independent* parallel scheme (§7 item 4) and
   leaves B8's measured slowness at large mass ratios, and Tonge's own "worse than PGS in all the tests" (§7),
   untouched. The previous text of this item, kept for the record:
   the paper states the results (§1.1, §5) and I have used only those.
   Re-checked 27 Nov against a second copy (`tonge2012.pdf`, again the 8-page main paper): what the main text
   *does* contain is the block method's argument, that Algorithm 1 is PGS on the split system with an exact
   fixed-joint (averaging) solve, PGS "unconditionally converges", so the block method converges (§5 opening,
   p. 105:4; §5.6, p. 105:6, despite its title "Implementation and Convergence Proof" this is the whole proof
   there). Deferred to the supplement, by the paper's own pointers: (i) the scalar method's convergence (after
   Eq. 15, p. 105:4: "We prove that our method converges in the supplement"; §5 says there is a "direct" proof for
   it); (ii) that solutions of the split system solve the original LCP (§1.1, p. 105:2; §5, p. 105:4; §5.6); (iii)
   the fixed-joint averaging, Eq. 38 (§5.5, p. 105:6); (iv) the velocity-iteration form, Eq. 34–37 (§5.4, p. 105:6).
   No friction convergence claim is made: friction is one mass-splitting sweep on the BLCP per normal sweep,
   staggered, not run to convergence (§6, Eq. 40–41).
8. **Order of accuracy of attitude interpolation for recorded tracks** (the v2 normalised-lerp claim is from
   reading `physics.md`, not from a measurement).

## 10. Further questions (not chased)

- Million-body mode: what is the right model (particles, granular/MPM, ballistic only) and what does the read
  need from it? Separate note before that demo.
- Structural collapse (support graph, §5.2 table in the plan) and the voxel-FE modal track the plan gives S7.
- Continuous collision for fast thin bodies; speculative margins sized by the read.
- Rolling resistance and contact-patch physics for voxel wheels; whether to smooth normals with an SDF.
- Articulated bodies/joints (v2 had none for rigid dynamics; skinned bodies are kinematic in S4).
- Exact Jacobi-elliptic free rotation vs high-order splitting for *very* long flights: measured by B10 (§9 7a);
  open only near the separatrix, where the published algorithm loses accuracy in `f64`.
- A distance transform of an octree body plus Macklin 2020's local optimisation, for sub-cell contact depth and
  smooth normals on a curved voxel body (the rolling-sphere decision); and SDF-against-SDF contact, which that
  paper names as future work.
- Cross-target determinism of the *fluid* and read kernels (separate from this note).

---

## Papers: read, and still unread

**Read from a primary copy, 25 Nov 2026 (owner's PDFs, not copied into the repository), with the extent read:**

- McNeely, Puterbaugh, Troy 1999, *Six degree-of-freedom haptic rendering using voxel sampling*, SIGGRAPH 99, pp. 401–408: all. https://doi.org/10.1145/311535.311600
- Tonge, Benevolenski, Voroshilov 2012, *Mass splitting for jitter-free parallel rigid body simulation*, ACM TOG 31(4), 105: all eight pages (not the supplement); a second copy fetched 27 Nov is the same main paper, §5–7 re-read from page images. https://doi.org/10.1145/2185520.2185601
- Macklin et al. 2019, *Non-smooth Newton methods for deformable multi-body dynamics*, ACM TOG 38(5), 140: abstract, §1–2, §8.1, §10 read; §3–9 by heading and keyword. https://doi.org/10.1145/3338695
- Macklin et al. 2020, *Local optimization for robust signed distance field collision*, PACMCGIT 3(1), 8: §1–2, §6–10; §3–5 by heading. https://doi.org/10.1145/3384538
- Müller, Chentanez, Kim 2013, *Real time dynamic fracture with volumetric approximate convex decompositions*, ACM TOG 32(4), 115: §1–3, §5–6 (the note's earlier author list, which added Macklin, was wrong: the PDF has three authors). https://doi.org/10.1145/2461912.2461934
- Dullweber, Leimkuhler, McLachlan 1997, *Symplectic splitting methods for rigid body molecular dynamics*, J. Chem. Phys. 107, 5840: §III, §IV figures, App. A–C. https://doi.org/10.1063/1.474310
- Van Zon, Omelyan, Schofield 2008, *Efficient algorithms for rigid body integration using optimized splitting methods and exact free rotational motion*, J. Chem. Phys. 128, 136102: both printed pages (136102-1–2; the PDF adds a publisher cover sheet), re-read 27 Nov from a second copy. https://doi.org/10.1063/1.2889937
- Lee, Leok, McClamroch 2007, *Lie group variational integrators for the full body problem*, CMAME 196, 2907: abstract, §1–2, §4.3, §4.4, §5, §6; the variational derivations (§3, §4.1–4.2) were not followed. https://doi.org/10.1016/j.cma.2007.01.017
- Hairer, Lubich, Wanner 2006, *Geometric numerical integration* (2nd ed.): §VII.5.3 pp. 284–285 (three-rotations and symmetric-plus-rotation splittings), Example II.4.2 (Triple Jump). https://doi.org/10.1007/3-540-30666-8
- Bender, Erleben, Trinkle, Coumans 2012 (STAR): §4.2 and §7.3 (pp. 127–129). https://doi.org/10.2312/conf/EG2012/stars/095-134 ; Bender, Erleben, Trinkle 2014 (CGF 33(1), 246–270): §4.1–4.2; its §7.3 equivalent does not exist (six sections). https://doi.org/10.1111/cgf.12272
- Tarjan 1975, *Efficiency of a good but not linear set union algorithm*, J. ACM 22(2), 215–225: abstract, introduction, Theorems 4, 13, 16, conclusion. https://doi.org/10.1145/321879.321884
- Read 28 Nov 2026 (owner's PDFs, not copied): van Zon, Schofield 2007, *Symplectic algorithms for simulations of rigid-body systems using the exact solution of free motion*, Phys. Rev. E 75, 056701: whole paper, Eq. (5)–(11) and §V A from page images. https://doi.org/10.1103/PhysRevE.75.056701 — Erleben 2007, *Velocity-based shock propagation for multibody dynamics animation*, ACM TOG 26(2), Article 12, 20 pp.: §1–5 read, §6 skimmed. https://doi.org/10.1145/1243980.1243986
- Read 28 Nov 2026, third batch (owner-supplied PDFs, not copied):
  van Zon, Schofield 2007, *Numerical implementation of the exact dynamics of free rigid bodies*, J. Comput. Phys. 225, 145–164: whole paper (§1–6, Eqs. (2.1)–(4.10), the §4.2 pseudo-code), ported as B10. https://doi.org/10.1016/j.jcp.2006.11.019 (printed on p. 145; Crossref confirms title, authors, volume 225 and pages 145–164, checked 28 Nov 2026). (The DOI given earlier from memory, 10.1016/j.jcp.2006.11.020, is Yue, Zhou, Feng 2007, the drop-shrinkage paper.)
  — Tonge, Benevolenski, Voroshilov 2012, *Proofs for "Mass Splitting for Jitter-Free Parallel Rigid Body Simulation"*, the 2-page supplement (NVIDIA): whole (§9 7b). No DOI of its own; the main paper's https://doi.org/10.1145/2185520.2185601 is Crossref-confirmed with the same three authors (the supplement prints the same three names).
  — Silcowitz, Niebe, Erleben 2009, *Nonsmooth Newton method for Fischer function reformulation of contact force problems for interactive rigid body simulation*, VRIPHYS 2009 (Eurographics), pp. 105–114: whole (§1–5, Figs. 2–10). https://doi.org/10.2312/PE/vriphys/vriphys09/105-114 (not in Crossref; found in DataCite with this title, the three authors and 2009, checked 28 Nov 2026; doi.org redirects to the Eurographics Digital Library).
- Earlier: Macklin et al. 2019 *Small steps* https://doi.org/10.1145/3309486.3340247; Müller et al. 2020 https://doi.org/10.1111/cgf.14105; Macklin et al. 2016 *XPBD* https://doi.org/10.1145/2994258.2994272; Müller et al. 2007 *PBD* https://doi.org/10.1016/j.jvcir.2007.01.005; Catto 2006 slides; McNeely et al. 2006 (follow-up); Rust and WebAssembly documentation. Blog posts by Catto and Gustafsson, the Gaffer article and the 80.lv article were read through a summarising fetch tool, so treat quotations as the tool's.

**Still unread, with what each would settle:**

- Celledoni, Säfström 2006, *Efficient time-symmetric simulation of torqued rigid bodies using Jacobi elliptic functions*, J. Phys. A: Math. Gen. 39, 5463–5478, https://doi.org/10.1088/0305-4470/39/19/S08 (title and pages from Ref. [12] of the JCP paper; DOI Crossref-confirmed 28 Nov 2026). Still missing, and now optional: §9 7a is closed without it; it would show a torqued integrator built on the exact Euler solution with an approximate attitude (JCP paper p. 161).
- Fassò 2003, *Comparison of splitting algorithms for the rigid body*, J. Comput. Phys. 189 (HLW's pointer; no DOI checked): which splitting of the kinetic energy is best.
- McNeely, Puterbaugh, Troy 2006 is the follow-up and was read; Harada 2007 (GPU Gems 3 ch. 29, no DOI) only through a fetch summary: https://developer.nvidia.com/gpugems/gpugems3/part-v-physics-simulation/chapter-29-real-time-rigid-body-simulation-gpus
