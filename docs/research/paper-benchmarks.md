# The paper's kernels against established gravity codes and the literature

For the draft in `docs/papers/polyhedral-gravity/` (draft 3), in answer to its referee report's S11
(missing prior art) and S6 (cost model). Two parts. The first measures the paper's closed form and
cube voxel sum against ESA's polyhedral-gravity and Fatiando's Harmonica, and against a 40-digit
reference. The second places the paper's methods in the literature. Scripts, data and the
criteria (written before measuring) are in [`paper-benchmarks/`](paper-benchmarks/). The literature
notes, per paper with their reading status, are in
[`paper-benchmarks/literature-notes.md`](paper-benchmarks/literature-notes.md).

**Short answer.**
- **Accuracy.** Near the body (≤ 1.5 m from a 1 m body) every code is within a few 10⁻¹⁴ of 40
  digits; a 5856-triangle mesh puts ESA at 8×10⁻¹³. Far away, the paper's form grows as ε(R/s)².
  ESA and Harmonica grow as ε(R/s)³, as Holstein & Ketteridge and Fukushima predict. That makes them
  10²–10⁵ times worse than the paper's form at 100–1000 body sizes.
- **Voxel sums.** For a voxelised body, Harmonica's prism sum and the paper's cube voxel sum agree
  to 10⁻¹¹ rad at 1.5 m. At 100 m the prism formula's own cancellation leaves Harmonica 2×10⁻⁹ rad
  off on a 3×10⁻³ rad signal; that cancellation is why Cavendish switches the cube kernel to the
  series beyond 3 sides.
- **Novelty.** The kernel is not new and the paper does not claim it is. "Cut-cell moments" exist
  in embedded-boundary codes and in exact conservative voxelisation. What looks new is using them
  as a gravity source, and the measured order ladder in phase units. The paper must cite this prior
  art and narrow its claims.

## 1. Benchmarks

**Setup.**
- Machine: AMD Ryzen 5 3600, a 2019 six-core desktop CPU, at below-normal priority; one thread
  unless stated.
- Software: Python 3.14.3, Harmonica 0.7.0 (choclo 0.5.0, numba 0.68.1), polyhedral-gravity 3.4.1
  (ESA, GPL-3.0, run as a tool and not linked).
- Our form, two implementations: the study's numpy (`skin_bench.polyhedron_field`, h_f·2A_f
  numerator), and **Cavendish's own Rust** `cav_core::polyhedral::Faceted`, run through
  [`kernel_bench`](paper-benchmarks/kernel_bench/src/main.rs).
