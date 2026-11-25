# Deformable solids: voxel finite elements for vibration, strain, thermal expansion and ground motion

Research note for the structural-dynamics track of S7 (`plan/VALIDATION-AND-RESEARCH.md` §4–5,
`plan/tasks.yaml` TASK-S7-05) and its users in S12 (seismic noise), S13 (thermal expansion, TASK-S13-06)
and S14 (earthquakes, TASK-S14-04/05). Written 25 Nov 2026.

**Tags.** **[R]** read in the primary source (section or equation given); **[A]** abstract or
bibliographic record only (in this version also: not fetched); **[M]** measured here (scripts in `elastic-solids/`, outputs beside them);
**[D]** derived here; **[U]** from memory, not checked: treat as a claim to verify.

---

## Changed after reading the primary sources (25 Nov 2026)

The owner fetched 82 of the papers on the first version's "Papers I could not read" list (about 380 MB; kept outside the
repository). I read the parts that bear on a claim the note makes or tagged [U], mostly as extracted text
and, wherever an equation mattered, from the rendered page (extracted PDF text loses Greek letters and square
roots, which is how the first version of §5.3 went wrong). Page, section and equation numbers are given
inline. **No recommendation reverses. Four change in substance** (the default element is bounded by Poisson's
ratio; the CPU oracle is an assembled sparse matrix, not matrix-free; the GPU precision pattern is f64 outer with
an f32 V-cycle; the memory budget is withdrawn pending a measurement), and **the shaft question is settled**.

**Settled.**
1. **The "factor ~1.5" between Harms Eq. 97 and Badurina App. B was my misreading, not a disagreement.** Two
   things combined. (a) The text extraction dropped the square root in `1 + √(q/s)` (Badurina Eqs. B.19, B.22
   as printed), so I compared `(1+q/s)` with `(1+ζ)`. (b) Harms Eq. 97 is the *potential*; Badurina's Eqs.
   B.21–B.22 are coefficients of the *phase*, i.e. of the z-derivative, which brings a factor `1/q` into the
   ratio. Read correctly, Harms Eq. 97, Badurina Eq. B.19 and Junca et al. Eqs. 11–14 are the same function:
   an independent numerical evaluation (surface mass sheet plus bulk `−ρ∇·ξ`, integrated layer by layer) agrees
   with Eq. 97 and with B.19 to 10⁻¹⁵, and Junca's differently written Eqs. 12–14 equal the horizontal
   derivative of Eq. 97 to 2×10⁻¹⁶ [M, `shaft_check.py`]. The vertical acceleration of Eq. 97 also jumps by
   exactly `4πGρξ_z` across the surface when joined to Eq. 96, as a mass sheet requires [M]. **The shaft
   formula is Harms Eq. 97 (cavity neglected); §5.3 now says so.**
2. **γ(ν) is right.** Harms Eq. 99 reproduces his Fig. 10 by eye (0.55 at ν = 0, 1 at ν = 0.5), equals Junca's
   Eq. 13 to rounding, and follows from Hughes & Thorne's Eq. 1.24 by the algebra in §5.2. The same Eq. 1.24
   evaluated on the half-space mode gives their published 0.86 (ν = 0.33) and 0.17 (subsurface term alone) [M].
3. **Harms's quote on kinematic simulation is exact** (§3.5, pp. 46–47: "not a single simulation of Newtonian noise
   has been carried out so far that could not have been done with a kinematic simulation") and dynamical
   simulation is "required to represent seismic fields in complex geologies and surface topographies".

**Corrected.**
- **Memory per element.** "~100 B per node" holds only for a hierarchy whose coarse operators are recomputed
  from the one element matrix. The GPU multigrid codes I could read store Galerkin stencils: Wu, Dick &
  Westermann 2016 §5 give **2.4 kB per element in double precision (2.3 GB per million elements)**, and fit
  14.2 million elements into a 6 GB card only by coarsening the finest level 4:1 and assembling its stencils
  on the fly. Their 621×488×1000 is the *grid*; the model had 14.19 million elements (Fig. 12). §1.4 and §8.
- **Matrix-free is not a free win at Q1.** Kronbichler & Kormann 2012 (§5.1, Fig. 4: a 3-D Laplacian, 1.8 million dof, CPU) find the cell-based
  operator beats a sparse matrix "for any polynomial order but one" (p = 1 is the exception); Ljungkvist 2017
  claims gains on GPUs from order two. What makes voxel Q1 matrix-free worthwhile is the single shared element matrix (Wu §5.1: on-the-fly
  assembly is about 2× faster than reading stored stencils), not matrix-free as such. Consequence: the CPU
  oracle should simply be an assembled sparse matrix. §1.4.
- **Precision.** Kronbichler & Ljungkvist 2019 §4.2: single precision throughout is *not* enough for a
  Krylov solve; the working pattern is an outer double-precision CG with a single-precision multigrid V-cycle
  (47–83 % faster than all-double, same accuracy; Poisson problems). My earlier "f32 operator with f64
  accumulation" is replaced by that. §8.
- **Saulson's β = 1/√3** is quoted by Hughes & Thorne (Eq. 1.9, citing Saulson's Eq. 21), not stated as β by
  Saulson; and Hughes & Thorne's `γ_J` (Eq. 1.18–1.19) is a two-test-mass correlation function, a different
  quantity from Harms's `γ(ν)`. §5.4.
- **Harms Eq. 39** (the Rayleigh displacement field): the first version said the PDF extraction had lost a minus
  sign on the horizontal component. The rendered page prints no such sign, and the field as printed is not
  traction-free; with the minus it is (§5.2). The closed forms are unaffected.
- Lamb 1904: the first file fetched was the three-page *abstract* in Proc. R. Soc. 72; the full Phil. Trans.
  A 203, 1–42 paper was fetched afterwards and is read (second fetch, below).
- Hughes 1980: B-bar and selective integration coincide for the undistorted cubes used here (his text; §1.1).

