//! Files in a browser page: kept in memory as downloads, named by their paths' last parts, until
//! the application takes them and hands them to the browser. At most
//! [`super::MAX_DOWNLOAD_BYTES`] wait at once.

use std::cell::RefCell;
use std::io::Write;
use std::path::Path;

use super::{Download, MAX_DOWNLOAD_BYTES, name_of};

/// Whether a folder of files (a bundle) can be written: a browser gives files one at a time, so
/// no.
pub const FOLDERS: bool = false;

thread_local! {
    /// The downloads not yet taken. A page has one thread.
    static WAITING: RefCell<Vec<Download>> = const { RefCell::new(Vec::new()) };
}

/// Keeps `bytes` as a download named by `path`'s last part.
///
/// # Errors
/// When the downloads waiting would pass [`MAX_DOWNLOAD_BYTES`].
pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    WAITING.with_borrow_mut(|waiting| {
        let held: usize = waiting.iter().map(|download| download.bytes.len()).sum();
        if held.saturating_add(bytes.len()) > MAX_DOWNLOAD_BYTES {
            return Err(std::io::Error::other(format!(
                "{} bytes are waiting to be downloaded and this file is {}: more than the {} MiB a page holds",
                held,
                bytes.len(),
                MAX_DOWNLOAD_BYTES >> 20
            )));
        }
        waiting.push(Download { name: name_of(path), bytes: bytes.to_vec() });
        Ok(())
    })
}

/// Nothing: a download has no folder.
///
/// # Errors
/// Never.
pub const fn create_dir_all(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// A file being written a piece at a time: its bytes, kept as a download when it is finished.
#[derive(Debug)]
pub struct File {
    path: std::path::PathBuf,
    bytes: Vec<u8>,
}

/// Starts a download to be named by `path`'s last part.
///
/// # Errors
/// Never.
pub fn create(path: &Path, _buffer: usize) -> std::io::Result<File> {
    Ok(File { path: path.to_path_buf(), bytes: Vec::new() })
}

impl File {
    /// Keeps what was written as a download.
    ///
    /// # Errors
    /// As [`write`].
    pub fn finish(self) -> std::io::Result<()> {
        write(&self.path, &self.bytes)
    }
}

impl Write for File {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > MAX_DOWNLOAD_BYTES {
            return Err(std::io::Error::other(format!(
                "a file of more than {} MiB cannot be downloaded from a page",
                MAX_DOWNLOAD_BYTES >> 20
            )));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The downloads waiting, oldest first; none are kept after this.
#[must_use]
pub fn take_downloads() -> Vec<Download> {
    WAITING.with_borrow_mut(std::mem::take)
}
