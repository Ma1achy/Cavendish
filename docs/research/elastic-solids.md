# Deformable solids: voxel finite elements for vibration, strain, thermal expansion and ground motion

Research note for the structural-dynamics track of S7 (`plan/VALIDATION-AND-RESEARCH.md` §4–5,
`plan/tasks.yaml` TASK-S7-05) and its users in S12 (seismic noise), S13 (thermal expansion, TASK-S13-06)
and S14 (earthquakes, TASK-S14-04/05). Written 25 Nov 2026.

**Tags.** **[R]** read in the primary source (section or equation given); **[A]** abstract or
bibliographic record only; **[M]** measured here (scripts in `elastic-solids/`, outputs beside them);
**[D]** derived here; **[U]** from memory, not checked: treat as a claim to verify.

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
   kept only for near-incompressible materials.
2. **Three solves on one operator, all matrix-free with geometric multigrid**: quasi-static `K u = f`
   (thermal strain, loads; PCG to a stated tolerance, not a fixed number of V-cycles as real-time codes do);
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
at 0.25, 0.799 at 0.30, 0.827 at 0.33** [M]. The form `Γ_zz = 2πGρ γ k ξ_z e^{−kh}` above ground holds
(derivative of Harms Eq. 100 [D]). Underground (a shaft) it does not: see §5.3 and the open question there.

**What could not be verified:** most FE and seismology papers were read only as records (list at the end);
the GPU cost figures are from Dick et al. and from extrapolation, not measured on the 1660 SUPER; the slab
estimate idealises the slab as simply supported.

---

## 1. Voxel and octree hexahedral FE

### 1.1 Elements and locking

A voxel is a trilinear hexahedron (Q1, 8 nodes, 24 dofs). All cubic elements of one size share one element
matrix: `K_e ∝ E·s` and `M_e ∝ ρ·s³`, so a matrix-free code stores a material index and a level per element
and nothing else, which is why voxel FE scales (Arbenz et al. 2008 [A]; Dick et al. 2011, §3 [R]).

Q1 with full 2×2×2 integration *shear-locks* in bending: it cannot represent pure bending without spurious
shear strain. Remedies in the literature:

| Remedy | What it fixes | Source |
|---|---|---|
| Incompatible modes (Q1 + 3 bubble modes per component, condensed) | bending (shear) locking; exact for pure bending on a rectangular element | Wilson et al. 1973 [U, book chapter, no DOI]; Taylor, Beresford & Wilson 1976 (patch-test correction) [A]; Simo & Rifai 1990 (as enhanced assumed strain) [A] |
| Selective reduced integration / B-bar | volumetric locking as ν → 0.5 | Hughes 1980 [A] |
| One-point integration + hourglass control | both, at the cost of hourglass modes to stabilise | Flanagan & Belytschko 1981 [A] |
| Several cells through every member | everything, at cost | — |

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

### 1.2 Octree meshes and hanging nodes

The body's tree merges uniform regions into large leaves (S3). One element per merged leaf would be far too
coarse for bending (a 16-cell merged block is a single Q1 element), so the **elastic mesh is derived from the
tree, not equal to it**: split leaves until (i) every element is at most a set size, (ii) the mesh is 2:1
graded across faces and edges, (iii) any member has the cells through its thickness that §1.1 needs. A
node on the face or edge of a coarser neighbour ("hanging") is constrained to the coarse element's trilinear
interpolation, which keeps the displacement continuous; this is the standard treatment in octree FE codes
(p4est's forests of octrees, Burstedde et al. 2011 [A]; matrix-free cell-based operators with hanging-node
constraints, Kronbichler & Kormann 2012 [A]; on GPUs, Ljungkvist 2017 and Kronbichler & Ljungkvist 2019 [A]).
The S9 fluid octree already enforces 2:1 balance; the elastic mesh should use the same balancing code.
Dick et al. build an octree only for the multigrid hierarchy over a *uniform* finest grid, with partly
filled coarse cells [R, §3.3]; they do not use hanging nodes.

