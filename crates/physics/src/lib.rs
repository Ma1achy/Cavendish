//! `physics` — the scene simulator: it produces mass configurations; the forward model reads them.
//!
//! Design: `design/physics.md`. Milestone: `milestones/M12-physics.md`.

pub mod body;
pub mod fluid;
pub mod grid;
pub mod kernel;
pub mod rigid;
pub mod scene;

pub use body::{Bone, Parcels, Plate, Pose, Skeleton, Walker};
pub use fluid::{Air, Fluid, FluidParams, Heat};
pub use grid::{flight_nodes, Grid3, GridField};
pub use rigid::{Collider, Plane, RecordedRigid, RigidBody, RigidWorld};
pub use scene::{simulate_air, Obstacle, Room};
