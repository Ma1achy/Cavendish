//! The rings tier: a cut made by distance alone, at radii measured from the ledger walk
//! (`docs/slices/S5.md` §2 "Measured radii", TASK-S5-14; `docs/research/s5-world-octree.md` §5).
//!
//! **Levels.** A node's ring level is the base-2 logarithm of its side in chunks: a block of a
//! chunk's tree of `2^k` cells is level `k − 5` (−4 to −1, [`super::inner`]), a chunk 0, an
//! octree node of level `L` is `L`. [`Radii`] holds, for each ring level, the least distance from a
//! node's centre to the atoms' path, in its own sides, at which a node of that level may be read
//! by its expansion.
//!
//! **Measuring** ([`measured`]). The ledger walk ([`super::walk::walk_with`]) is run at each of a
//! set of tower positions; for each level, the nearest distance (in sides) at which it accepted a
//! node of that level, the least over the positions, is the level's radius. A level it accepted at
//! no position has no radius: the rings never accept it.
//!
//! **The walk** ([`rings_with`], [`rings_into`]). From the roots, a node is accepted when its
//! level has a radius, its centre is at least that many of its sides from the path, its bound is
//! finite ([`super::walk::Q_MAX`]) and it is outside the bubble; otherwise it is opened (into its
//! children, a chunk into its tree, a block into its parts), and a chunk with no tree at hand is
//! read whole. The cut is the same kind as the ledger walk's ([`super::walk::Cut`]) and its ledger
//! is reported the same way (the accepted nodes' bounds): it may be above the tolerance, since the
//! rings are not held to it. **What it must not be used for:** a tolerance its table was not
//! measured for and checked at ([`RINGS`]); the gate is `validation_slow_the_rings_reproduce_their_tolerance`.
//!
//! Frames and units: the world's frame, metres, radians; radii in a node's own sides.

use cav_core::voxel::summary::Summary;
use cav_core::voxel::world::ChunkKey;

use super::inner::{ChunkTree, InnerKey, Part};
use super::octree::{NodeKey, Octree};
use super::walk::{Aim, Cut, summary_bound, walk_with};
use super::{World, box_to_segment, to_segment};

/// The lowest ring level: a block of 2 cells is `1 − 5`.
pub const LOWEST: i8 = -4;
/// How many ring levels a table holds: −4 (blocks of 2 cells) to 11 (octree level 11).
pub const LEVELS: usize = 16;

/// For each ring level from [`LOWEST`], the least distance (in the node's own sides) at which a node
/// of that level is read by its expansion; infinite: never.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Radii {
    /// The tolerance it was measured at, radians.
    pub tolerance: f64,
    /// The radius of each level, from [`LOWEST`] up, in sides.
    pub sides: [f64; LEVELS],
}

impl Radii {
    /// The radius of ring level `level`, in sides (infinite past the table).
    #[must_use]
    pub fn of(&self, level: i8) -> f64 {
        usize::try_from(i16::from(level) - i16::from(LOWEST))
            .ok()
            .and_then(|i| self.sides.get(i))
            .copied()
            .unwrap_or(f64::INFINITY)
    }

    /// Every radius times `factor`: for controls (`factor` ½), never for a read.
    #[must_use]
    pub fn scaled(&self, factor: f64) -> Self {
        Self { tolerance: self.tolerance, sides: self.sides.map(|r| r * factor) }
    }
}

const NEVER: f64 = f64::INFINITY;

/// The stored table: radii measured on `town` at its eight positions (`town_s_radii_measured`,
/// `docs/slices/S5.md` §7.11), each rounded up to a tenth of a side, one row a tolerance, the
/// largest tolerance first; levels −4 (blocks of 2 cells) up. A read asks for the row of the largest tolerance at most its own.
pub const RINGS: [Radii; 3] = [
    Radii {
        tolerance: 1e-2,
        sides: [
            17.8, 17.1, 23.5, 22.6, 20.2, 20.3, NEVER, NEVER, NEVER, NEVER, NEVER, NEVER, NEVER, NEVER,
            NEVER, NEVER,
        ],
    },
    Radii {
        tolerance: 1e-4,
        sides: [
            50.4, 43.2, 52.8, 51.3, 46.4, NEVER, NEVER, NEVER, NEVER, NEVER, NEVER, NEVER, NEVER, NEVER,
            NEVER, NEVER,
        ],
    },
    Radii {
        tolerance: 1e-6,
        sides: [
            119.3, 103.6, 132.8, 119.4, NEVER, NEVER, NEVER, NEVER, NEVER, NEVER, NEVER, NEVER, NEVER, NEVER,
            NEVER, NEVER,
        ],
    },
];