The gravity read keeps using the **mass tree** (S3, S5): the FE mesh only supplies `u` at its nodes, and
each mass leaf moves by `u` interpolated at its centroid (§6).

### 1.3 Curved and partial cells: the finite cell method

S3's leaves carry fill fractions (sample counts) and S5 has the exact skin. The finite cell method
(Parvizian, Düster & Rank 2007 [A]; review Schillinger & Ruess 2015 [A]) integrates stiffness only over the
material part of a cut cell, with a small fictitious stiffness α (typ. 10⁻⁶–10⁻¹⁰ [U]) in the void part; very
small cut fractions make the system ill-conditioned, which de Prenter et al. 2019 cure with a geometric
multigrid with Schwarz-type smoothers that gives cut-independent convergence [A].

For Cavendish: **mass stays exact** (the tree's mass, relocated); only stiffness is approximated. The first
version scales each partial cell's stiffness by its fill fraction (one-point finite cell) with a floor on the
fraction; what this costs in a curved body's frequencies is a measurement in the slice (§7, a voxel sphere's
breathing mode against Lamb's closed form, or a cylinder's bending against Euler–Bernoulli at several
pitches). Staircase geometry converges in the voxel-FE literature with refinement (Guldberg et al. 1998,
[A]: the title is the claim; details not read).

### 1.4 Solvers and cost

| Solver | Role | Evidence |
|---|---|---|
| Matrix-free PCG + geometric multigrid (V-cycle, Jacobi/Chebyshev smoothing) | quasi-static `K u = f` | GPU: Dick et al. 2011: 269,000 hexahedra at 11 steps/s and 12,000 at 120 steps/s on a Fermi GPU with **two** V-cycles a step, 27× one CPU core [R, abstract and §1]; Wu, Dick & Westermann 2016, voxel topology optimisation on one GPU at 621×488×1000 [A, search snippet]. CPU clusters: 5×10⁸ dof (Adams et al. 2004 [A]); µFE bone with ~10⁹ dof (Arbenz et al. 2008 [A, search snippet]); ~10⁹ voxels (Aage et al. 2017 [A]) |
| LOBPCG with the multigrid preconditioner | lowest m modes | Knyazev 2001 [A] |
| Shift-invert Lanczos (ARPACK) with a sparse direct factorisation | CPU oracle for small bodies | measured: 6 modes of a 19,440-dof beam in 1.4 s on one core [M]; direct fill grows superlinearly in 3-D, so stop at ~10⁵ dof |
| Explicit central differences / Newmark | wave propagation, nonlinear transients | not needed for the default (§2, §5) |

A real-time code's "two V-cycles a step" is an *unconverged* solve; Cavendish solves to a residual
tolerance and records it (a frame budget may preview, then refine, as §5.1 of the plan does).

**Scale.** At 10⁹–10¹¹ world cells the FE never covers the world. It covers the bodies flagged elastic,
inside the towers' bubble: a floor slab 10×10×0.2 m at 5 cm is 1.6×10⁵ elements (~5×10⁵ dof); a
two-storey building's walls and floors at 5 cm, ~10⁷ elements [D]. Matrix-free storage is a few vectors of
3 floats per node plus the hierarchy, roughly 100 B per node [D], so ~10⁷ nodes fit in the 1660 SUPER's
6 GB. Terrain and the far ground move by analytic fields (§5), never by FE.

---

## 2. Vibration: modes, damping, forcing, and what the gradiometer sees

**Modes.** `K φ = ω² M φ` for the lowest m modes (m ≤ 64 matches `Motion::Modes`'s cap). Free bodies have six
zero modes; they are the rigid motion that S7 owns, and are removed (§3).

**Modal superposition.** With mass-normalised modes, `u(t) = Σ q_m(t) φ_m` and each `q_m` is a damped
oscillator with a closed-form response; real-time vibration by modal synthesis is long established in
graphics (Pentland & Williams 1989 [A]; James & Pai 2002 [A]; reduced nonlinear: Barbič & James 2005 [A]).
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
  Westermann 2008 [A, record not found by the search; title only]).
