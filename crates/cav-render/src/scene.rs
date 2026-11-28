//! From a scene spec to things to draw, on the CPU: thick line segments, triangles and an optional
//! infinite ground.
//!
//! Positions are formed in `f64` as offsets from the view's target and only then rounded to `f32`
//! ([`local`]), so the GPU never sees a world coordinate (REBUILD §3.4).

use std::f64::consts::TAU;

use bytemuck::{Pod, Zeroable};
use cav_core::Vec3;
use cav_sim::{DrawStyle, SceneSpec, Target};

/// One thick line segment, drawn as a screen-facing quad. `repr(C)`: this is the instance layout.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct Line {
    /// Start: `[x, y, z]` (east, north, up), metres from the view's target.
    pub a: [f32; 3],
    /// Width, pixels.
    pub width: f32,
    /// End: `[x, y, z]` (east, north, up), metres from the view's target.
    pub b: [f32; 3],
    /// Padding that keeps `colour` on a 16-byte boundary, as WGSL lays the instance out. Always
    /// zero; it means nothing.
    pub _pad: f32,
    /// `[red, green, blue, alpha]`, each 0 to 1: linear light (not sRGB-encoded), alpha not
    /// premultiplied.
    pub colour: [f32; 4],
}

/// One triangle corner. `repr(C)`: this is the vertex layout.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct Vertex {
    /// `[x, y, z]` (east, north, up), metres from the view's target.
    pub position: [f32; 3],
    /// The unit vector out of the surface at this corner, `[x, y, z]` in the world's axes.
    pub normal: [f32; 3],
    /// `[red, green, blue, alpha]`, each 0 to 1: linear light (not sRGB-encoded).
    pub colour: [f32; 4],
}

/// What a run of lines and triangles belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// Tower `n`, from 0.
    Tower(usize),
    /// The room: its floor, its grid and its edges.
    Room,
    /// Body `n`, from 0.
    Body(usize),
    /// The overlays drawn over voxel bodies: leaf bounds, a cut.
    Overlay,
    /// The tree overlay: every node of the voxel bodies' trees, by depth.
    Tree,
    /// The world's base when it is a planet: its curved grid.
    Base,
}

/// An infinite horizontal floor, drawn as a grid that fades towards the horizon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    /// Height of the surface relative to the view's target, metres.
    pub height: f32,
    /// `[x, y]` (east, north): the target's world x and y modulo the 100 m grid period, metres, so
    /// the grid stays put as the camera moves without the shader ever seeing a large coordinate.
    pub phase: [f32; 2],
    /// The surface's colour: `[red, green, blue, alpha]`, linear light, each 0 to 1.
    pub surface: [f32; 4],
    /// The grid lines' colour: `[red, green, blue, alpha]`, linear light, each 0 to 1.
    pub line: [f32; 4],
}

/// Everything to draw for one view.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawables {
    /// Every line segment, part by part in the order of `parts`.
    pub lines: Vec<Line>,
    /// Every triangle corner, three to a triangle, part by part in the order of `parts`.
    pub triangles: Vec<Vertex>,
    /// The infinite floor, when the scene has one.
    pub ground: Option<Ground>,
    /// The background colour: `[red, green, blue]`, linear light, each 0 to 1.
    pub background: [f32; 3],
    /// `(part, lines end, corners end)`: each part with the index in `lines` and the index in
    /// `triangles` just past its last entry (a part's entries start where the previous part's
    /// ended, the first part's at 0).
    pub parts: Vec<(Part, usize, usize)>,
}

/// The colours of one draw style, as sRGB hex.
struct Palette {
    background: u32,
    towers: [u32; 5],
    room_edge: u32,
    grid: u32,
    grid_axis: u32,
    floor: u32,
    plate: u32,
    selection: u32,
    ground: u32,
    ground_line: u32,
    body: u32,
    trail: u32,
}

/// The mock-up's palette: near-black ground, five tower colours.
const DARK: Palette = Palette {
    background: 0x0a_0a0a,
    towers: [0xd6_a83c, 0xc4_677a, 0x4b_93a0, 0xb0_4a2c, 0x6d_7e35],
    room_edge: 0x5a_5a5a,
    grid: 0x24_2424,
    grid_axis: 0x3c_3c3c,
    floor: 0x12_1212,
    plate: 0x3c_3c3c,
    selection: 0x5a_b4ff,
    ground: 0x10_1010,
    ground_line: 0x38_3838,
    body: 0xe8_e2d0,
    trail: 0x6a_6658,
};

/// Ink on cream paper.
const PAPER: Palette = Palette {
    background: 0xf1_ede4,
    towers: [0x2b_2620; 5],
    room_edge: 0x2b_2620,
    grid: 0xd6_cfc0,
    grid_axis: 0xb8_ae9a,
    floor: 0xec_e7dc,
    plate: 0x9b_8f7c,
    selection: 0xb0_4a2c,
    ground: 0xec_e7dc,
    ground_line: 0x9b_8f7c,
    body: 0x2b_2620,
    trail: 0x9b_8f7c,
};

/// The background of the dark styles, `[red, green, blue]` as sRGB bytes: one of the colours the
/// tests look for in a picture.
pub const BACKGROUND_SRGB: [u8; 3] = [0x0a, 0x0a, 0x0a];
/// The background of the paper style, `[red, green, blue]` as sRGB bytes.
pub const PAPER_SRGB: [u8; 3] = [0xf1, 0xed, 0xe4];

/// A body's colour in the dark styles, `[red, green, blue]` as sRGB bytes.
pub const BODY_SRGB: [u8; 3] = [0xe8, 0xe2, 0xd0];

/// The smallest radius a body is drawn at, metres: a point mass has none of its own.
pub const BODY_RADIUS: f64 = 0.14;

/// Tower `n`'s colour (from 0; the five colours repeat) in the dark styles, `[red, green, blue]`
/// as sRGB bytes.
#[must_use]
pub fn tower_srgb(n: usize) -> [u8; 3] {
    let hex = DARK.towers[n % DARK.towers.len()];
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8]
}

