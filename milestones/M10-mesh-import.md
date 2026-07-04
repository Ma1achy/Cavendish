# M10 — Mesh import (implementation brief)

> Arbitrary geometry into the body dictionary — imported meshes indistinguishable downstream from
> primitives, with dirty meshes handled robustly and *loudly*. Off the critical path; schedulable
> any time after M2 (the voxeliser it plugs into). Read with `design/shape.md` §5.
>
> **Prereq:** M2. **Delivers to:** the body dictionary (richer shapes for rotation/quadrupole
> studies and Gradar realism). **Crates touched:** `shape` (the mesh path; parsers feature-gated).

---

## 1. Requirements

| ID | Requirement |
|---|---|
| M10-R1 | Parsers for STL, OBJ, glTF/GLB → one indexed triangle soup; **explicit `scale` mandatory** (`ScaleMissing` otherwise — no unit guessing). |
| M10-R2 | Watertightness classification on load: edge-manifold check (every edge shared by exactly two consistently-oriented triangles); counts in `MeshReport`. |
| M10-R3 | Watertight fast path: surface rasterisation → exterior flood fill → boundary sub-sampling by BVH parity rays. |
| M10-R4 | Robust path (open/non-manifold): **generalised winding numbers**, hierarchically accelerated; ambiguity diagnostic; `AmbiguousInterior` on genuinely dubious meshes — never silent garbage. |
| M10-R5 | The divergence-theorem volume cross-check for watertight meshes. |
| M10-R6 | A mesh voxelises through the *same* M2 pipeline (renormalise, recentre, canonical order, cache) — mesh ≡ primitive downstream. |
| M10-R7 | Principal-frame boundary: a mesh with off-diagonal inertia must be re-expressed in its principal frame before `Orient::FreeRotation`, which assumes principal-frame authoring (M4); the loader records the principal-axis rotation. (Axis-aligned primitives are already principal, so this is a mesh-only concern.) |

---

**Realisation (M10).** Choices taken during implementation, verified against the code:
- **`scale` is `Option<f64>`**, checked before any file read (`ScaleMissing` on `None`) — the sketch's
  non-optional `f64` cannot express "mandatory but absent" at runtime.
- **Only the parsers are feature-gated** (`stl`/`obj`/`gltf`); the geometry core (winding number,
  watertightness, both classifiers, volume, `MeshSolid`) is always compiled, so its integrity tests run
  in the blocking CI gate. A scoped `cargo test -p shape --all-features` step runs the gated parser
  round-trips (keeping the workspace test libpython-free). Most tests build meshes programmatically
  (an in-crate icosphere/cube), so no binary fixtures are committed.
- **The winding-number acceleration is error-bounded**, not a fixed radius ratio: the BVH multipole
  (dipole + first/second moments) is used for a node only when a bound on its truncation error is below
  tolerance. Two tolerances share one traversal — tight for the accurate winding (`fast_wn_matches_brute`
  ≤1e-6) and loose for inside/outside classification (occupancy needs only the sign of `w − ½`).
- **FreeRotation precondition — path (a):** `principal_frame(&Cloud)` diagonalises the inertia and
  authors the mesh body in its principal frame, **returning the recorded rotation `R`**. A mesh with
  off-diagonal inertia is rotated (explicitly, recorded), never tumbled with the M4 assumption violated.
- **`ShapeError` drops `Copy`/`Eq`** to carry payloads (`AmbiguousInterior(f64)`, `UnreadableMesh(String)`).

## 2. Equations

### 2.1 Generalised winding number (per query point `p`; triangles `t` with vertices `a,b,c` relative to `p`)

```
w(p) = (1/4π) Σ_t Ω_t(p)          w ≈ 1 inside, ≈ 0 outside; fractional near holes
solid angle (van Oosterom–Strackee):
   tan(Ω/2) = a·(b×c) / ( |a||b||c| + (a·b)|c| + (b·c)|a| + (c·a)|b| )
classify: inside ⇔ w > ½
ambiguity diagnostic:  A = mean over samples of min(|w|, |w−1|);  A > A_max ⇒ AmbiguousInterior
```

Brute force is `O(F)` per query — unusable at `10⁶` lattice queries × `10⁵` triangles — so the
robust path evaluates hierarchically: exact solid angles for near BVH nodes, a far-field
dipole approximation per distant cluster (fast winding numbers, Barill et al. 2018).

