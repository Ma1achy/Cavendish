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

**Updated 27 Nov 2026 from the owner's PDFs** (read as text, the scans and key pages as page images;
not copied into the repository): Werner 1994, Werner & Scheeres 1997, Werner 1997 (C&G), Werner
2017, Holstein & Ketteridge 1996, Okabe 1979, Petrović 1996, Tsoulis & Petrović 2001, Tsoulis 2012,
D'Urso 2014, Conway 2015, Nagy et al. 2000, Uieda et al. 2016, Heck & Seitz 2007, Mirtich 1996,
Johansen & Colella 1998, Schwartz et al. 2006, Salmon & Warren 1994 and Rathinam & Dempster 2019.
The entries below carry the new status; the answer on the h_f·2A_f numerator is in §(d) and in
[`../paper-benchmarks.md`](../paper-benchmarks.md) §2.

---

## 1. Prisms

### Nagy 1966 [NONE]
Nagy, D. "The gravitational attraction of a right rectangular prism." *Geophysics* 31(2), 362–371,
1966. doi:10.1190/1.1439779 (Crossref agrees with the bib entry.)

### Nagy, Papp & Benedek 2000 [PART: abstract and formula list]
Nagy, D., Papp, G., Benedek, J. "The gravitational potential and its derivatives for the prism."
*Journal of Geodesy* 74(7–8), 552–560, 2000. doi:10.1007/s001900000116 (verified).
Fukushima 2020 (below) describes it as the standard summary of "Bessel's exact formula": arctan and
log terms at the 8 vertices. The paper lists closed forms for the potential and its derivatives up to
third order, and its abstract claims they "satisfy the functional connections existing between these
quantities at the level of numerical precision applied". It has no far-field rounding analysis. **Relation:** a vertex-class formula (one term per vertex), so its
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

### Li & Chouteau 1998 [FULL] (batch 2, 28 Nov 2026)
Li, X., Chouteau, M. "Three-Dimensional Gravity Modeling In All Space." *Surveys in Geophysics*
19(4), 339–368, 1998. doi:10.1023/A:1006554408567 (verified).
- §5 "The Rounding Error" (pp. 355–359) restates H&K 1996: terms O(αγ^k), errors O(αγ^k ε); the
  vertex method needs γ ≫ ε^(1/4), and "double precision arithmetic even cannot operate at γ below
  10⁻⁴" (p. 357).
- Table I (p. 358), a 20 m cube: first breakdown at 10³ for single-precision Plouff-type codes, 10⁴
  for double-precision Haáz–Jung–Plouff, 10⁵ for double-precision Götze & Lahmeyer.
- Their verdict: rounding control "may have only theoretical meaning … does anyone care about a
  gravity effect of less than 1 µGal …? Definitely no" (p. 359); "insignificant … except if the
  available computer single precision is less than one part in 10⁷" (§8, p. 365). No h·2A form.
- **Relation:** a second measurement of vertex-class breakdown at ~10⁴ sizes in double precision,
  and a dismissal the draft should answer: at 10⁻⁷ of a gradiometer signal it matters.

---

## 2. Tesseroids

### Uieda, Barbosa & Braitenberg 2016 [PART: PDF, abstract checked against the full text]
Uieda, L., Barbosa, V. C. F., Braitenberg, C. "Tesseroids: Forward-modeling gravitational fields in
spherical coordinates." *Geophysics* 81(5), F41–F48, 2016. doi:10.1190/geo2015-0204.1 (verified).
The owner's PDF confirms the abstract's numbers below (D = 1, 1.5, 8 for 0.1 % against a spherical
shell; GLQ with a stack-based adaptive split). Nothing in it bears on rounding at 10⁻⁷.
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

### Heck & Seitz 2007 [FULL text; Fig. 7 viewed, Figs 8–9 not]
Heck, B., Seitz, K. "A comparison of the tesseroid, prism and point-mass approaches for mass
reductions in gravity field modelling." *Journal of Geodesy* 81(2), 121–136, 2007 (Crossref: online
2006-09-26). doi:10.1007/s00190-006-0094-0 (verified).
- Taylor (MacMillan) series of the tesseroid kept to second order (eq. 12); the second-order terms
  "vanish for a cube" (p. 125). Forsberg's TC program uses the exact prism near, the series in
  between and a point mass far (p. 125), the same layering as the draft's §7.
- Cost (Fig. 5, p. 129): for V the prism costs about 10× the tesseroid series, which costs 2× a point
  mass.
- **Rounding, the only remarks found:** the prism's logs "have been transformed with respect to
  Mader's (1951) formula in order to provide a better numerical stability" (p. 124); beyond about
  1.5° "the error is mainly induced by rounding errors of the computer (single precision IEEE
  arithmetic)" (p. 131); "the prism routine breaks down for large distances, due to the loss of
  significant digits" (p. 133). Unquantified, and no triple-product or area form.
- Fig. 7 (p. 131, read off a log plot, ±2×): at 10° the point mass is off by ~10⁻⁷ relative, the
  equal-mass prism by ~2×10⁻⁵ (wrong shape). Near zone (Figs 8–9, text only): nearby elements need
  the exact prism (p. 133).
- **Relation:** the published point-mass/second-moment/prism comparison; cite next to Grombein 2013.

---

## 3. Polyhedra