- **For Cavendish** the large rotations belong to S7's rigid bodies. The clean split is the *floating frame*:
  rigid motion from S7, elastic modes in the body's own frame, which is corotation at body level and exactly
  how per-object volumes already work (§5.2 of the plan). Within a body, strains of 10⁻⁴ give an error of
  10⁻⁴ relative in `u`: below the read's tolerance.
- **Hyperelastic models** (St Venant–Kirchhoff, Neo-Hookean; Bonet & Wood 2008 [A]; Smith, de Goes & Kim
  2018 for a robust Neo-Hookean [A, abstract read]) are for rubber, flesh and soft bodies at large strain.
  None of these move enough mass at µrad to need more than linear elasticity in a small-strain frame;
  a soft body that squashes visibly is an animation, and its mass is relocated exactly whatever model moved
  it.
- **XPBD** (Macklin, Müller & Chentanez 2016 [A]) makes stiffness independent of time step and iteration
  count in the limit, but the solution depends on the iterations actually run [U]; fine for visuals, labelled.
- **MPM** (Sulsky, Chen & Schreyer [A, report record]; Stomakhin et al. 2013 [A]; MLS-MPM, Hu et al. 2018
  [A, abstract read]; fracture: Wolper et al. 2019 [A]) is the one-solver option for snow, sand, rubble and
  fracture. The plan already keeps it as the granular layer idea; it is not the tool for small-amplitude
  structural response, where modal FE is exact and cheap (as the plan's §4 says).

---

## 4. Thermal expansion: coupling to S13's heat

Linear thermoelasticity adds the thermal strain `ε_th = αΔT I`; the load is `f = ∫ Bᵀ D ε_th dV`, solved
quasi-statically because elastic wave times (ms) are far below thermal times (the S13 note, §3.6).

**One-way coupling is enough.** The two-way term (heating by compression) is governed by the coupling number
`δ = (3K)² α² T₀ / ((λ+2µ) ρ c)` (Biot 1956 [A]; the formula is quoted from memory [U] and evaluated
here): **steel 1.1×10⁻², aluminium 2.6×10⁻², concrete 1.0×10⁻³, granite 1.5×10⁻³** [M, (c)]. It enters
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
whenever analytic fields exist: they are "faster by orders of magnitude", and no Newtonian-noise simulation
so far needed the dynamic kind [R, §3.5]. Dynamic simulations used COMSOL (Beker et al. [A]) and
specfem3d, the spectral-element code (Komatitsch & Vilotte 1998; Komatitsch & Tromp 1999 [A]), with Eq. 45
added for gravity (Harms et al. 2015 [A]). Staggered-grid velocity–stress finite differences (Virieux 1986
[A]) and the convolutional PML (Bérenger 1994; Komatitsch & Martin 2007 [A]) are the other standard tools;
GPU ports exist for both (Komatitsch et al. 2009; Michéa & Komatitsch 2010 [A]).

For Cavendish this means: **the voxel ground near the towers is moved by a superposition of analytic plane
waves** (Rayleigh, P, SV; S12's microseism spectrum and S14's teleseismic trains), read by the displaced-body
path, and **the far field is the closed form**. A dynamic solver is needed only for heterogeneous ground,
topography, or cavities comparable to the wavelength (the stairwell at 10 Hz, λ ≈ 25 m, is a case to check
later; Badurina et al. neglect scattering from a 5 m shaft for λ ≥ 40 m [R, §3.2]).

### 5.2 The Rayleigh-wave closed form, checked

Above a homogeneous half-space, a fundamental Rayleigh wave with vertical surface displacement `ξ_z` gives
`δa = 2πGρ γ(ν) ξ_z e^{−kh} (i cos φ, i sin φ, −1)` (Harms Eq. 100 [R]) with
`γ(ν) = k(1−ζ)/(q_zP − kζ)`, `ζ = √(q_zP/q_zS)`, `q_z{P,S} = √(k² − k_{P,S}²)` (Eqs. 38, 39, 98, 99 [R]).
γ = 1 would be the surface term alone; the compression inside the ground partly cancels it.

**Measured [M]** (`rayleigh.py`):
- The displacement field of Eq. 39 is traction-free at the Rayleigh speed to 10⁻¹³ (after restoring a minus
  sign on the horizontal component that the PDF extraction lost; derived from potentials). `c_R/c_S` =
  0.8931–0.9541 for ν = 0.1–0.49.
- γ(ν): 0.631 (0.10), 0.711 (0.20), 0.754 (0.25), 0.799 (0.30), 0.827 (0.33), 0.896 (0.40), 0.989 (0.49).
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

### 5.3 Vertical gradiometers below ground: an unresolved discrepancy

AION-10 sits in a stairwell, below ground level. Harms gives the underground potential as
`2πGρ A [−2e^{−h q_zP} + (1+ζ) e^{−hk}]` (Eq. 97 [R], cavity neglected). Badurina, Gibson, McCabe & Mitchell
(AION-REPORT/2022-3) use the same two-exponential structure, `A e^{−q ω z/c_H} + B e^{−ω z/c_H}`, but their
coefficients (Appendix B, Eqs. B.21–B.22 [R]) give a ratio B/A of `−(1+q/s)/(2q)`, where Harms's gives
`−(1+ζ)/2` with `ζ = √(q/s)` (their q, s are Harms's `q_zP/k`, `q_zS/k`). For their LIGO-Livingston numbers
(q = 0.88, s = 0.36) these differ by a factor of about 1.5. I have not found which is right; it may be a
normalisation I have misread from the extracted PDF. **How to resolve:** the same kinematic voxel sum
evaluated inside a vertical shaft cut out of the voxel ground, which is exactly the geometry Cavendish will
simulate (and includes the cavity term both papers drop).

The far-field sizes in `s14-wider-world.md` §2.4–2.5 used `Γ = γ2πGρkξ_z` with γ = 0.7; replace γ by Eq. 99
(+8 to +18 % for ν = 0.25–0.33) and, below ground, by whichever underground form the check above confirms.

### 5.4 Older Newtonian-noise literature

Saulson 1984 [A] estimated seismic gravity-gradient noise with a reduced transfer function β = 1/√3 ≈ 0.58;
Hughes & Thorne 1998 [R, abstract and §I] modelled layered-ground Rayleigh and Love modes and found β ≈
0.35–0.6 at quiet times and 0.15–1.4 at noisy times. β there is a test-mass transfer function for a
horizontal laser-interferometer arm, **not** the same quantity as Harms's γ. Love waves (SH) produce no
gravity perturbation in a half-space [R, Harms §3.4.1]. For atom interferometers: Vetrano & Viceré 2013 [A];
Junca et al. 2019 (MIGA, horizontal) [A]; vertical multigradiometers: Badurina et al. 2023 [R, §3.2];
atmospheric GGN for vertical instruments, arXiv:2412.05379 [A]; Carlton et al., arXiv:2308.10731 [A].

### 5.5 Earthquakes near the site

A nearby earthquake's strong motion is mm–cm of ground displacement at 0.1–10 Hz [U]; through the same
kinematic path its Newtonian part is sized like the Rayleigh case, and the instrument shaking it causes is
far larger and out of the forward model (as S14 records). The prompt elasto-gravity signal is S14's closed
form (Harms et al. 2015 [A]). Nothing here needs a dynamic FE solve.

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
| `cantilever_frequency` | f1 of a clamped voxel beam, 1–8 cells thick | Euler–Bernoulli with the shear correction; measured here: incompatible modes +0.28 % at 1 cell | plain Q1 at 1 cell is +21 % |
| `cantilever_tip_deflection` | static tip load | Euler–Bernoulli + Timoshenko shear term [U] | Q1 at 1 cell |
| `plate_modes` | simply supported square plate, `f₁₁ = (π/a²)√(D/ρh)` [U, Kirchhoff] | closed form | swap a boundary condition |
| `refined_equals_uniform` | an octree-graded mesh refined everywhere equals the uniform mesh | exact | — |
| `free_body_has_six_zero_modes` | rigid modes found and removed; their read equals S7's rigid read | exact | add a spurious constraint |
| `modal_read_linear` | mode-weight read = exact displaced re-read, difference ∝ amplitude² | S3 displaced-body path | wrong mode sign |
| `mass_conserved_by_relocation` | Σm bit-equal; centre of mass shift exact | exact | read via a voxelised `−ρ∇·u` (not bit-exact) |
| `gradiometer_null` | a mode at f = 1/T reads ~0 through the flight weighting | `sinc²(πfT)` | sampling `u` only at shot times |
| `harmonic_response` | steady-state amplitude and phase of a driven damped mode | SDOF closed form | drop damping |
| `rayleigh_wave_speed` | c_R from the solver's own field, or from the Rayleigh function | Vinh & Ogden 2004 closed form [A] | — |
| `rayleigh_gravity_kinematic` | voxel ground moved by the analytic field | Harms Eqs. 96–100; ≤ 0.2 % (measured 0.06–0.16 %) | surface term only (γ = 1) fails by ~17 % |
| `rayleigh_underground` | shaft geometry | resolves §5.3 | — |
| `lamb_problem` | surface response to a point load | Lamb 1904; Pekeris 1955; Richards 1979 [A] | only if a dynamic solver is built |
| S13's expansion rows | free bar, clamped bar, bimetal, gradient beam, sphere | S13 note §3.6 | as there |
| `macneal_harder` (optional) | the standard element-accuracy problems | MacNeal & Harder 1985 [A] | — |

---

## 8. Design and where it goes

**Slice.** The plan already puts "Voxel FE modal analysis: cantilever and plate modes" in S7 (TASK-S7-05).
It is bigger than one task; split it when S7's brief is written:

1. **S7-05a The elastic mesh and operator**: elastic mesh from the body's tree (2:1, hanging nodes, size
   caps); Q1 with incompatible modes; matrix-free apply on the CPU in f64; patch test; cantilever and plate
   with the CPU shift-invert oracle.
2. **S7-05b Multigrid and modes on the GPU**: geometric multigrid PCG and LOBPCG, matching the CPU oracle;
   budget visible in the profiler.
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
figures [R] scaled; 20 modes by LOBPCG in seconds; mode weights once, then nothing per shot. A building's
10⁷ elements is the build PC's ceiling. The read cost of the kinematic ground is the S5 walk's own.

**Precision.** f64 on the CPU for the oracle and the mode weights; f32 with f64 accumulation on the GPU for
the operator, verified against the CPU. The read is linear in `u`, so a relative error ε in `u` is ε in the
signal: 10⁻⁶ for µrad on a 1-rad deformation signal.

---

## 9. What I could not verify

- Most FE and seismology sources were read only as bibliographic records or abstracts: the incompatible-mode
  papers, Hughes's B-bar, Flanagan–Belytschko, the finite cell method, p4est, Kronbichler et al., LOBPCG,
  Komatitsch et al., Virieux, the PML papers, MPM, XPBD and the modal-synthesis papers. Their roles here are
  standard ones, and the measured cantilever and Rayleigh results do not depend on them.
- The GPU timings are Dick et al.'s, on a 2010–11 Fermi GPU with an unconverged solve; nothing was timed on
  the 1660 SUPER (no builds, as instructed).
- The thermoelastic coupling formula, Rayleigh damping, the mode-acceleration correction, the Kirchhoff plate
  frequency, Timoshenko's shear correction and tall-building periods are quoted from memory [U].
- The slab estimate (0.26–0.45 mrad) assumes a simply supported, isolated 6 m slab; continuous slabs on beams
  are stiffer, perhaps 2–5× [U]. Resolve with S7-05d's slab model.
- The residual 0.06–0.16 % in the Rayleigh voxel check is unexplained (§5.2).
- The Harms vs Badurina underground coefficients (§5.3) are unresolved.

## 10. Further questions (not chased)

1. Does the shaft or stairwell (λ ≈ 25 m at 10 Hz) scatter enough to matter at AION-10's depth? A kinematic
   field plus a cavity correction (Harms §3.3) or a small dynamic run.
2. Real slab stiffness and damping at the Beecroft site (a question for George: floor construction near the
   towers).
3. Which ground Poisson's ratio and Rayleigh speed apply at the site (CRUST1.0 or a site survey)?
4. How much of a structure's quasi-static deflection follows a moving load in the frequency band (walking
   pace 0.5–2 Hz sits on the response's shoulder)?
5. Whether the fill-fraction stiffness of partial cells (§1.3) shifts frequencies by more than 1 % on curved
   bodies at the pitches used.

---

## Sources

Read (sections cited inline):
- Harms, *Terrestrial gravity fluctuations*, Living Rev. Relativ. 22, 6 (2019), [doi:10.1007/s41114-019-0022-2](https://doi.org/10.1007/s41114-019-0022-2) (PDF via [Springer](https://rd.springer.com/content/pdf/10.1007/s41114-019-0022-2.pdf); earlier version [arXiv:1507.05850](https://arxiv.org/abs/1507.05850)). §3.2 Eq. 45; §3.4 Eqs. 86–99; §3.5; §3.6 Eqs. 100–101; §6.5.
- Badurina, Gibson, McCabe, Mitchell, *Ultralight dark matter searches at the sub-Hz frontier with atom multigradiometry*, [arXiv:2211.01854](https://arxiv.org/abs/2211.01854) (v3, 2023). §3.2, App. B.
- Hughes & Thorne, *Seismic gravity-gradient noise in interferometric gravitational-wave detectors*, Phys. Rev. D 58, 122002 (1998), [doi:10.1103/PhysRevD.58.122002](https://doi.org/10.1103/PhysRevD.58.122002), [arXiv:gr-qc/9806018](https://arxiv.org/abs/gr-qc/9806018). Abstract and §I only.
- Dick, Georgii, Westermann, *A real-time multigrid finite hexahedra method for elasticity simulation using CUDA*, Simul. Model. Pract. Theory 19, 801 (2011), [doi:10.1016/j.simpat.2010.11.005](https://doi.org/10.1016/j.simpat.2010.11.005) ([TUM PDF](https://www.cs.cit.tum.de/fileadmin/w00cfj/cg/Research/Publications/2011/Real-Time_Multigrid_Finite_Hexahedra_Method/SIMPAT11.pdf)). Abstract, §1–3.
- Müller & Gross, *Interactive virtual materials*, Graphics Interface 2004 ([PDF](https://matthias-research.github.io/pages/publications/GI2004.pdf)). Fig. 1, §1–3.
- Repository notes: `docs/research/s13-heat.md` §3.6, `docs/research/s14-wider-world.md` (E1)–(E5), §2.4–2.5,
  `docs/research/s5-world-octree.md` §3, `docs/slices/S3.md`.

Records or abstracts only: every other source; see the list below.

## Papers I could not read

- Wilson, Taylor, Doherty & Ghaboussi 1973, *Incompatible displacement models* (book chapter, no DOI found)
- [https://doi.org/10.1002/nme.1620100602](https://doi.org/10.1002/nme.1620100602) Taylor, Beresford & Wilson 1976, a non-conforming element for stress analysis
- [https://doi.org/10.1002/nme.1620290802](https://doi.org/10.1002/nme.1620290802) Simo & Rifai 1990, enhanced assumed strain and incompatible modes
- [https://doi.org/10.1002/nme.1620150914](https://doi.org/10.1002/nme.1620150914) Hughes 1980, selective integration (B-bar)
- [https://doi.org/10.1002/nme.1620170504](https://doi.org/10.1002/nme.1620170504) Flanagan & Belytschko 1981, hourglass control
- [https://doi.org/10.1007/s00466-007-0173-y](https://doi.org/10.1007/s00466-007-0173-y) Parvizian, Düster & Rank 2007, finite cell method
- [https://doi.org/10.1007/s11831-014-9115-y](https://doi.org/10.1007/s11831-014-9115-y) Schillinger & Ruess 2015, finite cell review
- [https://doi.org/10.1007/s00466-019-01796-y](https://doi.org/10.1007/s00466-019-01796-y) de Prenter et al. 2019, multigrid for immersed FE ([arXiv:1903.10977](https://arxiv.org/abs/1903.10977), abstract read)
- [https://doi.org/10.1137/100791634](https://doi.org/10.1137/100791634) Burstedde, Wilcox & Ghattas 2011, p4est
- [https://doi.org/10.1016/j.compfluid.2012.04.012](https://doi.org/10.1016/j.compfluid.2012.04.012) Kronbichler & Kormann 2012, matrix-free cell-based operators
- [https://doi.org/10.1145/3322813](https://doi.org/10.1145/3322813) Kronbichler & Ljungkvist 2019, matrix-free multigrid on GPUs
- [https://doi.org/10.22360/springsim.2017.hpc.004](https://doi.org/10.22360/springsim.2017.hpc.004) Ljungkvist 2017, matrix-free FE on GPUs with adaptive meshes
- [https://doi.org/10.1002/nme.2101](https://doi.org/10.1002/nme.2101) Arbenz et al. 2008, matrix-free µFE of bone
- [https://doi.org/10.1109/SC.2004.62](https://doi.org/10.1109/SC.2004.62) Adams et al. 2004, half a billion dof
- [https://doi.org/10.1038/nature23911](https://doi.org/10.1038/nature23911) Aage et al. 2017, giga-voxel morphogenesis
- [https://doi.org/10.1109/TVCG.2015.2502588](https://doi.org/10.1109/TVCG.2015.2502588) Wu, Dick & Westermann 2016, GPU topology optimisation
- [https://doi.org/10.1115/1.2798314](https://doi.org/10.1115/1.2798314) Guldberg, Hollister & Charras 1998, accuracy of voxel FE
- [https://doi.org/10.1016/0168-874X(85)90003-4](https://doi.org/10.1016/0168-874X(85)90003-4) MacNeal & Harder 1985, standard test problems
- [https://doi.org/10.1137/S1064827500366124](https://doi.org/10.1137/S1064827500366124) Knyazev 2001, LOBPCG
- [https://doi.org/10.1145/74334.74355](https://doi.org/10.1145/74334.74355) Pentland & Williams 1989, modal dynamics
- [https://doi.org/10.1145/566570.566621](https://doi.org/10.1145/566570.566621) James & Pai 2002, DyRT
- [https://doi.org/10.1145/1186822.1073300](https://doi.org/10.1145/1186822.1073300) Barbič & James 2005, reduced StVK
- [https://doi.org/10.1145/3180491](https://doi.org/10.1145/3180491) Smith, de Goes & Kim 2018, stable Neo-Hookean (abstract read)
- [https://doi.org/10.1017/cbo9780511755446](https://doi.org/10.1017/cbo9780511755446) Bonet & Wood 2008, nonlinear continuum mechanics
- [https://doi.org/10.1145/2994258.2994272](https://doi.org/10.1145/2994258.2994272) Macklin, Müller & Chentanez 2016, XPBD
- [https://doi.org/10.2172/10177049](https://doi.org/10.2172/10177049) Sulsky, Chen & Schreyer, particle method (MPM)
- [https://doi.org/10.1145/2461912.2461948](https://doi.org/10.1145/2461912.2461948) Stomakhin et al. 2013, MPM snow
- [https://doi.org/10.1145/3197517.3201293](https://doi.org/10.1145/3197517.3201293) Hu et al. 2018, MLS-MPM (abstract read)
- [https://doi.org/10.1145/3306346.3322949](https://doi.org/10.1145/3306346.3322949) Wolper et al. 2019, CD-MPM fracture
- [https://doi.org/10.1063/1.1722351](https://doi.org/10.1063/1.1722351) Biot 1956, thermoelasticity
- [https://doi.org/10.1785/bssa0880020368](https://doi.org/10.1785/bssa0880020368) Komatitsch & Vilotte 1998, spectral elements
- [https://doi.org/10.1046/j.1365-246x.1999.00967.x](https://doi.org/10.1046/j.1365-246x.1999.00967.x) Komatitsch & Tromp 1999, spectral elements in 3-D
- [https://doi.org/10.1016/j.jpdc.2009.01.006](https://doi.org/10.1016/j.jpdc.2009.01.006) Komatitsch, Michéa & Erlebacher 2009, SEM on GPUs
- [https://doi.org/10.1111/j.1365-246X.2010.04616.x](https://doi.org/10.1111/j.1365-246X.2010.04616.x) Michéa & Komatitsch 2010, FD on GPUs
- [https://doi.org/10.1190/1.1442147](https://doi.org/10.1190/1.1442147) Virieux 1986, staggered-grid P-SV
- [https://doi.org/10.1006/jcph.1994.1159](https://doi.org/10.1006/jcph.1994.1159) Bérenger 1994, PML
- [https://doi.org/10.1190/1.2757586](https://doi.org/10.1190/1.2757586) Komatitsch & Martin 2007, C-PML
- [https://doi.org/10.1098/rspl.1903.0029](https://doi.org/10.1098/rspl.1903.0029) Lamb 1904, tremors over an elastic solid
- [https://doi.org/10.1073/pnas.41.7.469](https://doi.org/10.1073/pnas.41.7.469) Pekeris 1955, the seismic surface pulse
- [https://doi.org/10.1785/bssa0690040947](https://doi.org/10.1785/bssa0690040947) Richards 1979, Lamb's problem for a point source
- [https://doi.org/10.1016/j.wavemoti.2003.08.004](https://doi.org/10.1016/j.wavemoti.2003.08.004) Vinh & Ogden 2004, Rayleigh wave speed formulas
- [https://doi.org/10.1103/PhysRevD.30.732](https://doi.org/10.1103/PhysRevD.30.732) Saulson 1984, terrestrial gravitational noise
- [https://doi.org/10.1093/gji/ggv090](https://doi.org/10.1093/gji/ggv090) Harms et al. 2015, transient gravity from earthquake rupture
- [https://doi.org/10.1088/1742-6596/363/1/012004](https://doi.org/10.1088/1742-6596/363/1/012004) Beker et al. 2012, Newtonian noise and ambient ground motion
- [https://doi.org/10.1140/epjc/s10052-013-2590-8](https://doi.org/10.1140/epjc/s10052-013-2590-8) Vetrano & Viceré 2013, Newtonian noise in atom interferometers
- [https://doi.org/10.1103/physrevd.99.104026](https://doi.org/10.1103/physrevd.99.104026) Junca et al. 2019, MIGA gravity fluctuations
- [https://doi.org/10.1103/PhysRevD.78.042003](https://doi.org/10.1103/PhysRevD.78.042003) Dimopoulos et al. 2008, GR effects in atom interferometry (atom-interferometer gravity couplings)
- [https://doi.org/10.48550/arXiv.2412.05379](https://doi.org/10.48550/arXiv.2412.05379) atmospheric gravity-gradient noise for vertical atom interferometers
- [https://doi.org/10.48550/arXiv.2308.10731](https://doi.org/10.48550/arXiv.2308.10731) Carlton et al., gravity-gradient noise mitigation for ULDM searches
- Georgii & Westermann 2008, *Corotated finite elements made fast and stable* (VRIPHYS; no DOI found)

## Files

`elastic-solids/` beside this note: `rayleigh.py` (+ `.out.txt`), `cantilever.py` (+ `.out.txt`),
`estimates.py` (+ `.out.txt`). Python 3.14 with NumPy 2.5 and SciPy 1.18 in a scratch venv; each script
states its pass criteria at the top, written before it was run.
