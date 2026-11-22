# Ideas backlog

Improvements found while building that were not done in their slice. One line each: the idea, why it
would help, the slice it belongs to. The builder adds; the owner prunes.

## From S0
- **Tab walks scopes, with a focus ring and a breadcrumb** (the mock-up's keyboard model): the whole app usable without a pointer. S2, with the property editors.
- **Redraw only the pane whose view changed.** Panes are keyed on the engine's generation, so any command redraws all of them; a key per pane (its view, the scene, the selection) would make a four-pane orbit cost one pane. S3, when scenes get heavy.
- **Copy and paste in the console** (eframe's `clipboard` feature, which brings `arboard`), and copy of a value or a path from the properties. S2.
- **Tab completion and a command palette (Ctrl+K)** from `COMMANDS` and the schema's paths. S2.
- **Ubuntu Mono** in place of Hack, once the owner agrees to a font file and its licence in the repo.
- **A view cube, a north arrow and a scale bar** on the figure's header strip; the paper style's are S4.
- **Frame the selection (F)** and orbit about it. S1, when there is a body to select.
- **Tower tags on the figure with the live reading**, as the mock-up has. S1.
- **`cav app`** to start the window from the CLI, and `cav-app <scene.ron>`. S2.
- **Docking and saved layouts** (`egui_dock` or `egui_tiles`) with the three presets of M13 §9.2. When panels multiply.
- **MCP resources** (the scene spec, the library index, the last bundle) and structured tool output (`rmcp::Json`). S2.
- **Run the GPU tier on both backends** (`WGPU_BACKEND=dx12` as well as Vulkan) and compare pictures byte for byte. S5, with the kernels' parity tests.
- **A screenshot test in the real window**, started minimised with a virtual display or an offscreen surface, to close "nobody has looked at the window".
- **Per-tower lanes that can be dragged** to set an offset. S2 (M13-R14).
- **A draw-style legend generated from what is drawn** (tower colours, selection). S3, with colour by field.

## From S1
- **rustfft in place of the in-house FFT** once series reach millions of shots: measured 3 to 5 times faster on powers of two and 30 times on other lengths (`docs/slices/S1.md` §7.1). S11.
- **A float-float NUFFT periodogram on wgpu.** The research note measured single precision wrong by 4% of the peak at 1e5 shots; float-float phases would fix that. S5, only if the CPU path is too slow for the cadence designer.
- **A span on `Query::Signal`** and decimation for plots of very long series. S2.
- **The audit's estimate as a per-tower `error_estimate` in bundles** (REBUILD §3.3): the forward stage already has it per source. S2.
- **Adaptive panels instead of global halving** in the audited read: a close pass needs fine panels only near closest approach. S5.
- **An off-axis finite floor** (elliptic integrals or the prism kernel), so that the edge-term convergence is a scene and not a single tower on a disc's axis. S3.
- **`SignalReply` values as `Option<f64>`**, so that a half-space's infinite single-interferometer channels survive JSON. S2.
- **The source's own time resolution in the script-parity cases**: `fig2_10m` b, c, d sit at 82% to 92% of their tolerance; a convergence case like the 100 m one would say whose error that is.
