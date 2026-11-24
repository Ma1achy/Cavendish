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
/// The most tree-overlay nodes drawn in one view (each twelve edges, and twelve triangles with
/// faces on): past it, whole depths from the deepest up are left out until the rest fit.
pub const MAX_TREE_BOXES: usize = 20_000;

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
    /// The leaf's depth in its tree: 0 for the root, its tree's `grid.depth` for a finest voxel
    /// (`level` counted from the other end). On the same map and range as the tree overlay
    /// ([`TreeOverlay`]), so that a leaf and a node of one depth are one colour.
    Depth,
}

impl LeafField {
    /// Every field, in the order menus list them.
    pub const ALL: [Self; 7] =
        [Self::Level, Self::Depth, Self::Mass, Self::Fill, Self::Height, Self::Distance, Self::Phase];

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
            Self::Depth => "depth",
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
            Self::Depth => "depth",
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
    /// The whole tree: every node, internal and leaf, from the root down, outlined in a colour
    /// for its depth. None for no tree.
    #[serde(default)]
    pub tree: Option<TreeOverlay>,
}

/// How the tree overlay fades with depth: which end is drawn faint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub enum Fade {
    /// Every depth solid.
    #[default]
    Off,
    /// Deep nodes solid, shallow ones faint: the fine structure stands out.
    Deep,
    /// Shallow nodes solid, deep ones faint: the coarse structure stands out.
    Shallow,
}

impl Fade {
    /// Every fade, in the order menus list them.
    pub const ALL: [Self; 3] = [Self::Off, Self::Deep, Self::Shallow];

    /// The word commands use.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Deep => "deep",
            Self::Shallow => "shallow",
        }
    }

    /// The fade a word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.name() == word)
    }
}

/// The opacity of the faintest end of a fading tree overlay, 0 to 1: faint, never gone.
pub const FAINTEST: f64 = 0.15;

/// The tree overlay: which depths are drawn and how.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TreeOverlay {
    /// The shallowest depth drawn: 0 for the root.
    #[serde(default)]
    pub from: u8,
    /// The deepest depth drawn; None for every depth down to the finest.
    #[serde(default)]
    pub to: Option<u8>,
    /// Opacity by depth.
    #[serde(default)]
    pub fade: Fade,
    /// Translucent faces as well as edges. Faces are see-through only in the x-ray style (which
    /// adds light); the other styles draw edges alone.
    #[serde(default)]
    pub faces: bool,
    /// The tree alone: the bodies' cubes are not drawn, so every node can be seen. A section's
    /// cells (the material in the plane) still are.
    #[serde(default)]
    pub alone: bool,
}

impl TreeOverlay {
    /// Whether depth `depth` is in the range asked for.
    #[must_use]
    pub fn holds(&self, depth: u8) -> bool {
        depth >= self.from && self.to.is_none_or(|to| depth <= to)
    }
}

/// The opacity of a tree node at `depth` with the depths drawn `(shallowest, deepest)`, from
/// [`FAINTEST`] to 1: 1 throughout with no fade, rising with depth for [`Fade::Deep`], falling
/// for [`Fade::Shallow`]. One depth drawn is solid.
#[must_use]
pub fn opacity(depth: u8, (shallowest, deepest): (u8, u8), fade: Fade) -> f64 {
    let span = f64::from(deepest.saturating_sub(shallowest));
    let t = if span > 0.0 { (f64::from(depth.clamp(shallowest, deepest) - shallowest)) / span } else { 1.0 };
    match fade {
        Fade::Off => 1.0,
        Fade::Deep => FAINTEST + (1.0 - FAINTEST) * t,
        Fade::Shallow if span > 0.0 => FAINTEST + (1.0 - FAINTEST) * (1.0 - t),
        Fade::Shallow => 1.0,
    }
}

/// One node of the tree overlay, internal or leaf, in the world's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeBox {
    /// The corner with the least x, y and z, metres.
    pub lo: Vec3,
    /// The corner with the greatest x, y and z, metres.
    pub hi: Vec3,
    /// Its depth in its tree: 0 for the root.
    pub depth: u8,
    /// Whether it is a leaf (else an internal node).
    pub leaf: bool,
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

