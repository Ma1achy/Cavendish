//! Importing a mesh file as a body (S19, `docs/slices/S19.md` §2): an OBJ or STL file's bytes,
//! scaled, made of a material, voxelised at a pitch, placed.
//!
//! The mesh is centred on the middle of its box and **held in the scene** as a
//! [`ShapeKind::Triangles`] when it has at most [`MAX_INLINE_TRIANGLES`] triangles, so the scene
//! needs no file beside it (in a page, where nothing is read by path, as natively). A larger mesh
//! read from a path natively is named by that path ([`ShapeKind::Mesh`]); in a page it is refused.
//! What [`body`] makes is an ordinary body, added by an ordinary `Insert`: one undo takes it out,
//! and a journal replays it without the file.
//!
//! Frame and units: the file's units times the scale are metres; the body's position is the
//! middle of the mesh's box, the world's frame.

use cav_core::voxel::mesh::Mesh;

use crate::shape::{
    Corners, MAX_INLINE_TRIANGLES, MAX_INLINE_VERTICES, MAX_SIZE, ShapeKind, ShapeSpec, Vertex,
};
use crate::{BodySpec, Motion, SceneSpec};

/// The material an imported body is made of unless another is named.
pub const DEFAULT_MATERIAL: &str = "steel";
/// How many voxels across its widest side an imported body has unless a pitch is named.
pub const DEFAULT_CELLS_ACROSS: f64 = 32.0;

/// How big the mesh is made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    /// Metres per unit of the file (1: the file is in metres).
    Scale(f64),
    /// The widest side of the mesh's box, metres: the scale is worked out from it.
    Fit(f64),
}

/// What an import is asked for, besides the file.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// How big.
    pub size: Size,
    /// The material's label (the scene's own or the library's).
    pub material: String,
    /// The voxel pitch, metres; none for the widest side over [`DEFAULT_CELLS_ACROSS`].
    pub pitch: Option<f64>,
    /// Where the middle of its box goes, metres, the world's frame.
    pub at: [f64; 3],
}

impl Default for Options {
    fn default() -> Self {
        Self { size: Size::Scale(1.0), material: DEFAULT_MATERIAL.to_owned(), pitch: None, at: [0.0; 3] }
    }
}

