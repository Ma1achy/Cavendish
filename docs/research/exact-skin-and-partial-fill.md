# An exact skin, or mass anti-aliasing: what boundary voxels should carry

Research note, 25 Nov 2026, for the owner's question ("for fun, and for the optimisations it might
give") and for S5 (the bounded walk, instrument LOD, analytic bodies) and any later revision of S3's
voxeliser. Method and numbers: [`exact-skin-and-partial-fill/`](exact-skin-and-partial-fill/)
(`skin_bench.py`, `tables.py`, `results/*.json`, `results/tables.md`).

## Changed after reading the primary sources (25 Nov 2026)

The owner supplied the full texts of Okabe 1979, Tsoulis & Petrović 2001, Tsoulis 2012, Holstein &
Ketteridge 1996 and Werner & Scheeres 1997 (read as PDFs, not stored in the repo; links in §1.1).
- **The benchmark's face form is Werner & Scheeres' eqs. (10), (15), (16), split face by face.** Checked
  term by term against the paper (§1.1); signs and the solid-angle formula (their eq. 27) agree. The
  "could not check against the text" caveat is gone.
- **Far-field rounding was mis-measured.** The self-test put a dyadic cube at the origin and the point
  at integer coordinates, so most products were exact. At general positions the triple product
  r₁·(r₂×r₃) of the solid angle loses ε(R/s)³, not ε(R/s)²: at R/s = 3000, 4×10⁻⁸ in U and 1×10⁻⁷ in g,
  not 1×10⁻⁹ and 6×10⁻⁹. Writing the numerator as h_f·|(v₂−v₁)×(v₃−v₁)| restores ε(R/s)², which is
  Holstein & Ketteridge's "line method" class. New measurement in §1.1, and a new rule for the kernel.
- **Holstein & Ketteridge's error analysis is now quoted from the text** (orders of error in γ = s/R,
  breakdown distances, their composite surface/line/vertex algorithm), not from the abstract.
- **Singular cases restated from Tsoulis & Petrović.** Their correction terms belong to the
  Petrović/Tsoulis line-integral form, where the *projection* of the point onto a face's plane falling
  inside the face, on an edge or on a vertex is singular. That happens for some face at almost every
  point. The solid-angle form used here does not need them; it has only three degenerate places (point
  on a face, on an edge, at a vertex). U and g are continuous everywhere; the tensor jumps across faces
  and is log-singular on edges.
- **The ESA polyhedral-gravity-model is named as the external oracle** (Tsoulis 2012 formulation, its
  1e-14 tolerance read from the source, GPL-3.0, so used as a tool to make fixtures, never linked).
- **Okabe's prism special case and Tsoulis 2012's boundary-point test table** replace "(abstracts)" in
  §6. Okabe's own log-cancellation fix (his eq. 45) is recorded.
- **The precision floor in §3 is higher than stated** for the D = 10 m column: the frac_cen_q numbers
  there are floor-dominated, as the note already suspected; the 1.5 m table and every recommendation
  stand.

## Summary (one page)

**What Cavendish does today.** A leaf stores `count`, how many of the `k³` sample points of its cell
lie inside the shape (`k` = 4 by default), and its mass is `count × 8^level × unit` with
`unit = M / Σ samples`, so the body's total mass is exact
([`voxeliser.rs`](../../crates/cav-core/src/voxel/voxeliser.rs), [`tree.rs`](../../crates/cav-core/src/voxel/tree.rs)
`leaf_mass`, [S3 §2](../slices/S3.md#the-tree-cav_corevoxel)). The read takes every leaf as a full
uniform cube of side h at the cell's centre ([`kernel.rs`](../../crates/cav-core/src/voxel/kernel.rs)
`TreeBody::new`). The viewer's `Fill` field is `mass / full`, the same count over `k³`
([`voxel_view.rs`](../../crates/cav-sim/src/voxel_view.rs) line 599). So Cavendish already does
"mass anti-aliasing", but the fraction is *sampled*: it is a 0/1 voxelisation at pitch h/k, lumped
into cells. It keeps no centroid and no shape of the partial cell.

**What was measured.** I built a benchmark (Python and numpy, about 400 lines; every number below
comes from it). It uses five convex polyhedra of about 1 m (a cube aligned with the lattice, the same
cube turned, an icosahedron, a 1280-face icosphere for the sphere, and a 1 m × 1 m × 5 cm plate) at
ρ = 2700 kg/m³. Each body is laid on lattices of pitch 1/8 to 1/64 m, at four random sub-cell
placements. Every boundary cell is clipped exactly into its inside piece. The error of each boundary
representation is a sum over boundary cells only, against the exact polyhedral field. The reading is
the AION-10 gradiometer, φ = k_eff T² [g_z(z + 5 m) − g_z(z)], with k_eff T² = 4.80×10⁹ rad per
m s⁻². It is taken at 1.5 m and 10 m from the body's centre, in three directions.

**What it found** (worst body; full tables in §3):

| boundary cell carries | order in h | phase error at h = 1/64 m, 1.5 m away (signal ≈ 200 rad) |
|---|---|---|
| 0/1 (centre inside), mass renormalised | **1** on faces aligned with the lattice; 1.8–2.8 on turned or curved ones | 0.9 rad |
| fill from 4³ samples, at the centre (**today**) | **1** aligned; 2.2–2.6 turned | 0.12 rad |
| exact fill, at the centre | **2** aligned; 3–4 turned | 5×10⁻³ rad |
| exact fill, at the piece's centroid | **3** | 4×10⁻⁵ rad |
| exact fill + centroid + the piece's second moment | **≥ 3.4** (3.4–4.9) | 4×10⁻⁷ rad |
| the exact piece (polyhedron) | exact (checked to 10⁻¹⁴) | ≈ 10⁻¹¹ rad, rounding |

1. **The brief's expectation holds where it matters most, on faces aligned with the lattice.** Walls,
   floors and boxes are common in a voxel world, and there 0/1 is first order, a fill fraction second
   order, and fill plus centroid third order. On *turned* faces everything converges faster than
   expected (0/1 at about h^2.5), because the cell-by-cell errors take every lattice phase and cancel.
   The worst case is the aligned one, and it is the one to design for.
2. **Today's sampled fill is still first order** where faces are aligned (0.12 rad at 1/64 m). It is
   about k = 4 times better than 0/1, not a higher order. On a 200 rad signal it is ~6×10⁻⁴ relative.
   That is far above the 10⁻⁴ rad/√Hz shot noise per shot and above every µrad-scale signal.
3. **The centroid is the cheap, robust win: three more numbers per boundary leaf.** Exact fill alone
   reaches order 3–4 on turned faces: its error is a double layer of dipoles, which is uniform on a
   closed surface and has no field outside (§2.3). Measured at the piece's centroid instead, it is
   *worse* there (order 3), but it is order 3 everywhere. Adding the piece's own second moment (six
   numbers) gives order ≥ 3.4 everywhere and 4×10⁻⁹–4×10⁻⁷ rad at 1/64 m.
4. **An exact polyhedral skin is affordable, but almost never needed.** One triangle costs about 55 ns
   per point (measured, one thread), against 8.5 ns for a cube's series. A clipped cell has 7–14
   triangles, so about 0.5–0.8 µs. Past a few cells from the point, a piece's moments already give
   it to rounding, so the exact piece is a *near* kernel, like the prism in S3's kernel. The
   whole body as one polyhedron (option a) costs F × 55 ns per point. That is cheap for meshes of
   10²–10⁴ faces, and it is exact: it is **the oracle** for every voxelised body made from a mesh.
   In f64 it is good to ~ε(R/s)² relative (10⁻¹⁰ at R/s = 1000) only if the solid angle's numerator
   is formed from the face's own edge vectors (§1.1); the textbook triple product gives ε(R/s)³.

**Recommendation** (§5): (1) make the polyhedral closed form (Werner & Scheeres' form, with the
robustness rules of §1.1) the oracle for mesh-made and analytic polyhedral bodies now, and check it
once against ESA's independent implementation; (2) replace the sampled `count` by **exact piece moments**: the
voxeliser clips each boundary cell exactly, and the tree's leaves and nodes hold the pieces' mass,
centroid and second (and third) moments, so the walk's node multipoles are the true body's;
(3) use the exact piece as a near-field leaf kernel only within a few cells of an arm, and for analytic
bodies with few faces use the whole polyhedron outright. Measured gain over today's sampled fill
at the same pitch (1/64 m, 1.5 m away): with the centroid, 10⁴× on the aligned cube (0.12 →
1.0×10⁻⁵ rad) and 4–1000× on the turned bodies. With second moments as well, 300–7000× on turned
bodies and 3×10⁷× on the aligned cube.

---

## 1. Methods compared

**Notation.** Pitch h. A boundary cell is one the surface crosses. Its *piece* is the part of the cell
inside the body (a polyhedron), of volume f h³ (f the fill), centroid c̄ and central second moment
I = ∫(x − c̄)(x − c̄)ᵀ ρ dV.

| | boundary cell's mass | where | shape | data per boundary leaf | cost per (leaf, point) |
|---|---|---|---|---|---|
| (i) 0/1 | ρh³ if its centre is inside | centre | cube h | none (a bit) | cube kernel, 8.5 ns |
| (ii-s) sampled fill (today) | ρh³·count/k³, renormalised | centre | cube h | 1 byte | cube kernel |
| (ii) exact fill | ρ f h³ | centre | cube h | f | cube kernel |
| (iii) fill + centroid | ρ f h³ | c̄ | cube h, or a point | f, c̄ (4 numbers) | cube kernel or point mass |
| (iii+) + second moment | ρ f h³ | c̄ | point + quadrupole I | f, c̄, I (10 numbers) | ≈ 2× point mass |
| (iv) exact piece | — | — | the clipped polyhedron | its faces (or recompute) | 7–14 triangles × 55 ns |
| (a) whole body as one polyhedron | — | — | the mesh | the mesh | F triangles × 55 ns |
| (b) interior voxels + clipped boundary cells | = (iv) | | | | |
| (c) interior voxels + a thin polyhedral shell | the region between the mesh and the voxel hull, as a polyhedron (signed) | | | the shell's faces | (mesh faces + hull faces) × 55 ns |

Option (c) is the same mass as (b), but its shell has the mesh's faces plus the voxel hull's
faces. The hull's staircase faces number about as many as the boundary cells, so (c) costs about as
much as (b) and is harder to build. It is not pursued.

### 1.1 The polyhedral closed form

