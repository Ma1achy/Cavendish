//! Mesh import (M10): triangle-soup parsing, the mandatory-scale gate, watertightness, the
//! generalised winding number, and the two inside/outside classifiers. An imported mesh voxelises
//! through the *same* pipeline as a primitive (`crate::voxelise`), so it is indistinguishable
//! downstream.
//!
//! Design: `design/shape.md` §5. The format parsers (STL/OBJ/glTF) are feature-gated
//! (`stl`/`obj`/`gltf`) so the core engine builds without them; the geometry core — winding number,
//! watertightness, classification — is always compiled and validated against analytic truth.

use crate::{Aabb, MassSpec, ShapeError, Solid, VoxelParams};
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

/// Import a mesh: apply the mandatory scale, parse to a scaled triangle soup, and classify it.
///
/// **Scale is checked first**, before any file read, so a missing scale fails loudly and cheaply.
/// (Voxelisation of the resulting solid arrives in a later M10 commit.)
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
                radius = radius.max(norm(sub(soup.verts[vi], centre)));
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
        };
        let aabb = soup_aabb(&soup);
        let bvh = Bvh::build(&soup);
        Ok((MeshSolid { soup, bvh, aabb }, report))
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
