# Extending Cavendish

A recipe for each kind of thing that can be added. Each names one existing example to read
beside it, lists the files to touch in order, and says which tests then hold the addition. Read
`docs/ARCHITECTURE.md` first for where things live, and `CONTRIBUTING.md` for the rules on tests.

| To add | Recipe | Read this example |
|---|---|---|
| a kind of motion, or a field of a body | [a-body-or-a-motion.md](a-body-or-a-motion.md) | `Motion::Linear` in `crates/cav-sim/src/motion.rs` |
| something with mass that is not a point or a sphere | [a-source.md](a-source.md) | `Slab` in `crates/cav-core/src/forward.rs` and `SourceId::Floor` in `crates/cav-sim/src/read.rs` |
| a plot a pane can show, and export as a figure | [a-plot.md](a-plot.md) | the flight diagram (`PaneContent::Flight`) |
| a view type a viewport can show, or a layout of the figure (a mosaic) | [a-view-or-a-layout.md](a-view-or-a-layout.md) | the Analysis layout in `crates/cav-app/ui/layouts.ron`; `Mosaic` in `crates/cav-sim/src/mosaic.rs` |
| a command, with its console, CLI and MCP faces | [a-command.md](a-command.md) | `Command::Compare` and the console's `compare` |
| a dev scene | [a-dev-scene.md](a-dev-scene.md) | `staggered` in `crates/cav-sim/src/dev.rs` |
| a validation case | [a-validation-case.md](a-validation-case.md) | `timing.offset_shifts_one_tower` in `crates/cav-sim/src/read.rs` |
| a shape a body can be made of | [a-shape.md](a-shape.md) | `Ellipsoid` in `crates/cav-core/src/voxel/shape.rs` |
| a body made of voxels | [a-voxel-body.md](a-voxel-body.md) | `platonic_row` in `crates/cav-sim/src/dev.rs` |
| static matter in the world volume: a kind of piece, or a material | [a-world-piece.md](a-world-piece.md) | `PieceKind::Terrain` in `crates/cav-sim/src/world.rs` |
| a material (the library or a scene's own), or a body of several | [a-material.md](a-material.md) | `scenes/materials.ron` |
| anything a browser page cannot do (threads, files, waiting on the GPU) | [the-web-build.md](the-web-build.md) | `crates/cav-engine/src/forward/inline.rs` |
| something the first-person camera collides with (a new kind of matter) | [something-the-camera-collides-with.md](something-the-camera-collides-with.md) | `SceneOccupancy::sphere` in `crates/cav-app/src/occupancy.rs` |
| to the window: an action (menu item, key, palette entry), a tool, a layout, a panel or window, a starter | [a-tool-or-panel.md](a-tool-or-panel.md) | `proj-toggle` in `crates/cav-app/ui/actions.ron`; the Place tool |

Whatever you add: every public item documented with its unit and frame; a test with a control
arm; and if its size can be set by a scene, a view or a command, a bound and a row in the table
of `docs/slices/S2.md` §3.1.

Every function, test and case a recipe names is checked against the code by a test (`xtask`:
`the_recipes_name_things_that_exist`), so a rename fails `quick` until the recipe follows it.
