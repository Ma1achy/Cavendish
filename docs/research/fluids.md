# Fluids for S8–S10: one solver for air and water

*Research note for slices S8 (fluids: air), S9 (the adaptive grid) and S10 (water) of `plan/REBUILD.md`
§9 and §9.1a. GPU-free: no shader ran. Written 25 Nov 2026, revised the same day against the primary
sources. Scripts and raw output: `fluids/` (`python fluids/cavity.py RA N`,
`python fluids/signals.py`, `python fluids/compare_refs.py`; numpy only).*

## Changed after reading the primary sources (25 Nov 2026)

The owner fetched 31 of the papers the first version could not open. I read them (page, section, table
and equation numbers are given where a claim now rests on one). **No recommendation reverses: one MAC
projection solver, flux-form scalars, VOF for water, Losasso-form T-junctions first with a measured
fallback, LBM rejected.** Four things moved, and several numbers and claims were wrong.

**Recommendations that moved.**
1. **The S8 grid requirements and the reference.** Use **Hortmann et al. 1990** as the cavity
   reference (accurate to 0.01 % by its own grids of up to 640², Table IV p. 202), not de Vahl Davis,
   whose own error estimate at Ra 10⁶ is **1 %** (his Table VII, p. 261). The pass criterion
   becomes: the finest grid within 1 %, and a **three-grid, observed-order** extrapolation within
   0.3 % of Hortmann (§10). The old two-grid, fixed-order extrapolation misses Hortmann by 0.21 % at
   Ra 10⁶.
2. **Lid-driven cavity: Ghia 1982 cannot be a 1 % line.** Its own 129² values are 0.85 % low in ψ and
   1.5–2.2 % low in the centreline extrema against Botella & Peyret (§11). Botella & Peyret stay the
   reference.
3. **T-junction stencil (S9).** Losasso et al. 2004 itself says nothing about rest states or parasitic
   currents; that is Aanjaneya's measurement. A 2006 follow-up by the same authors changes the
   stencil (one shared gradient per large face) and reports second-order convergence on refined
   test grids, with its own caveat that those grids are not graded (§5). Build that variant as
   option 1. The control arm and the power-diagram fallback stay.
4. **The Helmholtz term does less than §8 claimed.** Fuster & Popinet's implicit scheme neglects
   compressibility for wavelengths below about 40 c₀Δt, which is **about 690 m at Δt = 0.05 s**. It
   gives slow compressibility, not building acoustics (§8).

**Claims corrected.**
- *Martin & Moyce.* The recalled non-dimensionalisation T = t√(2g/a) was wrong. It is
  **T = n t (g/a)^½**, and the authors normalised every record to agree at **Z = 1.44**, because the
  start of motion was not recorded (§11). The dam-break pass test must align there, not at t = 0.
- *Cavity numbers* (§10, §11): the recalled Ra 10³ value 1.118 and all eight velocity maxima were
  right; the claim that the Richardson values "land within 0.1 % of the references" was true only
  against de Vahl Davis (+0.08 % at Ra 10⁶) and false against Hortmann (−0.21 %). The "grid for 1 %"
  figures change (below). The velocity-maxima agreement is 0.23 % at Ra 10⁶ against Hortmann, not 0.6 %.
- *Weymouth & Yue.* The conservation condition is on the **sum** of the directional Courant numbers
  (Δt Σ|u_d/Δx_d| < ½, eq. 21), not on one CFL. And the total does **not** telescope when the
  divergence residual is non-zero: the dilatation term adds Δt·Σ c_c(∇·u)V per step (§6).
- *Ng, Min & Gibou 2009.* Cut-cell projection is second-order in 2-D but only **first-order in 3-D**.
  Batty et al.'s variational solve has O(1) maximum-norm errors at the solid boundary (§7).
- *Batty et al. 2007.* The added-mass claim in the first version is not in the paper. What it does say
  about light bodies, hydrostatic rest and thin bodies is in §7.
- *LBM.* Rohde et al.'s refinement is mass-conservative *by construction*, but it does not conserve
  momentum a priori and gives viscosity-dependent errors and a staggered fine-grid solution, so
  "fully refined = uniform" is still not structural. Körner et al.'s free-surface LBM also conserves
  total mass exactly, so "only VOF conserves the water" was too strong (§4.3). The rejection stands
  on other grounds.
- *Hu et al. 2018.* The paper reports CPU timings only. "GPU-friendly" is not from it.
- *Popinet 2015* is the Serre–Green–Naghdi shallow-water solver. Its "well-balanced" is lake-at-rest
  by hydrostatic reconstruction, not the 3-D projection solver. The hydrostatic principle the note
  uses comes from Popinet 2018, §3.
- *Setaluri et al. 2014.* SPGrid's speed comes from x86 virtual memory (4 KB pages, `mmap`), Linux
  only, smoke only. The idea carries over; the implementation does not (§5, §9).
- *Gray & Giorgini 1976.* Boussinesq limits for the temperatures Cavendish uses are now stated (§8a).

## 1. Summary

