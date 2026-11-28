//! A closed mesh's pieces by the signed vertical-prism decomposition.
//!
//! For a closed, consistently wound mesh, a vertical ray up from a point inside crosses the
//! surface once more going out (a triangle facing up) than coming in (facing down); from a point
//! outside, as often each way. So the body's indicator is `Σ_t sign(n_z(t)) · [x in the prism
//! below t]`, and `cell ∩ body = Σ_t sign(n_z) · (cell ∩ prism below t)`. Each term is the cell
//! clipped by four half-spaces: the three vertical planes through the triangle's shadow's edges
//! and the triangle's own plane, below it. Vertical triangles add nothing. It is exact to
//! rounding and needs no case for faces lying on the lattice's planes (a clip by a plane through
//! a cell's wall keeps all of the cell or none of it).
//!
//! Only triangles whose shadow meets the cell's square and that reach above its floor can take
//! part. They are found from a 2-D bin of the shadows over the grid's columns, made once a grid
//! ([`MAX_BIN_ENTRIES`]).

use crate::Vec3;

use super::super::mesh::Mesh;
use super::super::{Aabb, Grid, VoxelError};
use super::convex::{Convex, plane_tolerance};
use super::{CLIP_WORK_BASE, CLIP_WORK_PER_PLANE, CellClip, Clip, Raw};

/// The most entries (a triangle in a bin) the 2-D bin of a mesh's shadows may hold: 64 MiB of
/// indices. The bins are made coarser (twice the columns a side, each time) until the count fits.
pub const MAX_BIN_ENTRIES: usize = 16 << 20;

/// What a triangle needs for the decomposition, made once.
struct Prism {
    /// Its corners, metres, the body's frame.
    corners: [Vec3; 3],
    /// `+1` facing up, `−1` facing down.
    sign: f64,
    /// Its shadow's box, x then y: `[lo_x, lo_y, hi_x, hi_y]`, metres.
    shadow: [f64; 4],
    /// Its highest z, metres.
    top: f64,
}

/// A mesh prepared for one grid.
struct Binned<'a> {
    prisms: Vec<Prism>,
    /// Columns a bin side.
    side: u32,
    /// Bins along x.
    across: u32,
    /// `start[b]..start[b + 1]` of `entries` are bin `b`'s triangles.
    start: Vec<u32>,
    entries: Vec<u32>,
    _mesh: &'a Mesh,
}

impl Clip for Mesh {
    /// The 2-D bin of the non-vertical triangles' shadows over `grid`'s columns.
    fn prepare(&self, grid: &Grid) -> Result<Box<dyn CellClip + '_>, VoxelError> {
        let mut prisms = Vec::with_capacity(self.triangles.len());
        for t in &self.triangles {
            let corners = t.map(|i| self.vertices[i as usize]);
            let [a, b, c] = corners;
            let up = (b - a).cross(c - a).z;
            if up == 0.0 {
                continue;
            }
            let shadow =
                [a.x.min(b.x).min(c.x), a.y.min(b.y).min(c.y), a.x.max(b.x).max(c.x), a.y.max(b.y).max(c.y)];
            prisms.push(Prism { corners, sign: up.signum(), shadow, top: a.z.max(b.z).max(c.z) });
        }
        let columns = [grid.cells[0].max(1), grid.cells[1].max(1)];
        let h = grid.pitch;
        let column = |x: f64, axis: usize| -> u32 {
            let o = [grid.origin.x, grid.origin.y][axis];
            ((x - o) / h).floor().clamp(0.0, f64::from(columns[axis] - 1)) as u32
        };
        let ranges: Vec<[u32; 4]> = prisms
            .iter()
            .map(|p| {
                [
                    column(p.shadow[0], 0),
                    column(p.shadow[1], 1),
                    column(p.shadow[2], 0),
                    column(p.shadow[3], 1),
                ]
            })
            .collect();
        // Count first, at the finest bins that fit.
        let mut side = 1u32;
        let count = |side: u32| -> usize {
            ranges
                .iter()
                .map(|r| ((r[2] / side - r[0] / side + 1) * (r[3] / side - r[1] / side + 1)) as usize)
                .sum()
        };
        while count(side) > MAX_BIN_ENTRIES {
            if side >= 1024 {
                return Err(VoxelError(format!(
                    "the mesh's {} triangles' shadows need more than {MAX_BIN_ENTRIES} bin entries",
                    prisms.len()
                )));
            }
            side *= 2;
        }
        let across = columns[0].div_ceil(side);
        let down = columns[1].div_ceil(side);
        let bins = (across * down) as usize;
        let mut start = vec![0u32; bins + 1];
        for r in &ranges {
            for by in r[1] / side..=r[3] / side {
                for bx in r[0] / side..=r[2] / side {
                    start[(by * across + bx) as usize + 1] += 1;
                }
            }
        }
        for b in 0..bins {
            start[b + 1] += start[b];
        }
        let mut fill = start.clone();
        let mut entries = vec![0u32; start[bins] as usize];
        for (t, r) in ranges.iter().enumerate() {
            for by in r[1] / side..=r[3] / side {
                for bx in r[0] / side..=r[2] / side {
                    let b = (by * across + bx) as usize;
                    entries[fill[b] as usize] = t as u32;
                    fill[b] += 1;
                }
            }
        }
        Ok(Box::new(Binned { prisms, side, across, start, entries, _mesh: self }))
    }
}