/// The stored row for a read at `tolerance`: the row of the largest tolerance at most it; `None`
/// when it is below every row's.
#[must_use]
pub fn row(tolerance: f64) -> Option<&'static Radii> {
    RINGS.iter().find(|r| r.tolerance <= tolerance)
}

/// The ring level of an octree node.
fn octree_level(key: NodeKey) -> i8 {
    i8::try_from(key.level).unwrap_or(i8::MAX)
}

/// The ring level of a block.
fn block_level(key: InnerKey) -> i8 {
    i8::try_from(key.level).unwrap_or(0) - 5
}

/// The side of a node of ring level `level`, metres, on a grid of chunks `chunk` metres a side.
fn side(chunk: f64, level: i8) -> f64 {
    chunk * 2f64.powi(i32::from(level))
}

/// Radii measured from the ledger walk at each of `aims` (the trees of every chunk the walks open
/// in `trees`, sorted by key; see [`super::walk::walk_into`]) at `tolerance`: the module's rule.
#[must_use]
pub fn measured(octree: &Octree, trees: &[ChunkTree], aims: &[Aim], tolerance: f64) -> Radii {
    let mut sides = [NEVER; LEVELS];
    let chunk = octree.grid().chunk_side();
    let mut note = |level: i8, summary: &Summary, aim: &Aim| {
        let i = usize::try_from(i16::from(level) - i16::from(LOWEST)).unwrap_or(LEVELS);
        if i < LEVELS {
            let r = to_segment(summary.centre, aim.base, aim.height) / side(chunk, level);
            sides[i] = sides[i].min(r);
        }
    };
    for aim in aims {
        let cut = walk_with(octree, &Aim { tolerance, ..*aim }, trees);
        for key in &cut.accepted {
            note(octree_level(*key), octree.summary(octree.find(*key).expect("a node of the octree")), aim);
        }
        for key in &cut.inner {
            let t = &trees
                [trees.binary_search_by_key(&key.chunk, |t| t.key).expect("the tree of an opened chunk")];
            note(block_level(*key), &t.node(t.find(*key).expect("a block of the tree")).summary, aim);
        }
    }
    Radii { tolerance, sides }
}

/// The rings' cut of `octree` for the tower `aim` describes at `radii`, each chunk it opens opened
/// into its tree when `trees` (sorted by key) holds it and read whole otherwise (the module's
/// rule). `aim.tolerance` is not used: the rings are held to their table, not to a ledger.
/// Deterministic: the cut's lists are sorted, and its ledger is summed in key order.
#[must_use]
pub fn rings_with(octree: &Octree, aim: &Aim, trees: &[ChunkTree], radii: &Radii) -> Cut {
    let chunk = octree.grid().chunk_side();
    let accept = |summary: &Summary, boxed: &cav_core::voxel::Aabb, level: i8| -> Option<f64> {
        if box_to_segment(boxed, aim.base, aim.height) <= aim.bubble {
            return None;
        }
        let far = to_segment(summary.centre, aim.base, aim.height) >= radii.of(level) * side(chunk, level);
        let bound = summary_bound(summary, aim);
        (far && bound.is_finite()).then_some(bound)
    };
    let mut accepted: Vec<(NodeKey, f64)> = Vec::new();
    let mut inner: Vec<(InnerKey, f64)> = Vec::new();
    let (mut opened, mut whole, mut runs) = (Vec::new(), Vec::new(), Vec::new());
    let mut leaves = 0u64;
    let mut stack: Vec<usize> = octree.roots().collect();
    let mut blocks: Vec<(usize, u32)> = Vec::new();
    let parts = |t: usize,
                 list: &[Part],
                 blocks: &mut Vec<(usize, u32)>,
                 runs: &mut Vec<(ChunkKey, std::ops::Range<u32>)>,
                 leaves: &mut u64| {
        for part in list {
            match part {
                Part::Leaves(r) => {
                    *leaves += u64::from(r.end - r.start);
                    runs.push((trees[t].key, r.clone()));
                }
                Part::Node(n) => blocks.push((t, *n)),
            }
        }
    };
    while let Some(id) = stack.pop() {
        let node = octree.node(id);
        if let Some(bound) = accept(&node.summary, &octree.boxed(id), octree_level(node.key)) {
            accepted.push((node.key, bound));
            continue;
        }
        let children = octree.children(id);
        if !children.is_empty() {
            stack.extend(children.iter().map(|&c| c as usize));
            continue;
        }
        let key = node.key.key;
        opened.push(key);
        match trees.binary_search_by_key(&key, |t| t.key) {
            Ok(t) => parts(t, trees[t].parts(), &mut blocks, &mut runs, &mut leaves),
            Err(_) => {
                whole.push(key);
                leaves += node.leaves;
            }
        }
    }
    while let Some((t, n)) = blocks.pop() {
        let block = trees[t].node(n);
        if let Some(bound) = accept(&block.summary, &block.boxed, block_level(block.key)) {
            inner.push((block.key, bound));
        } else {
            parts(t, trees[t].children(n), &mut blocks, &mut runs, &mut leaves);
        }
    }
    accepted.sort_unstable_by_key(|(k, _)| *k);
    inner.sort_unstable_by_key(|(k, _)| *k);
    opened.sort_unstable();
    whole.sort_unstable();
    runs.sort_unstable_by_key(|(k, r): &(ChunkKey, std::ops::Range<u32>)| (*k, r.start));
    // The ledger in the ledger walk's order: octree nodes by key, then blocks by key.
    let ledger = accepted.iter().map(|(_, b)| b).chain(inner.iter().map(|(_, b)| b)).sum();
    let mut per_level = vec![0usize; octree.levels()];
    for (key, _) in &accepted {
        per_level[usize::from(key.level)] += 1;
    }
    let mut inner_per_level = [0usize; 4];
    for (key, _) in &inner {
        inner_per_level[usize::from(key.level).clamp(1, 4) - 1] += 1;
    }
    let bounds = accepted.iter().map(|(_, b)| *b).chain(inner.iter().map(|(_, b)| *b)).collect();
    Cut {
        bounds,
        accepted: accepted.into_iter().map(|(k, _)| k).collect(),
        inner: inner.into_iter().map(|(k, _)| k).collect(),
        opened,
        whole,
        runs,
        ledger,
        settled: true,
        stopped: None,
        per_level,
        inner_per_level,
        leaves,
    }
}

