# Heat everywhere: methods, reference numbers and signal sizes for S13

*Research note for slice S13 (`plan/REBUILD.md` §9.2). GPU-free: no shader ran. Written 22 Nov 2026.
Scripts and raw output: `s13-heat/` (run `python3 -I checks.py all`).*

## 1. Answer in brief

**Methods (one recommendation each; reasons in §3).**

| Question | Recommendation |
|---|---|
| Conduction in solids | Conservative finite volumes on the voxel tree (flux form, harmonic-mean face conductance), advanced by **RKL2 super-time-stepping**. Implicit multigrid only as the steady-state oracle. Energy is conserved to round-off at every stage; no dot products, no solver tolerance, bit-deterministic. |
| Conjugate heat transfer | **One energy equation on one set of voxels**; each solid–fluid face carries a conductance, either `Resolved` (half-cell conduction) or `Film` (correlation). Two-rate time stepping with a flux register, as AMR does. No separate solid and fluid solvers exchanging boundary values. |
| Radiation | **Net-radiation method on coarse patches, view factors by Monte-Carlo rays through the coarse occupancy volume**, cached and refreshed at the thermal rate, coupled to conduction by a linearised radiative conductance with a Newton update. Reject hemicube and discrete ordinates. |
| Air beyond Boussinesq | Keep Boussinesq dynamics but read the **exact ideal-gas anomaly**. A guard at ΔT/T₀ > 0.1 switches to low-Mach variable density. The room's *closure* (sealed or leaky) matters more than either (§3.4). |
| Water | **A tabulated IAPWS-95 density at 1 K nodes with a cubic spline**: error 0.001 g/m³ over 0–95 °C. Maximum 3.978 °C, 999.9749 kg/m³. A quartic fit only to reproduce the published inversion benchmark. |
| Thermal expansion | Quasi-static linear thermoelasticity on S7's voxel finite elements, **trilinear hexahedra with incompatible modes** (or at least 4 cells through any thin member). Mass by parcel relocation; first-order read by response map. |

