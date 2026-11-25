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
