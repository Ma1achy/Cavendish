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
pub use crate::body_draw::{BodyDraw, MAX_DRAWN_TRIANGLES, Surface};
pub use cav_core::voxel::cut::PieceSurface;

/// The most leaf cubes drawn in one view.
pub const MAX_DRAWN_CUBES: usize = 20_000;
/// The most overlay boxes drawn in one view.
pub const MAX_OVERLAY_BOXES: usize = 20_000;
/// The most tree-overlay nodes drawn in one view (each twelve edges, and twelve triangles with
/// faces on): past it, whole depths from the deepest up are left out until the rest fit. A first
/// asked depth that alone has more is not drawn at all (the frame says so and names the deepest
/// that would fit): a part of a depth would be an arbitrary choice of its blocks.
pub const MAX_TREE_BOXES: usize = 20_000;
/// How many times the flight's nodes one leaf's read may refine to when leaves are coloured by
/// phase ([`leaf_phases`]); the work counted against the kernel's share is this ceiling.
pub const LEAF_REFINEMENT: u64 = 3;

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
    /// What the leaf is made of: each in its material's own colour (S19); a uniform body in
    /// [`UNIFORM_COLOUR`]. Its value is the material's place in the body's palette.
    Material,
}

/// The colour of a body with no material when the view colours by material: a neutral grey,
/// sRGB.
pub const UNIFORM_COLOUR: [u8; 3] = [0x9a, 0x9a, 0x9a];

impl LeafField {
    /// Every field, in the order menus list them.
    pub const ALL: [Self; 8] = [
        Self::Level,
        Self::Depth,
        Self::Mass,
        Self::Fill,
        Self::Height,
        Self::Distance,
        Self::Phase,
        Self::Material,
    ];

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
            Self::Material => "material",
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
            Self::Material => "material",
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
    /// The world's clipmap drawn in colours of its levels (the LOD bands, S4) in place of its
    /// materials: level 0's meshes, then each ray-marched level.
    #[serde(default)]
    pub bands: bool,
    /// A small cross at the centroid of each cut cell's piece (S5): where `Centroid` and `Second`
    /// put a boundary cell's mass. Only for a tree with exact pieces, drawn leaf by leaf.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub centroids: bool,
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
    /// Its own colour, sRGB, when the field names one (a material's, [`LeafField::Material`]);
    /// otherwise none, and the field's map colours it by its value.
    pub colour: Option<[u8; 3]>,
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
    /// Its own colour, as a [`Cube`]'s.
    pub colour: Option<[u8; 3]>,
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
            let (lo, hi) = match shape.kind.bounds() {
                None => (-2.0, 2.0),
                Some(b) => (b.lo.to_array()[a], b.hi.to_array()[a]),
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
    /// Where the body is at the frame's time: its tree's origin and its surface's, metres, the
    /// world's frame.
    pub at: Vec3,
    /// Its shape's surface, in the body's frame (add [`Self::at`]), when the view's draw mode
    /// draws it ([`BodyDraw::draws_surface`]); None otherwise, and for a body whose surface could
    /// not be made or did not fit (the frame's notes say why).
    pub surface: Option<Arc<Surface>>,
    /// Its cut cells drawn as their pieces in [`BodyDraw::Pieces`] (those not drawn as
    /// [`Self::cubes`]); None in the other modes and when they could not be drawn (said).
    pub pieces: Option<DrawnPieces>,
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
    /// With [`Overlay::centroids`]: each drawn cut cell's piece's centroid, metres, the world's
    /// frame, and its cell's side, metres (the mark's size); at most [`MAX_OVERLAY_BOXES`].
    pub centroids: Vec<(Vec3, f64)>,
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
    /// had to: the depths below it are not drawn. None when nothing had to be left out, and when
    /// the first asked depth alone is past the cap (then no node is drawn and a note says so).
    pub tree_capped_at: Option<u8>,
    /// With [`LeafField::Material`]: each material drawn, its label and colour, in the order
    /// first met (a uniform body as "uniform"), for the legend. Empty for the other fields.
    pub materials: Vec<(String, [u8; 3])>,
    /// How the bodies are drawn: their voxels solid, or as outlines under their surface, or
    /// their surface alone ([`BodyDraw`]). In [`BodyDraw::Fill`] the colouring is the exact fill
    /// whatever the view's `colour` says.
    pub draw: BodyDraw,
}

/// What a body's draw mode needs beyond its tree, made and kept by the engine: its surface and
/// its cut cells' pieces, each in the body's frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extras {
    /// The shape's surface, for [`BodyDraw::Mesh`] and [`BodyDraw::MeshOverVoxels`].
    pub surface: Option<Arc<Surface>>,
    /// The pieces of the cut cells of the tree drawn, in the order of its `cuts` (so by key), for
    /// [`BodyDraw::Pieces`].
    pub pieces: Option<Arc<Vec<PieceSurface>>>,
}

