# Architecture

How Cavendish is put together, for someone who has not seen it before: the crates and what each
may depend on, the path a command takes to a pixel and to a bundle, and where each kind of thing
lives. The plan is `plan/REBUILD.md`; this file says what exists. A test holds the dependency
table below to the code (`xtask`: `the_crates_depend_as_the_plan_says`), and each slice keeps the
rest true.

Conventions used everywhere: SI units; the world's frame is x east, y north, z up, in metres;
times are seconds on the scene's clock; phases are radians; interferometer 1 is the lower one and
the gradiometer reads lower minus upper unless a scene says otherwise. British English.

## The crates

| Crate | What it is for | May depend on (workspace) | Must not |
|---|---|---|---|
| `cav-core` | Pure numbers: vectors, constants with their sources, the forward model (flight, arms, the propagation integral, the audited read), spectra (FFT, Lomb–Scargle, NUFFT), cadence closed forms, `.npy`; the voxel tree, shapes and `cover`, the voxeliser and its raster oracle, the cube kernel; the world volume's chunks, bricks and region-file bytes | nothing | know what a scene, an engine or a file format for scenes is; allocate with the length of a flight |
| `cav-sim` | Scenes as data: the spec and its schema, paths, motions, presets, the dev scenes and the library, the schedule, the read of a scene (contributions, signals), bundles; shapes of bodies and what a view draws of voxel bodies | `cav-core` | hold state between calls; draw; start threads that outlive a call |
| `cav-gpu` | The wgpu device and reading a texture back; on wasm it is handed the browser's device (`Gpu::from_parts`), not asked to make one | `cav-core` | know about scenes |
| `cav-engine` | The one API: commands, queries, replies; the journal, undo, the event log; the forward workers and what they keep; the world's streaming | `cav-core`, `cav-sim` | draw, or depend on anything that does; block a command on a computation |
| `cav-render` | The 3-D picture of a scene: camera, geometry, the wgpu renderer, PNG | `cav-core`, `cav-sim`, `cav-gpu` | depend on the engine (it draws a `SceneView`, which the engine makes) |
| `cav-plot` | Plots as display lists (signal, periodogram, cadence, flight), painted by egui, written as SVG and PDF, drawn to PNG with no window; figure presets, data and provenance; the paper drawing (a plan or a section of the scene) | `cav-core`, `cav-sim`, `cav-engine`, `cav-gpu`, `cav-render` | open a window; depend on `cav-app`, eframe or winit |
| `cav-app` | The window: panels, editors, timeline, panes; the editor (the keyboard model, actions, tools, layouts and windows, as data in `ui/*.ron`); the same application drawn headless for screenshots and tests; and as a web page (`cav-web`, wasm32) | all of the above | hold scene state of its own: it sends commands and reads replies |
| `cav` | The command line and the MCP server | all but `cav-app` | link the window (eframe, winit) when built on its own |
| `cav-validate` | Declaring validation cases (`case!`) and judging them | nothing | depend on anything: every crate's tests use it |
| `cav-memtest` | A counting allocator and the tests that hold memory to a bound | dev-dependencies only | be linked by anything but its own tests (it is the one crate with `unsafe`) |
| `xtask` | The local tiers: `quick`, `gpu`, `oracle`, `web`, `demo`, `validate`, `hooks` | nothing | gain a dependency: it must never add to the build |
| `oracle/` | A workspace of its own: v1 at tag `v1-final` and George's scripts as fixtures, holding the engine to both | `cav-core`, `cav-sim`, `cav-validate` | be compiled by `quick` |

Dev-dependencies are apart from this table: any crate's tests may use `cav-validate`.

"Does not link the window" is held by a test for `cav`, `cav-plot`, `cav-engine` and `cav-sim`
(`xtask`: `slow_nothing_that_runs_without_a_window_links_one`), and it means **built on its own**
(`cargo build -p cav`, `cargo install`): cargo then gives `egui-wgpu` only the features `cav` asks
for. In a build of the whole workspace cargo unifies features, so `egui-wgpu` is compiled once with
the window's feature, which `cav-app` asks for, and that copy is the one `cav` links; no code of
`cav` reaches it. A binary to hand to someone else is built with `-p cav`.

