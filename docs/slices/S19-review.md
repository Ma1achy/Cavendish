# S19 — review (fresh eyes, 25 Nov 2026)

Reviewed on `main` at `9fcf231` (PRs #185–#188, #190, #195, #197; not tagged) against
`s18-the-editor`. I read the brief (`docs/slices/S19.md`), the requirement (`docs/design/panels.md`),
`docs/design/keyboard.md`, the owner's rulings in `plan/PROGRESS.md`, the skin note's §5, and the
diff. I looked at each picture in `docs/slices/S19/` beside `docs/slices/S18/editor-gizmo.png` and
`editor-instrument.png`. What I ran: `cargo xtask test material` (9 tests, all pass); the release
`cav-app` headless, once (`load town; tool probe; probe-at 0.5 0.5`, 180 s limit, own handle); the
release `cav exec` with the `a-material.md` recipe's lines (it works); and `cav run point` with a
steel box added, to measure the bundle finding below. I did not rerun `cargo xtask demo S19`,
because it writes into the main checkout. The binary I ran was built at 13:29, after the last code
change (the commit after it only ran `cargo fmt`).

**Validation report.** 115 of 115 against 113 at `s18-the-editor`. The two `materials.*` cases are
new, none was dropped, and no other case's tolerance or measured value changed (diffed from
`report.json`). The log's message cap went from 64 to 128 KiB. That cap is a schema limit, not a
tolerance, and §7.4 records the change.

**Verdict.** The physics of materials is sound: the layered voxeliser is a conservative walk that
equals its raster bit for bit, a cell two materials share keeps both masses, and undo returns the
same bits. The panels now read like an editor. Six things matter:

1. One consumer of a body's mass still reads the ignored `mass` field.
2. The Probe calls a merged block "the cell".
3. The Probe reads only part of the scene and does not say so.
4. On the web, real-sized meshes cannot be imported at all.
5. The inspector's structure departs from the design with no recorded call.
6. Several of the tests the design lists are missing, or have no working control.

## Important

**I1. The bundle's `source_mass` is the ignored `mass` field, not what a body of materials weighs.**
`crates/cav-sim/src/bundle.rs:372` writes `body.mass` for every body. With a material set, that
field is "not used" (`shape.rs` doc, brief §2), and an imported body carries a placeholder there
(`import.rs:130`).
- Measured: `cav run point --exec "add bodies {box 0.4 m, steel, mass 1.0}"` writes
  `source_mass = [100, 1]`. The tree weighs 502.4 kg (`probe 2 2 1` reports a 502.4 kg block).
- Effect: Gradar's labels are wrong for every body of a material. `scene_get` / `cav get` also show
  a mass that is not the body's.
