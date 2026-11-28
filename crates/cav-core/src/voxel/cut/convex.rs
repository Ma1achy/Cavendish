//! Convex pieces: a cell clipped by half-spaces, and the clippers of the convex shapes.

use crate::Vec3;

use super::super::shape::{Cuboid, Polyhedron};
use super::super::{Aabb, Grid, VoxelError};
use super::{Borrowed, CLIP_WORK_PER_PLANE, CellClip, Clip, Raw, Stateless};

/// A convex polyhedron as its corners and its faces, each face a loop of corner indices
/// anticlockwise seen from outside. Built from a cell's box ([`Self::from_box`]) and cut down by
/// half-spaces ([`Self::clip`]); in whatever frame its box was given (the clippers use the
/// cell's centre as origin, metres).
///
/// Corners no face uses may remain after clipping; they do not count. A face is at least three
/// corners; a piece clipped to nothing has no faces.
#[derive(Clone, Debug, PartialEq)]
pub struct Convex {
    /// Corners, metres.
    pub vertices: Vec<Vec3>,
    /// Faces, each a loop of indices into `vertices`, anticlockwise seen from outside.
    pub faces: Vec<Vec<u32>>,
}

impl Convex {
    /// The box from `lo` to `hi` (metres): eight corners, six faces.
    #[must_use]
    pub fn from_box(lo: Vec3, hi: Vec3) -> Self {
        let b = Aabb::new(lo, hi);
        let vertices = (0..8).map(|i| b.corner(i)).collect();
        // Corner i: bit 0 is +x, bit 1 +y, bit 2 +z. Each face anticlockwise seen from outside.
        let faces = vec![
            vec![0, 4, 6, 2], // −x
            vec![1, 3, 7, 5], // +x
            vec![0, 1, 5, 4], // −y
            vec![2, 6, 7, 3], // +y
            vec![0, 2, 3, 1], // −z
            vec![4, 5, 7, 6], // +z
        ];
        Self { vertices, faces }
    }

    /// Whether it has been clipped to nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// Keeps the part where `n·p ≤ d` (`n` need not be a unit vector; `d` in the same units as
    /// `n·p`). Signed distances within `tolerance` (the same units) of the plane are taken as on
    /// it, so a plane through a corner, an edge or a face makes no sliver.
    ///
    /// When no corner is strictly outside nothing changes; when none is strictly inside the piece
    /// becomes empty. Otherwise each face is clipped (Sutherland–Hodgman), each edge crossing the
    /// plane gets one new corner (made once, from its two ends in index order, so both faces of
    /// the edge share its bits), and the cap is the new corners and those on the plane, in order
    /// about the plane's normal.
    pub fn clip(&mut self, n: Vec3, d: f64, tolerance: f64) {
        if self.faces.is_empty() {
            return;
        }
        let s: Vec<f64> = self
            .vertices
            .iter()
            .map(|v| {
                let s = n.dot(*v) - d;
                if s.abs() <= tolerance { 0.0 } else { s }
            })
            .collect();
        let used = |f: &dyn Fn(f64) -> bool| self.faces.iter().flatten().any(|&i| f(s[i as usize]));
        if !used(&|s| s > 0.0) {
            return;
        }
        if !used(&|s| s < 0.0) {
            self.faces.clear();
            return;
        }
        // The corner made on each crossing edge, by its ends in index order.
        let mut made: Vec<([u32; 2], u32)> = Vec::new();
        let mut cap: Vec<u32> = Vec::new();
        let mut faces = Vec::with_capacity(self.faces.len() + 1);
        for face in &self.faces {
            let mut out: Vec<u32> = Vec::with_capacity(face.len() + 1);
            for k in 0..face.len() {
                let (i, j) = (face[k], face[(k + 1) % face.len()]);
                let (si, sj) = (s[i as usize], s[j as usize]);
                if si <= 0.0 {
                    out.push(i);
                    if si == 0.0 && !cap.contains(&i) {
                        cap.push(i);
                    }
                }
                if (si < 0.0 && sj > 0.0) || (si > 0.0 && sj < 0.0) {
                    let ends = if i < j { [i, j] } else { [j, i] };
                    let at = if let Some(&(_, m)) = made.iter().find(|(e, _)| *e == ends) {
                        m
                    } else {
                        let (a, b) = (ends[0] as usize, ends[1] as usize);
                        let t = s[a] / (s[a] - s[b]);
                        let p = self.vertices[a] + (self.vertices[b] - self.vertices[a]) * t;
                        self.vertices.push(p);
                        let m = (self.vertices.len() - 1) as u32;
                        made.push((ends, m));
                        cap.push(m);
                        m
                    };
                    out.push(at);
                }
            }
            if out.len() >= 3 {
                faces.push(out);
            }
        }
        if cap.len() >= 3 {
            // In order about n: angles in the plane's basis (u, w = n̂ × u), anticlockwise seen
            // from outside (n points out of the kept part).
            let centre =
                cap.iter().fold(Vec3::ZERO, |c, &i| c + self.vertices[i as usize]) * (1.0 / cap.len() as f64);
            let nh = n.normalised();
            let far = cap
                .iter()
                .map(|&i| self.vertices[i as usize] - centre)
                .fold(Vec3::ZERO, |a, r| if r.dot(r) > a.dot(a) { r } else { a });
            let u = far.normalised();
            let w = nh.cross(u);
            let angle = |i: u32| {
                let r = self.vertices[i as usize] - centre;
                r.dot(w).atan2(r.dot(u))
            };
            cap.sort_by(|&a, &b| angle(a).total_cmp(&angle(b)));
            faces.push(cap);
        }
        self.faces = faces;
    }

