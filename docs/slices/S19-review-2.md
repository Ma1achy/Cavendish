# S19 — second review (the fixes of the first), 25 Nov 2026

Reviewed on `main` at `6f2265c` (the merge of #209) against `docs/slices/S19-review.md` and
`s18-the-editor`. I read the first review, §7.9 of `docs/slices/S19.md`, `docs/design/panels.md`
with its "As built" section, `docs/design/workspace.md` §0, and the diffs of PRs #201, #202, #204
and #209. I looked at every picture in `docs/slices/S19/` beside its twin in
`S19/before-review/`. I compared `docs/validation/report.json` with the one at the tag, case by
case, by script. I ran no test, application, window or server: the one `cargo xtask quick` I
started stopped in its build because the disk was full (see the foot). The control arms below are
reasoned from the code, not from a build with the fix taken out, and "passes" below means "as
§7.9 records", not "as I saw".

**Verdict.** Five of the six important findings are fixed, each with a test that fails when the
fix is reverted: I1, I2, I3, I5, I6. I4 is fixed for the native path and, by reading, for the
page; but the page's half has no test that can fail for its central step, and its bookkeeping has
one fault on a side path. That is the one important finding left, and it is small. Everything
else is minor.

## Important

**J1. I4 on the web: the hand-off of a held mesh to a worker is not held by any test, and the
lane records a mesh as sent when the page had nothing to send.**

*(a) The test cannot fail.* `wire::tests::a_held_mesh_travels_with_the_job_that_needs_it`
(`crates/cav-engine/src/forward/wire.rs:453`) holds the mesh in the process's store first
(`assets::hold("wired.stl", …)`, line 462), then compares `serve(&sent)` with `serve(&request)`.
Both read the mesh that is already held. `serve`'s `ASSETS` branch calls `assets::hold` again
(`wire.rs:143`), which returns at once for a hash it has. Take line 143 out and the test still
passes. `lane::tests::a_held_mesh_goes_to_each_worker_once` checks the list of hashes only, and
`platform/web.rs` is not built natively. `cargo xtask web` imports no mesh past 10,000 triangles.
So nothing in any tier shows that a worker which starts with an empty store can read the body
after the message: the property the first review's I4 asked of the page.
- Fix: in the wire test, do not hold first. Build the scene's hash with `assets::hash(&bytes)`;
  assert that `serve(&request)` answers with the body unread (its warning names the hash); then
  `serve(&with_assets(…))` reads it, and `serve(&request)` after that gives the same bits. The
  first assertion is the control. (The store is one static per process, so use bytes no other
  test holds.)

*(b) The fault.* `Lane::pump` (`crates/cav-app/src/lane.rs:236-239`) inserts every hash a job
needs into `slot.holds` when it posts. `platform/web.rs:438-449` then sends only the meshes
`assets::bytes` finds on the page, and sends the plain request when it finds none.
- Scenario: a page is given a scene that names a mesh it does not hold (a body added by `add
  bodies {… "Mesh": {"file": …, "hash": …}}`, a `#exec=` link, a journal from another session).
  The first job marks the hash as held by the worker, with nothing sent. The body reads as
  nothing, and its warning says "drop or import the file again". The user does so; the page now
  holds the mesh; the lane never sends it, because `holds` already has the hash. The view draws
  the body, the towers do not read it, until the worker fails and is started again.
- Fix: record in `holds` only what was sent. Either give `pump` a `held: &dyn Fn(&str) -> bool`
  (the page's `assets::held`), or have the page report back the hashes it sent. Test in `lane`:
  a job naming a hash the page lacks sends none and leaves `holds` empty; once held, the next job
  sends it. Control: today's code sends none the second time.

Neither part touches the default flow (import in a page, then read), which reads correctly as far
as the code shows.

## The first review's important findings, one by one

**I1, one mass rule: fixed.** `BodySpec::mass_in` and `mass_with` (`cav-sim/src/spec.rs:753-769`)
are the rule; the bundle (`bundle.rs:213`, `:374`), the inspector's Info (`inspector.rs:357`), the
Material card's row (`editors.rs:653`) and the Kind switch (`inspector.rs:615`) use it. I searched
every `.mass` read in `crates/`: the rest either pass the field to `ShapeSpec::tree` (which
ignores it when a material is set: `shape.rs:640`), or are in the branch for a body with no shape
(`probe.rs:162`, `read.rs:531`), or key a cache (`voxels.rs:88`). None reports a weight.
`source_mass_is_what_each_body_weighs` fails with the fix reverted (1 kg against 502.4 kg), and
its uniform arm holds the field to the bit.

**I2, the finest cell: fixed.** `probe::finest_cell` (`probe.rs:111-125`) follows the tree's
convention (a leaf's key is its first finest cell; `count` is per cell). The test fails with the
merged leaf reported (`cell.side == 0.05`). See minor 3 for what the test does not reach.

**I3, what the Probe counts: fixed.** A tree the view left out is made (`probe.rs:166-175`); a
slab floor is `−2πGρ` times the matter below less the matter above, which is right for the
infinite slab the scene's floor is; a half-space, the world and a body that cannot be made are
named. `probe-town.png` shows "not counted: the world" and "no body here". See minor 4.

**I4, centring and large meshes: fixed natively; J1 for the page.** One rule
(`Mesh::centred_on_box`) in `import::body`, `assets::hold` and `assets::mesh`. The lopsided-torus
test fails if `hold` centres on the centroid. The caps are stated and refused with numbers
(32 MiB a file, 500,000 triangles, 256 MiB a session, 64 MiB of dropped files; a page checks the
size before reading). A worker that fails has its `holds` cleared (`lane.rs:180`), which is the
only path that replaces a worker, and the test covers it.

**I5, the inspector's order: fixed.** Transform, Shape, Material (Mass for a point), Parts,
Motion, Lifetime, Info, a ⋮ on each, in `inspector::body` (`inspector.rs:470`), held by
`a_body_s_cards_come_in_the_design_s_order_each_with_a_menu`. The departures are written in
`panels.md` "As built": Mass for a point's card, Lifetime open, the fixed width, Readings as a
tab. `inspector-parts.png` reads well: "Part 1 · Add lead · Sphere", "Part 2 · Paint copper ·
Stroke", each with its ⋮. Two things the design drew are not built and not listed there: "drag a
letter to scrub" and Compare "under Advanced". Add a line for each, or build them in S20.

**I6, the tests: fixed.** Readings from the engine's values with the towers differing as the
control; every registered target of the tree and the inspector measured both ways, the tree's eye
and lock among them (`outliner.rs:362-363`); every pair of `theme::text_pairs` at 4.5:1, with
the old key letters as the control; the fold test walks the inspector's own stops; rows record
what `row()` paints. See minors 7 and 8.

## Validation report

116 of 116 against `s18-the-editor` (113 there). By script over both `report.json` files: no
case dropped; for the 113 shared cases no `tolerance`, `measured`, `status`, `unit`, `reference`
or `against` differs; three cases added, all `materials.*`. The I2 control is back at 1 % against
the case's own tolerance, as minor 3 of the first review asked.

**The shared-cell case at 0.99 of its tolerance: honest, with one cost.**
`materials.shared_cells_are_within_their_dipole_bound` measures |ΔΦ − two prisms| over the
first-order dipole bound Σ|Δρ|·v·h/8·|∂ΔΦ/∂x|, tolerance 1.01. The body is built so that every
shared cell is the worst case (half and half, the interface mid-cell) and every cell's error has
one sign. The error therefore equals the first-order term, and 0.99994 says so to 6e-5. The 1 %
allowance is reasoned before measuring ((h/r)² ≈ 1e-3 at r ≥ 1.5 m). The derivation is right
(excess mass Δρ·v/2 with its centroid h/4 off centre). The case fails upwards if any other error
above 1 % of the bound enters, and its own asserts fail it downwards (error > half the bound,
bound > 1e-9 rad, the swapped materials far outside). So it is a real check and not a tuned one.
The cost: the report's `worst` is now this case for good (margin 0.990), so the summary and the
error-against-tolerance plot no longer show which case is truly nearest its tolerance. See
minor 9.

## Minor

1. `assets::mesh` with no hash (`cav-sim/src/assets.rs:160`) centres a file's mesh on its box.
   No test reads a file through it: put `.centred()` back there and nothing fails. Add to
   `a_mesh_is_held_by_the_hash_of_its_bytes`: the lopsided STL written to the temporary
   directory, read with `mesh(path, None)`, its box's middle at the origin. The read-and-hold
   path with a matching hash is also untested.
2. `crates/cav-sim/src/lib.rs:17-18`: the crate's opening doc has its last sentence twice, the
   second copy with a BEL byte where "[`assets`]" should be ("[\x07ssets]"). Delete the second
   sentence.
3. `the_cell_is_the_finest_cell_however_the_tree_merged` (`probe.rs:331`): the 0.4 m box at
   0.05 m pitch is one 8³ root leaf at key 0, so `finest_cell`'s offset by the leaf's corner
   (`probe.rs:119-121`) is never other than zero. Use a 0.6 m box (12 cells, merged leaves away
   from the origin) and keep the "cell holds the point" assertion.
4. "the world" in `not_counted` (`probe.rs:205-207`) is held by no test, only by
   `probe-town.png`. One assertion with `scene.world` set.
5. "the console" uses `mass_in`, say `spec.rs:744`, `docs/ARCHITECTURE.md:168` and
   `docs/slices/S19.md:80`. It does not: `cav get bodies[n].mass` still answers the placeholder,
   and no command answers what a body weighs. Add the weight to a console answer, or strike the
   word in the three places.
6. `BodySpec.mass` keeps `exclusiveMinimum: 0` in the schema (`spec.rs:402`) while `validate`
   now takes 0 for a body of materials (`spec.rs:962`). A body saved with mass 0 cannot then be
   made uniform from the Material drop-down until its mass is set (refused in words). Say "0
   allowed with a material" in the schema's description, or have "none: uniform" set the mass to
   what the body weighed, as the Kind switch does.
7. `every_target_in_the_side_panels_is_big_enough` (`tests/editor/s19.rs:368`) tests against a
   literal 23.5, not `theme::HIT`, and measures a point mass, the scene and a tower: never a body
   of materials (the Parts card's ⋮, "+ Add part", the material drop-downs). Derive the number
   from `theme::HIT` and add `select body` of the materials demo body.
8. `folding_a_card_keeps_it_a_stop_in_its_place` (`s19.rs:419-420`): the last control compares a
   list with "inspector/motion" removed against one that has it, which is true by arithmetic.
   The assertion above it is the one that fails when a folded card stops registering, so the
   property is held; drop the control or say what it is.
9. The shared-cell case pins the report's `worst` at 0.990. Either measure the residual
   (|error − bound| / bound, about 6e-5, tolerance 1e-2: the same claim) with a line in the slice
   notes for the change, or say in `docs/validation/README.md` that this case sits at its
   tolerance by design and have the summary name the next one.
10. `materials.png`, `panels-scene.png`, `panels-tower.png`: when the scroll bar shows, the
    cards narrow by its 10 pt and the material drop-down cuts its density ("steel 7850 k…",
    "lead 11340 k…", "copper 8960 k…"; the mass row "2849.38 kg, from its m…"). The picture
    meant to show densities shows them cut. Reserve the bar's width always, so that a row is
    the same width with and without it.
11. A uniform body's Material drop-down reads "none: uniform, of …" (`editors.rs:857`,
    `panels-body.png`). "Uniform" says it.
12. `probe-town.png`: with nothing counted the card reads "Gravity 0 m/s² · Of the bodies",
    the caveat below in weak 11 pt. Where no body exists and something is not counted, show "—"
    for Gravity.
13. The Probe card covers the figure's top-left (about 300 × 290 pt), against REBUILD §6 ("the
    figure is never covered"). The reading belongs in the right panel (a Probe tab beside
    Readings) or the status line; one for S20's workspace.
14. `bundle::write` voxelises each body of materials a second time for `source_mass`
    (`bundle.rs:213`); the read has just made the same trees. Harmless for one bundle, a doubling
    for Gradar's runs of many bundles with large bodies. Pass the read's trees to `mass_with`.
15. `Engine::probe` makes any tree the view left out on every click, on the frame's thread,
    with no byte limit (`probe.rs:168`, `ShapeSpec::tree`). Keep the made tree for the scene, or
    name the body as not counted past a size.
16. A worker's store keeps each mesh's file bytes (`assets::hold`) though only the page ever
    sends them: up to 256 MiB a worker in wasm32. Keep the bytes on the page only.
17. Locks are cleared when the engine's count of loads changes (`shell.rs` `keep_locks`); an
    undo of a load brings the old scene back without changing that count, so a lock set in the
    new scene passes to the old one's body of the same name. Count the undo of a load too.
18. `plan/IDEAS.md:176-177` ("the probe's field leaves out … the floor … it says bodies") is
    stale beside line 187; delete it.
19. `Mesh::centred` (`cav-core/src/voxel/mesh.rs:213`) is now used only by a test's control.
    Say so in its doc, or move it to the test.
20. `docs/slices/S19.md:89-93` still describes the page's dropped files as "`files::web` … which
    `Mesh::read` asks"; §7.9 has the truth. One line there pointing down to §7.9.

## Pictures

Each "after" picture is better than its "before". `panels-body.png`: Position is the first
card; Kind is a choice; Mass is an edited number under Material. `inspector-parts.png`: the parts
are readable, and the mass (2849.38 kg) agrees with my arithmetic for the picture's body (steel
0.6 m cube 1695.6 kg, the lead ball replacing its cap of steel and adding the rest, the copper
stroke a few kilograms more). `probe.png`: a card of rows and a towers table in µrad; the cell's
mass (2.91 g) is 4 of 64 samples of a 0.03125 m cell of a 100 kg ball, as it should be; gravity
91.2 nm/s² agrees with GM/r² at 0.27 m. `panels-readings.png`: ΔΦ = δφ₁ − δφ₂ on every row;
towers 1 and 2, and 4 and 5, read alike, as the scene's symmetry says. The column mixes "rad" and
"mrad" row by row, which makes magnitudes hard to compare down the column: one unit a column
would read faster (S20, with right-aligned numbers).

## Docs and recipes

- `docs/ARCHITECTURE.md`: the new rows (what a body weighs, the Probe, import with assets) point
  at code that exists; I checked each name (`probe_card`, `mass_in`, `with_assets`, `holds`).
  Minor 5 is its one untruth.
- `docs/extending/a-tool-or-panel.md` step 5 names `part_menu`, `fields_menu`, `draw_rows`,
  `choice_row`, `inspector::body`, `combo_width`, `bounded`, `theme::HIT`, `text_pairs`: all
  exist with those names.
- `docs/extending/a-material.md`: the example now sets the body's material first. The same two
  lines, with a sphere for the cuboid, are what `the_inspector_keeps_its_width_whatever_is_shown`
  runs, so the recipe is held by a test; I could not run it or `cav exec` myself (no build: the
  disk).
- `docs/extending/a-shape.md`: the import paragraph matches `import.rs` and `assets.rs`,
  including "the box's middle whatever the triangle count".
- Crate boundaries: no `Cargo.toml` dependency added (`web-sys` gains the `File` feature).
  `cav_sim::assets` is process-wide state in a crate whose rule is to hold none; the exception is
  written at the top of `lib.rs` and in §7.9. It is defensible (input bytes, keyed by content),
  but it is the engine's kind of job; when S5 or S20 next touches it, move the store to
  `cav-engine` and pass a lookup into `ShapeKind::solid`.

## For judgement: what S20 will have to shrink

Not findings. S19 built to `panels.md`'s 24 pt; `docs/design/workspace.md` §0 has since withdrawn
it on desktop (rows 20 px, controls 18–20, icon buttons 20–22, a 4 px grid, text 11–12 px, fields
that fill their column, numbers monospace and right-aligned, no prose in the chrome). Most of it
hangs from few places:

- `theme::HIT = 24` (`theme.rs:21`): every box, tick, drop-down, ⋮, eye, lock and `+`; and
  `interact_size` set from it (`outliner.rs:408`, `panels.rs:995`). To 20.
- `outliner::ROW = 24` (`outliner.rs:178`), drawn at a 28 pt pitch; names at 13 pt
  (`outliner.rs:216`). To 20 and 11–12.
- The inspector: rows at a 28–32 pt pitch, card headers about 32 pt, 8 pt inner margins, 10 pt
  between cards; `INSPECTOR_WIDTH = 352` (`panels.rs:974`); number boxes 88 pt with centred
  text (`editors.rs:153`); a 24 pt reset slot kept on every row (`editors.rs:538-543`).
- The scene's Description: a box 120 pt wide and nine lines tall beside empty space
  (`panels-scene.png`); §0 asks it to fill its column.
- The tool strip's 40 pt buttons (`panels-body.png`); the " + " button padded to reach 24 pt
  (`outliner.rs:415`).
- Lifetime open at first (two rows of empty boxes on every body); the "more below: scroll"
  badge beside a scroll bar that now shows.
- Prose in the chrome: the tool's hint row ("click a point: the gravity there of the bodies (and
  a slab floor) …"), the timeline's ("one lane per tower · each bar is a shot …"), "No bodies:
  the surroundings are read."
- Tests that encode 24 pt: `every_target_in_the_side_panels_is_big_enough` (the literal 23.5,
  and its control "the menu bar's are smaller", which stops being true at 20); the windows made
  taller for 24 pt rows (`tests/editors.rs` 1400, `tests/editor.rs` 1100, `s19.rs` 1600), which
  can come back down.
- Words: `panels.md` "As built" ("Targets are 24 pt a side"), `a-tool-or-panel.md` step 5
  ("at least `theme::HIT` (24 pt)").

## What I ran

`cargo xtask quick`, once, in this review's own worktree and its own `target`, below normal
priority, four jobs. `fmt` passed. `clippy` failed after 138 s while building dependencies, with
"There is not enough space on the disk (os error 112)": drive C had between 3 and 6 GiB free of
466 while other sessions were building, and a debug build of this workspace takes more than 3 GiB.
No test ran. I removed the 189 MiB that the attempt had written and did not try again, so as not
to take the owner's running builds down with it. Nothing in this review rests on that run: the
findings are from the code, the pictures and the two `report.json` files.
