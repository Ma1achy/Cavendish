//! What can be voxelised: a [`Shape`] says whether a point is inside it and, conservatively,
//! whether a whole box is.
//!
//! Every shape here is in a body's own frame, metres. The primitives ([`Sphere`], [`Cuboid`],
//! [`Cylinder`], [`Ellipsoid`]) and convex polyhedra ([`Polyhedron`], with the five Platonic solids)
//! answer `cover` from their geometry; meshes are in [`super::mesh`]. Each also gives its volume,
//! centroid and second moments in closed form, which the tests hold the voxelised body to.

use crate::Vec3;

use super::{Aabb, VoxelError};

/// What a shape says about a box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cover {
    /// Every point of the box is inside.
    Inside,
    /// No point of the box is inside.
    Outside,
    /// Some may be and some may not; always a correct answer.
    Mixed,
}

/// A solid that can be voxelised.
///
/// The contract the voxeliser relies on, and its tests hold: [`Self::cover`] is **conservative**
/// with respect to [`Self::contains`]. When it says `Inside`, `contains` is true at every point of
/// the box that it is asked about; when it says `Outside`, false at every such point. A shape
/// that is unsure says `Mixed`. Shapes keep a margin of about 10⁻¹² of their size for rounding.
pub trait Shape: Sync + Send {
    /// A box holding the whole shape, metres, the body's frame.
    fn bounds(&self) -> Aabb;
    /// Whether `point` (metres, the body's frame) is inside, its surface counting as inside.
    fn contains(&self, point: Vec3) -> bool;
    /// What the shape says about every point of `boxed` (see the trait's contract).
    fn cover(&self, boxed: &Aabb) -> Cover;
    /// The volume, cubic metres, in closed form.
    fn volume(&self) -> f64;
    /// The centroid, metres, the body's frame, in closed form.
    fn centroid(&self) -> Vec3;
    /// `∫ (x_a − c_a)(x_b − c_b) dV` about the centroid, m⁵, in the order xx, yy, zz, xy, xz, yz:
    /// the second moments of the shape filled at unit density.
    fn second_moments(&self) -> [f64; 6];
}

/// The relative margin every shape keeps for rounding.
const MARGIN: f64 = 1e-12;

/// The largest absolute coordinate of a box, for scaling margins.
fn reach(b: &Aabb) -> f64 {
    [b.lo.x, b.lo.y, b.lo.z, b.hi.x, b.hi.y, b.hi.z].iter().fold(0f64, |m, v| m.max(v.abs()))
}

fn component(v: Vec3, axis: usize) -> f64 {
    [v.x, v.y, v.z][axis]
}

/// Per axis, the nearest and farthest distance from `c` to the interval `[lo, hi]`.
fn extremes(lo: f64, hi: f64, c: f64) -> (f64, f64) {
    let near = if c < lo {
        lo - c
    } else if c > hi {
        c - hi
    } else {
        0.0
    };
    (near, (lo - c).abs().max((hi - c).abs()))
}

/// A ball.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    /// Its centre, metres.
    pub centre: Vec3,
    /// Its radius, metres; positive.
    pub radius: f64,
}

impl Shape for Sphere {
    fn bounds(&self) -> Aabb {
        Aabb::new(self.centre, self.centre).grown(self.radius)
    }

    fn contains(&self, p: Vec3) -> bool {
        let d = p - self.centre;
        d.dot(d) <= self.radius * self.radius
    }

    fn cover(&self, b: &Aabb) -> Cover {
        let (mut near, mut far) = (0.0, 0.0);
        for axis in 0..3 {
            let (n, f) = extremes(component(b.lo, axis), component(b.hi, axis), component(self.centre, axis));
            near += n * n;
            far += f * f;
        }
        let r2 = self.radius * self.radius;
        let slack = MARGIN * (self.radius + reach(b)).powi(2);
        if far < r2 - slack {
            Cover::Inside
        } else if near > r2 + slack {
            Cover::Outside
        } else {
            Cover::Mixed
        }
    }