- Fix: one `BodySpec::mass_in(&SceneSpec)` (the tree's mass when a material is set, else `mass`)
  used by the bundle, and a bundle test of a steel box against ρ·V. Control: a uniform body still
  writes its `mass`. While there: `spec.rs:921` still demands a positive `mass` that is never read.
  Either say so in the field's doc, or skip the check when a material is set.

**I2. The Probe's "cell" is whatever leaf holds the point, often a large merged block.**
`probe.rs:108-124` takes the first leaf whose box holds the point, at any level.
- Measured: `probe 2 2 1` at the middle of a 0.4 m steel box reports "cell of `box`: steel,
  5.024e2 kg, 0.4 m a side". Each tower's "cell's share of ΔΦ" is the whole box's (1.04 rad at
  tower 1).
- T1's test passes only because it probes a surface point, where leaves are finest
  (`probe.rs:228`).
- Effect: "this cell's share of each tower's phase" depends on how the tree merged.
- Fix: report the finest cell holding the point: its key at level 0, its mass from its tally or
  `mixed`, read as a one-cell body. If the merged leaf is kept, say "block of N³ cells".
- Test: an interior point's cell side equals the pitch. Control: the merged leaf's side does not.

**I3. The Probe reads part of the scene and does not say which part.**
- `Engine::probe` (`engine.rs:1767`) uses the view's trees. A voxel body that the view's byte
  budget left out is skipped silently (`probe.rs:136`), yet the card still says "g from the
  bodies".
- On a world scene the ground is not probed. On `town` a click on the terrain reads
  `g from the bodies: 0.000000e0 m/s²` and "no voxel cell here" (my run, `load town; tool probe`),
  while the header says "click a point of a body or the ground".
- Fix: build the probe's trees without the view's budget (as `read.rs` does), or list the bodies
  not counted. Say "the world, the floor and the room are not counted (S5)" in the card. Change
  "no voxel cell here" to "no body here".

**I4. On the web, real-sized meshes cannot be imported at all. Natively, a large mesh is placed by
a different point.**
- `import.rs:98-112` keeps a mesh in the scene only up to `MAX_INLINE_TRIANGLES` = 10,000.
  Beyond that, a page refuses the file ("simplify the mesh"). Most STL files found in the wild are
  larger than that.
- Natively the larger mesh is named by its path. `shape.rs:384` then centres it on its
  **centroid** (`Mesh::read(..).centred()`), whereas a held mesh is centred on its **box middle**
  (`import.rs:73-76`). So `import … at x y z` puts a different point at `(x, y, z)` depending on the
  triangle count. The docs (`import.rs:11-12`, `a-shape.md`) say "the middle of its box".
- Fix: centre path meshes on the box middle as well. Hold large meshes as a compact binary blob,
  or keep dropped bytes in the saved scene, instead of refusing them.
- Test: an asymmetric mesh of 10,001 triangles imported natively puts its box middle at `at`.
  Control: today's centroid does not.

**I5. The inspector's structure departs from `docs/design/panels.md`, and no call records it.**
In `panels-body.png` and `materials.png`:
- **Position is in a card called "Motion", while the motion's kind is a row of the Body card.**
  The design asks for Transform first, and Motion with its Kind. The cards fall out of the
  schema's sections (`editors.rs:237-243`), so `motion` (a choice) lands at top level and
  `motion.Static.position` in its own card. The thing a user edits most sits third.
- There is no Material card. Material is a row of Shape, and the drop-down gives no density
  (design: "Aluminium 2700 kg/m³").
- A body's parts are flattened into one run of rows: Mode, Material, At, Kind, Shape, Radius,
  Mode, Material… There is no header per part, and "Parts: List + −" is raw. In `materials.png`
  you cannot tell where the lead ball ends and the copper stroke begins.
- §7.1 says the cards each have a ⋮. Body, Scene, World, Array, Cadence and Instrument pass
  `None` (`editors.rs:274`, `panels.rs:977`, `999`, `1031`, `1049`, `1062`). Only the schema's
  sections (Shape, Motion, Timing) get one.
- Fix: a fixed card order per kind (Transform, then Shape, Material, Motion with its kind,
  Lifetime, Info), one card or sub-card per part with a title ("Part 1 · Add lead · sphere"),
  densities in the material list, and a ⋮ on every card. Or write the call into §7.1 and
  `panels.md`.

**I6. Several tests the design lists are missing or cannot fail.**
`docs/design/panels.md` § "How it is tested":
- *"Readings panel shows each tower's phase equal to the engine's reading, bit for bit"*: missing.
  `reading_words` (`tests/outliner.rs:78-82`) checks only that the text contains "rad" and ◐.
- *Hit targets ≥ 24 pt*: only the tree's rows are checked (`s19.rs:322-324`).
  - Measured on the pictures at 1 px = 1 pt: inspector checkboxes 14 pt; value boxes 20 pt tall;
    the tree's eye, lock and group `+` 20 × 20 (`outliner.rs:243`, `325-328`); the header's eye
    about 16 pt.
  - The rows meet 24 pt, but the targets do not, and nothing checks them.
- *Contrast ≥ 4.5:1*: only `TEXT` and `WEAK` on `PANEL` and `CARD` are checked (`s19.rs:315-321`).
  The tool strip's key letters are `WEAK` on the button ground: #8c8c8c on #3c3c3c is **3.3:1**
  (measured on `panels-scene.png`). So is a text field's hint (`filter`).
  Fix: check every (text token, ground token) pair the panels draw.
- *Tab order unchanged by folding*: `folding_a_card_leaves_the_tab_order` (`s19.rs:327-353`)
  compares the seven big scopes folded and unfolded. Folding a card cannot change those, so the
  test has no control that fails when the property breaks. It never checks the design's point:
  a folded card stays a stop, and the stops inside the inspector keep their order.
  Fix: walk the inspector's own stops (arrows) folded and unfolded, with a control that removes a
  folded card's registration.
- *No row carries a number*: `the_outliner_is_one_tree_of_named_rows` checks the `words` string
  given to `item()` (`tests/outliner.rs:124-126`). `row()` never draws that string, and towers pass
  the constant "tower". The control is S18's helper `tower_detail`, not the new code.
  Fix: record the text each row paints, and assert no digit beyond the label's own.

## Minor

1. `the_two_material_body` case (`voxel_bodies.rs:196-240`): both boxes' faces lie on cell faces,
   so the case has **no shared cell**. The mixed-cell mass reaches the read only through unit
   tests. Add a case whose interface lies mid-cell, against the sum of parts within the cell's
   dipole bound (see 2).
2. A shared cell is read as a uniform cube at the cell's centre (`tree.rs` `leaf_moments`). Its
   dipole error, up to (ρ_max − ρ_min)·v·h/4, is the same order as the existing partial-fill
   error, but CLAUDE.md asks for each approximation to be bounded or audited. Write one line of
   bound in §2.
3. I2's control (`voxel_bodies.rs:314-317`): the 1 % growth measured 1.2e-7 relative (§7.3), which
   already exceeds the case's own 1e-12 tolerance. The honest control is "exceeds the tolerance";
   the 1e-3 threshold is arbitrary. Moving to 10 % is not dishonest, but §7.3's "no sample
   changes" contradicts its own 1.2e-7.
4. `mesh.rs:84-101`: a triangle with a repeated corner (`[a, a, b]`) passes the closed-mesh check,
   because its own two edges pair up. Refuse zero-area triangles and test it.
5. `platform/web.rs:62`: `bytes_async()` reads the whole dropped file before the 64 MiB check
   (`engine.rs:300`). Check `file.size` first.
6. `shape.rs:520` lets through 255 parts of 255 distinct materials plus the body's own: 256
   materials, which `layers::check` then refuses at voxelising. Bound the palette in `validate`.
7. Hidden and locked state is keyed by label (`engine` `view.hidden`, `shell.rs:291`). A rename
   un-hides and unlocks the body. A lock carries over to another scene's body of the same name.
8. The Import window places the body with the **cube starter's** lift (`dialogs.rs:216` →
   `place_for("cube")`), not half the mesh's height. A mesh taller than the cube sinks into the
   floor.
