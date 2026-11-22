//! The built-in scene library: the `.ron` files under `scenes/`, compiled in so that every binary
//! has them wherever it runs.

use crate::{SceneSpec, SpecError};

/// One scene of the built-in library.
#[derive(Clone, Copy, Debug)]
pub struct LibraryScene {
    /// The name it is loaded by: `load room`.
    pub name: &'static str,
    /// Its path in the repository, relative to the root.
    pub path: &'static str,
    /// The file's text.
    pub ron: &'static str,
}

macro_rules! scene {
    ($name:literal, $path:literal) => {
        LibraryScene { name: $name, path: $path, ron: include_str!(concat!("../../../", $path)) }
    };
}

/// Every built-in scene, in the order the library shows them.
pub const LIBRARY: &[LibraryScene] = &[
    scene!("room", "scenes/library/room.ron"),
    scene!("void", "scenes/dev/void.ron"),
    scene!("infinite-floor", "scenes/dev/infinite-floor.ron"),
    scene!("point", "scenes/dev/point.ron"),
    scene!("pass", "scenes/dev/pass.ron"),
    scene!("comb", "scenes/dev/comb.ron"),
    scene!("folded-comb", "scenes/dev/folded-comb.ron"),
    scene!("oscillator-zoo", "scenes/dev/oscillator-zoo.ron"),
    scene!("superposition", "scenes/dev/superposition.ron"),
    scene!("mirror-pair", "scenes/dev/mirror-pair.ron"),
    scene!("switch-on", "scenes/dev/switch-on.ron"),
    scene!("flash", "scenes/dev/flash.ron"),
    scene!("time-reversal", "scenes/dev/time-reversal.ron"),
    scene!("ring-orbit", "scenes/dev/ring-orbit.ron"),
    scene!("through-the-tower", "scenes/dev/through-the-tower.ron"),
    scene!("precision-far", "scenes/dev/precision-far.ron"),
    scene!("staggered", "scenes/dev/staggered.ron"),
];