**Recommendation: one finite-volume MAC projection solver.** Use velocities on faces and scalars at
centres; solve the pressure by multigrid-preconditioned conjugate gradients; put buoyancy in Boussinesq
form, with the hydrostatic part removed. Store it as a 2:1-balanced pyramid of sparse uniform levels.
Advect every scalar the instrument reads (temperature, the liquid's volume fraction) in **conservative
flux form**. Give water a **geometric volume-of-fluid (VOF)** surface, not a level set. Couple solids
through face and volume fractions. Build the pressure operator with an optional Helmholtz term, so
slow compressibility can be switched on later without a second solver (it does not give building acoustics; §8). Reject lattice Boltzmann (LBM), SPH,
FLIP/APIC and MPM as the main solver. The reasons, in order of weight:

1. **The towers read mass, so the fluid must hold mass exactly where it is.** One gram of water 1 m
   below a tower is **3.1 × 10⁻⁴ rad**, six shots' worth. Over an hour's mean, **4 g** at 1 m and
   **16 g** at 2 m are detectable [M, §3]. A cubic-metre tank must therefore keep its mass to about
   10⁻⁶ of itself. Flux-form VOF conserves exactly when the velocity is discretely divergence-free
   ([Basilisk `vof.h`](http://basilisk.fr/src/vof.h) [S]; Weymouth & Yue 2010, Table 4: 1.3 × 10⁻¹² of
   the volume per step, against 12.6 % over one period for the baseline [P]). A level set does not
   (Enright et al. 2002, §3.5: −49 % of the volume at maximum stretch, −80 % after one cycle [P]).
   An LBM density is a pressure proxy that carries artificial fluctuations of order Ma², 10⁻³–10⁻²
   of ρ [D, §4.3]. That is larger than the whole 1 K air signal (3.4 × 10⁻³ of ρ) and must never be
   read as mass.
2. **Acoustics do not reach the phase, so incompressible is not an approximation that costs signal.**
   The interferometer weights the gradient with a triangle of width 2T = 1.46 s. Its response falls as
   (sin x / x)², with x = πfT: 0.93 at 0.2 Hz, 0.11 at 1 Hz, **6 × 10⁻⁴ at 17 Hz** [M]. The lowest
   acoustic mode of a 20 m hall is 8.6 Hz, and sound crosses the hall in 58 ms [M]. Inside a building,
   pressure is therefore uniform plus hydrostatic on every timescale the towers see. That is the
   low-Mach model the heat note already recommends, with p₀(t) for a sealed room
   ([`s13-heat.md` §3.4](s13-heat.md)). One pascal of air is equivalent to 2.1 mK, about 4 × 10⁻⁵ rad
   [M]. Infrasound from outside is an S14 far-field input, not something to resolve cell by cell.
3. **It can be validated against known answers at every stage.** I measured the de Vahl Davis cavity
   on the proposed MAC discretisation (§10). It converges at second order, its three-grid
   extrapolation reaches Hortmann et al.'s Nu within 0.03 % at Ra 10⁴–10⁶, and the projection leaves
   divergence at 10⁻¹³. "Fully refined = uniform" is a bit-level property of a MAC octree with a
   uniform special case. LBM has no such special case at refinement interfaces: they need dedicated
   treatment, and Rohde et al.'s mass-conservative scheme still departs from the uniform-fine
   solution (§4.3).
4. **It shares machinery with S13.** S13's conduction reuses this pressure multigrid, and its energy
   budget can close only with flux-form temperature advection ([`s13-heat.md` §8](s13-heat.md)). This
   note makes that S8 decision: flux form.
5. **It fits the GPU and WebGPU.** The scheme is f32 stencils, a geometric V-cycle with Jacobi or
   red-black smoothing, and about 40 B per cell. LBM needs 55–153 B per cell
   ([FluidX3D](https://github.com/ProjectPhysX/FluidX3D) [S]) against WebGPU's default 128 MiB binding
   and 8 storage buffers per stage ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/GPUSupportedLimits) [S]).
   On paper, LBM's speed advantage disappears for slow natural convection once its time step is
   counted (§9).

**What the measurement changes in the plan (§10).**
- The S8 exit "Nu within 1 % of the reference" needs, on a uniform grid [M, `fluids/results/compare_refs.txt`]:
  against **Hortmann** about **35 cells across at Ra 10⁴, 65 at 10⁵ and 120 at 10⁶** (N = 64 passes
  10⁴ at 0.29 % and just misses 10⁵ at 1.06 %; N = 128 passes all three at 0.07, 0.24 and 0.86 %);
  against **de Vahl Davis** about 50, 70 and 140. At 32² the error is 1.2 %, 5 % and 19 %. v2's 32²
  run passed at Ra 10³–10⁴ only. (The first version said 64, about 100 and 140–160; the middle
  figure was not supported by its own table.)
- At Ra 10⁶ the references differ by 0.29 %: 8.800 (de Vahl Davis, Table V p. 256 [P]) against 8.825
  (Hortmann et al., Table IV p. 202 [P]). De Vahl Davis estimates his own error at 1 % there
  (Table VII, p. 261 [P]); Hortmann's grids of up to 640² put his at 0.01 % (abstract). The brief
  should name Hortmann for Ra 10⁴–10⁶ and de Vahl Davis for Ra 10³.

**Other changes to the plan.**
- The free surface becomes VOF, not "a level set on the same octree" (§6).
- The lid-driven reference becomes Botella & Peyret 1998. Ghia 1982 is a historical line only: its
  129² values are 0.85–2.2 % from the spectral answer (§11).
- Losasso et al. 2004's first-order T-junction pressure is reported to produce parasitic currents "even
  for hydrostatic scenarios" by [Aanjaneya et al. 2017, §1](https://pages.cs.wisc.edu/~sifakis/papers/power_diagrams_2017.pdf) [P].
  Losasso et al. themselves neither test nor mention rest states (§4.2–4.3 read in full [P]). The
  "at rest on a graded grid" test therefore needs a control arm that shows the failure, and a
  stated fallback: the power-diagram discretisation (§5).

## 2. How to read the evidence

Tags as in the heat note:
- **[P]**: read in the primary source (full text extracted here; from the second pass, the paper
  itself, not a table quoted by another paper);
- **[S]**: a summary of a primary page or document (official documentation, source-code docs, a
  table quoted by another paper);
- **[R]**: recalled, **unverified**;
- **[D]**: derived here by hand;
- **[M]**: measured or computed here (`fluids/`).

Every DOI in the final section was checked against Crossref for title and year
(`fluids/results/doi_check.txt`). That check does not mean I read the paper; see the last section.

## 3. What the fluid must get right: signal sizes [M]

AION-10 constants as in the other notes: S = k_eff T² = 4.80 × 10⁹ rad per m s⁻², baseline 5 m,
T = 0.73 s, one shot 5 × 10⁻⁵ rad, a one-hour mean 1.2 × 10⁻⁶ rad
([`s13-heat.md` §5](s13-heat.md), [`exact-skin-and-partial-fill.md`](exact-skin-and-partial-fill.md)).
Output in `fluids/results/signals.txt`.

| 1 g of fluid at distance D from the lower interferometer point | beside | below | mass for one shot | mass for a 1 h mean |
|---|---|---|---|---|
| 1 m | 1.2 × 10⁻⁵ rad | 3.1 × 10⁻⁴ rad | 161 g | **3.9 g** |
| 2 m | 1.0 × 10⁻⁵ | 7.4 × 10⁻⁵ | 680 g | **16 g** |
| 5 m | 4.5 × 10⁻⁶ | 9.6 × 10⁻⁶ | 5.2 kg | 125 g |
| 10 m | 1.2 × 10⁻⁶ | 1.8 × 10⁻⁶ | 28 kg | 674 g |

**Consequences.**
- *Water.* Mass conservation is needed at the gram level per cubic metre (10⁻⁶) near a tower. A
  method that loses or gains percent of its volume is not usable, however good its surface looks.
- *Water in f32.* One unit of f32 rounding of a 1 cm cell of water is 6 × 10⁻⁸ g. Rounding is not
  the problem; non-conservative schemes and clipping are.
- *Air.* 1 K of mean hall air is 2 × 10⁻² rad (heat note), and 1 Pa is equivalent to 2.07 mK, so
  about 4 × 10⁻⁵ rad per pascal of uniform pressure change in the hall.
- *Sound inside.* Everyday sound (0.02 Pa at 60 dB) is under a shot and filtered by ≥ 10³ above
  17 Hz. A slammed door or ventilation could perhaps reach 1–10 Pa, but at frequencies the
  interferometer rejects [D].
- *Slow pressure.* Slow pressure (weather, a sealed room heating) arrives through p₀(t), not through
  waves.

**The interferometer's frequency response.** The phase is k ∫ g(t) f(t) dt with the triangular weight
f. Its transform relative to DC is (sin x / x)² with x = πfT. Quadrature and the closed form agree to
four digits [M]:

| f (Hz) | 0.05 | 0.2 | 0.5 | 1 | 2 | 17 |
|---|---|---|---|---|---|---|
| \|H\| | 0.996 | 0.93 | 0.63 | 0.107 | 0.047 | 6.1 × 10⁻⁴ |

## 4. The candidates

| Method | Mass conservation | Density accuracy for the read | Cost / GPU / WebGPU | Adaptivity | Free surface | Compressibility |
|---|---|---|---|---|---|---|
| **MAC projection** (Harlow & Welch 1965; Chorin 1968) + Boussinesq | exact for flux-form scalars; divergence to solver tolerance | read from θ (exact ideal gas) or VOF fraction: exact cell masses | stencils + MGPCG, ~40 B/cell, f32 fine | octree with 2:1 balance (Losasso 2004; Popinet 2003; Aanjaneya 2017) | VOF or level set on the same grid | add a Helmholtz term (all-Mach, Fuster & Popinet 2018) |
| **LBM** (incl. thermal DDF, octree, free surface) | exact on a uniform grid; refinement is mass-conservative (Rohde 2006) but not momentum-conservative a priori, and departs from the uniform-fine solution | density is p/c_s² with artificial c_s: Ma² artefacts; read mass from T or fill instead | very fast per cell (1,696 MLUPs/s on the GTX 1660 SUPER, FP32), 55–153 B/cell; time step tied to dx | waLBerla: ~10¹² cell updates/s on non-uniform grids (Schornbaum & Rüde 2016) [S] | free-surface LBM with a VOF-like fill-fraction ledger, total mass exact (Körner 2005, eqs. 9–11 [P]); capillary-wave frequency 4 % off at 100², first-order in resolution (§4.2.1 [P]) | weakly compressible by construction; physical sound speed costs 70× (§9) |
| **FLIP / PIC / APIC** | particle mass exact; volume drifts (clumping) | density from particle counts: noisy | particles + grid; GPU-friendly; sorting costs | grid adaptive possible | natural (particles) | no |
| **MPM / MLS-MPM** | particle mass exact | as FLIP; weakly compressible fluids | CPU timings only in Hu 2018 (4-core i7, SPGrid grid) [P]; GPU use is not shown there | sparse grids | natural | weakly |
| **SPH** (WCSPH, IISPH, DFSPH) | particle mass exact | DFSPH enforces volume compression below 0.01 % (Bender & Koschier 2015, abstract [P]); looser solvers ~1 % [R] | neighbour search, ~30–50 neighbours | particle splitting, hard | natural | weakly, or iterative incompressible |

### 4.1 MAC projection (the recommendation)

- **Placement.** Pressure at centres and normal velocities on faces. The discrete divergence and
  gradient are adjoint, so the projection is exact to the solver tolerance and has no checkerboard
  mode. This is the Harlow–Welch staggered grid (velocity and pressure as primary variables, pressure
  from a Poisson equation that keeps the divergence at zero; their free surface uses marker
  particles, not VOF: p. 2183 [P]) with Chorin's projection (primitive variables, 2-D and 3-D;
  abstract [P]).
- **Measured here.** Divergence after projection is 10⁻¹⁴–10⁻¹¹ (relative to the velocities of order
  1–200 in α/L units), with Nu equal on hot and cold walls to every printed digit [M, §10].
- **v2.** v2's CPU version passed the cavity at 32² for Ra 10³–10⁴
  ([`reference/design/physics.md` §5.5](../../reference/design/physics.md)).
- **What to change from v2.** v2 advected temperature by semi-Lagrangian MacCormack. That scheme is
  **not conservative**, so the total θ (the air's mass anomaly) drifts. The heat note found the S13
  energy budget cannot close with it ([`s13-heat.md` §1, §8](s13-heat.md)). Use a flux-form
  second-order upwind scheme with a limiter for every scalar: Bell–Colella–Glaz (Bell, Colella & Glaz
  1989), as Basilisk does ([`centered.h`](http://basilisk.fr/src/navier-stokes/centered.h) [S]).
  Momentum may keep MacCormack; its conservation is not read.

### 4.2 Pressure solver: MGPCG

- **McAdams, Sifakis & Teran (2010).** "Multigrid cycle as a preconditioner for the conjugate gradient
  method … typically reduces the residual by one order of magnitude every 2 iterations"; 10⁴ reduction
  at 128³ "in less than 0.75 seconds" on a 16-core machine
  ([PDF, abstract and §1](https://math.ucdavis.edu/~jteran/papers/MST10.pdf) [P]).
- **Domains.** The method is purely geometric, handles irregular voxel domains with mixed Neumann and
  Dirichlet conditions, and adds boundary smoothing sweeps where coarse cells straddle boundaries
  [P].
- **On octrees.** Aanjaneya et al. reach "satisfactory convergence within 6-10 iterations across all
  resolutions" with a first-order V-cycle preconditioning the second-order operator
  ([PDF §4](https://pages.cs.wisc.edu/~sifakis/papers/power_diagrams_2017.pdf) [P]). Their PCG counts
  are 4–10 per step (Table 2) [P].
- **Smoothers for the GPU.** Jacobi (damped by 2/3) or red-black Gauss–Seidel; Basilisk's multigrid
  uses red-black Gauss–Seidel on GPUs ([`poisson.h`](http://basilisk.fr/src/poisson.h) [S]).
- **Basilisk's tolerance.** The default is a residual of 10⁻³ (scaled), with relaxation counts adapted
  so that each cycle reduces the residual by 2–20 [S]. Cavendish needs tighter: see §6 on VOF and the
  divergence residual.

### 4.3 Lattice Boltzmann, and why not here

- **Strengths.** LBM is local, voxel-native and very fast per cell. FluidX3D reaches **1,696 MLUPs/s
  in FP32 and 3,551 in FP32/FP16 on a GTX 1660 SUPER**. It also has thermal (D3Q7 DDF) and VOF/PLIC
  free-surface extensions ([README](https://github.com/ProjectPhysX/FluidX3D) [S]).
- **Precision.** Lehmann et al. show that "pure FP32 precision is sufficient for most application
  scenarios of the LBM" ([arXiv:2112.08926, conclusion](https://arxiv.org/pdf/2112.08926) [P]).
- **Scale on refined grids.** waLBerla runs non-uniform grids at "close to a trillion" cell updates
  per second: 889,602 MLUPS on 458,752 cores for 886 billion cells (Schornbaum & Rüde 2016, §5.2 and §6,
  double precision [P]; [arXiv:1508.07982](https://arxiv.org/abs/1508.07982)). That is a petascale
  machine, not a GPU.
- **Free surface.** Körner et al. keep a fill-fraction mass M per interface cell, update it from the
  distribution-function fluxes (eq. 9–11) and spread any over- or under-shoot evenly over the new
  interface cells "to ensure exact mass conservation" (§2.3 [P]). So an LBM water surface *can* conserve
  total mass exactly. The bulk cell's M is still its lattice density, with the Ma² noise of point 1.
- **Licence.** FluidX3D is free for non-commercial use only [S], so it is a reference, not a
  dependency.

Against it, for Cavendish:

1. **Its density is not the fluid's density.** LBM's pressure is p = c_s²ρ with an artificial lattice
   sound speed, so dynamic pressure appears as δρ/ρ ~ u²/c_s² ~ Ma² [D]. That is 10⁻³–10⁻² at the
   usual lattice Ma of 0.03–0.1. For air the 1 K signal is 3.4 × 10⁻³ of ρ, so the LBM density must
   not be read. For water it would be 1–10 kg m⁻³ of spurious mass. The mass read has to come from T
   or from a fill fraction anyway, so LBM brings no read-side advantage.
2. **Refinement interfaces.** Interfaces between levels need explicit treatment. Rohde et al.'s
   volume-based scheme moves distributions as mass between coarse and fine cells, so mass is
   conserved by construction and the method is second-order for 2-D Poiseuille flow (abstract, §3
   [P]). But momentum is not conserved a priori (it sums to zero only in a periodic test, §3.2.2,
   Fig. 16), the errors depend on viscosity, body force and the angle of the flow to the interface,
   and for flow normal to it the fine grid shows a staggered solution that needs space-averaging
   (§3.2.3, conclusions [P]). Schornbaum & Rüde 2016 build on it, with 2:1 balance and a mass- and
   momentum-conserving coarse-to-fine interpolation (§1, §4.3 [P]), and say the interface "must
   receive special treatment" (abstract). Their accuracy validation is two Poiseuille flows only
   (§5). So "fully refined = uniform" is still not structural, and the grid levels must also be
   sub-cycled: acoustic scaling makes level L take 2^L steps per coarse step (§2.3 [P]).
   **Is the rejection still fair? Yes, but on different grounds.** The mass-conservation argument
   is weaker than the first version said. The strong grounds are the density artefact (point 1), the
   sub-cycled time step, the absence of a shared elliptic solver (point 4) and the memory.
3. **Time step at low speed.** LBM's speed advantage shrinks once the time step is counted (§9). The
   memory is 1.4–4× a MAC cell's.
4. **No reusable elliptic solver.** S13 needs a multigrid for steady conduction and the low-Mach
   divergence, and LBM does not provide one.

LBM remains the better choice if Cavendish ever wants physical acoustics or turbulent room flows at
fixed resolution on a uniform grid. Neither is a current need (§3).

### 4.4 Particles (FLIP, APIC, MPM, SPH)

- **What particles do well.** They carry mass exactly. APIC fixes PIC's dissipation and FLIP's noise
  and "allows for exact conservation of angular momentum across the transfers"
  ([Jiang et al. 2015, abstract](https://www.math.ucdavis.edu/~jteran/papers/JSSTS15.pdf) [P]).
- **What they do badly for this read.** The same paper names the hybrid methods' "particle positional
  artifacts such as noise, clumping and volume" loss (§1) [P]. The read needs the mass *distribution*
  accurate near a tower, and clumping is a distribution error.
- **SPH.** SPH needs particles throughout the air, which is hopeless at 10⁹ cells. DFSPH holds volume
  compression below 0.01 % (Bender & Koschier 2015, abstract [P]), so the density error is not the
  objection; the particle count and the cost per step are.
- **Where particles fit.** Keep MPM for granular matter (the Noita row of `REBUILD.md` §5.3). APIC
  could be a *visual* spray layer, labelled as such. Hu et al. 2018 report MLS-MPM about 2× faster
  than standard MPM, on CPU (Table 2 [P]); their background grid is an SPGrid.

## 5. The adaptive octree (S9)

- **Grid.** 2:1-balanced octree, stored as a pyramid of sparse uniform levels (the SPGrid layout,
  Setaluri et al. 2014; abstract, §4, Fig. 6(d) [P]). This is the layout Aanjaneya et al. use for
  adaptive liquids. They restrict grading so that "a cell and all its neighbors span only two levels of
  resolution" (§8) [P]; SPGrid's own paper handles "non-graded octrees" too (abstract). **Take the
  layout, not the implementation.** SPGrid gets its speed from x86 virtual memory (`mmap` with
  `MAP_NORESERVE`, 4 KB pages, the TLB; §3), runs on Linux only, reaches about 70 % of a dense C
  array's stencil throughput (Table 2, 8×4×4 blocks of 32-bit channels), and was shown on smoke, with
  free surfaces named as future work (§8 [P]). A wgpu or WebGPU build needs an explicit brick pool
  and page table instead (§9).
- **Balance algorithm.** p4est's 2:1 balance on Morton-ordered octants scales to 5.13 × 10¹¹ octants,
  "in less than 10 seconds per million octants per process"
  ([Burstedde et al. 2011, abstract](https://p4est.github.io/papers/BursteddeWilcoxGhattas11.pdf) [P]).
  Cavendish needs the single-octree, single-GPU version of the same algorithm on the Morton keys it
  already has ([`s5-world-octree.md`](s5-world-octree.md)). The fluid's active region is 10⁶–10⁷
  cells, not 10¹¹.
- **Pressure at T-junctions.** There are three published options:
  1. *Losasso, Gibou & Fedkiw 2004* [P: read in full, 6 pages]. Flux-form, symmetric positive
     definite, first order at T-junctions. Their face gradient is p_x = (p₂ − p₁)/Δ, where Δ may be
     the large cell's size, the small cell's size or the centre distance, because "O(Δx)
     perturbations in the pressure location still yield consistent approximations" (§4.2). That is
     the loss of orthogonality Aanjaneya describes. They say the dominant error is the
     first-order semi-Lagrangian advection and that a first-order Poisson solver is "on par
     asymptotically" with it (§4.3). Their Table 1 (unrestricted octree, exact solution) gives
     **L¹ orders falling from 2.2 to 1.1 and L∞ orders of 0.74–0.89**, i.e. first order at best in
     the maximum norm. The paper contains no rest-state or hydrostatic test and does not mention
     parasitic currents. The statement that follows is Aanjaneya's, not Losasso's: "The price for
     this choice was the loss of orthogonality between the stored velocity components at the
     T-junction cell faces and the corresponding discrete pressure gradients … gives rise to
     non-physical parasitic currents at T-junctions even for hydrostatic scenarios" (§1, citing
     five papers) [P]. The two accounts agree on the stencil; only Aanjaneya reports the symptom.
     Losasso's octree is *unrestricted*; Cavendish's 2:1 balance leaves fewer T-junction
     configurations. Their solver is PCG with a modified incomplete Cholesky preconditioner, about 20
     iterations to machine precision and 25 % of the run time (§6 [P]), not multigrid.
     **Follow-up, same authors:** Losasso, Fedkiw & Osher 2006 (Computers & Fluids 35, §3.3,
     pp. 1007–1008, Tables 1–2 [P]) replace the small-face gradients by the large face's
     area-weighted gradient, (p_x)_s = (p_x)_L, so that fine and coarse face velocities agree after
     projection, and the matrix stays symmetric. On a uniformly refined quadtree and octree
     they measure second order in L¹ and L∞ (orders 2.0–2.7). They warn that repeated uniform
     refinement does not make an octree graded, so the test may not exercise the worst case
     (p. 1008). **Build this variant as option 1.**
  2. *Power diagrams* (Aanjaneya et al. 2017). Symmetric positive definite, second order, works at
     solid and free-surface boundaries. The same storage, plus geometry tables per T-junction
     configuration [P].
  3. *Collocated cell-centred with an approximate projection* (Gerris/Basilisk). Read in the
     primary papers now [P]: Popinet 2003 stores velocity and pressure at cell centres (§2) and
     constrains neighbours, including diagonal neighbours, to differ by at most one level, and every
     neighbour of a cut cell to be at the same level (Fig. 2, constraints a–c). The coarse/fine
     gradient uses a second-order interpolation and "a consistent flux estimation" (§4.1, Fig. 4,
     eq. 10); with solid cuts it is first order (§4.1). Because an exact projection is "difficult"
     for a cell-centred velocity, the face (MAC) velocity is projected exactly and the
     cell-centred field only approximately (§6); "the face-centred normal advection field is
     exactly discretely divergence-free, so that the advection scheme is exactly conservative"
     (§6). The matrix is not symmetric (Losasso et al. §1 [P]: Popinet "solved this nonsymmetric linear
     system with a multilevel Poisson solver"), so it needs plain multigrid rather than PCG. Convergence is "close to second order in all norms" on
     refined and solid-refined meshes (§4.4, Figs. 11–18). Gerris 2003 is inviscid (Euler) with no free
     surface; Popinet 2009 adds the VOF surface (§3). Popinet 2015 measures what a tree costs:
     6× fewer points than Cartesian gave 2–3× less wall-clock; the per-point overhead is 2–3×
     for the adaptive tree and still up to 2× for a constant-resolution tree (§4.3, Table 1, p. 351
     [P]; shallow-water and SGN solvers, one core). **Plan S9's speed-up on that scale, not the
     cell-count ratio.** Its multigrid stops at a residual below g/1000 with four relaxations per
     level (§3.3 [P]).

  **Choose option 1 first** (the 2006 shared-gradient variant above), as `reference/design/voxel.md`
  §9.2 already plans, for two reasons. It is the simplest, and fluxes match across level jumps by
  construction, so mass is conserved. Combine it with **hydrostatic subtraction**: write the
  pressure as p = p_ref(z) + p′, with ρ_ref(z) the horizontally uniform reference column, and drive
  buoyancy by ρ − ρ_ref. A horizontally uniform stratification is then exactly at rest with any
  stencil [D]. Why it is needed: in hydrostatic balance the pressure at the centres of a large and
  a small cell across a T-junction differs by ρ g h/4 (h the large cell's size; the centres are h/4
  apart in height), so Losasso's stencil (p_s − p_L)/Δ with Δ = 0.75 h reads a horizontal pressure
  gradient of about ρ g/3 where the true value is zero [D]. If p′ = p − p_ref(z_c) is formed at the cell
  centres, the same stencil sees exactly zero. This is Popinet's well-balanced principle for a body
  force that is a gradient: "the hydrostatic pressure is thus subtracted analytically, which is
  advantageous for ensuring exact hydrostatic balance across the interface" (Popinet 2018, §3,
  p. 56 [P]). With a free surface the subtraction leaves an interface term, −[ρ] g·x n_s, from
  the density jump (same paragraph); that is where the still-water reference level enters.

  The failure Aanjaneya describes then remains only where the density varies *horizontally* across a
  T-junction (a plume crossing a level jump). It is measured by the plume test against the uniform
  run. If that test fails its tolerance, adopt option 2; the storage does not change.
- **Refinement.** Refine by what the towers see, as in `voxel.md` §9.3. Also refine in a band around
  the VOF interface and near walls. The cavity (§10) shows that resolution is needed in the wall
  boundary layers, not in the core.
- **Transfers.** Restriction sums masses. Prolongation is conservative; for VOF it is geometric
  (Basilisk's `fraction_refine`, [`vof.h`](http://basilisk.fr/src/vof.h) [S]).
- **Mass ledger.** If masses (θ·h³ and f·h³) are held as **fixed-point integers** (u32, or a
  two-word integer), every flux and every transfer is exactly reversible. The total is then identical
  bit for bit, whatever the summation order [D]. WGSL has `atomic<u32>`/`atomic<i32>` only, with no
  float atomics or f64 ([WGSL §6.2](https://www.w3.org/TR/WGSL/) [S]), so integer fluxes are also the
  portable choice. At 2⁻³² of a 1 cm cell, the resolution is 2 × 10⁻¹⁰ g of water.
- **Time step.** One global step limited by the finest level's CFL in S9. Sub-cycling with a flux
  register (Berger–Colella, as the heat note uses) is a later optimisation.

## 6. Free surface (S10)

| Method | Mass | Notes |
|---|---|---|
| Level set (Osher–Sethian; Losasso 2004's surface) | **not conserved**: reinitialisation and advection move the zero contour. Losasso et al. themselves note that semi-Lagrangian level sets lose "extreme" mass (§1 [P]); Popinet 2018 §4: "the main drawback of level-set formulations is the lack of discrete volume/mass conservation" [P] | smooth normals and curvature |
| Particle level set (Enright et al. 2002) | much better, not exact: in the 3-D deformation test (§3.5) the level set lost 49 % at maximum stretch and 80 % after one cycle; the particle level set gained 1.9 % and then lost 2.6 %. "Does not exactly conserve area, unlike VOF methods" (§3.3) [P] | the 2002–2006 graphics standard |
| Original VOF (Hirt & Nichols 1981) | clipped: F < 0 and F > 1 are reset after each advection, and the accumulated volume change is only reported; "a fraction of a per cent" after hundreds of cycles (§IV.D, p. 213 [P]) | donor–acceptor fluxes with a stair-step surface |
| **Geometric VOF, PLIC** (Weymouth & Yue 2010; Basilisk) | **exact** for a discretely divergence-free face velocity with Δt Σ_d \|u_d/Δx_d\| < ½ (eq. 21), no clipping, explicit sweeps. 1.3 × 10⁻¹² of the volume per step in a 3-D breaking-wave test (Table 4) [P]. Basilisk: "guarantees exact mass conservation for the multi-dimensional advection scheme (provided the advection velocity field is exactly non-divergent)" ([`vof.h`](http://basilisk.fr/src/vof.h) [S]) | Cartesian grids in the paper; the tree version is Basilisk's |
| Gerris-style VOF (Popinet 2009, §3.2) | **not exact**: each direction sweep resets over- and under-shoots, "which will lead to loss of exact mass conservation"; under 10⁻² % in the §6 tests with the divergence tolerance at 10⁻³ (§6.4), 0.02 % through jet breakup (§6.5) [P] | adaptive quad/octree, balanced-force surface tension, height functions |
| CLSVOF / VOF-derived level set | as VOF | a distance field from VOF for normals, if ever needed |

**Choose VOF.**
- Of the Eulerian schemes here it is the only one that conserves water to the gram (§3) *if* the
  Weymouth–Yue conditions hold. The Gerris-style scheme and the original VOF clip and lose 10⁻⁴ or
  more; that is 100 g in a cubic metre (§3). A free-surface LBM also conserves the total (§4.3).
- Its unknown, the fill fraction, is exactly what the gravity read needs: cell mass = ρ_w(T) · f · h³.
- It matches the world volume's "volume fraction exact" stamping (`REBUILD.md` §5.2).

Surface tension is irrelevant at these scales (capillary length about 2.7 mm [R]), so VOF's weak
curvature is no cost. Popinet 2009 shows adaptive VOF with the multigrid projection on quad- and
octrees, with all interface cells refined to one level and the face velocities re-projected
locally on refinement so that they stay divergence-free (§3.3, Fig. 3 [P]). Its tolerance on the
velocity divergence was 10⁻³ and mass was conserved to 10⁻² % (§6.4 [P]). Weymouth & Yue's paper is
Cartesian only, and Popinet 2009 names exactly-conserving schemes as the thing to add on trees.

**One condition matters, and the first version stated it too kindly.** Weymouth & Yue's step is
Δf = (Δt/ΔΩ)(Δ_d F_d + c_c (∂u_d/∂x_d) ΔΩ) in each sweep d (eq. 20), with c_c the cell-centre
indicator (1 if f > ½, eq. 19). The flux terms telescope. The dilatation term, summed over the sweeps,
is c_c ∇·u, which is zero only if the face velocity is exactly divergence-free. With an iterative
pressure solve leaving a residual ε the total therefore changes by about
Δt Σ c_c ε V per step, which does *not* telescope [D from eq. 20]. Their Table 4 loss of
10⁻¹² per step is the divergence they had left (PCG), not a property of the VOF step. **Clipping
f to [0, 1] is a second, independent leak** (their baseline loses 12.6 % in a period, the explicit
variants 1.4 % and −0.8 %, from over-filling and over-emptying, §3.3), and must be replaced by
redistribution recorded in the ledger. Two consequences for the build:
- The ledger needs a separate account for Σ c_c ε V Δt, and the pressure tolerance has to be set
  against it. For 10⁻⁶ of a tank over N steps, with random-sign residuals the bound scales as
  √N and with same-sign residuals as N; the test `tank_mass_ledger` measures which [D].
- The f32 solver's residual is far above W&Y's. Whether the GPU can reach the needed ε in f32 is
  not known (§12, open question 1 below).
Test it (§12).

**Gravity and the surface.** With p = p_ref + p′ and the free surface as a Dirichlet condition for p′
(the ghost-fluid treatment of Gibou et al. 2002), still water is exactly at rest [D].

## 7. Solids, bodies and heat

- **Solids in the pressure solve.** Use face-area fractions and volume fractions in the pressure
  matrix. What the two papers say, read in full [P]:
  - *Batty, Bertails & Bridson 2007.* The voxelised solve has stair-step artefacts "which
    unfortunately do not converge to zero as grid resolution is increased" (§1, Fig. 2). Their fix
    is variational: pressure minimises kinetic energy, with the fluid mass in each MAC cell set by
    the fluid volume or face area; the system is always symmetric positive semi-definite (abstract,
    §2, eq. 3). The voxel solve is the special case of masses equal to h³ or 0. No convergence order
    is claimed, only pictures. Ng et al. (below) find it **not convergent in the maximum norm**.
  - *Ng, Min & Gibou 2009.* A different discretisation, also symmetric positive definite, with
    length fractions and densities as weights (§3.1). **Second order in L¹ and L∞ in 2-D, first
    order in both in 3-D** (abstract, §3.4); the pressure is second order, its gradient and the
    projected velocity first order, "exactly observed in our three spatial dimensions examples"
    (§3.4, pp. 8813–8814). They also find the rasterised solution not L∞-convergent, and Batty's
    solution with O(1) maximum-norm errors at the boundary (§1, §4, Table 2). Validated by flow
    past a sphere against Johnson & Patel up to Re 300 (§7.2).
  So the cut-cell weights **replace the staircase's O(1) boundary error with a first-order one in
  3-D**, not a second-order one. The first version, "a well-behaved pressure at the boundary",
  overstated it. For Cavendish the read is a volume integral (cell volume fractions, which are
  exact), so the first-order near-wall velocity matters for the flow, not for the mass the towers
  weigh. This replaces the voxel staircase, which is first order and makes surface area 1.5× too
  large ([`s13-heat.md`](s13-heat.md), staircase row).
- **The air's mass.** The fluid volume of a cell is (1 − φ_solid) h³, so the air displaced by a body
  matches the body's stamped volume. That consistency is what the moving-ball oracle checks.
- **Two-way coupling** (S10: the floating box, boats). Monolithic coupling puts the rigid body's
  velocity in the pressure solve: the body's kinetic energy is added to the minimisation, a
  rank-6 update of the matrix (Batty et al. 2007, §3, eq. 13 [P]). What the paper actually says
  about robustness: Carlson et al.'s Distributed-Lagrange-Multiplier method "cannot stably handle
  light solids (less than 0.45 the fluid density)", whereas their own demonstration includes a body at
  0.1 of the fluid density (§1.1 and §3.3, Fig. 1 [P]). It does *not* use the words "added mass"; the
  first version's added-mass statement was recalled and is not in the paper. Two cautions from §3.1:
  - "Perfect hydrostatic rest with neutrally buoyant rigid bodies" needs the 6×6 body mass matrix
    to be consistent with the volume weights used in the pressure solve, "multiplied by rigid body
    density, rather than the exact mass matrix". That is the floating-box test's condition: build
    the body's mass from the same cut-cell volumes.
  - The method assumes bodies thick enough to have an interior sampled on the grid; thin shells
    need a pressure jump and are not covered.
  The floating box then reaches equilibrium at the Archimedes draught through the same pressure
  that holds the water at rest.
- **Heat.** The same flux-form advection carries θ. Conjugate transfer, flux registers and film
  conductances are as in the heat note §3.2. The multigrid is shared.

## 8. Compressibility and pressure waves

- **Low-Mach split.** Split the pressure into a uniform thermodynamic p₀(t) and a dynamic part (heat
  note §3.4). This captures the sealed-room effect, the only compressible effect inside a building
  that the towers can see (§3).
- **The optional Helmholtz term.** Build the pressure operator as ∇·(β∇p) + λp, with λ = −1/(ρc²Δt²)
  set to zero by default. That is Basilisk's all-Mach formulation, in which "by default the fluid is
  incompressible" and a finite ρc² adds the λ term to the same Poisson–Helmholtz multigrid
  ([`all-mach.h`](http://basilisk.fr/src/all-mach.h) [S]; Fuster & Popinet 2018 [P]). It is implicit,
  so it carries no acoustic CFL limit. **But it does not propagate short waves.** In their 1-D
  pressure-pulse test the scheme is stable for acoustic CFL above 0.5 and "neglects compressibility
  effects for wavelengths below λ_c = 40 c₀Δt" (§4.1.1, Figs. 1–2, p. 14 [P]); its numerical
  dissipation also exceeds a classical second-order Riemann solver's, so about double the resolution
  is needed for equal accuracy. At the plan's Δt = 0.05 s, **λ_c ≈ 40 × 343 × 0.05 ≈ 690 m** [D]:
  everything shorter, including infrasound and every hall mode, is treated as incompressible. The term
  therefore adds slow compressibility (which p₀(t) already supplies for a sealed room), not
  building-scale waves. Waves on the grid would need Δt of the order of h/c, as with any other
  method. The first version said these "cost no new solver"; true for the matrix, but the
  time step makes them a different run. Keep the λ plumbing (it is cheap) but do not promise
  acoustics from it.
- **Explicit compressible solvers.** These, and LBM at the physical sound speed, need Δt ≤ h/c, which
  is 1.5 × 10⁻⁴ s at 5 cm, for frequencies the interferometer filters out. Rejected.

## 8a. Where the Boussinesq approximation holds (Gray & Giorgini 1976) [P]

Gray & Giorgini derive, for a horizontal layer of depth L with temperature difference Θ, when each
neglected property variation stays below 10 % (their eq. 26, |ε_i| < 0.1), using properties
linearised about 15 °C and 1 atm (Table 1). The limits that bind (p. 549, eqs. 38–45, Table 2, Figs. 1–2):

| Fluid | Limit | What it protects |
|---|---|---|
| Air | Θ ≤ 28.6 K (eq. 42); L ≤ 830 m (eq. 43) | density variation with T and with P |
| Air | **L/Θ < 1020 cm/K = 10.2 m/K** (eq. 44); L < 4.1 km (eq. 45) | neglect of the pressure-work and viscous-dissipation terms in the energy equation |
| Water | **Θ ≤ 1.25 K** (eq. 38); L ≤ 2.4 km (eq. 39) | constant expansion coefficient α (its variation, ε₉ = 8 × 10⁻² Θ, is the most demanding term) |

For Cavendish:
- **Air, hall of L = 20 m with Θ = 1–10 K.** Eqs. 42–43 are met by a wide margin. Eq. 44 is **not**
  met for Θ = 1 K: it needs Θ > L/10.2 m per K ≈ 2 K. That is the adiabatic lapse rate (about
  9.8 K/km, 0.2 K over 20 m) competing with a 1 K signal [D]. The remedy is the reference column the
  note already proposes, made thermodynamic as well as hydrostatic: advect a potential-temperature
  anomaly (or T − T_ref(z) with T_ref the adiabatic profile), not raw T. **To check in the heat
  note:** that its θ is of that kind.
- **Water, Θ > 1.25 K.** A constant-α Boussinesq buoyancy has a 10 % error in the coefficient across
  1.25 K at 15 °C. Their text notes that water near 4 °C "should always be kept in mind" (the density
  maximum). Use the full ρ_w(T) in the buoyancy term for water, as the gravity read already does.
- The derivation is for a Rayleigh–Bénard layer with a 10 % criterion. I did not check how it
  transfers to an open hall with a localised plume; that needs a run against a full-density
  (low-Mach) model, which the S13 note's cases could supply.

## 9. Cost, GPU and WebGPU [D unless marked]

Only the cavity was measured. The GPU figures below are estimates from memory traffic, to be measured
in S8 on the build PC (a correctness-only GPU, per the owner's ruling).

- **Bandwidth.** The GTX 1660 SUPER's 1,696 MLUPs/s at 77 % roofline with about 153 B per D3Q19
  update implies about 340 GB/s ([FluidX3D](https://github.com/ProjectPhysX/FluidX3D) [S]).
- **MAC step.**
  - Advection and diffusion of u, v, w, θ (and f): about 100 B per cell.
  - MGPCG at about 10 iterations: about 120 B per cell per iteration (matrix–vector product, V-cycle
    with two sweeps each way at 8/7 for the coarse levels, dot products).
  - Total about **1.3 kB per cell per step**, so about 2 × 10⁸ cell-steps/s at 250 GB/s effective.
- **Air at 10⁷ active cells.** 5 cm, a 0.5 m/s plume, CFL 0.5, Δt = 0.05 s: **about 50 ms per step,
  about real time**, for dense bricks. Add the sparse-structure overhead: SPGrid reaches about 70 % of
  a dense array's stencil throughput on a CPU (Setaluri et al. Table 2 [P]), i.e. about 1.4×; a pointer
  tree cost Popinet 2–3× per point (Popinet 2015, §4.3 [P]). Budget **70–150 ms per step**, which is
  still about real time, but without margin for the f32 pressure tolerance of §6.
- **LBM at the same 10⁷ cells.** About 6 ms per step at FP32. At lattice Ma 0.1 and 0.5 m/s, Δt is
  about 5.8 ms, so 172 steps per simulated second, also **about real time**. At the physical sound
  speed, Δt = h/(√3 c) = 8.4 × 10⁻⁵ s, which is **about 70 s per simulated second**.
- **Uniform 10⁹ cells.** About 5 s per step. The fluid is adaptive or baked (`REBUILD.md` §5.1); it
  never runs uniformly over a 10⁹–10¹¹-cell world.
- **WebGPU.**
  - No f64 (WGSL scalars are bool, i32, u32, f32, f16 [S]), so the f64 twin lives on the CPU, as the
    plan says.
  - The default limits are maxStorageBufferBindingSize 128 MiB, maxBufferSize 256 MiB and 8 storage
    buffers per shader stage ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/GPUSupportedLimits) [S]).
    An advection kernel touching u, v, w, θ, f in and out would need 10 bindings. Interleave channels
    in bricks (SPGrid's "several distinct channels within a 4KB page", Aanjaneya §2.1 [P]; Setaluri
    et al. §3.2 and Fig. 5: a block of 8×4×4 cells holds all the 32-bit channels of one page [P]), and
    request the adapter's higher limits where available. SPGrid's virtual-memory paging is not
    available on a GPU, so the brick pool, its page table and the neighbour lookup are ours to build.
  - One f32 field of 3.3 × 10⁷ cells fills a default binding.

## 10. Measured: the de Vahl Davis cavity on the S8 discretisation [M]

**The solver** (`fluids/cavity.py`, 2D, numpy):
- MAC grid; second-order conservative central advection (the Harlow–Welch form); explicit diffusion.
- Boussinesq buoyancy on vertical faces.
- Exact Neumann projection by a cosine transform, applied at each SSP-RK3 stage.
- Pr = 0.71; hot wall +½, cold wall −½, adiabatic top and bottom.
- Run until max |∂T/∂t| < 10⁻⁵.
- Nu from the second-order one-sided wall gradient, averaged over the wall.

**References, now read in the primary papers** (`fluids/compare_refs.py`; output in
`fluids/results/compare_refs.txt`; no run was repeated, it reads the stored JSON).
- **de Vahl Davis 1983, Table V (p. 256):** Nu = 1.118 / 2.243 / 4.519 / 8.800 for Ra 10³–10⁶ (so the
  Ra 10³ value that the first version took from v2 is confirmed). Midplane maxima u_max =
  3.649 / 16.178 / 34.73 / 64.63 and w_max = 3.697 / 19.617 / 68.59 / 219.36. He estimates the
  accuracy of these as 0.1, 0.2, 0.3 and **1 %** (Table VII, p. 261), from 81 × 81 grids and
  extrapolation (§"Extrapolated solutions"). He recommends the mid-plane Nu_{1/2} (8.799 at Ra 10⁶)
  over the wall-averaged Nu₀ (8.817) as "the Nusselt number used to describe the heat flow" (p. 262).
- **Hortmann, Perić & Scheuerer 1990, Table IV (p. 202):** Nu = 2.24475 / 4.52164 / 8.82513 at
  Ra 10⁴ / 10⁵ / 10⁶, u_max = 16.1802 / 34.7399 / 64.8367, v_max = 19.6295 / 68.6396 / 220.461.
  Finite-volume multigrid on grids up to 640², "second-order monotonic convergence", stated accurate
  to 0.01 % (abstract; 0.001 % for the extrapolated values, conclusion 5). They find de Vahl Davis 0.08 / 0.06 / 0.2 % away in Nu
  (p. 204) and argue that the only consistent wall-gradient formula is the one used in the
  discrete energy equation (§3, p. 199). Their Nu is Q/Q_cond with Q the flux through any x = const line,
  equal to the wall flux by conservation (§3). Ra 10³ is not covered.
  (Gjesdal et al. 2003, the first version's secondhand source, rounded these to 2.245 / 4.522 / 8.825.)

| Ra | N | Nu (hot = cold) | error vs de Vahl Davis | error vs Hortmann | u_max, v_max (N = 128) | wall time |
|---|---|---|---|---|---|---|
| 10³ | 32 / 64 / 128 | 1.1201 / 1.1184 / 1.1179 | +0.19 / +0.03 / −0.01 % | (none) | 3.650, 3.698 | 3 / 39 / 348 s |
| 10⁴ | 32 / 64 / 128 | 2.2725 / 2.2512 / 2.2463 | +1.31 / +0.37 / +0.15 % | +1.23 / +0.29 / +0.07 % | 16.18, 19.63 | 3 / 33 / 304 s |
| 10⁵ | 32 / 64 / 128 | 4.7453 / 4.5697 / 4.5326 | +5.01 / +1.12 / +0.30 % | +4.95 / +1.06 / +0.24 % | 34.76, 68.67 | 4 / 29 / 262 s |
| 10⁶ | 32 / 64 / 128 | 10.491 / 9.184 / 8.901 | +19.2 / +4.37 / +1.15 % | +18.9 / +4.07 / +0.86 % | 64.99, 220.6 | 6 / 32 / 205 s |

| Ra | Richardson (64, 128), p = 2 | vs dVD / vs Hortmann | Observed order p (32, 64, 128) | Extrapolation with that p | vs dVD / vs Hortmann |
|---|---|---|---|---|---|
| 10³ | 1.1178 | −0.02 % / – | 2.06 | 1.1178 | −0.02 % / – |
| 10⁴ | 2.2447 | +0.08 % / −0.00 % | 2.13 | 2.2449 | +0.09 % / +0.01 % |
| 10⁵ | 4.5203 | +0.03 % / −0.03 % | 2.24 | 4.5227 | +0.08 % / +0.02 % |
| 10⁶ | 8.8071 | +0.08 % / **−0.21 %** | 2.21 | 8.8231 | +0.26 % / **−0.02 %** |

**What it shows.**
- **Order.** The error against Hortmann falls by 4.05–4.71 per halving of h at Ra 10⁴–10⁶ (observed
  order 2.1–2.2), and the Ra 10³ differences by 4.2: **second order**. (The first version said 3.5–4.4,
  taken against de Vahl Davis, whose own uncertainty distorts the ratio.)
- **Extrapolations.** This is where the first version was wrong. "Within 0.1 % of the references"
  holds against de Vahl Davis, because his Ra 10⁶ value is 0.28 % below Hortmann's. Against
  Hortmann the fixed-order (64, 128) extrapolation at Ra 10⁶ is **−0.21 %**, because at N = 64 the
  run is not yet asymptotic (ratio 4.7, not 4). **Using the order observed from three grids** puts all
  three Ra within **0.03 %** of Hortmann. That is the extrapolation to specify.
- **The velocity maxima.** The values I recalled are all confirmed by Table V (3.649/3.697,
  16.178/19.617, 34.73/68.59, 64.63/219.36). Against de Vahl Davis the Ra 10⁶ N = 128 run differs
  by +0.55 % in both; against Hortmann by **+0.23 % (u) and +0.04 % (v)**. So the 0.6 % in the first
  version was de Vahl Davis's own 1 % uncertainty showing, not a discretisation error.
- **The discretisation is sound.** The S8 exit is a question of resolution.
- **Grid for 1 % (error ∝ N⁻² from N = 128).** Against Hortmann: Ra 10⁴ needs about **35**, 10⁵ about
  **65**, 10⁶ about **120**. Against de Vahl Davis: about 50, 70 and **140** (the first version
  said 40, 70, 140). N = 64 passes 10⁴ and misses 10⁵ by 0.06 points; N = 128 passes all three
  against Hortmann (0.07, 0.24, 0.86 %), but Ra 10⁶ at 0.86 % has little margin.
- **What this does to the S8 exit.** The criterion "Nu within 1 % of the reference" is unchanged,
  but the reference must be Hortmann's for Ra 10⁴–10⁶, because de Vahl Davis's Ra 10⁶ value is only
  good to 1 % by his own estimate (Table VII): a code that is exactly right at Ra 10⁶ would be 0.3 %
  from de Vahl Davis, and a code 1 % high would "pass" against him at 0.7 %. Ra 10³ stays on de Vahl
  Davis (0.1 %).
- **Recommended exit (changed).** Run **N = 64, 128, 256**, extrapolate with the order observed from
  the three grids, and pass if the finest grid is within 1 % and the extrapolation is within 0.3 %
  of Hortmann et al. (measured margin at 32/64/128: 0.03 %). The old two-grid, fixed-order rule would
  have given −0.21 % at Ra 10⁶, inside 0.3 % but without margin.
- **3D.** Run the cases as a one-cell-thick slab of the 3D solver with free-slip or periodic z faces,
  so the same code is tested.
- **The adaptive run.** The S9 run should match the uniform-256 Nu at a fraction of the cells, by
  refining the wall boundary layers.

**Limits of this measurement.**
- It is 2D, CPU and f64; it says nothing about GPU f32 or about the octree.
- The central advection would need a limiter at higher cell Péclet numbers.
- Its wall times are numpy's, not the GPU's.

## 11. Validation cases and their numbers

| Case | Published numbers | Status here | Use in |
|---|---|---|---|
| Differentially heated cavity, 2-D (de Vahl Davis 1983; Hortmann et al. 1990) | Nu 1.118 / 2.243 / 4.519 / 8.800 at Ra 10³–10⁶, Pr 0.71, accurate to 0.1 / 0.2 / 0.3 / 1 % (de Vahl Davis Tables V, VII); Hortmann: 2.24475 / 4.52164 / 8.82513 at Ra 10⁴–10⁶, 0.01 % (Table IV). Velocity maxima in §10 | all [P]; reproduced [M] (§10) | S8, S9 |
| Cubic cavity, 3-D, Pr 0.71 (Fusegi et al. 1991) | Overall Nu at the isothermal walls **1.085 / 2.100 / 4.361 / 8.770** for Ra 10³–10⁶ (Table 1, p. 1554), 3-D lower than 2-D by 4.5 / 8.3 / 4.8 / 1.9 %. Symmetry-plane (z = ½) values in Table 2: mean Nu 1.105 / 2.302 / 4.646 / 9.012 | [P]. **Not a 1 % reference**: 62³ grid (32³ at Ra 10³), QUICK, SIMPLE; doubling the x-grid at Ra 10⁶ moved Nu by < 0.2 % and the maximum velocities by about 2 %; their own 2-D runs sit within 1.5 % of de Vahl Davis at Ra 10⁶ (p. 1546) | S8 (3-D): pass at **2 %** on the overall Nu, until a tighter 3-D reference is read (§"Further questions") |
| Lid-driven cavity, Re 1000 | **Botella & Peyret 1998** (spectral, N up to 160): ψ = 0.1189366, ω = 2.067753 at (0.4692, 0.5652) in their frame, where the lid moves in −x (Table 6, p. 425; Table 9, p. 426, has u(y = 1) = −1); in the usual frame with the lid moving +x that is **(0.5308, 0.5652)**. Centreline extrema (Table 11, p. 428): u_max 0.3885698 at y = 0.1717; v_max 0.3769447 at x = 0.8422; v_min −0.5270771 at x = 0.0908 (their frame). Full centreline profiles at Ghia's points, with pressure and vorticity, in Tables 9–10. The seventh digit of ω at N = 160 is "questionable" (rounding, p. 426). **Ghia et al. 1982** (129², Table V p. 408; Tables I–II pp. 398–399): ψ_min −0.117929, ω 2.04968 at (0.5313, 0.5625); u_min −0.38289 at y = 0.1719; v_max 0.37095 at x = 0.1563; v_min −0.51550 at x = 0.9063 | Both [P]. **Ghia's values are 0.85 % (ψ), 1.5 % (u), 1.6 % (v_max) and 2.2 % (v_min) from Botella's**, and, by Botella's p. 432, up to 4 % off in the vorticity along the lid (Table 19); Botella's Table 6 prints Ghia's ω as 2.0677 where Ghia's own Table V says 2.04968 (a transcription slip in one of them) | S8 (tests momentum advection, which the cavity does not stress): ψ within **1 %** and the centreline extrema within **1 %** of **Botella**; Ghia only as the historical line, tolerance 3 % |
| Rayleigh–Bénard onset | Ra_c = 1707.762, k_c = 3.1163 for rigid plates | [M] in [`s13-heat.md`](s13-heat.md) | S8: growth-rate sign change within 1 % |
| Dam break (Martin & Moyce 1952, Part IV) | Rectangular column of base semi-length a and height n²a, collapsing in plane symmetry. **Z = z/a, T = n t (g/a)^½, τ = t (g/a)^½, H = η/(a n²)** (p. 313); the first version's T = t√(2g/a) was wrong. **Times are normalised so that every record agrees at Z = 1.44** (footnote to Tables 1–2, p. 316): "it was not found possible … to record the exact time of beginning of motion" (p. 314); the diaphragm was waxed paper burnt through by a heater strip. Surge-front mean T at Z for **n² = 1** (Table 1, a = 2¼ in): Z 1.44 → 0.80, 2.11 → 1.29, 3.00 → 1.93, 4.11 → 2.71, 5.0 → 3.32, 7.0 → 4.49, 10.0 → 6.70, 13.0 → 9.53. **n² = 2** (Table 2, a = 1⅛ in): Z 1.44 → 1.19, 1.89 → 1.58, 2.33 → 1.91, 3.22 → 2.58, 4.11 → 3.26, 5.00 → 3.92, 6.33 → 4.95. Maximum front speed U = dZ/dT of 1.62–1.71 (p. 320), i.e. about 1.6–1.7 √(g h₀), against 2 for shallow-water theory without vertical acceleration. Column height H(τ) in Table 6 (p. 320): n² = 2, a = 2¼ in: H 0.50 at τ = 2.00, 0.22 at 3.44 | [P], read from the scanned tables (rotated pages; the values above are those I could read unambiguously, to the last digit shown). Hirt & Nichols's case is n² = 2 with a = 1, 40 columns, and "the greatest deviation from the experimental results is everywhere less than one cell width" ("Broken Dam Problem", pp. 217–219, Figs. 6–7 [P]; their reference 18 is Martin & Moyce). The a = 2¼ in, n² = 2 mean column in Table 2 was not transcribed (row alignment unclear) | S10: front Z(T) within **max(5 %, one cell)** *after shifting the time origin so that Z = 1.44 matches*; H(τ) within 10 % |
| Standing wave / sloshing | ω² = gk tanh(kh); viscous decay 2νk² (Lamb) for small amplitude in deep water; Prosperetti 1981 treats the linear initial-value problem for two superposed viscous fluids of **infinite depth**, with analytic inversion of the Laplace transform in only three special cases and numerical inversion otherwise (abstract, §I) | closed form [D]; Prosperetti [P]. For finite depth use ω² = gk tanh(kh) and measure the decay against a fine-grid run, not 2νk² | S10: frequency within 1 %, and the gravity read of the sloshing mass |
| Archimedes floating | draught d = (ρ_b/ρ_w)·H for a box | closed form; needs the body's mass built from the same cut-cell volumes as the pressure weights (Batty et al. §3.1 [P]) | S10 |
| Acoustic pulse | d'Alembert: a Gaussian travels at c = √(γp/ρ) = 343.2 m/s [M], unchanged in 1D | closed form. On the implicit solver only wavelengths above 40 c₀Δt propagate, and with more dissipation than a Riemann solver (Fuster & Popinet §4.1.1 [P]) | only if the Helmholtz term is switched on, and then with Δt ~ h/c |
| Plume (Morton, Taylor & Turner 1956) | centreline w ∝ z^(−1/3) | as in VALIDATION §2.5 | S9 plume vs uniform |

## 12. Staged plan, tests and control arms

Every test states its pass criterion before it is run (CLAUDE.md). The control arm is the deliberately
broken variant that must fail.

### S8: one solver, uniform GPU grid

**Build.**
- MAC bricks in the world volume.
- Flux-form BCG advection for scalars, with fixed-point mass and θ ledgers.
- Momentum by MacCormack or BCG.
- Explicit diffusion, and Boussinesq buoyancy with hydrostatic subtraction.
- MGPCG (damped Jacobi or red-black, f32), with an f64 CPU twin.
- Solid face and volume fractions; p₀(t) leakage closure from the heat note.
- The λ term plumbed but zero.

| Test | Pass | Control arm (must fail) |
|---|---|---|
| `projection_removes_divergence` | max \|∇·u\| ≤ 10⁻⁶ of before, around a solid | projection skipped |
| `heated_cavity_2d` | Nu on the finest of 64/128/256 within 1 % of Hortmann (Ra 10⁴–10⁶) and de Vahl Davis (Ra 10³); extrapolation with the order observed from the three grids within 0.3 % of Hortmann (0.03 % measured at 32/64/128) | N = 32 at Ra 10⁶ (+18.9 % measured) |
| `cavity_second_order` | error ratio 4.0–4.8 per halving, observed order 2.0–2.3 (measured 2.06–2.24) | first-order upwind θ advection (ratio ≈ 2) |
| `heated_cube_3d` | overall Nu within 2 % of Fusegi et al. (1.085 / 2.100 / 4.361 / 8.770) on the finest affordable grid | N = 16 grid, or a 2-D result (3-D is 2–8 % lower) |
| `lid_driven_cavity` | ψ and the centreline extrema (u_max, v_max, v_min) at Re 1000 within 1 % of Botella & Peyret on the finest grid, 256² or Richardson; a second-order code at 129² is only as accurate as Ghia's 0.85–2.2 % | viscosity doubled |
| `rb_onset` | growth rate changes sign within 1 % of 1707.76 | wrong sign of β |
| `closed_box_scalar_ledger` | total θ·h³ bit-identical over 10⁴ steps | semi-Lagrangian θ advection (drifts) |
| `isothermal_air_is_silent` | a moving ball in isothermal air: air channel exactly 0 | a ball 1 K warm, non-zero and of the right sign |
| `moving_ball_oracle` | as the plan | — |
| `gpu_matches_f64_twin` | stated tolerance | — |

### S9: the adaptive grid

**Build.** 2:1 balance on Morton keys, a pyramid of sparse levels, Losasso-form flux Laplacian with an
MGPCG V-cycle on the tree's levels, conservative transfers, and goal-oriented marking.

| Test | Pass | Control arm |
|---|---|---|
| `fully_refined_equals_uniform` | identical to the solver tolerance, step for step | one region coarsened |
| `stratified_at_rest_graded` | ≤ 10⁻⁹ m/s on a deliberately graded grid | hydrostatic subtraction off: spurious current at T-junctions above threshold (expect about g/3 from the stencil [D, §5]) |
| `tjunction_variant` | Losasso 2006 variant: L¹ and L∞ error orders ≥ 1.8 on a **graded** (not merely refined) quadtree against an exact Poisson solution | the 2004 stencil: L∞ order ≈ 0.8 (their Table 1) |
| `graded_projection_flux` | fine-face fluxes sum to the coarse face exactly | — |
| `regrid_conserves_and_is_silent` | ledger bit-identical; phase step < tol | injection prolongation |
| `heated_cavity_adaptive` | Nu within 1 % at fewer cells than uniform-256; cells and time recorded | — |
| `plume_vs_uniform` | rise and anomaly within 2 %, phase within tol | the decider for power diagrams |

### S10: water

**Build.**
- PLIC VOF (Weymouth–Yue split) with fixed-point fill and redistribution instead of clipping.
- Refinement at the interface.
- Full-density read ρ_w(T) f h³.
- The free-surface Dirichlet condition on p′; monolithic rigid coupling.

| Test | Pass | Control arm |
|---|---|---|
| `tank_mass_ledger` | total water = initial + the recorded divergence account Σ c_c ε V Δt, with the remainder bit-zero, through sloshing and regrids; the account itself below 10⁻⁶ of the tank per hour (§6) | clip f to [0, 1] (Weymouth & Yue: 1–12 % per period) |
| `still_water_at_rest` | ≤ 10⁻⁹ m/s with the surface across a level jump | full p (no subtraction) |
| `standing_wave` | ω within 1 % of ω² = gk tanh(kh); decay within 10 % of 2νk² only if kh ≳ 3, otherwise against a fine-grid run | depth halved: ω moves as tanh(kh) predicts |
| `dam_break` | n² = 2: front Z(T) within max(5 %, one cell) of Martin & Moyce Table 2 after shifting the time origin so that Z = 1.44 matches; maximum front speed 1.6–1.7 √(g h₀) | g scaled by 0.8 (the front moves 11 % in T); or the origin left at t = 0 |
| `floating_box_draught` | equilibrium draught within 1 % (cut cells), with the body's mass matrix built from the same cut-cell volumes as the pressure weights | box density +10 %: draught +10 %; or an exact (analytic) mass matrix with the cut-cell weights, which Batty et al. say breaks hydrostatic rest |
| `steady_river_silent` | phase standard deviation over a simulated hour < 1.2 × 10⁻⁶ rad | inflow +1 %: a visible level change |
| the S9 properties for water | fully refined = uniform; no step on regrid | as S9 |

## What I could not verify

After the second reading (25 Nov 2026, from the owner's 31 PDFs):

- **Scans and OCR.** Martin & Moyce (1952), de Vahl Davis (1983), Fusegi et al. (1991 Tables 1–2),
  Harlow & Welch (1965) and Prosperetti (1981) are scans. I read de Vahl Davis Table V, Fusegi Tables
  1–2 and Martin & Moyce Tables 1, 2 and 6 from rendered page images, once each, and cross-checked
  where a second source exists: Hortmann's stated differences from de Vahl Davis (0.08 / 0.06 / 0.2 %)
  match Table V's Nu at Ra 10⁴ and 10⁵ (0.08 and 0.06 %) but at Ra 10⁶ Table V gives 0.28 %, not
  0.2 % (I read 8.800 clearly; the difference may be to his Nu₀ = 8.817); Ghia's Tables I, II and V and
  Hortmann's Table IV from text and images, with Botella & Peyret's Tables 6 and 11 as an independent
  reprint of Ghia's extrema, which agree. **Martin & Moyce's Tables 3, 4, 5 and 7 were
  not read**, and the Table 2 mean column for a = 2¼ in has an ambiguous row alignment, so it is not
  quoted. The tabulated T(Z) values in §11 are the ones I could read cleanly.
- **Skimmed, not read end to end.** Losasso 2006 (only §3.3), Popinet 2003 and 2009 (the sections cited),
  Popinet 2018 (§3–4), Setaluri et al. (§3, §4, §5 and the performance tables), Rohde et al. and
  Schornbaum & Rüde (the conservation, validation and performance passages; no check of their
  derivations), Fuster & Popinet (§1 and §4.1.1), Hu et al. (the timing tables), Bender & Koschier
  (abstract, introduction, Fig. 1), Gibou et al. (abstract and introduction), Francois et al.
  (abstract and §2), Bell, Colella & Glaz and Chorin (abstract and introduction), Harlow & Welch
  (page 2183), Prosperetti (pp. 1–2). Enright et al.'s Zalesak-disk tables came out of the OCR too
  scrambled to quote, so only the text figures (§3.5) are used.
- **Still secondhand [S].** Every quotation from Basilisk's source documentation (`vof.h`, `poisson.h`,
  `centered.h`, `all-mach.h`), FluidX3D, MDN and the WGSL specification; Aanjaneya et al. 2017 and
  the other papers in the "read in full" list below were read in the first pass and not re-read.
- **Contradictory print in the sources.** Botella & Peyret's Table 6 attributes ω = 2.0677 to Ghia's
  129² run, while Ghia's Table V prints 2.04968. I could not tell which is the slip. Fusegi's
  Ra for the 32³ grid is ambiguous in the OCR (it reads "Ra = 10′"); I took it to be Ra 10³.
- **Every GPU cost in §9.** Estimates from memory traffic, unmeasured. The tree overheads quoted
  from Popinet 2015 and Setaluri et al. are CPU numbers.
- **The LBM density-artefact size.** Derived from p = c_s²ρ, not measured.
- **Whether Losasso-form T-junctions pass the plume test**, in either the 2004 or the 2006 form.
  Unknown until S9 measures it. Losasso 2006's second-order result is on uniformly refined test
  grids, which the paper itself says are not graded (p. 1008).
- **Whether an f32 pressure solve can reach the divergence residual §6 needs.** Weymouth & Yue's
  volume loss of 10⁻¹² per step used a PCG solve in double precision.
- **Gray & Giorgini's limits for an open hall.** Derived for a Rayleigh–Bénard layer (§8a).
- **A tight 3-D reference.** Fusegi et al. are good to a percent or two. A more accurate 3-D cubic
  cavity benchmark exists (Tric et al. 2000) and was not fetched.

## Further questions (not chased)

1. Build a 2D quadtree version of `cavity.py` with the Losasso 2004 and 2006 stencils, to measure the
   spurious current at a T-junction for a horizontally varying density and the order on a *graded*
   grid, before S9 starts. (Same question as before, now with two stencils to compare.)
2. Find the published water benchmark at Pr 7 for the S10 tank (the heat note chose Michalek et al.
   2005 for the inversion case).
3. Read Basilisk's GPU port (its multigrid already runs red-black on GPUs) for brick layout and
   smoother choice. Basilisk is GPL [R], so read it; do not copy.
4. Decide whether sub-cycling in time (a flux register per level) is needed in S9 or can wait.
5. Decide whether the river scene needs inflow and outflow boundary conditions beyond VOF fixed-flux
   faces.
6. Measure, in the S8 uniform solver, the pressure tolerance a single-precision MGPCG reaches and
   the volume drift it implies for the Weymouth–Yue scheme (the account Σ c_c ε V Δt of §6).
7. Fetch a tighter 3-D cubic-cavity benchmark: Tric, Labrosse & Betrouni 2000
   (https://doi.org/10.1016/S0017-9310(00)00037-5, Crossref title match, not read).
8. Check that the S13 heat note's θ is a potential-temperature anomaly (§8a).

## Papers

**Read in the primary text (second pass, from the owner's fetched PDFs):** de Vahl Davis 1983
(https://doi.org/10.1002/fld.1650030305), Hortmann, Perić & Scheuerer 1990
(https://doi.org/10.1002/fld.1650110206), Fusegi et al. 1991 (https://doi.org/10.1016/0017-9310(91)90295-P),
Ghia, Ghia & Shin 1982 (https://doi.org/10.1016/0021-9991(82)90058-4), Botella & Peyret 1998
(https://doi.org/10.1016/S0045-7930(98)00002-4), Martin & Moyce 1952 (https://doi.org/10.1098/rsta.1952.0006),
Chorin 1968, Harlow & Welch 1965, Bell, Colella & Glaz 1989, Losasso, Gibou & Fedkiw 2004,
Losasso, Fedkiw & Osher 2006, Popinet 2003, 2009, 2015 and 2018, Fuster & Popinet 2018, Hirt & Nichols
1981, Weymouth & Yue 2010, Enright et al. 2002, Francois et al. 2006, Gibou et al. 2002, Batty,
Bertails & Bridson 2007, Ng, Min & Gibou 2009, Setaluri et al. 2014, Schornbaum & Rüde 2016 (full text,
not only the abstract), Rohde et al. 2006, Körner et al. 2005, Bender & Koschier 2015, Hu et al. 2018,
Prosperetti 1981, Gray & Giorgini 1976. (Their DOIs, all checked against Crossref in the first pass,
are in `fluids/results/doi_check.txt`.)

**Read in full text in the first pass:** Aanjaneya et al. 2017
([doi](https://doi.org/10.1145/3072959.3073625)), McAdams et al. 2010 (no DOI found;
[PDF](https://math.ucdavis.edu/~jteran/papers/MST10.pdf)), Jiang et al. 2015
([doi](https://doi.org/10.1145/2766996)), Burstedde et al. 2011 ([doi](https://doi.org/10.1137/100791634)),
Lehmann et al. 2022 ([doi](https://doi.org/10.1103/PhysRevE.106.015308)), Gjesdal et al. 2003
(arXiv:physics/0305049), and Erturk & Dursun 2007 (arXiv:physics/0505121). Gjesdal and Erturk are
now superseded by the primary tables for the cavity values.

**Cited and still unread:** Tric, Labrosse & Betrouni 2000 (above); Morton, Taylor & Turner 1956 (the
plume law; no DOI located by a Crossref title search); Michalek et al. 2005 (the heat note's water
inversion case); Johnson & Patel 1999 (flow past a sphere, https://doi.org/10.1017/S0022112098003206,
cited by Ng et al.); Lamb's *Hydrodynamics* (the 2νk² decay).
