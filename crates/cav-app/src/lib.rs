//! The Cavendish application: an egui shell around the engine.
//!
//! [`Shell`] is the application without a window: its state, its input handling and its drawing,
//! all against an `egui::Ui` and a [`PaneSurface`]. `main.rs` puts it in an eframe window;
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

pub mod editors;
pub mod headless;
pub mod menus;
mod panels;
pub mod panes;
pub use cav_plot::plots;
pub mod shell;
pub mod theme;
pub mod timeline;

pub use panes::{GpuPanes, NoSurface, PaneSurface};
pub use shell::{APP_COMMANDS, ConsoleFilter, Layout, Shell};
