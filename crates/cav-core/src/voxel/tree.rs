//! The tree: leaves in Morton order, internal nodes made from their keys, and the mass moments of
//! every node.
//!
//! **Layout.** `leaves` is sorted by key; a leaf is an aligned cube of `2^level` finest cells.
//! `nodes` is breadth-first from the root; each node's children (a leaf or a node for each
//! occupied octant, in octant order) are a run of `children`, and every leaf under a node is one
//! run of `leaves` (Morton order keeps a block's leaves together). A tree that is one cube has no
//! node at all: its root is its only leaf.
//!
//! **Values.** A leaf holds an integer count of sample points and a material; its mass is
//! `count × 8^level × unit` kilograms, with `unit` the body's mass over the tree's samples. A leaf
//! is a uniform cube of that mass (the element law of `reference/design/voxel.md` §0).
//!
//! **Moments**, one record a node, in `f64`, bottom-up: mass, centre of mass, and the second and
//! third central moments of everything under it, each leaf a uniform cube.

use crate::Vec3;

use super::morton::{self, decode, span};
use super::{Grid, MAX_LEAVES, MAX_TREE_NODES, VoxelError};

/// One leaf: an aligned cube of `2^level` finest cells, every one holding `count` sample points of
/// the shape and the same material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Leaf {
    /// The Morton key of its first (smallest) finest cell; its low `3·level` bits are zero.
    pub key: u32,
    /// Its level: 0 for a finest cell, `depth` for the root.
    pub level: u8,
    /// How many of each finest cell's `k³` sample points are inside, from 1 to `k³`; the same for
    /// every cell of a merged leaf.
    pub count: u8,
    /// Its material: an index into the body's palette.
    pub material: u8,
}

impl Leaf {
    /// How many finest cells it spans: `8^level`.
    #[must_use]
    pub const fn cells(&self) -> u32 {
        span(self.level)
    }

    /// Its sample points inside the shape: `count × 8^level`.
    #[must_use]
    pub const fn samples(&self) -> u64 {
        self.count as u64 * span(self.level) as u64
    }

    /// The key just past its last finest cell.
    #[must_use]
    pub const fn end(&self) -> u64 {
        self.key as u64 + span(self.level) as u64
    }
}

/// An entry of `children`: a leaf or a node, by index. The top bit says which.
const NODE_FLAG: u32 = 1 << 31;

/// A child of a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Child {
    /// Leaf `n` of `leaves`.
    Leaf(usize),
    /// Node `n` of `nodes`.
    Node(usize),
}

/// One internal node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    /// The Morton key of its first finest cell.
    pub key: u32,
    /// Its level: its block is `2^level` finest cells across.
    pub level: u8,
    /// Which octants hold something: bit `o` for octant `o` (bit 0 of `o` is +x, bit 1 +y, bit 2 +z).
    pub mask: u8,
    /// Where its children start in `children`; there are as many as bits in `mask`.
    pub first_child: u32,
    /// The run of `leaves` under it: `first_leaf..end_leaf`.
    pub first_leaf: u32,
    /// Just past the last leaf under it.
    pub end_leaf: u32,
}

/// The mass moments of a node or a leaf, in the body's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moments {
    /// Kilograms.
    pub mass: f64,
    /// The centre of mass, metres, the body's frame.
    pub centre: Vec3,
    /// Second central moments `Σ m (x_a − c_a)(x_b − c_b)`, kg m², in the order xx, yy, zz, xy,
    /// xz, yz. A uniform cube of side `s` and mass `m` has `m s²/12` on the diagonal.
    pub second: [f64; 6],
    /// Third central moments, kg m³, in the order xxx, yyy, zzz, xxy, xxz, xyy, yyz, xzz, yzz, xyz.
    pub third: [f64; 10],
}

/// The position of the moment `ab` in [`Moments::second`].
const fn second_index(a: usize, b: usize) -> usize {
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    match (lo, hi) {
        (0, 0) => 0,
        (1, 1) => 1,
        (2, 2) => 2,
        (0, 1) => 3,
        (0, 2) => 4,
        _ => 5,
    }
}

