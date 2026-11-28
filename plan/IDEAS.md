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
- **Turning bodies**: a body's tree is in the world's axes; an orientation (a quaternion in the spec) is S6/S7's, with the rigid socket of `reference/design/voxel.md` §7.
- **Mesh import beyond OBJ and STL** (glTF, PLY), and meshes held in the scene file rather than a path relative to where the program runs.
- **A crossing warning for voxel bodies**: a body made of voxels that passes through the atoms' path is finite (cube kernel) but not said; points and spheres are.
- **Quickhull for polyhedra**: the hull is found by trying every triple (cap 100 vertices).
- **`quick` under 10 s again**: its tests take 99 s of CPU on 12 cores, spread flat. Options: share one built scene or tree per test binary (many tests voxelise or load the same few scenes); move the counting-allocator tests' largest cases to the oracle tier and keep a small case in `quick`; a nextest profile that starts the slowest binaries first. S3.
- **Sections in the vector figure and a dragged plane**: `cav-plot`'s vector view draws a section's cubes but not its cells; the app moves the plane with a slider, not by dragging it in the pane. An oblique plane (any normal) needs the cells as polygons. From TASK-S3-20.
- **Leaf cubes in a section clipped exactly at the plane in perspective**: today a cut-away clips whole blocks to the plane; the tree overlay's boxes are clipped the same way, so a node's outline shows where the plane cut it, not where its block ends. From TASK-S3-20.
- **The shell in the oracle's convergence table** (`oracle/tests/bodies.rs` `solids()`): added to `cav-core`'s voxel tests only, so the oracle tier's numbers did not move under the S3 review. From TASK-S3-20.
- **Lint the oracle crate in `quick`** only when its files changed (a clippy fault reached main in #102). S3.
- **The validation report's margin for a band**: a case whose measured value must sit inside a band (`voxel.cube_to_point_order`) shows 99% where it is where it should be; report the distance to the nearer edge instead. S3.
- **The demo's missing pictures**: a sphere, cylinder and mesh beside the solids; the scaffold's periodogram over a long run read once and kept; the cylinder sweep as a `paper-column` figure. S3.

- **A mesh's file relative to its scene file** (S3's review, minor 13): today a `Mesh` body's
  `file` is resolved against the working directory. Resolving it against the scene file's
  directory needs the loaded scene to remember where it came from (a field the spec does not
  serialise), kept through save, `cav run`, replay and MCP.

