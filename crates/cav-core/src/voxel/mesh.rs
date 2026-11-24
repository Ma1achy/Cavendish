//! Closed triangle meshes: read from Wavefront OBJ or STL, voxelised as any other shape.
//!
//! **Inside** is the parity of the crossings of a ray from the point along +x. To make that
//! answer the same for every point of a box that no triangle touches (which is what `cover`
//! relies on), the mesh must be watertight *in floating point*: a ray through a shared edge or a
//! vertex must cross exactly one of the triangles there. So each edge's side test is computed
//! from its two vertices in one canonical order (by vertex index), whichever triangle asks, and a
//! ray exactly on an edge is decided by a fixed symbolic nudge of the point (simulation of
//! simplicity), the same for every edge.
//!
//! **Caps** (checked before anything is reserved): the file is read up to [`MAX_MESH_BYTES`];
//! at most [`MAX_TRIANGLES`] triangles (an STL's count is in its header) and [`MAX_VERTICES`]
//! vertices. A mesh must be closed and consistently wound: every edge in exactly two triangles,
//! once each way. Files carry no unit: the scene gives metres per file unit.

use std::collections::HashMap;

use crate::Vec3;

use super::shape::{Cover, Shape};
use super::{Aabb, VoxelError};

/// The largest mesh file read, bytes.
pub const MAX_MESH_BYTES: u64 = 32 << 20;
/// The most triangles a mesh may have.
pub const MAX_TRIANGLES: usize = 500_000;
/// The most vertices a mesh may have.
pub const MAX_VERTICES: usize = 1_000_000;
/// Triangles in a leaf of the bounding-volume hierarchy, at most.
const LEAF_SIZE: usize = 4;

/// A closed, consistently wound triangle mesh, in a body's frame (metres).
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    /// The corners, metres.
    pub vertices: Vec<Vec3>,
    /// Each triangle's corners by index, anticlockwise seen from outside.
    pub triangles: Vec<[u32; 3]>,
    /// The hierarchy over the triangles: a node's box, and either two children or a run of
    /// `order`.
    nodes: Vec<BvhNode>,
    /// Triangle indices in the hierarchy's leaf order.
    order: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BvhNode {
    bounds: Aabb,
    /// For a leaf, the run `start..end` of `order`; for an inner node, `start` is the left child
    /// and `end` the right, and `leaf` is false.
    start: u32,
    end: u32,
    leaf: bool,
}

impl Mesh {
    /// The mesh of `vertices` (metres) and `triangles` (corner indices), checked.
    ///
    /// # Errors
    /// Past the caps; a corner index out of range or a vertex not finite; an edge not in exactly
    /// two triangles once each way (named); no volume.
    pub fn new(vertices: Vec<Vec3>, triangles: Vec<[u32; 3]>) -> Result<Self, VoxelError> {
        if vertices.len() > MAX_VERTICES || triangles.len() > MAX_TRIANGLES {
            return Err(VoxelError(format!(
                "a mesh of {} vertices and {} triangles is more than {MAX_VERTICES} and {MAX_TRIANGLES}",
                vertices.len(),
                triangles.len()
            )));
        }
        if triangles.len() < 4 {
            return Err(VoxelError(format!(
                "a closed mesh needs at least 4 triangles, not {}",
                triangles.len()
            )));
        }
        if vertices.iter().any(|v| !(v.x.is_finite() && v.y.is_finite() && v.z.is_finite())) {
            return Err(VoxelError("a mesh vertex is not finite".into()));
        }
        if let Some(t) = triangles.iter().find(|t| t.iter().any(|&i| i as usize >= vertices.len())) {
            return Err(VoxelError(format!("triangle {t:?} names a vertex that does not exist")));
        }
        // Each directed edge once, and its reverse once.
        let mut edges: HashMap<(u32, u32), u32> = HashMap::with_capacity(3 * triangles.len());
        for t in &triangles {
            for i in 0..3 {
                *edges.entry((t[i], t[(i + 1) % 3])).or_insert(0) += 1;
            }
        }
        let mut bad: Vec<(u32, u32)> = edges
            .iter()
            .filter(|&(&(a, b), &n)| n != 1 || edges.get(&(b, a)) != Some(&1))
            .map(|(&e, _)| e)
            .collect();
        bad.sort_unstable();
        if let Some(&(a, b)) = bad.first() {
            return Err(VoxelError(format!(
                "the mesh is not closed and consistently wound: the edge from vertex {a} to vertex {b} is not in \
                 exactly two triangles once each way ({} such edges)",
                bad.len()
            )));
        }
        let mut mesh = Self { vertices, triangles, nodes: Vec::new(), order: Vec::new() };
        if mesh.integrals().0 <= 0.0 {
            return Err(VoxelError("the mesh encloses no volume, or is wound inside out".into()));
        }
        mesh.build_hierarchy();
        Ok(mesh)
    }

