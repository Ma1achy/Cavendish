//! Cut cells: the exact piece of a shape inside each boundary cell, and its moments (S5,
//! `docs/slices/S5.md` §2 "Cut-cell moments"; the method is `docs/research/exact-skin-and-partial-fill.md`
//! §3.1 and §5, cited there as [X]).
//!
//! A body voxelised the S3 way ([`super::voxelise`]) weighs a boundary cell by how many of its
//! `k³` sample points are inside: first order in the pitch on faces along the lattice. Here a
//! boundary cell is **clipped exactly** instead, and keeps its piece's volume, centroid, and second
//! and third central moments ([`Piece`]). The tree's leaves are the same kind of thing as before
//! (full cubes inside, one leaf a boundary cell) and the pieces sit beside them in a sorted side
//! table, [`VoxTree::cuts`](super::VoxTree::cuts); a cut leaf weighs `density × volume`, and the
//! node moments are built from the pieces, so the multipoles a reader takes are the true body's.
//!
//! **The clippers** ([`Clip`], one for each kind of shape):
//! - convex polyhedra ([`super::shape::Polyhedron`], and so the Platonic solids): the cell cube
//!   clipped by each face's half-space ([`Convex::clip`], Sutherland–Hodgman on a polyhedron);
//!   [`super::shape::Cuboid`]: the intersection of two boxes, in closed form;
//! - closed meshes ([`super::mesh::Mesh`]): the signed vertical-prism decomposition. For a closed
//!   mesh the indicator at `x` is the number of triangles above `x` facing up less the number
//!   facing down, so `cell ∩ body = Σ_t sign(n_z) · (cell ∩ prism below t)`, each term the cube
//!   clipped by four half-spaces (the triangle's three vertical side planes and its own plane). The
//!   triangles that can take part are found from a 2-D bin of their shadows;
//! - the curved solids ([`super::shape::Sphere`], `Ellipsoid` by scaling, `Shell` as a difference
//!   of two balls, `Cylinder` by an axis permutation): the z-chord integrated exactly and `(x, y)`
//!   by Gauss–Legendre on pieces split at every kink, with a cosine map at each end ([X] §3.1's
//!   `analytic.py`, ported). No mesh anywhere.
//!
//! **What the pieces are for.** [`Representation`] chooses how a reader takes a cut cell
//! ([`super::kernel::TreeBody::read_as`]): at the cell's centre (`Fill`), at the piece's centroid
//! (`Centroid`), or as a point with the piece's quadrupole (`Second`). `Sampled` is S3's voxeliser,
//! unchanged to the bit, and makes no cuts.
//!
//! Frames and units: a cell and its piece are in the body's frame, metres; moments of a piece are
//! of the piece filled at unit density (m³, m⁴, m⁵, m⁶). Clipping work is counted against the
//! voxeliser's [`Budget::work`] before it is done, and the table's bytes against [`Budget::bytes`]
//! before each entry is pushed.

mod analytic;
mod convex;
mod mesh;
#[cfg(test)]
mod tests;

pub use convex::Convex;

use crate::Vec3;

use super::morton::{self};
use super::shape::{Cover, Shape};
use super::tree::{Leaf, THIRD, VoxTree, second_index, third_index};
use super::{Aabb, Budget, Grid, VoxelError};

/// How a reader takes a body's boundary cells (`docs/slices/S5.md` §2 "Reading a cut cell").
///
/// Only `Sampled` reads a tree made by [`super::voxelise`]; the others read a tree made by
/// [`voxelise_cut`], whose boundary cells carry their exact pieces. Interior cells are full cubes
/// in every case, read by the cube kernel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Representation {
    /// S3's: each boundary cell a full cube at its centre, weighed by its sampled count. The
    /// default; makes no cuts.
    #[default]
    Sampled,
    /// The exact fill: a cube of the cell at its centre, of the piece's mass. Second order on
    /// faces along the lattice.
    Fill,
    /// A cube of the cell's side at the piece's centroid, of the piece's mass. Third order.
    Centroid,
    /// A point at the piece's centroid with the piece's quadrupole (its second central moment);
    /// within [`super::kernel::PRISM_WITHIN`] cells of the point asked about, the `Centroid` cube
    /// (which is finite inside). Order about four.
    Second,
}