/// A block the section plane cuts, clipped: an axis-aligned box (flat in a plan view), the
/// world's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    /// The corner with the least x, y and z, metres.
    pub lo: Vec3,
    /// The corner with the greatest x, y and z, metres.
    pub hi: Vec3,
    /// Its value of the field, in the field's unit.
    pub value: f64,
    /// Its level in the tree.
    pub level: u8,
}

/// A section plane through the voxel bodies: square to an axis, at an offset along it. Part of a
/// view (`section` command).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Section {
    /// The axis the plane is square to.
    pub axis: crate::shape::Axis,
    /// Where the plane crosses that axis, metres, the world's frame.
    #[schemars(range(min = -1.0e7, max = 1.0e7))]
    #[schemars(extend("x-unit" = "m"))]
    pub offset: f64,
}

/// How close to square-on a view must look along a section's axis to draw it as a 2-D section:
/// the cosine of the angle between them (2.6°). Named views are exactly square-on.
pub const PLAN_COSINE: f64 = 0.999;

/// A section as one view takes it: the plane, and the direction from the view's target to its
/// camera (a unit vector, the world's axes).
///
/// Looked at square-on (a plan view, within [`PLAN_COSINE`]) the section is 2-D: each block the
/// plane passes through, flattened to the plane. Otherwise it is a cut-away: the half of the
/// bodies on the camera's side of the plane is taken away, the blocks the plane passes through
/// clipped to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cutting {
    /// The plane.
    pub section: Section,
    /// From the target to the camera, unit length.
    pub eye: Vec3,
}

impl Cutting {
    fn axis(&self) -> usize {
        self.section.axis as usize
    }

    /// Whether the view looks square-on along the section's axis.
    #[must_use]
    pub fn plan(&self) -> bool {
        self.eye.to_array()[self.axis()].abs() >= PLAN_COSINE
    }

    /// The box `lo`..`hi` (metres, the world's frame) as this cutting draws it: flattened to the
    /// plane in a plan view (when the plane passes through it, its low face counting and its high
    /// face not), clipped to the kept half otherwise; None when nothing of it is drawn.
    #[must_use]
    pub fn place(&self, lo: Vec3, hi: Vec3) -> Option<(Vec3, Vec3)> {
        let a = self.axis();
        let o = self.section.offset;
        let (mut lo, mut hi) = (lo.to_array(), hi.to_array());
        if self.plan() {
            if !(lo[a] <= o && o < hi[a]) {
                return None;
            }
            (lo[a], hi[a]) = (o, o);
        } else if self.eye.to_array()[a] >= 0.0 {
            // The camera on the high side: the low half is kept.
            if lo[a] >= o {
                return None;
            }
            hi[a] = hi[a].min(o);
        } else {
            if hi[a] <= o {
                return None;
            }
            lo[a] = lo[a].max(o);
        }
        Some((Vec3::from_array(lo), Vec3::from_array(hi)))
    }

    /// A small step towards the camera along the axis, metres, for outlines drawn on a 2-D section
    /// so they are not lost in the depth test against the cells under them: a fiftieth of `pitch`.
    fn nudge(&self, pitch: f64) -> Vec3 {
        let mut d = [0.0; 3];
        d[self.axis()] = 0.02 * pitch * self.eye.to_array()[self.axis()].signum();
        Vec3::from_array(d)
    }

    /// [`Self::place`] for a block of `tree` with its origin at `at`.
    fn place_block(&self, tree: &VoxTree, at: Vec3, block: (u32, u8)) -> Option<(Vec3, Vec3)> {
        let (lo, hi) = box_of(tree, at, block);
        self.place(lo, hi)
    }

    /// [`Self::place`] for an outline of a block of `tree`: on a 2-D section, a little towards
    /// the camera.
    fn place_outline(&self, tree: &VoxTree, lo: Vec3, hi: Vec3) -> Option<(Vec3, Vec3)> {
        let (lo, hi) = self.place(lo, hi)?;
        let d = if self.plan() { self.nudge(tree.side(0)) } else { Vec3::ZERO };
        Some((lo + d, hi + d))
    }
}

/// From a view's target to its camera, unit length: `yaw` degrees anticlockwise from east,
/// `pitch` degrees above the horizontal.
#[must_use]
pub fn eye_direction(yaw: f64, pitch: f64) -> Vec3 {
    let (y, p) = (yaw.to_radians(), pitch.to_radians());
    Vec3::new(p.cos() * y.cos(), p.cos() * y.sin(), p.sin())
}

