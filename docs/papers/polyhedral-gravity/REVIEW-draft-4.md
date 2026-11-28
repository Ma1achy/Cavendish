# Referee report: "Exact gravity of polyhedral boundary cells for simulating atom-interferometer gravity gradiometers" (draft 4, 28 Nov 2026)

Reviewed on `main` at 590d7d5. I read `paper.tex` in full, which is the same as `paper.pdf` and the
Downloads copy (checked by `cmp`). I also looked at pages 5–6 of the PDF as images. Sources traced:

- `generated/`, `data/level/conv.json`, `scripts/conv_level.py`;
- `docs/research/paper-real-read.md` with `tables-budget.md` and `conv_real.py`;
- `paper-flight-phase.md`;
- `paper-benchmarks.md` with `accuracy.json` and `literature-notes.md`;
- `surface-representation.md`;
- `exact-skin-and-partial-fill/skin_bench.py`;
- `docs/slices/S5.md` §7.1, §7.6 and §7.7;
- `proofs/polyhedral-gravity/` (every theorem the paper names).

All 56 DOIs in `refs.bib` were resolved through `api.crossref.org/works/<doi>` (§6 below). I made no
other external contact. I did not build the Lean proofs, because building needs the Mathlib cache from
the network; the `#print axioms` output is not in the repository either. The paper was not changed.

This is a fresh read, not a check against draft 3's report. Where a draft-3 item comes up again, I
say so.

## Summary

There are three contributions:

- the Werner–Scheeres closed form, kept accurate in f64 by the h_f·2A_f numerator and Holstein &
  Ketteridge's edge logarithm;
- a measured ladder of boundary-cell representations, with orders 1/2/3/4 and a "uniform double
  layer" argument for why turned faces do better;
- a Lean formalisation of the derivation.

Draft 4 adds the real interferometer read (the flight integral along the arms) and redoes every phase
number with it. That is the right change, and it is carried through carefully.

**What I checked and found correct:**

- **The derivations of §3.** I re-derived the following and they are right:
  - eq. 6, the volume-to-face step;
  - eq. 7: the in-plane divergence 1/r + h²/r³, with the sign of h·ω;
  - eq. 8: the edge integral and its second form;
  - the van Oosterom–Strackee D and N, and the modulus identity (proven in Lean);
  - eq. 10, with the signs of U, g and Σω (0 outside, 4π inside).
- **The near-edge form (eq. 11)** is an exact identity, and the rule "use whichever side adds" is
  correct in all three cases (the point projects inside the edge, before ℓ₁, or past ℓ₂).
- **The rounding analysis.** Face terms are O(sγ), edge terms O(s) and vertex terms O(s/γ). The triple
  product has an absolute error of εR³, which puts h_f·ω_f in the vertex class; h_f·2A_f keeps it in
  the face class, below the edge terms. This is consistent with H&K's Table 2 (absolute errors) and
  Table 4 (relative errors on an inclined track).
- **The double-layer argument of §4**, ⟨D⟩ = −h⁴n/(12‖n‖₁) with ‖n‖₁/h² cells per unit area, is right.
- **The transfer-function numbers** (1 % at 0.076 Hz, 10 % at 0.24 Hz, half at 0.61 Hz, a null at
  1.37 Hz, 1.2×10⁻³ at 10 Hz) and **k_eff T² = 4.80×10⁹** recompute exactly.
- **The arm kinematics.** The apex is 5.01 m, the separation 4.8 m, and v_kick = 6.58 m/s for ⁸⁷Sr.
  The unperturbed Mach–Zehnder closes, which the first-order result needs.
- **The bias bound.** The Cutler–Vallisneri formula, and its Cauchy–Schwarz bound
  |δθᵢ|/σᵢ ≤ ‖Δφ‖/σ, are correct.
- **The Lean statements** match the paper's Table 12 (§5 below).

Of the more than 40 numbers I traced, most match their source exactly (list in §7).

**What needs fixing:**

- Two physics statements in §2 are wrong as written, though neither changes a conclusion:
  - "the laser and separation phases change only at second order";
  - "above 1/T a line is read at ≲ 10⁻²".
- Several quoted ranges disagree with their own data, including the abstract's headline factor
  "1.5–3.3".
- The near-field conclusions depend strongly on the model's n = 1000 (the 4.8 m arm separation), and
  the paper does not say so.
- Effects the model leaves out are not bounded against the 10⁻⁸ rad errors the paper resolves.

**Recommendation: minor revision.** The central technical claims stand. Every must-fix item is a
local correction.

For a journal, the paper is long: 24 pages, 12 figures and 11 tables. I would move §3.1, §6 and
Appendix B to supplementary material (§8 below).

---

## 1. Must-fix (errors)

**M1. §2, `paper.tex:221–223`: the laser and separation phases do change at first order.** The
sentence reads: "The laser and separation phases change only at second order in the source, so
eq. (5) is the whole first-order phase."

The conclusion is right but the reason is not:

- To first order, the source shifts the arms by δz(t). The laser phase is k·z at the pulses, so it
  changes by k·δz(tᵢ), which is first order.
- The separation phase also changes at first order.
- These changes cancel against the boundary terms of the propagation phase, whose integrand is
  stationary between kicks. What remains is ∮δL along the unperturbed, closed arms (Storey &
  Cohen-Tannoudji 1994; Hogan et al. 2008 §5.2.1).
- The textbook counter-example is a uniform δg. In the standard split, the whole k_eff g T² is
  *laser* phase and the propagation phase is zero. So the laser phase changes at first order.

Suggested wording: "To first order the changes of the laser and separation phases cancel the end
terms of the propagation phase, so for a closed unperturbed interferometer eq. (5) on the unperturbed
arms is the whole first-order phase."

**M2. §2, `paper.tex:280` and the Fig. 3 caption (`:300`): "above 1/T a line is read at ≲ 10⁻² of its
instantaneous size" is false between 1/T and about 4 Hz.**

