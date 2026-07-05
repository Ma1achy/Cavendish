# Cavendish

*A Rust engine for atom-interferometer gravity gradiometry. It simulates the differential phase a
spatially-separated array of cold-atom gradiometers reads from moving external masses, and dumps the
**entire** simulation state as torch-ready tensors. All ten milestones (M0–M10) are complete.*

What follows is a short tour: the problem the engine exists to feed, how it is built, how its numbers
are checked, and how to run it.

Cavendish simulates the differential phase `ΔΦ` a spatially-separated array of cold-atom gradiometers
(AION-10 geometry) reads from moving external masses, and emits the **whole** per-measurement record —
source motion, static shape and inertia, array geometry, the multi-channel signal with its ground-truth
decomposition, and derived spectra — as one typed, tensor-native bundle. Generating ML datasets at scale
is a primary intended use: it is the data engine for **Gradar**, passive gradiometric tracking. But the
engine imposes no task — it dumps the entire state and lets the consumer decide what is input, label,
target, or context. The task lives downstream.

# **The Problem**

---

An atom gradiometer measures the tidal gravitational field by dropping cold-atom clouds and reading the
phase an interferometer accrues between two vertically separated points. A moving external mass — a
vehicle, a tumbling body, a person — perturbs that field, so the array's differential phase carries a
signature of the mass's position, motion, and shape. Recovering the mass from the signal — localising,
tracking, characterising it — is the downstream machine-learning problem, *Gradar*.

Cavendish is the forward model that generates the labelled data that problem needs. Given a scene of
sources, a detector array, a measurement schedule, and a noise/background stack, it computes `ΔΦ` end to
end and records every ground-truth quantity that produced it — because a dataset is only as useful as the
labels it carries, and the useful label is rarely known in advance. The expectation going in was that
completeness, not cleverness, is what keeps a simulator worth building: emit the whole record once, and
every downstream use — including the unimagined ones — stays possible.

# **The Engine**

---

Cavendish is one Cargo workspace: the `sim-core` engine plus two consumers (`sdk`, `viewer`) that depend
on it **one-way**. Internally the engine is strictly layered — dependencies point *up the layers only*, a
lower layer never naming a higher one. That single rule is what keeps the physics free of Python and UI,
and lets the SDK and viewer be swapped without touching a kernel.

```
L5  interfaces   sdk (PyO3 → torch)              viewer (egui/wgpu)
L4  execution    generate (run / stream; batch)  analysis (Fisher / CRB)
L3  assembly     scenario (Scenario, Prior)       state (StateBundle)
L2  domain       shape  source  instrument  uldm  noise   compute (CPU / GPU)
L1  kernel       gravity (Cloud, field, oracle)   reference (independent oracle)
L0  foundation   math (Scalar, Dual, Isometry3)   config (schema, Prior)
```

A few ideas carry the weight:

- **One kernel, three number types.** The gravity math is generic over a `Scalar` trait, so `f64`, `f32`,
  and forward-mode dual numbers all flow through the *same* code. Autodiff is therefore structural, not
  bolted on — which is what makes the Fisher/Cramér–Rao analysis possible at all.
- **A CPU reference, a GPU for scale.** The CPU backend calls the kernel directly (rayon, `f64`) and is
  the bit-exact oracle; the GPU backend re-expresses the same maths in WGSL (`f32`, *differential-first* —
  it forms `Γ_zz·L` directly, never subtracting two large phases) and is validated against the CPU to
  `≤1e-4`. Both satisfy one `ComputeBackend` trait.
- **Geometry becomes mass.** The `shape` crate voxelises primitives (with analytic moment oracles) and,
  from M10, arbitrary **triangle meshes** (STL/OBJ/glTF) — with a robust generalised-winding-number path
  that classifies dirty, open, or non-manifold meshes and fails *loudly* rather than emitting silent
  garbage. A mesh is indistinguishable downstream from a primitive.
- **Motion without a physics engine.** Closed-form paths (linear, oscillation, orbit/flyby) plus two ODE
  motions — a chaotic pendulum and the torque-free Euler top — integrated by one hand-written symplectic
  integrator, itself `Scalar`-generic so a trajectory can be differentiated through.
- **The whole state, once.** The engine's job ends at the `StateBundle`: the full per-measurement record
  as one torch-ready struct. `FieldSet` is a **cost knob**, never a task definition — the default is
  dump-everything; selecting fields only trims the dump for compute or storage.
- **Ground truth, decomposed.** By superposition the signal splits into target / atmospheric / ULDM /
  noise channels that sum back to the total exactly — so every contribution is labelled at source.
- **Identifiability.** The `analysis` crate runs the forward model in dual numbers to assemble the
  Fisher information and the Cramér–Rao bound, and scores array geometries against it.

## Guardrails

The invariants the code holds, enforced by tests and review: dependencies point **up the layers only** ·
the engine **dumps the entire state** and imposes no task · kernels are **`Scalar`-generic** (autodiff via
forward-mode `Dual`) · the CPU backend is the **bit-exact reference**, the GPU validated against it · work
is **differential-first** (never form large absolute potentials) · anchors are checked against an
**independent reference** — George's cases ported to Rust, quadrature versus voxels agreeing — **not**
remembered figures · **no off-the-shelf physics engine** · British English throughout.

# **Validation**

---

Numbers are never trusted because they look right. Each physics anchor is asserted against an *independent*
oracle — the `reference` crate, George's validation cases computed by direct quadrature — so agreement
means two methods (quadrature and voxels) arriving at the same value, not a remembered figure reproduced.

