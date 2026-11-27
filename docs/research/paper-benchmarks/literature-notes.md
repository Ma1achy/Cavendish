# Literature notes for the polyhedral-gravity draft

Survey date 2026-11-27. Primary sources only. Every DOI below was checked against
`https://api.crossref.org/works/<doi>`; the bibliographic data given is what Crossref returned.
Reading status is marked on each item:

- **[FULL]**: full text read (PDF text extracted, or the full HTML article).
- **[PART]**: full text available but only the named sections were read.
- **[ABS]**: abstract only (publisher, Crossref, OpenAlex or Semantic Scholar).
- **[NONE]**: metadata only. Springer (link.springer.com), SEG and Wiley pages refused automated
  fetches (cookie redirect or 403), so most J. Geodesy, CMDA and Geophysics papers fall here.

A number with no section, equation or table reference was not taken from the source and is not given.

---

## 1. Prisms

### Nagy 1966 [NONE]
Nagy, D. "The gravitational attraction of a right rectangular prism." *Geophysics* 31(2), 362–371,
1966. doi:10.1190/1.1439779 (Crossref agrees with the bib entry.)

### Nagy, Papp & Benedek 2000 [NONE]
Nagy, D., Papp, G., Benedek, J. "The gravitational potential and its derivatives for the prism."
*Journal of Geodesy* 74(7–8), 552–560, 2000. doi:10.1007/s001900000116 (verified).
Fukushima 2020 (below) describes it as the standard summary of "Bessel's exact formula": arctan and
log terms at the 8 vertices. **Relation:** a vertex-class formula (one term per vertex), so its
far-field rounding is in the class the draft attributes to vertex forms.

### Plouff 1976 [ABS]
Plouff, D. "Gravity and magnetic fields of polygonal prisms and application to magnetic terrain
corrections." *Geophysics* 41(4), 727–741, 1976. doi:10.1190/1.1440645 (verified).
Exact gravity and magnetic fields of vertical polygonal prisms. The abstract says they are faster and
more accurate than numerical integration over laminas. It gives no rounding analysis. **Relation:** a
special case: polygonal prisms are polyhedra.

### Banerjee & Das Gupta 1977 [ABS]
Banerjee, B., Das Gupta, S. P. "Gravitational attraction of a rectangular parallelepiped."
*Geophysics* 42(5), 1053–1055, 1977. doi:10.1190/1.1440766 (verified). A short note on the closed
form for the prism's attraction. The abstract has no accuracy claims. **Relation:** a special case.

### Fukushima 2020 [PART: HTML full text, about 100k of 125k characters read]
Fukushima, T. "Speed and accuracy improvements in standard algorithm for prismatic gravitational
field." *Geophysical Journal International* 222(3), 1898–1908, 2020. doi:10.1093/gji/ggaa240
(verified).
- It rewrites Bessel's formula with the addition theorems of arctan and log as a triple difference,
  which cuts the transcendental calls by 2.7–8.0× (§2.2.5).
- Speed-up is 2.3× for the potential and for **g**, and 3.7× for the gradient tensor (abstract,
  §3.1, Table 1; Intel i7-8750H, ifort 19.0).
- Accuracy: relative errors are 2–10× smaller than Bessel's (§3.2), measured against Bessel's
  formula in quadruple precision. The error metrics are eqs (73)–(75): δV = ΔV/V, δg = |Δg|/|g| and
  δΓ = |ΔΓ|/|Γ|.
- **Far field (key point):** §3.2 and the Fig. 2 caption say the errors "stay at the machine epsilon
  level inside the Brillouin sphere and grow cubically outside it". The Brillouin radius is
  R/c ≈ 1.28 for the test prism, and the plots run over 0.1 ≤ R/c ≤ 10³. The paper calls this "a
  well-known phenomenon (Forsberg 1984; Holstein & Ketteridge 1996)". It proposes no remedy: §3.1
  says "only the direct computation of the exact formula provides reliable results" for its test
  points. The text gives no numerical error values at given R/c; those are only in the figures, which
  I could not read.
- §2.1, eq. (24): the log terms "suffer from the catastrophic precision loss when a relative
  coordinate … is negative". The fix is a conditional switch to ln[(Y²+Z²)/(R−X)] when X < 0. This is
  the conjugate (rationalising) trick, the same idea as the draft's eq. (hk).
- The introduction cites another 2020 Fukushima paper (not read) for a switch to a truncated Taylor
  series far away.
- **Relation:** independent support for the draft's claim that vertex-class formulas lose accuracy
  as ε(R/s)³. Prior art for the conjugate trick in the log term. Cite it in §Numerical stability.

### Li & Chouteau 1998 [NONE]
Li, X., Chouteau, M. "Three-Dimensional Gravity Modeling In All Space." *Surveys in Geophysics*
19(4), 339–368, 1998. doi:10.1023/A:1006554408567 (verified). Not read; I do not know whether it
treats far-field rounding. It is worth fetching: it is a review of prism and polyhedron forward
formulas and their singularities.

---

## 2. Tesseroids

### Uieda, Barbosa & Braitenberg 2016 [ABS]
Uieda, L., Barbosa, V. C. F., Braitenberg, C. "Tesseroids: Forward-modeling gravitational fields in
spherical coordinates." *Geophysics* 81(5), F41–F48, 2016. doi:10.1190/geo2015-0204.1 (verified).
The postprint (arts.units.it) returned 403.
- Gauss–Legendre quadrature (GLQ) per tesseroid, with adaptive discretisation controlled by a
  distance-size ratio D and implemented with a stack.
- Abstract: for a **maximum relative error of 0.1 %** against a spherical shell, D = 1 for the
  potential, D = 1.5 for **g** and D = 8 for the gradients. "Contrary to previous assumptions" the
  three need different D.
- **Relation:** the analogue of the draft's opening criterion, at a target of 10⁻³ against the
  draft's ~10⁻⁷. Their D is a fixed geometric ratio set empirically per quantity. The draft's
  criterion is an error bound computed from the moments. Each GLQ node is a point mass, so a 2×2×2
  GLQ tesseroid is a small mascon cluster.