- sinc²(πfT) has its first sidelobe at f ≈ 1.96 Hz, with value 0.047.
- The envelope 1/(πfT)² is 0.10 at 1/T = 1.37 Hz.
- The response first stays below 10⁻² from πfT ≈ 10, which is f ≈ 4.4 Hz.
- The paper's own near-field data contradict the sentence: flight/frozen is 0.154 at 2 Hz and 0.019 at
  3 Hz (`paper-flight-phase.md` §3.3).
- The same sentence appears in that note's §4 table.

Say instead: "above 1/T at ≲ 0.05 (sinc², first sidelobe), falling as (πfT)⁻²; ≲ 10⁻² above about
4 Hz", and say that near a source the response departs from sinc² by factors of 0.3–3.

**M3. Abstract (`:61`) and §2 (`:261`): "a static 1 m body 1.5 m from a tower … differs by a factor
1.5–3.3" (smaller, on the 17° ray).** The flight/leading ratios in `paper-real-read.md` §2 are:

| shapes | ratio |
|---|---|
| the eight 1 m meshes | 1.56–2.26 (Buddha 1.56, Armadillo 2.26) |
| the 2 m sphere | 3.28 |
| the 2.8 m torus | 4.27 |

So:

- 3.3 belongs to the 2 m sphere, not to a 1 m body.
- The torus, at 4.3, falls outside the quoted range altogether.

The fix: "a 1 m body … 1.6–2.3 times smaller (3.3 for the 2 m sphere and 4.3 for the 2.8 m torus)".
The abstract should then say "1.6–2.8, larger or smaller with the geometry", or give both ranges.

**M4. §3, `:464–466`: the near-body accuracy figures are wrong for two bodies, and contradict
Table 11.** The text claims ours is "within 1.5×10⁻¹⁴ … on every body", and ESA "within 5×10⁻¹⁴ on
the benchmark bodies". The worst over three directions at R ≤ 1.5 m in
`paper-benchmarks/accuracy.json`:

| body | ours (Python) | ours (Cavendish) | ESA |
|---|---|---|---|
| turned plate | 3.8×10⁻¹⁴ | 3.0×10⁻¹⁴ | 1.2×10⁻¹³ |
| icosphere | 3.3×10⁻¹⁵ | 2.1×10⁻¹⁵ | 8.3×10⁻¹⁴ |

- Table 11 itself uses `\ClosedRel` = 3.8×10⁻¹⁴, which is the turned plate's value. So the paper
  contradicts itself.
- The fix: "within 4×10⁻¹⁴ (1.5×10⁻¹⁴ except the turned plate)". For ESA: "within 1.2×10⁻¹³ on the
  benchmark bodies, 2×10⁻¹³ on the aligned plate and 7.6×10⁻¹³ on Spot".

