//! Importing a mesh file as a body (S19, `docs/slices/S19.md` §2): an OBJ or STL file's bytes,
//! scaled, made of a material, voxelised at a pitch, placed.
//!
//! The mesh is centred on the middle of its box, whatever its size, and **held in the scene** as
//! a [`ShapeKind::Triangles`] when it has at most [`MAX_INLINE_TRIANGLES`] triangles, so the scene
//! needs no file beside it (in a page, where nothing is read by path, as natively). A larger mesh,
//! up to 32 MiB and 500,000 triangles, is kept by the session ([`crate::assets`]) and named by
//! its path (natively) or its name (a dropped file) and the hash of its bytes
//! ([`ShapeKind::Mesh`]), in a page as natively; past those caps it is refused with the numbers.
//! What [`body`] makes is an ordinary body, added by an ordinary `Insert`: one undo takes it out,
//! and a journal replays it in the session without the file (a held mesh: in a later session
//! once the file is read or dropped again).
//!
//! Frame and units: the file's units times the scale are metres; the body's position is the
//! middle of the mesh's box, the world's frame.

use cav_core::voxel::mesh::{MAX_MESH_BYTES, Mesh};

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
    if bytes.len() as u64 > MAX_MESH_BYTES {
        return Err(format!(
            "{name}: {} MiB is more than the {} MiB a mesh file may be; simplify the mesh",
            bytes.len() >> 20,
            MAX_MESH_BYTES >> 20
        ));
    }
    // Centred on its box, not its centroid, whatever its size and wherever it is kept: the box's
    // middle of a mesh symmetric in its numbers is exactly the origin, where a centroid summed
    // from its triangles is a rounding off it, and a grid one cell larger for it (S19: an imported
    // box was a sliver taller than the cuboid).
    let mesh = Mesh::parse(name, bytes, 1.0).map_err(|e| e.0)?.centred_on_box();
    let size = cav_core::voxel::Shape::bounds(&mesh).size();
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
    } else {
        // Too large to hold in the scene: kept by the session under the hash of its bytes, and
        // named by its path (natively) or its name (a dropped file) and that hash. The same mesh,
        // centred the same way, scaled the same way when it is voxelised.
        let (hash, _) = crate::assets::hold(name, bytes)?;
        ShapeKind::Mesh { file: path.unwrap_or(name).to_owned(), scale, hash: Some(hash) }
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

    /// A torus of `around × across × 2` triangles with its +x half stretched twice as long and its top half three times as tall: closed,
    /// not crossing itself, and lopsided, so that the middle of its box is not its centroid.
    fn lopsided(around: u32, across: u32) -> Vec<u8> {
        let (v, t) = cav_core::voxel::mesh::torus_mesh(1.0, 0.3, around, across);
        let v: Vec<Vec3> = v
            .into_iter()
            .map(|p| {
                Vec3::new(
                    if p.x > 0.0 { 2.0 * p.x } else { p.x },
                    p.y,
                    if p.z > 0.0 { 3.0 * p.z } else { p.z },
                )
            })
            .collect();
        to_stl(&v, &t)
    }

    #[test]
    fn one_centring_rule_whatever_the_size_or_where_it_is_kept() {
        // S19's review, I4: a mesh of more than 10,000 triangles, imported with a path (natively)
        // or without (a page), is kept by the session and named by its hash, and is placed by the
        // middle of its box as a held one is. Control: centred on its centroid (as a path's mesh
        // was) the box's middle is off `at` by centimetres. And a small mesh made through the
        // session's store gives the tree its held form gives, bit for bit.
        let scene = crate::dev::void();
        let bytes = lopsided(100, 51);
        let options = Options { at: [1.0, 2.0, 3.0], pitch: Some(0.1), ..Options::default() };
        for path in [Some("meshes/ring.stl"), None] {
            let body = body(&scene, "ring.stl", &bytes, path, &options).unwrap();
            let shape = body.shape.as_ref().unwrap();
            let ShapeKind::Mesh { file, hash, .. } = &shape.kind else { panic!("{:?}", shape.kind.name()) };
            assert_eq!(
                (file.as_str(), hash.as_deref()),
                (path.unwrap_or("ring.stl"), Some(crate::assets::hash(&bytes).as_str()))
            );
            let middle = shape.kind.bounds().expect("a held mesh's box is known").centre();
            assert!(middle.length() < 1e-12, "the box's middle is the body's position: {middle:?}");
            let solid = shape.kind.solid().unwrap();
            assert!(solid.bounds().centre().length() < 1e-12);
            let mut with = scene.clone();
            with.bodies.push(body);
            assert_eq!(with.validate(), Ok(()));
        }
        let centroid = Mesh::parse("ring.stl", &bytes, 1.0).unwrap().centred();
        let off = cav_core::voxel::Shape::bounds(&centroid).centre();
        assert!(off.length() > 1e-2, "the control: a centroid is not the box's middle ({off:?})");
        // A small lopsided mesh: held in the scene, and the same bytes through the store.
        let small = lopsided(24, 12);
        let held = body(&scene, "small.stl", &small, None, &options).unwrap().shape.unwrap();
        assert!(matches!(held.kind, ShapeKind::Triangles { .. }));
        let (hash, _) = crate::assets::hold("small.stl", &small).unwrap();
        let kept = ShapeSpec {
            kind: ShapeKind::Mesh { file: "small.stl".into(), scale: 1.0, hash: Some(hash) },
            ..held.clone()
        };
        let (a, b) = (held.tree(1.0, &[]).unwrap(), kept.tree(1.0, &[]).unwrap());
        assert!(a.leaves == b.leaves && a.grid == b.grid, "the same leaves on the same grid");
    }

    #[test]
    fn a_mesh_past_the_caps_is_refused_with_the_numbers() {
        let scene = crate::dev::void();
        let big = vec![0u8; (MAX_MESH_BYTES + 1) as usize];
        let why = body(&scene, "huge.stl", &big, None, &Options::default()).unwrap_err();
        assert!(why.contains("more than the 32 MiB a mesh file may be"), "{why}");
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