    fn volume(&self) -> f64 {
        4.0 / 3.0 * std::f64::consts::PI * self.radius.powi(3)
    }

    fn centroid(&self) -> Vec3 {
        self.centre
    }

    fn second_moments(&self) -> [f64; 6] {
        let s = self.volume() * self.radius * self.radius / 5.0;
        [s, s, s, 0.0, 0.0, 0.0]
    }
}

/// A box with its faces along the axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cuboid {
    /// Its centre, metres.
    pub centre: Vec3,
    /// Half its size along x, y and z, metres; each positive.
    pub half: Vec3,
}

impl Shape for Cuboid {
    fn bounds(&self) -> Aabb {
        Aabb::new(self.centre - self.half, self.centre + self.half)
    }

    fn contains(&self, p: Vec3) -> bool {
        let d = p - self.centre;
        d.x.abs() <= self.half.x && d.y.abs() <= self.half.y && d.z.abs() <= self.half.z
    }

    fn cover(&self, b: &Aabb) -> Cover {
        let slack = MARGIN * (self.half.length() + reach(b));
        let mut inside = true;
        for axis in 0..3 {
            let (c, h) = (component(self.centre, axis), component(self.half, axis));
            let (lo, hi) = (component(b.lo, axis), component(b.hi, axis));
            if hi < c - h - slack || lo > c + h + slack {
                return Cover::Outside;
            }
            inside &= lo > c - h + slack && hi < c + h - slack;
        }
        if inside { Cover::Inside } else { Cover::Mixed }
    }

    fn volume(&self) -> f64 {
        8.0 * self.half.x * self.half.y * self.half.z
    }

    fn centroid(&self) -> Vec3 {
        self.centre
    }

    fn second_moments(&self) -> [f64; 6] {
        let v = self.volume();
        let h = self.half;
        [v * h.x * h.x / 3.0, v * h.y * h.y / 3.0, v * h.z * h.z / 3.0, 0.0, 0.0, 0.0]
    }
}

/// A solid circular cylinder with its axis along x, y or z.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cylinder {
    /// The middle of its axis, metres.
    pub centre: Vec3,
    /// Metres; positive.
    pub radius: f64,
    /// Half its length, metres; positive.
    pub half_length: f64,
    /// Its axis: 0 for x, 1 for y, 2 for z.
    pub axis: usize,
}

impl Cylinder {
    /// The two axes across the cylinder.
    const fn across(&self) -> (usize, usize) {
        match self.axis {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        }
    }
}

impl Shape for Cylinder {
    fn bounds(&self) -> Aabb {
        let mut half = [self.radius; 3];
        half[self.axis] = self.half_length;
        let half = Vec3::from_array(half);
        Aabb::new(self.centre - half, self.centre + half)
    }

    fn contains(&self, p: Vec3) -> bool {
        let d = p - self.centre;
        let (u, v) = self.across();
        let (du, dv) = (component(d, u), component(d, v));
        du * du + dv * dv <= self.radius * self.radius && component(d, self.axis).abs() <= self.half_length
    }

    fn cover(&self, b: &Aabb) -> Cover {
        let slack = MARGIN * (self.radius + self.half_length + reach(b));
        let a = self.axis;
        let (c, h) = (component(self.centre, a), self.half_length);
        let (lo, hi) = (component(b.lo, a), component(b.hi, a));
        if hi < c - h - slack || lo > c + h + slack {
            return Cover::Outside;
        }
        let along_inside = lo > c - h + slack && hi < c + h - slack;
        let (u, v) = self.across();
        let (mut near, mut far) = (0.0, 0.0);
        for axis in [u, v] {
            let (n, f) = extremes(component(b.lo, axis), component(b.hi, axis), component(self.centre, axis));
            near += n * n;
            far += f * f;
        }
        let r2 = self.radius * self.radius;
        let slack2 = MARGIN * (self.radius + reach(b)).powi(2);
        if near > r2 + slack2 {
            Cover::Outside
        } else if along_inside && far < r2 - slack2 {
            Cover::Inside
        } else {
            Cover::Mixed
        }
    }

