# Contributing

How to build Cavendish, how it is tested, and how work is done on it. `docs/ARCHITECTURE.md` says
how the code is put together; `docs/extending/` has a recipe for each kind of thing you may want
to add. The plan is `plan/REBUILD.md`; where the work stands is `plan/PROGRESS.md`.

## Build

You need Rust (the version in `rust-toolchain.toml`), `cargo-nextest`, a GPU that wgpu can use
(Vulkan or DX12) for the `gpu` tier and the window, and, for the oracle tier only, Python 3 with
numpy, matplotlib and pyright.

```
cargo run -p cav-app                      # the window
cargo run -p cav -- help                  # the command line: doctor, render, scenes, get, set, ls, run, replay, figure, exec, mcp
cargo run -p cav -- exec "load comb; periodogram 1"
```

Everything runs locally. There is no CI.

## The tiers

| Command | What it runs | When |
|---|---|---|
| `cargo xtask quick` | `fmt --check`, `clippy -D warnings`, every test whose name has no `gpu_` in it | before every commit; under 10 s warm. `cargo xtask hooks` installs it as a pre-push hook |
| `cargo xtask gpu` | the tests named `gpu_…`: they need the local adapter | before a slice is done, and after touching the renderer or a plot's painter |
| `cargo xtask oracle` | the `oracle/` crate (v1 at `v1-final`, George's scripts), documentation examples, the Python package's types and tests | before a slice is done, and after touching the forward model or a spectrum |
| `cargo xtask validate [--report] [--against <tag>]` | every validation case, tabulated against its tolerance | at the end of a slice: `--report --against <the last slice's tag>` |
| `cargo xtask demo <slice>` | the slice's demo, headless, with its screenshots | at the end of a slice |

Each tier has a floor on how many tests it runs (`xtask/floors.txt`): fewer is a failure. Floors
only go up; raise one in the commit that adds tests.

A test's name decides its tier: put `gpu_` in the name of a test that needs the adapter. A test
that takes more than a second or two in `quick` should say why it must.

## Rules for tests

These are the ones that have each been paid for (the slice notes say where).

1. **A test that guards a property has a control arm**: in the same test, the thing done wrong,
   and the assertion seen to fail for it. A test that cannot fail is not a test.
2. **Write the pass criterion before measuring**, in the slice brief, with where the number comes
   from. If the measurement misses it, the tolerance does not move: find out why, and write down
   what you found. (S1's lesson: a bound loosened to fit was the wrong fix; the right criterion
   was a limit.)
3. **Never weaken an oracle or regenerate a fixture to pass.** A file format gaining a field is a
   reason to rewrite files; a number that changed is not.
4. **A suspicious number gets a measurement, not a story.**
5. **No clocks in assertions.** A busy machine is not a failing test. Hold workers at their gate
   (`Engine::hold_workers`) to say what did and did not run.
6. **Memory is measured**, with the counting allocator in `cav-memtest`, not inferred from time.

## Validation cases

A validation case compares the engine with something known and records the number. It is declared
beside the test of the thing it checks:

```rust
cav_validate::case! {
    test: validation_offset_shifts_one_tower_only,
    name: "timing.offset_shifts_one_tower",
    subsystem: "timing",
    slice: "S2",
    reference: Exact,                 // or ClosedForm, V1Oracle, Script, Paper, Recording
    against: "what exactly it is compared with, in a sentence",
    unit: Relative,                   // or Radians, Metres, Pixels, Count
    tolerance: 1e-12,
    measure: { /* … */ cav_validate::Measured::Error(worst) },
}
```

`cargo xtask validate --report` writes `docs/validation/` (the table, the JSON, the plots). The
rules, held by the command itself: **a case is never dropped**, and **a tolerance changes only
with a line in `docs/validation/acknowledged.txt`** and the same reason in the slice notes.
Recipe: `docs/extending/a-validation-case.md`.

## How work is done

- Work is in slices (`plan/REBUILD.md` §9). Each slice has a brief in `docs/slices/` written
  first: what is new, each requirement traced to a named test with its tolerance, the demo.
- One branch per task, `task/<TASK-id>` (ids are in `plan/tasks.yaml`; add, split or rename, never
  reuse an id). A pull request per task, merged with a merge commit (never squashed, rebased or
  force-pushed) once `cargo xtask quick` is green.
- Small commits. Main always builds and runs.
- A fresh reviewer reads the whole slice at the end; what it finds is fixed in further tasks; the
  slice is tagged when nothing important remains.
- British English. Simplicity over cleverness. A dependency must earn its place, and say how in
  the commit that adds it. `unsafe` is forbidden outside `cav-memtest`.

## Documentation

Every public item of a library crate is documented (`#![warn(missing_docs)]`, which `quick`'s
clippy makes an error): say the unit of every number, the frame of every position, and what the
item must not be used for. Each crate's `lib.rs` opens with what the crate is for and what it may
depend on. When you add a kind of thing, add or update its recipe in `docs/extending/`, written
by following it once and naming one existing example.

## Asking

Physics questions about the real instrument go in `plan/PROGRESS.md` under "For George", with the
default in use until answered. Anything about money, publishing, licences, deleting history or
decisions that cannot be undone goes to the owner first, in the same file.
