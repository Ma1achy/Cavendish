//! Fixes the engine's revision and dirty flag when it is compiled (`cav_engine::build`): git is
//! asked here, once, and never when a bundle or a figure is written. Built where there is no git
//! or no repository, both are `unknown` and the build goes on.

include!("provenance.rs");

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let root = Path::new(&manifest).join("../..");
    let made = stamp("git", &root);
    println!("cargo:rustc-env=CAVENDISH_REVISION={}", made.revision);
    println!("cargo:rustc-env=CAVENDISH_DIRTY={}", made.dirty);
    for path in &made.watch {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
