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
    let text = std::fs::read_to_string(name_or_path).map_err(|e| {
        SpecError(format!(
            "`{name_or_path}` is not a library scene ({}) and not a readable file: {e}",
            known.join(", ")
        ))
    })?;
    SceneSpec::from_ron(&text).map_err(|e| SpecError(format!("{name_or_path}: {e}")))
}

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
}