impl Binned<'_> {
    fn bin(&self, cell: [u32; 3]) -> &[u32] {
        let b = ((cell[1] / self.side) * self.across + cell[0] / self.side) as usize;
        match (self.start.get(b), self.start.get(b + 1)) {
            (Some(&s), Some(&e)) => &self.entries[s as usize..e as usize],
            _ => &[],
        }
    }
}

impl CellClip for Binned<'_> {
    fn work(&self, cell: [u32; 3], _boxed: &Aabb) -> u64 {
        // Each candidate is a box clipped by four planes (measured: 59 µs a cut cell of a
        // 16,384-triangle torus at 1/600 m).
        (CLIP_WORK_BASE / 4 + 4 * CLIP_WORK_PER_PLANE) * self.bin(cell).len() as u64
    }

    fn clip(&self, cell: [u32; 3], b: &Aabb) -> Raw {
        let mut out = Raw::default();
        for (sign, piece) in self.prisms_of(cell, b) {
            out = out.plus(sign, &piece.raw());
        }
        out
    }

    fn convex(&self, cell: [u32; 3], b: &Aabb) -> Option<Vec<(f64, Convex)>> {
        Some(self.prisms_of(cell, b))
    }

    fn surface(&self, _cell: [u32; 3], b: &Aabb) -> Option<Vec<[Vec3; 3]>> {
        Some(self.surface_of(b))
    }
}

impl Binned<'_> {
    /// The cell `b` (finest cell `cell`) clipped by each candidate triangle's prism: `(sign,
    /// piece)` in the frame of the cell's centre, the empty ones left out.
    fn prisms_of(&self, cell: [u32; 3], b: &Aabb) -> Vec<(f64, Convex)> {
        let c = b.centre();
        let side = b.size().x.max(b.size().y).max(b.size().z);
        let mut out = Vec::new();
        for &t in self.bin(cell) {
            let p = &self.prisms[t as usize];
            if p.top <= b.lo.z
                || p.shadow[2] <= b.lo.x
                || p.shadow[0] >= b.hi.x
                || p.shadow[3] <= b.lo.y
                || p.shadow[1] >= b.hi.y
            {
                continue;
            }
            let mut piece = Convex::from_box(b.lo - c, b.hi - c);
            // The shadow's edges, wound anticlockwise seen from above when the triangle faces up:
            // the inside is to the left of each edge, so its outward normal is (dy, −dx)·sign.
            for k in 0..3 {
                let (from, to) = (p.corners[k], p.corners[(k + 1) % 3]);
                let n = Vec3::new(to.y - from.y, from.x - to.x, 0.0) * p.sign;
                let length = n.length();
                if length == 0.0 {
                    continue;
                }
                let n = n * (1.0 / length);
                let d = n.dot(from);
                piece.clip(n, d - n.dot(c), plane_tolerance(n, d, c, side));
                if piece.is_empty() {
                    break;
                }
            }
            if piece.is_empty() {
                continue;
            }
            // Below the triangle's plane: its normal turned to face up.
            let [a, bb, cc] = p.corners;
            let m = (bb - a).cross(cc - a) * p.sign;
            let m = m * (1.0 / m.length());
            let d = m.dot(a);
            piece.clip(m, d - m.dot(c), plane_tolerance(m, d, c, side));
            if !piece.is_empty() {
                out.push((p.sign, piece));
            }
        }
        out
    }
}