## From a command to a pixel, and to a bundle

```
a key, a drag, an editor ─┐
a console line ───────────┤                       ┌─ Query ──► Reply ──► panels, console text, MCP
cav exec / set / run ─────┼─► Command ─► Engine ──┤
an MCP tool ──────────────┤     │          │      └─ Engine::frame(pane) ─► SceneView ─► cav-render ─► pixels
a journal being replayed ─┘     │          │                              └► cav-plot (plot panes)
                                │          ├─ journal (every applied command; `journal save`, `cav replay`)
                         prepare: paths    ├─ history (the inverse edit of each edit, by bytes)
                         + validate        ├─ log (what was done, what was refused and why)
                                           └─ Forward ─► workers ─► contributions ─► Signal ─► Export ─► bundle, figure
```

1. **Everything is a command.** `cav_engine::Command` (`command.rs`) is plain data: `Load`, `Set`,
   `Insert`, `Remove`, `Undo`, `Redo`, `Seek`, `Select`, `View`, `Run`, `Compare`, `Export`. A
   button, a console line (`console.rs` parses text into the same commands and prints them back),
   `cav set` and MCP's `scene_set` all end in `Engine::apply`. There is no second way in.
2. **Apply.** `Engine::prepare` works out what the command will do without changing anything: for
   an edit, `cav_sim::paths` makes the edited scene and `SceneSpec::validate` accepts or refuses
   it, and the step that undoes it is recorded. A refusal changes nothing but the log. Then
   `perform` swaps the scene in, the command goes in the journal, and the forward stage is told.
3. **Nothing waits.** `Forward` (`forward.rs`) hands work to two workers through one-item slots: a
   newer request replaces one not begun; work for a scene that has changed is dropped part-way.
   Each source's contribution is kept by a hash of what it depends on, so moving one body
   recomputes that body. Until the new signal is whole the old one stays, marked stale. The
   application calls `Engine::poll` every frame; text clients call `Engine::ask`, which waits a
   stated time and says `refined`, `stale` or `computing`.
