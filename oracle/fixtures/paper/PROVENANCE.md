# The polyhedral-gravity paper's fixtures

`polyhedral/` is written by `docs/papers/polyhedral-gravity/scripts/oracle_fixtures.py` (TASK-S5-18)
from two committed files of the paper:

- `data/esa_fixtures.json`: six bodies' triangles (f64), 18 points (three directions at 1, 1.5, 3,
  10, 30 and 100 m) and the values of ESA's polyhedral-gravity-model 3.4.1 (GPL-3.0, run as a tool
  in its own environment, never linked) at them;
- `data/mp_reference.json`: each code's relative error against a 40-digit evaluation of the
  Werner & Scheeres face form (`scripts/mp_reference.py`, mpmath at 40 digits) on the same f64
  vertices.

The script evaluates the 40-digit values with `mp_reference.field`, unchanged, at those points and
at 300 m and 1000 m, for the six bodies and the research benchmark's axis-aligned plate
(`docs/research/paper-benchmarks/common.py`'s `box_tris`), and stores each as a double-double
(`ref_*_hi` + `ref_*_lo`). It writes nothing unless ESA's 108 errors against these values reproduce
the errors `mp_reference.json` records (measured: to 9.3e-17 of each), and
`tests/polyhedral.rs`'s `the_fixture_is_the_recorded_reference` checks the same. Every array is a
flat little-endian float64 `.npy`; the script's docstring gives the shapes. Units: `U = ∫dV/r` (m²),
`g = ∇U` (m), density 1, no G.

Regenerate only if `esa_fixtures.json` changes, never to make a test pass.
