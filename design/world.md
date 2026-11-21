# Cavendish — `world` drill-down

> Subsystem design for the `world` crate: **the source seam, generalised.** v1's seam
> (`SourceDynamics`, `DESIGN.md` §3.1) describes a rigid body — a fixed cloud carried by a pose — and
> its own doc-comment names the deformable case as "the seam's growth point … noted, not built". The
> spec's appendix (`sec:parked`) sketches the general form: `cloud_at(t)`, a `Composite` of sources,
> a `CachedField` for solver output. This crate builds that general form and makes the rigid body one
> implementation of it. Companion to `DESIGN.md` (§3.1), `design/source.md` (the rigid contract it
> wraps), `design/instrument.md` (the forward model it routes into) and `design/physics.md` (the
> non-rigid implementations).
>
> **Dependencies:** `world → gravity, source, instrument, math`. Consumed by `scenario` (a scene is a
> list of `WorldSource`s), `generate` (routing) and `physics` (implementations).

---

## 1. The decision — one seam, the rigid body as an implementation

Before this crate a `Scenario` held exactly one `Box<dyn SourceDynamics>`, and anything that was not
a rigid body had to enter beside it as a `FieldContribution`. That gives two *representations* of "a
thing with mass" — and a third the moment a simulated field appears. Settled:

- **The general source is a world-frame mass distribution that varies with time** —
  `WorldSource::world_cloud_at(t)`. A rigid body, an articulated figure, a vibrating wall and a room
  of air are all this.
- **The rigid body keeps its fast path, hidden behind the seam.** The forward model already evaluates
  a rigid source body-frame at the back-transformed arm point (`design/gravity.md` §3) and uploads its
  cloud once to the GPU. That amortisation is why rigid scenes are cheap, and rigid scenes remain a
  large share of training data. It survives as *how `Rigid` implements the seam*, not as a second
  kind of source the caller chooses.
- **The forward-model spine is not edited.** `gravity` and `instrument` are validated against the
  reference oracle and stay byte-identical. `per_ifo_generic` already has two sockets — rigid sources
  and world-frame fields — and the seam says which one a source plugs into.

What is removed is the duplicate *representation*. What is kept is the choice of *evaluation
strategy*, as an implementation detail. "One path" must not be read as "drop the amortisation".

---

## 2. Responsibility & boundaries

**Owns:** the `WorldSource` trait; the `Eval` routing enum; the `Channel` tag; the `Track` label; the
`Rigid` and `Ambient` adaptors.

**Does not own:** any physics. Rigid motion is `source`; non-rigid bodies and fields are `physics`;
the phase integral is `instrument`; routing is `generate`. `world` defines the contract between them
and nothing else — it is deliberately tiny.

**Why a crate of its own.** The trait names `instrument` types (`Detector`, `InstrumentConfig`, for
the fast-path hook, §5), and `instrument` depends on `source`; so the trait cannot live in `source`
without a cycle. Placing it in `instrument` would have meant editing a spine crate. A sibling L2 crate
above both keeps the spine's diff empty.

---

## 3. The seam

```rust
pub enum Channel { Target, Ambient }

pub enum Eval<'a> {
    Rigid(&'a dyn SourceDynamics),            // body-frame, inverse-pose — the amortised socket
    Field(&'a dyn FieldContribution<f64>),    // world-frame potential at (p, t)
}

pub struct Track { pub pose: Isometry3, pub motion: BodyMotion }

pub trait WorldSource {
    fn eval(&self) -> Eval<'_>;
    fn world_cloud_at(&self, t: f64) -> Cloud;
    fn channel(&self) -> Channel { Channel::Target }
    fn track_at(&self, t: f64) -> Option<Track> { None }
    fn body_cloud(&self) -> Option<&Cloud> { None }
    fn per_ifo_fast(&self, cfg: &InstrumentConfig, det: &Detector, t: f64) -> Option<[f64; 2]> { None }
}
```

**Contract.**
- **Pre.** Queryable on `[t − 2T, t]` for every measurement time `t`. The propagation integral
  evaluates each contribution at `t_abs = (t − 2T) + τ` across the whole arm flight (2T ≈ 1.46 s), not
  at one instant — a simulated source must cover that window before the first measurement.
- **Post.** `world_cloud_at(t)` is the mass the forward model integrates: the phase obtained through
  `eval()` equals the phase of that cloud, to the tolerance the implementation documents. Total mass
  is conserved in time unless the source says otherwise.
- **Invariant.** Pure in `t` — no observable state, the same answer in any call order. (Interior
  caches are permitted and used; they must not change results.)

`eval()` is the one place the two sockets of the phase integral are visible. It is consumed by
`generate` (§6) and by nothing a scene author writes.

---

## 4. Channels and tracks — what reaches the bundle

**`channel()`** decides which ground-truth channel records the source's phase when
`FieldSet.decomposition` is on: `Target` → `signal_targets`, `Ambient` → `signal_atmospheric`
(alongside the analytic atmospheric field). A vibrating wall is rigid but is not a target, hence the
`Ambient<W>` wrapper. The decomposition identity (`design/generate.md` §4) is unchanged: it is exact by
linearity whatever the mix of sources.

