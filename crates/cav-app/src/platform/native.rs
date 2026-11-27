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

/// Captures and hides the pointer for the first-person camera (`on`), or gives it back. The
/// window is asked to lock the pointer where it is; where it cannot (`Locked` is not on every
/// platform) the request before it, to confine the pointer to the window, stands. Raw motion
/// ([`motion`]) arrives either way.
pub fn capture(ctx: &egui::Context, on: bool) {
    use egui::{CursorGrab, ViewportCommand};
    if on {
        ctx.send_viewport_cmd(ViewportCommand::CursorGrab(CursorGrab::Confined));
        ctx.send_viewport_cmd(ViewportCommand::CursorGrab(CursorGrab::Locked));
        ctx.send_viewport_cmd(ViewportCommand::CursorVisible(false));
    } else {
        ctx.send_viewport_cmd(ViewportCommand::CursorGrab(CursorGrab::None));
        ctx.send_viewport_cmd(ViewportCommand::CursorVisible(true));
    }
}

/// The mouse's raw motion this frame, counts to the right and down, as the device reports it
/// (not the pointer's, which stops at the window's edge and does not move while captured).
/// `None` until the window has reported any: the pointer's own motion then stands in.
#[must_use]
pub fn motion(ctx: &egui::Context) -> Option<[f64; 2]> {
    ctx.input(|i| i.pointer.motion()).map(|m| [f64::from(m.x), f64::from(m.y)])
}

/// Whether the platform took a captured pointer back by itself: natively never (a window that
/// loses its focus ends the flight by that).
#[must_use]
pub const fn capture_lost() -> bool {
    false
}

/// The files dropped on the window this frame, each as its name and (in a page) its bytes:
/// natively each is its path, and its bytes are read where it is imported.
#[must_use]
pub fn dropped(ctx: &egui::Context) -> Vec<(String, Option<Vec<u8>>)> {
    ctx.input(|i| {
        i.raw.dropped_files.iter().map(|f| (f.path().to_string_lossy().into_owned(), None)).collect()
    })
}
