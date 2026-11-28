//! A chunk opened into its own voxel tree's blocks, each with a summary and so a bound, so that
//! the walk goes on below level 0 instead of reading every leaf of every chunk it opens
//! (`docs/slices/S5.md` §7.9, TASK-S5-27).
//!
//! **What a [`ChunkTree`] is.** A chunk's leaves (aligned cubes of `2^l` finest cells, in Morton
//! order, as the voxeliser makes them) and, over them, the aligned **blocks** of its tree: a block
//! of level `k` (`1 ≤ k ≤ 4`) is `2^k` cells a side, its leaves one run of the chunk's. A block
//! with at least [`MIN_INNER_LEAVES`] leaves is a [`InnerNode`], with its [`Summary`] made
//! directly from its leaves about the block's geometric centre (so `A_2` and `A_4` exact, the rest
//! at each leaf's far corner, and the reach the leaves' own); a smaller block is not a node: its
//! leaves are read one by one, as a node of a few leaves would cost as much as they do. The chunk
//! (level 5) is the octree's own node; [`ChunkTree::parts`] gives what is under it, and
//! [`ChunkTree::children`] what is under a node.
//!
//! **What must not be done.** The blocks' summaries are of the chunk's leaves as voxelised (the
//! read's matter), never of the pieces; a tree is made for one chunk of one world and its leaf
//! ranges index that chunk's cubes only.
//!
//! Frames and units: the world's frame (x east, y north, z up), metres, kilograms.

use std::ops::Range;

use cav_core::voxel::Aabb;
use cav_core::voxel::summary::Summary;
use cav_core::voxel::world::{CHUNK_DEPTH, Chunk, ChunkKey};

use super::{Cube, World};

/// A block of fewer leaves than this is not a node of a chunk's tree: its leaves are read one by
/// one. A node read by its expansion costs about four leaves' kernel ([`super::walk`]'s work), so
/// a node of fewer than eight saves little and costs its summary (328 bytes).
pub const MIN_INNER_LEAVES: usize = 2;

/// Where a block of a chunk's tree is: its chunk, its level (`2^level` cells a side, 1 to 4) and
/// the Morton key of its first finest cell in the chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InnerKey {
    /// Its chunk.
    pub chunk: ChunkKey,
    /// Its level: a block of `2^level` finest cells a side, 1 to 4.
    pub level: u8,
    /// The Morton key of its first finest cell, in the chunk's own cells.
    pub key: u32,
}

/// How a block's summary is made from its leaves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tightness {
    /// About the centre of the box of its leaves, not of its block.
    pub centres: bool,
    /// Its even absolute moments `A_6 … A_16` the leaves' exact ones
    /// ([`cav_core::voxel::summary::cubes_absolute`], odd orders by Cauchy–Schwarz), where they
    /// are below the far-corner values.
    pub even: bool,
}

impl Tightness {
    /// The summary of `cubes` (a block's leaves) whose block is centred at `centre`.
    #[must_use]
    pub fn summary(self, centre: cav_core::Vec3, cubes: &[Cube]) -> Summary {
        let centre = if self.centres {
            let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
            for &(at, _, side) in cubes {
                for (a, v) in at.to_array().into_iter().enumerate() {
                    lo[a] = lo[a].min(v - 0.5 * side);
                    hi[a] = hi[a].max(v + 0.5 * side);
                }
            }
            cav_core::Vec3::new(0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]), 0.5 * (lo[2] + hi[2]))
        } else {
            centre
        };
        let mut out = Summary::of_cubes(centre, cubes.iter().copied());
        if self.even {
            let exact = cav_core::voxel::summary::cubes_absolute(centre, cubes.iter().copied());
            for (a, e) in out.absolute.iter_mut().zip(exact) {
                *a = a.min(e);
            }
        }
        out
    }
}

/// One part of what lies under a chunk or a block: a node, or a run of leaves read one by one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Part {
    /// Node `index` of the chunk's tree ([`ChunkTree::node`]).
    Node(u32),
    /// The chunk's leaves in this range ([`ChunkTree::cubes`]).
    Leaves(Range<u32>),
}

/// A block of a chunk's tree with enough leaves to be a node.
#[derive(Clone, Debug, PartialEq)]
pub struct InnerNode {
    /// Where it is.
    pub key: InnerKey,
    /// Its matter, about the centre of its block.
    pub summary: Summary,
    /// Its block, metres, the world's frame.
    pub boxed: Aabb,
    /// Its leaves: a run of the chunk's.
    pub leaves: Range<u32>,
    /// Its parts, a run of [`ChunkTree`]'s part list.
    parts: Range<u32>,
}

/// A chunk's leaves and the nodes of its tree.
#[derive(Clone, Debug, PartialEq)]
pub struct ChunkTree {
    /// Its key.
    pub key: ChunkKey,
    /// Its leaves as uniform cubes, in Morton order.
    pub cubes: Vec<Cube>,
    nodes: Vec<InnerNode>,
    /// Every node's parts in runs, and last the chunk's own.
    parts: Vec<Part>,
    /// The chunk's own parts: a run of `parts`.
    top: Range<u32>,
}

