//! How voxel bodies are looked at: which field colours their leaves, which overlays are drawn, and
//! the cubes and boxes a view of them is made of.
//!
//! The engine makes a [`VoxelFrame`] for each pane that shows the scene; the renderer draws its
//! cubes and boxes, the application and `cav-plot` draw its legend. The legend's range is the
//! range of the values drawn, never a range chosen beforehand (`plan/REBUILD.md` §6: "legends
//! generated from what is drawn").
//!
//! **Bounded** (`docs/slices/S3.md` §3.1, V11 and V13): at most [`MAX_DRAWN_CUBES`] cubes and
//! [`MAX_OVERLAY_BOXES`] boxes a view; past either, a body is drawn by a cut one level coarser
//! until it fits, and the frame says so. The `phase` field reads each leaf on its own, within
//! [`cav_core::voxel::kernel::MAX_KERNEL_WORK`].

use std::sync::Arc;

use cav_core::Vec3;
use cav_core::forward::{PHASE_TOLERANCE, Reader};
use cav_core::voxel::VoxTree;
use cav_core::voxel::kernel::{MAX_KERNEL_WORK, TreeBody};
use cav_core::voxel::tree::CutEntry;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::SceneSpec;

/// The most leaf cubes drawn in one view.
pub const MAX_DRAWN_CUBES: usize = 20_000;
/// The most overlay boxes drawn in one view.
pub const MAX_OVERLAY_BOXES: usize = 20_000;

/// What colours a voxel body's leaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub enum LeafField {
    /// The leaf's level in its tree: 0 for a finest voxel.
    #[default]
    Level,
    /// The leaf's mass, kilograms.
    Mass,
    /// How full the leaf is: its sample points inside over all of them, from 0 to 1.
    Fill,
    /// The height of the leaf's centre, metres.
    Height,
    /// From the leaf's centre to the nearest tower's axis, metres.
    Distance,
    /// What the leaf alone adds to a tower's ΔΦ, radians, the body held where it is at the
    /// playhead: "what this tower hears from where".
    Phase,
}

impl LeafField {
    /// Every field, in the order menus list them.
    pub const ALL: [Self; 6] = [Self::Level, Self::Mass, Self::Fill, Self::Height, Self::Distance, Self::Phase];

    /// The word commands use.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Level => "level",
            Self::Mass => "mass",
            Self::Fill => "fill",
            Self::Height => "height",
            Self::Distance => "distance",
            Self::Phase => "phase",
        }
    }

    /// The field a word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.name() == word)
    }

    /// Its unit, as a legend writes it.
    #[must_use]
    pub const fn unit(self) -> &'static str {
        match self {
            Self::Level => "level",
            Self::Mass => "kg",
            Self::Fill => "fraction",
            Self::Height | Self::Distance => "m",
            Self::Phase => "rad",
        }
    }

    /// Whether its values have a sign that matters (a diverging map about zero) rather than a
    /// size (a sequential map from the least to the greatest).
    #[must_use]
    pub const fn diverging(self) -> bool {
        matches!(self, Self::Phase)
    }
}

/// How a pane colours voxel bodies: the field, and for `phase` the tower whose phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Colouring {
    /// The field.
    pub field: LeafField,
    /// The tower whose phase `phase` shows, from 0. Ignored by the other fields.
    #[serde(default)]
    pub tower: usize,
}

/// What a pane draws over voxel bodies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Overlay {
    /// Each leaf's bounds, outlined in a colour for its level.
    #[serde(default)]
    pub leaves: bool,
    /// The cut at this level (each node of that level and each leaf at or above it), outlined:
    /// the level of detail a coarse view would take. None for no cut.
    #[serde(default)]
    pub cut: Option<u8>,
}

/// One cube to draw: a leaf, or a node of a cut standing for its leaves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cube {
    /// Its centre, metres, the world's frame.
    pub centre: Vec3,
    /// Its side, metres.
    pub side: f64,
    /// Its value of the field, in the field's unit.
    pub value: f64,
    /// Its level in the tree.
    pub level: u8,
}