    fn volume(&self) -> f64 {
        std::f64::consts::PI * self.radius * self.radius * 2.0 * self.half_length
    }

    fn centroid(&self) -> Vec3 {
        self.centre
    }

    fn second_moments(&self) -> [f64; 6] {
        let v = self.volume();
        let mut s = [v * self.radius * self.radius / 4.0; 3];
        s[self.axis] = v * self.half_length * self.half_length / 3.0;
        [s[0], s[1], s[2], 0.0, 0.0, 0.0]
    }
}

/// A solid ellipsoid with its axes along x, y and z.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ellipsoid {
    /// Its centre, metres.
    pub centre: Vec3,
    /// Its semi-axes along x, y and z, metres; each positive.
    pub radii: Vec3,
}

impl Shape for Ellipsoid {
    fn bounds(&self) -> Aabb {
        Aabb::new(self.centre - self.radii, self.centre + self.radii)
    }

    fn contains(&self, p: Vec3) -> bool {
        let d = p - self.centre;
        let r = self.radii;
        (d.x / r.x).powi(2) + (d.y / r.y).powi(2) + (d.z / r.z).powi(2) <= 1.0
    }

    fn cover(&self, b: &Aabb) -> Cover {
        // In units of the semi-axes the ellipsoid is the unit ball and the box is still a box.
        let (mut near, mut far) = (0.0, 0.0);
        for axis in 0..3 {
            let (c, r) = (component(self.centre, axis), component(self.radii, axis));
            let (n, f) = extremes(component(b.lo, axis), component(b.hi, axis), c);
            near += (n / r).powi(2);
            far += (f / r).powi(2);
        }
        let scale = self.radii.x.min(self.radii.y).min(self.radii.z);
        let slack = MARGIN * ((self.radii.length() + reach(b)) / scale).powi(2);
        if far < 1.0 - slack {
            Cover::Inside
        } else if near > 1.0 + slack {
            Cover::Outside
        } else {
            Cover::Mixed
        }
    }

    fn volume(&self) -> f64 {
        4.0 / 3.0 * std::f64::consts::PI * self.radii.x * self.radii.y * self.radii.z
    }

    fn centroid(&self) -> Vec3 {
        self.centre
    }

    fn second_moments(&self) -> [f64; 6] {
        let v = self.volume();
        let r = self.radii;
        [v * r.x * r.x / 5.0, v * r.y * r.y / 5.0, v * r.z * r.z / 5.0, 0.0, 0.0, 0.0]
    }
}

/// The most vertices a convex polyhedron may be given: its hull is found by trying every triple.
pub const MAX_POLYHEDRON_VERTICES: usize = 100;

/// A convex polyhedron: the intersection of half-spaces `n·p ≤ d`, with the faces that bound it.
#[derive(Clone, Debug, PartialEq)]
pub struct Polyhedron {
    /// Each face's outward unit normal and its offset `d`, metres: a point `p` is inside when
    /// `n·p ≤ d` for every face.
    pub planes: Vec<(Vec3, f64)>,
    /// The corners, metres, the body's frame.
    pub vertices: Vec<Vec3>,
    /// Each face as corner indices, anticlockwise seen from outside.
    pub faces: Vec<Vec<usize>>,
}

