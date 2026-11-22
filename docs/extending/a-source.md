# Adding a source: something with mass that is not a point or a sphere

**Example to read:** `Slab` (and `AxialDisc`) in `crates/cav-core/src/forward.rs`, and how the
floor becomes one in `crates/cav-sim/src/read.rs` (`SourceId::Floor`, `sources`, `contribution`).

A source is what the flight integral sums against. It answers one question: at an instant, what
is the difference of its potential between the upper and the lower arm of each interferometer?

## In `cav-core`: the kernel

1. Implement `cav_core::forward::Source` for your type:

   ```rust
   fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, time: f64) -> [f64; 2];
   ```

   It returns `(V(upper arm) − V(lower arm)) / G` for each interferometer, in kilograms per metre:
   `[lower interferometer, upper interferometer]`. `tower` is the tower's base in the world's
   frame (metres); `heights` are the two launch points' heights above it; `lower` and `upper` are
   the two arms' rise above their own launch point; `time` is seconds on the scene's clock.
   **Form the difference without cancellation**: work with offsets from the tower, and write
   `1/a − 1/b` as `(b² − a²)/(ab(a + b))` as `Mass` does. It must be exactly zero when `lower`
   equals `upper`.
2. If the source is not smooth in time (it appears, stops, starts), list the instants in
   `Source::breaks`, as `Mass` does.
3. Test it in `forward.rs` against something known another way: a closed form, or a sum of point
   masses (`a_disc_on_the_axis_matches_rings_of_point_masses` is the pattern), with a control arm.

## In `cav-sim`: the scene

4. Put what describes it in the spec (`crates/cav-sim/src/spec.rs`): a new optional part like
   `floor`, or a new kind of body; doc, unit and range on every field; a check in `validate`.
5. Give it an identity: a variant of `SourceId` in `read.rs`, and its place in `sources()`. The
   order of `sources()` is the order contributions are summed in, which is part of what makes a
   signal the same bits every time: add to the end unless there is a reason.
6. Read it in `contribution()`: for each tower and each shot the tower takes, `reader.read_to(…)`
   at `shots.readout(tower, k)`; untaken shots are exactly zero. Ask `cancelled()` between shots.
   Report the estimate per tower, and a `Warning::about(tower, …)` for anything a person should
   know. If your source is the same for every shot (as the slab is), read it once per tower.
7. Tell the engine what its contribution depends on: `key()` in
   `crates/cav-engine/src/forward.rs` hashes the part of the spec a source is made from. A source
   whose key leaves something out is served stale from the cache.

## Tests that then hold it

- A null or a closed form as a validation case beside `validation_infinite_floor_null` in
  `read.rs` (recipe: [a-validation-case.md](a-validation-case.md)).
- `engine.cached_sum_equals_cold_run` and `dragging_one_body_recomputes_that_body_only`
  (`crates/cav-engine/tests/forward.rs`): add an edit of your source and see it recomputed, and
  only it.
- `engine.thread_count` (`read.rs`): add a scene with your source.
- If its cost can be set by a field (a resolution, a count), a bound and a test in
  `crates/cav-memtest/tests/spec_bounds.rs`.

## Things that go wrong

- Differencing two large potentials: the gradiometer's signal is the small difference of two
  nearly equal numbers, and a kernel that subtracts them loses it. `forward.precision_far` is the
  test that finds this.
- A source that allocates per call: `difference` is called millions of times a second.
- Forgetting step 7: everything works until the second edit.