/// A body's cut cells drawn as their pieces: which of its pieces, each with its value of the
/// field (its exact fill).
#[derive(Clone, Debug, PartialEq)]
pub struct DrawnPieces {
    /// Every piece of the body's tree ([`Extras::pieces`]).
    pub surfaces: Arc<Vec<PieceSurface>>,
    /// The pieces drawn: an index into `surfaces` and the value, in the field's unit.
    pub drawn: Vec<(usize, f64)>,
}

/// How one view looks at the voxel bodies: the field, the overlays, the section and the draw
/// mode, as its [`crate::ViewSpec`] says.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Look {
    /// What colours the leaves (overridden by [`BodyDraw::Fill`]).
    pub colouring: Colouring,
    /// What is drawn over the bodies.
    pub overlay: Overlay,
    /// The section, if any.
    pub cutting: Option<Cutting>,
    /// The draw mode.
    pub draw: BodyDraw,
}

/// The world position of body `n` at `time` (its tree's origin), metres.
fn body_at(scene: &SceneSpec, n: usize, time: f64) -> Vec3 {
    Vec3::from_array(scene.bodies[n].motion.position(time, scene.array.instrument.gravity))
}

/// The leaves' share of tower `tower`'s ΔΦ, radians, each leaf alone, the body held at `at`
/// (metres, the world's frame) and read out at `time`.
///
/// # Errors
/// When there is no such tower, or the most cube evaluations the leaves' reads may make (4 a
/// node, up to [`LEAF_REFINEMENT`] times the flight's nodes a leaf) are more than the body's share
/// of [`MAX_KERNEL_WORK`] ([`crate::read::kernel_share`]), with the numbers.
pub fn leaf_phases(
    scene: &SceneSpec,
    tree: &VoxTree,
    at: Vec3,
    tower: usize,
    time: f64,
) -> Result<Vec<f64>, String> {
    let base =
        scene.array.towers.get(tower).ok_or_else(|| format!("there is no tower {}", tower + 1))?.position;
    // The tower's own instrument: a tower may override the array's (TASK-S3-08).
    let reader = Reader::new(scene.array.instrument_of(tower).instrument());
    let nodes = reader.instrument().node_count(&[]);
    // A leaf's read may refine to this many flight nodes, each 4 cube evaluations (two points for
    // each of two interferometers): the ceiling counted below is the work it may really do.
    let most_nodes = LEAF_REFINEMENT * nodes;
    // The body's share of one read's work: a frame colours every voxel body.
    let (work, share) = (4 * most_nodes * tree.leaves.len() as u64, crate::read::kernel_share(scene));
    if work > share {
        return Err(format!(
            "{} leaves × up to {most_nodes} flight nodes is more than the {share} cube evaluations allowed it \
             ({MAX_KERNEL_WORK} shared by the scene's voxel bodies)",
            tree.leaves.len()
        ));
    }
    Ok(tree.leaves.iter().map(|leaf| one_leaf(&reader, tree, leaf, at, base, time, most_nodes)).collect())
}