### Soler, Pesce, Gimenez & Uieda 2019 [PART: HTML full text, about 100k of 139k characters read]
Soler, S. R., Pesce, A., Gimenez, M. E., Uieda, L. "Gravitational field calculation in spherical
coordinates using variable densities in depth." *Geophysical Journal International* 218(3),
2150–2164, 2019. doi:10.1093/gji/ggz277 (verified). (This is the "adaptive discretisation" GJI 2019
paper. Its title does not mention adaptivity.)
- §2.2, eq. (11): split the tesseroid along dimension i while d/L_i < D. Here d is the distance to
  the tesseroid's geometric centre (eq. 12) and L_i its latitudinal or longitudinal size
  (eqs 15–16). GLQ is second order. A second, density-based split uses a "delta ratio".
- Results (abstract, §3.1, Fig. 3): 0.1 % accuracy needs D = 1 for V and D = 2.5 for g_z
  (D = 2 suffices at satellite height); the delta ratio is 0.1 for exponential and sinusoidal
  densities.
- **Relation:** as for Uieda 2016. Their published tolerance is 10⁻³, four orders of magnitude
  looser than the draft's.

### Grombein, Seitz & Heck 2013 [FULL: KIT author version]
Grombein, T., Seitz, K., Heck, B. "Optimized formulas for the gravitational field of a tesseroid."
*Journal of Geodesy* 87(7), 645–660, 2013. doi:10.1007/s00190-013-0636-1 (verified). The author's
version is OA at publikationen.bibliothek.kit.edu/1000035020.
- The kernel is expanded in a Taylor series about the tesseroid's geometric centre (eq. 33). Odd
  terms vanish by symmetry, so only zero- and second-order terms remain: a "fourth-order error in the
  spatial coordinates" (abstract, eqs 31–38). The zero-order term is the point mass (eq. 38).
- The Cartesian kernels remove the polar singularity and run about 45 % faster (abstract).
- §5.3: accuracy is "strongly sensitive" to the tesseroid's shape and to small distances. Heck &
  Seitz 2007 "found unacceptably large errors" in the near zone and recommended equivalent prisms
  there. This paper subdivides near-zone tesseroids instead, and says the origin of the near-zone
  problems "is the subject of ongoing investigations".
- §6: GOCE-height tests in mE, against prisms and a spherical shell (Figs 3–6).
- **Relation:** closely parallels the draft's order ladder. The point mass at the centre is order 2
  (order 3 when the centroid is the expansion point), and the second-moment Taylor terms give order 4.
  This is prior art for "point mass plus second moment of an elementary cell" in geodesy; the draft
  should cite it. The difference is that their cells are whole tesseroids with symmetric moments,
  where the draft's cut pieces are irregular.

### Heck & Seitz 2007 [NONE]
Heck, B., Seitz, K. "A comparison of the tesseroid, prism and point-mass approaches for mass
reductions in gravity field modelling." *Journal of Geodesy* 81(2), 121–136, 2007 (Crossref: online
2006-09-26). doi:10.1007/s00190-006-0094-0 (verified).
Known only through Grombein 2013: it gives the zero- and second-order Taylor coefficients and found
large near-zone errors. **Relation:** compares point mass, prism and tesseroid, the same comparison
as the draft's cell-approximation orders. Fetch it.

---

## 3. Polyhedra

### Werner & Scheeres 1997 [NONE]
Werner, R. A., Scheeres, D. J. "Exterior gravitation of a polyhedron derived and compared with
harmonic and mascon gravitation representations of asteroid 4769 Castalia." *Celestial Mechanics
and Dynamical Astronomy* 65(3), 313–344. Crossref gives the year as 1997 with no page field; other
listings give 1996. doi:10.1007/BF00053511.
Not read: Springer blocked the fetch and no free copy was found. **I could not check eqs 10, 15
and 17**, which the draft cites; the draft's line 246 uses "their eq. 17" for ∇²U = −Gρ Σ ω_f. The
mascon comparison numbers for Castalia were also not obtained. The abstract, via search snippets,
says harmonic and mascon models have large errors near the body.

### Werner 1994 [NONE] (missing from the brief; relevant)
Werner, R. A. "The gravitational potential of a homogeneous polyhedron or don't cut corners."
*Celestial Mechanics & Dynamical Astronomy* 59, 253–278, 1994. doi:10.1007/BF00692875 (verified).
The precursor of Werner & Scheeres.

### Werner 1997 [NONE]
Werner, R. A. "Spherical harmonic coefficients for the potential of a constant-density polyhedron."
*Computers & Geosciences* 23(10), 1071–1077, 1997. doi:10.1016/S0098-3004(97)00110-6 (verified).
Exact multipole (Stokes) coefficients of a uniform polyhedron. **Relation:** prior art for exact
polyhedral moments of every order. The draft's piece moments (orders 0–2) are its low-order case in
Cartesian form. Cite it next to Mirtich.

### Werner 2017 [NONE]
Werner, R. A. "The solid angle hidden in polyhedron gravitation formulations." *Journal of Geodesy*
91(3), 307–328. Crossref: online 2016-10-20, issue 2017. doi:10.1007/s00190-016-0964-z (verified).
Not read; the only full-text copy found was on an unauthorised mirror, which I did not use. Search
snippets say it shows that other formulations (Okabe, Holstein, Petrović/Tsoulis) contain the
per-face solid angle in disguise, and that a face's solid angle needs only one arctangent. Snippets
also say Holstein & Ketteridge 1996 still use two arctangents per edge of every face.
**It is unresolved whether Werner 2017 writes N = r₁·(r₂×r₃) as h_f·2A_f or discusses its rounding.
Read it before claiming the h_f·2A_f numerator as new.**

### Tsoulis 2012 [NONE]
Tsoulis, D. "Analytical computation of the full gravity tensor of a homogeneous arbitrarily shaped
polyhedral source using line integrals." *Geophysics* 77(2), F1–F11, 2012. doi:10.1190/geo2010-0334.1
(verified). Already in the bib. In the line-integral form the face term is multiplied by the
point-to-plane distance h_p explicitly: the height appears there, but as a factor, not inside the
solid angle's arctangent. This is from my knowledge of the form, not a reading this session.

