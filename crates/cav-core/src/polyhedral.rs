//! The gravity of a uniform polyhedron in closed form: Werner & Scheeres' face form, written with
//! the rounding rules of `docs/research/exact-skin-and-partial-fill.md` §1.1.
//!
//! A [`Faceted`] solid is a closed, consistently wound triangulated surface (each triangle
//! anticlockwise seen from outside), prepared for the closed form. For uniform density `ρ` at a
//! point `p`, with `r = v − p` (from the point to the body) and per triangle `f` its unit outward
//! normal `n_f`, twice its area `2A_f` and per edge `e` its in-plane outward normal `n_e`:
//!
//! ```text
//! h_f = n_f·r                   (any corner's r)
//! S_f = Σ_e (n_e·r_e) L_e,      L_e = log1p(2ℓ/(a + b − ℓ))
//! ω_f = 2 atan2(h_f·2A_f, r₁r₂r₃ + r₁ r₂·r₃ + r₂ r₃·r₁ + r₃ r₁·r₂)
//! U/G = ½ρ Σ_f h_f (S_f − h_f ω_f)          (W&S eq. 10, U positive)
//! ∇U/G = −ρ Σ_f n_f (S_f − h_f ω_f)          (W&S eq. 15, points at the mass)
//! Σ_f ω_f = 0 outside, 4π inside, 2π on a face (W&S eq. 17)
//! ```
//!
//! `a`, `b` are the distances to the edge's ends and `ℓ` its length. The repository's sign
//! convention is `V/G = −∫ρ/r` (negative, kg/m), so [`Faceted::potential`] returns `−U/G`; the
//! attraction [`Faceted::attraction`] is W&S's `∇U/G = −∇V/G`, pointing at the mass.
//!
//! **Rounding rules** ([X] §1.1, "What Cavendish's S5 kernel should follow", items 1–4):
//! - the solid angle's numerator is `h_f·2A_f` from the triangle's own edges, not the triple
//!   product `r₁·(r₂×r₃)` of vectors of length `R` (which makes the far-field error grow as
//!   `ε(R/s)³` instead of `ε(R/s)²`);
//! - `L_e` by `log1p`, never the ratio `ln((a+b+ℓ)/(a+b−ℓ))`;
//! - when the point projects onto the edge, `a + b − ℓ = r₀²/(a − ℓ₁) + r₀²/(b + ℓ₂)` (`r₀` the
//!   distance to the edge's line, `ℓ₁ ≤ 0 ≤ ℓ₂` the ends' offsets along it), which does not
//!   cancel beside the edge (Holstein & Ketteridge's `r − |ℓ| = r₀²/(r + |ℓ|)`);
//! - degenerate places are handled by dropping terms whose factor vanishes, never by NaN: on an
//!   edge or at a vertex `L_e` is infinite and its factor `n_e·r_e` is zero, so the term is
//!   dropped; a point within rounding of a face's plane has `ω_f = 0` (the mean of the two
//!   one-sided limits `±2π` inside the face, and the value outside it), where `h_f ω_f` and
//!   `h_f² ω_f` vanish anyway.
//!
//! **Where it holds.** Within [`EXACT_WITHIN`] sizes. Each edge term is `O(s)` while the field is
//! `O(s(s/R)²)`, so the relative error grows as `ε(R/s)²` (measured on a cube, the worst of `V` and
//! `g` over 16 directions: 4e-12 at `R/s` = 10, 8e-10 at 10³, 8e-8 at 10⁴; `docs/slices/S5.md`
//! §7.1). Beyond it a solid is read through its
//! exact moments ([`Faceted::moments`]).
//!
//! **Moments.** By the divergence theorem over the same triangles: each triangle and a reference
//! point make a signed tetrahedron, whose volume, first, second and third moments are closed
//! forms in its corners. Two passes: the centroid first, then the central moments about it, so
//! that nothing is moved by the parallel-axis rules and nothing cancels.
//!
//! Frames: everything is in the solid's own frame, metres.

use std::collections::HashMap;

use crate::Vec3;
use crate::voxel::mesh::{MAX_TRIANGLES, Mesh, box_mesh};
use crate::voxel::shape::{Cuboid, Platonic, Polyhedron};
use crate::voxel::tree::Moments;

