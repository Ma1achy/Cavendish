# Cavendish — `shape` drill-down

> Subsystem design for the `shape` crate: turning **geometry into mass** — analytic primitives and
> imported triangle meshes, voxelised into the `Cloud` of body-frame elements `(x,y,z,m)` that every
> downstream stage consumes. Companion to `DESIGN.md`, `design/gravity.md` (the `Cloud` type, the
> element law, the oracle), `design/source.md` (the body library that calls this), and the spec
> (§scope "mesh → voxel cloud, primitives as oracle", §mass model, `fig:voxel`, `sec:oracle`).
>
> **Dependencies:** `shape → gravity, math` (it *produces* `gravity::Cloud`; it does not evaluate
> fields). Consumed by `source` (the body library). Mesh-format parsers are feature-gated so the
> core engine builds without them.

---

## 1. Responsibility & boundaries — and an ownership revision

**Owns:** the `Solid` occupancy seam (§2); the single lattice voxeliser (§4); the primitive
catalogue **with analytic moment oracles** (§3); mesh import — formats, units, watertightness
classification, robust inside/outside, diagnostics (§5); the cloud cache the body dictionary sits on
(§7).

**Does not own:** the `Cloud` *type*, the element law (point vs Nagy prism) and near/far routing,
the analytic **field** oracle — all `gravity`'s; the second-moment reduction `C → I, Q` —
`gravity`'s; body + trajectory assembly — `source`'s.

**Revision to earlier drill-downs.** `gravity.md` §"voxelisation" and `source.md` §6 split this
between them (gravity: algorithm; source: file handling). That scattering is what this document
replaces: the correctness domain — watertightness, winding numbers, boundary occupancy, mass
renormalisation — is neither kernel maths nor motion, and mesh-parsing dependencies do not belong in
`gravity`, which must stay a lean, pure kernel crate. `shape` takes the whole geometry→mass
pipeline; `gravity` keeps the `Cloud` type it defines and the field-side oracle; `source` calls
`shape` and keeps none of it.

**Invariant it guarantees:** a shape-produced `Cloud` is **indistinguishable downstream** from any
other cloud — exact total mass, CoM at the body-frame origin, deterministic canonical element order
— whether it came from a sphere formula or an imported mesh.

---

## 2. The `Solid` seam — one occupancy contract, one pipeline

Everything reduces to a single in-crate seam:

```rust
pub trait Solid {
    /// Occupancy at a body-frame point: 1 in the interior, 0 outside;
    /// fractional values permitted near the surface.
    fn occupancy(&self, p: [f64; 3]) -> f64;
    /// A finite bound on the support (voxelisation domain).
    fn bbox(&self) -> Aabb;
}
```

