//! Where written files go: the platform seam for files.
//!
//! Natively a file is written where its path says. In a browser page there is no file system: a
//! file "written" is kept in memory under its name, as a download, until the application takes it
//! ([`take_downloads`]) and hands it to the browser. Everything that writes a file the person
//! asked for (an export, a figure, its data and its record) goes through here, so the web build
//! needs nothing else to give the same files.
//!
//! What a page cannot do at all is refused where it is asked for, with the reason: writing a
//! folder of files (a bundle, [`FOLDERS`]) and reading a file by its path (a scene, a preset or a
//! journal by path: std's own error, "operation not supported on this platform").
//!
//! Must not be used for files the engine reads back as part of its state, or for caches: in the
//! browser they would vanish into the downloads.

use std::path::Path;

#[cfg(not(target_arch = "wasm32"))]
#[path = "files/native.rs"]
mod platform;
#[cfg(target_arch = "wasm32")]
#[path = "files/web.rs"]
mod platform;

pub use platform::{FOLDERS, File, create, create_dir_all, take_downloads, write};

/// The most bytes of downloads a page holds before the application takes them: a write that
/// would pass it is refused. Natively nothing is held.
pub const MAX_DOWNLOAD_BYTES: usize = 256 << 20;

/// A file made in the browser, waiting to be handed to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Download {
    /// The file's name: the last part of the path it was written to.
    pub name: String,
    /// Its bytes.
    pub bytes: Vec<u8>,
}

/// The name a path's file is given as a download: its last part, or `cavendish` if it has none.
#[must_use]
pub fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(|| "cavendish".to_owned(), |name| name.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_download_is_named_by_the_paths_last_part() {
        assert_eq!(name_of(Path::new("out/figs/fig.svg")), "fig.svg");
        assert_eq!(name_of(Path::new("scene.ron")), "scene.ron");
        // Control: a path with no file in it still gives a name.
        assert_eq!(name_of(Path::new("")), "cavendish");
    }
}