### Werner & Scheeres 1997 [FULL]
Werner, R. A., Scheeres, D. J. "Exterior gravitation of a polyhedron derived and compared with
harmonic and mascon gravitation representations of asteroid 4769 Castalia." *Celestial Mechanics
and Dynamical Astronomy* 65(3), 313–344. Crossref gives the year as 1997 with no page field; other
listings give 1996. doi:10.1007/BF00053511. The printed footer reads "Celestial Mechanics and
Dynamical Astronomy 65:313–344, 1997" (received 2 Jan 1996, accepted 1 Aug 1996), so **1997 is
right**.
- **Eq. (10)** (p. 325, restated p. 333): U = ½Gσ Σ_e r_e·E_e·r_e L_e − ½Gσ Σ_f r_f·F_f·r_f ω_f.
  **Eq. (15)** (p. 333): ∇U = −Gσ Σ_e E_e·r_e L_e + Gσ Σ_f F_f·r_f ω_f. **Eq. (16)**: ∇∇U =
  Gσ Σ E_e L_e − Gσ Σ F_f ω_f. **Eq. (17)** (pp. 328, 334): ∇²U = −Gσ Σ_f ω_f, which "vanishes where
  the field point is outside the polyhedron … and equals −4π inside". r_i runs from the field point
  to vertex P_i (p. 332), as in the draft. With F_f·r_f = n̂_f h_f and E_e·r_e split between the two
  faces, these are the draft's eq. (result) term for term. **The draft's citation of eqs 10, 15 and 17
  stands.** Δz is their name for r_f·n̂_f (p. 332), as the draft says.
- **Eq. (7)**: L_e = ln[(r_i + r_j + e_ij)/(r_i + r_j − e_ij)], the ratio form (no rounding remark).
  **Eq. (27)** (p. 333): ω_f = 2 arctan[r_i·r_j×r_k / (r_i r_j r_k + r_i(r_j·r_k) + r_j(r_k·r_i) +
  r_k(r_i·r_j))], derived from the half-angle formula (eqs 22–25).
- **§2.5.4, the step before eq. (26)** (p. 332): "An unnormalized vector aligned with a convex
  polygon's face normal is n_f = (r₂ − r₁) × (r₃ − r₂) = r₁×r₂ + r₂×r₃ + r₃×r₁", and then
  sign(Δz) = sign(r₁·n_f) = sign(r₁·r₂×r₃). Since |n_f| = 2A_f and r₁·n̂_f = Δz, this is the identity
  r₁·(r₂×r₃) = Δz·2A_f, **used only for the sign**. The value is then written as the triple product.
- **Castalia mascons** (§3.2, pp. 336–339): "approximately 3300 point masses corresponding to a
  modeling resolution of 59 meters". Potential: "agreement to within 0.1% over most of the body, even
  when evaluated close to the surface" (no figure shown). Attraction (Fig. 16): "between 1 and 10%
  over most of the surface but swell to over 10% at isolated pockets"; away from the body "between
  0.1 and 1%". At 41 m (~9700 mascons) there are "still … appreciable errors" (Fig. 17). One cube
  against a sphere of equal mass (Fig. 15): up to ~50% at face midpoints, not below 0.1% "until
  greater than 2.5 units from the center". Scaling (p. 338–339): error ∝ mascon size, so ten times
  less error needs "a 3 order of magnitude increase" in mascons, i.e. first order in the pitch: the
  draft's row (a).

### Werner 1994 [FULL]
Werner, R. A. "The gravitational potential of a homogeneous polyhedron or don't cut corners."
*Celestial Mechanics & Dynamical Astronomy* 59, 253–278, 1994. doi:10.1007/BF00692875 (verified).
The precursor of Werner & Scheeres; potential only.
- Eq. (3.2) (p. 258): each face contributes Δz × (a line integral − sign(Δz)·A_image), A_image the
  area of the face's image on the tangent sphere, i.e. its solid angle.
- Eq. (3.3) (p. 261): L₁₂ = ln[(r₁+r₂+r₁₂)/(r₁+r₂−r₁₂)]/r₁₂, ratio form, via MacMillan §43.
- **Eq. (3.4) and the text above it (§3.2.2, p. 261)**: the spherical vertex angle is
  S₂ = arctan{Δz[η₁(ξ₃−ξ₂) + η₂(ξ₁−ξ₃) + η₃(ξ₂−ξ₁)] / (−{det₁₂det₂₃ + Δz²[…]}/r₂)}, with (ξ, η)
  the vertices in a face-fixed frame. Werner: "The bracketed factors in the numerator and denominator
  are invariant for a rigid face. The bracketed numerator factor [η₁(ξ₃−ξ₂)+η₂(ξ₁−ξ₃)+η₃(ξ₂−ξ₁)] is
  twice the area of the planar triangle defined by P₁, P₂, and P₃." **This is the h_f·2A_f numerator
  in print**: height times twice the area, the area from the face's own coordinates, computed once.
  His motive is precomputation ("computed once and saved", p. 262), not rounding, and it sits in a
  per-vertex spherical-excess form (n arctangents per face, eq. 3.4 summed), not in the
  van Oosterom–Strackee single arctangent. The denominator's det₁₂ = Δx₁Δy₂ − Δx₂Δy₁ is
  field-point-dependent and not discussed for rounding.

### Werner 1997 [PART: abstract and method]
Werner, R. A. "Spherical harmonic coefficients for the potential of a constant-density polyhedron."
*Computers & Geosciences* 23(10), 1071–1077, 1997. doi:10.1016/S0098-3004(97)00110-6 (verified).
Abstract: "Recurrent relationships for the definitions of fully normalized spherical harmonic
coefficients C_n,m and S_n,m are derived and integrated analytically to yield the gravitational
potential of a constant-density polyhedron", with a C program. **Relation:** prior art for exact
polyhedral moments of every order. The draft's piece moments (orders 0–2) are its low-order case in
Cartesian form. Cite it next to Mirtich. (Its §1 cites W&S as "Werner and Scheeres (1996)".)

### Werner 2017 [FULL]
Werner, R. A. "The solid angle hidden in polyhedron gravitation formulations." *Journal of Geodesy*
91(3), 307–328. Crossref: online 2016-10-20, issue 2017. doi:10.1007/s00190-016-0964-z (verified).
- Shows the arctangent terms of the conventional forms (Okabe, Holstein, Petrović/Tsoulis, Strakhov)
  are gores' spherical vertex angles (§§10–12), so a face needs one atan2 (§13.2, eqs 39–40).
- **Eq. (37)** (§13.1, p. 320): the van Oosterom & Strackee triangle formula, numerator the box
  product [r₁, r₂, r₃] (or of unit vectors), "coordinate-free". **No h·2A form is given for it.**