Primitives implement it analytically (a sphere's `occupancy` is a comparison); an imported mesh
implements it via its classifier (§5). **One voxeliser** (§4) samples any `Solid` onto a lattice and
emits a `Cloud`. That single pipeline is the point: primitives and meshes share every line of
element generation, so the primitives — whose moments are known in closed form — **validate the
voxeliser itself**, and a mesh then inherits that validation by construction. Composition is a
`Solid` too: `Union(Vec<Box<dyn Solid>>)` with `occupancy = max` (overlap handled correctly, no
double counting) — a scaffold is a union of cylinders. Difference/intersection are deferred (§12).

---

## 3. Primitives — the catalogue and its analytic oracle

`Sphere { r }`, `Shell { r }`, `Cuboid { half: [f64;3] }`, `Cylinder { r, half_l }`, plus `Union`.
Each exposes its closed-form moments about the CoM (uniform density, total mass `M`):

| Primitive | second moment `C` (diagonal, body axes) | notes |
|---|---|---|
| solid sphere, radius `R` | `(MR²/5)·𝟙` | `I = (2MR²/5)𝟙`, `Q = 0` — no quadrupole |
| thin shell, radius `R` | `(MR²/3)·𝟙` | `I = (2MR²/3)𝟙`, `Q = 0` |
| cuboid, sides `a,b,c` | `diag(Ma²,Mb²,Mc²)/12` | the wall/brick; `I_xx = M(b²+c²)/12` |
| cylinder, radius `R`, length `L` (axis `z`) | `diag(MR²/4, MR²/4, ML²/12)` | `I_zz = MR²/2` |

`I = tr(C)𝟙 − C` and `Q = 3C − tr(C)𝟙` follow from the one second moment (the identity pinned in
`gravity.md`/the spec); a `Union` of disjoint parts composes `C` additively. These tables are the
**moment oracle** — the voxelised primitive must converge to them (§9) — complementing `gravity`'s
**field** oracle (`sec:oracle`), which the voxelised cloud must also converge to. Two independent
oracles, one pipeline under test.

---

## 4. The voxeliser — lattice, boundary policy, and the two exact post-conditions

```rust
pub enum MassSpec { Density(f64), Total(f64) }               // ρ or M; the other derived
pub struct VoxelParams {
    pub pitch: Option<f64>,        // lattice spacing h (exactly one of pitch/target_n)
    pub target_n: Option<usize>,   // desired element count; h derived, one refinement pass
    pub supersample: u8,           // k: boundary cells get k³ occupancy sub-samples (default 2)
}
pub fn voxelise(s: &dyn Solid, v: &VoxelParams, m: MassSpec) -> Result<Cloud, ShapeError>;
```

1. **Lattice.** A regular cubic lattice of pitch `h` over `bbox`, cells visited in a fixed raster
   order. Cell centres classified by `occupancy`; interior cells get full mass `ρh³`; **boundary
   cells** (any cell whose neighbourhood straddles the surface) get `k³` sub-samples and fractional
   mass. Binary centre-testing is `k = 1`.
2. **Renormalise.** Element masses are scaled so `Σmᵢ = M` **exactly**. The monopole — the leading
   far-field term — carries no discretisation error by construction.
3. **Recentre.** Elements are translated so the discrete CoM is the body-frame origin: the dipole is
   **exactly zero**, which the rotation work assumes (rotation about the CoM; spec §5.4). The body
   frame is therefore *the authored frame translated to the CoM* — axes are never silently rotated;
   principal axes are reported downstream (`source_axes`), not imposed.
4. **Determinism.** Same `(Solid, VoxelParams, MassSpec)` → bit-identical `Cloud`, including element
   *order* (raster order is canonical). Sub-sample offsets are a fixed sub-lattice, not random —
   voxelisation is seed-free. Order matters because `f32` summation downstream is
   order-sensitive; a canonical order keeps runs reproducible across platforms.

There is deliberately no interior "hollowing": gravity needs the interior mass (unlike rendering).
Interior *coarsening* (octree super-voxels far from the surface) is a possible performance
extension, noted in §12 — an accuracy-preserving merge, not a removal.

---

## 5. Mesh import — formats, units, robustness

```rust
pub struct MeshImport {
    pub path: PathBuf,
    pub scale: Option<f64>,  // metres per model unit — MANDATORY; None ⇒ ScaleMissing (no guessing)
    pub voxel: VoxelParams,
    pub mass: MassSpec,
}
pub fn load_solid(m: &MeshImport) -> Result<(MeshSolid, MeshReport), ShapeError>;
```

- **Formats.** STL, OBJ, glTF/GLB (feature-gated crates: `stl_io`/`tobj`/`gltf`). Parsing is thin:
  everything becomes an indexed triangle soup. **Units are a trap** — STL and OBJ are unitless — so
  `scale` is mandatory; there is no guessing.
- **Realisation (M10).** `scale` is `Option<f64>` and checked *before* any file read, so a missing
  scale fails loudly and cheaply (`ScaleMissing`) — the sketch's non-optional `f64` could not express
  "mandatory but absent" at runtime. **Only the parsers gate:** the geometry core (winding number,
  watertightness, the two classifiers, volume, `MeshSolid`) is always compiled, so its integrity tests
  run in the blocking CI gate rather than skipping green behind a feature. The parsers are exercised by
  in-memory round-trip tests behind `--all-features` (a scoped `cargo test -p shape --all-features` CI
  step). The winding-number acceleration is an **error-bounded** BVH multipole (dipole + first/second
  moments) — a node is approximated only when a bound on its truncation error is below tolerance — with
  a tight tolerance for the accurate winding (`fast_wn_matches_brute` ≤1e-6) and a loose one for
  inside/outside classification (occupancy needs only the sign of `w − ½`, so it prunes aggressively).
- **Watertightness classification.** On load, an edge-manifold check: every edge shared by exactly
  two consistently-oriented triangles ⇒ *watertight*; otherwise *open/non-manifold*, with the open
  edge and flipped-triangle counts in the `MeshReport`.
- **Inside/outside — two classifiers behind one `Solid`:**
  - *Watertight fast path:* rasterise the surface into the lattice (conservative triangle–cell
    overlap), flood-fill the exterior from the bbox margin, interior = complement; boundary cells
    sub-sampled by parity ray tests against a BVH.
  - *Robust path (open/non-manifold):* the **generalised winding number** (Jacobson et al. 2013)
    at cell centres/sub-samples, thresholded at ½, evaluated hierarchically (fast winding numbers,
    Barill et al. 2018 — a BVH with far-field dipole approximations; brute force is `O(F)` per query
    and unusable at `10⁶` queries × `10⁵` triangles). Flood fill is *not* used here — it leaks
    through holes.
  - Selection is automatic from the watertightness check; the robust path always emits a
    **diagnostic** (mean `|w − round(w)|`, fraction of ambiguous samples), and a genuinely dubious
    mesh fails loudly (`ShapeError::AmbiguousInterior`) rather than voxelising to silent nonsense.
- **The volume cross-check.** For a watertight mesh the signed volume
  `V = Σ (1/6)·p₀·(p₁×p₂)` (divergence theorem) is computed at load and compared with the
  voxelised volume `N_eff·h³` — an independent consistency check that catches scale errors,
  inverted orientation, and gross classification bugs in one number.
- **Principal frame (M10-R7).** An imported mesh's inertia tensor is generally *not* diagonal in its
  authored frame, so `Orient::FreeRotation` — which reads the principal moments off the body-frame
  diagonal (M4) — would tumble it wrongly. `principal_frame(&Cloud)` re-expresses a voxelised mesh
  cloud in its principal frame (rotating by `Rᵀ`, `R` = the eigenvectors of `C` from
  `gravity::inertia`) so the inertia is diagonal, and **returns `R`** so a caller can recover the
  authored orientation. This is the one place `shape` rotates a body's axes; it is explicit and
  recorded, never silent (path (a): diagonalise-and-author-in-principal-frame). Total mass and the
  origin CoM are preserved. This is the mesh-only exception to §4's "axes are never silently rotated"
  — for a mesh the rotation is applied *and recorded*, because the authored frame is arbitrary.

---

## 6. Mass, density, and linearity

Uniform density is v1: the user supplies `Density(ρ)` or `Total(M)` (with the other derived from the
*discrete* volume, so `Σmᵢ = M` holds exactly either way). Heterogeneous density is representable —
the cloud carries per-element `m`, and the voxeliser already visits every centre, so a `ρ(x)`
callback is a cheap extension (§12) — but no import path assigns it yet (multi-material meshes are a
format problem, not a voxeliser one).

**Linearity in mass is exploited structurally:** the cache (§7) stores clouds at **unit total
mass**, and `source` applies the scenario's mass as a per-element multiply at assembly (one pass
over `N` floats). `ΔΦ` is linear in mass (spec `INV.2`), so a `Prior` drawing masses never
re-voxelises — one geometry, many masses, free.