### 2.2 Divergence-theorem volume (watertight, outward-oriented)

```
V_mesh = (1/6) Σ_t  p₀ · (p₁ × p₂)          compare: |V_voxel − V_mesh| / V_mesh ≤ tol
```

One number that catches scale errors, inverted orientation, and gross classification bugs at once.

---

## 3. Design

```
file ──parse──► TriSoup ──edge-manifold check──►┬ watertight ──► rasterise surface → flood-fill
     (scale                 (MeshReport:        │                exterior → boundary cells:
      applied)               F, open edges,     │                parity rays vs BVH
                             flipped, V_mesh)   │
                                                └ open/non-manifold ──► fast winding numbers
                                                       (BVH + far dipole), diagnostic A,
                                                       A > A_max ⇒ Err(AmbiguousInterior)
                    both ⇒ MeshSolid: occupancy(p) ∈ [0,1]
                                   │
                                   ▼
                    M2 voxeliser (unchanged) ──► renormalise ─► recentre ─► Cloud ─► cache
```

Body frame = the authored mesh frame, scaled, translated to the CoM (axes never silently rotated).
The classification grid is built **once** per (mesh, h) and memoised with the cloud.

---

## 4. Pseudocode

```
fn classify_watertight(soup, lattice) -> OccGrid:
    surf = conservative_rasterise(soup.triangles, lattice)     # cells touching any triangle
    outside = flood_fill(from=lattice.margin, blocked=surf)
    inside  = !outside & !surf
    for cell in surf:                                          # boundary only — the cheap trick
        occ[cell] = mean( parity_ray(p, bvh) for p in sublattice(cell, k) )
    occ[inside] = 1

fn winding(p, bvh_node) -> f64:
    if far(p, node): return dipole_approx(node, p)             # precomputed area-weighted normals
    if leaf:        return Σ solid_angle(tri, p) / 4π
    return Σ winding(p, child)
```

---

## 5. Tests

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `solid_angle_closed_cube` | `w` on a 12-triangle cube: 1 inside, 0 outside, ≈½ at face centres-on-surface | ≤1e-10 / ≤1e-3 |
| unit | `fast_wn_matches_brute` | hierarchical vs brute-force `w` on 10³ random points, 10⁴-triangle mesh | ≤1e-6 |
| unit | `watertight_check` | a punctured cube flagged (open edge count = hole boundary); a sound one passes | exact |
| unit | `scale_mandatory` | STL without `scale` ⇒ `ScaleMissing` | structural |
| integration | `mesh_eq_primitive` | an icosphere STL vs the primitive sphere at equal `h`: `C` agrees | ≤2% + mesh-facet term |
| integration | `volume_crosscheck` | watertight meshes: `V_voxel` vs `V_mesh` | ≤1% at h = bbox/50 |
| integration | `flood_vs_wn` | on a watertight mesh both classifiers agree cell-for-cell (boundary aside) | ≥99.9% cells |
| integration | `cache_once` | the same (mesh, h) voxelises once; mass draws rescale (M2's cache honoured) | structural |
| e2e | `dirty_mesh_loud` | an open mesh takes the WN path and emits the diagnostic; a severely ambiguous one errors with `AmbiguousInterior` — never a silent cloud | structural |
| e2e | `imported_body_runs` | an imported asymmetric mesh, spun about its intermediate axis (M4): tumbles, and its `ΔΦ` decomposes/streams like any primitive (spot-check through M5/M6 paths) | structural |

---

## 6. Exit requirements

| Requirement | Check | Tol |
|---|---|---|
| mesh ≡ primitive | `mesh_eq_primitive`, `imported_body_runs` | ≤2% / structural |
| robust & loud | `dirty_mesh_loud`, `watertight_check`, `solid_angle_*` | structural |
| independent checks | `volume_crosscheck`, `flood_vs_wn`, `fast_wn_matches_brute` | ≤1% / ≥99.9% / ≤1e-6 |
| no unit guessing | `scale_mandatory` | structural |
| cache honoured | `cache_once` | structural |

## 7. Traceability

M10-R1 → scale_mandatory, parser coverage in mesh_eq_primitive · M10-R2 → watertight_check · M10-R3 → flood_vs_wn, volume_crosscheck · M10-R4 → solid_angle_*, fast_wn_matches_brute, dirty_mesh_loud · M10-R5 → volume_crosscheck · M10-R6 → mesh_eq_primitive, cache_once, imported_body_runs.
