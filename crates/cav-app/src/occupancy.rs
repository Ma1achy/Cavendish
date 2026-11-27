//! What the physical camera collides with in a scene (`docs/slices/S20.md` §2, "The walker"):
//! the scene's matter as solid boxes, for [`crate::walk`]. Made anew each frame from the engine
//! by borrowing; it **reads the scene and never writes it** (the camera has no mass and is in
//! no scene, journal edit or reading).
//!
//! Metres in the world's frame (x east, y north, z up) throughout.
//!
//! What is solid, and how exactly:
//! - **The world's voxels** (`SceneSpec::world`), from its pieces and its grid, analytically:
//!   nothing is generated, streamed or waited for, so the walk is the same whatever is resident
//!   or drawn coarsely far away. A box piece is the block of finest cells with a sample point in
//!   it (its faces on the world's lattice, as it is voxelised and drawn); terrain is a column a
//!   cell, up to the last cell whose lowest sample is under the surface at the column's middle
//!   (the voxeliser asks every sample of the column: the two can differ by one cell where the
//!   surface crosses a cell boundary inside the column).
//! - **Voxel bodies**: the leaves of the engine's tree ([`cav_engine::Engine::body_tree`]) in
//!   the body's frame, moved to where the body is at the playhead. A leaf is solid whatever its
//!   fill. A body moves; the camera is not pushed by it: one that moves onto the camera lifts it
//!   out upward ([`crate::walk::Walker::lift`]).
//! - **Point masses with a radius**: the sphere, as columns about a sixth of its radius across.
//!   A point (radius 0) is nothing.
//! - **The floor** ([`cav_sim::FloorSpec`]): solid below its height (a slab: for its thickness).
//! - **The room** ([`cav_sim::RoomSpec`]): its floor, walls and ceiling as shells [`SHELL`]
//!   thick, outside the room's own volume, **for a camera inside it only**. The room is drawn as
//!   an outline the eye sees through, so it keeps a camera in the hall and never keeps one out
//!   (S20's review, finding 1): a camera whose box is not wholly inside the room's volume when
//!   the occupancy is made passes its walls and ceiling as if they were not there (and so falls
//!   through the roof onto the floor inside, or walks in through a wall); once wholly inside,
//!   it cannot leave with noclip off.
//! - **The net**: below everything, solid under [`SceneOccupancy::net`], so that off the edge
//!   of the world, or in a scene with no ground at all, the camera never falls for ever.
//!
//! A new kind of matter adds itself here: `docs/extending/something-the-camera-collides-with.md`.

use cav_core::voxel::VoxTree;
use cav_core::voxel::tree::Child;
use cav_core::voxel::world::Terrain;
use cav_engine::Engine;
use cav_sim::world::{PieceKind, WorldSpec};
use cav_sim::{FloorKind, SceneSpec};

use crate::walk::{Aabb, Occupancy, Solid};

/// How thick the room's floor, walls and ceiling are for the camera, metres.
pub const SHELL: f64 = 0.1;

/// How far down matter "all the way down" goes, metres.
const DEEP: f64 = 1.0e7;

/// The most columns of terrain made for one question; a region of more cells is answered with
/// columns several cells across.
const MAX_COLUMNS: f64 = 4096.0;

/// One body as the camera meets it now.
struct Body<'a> {
    /// Where it is at the playhead, metres.
    at: [f64; 3],
    /// Its voxels, for a body with a shape.
    tree: Option<&'a VoxTree>,
    /// Its radius, metres, for a sphere.
    radius: f64,
}

/// The scene's matter at the playhead, for the walker.
pub struct SceneOccupancy<'a> {
    spec: &'a SceneSpec,
    bodies: Vec<Body<'a>>,
    net: f64,
    /// Whether the room's shells are solid: the camera was wholly inside the room.
    room: bool,
}

/// The block of finest cells along one axis that have a sample point within `lo..=hi`, as its
/// two faces, metres: cells of `pitch` from `origin`, `k` samples a cell. `None` when no cell has.
fn cells_within(lo: f64, hi: f64, origin: f64, pitch: f64, k: f64) -> Option<(f64, f64)> {
    // Cell c's samples are origin + pitch·(c + (s + ½)/k), s from 0 to k − 1.
    let first = ((lo - origin) / pitch - 1.0 + 0.5 / k).ceil();
    let last = ((hi - origin) / pitch - 0.5 / k).floor();
    (last >= first).then_some((origin + first * pitch, origin + (last + 1.0) * pitch))
}

