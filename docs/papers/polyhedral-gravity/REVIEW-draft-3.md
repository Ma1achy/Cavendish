# Referee report: "Exact gravity of polyhedral boundary cells for simulating atom-interferometer gravity gradiometers" (draft 3, 27 Nov 2026)

Reviewed on `main` at 4465a05. Sources: `paper.tex`, `paper.pdf` (identical to the Downloads copy,
checked by `cmp`), every page read as an image at 70 dpi, `generated/`, `data/`,
`docs/research/exact-skin-and-partial-fill.md` with its scripts and `shapes/` output,
`docs/research/s5-world-octree.md`, `docs/slices/S4.md`, and `proofs/polyhedral-gravity/`. I did not
run `lake build`: there is no `.lake` cache in the proofs directory and Mathlib was not downloaded,
as instructed. All 19 DOIs were resolved with `curl -sI https://doi.org/…` and their metadata checked
against the Crossref API (§4 below). The paper was not changed.

## Summary for the editor

The paper does three things. It derives and stabilises the Werner–Scheeres polyhedral closed form,
using the h·2A numerator and a cancellation-free near-edge logarithm. It measures how accurate the
cheaper boundary-cell representations are ("cut-cell moments") in phase units for AION-10. And it
machine-checks most of the derivation in Lean 4.

I re-derived the derivations of §2–§3 and found them correct. The far-field and near-edge numerics
are honestly measured, and the Lean section is unusually careful about what it assumes. Most numbers
I traced (about 60) match the committed script output exactly.

The problems are of four kinds:

- A few quoted numbers do not match their data: the abstract's 10⁻¹² for our closed form at R/s = 100,
  ESA's lower bound, and the "23×" (d)/(c) ratio.
- The single lattice placement used in §5 is the near-symmetric one, which bears directly on the
  sphere "open question".
- The single-instant phase is applied to *moving* sources without the caveat that the interferometer
  averages over 2T = 1.46 s.
- Statements in the abstract and §7 describe the layered scheme as if it were measured.

The paper is also two papers in one: a numerics-and-proof paper, and a Cavendish engine design
document. A journal would ask for the engine material to be cut or moved.

Recommendation: major revision. None of the problems undermines the central technical claims.

---

## Must-fix (errors)

**M1. The abstract understates our closed form's 40-digit error by about 10×.** The abstract says
"our form loses 10⁻¹² of g at R/s = 100". `data/mp_reference.json`, worst of three directions at
R = 100 m, gives the following (ours / ESA):

| body | ours | ESA |
|---|---|---|
| cube_aligned | 9.9×10⁻¹² | 2.3×10⁻⁹ |
| cube | 1.4×10⁻¹¹ | 2.9×10⁻⁹ |
| icosahedron | 8.4×10⁻¹² | 4.8×10⁻⁹ |
| plate | **1.0×10⁻¹⁰** | 3.0×10⁻⁸ |
| sphere | 6.8×10⁻¹² | 2.5×10⁻⁸ |
| stellated | 9.4×10⁻¹² | 5.4×10⁻⁹ |

So ours is 7×10⁻¹²–10⁻¹⁰ (and ε(R/s)² = 2.2×10⁻¹² is the scale), not 10⁻¹². §3 says "≤10⁻¹⁰", which
is right. Make the abstract agree: "10⁻¹¹ (10⁻¹⁰ for the plate)".

**M2. §3 gives the wrong lower bound for ESA at 100 body sizes.** "ESA's values are off by
10⁻¹⁰–3×10⁻⁸" (`paper.tex:304`); the data say 2.3×10⁻⁹–3.0×10⁻⁸ (table above). Also, by the same
data our form exceeds the pre-written 10⁻¹² criterion from R/s ≈ 30 (1.6×10⁻¹² for the cube,
7.4×10⁻¹² for the plate). The text implies that only ESA fails it. State that both exceed 10⁻¹² at
R/s ≥ 30, ours as ε(R/s)² and ESA's as ε(R/s)³. The scaling is the real finding (ours rises ×100 per
decade from 10 to 100, ESA's about ×900), and it is correctly attributed.

**M3. The (d)-against-(c) ratio "23 to 2100 times at 128 across" is wrong, and contradicts the
"1.4 to 2000" range given for every pitch** (`paper.tex:486`). Computed from `shapes/sweep-*.json`
(|c|/|d| at 1.5 m, the nine shapes other than the sphere):

