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

/// How many straight pieces a body's path is drawn in.
pub const TRAIL_PIECES: u32 = 96;

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
    let step = mark_step(scene.array.instrument.tower_height);
    if step > 1.0 {
        notes.push(format!("tower ticks every {step} m"));
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

    let instrument = &scene.array.instrument;
    for (n, tower) in scene.array.towers.iter().enumerate() {
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
    for (n, body) in scene.bodies.iter().enumerate() {
        let selected = highlight == Some(Target::Body(n + 1));
        let at = Vec3::from_array(body.motion.position(time, gravity));
        let radius = body.radius.max(BODY_RADIUS);
        let (from, until) = body.exists();
        let colour = linear(if selected { palette.selection } else { palette.body }, 1.0);
        // Its path over the timeline, if it has one: a count of pieces, whatever the numbers.
        if body.motion.frequency(gravity).is_some() || !body.motion.breaks().is_empty() {
            let trail = linear(palette.trail, 1.0);
            let point = |k: u32| {
                let t = scene.duration.min(1.0e4) * f64::from(k) / f64::from(TRAIL_PIECES);
                Vec3::from_array(body.motion.position(t, gravity))
            };
            for k in 0..TRAIL_PIECES {
                b.line(point(k), point(k + 1), 1.0, trail);
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
                let rgb = crate::colour::of_value(cube.value, range, diverging, dark);
                let hex = (u32::from(rgb[0]) << 16) | (u32::from(rgb[1]) << 8) | u32::from(rgb[2]);
                let h = 0.5 * cube.side;
                let d = Vec3::new(h, h, h);
                if filled {
                    b.cuboid(cube.centre - d, cube.centre + d, linear(hex, 1.0));
                } else {
                    b.box_edges(cube.centre - d, cube.centre + d, 1.0, linear(hex, 1.0));
                }
            }
            if selected {
                let (lo, hi) = cubes.cubes.iter().fold((at, at), |(lo, hi), c| {
                    let h = 0.5 * c.side;
                    (
                        Vec3::new(lo.x.min(c.centre.x - h), lo.y.min(c.centre.y - h), lo.z.min(c.centre.z - h)),
                        Vec3::new(hi.x.max(c.centre.x + h), hi.y.max(c.centre.y + h), hi.z.max(c.centre.z + h)),
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
    b.out
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
        // Its path is drawn too: the void's lines, and one piece of trail for each step.
        let lines =
            |scene: &SceneSpec| drawables_at(scene, DrawStyle::Shaded, None, Vec3::ZERO, 0.0).lines.len();
        assert_eq!(lines(&scene) - lines(&dev::void()), TRAIL_PIECES as usize);
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

    #[test]
    fn a_voxel_body_is_drawn_as_its_leaves() {
        use cav_sim::voxel_view::{Colouring, LeafField, Overlay, voxel_frame};
        let scene = dev::sphere();
        let trees: Vec<_> =
            scene.bodies.iter().map(|b| b.shape.as_ref().map(|s| std::sync::Arc::new(s.tree(b.mass).unwrap()))).collect();
        let leaves = trees[0].as_ref().unwrap().leaves.len();
        let body = |d: &Drawables, n: usize| {
            let at = d.parts.iter().position(|(p, ..)| *p == Part::Body(n)).unwrap();
            let (start, end) = (if at == 0 { 0 } else { d.parts[at - 1].2 }, d.parts[at].2);
            d.triangles[start..end].to_vec()
        };
        let frame = voxel_frame(&scene, &trees, Colouring { field: LeafField::Height, tower: 0 }, Overlay::default(), 0.0);
        let d = drawables_with(&scene, DrawStyle::Shaded, None, Vec3::ZERO, 0.0, Some(&frame));
        // Twelve triangles a leaf, and the analytic sphere still a sphere.
        assert_eq!(body(&d, 0).len(), 36 * leaves);
        assert_eq!(body(&d, 1).len(), 8 * 16 * 3);
        // Coloured by the field: the highest leaf and the lowest are not the same colour.
        let colours: std::collections::HashSet<[u32; 4]> =
            body(&d, 0).iter().map(|v| v.colour.map(f32::to_bits)).collect();
        assert!(colours.len() > 3, "{} colours", colours.len());
        // Overlays: twelve edges a box.
        let with = voxel_frame(&scene, &trees, Colouring::default(), Overlay { leaves: true, cut: None }, 0.0);
        let lines = drawables_with(&scene, DrawStyle::Shaded, None, Vec3::ZERO, 0.0, Some(&with)).lines.len();
        let without = drawables_with(&scene, DrawStyle::Shaded, None, Vec3::ZERO, 0.0, Some(&frame)).lines.len();
        assert_eq!(lines - without, 12 * leaves);
        // Control: without the frame the voxel body is drawn as any body, a sphere.
        assert_eq!(body(&drawables(&scene, DrawStyle::Shaded, None, Vec3::ZERO), 0).len(), 8 * 16 * 3);
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
}
