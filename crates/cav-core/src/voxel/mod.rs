//! Matter as a sparse voxel tree: one representation for every body made of voxels.
//!
//! A body's shape is voxelised at a pitch `h` into a **linear octree** ([`VoxTree`]): leaves in
//! Morton order, each an aligned cube of `2^level` finest cells holding an integer `count` (how
//! many of a finest cell's `k³` sample points are inside the shape) and a material; internal
//! nodes made from the leaves' keys, each with the mass moments of what is under it. The design
//! is the research note's (`docs/research/s3-sparse-voxel-structures.md`, with its review), and
//! `reference/design/voxel.md` is the record it was re-derived from. The S3 brief
//! (`docs/slices/S3.md` §2) says why each choice was made.
//!
//! - [`morton`]: 30-bit keys, so a tree is at most 1024 cells across.
//! - [`shape`]: what can be voxelised, and the conservative `cover` of a box.
//! - [`mesh`]: closed triangle meshes, read from OBJ and STL.
//! - [`voxelise`](voxelise()): the top-down voxeliser, and [`raster`], the oracle it must equal.
//! - [`tree`]: the tree, its moments, its cuts.
//! - [`kernel`]: the potential of a uniform cube, exact to rounding (series or prism).
//!
//! Frames and units: a tree is in its body's own frame, metres, x east, y north, z up when the
//! body is unrotated; masses are kilograms. Every count that comes from a spec or a file has a
//! cap here, checked before anything is reserved.

pub mod kernel;
pub mod mesh;
pub mod morton;
pub mod shape;
pub mod tree;
mod voxeliser;

#[cfg(test)]
mod tests;

use crate::Vec3;

pub use shape::{Cover, Shape};
pub use tree::{Leaf, Moments, Node, VoxTree};
pub use voxeliser::{Raster, raster, unmerged, voxelise, voxelise_within};

/// The most leaves one tree may have. Leaves are eight bytes, so 16 MiB.
pub const MAX_LEAVES: usize = 2_000_000;
/// The most internal nodes one tree may have, each with its moments (about 172 bytes).
pub const MAX_TREE_NODES: usize = 1_000_000;
/// The most box tests and sample tests one voxelisation may make, together: what bounds its time.
pub const MAX_VOXEL_WORK: u64 = 500_000_000;
/// The finest supersampling: `k` sample points along each axis of a finest cell, `k³` in all,
/// which must fit a byte.
pub const MAX_SUPERSAMPLE: u8 = 6;
/// The most cells the raster oracle classifies (one byte each). It is the test's voxeliser, not
/// the engine's.
pub const MAX_RASTER_CELLS: u64 = 1 << 27;

/// The caps one voxelisation works within. [`Budget::DEFAULT`] is the engine's; a smaller one is
/// for tests of the refusals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// The most leaves.
    pub leaves: usize,
    /// The most internal nodes.
    pub nodes: usize,
    /// The most box tests and sample tests, together.
    pub work: u64,
}

impl Budget {
    /// The engine's caps: [`MAX_LEAVES`], [`MAX_TREE_NODES`], [`MAX_VOXEL_WORK`].
    pub const DEFAULT: Self = Self { leaves: MAX_LEAVES, nodes: MAX_TREE_NODES, work: MAX_VOXEL_WORK };
}

/// Why a voxelisation or a mesh was refused, in words that name the number at fault.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoxelError(pub String);

impl std::fmt::Display for VoxelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for VoxelError {}

/// An axis-aligned box: `lo` and `hi` corners, metres, in a body's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    /// The corner with the smallest x, y and z.
    pub lo: Vec3,
    /// The corner with the largest x, y and z.
    pub hi: Vec3,
}

impl Aabb {
    /// The box from `lo` to `hi`.
    #[must_use]
    pub const fn new(lo: Vec3, hi: Vec3) -> Self {
        Self { lo, hi }
    }

