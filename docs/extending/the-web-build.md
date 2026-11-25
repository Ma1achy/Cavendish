# Keeping a change working in the browser

**Example to read:** where the forward stage's work runs, `crates/cav-engine/src/forward/threads.rs`
beside `crates/cav-engine/src/forward/inline.rs` (and the bytes between the page and its workers,
`forward/wire.rs`), and how `crates/cav-engine/src/forward.rs` chooses one.

The application is also a web page (`docs/slices/S17.md`): `cav-app`'s `cav-web` for
`wasm32-unknown-unknown`, with WebGPU, published at https://ma1achy.github.io/Cavendish/ by
`.github/workflows/pages.yml` on every push to `main`.

## Building and serving it locally

Once, besides Rust: the wasm target, wasm-bindgen's command line at the version in `Cargo.lock`
(0.2.130 today; the build checks it), binaryen's `wasm-opt` from release `version_133`, Node.js
(the equivalence test and the headless pictures) and Chrome or Edge.

```
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130 --locked   # or the release's prebuilt binary
# wasm-opt: binaryen-version_133-<platform>.tar.gz from github.com/WebAssembly/binaryen/releases, on PATH
```

Then:

```
cargo xtask web build                     # target/web/dist (about 2 min, LTO)
cargo xtask web serve --exit-after 600    # http://127.0.0.1:8000/ for ten minutes
cargo xtask web                           # the tier: clippy for wasm32, the build, wasm numbers held to native
cargo xtask demo S17                      # the page in headless Chrome, pictures to docs/slices/S17/
```

`web serve` without `--exit-after` runs until it is stopped. **Stopping it with Ctrl-C can leave
`xtask.exe` running** (on Windows `cargo` passes the stop on only in a console that sends it; from
a script, an IDE's terminal or an agent, killing `cargo` leaves its child serving): scripts and
agents always give `--exit-after`. The server answers only files under `target/web/dist`, only on
`127.0.0.1`, a thread per connection (at most 16, 5 s timeouts). To time the page's frames, set
`CAV_FRAMES=50` for `node scripts/web_screenshot.mjs` (see its header).

## What a page cannot do, and where that is handled

Most code needs nothing for the web. A change needs care when it does something a page cannot:

| A page cannot | So | Where it is handled |
|---|---|---|
| start a thread (`std::thread::spawn` panics), or block its thread for long | the forward stage's work runs in two Web Workers (the same module), handed over as bytes; a new kind of long job goes there too, through `forward::wire` | `crates/cav-engine/src/forward/inline.rs`, `forward/wire.rs`, `crates/cav-app/src/platform/web.rs` |
| wait for an answer from a worker (it arrives from the event loop the waiting would block) | `Engine::settle` returns at once in a page; a client is told to try again; a link's figure waits in the `#exec` queue | `forward/inline.rs` (`receive`), `platform/web.rs` (`run_exec`) |
| read the clock with `std::time::Instant` (it panics) | use `web_time::Instant`: std's natively, `performance.now()` in a page | every crate that times anything |
| write or read a file by its path | a file the person asked for goes through `cav_engine::files` (`write`, `create`, `read`, `is_file`): natively a file, in a page a download; a folder is refused (`FOLDERS`). Test that a writer uses it with `files::capture` (natively the seam's double: every file kept, none on disc) | `crates/cav-engine/src/files.rs` |
| wait for the GPU (`pollster`, mapping a buffer, an error scope) | the device is eframe's (`Gpu::from_parts`); read-back is refused with the reason | `crates/cav-gpu/src/web.rs` |
| trust its address | `#exec` is capped (4 KiB, 32 commands); files a link makes wait behind a button; download names are made a plain basename with `.ron .json .svg .pdf .csv .npy` | `crates/cav-app/src/address.rs` |
| run inline script (the page's Content-Security-Policy) | the page's script is a file (`web/start.js`); a new file the page needs is copied in `xtask::web::build` | `crates/cav-app/web/` |

## If your change needs a new seam

1. **One module a platform**, both with the same items, chosen where the module is declared:
   `#[cfg(not(target_arch = "wasm32"))] #[path = "x/native.rs"] mod platform;` and the same with
   `#[cfg(target_arch = "wasm32")]` and `x/web.rs` (as `crates/cav-engine/src/files.rs` does).
   Callers use the items and never write a `cfg` of their own: `cargo xtask quick` fails on a
   `cfg(target_arch …)` anywhere but on a module's declaration
   (`xtask::web::seams::only_the_seams_name_the_target`). What the web side cannot do it refuses
   with a sentence that says why and where to do it instead.
2. **A row** in the table of seams in `docs/ARCHITECTURE.md`.
3. **The dependency for the web only**, if one is needed, under
   `[target.'cfg(target_arch = "wasm32")'.dependencies]`, at the version `Cargo.lock` already has.
4. **Its bound.** A page has the same caps as natively (`MAX_DOWNLOAD_BYTES` for downloads); a
   new store gets its cap in both modules.

## What then holds it

- `cargo xtask web`: clippy for wasm32 (every target of `cav-app`, and the equivalence test;
  native clippy never compiles the web modules), the build, and the engine's numbers in wasm in
  the shipped `web` profile held to native (`wasm_equals_native`: every number the same bits but
  the listed `THROUGH_LIBM` labels, with its control `the_comparison_finds_a_changed_bit`). Run
  it before merging any change to the engine, the read, the seams or the app's entry.
- `cargo xtask demo S17`: the page in headless Chrome with WebGPU; it fails on a console error, an
  error in the browser's log (a CSP violation is one), or a panic; it clicks a link's download
  button and holds the files to the native ones; it loads the page without WebGPU and without an
  adapter.
- After a merge, `.github/workflows/pages.yml` builds and publishes the page; a failed build
  leaves the last good page up.
