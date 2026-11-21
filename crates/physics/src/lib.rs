//! `physics` — the scene simulator: it produces mass configurations; the forward model reads them.
//!
//! Design: `design/physics.md`. Milestone: `milestones/M12-physics.md`.

pub mod fluid;
pub mod grid;
pub mod kernel;

pub use fluid::{Air, Fluid, FluidParams, Heat};
pub use grid::{flight_nodes, Grid3, GridField};
