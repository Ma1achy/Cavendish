//! The Cavendish application: an egui shell around the engine.
//!
//! [`Shell`] is the application without a window: its state, its input handling and its drawing,
//! all against an `egui::Ui` and a [`PaneSurface`]. `main.rs` puts it in an eframe window;
//! [`headless::Headless`] draws it into a texture for screenshots and tests.

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
