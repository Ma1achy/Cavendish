# S6: density as a function, and skinned bodies (TASK-S6-07, TASK-S6-08)

Written 28 Nov 2026 from the owner's third paper batch, for the briefs of TASK-S6-07 (density as a
function of position) and TASK-S6-08 (skinned deformation by a cage of tetrahedra). Every claim names
its source with an equation, section or page. Where I measured something, the script and its output
are in [`s6-density-and-skinning/`](s6-density-and-skinning/) (standard library only, `python -I`).
DOIs were checked against Crossref on 28 Nov 2026 unless the reading list says otherwise.

## The answer in one paragraph

**S6-07.** Read every density function the same way: as exact per-cell **moments** (mass, centre,
second and third central moments, which `cav_core::voxel::tree::Moments` already holds). For a
polynomial density they come in closed form, by Gauss–Legendre quadrature of the right order or by the
divergence theorem on cut cells. For improved Perlin noise and value noise the moments are *also*
exact, because inside one lattice cell those noises are polynomials (degree ≤ 6 per axis, measured in
B1). A 5-point Gauss–Legendre rule per axis on boxes split at the noise lattice's planes integrates
mass to third moments to rounding. Everything else (sums, products, sampled textures, simplex noise,
arbitrary closures) gets tensor Gauss–Legendre with the DLMF remainder bound, a split of cells at known
kinks, and an a-posteriori check (n against n+2) that the bound holds. The closed forms for
polynomial-density *polyhedra* (D'Urso 2014 for linear density with the full tensor; Ren et al. 2018 to
cubic with the tensor; D'Urso & Trotta 2017 to cubic, `g_z` only) are near-field kernels and oracles.
Their rounding error grows as `ε·(R/s)^κ` with κ ≈ 4.3–5.3 (Ren et al. 2018 Table 4), and Pohánka
(1998, p. 404) gives the same warning in words. So beyond a short distance the moments must take over,
which is where the tree's opening rule already hands over. Noise and hashes are deterministic if they
use only integer hashing and `+ − × floor`, evaluated once on the CPU in `f64` when the body is built.
A golden digest (B2) pins that.

**S6-08.** Tetrahedralise the body once, in the rest pose. For S6-08, use structured Kuhn-split cages
that Cavendish builds itself, so no external mesher is needed for any oracle. Fix each element's mass
at bind time, and classify elements at bind time as *rigid* (all four vertices weighted exactly 1 to
one bone) or *blended*. Rigid elements are read through that bone's per-bone tree, which is the only
way the "rigid limit bit for bit" oracle can hold for dual quaternions: DQS changes the bits of 97 %
of points even with a single unit weight (K1). Make linear blend skinning the default, because glTF
defines its skinning as LBS. Offer DQS (Kavan et al. 2008, Eq. 11, Algorithms 1–2) per body. Check
`det A` of every blended element every pose, and treat `≤ 0` as an error. Both blends invert elements
inside a tight bend (K1). Import glTF in `f64` with weights renormalised and the skinned mesh's node
transform ignored, as the specification requires. Interpolate rotations by nlerp, so they are
bit-reproducible. fTetWild (MPL-2.0) and TetGen (AGPL or commercial) come later, for imported meshes,
after the owner rules on licences. The piece that joins the two tasks is §4. A tetrahedron whose four vertices are moved by *any* skinning is
mapped exactly by one affine map. Its mass is fixed by construction, and its moments in the posed
frame are the rest moments transported by that map. This is exact for any polynomial rest density,
whatever the volume change (measured exactly in rationals, B4). So the far field of a skinned body is
exact per tetrahedron, and only the near field needs the polyhedral closed form.

---

## 1. Closed forms for polyhedra with polynomial density, and their numerical stability

### 1.1 What each source gives

| Source | Density | Output | Form | Validation it reports |
|---|---|---|---|---|
| [Pohánka 1998](https://doi.org/10.1046/j.1365-2478.1998.960335.x), *Geophys. Prospect.* 46, 391–404 | linear, `ρ(r) = ρ₀ + ρ₁·r` (Eq. 3) | gravity vector `E(r)` | Gauss's theorem twice: volume → faces (Eq. 8–9) → edge line integrals (Eq. 12–13, 31–33); final formula Eq. (52) with the functions of Eq. (54), (56); "optimum" algorithm, p. 402–404 | none numerical in the paper; it states the formula is "valid for every point of space" and "needs no special attention for the points near to or on the surface" (Concluding remarks, p. 404) |
| [D'Urso 2014](https://doi.org/10.1007/s10569-014-9578-z), *Celest. Mech. Dyn. Astron.* 120, 349–372 | linear, `o + g·r` | potential, gravity **and the gradient tensor** (§2–4; computational forms Eq. 51, 54, 60 in §5.2) | distribution theory; the linear-density field is the constant-density one scaled by the density at `P`, plus a correction: `d_pU = G[(o + g·p) d_pU_c + …]` (Eq. 28), tensor Eq. (43)–(47) | §6.1: a prism (Tsoulis 2012's) against the approaches of Pohánka 1998, Holstein 2003 and Hamayun et al. 2009, agreement stated but not tabulated; §6.2: asteroid 433 Eros models with 1,708 to 22,540 faces, checked to tend to the uniform case as `|g| → 0` and against Poisson's equation inside, `ΣU_ii = −4πGρ`: −22.388 against −22.377 ×10⁻⁷ s⁻² (Eq. 90, p. 370) |
| [D'Urso & Trotta 2017](https://doi.org/10.1007/s10712-017-9411-9), *Surv. Geophys.* 38, 781–832 | trivariate polynomial `Σ c_ijk xⁱ yʲ zᵏ` up to cubic in each (Eq. 3–4); "easily extended" to higher degree (abstract, §6) | **vertical component `Δg_z` only** (Eq. 2; "the procedure … can be equally applied to all components", p. 785) | Gauss's theorem through rank-m tensors (Eq. 17–18), face integrals → 1-D → algebraic in vertex coordinates (§2.3–2.6); an arbitrary observation point by `r = s − p` and re-expanding the polynomial about `P` (Eq. 120–126, §3) | §5: the García-Abdeslem 2005 prism `[10,20]×[10,20]×[0,8]` km with the cubic depth law of Eq. (162). **Table 1 (p. 816) prints the analytical values to 15 digits**, at y = 10, 11, 12.5, 15 km and x = 0…30 km, z = 0, against Matlab adaptive quadrature, which fails (NaN) or keeps only three digits over the top face (p. 818). They also report that García-Abdeslem 2005's Fig. 3 was computed with wrong integration limits (p. 814–815). |
| [Ren et al. 2018](https://doi.org/10.1007/s10712-018-9467-1), *Surv. Geophys.* 39, 901–935 (held; read 28 Nov for this note's question only) | polynomial up to cubic, horizontal and vertical | **gradient tensor** | volume → faces → edges with identities that lower the polynomial order (Introduction) | a depth-graded prism against García-Abdeslem 2005; an irregular polyhedron with linear density; a tetrahedron with horizontal and vertical cubic density. The trace (Laplace) is checked outside: relative trace error 2.74×10⁻¹³ % above the top face, 3.77×10⁻¹¹ % above a corner (§3, Table 2). **A measured far-field law** (§3.4, Eq. 25, Table 4), below. |
| [García-Abdeslem 1992](https://doi.org/10.1190/1.1443261), *Geophysics* 57, 470–473 | any `ρ(z)` on a right rectangular prism | `g_z` | analytic in `x, y` (Eq. 3, 10), Gauss–Legendre in `z` (Eq. 11–13) | the constant-density prism against Banerjee & Das Gupta 1977: max difference below 10⁻⁶ mGal with n = 10 (p. 471). The singular end point is handled by Gauss nodes lying inside the interval (p. 471). |

[Martins et al. 2010](https://doi.org/10.1190/1.3380225), *Geophysics* 75, I21–I28, was supplied as
`10.1190@1.2122413.pdf`. It is an inversion paper. It uses a rational ("parabolic") depth law
`Δρ(z) = Δρ₀³/(Δρ₀ − αz)²` (Eq. 1) on vertical prisms, forward-modelled with Chakravarthi et al.
2002's closed form (Eq. 3). For S6-07 it serves only as an example of a geological density law that
is not a polynomial, so it goes to quadrature (§2). **The DOI in the file name, 10.1190/1.2122413, is
García-Abdeslem 2005** ("…density varying with depth following a cubic polynomial", *Geophysics* 70,
J39–J42; Crossref). That is the benchmark D'Urso & Trotta and Ren et al. both use, and it was not in
the batch. It is now on `papers-to-fetch.md`.

### 1.2 Numerical stability: the far field is the problem, and it is known

- **Pohánka 1998, Concluding remarks (p. 404):** "in contrast to the formula for a polyhedral body of
  uniform density, for the points very far from the body, a substantial numerical error can arise …
  these terms behave, for large distances from the body, as second-order polynomials of the
  coordinates, compared with first-order polynomials in the case of uniform density. Therefore … the
  higher-precision expression of real numbers must be used." He changes one function (Eq. 55) "in
  order to decrease the error at points far away from the body", but does not remove the growth.
- **Ren et al. 2018 §3.4:** the relative error grows as `ε = ε₁ λ^{−κ}`, where `λ⁻¹` is the
  dimensionless target distance (Eq. 25, after Holstein & Ketteridge 1996 and Holstein 2003). For
  linear density they measure **κ ≈ 4.31** for `T_xx, T_yy, T_zz, T_xy` and **≈ 5.28** for `T_xz,
  T_yz`. This agrees with Holstein 2003's 4 and 5. Total loss of precision comes at 5×10³ to 7×10⁴
  body sizes (Table 4; `paper-benchmarks.md` records the same numbers).
  At `f64`, 10⁻¹⁶·(R/s)^4.3 is 2×10⁻¹¹ at `R/s = 10`, 4×10⁻⁷ at 100 and 10⁻² at 1000. The uniform
  kernel in `cav_core::polyhedral` grows as `ε(R/s)²` by its rounding rules
  ([`exact-skin-and-partial-fill.md`](exact-skin-and-partial-fill.md) §1.1).
- **D'Urso & Trotta 2017 §3** re-expand the polynomial about the observation point (Eq. 124–126:
  `s s s = (r+p)(r+p)(r+p)`). Each monomial of degree k then brings terms of size `|p|^k` that cancel.
  This is the same mechanism as Pohánka's, and it gets worse with degree.
- **What the about-P split itself costs (B3).** D'Urso 2014's Eq. (28) splits the field into the
  density at `P` times the uniform field, plus a correction. I measured that split alone, with
  quadrature in place of the closed form, so that only the split's own cancellation shows. It costs
  `ε·|∇ρ|R/ρ̄`, which is 2×10⁻¹³ relative at `R/s = 10⁵` (B3). **The growth Ren et al. measure
  therefore comes from the closed forms' face and edge terms, not from the density split.** A
  frame-shift trick will not cure it.

**Decision.** Use a polynomial-density polyhedral closed form only in the near field, where the
tree's opening rule already uses the exact kernel. The far field uses the exact moments (§2.1, §4),
which have no such growth. With the κ ≈ 5.3 worst case, keeping the closed-form relative error below
10⁻⁹ needs `R/s ≲ 30`. Measure the hand-over distance in the slice: run the near kernel against the
moments, with a pass criterion written first. Do not take it from this estimate.

**Which closed form.** The gradiometer needs the tensor and the potential along the atoms' paths
(plan §6 and `paper-flight-phase.md`), so `Δg_z`-only D'Urso & Trotta cannot be the kernel. Use
**D'Urso 2014** for linear density (potential, vector and tensor, singularities handled: the tensor is
singular only for a point on an edge's line, §7) and **Ren et al. 2018** for quadratic and cubic. Use
**D'Urso & Trotta 2017 Table 1** as a 15-digit oracle for `g_z`. Linear density covers a graded layer
and the polynomial density of a skinned tetrahedron (§4). Quadratic and cubic are rarer. A cheaper
alternative for them in the near field is to split the tetrahedron and apply the linear form with
moment correction. Measure that against Ren et al. before writing a cubic kernel.

### 1.3 Oracles with a known answer (for S6-07's list)

1. **Linearly graded sphere**, `ρ = ρ₀ + k z`, radius `a`. Outside, the field is exactly that of a
   point mass `M = 4πa³ρ₀/3` plus a point dipole `p_z = 4πka⁵/15`. The density has only `l = 0, 1`
   angular content, so all higher multipoles vanish. Quadrature agrees to 10⁻¹⁶–3×10⁻¹⁵ relative in
   `g` and `G_zz` (B5). The **radially graded sphere** (`ρ(r)`) is a point mass outside and
   `G M(r)/r²` inside. The infinite **graded slab** gives `g_z = 2πG∫ρ dz` and zero gradient. A
   finite slab needs the D'Urso & Trotta case below.
2. **D'Urso & Trotta 2017 Table 1** (p. 816): the cubic depth-law prism, 15 digits, including points
   on the top face, where adaptive quadrature fails. It tests a `g_z` read of a voxelised or polyhedral
   graded prism. The density is in kg/m³ with `z` in km (Eq. 162; G = 6.67259×10⁻¹¹, §5). Read Table 1
   from the page image when building the fixture: the text extraction interleaves its columns.
3. **Poisson and Laplace traces**: `ΣT_ii = 0` outside and `−4πGρ(P)` inside, with the *local*
   density at `P`. This is a free check of a graded kernel (D'Urso 2014 Eq. 90; Ren et al. 2018
   Table 2).
4. **The uniform limit**: as `|∇ρ| → 0` a graded kernel must tend to the uniform one (D'Urso 2014
   §6, Tables 1–2). Use it as the control arm against a sign or scaling error in the gradient term.
5. **Exact moments of a polynomial body** in rationals (B4's method), against the voxel tree's summed
   cell moments.

## 2. Quadrature for general density functions, with error bounds

### 2.1 Read moments, not fields

The voxel forward model reads each cell through its mass moments, up to third order
(`Moments::{mass, centre, second, third}`, `crates/cav-core/src/voxel/tree.rs`). The density function
enters only through `∫_cell ρ(x) x^α dV` for `|α| ≤ 3`, computed once when the body is built, in the
body's frame. So the quadrature question is about **moment integrals over boxes and cut cells**, with
smooth integrands. It is not about singular field integrals, which stay with the near kernel.

### 2.2 Polynomial densities: exact

An n-point Gauss–Legendre rule is exact for polynomials of degree ≤ 2n−1
([DLMF §3.5(v)](https://dlmf.nist.gov/3.5#v)). A density of degree d per axis with moments to order 3
needs **n = ⌈(d + 4)/2⌉ per axis**: n = 2 for uniform density, 3 for linear, 4 for cubic. On a cut
cell the moments are exact in closed form by the divergence theorem over the clipped polyhedron
([Mirtich 1996](https://doi.org/10.1080/10867651.1996.10487458), held). The existing exact-boundary
code computes these for uniform density. A polynomial density adds monomial weights to the same face
integrals (Mirtich's projection integrals carry any monomial). Alternatively, split the clipped
polyhedron into tetrahedra and use the barycentric formula
`∫_T λ₁^a λ₂^b λ₃^c λ₄^d dV = 6V·a!b!c!d!/(a+b+c+d+3)!`, which B4 checks exactly.

### 2.3 Improved noise and value noise: exact on the lattice

Improved noise ([Perlin 2002](https://doi.org/10.1145/566654.566636), *ACM TOG* 21(3), 681–682) is
built from the gradients `g_ijk`, the linear functions `g·(x−i, y−j, z−k)` at the cell's eight
corners, and the interpolant `6t⁵ − 15t⁴ + 10t³` (§2–3). Inside one lattice cell it is therefore a
**polynomial of degree ≤ 6 in each coordinate**. Value noise with the same fade is of degree ≤ 5.

Measured (B1, one lattice cell, mass −0.2143):

| n per axis | mass error | `x²`-moment error |
|---|---|---|
| 3 | 3.2×10⁻³ | 2.7×10⁻⁴ |
| 4 | 1.4×10⁻¹⁶ | 3.5×10⁻¹⁸ |
| 5–7 | ≤ 3×10⁻¹⁷ | ≤ 1.4×10⁻¹⁷ |

n = 4 is exact for the mass, as predicted, and n = 3 is not (the control arm). The `x²`-moment is also
exact at n = 4 in this test, because the moment is centred on the cell and the degree-8 term
integrates to zero by symmetry. That is a property of the test, so keep **n = 5 for moments to third
order** (degree 6 + 3 = 9 = 2·5 − 1).

A box that straddles lattice planes, integrated without splitting, converges only slowly: the error is
2.4×10⁻³ at n = 4 and 9.5×10⁻⁶ at n = 10. The fade's sixth derivative jumps at the lattice planes.
**Split at the lattice planes and it is exact again**: n = 4 and n = 6 agree to 8×10⁻¹⁷.
At a noise wavelength `λ` and cell size `h`, a cell is cut by at most `⌈h/λ⌉ + 1` planes per axis.
For `h ≫ λ` (noise finer than the cell), exact integration costs `(h/λ)³` sub-boxes × 125 points.
That is a choice, not a bug: noise finer than a cell mostly averages out, and the moments say by
how much.

(An aside on measuring: the first cell I chose had a cell mass of exactly zero, which made its mass
test vacuous. Other cells give multiples of 1/14, so I moved the test to one with mass −3/14.)

**Simplex noise is not like this.** Each simplex corner contributes `max(0, r₀² − |d|²)⁴ (g·d)`, so
the noise has kinks on spheres around the lattice points, not on planes. It is piecewise polynomial on
regions that boxes cannot follow, so it goes to §2.4. I have not read Perlin's or Gustavson's simplex
noise papers (not supplied); the form above is from general knowledge and must be checked against
the source before it is cited in code.

### 2.4 Everything else: tensor Gauss–Legendre with a bound and a check

For a smooth integrand on `[x₀, x₀ + h]`, the n-point remainder is
`E_n = h^{2n+1} (n!)⁴ / ((2n+1)[(2n)!]³) · f^{(2n)}(ξ)` ([DLMF 3.5.19](https://dlmf.nist.gov/3.5.E19)
with γ_n of 3.5.21, mapped to the interval). García-Abdeslem 1992 uses the same rule and cites
Davis & Polonsky for it (Eq. 11–12, p. 471).

- For a density with a known **smallest wavelength λ**, take `|f^{(2n)}| ≤ A(2π/λ)^{2n}`. This holds
  for a sinusoid exactly, and by Bernstein's inequality for any band-limited field. The bound is then
  explicit. B6 checks it on `1 + 0.5 sin(2πx/λ)`: the observed error stays between 0.6 and 0.985 of
  the bound in every case, for `h/λ` from 1/8 to 1 and n = 1 to 4, so the bound is tight. At
  `h/λ = 1/8`, n = 4 gives 5×10⁻¹².
- For **sampled 3-D textures** (trilinear), the density is a polynomial of degree 1 per axis inside
  each texel. Split at texel planes, as for noise: n = 3 is exact for moments to third order
  (1 + 3 = 4 ≤ 5).
- For **piecewise** densities with known breakpoints, split at the breakpoints. Each piece is then
  smooth (or polynomial).
- For **sums** of these, integrate each term on its own split. **Products** of two lattice noises are
  polynomial on the common refinement of both lattices (degree adds), so they stay exact.
- For **arbitrary closures** (unknown smoothness), do the a-posteriori check: compute with n and n+2
  on the cell, and accept when `|Q_n − Q_{n+2}|` is below the cell's share of the budget. Otherwise
  bisect the cell, at most to a stated depth, and record the cells that hit the depth. This is the
  adaptive Gauss–Legendre practice of tesseroid codes
  ([Uieda et al. 2016](https://doi.org/10.1190/geo2015-0204.1), held, not re-read for this note).

**From a mass error to radians.** An error `δm` in a cell's mass, at distance `R` from a tower,
perturbs `G_zz` by about `2Gδm/R³`. The phase budget in radians is the existing error budget's
business ([`paper-error-budget.md`](paper-error-budget.md)). The quadrature's job is to make `δm/m`,
and the moment errors, small and *reported*. Record the worst cell's estimated relative error in the
bundle, as plan §1.3 asks.

**Quadrature control arm (for the task's oracle list).** Run the n−1 rule on a polynomial density of
degree 2n. It must miss by more than a stated amount, as n = 3 misses the noise in B1. That proves the
test can fail.

## 3. Determinism of noise and hash functions

- **Perlin 2002, §5:** once the random gradient table is replaced by the fixed 12 directions, "the only
  pseudo-random component left is the ordering of the permutation table P. Once a standard permutation
  order is determined, it will at last be possible to give a uniform mathematical definition for the
  Noise function, identical across all software and hardware environments." So the noise is
  deterministic given (a) the hash or permutation, (b) the operations, (c) their order.
- **Use an integer hash, not a permutation table.** A 64-bit mix of the lattice coordinates (B2 uses
  the splitmix64 finaliser) has no table to ship and no 256-period. It is exact on every platform,
  because integer arithmetic is. The hash selects the gradient by `mod 12`, as Perlin selects by
  `P mod 12` (§3).
- **Floating point:** improved noise needs only `floor`, `+`, `−`, `×` (fade, lerp, the dot products
  with 0/±1 gradients), which are correctly rounded under IEEE 754. The fade is no transcendental.
  The risks are (i) a compiler contracting `a*b + c` into a fused multiply-add, and (ii) a different
  evaluation order. Rust does not contract floating-point expressions unless `mul_add` is called. I
  did not find a primary source to cite for that in this session; the golden test below settles it in
  practice. On the GPU, WGSL does not promise the same, and S17's web-equivalence note lists only the
  transcendental functions as differing. **Evaluate density functions on the CPU in `f64`, once, at
  body build time.** Their moments are what the GPU reads, so GPU arithmetic never touches the noise.
- **Coordinates:** evaluate in the body's frame, in metres times a stated frequency, in `f64`, so a
  body's density does not change when it moves. `floor` of a negative coordinate must give the lower
  integer (Rust's `f64::floor` does), and the cast to the hash's integer type must be explicit, not a
  saturating `as` on values that could exceed `i64`.
- **Golden test (B2):** SHA-256 of 4096 noise values at stated points is
  `716059dd…b303319e` (full digest in `density_bench.out.txt`). A Rust port of `noise()` in
  `density_bench.py` must reproduce it bit for bit on Windows and Linux. That is the cross-platform
  test, and its control arm is any one-ulp change (for example evaluating the fade as
  `6t⁵ − 15t⁴ + 10t³` term by term instead of Horner's form), which changes the digest.

## 4. Per-tetrahedron mass with exact moments under affine maps

This is the bridge between S6-07 and S6-08.

- **Any skinning maps a tetrahedron affinely.** Four vertex images, however they were computed (linear
  blend, dual quaternion, or a per-vertex solve), define exactly one affine map `x = A X + t` from the
  rest tetrahedron to the posed one, when the images are not coplanar. Its edges stay straight, so the
  posed tetrahedron is exactly a tetrahedron: there is no approximation in "each deformed tetrahedron
  is a polyhedron".
- **Mass is fixed; density follows.** Store each tetrahedron's mass `m`, and its rest density
  `ρ₀(X)` if graded, never a posed density. The posed density is `ρ(x) = ρ₀(A⁻¹(x − t)) / det A`. It is
  still a polynomial of the same degree in `x`, so §1's linear or cubic closed forms apply to the posed
  tetrahedron in the near field.
- **Moments transport exactly**: `m' = m`; first moment `A c + m t`; second (about the origin)
  `A S Aᵀ + (Ac)tᵀ + t(Ac)ᵀ + m t tᵀ`; third likewise by the binomial expansion. These depend on `A`
  and `t` but **not on `det A`**, because the `1/det A` in the density cancels the Jacobian. Measured
  exactly in rationals for a linearly graded tetrahedron under a map with `det A = 0.79` (21 % volume
  loss), mass, first and second moments are exactly equal (B4). The control arm, dropping the `1/det
  A`, breaks all three.
- **For a uniform tetrahedron** the second moment is `(m/20)(Σᵢ vᵢvᵢᵀ + s sᵀ)` with `s = Σᵢ vᵢ`
  (checked exactly, B4). It is a function of the posed vertices alone, with no division by the posed
  volume.
- **Consequences for S6-08's oracles:** "mass constant through every pose" holds by construction (the
  stored `m` is summed). "The rigid limit equals per-bone trees bit for bit" needs the posed vertices
  to be bitwise the rigid transform's output, which is §5's question. The far field of a skinned body
  is exact per tetrahedron, so "a bent arm converges as the cage refines" tests only the near field and
  the cage's geometric approximation of the skin.
- **Inversion:** `det A ≤ 0` means a tetrahedron turned inside out. The moments stay finite and
  "correct" for a body that no longer exists, so they will not flag it. Check the sign of each posed
  tetrahedron's volume every pose, and report and count inverted ones as an error, not a warning.

## 5. Skinned deformation (TASK-S6-08)

The task: a body embedded in a cage of tetrahedra, each tetrahedron carrying a fixed mass; the cage's vertices
driven by a skeleton and skin weights (linear blend and dual-quaternion); each deformed tetrahedron read by its
exact polyhedral field near and its affine-mapped moments far; glTF import; oracles "the rigid limit equals per-bone
trees bit for bit", "mass constant through every pose", "a bent arm converges as the cage refines"
(`plan/tasks.yaml`, TASK-S6-08).

What the skinning side hands the moments side, per pose: the four deformed vertex positions `x'₀…x'₃` of every
tetrahedron, in the body frame, in `f64`. Because a linear tetrahedron's deformation is exactly the affine map that
carries its rest vertices to its posed ones, `A = D′D⁻¹` with `D = [x₁−x₀, x₂−x₀, x₃−x₀]` (and translation
`x'₀ − A x₀`), the moments side needs nothing else. `det A = V′/V`; `det A ≤ 0` is an inverted (or degenerate)
element and is an error to report, not to read (below). The mass `m` of each tetrahedron is fixed at bind time;
its density after skinning is `m / V′`.

### Linear blend skinning (LBS)

- **Definition.** `v′ = Σᵢ wᵢ C_{jᵢ} v`, with `C_j ∈ SE(3)` the joint's rest-to-posed transform and convex weights
  (`Σ wᵢ = 1`, `wᵢ ≥ 0`, usually at most four influences): [Kavan et al. 2008](https://doi.org/10.1145/1409625.1409627),
  §2.2, Eq. (1), p. 105:4. Distributivity gives the same thing as one blended matrix applied to `v`, Eq. (2), p. 105:4;
  that blended matrix "is not necessarily a rigid transformation, even if all `C_j` are rigid" — "undesired scaling
  creeps into our transformation matrix", and in the extreme case it is rank-deficient (p. 105:4).
- **Candy wrapper and collapse.** Two joints, `w₁ = w₂ = ½`, the second twisted by 180° about the bone: the vertex
  lands exactly on the joint axis, "the skin collapses to a single point" (Kavan §2.2, p. 105:4, Fig. 4; realistic
  example Fig. 14 left). Le & Hodgins name the two LBS artefacts "elbow collapse" at bent joints and "candy wrapper"
  at twisted joints, both "due to the volume loss caused by the linear blending" ([Le & Hodgins 2016](https://doi.org/10.1145/2897824.2925959),
  §1, p. 1), and attribute them to the shearing transformations the linear blend produces (§4, p. 6).
- **Origin.** The skeleton-driven skin of the 1988 Graphics Interface paper usually cited as LBS's origin
  ([Magnenat-Thalmann, Laperrière & Thalmann 1988](https://doi.org/10.20380/GI1988.04), pp. 26–33) is *not* the
  weighted-matrix formula: it maps each vertex to its segment by projection, a ratio along the segment and
  normal interpolation between the joints' normals ("Joint-dependent Local Deformation" operators, §3–5, pp. 27–30,
  Figs. 5 and 9), plus ad hoc rounding and muscle-inflation terms driven by a flexion ratio (§6.1–6.2, p. 30). It
  is the historical precedent, not a source of equations for Cavendish; cite Kavan et al. for the formula.
- **glTF skinning is LBS by definition**: "glTF 2.0 mesh primitives support Linear Blend Skinning via skin objects,
  joint hierarchies, and designated vertex attributes" (glTF 2.0 specification, *Skins › Overview*), and the
  per-vertex matrix is `Σ Wᵢ · M_{Jᵢ} · B_{Jᵢ}` (*Instantiation › Skinned Mesh Primitives*). An imported asset's
  weights were authored and checked against LBS; DQS on the same weights is a different deformation.
- **Cost.** One 3×4 matrix (12 scalars) per joint, a weighted sum per vertex (Kavan §5, p. 105:15).

### Dual-quaternion skinning (DQS)

- **Definition (DLB).** `DLB(w; q̂₁…q̂ₙ) = (Σ wᵢ q̂ᵢ) / ‖Σ wᵢ q̂ᵢ‖` for unit dual quaternions `q̂ᵢ` (Kavan §3.2,
  Eq. (11), p. 105:9). The result is always a rigid transformation, so no candy wrapper (§2.2, p. 105:4; Fig. 14).
  DLB is bi-invariant (independent of the coordinate frame), Lemma 2, §3.3, p. 105:10. It is an approximation of the
  exact screw interpolation (ScLERP): rotation angles differ by less than 8.15° and translation along the screw axis
  by less than 15.1% of the relative translation, as upper bounds for two transforms (§3.4, pp. 105:10–11); for
  `n > 2` there is no simple bound (p. 105:11).
- **Normalisation in practice.** Algorithm 1 (§4, p. 105:12): blend `b̂ = Σ wᵢ q̂ᵢ`, then `c₀ = b₀/‖b₀‖`,
  `c_ε = b_ε/‖b₀‖`, and transform `v′ = v + 2d₀ × (d₀ × v + a₀v) + 2(a₀d − a d₀ + d₀ × d)` with Eq. (15); the
  scalar part of the dual-part correction is dropped because it cancels (pp. 105:11–12). Only `+ − × ÷ √`: no
  transcendental functions per vertex. Weight sums that are not exactly one cancel in the division (scale
  invariance of Eq. (11)), unlike LBS.
- **Antipodality.** `q̂` and `−q̂` are the same transform but blend differently (§4.1, p. 105:12, Fig. 22).
  Algorithm 2 flips each `q̂ᵢ` to a non-negative dot product (of the non-dual parts) with the first; Lemma 5 proves
  it finds a valid sign assignment whenever one exists (pp. 105:12–13). The cheaper alternative precomputes signs
  per joint against the parent, assuming no joint turns more than 180° relative to its parent (p. 105:13); not
  guaranteed, but "we have not encountered any problems with our test data". Le & Hodgins resolve per vertex the
  same way (their Algorithm 1, line 5, p. 5).
- **Flipping.** The shortest-path property makes the skin jump when a joint passes 180° (Kavan §6, p. 105:16,
  Fig. 20): a discontinuity in the mass distribution in time. Joint limits avoid it.
- **Non-rigid joints.** Dual quaternions cannot carry scale or shear; the remedy is two-phase skinning, scale by a
  linear blend in the rest pose, then rigid DQS (§4.2, pp. 105:13–14, Figs. 12–13), at 20 scalars per joint against
  12 for LBS and 8 for plain DQS, and about twice LBS's vertex-shader instructions (29 extra for `n = 4`), p. 105:15.
  glTF node transforms may carry a (non-uniform) `scale` (glTF *Transformations*; a `matrix` must decompose to TRS
  with no shear), so an imported rig can need this.
- **Cost.** "Slightly slower than both linear and direct quaternion blending", more than twice as fast as log-matrix
  and spherical blending (Kavan §5, p. 105:14; their timings are Fig. 18–19, images not readable as text). For
  Cavendish this is immaterial: cage vertices number far fewer than the field evaluations per tetrahedron.
- **The bulging artefact.** DQS "introduces a joint bulging artifact (at bent joints)" because "all vertices near a
  joint can only rotate around a single CoR (the joint)" (Le & Hodgins §1, p. 1, and §4, p. 6, citing Kim & Han 2014;
  Figs. 2, 6, 8: DQS cannot form a crease inside a bent knee).

**Is the rigid limit exact?** In exact arithmetic, yes for both: with one weight equal to one, LBS returns
`C_j v` and DQS returns the rigid transform of `q̂_j`. In floating point, measured (K1 Part C, 10⁵ random points
and 10³ random poses in `f64`, `docs/research/s6-density-and-skinning/k1_skin_volume.py`, output `k1.out`):

| path, weights exactly (0, 1) | points that differ in any bit from `R p + t` |
|---|---|
| LBS as `Σ wᵢ (Rᵢ p + tᵢ)` | 0 of 100,000 |
| DQS, Kavan Algorithm 1 | 96,900 of 100,000 (max 2.2×10⁻¹⁵ m for `‖p‖ ≤ 1.7 m`) |

The LBS result is bit-exact only because `0·x = 0` and `x + 0 = x` and the code applies the same `R`, `t` in the same
order as the direct path; blending the *matrices* first (glTF's `Σ Wᵢ M B` form) changes the rounding again. DQS
re-normalises a quaternion that is unit only to rounding, and rotates by Eq. (15), not by a matrix, so it is never
bit-exact. **What a builder must do for the bit-for-bit oracle:** classify at bind time. A tetrahedron all of whose
four vertices have a single weight of exactly one on the same bone is *rigid*: it is never skinned; it belongs to
that bone's per-bone tree, read in the bone's rest frame with the detector mapped by the bone's inverse transform,
exactly the path per-bone trees already take. Only the remaining (*blended*) tetrahedra are skinned per pose. Then
the oracle "a skinned body with every weight in {0, 1} reads bit for bit as per-bone trees" holds by construction,
its control arm is a body with one vertex weight set to 0.999…, and the cost of a pose scales with the blended
region only.

### Centres of rotation (Le & Hodgins 2016)

Per-vertex centre of rotation `p_i`, a similarity-weighted average of rest-pose positions over the surface with
similar skinning weights (Eq. (1)–(3c), pp. 3–4); it depends only on the rest pose and weights, so it is
precomputed (p. 4). At run time: quaternion-blend the rotation (QLERP, with the same antipodal flip), LBS-blend
the transform of `p_i` to get the translation, `v′ = R v + t` (Algorithm 1, p. 5). It removes the candy wrapper for
vertices influenced by two bones and reduces DQS bulging; for vertices influenced by more than two bones "our method
still exhibits a slight volume loss" because the CoRs form a 3-D region rather than a curve (§4, p. 6). Precompute:
0.7–18 s for 2,601–39,158-vertex models on an eight-core 2.4 GHz CPU (Table 1, p. 5); naive per-triangle
integration "can take up to an hour" (p. 4). Limitations: weights must be good, CoRs must be recomputed when weights
change, no collisions (§5, p. 6). **Not for S6-08's first cut** (the task names LBS and DQS); worth an `IDEAS.md`
line as the third blend, with K1 as its test.

### Bounded biharmonic weights, for bodies with no weights

BBW minimise the Laplacian energy `Σⱼ ½∫ ‖Δwⱼ‖² dV` subject to interpolating the handles, partition of unity and
`0 ≤ wⱼ ≤ 1` ([Jacobson et al. 2011](https://doi.org/10.1145/2010324.1964973), Eq. (2)–(6), p. 2). Discretised with
linear FEM on a *TetGen tetrahedral mesh of the volume* that contains bone sample points (§3.4, p. 4), solved as a
sparse QP with MOSEK. Bind time is the cost: e.g. Armadillo, 112,355 elements, 8.4 s per handle; the fast variant
drops partition of unity, solves each handle alone and normalises, 50× faster on the Gargoyle with mean weight
difference ~10⁻³ (Fig. 6, Table 2, pp. 5–6). They are local and smooth but "do not have the linear precision
property" (p. 6). Relevance: (i) the tet mesh BBW live on is the same kind of mesh as the cage, so BBW are the
sourced answer for a body that arrives with a skeleton but no weights; (ii) they need a QP solver (MOSEK is
commercial). **Not for S6-08's first cut**: imported glTF assets carry weights, and synthetic test bodies can use
an analytic ramp. Interior cage vertices still need weights, which glTF does not give (it weights surface vertices
only): my proposal, not from these papers, is the harmonic extension of the surface weights into the cage (one
sparse Laplace solve per bone with the surface values as Dirichlet data, then clamp to [0, 1] and renormalise),
which is cheaper than BBW and agrees with the asset's own weights on the surface. Flag it in the brief as a choice.

### Volume change and what it does to the mass

Neither blend preserves volume. With mass fixed per tetrahedron this is a density change, `ρ′ = m/V′`, and the
total mass is still exact. K1 Part A (bar 0.1 × 0.1 × 0.6 m, joint at mid-length, smoothstep weights over 0.1 m,
13,824 Kuhn tetrahedra):

| pose | LBS ΔV/V | DQS ΔV/V | LBS det A range | DQS det A range |
|---|---|---|---|---|
| bend 45° | −0.67% | −0.048% | 0.33 … 1.41 | 0.41 … 1.58 |
| bend 90° | −2.3% | −0.19% | −0.23 … 1.58 | −0.23 … 2.19 |
| bend 170° | −4.6% | −0.74% | −0.12 … 1.33 | −1.62 … 3.42 |
| twist 90° | −4.4% | −0.13% | 0.50 … 1.00 | 0.95 … 1.00 |
| twist 180° | −8.8% | −0.56% | 0 … 1.00 (collapse) | 0.76 … 1.00 |

Readings. (1) LBS's total volume loss is ten to thirty times DQS's, matching both papers; the candy wrapper shows as
`det A = 0` on the axis at 180° twist. (2) **Both blends invert tetrahedra on the inside of a tight bend** (from 90°
with a 0.1 m blend region on a 0.1 m bar; DQS more than LBS at 135°–170°); the material on the inner side passes
through itself. K1 Part D: widening the blend region removes them (no inversions at 90° for a 0.2 m region, none
at 135° for DQS at 0.3 m). This is a property of the weights and geometry, not of the blend, so the check is
needed whichever blend is used. (3) What the field sees is where the mass goes: between LBS and DQS the centroid of
the bar moves by 0.09 mm at 90° and 0.26 mm at 135°, and the second moment by 4×10⁻⁴ and 2.6×10⁻³ relative
(Part E); the same order as the discretisation error of a 25 mm cage (Part B below). Whether that is above the
phase tolerance depends on the distance to the towers; the slice should state it as a phase with the forward
model, not guess it here.

**Inverted elements.** An inverted tetrahedron is a reading hazard as well as a physical one: a signed
face-and-edge polyhedral formula evaluated on a tetrahedron whose orientation has flipped returns the field of
negative mass (the orientation enters every face normal), while the moments side would carry `+m`. So: compute
`det A` for every blended tetrahedron each pose; `det A ≤ 0` (or below a small positive threshold, to catch near-
degenerate elements whose density blows up) is a reported error naming the body, pose time and element count; never
silently read. A gentler rule (clamp, or read the rest-shape mass at the posed centroid) can come later. glTF's own
mirroring rule (negative determinant of the skeleton root's global transform flips the winding, *Instantiation ›
Skinned Mesh Primitives*) is a whole-body reflection, to be applied once at import, not per element.

### Tetrahedralising the body

**Tetrahedralise once, in the rest pose, and skin the tetrahedra's vertices.** Reasons: (i) mass is fixed per
element only if the elements are; re-meshing per pose would re-partition the mass and break bit-exact
conservation; (ii) S7's elastic solids deform the same elements (task text), which needs fixed topology; (iii) BBW
are defined on exactly such a rest-pose volume mesh (Jacobson §3.4, p. 4, Fig. 13); (iv) the convergence oracle is
then a statement about one mesh family under refinement. K1 Part B (DQS, 90° bend) measures it: centroid error
2.0, 0.69, 0.17, 0.033 mm and relative second-moment error 1.5×10⁻², 5.8×10⁻³, 1.4×10⁻³, 2.8×10⁻⁴ for cages of
1, 2, 4, 8 cells across the 0.1 m bar, against 16 across: about second order (ratios 2.9, 4.2, 5.1; the last is
inflated by the n = 16 reference). That is the shape of the oracle "a bent arm converges as the cage refines":
fix the pose and weights, refine the cage, and require the read (not only the moments) to converge at order ≥ 1.5,
with a control arm whose weights are deliberately non-smooth.

**Where the tetrahedra come from**, in order of preference for S6-08:

1. **Cavendish's own structured cage**, no external mesher: a body already given as voxels (or a box/cylinder test
   body) is cut cell by cell into the six Kuhn tetrahedra (as in K1). It is conforming across cells when every
   cell uses the same diagonal, every element has positive volume, and the masses are exactly the cells' masses
   split six ways (or, with a density function, the exact per-tet integrals of the density half). This is enough
   for every S6-08 oracle (bar, bent arm, rigid limit) and is fully deterministic.
2. **fTetWild** for imported surface meshes ([Hu et al. 2020](https://doi.org/10.1145/3386569.3392385)). Input: any
   triangle soup, "no assumptions on the input mesh", self-intersections and small gaps allowed (§3, p. 4); output:
   a mesh with floating-point coordinates whose elements are non-inverted and whose boundary lies within a
   user-set `ε`-envelope of the input (§3, p. 4); open boundaries closed, inside/outside by the generalised winding
   number (§3.6, p. 9, Fig. 11). Defaults `ε = 10⁻³ d`, target edge `ℓ = d/20`, `d` the bounding-box diagonal
   (p. 9). Thingi10k (10,000 real models): 100% success within 128 GB and 24 h, 99.97% within 3 h and 32 GB,
   average 49.8 s on 8 threads; TetGen 49.5%, CGAL 79.0% (§4.1, Table 2, pp. 9–10). On the 4,540 models all four
   codes mesh, mean 18.5 s against TetGen 22 s, CGAL 95 s, TetWild 107 s (§4.2, p. 10). **The catch for mass:**
   the boundary moves up to `ε`, so the meshed volume is not the input's; with the default `ε` on a 1.8 m figure
   (`d ≈ 2 m`) that is up to 2 mm of boundary, which on ~1.8 m² of skin bounds the volume change at ~4 litres
   (~6% of a 70 kg body), as an upper bound, not a typical value. So the tetrahedral mesh *is* the body (its
   volume defines the mass from a density, or a declared mass sets the density), `ε` is set from the phase
   tolerance, and the import reports the meshed volume against the input's signed volume when the input is
   closed.
3. **TetGen** ([Si 2015](https://doi.org/10.1145/2629697)) when the input is a valid piecewise linear complex
   (PLC: cells closed under faces, intersecting only in shared faces; §3.1, pp. 4–5) and the surface must be
   kept exactly: constrained Delaunay tetrahedralisation, boundary preserved (`-Y`, Fig. 23 caption, p. 32), robust
   through Shewchuk's filtered exact predicates (§6, pp. 14–15). Quality: constrained Delaunay refinement with
   radius-edge bound `B` terminates for `B ≥ 2`; with no small input angles no skinny elements remain, otherwise
   only elements touching the small angles may be skinny (§9.3, p. 21); slivers are not bounded by the
   radius-edge ratio and there is no dihedral-angle guarantee (§9.1–9.2, pp. 20–21). Its own stated limits:
   piecewise-linear input only, and mesh quality "may not be as good as other softwares" (§1, p. 2). Self-
   intersecting or non-manifold input is not a PLC; that is why it fails on half of Thingi10k (fTetWild Table 2).

**Licences (an owner question).** fTetWild's repository is MPL-2.0 (GitHub licence metadata of
`wildmeshing/fTetWild`, read 28 Nov 2026). TetGen is dual-licensed, AGPL v3 or a paid commercial licence from WIAS
(the `LICENSE` file of the libigl TetGen mirror, read 28 Nov 2026; the upstream page was not read). The papers
themselves state only that TetGen's source "is freely available" (Si, abstract and p. 2) and that fTetWild "will
be released as an open-source project" (Hu §4, p. 9). Both are C++. Before either becomes a dependency, the owner
should decide whether an AGPL tool may be used at all, and whether a mesher runs as a separate offline import
tool or is linked. The recommendation below does not need either in S6-08.

### glTF import specifics (glTF 2.0 specification, Khronos, `Specification.adoc` at commit `5b00b91`, 20 Nov 2026)

- **Units and frame** (*Coordinate System and Units*): right-handed, +Y up, front faces +Z; "units for all linear
  distances are meters"; angles in radians. Cavendish's body frame must be mapped once at import.
- **Skins** (*Skins*): `joints` (node indices, unique), optional `inverseBindMatrices` (MAT4, float32, identity if
  absent, fourth row exactly ⟨0,0,0,1⟩, no NaN/inf), optional `skeleton` root (must be the common root/ancestor;
  not needed to compute skinning). A "bind shape matrix" is not supported and must be premultiplied by exporters.
- **Influences** (*Skinned Mesh Primitive Attributes*): `JOINTS_n`/`WEIGHTS_n` in pairs, four per set, several sets
  allowed (a client "MAY support only a single set" at the cost of appearance); weights non-negative; joint indices
  with non-zero weight unique per vertex. Weight sums: exactly 1 for unorm8/unorm16 (check in integers: unorm8 × 257
  summed to 65535), and for float "as close as reasonably possible to 1.0", recommended tolerance `2×10⁻⁷` × the
  number of non-zero weights. For LBS a weight sum `1 + δ` scales the posed position by `δ` about the origin: at
  `δ = 8×10⁻⁷` and 1 m from the origin, 0.8 µm. Renormalise in `f64` at import, and keep positions in a body-local
  frame.
- **The node transform of a skinned mesh is ignored** (*Instantiation › Skinned Mesh Primitives*): "A skinned mesh
  primitive does not use the global transform of the node that instantiates the mesh"; the per-vertex transform is
  `Σᵢ Wᵢ · M_{Jᵢ} · B_{Jᵢ}` with `M` the joint node's global transform and `B` its inverse bind matrix. Winding
  follows the sign of the determinant of the skeleton root's (or the joints' lowest common ancestor's) global
  transform.
- **Precision**: positions, inverse bind matrices, keyframe times and values are float32 (*Accessor Data Types*,
  *Skins*, *Animation Samplers*; rotations may also be unorm16-quantised). Convert to `f64` once at import; the
  quantisation is then part of the body, fixed, and mass conservation is unaffected.
- **Animation** (*Animations*): times in seconds, float32, `min`/`max` required; before the first and after the
  last key the end values hold (*Applying Animation Data*). Interpolation `STEP`, `LINEAR` (default) or
  `CUBICSPLINE` (Hermite with in/out tangents scaled by the segment duration; rotations renormalised after
  interpolation). `LINEAR` on `rotation` is slerp with the short-path flip (`d < 0` → negate), `d` clamped to [0, 1],
  plain lerp when `1 − d ≤ ε` (the spec suggests `10⁻⁶`), and "Implementations MAY approximate these equations"
  (*Spherical Linear Interpolation*). **Determinism:** slerp needs `arccos` and `sin`, platform transcendentals
  that S17's web build already treats as a listed exception. Normalised lerp (`+ × ÷ √` only) is allowed by that
  sentence and is bit-reproducible; recommend it, with slerp available behind a flag, and a test that measures the
  pose difference between them for the clips used.
- **Morph targets** add displacements `A′ = A + Σ ΔA·W` (*Applying Morph Data*); the order of morphing and skinning
  was not found stated in the sections read. Morph targets change volume, so a body with morph targets would need
  its cage vertices morphed too; out of scope for S6-08 (reject, or ignore with a warning).
- **Non-rigid joints**: TRS `scale` may be non-uniform (*Transformations*), so DQS on an imported rig needs Kavan's
  two-phase scheme, or must be refused when any joint scale differs from one.

### Validation cases the sources use, and which to copy

| source | case | copy? |
|---|---|---|
| Kavan 2008 §2.2, Fig. 4 | two-joint arm, `w = ½`, 180° twist: LBS collapses to the axis | yes: the candy-wrapper oracle, exact (K1: `det A = 0` on the axis for LBS, ≥ 0.76 for DQS) |
| Kavan 2008 §5, Figs. 14–17 | human 5,002 vertices / 54 joints; cloth 6,000 / 49; visual | no (visual only) |
| Kavan 2008 §6, Fig. 20 | 179° → 181°: DQS flips | yes, as a test that the importer's joint limits or the flip detector catch it |
| Le & Hodgins 2016 Figs. 2, 6, 8 | bend (bulge, collapse) and twist (candy wrapper) on characters | the bend/twist pair, quantified by volume and moments as in K1 |
| Jacobson 2011 Table 2 | bind time and the fast-weights error | only if BBW is built |
| Si 2015 §10, Tables I–V | Delaunay speed, Steiner counts, quality against CGAL and GHS3D | no |
| Hu 2020 §4, Table 2 | Thingi10k success rate and time | no; a handful of real glTF assets with volume reported is enough |

Cavendish's own, from the task: rigid limit bit for bit (with the rigid/blended split above); mass constant through
every pose (exact, since mass is per element; control arm: a version that recomputes mass from `V′`); a bent arm
converging under cage refinement (K1 Part B's shape, on the read); plus from K1: the candy wrapper (LBS
`det A → 0`), the inverted-element detector (tight bend with a narrow blend region must raise the error), and
LBS-against-DQS volume numbers recorded as a measurement, not a pass criterion.

### Recommendation for S6-08

1. **Cage:** fixed rest-pose tetrahedra, Kuhn-split structured cages for S6-08's bodies and oracles; mass per
   element fixed at bind time; the deformed tetrahedron's four vertices (and `A = D′D⁻¹`) handed to the moments
   side. Imported surface meshes through an offline mesher come later, after the licence question (fTetWild first,
   for robustness; its envelope set from the phase tolerance and the volume change reported).
2. **Rigid/blended split** at bind time, so the rigid limit is per-bone trees by construction (the only way to make
   the bit-for-bit oracle hold for DQS, K1 Part C).
3. **Blends:** LBS as the default (it is what glTF assets mean and is the only one that is bit-exact on a single
   weight without the split); DQS per body as an option, with Kavan's Algorithm 2 antipodality per element (four
   vertices share the check cheaply) and refusal of non-unit joint scale until two-phase skinning is needed.
4. **Every pose:** `det A` of every blended element; `≤ 0` is an error. Report total `V′/V` as a diagnostic.
5. **glTF:** f64 at import, weights renormalised, units and frame fixed, node transform of the skinned mesh ignored,
   interior cage weights by harmonic extension of the surface weights, nlerp by default for rotations.

### What I could not verify (skinning)

- Kavan et al.'s timing figures (Figs. 18–19) are images and were not read; the cost statements above are their
  text's.
- The exact article numbers of Jacobson et al. 2011 (78), Le & Hodgins 2016 (37) and Hu et al. 2020 (117) are from
  memory; Crossref returns volume, issue and pages but no article number for them, and the author PDFs do not
  print them.
- TetGen's default `-q` bound and switch meanings are from its manual, not read; only the paper's `B ≥ 2`
  guarantee and the command lines in its figure captions are sourced.
- The upstream TetGen licence page and fTetWild's dependencies' licences were not read.
- Whether glTF specifies the order of morphing and skinning: not found in the sections read.
- The K1 numbers are for one bar, one weight profile and Kuhn cages; real characters with four influences were not
  tested. The phase size of the LBS-against-DQS difference was not computed (it needs the forward model and a tower
  geometry).

### Further questions (skinning)

- Should Le & Hodgins' CoR blend be the third option (it fixes both artefacts for two-bone regions, K1 can test it)?
- Collisions: inner-bend inversion is self-contact; is a contact-aware skin (or simply wider blend regions) S7's job?
- For a body with a density function (S6-07), are the per-element masses the exact integrals over the rest
  tetrahedra, and does any density field ride with the material (yes, in the body frame) or with space?
- Interior weights: is harmonic extension good enough, or do inner-bend inversions argue for BBW-like bounds?

### Bibliography for the skinning half

| authors | year | title | venue | DOI | checked |
|---|---|---|---|---|---|
| L. Kavan, S. Collins, J. Žára, C. O'Sullivan | 2008 | Geometric skinning with approximate dual quaternion blending | ACM Trans. Graph. 27(4), Article 105, 23 pp. | [10.1145/1409625.1409627](https://doi.org/10.1145/1409625.1409627) | Crossref (title, authors, 27(4), pp. 1–23, Oct 2008) |
| A. Jacobson, I. Baran, J. Popović, O. Sorkine | 2011 | Bounded biharmonic weights for real-time deformation | ACM Trans. Graph. 30(4), 8 pp. (Article 78 from memory) | [10.1145/2010324.1964973](https://doi.org/10.1145/2010324.1964973) | Crossref (title, authors, 30(4), pp. 1–8) |
| B. H. Le, J. K. Hodgins | 2016 | Real-time skeletal skinning with optimized centers of rotation | ACM Trans. Graph. 35(4), 10 pp. (Article 37 from memory) | [10.1145/2897824.2925959](https://doi.org/10.1145/2897824.2925959) | Crossref (title, authors, 35(4), pp. 1–10); DOI also printed on p. 1 |
| N. Magnenat-Thalmann, R. Laperrière, D. Thalmann | 1988 | Joint-dependent local deformations for hand animation and object grasping | Proc. Graphics Interface '88, Edmonton, pp. 26–33 (CIPS) | [10.20380/GI1988.04](https://doi.org/10.20380/GI1988.04) | DataCite (not Crossref): title, year, pp. 26–33; resolves to graphicsinterface.org |
| H. Si | 2015 | TetGen, a Delaunay-based quality tetrahedral mesh generator | ACM Trans. Math. Softw. 41(2), Article 11, 36 pp. | [10.1145/2629697](https://doi.org/10.1145/2629697) | Crossref (41(2), pp. 1–36); DOI printed on p. 1 |
| Y. Hu, T. Schneider, B. Wang, D. Zorin, D. Panozzo | 2020 | Fast tetrahedral meshing in the wild | ACM Trans. Graph. 39(4) (Article 117 from memory); read as arXiv:1908.03581v2 | [10.1145/3386569.3392385](https://doi.org/10.1145/3386569.3392385) | Crossref (title, authors, 39(4), Aug 2020); the arXiv PDF prints a placeholder DOI |
| Khronos Group | 2021– | glTF 2.0 Specification | `KhronosGroup/glTF`, `specification/2.0/Specification.adoc` (commit `5b00b91`) | none | read from source 28 Nov 2026 |

## 6. Recommendation for S6-07, with reasons

1. **One read path: exact per-cell moments in the body frame.** Polynomials use Gauss–Legendre with
   `n = ⌈(d+4)/2⌉` per axis on boxes and the divergence theorem on cut cells. Improved and value noise
   use `n = 5` on boxes split at the lattice planes. Trilinear textures use `n = 3` split at texel
   planes. Anything else uses tensor Gauss–Legendre with the DLMF bound where a wavelength is known,
   and the n against n+2 check with bounded bisection otherwise. *Reason:* every case except the last
   is exact against a known answer (B1, B4, B6), the tree already reads moments, and the GPU never sees
   the density function.
2. **Polynomial-density polyhedral closed forms only near.** Use D'Urso 2014 for linear density (it
   has the tensor) and Ren et al. 2018 for higher degree, with D'Urso & Trotta 2017 Table 1 as an
   oracle. *Reason:* their rounding grows as `(R/s)^{4.3…5.3}` (Ren et al. Table 4; Pohánka p. 404),
   while moments do not.
3. **Determinism by construction:** an integer hash, `+ − × floor` only, CPU `f64` at build time, and
   a golden digest test (B2) with a one-ulp control arm.
4. **Oracles:** the graded sphere (point mass plus dipole, B5), D'Urso & Trotta Table 1, the
   Laplace/Poisson trace with the local density, the uniform limit, and exact rational moments of a
   polynomial body. Quadrature control arm: the n−1 rule must visibly miss.

## 7. What I could not verify (density)

- Ren et al. 2018 was read only for its validation cases and §3.4/Table 4, through text extraction.
  The κ values agree with `paper-benchmarks.md`'s earlier reading.
- D'Urso & Trotta's Table 1 columns are interleaved in the text extraction. Read the page image before
  turning it into a fixture.
- D'Urso 2014's §6.1 comparison with Pohánka 1998, Holstein 2003 and Hamayun et al. 2009 is stated,
  not tabulated. Holstein 2003 and García-Abdeslem 2005 (the shared benchmark) were not supplied.
- The simplex-noise form in §2.3 is from general knowledge; the source papers were not supplied.
- "Rust does not contract floating-point expressions" is stated without a primary source; B2's golden
  test is what settles it.
- B3 measures the cancellation of the about-P split with quadrature, not any paper's closed form. The
  closed forms' own growth is Ren et al.'s measurement, not mine.
- The phase that a quadrature error of a given size produces at a tower was not computed here; that
  belongs to the error budget and the slice.

## 8. Further questions

- Is a tetrahedron-split linear kernel with a moment correction accurate enough in the near field to
  avoid writing Ren et al.'s cubic kernel? This needs a measurement in the slice.
- Should noise finer than a cell (`λ ≪ h`) be integrated exactly at `(h/λ)³` cost, or band-limited at
  build time with the dropped power reported?
- Does a density function ride with the material through skinning? This note assumes yes, as a rest
  density `ρ₀(X)` per element, so §4 makes it exact.
- García-Abdeslem 2005 and Holstein 2003: fetch them, to have the benchmark's own table and the κ
  source.

## Reading list (density half)

| Source | Read | DOI (Crossref, 28 Nov 2026) |
|---|---|---|
| Pohánka, V. 1998, *Optimum expression for computation of the gravity field of a polyhedral body with linearly increasing density*, Geophys. Prospect. 46, 391–404 (the batch's file name says 2002) | whole | [10.1046/j.1365-2478.1998.960335.x](https://doi.org/10.1046/j.1365-2478.1998.960335.x) |
| D'Urso, M. G. 2014, *Gravity effects of polyhedral bodies with linearly varying density*, Celest. Mech. Dyn. Astron. 120, 349–372 | §1, §4 opening, §6–7 | [10.1007/s10569-014-9578-z](https://doi.org/10.1007/s10569-014-9578-z) |
| D'Urso, M. G. & Trotta, S. 2017, *Gravity anomaly of polyhedral bodies having a polynomial density contrast*, Surv. Geophys. 38, 781–832 (the batch also held the submitted manuscript, 107 pp.) | §1–3 openings, §4 opening, §5–6 | [10.1007/s10712-017-9411-9](https://doi.org/10.1007/s10712-017-9411-9) |
| García-Abdeslem, J. 1992, *Gravitational attraction of a rectangular prism with depth-dependent density*, Geophysics 57, 470–473 | whole | [10.1190/1.1443261](https://doi.org/10.1190/1.1443261) |
| Martins, C. M., Barbosa, V. C. F. & Silva, J. B. C. 2010, *Simultaneous 3D depth-to-basement and density-contrast estimates using gravity data and depth control at few points*, Geophysics 75, I21–I28 | methodology | [10.1190/1.3380225](https://doi.org/10.1190/1.3380225) |
| Ren, Z. et al. 2018, *Gravity gradient tensor of arbitrary 3D polyhedral bodies with up to third-order polynomial horizontal and vertical mass contrasts*, Surv. Geophys. 39, 901–935 | introduction, validation, §3.4 | [10.1007/s10712-018-9467-1](https://doi.org/10.1007/s10712-018-9467-1) |
| Perlin, K. 2002, *Improving noise*, ACM Trans. Graph. 21(3), 681–682 | whole | [10.1145/566654.566636](https://doi.org/10.1145/566654.566636) |
| NIST DLMF §3.5(v), Gauss quadrature, Eq. 3.5.19–3.5.21 | the section | https://dlmf.nist.gov/3.5#v |
| Mirtich, B. 1996, *Fast and accurate computation of polyhedral mass properties*, J. Graphics Tools 1(2), 31–50 | cited for the method only, not re-read | [10.1080/10867651.1996.10487458](https://doi.org/10.1080/10867651.1996.10487458) |

Scripts: [`density_bench.py`](s6-density-and-skinning/density_bench.py) (B1–B6, standard library,
output `density_bench.out.txt`) and [`k1_skin_volume.py`](s6-density-and-skinning/k1_skin_volume.py)
(K1, needs NumPy, output `k1.out`).
