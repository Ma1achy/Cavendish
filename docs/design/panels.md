# The side panels: outliner and inspector

> **Status: requirement** (owner, 25 Nov 2026: "the left and right side panels are awful UX wise").
> Written from the S18 pictures (`docs/slices/S18/editor-gizmo.png`, `editor-instrument.png`).
> Models: Godot's Scene dock and Inspector, Unity's Hierarchy and Inspector, Blender's Outliner and
> Properties. Keeps `docs/design/keyboard.md` (scopes, Tab order) and principle 11 (panels from data).

## What is wrong now

Left panel (outliner):
1. It is not a tree. Four flat lists under headings (Sources, Array, Surroundings, Run); nothing folds,
   nothing nests, no icons say what a row is.
2. It mixes the scene with results: tower rows carry phase readings ("-1.286 rad"), body rows carry
   "100 kg · static". A hierarchy should name things; numbers belong in a readings view.
3. Adding is a lone "Add ›" under Sources only; towers and surroundings have none.
4. "Run" status and a help paragraph sit at the foot; the rest of the column is empty space.
5. The tool strip is a second column beside it, so two narrow columns eat the left edge.

Right panel (inspector):
1. No header: the body's name is just the first field; no icon, no type, no menu. "remove this body" is a
   loose button in the middle of the fields.
2. The value column wanders: top fields start at one x, the Shape and Motion fields at another; widths differ;
   Position is three bare boxes with no x, y, z.
3. Optional things read as code: "Appears none add", "Vanishes none add", "Jitter none add" ×4.
4. Derived values ("Now at 2.000, 2.000, 0.250 m") and help paragraphs are mixed in with editable fields.
5. Raw names and values remain: "Loss 0", "Seed 0", preset/compare as rows of toggle buttons.
6. Sections are thin text lines, not cards; nothing groups visually; it is long and clipped.

## The design

### Outliner — one tree (Godot Scene dock, Blender Outliner)
```
┌ Scene ───────────────── [+] [⌕] ┐
│ ▾ ◈ point                        │   root: the scene
│   ▾ ⊞ Array  AION-10             │   group; + on hover adds a tower
│       ┃ tower 1            👁 ● │   ● = reading status dot (settled / stale / truncated)
│       ┃ tower 2            👁 ● │
│   ▾ ◇ Bodies                     │   group; + on hover opens the Add menu here
│       ● mass               👁 🔒 │   type icon: point, sphere, cube, mesh, voxel …
│       ● sphere             👁 🔒 │
│   ▸ ▭ Surroundings               │   floor, room, world
└──────────────────────────────────┘
```
- Disclosure triangles, indent guides, a type icon per row; groups fold (state kept per scene, UI-only).
- Row right side, shown on hover or when set: visibility (eye) and lock. Hidden bodies still read unless
  "excluded" is chosen (a separate, explicit property).
- No numbers in rows. One status dot per tower (colour + tooltip); readings live in a **Readings panel**
  (table: tower, phase, ΔΦ, state) docked in the Instrument and Analysis layouts.
- Header: "+" opens the Add menu into the selected group; ⌕ filters rows by name.
- Ctrl/Shift multi-select; drag to reorder; double-click or F2 renames; right-click menu as today.
- Run status moves to the footer status bar; the help paragraph goes (empty-state guidance and tooltips stay).

### Tool strip — inside the viewport (Blender, Unity scene toolbar)
A floating vertical bar overlaid at the viewport's left edge, not a panel column. Snap and its step sit in
the tool-options header beside the tool's hint. Frees a column of width.

### Inspector — component cards (Unity Inspector, Godot Inspector)
```
┌ ● sphere            Sphere · static   [👁] [⋮] ┐   header: icon, editable name, type, menu
├ ▾ Transform                              [⋮] ┤   ⋮: reset, copy, paste
│  Position   X [ 2.000 ] Y [ 2.000 ] Z [ 0.250 ] m│   axis letters coloured, drag a letter to scrub
├ ▾ Shape                                  [⋮] ┤
│  Kind       [ Sphere              ▾ ]          │
│  Radius     [ 0.25        ] m                  │
│  Pitch      [ 0.03125     ] m                  │
├ ▾ Material                               [⋮] ┤
│  Material   [ Aluminium  2700 kg/m³  ▾ ]       │
│  Mass       100 kg  (from material and volume) │
├ ▾ Motion                                 [⋮] ┤
│  Kind       [ Static              ▾ ]          │
├ ▸ Lifetime  always present                [⋮] ┤   optional fields behind a checkbox, not "none add"
├ ▸ Info                                         ┤   read-only, muted: now at, volume, cells
└────────────────────────────────────────────────┘
```
- Header carries name, type and a ⋮ menu (duplicate, delete, reset all, copy, paste). No loose buttons in fields.
- Every card uses one grid: a fixed label column (40% of the panel, min 96 pt), values fill the rest, one
  field height, units as a muted suffix outside the box. The value x is the same on every row of the panel.
