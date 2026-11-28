# Pass criteria, written before measuring (28 Nov 2026)

For `docs/research/surface-representation.md`. Every number in the note comes from a script in this
folder and its JSON. Nothing below is moved after a run; a failure is reported as a finding.

The read is the AION-10 flight integral with the source static (`paper-flight-phase/flight.py`,
`frozen_phase`'s nodes, Gauss order 6, panels ≤ 0.1 s: the engine's rule), the tower's base at the
stated distance from the body's surface along (1, 0.2, −0.3) from its bounding-box centre, as the
shapes study. ρ = 2700 kg/m³. Errors are of a boundary representation against the exact clipped
pieces, summed over boundary cells (the interior is common to all and cancels).

| id | property | pass | control arm (must fail) |
|---|---|---|---|
| C1 | the bench's triangle-soup potential kernel is Cavendish's `Faceted::potential` | relative difference ≤ 1e-13 on the icosphere and a cube at 1.5–20 m | one vertex moved by 1e-6 m: difference > 1e-9 |
| C2 | the exact pieces partition the body | Σ piece volumes + interior = mesh volume to 1e-12; Σ piece phase + interior phase = whole-mesh phase to 1e-10 rad at 1.5 m (bracket, h = 1/32 m) | one piece dropped: > 1e-6 rad |
| C3 | the PLIC piece is built right | its volume equals the fill to 1e-12 relative; on cells cut only by one flat mesh face the PLIC piece's phase equals the exact piece's to 1e-12 rad per cell | normal turned by 1e-3 rad: > 1e-12 rad on those cells |
| C4 | the moment ladder behaves as §2.1 of the skin note says | at 20 m the error falls at every rung (fill > +centroid > +second > +third) on all five shapes | none (a monotonicity check) |
| C5 | the CPU timing is consistent with the earlier studies | the soup triangle kernel within 2× of 75–107 ns per triangle per point (`paper-benchmarks.md` Table D) | — |
| C6 | the GPU kernels compute the same thing | float-float element phase within 1e-9 relative of the CPU f64 kernel (polyhedra and PLIC pieces at 1.5 m); f32 moments within 1e-6 relative | f32 polyhedron kernel (expected ≥ 1e-5 relative) |
| C7 | adaptive refinement meets its budget B | measured \|error\| ≤ B at the tower it was built for | the same tree read from a nearer tower exceeds B |
| C8 | the error-bounded walk meets its budget B | measured \|error\| ≤ B at every tower, and every node's measured error ≤ its bound | the bound with one order too many ((b/d)^(p+2)) exceeds B or a node's bound somewhere |
| C9 | the free-box series is right | box series to l = 2, 4, 6 against the box's closed form: fitted slope of the error in s/R within ±0.3 of 4, 6, 8 | — |
| C10 | the BVH walk meets the same budget | as C8, with the BVH's nodes | as C8 |

GPU numbers are indicative only (the GTX 1660 SUPER is correctness-only by the owner's ruling); no
GPU speed is a pass criterion.