**Second fetch (Simo & Armero 1992; Lamb 1904, full; Kozar et al. 2018).**
- **My 9-mode brick is theirs.** Simo & Armero (§3.4, Eqs. 66–69) define Q1/E4 as Q1 with "4 extra modes (9
  modes in 3-D)", equal for linear isotropic elasticity to the QM6 element of Taylor et al.; for the brick
  `E(ξ) = diag[ξ₁ ξ₂ ξ₃]` (Eq. 69, from Wilson's functions `ξᵢ² − 1`), scaled by `det J₀/det J` with the
  Jacobian at the centroid. For cubes that scaling is 1, so `cantilever.py`'s element (modes `1 − ξᵢ²`, one
  per direction and displacement component, condensed) is this element. Their conditions, which my patch test
  checks: the enhanced and compatible gradient spaces intersect only in zero (Eq. 41, "precludes ... rank-
  deficient methods; for example, the one-point uniformly reduced integration" element), and the stress
  interpolation is L2-orthogonal to the enhanced strain (Eq. 44) with piecewise-constant stress admitted
  (Eq. 45; "classical patch test").
- **No better cure for ν → 0.5 in the source; my numbers stand.** Simo & Armero do not give a different
  volumetric-locking remedy: their cure *is* Q1/E4, which they call "excellent" in the three-dimensional nearly
  incompressible problem (Introduction, p. 1414) and show on a finite-strain necking bar (960 bricks, K/µ ≈ 10⁴,
  §4.2.2, Figs. 11–12), where the mean-dilatation brick Q1/P0 develops an hourglass mode and Q1/E4 does not.
  That is a qualitative comparison against Q1/P0, on a many-element, finite-deformation problem; it does not
  contradict the 4 % (ν = 0.49) and 13 % (ν = 0.4999) stiffness of a one-cell-thick *linear* beam in §1.1, which
  is still far better than plain Q1 (+60 %, +297 %). They also report that Q1/P0 is unstable in 3-D at very high
  aspect ratios (conclusions) and say mean dilatation "behave[s] poorly in the three dimensional problem" —
  at finite strain. My linear selective-integration beam showed no such problem, so that warning is about
  nonlinear use. A further caveat for us: they note "Q1/E4 may exhibit undesirable modes in the presence of
  fairly high strains, specially in the compressive regime" (conclusions); our strains are 10⁻⁹–10⁻⁴.
  **Recommendation unchanged: incompatible modes for ν ≤ 0.35, selective integration with ≥ 2 cells above.**
- **Wilson 1973 is not fetched, but its DOI and pages are now known** (from Kozar et al.'s reference list):
  pp. 43–57, doi:10.1016/B978-0-12-253250-4.50008-7.
- **Kozar, Rukavina & Ibrahimbegović 2018** (the third file; *Građevinar* 70(1), 19–29, "Method of
  incompatible modes — overview and application", doi:10.14256/JCE.2078.2017) is a subject review, not a
  source of new results. It restates the conditions (§2: incompatible strain independent of the compatible
  one, Eq. 17; stress orthogonal to the incompatible parameters, Eq. 18; patch test, Eq. 19), builds Q4 and Q8
  elements with `B_m = [B G]` and static condensation (§3), and shows on a thin cantilever (E = 3×10⁷, ν = 0.33,
  L = 3, h = 0.2) tip displacements of about 0.54 (2-D and 3-D with modes) for the force load against exact
  0.562, and 0.112 for the moment load (exact 0.112), against 0.169 and 0.167 without modes (Tables 1–2; the
  extracted columns are jumbled, so read these to two figures only). This
  is the same behaviour as in §1.1 (a few per cent for the force case from shear, exact for pure bending). One
  inconsistency: they quote the Q8 + modes stiffness as 32×32, whereas 24 dof + 9 modes is 33, and the mode
  matrix of their Eq. 68 does not survive text extraction; I cannot say which is right.
- **Lamb 1904 (full)**: the scanned paper (43 pages) was read on the rendered pages 1–4, 40–43 (Eq. 165's
  impulse, Fig. 10, the Summary); the detailed integrals of §§ 2–39 were not. Its doi is confirmed on the file
  as 10.1098/rsta.1904.0013 (the from-memory DOI was right; the earlier list's rspl DOI is the abstract).
  What it provides for the validation case is in §5.1 and §7 below.

**Sharpened (no change of direction).**
- **Q1 + incompatible modes stays the default, but not for near-incompressible materials.** Measured here
  (new, `nu_locking.py`): at ν = 0.2–0.3 it is within 0.7 % at one cell through the thickness; at ν = 0.49 it
  is 4 % stiff (one cell) and 2.4 % (two); at ν = 0.4999, 13 % and 11 %, and still 3.5 % at four cells. Plain
  selective reduced integration (= B-bar on cubes) handles ν → 0.5 (within about 2 % at two cells) but is
  −5 % to −15 % at one cell. So: incompatible modes for metals, concrete, glass and masonry (ν ≤ 0.35);
  a selective-integration variant, with at least two cells, for rubber, flesh, saturated soil (§1.1).
- Finite-cell stiffness floor α: Parvizian et al. report that 10⁻¹⁰ "can replace zero" (§2.3) [R].
- The thermoelastic coupling number is now read from Biot 1956 (§2, text after Eq. 2.16) rather than recalled (§4).
- XPBD: stiffness is iteration-count independent, but the solution is only the implicit one in the limit of
  many iterations (Macklin et al. §1–§4): unchanged conclusion (§3).
- The prompt elasto-gravity closed form (Harms et al. 2015) is a *full-space* solution; it degrades once P waves
  reach the surface, and badly for shallow events near the epicentre (§5.5).
- Beker et al. 2012 §4: for *local* sources the fall of Newtonian noise with depth is much weaker than the
  distant-Rayleigh model predicts, because body waves dominate at depth: the kinematic ground must carry P and
  SV, not Rayleigh only (§5.1, already the recommendation; now with its reason).

**Still not read** (and why) is in §9 and at the end.

---

## Summary (one page)

**What the instrument cares about.** The read is linear in displacement: a deformed body is its mass
parcels moved by `u`, and the gravity change is the dipole sum
`δa(r₀) = G Σ ρΔV [u − 3(e·u)e]/|r−r₀|³` (Harms 2019, Eq. 45 [R]), which already contains both the bulk
density change `−ρ∇·u` and the surface term (Harms Eqs. 46–47 [R]). Mass is conserved exactly by
relocation (as the S13 note already decided). Displacements of interest are nm–mm and strains 10⁻⁹–10⁻⁴,
so **small-strain linear elasticity is the right physics for everything the towers read**; the
second-order error is `|u|/r` relative (10⁻³ for 1 mm at 1 m), bounded and recorded.

**The frequency band decides the priorities.** AION-10's three-pulse sequence weights a differential
acceleration by `|H|/T² = sinc²(πfT)`, T = 0.73 s: 1.0 below 0.1 Hz, 0.11 at 1 Hz, **a null at 1.37 Hz**,
1.2×10⁻³ at 10 Hz, 7.6×10⁻⁵ at 50 Hz [M, (a)], then sampled at 0.5 Hz so that every structural mode
aliases into 0–0.25 Hz. Structural ringing (5–50 Hz) is suppressed by 10³–10⁴; **quasi-static
deformation** (thermal expansion, a floor sagging under a person or a vehicle, ground loading) and
low-frequency motion (sway, microseisms, surface waves) is read at full weight. One example sized here: a
6 m concrete slab sagging 14 µm under a 70 kg person reads **0.26–0.45 mrad**, 5–9× one shot's noise and
~10⁻³ of the person's own signal [M, (b)]. So the quasi-static solve matters as much as the modes.

**Recommendation.**
1. **One linear-elastic voxel FE core** (S7 track, TASK-S7-05 split into four tasks, §8): one trilinear
   hexahedron per finest-level cell of an *elastic mesh* derived from the body's tree (2:1 graded, hanging
   nodes constrained, as S9's fluid octree), with **incompatible modes** (Wilson/Taylor), condensed per
   element. Measured on a clamped voxel beam: plain Q1 is **21 % stiff** in the first frequency with one
   cell through the thickness and 6 % with two; incompatible modes are **within 0.3 % at one cell** [M].
   Selective reduced integration (the B-bar family) does not cure bending locking (−5 % at one cell) and is
   kept only for near-incompressible materials (ν ≳ 0.4), where incompatible modes alone lock (4 % stiff at
   ν = 0.49, 13 % at 0.4999 with one cell) [M, `nu_locking.py`].
2. **Three solves on one operator, on the GPU matrix-free at the finest level with geometric multigrid**
   (an assembled sparse matrix is the CPU oracle: matrix-free is no win at Q1 on a CPU, §1.4): quasi-static
   `K u = f` (thermal strain, loads; outer double-precision PCG to a stated tolerance with a single-precision
   V-cycle, not a fixed number of V-cycles as real-time codes do);
   modes by **LOBPCG** preconditioned by the same multigrid (shift-invert Lanczos on the CPU as the oracle);
   time response by **modal superposition** with per-mode damping (Rayleigh as the default rule), exact per
   mode, so no time-step error.
3. **The read by mode weights**: one weight per mode and tower (`w = Σ m ∇K·φ`), then each shot costs one
   multiply-add per mode and tower; node moments of the S5 LOD walk get linear per-mode deltas.
4. **Ground motion is kinematic, not dynamic.** Move the voxel ground by analytic seismic fields (Rayleigh
   and body waves) and read it with the same displaced-body path; use the closed form (Harms Eqs. 96–101)
   far away. Harms: no published Newtonian-noise simulation "could not have been done with a kinematic
   simulation" [R, §3.5]. **Measured here: the voxel kinematic sum reproduces Harms's Rayleigh-wave closed
   form to 0.06–0.16 %** with cells no larger than the lowest atom's height and a ground patch of ±2λ [M].
   A dynamic wave solver (spectral elements or staggered FD with C-PML) is a later, optional research track
   for heterogeneous ground and topography.
5. **Thermal expansion is one-way**: the thermoelastic coupling number is 10⁻³ (concrete) to 3×10⁻²
   (aluminium) [M, (c)]; S13 calls the quasi-static solve whenever its temperature field changes.
6. **Large deformation is not in this track**: corotation/floating frame for bodies that also move rigidly
   (S7), XPBD only as a visual, labelled; MPM stays the granular/fracture idea it already is in the plan.

**Correction to an earlier note.** `docs/research/s14-wider-world.md` used γ ≈ 0.7 "to be taken from the
primary source". The source gives `γ(ν) = k(1−ζ)/(q_zP − kζ)` (Harms Eq. 99 [R]): **0.711 at ν = 0.20, 0.754
at 0.25, 0.799 at 0.30, 0.827 at 0.33** [M]; the same function appears as Junca et al. Eq. 13 and follows from
Hughes & Thorne Eq. 1.24 (§5.2). The form `Γ_zz = 2πGρ γ k ξ_z e^{−kh}` above ground holds (derivative of
Harms Eq. 100 [D]). Underground (a shaft) the two-exponential form of Harms Eq. 97 applies instead (§5.3);
Harms, Badurina et al. and Junca et al. agree on it once read correctly.

**What could not be verified:** the GPU cost figures are from Dick et al., Wu et al. and extrapolation, on 2010–11
Fermi hardware, not measured on the 1660 SUPER; the slab estimate idealises the slab as simply supported;
the three-dimensional incompatible-mode element (9 modes) is now read in Simo & Armero and agrees with mine,
but Wilson et al. 1973 itself was not fetched; and the full Harms 2019 Living Review was not among the fetched files (I used the copy already
read, whose equation numbers the note cites). See §9.

---

## 1. Voxel and octree hexahedral FE

### 1.1 Elements and locking

A voxel is a trilinear hexahedron (Q1, 8 nodes, 24 dofs). All cubic elements of one size share one element
matrix: `K_e ∝ E·s` and `M_e ∝ ρ·s³`, so a matrix-free code stores a material index and a level per element
and nothing else, which is why voxel FE scales (Arbenz et al. 2008 [R, abstract]; Dick et al. 2011, §3 [R]; Wu et al. 2016, §5.1 [R]: "since the element matrices scale linearly with the
Young's modulus value, only a single element matrix K0 is required").

Q1 with full 2×2×2 integration *shear-locks* in bending: it cannot represent pure bending without spurious
shear strain. Remedies in the literature:

| Remedy | What it fixes | Source |
|---|---|---|
| Incompatible modes (Q1 + 3 bubble modes per component, condensed) | bending (shear) locking; exact for pure bending on a rectangular element; weak near ν = 0.5 (measured below) | Wilson et al. 1973 [A, not fetched; pp. 43–57, doi:10.1016/B978-0-12-253250-4.50008-7 per Kozar et al.]; Simo & Armero 1992 [R]: the 3-D brick Q1/E4 with 9 modes, equal to QM6 for linear isotropic elasticity (Eqs. 66–69); Kozar et al. 2018 [R, review]; Taylor, Beresford & Wilson 1976 [R]: the original Q6 fails the patch test unless the element is a parallelogram, QM6 repairs it (pp. 1211–1216, Table I: both exact in pure bending on the rectangular mesh); Ibrahimbegovic & Wilson 1991 [R]: the same repair by Lagrange multipliers for any choice of modes; Simo & Rifai 1990 [R]: embeds it in a three-field enhanced-strain form, the patch test holds if the stress interpolation contains constants and is L2-orthogonal to the enhanced strain (§2) |
| Selective reduced integration / B-bar | volumetric locking as ν → 0.5 | Hughes 1980 [R]: generalises selective integration and mean dilatation; "in rectilinear situations it is identical to a selective integration approach" (Introduction), i.e. for cubes |
| One-point integration + hourglass control | both, at the cost of hourglass modes to stabilise | Flanagan & Belytschko 1981 [R, summary and introduction]: they present it for explicit transient analysis and note that selective schemes entail "severe computational penalties for transient analysis"; not our case (linear, implicit) |
| Several cells through every member | everything, at cost | Guldberg et al. 1998 [R]: 3-D digital (voxel) cantilever, "a minimum discretization of three to four digital finite elements through the beam cross section to avoid high numerical stiffening errors under bending" |

**Measured [M]** (`cantilever.py`, concrete E = 30 GPa, ν = 0.2, clamped square beam L/h = 10, cubic
cells, consistent mass, first bending frequency against the n = 8 incompatible-mode result; Euler–Bernoulli
is 0.5 % above it, the expected shear and rotary-inertia correction at L/h = 10):

| cells through the thickness | 1 | 2 | 4 | 8 |
|---|---|---|---|---|
| Q1 (full integration) | +21.5 % | +6.0 % | +1.5 % | +0.38 % |
| Q1 selective reduced integration | −5.0 % | +0.87 % | +0.32 % | — |
| Q1 + incompatible modes | **+0.28 %** | +0.20 % | +0.06 % | ref. |

All three pass the constant-strain patch test to 5×10⁻¹⁶ and the consistent mass matrix sums to ρV to
10⁻¹⁶. This repeats in 3-D and for dynamics what the S13 note measured in 2-D for static thermal bending
(Q4 3.7 % low at one cell, incompatible modes exact). Walls and floors are 1–3 cells thick at 5–10 cm pitch,
so **incompatible modes are the default element**. For distorted elements the Taylor correction matters;
voxels are undistorted cubes, so it costs nothing here, but keep it so the patch test holds if non-cubic
cells ever appear.

**What the sources add to this, and what I measured after reading them.**
- The Q1 numbers agree with the one voxel-specific study I could read: Guldberg et al. 1998 (abstract, §Results)
  find a 3-D digital cantilever needs three to four elements through the section to avoid stiffening in
  bending; mine: Q1 +1.5 % at four cells, +6 % at two. Their digital boundaries also oscillate about the exact
  stress, so stress in boundary cells is noisy even when displacements are good; the read uses displacements.
- MacNeal & Harder 1985 (Table 6) describe MSC/NASTRAN's HEXA(8) as "isoparametric with selective reduced
  order integration and internal strain functions equivalent to bubble functions", i.e. both remedies together;
  on their simply supported square plate under uniform load it is within about 1 % of the exact deflection on
  the coarsest mesh and within 0.2 % one refinement later (Table 11a; the extracted table columns are scrambled,
  so treat as indicative); the fully integrated 20-node HEX20 is the poor one (0.02 of exact on the coarsest
  mesh).
- Simo & Rifai 1990 (§6.1, §6.1.2) report the *two-dimensional* Taylor element as excellent in bending and,
  "not widely appreciated", good in the incompressible limit (driven-cavity pressures oscillate but converge).
  They do not treat the three-dimensional brick; Simo & Armero 1992 do (second fetch, above) and call the 3-D
  Q1/E4 brick excellent for nearly incompressible *finite-strain* problems against the mean-dilatation brick,
  without a linear bending-beam number. I therefore measured it.

**Near-incompressible materials [M]** (`nu_locking.py`, same beam, error in f₁ against the n = 8 result of a
selective-plus-incompatible-modes element built for this test; its own n = 1 result is unusable, −90 %, so it
is not a candidate and its cause is not investigated; this reference is 0.34 % below the Q1 + incompatible-modes n = 8 value used for the first table, so the incompatible-mode column here reads that much higher than there at ν = 0.2):

| ν | cells | Q1 | Q1 selective (B-bar) | Q1 + incompatible modes |
|---|---|---|---|---|
| 0.3 | 1 / 2 / 4 | +24.6 / +7.3 / +2.2 % | −8.6 / +1.2 / +0.7 % | +0.7 / +0.7 / +0.5 % |
| 0.49 | 1 / 2 / 4 | +60 / +29 / +13 % | −15 / +2.1 / +0.9 % | +4.1 / +2.4 / +0.9 % |
| 0.4999 | 1 / 2 / 4 | +297 / +159 / +85 % | −16 / +2.2 / +0.95 % | +13 / +10.8 / +3.5 % |

So the 9-mode element does *not* escape volumetric locking: fine for ν ≤ 0.35 (concrete 0.2, steel 0.3,
aluminium 0.33, granite, glass), poor for rubber, tissue and saturated soil. For those, use selective
(mean-dilatation) integration with two or more cells through the thickness. The material's ν is a property of
the body, so the element kind can be chosen per body; both pass the patch test. This does not overturn the
default; it bounds it.

### 1.2 Octree meshes and hanging nodes

The body's tree merges uniform regions into large leaves (S3). One element per merged leaf would be far too
coarse for bending (a 16-cell merged block is a single Q1 element), so the **elastic mesh is derived from the
tree, not equal to it**: split leaves until (i) every element is at most a set size, (ii) the mesh is 2:1
graded across faces and edges, (iii) any member has the cells through its thickness that §1.1 needs. A
node on the face or edge of a coarser neighbour ("hanging") is constrained to the coarse element's trilinear
interpolation, which keeps the displacement continuous; this is the standard treatment in octree FE codes
(p4est's forests of octrees with a scalable 2:1 balance algorithm, Burstedde et al. 2011 [R, abstract, §1];
matrix-free cell-based operators with hanging-node constraints, Kronbichler & Kormann 2012 [R, §3.1]; on GPUs,
Ljungkvist 2017 [R, abstract] and Kronbichler & Ljungkvist 2019 [R, abstract, §1]).
The S9 fluid octree already enforces 2:1 balance; the elastic mesh should use the same balancing code.
Dick et al. build an octree only for the multigrid hierarchy over a *uniform* finest grid, with partly
filled coarse cells [R, §3.3]; they do not use hanging nodes; nor do Wu et al., who create a coarse cell "iff
it covers at least one cell on the previous level" (§5).

What the matrix-free hanging-node papers do, concretely [R]: a constrained node is eliminated and replaced, in
the cell's list of global indices, by the nodes it is constrained to, with the weights kept in a small pool,
because the number of distinct weight combinations depends on the dimension and degree but not on the problem
size (Kronbichler & Kormann §3.1.2; for Q1 in 2-D every hanging node has weights ½, ½; in 3-D they are ½, ½ on
an edge and ¼ ×4 on a face, by the same trilinear interpolation, my reading). The cell operator is applied as
`P_k C u`, constraint application interleaved with the cell kernel (their strategy ii). For *multigrid* on a
graded mesh two approaches exist: global coarsening (hanging nodes on the coarse levels too) and local
smoothing (level by level, with zero Dirichlet values on the interfaces to coarser cells), which "has the
advantage of always smoothing on meshes without hanging nodes and a somewhat lower computational complexity";
Kronbichler & Ljungkvist chose local smoothing (§1). Their GPU solver was tested on scalar Poisson problems
(1.5–2× faster than a multicore CPU, P100 against Broadwell), not elasticity: **the vector-valued elastic case
on a graded voxel mesh is an extrapolation of their method, to be validated against the CPU oracle**.

The gravity read keeps using the **mass tree** (S3, S5): the FE mesh only supplies `u` at its nodes, and
each mass leaf moves by `u` interpolated at its centroid (§6).

### 1.3 Curved and partial cells: the finite cell method

S3's leaves carry fill fractions (sample counts) and S5 has the exact skin. The finite cell method
(Parvizian, Düster & Rank 2007 [R]; review Schillinger & Ruess 2014/2015 [R, abstract, §1–2]) integrates
stiffness only over the material part of a cut cell, with a small fictitious stiffness α in the void part:
"any α as small as 10⁻¹⁰ can replace zero" (Parvizian §2.3; their examples are 2-D linear elasticity, with
p-extension for exponential convergence, which a Q1 voxel does not have: h-extension converges algebraically,
§2.4 case 1). Very small cut fractions make the system ill-conditioned (Schillinger & Ruess, §2.2.1);
de Prenter et al. 2019 [R, abstract] cure that with a geometric multigrid whose Schwarz-type smoothers give
"mesh-independent and cut-element-independent convergence rates", at cost linear in the number of dof.

For Cavendish: **mass stays exact** (the tree's mass, relocated); only stiffness is approximated. The first
version scales each partial cell's stiffness by its fill fraction (one-point finite cell) with a floor on the
fraction; what this costs in a curved body's frequencies is a measurement in the slice (§7, a voxel sphere's
breathing mode against Lamb's closed form, or a cylinder's bending against Euler–Bernoulli at several
pitches). Staircase geometry converges in the voxel-FE literature with refinement: Guldberg et al. 1998 [R, abstract]
compared digital (voxel) and smooth-boundary models for a 2-D plate with a hole and a 3-D *circular* cantilever;
the digital boundary solution oscillates about the exact one, the cantilever's free-end displacement came
within 1 % of theory and the section-averaged von Mises stress converged to under 4 % as the beam diameter
to voxel size grew. Their circular cantilever is a ready validation case for §7's curved-body test. The
fill-fraction scaling proposed here is the "density" device of voxel topology optimisation (Wu et al. §5: void
elements carry ρ_min = 0.001), not the finite cell method proper.

### 1.4 Solvers and cost

| Solver | Role | Evidence |
|---|---|---|
| Geometric multigrid (Galerkin coarse operators, trilinear transfer, multicolour Gauss–Seidel) as a CG preconditioner; fine level matrix-free | quasi-static `K u = f` | **GPU:** Dick et al. 2011: 269,000 hexahedra at 11 steps/s and 12,000 at 120 steps/s on a Fermi GPU with two V-cycles a step (unconverged), up to 27× one CPU core and 4× eight cores [R, abstract and results]. Wu, Dick & Westermann 2016 [R, §5, §7, Fig. 7, 8, 12]: voxel topology optimisation on a Tesla C2070 (6 GB): 1–15 million elements, Neptune 14.19 million elements (621×488×1000 grid) in 12 min for the whole optimisation; V-cycle with one pre- and one post-smoothing 8-colour Gauss–Seidel step inside CG, stopped at ‖r‖ ≤ 10⁻²‖f‖; finest-level stencils assembled on the fly, coarsened 4:1 from level 0 to 2 then 2:1, which "only slightly decreases the convergence rate" but "significantly reduces the memory requirements"; the coarsest level is solved on the CPU by Cholesky. **CPU clusters:** Adams et al. 2004 [R, abstract, Table in §5]: smoothed-aggregation AMG, 537 million dof (a human vertebral body, non-linear micro-FE) on up to 4088 processors; Arbenz et al. 2008 [R, abstract]: aggregation multigrid *without* assembling the matrix (element-by-element), 247,734,272 elements, 1,178,736,360 rows in 12 min on 1024 Cray XT3 processors; Aage et al. 2017 [R, abstract, Methods]: 1.1 billion voxels (3×10⁹ dof), Galerkin multigrid with SOR smoothing in PETSc, 1–5 days on 8,000 CPUs for the whole optimisation |
| LOBPCG, preconditioned by the same multigrid | lowest m modes | Knyazev 2001 [R, abstract, §5]: "practically as efficient as the 'ideal' algorithm" (preconditioned CG for one known eigenvalue) with the same preconditioner, and about twice as fast as Jacobi–Davidson on his tests; his tests use model problems (Laplacians) and random preconditioners, so *a multigrid preconditioner on the elasticity operator with six rigid-body modes is my pairing, untested here* |
| Shift-invert Lanczos (ARPACK) with a sparse direct factorisation | CPU oracle for small bodies | measured: 6 modes of a 19,440-dof beam in 1.4 s on one core [M]; direct fill grows superlinearly in 3-D, so stop at ~10⁵ dof |
| Assembled sparse matrix + CG/AMG | CPU oracle at 10⁵–10⁶ dof | cell-based matrix-free beats a sparse matrix only from degree 2 on a CPU (Kronbichler & Kormann §5.1, Fig. 4); Ljungkvist 2017 abstract: 8× faster than CUSPARSE matrix-based, but for degree ≥ 2 |
| Explicit central differences / Newmark | wave propagation, nonlinear transients | not needed for the default (§2, §5) |

Two things the sources show that the first version missed. **(i) The multigrid hierarchy is the memory.** Dick
et al. and Wu et al. both use Galerkin coarse operators with 27 3×3 blocks per vertex (their "stencil", Dick
§3; Wu §5), so storage is 2.4 kB per element in double precision with a standard 2:1 hierarchy (Wu §5,
"2.3 GB per one million finite elements"), about 1.2 kB in single [D]; the 4:1 first coarsening and on-the-fly
finest level are what let 14.2 million elements fit in 6 GB. For *linear* elasticity on a fixed mesh the
Galerkin hierarchy is built once, so the cost is memory, not time. **(ii) Matrix-free is the right word only
for the finest level**, and its advantage there comes from the shared element matrix (§1.1), not from sum
factorisation, which Q1 does not use.

A real-time code's "two V-cycles a step" is an *unconverged* solve; Cavendish solves to a residual
tolerance and records it (a frame budget may preview, then refine, as §5.1 of the plan does). Note that even
Wu et al. stop at 10⁻² relative residual because a topology-optimisation step does not need more; the read
here needs more (§8, Precision).

**Scale.** At 10⁹–10¹¹ world cells the FE never covers the world. It covers the bodies flagged elastic,
inside the towers' bubble: a floor slab 10×10×0.2 m at 5 cm is 1.6×10⁵ elements (~5×10⁵ dof); a
two-storey building's walls and floors at 5 cm, ~10⁷ elements [D]. **Memory: ~1.2–2.4 kB per element with
Galerkin stencils stored (Wu §5), so 10⁷ elements is 12–24 GB and does not fit the 1660 SUPER's 6 GB unless the
4:1 coarsening trick or rediscretised coarse operators (about 100 B per node [D], not demonstrated in the
sources) are used; the first version's "~10⁷ nodes fit in 6 GB" rested on the 100 B figure and is withdrawn
as a claim, to be settled by a measurement in S7-05b.** The slab (1.6×10⁵ elements, ~0.4 GB) fits easily.
Terrain and the far ground move by analytic fields (§5), never by FE.

---

## 2. Vibration: modes, damping, forcing, and what the gradiometer sees

**Modes.** `K φ = ω² M φ` for the lowest m modes (m ≤ 64 matches `Motion::Modes`'s cap). Free bodies have six
zero modes; they are the rigid motion that S7 owns, and are removed (§3).

**Modal superposition.** With mass-normalised modes, `u(t) = Σ q_m(t) φ_m` and each `q_m` is a damped
oscillator with a closed-form response; real-time vibration by modal synthesis is long established in
graphics (Pentland & Williams 1989 [R, abstract, §1]: "up to two orders of magnitude" in efficiency and
"simple, closed-form solutions" per mode; James & Pai 2002 [R, abstract]: precomputed modal vibration models
on graphics hardware; reduced nonlinear: Barbič & James 2005 [R, abstract]: for St Venant–Kirchhoff the
reduced internal force is a cubic polynomial in the reduced coordinates).
Here it is also *exact per mode*: the only error is truncation of higher modes, bounded by adding the static
residual of the truncated modes (mode-acceleration correction [U]) or by showing the read's change as modes
are added.

**Damping.** Rayleigh damping `C = αM + βK` gives the modal ratio `ζ_m = α/(2ω_m) + βω_m/2` [U, standard];
since the read is modal, store a damping ratio per mode (from Rayleigh's rule or set per material) instead.
Structural damping of concrete and steel is of order 1–5 % [U]; it decides how long a struck slab rings and
nothing in the gravity sum.

**Harmonic response to machines.** A force `F e^{iωt}` gives `q_m = φ_mᵀF / (ω_m² − ω² + 2iζ_mω_mω)` in
steady state [D]; a pump at 25 Hz on a slab is two lines of code once the modes exist.

**What the gradiometer sees [M, (a)].** `|H(f)|/T² = sinc²(πfT)` (the triangle sensitivity of the
three-pulse sequence; derived in `docs/research/s14-wider-world.md` (E2); the same `sin²(ωT/2)/ω²` factor
appears in Badurina et al. 2023, Eq. 3.16 [R]):

| f (Hz) | 0.1 | 0.5 | 1.0 | 1.37 | 2 | 5 | 10 | 20 | 50 |
|---|---|---|---|---|---|---|---|---|---|
| \|H\|/T² | 0.98 | 0.63 | 0.11 | 1×10⁻⁸ (null) | 0.047 | 6.0×10⁻³ | 1.2×10⁻³ | 4.3×10⁻⁴ | 7.6×10⁻⁵ |

Sampled at 0.5 Hz, the suppressed high-frequency remainder aliases into the band. So a slab mode at 10 Hz
with 1 µm amplitude 5 m away, about 70 µrad if quasi-static (the S13 note's wall estimate), reads ~0.1 µrad.
Large slow structures (building sway near 1 Hz, tall-building periods ~ storeys/10 s [U]) and quasi-static
deformation are what count. The engine must apply the flight's own time weighting rather than sampling `u`
at shot times, or the 10³ suppression is lost; the existing read already integrates over the flight.

---

## 3. Large deformation: when linear is enough

- **Strain and rotation.** Linear strain ignores the quadratic term `½∇uᵀ∇u`; under a rigid rotation θ it
  invents strain ~θ²/2 and visibly inflates a turning body (the "pitbull" artefact of Müller & Gross 2004,
  Fig. 1 [R]). Corotated (warped-stiffness) FE removes each element's rotation before applying linear
  stiffness (Müller & Gross 2004 [R, §3]; on hexahedra on the GPU, Dick et al. 2011 [R]; Georgii &
  Westermann 2008 [R, abstract, §1, Fig. 1]: polar decomposition of the deformation gradient "can introduce
  instabilities for large element rotations", and corotation "requires to continually rebuild consistent
  system matrices at different resolution levels", which held them to 17 steps/s for up to 12k elements on a
  desktop, "prohibit[ing] the use of large numbers of finite elements at rates comparable to the small-strain
  finite element formulation". This is the strongest reason to keep the linear core: a fixed matrix
  factorised or hierarchy-built once).
- **For Cavendish** the large rotations belong to S7's rigid bodies. The clean split is the *floating frame*:
  rigid motion from S7, elastic modes in the body's own frame, which is corotation at body level and exactly
  how per-object volumes already work (§5.2 of the plan). Within a body, strains of 10⁻⁴ give an error of
  10⁻⁴ relative in `u`: below the read's tolerance.
- **Hyperelastic models** (St Venant–Kirchhoff, Neo-Hookean; Bonet & Wood 2008 [A, the fetched file is the front matter only]; Smith, de Goes & Kim
  2018 for a robust Neo-Hookean [R, Fig. 1, abstract]: a 45,809-element hexahedral lattice at ν = 0.488 took
  on average 13.7 Newton and 5,860 CG iterations, 25.6 s per step, which is the cost of this class of model
  next to a fixed-matrix linear solve, and their comparison shows corotational elasticity failing to preserve
  volume at that ν) are for rubber, flesh and soft bodies at large strain.
  None of these move enough mass at µrad to need more than linear elasticity in a small-strain frame;
  a soft body that squashes visibly is an animation, and its mass is relocated exactly whatever model moved
  it.
- **XPBD** (Macklin, Müller & Chentanez 2016 [R, abstract, §1–4]) removes PBD's dependence of stiffness on
  time step and iteration count by giving constraints a compliance tied to an elastic potential; "the solution
  approaches that of Newton's method as the number of iterations increases" (§2) and they compare with
  non-linear solvers only for "visually similar results" (abstract). So at finite iterations it is an inexact
  implicit solve with no residual guarantee; fine for visuals, labelled.
- **MPM** (Sulsky, Chen & Schreyer 1993 [R, abstract]; Stomakhin et al. 2013 [R, abstract]; MLS-MPM, Hu et
  al. 2018 [R, abstract, §7]; fracture: Wolper et al. 2019 [R, abstract]) is the one-solver option for snow,
  sand, rubble and fracture. Hu et al. Fig. 17 shows "numerical dissipation" in an oscillating elastic cube
  for both MPM and MLS-MPM ("nearly identical"); that is a visual-plausibility trade-off, and it is the wrong
  property for a structural mode whose ring-down matters. The plan already keeps MPM as the granular layer
  idea; it is not the tool for small-amplitude structural response, where modal FE is exact and cheap (as the
  plan's §4 says).

---

## 4. Thermal expansion: coupling to S13's heat

Linear thermoelasticity adds the thermal strain `ε_th = αΔT I`; the load is `f = ∫ Bᵀ D ε_th dV`, solved
quasi-statically because elastic wave times (ms) are far below thermal times (the S13 note, §3.6).

**One-way coupling is enough.** The two-way term (heating by compression) is governed by the coupling number
`δ = (3K)² α² T₀ / ((λ+2µ) ρ c)`, which is Biot 1956's result [R, §2, Eqs. 2.2–2.3 and the text after
Eq. 2.16: for adiabatic deformation "the Lamé constant λ is replaced by λ + β²(T/c) and µ remains unchanged",
with β = (λ + 2µ/3) α_t, c the heat capacity of the unit volume, α_t his coefficient of thermal expansion; his
Eq. 2.3 gives β = Kα_t, so α_t must be the *volumetric* coefficient [D], and with the linear coefficient α, β = 3Kα, which
is the form evaluated here; the relative change of λ+2µ is then exactly δ with c_v taken as c_p] and evaluated
here: **steel 1.1×10⁻², aluminium 2.6×10⁻², concrete 1.0×10⁻³, granite 1.5×10⁻³** [M, (c)]. It enters
the elastic response as a correction of order δ (the adiabatic/isothermal modulus difference), so a
quasi-static, one-way solve is within 1–3 % for metals and 0.1 % for masonry, and it changes the
*displacement* by that fraction, not the conserved mass. Thermoelastic damping is negligible for the read.

S13's validation rows for expansion (free bar, clamped bar, bimetal, gradient-bent beam, the thermoelastic
sphere interior and exterior) are all tests of this solve and stay as written in the S13 note.

---

## 5. Ground motion and seismic waves

### 5.1 Kinematic, not dynamic

Harms distinguishes *kinematic* simulations (each element displaced by an analytic seismic field, gravity
summed by Eq. 45) from *dynamic* ones (the field evolved by FE/SEM from sources), and recommends kinematic
whenever analytic fields exist: "kinematic simulations are faster by orders of magnitude", "not a single
simulation of Newtonian noise has been carried out so far that could not have been done with a kinematic
simulation", while "dynamical simulations are required to represent seismic fields in complex geologies and
surface topographies" [R, §3.5, pp. 46–48; the kinematic path fails "whenever it is impossible to provide
analytical expressions for the seismic field"]. Dynamic simulations used COMSOL (Beker et al. 2012, §4 [R]: a
finite-element sum `a = G Σ r_i/|r_i|³ δρ_i m_i` over nodes, their Eq. 3) and specfem3d, the spectral-element
code (Komatitsch & Vilotte 1998 [A]; Komatitsch & Tromp 1999 [R]), with Eq. 45 added for gravity (Harms et al.
2015 [R], full-space prompt solution checked against SPECFEM3D). Staggered-grid velocity–stress finite
differences (Virieux 1986 [R]) and the perfectly matched layer (Bérenger 1994 [R, abstract]: introduced for
*electromagnetic* FDTD, the elastic versions descend from it; convolutional form: Komatitsch & Martin 2007 [R,
abstract]) are the other standard tools; GPU ports exist for both (Komatitsch, Michéa & Erlebacher 2009 [R,
abstract]: the spectral-element code in *single precision* on GeForce 8800/GTX 280; Michéa & Komatitsch 2010
[R, summary]: finite differences with convolutional PML, 20–60× a serial CPU).

What the solver papers say about cost, to confirm that deferring a wave solver is right [R]:
- Komatitsch & Tromp 1999 §6.1: the spectral-element method wants polynomial degree 5–10 and "roughly 5 points
  per minimum wavelength", on hexahedra *deformed* to follow the free surface and interfaces ("mesh design is a
  critical and difficult step", §6.2). It is a different element (Gauss–Lobatto–Legendre, diagonal mass
  matrix) from the Q1 voxel operator, so it does not come free with the S7 core. Its free-surface condition
  is natural, so Rayleigh waves are exact in the weak form.
- Virieux 1986 (§ on Rayleigh waves, Fig. 4 discussion): "The numerical Rayleigh wave has a lower amplitude than
  does the analytical Rayleigh wave", a misfit that "does not depend on the propagation" and is put down to
  discretisation at the source.
- Komatitsch & Martin 2007 (abstract): the classical discrete PML "suffers from large spurious reflections" at
  grazing incidence, which matters for a thin near-surface model; the convolutional PML improves it, and both
  are "intrinsically unstable in the case of some anisotropic materials".
- Beker et al. 2012 (§4, Figs. 5–6): the finite-element models show that for *local* sources the fall of
  Newtonian noise with depth "is significantly less than expected from the previously discussed distant source
  Rayleigh wave models", i.e. "NN at depth is dominated by contributions from body waves", and the suppression
  with depth increases with lower seismic velocity and higher frequency. For a shaft this says: a Rayleigh-only
  kinematic ground under-counts at depth; the P and SV body-wave plane waves are part of the kinematic set,
  and a local source (a pump, a lift) needs its own radiated field, not a Rayleigh train.

Cheapness at *our* scale is a point for a later dynamic solver rather than against it: 10 Hz in 250 m/s ground
has λ = 25 m, so five points per wavelength is a 5 m node spacing and a 100 m cube is 8,000 nodes [D]. The
reasons to defer remain the ones that follow: the closed forms exist for the flat homogeneous ground the
instrument first needs, a kinematic voxel sum can be validated against them to 0.06–0.16 % (§5.2), and a
dynamic solver adds a second element, absorbing boundaries and a source model, none of which has a known-answer
test at the µrad level except Lamb's problem (Lamb 1904 and Pekeris 1955, below and §7).

**Lamb's problem, as the reference for a dynamic solver [R, Lamb 1904 pp. 1–3, 40–43; Pekeris 1955].** Lamb
treats a half-space with a plane free surface, gravity ignored, forced at a point, chiefly by an *impulse
applied vertically to the surface* (his Eq. 165 is the type of impulse), after first doing the two-dimensional
problem of a pressure along a *line* of the surface, whose analysis "can be utilized afterwards for the
three-dimensional cases" (§1); he also gives simple-harmonic sources and new solutions for a periodic force
transverse to a line or at a point in an unlimited solid. His results for a surface impulse (Introduction and
Summary, pp. 2, 41): the disturbance divides into a **minor tremor**, which "sets in ... with some abruptness, after an interval
equal to the time which a wave of longitudinal displacement would take to traverse the distance from the
source", with a further marked feature at the arrival of transversal waves, and the **main shock**,
"propagated as a solitary wave (with one maximum and one minimum, in both the horizontal and vertical
displacements); its time-scale is constant; and its amplitude diminishes only in accordance with the usual law
of annular divergence", i.e. as r^{−1/2}, at the velocity of free Rayleigh waves. The horizontal displacement
shows first a single oscillation, then quiescence, then another at the transverse-wave epoch; the vertical
displacement leads up more gradually, with a jerk at the transverse-wave epoch, and at the Rayleigh arrival
"the vertical amplitude is distinctly the greater" (p. 42), consistent with the anisotropy ratio 2.2 of Hughes &
Thorne at ν = 0.33. The minor tremor depends on the time-integral of the impulse, the main shock on the impulse
itself (p. 41). Lamb gives the answer as integrals (Fig. 10 sketches the horizontal and vertical displacement
histories); it is Pekeris 1955 who reduced it to closed form for a Heaviside surface load (`w(t) = (1/r)f(τ)`,
his Eqs. 45–48, 66, singular at the Rayleigh arrival) and notes that Lamb's "method is, however, very
intricate" (§1). So: the *qualitative* test (a P-arrival precursor, a transverse-arrival feature, then a
solitary Rayleigh pulse of constant shape and r^{−1/2} amplitude, vertical larger than horizontal, speed c_R)
can be taken from Lamb; the *quantitative* test from Pekeris. I did not extract Lamb's own formulas, and a
test must not rely on the extracted text of the scan.

For Cavendish this means: **the voxel ground near the towers is moved by a superposition of analytic plane
waves** (Rayleigh, P, SV; S12's microseism spectrum and S14's teleseismic trains), read by the displaced-body
path, and **the far field is the closed form**. A dynamic solver is needed only for heterogeneous ground,
topography, or cavities comparable to the wavelength (the stairwell at 10 Hz, λ ≈ 25 m, is a case to check
later; Badurina et al. neglect scattering from a 5 m shaft for λ ≥ 40 m [R, §3.2: "negligible for cavity
radii much smaller than the wavelength", wavelengths 40 m to 10⁴ m, shaft radius O(5 m)]; Harms §3.3 gives
the cavity correction for a spherical cavity, with error O((ka)²) and a scattering amplitude of order
(k_P a)³ [R, §3.3.1–3.3.3 of the 2015 arXiv version]).

### 5.2 The Rayleigh-wave closed form, checked

Above a homogeneous half-space, a fundamental Rayleigh wave with vertical surface displacement `ξ_z` gives
`δa = 2πGρ γ(ν) ξ_z e^{−kh} (i cos φ, i sin φ, −1)` (Harms Eq. 100 [R]) with
`γ(ν) = k(1−ζ)/(q_zP − kζ)`, `ζ = √(q_zP/q_zS)`, `q_z{P,S} = √(k² − k_{P,S}²)` (Eqs. 38, 39, 98, 99 [R]).
γ = 1 would be the surface term alone; the compression inside the ground partly cancels it.

**Measured [M]** (`rayleigh.py`):
- The displacement field of Eq. 39 is traction-free at the Rayleigh speed to 10⁻¹³ *with a minus sign on the
  horizontal component, which I derived from potentials*. **Correction to the first version, which said the PDF
  extraction had lost that sign: the rendered page (Living Review p. 29) prints Eq. 39 without it, and as
  printed the field is not traction-free** (σ_xz = 1.24, σ_zz = 0.85 for ν = 0.25 in units ρ = c_S = A = 1;
  `shaft_check.out.txt`). It is either a misprint or the convention is `sin(ωt − k·ϱ)`; the closed forms
  (Eqs. 96–100) and γ are unaffected, and the voxel sums below used the traction-free field. A builder
  implementing Eq. 39 from the paper must check this sign against a traction-free test.
  `c_R/c_S` = 0.8931–0.9541 for ν = 0.1–0.49 (Hughes & Thorne Appendix B quote 0.904 at ν = 0.16 and 0.955 at
  0.5, which agrees; the secular equation is Vinh & Ogden 2004 Eq. 1 [R], which `rayleigh.py` solves by
  root-finding; their closed-form Eq. 21 was not implemented).
- γ(ν): 0.631 (0.10), 0.711 (0.20), 0.754 (0.25), 0.799 (0.30), 0.827 (0.33), 0.896 (0.40), 0.989 (0.49).
  **Checks against the sources after reading them** (`shaft_check.py`): (i) Harms Fig. 10 (p. 45) plots γ from
  about 0.55 at ν = 0 to 1 at ν = 0.5, consistent with these values. (ii) Junca et al. Eq. 13 (written with
  `c_R/α`, `c_R/β`) equals Harms Eq. 99 to six figures (0.754171 at ν = 0.25, 0.827339 at 0.33). (iii) Hughes &
  Thorne's Eq. 1.24 defines the mode's reduced transfer function as
  `β′ = √(3/2) |ξ_V + ∫₀^∞ ℛ(z) e^{−kz} dz| / √(|ξ_H|² + |ξ_V|²)`, ξ_V the surface source (the displaced
  surface) and the integral the subsurface source (the bulk compression). With ℛ = −∇·ξ of the half-space mode
  the bracket is `ξ_V (1 + (k−q_zP)/(q_zP − ζk)) = ξ_V γ` [D], which is Harms's Eq. 99, and evaluated
  numerically it gives β′ = 0.853 at ν = 0.33, against their published 0.86 (Eq. 2.7, p. 9), and 0.178 for the
  subsurface term alone against their 0.17 (same page); the anisotropy ratio comes out 2.21, theirs 2.2.
  Hughes & Thorne thus supply an independent derivation of γ, from 1998.
- **Kinematic voxel sum against the closed form**, ν = 0.33, ρ = 1800 kg/m³, λ_R = 25 m (10 Hz at
  250 m/s), Δa_z between 1 m and 11 m above ground:

| cell (m) | patch half-width (m) | depth (m) | cells | error |
|---|---|---|---|---|
| 1.0 | 25 (1λ) | 40 | 1.0×10⁵ | 0.79 % |
| 1.0 | 50 (2λ) | 60 | 6.0×10⁵ | 0.059 % |
| 1.0 | 100 | 80–240 | 3.2×10⁶ | 0.064 % |
| 0.5 | 100–200 | 80–160 | 2.6×10⁷–10⁸ | 0.08–0.09 % |
| 0.25 | 50 | 120 | 1.9×10⁷ | 0.16 % |
| 2.0 | 200–400 | 80 | 1.6–6.4×10⁶ | 7.2 % |

Two rules come out: **cells no larger than the lowest atom's height above ground** (2 m cells at 1 m height
fail by 7 %), and **a voxel patch of ±2λ** reaches 0.06 %. A residual of 0.06–0.16 % does not fall with
cell size or depth; I did not find its cause (candidates: the patch's lateral truncation interacting with the
near point, or point-dipole parcels next to the observation point). It is far inside the S14 model error and
is recorded as an open question, not explained.

**Pass criterion and control arm for the slice:** the voxel sum within 0.2 % of Eq. 96–100; with the bulk
term dropped (surface displacement only) the same test must fail by 1−γ ≈ 17 %.

### 5.3 Vertical gradiometers below ground: resolved

AION-10 sits in a stairwell, below ground level. The first version of this note found a factor of about 1.5
between Harms and Badurina et al. for the potential below ground. **There is no disagreement; the first
version misread the extracted text.** What the sources say [R] and what I checked [M, `shaft_check.py`]:

1. **Harms Eq. 97** (Living Review p. 45; cavity neglected, its wall to be added "at least for a very small
   cavity" from §3.3.1): at depth `h` below the surface the potential of a fundamental Rayleigh wave is
   `δφ = 2πGρ A e^{i(k·ϱ−ωt)} [ −2 e^{−h q_zP} + (1 + ζ) e^{−hk} ]`, `A = ξ_z(0,0)/(q_zP − kζ)` (Eq. 98),
   `ζ = √(q_zP/q_zS)`.
2. **Badurina et al. Eq. B.19** (p. 32, from Harms's Ref. 26): `V = −2πGρ Σ ξ (c_H/(qω)) ((1+s²)/(1−s²))
   [ (1 + √(q/s)) e^{−ωz/c_H} − 2 e^{−qωz/c_H} ]`, with `q = q_zP/k`, `s = q_zS/k`. The square root is
   in the printed equation and in the rendered page (it is lost in the extracted text, which is what I read
   first). The prefactor `(c_H/qω)(1+s²)/(1−s²)` equals the magnitude of Harms's `A/ξ_z = 1/(k(q − ζ))` exactly (1.47504 at ν = 0.33; the sign is opposite
   because Harms's `q_zP − kζ` is negative), and the bracket is Harms's.
3. **Junca et al. 2019 Eq. 11** (p. 4, MIGA, citing Harms) is Eq. 97 verbatim; their Eqs. 12–14 write the
   horizontal acceleration as `2πGρ γ ξ_z [e^{−hk} + b(e^{−h q_zP} − e^{−hk})] i cos φ` with `b = 2/(1−ζ)`;
   this equals `−∂_x` of Eq. 97 to 2×10⁻¹⁶ at three depths for ν = 0.25 and 0.33.
4. **Independent evaluation**: the Rayleigh field's surface mass sheet `ρ ξ_z` plus bulk density `−ρ∇·ξ`,
   each layer contributing `−2πGσ e^{−k|Δz|}/k`, integrated numerically at depths `kh` = 0.5, 1 and 3, equals
   Eq. 97 and B.19 to 10⁻¹⁵ relative.
5. **Surface consistency**: the vertical acceleration of Eq. 97 (below) and of Eq. 96 (above) differ at the
   surface by `4πGρ ξ_z` to 12 figures, the jump a mass sheet `ρ ξ_z` must produce.
6. **Where 1.5 came from.** Badurina's Eqs. B.21–B.22 are coefficients of the *phase*, which carries the
   z-derivative; the ratio of their two exponentials' coefficients is `−(1+√(q/s))/(2q)` (−1.450 at ν = 0.33),
   while Harms's potential has `−(1+ζ)/2` (−1.280); and with the root lost it reads `−(1+q/s)/(2q)` = −1.946.
   1.946/1.280 = 1.52, and 1.527 with the rounded `q = 0.88, s = 0.36`: the "factor 1.5" is that.

**Consequence for the gradiometer.** Differentiating Eq. 97, the vertical gradient at depth `h` is
`Γ_zz = 2πGρ A [ 2 q_zP² e^{−h q_zP} − (1+ζ) k² e^{−hk} ]` [D] (the z-derivative of the acceleration
`a_z = 2πGρ A [ 2 q_zP e^{−h q_zP} − (1+ζ) k e^{−hk} ]`, the sign convention that of Eq. 96), and the phase of
Badurina's Eq. B.24 is its difference between two heights. For a gradiometer whose lower and upper atoms are
at depths `h₁ < h₂`, evaluate that difference. At the surface the vertical acceleration, in units of
`2πGρ ξ_z`, is `−γ = −(1−ζ)/(q−ζ)` just above ground and `(2q−1−ζ)/(q−ζ)` just below, a difference of exactly
2 (item 5). The earlier "γ ≈ 0.7" replaced by Eq. 99 is for sites above ground; below it, use Eq. 97.

**What this does not settle.** The cavity: both papers neglect it (Badurina §3.2: scattering "negligible for
cavity radii much smaller than the wavelength", wavelength 40 m to 10⁴ m, shaft O(5 m)); Harms §3.3 gives the
error for a spherical cavity as O((ka)²) in the field and (k_P a)³ in the scattered amplitude. A stairwell
that is not small against 25 m (10 Hz) needs the kinematic voxel sum in a cut-out shaft, which is still the
right check, now a test of the *cavity*, not of the formula (§7). And Beker et al. (§5.1) say local sources
and body waves matter more at depth than the distant-Rayleigh form implies.

The far-field sizes in `s14-wider-world.md` §2.4–2.5 used `Γ = γ2πGρkξ_z` with γ = 0.7; replace γ by Eq. 99
(+8 to +18 % for ν = 0.25–0.33) above ground and use Eq. 97 below.

### 5.4 Older Newtonian-noise literature

- **Saulson 1984** [R, whole paper, five pages]. A dimensional-analysis model: the ground is cut into
  independently fluctuating regions of size λ_P/2 with an inner cutoff r_min = λ/4 (Eq. 5: Σ cos²θ/r⁴ =
  64π/(3λ⁴)); for P waves in the Earth, differential arm motion obeys `(ω⁴)|Δx|² = (16π²/3) G²ρ²|ΔX|²`
  (his Eq. 12, one-degree-of-freedom form used for the numbers, Eq. 21), plus an order-of-magnitude-smaller
  surface term (Eq. 16). He says plainly the cutoff "cannot be regarded as exact" and that coherent regions
  longer than a wavelength would raise the result, but argues scattering makes the model "a substantial
  overestimate" above 1 Hz; an appendix treats λ ≫ L where the noise grows linearly with baseline. Fig. 2's
  spectra use `|ΔX|² = 3×10⁻¹³ cm²/Hz (1 Hz/f)⁴` at an average site, two orders lower at a quiet one.
  His numbers are for a kilometre-scale laser interferometer on the ground (baselines 1–10 km), not an atom gradiometer.
- **Hughes & Thorne 1998** [R, §I, §II, Appendix B, Eqs. 1.6–1.26 and 2.1–2.8; rest of §III–IV and the
  appendices on layered media not read]. They define `T(f) = Δx̃/W̃ = 4πGρ β(f)/(2πf)²` for `f ≳ 3 Hz`
  (Eq. 1.8), W̃ the displacement spectrum averaged over three directions (`W̃ = √((2X̃² + Z̃²)/3)`, Eq. 1.7),
  and quote "Saulson's estimate β = 1/√3 = 0.58" (Eq. 1.9, citing his Eq. 21; from Saulson's Eq. 12 above,
  `ω²|x| = (2π/√3)Gρ|X|` per mass, 4π/√3 for the difference of two arms: consistent). Their layered-ground
  model gives `β ≈ 0.35–0.6` at quiet times and `0.15–1.4` at noisy times (abstract; Table I): fundamental
  Rayleigh (RF) modes `β′ ≈ 0.7` when the surface source dominates (Eq. 1.26b), Rayleigh-P overtones (RP)
  `≲ 0.15` because surface and subsurface sources cancel (Eq. 1.26a, "a consequence of mass conservation"),
  and Love modes zero (Eq. 1.11). They also write `β_J = γ_J Γ_J β′_J` (Eq. 1.17) with
  `Γ_J = exp(−ωℋ/c_HJ)` the height factor (Eq. 1.20, the same `e^{−kh}` as Harms) and `γ_J(y) =
  √(1 − J₂(y)/2)` a two-test-mass *correlation* factor (Eq. 1.19). **That `γ_J` is not Harms's γ(ν)**; the
  quantity comparable to Harms's γ is their `β′`, normalised to W̃ rather than to the vertical displacement
  (with the vertical normalisation, "β′ would be 1/√2" for a surface-dominated mode, p. 6), and a
  *horizontal* gradient on a laser arm rather than a vertical gradiometer. Their half-space number
  (β′_RF = 0.86 at ν = 0.33; 0.17 from the subsurface term alone) is reproduced in §5.2, and is a published
  validation number for any code that computes the Rayleigh read. What a layered ground does (RP modes
  cancelling, SV-dominated modes up to 1.4) is not in Harms's half-space γ; Cavendish's ground model is a
  half-space until the site survey says otherwise.
- **Harms 2019** [R, §3.2 Eq. 45; §3.4 Eqs. 86–99; §3.5; the rest of the 154 pages not read]: above.
- **Vetrano & Viceré 2013** [R, §1–§4 in part]: they extend Saulson's uncorrelated-cell model to one or two
  atom interferometers (Eqs. 25–32; geometry Fig. 3), find Newtonian noise limits "comparable to those
  affecting large-scale optical interferometers" in 0.1–10 Hz, take a Rayleigh-dominated spectrum
  `W̃ ∝ ω⁻²` and `h_NN ∝ ω⁻⁴`, and state that retaining only the first term "would give the same result as in
  [Saulson], corrected for a factor 2 which is wrong in the original paper" (just after Eq. 32). Their
  detector sits in a medium homogeneous around it ("a deep mine"), so no free surface.
- **Junca et al. 2019** [R, §III, Eqs. 10–19]: MIGA, horizontal baselines underground at LSBB; Rayleigh waves
  only ("Love waves ... do not produce GGN in an isotropic and homogeneous medium"), Eq. 11 = Harms Eq. 97,
  a spatial-correlation function `χ₁(k d) = 1 − J₀(kd) + J₂(kd)` for a baseline `d` (Eq. 19), and an
  atmospheric part not read.
- **Beker et al. 2012** [R, §4]: FE and analytic Newtonian-noise models, Rayleigh dominating near the surface
  and body waves at depth (§5.1). Their quiet-site benchmark `0.5 nm/√Hz (1 Hz/f)²` above 1 Hz (abstract).
- **Badurina et al. 2023** [R, §3.2, App. B]: see §5.3. **Carlton & McCabe 2023** (arXiv:2308.10731) [R, §II
  A, Eqs. 10–13, Fig. 3]: *anthropogenic* sources (a lift, a person on the stairs, a car, small animals) for
  AION-10, not seismic; the point-mass phase for a gradiometer and `ΔΦ ≈ −nk T² G M r (D² − 2h²)/(D² + h²)^{5/2}`
  (Eq. 13, the quadrupole limit, r the baseline), with the sign change at D ≈ h. A person on the stairs gives the largest
  phase; five small animals O(50) µrad. That is the validation case for the *rigid* read, and it contains no
  elasticity. **Carlton et al. 2024** (arXiv:2412.05379) is about *atmospheric* Newtonian noise and the benefit
  of going underground; I read its abstract and searched it for seismic or elastic content and found none to
  cite here.
- **Love waves (SH) produce no gravity perturbation in a half-space** [R, Harms §3.4.1; Junca §III; Hughes &
  Thorne Eq. 1.11].

### 5.5 Earthquakes near the site

A nearby earthquake's strong motion is mm–cm of ground displacement at 0.1–10 Hz [U]; through the same
kinematic path its Newtonian part is sized like the Rayleigh case, and the instrument shaking it causes is
far larger and out of the forward model (as S14 records). The prompt elasto-gravity signal is S14's closed
form (Harms et al. 2015 [R, abstract, §2, §3, Eq. 18]): an analytic solution for a point double couple in a
**homogeneous, infinite, non-self-gravitating** medium, with the prompt perturbation proportional to the second
time integral of the moment `I₂[M₀](t)` (Eq. 18: `a = (6G/r₀⁴)(…)I₂[M₀]`). Caveats the paper states: the
solution "was derived for infinite media", so for a source at depth `h` "we can expect deviations ... as soon
as P waves reach the surface at time h/α, which is typically a few seconds only"; against SPECFEM3D half-space
runs the agreement was good within the first seconds even for a dip-slip rupture that reaches the surface,
and "strong deviations ... were only observed for shallow events in cases where gravity was evaluated close to
the epicentre so that the contribution from passing Rayleigh waves was dominant" (§3); their conclusion is
that observability within 10 s "is severely challenged by the background microseism noise". Nothing here needs
a dynamic FE solve, but S14's near-epicentre case is exactly the one where the closed form is weakest and the
kinematic Rayleigh path takes over.

---

## 6. Mass moments of a deformed voxel body

- **The parcel rule.** Each mass leaf (S3) is a parcel of mass m_i at centroid x_i; it moves by `u(x_i)`
  interpolated from the FE nodes (trilinear in the element that contains x_i). Total mass is unchanged to the
  bit; the centre of mass moves by `Σ m_i u_i / M` exactly. This is the S13 note's rule and Harms's Eq. 45:
  relocating parcels *is* `−ρ∇·u` in the bulk plus `ρ u·n` at surfaces (Eqs. 46–47), so the voxel read never
  differentiates `u`. The density anomaly `−ρ∇·u` is still worth drawing in the viewer.
- **The cell's own strain** changes its second moment by `O(m s² ε)`; against the parcel's shift it is
  `O(s ε / |u|)` ~ s/L of the dipole term, second order for a smooth field [D]. Bound it in the slice by
  comparing to the leaf split into its eight children.
- **First order in modes.** For mode φ_m, the per-tower weight is `w_{m,t} = Σ_i m_i ∇K_t(x_i)·φ_m(x_i)`
  (one kernel gradient per leaf, mode and tower, done once); the phase at a shot is `Σ_m q_m w_{m,t}` with the
  flight's time weighting applied to `q_m`. Cost [M, (d)]: 10⁸ leaves × 64 modes × 5 towers is 3×10¹⁰ kernel
  gradients once, then 320 multiply-adds a shot. The weights can be computed without a new kernel as
  `(read(+εφ) − read(−εφ))/2ε` through S3's displaced-body path, error O(ε²); S3 already holds that path to
  the table method at 8×10⁻⁸ and 10⁻⁹ (the scaffold cases, `docs/slices/S3.md` R12 [R]).
- **The S5 LOD walk.** Node moments are sums of leaf moments and the far field is linear in them
  (`docs/research/s5-world-octree.md` §3 [R]). Per mode, store each node's first-moment delta `Σ m u` and
  second-moment delta `Σ m (x uᵀ + u xᵀ)`; at time t a node's moments are `M₀ + Σ q_m δM_m` (exact for the
  first moment, dropping `O(u²)` in the second, bounded by `m|u|²`). The same holds for a quasi-static field
  (one "mode" with amplitude 1).
- **Exactness for the second order.** Where amplitude is large (mm at 1 m: 10⁻³ relative), re-read the
  displaced body exactly (the S3 path) and record the difference as the linearisation's error, as the plan's
  phase-tolerance rule asks.

---

## 7. Validation cases (each with a control arm)

| Test | Asserts | Against | Control arm (must fail) |
|---|---|---|---|
| `fe_patch_test` | constant strain reproduced exactly on a graded mesh with hanging nodes | exact | release the hanging-node constraint |
| `cantilever_frequency` | f1 of a clamped voxel beam, 1–8 cells thick | Euler–Bernoulli with the shear correction; measured here: incompatible modes +0.28 % at 1 cell (ν = 0.2) | plain Q1 at 1 cell is +21 % |
| `cantilever_near_incompressible` | the selective-integration element at ν = 0.49, 2+ cells | same beam; measured here: within 2.1 % at two cells, 0.9 % at four | incompatible modes alone at ν = 0.4999: +11 % at two cells |
| `cantilever_tip_deflection` | static tip load | Euler–Bernoulli + Timoshenko shear term [U] | Q1 at 1 cell |
| `plate_modes` | simply supported square plate, `f₁₁ = (π/a²)√(D/ρh)` [U, Kirchhoff] | closed form | swap a boundary condition |
| `refined_equals_uniform` | an octree-graded mesh refined everywhere equals the uniform mesh | exact | — |
| `free_body_has_six_zero_modes` | rigid modes found and removed; their read equals S7's rigid read | exact | add a spurious constraint |
| `modal_read_linear` | mode-weight read = exact displaced re-read, difference ∝ amplitude² | S3 displaced-body path | wrong mode sign |
| `mass_conserved_by_relocation` | Σm bit-equal; centre of mass shift exact | exact | read via a voxelised `−ρ∇·u` (not bit-exact) |
| `gradiometer_null` | a mode at f = 1/T reads ~0 through the flight weighting | `sinc²(πfT)` | sampling `u` only at shot times |
| `harmonic_response` | steady-state amplitude and phase of a driven damped mode | SDOF closed form | drop damping |
| `rayleigh_wave_speed` | c_R from the solver's own field, or from the Rayleigh function | the secular equation, Vinh & Ogden 2004 Eq. 1 [R]; Hughes & Thorne's c_H/c_S = 0.904 (ν = 0.16), 0.955 (ν = 0.5) [R] | a wrong ν |
| `rayleigh_traction_free` | the field used to move the ground has vanishing free-surface tractions | exact, to 10⁻¹³ (measured) | Harms Eq. 39 *as printed* fails (σ_xz = 1.2, §5.2) |
| `rayleigh_gravity_kinematic` | voxel ground moved by the analytic field | Harms Eqs. 96–100; ≤ 0.2 % (measured 0.06–0.16 %) | surface term only (γ = 1) fails by ~17 % |
| `rayleigh_underground` | voxel ground, observer at depth `h` in the *uncut* half-space, ν = 0.33 | Harms Eq. 97 (equivalently Badurina B.19, Junca Eqs. 11–14); ≤ 0.2 % at `kh` = 0.5, 1, 3 | drop the bulk term (surface sheet only); or use the root-lost reading `(1+q/s)` of B.19 (off by 1.5×) |
| `surface_jump` | vertical acceleration across the surface | jump `4πGρ ξ_z` between Eqs. 96 and 97, 12 figures (measured) | omit the surface mass sheet |
| `rayleigh_shaft_cavity` | the same observer inside a vertical shaft cut out of the voxel ground | the uncut result plus the cavity correction O((ka)²) (Harms §3.3); a *new* result, no published number | — |
| `hughes_thorne_beta` | the Rayleigh read's reduced transfer function, ν = 0.33 | β′ = 0.86, subsurface term alone 0.17, anisotropy ratio 2.2 (Hughes & Thorne Eqs. 2.6–2.7); measured 0.853, 0.178, 2.21 | drop the surface source: 0.17 instead of 0.86 |
| `lamb_problem` | surface response to a point load | Pekeris 1955 [R]: closed forms for the vertical and horizontal surface displacement under a Heaviside point load, `w(t) = (1/r) f(τ)`, `τ = c_S t/r` (his Eqs. 45–48 and 66), singular at the Rayleigh arrival; Lamb 1904 [R, pp. 1–3, 40–43]: qualitative reference (minor tremor from the P arrival, transverse-arrival feature, solitary Rayleigh main shock of constant shape and r^(−1/2) amplitude, vertical larger than horizontal); Richards 1979 [A, not fetched] | only if a dynamic solver is built |
| S13's expansion rows | free bar, clamped bar, bimetal, gradient beam, sphere | S13 note §3.6 | as there |
| `macneal_harder` (optional) | straight (L = 6, 0.2 × 0.1, E = 10⁷, ν = 0.3), curved and twisted cantilevers, unit tip load | MacNeal & Harder 1985 Table 3 [R]: straight beam in-plane shear 0.1081, out-of-plane shear 0.4321, twist 0.03208, extension 3.0×10⁻⁵; curved 0.08734 / 0.5022; twisted 0.005424 / 0.001754; a patch test for solids (Fig. 3, Table 7) | HEX20 fully integrated is the failing element on thin plates (0.02 of exact) |
| `guldberg_circular_cantilever` (optional) | a voxelised *circular* cantilever, the staircase-boundary case | theory; free-end displacement within 1 % and averaged stress under 4 % as the diameter grows (Guldberg et al. 1998 [R]) | a coarse staircase (diameter 2 cells) |

---

## 8. Design and where it goes

**Slice.** The plan already puts "Voxel FE modal analysis: cantilever and plate modes" in S7 (TASK-S7-05).
It is bigger than one task; split it when S7's brief is written:

1. **S7-05a The elastic mesh and operator**: elastic mesh from the body's tree (2:1, hanging nodes, size
   caps); Q1 with incompatible modes, and the selective-integration kind chosen per body for ν ≳ 0.4;
   assembled sparse matrix on the CPU in f64 (matrix-free buys nothing at Q1 on a CPU, §1.4); patch test;
   cantilever and plate with the CPU shift-invert oracle; MacNeal–Harder beams.
2. **S7-05b Multigrid and modes on the GPU**: Galerkin geometric multigrid as the CG preconditioner (local
   smoothing on the graded mesh), finest level matrix-free from the shared element matrix, and LOBPCG, all
   matching the CPU oracle; **measure memory per element first** (§1.4: 2.4 kB in f64 with stored stencils
   against ~100 B if coarse operators are rediscretised) because it fixes the largest body the 6 GB card
   takes; budget visible in the profiler.
3. **S7-05c Modal playback through the read**: mode weights per tower, damping, harmonic forcing, the
   flight's time weighting, node-moment deltas for the S5 walk, `Motion::Modes` generalised from scalar
   amplitudes to computed modes.
4. **S7-05d Quasi-static loads**: gravity sag and moving loads (a person on a floor, a vehicle on a road
   slab), with the summary's slab estimate measured on a real slab model.

S13 (TASK-S13-06) then only adds the thermal load vector. S12 (TASK-S12-02) and S14 (TASK-S14-04/05) use the
**kinematic** ground (a voxel ground patch of ±2λ around the towers moved by plane waves, closed form beyond),
which needs no FE at all and can be built before S7 if S12 comes first. A dynamic wave solver and MPM go to
`plan/IDEAS.md`.

**Costs [D, M].** A 1.6×10⁵-element slab: GMG-PCG on the GPU in the tens of milliseconds by Dick et al.'s
figures [R] scaled (their 12,000 elements at 120 steps/s and 269,000 at 11 steps/s bracket it, two V-cycles a
step, on 2010 hardware); 20 modes by LOBPCG in seconds; mode weights once, then nothing per shot. A building's
10⁷ elements is the build PC's ceiling *only if* the memory per element is far below Wu et al.'s 2.4 kB
(§1.4); with their figure the ceiling is about 2.5×10⁶ elements in f64 on 6 GB, or 14×10⁶ with their 4:1
coarsening and on-the-fly finest level. The read cost of the kinematic ground is the S5 walk's own.

**Precision.** f64 on the CPU for the oracle and the mode weights. On the GPU follow Kronbichler & Ljungkvist
2019 §4.2 [R]: the outer Krylov iteration (residual, operator application) in double precision, the multigrid
V-cycle as a single-precision preconditioner, which kept the accuracy and iteration counts of an all-double
run and was 47–83 % faster on their Poisson tests; they cite Göddeke et al. for the finding that single
precision throughout fails to reduce the true residual. (Spectral-element *wave* codes run in single precision,
Komatitsch et al. 2009, but those are explicit time stepping, not a converged static solve.) The read is
linear in `u`, so a relative error ε in `u` is ε in the signal: 10⁻⁶ for µrad on a 1-rad deformation signal;
the outer residual must be driven to about that, not to Wu et al.'s 10⁻² (topology optimisation does not need
more). Whether the 1660 SUPER's f64 rate (a small fraction of its f32 rate, 1/32 by my recollection [U]) makes the outer operator the bottleneck is a
measurement for S7-05b.

---

## 9. What I could not verify

- **The 3-D incompatible-mode element**: now read in Simo & Armero 1992 (§3.4) and consistent with my
  implementation; Wilson et al. 1973 itself is not fetched. The linear near-incompressible *bending-beam*
  numbers (§1.1) are still mine: no fetched source gives one for the 3-D brick, and Simo & Armero's evidence
  is finite-strain and qualitative.
- The GPU timings are Dick et al.'s and Wu et al.'s, on 2010–11 Fermi hardware, and Kronbichler & Ljungkvist's
  on a P100 for *scalar* Poisson problems; nothing was timed on the 1660 SUPER (no builds, as instructed), and
  no source shows a *vector-valued elastic* matrix-free multigrid with local smoothing on a graded octree. That
  composition is an extrapolation (§1.2).
- The memory per element of a rediscretised (non-Galerkin) hierarchy, the "about 100 B per node" of the first
  version, is not demonstrated in any source I read (§1.4).
- Rayleigh damping, the mode-acceleration correction, the Kirchhoff plate frequency, Timoshenko's shear
  correction, tall-building periods and the 1660 SUPER's f64 ratio are quoted from memory [U]. The
  thermoelastic coupling formula is now read (Biot §2) but its numbers (c_v ≈ c_p) are mine.
- The slab estimate (0.26–0.45 mrad) assumes a simply supported, isolated 6 m slab; continuous slabs on beams
  are stiffer, perhaps 2–5× [U]. Resolve with S7-05d's slab model.
- The residual 0.06–0.16 % in the Rayleigh voxel check is still unexplained (§5.2); it is far inside the
  S14 model error.
- **The sign in Harms Eq. 39** (§5.2): I cannot tell from the page whether it is a misprint or a convention
  (`sin(ωt − k·ϱ)`); the Hassan & Nagy (1998) source he cites was not fetched.
- The Harms 2019 Living Review (154 pages) was not among the fetched files; I used the copy read for the first
  version and have now read, on the rendered page, §3.4 (pp. 29, 44–45) and §3.5. Harms's own treatment of the
  cavity (§3.3) I read only in the 2015 arXiv version.
- Layered ground: Hughes & Thorne's multilayer results (RP modes cancelling, β′ up to 1.4 for SV-dominated
  modes) were read for their statements, not re-derived; Cavendish's closed forms are half-space.
- The statement that every mode aliases into 0–0.25 Hz and the `sinc²` weighting are from the earlier S14 note
  and my own derivation; none of the fetched papers covers AION-10's sampling.

## 10. Further questions (not chased)

1. Does the shaft or stairwell (λ ≈ 25 m at 10 Hz) scatter enough to matter at AION-10's depth? **Now the
   concrete test `rayleigh_shaft_cavity`** (§7): the voxel sum in a cut-out shaft against the uncut result and
   Harms's O((ka)²) estimate.
2. Real slab stiffness and damping at the Beecroft site (a question for George: floor construction near the
   towers).
3. Which ground Poisson's ratio and Rayleigh speed apply at the site (CRUST1.0 or a site survey)? Hughes &
   Thorne find ν ≈ 0.46 in saturated alluvium (their §III) where the half-space γ is 0.94 and the volumetric
   locking of §1.1 matters for any FE of that ground.
4. How much of a structure's quasi-static deflection follows a moving load in the frequency band (walking
   pace 0.5–2 Hz sits on the response's shoulder)?
5. Whether the fill-fraction stiffness of partial cells (§1.3) shifts frequencies by more than 1 % on curved
   bodies at the pitches used; Guldberg's circular cantilever is the case.
6. Memory per element of the GPU hierarchy (Galerkin stored versus rediscretised) and the f64 outer-operator
   rate on the 1660 SUPER: the first S7-05b measurement.
7. Does LOBPCG with a multigrid preconditioner converge on the elastic operator with its rigid-body null
   space as it does on Knyazev's Laplacians?
8. Is the sign in Harms Eq. 39 a misprint? (Check Hassan & Nagy 1998, or the published erratum, if any.)
9. A layered-ground γ (Hughes & Thorne's β′ for RP and RS modes) in the kinematic ground, once a site model
   exists.

---

## Sources

Read in the primary text (sections, pages and equations cited inline). The first-pass items were read for the
first version of the note; the rest after the owner fetched them (25 Nov 2026).

*Newtonian noise and seismology*
- Harms, *Terrestrial gravity fluctuations*, Living Rev. Relativ. 22, 6 (2019), [doi:10.1007/s41114-019-0022-2](https://doi.org/10.1007/s41114-019-0022-2) (PDF via [Springer](https://rd.springer.com/content/pdf/10.1007/s41114-019-0022-2.pdf)); first pass, and this pass re-read from the rendered pages 29, 44–45 and §3.5 (pp. 46–48). The 2015 arXiv version [arXiv:1507.05850](https://arxiv.org/abs/1507.05850) for the cavity section (§3.3). §3.2 Eq. 45; §3.4 Eqs. 38–41, 86–99; §3.5; §3.6 Eqs. 100–101.
- Badurina, Gibson, McCabe, Mitchell, *Ultralight dark matter searches at the sub-Hz frontier with atom multigradiometry*, [arXiv:2211.01854](https://arxiv.org/abs/2211.01854) (v3, 2023). §3.2, App. B (Eqs. B.15–B.25, from the rendered pages 32–33).
- Saulson 1984, [doi:10.1103/PhysRevD.30.732](https://doi.org/10.1103/PhysRevD.30.732): whole paper.
- Hughes & Thorne 1998, [doi:10.1103/PhysRevD.58.122002](https://doi.org/10.1103/PhysRevD.58.122002), [arXiv:gr-qc/9806018](https://arxiv.org/abs/gr-qc/9806018): §I (Eqs. 1.1–1.26), §II, Appendix B; §III–IV and the other appendices not read.
- Junca et al. 2019, [doi:10.1103/physrevd.99.104026](https://doi.org/10.1103/physrevd.99.104026): §III, Eqs. 10–19 (rendered page 4).
- Vetrano & Viceré 2013, [doi:10.1140/epjc/s10052-013-2590-8](https://doi.org/10.1140/epjc/s10052-013-2590-8): §1, §3–4 (Eqs. 25–32).
- Beker et al. 2012, [doi:10.1088/1742-6596/363/1/012004](https://doi.org/10.1088/1742-6596/363/1/012004): §4.
- Harms et al. 2015, [doi:10.1093/gji/ggv090](https://doi.org/10.1093/gji/ggv090): abstract, §1–3.
- Carlton & McCabe 2023, [arXiv:2308.10731](https://arxiv.org/abs/2308.10731): §II (Eqs. 10–13, Fig. 3). Carlton et al. 2024, [arXiv:2412.05379](https://arxiv.org/abs/2412.05379): abstract and keyword search only.
- Komatitsch & Tromp 1999 (abstract, §6.1–6.2), Komatitsch & Martin 2007 (abstract), Virieux 1986 (Rayleigh-wave discussion), Bérenger 1994 (abstract), Komatitsch et al. 2009 and Michéa & Komatitsch 2010 (abstracts), Pekeris 1955 (§1, §6), Vinh & Ogden 2004 (§1–2, Eq. 1), Lamb 1904 (the three-page Proc. R. Soc. abstract, and the full paper, pp. 1–3 and 40–43), Biot 1956 (§2).

*Finite elements, multigrid, GPUs*
- Simo & Armero 1992 (summary, §1, §3.4, §4.2.2, conclusions; [doi:10.1002/nme.1620330705](https://doi.org/10.1002/nme.1620330705), DOI from memory [U]: the file carries none); Kozar, Rukavina & Ibrahimbegović 2018, [doi:10.14256/JCE.2078.2017](https://doi.org/10.14256/JCE.2078.2017) (§2–§3, Tables 1–2); Taylor, Beresford & Wilson 1976; Ibrahimbegovic & Wilson 1991 (introduction); Simo & Rifai 1990 (summary, §1, §4 remarks, §6.1); Hughes 1980 (summary, introduction, start of the theory); Flanagan & Belytschko 1981 (summary, introduction); MacNeal & Harder 1985 (Tables 3, 6–13, Figs. 3–4); Guldberg et al. 1998 (abstract, results).
- Dick, Georgii, Westermann 2011, [doi:10.1016/j.simpat.2010.11.005](https://doi.org/10.1016/j.simpat.2010.11.005) ([TUM PDF](https://www.cs.cit.tum.de/fileadmin/w00cfj/cg/Research/Publications/2011/Real-Time_Multigrid_Finite_Hexahedra_Method/SIMPAT11.pdf)): abstract, §1–3 (first pass), results.
- Wu, Dick, Westermann 2016 (§5, §7, Figs. 7–8, 12, Table 1); Kronbichler & Kormann 2012 (abstract, §3.1, §5.1); Kronbichler & Ljungkvist 2019 (abstract, §1, §4.2); Ljungkvist 2017 (abstract, results); Adams et al. 2004 (abstract, §5); Arbenz et al. 2008 (abstract); Aage et al. 2017 (abstract, Methods); Knyazev 2001 (abstract, §5); Burstedde et al. 2011 (abstract); Parvizian et al. 2007 (§2.3–2.4); Schillinger & Ruess (abstract, §2.2); de Prenter et al. 2019 (abstract).
- Müller & Gross 2004 ([PDF](https://matthias-research.github.io/pages/publications/GI2004.pdf)): Fig. 1, §1–3 (first pass). Georgii & Westermann 2008 (abstract, §1); Pentland & Williams 1989, James & Pai 2002, Barbič & James 2005 (abstracts); Macklin et al. 2016 (§1–4); Smith et al. 2018 (abstract, Fig. 1); MPM: Sulsky et al. 1993 (abstract), Stomakhin et al. 2013 (abstract), Hu et al. 2018 (§7, Fig. 17), Wolper et al. 2019 (abstract).
- Repository notes: `docs/research/s13-heat.md` §3.6, `docs/research/s14-wider-world.md` (E1)–(E5), §2.4–2.5, `docs/research/s5-world-octree.md` §3, `docs/slices/S3.md`.

## Papers still unread, or read only in part

Not fetched, so not read:
- Wilson, Taylor, Doherty & Ghaboussi 1973, *Incompatible displacement models* (book chapter in *Numerical and Computer Methods in Structural Mechanics*, Academic Press, pp. 43–57; [doi:10.1016/B978-0-12-253250-4.50008-7](https://doi.org/10.1016/B978-0-12-253250-4.50008-7) as given in Kozar et al.'s reference list)
- [https://doi.org/10.1785/bssa0880020368](https://doi.org/10.1785/bssa0880020368) Komatitsch & Vilotte 1998, the spectral-element method (SPECFEM's origin)
- [https://doi.org/10.1785/bssa0690040947](https://doi.org/10.1785/bssa0690040947) Richards 1979, Lamb's problem for a point source
- Hassan & Nagy 1998 (cited by Harms for Eq. 39; not on the list)

Fetched, opened only in part (the part is named in the Sources):
- [https://doi.org/10.1007/s41114-019-0022-2](https://doi.org/10.1007/s41114-019-0022-2) Harms 2019: §3.2–3.5 of 154 pages (and §3.3 from the 2015 version)
- [https://doi.org/10.1103/PhysRevD.58.122002](https://doi.org/10.1103/PhysRevD.58.122002) Hughes & Thorne 1998: §I, §II, Appendix B of 27 pages
- [https://doi.org/10.1103/PhysRevD.78.042003](https://doi.org/10.1103/PhysRevD.78.042003) Dimopoulos et al. 2008, GR effects in atom interferometry: fetched, not opened (it concerns the interferometer's phase shift, not the elastic solid)
- [https://doi.org/10.1017/cbo9780511755446](https://doi.org/10.1017/cbo9780511755446) Bonet & Wood 2008: the fetched file is the 8-page front matter
- [https://doi.org/10.48550/arXiv.2412.05379](https://doi.org/10.48550/arXiv.2412.05379) atmospheric GGN (Carlton et al. 2024): abstract and a keyword search only
- [https://doi.org/10.1145/3322813](https://doi.org/10.1145/3322813) Kronbichler & Ljungkvist 2019; [https://doi.org/10.1145/2994258.2994272](https://doi.org/10.1145/2994258.2994272) XPBD; [https://doi.org/10.1145/3306346.3322949](https://doi.org/10.1145/3306346.3322949) CD-MPM; and the other graphics and MPM papers: the parts named in the Sources, not their numerical sections

## Fetched files not opened, and why

The first fetched folder holds 82 PDFs, `fluids.zip` and a `__MACOSX` folder of resource forks; a second folder of three PDFs (Simo & Armero, Lamb full, Kozar et al.) was read as described above.
- **Not relevant to this note (they belong to the fluids and free-surface research):** `harlow1965`, `chorin1968`,
  `hirt1981`, `bell1989`, `ghia1982`, `hortmann1990`, `fusegi1991`, `devahldavis1983`, `gray1976`,
  `martin1952`, `prosperetti1981`, `botella1998`, `francois2006`, `popinet2003/2009/2015/2018`,
  `fuster2018`, `weymouth2010`, `ng2009`, `rohde2005`, `gibou2002`, `enright2002`, `losasso2004/2006`,
  `setaluri2014`, `batty2007`, `bender2015` (divergence-free SPH), `schornbaum2016` (lattice Boltzmann on
  octrees), `10.1007@s10955-005-8879-8` (lattice Boltzmann free surface). I read only each file's first lines
  to identify it. `fluids.zip` holds some of these (`gray1976.pdf`, ...) and was not extracted.
- **Duplicates, not read twice:** `deprenter2019-2` (same text as `deprenter2019`), `schillinger2014-2`,
  `hu2018-2`.
- **Proceedings volume `!_WSCG2012-Communications-1`** (212 pages): its contents page lists Warburton &
  Maddock, *Creating animatable non-conforming hexahedral finite element facial soft-tissue models for GPU
  simulation* (p. 317), which would bear on non-conforming hexahedra, but that paper's text is not in the
  volume (I searched for it); no other paper in it concerns this note.
- **`dimopoulos2008`** and **`frontmatter`** (Bonet & Wood front matter): see above.
- `__MACOSX/`: resource-fork files, not papers.

Copies already held from the first pass (Harms 2019 and its 2015 arXiv version, Badurina, Carlton & McCabe) were
used for the shaft question alongside the fetched files; the fetched `2308.10731v1.pdf` is the same Carlton &
McCabe paper.

## Files

`elastic-solids/` beside this note: `rayleigh.py` (+ `.out.txt`), `cantilever.py` (+ `.out.txt`),
`estimates.py` (+ `.out.txt`) and, added after the primary sources were read, `shaft_check.py` (+ `.out.txt`:
the Eq. 97 / B.19 / Junca / Hughes–Thorne / surface-jump / Eq. 39 checks of §5.2–5.3) and `nu_locking.py` (+
`.out.txt`: §1.1's near-incompressible table; it imports `cantilever.py` and `rayleigh.py`, so run both new
scripts from their own directory). Python 3.14 with NumPy and SciPy: the first three in a scratch venv
(NumPy 2.5, SciPy 1.18), the two new ones with the system interpreter (NumPy 2.4.3). Each script states its
pass criteria at the top, written before it was run.
