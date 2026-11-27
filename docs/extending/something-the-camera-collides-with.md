# Something the camera collides with

The first-person camera is physical (`docs/slices/S20.md` §2, "The walker"): a player-sized box
that matter stops, walking or flying, unless `noclip` is on. A new kind of matter in a scene (a
new piece of the world, a new kind of body, a new surrounding) is solid to the camera only when
it says where it is solid. This recipe adds that.

**Example to read:** `SceneOccupancy::sphere` in `crates/cav-app/src/occupancy.rs`: a point mass
with a radius, as square columns.

## How it fits together

- `crates/cav-app/src/walk.rs` is the camera's box and knows nothing of the scene. It asks an
  `Occupancy` one question: `solids(region, out)` — every solid box (`Solid`: an `Aabb` in
  metres, the world's frame, and the side of a voxel of that matter, 0 if it is not made of
  voxels) that overlaps a region. `Walker::step` asks once for the region a move covers and
  sweeps the box through the answer one axis at a time, so nothing is skipped at any speed.
- `crates/cav-app/src/occupancy.rs` is the scene's answer, `SceneOccupancy`, made anew each
  frame by borrowing the engine (`SceneOccupancy::new`). It reads the scene and never writes
  it: the camera has no mass and is in no scene, edit or reading.

## Steps

1. In `crates/cav-app/src/occupancy.rs`, add a function beside `SceneOccupancy::sphere` that
   pushes your matter's boxes that touch `region` onto `out`. Rules:
   - **Cover all of it** inside the region: a gap is a hole the camera walks through. Boxes may
     overlap one another and reach outside the region.
   - **Read, never generate.** The function runs on the frame's thread, perhaps several times a
     frame: use what the spec says analytically (as `SceneOccupancy::world` does for the
     world's pieces) or what the engine already holds (as `SceneOccupancy::tree` does for a
     voxel body's tree, `Engine::body_tree`). Do not voxelise, stream or wait.
   - **Bound the answer**: the number of boxes for a region the size of the camera's sweep
     (`MAX_SWEEP` and the box) must not grow with the scene's size. `SceneOccupancy::terrain`
     caps its columns at `MAX_COLUMNS` by making them several cells across.
   - **Say the voxel's side** if the matter is made of voxels, so that a step of one voxel is
     climbed (`Solid::voxel`); 0 otherwise.
   - A body moves: place it where it is at the playhead (`SceneOccupancy::new` collects each
     body's place once a frame).
2. Call it from `SceneOccupancy`'s `solids`, where the floor, the room, the world and the
   bodies are.
3. If your matter can lie below everything else in the scene, lower the net with it
   (`SceneOccupancy::net`: what holds the camera up where the scene has nothing below).
4. Say what is solid, and how exactly, in the list at the top of `occupancy.rs`.

## Tests that then hold it

- A unit test beside `a_world_box_is_the_cells_the_voxeliser_fills`: the boxes are where the
  matter is drawn, with a control (a place just outside is free).
- A walk into it: add an arm to `slow_walk_through_town_hits_a_wall_and_noclip_passes` — the
  camera stops at it with noclip off and passes with noclip on.
- `the_camera_never_enters_the_scene_or_the_read` must still pass: nothing about the camera is
  in the scene, the journal's edits, the undo history or a reading.
- `the_walker_obeys_its_box` is the walker's own (standing, sliding, stepping, jumping); a new
  kind of matter does not change it.