impl Polyhedron {
    /// The convex hull of `points` (metres, the body's frame).
    ///
    /// # Errors
    /// With fewer than four points or more than [`MAX_POLYHEDRON_VERTICES`], a point that is not
    /// finite, or points that span no volume.
    pub fn hull(points: &[Vec3]) -> Result<Self, VoxelError> {
        if points.len() < 4 || points.len() > MAX_POLYHEDRON_VERTICES {
            return Err(VoxelError(format!(
                "a polyhedron needs from 4 to {MAX_POLYHEDRON_VERTICES} vertices, not {}",
                points.len()
            )));
        }
        if points.iter().any(|p| !(p.x.is_finite() && p.y.is_finite() && p.z.is_finite())) {
            return Err(VoxelError("a polyhedron's vertex is not finite".into()));
        }
        let scale = points.iter().fold(0f64, |m, p| m.max(p.length())).max(f64::MIN_POSITIVE);
        let tol = 1e-9 * scale;
        let mut planes: Vec<(Vec3, f64)> = Vec::new();
        for i in 0..points.len() {
            for j in i + 1..points.len() {
                for k in j + 1..points.len() {
                    let n = (points[j] - points[i]).cross(points[k] - points[i]);
                    if n.length() <= 1e-12 * scale * scale {
                        continue;
                    }
                    let n = n.normalised();
                    let d = n.dot(points[i]);
                    let side = points.iter().map(|p| n.dot(*p) - d);
                    let (lo, hi) = side.fold((0f64, 0f64), |(lo, hi), s| (lo.min(s), hi.max(s)));
                    let plane = if hi <= tol {
                        (n, d)
                    } else if lo >= -tol {
                        (-n, -d)
                    } else {
                        continue;
                    };
                    let known = planes
                        .iter()
                        .any(|(m, e)| (*m - plane.0).length() < 1e-9 && (e - plane.1).abs() < tol);
                    if !known {
                        planes.push(plane);
                    }
                }
            }
        }
        if planes.len() < 4 {
            return Err(VoxelError("a polyhedron's vertices span no volume".into()));
        }
        // Each face: the points on its plane, anticlockwise about the plane's normal.
        let mut faces = Vec::with_capacity(planes.len());
        for &(n, d) in &planes {
            let mut on: Vec<usize> =
                (0..points.len()).filter(|&v| (n.dot(points[v]) - d).abs() <= tol).collect();
            let centre = on.iter().fold(Vec3::ZERO, |s, &v| s + points[v]) * (1.0 / on.len() as f64);
            let u = (points[on[0]] - centre).normalised();
            let w = n.cross(u);
            on.sort_by(|&a, &b| {
                let angle = |v: usize| {
                    let r = points[v] - centre;
                    r.dot(w).atan2(r.dot(u))
                };
                angle(a).total_cmp(&angle(b))
            });
            faces.push(on);
        }
        Ok(Self { planes, vertices: points.to_vec(), faces })
    }

    /// The same polyhedron moved by `by` metres.
    #[must_use]
    pub fn moved(mut self, by: Vec3) -> Self {
        for v in &mut self.vertices {
            *v = *v + by;
        }
        for (n, d) in &mut self.planes {
            *d += n.dot(by);
        }
        self
    }

    /// Volume, centroid and second moments about the origin, from tetrahedra on each face's fan
    /// and the vertices' mean.
    fn integrals(&self) -> (f64, Vec3, [[f64; 3]; 3]) {
        let o = self.vertices.iter().fold(Vec3::ZERO, |s, v| s + *v) * (1.0 / self.vertices.len() as f64);
        let (mut volume, mut first, mut second) = (0.0, Vec3::ZERO, [[0.0; 3]; 3]);
        for face in &self.faces {
            for t in 1..face.len() - 1 {
                let [a, b, c] = [self.vertices[face[0]], self.vertices[face[t]], self.vertices[face[t + 1]]];
                let v = (a - o).dot((b - o).cross(c - o)) / 6.0;
                volume += v;
                let corners = [o, a, b, c];
                let sum = corners.iter().fold(Vec3::ZERO, |s, p| s + *p);
                first = first + sum * (v / 4.0);
                // ∫ x_a x_b dV over a tetrahedron = V/20 (Σ v_a v_b + (Σ v_a)(Σ v_b)).
                for (i, row) in second.iter_mut().enumerate() {
                    for (j, entry) in row.iter_mut().enumerate() {
                        let own: f64 = corners.iter().map(|p| component(*p, i) * component(*p, j)).sum();
                        *entry += v / 20.0 * (own + component(sum, i) * component(sum, j));
                    }
                }
            }
        }
        (volume, first * (1.0 / volume), second)
    }
}