- **Eq. (31)** (§9.4, p. 317): for a *gore* (a triangle with one vertex at the foot of the
  perpendicular), [r̂_F, r̂₁g, r̂₂g] = sgn h_ef · sgn v_f · ℓ_e|h_ef|/(r₁g r₂g), with r̂_F the unit
  vector to the foot: edge length times in-plane altitude, twice the gore's area; with r_F itself
  (length |v_f|) the triple product is v_f · ℓ_e h_ef, height × twice the area. Derived to fix signs.
- **§16 Conclusion (p. 323): "The numerical or computational properties of this paper's solid-angle
  formulas have not been analyzed."** So the paper makes no rounding claim at all.
- §15, Table 1 (p. 323): at three singular points of a 20×10×10 m prism, Werner agrees with D'Urso
  2014 to 10⁻¹⁴–10⁻¹⁵ and with Tsoulis 2012 to a uniform 7.4×10⁻⁸; footnote 2 guesses Tsoulis's
  published values are single precision.
- **Relation:** not prior art for h_f·2A_f in the van Oosterom–Strackee form, and explicit that the
  numerics are open. Cite it for the one-arctangent solid angle and for "not analysed".

### Tsoulis 2012 [PART: method and numerical assessment]
Tsoulis, D. "Analytical computation of the full gravity tensor of a homogeneous arbitrarily shaped
polyhedral source using line integrals." *Geophysics* 77(2), F1–F11, 2012. doi:10.1190/geo2010-0334.1
(verified). Already in the bib. Line-integral form after Petrović 1996 and Tsoulis & Petrović 2001,
with a documented FORTRAN program; h_p, the point-to-plane distance, appears as an explicit factor
(p. F3). Numerical assessment (p. F8): against the prism formula at points far away, on the surface
and on edge/face extensions, agreement "reaches at all accounts the internal accuracy levels of the
programming language", also "when the computed effects themselves reach the 10⁻¹⁷–10⁻²⁰ level in
absolute sense". It gives no relative error against distance, so it does not contradict the
ε(R/s)³ growth measured in ESA's implementation; the claim is in absolute terms.

### Tsoulis & Petrović 2001 [FULL]
"On the singularities of the gravity field of a homogeneous polyhedral body." *Geophysics* 66(2),
535–539 (already in the bib). The correction terms when the point's projection falls inside a face,
on an edge or on a vertex come from excluding a vanishing circle, semicircle or sector around it
(Figs 2–3) before applying the divergence theorem. Singularities only; no rounding analysis. The
draft's use of it (§"On the surface") stands.

### Petrović 1996 [PART: abstract and structure]
Petrović, S. "Determination of the potential of homogeneous polyhedral bodies using line integrals."
*Journal of Geodesy* 71(1), 44–52, 1996. doi:10.1007/s001900050074 (verified). Abstract: all ten
quantities (U, g, the tensor) from "only two different line integrals" per edge, with coordinate
transformations chosen so "they do not appear in the final result". There is no numerical section;
for P inside, "the formulas must be modified in the well-known usual way". Nothing on rounding.

### Holstein & Ketteridge 1996 [FULL, page images]
Holstein, H., Ketteridge, B. "Gravimetric analysis of uniform polyhedra." *Geophysics* 61(2),
357–364, 1996. doi:10.1190/1.1443964 (verified). Gravity (attraction) only, no potential.
- Notation (Table 1, p. 359): α the target size, δ its distance, **γ = α/δ**; ε the floating-point
  precision; κ = 1, 0, −1 for the surface, line and vertex methods.
- **Table 2** (p. 360), absolute error O(αγ^κ ε): surface O(αγε) "convergent", line O(αε)
  "bounded", vertex O(αγ⁻¹ε) "unbounded".
- **Table 3** (horizontal track, vertical component, p. 360), relative error O(γ^(κ−3)ε): surface
  γ⁻²ε, line γ⁻³ε, vertex γ⁻⁴ε; γ_crit = ε^(1/2), ε^(1/3), ε^(1/4).
- **Table 4** (inclined track, direction cosine γ′ fixed), relative error O(γ^(κ−2)ε): surface
  γ⁻¹ε, **line γ⁻²ε, vertex γ⁻³ε**.
- **Table 5** (p. 363): measured slopes 2.00, 3.02, 4.01 (Fig. 4, horizontal track) against theory
  2, 3, 4; γ_crit observed 10⁻⁷·⁰, 10⁻⁴·⁸, 10⁻³·⁵, matching theory with an effective ε = 10⁻¹⁴.
- **Eq. (22)** (p. 361), the surface method: n_i·∮ r∧dl/(r_ic+v_i) = 2A_i/(r_ic+v_i), "where A_i is
  the area of S_i. The area is a facet property that can be evaluated accurately without reference to
  the observation point."
- **Eq. (24)** (p. 361), the analytical line method: c = 2h arctanh[(ℓ₂−ℓ₁)/(r₁+r₂)] + c_v. "The
  quantity (ℓ₂ − ℓ₁) is the length of the edge … and can be calculated directly as an O(α) quantity
  rather than the difference of two O(δ) quantities." And: "many implementations define arctanh(t) to
  equal ½ ln((1+t)/(1−t)), which is subject to large rounding error when t ≈ γ ≪ 1." **This is the
  draft's edge rule**: L_e = ln[(a+b+ℓ)/(a+b−ℓ)] = 2 artanh(ℓ/(a+b)), and log1p(2ℓ/(a+b−ℓ)) is
  the accurate artanh they ask for. Eqs (25)–(26) give the matching arctangent c_v, its numerator
  built from (ℓ₂−ℓ₁), h and v.
- **p. 362, vertex method**: "Replacing differences of the form r − |ℓ| by r₀²/(r + |ℓ|) … we
  arrive at two cases", eqs (27)–(28). **The identity the draft cites is verbatim here.** Eq. (30):
  near an edge (η = r₀/L ≪ 1) the log term is O(Lη ln η) and is dropped below rounding.