    /// The mesh in `bytes` (an `.obj` or an `.stl`, binary or text, told apart by `name`'s
    /// extension), scaled by `scale` metres per file unit.
    ///
    /// # Errors
    /// As [`Self::new`], and for a file that cannot be read as its format.
    pub fn parse(name: &str, bytes: &[u8], scale: f64) -> Result<Self, VoxelError> {
        if bytes.len() as u64 > MAX_MESH_BYTES {
            return Err(VoxelError(format!("{name}: {} bytes is more than {MAX_MESH_BYTES}", bytes.len())));
        }
        let lower = name.to_ascii_lowercase();
        let (vertices, triangles) = if lower.ends_with(".obj") {
            parse_obj(bytes)?
        } else if lower.ends_with(".stl") {
            parse_stl(bytes)?
        } else {
            return Err(VoxelError(format!("{name}: a mesh is an .obj or an .stl file")));
        };
        if !(scale.is_finite() && scale > 0.0) {
            return Err(VoxelError(format!("scale: {scale} must be a positive number")));
        }
        Self::new(vertices.into_iter().map(|v| v * scale).collect(), triangles)
    }

    /// The mesh in the file at `path`, read up to [`MAX_MESH_BYTES`] and one byte.
    ///
    /// # Errors
    /// As [`Self::parse`], and when the file cannot be read.
    pub fn read(path: &std::path::Path, scale: f64) -> Result<Self, VoxelError> {
        use std::io::Read;
        let name = path.display().to_string();
        let file = std::fs::File::open(path).map_err(|e| VoxelError(format!("{name}: {e}")))?;
        // One buffer, reserved once: a buffer that doubled as it read would hold three times the
        // cap for a moment.
        let length = file.metadata().map_or(0, |m| m.len());
        let mut bytes = Vec::with_capacity((length.min(MAX_MESH_BYTES) + 1) as usize);
        file.take(MAX_MESH_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| VoxelError(format!("{name}: {e}")))?;
        Self::parse(&name, &bytes, scale)
    }

    /// The same mesh moved so that its centroid is at the origin.
    #[must_use]
    pub fn centred(self) -> Self {
        let c = self.integrals().1;
        let vertices = self.vertices.iter().map(|v| *v - c).collect();
        Self::new(vertices, self.triangles).expect("moving a valid mesh keeps it valid")
    }

    fn triangle_bounds(&self, t: usize) -> Aabb {
        let [a, b, c] = self.triangles[t].map(|i| self.vertices[i as usize]);
        Aabb::new(a, a).union(&Aabb::new(b, b)).union(&Aabb::new(c, c))
    }

    fn build_hierarchy(&mut self) {
        self.order = (0..self.triangles.len() as u32).collect();
        let centres: Vec<Vec3> =
            (0..self.triangles.len()).map(|t| self.triangle_bounds(t).centre()).collect();
        self.nodes.clear();
        self.nodes.push(BvhNode {
            bounds: self.bounds(),
            start: 0,
            end: self.order.len() as u32,
            leaf: true,
        });
        let mut stack = vec![0usize];
        while let Some(n) = stack.pop() {
            let (start, end) = (self.nodes[n].start as usize, self.nodes[n].end as usize);
            let bounds = self.order[start..end]
                .iter()
                .map(|&t| self.triangle_bounds(t as usize))
                .reduce(|a, b| a.union(&b))
                .expect("a node holds a triangle");
            self.nodes[n].bounds = bounds;
            if end - start <= LEAF_SIZE {
                continue;
            }
            let size = bounds.size();
            let axis = if size.x >= size.y && size.x >= size.z {
                0
            } else if size.y >= size.z {
                1
            } else {
                2
            };
            let key = |t: &u32| {
                let c = centres[*t as usize];
                [c.x, c.y, c.z][axis]
            };
            let middle = start + (end - start) / 2;
            self.order[start..end].select_nth_unstable_by(middle - start, |a, b| key(a).total_cmp(&key(b)));
            let left = self.nodes.len();
            self.nodes.push(BvhNode { bounds, start: start as u32, end: middle as u32, leaf: true });
            self.nodes.push(BvhNode { bounds, start: middle as u32, end: end as u32, leaf: true });
            self.nodes[n] = BvhNode { bounds, start: left as u32, end: left as u32 + 1, leaf: false };
            stack.push(left);
            stack.push(left + 1);
        }
    }