/// One box of an overlay: an axis-aligned box in the world's frame and the level it is of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayBox {
    /// The corner with the least x, y and z, metres.
    pub lo: Vec3,
    /// The corner with the greatest x, y and z, metres.
    pub hi: Vec3,
    /// Its level in its tree.
    pub level: u8,
}

/// A body made of voxels, as one view draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyCubes {
    /// The body, from 0 in the scene's order.
    pub body: usize,
    /// The cubes, in Morton order.
    pub cubes: Vec<Cube>,
    /// The level of the cut drawn: 0 when every leaf is drawn as itself.
    pub cut: u8,
}

/// Everything a view draws of the scene's voxel bodies, and what its legend says.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct VoxelFrame {
    /// Each voxel body there is at the playhead.
    pub bodies: Vec<BodyCubes>,
    /// The overlays' boxes.
    pub boxes: Vec<OverlayBox>,
    /// The field and tower drawn.
    pub colouring: Colouring,
    /// The least and the greatest value drawn, in the field's unit; for a diverging field,
    /// `−m` and `m` with `m` the largest magnitude drawn. None when nothing is drawn.
    pub range: Option<(f64, f64)>,
    /// What a person should know: a cut drawn in place of the leaves, a field that could not be
    /// made, and why.
    pub notes: Vec<String>,
}

/// The world position of body `n` at `time` (its tree's origin), metres.
fn body_at(scene: &SceneSpec, n: usize, time: f64) -> Vec3 {
    Vec3::from_array(scene.bodies[n].motion.position(time, scene.array.instrument.gravity))
}

/// The leaves' share of tower `tower`'s ΔΦ, radians, each leaf alone, the body held at `at`
/// (metres, the world's frame) and read out at `time`.
///
/// # Errors
/// When there is no such tower, or the leaves × flight nodes are more than
/// [`MAX_KERNEL_WORK`], with the numbers.
pub fn leaf_phases(scene: &SceneSpec, tree: &VoxTree, at: Vec3, tower: usize, time: f64) -> Result<Vec<f64>, String> {
    let base = scene
        .array
        .towers
        .get(tower)
        .ok_or_else(|| format!("there is no tower {}", tower + 1))?
        .position;
    let reader = Reader::new(scene.array.instrument.instrument());
    let nodes = reader.instrument().node_count(&[]);
    let work = 4 * nodes * tree.leaves.len() as u64;
    if work > MAX_KERNEL_WORK {
        return Err(format!(
            "{} leaves × {nodes} flight nodes is more than the {MAX_KERNEL_WORK} cube evaluations allowed",
            tree.leaves.len()
        ));
    }
    let sign = reader.instrument().sign;
    let one = |leaf: &cav_core::voxel::Leaf| {
        // The leaf on its own, read with the rule of the instrument: one rule, and its check.
        let body = TreeBody::single(tree, leaf, |_| at);
        reader.read_to_within(Vec3::from_array(base), &body, time, PHASE_TOLERANCE, 3 * nodes).reading.gradiometer(sign)
    };
    Ok(tree.leaves.iter().map(one).collect())
}