- Their composite algorithm (Fig. 3) switches vertex → line → surface method with γ; in single
  precision it keeps convergent absolute error to 1/γ = 10²·⁶ where Götze's double-precision vertex
  code fails (Fig. 6, p. 363).
- **Relation: the closest prior art for the draft's §Numerical stability, now confirmed in full.**
  The rounding classes, the edge-length-not-difference principle, the facet area as an
  observation-independent quantity, the accurate artanh for the edge log and r − |ℓ| = r₀²/(r+|ℓ|)
  are all here. What is not here is the solid-angle (Werner–Scheeres) form, so the specific step
  "the van Oosterom–Strackee triple product is vertex-class; write it as h_f·2A_f" is not in H&K.

### Okabe 1979 [PART: §§ on g and the gradient tensor, page images pp. 733–736]
Okabe, M. "Analytical expressions for gravity anomalies due to homogeneous polyhedral bodies and
translations into magnetic anomalies." *Geophysics* 44(4), 730–741, 1979. doi:10.1190/1.1440973.
- Eqs (33)–(35) (p. 734): S_yz, S_zx, S_xy are "twice the projected areas of the triangular
  surface", used for the face's orientation angles, not for a solid-angle numerator.
- **Eq. (45)** (p. 736), in the second-derivative section: when η and Z tend to zero and ξ_j,
  ξ_{j+1} are both non-positive, {ln[ξ + (ξ²+η²+Z²)^½]} is replaced by {−ln[(ξ²+η²+Z²)^½ − ξ]}
  between the limits. This is the conjugate move; the two differ by ln(η²+Z²), which cancels between
  the limits. Okabe presents it as avoiding "the singularity", not rounding. **The draft's "Okabe's
  eq. 45 makes the same move" stands**; "for the gradient tensor's log, to avoid the singularity"
  would be more exact.

### Holstein 2002 [FULL] (batch 2, 28 Nov 2026); Holstein 2003 [NONE]
Holstein, H. "Gravimagnetic similarity in anomaly formulas for uniform polyhedra." *Geophysics*
67(4), 1126–1133, 2002. doi:10.1190/1.1500373 (verified).
- One edge vector b_ij = ĥ_ij C_ij − n_i Ω_ij carries gravity and magnetics (eq. 20); the Laplacian
  is −Σ Ω_ij, an inside/outside check (eq. 30).
- The per-edge solid angle is a half-angle form, Ω_ij = sign(v_i)·2 arctan λ_ij with
  λ_ij = h_ij Λ_ij/(|v_i| + Σ) and Λ_ij = L_ij/(r₂+r₁) (eqs 35–36, p. 1130): numerator h_ij·L_ij,
  twice the area of the edge's triangle with apex at the sub-observation point.
- **App. B, eq. (B-2), p. 1133**: "The summand h_ij L_ij is twice the area of the patch ij", with
  h_ij L_ij = n_i·(r₁_ij × r₂_ij), and "summation over all edges therefore gives twice the scalar
  facet area, 2A_i".
