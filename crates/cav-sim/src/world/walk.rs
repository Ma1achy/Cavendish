//! The gravity read's walk over the world's octree (`docs/slices/S5.md` §2 "The walk";
//! `docs/research/s5-world-octree.md` top block, §2.1): which nodes a tower reads through their
//! expansion, which chunks it reads leaf by leaf, and the ledger that bounds what that leaves out.
//!
//! **The target.** A tower's atoms fly on the vertical segment from its base up its height
//! ([`Aim`]). A node's bound `B` ([`node_bound`]) is the most the orders its expansion leaves out
//! (`l ≥ 4`) can change `V(upper) − V(lower)` between two points the tower's height apart, at the
//! least distance `R` from the node's expansion centre to the segment ([`Summary::bound`]), times
//! the flight's radians per kg/m summed over the two interferometers ([`Aim::per`]): so it bounds
//! the node's share of the gradiometer's error, and of each interferometer's. `B = ∞` when the
//! node's reach is at least [`Q_MAX`] of `R`: such a node is never accepted, whatever the
//! tolerance (Salmon & Warren's footnote 3: the bound needs the target well outside the node).
//!
//! **The Sum MAC** ([`walk`]). From the octree's roots, a priority queue on `B`: the node with the
//! largest bound is popped and replaced by its children, until the bounds left in the queue sum to
//! at most the tolerance `τ`. A chunk (level 0) that is popped is **opened**: when the walk is
//! given the chunk's own tree ([`walk_with`], [`super::inner::ChunkTree`]) its blocks go into the
//! queue as nodes like any other (each with its summary and bound) and its small blocks' leaves
//! are read one by one; without it ([`walk`]) the whole chunk is read leaf by leaf. A leaf adds
//! nothing to the ledger: it is read with the exact cube kernel. [`walk_into`] makes the trees of
//! the chunks a walk opens and walks again until every chunk it opens was opened into its tree. Every node whose box is within
//! the **bubble** ([`Aim::bubble`], `world.read.radius`) of the segment is opened whatever its
//! bound (so a node the atoms fly through always is). Ties are broken by key, so the cut is the
//! same bits on any machine. What is left in the queue is the cut's **accepted** nodes; with the
//! opened chunks they cover every chunk of the world exactly once (the partition,
//! `the_cut_is_a_partition`).
//!
//! **Caps** (`docs/slices/S5.md` §3.1, X4). At most [`MAX_CUT_NODES`] accepted nodes, and a cap on
//! the work of a flight node ([`Aim::max_work`]: the opened chunks' leaves, plus four for each
//! accepted node, as S4 counted a chunk read through its moments). A step that would pass either
//! is not taken: the walk stops there, the cut is **not settled**, and its ledger (which may then
//! be above `τ`, or infinite) says by how much. Nothing is left out silently.
//!
//! **What is read** ([`far_field`], [`LedgerSource`]). The accepted nodes are read through their
//! expansion to the octupole about each node's centre ([`Summary::difference`]); the ledger the
//! read reports is their bounds read through the flight ([`LedgerSource`], the quadrature's weights
//! positive), as S4's moments' bound was. The read gathers the far ones into a local expansion
//! along the tower's line ([`super::local`]).
//!
//! Frames and units: the world's frame (x east, y north, z up), metres, kilograms, radians.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use cav_core::Vec3;
use cav_core::forward::Source;
use cav_core::voxel::Aabb;
use cav_core::voxel::summary::{FIRST_LEFT_OUT, Summary};
use cav_core::voxel::world::ChunkKey;

use super::inner::{ChunkTree, InnerKey, Part};
use super::octree::{NodeKey, Octree};
use super::{World, box_to_segment, to_segment};

/// A node whose reach is at least this fraction of its least distance to the atoms' path has an
/// infinite bound: it is opened, never accepted ([`node_bound`]).
pub const Q_MAX: f64 = 0.5;

/// The most nodes a cut may accept, and so the most its queue holds. Since the walk goes into the
/// chunks' trees (TASK-S5-27) a cut of `town` at 1e-6 rad accepts 309,558 blocks
/// (`docs/slices/S5.md` §7.9); 2^19 holds the queue in about 25 MB and the accepted nodes'
/// summaries in 172 MB.
pub const MAX_CUT_NODES: usize = 524_288;

/// How a node's bound and its reading are made: the read's rule, or a control that breaks it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rule {
    /// The expansion to the octupole about the node's centre, and the bound on the orders from 4
    /// with the gradient's factor `l + 1`.
    #[default]
    Octupole,
    /// A control (`the_walk_needs_its_bound`): the octupole dropped from the expansion and the
    /// factor `l + 1` replaced by 1 in the bound, which no longer covers what is left out. Never
    /// for a read.
    Broken,
}

