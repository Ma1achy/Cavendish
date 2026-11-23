# Adding a kind of motion, or a field of a body

**Example to read:** `Motion::Linear` in `crates/cav-sim/src/motion.rs`, and the field `radius`
of `BodySpec` in `crates/cav-sim/src/spec.rs`.

A body is data (`BodySpec`: a label, a mass, a radius, a `Motion`, when it exists). A motion is a
closed form in time: the forward model asks where the body is at any instant of a flight, so a
motion must be a function of `t` alone, cheap, and smooth between the times it says it is not.

## A new kind of motion

1. **The variant.** In `crates/cav-sim/src/motion.rs`, add a variant to `enum Motion` with its
   fields. Every field gets a doc comment (it becomes the editor's hover text and the schema's
   description), a unit (`#[schemars(extend("x-unit" = "m"))]`: `m`, `s`, `Hz`, `°`, `1` for a pure
   number), and a range where it has one (`#[schemars(range(min = 0.0, max = 1.0e3))]`, and
   `extend("exclusiveMinimum" = 0.0)` when zero itself is not allowed). Only plain fields: RON
   reads nothing else reliably.
2. **Where it is.** In `Motion::position(t, gravity)`, the closed form: metres, in the world's
   frame, at `t` seconds on the scene's clock.
3. **Where it is not smooth.** In `Motion::breaks()`, the times at which the path has a kink (a
   pass starts and stops). The quadrature cuts its panels there; a kink that is not listed costs
   accuracy silently.
4. **What a drag moves.** In `Motion::anchor()`, the point a drag in a pane moves and the name of
   the field that holds it.
5. **Its frequency**, if it is periodic, in `Motion::frequency()`: the periodogram draws a comb
   at its harmonics.
6. **Its name** in `Motion::kind()` (the outliner's word) and in `Motion::KINDS`; **what it
   starts as** in `Motion::starter()` (what the editor's drop-down gives when this kind is
   chosen: valid as it stands).
7. **What is refused** in `Motion::validate()`: each check names the field
   (`{at}.YourKind.field: …`) so that a refusal says where.

Nothing else needs touching: paths, `ls`, the schema, the editors, the console's `set`, MCP's
`scene_set`, saving and loading all follow from the type.

## A new field of a body

1. Add it to `BodySpec` in `crates/cav-sim/src/spec.rs` with its doc, unit and range, and
   `#[serde(default)]` (with a default that changes nothing) so that scene files written before it
   still load.
2. Check it in `SceneSpec::validate` and add its limit to `spec::limits`.
3. Use it where it matters: `mass()` in `crates/cav-sim/src/read.rs` builds what the forward model
   sees of a body.
4. If the dev scene files now differ from their builders' text, rewrite them once:
   `CAV_REWRITE_SCENES=1 cargo nextest run -p cav-sim -E "test(the_s1_scene_files_are_their_builders)"`,
   and read the diff before committing it (it should be additions of the new field only).

## Tests that then hold it

- `motion::tests::motions_follow_their_formulas`: add the new kind, with points worked by hand.
- `motion::tests::every_kind_has_a_starter_that_is_valid` and
  `every_motion_names_the_point_a_drag_moves`: fail until steps 4 and 6 are done.
- `motion::tests::motions_that_describe_no_path_are_refused_with_the_field`: add a refusal.
- `paths::tests::every_leaf_has_a_unit_or_is_dimensionless_and_a_stage`: fails if a number has no
  unit.
- `paths::tests::every_path_gets_and_sets` and the app's `every_schema_field_has_a_widget` walk a
  scene with every kind in it (`everything()` in `paths.rs`): add a body of the new kind there.

## Things that go wrong

- A motion that depends on anything but `t` (a random number, the previous position) cannot be
  read at an arbitrary instant and breaks replay.
- Forgetting `breaks()`: the read still settles, with more halvings, or says it did not.
- A field without `x-unit`: `quick` fails in `cav-sim`, naming the path.
