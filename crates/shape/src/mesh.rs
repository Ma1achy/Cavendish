//! Mesh import (M10): triangle-soup parsing, the mandatory-scale gate, watertightness, the
//! generalised winding number, and the two inside/outside classifiers. An imported mesh voxelises
//! through the *same* pipeline as a primitive (`crate::voxelise`), so it is indistinguishable
//! downstream.
//!
//! Design: `design/shape.md` §5. The format parsers (STL/OBJ/glTF) are feature-gated
//! (`stl`/`obj`/`gltf`) so the core engine builds without them; the geometry core — winding number,
//! watertightness, classification — is always compiled and validated against analytic truth.

use crate::{Aabb, MassSpec, ShapeError, Solid, VoxelParams};
use gravity::Cloud;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

/// An indexed triangle soup in metres — the common target of every parser, and the input to
/// classification. Vertices are shared; a triangle names three of them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TriSoup {
    pub verts: Vec<[f64; 3]>,
    pub tris: Vec<[usize; 3]>,
}

impl TriSoup {
    /// The number of triangles.
    pub fn len(&self) -> usize {
        self.tris.len()
    }

    /// Whether the soup has no triangles.
    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }

    /// A copy with every vertex multiplied by `scale` (metres per model unit).
    pub fn scaled(&self, scale: f64) -> TriSoup {
        TriSoup {
            verts: self
                .verts
                .iter()
                .map(|v| [v[0] * scale, v[1] * scale, v[2] * scale])
                .collect(),
            tris: self.tris.clone(),
        }
    }
}

/// A mesh import request. `scale` is **mandatory** — a mesh file carries no units, so an absent
/// scale is refused (`ScaleMissing`) rather than guessed.
#[derive(Clone, Debug)]
pub struct MeshImport {
    pub path: PathBuf,
    pub scale: Option<f64>,
    pub voxel: VoxelParams,
    pub mass: MassSpec,
}

/// What classification found — a warning-grade record that travels with an imported mesh so a dirty
/// mesh is *classified*, not silently accepted. (Fields accrete across the M10 commits.)
#[derive(Clone, Debug, PartialEq)]
pub struct MeshReport {
    /// Triangle count.
    pub faces: usize,
    /// Boundary edges — shared by only one triangle. Zero on a sound mesh.
    pub open_edges: usize,
    /// Edges shared by two triangles that traverse them the *same* way (inconsistent winding).
    pub flipped: usize,
    /// Every edge shared by exactly two consistently-oriented triangles.
    pub watertight: bool,
    /// Signed volume by the divergence theorem `V = (1/6) Σ p₀·(p₁×p₂)`. Positive for an
    /// outward-oriented mesh; the sign catches inverted orientation, the magnitude gross scale errors.
    pub volume: f64,
    /// The robust classifier's ambiguity diagnostic `A = mean min(|w|, |w−1|)`, when the robust path
    /// was taken (open/non-manifold). `None` for a watertight mesh classified by the fast path.
    pub ambiguity: Option<f64>,
}