/// [`rings_with`] with every chunk it opens opened into its tree: the trees of the chunks it read
/// whole are made ([`super::open`]) and added to `trees` (kept sorted), and again, as
/// [`super::walk::walk_into`] does. `Ok(None)` when cancelled.
///
/// # Errors
/// When a chunk cannot be voxelised.
pub fn rings_into(
    octree: &Octree,
    aim: &Aim,
    world: &World,
    trees: &mut Vec<ChunkTree>,
    radii: &Radii,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Option<Cut>, String> {
    loop {
        let cut = rings_with(octree, aim, trees, radii);
        if cut.whole.is_empty() {
            return Ok(Some(cut));
        }
        let Some(made) = super::open(world, &cut.whole, cancelled)? else { return Ok(None) };
        trees.extend(made);
        trees.sort_unstable_by_key(|t| t.key);
    }
}

/// The error of a cut's accepted nodes and blocks read by their expansion, against their own leaves
/// read with the exact kernel, through `reader` at `base` (readout 0), radians: the worst of the
/// two interferometers and the gradiometer. The leaves a cut reads one by one are the same in both
/// reads, so this is the cut's error against the leaf-exact read of the whole world, without the
/// rounding of two whole-world sums (as W2 forms it). `trees` holds every opened chunk's tree and
/// those of the chunks under every accepted node.
///
/// # Panics
/// When a tree it needs is not in `trees`.
#[must_use]
pub fn cut_error(
    octree: &Octree,
    trees: &[ChunkTree],
    cut: &Cut,
    reader: &cav_core::forward::Reader,
    base: cav_core::Vec3,
) -> f64 {
    let tree_of =
        |k: &ChunkKey| &trees[trees.binary_search_by_key(k, |t| t.key).expect("the tree of a chunk")];
    let mut cubes = Vec::new();
    let mut stack: Vec<usize> =
        cut.accepted.iter().map(|k| octree.find(*k).expect("a node of the octree")).collect();
    while let Some(id) = stack.pop() {
        match octree.chunk(id) {
            Some(key) => cubes.extend_from_slice(&tree_of(&key).cubes),
            None => stack.extend(octree.children(id).iter().map(|&c| c as usize)),
        }
    }
    for key in &cut.inner {
        let t = tree_of(&key.chunk);
        let r = &t.node(t.find(*key).expect("a block of the tree")).leaves;
        cubes.extend_from_slice(&t.cubes[r.start as usize..r.end as usize]);
    }
    let rule = super::walk::Rule::Octupole;
    let leaves = super::WorldSource { cubes, far: super::walk::Far { nodes: Vec::new(), rule }.into() };
    let exact = reader.read(base, &leaves, 0.0);
    let got = reader.read(base, &super::walk::far_field(octree, trees, cut, rule), 0.0);
    let arm = |e: f64, g: f64| (g - e).abs();
    arm(exact.lower, got.lower)
        .max(arm(exact.upper, got.upper))
        .max(arm(exact.lower - exact.upper, got.lower - got.upper))
}

#[cfg(test)]
mod tests;