impl<'a> SceneOccupancy<'a> {
    /// The matter of `engine`'s scene at its playhead, as the camera whose box is `camera`
    /// (metres, the world's frame) meets it. `ground` is the scene's ground level, metres
    /// (`Shell::ground`): the net is there, or at the world's underside if that is lower.
    /// `camera` decides only whether the room holds it ([`Self::in_room`]); nothing else depends on
    /// where the camera is. Make the occupancy anew when the camera has moved (the window does,
    /// each frame): one made for a camera outside the room never stops it at the room's walls.
    #[must_use]
    pub fn new(engine: &'a Engine, ground: f64, camera: &Aabb) -> Self {
        let spec = engine.spec();
        let (time, gravity) = (engine.time(), spec.array.instrument.gravity);
        let bodies = spec
            .bodies
            .iter()
            .enumerate()
            .filter(|(_, body)| {
                let (from, until) = body.exists();
                time >= from && time < until
            })
            .filter_map(|(n, body)| {
                let tree = engine.body_tree(n).map(|tree| &**tree);
                (tree.is_some() || body.radius > 0.0).then(|| Body {
                    at: body.motion.position(time, gravity),
                    tree,
                    radius: if body.shape.is_some() { 0.0 } else { body.radius },
                })
            })
            .collect();
        let underside = spec.world.as_ref().into_iter().flat_map(|w| &w.pieces).map(|p| match &p.kind {
            PieceKind::Box { centre, size } => centre[2] - 0.5 * size[2],
            PieceKind::Terrain { bottom, .. } => *bottom,
        });
        let net = underside.fold(ground, f64::min);
        let room = spec.room.as_ref().is_some_and(|room| {
            let hi = [0, 1, 2].map(|a| room.origin[a] + room.size[a]);
            let eps = crate::walk::EPS;
            (0..3).all(|a| camera.min[a] >= room.origin[a] - eps && camera.max[a] <= hi[a] + eps)
        });
        Self { spec, bodies, net, room }
    }

    /// Whether the room's walls, floor and ceiling are solid for this occupancy: the scene has a
    /// room and the camera's box was wholly inside its volume (to [`crate::walk::EPS`]).
    #[must_use]
    pub const fn in_room(&self) -> bool {
        self.room
    }

    /// The height under which everything is solid, metres: what holds the camera up where the
    /// scene has nothing below it.
    #[must_use]
    pub const fn net(&self) -> f64 {
        self.net
    }

    fn world(world: &WorldSpec, region: &Aabb, out: &mut Vec<Solid>) {
        let (pitch, k, o) = (world.pitch, f64::from(world.supersample.max(1)), world.origin);
        for piece in &world.pieces {
            match &piece.kind {
                PieceKind::Box { centre, size } => {
                    let side = |a: usize| {
                        cells_within(centre[a] - 0.5 * size[a], centre[a] + 0.5 * size[a], o[a], pitch, k)
                    };
                    if let (Some(x), Some(y), Some(z)) = (side(0), side(1), side(2)) {
                        let bounds = Aabb { min: [x.0, y.0, z.0], max: [x.1, y.1, z.1] };
                        if bounds.grown([pitch; 3]).overlaps(region) {
                            out.push(Solid { bounds, voxel: pitch });
                        }
                    }
                }
                PieceKind::Terrain { lo, hi, bottom, level, relief, wavelength, seed } => {
                    let terrain = Terrain {
                        lo: *lo,
                        hi: *hi,
                        bottom: *bottom,
                        level: *level,
                        relief: *relief,
                        wavelength: *wavelength,
                        seed: u64::from(*seed),
                    };
                    Self::terrain(&terrain, region, o, pitch, k, out);
                }
            }
        }
    }