impl Binned<'_> {
    /// The boundary of the mesh's part of the cell `b` as triangles in the frame of `b`'s centre,
    /// anticlockwise seen from outside the piece ([`CellClip::surface`]): the mesh's own triangles
    /// clipped to the cell, and on each of the cell's six walls the part of the wall inside the
    /// body (its *cap*).
    ///
    /// A cap is the wall's square cut by the segments where the mesh crosses the wall's plane, in
    /// slabs between the segments' ends: within a slab no two segments cross (a closed mesh's
    /// section is closed curves that do not meet), so each of the slab's intervals between them is in
    /// the body or out of it throughout, told by asking the mesh at its middle. What is
    /// taken is the limit from inside the cell (the segments of triangles that reach into the
    /// cell's side of the plane; the question a hair inside it), so a mesh face lying on a wall
    /// counts once, as the cap, and only when the body is on the cell's side.
    fn surface_of(&self, b: &Aabb) -> Vec<[Vec3; 3]> {
        let c = b.centre();
        let side = b.size().x.max(b.size().y).max(b.size().z);
        let tolerance = 64.0 * f64::EPSILON * (side + c.x.abs().max(c.y.abs()).max(c.z.abs()));
        let mesh = self._mesh;
        let near = |t: &Aabb| {
            t.lo.x <= b.hi.x
                && t.hi.x >= b.lo.x
                && t.lo.y <= b.hi.y
                && t.hi.y >= b.lo.y
                && t.lo.z <= b.hi.z
                && t.hi.z >= b.lo.z
        };
        let mut candidates: Vec<[Vec3; 3]> = Vec::new();
        mesh.each_triangle(&|t| near(t), &mut |t| {
            candidates.push(mesh.triangles[t].map(|i| mesh.vertices[i as usize]));
        });
        let lo = b.lo.to_array();
        let hi = b.hi.to_array();
        let mut out = Vec::new();
        // The mesh's own triangles in the cell: each clipped by the six walls; one lying on a wall
        // is the cap's to draw.
        for t in &candidates {
            let on_wall = (0..3).any(|a| {
                [lo[a], hi[a]].iter().any(|&w| t.iter().all(|p| (p.to_array()[a] - w).abs() <= tolerance))
            });
            if on_wall {
                continue;
            }
            let mut polygon: Vec<Vec3> = t.to_vec();
            for a in 0..3 {
                for (w, keep_above) in [(lo[a], true), (hi[a], false)] {
                    polygon = clip_polygon(&polygon, a, w, keep_above);
                    if polygon.len() < 3 {
                        break;
                    }
                }
                if polygon.len() < 3 {
                    break;
                }
            }
            for k in 1..polygon.len().saturating_sub(1) {
                out.push([polygon[0] - c, polygon[k] - c, polygon[k + 1] - c]);
            }
        }
        // The caps.
        for a in 0..3 {
            let (u, v) = ((a + 1) % 3, (a + 2) % 3);
            for (w, inward) in [(lo[a], 1.0), (hi[a], -1.0)] {
                let segments = wall_segments(&candidates, a, w, inward, tolerance);
                let square = ([lo[u], hi[u]], [lo[v], hi[v]]);
                let inside = |pu: f64, pv: f64| {
                    let mut p = [0.0; 3];
                    p[a] = w + inward * 1e-9 * side;
                    p[u] = pu;
                    p[v] = pv;
                    super::super::shape::Shape::contains(mesh, Vec3::from_array(p))
                };
                for quad in cap(&segments, square, &inside) {
                    // Each corner back in 3-D; wound so its normal points out of the cell.
                    let corner = |q: [f64; 2]| {
                        let mut p = [0.0; 3];
                        p[a] = w;
                        p[u] = q[0];
                        p[v] = q[1];
                        Vec3::from_array(p) - c
                    };
                    let [q0, q1, q2, q3] = quad.map(corner);
                    let mut outward = [0.0; 3];
                    outward[a] = -inward;
                    let outward = Vec3::from_array(outward);
                    for tri in [[q0, q1, q2], [q0, q2, q3]] {
                        let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]);
                        if n.length() <= tolerance * side {
                            continue;
                        }
                        out.push(if n.dot(outward) > 0.0 { tri } else { [tri[0], tri[2], tri[1]] });
                    }
                }
            }
        }
        out
    }
}

/// The part of `polygon` on one side of the plane `x_axis = w`: above it (`keep_above`) or below.
fn clip_polygon(polygon: &[Vec3], axis: usize, w: f64, keep_above: bool) -> Vec<Vec3> {
    let s = |p: &Vec3| {
        let d = p.to_array()[axis] - w;
        if keep_above { d } else { -d }
    };
    let mut out = Vec::with_capacity(polygon.len() + 2);
    for k in 0..polygon.len() {
        let (p, q) = (polygon[k], polygon[(k + 1) % polygon.len()]);
        let (sp, sq) = (s(&p), s(&q));
        if sp >= 0.0 {
            out.push(p);
        }
        if (sp > 0.0 && sq < 0.0) || (sp < 0.0 && sq > 0.0) {
            out.push(p + (q - p) * (sp / (sp - sq)));
        }
    }
    out
}

