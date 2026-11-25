//! The meshes this session was given, by the hash of their bytes (S19's review, I4): what a
//! scene's large imported mesh ([`crate::ShapeKind::Mesh`] with a `hash`) is made from.
//!
//! A mesh of more than [`crate::shape::MAX_INLINE_TRIANGLES`] triangles is too large to hold in
//! the scene itself, so an import keeps its bytes here and the scene names it by file and hash. A
//! page cannot read a file by its path, so this is the only place a dropped file's mesh lives in
//! one; natively the file is read by its path when the store does not have it, and its bytes must
//! still have the hash the scene names (a file that changed is refused, not read as something
//! else). Natively the forward stage's threads share this store; in a page they run on the one
//! thread. A saved scene names the file and the hash; a later session holds the mesh again when
//! the file is read or dropped again.
//!
//! What is kept is the mesh parsed and centred on the middle of its box, in the file's units:
//! a body scales it ([`Mesh::scaled`]), one pass over its corners. This is the one piece of state
//! in this crate: the bytes of a file are input, not a cache of anything this crate computes.
//!
//! Caps: one file at most [`cav_core::voxel::mesh::MAX_MESH_BYTES`] (32 MiB) and
//! [`cav_core::voxel::mesh::MAX_TRIANGLES`] triangles; all held together at most
//! [`MAX_HELD_BYTES`]. Past either, the file is refused with the numbers.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use cav_core::voxel::mesh::{MAX_MESH_BYTES, Mesh};

/// The most bytes of mesh files one session holds together.
pub const MAX_HELD_BYTES: usize = 256 << 20;

/// One held mesh.
struct Held {
    /// The file's name, as it was given.
    name: String,
    /// The file's bytes: what is sent to a page's workers ([`bytes`]), counted against
    /// [`MAX_HELD_BYTES`].
    bytes: Arc<[u8]>,
    /// The mesh in the file's units, centred on the middle of its box.
    mesh: Arc<Mesh>,
}

fn store() -> &'static Mutex<HashMap<String, Held>> {
    static STORE: OnceLock<Mutex<HashMap<String, Held>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The hash a scene names a mesh file's bytes by: 64-bit FNV-1a of the bytes, as 16 lower-case
