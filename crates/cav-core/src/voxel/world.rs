//! The world volume: static matter (terrain, buildings, slabs) in chunks of 32³ finest cells, each
//! chunk a voxel tree with its own palette of materials, and the coarse bricks the far field draws.
//!
//! This is `plan/REBUILD.md` §5 and §5.2 ("static matter lives directly in the world volume, in
//! chunks") as the research note sizes it (`docs/research/s3-sparse-voxel-structures.md` §8): a
//! chunk is the **same tree type** as a body ([`VoxTree`]) on a grid of its own, and the world is a
//! sparse set of them keyed by [`ChunkKey`]. Nothing here holds the set: a chunk is made from the
//! world's pieces when it is asked for ([`voxelise_chunk`]), kept by whoever asked (the engine's
//! streaming, the read), and written to region files ([`super::region`]).
//!
//! Frames and units: the world's frame (x east, y north, z up), metres. A chunk's corner is
//! `origin + 32·pitch·key`; its cells are `pitch` metres. Masses are kilograms, densities kg/m³.
//!
//! **Several materials.** A chunk's leaves carry an index into the chunk's palette, which lists
//! the world materials it holds (at most [`MAX_MATERIALS`]). Pieces may overlap: a sample point
//! belongs to the first piece, in the order given, that holds it. A finest cell whose samples
//! belong to two materials takes the material with more of them, and all its samples: the volume
//! is kept, and the minority's density is the majority's (an error of at most one cell layer at an
//! interface). So a chunk's mass is not `unit × samples` as a body's is: [`Chunk::cubes`] and
//! [`Chunk::moments`] weigh each leaf by its own material, and the tree's own `unit` and
//! `moments` (which weigh every sample alike) must not be used for a world chunk's mass.

use crate::Vec3;

use super::morton::{self, decode, span};
use super::shape::{Cover, Shape};
use super::tree::{Leaf, Moments, VoxTree};
use super::{Aabb, Budget, Grid, VoxelError};

/// A chunk's tree is `2^5 = 32` cells across.
pub const CHUNK_DEPTH: u8 = 5;
/// Finest cells along each side of a chunk.
pub const CHUNK_SIDE: u32 = 1 << CHUNK_DEPTH;
/// Finest cells in a chunk, and so the most leaves its tree can have.
pub const CHUNK_CELLS: usize = (CHUNK_SIDE * CHUNK_SIDE * CHUNK_SIDE) as usize;
/// The most materials a world may have: a leaf's palette index is a byte.
pub const MAX_MATERIALS: usize = 255;
/// The most box tests and sample tests the voxelising of one chunk may make. A chunk's leaves are
/// bounded by its cells; this bounds its time (32³ cells × 64 samples × 4 pieces is about 8.4e6,
/// so a chunk cut by a few pieces at the finest sampling fits; one cut by hundreds is refused).
pub const MAX_CHUNK_WORK: u64 = 16_000_000;
/// Cells along each side of a brick: a brick is a chunk's size at its level.
pub const BRICK_SIDE: u32 = 32;
/// Cells along each side of a brick's sub-block: a brick is 4³ of them.
pub const SUB_SIDE: u32 = 8;

/// Where a chunk is: its place in the world's grid of chunks, `[x, y, z]`, from the world's
/// origin. A brick of level `k` is keyed the same way in units of `2^k` chunks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChunkKey {
    /// East.
    pub x: i32,
    /// North.
    pub y: i32,
    /// Up.
    pub z: i32,
}

impl ChunkKey {
    /// The key `[x, y, z]`.
    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// The key of the brick of the next coarser level that holds this one: each coordinate
    /// halved, rounding down.
    #[must_use]
    pub const fn parent(self) -> Self {
        Self { x: self.x.div_euclid(2), y: self.y.div_euclid(2), z: self.z.div_euclid(2) }
    }

    /// The key as an array.
    #[must_use]
    pub const fn to_array(self) -> [i32; 3] {
        [self.x, self.y, self.z]
    }
}

/// What can be put in the world: anything with a box, a test of a point and a conservative
/// `cover` (the [`Shape`] contract without the closed-form moments, which terrain has not).
pub trait Solid: Sync + Send {
    /// A box holding the whole solid, metres, the world's frame.
    fn bounds(&self) -> Aabb;
    /// Whether `point` is inside, its surface counting as inside.
    fn contains(&self, point: Vec3) -> bool;
    /// What the solid says about every point of `boxed`: conservative, as [`Shape::cover`].
    fn cover(&self, boxed: &Aabb) -> Cover;
}

impl<S: Shape + ?Sized> Solid for S {
    fn bounds(&self) -> Aabb {
        Shape::bounds(self)
    }
    fn contains(&self, point: Vec3) -> bool {
        Shape::contains(self, point)
    }
    fn cover(&self, boxed: &Aabb) -> Cover {
        Shape::cover(self, boxed)
    }
}

/// One piece of the world: a solid of one world material (an index into the world's materials).
#[derive(Clone, Copy)]
pub struct Piece<'a> {
    /// The solid, in the world's frame.
    pub solid: &'a dyn Solid,
    /// Its material: an index into the world's materials, below [`MAX_MATERIALS`].
    pub material: u8,
}

/// The world's grid: where chunk `[0, 0, 0]` starts, the side of a finest cell and how finely a
/// cell is sampled. Every chunk of a world is on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldGrid {
    /// The corner of chunk `[0, 0, 0]`, metres, the world's frame.
    pub origin: Vec3,
    /// The side of a finest cell, metres.
    pub pitch: f64,
    /// Sample points along each side of a finest cell, 1 to [`super::MAX_SUPERSAMPLE`].
    pub supersample: u8,
}

