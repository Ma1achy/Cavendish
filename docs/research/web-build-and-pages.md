# Research: Cavendish in the browser, published on GitHub Pages

24 Nov 2026. For the builder who does the web build next. Read from the repository (Cargo.lock, crate sources,
`progress.yml`), the vendored sources of eframe 0.37.0, egui-wgpu 0.37.0 and wgpu 30.0.2 in the cargo registry,
and the linked pages. Versions of actions and tools were read from the GitHub API on this date. **Nothing was
compiled for wasm**: the `wasm32-unknown-unknown` target is not installed on the build PC and installing it is a
change outside the repository, so every statement about what will build or run is read from sources, not
measured. §9 says what could not be verified and how to settle each.

## Summary

- **Feasible, and mostly small.** egui/eframe 0.37 and wgpu 30 both target the web, and the repository already
  locks `wasm-bindgen 0.2.130`, `web-sys`, `web-time` (Cargo.lock). The renderer uses only a uniform buffer,
  vertex buffers and a 32-bit depth texture (`crates/cav-render/src/renderer.rs`, `shaders.wgsl`): nothing that
  needs compute. The GPU is used for drawing only today.
- **Toolchain: no trunk; three pinned binaries run from one script.** `cargo build --target wasm32-unknown-unknown`,
  `wasm-bindgen-cli =0.2.130` (must equal the lock's version) and `wasm-opt` (binaryen version_133), plus a
  hand-written `index.html` of about 25 lines. Trunk 0.22.2 is the credible alternative (§1).
- **Backend: WebGPU only in the first version.** Add wgpu's `webgpu` feature for wasm; leave `webgl` out and show
  a plain message when `navigator.gpu` is missing. WebGL2 would draw the current scene but has no compute
  shaders and no storage buffers, which S5's GPU kernels need (§2).
- **Threads: single-threaded on the web.** No SharedArrayBuffer, no nightly `build-std`, no service worker. This
  is forced as much as chosen: `std::thread::spawn` panics on this target, and `Forward::new` calls it and
  `expect`s. The engine's forward stage gets a second implementation on wasm that runs its jobs from `poll()` in
  frame-sized slices, so the *API* still never blocks; a very large scene will make frames stutter (§3).
- **Almost everything compiles unchanged on wasm, and fails at run time.** std is present on this target but
  `thread::spawn` panics and `std::fs` errors ([platform support](https://doc.rust-lang.org/nightly/rustc/platform-support/wasm32-unknown-unknown.html)).
  `Instant::now()` panics too, as far as I know (§4). So `cargo check --target wasm32-unknown-unknown` catches
  little; it needs a browser smoke test beside it.
- **Hosting:** Pages source = GitHub Actions; one new workflow `pages.yml` builds, uploads
  `actions/upload-pages-artifact`, deploys `actions/deploy-pages`. It commits nothing to `main` and is independent
  of `progress.yml`. The repository is public, so Pages costs nothing. **The owner must set the Pages source once**
  (§5.1).
- **First version (two tasks):** the app opens a library scene in a canvas, with panes, plots and the console;
  files in and out are left for later (§6).

## 1. Toolchain

**What eframe offers.** `eframe::WebRunner::start(canvas, WebOptions, app_creator)` is an `async fn` that makes the
wgpu instance, adapter and device itself, installs the event handlers and runs the app
(`eframe-0.37.0/src/web/web_runner.rs`, [docs](https://docs.rs/eframe/0.37.0/eframe/struct.WebRunner.html)).
eframe re-exports `wasm_bindgen` and `web_sys` (`eframe-0.37.0/src/lib.rs`, lines 175 and 178), so the web entry
needs no new direct dependency except `wasm-bindgen-futures` (for `spawn_local`; already in Cargo.lock, 0.4.80).
The [eframe README](https://github.com/emilk/egui/blob/main/crates/eframe/README.md) says it "compiles to Wasm
using either WebGPU (when available) or WebGL2".

**The template** ([eframe_template](https://github.com/emilk/eframe_template)) uses trunk
(`cargo install --locked trunk`, `trunk serve`, `trunk build --release`), and its
[workflow](https://raw.githubusercontent.com/emilk/eframe_template/main/.github/workflows/pages.yml) pushes `dist`
to a `gh-pages` branch with `JamesIves/github-pages-deploy-action`, with trunk and Rust unpinned. Do not copy that:
pushing to a branch is the older Pages route, and unpinned tools break a build for no change of ours. Its
`assets/sw.js` caches the app for offline use; leave that out, since a stale cache is the usual way a deployed
page looks as if it did not update.

**Trunk or by hand.**

| | trunk 0.22.2 ([release](https://github.com/trunk-rs/trunk/releases/tag/v0.22.2)) | cargo + wasm-bindgen-cli + wasm-opt |
|---|---|---|
| Needs | one binary, `Trunk.toml`, `index.html` with `<link data-trunk rel="rust">` | three binaries and a 25-line `index.html` |
| Rust asset attributes | `data-wasm-opt` (`0`–`4`, `s`, `z`, "only used in `--release`"), `data-bin`, `data-cargo-features`, `data-cargo-no-default-features`, `data-bindgen-target` ([assets](https://trunk-rs.github.io/trunk/guide/assets/)) | flags on each command |
| Versions of wasm-bindgen and wasm-opt | trunk fetches its own; **I could not confirm from its docs that it picks the lock's wasm-bindgen version** | explicit: the lock's `0.2.130`, binaryen `version_133` |
| Works the same locally and in CI | yes | yes, if the script is one file (`cargo xtask web`, or `scripts/build_web.sh`) |

The wasm-bindgen CLI and the `wasm-bindgen` crate must be the same version or the glue refuses to load; the lock
says `0.2.130` and the CLI 0.2.130 has prebuilt Linux x86-64 tarballs with `.sha256sum` files
([release](https://github.com/wasm-bindgen/wasm-bindgen/releases/tag/0.2.130)), so CI does not compile it. binaryen
`version_133` has `binaryen-version_133-x86_64-linux.tar.gz` with a `.sha256`
([release](https://github.com/WebAssembly/binaryen/releases/tag/version_133)).
**Recommendation: by hand,** because three versions are then pinned in one place and the CI step that reads the
lock's wasm-bindgen version cannot drift. If the builder prefers trunk, pin the binary by release and check its
log for the wasm-bindgen version it downloaded.

The commands (build, bindgen, optimise), as the script should run them:

```sh
cargo build --profile web --target wasm32-unknown-unknown -p cav-app --bin cav-web
wasm-bindgen --target web --no-typescript --out-dir dist --out-name cavendish \
  target/wasm32-unknown-unknown/web/cav-web.wasm
wasm-opt -O2 -o dist/cavendish_bg.wasm dist/cavendish_bg.wasm      # see "size" for -O2 vs -Oz
cp crates/cav-app/web/index.html dist/
```

`--target web` gives an ES module (`import init from './cavendish.js'; await init();`), which every browser in §2
loads. Trunk's default is `no-modules`.

**Rust target.** `rust-toolchain.toml` pins `1.100.0` with `rustfmt` and `clippy` only. CI adds the target with
`rustup target add wasm32-unknown-unknown`. Adding `targets = ["wasm32-unknown-unknown"]` to the file would make
rustup fetch it on the owner's PC at the next `cargo` run; that is a small download but a change on his machine, so
it is the owner's call (`plan/PROGRESS.md`, "For the owner"). Rust 1.100 turns on `bulk-memory`, `sign-ext`,
`reference-types`, `multivalue`, `mutable-globals` and `nontrapping-fptoint` by default for this target (same
platform page), which every current browser supports; `wasm-opt` needs `--enable-bulk-memory` and the others only
if an old binaryen rejects them, which version_133 should not (not tested).

**Size.** Not measured. For a plan, an egui + wgpu app is megabytes of wasm, not hundreds of kilobytes; Pages
serves it compressed (I did not check how: not stated in the Pages docs I read), and the 1 GB site limit
([limits](https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits)) is far away. What keeps it down, cheapest
first:

1. A separate `[profile.web]` that `inherits = "release"` with `lto = true`, `codegen-units = 1`, `strip = true`,
   `panic = "abort"`, so the native profiles are untouched. Keep `opt-level = 3` for `cav-core` and `cav-sim` (the
   numbers' speed matters), `"s"` for everything else. Measure both settings.
2. `default-features = false` is already set on eframe and egui; keep `webgl` out (it pulls the GLES backend into
   the module; the size of that was not measured).
3. `wasm-opt -O2` for speed, `-Oz` for size; build once with each and keep the one whose wall-clock per frame is
   not worse.
4. `twiggy` or `cargo bloat --target wasm32-unknown-unknown` on the first build to see what is large. `ron`,
   `serde_json`, `schemars` (the schema in the console) and `png` are the likely non-GUI weights.

Debug builds of this app in a browser will be too slow to use; the script builds `web` only.

## 2. WebGPU and the fallback

**What wgpu does.** On wasm, `webgpu` is on by default in wgpu's own feature set, `webgl` is opt-in ("Enables the
GLES backend on WebAssembly only"), and "only `webgpu` is enabled by default for WebAssembly"
([wgpu feature flags](https://docs.rs/wgpu/30.0.2/wgpu/#feature-flags)). **Our workspace sets
`default-features = false` with `vulkan`, `dx12`, `wgsl`**, so on wasm the build has no backend at all, and eframe's
`wgpu_no_default_features` does not turn `egui-wgpu/default` (which enables `wgpu/webgl`) back on
(`eframe-0.37.0/Cargo.toml`, features `wgpu`, `wgpu_no_default_features`). So the web build must add, for wasm
only:

```toml
# crates/cav-gpu/Cargo.toml (and any crate that names wgpu with features)
[target.'cfg(target_arch = "wasm32")'.dependencies]
wgpu = { workspace = true, features = ["webgpu"] }
```

Cargo unifies features per build, so this reaches `egui-wgpu` and `eframe` in the wasm build only. egui-wgpu's
default web setup prefers WebGPU and falls back to WebGL "if the `webgl` feature is on", and refuses WebGPU in an
insecure context (`egui-wgpu-0.37.0/src/setup.rs`, around lines 104–131 and 180); GitHub Pages is HTTPS, and
`localhost` counts as secure.

**Which browsers ship WebGPU** (read 24 Nov 2026; the sources disagree on Firefox, so both are given):

| Browser | Status | Source |
|---|---|---|
| Chrome, Edge | Windows, macOS, ChromeOS since 113; Android 12+ since 121; Linux only for Intel Gen12+ (144) and NVIDIA on Wayland (147), otherwise behind a flag | [gpuweb implementation status](https://github.com/gpuweb/gpuweb/wiki/Implementation-Status), [web.dev, 25 Nov 2025](https://web.dev/blog/webgpu-supported-major-browsers) |
| Safari | macOS, iOS, iPadOS, visionOS 26 and later, on by default; caniuse marks desktop "partial" | same two; [caniuse](https://caniuse.com/webgpu) |
| Firefox | Windows since 141 (Mozilla's [141 notes](https://developer.mozilla.org/en-US/docs/Mozilla/Firefox/Releases/141): "except in service workers"); Apple Silicon macOS 145 on macOS 26, 147 on all macOS (gpuweb wiki); Linux, Intel Macs and Android not yet. **caniuse lists Firefox as "disabled by default" in every version**, which contradicts the wiki; I take the wiki and Mozilla's notes as primary and caniuse as lagging, but did not run Firefox | as listed |
| Global | caniuse gives about 88.6 % of users with full or partial support | [caniuse](https://caniuse.com/webgpu) |

Net: the owner's likely machines (Windows Chrome, Edge, Firefox; a Mac with Safari 26) work. Linux browsers are the
weak spot. WebGPU needs an HTTPS or localhost page ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/WebGPU_API)).

**WebGL2 fallback: not in version 1.** Reasons:

- What would still work: the renderer. It uses `var<uniform>`, vertex buffers, `@builtin(vertex_index)`,
  `Rgba8UnormSrgb` and `Depth32Float` targets (`renderer.rs` lines 13–16, 276–286; `shaders.wgsl`), all available on
  WebGL2-level devices. egui-wgpu chooses `Limits::downlevel_webgl2_defaults()` when the adapter is GL
  (`setup.rs` line 262). I did not run it.
- What would break: compute shaders and storage buffers, which downlevel WebGL2 limits do not offer. S5's "gravity
  on the GPU" kernels and the GPU/CPU parity cases (`plan/REBUILD.md` rows 39–41, 120, 340) are exactly that. A WebGL
  build would show scenes with the CPU path only and would need a second set of expectations; and `GpuPanes`,
  the plots' texture path and any readback assumptions would each need a check on it.
- It costs size (the GLES backend) and a second configuration to test, which nobody can test in CI.

When a browser has no WebGPU, show a static message (the `index.html` checks `navigator.gpu` before loading the
module) naming Chrome, Edge, Safari 26 or Firefox on Windows. Revisit WebGL only if the owner needs phones or Linux
Firefox, and then render-only.

## 3. Threads

**What forces the choice.** `std::thread::spawn` panics on `wasm32-unknown-unknown` and `std::fs` always errors
([platform support](https://doc.rust-lang.org/nightly/rustc/platform-support/wasm32-unknown-unknown.html)). The
engine's `Forward::new` spawns two threads and `expect`s them
(`crates/cav-engine/src/forward.rs:270,294`), and `Engine::settle` sleeps in a loop
(`engine.rs:1357`); these would panic at start-up on the web. The reads inside a worker (`cav-sim/src/read.rs`)
already take the one-thread path when `threads()` is 1: `read.rs:633–640` falls back to 1 when the environment has
no `CAV_THREADS` and `available_parallelism` fails (the first from `std::env::var`, the second `Err` on this target;
both by reading, not by running), and `contribution_on` runs `work()` inline when `workers == 1` (`read.rs:831`).

**Option A, real threads.** Needs a nightly compiler, `-C target-feature=+atomics,+bulk-memory,+mutable-globals`,
`-Z build-std=panic_abort,std`, a shared-memory module and a pool of Web Workers
([wasm-bindgen threads guide](https://rustwasm.github.io/docs/wasm-bindgen/examples/raytrace.html)), and
SharedArrayBuffer, which needs cross-origin isolation (COOP and COEP headers). GitHub Pages does not let a site set
headers (its docs mention none:
[limits page](https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits)). The
workaround is [coi-serviceworker](https://github.com/gzuidhof/coi-serviceworker): a service worker that adds the
headers. Its README states the caveats: the page reloads once on first visit, HTTPS only, the script must be its own
file on the same origin, and it defaults to `credentialless` COEP. Add: a service worker is exactly the caching layer
I advised against above; wgpu on the web also wants `fragile-send-sync-non-atomic-wasm` only without atomics, so
atomics change wgpu's thread-safety story ([wgpu flags](https://docs.rs/wgpu/30.0.2/wgpu/#feature-flags)); and the
whole workspace is rebuilt with `build-std` on nightly, which breaks the pinned-stable toolchain rule. Not worth it
for a demo.

**Option B, one thread (recommended).** Add a second `Forward` implementation behind
`cfg(target_arch = "wasm32")` with the same public surface (`submit`, `poll`, `settle`, `signal`, `state`,
`reading`). `poll()` runs the pending job for a bounded time per call (say 8 ms), then returns; "stale until whole"
and "a newer request replaces one not begun" survive, since both are decisions about a queue, not about threads.
`settle()` on the web simply is not called (the app polls; `settle` is for the CLI and MCP).

What it costs:

- **The rule "the API never blocks on a solve" is kept at the API, not at the frame.** A command returns at once and
  marks the signal stale. But one source's contribution is one call to `read::contribution`, which does not yield.
  For the default scene (`scenes/library/room.ron`: 5 towers, 12 s, a shot every 2 s, so about 30 shots) that is
  tiny; for a scene with thousands of shots the frame that runs it stutters. `read.rs` already cuts a tower's shots
  into pieces of `PIECE = 64` shots with a fixed order and no cross-piece sums (`read.rs:608–610`), so the natural
  second step is to make that loop resumable and yield between pieces. That is a change to `cav-sim`, so it
  is a later task, with the measure: longest frame while a 10 000-shot scene is read.
- **No second core.** Native reads use up to 8 threads (`read.rs:629–640`); the web read is one. The result is the
  same numbers ("the same bits however it was arrived at, on any number of threads", `docs/ARCHITECTURE.md`), only
  later.
- **Bits against native are not promised.** wasm arithmetic is IEEE 754 and does not fuse multiply-adds, but
  `sin`, `exp` and friends come from Rust's bundled maths library on this target, not the platform's, so a result
  may differ from the native one in the last bits. Phases at µrad are far above that, but a case that compares
  *bit patterns* across platforms would fail. I did not check this; it is reasoned from how the targets differ.
- **An alternative that keeps a real background:** run the engine inside a dedicated Web Worker and send commands
  and replies by `postMessage` (no SharedArrayBuffer needed). That is a rewrite of the app's engine access into a
  message protocol; the architecture table says the app "sends commands and reads replies", so it fits in principle,
  but it is a much larger task than B. Leave it unless B's stutter matters.

## 4. What changes, crate by crate

Rule of thumb from the grep: Rust *compiles* these uses on wasm and they fail when run, so each needs a gate or a
replacement, found by reading, not by `cargo check`.

| Crate | Uses | On wasm | Change |
|---|---|---|---|
| `cav-core` | `std::thread::scope` and `std::fs` in `cadence.rs:505`, `voxel/tests.rs:299` (tests only); `npy.rs:287,304` and `voxel/mesh.rs:140` read and write files; `Instant` in `spectrum.rs:794` (a test) | compiles; the file functions return errors | none for v1. File readers keep their paths; the web build never calls them |
| `cav-sim` | `std::fs` in `library.rs:69`, `presets.rs:85`, `spec.rs:770`, `bundle.rs`; `std::env` in `read.rs:634` and `bundle.rs:430`; `thread::scope` in `read.rs:833` | the library scenes are already `include_str!` (`library.rs:19`); the thread path is not taken with one worker | none for v1, once the web `threads()` is 1. Check that `std::env::consts` (`bundle.rs:430`) is fine: it is a constant |
| `cav-engine` | `Forward::new` threads; `Instant` in `forward.rs`, `log.rs:132`, `engine.rs:1345`; `std::fs` in `engine.rs:361,371` (export) and `journal.rs:75`; `build.rs` and `provenance.rs` use `std::process` and run git | **panics at start** (threads); `Instant::now()` panics on this target (not verified here: I know `std::time::Instant` is unsupported on `wasm32-unknown-unknown`; `web-time` 1.1.0 is already in the lock for egui) | (1) wasm `Forward` as in §3. (2) `use web_time::Instant` under `cfg(target_arch = "wasm32")` in one small module and import from there (`web-time` as a target dependency). (3) file commands (`Export`, journal save/load by path) return a clear refusal on wasm. (4) **`build.rs` needs no change**: a build script runs on the host, so it still asks git, which CI has after `actions/checkout`; provenance is stamped as it is today. Check `fetch-depth` is enough (`dirty` and `revision` need only HEAD) |
| `cav-gpu` | `Gpu::new` uses `pollster::block_on` for adapter and device; `checked` blocks on `scope.pop()`; `read_rgba8` polls `wait_indefinitely` | `block_on` on the browser's main thread cannot wait for a promise: it hangs or panics; this is the second most important break | on wasm, do not create a device here. Add `Gpu::from_parts(instance, adapter, device, queue)` (the fields are `pub`, so a struct literal already works) and build it from eframe's `wgpu_render_state`, which carries all four (`egui-wgpu-0.37.0/src/lib.rs`, `RenderState`). `checked` on wasm just runs the closure (errors then surface as wgpu's uncaptured-error handler: log them); `read_rgba8` and `all_adapters` are native-only (`#[cfg(not(target_arch = "wasm32"))]`). `wgpu` gets `webgpu` for wasm (§2). `pollster` becomes native-only |
| `cav-render` | `Instant` in `scene.rs:874`; `include_str!` shader; `render_png`/readback via `Gpu` | shader embedded already; readback gated with `cav-gpu` | `web_time::Instant`; gate `render_rgba` and `render_png` (screenshots) native-only |
| `cav-plot` | `std::fs` and `File` in `figure.rs` (figure export and committed figures); `headless.rs` makes its own `Gpu::new()`; fonts are `include_bytes!` (`fonts.rs:14`) | export and headless fail at run time; painting through egui works | gate `headless` and the file writers native-only, or leave them and never call them; v1 does not export figures |
| `cav-app` | `main.rs`: `std::env::args`, `std::fs::write`, `Instant`, `eframe::run_native`, `Gpu::new` | would not run | add a web entry (below); keep `main.rs` native. `egui-wgpu = { features = ["winit"] }` should be `cfg(not(wasm32))` only: eframe on web does not use winit |
| `cav` (CLI, MCP: tokio, rmcp, `std::process`) | everything | not built | nothing: `cargo build -p cav-app --target wasm32-unknown-unknown` does not compile `cav` |
| `cav-memtest` | a counting allocator, `unsafe` | dev-dependency of tests only | nothing |
| `cav-validate`, `xtask`, `oracle/` | `std::env`, files | tests and tools, native only | nothing |

**A web entry in `cav-app`.** A new `[[bin]] cav-web` (`src/web.rs`) whose body is `#[cfg(target_arch = "wasm32")]`
and, on other targets, a `main` that prints "build for wasm32-unknown-unknown". The wasm `main` follows the template:

```rust
fn main() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let canvas = web_sys::window().and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("cavendish"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok()).expect("canvas #cavendish");
        eframe::WebRunner::new().start(canvas, eframe::WebOptions::default(), Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
            let state = cc.wgpu_render_state.as_ref().ok_or("no wgpu render state")?;
            let gpu = cav_gpu::Gpu::from_parts(state.instance.clone(), state.adapter.clone(),
                                               state.device.clone(), state.queue.clone());
            let panes = GpuPanes::new(gpu, Arc::clone(&state.renderer));
            Ok(Box::new(App::web(Shell::new(), panes)))   // `App` as in main.rs, minus --exec and --minimised
        })).await.expect("starting Cavendish");
    });
}
```

This is a sketch from the 0.37 signatures, not compiled. `App` (in `main.rs`) should move to a small shared module so
both entries use it. `Shell::new()` loads nothing from disk. The `--exec` console line can come from the URL's
`#fragment` later.

**Fonts.** `default_fonts` is on for egui (workspace `Cargo.toml`), `system_fonts` is not (eframe's default features
are off), and the plots' mono font is `include_bytes!` (`cav-plot/src/fonts.rs`). So nothing to do.
**Clipboard, file dialogs.** None are used (`rfd`, `arboard` are not in the lock; the "clipboard" feature of eframe
is off). eframe's web backend handles copy and paste events itself; not checked against the console's text field.
**Scenes.** The library is embedded with `include_str!`; the 23 dev scenes in `scenes/dev/*.ron` are embedded
too if the `dev` module is (not checked; `dev.rs:627` reads files only in a test). 98 KB of scenes in all.
**Saving and loading.** Version 1: none. Later: save = build the RON in memory and give it to the browser as a
download (`web_sys::Blob` + an `<a download>`); load = an `<input type=file>` or the canvas drop (eframe has
`dropped_file.rs`), parsed with `SceneSpec::from_ron`, which the engine's `Load` of an inline scene already accepts
(`library::inline`). Autosave to `localStorage` is possible (eframe has `storage.rs` and a `persistence` feature) but
the journal would need a stable format first; do not start it here.
**Memory.** wasm32 has a 4 GiB address space and browsers allow less in practice, so the 10⁹–10¹¹ cell scales of
the plan are native only. The web build must cap voxel and shot counts to a size the tab survives; the engine's
existing bounds (`cav-memtest`) tell which. A web limit should be a stated constant, refused with a message, not a
tab crash.

**Keeping native and `quick` unaffected.** Every change is a `cfg(target_arch = "wasm32")` gate or a
target-specific dependency, so native resolution is unchanged; `Cargo.lock` gains nothing as the web crates are
already in it. `xtask`'s test `the_crates_depend_as_the_plan_says` reads workspace dependencies, so a new
target-specific `web-time`/`wasm-bindgen-futures` entry should be checked against it, and the architecture table
gets one line: "`cav-gpu`: on wasm it is handed the browser's device, not asked to make one".

**A CI check that it compiles.** `cargo check --target wasm32-unknown-unknown -p cav-app --bin cav-web` (plus
`--profile web` build is the real check). Cost: not measured. A first check compiles the whole graph (wgpu, egui,
naga) for a second target, which on the build PC will be minutes cold; warm it is the cost of re-checking the
crates that changed, probably a few seconds. `quick` has a 30 s warm budget (CLAUDE.md), so **keep it out of
`quick`** until the warm time is measured under 5 s, and put it in `cargo xtask gpu` or the slice-done tier, as
`cargo xtask web` (check, and in CI the full build). As stated in the Summary, a successful `check` does not prove
the app starts; add to the Pages workflow a smoke step that loads `dist/` in headless Chrome and fails on a console
panic (not designed here: a question below).

## 5. The GitHub Actions workflow

### 5.1 The repository setting

Pages is not enabled now (`gh api repos/Ma1achy/Cavendish/pages` returns 404, read today). The repository is
public (`gh repo view`), so Pages is free. The owner's `gh` (needs repository admin) sets the source to Actions:

```sh
gh api -X POST repos/Ma1achy/Cavendish/pages -f build_type=workflow
```

`POST /repos/{owner}/{repo}/pages` takes `build_type` of `legacy` or `workflow`, and needs a repository administrator
([REST docs](https://docs.github.com/en/rest/pages/pages#create-a-apiname-pages-site)); the docs' only example sends
`source`, so the `build_type=workflow` form is from the parameter list, not from an example I saw run. If the call
returns 409 (a site exists), use `-X PUT` with the same field; or in the browser: Settings, Pages, Build and
deployment, Source = GitHub Actions. The first deploy also creates the `github-pages` environment. The default
workflow token is read-only (`gh api repos/Ma1achy/Cavendish/actions/permissions/workflow`, read today), so the
workflow below must ask for its own permissions, as it does. The site is then at
`https://ma1achy.github.io/Cavendish/`, so `index.html` must use relative URLs.

### 5.2 `.github/workflows/pages.yml` (sketch, not run)

Action versions are the latest releases read on 24 Nov 2026 with their commit SHAs from the GitHub API, in
`progress.yml`'s style. `progress.yml` pins checkout `v4.4.0`; this one pins newer majors, so update `progress.yml`
separately if you want one version everywhere.

```yaml
name: pages
on:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read

concurrency:
  group: pages
  cancel-in-progress: false   # a deployment is never cut short; a newer push waits its turn

jobs:
  build:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
        with: { ref: main, fetch-depth: 1 }          # build.rs asks git for HEAD
      - run: rustup target add wasm32-unknown-unknown   # the toolchain is pinned by rust-toolchain.toml
      - uses: Swatinem/rust-cache@d1527c8f67ef9af357290475d4af4b0fab8f12e0 # v2.9.3
        with: { key: web }
      - name: Tools (versions pinned; wasm-bindgen from Cargo.lock)
        run: |
          set -euo pipefail
          wb=$(awk '/^name = "wasm-bindgen"$/{getline; gsub(/[^0-9.]/,"",$3); print $3; exit}' Cargo.lock)
          # download the release tarball and its .sha256sum, check, put `wasm-bindgen` on PATH
          # download binaryen-version_133-x86_64-linux.tar.gz and its .sha256, check, put `wasm-opt` on PATH
      - run: scripts/build_web.sh        # the same script the builder runs locally; writes dist/
      - uses: actions/upload-pages-artifact@fc324d3547104276b827a68afc52ff2a11cc49c9 # v5.0.0
        with: { path: dist }

  deploy:
    needs: build
    runs-on: ubuntu-24.04
    permissions:
      pages: write
      id-token: write
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - id: deployment
        uses: actions/deploy-pages@368f82528645a54fb793d4d04e342629a3f51346 # v5.0.1
```

Why this shape, from the docs: the deploy job "must have a minimum of `pages: write` and `id-token: write`", targets
the `github-pages` environment, and `needs` the build job
([custom workflows](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages),
[deploy-pages](https://github.com/actions/deploy-pages)); `upload-pages-artifact` takes `path` and leaves out dot
files unless `include-hidden-files` is set, and the artifact must hold no symbolic or hard links
([upload-pages-artifact](https://github.com/actions/upload-pages-artifact)), so do not put a symlinked `dist`.
`configure-pages` (v6.0.0, `45bfe019…`) is optional: it only reads site metadata, which a relative-URL site does not
need. I did not run the deploy actions' v5 against v5 of the uploader together; their READMEs I read show examples
with older tags (v3, v4). **Verify on the first run** that the pair works.

- **Does not commit to `main`.** It writes no branch at all; the artifact goes to Pages' store. (The old template
  route pushes `gh-pages`; this does not.)
- **Beside `progress.yml`.** Both trigger on push to `main`, run independently (different `concurrency` groups,
  `progress` and `pages`), and neither reads the other's output. `progress` force-pushes the orphan branch
  `progress`; with Pages' source set to Actions that branch is not served. `progress.yml` has `contents: write`;
  `pages.yml` only `contents: read` at top level.
- **Failure.** A failed Pages build leaves the last good site up. Do not make `pages` a gate of anything; the README
  can link the site. The pre-push hook still runs `cargo xtask quick` only.
- **Limits.** 1 GB site, 100 GB/month soft bandwidth, 10-minute deployment timeout, no limit on workflow builds per
  hour ([limits](https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits)).
- **Caching and time.** `Swatinem/rust-cache` v2.9.3 keys on `Cargo.lock` and the toolchain; give the web build its
  own `key` so the native and wasm target directories do not evict each other. Build time on a hosted runner is
  **not measured**. My estimate, from nothing better than the shape of the graph, is 5–12 min cold and 1–3 min
  warm with `lto = true`, because wgpu, naga, egui and our own crates are compiled once per `Cargo.lock` change.
  Timing the first run is the measurement; if LTO makes it slow, build with `lto = "thin"` in CI first.

## 6. A first version, and what to leave out

**Task A, make it build (one task).** `cav-gpu` from-parts and wasm gates; `web-time`; the wasm `Forward` (same
surface, run from `poll`); gates for the file commands; `cav-web` bin; `[profile.web]`; `scripts/build_web.sh`
(or `cargo xtask web`); `index.html` with the `navigator.gpu` check and an HTML fallback message. Exit: `dist/`
served from `python -m http.server` shows the app's `room` scene in Chrome, the console accepts `load` of a library
scene, a pane renders, a plot draws, the timeline plays; `cargo xtask quick` and the native demo unchanged; the
wasm size and warm build time recorded in `plan/PROGRESS.md`.

**Task B, publish it.** `pages.yml` as above, the owner's one setting, the README link, and a note in `CONTRIBUTING`
on the local command. Exit: a push to `main` produces a page at the URL; the run's log shows the revision stamped.

**Leave out of v1:** WebGL2; real threads and COOP/COEP; a service worker or offline use; saving and loading files
(the download/upload path above); the `--exec` URL fragment; touch and mobile layout; MCP and CLI; the screenshot and
figure exports; the oracle and validation tiers on wasm; any attempt at the 10⁹–10¹¹-cell scenes (state a cap).

## 7. Ideas that are not for now

- A resumable `read::contribution` that yields between `PIECE`-shot pieces (frame-stutter fix for big scenes).
- Engine in a Web Worker with messages, to get a real background without SharedArrayBuffer.
- A wasm-side run of the cheap validation cases (`wasm-bindgen-test` in headless Chrome) to prove the numbers match
  native to tolerance; this also settles the `libm` question in §3.
- `?scene=name` and `#exec=...` in the URL so a link opens a given scene and view.

## 8. Sources

eframe 0.37.0, egui-wgpu 0.37.0, wgpu 30.0.2 sources in the local cargo registry (`web/web_runner.rs`, `setup.rs`,
`lib.rs`, `Cargo.toml` of each); [eframe README](https://github.com/emilk/egui/blob/main/crates/eframe/README.md);
[eframe_template](https://github.com/emilk/eframe_template) and its Pages workflow; [wgpu feature flags](https://docs.rs/wgpu/30.0.2/wgpu/#feature-flags);
[Rust platform support, wasm32-unknown-unknown](https://doc.rust-lang.org/nightly/rustc/platform-support/wasm32-unknown-unknown.html);
[wasm-bindgen threads guide](https://rustwasm.github.io/docs/wasm-bindgen/examples/raytrace.html);
[coi-serviceworker](https://github.com/gzuidhof/coi-serviceworker); [trunk assets](https://trunk-rs.github.io/trunk/guide/assets/);
[caniuse WebGPU](https://caniuse.com/webgpu); [gpuweb implementation status](https://github.com/gpuweb/gpuweb/wiki/Implementation-Status);
[web.dev](https://web.dev/blog/webgpu-supported-major-browsers); [MDN WebGPU](https://developer.mozilla.org/en-US/docs/Web/API/WebGPU_API);
[Firefox 141](https://developer.mozilla.org/en-US/docs/Mozilla/Firefox/Releases/141);
[Pages custom workflows](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages);
[Pages limits](https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits);
[Pages REST](https://docs.github.com/en/rest/pages/pages#create-a-apiname-pages-site);
[deploy-pages](https://github.com/actions/deploy-pages); [upload-pages-artifact](https://github.com/actions/upload-pages-artifact).
Repository: `Cargo.lock`, `Cargo.toml`, the crates' sources at the lines cited, `.github/workflows/progress.yml`,
`docs/research/s0-app-stack-and-build-speed.md`, `docs/ARCHITECTURE.md`.

## 9. What I could not verify

1. **Nothing was compiled or run for wasm.** The target is not installed; installing it is outside the repository.
   Resolve with `rustup target add wasm32-unknown-unknown` then `cargo check --target wasm32-unknown-unknown -p
   cav-app` as the first step of Task A, before changing code, to see what really fails. A likely first failure is
   `getrandom` 0.4 on this target (reached through `accesskit` and `uuid` in the lock), which needs its `wasm_js`
   backend; this is a guess from the dependency chain.
2. **wasm size, build time and warm `check` time** are estimates or absent. Measure on the first build.
3. **`Instant::now()` panicking on this target** is from my knowledge of std, not from a page I could fetch (the
   platform page does not mention time); `web-time` is the standard cure either way.
4. **Whether trunk picks the lock's wasm-bindgen version**, and **the `build_type=workflow` form of the Pages call**,
   are from partial documentation. The call returns 404 now, and the first real run shows if it worked.
5. **Firefox's WebGPU default** conflicts between caniuse and the gpuweb wiki/Mozilla's notes; I did not run any
   browser. Safari desktop is "partial" in caniuse; what is missing was not read.
6. **Whether Pages compresses `.wasm`** and with what: not in the docs I read; check the transfer size in the browser
   on the first deploy.
7. **`upload-pages-artifact` v5 with `deploy-pages` v5** together: only older examples seen.
8. **Cross-platform last-bit differences** in `sin`/`exp` (§3) are reasoned, not measured.
9. **That the engine's `Forward` can be given a run-from-poll twin** with the same observable behaviour was read
   from `forward.rs:255–340` (its slots and gates) and not prototyped.
