//! What the read made of the world, kept for drawing (`docs/slices/S5.md` §2 "Draw modes and LOD
//! views"; `docs/research/s5-world-octree.md` §7a): the octree's nodes and each tower's cut, with
//! the bound of every node it accepted ([`ReadView`]); the overlays drawn from them
//! ([`WorldOverlay`]: the octree by level, a view's entries by level, by the tower that set them,
//! by their share of the ledger); and the LOD views ([`LodView`]: the camera's clipmap, a tower's
//! cut, the common refinement of several towers' cuts).
//!
//! **No new walk.** A tower's LOD view is the cut its read made ([`TowerCut`], from
//! [`super::walk::Cut`]), handed to the engine with the world's contribution. A view of several
//! towers is their common refinement ([`union`]): every strict ancestor of an entry of any of the
//! cuts is opened, and the rest emitted, so each region is drawn at the finest level any of the
//! towers reads it at (a partition of the world's chunks, finer than or equal to each cut, every
//! entry in some cut). The naive union is not a partition: it covers a region twice where two
//! towers read it at different levels (`the_union_of_towers_is_a_common_refinement`'s control).
//!
//! **Below the chunk** (TASK-S5-27). A tower's walk goes into the trees of the chunks it opens and
//! accepts their blocks of 2 to 16 cells (levels −4 … −1 in [`super::rings`]' numbering). They are
//! **drawn merged into their chunk** (a chunk opened is drawn as its own mesh, the finest there is)
//! and **outlined** by the overlays, each block at its level; the leaves the walk reads one by one
//! are what is left of an opened chunk outside its blocks.
//!
//! **Bounded** (`docs/slices/S5.md` §3.1, X6): a draw list of at most [`MAX_DRAW_LIST`] entries
//! (past it the finest level is merged into its parents until it fits, said); at most
//! [`MAX_OVERLAY_NODES`] overlay boxes (whole levels left out, the finest first, said).
//!
//! Frames and units: the world's frame (x east, y north, z up), metres; bounds in radians.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use cav_core::voxel::Aabb;
use cav_core::voxel::world::{ChunkKey, WorldGrid};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::inner::InnerKey;
use super::octree::NodeKey;

/// The most entries a LOD view's draw list holds: what the renderer has room for (its chunk
/// meshes and its bricks, `cav_render::world::MAX_CHUNK_SLOTS` and `MAX_BRICK_SLOTS`).
pub const MAX_DRAW_LIST: usize = 16_384;

/// The most boxes an overlay of the world draws.
pub const MAX_OVERLAY_NODES: usize = 20_000;

/// Which levels of detail a pane draws the world at.
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub enum LodView {
    /// The camera's clipmap (S4): the levels its screen-space error wants.
    #[default]
    Camera,
    /// The levels tower `n` (from 0) reads: its read's cut.
    Tower(usize),
    /// The finest level any of these towers (from 0) reads each region at: the common refinement
    /// of their cuts. At most [`MAX_LOD_TOWERS`] towers, each once.
    Towers(Vec<usize>),
}

/// The most towers one LOD view names.
pub const MAX_LOD_TOWERS: usize = 64;

impl LodView {
    /// Whether it is the default ([`Self::Camera`]): a view on the camera does not write the
    /// field.
    #[must_use]
    pub fn is_camera(&self) -> bool {
        *self == Self::Camera
    }

    /// The towers it draws the cuts of, from 0, in order; none for the camera.
    #[must_use]
    pub fn towers(&self) -> Vec<usize> {
        match self {
            Self::Camera => Vec::new(),
            Self::Tower(n) => vec![*n],
            Self::Towers(towers) => towers.clone(),
        }
    }
}

/// What an overlay of the world colours, and by what.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub enum WorldOverlay {
    /// None.
    #[default]
    None,
    /// The world's octree: every node outlined in its level's colour.
    Octree,
    /// The view's entries outlined in their level's colour (an opened chunk's blocks at theirs).
    Level,
    /// The view's entries outlined in the colour of the tower that set their level (ties to the
    /// lowest).
    Tower,
    /// The view's entries outlined by their share of their tower's ledger (an opened chunk, read
    /// leaf by leaf, has none).
    Ledger,
}

impl WorldOverlay {
    /// Every one, in the order menus list them.
    pub const ALL: [Self; 5] = [Self::None, Self::Octree, Self::Level, Self::Tower, Self::Ledger];

    /// The word commands use.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Octree => "octree",
            Self::Level => "level",
            Self::Tower => "tower",
            Self::Ledger => "ledger",
        }
    }

    /// The one a word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|o| o.name() == word)
    }

    /// Whether it is [`Self::None`].
    #[must_use]
    pub fn is_none(&self) -> bool {
        *self == Self::None
    }
}

