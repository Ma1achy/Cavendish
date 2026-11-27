# The workspace: viewports, their headers, the tool palette, transport and density

> **Status: requirement** (owner, 25 Nov 2026, after using S19's build). Companion to
> `docs/design/panels.md` and `docs/design/keyboard.md`; where this and `panels.md` disagree, this wins.
> Models: Blender (areas, viewport header, overlays popover, navigation gizmo), Photoshop (floating tool
> palette, options bar), matplotlib's `subplot_mosaic` (layouts as a small grid of labels), DAW and video
> transport bars.

## 0. Density (overrides `panels.md`)

A professional scientific desktop app: **dense and useful beats spacious**. Everything on screen is
functional; no explanatory prose in the chrome (hints live in tooltips, the `?` sheet and the status bar).
- Desktop rows 20 px, controls 18–20 px, icon buttons 20–22 px, 4 px spacing grid, 11–12 px text.
  `panels.md`'s 24 pt minimum is withdrawn for desktop; the 44 pt touch rule stays for the phone and
  tablet layout only.
- Contrast stays ≥ 4.5:1. Numbers monospace, right-aligned, with units.
- A field takes the width it is given: text and description boxes fill their column; no half-width boxes
  beside empty space. No button without an obvious action: each has an icon or a verb, a tooltip with its
  key, and a disabled state that says why.

## 1. Viewports as a mosaic

The centre is a **mosaic of viewports**, not "1, 2 or 4 panes".
- A layout is data, as in `matplotlib.pyplot.subplot_mosaic`: a small grid of labels, where a label that
  repeats spans cells — `"AAB;CDB"` is a wide viewport over two small ones beside a tall one. Row and
  column weights are part of the layout. Stored in `ui/layouts.ron`, in the scene's saved workspace, and
  editable (`layout "AAB;CDB"`).
- Any number of viewports (bounded by a stated cap, with a message at the cap). Splitters between them
  drag to resize; double-click a splitter to even it out.
- Each viewport: split right / split down, close, maximise (Ctrl+Space), **pop out** to its own OS window
  and dock back (floating in-page on the web), swap with another by dragging its header.
- Each viewport is one **view type**: Scene (3-D), Signal, Periodogram, Flight, Cadence, Readings… chosen
  from the first control of its header. Adding a view type is a recipe (`docs/extending/`).
- Keyboard: the mosaic is one big scope; arrows move between viewports in reading order; the order comes
  from the layout's labels, never from which are popped out (`keyboard.md` §9).

## 2. The viewport header

Replaces today's row of text chips ("point · iso · perspective · shaded · AION-10 · pane 1 of 4 …").
One compact row per viewport, icons with tooltips, left to right:

```
[▦ Scene ▾] [🎥 Persp ▾] [◐ ◑ ▣ ⬚] [Overlays ▾] ........................ [⤢] [⧉] [✕]
 view type   camera       shading     popover                          max  pop  close
```
- **View type** dropdown.
- **Camera** menu: perspective / orthographic; named views (top, front, right, iso…); camera mode (orbit,
  fly, follow <body>); frame selection (F), home; bookmarks.
- **Shading** as a segmented icon group: shaded, flat, x-ray, wireframe (paper as an export, not a mode).
- **Overlays** popover (checkboxes): grid, towers' arms, octree, LOD bands, sections, labels, legend; and
  the colour field (level, material, depth, partial fill…).
- Right side: maximise, pop out, close.
- **No status in the header.** Instrument preset, signal state ("within 5e-14 rad", "truncated"), selection
  and scene name go to the **status bar** at the foot; a warning shows as one coloured dot in the header
  with the text in its tooltip.
- A **navigation gizmo** (axis triad; click an axis to snap the view, drag to orbit) in each 3-D viewport's
  top-right corner, with zoom, pan and perspective/ortho buttons under it, as in Blender. It replaces the
  "Home" button.
- Plot viewports get the same header shape: series picker, axes (log/linear), export.

## 3. The tool palette

A **floating palette inside the active 3-D viewport** (Photoshop's toolbox, Blender's T panel): icon-only,
one column (two when short), draggable by its grip to any edge or corner of the viewport, remembered per
layout. It overlaps the canvas; it is not a column beside it. Tools with variants (Place ▸ sphere, cube,
mesh…) show a corner mark and open a flyout on hold. The active tool's settings (snap step, axis, numeric
entry) sit in a one-row **options bar** under the viewport header, shown only while they apply.

## 4. Top bar

One row: app menu (File Edit Add View Scene Array Help) · workspace tabs (Scene, Instrument, Analysis, +)
· command search (Ctrl+K) · right-aligned: scene name with its unsaved mark. Nothing else: the time
readout moves to the transport.

## 5. Transport (timeline controls)

Real transport buttons, as a media player's, at the timeline's left:

```
[⏮] [⏪] [▶/⏸] [⏩] [⏭]   [⟲ loop]   1× ▾     t 3.20 s / 12.00 s   shot 3 / 6
start step  play   step end              speed   editable time           editable
```
- Step moves one shot (Shift: one cadence cycle); hold to scrub. Speed: 0.1×–10× and "as fast as it runs".
- Space plays/pauses in the timeline scope (as today); the buttons carry their keys in tooltips.
- The one-line explanation of the lanes goes to a tooltip on the timeline's title.

## 6. Scene tree: groups and folders

The tree's built-in groups (Array, Bodies, Surroundings) stay; the user can add **folders** inside them
(New folder, drag rows in and out, nest, rename, fold). A folder has an eye and a lock that apply to its
children, and can be selected to move or duplicate everything in it. Folders are scene data (they save and
replay), not UI state.

