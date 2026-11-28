# Draft 6: the restructure

Draft 6 is draft 5 put in a different order. No number or claim has changed. Text was moved, merged,
or given new transitions. Draft 5 was 27 pages, and so is draft 6.

## 1. Draft 5 read once, in order: where the argument breaks

**§1 "The problem" does four jobs and finishes none of them.**
- It opens with the instrument's parameters, which belong in §2.
- It then derives the error orders (eq. 1, Table 1) before the reader knows what a cut cell is.
  "Cut-cell moments" are defined in a paragraph *after* Table 1, which already uses them.
- It restates prior art for cut cells, and then states the novelty twice: "What is new here is…"
  in that paragraph, and claim (ii) of §1.1.
- It defines the facet and voxel errors by reference to bodies in §5 and §6.
- There is no question and no key result. A reader gets to §2 without knowing what the paper
  concludes.

**Results come before their method.**
- §2's leading-read paragraph quotes figures for the Buddha, the Armadillo and the icosahedron, in
  geometries defined in §5 and §6.
- §2's "What the model leaves out" sizes the effects for "the bodies of §§5 and 8" and compares them
  with representation errors from Table 5 (§5). The reader has seen neither.
- Fig. 3 (c, d) shows voxel errors of a torus in §2, before any representation is defined. This
  remains (see §3 below), but the techniques are now named in the introduction.

**The closed form is spread over three distant places.**
- §3 gives the derivation and §4 its stability.
- §4 then compares it with other codes, and §4.1 tests the engine.
- The Lean proof of the same derivation is §9, five sections later, after the error budget.

**Cost is spread over four places.**
- §4.1 gives kernel timings, clipping cost and single precision, all inside the stability section.
- §6 "Error against cost" models cost inside the shapes section.
- §7 measures cheaper leaves.
- §11 adds walk cost.

**"How good is good enough" (§8) comes after "cheaper representations" (§7).** The reader is told
which leaves are cheaper before learning what accuracy is needed.