**`track_at` / `body_cloud`** decide whether a source appears on the bundle's source axis `S`. A
source with a body frame (a rigid body, an articulated figure by its root) returns both and gets a
row in `source_position`, `source_orientation`, … and `source_cloud`; a source without one (a fluid)
returns `None` and is present only through its channel. `StateBundle` itself is **unchanged** — the
`S` axis already existed (`tab:bundle`); it now carries more than one source.

---

## 5. The fast-path hook

`per_ifo_fast` lets a source supply the two single-interferometer propagation phases directly when it
has an exact reformulation cheaper than quadrature. It exists for one case — a field on a fixed
lattice, whose propagation integral is a fixed linear functional of its frames
(`design/physics.md` §4) — and is general enough for any source with that structure.

**Contract.** It must agree with `PropagationIntegral::per_ifo` of `eval()` to rounding; it is a
reformulation of the same quadrature, never an approximation. Because it reformulates the propagation
integral specifically, `generate` uses it only when that is the phase model in force; under
`QuasiStaticGradient` every field goes through `gradient_tensor`.

---

## 6. Routing in `generate`

`generate::run` sorts the scene once:

```
for src in scenario.sources:
    group = targets | ambient                      by src.channel()
    match src.eval():
        Rigid(r) → group.rigid.push(r)
        Field(f) → if PI in force and src offers per_ifo_fast: group.fast.push(src)
                   else:                                       group.fields.push(f)
ambient.fields.insert(0, atmospheric field)        # analytic GGN, if configured
```

then per (measurement, detector): one combined `delta_phi(all rigid, all fields)` plus the fast
sources' double-differences; or, when decomposing, a targets pass and an ambient pass.

**Bit-identity for v1 scenes.** A v1 scene is one `Rigid` target and (optionally) the atmosphere. It
produces the same argument lists in the same order as before, and when no source offers a fast path
*nothing is added* (not even `+ 0.0`). The bundles are therefore byte-identical, not merely close —
checked by `generate/examples/bundle_hash.rs` (§9).

`Scenario::new(Box<dyn SourceDynamics>, …)` is unchanged; it wraps its argument in `Rigid`.
`Scenario::scene(Vec<Box<dyn WorldSource>>, …)` and `.with_source(…)` build general scenes.

---

## 7. What deliberately did not change

- **`PhaseModel::delta_phi(sources, fields, det, t)`** keeps its two-list signature. Collapsing it to
  one list would be a cosmetic unification bought by editing the validated spine and every call site.
- **The differentiable path** (`compute::forward_dual`, `analysis`) remains parametric rigid sources.
  The CRB differentiates with respect to source parameters; a simulated field has none.
- **`ComputeBackend` / `EvalBatch`** do not yet carry non-rigid sources (§10).

---

## 8. Errors & API

No fallible construction. A source queried outside the span it is defined on must fail loudly —
`physics` sources panic with the offending time and span — never clamp silently: a plausible-looking
wrong field is worse than a stop. This is the `PhaseModel` precondition ("queryable on `[t − 2T, t]`")
enforced at the source.

Public surface: `WorldSource`, `Eval`, `Channel`, `Track`, `Rigid`, `Ambient` — re-exported through
`scenario` and `generate`.

---

## 9. Exit requirements (definition of done)

| Requirement | Check | Tol |
|---|---|---|
| v1 scenes unchanged | `generate/examples/bundle_hash.rs` output identical on `main` and here (24 scenes: both phase models, all field sets, free rotation, libration, atmosphere, ULDM, noise, tilted detector, jittered schedule) | **byte-identical** |
| rigid fast path ≡ general path | `world::rigid_fast_path_equals_world_cloud` | ≤1e-11 |
| a rigid source reports its track and conserves mass | `world::rigid_reports_track_and_conserves_mass` | ≤1e-12 |
| sources superpose | `generate/tests/world::sources_superpose` | ≤1e-12 |
| mixed scene decomposes | `generate/tests/world::mixed_scene_decomposes_exactly` | ≤1e-10 |
| every v1 test still passes | `cargo test --workspace` | — |
| spine untouched | `git diff main -- crates/{math,gravity,instrument,analysis,compute,reference}` empty | exact |

---

## 10. Open sub-questions

- **Batch and GPU.** `EvalBatch` encodes rigid sources as parameters and the atmosphere as modes; it
  has no encoding for a per-tick cloud or a grid field. Until it does, non-rigid scenes run through
  `generate::run` / `generate::batch` on the CPU. The grid field's response matrix is the natural GPU
  payload (a matrix–vector product); a per-tick parcel cloud needs a streaming upload
  (`milestones/V2-HANDOFF.md` step 3).
- **Per-source target channels.** With several targets, `signal_targets` is their sum. A `(S, T, D)`
  decomposition would be useful ground truth for multi-target tracking; it is a `StateBundle` change
  and so a spec change first.
- **Naming.** The spec and `DESIGN.md` call `SourceDynamics` "the source seam". It is now the rigid
  implementation's contract, with `WorldSource` the seam. The documents say so; the trait keeps its
  name to leave the spine and its tests untouched.
