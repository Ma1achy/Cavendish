# Adding a validation case

**Example to read:** `validation_offset_shifts_one_tower_only` in `crates/cav-sim/src/read.rs`
(case `timing.offset_shifts_one_tower`), and `docs/validation/README.md`.

A validation case compares the engine with something known (a closed form, v1, one of George's
scripts, a recording, or the engine against itself where two results must be identical) and
records the measured error beside a tolerance. It is an ordinary test, so `quick` runs it; and it
is a row of `docs/validation/report.md`, so a reader can see what the engine is held to.

1. **Write the criterion first**, in the slice brief (`docs/slices/<slice>.md`): what is compared
   with what, the tolerance, and where the tolerance comes from (the rounding of two orders of
   the same arithmetic; a truncation error with its order; the standard error of a Monte Carlo).
   Then measure. If it misses, do not move the number: find out why and write that down.
2. **Declare it beside the test of the thing it checks**, in that crate's test module:

   ```rust
   cav_validate::case! {
       test: validation_my_case,            // validation_gpu_… if it needs the adapter
       name: "subsystem.my_case",           // lower case, unique: the row's name for ever
       subsystem: "subsystem",              // the report groups by it
       slice: "S2",
       reference: ClosedForm,               // or V1Oracle, Script, Paper, Recording, Exact
       against: "what exactly it is compared with, and what the number is, in one sentence",
       unit: Relative,                      // or Radians, Metres, Pixels, Count
       tolerance: 1e-12,
       // order: (2.0, 0.2),                // a convergence case: expected order, ± allowed
       measure: {
           // … compute …
           // the control arm: the same measure with the property broken must be far outside
           assert!(broken > 1e-3, "{broken:e}");
           cav_validate::Measured::Error(worst)   // or Measured::Convergence(vec![(step, error), …])
       },
   }
   ```

   The fields come in this order, one per line, each a literal: `cargo xtask validate` finds
   cases by reading the source. A case for the oracle tier goes in `oracle/tests/`.
3. **The control arm is part of the case**: inside `measure`, break the property and assert the
   measure sees it. A case that cannot fail is not a case.
4. **Run it**: `cargo xtask quick` (it is a test), then `cargo xtask validate` to see its row.
   The report itself is regenerated at the end of the slice with
   `cargo xtask validate --report --against <the last slice's tag>` and committed.

## The rules the command holds

- **Never drop a case.** A case that has gone from the source fails `validate` unless
  `docs/validation/acknowledged.txt` has `dropped <name>: <why what it checked has gone>`.
- **A tolerance changes only with a reason**: `tolerance <name> <old> -> <new>: <why>` in the
  same file, and the same reason in the slice notes.
- A case that is declared and did not run is reported as **missing**, never skipped.

## Things that go wrong

- A tolerance typed with no prediction behind it. Say where the number comes from or measure
  first on a control and reason from that.
- A control arm that asserts a guess about size ("ten times larger") rather than the property.
  Assert what must be true, not what you expect to see.
- Comparing through a path that is the same code on both sides: the case then says nothing.
  Compute the reference another way (S1's arithmetic written out; the exact sum; another crate).
- Two cases with one name: the second run fails to write its record, saying so.