/// The position of the moment `abc` in [`Moments::third`].
const fn third_index(a: usize, b: usize, c: usize) -> usize {
    // Sort the three axes.
    let (mut x, mut y, mut z) = (a, b, c);
    if x > y {
        (x, y) = (y, x);
    }
    if y > z {
        (y, z) = (z, y);
    }
    if x > y {
        (x, y) = (y, x);
    }
    match (x, y, z) {
        (0, 0, 0) => 0,
        (1, 1, 1) => 1,
        (2, 2, 2) => 2,
        (0, 0, 1) => 3,
        (0, 0, 2) => 4,
        (0, 1, 1) => 5,
        (1, 1, 2) => 6,
        (0, 2, 2) => 7,
        (1, 2, 2) => 8,
        _ => 9,
    }
}

impl Moments {
    /// A uniform cube of `mass` kilograms and side `side` metres centred at `centre`.
    #[must_use]
    pub fn cube(mass: f64, side: f64, centre: Vec3) -> Self {
        let own = mass * side * side / 12.0;
        Self { mass, centre, second: [own, own, own, 0.0, 0.0, 0.0], third: [0.0; 10] }
    }

    /// The second moment `ab` (axes 0, 1, 2 for x, y, z).
    #[must_use]
    pub const fn second_at(&self, a: usize, b: usize) -> f64 {
        self.second[second_index(a, b)]
    }

    /// The third moment `abc`.
    #[must_use]
    pub const fn third_at(&self, a: usize, b: usize, c: usize) -> f64 {
        self.third[third_index(a, b, c)]
    }

    /// The moments of several parts together, moved to their common centre of mass by the
    /// parallel-axis rules. Summed in the order given.
    #[must_use]
    pub fn combine(parts: &[Self]) -> Self {
        let mass: f64 = parts.iter().map(|p| p.mass).sum();
        let weighted = parts.iter().fold(Vec3::ZERO, |sum, p| sum + p.centre * p.mass);
        let centre = weighted * (1.0 / mass);
        let mut second = [0.0; 6];
        let mut third = [0.0; 10];
        for part in parts {
            let d = part.centre - centre;
            let d = [d.x, d.y, d.z];
            for a in 0..3 {
                for b in a..3 {
                    second[second_index(a, b)] += part.second_at(a, b) + part.mass * d[a] * d[b];
                }
            }
            for (a, b, c) in THIRD {
                third[third_index(a, b, c)] += part.third_at(a, b, c)
                    + part.second_at(a, b) * d[c]
                    + part.second_at(a, c) * d[b]
                    + part.second_at(b, c) * d[a]
                    + part.mass * d[a] * d[b] * d[c];
            }
        }
        Self { mass, centre, second, third }
    }

    /// The inertia tensor about the centre of mass, kg m², as `[xx, yy, zz, xy, xz, yz]`:
    /// `I = tr(S)·1 − S` from the second moments `S`.
    #[must_use]
    pub fn inertia(&self) -> [f64; 6] {
        let s = self.second;
        let trace = s[0] + s[1] + s[2];
        [trace - s[0], trace - s[1], trace - s[2], -s[3], -s[4], -s[5]]
    }
}

/// The ten distinct third moments, as axis triples in the order of [`Moments::third`].
const THIRD: [(usize, usize, usize); 10] = [
    (0, 0, 0),
    (1, 1, 1),
    (2, 2, 2),
    (0, 0, 1),
    (0, 0, 2),
    (0, 1, 1),
    (1, 1, 2),
    (0, 2, 2),
    (1, 2, 2),
    (0, 1, 2),
];