impl ChunkTree {
    /// The tree of `chunk` (on `world`'s grid, its masses by `world`'s densities).
    #[must_use]
    pub fn of(world: &World, chunk: &Chunk) -> Self {
        Self::of_with(world, chunk, Tightness::default())
    }

    /// [`Self::of`] with the blocks' summaries made as `tight` says: for measuring what each way
    /// of tightening them is worth (`docs/slices/S5.md` §7.9).
    #[must_use]
    pub fn of_with(world: &World, chunk: &Chunk, tight: Tightness) -> Self {
        let cubes = chunk.cubes(&world.grid, &world.densities);
        let leaves: Vec<(u32, u8)> = chunk.tree.leaves.iter().map(|l| (l.key, l.level)).collect();
        let mut out = Self { key: chunk.key, cubes, nodes: Vec::new(), parts: Vec::new(), top: 0..0 };
        let all = 0..out.cubes.len() as u32;
        let top = out.split(chunk, &leaves, 0, CHUNK_DEPTH, all, tight);
        let start = out.parts.len() as u32;
        out.parts.extend(top);
        out.top = start..out.parts.len() as u32;
        out
    }

    /// An empty chunk's tree (no leaves).
    #[must_use]
    pub const fn empty(key: ChunkKey) -> Self {
        Self { key, cubes: Vec::new(), nodes: Vec::new(), parts: Vec::new(), top: 0..0 }
    }

    /// The parts of the block of level `level` whose first cell is `first`, holding the leaves
    /// `run`: each of its eight octants with leaves, as a node, or as its leaves.
    fn split(
        &mut self,
        chunk: &Chunk,
        leaves: &[(u32, u8)],
        first: u32,
        level: u8,
        run: Range<u32>,
        tight: Tightness,
    ) -> Vec<Part> {
        let slice = &leaves[run.start as usize..run.end as usize];
        // One leaf covering the block: it is read as itself.
        if slice.len() == 1 && slice[0].1 >= level {
            return vec![Part::Leaves(run)];
        }
        let span = 1u32 << (3 * (u32::from(level) - 1));
        let mut out = Vec::new();
        let mut at = run.start;
        for octant in 0..8u32 {
            let lo = first + octant * span;
            let end = run.start + slice.partition_point(|&(key, _)| key < lo + span) as u32;
            if end > at {
                let child = at..end;
                let count = (end - at) as usize;
                let level = level - 1;
                if level == 0 || count < MIN_INNER_LEAVES {
                    out.push(Part::Leaves(child));
                } else {
                    let parts = self.split(chunk, leaves, lo, level, child.clone(), tight);
                    let start = self.parts.len() as u32;
                    self.parts.extend(parts);
                    let centre = chunk.tree.block_centre(lo, level);
                    let half = 0.5 * chunk.tree.side(level);
                    let corner = cav_core::Vec3::new(half, half, half);
                    let boxed = Aabb::new(centre - corner, centre + corner);
                    let cubes = &self.cubes[child.start as usize..child.end as usize];
                    let summary = tight.summary(centre, cubes);
                    self.nodes.push(InnerNode {
                        key: InnerKey { chunk: self.key, level, key: lo },
                        summary,
                        boxed,
                        leaves: child,
                        parts: start..self.parts.len() as u32,
                    });
                    out.push(Part::Node(self.nodes.len() as u32 - 1));
                }
            }
            at = end;
        }
        out
    }

    /// What lies under the chunk itself.
    #[must_use]
    pub fn parts(&self) -> &[Part] {
        &self.parts[self.top.start as usize..self.top.end as usize]
    }

    /// Node `index`.
    ///
    /// # Panics
    /// When `index` is not one of its nodes.
    #[must_use]
    pub fn node(&self, index: u32) -> &InnerNode {
        &self.nodes[index as usize]
    }

    /// What lies under node `index`.
    ///
    /// # Panics
    /// As [`Self::node`].
    #[must_use]
    pub fn children(&self, index: u32) -> &[Part] {
        let r = &self.nodes[index as usize].parts;
        &self.parts[r.start as usize..r.end as usize]
    }

    /// How many nodes it has.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether it has no nodes (its leaves are all read one by one).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The index of the node `key`, if it is one.
    #[must_use]
    pub fn find(&self, key: InnerKey) -> Option<u32> {
        self.nodes.iter().position(|n| n.key == key).map(|i| i as u32)
    }

    /// The bytes it holds: its cubes, nodes and parts.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.cubes.len() * size_of::<Cube>()
            + self.nodes.len() * size_of::<InnerNode>()
            + self.parts.len() * size_of::<Part>()
            + size_of::<Self>()
    }
}

#[cfg(test)]
mod tests;
