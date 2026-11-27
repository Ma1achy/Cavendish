# Exact gravity of polyhedral boundary cells (draft paper)

Malachy Doherty (affiliation to be filled in). **A draft for comments. Not submitted anywhere;
whether, where and with whom to publish is the author's decision.**

`paper.tex` + `refs.bib` → `paper.pdf` (built with tectonic). The method and the measurements are
from the research notes [`exact-skin-and-partial-fill.md`](../../research/exact-skin-and-partial-fill.md)
and [`s5-world-octree.md`](../../research/s5-world-octree.md); the derivation is machine-checked in
[`proofs/polyhedral-gravity/`](../../../proofs/polyhedral-gravity/).

## Rebuilding

```text
python make.py all                 # tables, far-field, near-edge, cross-check, 40-digit reference,
                                   # scenes, diagrams, figures, PDF (minutes; stored benchmark output)
python make.py bench               # rerun the boundary-cell benchmark (~40 min, one core, below normal)
python make.py esa <venv-python>   # rerun ESA's oracle in its own venv, then crosscheck + mp
python make.py render              # Cavendish draws scenes/cube-1.5m.ron (needs a built `cav`)
python make.py shapes              # the shapes study's figures and tables, from its committed output
python make.py shapes-study        # rerun the shapes study itself (hours; downloads the meshes)
```

