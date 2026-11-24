//! The top-down voxeliser, and the raster voxeliser it must equal bit for bit.
//!
//! Both ask the shape about the same sample points ([`Grid::sample`]): `k³` points on a regular
//! lattice inside each finest cell, a cell's `count` being how many are inside. The raster
//! classifies every cell of the grid; the top-down voxeliser asks [`Shape::cover`] about whole
//! blocks first and samples only the cells of blocks it calls `Mixed`. Because `cover` is
//! conservative for the very points the raster samples, the two agree count for count.

use super::morton::{self, span};
use super::shape::{Cover, Shape};
use super::tree::{Leaf, VoxTree};
use super::{Budget, Grid, MAX_RASTER_CELLS, VoxelError};

/// The counts of every finest cell of a grid's used region, one byte a cell: the raster
/// voxeliser's result, and what a tree expands to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raster {
    /// Cells along x, y and z: the grid's used region.
    pub cells: [u32; 3],
    /// The count of each cell, x fastest, then y, then z.
    pub counts: Vec<u8>,
}

impl Raster {
    /// The index of `cell` in `counts`.
    #[must_use]
    pub fn index(&self, cell: [u32; 3]) -> usize {
        let [nx, ny, _] = self.cells.map(|c| c as usize);
        cell[0] as usize + nx * (cell[1] as usize + ny * cell[2] as usize)
    }

    /// The tree expanded to its finest cells. Leaves that reach outside the used region (none
    /// should) are reported as an error.
    ///
    /// # Errors
    /// When the used region has more than [`MAX_RASTER_CELLS`] cells, or a leaf has a cell outside it.
    pub fn of_tree(tree: &VoxTree) -> Result<Self, VoxelError> {
        let mut out = Self::empty(tree.grid.cells)?;
        for leaf in &tree.leaves {
            let corner = morton::decode(leaf.key);
            let side = 1u32 << leaf.level;
            for z in corner[2]..corner[2] + side {
                for y in corner[1]..corner[1] + side {
                    for x in corner[0]..corner[0] + side {
                        if x >= out.cells[0] || y >= out.cells[1] || z >= out.cells[2] {
                            return Err(VoxelError(format!("leaf {leaf:?} reaches outside the used cells")));
                        }
                        let i = out.index([x, y, z]);
                        out.counts[i] = leaf.count;
                    }
                }
            }
        }
        Ok(out)
    }

    fn empty(cells: [u32; 3]) -> Result<Self, VoxelError> {
        let total = cells.iter().map(|&c| u64::from(c)).product::<u64>();
        if total > MAX_RASTER_CELLS {
            return Err(VoxelError(format!(
                "{total} cells is more than the raster oracle holds, {MAX_RASTER_CELLS}"
            )));
        }
        Ok(Self { cells, counts: vec![0; total as usize] })
    }
}

/// How many of the `k³` sample points of `cell` are inside `shape`.
fn count(shape: &dyn Shape, grid: &Grid, cell: [u32; 3]) -> u8 {
    let k = u32::from(grid.supersample);
    let mut inside = 0u8;
    for c in 0..k {
        for b in 0..k {
            for a in 0..k {
                inside += u8::from(shape.contains(grid.sample(cell, [a, b, c])));
            }
        }
    }
    inside
}

/// The raster voxeliser: every finest cell of `grid`'s used region, counted. The oracle the tree
/// is held to; it touches every cell, so it is for tests and small grids.
///
/// # Errors
/// When the used region has more than [`MAX_RASTER_CELLS`] cells.
pub fn raster(shape: &dyn Shape, grid: &Grid) -> Result<Raster, VoxelError> {
    let mut out = Raster::empty(grid.cells)?;
    for z in 0..grid.cells[2] {
        for y in 0..grid.cells[1] {
            for x in 0..grid.cells[0] {
                let i = out.index([x, y, z]);
                out.counts[i] = count(shape, grid, [x, y, z]);
            }
        }
    }
    Ok(out)
}