- **The oracle under five minutes** (S3's second review): the two scaffold series cases are 256 s of
  the tier's 434 s, all in 6.0e9 leaf evaluations a case. Ways down, each a change to what the case
  measures: a far-field cut of the members' trees with its error audited against the case's 1e-3
  allowance, or fewer Chebyshev points in α with case (b) re-measured; or a slow oracle sub-tier.

## From S17 (in the browser)
- **A resumable contribution** on the web (not needed now the jobs run in Web Workers; kept for a page without workers): `read::contribution` cut at its 64-shot pieces with the tree kept between calls, so that one source never holds a frame (the voxel sphere's is about 5 s in Chrome on the build PC, one frame). Needs the spec and tree owned by the job, not borrowed. Next web task.
- **The engine in a Web Worker**: done for the forward stage's jobs (S17's review fixes, `forward/wire.rs`).
- **Cancelling a worker's job** on the web: a job for a scene that has since changed runs to its end. Terminating and restarting the worker when the epoch moves past its job (the module is compiled once, so a restart is an instantiation) would free it for the current scene during a drag of a voxel body.
- **The page's first frame after start-up** is 150–190 ms in every scene (room included): egui's first layout and WebGPU's pipelines. Warm them before the loading message goes, or spread them over frames.
- **The voxel view's trees on the page's thread**: an edit of a voxel body's mass rebuilds its tree on the page (30–40 ms for the dev scenes); a larger body would want it in the worker too.
- **wasm SIMD** (`-C target-feature=+simd128`, every WebGPU browser has it) for the cube kernel, measured against the scalar build.
- **Open and save files in the page**: a scene by `<input type=file>` or a drop on the canvas (`SceneSpec::from_ron`, the engine's inline load), a bundle as a `.zip` download.
- **`?scene=` and a "copy link" button** that writes the current view as `#exec=`, so a link shows exactly what was on screen.
- **PNG figures on the web** by an asynchronous read-back (`map_async` and a later frame), then the PNG written as a download.
- **A smoke test in the Pages workflow**: headless Chrome on the runner (with SwiftShader's WebGPU) loading the built page and failing on a console panic before deploying.
- **The paper drawing's materials**: in plan the road is the ground's height and is not drawn; outlines where the top cell's material changes (from a chunk's palette at the finer levels) would draw it, and each material could take its own hatch in a section (the usual concrete, brick and earth patterns).
- **A section's vertical exaggeration** (stated in the title block, `section y 64 exaggerated 10`): a kilometre of ground 25 m deep is a strip a few millimetres high at any honest scale.
- **A paper drawing cropped to a box** (`figure paper plan within <x0,y0,x1,y1>`), so a part of a big world is drawn at a scale that shows its buildings, and a section's cut line drawn on the plan.

## From S4 (big scenes)
- **Hierarchical-Z occlusion** for the near field (a depth pyramid from the last frame, a second cull pass): only frustum culling is on the GPU now.
- **Early depth for the ray-marched bricks**: the march writes `frag_depth`, which turns off early depth rejection, so the far field costs every brick's box in pixels (the flythrough's 99th percentile was 42–59 ms with the far field cut, and is 59–64 ms since the budget counts only visible bricks, S4's review I6; the draw's time correlates only 0.13–0.17 with the marched pixels the budget estimates, so the first measurement to make is the draw with the bricks off). Draw bricks near to far into a depth pre-pass of their boxes, or march in one full-screen pass over a brick table, or use conservative depth where the platform has it.
- **Streaming on worker threads that outlive a poll** natively (now: scoped threads within each step's 6 ms), and the world's chunks in the page's Web Workers.
- **The read through the chunk pyramid**: a coarse-to-fine walk over level-k bricks with mass moments (S5's bounded walk), so a whole 10¹⁰-cell world is read within its error budget, not a radius around each tower.
- **A tighter moment bound**: after TASK-S4-12 (each order's `Σ m s^l`, the fourth exact, the factor `l+1`) the bound on the octupole's remainder is 44–457 times the error. The hexadecapole in the expansion (fourth central moments, 15 numbers a chunk), with the bound from l = 5, would bring it within ten near and let the octupole left out break it at q = 0.1, R7's first control (`docs/slices/S4.md` §7.6); the arms' line rather than its nearest point would gain a little more.
- **DAG compression of static chunks** (the note §4.1) for the region files and the far field, measured against streaming bandwidth.
- **Smooth-surface draw style** for terrain (dual contouring of the chunk cells), visual only.

## From S4's review (minors left, `docs/slices/S4-review.md`)
- **A legend for `overlay bands`** (minor 11): `world-lod.png` has two flat colours and no key; the bands' legend should be made from what is drawn (level, cell size, metres from the camera), as the voxel view's is, in the pane's corner.
- **The world box beside its voxel body in one picture, and the far field in the near picture** (minor 12): the demo's `world-box.png` shows the world box alone, `world-near.png` no far rings; a scene with both (a voxel body beside the box) and a higher view of `town` would show what §4 promised.
- **Meshes freed when a chunk leaves level 0** (minor 14): a chunk that stays resident (a tower's, say) keeps its mesh on the GPU after the camera moves away; free meshes not drawn for some frames, under a cap on held but undrawn vertices.
- **The entry face with the eye inside a brick** (minor 15): the march starts at the eye, but its first cell's face comes from the negative `tmin`; shade the first cell by the ray's dominant axis instead (visual only, one cell).
- **`WorldGpu` per pane** (minor 16): one is shared by every pane, so its notes, `incomplete` and `marched` describe the last pane prepared, and two panes of different cameras evict each other's meshes. A copy per pane (sharing the brick store) would make the badges per pane.
- **Concurrent writers of one region file**: two processes caching the same region at once are not coordinated (a record a reader cannot decode is passed over); a lock file or one writer per cache folder would make it safe.
- **The seam at the tower bases** (S4's second review, minor 14): in `world-east-ortho.png` dark notches stand where the conservative coarse bricks rise above the fine meshes; bricks made from the finer chunks where they are resident (not from `cover`) would close it.
- **Prefetch along the camera's motion** (TASK-S4-27 left it): queue what the clipmap will want a second ahead, after what it wants now. Measure first: the queue is nearest first and stand-ins hide what is late; prefetch is thrown away when the camera turns.
- **The stress-1e10 flythrough's 99th percentile after TASK-S4-27**, with the far field off and on (the town trace's tail was the meshing, now 4–5 ms): on a machine where stress-1e10 may run.
- **Re-meshing on a neighbour's arrival only when the frame has time**: since TASK-S4-27 a chunk meshed in a later batch is meshed again more often (6 250 → 6 700 meshes over the town loop); an arrival only buries faces (the picture is the same), so it could wait for a frame with time over.
- **Coarse bricks that overreach** (measured in TASK-S4-26): a brick's cell is filled when a piece overlaps it at all (`fill` at side 1), a chunk's when its centre is inside, so a coarse brick can hold a slab one coarse cell thick where the chunks below it hold nothing. In `town` the level-1 brick (−9, 5, 0) holds 288 cells (one 20 cm layer beyond building 8's face at y = 32 m) over eight empty chunks; flying the test's loop, 14 drawn items go this way, rightly, when the finer level takes over. Sampling the brick's leaf cells at their centres (as the chunks are) would make the levels agree; it changes the far field's look and its tower-base seam (the line above), so measure both.
- **The cache's short files** (S4's second review, minor 9): a crash between `create_new` and the header leaves a short region file that every later session passes over; write the header to a temporary file and link it into place, or replace a short file over a minute old, and say "delete it to cache again". `load` re-opens passed-over regions, and the old `<hash>/` folders are orphaned.
- **The bytes cap's batch** (S4's second review, minor 10): a batch is made before `fits` is checked (about 400 KB a thread), the memtest's fixed 2 MiB margin does not scale with threads, and once full each refresh remakes one batch to throw it away; the cap keeps what came first, not what is nearest.
- **The axis rays' GPU arm** (S4's second review, minor 7): it checks only that the CPU and GPU agree; assert that each axis ray meets its row's first occupied cell, as the CPU test does.
- **The page's pictures after the review** (S4's second review, minor 12): `web-town.png` and `web-world-box.png` predate the badges and the world row; retake them with `cargo xtask demo S4` (its own server) when a local server is allowed.
- **A chunk meshed against a neighbour that is not drawn** (S4's third review, minor 6): at a mesh cap, a chunk re-meshed because a neighbour arrived buries its face toward that neighbour even when the neighbour itself did not fit and is not drawn, so the chunk is see-through on that side. Mesh against the chunks that are drawn, or keep the old mesh while the burying neighbour is undrawn.

## From S18
- **The read near a tower's atom path is slow**: a voxel body moved onto a tower's axis (a sphere dragged 2 m east of the default place lands on tower 2) made one reading take over two minutes in a debug build. Say so in the badge while it refines, and let S5's bounded walk cap it. S5.
- **Camera bookmarks in the scene** (`views`, `view save|goto`, `,` and `.`): every full `SceneSpec` builder gains a field. S19 (TASK-S19-05).
- **The Move tool's square moves in the view's plane without snapping**; snap it to the step in the plane. S19.
- **A Pass motion moves only its start** under the gizmo (the anchor); moving both ends needs a grouped edit. S19 (TASK-S19-07).
- **Reorder in lists and in the scene tree** (Ctrl+↑/↓) as one grouped edit. S19 (TASK-S19-07).
- **The palette learns**: recently used results first, and the ranking weights measured on what people type. S19.
- **Browser keys**: Ctrl+N, Ctrl+K, Ctrl+S, Ctrl+D, Ctrl+1–3 and F3 are the browser's own on the web; the page must `preventDefault` those it can and the sheet show the others' alternatives. The web check of S19.
- **A user keymap file** merged over `ui/actions.ron`'s keys, refused with the line and the reason when it clashes. S19.
- **The context menu in the view** (right-click a body): the tree's, for the body under the pointer. Since S18's review the right button flies (held) and the middle button pans, so the menu would be a right *click* without a drag. S19.

## From S18's first review (left, with why)

- **Home after an undo drops the redo stack** (review minor 2). It follows the owner's rule that Home is an undoable step (`docs/slices/S18.md` §7.3.6): undo a delete, press Home to look, and the delete cannot be redone. A separate one-level camera undo (the view's own, not the scene's) would keep both. For the owner's second look; not changed.
- **Browser-reserved keys on the sheet** (minor 14): the page cannot take Ctrl+N, Ctrl+1–3, Ctrl+Shift+P; the `?` sheet could mark them "not in the page" until S19's web check gives them alternatives. With the browser-keys item above, S19.
- **The history's console lines** are made only for the hovered row since the review (minor 8); `Engine::steps` still writes every inserted value whole for the console's `history`. If a scene of large meshes makes that slow, cache each step's line by its place in the history. Measure first.
- **A wizard scene undone** keeps the `*` of the scene it replaced until the next save or load (`Shell::untitled` is the window's, not the history's). Harmless; tie it to the history step if it confuses anyone.
- **Keyboard for the right-click menu** (Shift+F10, the Menu key): `context` opens the selection's row menu from the console; a key for it, with the menu walkable by the arrows, belongs with S19's keymap work.
- **The inspector's name column** is 92 points: "Half sequence T" fits, longer titles are cut with the full name on hover. A panel wider than its default could give the names the extra room. S19, with docking.

## From S18's third review (left, with why)

- **The wizard's tiles share icons** (minor 8): Cube and Box one, the four polyhedra a triangle,
  Oscillating mass, Pendulum and Walker one. Each starter could draw its own small picture from
  its shape (a projected outline, as the Place ghost does). Not done: a picture a tile is polish
  the wizard does not need to be used.

## From S19 (left, with why)

- **Progressive refinement** (preview while a drag goes on, the chosen tier on idle): the detail
  tiers' main use. With the tiers (TASK-S19-06), in S22.
- **Mixed cells hold mass, not moments**: a cell two materials share keeps the sum of their masses
  at its centre (`VoxTree::mixed`); S5's exact piece moments (TASK-S5-09) fill the same table with
  centroids and second moments. The table is where that revision goes.
- **The Paint tool's stroke as one undo** needs a `Set` inside an item just inserted to continue
  the `Insert`'s step (the engine's run keeps an item path as it keeps a field's). Designed in the
  S19 brief (§2), not built: TASK-S19-20.
- **Lead and steel look alike** in the material library's colours (two greys); a palette chosen
  for telling materials apart (and colour-blind-safe) would read better than "looks like the metal".
  (Done after S19's review: every two at least 15 apart in L*a*b*; colour-blind safety below.)
- **The label column follows the inspector's first width**, not the width it is dragged to (egui
  widens a panel to its content, and a column that followed it pushed it wider every frame): with
  docking (S22), take the dock area's own width, which egui_dock does not grow to content. (Since
  S19's review the panel no longer grows to its content, so the column could follow it now.)
- **A material's elastic and thermal numbers are read by nothing yet**: rigid dynamics (S7) and
  heat (S13) will; until then the inspector could fold them under "Advanced".
- **The Probe reads the world** (after S19's review it names it as not counted): S5's world
  octree gives its field at a point cheaply (moments far, leaves near), and its finest cell.
- **A body's identity apart from its label** (S19's review, minor 7): the eye and the lock follow a
  rename and a new scene clears them, but they are still keyed by name; an engine-assigned id kept
  through inserts, removals and reorders would let a lock survive a delete-and-undo too.
- **Colour-blind-safe library colours**: every two are now at least 15 apart in CIE76 L*a*b*;
  CIEDE2000 and a deuteranopia simulation would hold them to what more people see.
- **A large mesh in a saved scene**: today a later session needs the file read or dropped again;
  a scene folder with an `assets/` beside the RON (the bytes by hash) would make it whole.
- **The probe makes a left-out tree on every click** (S19's second review, minor 15): a body the
  view's byte budget left out is voxelised on the frame's thread each time `Engine::probe` is
  asked, with no byte limit. Keep the tree made for the scene (the engine's cache, a second
  budget), or name the body as not counted past a size.
- **A page's worker keeps each mesh file's bytes** (minor 16): `assets::hold` keeps the bytes for
  sending, but only the page ever sends; a worker needs the parsed mesh alone (up to 256 MiB a
  worker saved in wasm32). A `hold` that drops the bytes, used by `wire::serve`.
- **Locks across the undo of a load** (minor 17): locks are cleared when the engine's count of
  loads changes, and undoing a load brings the old scene back without changing it, so a lock set
  in the new scene passes to the old one's body of the same name. Count the undo of a load too,
  or key locks by the engine-assigned id above.
- **`cav_sim::assets` belongs to the engine** (the second review's note on crate boundaries): it is
  process-wide state in a crate whose rule is to hold none. When S5 or S20 next touches it, move
  the store to `cav-engine` and pass a lookup into `ShapeKind::solid`.
- **A fast arm in `quick` for each slow test** (TASK-S19-26): the 45 tests named `slow_` run only
  with the oracle tier. Where one guards a property a small scene would also show (the probe's
  field on a coarse ball, the wire's world job on one chunk), a cheap twin in `quick` would catch
  a break at the commit and not at the slice's end.

## From S20 (the workspace)

- **Closing a viewport into several neighbours** (TASK-S20-03): `Mosaic::close` lets one
  neighbour that shares a whole edge grow into the closed place, and refuses when none does (`A`
  in `AAB;CDB`, whose lower edge is C's and D's together). Letting the neighbours that tile one
  edge between them each grow would close anything; so would Blender's "join areas" by a drag.
- **Which neighbour grows** (TASK-S20-03): the left one first, then above, right, below, so a
  split down of a viewport with a neighbour to its left closes into that neighbour and not back
  into what was split. Remembering the split's parent in the window (not in the mosaic) would
  make close the exact inverse of split.
- **Home's undo step names a pane by number** (TASK-S20-03): `Edit::View { pane }` was made when
  panes never changed number. After a close or a typed mosaic renumbers the viewports, undoing
  a Home made before it puts the camera back in whichever viewport now has that number. Key the
  step by label.
- **The splitters' crossings; a test of the header's drag** (TASK-S20-03, TASK-S20-04): the
  2 px square where a column's gap crosses a row's is no handle: a four-way handle there would
  resize both at once. And the swap by dragging a viewport's header onto another is built
  (TASK-S20-04) but no test drives it by events; `swap <a> <b>` and "Swap with the next" are
  tested.
- **The flyout by one gesture; the chosen variant on the button** (TASK-S20-05): Photoshop
  takes a variant by press, slide and let go; here the hold opens the flyout and a second click
  chooses. And the Place button could draw the chosen starter's own icon (a cube, a sphere), so
  the palette says what a click will add without the options row.
- **The palette across viewports** (TASK-S20-05): it is dragged within its host's canvas only;
  dragging it into another scene viewport could make that one the host (today: click there).
  Its anchor is kept for each layout name, not for each viewport, and not saved with the
  workspace (nor is any window state yet).
- **A keyboard-opened flyout stays open after a click elsewhere** (TASK-S20-05): as the
  header's keyboard menus do (`Keyboard::popup` is closed by keys only). A pointer press should
  close any keyboard drop-down.
- **Eased orbit and pan from the keyboard** (TASK-S20-10): the task's title asks for "orbit and
  pan given the same per-frame smoothing". Not done: the keyboard's orbit, pan and zoom are the
  spec's discrete steps (1°, 2 % of the distance, with DAS/ARR repeat), each landing exactly, and
  the tests of the keyboard model read the view the frame after a press; mouse orbit is direct
  manipulation and stays 1:1. A glide could be added as an animation toward the stepped view
  (the command issued at once, the drawn camera easing to it with `camera::ease`), which keeps
  every step exact.
- **A page reserves Ctrl+W** (TASK-S20-10, for TASK-S19-11): in a browser, sprinting forward
  with Ctrl held is Ctrl+W, which closes the tab before the page hears it. There the double tap
  of W is the sprint; the keyboard lock API (full screen only) or another sprint key on the web
  would settle it.
- **A flight during play journals every frame** (TASK-S20-10): consecutive camera commands are
  one journal entry, but a seek between them (playing) breaks the run, as it does for an orbit
  drag during play. The journal's byte budget bounds it; coalescing "seek, camera, seek,
  camera" pairwise would shrink it.
- **Mend a mesh on import instead of refusing it** (shapes study §3.2–3.3, 27 Nov 2026): the
  importer refuses the Stanford bunny (open base), scans past 500,000 triangles or 32 MiB, and
  decimations that fold through themselves. The study's repair (weld, drop non-manifold edges,
  ear-clip holes, cut out and re-close self-crossings) and a decimation to a stated Hausdorff bound
  would let `import` take them, reporting what it changed.
- **The walker's terrain is the column's middle** (TASK-S20-11): the camera meets terrain as a
  column a cell, its top from the surface at the column's middle; the voxeliser asks every
  sample of the column, so with two or more samples a cell the two can differ by one cell where
  the surface crosses a cell boundary inside a column (a 10 cm lip in `town` at worst, which a
  step climbs). Asking the corner samples too would make them agree exactly.
- **A door in the room** (TASK-S20-11; since TASK-S20-15 the room keeps a camera in but never
  out, so the way in is open): one inside still cannot walk out. A `RoomSpec` has no door; one
  could be a view-side convenience (a gap the camera passes) or scene data.
- **Riding and being pushed** (TASK-S20-11): the camera is not carried by a body it stands on
  nor pushed by one that moves into it (it is lifted out upward). A platform that carries its
  rider needs the body's velocity at the playhead in `SceneOccupancy`.
- **The mode's mark in the canvas** (TASK-S20-11): only `noclip` and the speed are marked; a
  small "walk" or "fly" beside them would say which the double tap left you in.
- **First-person settings are the session's** (TASK-S20-11): `camera-set` is not saved; a user
  settings file (with the keymap, TASK-S20-02) would keep sensitivity and speeds.
- **Reverse-Z depth** (S20's review, minor 8): the first-person near plane of 2 cm with a far
  plane of 5000 m on forward-Z `Depth32Float` (`cav-render/src/camera.rs`, `renderer.rs`) gives
  about 3 to 7 cm of depth resolution at 100 to 150 m, which may be the speckle at `town`'s
  horizon in `S20/fly.png`. Reverse-Z (depth cleared to 0, compare `Greater`, the projection's
  near and far swapped) spreads float precision evenly in distance and would remove it; measure
  the speckle before and after.

## From S5

- **A cancellation-free closed-form difference** (TASK-S5-08): `Faceted::difference` subtracts
  two potentials, losing `R/gap` of their digits (1e-9 only to about 30 sizes for a gap of one
  size, `docs/slices/S5.md` §7.1). Each edge's `L(p₁) − L(p₂)` could be formed as one `log1p` of
  a ratio and each face's `ω` difference as one `atan2`, so that the closed form's difference
  holds as far as its potential does; measure against the moments' read before adopting.
- **Absolute moments carried exactly at even orders** (TASK-S5-11; measured moot in TASK-S5-27, §7.9:
  once the walk goes below the chunk, no node above the chunks is accepted on the town at 1e-4 or
  1e-6 rad, and a chunk's `A_4` is already exact; exact `A_6 … A_16` for the blocks read 2 to 7 %
  fewer leaves, under the 20 % asked): a node's `A_4` is exact for a
  chunk but carried upward by the triangle inequality, so on `town` it is a median 2.6 to 6.2 times
  the leaves' true `Σ|m|s⁴` above the chunks (worst 21 at level 4), and `A_16` up to 1.9×10⁴
  (`docs/slices/S5.md` §7.2). Carrying the `|m|`-weighted raw moments to the fourth order (35
  numbers: `Σ|m|`, `Σ|m|u`, `Σ|m|u⊗u`, … each shifts exactly by the binomial rules) would make
  `A_2` and `A_4` exact at every level, and odd orders could take Cauchy–Schwarz,
  `A_l ≤ √(A_{l−1}A_{l+1})`. The walk accepts a node at a distance that goes as the bound's
  `1/7`th power, so a 3× tighter `A_4` is about 15 % nearer: measure the walk's nodes with and
  without before paying 280 bytes a node. A chunk's own `A_6 … A_16` could likewise be exact
  (`cube_even_powers`, about 500 flops a leaf).
- **The engine keeps the world's octree and each tower's world reading** (TASK-S5-12): `cav-sim`
  keeps nothing between calls, so every world contribution, and every reading under the playhead
  (`read::reading` reads every source again), builds the octree anew (the town's: 0.8 s on 8
  threads) and walks and reads each tower again, though the world is static. The engine already
  keeps contributions apart; it could keep the world's `Octree` and the world contribution by the
  world's hash and compose a playhead reading from the kept one (any shot of a static world reads
  the same), so that scrubbing a town costs what it did in S4. Needs `read::reading` to take the
  kept world part. Since TASK-S5-22 each tower's local expansion (`world::local::Series`, keyed by
  the world's hash, the tower's line and span and the tolerance) is worth keeping too: a change of
  the instrument's flight alone needs only the near part and the series read again.
- **Levels below the chunk for opened chunks** (TASK-S5-12): on `town` the walk accepts a 3.2 m chunk
  only from about 90 m at 1e-2 rad (the bound is 100 to 900 times the error), so the cut is mostly
  opened chunks read leaf by leaf: 31 s a tower, and past 1e-3 the audited read exceeds the kernel's
  share and does not settle (`docs/slices/S5.md` §7.3). Opening a chunk into its own voxel tree's
  nodes, each with a summary, would let the walk go on below level 0 instead of reading every leaf;
  so would a tighter bound (the exact `A_4` above) or reading the towers in parallel. **Done in
  TASK-S5-27** (§7.9): the blocks below the chunk. What is left of the idea: reading the towers in
  parallel, and a cheaper near kernel for the leaves next to the arm, which are now the read's cost.
- **`cube_shift`'s closed form near a cube, error-bounded as the arms' difference is** (TASK-S5-24):
  a body's small motions are read as `V(d − shift) − V(d)`, and near a cube that is two closed-form
  potentials subtracted, which loses `ε·(the terms' size)/|change|`. Split the cube as
  `cube_difference` now does when that is above `DIFFERENCE_EPSILON` of the change's scale; measure
  on the scaffold (micrometre shifts) before adopting.
- **The analytic clipper's inner integral in closed form** (TASK-S5-09): a sphere's cut cell costs
  about 46 µs (24 × 24 cosine-mapped Gauss–Legendre nodes a piece, about 2,300 chords a cell),
  against 0.6 µs for its 64 samples. On each y piece the chord's limits are constants or
  `±√(A − y²)`, so `∫ y^q (z-moments) dy` is a sum of `∫ y^q (A − y²)^{k/2} dy`, `k ≤ 4`, in closed
  form (arcsines for odd `k`): one quadrature instead of two, about 24× fewer chords. Watch the
  cancellation of the arcsine terms near a tangency; hold it to C1 and to the 48-point rule.
- **`Second` near an arm** (TASK-S5-10, `docs/slices/S5.md` §7.7): its fallback, the `Centroid`
  cube within three cells, reads 1.5e-4 relative on C3's geometry where the bare quadrupole reads
  6.3e-6, and a fallback only within the convergence radius read 2.4e-4. The nearest cells' errors
  have both signs. A near read that is the piece's shape (PLIC + residual of
  `docs/research/surface-representation.md`, or `Pieces`) is the principled fix; measure it on C3's
  geometry against both.
- **Third moments read near the arm** (TASK-S5-09): the cut cells keep their pieces' third
  moments (they make the node moments exact), but `Second` reads only to the quadrupole. A `Third`
  rung (point + quadrupole + octupole, 12–21 ns) is 20–60× better again at 0.5 m on four of the
  five shapes of `docs/research/surface-representation.md` §1.1; so is PLIC + residual, which the
  same note measures best of all near the body.
- **Free-size boxes with the l ≤ 6 box series** (`docs/research/surface-representation.md` §4,
  noted by TASK-S5-09): the cube kernel already takes its series to order 16 where it converges, so
  octree blocks gain nothing; a body's interior as free rectangular boxes read by an l ≤ 6 box
  series (43 to 80 flops, checked to 40 digits there) is 4 to 33 times cheaper than octree blocks
  at 20 m and loses near the body. Worth it only with a walk that keeps the far advantage.
- **LOD radii by [O] §5's level-by-level protocol** (TASK-S5-14): the stored radii are the ledger
  walk's nearest acceptances, at which the rings err by 1 to 4 % of τ on town. The protocol of
  `docs/research/s5-world-octree.md` §5 (each level alone by its moments beyond ρ, leaf-exact
  nearer, the error worst over positions, with a safety factor) would give the smallest radii that
  hold τ, and a cheaper rings tier; about 80 whole-town leaf-exact reads, minutes in release. The
  errors are linear in the nodes, so one read per node per position, summed from the far end
  inwards, gives every ρ at once.