/// hexadecimal digits. The same on every platform and build. Not cryptographic: it tells a
/// person's files apart, it does not guard against a made collision.
#[must_use]
pub fn hash(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Whether `text` is a hash as [`hash`] writes one.
#[must_use]
pub fn is_hash(text: &str) -> bool {
    text.len() == 16 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Parses `bytes` (the file `name`, `.obj` or `.stl`), centres it on the middle of its box, keeps
/// it, and gives its hash and the mesh. A mesh already held is not parsed again.
///
/// # Errors
/// When the file is larger than 32 MiB, is not a closed mesh of its format, or would take the
/// meshes held past [`MAX_HELD_BYTES`], in words with the numbers.
pub fn hold(name: &str, bytes: &[u8]) -> Result<(String, Arc<Mesh>), String> {
    let key = hash(bytes);
    if let Some(mesh) = held(&key) {
        return Ok((key, mesh));
    }
    if bytes.len() as u64 > MAX_MESH_BYTES {
        return Err(format!(
            "{name}: {} MiB is more than the {} MiB a mesh file may be; simplify the mesh",
            bytes.len() >> 20,
            MAX_MESH_BYTES >> 20
        ));
    }
    let mesh = Arc::new(Mesh::parse(name, bytes, 1.0).map_err(|e| e.0)?.centred_on_box());
    let mut store = store().lock().unwrap_or_else(PoisonError::into_inner);
    let total: usize = store.values().map(|h| h.bytes.len()).sum();
    if total + bytes.len() > MAX_HELD_BYTES {
        return Err(format!(
            "{name}: the meshes held this session take {} MiB, and this one would take them past the {} MiB \
             a session holds",
            total >> 20,
            MAX_HELD_BYTES >> 20
        ));
    }
    store.insert(
        key.clone(),
        Held { name: name.to_owned(), bytes: Arc::from(bytes), mesh: Arc::clone(&mesh) },
    );
    Ok((key, mesh))
}

/// The mesh held under `hash`, if it is held.
#[must_use]
pub fn held(hash: &str) -> Option<Arc<Mesh>> {
    store().lock().unwrap_or_else(PoisonError::into_inner).get(hash).map(|h| Arc::clone(&h.mesh))
}

/// The name and bytes of the file held under `hash`, if it is held: what a page sends a worker
/// that does not have it yet.
#[must_use]
pub fn bytes(hash: &str) -> Option<(String, Arc<[u8]>)> {
    store()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(hash)
        .map(|h| (h.name.clone(), Arc::clone(&h.bytes)))
}

/// The hashes of the held meshes `scene`'s bodies name, each once, in the bodies' order.
#[must_use]
pub fn named(scene: &crate::SceneSpec) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for body in &scene.bodies {
        if let Some(crate::ShapeKind::Mesh { hash: Some(h), .. }) = body.shape.as_ref().map(|s| &s.kind)
            && !out.contains(h)
        {
            out.push(h.clone());
        }
    }
    out
}

/// The mesh of the file `file` in the file's units, centred on the middle of its box: the one
/// held under `hash` when there is one; else the file read by its path (natively), whose bytes
/// must have `hash` when one is named, and which is then held.
///
/// # Errors
/// When the file cannot be read (always, in a page, for a mesh not held), its bytes do not have
/// the hash the scene names, or it is not a mesh (as [`hold`]).
pub fn mesh(file: &str, hash: Option<&str>) -> Result<Arc<Mesh>, String> {
    if let Some(mesh) = hash.and_then(held) {
        return Ok(mesh);
    }
    let bytes = read(file).map_err(|why| match hash {
        Some(h) => format!(
            "{file}: the mesh (hash {h}) is not held in this session and the file cannot be read ({why}); \
             drop or import the file again"
        ),
        None => format!("{file}: {why}"),
    })?;
    let found = self::hash(&bytes);
    if let Some(want) = hash
        && found != want
    {
        return Err(format!(
            "{file}: its bytes have the hash {found}, not the scene's {want}: the file has changed since it was \
             imported; import it again"
        ));
    }
    if hash.is_none() {
        // A file named without a hash is read each time it is asked for, as before S19's review.
        return Mesh::parse(file, &bytes, 1.0).map(|m| Arc::new(m.centred_on_box())).map_err(|e| e.0);
    }
    hold(file, &bytes).map(|(_, mesh)| mesh)
}

/// The file's bytes, at most 32 MiB and one byte (so that a larger file is refused by its size).
fn read(file: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let handle = std::fs::File::open(file).map_err(|e| e.to_string())?;
    let length = handle.metadata().map_or(0, |m| m.len());
    let mut bytes = Vec::with_capacity((length.min(MAX_MESH_BYTES) + 1) as usize);
    handle.take(MAX_MESH_BYTES + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cav_core::Vec3;
    use cav_core::voxel::mesh::{box_mesh, to_stl};

    #[test]
    fn a_mesh_is_held_by_the_hash_of_its_bytes() {
        // The hash is FNV-1a's (its published value for "a"); a held mesh is found by it and is
        // centred on its box; a file whose bytes have another hash is refused, not read.
        // Control: an unknown hash with no file is refused in words.
        assert_eq!(hash(b"a"), "af63dc4c8601ec8c");
        assert!(is_hash(&hash(b"")) && !is_hash("AF63DC4C8601EC8C") && !is_hash("af63"));
        let (v, t) = box_mesh(Vec3::new(1.0, 2.0, 3.0), Vec3::new(2.0, 2.5, 3.25));
        let bytes = to_stl(&v, &t);
        let (key, mesh) = hold("held.stl", &bytes).unwrap();
        assert_eq!(key, hash(&bytes));
        let bounds = cav_core::voxel::Shape::bounds(mesh.as_ref());
        assert_eq!(bounds.centre(), Vec3::ZERO, "centred on its box");
        assert!(Arc::ptr_eq(&held(&key).unwrap(), &mesh));
        assert!(Arc::ptr_eq(&super::mesh("anywhere.stl", Some(&key)).unwrap(), &mesh), "no file read");
        let dir = std::env::temp_dir().join(format!("cav-assets-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let other = dir.join("changed.stl");
        let (v2, t2) = box_mesh(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        std::fs::write(&other, to_stl(&v2, &t2)).unwrap();
        let wrong = "0123456789abcdef";
        let why = super::mesh(other.to_str().unwrap(), Some(wrong)).unwrap_err();
        assert!(why.contains("has changed"), "{why}");
        let why = super::mesh("no/such/file.stl", Some(wrong)).unwrap_err();
        assert!(why.contains("not held in this session"), "{why}");
        // A file read by its path (S19's second review, minor 1): a lopsided mesh (a torus with
        // its +x and +z halves stretched, so the middle of its box is not its centroid) with no hash is
        // centred on its box and is not held; named with its own hash it is read, held and the
        // same mesh. Control: centred on its centroid, the box's middle is centimetres off.
        let (v, t) = cav_core::voxel::mesh::torus_mesh(1.0, 0.3, 24, 12);
        let stretch = |x: f64, by: f64| if x > 0.0 { by * x } else { x };
        let v: Vec<Vec3> =
            v.into_iter().map(|p| Vec3::new(stretch(p.x, 2.0), p.y, stretch(p.z, 3.0))).collect();
        let lopsided = to_stl(&v, &t);
        let file = dir.join("lopsided.stl");
        std::fs::write(&file, &lopsided).unwrap();
        let file = file.to_str().unwrap();
        let middle = |m: &Mesh| cav_core::voxel::Shape::bounds(m).centre().length();
        let read = super::mesh(file, None).unwrap();
        assert!(middle(&read) < 1e-6, "a file's mesh is centred on its box: {} m off", middle(&read));
        let centroid = Mesh::parse(file, &lopsided, 1.0).unwrap().centred();
        assert!(middle(&centroid) > 1e-2, "the control: its centroid is not its box's middle");
        let own = hash(&lopsided);
        assert!(held(&own).is_none(), "a file named without a hash is not held");
        let kept = super::mesh(file, Some(&own)).unwrap();
        assert!(Arc::ptr_eq(&held(&own).unwrap(), &kept), "read with its hash, it is held");
        assert_eq!(kept.vertices, read.vertices, "and it is the same mesh");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