For a uniform polyhedron of density ρ, the potential U = Gρ∫dV/r and attraction g = ∇U at a point are
finite sums over faces and edges. Each edge carries a logarithm and each face a solid angle (an
arctangent). The sources, all read in full (PDFs supplied by the owner, not kept in the repo):
- **Okabe, M. (1979).** "Analytical expressions for gravity anomalies due to homogeneous polyhedral
  bodies and translations into magnetic anomalies." *Geophysics* 44(4), 730–741.
  [doi:10.1190/1.1440973](https://doi.org/10.1190/1.1440973),
  [SEG page](https://pubs.geoscienceworld.org/seg/geophysics/article-abstract/44/4/730/68303/Analytical-expressions-for-gravity-anomalies-due).
  First and second derivatives by the divergence theorem applied twice, in a rotated frame per facet
  and per edge (eqs. 15–36, 37–45); one arctangent per facet *vertex* (eq. 30); valid for internal
  and external points, with "essential singularities" for points on a facet (p. 734, p. 736); the
  rectangular prism as a special case (eqs. 46–56; eq. 48 "is identical to … Nagy (1966)", p. 737).
  Note his frame has z downward and facet vertices numbered clockwise in the facet frame (p. 733).
- **Werner, R. A. & Scheeres, D. J. (1997).** "Exterior gravitation of a polyhedron derived and
  compared with harmonic and mascon gravitation representations of asteroid 4769 Castalia."
  *Celestial Mechanics and Dynamical Astronomy* 65, 313–344 (received 1996, published 1997).
  [doi:10.1007/BF00053511](https://doi.org/10.1007/BF00053511),
  [ADS PDF](https://adsabs.harvard.edu/pdf/1997cemda..65..313w). The edge-dyad and face-dyad form,
  one logarithm per edge and one arctangent per face (summary §2.6, p. 333–334).
- **Tsoulis, D. & Petrović, S. (2001).** "On the singularities of the gravity field of a homogeneous
  polyhedral body." *Geophysics* 66(2), 535–539.
  [doi:10.1190/1.1444944](https://doi.org/10.1190/1.1444944),
  [open copy](https://gfzpublic.gfz.de/pubman/faces/ViewItemFullPage.jsp?itemId=item_238635_1).
- **Tsoulis, D. (2012).** "Analytical computation of the full gravity tensor of a homogeneous
  arbitrarily shaped polyhedral source using line integrals." *Geophysics* 77(2), F1–F11.
  [doi:10.1190/geo2010-0334.1](https://doi.org/10.1190/geo2010-0334.1),
  [SEG page](https://pubs.geoscienceworld.org/seg/geophysics/article-abstract/77/2/F1/275123/Analytical-computation-of-the-full-gravity-tensor).
  The Petrović line-integral form with the 2001 singular terms, and the FORTRAN program
  `polyhedron.f` that ESA's library descends from.
- **Holstein, H. & Ketteridge, B. (1996).** "Gravimetric analysis of uniform polyhedra."
  *Geophysics* 61(2), 357–364. [doi:10.1190/1.1443964](https://doi.org/10.1190/1.1443964),
  [SEG page](https://pubs.geoscienceworld.org/seg/geophysics/article-abstract/61/2/357/72863/Gravimetric-analysis-of-uniform-polyhedra).
  The rounding-error analysis used below.
- [D'Urso 2013](https://doi.org/10.1007/s00190-012-0592-1) and
  [D'Urso 2014](https://doi.org/10.1007/s00190-013-0664-x): treatments of the singularities. Still not
  read; see §7.

**Werner & Scheeres' result** (r points from the field point to the body, Fig. 1, p. 316; U positive;
g = ∇U points at the mass):

```text
U    = ½Gρ Σ_edges r_e·E_e·r_e L_e − ½Gρ Σ_faces r_f·F_f·r_f ω_f                   (10)
∇U   = −Gρ Σ_edges E_e·r_e L_e   + Gρ Σ_faces F_f·r_f ω_f                          (15)
∇∇U  =  Gρ Σ_edges E_e L_e       − Gρ Σ_faces F_f ω_f                              (16)
∇²U  = −Gρ Σ_faces ω_f   (0 outside, −4πGρ inside)                                  (17)
E_e = n_A n_e^A + n_B n_e^B (the two faces' normals times their in-plane edge normals), F_f = n_f n_f
L_e = ln((a+b+ℓ)/(a+b−ℓ))                                                          (7)
ω_f = 2 atan2(r₁·r₂×r₃, r₁r₂r₃ + r₁(r₂·r₃) + r₂(r₃·r₁) + r₃(r₁·r₂))                  (27)
```

Splitting each edge dyad between its two faces gives r_e·E_e·r_e L_e = Σ over the two faces of
h_f (n_e^f·r_e) L_e, and r_f·F_f·r_f = h_f². So the benchmark's face-by-face form is exactly (10) and
(15), and the face's tensor contribution is Gρ[n_f (Σ_e n_e^f L_e)ᵀ − n_f n_fᵀ ω_f], symmetric once
summed over faces:

```text
for each face f (unit outward normal n, any vertex offset r = v − p from the point p):
  h_f   = n·r
  S_f   = Σ_edges e of f  (n_e·r_e) · L_e,   L_e = log1p(2ℓ/(a+b−ℓ))
  ω_f   = 2·atan2(h_f·|(v₂−v₁)×(v₃−v₁)|, r₁r₂r₃ + r₁ r₂·r₃ + r₂ r₃·r₁ + r₃ r₁·r₂)
  U    += ½ ρ h_f (S_f − h_f ω_f)
  g    += −ρ n (S_f − h_f ω_f)
```

Here n_e is the edge's outward normal in the face's plane, a and b are the distances to the edge's
ends, and ℓ is its length. The numerator h_f·|(v₂−v₁)×(v₃−v₁)| equals r₁·(r₂×r₃) for an anticlockwise
triangle (W&S derive the same identity for the sign, eq. 26, p. 332) but is computed from the face's
own edge vectors, which is what keeps far-field rounding at ε(R/s)² (below). `skin_bench.py` still uses
the triple product; that does not change any result in §3 at 1.5 m (R/s ≲ 130 there).

Checked numerically (`skin_bench.py selftest`):
- **Against Nagy's prism.** A cube matches the prism to 1×10⁻¹⁵ relative near and 1×10⁻¹² at
  10 sides ([Nagy 1966](https://doi.org/10.1190/1.1439779); the repo's `kernel.rs` form).
- **Against the cube's series.** It matches the monopole + l = 4 series to 8×10⁻¹² at 157 sides.
- **Against a numerical gradient.** g matches a central difference of U to 2×10⁻¹⁰, the
  difference's own error.
- **Interior cubes + clipped pieces = the whole body:** to 2–8×10⁻¹⁵ in U and 1×10⁻¹⁴–4×10⁻¹⁴ in
  g, for all four bodies at h = 1/8 m. The volumes agree to 10⁻¹⁵.

**Numerical issues, from the sources and measured.**
- *Cancellation far away: what Holstein & Ketteridge show.* With γ = s/R (body or facet size over
  distance), the anomaly is O(sγ²), each face integral O(sγ), each edge term O(s), and each vertex
  primitive O(sγ⁻¹) (p. 357, Table 2 p. 360). The sum is computed to ε times its largest term, so the
  absolute error is O(sγᵏε) with κ = 1, 0, −1 for their surface, line and vertex methods, and the
  relative error is O(γ^(κ−2)ε) for a component along the line of sight ("inclined track", Table 4)
  and O(γ^(κ−3)ε) for a foreshortened one ("horizontal track", Table 3). Their measured breakdown in
  f64 for the vertex method (the classic closed form, Okabe's included) is at γ = 10^−3.5 with an
  error slope of exactly 4 (Table 5, p. 363; effective ε 10⁻¹⁴). Their remedy (p. 361, Fig. 3) is a
  decision tree that falls back to numerical quadrature of the edge integrals (four-point
  Gauss–Lobatto) and to a facet-area form (eqs. 20–23) as γ falls, which runs in f32 with errors that
  shrink with distance.
- *Where the Werner–Scheeres form sits.* Its edge terms (n_e·r_e)L_e are O(R)·O(ℓ/R) = O(s): the
  line-method class, κ = 0, **provided** L_e = log1p(2ℓ/(a+b−ℓ)) (no subtraction of near-equal
  numbers far away) and the solid angle's numerator is h_f·2A_f. Formed as r₁·(r₂×r₃) from vectors of
  length R, that numerator carries an absolute error εR³ and the face term becomes vertex-class.
  Measured on a 1/64 m cube with its centre at a non-dyadic point, points along a skew direction,
  reference the monopole + l = 4 series (pure Python, f64,
  [`exact-skin-and-partial-fill/farfield.py`](exact-skin-and-partial-fill/farfield.py)):

  | R/s | triple product (as `skin_bench.py`): U, g | h_f·2A_f numerator: U, g | h_f·2A_f, naive ln ratio: U, g |
  |---|---|---|---|
  | 10² | 7×10⁻¹³, 2×10⁻¹¹ | 3×10⁻¹⁴, 2×10⁻¹² | 1×10⁻¹⁰, 2×10⁻¹⁰ |
  | 10³ | 4×10⁻⁹, 9×10⁻⁹ | 1×10⁻¹⁰, 3×10⁻¹⁰ | 6×10⁻⁸, 1×10⁻⁷ |
  | 3×10³ | 4×10⁻⁸, 1×10⁻⁷ | 2×10⁻⁹, 4×10⁻⁹ | 3×10⁻⁶, 7×10⁻⁶ |
  | 10⁴ | 6×10⁻⁷, 2×10⁻⁵ | 1×10⁻⁸, 2×10⁻⁸ | 4×10⁻⁵, 8×10⁻⁵ |
  | 10⁵ | 5×10⁻³, 1×10⁻² | 1×10⁻⁶, 4×10⁻⁶ | 3×10⁻², 1×10⁻¹ |

  The middle column follows ε(R/s)² (Table 4's line method); the left one steepens to ε(R/s)³ and
  worse. The earlier "ε(R/s)²/10" came from the self-test's dyadic cube at the origin and integer
  field points, where most products are exact; the same script with that set-up reproduces it
  (2×10⁻¹⁰ and 7×10⁻⁹ at R/s = 3000). For Cavendish this means the closed form belongs near and the
  moments far, the same rule as S3's prism and series (`PRISM_WITHIN = 3`). In f32 (ε = 6×10⁻⁸) even
  the line-class form is near 10⁻⁵ relative by R/s = 10, so on the GPU it is a near kernel only, and
  any far use needs f64 or float-float (derived from the scaling, not measured in f32).
- *Points near an edge.* L_e is infinite only on the edge segment itself (a + b = ℓ); on the edge's
  extension a + b − ℓ = 2 min(a, b). Close to the segment, a + b − ℓ is a difference of near-equal
  numbers. Holstein & Ketteridge replace r − |ℓ| by r₀²/(r + |ℓ|) (r₀ the distance to the edge's line,
  p. 362, eqs. 27–28); Okabe makes the same move for negative ξ (eq. 45, p. 736). Applied here: when
  the point projects onto the segment (ℓ₁ ≤ 0 ≤ ℓ₂ along the edge), a + b − ℓ = r₀²/(a − ℓ₁) + r₀²/(b +
  ℓ₂) with r₀ = |r₁ × ê|. Measured on a 1/64 m edge at distance d from its middle: the direct form is
  wrong by 5×10⁻¹² at d = 10⁻⁶ m, 3×10⁻⁶ at 10⁻⁸ m and divides by zero at 10⁻¹⁰ m; the r₀² form stays
  below 6×10⁻¹¹ ([`exact-skin-and-partial-fill/nearedge.py`](exact-skin-and-partial-fill/nearedge.py))
  down to d = 10⁻⁸ m. (Corrected 25 Nov 2026, for the paper in `docs/papers/polyhedral-gravity/`:
  below that the same script gives 7×10⁻⁹ at 10⁻¹⁰ m and 2×10⁻⁵ at 10⁻¹⁴ m. That is the floor set by
  rounding the point's own position, ε|p|/(d·L), which no formula removes, not a fault of the r₀²
  form.) In U and g the log is multiplied by (n_e·r_e) → 0, so
  the product tends to 0; H&K omit the term once r₀/L is too small for r₀² to register (eq. 30).
- *Points on a face, an edge or a vertex.* Tsoulis & Petrović (p. 539) prove that U and g are defined
  and continuous everywhere, on the surface included; the second derivatives jump across a face (the
  "Poisson jump", Laplacian −4πGρ inside, 0 outside) and are log-singular on edges. Their correction
  terms (−2πh_p, −πh_p, −ϑh_p for the potential's surface integral, eqs. 24–25; −2π, −π, −ϑ times
  cos(N_p, e_j)σ_p for the derivatives, eqs. 28–29) are needed in the Petrović/Tsoulis form, where the
  per-edge arctangents AN_pq are singular whenever the point's *projection* onto a face's plane falls
  inside the face, on an edge or on a vertex. Tsoulis 2012 stresses that for any point "there is
  bound to exist at least one polyhedral face" whose projection falls inside (p. F7), so those terms
  are routine there, not edge cases. The solid-angle form does not need them: atan2 of eq. (27) is
  continuous wherever the point is off the face's plane, and Holstein & Ketteridge's form likewise
  avoids "discriminat[ing] between projected observation points that lie inside, on, or outside a
  target facet boundary" (abstract, p. 357; p. 359). What remains degenerate in the solid-angle form:
  a point *in* a face's plane (ω_f = 0 outside the face, ±2π inside, but always multiplied by h_f = 0
  in U and g; in the tensor it is the Poisson jump, so return the one-sided limit or flag it), on an
  edge (above), or at a vertex (r_i = 0: drop that face's and edges' terms, all of which carry a
  vanishing factor in U and g). Werner & Scheeres do not treat surface points at all ("We suspect the
  violation is benign", p. 317).
- *Tolerances in practice.* ESA's [polyhedral-gravity-model](https://github.com/esa/polyhedral-gravity-model)
  makes its singular-case decisions with one absolute constant, `EPSILON_ZERO_OFFSET = 1e-14` in mesh
  units (`src/polyhedralGravity/util/UtilityFloatArithmetic.h`, read via the GitHub API), and its
  repository mentions no compensated summation. Tsoulis 2012 notes that for points on the
  *extensions* of edges some mixed second derivatives come out "finite but not reliable" (p. F8).
- Arm points sit in vacuum tubes, away from any body's face, so the singular cases matter only for the
  oracle's own tests. They should still be handled, and tested on a point on a face, on an edge and
  at a vertex, and on an edge's extension.

**What Cavendish's S5 kernel should follow** (for an analytic polyhedral body read directly, and for
exact pieces as near leaf kernels):
1. Werner & Scheeres' face-by-face form above, f64, faces stored as (n_f, 2A_f, vertices) so the
   solid angle's numerator is h_f·2A_f; L_e by log1p, with the r₀² form of a + b − ℓ when the point
   projects onto the edge; degenerate cases as listed, each with a test.
2. Use it only within R/s ≲ 10³ of the facet size (relative error ≲ 3×10⁻¹⁰); beyond, the body's or
   piece's exact moments, as now. The switch distance is the same kind of rule as `PRISM_WITHIN`.
3. The Laplacian −Gρ Σω_f (eq. 17) comes free: 0 outside, −4πGρ inside. Assert it as an
   inside/outside check on every oracle point (a point inside a body is a bug in the scene).
4. Do not adopt Holstein & Ketteridge's quadrature hybrid: its point is to stretch the closed form to
   large R/s, and Cavendish already has exact moments there. Adopt their error analysis as the
   reason for the switch distance.

## 2. Mass anti-aliasing: what the orders are, and why

### 2.1 Error of a boundary cell, to first orders

Take a cell (centre c, pitch h) and a point p far from it (d = |p − c| ≫ h). The cell's piece P
creates the field ∫_P ρK, with K(x) = 1/|x − p|. Expanding K about c:

```text
∫_P ρK = ρ|P| K(c) + ρ∫_P (x−c)·∇K + ½ ρ∫_P (x−c)(x−c)ᵀ : ∇∇K + O(ρh⁶/d⁴)
          monopole     dipole ~ ρh⁴/d²     quadrupole ~ ρh⁵/d³
```

There are about A/h² boundary cells for a surface of area A:
- **(i) 0/1:** the monopole is wrong by up to ρh³ a cell, i.e. ρAh in total. **First order.**
  Renormalising the total mass removes the sum but not its distribution.
- **(ii) exact fill at the centre:** the monopole is exact and the dipole is wrong by m(c̄ − c),
  i.e. ρAh² in total. **Second order.**
- **(iii) fill at the centroid:** monopole and dipole exact; the quadrupole is wrong (a cube's or a
  point's, not the piece's), ρAh³. **Third order.**
- **(iii+) with the second moment:** the next term, ρAh⁴. **Fourth order.**
- **(ii-s) today's sampled fill:** the count is a 0/1 voxelisation at δ = h/k. Its monopole error is
  ρAδ = ρAh/k, so it is **first order** with a constant k times smaller. The measured ratio of
  zero_one to ss4 on the aligned cube is 3–11, against k = 4 expected (§3).

### 2.2 Why turned faces do better: equidistribution

These are worst-case bounds. When a face is turned off the lattice, consecutive cells along it cut
it at every phase, and the cell errors average:
- For 0/1, the signed volume errors average to zero over a full period of phase, so what remains
  is a discrepancy-like remainder; the measured order is 2.3–2.8.
- The S3 brief's flat-face envelope is first order ([S3 §8.1](../slices/S3.md#81-criteria-written-before-measuring)).
  That is the right *bound*, and the aligned cube shows it is attained.

### 2.3 Why exact fill at the centre does so well on turned faces (derived, then measured)

Average the dipole error of (ii) over the phase s of a plane with unit normal n crossing a cube
(inside where n·x < s). Then ∫ds ∫_cube χ(n·x < s)(x − c) dx = −∫_cube (x − c)(n·(x − c)) dx =
−(h⁵/12) n. This holds for *every* n, because the cube's second moment is isotropic. The cells per
unit area per unit phase number 1/h³, so the surface carries on average a dipole layer of density
−(ρh²/12) n̂. A uniform double layer on a closed surface has no exterior field: the divergence
theorem gives ∮ n̂·∇K dA = ∫∇²K dV = 0 for p outside. So the second-order term vanishes on
average. What remains comes from imperfect equidistribution (edges, corners, faces aligned with the
lattice) and from curvature, which makes the layer non-uniform at O(h/R_curv).

Measured: order 3.5–4.4 on the turned cube, the icosahedron and the plate; 3.0 on the sphere
(curvature); 2.0 on the aligned cube (no equidistribution, each face at a single phase).

The centroid variant (iii) removes the dipole exactly but replaces it with a quadrupole layer
(the piece is flat, anisotropic). That layer is not harmonic-null, so (iii) sits at order 3 even where
(ii) does better. (iii) is still the robust choice: it is order 3 on *every* body, while (ii) falls
to order 2 on aligned faces, which are the common case in a voxel world. (iii+) is better than both
everywhere.

This parallels interface reconstruction in multi-material flow. There a cell's volume fraction alone
fixes the interface to first order, and the fraction plus the material centroid (moment of fluid)
to second ([Dyadechko & Shashkov 2008](https://doi.org/10.1016/j.jcp.2007.12.029); claim not checked
against the text, §7).

## 3. Measurements

Bodies: `cube_aligned` (0.8 m, faces on lattice planes), `cube` (turned), `icosahedron`
(circumradius 0.6 m, turned), `sphere` (an icosphere of 1280 faces, radius 0.5 m, so faces of about
5 cm: for h ≥ 1/16 m it acts as curved, at 1/64 m as faceted), and `plate` (1 m × 1 m × 5 cm,
turned: thinner than a cell for h ≥ 1/16 m). ρ = 2700 kg/m³. Lattice offsets are uniform random in
[0, h)³, four placements per pitch. The tables give the RMS over the placements of the worst of
three directions. Phase errors are in radians and scale with ρ. The signal row is the phase the body
itself gives.

**D = 1.5 m**: phase error (rad) at h = 1/64 m, RMS over placements, worst direction; (order over h = 1/8 to 1/64)
| representation | cube_aligned | cube | icosahedron | plate | sphere |
|---|---|---|---|---|---|
| *signal* | *201* | *200* | *214* | *18.3* | *203* |
| zero_one_raw | 3.5e+00 (1.0) | 5.6e-03 (2.7) | 1.7e-02 (2.5) | 1.1e-02 (2.3) | 5.1e-02 (1.8) |
| zero_one | 9.1e-01 (0.9) | 6.5e-03 (2.3) | 5.3e-03 (2.8) | 4.7e-03 (2.3) | 1.3e-02 (2.2) |
| ss4 | 1.2e-01 (1.2) | 1.2e-04 (2.6) | 2.7e-04 (2.4) | 1.8e-04 (2.5) | 6.8e-04 (2.2) |
| frac | 4.9e-03 (2.0) | 2.5e-06 (3.8) | 1.3e-05 (3.5) | 2.0e-06 (3.8) | 7.6e-05 (2.9) |
| frac_cen | 1.0e-05 (3.3) | 1.7e-05 (2.9) | 2.9e-06 (2.9) | 4.2e-05 (2.8) | 6.8e-07 (3.8) |
| frac_pt | 1.0e-05 (3.3) | 1.7e-05 (2.9) | 2.9e-06 (2.9) | 4.2e-05 (2.8) | 6.8e-07 (3.8) |
| frac_cen_q | 3.9e-09 (4.9) | 4.0e-07 (3.8) | 3.0e-07 (3.9) | 2.6e-08 (3.4) | 2.5e-07 (3.8) |

**D = 10 m**: phase error (rad) at h = 1/64 m, RMS over placements, worst direction; (order over h = 1/8 to 1/64)
| representation | cube_aligned | cube | icosahedron | plate | sphere |
|---|---|---|---|---|---|
| *signal* | *2.89* | *2.89* | *3.09* | *0.282* | *2.93* |
| zero_one_raw | 4.8e-02 (1.0) | 1.2e-04 (2.5) | 2.3e-04 (2.6) | 1.9e-04 (2.3) | 6.8e-04 (1.8) |
| zero_one | 9.7e-03 (0.9) | 4.2e-05 (2.6) | 4.0e-05 (2.6) | 5.2e-05 (2.3) | 9.1e-05 (2.1) |
| ss4 | 9.0e-04 (1.3) | 6.6e-07 (2.8) | 1.2e-06 (2.9) | 1.9e-06 (2.7) | 6.1e-06 (2.0) |
| frac | 3.1e-05 (2.2) | 6.4e-09 (4.4) | 8.3e-08 (3.9) | 3.5e-09 (4.2) | 5.8e-07 (3.0) |
| frac_cen | 3.2e-08 (3.0) | 3.2e-09 (3.0) | 4.6e-10 (4.2) | 1.3e-08 (2.9) | 2.2e-10 (4.2) |
| frac_pt | 3.2e-08 (3.0) | 3.2e-09 (3.0) | 4.6e-10 (4.2) | 1.3e-08 (2.9) | 2.2e-10 (4.2) |
| frac_cen_q | 1.1e-10 (2.2) | 3.4e-11 (3.3) | 4.0e-11 (3.7) | 9.5e-12 (2.9) | 4.3e-11 (3.6) |

Full per-body tables at both distances: [`results/tables.md`](exact-skin-and-partial-fill/results/tables.md).
Boundary cells at h = 1/64 m: 15,761 (aligned cube), 24,442 (cube), 21,163 (icosahedron), 13,774
(plate), 19,196 (sphere).

Reading the numbers:
- **Precision floor (corrected after reading Holstein & Ketteridge).** Pieces of 1/64 m seen from
  10 m are at R/s ≈ 600, where the benchmark's triple-product solid angle is good to only ~10⁻⁹
  relative per piece (§1.1; the earlier, lower floor came from a dyadic self-test). The boundary
  pieces carry roughly 0.1–0.2 rad of the 10 m signal, so the reference itself may be off by up to
  ~10⁻¹⁰ rad if those errors add. The 10 m frac_cen_q row (about 10⁻¹¹ rad) is therefore below the
  floor and means nothing beyond "small"; the 10 m frac_cen entries of 2–5×10⁻¹⁰ rad (sphere,
  icosahedron) are within a few times of it. Everything else at 10 m is at least 30× above it, and
  the 1.5 m table (R/s ≲ 130, rounding ≲ 10⁻¹¹ relative per piece) is unaffected. Rerunning
  `skin_bench.py` with the h_f·2A_f numerator would settle the 10 m rows.
- **Cube or point for a partial cell.** frac_cen and frac_pt agree to 3 digits: at 1.5 m and beyond
  it makes no difference whether a partial cell is drawn as a cube or as a point at its centroid.
- **Renormalisation.** It helps 0/1 by 2–4× near; it does not change the order.
- **Orders.** They are least-squares slopes over four pitches and four placements; read them to
  ±0.3.

**Measured cost** (`kbench`, a 60-line Rust program in the scratchpad, not committed). It ran on one
thread on the owner's PC, below normal priority, with one other benchmark process running, so the
numbers are upper bounds:

| kernel | ns per evaluation |
|---|---|
| cube series, monopole + l = 4, U only (S3's far kernel) | 8.5 |
| Nagy prism, U only (S3's near kernel) | 370 |
| one polyhedron triangle, U and g | 55 |

### 3.1 Curved and non-convex shapes: sphere, torus, Utah teapot, Suzanne (25 Nov 2026)

Code and numbers: [`exact-skin-and-partial-fill/shapes/`](exact-skin-and-partial-fill/shapes/)
(`shapes_lib.py`, `analytic.py`, `sweep.py`, `regimes.py`, `primitives.py`, `figures.py`). Every run
was one process at a time, below normal priority, under a time limit. The tower and phase are as in §3,
ρ = 2700 kg/m³, and the lower cloud is 1.5 m or 5 m from the surface along (1, 0.2, −0.3) from the
body's centre.

**Shapes.**
- **Analytic sphere and torus.** The sphere has radius 1 m; the torus has R = 1 m and r = 0.4 m. Their
  references use no mesh. The sphere's is the closed form (shell theorem). The torus's is a sum of thin
  rings, each in closed form with complete elliptic integrals, over Gauss–Legendre × trapezoid
  quadrature of the cross-section. Doubling that quadrature moves the phase by 3×10⁻¹⁴ rad.
- **How the cells are built for them.** Techniques (a)–(e) voxelise a mesh (an icosphere of 5120
  triangles; a 64 × 32 torus). Techniques (c★) and (d★) take each cell's fill, centroid and second
  moment from the analytic surface directly. The z-chords are exact, and the (x, y) quadrature is split
  at every kink. Volume is right to 10⁻¹³–10⁻¹¹ relative.
- **Teapot and Suzanne.** The teapot is Newell's, 6320 triangles; Suzanne is Blender's, 968
  triangles. Both are downloaded at run time from a pinned commit of `common-3d-test-models` and
  checked against a SHA-256. That repository states no licence for either, so neither mesh nor
  anything derived from it is committed.

**Repairing the open meshes.** Both downloads are open. The first closing re-meshed them with
marching cubes on a 1/48 m grid, which gave the jagged lid rim, the stray spout fragment and the column
under Suzanne's chin. It is replaced by a repair that keeps the original triangles:
1. Weld vertices that agree to 8 decimals.
2. Drop back-to-back triangle pairs (Suzanne has one, a zero-thickness sheet).
3. Split into edge-connected parts: the teapot's body, lid, spout and handle; Suzanne's head and two
   eyes.
4. Cap each hole with a fan of triangles to its centroid: 6 holes on the teapot, 4 on Suzanne.
5. Unite the parts with manifold3d in double precision, so a spout or an eye that passes into the body
   counts once.

**The automatic check** (`python shapes_lib.py` writes `mesh-check.md`). The pass criterion was
written before measuring:
- the result is closed;
- it has the expected number of edge-connected pieces (one for each mesh);
- every vertex of the result lies within 10⁻⁶ m of the original triangles or the caps;
- every original vertex that is not inside another part lies within 10⁻⁶ m of the result.

The last two together are a vertex-sampled two-sided Hausdorff distance. The control arm runs the same
check on the rejected marching-cubes closing, which must fail it:

| mesh | closed | pieces (expected) | result → original (m) | original → result (m) | verdict |
|---|---|---|---|---|---|
| teapot, repaired | yes | 1 (1) | 1.5×10⁻¹⁷ | 0 | pass |
| teapot, marching cubes (control) | yes | **2** (1) | **7.6×10⁻³** | **2.4×10⁻²** | fail |
| Suzanne, repaired | yes | 1 (1) | 5.4×10⁻¹⁷ | 0 | pass |
| Suzanne, marching cubes (control) | yes | 1 (1) | **8.6×10⁻³** | **1.8×10⁻²** | fail |

The spout fragment fails on the piece count. Suzanne's column is attached to the head, so it passes
the count but fails on distance. Front and side renders of the repaired meshes match the raw
downloads; Suzanne's long muzzle is in the original. The pictures label the left panel "repaired
mesh" or "analytic" and say what was done:
[`teapot.png`](exact-skin-and-partial-fill/shapes/teapot.png),
[`suzanne.png`](exact-skin-and-partial-fill/shapes/suzanne.png),
[`sphere.png`](exact-skin-and-partial-fill/shapes/sphere.png),
[`torus.png`](exact-skin-and-partial-fill/shapes/torus.png) and their `-cutaway` versions.

**Resolution sweeps** ([`sweep.png`](exact-skin-and-partial-fill/shapes/sweep.png), full tables
with cell counts, boundary counts, piece triangles, clip time, cost and memory in
[`sweep.md`](exact-skin-and-partial-fill/shapes/sweep.md)). Pitches are body size / h = 4 … 128
(at 128: 77,012 boundary cells on the sphere, 38,399 on Suzanne). Techniques (a)–(e) are measured
against the mesh (the voxel error); (c★) and (d★) against the analytic shape.

Phase error at 1.5 m (rad) at size/h = 128, and the fitted order in h (size/h ≥ 8):
| technique | sphere | torus | teapot | Suzanne |
|---|---|---|---|---|
| (a) cubes (centre inside) | 5.2×10⁻² (2.0) | 1.6×10⁻¹ (1.7) | 1.8×10⁻² (1.2) | 2.0×10⁻³ (1.4) |
| (b) sampled fill 4³, renormalised | 4.8×10⁻⁴ (2.2) | 5.4×10⁻⁴ (2.2) | 3.4×10⁻⁴ (0.9) | 1.6×10⁻⁴ (1.9) |
| (c) exact fill at the centroid | 8.9×10⁻⁸ (3.1) | 1.1×10⁻⁵ (2.6) | 1.2×10⁻⁷ (3.0) | 9.0×10⁻⁸ (3.1) |
| (d) (c) + second moment | 9.6×10⁻⁸ (3.9) | 2.4×10⁻⁷ (3.8) | 5.9×10⁻¹⁰ (3.8) | 7.8×10⁻¹⁰ (3.8) |
| (e) clipped pieces | ≤ 10⁻¹¹ at every pitch | ≤ 2×10⁻¹² | ≤ 2×10⁻¹² | ≤ 2×10⁻¹² |
| (c★) analytic fill at the centroid | 1.8×10⁻⁷ (2.9) | 1.4×10⁻⁵ (2.6) | — | — |
| (d★) (c★) + second moment | 9.6×10⁻⁸ (3.9) | 2.4×10⁻⁷ (3.8) | — | — |

- **(e) is flat at rounding** at every pitch and shape (shown in
  [`polyhedra.png`](exact-skin-and-partial-fill/shapes/polyhedra.png)), and its mass error is
  10⁻¹⁶. (d★) agrees with (d) to two digits at every pitch: the voxel error does not depend on
  whether the pieces come from the mesh or the analytic surface.
- **The facet error dominates for the analytic shapes.** The mesh's own error against the analytic
  shape falls as (triangle count)^−0.99, which is order 2 in the edge length. The 5120-triangle
  icosphere is off by 0.64 rad at 1.5 m and the 4096-triangle torus by 1.8 rad. At 81,920 and
  262,144 triangles they are still off by 0.04 and 0.03 rad, which is 10⁵ times the voxel error of
  (d) at size/h = 128. For an analytic body, the mesh, not the voxels, limits the accuracy. Building
  the cells from the analytic surface ((c★), (d★)) removes the mesh from the chain.
- **Measured anomaly: on the sphere, (d) is not better than (c).** The pass criterion was written
  first: if (d★) beat (c★) in RMS over 26 random tower directions, the one-direction result was a
  coincidence. It did not. At h = 0.25, 0.125 and 0.0625 m, the RMS of (c★) is 1.7×10⁻³, 5.0×10⁻⁵ and
  9.0×10⁻⁶ rad, and of (d★) 4.3×10⁻³, 3.1×10⁻⁴ and 2.1×10⁻⁵ rad. (d★) wins in only 1–2 of the 26
  directions. On the sphere, the pieces' summed quadrupole term (RMS 4.0×10⁻³, 3.5×10⁻⁴, 1.2×10⁻⁵ rad)
  is largely cancelled by their higher moments, which (c) also leaves out. The two meet at
  size/h = 128. On the torus, teapot and Suzanne, (d) beats (c) by 10–200× at every pitch. This is not
  explained; it is listed in §8.

**Error against cost** ([`pareto.png`](exact-skin-and-partial-fill/shapes/pareto.png)). The cost is
the §3 kernel model for a direct sum per evaluation point: 8.5 ns per cube or point, 17 ns for point
+ quadrupole, 55 ns per triangle. Memory is modelled at 7 bytes per leaf, plus 16 bytes per boundary
leaf for (c), 40 for (d), or 36 per piece triangle for (e).

Cheapest cost per point (µs) to bring the 1.5 m error below 10⁻⁴ / 10⁻⁶ / 10⁻⁸ rad within
size/h ≤ 128 (— means not reached):
| technique | sphere | torus | teapot | Suzanne |
|---|---|---|---|---|
| (b) sampled fill | — / — / — | — / — / — | — / — / — | 100 / — / — |
| (c) fill at centroid | 27 / 510 / — | 324 / — / — | 5.1 / 259 / — | 3.6 / 211 / — |
| (d) + second moment | 77 / 1890 / — | 49 / 1160 / — | 0.81 / 14 / 286 | 1.3 / 19 / 407 |
| (e) clipped pieces | 690 at size/h = 4 | 515 | 674 | 218 |
| the whole mesh, directly (exact) | 282 | 225 | 366 | 63 |

- **(d) is the cheapest route to 10⁻⁴ and 10⁻⁶ rad** on the teapot and Suzanne, by 3–18× over (c).
- **For these 1,000–7,000-triangle meshes, summing the whole mesh directly is exact and cheaper than
  (e) everywhere.** It is cheaper than every voxel technique at 10⁻⁸ rad except the teapot's (d), at
  286 against 366 µs.
- **The caveat is the cost model.** It has no tree walk. A walk aggregates far cells, so for many
  bodies, large meshes or far evaluation points the balance moves towards the voxel techniques.
  What the table settles is the ordering within the voxel techniques: (d) < (c) ≪ (b) < (a) per unit
  of accuracy.

**Near, mid and far** ([`regimes-distance.png`](exact-skin-and-partial-fill/shapes/regimes-distance.png),
[`regimes-distance.md`](exact-skin-and-partial-fill/shapes/regimes-distance.md)). Distance is from the
surface. Each entry is where the technique's error stays below the tolerance for every larger sampled
distance:
| technique | sphere (h 1/16 m) 1e-6 / 1e-10 rad | torus (h 0.05 m) | teapot (h 1/32 m) |
|---|---|---|---|
| (b) sampled fill | 28 m / 167 m | 28 m / 409 m | 28 m / 261 m |
| (c) fill at centroid | 3.0 m / 18 m (48 / 286 cells) | 11 m / 68 m | 4.7 m / 44 m |
| (d) + second moment | 4.7 m / 28 m (75 / 447 cells) | 3.0 m / 18 m (60 / 358 cells) | 1.2 m / 7.3 m (39 / 234 cells) |
| (e) clipped pieces | from 0.5 cells / from 0.5 cells | same | same |
| (f) body multipole to l = 3, no voxels | 4.7 m / 18 m | 28 m / 107 m | 7.3 m / 28 m |
| (g) point mass | 4.7 m / 18 m | 107 m / 639 m | 28 m / 167 m |

(a) reaches 10⁻⁶ rad only beyond 260–640 m and never reaches 10⁻¹⁰ rad in range (to 1000 sizes). On
the sphere, (f) and (g) measure the icosphere's departure from a sphere.

**Voxel primitives** ([`primitives.png`](exact-skin-and-partial-fill/shapes/primitives.png),
[`primitives.md`](exact-skin-and-partial-fill/shapes/primitives.md)). Every primitive is given each
cell's exact piece mass; only its shape and position change. Phase error (rad) at 1, 10 and 100 cells
from the surface, teapot at h = 1/16 m, with numpy time per (cell, point) pair:
| primitive | 1 cell | 10 cells | 100 cells | ns per pair (numpy) |
|---|---|---|---|---|
| (1) point at the cell centre | 3×10⁻¹ | 9×10⁻³ | 1×10⁻⁵ | 99 |
| (2) point at the piece's centroid | 2×10⁻¹ | 1×10⁻³ | 1×10⁻⁶ | 95 |
| (3) uniform sphere at the centre | 3×10⁻¹ | 9×10⁻³ | 1×10⁻⁵ | 96 |
| (4) uniform cube (prism) | 4×10⁻¹ | 9×10⁻³ | 1×10⁻⁵ | 3832 |
| (5) cube series, l ≤ 4 | 3×10⁻¹ | 9×10⁻³ | 1×10⁻⁵ | 491 |
| (6) Gaussian, σ = h/√12 | 3×10⁻¹ | 9×10⁻³ | 1×10⁻⁵ | 210 |
| (7) trilinear density | 1×10⁰ | 9×10⁻³ | 1×10⁻⁵ | 101,516 |
| (8) exact clipped piece | 9×10⁻¹³ | 4×10⁻¹³ | 3×10⁻¹³ | 14,298 |

Beyond about two cells, the primitives centred on the cell, (1) and (3)–(7), give the same error to
the digit. Their shape matters only within a cell or two. Where the mass sits matters at every
distance: (2), the centroid, is 7–100× better. The sphere and torus rows say the same (`primitives.md`).
The numpy times compare primitives with each other; they are not engine costs (§3 has those).

**Orientation and motion.**
- **Static angles** ([`orientation.png`](exact-skin-and-partial-fill/shapes/orientation.png),
  h = 0.1 m).
  - Torus turned about its axis: (c) and (d) do not change, at 1.2×10⁻³ and 8.6×10⁻⁵ rad to two
    digits, while (b) wanders between 5×10⁻⁴ and 3×10⁻² rad.
  - Box turned about a generic axis: (c) is best aligned (1.9×10⁻⁵) and worst at 10° (1.0×10⁻³). That
    is §2.3's aligned-face cancellation.
  - Teapot: (d) stays at 1.0–1.3×10⁻⁵ rad at every angle.
- **Translation past a tower** ([`translation.png`](exact-skin-and-partial-fill/shapes/translation.png),
  [`translation-spectrum.png`](exact-skin-and-partial-fill/shapes/translation-spectrum.png)). The
  torus passes the tower at 1.5 m closest approach, h = 0.1 m, sampled at h/16.
  - Body-frame voxels, voxelised once and moved rigidly, give a smooth error with no lines.
  - World-fixed voxels, re-voxelised on a fixed grid, add lines at k/h = 10, 20, 30 per metre (10 Hz
    per m/s of speed). The amplitude at 1/h is 0.22 rad for (a), 2.3×10⁻⁴ for (b), 3.6×10⁻⁷ for (c)
    and 1.0×10⁻⁸ for (d).
  - The largest error of (d) is 2.8×10⁻⁷ rad in either frame.
- **Spin** ([`spin.png`](exact-skin-and-partial-fill/shapes/spin.png)). The torus spins about its
  axis, h = 0.1 m.
  - Body frame: lattice harmonics 4 and 8 of the spin, plus a once-per-turn term (2×10⁻² rad for (a)),
    because the voxelised mass's centre is off the axis. (d)'s peak-to-peak is 7×10⁻⁸ rad.
  - World-fixed: the error repeats every 5.625°, the mesh's 64-fold symmetry, so its harmonics are
    64k of the spin. At harmonic 64, (c) is 6×10⁻⁶ rad and (d) 3×10⁻⁷ rad, on top of a steady offset.

### 3.2 Famous meshes: scans, a cow and a CAD part (26–27 Nov 2026)

The same machinery (`shapes_lib.py`, `sweep.py`, `regimes.py`, `figures.py`) on six more meshes. Every
run was one process at a time, below normal priority, under a time limit.

**Sources and licences.** Each is downloaded at run time from the URL below and checked against a
pinned SHA-256 (`shapes_lib.SOURCES`). No mesh file, and nothing made from one but numbers and
pictures, is committed. The Stanford URLs are not versioned; the hash pins the content.

| mesh | source (file used) | licence, as stated by the source |
|---|---|---|
| Stanford Bunny | Stanford 3D Scanning Repository, `bunny.tar.gz` (`bun_zipper.ply`, 69,451 triangles) | research use with credit to the Stanford Computer Graphics Laboratory; "you can do anything you want to the Stanford bunny or the armadillo" |
| Spot | Keenan Crane's 3D model repository (CMU), `spot.zip` (`spot_triangulated.obj`, 5,856) | CC0 1.0 |
| Stanford Dragon | Stanford repository, `dragon_recon.tar.gz` (`dragon_vrip.ply`, 871,414) | research use with credit; not for commercial use or products |
| Armadillo | Stanford repository (Venkat Krishnamurthy), `Armadillo.ply.gz` (345,944) | as the bunny |
| Happy Buddha | Stanford repository, `happy_recon.tar.gz` (`happy_vrip.ply`, 1,087,716) | as the dragon |
| mk7 extruder drive (CAD part) | Thingi10K file 44950 (Thingiverse thing 13727, by jag), Hugging Face mirror at a pinned commit | public domain (Thingi10K's metadata) |

Thingi10K has no single canonical part. This one was picked from its metadata: public domain, one
closed, edge-manifold, oriented component with no self-intersections, about 5,000 triangles, and
mechanical (flat faces, bolt holes, a fillet). Lucy (Stanford, 28,055,742 triangles in `lucy.ply`,
read from the archive's header) is in §3.3 only.

**Orientation and size.** Each is turned so its up is +z and its front faces +y (towards the camera)
and scaled so its largest extent is 1 m, as the teapot and Suzanne.

**Repair, decimation and the check.** The repair of §3.1 keeps the original triangles. Two changes,
both found by `cav`'s own importer, which refused the first results as crossing themselves:
- **Holes are ear-clipped**, not fanned to their centroid. The bunny's five base holes are bent, and a
  fan to the centroid passed through the surface beside one of them. Ear clipping uses only the rim
  vertices; the fan is the fallback (not needed for any hole here).
- **The check counts self-crossings** (an edge through a triangle by more than 10⁻⁹ m, the importer's
  test, all counted: `self_crossings`). Its control arms: it finds none in the meshes `cav` accepts,
  and 33 in a decimated dragon `cav` refused. The 10⁻⁹ m depth is there because manifold3d's union
  leaves two sliver touches at Suzanne's eye rims, 10⁻¹² m deep, which `cav` accepts.

The dragon, Armadillo and Buddha are decimated before voxelising (the Python clipper costs grow
with cells × triangles): quadric-error decimation (`fast_simplification`, MIT) of the welded scan,
then the repair (it closes the few non-manifold edges the decimator leaves), then the triangles
around any self-crossing are cut out and the holes closed again, until none is left. manifold3d's
`simplify(tolerance)` was tried first: it never crosses itself, but it removed thin parts (original
vertices 1–2 cm from its result at 50,000 and 112,000 triangles).

[`mesh-check.md`](exact-skin-and-partial-fill/shapes/mesh-check.md), the pass criteria written
before measuring (as §3.1, plus no self-crossings):

| mesh | triangles used | holes capped | pieces | result → original (m) | original → result (m) | self-crossings | control (marching cubes 1/48 m) |
|---|---|---|---|---|---|---|---|
| bunny | 69,664 (all 69,451 kept) | 5, ear-clipped | 1 | 0 | 0 | 0 | fails: 7.7×10⁻³ / 1.5×10⁻² m |
| Spot | 5,856 (closed as downloaded) | 0 | 1 | 0 | 0 | 0 | fails: 8.8×10⁻³ / 1.2×10⁻² m |
| dragon | 49,138 (decimated) | 0 | 1 | 0 | 0 | 0 | fails: 6 pieces, 8.2×10⁻³ / 4.0×10⁻² m |
| Armadillo | 59,996 (decimated) | 0 | 1 | 0 | 0 | 0 | fails: 4 pieces, 1.1×10⁻² / 3.4×10⁻² m |
| Buddha | 47,974 (decimated) | 0 | 1 | 0 | 0 | 0 | fails: 6 pieces, 8.6×10⁻³ / 3.1×10⁻² m |
| mk7 part | 5,280 (closed as downloaded) | 0 | 1 | 0 | 0 | 0 | fails: 9.0×10⁻³ / 2.4×10⁻² m |

(For a decimated mesh "original" here is the decimated mesh: the repair check and the decimation
check are separate.) The teapot and Suzanne pass the new criterion as well.

**The decimation against the original download** (same frame, vertex-sampled both ways, exact
point-to-triangle distances). The pass criterion, written first, was every distance below half the
finest pitch of the sweep, 1/256 m = 3.9 mm:

| mesh | original | decimated to | cut out where it crossed itself | after repair | decimated → original (m) | original → decimated (m), max / RMS | original vertices beyond 3.9 mm | volume change | verdict |
|---|---|---|---|---|---|---|---|---|---|
| dragon | 871,414 | 50,000 | 118 triangles, 1 round | 49,138 | 2.2×10⁻³ | 2.2×10⁻² / 5.8×10⁻⁴ | 691 | −7.1×10⁻⁴ | **fail** |
| Armadillo | 345,944 | 60,000 | 0 | 59,996 | 2.6×10⁻³ | 3.3×10⁻³ / 2.8×10⁻⁴ | 0 | −5.7×10⁻⁴ | pass |
| Buddha | 1,087,716 (78 on non-manifold edges dropped) | 50,000 | 91, 1 round | 47,974 | 2.5×10⁻³ | 1.0×10⁻² / 5.0×10⁻⁴ | 1,912 | −3.9×10⁻⁴ | **fail** |

- The Armadillo first missed by 0.1 mm at 50,000 triangles (4.0 mm); 60,000 passes.
- The dragon and the Buddha fail, and not for want of triangles: at 150,000 triangles the dragon's
  worst original vertex is still 2.2×10⁻² m away (615 beyond 3.9 mm) and the Buddha's 6.8×10⁻³ m
  (737), and manifold3d's simplify gave 2.3×10⁻² m for the dragon. The far vertices sit in a few
  clusters low on each body (the dragon: a fin-like feature 3 cm tall under it; the Buddha: thin
  walls under the robe, z ≈ −0.3 m). Read: features thinner than a decimated triangle, which any
  decimation to this size collapses. They carry little mass (volume change 4–7×10⁻⁴). The tolerance
  was not moved; the meshes are used as they are, as what the voxels are measured against. §3.4
  measures what the decimation changes in the signal.

**Pictures** (pitch 1/16 m; repaired mesh | cube voxels | cube interiors and clipped pieces, and
the same cut away at the lattice wall nearest the centre plane):
[`bunny.png`](exact-skin-and-partial-fill/shapes/bunny.png),
[`spot.png`](exact-skin-and-partial-fill/shapes/spot.png),
[`dragon.png`](exact-skin-and-partial-fill/shapes/dragon.png),
[`armadillo.png`](exact-skin-and-partial-fill/shapes/armadillo.png),
[`buddha.png`](exact-skin-and-partial-fill/shapes/buddha.png),
[`bracket.png`](exact-skin-and-partial-fill/shapes/bracket.png), each with its `-cutaway`.

**Resolution sweeps** ([`sweep-more.png`](exact-skin-and-partial-fill/shapes/sweep-more.png);
tables in [`sweep.md`](exact-skin-and-partial-fill/shapes/sweep.md)). Phase error at 1.5 m at
size/h = 128 against the mesh, and the fitted order in h (size/h ≥ 8):

| technique | bunny | Spot | dragon | Armadillo | Buddha | mk7 part |
|---|---|---|---|---|---|---|
| (a) cubes | 3.1×10⁻⁴ (2.8) | 3.1×10⁻³ (1.4) | 8.7×10⁻⁴ (2.2) | 1.7×10⁻³ (1.7) | 2.9×10⁻⁴ (2.2) | 8.6×10⁻³ (1.6) |
| (b) sampled fill | 2.5×10⁻⁶ (3.4) | 3.4×10⁻⁵ (2.4) | 6.8×10⁻⁵ (2.6) | 5.0×10⁻⁵ (1.2) | 1.0×10⁻⁴ (1.9) | 2.7×10⁻³ (0.6) |
| (c) fill at centroid | 4.0×10⁻⁸ (2.9) | 2.1×10⁻⁷ (3.2) | 1.1×10⁻⁷ (2.8) | 8.4×10⁻⁸ (2.7) | 1.3×10⁻⁷ (3.0) | 1.5×10⁻⁷ (3.3) |
| (d) + second moment | 1.1×10⁻⁹ (3.8) | 9.1×10⁻¹⁰ (3.8) | 3.9×10⁻¹⁰ (3.6) | 4.2×10⁻¹⁰ (3.8) | 2.3×10⁻¹⁰ (2.9) | 7.3×10⁻¹¹ (4.5) |
| (e) clipped pieces | ≤ 4×10⁻¹² | ≤ 2×10⁻¹² | ≤ 8×10⁻¹³ | ≤ 1×10⁻¹² | ≤ 9×10⁻¹³ | ≤ 1×10⁻¹² |

At size/h = 128: 56,987 boundary cells on the bunny, 32,552 on the Buddha; clipping took 60–128 s
per shape in Python.

Cheapest cost per evaluation point (µs, model of §3.1) to bring the 1.5 m error below 10⁻⁴ / 10⁻⁶ /
10⁻⁸ rad within size/h ≤ 128 ([`pareto.png`](exact-skin-and-partial-fill/shapes/pareto.png) now has
all ten shapes):

| technique | bunny | Spot | dragon | Armadillo | Buddha | mk7 part |
|---|---|---|---|---|---|---|
| (b) sampled fill | 844 / — / — | 673 / — / — | 65 / — / — | 883 / — / — | — / — / — | — / — / — |
| (c) fill at centroid | 1.6 / 160 / — | 9.9 / 555 / — | 2.1 / 151 / — | 1.8 / 157 / — | 2.5 / 133 / — | 7.6 / 280 / — |
| (d) + second moment | 1.4 / 40 / 893 | 1.4 / 25 / 582 | 0.51 / 7.0 / 175 | 1.2 / 11 / 208 | 0.36 / 1.9 / 116 | 0.59 / 7.0 / 48 |
| (e) clipped pieces | 5,010 | 594 | 3,740 | 4,330 | 3,280 | 597 |
| whole mesh, directly (exact) | 3,830 | 322 | 2,700 | 3,300 | 2,640 | 290 |

**Near, mid and far** ([`regimes-distance.png`](exact-skin-and-partial-fill/shapes/regimes-distance.png),
all nine shapes at h = 1/32 m for the 1 m bodies). Where each technique stays below the tolerance for
every larger sampled distance, 10⁻⁶ / 10⁻¹⁰ rad:

| technique | bunny | Spot | dragon | Armadillo | Buddha | mk7 part |
|---|---|---|---|---|---|---|
| (a) cubes | 167 m / never | 167 m / never | 107 m / never | 107 m / never | 167 m / never | 261 m / never |
| (b) sampled fill | 11 m / 167 m | 18 m / 261 m | 18 m / 167 m | 18 m / 261 m | 28 m / 261 m | 44 m / 409 m |
| (c) fill at centroid | 4.7 m / 44 m | 4.7 m / 44 m | 3.0 m / 44 m | 3.0 m / 28 m | 3.0 m / 28 m | 3.0 m / 18 m |
| (d) + second moment | 1.2 m / 7.3 m | 1.2 m / 7.3 m | 1.2 m / 7.3 m | 1.2 m / 7.3 m | 0.78 m / 7.3 m | 0.78 m / 3.0 m |
| (f) body multipole, no voxels | 11 m / 44 m | 11 m / 44 m | 11 m / 28 m | 7.3 m / 44 m | 11 m / 28 m | 11 m / 44 m |
| (g) point mass | 44 m / 261 m | 44 m / 261 m | 44 m / 167 m | 44 m / 261 m | 44 m / 167 m | 44 m / 261 m |

(e) is below both from half a cell out, on every shape.

**What the new shapes say.**
- **Thin features do not change the ranking.** On the dragon (horns, whiskers, claws) and the Buddha
  (folds, the gaps by the arms), (d) beats (c) at every pitch, and (c) beats (b) and (b) beats (a)
  at every pitch but one each (the Armadillo's (b) at size/h = 4; the Buddha's (a) at 16, a lucky
  cancellation). (d) is the cheapest route to 10⁻⁴, 10⁻⁶ and 10⁻⁸ rad on all six: 1.1–13× cheaper
  than (c) at 10⁻⁴ rad and 4–70× at 10⁻⁶ rad; only (d) reaches 10⁻⁸ rad within size/h ≤ 128.
- **What thin features do change is the smoothness.** The Buddha's (d) has a dip at size/h = 16
  (2.8×10⁻⁸, then 9.2×10⁻⁸ at 32), so its fitted order is 2.9 against 3.6–3.8 on the other organic
  shapes; (a) and (b) jump about by up to 10× from one pitch to the next on the dragon and the
  Buddha: a fold is a cell or two across, and whether a centre or a sample lands in it flips with the
  lattice. (c) and (d) carry the exact mass and do not jump.
- **The CAD part is where sampling is worst and moments best.** Its large flat faces sit at fixed
  heights, so the sampled fill's bias does not average out: (b) has order 0.6 and is still at
  2.7×10⁻³ rad at size/h = 128 (§2.3's aligned-face case). (d) is the best of all ten shapes there
  (7.3×10⁻¹¹ rad, order 4.5) and reaches 10⁻¹⁰ rad from 3 m.
- **At 50,000–70,000 triangles the whole mesh is no longer cheap.** A direct sum costs 2.6–3.8 ms per
  point, against 0.4–1.4 µs per point for (d) at 10⁻⁴ rad: a factor 2,000–7,000. For the small meshes
  (Spot, the CAD part) the direct sum and (e) cost about the same, as for the teapot.

### 3.3 Full resolution: a performance test (27 Nov 2026)

The scans as downloaded, in the study's frame (1 m largest extent), written as binary STL and as raw
f64 triangles by [`fullres.py`](exact-skin-and-partial-fill/shapes/fullres.py). Triangle counts are
read from the files: bunny 69,451 (69,664 once its five holes are capped), Armadillo 345,944, dragon
871,414 (871,306 after welding drops 108 degenerate ones), Happy Buddha 1,087,716 (1,087,474 after
welding; 32 edges in more than two triangles), Lucy 28,055,742 (14,027,872 vertices, from the header
of `lucy.ply` in the 322 MB archive). Lucy was not downloaded whole: as a binary STL it would be
1.40 GB, and every number below that it needs follows from its triangle count.

**1. Cavendish itself.** [`mesh_bench/src/bin/voxelise.rs`](exact-skin-and-partial-fill/shapes/mesh_bench/src/bin/voxelise.rs)
calls `cav_core::voxel` (release build, one thread, below normal priority): `Mesh::read` for an STL
(the importer's path: the 32 MiB byte cap, the 500,000-triangle cap, closed and no-crossing checks)
or `Mesh::new` for the f64 triangles (the same checks, no byte cap), then `voxelise` with the engine's
default budget, supersample 2. Peak memory is the process's peak working set. The caps were not
changed.

| mesh | triangles | import (s) | voxelise at h = 1/32, 1/64, 1/128 m (s) | leaves | partial leaves | peak memory |
|---|---|---|---|---|---|---|
| bunny, full (holes capped) | 69,664 | 0.18 | 0.45 / 2.0 / 8.3 | 3,550 / 15,542 / 65,132 | 1,849 / 7,183 / 29,045 | 19 MiB |
| Armadillo, full | 345,944 | 1.6 | 1.5 / 6.5 / 26.9 | 2,140 / 9,814 / 43,039 | 1,249 / 5,068 / 20,095 | 85 MiB |
| Armadillo, decimated | 59,996 | 0.15 | 0.29 / 1.3 / 5.4 | 2,086 / 9,798 / 42,665 | 1,223 / 4,955 / 20,188 | 22 MiB |
| dragon, decimated | 49,138 | 0.13 | 0.24 / 1.0 / 4.7 | 2,056 / 9,887 / 44,200 | 1,246 / 5,158 / 20,980 | 19 MiB |
| Buddha, decimated | 47,974 | 0.13 | 0.17 / 0.71 / 3.2 | 1,655 / 7,017 / 33,411 | 1,006 / 3,815 / 16,140 | 19 MiB |
| Spot | 5,856 | 0.02 | 0.04 / 0.19 / 0.79 | 2,787 / 12,146 / 50,228 | 1,352 / 5,667 / 22,240 | 7 MiB |
| mk7 part | 5,280 | 0.02 | 0.03 / 0.14 / 0.52 | 1,639 / 9,491 / 32,237 | 824 / 4,658 / 13,978 | 6 MiB |

Refused, with the importer's own words:
- **bunny as downloaded**: "the mesh is not closed and consistently wound: the edge from vertex 250 to
  vertex 251 is not in exactly two triangles once each way (223 such edges)" (its base holes).
- **dragon and Buddha as STL** (43.6 and 54.4 MB): "… 33554433 bytes is more than 33554432" — the
  32 MiB cap, at reading.
- **dragon and Buddha given as arrays** (no byte cap): "a mesh of 435545 vertices and 871306 triangles
  is more than 1000000 and 500000", and the same with 543,505 and 1,087,422 for the Buddha (closed).
- **Lucy**: would be refused at the byte cap (1.40 GB against 32 MiB) and then at the triangle cap
  (56× it).
- Two meshes of ours were refused on the way and fixed (§3.2): the bunny with centroid-fan caps and
  the quadric-decimated dragon and Buddha ("the mesh crosses itself (an edge of triangle 16437 passes
  through triangle 16440)"). Decimations to 480,000 triangles (to fit under the cap) were refused for
  the same reason; they were not mended (not needed for anything here).

What it shows: import is 2.6–4.6 µs per triangle (parsing, the checks, the hierarchy, the crossing
search);
voxelising grows 4× per halving of the pitch (with the boundary cells) and about in proportion to
the triangles near each cell — the full Armadillo takes 5× as long as its 60,000-triangle decimation
for the same leaves (to 1%). Memory stays under 100 MiB. The tree depends on the shape, not the
triangle count: 43,039 leaves for the full Armadillo at 1/128 m, 42,665 for the decimated one.

**2. The direct whole-mesh sum** ([`mesh_bench/src/main.rs`](exact-skin-and-partial-fill/shapes/mesh_bench/src/main.rs):
Werner & Scheeres per face, the same per-triangle work as §3's `kbench`, one thread, 8 points
1.5–2 m away, each triangle's normal and edge data precomputed):

| mesh | triangles | ms per evaluation point | ns per triangle per point |
|---|---|---|---|
| mk7 part | 5,280 | 0.29 | 55.5 |
| Spot | 5,856 | 0.34 | 57.6 |
| dragon, decimated | 49,138 | 2.7 | 54.1 |
| Armadillo, decimated | 59,996 | 3.2 | 53.3 |
| bunny | 69,664 | 3.7 | 53.5 |
| Armadillo, full | 345,944 | 20.2 | 58.4 |
| dragon, full | 871,306 | 52–59 | 59–67 |
| Buddha, full | 1,087,474 | 66–68 | 61–63 |
| Lucy (extrapolated) | 28,055,742 | ≈ 1,700 | (60) |

The cost per triangle holds at 53–67 ns from 5,000 to a million triangles: about 15% more once the
triangles (~100 bytes each) no longer fit in cache. So a whole-mesh read is 0.07 s per point for the
Buddha and 1.7 s for Lucy, and a signal needs it at every sampled time of every cloud of every tower.
Voxelised once (3–27 s above), the same body read through the tree's far cells and (d)'s cut-cell
moments costs what its leaves cost, which does not grow with the triangle count: that is why the
octree and the boundary moments matter at this size.

**3. The Python clipper at full resolution** (`fullres.py clip`, one process, 10-minute cap): it
finished every run, so nothing is extrapolated.

| mesh | triangles | size/h | boundary cells | piece triangles | seconds | peak memory |
|---|---|---|---|---|---|---|
| Armadillo | 345,944 | 32 / 128 | 2,440 / 40,316 | 0.72 M / 2.5 M | 38 / 144 | 214 / 581 MB |
| dragon | 871,306 | 32 / 128 | 2,459 / 41,677 | 1.46 M / 3.9 M | 75 / 247 | 506 / 910 MB |
| Buddha | 1,087,474 | 32 | 1,789 | 1.64 M | 85 | 604 MB |

The pieces' volumes add up to the mesh's to 10⁻¹⁵. The clipping is cheap; what the full meshes cost
in the sweep is the reference field (55–67 ns × 4 M piece triangles per point) and, for (e), the
piece triangles themselves.

### 3.4 Each mesh as one exact polyhedron: what a gradiometer reads (27 Nov 2026)

No voxels and no clipping: each body is its mesh, and its field is the polyhedral closed form summed
over every triangle ([`exact.py`](exact-skin-and-partial-fill/shapes/exact.py),
[`exact.png`](exact-skin-and-partial-fill/shapes/exact.png),
[`exact.md`](exact-skin-and-partial-fill/shapes/exact.md)). ρ = 2700 kg/m³, 1 m largest extent, the
tower's lower cloud at the origin and the upper one 5 m above, the body's centre of mass level with
the lower cloud.

**What is exact and what is not.**
- Exact to rounding: the field of the mesh at each point (Werner & Scheeres per face with the stable
  numerator, as `skin_bench.py` and §3.1), and the mass moments, by the divergence theorem over the
  same triangles (volume, centre of mass, second and third moments, hence the inertia tensor
  I = ρ(tr M₂ 𝟙 − M₂)).
- Not exact: the phase is k_eff T² Δg_z at the clouds' mean positions at one instant, not the
  integral along the atoms' paths over the sequence's 2T; the multipole curves are series cut at
  l = 2 or 3; and a mesh is an approximation of the object (a scan, its decimation, capped holes).

**Mass moments** (closed form; `exact.md` has all nine, decimated and full):

| mesh | mass (kg) | centre of mass in the box frame (m) | principal moments of inertia (kg m²) |
|---|---|---|---|
| bunny | 540.0 | (+0.026, +0.080, −0.149) | 23.5, 43.4, 48.7 |
| Spot | 382.5 | (0.000, +0.001, −0.069) | 11.7, 34.9, 37.8 |
| dragon (decimated / full) | 150.03 / 150.14 | (+0.030, −0.023, −0.107) | 4.88, 8.48, 11.99 / 4.88, 8.49, 12.01 |
| Armadillo (decimated / full) | 185.33 / 185.43 | (−0.013, −0.042, +0.112) | 6.36, 12.03, 14.75 / 6.37, 12.04, 14.76 |
| Buddha (decimated / full) | 123.11 / 123.16 | (−0.019, −0.020, −0.089) | 1.53, 8.82, 9.02 / 1.54, 8.83, 9.03 |
| mk7 part | 184.3 | (+0.007, +0.151, −0.019) | 4.73, 19.6, 21.3 |

**Phase against distance** (top two rows of `exact.png`, 0.5–50 m from the centre of mass to the
tower's axis). Beyond about 2 m every body reads like its mass: the departure of a point mass from
the exact polyhedron is 3–40% at 1.5 m, 0.05–0.6% at 5 m and 3×10⁻⁵–8×10⁻⁴ at 20 m. Adding the
second moments brings 20 m to 2–8×10⁻⁶, and the third moments to 0.5–2×10⁻⁷. Inside about 2 m the
shape takes over: within one body radius of the axis (grey, 0.57–0.81 m) the lower cloud is in or at
the body, and the bunny, Spot and the dragon show a sign change of the phase near 1 m: a point mass
level with the lower cloud pulls it sideways only, but the real body's mass sits unevenly above and
below that level, and close in its vertical pull on the lower cloud outweighs the upper cloud's.

**Decimation, measured as signal.** The decimated dragon, Armadillo and Buddha differ from the full
scans by a nearly constant 4–7×10⁻⁴ of the signal at every distance (1.1×10⁻³, 1.3×10⁻³ and
6×10⁻⁴ rad at 1.5 m): that is their mass change (§3.2's volume column). It is about a million times
the voxel error of (d) at size/h = 128 against the decimated mesh (2–4×10⁻¹⁰ rad). As with the facet error of the
analytic shapes (§3.1), the mesh, not the voxels, limits the accuracy: a body must be meshed to the
accuracy wanted before its voxels can deliver it.

**Walking past** (bottom row: the centre of mass 2 m from the axis, 1.4 m/s, three poses of the
same body). Same mass, different signal:

| mesh | peak phase, upright / turned 90° about z / on its side / point mass (rad) | largest pose difference |
|---|---|---|
| bunny | 5.00 / 5.52 / 5.53 / 5.54 | 0.55 rad (10%) |
| Spot | 3.98 / 4.68 / 4.00 / 3.92 | 0.84 rad (18%) |
| dragon | 1.49 / 1.50 / 1.54 / 1.54 | 0.06 rad (4%) |
| Armadillo | 1.92 / 2.03 / 1.93 / 1.90 | 0.13 rad (6%) |
| Buddha | 1.29 / 1.24 / 1.26 / 1.26 | 0.05 rad (4%) |
| mk7 part | 1.94 / 1.90 / 2.31 / 1.89 | 0.42 rad (18%) |

At 2 m a 1 m body's shape and pose move the peak by 4–18%, far above any instrument floor: a
gradiometer that sees a walker-sized body pass at 2 m sees its shape, not just its mass. The bodies
with a long axis differ most when it turns towards the tower (Spot turned end-on, the CAD part on its
side); the compact ones least (the Buddha, the dragon).

## 4. Cost at Cavendish's scales (derived)

- **Boundary leaves.** A body of N finest cells has about 6N^(2/3) boundary cells (cube-like):
  6×10⁶ at N = 10⁹, 1.3×10⁸ at 10¹¹. The S3 study found that partial cells rarely merge, so these
  are most of the leaves ([S3 sparse-voxel note](s3-sparse-voxel-structures.md)).
- **(a) Whole body as one polyhedron.** F × 55 ns per point. That is 55 µs for a 1,000-face
  mesh, 0.55 ms for 10⁴. A read of 5 towers × 2 interferometers × 2 arms × ~10 flight nodes is ~200
  points, so 11–110 ms per shot on one thread, and parallel across points. It is exact to rounding
  out to R/s of a few thousand. It needs no voxels at all, but only for a uniform (or piecewise
  uniform) mesh body. Its cost does not depend on the pitch.
- **(b) Exact pieces as leaf kernels, everywhere.** 6×10⁶ × ~0.6 µs ≈ 4 s per point at 10⁹ cells.
  Out of the question as a far kernel, and pointless: past ~3 cells the piece's moments give the
  same answer (§3).
- **(b) Exact pieces near only.** Only leaves within ~3h of a point would use them. Arms run in
  vacuum tubes, so that is a handful of leaves at most, and usually none. The cost is negligible.
- **Exact moments in the tree.** Clipping moves to voxelisation, which runs once per body, or per
  edit. The read does not change: nodes already carry mass, centroid, second and third moments
  ([S3 §2](../slices/S3.md#the-tree-cav_corevoxel)). They would be *exact* for the true body
  instead of exact for the staircase. My Python clipper ran at 1–6 ms per cell. A compiled clipper
  in the style of r3d ([Powell & Abel 2015](https://doi.org/10.1016/j.jcp.2015.05.022), clipping
  one plane at a time, moments by simplex decomposition) should be about 1000× faster. That is not
  measured; I estimate 10⁻⁶–10⁻⁵ s per cell, so 6–60 s for 6×10⁶ boundary cells on one thread,
  divided by the cores. This needs a measurement in the slice.
- **Storage.**
  - Today a leaf is `{key u32, level u8, count u8, material u8}`.
  - Exact fill + centroid as quantised integers (u16 fill, 3 × i16 centroid in units of h/2¹⁵) add
    8 bytes per boundary leaf. Quantising to h/2¹⁵ adds a dipole error far below the order-3 term.
  - Second moments add 6 × i16 or f16, 12 bytes more.
  - Interior leaves do not change: they are full cubes, f = 1, c̄ = c, I = h²/12·𝟙.
  - The S3 note's DAG and compression analysis assumed 8-bit fractions. Wider boundary data favours
    keeping it in a side table indexed by the boundary leaves.

## 5. Recommendation for Cavendish

1. **Now: the polyhedral closed form as the oracle for mesh-made bodies** (in `oracle/`, f64,
   Werner & Scheeres' form with the rules at the end of §1.1). This is the validation case with a
   known answer that S3's voxelised meshes lack today: a voxelised mesh's phase converges to the
   polyhedron's at the measured order.
   - **Pass criteria:** the self-test's checks (prism, series, gradient, pieces sum to the body);
     points at general (non-dyadic) positions out to R/s = 10³ within 10⁻⁹ of the series; a point
     on a face, on an edge, at a vertex and on an edge's extension, finite and continuous with
     points 10⁻⁶ s away (Tsoulis & Petrović; Tsoulis 2012's test set); Σω_f = 0 outside, 4π
     inside.
   - **An independent implementation as a second oracle.** ESA's
     [polyhedral-gravity-model](https://github.com/esa/polyhedral-gravity-model) (C++17 with Python
     bindings, `pip install polyhedral-gravity`) implements Tsoulis 2012's line-integral form with
     the singular terms, returns V, ∇V and the six tensor components with V positive and g = +∇V,
     and checks or repairs face orientation. It is a different formulation from ours, so agreement
     tests the algebra, not just the code. Its licence is GPL-3.0: use it in a script that writes
     fixture files (numbers are not a derived work) and never link or vendor it; if fixtures from it
     are committed, note the tool and version beside them. Pass criterion: agreement to 10⁻¹²
     relative on U and g at R/s ≤ 10², for the four benchmark bodies and a non-convex mesh.
   - **For S5's analytic bodies:** a turned box, a prism or a convex hull can be read *directly* by
     the closed form when F is small (≲ 10³ faces) and the point is within R/s ≲ 10³ of the facet
     size. Beyond that, use its exact moments. The two should match where they overlap.
2. **Next voxeliser revision: exact piece moments instead of sampled counts.**
   - **What it changes.** Clip each boundary cell against the shape. For convex shapes, the cell is
     cut by the shape's planes, as in the benchmark. For meshes, take the cell ∩ the signed tetrahedra
     of the mesh's triangles, which is what r3d does for tetrahedral meshes ([Powell & Abel
     2015](https://doi.org/10.1016/j.jcp.2015.05.022)). Store mass, centroid and second moments; the
     third come from the same decomposition if wanted. Build node moments from them.
   - **What it buys.** With centroids, order 3 everywhere. That is 10⁴× less error than today on the
     aligned cube at 1/64 m, and 4–1000× less on the turned bodies. With second moments, order ≥ 3.4
     and 4×10⁻⁹–4×10⁻⁷ rad at 1/64 m and 1.5 m, about 2×10⁻⁹ relative.
   - **The pitch it saves.** Below 1 µrad near a ~1.4 t body takes h = 1/64 m with second moments
     (1/32 m for the aligned cube). Today's sampled fill stays at ≥ 10⁻⁴ rad at 1/64 m, and on
     aligned faces it would need about 10⁵ times finer cells at its first order. In practice the
     choice is between storing moments and not reaching µrad at all near a body. That is the
     optimisation.
   - **Keep the bit-for-bit raster oracle.** The exact moments are floats, so "the tree equals the
     raster" becomes "the pieces' moments sum to the shape's exact moments to rounding". That is a
     stronger test (shown in the self-test at 10⁻¹⁵).
   - **Keep `count`**, or a quantised fill, for rendering and the `Fill` view.
   - **Analytic shapes** (sphere, ellipsoid, cylinder) are not polyhedra. Their pieces need
     either exact cell integrals of the implicit shape or a fine polyhedral stand-in. The sphere here
     was a 1280-face icosphere. This is a design call for the builder.
3. **Exact pieces as near leaf kernels:** only if a body may come within ~3 cells of an arm point.
   Otherwise the moments suffice. Implement after (2), behind the same distance switch as
   `PRISM_WITHIN`.
4. **Not recommended:** option (c), the thin polyhedral shell. It costs about the same as (b) and is
   harder to build. Also not recommended: carrying on with sampled counts and raising k. The sampled
   fill stays first order on aligned faces; doubling k halves the error at 8× the sampling work.

**The octree merge.** The lossless merge of eight equal leaves stays valid for interior leaves
(f = 1). Boundary leaves with exact moments will essentially never be equal, as S3 already measured
for counts. Node moments are sums of the leaves' (the parallel-axis rule, as now), and they become
exact.

**Materials of different densities.** Store each boundary leaf's *mass moments*, not a fill times
one density. A cell crossed by two materials holds the sum of both pieces' moments, because moments
add. The palette index then serves rendering only (the dominant material), and the gravity data is
material-agnostic. A per-voxel density times a fill could not represent a two-material cell.

**Moving bodies.** Trees are in the body's frame (S3), so rigid motion leaves the leaf data unchanged.
A rotation turns the second and third moments as tensors in the node kernel, as it would anyway.
Editing a body, or bodies voxelised in the world's frame (terrain, fluid), needs the touched boundary
cells clipped again. That is a per-edit cost proportional to the cells edited.

## 6. Validation cases the literature uses

- **Prism against polyhedron.** A rectangular prism has two independent closed forms, Nagy's and the
  polyhedron's. [Okabe 1979](https://doi.org/10.1190/1.1440973) derives the prism from his general
  form (eqs. 46–56) and notes that his U_z (eq. 48) is Nagy's; he also checks the second derivative
  against a finite difference of the first (eq. 68) and the symmetry of a pyramid's anomaly (p. 739).
  [Tsoulis 2012](https://doi.org/10.1190/geo2010-0334.1) tests `polyhedron.f` against Mader's (1951)
  prism formulas (Appendix B) at 28 points in three groups: outside at any distance, *on* the
  boundary (a vertex, two edges, a face), and on the extensions of edges and faces. Table 1 (p. F8)
  shows agreement to 10⁻²⁰–10⁻²¹ m s⁻² on values of 10⁻⁶ (about 10⁻¹⁵ relative), and Laplace's
  equation summing to zero outside. Used here (prism, 10⁻¹⁵); the on-boundary and extension points
  are to be added (§5).
- **Asteroid shape models against harmonics and mascons.** [Werner & Scheeres
  1997](https://doi.org/10.1007/BF00053511), §3: Castalia as 3300 faces at 2.1 g/cm³. A degree-20
  harmonic expansion diverges inside the circumscribing sphere, with potential errors above 100%
  near the surface (Figs. 13–14). A 3300-mascon model on a 59 m grid has attraction errors of 1–10%
  over most of the surface, above 10% in pockets, and 0.1–1% well away from the body; tripling the
  mascons (41 m grid) still leaves appreciable errors (Figs. 16–17, pp. 339–341). Their reasoning
  is this note's first-order argument: the mascon error is proportional to the mascon size, so a
  tenfold gain needs a thousandfold more mascons (p. 339). A single cube against a sphere of the
  same mass errs by up to ~50% at the face centres and is below 0.1% only beyond 2.5 sides (Fig.
  15). The [polyhedral-gravity-model paper](https://doi.org/10.21105/joss.06384) cites Wittick &
  Russell (2017) to the same effect; I could not read Wittick & Russell.
- **Singular points.** A point on a face, on an edge and at a vertex, against the limit ([Tsoulis &
  Petrović 2001](https://doi.org/10.1190/1.1444944), Fig. 2 and eqs. 24–29), and Tsoulis 2012's
  boundary and edge-extension points.
- **Far-field rounding.** Anomaly relative to an equivalent point mass against log(1/γ) for each
  method, with the breakdown distance and error slope compared to theory ([Holstein & Ketteridge
  1996](https://doi.org/10.1190/1.1443964), Figs. 4–6, Table 5). Measured here (§1.1): ε(R/s)² for
  the line-class form, ε(R/s)³ and worse with the triple-product solid angle.
- **Exact voxelisation.** Pieces sum to the whole: mass, momentum and moments conserved to rounding
  ([Powell & Abel 2015](https://doi.org/10.1016/j.jcp.2015.05.022)). Used here; 10⁻¹⁵.

## 7. What I could not verify

- **Now read in full (25 Nov 2026):** Okabe 1979, Werner & Scheeres 1997 (published 1997, vol. 65,
  313–344; the 1996 on Springer's page is the receipt year), Tsoulis & Petrović 2001, Tsoulis 2012,
  Holstein & Ketteridge 1996. The earlier caveats about them are withdrawn; Holstein &
  Ketteridge's Figs. 4–5 are the digits-against-distance figure the note said did not exist.
- **Not run: ESA's library against ours.** The cross-check of §5 is specified, not done (it needs
  `pip install polyhedral-gravity` and a fixture script). Its internals were read only as far as the
  tolerance constant; how it treats edge-extension points was not checked.
- **Not rerun: `skin_bench.py` with the h_f·2A_f numerator.** The far-field table in §1.1 comes from
  a separate pure-Python script on single cubes; the 10 m rows of §3 have not been regenerated.
- **Not measured: the f32 error of the closed form.** The "near 10⁻⁵ at R/s = 10" is ε(R/s)² with
  f32's ε.
- **D'Urso 2013 and 2014.** Bibliographic details only; what they claim was not read.
- **Dyadechko & Shashkov 2008.** The moment-of-fluid order claim is from memory, not checked.
- **Wittick & Russell 2017; Park, Werner & Bhaskaran 2010.** Not accessible; their convergence
  figures are not used.
- **Lattice-point bounds.** I found no source for the first-order flat-face statement or the smooth
  convex case. The aligned cube's measured order 1.0 stands in for the first; S3 already measured
  the sphere's volume at order 2.4.
- **Not measured: clipping cost in compiled code.** The 10⁻⁶–10⁻⁵ s per cell is an estimate. Measure
  it in the slice: a Rust clipper on 10⁶ cells.
- **Non-convex meshes and analytic curved shapes: now measured (§3.1).** The clipper works for any
  closed mesh: caps on the cell walls come from lattice lines cast through the mesh. The costs in §3.1
  are a model, not engine timings, and the Python clipping time (3–70 ms per boundary cell) says
  nothing about compiled code.
- **The two meshes' licences.** `common-3d-test-models` states none for the teapot or Suzanne, so they
  are only downloaded at run time, never committed.
- **Not measured: the 20 m column, at the 10⁻¹¹ rad floor.** The JSON files hold every distance
  from 1 to 20 m; the tables show 1.5 m and 10 m.

## 8. Further questions (not chased)

- Why does adding the pieces' second moments make the sphere worse (§3.1), when it helps the torus,
  teapot and Suzanne by 10–200×? Adding the pieces' third moments would test whether the cancellation
  is between orders 2 and 3.

- Third moments per boundary leaf: the walk carries them per node. Would exact third moments make
  the order 5, and is it worth 10 more numbers?
- Can the double-layer cancellation of §2.3 be made exact on aligned faces? For example, a fixed
  sub-cell jitter of the lattice per body, at no storage cost, would restore equidistribution.
- Quantisation: how many bits of fill and centroid keep the error under PHASE_TOLERANCE = 10⁻¹⁰
  rad (`forward.rs`) for a 10⁹-cell body?
- On the GPU, should the near exact-piece kernel be f64, or float-float, given the (R/s)²
  cancellation? Holstein & Ketteridge's surface method (facet area over a constant distance plus a
  small quadrature remainder, eqs. 20–23) runs in f32 with errors that fall with distance; is it a
  better GPU near kernel than float-float?
- For terrain from height fields: exact pieces of a bilinear surface, or of two triangles per
  height-field quad. The latter is polyhedral, so this method applies directly.