**What matters to the gravity gradiometer (§5, estimated here, factor 2–3).** With 5 × 10⁻⁵ rad per shot (and 10⁻⁶ rad
in an hour's mean) as the thresholds, the ranking is not the one the plan implies. Plumes are small:

| Effect | Phase |
|---|---|
| a 100 W person's plume, 1 m / 2 m from the tower | 6 × 10⁻⁵ / 7 × 10⁻⁶ rad |
| a 500 W convective radiator plume, 1 m | 3 × 10⁻⁴ rad |
| **mean temperature of the air in the hall**, per kelvin | **2 × 10⁻² rad**, up to 8 × 10⁻² |
| **a sunlit 10 × 10 × 0.3 m concrete wall** expanding freely, 5 m away | 4 × 10⁻² rad, 0.3 rad if it bows |
| **a 1 m³ open water tank 20 → 60 °C**, 2 m away | 4 × 10⁻² rad (5 × 10⁻⁴ rad per kelvin) |

The bulk air temperature, structure that moves by a millimetre and the volume of water are the signals;
the plume is a detail. A second consequence: whether a room is *sealed* or *leaky* changes the air signal
from near zero to 2 × 10⁻² rad per kelvin, and no benchmark in the plan decides it.

**Corrections to `VALIDATION-AND-RESEARCH.md` §2.5 (§4).** Two rows are wrong as written
(`heated_infinite_slab_is_silent`, `radiator_anchor`), two are weak (they pass when the property is broken),
ten need a corrected statement, and two cannot be given numbers until their tables are read.
`closed_box_conserves_energy` cannot hold to round-off with the S8 advection scheme. Many reference numbers
are from memory and flagged (§8).

**Evidence.** I could not open any paper (network limits, §2). What is firm was computed here:
Rayleigh–Bénard critical numbers, view factors, Timoshenko's bimetal formula, the thermoelastic sphere,
IAPWS-95 densities, element locking. Reference numbers from papers are tagged by how I came by them.

## 2. How to read the evidence

The session's network reached GitHub, PyPI, npm and conda, plus web search that returns summaries.
No paper, DOI resolver, NIST or arXiv page opened. Tags used throughout:

- **[M]** measured or computed here, `s13-heat/checks.py`, output in `s13-heat/results/`;
- **[S]** seen in a search summary or package documentation (secondary; link given);
- **[R]** recalled from memory, **unverified**: to be checked against the paper before a tolerance is written;
- **[D]** derived here by hand, to be checked in the slice.

Where a primary DOI appeared in search text it is linked; where I knew only the citation it is given without a link.

## 3. Question 1 — methods at Cavendish's scales

Scales: rooms 10–10³ m³ to a town of 10⁹–10¹¹ voxels at 1–10 cm, temperatures held in `f32` on the GPU, a CPU
`f64` twin for every kernel. Heat is slow: a 0.3 m concrete wall has L²/α ≈ 1.3 × 10⁵ s [D], so the stiff
part is small, and most cells are ambient and need no heat work at all.

**Numbers that shape the choices** (script `references`, [M]):

| Pitch | Explicit limit h²/6α: concrete | steel | copper | air | water |
|---|---|---|---|---|---|
| 5 cm | 595 s | 32 s | 3.8 s | 19 s | 2976 s |
| 1 cm | 24 s | 1.3 s | 0.15 s | 0.76 s | 119 s |
| 0.5 cm | 6 s | 0.32 s | 0.04 s | 0.19 s | 30 s |

At 5 cm the fluid's own step (CFL, ~0.01–0.1 s) is far below the diffusion limit for every material, so
explicit diffusion is free in air and water. Solids are the other way round: their physics is hours, so they
want steps of 10–1000 s and stability beyond the explicit limit only where pitch is fine or the metal is good.

### 3.1 Conduction in solids

| Option | Order and stability | Conserves energy to round-off? | GPU fit | Validated by |
|---|---|---|---|---|
| Explicit Euler | 1st in time, h²/6α | yes | trivial | erfc, slab |
| **RKL2 super-time-stepping** | 2nd, stable to ≈ (s²+s−2)/2 × explicit step with s stages [R] | **yes**, every stage is flux form | stencil sweeps only, no reductions | erfc, slab; order test |
| Backward Euler / Crank–Nicolson + multigrid-CG | unconditionally stable; CN 2nd | only to the solver tolerance | reductions, V-cycles | same |

Recommendation: **RKL2** for solids ([Meyer, Balsara & Aslam 2014](https://www.osti.gov/scitech/biblio/22230848) [S];
the first-order version is Alexiades, Amiez & Gremaud, Commun. Numer. Meth. Eng. 12, 31 (1996) [S, citation only]; a GPU-oriented use is [arXiv:1702.05487](https://arxiv.org/pdf/1702.05487) [S]).
Reasons: (a) the plan promises energy conserved *to round-off*, which a tolerance-limited multigrid solve
cannot give; (b) no global reductions, so the CPU `f64` twin and the GPU agree up to rounding, and the
result is bit-reproducible; (c) the stage count is set from a bound, not tuned: the symmetric conductance
network has `λ_max ≤ 2 maxᵢ Σⱼ Gᵢⱼ/Cᵢ` by Gershgorin [D], so `s` follows from the largest cell, with no
spectral-radius estimate. Worked case: 1 cm concrete, 600 s step, explicit limit 24 s, ratio 25, `s = 7`.
Keep the multigrid solve as the steady-state oracle for `steady_state_balances` and `plate_fourier_series`;
the fluid's pressure multigrid is reused there, not duplicated.

Precision [D]: store **θ = T − T_ref**, not T. An `f32` at 300 K has resolution 3 × 10⁻⁵ K, so a 0.05 s air step
that changes a cell by 5 × 10⁻⁵ K is lost in rounding; θ near zero keeps 10⁻⁷ K. Energy and mass-anomaly
accumulators in float-float. At the gravity read's sensitivity (2 × 10⁻² rad/K, §5) 3 × 10⁻⁵ K is already 6 × 10⁻⁷ rad.

Staircase [M]: the area of a voxel-face surface that approximates a sphere is **1.4998 ×** the true area
(mean of |nx|+|ny|+|nz| over the sphere, 3/2). Faces on curved surfaces overstate conduction and
convection by up to 50 %. Take surface area and normals from the smoothed volume fraction (|∇φ|, as
`reference/design/physics.md` §5.4, which read 8 % low at four cells' radius), not by counting faces.

### 3.2 Conjugate heat transfer with a MAC Boussinesq fluid

Write one equation, ρc ∂T/∂t + ρc u·∇T = ∇·(k∇T) + q, with u = 0 in solids and k, ρc per cell. Each
face between cells of conductivities kₐ, k_b and half-widths has conductance G = 1/(h/2kₐ + 1/h_f + h/2k_b),
with `1/h_f` present only on solid–fluid faces. This is the same quantity as the plan's `Heat` conditions
(`physics.md` §5.4): `Resolved` is h_f → ∞ with the fluid boundary layer on the grid; `Film` is h_f from a
correlation. A cell's boundary layer is δ_T ≈ L/Nu, so the grid resolves it only when pitch ≲ δ_T/3
(from Churchill–Chu, [M] table in §4: at Ra_L = 10⁷, δ ≈ 0.032 L; at room scale, Ra ~ 10⁹, ~1 cm). Choose per
surface by comparing the two, and have the bundle say which was used. The `Film` coefficient is a model
error of tens of percent (the correlation's own scatter, §4) and must be recorded as such.

Time integration: fluid cells step explicitly at the flow step; solids take RKL2 super-steps. A
solid–fluid face accumulates its flux over the fluid sub-steps in a **flux register** (Berger & Colella 1989
[R]; S9's adaptive grid needs these anyway) and the solid super-step uses the accumulated mean. Both sides
then receive the same energy, so the budget closes exactly. The air is the light side (effusivity ratio
√(kρc) air/concrete ≈ 3 × 10⁻³ [D]): the partitioned Dirichlet-on-the-stiff-side form is stable for such a ratio
(Giles, Int. J. Numer. Methods Fluids 25, 421 (1997) [R, no link found]).
Test it with the Biot-number slab in §4.

### 3.3 Radiation between surfaces

| Option | Cost | Accuracy | Verdict |
|---|---|---|---|
| **Monte-Carlo ray-traced view factors** on a coarse occupancy volume | N_p × R rays; GPU DDA | σ(F)/F = √((1−F)/(F N)), unbiased | **chosen** |
| Hemicube (Cohen & Greenberg 1985 [R]) | one rasterised pass per patch | biased by pixel size; aliasing | rejected: needs a rasteriser per patch, no error control |
| Discrete ordinates (Fiveland 1984 [R]) | N_cells × N_dir everywhere | ray effects, false scattering; ~100 directions | rejected: pays for a participating medium that is out of scope |
| Linearised radiative coefficient alone | none | exact only for small ΔT | used *inside* the chosen method |

**Measured here [M]:** two unit squares at distance 1, `F = 0.199825` (closed form), Monte Carlo
0.19980 ± 0.00007 at 10⁶ samples; perpendicular unit squares with a common edge, `F = 0.200044`, Monte
Carlo 0.1981 ± 0.0015 at 10⁶; four perpendicular faces plus one parallel face of a unit cube sum to 1.000000;
coaxial discs of unit radius at unit distance 0.381966. Uniform area sampling has a diverging variance for
adjacent patches (the integrand falls as 1/r² at the shared edge), so use cosine-weighted rays from each patch.

Why the matrix is manageable: dense F costs 4 N_p² bytes: 8 000 patches (a 10 × 10 × 3 m room at 20 cm) 256 MB,
30 000 patches 3.6 GB. Beyond that, only patches whose temperature differs from ambient matter: the exchange
between surfaces at one temperature is exactly zero, so formulate it as an *anomaly*, with ambient as the
sink. The linearised conductance `4εσT³ = 5.71 ε W m⁻² K⁻¹` at 293 K [M] is the same size as natural convection
(`h_c` a few W m⁻² K⁻¹), so radiation is half of a room's surface exchange and not negligible. The
linearisation error is `(ΔT/2T_m)²` [D, M]: 0.7 % for a 70 °C radiator in a 20 °C room, 6 % at ΔT = 200 K,
so keep σT⁴ exact in a Newton step for hot sources. Emissivity and a grey diffuse surface follow the plan;
the radiator's **radiative fraction** (about a quarter of a panel's output, [R]) decides how much of its
power reaches the plume at once and how much warms the walls first.

### 3.4 Where Boussinesq fails for air, and what replaces it

Boussinesq is a statement about *dynamics*: density varies in the buoyancy term only. The gravity *read*
needs no such approximation: use `ρ = p₀/(R T)`. The error of the *linear* anomaly against the ideal gas is
`ΔT/T` [D, M]: 0.34 % at 1 K, 3.4 % at 10 K, 10 % at 30 K, 17 % at 50 K. That is the second-order share for
`response_map_superposes`, and also the size of the buoyancy error in the boundary layer of a 70 °C surface.
Gray and Giorgini's criteria ([Int. J. Heat Mass Transfer 19, 545 (1976)](https://doi.org/10.1016/0017-9310(76)90168-X))
bound the temperature difference at which Boussinesq holds; I recall a figure near 29 K for air
and could not confirm it [R]. Spiegel and Veronis (1960) add that the depth must be small against the scale
height, which for air is `RT/g ≈ 8.4 km` [D]: fine for a hall (10⁻³), not for a town column (a
12 % density change over 1 km). So at town scale the reference is a hydrostatic column ρ_ref(z) and the
anomaly is taken against it, as `voxel.md` §9.2's well-balanced scheme already suggests.

**Recommendation:** Boussinesq dynamics, exact anomaly in the read, and a guard (`max |ΔT|/T₀ > 0.1`, about 30 K)
that the bundle reports. When tripped, switch to **low-Mach variable density** (Paolucci 1982 [R], with a secondary
description in [arXiv:2006.04400](https://arxiv.org/pdf/2006.04400) [S]): split pressure into a uniform
thermodynamic part p₀(t) and a mechanical part; the divergence becomes
`∇·u = (1/ρc_pT)(∇·k∇T + q) − (1/γp₀) dp₀/dt`; the projection keeps its MAC form with this right-hand side and a
variable-coefficient Poisson operator (multigrid still works). Reference: the non-Boussinesq cavity of
[Le Quéré et al. 2005](https://www.esaim-m2an.org/articles/m2an/pdf/2005/03/m2an10EDP.pdf) (ε = 0.6, Ra = 10⁶) and
[Vierendeels, Merci & Dick 2003](https://emerald.com/insight/content/doi/10.1108/09615530310501957/full/html) [S]; I
could not read their tables.

**The closure matters more.** `p₀` is the choice that sets the air signal:

- **Leaky or open room** (p₀ fixed at ambient): the anomaly is the full −ρ₀ΔT/T and the missing mass is outside. The
  hall's reading is 2 × 10⁻² rad per kelvin of mean temperature (§5) [M].
- **Sealed room**: mass is fixed, so p₀ rises by `dp₀/dt = (γ−1)P/V`: 100 W in 50 m³ gives 0.8 Pa/s [D, M] (1 kPa in
  21 minutes), the mean density does not change, and only the *redistribution* is read.

A real room is sealed for fast changes and leaky for slow ones; the crossover is the air-change rate (typically
0.3–1 h⁻¹ [R]). **Make `leakage` (air changes per hour) a room parameter, default open, recorded in the bundle.**

### 3.5 Water's equation of state

Computed from IAPWS-95 (Wagner & Pruß, J. Phys. Chem. Ref. Data 31, 387 (2002)), evaluated with the
[`iapws` package](https://pypi.org/project/iapws/), at 101.325 kPa [M]:

- maximum density **999.9749 kg/m³ at 3.978 °C**; at 10 MPa the maximum is at 1.95 °C, so −0.2 K/MPa;
- the CIPM-2001 formula ([Tanaka et al., Metrologia 38, 301 (2001)](https://doi.org/10.1088/0026-1394/38/4/3), via the
  [`chempy` transcription](https://pythonhosted.org/chempy/_modules/chempy/properties/water_density_tanaka_2001.html))
  differs from IAPWS-95 by at most 1.14 g/m³ (1.15 ppm) over 0–40 °C; [Harvey et al. 2009 (Metrologia)](https://bvbvf1.bib-bvb.de/vufind/PrimoRecord/cdi_proquest_miscellaneous_743409335) [S] report the two as consistent;
- hydrostatic compression: +1 m of head raises the density by 4.9 × 10⁻⁶ (relative), i.e. 4.9 × 10⁻¹⁰ Pa⁻¹.

Maximum error against IAPWS-95 (g/m³) of candidate forms [M]:

| Form | 0–10 °C | 0–40 °C | 0–95 °C |
|---|---|---|---|
| polynomial, degree 2 | 4.07 | 143 | 888 |
| degree 4 (the "quartic", cf. [Michalek & Kowalewski 2003](https://journal.mostwiedzy.pl/TASKQuarterly/article/download/2190/2104) [S]) | 0.01 | 2.42 | 45 |
| degree 6 | 0.00 | 0.05 | 2.7 |
| degree 8 | 0.00 | 0.00 | 0.17 |
| Gebhart–Mollendorf power law, as I recall its coefficients [R] | 16 (0–20 °C) | 515 | — |
| **cubic spline on 1 K nodes of IAPWS-95** | — | — | **0.001** (dρ/dT error 10⁻⁵ kg m⁻³ K⁻¹) |
| cubic spline on 2 K nodes | — | — | 0.016 |
| linear β at 20 °C (β = 207 × 10⁻⁶ K⁻¹), ±10 K | — | 0.57 kg/m³ | — |

**Recommendation:** a 96-entry table of IAPWS-95 (generated by a script with its provenance, no dependency in the engine) with a
cubic spline, in a small GPU lookup. A single polynomial over a wide range is a poor fit near the maximum; a
quartic is adequate only over 0–10 °C. For the inversion benchmark use whatever polynomial that paper specifies,
so the comparison tests the solver and not the fit; measure its distance from IAPWS-95 separately. A tank's
phase depends on the *volume* change, so 1 ppm of density on 1000 kg is 1 g and is negligible (§5).

### 3.6 Thermal expansion as linear thermoelasticity on voxel finite elements

Load and solve: `K u = ∫ Bᵀ D ε_th dV`, `ε_th = αΔT·I`; quasi-static, because the elastic wave time (10 m at
5 km/s, 2 ms) is far below any thermal time, and the thermoelastic back-coupling is small (a few percent at most
for metals, [R]). The solve is S7's: matrix-free CG with geometric multigrid, hanging-node constraints on the
graded tree (as micro-CT bone mechanics does at 10⁸–10⁹ elements, Adams et al., SC04, 2004 [R, no link found]).

**Locking, measured [M].** A free beam, length ten thicknesses, linear temperature gradient (exact curvature αΔT/h),
square plane-stress elements:

| Cells through the thickness | 1 | 2 | 4 | 8 |
|---|---|---|---|---|
| Q4 (the 2-D analogue of the Q1 hexahedron): κ_FE / κ | 0.963 | 0.988 | 0.997 | 0.999 |
| Q4 + incompatible modes | 1.0000 | 1.0000 | 1.0000 | 1.0000 |

Plain Q1 is 3.7 % low with one cell across a wall, 1.2 % with two; the incompatible-mode element
(Wilson 1973 / Taylor 1976 [R]) is exact for pure thermal bending at any resolution. Use it, since thin walls and
floors are one to three cells thick at 5–10 cm pitch. Cubic cells are the favourable case: elongated cells lock far worse.

**The read.** The displacement of each parcel is `u(x)` from nodal interpolation; mass is relocated not re-weighed (exact), and
first order in the gravity signal is `δΦ = S Σᵢ mᵢ ∇ᵢK(xᵢ)·uᵢ` [D, used for §5], so the *response map* applies unchanged. The
cell's own distortion is second order: strain 10⁻⁴ against cell size is 10⁻⁴.

**Check of the interior solution [M, symbolic].** For a free sphere with a radial temperature profile
`T(r)`, the Timoshenko–Goodier solution (*Theory of Elasticity*, art. 147 [R])

```
u(r) = (1+ν)/(1−ν) · α/r² ∫₀ʳ T s² ds  +  2(1−2ν)/(1−ν) · α r/R³ ∫₀ᴿ T s² ds
```

satisfies the radial Navier equation, the traction-free surface condition, and u = αTr for uniform T, checked
symbolically for T = 1, r², r⁴+3, exp(−r²) (`checks.py sphere`, all residuals exactly 0). Mass conservation gives
the enclosed-mass change `δM(r) = −4πρ₀r²u(r)`, so inside the sphere the field is closed form; outside, it is
**exactly unchanged**, for *any* `T(r)`. That is a stronger null than "uniformly heated sphere".

## 4. Question 2 — validation: review of §2.5 and the sources

### 4.1 Verdicts row by row

Verdicts: **Keep**, **Fix** (right idea, wrong statement), **Wrong**, **Weak** (passes when the thing is broken),
**Number** (cannot be confirmed here). Tolerances are what the literature supports, not what a solver reaches.

**Conservation and nulls**

| Row | Verdict | Finding |
|---|---|---|
| `closed_box_conserves_energy` | **Fix** | The conserved quantity is *thermal* energy only: Boussinesq drops viscous dissipation and pressure work, whose ratio to sensible heat is `gβL/c_p` = 1.0 × 10⁻⁴ for air over 3 m, 4.9 × 10⁻⁷ for water over 1 m [M]. Round-off conservation is impossible with the semi-Lagrangian/MacCormack advection of `physics.md` §5.2 (neither is a flux form), and with a flux form it holds only to the projection residual times T. State: pure conduction to round-off; with flow, to the reported divergence residual. |
| `steady_state_balances` | Keep | Tolerance is the iteration's own residual, reported. |
| `isothermal_world_is_silent` | Keep | Control arm: raise one cell by 1 mK and require the exact predicted non-zero read. Exact zero needs θ = T − T_ref stored (§3.1). |
| `uniformly_heated_sphere_is_silent` | **Fix** | True only *outside* a *free*, *continuous* sphere. A voxel sphere's exterior multipoles change by O(αΔT) × the cubic-anisotropy moments of the staircase. Control arm: a heated cube is *not* silent. Strengthen with a non-uniform T(r) and the §3.6 solution: the discretisation error becomes a measured budget. |
| `heated_infinite_slab_is_silent` | **Wrong** | A free slab expands laterally too, so its areal density drops by 2αΔT and its field with it; "areal mass unchanged" holds only if laterally constrained. And a vertical slab's field is uniform along a vertical tower, so the read is zero *for any areal density*: the test passes with a broken expansion. A finite wall is not silent: 3.5 × 10⁻² rad for a 10 m wall at 5 m [M, §5]. Replace by the finite-slab first-order formula and by the Poisson column (§4.5). |
| `expansion_conserves_mass` | Keep | Structural by relocation; pair it with the two-path test. |

**Conduction**

| Row | Verdict | Source and numbers | Tolerance |
|---|---|---|---|
| `slab_steady_profile` | Keep | `q = ΔT / Σ(Lᵢ/kᵢ)`; textbook (Incropera). | round-off for a linear profile |
| `semi_infinite_step` | **Fix** | Carslaw & Jaeger, *Conduction of Heat in Solids* §2.5: `(T−T_i)/(T_s−T_i) = erfc(x/2√αt)`. A *step* is singular at t = 0: the cell next to the surface errs at O(h) until √αt ≳ 4h; compare the absorbed energy `2k(T_s−T_i)√(t/πα)` and the profile only for t beyond that. "Second order in space" holds for smooth data. | second order in h, first in dt for Euler, second for RKL2 |
| `plate_fourier_series` | Keep | steady T in a rectangle with one hot edge: series `(4/π) Σ_{n odd} (1/n) sin(nπx/a) sinh(nπy/a)/sinh(nπb/a)`. For a square the centre value is exactly **T₀/4** by superposition, and the series gives 0.250000 [M]. Corners are singular: use L² or points away from them. | 10⁻⁶ in the interior |
| `lumped_cooling` | **Fix** | `Bi = hL_c/k < 0.1`, error O(Bi). Replace by the **slab with a convective surface**: `θ = Σ Cₙ e^{−ζₙ²Fo} cos(ζₙ x/L)`, `ζₙ tan ζₙ = Bi`. It tests `Film` and conjugate coupling at any Biot number. | 10⁻³ in the interior |
| `contact_between_bodies` | **Fix** | The final mean `(C₁T₁+C₂T₂)/(C₁+C₂)` is only energy conservation. Add the early-time interface temperature `T_i = (e₁T₁+e₂T₂)/(e₁+e₂)`, `e = √(kρc)` (Carslaw & Jaeger), which tests the face conductance. | 1 % |

**Radiation**

| Row | Verdict | Finding |
|---|---|---|
| `view_factors_closed_forms` | Keep | Howell's catalogue (*Thermal Radiation Heat Transfer*; thermalradiation.net, which I could not open [R]): aligned parallel rectangles (C-11) and perpendicular rectangles with a common edge (C-14), implemented and checked against Monte Carlo [M]: 0.199825 and 0.200044 for unit squares; coaxial discs `F = ½(S − √(S² − 4(r₂/r₁)²))`, `S = 1 + (1+R₂²)/R₁²`, 0.381966 at r = h; concentric spheres `F₂₂ = 1 − (r₁/r₂)²`. Tolerance is the ray noise, `√((1−F)/(FN))`: 1 % on F = 0.2 needs 4 × 10⁴ rays. |
| `view_factor_rules` | **Weak** | Summation is automatic in a closed room if every ray hits something, and reciprocity holds only statistically for ray counts. Test the second with a tolerance `∝ N^{−1/2}`, and apply it to staircase geometry to expose the 3/2 area error. |
| `two_surface_enclosure` | Keep | `q = σ(T₁⁴−T₂⁴) / [(1−ε₁)/ε₁A₁ + 1/A₁F₁₂ + (1−ε₂)/ε₂A₂]` (Siegel & Howell; Incropera). |
| `radiation_shield` | **Fix** | Exchange is halved only if all emissivities are equal; N shields give 1/(N+1). State the condition. |

**Convection in air**

| Row | Verdict | Reference numbers | Tolerance |
|---|---|---|---|
| `heated_cavity_2d` | Keep; refine | [de Vahl Davis, IJNMF 3, 249 (1983)](https://doi.org/10.1002/fld.1650030305), Pr = 0.71, mean Nu: Ra 10³ **1.118** [R, matches `physics.md`], 10⁴ **2.243** [S], 10⁵ **4.519** [S], 10⁶ **8.800** [S]; midplane u_max at 10⁴ **16.178** [S]. The converged values of Hortmann, Perić & Scheuerer (1990) [R] are 1.1178, 2.2448, 4.5216, 8.8252: de Vahl Davis's own extrapolation is 0.0–0.3 % low, and a tutorial quotes "8.80–8.83" at 10⁶ [S](https://mfix.netl.doe.gov/doc/mfix-exa/guide/latest/tutorials/fluid/steady-natural-convection-cavity.html). | 1 % on Nu after grid convergence (the references' own spread is 0.3 %); hot-wall = cold-wall Nu within 1 %; observed order ≈ 2 on the uniform grid. Ra 10⁷–10⁸ at 30.225 [R, Le Quéré 1991](https://doi.org/10.1016/0045-7930(91)90025-D) only after the table is read |
| `heated_cavity_3d` | **Fix** | The cube's reference is **not** Fusegi alone. [Fusegi, Hyun, Kuwahara & Farouk, IJHMT 34, 1543 (1991)](https://doi.org/10.1016/0017-9310(91)90295-P) report (my recollection) mean Nu 1.105, 2.302, 4.646, 9.012 [R] for Ra 10³–10⁶. The converged spectral values of Tric, Labrosse & Betrouni (IJHMT 43, 4043 (2000)) are, as I recall, 1.071, 2.054, 4.337, 8.637 [R]. Fusegi's finite-difference values being *above* the 2-D numbers at 10⁴ (2.302 against 2.243), when the cube's end walls must lower them, points to coarse-grid error; experiments: [Leong, Hollands & Brunger](https://people.ryerson.ca/weyleong/benchmark/benchmark1.html) [S]. | 1 % to Tric; Fusegi cannot be held tighter than ~5 % |
| `rayleigh_benard_onset` | Keep; **Fix** setup | `Ra_c = 1707.762`, `k_c = 3.1163` for rigid plates; 1100.650 rigid–free; 657.511 = 27π⁴/4 free–free: all **reproduced here from the linear problem by Chebyshev collocation** [M] (`checks.py references`); also stated in [arXiv:comp-gas/9612001](https://arxiv.org/pdf/comp-gas/9612001) [S]. Onset is a growth-rate extrapolation (σ → 0), not a threshold. **A box with free-slip, adiabatic side walls of width π/k_c = 1.008 depths carries the mode exactly**, so 1707.76 holds in the engine's closed box [D]; no-slip sides raise it. | growth-rate zero crossing within 1 % of 1707.76 |
| `vertical_plate` | **Fix** | Churchill & Chu, IJHMT 18, 1323 (1975): `Nu_L = {0.825 + 0.387 Ra_L^{1/6} / [1 + (0.492/Pr)^{9/16}]^{8/27}}²` (checked against the textbook worked example, 147.16 for Pr 0.69, Gr 2.63 × 10⁹, [M] and [`ht` docs](https://ht.readthedocs.io/en/latest/_modules/ht/conv_free_immersed.html) [S]); laminar form `0.68 + 0.670 Ra^{1/4}/[1+(0.492/Pr)^{9/16}]^{4/9}` for Ra < 10⁹ [R]. At Pr = 0.71 [M]: Nu = 9.21 (10⁵), 16.56 (10⁶), 31.2 (10⁷), 61.1 (10⁸), 123 (10⁹). The two forms differ by 5 % at 10⁷ and 25 % at 10⁹. The stated uncertainty: not found; later comparison with data gives errors of 4.7 %–16.4 % by dataset ([Jaffer 2022](https://arxiv.org/abs/2201.02612) [S]). The correlation is for a plate in an *infinite* ambient; the engine's closed box stratifies, so run for a time short against filling. | ±10 % for 10⁵ < Ra_L < 10⁹; report the band, do not assert it |
| `plume_scaling` | **Weak** | Morton, Taylor & Turner, Proc. R. Soc. A 234, 1 (1956): top-hat plume `b = 6αz/5`, `w = (5/6α)(9αB/10)^{1/3} z^{−1/3}`, `g' = (5B/6α)(9αB/10)^{−1/3} z^{−5/3}`, volume flux `Q = (6πα/5)(9αB/10)^{1/3} z^{5/3}` [D from the similarity form]. That gives 18.5 L/s at 1 m for 100 W [M]. Entrainment: top-hat α = 0.083 [R], reported 0.10–0.16 for plumes under other profile definitions ([search summary](https://researchrepository.ul.ie/server/api/core/bitstreams/d9b51b80-16d7-4788-9e8e-d01e07103a79/content)), so test exponents and integrated flux, not α. Three reasons it is weak: (i) a laminar plume has *different* exponents (by scale analysis, ΔT ∝ z⁻¹ and w independent of z [D]; I could not confirm it against Fujii 1963 or Yih 1951); (ii) a closed room stratifies after the first ceiling pass (filling box, Baines & Turner 1969 [R]), ending the unstratified scaling; (iii) 0.3–2 m is a decade of range. | exponent ±0.05 over ≥ 8× in z, flux ±15 % |
| `conjugate_cavity` | **Number** | Kaminski & Prakash, IJHMT 29(12), 1979 (1986): the tables could not be found. Substitute the Biot slab above and the finite-conductivity-plate onset (Sparrow, Goldstein & Jonsson, [JFM 18, 513 (1964)](https://doi.org/10.1017/S0022112064000386): onset falls monotonically from 1708 for fixed-temperature plates to about 720 for fixed-flux plates [R]) until the tables are read. | to be set from the table |

**Water**

| Row | Verdict | Finding |
|---|---|---|
| `water_density_maximum` | Keep; sharpen | IAPWS-95: **3.978 °C**, 999.9749 kg/m³ [M]; CIPM-2001 agrees to 1.15 ppm; the maximum moves −0.2 K/MPa. "Near 4 °C" should be a number with a pressure. Tolerance: 10⁻⁸ against the table form chosen in §3.5. |
| `water_cavity_with_inversion` | **Number** | See §4.2. |
| `stable_stratification_is_still` | Keep | Internal waves: `N² = −(g/ρ₀) dρ/dz`, using the *local* dρ/dT, which vanishes at 4 °C. In air `N² = (g/T) dθ/dz`. Tolerance 1–2 % on ω, damping rate ν k²/2 allowed for. |
| `heated_tank_from_below` | **Fix** | See §4.3. |

**Thermal expansion**

| Row | Verdict | Finding |
|---|---|---|
| `free_bar_expands` | Keep | `ΔL = αLΔT`; for finite ΔT use `∫α dT`. |
| `clamped_bar_stress` | Keep | `σ = −EαΔT` for a bar held at both ends with the sides free; fully confined in 3-D it is `−EαΔT/(1−2ν)`. State which. |
| `bimetal_strip` | Keep | Timoshenko, J. Opt. Soc. Am. 11, 233 (1925), as restated in [arXiv:1504.03010](https://arxiv.org/pdf/1504.03010) or [arXiv:1702.03243](https://arxiv.org/pdf/1702.03243) [S]: `κh = 6Δε(1+m)² / [3(1+m)² + (1+mn)(m² + 1/mn)]`, `m` thickness ratio, `n` modulus ratio, `Δε = (α₂−α₁)ΔT`. **Agrees with plane-section beam theory to 10⁻¹⁶** [M]: for 1 mm steel on 1 mm aluminium at 50 K, radius 2.607 m. κh is dimensionless in the ratios, so the test runs at centimetre pitch with a thick strip (8 cells per layer). One printed variant has `3 + (1+m)²` instead of `3(1+m)²` (a patent) [S]; it is a typo. |
| `gradient_bends_a_beam` | Keep | `κ = αΔT_faces/h`. This is the test that exposes Q1 locking (§3.6): 3.7 % low at one cell. |

**Heat through the instrument**

| Row | Verdict | Finding |
|---|---|---|
| `warm_air_column_phase` | Keep | Use the ideal-gas anomaly `ρ₀(T₀/T − 1)`, not `−ρβΔT`: they differ by ΔT/T (3.4 % at 10 K). |
| `radiator_anchor` | **Wrong** | v2's 2.2 × 10⁻² rad is the output of an unvalidated simulation of a closed adiabatic room (`physics.md` §10, `V2-HANDOFF.md`), not an anchor. A plume from 500 W gives 3 × 10⁻⁴ rad at 1 m [M, §5], 80 × smaller; 2.2 × 10⁻² is what 1 K of mean hall temperature gives, which a closed room reaches in about two minutes at that power (0.5 kW into 60 kg of air is 8 mK/s [D]). Replace by an energy-bookkeeping test: predicted phase = response map × the room's mean-temperature rise from the energy input. |
| `response_map_superposes` | Keep | Closed form: for a uniform θ = ΔT/T₀, full = linear × 1/(1+θ), second-order share θ [M]. |
| `expanding_wall_phase` | Keep | The two paths agree to first order; they differ at u/D (2 × 10⁻⁴ for 1 mm at 5 m). |

### 4.2 Natural convection of water across its density maximum

**Choose the Michalek, Kowalewski & Šarler benchmark** ([Prog. Comput. Fluid Dyn. 5, 158 (2005)](https://www.inderscience.com/info/inarticle.php?artid=6751),
earlier version [Task Quarterly 2003](https://journal.mostwiedzy.pl/TASKQuarterly/article/download/2190/2104)) [S]. It is a steady
differentially heated cavity near the freezing point, set up as a *numerical benchmark* with finite-difference,
finite-volume, finite-element and mesh-free solutions compared, and with the anomalous density entering the buoyancy
term only through a fourth-order polynomial. The same group ran the matching experiments. Second choice:
[Lin & Nansteel (IJHMT 30, 2319 (1987))](https://www.cambridge.org/core/journals/journal-of-fluid-mechanics/article/natural-convection-in-an-inclined-square-cavity-in-regions-of-density-inversion-of-water/ABF512B764BD76436672B9F19E5D2BF6) [S, linked page is a later paper citing it], Ra 10³–10⁶ with a density-inversion parameter. Experiment: Braga & Viskanta (IJHMT 35, 861 (1992)) [S], transient.
**I could not retrieve the geometry, boundary temperatures or Nusselt tables**, so no number is given here; the first act of the S10/S13
slice is to fetch them. Pass criterion to write before measuring: two counter-rotating cells, Nu within the table's
inter-method spread (the paper's own comparison), and an unchanged result when the quartic is replaced by the spline of §3.5.

### 4.3 Rayleigh–Bénard near Pr = 7

Water at 20 °C has Pr ≈ 7.0 (13.5 at 0 °C, 6.1 at 25 °C) [R]. Use three things, in order of strength:

1. **Onset**, exact: 1707.762 with rigid plates, k_c = 3.1163 [M], in the free-slip box of §4.1. Pr-independent.
2. **Weakly nonlinear slope** (Schlüter, Lortz & Busse 1965 [R]): `Nu − 1 = ε / (0.69942 − 0.00472/Pr + 0.00832/Pr²)`, `ε = (Ra − Ra_c)/Ra_c`, giving 1.4308 at Pr = 7 and 1.4298 as Pr → ∞ [M, from the recalled coefficients]. Valid for small ε in a laterally periodic roll state.
3. **Turbulent Nu**: DNS at Pr = 6.4, Ra = 10⁸ gives **Nu = 31.01** in a table of [arXiv:0811.0462](https://arxiv.org/pdf/0811.0462) [S, aspect ratio and wall conditions not confirmed]. A 2-D run at Ra = 10⁸ gives about 27 ([arXiv:1501.01827](https://arxiv.org/pdf/1501.01827) [S]): **2-D is not a valid substitute for 3-D** at these Rayleigh numbers, 15 % apart. Cross-code scatter is in [Kooij et al., arXiv:1802.09054](https://arxiv.org/pdf/1802.09054) [S, unread]; the standard compilation is Ahlers, Grossmann & Lohse, Rev. Mod. Phys. 81, 503 (2009) [R].

Set the Nu tolerance from the cross-code study after reading it. Closed tanks have side walls: their onset is *above* 1708 (Davis 1967 [R]); do not
expect 1708 there.

### 4.4 What the literature supports, summarised

Reference accuracy: de Vahl Davis 0.3 %; Tric (3-D) the converged one, Fusegi 5 %; Churchill–Chu 5–15 %; plume constants a factor 1.5 in α, which is ≤ 55 % in phase
(phase ∝ α^{2/3} [M]); view factors exact, with ray noise `N^{−1/2}`; Timoshenko exact for thin elastic strips; water density 1 ppm.

### 4.5 Missing cases, especially where the gravity signal has a closed form

| Proposed test | Closed form | Why it earns a place |
|---|---|---|
| `uniform_gradient_phase` | for a uniform Γ_zz over the flight, **ΔΦ = k_eff T² Γ_zz Δr** exactly [D]: the two interferometers' arm-midpoints differ by Δr for every t, and ∫(arm separation) dt = v_rec T² with m v_rec = nħk | the instrument's own response to a linear field; checks `S` |
| `stratified_column_gradient` | Poisson in 1-D: for any horizontally uniform δρ(z), **a_z(z₂) − a_z(z₁) = −4πG ∫δρ dz** [D]; slab limit 8.2 × 10⁻² rad/K [M] | the gravity of any layered temperature field, no source geometry; the finite-box truncation error is measured against it |
| `thermoelastic_sphere_exterior` | exterior field unchanged for any T(r) (§3.6) | measures the whole chain (FE, parcels, kernel) as an error budget |
| `thermoelastic_sphere_interior` | δM(r) = −4πρ₀r²u(r) with the verified u(r) | closed-form interior field |
| `heat_kernel_exterior` | a point release Q in a still medium: δρ = −(βQ/c_p)(4πκt)^{−3/2} e^{−r²/4κt}; total deficit **βQ/c_p** constant in time (0.339 g for 100 J in air); enclosed fraction `F = erf(r/2√κt) − (r/√πκt) e^{−r²/4κt}` (checked by quadrature to 8 digits [M]); the field outside the cloud is constant while it diffuses | tests conduction and the equation-of-state read together; needs buoyancy off |
| `leakage_closure` | sealed: mean ρ fixed, `dp₀/dt = (γ−1)P/V`; open: full anomaly | decides the air signal (§3.4) |
| `solid_bar_with_film` | Biot slab series `ζ tan ζ = Bi` | conjugate coupling at any Bi |
| `interface_temperature` | `T_i = (e₁T₁+e₂T₂)/(e₁+e₂)` | face conductance |
| `staircase_area` | a voxel sphere's face area is 1.4998× the true area; the ‖∇φ‖ area within a stated band | catches the wrong surface area |
| `filling_box` | Baines & Turner 1969 similarity [R] | plume in a closed room, after the unstratified scaling |
| `rb_growth_rate` | σ → 0 at 1707.762 | onset without the threshold ambiguity |

## 5. Question 3 — sizes

### 5.1 Method and calibration

For a field that varies slowly over the flight, the phase is the difference of the vertical accelerations at the two
interferometers times `S = k_eff T²`, with `k_eff = n·2π/λ = 1000 × 2π/698 nm = 9.00 × 10⁹ rad/m`, `T = 0.73 s`,
so **S = 4.80 × 10⁹ rad per m/s²**; exact for a uniform gradient (§4.5). Interferometers at 2.5 m and 7.5 m (separation 5 m,
`instrument.md`), sources by direct summation of point masses, `δΦ = S[a_z(z₂) − a_z(z₁)]` (`checks.py signals`). The owner's "10 m baseline" is the tower: the
repository's AION-10 has IFO separation 5 m. A 10 m separation doubles gradient-like signals, not local ones.

Calibration: 70 kg at 2 m mid-height gives **3.41 rad**, against the plan's "~3 rad" [M, `REBUILD.md` §5.2]. 10 kg at 10 m static gives 14.6 mrad against
the anchor's ~7 mrad for a moving body (`instrument.md` §12), a factor 2: I quote everything to a factor 2–3.

Noise: PSD 10⁻⁴ rad/√Hz. One shot at the 2 s cycle (f_N = 0.25 Hz): **5 × 10⁻⁵ rad**. A mean over 1 h: `10⁻⁴/√(2·3600)` = **1.2 × 10⁻⁶ rad**, over a day 2.4 × 10⁻⁷ rad. Heat acts on hours,
so the slow thresholds apply to everything except a person's plume.

### 5.2 The four effects

1. **A person as a 100 W source.** Plume in the Morton–Taylor–Turner top-hat form: buoyancy flux `B = gP/(ρ₀c_pT₀) = 2.8 × 10⁻³ m⁴ s⁻³`, mass deficit per unit height
   `λ(z) = ρ₀B/(g w(z))` (from `B = πb²w g'`) with `w` as in §4.1; the deficit of the plume from 1 m to a 2.6 m ceiling is **0.78 g** (0.56 g for 60 W), because
   `λ ∝ α^{2/3}`. At 100 W, source 1 m high [M]: **5.8 × 10⁻⁵ rad at 1 m**, 6.6 × 10⁻⁶ at 2 m, 2.3 × 10⁻⁶ at 5 m (sign flips),
   8 × 10⁻⁷ at 10 m; α = 0.12 or 0.16 raises it by 28 % or 55 %. The person's own 70 kg at 2 m reads 1.7 rad, so the plume is 4 × 10⁻⁶ of the body. Matters only within ~1–2 m and at shot level.
   But the *energy* stays in the room: 100 W into a hall with `UA ≈ 300 W/K` (6 × 10⁻³ rad at steady state if no ventilation removes it [D]) is the larger term, and 100 such people are a 10 kW source.
2. **A radiator.** 500 W convective: deficit **3.8 g**; **2.7 × 10⁻⁴ rad at 1 m**, 4.8 × 10⁻⁵ at 2 m, 4.5 × 10⁻⁶ at 3 m [M]. The radiative part
   (a quarter [R]) heats walls first. The plume is above one-shot noise only within 2 m. v2's 2.2 × 10⁻² rad is the room warming (§4.1).
3. **A sunlit wall expanding.** 10 × 10 × 0.3 m concrete, 2400 kg/m³ (72 t), plane at D, free expansion with strain 10⁻⁴ (α = 10⁻⁵ K⁻¹ × 10 K mean), about the
   centre of the base [M]:

   | | D = 5 m | D = 10 m |
   |---|---|---|
   | isotropic strain 10⁻⁴ (top rises 1 mm) | +3.5 × 10⁻² rad | +4.6 × 10⁻³ |
   | bow of 4 mm at mid-height (`κ = αΔT_face/h = 3.3 × 10⁻⁴ m⁻¹`, `δ = κL²/8`) | +0.27 | +4.7 × 10⁻² |
   | whole wall 1 mm nearer | −6.8 × 10⁻² | −1.5 × 10⁻² |

   The last row converts at 70 µrad per micrometre at 5 m, consistent with the plan's "concrete wall ~50 µrad" anchor for a micrometre-scale oscillation. Real frames restrain expansion, so
   take 3–10 × less; still 10²–10⁴ × the hourly threshold. Mass is unchanged; only the position moves. The diurnal cycle's period is 10⁵ s, so the bandwidth-limited noise is 10⁻⁶ rad.
4. **A warm water tank.** Open tank 1 m² × 1 m from 20 °C with its mass fixed [M]:

   | Case | D = 2 m | D = 5 m |
   |---|---|---|
   | uniform 60 °C (surface rises 15.3 mm) | +3.5 × 10⁻² rad | −1.4 × 10⁻² |
   | uniform 30 °C (2.6 mm) | +6.0 × 10⁻³ | −2.4 × 10⁻³ |
   | stratified 20 °C bottom to 60 °C top (6.6 mm) | +7.9 × 10⁻³ | −4.0 × 10⁻³ |
   | 20 → 21 °C (0.21 mm) | +5.0 × 10⁻⁴ | — |
   | a 200 L cylinder, 0.2 m², uniform 60 °C | +5.9 × 10⁻³ | — |

   Only the redistribution is read: a closed, full tank would be silent. The gravity read uses the *full* density in the liquid (`voxel.md` §9.6).

**And the air itself, per kelvin of mean air temperature** (leaky room, constant pressure) [M]:

| Region of air | Phase per K |
|---|---|
| a 1 m radius shaft, 10 m tall | 2.2 × 10⁻³ rad |
| hall of radius 5 m, 6 m tall | 2.2 × 10⁻² |
| hall of radius 5 m, 12 m tall | 2.1 × 10⁻² |
| hall of radius 10 m, 12 m tall | 4.1 × 10⁻² |
| infinite slab, 5 m between interferometers (closed form `4πG S abs(δρ) Δz`) | 8.2 × 10⁻² |

Thus **a 10 mK drift in the hall's mean air temperature gives ≳ 2 × 10⁻⁴ rad**, above one shot; this is the leading heat systematic for a real site, and the only one for
which the closure of §3.4 decides between full size and none.

### 5.3 What this means for the builder

| Effect | Matters? | Needs in S13 |
|---|---|---|
| room-mean air temperature | **yes**, to 10 mK | leakage closure; energy budget of the whole room; walls |
| structure expansion | **yes**, 10⁻⁴ strain | thermoelasticity; the wall's temperature field, diurnal |
| water volume | **yes**, from 1 K | EOS table; free-surface rise |
| radiator plume | within 2 m | `Film` surface; plume under-resolved at 5 cm (b = 0.1z) |
| person plume | within 1–2 m, shot level | a plume sub-model, not resolution |
| radiation | indirect: surface temperatures | patch radiosity |

An under-resolved plume is *slow* (too wide), and `λ = ρ₀B/g w` grows as the velocity falls, so it overstates the deficit; an analytic plume
line source for person-scale sources (a few watts of grid-unresolved plume) is cheaper and, with MTT, validated.

## 6. Question 4 — what only experiment can settle

1. **The real site's thermal state.** Air-change rate, wall U-values, HVAC control, solar gain, occupancy, door use: parameters, not physics; ±30 % or worse and unknown for any real hall.
2. **Turbulent room convection statistics** (spectra, correlation lengths) at Ra ~ 10⁹–10¹¹, where the scheme's own dissipation is the model (`physics.md` §5.5). Idealised-box DNS covers far smaller domains than a room;
   a real room needs measurement. These decide whether a denoiser trained here transfers.
3. **Structural restraint.** How much a real building's frame, joints and ground hold a wall back: a factor of 3–10 on the largest signal in §5, found only by displacement monitoring.
4. **Material parameters** in situ: k, c, α, ε of weathered concrete, brick, paint, with moisture; contact resistance of real joints; solar absorptivity. Tables give 10–30 %.
5. **Film coefficients** of real surfaces (the correlations' scatter, 5–15 %, §4.1), and mixed convection from fans and doors.
6. **The end-to-end phase from a known heater** in the real instrument: the only test of the whole chain (heat → density or displacement → gravity gradient → atom-interferometer phase). Everything above is internal consistency.
7. **Barometric pressure.** 100 Pa is a 10⁻³ density change, 3 × the effect of 1 K; it enters the closure of §3.4 and is out of S13, but a site comparison will contain it.
8. **Water**: the inversion benchmark's agreement with experiment (Braga & Viskanta), not just between codes, and tank stratification in practice.
9. **Heat that is not gravity.** `REBUILD.md` §9.3 (S15) lists the blackbody shift from warm surfaces as an instrument systematic fed by S13's temperature
   field. It is outside the four questions and I did not size it, but a first estimate says it must be: the blackbody shift of the ⁸⁷Sr clock transition is about
   2.35 Hz at 300 K, ∝ T⁴ [R, from memory of the Sr clock literature], so a 1 K difference between what the two interferometers see is
   ≈ 4(ΔT/T) × 2.35 Hz ≈ 0.03 Hz; over the 0.73 s the atom spends in the excited state that is ~0.1 rad per kelvin [D, unverified, and I do not know how the pulse
   sequence cancels it]. If anything like that survives the double difference it would exceed every gravity effect in §5. S15's note should size it before S13's temperature field is promised to it.

## 7. Recommendations

1. **Adopt the §3 methods**, in the order: flux-form conservative conduction with RKL2; one-domain conjugate with face conductances and flux registers; Monte-Carlo patch radiosity; exact anomaly read with a guard; IAPWS-95 spline table; Q1 with incompatible modes.
2. **Change the plan's energy statement** (§4.1): thermal energy; round-off for pure conduction; flow cases to the reported projection residual. This needs flux-form temperature advection in S8/S9 instead of semi-Lagrangian MacCormack, or the S13 budget cannot close. Decide in S8, not S13.
3. **Add the room closure** (`leakage`, default open) to the world model and the bundle; this is the largest single uncertainty in the air signal.
4. **Edit §2.5:** replace `heated_infinite_slab_is_silent`, `radiator_anchor`; fix `closed_box_conserves_energy`, `uniformly_heated_sphere_is_silent`, `semi_infinite_step`, `lumped_cooling`, `contact_between_bodies`, `radiation_shield`, `heated_cavity_3d`, `vertical_plate`, `plume_scaling`; add the §4.5 cases. Use Tric, not Fusegi, as the cube's reference.
5. **Write pass criteria before measuring.** Starting points: cavity Nu 1 %; 1707.76 growth-rate zero within 1 %; bimetal and clamped bar 10⁻⁶ in the interior; view factors `4/√N` of the ray noise; phase cases against §4.5 to 10⁻⁶ of the signal for the linear-order ones.
6. **First measurements in the slice:** (a) fetch the benchmark tables marked **Number** and [R]; (b) the staircase area error on the engine's own sphere; (c) the `f32` θ accumulation over a day (stagnation); (d) RKL2 order and stability on a concrete/air interface with a 10⁵ conductivity contrast; (e) the radiator in a room with and without leakage, next to §5.
7. **Dependency policy:** none needed. The water table is generated by a script that uses the `iapws` package offline and ships as a const table.

**Calls worth a second look (owner):** the default `leakage` (open); dropping v2's radiator number as an anchor; replacing the slab row; the Boussinesq guard at ΔT/T₀ = 0.1.

**For George** (to `PROGRESS.md`): the hall's dimensions and air-change rate; the temperature and pressure logs at the site (the only real data
the heat signals can be compared with); wall construction within 10 m of a tower; IFO separation (5 m assumed) and which heights; HVAC regulation.

## 8. What I could not verify

- **Every paper's content.** No PDF opened. Numbers marked [R] are from memory and could be wrong: Fusegi's and Tric's cube values; the Hortmann et al. values; de Vahl Davis Ra = 10³ (1.118, though `physics.md` already quotes it);
  Le Quéré's Nu at 10⁷ and 10⁸; Sparrow et al.'s 720; the Schlüter–Lortz–Busse coefficients; Churchill & Chu's laminar form and *original* stated uncertainty; Gray & Giorgini's 29 K; Gebhart–Mollendorf's coefficients; α = 0.083.
- **Benchmark tables** (all "Number"): Michalek et al.; Lin & Nansteel; Kaminski & Prakash; Le Quéré et al. 2005; Vierendeels et al. 2003; the Pr = 6.4 DNS aspect ratio; the cross-code spread of Kooij et al.
- **DOIs** marked "recalled" were not seen.
- **Fujii 1963 and Yih 1951 plume exponents**; the laminar scaling is my scale analysis.
- **The thermoelastic back-coupling size**, the typical air-change rate, the panel radiator's radiative fraction, and the Prandtl numbers of water: from memory.
- **GPU timing** of RKL2, ray casting and the Q1 element (everything here is CPU, counts not times).
- **Whether v2's room was closed and adiabatic**, and its size and duration: `V2-HANDOFF.md` does not say; my explanation of 2.2 × 10⁻² rad is a hypothesis to test in the slice.
- **The sizes in §5** are first-order estimates by direct summation with assumed geometry (tower axis, interferometers at 2.5 and 7.5 m, uniform ΔT, free expansion); they have not been run through the propagation integral.

## 9. Further questions (not chased)

1. Soil and foundation heat: ground-coupled walls, the thermal inertia of the building shell at days to seasons.
2. A subgrid plume model: line-source plume with a tested handover to the grid.
3. Humidity and condensation (excluded by the plan, but latent heat of a person is half of their output).
4. The `f32` heat solve: what accuracy the response map needs, as a tolerance in kelvin per cell.
5. Hierarchical (clustered) radiosity for town-scale patch counts.
6. Is a first-order response map for the *air's pressure* (barometric) worth a place beside the thermal map?
7. Size the blackbody shift (§6 item 9) before S15: it may be the largest heat effect on the phase.

## Files

- `s13-heat/checks.py`: every measurement above (`water`, `spline`, `views`, `bimetal`, `sphere`, `beam`, `references`, `signals`).
- `s13-heat/results/`: raw output of each part.