9. The default "Widest side, m" is 1 (`dialogs.rs` `ImportForm::new`), so a 200 m building
   arrives at 1 m unless the user notices.
10. `ui/tools.ron:3-4` still says "Rotate, Scale, Paint, Measure, Slice and Probe come in S19".
11. `console.rs:201`: `colour` usage lists no `material`, though `colour material` works.
12. `docs/ARCHITECTURE.md:181` lists tools as "(Select, Move, Place)". `cav-sim/src/probe.rs` and
    the `probe` command are in no row.
13. `a-material.md`'s example `add bodies[1].shape.parts …` fails unless the body already has a
    material ("a body with parts has a material of its own"). Show the `set
    bodies[1].shape.material` line first. (With it, the recipe works: I ran it through `cav exec`.)
14. Brief §2 says the probe's field leaves out the leaf under the point. The code includes it,
    exactly, as a cube (`probe.rs:111`). The code is better; update the brief.
15. The floating strip covers the left pane's corner label ("iso") in the Instrument layout
    (`panels-scene.png`, x 262–290, y 80–90).
16. No visible scrollbar (design: "long inspectors scroll with a visible scrollbar"). The
    "more below: scroll" badge covers the last row's value (`panels-scene.png`: Gravity 9.81).
17. The scene's Info card sits between Scene and Array, not last (`panels.rs:1017`).
18. The header's icon is a gold dot for every kind (scene, tower, body), not the tree's type icon.
19. The root does not nest its groups: Array, Bodies and Surroundings are drawn at depth 0 beside
    the scene row (`outliner.rs:237`).
