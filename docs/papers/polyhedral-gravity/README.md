# Exact gravity of polyhedral boundary cells (draft paper)

Malachy Doherty (affiliation to be filled in). **Draft for comments, not submitted. Whether, where
and with whom to publish is the author's decision.**

`paper.tex` + `refs.bib` → `paper.pdf` (built with tectonic). Draft 5 (28 Nov 2026) answers the
referee's report on draft 4 ([`REVIEW-draft-4.md`](REVIEW-draft-4.md), with the response at its end). It covers the
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
reports are [`REVIEW-draft-3.md`](REVIEW-draft-3.md) and [`REVIEW-draft-4.md`](REVIEW-draft-4.md).

## Rebuilding

```text
python make.py all                 # summaries, far-field and near-edge (Cavendish), Cavendish against
                                   # 40 digits, cross-check, scenes, diagrams, the engine's table,
                                   # real-read tables and figures, PDF
python make.py conv                # Cavendish reads the benchmark bodies (oracle/tests/benchmark.rs;
                                   # minutes) -> data/cavendish/conv.json; then `realread pdf`
python scripts/oracle_fixtures.py  # the oracle's 40-digit fixture from esa_fixtures.json (a minute;
                                   # only if that file changes, never to pass a test)
python make.py omitted             # sizes of the effects the first-order model leaves out
                                   # (scripts/omitted.py -> data/omitted.txt; seconds)
python make.py level               # rerun the benchmark with the tower beside each body (~10 min;
                                   # needs `cargo build --release` in docs/research/paper-real-read/kern)
python make.py realread-rerun      # rerun the real-read and flight-phase notes' analysis and figures
python make.py bench               # rerun draft 3's leading-read benchmark (~40 min)
python make.py esa <venv-python>   # rerun ESA's oracle in its own venv, then crosscheck + mp
python make.py shapes-study        # rerun the shapes study itself (hours; downloads the meshes)
```

Every phase in drafts 4 and 5 is the flight-integral read (the static axis-weight form for bodies at rest,
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

## Every table and figure: where it comes from (TASK-S5-18)

TASK-S5-18 wired the rows marked **Cavendish** to Cavendish's own tests, which write
`data/cavendish/`; the paper's figures are drawn from that by the same Python and in the same style
as before, and their *provisional* mark is gone. The rows still marked *provisional* come from the
research scripts, with one line on why. The Python stays as the independent check (Appendix B).
Names, not numbers, because numbers move; `docs/slices/S5.md` §7.15 lists every number that
changed.

| item | source (`make.py` target → what runs) | state | scene(s) |
|---|---|---|---|
| Table "what a boundary cell carries" | derived | — | — |
| Fig. "three ways to represent a body", "cut-cell moments", "closed form's ingredients", "one cell, many cuts" | `diagrams` → `scripts/diagrams.py` (explanatory; the 3-D pieces from the benchmark's clipper) | explanatory, not measured; could be redrawn from S5's draw modes | — |
| Fig. "the flight-integral read against the leading read" | `realread` copies `docs/research/paper-flight-phase/flight-phase.pdf`; `realread-rerun` → that note's `fig_paper.py` | *provisional*: needs `cav run` sweeps of moving point masses (panel a, hundreds of passes), Spot (a downloaded mesh, panel b) and a moving voxel torus (c, d); not wired in S5-18 (TASK-S5-29) | the note's scenes |
| Table "effects the model leaves out" | `omitted` → `scripts/omitted.py` (point-mass stand-ins, derived; `data/omitted.txt`) | *provisional*: Earth's rotation and finite pulses are not in the engine's arms | — |
| Fig. far field and near edge | `farfield`, `nearedge` → `cargo test -p cav-core --lib far_field_by_distance` / `near_edge_by_distance -- --ignored` (`src/polyhedral/tests.rs`) → `data/cavendish/farfield.txt`, `nearedge.txt`; drawn by `figures` | **Cavendish** | none |
| Table "four codes against 40 digits" | `forty` → the oracle crate's `forty_digits_for_the_paper` (`oracle/tests/polyhedral.rs`) → `data/cavendish/forty-digits.json`; `realread` | **Cavendish** column from it (Spot's cells excepted: its mesh is downloaded, so they stay the benchmark harness's run of the same kernel); Python, ESA and Harmonica from `docs/research/paper-benchmarks/accuracy.json` (other codes) | the fixture `oracle/fixtures/paper/polyhedral/` (`scripts/oracle_fixtures.py` from `esa_fixtures.json`, `mp_reference.json`) |
| Fig. "against a 40-digit evaluation" | `forty`, then `figures` | **Cavendish** ("this form"); ESA from `mp_reference.json` | as above |
| P8 (≤ 1e-12 to 100 sizes) and the rounding class | `cargo xtask oracle`: `validation_polyhedral_forty_digit_class` (and P8 as written, TASK-S5-28) | **Cavendish**; P8 as written fails (S5.md §7.15) | as above |
| Table "Cavendish's polyhedral kernel" (P1–P7) | `engine` → `scripts/engine_table.py` from `docs/validation/report.json` → `generated/rr-engine.tex` | **Cavendish** (the report's `polyhedral.*` cases) | — |
| Table "phase error at h = 1/64 m" and Fig. "against pitch" | `conv` → the oracle crate's `the_benchmark_for_the_paper` (`oracle/tests/benchmark.rs`, case `cut.benchmark_bodies`) → `data/cavendish/conv.json`; `realread` | **Cavendish** (the flight read); the dotted leading read and the arms' clearance from `data/level/conv.json` (`level` → `scripts/conv_level.py`), as Cavendish has no leading read | the bodies from the fixture; `scenes/` holds the same bodies for `cav` |
| Table "static phase errors behind the budget" | `realread` (`generated/numbers.tex`) | **Cavendish** (`data/cavendish/conv.json`, `forty-digits.json`) | as above |
| Fig. "the shapes, cut away" (torus, bunny) | `shapes` → `scripts/shapes.py` crops the study's renders | *provisional*: S5's piece view exists, but the ten shapes' meshes are downloaded by the study and not in the repository (TASK-S5-29) | the ten shapes |
| Fig. "phase error against cells across" and Table "sweep" | `realread` copies `paper-real-read/fig-sweep.pdf`, parses `tables-sweep.md` (`analyse_real.py sweep`) | *provisional*: as above, the ten shapes' scenes are not written (TASK-S5-29) | the ten shapes, to be written |
| Table "cost" | `realread` from `tables-sweep.md` (a model) | *provisional*: needs the ten shapes and the walk's cost per point | as above |
| Table "crossover distances" | `realread` from `tables-regimes.md` (`analyse_real.py regimes`) | *provisional*: as above | as above |
| Table "each mesh as one polyhedron" | `realread` from `tables-regimes.md`, `paper-real-read/data/walkpast.json` | *provisional*: as above (the meshes) | the six meshes |
| Table "decimation" | `realread` from the study's `mesh-check.json` and `paper-real-read/data/exact-distance.json` | *provisional*: as above (the scans) | the three scans, full and decimated |
| Table "thresholds", Fig. "bias against pitch", Fig. "the bunny's budget" | `realread` from `tables-budget.md`, copies `fig-bias.pdf`, draws `budget.pdf` (`analyse_real.py budget`) | *provisional*: a Gradar-side inference over `cav` passes, not Cavendish's | the three bodies' passes |
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
