//! Cavendish core: units, maths, the instrument's geometry and the forward model.
//!
//! `f64` vectors and matrices for cameras and frames, the default array and room, the physical
//! constants with their sources, and (from S1) the forward model: flight, arms, the propagation
//! integral and the channels. The voxel tree and the kernels arrive with their slices.

pub mod anchors;
pub mod constants;
pub mod defaults;
pub mod forward;
pub mod maths;

pub use maths::{Mat4, Vec3};