`data/` holds the regenerated benchmark output (`<body>.json` with the triple-product numerator, as
the research note's `results/`; `<body>-edges.json` with the h_f·2A_f numerator), the summaries,
`esa_fixtures.json` (numbers from ESA's GPL-3.0 tool; tool and version inside the file),
`crosscheck.json`, `mp_reference.json` and `storage.json`. `generated/` holds the LaTeX tables and
numbers `make.py` writes.

## Naming

The technique is called **cut-cell moments** for now (a boundary cell cut by the surface is a *cut
cell*; it stores the mass, centroid and second moment of its filled part); its scalar is the **fill
fraction**. The name is the macro `\cutcell` / `\Cutcell` at the top of `paper.tex`, so it changes in
one place. The body's tree is an **octree**; 2-D pictures are cross-sections through it.

"Exact" means the analytic shape (a sphere's closed form, a torus by ring integrals). A triangle mesh
is already an approximation, and a clipped piece is exact *to the mesh* only. The paper keeps the
**facet error** (mesh against analytic shape) and the **voxel error** (voxels against mesh) apart
throughout. The shapes section measures both; everything else measured here is voxel error.

## Every table and figure: where it comes from now, and where it will come from (S5)

Until S5 lands, every measured result is from the research scripts and is marked *provisional* in
the PDF. The S5 builder should wire each row's Cavendish command, regenerate, and drop the mark; the
Python stays as the independent check (the paper's Appendix B).

| item (by name; numbers move) | now | after S5 (Cavendish) — to be wired | scene(s) |
|---|---|---|---|
| Table "what a boundary cell carries" (orders) | derived | — | — |
| Table "phase error at h = 1/64 m" (convergence, 1.5 m and 10 m) | `make.py bench`, `make.py tables` (`skin_bench.py run`, `tables.py`) | `cav run scenes/<body>-<D>m.ron --out … --exec "set bodies.0.shape.representation <rep>; set bodies.0.shape.pitch <h>"` per representation (sampled fill, fill fraction, cut-cell moments with and without the second moment, clipped pieces) and pitch 1/8…1/64, four lattice offsets; the error is the gradiometer channel against the clipped-piece representation. Needs S5's representation and lattice-offset fields (names for S5 to fix). Better: a validation case `polyhedral.boundary_convergence` in `cargo xtask validate` that writes the table | `scenes/<body>-1.5m.ron`, `scenes/<body>-10m.ron` |
| Figure "phase error at 1.5 m against pitch" | `make.py figures` from `data/convergence.json` | the validation case emits the data; `make.py figures` keeps drawing it | as above |
| Figure far-field rounding (left) | `farfield.py` | kernel validation case `polyhedral.farfield`: the polyhedral kernel against the cube series at R/s = 10…3×10⁵ | none |
| Figure near-edge (right) | `nearedge.py` | kernel validation case `polyhedral.near_edge` | none |
| Figure "against a 40-digit evaluation" | `scripts/esa_fixtures.py` (ESA tool), `scripts/crosscheck.py`, `scripts/mp_reference.py` | `cargo xtask oracle` with `data/esa_fixtures.json` and `data/mp_reference.json` as fixtures against the Rust kernel | the six bodies' triangles are in `esa_fixtures.json` |
| Table "error budget" | `make.py figures` (`budget()`) | the same rows from the validation report (`docs/validation/`) and the S5 walk's ledger | `scenes/<body>-1.5m.ron` |
| Octree section's measured numbers (ring widths, interaction counts, bound looseness, baseline help) | quoted from `docs/research/s5-world-octree.md` (its benchmarks) | S5's walk benchmark and ledger on the town scene | the town |
| Figure "cells stored" | `scripts/octree_diagrams.py` (counts a sphere's octree) | `cav` stats of a voxelised sphere at each pitch (leaf and cut-cell counts) | a sphere scene at N = 8…256 |
| Figure "the benchmark's turned cube as a Cavendish scene" | `cav render scenes/cube-1.5m.ron --out figures/scene-cube-1.5m.png --view iso --style paper --size 1200x900` (**already Cavendish**) | add S5's draw modes (fill fraction, mesh, clipped pieces) | `scenes/cube-1.5m.ron` |
| Section "Curved, scanned and CAD shapes": figures (representations of the torus and bunny, phase error against pitch, against modelled cost, against distance, the moving torus's error spectrum, each mesh as one polyhedron) and tables (sweep, cost, crossover distances, decimation, mesh as one polyhedron) | `make.py shapes` (`scripts/shapes.py`) draws them from the shapes study's committed output, read in place from `docs/research/exact-skin-and-partial-fill/shapes/` (`sweep-*.json`, `regimes-distance.json`, `translation.json`, `exact-*.json`, `mesh-check.json`); the two renders are the study's `<shape>-cutaway.png`, cropped to the panels into `figures/shapes-<shape>.png`. `make.py shapes-study` reruns the study's own scripts first (hours; they download the meshes, pinned by SHA-256) | the sweeps and crossovers: validation cases over S5's representation field on the ten shapes (meshes imported by `cav`, which today refuses the full dragon and Buddha at its caps); the moving-body spectrum: `cav run` of the torus passing a tower, body-frame and world-fixed voxels; each mesh as one polyhedron: the polyhedral kernel on the whole mesh | the ten shapes, to be written by S5 (the meshes are downloaded, never committed) |
| Diagrams: representations, cut-cell moments, one cell many cuts, closed-form ingredients, octree, layered read | `scripts/diagrams.py`, `scripts/octree_diagrams.py` (explanatory; the 3-D pieces come from the benchmark's own clipper) | could be redrawn from S5's draw modes; not required | — |

Scenes: `scripts/make_scenes.py` writes `scenes/<body>-<D>m.ron` for the benchmark's five bodies at
1.5 m and 10 m. Each holds the body's triangles (`Triangles`, the same as `skin_bench.body(name)`),
ρ = 2700 kg/m³, pitch 1/64 m, supersample 4, and three towers whose bases sit at D along the
benchmark's three directions. All ten load with today's `cav get`.

## Notes for the reader

- The read in the benchmark is the leading-order k_eff T² Δg_z, not the full flight integral; once
  Cavendish computes the table, the flight integral replaces it.
- The shapes study reports, as measured: the facet error dominating for the analytic shapes; second
  moments making the sphere (only) worse than without (open); the dragon and Buddha decimations failing
  their pre-written 1/256 m criterion; the whole-mesh direct sum being competitive for one small body;
  `cav`'s import caps refusing the full dragon and Buddha.
- The ESA cross-check failed its pre-written criterion (10⁻¹² to R/s = 100): the 40-digit referee
  shows ESA's line-integral form losing digits as ε(R/s)³–⁴ while ours stays at ε(R/s)², so the
  mismatch is rounding in the other implementation, not an error in either algebra.
