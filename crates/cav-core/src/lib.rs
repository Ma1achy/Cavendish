//! Cavendish core: units, maths, the instrument's geometry and the forward model.
//!
//! `f64` vectors and matrices for cameras and frames, the default array and room, the physical
//! constants with their sources, the forward model (flight, arms, the propagation integral, the
//! audited read and the channels), spectra (FFT, Lomb–Scargle, the non-uniform FFT), the closed
//! forms of a cadence's spectral window with keyed random numbers, and `.npy` files. The voxel
//! tree and the kernels arrive with their slices.
//!
//! ## What this crate is for
//! The numbers and the physics every other crate shares: `f64` vectors and matrices, the physical
//! constants with their sources, the default array and room, the forward model (flight, arms, the
//! propagation integral, the channels), spectra of unevenly sampled series, the cadence of a
//! schedule (keyed random numbers, spectral windows) and `.npy` files.
//!
//! ## What it may depend on
//! No other workspace crate, and no outside crate either (`cav-validate` in its tests only).
//! It must not hold anything that knows about a scene's file format, a GPU, a window or the
//! engine: serde, wgpu and egui stay out, so that the hot numerical crate compiles fast and can
//! be optimised on its own.

#![warn(missing_docs)]

pub mod anchors;
pub mod cadence;
pub mod constants;
pub mod defaults;
pub mod forward;
pub mod maths;
pub mod npy;
pub mod spectrum;
pub mod voxel;

pub use maths::{Mat4, Vec3};