4. **The read.** `cav_sim::read` turns a scene into `Shots` and reads each source's contribution
   with `cav_core::forward::Reader::read_to`, which halves its panels until two rules agree and
   reports their difference as its error estimate. The signal is the contributions summed in the
   scene's order: the same bits however it was arrived at, on any number of threads.
   **Each tower flies its own instrument** (S3): `ArraySpec::instrument_of(n)` is the array's
   instrument with the tower's override (`array.towers[n].instrument`) in place; towers with
   the same instrument share one `Reader` (`read::Readers`, keyed by every field's bits). So each
   tower has its own shots, at its own cycle and 2T from the array's schedule, and `Shots`,
   `Signal` and `Contribution` hold them **ragged, tower after tower** (CSR: values flat, a tower's
   at `starts[d]..starts[d + 1]`). Go through the accessors (`count`, `times`, `readout`,
   `taken`, `channel`), never `tower × shots` arithmetic. Bodies move in the array's gravity
   whatever a tower's own. A scene may have no towers: it reads nothing, and every plot, pane,
   bundle and command says so rather than failing.
5. **To a pixel.** `Engine::frame(pane)` is the one description of what a pane shows. A scene pane
   goes to `cav_render::Renderer`; a plot pane to `cav_plot`, which builds a display list and
   paints it. The window (`cav-app`), `cav render` and MCP's `render_view` all ask the same frame.
6. **To a file.** `Command::Export` writes a scene, a journal, a bundle (`cav_sim::bundle`: the
   `(T_max, D)` arrays padded where a tower has fewer shots, with `mask` and `n_shots`, beside the
   `_flat` arrays and `tower_offsets` that hold every tower's shots as the engine does; the
   manifest lists each distinct instrument once and which each tower flies) or a
   figure (`cav-plot`, through an exporter the client installs, since the engine does not draw).
   Bundles and figures are written only from a refined signal. Replaying a journal never writes
   where the journal says: exports go into the replayer's own directory, or nowhere.

## The web build and its platform seams (S17)

The application also runs as a web page (`https://ma1achy.github.io/Cavendish/`): `cav-app`'s
binary `cav-web`, compiled for `wasm32-unknown-unknown` with WebGPU, built by `cargo xtask web
build` into `target/web/dist` and published by `.github/workflows/pages.yml` on every push to
`main` (`docs/slices/S17.md`). Everything above holds there; what differs is gathered in **one
module per seam**, chosen with `#[cfg(target_arch = "wasm32")]` where the module is declared, and
nowhere else in the code (`xtask`'s `only_the_seams_name_the_target` holds every crate's `src/` to
that):

| Seam | Native | Web |
|---|---|---|
| The device: `cav-gpu` `native.rs` / `web.rs` | made by `Gpu::new`; errors caught in scopes; pixels read back | eframe's, through `Gpu::from_parts`; a page cannot wait, so errors go to the browser's console and read-back is refused |
| Where the forward work runs: `cav-engine` `forward/threads.rs` / `forward/inline.rs` | two threads | two Web Workers running the same module: `poll` hands each job over as bytes (`forward/wire.rs`, bit-exact) and takes the answers, never computing; with no workers (Node.js) `poll` serves the same bytes itself for `POLL_BUDGET` (8 ms) a frame. Same slots, epochs, store and summing order, so the same signal |
| The world's chunk cache: `cav-engine` `world/cache_native.rs` / `world/cache_web.rs` | region files under a folder set by `set_world_cache` | none: chunks are made again when wanted again |
| Files: `cav-engine` `files/native.rs` / `files/web.rs` | written and read where named (`files::capture` is the test double: writes kept as a page keeps them) | kept as downloads, which the app hands to the browser after each frame (a link's behind a click); reads by path and folders (bundles) refused |
| The figure exporter's pixels: `cav-plot` `figure/studio.rs` / `studio_web.rs` | its own device | none: PNG and the renderer's view refused, vector figures made |
| Around each frame: `cav-app` `platform/native.rs` / `platform/web.rs` (`bin/cav-web.rs` calls `platform::start`) | nothing | eframe's `WebRunner` on the page's canvas, `#exec=` in the address (`address.rs`: capped, names made safe), downloads, the Web Workers, a panic's message on the page |
| The window: `cav-app` `window/native.rs` / `window/web.rs` (`main.rs` calls `window::main`) | eframe's native window, `--exec`, `--screenshot` | none: says so |

What stays the same without a seam: `web_time::Instant` is std's `Instant` natively and
`performance.now()` in a page, so shipped code reads the clock through it (std's panics on wasm);
`cav_sim::read::threads()` is 1 on the web (no environment, no parallelism), which takes the read's
one-thread path. The engine's numbers in wasm are held to native by `cargo xtask web`
(`cav-engine/tests/web_equivalence.rs`): every number the same bits, except a listed few labels
whose arithmetic goes through `sin`, `exp` and the like from another maths library
(`THROUGH_LIBM`), held within 1e-12. Code that writes a file the
person asked for goes through `cav_engine::files`; code that must not run in a page (threads,
`std::fs` by path, blocking on the GPU) belongs behind one of these seams, never behind a scattered
`cfg`.

## The editor: data, the keyboard, one path (S18)

The window's *what and where* is data, loaded and checked like scenes: `cav-app/ui/actions.ron`
(the seven big scopes in Tab order, the menus, every action with its console line, keys, scope and
when it can run), `ui/tools.ron` (the tool palette: the tools, and where a tool's flyout takes
its variants from), `ui/layouts.ron` (the layouts), and
`scenes/starters.ron` (what Add adds, in `cav-sim` so the CLI and MCP add the same). Code holds the
mechanisms: the router, the widgets, the gizmo's geometry.

Each frame the panels draw and *register* every control in a `FocusTree` (`keyboard.rs`):
`Shell::register(scope, id, name, kind, rect)`. The next frame's keys are routed over that tree by
`Keyboard::route`, in the fixed priority of `docs/design/keyboard.md` (a text field, an open
drop-down, global keys, the scope shortcuts from the innermost scope out, then Tab, Enter, Esc and
the arrows), into `Effect`s: an action runs its console line at once; a click, an adjustment or a
start of typing waits for its widget, which takes it as it is drawn (`take_click`, `take_adjust`).
The keyboard's state is UI only: never journalled, never undone. The inspector names fields from
the schema (`paths::Entry::label`: a `title` where one is given, else the name in words) and the
history list says each step in words (`dialogs::step_words`); the console's paths and lines stay
one hover away.

**The mosaic (S20).** The figure is a mosaic of up to twelve viewports
(`docs/design/workspace.md` §1). Where they are is a `cav_sim::mosaic::Mosaic` the engine holds:
view state, set by `ViewCommand::Mosaic` (journalled and replayed, never an edit, never undone;
`panes 1|2|4` are shorthands for `A`, `AB`, `AB;CD`). A viewport is known by its label: its
number is its label's rank in the alphabet among those present, and its camera and content go
with the label through every split, close and swap. The window lays the figure out from
`Mosaic::rects`, and keeps for itself only what is not the session's: a splitter's drag while it
lasts (one command when it ends), which viewport is maximised, and which float over the window
and where (`Shell::floating`: a floating viewport's place in the mosaic stays, empty, with a
dock button). The keyboard walks the viewports as stops of one scope, `viewport/mosaic`, in the
labels' order, so neither a swap nor a pop-out changes Tab's or the arrows' order.

**The viewport header, the gizmo and the status bar (S20).** Each viewport has one compact row
along its top (`cav-app/src/header.rs`, `docs/design/workspace.md` §2): the view type first
(`PaneContent::ALL`), then a scene's camera menu, shading group and overlays popover or a plot's
"show all", and at the right a warning dot, maximise, pop out and close. It is made from data
(`header::CONTROLS`; the menus' rows are actions of `ui/actions.ron`) and every control runs a
console line addressed to its own viewport (`show periodogram @3`). A header dragged onto
another viewport swaps the two (the engine's `swap`); a floating viewport's header is the bar it
is dragged by. The picture is drawn under it: `Layout::panes` is the viewport, `Layout::canvases`
the canvas, and pointer arithmetic is the canvas's. A scene's canvas has a navigation gizmo in
its top right corner (`cav-app/src/navgizmo.rs`): the view's own axes, an end clicked to look
from it (the named views), a drag to orbit by the view's own `Orbit` command, and zoom, pan and
projection buttons under it; it took the place of the Home button. No status is drawn in a
header: the status bar (`footer` in `cav-app/src/panels.rs`) says, in a fixed order, the
selection, the instrument (and the one compared with), the signal's state in the engine's words
(`estimate_text`), the forward stage, the counts of warnings and errors and the frame's time;
what each part says is in `Layout::status_bar`. The keyboard reaches the active viewport's
header as the scope `viewport/header`, its stops `CONTROLS` in that order whatever is shown.

**The tool palette, its flyouts and the options row (S20).** The tools of `ui/tools.ron` are a
palette that floats inside the canvas of one scene viewport, its host (`tool_palette::host`: the
active viewport if it shows the scene, else the first that does; none if none does, when the
tools' keys still take a tool): `cav-app/src/tool_palette.rs`, `docs/design/workspace.md` §3. It
is drawn over the picture in the viewport's own layer, so it changes no canvas's size and a
window or a floating viewport covers it as it covers the picture. Where it stands is one of
eight anchors of the canvas (`Anchor`), kept for each layout name in the window's state
(`Shell::tool_anchors`; a drag of its grip, or `tool-palette right`; never journalled);
`tool_palette::plan` is the layout as a pure function (one lane, two in a short canvas, clear of
the navigation gizmo). A tool with `variants` in the file has a flyout (a hold of 0.4 s, a
right-click, or its own keyboard stop) whose rows run `tool <id> <variant>`: Place's are the
starters. The tool's settings are one row under the host's header, shown only while the tool has
any (`Behaviour::has_options`) or a transform session is under way (`tool_palette::options_row`;
its rectangle is `Layout::options`, which `Layout::panel("options")` also gives): it takes its
height from that viewport's canvas, and with Select or Probe in hand it is not there. The
keyboard's `toolbar` scope is the tools in the file's order, a tool's variants as a drop-down
after it, and the snap switch last, disabled while not shown (`tool_palette::register`).

**The top bar and the transport (S20).** The top bar is one row (`menu_bar` in
`cav-app/src/panels.rs`, `docs/design/workspace.md` §4): the menus, the layouts as tabs (the
built-in ones of `ui/layouts.ron`, then the session's own: `+` runs `layout save`, which keeps
the mosaic and what each viewport shows as a new `LayoutData` in `Shell::session_layouts`,
window state, gone with the window), the command search, and at the right the scene's name with
its unsaved mark; the keyboard's breadcrumb stands before the name while navigation is on. What
it paints is in `Layout::top_bar`. The time is the transport's: the timeline's head row
(`cav-app/src/transport.rs`, the design's §5) has start, step back, play/pause, step forward and
end as icon buttons, each an action of `ui/actions.ron` (`seek start`, `step -1`, `play`,
`step 1`, `seek end`; their keys work in the `timeline` scope; `needs` says when each is
disabled and why), the speed (a drop-down whose rows are the actions of the menu `Scene/Speed`;
`Shell::speed` multiplies the frame's step in `Shell::advance`), what playing does at the end
(`transport::AtEnd`, `end loop|stop|run-on`, the rows of the menu `Scene/At the end`), and
`t [time] / [duration] s` and `shot [n] / count` as fields: a time or a shot's number typed
seeks there, and a duration typed is the `Set` of `duration` the Scene card's editor issues
(`editors::command_for`: one edit, one undo, refused by the engine as the card's is). At the end
of the timeline `Shell::advance` loops to 0, or stops with the playhead at exactly the duration,
or runs on (`Shell::run_on_past`): the duration grows by whole blocks of its length when the run
first reached the end, each extension a `Set` that continues the one before (the engine folds a
run of sets of one path into one step of undo whatever seeks come between), up to
`cav_sim::spec::limits::DURATION`, where playing stops and the log says so. Each extension is an
edit, so the signal is stale until the forward worker has computed it for the longer timeline. A step lands on a readout of the engine's own shots
(`Engine::shots`; `transport::place` finds the neighbours by bisection and makes no time of its
own), those of the selected tower, else the first's; with Shift it moves one mean cadence cycle;
a held button repeats on the keyboard's clock. `seek start` and `seek end` are the engine's
console's (read against the scene, journalled as the seek to that time); `step` and `speed` are
the window's, as `end` is. The keyboard's `timeline` scope is `transport::CONTROLS` in that
order, then the scrubber. What was drawn is in `Layout::transport`.

Every edit, from the viewport, a panel, a key or the palette, ends in one `Command` through
`Engine::apply`: the gizmo, the transform session and the inspector's field all issue the same
`Set` of a body's anchor; a run of them (a drag, a held key, a typed distance) is one undo whatever
comes between its steps (seeks while the scene plays, camera moves: the engine keeps the run's
path until another edit, Home included: the engine's private `run`), and a run that ends where it began leaves no step
(`History::continued`). A transform session holds every key that is not a global; a global ends it
cancelled first, and anything that takes the keyboard out of it (a click in a window, a drag in
the view) ends it with its move kept. Camera
moves are `ViewCommand`s, journalled and never undone, but for `Home`, whose `Edit::View` step
undo takes back. The engine keeps the scene as last opened or saved (`Engine::baseline`) for a
field's reset and the window's unsaved mark.

## Where each kind of thing lives

| Thing | Where | Recipe |
|---|---|---|
| A physical constant, with its source | `cav-core/src/constants.rs` | — |
| The forward model: arms, quadrature, the audited read, the `Source` trait | `cav-core/src/forward.rs` | [a source](extending/a-source.md) |
| Periodograms, the NUFFT | `cav-core/src/spectrum.rs` | — |
| Cadence closed forms, keyed random numbers | `cav-core/src/cadence.rs` | — |
| A field of a scene (its type, unit, range, doc string) | `cav-sim/src/spec.rs`, `motion.rs` | [a body or a motion](extending/a-body-or-a-motion.md) |
| Getting, setting and listing fields by path | `cav-sim/src/paths.rs` | — |
| How a scene is read into a signal | `cav-sim/src/read.rs` | [a source](extending/a-source.md) |
| What a schedule is predicted to do | `cav-sim/src/schedule.rs` | — |
| An instrument preset | `cav-sim/src/presets.rs` | — |
| A dev scene | `cav-sim/src/dev.rs` and `scenes/dev/*.ron` | [a dev scene](extending/a-dev-scene.md) |
| The bundle's arrays and manifest | `cav-sim/src/bundle.rs` | — |
| What a body weighs (its materials' or its `mass`: one rule for the bundle and the inspector; `get bodies[n].mass` answers the field) | `cav-sim/src/spec.rs` (`BodySpec::mass_in`, `mass_with`) | — |
| The Probe's reading of a point (the field of every body and a slab floor, the finest cell, its share of each tower's phase, what is not counted) | `cav-sim/src/probe.rs`; `probe x y z` in `cav-engine/src/console.rs`, `Engine::probe`; the right panel's Probe tab in `cav-app/src/outliner.rs` (`probe`), the point's mark in `panels.rs` (`probe_mark`) | — |
| A command, a query, a reply | `cav-engine/src/command.rs`; applied in `engine.rs`; its words in `console.rs` | [a command](extending/a-command.md) |
| The workers, the cache of contributions | `cav-engine/src/forward.rs` | — |
| Undo and the journal's budget | `cav-engine/src/history.rs`, `journal.rs` | — |
| A plot or a figure | `cav-plot` | [a plot](extending/a-plot.md) |
| What colours voxel bodies, their cubes and overlays in a view | `cav-sim/src/voxel_view.rs`; trees kept by `cav-engine/src/voxels.rs` | [a plot](extending/a-plot.md) |
| The colour maps of fields (sequential, diverging) | `cav-render/src/colour.rs` | — |
| A legend of a field or of the tree overlay's depths; a view of voxel bodies as a vector figure | `cav-plot/src/voxels.rs` (`legend`, `tree_legend`); pasted into `cav render` pictures by `cav/src/session.rs` | [a plot](extending/a-plot.md) |
| The scene as a technical drawing on paper (`figure paper`: a plan or a section, ink on cream at a scale 1:N, outlines of the world from coarse bricks, hatching, scale bar, north arrow, dimensions, title block; at most `MAX_PAPER_BRICKS` bricks, `MAX_PAPER_ITEMS` items, `MAX_PAPER_POINTS` points) | `cav-plot/src/paper.rs`; `PaperView` in `cav-engine/src/command.rs`; written by `paper` in `cav-plot/src/figure.rs` | [a plot](extending/a-plot.md) |
| Redrawing a figure from its data, in Python | `python/cavendish_figures` (matplotlib style sheets, `load`, `redraw`) | [a plot](extending/a-plot.md) |
| A panel, an editor (the inspector's sections, resets, lists), the timeline's lanes | `cav-app/src/panels.rs`, `editors.rs`, `timeline.rs` | [the window](extending/a-tool-or-panel.md) |
| The top bar (menus, layout tabs, search, the scene's name) and the transport (the timeline's buttons, what playing does at the end, speed, time, duration and shot; `docs/design/workspace.md` §4, §5) | `menu_bar` in `cav-app/src/panels.rs`; `cav-app/src/transport.rs` (`CONTROLS`, `place`, `step`, `AtEnd`), `Shell::advance` and `Shell::run_on_past` in `shell.rs`, its actions in `cav-app/ui/actions.ron` | [the window](extending/a-tool-or-panel.md) |
| The tool palette (its place, its flyouts) and a tool's options row (`docs/design/workspace.md` §3) | `cav-app/ui/tools.ron`, `cav-app/src/tools.rs` (`Variants`, `Behaviour::has_options`), `cav-app/src/tool_palette.rs` | [the window](extending/a-tool-or-panel.md) |
| A viewport's header and its controls; the navigation gizmo; the status bar (`docs/design/workspace.md` §2) | `cav-app/src/header.rs`, `cav-app/src/navgizmo.rs`, `footer` in `cav-app/src/panels.rs` | [a view type's header controls](extending/a-view-or-a-layout.md) |
| The scene tree (groups, icons, eye and lock, status dots), the Readings tab and the Probe tab; the inspector's header, cards and Info card (`docs/design/panels.md`); the panels' sizes (`cav-app/src/theme.rs`, `docs/design/workspace.md` §0) | `cav-app/src/outliner.rs`, `cav-app/src/inspector.rs` | [the window](extending/a-tool-or-panel.md) |
| An action: a menu item, a key, a palette entry; the big scopes' order | `cav-app/ui/actions.ron`, read by `cav-app/src/actions.rs`; drawn by `menus.rs`, `palette.rs`, `sheet.rs` | [the window](extending/a-tool-or-panel.md) |
| The keyboard model: scopes, modes, the ring, the router, held keys | `cav-app/src/keyboard.rs` (pure logic, tested as a state machine); the frame registers into it from `panels.rs` and `editors.rs` | [the window](extending/a-tool-or-panel.md) |
| A tool (Select, Move, Place, Probe), the gizmo, the transform session | `cav-app/ui/tools.ron`, `cav-app/src/tools.rs`; its pointer handling in `panels.rs` `viewport` | [the window](extending/a-tool-or-panel.md) |
| A layout of the window | `cav-app/ui/layouts.ron`, `cav-app/src/layouts.rs` | [the window](extending/a-tool-or-panel.md) |
| The mosaic of viewports: the grid of labels and its weights, split, close, swap, resize (pure data and functions) | `cav-sim/src/mosaic.rs` (`Mosaic`); held by the engine (`Engine::mosaic`, `ViewCommand::Mosaic`, views carried with their labels in `engine.rs` `set_mosaic`); `mosaic`, `split`, `close`, `swap` in `console.rs` | [a view or a layout](extending/a-view-or-a-layout.md) |
| A viewport as drawn: its picture or plot, its pointer, its right-click menu; the splitters; a floating viewport and its empty place | `cav-app/src/panels.rs` (`viewport`, `splitters`, `floating_viewport`, `figure`); the window's own state in `shell.rs` (`Shell::floating`, `Shell::maximised`; `viewport <close\|swap\|pop-out\|dock>`) | [a view or a layout](extending/a-view-or-a-layout.md) |
| The camera's arithmetic: frame, zoom toward the pointer, fly, look | `cav-app/src/camera.rs`; `ViewCommand::Camera` in `cav-engine/src/command.rs` | — |
| The editor's windows: history, new-scene wizard, Open, Save | `cav-app/src/dialogs.rs` | [the window](extending/a-tool-or-panel.md) |
| What Add adds (starters) | `scenes/starters.ron`, `cav-sim/src/starters.rs` (`add <name> [at x y z]`, `duplicate <path>`) | [the window](extending/a-tool-or-panel.md) |
| A CLI subcommand, an MCP tool | `cav/src/main.rs`, `mcp.rs` | [a command](extending/a-command.md) |
| A validation case | beside the test of what it checks, with `cav_validate::case!` | [a validation case](extending/a-validation-case.md) |
| A shape a body can be made of; `cover`; meshes | `cav-core/src/voxel/shape.rs`, `mesh.rs` | [a shape](extending/a-shape.md) |
| The voxel tree, its moments and cuts; the voxeliser and the raster oracle | `cav-core/src/voxel/tree.rs`, `voxeliser.rs` | — |
| The potential of a uniform cube (series, octants, prism); a voxel body as a source | `cav-core/src/voxel/kernel.rs` | — |
| A body made of voxels (`BodySpec.shape`) | `cav-sim/src/shape.rs`; read in `read.rs` (`voxel_body`) | [a voxel body](extending/a-voxel-body.md) |
| A material (density, colour, elastic and thermal properties); the library; a scene's own | `scenes/materials.ron`, `cav-sim/src/materials.rs` | [a material](extending/a-material.md) |
| Importing a mesh file as a body (OBJ, STL; held in the scene as `Triangles` up to 10,000 triangles, larger ones kept by the session by the hash of their bytes as `Mesh { hash }`, sent to a page's workers once); files dropped on the window | `cav-sim/src/import.rs`, `cav-sim/src/assets.rs`, `cav-engine/src/forward/wire.rs` (`with_assets`), `cav-app/src/lane.rs` (`holds`); `import` in `cav-engine/src/console.rs`, `Engine::keep_dropped`; the window in `cav-app/src/dialogs.rs` (`ImportForm`); a page's dropped bytes through `platform::dropped` | [a shape](extending/a-shape.md) |
| A body of several materials: parts added, painted and erased in order; a cell two materials share | `cav-core/src/voxel/layers.rs` (`voxelise_layers`), `VoxTree::mixed`; the parts in `cav-sim/src/shape.rs` (`PartSpec`) | [a material](extending/a-material.md) |
| Colour by field, overlays (leaves, a cut, the whole tree by depth), section planes (`Cutting`: a 2-D section square-on, a cut-away otherwise), the cubes and cells a view draws | `cav-sim/src/voxel_view.rs`; drawn in `cav-render/src/scene.rs` (`drawables_with`) with `colour.rs`; trees kept by `cav-engine/src/voxels.rs` | — |
| The world volume: chunks of 32³ cells with palettes, the far field's bricks, terrain; region files (bytes) | `cav-core/src/voxel/world.rs`, `region.rs` | [a world piece](extending/a-world-piece.md) |
| A scene's world: its spec, its pieces made solids, its read (leaves near, moments far, the bound) | `cav-sim/src/world.rs`; source `SourceId::World` in `read.rs` | [a world piece](extending/a-world-piece.md) |
| The clipmap (which bricks a camera draws at which level), the handover between levels (`stand_ins`: what was drawn stays until what replaces it is ready) and what a renderer is handed of the world (`WorldFrame`, with the streaming's queue as `waiting`) | `cav-sim/src/world/clipmap.rs` | — |
| Streaming the world around the cameras and towers; the region-file cache (natively) | `cav-engine/src/world.rs`, `world/cache_native.rs` (a seam: `cache_web.rs` keeps none) | — |
| Drawing the world: greedy meshes and GPU culling near, ray-marched bricks far; the kernels' CPU twins; GPU copies of what the streaming let go kept, retired, as stand-ins | `cav-render/src/world.rs`, `world_*.wgsl` | — |
| A bound on memory | `cav-memtest/tests/` | — |
| Something that differs in a browser page (threads, files, the device, waiting) | one module per seam, listed under "The web build and its platform seams" | [the web build](extending/the-web-build.md) |
| The web page itself, its build and its deployment | `crates/cav-app/web/` (`index.html`, `start.js`, `worker.js`), `cav-app/src/platform/web.rs`, `xtask/src/web.rs`, `.github/workflows/pages.yml` | [the web build](extending/the-web-build.md) |
| The page's worker lane: which Web Worker holds which job, when a failed one is started again, when the page runs a job itself (never one a worker failed on) | `cav-app/src/lane.rs` (a state machine with native tests; `platform/web.rs` turns its actions into `Worker` calls) | — |
| Leftover processes: an agent's `cargo xtask` records itself (id, start, heartbeat) under `target/strays/`; `strays --stop` stops only what such a run started | `xtask/src/strays.rs`, `scripts/strays.ps1`, `scripts/strays.test.ps1` | — |

## Rules that hold everywhere

- **One error budget, in radians.** Every read reports how far its last two rules disagreed; a
  signal carries the largest, per tower; a read that did not settle says so and is never shown as
  "within".
- **Everything sized by a spec, a view or a command has a bound**, counted before anything is
  reserved, and a test with the counting allocator (`docs/slices/S2.md` §3.1). A new allocation
  or loop whose size comes from such a value gets a row in that table and a test.
- **Determinism.** Bundles and figures are byte-identical from the same commands with the same
  binary on the same platform. What keeps it so: no sum across threads; no iteration of a
  `HashMap` into a number or a file; random numbers keyed by `(seed, stream, tower, shot)`, never
  drawn from a stream; no wall-clock time in any file; nothing asked of the machine when a file
  is written. The revision and the dirty flag a manifest and a figure's `.json` carry are the
  build's: `cav-engine`'s build script asks git once, when the engine is compiled, and they are
  compiled in (`unknown` when built outside a repository). So the same binary writes the same
  bytes whatever has happened to the source tree since and wherever it is run. `dirty` is about
  the engine's sources (`cav-core`, `cav-sim`, `cav-engine`, the built-in scenes, the
  workspace's manifest, lock file and toolchain); the cost is that the engine and what depends
  on it are compiled again after every commit (`docs/slices/S2.md` §7.12).
- **A test that guards a property has a control arm** that fails when the property is broken; a
  tolerance is written before it is measured and is never moved to pass (`CONTRIBUTING.md`).