---

## 7. The cache and the body dictionary

Voxelising a large mesh (BVH build + classification) is the expensive step, and the dataset path
reuses one body across millions of scenarios. So: an in-memory registry keyed by
`(geometry hash, scale, pitch, supersample)` — a content hash for meshes, the parameter tuple for
primitives — holding `Arc<Cloud>` at unit mass. The body dictionary (`source.md` §6, `scenario.md`
§4) is a set of registry entries plus mass ranges; a `Prior` draw resolves to a shared cloud and a
mass scalar. An optional on-disk cloud cache (same key) is deferred until load times demand it
(§12).

---

## 8. The autodiff boundary

Voxelisation is **preprocessing, off the differentiable path**: the cloud's geometry is constant
with respect to autodiff, and `d(signal)/d(mass)` flows through the assembly multiplier (§6), which
is on the `Dual` path. Consequence for `analysis`/CRB: sensitivities to *shape parameters* (a
primitive's dimensions, a mesh's scale) are out of scope in v1 — obtainable only by finite
differences of re-voxelised clouds, or analytically for primitives (closed-form `∂C/∂dims`) if ever
needed. Stated here so the CRB's parameter set is chosen with eyes open.

---

## 9. Accuracy model — what error, where it matters

The gradiometer senses the cloud's **field**, so the correctness budget is stated in field terms,
split by regime:

- **Far field** (`D ≫` body size): the multipoles. Monopole error is zero (renormalisation §4);
  dipole is zero (recentring §4); the leading error is the **quadrupole**, first order in `h`
  from boundary misclassification, with the constant shrunk by supersampling `k`. The
  *requirement* is the measured convergence test — halving `h` halves the `C` error on the sphere
  and cuboid — not an asserted order.
- **Near field** (standoff `d` from the nearest element): replacing a voxel by a point at its
  centre costs a relative field error of order `(h/d)²` (the voxel's own second moment against its
  monopole). Rule of thumb: `h ≲ d_min/10` keeps discretisation below the percent level — and
  below that, `gravity`'s **Nagy prism element** (the exact homogeneous-prism law, spec §mass
  model) takes over near the body, which is precisely the near/far element routing `gravity` owns.
  `shape`'s job is only to choose `h` sensibly against the scenario's closest approach.
