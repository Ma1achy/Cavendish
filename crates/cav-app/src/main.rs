//! `cav-app`: the Cavendish window, or (with `--screenshot`) the same application drawn to a PNG
//! with no window at all. The work is `cav_app::window` (natively; a page has no window).

fn main() -> std::process::ExitCode {
    cav_app::window::main()
}