/// The voxel frame of `scene` at `time`, with `trees[n]` body `n`'s tree (None for a body that is
/// not made of voxels), coloured by `colouring`, with `overlay`.
#[must_use]
pub fn voxel_frame(
    scene: &SceneSpec,
    trees: &[Option<Arc<VoxTree>>],
    colouring: Colouring,
    overlay: Overlay,
    time: f64,
) -> VoxelFrame {
    let mut frame = VoxelFrame { colouring, ..VoxelFrame::default() };
    let present: Vec<(usize, &Arc<VoxTree>)> = trees
        .iter()
        .enumerate()
        .filter_map(|(n, tree)| {
            let (from, until) = scene.bodies.get(n)?.exists();
            tree.as_ref().filter(|_| time >= from && time < until).map(|t| (n, t))
        })
        .collect();
    // The cut level: the finest at which every body together fits the budget.
    let fits = |level: u8| present.iter().map(|(_, t)| cut_size(t, level)).sum::<usize>() <= MAX_DRAWN_CUBES;
    let deepest = present.iter().map(|(_, t)| t.grid.depth).max().unwrap_or(0);
    let level = (0..=deepest).find(|&l| fits(l)).unwrap_or(deepest);
    if level > 0 {
        frame.notes.push(format!("more than {MAX_DRAWN_CUBES} leaves: drawn by the cut at level {level}"));
    }
    let towers: Vec<Vec3> = scene.array.towers.iter().map(|t| Vec3::from_array(t.position)).collect();
    for (n, tree) in present {
        let at = body_at(scene, n, time);
        let phases = if colouring.field == LeafField::Phase {
            match leaf_phases(scene, tree, at, colouring.tower, time) {
                Ok(phases) => Some(phases),
                Err(why) => {
                    frame.notes.push(format!("`{}` is not coloured by phase: {why}", scene.bodies[n].label));
                    None
                }
            }
        } else {
            None
        };
        let k3 = f64::from(tree.grid.samples_per_cell());
        let mut cubes = Vec::new();
        for entry in tree.cut(level) {
            let (key, lvl, mass, leaves) = match entry {
                CutEntry::Leaf(l) => {
                    let leaf = tree.leaves[l];
                    (leaf.key, leaf.level, tree.leaf_mass(&leaf), l..l + 1)
                }
                CutEntry::Node(m) => {
                    let node = tree.nodes[m];
                    (node.key, node.level, tree.moments[m].mass, node.first_leaf as usize..node.end_leaf as usize)
                }
            };
            let centre = at + tree.block_centre(key, lvl);
            let side = tree.side(lvl);
            let full = tree.unit * k3 * f64::from(1u32 << (3 * u32::from(lvl)));
            let value = match colouring.field {
                LeafField::Level => f64::from(lvl),
                LeafField::Mass => mass,
                LeafField::Fill => mass / full,
                LeafField::Height => centre.z,
                LeafField::Distance => towers
                    .iter()
                    .map(|t| (centre.x - t.x).hypot(centre.y - t.y))
                    .fold(f64::INFINITY, f64::min),
                LeafField::Phase => phases.as_ref().map_or(0.0, |p| p[leaves].iter().sum()),
            };
            cubes.push(Cube { centre, side, value, level: lvl });
        }
        if overlay.leaves || overlay.cut.is_some() {
            let leaves_boxes = overlay.leaves.then_some(0u8);
            for cut in [leaves_boxes, overlay.cut].into_iter().flatten() {
                for entry in tree.cut(cut) {
                    if frame.boxes.len() >= MAX_OVERLAY_BOXES {
                        break;
                    }
                    let (key, lvl) = match entry {
                        CutEntry::Leaf(l) => (tree.leaves[l].key, tree.leaves[l].level),
                        CutEntry::Node(m) => (tree.nodes[m].key, tree.nodes[m].level),
                    };
                    let c = at + tree.block_centre(key, lvl);
                    let h = 0.5 * tree.side(lvl);
                    let d = Vec3::new(h, h, h);
                    frame.boxes.push(OverlayBox { lo: c - d, hi: c + d, level: lvl });
                }
            }
        }
        frame.bodies.push(BodyCubes { body: n, cubes, cut: level });
    }
    if frame.boxes.len() >= MAX_OVERLAY_BOXES {
        frame.notes.push(format!("the overlay stops at {MAX_OVERLAY_BOXES} boxes"));
    }
    let values = frame.bodies.iter().flat_map(|b| b.cubes.iter().map(|c| c.value)).filter(|v| v.is_finite());
    frame.range = values.fold(None, |r: Option<(f64, f64)>, v| Some(r.map_or((v, v), |(a, b)| (a.min(v), b.max(v)))));
    if colouring.field.diverging() {
        frame.range = frame.range.map(|(a, b)| {
            let m = a.abs().max(b.abs());
            (-m, m)
        });
    }
    frame
}

/// How many entries the cut of `tree` at `level` has, without making it.
fn cut_size(tree: &VoxTree, level: u8) -> usize {
    tree.leaves.iter().filter(|l| l.level >= level).count() + tree.nodes.iter().filter(|n| n.level == level).count()
}