/// One tower's cut as the read made it, for drawing: what it accepted (with each node's bound
/// `B`, radians) and what it opened.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TowerCut {
    /// The octree's nodes read through their expansion, in key order, each with its bound.
    pub accepted: Vec<(NodeKey, f64)>,
    /// The blocks of opened chunks read through their expansion, in key order, each with its
    /// bound.
    pub inner: Vec<(InnerKey, f64)>,
    /// The chunks it opened (into their blocks and leaves), in key order.
    pub opened: Vec<ChunkKey>,
    /// The sum of its accepted bounds, radians.
    pub ledger: f64,
}

impl TowerCut {
    /// A walk's cut as drawn: its accepted nodes and blocks with their bounds, its opened chunks.
    #[must_use]
    pub fn of(cut: &super::walk::Cut) -> Self {
        let n = cut.accepted.len();
        Self {
            accepted: cut.accepted.iter().copied().zip(cut.bounds.iter().copied()).collect(),
            inner: cut
                .inner
                .iter()
                .copied()
                .zip(cut.bounds[n.min(cut.bounds.len())..].iter().copied())
                .collect(),
            opened: cut.opened.clone(),
            ledger: cut.ledger,
        }
    }
}

/// What the read made of the world, for the views: the octree's nodes and every tower's cut.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ReadView {
    /// The world's grid.
    pub grid: Option<WorldGrid>,
    /// The octree's nodes, by level from the chunks up, each level in key order.
    pub nodes: Vec<NodeKey>,
    /// Each tower's cut, in the towers' order.
    pub towers: Vec<TowerCut>,
}

impl ReadView {
    /// The bytes it holds, for the forward stage's budget.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.nodes.len() * size_of::<NodeKey>()
            + self
                .towers
                .iter()
                .map(|t| {
                    t.accepted.len() * size_of::<(NodeKey, f64)>()
                        + t.inner.len() * size_of::<(InnerKey, f64)>()
                        + t.opened.len() * size_of::<ChunkKey>()
                })
                .sum::<usize>()
    }
}

/// An entry of a LOD view at the chunk's level or above: a node of the octree (level 0 for a chunk,
/// accepted whole or opened).
pub type Entry = (u8, ChunkKey);

/// A tower's entries at the chunk's level and above: what it accepted, and the chunks it opened.
#[must_use]
pub fn entries(cut: &TowerCut) -> Vec<Entry> {
    let mut out: Vec<Entry> = cut.accepted.iter().map(|(k, _)| (k.level, k.key)).collect();
    out.extend(cut.opened.iter().map(|k| (0, *k)));
    out.sort_unstable();
    out.dedup();
    out
}

/// The key of the node of level `to` that holds node `(level, key)` (`to ≥ level`).
fn ancestor((level, key): Entry, to: u8) -> Entry {
    (to, super::clipmap::ancestor(level, key, to))
}

/// The common refinement of the cuts `cuts`, at the chunk's level and above: every strict ancestor
/// of an entry of any cut is opened, and the entries that are not are emitted, each once, in key
/// order, with the lowest-numbered of the cuts (an index into `cuts`) holding it.
#[must_use]
pub fn union(cuts: &[&TowerCut]) -> Vec<(Entry, usize)> {
    let mut held: BTreeMap<Entry, usize> = BTreeMap::new();
    for (t, cut) in cuts.iter().enumerate() {
        for e in entries(cut) {
            held.entry(e).or_insert(t);
        }
    }
    // Every strict ancestor of an entry: opened.
    let mut open: HashSet<Entry> = HashSet::new();
    for &(level, key) in held.keys() {
        for to in level + 1..=super::octree::MAX_LEVEL {
            if !open.insert(ancestor((level, key), to)) {
                break; // in already, and so are the nodes above it
            }
        }
    }
    held.into_iter().filter(|(e, _)| !open.contains(e)).collect()
}

/// The blocks of opened chunks the common refinement of `cuts` keeps: a block of one tower stands
/// when every tower that opened its chunk has a block holding it (itself or coarser); otherwise
/// some tower reads part of it finer (by a smaller block or leaf by leaf) and it is opened. With
/// the lowest-numbered cut holding it.
#[must_use]
pub fn union_inner(cuts: &[&TowerCut]) -> Vec<(InnerKey, usize)> {
    let opened_by: Vec<BTreeSet<ChunkKey>> =
        cuts.iter().map(|c| c.opened.iter().copied().collect()).collect();
    let blocks: Vec<BTreeSet<InnerKey>> =
        cuts.iter().map(|c| c.inner.iter().map(|(k, _)| *k).collect()).collect();
    // Whether tower `t` has a block holding `b` (itself or an ancestor within the chunk).
    let holds = |t: usize, b: &InnerKey| {
        (b.level..=4).any(|level| {
            let span = 3 * u32::from(level);
            let key = (b.key >> span) << span;
            blocks[t].contains(&InnerKey { chunk: b.chunk, level, key })
        })
    };
    let mut out: BTreeMap<InnerKey, usize> = BTreeMap::new();
    for (t, set) in blocks.iter().enumerate() {
        for b in set {
            let stands = (0..cuts.len()).all(|u| !opened_by[u].contains(&b.chunk) || holds(u, b));
            if stands {
                out.entry(*b).or_insert(t);
            }
        }
    }
    out.into_iter().collect()
}

