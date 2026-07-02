# Cavendish — CPU/GPU compute split (working note)

Scratch consolidation of the device split, derived from where the data
dependencies actually fall. Destined for the compute section of `cavendish.tex`,
replacing the "a compute backend slots in later" placeholder.

## Principle

The cut is not chosen, it is read off the dependency structure: **parallel
arithmetic and reductions go on the GPU; sequential control flow, one-time setup,
and I/O stay on the CPU.** Everything below is a consequence of that one rule plus
one wrinkle (the ODE trajectory) where the naive reading is wrong.

## The pipeline, per dataset

**Setup — once per body template (at load).** Voxelise the mesh into a point-mass
cloud, N ~ 10⁵–10⁷; optionally reduce the cloud to a handful of multipole moments.

**Per scenario.** Sample from the prior (which body, trajectory parameters,
closest-approach distance, speed, noise level); set up the RNG stream.

**Per scenario × measurement ℓ (cadence Δt) × IFO (2) × arm (2) × fine_dt step
(~146 across the 2T = 1.46 s flight).** Place the atom on its closed-form
parabola; place the source at that sub-time; evaluate V = −G Σᵢ mᵢ/|r − rᵢ| over
the cloud; accumulate the quadrature. Then assemble δφ → per-IFO phase →
gradiometer double-difference ΔΦ_ℓ; add noise; serialise.

## Dependency analysis

- **Independent axes** (embarrassingly parallel): scenarios, measurements, IFOs,
  arms, and the fine_dt integrand samples are all mutually independent.
- **Two reductions**: the sum over N voxels to get V at a point, and the
  quadrature sum over fine_dt to get an arm's phase. Reductions are exactly what
  a GPU does well.
- **One genuinely sequential dependency** in the entire model: integrating an ODE
  trajectory in time (PendulumODE, θ̈ = −(g/L)sinθ), where step t needs step t−1.
  Everything else is parallel or a reduction.

So the naive cut writes itself — parallel arithmetic on GPU, the one sequential
thing plus setup/IO on CPU — and it is *almost* right. The interesting part is
the single place it isn't (the ODE; see below).

## GPU — the field-evaluation outer product

This is the bulk of the FLOPs and the entire reason for going GPU.

- Source-pose evaluation from trajectory parameters (closed-form timings inline).
- Atom arm parabolas (closed-form, inline).
- **The potential / gravity-tensor sum over the voxel cloud** at every field
  point — the near-field N-body reduction, the dominant cost. With voxel clouds
  at N ~ 10⁵–10⁷ this is what makes CPU hopeless and GPU necessary.
- Multipole far-field evaluation (O(1) per point from precomputed moments) — the
  cheap branch of the near/far cutoff.
- Flight-integral quadrature (parallel integrand evaluation + reduction).
- Differential-phase assembly and the gradiometer double-difference.
- Noise injection via **counter-based RNG (philox/threefry)**: noise[scenario, ℓ]
  is computed independently per index, so it is parallel, not a sequential draw.

## CPU — setup, control flow, I/O

- **Voxelisation and multipole reduction**, once per template at load. The
  reduction is structurally parallel but amortised over millions of scenarios →
  effectively free → no reason to ship it to the GPU. (This is the *only* compute
  that is ever legitimately "on CPU".)
- **Scenario sampling** from the prior — discrete and branchy, cheap.
- **Orchestration**: batching, kernel launch, data movement.
- **Serialisation** (offline shards) or the tensor handoff (fused-in-training).

## The one case the naive cut gets wrong: the ODE trajectory

The ODE timing is sequential *in time*, which says "CPU" — but it is independent
*across scenarios*, and the poses it produces are an intermediate consumed
immediately by the field evaluation. The deciding factor is not parallelism, it
is **locality**.

If the CPU integrates the ODE and ships poses to the GPU, you pay a fat upload:
the field eval needs source poses at fine_dt resolution, because the source moves
appreciably *during* the 1.46 s flight (that is the whole reason fine_dt exists),
so it is ~(measurements × 146) poses per scenario — gigabytes for a real batch.

Resolution: **integrate the ODE on the GPU**, one thread per scenario marching in
time, so the poses are produced exactly where they are consumed. Sequential in
time, parallel across the batch, kept on-device. The visualiser (one scenario,
real-time) can integrate on the CPU because there is no batch and no upload; the
data cannon must not.

## Load-bearing rule: upload parameters, not poses

The general rule that falls out of the ODE case, and governs the whole boundary:
the CPU hands the GPU tiny trajectory-parameter sets and RNG keys; the GPU
evaluates poses from them inline and never sends the fat intermediate back across
the bus.

**Boundary contract**

- **CPU → GPU**: the static body cloud, once per template; per-scenario
  trajectory parameters + RNG keys (tiny); instrument constants.
- **GPU → CPU**: the signal ΔΦ_ℓ and the per-measurement state for the training
  bundle.

## Hardware reality (M3 MacBook / RTX 2080 Super — no A100)

The 2080 Super has 8 GB. A 10⁷-voxel body (~160 MB at f32 × 4 fields) fits, but
you cannot hold many bodies resident, and a large batch's output must be streamed
rather than held. This is an **independent** argument for the multipole path when
N or the batch is large (moments are kilobytes), for keeping one body resident at
a time, and for streaming batches through. Consequence: the near/far cutoff is
partly a memory-placement decision, not only an accuracy one.

## Summary

GPU owns everything inside the per-(scenario × timestep × field-point) loop,
including both reductions. CPU owns the one-time setup, the branchy sampling, the
orchestration, and the I/O. The lone sequential axis (ODE-in-time) stays on the
GPU in the batch path for locality, dropping to the CPU only in the
single-scenario visualiser. The governing constraint across the bus is **upload
parameters, not poses**.
