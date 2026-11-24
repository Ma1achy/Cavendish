//! Files on a native process: written where their paths say.

use std::io::Write;
use std::path::Path;

use super::Download;

/// Whether a folder of files (a bundle) can be written: natively, yes.
pub const FOLDERS: bool = true;

/// Writes `bytes` to `path`, replacing what is there.
///
/// # Errors
/// What the file system refused.
pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)
}

/// Makes `path` and every folder above it that is missing.
///
/// # Errors
/// What the file system refused.
pub fn create_dir_all(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)
}

/// A file being written a piece at a time, through a buffer of `buffer` bytes.
#[derive(Debug)]
pub struct File(std::io::BufWriter<std::fs::File>);

/// Makes the file at `path`, empty, to be written through a buffer of `buffer` bytes.
///
/// # Errors
/// What the file system refused.
pub fn create(path: &Path, buffer: usize) -> std::io::Result<File> {
    Ok(File(std::io::BufWriter::with_capacity(buffer, std::fs::File::create(path)?)))
}

impl File {
    /// Writes out what is buffered.
    ///
    /// # Errors
    /// What the file system refused.
    pub fn finish(mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

impl Write for File {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.write(bytes)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

/// Nothing: natively files are written where they are named.
#[must_use]
pub const fn take_downloads() -> Vec<Download> {
    Vec::new()
}