- **The far-field surface method uses it to remove cancellation**: 2Σ h_ij Λ*_ij = 2A_i/r_ic
  (eq. 42), the solid-angle offset likewise through A_i (eq. 43); p. 1131: "Analytical cancellation in
  the differences is essential for achieving far-field numerical accuracy." No error measurements of
  its own (it defers to H&K 1996 and Holstein et al. 1999). Eq. (43) as printed multiplies by
  (r_ic+|v_i|) where eq. (38) implies a division (a helper's derivation, not checked by me).
- **Relation: the closest published use of the triple product = twice-area identity against
  far-field cancellation.** It is per edge (h_ij the in-plane edge distance), inside Holstein's
  surface method, not the facet height h_f times 2A_f inside the van Oosterom–Strackee atan2. Cite it
  with Werner 1994 and H&K 1996.
Holstein, H. "Gravimagnetic anomaly formulas for polyhedra of spatially linear media." *Geophysics*
68(1), 157–167. Crossref: online 2002-09-10, issue 2003. doi:10.1190/1.1543203 (verified). Not
supplied; Ren et al. 2018 cite its κ estimates of 4 and 5 for linear density.

### D'Urso 2013 [FULL, batch 2] and 2014 [PART: abstract, §6]
D'Urso, M. G. "On the evaluation of the gravity effects of polyhedral bodies and a consistent
treatment of related singularities." *Journal of Geodesy* 87(3), 239–252. Crossref: online
2012-09-26, issue 2013. doi:10.1007/s00190-012-0592-1 (verified).
D'Urso, M. G. "Analytical computation of gravity effects for polyhedral bodies." *Journal of
Geodesy* 88(1), 13–29. Crossref: online 2013-10-18, issue 2014. doi:10.1007/s00190-013-0664-x
(verified).
Both treat singularities, which matter for the draft's "on the surface" paragraph. I did not find
the brief's *Surveys in Geophysics* D'Urso paper, so no DOI is proposed for it.
- **2013**: singular terms included a priori through distribution theory (div(r/|r|³) as a Dirac
  measure weighted by the solid angle of the body's intersection with a small sphere, eqs 5–6,
  p. 241); U and g free of singularity at vertices, the second derivative not (§8). The appendix
  (eqs 108–112) merges arctangents for operation count, not rounding. No far-field analysis, no h·2A.
- **2014** (abstract, §6): formulas in the vertex coordinates of each face; no singularity unless
  the point is aligned with an edge, where the edge's contribution is set to zero "on the basis of
  some claims which still require a rigorous mathematical proof"; only the off-diagonal tensor
  entries are truly singular there. §6 validates on Tsoulis 2012's prism and on asteroids up to
  22,540 faces. No far-field rounding analysis. Werner 2017 (Table 1) finds D'Urso 2014's values
  agree with his to 10⁻¹⁴–10⁻¹⁵.

### Conway 2015 [PART: abstract, §4]
Conway, J. T. "Analytical solution from vector potentials for the gravitational field of a general
polyhedron." *Celestial Mechanics and Dynamical Astronomy* 121(1), 17–38. Crossref: online
2014-11-15, issue 2015. doi:10.1007/s10569-014-9588-x (verified). One expression per edge from a
gravitational vector potential, also valid inside (abstract). §4: agreement with Mader's prism
solution, both in Mathematica, "to (at least) 106 significant figures"; no floating-point rounding
analysis. **Relation:** a further formulation to cite; nothing on the numerator.

### Ren et al. 2018 [FULL, online-first copy] (batch 2); Ren et al. arXiv 2018 [ABS]
Ren, Z., Zhong, Y., Chen, C., Tang, J., et al. "Gravity Gradient Tensor of Arbitrary 3D Polyhedral
Bodies with up to Third-Order Polynomial Horizontal and Vertical Mass Contrasts." *Surveys in
Geophysics* 39, 901–935, 2018. doi:10.1007/s10712-018-9467-1 (verified). The copy read prints no
volume or pages. **§3.4 "Numerical Stability"** (PDF p. 18), eq. (25): η = ε(δ/α)^κ after H&K 1996
and Holstein 2003. **Table 4** (PDF p. 21, double precision, a tetrahedron against a point mass):
for constant density κ = 4.21 (T_xz, T_yz) and 3.23 (the rest), total loss at 1/γ_crit = 5195 and
70,079 body sizes; κ grows by ~1 per density order. Face term: per-edge arctan differences (App. B,
eqs B.3–B.4), no h·2A form. **Relation:** a measured far-field rounding law for the gradient tensor,
the draft's quantity, in the vertex class; a benchmark the draft's rules should be compared with.
Ren, Z. et al. "Exact gravity field for polyhedrons with polynomial density contrasts of arbitrary
orders." arXiv:1810.11768, 2018 [ABS]: singularity-free, with no far-field stability analysis.
**Relation:** low relevance to the stability question.

### Mirtich 1996 [FULL]
Mirtich, B. "Fast and Accurate Computation of Polyhedral Mass Properties." *Journal of Graphics
Tools* 1(2), 31–50, 1996. doi:10.1080/10867651.1996.10487458 (verified).
Centre of mass and moments and products of inertia of uniform polyhedra: the ten integrals up to
second order (eqs 18–27) by divergence theorem → projection onto a coordinate plane → Green's
theorem (§§4–7, eqs 28–98); Theorem 4 (eq. 77) is general in the powers, so higher moments follow
the same way. Its only rounding rule: project each face along its normal's largest component, so
|n_γ| ≥ 1/√3 (§5, p. 37). Accuracy: ≥ 15 significant figures on a cube and a tetrahedron (Table 1);
symmetric zeros below 10⁻¹⁵ on icospheres to 20,480 faces (Table 2). Moments are taken about the
frame origin and shifted (eqs 12–17), which loses ~(r/s)² far from the origin (my derivation): the
draft should integrate each piece about its cell centre, as Powell & Abel found essential. No solid
angle or triple product anywhere. **Relation:** standard prior
art for the exact mass, centroid and second moment of a polyhedral piece. The draft's piece moments
are not new as a computation, and the draft should cite Mirtich (and Powell & Abel, item 7).

---

## 4. Mascons

### Muller & Sjogren 1968 [FULL]
Muller, P. M., Sjogren, W. L. "Mascons: Lunar Mass Concentrations." *Science* 161(3842), 680–684,
1968. doi:10.1126/science.161.3842.680 (verified). The copy prints folios 680–683 (Crossref says
680–684). Lunar Orbiter V Doppler residuals show positive mass concentrations under the five ringed
maria; Imbrium ~20×10⁻⁶ lunar masses at an assumed 50 km depth. Historical citation only.

### Wittick & Russell 2017 [FULL]; Wittick & Russell 2019 [FULL] (batch 2)
"Mascon models for small body gravity fields", AAS 17-743 (2017): the paper prints only the AAS
number; no venue, date or DOI. Truth: a 10,152-facet Eros polyhedron. Table 2 (p. 12): uniform
single-layer mascons inside an 85 % shape, 884 → 12,199 mascons give RMS low-altitude errors
3.3×10⁻⁴ → 5.8×10⁻⁶ m/s² (surface acceleration ~5 mm/s², p. 15), so ~10⁻³ relative at best. No
scaling law stated.
Wittick, P. T., Russell, R. P. "Mixed-model gravity representations for small celestial bodies using
mascons and spherical harmonics." *Celestial Mechanics and Dynamical Astronomy* 131:31, 2019.
doi:10.1007/s10569-019-9904-6 (verified). Masses and SH coefficients fitted by two-step least squares
(§3, eqs 18–20). Table 3 (pp. 20–21): 874 → 12,296 infill mascons give 0.19 % → 1.3×10⁻² % low-
altitude RMS (a helper's fit: ∝ N⁻¹, i.e. h³ for fitted masses); the best hybrids reach 1.5×10⁻⁵
relative low and 5.5×10⁻⁷ high. **Relation:** fitted point masses stop two orders short of 10⁻⁷ near
the surface.

### Rathinam & Dempster 2019 [FULL] (missing from the brief; directly relevant)
Rathinam, A., Dempster, A. G. "Octree-Based Mascon Model for Small Body Gravity Fields." *Journal of
Guidance, Control, and Dynamics* 42(11), 2557–2567, 2019. doi:10.2514/1.g004008 (verified). An
AIAA Engineering Note; the copy read is the "Article in Advance" (published online 26 June 2019).
- The octree is built from the shape model's **vertices as a point cloud** (§II.A–B), uniform depth
  (Uni128, Uni256: leaves 1/128, 1/256 of the root) or variable depth (an occupied node is split to
  the target depth; empty interior nodes stay large). Interior leaves are found by slicing and
  alpha-shape polygons per layer (§II.C, MATLAB `alphaShape`).
- **Each leaf is one point mass at its centroid** (§III, eqs 2–3, 5, 8). The boundary leaves' density
  is scaled so the total volume matches the polyhedron (eq. 1); the centre of mass is then off "a few
  centimeters" and not corrected. A field point inside a leaf treats it as a uniform sphere of
  radius half the side (eqs 4, 6, 9).
- **Evaluation is a direct sum over every occupied leaf** (eqs 3, 5, 8). There are no node moments,
  no multipoles and no opening criterion: the octree only sizes the leaves.
- Results against the W&S polyhedron (§V, Tables 3–6): mean relative potential error on the surface
  10⁻³–10⁻⁴ (§V.A); Var256 (~10⁵ mascons) reaches about 1 % in acceleration on Itokawa's surface
  (§V.C); in the region around the body errors are "well below 1 %". Uni256 (1.3 M occupied nodes,
  Table 2) costs 2–3 times the polyhedron's time (Table 3). Error ∝ N^(−1/3), after W&S (§V.C).
- **Relation:** an adaptive-size mascon set, not a hierarchical evaluator. It confirms the first-order
  (0/1 fill, centre point) accuracy level at ~10⁶ cells, and leaves the draft's point (ii) of §(d)
  — moment-summed nodes with error-bound acceptance — without a precedent in this line of work.

### Fanti & Izzo, MasconCube (arXiv:2509.08607, 2025/26) [PART: HTML]
A regular 100³ grid of point masses (§2.2). Mascons with centres outside the mesh are removed, then
the masses are fitted. A "cubified" ground truth (cells inside the mesh at their centres) has a
relative acceleration error of 2.56×10⁻⁴ on Eros (Table 3), against 4.43×10⁻⁴ for the fitted
MasconCube. There is no resolution-scaling study. **Relation:** confirms that 0/1 cube filling sits
at the ~10⁻⁴ level at 100³ cells, roughly what the draft's first-order bound predicts. An arXiv
preprint, so no Crossref DOI.

### Mascon error numbers for Castalia (Werner & Scheeres 1997)
Now obtained; see item 3 (§3.2, pp. 336–339): ~3300 mascons at 59 m give 1–10 % attraction error
over most of the surface and 0.1–1 % away from it; 41 m (~9700) still has appreciable error. The draft
does not quote these numbers; they belong in its related work as the classic first-order mascon
result.

---

## 5. Cut cells and embedded boundaries

### Johansen & Colella 1998 [FULL]
Johansen, H., Colella, P. "A Cartesian Grid Embedded Boundary Method for Poisson's Equation on
Irregular Domains." *Journal of Computational Physics* 147(1), 60–85, 1998.
doi:10.1006/jcph.1998.5965 (verified). Two-dimensional, conservative differencing on cut-cell
volumes. Truncation error O(Δx²) in full cells, O(Δx/Λ) in cut cells (eq. 20, p. 69; Table 1
measures 0.98 and 2.0); treated as charges, the cut-cell share converges at O(Δx³) with a Dirichlet
boundary (a dipole layer) and O(Δx²) with Neumann (pp. 65–66, 70, 78; Table 2, 2.8–3.0). Right-hand
side at cell centroids, the cell average to O(Δx²) (p. 74). **Relation:** the same boundary-layer
counting argument as the draft's order table, in a PDE.

### Colella, Graves, Keen & Modiano 2006 [PART: OSTI PDF, geometry section]
Colella, P., Graves, D. T., Keen, B. J., Modiano, D. "A Cartesian grid embedded boundary method for
hyperbolic conservation laws." *Journal of Computational Physics* 211(1), 347–366, 2006.
doi:10.1016/j.jcp.2005.05.026 (verified). The per-cell geometry stored is the volume fraction κ, the
area fractions α, the face and EB-face centroids, and the mean EB normal (geometry list in §2).

### Schwartz, Barad, Colella & Ligocki 2006 [FULL]
"A Cartesian grid embedded boundary method for the heat equation and Poisson's equation in three
dimensions." *Journal of Computational Physics* 211(2), 531–550, 2006. doi:10.1016/j.jcp.2005.06.010
(verified). Volume fractions, apertures, centroids and normals are assumed accurate only to O(h²)
(§2, p. 533) and their computation is not given (§4.2, p. 547); small cells via κ(Δ_hφ) = κρ
(eq. 5), local error O(h/κ) (eqs 11–13) with a second-order solution. **Relation:** O(h²) geometry,
too coarse for the draft's orders 3–4; cite for the 3-D EB framework only.

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

### Aftosmis, Berger & Melton 1998 [PART: §§II–III] (batch 2)
"Robust and Efficient Cartesian Mesh Generation for Component-Based Geometry." *AIAA Journal* 36(6),
952–960, 1998. doi:10.2514/2.464 (verified). Every connectivity decision is the sign of a 4×4
determinant (6× a signed tetrahedron volume, eq. 2), filtered in floating point with Shewchuk's bound
(eq. 5, p. 955) and redone in adaptive exact arithmetic when uncertain (68 of 1.37 M on the
helicopter); exact zeros go to Simulation of Simplicity (pp. 955–956). §III.C covers only fast
triangle–cell intersection tests; **no cut-cell volume or area accuracy is reported**. **Relation:**
prior art for robust predicates in the clipper, not for moment accuracy.

### Müller, Kummer & Oberlack 2013 [PART: §§1–2, 5] (batch 2)
"Highly accurate surface and volume integration on implicit domains by means of moment-fitting."
*International Journal for Numerical Methods in Engineering* 96(8), 512–528, 2013.
doi:10.1002/nme.4569 (verified). Fixed nodes, weights from a linear system matching moments to degree
p, hierarchically line → surface → volume by the divergence theorem (§2). Measured orders exceed h^p
(abstract; §5.1 suggests h^(p+1) in 2-D). Table V (p. 523): ellipsoid volume error 1.5×10⁻³ (p = 0)
to 1.6×10⁻⁷ (p = 3). **Relation:** approximate moments on curved cells; the draft's polyhedral
pieces have exact ones.

### Saye 2015 [PART: abstract, §§1, 3.4, 4] (batch 2)
Saye, R. I. "High-Order Quadrature Methods for Implicitly Defined Surfaces and Volumes in
Hyperrectangles." *SIAM Journal on Scientific Computing* 37(2), A993–A1019, 2015.
doi:10.1137/140966290 (verified). Strictly positive weights, nodes inside the domain, order ≈ 2q
for a q-point Gauss scheme (§1, p. A994); Table 1 (p. A1010) measures orders 2.0 to 20.0 for q = 1…10,
shown in quad-double arithmetic because double precision hides rates above ~10 (§4, p. A1009).
Implicit geometry, not polyhedral: related, not prior art for exactness.

---

## 6. Tree gravity

### Barnes & Hut 1986 [FULL, scan]
"A hierarchical O(N log N) force-calculation algorithm." *Nature* 324(6096), 446–449, 1986.
doi:10.1038/324446a0 (verified; the bib entry lacks issue 6096, which is fine). Opening rule l/D < θ,
D from the cell's centre of mass, "θ … ~1" (p. 447); "even with … θ = 1 … still accurate to ~1%";
the dipole vanishes about the centroid, so the leading error is the quadrupole (p. 448); a rigorous
bound is promised for "a more detailed paper".

### Greengard & Rokhlin 1987 [FULL] (batch 2)
"A fast algorithm for particle simulations." *Journal of Computational Physics* 73(2), 325–348,
1987. doi:10.1016/0021-9991(87)90140-9 (verified; the first page prints 325–348). Theorem 2.1
(p. 328, eqs 2.4–2.6): for the 2-D log kernel a p-term expansion errs by at most (A/(c−1))(1/c)^p,
A(1/2)^p at c ≥ 2; translation Lemmas 2.3–2.5. **Relation:** the a-priori truncation-bound template,
geometric in the separation; 2-D, not the 3-D 1/r kernel.

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

### Salmon & Warren 1994 [FULL, scan read as page images]; Warren & Salmon 1993 [PART]
Salmon, J. K., Warren, M. S. "Skeletons from the Treecode Closet." *Journal of Computational
Physics* 111(1), 136–155, 1994. doi:10.1006/jcph.1994.1050 (verified). Received 1992, revised 1993.
- Abstract and App. A: the BH MAC "can introduce potentially unbounded errors unless θ < 1/√3".
- **The bound, eq. (9)** (p. 142) = **eq. (62)** (p. 154): Δa_(p)(r) ≤ (1/d²)·1/(1 − b_max/d)² ·
  ((p+2)⌈B_(p+1)⌉/d^(p+1) − (p+1)⌊B_(p+2)⌋/d^(p+2)), with B_(n) = ∫|ρ||x − r₀|ⁿ (eq. 61) and upper
  and lower bounds on B from eqs (64)–(68). Eq. (60) is the same with exact B. Absolute and
  Relative MACs: eqs (10), (11).
- **Tightness they report** (Fig. 7 caption, p. 143): for the 32k-body cosmology model, Sum Squares
  MAC, monopole only (p = 1), "the mean ratio [actual/bound] in this sample is 0.22. The rms ratio is
  0.28." Table I (p. 143): BH θ = 0.65 maximum relative errors 0.0135–0.0701; Sum Squares f = 0.01,
  0.0045–0.0057.
- **The draft's use (line 694) stands:** "Salmon & Warren's bound … with exact absolute moments" is
  eq. (60)/(62) with exact B_(n), which is what `s5-world-octree/mac_bound.py` evaluates. The draft's
  109–4800× is Cavendish's measurement on a uniform cube, not a number from the paper. For fairness
  the draft could add that S&W measure the bound as tight (mean actual/bound 0.22) on clumpy particle
  sets, where the first omitted multipole is not zero; the cube's looseness comes from its vanishing
  low multipoles, as the draft says.
Also read before (summary of the same bound): Warren & Salmon, "A Parallel Hashed Oct-Tree N-Body
Algorithm" (Supercomputing '93), §2.2:
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

### Catmull 1978 [FULL] (batch 2)
"A hidden-surface algorithm with anti-aliasing." *Proceedings of the 5th Annual Conference on
Computer Graphics and Interactive Techniques (SIGGRAPH '78)*, 6–11, 1978.
doi:10.1145/800248.807360 (verified; the copy prints only folios 6–11). Box-filter "area sampling"
(p. 6); polygons clipped per pixel by a Sutherland–Hodgman variant (p. 7) and colour weighted by
visible area (pp. 9–10). Zeroth moment only. The 2-D order-0 analogue of exact fill.

### Duff 1989 [FULL, 2024 retyped copy] (batch 2)
Duff, T. "Polygon scan conversion by exact convolution." October 1989; the copy read is a retyped
version with a note of August 2024, paginated 1–15, which **does not print the venue**. The citation
*Raster Imaging and Digital Typography*, CUP, 1989, pp. 154–168 (from a patent) remains unchecked;
there is no DOI. §3 (pp. 4–6): a piecewise-polynomial kernel integrated exactly over the polygon
clipped to each kernel piece by signed trapezoids under the edges (eqs 4–6): "Since h_ij is a
polynomial, so are the definite integrals above." §6: linear shading exact too. The 2-D analogue of
the draft's piece moments.

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

### Hockney & Eastwood [front matter only] (batch 2)
*Computer Simulation Using Particles.* Crossref holds the CRC reissue, doi:10.1201/9780367806934
(issued 2021-03-24); the original is 1981 (McGraw-Hill, as Barnes & Hut cite it), with a 1988
edition from Adam Hilger. The owner's copy is 8 pages: the IOP Publishing (Bristol and Philadelphia)
paperback's title page and contents ("Preface to the Paperback Edition"), with no year or ISBN. Cloud-in-cell and
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

Updated 28 Nov 2026. Everything this list asked for in the first pass has now been supplied and read
(see each entry's status), except:
- Holstein 2003, "Gravimagnetic anomaly formulas for polyhedra of spatially linear media",
  10.1190/1.1543203 (optional; the κ source Ren et al. 2018 cite).
- Strakhov et al. 1986 and Strakhov & Lapina 1990 (cited by Werner 2017 §13.3 for a gore half-angle
  form with an h·length numerator); no DOIs found, Russian-language journals.
- Duff 1989's printed venue and pages (the copy read is a 2024 retyping without them).
These are carried in the repository's `papers-to-fetch.md`.

## (d) What is new and what is already in the literature

- **Far-field rounding classes are not new.** Holstein & Ketteridge 1996 analyse them in full
  (Tables 2–5: absolute error O(αγ^κ ε), relative O(γ^(κ−2)ε) on an inclined track and O(γ^(κ−3)ε)
  on a horizontal one, measured slopes 2.00/3.02/4.01) and build a composite algorithm whose error
  converges with distance. Fukushima 2020 measures cubic growth outside the Brillouin sphere for the
  prism (§3.2, Fig. 2). The draft's contribution is narrower: a specific stabilisation of the
  Werner–Scheeres solid-angle form. It should be framed as that and should cite both.
- **The h_f·2A_f numerator: the identity is published; its use against rounding in the
  van Oosterom–Strackee form is not, in anything read.** (27 Nov 2026, after reading all the
  polyhedral sources.)
  - **Werner 1994, eq. (3.4), p. 261** writes the numerator of each spherical vertex angle as
    Δz·[η₁(ξ₃−ξ₂)+η₂(ξ₁−ξ₃)+η₃(ξ₂−ξ₁)] and says the bracket "is twice the area of the planar
    triangle" and "invariant for a rigid face": height × twice the area, from face-fixed coordinates.
    His reason is precomputation, and the form is the n-arctangent spherical excess.
  - **Werner & Scheeres 1997, §2.5.4, p. 332**, use r₁·n_f = r₁·(r₂×r₃) with
    n_f = (r₂−r₁)×(r₃−r₂) (so |n_f| = 2A_f and r₁·n̂_f = Δz), but only for sign(Δz); their eq. (27)
    then writes the triple product.
  - **Werner 2017, eq. (31)**, has the gore form (height × edge × in-plane altitude), for signs; its
    eq. (37) keeps the box product; and §16 says "The numerical or computational properties of this
    paper's solid-angle formulas have not been analyzed."
  - **Holstein & Ketteridge 1996** have the principle but not this step: eq. (22) uses the facet area
    2A_i as "a facet property that can be evaluated accurately without reference to the observation
    point", and p. 361 asks for O(α) quantities "rather than the difference of two O(δ) quantities".
    They never use the solid-angle form.
  - **Holstein 2002 (batch 2) is the closest published use against rounding**: App. B, eq. (B-2),
    "The summand h_ij L_ij is twice the area of the patch ij", h_ij L_ij = n_i·(r₁_ij × r₂_ij), summed
    to 2A_i, which his far-field surface method uses so that "analytical cancellation" happens before
    evaluation (eqs 42–43, p. 1131). His h_ij is the in-plane edge distance and the identity is per
    edge, inside his own surface method, not h_f·2A_f inside the van Oosterom–Strackee atan2.
  - Okabe 1979 (eqs 33–35), Petrović 1996, Tsoulis & Petrović 2001, Tsoulis 2012, D'Urso 2013 and
    2014, Conway 2015, Ren et al. 2018 and Li & Chouteau 1998 have nothing like it; Ren (§3.4,
    Table 4) and Li & Chouteau (§5, Table I) do measure vertex-class far-field breakdown.
  - **So the draft must not present N = h_f·2A_f as new.** It should cite Werner 1994 eq. (3.4) for
    the identity and say what is added: the observation that the van Oosterom–Strackee triple product,
    evaluated as written (W&S eq. 27), has absolute error εR³ and puts the face term in H&K's vertex
    class, and that the face-fixed h_f·2A_f (Werner 1994's bracket, H&K's facet-area principle,
    Holstein 2002's per-edge use of the same identity) keeps it in the line class, measured and
    machine-checked.
- **The edge-log rule is Holstein & Ketteridge's.** Their eq. (24), c = 2h·artanh[(ℓ₂−ℓ₁)/(r₁+r₂)],
  is the draft's L_e = 2·artanh(ℓ/(a+b)), and p. 361 warns that ½ln((1+t)/(1−t)) "is subject to large
  rounding error when t ≈ γ ≪ 1". The draft's log1p(2ℓ/(a+b−ℓ)) is that accurate artanh; cite H&K for
  it, not only for the near-edge form. W&S eq. (7) and Werner 1994 eq. (3.3) use the ratio form.
- **The near-edge conjugate form is known.** H&K p. 362 ("Replacing differences of the form r − |ℓ|
  by r₀²/(r + |ℓ|)", eqs 27–28) and Okabe eq. (45) (the conjugate log for the gradient tensor, to
  avoid a singularity). Fukushima 2020 eq. (24) uses the same move for the prism.
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
  & Dempster 2019, now read, do not pre-empt (ii): their octree only sizes the leaves, each leaf is a
  point mass at its centroid, and the field is a direct sum over all leaves (eqs 2–9), with no node
  moments or acceptance test. Werner & Scheeres's Castalia mascons (§3.2) and Rathinam & Dempster's
  N^(−1/3) scaling are the published first-order baseline for (i).
- **Accuracy context for the introduction:** typical tree codes run at 10⁻³–10⁻² (GADGET-2 Fig. 1),
  tesseroid codes at 10⁻³ (Uieda 2016, Soler 2019), and a 100³ cubified mascon model at about
  2.6×10⁻⁴ (MasconCube Table 3), Castalia's 3300 mascons at 1–10 % on the surface (W&S 1997 §3.2)
  and octree mascons at ~1 % (Rathinam & Dempster §V.C). The draft's ~10⁻⁷ is the regime only Dehnen
  2014 (FMM, 10⁻⁷) and Warren & Salmon ("down to machine accuracy") address.
