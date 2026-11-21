//! `cargo xtask demo <slice>`: the slice's demo, run headlessly, with screenshots in `docs/slices/<slice>/`.

use std::path::Path;

pub fn run(_root: &Path, slice: &str) -> Result<(), String> {
    match slice {
        "S0" | "s0" => {
            Err("S0's demo is not built yet: it arrives with TASK-S0-09 (docs/slices/S0.md)".into())
        }
        other => Err(format!("no demo for slice `{other}`: the slices with demos are: none yet")),
    }
}