/// A body's voxel tree.
#[derive(Clone, Debug, PartialEq)]
pub struct VoxTree {
    /// Where its cells are.
    pub grid: Grid,
    /// The leaves, sorted by key.
    pub leaves: Vec<Leaf>,
    /// The internal nodes, breadth-first from the root (node 0); empty when the root is a leaf.
    pub nodes: Vec<Node>,
    /// The nodes' children, a run for each node (see [`Node::first_child`]).
    pub children: Vec<u32>,
    /// The moments of each node, in the order of `nodes`.
    pub moments: Vec<Moments>,
    /// Kilograms a sample point: the body's mass over the tree's samples.
    pub unit: f64,
    /// Every leaf's sample points: `Σ count × 8^level`, exactly.
    pub samples: u64,
}

impl VoxTree {
    /// The tree of `leaves` on `grid`, for a body of `mass` kilograms: the leaves sorted (in any
    /// order given, the tree is the same bytes), the nodes made from their keys, the moments
    /// bottom-up.
    ///
    /// # Errors
    /// When there are no leaves, more than [`MAX_LEAVES`] of them, or their nodes would be more
    /// than [`MAX_TREE_NODES`] (counted before the moments are reserved); when a leaf is not
    /// aligned, overlaps another, lies outside the root, or has a count outside 1 to `k³`; when the
    /// mass is not a positive number.
    pub fn build(grid: Grid, leaves: Vec<Leaf>, mass: f64) -> Result<Self, VoxelError> {
        Self::build_within(grid, leaves, mass, MAX_TREE_NODES)
    }

    /// [`Self::build`] with at most `max_nodes` internal nodes in place of [`MAX_TREE_NODES`].
    ///
    /// # Errors
    /// As [`Self::build`].
    pub fn build_within(
        grid: Grid,
        mut leaves: Vec<Leaf>,
        mass: f64,
        max_nodes: usize,
    ) -> Result<Self, VoxelError> {
        if leaves.is_empty() {
            return Err(VoxelError(
                "no sample point of the shape is inside it at this pitch: use a finer pitch".into(),
            ));
        }
        if leaves.len() > MAX_LEAVES {
            return Err(VoxelError(format!(
                "{} leaves is more than a tree holds, {MAX_LEAVES}",
                leaves.len()
            )));
        }
        if !(mass.is_finite() && mass > 0.0) {
            return Err(VoxelError(format!("mass: {mass} kg must be a positive number")));
        }
        leaves.sort_unstable_by_key(|leaf| leaf.key);
        let full = grid.samples_per_cell();
        let root_end = u64::from(span(grid.depth));
        let mut previous_end = 0u64;
        let mut samples = 0u64;
        for leaf in &leaves {
            if leaf.level > grid.depth || morton::block(leaf.key, leaf.level) != leaf.key {
                return Err(VoxelError(format!("leaf {leaf:?} is not an aligned block of this grid")));
            }
            if u64::from(leaf.key) < previous_end || leaf.end() > root_end {
                return Err(VoxelError(format!("leaf {leaf:?} overlaps another or lies outside the root")));
            }
            if leaf.count == 0 || u32::from(leaf.count) > full {
                return Err(VoxelError(format!("leaf {leaf:?}: a count is from 1 to {full}")));
            }
            previous_end = leaf.end();
            samples += leaf.samples();
        }
        let unit = mass / samples as f64;
        let (nodes, children) = Self::link(&grid, &leaves, max_nodes)?;
        let mut tree = Self { grid, leaves, nodes, children, moments: Vec::new(), unit, samples };
        tree.moments = tree.bottom_up();
        Ok(tree)
    }