/// Within this many sizes ([`Faceted::size`]) of a solid the closed form is the read; beyond it
/// the solid is read through its exact moments ([`Faceted::moments`]) and the multipole
/// expansion. [X] §1.1 rule 2: the closed form's relative error grows as `ε(R/s)²`, about 1e-10
/// at 10³ sizes (measured 8e-10), while the moments' expansion improves with distance.
///
/// That holds for `V` and `g`. The difference `V(upper) − V(lower)` of two potentials of the
/// same size ([`Faceted::difference`]) loses a further `R/gap`: measured, it keeps 1e-9 only to
/// about 30 sizes for a gap of one size and 100 for a gap of ten (`docs/slices/S5.md` §7.1), so a
/// reader of the arms' difference switches to the moments nearer than this. The switch is made
/// by the reader (`cav_sim`); this module defines where.
pub const EXACT_WITHIN: f64 = 1e3;

/// The most triangles a [`Faceted`] solid may have: a mesh's limit ([`MAX_TRIANGLES`]). Checked
/// before anything is reserved.
pub const MAX_FACES: usize = MAX_TRIANGLES;

/// A point within `PLANE_ROUNDING × (|p| + reach)` of a face's plane is on it (its `ω_f` is 0):
/// the rounding of `h_f = n_f·(v − p)` formed from coordinates of that size.
const PLANE_ROUNDING: f64 = 8.0 * f64::EPSILON;

/// Why a set of triangles is not a solid the closed form can read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FacetError(pub String);

impl std::fmt::Display for FacetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FacetError {}

/// One triangle of a [`Faceted`] solid, with what the closed form needs of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Face {
    /// Its corners, indices into [`Faceted::vertices`], anticlockwise seen from outside.
    pub corners: [u32; 3],
    /// Its edges, indices into [`Faceted::edges`]: edge `k` runs from corner `k` to corner
    /// `k + 1` (mod 3).
    pub edges: [u32; 3],
    /// Its unit outward normal, the solid's frame.
    pub normal: Vec3,
    /// Twice its area, m²: `|(v₂ − v₁) × (v₃ − v₁)|`.
    pub twice_area: f64,
    /// Per edge `k`, the unit normal to the edge in the triangle's plane, pointing out of the
    /// triangle: `ê_k × n`, with `ê_k` from corner `k` to corner `k + 1`.
    pub edge_normals: [Vec3; 3],
}

/// One edge of a [`Faceted`] solid, shared by two triangles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    /// Its ends, indices into [`Faceted::vertices`], the lower index first.
    pub ends: [u32; 2],
    /// The unit vector from the first end to the second, the solid's frame.
    pub direction: Vec3,
    /// Its length, metres.
    pub length: f64,
}

/// The potential, attraction and solid-angle sum of a solid at one point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Field {
    /// `V/G`, kg/m, negative (the repository's convention `V = −G∫ρ/r`).
    pub potential: f64,
    /// `g/G = −∇V/G`, kg/m², the solid's frame, pointing at the mass.
    pub attraction: Vec3,
    /// `Σ_f ω_f`, steradians: 0 outside, 4π inside, 2π on a face.
    pub solid_angle: f64,
}

/// A closed, consistently wound triangulated solid prepared for the polyhedral closed form, in
/// its own frame (metres).
#[derive(Clone, Debug, PartialEq)]
pub struct Faceted {
    vertices: Vec<Vec3>,
    faces: Vec<Face>,
    edges: Vec<Edge>,
    /// The largest `|v|` of a vertex, metres: the scale of the coordinates' rounding.
    reach: f64,
    /// Twice the largest distance of a vertex from the centroid, metres.
    size: f64,
}

/// A coordinate's bits, with −0 taken as 0, for finding vertices that are the same point.
fn bits(v: Vec3) -> [u64; 3] {
    v.to_array().map(|x| if x == 0.0 { 0 } else { x.to_bits() })
}