/// Where the triangles cross the plane `x_axis = w`, as segments in the plane's coordinates
/// `(u, v)` (the two other axes in cyclic order): the limit from the side `inward` points to, so a
/// triangle counts only if it reaches into that side.
fn wall_segments(
    triangles: &[[Vec3; 3]],
    axis: usize,
    w: f64,
    inward: f64,
    tolerance: f64,
) -> Vec<[[f64; 2]; 2]> {
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let mut out = Vec::new();
    for t in triangles {
        let p = t.map(|p| p.to_array());
        let d = p.map(|p| {
            let d = inward * (p[axis] - w);
            if d.abs() <= tolerance { 0.0 } else { d }
        });
        if !d.iter().any(|&d| d > 0.0) || !d.iter().any(|&d| d <= 0.0) {
            continue;
        }
        let mut points: Vec<[f64; 2]> = Vec::new();
        for k in 0..3 {
            if d[k] == 0.0 {
                points.push([p[k][u], p[k][v]]);
            }
            let j = (k + 1) % 3;
            if (d[k] > 0.0 && d[j] < 0.0) || (d[k] < 0.0 && d[j] > 0.0) {
                let s = d[k] / (d[k] - d[j]);
                points.push([p[k][u] + (p[j][u] - p[k][u]) * s, p[k][v] + (p[j][v] - p[k][v]) * s]);
            }
        }
        if points.len() == 2 && points[0] != points[1] {
            out.push([points[0], points[1]]);
        }
    }
    out
}

/// The part of the square `([u0, u1], [v0, v1])` inside the body as quads `[(u, v); 4]`: slabs
/// between the segments' ends, each cut by the segments that span it into intervals in and out by
/// turns, each told by `inside` at its middle.
fn cap(
    segments: &[[[f64; 2]; 2]],
    ([u0, u1], [v0, v1]): ([f64; 2], [f64; 2]),
    inside: &dyn Fn(f64, f64) -> bool,
) -> Vec<[[f64; 2]; 4]> {
    // Each segment clipped to the square (Liang–Barsky), left end first.
    let mut lines: Vec<[[f64; 2]; 2]> = Vec::new();
    let mut cuts: Vec<f64> = vec![u0, u1];
    for s in segments {
        let (p, q) = (s[0], s[1]);
        let d = [q[0] - p[0], q[1] - p[1]];
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        let mut keep = true;
        for (dp, lo, hi, x) in [(d[0], u0, u1, p[0]), (d[1], v0, v1, p[1])] {
            if dp == 0.0 {
                if x < lo || x > hi {
                    keep = false;
                }
                continue;
            }
            let (a, b) = ((lo - x) / dp, (hi - x) / dp);
            let (a, b) = if a < b { (a, b) } else { (b, a) };
            t0 = t0.max(a);
            t1 = t1.min(b);
        }
        if !keep || t0 >= t1 {
            continue;
        }
        let at = |t: f64| [p[0] + d[0] * t, p[1] + d[1] * t];
        let (a, b) = (at(t0), at(t1));
        // Every end is where a slab begins (a segment along v too: the body changes across it);
        // only a segment that runs along u bounds intervals within a slab.
        cuts.extend([a[0], b[0]]);
        if a[0] != b[0] {
            lines.push(if a[0] < b[0] { [a, b] } else { [b, a] });
        }
    }
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    let mut out = Vec::new();
    for pair in cuts.windows(2) {
        let (ua, ub) = (pair[0], pair[1]);
        if ub - ua <= 1e-12 * (u1 - u0) {
            continue;
        }
        let um = 0.5 * (ua + ub);
        let at =
            |l: &[[f64; 2]; 2], u: f64| l[0][1] + (l[1][1] - l[0][1]) * (u - l[0][0]) / (l[1][0] - l[0][0]);
        // The lines that span the slab, top first (by their height at its middle).
        let mut spanning: Vec<&[[f64; 2]; 2]> =
            lines.iter().filter(|l| l[0][0] <= ua && l[1][0] >= ub).collect();
        spanning.sort_by(|a, b| at(b, um).total_cmp(&at(a, um)));
        // Bounds from the top: the square's top, the lines, the square's bottom.
        let mut bounds: Vec<([f64; 2], f64)> = vec![([v1, v1], v1)];
        for l in &spanning {
            bounds.push(([at(l, ua), at(l, ub)], at(l, um)));
        }
        bounds.push(([v0, v0], v0));
        // Each interval between two bounds is in the body or out of it throughout (no segment
        // crosses it): asked at its middle, which lies on no segment. (Counting turns from the
        // top was wrong where a segment runs along the square's edge: a face on a wall's edge.)
        for k in 0..bounds.len() - 1 {
            let (top, bottom) = (bounds[k].1, bounds[k + 1].1);
            if top - bottom > 1e-12 * (v1 - v0) && inside(um, 0.5 * (top + bottom)) {
                let (top, bottom) = (bounds[k].0, bounds[k + 1].0);
                out.push([[ua, top[0]], [ub, top[1]], [ub, bottom[1]], [ua, bottom[0]]]);
            }
        }
    }
    out
}