- Optional properties (appears, vanishes, jitter, gaps, window…) show as a checkbox plus their fields when on,
  or as an "+ Add effect" list on the Cadence card.
- Read-only derived values live in an Info card, formatted with units. No help paragraphs: tooltips on labels.
- Scene selected: Scene card (name, description, duration), Array card (preset as a dropdown; compare under
  "Advanced"), Cadence card ("Shot loss", "Random seed", effects list), Instrument card.
- Long inspectors scroll with a visible scrollbar; cards remember fold state per type.

### Panel chrome
- Panel titles as tabs (ready for S19's docking). 8 pt spacing grid; rows at least 24 pt; text contrast at
  least 4.5:1 against its background (from the theme tokens). Proportional font for labels, monospace only
  for numbers.

## How it is tested
- Outliner: the tree's structure for `point`, `town`, `world-box` (groups, children, fold); no row carries a
  number (control: today's rows do); status dot per tower matches the reading's state.
- Inspector: every editable row's value rectangle starts at the same x (from the layout recorder; control: a
  card with a wrong label width fails); the header menu holds delete and duplicate; optional properties are
  checkbox-gated; no help text inside the panel.
- Readings panel shows each tower's phase equal to the engine's reading, bit for bit.
- Hit targets ≥ 24 pt and text contrast ≥ 4.5:1 checked from the theme (control: a dimmed token fails).
- Keyboard: Tab order unchanged by folding (a folded card is still a stop); all the above reachable by keys.

## As built after S19's review (25 Nov 2026)

The inspector follows the drawing above, card for card, with these calls (each worth a second
look):
- **A body's cards** are Transform (its motion's anchor: a static body's Position), Shape (Kind:
  point mass or shape (voxels), an explicit choice; then the shape's Solid, size, pitch and
  samples, or a point's radius), Material (with its colour and density in the drop-down, and the
  mass the materials give), Parts (a body of materials: each part under a header "Part 1 · Add
  lead · Sphere" with a ⋮ to move it up or down or remove it, and "+ Add part"), Motion (Kind,
  then its numbers), Lifetime (appears and vanishes behind their boxes), Info. Every card has a ⋮.
- **A point mass's Material card is called Mass**: it has no material, only its mass.
- **Lifetime is open at first** (until S20: see below), not folded as drawn: S18's E6 asks every field's editor to be in
  sight, and a card folded at first hides two.
- **The inspector's width is fixed** (352 pt at first, still draggable): its contents are drawn
  in a child of the panel's width and cut to it, so it does not jump with the selection; rows fit
  that width (drop-downs cut their words).
- **Targets are 24 pt a side** (20 since S20: see below) in both panels (rows, boxes, ticks, drop-downs, eye, lock, +, ⋮),
  and every text colour is held to 4.5:1 on every ground it is drawn on (`theme::text_pairs`).
- **Not built, and not drawn as built** (S19's second review): "drag a letter to scrub" (a
  number is dragged in its box, not by its axis letter) and Compare "under Advanced" (it stays
  where S18 put it). Both are S20's, with the dense panels (`docs/design/workspace.md`).
- **Readings** is a tab in every layout (not docked in two): a tab is one click from the Inspector
  wherever you are.

## As built after S20's density pass (26 Nov 2026)

`docs/design/workspace.md` §0 overrides this document's sizes on desktop (the owner's ruling of
25 Nov 2026); what the panels are is unchanged. What is different from "As built after S19's
review" above:
- **Targets are 20 pt a side** (`theme::HIT`), not 24: rows, boxes, ticks, drop-downs, eye, lock,
  `+`, ⋮. Icon buttons are 22 (`theme::ICON`), the grid is 4 (`theme::GRID`), text is 12 pt (11
  small, 12 monospace). Tree rows touch (a pitch of 20); the inspector's rows are 2 pt apart (a
  pitch of 22), card headers 22, margins and the gap between cards 4. Every size a panel uses is
  the theme's.
- **The inspector is 320 pt wide** at first (352 before), its label column a third of it (40 %
  before). A number is monospace and right-aligned in a box that fills its column, its unit a
  muted suffix in a place of its own, so the boxes of a card are one width; a drop-down and a text
  field fill the column too. The reset keeps its place (a target) at the row's end.