impl WorldGrid {
    /// The side of a brick of level `level` (a chunk at level 0), metres.
    #[must_use]
    pub fn brick_side(&self, level: u8) -> f64 {
        self.pitch * f64::from(BRICK_SIDE) * f64::from(1u32 << level)
    }

    /// The side of a chunk, metres.
    #[must_use]
    pub fn chunk_side(&self) -> f64 {
        self.brick_side(0)
    }

    /// The box of brick `key` of level `level` (a chunk at level 0), metres, the world's frame.
    #[must_use]
    pub fn brick_box(&self, level: u8, key: ChunkKey) -> Aabb {
        let side = self.brick_side(level);
        let lo = self.origin + Vec3::new(f64::from(key.x), f64::from(key.y), f64::from(key.z)) * side;
        Aabb::new(lo, lo + Vec3::new(side, side, side))
    }

    /// The box of chunk `key`.
    #[must_use]
    pub fn chunk_box(&self, key: ChunkKey) -> Aabb {
        self.brick_box(0, key)
    }

    /// The tree grid of chunk `key`: depth 5, every cell used.
    #[must_use]
    pub fn chunk_grid(&self, key: ChunkKey) -> Grid {
        Grid {
            origin: self.chunk_box(key).lo,
            pitch: self.pitch,
            depth: CHUNK_DEPTH,
            cells: [CHUNK_SIDE; 3],
            supersample: self.supersample,
        }
    }

    /// The keys of the bricks of level `level` that `boxed` touches, as an inclusive range `(lo,
    /// hi)`. A coordinate beyond the `i32` range is clamped to it.
    #[must_use]
    pub fn bricks_of(&self, level: u8, boxed: &Aabb) -> (ChunkKey, ChunkKey) {
        let side = self.brick_side(level);
        let at =
            |v: f64, o: f64| ((v - o) / side).floor().clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
        let (o, lo, hi) = (self.origin, boxed.lo, boxed.hi);
        (
            ChunkKey::new(at(lo.x, o.x), at(lo.y, o.y), at(lo.z, o.z)),
            ChunkKey::new(at(hi.x, o.x), at(hi.y, o.y), at(hi.z, o.z)),
        )
    }

    /// The volume a sample point stands for, cubic metres: `pitch³ / k³`.
    #[must_use]
    pub fn sample_volume(&self) -> f64 {
        self.pitch.powi(3) / f64::from(u32::from(self.supersample).pow(3))
    }
}

/// One chunk of the world: its tree on its own grid, and the world materials its leaves name.
#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    /// Where it is.
    pub key: ChunkKey,
    /// Its matter: leaves of counts, each leaf's `material` an index into `palette`. The tree's
    /// `unit` and `moments` weigh every sample alike; for masses use [`Self::cubes`].
    pub tree: VoxTree,
    /// The world materials it holds, ascending: leaf material `i` is world material `palette[i]`.
    pub palette: Vec<u8>,
}

impl Chunk {
    /// The world material of `leaf`.
    #[must_use]
    pub fn material(&self, leaf: &Leaf) -> u8 {
        self.palette[usize::from(leaf.material)]
    }

    /// The leaves as uniform cubes: `(centre, metres, the world's frame; mass, kg; side, m)`, in
    /// Morton order. `densities` are the world materials', kg/m³, by world index.
    ///
    /// # Panics
    /// When a palette entry has no density in `densities`.
    #[must_use]
    pub fn cubes(&self, grid: &WorldGrid, densities: &[f64]) -> Vec<(Vec3, f64, f64)> {
        let sample = grid.sample_volume();
        self.tree
            .leaves
            .iter()
            .map(|leaf| {
                let density = densities[usize::from(self.material(leaf))];
                let mass = leaf.samples() as f64 * sample * density;
                (self.tree.block_centre(leaf.key, leaf.level), mass, self.tree.side(leaf.level))
            })
            .collect()
    }

    /// The chunk's mass moments, each leaf a uniform cube of its own material's density, summed in
    /// Morton order ([`Moments::combine`]). Zero mass when every material it holds weighs nothing.
    #[must_use]
    pub fn moments(&self, grid: &WorldGrid, densities: &[f64]) -> Moments {
        let parts: Vec<Moments> = self
            .cubes(grid, densities)
            .into_iter()
            .filter(|(_, mass, _)| *mass > 0.0)
            .map(|(centre, mass, side)| Moments::cube(mass, side, centre))
            .collect();
        if parts.is_empty() {
            return Moments {
                mass: 0.0,
                centre: grid.chunk_box(self.key).centre(),
                second: [0.0; 6],
                third: [0.0; 10],
            };
        }
        Moments::combine(&parts)
    }