/// Loads a scene by library name, or else from a `.ron` file at that path.
///
/// # Errors
/// When the name is neither a library scene nor a readable file, or the spec is refused; the message
/// lists the library's names.
pub fn load(name_or_path: &str) -> Result<SceneSpec, SpecError> {
    if let Some(scene) = LIBRARY.iter().find(|s| s.name == name_or_path) {
        return SceneSpec::from_ron(scene.ron).map_err(|e| SpecError(format!("{}: {e}", scene.path)));
    }
    let known: Vec<&str> = LIBRARY.iter().map(|s| s.name).collect();
    let unreadable = |e: std::io::Error| {
        SpecError(format!(
            "`{name_or_path}` is not a library scene ({}) and not a readable file: {e}",
            known.join(", ")
        ))
    };
    // At most one byte past the cap is ever read, whatever the file's size claims to be: a scene
    // file is text a person can read, and a path given by an agent may point at anything.
    let file = std::fs::File::open(name_or_path).map_err(unreadable)?;
    // Room for the cap and the one byte that says the file is over it, reserved once: a buffer
    // that grew by doubling would hold three times the cap for a moment.
    let mut bytes = Vec::with_capacity(MAX_SCENE_FILE_BYTES as usize + 1);
    std::io::Read::read_to_end(&mut std::io::Read::take(file, MAX_SCENE_FILE_BYTES + 1), &mut bytes)
        .map_err(unreadable)?;
    if bytes.len() as u64 > MAX_SCENE_FILE_BYTES {
        return Err(SpecError(format!(
            "{name_or_path}: the file is larger than {MAX_SCENE_FILE_BYTES} bytes (1 MiB), the most a scene \
             file may be; it was not read"
        )));
    }
    let text = String::from_utf8(bytes)
        .map_err(|e| unreadable(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;
    SceneSpec::from_ron(&text).map_err(|e| SpecError(format!("{name_or_path}: {e}")))
}

/// A scene given whole, as JSON: what an agent sends in place of a name or a path. Held to the
/// size of a scene file, counted on the text before it is parsed.
///
/// # Errors
/// When the text is over [`MAX_SCENE_FILE_BYTES`], is not a scene, or is a scene that is refused.
pub fn inline(json: &str) -> Result<SceneSpec, SpecError> {
    if json.len() as u64 > MAX_SCENE_FILE_BYTES {
        return Err(SpecError(format!(
            "an inline scene of {} bytes is larger than {MAX_SCENE_FILE_BYTES} bytes (1 MiB), the most a scene \
             may be",
            json.len()
        )));
    }
    SceneSpec::from_json(json)
}

/// The largest scene file [`load`] reads, bytes: 1 MiB. The largest valid spec (a thousand towers)
/// is about a tenth of that.
pub const MAX_SCENE_FILE_BYTES: u64 = 1 << 20;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// Every `.ron` file under a directory, as paths relative to the repository root.
    fn ron_files(dir: &Path, root: &Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                ron_files(&path, root, out);
            } else if path.extension().is_some_and(|e| e == "ron") {
                let relative = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                out.push(relative);
            }
        }
    }

    #[test]
    fn scene_files_on_disk_are_the_built_in_library() {
        let root = repo_root().canonicalize().unwrap();
        let mut on_disk = Vec::new();
        ron_files(&root.join("scenes"), &root, &mut on_disk);
        on_disk.sort();
        let mut built_in: Vec<String> = LIBRARY.iter().map(|s| s.path.to_owned()).collect();
        built_in.sort();
        // A file added to scenes/ but not to LIBRARY (or the reverse) fails here.
        assert_eq!(on_disk, built_in);
        for scene in LIBRARY {
            let spec = SceneSpec::from_ron(scene.ron).unwrap_or_else(|e| panic!("{}: {e}", scene.path));
            assert_eq!(spec.name, scene.name, "{}: the file's name is its library name", scene.path);
            let stem = Path::new(scene.path).file_stem().unwrap().to_string_lossy().into_owned();
            assert_eq!(stem, scene.name, "{}: the file is named after the scene", scene.path);
        }
    }

    #[test]
    fn the_room_file_is_the_default_scene() {
        assert_eq!(load("room").unwrap(), SceneSpec::room());
        // Control: the comparison sees a change of one field.
        let moved = LIBRARY[0].ron.replacen("duration: 12.0", "duration: 12.5", 1);
        assert_ne!(moved, LIBRARY[0].ron, "the control edits the file's text");
        assert_ne!(SceneSpec::from_ron(&moved).unwrap(), SceneSpec::room());
    }

    #[test]
    fn library_names_resolve_and_unknown_names_do_not() {
        for scene in LIBRARY {
            assert_eq!(load(scene.name).unwrap().name, scene.name);
        }
        // A path to a scene file works as well as a name.
        let path = repo_root().join(LIBRARY[0].path);
        assert_eq!(load(path.to_str().unwrap()).unwrap(), load(LIBRARY[0].name).unwrap());
        let why = load("no-such-scene").unwrap_err().0;
        assert!(why.contains("no-such-scene") && why.contains("room"), "{why}");
    }

    #[test]
    fn a_scene_file_over_the_cap_is_refused_unread() {
        let dir = repo_root().join("target/tmp/scene size");
        std::fs::create_dir_all(&dir).unwrap();
        // A valid scene padded with a comment: at the cap it loads, one byte over it does not.
        let scene = crate::dev::void().to_ron();
        let padded = |total: u64| {
            let filler = "x".repeat(usize::try_from(total).unwrap() - scene.len() - 4);
            format!("// {filler}\n{scene}")
        };
        let at = dir.join("at-the-cap.ron");
        std::fs::write(&at, padded(MAX_SCENE_FILE_BYTES)).unwrap();
        assert_eq!(std::fs::metadata(&at).unwrap().len(), MAX_SCENE_FILE_BYTES);
        assert_eq!(
            load(at.to_str().unwrap()).unwrap(),
            crate::dev::void(),
            "the control: the cap itself loads"
        );
        let over = dir.join("over-the-cap.ron");
        std::fs::write(&over, padded(MAX_SCENE_FILE_BYTES + 1)).unwrap();
        let why = load(over.to_str().unwrap()).unwrap_err().0;
        assert!(why.contains("larger than 1048576 bytes") && why.contains("not read"), "{why}");
    }
}
