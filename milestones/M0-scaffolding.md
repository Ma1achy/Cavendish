# M0 — Scaffolding & CI (implementation brief)

> Self-contained brief for the M0 build. Read with `DESIGN.md` (layering, seams) and
> `design/compute.md` §8 (the `Dual`/`Scalar` contract). No physics in this milestone.
>
> **Prereq:** none. **Delivers to:** every later milestone (the workspace, `math`, the seams, CI).
> **Crates touched:** all sixteen (skeletons); `math` in full.

---

## 1. Requirements

| ID | Requirement |
|---|---|
| M0-R1 | A cargo workspace of sixteen crates compiles, with dependency edges exactly as `DESIGN.md` §1 (edges point up the layers only; a lower layer never names a higher one). |
| M0-R2 | `math` is complete: a `Scalar` trait abstracting the numeric type, and a forward-mode `Dual` implementing it, with derivatives correct against analytic references. |
| M0-R3 | The five seam traits are declared with doc-contracts and no implementations: `SourceDynamics`, `PhaseModel`, `NoiseSource`, `ComputeBackend`, `FieldContribution`. |
| M0-R4 | Infra committed and working: `.devcontainer/`, `.gitignore`, `.github/workflows/ci.yml`. |
| M0-R5 | CI green on all four jobs (rust, gpu-nonblocking, python-skipped-cleanly, spec). |

---

## 2. Design

### 2.1 Workspace and dependency DAG

```
cavendish/
├── Cargo.toml            (workspace; resolver = "2")
├── crates/
│   ├── math/  config/                                  L0
│   ├── gravity/ reference/                             L1
│   ├── shape/ compute/ source/ instrument/ uldm/ noise/ L2
│   ├── scenario/ state/                                L3
│   ├── generate/ analysis/                             L4
│   └── sdk/ viewer/                                    L5
├── python/               (pytest home; empty until M7)
├── cavendish-spec/       (cavendish.tex)
├── milestones/ design/ DESIGN.md MILESTONES.md
└── .devcontainer/ .github/workflows/ .gitignore

edges (each points up):
  gravity→math   reference→math   shape→gravity,math   compute→gravity,source,instrument,math
  source→shape,gravity,math   instrument→gravity,source,math   uldm→math   noise→gravity,math
  scenario→source,instrument,uldm,noise,config      state→math
  generate→scenario,instrument,gravity,compute,state,noise,uldm
  analysis→gravity,source,instrument,compute,state
  sdk→generate,scenario,config,state                viewer→generate,state,gravity
```

Enforcement: the edges *are* each crate's `[dependencies]`; anything else fails review. A
`cargo-deny`/`cargo tree` check in CI is optional hardening (open question §7).

### 2.2 `math`: `Scalar` and forward-mode `Dual`

`Scalar` is the one numeric abstraction the whole kernel is generic over (`f64` for the CPU
reference, `f32` re-expressed in WGSL, `Dual` for derivatives):

```rust
pub trait Scalar:
    Copy + PartialOrd + core::fmt::Debug
    + Add<Output=Self> + Sub<Output=Self> + Mul<Output=Self> + Div<Output=Self> + Neg<Output=Self>
{
    fn from_f64(x: f64) -> Self;
    fn value(self) -> f64;          // the primal channel
    fn sqrt(self) -> Self;
    fn sin(self) -> Self;  fn cos(self) -> Self;
    fn exp(self) -> Self;  fn ln(self) -> Self;
    fn powi(self, n: i32) -> Self;
    fn abs(self) -> Self;  fn max(self, o: Self) -> Self;
}

#[derive(Clone, Copy, Debug)]
pub struct Dual { pub v: f64, pub d: f64 }   // value + one tangent
```

Also in `math`: `Vec3`, `Mat3`, `Quat` (wxyz), `Isometry3` — all generic over `Scalar` where they
sit on the kernel path (`Vec3<S>`, `Mat3<S>`); plain `f64` variants where they do not.

### 2.3 Seam traits (declared, not implemented)