- **The scene's Description** is a box as wide as its card under its name, three lines tall and
  growing, not a 120 pt box beside empty space.
- **The scroll bar's width is always kept free**, so a row is as wide with the bar as without
  (the second review's minor 10). The "more below: scroll" badge is gone: the solid bar says it.
- **Lifetime is folded at first** on a body that is always there (its title says so), as first
  drawn; a body with a time has it open. The tests that ask for every field's editor open it.
- **A uniform body's Material reads "Uniform"** (minor 11).
- **The Probe reads into the right panel**: a third tab, Probe, beside Inspector and Readings,
  which comes forward when a point is probed (`tab probe`); only the point's mark is in the view
  (minor 13). **Readings** and the Probe's towers have one unit a column, in its heading, chosen
  from the column's largest magnitude; numbers monospace, right-aligned.
- **No sentences in the chrome**: a tool's hint is the tooltip of its name in the options row,
  the timeline's caption is its title's tooltip (the head keeps the shot count and 2T), and what
  an empty scene means is the Bodies group's tooltip.

## As built after S20's top bar and transport (26 Nov 2026)

`docs/design/workspace.md` §4 and §5. What is different from the above:
- **The top bar holds no time and no brand.** Menus, the layouts' tabs with a `+` (the present
  arrangement kept as a new tab for the session), the search box (Ctrl+K), and at the right the
  scene's name with ` *` while it has unsaved edits. The breadcrumb stands before the name while
  keyboard navigation is on.
- **The timeline's head is a transport**: start, step back, play/pause, step forward, end as
  icon buttons (`[` and `]` step a shot in the timeline, Shift a cadence cycle, Alt the ends;
  Space plays anywhere), the speed (0.1× to 10×), `t [time] / duration s` and `shot [n] / count`,
  the time and the shot typed to seek. The "play" and "rewind" word buttons are gone. A step is
  to a readout of the engine's shots (the selected tower's, else tower 1's). The shots' count
  and 2T stand at the row's right while there is room, and in the title's tooltip always.
- "As fast as it runs" (the design's §5) is not offered: playing moves a playhead over a signal
  already computed, so there is no run to be as fast as.

## As built after S20's viewport header (26 Nov 2026)

`docs/design/workspace.md` §2, the owner's ruling. What is different from the above:
- **The header strip above the figure is gone**, with its text chips and its "View ›" popover.
  Each viewport has its own header row (`cav-app/src/header.rs`): view type, camera menu, shading
  group, overlays popover (a plot: show all), then a warning dot, maximise, pop out, close. The
  camera's target, angles and distance are read-only rows at the foot of the camera menu.
- **Home and Restore are off the figure.** A scene's canvas has the navigation gizmo in its top
  right corner (`cav-app/src/navgizmo.rs`); the Home key and View ▸ Home stay, and maximise and
  restore are the header's button (Ctrl+Space).
- **Status is the status bar's**: selection, instrument (and comparison), the signal's state,
  the forward stage, warnings and errors, the frame's time, the console. Its "Space play" hint is
  gone (the key is on the `?` sheet and the Scene menu).
- The tool strip stood under the header of the viewport in the figure's corner until the
  floating palette replaced it (below).

## As built after S20's tool palette (26 Nov 2026)

`docs/design/workspace.md` §3, the owner's ruling ("the tool palette must overlap the viewport
like Photoshop or Blender"). What is different from the above:
- **The tool strip is the tool palette** (`cav-app/src/tool_palette.rs`): icon-only buttons of
  `theme::ICON` with a grip at its head, inside the canvas of the active scene viewport (a plot
  active: the first viewport that shows the scene; none: no palette, the keys still take a
  tool). Dragged by its grip it snaps to one of eight anchors of the canvas (corners and edge
  middles), remembered for each layout; at a top or bottom middle it lies along the edge; a
  short canvas gives it two columns; it stands clear of the navigation gizmo. The key's letter
  in each button's corner is gone (it is in the tooltip). It changes no canvas's size.
- **Flyouts.** A tool with variants (`variants` in `ui/tools.ron`; Place: the starters) has a
  corner mark; a hold of 0.4 s or a right-click opens its variants beside the palette, and
  choosing one takes the tool with it (`tool place cube`). A plain click only takes the tool.
- **The options panel above the figure is gone.** The tool's options are one row under the
  palette's viewport's header, there only while they apply: Move, the snap switch with its
  step; Place, what it places and the ground's height; a transform session, its numbers. With
  Select or Probe in hand there is no row. The tool's name is no longer written there (the
  palette shows which is lit); snap stays an action (S, Edit ▸ Snap to the step) when its
  switch is not shown.
