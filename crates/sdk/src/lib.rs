//! `sdk` — the Python/torch boundary (PyO3 + DLPack). Marshals a `StateBundle` into torch tensors.
//!
//! Design: `design/sdk.md`. Milestone: `milestones/M7-sdk-python-torch.md`.
//!
//! The PyO3 layer is gated behind the `extension-module` feature so the blocking `cargo test`
//! gate stays libpython-free; maturin (and `clippy --all-features`) enable it. `run` is CPU-only,
//! so every tensor is host-resident: contiguous bundle fields hand off copy-free; the nested
//! `Vec<Vec<…>>` fields are flattened once (row-major, byte-exact) into an SDK-owned buffer that
//! torch then shares via DLPack. Ownership transfers into the capsule, so a tensor safely outlives
//! its bundle.

#[cfg(test)]
mod names_generate {
    #[allow(unused_imports)]
    use generate::{ComputeBackend, FieldContribution, NoiseSource, PhaseModel, SourceDynamics};
}

#[cfg(feature = "extension-module")]
mod py;
