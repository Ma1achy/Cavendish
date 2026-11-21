//! The Cavendish renderer. S0 draws the array, the room and the infinite floor; the big-scene
//! renderer of REBUILD §5 grows from here.
//!
//! One entry point, [`Renderer::draw`], draws a [`Frame`] into a [`PaneTarget`]'s texture. The app
//! shows that texture in a pane; `cav render` and MCP `render_view` read it back and encode a PNG
//! ([`Renderer::render_png`]). Nothing here knows about windows or egui.

pub mod camera;
pub mod renderer;
pub mod scene;

pub use camera::{Camera, PaneRect, pane_rects};
pub use renderer::{COLOUR_FORMAT, Frame, PaneTarget, Renderer, decode_png, encode_png};
pub use scene::{BACKGROUND_SRGB, Drawables, PAPER_SRGB, Part, drawables, tower_srgb};
