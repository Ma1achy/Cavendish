//! From a scene spec to things to draw, on the CPU: thick line segments, triangles and an optional
//! infinite ground.
//!
//! Positions are formed in `f64` as offsets from the view's target and only then rounded to `f32`
//! ([`local`]), so the GPU never sees a world coordinate (REBUILD §3.4).

use std::f64::consts::TAU;

use bytemuck::{Pod, Zeroable};
use cav_core::Vec3;
use cav_sim::{DrawStyle, SceneSpec};

/// One thick line segment, drawn as a screen-facing quad. `repr(C)`: this is the instance layout.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct Line {
    /// Start, metres from the view's target.
    pub a: [f32; 3],
    /// Width, pixels.
    pub width: f32,
    /// End, metres from the view's target.
    pub b: [f32; 3],
    pub _pad: f32,
    /// Linear RGB and alpha.
    pub colour: [f32; 4],
}

/// One triangle corner. `repr(C)`: this is the vertex layout.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct Vertex {
    /// Metres from the view's target.
    pub position: [f32; 3],
    pub normal: [f32; 3],
    /// Linear RGB and alpha.
    pub colour: [f32; 4],
}

/// What a run of lines and triangles belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// Tower `n`, from 0.
    Tower(usize),
    Room,
}

/// What is highlighted as the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Highlight {
    #[default]
    None,
    /// Tower `n`, from 0.
    Tower(usize),
    Room,
    Floor,
}

/// An infinite horizontal floor, drawn as a grid that fades towards the horizon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    /// Height of the surface relative to the view's target, metres.
    pub height: f32,
    /// The target's x and y modulo the 100 m grid period, so the grid stays put as the camera moves
    /// without the shader ever seeing a large coordinate.
    pub phase: [f32; 2],
    /// Linear RGB of the surface and of its grid lines.
    pub surface: [f32; 4],
    pub line: [f32; 4],
}

/// Everything to draw for one view.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawables {
    pub lines: Vec<Line>,
    pub triangles: Vec<Vertex>,
    pub ground: Option<Ground>,
    /// The background colour, linear RGB.
    pub background: [f32; 3],
    /// Each part with where its lines and triangle corners end (they start where the previous ended).
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
};

/// The sRGB colours the tests look for: the background and tower colours of the dark styles.
pub const BACKGROUND_SRGB: [u8; 3] = [0x0a, 0x0a, 0x0a];
pub const PAPER_SRGB: [u8; 3] = [0xf1, 0xed, 0xe4];

/// Tower `n`'s colour in the dark styles, sRGB bytes.
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

/// A world position as an `f32` offset from `origin`. The subtraction is in `f64`; only the small
/// result is rounded.
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

/// Builds what to draw for `scene` in `style`, relative to `origin` (the view's target).
#[must_use]
pub fn drawables(scene: &SceneSpec, style: DrawStyle, highlight: Highlight, origin: Vec3) -> Drawables {
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
        let selected = highlight == Highlight::Room;
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
        // A 1 m grid on the floor, with the lines through the world axes a little stronger.
        let grid = linear(palette.grid, 1.0);
        let axis = linear(palette.grid_axis, 1.0);
        let mut x = lo.x.ceil();
        while x <= hi.x {
            b.line(
                Vec3::new(x, lo.y, lo.z),
                Vec3::new(x, hi.y, lo.z),
                1.0,
                if x == 0.0 { axis } else { grid },
            );
            x += 1.0;
        }
        let mut y = lo.y.ceil();
        while y <= hi.y {
            b.line(
                Vec3::new(lo.x, y, lo.z),
                Vec3::new(hi.x, y, lo.z),
                1.0,
                if y == 0.0 { axis } else { grid },
            );
            y += 1.0;
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
        let selected = highlight == Highlight::Tower(n);
        let colour = linear(palette.towers[n % palette.towers.len()], 1.0);
        // The baseline, with a tick every metre.
        b.line(base, top, if selected { 4.0 } else { 2.5 }, colour);
        let ticks = instrument.tower_height.floor() as u32;
        for k in 0..=ticks {
            let at = base + Vec3::Z * f64::from(k);
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
                b.sphere(base + Vec3::Z * height, 0.085, colour);
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

    if let Some(floor) = &scene.floor {
        let selected = highlight == Highlight::Floor;
        let period = 100.0;
        b.out.ground = Some(Ground {
            height: (floor.height - origin.z) as f32,
            phase: [origin.x.rem_euclid(period) as f32, origin.y.rem_euclid(period) as f32],
            surface: linear(palette.ground, 1.0),
            line: linear(if selected { palette.selection } else { palette.ground_line }, 1.0),
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
    fn drawables_follow_the_spec() {
        let origin = Vec3::new(0.0, 0.0, 5.0);
        let room = drawables(&SceneSpec::room(), DrawStyle::Shaded, Highlight::None, origin);
        let void = drawables(&dev::void(), DrawStyle::Shaded, Highlight::None, origin);
        let floor = drawables(&dev::infinite_floor(), DrawStyle::Shaded, Highlight::None, origin);
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
        six.array.towers.push(cav_sim::TowerSpec { label: "tower 6".into(), position: [2.0, 2.0, 0.0] });
        assert_eq!(towers(&drawables(&six, DrawStyle::Shaded, Highlight::None, origin)), 6);
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
        let shaded = build(DrawStyle::Shaded, Highlight::None);
        // Wireframe has the same lines and no surfaces.
        let wire = build(DrawStyle::Wireframe, Highlight::None);
        assert!(wire.triangles.is_empty() && !shaded.triangles.is_empty());
        assert_eq!(wire.lines, shaded.lines);
        // Paper is light with dark ink; the dark styles are the reverse.
        let paper = build(DrawStyle::Paper, Highlight::None);
        assert!(paper.background[1] > 0.8 && shaded.background[1] < 0.01);
        assert!(paper.lines.iter().all(|l| l.colour[1] < paper.background[1]));
        // Selecting a tower adds its ring and nothing else; selecting nothing adds nothing.
        let selected = build(DrawStyle::Shaded, Highlight::Tower(2));
        assert_eq!(selected.lines.len(), shaded.lines.len() + 48);
        assert_eq!(selected.triangles, shaded.triangles);
        assert_eq!(build(DrawStyle::Shaded, Highlight::None), shaded);
        // A highlight for something the scene lacks changes nothing.
        assert_eq!(build(DrawStyle::Shaded, Highlight::Floor), shaded);
    }

    #[test]
    fn normals_face_outwards() {
        let scene = dev::void();
        let d = drawables(&scene, DrawStyle::Shaded, Highlight::None, Vec3::ZERO);
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

    #[test]
    fn vertices_are_camera_relative_far_from_the_origin() {
        // The same scene at the world origin and 10 km away, each seen from its own target.
        let shift = Vec3::new(10_000.0, -7_000.0, 300.0);
        let near = SceneSpec::room();
        let mut far = near.clone();
        for tower in &mut far.array.towers {
            tower.position = (Vec3::from_array(tower.position) + shift).to_array();
        }
        let room = far.room.as_mut().unwrap();
        room.origin = (Vec3::from_array(room.origin) + shift).to_array();
        let target = Vec3::new(0.3, -0.7, 5.1);
        let a = drawables(&near, DrawStyle::Shaded, Highlight::Tower(1), target);
        let b = drawables(&far, DrawStyle::Shaded, Highlight::Tower(1), target + shift);
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
        assert!(worst <= 1e-5, "10 km from the origin the vertices moved by {worst} m");

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
}
