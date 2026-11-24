//! `cav-web`: the application as a web page. Built for `wasm32-unknown-unknown` by
//! `cargo xtask web` (which also runs wasm-bindgen and wasm-opt and writes `target/web/dist`);
//! the page is `crates/cav-app/web/index.html`. On any other target it only says so.

#[cfg(target_arch = "wasm32")]
fn main() {
    cav_app::web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    eprintln!("cav-web is the browser build: `cargo xtask web` builds it into target/web/dist");
    std::process::ExitCode::FAILURE
}
