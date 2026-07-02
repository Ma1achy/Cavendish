# Cavendish — `gravity` drill-down

> Subsystem design for the `gravity` crate. **Foundational** — everything depends on it, and it
> builds and verifies with no other subsystem present. Companion to `DESIGN.md` (§3.5 data contract,
> §5 cross-cutting, §6 inventory) and the spec (§4 mass model, `sec:oracle`, `fig:voxel`, the
> invariants `INV.*`, tests `T2.*`). Signatures are the intended contract; types may gain
> fields in implementation but not change shape.

---

## 1. Responsibility & boundaries

**Owns:** the `Cloud` (a body's mass distribution), the element finite-size law, the
field kernel (`potential` / `field` / `gradient_tensor`), the load-time reductions (multipole
moments **and** the second-moment/inertia tensor), the near/far routing, and the analytic
**oracle**. (Voxelisation — geometry → `Cloud` — moved to the dedicated `shape` crate,
`design/shape.md`; `gravity` keeps the `Cloud` type and the element law.)

**Does not own:** devices or parallelism (that is `compute`), trajectories or poses-over-time
(`source`), the instrument or the phase integral (`instrument`). `gravity` is pure, synchronous,
and **generic over the scalar type** — no GPU, no `async`, no global state.

**Promises (the invariants the crate guarantees):** in vacuum the gradient tensor is symmetric
and trace-free (`INV.1`); a distant cloud's field equals its monopole's (`INV.3`); the field converges to the analytic
oracle as the cloud is refined (with `shape`'s voxeliser, `T2.1/T2.2`); the kernel is differentiable
via the scalar seam.

---

## 2. The `Scalar` seam (autodiff)

The kernel is written **once**, generic over a scalar, so `f64`, `f32`, and a forward-mode dual
all flow through the same code — this is how the Fisher/CRB Jacobians (spec §Gradar) are obtained
without a second implementation. (The wgpu path re-expresses the kernel in f32 WGSL and does *not*
use this seam; autodiff is the CPU/CUDA path.)

```rust
pub trait Scalar:
    Copy + Add<Output=Self> + Sub<Output=Self> + Mul<Output=Self>
        + Div<Output=Self> + Neg<Output=Self>
{
    fn from_f64(x: f64) -> Self;   // constants: G, softening, coefficients
    fn sqrt(self) -> Self;
    fn recip(self) -> Self;        // 1/self  (one division, reused for 1/r powers)
}
// impl Scalar for f64, f32, Dual<f64> (forward-mode, one seed dimension).
```

**The differentiation pattern.** To obtain `∂(output)/∂(param)`, seed `param` as a dual and let
it propagate: the body→world pose transform (§3) and the kernel (§5) are both generic over
`Scalar`, so a dual source position, velocity, or moment reaches the scalar output. Forward-mode
suits the handful of parameters per source the CRB needs; reverse-mode is a later option if the
parameter count grows. Geometry decisions that are *not* differentiated (the near/far cutoff
test) are done in `f64` outside the generic path.

---

## 3. `Cloud` — the mass distribution (SoA, body-frame)

Resolves the `DESIGN.md` §7 layout question: **structure-of-arrays, body-frame, fixed.** Separate
coordinate arrays give GPU coalescing and CPU SIMD; body-frame + a per-tick pose is exactly the
"upload the cloud once, upload parameters per tick" shape (spec §3.4).

```rust
pub struct Cloud {
    // body-frame element centres (SoA) + masses
    pub x: Vec<f64>, pub y: Vec<f64>, pub z: Vec<f64>,
    pub m: Vec<f64>,
    pub element: ElementKind,         // shared finite-size law (§4)
    // cached load-time reductions (§6), about the centre of mass:
    pub com: Vec3,
    pub multipole: Multipole,         // M, p(≈0), Q
    pub inertia: Inertia,             // C, I, principal axes + moments
}
```

**Body-frame evaluation (the key trick).** There are many elements and few field points (atom
clouds), so we never move the cloud to world frame. Instead, transform the *point* into the body
frame, sum over the fixed cloud, and rotate the result back. For a pose `(R, t)`
(world = `R·body + t`):

```
point_body          = Rᵀ (point_world − t)
(g_body, Γ_body)    = Σ over elements                     // §5
g_world             = R · g_body
Γ_world             = R · Γ_body · Rᵀ                     // (0,2) tensor transform
```

One transform per point instead of `N` per tick; the cloud stays uploaded-once.

---

## 4. Elements — the near-field finite-size law

The element kind only matters in the **near** field, where an element's size is comparable to its
distance to the point; far away every element is a point mass. Default is `Point` with softening
(guards the `r→0` singularity; external sources never sit on an atom, but it keeps the kernel
total).

```rust
pub enum ElementKind {
    Point  { softening: f64 },   // default; −Gm/√(r²+ε²)
    Sphere { radius: f64 },      // exact: point outside (shell theorem), linear inside
    Cube   { half: f64 },        // prism law (Nagy); rarely needed if voxels are small
}
```

Each kind contributes `(potential, field, gradient-tensor)` at a body-frame offset `d`. Only
`Point` is on the critical path; `Sphere`/`Cube` are convergence aids validated against the
oracle (§9).

---

## 5. The kernel — analytic, differential-first

Three functions, generic over `Scalar`, each a sum of per-element contributions. The gradient
tensor is computed **analytically** (each element's exact Hessian), never by finite-differencing
the potential — this is the differential-first rule (spec `nfr:cond`) that keeps f32 above the
mrad floor, and it makes trace-freeness exact by construction.

```rust
pub fn potential<S: Scalar>(cloud: &CloudView<S>, p_body: Vec3<S>) -> S;
pub fn field<S: Scalar>(cloud: &CloudView<S>, p_body: Vec3<S>) -> Vec3<S>;          // g = −∇Φ
pub fn gradient_tensor<S: Scalar>(cloud: &CloudView<S>, p_body: Vec3<S>) -> Mat3<S>; // Γ = ∇g
```

Per `Point` element at offset `d = p − xᵢ`, `r² = d·d + ε²`:
- `Φ  += −G mᵢ / r`
- `g  += −G mᵢ d / r³`
- `Γ  += −G mᵢ / r³ · (𝟙 − 3 d dᵀ / r²)`   ⟹ `tr Γ = 0`, `Γ = Γᵀ` exactly.

`CloudView<S>` is the borrowed, scalar-typed view of a `Cloud` (zero-copy for `f64`; for a dual
run the differentiated coordinates are lifted to `S`). The reduction over elements is a plain
fold here; `compute` parallelises/ports it (the CPU backend calls these directly).

---

## 6. Reductions — once per body, at load

Both reductions are the same second-moment sum over the cloud, computed once and cached on the
`Cloud`. (Spec: the multipole far field; the `Q`/`I` shared-axes identity.)

```rust
pub struct Multipole { pub m: f64, pub p: Vec3, pub q: Mat3 }      // monopole, dipole(≈0), quadrupole
pub struct Inertia   { pub c: Mat3, pub i: Mat3,                   // second moment, inertia
                       pub axes: Mat3, pub moments: [f64;3] }      // principal frame (eig of C)
```

- `C = Σ mᵢ (rᵢ−com)(rᵢ−com)ᵀ`; then `Q = 3C − (tr C)𝟙`, `I = (tr C)𝟙 − C` — linear maps of `C`,
  so `axes` (eigenvectors of the symmetric `C`) serve all three. `moments` (the principal moments)
  and `axes` are what `source`'s `FreeRotation` integrates against (spec §motion).
- **Far-field evaluation** from `(M, Q)` is the multipole series (dipole vanishes about the com);
  same body-frame-then-rotate pattern as §3.

The symmetric-3×3 eigensolve is a closed-form/Jacobi routine (no external linalg dependency on the
hot path; runs once per template).

---

## 7. Near/far routing

Per body, per field point: if `|p_world − com| > far_cutoff`, evaluate from the multipole (§6);
else the direct element sum (§5). `far_cutoff` is set by the prior's closest-approach distribution
(spec). **Continuity requirement:** at the cutoff the truncated multipole must match the direct
sum to a stated tolerance — asserted by a test (§9), and the empirical basis of `T2.4`.

---

## 8. Cloud construction — owned by `shape`

`gravity` defines the `Cloud` type and a raw constructor from explicit elements
(`Cloud::from_elements`); it does **not** build clouds from geometry. Turning primitives or meshes
into a `Cloud` — the lattice voxeliser, watertightness, winding numbers, mass renormalisation and
CoM-centring — is the **`shape` crate**'s job (`design/shape.md`), which produces a `gravity::Cloud`
and hands it on. This keeps mesh-parsing dependencies and the occupancy-classification correctness
domain out of the lean kernel crate. What `gravity` keeps is the element law, the field evaluation,
the reduction, and the analytic **field** oracle the voxelised cloud is validated against (§9) —
distinct from `shape`'s **moment** oracle.

---

## 9. The analytic oracle (validation only)

Closed-form / quadrature potentials of homogeneous primitives — the reference the voxel cloud is
checked against, **never used in generation** (spec `sec:oracle`):

- **Point:** `−Gm/r`.
- **Sphere:** exact (shell theorem) — point outside, linear inside.
- **Cylinder:** the radial quadrature in the spec (the `T2.1/T2.4` reference).
- **Cuboid:** the rectangular-prism closed form and its derivatives (Nagy, `nagy2000`).

Kept in a `gravity::oracle` module behind a feature/test boundary so it is obviously not part of
the generation path.

---

## 10. Errors & invariants

- **Construction validates, the hot path is infallible.** `gravity`'s `Cloud::from_elements`
  validates only the trivial cases (empty cloud); the geometry constructors and their rich failure
  modes live in `shape` (`ShapeError`). After construction `potential`/`field`/`gradient_tensor`
  cannot fail and do not allocate.
- The crate **guarantees** `INV.1` (Γ symmetric, trace-free in vacuum — exact by §5) and
  converges to `INV.3`/oracle within tolerance.

---

## 11. Public API surface

What other crates name: `Scalar`, `Cloud`, `CloudView`, `ElementKind`, `Multipole`, `Inertia`,
`potential` / `field` / `gradient_tensor`, `Cloud::from_elements`, and (test/feature only) `oracle`.
`shape` builds `Cloud`s (and consumes the reduction); `compute` consumes `CloudView` + the kernel
fns; `source` consumes `Cloud` + `Inertia`; `instrument` consumes `field`/`gradient_tensor`.

---

## 12. Exit requirements (definition of done)

`gravity` is complete when these pass (the spec's exit criteria, made concrete):

| Test | Check | Tol |
|---|---|---|
| `INV.1` | Γ symmetric and trace-free at points outside an arbitrary cloud | machine ε |
| `INV.3` | distant cloud field = monopole (point mass at `com`) | tol |
| `T2.1`/`T2.2` | sphere & cylinder clouds' field matches the analytic oracle (the voxel→0 convergence itself is `shape`'s, `design/shape.md`) | tol |
| `T2.4` | cylinder-vs-point sweep over `(R,H,distance)` reproduces departure-from-monopole | match |
| near/far | truncated multipole matches direct sum at `far_cutoff` | tol |
| reductions | `C`/`I`/`Q` and principal axes match analytic for a uniform rod/cuboid; `Q`,`I` share axes | tol |
| autodiff | `∂Φ/∂param` via `Dual` matches central differences | ~1e-6 |

These need **no** other subsystem — `gravity` is buildable and verifiable in isolation, which is
why it leads the drill-down.

---

## 13. Open sub-questions (resolve in implementation)

- **Dual backing.** Hand-rolled `Dual<f64>` vs a crate (`num-dual`). Leaning hand-rolled to keep
  the `Scalar` seam minimal and dependency-free.
- **Eigensolve.** Closed-form symmetric-3×3 vs iterative Jacobi — both fine at once-per-template;
  pick on numerical robustness for near-degenerate moments (which is exactly the Dzhanibekov case
  that matters, spec §motion).
- **Multipole order.** Quadrupole is the floor (it carries shape); whether octupole earns its
  place is a `T2.4`-driven empirical call, not fixed now.
- **`CloudView` for duals.** Exact mechanism for lifting only the differentiated coordinates to
  `S` while keeping the rest `f64` — an ergonomics detail of the `Scalar` seam.