### Petrović 1996 [NONE]
Petrović, S. "Determination of the potential of homogeneous polyhedral bodies using line integrals."
*Journal of Geodesy* 71(1), 44–52, 1996. doi:10.1007/s001900050074 (verified).

### Holstein & Ketteridge 1996 [ABS]
Holstein, H., Ketteridge, B. "Gravimetric analysis of uniform polyhedra." *Geophysics* 61(2),
357–364, 1996. doi:10.1190/1.1443964 (verified).
The abstract says, verbatim in substance, that the closed forms have "numerical error that increases
with distance from the target, while the anomaly decreases", which limits their operating range. The
paper proposes procedures with "(1) errors that diminish with distance, (2) enhanced operating range,
and (3) algorithmic simplicity". A published code needs double precision (1 part in 10¹⁶) where
theirs needs single (1 part in 10⁷) for comparable results, in about half the time.
**Tables 2–5 and the identity r − |ℓ| = r₀²/(r+|ℓ|), both cited by the draft (lines 256 and 284),
could not be checked; the full text is paywalled at SEG.** This is the **closest prior art for the
draft's §Numerical stability**: it already analyses far-field rounding classes and fixes them. The
draft must show what it adds beyond this paper, for example the solid-angle form and the h_f·2A_f
numerator, if those are not already there.

### Holstein 2002 [ABS]; Holstein 2003 [NONE]
Holstein, H. "Gravimagnetic similarity in anomaly formulas for uniform polyhedra." *Geophysics*
67(4), 1126–1133, 2002. doi:10.1190/1.1500373 (verified). Unifies the gravity and magnetic formulas
through per-edge vector functions and discusses singularities. The abstract has no rounding numbers.
Holstein, H. "Gravimagnetic anomaly formulas for polyhedra of spatially linear media." *Geophysics*
68(1), 157–167. Crossref: online 2002-09-10, issue 2003. doi:10.1190/1.1543203 (verified).
Both are optional for the draft.

### D'Urso 2013 and 2014 [NONE]
D'Urso, M. G. "On the evaluation of the gravity effects of polyhedral bodies and a consistent
treatment of related singularities." *Journal of Geodesy* 87(3), 239–252. Crossref: online
2012-09-26, issue 2013. doi:10.1007/s00190-012-0592-1 (verified).
D'Urso, M. G. "Analytical computation of gravity effects for polyhedral bodies." *Journal of
Geodesy* 88(1), 13–29. Crossref: online 2013-10-18, issue 2014. doi:10.1007/s00190-013-0664-x
(verified).
Both treat singularities, which matter for the draft's "on the surface" paragraph. I did not find
the brief's *Surveys in Geophysics* D'Urso paper, so no DOI is proposed for it.

### Conway 2015 [NONE]
Conway, J. T. "Analytical solution from vector potentials for the gravitational field of a general
polyhedron." *Celestial Mechanics and Dynamical Astronomy* 121(1), 17–38. Crossref: online
2014-11-15, issue 2015. doi:10.1007/s10569-014-9588-x (verified).

### Ren et al. 2018 [ABS]; Ren et al. arXiv 2018 [ABS]
Ren, Z., Zhong, Y., Chen, C., Tang, J., et al. "Gravity Gradient Tensor of Arbitrary 3D Polyhedral
Bodies with up to Third-Order Polynomial Horizontal and Vertical Mass Contrasts." *Surveys in
Geophysics* 39, 901–935, 2018. doi:10.1007/s10712-018-9467-1 (verified; OA, but the PDF fetch
returned HTML). The abstract says their constant and linear solutions "cover all previously published
vertex-type exact solutions … though the associated algorithms may differ in numerical stability".
It gives no rounding numbers.
Ren, Z. et al. "Exact gravity field for polyhedrons with polynomial density contrasts of arbitrary
orders." arXiv:1810.11768, 2018 [ABS]: singularity-free, with no far-field stability analysis.
**Relation:** low relevance to the stability question.

### Mirtich 1996 [ABS: author page]
Mirtich, B. "Fast and Accurate Computation of Polyhedral Mass Properties." *Journal of Graphics
Tools* 1(2), 31–50, 1996. doi:10.1080/10867651.1996.10487458 (verified).
Centre of mass and moments and products of inertia of uniform polyhedra. All the volume integrals
are computed together by the divergence and Green's theorems, reduced to projection integrals, and
the method is "designed to minimize the numerical errors that can result from poorly conditioned
alignment of polyhedral faces". The author page gives no error numbers. **Relation:** standard prior
art for the exact mass, centroid and second moment of a polyhedral piece. The draft's piece moments
are not new as a computation, and the draft should cite Mirtich (and Powell & Abel, item 7).

---

## 4. Mascons

### Muller & Sjogren 1968 [ABS]
Muller, P. M., Sjogren, W. L. "Mascons: Lunar Mass Concentrations." *Science* 161(3842), 680–684,
1968. doi:10.1126/science.161.3842.680 (verified). The origin of the term: Lunar Orbiter tracking
shows mass concentrations under the ringed maria. Historical citation only.

### Wittick & Russell 2017 [NONE]; Wittick & Russell 2019 [NONE]
"Mascon models for small body gravity fields" (2017) appears to be an AAS/AIAA conference paper.
Neither Crossref nor search found a DOI, so **no BibTeX entry is proposed**. A later journal paper
does exist: Wittick, P. T., Russell, R. P. "Mixed-model gravity representations for small celestial
bodies using mascons and spherical harmonics." *Celestial Mechanics and Dynamical Astronomy* 131,
2019. doi:10.1007/s10569-019-9904-6 (verified; Crossref gives no pages, it is an article-number
journal).

### Rathinam & Dempster 2019 [NONE] (missing from the brief; directly relevant)
Rathinam, A., Dempster, A. G. "Octree-Based Mascon Model for Small Body Gravity Fields." *Journal of
Guidance, Control, and Dynamics* 42(11), 2557–2567, 2019. doi:10.2514/1.g004008 (verified). Mascons placed on an octree of a shape model: **closest prior
art for "sparse octree of voxel masses for a body's gravity"**. Not read; fetch it.