Signatures per `DESIGN.md` §3; doc-comments carry the contracts (pre/post/invariants) verbatim from
the spec's contract boxes. Example shapes:

```rust
pub trait SourceDynamics { fn pose_at(&self, t: f64) -> Isometry3<f64>; /* + motion_at */ }
pub trait PhaseModel     { fn delta_phi<S: Scalar>(&self, src: &dyn Fields<S>, det: &Detector, t: f64) -> S; }
pub trait NoiseSource    { fn add(&self, t: &[f64], clean: &mut [f64], rng: &mut KeyRng); }
pub trait ComputeBackend { fn eval(&self, batch: &EvalBatch) -> SignalBatch; }
pub trait FieldContribution<S: Scalar> { fn potential(&self, p: Vec3<S>, t: f64) -> S; }
```

(Exact generics may shift in M1; the *names and roles* are fixed here so every crate can compile
against them.)

---

## 3. Equations — the `Dual` laws

```
(a + b·ε)  +  (c + d·ε)  =  (a+c) + (b+d)·ε
(a + b·ε)  ·  (c + d·ε)  =  a·c   + (a·d + b·c)·ε          (ε² = 0)
(a + b·ε)  /  (c + d·ε)  =  a/c   + (b·c − a·d)/c² · ε
f(a + b·ε)               =  f(a)  + f′(a)·b·ε               (chain rule)
  sqrt: f′ = 1/(2√a)   sin: cos a   cos: −sin a   exp: eᵃ   ln: 1/a   powi n: n·aⁿ⁻¹
```

Seeding `d = 1` on one input and `0` elsewhere makes the tangent channel the partial derivative
with respect to that input — the mechanism `analysis` (M8) builds `J` from.

---

## 4. Pseudocode

```
impl Mul for Dual:
    (v: a.v*b.v,  d: a.v*b.d + a.d*b.v)

impl Scalar for Dual:
    sqrt(self):  s = sqrt(self.v);  Dual { v: s, d: self.d / (2*s) }
    sin(self):   Dual { v: sin(self.v), d: self.d * cos(self.v) }
    ...
```

Skeleton crates: each `lib.rs` exports an empty module + the crate-level doc pointing at its
`design/*.md`; `scenario`/`state`/`generate` re-export the seam-trait names they will consume so
M0-R1's reachability holds.

---

## 5. Tests

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `dual_arithmetic` | +,−,×,÷ laws on random pairs vs symbolic results | ≤1e-12 rel |
| unit | `dual_chain` | d/dx of `sin(x²)·exp(−x)`, `1/√(1+x²)`, `ln(x)·x³` vs closed-form derivatives at 50 points | ≤1e-12 rel |
| unit | `dual_value_channel` | for every op, `Dual.v` equals the `f64` computation exactly | exact |
| unit | `quat_norm_ops` | `Quat` multiply/normalise; `Isometry3` compose/inverse round-trip | ≤1e-14 |
| integration | `workspace_builds` | `cargo build --workspace --all-features` | exit 0 |
| integration | `edges_reachable` | `generate` names every L≤3 crate's public surface; `sdk`/`viewer` name `generate`'s | compiles |
| e2e | `ci_green` | all four CI jobs pass on the skeleton (python job exits cleanly with "no sdk yet"; gpu job non-blocking) | green |

---

## 6. Exit requirements

| Requirement | Check | Tol |
|---|---|---|
| workspace | sixteen crates; dependency edges exactly §2.1 | exact |
| `Dual` verified | the §5 unit suite | ≤1e-12 |
| seams declared | five traits with doc-contracts; no impls | structural |
| infra | devcontainer builds; `.gitignore` effective; CI green | green |

---

## 7. Open questions (resolve in-milestone)

- Whether to add a `cargo-deny`/dep-graph CI check enforcing §2.1 mechanically.
- `Quat` storage order fixed as **wxyz** here (matches the bundle) — confirm no library conflict.

## 8. Traceability

M0-R1 → workspace_builds, edges_reachable · M0-R2 → dual_* , quat_norm_ops · M0-R3 → edges_reachable (names exist) · M0-R4/R5 → ci_green.