/// The box of a block of a chunk's tree on `grid`, metres, the world's frame.
#[must_use]
pub fn inner_box(grid: &WorldGrid, key: InnerKey) -> Aabb {
    let chunk = grid.chunk_box(key.chunk);
    let [x, y, z] = cav_core::voxel::morton::decode(key.key);
    let side = grid.pitch * f64::from(1u32 << key.level);
    let lo = chunk.lo + cav_core::Vec3::new(f64::from(x), f64::from(y), f64::from(z)) * grid.pitch;
    Aabb::new(lo, lo + cav_core::Vec3::new(side, side, side))
}

/// A common refinement as the frame takes it: its entries and its blocks, each with the tower
/// that set it.
type Refinement = (Vec<(Entry, usize)>, Vec<(InnerKey, usize)>);

/// One box of a world overlay: where it is, its level (−4 … −1 for a block of 2 to 16 cells, 0 for
/// a chunk, the octree's above), and its value of the overlay's field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodBox {
    /// Its box, metres, the world's frame.
    pub boxed: Aabb,
    /// Its level.
    pub level: i8,
    /// The tower that set it (from 0), for [`WorldOverlay::Tower`].
    pub tower: Option<usize>,
    /// Its share of its tower's ledger (its bound over the ledger), for [`WorldOverlay::Ledger`];
    /// 0 for an opened chunk.
    pub share: f64,
}

/// What a pane draws of the world's levels: the draw list of its LOD view (none for the camera,
/// whose clipmap the renderer works out), and its overlay's boxes.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct LodFrame {
    /// The LOD view.
    pub view: LodView,
    /// The overlay.
    pub overlay: WorldOverlay,
    /// For a tower's or towers' view: the entries to draw, `(level, key)` (level 0 a chunk's mesh,
    /// above a brick), in key order; none for the camera's.
    pub list: Option<Vec<Entry>>,
    /// The overlay's boxes, at most [`MAX_OVERLAY_NODES`].
    pub boxes: Vec<LodBox>,
    /// The levels drawn by the overlay, finest first, for its legend.
    pub levels: Vec<i8>,
    /// What a person should know: a level left out of the overlay, a list merged to fit, a tower
    /// with no cut yet.
    pub notes: Vec<String>,
}

/// Keeps whole levels of `boxes`, coarsest first, while they fit [`MAX_OVERLAY_NODES`]; says what
/// was left out.
fn cap_levels(mut boxes: Vec<LodBox>, notes: &mut Vec<String>) -> Vec<LodBox> {
    let mut counts: BTreeMap<i8, usize> = BTreeMap::new();
    for b in &boxes {
        *counts.entry(b.level).or_default() += 1;
    }
    let mut total = 0;
    let mut finest_kept = None;
    for (&level, &n) in counts.iter().rev() {
        if total + n > MAX_OVERLAY_NODES {
            break;
        }
        total += n;
        finest_kept = Some(level);
    }
    let dropped: Vec<i8> = counts.keys().copied().filter(|l| finest_kept.is_none_or(|f| *l < f)).collect();
    if !dropped.is_empty() {
        notes.push(format!(
            "the overlay draws at most {MAX_OVERLAY_NODES} nodes: level{} {} left out",
            if dropped.len() > 1 { "s" } else { "" },
            dropped.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
        ));
    }
    boxes.retain(|b| finest_kept.is_some_and(|f| b.level >= f));
    boxes
}

/// The octree's nodes as overlay boxes, by level ([`WorldOverlay::Octree`]).
#[must_use]
pub fn octree_boxes(read: &ReadView, notes: &mut Vec<String>) -> Vec<LodBox> {
    let Some(grid) = read.grid else { return Vec::new() };
    let boxes = read
        .nodes
        .iter()
        .map(|n| LodBox {
            boxed: grid.brick_box(n.level, n.key),
            level: n.level as i8,
            tower: None,
            share: 0.0,
        })
        .collect();
    cap_levels(boxes, notes)
}