    /// The nodes and their children, breadth-first from the root.
    fn link(grid: &Grid, leaves: &[Leaf], max_nodes: usize) -> Result<(Vec<Node>, Vec<u32>), VoxelError> {
        let mut nodes = Vec::new();
        let mut children = Vec::new();
        if leaves.len() == 1 && leaves[0].level == grid.depth {
            return Ok((nodes, children));
        }
        // Count first, so that nothing is reserved for a tree that will be refused: a node is a
        // block of level ≥ 1 holding a leaf of a lower level.
        let mut count = 0usize;
        Self::visit_blocks(leaves, grid.depth, &mut |_, _, _| count += 1);
        if count > max_nodes {
            return Err(VoxelError(format!(
                "{count} nodes is more than a tree holds, {max_nodes}: use a coarser pitch"
            )));
        }
        nodes.reserve_exact(count);
        nodes.push(Node {
            key: 0,
            level: grid.depth,
            mask: 0,
            first_child: 0,
            first_leaf: 0,
            end_leaf: leaves.len() as u32,
        });
        let mut at = 0;
        while at < nodes.len() {
            let node = nodes[at];
            nodes[at].first_child = children.len() as u32;
            let mut mask = 0u8;
            for octant in 0..8u32 {
                let key = morton::child(node.key, node.level, octant);
                let end = u64::from(key) + u64::from(span(node.level - 1));
                let range = node.first_leaf as usize..node.end_leaf as usize;
                let from = range.start + leaves[range.clone()].partition_point(|l| l.key < key);
                let to = range.start + leaves[range].partition_point(|l| u64::from(l.key) < end);
                if from == to {
                    continue;
                }
                mask |= 1 << octant;
                if to - from == 1 && leaves[from].level == node.level - 1 {
                    children.push(from as u32);
                } else {
                    children.push(nodes.len() as u32 | NODE_FLAG);
                    nodes.push(Node {
                        key,
                        level: node.level - 1,
                        mask: 0,
                        first_child: 0,
                        first_leaf: from as u32,
                        end_leaf: to as u32,
                    });
                }
            }
            nodes[at].mask = mask;
            at += 1;
        }
        debug_assert_eq!(nodes.len(), count);
        Ok((nodes, children))
    }

    /// Calls `found(key, level, leaves)` for every block that would be a node: every block of
    /// level ≥ 1, from the root down, that holds more than one leaf or one leaf of a lower level.
    fn visit_blocks(leaves: &[Leaf], level: u8, found: &mut dyn FnMut(u32, u8, usize)) {
        fn walk(leaves: &[Leaf], key: u32, level: u8, found: &mut dyn FnMut(u32, u8, usize)) {
            if leaves.is_empty() || (leaves.len() == 1 && leaves[0].level == level) {
                return;
            }
            found(key, level, leaves.len());
            for octant in 0..8u32 {
                let child = morton::child(key, level, octant);
                let end = u64::from(child) + u64::from(span(level - 1));
                let from = leaves.partition_point(|l| l.key < child);
                let to = leaves.partition_point(|l| u64::from(l.key) < end);
                walk(&leaves[from..to], child, level - 1, found);
            }
        }
        walk(leaves, 0, level, found);
    }

