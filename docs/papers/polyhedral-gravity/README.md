# Exact gravity of polyhedral boundary cells (draft paper)

Malachy Doherty (affiliation to be filled in). **Draft for comments, not submitted. Whether, where
and with whom to publish is the author's decision.**

`paper.tex` + `refs.bib` → `paper.pdf` (built with tectonic). Draft 4 (28 Nov 2026) covers the
polyhedral closed form, its numerics and its Lean proof, the boundary-cell representations, and the
signal the interferometer actually reads; the world octree, the layered read and the tree walk are
left to a short future-work section until more of Cavendish is built. The measurements are from the
research notes [`paper-real-read.md`](../../research/paper-real-read.md),
[`paper-flight-phase.md`](../../research/paper-flight-phase.md),
[`paper-error-budget.md`](../../research/paper-error-budget.md),
[`paper-benchmarks.md`](../../research/paper-benchmarks.md),
[`paper-boxes-and-precision.md`](../../research/paper-boxes-and-precision.md),
[`surface-representation.md`](../../research/surface-representation.md) and
[`exact-skin-and-partial-fill.md`](../../research/exact-skin-and-partial-fill.md) §3, and from
Cavendish's own S5 results ([`docs/slices/S5.md`](../../slices/S5.md) §7); the derivation is
machine-checked in [`proofs/polyhedral-gravity/`](../../../proofs/polyhedral-gravity/). The referee's
report on draft 3 is [`REVIEW-draft-3.md`](REVIEW-draft-3.md).

## Rebuilding

```text
python make.py all                 # summaries, far-field, near-edge, cross-check, 40-digit reference,
                                   # scenes, diagrams, renders, real-read tables and figures, PDF
python make.py level               # rerun the benchmark with the tower beside each body (~10 min;
                                   # needs `cargo build --release` in docs/research/paper-real-read/kern)
python make.py realread-rerun      # rerun the real-read and flight-phase notes' analysis and figures
python make.py bench               # rerun draft 3's leading-read benchmark (~40 min)
python make.py esa <venv-python>   # rerun ESA's oracle in its own venv, then crosscheck + mp
python make.py shapes-study        # rerun the shapes study itself (hours; downloads the meshes)
```

Every phase in draft 4 is the flight-integral read (the static axis-weight form for bodies at rest,
the flight nodes for moving ones), unless the text says "leading read".