impl Shape for Polyhedron {
    fn bounds(&self) -> Aabb {
        let first = Aabb::new(self.vertices[0], self.vertices[0]);
        self.vertices.iter().fold(first, |b, v| b.union(&Aabb::new(*v, *v)))
    }

    fn contains(&self, p: Vec3) -> bool {
        self.planes.iter().all(|(n, d)| n.dot(p) <= *d)
    }

    fn cover(&self, b: &Aabb) -> Cover {
        let scale = self.vertices.iter().fold(0f64, |m, v| m.max(v.length())) + reach(b);
        let slack = MARGIN * scale;
        let mut inside = true;
        for (n, d) in &self.planes {
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for i in 0..8 {
                let s = n.dot(b.corner(i)) - d;
                lo = lo.min(s);
                hi = hi.max(s);
            }
            if lo > slack {
                return Cover::Outside;
            }
            inside &= hi < -slack;
        }
        if inside { Cover::Inside } else { Cover::Mixed }
    }

    fn volume(&self) -> f64 {
        self.integrals().0
    }

    fn centroid(&self) -> Vec3 {
        self.integrals().1
    }

    fn second_moments(&self) -> [f64; 6] {
        let (v, c, s) = self.integrals();
        let c = [c.x, c.y, c.z];
        let at = |i: usize, j: usize| s[i][j] - v * c[i] * c[j];
        [at(0, 0), at(1, 1), at(2, 2), at(0, 1), at(0, 2), at(1, 2)]
    }
}

/// The corner `i` (0 to 7) of the cube from −1 to 1: bit 0 the sign of x, bit 1 of y, bit 2 of z.
fn signs(i: u32) -> [f64; 3] {
    [0, 1, 2].map(|b| if i & (1 << b) == 0 { -1.0 } else { 1.0 })
}

/// The golden ratio, φ.
const PHI: f64 = std::f64::consts::GOLDEN_RATIO;

/// The five Platonic solids, each centred on the origin, by edge length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platonic {
    /// Four triangles.
    Tetrahedron,
    /// Six squares.
    Cube,
    /// Eight triangles.
    Octahedron,
    /// Twelve pentagons.
    Dodecahedron,
    /// Twenty triangles.
    Icosahedron,
}

impl Platonic {
    /// All five, in order of their faces.
    pub const ALL: [Self; 5] =
        [Self::Tetrahedron, Self::Cube, Self::Octahedron, Self::Dodecahedron, Self::Icosahedron];