    /// The children of node `n`, in octant order.
    pub fn children_of(&self, n: usize) -> impl Iterator<Item = Child> + '_ {
        let node = self.nodes[n];
        let count = node.mask.count_ones() as usize;
        let first = node.first_child as usize;
        self.children[first..first + count].iter().map(|&c| {
            if c & NODE_FLAG == 0 { Child::Leaf(c as usize) } else { Child::Node((c & !NODE_FLAG) as usize) }
        })
    }

    /// The finest cell at which leaf `leaf` starts, `[x, y, z]`.
    #[must_use]
    pub fn leaf_corner(&self, leaf: &Leaf) -> [u32; 3] {
        decode(leaf.key)
    }

    /// The side of a block of level `level`, metres.
    #[must_use]
    pub fn side(&self, level: u8) -> f64 {
        self.grid.pitch * f64::from(1u32 << level)
    }

    /// The centre of the block of level `level` whose key is `key`, metres, the body's frame.
    #[must_use]
    pub fn block_centre(&self, key: u32, level: u8) -> Vec3 {
        let [x, y, z] = decode(key);
        let half = 0.5 * f64::from(1u32 << level);
        let o = self.grid.origin;
        let h = self.grid.pitch;
        Vec3::new(
            o.x + h * (f64::from(x) + half),
            o.y + h * (f64::from(y) + half),
            o.z + h * (f64::from(z) + half),
        )
    }

    /// A leaf's mass, kilograms: `count × 8^level × unit`.
    #[must_use]
    pub fn leaf_mass(&self, leaf: &Leaf) -> f64 {
        leaf.samples() as f64 * self.unit
    }

    /// A leaf's moments: a uniform cube.
    #[must_use]
    pub fn leaf_moments(&self, leaf: &Leaf) -> Moments {
        Moments::cube(self.leaf_mass(leaf), self.side(leaf.level), self.block_centre(leaf.key, leaf.level))
    }

    /// The moments of every node, children before parents.
    fn bottom_up(&self) -> Vec<Moments> {
        let empty = Moments { mass: 0.0, centre: Vec3::ZERO, second: [0.0; 6], third: [0.0; 10] };
        let mut moments = vec![empty; self.nodes.len()];
        let mut parts = Vec::with_capacity(8);
        for n in (0..self.nodes.len()).rev() {
            parts.clear();
            for child in self.children_of(n) {
                parts.push(match child {
                    Child::Leaf(l) => self.leaf_moments(&self.leaves[l]),
                    Child::Node(m) => moments[m],
                });
            }
            moments[n] = Moments::combine(&parts);
        }
        moments
    }

    /// The moments of the whole body: the root's, or its one leaf's.
    #[must_use]
    pub fn body(&self) -> Moments {
        self.moments.first().copied().unwrap_or_else(|| self.leaf_moments(&self.leaves[0]))
    }

    /// The moments of node `n` summed directly over its leaves, in two passes (the centre, then
    /// the central moments): the check of [`Self::moments`], for tests.
    #[must_use]
    pub fn direct_moments(&self, n: usize) -> Moments {
        let node = self.nodes[n];
        let leaves = &self.leaves[node.first_leaf as usize..node.end_leaf as usize];
        let parts: Vec<Moments> = leaves.iter().map(|leaf| self.leaf_moments(leaf)).collect();
        let mass: f64 = parts.iter().map(|p| p.mass).sum();
        let centre = parts.iter().fold(Vec3::ZERO, |sum, p| sum + p.centre * p.mass) * (1.0 / mass);
        let mut second = [0.0; 6];
        let mut third = [0.0; 10];
        for p in &parts {
            let d = p.centre - centre;
            let d = [d.x, d.y, d.z];
            for a in 0..3 {
                for b in a..3 {
                    second[second_index(a, b)] += p.second_at(a, b) + p.mass * d[a] * d[b];
                }
            }
            for (a, b, c) in THIRD {
                // A cube's own second moment is diagonal: it adds to `aac`-type terms only.
                third[third_index(a, b, c)] += p.second_at(a, b) * d[c]
                    + p.second_at(a, c) * d[b]
                    + p.second_at(b, c) * d[a]
                    + p.mass * d[a] * d[b] * d[c];
            }
        }
        Moments { mass, centre, second, third }
    }

    /// The cut at level `level`: every internal node of that level, and every leaf of that level
    /// or above. Each leaf of the tree is under exactly one entry of the cut, so the cut holds
    /// all the body's mass (a level of detail drops structure, never mass). In Morton order.
    #[must_use]
    pub fn cut(&self, level: u8) -> Vec<CutEntry> {
        let mut out = Vec::new();
        if self.nodes.is_empty() {
            out.push(CutEntry::Leaf(0));
            return out;
        }
        let mut stack = vec![0usize];
        while let Some(n) = stack.pop() {
            if self.nodes[n].level <= level {
                out.push(CutEntry::Node(n));
                continue;
            }
            let children: Vec<Child> = self.children_of(n).collect();
            for child in children.into_iter().rev() {
                match child {
                    Child::Leaf(l) => out.push(CutEntry::Leaf(l)),
                    Child::Node(m) => stack.push(m),
                }
            }
        }
        out.sort_by_key(|entry| match *entry {
            CutEntry::Leaf(l) => self.leaves[l].key,
            CutEntry::Node(n) => self.nodes[n].key,
        });
        out
    }

    /// The bytes the tree holds, roughly: what the engine's cache counts.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.leaves.len() * size_of::<Leaf>()
            + self.nodes.len() * (size_of::<Node>() + size_of::<Moments>())
            + self.children.len() * size_of::<u32>()
    }
}

