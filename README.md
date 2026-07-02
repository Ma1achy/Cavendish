# Cavendish

A Rust gravitational atom-gradiometer simulator (AION-10 geometry): it simulates the
differential phase a spatially-separated array of gradiometers reads from moving external
masses, and dumps the complete simulation state as torch-ready tensors. Generating ML
datasets at scale is a primary intended use — it is the data engine for **Gradar**
(passive gradiometric tracking) — but the engine emits the full record and lets the
consumer decide what is input, label, target, or context.

## Document hierarchy (the contract)

Read in this order; lower documents defer to higher ones:

1. **`cavendish-spec/cavendish.tex`** — the authoritative specification (physics + requirements: the *what*).
2. **`DESIGN.md`** — top-level engineering design (the *how*: layering, seams, the data contract).
3. **`design/*.md`** — twelve subsystem drill-downs, one per crate (types, seam, exit requirements).
4. **`MILESTONES.md`** + **`milestones/*.md`** — the build plan and per-milestone implementation
   briefs (requirements, equations, pseudocode, tests, exit). Start at
   `milestones/M0-scaffolding.md`.

`docs/notes/` is historical and superseded — not the contract.

## Repository layout

```
Cargo.toml            workspace (resolver 2)
crates/               the sixteen crates, by layer:
  math/ config/                                   L0
  gravity/ reference/                             L1
  shape/ compute/ source/ instrument/ uldm/ noise/ L2
  scenario/ state/                                L3
  generate/ analysis/                             L4
  sdk/ viewer/                                    L5
python/               SDK package + pytest (M7)
cavendish-spec/       the LaTeX spec
design/ milestones/   the drill-downs and briefs
.devcontainer/ .github/  dev environment and CI
```

Dependency edges point **up the layers only** (a lower crate never names a higher one).
The edges are each crate's `[dependencies]`; that is enforced by review.

## Building

```
cargo build --workspace      # the empty skeleton compiles
cargo test  --workspace      # the M0 gate (fmt + clippy + tests in CI)
```

Use the dev container (`.devcontainer/`) for a reproducible toolchain (Rust + wgpu via
software Vulkan + maturin). GPU tests run under lavapipe in CI and are non-blocking; the
`reference` crate's agreement suite is the anchor gate.

## Current state

M0 skeleton: every crate compiles, dependency edges wired, infra in place, CI green. The
first implementation session is **M0** — `math`'s `Scalar`/`Dual` and the seam traits —
then M1 (the physics spine and its `reference::cuboid` anchor). Each milestone brief is a
self-contained unit of work.

George's validation cases (the `reference` oracle) are the upstream repo
`Thranduil02/atom-interferometry`; see `milestones/reference-port.md`.
