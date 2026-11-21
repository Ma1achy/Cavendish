//! Cavendish core: units, maths and the instrument's geometry.
//!
//! S0 holds only what the shell needs: `f64` vectors and matrices for cameras and frames, and the
//! default array and room. The voxel tree, the kernels and the forward model arrive with their slices.

pub mod defaults;
pub mod maths;

pub use maths::{Mat4, Vec3};
