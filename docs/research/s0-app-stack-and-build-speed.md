# S0 research: app stack, MCP server, build speed, schema

21 Nov 2026. For the builder starting S0 from an empty workspace. Versions were read from the
crates.io API on that date; API claims marked **[compiled]** or **[ran]** were checked in a throwaway
288-line skeleton (`cav doctor | render | mcp | app | schema`, outside the repo) on the build PC:
Ryzen 5 3600 (6c/12t), 24 GB, GTX 1660 SUPER (driver 581.29), Windows 10, SATA SSD,
rustc/cargo 1.100.0 stable `x86_64-pc-windows-msvc`. Everything else is from the linked source only.

## Summary

- **Stack:** `eframe 0.37.0` + `wgpu 30.0.2` (winit 0.30.13 comes transitively). Create the wgpu device
  ourselves in `cav-gpu` and hand it to eframe; `cav-render` draws each pane into its own texture, which
  egui shows as an image. The same call renders to PNG with no window.
- **MCP:** `rmcp 3.11.0` on a current-thread tokio runtime. `scene_schema` and an image-returning
  `render_view` work over stdio today.
- **Build:** the S0 exit "under a minute cold" is not met reliably on this PC: 60–100 s measured, median
  about 75 s, for the skeleton alone. Warm rebuild is 4.5 s and warm `cargo check` 1.4 s. Proposed
  budget in §3.
- **Schema:** `schemars 1.2.3` (the version rmcp itself uses) for `scene_schema`; RON 0.12.3 for files.

## 1. The app stack