`data/level/` holds the benchmark with the tower beside each body (`scripts/conv_level.py`, which is
`docs/research/paper-real-read/conv_real.py` with the tower moved: lower launch point level with the
body's centre, 1.5 m and 10 m from it horizontally, so every arm point stays outside the sphere that
holds the body). `data/` also holds draft 3's leading-read benchmark (`<body>.json`, triple-product
numerator; `<body>-edges.json`, h_f·2A_f) and its summaries, which §5 cites for the numerator's effect;
`esa_fixtures.json` (numbers from ESA's GPL-3.0 tool), `crosscheck.json`, `mp_reference.json`.
`generated/` holds the LaTeX tables and `numbers.tex` that the scripts write.

## Naming

The technique is called **cut-cell moments** for now (a boundary cell cut by the surface is a *cut
cell*; it stores the mass, centroid and second moment of its filled part); its scalar is the **fill
fraction**. The name is the macro `\cutcell` / `\Cutcell` at the top of `paper.tex`. The body's tree is
an **octree**.

"Exact" means an analytic shape (a sphere's closed form, a torus by ring integrals, a polyhedron by
the closed form). A triangle mesh standing in for a curved shape is already an approximation, and a
clipped piece of a mesh is exact *to the mesh* only. The paper keeps the **facet error** (mesh against
analytic shape) and the **voxel error** (voxels against mesh) apart throughout.

## Every table and figure: where it comes from now, and where it will come from (TASK-S5-18)

Every measured result is from research scripts and is marked *provisional* in the PDF.
TASK-S5-18 wires each row's Cavendish command, regenerates, and drops the mark; the Python stays as
the independent check (Appendix B). Names, not numbers, because numbers move.

| item | now (`make.py` target → script) | after S5 (Cavendish) — to be wired | scene(s) |
|---|---|---|---|
| Table "what a boundary cell carries" | derived | — | — |
| Fig. "three ways to represent a body", "cut-cell moments", "closed form's ingredients", "one cell, many cuts" | `diagrams` → `scripts/diagrams.py` (explanatory; the 3-D pieces from the benchmark's clipper) | could be redrawn from S5's draw modes; not required | — |
| Fig. "the flight-integral read against the leading read" | `realread` copies `docs/research/paper-flight-phase/flight-phase.pdf`; `realread-rerun` → that note's `fig_paper.py` | `cav run` of the passes (point masses, Spot, the torus) with `scenes/` like the note's `paper-flight-phase/scenes/` | the note's scenes |
| Fig. far field and near edge | `farfield`, `nearedge` → the benchmark's `farfield.py`, `nearedge.py`; drawn by `figures` | the kernel diagnostics `far_field_by_distance`, `near_edge_by_distance` in `cav-core` (S5.md §7.1) | none |
| Table "four codes against 40 digits" | `realread` from `docs/research/paper-benchmarks/accuracy.json` (`bench_accuracy.py`) | `cargo xtask oracle` with `data/esa_fixtures.json` and `data/mp_reference.json` as fixtures | the bodies' triangles are in `esa_fixtures.json` |
| Fig. "against a 40-digit evaluation" | `crosscheck`, `mp` → `scripts/crosscheck.py`, `scripts/mp_reference.py`; drawn by `figures` | as above | as above |
| Table "Cavendish's polyhedral kernel" (P1–P7) | quoted from `docs/slices/S5.md` §7.1 (already Cavendish) | the validation report's rows | — |
| Table "phase error at h = 1/64 m" and Fig. "against pitch" | `level` → `scripts/conv_level.py`; `realread` writes `generated/rr-conv.tex`, `figures/convergence.pdf` | a validation case over S5's `Representation` field on the five bodies, four lattice offsets, tower beside the body, `cav run` read (S5.md §7.6's C2 is this for three bodies) | `scenes/<body>-1.5m.ron`, `-10m.ron` (to be moved to the level geometry) |
| Table "static phase errors behind the budget" | `realread` (`generated/numbers.tex`) from `data/level/conv.json` and `data/mp_reference.json` | the same rows from the validation report | as above |
| Fig. "the shapes, cut away" (torus, bunny) | `shapes` → `scripts/shapes.py` crops the study's renders | S5's draw modes (cut cells, pieces) | the ten shapes |
| Fig. "phase error against cells across" and Table "sweep" | `realread` copies `paper-real-read/fig-sweep.pdf`, parses `tables-sweep.md` (`analyse_real.py sweep`) | validation cases over `Representation` on the ten shapes (meshes imported by `cav`) | the ten shapes, to be written |
| Table "cost" | `realread` from `tables-sweep.md` (a model) | the S5 walk's measured cost per point | as above |
| Table "crossover distances" | `realread` from `tables-regimes.md` (`analyse_real.py regimes`) | a validation case reading each shape at the 18 distances | as above |
| Table "each mesh as one polyhedron" | `realread` from `tables-regimes.md`, `paper-real-read/data/walkpast.json` | `cav run` with the mesh as a `Faceted` body, walking past | the six meshes |
| Table "decimation" | `realread` from the study's `mesh-check.json` and `paper-real-read/data/exact-distance.json` | the same, through `cav` import | the three scans, full and decimated |
| Table "thresholds", Fig. "bias against pitch", Fig. "the bunny's budget" | `realread` from `tables-budget.md`, copies `fig-bias.pdf`, draws `budget.pdf` (`analyse_real.py budget`) | a Gradar-side inference over `cav` passes | the three bodies' passes |
| Table "what is machine-checked" | `proofs/polyhedral-gravity` (`lake build`) | — | — |

Retired with draft 3: the octree figures (`scripts/octree_diagrams.py` still draws them, for the paper
that will carry the world octree), the scene render, and the leading-read shapes figures (pareto,
regimes, motion, each mesh as one polyhedron).

## Notes for the reader

- The leading read k_eff T² Δg_z at the launch points is not the instrument's read within a few metres:
  the arms separate by 4.8 m and span 10 m. Draft 4 quotes it only where it holds or for comparison.
- The shapes study reports, as measured: the facet error dominating for the analytic shapes; the
  dragon and Buddha decimations failing their pre-written 1/256 m criterion; the whole-mesh direct sum
  being competitive for one small body. Draft 3's "sphere open question" was the lattice placement.
- The ESA cross-check failed its pre-written criterion (10⁻¹² to R/s = 100): the 40-digit referee
  shows ESA's and Harmonica's forms losing digits as ε(R/s)³ while ours stays at ε(R/s)², so the
  mismatch is rounding in the other implementations, not an error in any algebra.