**M5. The (d) threshold at 1.5 m: "adequate … at the coarsest pitch tried" is not true for the bunny.**
This appears in the abstract (`:67–68`), §8 (`:941`) and the Fig. 11 caption (`:952`, "below 0.1 at
every pitch at 1.5 m").

- In `paper-real-read/tables-budget.md`, the bunny's (d) at n = 4 (250 mm) is **0.18 σ**, and its
  threshold is 220 mm.
- The cube (0.049) and the CAD part (0.090) do pass at 250 mm.
- Say: "at 220 mm or coarser (the bunny fails the coarsest pitch run, 250 mm, at 0.18 σ)", and fix the
  caption.

**M6. Abstract, `:68–69`: "a sampled fill only below a millimetre".** The extrapolated thresholds at
1.5 m are 0.13 mm (cube), 0.67 mm (CAD part) and **4.4 mm** (bunny). §8 itself says "0.1–4.4 mm". Make
the abstract agree, and say that these pitches are extrapolated (the fill does not converge in the
range run).

**M7. §4, `:616`: "At 10 m … the errors 50–6000 times [smaller]".** From `data/level/conv.json` (RMS
at 1/64 m, 1.5 m against 10 m):

- the ratios run from 59 (0/1) up to **13 200** (the plate's second moment);
- the icosahedron's second moment is 7 300 and the icosphere's 6 400.

Say "60–13 000".

## 2. Should-fix (over-claims and missing caveats)

**S1. The headline "of the opposite sign" and "not the leading term of anything" depend on where the
leading read is taken.** This is `:60–62` and `:260–264`.

- The comparison read is k_eff T² Δg_z, with g_z taken at the two *launch points*. With the body level
  with the lower launch point, g_z there is near zero by symmetry. That makes the sign flip and the
  factor of 3 partly a property of the reference point.
- The paper itself shows that the gradient at the arms' Δz-weighted mean height (2.17 m) is exact for
  a quadratic potential, and is only 22–25 % high for a walker at 2 m (`:264–266`).
- So at 2 m the instrument reads something within 25 % of *a* gradient read. "Not the leading term
  of anything" (`:260`) over-states this.

The fix: in the abstract, either compare with the mean-height read as well, or add "(against the
gradient between the launch points; the gradient at the arms' mean height is 22–25 % high)".

**S2. The AION-10 parameters are not labelled as the model's assumptions where they first appear, and
the near-field conclusions' dependence on them is not stated.**

- §1 (`:88–92`) gives n = 1000, λ = 698 nm, T = 0.73 s and Δr = 5 m as facts of AION-10, citing
  Badurina et al. 2020.
- Only §2 (`:224`) and the limitations section call them "Cavendish's model".
- As I recall Badurina et al.'s Table 1 (I did not check it against the paper), n = 1000 and
  10⁻⁴ rad/√Hz are the *goal* configuration, the initial one being n = 100 and 10⁻³. T = 0.73 s and
  u₀ = 3.86 m/s are Carlton & McCabe's choices. Please check.

Every near-field result scales with the arm separation v_kick T ∝ n:

- the sign flip;
- "not a gradient read at 1.5–2 m";
- the 94 m and 640 m distances at which the reads agree to 1 %;
- the Fig. 3a map;
- the 2.5–3× looser CRBs.

With n = 100 the arms separate by 0.48 m, and the leading read would hold much closer.

The fix:

- label the parameters in §1 and in the abstract as the model's ("in Cavendish's model of AION-10's
  goal configuration…");
- add one sentence on the scaling with n;
- name the isotope (⁸⁷Sr: v_kick = 6.58 m/s needs it).

**S3. Effects the model leaves out are listed but not bounded, and some may be large next to the
errors the paper resolves.** The limitations section says that second-order effects in the source,
Earth's gradient and finite pulses are not included. The paper resolves representation errors of
2×10⁻⁸ rad (Table 11) and position CRBs of 2.6 µm. I made rough estimates of my own, for a 1.4–2.7 t
body at 1.5 m; please check them:

| effect | my estimate | how |
|---|---|---|
| second order in the source | ~10⁻⁶ rad | (m/ħ)·|δa|²·T³ with |δa| ≈ 4–8×10⁻⁸ m s⁻² |
| Earth's rotation | ~10⁻⁶–10⁻⁵ rad | moves the arms apart sideways by 2Ω cos λ·v_kick·T²/2 ≈ 0.16 mm, a 10⁻⁶–10⁻⁵ rad change in a 1.5 m read, and a bias of order 100 µm on the fitted position |
| finite pulses | 0.14–7 % of the signal | about 0.06–3 rad on 46 rad |

None of these changes the *ranking* of representations, because each is common to all of them.
They do bear on:

- Table 11, a budget of phase contributions that omits them;
- §8's absolute claims ("knowing the body to better than a millimetre"; CRBs of a few µm).

The project's quality bar asks that every approximation be bounded. Add one paragraph bounding each
effect, or make Table 11 explicitly "representation error only, against the first-order model".

**S4. The sign convention of eq. (5) is unusual and is not stated.**

- With L = ½mv² − mV, the propagation phase of upper minus lower is −(m/ħ)∫(V_u − V_l)dt. Eq. (5) has
  the opposite sign.
- Eq. (5) and eq. (6) agree only if interferometer 1 is the *lower* one, and the paper never says
  which is which.
- Because "positive" and "opposite sign" are headline results, state the convention, and that the
  *relative* sign of the flight and leading reads does not depend on it (both come from eq. 5).

**S5. `:413–414`: "the face term stays in the line class (machine-checked as `numerator_rule`)"** reads
as if the rounding class were machine-checked. `numerator_rule` proves the identity
r₁·(r₂×r₃) = h·|N| over ℝ. Write "the identity is machine-checked as `numerator_rule`; the rounding
class is measured (Fig. 6)".

**S6. The inference criterion: add the parametrisation-free number, and the scaling with data.**

- The paper calls a representation adequate when max |δθᵢ|/σᵢ < 0.1 over four parameters
  (ln M, x₀, y₀, v). That depends on the parameter set: with pose, shape or more towers added the bias
  can grow, up to ‖Δφ‖/σ.
- Report ‖Δφ‖/σ for (c) and (d) beside the four-parameter bias (Table 10, or one sentence). It is the
  number §8 itself bounds by Cauchy–Schwarz, and it is the right comparison with Lindblom et al.'s
  criterion.
- Also say that the ratios grow as √(data): one tower, one pass, 61 shots here. The five-tower array
  or repeated passes tighten every threshold. The paper says this only for the noise level (`:946`).
- `:945–946`: "the flight read is smaller and less degenerate at 1.5 m, so the bounds widen". A less
  degenerate read narrows the bounds. At 1.5 m the bounds widen because the signal is smaller; "less
  degenerate" is the 20 m result.

**S7. Cheinet et al. 2008 "eq. 9".**

- `paper-flight-phase.md` §2 derives |H(f)| from Cheinet's eqs. 6, 9 and 20 of the *arXiv preprint*
  (physics/0510197). Eq. 9 there is |G(ω)| = (4/ω) sin²(ωT/2), the phase response, not the
  acceleration response of eq. (8).
- The note also says the published IEEE TIM version was not read, and its numbering may differ.
- Cite "their eqs. 6, 9, 20 (arXiv numbering)", or check the published numbering.
- Give the time-domain sensitivity in one line: the gradient enters with the weight k_eff·(T − |t − T|),
  the triangle. A reader from the atom-interferometry side will look for it.

**S8. The engine's timing is quoted three ways and not reconciled.**

- The cost model (`:755–756`) uses 55 ns per triangle.
- S5 §7.1 measures 48.5 ns per face (4096 faces).
- `paper-benchmarks.md` Table D measures the same `Faceted::field` at 75–107 ns per triangle.

Say which machine and body each is, or use one figure. The cost verdicts against the whole mesh in
Table 6 move by up to 2× with the choice.

**S9. Data and code availability.** Five of the cited works are research notes "in the Cavendish
repository" (techreport entries), and every table's provenance points into it. A colleague outside
the project cannot follow these. Add a repository URL and commit, or an archive DOI, in a "Data and
code availability" paragraph.

## 3. The real read (§2) in detail

These points are beyond M1, M2, S1 and S2.

- **Eq. (5), first order, unperturbed arms.**
  - It is correct for a closed sequence: instantaneous kicks, uniform g. I checked that the arms meet
    at 2T, both at −1.65 cm, which the engine's floor stops at 0.
  - The arms are not closed once Earth's gradient is included. The paper excludes that gradient
    (`:1046–1047`), so that is consistent.
  - The floor changes the arms' area by 7.5×10⁻⁶ (flight-phase note), which is negligible.
- **Eq. (6), the axis read.**
  - Correct.
  - Σ A_j = ∫Δz dt = v_kick T² holds.
  - The 48 Legendre nodes span [0, max arm height]. The arms never go below 0 because of the floor,
    so no extrapolation is needed.
  - R1 (3.9×10⁻¹¹ against the flight integral) is the right check.
  - "98 field points" = 2 × (48 + 1). Correct.
- **Eq. (8)** is correct for instantaneous pulses: the acceleration response k_eff·4sin²(ωT/2)/ω² is
  k_eff T² sinc²(ωT/2), with ωT/2 = πfT. The aliasing at the 2 s cycle (Nyquist 0.25 Hz) is right.
- **"At 1.5–2 m the instrument does not read a gradient."** It is right that the arms (0–10 m,
  4.8 m apart) see a potential far from quadratic at 1.5–2 m. "Does not read the *launch-point*
  gradient" is fully supported; for the general claim, see S1.
- **Per-shot noise.** 10⁻⁴ rad Hz⁻¹ᐟ² at a 2 s cycle gives 5×10⁻⁵ rad per shot under the one-sided
  convention σ = S/√(2Δt). Say which convention; the other gives 7×10⁻⁵.

## 4. Derivations and prior art (§3–4)

- **The numerator rule.**
  - The identity is correct, and correctly credited to Werner 1994 eq. 3.4, p. 261, where it is
    written per vertex for precomputation. `literature-notes.md` quotes it.
  - The claimed novelty is narrow and accurate: placing the identity in the single atan2, and the εR³
    analysis of the triple product.
  - Holstein 2002: the identity is in App. B, eq. B-2. Its use against far-field cancellation is in
    eqs. 42–43, p. 1131. "[App. B] uses the same identity … against cancellation" (`:412`) merges the
    two; cite "App. B and eqs. 42–43".
- **The edge logarithm.** H&K eq. 24 and p. 361 are correctly credited, for both the accurate artanh
  and the edge length formed directly.
- **The near-edge form.** H&K p. 362, r − |ℓ| = r₀²/(r + |ℓ|), and Okabe eq. 45 are correct. The
  attribution "to avoid a singularity" matches Okabe's own framing (p. 736).
- **H&K Table 4 against Table 3.** Correctly stated: γ⁻²ε and γ⁻³ε on the inclined track, one power
  worse on the horizontal one.
- **Measured growth.** ×116 per decade (ours) against ×880 (ESA) and ×2800 (Harmonica) matches
  `accuracy.json`. It is the ε(R/s)² class against the ε(R/s)³ class.
- **The εR² and εR³ argument.**
  - The rounding of p's own position (εR in r) is a common shift of the field point, so it costs only
    ε relative. That is consistent with the dashed floor in Fig. 6, right.
  - One addition would help: the error analysis holds per face. The sum over many faces adds rounding
    as √F (as §3.1's f32 paragraph says). The f64 table shows Spot (5856 triangles) at 2.1×10⁻¹¹
    against 7–10×10⁻¹² for the small bodies.
- **§4, the double layer.** The algebra is correct. The averaging over the offset s assumes the
  offsets are equidistributed over patches many cells wide: the surface is locally flat on the cell
  scale, and its normal is not commensurate with the lattice. Say so in one clause; it is why
  aligned faces and curvature leave residues.
- **Table 1 and eq. (2).** The orders and the 1/4/10 numbers per cell are right. The remainder
  O(ρh⁶/d⁴) is per cell.

## 5. The Lean section

I spot-checked `Edge.lean`, `SolidAngle.lean`, `Polyhedron.lean`, `Box.lean`, `BoxFaces.lean` and
`Superposition.lean` against Table 12. Every name exists and states what the table says:

- `edge_integral` is stated in an arbitrary real inner-product space `E`, with the point off the line.
- `numerator_rule` and `height_const` are pure algebra.
- `vos_modulus` and `vos_ne_zero` are there. The converse ("a factor vanishes only on a closed edge")
  is correctly marked as not proven.
- `box_potential`, `box_attraction` and `box_gauss` are proven from Mathlib's divergence theorem on
  boxes. Their only hypotheses are `a ≤ b` and `p ∉ Icc a b`.
- `rect_face` and `rect_edge` match.
- The theorems that compose the box face and edge steps are absent, as the paper says.
- `potential_faces_edges`, `attraction_faces_edges` and `solid_angles_sum_zero` take
  `DivergenceTheorem V K F` and `GreenTheorem` as hypotheses. `Setting` requires K compact with V ⊆ K,
  the point outside K, the face points in K, and the point off every edge's line (`hoff` is the
  squared distance to the line). Orthonormal frames come via `Face.parseval`. All as the paper says.
- `phase_sum`, `quadrature_error_additive` and `exact_pieces_exact_phase` are linear in V with the
  arms fixed, as stated.

Three small points:

1. **"Every main theorem" (`:997`).** `PolyhedralGravity.lean` prints axioms for 17 theorems.
   `phase_sum`, `height_const` and `integral_pieces`, which Table 12 names, are not among them (they
   are used by listed ones, or are trivial). The output itself is not committed. Commit the
   `#print axioms` log, or say "on the 17 theorems listed in `PolyhedralGravity.lean`".
2. **The `DivergenceTheorem` hypothesis** quantifies over fields that are merely differentiable on a
   neighbourhood of K, with integrable divergence; it does not ask for C¹. That is slightly stronger
   than the textbook statement, though still true (Gauss–Green for differentiable fields, after
   Pfeffer). "The classical statement" (`:1032`) should say "for differentiable fields with integrable
   divergence, as Mathlib's box version", so that a reader does not suspect the hypothesis is false.
3. **`edgeLog` is the ratio form, not the `log1p` form.** The edge used by `potential_faces_edges` is
   `edgeLog`, the ratio form ln((r₁+r₂+ℓ)/(r₁+r₂−ℓ)). `edge_integral_log1p` proves the log1p form
   equal to it separately. That is fine; the table could say "the closed form is proven with the ratio
   form; `edge_integral_log1p` gives the equal log1p form".

## 6. References (Crossref)

**All 56 DOIs resolve, and the titles, first authors, volumes and pages match the bib entries.**

- These are online-first entries whose bib year is the issue year, correctly: heck2007 (online 2006),
  werner2017 (2016), conway2015 (2014), durso2013 (2012) and durso2014 (2013).
- werner1997: Crossref has no pages; the bib's 313–344 is the published range.
- crane2013: "61:1–61:10" is article 61; Crossref gives 1–10.

Entries without a DOI:

- **hogan2008**: arXiv only. It was published in the Proceedings of the International School of
  Physics "Enrico Fermi", Course 168 (IOS Press, 2009). Cite the published version (the DOI I tried
  was not in Crossref, so this is not DOI-checked).
- **carlton2023**: arXiv only. Check whether it has since been published.
- **zhou2016**: arXiv. Its DOI 10.48550/arXiv.1605.04797 is registered with DataCite, not Crossref.
- **darnell2011**: a magazine column, with no DOI to expect.

13 entries are not cited (aftosmis1998, banerjee1977, catmull1978, colella2006, durso2013,
greengard1987, li1998, muller2013, plouff1976, saye2015, springel2005, werner1997sh, wittick2019).
natbib does not print them, but they should be pruned.

## 7. Claims against evidence

**Numbers traced and found to match.** Each entry gives the source in brackets.

- **§2, the real read:**
  - +6.5 against −4.0 rad for Spot; 0.944–0.951 of the frozen peak; FWHM 2.4 → 2.6 s; 22–25 % high
    at z_eff = 2.17 m (`paper-flight-phase.md` §3.1).
  - 29/94 m and 15/48 m for the leading read (§3.1).
  - vT/d ≲ 0.2 / 0.8; walker 3–5 %; car 8–10 % (§3.2).
  - 1.8×10⁻¹⁵ rad against `cav run` (C1); 8.9×10⁻¹⁶ and 1.1×10⁻⁵ for the transfer function (C2).
  - 44–68 m and 640 m on the paper's ray; 5.5 % at 100 m; 1.5 % at 400 m (`paper-real-read.md` §4,
    Q5).
- **§3, the closed form:**
  - ×116, ×880 and ×2800 per decade; 7×10⁻¹²–2×10⁻¹¹ at 100 sizes; 0.9–1.6×10⁻¹⁰ for the plate;
    ESA 2.3×10⁻⁹–4.3×10⁻⁸ and 3.8×10⁻⁷ (Spot); Harmonica 6×10⁻⁹–5×10⁻⁸ (`accuracy.json`).
  - Near edge: 3×10⁻⁶ at 10⁻⁸ m, ∞ at 10⁻¹⁰ m, and 5.9×10⁻¹¹ with the near-edge form
    (`data/nearedge.txt`).
  - P1–P7, 48.5 ns, 5.0–6.4 µs and 46–59 µs (S5 §7.1, §7.6).
  - ESA 824–953 ns; Harmonica 460–673 ns (Table D).
- **§4 and Table 4** (`data/level/conv.json`, regenerated):
  - every order (1.0/1.1/2.1/3.0/4.9 on the aligned cube; 3.5–4.2 and 3.1 for the fill; 2.8–3.8;
    3.7–4.9);
  - \AlignedSampled, \GainCentroid (1.03×10⁴) and \GainSecond (1.85×10⁸);
  - \ArmClearance = 0.876 m (the plate);
  - Cavendish's ladder 0.95/1.99/2.96/4.92 (S5 §7.6).
- **§5, the shapes:**
  - (d) orders 3.6–3.9 and 4.5; 1.7×10⁻¹¹–2.9×10⁻¹⁰; (c)/(d) 14–4040 and 95–4040; (e) ≤ 3.5×10⁻¹¹.
  - Sphere placement: 3.6–5600×; (d) beats (c) 1.4–3.5×; 2.1/2.8×10⁻⁸ at 128.
  - Facets: 0.19/0.43 rad and 0.012/0.0068 rad; (d★) 2.8/6.7×10⁻⁸.
  - Table 7, the regimes; Table 8's walk rows and the point mass's departures 0.5–4 %, 3–9 % and
    2–6×10⁻⁴.
  - The orientation numbers.
  - The C6 sphere result: 3 of 3 pitches (S5 §7.7).
- **§8, the budget** (`tables-budget.md`):
  - CRBs 1.3×10⁻⁵–1.8×10⁻⁴ and 2.6–33 µm; σ(M)/M at 20 m of 1.4/5.8/20 %.
  - (c) thresholds 46–64 mm and 72–118 mm.
  - Physical rows: 57–800, 3.7–390, 0.3–30 and 56–1800 σ; 640 σ.
  - (c) and (d) at 1.6×10⁻³ and 1.5×10⁻⁶ σ; sampled 0.7–710 σ.
  - V1: 0.010000; 1.0000 cm; 0.994–0.997 cm; the 30 cm control misses by 9–16 %.
- **§9:** 1.3×10⁻⁶ relative for the icosahedron with pieces within three cells (S5 §7.7, C3: 1.34×10⁻⁶).

**Measured, modelled and derived labels.** These are used correctly throughout:

- Table 6 and the octree-merge sentence are \tagMo.
- The isotropy argument and Table 1 are \tagD.
- The (d) world-frame line (≈5×10⁻¹¹) is explicitly "an estimate, not measured".
- One gap: §6's fourth bullet is labelled \tagM/\tagMo. Say which half is which: the l = 6 series
  slopes are measured; the "20–110 times cheaper" is modelled.

**Smaller mismatches** (minor; listed so they do not survive into draft 5):

- `:62`, "a body walking past at 2 m gives 1.6–1.9 times more": true upright. Over all three poses it
  is 1.4–1.9 (Spot turned: 6.75/4.68 = 1.44; CAD part on its side: 3.30/2.31 = 1.43).
- `:263`, "1.7–2.8 times larger" in the level geometry: true for the worst direction's magnitudes. Per
  direction, the plate reaches 5.05 (4.25 against −0.84 rad).
- Table 4 prints the signals unsigned. The flight read is +43 rad at 1.5 m and −0.83 rad at 10 m: it
  changes sign between the two. A sign column would help.
- Table 11's caption says "a tonne-scale body … worst body", but the worst body for fill + centroid is
  the plate (135 kg).

**The new tower geometry (`scripts/conv_level.py`).** It is correct, and the "outside the
circumsphere" argument holds for every body of §4:

- The three directions are projected onto the horizontal. None is vertical, so none degenerates.
- The lower launch point is 1.5 m (or 10 m) from the body's centre, and the body is centred on the
  origin (`skin_bench.body`). The lattice, not the body, moves between placements.
- The arms are vertical and never go below the launch height (the floor). So every arm point is at
  least 1.5 m from the centre.
- The circumradii are: aligned and turned cube 0.693 m, icosahedron 0.60 m, icosphere 0.50 m, plate
  0.708 m.
- The measured surface clearance is 0.876–1.07 m.

Three remarks:

1. **"More than twice the largest circumradius" (`:556`) is more than the argument needs.** The
   body's multipole series converges for any distance greater than the circumradius. The per-cell
   expansions, which are what (c) and (d) truncate, converge on the scale of the cell, not the body.
   Say "outside the circumsphere (1.5 m > 0.71 m)".
2. **The clearance is sampled at 41 heights 0.25 m apart.** For the turned plate the nearest point may
   fall between samples, which can overstate the clearance by up to about 1 cm. It is printed as 0.88,
   so this is harmless; or minimise over z exactly.
3. **The argument does not carry over unchanged to §5, §7 and §8**, which use other geometries.
   - §5's 17° ray: the axis passes 1.31–1.50 m from each mesh. For the 1 m meshes, whose centres lie
     inside them, the distance to the centre is at least the distance to the surface, so the axis is
     outside the circumsphere (at most about 0.87 m). For the torus, whose centre is in the hole, the
     axis passes beside the outer rim, about 2.7 m from the centre, against a circumradius of 1.4 m. ✓
   - The regimes table (Table 7) goes down to 0.5 m from the surface, where the arms can be inside a
     1 m mesh's circumsphere. Only row (f), the body multipole, is affected; its crossing points are
     far out (7–11 m), but its error at the near grid points is that of a divergent series. One
     clause would cover it.
   - §8's track (centre 1 m below the launch, 1.5 m from the surface) and the 2 m walk past are
     outside. ✓

## 8. Presentation

- **Structure.** The order (problem, the real read, closed form, stability, benchmark, shapes, cheaper
  leaves, inference, Lean, limitations, future work) reads well. §2 before §3 is a good choice: the
  reader learns what is being read before the kernels.
- **Length.**
  - The paper is 24 pages with 12 figures and 11 tables, and the abstract is about 330 words with
    some 25 numbers.
  - For a journal I would move §3.1 (the engine), §6 (cheaper leaves, from a separate note) and
    Appendix B to supplementary material. That would cut the abstract to the three contributions with
    one number each.
  - Remarks about draft 3 (`:254`, `:557–560`, `:620–623`, `:729–730`, `:840–841`) belong in a change
    note to the colleague, not in the paper.
- **Figures.**
  - Fig. 5 (`fig:cuts`) is cited at `:133`, before Fig. 1, so the figure numbers run out of citation
    order. Cite Fig. 1 first, or move the cuts figure forward.
  - Fig. 3's caption uses (c) and (d) both for panels and for techniques ("(c) A torus … (c) fill +
    centroid and (d) cut-cell moments errors … (d) The torus's lines"). Use "fill + centroid" and
    "cut-cell moments" in panel (c) and keep the letters for the panels.
  - In Fig. 4(b) the subscripts of the near-edge identity are hard to read at normal zoom ("b + ℓ₂"
    reads as "b + ε₂" at 60 dpi). Enlarge them.
- **Captions.** They are good and self-contained, and the "Look at …" sentences help a first reader.
  The \provisional tags are clear.
- **Wording.**
  - `:731`, "within 3×10⁻³ of a cell of the sphere's centre" should be "within 3×10⁻³ h of the
    sphere's centre" (as `:638` says).
  - `:946`: see S6.
- **Status box.** The labelling scheme (\tagM, \tagMo, \tagD, "leading read") is exactly what a
  colleague needs. Keep it.

## 9. Draft-3 items seen again

- **Draft 3's M1 and M2** (the 40-digit numbers at 100 sizes, and ESA's lower bound) are fixed. The
  near-body figures at `:464–466` are now wrong instead (M4 above).