## 7. How it is tested

- Mosaic: a layout string parses to the rectangles `subplot_mosaic` would give (controls: a non-rectangular
  label region and an unknown view type are refused with a message); splitter drags keep the weights' sum;
  more than four viewports render; pop-out and dock return the same layout; Tab order equals label order.
- Header: no text chip carries status; every header control has a tooltip with its key; the status bar
  shows the signal state the engine reports, bit for bit.
- Palette: its rectangle lies inside the viewport's canvas and moves with a drag; the canvas width does not
  change when it is shown (control: a docked column would change it).
- Transport: each button issues its command; step lands exactly on a shot time; speed scales playback.
- Density: row and control sizes read from the theme equal §0's; every text or description field's width
  equals its column's (control: a fixed-width field fails).
- Folders: create, nest, move, hide via the folder, save and reopen; undo restores each step.

## As built in S20

The brief, its requirements and what was measured are in `docs/slices/S20.md`; the pictures in
`docs/slices/S20/` (`cargo xtask demo S20`); how the code is laid out in `docs/ARCHITECTURE.md`
("The editor"). Per-panel detail is in `docs/design/panels.md` ("As built after S20's …").

What was built, by section:
- §0 Density: the sizes are the theme's, `crates/cav-app/src/theme.rs` (`HIT` 20, `ICON` 22,
  `GRID` 4, `ROW_GAP` 2, `HEADER` 22, text 12, 11 small, 12 monospace); the inspector is 320 pt
  wide with a label column of a third (`crates/cav-app/src/panels.rs`, `inspector.rs`).
- §1 The mosaic, up to twelve viewports: `crates/cav-sim/src/mosaic.rs` (the data; split, close,
  swap, resize, even), held by the engine as view state (`ViewCommand::Mosaic`, journalled and
  replayed, never undone); splitters, floating viewports and maximise in
  `crates/cav-app/src/panels.rs` and `shell.rs`. Console: `mosaic "AAB;CDB" [cols … rows …]`,
  `split right|down`, `close`, `swap a b`, `mosaic even`, `layout "AAB;CDB"`,
  `viewport close|swap|pop-out|dock [N]`.
- §2 The viewport header: `crates/cav-app/src/header.rs`; the navigation gizmo:
  `crates/cav-app/src/navgizmo.rs`; the status bar: `footer` in `panels.rs`.
  `viewport type|camera|overlays [N]` opens a header's menu from a script.
- §3 The tool palette, its flyouts and the options row: `crates/cav-app/src/tool_palette.rs`,
  `crates/cav-app/ui/tools.ron` (`tool <id> <variant>`, `tool-palette <anchor>`, `flyout`).
- §4 The top bar: `menu_bar` in `panels.rs`; a session's own layouts by `+` (`layout save`,
  `layout forget`), at most six.
- §5 The transport: `crates/cav-app/src/transport.rs` (`step <n>`, `speed <x>`,
  `end loop|stop|run-on`); the time, the duration and the shot are typed where they are read.
- §6 Folders: `crates/cav-sim/src/folders.rs`, `Command::Folder` (`folder
  new|move|rename|remove|duplicate`), drawn in `crates/cav-app/src/outliner.rs`.
- Not in this design, from the owner's rulings of 25 Nov 2026: the first-person camera
  (`crates/cav-app/src/camera.rs`, `first_person.rs`, `crates/cav-app/ui/camera.ron`), the walk
  camera and what it collides with (`walk.rs`, `occupancy.rs`), `noclip`; and a body's path drawn
  by its curvature (`crates/cav-render/src/path.rs`).

Where it departs from the design above:
- **A viewport's number is its label's alphabetical rank** among the labels present, not the
  reading order of first appearance: in `"BA"` viewport 1 is on the right. A swap then keeps
  every number, which the keyboard's order needs.
- **Pop-out is a floating frame inside the window**, natively as on the web. Windows of the
  system's are S22 (TASK-S19-02).
- **Swapping by dragging a header** is implemented but no test drives it by events (`swap` and
  "Swap with the next" are tested).
- **Layouts are not saved with the scene.** The built-in ones are `ui/layouts.ron`; a session's
  own are the window's and go with it. The palette's anchor is kept the same way.
- **The view types** are Scene, Signal, Periodogram, Flight and Cadence. Readings is a tab of
  the right panel, not a view type.
- **The header has no bookmarks, no follow mode, no log/linear axes, no series picker, no
  export, and no grid or labels toggles**: there are no commands for them yet. A plot's header
  has the view type and "show all". The camera's modes are orbit, fly and walk.
- **The tool palette is drawn in its viewport's own layer**, not as an `egui::Area`, so a
  floating viewport or a window covers it as it covers the picture.
- **"As fast as it runs" is left out of the speeds**: playing moves a playhead over a signal
  already computed, so it has no meaning. The speeds are 0.1× to 10×.
- **At the end** the transport loops, stops or runs on (the duration grows, one undo a run). The
  live rolling mode, bounded in memory, is S22 (TASK-S22-01).
- **No folders in the Surroundings** (three single items); only the Array and the Bodies take
  them. A folder's eye and lock are for folders of bodies.
- **A folder cannot be selected to move what it holds**: that needs multi-selection (S22,
  TASK-S19-07). It can be duplicated, renamed, hidden and locked.
- **Keyboard orbit and pan are not eased**: they are the keyboard spec's exact steps. Only the
  first-person camera's movement eases.
- **Moved to S22**: docking with tabs, Scale, Rotate, Measure, Slice and Paint, multi-selection,
  units typed with values, a user keymap, phones and tablets (the 44 pt touch rule with them).
