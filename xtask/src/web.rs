//! `cargo xtask web`: the application as a web page (`docs/slices/S17.md`).
//!
//! `cargo xtask web build` builds `target/web/dist/` (what GitHub Pages serves): the `cav-web`
//! binary for `wasm32-unknown-unknown` in the `web` profile, wasm-bindgen's glue, wasm-opt, and
//! the page with its revision stamped. The Pages workflow runs exactly this.
//!
//! `cargo xtask web` is the tier: the web code's lints (native clippy does not compile it), the
//! build, and the engine's numbers in wasm held to the native ones (`web_equivalence`).
//!
//! The tools are pinned: wasm-bindgen must be the version `Cargo.lock` has (the glue and the crate
//! must agree), wasm-opt is binaryen's `version_133`. Neither is installed by this command.

use std::path::Path;
use std::process::Command;

use crate::{cargo, step};

/// Where the page is written.
pub const DIST: &str = "target/web/dist";
/// The binaryen release wasm-opt must come from.
const BINARYEN: &str = "version_133";
/// The wasm features rustc 1.100 turns on for this target by default, which every browser with
/// WebGPU has; wasm-opt is told so, and keeps to them.
const FEATURES: [&str; 6] = [
    "--enable-bulk-memory",
    "--enable-sign-ext",
    "--enable-reference-types",
    "--enable-multivalue",
    "--enable-mutable-globals",
    "--enable-nontrapping-float-to-int",
];
const TARGET: &str = "wasm32-unknown-unknown";

/// The tier: lints, the build, the equivalence of the numbers.
pub fn tier(root: &Path) -> Result<(), String> {
    step(
        "clippy (wasm32)",
        cargo(
            root,
            &[
                "clippy", "--target", TARGET, "-p", "cav-app", "--lib", "--bin", "cav-web", "--quiet", "--",
                "-D", "warnings",
            ],
        ),
    )?;
    build(root)?;
    equivalence(root)
}

/// Builds `target/web/dist`.
pub fn build(root: &Path) -> Result<(), String> {
    let bindgen = wasm_bindgen_version(root)?;
    check_tool(
        "wasm-bindgen",
        &["--version"],
        &format!("wasm-bindgen {bindgen}"),
        &format!(
            "cargo install wasm-bindgen-cli --version {bindgen} --locked (or the release's prebuilt binary)"
        ),
    )?;
    check_tool(
        "wasm-opt",
        &["--version"],
        &format!("({BINARYEN})"),
        &format!("binaryen {BINARYEN} from https://github.com/WebAssembly/binaryen/releases/tag/{BINARYEN}"),
    )?;
    step(
        "build (wasm32, profile web)",
        cargo(root, &["build", "--profile", "web", "--target", TARGET, "-p", "cav-app", "--bin", "cav-web"]),
    )?;
    let dist = root.join(DIST);
    if dist.exists() {
        std::fs::remove_dir_all(&dist).map_err(|e| format!("{}: not emptied: {e}", dist.display()))?;
    }
    let built = root.join("target").join(TARGET).join("web").join("cav-web.wasm");
    let mut bindgen = Command::new("wasm-bindgen");
    bindgen.current_dir(root).args(["--target", "web", "--no-typescript", "--out-dir", DIST, "--out-name"]);
    bindgen.arg("cavendish").arg(&built);
    step("wasm-bindgen", bindgen)?;
    let module = dist.join("cavendish_bg.wasm");
    let before = size(&module)?;
    let mut opt = Command::new("wasm-opt");
    opt.args(["-O2", "-o"]).arg(&module).arg(&module).args(FEATURES);
    step("wasm-opt -O2", opt)?;
    let page = std::fs::read_to_string(root.join("crates/cav-app/web/index.html"))
        .map_err(|e| format!("crates/cav-app/web/index.html: {e}"))?;
    let page = page.replace("{{revision}}", &revision(root));
    std::fs::write(dist.join("index.html"), page).map_err(|e| format!("{DIST}/index.html: {e}"))?;
    eprintln!(
        "xtask: web: {DIST}: cav-web.wasm {} bytes from cargo, {before} after wasm-bindgen, {} after wasm-opt; \
         cavendish.js {} bytes",
        size(&built)?,
        size(&module)?,
        size(&dist.join("cavendish.js"))?
    );
    Ok(())
}

