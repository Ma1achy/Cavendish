# Adding a body made of voxels

**Example to read:** `platonic_row()` in `crates/cav-sim/src/dev.rs`, and the dev scene file
`scenes/dev/platonic-row.ron`.

A body made of voxels is a `BodySpec` with a `shape`: a solid of uniform density, centred on the
body's position, voxelised at `pitch` metres with `supersample³` sample points a voxel, its mass
the body's `mass` to rounding however it voxelises. The read takes it leaf by leaf, each leaf a
uniform cube (`cav_core::voxel::kernel`), exact to rounding: no point approximation and no level
of detail. Its motion is any `Motion`; the body does not turn.

## In a scene file

```ron
(
    label: "plate",
    mass: 2400.0,
    radius: 0.0,
    shape: Some((
        kind: Cuboid(size: (2.0, 2.0, 0.25)),
        pitch: 0.125,
        supersample: 2,
    )),
    motion: Static(position: (0.4, 0.0, -1.0)),
    appears: None,
    vanishes: None,
)
```

`radius` must be 0 for a body with a shape. From the console: `set bodies[0].shape.pitch 0.05`;
`add bodies {...}` with the JSON form; `colour height`, `overlay leaves` to look at it.

## Choosing the pitch

- **At most 1024 voxels across** the shape's widest side (one tree, 30-bit keys): a finer pitch
  is refused, naming `bodies[n].shape.pitch`.
- **What the read costs** is leaves × flight nodes: a body of 10,000 leaves at one AION-10 tower
  is a few tens of milliseconds a shot; the read refuses a body whose leaves × nodes would pass
  `MAX_KERNEL_WORK` (2×10⁹ cube evaluations). A body that never moves (`Static`, with no
  `appears` or `vanishes`) is read once a tower.
- **What the pitch buys:** a voxel sphere of radius r at r/16 reads as the analytic sphere to
  2×10⁻⁷ at 1.6 m (`voxel.sphere_reads_as_the_sphere`). A box on a grid that fits its faces is
  exact: choose the pitch so that each side is a whole number of voxels.
- **Supersampling** weighs the voxels the surface cuts: 4 (the default) is 64 points a voxel.

## As a dev scene

Follow [a-dev-scene.md](a-dev-scene.md): a builder in `crates/cav-sim/src/dev.rs` using
`BodySpec::shaped(label, mass, ShapeSpec { kind, pitch, supersample }, motion)`, added to
`S3_SCENES`, a `LIBRARY` line, and the file written from the builder. Keep the leaves to a few
thousand: `quick` parses every library scene, and the application reads the one it loads.

## Tests that then hold it

- `cav-sim`, `tests/voxel_bodies.rs`: `a_voxel_body_round_trips_and_edits_by_path` shows the
  pattern for a scene with a shape; `validation_slow_a_voxel_sphere_reads_as_the_sphere` and
  `validation_the_plate_is_the_prism` hold a body's reading to a closed form.
- `dev::tests::the_s1_scene_files_are_their_builders` holds the file to the builder.
- `tests/voxel_view.rs`: what a view draws of it (cubes, the legend's range, overlays).

## Things that go wrong

- A mesh file that is not closed: the body is not read, and the warning names the first open
  edge. Every edge must be in exactly two triangles, once each way.
- A body near a tower at a coarse pitch: the voxels are the body, so a coarse sphere is a
  lumpy sphere. Halve the pitch and see that the reading settles.
- A shape that moves fast near a tower: the read halves its panels until it settles, and each
  halving costs every leaf again.
