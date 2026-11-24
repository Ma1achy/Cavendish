//! `cav-web`: the application as a web page. Built for `wasm32-unknown-unknown` by
//! `cargo xtask web` (which also runs wasm-bindgen and wasm-opt and writes `target/web/dist`);
//! the page is `crates/cav-app/web/index.html`. On any other target it only says so
//! (`cav_app::platform`, the seam, chooses).

fn main() {
    cav_app::platform::start();
}