- Reference: the Werner & Scheeres form at 40 digits with mpmath on the same f64 vertices (the
  paper's `mp_reference.field`). For boxes and voxel sums the reference is Nagy's corner sum at 40
  digits.
- Conventions: every code is brought to U = ∫dV/r, g = ∇U, at density 1 with no G. Harmonica's
  g_z is *downward* and in mGal; it is converted (`common.harmonica_boxes`). All four codes agree on
  sign and value at a generic point to 8 digits; that check was run before the runs below.
- Bodies: the paper's five; an axis-aligned plate (Harmonica models only axis-aligned prisms); the
  stellated icosahedron; and Keenan Crane's Spot (CC0, 5856 triangles, downloaded and checked
  against the study's pinned SHA-256).

**Criteria** ([`criteria.md`](paper-benchmarks/criteria.md)):
- A1: near (R ≤ 3 m), every code within 10⁻¹³.
- A2: far, ours ≤ 10⁻¹⁰ at R/s = 100 and ≤ 10⁻⁸ at 1000.
- A3: Rust and numpy agree to 10⁻¹³ near.
- B1: voxel sums within 10⁻⁹ rad and 10⁻¹² in U of 40 digits.
- B2: every singular point finite and right to 10⁻¹².
- T: Cavendish's kernel within 2× of the 55 ns per triangle the paper quotes.

### Table A: relative error of g against 40 digits, worst of three directions ([`accuracy.json`](paper-benchmarks/accuracy.json), [`bench_accuracy.py`](paper-benchmarks/bench_accuracy.py))

| body (triangles) | R | ours, numpy | ours, Cavendish | ESA | Harmonica |
|---|---|---|---|---|---|
| cube, aligned (12) | 1.5 m | 2.1e-15 | 2.1e-15 | 2.9e-14 | 3.5e-15 |
| | 10 m | 8.5e-14 | 8.5e-14 | 2.6e-12 | 2.3e-12 |
| | 100 m | 9.9e-12 | 9.9e-12 | 2.3e-9 | 6.4e-9 |
| | 1000 m | 4.9e-10 | 4.9e-10 | 1.2e-6 | 5.4e-6 |
| plate, aligned (12) | 1.5 m | 1.3e-14 | 1.3e-14 | 2.0e-13 | 5.9e-14 |
| | 100 m | 8.7e-11 | 8.7e-11 | 4.3e-8 | 4.5e-8 |
| | 1000 m | 1.1e-8 | 1.1e-8 | 2.4e-5 | 4.7e-5 |
| icosphere (1280) | 1.5 m | 3.3e-15 | 2.1e-15 | 8.3e-14 | — |
| | 100 m | 6.8e-12 | 8.6e-12 | 2.5e-8 | — |
| | 1000 m | 7.2e-10 | 5.4e-10 | 3.2e-5 | — |
| stellated (60) | 100 m | 9.4e-12 | 1.0e-11 | 5.4e-9 | — |
| Spot (5856) | 1.5 m | 6.5e-15 | 4.8e-15 | 7.6e-13 | — |
| | 100 m | 2.1e-11 | 1.6e-11 | 3.8e-7 | — |
| | 1000 m | 2.9e-9 | 2.6e-9 | 1.7e-4 | — |

The full grid (8 distances × 8 bodies, U as well) is in the JSON and in
[`acc.log`](paper-benchmarks/acc.log).

**Growth per decade** (aligned cube):

| span | ours | ESA | Harmonica |
|---|---|---|---|
| 10 → 100 m | ×116 | ×880 | ×2800 |
| 100 → 1000 m | ×49 | ×520 | ×840 |

Ours is the ε(R/s)² class. ESA and Harmonica are the ε(R/s)³ class: vertex-sized terms, because
Harmonica sums Nagy's corner antiderivatives (8 vertex terms of size ~R) and ESA uses Tsoulis's line
integrals. This is Fukushima (2020, §3.2, Fig. 2), "errors … grow cubically outside [the
Brillouin sphere]", measured here in a second code. ESA's error also grows with the face count:
Spot is 1.7×10⁻⁴ at 1000 m, against 10⁻⁶–10⁻⁵ for the small bodies. Ours does not grow that way
(2.6×10⁻⁹).

**Against the criteria:**
- **A1 holds at ≤ 1.5 m for ours and Harmonica, and fails at 3 m on the plate.**
  - Ours is 1.1–1.4×10⁻¹³ there; Harmonica 5.8×10⁻¹³.
  - ESA fails A1 from 1.5 m on the plate and Spot (2×10⁻¹³, 7.6×10⁻¹³), and at 3 m on Spot
    (6.3×10⁻¹²).
  - The plate is the hard case for every code: it is 1 m wide but 5 cm thick. Its field is 20× a
    cube's smaller for the same face terms, so every error is about 20× larger. The pass level was
    not moved.
- **A2 holds for six of seven analytic bodies, and fails on the plates.**
  - At R/s = 100 ours is 7×10⁻¹²–2×10⁻¹¹, except the plates: 0.9–1.6×10⁻¹⁰, numpy and Rust.
  - At 1000, ours is ≤ 3×10⁻⁹ except the plates: 1.1–1.8×10⁻⁸.
  - The same thickness factor is at work. The paper's ε(R/s)² statement should say that the ratio
    is to the body's *thinnest* dimension, or carry a mass-to-area factor.
- **A3 holds.** Rust and numpy agree within their common rounding (≤ 10⁻¹⁴ near). These
  numbers reproduce the paper's `mp_reference.json` digit for digit for the numpy form (cube_aligned
  9.9×10⁻¹² at 100 m). They extend it to the engine's own kernel, which the paper does not yet show.

### Table B: singular points of the aligned cube, relative error of U and g ([`accuracy.json`](paper-benchmarks/accuracy.json) `singular`)