/// The engine's numbers in wasm, under wasm-bindgen's runner in Node.js, against the native ones.
fn equivalence(root: &Path) -> Result<(), String> {
    let numbers = root.join("target/web/equivalence-native.txt");
    let numbers = numbers.to_string_lossy().replace('\\', "/");
    let mut native =
        cargo(root, &["test", "-p", "cav-engine", "--test", "web_equivalence", "--", "--ignored"]);
    native.env("CAV_WEB_EQUIVALENCE", &numbers);
    step("equivalence: native numbers", native)?;
    check_tool("wasm-bindgen-test-runner", &["--version"], "", "it comes with wasm-bindgen-cli")?;
    let mut wasm = cargo(
        root,
        &["test", "--target", TARGET, "-p", "cav-engine", "--test", "web_equivalence", "--", "--nocapture"],
    );
    wasm.env("CAV_WEB_EQUIVALENCE", &numbers)
        .env("CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER", "wasm-bindgen-test-runner");
    step("equivalence: wasm against native (Node.js)", wasm)
}

/// The wasm-bindgen version `Cargo.lock` holds.
fn wasm_bindgen_version(root: &Path) -> Result<String, String> {
    let lock = std::fs::read_to_string(root.join("Cargo.lock")).map_err(|e| format!("Cargo.lock: {e}"))?;
    lock_version(&lock, "wasm-bindgen").ok_or_else(|| "Cargo.lock has no wasm-bindgen".into())
}

/// The version of package `name` in a lock file's text.
fn lock_version(lock: &str, name: &str) -> Option<String> {
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line == format!("name = \"{name}\"") {
            let version = lines.next()?.strip_prefix("version = \"")?.strip_suffix('"')?;
            return Some(version.to_owned());
        }
    }
    None
}

/// Runs `tool` with `args` and fails, saying how to get it, unless its output contains `wanted`.
fn check_tool(tool: &str, args: &[&str], wanted: &str, install: &str) -> Result<(), String> {
    let output =
        Command::new(tool).args(args).output().map_err(|e| format!("{tool}: not found ({e}); {install}"))?;
    let said = String::from_utf8_lossy(&output.stdout);
    if said.contains(wanted) {
        Ok(())
    } else {
        Err(format!("{tool} says `{}`; this build needs `{wanted}`: {install}", said.trim()))
    }
}

fn size(path: &Path) -> Result<u64, String> {
    std::fs::metadata(path).map(|m| m.len()).map_err(|e| format!("{}: {e}", path.display()))
}

/// The commit the page is built from, as git names it (`GITHUB_SHA` in the workflow is the same).
fn revision(root: &Path) -> String {
    Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map_or_else(
            || "unknown".to_owned(),
            |output| String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_names_wasm_bindgens_version() {
        let lock = "[[package]]\nname = \"wasm-bindgen-futures\"\nversion = \"0.4.80\"\n\n\
                    [[package]]\nname = \"wasm-bindgen\"\nversion = \"0.2.130\"\n";
        assert_eq!(lock_version(lock, "wasm-bindgen").as_deref(), Some("0.2.130"));
        // Control: a name that is only a prefix of another is not found in it.
        assert_eq!(lock_version(lock, "wasm"), None);
        // The real lock has one, and the version the workflow and the tier pin.
        let real = std::fs::read_to_string(crate::root().join("Cargo.lock")).unwrap();
        assert!(lock_version(&real, "wasm-bindgen").is_some());
    }
}