| n across | 4 | 8 | 16 | 32 | 64 | 128 |
|---|---|---|---|---|---|---|
| min ratio | 1.6 (Suzanne) | 1.43 (torus) | 5.8 | 9.3 | 19.9 | **36.8 (bunny)** |
| max ratio | 77 | 368 | 1770 (Buddha) | 648 | 1310 | **2110 (mk7)** |

So the ratio is 1.4–2100 at every pitch and 37–2100 at 128 across. The upper end at every pitch
cannot be smaller than the upper end at 128.

---

## Should-fix (over-claims, missing caveats, unsupported statements)

**S1. The "one lattice placement" is the near-symmetric placement, which may explain the sphere
anomaly.** `shapes_lib.GENERIC = (0.00137, 0.00291, 0.00073)` in units of h, and the sphere and
torus are centred on the origin. So a lattice node sits within 0.3 % of a cell of the sphere's
centre. The cut pattern then has cubic symmetry to that accuracy, which suppresses the low
multipoles of each representation's error field in different ways for (c) and (d).

The criterion test of §5 ("26 random tower directions") rules out a direction coincidence but not a
placement coincidence. The paper's own benchmark points the same way. With four *random* placements,
the 1280-face icosphere has (d) *better* than (c): 2.5×10⁻⁷ against 6.8×10⁻⁷ rad at 1.5 m, and
4.3×10⁻¹¹ against 2.2×10⁻¹⁰ at 10 m (Table 2 and `data/summary.md`). That contradicts "on the sphere
alone adding the second moment makes the result worse" (abstract, §5, §9), and the paper does not
mention it.

Before it is called an open question in the abstract, rerun (c\*)/(d\*) on the analytic sphere with
the benchmark's four random placements. Also say in §5 that "one lattice placement" is the one at
≈ h·10⁻³ from the centre. The same caveat applies to the torus orientation study and to irregular
fitted orders such as the Buddha's dip at 16 cells across. §9's limitations bullet covers the
roughness of the orders but not this.

**S2. The single-instant phase is used for moving sources without saying what the instrument
actually does.** The leading-order k_eff T² Δg_z is stated in §1, §4, §5 and §9, and "at one
instant" once (§5, *Each mesh as one polyhedron*). Two results depend on it much more than "the
prefactors slightly do" (§9):

- *Orientation and motion* (`paper.tex:557–567`, Fig. 12). The error lines at k/h = 10 per metre,
  i.e. 10 Hz per m s⁻¹, are instantaneous phase-error amplitudes. A Mach–Zehnder gradiometer
  responds to a time-varying Δg_z through its sensitivity function. The transfer function, relative
  to k_eff T², is [sin(ωT/2)/(ωT/2)]². With T = 0.73 s this is ≤ 2×10⁻³ at 10 Hz. So at 1 m s⁻¹ the
  0.22 rad line of (a) reads as ≲ 4×10⁻⁴ rad, and (d)'s 10⁻⁸ rad line as ≲ 2×10⁻¹¹ rad. Fig. 12's
  axis "Hz at 1 m/s" invites exactly the reading the instrument does not make. State that these are
  instantaneous amplitudes, and either apply the transfer function or say that it is not applied.
- *Walking past* (`paper.tex:613–616`, Table 7, Fig. 13c–d). The walker moves 2 m during one 1.46 s
  shot, at 2 m from the axis. The peak of a time-averaged read will be lower and broader. That may
  move the 4–18 % pose differences, which are the basis of "sees its shape, not only its mass". Add
  the caveat in the paragraph and in Table 7's caption, or integrate over the shot.

The error-budget caption (Table 8) should also say that its measured rows are leading-order and
static. Its "flight quadrature" row refers to the full flight integral (eq. 1), which the measured
rows do not use.

**S3. The layered scheme is described as measured; it is a projection.** The abstract says the
scheme "holds the representation error near 10⁻⁷ rad". §7 says "The far and quadrature rows are held
at 10⁻¹⁰ rad by the walk's bound and the audited read". Neither the layered read nor the S5 walk
exists yet. Only the per-representation errors (Table 2) and the cross-check are measured. Write
"would hold … (projected from Table 2 plus the S5 design tolerances)". Also, "near 10⁻⁷" should be
"a few 10⁻⁷" (the worst body is 4.0×10⁻⁷ rad, Table 8).