impl Representation {
    /// Every one, in the order of the ladder.
    pub const ALL: [Self; 4] = [Self::Sampled, Self::Fill, Self::Centroid, Self::Second];

    /// Whether the body's tree must carry exact pieces ([`voxelise_cut`]): every one but `Sampled`.
    #[must_use]
    pub const fn cuts(self) -> bool {
        !matches!(self, Self::Sampled)
    }
}

/// A piece below this fraction of its cell's volume is dropped, and one within it of the whole
/// cell is taken as a full cell (no cut entry): what the clippers' rounding cannot tell apart from
/// nothing or from everything. It moves a body's mass by at most this fraction of a cell a
/// boundary cell.
pub const CUT_SNAP: f64 = 1e-13;

/// The moments of a solid about a local origin, filled at unit density: `∫ 1`, `∫ x_a`,
/// `∫ x_a x_b` (in [`super::Moments::second`]'s order xx, yy, zz, xy, xz, yz) and `∫ x_a x_b x_c`
/// (in [`super::Moments::third`]'s order), m³ to m⁶. What a clipper returns, about the cell's
/// centre (so that nothing is large and nothing cancels); they add, so a signed sum of pieces is
/// the sum of their moments.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Raw {
    /// m³.
    pub volume: f64,
    /// m⁴.
    pub first: [f64; 3],
    /// m⁵.
    pub second: [f64; 6],
    /// m⁶.
    pub third: [f64; 10],
}

