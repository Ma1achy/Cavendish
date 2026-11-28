//! The world as one octree of summaries: its chunks (level 0) merged upward into coarser nodes
//! (level 1, 2, …), each node a [`Summary`] of its matter about its own geometric centre
//! (`docs/slices/S5.md` §2 "The world octree"; `docs/research/s5-world-octree.md` §2, §3).
//!
//! **Keys.** Node [`NodeKey`] `(level, key)` is S4's brick key: a cube of `32·2^level` cells
//! ([`WorldGrid::brick_box`]); its parent is `(level + 1, key.parent())`
//! ([`super::clipmap::ancestor`] for several levels at once) and its octant is the low bit of each
//! coordinate (bit 0 +x, bit 1 +y, bit 2 +z, as a voxel tree's nodes).
//!
//! **The build** ([`Octree::build`]). The chunks that may hold matter are found top-down from the
//! world's extent: from the coarsest level at which the extent is at most two bricks a side, each
//! brick that [`World::reaches`] is split into the eight below it, down to chunks
//! ([`chunk_keys`]). Those are voxelised in parallel, a chunk to a thread at a time, on
//! [`crate::read::threads`] threads; each chunk's summary is summed from its cubes about the
//! chunk's centre; the chunk itself is then dropped. Chunks with no matter are left out. Levels
//! are made upward, each parent the [`Summary::sum`] of its children in octant order, until a
//! level has at most [`MAX_ROOTS`] nodes: those are the roots (a world that straddles the
//! lattice's planes through the origin keeps up to eight roots at every level, so a single root
//! is not asked for). The result is the same bits whatever the number of threads: each chunk's
//! summary depends on its key alone, and everything above is made in key order.
//!
//! **What it keeps.** Summaries and leaf counts only (360 bytes a node, [`Octree::bytes`]); no chunk's
//! cells. The walk ([`super::walk`]) makes again the chunks it opens, by key ([`chunk_cubes`]).
//!
//! **Caps** (`docs/slices/S5.md` §3.1, X3), counted before anything is reserved: at most
//! [`MAX_READ_CHUNKS_WHOLE`] chunks that may hold matter (and as many bricks at any level of the
//! search), and at most [`MAX_WORLD_NODES`] nodes, refused with words that name the number.
//!
//! Frames and units: the world's frame (x east, y north, z up), metres, kilograms.

use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use cav_core::voxel::Aabb;
use cav_core::voxel::summary::Summary;
use cav_core::voxel::world::{ChunkKey, WorldGrid};

use super::{Cube, World};

/// The most chunks a whole-world octree is made from (chunks that may hold matter), and the
/// most bricks of any one level its search for them keeps. `town` searches 5,454 and has 5,350
/// with matter (`docs/slices/S5.md` §7.2); 2^17 is 24 times that. Their summaries, held while
/// the levels are made, are at most 43 MB (328 bytes each).
pub const MAX_READ_CHUNKS_WHOLE: usize = 131_072;
/// The most nodes of all levels a whole-world octree may have. A world that is one layer of
/// chunks (terrain) has `1 + 1/4 + 1/16 + … = 4/3` nodes a chunk (`town`: 7,064 for 5,350, 1.32),
/// a solid one `8/7`; so `4/3` of [`MAX_READ_CHUNKS_WHOLE`] (174,763) and some room, at most 65 MB
/// at [`Octree::NODE_BYTES`] (360) each.
pub const MAX_WORLD_NODES: usize = 180_000;
/// Levels are made upward until one has at most this many nodes: the roots.
pub const MAX_ROOTS: usize = 8;
/// The coarsest level a node may have: a key's coordinate halved 30 times is 0 or −1.
pub const MAX_LEVEL: u8 = 30;

/// A node of the world's octree: its level (0 a chunk) and its key at that level (S4's brick key).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeKey {
    /// 0 for a chunk; a node of level `l` is `2^l` chunks a side.
    pub level: u8,
    /// Where it is, in units of its own side from the world's origin.
    pub key: ChunkKey,
}