- **Anchors.** The concrete-wall (`≈ 50 µrad`), moving-mass (`≈ 7 mrad`), and oscillation (`≈ 2 mrad`)
  cases reproduce to tolerance on properly voxelised clouds.
- **Shell theorem.** A voxelised sphere's external field matches a point mass; `ΔΦ` is exactly linear in
  source mass.
- **CPU ≡ GPU.** The two backends agree across the anchors to the validation tolerance; batched runs are
  invariant and replay identically from a seed.
- **Decomposition identity.** `signal_targets + signal_atmospheric + signal_uldm + signal_noise == signal`
  to numerical precision.
- **Spectra and bounds.** Lomb–Scargle recovers a planted line on a gappy, non-uniform schedule; the CRB
  matches a known-Fisher analytic case.

# **Dependencies**

---

- **Rust** (stable toolchain; `rustfmt` + `clippy`).
- A **wgpu-capable GPU** for the GPU backend and the viewer — Metal on macOS, Vulkan on Linux, or
  **software Vulkan/OpenGL** (lavapipe/llvmpipe) where no GPU is available (CI and the dev container).
- **Python 3.11 + maturin + torch** for the SDK.

The quickest entry point is the dev container (`.devcontainer/`), which provisions the whole toolchain —
Rust, the software-Vulkan/GL stack, and maturin. On a real display the viewer uses the native GPU; in a
headless container it falls back to software rendering (no GPU passthrough exists to a Linux VM on macOS).

# **Usage**

---

Build and test the workspace:

```
cargo build --workspace
cargo test  --workspace          # fmt + clippy + tests are the CI gate
```

Run the viewer — an egui/wgpu inspector for a run (3D scene, signal and periodogram panels, a time
scrubber):

```
cargo run -p viewer              # a native window (Metal / Vulkan, or software GL when headless)
```

Drive the engine from Python and receive torch tensors:

```
maturin develop --manifest-path crates/sdk/Cargo.toml
```

```python
import cavendish as cv

body = cv.cuboid(half=[0.2, 0.2, 0.2], pitch=0.2, mass=1000.0)
scenario = cv.Scenario(
    body,
    cv.Trajectory(placement=[3.0, 0.0, 0.0]),   # a static cuboid at 3 m standoff
    cv.DetectorArray.line([0.0, 1.0]),           # two detectors
    cv.Schedule.uniform(2.0, 4),                 # four measurement cycles
    field_set=cv.FieldSet(decomposition=True),
    uldm=cv.UldmConfig(amplitude=1e-3, frequency=0.1),
    seed=3,
)

bundle = cv.run(scenario)        # every field is a torch tensor
bundle.signal                    # (T, D) differential phase per detector
bundle.source_position           # (S, T, 3) ground-truth track
bundle.signal_targets            # the decomposed target-only channel
```

For datasets, describe a `cv.Prior` over the scenario parameters and `cv.stream` it in memory-bounded
batches — reproducible from a single seed.

# **Repository**

---

| Path | Contents |
| --- | --- |
| `crates/math`, `crates/config` | **L0** — the `Scalar`/`Dual` autodiff foundation, vectors, quaternions, `Isometry3`; the typed config schema and `Prior`. |
| `crates/gravity`, `crates/reference` | **L1** — the differential-first kernel (`Cloud`, potential/field/gradient, analytic oracle); the independent reference oracle (George's cases by quadrature). |
| `crates/shape` `compute` `source` `instrument` `uldm` `noise` | **L2** — geometry→mass and mesh import; the CPU/GPU backends; source dynamics; the phase model; the ULDM line; the noise stack. |
| `crates/scenario`, `crates/state` | **L3** — the runnable `Scenario`/`Schedule`/`Prior`; the `StateBundle` output contract. |
| `crates/generate`, `crates/analysis` | **L4** — run/stream orchestration and batch dispatch; the Fisher/CRB analysis. |
| `crates/sdk`, `crates/viewer` | **L5** — the PyO3/torch SDK; the egui/wgpu inspector. |
| `python/` | the SDK Python package and its pytest suite. |
| `cavendish-spec/` | `cavendish.tex`, the authoritative specification. |
| `design/`, `milestones/` | the per-subsystem design drill-downs and per-milestone implementation briefs. |
| `.devcontainer/`, `.github/` | the reproducible dev environment and CI. |

The dependency edges *are* each crate's `[dependencies]`; a lower crate never names a higher one.

# **Documents**

---

The design *is* the contract. Read top-down; lower documents defer to higher ones.

- **`cavendish-spec/cavendish.tex`** — the authoritative specification (physics + requirements: the *what*).
- **`DESIGN.md`** — the top-level engineering design (the *how*: layering, the four seams, the data contract).
- **`design/*.md`** — the per-subsystem drill-downs (types, the seam, exit requirements).
- **`MILESTONES.md`** — the vertical-slice build plan (M0–M10 + the reference-port thread).
- **`milestones/*.md`** — the per-milestone implementation briefs (requirements, equations, pseudocode, tests, exit).

# **Status**

---

**Complete.** All ten milestones (M0–M10) have landed on `main` with CI green — the physics spine, the
motion library, the detector array, rotation, the channels and their decomposition, the GPU/batch path,
the Python SDK, the CRB analysis, the viewer, and mesh import. The engine runs `Scenario → StateBundle`
end to end, on CPU or GPU, from Rust or Python.

To read the code, start at `DESIGN.md` for the shape of the whole, then a subsystem's `design/*.md` and its
`milestones/*.md` brief. George's validation cases live upstream at `Thranduil02/atom-interferometry`; see
`milestones/reference-port.md` for how they are ported into the independent `reference` oracle.
