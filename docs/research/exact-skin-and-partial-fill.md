# An exact skin, or mass anti-aliasing: what boundary voxels should carry

Research note, 25 Nov 2026, for the owner's question ("for fun, and for the optimisations it might
give") and for S5 (the bounded walk, instrument LOD, analytic bodies) and any later revision of S3's
voxeliser. Method and numbers: [`exact-skin-and-partial-fill/`](exact-skin-and-partial-fill/)
(`skin_bench.py`, `tables.py`, `results/*.json`, `results/tables.md`).

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

**Recommendation** (§5): (1) make the polyhedral closed form the oracle for mesh-made and analytic
polyhedral bodies now; (2) replace the sampled `count` by **exact piece moments**: the
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
arctangent). The sources:
- [Okabe 1979](https://doi.org/10.1190/1.1440973): closed forms for a homogeneous polyhedron by the
  divergence theorem.
- [Werner & Scheeres 1997](https://doi.org/10.1007/BF00053511): the edge-dyad and face-dyad form used
  for asteroids, compared there with harmonic and mascon models.
- [Tsoulis & Petrović 2001](https://doi.org/10.1190/1.1444944): the singular cases, where the point
  projects onto a face or an edge.
- [Tsoulis 2012](https://doi.org/10.1190/geo2010-0334.1): the full tensor by line integrals.
- [D'Urso 2013](https://doi.org/10.1007/s00190-012-0592-1) and
  [D'Urso 2014](https://doi.org/10.1007/s00190-013-0664-x): treatments of the singularities. I did
  not read these; see §7.

The benchmark writes the Werner–Scheeres sums face by face, splitting each edge's dyad between its two
faces, so a triangle soup with weights can be summed directly:

```text
for each face f (unit outward normal n, any vertex offset r = v − p from the point p):
  h_f   = n·r
  S_f   = Σ_edges e of f  (n_e·r_e) · L_e,   L_e = ln((a+b+ℓ)/(a+b−ℓ)) = log1p(2ℓ/(a+b−ℓ))
  ω_f   = 2·atan2(r₁·(r₂×r₃), |r₁||r₂||r₃| + |r₁| r₂·r₃ + |r₂| r₃·r₁ + |r₃| r₁·r₂)   (Van Oosterom & Strackee)
  U    += ½ ρ h_f (S_f − h_f ω_f)
  g    += −ρ n (S_f − h_f ω_f)
```

Here n_e is the edge's outward normal in the face's plane, a and b are the distances to the edge's
ends, and ℓ is its length.

I could not check the face/edge decomposition against the Werner–Scheeres text (§7). It is checked
numerically instead (`skin_bench.py selftest`):
- **Against Nagy's prism.** A cube matches the prism to 1×10⁻¹⁵ relative near and 1×10⁻¹² at
  10 sides ([Nagy 1966](https://doi.org/10.1190/1.1439779); the repo's `kernel.rs` form).
- **Against the cube's series.** It matches the monopole + l = 4 series to 8×10⁻¹² at 157 sides.
- **Against a numerical gradient.** g matches a central difference of U to 2×10⁻¹⁰, the
  difference's own error.
- **Interior cubes + clipped pieces = the whole body:** to 2–8×10⁻¹⁵ in U and 1×10⁻¹⁴–4×10⁻¹⁴ in
  g, for all four bodies at h = 1/8 m. The volumes agree to 10⁻¹⁵.

**Numerical issues.**
- *Cancellation far away.* The face terms are of size R²·ω ~ (face area) while U ~ V/R, so the
  closed form loses about (R/s)² in relative precision, s being the facet size. Measured: a cube of
  side 1/64 m at 47 m (R/s = 3000) is good to 1.2×10⁻⁹ in U and 6×10⁻⁹ in g; at side 1/128 m, to
  5×10⁻⁹ and 1.3×10⁻⁸. That is about ε(R/s)²/10. [Holstein & Ketteridge
  1996](https://doi.org/10.1190/1.1443964) report the same limit, from the abstract only: rounding
  dominates beyond some distance. They propose a hybrid numerical and analytical treatment for it.
  For Cavendish this means the closed form belongs near and the moments far, the same rule as S3's
  prism and series (`PRISM_WITHIN = 3`). In f32 on the GPU the closed form is unusable beyond a few
  facet sizes. Float-float, or f64 on the CPU, is needed for any far use.
- *Points near a face or an edge.* L_e diverges on an edge's line and ω_f jumps by 4π across a face.
  The robust forms handle these cases ([Tsoulis & Petrović 2001](https://doi.org/10.1190/1.1444944)).
  The [polyhedral-gravity-model](https://doi.org/10.21105/joss.06384) library uses a fixed tolerance,
  1e-14, for its singular-case decisions, and plain double sums with no compensated summation. Arm
  points sit in vacuum tubes, away from any body's face, so the singular cases matter only for the
  oracle's own tests. They should still be handled, and tested on a point placed on a face and on an
  edge.
- *log1p.* Writing L_e = log1p(2ℓ/(a+b−ℓ)) keeps its digits when a + b ≫ ℓ (far). The naive ratio
  loses them.

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
- **Precision floor.** The frac_cen_q errors at 10 m (about 10⁻¹¹ rad) are within a factor of
  about 10 of the closed form's own rounding for pieces of 1/64 m at that range (§1.1). Their orders
  are not to be trusted. Everything else is at least 100× above it.
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
   singular cases handled). This is the validation case with a known answer that S3's voxelised
   meshes lack today: a voxelised mesh's phase converges to the polyhedron's at the measured order.
   - **Pass criteria:** the self-test's checks (prism, series, gradient, pieces sum to the body), and
     a point on a face and on an edge (Tsoulis & Petrović's cases).
   - **For S5's analytic bodies:** a turned box, a prism or a convex hull can be read *directly* by
     the closed form when F is small (≲ 10³ faces) and the point is within R/s ≲ 10³. Beyond that,
     use its exact moments. The two should match where they overlap.
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
  polyhedron's. Both [Okabe 1979](https://doi.org/10.1190/1.1440973) and [Tsoulis
  2012](https://doi.org/10.1190/geo2010-0334.1) treat the prism as a special case (abstracts). Used
  here; 10⁻¹⁵.
- **Asteroid shape models against harmonics and mascons.** [Werner & Scheeres
  1997](https://doi.org/10.1007/BF00053511) compare the polyhedron with harmonic and mascon
  representations of Castalia. They find that harmonic and mascon expressions err badly near the body
  (from the abstract, via snippets). The [polyhedral-gravity-model
  paper](https://doi.org/10.21105/joss.06384) cites Wittick & Russell (2017): mascon models need very
  many mascons to model the surface. I could not read Wittick & Russell.
- **Singular points.** A point on a face, on an edge and at a vertex, against the limit ([Tsoulis &
  Petrović 2001](https://doi.org/10.1190/1.1444944)).
- **Far-field rounding.** Error against distance over size ([Holstein & Ketteridge
  1996](https://doi.org/10.1190/1.1443964)). Measured here as about ε(R/s)²/10.
- **Exact voxelisation.** Pieces sum to the whole: mass, momentum and moments conserved to rounding
  ([Powell & Abel 2015](https://doi.org/10.1016/j.jcp.2015.05.022)). Used here; 10⁻¹⁵.

## 7. What I could not verify

- **Werner & Scheeres.** Springer blocked access to the text. The face and edge form above is
  verified numerically (§1.1), not against the paper's equations. The year is 1997 in Crossref and
  1996 on Springer's page (vol. 65, 313–344).
- **Abstracts only.** Okabe 1979, Tsoulis & Petrović 2001, Tsoulis 2012 and Holstein & Ketteridge
  1996 were read in abstract only (SEG blocks automated fetches). There is no published
  digits-against-distance figure; the ε(R/s)²/10 is my measurement.
- **D'Urso 2013 and 2014.** Bibliographic details only; what they claim was not read.
- **Dyadechko & Shashkov 2008.** The moment-of-fluid order claim is from memory, not checked.
- **Wittick & Russell 2017; Park, Werner & Bhaskaran 2010.** Not accessible; their convergence
  figures are not used.
- **Lattice-point bounds.** I found no source for the first-order flat-face statement or the smooth
  convex case. The aligned cube's measured order 1.0 stands in for the first; S3 already measured
  the sphere's volume at order 2.4.
- **Not measured: clipping cost in compiled code.** The 10⁻⁶–10⁻⁵ s per cell is an estimate. Measure
  it in the slice: a Rust clipper on 10⁶ cells.
- **Not measured: non-convex meshes and analytic curved shapes.** Every benchmark body is convex.
  The method for meshes, signed tetrahedra clipped to cells, is described, not built.
- **Not measured: the 20 m column, at the 10⁻¹¹ rad floor.** The JSON files hold every distance
  from 1 to 20 m; the tables show 1.5 m and 10 m.

## 8. Further questions (not chased)

- Third moments per boundary leaf: the walk carries them per node. Would exact third moments make
  the order 5, and is it worth 10 more numbers?
- Can the double-layer cancellation of §2.3 be made exact on aligned faces? For example, a fixed
  sub-cell jitter of the lattice per body, at no storage cost, would restore equidistribution.
- Quantisation: how many bits of fill and centroid keep the error under PHASE_TOLERANCE = 10⁻¹⁰
  rad (`forward.rs`) for a 10⁹-cell body?
- On the GPU, should the near exact-piece kernel be f64, or float-float, given the (R/s)²
  cancellation?
- For terrain from height fields: exact pieces of a bilinear surface, or of two triangles per
  height-field quad. The latter is polyhedral, so this method applies directly.