| point | ours, numpy | ours, Cavendish | ESA | Harmonica |
|---|---|---|---|---|
| on a face (generic) | 2e-16 | 2e-16 | 5e-16 | 2e-16 |
| face centre (on the triangulation's diagonal) | **NaN** | 2e-16 | 4e-16 | 3e-17 |
| on an edge | **NaN** | 2e-16 | 8e-16 | 3e-16 |
| at a vertex | **NaN** | 4e-16 | 5e-16 | 3e-16 |
| face plane / edge line, outside | 5e-16 | 9e-16 | 3e-15 | 2e-15 |
| 1e-9 m off an edge | **NaN** | 4e-16 | **3e-8** (g) | 1e-16 |

**B2 fails for the study's numpy and passes for Cavendish's Rust.**
- The numpy form has neither the near-edge form nor the dropped-term rule. It gives NaN on edges
  and vertices, and on a face's diagonal, which is an edge of the triangulation. It should never be
  cited as the oracle at such points; the paper's Appendix B should say so.
- ESA is finite everywhere (Tsoulis & Petrović's singular terms). However, it loses 3×10⁻⁸ in g at
  10⁻⁹ m from an edge: the near-edge cancellation that the paper's eq. (hk) removes, now seen in an
  independent code.
- Harmonica's safe-log and safe-atan (Fukushima's conjugate trick, his eq. 24) are exact on all of
  these points.

### Table C: a voxelised body summed cube by cube, h = 1/16 m ([`voxel.json`](paper-benchmarks/voxel.json), [`bench_voxel.py`](paper-benchmarks/bench_voxel.py))

The phase error is the gradiometer read k_eff T² Δg_z (5 m baseline, ρ = 2700 kg/m³) against the 40-digit Nagy
sum, worst of three directions. "U" is the relative error of the potential. Cavendish's cube kernel
is a potential kernel, so it has U only.

| body (cubes) | lower point | signal (rad) | paper's cube voxel sum, phase / U | Harmonica, phase / U | Cavendish `cube_potential`, U | Nagy `prism_potential` alone, U |
|---|---|---|---|---|---|---|
| turned cube (2088) | 1.5 m | 2.0e2 | 5e-13 / 1e-15 | 5e-12 / 3e-14 | 2e-15 | 7e-15 |
| | 10 m | 2.9 | 1e-12 / 1e-13 | 1e-10 / 5e-12 | 1e-15 | 5e-12 |
| | 100 m | 2.8e-3 | 5e-13 / 4e-12 | 9e-10 / 7e-9 | 3e-15 | 2e-9 |
| icosahedron (2246) | 1.5 m | 2.1e2 | 9e-13 / 2e-15 | 5e-12 / 1e-14 | 8e-16 | 9e-15 |
| | 100 m | 3.0e-3 | 3e-13 / 4e-12 | 2e-9 / 1e-8 | 3e-15 | 1e-8 |
| stellated (3578) | 1.5 m | 3.4e2 | 7e-13 / 1e-15 | 1e-11 / 8e-15 | 3e-15 | 1e-14 |
| | 10 m | 4.9 | 6e-13 / 1e-13 | 2e-10 / 6e-12 | 2e-15 | 1e-11 |
| | 100 m | 4.9e-3 | 3e-13 / 2e-12 | 2e-9 / 1e-8 | 1e-15 | 9e-9 |

**What B1 shows:**
- **The paper's cube voxel sum and Harmonica's prism sum agree, as they must.** At 1.5 m they are
  within 10⁻¹¹ rad of each other and of 40 digits; B1 passes there for both.
- **At 10 and 100 m Harmonica fails B1 by its own rounding,** not by any disagreement in the
  physics: 2×10⁻¹⁰ and 2×10⁻⁹ rad, and 10⁻⁸ in U.
  - Each 1/16 m prism at 100 m is at R/h = 1600, deep in the prism formula's cubic-cancellation
    regime.
  - Relative to the 3×10⁻³ rad signal that is 7×10⁻⁷: six digits lost for a gradiometer read 100 m
    from a voxelised body.
  - Cavendish's Nagy `prism_potential` alone fails in the same way (10⁻⁸). That is the measured
    reason its cube kernel uses the prism only within `PRISM_WITHIN` = 3 sides and the series
    beyond. The kernel as a whole is at ≤ 3×10⁻¹⁵ at every distance, and passes.
- **The paper's cube voxel sum** (12 triangles per cube, ε(R/s)² class) stays at ≤ 10⁻¹² rad. Its U
  reaches 4×10⁻¹² at 100 m, failing the 10⁻¹² U criterion by 4×; the phase criterion holds.

### Table D: cost per field point, one thread, Ryzen 5 3600 ([`timing.json`](paper-benchmarks/timing.json), [`bench_timing.py`](paper-benchmarks/bench_timing.py))

Polyhedra, ns per point per triangle:

| code (what one call computes) | cube (12) | icosphere (1280) | Spot (5856) |
|---|---|---|---|
| Cavendish `Faceted::field` (U, g, Σω) | 107 | 75 | 97 |
| ESA `GravityEvaluable` (U, g, full tensor) | 824 | 925 | 953 |
| study numpy (U, g; vectorised) | 237 | 1098 | 890 |

Voxels, 2249 cubes of 1/16 m, points 1.5–3 m away, ns per point per prism:

| code | ns |
|---|---|
| Cavendish `cube_potential` (U; prism near, halves, series far) | 271 |
| Cavendish Nagy `prism_potential` (U) | 731 |
| Harmonica, per field: U / g_e / g_n / g_z | 673 / 630 / 460 / 527 |
| Harmonica g_z, numba-parallel, 4 threads | 202 |
| the paper's cube voxel sum, 12 triangles per cube, numpy (U, g) | 10 600 |

**T passes, at the edge.** `Faceted::field` costs 75–107 ns, 1.4–1.9 times the paper's 55 ns.
- The 55 ns is the shapes study's `mesh_bench`, a different program with the per-triangle data laid
  out flat.
- `Faceted::field` allocates two vectors per call and was run at below-normal priority on a
  working desktop.
- The paper should quote the engine's number, or say which program it timed.

**Against ESA.** Per triangle, ESA does about 9× the work of Cavendish's kernel, but it also returns
the full gravity tensor, so this is not a like-for-like comparison.

**Against Harmonica.** Harmonica's numba prism costs about 2.3 µs per prism for U and g together
(four calls). That is in line with Cavendish's own Nagy prism (0.73 µs for U alone).

**For S6 (cost model).** The model's 8.5 ns per cube is the far-field series, not a prism. A prism
or polyhedral read costs 75–730 ns, 10–90× more, so the near layer is where the cost is. The model
should carry these numbers when it prices the clipped-piece layer.

## 2. The literature, and how the paper's methods relate

Reading status per item is in [`literature-notes.md`](paper-benchmarks/literature-notes.md):
FULL, PART, ABS or NONE. SEG, Springer and Wiley refused automated fetches, so **several sources
the draft already cites were not read here**:
- Werner & Scheeres 1997: its eqs 10, 15 and 17, and its Castalia mascon numbers;
- Holstein & Ketteridge 1996: its Tables 2–5;
- Okabe 1979: its eq. 45;
- Salmon & Warren 1994: the bound.

They are listed in `papers-to-fetch.md`. What is stated below comes from the parts that were read.

- **Prisms (Nagy 1966; Nagy, Papp & Benedek 2000; Plouff 1976; Banerjee & Das Gupta 1977;
  Fukushima 2020).** These compute the closed-form field of a right rectangular prism (Plouff: of
  polygonal prisms).
  - Fukushima ([GJI 222:1898](https://doi.org/10.1093/gji/ggaa240), §3.2) says the standard form is
    accurate to machine ε inside the Brillouin sphere and grows cubically outside it.
  - His eq. 24 swaps the log for its conjugate form when X < 0, the same move as the draft's
    eq. (hk).
  - Table A and Table C measure this cubic growth in Harmonica, which uses that form.
  - **Relation:** the draft's interior cube is this prism; Cavendish already limits it to 3 sides
    (Table C).
- **Tesseroids (Uieda et al. 2016; Soler et al. 2019; Grombein et al. 2013; Heck & Seitz 2007).**
  These are spherical-shell cells for global and regional models.
  - The accuracy target is 0.1%, met by adaptive subdivision at a distance-to-size ratio D (D = 1
    for V, 1.5–2.5 for g; Uieda 2016 abstract, Soler 2019 eq. 11).
  - Grombein's optimised formulas are a Taylor expansion of the cell kept to zero and second order,
    i.e. point mass plus second moment, with an error of fourth order.
  - **Relation:** the draft's "fill + centroid + second moment" is the same truncation, applied to
    *cut* cells with their exact moments. The tesseroid community's 10⁻³ target is four orders
    looser than AION's 10⁻⁷. At metre scales the spherical geometry is irrelevant; prisms or
    polyhedra are the right cells.
- **Polyhedra (Okabe 1979; Werner 1994; Werner & Scheeres 1997; Petrović 1996; Holstein &
  Ketteridge 1996; Tsoulis 2012; D'Urso 2013/2014; Conway 2015; Werner 2017).** These are the same
  closed-form field, written in face–edge (Werner), line-integral (Petrović, Tsoulis) or
  vector-potential (Conway) form.
  - Holstein & Ketteridge's abstract already analyses error growing with distance and gives
    remedies.
  - Werner 2017 is about the solid angle hidden in all these formulations. **Whether h_f·2A_f is
    already in it, or in Holstein & Ketteridge, is unverified**; both are on the fetch list.
  - **Relation:** the draft rightly claims no new kernel. Its numerical rules should be framed as
    "a specific, measured fix for the Werner–Scheeres solid-angle form", citing Holstein &
    Ketteridge and Fukushima for the rounding classes.
  - ESA's code is a Tsoulis implementation; Table A confirms the paper's ε(R/s)³ attribution for it
    independently.
- **Mascons (Muller & Sjogren 1968; Werner & Scheeres 1997; Wittick & Russell 2019; Rathinam &
  Dempster 2019).** These are point masses filling a shape model.
  - The one recent number read: a cubified 100³ mascon model of Eros has a relative acceleration
    error of 2.6×10⁻⁴ (MasconCube, arXiv:2509.08607, Table 3).
  - Rathinam & Dempster put mascons on an octree, the closest analogue to the paper's tree, but
    were **not read**.
  - **Relation:** mascons are the paper's row (a), plain cubes. The paper's order ladder quantifies
    what mascon work reports as a single number.
- **Cut cells and embedded boundaries (Johansen & Colella 1998; Aftosmis, Berger & Melton 1998;
  Colella et al. 2006; Schwartz et al. 2006, 2015; Müller, Kummer & Oberlack 2013; Saye 2015).**
  - Schwartz et al. 2015 ([CAMCoS 10:83](https://doi.org/10.2140/camcos.2015.10.83), read in full)
    store, per cut cell, the volume moments ∫(x−x̄)^p dV to any order (eq. 3).
    - The volume fraction is the zeroth moment (eq. 9).
    - Order P+D moments are needed for a P-th order scheme (eq. 17).
    - Coarse cells' moments are the sums of their children's, by the shift rule (Fig. 3, §3.1).
  - **Relation: this is the paper's "cut-cell moments", and its node aggregation, as prior art.**
    The paper borrows the name; it must cite the method. What differs is the use: there the moments
    feed a PDE stencil, here they are a gravity source measured in phase.
- **Exact voxelisation and polyhedral moments (Powell & Abel 2015; Mirtich 1996; Werner 1997).**
  - Powell & Abel ([JCP 297:340](https://doi.org/10.1016/j.jcp.2015.05.022), read in full) clip
    polyhedra against voxels exactly. They compute each piece's volume and moments up to quadratic
    order, ten numbers, as the draft does, in the r3d code.
  - They show that voxel-centred coordinates are essential for rounding (eq. 4.3), and measure
    conservation to 1.7×10⁻¹² rms (Table 3).
  - Mirtich gives exact mass, centroid and inertia of a polyhedron; Werner 1997 gives its spherical
    harmonics.
  - **Relation:** the draft's clipped pieces and their 10 moments are Powell & Abel's construction.
    The draft has it in `refs.bib` but never cites it (S11).
- **Tree gravity (Barnes & Hut 1986; Greengard & Rokhlin 1987; Salmon & Warren 1994; Springel 2005;
  Dehnen 2014).**
  - GADGET-2's relative opening criterion (eq. 18) gives 10⁻⁴–10⁻² force errors (Fig. 1).
  - Dehnen's FMM reaches 10⁻⁷ relative.
  - Warren & Salmon's (1993) data-dependent bound is the ancestor of the paper's rigorous one.
  - **Relation:** the paper's acceptance by a phase bound is an instance of Salmon–Warren and
    Dehnen error control. Its accuracy regime, 10⁻⁷ relative, is Dehnen's, not GADGET's.
- **Anti-aliasing analogues (Catmull 1978; Duff 1989; Hockney & Eastwood).** Exact area coverage
  per pixel (Catmull) is the graphics analogue of the exact fill fraction. Cloud-in-cell mass
  assignment is the physics analogue of spreading a cell's mass to fixed points. Only abstracts or
  nothing were read here; cite one at most, as an analogy.

### Related-work paragraph (draft, for the paper's §1 or a new §1.1)

> The field of a uniform polyhedron is classical: in face–edge form (Okabe 1979; Werner 1994;
> Werner & Scheeres 1997), as line integrals (Petrović 1996; Tsoulis 2012; Tsoulis & Petrović 2001
> for the singular points; D'Urso 2014) and from vector potentials (Conway 2015), with the
> rectangular prism (Nagy 1966; Nagy et al. 2000; Fukushima 2020) as its axis-aligned case. That
> these forms lose accuracy with distance, and how to recover it, goes back to Holstein & Ketteridge
> (1996); Fukushima (2020) shows the prism's error growing as the cube of distance outside its
> Brillouin sphere. Our rounding rules are one such remedy for the solid-angle form. Representing a
> body by cells whose fields are expanded about their centres is the basis of mascon models (Muller
> & Sjogren 1968; Werner & Scheeres 1997; Rathinam & Dempster 2019) and of tesseroid modelling, where
> Grombein et al. (2013) keep the zeroth and second-order terms and Uieda et al. (2016) and Soler et
> al. (2019) subdivide adaptively to a 0.1% target. Storing per cut cell the exact volume moments of
> the part inside the body is the geometric core of embedded-boundary methods (Johansen & Colella
> 1998; Schwartz et al. 2006, 2015), whose coarse cells also carry the sums of their children's
> moments; computing those moments exactly by clipping a polyhedron against the lattice is the
> conservative voxelisation of Powell & Abel (2015), and exact polyhedral mass properties go back to
> Mirtich (1996). Hierarchical evaluation with error-controlled acceptance follows Barnes & Hut (1986),
> Salmon & Warren (1994) and Dehnen (2014). What we add is the measurement: the order in the pitch
> of each cut-cell representation as a gravity source, in the phase of an atom-interferometer
> gradiometer, at a tolerance (10⁻⁷ of the signal) four orders tighter than geophysical forward
> modelling usually targets.

(Before submission: check this paragraph against the fetched papers. In particular, check Werner
2017 and Holstein & Ketteridge 1996 for h_f·2A_f. If either has it, the paper's §3 must say "as in …".)

### Proposed bib entries

There are 31 entries in [`literature-notes.md`](paper-benchmarks/literature-notes.md) §(b), every
DOI checked against `api.crossref.org`. Four were re-checked here by hand: Schwartz 2015, Fukushima
2020, Rathinam & Dempster 2019 and Mirtich 1996. The ones the paragraph above needs:
- `nagy2000`, `fukushima2020`, `werner1994`, `petrovic1996`, `durso2014`, `conway2015`;
- `muller1968`, `rathinam2019`, `grombein2013`, `uieda2016`, `soler2019`;
- `johansen1998`, `schwartz2006`, `schwartz2015`, `mirtich1996`;
- `powell2015` (already in `refs.bib`) and `tsoulis2001` (already there).

`werner2017` and `holstein1996` belong in it once they are read.

## 3. What the paper should change

1. **Cite the prior art for cut-cell moments and narrow the novelty claim** (§1, "Cut-cell
   moments" paragraph; §6).
   - Schwartz et al. 2015 and Johansen & Colella 1998: per-cut-cell volume moments, and coarse
     cells as sums of their children.
   - Powell & Abel 2015: exact clipping and the 10 moments.
   - Mirtich 1996.
   - Grombein et al. 2013 and Heck & Seitz 2007: point plus second moment per cell.
   - The contribution is the measured order ladder as a *gravity source* in phase units.
2. **Frame §3's rules against Holstein & Ketteridge and Fukushima.** The ε(R/s)³ class for vertex
   and line-integral forms is now measured in two independent codes (ESA ×880 and Harmonica ×2800
   per decade at 10–100 m; Table A). Add Harmonica as a second independent code in Fig. 6.
   - It covers only boxes, but it is BSD-licensed and needs no "used as a tool" caveat.
3. **State the ε(R/s)² scaling with the thin dimension.** The plate is 10⁻¹⁰ at R/s = 100 because
   it is 5 cm thick; the abstract's single number should be "7×10⁻¹²–1.6×10⁻¹⁰, worst for the
   plate" (agrees with referee M1).
4. **Quote the engine's kernel, not only the study's numpy.** Cavendish's `Faceted` reproduces the
   numpy errors at every distance (Table A). It is the code the paper describes, and it is singular-safe
   where the numpy is not (Table B). Appendix B should say that the numpy form is NaN on edges,
   vertices and a face's diagonals.
5. **The near-edge claim gains an independent witness.** ESA loses 3×10⁻⁸ in g at 10⁻⁹ m from an
   edge, while the paper's form and Harmonica (Fukushima's conjugate form) stay at 10⁻¹⁶ (Table B).
6. **Cost (S6):** replace "55 ns per triangle" with the engine's measured 75–107 ns (Table D), or
   name the program timed. Give the near-layer costs (prism 0.7 µs, cube kernel 0.27 µs, piece
   7–14 triangles × ~0.1 µs) beside the 8.5 ns far-field figure, and say that ESA and Harmonica
   cost 0.5–1 µs per face or prism per field on the same machine.
7. **A sentence for §7 ("why layer"):** a plain prism sum, as in Harmonica, loses six digits of a
   gradiometer read 100 m from a voxelised body (2×10⁻⁹ rad on 3×10⁻³; Table C). That is the case
   for the cube series beyond a few sides, which Cavendish already has.

## 4. Could not verify, and further questions

**Not verified:**
- Whether h_f·2A_f (or the near-edge form) appears in Werner 2017 or Holstein & Ketteridge 1996.
- The draft's citations to W&S eqs 10/15/17, H&K Tables 2–5, Okabe eq. 45 and the Salmon–Warren
  bound.
- The Castalia mascon numbers.
- Rathinam & Dempster's octree mascons.

All are in `papers-to-fetch.md`.

**Not measured:**
- Tesseroids. Harmonica has them, but they are spherical cells and irrelevant at metre scale.
- The `polyhedral-gravity` tensor accuracy.
- f32/GPU behaviour.

**Further questions:**
- Does ESA's face-count growth (Spot 1.7×10⁻⁴ at 1000 m) come from summing many vertex-class
  terms, or from its normal-orientation and segment handling? A Spot-sized sweep of the face count
  would tell.
- Would a compensated sum in `Faceted::field` remove the plate's thickness factor?
- Should `Faceted::field` avoid its per-call allocations, to reach the study's 55 ns?

## Reproducing

```
python -m venv .venv-bench && .venv-bench/Scripts/python -m pip install harmonica polyhedral-gravity mpmath numpy
cargo build --release --manifest-path docs/research/paper-benchmarks/kernel_bench/Cargo.toml
set KERNEL_BENCH=<target>/release/kernel_bench.exe
cd docs/research/paper-benchmarks
python bench_accuracy.py    # about 2 min  → accuracy.json, acc.log
python bench_voxel.py       # about 6 min  → voxel.json, vox.log
python bench_timing.py      # about 1 min  → timing.json, tim.log
```

Nothing third-party is committed: Spot is downloaded into `.cache/` and checked against its hash,
and ESA and Harmonica live only in the venv.
