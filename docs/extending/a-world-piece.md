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
- A piece far larger than the read radius is read only within the radius of each tower; the
  read says so.