- **Draft 3's S2** (time-averaging over the shot) is answered thoroughly by the flight read.
- **Draft 3's S1** (the sphere "open question") is resolved as a placement effect, with an independent
  engine test (C6). This is convincing.

---

## Response to the referee (draft 5)

Draft 5 (`paper.tex`, 27 pages) answers every item. Line numbers are draft 5's `paper.tex`. Each
changed number was re-traced to its source; where my trace differs from the report, the row says so.

**Must-fix**

| item | what changed | where |
|---|---|---|
| M1 | The laser and separation phases change at first order and cancel the propagation phase's end terms; eq. (2) on the closed unperturbed arms is the whole first-order phase. The uniform-δg example is added. | §2, the flight integral (:230–236) |
| M2 | ≲ 0.05 above 1/T (first sidelobe 0.047 at 1.96 Hz), falling as (πfT)⁻². Near a source the response departs from sinc² by 0.3–3 (flight/frozen 0.154 at 2 Hz). Recomputed: sinc² itself stays below 10⁻² above **3.7 Hz**; its envelope does so above 4.4 Hz. The text gives both. | §2, moving sources (:313–318); Fig. 3 caption (d) |
| M3 | 1.6–2.3 for the 1 m meshes (Buddha 1.56, Armadillo 2.26, from `paper-real-read.md` §2); 3.3 for the 2 m sphere; 4.3 for the 2.8 m torus. The abstract gives 1.6–2.8, smaller or larger with the geometry. | abstract (:58–66); §2 (:283–287) |
| M4 | Ours: within 4×10⁻¹⁴ (3.8×10⁻¹⁴ on the turned plate, the value Table 12 uses). On every other body it is **1.7×10⁻¹⁴**, not the suggested 1.5: the aligned plate, Python, in `accuracy.json`. ESA: 1.2×10⁻¹³ on the benchmark bodies, 2×10⁻¹³ on the aligned plate, 7.6×10⁻¹³ on Spot. Harmonica: 6×10⁻¹⁴. | §4 (:560–565) |
| M5 | (d) is adequate at 220 mm or coarser. The cube passes 250 mm at 0.049 σ and the CAD part at 0.090 σ; the bunny fails 250 mm at 0.18 σ and passes from 220 mm. The Fig. 11 caption is fixed. | abstract; §8 Thresholds (:1061–1063); Fig. 11 caption |
| M6 | The sampled fill needs 0.1–4.4 mm, extrapolated: it does not converge in the range run. | abstract |
| M7 | 60–13 000, from `data/level/conv.json` at 1/64 m: 59 for 0/1 on the turned cube up to 13 166 for the plate's second moment. The paragraph also says that the flight read changes sign between 1.5 m and 10 m. | §5 (:722–725) |