    /// Calls `visit` with every triangle whose box might pass `keep`.
    fn each_triangle(&self, keep: &dyn Fn(&Aabb) -> bool, visit: &mut dyn FnMut(usize)) {
        let mut stack = vec![0usize];
        while let Some(n) = stack.pop() {
            let node = self.nodes[n];
            if !keep(&node.bounds) {
                continue;
            }
            if node.leaf {
                for &t in &self.order[node.start as usize..node.end as usize] {
                    if keep(&self.triangle_bounds(t as usize)) {
                        visit(t as usize);
                    }
                }
            } else {
                stack.push(node.start as usize);
                stack.push(node.end as usize);
            }
        }
    }

    /// The side of `q = (y, z)` of the edge from vertex `a` to vertex `b`, projected along x:
    /// +1, −1, never 0 for a proper edge. Computed in one order of the two vertices whichever
    /// triangle asks, and nudged symbolically when `q` is on the edge's line.
    fn side(&self, a: u32, b: u32, q: (f64, f64)) -> i32 {
        let (lo, hi, flip) = if a < b { (a, b, 1) } else { (b, a, -1) };
        let (u, v) = (self.vertices[lo as usize], self.vertices[hi as usize]);
        let (dy, dz) = (v.y - u.y, v.z - u.z);
        let e = dy * (q.1 - u.z) - dz * (q.0 - u.y);
        let sign = if e > 0.0 {
            1
        } else if e < 0.0 {
            -1
        } else if dz != 0.0 {
            // q nudged by (ε, ε²): the change is dy·ε² − dz·ε.
            if dz > 0.0 { -1 } else { 1 }
        } else if dy > 0.0 {
            1
        } else if dy < 0.0 {
            -1
        } else {
            0
        };
        sign * flip
    }

    /// The crossings beyond `p` of the ray from `p` along +x.
    fn crossings(&self, p: Vec3) -> u32 {
        let q = (p.y, p.z);
        let mut count = 0u32;
        self.each_triangle(
            &|b: &Aabb| b.lo.y <= p.y && p.y <= b.hi.y && b.lo.z <= p.z && p.z <= b.hi.z && b.hi.x >= p.x,
            &mut |t| {
                let [i0, i1, i2] = self.triangles[t];
                let s = [self.side(i0, i1, q), self.side(i1, i2, q), self.side(i2, i0, q)];
                if !(s == [1, 1, 1] || s == [-1, -1, -1]) {
                    return;
                }
                // Where the ray meets the triangle: barycentric weights from the projected areas.
                let [a, b, c] = [i0, i1, i2].map(|i| self.vertices[i as usize]);
                let area = |u: Vec3, v: Vec3| (v.y - u.y) * (q.1 - u.z) - (v.z - u.z) * (q.0 - u.y);
                let (w0, w1, w2) = (area(b, c), area(c, a), area(a, b));
                let sum = w0 + w1 + w2;
                if sum == 0.0 {
                    return;
                }
                let x = (w0 * a.x + w1 * b.x + w2 * c.x) / sum;
                if x > p.x {
                    count += 1;
                }
            },
        );
        count
    }

