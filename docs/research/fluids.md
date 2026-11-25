# Fluids for S8–S10: one solver for air and water

*Research note for slices S8 (fluids: air), S9 (the adaptive grid) and S10 (water) of `plan/REBUILD.md`
§9 and §9.1a. GPU-free: no shader ran. Written 25 Nov 2026. Scripts and raw output: `fluids/`
(`python fluids/cavity.py RA N`, `python fluids/signals.py`; numpy only).*

## 1. Summary

**Recommendation: one finite-volume MAC projection solver.** Use velocities on faces and scalars at
centres; solve the pressure by multigrid-preconditioned conjugate gradients; put buoyancy in Boussinesq
form, with the hydrostatic part removed. Store it as a 2:1-balanced pyramid of sparse uniform levels.
Advect every scalar the instrument reads (temperature, the liquid's volume fraction) in **conservative
flux form**. Give water a **geometric volume-of-fluid (VOF)** surface, not a level set. Couple solids
through face and volume fractions. Build the pressure operator with an optional Helmholtz term, so
compressibility can be switched on later without a second solver. Reject lattice Boltzmann (LBM), SPH,
FLIP/APIC and MPM as the main solver. The reasons, in order of weight:

1. **The towers read mass, so the fluid must hold mass exactly where it is.** One gram of water 1 m
   below a tower is **3.1 × 10⁻⁴ rad**, six shots' worth. Over an hour's mean, **4 g** at 1 m and
   **16 g** at 2 m are detectable [M, §3]. A cubic-metre tank must therefore keep its mass to about
   10⁻⁶ of itself. Flux-form VOF conserves exactly when the velocity is discretely divergence-free
   ([Basilisk `vof.h`](http://basilisk.fr/src/vof.h) [S]; Weymouth & Yue 2010). A level set does not.
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
   on the proposed MAC discretisation (§10). It converges at second order, its Richardson extrapolation
   reaches the references within 0.1 %, and the projection leaves divergence at 10⁻¹³. "Fully refined =
   uniform" is a bit-level property of a MAC octree with a uniform special case. LBM has no such
   special case at refinement interfaces: they need dedicated mass-conserving treatment (Rohde et al.
   2006).
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
- The S8 exit "de Vahl Davis Nu within 1 %" needs **≥ 64 cells across at Ra 10⁴, about 100 at 10⁵,
  and about 140–160 at 10⁶** on a uniform grid [M]. At 32² the error is 1.3 %, 5 % and 19 %. v2's
  32² run passed at Ra 10³–10⁴ only.
- At Ra 10⁶ the references disagree by 0.3 %: 8.800 (de Vahl Davis) against 8.825 (Hortmann et al.
  1990) ([Gjesdal et al., Table 1](https://arxiv.org/pdf/physics/0305049) [P]). The brief should name
  one, preferably the later.

**Other changes to the plan.**
- The free surface becomes VOF, not "a level set on the same octree" (§6).
- The lid-driven reference becomes Botella & Peyret 1998, with Ghia 1982 as a second line (§11).
- Losasso et al. 2004's first-order T-junction pressure is known to produce parasitic currents "even
  for hydrostatic scenarios" ([Aanjaneya et al. 2017, §1](https://pages.cs.wisc.edu/~sifakis/papers/power_diagrams_2017.pdf) [P]).
  The "at rest on a graded grid" test therefore needs a control arm that shows the failure, and a
  stated fallback: the power-diagram discretisation (§5).

## 2. How to read the evidence

Tags as in the heat note:
- **[P]**: read in the primary source (full text extracted here);
- **[S]**: a summary of a primary page or document (official documentation, source-code docs, a
  table quoted by another paper);
- **[R]**: recalled, **unverified**;
- **[D]**: derived here by hand;
- **[M]**: measured or computed here (`fluids/`).

Every DOI below was checked against Crossref for title and year
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
| **LBM** (incl. thermal DDF, octree, free surface) | exact on a uniform grid; refinement interfaces need special care (Rohde 2006) | density is p/c_s² with artificial c_s: Ma² artefacts; read mass from T or fill instead | very fast per cell (1,696 MLUPs/s on the GTX 1660 SUPER, FP32), 55–153 B/cell; time step tied to dx | waLBerla: ~10¹² cell updates/s on non-uniform grids (Schornbaum & Rüde 2016) [S] | free-surface LBM with VOF-like mass exchange (Körner 2005) [R] | weakly compressible by construction; physical sound speed costs 70× (§9) |
| **FLIP / PIC / APIC** | particle mass exact; volume drifts (clumping) | density from particle counts: noisy | particles + grid; GPU-friendly; sorting costs | grid adaptive possible | natural (particles) | no |
| **MPM / MLS-MPM** | particle mass exact | as FLIP; weakly compressible fluids | GPU-friendly (Hu 2018) [R] | sparse grids | natural | weakly |
| **SPH** (WCSPH, IISPH, DFSPH) | particle mass exact | kernel density error ~0.01–1 %, noisy [R] | neighbour search, ~30–50 neighbours | particle splitting, hard | natural | weakly, or iterative incompressible |

### 4.1 MAC projection (the recommendation)

- **Placement.** Pressure at centres and normal velocities on faces. The discrete divergence and
  gradient are adjoint, so the projection is exact to the solver tolerance and has no checkerboard
  mode. This is the Harlow–Welch staggered grid with Chorin's projection.
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
  per second ([arXiv:1508.07982](https://arxiv.org/abs/1508.07982) [S]).
- **Licence.** FluidX3D is free for non-commercial use only [S], so it is a reference, not a
  dependency.

Against it, for Cavendish:

1. **Its density is not the fluid's density.** LBM's pressure is p = c_s²ρ with an artificial lattice
   sound speed, so dynamic pressure appears as δρ/ρ ~ u²/c_s² ~ Ma² [D]. That is 10⁻³–10⁻² at the
   usual lattice Ma of 0.03–0.1. For air the 1 K signal is 3.4 × 10⁻³ of ρ, so the LBM density must
   not be read. For water it would be 1–10 kg m⁻³ of spurious mass. The mass read has to come from T
   or from a fill fraction anyway, so LBM brings no read-side advantage.
2. **Refinement interfaces.** Interfaces between levels need explicit treatment to conserve mass
   (Rohde et al. 2006 [R]; Schornbaum & Rüde 2016 say the interface "must receive special treatment"
   [S]). With that treatment, "fully refined = uniform" is not structural.
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
- **SPH.** SPH needs particles throughout the air, which is hopeless at 10⁹ cells. Its kernel density
  is noisy, and adaptivity is hard.
- **Where particles fit.** Keep MPM for granular matter (the Noita row of `REBUILD.md` §5.3). APIC
  could be a *visual* spray layer, labelled as such.

## 5. The adaptive octree (S9)

- **Grid.** 2:1-balanced octree, stored as a pyramid of sparse uniform levels (SPGrid, Setaluri et al.
  2014). This is the layout Aanjaneya et al. use for adaptive liquids. They restrict grading so that
  "a cell and all its neighbors span only two levels of resolution" (§8) [P].
- **Balance algorithm.** p4est's 2:1 balance on Morton-ordered octants scales to 5.13 × 10¹¹ octants,
  "in less than 10 seconds per million octants per process"
  ([Burstedde et al. 2011, abstract](https://p4est.github.io/papers/BursteddeWilcoxGhattas11.pdf) [P]).
  Cavendish needs the single-octree, single-GPU version of the same algorithm on the Morton keys it
  already has ([`s5-world-octree.md`](s5-world-octree.md)). The fluid's active region is 10⁶–10⁷
  cells, not 10¹¹.
- **Pressure at T-junctions.** There are three published options:
  1. *Losasso, Gibou & Fedkiw 2004.* Flux-form, symmetric positive definite, first order at
     T-junctions. "The price for this choice was the loss of orthogonality between the stored velocity
     components at the T-junction cell faces and the corresponding discrete pressure gradients … gives
     rise to non-physical parasitic currents at T-junctions even for hydrostatic scenarios"
     (Aanjaneya et al. §1, citing five papers) [P].
  2. *Power diagrams* (Aanjaneya et al. 2017). Symmetric positive definite, second order, works at
     solid and free-surface boundaries. The same storage, plus geometry tables per T-junction
     configuration [P].
  3. *Collocated cell-centred with an approximate projection* (Gerris/Basilisk). The faces are
     projected exactly, and the tree residual uses a "conservative coarse/fine discretisation (2nd
     order)" ([`poisson.h`](http://basilisk.fr/src/poisson.h) [S]). The matrix is not symmetric, so
     it needs plain multigrid rather than PCG.

  **Choose option 1 first**, as `reference/design/voxel.md` §9.2 already plans, for two reasons. It is
  the simplest, and fluxes match across level jumps by construction, so mass is conserved. Combine it
  with **hydrostatic subtraction**: write the pressure as p = p_ref(z) + p′, with ρ_ref(z) the
  horizontally uniform reference column, and drive buoyancy by ρ − ρ_ref. A horizontally uniform
  stratification is then exactly at rest with any stencil [D].

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
| Level set (Osher–Sethian; Losasso 2004's surface) | **not conserved**: reinitialisation and advection move the zero contour | smooth normals and curvature |
| Particle level set (Enright et al. 2002) | much better, not exact | the 2002–2006 graphics standard [R] |
| **Geometric VOF/PLIC** (Hirt & Nichols 1981; Weymouth & Yue 2010) | **exact** for a discretely divergence-free face velocity, with CFL < 0.5 | Basilisk: "guarantees exact mass conservation for the multi-dimensional advection scheme (provided the advection velocity field is exactly non-divergent)" ([`vof.h`](http://basilisk.fr/src/vof.h) [S]) |
| CLSVOF / VOF-derived level set | as VOF | a distance field from VOF for normals, if ever needed |

**Choose VOF.**
- It is the only one of these that conserves water to the gram (§3).
- Its unknown, the fill fraction, is exactly what the gravity read needs: cell mass = ρ_w(T) · f · h³.
- It matches the world volume's "volume fraction exact" stamping (`REBUILD.md` §5.2).

Surface tension is irrelevant at these scales (capillary length about 2.7 mm [R]), so VOF's weak
curvature is no cost. Basilisk has shown adaptive VOF with multigrid on quadtrees and octrees at length
(Popinet 2009; Popinet 2018 review), but I could not read either.

**One condition matters.** Exactness requires the face velocity to be exactly divergence-free. An
iterative pressure solve leaves a residual ε, and the VOF step then over- or under-fills cells by
about ε · Δt. In flux form the total still telescopes exactly. **Clipping f to [0, 1] does not**, and
must be replaced by redistribution, recorded in the ledger [D]. Test it (§12).

**Gravity and the surface.** With p = p_ref + p′ and the free surface as a Dirichlet condition for p′
(the ghost-fluid treatment of Gibou et al. 2002), still water is exactly at rest [D].

## 7. Solids, bodies and heat

- **Solids in the pressure solve.** Use face-area fractions and volume fractions in the pressure
  matrix: a symmetric positive definite matrix and a well-behaved pressure at the boundary (Batty,
  Bertails & Bridson 2007; Ng, Min & Gibou 2009 [R]). This replaces the voxel staircase, which is
  first order and makes surface area 1.5× too large ([`s13-heat.md`](s13-heat.md), staircase row).
- **The air's mass.** The fluid volume of a cell is (1 − φ_solid) h³, so the air displaced by a body
  matches the body's stamped volume. That consistency is what the moving-ball oracle checks.
- **Two-way coupling** (S10: the floating box, boats). Monolithic coupling puts the rigid body's
  velocity in the pressure solve (Batty et al. 2007 [R]). It avoids the added-mass instability of
  partitioned coupling for bodies lighter than the water they move [R]. The floating box then reaches
  equilibrium at the Archimedes draught through the same pressure that holds the water at rest.
- **Heat.** The same flux-form advection carries θ. Conjugate transfer, flux registers and film
  conductances are as in the heat note §3.2. The multigrid is shared.

## 8. Compressibility and pressure waves

- **Low-Mach split.** Split the pressure into a uniform thermodynamic p₀(t) and a dynamic part (heat
  note §3.4). This captures the sealed-room effect, the only compressible effect inside a building
  that the towers can see (§3).
- **The optional Helmholtz term.** Build the pressure operator as ∇·(β∇p) + λp, with λ = −1/(ρc²Δt²)
  set to zero by default. That is Basilisk's all-Mach formulation, in which "by default the fluid is
  incompressible" and a finite ρc² adds the λ term to the same Poisson–Helmholtz multigrid
  ([`all-mach.h`](http://basilisk.fr/src/all-mach.h) [S]; Fuster & Popinet 2018 [R]). It is implicit,
  so it carries no acoustic CFL limit. If a far-field infrasound wave or a building-scale pressure
  wave is ever wanted on the grid, it costs no new solver.
- **Explicit compressible solvers.** These, and LBM at the physical sound speed, need Δt ≤ h/c, which
  is 1.5 × 10⁻⁴ s at 5 cm, for frequencies the interferometer filters out. Rejected.

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
  about real time.**
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
    in bricks (SPGrid's "several distinct channels within a 4KB page", Aanjaneya §2.1 [P]), and
    request the adapter's higher limits where available.
  - One f32 field of 3.3 × 10⁷ cells fills a default binding.

## 10. Measured: the de Vahl Davis cavity on the S8 discretisation [M]

**The solver** (`fluids/cavity.py`, 2D, numpy):
- MAC grid; second-order conservative central advection (the Harlow–Welch form); explicit diffusion.
- Boussinesq buoyancy on vertical faces.
- Exact Neumann projection by a cosine transform, applied at each SSP-RK3 stage.
- Pr = 0.71; hot wall +½, cold wall −½, adiabatic top and bottom.
- Run until max |∂T/∂t| < 10⁻⁵.
- Nu from the second-order one-sided wall gradient, averaged over the wall.

**References.** de Vahl Davis 1983 for Ra 10⁴–10⁶ is quoted in
[Gjesdal et al. 2003, Table 1](https://arxiv.org/pdf/physics/0305049) [P]: 2.243 / 4.519 / 8.800, with
Hortmann et al. 1990 at 2.245 / 4.522 / 8.825. The Ra 10³ value 1.118 is v2's recorded reference [R].

| Ra | N | Nu (hot = cold) | error vs de Vahl Davis | u_max, v_max on midlines | Richardson (64, 128) | wall time |
|---|---|---|---|---|---|---|
| 10³ | 32 / 64 / 128 | 1.1201 / 1.1184 / 1.1179 | +0.19 / +0.03 / −0.01 % | 3.650, 3.698 | 1.1177 | 3 / 39 / 348 s |
| 10⁴ | 32 / 64 / 128 | 2.2725 / 2.2512 / 2.2463 | +1.31 / +0.37 / +0.15 % | 16.18, 19.63 | 2.2447 | 3 / 33 / 304 s |
| 10⁵ | 32 / 64 / 128 | 4.7453 / 4.5697 / 4.5326 | +5.0 / +1.12 / +0.30 % | 34.76, 68.67 | 4.5202 | 4 / 29 / 262 s |
| 10⁶ | 32 / 64 / 128 | 10.491 / 9.184 / 8.901 | +19.2 / +4.37 / +1.15 % | 64.99, 220.6 | 8.807 | 6 / 32 / 205 s |

**What it shows.**
- **Order.** Errors fall by 3.5–4.4 per halving of h: **second order**.
- **Extrapolations.** These land within 0.1 % of the references: 8.807 against 8.800 and 8.825.
- **The velocity maxima.** These match the de Vahl Davis values I recall (3.649/3.697, 16.18/19.62,
  34.73/68.59, 64.63/219.4 [R]) to within 0.6 %. That is consistent but not proof, since I have not
  read his tables.
- **The discretisation is sound.** The S8 exit is a question of resolution.
- **Grid for 1 % (error ∝ N⁻²).** Ra 10⁴ needs N ≥ 40, 10⁵ needs N ≥ 70 and **10⁶ needs N ≥ 140**,
  against de Vahl Davis. Against Hortmann, 128 already passes at 0.87 %.
- **Recommended exit.** State the reference and use **N = 64, 128, 256 and Richardson**. Pass if the
  finest grid is within 1 % *and* the extrapolation is within 0.3 % of Hortmann et al.
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
| Differentially heated cavity (de Vahl Davis 1983) | Nu 1.118, 2.243, 4.519, 8.800 for Ra 10³–10⁶, Pr 0.71; Hortmann et al. 1990: 2.245, 4.522, 8.825 | 10⁴–10⁶ [P via Gjesdal]; 10³ [R]; reproduced [M] | S8, S9 |
| Cubic cavity, 3D (Fusegi et al. 1991) | Nu by Ra | not read [R] | S8 (3D) |
| Lid-driven cavity, Re 1000 | **Botella & Peyret 1998** (spectral): ψ_max 0.1189366, ω 2.067753 at (0.5308, 0.5652); Ghia et al. 1982 (129²): about 0.1179 at (0.5313, 0.5625) | Botella [P via Erturk & Dursun 2007, Table 1](https://arxiv.org/pdf/physics/0505121); Ghia [R] | S8 (tests momentum advection, which the cavity does not stress) |
| Rayleigh–Bénard onset | Ra_c = 1707.762, k_c = 3.1163 for rigid plates | [M] in [`s13-heat.md`](s13-heat.md) | S8: growth-rate sign change within 1 % |
| Dam break (Martin & Moyce 1952) | surge-front position against time for a collapsing water column, non-dimensional Z = z/a against T = t√(2g/a) [R]; simulations usually lead the data early because of the gate | scanned Part IV is online, [NTU copy](https://www.math.ntu.edu.tw/~shyue/orch08/benchmarks/lowmach/broken-dam/91519.pdf), not read | S10, a few % on front position |
| Standing wave / sloshing | ω² = gk tanh(kh); viscous decay 2νk² (Lamb) for small amplitude; Prosperetti 1981 gives the exact initial-value solution | closed form [D]; Prosperetti [R] | S10: frequency within 1 %, and the gravity read of the sloshing mass |
| Archimedes floating | draught d = (ρ_b/ρ_w)·H for a box | closed form | S10 |
| Acoustic pulse | d'Alembert: a Gaussian travels at c = √(γp/ρ) = 343.2 m/s [M], unchanged in 1D | closed form | only if the Helmholtz term is switched on |
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
| `heated_cavity_2d` | Nu at Ra 10³–10⁶ within 1 % on the finest of 64/128/256, Richardson within 0.3 % of Hortmann | N = 32 at Ra 10⁶ (+19 % measured) |
| `cavity_second_order` | error ratio 3.5–4.5 per halving | first-order upwind θ advection (ratio ≈ 2) |
| `lid_driven_cavity` | ψ_max at Re 1000 within 1 % of Botella & Peyret | viscosity doubled |
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
| `stratified_at_rest_graded` | ≤ 10⁻⁹ m/s on a deliberately graded grid | hydrostatic subtraction off: spurious current at T-junctions above threshold |
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
| `tank_mass_ledger` | total water bit-identical through sloshing and regrids | clip f to [0, 1] |
| `still_water_at_rest` | ≤ 10⁻⁹ m/s with the surface across a level jump | full p (no subtraction) |
| `standing_wave` | ω within 1 % of linear theory; decay within 10 % of 2νk² | depth halved: ω moves as tanh(kh) predicts |
| `dam_break` | front position within a few % of Martin & Moyce after the gate transient | — |
| `floating_box_draught` | equilibrium draught within 1 % (cut cells) | box density +10 %: draught +10 % |
| `steady_river_silent` | phase standard deviation over a simulated hour < 1.2 × 10⁻⁶ rad | inflow +1 %: a visible level change |
| the S9 properties for water | fully refined = uniform; no step on regrid | as S9 |

## What I could not verify

- **Papers I could not open** (list below). I could not read Losasso et al. 2004, Gerris 2003, the VOF
  papers or Batty 2007. What this note says of them comes from Aanjaneya et al.'s account of Losasso
  2004 [P], from Basilisk's source documentation [S], or from memory [R].
- **The de Vahl Davis Ra 10³ value (1.118) and the velocity maxima.** These are recalled. My solver's
  extrapolation agrees with them to 0.03 % and 0.6 %.
- **Ghia's numbers.** Recalled; Botella & Peyret's were read in a table.
- **Martin & Moyce's data and non-dimensionalisation.** Recalled.
- **Every GPU cost in §9.** Estimates from memory traffic, unmeasured.
- **The LBM density-artefact size.** Derived from p = c_s²ρ, not measured.
- **Whether Losasso-form T-junctions pass the plume test.** Unknown until S9 measures it.

## Further questions (not chased)

1. Build a 2D quadtree version of `cavity.py` with the Losasso stencil, to measure the spurious current
   at a T-junction for a horizontally varying density, before S9 starts.
2. Find the published water benchmark at Pr 7 for the S10 tank (the heat note chose Michalek et al.
   2005 for the inversion case).
3. Read Basilisk's GPU port (its multigrid already runs red-black on GPUs) for brick layout and
   smoother choice. Basilisk is GPL [R], so read it; do not copy.
4. Decide whether sub-cycling in time (a flux register per level) is needed in S9 or can wait.
5. Decide whether the river scene needs inflow and outflow boundary conditions beyond VOF fixed-flux
   faces.

## Papers I could not read

- de Vahl Davis 1983: https://doi.org/10.1002/fld.1650030305
- Hortmann, Perić & Scheuerer 1990: https://doi.org/10.1002/fld.1650110206
- Fusegi et al. 1991: https://doi.org/10.1016/0017-9310(91)90295-P
- Ghia, Ghia & Shin 1982: https://doi.org/10.1016/0021-9991(82)90058-4
- Botella & Peyret 1998: https://doi.org/10.1016/S0045-7930(98)00002-4
- Martin & Moyce 1952: https://doi.org/10.1098/rsta.1952.0006
- Chorin 1968: https://doi.org/10.1090/S0025-5718-1968-0242392-2
- Harlow & Welch 1965: https://doi.org/10.1063/1.1761178
- Bell, Colella & Glaz 1989: https://doi.org/10.1016/0021-9991(89)90151-4
- Losasso, Gibou & Fedkiw 2004: https://doi.org/10.1145/1015706.1015745
- Losasso, Fedkiw & Osher 2006: https://doi.org/10.1016/j.compfluid.2005.01.006
- Popinet 2003 (Gerris): https://doi.org/10.1016/S0021-9991(03)00298-5
- Popinet 2009: https://doi.org/10.1016/j.jcp.2009.04.042
- Popinet 2015: https://doi.org/10.1016/j.jcp.2015.09.009
- Popinet 2018: https://doi.org/10.1146/annurev-fluid-122316-045034
- Fuster & Popinet 2018: https://doi.org/10.1016/j.jcp.2018.07.055
- Hirt & Nichols 1981: https://doi.org/10.1016/0021-9991(81)90145-5
- Weymouth & Yue 2010: https://doi.org/10.1016/j.jcp.2009.12.018
- Enright et al. 2002: https://doi.org/10.1006/jcph.2002.7166
- Francois et al. 2006: https://doi.org/10.1016/j.jcp.2005.08.004
- Gibou et al. 2002: https://doi.org/10.1006/jcph.2001.6977
- Batty, Bertails & Bridson 2007: https://doi.org/10.1145/1276377.1276502
- Ng, Min & Gibou 2009: https://doi.org/10.1016/j.jcp.2009.08.032
- Setaluri et al. 2014 (SPGrid): https://doi.org/10.1145/2661229.2661269
- Schornbaum & Rüde 2016 (abstract only): https://doi.org/10.1137/15M1035240
- Rohde et al. 2006: https://doi.org/10.1002/fld.1140
- Körner et al. 2005: https://doi.org/10.1007/s10955-005-8879-8
- Bender & Koschier 2015 (DFSPH): https://doi.org/10.1145/2786784.2786796
- Hu et al. 2018 (MLS-MPM): https://doi.org/10.1145/3197517.3201293
- Prosperetti 1981: https://doi.org/10.1063/1.863522
- Gray & Giorgini 1976: https://doi.org/10.1016/0017-9310(76)90168-X

**Read in full text here:** Aanjaneya et al. 2017
([doi](https://doi.org/10.1145/3072959.3073625)), McAdams et al. 2010 (no DOI found;
[PDF](https://math.ucdavis.edu/~jteran/papers/MST10.pdf)), Jiang et al. 2015
([doi](https://doi.org/10.1145/2766996)), Burstedde et al. 2011 ([doi](https://doi.org/10.1137/100791634)),
Lehmann et al. 2022 ([doi](https://doi.org/10.1103/PhysRevE.106.015308)), Gjesdal et al. 2003
(arXiv:physics/0305049), and Erturk & Dursun 2007 (arXiv:physics/0505121).