/// The span of the scene's voxel bodies along `axis` at `time`, metres, the world's frame: where
/// a section plane finds something. None when no body is made of voxels (a mesh's extent is not
/// known without its file and counts as 2 m either side of its position).
#[must_use]
pub fn section_span(scene: &SceneSpec, axis: crate::shape::Axis, time: f64) -> Option<(f64, f64)> {
    let a = axis as usize;
    scene
        .bodies
        .iter()
        .enumerate()
        .filter_map(|(n, body)| {
            let shape = body.shape.as_ref()?;
            let at = body_at(scene, n, time).to_array()[a];
            let (lo, hi) = match &shape.kind {
                crate::ShapeKind::Mesh { .. } => (-2.0, 2.0),
                kind => kind.solid().ok().map(|s| {
                    let b = s.bounds();
                    (b.lo.to_array()[a], b.hi.to_array()[a])
                })?,
            };
            Some((at + lo, at + hi))
        })
        .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
}

/// A body made of voxels, as one view draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyCubes {
    /// The body, from 0 in the scene's order.
    pub body: usize,
    /// The cubes, in Morton order.
    pub cubes: Vec<Cube>,
    /// With a section: the leaves (or cut entries) the plane cuts, clipped to the half the view
    /// keeps (a cut-away), or flattened to the plane itself in a plan view (a 2-D section). They
    /// are the material where the plane passes; where the body is hollow there is none.
    pub cells: Vec<Cell>,
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
    /// The tree overlay's nodes, every body's, shallowest first; at most [`MAX_TREE_BOXES`].
    pub tree: Vec<TreeBox>,
    /// The section the frame was cut by, if any.
    pub section: Option<Cutting>,
    /// The tree overlay as asked for, when it is on.
    pub tree_overlay: Option<TreeOverlay>,
    /// The shallowest and the deepest depth the tree overlay's map runs over: those drawn, and
    /// with [`LeafField::Depth`] the leaves' depths too, so leaves and nodes agree. None when no
    /// tree node is drawn.
    pub depths: Option<(u8, u8)>,
    /// The deepest depth the tree overlay stopped at to stay within [`MAX_TREE_BOXES`], when it
    /// had to: the depths below it are not drawn.
    pub tree_capped_at: Option<u8>,
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
pub fn leaf_phases(
    scene: &SceneSpec,
    tree: &VoxTree,
    at: Vec3,
    tower: usize,
    time: f64,
) -> Result<Vec<f64>, String> {
    let base =
        scene.array.towers.get(tower).ok_or_else(|| format!("there is no tower {}", tower + 1))?.position;
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
        reader
            .read_to_within(Vec3::from_array(base), &body, time, PHASE_TOLERANCE, 3 * nodes)
            .reading
            .gradiometer(sign)
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
    voxel_frame_cut(scene, trees, colouring, overlay, None, time)
}