    /// Volume, centroid and second moments about the origin, from signed tetrahedra on the origin.
    fn integrals(&self) -> (f64, Vec3, [[f64; 3]; 3]) {
        let (mut volume, mut first, mut second) = (0.0, Vec3::ZERO, [[0.0; 3]; 3]);
        for t in &self.triangles {
            let [a, b, c] = t.map(|i| self.vertices[i as usize]);
            let v = a.dot(b.cross(c)) / 6.0;
            volume += v;
            let sum = a + b + c;
            first = first + sum * (v / 4.0);
            let corners = [a, b, c];
            let s = sum.to_array();
            for (i, row) in second.iter_mut().enumerate() {
                for (j, entry) in row.iter_mut().enumerate() {
                    let own: f64 = corners.iter().map(|p| p.to_array()[i] * p.to_array()[j]).sum();
                    *entry += v / 20.0 * (own + s[i] * s[j]);
                }
            }
        }
        (volume, first * (1.0 / volume), second)
    }
}

impl Shape for Mesh {
    fn bounds(&self) -> Aabb {
        let first = Aabb::new(self.vertices[0], self.vertices[0]);
        self.vertices.iter().fold(first, |b, v| b.union(&Aabb::new(*v, *v)))
    }

    fn contains(&self, p: Vec3) -> bool {
        self.crossings(p) % 2 == 1
    }

    fn cover(&self, b: &Aabb) -> Cover {
        let reach = [b.lo, b.hi].iter().fold(0f64, |m, v| m.max(v.x.abs()).max(v.y.abs()).max(v.z.abs()));
        let slack = 1e-9 * (self.bounds().size().length() + reach);
        let grown = b.grown(slack);
        let touched = std::cell::Cell::new(false);
        self.each_triangle(&|t: &Aabb| !touched.get() && t.overlaps(&grown), &mut |t| {
            // A triangle whose plane has every corner of the box on one side, by the slack,
            // cannot meet it.
            let [p, q, r] = self.triangles[t].map(|i| self.vertices[i as usize]);
            let n = (q - p).cross(r - p);
            let length = n.length();
            if length == 0.0 {
                return;
            }
            let n = n * (1.0 / length);
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for i in 0..8 {
                let s = n.dot(b.corner(i) - p);
                lo = lo.min(s);
                hi = hi.max(s);
            }
            if lo <= slack && hi >= -slack {
                touched.set(true);
            }
        });
        if touched.get() {
            Cover::Mixed
        } else if self.contains(b.centre()) {
            Cover::Inside
        } else {
            Cover::Outside
        }
    }

    fn volume(&self) -> f64 {
        self.integrals().0
    }

    fn centroid(&self) -> Vec3 {
        self.integrals().1
    }

    fn second_moments(&self) -> [f64; 6] {
        let (v, c, s) = self.integrals();
        let c = c.to_array();
        let at = |i: usize, j: usize| s[i][j] - v * c[i] * c[j];
        [at(0, 0), at(1, 1), at(2, 2), at(0, 1), at(0, 2), at(1, 2)]
    }
}

fn parse_obj(bytes: &[u8]) -> Result<(Vec<Vec3>, Vec<[u32; 3]>), VoxelError> {
    let text = std::str::from_utf8(bytes).map_err(|e| VoxelError(format!("an .obj file is text: {e}")))?;
    let (mut vertices, mut triangles) = (Vec::new(), Vec::new());
    for (n, line) in text.lines().enumerate() {
        let mut words = line.split_whitespace();
        let fail = |why: &str| VoxelError(format!("line {}: {why}", n + 1));
        match words.next() {
            Some("v") => {
                if vertices.len() >= MAX_VERTICES {
                    return Err(fail(&format!("more than {MAX_VERTICES} vertices")));
                }
                let mut c = [0.0; 3];
                for value in &mut c {
                    *value = words
                        .next()
                        .and_then(|w| w.parse().ok())
                        .ok_or_else(|| fail("a vertex needs x y z"))?;
                }
                vertices.push(Vec3::from_array(c));
            }
            Some("f") => {
                let mut corners = Vec::new();
                for word in words {
                    let index: i64 = word
                        .split('/')
                        .next()
                        .and_then(|w| w.parse().ok())
                        .ok_or_else(|| fail("a face's corner"))?;
                    let resolved = if index > 0 { index - 1 } else { vertices.len() as i64 + index };
                    if !(0..vertices.len() as i64).contains(&resolved) {
                        return Err(fail(&format!("corner {index} names no vertex")));
                    }
                    corners.push(resolved as u32);
                    if corners.len() > 64 {
                        return Err(fail("a face of more than 64 corners"));
                    }
                }
                if corners.len() < 3 {
                    return Err(fail("a face needs three corners"));
                }
                for k in 1..corners.len() - 1 {
                    if triangles.len() >= MAX_TRIANGLES {
                        return Err(fail(&format!("more than {MAX_TRIANGLES} triangles")));
                    }
                    triangles.push([corners[0], corners[k], corners[k + 1]]);
                }
            }
            _ => {}
        }
    }
    Ok((vertices, triangles))
}