impl Rule {
    /// The most the orders this rule's expansion leaves out can change `V/G` between two points
    /// `separation` metres apart, all at least `distance` metres from the summary's centre, kg/m.
    #[must_use]
    pub fn bound(self, summary: &Summary, distance: f64, separation: f64) -> f64 {
        match self {
            Self::Octupole => summary.bound(distance, separation),
            Self::Broken => summary.bound_by(FIRST_LEFT_OUT, &|_| 1.0, distance, separation),
        }
    }

    /// `(V(upper) − V(lower))/G`, kg/m, by this rule's expansion (as [`Summary::difference`]).
    #[must_use]
    pub fn difference(self, summary: &Summary, across: [f64; 2], below: f64, above: f64, gap: f64) -> f64 {
        let order = match self {
            Self::Octupole => 3,
            Self::Broken => 2,
        };
        summary.difference_to(order, across, below, above, gap)
    }
}

/// What one tower's walk is aimed at and held to.
#[derive(Clone, Copy, Debug)]
pub struct Aim {
    /// The tower's base, metres, the world's frame: the bottom of its atoms' path.
    pub base: Vec3,
    /// The tower's height, metres: the atoms' path is the vertical segment from `base` up this.
    pub height: f64,
    /// Radians the flight makes of 1 kg/m held at every instant, the two interferometers' added
    /// (`|lower| + |upper|` of a reading of a unit source): what turns a bound in kg/m into one on
    /// the gradiometer in radians.
    pub per: f64,
    /// The ledger's target, radians (`world.read.tolerance`).
    pub tolerance: f64,
    /// Every node whose box is within this many metres of the atoms' path is opened, whatever its
    /// bound (`world.read.radius`); 0 opens only those the path passes through or touches.
    pub bubble: f64,
    /// The most work a flight node may do for one point pair: the opened chunks' leaves plus four
    /// for each accepted node (kernel evaluations; S4's `work_share` over the first rule's nodes).
    pub max_work: u64,
    /// The most nodes the cut may accept: [`MAX_CUT_NODES`] for a read (a larger number is taken
    /// as that), fewer to try the cap.
    pub max_nodes: usize,
    /// The rule: [`Rule::Octupole`] for every read.
    pub rule: Rule,
}

/// A tower's cut of the octree: what it reads through expansions, what leaf by leaf, and the
/// ledger.
#[derive(Clone, Debug, PartialEq)]
pub struct Cut {
    /// The octree's nodes read through their expansion, in key order.
    pub accepted: Vec<NodeKey>,
    /// The blocks of opened chunks' trees read through their expansion, in key order.
    pub inner: Vec<InnerKey>,
    /// Every chunk the walk opened, in key order: those of [`Self::whole`] and those opened into
    /// their trees.
    pub opened: Vec<ChunkKey>,
    /// The opened chunks the walk had no tree for: each read leaf by leaf, whole, in key order.
    pub whole: Vec<ChunkKey>,
    /// The runs of leaves read one by one of the chunks opened into their trees: `(chunk, the run
    /// of its cubes)`, in order.
    pub runs: Vec<(ChunkKey, std::ops::Range<u32>)>,
    /// The sum of the accepted nodes' bounds `B`, radians (infinite when the walk stopped with a
    /// node it could not accept). The read reports the tighter [`LedgerSource`] reading of the
    /// same nodes; this is what the walk held to `τ`.
    pub ledger: f64,
    /// Whether the walk reached its tolerance: false when a cap stopped it ([`Self::stopped`]).
    pub settled: bool,
    /// Why the walk stopped short, in words; `None` when it settled.
    pub stopped: Option<String>,
    /// Accepted nodes of each level, from the chunks up (as many entries as the octree's levels).
    pub per_level: Vec<usize>,
    /// Accepted blocks of the chunks' trees by level, `[1, 2, 3, 4]`: 2, 4, 8 and 16 cells a side.
    pub inner_per_level: [usize; 4],
    /// The leaves read one by one, in all.
    pub leaves: u64,
    /// Each accepted node's bound `B`, radians, in the order of [`Self::accepted`] then
    /// [`Self::inner`] (infinite for one a stopped walk could not accept): what the ledger is made
    /// of, kept for drawing each node's share of it (`super::lod_view`).
    pub bounds: Vec<f64>,
}

impl Cut {
    /// The cut's entries: accepted nodes and blocks, and opened chunks.
    #[must_use]
    pub fn nodes(&self) -> usize {
        self.accepted.len() + self.inner.len() + self.opened.len()
    }
}

