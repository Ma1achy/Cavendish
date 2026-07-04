//! `analysis` — the Fisher information and Cramér–Rao bound via the `Dual` forward model.
//!
//! Design: `design/state.md` §7 (the placement decision). Milestone: `milestones/M8-analysis-crb.md`.
//!
//! Identifiability from the differentiable forward model: the Jacobian `J = ∂signal/∂θ` assembled by
//! forward-mode `Dual` sweeps through the whole forward model (`compute::forward_dual`), the Fisher
//! information `F = JᵀΣ⁻¹J`, the Cramér–Rao floor `CRB = F⁻¹`, and array-geometry scoring. The CRB
//! lives here (not in `state`) because it needs the `Dual` forward model — `analysis` depends on the
//! forward crates; `state` never does. Degeneracy is **reported, not hidden**: a near-singular Fisher
//! surfaces as a conditioning flag rather than a confident-looking, meaningless covariance.

mod jacobian;
pub use jacobian::Jacobian;

#[cfg(test)]
mod tests {
    /// M8-R6 (`dep_hygiene`): `state` — the data-contract crate — must not depend on the forward-model
    /// crates; the CRB lives in `analysis` precisely because it needs the `Dual` forward model. Read
    /// `state`'s manifest and assert its dependency edges are `math`-only. (`analysis` compiling
    /// against its declared edge set is the other half, checked by the build itself.)
    #[test]
    fn dep_hygiene() {
        let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/../state/Cargo.toml");
        let toml = std::fs::read_to_string(manifest).expect("read state/Cargo.toml");
        let deps = toml
            .split_once("[dependencies]")
            .expect("state has a [dependencies] section")
            .1
            .split("\n[") // the section runs to the next table header (or EOF)
            .next()
            .unwrap();
        for forward in ["gravity", "source", "instrument", "compute", "analysis"] {
            assert!(
                !deps.contains(forward),
                "state must not depend on the forward-model crate `{forward}` — the CRB lives in \
                 analysis for exactly this reason (design/state.md §7)"
            );
        }
        assert!(
            deps.contains("math"),
            "state should still depend on math (the Scalar/Dual core)"
        );
    }
}