    /// The terrain's columns under `region`: one a cell (or a square of cells, where the region
    /// holds more than [`MAX_COLUMNS`] of them).
    fn terrain(terrain: &Terrain, region: &Aabb, o: [f64; 3], pitch: f64, k: f64, out: &mut Vec<Solid>) {
        if region.min[2] > terrain.level + terrain.relief.abs() + pitch
            || region.max[2] < terrain.bottom - pitch
        {
            return;
        }
        let cell = |a: usize, x: f64| ((x - o[a]) / pitch).floor();
        // The cells of the region that have a sample within the terrain's rectangle.
        let range = |a: usize| {
            let (first, last) = cells_within(terrain.lo[a], terrain.hi[a], o[a], pitch, k)?;
            let from = cell(a, region.min[a]).max(cell(a, first + 0.5 * pitch));
            let to = cell(a, region.max[a]).min(cell(a, last - 0.5 * pitch));
            (to >= from).then_some((from, to))
        };
        let (Some((i0, i1)), Some((j0, j1))) = (range(0), range(1)) else { return };
        let Some((floor, _)) = cells_within(terrain.bottom, f64::INFINITY, o[2], pitch, k) else { return };
        let stride = (((i1 - i0 + 1.0) * (j1 - j0 + 1.0)) / MAX_COLUMNS).sqrt().ceil().max(1.0);
        let mut i = i0;
        while i <= i1 {
            let mut j = j0;
            while j <= j1 {
                let (x, y) = (o[0] + i * pitch, o[1] + j * pitch);
                let across = stride * pitch;
                let surface = terrain.height(x + 0.5 * across, y + 0.5 * across);
                // The last cell whose lowest sample is under the surface.
                let top = o[2] + (((surface - o[2]) / pitch - 0.5 / k).floor() + 1.0) * pitch;
                if top > floor {
                    out.push(Solid {
                        bounds: Aabb { min: [x, y, floor], max: [x + across, y + across, top] },
                        voxel: pitch,
                    });
                }
                j += stride;
            }
            i += stride;
        }
    }

    /// The leaves of `tree` (a body at `at`) that touch `region`.
    fn tree(tree: &VoxTree, at: [f64; 3], region: &Aabb, out: &mut Vec<Solid>) {
        let local = region.moved([-at[0], -at[1], -at[2]]).grown([crate::walk::EPS; 3]);
        let block = |key: u32, level: u8| {
            let (c, half) = (tree.block_centre(key, level), 0.5 * tree.side(level));
            Aabb { min: [c.x - half, c.y - half, c.z - half], max: [c.x + half, c.y + half, c.z + half] }
        };
        let mut push = |n: usize| {
            let leaf = tree.leaves[n];
            let bounds = block(leaf.key, leaf.level);
            if bounds.overlaps(&local) {
                out.push(Solid { bounds: bounds.moved(at), voxel: tree.grid.pitch });
            }
        };
        if tree.nodes.is_empty() {
            (0..tree.leaves.len()).for_each(push);
            return;
        }
        let mut open = vec![0];
        while let Some(n) = open.pop() {
            let node = tree.nodes[n];
            if !block(node.key, node.level).overlaps(&local) {
                continue;
            }
            for child in tree.children_of(n) {
                match child {
                    Child::Leaf(leaf) => push(leaf),
                    Child::Node(node) => open.push(node),
                }
            }
        }
    }

    /// A sphere of `radius` at `at`, as square columns under `region`.
    fn sphere(at: [f64; 3], radius: f64, region: &Aabb, out: &mut Vec<Solid>) {
        let across = (radius / 6.0).max(0.02);
        let from = |a: usize| ((region.min[a].max(at[a] - radius) - at[a]) / across).floor();
        let to = |a: usize| ((region.max[a].min(at[a] + radius) - at[a]) / across).ceil();
        let mut i = from(0);
        while i < to(0) {
            let mut j = from(1);
            while j < to(1) {
                // The column's middle, from the sphere's centre.
                let (x, y) = ((i + 0.5) * across, (j + 0.5) * across);
                let reach = radius * radius - x * x - y * y;
                if reach > 0.0 {
                    let half = reach.sqrt();
                    out.push(Solid {
                        bounds: Aabb {
                            min: [at[0] + i * across, at[1] + j * across, at[2] - half],
                            max: [at[0] + (i + 1.0) * across, at[1] + (j + 1.0) * across, at[2] + half],
                        },
                        voxel: 0.0,
                    });
                }
                j += 1.0;
            }
            i += 1.0;
        }
    }
}

