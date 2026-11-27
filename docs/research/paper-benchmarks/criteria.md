# Pass criteria, written before any measurement (27 Nov 2026)

Reference: the Werner & Scheeres face form evaluated at 40 significant digits with mpmath on the same
f64 vertices (`docs/papers/polyhedral-gravity/scripts/mp_reference.py`'s `field`), and for boxes and
voxel sums Nagy's prism formula at 40 digits. Error is relative: `|g − g_ref| / |g_ref|` (vector
norm) and `|U/U_ref − 1|`, worst of three directions. "Ours" is the paper's form, in two
implementations: the study's numpy (`skin_bench.polyhedron_field(stable=True)`) and Cavendish's Rust
(`cav_core::polyhedral::Faceted`).

**A1, near.** At R ≤ 3 m from the centre of a ~1 m body, every code (ours ×2, ESA's
polyhedral-gravity, Harmonica's prism on the axis-aligned boxes) is within 1e-13 of the reference in
U and g.

**A2, far.** At R/s = 100, ours (both) ≤ 1e-10 in g; at R/s = 1000, ≤ 1e-8. For ESA and Harmonica no
pass level is set: we record the error and its growth per decade of R (ε(R/s)² class: ×100 per
decade; ε(R/s)³ class: ×1000).

**A3, ours against ours.** Rust and numpy agree to 1e-13 at R ≤ 3 m.

**B1, voxel sums.** A body voxelised to whole cubes (centre inside) at h = 1/16 m: the gradiometer phase
`k_eff T² Δg_z` (lower point 1.5 m or 10 m from the centre, upper 5 m above) from Harmonica's prism
sum, the study's cube voxel sum (closed form over each cube's 12 triangles) and Cavendish's cube
kernel (`cube_potential`, potential only: compared as U) each agree with the 40-digit Nagy sum to
1e-9 rad (phase) and 1e-12 relative (U).

**B2, singular points.** On a face, on an edge, at a vertex of the aligned cube, and on a face's
plane or an edge's line outside the body: every code returns a finite U and g (or documents NaN),
and where finite it matches the reference (the continuous limit) to 1e-12.

**T, cost.** Recorded, no pass level, except a sanity check: Cavendish's `Faceted::field` is within
a factor 2 of the 55 ns per triangle per point the paper quotes.
