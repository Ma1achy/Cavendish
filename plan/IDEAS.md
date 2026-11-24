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

## From the owner's brainstorm (22 Nov 2026), not yet in a slice
- **What is underground**: tunnels, caves, sewers, water mains with flow, tube trains; and the inverse problem, finding a void from the array. After S3 (voxel bodies) and S11 (baselines).
- **A building that lives**: lifts, tanks filling, air handling, occupancy by hour, a car park, a crowd leaving a stadium. S6 and S12.
- **Wind on things**: trees and buildings swaying, turbines, a thunderstorm. After S7's modal structure and S14's weather.
- **The slow Earth**: seasons, sea level, a landslide, magma under a volcano. After S14.
- **Live feeds**: open flight, ship, train-timetable and road-traffic data driving the scene around a site. After S6 and S14's replay.
- **A simulated year** as the long test set for Gradar and the cadence studies. S11 and S14.
- **Nulls for show**: the space station, Jupiter, a passing asteroid read zero, convincingly; useful for outreach. S14's `distant_nulls`.
- **Phase change, humidity, combustion** in the thermal model. After S13.

## From S2
- **Tab walks scopes, tab completion and a command palette (Ctrl+K)** over `COMMANDS` and the schema's paths (`cav_sim::paths::ls` gives the children to complete). Carried from S0; not done in S2. S3.
- **Copy and paste in the console, copy a value or a path from an editor** (eframe's clipboard feature). Carried from S0. S3.
- **MCP resources** (the scene spec, the library index, the last bundle) and structured tool output. Carried from S0. S11, with datasets.
- **Docking and saved layouts.** Carried from S0; the panels are still fixed. When panels multiply (S3's legends and debug overlays).
- **Addressing list items by label** (`bodies[label=tank].mass`) so that an agent's paths survive a removal; today indices are positions and the tools say so. S3.
- **`reset <path>`** to a field's default, and a mark on a field edited since the last run (M13 §9.4). S3.
- **`cav verify <bundle>`**: regenerate a bundle from its manifest's scene and compare bytes; `cli_run_writes_a_bundle_twice_the_same` does it by hand. S11 (M16-R5).
- **A cryptographic hash in the manifest** for each array (the digest of state is FNV-1a, fine for a test, not for a dataset). S11.
- **Per-tower instruments and per-tower jitter** (M15-R2): every tower shares one instrument and one schedule; only the offset is its own. S15.
- **The expected window for combined features** (jitter with gaps, loss with alternation): each is predicted as if alone and the realisation drawn beside it; the note's composition rule (loss multiplies the lines by 1 − p and adds a floor) could be shown. S11, with the sampling studies.
- **The periodic-source table** of the research note (§4.3): one simulated period of an oscillator serves any cadence without the forward model. Not built: the read in parallel made a 10,000-shot series 0.6 s. S11, if the cadence studies need millions of shots.
- **A pure-Rust `libm` for the forward model**, so that bundles are identical across Windows, Linux and macOS (the note, §5.8): a decision for the owner; it would change every golden once.
- **Vector export of the 3-D view**: a figure of a scene pane is a PNG, or an SVG/PDF holding that picture. S4's paper style.
- **A scene's memory at 1000 bodies × 1000 towers × long labels**: a `set` takes tens of milliseconds there (each edit is a trip of the whole spec through JSON); splice the smallest enclosing item instead (the note, §3.2). When a real scene is that large.
- **Figures of the comparison** (`compare aice`): a figure draws the scene's own instrument only; the panes draw both. A `compared` word on the `figure` line, and both on one page. S4.
- **Kerning in a figure's text**: outlines are placed by each glyph's own advance, so a PNG (egui's shaping) and the SVG/PDF of one figure can differ by a fraction of a point in a label's width. Read the fonts' `kern`/GPOS pairs, or lay figures' PNG text out from the same outlines. When a figure's labels are tight.
- **The Python helper's flight diagram** draws each shot as a bar: the arms' arcs are the forward model's. Export the arcs as data, or wait for the Python bindings. S11.
- **pytest for `python/`**: the tests are plain asserts with a runner of their own, because pytest is not installed on the build PC; they run under pytest unchanged. Install it when the Python side grows (S11).
- **One figure, several formats**: `fig.svg`, `fig.pdf` and `fig.png` share `fig.csv` and `fig.json`, and the `.json` names the format written last. A `formats` list in one command would say all three.
- **Say when the arms do not fit the tower.** `Instrument::reach()` is the height the highest arm gets to; `validate` checks only that Δr is less than the tower. A warning in the properties panel (not a refusal: AICE's own preset reaches 108.9 m in its 100 m) would have caught the 10 m tower. S3, with mixed towers.
- **`preset aice <n>`** on the console: today a larger kick number is `set array.instrument.kicks <n>` then `launch symmetric`, or `presets::aice_with_kicks` from code. S3.
- **`Arm::rise` at exactly 2T** returns 0 for an arm that landed before the kick and is still in the air at 2T (seen with a launch of 0.5 m/s and n = 3000: it is 11.8 m up). No preset does that and no Gauss node sits at 2T, but Simpson's and the trapezoid's last node does. Found while testing `Arm::apex`; left alone because it would change a number. S3.
- **The whole instrument on screen at 900 px**: with the kick number the properties panel's instrument block is one row longer than a 1440 × 900 window shows without scrolling (`launch_floor` and `sign` are below the fold). Collapse the schedule block by default, or two columns. S3.

- ~~The Python redraw's palette (`python/cavendish_figures/style.py` and its style sheet) is a copy of `PAPER_SERIES`; nothing holds the two together but the redraw taking colours from the record. A test that reads the Rust constant would (S2 review 28).~~ *Done in S3:* a Python test reads the Rust constants (`python/tests/test_figures.py`).

## From S3 (voxel bodies)
- **The scalar trick as the engine's fast path for Motion::Modes**: one `V(α)` per flight node and tower (Chebyshev in α), then every shot is a polynomial evaluation instead of every leaf again. The oracle's table method is that, test-side; the engine reads George's scaffold leaf by leaf, minutes for 22 Hz modes. With S5's modal synthesis (VALIDATION §4).
- **Voxelise on a worker**: the engine makes a body's tree on the thread that applies the command (milliseconds for the dev scenes, up to the voxeliser's work cap for any scene). The forward workers already build their own; the view's could arrive stale-not-blocked like the signal.
- **Draw only exposed faces** of voxel bodies (greedy faces per tree): today every drawn leaf is twelve triangles, capped at 20,000 cubes a view. S4's meshing.
- **Cache the per-leaf phase field** by body position and tower: colour phase reads every leaf again at every seek. Static bodies could keep it.
- **Turning bodies**: a body's tree is in the world's axes; an orientation (a quaternion in the spec) is S6/S7's, with the rigid socket of eference/design/voxel.md §7.
- **Mesh import beyond OBJ and STL** (glTF, PLY), and meshes held in the scene file rather than a path relative to where the program runs.
- **A crossing warning for voxel bodies**: a body made of voxels that passes through the atoms' path is finite (cube kernel) but not said; points and spheres are.
- **Quickhull for polyhedra**: the hull is found by trying every triple (cap 100 vertices).