impl NodeKey {
    /// The chunk `key` as a node.
    #[must_use]
    pub const fn chunk(key: ChunkKey) -> Self {
        Self { level: 0, key }
    }

    /// The node of the next level that holds this one.
    #[must_use]
    pub const fn parent(self) -> Self {
        Self { level: self.level + 1, key: self.key.parent() }
    }

    /// Which of its parent's eight it is: bit 0 for +x, bit 1 for +y, bit 2 for +z.
    #[must_use]
    pub fn octant(self) -> u8 {
        let bit = |v: i32| v.rem_euclid(2) as u8;
        bit(self.key.x) | bit(self.key.y) << 1 | bit(self.key.z) << 2
    }

    /// Its child in `octant` (of a node of level 1 or more).
    #[must_use]
    pub fn child(self, octant: u8) -> Self {
        let bit = |b: u8| i32::from((octant >> b) & 1);
        let k = self.key;
        Self {
            level: self.level.saturating_sub(1),
            key: ChunkKey::new(2 * k.x + bit(0), 2 * k.y + bit(1), 2 * k.z + bit(2)),
        }
    }
}

/// One node: its key, its summary about its own centre, and where its children are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OctNode {
    /// Where it is.
    pub key: NodeKey,
    /// Its matter about the centre of its box ([`Octree::boxed`]).
    pub summary: Summary,
    /// Which octants hold a child with matter (0 for a chunk).
    pub mask: u8,
    /// Where its children start in the octree's child list; as many as bits in `mask`.
    pub first_child: u32,
    /// How many leaf cubes its chunks hold in all: what opening every chunk under it would cost
    /// the kernel (the walk's work cap counts it before a chunk is made).
    pub leaves: u64,
}

/// The world's octree: nodes level by level from the chunks up, each level sorted by key.
#[derive(Clone, Debug, PartialEq)]
pub struct Octree {
    grid: WorldGrid,
    nodes: Vec<OctNode>,
    /// Level `l`'s nodes are `starts[l]..starts[l + 1]`.
    starts: Vec<usize>,
    /// Each node's children, as indices into `nodes`, a run per node in octant order.
    children: Vec<u32>,
}

impl Octree {
    /// The bytes one node takes.
    pub const NODE_BYTES: usize = size_of::<OctNode>();

    /// The octree of `world`'s matter, made on [`crate::read::threads`] threads. `cancelled` is
    /// asked between chunks; `Ok(None)` when it said so.
    ///
    /// # Errors
    /// When the world has more chunks or nodes than the caps, or a chunk cannot be voxelised.
    pub fn build(world: &World, cancelled: &(dyn Fn() -> bool + Sync)) -> Result<Option<Self>, String> {
        Self::build_on(crate::read::threads(), world, cancelled)
    }

