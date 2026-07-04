//! Mesh import (M10): triangle-soup parsing, the mandatory-scale gate, watertightness, the
//! generalised winding number, and the two inside/outside classifiers. An imported mesh voxelises
//! through the *same* pipeline as a primitive (`crate::voxelise`), so it is indistinguishable
//! downstream.
//!
//! Design: `design/shape.md` §5. The format parsers (STL/OBJ/glTF) are feature-gated
//! (`stl`/`obj`/`gltf`) so the core engine builds without them; the geometry core — winding number,
//! watertightness, classification — is always compiled and validated against analytic truth.

use crate::{MassSpec, ShapeError, VoxelParams};
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

/// Import a mesh: apply the mandatory scale, then parse to a scaled triangle soup.
///
/// **Scale is checked first**, before any file read, so a missing scale fails loudly and cheaply.
/// (The classification and voxelisation of the soup arrive in the later M10 commits.)
pub fn load_solid(import: &MeshImport) -> Result<(TriSoup, MeshReport), ShapeError> {
    let scale = import.scale.ok_or(ShapeError::ScaleMissing)?;
    let soup = parse_path(&import.path)?.scaled(scale);
    let report = MeshReport { faces: soup.len() };
    Ok((soup, report))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit cube [-0.5, 0.5]³ as 12 outward-oriented triangles (8 shared vertices).
    #[cfg(any(feature = "stl", feature = "obj", feature = "gltf"))]
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