**S4. The abstract over-claims the shapes result.** "On ten further shapes … cut-cell moments reach
10⁻¹⁰–10⁻⁹ rad at 1.5 m with 128 cells across." That holds for the eight meshes (7.3×10⁻¹¹–1.1×10⁻⁹,
`generated/shapes-sweep.tex`). On the sphere and torus (d) is 9.6×10⁻⁸ and 2.4×10⁻⁷ rad. Say "on the
eight meshes".

**S5. "Fill + centroid is third order everywhere" (Fig. 7 caption, §4 third finding) is too strong.**
Table 2 gives 3.8 for the sphere at 1.5 m, and 4.2 for the icosahedron and the sphere at 10 m. Write
"at least third order; exactly third on the cubes and the plate". Likewise, "on turned faces the
exact fill … order 3.5–4.4" leaves out the icosphere (2.9 at 1.5 m, 3.0 at 10 m), whose faces are all
turned; say why (curvature, as the next sentence half-says). "Read orders to ±0.3" has no stated
basis. Give the fit's standard error, or the spread over placements.

**S6. The cost comparison needs its model stated where it is used, and one sentence is wrong for
four of the shapes.** The paragraph labels cost as a *model* correctly. Two gaps remain:

- The model sums every interior cell individually (`sweep.py`: `n_in * NS_SERIES`), with no octree
  merging. The voxel costs therefore grow as N³ and are pessimistic against the whole-mesh sum by a
  large factor at fine pitch. Say so in the caption of Table 5 and Fig. 10, not only "no tree walk".
- "At 10⁻⁸ rad [the direct sum is] cheaper than every voxel technique except (d) on the teapot … and
  the CAD part" (`paper.tex:497–499`). By Table 5, (d) also beats the direct sum at 10⁻⁸ on the
  bunny (893 against 3 832 µs), dragon (175/2 703), Armadillo (208/3 300) and Buddha (116/2 639). The
  sentence is true only of the small meshes; scope it to "of 1 000–7 000 triangles".

**S7. The crossover distances are grid-quantised.** Table 6's distances come from a log grid with a
ratio of ≈ 1.56 (0.78, 1.22, 2.99, 4.68, 7.31, 11.4, 17.9, 28, 43.7, … m; `regimes-distance.md`), so
"0.8–1.2 m", "3–7.3 m" and so on are grid points, not interpolated crossings. Say so in the caption.

**S8. The bound-looseness figure "27–457×" does not trace to a single source.**
`s5-world-octree.md` says "Cavendish's 27–457×" at line 58, but "175–457×" at lines 183, 231 and 569.
`docs/slices/S4.md` R7 says "worst 1/44 of the bound; loosest 457×", i.e. 44–457×. Pick the figure
the S4 validation supports and cite that row.

**S9. The storage claims are inconsistent.** §6 gives "8–20 bytes of cut-cell moments per cut cell".
That is the quantised layout of the research note §4 (u16 fill, 3×i16 centroid, then 6×i16/f16), but
the paper never says the moments are quantised or bounds the quantisation error. The shapes study's
memory model also uses 16 and 40 bytes (`sweep.py`). Say "quantised to 16 bits (error bounded in …)"
or use f32 numbers, and make the two consistent.

**S10. The Lean claims are slightly broader than the theorems.**
- Abstract: "for a rectangular box … the box's face form carries no hypothesis at all". That is true
  of `box_potential`/`box_attraction` (volume to faces) and of `rect_face`/`rect_edge` (one face to
  edges). Table 9 itself says the composition into a single face-and-edge theorem for the box is not
  written. Say "each step carries no hypothesis; they are not yet composed into one theorem".
- §2 says the three factors of D² + N² "vanish only when p is on a closed edge". `vos_ne_zero`
  (`SolidAngle.lean:101`) *assumes* each factor is strictly positive; the "only when" direction (the
  equality case of Cauchy–Schwarz) is not proven. Table 9's "factors ≥ 0" is accurate; the prose
  should not imply more.
