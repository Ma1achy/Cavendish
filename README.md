# Cavendish

*A Rust gravitational atom-gradiometer simulator. Status: **design complete, scaffolding — not yet implemented**.*

Cavendish simulates the differential phase a spatially-separated array of atom gradiometers
(AION-10 geometry) reads from moving external masses, and dumps the **entire** simulation state as
torch-ready tensors. Generating ML datasets at scale is a primary intended use — it is the data
engine for **Gradar** (passive gradiometric tracking) — but the engine emits the full record and
lets the consumer decide what is input, label, target, or context. The task lives downstream.

# **Documents**

---

The design *is* the contract. Read top-down; lower documents defer to higher ones.

- **`cavendish-spec/cavendish.tex`** — the authoritative specification (physics + requirements: the *what*).
- **`DESIGN.md`** — top-level engineering design (the *how*: layering, the four seams, the data contract).
- **`design/*.md`** — twelve subsystem drill-downs, one per crate (types, the seam, exit requirements).
- **`MILESTONES.md`** — the vertical-slice build plan (M0–M10 + the reference-port thread).
- **`milestones/*.md`** — per-milestone implementation briefs (requirements, equations, pseudocode, tests, exit).
- **`docs/notes/`** — historical scratchpads and tombstones; superseded, **not** the contract.

# **Guardrails**

---

The invariants the code must hold — enforced by tests and review:

Dependencies point **up the layers only** · the engine **dumps the entire state** and imposes no task ·
`FieldSet` is a **cost knob**, never a task definition · kernels are **`Scalar`-generic** (autodiff via
forward-mode `Dual`) · the CPU backend is the **bit-exact reference**, the GPU validated against it (≤1e-4) ·
**differential-first** (never form large absolute potentials) · anchors are validated against an
**independent reference** — George's cases ported to Rust, quadrature vs voxels agreeing — **not** remembered
figures · **no off-the-shelf physics engine** (a hand-written `Scalar`-generic integrator) · British English.

# **Layout**

---

```
Cargo.toml            workspace (resolver 2)
crates/               the sixteen crates, by layer:
  math/ config/                                     L0
  gravity/ reference/                               L1
  shape/ compute/ source/ instrument/ uldm/ noise/  L2
  scenario/ state/                                  L3
  generate/ analysis/                               L4
  sdk/ viewer/                                      L5
python/               SDK package + pytest (M7)
cavendish-spec/       the LaTeX spec
design/ milestones/   the drill-downs and briefs
.claude/skills/       project skills for Claude Code sessions
.devcontainer/ .github/  dev environment and CI
```

The dependency edges *are* each crate's `[dependencies]`; a lower crate never names a higher one.

# **Development**

---

```
cargo build --workspace      # the empty skeleton compiles
cargo test  --workspace      # the M0 gate (fmt + clippy + tests in CI)
```

A dev container (`.devcontainer/`) provisions the toolchain (Rust + wgpu via software Vulkan + maturin).
GPU tests run under lavapipe in CI and are non-blocking; the `reference` crate's agreement suite is the
anchor gate; the `python` job is skipped until M7 adds the PyO3 extension.

# **Status**

---

M0 skeleton: every crate compiles, dependency edges wired, infra in place, CI green — **no engine code yet**.
Work proceeds one milestone at a time against its brief. Start at `milestones/M0-scaffolding.md` (`math`'s
`Scalar`/`Dual` and the seam traits), then M1 (the physics spine and its first `reference::cuboid` anchor).
George's validation cases live upstream at `Thranduil02/atom-interferometry`; see `milestones/reference-port.md`.