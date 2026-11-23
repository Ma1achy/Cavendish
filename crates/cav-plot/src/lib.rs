//! Plots and figures.
//!
//! ## What this crate is for
//! The plots a pane can show in place of the scene (signal, periodogram, cadence window, flight
//! diagram), the fonts they are written in, and a painter with no window that turns them into
//! pictures. The application (`cav-app`) and the command line (`cav`) both draw through it, so
//! the command line does not link the application.
//!
//! ## What it may depend on
//! `cav-core`, `cav-sim`, `cav-engine`, `cav-gpu`, `cav-render`; `egui` and `egui-wgpu` (never
//! `eframe` or `winit`: nothing here opens a window); `serde_json`; `skrifa`, which egui already
//! compiles, for glyph outlines.

#![warn(missing_docs)]

pub mod drawing;
pub mod fonts;
pub mod headless;
pub mod paint;
pub mod plots;
pub mod style;
pub mod theme;