/// Vertices given by value (as STL gives them) joined into an indexed mesh by their bits.
struct Joiner {
    vertices: Vec<Vec3>,
    index: HashMap<[u64; 3], u32>,
}

impl Joiner {
    fn add(&mut self, v: Vec3) -> Result<u32, VoxelError> {
        let key = [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()];
        if let Some(&i) = self.index.get(&key) {
            return Ok(i);
        }
        if self.vertices.len() >= MAX_VERTICES {
            return Err(VoxelError(format!("more than {MAX_VERTICES} vertices")));
        }
        let i = self.vertices.len() as u32;
        self.vertices.push(v);
        self.index.insert(key, i);
        Ok(i)
    }
}

fn parse_stl(bytes: &[u8]) -> Result<(Vec<Vec3>, Vec<[u32; 3]>), VoxelError> {
    let mut joiner = Joiner { vertices: Vec::new(), index: HashMap::new() };
    let mut triangles = Vec::new();
    let binary = bytes.len() >= 84 && {
        let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as u64;
        bytes.len() as u64 == 84 + 50 * count
    };
    if binary {
        let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
        if count > MAX_TRIANGLES {
            return Err(VoxelError(format!("the file says {count} triangles, more than {MAX_TRIANGLES}")));
        }
        triangles.reserve_exact(count);
        for t in 0..count {
            let at = 84 + 50 * t + 12;
            let mut corners = [0u32; 3];
            for (k, corner) in corners.iter_mut().enumerate() {
                let f = |i: usize| {
                    let o = at + 12 * k + 4 * i;
                    f64::from(f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]))
                };
                *corner = joiner.add(Vec3::new(f(0), f(1), f(2)))?;
            }
            triangles.push(corners);
        }
    } else {
        let text =
            std::str::from_utf8(bytes).map_err(|e| VoxelError(format!("a text .stl file is text: {e}")))?;
        let mut corners = Vec::with_capacity(3);
        for (n, line) in text.lines().enumerate() {
            let mut words = line.split_whitespace();
            if words.next() == Some("vertex") {
                let mut c = [0.0; 3];
                for value in &mut c {
                    *value = words
                        .next()
                        .and_then(|w| w.parse().ok())
                        .ok_or_else(|| VoxelError(format!("line {}: a vertex needs x y z", n + 1)))?;
                }
                corners.push(joiner.add(Vec3::from_array(c))?);
                if corners.len() == 3 {
                    if triangles.len() >= MAX_TRIANGLES {
                        return Err(VoxelError(format!("more than {MAX_TRIANGLES} triangles")));
                    }
                    triangles.push([corners[0], corners[1], corners[2]]);
                    corners.clear();
                }
            }
        }
    }
    Ok((joiner.vertices, triangles))
}

/// Writes `mesh` as text OBJ: for tests and fixtures.
#[must_use]
pub fn to_obj(vertices: &[Vec3], triangles: &[[u32; 3]]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for v in vertices {
        let _ = writeln!(out, "v {:?} {:?} {:?}", v.x, v.y, v.z);
    }
    for t in triangles {
        let _ = writeln!(out, "f {} {} {}", t[0] + 1, t[1] + 1, t[2] + 1);
    }
    out
}

/// Writes triangles as binary STL (single precision, as the format is): for tests.
#[must_use]
pub fn to_stl(vertices: &[Vec3], triangles: &[[u32; 3]]) -> Vec<u8> {
    let mut out = vec![0u8; 80];
    out.extend((triangles.len() as u32).to_le_bytes());
    for t in triangles {
        out.extend([0u8; 12]);
        for &i in t {
            let v = vertices[i as usize];
            for c in [v.x, v.y, v.z] {
                out.extend((c as f32).to_le_bytes());
            }
        }
        out.extend([0u8; 2]);
    }
    out
}