| Crate | Version (date) | MSRV | Notes |
|---|---|---|---|
| [eframe](https://crates.io/crates/eframe) / egui / egui-wgpu / egui-winit | 0.37.0 (2026-10-15) | 1.95 | requires `wgpu ^30.0`, `winit ^0.30.13` |
| [wgpu](https://crates.io/crates/wgpu) | 30.0.2 (2026-11-12) | 1.87 | 30.0.0 was 2026-07-01 |
| [winit](https://crates.io/crates/winit) | 0.30.13 (2026-03-02) | 1.70 | 0.31 is still beta (beta.4, 2026-10-24); do not use |
| [egui_plot](https://crates.io/crates/egui_plot) 0.38.0, [egui_dock](https://crates.io/crates/egui_dock) 0.22.0, [egui_tiles](https://crates.io/crates/egui_tiles) 0.18.0 | Oct–Nov 2026 | 1.95 | all require `egui ^0.37` |

So the only compatible set is egui 0.37 / wgpu 30 / winit 0.30. Changes since v1's egui 0.29 that the
builder will meet ([egui changelog](https://github.com/emilk/egui/blob/main/CHANGELOG.md),
[eframe changelog](https://github.com/emilk/egui/blob/main/crates/eframe/CHANGELOG.md)): wgpu is
eframe's default renderer (0.34); `App::update` is replaced by `App::ui(&mut self, ui, frame)` plus
`App::logic` (0.34); `default_fonts` is no longer a default feature (0.37); `Panel::show_inside` is
deprecated in favour of `show(ui, …)` **[compiled]**; wgpu 30 makes `get_mapped_range()` return a
`Result` and `VertexState::buffers` take `Option`s
([wgpu changelog](https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md)).

**eframe, not egui-winit + egui-wgpu by hand.** The reason to go direct used to be device ownership.
That is now covered: `NativeOptions.wgpu_options.wgpu_setup = WgpuSetup::Existing(WgpuSetupExisting {
instance, adapter, device, queue })`
([docs](https://docs.rs/egui-wgpu/0.37.0/egui_wgpu/enum.WgpuSetup.html)) runs eframe on a device we
created **[ran, Vulkan and DX12, 6 s each without a panic; the window contents were not inspected]**.
eframe then supplies the event loop, DPI, IME, clipboard and OS windows for free, and going direct
would not shorten the build (egui-winit and egui-wgpu are compiled either way).

**Panes are textures, not paint callbacks.** egui-wgpu offers two ways to put our pixels in the UI:

1. `CallbackTrait::paint` draws into egui's own render pass
   ([docs](https://docs.rs/egui-wgpu/0.37.0/egui_wgpu/trait.CallbackTrait.html)). The pass, its format
   and its depth/sample state belong to egui, and it does not exist without a window.
2. `Renderer::register_native_texture(device, &view, filter) -> TextureId`, then `ui.image(...)`;
   on resize, `update_egui_texture_from_wgpu_texture`
   ([docs](https://docs.rs/egui-wgpu/0.37.0/egui_wgpu/struct.Renderer.html)) **[compiled, ran]**.

Use (2). `cav-render` exposes one entry point that draws a view into a `wgpu::TextureView` it is
given, with its own passes, depth buffer and formats. A pane is a texture of the pane's pixel size;
`cav render` and MCP `render_view` create a texture, call the same function, copy to a buffer and
encode a PNG **[ran: 1280×720 PNG in 0.46–0.70 s per process including device creation]**. 1, 2 or 4
panes are 1, 2 or 4 textures in one window. Note the naming clash: egui's "viewport" means an OS
window (`Context::show_viewport_deferred`); the plan's viewports are panes.

Crate consequences: `cav-gpu` owns instance/adapter/device creation; `cav-render` depends on `wgpu`
only (no egui); only `cav-app` depends on eframe. That keeps eframe out of `cav`'s build unless `cav`
launches the GUI.

**Backend on Windows/NVIDIA.** wgpu lists Vulkan and DX12 both as first-class on Windows
([README](https://github.com/gfx-rs/wgpu#supported-platforms)). With both enabled, wgpu chose Vulkan on
this PC, and both backends rendered the PNG and ran the app **[ran]**. Recommendation: enable both,
default to Vulkan, keep DX12 as the fallback selected with `WGPU_BACKEND=dx12`; `cav doctor` prints
every adapter. Reasons: Vulkan is the path shared with Linux, and on DX12 shaders go WGSL → HLSL →
a Microsoft compiler, where wgpu falls back to the old FXC unless `dxcompiler.dll` is shipped or the
`static-dxc` feature is on (README, "Environment variables"). Known pitfalls:

- Device creation costs 0.2–0.5 s per process (measured), so the MCP server must keep one device
  for its lifetime rather than one per call, as the skeleton lazily did.
- In debug builds wgpu validation is on by default (`InstanceFlags::from_build_config`); expect
  slower frames in dev than in release.
- Not checked: whether the two backends give bit-identical kernel results. That belongs to the GPU
  parity tests (S5); run them on both.

**Fonts.** egui's `default_fonts` feature already embeds `Ubuntu-Light.ttf` (proportional) and
Hack (monospace) ([epaint_default_fonts](https://crates.io/crates/epaint_default_fonts), licence field
`(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0`). Ubuntu Mono is not included: add
`UbuntuMono-Regular.ttf` under `assets/fonts/` with the licence text and install it with
`ctx.add_font(FontInsert::new("UbuntuMono", FontData::from_static(include_bytes!(…)),
vec![InsertFontFamily { family: FontFamily::Monospace, priority: FontPriority::Highest }]))`
(signature read from epaint 0.37 source; not compiled). Set `NativeOptions::system_font_fallback =
false` (or leave the `system_fonts` feature off) so screenshots are identical on every machine.
Licence: the [Ubuntu Font Licence 1.0](https://ubuntu.com/legal/font-licence) allows the fonts to be
"bundled, embedded, and redistributed"; each copy must carry the copyright notice and the licence;
unmodified fonts keep their name; the fonts stay under the UFL, which places no condition on the
licence of the program. Shipping the `.ttf` plus `UFL.txt` satisfies it. This is my reading of the
text, not legal advice; publishing is an owner decision anyway.

## 2. The MCP server

[rmcp](https://crates.io/crates/rmcp) 3.11.0 (2026-11-17), Apache-2.0, MSRV 1.88, from
[modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk). It released six
minor versions in the last month (3.6 on 20 Oct to 3.11), so pin it exactly and upgrade on purpose.
It implements MCP `2026-07-28` and negotiates older versions; a `2025-11-25` client handshake
succeeded **[ran]**. It requires tokio (`sync, macros, rt, time`; `transport-io` adds `io-std`, `net`);
a current-thread runtime is enough **[ran]**.

Minimal shape **[compiled, ran: initialize 9 ms, `tools/list`, `scene_schema`, `render_view`]**:

```rust
#[derive(Clone)] pub struct Cav;                      // will hold an engine handle

#[tool_router]
impl Cav {
    /// JSON Schema of the scene spec.               // the doc comment becomes the description
    #[tool(name = "scene_schema")]
    fn scene_schema(&self) -> String { serde_json::to_string(&schemars::schema_for!(SceneSpec)).unwrap() }

    #[tool(name = "render_view")]
    fn render_view(&self, Parameters(a): Parameters<RenderArgs>) -> Result<CallToolResult, String> {
        Ok(CallToolResult::success(vec![ContentBlock::image(base64_png, "image/png")]))
    }
}
#[tool_handler(name = "cav", version = "0.0.0", instructions = "…")]
impl ServerHandler for Cav {}

// main: tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(async {
//     Cav.serve(rmcp::transport::stdio()).await?.waiting().await })
```

- Tool arguments are a struct deriving `Deserialize + JsonSchema`; its schema is the tool's
  `inputSchema`. Returning `rmcp::Json<T>` gives structured output with an `outputSchema`
  (from rmcp's `tests/test_structured_output.rs`; not run here).
- Images are base64 text plus a MIME type, as above.
- Resources (scene spec, library index, last bundle): implement `list_resources` and `read_resource`
  on `ServerHandler` and advertise the capability
  ([README](https://github.com/modelcontextprotocol/rust-sdk#readme)). Not exercised; S0 does not
  need them.
- Pitfalls: stdout is the protocol, so all logging goes to stderr; async tool bodies must be `Send`
  (3.10 changelog); a pending stdin read cannot be cancelled, so shutdown waits for EOF
  (`transport/io.rs` docs). Tools should send a command to the engine's worker and await the reply,
  never run a solve on the runtime thread.

## 3. Build speed

Measured on the skeleton (261 compilation units, registry already downloaded, target directory
deleted before each cold run, Windows Defender real-time protection on):

| Configuration | Cold `cargo build` | Rebuild after touching `main.rs` |
|---|---|---|
| Cargo defaults, `link.exe` | 81 s (1 run) | 5.9, 6.3 s |
| §8 profile (below), `link.exe` | 60, 64, 77, 102 s | 4.2–5.5 s (8 runs, median 4.5) |
| §8 profile, `rust-lld` | 71, 76, 77 s | 4.7–6.4 s (8 runs, median 4.9) |
| §8 profile, `cargo check` | 54 s (1 run) | 1.4 s |
| default crate features instead of trimmed (320 units) | 78 s (1 run) | — |

What the numbers say:

- **Cold is 60–100 s and noisy.** The run-to-run spread (60 to 102 s for the same configuration) is
  larger than any difference between configurations. I did not find the cause; antivirus scanning
  of the target directory is the usual suspect ([nextest's Windows notes](https://nexte.st/docs/installation/windows/))
  and is untested here because changing Defender settings is the owner's call.
- **The cold build is front-end bound.** `cargo check` (no code generation, no linking) takes 54 s
  against 60 s for the best full build, and `--timings` shows 508–663 CPU-seconds over 12 threads.
  The largest units: `windows` 26–30 s, `ash` 24–27 s, `vello_cpu` 22–34 s (egui's text rasteriser),
  `read-fonts` 18–27 s, `naga` 16–28 s, `moxcms` 17–22 s (pulled in by `image`), `rmcp` 13–19 s,
  `egui` 9–18 s. None is optional for this stack.
- **The profile settings help.** `debug = "line-tables-only"` with `debug = false` for dependencies
  ([Cargo book](https://doc.rust-lang.org/cargo/guide/build-performance.html)) cut the target
  directory from 2.2 GB to 1.3 GB, the executable from 50 MB to 43 MB and the rebuild from about 6 s
  to 4.5 s.
- **rust-lld buys 0.3 s.** It ships in the stable toolchain and is enabled with
  `[target.x86_64-pc-windows-msvc] linker = "rust-lld.exe"` **[ran]**; it is not the default on MSVC and
  the tracking issue is still open ([rust#71520](https://github.com/rust-lang/rust/issues/71520)).
  `-Ztime-passes` on the final crate: `run_linker` 1.68 s with `link.exe`, 1.38 s with rust-lld (two
  runs each, repeatable to 0.02 s); wall-clock rebuild times are indistinguishable. Recommendation:
  leave the default linker in S0 and re-measure when the binary has grown; this departs from
  REBUILD §8, which names rust-lld.
- **Feature trimming** removes 59 units and about 4% of CPU time; worth doing, not decisive.
- Of a 4.5 s rebuild, rustc is 2.6–3.0 s (link 1.4–1.7 s of that); the rest is Cargo.

**Nightly options (not measured: only stable is installed, and I did not add toolchains).**
Cranelift: the [README](https://github.com/rust-lang/rustc_codegen_cranelift#platform-support) lists
Windows x86_64 as fully supported, via `rustup component add rustc-codegen-cranelift-preview
--toolchain nightly`, but unwinding on panic is not supported on Windows, so `#[should_panic]` tests
and `catch_unwind` would not work. It replaces code generation only, which the check-vs-build
numbers above show is a small share of the cold build. Parallel front end (`-Zthreads=8`): still
nightly-only, stabilisation is a 2026 project goal with no date
([goal](https://goals.rust-lang.org/2026/parallel-front-end.html)); it targets the phase that
dominates here, so it is the one worth a trial. Both are optional experiments, not S0 work.

**Honest budget, proposed in place of "under a minute cold":**

| Gate | Meaning | Budget | Measured today |
|---|---|---|---|
| cold | registry downloaded, empty `target/`, `cargo build` of the app | under 2 min, recorded not gated | 60–102 s |
| warm build | one-line change in a leaf crate, `cargo build` | under 10 s | 4.5 s |
| `quick` | `cargo check` + clippy on changed crates + nextest unit tests, warm | under 10 s (S0), 30 s later | check alone 1.4 s; clippy and nextest unmeasured |

The warm numbers are for a single 288-line crate. They will rise with the real workspace, so S0
should record all three with `cargo build --timings` in the slice notes.

**cargo-nextest** 0.9.152 (2026-11-13): signed Windows binaries, installed with
`irm https://get.nexte.st/install.ps1 | iex` or `winget install nextest.cargo-nextest`
([docs](https://nexte.st/docs/installation/pre-built-binaries/)); do not `cargo install` it (a long
compile). Its docs say it "is usually slower" on Windows because it starts a process per test, and
recommend benchmarking. It was not installed when I measured, so `quick`'s test step is unmeasured.
Dev Drive, their other suggestion, needs Windows 11.

**xtask**: a workspace member `xtask` plus `[alias] xtask = "run --package xtask --"` in
`.cargo/config.toml` ([matklad/cargo-xtask](https://github.com/matklad/cargo-xtask)). Give it no
dependencies (std `Command` only) so it never adds to the build.

## 4. Schema and serialisation

- **[schemars](https://crates.io/crates/schemars) 1.2.3** (2026-10-12, MIT, MSRV 1.74). rmcp's
  `server` feature already depends on `schemars ^1.0` and re-exports it, and tool argument types must
  implement its `JsonSchema`, so any other schema crate would be a second one. It emits JSON Schema
  draft 2020-12, reads serde attributes, and turns doc comments into `description`
  **[ran: schema for a `SceneSpec` with nested structs and an enum]**.
- **[ron](https://crates.io/crates/ron) 0.12.3** (2026-10-28) for scene files; a `SceneSpec`
  round-tripped **[ran]**. Its README warns that `#[serde(tag = …)]`, `untagged` and `flatten` have
  only limited support ([limitations](https://github.com/ron-rs/ron#limitations)): keep the spec to
  plain structs and externally tagged enums.
- The schema describes the **JSON** shape of the serde types (an enum variant is
  `{"Point": {"mass": …}}`), not RON's surface syntax (`Point(mass: …)`). That is right for MCP, the
  CLI and Python, which speak JSON; it means property panels are generated from the schema but RON is
  not validated by it. Deserialising is the validation.
- Enable `serde_json`'s `float_roundtrip` feature: its manifest says only that feature "makes
  f64 -> JSON -> f64 produce output identical to the input", which S2's bit-identical replay needs.
  Whether RON round-trips every f64 bit-exactly is unverified; S2 should test it with a control arm.

## 5. Recommended pins

`[workspace.dependencies]`; exact pins for the fast-moving crates, Cargo.lock committed.

| Crate | Version | Features (`default-features = false` unless noted) | Why |
|---|---|---|---|
| eframe | =0.37.0 | `wgpu_no_default_features`, `default_fonts`, `clipboard` | no glow, x11, wayland, accesskit, links, system fonts; Ubuntu Light bundled |
| egui | =0.37.0 | — | same release as eframe |
| egui-wgpu | =0.37.0 | `winit` | only if `cav-app` names its types directly; eframe re-exports it |
| wgpu | =30.0.2 | `std`, `parking_lot`, `vulkan`, `dx12`, `wgsl` | the version egui 0.37 requires; no GLES, Metal or web on the PC (add `metal` for a Mac) |
| rmcp | =3.11.0 | `server`, `macros`, `transport-io`, `base64` | stdio server only; no HTTP, auth or client |
| tokio | 1.54 | `rt`, `macros`, `io-std` | current-thread runtime for `cav mcp` only |
| schemars | 1.2 | defaults | must match rmcp's |
| serde | 1.0 | `derive` | — |
| serde_json | 1.0 | defaults + `float_roundtrip` | exact f64 round trip |
| ron | 0.12.3 | defaults | scene files |
| png | 0.18 | — | PNG out without the `image` crate's other codecs (`image` still arrives via eframe) |
| pollster | 1.0 | — | block on wgpu's two async calls |
| bytemuck | 1.25 | `derive` | GPU buffers |
| clap | 4.6 | defaults + `derive` | CLI; about 9 s of CPU off the critical path. A hand-rolled parser is the alternative if cold time matters more |
| anyhow | 1 | defaults | errors in binaries |

Later, not S0: `egui_plot 0.38`, `egui_dock 0.22` or `egui_tiles 0.18` (both track egui 0.37; choose
when docking is built), `rayon 1.12`, `criterion 0.8`.

Workspace configuration:

```toml
# Cargo.toml
[workspace]
resolver = "3"
members = ["crates/*", "xtask"]

[workspace.package]
edition = "2024"
rust-version = "1.95"            # egui 0.37's MSRV; the PC has 1.100

[profile.dev]
debug = "line-tables-only"       # Cargo book; measured 6 s -> 4.5 s rebuild, 2.2 -> 1.3 GB

[profile.dev.package."*"]
debug = false                    # dependencies carry no debug info

[profile.dev.package.cav-core]
opt-level = 3                    # REBUILD §8; cost on rebuild not measured, crate does not exist yet

[profile.debugging]
inherits = "dev"
debug = true                     # full debug info on demand: cargo build --profile debugging

# .cargo/config.toml
[alias]
xtask = "run --package xtask --"
# Optional, measured gain 0.3 s per link; off by default:
# [target.x86_64-pc-windows-msvc]
# linker = "rust-lld.exe"
```

Add `rust-toolchain.toml` with `channel = "1.100.0"` and components `rustfmt`, `clippy`, so every
session builds with the same compiler.

## 6. What I could not verify

- **The cause of the cold-build spread (60–102 s).** Resolve by timing three cold builds with the
  target directory excluded from Defender, if the owner agrees to the exclusion.
- **Cranelift and the parallel front end on this PC.** Resolve by installing nightly and timing
  cold and warm builds with each; keep only if the warm build improves by more than a second.
- **That the app window draws the pane correctly.** It started and ran on both backends, but nobody
  looked at it. S0's demo screenshot settles it.
- **`quick` end to end** (clippy and nextest were not timed; nextest was not installed).
- **Timing of the real workspace.** Six crates build differently from one; pipelining may help or
  hurt. Record `--timings` in S0.
- **rmcp resources, `Json<T>` structured output and async tools**: read in source and tests, not run.
- **The Ubuntu Mono insertion snippet**: signature read, not compiled. The licence reading is mine.
- **Backend pitfalls beyond the README and changelog.** I found no NVIDIA-on-Windows issue in the
  wgpu 30 changelog, but did not search the issue tracker. I could read only the first quarter of
  that changelog.
- **The report that nightly enables the parallel front end by default from 2026-11-14** comes from a
  news summary, not a primary source; it is not relied on above.
- **Published cold-build numbers for this stack**: I found none that are current. The only figure
  was an anecdote in [egui#4481](https://github.com/emilk/egui/issues/4481) (about 10 s for eframe on
  an M3, wgpu adding about 10 s), seen in a search summary only. The measurements above replace it.
- **Alternatives to schemars** were not surveyed, because rmcp fixes the choice.

## 7. Further questions

- Should `cav` (CLI, MCP) and the GUI be one executable or two? Two keeps eframe out of the
  CLI's build and tests.
- `egui_dock` or `egui_tiles` for the docking layout of M13 §9.2.
- Whether to ship `dxcompiler.dll` or enable `static-dxc` if DX12 ever becomes the primary backend.
- For the owner: a Defender exclusion for the repo's `target/` directory (outside the repo, so
  theirs to decide).
