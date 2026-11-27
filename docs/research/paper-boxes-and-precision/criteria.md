# Pass criteria, written before measuring (27 Nov 2026)

The tolerance is Cavendish's `PHASE_TOLERANCE` = 1e-10 rad (with `PHASE_TOLERANCE_RELATIVE` = 1e-12 of
the phase as a floor), `crates/cav-core/src/forward.rs`; the paper's representation budget is "a few
10⁻⁷ rad" at 1.5 m; the shot-noise floors are 1e-4 and 1e-5 rad Hz^-1/2.

## A. Interior boxes

- **A1 partition.** Every decomposition covers each interior cell exactly once and nothing else
  (checked by rasterising the boxes back onto the lattice). Control: the same check with the largest
  box removed must fail.
- **A2 rounding.** At every pitch and every tower distance (0.1 m to 1 km from the surface), the
  phase of the interior as boxes (each box's closed form) differs from the interior as single cubes
  (each cube exact, or its l ≤ 4 series where (h/2)/d < 0.01, as the shapes study) by at most
  1e-11 rad or 1e-14 of the interior's own phase, whichever is larger. Control: with the largest box
  dropped the difference must exceed 1e-6 rad at 1.5 m.
- **A3 reproduction.** With boxes for the interior and cut-cell moments (d) for the boundary, the
  1.5 m error at 128 cells across reproduces the shapes study's (d) (`sweep-<shape>.json`) to
  1e-12 rad: coalescing must not change the error.
- **A4 box kernel.** The box closed form (12 triangles, h·2A numerator, log1p edges) agrees with a
  40-digit evaluation to 1e-13 relative at R/s ≤ 10 and to 30 ε(R/s)² beyond, for aspect ratios up
  to 1:256.
- What would change the paper's ranking: if, with box interiors, (d) became cheaper than the
  whole-mesh direct sum at 1e-8 rad on a shape where it is not today (teapot and CAD part already
  are; every other mesh then the question is the size of the gap), or the order (d) < (c) ≪ (b) < (a)
  changed. Reported either way.

- **A5 the octree body (added at the owner's request, before measuring).** Interior merged into the
  largest aligned 2^k blocks wholly inside; each block read by its closed form near and its l ≤ 4
  series far. (i) Interior blocks grow as N² (fitted exponent of block count against cells across
  between 2.0 and 2.4 over 16–128), against N³ for the cells. (ii) With the strict switch ((s/2)/d <
  0.01) the octree interior meets A2. (iii) With S3's switch (closed form within 3 sides) the extra
  error is reported against 1e-10 rad; if it exceeds it at 1.5 m the rule is not good enough for the
  oracle and the note says so.

## B. Single precision

Reference: the same expression in f64 (f64's own error is ≥ 10⁸ times smaller than f32's for the
same expression; checked against 40 digits at a few points).

- **B1 kernel scaling.** Per triangle and per box, the f32 relative error of g with the paper's rules
  (local offsets formed in f64, h·2A numerator, log1p edge) grows no faster than 30 ε₃₂ (R/s)², with
  ε₃₂ = 6e-8; with the triple product it grows as (R/s)³. If the h·2A form exceeds 30 ε₃₂(R/s)² the
  paper's scaling argument does not carry to f32 and must be stated.
- **B2 safe in f32** means: the phase error of the f32 evaluation against f64 is ≤ 1e-10 rad
  (`PHASE_TOLERANCE`). **Safe against noise** means ≤ 1e-6 rad (a hundredth of the 1e-4 rad Hz^-1/2
  floor for one second). Each (representation, distance, pitch, summation) cell of the table is
  marked by which it meets.
- **B3 fixes.** A fix counts only if it brings a failing cell under 1e-10 rad; its cost is stated in
  f32-operation equivalents (float-float add ≈ 20 flops, mul ≈ 10 with FMA, 17 without, Joldes et
  al. 2017 counts) or measured.
- **B4 WGSL.** The same body sum with WGSL's permitted worst-case function errors (atan2 4096 ULP;
  log absolute 2^-21 on [0.5, 2]; WGSL §15.7.4) injected as random perturbations: reported, with
  the threshold distance where it fails B2.