20. Library colours: tungsten #8a8f96, steel #7d838a and lead #5b6572 are hard to tell apart
    (`materials.png`: steel against lead), though the file says the colours are "chosen to tell the
    materials apart".
21. "Point radius" is shown, and editable, on voxel bodies, where validation demands 0.
22. Brief §7.1 says Readings is "beside the Inspector … docked in the Instrument and Analysis
    layouts" (design). It is a tab in every layout, which is fine; the brief should say so.

## UX, judged afresh (as a Unity / Godot / Blender user meeting it cold)

The step from S18 is large, and it is in the right direction. One tree with fold triangles,
icons, indent guides and hover-revealed eye and lock; tabs at the top of the right panel; a header
holding the name, the type and ⋮; cards; X/Y/Z coloured; units outside the boxes; checkboxes
instead of "none add"; Shots lost / Random seed; the status moved to the footer; the strip floating
over the view. A Godot user would recognise every piece of it. What still confuses on first
contact:

- **Where is Position?** It is the third card, under "Motion", while "Motion: Static" sits in the
  Body card above (I5). Every engine puts the transform first.
- **The panel width jumps.** Body selected, the inspector is 352 pt; scene, 317; tower, 325;
  Readings, 390 (the four S19 pictures). The view re-centres each time you click in the tree.
  egui grows the panel to its widest row (the material combo's "none: uniform, of its mass").
  Fix the content width to the panel, or clip the combo text.
- **The Probe card** is a monospace block of scientific notation laid over the view
  (`probe.png`). A small table would read faster: tower | distance | share of ΔΦ (mrad), with g in
  µGal or nm/s² beside an arrow. It also calls a whole block "the cell" (I2), and says nothing of
  the ground in a world scene (I3).
- **A body's parts** are unreadable as a flat list (I5). This is where materials become usable or
  not.
- **The "Shape ☑" box inside the Shape card** turns a voxel body into a point mass when unticked.
  That is a destructive switch styled like a minor option. Put it in the ⋮ ("Make a point mass").
- **Selecting while Readings is the tab** keeps Readings shown. Unity and Godot bring the
  Inspector forward on selection.
- **A click on the selected row deselects it** (`outliner.rs:357`, carried over from S18). In
  Unity, Godot and Blender it stays selected, and Ctrl+click toggles.
- The scene's description is one clipped line ("stress-1e10 scaled d"). Make it a multi-line field.
- Materials read as lower-case words with no swatch or density. Add a colour chip and "2700 kg/m³"
  in the list.

## How the slice holds to the plan

- Crate boundaries: no new dependency; `Cargo.toml` files unchanged; `layers` sits in `cav-core`,
  `materials` / `import` / `probe` in `cav-sim`, the drop store in `cav-engine`. All as the table
  says.
- Precision: nothing new on the GPU. The probe's central difference (h = 1e-5 side, within 8 sides)
  has rounding error near 1e-10 relative. That is fine for a reading, and the module says how it
  is made.
- Recipes: `a-tool-or-panel.md`'s tool steps match how Probe was added (`Behaviour::Probe`,
  `tools.ron`, both actions, icon, `figure`). `a-material.md` runs (minor 13). `a-shape.md`'s
  import paragraph matches the code, except for I4's centring.
- Tests changed for the redesign (§7.1, §7.5): each change is honest. Each one follows the
  layout's move (tabs, header menu, strip over the view, taller cards) and keeps its check. The
  fly change follows the third review's ruling. None weakens a property.