impl Faceted {
    /// The solid bounded by `triangles` (each three corners, metres, anticlockwise seen from
    /// outside). Corners that are the same point (to the bit) are one vertex.
    ///
    /// # Errors
    /// As [`Self::indexed`].
    pub fn new(triangles: &[[Vec3; 3]]) -> Result<Self, FacetError> {
        if triangles.len() > MAX_FACES {
            return Err(FacetError(format!("{} triangles is more than {MAX_FACES}", triangles.len())));
        }
        let mut index: HashMap<[u64; 3], u32> = HashMap::with_capacity(triangles.len());
        let mut vertices = Vec::with_capacity(triangles.len() / 2 + 2);
        let mut indexed = Vec::with_capacity(triangles.len());
        for t in triangles {
            indexed.push(t.map(|v| {
                *index.entry(bits(v)).or_insert_with(|| {
                    vertices.push(v);
                    (vertices.len() - 1) as u32
                })
            }));
        }
        Self::indexed(vertices, &indexed)
    }

    /// The solid of `vertices` (metres) and `triangles` (corner indices, anticlockwise seen from
    /// outside). Vertices no triangle uses are dropped.
    ///
    /// # Errors
    /// Fewer than 4 triangles or more than [`MAX_FACES`]; a corner index out of range or a vertex
    /// not finite; a triangle with no area; an edge not in exactly two triangles once each way
    /// (not closed, or not consistently wound); no volume, or wound inside out.
    pub fn indexed(vertices: Vec<Vec3>, triangles: &[[u32; 3]]) -> Result<Self, FacetError> {
        if triangles.len() < 4 || triangles.len() > MAX_FACES {
            return Err(FacetError(format!(
                "a closed solid needs from 4 to {MAX_FACES} triangles, not {}",
                triangles.len()
            )));
        }
        if let Some(t) = triangles.iter().find(|t| t.iter().any(|&i| i as usize >= vertices.len())) {
            return Err(FacetError(format!("triangle {t:?} names a vertex that does not exist")));
        }
        // Keep only the vertices used, in their order.
        let mut map = vec![u32::MAX; vertices.len()];
        let mut used = Vec::new();
        for t in triangles {
            for &i in t {
                if map[i as usize] == u32::MAX {
                    map[i as usize] = used.len() as u32;
                    used.push(vertices[i as usize]);
                }
            }
        }
        let vertices = used;
        let triangles: Vec<[u32; 3]> = triangles.iter().map(|t| t.map(|i| map[i as usize])).collect();
        if vertices.iter().any(|v| !(v.x.is_finite() && v.y.is_finite() && v.z.is_finite())) {
            return Err(FacetError("a vertex is not finite".into()));
        }
        // Each directed edge once and its reverse once; one undirected edge per pair.
        let mut directed: HashMap<(u32, u32), u32> = HashMap::with_capacity(3 * triangles.len());
        for t in &triangles {
            for k in 0..3 {
                *directed.entry((t[k], t[(k + 1) % 3])).or_insert(0) += 1;
            }
        }
        let mut bad: Vec<(u32, u32)> = directed
            .iter()
            .filter(|&(&(a, b), &n)| n != 1 || directed.get(&(b, a)) != Some(&1))
            .map(|(&e, _)| e)
            .collect();
        bad.sort_unstable();
        if let Some(&(a, b)) = bad.first() {
            return Err(FacetError(format!(
                "not closed and consistently wound: the edge from vertex {a} to vertex {b} is not in exactly two \
                 triangles once each way ({} such edges)",
                bad.len()
            )));
        }
        let mut edge_of: HashMap<(u32, u32), u32> = HashMap::with_capacity(directed.len() / 2);
        let mut edges = Vec::with_capacity(directed.len() / 2);
        let mut faces = Vec::with_capacity(triangles.len());
        for (n, t) in triangles.iter().enumerate() {
            let [a, b, c] = t.map(|i| vertices[i as usize]);
            let cross = (b - a).cross(c - a);
            let twice_area = cross.length();
            if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] || twice_area == 0.0 {
                return Err(FacetError(format!("triangle {n} ({t:?}) has no area")));
            }
            let normal = cross * (1.0 / twice_area);
            let mut face_edges = [0u32; 3];
            let mut edge_normals = [Vec3::ZERO; 3];
            for k in 0..3 {
                let (i, j) = (t[k], t[(k + 1) % 3]);
                let along = vertices[j as usize] - vertices[i as usize];
                edge_normals[k] = along.normalised().cross(normal);
                let key = (i.min(j), i.max(j));
                face_edges[k] = *edge_of.entry(key).or_insert_with(|| {
                    let d = vertices[key.1 as usize] - vertices[key.0 as usize];
                    let length = d.length();
                    edges.push(Edge { ends: [key.0, key.1], direction: d * (1.0 / length), length });
                    (edges.len() - 1) as u32
                });
            }
            faces.push(Face { corners: *t, edges: face_edges, normal, twice_area, edge_normals });
        }
        let reach = vertices.iter().fold(0f64, |m, v| m.max(v.length()));
        let mut solid = Self { vertices, faces, edges, reach, size: 0.0 };
        let (volume, centroid) = solid.volume_and_centroid();
        if volume.is_nan() || volume <= 0.0 {
            return Err(FacetError("the triangles enclose no volume, or are wound inside out".into()));
        }
        solid.size = 2.0 * solid.vertices.iter().fold(0f64, |m, v| m.max((*v - centroid).length()));
        Ok(solid)
    }

    /// The convex polyhedron `polyhedron`, each face fan-triangulated from its first corner.
    ///
    /// # Errors
    /// As [`Self::indexed`] (a hull from `Polyhedron::hull` passes).
    pub fn from_polyhedron(polyhedron: &Polyhedron) -> Result<Self, FacetError> {
        let mut triangles = Vec::new();
        for face in &polyhedron.faces {
            for t in 1..face.len().saturating_sub(1) {
                triangles.push([face[0], face[t], face[t + 1]].map(|i| i as u32));
            }
        }
        Self::indexed(polyhedron.vertices.clone(), &triangles)
    }

    /// The Platonic solid `solid` of edge `edge` metres, centred on the origin.
    ///
    /// # Errors
    /// For an edge that is not positive and finite.
    pub fn platonic(solid: Platonic, edge: f64) -> Result<Self, FacetError> {
        if !(edge > 0.0 && edge.is_finite()) {
            return Err(FacetError(format!(
                "a Platonic solid's edge must be positive and finite, not {edge}"
            )));
        }
        Self::from_polyhedron(&solid.solid(edge))
    }

    /// The box `cuboid`, twelve triangles.
    ///
    /// # Errors
    /// For a box with no volume.
    pub fn from_cuboid(cuboid: &Cuboid) -> Result<Self, FacetError> {
        let (vertices, triangles) = box_mesh(cuboid.centre - cuboid.half, cuboid.centre + cuboid.half);
        Self::indexed(vertices, &triangles)
    }

    /// The closed mesh `mesh`, its triangles as they are.
    ///
    /// # Errors
    /// As [`Self::indexed`] (a mesh from `Mesh::new` passes).
    pub fn from_mesh(mesh: &Mesh) -> Result<Self, FacetError> {
        Self::indexed(mesh.vertices.clone(), &mesh.triangles)
    }

    /// The vertices, metres, the solid's frame.
    #[must_use]
    pub fn vertices(&self) -> &[Vec3] {
        &self.vertices
    }

    /// The triangles.
    #[must_use]
    pub fn faces(&self) -> &[Face] {
        &self.faces
    }

    /// The edges, each shared by two triangles.
    #[must_use]
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// The solid's size `s`, metres: twice the largest distance of a vertex from its centroid
    /// (the diameter of the ball about the centroid that holds it). [`EXACT_WITHIN`] is in these
    /// units.
    #[must_use]
    pub const fn size(&self) -> f64 {
        self.size
    }

    /// The potential, attraction and solid-angle sum at `point` (metres, the solid's frame) of
    /// the solid at uniform `density` kg/m³. Finite everywhere, the surface included.
    #[must_use]
    pub fn field(&self, density: f64, point: Vec3) -> Field {
        // Per vertex, r = v − p and |r|; per edge, its logarithm (infinite on the edge: dropped).
        let r: Vec<(Vec3, f64)> = self
            .vertices
            .iter()
            .map(|v| {
                let r = *v - point;
                (r, r.length())
            })
            .collect();
        let logs: Vec<f64> = self
            .edges
            .iter()
            .map(|e| {
                let (ra, a) = r[e.ends[0] as usize];
                let (rb, b) = r[e.ends[1] as usize];
                edge_log(ra, rb, a, b, e.direction, e.length)
            })
            .collect();
        let on_plane = PLANE_ROUNDING * (point.length() + self.reach);
        let (mut u, mut g, mut omega) = (0.0, Vec3::ZERO, 0.0);
        for f in &self.faces {
            let [(r1, l1), (r2, l2), (r3, l3)] = f.corners.map(|c| r[c as usize]);
            let h = f.normal.dot(r1);
            let w = if h.abs() <= on_plane {
                0.0
            } else {
                let den = l1 * l2 * l3 + l1 * r2.dot(r3) + l2 * r3.dot(r1) + l3 * r1.dot(r2);
                2.0 * (h * f.twice_area).atan2(den)
            };
            let mut s = 0.0;
            for k in 0..3 {
                let log = logs[f.edges[k] as usize];
                if log.is_finite() {
                    s += f.edge_normals[k].dot(r[f.corners[k] as usize].0) * log;
                }
            }
            let b = s - h * w;
            u += 0.5 * h * b;
            g = g - f.normal * b;
            omega += w;
        }
        Field { potential: -density * u, attraction: g * density, solid_angle: omega }
    }

    /// `V/G`, kg/m (negative), at `point` (metres, the solid's frame) of the solid at uniform
    /// `density` kg/m³: minus Werner & Scheeres' `U/G`.
    #[must_use]
    pub fn potential(&self, density: f64, point: Vec3) -> f64 {
        self.field(density, point).potential
    }

    /// `g/G = −∇V/G`, kg/m², at `point` (metres, the solid's frame) of the solid at uniform
    /// `density` kg/m³: Werner & Scheeres' `∇U/G`, pointing at the mass.
    #[must_use]
    pub fn attraction(&self, density: f64, point: Vec3) -> Vec3 {
        self.field(density, point).attraction
    }

    /// `Σ_f ω_f` at `point` (metres, the solid's frame), steradians: 0 outside, 4π inside, 2π on
    /// a face (less on an edge or at a vertex: the solid angle the solid fills there).
    #[must_use]
    pub fn solid_angle(&self, point: Vec3) -> f64 {
        self.field(1.0, point).solid_angle
    }

    /// `(V(upper) − V(lower))/G` (kg/m) for the solid at uniform `density` kg/m³, the two points
    /// on a vertical line at `across` (its horizontal offset from the solid's origin, metres) at
    /// heights `below` and `above` metres over the origin, as `voxel::kernel::cube_difference`
    /// takes them. `gap` is `below − above` as the caller knows it exactly; the closed form does
    /// not need it (the two potentials are formed and subtracted, which loses `R/gap` of their
    /// digits: hence [`EXACT_WITHIN`]), and a zero gap is exactly zero.
    #[must_use]
    pub fn difference(&self, density: f64, across: [f64; 2], below: f64, above: f64, gap: f64) -> f64 {
        if gap == 0.0 {
            return 0.0;
        }
        let upper = self.potential(density, Vec3::new(across[0], across[1], above));
        let lower = self.potential(density, Vec3::new(across[0], across[1], below));
        upper - lower
    }

    /// The volume, m³, and the centroid, metres (the solid's frame), by tetrahedra on the vertices'
    /// mean.
    fn volume_and_centroid(&self) -> (f64, Vec3) {
        let o = self.vertices.iter().fold(Vec3::ZERO, |s, v| s + *v) * (1.0 / self.vertices.len() as f64);
        let (mut volume, mut first) = (0.0, Vec3::ZERO);
        for f in &self.faces {
            let [a, b, c] = f.corners.map(|i| self.vertices[i as usize] - o);
            let v = a.dot(b.cross(c)) / 6.0;
            volume += v;
            first = first + (a + b + c) * (v / 4.0);
        }
        (volume, o + first * (1.0 / volume))
    }

    /// The volume, m³.
    #[must_use]
    pub fn volume(&self) -> f64 {
        self.volume_and_centroid().0
    }

    /// The solid's exact mass moments at uniform `density` kg/m³, by the divergence theorem over
    /// its triangles: mass (kg), centroid (metres, the solid's frame), the second central moments
    /// (kg m², `Moments::second`'s order xx, yy, zz, xy, xz, yz) and the third (kg m³,
    /// `Moments::third`'s order xxx, yyy, zzz, xxy, xxz, xyy, yyz, xzz, yzz, xyz).
    #[must_use]
    pub fn moments(&self, density: f64) -> Moments {
        let (volume, centre) = self.volume_and_centroid();
        // About the centroid: each triangle with the centroid makes a signed tetrahedron whose
        // fourth corner is the origin of the sums. For a tetrahedron of volume V and corners q_i,
        // ∫x_a x_b = V/20 (Σ q_a q_b + S_a S_b) and
        // ∫x_a x_b x_c = V/120 (S_a S_b S_c + Σ(q_a q_b S_c + q_a q_c S_b + q_b q_c S_a)
        // + 2 Σ q_a q_b q_c), S = Σ q_i (the Dirichlet moments of the barycentric coordinates).
        let mut second = [0.0; 6];
        let mut third = [0.0; 10];
        const SECOND: [(usize, usize); 6] = [(0, 0), (1, 1), (2, 2), (0, 1), (0, 2), (1, 2)];
        const THIRD: [(usize, usize, usize); 10] = [
            (0, 0, 0),
            (1, 1, 1),
            (2, 2, 2),
            (0, 0, 1),
            (0, 0, 2),
            (0, 1, 1),
            (1, 1, 2),
            (0, 2, 2),
            (1, 2, 2),
            (0, 1, 2),
        ];
        for f in &self.faces {
            let q = f.corners.map(|i| (self.vertices[i as usize] - centre).to_array());
            let v = Vec3::from_array(q[0]).dot(Vec3::from_array(q[1]).cross(Vec3::from_array(q[2]))) / 6.0;
            let s = [0, 1, 2].map(|a| q[0][a] + q[1][a] + q[2][a]);
            for (slot, &(a, b)) in SECOND.iter().enumerate() {
                let own: f64 = q.iter().map(|p| p[a] * p[b]).sum();
                second[slot] += v / 20.0 * (own + s[a] * s[b]);
            }
            for (slot, &(a, b, c)) in THIRD.iter().enumerate() {
                let mut sum = s[a] * s[b] * s[c];
                for p in &q {
                    sum += p[a] * p[b] * s[c]
                        + p[a] * p[c] * s[b]
                        + p[b] * p[c] * s[a]
                        + 2.0 * p[a] * p[b] * p[c];
                }
                third[slot] += v / 120.0 * sum;
            }
        }
        Moments {
            mass: density * volume,
            centre,
            second: second.map(|x| density * x),
            third: third.map(|x| density * x),
        }
    }
}

/// `L_e = ln((a + b + ℓ)/(a + b − ℓ))` for an edge of length `length` along the unit `direction`
/// from its first end to its second, `ra`, `rb` the ends' offsets from the point (metres) and `a`,
/// `b` their lengths: by `log1p`, with `a + b − ℓ` in the `r₀²` form when the point projects onto
/// the edge. Infinite on the edge itself (`a + b = ℓ`), where its factor vanishes.
fn edge_log(ra: Vec3, rb: Vec3, a: f64, b: f64, direction: Vec3, length: f64) -> f64 {
    let (l1, l2) = (ra.dot(direction), rb.dot(direction));
    let gap = if l1 <= 0.0 && l2 >= 0.0 {
        let c = ra.cross(direction);
        let r0sq = c.dot(c);
        if r0sq == 0.0 {
            return f64::INFINITY;
        }
        r0sq / (a - l1) + r0sq / (b + l2)
    } else {
        a + b - length
    };
    if gap > 0.0 { (2.0 * length / gap).ln_1p() } else { f64::INFINITY }
}

#[cfg(test)]
mod tests;