/// A closed triangle mesh of an axis-aligned box from `lo` to `hi`: twelve triangles, wound outwards.
#[must_use]
pub fn box_mesh(lo: Vec3, hi: Vec3) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let vertices: Vec<Vec3> = (0..8).map(|i| Aabb::new(lo, hi).corner(i)).collect();
    // Faces as quads of corner indices, anticlockwise from outside; bit 0 x, bit 1 y, bit 2 z.
    let quads = [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]];
    let triangles = quads.iter().flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).collect();
    (vertices, triangles)
}

/// A closed torus about the z axis (major radius `major`, tube radius `minor`, metres) as
/// `around × across` quads split into triangles: a shape that is not convex, for tests.
#[must_use]
pub fn torus_mesh(major: f64, minor: f64, around: u32, across: u32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    use std::f64::consts::TAU;
    let mut vertices = Vec::new();
    for i in 0..around {
        let u = TAU * f64::from(i) / f64::from(around);
        for j in 0..across {
            let v = TAU * f64::from(j) / f64::from(across);
            let r = major + minor * v.cos();
            vertices.push(Vec3::new(r * u.cos(), r * u.sin(), minor * v.sin()));
        }
    }
    let at = |i: u32, j: u32| (i % around) * across + (j % across);
    let mut triangles = Vec::new();
    for i in 0..around {
        for j in 0..across {
            let (a, b, c, d) = (at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1));
            triangles.push([a, b, c]);
            triangles.push([a, c, d]);
        }
    }
    (vertices, triangles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mesh_is_read_or_refused_as_stated() {
        let (v, t) = box_mesh(Vec3::new(-1.0, -0.5, -0.25), Vec3::new(1.0, 0.5, 0.25));
        // OBJ and binary STL read back to the same solid (STL in single precision: these are exact).
        let obj = Mesh::parse("box.obj", to_obj(&v, &t).as_bytes(), 1.0).unwrap();
        let stl = Mesh::parse("box.stl", &to_stl(&v, &t), 1.0).unwrap();
        assert!((obj.volume() - 1.0).abs() < 1e-14 && (stl.volume() - 1.0).abs() < 1e-14);
        assert_eq!(obj.triangles.len(), 12);
        // The scale is metres per file unit.
        let big = Mesh::parse("box.obj", to_obj(&v, &t).as_bytes(), 2.0).unwrap();
        assert!((big.volume() - 8.0).abs() < 1e-13);
        // Inside and outside, and a torus's hole is outside.
        assert!(obj.contains(Vec3::new(0.3, 0.1, 0.0)) && !obj.contains(Vec3::new(1.3, 0.0, 0.0)));
        let (tv, tt) = torus_mesh(1.0, 0.3, 24, 12);
        let torus = Mesh::new(tv, tt).unwrap();
        assert!(!torus.contains(Vec3::ZERO) && torus.contains(Vec3::new(1.0, 0.0, 0.0)));
        // Refusals, each with its reason: an open mesh, a reversed triangle, too many, a bad file.
        let mut open = t.clone();
        open.pop();
        assert!(Mesh::new(v.clone(), open).unwrap_err().0.contains("not closed"));
        let mut flipped = t.clone();
        flipped[0].swap(1, 2);
        assert!(Mesh::new(v.clone(), flipped).is_err());
        let mut header = to_stl(&v, &t);
        header[80..84].copy_from_slice(&(MAX_TRIANGLES as u32 + 1).to_le_bytes());
        header.resize(84, 0);
        assert!(Mesh::parse("huge.stl", &header, 1.0).is_err());
        assert!(Mesh::parse("box.ply", b"ply", 1.0).is_err());
        assert!(Mesh::parse("box.obj", b"v 1 2\n", 1.0).unwrap_err().0.contains("line 1"));
        // Control: the closed mesh's volume is not that of a mesh missing a face.
        assert!((Mesh::new(v, t).unwrap().volume() - 1.0).abs() < 1e-14);
    }
}
