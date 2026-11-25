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
