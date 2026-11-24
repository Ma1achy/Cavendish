//! The application around its frames on a native process: the platform seam of `cav-app`
//! (`platform/web.rs` is the page's). Natively there is nothing to do around a frame (files are
//! on disc when they are written), and the browser build's entry only says where it is built.

use crate::Shell;

/// Natively: says that `cav-web` is the browser build and how to build it, and ends the process
/// unsuccessfully.
pub fn start() {
    eprintln!("cav-web is the browser build: `cargo xtask web build` builds it into target/web/dist");
    std::process::exit(1);
}

/// Natively nothing: what a page needs after each frame.
pub const fn after_frame(_shell: &mut Shell, _ctx: &egui::Context) {}