/// An sRGB hex colour as linear RGB with the given alpha.
fn linear(hex: u32, alpha: f32) -> [f32; 4] {
    let channel = |shift: u32| {
        let c = f32::from(((hex >> shift) & 0xff) as u8) / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    [channel(16), channel(8), channel(0), alpha]
}

/// The colour a fraction `t` of the way from `a` to `b`.
fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

/// The radius the launch points are drawn at, metres: far larger than an atom cloud, so that they
/// can be seen from across the room.
pub const CLOUD_RADIUS: f64 = 0.16;

/// The most grid lines across a room, and the most ticks up a tower.
pub const MAX_MARKS: u32 = 200;

/// The spacing of marks along `extent` metres: one metre, or the smallest power of ten of metres
/// that needs no more than [`MAX_MARKS`] of them.
#[must_use]
pub fn mark_step(extent: f64) -> f64 {
    let mut step = 1.0;
    while extent / step > f64::from(MAX_MARKS) && step < 1.0e12 {
        step *= 10.0;
    }
    step
}

/// The marks at whole multiples of `step` from `lo` to `hi`: `(index, position)`, the index being
/// the multiple, so index 0 is the mark at the origin. Never more than `MAX_MARKS + 1` of them,
/// whatever the numbers, and none at all if they are not finite.
fn marks(lo: f64, hi: f64, step: f64) -> impl Iterator<Item = (i64, f64)> {
    let first = (lo / step).ceil();
    let last = (hi / step).floor();
    let count = if first.is_finite() && last.is_finite() && last >= first {
        ((last - first) as u32).min(MAX_MARKS) + 1
    } else {
        0
    };
    (0..count).map(move |k| {
        let index = first + f64::from(k);
        (index as i64, index * step)
    })
}

/// What is drawn coarser than a metre for `scene`, in words; empty when nothing is. The renderer
/// thins grid lines and ticks rather than drawing thousands; this says so, for a badge or a log.
#[must_use]
pub fn decimation(scene: &SceneSpec) -> Vec<String> {
    let mut notes = Vec::new();
    if let Some(room) = &scene.room {
        let step = mark_step(room.size[0].max(room.size[1]));
        if step > 1.0 {
            notes.push(format!("room grid every {step} m"));
        }
    }
    // The tallest tower's ticks are the coarsest (towers may have heights of their own).
    let tallest = scene.array.instruments().iter().map(|i| i.tower_height).fold(0.0, f64::max);
    let step = mark_step(tallest);
    if step > 1.0 {
        notes.push(format!("tower ticks every {step} m"));
    }
    // A body's path that met its cap of pieces is drawn coarser than its bound (`crate::path`).
    let pieces = crate::path::allowance(scene);
    match crate::path::capped(scene).as_slice() {
        [] => {}
        [one] => notes.push(format!("the path of `{one}` is drawn coarsely: {pieces} pieces, its cap")),
        many => notes.push(format!(
            "the paths of {} bodies are drawn coarsely: {pieces} pieces each, their cap",
            many.len()
        )),
    }
    notes
}

/// A world position as an `f32` offset from `origin`: `[x, y, z]` (east, north, up), metres.
/// Both arguments are in the world's frame. The subtraction is in `f64`; only the small result
/// is rounded.
#[must_use]
pub fn local(world: Vec3, origin: Vec3) -> [f32; 3] {
    (world - origin).to_f32()
}

struct Builder {
    origin: Vec3,
    out: Drawables,
}

impl Builder {
    fn line(&mut self, a: Vec3, b: Vec3, width: f32, colour: [f32; 4]) {
        self.out.lines.push(Line {
            a: local(a, self.origin),
            width,
            b: local(b, self.origin),
            _pad: 0.0,
            colour,
        });
    }

    fn triangle(&mut self, corners: [Vec3; 3], normals: [Vec3; 3], colour: [f32; 4]) {
        for (corner, normal) in corners.into_iter().zip(normals) {
            self.out.triangles.push(Vertex {
                position: local(corner, self.origin),
                normal: normal.to_f32(),
                colour,
            });
        }
    }

    /// A flat quad with corners in order around it.
    fn quad(&mut self, c: [Vec3; 4], colour: [f32; 4]) {
        let normal = (c[1] - c[0]).cross(c[3] - c[0]).normalised();
        self.triangle([c[0], c[1], c[2]], [normal; 3], colour);
        self.triangle([c[0], c[2], c[3]], [normal; 3], colour);
    }

    /// An axis-aligned box.
    fn cuboid(&mut self, lo: Vec3, hi: Vec3, colour: [f32; 4]) {
        let p = |x: bool, y: bool, z: bool| {
            Vec3::new(if x { hi.x } else { lo.x }, if y { hi.y } else { lo.y }, if z { hi.z } else { lo.z })
        };
        let (f, t) = (false, true);
        self.quad([p(f, f, t), p(t, f, t), p(t, t, t), p(f, t, t)], colour); // top
        self.quad([p(f, t, f), p(t, t, f), p(t, f, f), p(f, f, f)], colour); // bottom
        self.quad([p(f, f, f), p(t, f, f), p(t, f, t), p(f, f, t)], colour); // south
        self.quad([p(t, t, f), p(f, t, f), p(f, t, t), p(t, t, t)], colour); // north
        self.quad([p(t, f, f), p(t, t, f), p(t, t, t), p(t, f, t)], colour); // east
        self.quad([p(f, t, f), p(f, f, f), p(f, f, t), p(f, t, t)], colour); // west
    }

    /// The twelve edges of an axis-aligned box, as lines of `width` pixels.
    fn box_edges(&mut self, lo: Vec3, hi: Vec3, width: f32, colour: [f32; 4]) {
        let corner = |i: u32| {
            Vec3::new(
                if i & 1 == 0 { lo.x } else { hi.x },
                if i & 2 == 0 { lo.y } else { hi.y },
                if i & 4 == 0 { lo.z } else { hi.z },
            )
        };
        for i in 0..8u32 {
            for bit in [1, 2, 4] {
                if i & bit == 0 {
                    self.line(corner(i), corner(i | bit), width, colour);
                }
            }
        }
    }

    /// A vertical column with `SIDES` faces, smooth-shaded, capped at the top.
    fn column(&mut self, base: Vec3, height: f64, radius: f64, colour: [f32; 4]) {
        const SIDES: u32 = 12;
        let around = |k: u32| {
            let angle = TAU * f64::from(k) / f64::from(SIDES);
            Vec3::new(angle.cos(), angle.sin(), 0.0)
        };
        let top = base + Vec3::Z * height;
        for k in 0..SIDES {
            let (n0, n1) = (around(k), around(k + 1));
            let (a, b) = (base + n0 * radius, base + n1 * radius);
            let (c, d) = (top + n1 * radius, top + n0 * radius);
            self.triangle([a, b, c], [n0, n1, n1], colour);
            self.triangle([a, c, d], [n0, n1, n0], colour);
            self.triangle([top, d, c], [Vec3::Z; 3], colour);
        }
    }

    /// A sphere: an octahedron subdivided twice, smooth-shaded.
    fn sphere(&mut self, centre: Vec3, radius: f64, colour: [f32; 4]) {
        fn split(a: Vec3, b: Vec3, c: Vec3, depth: u32, out: &mut Vec<[Vec3; 3]>) {
            if depth == 0 {
                out.push([a, b, c]);
                return;
            }
            let (ab, bc, ca) = ((a + b).normalised(), (b + c).normalised(), (c + a).normalised());
            split(a, ab, ca, depth - 1, out);
            split(ab, b, bc, depth - 1, out);
            split(ca, bc, c, depth - 1, out);
            split(ab, bc, ca, depth - 1, out);
        }
        let mut faces = Vec::new();
        for z in [Vec3::Z, -Vec3::Z] {
            for k in 0..4 {
                let (x, y) =
                    ([Vec3::X, Vec3::Y, -Vec3::X, -Vec3::Y][k], [Vec3::Y, -Vec3::X, -Vec3::Y, Vec3::X][k]);
                // Keep the winding outward-facing on both hemispheres.
                if z.z > 0.0 { split(x, y, z, 2, &mut faces) } else { split(y, x, z, 2, &mut faces) }
            }
        }
        for face in faces {
            self.triangle(face.map(|n| centre + n * radius), face, colour);
        }
    }

    fn end_part(&mut self, part: Part) {
        self.out.parts.push((part, self.out.lines.len(), self.out.triangles.len()));
    }
}

/// Builds what to draw for `scene` at time zero; see [`drawables_at`].
#[must_use]
pub fn drawables(scene: &SceneSpec, style: DrawStyle, highlight: Option<Target>, origin: Vec3) -> Drawables {
    drawables_at(scene, style, highlight, origin, 0.0)
}

/// Builds what to draw for `scene` in `style`, relative to `origin` (the view's target, metres,
/// the world's frame), with `highlight` drawn as the selection and every body where it is at
/// `time` seconds on the scene's clock.
#[must_use]
pub fn drawables_at(
    scene: &SceneSpec,
    style: DrawStyle,
    highlight: Option<Target>,
    origin: Vec3,
    time: f64,
) -> Drawables {
    drawables_with(scene, style, highlight, origin, time, None)
}

/// The colours of the overlay boxes, by tree level (repeating): the dark styles' tower colours.
const LEVEL_COLOURS: [u32; 5] = DARK.towers;

/// A flat section cell (`lo` and `hi` equal along one axis) drawn a little inside its square,
/// [`SECTION_GAP`] of its side from each edge, so that neighbouring cells of one colour are told
/// apart and the quadtree's outlines over them show on the gap; any other box as it is.
fn inset(lo: Vec3, hi: Vec3) -> (Vec3, Vec3) {
    let d = hi - lo;
    let side = d.x.max(d.y).max(d.z);
    if d.x.min(d.y).min(d.z) > 0.0 {
        return (lo, hi);
    }
    let g = SECTION_GAP * side;
    let step = Vec3::new(
        if d.x > 0.0 { g } else { 0.0 },
        if d.y > 0.0 { g } else { 0.0 },
        if d.z > 0.0 { g } else { 0.0 },
    );
    (lo + step, hi - step)
}

/// The gap round a flat section cell, as a fraction of its side.
const SECTION_GAP: f64 = 0.04;

/// How much of a tree node's colour its faces add in the x-ray style, times its opacity: nested
/// faces add up, so each is faint.
const TREE_FACE: f32 = 0.08;

/// The colour of a tree node at `depth` with the depths drawn `range`, linear light, opaque: the
/// sequential map ([`crate::colour::sequential`]) from the shallowest to the deepest, as
/// `colour depth` colours a leaf of that depth. `dark` for a dark surface.
#[must_use]
pub fn tree_colour(depth: u8, range: (u8, u8), dark: bool) -> [f32; 4] {
    let rgb =
        crate::colour::of_value(f64::from(depth), (f64::from(range.0), f64::from(range.1)), false, dark);
    linear((u32::from(rgb[0]) << 16) | (u32::from(rgb[1]) << 8) | u32::from(rgb[2]), 1.0)
}

/// [`drawables_at`], with the voxel bodies of `voxels` drawn as their cubes (coloured by the
/// frame's field over the range it says) and its overlay boxes as lines. A voxel body the frame
/// does not hold is drawn as any body is.
#[must_use]
pub fn drawables_with(
    scene: &SceneSpec,
    style: DrawStyle,
    highlight: Option<Target>,
    origin: Vec3,
    time: f64,
    voxels: Option<&cav_sim::voxel_view::VoxelFrame>,
) -> Drawables {
    drawables_hiding(scene, style, highlight, origin, time, voxels, &[])
}

/// [`drawables_with`] without the bodies whose labels are in `hidden` (a view's hidden bodies,
/// `ViewSpec::hidden`): they are not drawn at all, not even as an outline.
#[must_use]
pub fn drawables_hiding(
    scene: &SceneSpec,
    style: DrawStyle,
    highlight: Option<Target>,
    origin: Vec3,
    time: f64,
    voxels: Option<&cav_sim::voxel_view::VoxelFrame>,
    hidden: &[String],
) -> Drawables {
    let paper = style == DrawStyle::Paper;
    let palette = if paper { &PAPER } else { &DARK };
    // Wireframe and paper draw edges; the other styles also fill surfaces.
    let filled = !matches!(style, DrawStyle::Wireframe);
    let mut b = Builder { origin, out: Drawables::default() };
    let [r, g, bl, _] = linear(palette.background, 1.0);
    b.out.background = [r, g, bl];

    if let Some(room) = &scene.room {
        let lo = Vec3::from_array(room.origin);
        let size = Vec3::from_array(room.size);
        let hi = lo + size;
        let selected = highlight == Some(Target::Room);
        // The floor: a dark panel a hair below z so the grid lines on it win the depth test.
        if filled {
            let z = lo.z - 0.004;
            let colour = linear(palette.floor, 1.0);
            b.quad(
                [
                    Vec3::new(lo.x, lo.y, z),
                    Vec3::new(hi.x, lo.y, z),
                    Vec3::new(hi.x, hi.y, z),
                    Vec3::new(lo.x, hi.y, z),
                ],
                colour,
            );
        }
        // A grid on the floor (1 m unless the room is huge), the lines through the world axes stronger.
        let grid = linear(palette.grid, 1.0);
        let axis = linear(palette.grid_axis, 1.0);
        // Counted by whole index, never by adding to a float: a loop that adds 1.0 to 1e17 never ends.
        let step = mark_step(size.x.max(size.y));
        for (k, x) in marks(lo.x, hi.x, step) {
            b.line(Vec3::new(x, lo.y, lo.z), Vec3::new(x, hi.y, lo.z), 1.0, if k == 0 { axis } else { grid });
        }
        for (k, y) in marks(lo.y, hi.y, step) {
            b.line(Vec3::new(lo.x, y, lo.z), Vec3::new(hi.x, y, lo.z), 1.0, if k == 0 { axis } else { grid });
        }
        // The twelve edges of the box.
        let edge = linear(if selected { palette.selection } else { palette.room_edge }, 1.0);
        let width = if selected { 2.5 } else { 1.5 };
        let corner = |i: u32| {
            Vec3::new(
                if i & 1 == 0 { lo.x } else { hi.x },
                if i & 2 == 0 { lo.y } else { hi.y },
                if i & 4 == 0 { lo.z } else { hi.z },
            )
        };
        for i in 0..8u32 {
            for bit in [1, 2, 4] {
                if i & bit == 0 {
                    b.line(corner(i), corner(i | bit), width, edge);
                }
            }
        }
        b.end_part(Part::Room);
    }

    for (n, tower) in scene.array.towers.iter().enumerate() {
        // Each tower at its own height, with its own interferometers' launch points.
        let instrument = &scene.array.instrument_of(n);
        let base = Vec3::from_array(tower.position);
        let top = base + Vec3::Z * instrument.tower_height;
        let selected = highlight == Some(Target::Tower(n + 1));
        let colour = linear(palette.towers[n % palette.towers.len()], 1.0);
        // The baseline, with a tick every metre (coarser on a very tall tower).
        b.line(base, top, if selected { 4.0 } else { 2.5 }, colour);
        let step = mark_step(instrument.tower_height);
        for (k, height) in marks(0.0, instrument.tower_height, step) {
            let at = base + Vec3::Z * height;
            let half = if k % 5 == 0 { 0.16 } else { 0.09 };
            b.line(at - Vec3::X * half, at + Vec3::X * half, 1.5, colour);
            b.line(at - Vec3::Y * half, at + Vec3::Y * half, 1.5, colour);
        }
        if filled {
            b.cuboid(
                base + Vec3::new(-0.3, -0.3, -0.003),
                base + Vec3::new(0.3, 0.3, 0.03),
                linear(palette.plate, 1.0),
            );
            b.column(base, instrument.tower_height, 0.035, colour);
            // The atom clouds at rest: one at each interferometer's launch point.
            for height in [0.0, instrument.ifo_separation] {
                b.sphere(base + Vec3::Z * height, CLOUD_RADIUS, colour);
            }
        }
        if selected {
            let ring = linear(palette.selection, 1.0);
            const STEPS: u32 = 48;
            for k in 0..STEPS {
                let at = |k: u32| {
                    let angle = TAU * f64::from(k) / f64::from(STEPS);
                    base + Vec3::new(0.55 * angle.cos(), 0.55 * angle.sin(), 0.01)
                };
                b.line(at(k), at(k + 1), 2.0, ring);
            }
        }
        b.end_part(Part::Tower(n));
    }

    let gravity = scene.array.instrument.gravity;
    let path_pieces = crate::path::allowance(scene);
    for (n, body) in scene.bodies.iter().enumerate() {
        if hidden.contains(&body.label) {
            continue;
        }
        let selected = highlight == Some(Target::Body(n + 1));
        let at = Vec3::from_array(body.motion.position(time, gravity));
        let radius = body.radius.max(BODY_RADIUS);
        let (from, until) = body.exists();
        let colour = linear(if selected { palette.selection } else { palette.body }, 1.0);
        // Its path, if it has one: cut by its curvature while the body exists, a repeating
        // motion over one period, within a cap of pieces (`crate::path`; S20: it was 96 pieces
        // across the timeline, which drew a quick circle as a square).
        if crate::path::has_path(body, gravity) {
            let trail = linear(palette.trail, 1.0);
            let path = crate::path::path(body, gravity, scene.duration, path_pieces);
            for piece in path.knots.windows(2) {
                b.line(piece[0].at, piece[1].at, 1.0, trail);
            }
        }
        // A body that does not exist at this moment is an outline of where it will be.
        let present = time >= from && time < until;
        let cubes = voxels.and_then(|v| v.bodies.iter().find(|c| c.body == n).map(|c| (v, c)));
        if let (Some((frame, cubes)), true) = (cubes, present) {
            let dark = !paper;
            let diverging = frame.colouring.field.diverging();
            let range = frame.range.unwrap_or((0.0, 1.0));
            for cube in &cubes.cubes {
                let rgb = cube
                    .colour
                    .unwrap_or_else(|| crate::colour::of_value(cube.value, range, diverging, dark));
                let hex = (u32::from(rgb[0]) << 16) | (u32::from(rgb[1]) << 8) | u32::from(rgb[2]);
                let h = 0.5 * cube.side;
                let d = Vec3::new(h, h, h);
                if filled {
                    b.cuboid(cube.centre - d, cube.centre + d, linear(hex, 1.0));
                } else {
                    b.box_edges(cube.centre - d, cube.centre + d, 1.0, linear(hex, 1.0));
                }
            }
            // What a section plane cuts: clipped blocks, or flat squares on a 2-D section, in the
            // field's colours like the cubes.
            for cell in &cubes.cells {
                let rgb = cell
                    .colour
                    .unwrap_or_else(|| crate::colour::of_value(cell.value, range, diverging, dark));
                let hex = (u32::from(rgb[0]) << 16) | (u32::from(rgb[1]) << 8) | u32::from(rgb[2]);
                let (lo, hi) = inset(cell.lo, cell.hi);
                if filled {
                    b.cuboid(lo, hi, linear(hex, 1.0));
                } else {
                    b.box_edges(lo, hi, 1.0, linear(hex, 1.0));
                }
            }
            if selected {
                let (lo, hi) = cubes.cubes.iter().fold((at, at), |(lo, hi), c| {
                    let h = 0.5 * c.side;
                    (
                        Vec3::new(
                            lo.x.min(c.centre.x - h),
                            lo.y.min(c.centre.y - h),
                            lo.z.min(c.centre.z - h),
                        ),
                        Vec3::new(
                            hi.x.max(c.centre.x + h),
                            hi.y.max(c.centre.y + h),
                            hi.z.max(c.centre.z + h),
                        ),
                    )
                });
                b.box_edges(lo, hi, 2.0, linear(palette.selection, 1.0));
            }
            b.end_part(Part::Body(n));
            continue;
        }
        if filled && present {
            b.sphere(at, radius, colour);
        }
        if !filled || !present || selected {
            const STEPS: u32 = 32;
            let ring = |axis: usize, k: u32| {
                let (sin, cos) = (TAU * f64::from(k) / f64::from(STEPS)).sin_cos();
                let reach = radius * if selected && filled && present { 1.6 } else { 1.0 };
                at + match axis {
                    0 => Vec3::new(cos, sin, 0.0),
                    1 => Vec3::new(cos, 0.0, sin),
                    _ => Vec3::new(0.0, cos, sin),
                } * reach
            };
            for axis in 0..3 {
                for k in 0..STEPS {
                    b.line(ring(axis, k), ring(axis, k + 1), if selected { 2.0 } else { 1.2 }, colour);
                }
            }
        }
        b.end_part(Part::Body(n));
    }

    // The overlays over voxel bodies: each box outlined in its level's colour.
    if let Some(frame) = voxels.filter(|v| !v.boxes.is_empty()) {
        for overlay in &frame.boxes {
            let colour = linear(LEVEL_COLOURS[usize::from(overlay.level) % LEVEL_COLOURS.len()], 1.0);
            b.box_edges(overlay.lo, overlay.hi, 1.0, colour);
        }
        b.end_part(Part::Overlay);
    }

    // The whole tree: every node's box in the colour of its depth, on the sequential map over the
    // depths drawn, and as opaque as its depth's fade says.
    if let (Some(frame), Some(asked)) = (voxels, voxels.and_then(|v| v.tree_overlay)) {
        let xray = style == DrawStyle::XRay;
        let range = frame.depths.unwrap_or((0, 0));
        let background = linear(palette.background, 1.0);
        for node in &frame.tree {
            let alpha = cav_sim::voxel_view::opacity(node.depth, range, asked.fade) as f32;
            let colour = tree_colour(node.depth, range, !paper);
            // X-ray adds light scaled by alpha; the opaque styles have no blending, so a faint
            // line is drawn as that much of its colour over the background.
            let (edge, face) = if xray {
                (
                    [colour[0], colour[1], colour[2], alpha],
                    Some([colour[0], colour[1], colour[2], TREE_FACE * alpha]),
                )
            } else {
                (mix(background, colour, alpha), None)
            };
            b.box_edges(node.lo, node.hi, 1.0, edge);
            if let (true, Some(face)) = (asked.faces, face) {
                b.cuboid(node.lo, node.hi, face);
            }
        }
        b.end_part(Part::Tree);
    }

    if let Some(floor) = &scene.floor {
        let selected = highlight == Some(Target::Floor);
        let period = 100.0;
        b.out.ground = Some(Ground {
            height: (floor.height - origin.z) as f32,
            phase: [origin.x.rem_euclid(period) as f32, origin.y.rem_euclid(period) as f32],
            surface: linear(palette.ground, 1.0),
            // A selected floor is tinted, not lit up: its grid covers most of the picture.
            line: if selected {
                mix(linear(palette.ground_line, 1.0), linear(palette.selection, 1.0), 0.18)
            } else {
                linear(palette.ground_line, 1.0)
            },
        });
    }
    // The world's base (`world.base`, TASK-S5-13): the plane as the infinite floor is, a hair
    // below its surface so that what is cut flush into it (a ditch's top) wins the depth test;
    // the planet as a curved grid out to its equator. A scene's own floor, when it has one, is
    // the ground drawn.
    match scene.world.as_ref().map(|w| &w.base) {
        Some(cav_sim::world::BaseSpec::Plane { surface, .. }) if b.out.ground.is_none() => {
            let period = 100.0;
            b.out.ground = Some(Ground {
                height: (surface - origin.z - PLANE_BELOW) as f32,
                phase: [origin.x.rem_euclid(period) as f32, origin.y.rem_euclid(period) as f32],
                surface: linear(palette.ground, 1.0),
                line: linear(palette.ground_line, 1.0),
            });
        }
        Some(cav_sim::world::BaseSpec::Sphere { radius, .. }) => {
            planet_grid(&mut b, *radius, linear(palette.ground_line, 1.0));
        }
        _ => {}
    }
    b.out
}

/// How far below a plane base's surface its ground is drawn, metres.
pub const PLANE_BELOW: f64 = 0.004;
/// Segments a planet's grid draws each of its circles of latitude with.
pub const PLANET_RING_SEGMENTS: u32 = 96;
/// Meridians a planet's grid draws.
pub const PLANET_MERIDIANS: u32 = 24;

/// The arc distances from the pole, metres, of a planet's circles of latitude: 1 to 9 times each
/// power of ten from 10 m up to a quarter of its circumference (the equator), so the grid is fine
/// near the array and reaches the horizon in about forty circles.
#[must_use]
pub fn planet_rings(radius: f64) -> Vec<f64> {
    let quarter = 0.5 * std::f64::consts::PI * radius;
    let mut out = Vec::new();
    let mut decade = 10.0;
    while decade < quarter && out.len() < 200 {
        for k in 1..10 {
            let arc = decade * f64::from(k);
            if arc >= quarter {
                break;
            }
            out.push(arc);
        }
        decade *= 10.0;
    }
    out.push(quarter);
    out
}

/// The point of a planet of `radius` (centre `(0, 0, −radius)`) at arc `arc` metres from its pole
/// towards the azimuth `phi` radians (from east, anticlockwise), the world's frame. Its height
/// below the pole is formed as `2R sin²(θ/2)`, without the cancellation of `R − R cos θ`.
#[must_use]
pub fn on_planet(radius: f64, arc: f64, phi: f64) -> Vec3 {
    let theta = arc / radius;
    let (s, c) = phi.sin_cos();
    let across = radius * theta.sin();
    let half = (0.5 * theta).sin();
    Vec3::new(across * c, across * s, -2.0 * radius * half * half)
}

/// A planet's grid: its circles of latitude ([`planet_rings`]) and [`PLANET_MERIDIANS`]
/// meridians, as lines of `colour`.
fn planet_grid(b: &mut Builder, radius: f64, colour: [f32; 4]) {
    let rings = planet_rings(radius);
    let azimuth = |k: u32, n: u32| std::f64::consts::TAU * f64::from(k) / f64::from(n);
    for &arc in &rings {
        for k in 0..PLANET_RING_SEGMENTS {
            let (p, q) = (azimuth(k, PLANET_RING_SEGMENTS), azimuth(k + 1, PLANET_RING_SEGMENTS));
            b.line(on_planet(radius, arc, p), on_planet(radius, arc, q), 1.0, colour);
        }
    }
    for m in 0..PLANET_MERIDIANS {
        let phi = azimuth(m, PLANET_MERIDIANS);
        let mut from = 0.0;
        for &arc in &rings {
            // Each piece at most a twentieth of a radian of arc, so its chord stays on the sphere.
            let pieces = ((arc - from) / (0.05 * radius)).ceil().max(1.0) as u32;
            for p in 0..pieces {
                let at = |q: u32| from + (arc - from) * f64::from(q) / f64::from(pieces);
                b.line(on_planet(radius, at(p), phi), on_planet(radius, at(p + 1), phi), 1.0, colour);
            }
            from = arc;
        }
    }
    b.end_part(Part::Base);
}

#[cfg(test)]
mod tests {
    use super::*;
    use cav_sim::dev;

    fn towers(d: &Drawables) -> usize {
        d.parts.iter().filter(|(p, ..)| matches!(p, Part::Tower(_))).count()
    }

    fn has_room(d: &Drawables) -> bool {
        d.parts.iter().any(|(p, ..)| *p == Part::Room)
    }

    #[test]
    fn bodies_are_drawn_where_they_are_at_the_playhead() {
        let scene = dev::pass();
        let body = |d: &Drawables| {
            let n = d.parts.iter().position(|(p, ..)| *p == Part::Body(0)).expect("the body is a part");
            let (first, last) = (d.parts[n - 1].2, d.parts[n].2);
            d.triangles[first..last].to_vec()
        };
        // The centre of the body's sphere, from its triangles' corners.
        let centre = |d: &Drawables| {
            let corners = body(d);
            let sum = corners.iter().fold([0.0f64; 3], |s, v| {
                [
                    s[0] + f64::from(v.position[0]),
                    s[1] + f64::from(v.position[1]),
                    s[2] + f64::from(v.position[2]),
                ]
            });
            sum.map(|c| c / corners.len() as f64)
        };
        for time in [0.0, 4.0, 10.0] {
            let d = drawables_at(&scene, DrawStyle::Shaded, None, Vec3::ZERO, time);
            let [x, y, z] = centre(&d);
            let expected = [10.0, 0.0, -10.0 + 2.5 * time];
            assert!(
                (x - expected[0]).abs() < 1e-3 && y.abs() < 1e-3 && (z - expected[2]).abs() < 1e-3,
                "{time}: {x} {y} {z}"
            );
            assert_eq!(body(&d).len(), 8 * 16 * 3, "a sphere of 128 faces");
        }
        // Its path is drawn too: the void's lines, and the pass's one. (Rewritten in S20,
        // TASK-S20-08: this expected 96 lines, the pieces the old rule cut every path into
        // across the timeline. A path is now cut by its curvature, and a straight pass from its
        // start to its stop is one straight line.)
        let lines =
            |scene: &SceneSpec| drawables_at(scene, DrawStyle::Shaded, None, Vec3::ZERO, 0.0).lines.len();
        assert_eq!(lines(&scene) - lines(&dev::void()), 1);
        // A mass at rest has no trail; selected, it has rings round it and the selection's colour.
        let point = dev::point();
        assert_eq!(lines(&point), lines(&SceneSpec::room()));
        let selected = drawables_at(&point, DrawStyle::Shaded, Some(Target::Body(1)), Vec3::ZERO, 0.0);
        assert_eq!(selected.lines.len() - lines(&point), 3 * 32);
        assert_ne!(
            body(&selected)[0].colour,
            body(&drawables(&point, DrawStyle::Shaded, None, Vec3::ZERO))[0].colour
        );
        // A body that does not exist yet is an outline, not a solid.
        let flash = dev::flash();
        assert!(body(&drawables_at(&flash, DrawStyle::Shaded, None, Vec3::ZERO, 1.0)).is_empty());
        assert!(!body(&drawables_at(&flash, DrawStyle::Shaded, None, Vec3::ZERO, 4.5005)).is_empty());
        // Control: with no bodies there is no such part.
        let none = drawables(&dev::void(), DrawStyle::Shaded, None, Vec3::ZERO);
        assert!(!none.parts.iter().any(|(p, ..)| matches!(p, Part::Body(_))));
    }

    /// The rule a path was drawn by until S20, kept for the control arm of
    /// `a_drawn_path_follows_its_motion`: 96 straight pieces across the timeline (at most
    /// 10⁴ s of it), whatever the motion. The pieces' knots.
    fn old_rule(motion: &cav_sim::Motion, gravity: f64, duration: f64) -> Vec<crate::path::Knot> {
        (0..=96)
            .map(|k| {
                let t = duration.min(1.0e4) * f64::from(k) / 96.0;
                crate::path::Knot { t, at: Vec3::from_array(motion.position(t, gravity)) }
            })
            .collect()
    }

    /// The largest sag of the chords between `knots`: how far the path's point at the middle of
    /// each piece's time is from the piece's chord, metres.
    fn midpoint_sag(motion: &cav_sim::Motion, gravity: f64, knots: &[crate::path::Knot]) -> f64 {
        knots
            .windows(2)
            .map(|w| {
                let middle = Vec3::from_array(motion.position(0.5 * (w[0].t + w[1].t), gravity));
                let chord = w[1].at - w[0].at;
                let along = if chord.dot(chord) > 0.0 {
                    ((middle - w[0].at).dot(chord) / chord.dot(chord)).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                (middle - (w[0].at + chord * along)).length()
            })
            .fold(0.0, f64::max)
    }

    #[test]
    fn a_drawn_path_follows_its_motion() {
        use crate::path;
        use cav_sim::{BodySpec, Motion};
        // D1 (`docs/slices/S20.md` §4). Criterion, written before measuring: a circle of radius
        // 2 m at 0.01, 1 and 50 Hz over 12 s is drawn with every vertex on the path to 1e-9 m
        // and every chord's sag at most 1 % of the radius; a body that appears at 3 s and
        // vanishes at 5 s draws no line outside those times. Controls: the old rule (96 pieces
        // across the timeline) has a sag of over 25 % of the radius at 50 Hz (the square); the
        // count of lines stays under the cap.
        let (radius, centre) = (2.0, [1.0, -1.0, 3.0]);
        let ring = |frequency: f64| Motion::Circular {
            centre,
            u: [radius, 0.0, 0.0],
            v: [0.0, radius, 0.0],
            frequency,
            phase: 0.0,
        };
        let scene_of = |body: BodySpec| {
            let mut scene = dev::void();
            scene.duration = 12.0;
            scene.bodies = vec![body];
            assert_eq!(scene.validate(), Ok(()));
            scene
        };
        let gravity = dev::void().array.instrument.gravity;
        let lines = |scene: &SceneSpec| drawables(scene, DrawStyle::Shaded, None, Vec3::ZERO).lines;
        let bare = lines(&dev::void()).len();
        for frequency in [0.01, 1.0, 50.0] {
            let motion = ring(frequency);
            let scene = scene_of(BodySpec::moving("ring", 10.0, motion.clone()));
            let drawn = path::path(&scene.bodies[0], gravity, scene.duration, path::allowance(&scene));
            assert!(!drawn.capped, "{frequency} Hz: {} pieces", drawn.pieces());
            // Every vertex on the path: where the motion is at its time, and on the circle.
            let mut off: f64 = 0.0;
            for knot in &drawn.knots {
                assert!((0.0..=12.0).contains(&knot.t), "{frequency} Hz: a vertex at {} s", knot.t);
                let on = Vec3::from_array(motion.position(knot.t, gravity));
                let from_centre = (knot.at - Vec3::from_array(centre)).length();
                off = off.max((knot.at - on).length()).max((from_centre - radius).abs());
            }
            assert!(off <= 1e-9, "{frequency} Hz: a vertex {off} m off the path");
            // Every chord's sag: for a circle, r·(1 − cos(Δθ/2)) exactly; and as measured.
            let exact = drawn
                .knots
                .windows(2)
                .map(|w| radius * (1.0 - (std::f64::consts::PI * frequency * (w[1].t - w[0].t)).cos()))
                .fold(0.0, f64::max);
            let measured = midpoint_sag(&motion, gravity, &drawn.knots);
            assert!(exact <= 0.01 * radius, "{frequency} Hz: a sag of {exact} m");
            assert!((measured - exact).abs() < 1e-9, "{measured} against {exact}");
            // What is drawn: the whole circle once when a turn fits in the timeline, else the
            // arc the timeline covers.
            let turned = frequency * (drawn.knots[drawn.pieces()].t - drawn.knots[0].t);
            assert!((turned - (frequency * 12.0).min(1.0)).abs() < 1e-12, "{frequency} Hz: {turned} turns");
            // And these pieces are the lines the scene draws for it.
            let all = lines(&scene);
            assert_eq!(all.len() - bare, drawn.pieces(), "{frequency} Hz");
            for knot in &drawn.knots {
                let at = knot.at.to_f32();
                assert!(all.iter().any(|l| l.a == at || l.b == at), "{frequency} Hz: a vertex not drawn");
            }
            // Controls: the old rule at 50 Hz is the square; the lines are under the cap.
            let old = midpoint_sag(&motion, gravity, &old_rule(&motion, gravity, 12.0));
            println!(
                "{frequency} Hz: {} pieces, sag {:.4} % of the radius (the old rule: 96 pieces, {:.2} %)",
                drawn.pieces(),
                100.0 * exact / radius,
                100.0 * old / radius
            );
            if frequency == 50.0 {
                assert!(old > 0.25 * radius, "control: the old rule's sag is {old} m");
            }
            assert!(drawn.pieces() <= path::PATH_PIECES && drawn.pieces() >= 4);
        }
        // A body that appears at 3 s and vanishes at 5 s draws nothing outside: on a slow
        // circle, the arc of those two seconds; on a quick one, one turn from when it appears.
        for (frequency, until) in [(0.01, 5.0), (1.0, 4.0), (50.0, 3.02)] {
            let body = BodySpec {
                appears: Some(3.0),
                vanishes: Some(5.0),
                ..BodySpec::moving("ring", 10.0, ring(frequency))
            };
            let scene = scene_of(body);
            let drawn = path::path(&scene.bodies[0], gravity, scene.duration, path::allowance(&scene));
            let times: Vec<f64> = drawn.knots.iter().map(|k| k.t).collect();
            assert!(times.iter().all(|t| (3.0..=5.0).contains(t)), "{frequency} Hz: {times:?}");
            assert_eq!(times[0], 3.0);
            assert!((times[times.len() - 1] - until).abs() < 1e-12, "{frequency} Hz: to {:?}", times.last());
            // (Not yet there at the playhead, the body itself is an outline of lines: the same
            // body at rest draws those and no path.)
            let mut still = scene.clone();
            still.bodies[0].motion = Motion::Static { position: centre };
            assert_eq!(lines(&scene).len() - lines(&still).len(), drawn.pieces());
        }
        // A body that is never there in the timeline has no path at all.
        let late = scene_of(BodySpec { appears: Some(20.0), ..BodySpec::moving("ring", 10.0, ring(1.0)) });
        let mut still = late.clone();
        still.bodies[0].motion = Motion::Static { position: centre };
        assert_eq!(lines(&late).len(), lines(&still).len());
        // The cap: a thousand quick circles share the scene's pieces and stay under them; a
        // path that cannot be drawn within its share is reported, never silently coarse.
        let mut crowd = dev::void();
        crowd.duration = 12.0;
        crowd.bodies = (0..1000).map(|n| BodySpec::moving(&format!("ring {n}"), 10.0, ring(50.0))).collect();
        assert_eq!(path::allowance(&crowd), path::SCENE_PATH_PIECES / 1000);
        let drawn = lines(&crowd).len() - bare;
        assert!((1000 * 16..=path::SCENE_PATH_PIECES).contains(&drawn), "{drawn} lines");
        assert!(decimation(&crowd).is_empty(), "each fits its share: {:?}", decimation(&crowd));
        let buzz = |tilt_frequency: f64| Motion::Sphere {
            centre,
            radius: 1.5,
            tilt: 60.0,
            tilt_frequency,
            spin_frequency: 0.073 * tilt_frequency,
        };
        let mut scene = scene_of(BodySpec::moving("buzz", 10.0, buzz(300.0)));
        assert_eq!(lines(&scene).len() - bare, path::PATH_PIECES, "drawn at its cap");
        assert_eq!(decimation(&scene), ["the path of `buzz` is drawn coarsely: 4096 pieces, its cap"]);
        // Control: the same body slow enough is drawn in full, and nothing is reported.
        scene.bodies[0].motion = buzz(0.3);
        assert!(lines(&scene).len() - bare < path::PATH_PIECES);
        assert!(decimation(&scene).is_empty());
    }

    #[test]
    fn a_path_of_ten_thousand_periods_is_built_within_its_cap() {
        use crate::path;
        use cav_sim::Motion;
        // S20 §6 (recorded, not gated): the time to cut the path of a body that goes round 10⁴
        // times in the timeline, within the cap of pieces. Two cases: a circle (one period is
        // drawn), and a motion that never repeats (it meets its cap). The times are printed;
        // the count of pieces is what is held.
        let gravity = 9.81;
        let ring = Motion::Circular {
            centre: [0.0; 3],
            u: [2.0, 0.0, 0.0],
            v: [0.0, 2.0, 0.0],
            frequency: 1000.0,
            phase: 0.0,
        };
        let buzz = Motion::Sphere {
            centre: [0.0; 3],
            radius: 1.5,
            tilt: 60.0,
            tilt_frequency: 1000.0,
            spin_frequency: 0.37,
        };
        for (name, motion, capped) in [("a circle", ring, false), ("a forced sphere", buzz, true)] {
            let body = cav_sim::BodySpec::moving("b", 1.0, motion);
            let build = || path::path(&body, gravity, 10.0, path::PATH_PIECES);
            let drawn = build();
            assert_eq!(drawn.capped, capped, "{name}");
            assert!(
                drawn.pieces() >= 16 && drawn.pieces() <= path::PATH_PIECES,
                "{name}: {}",
                drawn.pieces()
            );
            // The median of 21 builds.
            let mut times: Vec<f64> = (0..21)
                .map(|_| {
                    let started = std::time::Instant::now();
                    std::hint::black_box(build());
                    started.elapsed().as_secs_f64()
                })
                .collect();
            times.sort_by(f64::total_cmp);
            println!(
                "path of 10^4 periods, {name}: {} pieces in {:.1} µs (the budget: 2000 µs)",
                drawn.pieces(),
                1.0e6 * times[10]
            );
        }
    }

    #[test]
    fn a_voxel_body_is_drawn_as_its_leaves() {
        use cav_sim::voxel_view::{Colouring, LeafField, Overlay, voxel_frame};
        let scene = dev::sphere();
        let trees: Vec<_> = scene
            .bodies
            .iter()
            .map(|b| b.shape.as_ref().map(|s| std::sync::Arc::new(s.tree(b.mass, &scene.materials).unwrap())))
            .collect();
        let leaves = trees[0].as_ref().unwrap().leaves.len();
        let body = |d: &Drawables, n: usize| {
            let at = d.parts.iter().position(|(p, ..)| *p == Part::Body(n)).unwrap();
            let (start, end) = (if at == 0 { 0 } else { d.parts[at - 1].2 }, d.parts[at].2);
            d.triangles[start..end].to_vec()
        };
        let frame = voxel_frame(
            &scene,
            &trees,
            Colouring { field: LeafField::Height, tower: 0 },
            Overlay::default(),
            0.0,
        );
        let d = drawables_with(&scene, DrawStyle::Shaded, None, Vec3::ZERO, 0.0, Some(&frame));
        // Twelve triangles a leaf, and the analytic sphere still a sphere.
        assert_eq!(body(&d, 0).len(), 36 * leaves);
        assert_eq!(body(&d, 1).len(), 8 * 16 * 3);
        // Coloured by the field: the highest leaf and the lowest are not the same colour.
        let colours: std::collections::HashSet<[u32; 4]> =
            body(&d, 0).iter().map(|v| v.colour.map(f32::to_bits)).collect();
        assert!(colours.len() > 3, "{} colours", colours.len());
        // Overlays: twelve edges a box.
        let with = voxel_frame(
            &scene,
            &trees,
            Colouring::default(),
            Overlay { leaves: true, ..Overlay::default() },
            0.0,
        );
        let lines = drawables_with(&scene, DrawStyle::Shaded, None, Vec3::ZERO, 0.0, Some(&with)).lines.len();
        let without =
            drawables_with(&scene, DrawStyle::Shaded, None, Vec3::ZERO, 0.0, Some(&frame)).lines.len();
        assert_eq!(lines - without, 12 * leaves);
        // Control: without the frame the voxel body is drawn as any body, a sphere.
        assert_eq!(body(&drawables(&scene, DrawStyle::Shaded, None, Vec3::ZERO), 0).len(), 8 * 16 * 3);
    }

    #[test]
    fn a_tree_node_is_drawn_in_its_depth_s_colour_and_opacity() {
        use cav_sim::voxel_view::{Colouring, Fade, Overlay, TreeOverlay, voxel_frame};
        let scene = dev::sphere();
        let trees: Vec<_> = scene
            .bodies
            .iter()
            .map(|b| b.shape.as_ref().map(|s| std::sync::Arc::new(s.tree(b.mass, &scene.materials).unwrap())))
            .collect();
        let tree_lines = |style: DrawStyle, fade: Fade, faces: bool| {
            let overlay = Overlay {
                tree: Some(TreeOverlay { fade, faces, ..TreeOverlay::default() }),
                ..Overlay::default()
            };
            let frame = voxel_frame(&scene, &trees, Colouring::default(), overlay, 0.0);
            let d = drawables_with(&scene, style, None, Vec3::ZERO, 0.0, Some(&frame));
            let at = d.parts.iter().position(|(p, ..)| *p == Part::Tree).expect("a tree part");
            let (l0, t0) = if at == 0 { (0, 0) } else { (d.parts[at - 1].1, d.parts[at - 1].2) };
            (
                frame,
                d.lines[l0..d.parts[at].1].to_vec(),
                d.triangles.len() - t0 - (d.triangles.len() - d.parts[at].2),
            )
        };
        let (frame, lines, faces) = tree_lines(DrawStyle::XRay, Fade::Off, false);
        let range = frame.depths.unwrap();
        assert!(range.1 >= 2, "a tree of several depths: {range:?}");
        assert_eq!((lines.len(), faces), (12 * frame.tree.len(), 0));
        // Each node's twelve edges are its depth's colour on the map, whole.
        for (node, edges) in frame.tree.iter().zip(lines.chunks(12)) {
            let want = tree_colour(node.depth, range, true);
            assert!(
                edges.iter().all(|e| e.colour == [want[0], want[1], want[2], 1.0]),
                "depth {}",
                node.depth
            );
        }
        // Control: the shallowest and the deepest are not one colour.
        assert_ne!(tree_colour(range.0, range, true), tree_colour(range.1, range, true));
        // Fading with depth in x-ray: alpha rises with depth, the deepest whole; faces when asked.
        let (frame, lines, faces) = tree_lines(DrawStyle::XRay, Fade::Deep, true);
        assert_eq!(faces, 36 * frame.tree.len());
        let alpha = |depth: u8| {
            let n = frame.tree.iter().position(|b| b.depth == depth).unwrap();
            lines[12 * n].colour[3]
        };
        for depth in range.0..range.1 {
            assert!(alpha(depth) < alpha(depth + 1), "{depth}");
        }
        assert_eq!(alpha(range.1), 1.0);
        // In an opaque style a faint line is that much of its colour over the background: the
        // shallowest is nearer the background than the deepest is, and no faces are drawn.
        let (frame, lines, faces) = tree_lines(DrawStyle::Shaded, Fade::Deep, true);
        assert_eq!(faces, 0);
        let bg = linear(DARK.background, 1.0);
        let off = |n: usize| {
            let c = lines[12 * n].colour;
            (0..3).map(|i| (c[i] - bg[i]).abs()).sum::<f32>()
        };
        let want = tree_colour(range.0, range, true);
        let shallow = (0..3).map(|i| (want[i] - bg[i]).abs()).sum::<f32>();
        assert!((off(0) - cav_sim::voxel_view::FAINTEST as f32 * shallow).abs() < 1e-4, "{}", off(0));
        assert_eq!(frame.tree[0].depth, range.0);
    }

    #[test]
    fn drawables_follow_the_spec() {
        let origin = Vec3::new(0.0, 0.0, 5.0);
        let room = drawables(&SceneSpec::room(), DrawStyle::Shaded, None, origin);
        let void = drawables(&dev::void(), DrawStyle::Shaded, None, origin);
        let floor = drawables(&dev::infinite_floor(), DrawStyle::Shaded, None, origin);
        assert_eq!((towers(&room), has_room(&room), room.ground.is_some()), (5, true, false));
        assert_eq!((towers(&void), has_room(&void), void.ground.is_some()), (5, false, false));
        assert_eq!((towers(&floor), has_room(&floor), floor.ground.is_some()), (5, false, true));
        // The room is exactly what the void lacks: 22 grid lines, 12 edges and a floor panel.
        assert_eq!(room.lines.len() - void.lines.len(), 11 + 11 + 12);
        assert_eq!(room.triangles.len() - void.triangles.len(), 6);
        // The towers are the same in every scene (the room part comes first, so compare the tails).
        assert_eq!(room.lines[34..], void.lines[..]);
        assert_eq!(floor.lines, void.lines);
        // Each tower: its baseline and 11 ticks of two strokes; a tower more is a part more.
        assert_eq!(void.lines.len(), 5 * (1 + 2 * 11));
        let mut six = dev::void();
        six.array.towers.push(cav_sim::TowerSpec::at("tower 6", [2.0, 2.0, 0.0]));
        assert_eq!(towers(&drawables(&six, DrawStyle::Shaded, None, origin)), 6);
        // The floor's surface is where the spec puts it, relative to the target.
        assert_eq!(floor.ground.unwrap().height, -5.0);
        // Nothing is NaN, and triangles come in threes.
        assert!(room.lines.iter().all(|l| l.a.iter().chain(&l.b).all(|v| v.is_finite())));
        assert!(room.triangles.iter().all(|v| v.position.iter().chain(&v.normal).all(|c| c.is_finite())));
        assert_eq!(room.triangles.len() % 3, 0);
    }

    #[test]
    fn a_world_s_base_is_drawn() {
        let origin = Vec3::new(1.0, 2.0, 5.0);
        let part = |d: &Drawables, p: Part| d.parts.iter().any(|(q, ..)| *q == p);
        // The plane: the ground a hair under its surface, and nothing else added.
        let plane = drawables(&dev::plane(), DrawStyle::Shaded, None, origin);
        let ground = plane.ground.expect("the plane is the ground");
        assert!((f64::from(ground.height) - (0.0 - 5.0 - PLANE_BELOW)).abs() < 1e-6);
        assert!(!part(&plane, Part::Base));
        // The planet: its grid, every line on the sphere (to the f32 of the target-relative
        // ends), the rings out to the equator, no flat ground.
        let planet = drawables(&dev::planet(), DrawStyle::Shaded, None, origin);
        assert!(planet.ground.is_none() && part(&planet, Part::Base));
        let radius = 2.0e4;
        let rings = planet_rings(radius);
        assert!(
            rings.len() < 60 && (rings[rings.len() - 1] - 0.5 * std::f64::consts::PI * radius).abs() < 1e-6
        );
        let grid: Vec<&Line> = planet.lines.iter().filter(|l| l.width == 1.0).collect();
        assert!(grid.len() >= rings.len() * PLANET_RING_SEGMENTS as usize);
        for line in &grid {
            for end in [line.a, line.b] {
                let p = origin + Vec3::new(f64::from(end[0]), f64::from(end[1]), f64::from(end[2]));
                let off = (p - Vec3::new(0.0, 0.0, -radius)).length() - radius;
                // f32 keeps 24 bits: 2e4 m to about 1 mm.
                assert!(off.abs() < 5e-3 * (1.0 + p.length() / 1e3), "{off} at {p:?}");
            }
        }
        // The pole is on the surface and a ring 10 m out drops by its sagitta.
        assert_eq!(on_planet(radius, 0.0, 0.3).z, 0.0);
        assert!((on_planet(radius, 10.0, 0.0).z + 100.0 / (2.0 * radius)).abs() < 1e-9);
        // Control: a patch draws neither.
        let mut patch = dev::planet();
        patch.world.as_mut().unwrap().base = cav_sim::world::BaseSpec::Patch;
        let patch = drawables(&patch, DrawStyle::Shaded, None, origin);
        assert!(patch.ground.is_none() && !part(&patch, Part::Base));
    }

    #[test]
    fn styles_and_selection_change_what_is_built() {
        let scene = SceneSpec::room();
        let build = |style, highlight| drawables(&scene, style, highlight, Vec3::ZERO);
        let shaded = build(DrawStyle::Shaded, None);
        // Wireframe has the same lines and no surfaces.
        let wire = build(DrawStyle::Wireframe, None);
        assert!(wire.triangles.is_empty() && !shaded.triangles.is_empty());
        assert_eq!(wire.lines, shaded.lines);
        // Paper is light with dark ink; the dark styles are the reverse.
        let paper = build(DrawStyle::Paper, None);
        assert!(paper.background[1] > 0.8 && shaded.background[1] < 0.01);
        assert!(paper.lines.iter().all(|l| l.colour[1] < paper.background[1]));
        // Selecting a tower adds its ring and nothing else; selecting nothing adds nothing.
        let selected = build(DrawStyle::Shaded, Some(Target::Tower(3)));
        assert_eq!(selected.lines.len(), shaded.lines.len() + 48);
        assert_eq!(selected.triangles, shaded.triangles);
        assert_eq!(build(DrawStyle::Shaded, None), shaded);
        // A highlight for something the scene lacks changes nothing.
        assert_eq!(build(DrawStyle::Shaded, Some(Target::Floor)), shaded);
    }

    #[test]
    fn normals_face_outwards() {
        let scene = dev::void();
        let d = drawables(&scene, DrawStyle::Shaded, None, Vec3::ZERO);
        // Every triangle's winding agrees with its stored normals (counter-clockwise from outside).
        for corners in d.triangles.as_chunks::<3>().0 {
            let p: Vec<Vec3> = corners
                .iter()
                .map(|v| Vec3::new(v.position[0].into(), v.position[1].into(), v.position[2].into()))
                .collect();
            let geometric = (p[1] - p[0]).cross(p[2] - p[0]);
            if geometric.length() < 1e-9 {
                continue;
            }
            for v in corners {
                let stored = Vec3::new(v.normal[0].into(), v.normal[1].into(), v.normal[2].into());
                assert!((stored.length() - 1.0).abs() < 1e-5);
                assert!(geometric.normalised().dot(stored) > 0.2, "a triangle is wound inwards: {corners:?}");
            }
        }
    }

    /// The largest difference, in metres, between the vertices of the default scene built at the
    /// world origin and built `shift` away, each relative to its own target.
    fn worst_vertex_shift(shift: Vec3) -> f32 {
        let near = SceneSpec::room();
        let mut far = near.clone();
        for tower in &mut far.array.towers {
            tower.position = (Vec3::from_array(tower.position) + shift).to_array();
        }
        let room = far.room.as_mut().unwrap();
        room.origin = (Vec3::from_array(room.origin) + shift).to_array();
        let target = Vec3::new(0.3, -0.7, 5.1);
        let a = drawables(&near, DrawStyle::Shaded, Some(Target::Tower(2)), target);
        let b = drawables(&far, DrawStyle::Shaded, Some(Target::Tower(2)), target + shift);
        assert_eq!((a.lines.len(), a.triangles.len()), (b.lines.len(), b.triangles.len()));
        let mut worst = 0f32;
        for (p, q) in a.triangles.iter().zip(&b.triangles) {
            for axis in 0..3 {
                worst = worst.max((p.position[axis] - q.position[axis]).abs());
            }
        }
        for (p, q) in a.lines.iter().zip(&b.lines) {
            for axis in 0..3 {
                worst = worst.max((p.a[axis] - q.a[axis]).abs()).max((p.b[axis] - q.b[axis]).abs());
            }
        }
        worst
    }

    cav_validate::case! {
        test: validation_vertices_are_local_10_km_from_the_origin,
        name: "render.local_frame_at_10_km",
        subsystem: "render",
        slice: "S0",
        reference: Exact,
        against: "CPU geometry only: the vertices of the same scene built at the world origin, each relative to its own target",
        unit: Metres,
        tolerance: 1e-5,
        measure: cav_validate::Measured::Error(f64::from(worst_vertex_shift(Vec3::new(10_000.0, -7_000.0, 300.0)))),
    }

    #[test]
    fn vertices_are_camera_relative_far_from_the_origin() {
        let shift = Vec3::new(10_000.0, -7_000.0, 300.0);
        let target = Vec3::new(0.3, -0.7, 5.1);
        let worst = worst_vertex_shift(shift);
        assert!(worst <= 1e-5, "10 km from the origin the vertices moved by {worst} m");
        // No shift at all is no difference at all: the measure is not noise.
        assert_eq!(worst_vertex_shift(Vec3::ZERO), 0.0);

        // Control arm: the shortcut this guards against. Rounding world positions to f32 before
        // subtracting loses over a tenth of a millimetre at 10 km, which the tolerance would catch.
        let point = Vec3::new(4.085, 0.035, 5.085) + shift;
        let origin = target + shift;
        let careful = local(point, origin);
        let naive = [
            point.x as f32 - origin.x as f32,
            point.y as f32 - origin.y as f32,
            point.z as f32 - origin.z as f32,
        ];
        let error = (0..3).map(|i| (careful[i] - naive[i]).abs()).fold(0f32, f32::max);
        assert!(error > 1e-4, "the naive path should be visibly worse at 10 km, but differs by {error} m");
    }

    #[test]
    fn marks_are_counted_and_capped_whatever_the_numbers() {
        let count = |lo: f64, hi: f64, step: f64| marks(lo, hi, step).count();
        // The default room: eleven lines a metre apart, the one through the origin with index 0.
        let room: Vec<(i64, f64)> = marks(-5.0, 5.0, 1.0).collect();
        assert_eq!(room.len(), 11);
        assert_eq!((room[0], room[5], room[10]), ((-5, -5.0), (0, 0.0), (5, 5.0)));
        assert_eq!(mark_step(10.0), 1.0, "a room of ordinary size is not thinned");
        assert_eq!(mark_step(200.0), 1.0);
        // Big extents are thinned to at most MAX_MARKS intervals.
        for extent in [201.0, 3.0e3, 1.0e4, 3.0e6, 1.0e10, 1.0e300] {
            let step = mark_step(extent);
            assert!(
                step > 1.0 && count(0.0, extent, step) <= MAX_MARKS as usize + 1,
                "{extent}: step {step}"
            );
        }
        assert_eq!(mark_step(3.0e6), 1.0e5);
        // The numbers that hung the old loops (S0 review, finding 1): 1e17 + 1.0 == 1e17, and a
        // tower of 3e6 m. Even with the wrong step, the count is capped; with odd numbers, empty.
        assert!(count(1.0e17, 1.0e17 + 10.0, 1.0) <= 17, "and the loop ends");
        assert_eq!(count(0.0, 1.0e10, 1.0), MAX_MARKS as usize + 1);
        assert_eq!(count(0.0, 3.0e6, 1.0), MAX_MARKS as usize + 1);
        assert_eq!(count(f64::NAN, 5.0, 1.0), 0);
        assert_eq!(count(0.0, f64::INFINITY, 1.0), 0);
        assert_eq!(count(5.0, -5.0, 1.0), 0);
    }

    #[test]
    fn the_largest_valid_scene_is_drawn_with_a_bounded_number_of_things() {
        use cav_sim::spec::limits;
        // Every range at its limit: the most towers, the tallest, in the biggest room, far away.
        let mut scene = SceneSpec::room();
        scene.array.instrument.tower_height = limits::TOWER_HEIGHT;
        scene.array.instrument.ifo_separation = 500.0;
        scene.array.towers = (0..limits::TOWERS)
            .map(|n| cav_sim::TowerSpec::at(&format!("t{n}"), [n as f64, 0.0, 0.0]))
            .collect();
        let room = scene.room.as_mut().unwrap();
        (room.origin, room.size) = ([limits::POSITION - limits::ROOM_SIZE; 3], [limits::ROOM_SIZE; 3]);
        assert_eq!(scene.validate(), Ok(()));
        let started = std::time::Instant::now();
        let d = drawables(&scene, DrawStyle::Shaded, Some(Target::Tower(7)), Vec3::ZERO);
        // Per tower: a baseline and at most MAX_MARKS + 1 ticks of two strokes; the room: two sets
        // of grid lines and twelve edges; one selection ring.
        let per_tower = 1 + 2 * (MAX_MARKS as usize + 1);
        assert!(
            d.lines.len() <= limits::TOWERS * per_tower + 2 * (MAX_MARKS as usize + 1) + 12 + 48,
            "{} lines",
            d.lines.len()
        );
        // 48 bytes a line, 40 a vertex: well inside a 256 MB buffer, the smallest limit wgpu promises.
        let bytes = d.lines.len() * size_of::<Line>() + d.triangles.len() * size_of::<Vertex>();
        assert!(bytes < 128 << 20, "{bytes} bytes of geometry");
        assert!(started.elapsed().as_secs() < 5, "building it took {:?}", started.elapsed());
        // It says what it thinned; the default scene thins nothing.
        assert_eq!(decimation(&scene), ["room grid every 100 m", "tower ticks every 10 m"]);
        assert!(decimation(&SceneSpec::room()).is_empty(), "the control: an ordinary scene is drawn in full");
    }

    #[test]
    fn each_tower_is_drawn_at_its_own_height_and_none_is_drawn_for_none() {
        // The lines of tower `n`: its baseline is the first.
        fn baseline(d: &Drawables, n: usize) -> Line {
            let part = d.parts.iter().position(|(p, ..)| *p == Part::Tower(n)).unwrap();
            let first = if part == 0 { 0 } else { d.parts[part - 1].1 };
            d.lines[first]
        }
        let mixed = dev::mixed_array();
        let d = drawables(&mixed, DrawStyle::Shaded, None, Vec3::ZERO);
        let height = |n: usize| f64::from(baseline(&d, n).b[2] - baseline(&d, n).a[2]);
        assert!(
            (height(0) - 10.1).abs() < 1e-4 && (height(1) - 140.0).abs() < 1e-3,
            "{} {}",
            height(0),
            height(1)
        );
        // Its upper launch point is drawn at its own separation: 70 m up, not 5.
        assert_eq!(towers(&d), 3);
        // A metre a tick up the 100 m tower: a hundred ticks, under the cap, so nothing is thinned.
        assert!(decimation(&mixed).is_empty());
        // Control: the same towers on the array's instrument are all 10.1 m.
        let mut plain = mixed.clone();
        plain.array.towers.iter_mut().for_each(|t| t.instrument = None);
        let p = drawables(&plain, DrawStyle::Shaded, None, Vec3::ZERO);
        assert!((f64::from(baseline(&p, 1).b[2] - baseline(&p, 1).a[2]) - 10.1).abs() < 1e-4);
        // No towers: no tower parts, nothing NaN, and the bodies and the room are still drawn.
        let none = dev::no_towers();
        let d = drawables(&none, DrawStyle::Shaded, Some(Target::Tower(1)), Vec3::ZERO);
        assert_eq!(towers(&d), 0);
        assert!(has_room(&d) && d.parts.iter().any(|(p, ..)| *p == Part::Body(0)));
        assert!(d.lines.iter().all(|l| l.a.iter().chain(&l.b).all(|v| v.is_finite())));
    }
}