    /// [`Self::build`] on `threads` threads (at least one). The result does not depend on it.
    ///
    /// # Errors
    /// As [`Self::build`].
    pub fn build_on(
        threads: usize,
        world: &World,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Option<Self>, String> {
        let Some(keys) = chunk_keys(world, cancelled)? else { return Ok(None) };
        check_nodes(&keys)?;
        let Some(chunks) = summaries(threads, world, &keys, cancelled)? else { return Ok(None) };
        Self::from_chunks(world.grid, chunks).map(Some)
    }

    /// The octree over the chunk summaries `chunks` (each `(key, summary about the chunk's
    /// centre, how many leaf cubes it has)`; empty ones dropped; any order, each key once) on
    /// `grid`: what [`Self::build`] makes once the chunks are summarised, and how a test or a base
    /// (TASK-S5-13) makes one from its own.
    ///
    /// # Errors
    /// When a key comes twice or there would be more than [`MAX_WORLD_NODES`] nodes.
    pub fn from_chunks(grid: WorldGrid, mut chunks: Vec<Counted>) -> Result<Self, String> {
        chunks.retain(|(_, s, _)| !s.is_empty());
        chunks.sort_by_key(|(key, _, _)| *key);
        if let Some(twice) = chunks.windows(2).find(|w| w[0].0 == w[1].0) {
            return Err(format!("chunk {:?} is given twice", twice[0].0.to_array()));
        }
        let keys: Vec<ChunkKey> = chunks.iter().map(|(k, _, _)| *k).collect();
        let total = check_nodes(&keys)?;
        let mut nodes = Vec::with_capacity(total);
        nodes.extend(chunks.into_iter().map(|(key, summary, leaves)| OctNode {
            key: NodeKey::chunk(key),
            summary,
            mask: 0,
            first_child: 0,
            leaves,
        }));
        let mut starts = vec![0, nodes.len()];
        let mut children = Vec::with_capacity(total.saturating_sub(keys.len()));
        let mut level = 0u8;
        while starts[starts.len() - 1] - starts[starts.len() - 2] > MAX_ROOTS && level < MAX_LEVEL {
            let below = starts[starts.len() - 2]..starts[starts.len() - 1];
            // Each child under its parent, in octant order; parents then come in key order.
            let mut order: Vec<(ChunkKey, u8, u32)> =
                below.map(|i| (nodes[i].key.key.parent(), nodes[i].key.octant(), i as u32)).collect();
            order.sort_unstable();
            let mut at = 0;
            while at < order.len() {
                let parent = order[at].0;
                let end = at + order[at..].iter().take_while(|o| o.0 == parent).count();
                let key = NodeKey { level: level + 1, key: parent };
                let parts: Vec<Summary> =
                    order[at..end].iter().map(|o| nodes[o.2 as usize].summary).collect();
                let centre = grid.brick_box(key.level, key.key).centre();
                let first_child = children.len() as u32;
                let mask = order[at..end].iter().fold(0u8, |m, o| m | 1 << o.1);
                let leaves = order[at..end].iter().map(|o| nodes[o.2 as usize].leaves).sum();
                children.extend(order[at..end].iter().map(|o| o.2));
                nodes.push(OctNode { key, summary: Summary::sum(centre, &parts), mask, first_child, leaves });
                at = end;
            }
            starts.push(nodes.len());
            level += 1;
        }
        Ok(Self { grid, nodes, starts, children })
    }

    /// The world's grid.
    #[must_use]
    pub const fn grid(&self) -> &WorldGrid {
        &self.grid
    }

    /// How many nodes it has, of all levels.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether it has no nodes (a world with no matter).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// How many levels it has (0 for an empty octree; 1 when the chunks are the roots).
    #[must_use]
    pub fn levels(&self) -> usize {
        if self.nodes.is_empty() { 0 } else { self.starts.len() - 1 }
    }

    /// The nodes of level `level`, as a range of node indices (empty past the top).
    #[must_use]
    pub fn level(&self, level: usize) -> Range<usize> {
        match (self.starts.get(level), self.starts.get(level + 1)) {
            (Some(&a), Some(&b)) => a..b,
            _ => 0..0,
        }
    }

    /// The roots: the top level's nodes, at most [`MAX_ROOTS`] (more only when the keys ran out
    /// of levels, which no world within the caps does).
    #[must_use]
    pub fn roots(&self) -> Range<usize> {
        self.level(self.levels().saturating_sub(1))
    }

    /// Node `id`.
    ///
    /// # Panics
    /// When `id` is not one of its nodes.
    #[must_use]
    pub fn node(&self, id: usize) -> &OctNode {
        &self.nodes[id]
    }

    /// Node `id`'s summary, about the centre of [`Self::boxed`].
    ///
    /// # Panics
    /// As [`Self::node`].
    #[must_use]
    pub fn summary(&self, id: usize) -> &Summary {
        &self.nodes[id].summary
    }

    /// Node `id`'s box, metres, the world's frame.
    ///
    /// # Panics
    /// As [`Self::node`].
    #[must_use]
    pub fn boxed(&self, id: usize) -> Aabb {
        let key = self.nodes[id].key;
        self.grid.brick_box(key.level, key.key)
    }

    /// Node `id`'s children with matter, as node indices, in octant order (empty for a chunk).
    ///
    /// # Panics
    /// As [`Self::node`].
    #[must_use]
    pub fn children(&self, id: usize) -> &[u32] {
        let node = &self.nodes[id];
        let first = node.first_child as usize;
        &self.children[first..first + node.mask.count_ones() as usize]
    }

    /// The chunk key of node `id` when it is a chunk (level 0).
    ///
    /// # Panics
    /// As [`Self::node`].
    #[must_use]
    pub fn chunk(&self, id: usize) -> Option<ChunkKey> {
        let key = self.nodes[id].key;
        (key.level == 0).then_some(key.key)
    }

    /// The index of the node `key`, if it has matter.
    #[must_use]
    pub fn find(&self, key: NodeKey) -> Option<usize> {
        let range = self.level(usize::from(key.level));
        let start = range.start;
        self.nodes[range].binary_search_by_key(&key.key, |n| n.key.key).ok().map(|i| start + i)
    }

    /// Nodes in each level, from the chunks up.
    #[must_use]
    pub fn per_level(&self) -> Vec<usize> {
        (0..self.levels()).map(|l| self.level(l).len()).collect()
    }

    /// The bytes it holds: its nodes, its child list and its level table.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.nodes.len() * Self::NODE_BYTES
            + self.children.len() * size_of::<u32>()
            + self.starts.len() * size_of::<usize>()
            + size_of::<Self>()
    }
}