/// The least distance from node `id`'s expansion centre to the atoms' path, metres.
#[must_use]
pub fn node_distance(octree: &Octree, id: usize, aim: &Aim) -> f64 {
    to_segment(octree.summary(id).centre, aim.base, aim.height)
}

/// Node `id`'s bound `B`, radians: [`summary_bound`] of its summary.
#[must_use]
pub fn node_bound(octree: &Octree, id: usize, aim: &Aim) -> f64 {
    summary_bound(octree.summary(id), aim)
}

/// The bound `B` of a summary, radians: [`Rule::bound`] between two points the tower's height
/// apart at the least distance from its centre to the atoms' path, times [`Aim::per`]; infinite
/// when its reach is at least [`Q_MAX`] of that distance. (The bubble is the walk's, not the
/// bound's.)
#[must_use]
pub fn summary_bound(summary: &Summary, aim: &Aim) -> f64 {
    let distance = to_segment(summary.centre, aim.base, aim.height);
    if summary.is_empty() {
        return 0.0;
    }
    if summary.reach >= Q_MAX * distance {
        return f64::INFINITY;
    }
    aim.rule.bound(summary, distance, aim.height) * aim.per
}

/// What a cut entry is, for ordering ties: an octree node, or a block of a chunk's tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Node(NodeKey),
    Inner(InnerKey),
}

/// Where an entry's node is: in the octree, or block `node` of tree `tree` (an index into the
/// walk's trees).
#[derive(Clone, Copy, Debug)]
enum Item {
    Octree(usize),
    Inner { tree: usize, node: u32 },
}