    /// The solid of edge `edge` metres, centred on the origin.
    ///
    /// # Panics
    /// Never for a positive finite edge: the vertices are the textbook ones.
    #[must_use]
    pub fn solid(self, edge: f64) -> Polyhedron {
        let (points, unit_edge): (Vec<[f64; 3]>, f64) = match self {
            Self::Tetrahedron => (
                vec![[1.0, 1.0, 1.0], [1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [-1.0, -1.0, 1.0]],
                2.0 * std::f64::consts::SQRT_2,
            ),
            Self::Cube => ((0..8).map(signs).collect(), 2.0),
            Self::Octahedron => (
                vec![
                    [1.0, 0.0, 0.0],
                    [-1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [0.0, -1.0, 0.0],
                    [0.0, 0.0, 1.0],
                    [0.0, 0.0, -1.0],
                ],
                std::f64::consts::SQRT_2,
            ),
            Self::Dodecahedron => {
                let mut p: Vec<[f64; 3]> = (0..8).map(signs).collect();
                for a in [-1.0, 1.0] {
                    for b in [-1.0, 1.0] {
                        p.push([0.0, a / PHI, b * PHI]);
                        p.push([a / PHI, b * PHI, 0.0]);
                        p.push([a * PHI, 0.0, b / PHI]);
                    }
                }
                (p, 2.0 / PHI)
            }
            Self::Icosahedron => {
                let mut p = Vec::new();
                for &a in &[-1.0, 1.0] {
                    for &b in &[-1.0, 1.0] {
                        p.push([0.0, a, b * PHI]);
                        p.push([a, b * PHI, 0.0]);
                        p.push([a * PHI, 0.0, b]);
                    }
                }
                (p, 2.0)
            }
        };
        let scale = edge / unit_edge;
        let points: Vec<Vec3> = points.iter().map(|p| Vec3::from_array(*p) * scale).collect();
        Polyhedron::hull(&points).expect("a Platonic solid is a polyhedron")
    }

    /// The textbook volume over the cube of the edge.
    #[must_use]
    pub fn volume_factor(self) -> f64 {
        let r5 = 5f64.sqrt();
        match self {
            Self::Tetrahedron => 1.0 / (6.0 * std::f64::consts::SQRT_2),
            Self::Cube => 1.0,
            Self::Octahedron => std::f64::consts::SQRT_2 / 3.0,
            Self::Dodecahedron => (15.0 + 7.0 * r5) / 4.0,
            Self::Icosahedron => 5.0 * (3.0 + r5) / 12.0,
        }
    }

    /// The textbook moment of inertia about any axis through the centre, over mass × edge².
    #[must_use]
    pub fn inertia_factor(self) -> f64 {
        let r5 = 5f64.sqrt();
        match self {
            Self::Tetrahedron => 1.0 / 20.0,
            Self::Cube => 1.0 / 6.0,
            Self::Octahedron => 1.0 / 10.0,
            Self::Dodecahedron => (95.0 + 39.0 * r5) / 300.0,
            Self::Icosahedron => (3.0 + r5) / 20.0,
        }
    }

    /// Its faces.
    #[must_use]
    pub const fn faces(self) -> usize {
        match self {
            Self::Tetrahedron => 4,
            Self::Cube => 6,
            Self::Octahedron => 8,
            Self::Dodecahedron => 12,
            Self::Icosahedron => 20,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_platonic_solids_are_their_textbook_selves() {
        for solid in Platonic::ALL {
            let edge = 0.7;
            let p = solid.solid(edge);
            assert_eq!(p.faces.len(), solid.faces(), "{solid:?}");
            // Every vertex the same distance from the centre, every edge the same length.
            let radius = p.vertices[0].length();
            assert!(p.vertices.iter().all(|v| (v.length() - radius).abs() < 1e-12), "{solid:?}");
            for face in &p.faces {
                for i in 0..face.len() {
                    let e = (p.vertices[face[i]] - p.vertices[face[(i + 1) % face.len()]]).length();
                    assert!((e - edge).abs() < 1e-12, "{solid:?}: an edge of {e}");
                }
            }
            // The polyhedral integrals give the textbook volume and inertia.
            let v = p.volume();
            assert!((v / (solid.volume_factor() * edge.powi(3)) - 1.0).abs() < 1e-12, "{solid:?}: {v}");
            let s = p.second_moments();
            // Unit density: mass v. I_xx = S_yy + S_zz, the same about every axis through the centre.
            let inertia = s[1] + s[2];
            let want = solid.inertia_factor() * v * edge * edge;
            assert!((inertia / want - 1.0).abs() < 1e-12, "{solid:?}: {inertia} against {want}");
            assert!(s[3].abs() + s[4].abs() + s[5].abs() < 1e-12 * want, "{solid:?}: products {s:?}");
            assert!(p.centroid().length() < 1e-14);
            // The planes and the vertices agree: every vertex inside, the centre inside, a point
            // beyond a vertex outside.
            assert!(p.vertices.iter().all(|v| p.contains(*v * (1.0 - 1e-9))));
            assert!(p.contains(Vec3::ZERO) && !p.contains(p.vertices[0] * 1.01));
        }
        // Control: the cube's factor is not the octahedron's; a wrong factor would be caught.
        assert!((Platonic::Cube.inertia_factor() - Platonic::Octahedron.inertia_factor()).abs() > 0.05);
        // Refusals: too few points, points in a plane.
        assert!(Polyhedron::hull(&[Vec3::ZERO, Vec3::X, Vec3::Y]).is_err());
        assert!(Polyhedron::hull(&[Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::X + Vec3::Y]).is_err());
    }
}