impl Raw {
    /// The moments whose monomial `x^e₀ y^e₁ z^e₂` integrates to `integral(e)`.
    pub(crate) fn from_monomials(integral: impl Fn([usize; 3]) -> f64) -> Self {
        let unit = |a: usize| {
            let mut e = [0; 3];
            e[a] = 1;
            e
        };
        let add = |a: [usize; 3], b: [usize; 3]| [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
        let mut out = Self { volume: integral([0; 3]), ..Self::default() };
        for a in 0..3 {
            out.first[a] = integral(unit(a));
            for b in a..3 {
                out.second[second_index(a, b)] = integral(add(unit(a), unit(b)));
            }
        }
        for (a, b, c) in THIRD {
            out.third[third_index(a, b, c)] = integral(add(add(unit(a), unit(b)), unit(c)));
        }
        out
    }

    /// `self + sign × other`.
    #[must_use]
    pub fn plus(mut self, sign: f64, other: &Self) -> Self {
        self.volume += sign * other.volume;
        for a in 0..3 {
            self.first[a] += sign * other.first[a];
        }
        for i in 0..6 {
            self.second[i] += sign * other.second[i];
        }
        for i in 0..10 {
            self.third[i] += sign * other.third[i];
        }
        self
    }

    /// Adds the signed tetrahedron `q` (corners about the same origin, metres): its volume is
    /// `(q₁ − q₀)·((q₂ − q₀) × (q₃ − q₀))/6`, positive when `q₀` lies inside a face `q₁ q₂ q₃`
    /// wound anticlockwise seen from outside. The moments are the Dirichlet moments of
    /// the barycentric coordinates: with `S = Σ q_i`, `∫x_a = V S_a/4`,
    /// `∫x_a x_b = V/20 (Σ q_a q_b + S_a S_b)`, `∫x_a x_b x_c = V/120 (S_a S_b S_c + Σ(q_a q_b S_c +
    /// q_a q_c S_b + q_b q_c S_a) + 2 Σ q_a q_b q_c)`.
    pub(crate) fn add_tetrahedron(&mut self, q: [Vec3; 4]) {
        let v = (q[1] - q[0]).dot((q[2] - q[0]).cross(q[3] - q[0])) / 6.0;
        if v == 0.0 {
            return;
        }
        let p = q.map(Vec3::to_array);
        let s = [0, 1, 2].map(|a| p[0][a] + p[1][a] + p[2][a] + p[3][a]);
        self.volume += v;
        for a in 0..3 {
            self.first[a] += v * s[a] / 4.0;
            for b in a..3 {
                let own: f64 = p.iter().map(|q| q[a] * q[b]).sum();
                self.second[second_index(a, b)] += v / 20.0 * (own + s[a] * s[b]);
            }
        }
        for (a, b, c) in THIRD {
            let mut sum = s[a] * s[b] * s[c];
            for q in &p {
                sum +=
                    q[a] * q[b] * s[c] + q[a] * q[c] * s[b] + q[b] * q[c] * s[a] + 2.0 * q[a] * q[b] * q[c];
            }
            self.third[third_index(a, b, c)] += v / 120.0 * sum;
        }
    }

    /// The piece these moments describe, their origin at `origin` (metres, the body's frame):
    /// its centroid and central moments.
    #[must_use]
    pub fn piece(&self, origin: Vec3) -> Piece {
        let v = self.volume;
        let c = self.first.map(|m| m / v);
        let mut second = [0.0; 6];
        for a in 0..3 {
            for b in a..3 {
                let i = second_index(a, b);
                second[i] = self.second[i] - v * c[a] * c[b];
            }
        }
        let mut third = [0.0; 10];
        for (a, b, cc) in THIRD {
            let m2 = |x: usize, y: usize| self.second[second_index(x, y)];
            third[third_index(a, b, cc)] =
                self.third[third_index(a, b, cc)] - c[a] * m2(b, cc) - c[b] * m2(a, cc) - c[cc] * m2(a, b)
                    + 2.0 * v * c[a] * c[b] * c[cc];
        }
        Piece { volume: v, centroid: origin + Vec3::from_array(c), second, third }
    }
}

/// A shape's piece of one cell, filled at unit density: volume (m³), centroid (metres, the body's
/// frame), central second moments (m⁵, xx, yy, zz, xy, xz, yz) and central third moments (m⁶, in
/// [`super::Moments::third`]'s order).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    /// m³; above 0 and below the cell's volume (by [`CUT_SNAP`]).
    pub volume: f64,
    /// Metres, the body's frame.
    pub centroid: Vec3,
    /// `∫ (x_a − c_a)(x_b − c_b) dV`, m⁵.
    pub second: [f64; 6],
    /// `∫ (x_a − c_a)(x_b − c_b)(x_c − c_c) dV`, m⁶.
    pub third: [f64; 10],
}

/// One entry of a tree's `cuts`: the piece of the finest cell whose Morton key is `key` (a leaf of
/// level 0 of the same key). 168 bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CutCell {
    /// The cell's Morton key.
    pub key: u32,
    /// Its piece.
    pub piece: Piece,
}

/// A shape that can be clipped by cells exactly. Each kind of shape has its own; a new kind of
/// shape needs one for any representation but `Sampled` (`docs/extending/a-shape.md`).
pub trait Clip: Sync {
    /// The clipper for the cells of `grid`: what it needs made once (a mesh's 2-D bin of its
    /// triangles' shadows) is made here.
    ///
    /// # Errors
    /// When what it needs would pass its cap, in words.
    fn prepare(&self, grid: &Grid) -> Result<Box<dyn CellClip + '_>, VoxelError>;
}