/// The top-down voxeliser: `shape` on `grid` as a tree of `mass` kilograms, every leaf of
/// material `material`.
///
/// From the root down: a block the shape covers wholly is one full leaf, a block it misses is
/// dropped, a mixed block is split into its eight children (in Morton order, so the leaves come
/// out sorted), and at the finest level the sample points are counted. A block whose eight
/// children came back as eight equal leaves becomes one leaf of its own level: the lossless merge.
///
/// # Errors
/// When the work would pass [`MAX_VOXEL_WORK`], the leaves [`MAX_LEAVES`], or the tree's own caps
/// ([`VoxTree::build`]); when the shape has no sample point inside at this pitch.
pub fn voxelise(shape: &dyn Shape, grid: &Grid, mass: f64, material: u8) -> Result<VoxTree, VoxelError> {
    voxelise_within(shape, grid, mass, material, Budget::DEFAULT)
}

/// [`voxelise`] within `budget`'s caps in place of the engine's: for tests of the refusals.
///
/// # Errors
/// As [`voxelise`], with `budget`'s numbers.
pub fn voxelise_within(
    shape: &dyn Shape,
    grid: &Grid,
    mass: f64,
    material: u8,
    budget: Budget,
) -> Result<VoxTree, VoxelError> {
    let mut walk = Walk { shape, grid, material, leaves: Vec::new(), work: 0, budget };
    walk.visit(0, grid.depth)?;
    VoxTree::build_within(*grid, walk.leaves, mass, budget)
}

struct Walk<'a> {
    shape: &'a dyn Shape,
    grid: &'a Grid,
    material: u8,
    leaves: Vec<Leaf>,
    work: u64,
    budget: Budget,
}

impl Walk<'_> {
    fn spend(&mut self, work: u64) -> Result<(), VoxelError> {
        self.work += work;
        if self.work > self.budget.work {
            return Err(VoxelError(format!(
                "voxelising would take more than {} box and sample tests: use a coarser pitch or less \
                 supersampling",
                self.budget.work
            )));
        }
        Ok(())
    }

    fn emit(&mut self, leaf: Leaf) -> Result<(), VoxelError> {
        if self.leaves.len() >= self.budget.leaves {
            return Err(VoxelError(format!(
                "the tree would have more than {} leaves: use a coarser pitch",
                self.budget.leaves
            )));
        }
        let bytes = (self.leaves.len() + 1) * size_of::<Leaf>();
        if bytes > self.budget.bytes {
            return Err(VoxelError(format!(
                "the tree's leaves alone would pass the {} bytes allowed it: use a coarser pitch",
                self.budget.bytes
            )));
        }
        self.leaves.push(leaf);
        Ok(())
    }

    fn visit(&mut self, key: u32, level: u8) -> Result<(), VoxelError> {
        let corner = morton::decode(key);
        if !self.grid.touches_used(corner) {
            return Ok(());
        }
        let side = 1u32 << level;
        // A block that reaches past the used region is never `Inside`: the used region holds the
        // shape's box, so such a block has sample points outside the shape's box, and `cover` is
        // conservative. (S3 had a guard here for it, which no test could reach: S3's review.)
        self.spend(1)?;
        let cover = self.shape.cover(&self.grid.sample_box(corner, side));
        let full = self.grid.samples_per_cell() as u8;
        match cover {
            Cover::Outside => Ok(()),
            Cover::Inside => self.emit(Leaf { key, level, count: full, material: self.material }),
            _ if level == 0 => {
                self.spend(u64::from(self.grid.samples_per_cell()))?;
                let inside = count(self.shape, self.grid, corner);
                if inside > 0 {
                    self.emit(Leaf { key, level, count: inside, material: self.material })?;
                }
                Ok(())
            }
            _ => {
                let before = self.leaves.len();
                for octant in 0..8 {
                    self.visit(morton::child(key, level, octant), level - 1)?;
                }
                // The lossless merge: eight equal leaves of the level below are one leaf.
                let new = &self.leaves[before..];
                if new.len() == 8
                    && new.iter().all(|l| {
                        l.level == level - 1 && l.count == new[0].count && l.material == new[0].material
                    })
                {
                    let merged = Leaf { key, level, count: new[0].count, material: new[0].material };
                    self.leaves.truncate(before);
                    self.leaves.push(merged);
                }
                Ok(())
            }
        }
    }
}

/// The leaves of `tree` split back to finest cells, for tests of the merge: the same body, unmerged.
#[must_use]
pub fn unmerged(tree: &VoxTree) -> Vec<Leaf> {
    let mut out = Vec::new();
    for leaf in &tree.leaves {
        for i in 0..span(leaf.level) {
            out.push(Leaf { key: leaf.key + i, level: 0, ..*leaf });
        }
    }
    out
}