/// The LOD frame of `view` with `overlay`, from what the read made (`read`, None before the world
/// has been read): the draw list of a tower's or towers' view, merged to [`MAX_DRAW_LIST`] if need
/// be, and the overlay's boxes.
#[must_use]
pub fn lod_frame(view: &LodView, overlay: WorldOverlay, read: Option<&ReadView>) -> LodFrame {
    let mut frame = LodFrame { view: view.clone(), overlay, ..LodFrame::default() };
    let grid = read.and_then(|r| r.grid);
    let towers = view.towers();
    let cuts: Vec<(usize, &TowerCut)> = match read {
        Some(read) => towers
            .iter()
            .filter_map(|&n| match read.towers.get(n) {
                Some(cut) => Some((n, cut)),
                None => {
                    frame.notes.push(format!("there is no tower {}", n + 1));
                    None
                }
            })
            .collect(),
        None => Vec::new(),
    };
    if !towers.is_empty() && read.is_none() {
        frame
            .notes
            .push("the towers' cuts come with the world's read: drawn by the camera until then".into());
    }
    // The entries: the tower's cut, or the towers' common refinement, each with the tower that
    // set it.
    let refs: Vec<&TowerCut> = cuts.iter().map(|(_, c)| *c).collect();
    let (list, blocks): Refinement = if refs.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        let tower_of = |i: usize| cuts[i].0;
        (
            union(&refs).into_iter().map(|(e, i)| (e, tower_of(i))).collect(),
            union_inner(&refs).into_iter().map(|(k, i)| (k, tower_of(i))).collect(),
        )
    };
    if !refs.is_empty() {
        let mut drawn: Vec<Entry> = list.iter().map(|(e, _)| *e).collect();
        // Past the renderer's room, the finest level is merged into its parents until it fits.
        while drawn.len() > MAX_DRAW_LIST {
            let finest = drawn.iter().map(|e| e.0).min().unwrap_or(0);
            let mut merged: Vec<Entry> =
                drawn.iter().map(|&e| if e.0 == finest { ancestor(e, finest + 1) } else { e }).collect();
            merged.sort_unstable();
            merged.dedup();
            frame.notes.push(format!(
                "the view's {} entries are past the {MAX_DRAW_LIST} drawn: level {finest} is drawn by its parents",
                drawn.len()
            ));
            drawn = merged;
        }
        frame.list = Some(drawn);
    }
    let Some(grid) = grid else {
        if matches!(overlay, WorldOverlay::Octree | WorldOverlay::Tower | WorldOverlay::Ledger) {
            frame.notes.push(format!("the {} overlay comes with the world's read", overlay.name()));
        }
        return frame;
    };
    let read = read.expect("a grid comes with a read");
    let bound = |n: usize, e: Entry| -> f64 {
        let cut = &read.towers[n];
        cut.accepted
            .binary_search_by_key(&e, |(k, _)| (k.level, k.key))
            .map_or(0.0, |i| cut.accepted[i].1 / cut.ledger.max(f64::MIN_POSITIVE))
    };
    // The blocks below the chunk the view reads, as boxes at their levels (−4 … −1).
    let block_boxes = || -> Vec<LodBox> {
        blocks
            .iter()
            .map(|&(key, n)| {
                let cut = &read.towers[n];
                let share = cut
                    .inner
                    .binary_search_by_key(&key, |(k, _)| *k)
                    .map_or(0.0, |i| cut.inner[i].1 / cut.ledger.max(f64::MIN_POSITIVE));
                LodBox { boxed: inner_box(&grid, key), level: key.level as i8 - 5, tower: Some(n), share }
            })
            .collect()
    };
    let boxes: Vec<LodBox> = match overlay {
        WorldOverlay::None => Vec::new(),
        WorldOverlay::Octree => octree_boxes(read, &mut frame.notes),
        // The world is tinted by level where it is drawn; what lies below the chunk (merged into
        // its chunk's mesh) is outlined.
        WorldOverlay::Level => cap_levels(block_boxes(), &mut frame.notes),
        WorldOverlay::Tower | WorldOverlay::Ledger if refs.is_empty() => {
            frame.notes.push(format!(
                "the {} overlay outlines a tower's cut: choose `lod-view tower N`",
                overlay.name()
            ));
            Vec::new()
        }
        _ => {
            let mut boxes: Vec<LodBox> = list
                .iter()
                .map(|&((level, key), n)| LodBox {
                    boxed: grid.brick_box(level, key),
                    level: level as i8,
                    tower: Some(n),
                    share: bound(n, (level, key)),
                })
                .collect();
            boxes.extend(block_boxes());
            cap_levels(boxes, &mut frame.notes)
        }
    };
    let mut levels: Vec<i8> = boxes.iter().map(|b| b.level).collect::<BTreeSet<_>>().into_iter().collect();
    levels.sort_unstable();
    frame.levels = levels;
    frame.boxes = boxes;
    frame
}