/// The leaf on its own, read with the rule of the instrument: one rule, and its check.
fn one_leaf(
    reader: &Reader,
    tree: &VoxTree,
    leaf: &cav_core::voxel::Leaf,
    at: Vec3,
    base: [f64; 3],
    time: f64,
    most_nodes: u64,
) -> f64 {
    let body = TreeBody::single(tree, leaf, |_| at);
    reader
        .read_to_within(Vec3::from_array(base), &body, time, PHASE_TOLERANCE, most_nodes)
        .reading
        .gradiometer(reader.instrument().sign)
}

/// One leaf's share of tower `tower`'s ΔΦ, radians, the body held at `at` (metres, the world's
/// frame) and read out at `time`: the number [`leaf_phases`] gives that leaf (the Phase
/// colouring's), for the Probe tool.
///
/// # Errors
/// When there is no such tower.
pub fn leaf_phase(
    scene: &SceneSpec,
    tree: &VoxTree,
    leaf: &cav_core::voxel::Leaf,
    at: Vec3,
    tower: usize,
    time: f64,
) -> Result<f64, String> {
    let base =
        scene.array.towers.get(tower).ok_or_else(|| format!("there is no tower {}", tower + 1))?.position;
    let reader = Reader::new(scene.array.instrument_of(tower).instrument());
    let most_nodes = LEAF_REFINEMENT * reader.instrument().node_count(&[]);
    Ok(one_leaf(&reader, tree, leaf, at, base, time, most_nodes))
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
    voxel_frame_drawn(scene, trees, &[], Look { colouring, overlay, cutting, draw: BodyDraw::Voxels }, time)
}

