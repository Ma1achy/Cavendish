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
| `cav-core` | Pure numbers: vectors, constants with their sources, the forward model (flight, arms, the propagation integral, the audited read), spectra (FFT, Lomb–Scargle, NUFFT), cadence closed forms, `.npy` | nothing | know what a scene, an engine or a file format for scenes is; allocate with the length of a flight |
| `cav-sim` | Scenes as data: the spec and its schema, paths, motions, presets, the dev scenes and the library, the schedule, the read of a scene (contributions, signals), bundles | `cav-core` | hold state between calls; draw; start threads that outlive a call |
| `cav-gpu` | The wgpu device and reading a texture back | `cav-core` | know about scenes |
| `cav-engine` | The one API: commands, queries, replies; the journal, undo, the event log; the forward workers and what they keep | `cav-core`, `cav-sim` | draw, or depend on anything that does; block a command on a computation |
| `cav-render` | The 3-D picture of a scene: camera, geometry, the wgpu renderer, PNG | `cav-core`, `cav-sim`, `cav-gpu` | depend on the engine (it draws a `SceneView`, which the engine makes) |
| `cav-plot` | Plots as display lists (signal, periodogram, cadence, flight), painted by egui, written as SVG and PDF, drawn to PNG with no window; figure presets, data and provenance | `cav-core`, `cav-sim`, `cav-engine`, `cav-gpu`, `cav-render` | open a window; depend on `cav-app`, eframe or winit |
| `cav-app` | The window: panels, editors, timeline, panes; and the same application drawn headless for screenshots and tests | all of the above | hold scene state of its own: it sends commands and reads replies |
| `cav` | The command line and the MCP server | all but `cav-app` | link the window (eframe, winit) |
| `cav-validate` | Declaring validation cases (`case!`) and judging them | nothing | depend on anything: every crate's tests use it |
| `cav-memtest` | A counting allocator and the tests that hold memory to a bound | dev-dependencies only | be linked by anything but its own tests (it is the one crate with `unsafe`) |
| `xtask` | The local tiers: `quick`, `gpu`, `oracle`, `demo`, `validate`, `hooks` | nothing | gain a dependency: it must never add to the build |
| `oracle/` | A workspace of its own: v1 at tag `v1-final` and George's scripts as fixtures, holding the engine to both | `cav-core`, `cav-validate` | be compiled by `quick` |

Dev-dependencies are apart from this table: any crate's tests may use `cav-validate`.

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
4. **The read.** `cav_sim::read` turns a scene into `Shots` (the array's clock from the schedule,
   each tower's offset, which shots are taken) and reads each source's contribution with
   `cav_core::forward::Reader::read_to`, which halves its panels until two rules agree and reports
   their difference as its error estimate. The signal is the contributions summed in the scene's
   order: the same bits however it was arrived at, on any number of threads.
5. **To a pixel.** `Engine::frame(pane)` is the one description of what a pane shows. A scene pane
   goes to `cav_render::Renderer`; a plot pane to `cav_plot`, which builds a display list and
   paints it. The window (`cav-app`), `cav render` and MCP's `render_view` all ask the same frame.
6. **To a file.** `Command::Export` writes a scene, a journal, a bundle (`cav_sim::bundle`) or a
   figure (`cav-plot`, through an exporter the client installs, since the engine does not draw).
   Bundles and figures are written only from a refined signal. Replaying a journal never writes
   where the journal says: exports go into the replayer's own directory, or nowhere.

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
| A command, a query, a reply | `cav-engine/src/command.rs`; applied in `engine.rs`; its words in `console.rs` | [a command](extending/a-command.md) |
| The workers, the cache of contributions | `cav-engine/src/forward.rs` | — |
| Undo and the journal's budget | `cav-engine/src/history.rs`, `journal.rs` | — |
| A plot or a figure | `cav-plot` | [a plot](extending/a-plot.md) |
| A panel, an editor, the timeline | `cav-app/src/panels.rs`, `editors.rs`, `timeline.rs` | — |
| A CLI subcommand, an MCP tool | `cav/src/main.rs`, `mcp.rs` | [a command](extending/a-command.md) |
| A validation case | beside the test of what it checks, with `cav_validate::case!` | [a validation case](extending/a-validation-case.md) |
| A bound on memory | `cav-memtest/tests/` | — |

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
  drawn from a stream; no wall-clock time in any file.
- **A test that guards a property has a control arm** that fails when the property is broken; a
  tolerance is written before it is measured and is never moved to pass (`CONTRIBUTING.md`).