/// [`voxel_frame`] with a section: every cube, cell and overlay box is as `cutting` places it
/// ([`Cutting::place`]), and the budgets count only what it keeps.
#[must_use]
pub fn voxel_frame_cut(
    scene: &SceneSpec,
    trees: &[Option<Arc<VoxTree>>],
    colouring: Colouring,
    overlay: Overlay,
    cutting: Option<Cutting>,
    time: f64,
) -> VoxelFrame {
    let mut frame = VoxelFrame { colouring, section: cutting, ..VoxelFrame::default() };
    let present: Vec<(usize, &Arc<VoxTree>)> = trees
        .iter()
        .enumerate()
        .filter_map(|(n, tree)| {
            let (from, until) = scene.bodies.get(n)?.exists();
            tree.as_ref().filter(|_| time >= from && time < until).map(|t| (n, t))
        })
        .collect();
    // The cut level: the finest at which every body together fits the budget.
    let fits = |level: u8| {
        let size = |&(n, tree): &(usize, &Arc<VoxTree>)| match cutting {
            None => cut_size(tree, level),
            Some(c) => {
                let at = body_at(scene, n, time);
                tree.cut(level)
                    .into_iter()
                    .filter(|&e| c.place_block(tree, at, block(tree, e)).is_some())
                    .count()
            }
        };
        present.iter().map(size).sum::<usize>() <= MAX_DRAWN_CUBES
    };
    let deepest = present.iter().map(|(_, t)| t.grid.depth).max().unwrap_or(0);
    let level = (0..=deepest).find(|&l| fits(l)).unwrap_or(deepest);
    if level > 0 {
        frame.notes.push(format!("more than {MAX_DRAWN_CUBES} leaves: drawn by the cut at level {level}"));
    }
    if let Some(asked) = overlay.tree {
        tree_overlay(scene, &present, asked, cutting, time, &mut frame);
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
        let mut cells = Vec::new();
        for entry in tree.cut(level) {
            let whole = box_of(tree, at, block(tree, entry));
            let placed = cutting.map(|c| c.place(whole.0, whole.1));
            if placed == Some(None) {
                continue;
            }
            let (key, lvl, mass, leaves) = match entry {
                CutEntry::Leaf(l) => {
                    let leaf = tree.leaves[l];
                    (leaf.key, leaf.level, tree.leaf_mass(&leaf), l..l + 1)
                }
                CutEntry::Node(m) => {
                    let node = tree.nodes[m];
                    (
                        node.key,
                        node.level,
                        tree.moments[m].mass,
                        node.first_leaf as usize..node.end_leaf as usize,
                    )
                }
            };
            let centre = at + tree.block_centre(key, lvl);
            let side = tree.side(lvl);
            let full = tree.unit * k3 * f64::from(1u32 << (3 * u32::from(lvl)));
            let value = match colouring.field {
                LeafField::Level => f64::from(lvl),
                LeafField::Depth => f64::from(tree.grid.depth.saturating_sub(lvl)),
                LeafField::Mass => mass,
                LeafField::Fill => mass / full,
                LeafField::Height => centre.z,
                LeafField::Distance => towers
                    .iter()
                    .map(|t| (centre.x - t.x).hypot(centre.y - t.y))
                    .fold(f64::INFINITY, f64::min),
                LeafField::Phase => phases.as_ref().map_or(0.0, |p| p[leaves].iter().sum()),
            };
            match placed {
                Some(Some(cell)) if cell != whole => {
                    cells.push(Cell { lo: cell.0, hi: cell.1, value, level: lvl });
                }
                _ => cubes.push(Cube { centre, side, value, level: lvl }),
            }
        }
        if overlay.leaves || overlay.cut.is_some() {
            let leaves_boxes = overlay.leaves.then_some(0u8);
            for cut in [leaves_boxes, overlay.cut].into_iter().flatten() {
                for entry in tree.cut(cut) {
                    if frame.boxes.len() >= MAX_OVERLAY_BOXES {
                        break;
                    }
                    let (key, lvl) = block(tree, entry);
                    let (lo, hi) = box_of(tree, at, (key, lvl));
                    let Some((lo, hi)) = cutting.map_or(Some((lo, hi)), |c| c.place_outline(tree, lo, hi))
                    else {
                        continue;
                    };
                    frame.boxes.push(OverlayBox { lo, hi, level: lvl });
                }
            }
        }
        frame.bodies.push(BodyCubes { body: n, cubes, cells, cut: level });
    }
    if frame.boxes.len() >= MAX_OVERLAY_BOXES {
        frame.notes.push(format!("the overlay stops at {MAX_OVERLAY_BOXES} boxes"));
    }
    if overlay.tree.is_some_and(|t| t.alone) {
        for body in &mut frame.bodies {
            body.cubes.clear();
        }
    }
    let values = frame
        .bodies
        .iter()
        .flat_map(|b| b.cubes.iter().map(|c| c.value).chain(b.cells.iter().map(|c| c.value)))
        .filter(|v| v.is_finite());
    frame.range =
        values.fold(None, |r: Option<(f64, f64)>, v| Some(r.map_or((v, v), |(a, b)| (a.min(v), b.max(v)))));
    if colouring.field.diverging() {
        frame.range = frame.range.map(|(a, b)| {
            let m = a.abs().max(b.abs());
            (-m, m)
        });
    }
    // Leaves by depth and the tree's nodes on one map: each range takes in the other.
    if colouring.field == LeafField::Depth
        && let (Some((a, b)), Some((lo, hi))) = (frame.depths, frame.range)
    {
        let (lo, hi) = (lo.min(f64::from(a)), hi.max(f64::from(b)));
        frame.range = Some((lo, hi));
        frame.depths = Some((lo as u8, hi as u8));
    }
    frame
}

