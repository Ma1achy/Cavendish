//! The Cavendish renderer: the array, the room, the infinite floor and the bodies of a scene,
//! drawn in three dimensions. The big-scene renderer of REBUILD §5 grows from here.
//!
//! One entry point, [`Renderer::draw`], draws a [`Frame`] into a [`PaneTarget`]'s texture. The app
//! shows that texture in a pane; `cav render` and MCP `render_view` read it back and encode a PNG
//! ([`Renderer::render_png`]). Nothing here knows about windows or egui.
//!
//! ## What this crate is for
//! Turning a scene spec and a view into pixels in a texture, with or without a window: the
//! camera, the drawables of a scene (towers, room, floor, bodies, marks) and the renderer that
//! draws them and encodes PNGs.
//!
//! ## What it may depend on
//! Of the workspace, `cav-core`, `cav-sim` and `cav-gpu` (`cav-validate` in its tests).
//! It must not depend on the engine, and must not hold anything that knows about windows, egui
//! or plots: the app and `cav-plot` sit above it.

#![warn(missing_docs)]

pub mod camera;
pub mod colour;
pub mod renderer;
pub mod scene;
pub mod world;

pub use camera::{Camera, PaneRect, pane_rects};
pub use renderer::{COLOUR_FORMAT, Frame, PaneTarget, Renderer, decode_png, encode_png};
pub use scene::{
    BACKGROUND_SRGB, BODY_RADIUS, BODY_SRGB, Drawables, MAX_MARKS, PAPER_SRGB, Part, decimation, drawables,
    drawables_at, mark_step, tower_srgb,
};