- **Cost coupling.** `N ≈ V/h³` multiplies every forward evaluation (per measurement × detector ×
  arm sample). At `N = 10⁵`, `D = 4`, ~146 arm samples, that is ~6×10⁷ kernel evaluations per
  measurement per source — GPU-comfortable, CPU-heavy — and the quasi-static path collapses the arm
  samples when appropriate. Guidance: `target_n` of `10³–10⁵` covers the anchors and Gradar scenes;
  a configurable cap (`ShapeError::TooManyElements`, default 2×10⁶) protects the 8 GB card from an
  accidental `h`.

---

## 10. Errors & API

`ShapeError`: `UnreadableMesh(String)`, `ScaleMissing`, `EmptySolid` (pitch coarser than the
geometry), `AmbiguousInterior(f64)` (the robust classifier's diagnostic exceeded threshold, carrying
`A`), `TooManyElements`, `BadParams`, plus warning-grade counts in `MeshReport` (open edges, flipped
triangles). The mesh variants carry payloads, so `ShapeError` is **not** `Copy`/`Eq` (it stays
`Clone`/`Debug`/`PartialEq`) — a dirty mesh fails loudly *with context*. Public surface: `Solid` + the
primitive types + `Union`; `voxelise`; `MeshImport`/`load_solid`/`MeshReport`; the mesh core
(`TriSoup`, `MeshSolid`, `voxelise_mesh`, `principal_frame`, `mesh_cache_key`, `signed_volume`,
`winding_brute`) and the feature-gated `parse_stl`/`parse_obj`/`parse_gltf`; `MassSpec`/`VoxelParams`;
the registry (`BodyDict` entry resolution). `source` consumes clouds; `gravity` is a type/oracle *and*
inertia-reduction dependency (`principal_frame` calls `gravity::inertia`).

---

## 11. Exit requirements (definition of done)

| Requirement | Check | Tol |
|---|---|---|
| analytic moments | the §3 table verified against independent formulae | exact |
| mass & CoM | `Σmᵢ = M`; discrete CoM at the origin — every produced cloud | exact |
| determinism | same inputs → bit-identical cloud, element order included | exact |
| convergence | halving `h` halves the `C` error (sphere, cuboid); `k` shrinks the constant | tol |
| shell theorem | a voxelised sphere's external field matches a point mass (`d ≥ 2R`) | tol |
| field oracle | voxelised primitives converge to `gravity`'s analytic field oracle as `h → 0` | tol |
| mesh ≡ primitive | an icosphere mesh voxelises to the primitive sphere's moments at equal `h` (`mesh_eq_primitive`) | tol |
| winding number | the accelerated winding number matches brute force (`fast_wn_matches_brute`); the two classifiers agree (`flood_vs_wn`); `w` matches the analytic solid angle (`solid_angle_closed_cube`) | 1e-6 / 99.9% / 1e-10 |
| volume cross-check | watertight mesh: voxelised volume vs divergence-theorem volume (`volume_crosscheck`) | ≤1% |
| dirty mesh | an open mesh takes the robust path with a diagnostic; ambiguous ⇒ `AmbiguousInterior` (`dirty_mesh_loud`, `watertight_check`) | structural |
| principal frame | a mesh with off-diagonal inertia is re-expressed in its principal frame so FreeRotation holds (`principal_frame_diagonalises`, `imported_body_runs`) | structural |
| anchor via shape | the concrete-wall anchor (~50 µrad) reproduced with the voxelised cuboid | model |
| cache & linearity | a dictionary body voxelises once; mass draws re-scale, never re-voxelise (`cache_once`) | structural |

The shell theorem row is the flagship: it exercises lattice, boundary policy, renormalisation and
the field path in one closed-form comparison.

---

## 12. Open sub-questions (resolve in implementation)

- **Fast winding numbers: library vs hand-rolled.** *Resolved (M10): hand-rolled.* The BVH carries
  per-node multipole moments (dipole + first/second) and is shared with the parity-ray path; a file
  format is a vetted dependency, but the winding number is the correctness core and is written here,
  verified against the analytic solid angle and brute force.
- **Format set.** STL/OBJ/glTF proposed; PLY and STEP (CAD) deferred until a real need appears.
- **Interior coarsening.** Octree super-voxels away from the surface — an accuracy-preserving `N`
  reduction for very large bodies; interacts with `gravity`'s element law (bigger prisms near-exact).
- **Heterogeneous density.** The `ρ(x)` callback is cheap; the import-side assignment (materials,
  per-region density) is the real question.
- **CSG beyond union.** Difference/intersection as `Solid` combinators if scenes need cavities.
- **Shape derivatives for the CRB.** Analytic `∂C/∂dims` for primitives, if `analysis` ever wants
  shape in the Fisher parameter set (§8).
- **On-disk cloud cache.** Same key as the registry; only if mesh load times start to matter.