/// Chunk `key`'s leaves as uniform cubes (empty when it holds nothing), made again from the
/// world's pieces: what the walk reads leaf by leaf when it opens a chunk, since the octree keeps
/// only summaries.
///
/// # Errors
/// When voxelising it passes its caps.
pub fn chunk_cubes(world: &World, key: ChunkKey) -> Result<Vec<Cube>, String> {
    Ok(world.chunk(key)?.map(|c| c.cubes(&world.grid, &world.densities)).unwrap_or_default())
}

/// Chunk `key`'s summary about its centre (empty when it holds nothing), and how many leaf cubes
/// it has.
///
/// # Errors
/// As [`chunk_cubes`].
pub fn chunk_summary(world: &World, key: ChunkKey) -> Result<(Summary, u64), String> {
    let centre = world.grid.chunk_box(key).centre();
    let cubes = chunk_cubes(world, key)?;
    let leaves = cubes.len() as u64;
    Ok((Summary::of_cubes(centre, cubes), leaves))
}

/// The keys, sorted, of the chunks of `world` that may hold matter: found top-down from the
/// coarsest level at which the world's extent is at most two bricks a side, keeping each brick
/// that [`World::reaches`] and splitting it into the eight below. Conservative: every chunk with
/// matter is among them, and some may turn out empty. `cancelled` is asked between levels.
///
/// # Errors
/// When some level of the search would keep more than [`MAX_READ_CHUNKS_WHOLE`] bricks.
pub fn chunk_keys(
    world: &World,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Option<Vec<ChunkKey>>, String> {
    let Some(extent) = world.extent() else { return Ok(Some(Vec::new())) };
    let grid = &world.grid;
    let span = |level: u8| {
        let (lo, hi) = grid.bricks_of(level, &extent);
        (lo, hi, [hi.x - lo.x, hi.y - lo.y, hi.z - lo.z].into_iter().all(|d| d <= 1))
    };
    let mut top = 0u8;
    while !span(top).2 && top < MAX_LEVEL {
        top += 1;
    }
    let (lo, hi, _) = span(top);
    let mut kept = Vec::new();
    for x in lo.x..=hi.x {
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                let key = ChunkKey::new(x, y, z);
                if world.reaches(&grid.brick_box(top, key)) {
                    kept.push(key);
                }
            }
        }
    }
    for level in (0..top).rev() {
        if cancelled() {
            return Ok(None);
        }
        let mut next = Vec::new();
        for key in &kept {
            for octant in 0..8 {
                let child = NodeKey { level: level + 1, key: *key }.child(octant).key;
                let boxed = grid.brick_box(level, child);
                if boxed.overlaps(&extent) && world.reaches(&boxed) {
                    if next.len() == MAX_READ_CHUNKS_WHOLE {
                        return Err(too_many(level));
                    }
                    next.push(child);
                }
            }
        }
        next.sort_unstable();
        kept = next;
    }
    Ok(Some(kept))
}