/// The body that importing `bytes` (the file `name`, `.obj` or `.stl`) into `scene` makes, by
/// `options`: labelled after the file, unique in the scene. `path` is the file's path when it was
/// read from one (natively), which names a mesh too large to hold in the scene.
///
/// # Errors
/// When the bytes are not a closed mesh of that format, a number is out of range, the material
/// is not found, the mesh is too large to hold and there is no path, or the body would not be a
/// valid part of the scene: each in words that name it.
pub fn body(
    scene: &SceneSpec,
    name: &str,
    bytes: &[u8],
    path: Option<&str>,
    options: &Options,
) -> Result<BodySpec, String> {
    let parsed = Mesh::parse(name, bytes, 1.0).map_err(|e| e.0)?;
    // Centred on its box, not its centroid: the box's middle of a mesh symmetric in its numbers
    // is exactly the origin, where a centroid summed from its triangles is a rounding off it, and
    // a grid one cell larger for it (S19: an imported box was a sliver taller than the cuboid).
    let bounds = cav_core::voxel::Shape::bounds(&parsed);
    let middle = bounds.centre();
    let mesh = Mesh::new(parsed.vertices.iter().map(|v| *v - middle).collect(), parsed.triangles.clone())
        .map_err(|e| e.0)?;
    let size = bounds.size();
    let widest = size.x.max(size.y).max(size.z);
    let scale = match options.size {
        Size::Scale(s) => s,
        Size::Fit(metres) => {
            if !(metres.is_finite() && metres > 0.0 && metres <= MAX_SIZE) {
                return Err(format!("fit: {metres} m must be above 0 and at most {MAX_SIZE}"));
            }
            metres / widest
        }
    };
    if !(scale.is_finite() && scale > 0.0 && scale * widest <= MAX_SIZE) {
        return Err(format!(
            "scale: {scale} makes the mesh {} m across; it must be above 0 and at most {MAX_SIZE} m",
            scale * widest
        ));
    }
    let material = crate::materials::find(&scene.materials, &options.material).ok_or_else(|| {
        format!("material: `{}` is not a material of the scene's or of the library's", options.material)
    })?;
    let pitch = options.pitch.unwrap_or(scale * widest / DEFAULT_CELLS_ACROSS);
    let kind = if mesh.triangles.len() <= MAX_INLINE_TRIANGLES && mesh.vertices.len() <= MAX_INLINE_VERTICES {
        ShapeKind::Triangles {
            vertices: mesh.vertices.iter().map(|v| Vertex((*v * scale).to_array())).collect(),
            triangles: mesh.triangles.iter().copied().map(Corners).collect(),
        }
    } else if let Some(path) = path {
        // Too large to hold: named by its path (a file's mesh is centred on its centroid when
        // it is read).
        ShapeKind::Mesh { file: path.to_owned(), scale }
    } else {
        return Err(format!(
            "{name}: {} triangles is more than a scene holds ({MAX_INLINE_TRIANGLES}); a page cannot name a file \
             by its path, so open it in the application, or simplify the mesh",
            mesh.triangles.len()
        ));
    };
    let mut shape = ShapeSpec::new(kind, pitch, 2);
    shape.material = Some(material.label.clone());
    shape.validate("shape")?;
    // The mass the material gives the mesh's own volume: not what the body weighs (its voxels
    // do), but the field holds a number that says so to the right order.
    let volume = cav_core::voxel::Shape::volume(&mesh) * scale * scale * scale;
    let stem = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let stem = stem.rsplit_once('.').map_or(stem, |(s, _)| s);
    let taken = |label: &str| scene.bodies.iter().any(|b| b.label == label);
    let base = if stem.trim().is_empty() { "mesh" } else { stem.trim() };
    let label = std::iter::once(base.to_owned())
        .chain((2..).map(|n| format!("{base} {n}")))
        .find(|l| !taken(l))
        .expect("a free label");
    Ok(BodySpec::shaped(
        &label,
        (material.density * volume).max(1e-9),
        shape,
        Motion::Static { position: options.at },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cav_core::Vec3;
    use cav_core::voxel::mesh::{box_mesh, to_stl};

    fn stl_box(half: [f64; 3]) -> Vec<u8> {
        let h = Vec3::from_array(half);
        let (v, t) = box_mesh(-h, h);
        to_stl(&v, &t)
    }

    #[test]
    fn a_box_is_held_in_the_scene_fitted_and_of_its_material() {
        // Fit 2 m: the widest side is 2 m, to rounding; held as triangles; of lead; labelled after
        // the file, uniquely. Control: a scale of 1 keeps the file's size.
        let scene = crate::dev::void();
        let bytes = stl_box([0.5, 0.25, 0.1]);
        let options = Options { size: Size::Fit(2.0), material: "lead".into(), ..Options::default() };
        let body = body(&scene, "parts/crate.stl", &bytes, None, &options).unwrap();
        assert_eq!(body.label, "crate");
        let shape = body.shape.as_ref().unwrap();
        assert_eq!(shape.material.as_deref(), Some("lead"));
        let bounds = shape.kind.bounds().unwrap();
        assert!((bounds.size().x - 2.0).abs() < 1e-12, "{:?}", bounds);
        assert!((shape.pitch - 2.0 / DEFAULT_CELLS_ACROSS).abs() < 1e-15);
        assert!(matches!(shape.kind, ShapeKind::Triangles { .. }));
        let mut with = scene.clone();
        with.bodies.push(body);
        assert_eq!(with.validate(), Ok(()));
        let again = super::body(&with, "crate.stl", &bytes, None, &Options::default()).unwrap();
        assert_eq!(again.label, "crate 2");
        assert!((again.shape.unwrap().kind.bounds().unwrap().size().x - 1.0).abs() < 1e-12, "scale 1");
    }

    #[test]
    fn what_is_not_a_mesh_is_refused_in_words() {
        let scene = crate::dev::void();
        let why = body(&scene, "notes.txt", b"hello", None, &Options::default()).unwrap_err();
        assert!(why.contains(".obj or an .stl"), "{why}");
        let why = body(&scene, "a.stl", b"solid nothing\nendsolid\n", None, &Options::default()).unwrap_err();
        assert!(!why.is_empty());
        let bytes = stl_box([0.5; 3]);
        let unknown = Options { material: "unobtainium".into(), ..Options::default() };
        assert!(body(&scene, "a.stl", &bytes, None, &unknown).unwrap_err().starts_with("material"));
        let huge = Options { size: Size::Fit(5e3), ..Options::default() };
        assert!(body(&scene, "a.stl", &bytes, None, &huge).unwrap_err().starts_with("fit"));
    }
}