**Ideas repeated in several places:**
- **The letters (a)–(e).** They are defined twice: in Table 1 (§1) and again in §6 ("The
  techniques").
- **"The mesh, not the voxels, limits the accuracy."** It is argued twice in §6: once in "The facet
  error dominates" and again in "Each mesh as one polyhedron", with the decimation failure in a
  third place, "Meshes, repaired and checked".
- **The isotropy argument.** This derivation is buried in the *second finding* of the measured
  benchmark (§5). It is cited from §6 as "the isotropy argument of §5".
- **The Earth's-rotation caveat.** It appears three times: in §2, in §2's closing paragraph, and in
  §8's physical-uncertainty paragraph.

**Caveats interrupt the flow or repeat themselves.**
- The mesh-repair procedure (17 lines) sits in the middle of §6's results.
- The Lean section's "two remarks" on Mathlib details interrupt the argument.
- §10, "Limitations", repeats about two-thirds of what is said in place: the provisional status, the
  read model, the omitted effects, the cost model, the decimations, and Lean's hypotheses. It adds
  four items that are stated nowhere else.

**Figure and table numbers are out of citation order.** For example:
- The cuts figure (old Fig. 5) is needed in §4 (representations) but was placed in §3.
- Table 12 (the budget) is cited from §4 and from Table 3's text.
- The status box cites Tables 1, 2 and 13 by number before any of them.

## 2. Old and new section orders

| draft 5 | draft 6 | page (d6) |
|---|---|---|
| Abstract, status box | Abstract (reordered to the new line; shorter), status box | 1 |
| 1 The problem | **1 The question**: question, approach, the answers in brief (one bullet per section) | 1 |
| 1.1 Related work, and what is new | 1.1 Related work (now also holds the cut-cell prior art) | 2 |
| 2 What the instrument reads | **2 What the instrument reads**: model parameters (from old §1), flight integral, axis read, leading read and where it holds, moving sources; Fig. 2 | 3 |
| 3 The closed form and its derivation | **3 The exact field of a polyhedron** | 5 |
| | 3.1 The closed form and its derivation (old §3) | 5 |
| 4 Numerical stability | 3.2 Stable evaluation in double precision (old §4: far field, near edge, on the surface) | 6 |
| (in 4) Against independent implementations | 3.3 Against independent implementations and 40 digits | 8 |
| 4.1 The engine's kernel | 3.4 The engine's kernel (accuracy only; timings moved to 6.1) | 8 |
| 9 The Lean formalisation | 3.5 The derivation, machine-checked (Mathlib remarks moved to App. B) | 9 |
| | **4 Representing a body on a grid** | 9 |
| (in 1) cut-cell moments, Table 1, Figs. 2, 5, two errors | 4.1 What a boundary cell carries: definitions, letters (a)–(e) defined once, clipped pieces, facet error and voxel error | 9 |
| (in 1 and 5) eq. 1 orders, isotropy derivation | 4.2 Error orders, derived: the Taylor ladder; why turned faces do better | 11 |
| 5 Cut-cell moments: the benchmark | 4.3 Measured on five simple bodies | 12 |
| 6 Curved, scanned and CAD shapes | 4.4 Measured on ten shapes (sweep, sphere and placement) | 13 |
| (in 6) near/middle/far, orientation and motion | 4.5 Distance, orientation and motion | 16 |
| (in 6) facet error, each mesh as one polyhedron, decimation | 4.6 The mesh, not the voxels, limits the accuracy | 16 |
| 8 How accurate is enough | **5 How good is good enough: the error budget** (opens with Table 10) | 18 |
| | 5.1 The bias on a fitted body | 18 |
| | 5.2 Against what is not known about the body | 19 |
| (in 2) What the model leaves out | 5.3 Against what the model leaves out (Table 12) | 19 |
| | **6 Cost and cheaper representations** | 21 |
| (in 4.1) kernel timings, clipping cost | 6.1 What the kernels cost | 21 |
| (in 6) Error against cost, octree merging, free boxes | 6.2 Error against cost | 21 |
| 7 Cheaper representations near the body | 6.3 Cheaper leaves near the body | 22 |
| (in 4.1) Single precision | 6.4 Single precision | 23 |
| 10 Limitations; 11 Future work | **7 Limitations and future work**; 7.1 the world octree | 23 |
| Data and code availability | Data and code availability | 23 |
| App. B The Python cross-check | App. A The Python cross-check (now cited from §3.3) | 24 |
| (in 9) two remarks | App. B Notes on the Lean formalisation | 24 |
| App. C Mesh sources | App. C Mesh sources and repair (repair procedure moved from §6) | 24 |
| App. A Reproducing this paper | App. D Reproducing this paper | 24 |
| References | References | 25–27 |

## 3. Figures and tables, renumbered in citation order

Nothing was cut or merged. Every float is placed in the source just after its first citation.

| draft 5 | draft 6 |
|---|---|
| Fig. 1 representations | Fig. 1 |
| Fig. 3 flight read | Fig. 2 |
| Fig. 4 closed-form ingredients | Fig. 3 |
| Fig. 6 far field / near edge | Fig. 4 |
| Fig. 7 40-digit cross-check | Fig. 5 |
| Fig. 2 cut-cell moments | Fig. 6 |
| Fig. 5 one cell, many cuts | Fig. 7 |
| Figs. 8–12 | Figs. 8–12 (unchanged) |
| Table 3 codes vs 40 digits | Table 1 |
| Table 4 engine checks P1–P7 | Table 2 |
| Table 13 Lean | Table 3 |
| Table 1 orders | Table 4 |
| Table 5 benchmark | Table 5 |
| Table 6 sweep | Table 6 |
| Table 8 regimes | Table 7 |
| Table 9 each mesh as one polyhedron | Table 8 |
| Table 10 decimation | Table 9 |
| Table 12 budget phase errors | Table 10 |
| Table 11 thresholds | Table 11 |
| Table 2 omitted effects | Table 12 |
| Table 7 cost | Table 13 |

Fig. 2 (flight read) still has panels (c) and (d), which show torus voxel errors. It is a single
rendered PDF from the flight-phase note, and splitting it would mean re-rendering. It stays in §2,
where (a) and (b) are first cited, and §4.5 cites (c) and (d).

## 4. What was merged, dropped or reworded

None of these changes alters a number or a claim. Every sentence of draft 5 is either still in the
paper, word for word apart from the edits listed here, or covered by an item below.

**Rewritten from scratch:**
- **The abstract.** It now follows the new order. It is shorter, and it adds a closing sentence on
  cost taken from §6.2. These details are no longer in the abstract but remain in the body:
  - 22–25% at the mean height, and 1.4–1.9 times for the walk past (§2);
  - 1.6×10⁻¹⁰ for the 5 cm plate (§3.3);
  - the Lean details on the box and the hypotheses (§3.5).
- **The introduction.** It now has the question, the answers in brief, and a pointer to each
  section.
- **The section openers.**

**Merged or dropped:**
- **"What is new here is their use as the source of a gravity-gradient forward model…"** was
  dropped. It repeated claim (ii) of §1.1, which stays.
- **The draft 5 limitations list was dissolved.** Each item now lives where it applies, or was
  already stated there:
  - Provisional status, and "where Cavendish measured the same it agrees": the status box.
  - "The real sequence is not public in that detail": §2.
  - "No tree walk, cache or GPU": §6.2.
  - "Physical rows are biases, not post-fit misfits": §5.2.
  - "Curved analytic shapes have no clipped pieces…": §4.6.
  - "The full scans enter only through the decimation rows": §4.6.
  - Edge extension, and "∫r/r³ = ∇U not formalised": §3.5.
  - Read-model parameters, omitted-effect sizes, cost-model and decimation failures: dropped from
    the list, because they are already stated in §2, §5.3, §6.2 and §4.6.
  - What §7 keeps: the study's scope (ten shapes, three bodies, one tower) and the leading-read
    results that were not recomputed.

**Caveats now stated once:**
- **Earth's rotation.** In §5.2, the clause "Earth's rotation alone moves the read by about as much
  as 0.1 mm of position (Table 2)" is replaced by a pointer to §5.3. §5.3 states the same fact
  with its number.
- **Mesh repair and decimation.** The repair procedure moved to App. C. The decimation criterion
  and its failure for the dragon and the Buddha moved into §4.6, where the decimated meshes'
  signal change is measured.

**Small rewordings to keep sentences true after the moves:**
- "the value Table 12 uses" became "the value the budget of §5 uses".
- "the isotropy argument of §5" became §4.2.
- "the artefact above" became "the placement artefact of §4.4".
- "kernel measures 48.5 ns (§4.1)" became §6.1.
- "the position rows of §8" became §5.2.
- The status box names its tables by section instead of by number.

**Labels:** every draft-5 label is kept and still points at the same content. New labels mark the
new subsections.