/// The key and level of a cut's entry.
fn block(tree: &VoxTree, entry: CutEntry) -> (u32, u8) {
    match entry {
        CutEntry::Leaf(l) => (tree.leaves[l].key, tree.leaves[l].level),
        CutEntry::Node(m) => (tree.nodes[m].key, tree.nodes[m].level),
    }
}

/// The box of the block `(key, level)` of `tree` with the tree's origin at `at`: its least and
/// greatest corners, metres, the world's frame.
fn box_of(tree: &VoxTree, at: Vec3, (key, level): (u32, u8)) -> (Vec3, Vec3) {
    let c = at + tree.block_centre(key, level);
    let h = 0.5 * tree.side(level);
    let d = Vec3::new(h, h, h);
    (c - d, c + d)
}

/// The depth of a block of `level` in `tree`: 0 for the root.
fn depth_of(tree: &VoxTree, level: u8) -> u8 {
    tree.grid.depth.saturating_sub(level)
}

/// Fills `frame`'s tree overlay: every node and leaf of the `present` bodies at the depths
/// `asked` takes, shallowest first, whole depths left out from the deepest up past
/// [`MAX_TREE_BOXES`] (and the frame says so).
fn tree_overlay(
    scene: &SceneSpec,
    present: &[(usize, &Arc<VoxTree>)],
    asked: TreeOverlay,
    cutting: Option<Cutting>,
    time: f64,
    frame: &mut VoxelFrame,
) {
    let kept = |tree: &VoxTree, at: Vec3, block: (u32, u8)| {
        let (lo, hi) = box_of(tree, at, block);
        cutting.map_or(Some((lo, hi)), |c| c.place_outline(tree, lo, hi))
    };
    frame.tree_overlay = Some(asked);
    // How many blocks there are at each depth, every body together, counted before any is made.
    let mut counts = [0usize; 33];
    for &(n, tree) in present {
        let at = body_at(scene, n, time);
        let blocks =
            tree.nodes.iter().map(|b| (b.key, b.level)).chain(tree.leaves.iter().map(|l| (l.key, l.level)));
        for (key, level) in blocks {
            if cutting.is_none() || kept(tree, at, (key, level)).is_some() {
                counts[usize::from(depth_of(tree, level)).min(32)] += 1;
            }
        }
    }
    let mut deepest = None;
    let mut total = 0;
    for depth in (0..=32u8).filter(|&d| asked.holds(d) && counts[usize::from(d)] > 0) {
        let more = counts[usize::from(depth)];
        if total + more > MAX_TREE_BOXES {
            // One depth alone past the budget is drawn in part, never not at all.
            if deepest.is_none() {
                deepest = Some(depth);
            }
            frame.tree_capped_at = deepest;
            break;
        }
        total += more;
        deepest = Some(depth);
    }
    let Some(deepest) = deepest else {
        frame.notes.push(format!(
            "the tree overlay has no node at depths {}{}",
            asked.from,
            asked.to.map_or_else(|| " and deeper".to_owned(), |to| format!(" to {to}"))
        ));
        return;
    };
    let wanted = |depth: u8| asked.holds(depth) && depth <= deepest;
    for (n, tree) in present {
        let at = body_at(scene, *n, time);
        let blocks = tree.nodes.iter().map(|b| (b.key, b.level, false));
        for (key, level, leaf) in blocks.chain(tree.leaves.iter().map(|l| (l.key, l.level, true))) {
            let depth = depth_of(tree, level);
            if wanted(depth)
                && let Some((lo, hi)) = kept(tree, at, (key, level))
            {
                frame.tree.push(TreeBox { lo, hi, depth, leaf });
            }
        }
    }
    frame.tree.sort_by_key(|b| b.depth);
    frame.tree.truncate(MAX_TREE_BOXES);
    let lo = frame.tree.first().map_or(deepest, |b| b.depth);
    frame.depths = Some((lo, deepest));
    if let Some(depth) = frame.tree_capped_at {
        frame.notes.push(format!(
            "more than {MAX_TREE_BOXES} tree nodes: the tree overlay is drawn to depth {depth} of {}",
            present.iter().map(|(_, t)| t.grid.depth).max().unwrap_or(0)
        ));
    }
}

/// How many entries the cut of `tree` at `level` has, without making it.
fn cut_size(tree: &VoxTree, level: u8) -> usize {
    tree.leaves.iter().filter(|l| l.level >= level).count()
        + tree.nodes.iter().filter(|n| n.level == level).count()
}