    /// The moments about the frame's origin, filled at unit density ([`Raw`]): a fan of signed
    /// tetrahedra on each face, from the mean of the corners in use.
    #[must_use]
    pub fn raw(&self) -> Raw {
        let mut out = Raw::default();
        if self.faces.is_empty() {
            return out;
        }
        let corners = self.faces.iter().map(Vec::len).sum::<usize>() as f64;
        let o = self.faces.iter().flatten().fold(Vec3::ZERO, |c, &i| c + self.vertices[i as usize])
            * (1.0 / corners);
        for face in &self.faces {
            let first = self.vertices[face[0] as usize];
            for t in 1..face.len() - 1 {
                out.add_tetrahedron([
                    o,
                    first,
                    self.vertices[face[t] as usize],
                    self.vertices[face[t + 1] as usize],
                ]);
            }
        }
        out
    }
}

/// `∫ x^k` over `[a, b]`, for `k` from 0 to 3.
fn powers(a: f64, b: f64) -> [f64; 4] {
    [b - a, 0.5 * (b * b - a * a), (b * b * b - a * a * a) / 3.0, (b.powi(4) - a.powi(4)) / 4.0]
}

/// The moments of the box from `lo` to `hi` about the origin, filled at unit density, in closed
/// form.
#[must_use]
pub fn box_raw(lo: Vec3, hi: Vec3) -> Raw {
    let (x, y, z) = (powers(lo.x, hi.x), powers(lo.y, hi.y), powers(lo.z, hi.z));
    Raw::from_monomials(|e| x[e[0]] * y[e[1]] * z[e[2]])
}

/// The tolerance on a plane's signed distance in a cell's frame: the rounding of moving the
/// plane there (`d` and the cell's centre about the body's size) and of the cell's own corners.
pub(crate) fn plane_tolerance(n: Vec3, d: f64, centre: Vec3, side: f64) -> f64 {
    8.0 * f64::EPSILON * (d.abs() + n.dot(centre).abs() + side)
}

impl Stateless for Cuboid {
    fn work(&self, _boxed: &Aabb) -> u64 {
        CLIP_WORK_PER_PLANE
    }

    /// The intersection of two boxes, in closed form.
    fn clip(&self, b: &Aabb) -> Raw {
        let c = b.centre();
        let lo = self.centre - self.half;
        let hi = self.centre + self.half;
        let lo = Vec3::new(lo.x.max(b.lo.x), lo.y.max(b.lo.y), lo.z.max(b.lo.z)) - c;
        let hi = Vec3::new(hi.x.min(b.hi.x), hi.y.min(b.hi.y), hi.z.min(b.hi.z)) - c;
        if lo.x >= hi.x || lo.y >= hi.y || lo.z >= hi.z {
            return Raw::default();
        }
        box_raw(lo, hi)
    }
}

impl Clip for Cuboid {
    fn prepare(&self, _grid: &Grid) -> Result<Box<dyn CellClip + '_>, VoxelError> {
        Ok(Box::new(Borrowed(self)))
    }
}

impl Polyhedron {
    /// The piece of `b` inside the polyhedron, in the frame of `b`'s centre (metres).
    #[must_use]
    pub fn piece_of(&self, b: &Aabb) -> Convex {
        let c = b.centre();
        let mut piece = Convex::from_box(b.lo - c, b.hi - c);
        let side = b.size().x.max(b.size().y).max(b.size().z);
        for &(n, d) in &self.planes {
            let local = d - n.dot(c);
            piece.clip(n, local, plane_tolerance(n, d, c, side));
            if piece.is_empty() {
                break;
            }
        }
        piece
    }
}

impl Stateless for Polyhedron {
    fn work(&self, _boxed: &Aabb) -> u64 {
        CLIP_WORK_PER_PLANE * self.planes.len() as u64
    }

    fn clip(&self, b: &Aabb) -> Raw {
        self.piece_of(b).raw()
    }
}

impl Clip for Polyhedron {
    fn prepare(&self, _grid: &Grid) -> Result<Box<dyn CellClip + '_>, VoxelError> {
        Ok(Box::new(Borrowed(self)))
    }
}
