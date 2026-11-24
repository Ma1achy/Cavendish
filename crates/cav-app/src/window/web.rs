//! The `window` seam in a browser page: there is no native window and no command line there (the
//! page is `cav-web`, `platform/web.rs`). The `cav-app` binary is not built for the web; if it is
//! run there all the same, it says so and fails.

use std::process::ExitCode;

/// Says that the native window is not part of the web build, and fails.
pub fn main() -> ExitCode {
    log::error!("cav-app is the native window; the web page is cav-web (`cargo xtask web build`)");
    ExitCode::FAILURE
}