/// [`voxel_frame_cut`] in a draw mode ([`Look::draw`]), with `extras[n]` what body `n`'s draw mode
/// needs beyond its tree (a shorter slice for bodies with none). In [`BodyDraw::Fill`] and
/// [`BodyDraw::Pieces`] the
/// leaves are coloured by their fill ([`LeafField::Fill`]), which is exact for a tree with cut
/// cells (a cut leaf weighs `ρ ×` its piece's volume, so its mass over a full cell's is the
/// piece's share of the cell). The surfaces of the bodies drawn together have at most
/// [`MAX_DRAWN_TRIANGLES`] triangles, and so have their pieces: a body whose surface or pieces
/// would pass it is drawn without them, and the frame says so. Pieces are drawn only when every
/// leaf is (the cut at level 0) and with no section (which does not cut them); the frame says so
/// otherwise.
#[must_use]
pub fn voxel_frame_drawn(
    scene: &SceneSpec,
    trees: &[Option<Arc<VoxTree>>],
    extras: &[Extras],
    look: Look,
    time: f64,
) -> VoxelFrame {
    let Look { overlay, cutting, draw, .. } = look;
    let colouring =
        if draw.shades_by_fill() { Colouring { field: LeafField::Fill, tower: 0 } } else { look.colouring };
    let mut frame = VoxelFrame { colouring, section: cutting, draw, ..VoxelFrame::default() };
    let mut triangles = 0usize;
    let mut piece_triangles = 0usize;
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
    if towers.is_empty() && colouring.field == LeafField::Distance {
        frame.notes.push(
            "the scene has no towers: no leaf has a distance to the nearest, so the legend is empty".into(),
        );
    }
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
        // The body's materials' colours, by a leaf's material, for colour by material.
        let palette: Vec<(String, [u8; 3])> = if colouring.field == LeafField::Material {
            let found = scene.bodies[n].shape.as_ref().map(|s| s.materials(&scene.materials, "")).transpose();
            match found {
                Ok(Some(m)) if !m.is_empty() => m.iter().map(|m| (m.label.clone(), m.rgb())).collect(),
                _ => vec![("uniform".to_owned(), UNIFORM_COLOUR)],
            }
        } else {
            Vec::new()
        };
        for entry in &palette {
            if !frame.materials.iter().any(|(label, _)| *label == entry.0) {
                frame.materials.push(entry.clone());
            }
        }
        let mut cubes = Vec::new();
        let mut cells = Vec::new();
        let label = &scene.bodies[n].label;
        let pieces = if draw.draws_pieces() {
            let given = extras.get(n).and_then(|e| e.pieces.clone());
            match given {
                _ if tree.cuts.is_empty() => None,
                None => {
                    frame.notes.push(format!("`{label}` has no pieces to draw: it is shaded by its fill"));
                    None
                }
                Some(_) if level > 0 || cutting.is_some() => {
                    frame.notes.push(format!(
                        "`{label}`'s pieces are drawn only with every leaf and no section: it is shaded by its fill"
                    ));
                    None
                }
                Some(p) => {
                    let more: usize = p.iter().map(|s| s.triangles.len()).sum();
                    if piece_triangles + more > MAX_DRAWN_TRIANGLES {
                        frame.notes.push(format!(
                            "`{label}`'s pieces are not drawn: their {more} triangles would take the view past the                              {MAX_DRAWN_TRIANGLES} it draws"
                        ));
                        None
                    } else {
                        piece_triangles += more;
                        Some(p)
                    }
                }
            }
        } else {
            None
        };
        let mut drawn_pieces = Vec::new();
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
            // A block's material: its first leaf's (a node's leaves are a run in Morton order).
            let material = tree.leaves[leaves.start].material;
            let value = match colouring.field {
                LeafField::Level => f64::from(lvl),
                LeafField::Depth => f64::from(tree.grid.depth.saturating_sub(lvl)),
                LeafField::Mass => mass,
                // A body of materials weighs its points by their densities: its fill is counted
                // in sample points (a uniform body's is the same number, kept as it was made).
                LeafField::Fill if !tree.units.is_empty() => {
                    tree.leaves[leaves.clone()].iter().map(|l| l.samples() as f64).sum::<f64>()
                        / (k3 * f64::from(1u32 << (3 * u32::from(lvl))))
                }
                LeafField::Fill => mass / full,
                LeafField::Material => f64::from(material),
                LeafField::Height => centre.z,
                LeafField::Distance => towers
                    .iter()
                    .map(|t| (centre.x - t.x).hypot(centre.y - t.y))
                    .fold(f64::INFINITY, f64::min),
                LeafField::Phase => phases.as_ref().map_or(0.0, |p| p[leaves].iter().sum()),
            };
            let colour = palette.get(usize::from(material)).or(palette.first()).map(|(_, rgb)| *rgb);
            // A cut cell whose piece is drawn is not a cube.
            if let (Some(p), CutEntry::Leaf(_)) = (&pieces, entry)
                && let Ok(i) = p.binary_search_by_key(&key, |s| s.key)
                && lvl == 0
            {
                drawn_pieces.push((i, value));
                continue;
            }
            match placed {
                Some(Some(cell)) if cell != whole => {
                    cells.push(Cell { lo: cell.0, hi: cell.1, value, level: lvl, colour });
                }
                _ => cubes.push(Cube { centre, side, value, level: lvl, colour }),
            }
        }
        if overlay.centroids && level == 0 {
            for cut in &tree.cuts {
                if frame.centroids.len() >= MAX_OVERLAY_BOXES {
                    break;
                }
                let c = at + cut.piece.centroid;
                if cutting.is_none_or(|k| k.place(c, c).is_some()) {
                    frame.centroids.push((c, tree.grid.pitch));
                }
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
        let surface = if draw.draws_surface() {
            match extras.get(n).and_then(|e| e.surface.clone()) {
                Some(s) if triangles + s.triangles.len() > MAX_DRAWN_TRIANGLES => {
                    frame.notes.push(format!(
                        "`{}` is drawn without its surface: its {} triangles would take the view past the \
                         {MAX_DRAWN_TRIANGLES} it draws",
                        scene.bodies[n].label,
                        s.triangles.len()
                    ));
                    None
                }
                Some(s) => {
                    triangles += s.triangles.len();
                    if s.tessellated {
                        frame.notes.push(format!(
                            "`{}` is a curved solid: its surface is drawn tessellated",
                            scene.bodies[n].label
                        ));
                    }
                    Some(s)
                }
                None => None,
            }
        } else {
            None
        };
        let pieces = pieces.map(|surfaces| DrawnPieces { surfaces, drawn: drawn_pieces });
        frame.bodies.push(BodyCubes { body: n, at, surface, pieces, cubes, cells, cut: level });
    }
    if frame.boxes.len() >= MAX_OVERLAY_BOXES {
        frame.notes.push(format!("the overlay stops at {MAX_OVERLAY_BOXES} boxes"));
    }
    if frame.centroids.len() >= MAX_OVERLAY_BOXES {
        frame.notes.push(format!("the centroids overlay stops at {MAX_OVERLAY_BOXES}"));
    }
    if overlay.tree.is_some_and(|t| t.alone) {
        for body in &mut frame.bodies {
            body.cubes.clear();
        }
    }
    let values = frame
        .bodies
        .iter()
        .flat_map(|b| {
            let pieces = b.pieces.iter().flat_map(|p| p.drawn.iter().map(|(_, v)| *v));
            b.cubes.iter().map(|c| c.value).chain(b.cells.iter().map(|c| c.value)).chain(pieces)
        })
        .filter(|v| v.is_finite());
    frame.range =
        values.fold(None, |r: Option<(f64, f64)>, v| Some(r.map_or((v, v), |(a, b)| (a.min(v), b.max(v)))));
    if colouring.field.diverging() {
        frame.range = frame.range.map(|(a, b)| {
            let m = a.abs().max(b.abs());
            (-m, m)
        });
    }
    // Drawn by their surfaces alone, the voxels' field has nothing to show: no legend.
    if draw == BodyDraw::Mesh {
        frame.range = None;
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
/// [`MAX_TREE_BOXES`] (and the frame says so). Every depth's blocks are counted before any box is
/// made, so no more than the cap is ever made; a first asked depth that alone is past it draws
/// nothing and says so.
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
    // Whole depths, shallowest first, while they fit; nothing is made past the budget.
    let mut deepest = None;
    let mut total = 0;
    let mut refused = None;
    for depth in (0..=32u8).filter(|&d| asked.holds(d) && counts[usize::from(d)] > 0) {
        let more = counts[usize::from(depth)];
        if total + more > MAX_TREE_BOXES {
            match deepest {
                Some(_) => frame.tree_capped_at = deepest,
                // The first depth alone is past the budget: which part of it to draw would be an
                // arbitrary choice, so none is, and the frame says which depth would fit.
                None => refused = Some((depth, more)),
            }
            break;
        }
        total += more;
        deepest = Some(depth);
    }
    let Some(deepest) = deepest else {
        frame.notes.push(match refused {
            Some((depth, more)) => {
                let fits = (0..depth).rev().find(|&d| counts[usize::from(d)] <= MAX_TREE_BOXES);
                format!(
                    "depth {depth} alone has {more} tree nodes, more than the {MAX_TREE_BOXES} drawn at most: the \
                     tree overlay is not drawn{}",
                    fits.map_or_else(String::new, |d| format!("; depth {d} and shallower fit one at a time"))
                )
            }
            None => format!(
                "the tree overlay has no node at depths {}{}",
                asked.from,
                asked.to.map_or_else(|| " and deeper".to_owned(), |to| format!(" to {to}"))
            ),
        });
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
    debug_assert!(
        frame.tree.len() == total && total <= MAX_TREE_BOXES,
        "counted {total}, made {}",
        frame.tree.len()
    );
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
