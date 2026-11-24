# Keeping a change working in the browser

**Example to read:** where the forward stage's work runs, `crates/cav-engine/src/forward/threads.rs`
beside `crates/cav-engine/src/forward/inline.rs`, and how `crates/cav-engine/src/forward.rs`
chooses one.

The application is also a web page (`docs/slices/S17.md`): `cav-app`'s `cav-web` for
`wasm32-unknown-unknown`, with WebGPU and one thread. Most code needs nothing for it. A change
needs care when it does something a browser page cannot:

| A page cannot | So | Where it is handled |
|---|---|---|
| start a thread (`std::thread::spawn` panics) | work that must not block runs from `Forward::poll` in slices | `crates/cav-engine/src/forward/inline.rs` |
| read the clock with `std::time::Instant` (it panics) | use `web_time::Instant`: std's natively, `performance.now()` in a page | every crate that times anything |
| write or read a file by its path | a file the person asked for goes through `cav_engine::files` (`write()`, `create()`): natively a file, in a page a download; a folder is refused (`FOLDERS`) | `crates/cav-engine/src/files.rs` |
| wait for the GPU (`pollster`, mapping a buffer, an error scope) | the device is eframe's (`Gpu::from_parts`); read-back is refused with the reason | `crates/cav-gpu/src/web.rs` |

## If your change needs a new seam

1. **One module a platform**, both with the same items, chosen where the module is declared:
   `#[cfg(not(target_arch = "wasm32"))] #[path = "x/native.rs"] mod platform;` and the same with
   `#[cfg(target_arch = "wasm32")]` and `x/web.rs` (as `crates/cav-engine/src/files.rs` does).
   Callers use the items and never write a `cfg` of their own. What the web side cannot do it
   refuses with a sentence that says why and where to do it instead.
2. **A row** in the table of seams in `docs/ARCHITECTURE.md`.
3. **The dependency for the web only**, if one is needed, under
   `[target.'cfg(target_arch = "wasm32")'.dependencies]`, at the version `Cargo.lock` already has.
4. **Its bound.** A page has the same caps as natively (`MAX_DOWNLOAD_BYTES` for downloads); a
   new store gets its cap in both modules.

## What then holds it

- `cargo xtask web`: clippy for wasm32 (native clippy never compiles the web modules), the build,
  and the engine's numbers in wasm held to native (`wasm_equals_native`, with its control
  `the_comparison_finds_a_changed_bit`). Run it before merging any change to the engine, the
  read, the seams or the app's entry.
- `cargo xtask demo S17`: the page in headless Chrome with WebGPU; it fails on a console error or
  a panic.
- After a merge, `.github/workflows/pages.yml` builds and publishes the page; a failed build
  leaves the last good page up.
