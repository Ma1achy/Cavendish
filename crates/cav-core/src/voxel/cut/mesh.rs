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
use super::{CLIP_WORK_PER_PLANE, CellClip, Clip, Raw};

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
        4 * CLIP_WORK_PER_PLANE * self.bin(cell).len() as u64
    }

    fn clip(&self, cell: [u32; 3], b: &Aabb) -> Raw {
        let c = b.centre();
        let side = b.size().x.max(b.size().y).max(b.size().z);
        let mut out = Raw::default();
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
                out = out.plus(p.sign, &piece.raw());
            }
        }
        out
    }
}