- `quadrature_error_additive` and `phase_sum` take the arm trajectories as fixed functions. Superposition
  of phases therefore holds with trajectories fixed, i.e. to first order in the source's own
  perturbation. Say so in §7 ("the phase is a linear functional of V") and in Table 9.
- Otherwise the Lean section is accurate. I checked `DivergenceTheorem`/`GreenTheorem`
  (`Polyhedron.lean:114–129`): they are quantified over all fields differentiable on an open
  neighbourhood, as §8 says. `Face.ω` is defined as the integral. `Setting` requires the point
  outside a compact K ⊇ V and off every edge's line. g = ∇U is not formalised. The inside value 4π
  is not stated. All of this matches Table 9 and §9.

**S11. Prior art is missing.** `refs.bib` contains Powell & Abel (2015), on exact conservative
voxelisation and directly relevant to exact cut-cell volumes, but the paper never cites it (natbib
drops it silently). A referee would also expect the following, for the author to select and check
(I give no references from memory):
- the cut-cell / embedded-boundary literature (the paper borrows the name), and moment-fitting
  quadrature on cut cells;
- geophysical prism/tesseroid forward modelling, where point-mass and multipole approximations of
  cells have known error analyses.

**S12. "Near the body both agree with the 40-digit values to a few ε" over-claims.** At R = 1 m the
worst values are 1.5×10⁻¹⁴ (ours, plate) and 5.3×10⁻¹⁴ (ESA, plate), i.e. 70–240 ε. Write "to
≤ 5×10⁻¹⁴".

**S13. The phase error is compared with a noise spectral density without a bandwidth.** The abstract
and §1 compare 10⁻¹ rad with "the shot noise" of 10⁻⁴ rad Hz⁻¹ᐟ². Give the per-shot noise (or the
noise for a stated integration time), so that "three orders above" is dimensionally meaningful.

---

## Minor (style, presentation, small inaccuracies)

**Layout and figures**

1. **The layout gap on page 14, and the half-empty page 12.** The `\FloatBarrier` before §6
   (`paper.tex:637`) flushes Table 7, Fig. 12 and Fig. 13 at once. Page 12 ends half empty, and
   Fig. 13 lands alone on a float page that LaTeX centres vertically, leaving a large gap above it.
   Possible fixes:
   - drop or move the barrier;
   - use `[tbp]` and `\renewcommand{\floatpagefraction}{.8}`;
   - set `\@fptop` to `0pt` so float pages fill from the top;
   - or, best, cut §5's float count (see item 10).
2. Table 9 (Lean) floats into the middle of §9's bullet list (page 18). Its first column is
   justified with large word gaps; use `>{\raggedright\arraybackslash}p{…}`.
3. Fig. 10: the x tick labels of neighbouring panels collide ("10³10⁵10⁻¹"). Fig. 9 and Fig. 13b
   have very small fonts and dense line sets, so at print size the reader cannot follow a line.
   Fig. 1's legend strip is unreadable at print size.
4. Fig. 17 (the scene render) shows a tiny cube and three sticks. It conveys little; cut it, or
   crop and enlarge it.
5. Captions that say what to look at are good in Figs. 1, 2, 14 and 16. Figs. 5–7, 9–13 and 15
   would benefit from one "look at …" sentence each, e.g. Fig. 9: "(d) falls fastest on every panel
   except the sphere, where it meets (c) at 128 across".

**Equations and notation**

6. Notation drift between text and figure:
   - Fig. 3b uses r₁, r₂ and L for what eq. (5) calls a, b and ℓ, and L clashes with L_e.
   - §2 switches from ν̂_e to n̂_e for the in-plane edge normal without saying so.
   - Eq. (2)'s ∇ is with respect to **c**; say so.
7. §4, second finding: "−(h⁵/12) n̂" is the integral, not yet the average. The average over the cut
   offset has a 1/(h‖n‖₁) factor, which cancels against the ‖n‖₁/h² cells cut per unit area to give
   the isotropic −(h²/12) n̂ per unit area. Show that line; it is the crux of the argument.
8. §3, near an edge: say how the case is handled when the projection falls *outside* the segment
   (then one of a ± ℓ₁ terms is used directly; there is no cancellation).

**Wording**