impl Occupancy for SceneOccupancy<'_> {
    fn solids(&self, region: &Aabb, out: &mut Vec<Solid>) {
        // A slab across the whole region, between two heights.
        let slab = |bottom: f64, top: f64| Solid {
            bounds: Aabb {
                min: [region.min[0] - 1.0, region.min[1] - 1.0, bottom],
                max: [region.max[0] + 1.0, region.max[1] + 1.0, top],
            },
            voxel: 0.0,
        };
        if region.min[2] < self.net {
            out.push(slab(self.net - DEEP, self.net));
        }
        if let Some(floor) = &self.spec.floor
            && region.min[2] < floor.height
        {
            let bottom = match floor.kind {
                FloorKind::HalfSpace => floor.height - DEEP,
                FloorKind::Slab { thickness } => floor.height - thickness,
            };
            out.push(slab(bottom, floor.height));
        }
        if let Some(room) = self.spec.room.as_ref().filter(|_| self.room) {
            let (lo, hi) = (room.origin, [0, 1, 2].map(|a| room.origin[a] + room.size[a]));
            let outer = Aabb { min: lo, max: hi }.grown([SHELL; 3]);
            if outer.grown([crate::walk::EPS; 3]).overlaps(region) {
                // Each face's shell: the outer box cut down to the slab outside that face.
                for axis in 0..3 {
                    let (mut below, mut above) = (outer, outer);
                    below.max[axis] = lo[axis];
                    above.min[axis] = hi[axis];
                    out.push(Solid { bounds: below, voxel: 0.0 });
                    out.push(Solid { bounds: above, voxel: 0.0 });
                }
            }
        }
        if let Some(world) = &self.spec.world {
            Self::world(world, region, out);
        }
        for body in &self.bodies {
            if let Some(tree) = body.tree {
                Self::tree(tree, body.at, region, out);
            } else if body.radius > 0.0 {
                Self::sphere(body.at, body.radius, region, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_world_box_is_the_cells_the_voxeliser_fills() {
        // The camera meets a box piece where its voxels are: the cells with a sample point in
        // it. On the lattice that is the box itself; off it, the cells its samples reach.
        // Control: the voxeliser's own chunk of `world-box` is occupied in exactly those cells
        // along a line through the box.
        let scene = cav_sim::load("world-box").unwrap();
        let world = scene.world.as_ref().unwrap();
        let mut solids = Vec::new();
        let everywhere = Aabb { min: [-100.0; 3], max: [100.0; 3] };
        SceneOccupancy::world(world, &everywhere, &mut solids);
        assert_eq!(solids.len(), 1);
        let b = solids[0].bounds;
        assert_eq!((b.min, b.max), ([6.0, 5.0, 0.25], [10.0, 9.0, 2.25]), "on the lattice: the box");
        assert!((solids[0].voxel - 0.25).abs() < 1e-15);
        // Along x through the box's middle, cell by cell, against the voxeliser.
        let made = cav_sim::world::World::new(world);
        let grid = made.grid;
        let inside = |x: f64| b.min[0] <= x && x < b.max[0];
        let mut agreed = 0;
        let mut chunks = std::collections::BTreeMap::new();
        for n in 0..64 {
            let x = (f64::from(n) + 0.5) * world.pitch;
            let point = [x, 7.1, 1.1];
            let key = cav_core::voxel::world::ChunkKey::new(
                (point[0] / grid.chunk_side()).floor() as i32,
                (point[1] / grid.chunk_side()).floor() as i32,
                (point[2] / grid.chunk_side()).floor() as i32,
            );
            let cell = [0, 1, 2].map(|a| {
                let start = f64::from(key.to_array()[a]) * grid.chunk_side();
                ((point[a] - start) / world.pitch).floor() as u32
            });
            let chunk = chunks.entry(key.to_array()).or_insert_with(|| made.chunk(key).unwrap());
            let filled = chunk.as_ref().is_some_and(|chunk| chunk.occupied(cell));
            assert_eq!(filled, inside(x), "the cell at x = {x}");
            agreed += usize::from(filled);
        }
        assert_eq!(agreed, 16, "the box is 16 cells along x");
        // Off the lattice, with two samples a cell: a face at 0.30 m is past the first sample of
        // the cell from 0.25 (at 0.3125) but short of its second (0.4375), which is inside:
        // the cell counts, from 0.25.
        assert_eq!(cells_within(0.30, 1.0, 0.0, 0.25, 2.0), Some((0.25, 1.0)));
        assert_eq!(cells_within(0.45, 1.0, 0.0, 0.25, 2.0), Some((0.5, 1.0)), "no sample of it inside");
        assert_eq!(cells_within(0.30, 0.31, 0.0, 0.25, 2.0), None, "between two samples: no cell");
    }

    #[test]
    fn the_net_is_under_everything() {
        // In `town` the net is at the terrain's underside, below the ground's surface, and the
        // terrain's columns stand on the lattice with their tops a cell apart at most between
        // neighbours (the slope is gentle). In `point` (a room, no world) it is the room's floor.
        let mut engine = Engine::new();
        engine.run_line("load town").unwrap();
        let scene = SceneOccupancy::new(&engine, 0.0, &Aabb { min: [0.0; 3], max: [0.0; 3] });
        assert!((scene.net() + 1.55).abs() < 1e-12, "{}", scene.net());
        let mut solids = Vec::new();
        let region = Aabb { min: [10.0, 10.0, -1.0], max: [10.6, 10.6, 0.8] };
        scene.solids(&region, &mut solids);
        let columns: Vec<&Solid> = solids.iter().filter(|s| (s.voxel - 0.1).abs() < 1e-12).collect();
        assert!((36..=64).contains(&columns.len()), "{} columns under a 0.6 m box", columns.len());
        let tops: Vec<f64> = columns.iter().map(|s| s.bounds.max[2]).collect();
        let (low, high) =
            tops.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), t| (l.min(*t), h.max(*t)));
        assert!(high - low <= 0.1 + 1e-9 && high < 0.0 && low > -0.6, "{low} {high}");
        for top in tops {
            assert!(((top / 0.1).round() * 0.1 - top).abs() < 1e-9, "a top on the lattice: {top}");
        }
        let mut engine = Engine::new();
        engine.run_line("load point").unwrap();
        assert!(
            SceneOccupancy::new(&engine, 0.0, &Aabb { min: [0.0; 3], max: [0.0; 3] }).net().abs() < 1e-15
        );
    }

    #[test]
    fn the_room_holds_a_camera_inside_and_never_keeps_one_out() {
        // S20's review, finding 1. Criterion: in `point` (a room 10 × 10 × 3 m from (−5, −5, 0)),
        // a camera wholly inside the room meets the room's six shells; one outside it, or one
        // straddling a wall or the ceiling, meets none of them. Control: the same region asked
        // of the two occupancies differs only by the shells.
        let mut engine = Engine::new();
        engine.run_line("load point").unwrap();
        let person = |x: f64, z: f64| Aabb { min: [x - 0.3, -0.3, z], max: [x + 0.3, 0.3, z + 1.8] };
        let everywhere = Aabb { min: [-20.0; 3], max: [20.0; 3] };
        let shells = |camera: &Aabb| {
            let world = SceneOccupancy::new(&engine, 0.0, camera);
            let mut out = Vec::new();
            world.solids(&everywhere, &mut out);
            (world.in_room(), out.len())
        };
        // `point` has no world and no body with a size: the net is the one solid; the room adds six.
        assert_eq!(shells(&person(0.0, 0.0)), (true, 7), "inside, standing on the floor");
        assert_eq!(shells(&person(4.7, 1.2)), (true, 7), "inside, touching the east wall and the ceiling");
        assert_eq!(shells(&person(8.0, 0.0)), (false, 1), "outside, east of it");
        assert_eq!(shells(&person(0.0, 3.1)), (false, 1), "above the ceiling");
        assert_eq!(shells(&person(5.0, 0.0)), (false, 1), "straddling the east wall");
        assert_eq!(shells(&person(0.0, 2.0)), (false, 1), "straddling the ceiling");
    }
}
