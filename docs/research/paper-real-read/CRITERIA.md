# Pass criteria, written before measuring (28 Nov 2026)

Committed before any number of this study was computed. The note (`../paper-real-read.md`) reports
each against these and says so where one fails; none is moved after the fact.

Two reads of one shot are compared throughout:
- **real**: Cavendish's flight integral, (m_A/ħ)∫₀^{2T}[V(upper arm) − V(lower arm)]dt per
  interferometer, differenced, with the body moving during the shot where it moves
  (`../paper-flight-phase/flight.py`, which is `cav run` to 1.8×10⁻¹⁵ rad);
- **leading**: the paper's k_eff T² Δg_z at the two launch points, one instant.

For a static body the real read is computed as a weighted sum of g_z at fixed heights on the two
launch axes: g_z interpolated in height (Legendre nodes on the arms' range) and integrated exactly
between the arms at every flight node. For a moving body it is V summed at the flight nodes
directly. Both reuse the shapes study's voxeliser, pieces and moment kernels unchanged; only the
points and the weights change.

## Checks of the machinery (must pass, or the numbers are not reported)

- **R1, the static read is the flight integral.** The height-weighted g_z read equals
  `flight.frozen_phase` (V at the arms' nodes) to ≤ 1e-10 relative for a point mass 1.2–1.5 m from
  the axis at five heights from −1 to 11 m, and for the bunny mesh 1.5 m from the lower launch.
  Control arm: the leading read misses the same numbers by more than 10 %.
- **R2, the compiled kernel is the Python kernel.** The Rust triangle-soup kernel (U and g) equals
  `skin_bench.polyhedron_field(stable=True)` to ≤ 1e-12 of the largest |U|, |g| on a random soup of
  clipped pieces; the point-mass, quadrupole and cube-series kernels equal their numpy forms to
  ≤ 1e-12. Control arm: moving one vertex by 1e-6 m changes the result by more than 1e-10 relative.
- **R3, only the read changed.** With the leading read and the paper's lattice placement (GENERIC),
  the new sweep runner reproduces the committed `sweep-<shape>.json` errors at 1.5 m and 5 m to
  1e-6 relative or 1e-13 rad, whichever is larger, at every pitch; the new budget runner's leading
  signal reproduces `truth-<body>.json` to 1e-9 relative.
- **R4, the moving read is converged.** For the passes, the chosen flight-node rule changes the
  signal by ≤ 1e-6 of its peak and every residual by ≤ 1e-3 of its own maximum against a rule of
  twice the order and half the panel.
- **R5, the reference is good enough.** Interior cubes plus clipped pieces reproduce the mesh in the
  real read to ≤ 1e-10 rad at 1.5 m, at every pitch, shape and placement (as the leading read's (e)
  did). Where a technique's error falls below the reference's, it is reported as "at the floor".
- **R6, the Jacobian.** The analytic flight-read Jacobian (from g at the nodes) equals 1 mm central
  differences of the exact mesh to 1e-5 relative, for every parameter. Control: a 30 cm step
  disagrees by more than 1e-3 somewhere.

## Questions with a stated test (the answer is the finding; reported either way)

- **Q1, ranking.** At 128 cells across and 1.5 m, (d) < (c) < (b) < (a) in the real read on the nine
  shapes other than the sphere, for every placement. If it fails on any, say where.
- **Q2, orders.** The real read's fitted orders (size/h ≥ 8, as the paper) of (c) and (d) are within
  ±0.5 of the leading read's on each shape. Spread over placements reported as the orders' error.
- **Q3, the sphere anomaly (referee S1).** If, over the random placements, (d) beats (c) in RMS at
  64 and 128 cells across (and (d★) beats (c★)), the paper's anomaly was the near-symmetric
  placement. If (d) loses on the random placements too, it is real and stays open.
- **Q4, the error budget.** The pitch at which max |bias|/σ_CRB < 0.1, worst of the same three
  placements as the earlier study, interpolated log–log, with the real read throughout (signal,
  Jacobian, residuals). Reported as the ratio to the leading-read pitch.
- **Q5, where the leading read holds.** The distance beyond which real and leading signals agree to
  1 % and 10 % on the shapes (static, along the paper's tower ray), against the flight-phase study's
  94 m / 29 m for a point source level with the lower launch.
- **Q6, does any conclusion change.** A conclusion of the paper is taken as changed if a ranking
  flips, an order moves by more than 0.5, or a threshold or crossover moves by more than a factor 2.