### Fanti & Izzo, MasconCube (arXiv:2509.08607, 2025/26) [PART: HTML]
A regular 100³ grid of point masses (§2.2). Mascons with centres outside the mesh are removed, then
the masses are fitted. A "cubified" ground truth (cells inside the mesh at their centres) has a
relative acceleration error of 2.56×10⁻⁴ on Eros (Table 3), against 4.43×10⁻⁴ for the fitted
MasconCube. There is no resolution-scaling study. **Relation:** confirms that 0/1 cube filling sits
at the ~10⁻⁴ level at 100³ cells, roughly what the draft's first-order bound predicts. An arXiv
preprint, so no Crossref DOI.

### Mascon error numbers for Castalia (Werner & Scheeres 1997)
Not obtained; see item 3. This is the largest gap in the survey.

---

## 5. Cut cells and embedded boundaries

### Johansen & Colella 1998 [ABS: OSTI report]
Johansen, H., Colella, P. "A Cartesian Grid Embedded Boundary Method for Poisson's Equation on
Irregular Domains." *Journal of Computational Physics* 147(1), 60–85, 1998.
doi:10.1006/jcph.1998.5965 (verified). Two-dimensional, conservative differencing on cut-cell
volumes, second order (abstract).

### Colella, Graves, Keen & Modiano 2006 [PART: OSTI PDF, geometry section]
Colella, P., Graves, D. T., Keen, B. J., Modiano, D. "A Cartesian grid embedded boundary method for
hyperbolic conservation laws." *Journal of Computational Physics* 211(1), 347–366, 2006.
doi:10.1016/j.jcp.2005.05.026 (verified). The per-cell geometry stored is the volume fraction κ, the
area fractions α, the face and EB-face centroids, and the mean EB normal (geometry list in §2).

### Schwartz, Barad, Colella & Ligocki 2006 [NONE]
"A Cartesian grid embedded boundary method for the heat equation and Poisson's equation in three
dimensions." *Journal of Computational Physics* 211(2), 531–550, 2006. doi:10.1016/j.jcp.2005.06.010
(verified). The UNT PDF did not download.

### Schwartz, Percelay, Ligocki, Johansen, Graves, Devendran, Colella & Ateljevich 2015 [FULL]
**This is the paper the brief was looking for.** "High-accuracy embedded boundary grid generation
using the divergence theorem." *Communications in Applied Mathematics and Computational Science*
10(1), 83–96, 2015. doi:10.2140/camcos.2015.10.83 (verified; OA at msp.org). Crossref lists the
first five authors. Project Euclid also lists Devendran, Colella and Ateljevich.
- eq. (3): the **volume moments of each cut cell**, m_v^p(x̄) = ∫_V (x − x̄)^p dV for multi-indices p.
  Eq. (4) gives face moments, eqs (5)–(6) EB-face moments, and eq. (9) the volume fraction
  κ = h^{−D} m_v^0.
- eq. (17): a finite-volume divergence of order P needs fluxes of order Q = P+1 and moments of order
  R = P+D.
- Algorithm (§3, Fig. 2): moments from an implicit function by the divergence theorem, recursively
  by dimension. They are approximate, O(h^R). Fig. 3 and §3.1: **when the geometry is under-resolved,
  refine locally and sum the finer cells' moments to get the coarse cell's**.
- §4.2, Tables 1–3: moments converge at O(h⁷) in 2D and O(h⁸) in 3D for P = 5. §4.3, Table 4: flux
  divergence converges at 3rd order at the boundary and 4th away from it.
- Implemented in Chombo (LBNL).
- **Relation:** per-cut-cell volume moments of arbitrary order, used to raise the scheme's order, are
  established in EB finite-volume methods. Summing child moments into a parent cell is in the same
  paper. The draft's "cut-cell moments" are therefore **not new as a data structure**. The draft
  differs in three ways: (a) exact moments of polyhedral pieces, where Schwartz et al. compute
  approximate moments from implicit functions; (b) the use: moments as a gravity multipole source,
  with error order measured in far-field gravity, not in a flux divergence; (c) the error-order
  ladder 1/2/3/4 for gravity. Cite this paper.

### Aftosmis, Berger & Melton 1998 [NONE]
"Robust and Efficient Cartesian Mesh Generation for Component-Based Geometry." *AIAA Journal* 36(6),
952–960, 1998. doi:10.2514/2.464 (verified). Robust intersection of triangulations with Cartesian
cells, giving cut-cell volumes and areas. Robustness prior art for the clipper.

### Müller, Kummer & Oberlack 2013 [NONE]
"Highly accurate surface and volume integration on implicit domains by means of moment-fitting."
*International Journal for Numerical Methods in Engineering* 96(8), 512–528, 2013.
doi:10.1002/nme.4569 (verified). Quadrature rules on cut cells fitted to moments.

### Saye 2015 [ABS]
Saye, R. I. "High-Order Quadrature Methods for Implicitly Defined Surfaces and Volumes in
Hyperrectangles." *SIAM Journal on Scientific Computing* 37(2), A993–A1019, 2015.
doi:10.1137/140966290 (verified). Recursive height-function quadrature with positive weights. The
abstract reports convergence orders up to 20. Its target is implicit geometry, not polyhedral, so it
is related but is not prior art for exactness.

---

## 6. Tree gravity

### Barnes & Hut 1986 [NONE]
"A hierarchical O(N log N) force-calculation algorithm." *Nature* 324(6096), 446–449, 1986.
doi:10.1038/324446a0 (verified; the bib entry lacks issue 6096, which is fine).

### Greengard & Rokhlin 1987 [ABS]
"A fast algorithm for particle simulations." *Journal of Computational Physics* 73(2), 325–348,
1987. doi:10.1016/0021-9991(87)90140-9 (verified). The abstract claims O(N) work "to within roundoff
error".

### Springel 2005 [PART: arXiv version, §3.1]
"The cosmological simulation code GADGET-2." *MNRAS* 364(4), 1105–1134, 2005.
doi:10.1111/j.1365-2966.2005.09655.x (verified).
- eq. (18): the relative opening criterion (GM/r²)(l/r)² ≤ α|a|, with |a| the previous step's total
  acceleration. It tries to "limit the absolute force error … in each particle-node interaction".
