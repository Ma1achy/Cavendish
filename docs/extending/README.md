# Extending Cavendish

A recipe for each kind of thing that can be added. Each names one existing example to read
beside it, lists the files to touch in order, and says which tests then hold the addition. Read
`docs/ARCHITECTURE.md` first for where things live, and `CONTRIBUTING.md` for the rules on tests.

| To add | Recipe | Read this example |
|---|---|---|
| a kind of motion, or a field of a body | [a-body-or-a-motion.md](a-body-or-a-motion.md) | `Motion::Linear` in `crates/cav-sim/src/motion.rs` |
| something with mass that is not a point or a sphere | [a-source.md](a-source.md) | `Slab` in `crates/cav-core/src/forward.rs` and `SourceId::Floor` in `crates/cav-sim/src/read.rs` |
| a plot a pane can show, and export as a figure | [a-plot.md](a-plot.md) | the flight diagram (`PaneContent::Flight`) |
| a command, with its console, CLI and MCP faces | [a-command.md](a-command.md) | `Command::Compare` and the console's `compare` |
| a dev scene | [a-dev-scene.md](a-dev-scene.md) | `staggered` in `crates/cav-sim/src/dev.rs` |
| a validation case | [a-validation-case.md](a-validation-case.md) | `timing.offset_shifts_one_tower` in `crates/cav-sim/src/read.rs` |

Whatever you add: every public item documented with its unit and frame; a test with a control
arm; and if its size can be set by a scene, a view or a command, a bound and a row in the table
of `docs/slices/S2.md` §3.1.

How these were written: each was followed once, on a throwaway branch, by someone who had not
written the code, adding a small made-up example and running `cargo xtask quick`; what the first
attempt got wrong is what the "Things that go wrong" section of each records.