/// What a refusal of the search says.
fn too_many(level: u8) -> String {
    let what = if level == 0 { "chunks".to_owned() } else { format!("bricks of level {level}") };
    format!(
        "the world has more than {MAX_READ_CHUNKS_WHOLE} {what} that may hold matter: a whole-world read is made \
         from at most {MAX_READ_CHUNKS_WHOLE} chunks; use a coarser world.pitch or a smaller world"
    )
}

/// How many nodes an octree over the chunks `keys` (sorted, each once) has, levels made upward
/// until one has at most [`MAX_ROOTS`].
///
/// # Errors
/// When that is more than [`MAX_WORLD_NODES`], in words.
fn check_nodes(keys: &[ChunkKey]) -> Result<usize, String> {
    let mut level: Vec<ChunkKey> = keys.to_vec();
    let mut total = level.len();
    let mut depth = 0u8;
    while level.len() > MAX_ROOTS && depth < MAX_LEVEL {
        for key in &mut level {
            *key = key.parent();
        }
        level.dedup();
        // Parents of sorted keys need not be sorted along y and z: sort, then drop repeats again.
        level.sort_unstable();
        level.dedup();
        total += level.len();
        depth += 1;
        if total > MAX_WORLD_NODES {
            break;
        }
    }
    if total > MAX_WORLD_NODES {
        return Err(format!(
            "the world's octree would have more than {MAX_WORLD_NODES} nodes ({} chunks at its finest level); use a \
             coarser world.pitch or a smaller world",
            keys.len()
        ));
    }
    Ok(total)
}

/// A chunk's key, its summary about its centre and how many leaf cubes it has.
pub type Counted = (ChunkKey, Summary, u64);

/// One chunk's summary and leaf count, or why it could not be made, by the key's index.
type Found = (usize, Result<(Summary, u64), String>);

/// The summaries of the chunks `keys` that hold matter, in key order, made on `threads` threads;
/// `Ok(None)` when cancelled. Keys are handed out in order, so when one fails every earlier key
/// has been made too, and the failure reported (the first by key) is the same on any number of
/// threads.
fn summaries(
    threads: usize,
    world: &World,
    keys: &[ChunkKey],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Option<Vec<Counted>>, String> {
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let was_cancelled = AtomicBool::new(false);
    let found: Mutex<Vec<Found>> = Mutex::new(Vec::with_capacity(keys.len()));
    let work = || {
        let mut mine: Vec<Found> = Vec::new();
        while !stop.load(Ordering::Relaxed) {
            let at = next.fetch_add(1, Ordering::Relaxed);
            if at >= keys.len() {
                break;
            }
            if cancelled() {
                was_cancelled.store(true, Ordering::Relaxed);
                stop.store(true, Ordering::Relaxed);
                break;
            }
            let made = chunk_summary(world, keys[at]);
            if made.is_err() {
                stop.store(true, Ordering::Relaxed);
            }
            mine.push((at, made));
        }
        found.lock().unwrap_or_else(PoisonError::into_inner).extend(mine);
    };
    let workers = threads.clamp(1, keys.len().max(1));
    std::thread::scope(|scope| {
        for _ in 1..workers {
            scope.spawn(work);
        }
        work();
    });
    if was_cancelled.load(Ordering::Relaxed) {
        return Ok(None);
    }
    let mut found = found.into_inner().unwrap_or_else(PoisonError::into_inner);
    found.sort_by_key(|(at, _)| *at);
    let mut out = Vec::with_capacity(found.len());
    for (at, made) in found {
        let (summary, leaves) = made.map_err(|why| format!("chunk {:?}: {why}", keys[at].to_array()))?;
        if !summary.is_empty() {
            out.push((keys[at], summary, leaves));
        }
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests;
