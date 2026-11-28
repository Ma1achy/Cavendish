# Adding a shape

**Example to read:** `Ellipsoid` in `crates/cav-core/src/voxel/shape.rs`, and
`ShapeKind::Ellipsoid` in `crates/cav-sim/src/shape.rs`.

A shape is something a body can be made of voxels as. The voxeliser asks it two things: whether
a sample point is inside (`contains`), and what it can say about a whole box at once (`cover`:
`Inside`, `Outside` or `Mixed`). It asks nothing else, so a shape is correct when `cover` is
**conservative**: `Inside` only if every point of the box is inside, `Outside` only if none is.
`Mixed` is always a correct answer; a shape that says only `Mixed` is right and slow.

## 1. The solid, in `cav-core`

1. In `crates/cav-core/src/voxel/shape.rs`, a struct with its fields documented (metres, the
   body's frame) and an `impl Shape`:
   - `bounds`: a box holding it all.
   - `contains(p)`: the surface counts as inside.
   - `cover(b)`: from the box's geometry. Keep a margin for rounding of about `1e-12` of the
     shape's size and the box's distance from the origin (`MARGIN` and `reach` beside the other
     shapes): `Inside` when the box is inside by more than the margin, `Outside` when it is
     outside by more than it, `Mixed` otherwise. The ellipsoid's is a model: scale the axes so
     the shape is a unit ball, and the box stays a box.
   - `volume`, `centroid`, `second_moments`: in closed form, from a textbook (say which in the
     doc comment). The tests hold the voxelised body to them.
   - `clip`: its cut-cell clipper (`crate::voxel::cut::Clip`), returning `Some(self)`, for every
     representation but `Sampled` (S5). Without one the default `None` leaves the shape readable
     only by its samples, and `voxelise_cut` refuses it in words. The clipper gives the moments of
     the shape's part of a cell, about the cell's centre (`cut::Raw`, to third order), exactly or to
     rounding: a convex solid clips `cut::Convex::from_box` by its faces' half-spaces (`Polyhedron`
     is the model); a curved one integrates its chord along one axis exactly and the other two by
     quadrature split at every kink (`cut/analytic.rs`: write the solid as a `Radial`, or scale or
     permute it into one, as `Ellipsoid` and `Cylinder` do). Give it a `work` in the voxeliser's
     units, calibrated by time (about 13 ns a unit: time it with `clip_time_per_cell`). A shape
     whose pieces are polyhedra also gives them as signed convex pieces (`CellClip::convex`), which
     `Pieces` reads by the polyhedral kernel near an arm; a curved one leaves the default `None`.
     `draw pieces` draws each cut cell by `CellClip::surface`: the default fans a convex shape's
     one positive piece; a shape whose pieces are signed (a mesh's prisms) must give the closed
     boundary itself (`cut/mesh.rs` is the model), held by `a_piece_s_surface_is_its_piece`.
2. A shape that is a polyhedron needs no new type: give `Polyhedron::hull` its vertices (at most
   100). A shape from a file is a mesh (`crate::voxel::mesh`).

## 2. The spec, in `cav-sim`

1. A variant of `ShapeKind` in `crates/cav-sim/src/shape.rs`, every field with a doc comment, a
   unit (`x-unit`) and a range: the schema, the editors, `ls`, `set` and MCP follow from it.
2. Its name in `ShapeKind::KINDS` and `ShapeKind::name`.
3. `ShapeKind::solid`: the core shape, centred on the origin (the body's position).
4. `ShapeKind::validate`: each number in its range, the field named in the refusal
   (`{at}.YourShape.field: …`).
5. Its drawn surface, for `draw mesh` and `draw mesh-over-voxels` (S5): an arm of
   `cav_sim::body_draw::surface` giving its triangles in the body's frame, anticlockwise seen from
   outside, counted against `MAX_DRAWN_TRIANGLES` before any is made. A polyhedron gives its own
   faces (`Surface::tessellated` false); a curved solid a tessellation (`true`: the frame then
   says so). `a_surface_encloses_its_shape` holds the triangles to the shape's volume (add a
   line, with the tessellation's tolerance).

## 3. The tests that then hold it

- `cav-core`, `voxel::tests`: add it to `shapes()`. Then
  `cover_is_conservative_for_every_shape` checks the contract on 3000 random boxes,
  `validation_slow_the_tree_is_the_raster` checks that the tree expanded is the raster voxeliser
  cell for cell at three pitches and three supersamplings, `validation_the_mass_is_the_spec_s`
  that the mass is the spec's, `validation_volume_and_inertia_match_the_analytic` that the
  voxelised volume and inertia are your closed forms at 64 cells across, and
  `validation_slow_merged_equals_unmerged` that merging changes nothing.
- `cav-core`, `voxel::cut::tests`: add it to `c1_shapes()` with its closed-form moments; then
  `validation_cut_cells_sum_to_the_shape` holds the interior cubes and pieces to them (volume,
  centroid, second and third moments) to 1e-12, with one piece dropped as the control.
- `oracle/tests/bodies.rs`: add it to `solids()` for the convergence to 128 cells.
- `cav-sim`: `shape::tests::every_kind_voxelises_and_is_named` (add a line), and
  `paths::tests::every_leaf_has_a_unit_or_is_dimensionless_and_a_stage` fails if a number has no
  unit.

Followed end to end on 24 Nov 2026 with a made-up shape (a capsule along z, written from the
steps above and thrown away): step 1 and the `shapes()` line, and the nine voxel tests passed on
the first run, its closed-form second moments included (the hemispheres' `r²/5 + h² + 3rh/4`).
The steps of section 2 were not needed to test it and were not followed then; the voxel-body
recipe covers putting a shape in a scene.

## A mesh rather than a new kind

Often a new kind is not needed: an OBJ or STL file is imported as a body (`import <file> fit
<metres> material <label>`, File › Import mesh…, or a file dropped on the window;
`cav_sim::import::body`). A mesh of at most `MAX_INLINE_TRIANGLES` triangles is held in the scene
as `ShapeKind::Triangles`, so the scene needs no file beside it (in a page as natively). A larger
one (up to 32 MiB and 500,000 triangles) is kept by the session in `cav_sim::assets`, by the hash of
its bytes, and named by its path or dropped name and that hash (`ShapeKind::Mesh { hash }`); a
page sends it to each forward worker once (`forward::wire::with_assets`, the lane's `holds`).
Either way it is centred on the middle of its box (`Mesh::centred_on_box`), so `at` is the box's
middle whatever the triangle count. A saved scene names the file and hash: a later session holds
the mesh again when the file is read or dropped again. An STL
holds single-precision numbers: a side of 0.2 m comes back as 0.2000000030 m, which can take one
more cell of the grid. The case `materials.imported_box_mesh_is_the_cuboid` holds an imported box
to the `Cuboid` of the same size, leaf for leaf.

## Things that go wrong

- A `cover` that says `Inside` for a box that pokes out: `cover_is_conservative_for_every_shape`
  fails, and so does the raster test. Do not widen the margin to hide it; find the box.
- A closed form copied with a factor wrong: the volume and inertia test fails at 64 cells. Check
  it against a polyhedron or a quadrature before trusting the voxels.
- A shape whose `contains` and `cover` disagree at the surface: they must agree on which side
  the surface is (inside).
