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

/// The files dropped on the window this frame, each as its name and (in a page) its bytes:
/// natively each is its path, and its bytes are read where it is imported.
#[must_use]
pub fn dropped(ctx: &egui::Context) -> Vec<(String, Option<Vec<u8>>)> {
    ctx.input(|i| {
        i.raw.dropped_files.iter().map(|f| (f.path().to_string_lossy().into_owned(), None)).collect()
    })
}