**Should-fix**

| item | what changed | where |
|---|---|---|
| S1 | "Opposite sign" is now relative to the gradient between the launch points. Added: g_z there is near zero by symmetry; the gradient at the arms' mean height is the better reference, but is still 22–25 % high for the walker at 2 m. "Not the leading term of anything" is gone. | abstract; §2 (:283–295) |
| S2 | The parameters are labelled as Cavendish's model of AION-10's goal configuration: ⁸⁷Sr; n = 1000 is the goal and n = 100 the initial configuration (Badurina et al. Table 1, which takes T = 1.4 s); T = 0.73 s, u₀ = 3.86 m/s, Δr = 5 m and the 2 s cycle are Carlton & McCabe's choices. Checked against `docs/research/s15-instruments-anywhere.md`, which read Badurina's Table 1. Every near-field result scales with the arm separation ∝ n; at n = 100 the arms separate by 0.48 m. | abstract; §1 (:91–99); §2 (:244–250); limitations |
| S3 | Both changes were made. (1) A new paragraph and Table 2 ("What the model leaves out") bound four effects for point-mass stand-ins of the 2.7 t and 1.4 t cubes at 1.5 m. The script is `scripts/omitted.py` (`make.py omitted`, output in `data/omitted.txt`). (2) Table 12 is relabelled "representation error only, against the first-order model". These effects are common to all representations; through a representation's error they contribute below 10⁻⁹ rad (fill + centroid) and 10⁻¹² rad (cut-cell moments). §8 now says that its absolute claims need rotation and the real pulses in the model. The values, with how they differ from the report's estimates, are listed below the table. | §2 (:328–375); Table 12 caption; §8 (:1088–1090); limitations |
| S4 | The sign convention is stated: interferometer 1 is the lower one, and δφ_k is the propagation phase of its lower arm minus its upper arm. The headline signs are relative signs of two reads from eq. (2), so they do not depend on the convention. | §2 (:238–242) |
| S5 | The identity is machine-checked as `numerator_rule`; the rounding class is measured (Fig. 6). | §4 (:506–508) |
| S6 | ‖Δφ‖/σ < 0.1 pitches are added to Table 11 from `budget.json`, generated by `realread.py`. At 1.5 m: (c) 21–62 mm, (d) 161–189 mm. At 1/64 m the norm is (c) 2×10⁻³–4×10⁻² and (d) 1–14×10⁻⁶. Stated: both criteria grow as √(data) (one tower, one pass, 61 shots), and the five-tower array or repeated passes tighten them. The "smaller and less degenerate" sentence is fixed: at 1.5 m the signal is smaller (44 against 65 rad) and the residuals shrink; "less degenerate" is the 20 m result. | §8 (:1020–1026, :1061–1072); Table 11 |
| S7 | Cites Cheinet's eqs. 6, 9 and 20 in the arXiv numbering (physics/0510197). States the triangular weight ΔΦ = k_eff Δr ∫(T − \|t − T\|)Γ dt, in eq. (5). | §2 (:298–311) |
| S8 | 55 ns is the shapes study's compiled Rust `mesh_bench`, one thread, per-triangle data precomputed; it measured 53–67 ns from 5 000 to 10⁶ triangles. The brief called it "numpy", but `exact-skin-and-partial-fill.md` §3 shows it is compiled; the study's numpy costs 237–1098 ns. 48.5 ns is the engine kernel's own diagnostic (S5 §7.1, 4096 faces) and is the figure to quote for the kernel. 75–107 ns is the public `Faceted::field` call (two allocations per call, below-normal priority; `paper-benchmarks` Table D). Checked: no verdict of Table 7 changes anywhere from 48.5 to 107 ns. Suzanne's whole mesh, at 121 µs, still beats (d)'s 154 µs. | §4.1 (:603–607); §6 Error against cost (:862–868) |
| S9 | A "Data and code availability" section: the public repository URL at the build commit; the notes, scripts, data and proofs are in it and regenerated by `make.py`; the meshes are downloaded from pinned sources and SHA-256-checked, not redistributed. The techreport entries carry the repository URL. | before the appendices (:1218–1228) |