/// A clipper prepared for one grid ([`Clip::prepare`]).
pub trait CellClip {
    /// The work clipping the finest cell `cell` (its box `boxed`, metres, the body's frame) will
    /// take, in the voxeliser's units (about one sample test each), counted before it is done.
    fn work(&self, cell: [u32; 3], boxed: &Aabb) -> u64;
    /// The moments of the shape's part of `boxed` about `boxed`'s centre ([`Raw`]).
    fn clip(&self, cell: [u32; 3], boxed: &Aabb) -> Raw;
}

/// A clipper that needs nothing prepared: the shape itself.
pub(crate) struct Borrowed<'a, T>(pub &'a T);

/// A clipper that needs nothing made for the grid.
pub(crate) trait Stateless: Sync {
    /// As [`CellClip::work`], without the cell's index.
    fn work(&self, boxed: &Aabb) -> u64;
    /// As [`CellClip::clip`].
    fn clip(&self, boxed: &Aabb) -> Raw;
}

impl<T: Stateless> CellClip for Borrowed<'_, T> {
    fn work(&self, _cell: [u32; 3], boxed: &Aabb) -> u64 {
        self.0.work(boxed)
    }
    fn clip(&self, _cell: [u32; 3], boxed: &Aabb) -> Raw {
        self.0.clip(boxed)
    }
}

/// What a cut body weighs: its whole mass (a uniform body: the density is the mass over the exact
/// volume voxelised) or its density (a body of one material).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Weight {
    /// Kilograms, the whole body.
    Mass(f64),
    /// Kilograms per cubic metre.
    Density(f64),
}

/// `shape` on `grid` as a tree whose boundary cells carry their exact pieces ([`VoxTree::cuts`]),
/// every leaf of material `material`.
///
/// From the root down, as [`super::voxelise`]: a block the shape covers wholly (asked about the
/// block's own box, not its sample points) is one full leaf, a block it misses is dropped, a mixed
/// block is split, and at the finest level a mixed cell is clipped. A piece is a cut leaf (its
/// `count` the fill quantised to `k³` levels, at least 1: for the picture only); a piece within
/// [`CUT_SNAP`] of nothing is dropped and of the whole cell is a full leaf. Eight full leaves
/// with no cut among them are merged.
///
/// # Errors
/// When the shape has no clipper ([`Shape::clip`]); when the work (box tests and clipping) would
/// pass `budget.work`, the leaves `budget.leaves`, or the leaves and cuts together
/// `budget.bytes`, with the numbers; as [`VoxTree::build_cut`]; when nothing of the shape is in
/// the grid; when the weight is not a positive number.
pub fn voxelise_cut(
    shape: &dyn Shape,
    grid: &Grid,
    weight: Weight,
    material: u8,
    budget: Budget,
) -> Result<VoxTree, VoxelError> {
    let Some(clip) = shape.clip() else {
        return Err(VoxelError(
            "this shape has no cut-cell clipper: read it by its samples (Sampled)".into(),
        ));
    };
    let w = match weight {
        Weight::Mass(m) | Weight::Density(m) => m,
    };
    if !(w.is_finite() && w > 0.0) {
        return Err(VoxelError(format!("{weight:?} must be a positive number")));
    }
    let clipper = clip.prepare(grid)?;
    let mut walk = Walk {
        shape,
        clipper: clipper.as_ref(),
        grid,
        material,
        leaves: Vec::new(),
        cuts: Vec::new(),
        work: 0,
        budget,
    };
    walk.visit(0, grid.depth)?;
    let (leaves, cuts) = (walk.leaves, walk.cuts);
    let h3 = grid.pitch.powi(3);
    let full_cells: u64 = leaves
        .iter()
        .filter(|l| cuts.binary_search_by_key(&l.key, |c: &CutCell| c.key).is_err() || l.level > 0)
        .map(|l| u64::from(morton::span(l.level)))
        .sum();
    let volume = full_cells as f64 * h3 + cuts.iter().map(|c| c.piece.volume).sum::<f64>();
    let density = match weight {
        Weight::Mass(m) => m / volume,
        Weight::Density(rho) => rho,
    };
    VoxTree::build_cut(*grid, leaves, cuts, density, budget)
}