/// Lower-case file extension, or `""`.
fn ext(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Read a mesh file and parse it to a triangle soup, dispatching on the extension. Each parser is
/// feature-gated; an extension whose feature is off is reported, not silently skipped.
fn parse_path(path: &Path) -> Result<TriSoup, ShapeError> {
    let bytes = std::fs::read(path).map_err(|e| ShapeError::UnreadableMesh(e.to_string()))?;
    match ext(path).as_str() {
        "stl" => parse_stl(&bytes),
        "obj" => parse_obj(&bytes),
        "gltf" | "glb" => parse_gltf(&bytes),
        other => Err(ShapeError::UnreadableMesh(format!(
            "unsupported mesh extension: '{other}'"
        ))),
    }
}

/// Import a mesh: apply the mandatory scale, parse to a scaled triangle soup, and classify it. The
/// returned [`MeshSolid`] voxelises via [`voxelise_mesh`].
///
/// **Scale is checked first**, before any file read, so a missing scale fails loudly and cheaply.
pub fn load_solid(import: &MeshImport) -> Result<(MeshSolid, MeshReport), ShapeError> {
    let scale = import.scale.ok_or(ShapeError::ScaleMissing)?;
    let soup = parse_path(&import.path)?.scaled(scale);
    MeshSolid::from_soup(soup)
}

// ── Parsers: feature-gated, each a thin adaptor from a vetted format crate to `TriSoup`. ──

/// Parse binary or ASCII STL from bytes.
#[cfg(feature = "stl")]
pub fn parse_stl(bytes: &[u8]) -> Result<TriSoup, ShapeError> {
    let mut cur = std::io::Cursor::new(bytes);
    let mesh = stl_io::read_stl(&mut cur).map_err(|e| ShapeError::UnreadableMesh(e.to_string()))?;
    let verts = mesh
        .vertices
        .iter()
        .map(|v| [v[0] as f64, v[1] as f64, v[2] as f64])
        .collect();
    let tris = mesh.faces.iter().map(|f| f.vertices).collect();
    Ok(TriSoup { verts, tris })
}

#[cfg(not(feature = "stl"))]
pub fn parse_stl(_bytes: &[u8]) -> Result<TriSoup, ShapeError> {
    Err(ShapeError::UnreadableMesh(
        "STL support (feature 'stl') is not enabled".into(),
    ))
}

/// Parse Wavefront OBJ from bytes (triangulated, single-indexed).
#[cfg(feature = "obj")]
pub fn parse_obj(bytes: &[u8]) -> Result<TriSoup, ShapeError> {
    let mut reader = std::io::BufReader::new(bytes);
    let (models, _) = tobj::load_obj_buf(&mut reader, &tobj::GPU_LOAD_OPTIONS, |_| {
        // No material library is referenced by an imported geometry mesh; this is never called.
        Err(tobj::LoadError::GenericFailure)
    })
    .map_err(|e| ShapeError::UnreadableMesh(e.to_string()))?;
    let mut verts = Vec::new();
    let mut tris = Vec::new();
    for m in &models {
        let base = verts.len();
        for c in m.mesh.positions.chunks_exact(3) {
            verts.push([c[0] as f64, c[1] as f64, c[2] as f64]);
        }
        for idx in m.mesh.indices.chunks_exact(3) {
            tris.push([
                base + idx[0] as usize,
                base + idx[1] as usize,
                base + idx[2] as usize,
            ]);
        }
    }
    Ok(TriSoup { verts, tris })
}

#[cfg(not(feature = "obj"))]
pub fn parse_obj(_bytes: &[u8]) -> Result<TriSoup, ShapeError> {
    Err(ShapeError::UnreadableMesh(
        "OBJ support (feature 'obj') is not enabled".into(),
    ))
}

/// Parse glTF/GLB from bytes.
#[cfg(feature = "gltf")]
pub fn parse_gltf(bytes: &[u8]) -> Result<TriSoup, ShapeError> {
    let (doc, buffers, _) =
        gltf::import_slice(bytes).map_err(|e| ShapeError::UnreadableMesh(e.to_string()))?;
    let mut verts = Vec::new();
    let mut tris = Vec::new();
    for mesh in doc.meshes() {
        for prim in mesh.primitives() {
            let reader = prim.reader(|b| buffers.get(b.index()).map(|d| d.0.as_slice()));
            let base = verts.len();
            let positions = reader.read_positions().ok_or_else(|| {
                ShapeError::UnreadableMesh("glTF primitive carries no POSITION".into())
            })?;
            for p in positions {
                verts.push([p[0] as f64, p[1] as f64, p[2] as f64]);
            }
            match reader.read_indices() {
                Some(idx) => {
                    let idx: Vec<u32> = idx.into_u32().collect();
                    for t in idx.chunks_exact(3) {
                        tris.push([
                            base + t[0] as usize,
                            base + t[1] as usize,
                            base + t[2] as usize,
                        ]);
                    }
                }
                None => {
                    // Non-indexed: three consecutive vertices per triangle.
                    let n = verts.len() - base;
                    for t in (0..n / 3).map(|i| i * 3) {
                        tris.push([base + t, base + t + 1, base + t + 2]);
                    }
                }
            }
        }
    }
    Ok(TriSoup { verts, tris })
}

#[cfg(not(feature = "gltf"))]
pub fn parse_gltf(_bytes: &[u8]) -> Result<TriSoup, ShapeError> {
    Err(ShapeError::UnreadableMesh(
        "glTF support (feature 'gltf') is not enabled".into(),
    ))
}

// ── The geometry core (always compiled): watertightness, the winding number, and the BVH. ──

#[inline]
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
#[inline]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
#[inline]
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
#[inline]
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// The solid angle subtended by triangle `(a, b, c)` at the origin (vertices already relative to the
/// query point), via the van Oosterom–Strackee formula. Signed by the triangle's orientation.
fn solid_angle(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    let (la, lb, lc) = (norm(a), norm(b), norm(c));
    let num = dot(a, cross(b, c));
    let den = la * lb * lc + dot(a, b) * lc + dot(b, c) * la + dot(c, a) * lb;
    2.0 * num.atan2(den)
}

/// The generalised winding number by brute force: `w = (1/4π) Σ_t Ω_t`. Exact, `O(F)` per query —
/// the reference the accelerated path must match.
pub fn winding_brute(soup: &TriSoup, p: [f64; 3]) -> f64 {
    let mut s = 0.0;
    for t in &soup.tris {
        s += solid_angle(
            sub(soup.verts[t[0]], p),
            sub(soup.verts[t[1]], p),
            sub(soup.verts[t[2]], p),
        );
    }
    s / (4.0 * PI)
}

/// The signed volume enclosed by a mesh via the divergence theorem: `V = (1/6) Σ_t p₀·(p₁×p₂)`.
/// Positive for an outward-oriented watertight mesh. An independent cross-check of the voxelised
/// volume (catches scale errors, inverted orientation, and gross classification bugs in one number).
pub fn signed_volume(soup: &TriSoup) -> f64 {
    let mut v = 0.0;
    for t in &soup.tris {
        v += dot(soup.verts[t[0]], cross(soup.verts[t[1]], soup.verts[t[2]]));
    }
    v / 6.0
}

/// Edge-manifold classification: `(open_edges, flipped, watertight)`. Watertight ⇔ every edge is
/// shared by exactly two triangles that traverse it in opposite directions.
fn edge_stats(soup: &TriSoup) -> (usize, usize, bool) {
    use std::collections::HashMap;
    // Per undirected edge, count traversals each way.
    let mut edges: HashMap<(usize, usize), (u32, u32)> = HashMap::new();
    for t in &soup.tris {
        for &(u, v) in &[(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (key, forward) = if u < v {
                ((u, v), true)
            } else {
                ((v, u), false)
            };
            let e = edges.entry(key).or_insert((0, 0));
            if forward {
                e.0 += 1;
            } else {
                e.1 += 1;
            }
        }
    }
    let (mut open, mut flipped, mut manifold) = (0usize, 0usize, true);
    for &(f, b) in edges.values() {
        match f + b {
            1 => open += 1,             // boundary edge
            2 if f == 1 && b == 1 => {} // sound: shared, opposite directions
            2 => flipped += 1,          // (2,0)/(0,2): same direction ⇒ inconsistent
            _ => manifold = false,      // shared by three or more ⇒ non-manifold
        }
    }
    (open, flipped, open == 0 && flipped == 0 && manifold)
}

fn soup_aabb(soup: &TriSoup) -> Aabb {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for v in &soup.verts {
        for i in 0..3 {
            min[i] = min[i].min(v[i]);
            max[i] = max[i].max(v[i]);
        }
    }
    Aabb { min, max }
}

/// Far-field acceptance is **error-bounded**, not a fixed radius ratio: an internal node is
/// approximated (dipole + first- and second-moment terms) only when a conservative bound on the
/// truncation error — the first neglected term, `ERR_C · area · radius³ / dist⁶` — is below the
/// caller's `tau`. So distant and small clusters prune while the near surface recurses to exact
/// leaves, at any mesh scale. (A fixed radius ratio cannot: near, non-tiny clusters carry
/// percent-level truncation error regardless of expansion order.)
const ERR_C: f64 = 30.0;
/// Accuracy for the *accurate* winding number ([`MeshSolid::winding`]): agrees with brute force to
/// ≤1e-6 (`fast_wn_matches_brute`). Conservative — pruning helps only far-field / very large meshes.
const TAU_EXACT: f64 = 1e-9;
/// Accuracy for inside/outside *classification* ([`MeshSolid::occupancy`], threshold ½): far looser,
/// because only the sign of `w − ½` matters. Prunes aggressively — ~tens of times faster than brute
/// on a large mesh — while staying orders of magnitude away from the ½ threshold except on a
/// sub-voxel shell the boundary supersampling already averages over.
const TAU_CLASS: f64 = 1e-3;
/// Triangles per BVH leaf.
const LEAF: usize = 8;

enum NodeKind {
    Leaf { start: usize, count: usize },
    Internal { left: usize, right: usize },
}

/// One BVH node: its far-field multipole summary and either a triangle span or two children. The
/// summary is the area-weighted moments of `g = area·normal` about `centre`: dipole `p = Σ g`, first
/// moment `m = Σ g⊗δ`, second moment `tt = Σ g⊗δ⊗δ` (`δ` = triangle centroid − node centre).
struct Node {
    centre: [f64; 3],
    radius: f64,
    /// Total triangle area in the subtree — the scale of the far-field error bound.
    area: f64,
    /// Tight AABB over the subtree's triangle vertices — for ray-cast (parity) pruning.
    bmin: [f64; 3],
    bmax: [f64; 3],
    p: [f64; 3],
    m: [[f64; 3]; 3],
    tt: [[[f64; 3]; 3]; 3],
    kind: NodeKind,
}

/// A bounding-volume hierarchy over a soup's triangles, carrying the per-node multipole summaries the
/// fast winding number needs. Brute force is `O(F)` per query; this prunes distant clusters.
struct Bvh {
    nodes: Vec<Node>,
    order: Vec<usize>,
    centroid: Vec<[f64; 3]>,
    /// Area-weighted normal `g = ½(B−A)×(C−A)`; `|g|` is the triangle area, `g/|g|` its normal.
    area_vec: Vec<[f64; 3]>,
}

impl Bvh {
    fn build(soup: &TriSoup) -> Bvh {
        let n = soup.tris.len();
        let mut centroid = Vec::with_capacity(n);
        let mut area_vec = Vec::with_capacity(n);
        for t in &soup.tris {
            let (a, b, c) = (soup.verts[t[0]], soup.verts[t[1]], soup.verts[t[2]]);
            centroid.push([
                (a[0] + b[0] + c[0]) / 3.0,
                (a[1] + b[1] + c[1]) / 3.0,
                (a[2] + b[2] + c[2]) / 3.0,
            ]);
            let g = cross(sub(b, a), sub(c, a));
            area_vec.push([0.5 * g[0], 0.5 * g[1], 0.5 * g[2]]);
        }
        let mut bvh = Bvh {
            nodes: Vec::new(),
            order: (0..n).collect(),
            centroid,
            area_vec,
        };
        if n > 0 {
            bvh.build_node(soup, 0, n);
        }
        bvh
    }

    /// Build the node spanning `order[lo..hi]`, returning its index. Splits on the longest centroid
    /// axis at the median.
    fn build_node(&mut self, soup: &TriSoup, lo: usize, hi: usize) -> usize {
        // Multipole summary: dipole P = Σ g, area-weighted centre, first moment M = Σ g⊗(cᵢ−centre).
        let mut p = [0.0; 3];
        let mut wsum = 0.0;
        let mut cw = [0.0; 3];
        for &t in &self.order[lo..hi] {
            let g = self.area_vec[t];
            let a = norm(g);
            for k in 0..3 {
                p[k] += g[k];
                cw[k] += a * self.centroid[t][k];
            }
            wsum += a;
        }
        let centre = if wsum > 0.0 {
            [cw[0] / wsum, cw[1] / wsum, cw[2] / wsum]
        } else {
            self.centroid[self.order[lo]]
        };
        let mut m = [[0.0; 3]; 3];
        let mut tt = [[[0.0; 3]; 3]; 3];
        let mut radius = 0.0f64;
        let mut bmin = [f64::INFINITY; 3];
        let mut bmax = [f64::NEG_INFINITY; 3];
        for &t in &self.order[lo..hi] {
            let g = self.area_vec[t];
            let d = sub(self.centroid[t], centre);
            for a in 0..3 {
                for b in 0..3 {
                    m[a][b] += g[a] * d[b];
                    for c in 0..3 {
                        tt[a][b][c] += g[a] * d[b] * d[c];
                    }
                }
            }
            for &vi in &soup.tris[t] {
                let v = soup.verts[vi];
                radius = radius.max(norm(sub(v, centre)));
                for k in 0..3 {
                    bmin[k] = bmin[k].min(v[k]);
                    bmax[k] = bmax[k].max(v[k]);
                }
            }
        }

        let kind = if hi - lo <= LEAF {
            NodeKind::Leaf {
                start: lo,
                count: hi - lo,
            }
        } else {
            // Split on the longest axis of the centroid bounds, at the median.
            let mut cmin = [f64::INFINITY; 3];
            let mut cmax = [f64::NEG_INFINITY; 3];
            for &t in &self.order[lo..hi] {
                for k in 0..3 {
                    cmin[k] = cmin[k].min(self.centroid[t][k]);
                    cmax[k] = cmax[k].max(self.centroid[t][k]);
                }
            }
            let axis = (0..3)
                .max_by(|&i, &j| (cmax[i] - cmin[i]).total_cmp(&(cmax[j] - cmin[j])))
                .unwrap();
            let centroid = &self.centroid;
            self.order[lo..hi].sort_by(|&x, &y| centroid[x][axis].total_cmp(&centroid[y][axis]));
            let mid = (lo + hi) / 2;
            let left = self.build_node(soup, lo, mid);
            let right = self.build_node(soup, mid, hi);
            NodeKind::Internal { left, right }
        };

        self.nodes.push(Node {
            centre,
            radius,
            area: wsum,
            bmin,
            bmax,
            p,
            m,
            tt,
            kind,
        });
        self.nodes.len() - 1
    }

    /// The winding number `w(p) = (1/4π) Σ_t Ω_t`, evaluated hierarchically to accuracy `tau`.
    fn winding(&self, soup: &TriSoup, p: [f64; 3], tau: f64) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        self.node_sum(self.nodes.len() - 1, soup, p, tau) / (4.0 * PI)
    }

    /// Raw solid-angle sum over a node: exact at leaves; the far-field expansion for distant internal
    /// clusters whose truncation-error bound is below `tau`; recursion otherwise.
    fn node_sum(&self, i: usize, soup: &TriSoup, p: [f64; 3], tau: f64) -> f64 {
        let nd = &self.nodes[i];
        match nd.kind {
            NodeKind::Leaf { start, count } => {
                let mut s = 0.0;
                for &t in &self.order[start..start + count] {
                    s += solid_angle(
                        sub(soup.verts[soup.tris[t][0]], p),
                        sub(soup.verts[soup.tris[t][1]], p),
                        sub(soup.verts[soup.tris[t][2]], p),
                    );
                }
                s
            }
            NodeKind::Internal { left, right } => {
                let r = sub(nd.centre, p);
                let dist = norm(r);
                let d6 = dist.powi(6);
                let err_bound = ERR_C * nd.area * nd.radius.powi(3) / d6;
                if dist > nd.radius && err_bound < tau {
                    approx_solid_angle(nd, r, dist)
                } else {
                    self.node_sum(left, soup, p, tau) + self.node_sum(right, soup, p, tau)
                }
            }
        }
    }

    /// Count forward ray–triangle crossings from `origin` along `dir`, pruning by node AABB. Parity
    /// (odd ⇒ inside) classifies a watertight mesh independently of the winding number.
    fn ray_crossings(&self, soup: &TriSoup, origin: [f64; 3], dir: [f64; 3]) -> usize {
        if self.nodes.is_empty() {
            return 0;
        }
        let inv = [1.0 / dir[0], 1.0 / dir[1], 1.0 / dir[2]];
        let mut count = 0;
        let mut stack = vec![self.nodes.len() - 1];
        while let Some(i) = stack.pop() {
            let nd = &self.nodes[i];
            if !ray_aabb(origin, inv, nd.bmin, nd.bmax) {
                continue;
            }
            match nd.kind {
                NodeKind::Leaf { start, count: c } => {
                    for &t in &self.order[start..start + c] {
                        let tri = soup.tris[t];
                        if ray_triangle(
                            origin,
                            dir,
                            soup.verts[tri[0]],
                            soup.verts[tri[1]],
                            soup.verts[tri[2]],
                        ) {
                            count += 1;
                        }
                    }
                }
                NodeKind::Internal { left, right } => {
                    stack.push(left);
                    stack.push(right);
                }
            }
        }
        count
    }
}

/// Slab test: does the forward ray `origin + t·dir` (`t > 0`, `inv = 1/dir`) meet the box?
fn ray_aabb(origin: [f64; 3], inv: [f64; 3], bmin: [f64; 3], bmax: [f64; 3]) -> bool {
    let mut tmin = 0.0f64;
    let mut tmax = f64::INFINITY;
    for k in 0..3 {
        let t1 = (bmin[k] - origin[k]) * inv[k];
        let t2 = (bmax[k] - origin[k]) * inv[k];
        let (lo, hi) = if t1 <= t2 { (t1, t2) } else { (t2, t1) };
        tmin = tmin.max(lo);
        tmax = tmax.min(hi);
    }
    tmax >= tmin
}

/// Möller–Trumbore: does the forward ray `origin + t·dir` (`t > ε`) cross triangle `(a, b, c)`?
fn ray_triangle(origin: [f64; 3], dir: [f64; 3], a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> bool {
    const EPS: f64 = 1e-12;
    let e1 = sub(b, a);
    let e2 = sub(c, a);
    let h = cross(dir, e2);
    let det = dot(e1, h);
    if det.abs() < EPS {
        return false; // ray parallel to the triangle
    }
    let inv = 1.0 / det;
    let s = sub(origin, a);
    let u = inv * dot(s, h);
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let q = cross(s, e1);
    let v = inv * dot(dir, q);
    if v < 0.0 || u + v > 1.0 {
        return false;
    }
    inv * dot(e2, q) > EPS // forward hit
}

/// Far-field approximation of a node's raw solid-angle sum: the Taylor expansion of `g·K(r+δ)` about
/// the node centre to second order in `δ`, where `K(r) = r/|r|³` is the point-source kernel and its
/// derivatives are `∂_jK_i = δ_ij/r³ − 3r_ir_j/r⁵` and `∂_j∂_kK_i = −3(δ_ij r_k + δ_ik r_j + δ_jk r_i)/r⁵ + 15 r_ir_jr_k/r⁷`.
fn approx_solid_angle(nd: &Node, r: [f64; 3], dist: f64) -> f64 {
    let s3 = dist.powi(3);
    let s5 = dist.powi(5);
    let s7 = dist.powi(7);

    // Order 0 — the dipole: K(r)·P.
    let k = [r[0] / s3, r[1] / s3, r[2] / s3];
    let order0 = dot(k, nd.p);

    // Order 1 — Σ_ij (∂_jK_i) M_ij = tr(M)/r³ − 3 rᵀMr/r⁵.
    let tr_m = nd.m[0][0] + nd.m[1][1] + nd.m[2][2];
    let mut rmr = 0.0;
    for a in 0..3 {
        for b in 0..3 {
            rmr += r[a] * nd.m[a][b] * r[b];
        }
    }
    let order1 = tr_m / s3 - 3.0 * rmr / s5;

    // Order 2 — ½ Σ_ijk (∂_j∂_kK_i) T_ijk, with T symmetric in (j,k). Four contractions of T with r.
    let tt = &nd.tt;
    let (mut ca, mut cb, mut cc, mut cd) = (0.0, 0.0, 0.0, 0.0);
    for i in 0..3 {
        for k2 in 0..3 {
            ca += tt[i][i][k2] * r[k2]; // Σ_i Σ_k T_iik r_k
            cb += tt[i][k2][i] * r[k2]; // Σ_i Σ_j T_iji r_j
            cc += tt[i][k2][k2] * r[i]; // Σ_i Σ_j T_ijj r_i
        }
    }
    for i in 0..3 {
        for j in 0..3 {
            for k2 in 0..3 {
                cd += tt[i][j][k2] * r[i] * r[j] * r[k2];
            }
        }
    }
    let order2 = 0.5 * (-3.0 * (ca + cb + cc) / s5 + 15.0 * cd / s7);

    order0 + order1 + order2
}

/// An imported mesh as a `Solid`: inside/outside by the generalised winding number, thresholded at ½.
/// Robust on open and non-manifold meshes (a watertight-only test fails on dirty input).
pub struct MeshSolid {
    soup: TriSoup,
    bvh: Bvh,
    aabb: Aabb,
    watertight: bool,
}

impl MeshSolid {
    /// Build a mesh solid from a soup (already scaled): classify its edges, bound it, and build the
    /// BVH. Returns the solid and its report. `EmptySolid` if the soup has no triangles.
    pub fn from_soup(soup: TriSoup) -> Result<(MeshSolid, MeshReport), ShapeError> {
        if soup.is_empty() {
            return Err(ShapeError::EmptySolid);
        }
        let (open_edges, flipped, watertight) = edge_stats(&soup);
        let report = MeshReport {
            faces: soup.len(),
            open_edges,
            flipped,
            watertight,
            volume: signed_volume(&soup),
            ambiguity: None,
        };
        let aabb = soup_aabb(&soup);
        let bvh = Bvh::build(&soup);
        Ok((
            MeshSolid {
                soup,
                bvh,
                aabb,
                watertight,
            },
            report,
        ))
    }

    /// The generalised winding number at `p`, accurate (agrees with brute force to ≤1e-6).
    pub fn winding(&self, p: [f64; 3]) -> f64 {
        self.bvh.winding(&self.soup, p, TAU_EXACT)
    }

    /// Inside/outside at `p` by the winding number thresholded at ½, at classification accuracy
    /// (fast: only the sign of `w − ½` is needed). The occupancy seam the voxeliser samples.
    pub fn inside(&self, p: [f64; 3]) -> bool {
        self.bvh.winding(&self.soup, p, TAU_CLASS) > 0.5
    }
}

// ── The two classifiers over a lattice: the watertight fast path and the robust winding path. ──

/// A cubic lattice over a bounding box — the classification and voxelisation domain. Mirrors the
/// lattice `crate::voxelise` builds (same pitch rounding, same cell centres), so a mesh classified
/// here voxelises identically to a primitive.
struct Lattice {
    origin: [f64; 3],
    h: f64,
    dims: [usize; 3],
}

impl Lattice {
    fn new(bbox: Aabb, h: f64) -> Lattice {
        let dims = [
            ((bbox.max[0] - bbox.min[0]) / h).ceil().max(1.0) as usize,
            ((bbox.max[1] - bbox.min[1]) / h).ceil().max(1.0) as usize,
            ((bbox.max[2] - bbox.min[2]) / h).ceil().max(1.0) as usize,
        ];
        Lattice {
            origin: bbox.min,
            h,
            dims,
        }
    }

    fn count(&self) -> usize {
        self.dims[0] * self.dims[1] * self.dims[2]
    }

    fn index(&self, i: usize, j: usize, k: usize) -> usize {
        i + self.dims[0] * (j + self.dims[1] * k)
    }

    fn centre(&self, i: usize, j: usize, k: usize) -> [f64; 3] {
        [
            self.origin[0] + (i as f64 + 0.5) * self.h,
            self.origin[1] + (j as f64 + 0.5) * self.h,
            self.origin[2] + (k as f64 + 0.5) * self.h,
        ]
    }
}

/// The tri-state a cell falls into under the watertight fast path.
#[derive(Clone, Copy, PartialEq)]
enum Cell {
    Outside,
    Inside,
    Boundary,
}

/// The watertight fast path: conservatively rasterise the surface, flood-fill the exterior from the
/// bbox margin, and sub-sample boundary cells by BVH parity rays. Returns per-cell occupancy and the
/// boundary mask (boundary cells are excluded from `flood_vs_wn`, which compares the definite cells).
fn classify_watertight(
    soup: &TriSoup,
    bvh: &Bvh,
    lattice: &Lattice,
    supersample: usize,
) -> (Vec<f64>, Vec<bool>) {
    let n = lattice.count();
    let [nx, ny, nz] = lattice.dims;

    // 1. Conservative surface rasterisation: mark every cell overlapping a triangle's AABB.
    let mut state = vec![Cell::Inside; n];
    for t in &soup.tris {
        let mut lo = [usize::MAX; 3];
        let mut hi = [0usize; 3];
        let mut tmin = [f64::INFINITY; 3];
        let mut tmax = [f64::NEG_INFINITY; 3];
        for &vi in t {
            for k in 0..3 {
                tmin[k] = tmin[k].min(soup.verts[vi][k]);
                tmax[k] = tmax[k].max(soup.verts[vi][k]);
            }
        }
        for k in 0..3 {
            let a = ((tmin[k] - lattice.origin[k]) / lattice.h).floor();
            let b = ((tmax[k] - lattice.origin[k]) / lattice.h).floor();
            lo[k] = a.max(0.0) as usize;
            hi[k] = (b.max(0.0) as usize).min(lattice.dims[k] - 1);
        }
        for k in lo[2]..=hi[2] {
            for j in lo[1]..=hi[1] {
                for i in lo[0]..=hi[0] {
                    state[lattice.index(i, j, k)] = Cell::Boundary;
                }
            }
        }
    }

    // 2. Flood-fill the exterior from the bbox margin through non-boundary cells (6-connectivity).
    let mut stack: Vec<(usize, usize, usize)> = Vec::new();
    let push_seed = |i, j, k, state: &mut [Cell], stack: &mut Vec<_>| {
        let idx = lattice.index(i, j, k);
        if state[idx] == Cell::Inside {
            state[idx] = Cell::Outside;
            stack.push((i, j, k));
        }
    };
    for j in 0..ny {
        for i in 0..nx {
            push_seed(i, j, 0, &mut state, &mut stack);
            push_seed(i, j, nz - 1, &mut state, &mut stack);
        }
    }
    for k in 0..nz {
        for i in 0..nx {
            push_seed(i, 0, k, &mut state, &mut stack);
            push_seed(i, ny - 1, k, &mut state, &mut stack);
        }
    }
    for k in 0..nz {
        for j in 0..ny {
            push_seed(0, j, k, &mut state, &mut stack);
            push_seed(nx - 1, j, k, &mut state, &mut stack);
        }
    }
    while let Some((i, j, k)) = stack.pop() {
        let visit = |i: usize, j: usize, k: usize, state: &mut [Cell], stack: &mut Vec<_>| {
            let idx = lattice.index(i, j, k);
            if state[idx] == Cell::Inside {
                state[idx] = Cell::Outside;
                stack.push((i, j, k));
            }
        };
        if i > 0 {
            visit(i - 1, j, k, &mut state, &mut stack);
        }
        if i + 1 < nx {
            visit(i + 1, j, k, &mut state, &mut stack);
        }
        if j > 0 {
            visit(i, j - 1, k, &mut state, &mut stack);
        }
        if j + 1 < ny {
            visit(i, j + 1, k, &mut state, &mut stack);
        }
        if k > 0 {
            visit(i, j, k - 1, &mut state, &mut stack);
        }
        if k + 1 < nz {
            visit(i, j, k + 1, &mut state, &mut stack);
        }
    }

    // 3. Occupancy: interior 1, exterior 0, boundary = fraction of k³ sub-samples inside by parity ray.
    let k = supersample.max(1);
    // A fixed, slightly skew ray direction so it rarely grazes an edge or vertex.
    let dir = [0.5731, 0.3319, 0.7490];
    let mut occ = vec![0.0f64; n];
    let mut boundary = vec![false; n];
    for kk in 0..nz {
        for jj in 0..ny {
            for ii in 0..nx {
                let idx = lattice.index(ii, jj, kk);
                match state[idx] {
                    Cell::Inside => occ[idx] = 1.0,
                    Cell::Outside => occ[idx] = 0.0,
                    Cell::Boundary => {
                        boundary[idx] = true;
                        let c = lattice.centre(ii, jj, kk);
                        let mut inside = 0;
                        for a in 0..k {
                            let ox = ((a as f64 + 0.5) / k as f64 - 0.5) * lattice.h;
                            for b in 0..k {
                                let oy = ((b as f64 + 0.5) / k as f64 - 0.5) * lattice.h;
                                for d in 0..k {
                                    let oz = ((d as f64 + 0.5) / k as f64 - 0.5) * lattice.h;
                                    let p = [c[0] + ox, c[1] + oy, c[2] + oz];
                                    if bvh.ray_crossings(soup, p, dir) % 2 == 1 {
                                        inside += 1;
                                    }
                                }
                            }
                        }
                        occ[idx] = inside as f64 / (k * k * k) as f64;
                    }
                }
            }
        }
    }
    (occ, boundary)
}

/// The ambiguity threshold: above this, the robust classifier's mean `min(|w|, |w−1|)` marks the
/// interior as genuinely undecidable and `load`/voxelisation fails loudly rather than emitting a
/// silent garbage cloud. A sound (even lightly open) mesh sits well below; a broken one well above.
const A_MAX: f64 = 0.15;

/// The robust path (open/non-manifold): per-cell occupancy by the winding number, with the ambiguity
/// diagnostic `A = mean min(|w|, |w−1|)` over cell centres. `A > A_MAX ⇒ AmbiguousInterior` — the
/// interior is undecidable, so no cloud is emitted (never a silent garbage cloud). With `supersample
/// = 1` the occupancy is the binary cell-centre classification; otherwise each cell is the fraction
/// of its `k³` sub-samples inside. Returns the occupancy grid and `A`.
fn classify_winding(
    soup: &TriSoup,
    bvh: &Bvh,
    lattice: &Lattice,
    supersample: usize,
) -> Result<(Vec<f64>, f64), ShapeError> {
    let n = lattice.count();
    let k = supersample.max(1);
    let mut occ = vec![0.0f64; n];
    let mut amb = 0.0;
    for kk in 0..lattice.dims[2] {
        for jj in 0..lattice.dims[1] {
            for ii in 0..lattice.dims[0] {
                let c = lattice.centre(ii, jj, kk);
                let wc = bvh.winding(soup, c, TAU_CLASS);
                amb += wc.abs().min((wc - 1.0).abs());
                let idx = lattice.index(ii, jj, kk);
                occ[idx] = if k == 1 {
                    f64::from(wc > 0.5)
                } else {
                    let mut inside = 0;
                    for a in 0..k {
                        let ox = ((a as f64 + 0.5) / k as f64 - 0.5) * lattice.h;
                        for b in 0..k {
                            let oy = ((b as f64 + 0.5) / k as f64 - 0.5) * lattice.h;
                            for d in 0..k {
                                let oz = ((d as f64 + 0.5) / k as f64 - 0.5) * lattice.h;
                                let p = [c[0] + ox, c[1] + oy, c[2] + oz];
                                if bvh.winding(soup, p, TAU_CLASS) > 0.5 {
                                    inside += 1;
                                }
                            }
                        }
                    }
                    inside as f64 / (k * k * k) as f64
                };
            }
        }
    }
    let a = amb / n as f64;
    if a > A_MAX {
        return Err(ShapeError::AmbiguousInterior(a));
    }
    Ok((occ, a))
}

/// Voxelise a mesh through the **same** pipeline as a primitive: build the lattice, classify it (the
/// watertight fast path or the robust winding path, which may fail loudly with `AmbiguousInterior`),
/// then emit the cloud through the shared [`crate::finish_cloud`] tail — so the result is
/// indistinguishable downstream (exact mass, zero dipole, canonical x-fastest order).
pub fn voxelise_mesh(
    mesh: &MeshSolid,
    params: &VoxelParams,
    mass: MassSpec,
) -> Result<Cloud, ShapeError> {
    let h = crate::pitch_for(&mesh.aabb, params)?;
    let lattice = Lattice::new(mesh.aabb, h);
    let k = params.supersample.max(1) as usize;
    let occ = if mesh.watertight {
        classify_watertight(&mesh.soup, &mesh.bvh, &lattice, k).0
    } else {
        classify_winding(&mesh.soup, &mesh.bvh, &lattice, k)?.0
    };

    let cell = h * h * h;
    let (mut xs, mut ys, mut zs, mut vols) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for kk in 0..lattice.dims[2] {
        for jj in 0..lattice.dims[1] {
            for ii in 0..lattice.dims[0] {
                let o = occ[lattice.index(ii, jj, kk)];
                if o > 0.0 {
                    let c = lattice.centre(ii, jj, kk);
                    xs.push(c[0]);
                    ys.push(c[1]);
                    zs.push(c[2]);
                    vols.push(o * cell);
                    if xs.len() > crate::ELEMENT_CAP {
                        return Err(ShapeError::TooManyElements);
                    }
                }
            }
        }
    }
    crate::finish_cloud(xs, ys, zs, vols, mass)
}

impl Solid for MeshSolid {
    fn occupancy(&self, p: [f64; 3]) -> f64 {
        if self.inside(p) {
            1.0
        } else {
            0.0
        }
    }
    fn bbox(&self) -> Aabb {
        self.aabb
    }
}

/// Re-express a cloud in its **principal frame**: rotate every element by `Rᵀ`, where the columns of
/// `R` are the principal axes (eigenvectors of the second moment `C`, from [`gravity::inertia`]). In
/// the returned cloud the inertia tensor is diagonal, so `Orient::FreeRotation` — which reads the
/// principal moments off the body-frame diagonal (M4) — holds for an imported mesh whose authored
/// frame is not principal. The rotation `R` (body ← principal) is returned and **recorded** so a
/// caller can compose it into the initial pose to recover the authored orientation.
///
/// This is M10-R7's resolution, path (a): diagonalise-and-author-in-principal-frame. It is the only
/// place the shape crate rotates a body's axes; it is explicit and recorded, never silent. Total mass
/// and the (origin) CoM are preserved — a rotation about the origin moves neither.
pub fn principal_frame(cloud: &Cloud) -> (Cloud, [[f64; 3]; 3]) {
    let mut r = gravity::inertia(cloud).axes.m;
    // Keep a proper rotation (det = +1): a reflected eigenbasis would mirror the body. Flip the third
    // axis if the permuted eigenvectors form a left-handed frame.
    if det3(&r) < 0.0 {
        for row in r.iter_mut() {
            row[2] = -row[2];
        }
    }
    let n = cloud.len();
    let mut xs = Vec::with_capacity(n);
    let mut ys = Vec::with_capacity(n);
    let mut zs = Vec::with_capacity(n);
    for k in 0..n {
        let p = [cloud.xs[k], cloud.ys[k], cloud.zs[k]];
        // q = Rᵀ p — the coordinate of p along each principal axis (column of R).
        xs.push(r[0][0] * p[0] + r[1][0] * p[1] + r[2][0] * p[2]);
        ys.push(r[0][1] * p[0] + r[1][1] * p[1] + r[2][1] * p[2]);
        zs.push(r[0][2] * p[0] + r[1][2] * p[1] + r[2][2] * p[2]);
    }
    (
        Cloud {
            xs,
            ys,
            zs,
            ms: cloud.ms.clone(),
        },
        r,
    )
}

fn det3(m: &[[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// A content-addressed cache key for a mesh at a given voxelisation: the triangle data plus the
/// lattice parameters (`design/shape.md` §7). Feeds `Registry::resolve` so the same (mesh, h)
/// voxelises once and mass draws only rescale — the cache honoured for meshes as for primitives.
pub fn mesh_cache_key(soup: &TriSoup, params: &VoxelParams) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for v in &soup.verts {
        for &c in v {
            c.to_bits().hash(&mut h);
        }
    }
    for t in &soup.tris {
        t.hash(&mut h);
    }
    params.pitch.map(f64::to_bits).hash(&mut h);
    params.target_n.hash(&mut h);
    params.supersample.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit cube [-0.5, 0.5]³ as 12 outward-oriented triangles (8 shared vertices).
    pub(super) fn unit_cube() -> TriSoup {
        let verts = vec![
            [-0.5, -0.5, -0.5],
            [0.5, -0.5, -0.5],
            [0.5, 0.5, -0.5],
            [-0.5, 0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
            [-0.5, 0.5, 0.5],
        ];
        // Outward-facing (CCW seen from outside) triangles for each of the six faces.
        let tris = vec![
            [0, 2, 1],
            [0, 3, 2], // −z
            [4, 5, 6],
            [4, 6, 7], // +z
            [0, 1, 5],
            [0, 5, 4], // −y
            [2, 3, 7],
            [2, 7, 6], // +y
            [0, 4, 7],
            [0, 7, 3], // −x
            [1, 2, 6],
            [1, 6, 5], // +x
        ];
        TriSoup { verts, tris }
    }

    /// A unit-radius icosphere as a watertight, outward-oriented soup (`20·4^subdiv` triangles).
    pub(super) fn icosphere(subdiv: u32) -> TriSoup {
        use std::collections::HashMap;
        let t = (1.0 + 5.0f64.sqrt()) / 2.0;
        let mut verts: Vec<[f64; 3]> = [
            [-1.0, t, 0.0],
            [1.0, t, 0.0],
            [-1.0, -t, 0.0],
            [1.0, -t, 0.0],
            [0.0, -1.0, t],
            [0.0, 1.0, t],
            [0.0, -1.0, -t],
            [0.0, 1.0, -t],
            [t, 0.0, -1.0],
            [t, 0.0, 1.0],
            [-t, 0.0, -1.0],
            [-t, 0.0, 1.0],
        ]
        .into_iter()
        .map(|v| {
            let n = norm(v);
            [v[0] / n, v[1] / n, v[2] / n]
        })
        .collect();
        // The 20 icosahedron faces, wound outward (CCW seen from outside).
        let mut tris: Vec<[usize; 3]> = vec![
            [0, 11, 5],
            [0, 5, 1],
            [0, 1, 7],
            [0, 7, 10],
            [0, 10, 11],
            [1, 5, 9],
            [5, 11, 4],
            [11, 10, 2],
            [10, 7, 6],
            [7, 1, 8],
            [3, 9, 4],
            [3, 4, 2],
            [3, 2, 6],
            [3, 6, 8],
            [3, 8, 9],
            [4, 9, 5],
            [2, 4, 11],
            [6, 2, 10],
            [8, 6, 7],
            [9, 8, 1],
        ];
        for _ in 0..subdiv {
            let mut cache: HashMap<(usize, usize), usize> = HashMap::new();
            let mut mid = |a: usize, b: usize, verts: &mut Vec<[f64; 3]>| {
                let key = if a < b { (a, b) } else { (b, a) };
                *cache.entry(key).or_insert_with(|| {
                    let (va, vb) = (verts[a], verts[b]);
                    let m = [
                        (va[0] + vb[0]) / 2.0,
                        (va[1] + vb[1]) / 2.0,
                        (va[2] + vb[2]) / 2.0,
                    ];
                    let n = norm(m);
                    verts.push([m[0] / n, m[1] / n, m[2] / n]);
                    verts.len() - 1
                })
            };
            let mut next = Vec::with_capacity(tris.len() * 4);
            for f in &tris {
                let a = mid(f[0], f[1], &mut verts);
                let b = mid(f[1], f[2], &mut verts);
                let c = mid(f[2], f[0], &mut verts);
                next.push([f[0], a, c]);
                next.push([f[1], b, a]);
                next.push([f[2], c, b]);
                next.push([a, b, c]);
            }
            tris = next;
        }
        TriSoup { verts, tris }
    }

    /// A deterministic pseudo-random point in `[-2, 2]³` (no rng dependency).
    fn scatter(i: u64) -> [f64; 3] {
        let h = |salt: u64| {
            let mut x = i.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(salt);
            x ^= x >> 33;
            x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
            x ^= x >> 33;
            4.0 * (x >> 11) as f64 / (1u64 << 53) as f64 - 2.0
        };
        [h(0x1), h(0x2), h(0x3)]
    }

    #[test]
    fn solid_angle_closed_cube() {
        // w on the 12-triangle cube: 1 inside, 0 outside, ≈½ at a face centre on the surface.
        let (cube, _) = MeshSolid::from_soup(unit_cube()).unwrap();
        assert!(
            (cube.winding([0.0, 0.0, 0.0]) - 1.0).abs() <= 1e-10,
            "inside"
        );
        assert!(cube.winding([2.0, 2.0, 2.0]).abs() <= 1e-10, "outside");
        assert!(
            (cube.winding([0.0, 0.0, 0.5]) - 0.5).abs() <= 1e-3,
            "on +z face centre"
        );
    }

    #[test]
    fn watertight_check() {
        // A sound cube passes; a punctured one is flagged with open_edge count = the hole boundary.
        let (_, sound) = MeshSolid::from_soup(unit_cube()).unwrap();
        assert!(sound.watertight);
        assert_eq!(sound.open_edges, 0);
        assert_eq!(sound.flipped, 0);

        let mut punctured = unit_cube();
        punctured.tris.pop(); // remove one triangle ⇒ a triangular hole (3 boundary edges)
        let (_, rep) = MeshSolid::from_soup(punctured).unwrap();
        assert!(!rep.watertight);
        assert_eq!(rep.open_edges, 3, "hole boundary is three edges");
    }

    #[test]
    fn fast_wn_matches_brute() {
        // The BVH+far-field expansion must agree with brute force to ≤1e-6 on a large mesh.
        let soup = icosphere(4); // 20·4⁴ = 5120 triangles (order 10⁴)
        let (solid, _) = MeshSolid::from_soup(soup.clone()).unwrap();
        let mut worst = 0.0f64;
        for i in 0..1000 {
            let p = scatter(i);
            let diff = (solid.winding(p) - winding_brute(&soup, p)).abs();
            worst = worst.max(diff);
        }
        assert!(worst <= 1e-6, "fast WN vs brute worst {worst}");
    }

    #[test]
    fn flood_vs_wn() {
        // On a watertight mesh the fast (flood-fill + parity ray) and robust (winding) classifiers
        // must agree cell-for-cell away from the boundary shell — two independent inside tests.
        let soup = icosphere(3); // 1280 triangles
        let bvh = Bvh::build(&soup);
        let bbox = soup_aabb(&soup);
        let h = (bbox.max[0] - bbox.min[0]) / 40.0;
        let lat = Lattice::new(bbox, h);
        let (occ_wt, boundary) = classify_watertight(&soup, &bvh, &lat, 1);
        let (occ_wn, _a) = classify_winding(&soup, &bvh, &lat, 1).unwrap();

        let (mut agree, mut total) = (0usize, 0usize);
        for idx in 0..lat.count() {
            if boundary[idx] {
                continue; // definite cells only — the boundary is where the two legitimately differ
            }
            total += 1;
            if (occ_wt[idx] > 0.5) == (occ_wn[idx] > 0.5) {
                agree += 1;
            }
        }
        let frac = agree as f64 / total as f64;
        assert!(
            frac >= 0.999,
            "flood vs winding agree {frac} over {total} cells"
        );
    }

    #[test]
    fn volume_crosscheck() {
        // The voxelised volume matches the divergence-theorem volume to ≤1% at h = bbox/50 —
        // one number catching scale errors, inverted orientation, and gross classification bugs.
        let soup = icosphere(2); // 320 triangles
        let bvh = Bvh::build(&soup);
        let bbox = soup_aabb(&soup);
        let h = (bbox.max[0] - bbox.min[0]) / 50.0;
        let lat = Lattice::new(bbox, h);
        let (occ, _b) = classify_watertight(&soup, &bvh, &lat, 3);
        let v_voxel: f64 = occ.iter().sum::<f64>() * h.powi(3);
        let v_mesh = signed_volume(&soup);
        assert!(v_mesh > 0.0, "outward orientation ⇒ positive volume");
        let rel = (v_voxel - v_mesh).abs() / v_mesh;
        assert!(
            rel <= 0.01,
            "voxel {v_voxel} vs divergence {v_mesh}: rel {rel}"
        );
    }

    fn rel_frob(a: &math::Mat3<f64>, b: &math::Mat3<f64>) -> f64 {
        let (mut num, mut den) = (0.0, 0.0);
        for i in 0..3 {
            for j in 0..3 {
                num += (a.m[i][j] - b.m[i][j]).powi(2);
                den += b.m[i][j].powi(2);
            }
        }
        (num / den).sqrt()
    }

    /// An asymmetric box (half-extents `h`) tilted out of its principal frame, so its inertia tensor
    /// is genuinely non-diagonal in the authored frame — the case M10-R7 must resolve.
    fn tilted_box(h: [f64; 3]) -> TriSoup {
        let mut s = unit_cube();
        let (a, b) = (0.5f64, 0.7f64); // yaw, then pitch — an arbitrary tilt
        let (ca, sa, cb, sb) = (a.cos(), a.sin(), b.cos(), b.sin());
        for v in &mut s.verts {
            let p = [v[0] * 2.0 * h[0], v[1] * 2.0 * h[1], v[2] * 2.0 * h[2]]; // ±0.5 → ±h
            let z = [ca * p[0] - sa * p[1], sa * p[0] + ca * p[1], p[2]]; // about z
            *v = [z[0], cb * z[1] - sb * z[2], sb * z[1] + cb * z[2]]; // about x
        }
        s
    }

    #[test]
    fn mesh_eq_primitive() {
        // An icosphere mesh voxelises to the primitive sphere's second moment at equal h — proof the
        // mesh goes through the identical shape pipeline. Tolerance: 2% voxelisation + facet term.
        let soup = icosphere(4); // vertices on the unit sphere; a near-sphere polyhedron
        let (mesh, _) = MeshSolid::from_soup(soup).unwrap();
        let cloud = voxelise_mesh(&mesh, &VoxelParams::pitch(0.05), MassSpec::Total(7.0)).unwrap();
        let c_mesh = crate::second_moment(&cloud);
        let c_prim = crate::Sphere { r: 1.0 }.analytic_c(7.0);
        let rel = rel_frob(&c_mesh, &c_prim);
        assert!(rel <= 0.03, "mesh sphere C vs primitive: rel {rel}");
    }

    #[test]
    fn cache_once() {
        // The same (mesh, h) voxelises once: a second resolve returns the same Arc; a mass draw only
        // rescales (M2's cache honoured for meshes as for primitives).
        use std::sync::Arc;
        let soup = icosphere(2);
        let (mesh, _) = MeshSolid::from_soup(soup.clone()).unwrap();
        let params = VoxelParams::pitch(0.1);
        let key = mesh_cache_key(&soup, &params);
        let reg = crate::Registry::new();
        let build = || voxelise_mesh(&mesh, &params, MassSpec::Total(1.0));
        let a = reg.resolve(key, build).unwrap();
        let b = reg.resolve(key, build).unwrap();
        assert!(Arc::ptr_eq(&a, &b), "cache miss on repeat resolve");

        let scaled = crate::scale_mass(&a, 5.0);
        assert!((scaled.ms.iter().sum::<f64>() - 5.0).abs() / 5.0 <= 1e-12);
        assert_eq!(scaled.xs, a.xs); // positions unchanged — no re-voxelise
    }

    #[test]
    fn principal_frame_diagonalises() {
        // A tilted asymmetric mesh has a non-diagonal inertia in its authored frame; principal_frame
        // rotates the cloud so the second moment is diagonal — M10-R7's precondition made to hold.
        let soup = tilted_box([0.35, 0.2, 0.12]);
        let (mesh, _) = MeshSolid::from_soup(soup).unwrap();
        let cloud = voxelise_mesh(&mesh, &VoxelParams::pitch(0.02), MassSpec::Total(1.0)).unwrap();

        let c0 = crate::second_moment(&cloud);
        let off0 = c0.m[0][1].abs() + c0.m[0][2].abs() + c0.m[1][2].abs();
        assert!(off0 > 1e-3, "authored frame is non-principal (off {off0})");

        let (pc, _r) = principal_frame(&cloud);
        let c1 = crate::second_moment(&pc);
        let off1 = c1.m[0][1].abs() + c1.m[0][2].abs() + c1.m[1][2].abs();
        let diag = c1.m[0][0].abs() + c1.m[1][1].abs() + c1.m[2][2].abs();
        assert!(
            off1 / diag < 1e-6,
            "principal frame diagonalises C (off {off1}, diag {diag})"
        );
    }

    #[test]
    fn scale_mandatory() {
        // A mesh without an explicit scale is refused before any file is touched.
        let import = MeshImport {
            path: PathBuf::from("does-not-exist.stl"),
            scale: None,
            voxel: VoxelParams::pitch(0.1),
            mass: MassSpec::Total(1.0),
        };
        assert!(matches!(load_solid(&import), Err(ShapeError::ScaleMissing)));
    }

    #[cfg(feature = "stl")]
    #[test]
    fn stl_roundtrip() {
        let cube = unit_cube();
        let triangles = cube.tris.iter().map(|t| {
            let v = [cube.verts[t[0]], cube.verts[t[1]], cube.verts[t[2]]];
            stl_io::Triangle {
                normal: stl_io::Normal::new([0.0, 0.0, 0.0]),
                vertices: [
                    stl_io::Vertex::new([v[0][0] as f32, v[0][1] as f32, v[0][2] as f32]),
                    stl_io::Vertex::new([v[1][0] as f32, v[1][1] as f32, v[1][2] as f32]),
                    stl_io::Vertex::new([v[2][0] as f32, v[2][1] as f32, v[2][2] as f32]),
                ],
            }
        });
        let mut bytes = Vec::new();
        stl_io::write_stl(&mut bytes, triangles).unwrap();
        let soup = parse_stl(&bytes).unwrap();
        assert_eq!(soup.len(), 12, "cube round-trips to 12 triangles");
    }

    #[cfg(feature = "obj")]
    #[test]
    fn obj_roundtrip() {
        let cube = unit_cube();
        let mut obj = String::new();
        for v in &cube.verts {
            obj.push_str(&format!("v {} {} {}\n", v[0], v[1], v[2]));
        }
        for t in &cube.tris {
            // OBJ indices are 1-based.
            obj.push_str(&format!("f {} {} {}\n", t[0] + 1, t[1] + 1, t[2] + 1));
        }
        let soup = parse_obj(obj.as_bytes()).unwrap();
        assert_eq!(soup.len(), 12, "cube round-trips to 12 triangles");
    }

    #[cfg(feature = "gltf")]
    #[test]
    fn gltf_roundtrip() {
        let bytes = glb_from_soup(&unit_cube());
        let soup = parse_gltf(&bytes).unwrap();
        assert_eq!(soup.len(), 12, "cube round-trips to 12 triangles");
    }

    /// Assemble a minimal self-contained GLB (non-indexed POSITION only) from a soup — no fixture
    /// file. Mirrors the STL/OBJ in-memory writers so all three parsers round-trip symmetrically.
    #[cfg(feature = "gltf")]
    fn glb_from_soup(soup: &TriSoup) -> Vec<u8> {
        let mut pos: Vec<f32> = Vec::new();
        for t in &soup.tris {
            for &vi in t {
                let v = soup.verts[vi];
                pos.extend([v[0] as f32, v[1] as f32, v[2] as f32]);
            }
        }
        let count = pos.len() / 3;
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for c in pos.chunks_exact(3) {
            for i in 0..3 {
                min[i] = min[i].min(c[i]);
                max[i] = max[i].max(c[i]);
            }
        }
        let bin: Vec<u8> = pos.iter().flat_map(|f| f.to_le_bytes()).collect();
        let f3 = |a: [f32; 3]| format!("{},{},{}", a[0], a[1], a[2]);
        let json = format!(
            "{{\"asset\":{{\"version\":\"2.0\"}},\
             \"buffers\":[{{\"byteLength\":{bl}}}],\
             \"bufferViews\":[{{\"buffer\":0,\"byteOffset\":0,\"byteLength\":{bl}}}],\
             \"accessors\":[{{\"bufferView\":0,\"componentType\":5126,\"count\":{count},\
             \"type\":\"VEC3\",\"min\":[{mn}],\"max\":[{mx}]}}],\
             \"meshes\":[{{\"primitives\":[{{\"attributes\":{{\"POSITION\":0}}}}]}}]}}",
            bl = bin.len(),
            count = count,
            mn = f3(min),
            mx = f3(max),
        );
        // Pad each chunk to a four-byte boundary: JSON with spaces, BIN with zeros.
        let mut json_bytes = json.into_bytes();
        while !json_bytes.len().is_multiple_of(4) {
            json_bytes.push(b' ');
        }
        let mut bin_bytes = bin;
        while !bin_bytes.len().is_multiple_of(4) {
            bin_bytes.push(0);
        }
        let total = 12 + 8 + json_bytes.len() + 8 + bin_bytes.len();
        let mut glb = Vec::with_capacity(total);
        glb.extend(0x4654_6C67u32.to_le_bytes()); // "glTF"
        glb.extend(2u32.to_le_bytes()); // version 2
        glb.extend((total as u32).to_le_bytes());
        glb.extend((json_bytes.len() as u32).to_le_bytes());
        glb.extend(0x4E4F_534Au32.to_le_bytes()); // "JSON"
        glb.extend(&json_bytes);
        glb.extend((bin_bytes.len() as u32).to_le_bytes());
        glb.extend(0x004E_4942u32.to_le_bytes()); // "BIN\0"
        glb.extend(&bin_bytes);
        glb
    }
}
