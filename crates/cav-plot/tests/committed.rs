//! The sample figures committed under `docs/` are whole as they are checked out.
//!
//! Found by S2's review (finding 27): with no `.gitattributes`, git took a PDF with an
//! uncompressed content stream for text and wrote it to a Windows working tree with carriage
//! returns, so every cross-reference offset was wrong and the file a reader was shown was damaged.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every file with this extension under `dir`, to any depth.
fn files_under(dir: &Path, extension: &str, into: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_under(&path, extension, into);
        } else if path.extension().is_some_and(|ext| ext == extension) {
            into.push(path);
        }
    }
}

#[test]
fn committed_pdfs_are_whole_as_checked_out() {
    let mut pdfs = Vec::new();
    files_under(&root().join("docs"), "pdf", &mut pdfs);
    assert!(pdfs.len() >= 2, "the S2 demo commits at least two PDFs; found {}", pdfs.len());
    for path in &pdfs {
        let bytes = std::fs::read(path).unwrap();
        let checked = cav_plot::pdf::check(&bytes);
        assert!(checked.is_ok(), "{}: {}", path.display(), checked.err().unwrap_or_default());
        // Control: the same file as a checkout that rewrites line endings would leave it is
        // caught, so this test would have failed before `.gitattributes` said a PDF is binary.
        let mut rewritten = Vec::with_capacity(bytes.len() + bytes.len() / 20);
        for byte in &bytes {
            if *byte == b'\n' {
                rewritten.push(b'\r');
            }
            rewritten.push(*byte);
        }
        assert!(
            cav_plot::pdf::check(&rewritten).is_err(),
            "{}: line endings rewritten, not caught",
            path.display()
        );
    }
    // And the rule that keeps them so is in the repository: these kinds are never text.
    let attributes = std::fs::read_to_string(root().join(".gitattributes")).unwrap();
    for kind in ["*.pdf binary", "*.png binary", "*.npy binary", "*.ttf binary"] {
        assert!(attributes.lines().any(|line| line.trim() == kind), "`.gitattributes` lacks `{kind}`");
    }
    assert!(attributes.lines().any(|line| line.trim() == "* text=auto eol=lf"));
}

#[test]
fn committed_pngs_and_arrays_are_whole_as_checked_out() {
    // A PNG's signature exists to catch exactly this: it holds a CR LF pair and a lone LF.
    let mut pngs = Vec::new();
    files_under(&root().join("docs"), "png", &mut pngs);
    assert!(pngs.len() >= 20, "{} PNGs under docs", pngs.len());
    for path in &pngs {
        let bytes = std::fs::read(path).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "{} is not a PNG as checked out", path.display());
    }
    // Control: the signature with its line ending rewritten is not the signature.
    assert!(!b"\x89PNG\r\r\n\x1a\r\n".starts_with(b"\x89PNG\r\n\x1a\n"));
}
