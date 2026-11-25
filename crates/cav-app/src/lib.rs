//! The Cavendish application: an egui shell around the engine.
//!
//! [`Shell`] is the application without a window: its state, its input handling and its drawing,
//! all against an `egui::Ui` and a [`PaneSurface`]. [`app::App`] is it inside eframe: [`window`]
//! puts that in a native window, and [`platform`] (on `wasm32`) on a browser page's canvas;
//! [`headless::Headless`] draws it into a texture for screenshots and tests.
//!
//! ## What this crate is for
//! The window: the menus, the outliner, the properties and their editors (made from the scene
//! spec's schema), the timeline with a lane per tower, the panes and the console. Lengths on
//! screen are egui points; times are seconds on the scene's clock.
//!
//! ## What it may depend on
//! Every other library crate of the workspace (`cav-core`, `cav-sim`, `cav-engine`, `cav-gpu`,
//! `cav-render`, `cav-plot`), and nothing may depend on it: the command line and the MCP server do
//! not link the window. It must hold no scene state of its own: whatever a panel does is a
//! [`cav_engine::Command`], and whatever it shows is a reply.

#![warn(missing_docs)]

pub mod app;
pub mod editors;
pub mod headless;
pub mod lane;
pub mod menus;
mod panels;
pub use panels::MAX_OUTLINE_ROWS;
/// The outliner's words, for its tests: what a tower's row says, and whether a scene reads
/// nothing. Not an interface.
#[doc(hidden)]
pub mod panels_for_tests {
    pub use crate::panels::{silent, tower_detail};
}
pub mod panes;
pub use cav_plot::plots;
pub mod shell;
pub mod theme;
pub mod timeline;

pub mod address;
// The platform seam: the application's entry and what it does around each frame. Natively next
// to nothing (the window is `main.rs`); in a page, eframe's web runner, the address's `#exec`,
// downloads, the forward stage's workers.
#[cfg(not(target_arch = "wasm32"))]
#[path = "platform/native.rs"]
pub mod platform;
#[cfg(target_arch = "wasm32")]
#[path = "platform/web.rs"]
pub mod platform;
// The native window and its command line (the `cav-app` binary); a page has neither.
#[cfg(not(target_arch = "wasm32"))]
#[path = "window/native.rs"]
pub mod window;
#[cfg(target_arch = "wasm32")]
#[path = "window/web.rs"]
pub mod window;

pub use panes::{GpuPanes, NoSurface, PaneSurface};
pub use shell::{APP_COMMANDS, ConsoleFilter, Layout, Shell};