    /// The bytes it holds, roughly: the tree's and the palette's.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.tree.bytes() + self.palette.len() + size_of::<Self>()
    }

    /// Whether finest cell `[x, y, z]` (each below 32) holds matter.
    #[must_use]
    pub fn occupied(&self, cell: [u32; 3]) -> bool {
        let key = morton::encode(cell);
        let at = self.tree.leaves.partition_point(|leaf| leaf.end() <= u64::from(key));
        self.tree.leaves.get(at).is_some_and(|leaf| leaf.key <= key)
    }

    /// Every finest cell's count and world material (0 where empty), x fastest: the chunk
    /// expanded, as the raster oracle ([`raster_chunk`]) gives it.
    #[must_use]
    pub fn expand(&self) -> (Vec<u8>, Vec<u8>) {
        let mut counts = vec![0u8; CHUNK_CELLS];
        let mut materials = vec![0u8; CHUNK_CELLS];
        for leaf in &self.tree.leaves {
            let [cx, cy, cz] = decode(leaf.key);
            let side = 1u32 << leaf.level;
            for z in cz..cz + side {
                for y in cy..cy + side {
                    for x in cx..cx + side {
                        let i = cell_index([x, y, z]);
                        counts[i] = leaf.count;
                        materials[i] = self.material(leaf);
                    }
                }
            }
        }
        (counts, materials)
    }
}

/// What the picture needs of a chunk: its leaves and palette, without the tree's nodes and
/// moments (which weigh twenty times as much and only the read needs). What streaming keeps
/// resident and the region files hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkCells {
    /// Where it is.
    pub key: ChunkKey,
    /// Its leaves, in Morton order, materials indexing `palette`.
    pub leaves: Vec<Leaf>,
    /// The world materials it holds, ascending.
    pub palette: Vec<u8>,
}

impl ChunkCells {
    /// A chunk's leaves and palette.
    #[must_use]
    pub fn of(chunk: &Chunk) -> Self {
        Self { key: chunk.key, leaves: chunk.tree.leaves.clone(), palette: chunk.palette.clone() }
    }

    /// Every finest cell's world material plus one (0 where empty), x fastest ([`cell_index`]).
    #[must_use]
    pub fn materials(&self) -> Vec<u8> {
        let mut out = vec![0u8; CHUNK_CELLS];
        for leaf in &self.leaves {
            let [cx, cy, cz] = decode(leaf.key);
            let side = 1u32 << leaf.level;
            let m = self.palette[usize::from(leaf.material)].saturating_add(1);
            for z in cz..cz + side {
                for y in cy..cy + side {
                    for x in cx..cx + side {
                        out[cell_index([x, y, z])] = m;
                    }
                }
            }
        }
        out
    }

    /// The bytes it holds.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.leaves.len() * size_of::<Leaf>() + self.palette.len() + size_of::<Self>()
    }
}

/// The index of chunk cell `[x, y, z]` in an expanded chunk: x fastest, then y, then z.
#[must_use]
pub const fn cell_index(cell: [u32; 3]) -> usize {
    (cell[0] + CHUNK_SIDE * (cell[1] + CHUNK_SIDE * cell[2])) as usize
}