    /// The middle of the box.
    #[must_use]
    pub fn centre(&self) -> Vec3 {
        (self.lo + self.hi) * 0.5
    }

    /// The box's extent along each axis, metres.
    #[must_use]
    pub fn size(&self) -> Vec3 {
        self.hi - self.lo
    }

    /// Corner `i` (0 to 7): bit 0 picks x from `hi`, bit 1 y, bit 2 z.
    #[must_use]
    pub fn corner(&self, i: u32) -> Vec3 {
        Vec3::new(
            if i & 1 == 0 { self.lo.x } else { self.hi.x },
            if i & 2 == 0 { self.lo.y } else { self.hi.y },
            if i & 4 == 0 { self.lo.z } else { self.hi.z },
        )
    }

    /// Whether this box and `other` share any point (touching counts).
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.lo.x <= other.hi.x
            && other.lo.x <= self.hi.x
            && self.lo.y <= other.hi.y
            && other.lo.y <= self.hi.y
            && self.lo.z <= other.hi.z
            && other.lo.z <= self.hi.z
    }

    /// The box grown by `by` metres on every side.
    #[must_use]
    pub fn grown(&self, by: f64) -> Self {
        let d = Vec3::new(by, by, by);
        Self { lo: self.lo - d, hi: self.hi + d }
    }

    /// The smallest box holding both.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        Self {
            lo: Vec3::new(self.lo.x.min(other.lo.x), self.lo.y.min(other.lo.y), self.lo.z.min(other.lo.z)),
            hi: Vec3::new(self.hi.x.max(other.hi.x), self.hi.y.max(other.hi.y), self.hi.z.max(other.hi.z)),
        }
    }
}

/// Where a tree's finest cells are: a regular grid of cubes of side `pitch`, the root a cube of
/// `2^depth` of them, with corner `origin`.
///
/// The grid is centred on the shape's box ([`Grid::around`]), so a symmetric shape sits
/// symmetrically in it. Each finest cell is sampled at `k³` points on a regular lattice
/// ([`Grid::sample`]); the voxeliser and the raster oracle both ask the shape about exactly those
/// points, which is what makes them agree bit for bit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    /// The corner of cell `[0, 0, 0]`, metres, in the body's frame.
    pub origin: Vec3,
    /// The side of a finest cell, metres.
    pub pitch: f64,
    /// The root's level: the root is `2^depth` finest cells across, at most 1024 (depth 10).
    pub depth: u8,
    /// How many cells along x, y and z the shape's box needs (each at most `2^depth`): the
    /// cells beyond are empty.
    pub cells: [u32; 3],
    /// `k`: sample points along each axis of a finest cell, from 1 to [`MAX_SUPERSAMPLE`].
    pub supersample: u8,
}

impl Grid {
    /// The grid of `pitch` metres that holds `bounds`, centred on it, sampled `supersample` times
    /// along each axis of a cell.
    ///
    /// # Errors
    /// When the pitch is not a positive number, the supersampling is outside 1 to
    /// [`MAX_SUPERSAMPLE`], the box is not finite, or the box needs more than 1024 cells along
    /// an axis (a finer pitch than one tree can hold), with the numbers.
    pub fn around(bounds: Aabb, pitch: f64, supersample: u8) -> Result<Self, VoxelError> {
        if !(pitch.is_finite() && pitch > 0.0) {
            return Err(VoxelError(format!("pitch: {pitch} m must be a positive number")));
        }
        if !(1..=MAX_SUPERSAMPLE).contains(&supersample) {
            return Err(VoxelError(format!(
                "supersample: {supersample} must be from 1 to {MAX_SUPERSAMPLE}"
            )));
        }
        let size = bounds.size();
        let extent = [size.x, size.y, size.z];
        if !extent.iter().all(|e| e.is_finite() && *e >= 0.0) {
            return Err(VoxelError(format!("the shape's box {bounds:?} is not finite")));
        }
        let mut cells = [1u32; 3];
        for axis in 0..3 {
            // A whole number of cells to the last part in 10⁹: a 0.225 m wall at 7.5 cm is three
            // cells, not four.
            let needed = (extent[axis] / pitch * (1.0 - 1e-9)).ceil().max(1.0);
            if needed > f64::from(morton::SIDE) {
                return Err(VoxelError(format!(
                    "pitch: {pitch} m needs {needed} cells across the shape's {} m, more than the {} one \
                     tree holds; use a pitch of at least {} m",
                    extent[axis],
                    morton::SIDE,
                    extent[axis] / f64::from(morton::SIDE)
                )));
            }
            cells[axis] = needed as u32;
        }
        let widest = cells.iter().copied().max().unwrap_or(1);
        let depth = (32 - (widest - 1).leading_zeros()) as u8;
        let centre = bounds.centre();
        let half = |axis: usize| 0.5 * f64::from(cells[axis]) * pitch;
        let origin = Vec3::new(centre.x - half(0), centre.y - half(1), centre.z - half(2));
        Ok(Self { origin, pitch, depth, cells, supersample })
    }

