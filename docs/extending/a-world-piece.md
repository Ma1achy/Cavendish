# Adding a kind of world piece or a world material

**Example to read:** `PieceKind::Terrain` in `crates/cav-sim/src/world.rs`, built as
`cav_core::voxel::world::Terrain` in `crates/cav-core/src/voxel/world.rs`; the scene that uses
it is `stress_1e10()` in `crates/cav-sim/src/dev.rs`.

The world volume (S4) holds a scene's static matter: terrain, buildings, roads, slabs. It is a
list of pieces, each a solid of one material, voxelised into chunks of 32³ cells when they are
wanted. Moving matter is not a world piece: it is a body (`a-voxel-body.md`).

## A material

A material is data, not code: add an entry to `world.materials` of the scene (its `label`, its
`density` in kg/m³, its `colour` as `#rrggbb`) and name it by index from a piece. Nothing else
changes. A world holds at most 255 (`MAX_MATERIALS`).

## A kind of piece

1. **The solid.** In `crates/cav-core/src/voxel/world.rs` (or `shape.rs`, if it has closed-form
   moments and could also be a body's shape), a type that implements `Solid`: `bounds`,
   `contains`, and a **conservative** `cover` (when it says `Inside` every point of the box is
   inside; `Outside`, none is; unsure is `Mixed`). Every `Shape` is a `Solid` already. Its frame
   is the world's, metres.
2. **Its test.** A test that `cover` is conservative on thousands of boxes of many sizes, with a
   control that a cover without its slack term would be wrong (see
   `terrain_cover_is_conservative`). The chunk oracle then holds it for free: add it to
   `a_chunk_is_the_raster_of_its_pieces`, which compares the voxelised chunk with the raster of
   the same pieces, cell by cell, material by material.
3. **The spec.** A variant of `PieceKind` in `crates/cav-sim/src/world.rs`, every field documented
   with its unit (`x-unit`); its ranges in `WorldSpec::validate`, each refusal naming its path,
   and a line for it in `the_world_is_bounded_in_the_spec`.
4. **Build it.** A match arm in `World::new` that makes the solid from the variant, and one in
   `World::cells` (its volume, for saying how big a world is).
5. **The paper drawing** draws pieces from their chunks and bricks, so a new kind needs nothing
   there; the far field draws it from `cover` (`brick_of_pieces`), which is why `cover` must be
   cheap as well as conservative.
6. **The cache.** A new kind makes no old chunk differ, so the region cache needs nothing. But a
   change to how an existing kind (or the voxeliser, or the precedence of pieces) makes its
   chunks must bump `VOXELISER_VERSION` in `crates/cav-core/src/voxel/world.rs`: cached chunks
   are keyed by it, and a warm cache would otherwise serve the old ones with no warning.

## Tests that then hold it

- `a_chunk_is_the_raster_of_its_pieces` and `a_coarse_brick_holds_its_children` (`cav-core`).
- `the_world_is_bounded_in_the_spec` and `paths::tests::every_leaf_has_a_unit_or_is_dimensionless_and_a_stage`
  (`cav-sim`): a field without a unit fails.
- The read: `world.box_reads_as_a_voxel_body` holds the world's read to a voxel body's; a piece
  whose matter can be written as a box can be checked the same way.

## Things that go wrong

- A `cover` that says `Inside` for a box only touching the solid: the chunk then holds matter the
  raster does not, and the chunk oracle fails at that cell.
- Overlapping pieces: the earlier in `world.pieces` holds the shared matter. A cell two materials
  share takes the one with more of its sample points.
- The whole world is read (S5): far matter through the octree's nodes, within the walk's ledger,
  so a piece needs nothing for the read beyond its chunks. A world whose chunks are past the
  octree's cap (`MAX_READ_CHUNKS_WHOLE`) is not read at all; the read says so.

## A kind of base (`world.base`)

**Example to read:** `BaseSpec::Sphere` in `crates/cav-sim/src/world/base.rs`; the scenes
`plane()` and `planet()` in `crates/cav-sim/src/dev.rs`.

A base is matter under the pieces that the read takes in closed form, never as cells; the pieces
are departures from it (S5, TASK-S5-13). A new kind needs:

1. **The spec.** A variant of `BaseSpec`, its fields with units, its ranges in
   `BaseSpec::validate`, and any rule on the pieces (a planet's may not reach below its top
   layer: the overlap is taken at one density).
2. **The solid and the density.** A `BaseSolid` variant (a conservative `cover`, with no
   cancellation near the surface: `ball_excess` is the example) and `Base::density`, the density
   the pieces' overlap with it is taken at, with a minus sign. `World::read_leaves` then makes the
   departures with no new code (`merge`).
3. **The closed form.** A `BaseSource` variant, read for every tower beside the departures
   (`read_world`), valid where `Base::stands_above` says; towers below are refused in words by
   `SceneSpec::validate`.
4. **The drawing.** A match arm in `drawables_hiding` (`cav-render/src/scene.rs`).
5. **Tests.** Its closed form against an independent one (B1, B3), departures against prisms
   (B2), `the_merge_is_the_cells_added` with it as a base, and the signed walk test.
