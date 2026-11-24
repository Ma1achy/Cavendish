//! Files on a native process: written where their paths say, or, inside [`capture`], kept as
//! downloads exactly as a page keeps them (the seam's test double).

use std::cell::RefCell;
use std::io::{Read, Write};
use std::path::Path;

use super::{Download, name_of};

/// Whether a folder of files (a bundle) can be written: natively, yes.
pub const FOLDERS: bool = true;

thread_local! {
    /// Inside [`capture`]: the files "written" on this thread, as a page would keep them.
    static CAPTURED: RefCell<Option<Vec<Download>>> = const { RefCell::new(None) };
}

/// Whether this thread is inside [`capture`].
fn capturing() -> bool {
    CAPTURED.with_borrow(Option::is_some)
}

/// The error a read gives inside [`capture`]: the one a page gives.
fn unsupported() -> std::io::Error {
    std::io::Error::from(std::io::ErrorKind::Unsupported)
}

/// Runs `work` with every file it writes through this module, on this thread, kept as a
/// download instead of written, and reads refused, as in a browser page; answers with what
/// `work` returned and the downloads, oldest first. For tests that must say a writer goes
/// through the seam: a file written around it lands on disc and is not in the list. Not for
/// shipped code.
pub fn capture<R>(work: impl FnOnce() -> R) -> (R, Vec<Download>) {
    let outer = CAPTURED.replace(Some(Vec::new()));
    let result = work();
    let captured = CAPTURED.replace(outer).unwrap_or_default();
    (result, captured)
}

/// Writes `bytes` to `path`, replacing what is there.
///
/// # Errors
/// What the file system refused.
pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let held = CAPTURED.with_borrow_mut(|captured| {
        captured.as_mut().map(|list| list.push(Download { name: name_of(path), bytes: bytes.to_vec() }))
    });
    if held.is_some() { Ok(()) } else { std::fs::write(path, bytes) }
}

/// The bytes of the file at `path`, if it has at most `most` of them.
///
/// # Errors
/// What the file system refused; a file longer than `most` (`InvalidData`).
pub fn read(path: &Path, most: u64) -> std::io::Result<Vec<u8>> {
    if capturing() {
        return Err(unsupported());
    }
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > most {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("longer than {most} bytes"),
        ));
    }
    let mut bytes = Vec::new();
    file.take(most).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Whether `path` is a file that can be read.
#[must_use]
pub fn is_file(path: &Path) -> bool {
    !capturing() && path.is_file()
}

/// Makes `path` and every folder above it that is missing.
///
/// # Errors
/// What the file system refused.
pub fn create_dir_all(path: &Path) -> std::io::Result<()> {
    if capturing() { Ok(()) } else { std::fs::create_dir_all(path) }
}

/// A file being written a piece at a time, through a buffer.
#[derive(Debug)]
pub struct File(Inner);

#[derive(Debug)]
enum Inner {
    Disc(std::io::BufWriter<std::fs::File>),
    /// Inside [`capture`]: its bytes, kept as a download when it is finished.
    Held {
        path: std::path::PathBuf,
        bytes: Vec<u8>,
    },
}

/// Makes the file at `path`, empty, to be written through a buffer of `buffer` bytes.
///
/// # Errors
/// What the file system refused.
pub fn create(path: &Path, buffer: usize) -> std::io::Result<File> {
    if capturing() {
        return Ok(File(Inner::Held { path: path.to_path_buf(), bytes: Vec::new() }));
    }
    Ok(File(Inner::Disc(std::io::BufWriter::with_capacity(buffer, std::fs::File::create(path)?))))
}

impl File {
    /// Writes out what is buffered.
    ///
    /// # Errors
    /// What the file system refused.
    pub fn finish(self) -> std::io::Result<()> {
        match self.0 {
            Inner::Disc(mut writer) => writer.flush(),
            Inner::Held { path, bytes } => write(&path, &bytes),
        }
    }
}

impl Write for File {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        match &mut self.0 {
            Inner::Disc(writer) => writer.write(bytes),
            Inner::Held { bytes: held, .. } => {
                held.extend_from_slice(bytes);
                Ok(bytes.len())
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.0 {
            Inner::Disc(writer) => writer.flush(),
            Inner::Held { .. } => Ok(()),
        }
    }
}

/// Nothing: natively files are written where they are named.
#[must_use]
pub const fn take_downloads() -> Vec<Download> {
    Vec::new()
}
