# Validation

What Cavendish has been checked against, and by how much. `report.md` is the table; `report.json` is the
same data for scripts; the SVGs are its plots. All of them are written by one command and committed with
each slice:

```
cargo xtask validate            # run every case and print the table; fails on a failed or missing case
cargo xtask validate --report   # the same, and write this directory
```

## What a case is

A validation case compares the engine with something known (a closed form, the v1 oracle, one of George's
scripts, a published result, recorded reference data, or the engine against itself where two results must
be identical) and records a number: the measured error, beside the tolerance, in a stated unit. A
convergence case also records the measured order beside the expected one.

A case is declared next to the code it checks:

```rust
cav_validate::case! {
    test: validation_perspective_scales_inversely_with_distance,   // validation_gpu_… if it needs the adapter
    name: "maths.perspective_divide",
    subsystem: "maths",
    slice: "S0",
    reference: ClosedForm,          // or V1Oracle, Script, Paper, Recording, Exact
    against: "a point ten times further away projects ten times nearer the axis",
    unit: Relative,                 // or Radians, Metres, Pixels, Count
    tolerance: 1e-12,
    // order: (2.0, 0.2),           // convergence cases: expected order, and how far off it may be
    measure: cav_validate::Measured::Error(/* the measured error */),
}
```

The macro makes an ordinary test, so `cargo xtask quick` (or `gpu`) runs it and it fails like any other.
Nothing else needs registering: `validate` finds declarations by reading the source, runs the tests, and
lays the two lists side by side.

## What the command guarantees

- **A case that is declared but did not run is reported as missing, and the command fails.** It is never
  skipped. (Tests: `a_declared_case_that_did_not_run_is_missing_and_fails_the_command`; tried end to end
  by marking a real case `#[ignore]`.)
- A case declared twice, a case with no tolerance, a case whose test the filter would not run, and a
  record that no declaration accounts for are errors, not rows
  (`a_collection_that_cannot_be_trusted_is_an_error`).
- A tolerance that is negative, infinite or not a number fails its case: a case that cannot fail is not a
  case (`a_case_without_a_usable_tolerance_fails`).
- A failing case is still a row: it records what it measured before it fails.
- **The committed report is the memory.** `validate` holds every case and tolerance to
  `docs/validation/report.json` as committed at `HEAD` (or at `--against <revision>`, such as the last
  slice's tag). A case that has gone fails the command; so does a tolerance that has changed, looser or
  tighter. The only way past is a line in `acknowledged.txt` naming the case and the reason, which the
  report then prints (`a_dropped_case_or_a_changed_tolerance_fails_unless_it_is_acknowledged`).
- `report.json` holds no time and no commit, so regenerating it without a change in the results changes
  nothing; the history plot reads its earlier versions from git.

## The rules

1. **Never drop a case to make the report green.** A case leaves only when what it checks leaves, and the
   slice notes say so.
2. **A tolerance changes only with a line in the slice notes saying why**, and the same line in
   `acknowledged.txt`, without which `validate` fails.
3. **Every slice ends by regenerating the report** (`cargo xtask validate --report`) and committing
   `docs/validation/`.
4. **The reviewer checks the report**: `cargo xtask validate --against <the last slice's tag>` must pass,
   and every line of `acknowledged.txt` must have its reason in the slice notes.

## The plots

One axis serves every case whatever its unit: the **margin**, the measured error as a fraction of its
tolerance, on a log scale. A case passes to the left of (or below) 1. Cases that measured zero have
their own column, since zero has no logarithm; only a case whose tolerance is zero is called exact.

- `error-vs-tolerance-*.svg`: every case's margin, grouped by subsystem.
- `convergence-*.svg`: error against step for each convergence case, with the expected rate drawn in.
- `history-*.svg`: every case's margin at every commit that changed `report.json`.

They are drawn by `scripts/validation_plots.py` in the README grid's inks, light and dark. The same script
draws the banner under the README's progress grid; the `progress` workflow publishes it to the `progress`
branch beside the grid, so nothing is committed to `main` by a workflow.