/// The clip work a convex piece costs a half-space, in the voxeliser's units.
pub(crate) const CLIP_WORK_PER_PLANE: u64 = 8;

struct Walk<'a> {
    shape: &'a dyn Shape,
    clipper: &'a dyn CellClip,
    grid: &'a Grid,
    material: u8,
    leaves: Vec<Leaf>,
    cuts: Vec<CutCell>,
    work: u64,
    budget: Budget,
}

impl Walk<'_> {
    fn spend(&mut self, work: u64) -> Result<(), VoxelError> {
        self.work = self.work.saturating_add(work);
        if self.work > self.budget.work {
            return Err(VoxelError(format!(
                "voxelising with exact cut cells would take more than {} box tests and clipping: use a \
                 coarser pitch",
                self.budget.work
            )));
        }
        Ok(())
    }

    /// Pushes a leaf (and its cut), refusing first what would pass the caps.
    fn emit(&mut self, leaf: Leaf, cut: Option<Piece>) -> Result<(), VoxelError> {
        if self.leaves.len() >= self.budget.leaves {
            return Err(VoxelError(format!(
                "the tree would have more than {} leaves: use a coarser pitch",
                self.budget.leaves
            )));
        }
        let cuts = self.cuts.len() + usize::from(cut.is_some());
        let bytes = (self.leaves.len() + 1) * size_of::<Leaf>() + cuts * size_of::<CutCell>();
        if bytes > self.budget.bytes {
            return Err(VoxelError(format!(
                "the tree's leaves and cut cells alone would pass the {} bytes allowed it: use a coarser \
                 pitch",
                self.budget.bytes
            )));
        }
        if let Some(piece) = cut {
            self.cuts.push(CutCell { key: leaf.key, piece });
        }
        self.leaves.push(leaf);
        Ok(())
    }

    fn visit(&mut self, key: u32, level: u8) -> Result<(), VoxelError> {
        let corner = morton::decode(key);
        if !self.grid.touches_used(corner) {
            return Ok(());
        }
        let side = 1u32 << level;
        self.spend(1)?;
        let boxed = self.grid.block_box(corner, side);
        let full = self.grid.samples_per_cell() as u8;
        let whole = Leaf { key, level, count: full, material: self.material };
        match self.shape.cover(&boxed) {
            Cover::Outside => Ok(()),
            Cover::Inside => self.emit(whole, None),
            Cover::Mixed if level == 0 => {
                self.spend(self.clipper.work(corner, &boxed))?;
                let raw = self.clipper.clip(corner, &boxed);
                let cell = self.grid.pitch.powi(3);
                if !raw.volume.is_finite() {
                    return Err(VoxelError(format!(
                        "the clipper gave cell {corner:?} a volume of {}",
                        raw.volume
                    )));
                }
                if raw.volume <= CUT_SNAP * cell {
                    return Ok(());
                }
                if raw.volume >= (1.0 - CUT_SNAP) * cell {
                    return self.emit(whole, None);
                }
                let k3 = f64::from(full);
                let count = (raw.volume / cell * k3).round().clamp(1.0, k3) as u8;
                self.emit(Leaf { count, ..whole }, Some(raw.piece(boxed.centre())))
            }
            Cover::Mixed => {
                let (before, cuts_before) = (self.leaves.len(), self.cuts.len());
                for octant in 0..8 {
                    self.visit(morton::child(key, level, octant), level - 1)?;
                }
                // Eight full leaves of the level below, none cut, are one leaf.
                let new = &self.leaves[before..];
                if new.len() == 8
                    && self.cuts.len() == cuts_before
                    && new.iter().all(|l| l.level == level - 1 && l.count == full)
                {
                    self.leaves.truncate(before);
                    self.leaves.push(whole);
                }
                Ok(())
            }
        }
    }
}