**S3, the bounds** (`data/omitted.txt`; first-order signals of the stand-ins +42.4 and +43.3 rad,
which check against the paper's 44 and 43.4):

- **Second order in the source: 2.3 and 3.9×10⁻⁷ rad.** The formula is derived in the draft as
  eq. (6), from the action expanded to second order with the laser/kick and separation terms. The
  report's (m/ħ)|δa|²T³ ≈ 10⁻⁶ is an upper bound; the arm-difference structure cancels most of it.
- **Earth's rotation: 1.0×10⁻³ rad with the body east or west**, from the read along
  Coriolis-deflected arms at 51.8° N; the deflection reaches 0.39 mm.
  - With the body north or south it is ~10⁻⁶ rad (second order). This matches the report's estimate
    for that case.
  - East or west it is three orders larger than the report's 10⁻⁶–10⁻⁵. It equals moving the body
    by about 0.1 mm, so the report's "bias of order 100 µm on the fitted position" holds.
  - Not included: the separation phase of an uncompensated sequence's 0.32 mm gap, which depends on
    how the real sequence closes the arms.
- **Earth's gradient: 4.5×10⁻⁵ rad.** The arms open by 7.9 µm.
- **Finite pulses: τ/T = 1.4×10⁻³–6.7×10⁻²** of every signal for 1–50 ms ramps, which is 0.06–3 rad
  on 44 rad, as the report estimated.

**References**

- **hogan2008**: now the published chapter. It is in *Atom Optics and Space Physics*, Proc. Int.
  School of Physics "Enrico Fermi" Course 168, IOS Press 2009, pp. 411–447, DOI
  10.3254/978-1-58603-990-5-411.
  - The DOI is registered with mEDRA, not Crossref. Crossref's search does not find the chapter.
  - The DOI handle resolves (doi.org handle API), and IOS Press's volume page gives the pages.
- **carlton2023**: Crossref shows it was published as Phys. Rev. D 108, 123004 (2023), retitled
  "Mitigating anthropogenic and synanthropic noise…", DOI 10.1103/PhysRevD.108.123004.
  - The entry is now the article, with the arXiv number in a note.
  - The text says the equation numbers are the arXiv version's. The published numbering was not
    checked.
- **Uncited entries**: 12 of the 13 are now cited where they belong:
  - banerjee1977, plouff1976, li1998, werner1997sh, durso2013, wittick2019, greengard1987 and
    springel2005 in §1.1;
  - aftosmis1998 and colella2006 at "cut cell";
  - muller2013 and saye2015 at (c★)/(d★);
  - werner1997sh also at (f).
- **catmull1978** (anti-aliasing) is removed. The bib now has 64 entries, all cited.

**Minor items**

| item | what changed |
|---|---|
| §3 per-shot noise convention | Stated: one-sided σ = S/√(2Δt); the two-sided convention gives 7×10⁻⁵ (:99). |
| §4 Holstein 2002 | "App. B, eq. B-2" for the identity; "eqs. 42–43" for its use (§1.1 and §4). |
| §4 rounding adds over faces | One sentence added (:508–511). Measured, the face count is not the main factor: Spot is at 2.1×10⁻¹¹, the 12–60-face bodies at 0.8–1.4×10⁻¹¹, and the 1280-face icosphere at 6.8×10⁻¹². The √F story is therefore not claimed as the explanation. |
| §4 double layer | The equidistribution assumption is stated, and why aligned faces and curvature break it (:701–703). |
| §4 remainder O(ρh⁶/d⁴) | "per cell" (:116). |
| §5.1 "every main theorem" | Now "on the 17 theorems listed in `PolyhedralGravity.lean`" (checked: 17 `#print axioms`). The log is still not committed: building needs the Mathlib cache. |
| §5.2 `DivergenceTheorem` | Stated as for differentiable fields with integrable divergence, as Mathlib's box version is; stronger than the C¹ textbook statement, still true (§9). |
| §5.3 `edgeLog` | Table 13 says that the closed form uses the ratio form and that `edge_integral_log1p` proves it equal to the log1p form. |
| §7 tags of §6's fourth bullet | Split: the l = 6 slopes and costs are \tagM; "20–110 times cheaper" is \tagMo. |
| §7 "1.6–1.9 times more" | 1.4–1.9 over all three poses (abstract); Fig. 3 caption 1.4–1.6 for Spot. |
| §7 "1.7–2.8 times larger" | Now "in the worst direction; up to 5.1 in one direction for the plate". |
| §7 Table 4 unsigned | The signal row now prints the worst direction's flight read with its sign and the leading read in brackets (`realread.py`). |
| §7 Table 11 (now 12) caption | Names the worst body per row: aligned cube for the sampled fill, turned cube for (d), the 135 kg plate for (c). |
| §7 circumsphere | Now "outside the circumsphere (1.5 m against 0.71 m)"; the clearance is noted as sampled every 0.25 m. |
| §7 row (f) near the body | A clause: at the nearest grid points the arms can enter a 1 m mesh's circumsphere, where (f) is a divergent series (§6). |
| §8 remarks about draft 3 | Removed from the paper (:254, :557–560, :620–623, :729–730, :840–841), and reworded without "draft 3" where the content is needed. |
| §8 figure order | The forward reference to the cuts figure in §1 is removed; it is now cited in §3 (:461), so figures are cited in order. |
| §8 Fig. 3 caption letters | Panel (c) says "fill + centroid and cut-cell moments"; letters are panels only. |
| §8 Fig. 4(b) | The near-edge identity is written inline at a larger size (`diagrams.py`); ℓ₁ and ℓ₂ read at print size. |
| §8 "3×10⁻³ of a cell" | Now "3×10⁻³h of the sphere's centre" (:840). |
| §8 length | Not restructured. Moving §3.1, §6 and App. B to supplementary material is a venue decision for the author. Draft 5 is 27 pages: Table 2 and the availability section added length, and a smaller bibliography font took some back. |