/// One entry of a cut ([`VoxTree::cut`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CutEntry {
    /// Leaf `n`, drawn and weighed as itself.
    Leaf(usize),
    /// Node `n`, standing for every leaf under it.
    Node(usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::{Aabb, Grid};

    /// A grid of 8 cells across (depth 3), `k = 2`.
    fn grid() -> Grid {
        Grid::around(Aabb::new(Vec3::ZERO, Vec3::new(8.0, 8.0, 8.0)), 1.0, 2).unwrap()
    }

    /// A small, uneven set of leaves: a merged block, single cells, partial counts.
    fn leaves() -> Vec<Leaf> {
        let mut out = vec![Leaf { key: morton::encode([0, 0, 0]), level: 2, count: 8, material: 0 }];
        for (cell, count) in [([4, 0, 0], 3), ([5, 1, 0], 8), ([4, 4, 4], 1), ([7, 7, 7], 5), ([0, 6, 2], 2)]
        {
            out.push(Leaf { key: morton::encode(cell), level: 0, count, material: 0 });
        }
        out.push(Leaf { key: morton::encode([6, 0, 4]), level: 1, count: 4, material: 1 });
        out
    }

    #[test]
    fn a_tree_is_the_same_whatever_order_its_leaves_come_in() {
        let a = VoxTree::build(grid(), leaves(), 3.0).unwrap();
        let mut reversed = leaves();
        reversed.reverse();
        let b = VoxTree::build(grid(), reversed, 3.0).unwrap();
        let mut shuffled = leaves();
        shuffled.swap(0, 4);
        shuffled.swap(2, 6);
        let c = VoxTree::build(grid(), shuffled, 3.0).unwrap();
        assert_eq!(a, b);
        assert_eq!(a, c);
        // Breadth-first: every node's children come after it, and levels never rise.
        for (n, node) in a.nodes.iter().enumerate().skip(1) {
            assert!(node.level <= a.nodes[n - 1].level);
        }
        // Control: another leaf set is another tree.
        let mut other = leaves();
        other[3].count = 2;
        assert_ne!(VoxTree::build(grid(), other, 3.0).unwrap(), a);
        // Refusals: overlap, misalignment, a count of zero or above k³, nothing at all.
        let mut overlap = leaves();
        overlap.push(Leaf { key: morton::encode([1, 1, 1]), level: 0, count: 1, material: 0 });
        assert!(VoxTree::build(grid(), overlap, 3.0).is_err());
        let mut skew = leaves();
        skew[0].key = morton::encode([1, 0, 0]);
        assert!(VoxTree::build(grid(), skew, 3.0).is_err());
        let mut zero = leaves();
        zero[1].count = 0;
        assert!(VoxTree::build(grid(), zero, 3.0).is_err());
        let mut over = leaves();
        over[1].count = 9;
        assert!(VoxTree::build(grid(), over, 3.0).is_err());
        assert!(VoxTree::build(grid(), Vec::new(), 3.0).is_err());
    }

    #[test]
    fn every_leaf_is_reached_from_the_root_at_its_ordinal() {
        let tree = VoxTree::build(grid(), leaves(), 3.0).unwrap();
        let mut reached = Vec::new();
        let mut stack = vec![0usize];
        while let Some(n) = stack.pop() {
            let node = tree.nodes[n];
            for (child, octant) in tree.children_of(n).zip((0..8).filter(|o| node.mask & (1 << o) != 0)) {
                let key = morton::child(node.key, node.level, octant);
                match child {
                    Child::Leaf(l) => {
                        // The leaf is the block in that octant, exactly.
                        assert_eq!((tree.leaves[l].key, tree.leaves[l].level), (key, node.level - 1));
                        reached.push(l);
                    }
                    Child::Node(m) => {
                        assert_eq!((tree.nodes[m].key, tree.nodes[m].level), (key, node.level - 1));
                        stack.push(m);
                    }
                }
            }
            // A node's run of leaves is exactly the leaves under it.
            let keys = node.key..node.key + span(node.level);
            for (l, leaf) in tree.leaves.iter().enumerate() {
                assert_eq!(
                    keys.contains(&leaf.key),
                    (node.first_leaf as usize..node.end_leaf as usize).contains(&l)
                );
            }
        }
        reached.sort_unstable();
        assert_eq!(reached, (0..tree.leaves.len()).collect::<Vec<_>>());
        // A tree that is one cube has no node, and its body is its leaf.
        let one =
            VoxTree::build(grid(), vec![Leaf { key: 0, level: 3, count: 8, material: 0 }], 2.0).unwrap();
        assert!(one.nodes.is_empty() && one.body().mass == 2.0);
        // Control: a node whose children were swapped no longer lists its leaves in octant order.
        let mut swapped = tree.clone();
        let first = swapped.nodes[0].first_child as usize;
        swapped.children.swap(first, first + 1);
        assert_ne!(swapped.children, tree.children);
    }

    #[test]
    fn validation_moments_are_the_sums_of_their_leaves() {
        let tree = VoxTree::build(grid(), leaves(), 3.0).unwrap();
        let worst = moments_error(&tree);
        assert!(worst <= 1e-13, "{worst}");
        // The body's mass is the spec's.
        assert!((tree.body().mass - 3.0).abs() <= 1e-15 * 3.0);
        // Control: a third moment zeroed is seen.
        let mut broken = tree.clone();
        broken.moments[0].third[3] = 0.0;
        assert!(moments_error(&broken) > 1e-6, "{}", moments_error(&broken));
    }

    /// The worst difference between each node's moments and the direct sum over its leaves, each
    /// relative to the node's own scale (mass; mass × side; mass × side²; mass × side³).
    pub(crate) fn moments_error(tree: &VoxTree) -> f64 {
        let mut worst = 0f64;
        for n in 0..tree.nodes.len() {
            let (got, want) = (tree.moments[n], tree.direct_moments(n));
            let side = tree.side(tree.nodes[n].level);
            let m = want.mass;
            worst = worst.max((got.mass - m).abs() / m);
            let d = got.centre - want.centre;
            worst = worst.max(d.x.abs().max(d.y.abs()).max(d.z.abs()) / side);
            for i in 0..6 {
                worst = worst.max((got.second[i] - want.second[i]).abs() / (m * side * side));
            }
            for i in 0..10 {
                worst = worst.max((got.third[i] - want.third[i]).abs() / (m * side.powi(3)));
            }
        }
        worst
    }

    #[test]
    fn a_cut_partitions_the_leaves_and_keeps_the_mass() {
        let tree = VoxTree::build(grid(), leaves(), 3.0).unwrap();
        for level in 0..=tree.grid.depth {
            let cut = tree.cut(level);
            let mut covered = vec![0u32; tree.leaves.len()];
            let mut mass = 0.0;
            for entry in &cut {
                match *entry {
                    CutEntry::Leaf(l) => {
                        assert!(tree.leaves[l].level >= level || tree.nodes.is_empty());
                        covered[l] += 1;
                        mass += tree.leaf_mass(&tree.leaves[l]);
                    }
                    CutEntry::Node(n) => {
                        assert_eq!(tree.nodes[n].level, level);
                        for c in
                            &mut covered[tree.nodes[n].first_leaf as usize..tree.nodes[n].end_leaf as usize]
                        {
                            *c += 1;
                        }
                        mass += tree.moments[n].mass;
                    }
                }
            }
            assert!(covered.iter().all(|&c| c == 1), "level {level}: {covered:?}");
            assert!((mass - 3.0).abs() <= 1e-12 * 3.0, "level {level}: {mass}");
        }
        // Control: a cut that left out its first entry would not hold the mass.
        let cut = tree.cut(1);
        let lost = match cut[0] {
            CutEntry::Leaf(l) => tree.leaf_mass(&tree.leaves[l]),
            CutEntry::Node(n) => tree.moments[n].mass,
        };
        assert!(lost > 1e-3);
    }
}