- eq. (19): an extra safeguard, |r_k − c_k| ≤ 0.6 l, which forces the node open if the particle
  lies within a box 20 % larger than the node. It is there because of Salmon & Warren's unbounded
  worst cases.
- Fig. 1: tolerance α ∈ {0.0005 … 0.02} and θ ∈ {0.3 … 0.8}. The force-error axis spans
  10⁻⁴ … 10⁻¹, and the 99.9th-percentile errors sit around 10⁻³–10⁻². §1 says errors up to about
  1 % are acceptable for collisionless dynamics.
- **Relation:** the usual tree-code accuracy (10⁻³ to 10⁻⁴) is three to four orders looser than the
  draft's 10⁻⁷. The draft's acceptance rule is in the Salmon–Warren/Dehnen family, not the GADGET
  family.

### Salmon & Warren 1994 [NONE]; Warren & Salmon 1993 [PART]
Salmon, J. K., Warren, M. S. "Skeletons from the Treecode Closet." *Journal of Computational
Physics* 111(1), 136–155, 1994. doi:10.1006/jcph.1994.1050 (verified). Not read.
Read instead: Warren & Salmon, "A Parallel Hashed Oct-Tree N-Body Algorithm" (Supercomputing '93),
§2.2, which summarises the Skeletons bound:
- BH worst-case errors "can be quite large (in fact, unbounded)" for common θ.
- The bound (their eq. 1) depends on d, the cell's b_max, the order p and moments B_(n) (eq. 2).
- The MAC becomes d > r_c, with r_c solved once per cell from a user absolute tolerance (eq. 3).
  For p = 1, r_c has a closed form using B₂, "the trace of the quadrupole moment tensor".
- The abstract claims accuracy adjustable "between a few percent relative accuracy, down to machine
  arithmetic accuracy".
No DOI was checked for the SC'93 paper.

### Dehnen 2014 [PART: arXiv, abstract and §3–5 skimmed]
"A fast multipole method for stellar dynamics." *Computational Astrophysics and Cosmology* 1,
article 1, 2014. doi:10.1186/s40668-014-0001-7 (verified). A new error estimate sets per-interaction
opening. The abstract reports **relative force errors of 10⁻⁷ at empirical cost ∝ N^0.87**,
outperforming GPU direct summation for N ≳ 10⁵. **Relation:** the one tree/FMM paper that works at
the draft's 10⁻⁷ level. It supports the draft's claim that error-bound acceptance, not angle
acceptance, is needed there.

---

## 7. Graphics and particle-mesh analogues

### Catmull 1978 [ABS]
"A hidden-surface algorithm with anti-aliasing." *Proceedings of the 5th Annual Conference on
Computer Graphics and Interactive Techniques (SIGGRAPH '78)*, 6–11, 1978.
doi:10.1145/800248.807360 (verified). Exact area-weighted pixel coverage by clipping polygons per
pixel. This is the 2-D order-0 analogue of exact fill.

### Duff 1989 [NONE]
Duff, T. "Polygon scan conversion by exact convolution." In *Raster Imaging and Digital Typography*
(Proc. Int. Conf.), Cambridge University Press, 1989, pp. 154–168. The citation comes from a patent;
there is no DOI. Exact convolution of polygons with polynomial (spline) filters, which amounts to
integrating polynomials over clipped polygons. It is the 2-D analogue of the draft's higher piece
moments. Bib entry without DOI; check the pages.

### Powell & Abel 2015 [FULL: arXiv:1412.4941]
"An exact general remeshing scheme applied to physically conservative voxelization." *Journal of
Computational Physics* 297, 340–356, 2015. doi:10.1016/j.jcp.2015.05.022 (verified). Code: r3d
(github.com/devonmpowell/r3d).
- §3: each tetrahedron–voxel intersection is a convex polyhedron. Its **volume and coordinate
  moments up to quadratic order** (1, x, y, z, x², …, yz: 10 moments) come from a simplicial
  decomposition (§4.3).
- §4.2, eq. (4.3): rounding grows cubically in |x|/Δx when moments are taken in absolute coordinates
  and only linearly in a voxel-centred frame. The paper calls this "essential to the overall
  accuracy".
- Table 3 (10⁵ random tetrahedra, 128³ grid, double precision): moment conservation is rms
  1.6–1.7×10⁻¹², max 5.2–5.7×10⁻¹⁰ for constant, linear and quadratic moments. For grid-degenerate
  vertices (Table 4) it is rms 5.6–6.1×10⁻¹⁴.
- Used for density deposition (dark-matter sheet, PM, lensing), not for evaluating gravity from
  piece moments.
- **Relation: the most direct prior art.** Exact per-voxel mass, first moments and second moments of
  clipped polyhedral pieces, with a cancellation analysis of the moments. The draft's "cut-cell
  moments" as a computation are this. The draft's new part is using them as a gravity source and
  showing the error-order ladder in the field. The draft should also say whether it centres
  coordinates per cell, as Powell & Abel found essential.

### Hockney & Eastwood [NONE]
*Computer Simulation Using Particles.* Crossref holds the CRC reissue, doi:10.1201/9780367806934
(issued 2021-03-24); the original is 1981, with a 1988 edition from Adam Hilger. Cloud-in-cell and
TSC mass assignment are the particle-mesh analogue. Cite the edition used, with this DOI flagged as
the reissue.

---

## (b) Proposed BibTeX entries (Crossref-verified DOIs only)

```bibtex
@article{nagy2000,
  author={Nagy, Dezs{\H o} and Papp, G{\'a}bor and Benedek, Judit},
  title={The gravitational potential and its derivatives for the prism},
  journal={Journal of Geodesy}, volume={74}, number={7--8}, pages={552--560}, year={2000},
  doi={10.1007/s001900000116}}
@article{plouff1976,
  author={Plouff, Donald},
  title={Gravity and magnetic fields of polygonal prisms and application to magnetic terrain corrections},
  journal={Geophysics}, volume={41}, number={4}, pages={727--741}, year={1976}, doi={10.1190/1.1440645}}
@article{banerjee1977,
  author={Banerjee, B. and Das Gupta, S. P.},
  title={Gravitational attraction of a rectangular parallelepiped},
  journal={Geophysics}, volume={42}, number={5}, pages={1053--1055}, year={1977}, doi={10.1190/1.1440766}}
@article{fukushima2020,
  author={Fukushima, Toshio},
  title={Speed and accuracy improvements in standard algorithm for prismatic gravitational field},
  journal={Geophysical Journal International}, volume={222}, number={3}, pages={1898--1908}, year={2020},
  doi={10.1093/gji/ggaa240}}
@article{li1998,
  author={Li, Xiong and Chouteau, Michel},
  title={Three-dimensional gravity modeling in all space},
  journal={Surveys in Geophysics}, volume={19}, number={4}, pages={339--368}, year={1998},
  doi={10.1023/A:1006554408567}}
@article{uieda2016,
  author={Uieda, Leonardo and Barbosa, Val{\'e}ria C. F. and Braitenberg, Carla},
  title={Tesseroids: Forward-modeling gravitational fields in spherical coordinates},
  journal={Geophysics}, volume={81}, number={5}, pages={F41--F48}, year={2016}, doi={10.1190/geo2015-0204.1}}
@article{soler2019,
  author={Soler, Santiago R. and Pesce, Agustina and Gimenez, Mario E. and Uieda, Leonardo},
  title={Gravitational field calculation in spherical coordinates using variable densities in depth},
  journal={Geophysical Journal International}, volume={218}, number={3}, pages={2150--2164}, year={2019},
  doi={10.1093/gji/ggz277}}
@article{grombein2013,
  author={Grombein, Thomas and Seitz, Kurt and Heck, Bernhard},
  title={Optimized formulas for the gravitational field of a tesseroid},
  journal={Journal of Geodesy}, volume={87}, number={7}, pages={645--660}, year={2013},
  doi={10.1007/s00190-013-0636-1}}
@article{heck2007,
  author={Heck, Bernhard and Seitz, Kurt},
  title={A comparison of the tesseroid, prism and point-mass approaches for mass reductions in gravity field modelling},
  journal={Journal of Geodesy}, volume={81}, number={2}, pages={121--136}, year={2007},
  doi={10.1007/s00190-006-0094-0}}
@article{werner1994,
  author={Werner, Robert A.},
  title={The gravitational potential of a homogeneous polyhedron or don't cut corners},
  journal={Celestial Mechanics and Dynamical Astronomy}, volume={59}, pages={253--278}, year={1994},
  doi={10.1007/BF00692875}}
@article{werner1997sh,
  author={Werner, Robert A.},
  title={Spherical harmonic coefficients for the potential of a constant-density polyhedron},
  journal={Computers \& Geosciences}, volume={23}, number={10}, pages={1071--1077}, year={1997},
  doi={10.1016/S0098-3004(97)00110-6}}
@article{werner2017,
  author={Werner, Robert A.},
  title={The solid angle hidden in polyhedron gravitation formulations},
  journal={Journal of Geodesy}, volume={91}, number={3}, pages={307--328}, year={2017},
  doi={10.1007/s00190-016-0964-z}}
@article{petrovic1996,
  author={Petrovi{\'c}, Svetozar},
  title={Determination of the potential of homogeneous polyhedral bodies using line integrals},
  journal={Journal of Geodesy}, volume={71}, number={1}, pages={44--52}, year={1996},
  doi={10.1007/s001900050074}}
@article{holstein2002,
  author={Holstein, Horst},
  title={Gravimagnetic similarity in anomaly formulas for uniform polyhedra},
  journal={Geophysics}, volume={67}, number={4}, pages={1126--1133}, year={2002}, doi={10.1190/1.1500373}}
@article{durso2013,
  author={D'Urso, M. G.},
  title={On the evaluation of the gravity effects of polyhedral bodies and a consistent treatment of related singularities},
  journal={Journal of Geodesy}, volume={87}, number={3}, pages={239--252}, year={2013},
  doi={10.1007/s00190-012-0592-1}}
@article{durso2014,
  author={D'Urso, M. G.},
  title={Analytical computation of gravity effects for polyhedral bodies},
  journal={Journal of Geodesy}, volume={88}, number={1}, pages={13--29}, year={2014},
  doi={10.1007/s00190-013-0664-x}}
@article{conway2015,
  author={Conway, John T.},
  title={Analytical solution from vector potentials for the gravitational field of a general polyhedron},
  journal={Celestial Mechanics and Dynamical Astronomy}, volume={121}, number={1}, pages={17--38}, year={2015},
  doi={10.1007/s10569-014-9588-x}}
@article{mirtich1996,
  author={Mirtich, Brian},
  title={Fast and accurate computation of polyhedral mass properties},
  journal={Journal of Graphics Tools}, volume={1}, number={2}, pages={31--50}, year={1996},
  doi={10.1080/10867651.1996.10487458}}
@article{muller1968,
  author={Muller, P. M. and Sjogren, W. L.},
  title={Mascons: Lunar mass concentrations},
  journal={Science}, volume={161}, number={3842}, pages={680--684}, year={1968},
  doi={10.1126/science.161.3842.680}}
@article{rathinam2019,
  author={Rathinam, Arunkumar and Dempster, Andrew G.},
  title={Octree-based mascon model for small body gravity fields},
  journal={Journal of Guidance, Control, and Dynamics}, volume={42}, number={11}, pages={2557--2567}, year={2019},
  doi={10.2514/1.G004008}}
@article{wittick2019,
  author={Wittick, Patrick T. and Russell, Ryan P.},
  title={Mixed-model gravity representations for small celestial bodies using mascons and spherical harmonics},
  journal={Celestial Mechanics and Dynamical Astronomy}, volume={131}, number={7}, year={2019},
  doi={10.1007/s10569-019-9904-6}}
@article{johansen1998,
  author={Johansen, Hans and Colella, Phillip},
  title={A {Cartesian} grid embedded boundary method for {Poisson}'s equation on irregular domains},
  journal={Journal of Computational Physics}, volume={147}, number={1}, pages={60--85}, year={1998},
  doi={10.1006/jcph.1998.5965}}
@article{colella2006,
  author={Colella, Phillip and Graves, Daniel T. and Keen, Benjamin J. and Modiano, David},
  title={A {Cartesian} grid embedded boundary method for hyperbolic conservation laws},
  journal={Journal of Computational Physics}, volume={211}, number={1}, pages={347--366}, year={2006},
  doi={10.1016/j.jcp.2005.05.026}}
@article{schwartz2006,
  author={Schwartz, Peter and Barad, Michael and Colella, Phillip and Ligocki, Terry},
  title={A {Cartesian} grid embedded boundary method for the heat equation and {Poisson}'s equation in three dimensions},
  journal={Journal of Computational Physics}, volume={211}, number={2}, pages={531--550}, year={2006},
  doi={10.1016/j.jcp.2005.06.010}}
@article{schwartz2015,
  author={Schwartz, Peter and Percelay, Julie and Ligocki, Terry J. and Johansen, Hans and Graves, Daniel T. and Devendran, Dharshi and Colella, Phillip and Ateljevich, Eli},
  title={High-accuracy embedded boundary grid generation using the divergence theorem},
  journal={Communications in Applied Mathematics and Computational Science}, volume={10}, number={1}, pages={83--96}, year={2015},
  doi={10.2140/camcos.2015.10.83}}
@article{aftosmis1998,
  author={Aftosmis, M. J. and Berger, M. J. and Melton, J. E.},
  title={Robust and efficient {Cartesian} mesh generation for component-based geometry},
  journal={AIAA Journal}, volume={36}, number={6}, pages={952--960}, year={1998}, doi={10.2514/2.464}}
@article{muller2013,
  author={M{\"u}ller, B. and Kummer, F. and Oberlack, M.},
  title={Highly accurate surface and volume integration on implicit domains by means of moment-fitting},
  journal={International Journal for Numerical Methods in Engineering}, volume={96}, number={8}, pages={512--528}, year={2013},
  doi={10.1002/nme.4569}}
@article{saye2015,
  author={Saye, R. I.},
  title={High-order quadrature methods for implicitly defined surfaces and volumes in hyperrectangles},
  journal={SIAM Journal on Scientific Computing}, volume={37}, number={2}, pages={A993--A1019}, year={2015},
  doi={10.1137/140966290}}
@article{greengard1987,
  author={Greengard, L. and Rokhlin, V.},
  title={A fast algorithm for particle simulations},
  journal={Journal of Computational Physics}, volume={73}, number={2}, pages={325--348}, year={1987},
  doi={10.1016/0021-9991(87)90140-9}}
@article{springel2005,
  author={Springel, Volker},
  title={The cosmological simulation code {GADGET-2}},
  journal={Monthly Notices of the Royal Astronomical Society}, volume={364}, number={4}, pages={1105--1134}, year={2005},
  doi={10.1111/j.1365-2966.2005.09655.x}}
@inproceedings{catmull1978,
  author={Catmull, Edwin},
  title={A hidden-surface algorithm with anti-aliasing},
  booktitle={Proceedings of the 5th Annual Conference on Computer Graphics and Interactive Techniques (SIGGRAPH '78)},
  pages={6--11}, year={1978}, doi={10.1145/800248.807360}}
```

Without a DOI, as flagged above: Duff 1989 (Raster Imaging and Digital Typography, CUP, pp. 154–168)
and Hockney & Eastwood (edition to choose; the CRC reissue is doi:10.1201/9780367806934).

Mismatches and flags:
- Werner & Scheeres: Crossref year 1997 with no pages; other listings say 1996. Keep pages 313–344.
- Werner 2017, D'Urso 2013/2014, Conway 2015 and Holstein 2003: Crossref's online date is the year
  before the issue year. The issue year is used above.
- Existing bib entries checked and consistent: nagy1966, werner1997 (W&S), tsoulis2012, holstein1996,
  barnes1986, salmon1994, dehnen2014, powell2015.

## (c) papers-to-fetch

Papers not read in full this session:
- Exterior gravitation of a polyhedron derived and compared with harmonic and mascon gravitation
  representations of asteroid 4769 Castalia (Werner & Scheeres): 10.1007/BF00053511. **Check eqs 10,
  15, 17 and the mascon numbers.**
- The solid angle hidden in polyhedron gravitation formulations (Werner 2017):
  10.1007/s00190-016-0964-z. **Check for h_f·2A_f.**
- Gravimetric analysis of uniform polyhedra (Holstein & Ketteridge 1996): 10.1190/1.1443964.
  **Check Tables 2–5 and r − |ℓ| = r₀²/(r+|ℓ|).**
- Analytical expressions for gravity anomalies due to homogeneous polyhedral bodies… (Okabe 1979):
  10.1190/1.1440973. **Check eq. 45.**
- The gravitational potential of a homogeneous polyhedron or don't cut corners (Werner 1994):
  10.1007/BF00692875
- Spherical harmonic coefficients for the potential of a constant-density polyhedron (Werner 1997):
  10.1016/S0098-3004(97)00110-6
- The gravitational potential and its derivatives for the prism (Nagy, Papp, Benedek 2000):
  10.1007/s001900000116
- Three-Dimensional Gravity Modeling In All Space (Li & Chouteau 1998): 10.1023/A:1006554408567
- A comparison of the tesseroid, prism and point-mass approaches for mass reductions in gravity field
  modelling (Heck & Seitz 2007): 10.1007/s00190-006-0094-0
- Tesseroids: Forward-modeling gravitational fields in spherical coordinates (Uieda et al. 2016):
  10.1190/geo2015-0204.1
- Determination of the potential of homogeneous polyhedral bodies using line integrals (Petrović
  1996): 10.1007/s001900050074
- Analytical computation of the full gravity tensor of a homogeneous arbitrarily shaped polyhedral
  source using line integrals (Tsoulis 2012): 10.1190/geo2010-0334.1
- On the evaluation of the gravity effects of polyhedral bodies and a consistent treatment of related
  singularities (D'Urso 2013): 10.1007/s00190-012-0592-1
- Analytical computation of gravity effects for polyhedral bodies (D'Urso 2014):
  10.1007/s00190-013-0664-x
- Analytical solution from vector potentials for the gravitational field of a general polyhedron
  (Conway 2015): 10.1007/s10569-014-9588-x
- Gravimagnetic similarity in anomaly formulas for uniform polyhedra (Holstein 2002):
  10.1190/1.1500373
- Gravity Gradient Tensor of Arbitrary 3D Polyhedral Bodies with up to Third-Order Polynomial
  Horizontal and Vertical Mass Contrasts (Ren et al. 2018): 10.1007/s10712-018-9467-1
- Fast and Accurate Computation of Polyhedral Mass Properties (Mirtich 1996):
  10.1080/10867651.1996.10487458
- Octree-Based Mascon Model for Small Body Gravity Fields (Rathinam & Dempster 2019):
  10.2514/1.G004008
- Mixed-model gravity representations for small celestial bodies using mascons and spherical
  harmonics (Wittick & Russell 2019): 10.1007/s10569-019-9904-6
- Mascon models for small body gravity fields (Wittick & Russell 2017): no DOI found
- Mascons: Lunar Mass Concentrations (Muller & Sjogren 1968): 10.1126/science.161.3842.680
- A Cartesian Grid Embedded Boundary Method for Poisson's Equation on Irregular Domains (Johansen &
  Colella 1998): 10.1006/jcph.1998.5965
- A Cartesian grid embedded boundary method for the heat equation and Poisson's equation in three
  dimensions (Schwartz et al. 2006): 10.1016/j.jcp.2005.06.010
- Robust and Efficient Cartesian Mesh Generation for Component-Based Geometry (Aftosmis et al.
  1998): 10.2514/2.464
- Highly accurate surface and volume integration on implicit domains by means of moment-fitting
  (Müller et al. 2013): 10.1002/nme.4569
- High-Order Quadrature Methods for Implicitly Defined Surfaces and Volumes in Hyperrectangles (Saye
  2015): 10.1137/140966290
- A hierarchical O(N log N) force-calculation algorithm (Barnes & Hut 1986): 10.1038/324446a0
- A fast algorithm for particle simulations (Greengard & Rokhlin 1987):
  10.1016/0021-9991(87)90140-9
- Skeletons from the Treecode Closet (Salmon & Warren 1994): 10.1006/jcph.1994.1050. **Check the
  bound the draft uses at line 694.**
- A hidden-surface algorithm with anti-aliasing (Catmull 1978): 10.1145/800248.807360
- Polygon scan conversion by exact convolution (Duff 1989): no DOI
- Computer Simulation Using Particles (Hockney & Eastwood): 10.1201/9780367806934 (reissue)

## (d) What is new and what is already in the literature

- **Far-field rounding classes are not new.** Holstein & Ketteridge 1996 (abstract) already analyse
  errors that grow with distance and give formulas whose errors diminish with distance. Fukushima
  2020 measures cubic growth outside the Brillouin sphere for the prism (§3.2, Fig. 2). The draft's
  contribution is narrower: a specific stabilisation of the Werner–Scheeres solid-angle form. It
  should be framed as that and should cite both.
- **The h_f·2A_f numerator: unverified.** The identity r₁·(r₂×r₃) = (n̂·r₁)·|(v₂−v₁)×(v₃−v₁)| is
  elementary. Line-integral forms (Petrović, Tsoulis) carry h_p as an explicit factor, and Werner
  2017 is specifically about the solid angle in these formulations. Neither Werner 2017 nor Holstein
  & Ketteridge could be read, so the claim is "plausibly new in the solid-angle form, pending Werner
  2017 and Holstein & Ketteridge 1996". Do not claim priority before reading both.
- **The log1p and conjugate forms of the edge log are known.** The draft itself credits Holstein &
  Ketteridge and Okabe eq. 45 for r − |ℓ| = r₀²/(r+|ℓ|). Fukushima 2020 eq. (24) uses the same
  conjugate move, ln[(Y²+Z²)/(R−X)], for the prism. log1p(2ℓ/(a+b−ℓ)) is the idiomatic
  floating-point form of 2·atanh(ℓ/(a+b)) and is unlikely to be a citable novelty.
- **Cut-cell moments are established prior art.** Per-cut-cell volume moments of arbitrary order are
  standard in Chombo-style embedded-boundary methods: Schwartz et al. 2015, eqs (3), (9), (17), which
  also refine and sum child moments for under-resolved cells. *Exact* per-voxel moments up to second
  order of clipped polyhedral pieces are in Powell & Abel 2015 (§4.3, Tables 3–4, code r3d). Exact
  polyhedron mass, centroid and inertia go back to Mirtich 1996, and exact multipoles of all orders to
  Werner 1997 (C&G). Point mass plus second-moment Taylor cells are in Heck & Seitz 2007 and Grombein
  et al. 2013.
- **What looks new:** (i) using exact cut-piece moments as the source representation for a
  gravity-gradient forward model, with a measured error-order ladder (1/2/3/4: 0/1 fill, fill at the
  centre, fill at the centroid, plus the second moment) in the far field. I found no paper that sets
  out this ladder for gravity from voxelised bodies. Tesseroid and mascon work tunes the distance
  ratio at 0.1 %, and MasconCube reports cubified error at about 2.6×10⁻⁴ without a scaling study.
  (ii) Operating a moment-summed sparse octree with error-bound acceptance at 10⁻⁷ relative accuracy
  over 10⁹–10¹¹ cells. Error-bound acceptance itself is Salmon & Warren/Dehnen, and Dehnen 2014
  reaches 10⁻⁷ for particles; the application to exact-moment voxel cells is the new part. Rathinam
  & Dempster 2019 (octree mascons) must be read to confirm (ii).
- **Accuracy context for the introduction:** typical tree codes run at 10⁻³–10⁻² (GADGET-2 Fig. 1),
  tesseroid codes at 10⁻³ (Uieda 2016, Soler 2019), and a 100³ cubified mascon model at about
  2.6×10⁻⁴ (MasconCube Table 3). The draft's ~10⁻⁷ is the regime only Dehnen 2014 (FMM, 10⁻⁷) and
  Warren & Salmon ("down to machine accuracy") address.