9. Small textual slips:
   - Fig. 2 caption: "the first-order term once the monopole is right" means the *leading* term; the
     dipole is second order in h.
   - §1: "``sphere'" has mismatched quotes, and "for it the two coincide" should be "for them".
   - Werner & Scheeres equation numbers (10), (15), (17), Holstein & Ketteridge "Tables 2–5" and
     Okabe's "eq. 45" could not be checked without the papers; please confirm them.

**Tables**

10. Table 2 has two rows that are identical in every cell: "fill + centroid (cube)" and "(point)".
    Either say that this is the point of §5's "what a cell is drawn as matters little", or drop one.
    "0/1, renormalised" is not defined in the text.

**Descriptions of the studies**

11. §5 says the teapot, Suzanne and bunny holes were "capped by ear clipping". `sweep-teapot.json`'s
    own description says the teapot's six holes were "capped by a fan to its centroid" (the switch
    to ear clipping came in §3.2 of the study). Check which describes the committed output.
12. The repair description says "every original triangle is kept". For the decimated Buddha,
    78 triangles on non-manifold edges were dropped and others cut out where the mesh crossed itself
    (`mesh-check.md`). Scope the statement to the repaired, undecimated meshes.

**Physics framing**

13. Eq. (1) is the propagation phase only. Say that at leading order, for a potential linear across
    the arms, the laser and separation phases complete it to k_eff T² Δg_z. Otherwise a physicist
    reader will ask.

## Structure: what a referee would ask to cut or move

- **Two papers in one.** §2–§3 plus §8 (a stable, machine-checked polyhedral kernel, with the ESA
  and 40-digit cross-check) form a self-contained numerics paper. §1 and §4–§7 (cut-cell moments for
  voxel gravity in phase units) form the application paper. Either split them, or make the
  application the spine and move §3's ESA comparison and most of Table 9 to appendices.
- **Engine-internal material belongs in supplementary material.** This covers S5 and slice names,
  `PHASE_TOLERANCE`, the importer's byte and triangle caps, `cav render`, the scene files, the status
  box and "provisional" marks (fine for a draft, but not for submission), and the README
  regeneration table. The full-resolution importer paragraph of §5 is a software benchmark, not
  physics.
- **§5 is about 40 % of the paper.** Keep the sweep, the facet error and the cost model. Move
  orientation/motion, the primitive comparison, the decimation table and "each mesh as one
  polyhedron" to a supplement, or to a short separate section on "what a gradiometer sees", fixed
  per S2.
- **§6 (octree) is mostly design.** Its measured numbers are quoted from a research note. Shorten it
  to the "node moments are the true body's" argument plus a pointer.

## Claims traced against data (sample)

All of the following match, except where marked:

**Tables 2, 8 and storage** (`generated/convergence-*.tex`, `data/summary.md`, `budget.tex`,
`storage.json`)
- the 10⁴ and 10⁷ improvements on the aligned cube;
- 2.2–3.7 / 3.8–4.3 orders at 10 m with and without h·2A;
- 214 rad from the 1.48 t icosahedron (the 0.8 m cube is 1.38 t);
- ClosedRel 3.8×10⁻¹⁴ × 214 = 8.2×10⁻¹² rad;
- storage 4.26×10⁶ / 3.09×10⁵ = 13.8.

**Near edge and far field** (`nearedge.txt`, `farfield.txt`)
- near edge: 3.3×10⁻⁶ at 10⁻⁸ m, inf at 10⁻¹⁰, 5.9×10⁻¹¹ for eq. (8);
- far field: variant B follows ε(R/s)² to 3×10⁵.

**Shapes sweep, facet error and anomaly**
- (d) orders 3.6–4.5 on nine shapes and 7.3×10⁻¹¹–1.1×10⁻⁹ on the meshes;
- (c) 4×10⁻⁸–2×10⁻⁷;
- (b) 0.6–3.4 and 2.5×10⁻⁶–2.7×10⁻³;
- facet 0.64 → 0.04, 1.8 → 0.028;
- sphere (d) up to 29× worse than (c) at 8 across.

**Cost** (Table 5)
- ratios 1.1–13 at 10⁻⁴ and 4–70 at 10⁻⁶.

**Crossovers** (Table 6)
- all ranges quoted in §5.

**Exact meshes** (Table 7)
- 3–40 %, 0.05–0.6 %, 3×10⁻⁵–8×10⁻⁴, 4–18 %, decimation 4–7×10⁻⁴ and 0.6–1.3×10⁻³ rad.

