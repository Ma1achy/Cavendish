//! `viewer` — the egui + wgpu inspector: eyes on the forward model. It runs the engine (or loads a
//! serialised bundle) and draws it — posed clouds, the array, the signal, the spectrum, a scrubber.
//!
//! Design: `design/viewer.md`. Milestone: `milestones/M9-viewer.md`.
//!
//! Two spines, both testable headlessly (the eye is not the check): **coherence** — the 3D pose and the
//! 2D cursor read one scrub index `ℓ` ([`scrub`]) and the drawn geometry equals the bundle ([`pose`]);
//! and **failing soft** — a run panic becomes a toast, a degenerate scene an empty one, never a crash.
//! Everything asserted lives in these pure modules or the borrowed-device [`render`]er, below the App.

pub mod app;
pub mod camera;
pub mod data;
pub mod editor;
pub mod field_slice;
pub mod gizmo;
pub mod panels;
pub mod params;
pub mod pose;
pub mod presets;
pub mod render;
pub mod scene;
pub mod scrub;