/// How one finest cell's samples fall: the count inside any piece and the material it takes
/// (the one with most samples; on a tie, the lower world index), or `None` when empty.
fn classify(pieces: &[Piece<'_>], grid: &Grid, cell: [u32; 3], work: &mut u64) -> Option<(u8, u8)> {
    let k = u32::from(grid.supersample);
    let mut per = [0u16; MAX_MATERIALS + 1];
    let mut inside = 0u8;
    for c in 0..k {
        for b in 0..k {
            for a in 0..k {
                let point = grid.sample(cell, [a, b, c]);
                for piece in pieces {
                    *work += 1;
                    if piece.solid.contains(point) {
                        per[usize::from(piece.material)] += 1;
                        inside += 1;
                        break;
                    }
                }
            }
        }
    }
    if inside == 0 {
        return None;
    }
    let mut best = 0usize;
    for m in 1..per.len() {
        if per[m] > per[best] {
            best = m;
        }
    }
    Some((inside, best as u8))
}

/// The raster oracle for a chunk: every finest cell classified sample by sample, no `cover` at
/// all. Returns `(counts, world materials)` as [`Chunk::expand`] lays them out. For tests.
#[must_use]
pub fn raster_chunk(pieces: &[Piece<'_>], grid: &WorldGrid, key: ChunkKey) -> (Vec<u8>, Vec<u8>) {
    let tree_grid = grid.chunk_grid(key);
    let mut counts = vec![0u8; CHUNK_CELLS];
    let mut materials = vec![0u8; CHUNK_CELLS];
    let mut work = 0;
    for z in 0..CHUNK_SIDE {
        for y in 0..CHUNK_SIDE {
            for x in 0..CHUNK_SIDE {
                if let Some((count, material)) = classify(pieces, &tree_grid, [x, y, z], &mut work) {
                    let i = cell_index([x, y, z]);
                    counts[i] = count;
                    materials[i] = material;
                }
            }
        }
    }
    (counts, materials)
}

/// Chunk `key` of the world made of `pieces` (in order of precedence) on `grid`, top-down: a
/// block no piece reaches is skipped, a block the first piece that reaches it covers wholly is one
/// full leaf, a mixed block is split, and at the finest level the samples are counted
/// ([`raster_chunk`]'s rule). Eight equal leaves are merged, as a body's are. `None` when the
/// chunk holds nothing.
///
/// # Errors
/// When the work passes [`MAX_CHUNK_WORK`], a material is not below [`MAX_MATERIALS`], or the
/// supersampling is outside 1 to [`super::MAX_SUPERSAMPLE`].
pub fn voxelise_chunk(
    pieces: &[Piece<'_>],
    grid: &WorldGrid,
    key: ChunkKey,
) -> Result<Option<Chunk>, VoxelError> {
    voxelise_chunk_within(pieces, grid, key, MAX_CHUNK_WORK)
}

/// [`voxelise_chunk`] within `max_work` box and sample tests: for tests of the refusal.
///
/// # Errors
/// As [`voxelise_chunk`], with `max_work`.
pub fn voxelise_chunk_within(
    pieces: &[Piece<'_>],
    grid: &WorldGrid,
    key: ChunkKey,
    max_work: u64,
) -> Result<Option<Chunk>, VoxelError> {
    if !(1..=super::MAX_SUPERSAMPLE).contains(&grid.supersample) {
        return Err(VoxelError(format!(
            "supersample: {} must be from 1 to {}",
            grid.supersample,
            super::MAX_SUPERSAMPLE
        )));
    }
    if let Some(piece) = pieces.iter().find(|p| usize::from(p.material) >= MAX_MATERIALS) {
        return Err(VoxelError(format!(
            "material {} is past the {MAX_MATERIALS} a world holds",
            piece.material
        )));
    }
    let tree_grid = grid.chunk_grid(key);
    let boxed = grid.chunk_box(key);
    // Only the pieces whose boxes reach the chunk, in their order.
    let near: Vec<Piece<'_>> = pieces.iter().copied().filter(|p| p.solid.bounds().overlaps(&boxed)).collect();
    if near.is_empty() {
        return Ok(None);
    }
    let mut walk = ChunkWalk { grid: &tree_grid, leaves: Vec::new(), work: 0, max_work };
    walk.visit(&near, 0, CHUNK_DEPTH)?;
    if walk.leaves.is_empty() {
        return Ok(None);
    }
    // The palette: the world materials the leaves name, ascending; leaves renamed to it.
    let mut palette: Vec<u8> = walk.leaves.iter().map(|l| l.material).collect();
    palette.sort_unstable();
    palette.dedup();
    for leaf in &mut walk.leaves {
        leaf.material = palette.binary_search(&leaf.material).unwrap_or(0) as u8;
    }
    let samples: u64 = walk.leaves.iter().map(Leaf::samples).sum();
    // The tree's own unit is one sample's worth; masses come from `Chunk::cubes`.
    let tree = VoxTree::build_within(tree_grid, walk.leaves, samples as f64, Budget::DEFAULT)?;
    Ok(Some(Chunk { key, tree, palette }))
}

struct ChunkWalk<'a> {
    grid: &'a Grid,
    leaves: Vec<Leaf>,
    work: u64,
    max_work: u64,
}

impl ChunkWalk<'_> {
    fn spend(&mut self) -> Result<(), VoxelError> {
        if self.work > self.max_work {
            return Err(VoxelError(format!(
                "voxelising a chunk would take more than {} box and sample tests: use fewer overlapping \
                 pieces, a coarser pitch or less supersampling",
                self.max_work
            )));
        }
        Ok(())
    }

    fn visit(&mut self, pieces: &[Piece<'_>], key: u32, level: u8) -> Result<(), VoxelError> {
        let corner = decode(key);
        let side = 1u32 << level;
        let boxed = self.grid.sample_box(corner, side);
        // The pieces that may reach this block, in order; the first that covers it wholly owns it.
        let mut active: Vec<Piece<'_>> = Vec::with_capacity(pieces.len());
        let full = self.grid.samples_per_cell() as u8;
        for piece in pieces {
            self.work += 1;
            match piece.solid.cover(&boxed) {
                Cover::Outside => {}
                Cover::Inside if active.is_empty() => {
                    self.spend()?;
                    self.leaves.push(Leaf { key, level, count: full, material: piece.material });
                    return Ok(());
                }
                _ => active.push(*piece),
            }
        }
        self.spend()?;
        if active.is_empty() {
            return Ok(());
        }
        if level == 0 {
            let found = classify(&active, self.grid, corner, &mut self.work);
            self.spend()?;
            if let Some((count, material)) = found {
                self.leaves.push(Leaf { key, level, count, material });
            }
            return Ok(());
        }
        let before = self.leaves.len();
        for octant in 0..8 {
            self.visit(&active, morton::child(key, level, octant), level - 1)?;
        }
        let new = &self.leaves[before..];
        if new.len() == 8
            && new
                .iter()
                .all(|l| l.level == level - 1 && l.count == new[0].count && l.material == new[0].material)
        {
            let merged = Leaf { key, level, count: new[0].count, material: new[0].material };
            self.leaves.truncate(before);
            self.leaves.push(merged);
        }
        debug_assert!(self.leaves.len() <= before + span(level) as usize);
        Ok(())
    }
}

/// A brick of the far field: 32³ cells of `2^level · pitch` metres, as occupancy bits, a 4³ mask of
/// its 8³ sub-blocks (the two-level "64-tree" form: a ray skips an empty sub-block in one step),
/// and the material of each sub-block. What the renderer marches; never read by the instrument.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Brick {
    /// Its level: cells of `2^level` finest cells.
    pub level: u8,
    /// Where it is, in units of bricks of its level.
    pub key: ChunkKey,
    /// Cell `[x, y, z]` is bit `i % 32` of word `i / 32`, `i = x + 32 y + 1024 z`.
    pub bits: Vec<u32>,
    /// Sub-block `[a, b, c]` (each 0 to 3) is bit `a + 4 b + 16 c`: set when any of its cells is.
    pub mask: u64,
    /// The world material of each sub-block (in the mask's order), 0 where it is empty: the
    /// commonest among its cells.
    pub materials: [u8; 64],
}

impl Brick {
    /// Words of occupancy bits a brick holds.
    pub const WORDS: usize = (BRICK_SIDE * BRICK_SIDE * BRICK_SIDE / 32) as usize;

    /// An empty brick.
    #[must_use]
    pub fn empty(level: u8, key: ChunkKey) -> Self {
        Self { level, key, bits: vec![0; Self::WORDS], mask: 0, materials: [0; 64] }
    }

    /// Whether cell `[x, y, z]` (each below 32) is occupied.
    #[must_use]
    pub fn get(&self, cell: [u32; 3]) -> bool {
        let i = cell[0] + BRICK_SIDE * (cell[1] + BRICK_SIDE * cell[2]);
        self.bits[(i / 32) as usize] >> (i % 32) & 1 == 1
    }

    /// Marks cell `[x, y, z]` occupied, and its sub-block.
    pub fn set(&mut self, cell: [u32; 3]) {
        let i = cell[0] + BRICK_SIDE * (cell[1] + BRICK_SIDE * cell[2]);
        self.bits[(i / 32) as usize] |= 1 << (i % 32);
        self.mask |= 1 << sub_index(cell);
    }

    /// How many cells are occupied.
    #[must_use]
    pub fn count(&self) -> u32 {
        self.bits.iter().map(|w| w.count_ones()).sum()
    }

    /// The level-0 brick of a chunk: a cell is occupied when its count is not zero; each
    /// sub-block takes its commonest world material.
    #[must_use]
    pub fn of_chunk(chunk: &Chunk) -> Self {
        let mut brick = Self::empty(0, chunk.key);
        let mut tally = vec![[0u32; MAX_MATERIALS + 1]; 64];
        for leaf in &chunk.tree.leaves {
            let [cx, cy, cz] = decode(leaf.key);
            let side = 1u32 << leaf.level;
            for z in cz..cz + side {
                for y in cy..cy + side {
                    for x in cx..cx + side {
                        brick.set([x, y, z]);
                        tally[sub_index([x, y, z])][usize::from(chunk.material(leaf))] += 1;
                    }
                }
            }
        }
        brick.take_materials(&tally);
        brick
    }

    fn take_materials(&mut self, tally: &[[u32; MAX_MATERIALS + 1]]) {
        for (s, counts) in tally.iter().enumerate() {
            if self.mask >> s & 1 == 1 {
                let best = (0..counts.len()).fold(0, |b, m| if counts[m] > counts[b] { m } else { b });
                self.materials[s] = best as u8;
            }
        }
    }
}

/// The index of the sub-block holding cell `[x, y, z]` of a brick.
#[must_use]
pub const fn sub_index(cell: [u32; 3]) -> usize {
    ((cell[0] / SUB_SIDE) + 4 * (cell[1] / SUB_SIDE) + 16 * (cell[2] / SUB_SIDE)) as usize
}

/// Brick `key` of level `level` straight from the pieces: a cell is occupied when some piece's
/// `cover` of the cell's box is not `Outside` (conservative: it holds every finer brick's cells
/// and may add some at a surface), its material the first such piece's. For level 1 and up; level
/// 0 is [`Brick::of_chunk`]. `None` when empty.
#[must_use]
pub fn brick_of_pieces(pieces: &[Piece<'_>], grid: &WorldGrid, level: u8, key: ChunkKey) -> Option<Brick> {
    let boxed = grid.brick_box(level, key);
    let near: Vec<Piece<'_>> = pieces.iter().copied().filter(|p| p.solid.bounds().overlaps(&boxed)).collect();
    if near.is_empty() {
        return None;
    }
    let cell = boxed.size().x / f64::from(BRICK_SIDE);
    let cell_box = |c: [u32; 3], side: u32| {
        let lo = boxed.lo + Vec3::new(f64::from(c[0]), f64::from(c[1]), f64::from(c[2])) * cell;
        let s = f64::from(side) * cell;
        Aabb::new(lo, lo + Vec3::new(s, s, s))
    };
    let mut brick = Brick::empty(level, key);
    let mut tally = vec![[0u32; MAX_MATERIALS + 1]; 64];
    let mut active: Vec<Piece<'_>> = Vec::with_capacity(near.len());
    for s in 0..64u32 {
        let corner = [(s % 4) * SUB_SIDE, (s / 4 % 4) * SUB_SIDE, (s / 16) * SUB_SIDE];
        let sub = cell_box(corner, SUB_SIDE);
        active.clear();
        active.extend(near.iter().copied().filter(|p| p.solid.cover(&sub) != Cover::Outside));
        if active.is_empty() {
            continue;
        }
        for z in corner[2]..corner[2] + SUB_SIDE {
            for y in corner[1]..corner[1] + SUB_SIDE {
                for x in corner[0]..corner[0] + SUB_SIDE {
                    let b = cell_box([x, y, z], 1);
                    if let Some(p) = active.iter().find(|p| p.solid.cover(&b) != Cover::Outside) {
                        brick.set([x, y, z]);
                        tally[s as usize][usize::from(p.material)] += 1;
                    }
                }
            }
        }
    }
    if brick.mask == 0 {
        return None;
    }
    brick.take_materials(&tally);
    Some(brick)
}

/// Rolling terrain over a rectangle: matter from `bottom` up to a surface `level + relief·f(x, y)`
/// with `|f| ≤ 1`, a sum of three octaves of crossed sines of wavelengths `wavelength`, half
/// and a quarter of it, their phases drawn from `seed`. Deterministic, smooth, bounded in slope,
/// so its `cover` is exact enough to voxelise a kilometre at centimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Terrain {
    /// The rectangle's south-west corner, `[x, y]`, metres.
    pub lo: [f64; 2],
    /// Its north-east corner, `[x, y]`, metres.
    pub hi: [f64; 2],
    /// The underside, metres, up.
    pub bottom: f64,
    /// The mean surface, metres, up.
    pub level: f64,
    /// How far the surface rises and falls about `level`, metres (0 for flat).
    pub relief: f64,
    /// The longest wavelength of the hills, metres.
    pub wavelength: f64,
    /// The phases' seed.
    pub seed: u64,
}

/// The weights of the octaves (1, ½, ¼), summing to 1.75, and so normalised.
const OCTAVES: [f64; 3] = [1.0 / 1.75, 0.5 / 1.75, 0.25 / 1.75];

/// A number from `seed` and `n`, uniform in `[0, 1)`: one round of SplitMix64.
fn unit_from(seed: u64, n: u64) -> f64 {
    let mut z = seed.wrapping_add(n.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

impl Terrain {
    /// The surface's height at `(x, y)`, metres (inside the rectangle or not).
    #[must_use]
    pub fn height(&self, x: f64, y: f64) -> f64 {
        if self.relief == 0.0 {
            return self.level;
        }
        let mut sum = 0.0;
        for (o, weight) in OCTAVES.iter().enumerate() {
            let k = std::f64::consts::TAU * f64::from(1u32 << o) / self.wavelength;
            let (px, py) = (unit_from(self.seed, 2 * o as u64), unit_from(self.seed, 2 * o as u64 + 1));
            sum += weight
                * (k * x + std::f64::consts::TAU * px).sin()
                * (k * y + std::f64::consts::TAU * py).sin();
        }
        self.level + self.relief * sum
    }

    /// The most the surface can rise per metre in any direction: `√2 · relief · Σ wᵢ·2π·2ⁱ/λ`.
    #[must_use]
    pub fn slope(&self) -> f64 {
        let per: f64 = OCTAVES.iter().enumerate().map(|(o, w)| w * f64::from(1u32 << o)).sum();
        std::f64::consts::SQRT_2 * self.relief.abs() * std::f64::consts::TAU * per / self.wavelength
    }
}

impl Solid for Terrain {
    fn bounds(&self) -> Aabb {
        Aabb::new(
            Vec3::new(self.lo[0], self.lo[1], self.bottom),
            Vec3::new(self.hi[0], self.hi[1], self.level + self.relief.abs()),
        )
    }

    fn contains(&self, p: Vec3) -> bool {
        p.x >= self.lo[0]
            && p.x <= self.hi[0]
            && p.y >= self.lo[1]
            && p.y <= self.hi[1]
            && p.z >= self.bottom
            && p.z <= self.height(p.x, p.y)
    }

    fn cover(&self, b: &Aabb) -> Cover {
        let whole = self.bounds();
        let scale = whole.lo.length().max(whole.hi.length()).max(b.lo.length()).max(b.hi.length());
        let slack = 1e-12 * scale;
        if !b.grown(slack).overlaps(&whole) {
            return Cover::Outside;
        }
        // The surface over the box's footprint: within slope × half-diagonal of its middle's.
        let (cx, cy) = (0.5 * (b.lo.x + b.hi.x), 0.5 * (b.lo.y + b.hi.y));
        let reach = 0.5 * (b.hi.x - b.lo.x).hypot(b.hi.y - b.lo.y);
        let h = self.height(cx, cy);
        let spread = self.slope() * reach + 1e-9 * (self.relief.abs() + slack) + slack;
        if b.lo.z > h + spread {
            return Cover::Outside;
        }
        let within = b.lo.x > self.lo[0] + slack
            && b.hi.x < self.hi[0] - slack
            && b.lo.y > self.lo[1] + slack
            && b.hi.y < self.hi[1] - slack
            && b.lo.z > self.bottom + slack;
        if within && b.hi.z < h - spread { Cover::Inside } else { Cover::Mixed }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::shape::Cuboid;

    fn grid(pitch: f64, supersample: u8) -> WorldGrid {
        WorldGrid { origin: Vec3::new(-1.0, -2.0, -0.5), pitch, supersample }
    }

    fn cuboid(lo: [f64; 3], hi: [f64; 3]) -> Cuboid {
        let (lo, hi) = (Vec3::from_array(lo), Vec3::from_array(hi));
        Cuboid { centre: (lo + hi) * 0.5, half: (hi - lo) * 0.5 }
    }

    fn hills() -> Terrain {
        Terrain {
            lo: [-3.0, -4.0],
            hi: [9.0, 7.0],
            bottom: -0.4,
            level: 0.9,
            relief: 0.6,
            wavelength: 3.0,
            seed: 7,
        }
    }

    #[test]
    fn a_chunk_is_the_raster_of_its_pieces() {
        // Terrain, a building on it of another material, a slab through both (later, so lower in
        // precedence): every case of the walk, at a pitch that cuts all of them.
        let terrain = hills();
        let house = cuboid([0.3, 0.1, 0.2], [2.05, 1.6, 2.4]);
        let slab = cuboid([-2.0, -3.0, 0.0], [5.0, 5.0, 0.33]);
        let pieces = [
            Piece { solid: &house, material: 2 },
            Piece { solid: &terrain, material: 0 },
            Piece { solid: &slab, material: 5 },
        ];
        let g = grid(0.1, 2);
        let mut seen = 0;
        for key in
            [ChunkKey::new(0, 0, 0), ChunkKey::new(1, 0, 0), ChunkKey::new(0, 1, 0), ChunkKey::new(1, 1, 0)]
        {
            let raster = raster_chunk(&pieces, &g, key);
            match voxelise_chunk(&pieces, &g, key).unwrap() {
                Some(chunk) => {
                    assert_eq!(chunk.expand(), raster, "chunk {key:?}");
                    assert!(chunk.palette.windows(2).all(|w| w[0] < w[1]));
                    seen += chunk.palette.len();
                }
                None => assert!(raster.0.iter().all(|&c| c == 0), "chunk {key:?}"),
            }
        }
        assert!(seen >= 4, "the chunks hold several materials between them: {seen}");
        // Control: a raster of other pieces (precedence swapped) differs where they overlap.
        let swapped = [
            Piece { solid: &slab, material: 5 },
            Piece { solid: &terrain, material: 0 },
            Piece { solid: &house, material: 2 },
        ];
        let chunk = voxelise_chunk(&pieces, &g, ChunkKey::new(0, 0, 0)).unwrap().unwrap();
        assert_ne!(chunk.expand(), raster_chunk(&swapped, &g, ChunkKey::new(0, 0, 0)));
    }

    #[test]
    fn chunks_tile_a_body_exactly() {
        // A box over several chunks, voxelised as one body on the same lattice (the body's grid
        // is centred on its box; its origin is put on the world's lattice by choosing the box).
        let g = WorldGrid { origin: Vec3::ZERO, pitch: 0.25, supersample: 3 };
        let boxed = cuboid([1.1, 2.0, 0.9], [13.3, 9.6, 4.45]);
        let pieces = [Piece { solid: &boxed, material: 0 }];
        let (lo, hi) = g.bricks_of(0, &Solid::bounds(&boxed));
        let mut samples = 0u64;
        let mut chunks = 0;
        for x in lo.x..=hi.x {
            for y in lo.y..=hi.y {
                for z in lo.z..=hi.z {
                    if let Some(chunk) = voxelise_chunk(&pieces, &g, ChunkKey::new(x, y, z)).unwrap() {
                        samples += chunk.tree.samples;
                        chunks += 1;
                    }
                }
            }
        }
        assert!(chunks >= 4, "{chunks}");
        // The same samples, counted cell by cell on the world's lattice.
        let k = 3u64;
        let along = |a: f64, b: f64| -> u64 {
            // Sample coordinates i·pitch/k + pitch/(2k) inside [a, b].
            let step = g.pitch / k as f64;
            (0..((b / step).ceil() as i64 + 2))
                .filter(|i| {
                    let s = (*i as f64 + 0.5) * step;
                    s >= a && s <= b
                })
                .count() as u64
        };
        let want = along(1.1, 13.3) * along(2.0, 9.6) * along(0.9, 4.45);
        assert_eq!(samples, want);
        // Control: the sum with one chunk left out is short.
        let one = voxelise_chunk(&pieces, &g, lo).unwrap().map_or(0, |c| c.tree.samples);
        assert!(one > 0 && samples - one < want);
    }

    #[test]
    fn a_chunk_past_its_work_is_refused() {
        let terrain = hills();
        let pieces = [Piece { solid: &terrain, material: 0 }];
        let g = grid(0.1, 4);
        assert!(voxelise_chunk(&pieces, &g, ChunkKey::new(0, 0, 0)).unwrap().is_some());
        let why = voxelise_chunk_within(&pieces, &g, ChunkKey::new(0, 0, 0), 1000).unwrap_err();
        assert!(why.0.contains("more than 1000"), "{why}");
        // A material past the palette, and a bad supersampling, are refused.
        let bad = [Piece { solid: &terrain, material: 255 }];
        assert!(voxelise_chunk(&bad, &g, ChunkKey::new(0, 0, 0)).is_err());
        assert!(voxelise_chunk(&pieces, &grid(0.1, 0), ChunkKey::new(0, 0, 0)).is_err());
    }

    #[test]
    fn terrain_cover_is_conservative() {
        let terrain = hills();
        // Boxes of many sizes over the surface: an `Inside` box has every corner and middle inside,
        // an `Outside` box none.
        let mut said = [0usize; 3];
        for i in 0..4000u64 {
            let at = |n: u64| unit_from(99, 4 * i + n);
            let size = 0.01 + 1.5 * at(3).powi(2);
            let lo = Vec3::new(-4.0 + 14.0 * at(0), -5.0 + 13.0 * at(1), -0.8 + 2.6 * at(2));
            let b = Aabb::new(lo, lo + Vec3::new(size, size * 0.7, size * 0.5));
            let points: Vec<Vec3> = (0..8).map(|c| b.corner(c)).chain([b.centre()]).collect();
            match terrain.cover(&b) {
                Cover::Inside => {
                    said[0] += 1;
                    assert!(points.iter().all(|p| terrain.contains(*p)), "{b:?}");
                }
                Cover::Outside => {
                    said[1] += 1;
                    assert!(points.iter().all(|p| !terrain.contains(*p)), "{b:?}");
                }
                Cover::Mixed => said[2] += 1,
            }
        }
        assert!(said.iter().all(|&n| n > 200), "every answer is given: {said:?}");
        // Control: a terrain whose slope were taken as zero would say Inside where it is not.
        let mut flat_claim = 0;
        for i in 0..2000u64 {
            let at = |n: u64| unit_from(5, 3 * i + n);
            let lo = Vec3::new(-2.0 + 10.0 * at(0), -3.0 + 9.0 * at(1), 0.2 + 1.2 * at(2));
            let b = Aabb::new(lo, lo + Vec3::new(0.8, 0.8, 0.05));
            let h = terrain.height(b.centre().x, b.centre().y);
            if b.hi.z < h && !(0..8).all(|c| terrain.contains(b.corner(c))) {
                flat_claim += 1;
            }
        }
        assert!(flat_claim > 0, "the slope term is needed");
        // The height stays within its relief.
        for i in 0..1000u64 {
            let h = terrain.height(30.0 * unit_from(1, i), 30.0 * unit_from(2, i));
            assert!((h - terrain.level).abs() <= terrain.relief);
        }
    }

    #[test]
    fn a_coarse_brick_holds_its_children() {
        let terrain = hills();
        let house = cuboid([0.3, 0.1, 0.2], [2.05, 1.6, 2.4]);
        let pieces = [Piece { solid: &house, material: 2 }, Piece { solid: &terrain, material: 0 }];
        let g = grid(0.05, 2);
        // Level 0 from chunks, levels 1 to 3 from the pieces: each level's eight children's
        // cells, halved, are within the parent's.
        for level in 1..=3u8 {
            let parent_key = ChunkKey::new(0, 0, 0);
            let parent = brick_of_pieces(&pieces, &g, level, parent_key).unwrap();
            let mut children_cells = 0;
            for c in 0..8 {
                let key = ChunkKey::new(c & 1, c >> 1 & 1, c >> 2 & 1);
                let child = if level == 1 {
                    voxelise_chunk(&pieces, &g, key).unwrap().map(|chunk| Brick::of_chunk(&chunk))
                } else {
                    brick_of_pieces(&pieces, &g, level - 1, key)
                };
                let Some(child) = child else { continue };
                for z in 0..32 {
                    for y in 0..32 {
                        for x in 0..32 {
                            if child.get([x, y, z]) {
                                children_cells += 1;
                                let up = [
                                    (x + 32 * key.x as u32) / 2,
                                    (y + 32 * key.y as u32) / 2,
                                    (z + 32 * key.z as u32) / 2,
                                ];
                                assert!(parent.get(up), "level {level}: child {key:?} cell {:?}", [x, y, z]);
                            }
                        }
                    }
                }
            }
            assert!(children_cells > 1000, "{children_cells}");
            // Every set cell's sub-block is in the mask, with a material.
            for z in 0..32 {
                for y in 0..32 {
                    for x in 0..32 {
                        if parent.get([x, y, z]) {
                            assert!(parent.mask >> sub_index([x, y, z]) & 1 == 1);
                        }
                    }
                }
            }
        }
        // Equal for a box on the cells: a cell-aligned cuboid's level-1 brick is exactly the OR
        // of its level-0 cells. (Control: one cell cleared in the parent is seen.)
        let aligned = cuboid([-1.0 + 0.2, -2.0 + 0.4, -0.5 + 0.1], [-1.0 + 1.4, -2.0 + 1.0, -0.5 + 0.9]);
        let pieces = [Piece { solid: &aligned, material: 1 }];
        let mut parent = brick_of_pieces(&pieces, &g, 1, ChunkKey::new(0, 0, 0)).unwrap();
        let chunk = voxelise_chunk(&pieces, &g, ChunkKey::new(0, 0, 0)).unwrap().unwrap();
        let child = Brick::of_chunk(&chunk);
        let mut or = Brick::empty(1, ChunkKey::new(0, 0, 0));
        for z in 0..32 {
            for y in 0..32 {
                for x in 0..32 {
                    if child.get([x, y, z]) {
                        or.set([x / 2, y / 2, z / 2]);
                    }
                }
            }
        }
        // Cells of the cuboid whose boxes only touch it are `Mixed` and so set in the parent; the
        // parent may hold a rim more, never less.
        assert!(or.bits.iter().zip(&parent.bits).all(|(a, b)| a & !b == 0));
        assert!(
            parent.count() >= or.count() && parent.count() <= or.count() * 2,
            "{} {}",
            parent.count(),
            or.count()
        );
        let first = (0..Brick::WORDS).find(|&w| or.bits[w] != 0).unwrap();
        parent.bits[first] &= !or.bits[first];
        assert!(!or.bits.iter().zip(&parent.bits).all(|(a, b)| a & !b == 0));
    }

    #[test]
    fn a_chunks_moments_weigh_each_material_by_its_density() {
        let a = cuboid([0.0, 0.0, 0.0], [1.0, 2.0, 1.0]);
        let b = cuboid([1.0, 0.0, 0.0], [3.0, 2.0, 1.0]);
        let g = WorldGrid { origin: Vec3::ZERO, pitch: 0.125, supersample: 2 };
        let pieces = [Piece { solid: &a, material: 0 }, Piece { solid: &b, material: 1 }];
        let chunk = voxelise_chunk(&pieces, &g, ChunkKey::new(0, 0, 0)).unwrap().unwrap();
        let densities = [1000.0, 3000.0];
        let m = chunk.moments(&g, &densities);
        // 2 m³ at 1000 and 4 m³ at 3000: 14,000 kg, its centre at x = (2·0.5·1 + 12·2) / 14.
        assert!((m.mass - 14_000.0).abs() < 1e-9 * 14_000.0, "{}", m.mass);
        assert!((m.centre.x - (1000.0 + 24_000.0) / 14_000.0).abs() < 1e-12, "{:?}", m.centre);
        // Control: the tree's own moments weigh both alike and put the centre elsewhere.
        assert!((chunk.tree.body().centre.x - m.centre.x).abs() > 0.1);
        assert_eq!(chunk.palette, [0, 1]);
    }
}