**Motion and orientation** (`translation.json`, recomputed with the study's spectrum)
- lines 0.22 / 2.3×10⁻⁴ / 3.6×10⁻⁷ / 1.0×10⁻⁸ rad;
- (d) maximum 2.6–2.8×10⁻⁷;
- orientation 1.2×10⁻³ / 8.6×10⁻⁵, box 1.9×10⁻⁵ → 10⁻³ at 10°, teapot 1.0–1.3×10⁻⁵.

**Full resolution and mesh check** (research note §3.3, `mesh-check.md`)
- 0.18 s, 2.6–4.6 µs/triangle, 0.5–27 s, 43 039 / 42 665 leaves, 53–67 ns, 0.07 s and 1.7 s;
- control arm 1.2–4.0 cm;
- 691 / 1 912 vertices, at most 2.2 / 1.0 cm.

**Octree** (`s5-world-octree.md`)
- 2.6–5.7 sides; 6.5×10⁶ / 1.9×10⁴ / 3×10³;
- 109–4 800, 38–350, 6–65; 3–27; 0.7× at 100 m and 0.5× at 1 km;
- 27–457 is the exception (S8).

**Labels.** "Measured", "model" and "derived" are used honestly, with the exceptions S3 and S6. The
provisional mark is on every measured table and figure; the explanatory diagrams, Table 1 and
Table 9 are rightly unmarked, but the status box says "every table and figure".

## References

All 19 DOIs resolve (HTTP 302 at doi.org). Crossref returns the cited title, venue, volume, pages,
year and first authors for each:

- Okabe; Werner & Scheeres; Tsoulis & Petrović; Tsoulis 2012; Holstein & Ketteridge; Nagy;
- Van Oosterom & Strackee; Badurina et al.; Powell & Abel; Schuhmacher et al.;
- mathlib; de Moura & Ullrich; Barnes & Hut; Salmon & Warren; Dehnen;
- Turk & Levoy; Curless & Levoy; Krishnamurthy & Levoy; Crane, Pinkall & Schröder.

The five added from memory (Turk 1994, Curless 1996, Krishnamurthy 1996, Crane 2013, and Zhou 2016
via its arXiv DataCite DOI 10.48550/arXiv.1605.04797, which resolves) are correct. Minor details:
- Crossref lists Werner & Scheeres without page numbers; 313–344 is the commonly cited range, so
  check it against the PDF.
- Powell & Abel is in `refs.bib` but uncited (S11).

## Licences: what a journal submission would need (the owner decides)

**What the paper uses.**
- **Stanford dragon and Happy Buddha** (and the bunny and Armadillo): the repository permits
  research use with credit, and the dragon and Buddha are "not for commercial use". The paper shows
  no image of the dragon or Buddha, only numbers and plots derived from them, plus a render of the
  bunny (Fig. 8). The credit line is present.
- **Utah teapot and Suzanne**: `common-3d-test-models` states no licence. The paper shows numbers
  and plots derived from them, not images.

**What a submission would need.**
1. A data-availability statement naming each source and the licence it states, as Appendix C
   already does.
2. For a journal that publishes under CC BY, or that is a commercial publisher, the safe course for
   the dragon and Buddha is one of:
   - written permission from the Stanford Computer Graphics Laboratory for use in a published
     article;
   - keeping only derived numbers and plots (no renders) and stating that;
   - substituting shapes under a permissive licence (e.g. CC0, like Spot).
3. For the teapot and Suzanne, cite an original source with a stated licence, or replace them. Two
   possible originals: Newell's 1975 data set, whose usual published sources call it freely
   available; and a Suzanne exported by the author from Blender, where Blender's output is generally
   not covered by its GPL. Do not rely on the unlicensed repository copy.
4. Note that the study's committed PNG renders of the teapot, Suzanne, dragon and Buddha
   (`docs/research/exact-skin-and-partial-fill/shapes/*.png`) are already public in this repository.
   That is a separate question from the paper, for the owner.
5. ESA's tool is GPL-3.0 and only its output numbers are used (`esa_fixtures.json`). That needs only
   a citation, which is present.

This is a description of what the licences say, not legal advice; the owner decides.