    /// The coordinate of sample `sub` (0 to k − 1) of cell `cell` along one axis whose cells start
    /// at `origin`: `origin + pitch·(cell + (sub + ½)/k)`. Rising in `(cell, sub)`.
    #[must_use]
    pub fn coordinate(&self, origin: f64, cell: u32, sub: u32) -> f64 {
        let k = f64::from(self.supersample);
        origin + self.pitch * (f64::from(cell) + (f64::from(sub) + 0.5) / k)
    }

    /// Sample point `sub` (each 0 to k − 1) of the finest cell `cell`, metres, the body's frame.
    #[must_use]
    pub fn sample(&self, cell: [u32; 3], sub: [u32; 3]) -> Vec3 {
        Vec3::new(
            self.coordinate(self.origin.x, cell[0], sub[0]),
            self.coordinate(self.origin.y, cell[1], sub[1]),
            self.coordinate(self.origin.z, cell[2], sub[2]),
        )
    }

    /// The smallest box holding every sample point of the block of `side` finest cells whose
    /// first cell is `corner`. Its corners are sample points, computed as [`Self::sample`]
    /// computes them, so every sample of the block is inside it exactly.
    #[must_use]
    pub fn sample_box(&self, corner: [u32; 3], side: u32) -> Aabb {
        let last = self.supersample.saturating_sub(1).into();
        let end = corner.map(|c| c + side - 1);
        Aabb::new(self.sample(corner, [0; 3]), self.sample(end, [last; 3]))
    }

    /// The box of the block of `side` finest cells whose first cell is `corner`: its faces are
    /// cell faces, metres, the body's frame.
    #[must_use]
    pub fn block_box(&self, corner: [u32; 3], side: u32) -> Aabb {
        let at = |axis: usize, c: u32| {
            let o = [self.origin.x, self.origin.y, self.origin.z][axis];
            o + self.pitch * f64::from(c)
        };
        Aabb::new(
            Vec3::new(at(0, corner[0]), at(1, corner[1]), at(2, corner[2])),
            Vec3::new(at(0, corner[0] + side), at(1, corner[1] + side), at(2, corner[2] + side)),
        )
    }

    /// `k³`: the sample points of one finest cell, and so the count of a full cell.
    #[must_use]
    pub fn samples_per_cell(&self) -> u32 {
        u32::from(self.supersample).pow(3)
    }

    /// Whether an aligned block whose first cell is `corner` has any cell inside the grid's used
    /// region ([`Self::cells`]): blocks start at their smallest cell, so it does when that cell
    /// is inside along every axis.
    #[must_use]
    pub fn touches_used(&self, corner: [u32; 3]) -> bool {
        (0..3).all(|axis| corner[axis] < self.cells[axis])
    }
}
