---
name: cavendish-conventions
description: Core architecture and conventions for the Cavendish repo — the layered crate graph, the four seam traits, Scalar-generic kernels, the dump-everything data contract, differential-first arithmetic, and British English. Consult before writing or reviewing any crate code, adding a dependency, or touching the StateBundle.
---

# Cavendish conventions

The authority is `cavendish-spec/cavendish.tex` (the *what*) and `DESIGN.md` (the *how*). This skill
is the quick reference; when they disagree with anything here, they win.

## Layering — dependencies point up only

Sixteen crates in six layers: L0 `math` `config`; L1 `gravity` `reference`; L2 `shape` `compute`
`source` `instrument` `uldm` `noise`; L3 `scenario` `state`; L4 `generate` `analysis`; L5 `sdk`
`viewer`. A crate's `[dependencies]` *are* its edges, and a lower crate never names a higher one. If
you reach for an upward dependency, the design is wrong — stop and reconsider, or raise it as a
doc-sync change. `state` never depends on the forward-model crates; `analysis` (not `state`) owns the
CRB because it needs the `Dual` forward model.

## The four seams

Everything swappable goes through a trait, and new capability lands as a new impl, not a change to the
core: `SourceDynamics` (mass configuration vs time), `PhaseModel` (`PropagationIntegral` reference /
`QuasiStaticGradient` fast path), `NoiseSource` (the post-hoc stack), `ComputeBackend` (CPU reference /
wgpu). Atmospheric GGN is the exception that proves the rule — it is a `FieldContribution` summed into
the potential in the forward pass, never a post-hoc `NoiseSource`.

## Scalar-generic kernels

The kernel is generic over a `Scalar` trait: `f64` is the CPU reference, `f32` is re-expressed in WGSL,
and forward-mode `Dual` carries derivatives for `analysis`. Anything on the differentiable path —
including the rotation integrator — must be `fn …<S: Scalar>(…)`. This is why no off-the-shelf physics
engine is allowed: they are neither differentiable nor `Scalar`-generic. Write the ~200 lines of Euler +
quaternion yourself.

## The data contract — dump everything

The engine emits the complete `StateBundle` (27 fields, torch-ready) and stops. It imposes **no**
input/label/target split, no task, no train/eval structure — that lives entirely downstream in the
consumer. Generating ML datasets at scale is a primary *intended use* the engine is *built for*
(batched, reproducible-from-a-seed, tensor-native), but it serves that use by dumping the full record.
`FieldSet` is a **cost/storage knob** over a dump-everything default — it trims what is computed for
compute/storage reasons, and is never a task definition. `Prior` is optional sugar for batched scenario
generation; the primary contract is `Scenario → StateBundle` by direct construction.

## Differential-first

Every quantity of interest is a difference or gradient of the potential; never form large absolute
potentials and subtract. This keeps the f32 GPU path above the precision floor and is a hard rule in
`gravity`, `instrument`, and the WGSL kernels.

## Style

British English throughout (realise, optimise, behaviour, colour, centre, artefact, …), in code
comments, docs, and identifiers. Terse and technical. Match the existing house style in the docs.
