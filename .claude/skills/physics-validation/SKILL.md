---
name: physics-validation
description: The validation discipline for Cavendish's forward model — the invariants and their exact tolerances, the independent reference-port strategy (George's cases in Rust), George's pinned constants, and differential-first. Use when writing or debugging any physics, its tests, or the reference crate.
---

# Physics validation

## Anchors are checked against an independent reference, not figures

George Parish's validation cases are ported to the `reference` crate as an **independent** oracle: it
reproduces his *method* (direct quadrature potentials, his two-segment arm construction, his FFT/LS),
which is deliberately different from Cavendish's voxelised, differential-first, Nagy-prism path. Each
anchor asserts `cavendish ≈ reference` — two independent implementations agreeing — rather than
`cavendish ≈ 50 µrad`. Agreement tolerances are the looser 2–5% dominated by the voxel term (shrinking
with lattice pitch `h`); the internal invariants below stay tight. See `milestones/reference-port.md`
and the upstream repo `Thranduil02/atom-interferometry`.

## The invariants (with tolerances)

- **Trace-free, symmetric gradient tensor** in vacuum: `Γ = Γᵀ`, `tr Γ = 0` — ≤1e-12.
- **Mass-linearity**: `ΔΦ(αm) = α·ΔΦ(m)` — ≤1e-12. (A mass draw rescales a cached unit-mass cloud; it
  never re-voxelises.)
- **Exact cloud post-conditions**: `Σmᵢ = M` (renormalised, zero monopole error) and CoM at the origin
  (zero dipole) — exact.
- **Shell theorem**: a voxelised sphere's external field matches a point mass — ≤1e-3.
- **Decomposition superposition**: `signal_targets + signal_atmospheric + signal_uldm + signal_noise =
  signal` — ≤1e-10 (f64), by linearity of the phase in the potential.
- **CPU ≡ GPU**: the WGSL f32 path matches the f64 CPU reference on the anchors — ≤1e-4.
- **Rotation**: quaternion norm preserved (≤1e-12); energy and |L| bounded with no secular drift; a
  CoM-fixed rotation is a pure-quadrupole signal (monopole constant, dipole ≡ 0) — ≤1e-3.
- **Reproducibility**: `(Prior, seed)` replays identical tensors on any machine, order-independent
  (counter-based RNG, not sequential state).

## George's pinned constants (adopt verbatim in `reference`)

`G = 6.67430e-11`, `m_atom = 1.46e-25` (⁸⁷Sr — the spec is correct; George's `# Sr-88` comment is
incidental and the physics depends only on the mass), `hbar = 1.055e-34`, `λ = 698e-9`,
`n = 1000`, `T = 0.73` (2T = 1.46), `g = 9.81`, `u_initial = 3.86`, `Δr = 5`, cadence 2 s, `fine_dt = 0.01`.
George forms `ΔΦ = δφ₁ − δφ₂`; the spec uses `δφ₂ − δφ₁`, so agreement tests align the global sign or
compare magnitudes.

## Differential-first

Compute differences/gradients of the potential; never form large absolute potentials and subtract. This
is what keeps the f32 GPU path above the floor — it is a correctness rule, not an optimisation.