/// One node in the queue: its bound, its key (for ties) and where it is.
#[derive(Clone, Copy, Debug)]
struct Entry {
    bound: f64,
    key: Key,
    item: Item,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Entry {}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Entry {
    /// The larger bound first; of two equal bounds, the smaller key first.
    fn cmp(&self, other: &Self) -> Ordering {
        self.bound.total_cmp(&other.bound).then_with(|| other.key.cmp(&self.key))
    }
}

/// The sum of the queue's bounds in key order (the same bits however the heap holds them).
fn exact_sum(queue: &BinaryHeap<Entry>) -> f64 {
    let mut entries: Vec<&Entry> = queue.iter().collect();
    entries.sort_by_key(|e| e.key);
    entries.iter().map(|e| e.bound).sum()
}

/// Walks `octree` for the tower `aim` describes, each chunk it opens read leaf by leaf, whole:
/// [`walk_with`] given no chunk's tree.
#[must_use]
pub fn walk(octree: &Octree, aim: &Aim) -> Cut {
    walk_with(octree, aim, &[])
}

/// Walks `octree` for the tower `aim` describes: the Sum MAC on one global ledger (the module's
/// documentation). A chunk it opens whose tree is in `trees` (sorted by key) is opened into it;
/// one whose tree is not is read whole ([`Cut::whole`]). Deterministic: the same octree, aim and
/// trees give the same cut, bit for bit, and the tree of a chunk the walk does not open changes
/// nothing.
#[must_use]
pub fn walk_with(octree: &Octree, aim: &Aim, trees: &[ChunkTree]) -> Cut {
    let levels = octree.levels();
    let bounded = |summary: &Summary, boxed: &Aabb| {
        let forced = box_to_segment(boxed, aim.base, aim.height) <= aim.bubble;
        if forced { f64::INFINITY } else { summary_bound(summary, aim) }
    };
    let entry = |id: usize| Entry {
        bound: bounded(octree.summary(id), &octree.boxed(id)),
        key: Key::Node(octree.node(id).key),
        item: Item::Octree(id),
    };
    let block = |tree: usize, node: u32| {
        let n = trees[tree].node(node);
        Entry {
            bound: bounded(&n.summary, &n.boxed),
            key: Key::Inner(n.key),
            item: Item::Inner { tree, node },
        }
    };
    let mut queue: BinaryHeap<Entry> = BinaryHeap::new();
    let mut opened: Vec<ChunkKey> = Vec::new();
    let mut whole: Vec<ChunkKey> = Vec::new();
    let mut runs: Vec<(ChunkKey, std::ops::Range<u32>)> = Vec::new();
    let mut leaves = 0u64;
    // The bounds in the queue: the finite ones summed as they come and go (made exact again
    // before the walk is let stop), and how many are infinite.
    let (mut finite, mut infinite) = (0f64, 0usize);
    let add = |e: Entry, queue: &mut BinaryHeap<Entry>, finite: &mut f64, infinite: &mut usize| {
        if e.bound.is_finite() {
            *finite += e.bound;
        } else {
            *infinite += 1;
        }
        queue.push(e);
    };
    let work = |leaves: u64, accepted: usize| leaves.saturating_add(4 * accepted as u64);
    let most = aim.max_nodes.min(MAX_CUT_NODES);
    let mut stopped = None;
    let roots = octree.roots();
    if roots.len() > most || work(0, roots.len()) > aim.max_work {
        stopped = Some(format!(
            "the octree's {} roots are past the walk's caps ({most} nodes, {} work a flight node)",
            roots.len(),
            aim.max_work
        ));
    }
    for id in roots {
        add(entry(id), &mut queue, &mut finite, &mut infinite);
    }
    while stopped.is_none() {
        if infinite == 0 && finite <= aim.tolerance {
            finite = exact_sum(&queue);
            if finite <= aim.tolerance {
                break;
            }
        }
        let Some(top) = queue.pop() else { break };
        // What opening it gives: a chunk with no tree read whole, or the parts it holds (an
        // octree node's children, a chunk's or a block's parts in its tree).
        let opening = match top.item {
            Item::Octree(id) if octree.children(id).is_empty() => {
                let key = octree.node(id).key.key;
                match trees.binary_search_by_key(&key, |t| t.key) {
                    Ok(t) => Opening::Parts(t, trees[t].parts()),
                    Err(_) => Opening::Whole(key, octree.node(id).leaves),
                }
            }
            Item::Octree(id) => Opening::Children(octree.children(id)),
            Item::Inner { tree, node } => Opening::Parts(tree, trees[tree].children(node)),
        };
        let (more_leaves, more_nodes) = match &opening {
            Opening::Whole(_, l) => (*l, 0),
            Opening::Children(c) => (0, c.len()),
            Opening::Parts(_, parts) => parts.iter().fold((0, 0), |(l, n), p| match p {
                Part::Leaves(r) => (l + u64::from(r.end - r.start), n),
                Part::Node(_) => (l, n + 1),
            }),
        };
        let (after_leaves, after_queue) = (leaves.saturating_add(more_leaves), queue.len() + more_nodes);
        if after_queue > most || work(after_leaves, after_queue) > aim.max_work {
            stopped = Some(if after_queue > most {
                format!("the cut would accept more than {most} nodes")
            } else {
                format!(
                    "opening further would pass the read's work of {} kernel evaluations a flight node",
                    aim.max_work
                )
            });
            queue.push(top);
            break;
        }
        if top.bound.is_finite() {
            finite -= top.bound;
        } else {
            infinite -= 1;
        }
        leaves = after_leaves;
        match opening {
            Opening::Whole(key, _) => {
                opened.push(key);
                whole.push(key);
            }
            Opening::Children(children) => {
                for &child in children {
                    add(entry(child as usize), &mut queue, &mut finite, &mut infinite);
                }
            }
            Opening::Parts(t, parts) => {
                if matches!(top.item, Item::Octree(_)) {
                    opened.push(trees[t].key);
                }
                for part in parts {
                    match part {
                        Part::Leaves(r) => runs.push((trees[t].key, r.clone())),
                        Part::Node(n) => add(block(t, *n), &mut queue, &mut finite, &mut infinite),
                    }
                }
            }
        }
    }
    let mut accepted: Vec<(NodeKey, f64)> = Vec::new();
    let mut inner: Vec<(InnerKey, f64)> = Vec::new();
    for e in &queue {
        match e.key {
            Key::Node(k) => accepted.push((k, e.bound)),
            Key::Inner(k) => inner.push((k, e.bound)),
        }
    }
    accepted.sort_unstable_by_key(|(k, _)| *k);
    inner.sort_unstable_by_key(|(k, _)| *k);
    let bounds = accepted.iter().map(|(_, b)| *b).chain(inner.iter().map(|(_, b)| *b)).collect();
    let accepted: Vec<NodeKey> = accepted.into_iter().map(|(k, _)| k).collect();
    let inner: Vec<InnerKey> = inner.into_iter().map(|(k, _)| k).collect();
    opened.sort_unstable();
    whole.sort_unstable();
    runs.sort_unstable_by_key(|(k, r)| (*k, r.start));
    let mut per_level = vec![0usize; levels];
    for key in &accepted {
        per_level[usize::from(key.level)] += 1;
    }
    let mut inner_per_level = [0usize; 4];
    for key in &inner {
        inner_per_level[usize::from(key.level).clamp(1, 4) - 1] += 1;
    }
    let ledger = exact_sum(&queue);
    let settled = stopped.is_none() && ledger <= aim.tolerance;
    if !settled && stopped.is_none() {
        stopped = Some("the walk ran out of nodes above its tolerance".to_owned());
    }
    Cut {
        accepted,
        inner,
        opened,
        whole,
        runs,
        ledger,
        settled,
        stopped,
        per_level,
        inner_per_level,
        leaves,
        bounds,
    }
}

/// What popping an entry opens.
enum Opening<'a> {
    /// A chunk the walk has no tree for, and its leaves: read whole.
    Whole(ChunkKey, u64),
    /// An octree node's children.
    Children(&'a [u32]),
    /// A chunk's or a block's parts in tree `.0`.
    Parts(usize, &'a [Part]),
}

/// Walks `octree` for the tower `aim` describes with every chunk it opens opened into its own
/// tree: [`walk_with`] on `trees`, then the trees of the chunks it read whole made
/// ([`super::open`]) and added to `trees` (kept sorted, so that other towers' walks share them),
/// and again, until it reads no chunk whole. Each round adds a tree, so it ends; the cut is the
/// one [`walk_with`] gives with every chunk's tree at hand. `Ok(None)` when cancelled.
///
/// # Errors
/// When a chunk cannot be voxelised.
pub fn walk_into(
    octree: &Octree,
    aim: &Aim,
    world: &World,
    trees: &mut Vec<ChunkTree>,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Option<Cut>, String> {
    loop {
        let cut = walk_with(octree, aim, trees);
        if cut.whole.is_empty() {
            return Ok(Some(cut));
        }
        let Some(made) = super::open(world, &cut.whole, cancelled)? else { return Ok(None) };
        trees.extend(made);
        trees.sort_unstable_by_key(|t| t.key);
    }
}

/// The far field of a cut: its accepted nodes read through their expansion. The one place the
/// far part of a tower's source is made (TASK-S5-22's local expansion takes its place).
#[derive(Clone, Debug)]
pub struct Far {
    /// The accepted nodes' summaries, in the cut's key order.
    pub nodes: Vec<Summary>,
    /// How they are read.
    pub rule: Rule,
}

/// The far field of `cut` over `octree` (and `trees`, which hold the trees of the chunks it
/// opened into theirs), read by `rule`: the accepted octree nodes in key order, then the
/// accepted blocks in key order.
///
/// # Panics
/// When a key of the cut is not one of the octree's nodes or of the trees' blocks (a cut of
/// another octree, or trees missing).
#[must_use]
pub fn far_field(octree: &Octree, trees: &[ChunkTree], cut: &Cut, rule: Rule) -> Far {
    let mut nodes: Vec<Summary> = cut
        .accepted
        .iter()
        .map(|key| *octree.summary(octree.find(*key).expect("a cut of this octree")))
        .collect();
    for key in &cut.inner {
        let t = trees.binary_search_by_key(&key.chunk, |t| t.key).expect("the tree of an opened chunk");
        let n = trees[t].find(*key).expect("a block of the tree");
        nodes.push(trees[t].node(n).summary);
    }
    Far { nodes, rule }
}

impl Far {
    /// `(V(upper) − V(lower))/G`, kg/m, of every node at the vertical line through `tower`
    /// between heights `height + lower` and `height + upper`.
    #[must_use]
    pub fn at(&self, tower: Vec3, height: f64, lower: f64, upper: f64) -> f64 {
        let mut sum = 0.0;
        for node in &self.nodes {
            let at = node.centre - tower;
            let (below, above) = (height + lower - at.z, height + upper - at.z);
            sum += self.rule.difference(node, [-at.x, -at.y], below, above, lower - upper);
        }
        sum
    }
}

impl Source for Far {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, _time: f64) -> [f64; 2] {
        heights.map(|height| self.at(tower, height, lower, upper))
    }
}

/// The bound on what reading the accepted nodes through their expansion leaves out: at each
/// instant, for each interferometer, the sum of each node's [`Rule::bound`] between its two arm
/// points at their least distance from the node. Read through the flight like any source; the
/// quadrature's weights are positive, so its reading bounds the far field's error, radians.
pub struct LedgerSource<'a> {
    /// The accepted nodes.
    pub nodes: &'a [Summary],
    /// The rule they are read by.
    pub rule: Rule,
}

impl Source for LedgerSource<'_> {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, _time: f64) -> [f64; 2] {
        heights.map(|height| {
            let (z0, z1) = (tower.z + height + lower.min(upper), (upper - lower).abs());
            let base = Vec3::new(tower.x, tower.y, z0);
            self.nodes.iter().map(|n| self.rule.bound(n, to_segment(n.centre, base, z1), z1)).sum()
        })
    }
}

#[cfg(test)]
pub(crate) mod tests;
