# Exact gravity of polyhedral boundary cells (draft paper)

**A draft for George's comments. Not submitted anywhere; publishing is the owner's decision.**

`paper.tex` + `refs.bib` → `paper.pdf` (built with tectonic). The method and the measurements are
from the research note [`docs/research/exact-skin-and-partial-fill.md`](../../research/exact-skin-and-partial-fill.md);
the derivation is machine-checked in [`proofs/polyhedral-gravity/`](../../../proofs/polyhedral-gravity/).

## Rebuilding

```text
python make.py all                 # tables, far-field, near-edge, cross-check, 40-digit reference,
                                   # scenes, figures, PDF (minutes; uses the stored benchmark output)
python make.py bench               # rerun the boundary-cell benchmark (~40 min, one core, below normal)
python make.py esa <venv-python>   # rerun ESA's oracle in its own venv, then crosscheck + mp
python make.py render              # Cavendish draws scenes/cube-1.5m.ron (needs a built `cav`)
```

`data/` holds the regenerated benchmark output (`<body>.json` with the triple-product numerator, as
the research note's `results/`; `<body>-edges.json` with the h_f·2A_f numerator), the summaries,
`esa_fixtures.json` (numbers from ESA's GPL-3.0 tool; tool and version inside the file),
`crosscheck.json` and `mp_reference.json`. `generated/` holds the LaTeX tables `make.py` writes.

## Every table and figure: where it comes from now, and where it will come from (S5)

Until S5 lands, every result is from the research scripts and is marked *provisional* in the PDF.
The S5 builder should wire each row's Cavendish command, regenerate, and drop the mark; the Python
stays as the independent check (the paper's Appendix B).

| item | now (Python) | after S5 (Cavendish) — to be wired | scene(s) |
|---|---|---|---|
| Table 2 (orders), derived | — | — | — |
| Table 3, convergence at 1.5 m and 10 m | `make.py bench`, `make.py tables` (`skin_bench.py run`, `tables.py`) | `cav run scenes/<body>-<D>m.ron --out … --exec "set bodies.0.shape.representation <rep>; set bodies.0.shape.pitch <h>"` for each representation (sampled, fill, fill+centroid, +second moment, exact pieces) and pitch 1/8…1/64, four lattice offsets; the error is the gradiometer channel against the `exact` representation. Needs S5's representation field and a lattice-offset field (names to be fixed by S5). Better still, a validation case `polyhedral.boundary_convergence` in `cargo xtask validate` that writes the table | `scenes/<body>-1.5m.ron`, `scenes/<body>-10m.ron` |
| Figure 3, convergence curves | `make.py figures` from `data/convergence.json` | `cav figure` cannot draw this today; S5 can emit the convergence data from the validation case and `make.py figures` keeps drawing it | as above |
| Figure 1 left, far-field rounding | `farfield.py` | a `cav-core` test/bench of the polyhedral kernel against the cube series at R/s = 10…3×10⁵ (`cargo xtask validate`, case `polyhedral.farfield`) | none (kernel-level) |
| Figure 1 right, near-edge | `nearedge.py` | kernel-level validation case `polyhedral.near_edge` | none |
| Figure 2, cross-check | `scripts/esa_fixtures.py` (ESA tool), `scripts/crosscheck.py`, `scripts/mp_reference.py` | `cargo xtask oracle` reading `data/esa_fixtures.json` and `data/mp_reference.json` as fixtures against the Rust kernel | the six bodies' triangles are in `esa_fixtures.json` |
| Table 4, error budget | `make.py figures` (`budget()`), from the above | the same rows from the validation report (`docs/validation/`) and the S5 walk's ledger | `scenes/<body>-1.5m.ron` |
| Figure 4, the scene | `cav render scenes/cube-1.5m.ron --out figures/scene-cube-1.5m.png --view iso --style paper --size 1200x900` (**already Cavendish**) | add the S5 draw modes: `--exec "view fill"`, mesh, clipped pieces | `scenes/cube-1.5m.ron` |

Scenes: `scripts/make_scenes.py` writes `scenes/<body>-<D>m.ron` for the benchmark's five bodies at
1.5 m and 10 m. Each holds the body's exact triangles (`Triangles`, the same as
`skin_bench.body(name)`), ρ = 2700 kg/m³, pitch 1/64 m, supersample 4, and three towers whose bases
sit at D along the benchmark's three directions. All ten load with today's `cav get`.

| Figure 4, the shapes study | **placeholder** (the study is being redone: repaired teapot and Suzanne meshes, analytic sphere and torus references, resolution sweeps, error against cost; the earlier teapot and Suzanne pictures are withdrawn) | the shapes study's commands, once it lands | — |

## Wording

"Exact" means the analytic shape (a sphere's closed form, a torus by ring integrals). A triangle mesh
is already an approximation, and a clipped piece is exact *to the mesh* only. The paper keeps the
**facet error** (mesh against analytic shape) and the **voxel error** (voxels against mesh) apart
throughout; everything measured here is voxel error.

## Notes for George

- The read in the benchmark is the leading-order kₑff T² Δg_z, not the full flight integral; once
  Cavendish computes the table, the flight integral replaces it.
- The ESA cross-check failed its pre-written criterion (10⁻¹² to R/s = 100): the 40-digit referee
  shows ESA's line-integral form losing digits as ε(R/s)³–⁴ while ours stays at ε(R/s)², so the
  mismatch is rounding in the other implementation, not an error in either algebra.
