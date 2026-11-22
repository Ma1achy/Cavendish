//! A journal as a file: the commands of a session, in order, as JSON.
//!
//! ```text
//! { "cavendish_journal": 1, "engine": "0.0.0", "commands": [ {"Load": …}, {"Set": …}, … ] }
//! ```
//!
//! Every number in a command is finite (the engine refuses the others before they are
//! journalled) and JSON keeps every finite `f64` exactly, so a journal read back is the journal
//! that was written, to the bit. Replaying it gives the same state and the same exported bytes
//! **with the same binary on the same platform**: the forward model's `sin` and `cos` are the
//! platform's (`docs/research/s2-replay-spec-paths-cadence.md` §2.4).

use serde::{Deserialize, Serialize};

use crate::command::Command;

/// The version of the journal's file format this build reads and writes.
pub const JOURNAL_VERSION: u32 = 1;

/// The largest journal file [`read`] reads, bytes: the size at which a journal stops recording.
pub const MAX_JOURNAL_FILE_BYTES: u64 = crate::history::JOURNAL_BYTES as u64;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    cavendish_journal: u32,
    engine: String,
    commands: Vec<Command>,
}

/// The journal as the text of a file.
#[must_use]
pub fn write(commands: &[Command]) -> String {
    let file = File {
        cavendish_journal: JOURNAL_VERSION,
        engine: crate::VERSION.into(),
        commands: commands.to_vec(),
    };
    serde_json::to_string(&file).expect("commands are JSON")
}

/// The commands in the text of a journal file. A bare list of commands (what the console's
/// `journal` prints) is read as well.
///
/// # Errors
/// When the text is neither, or is a journal of another version.
pub fn parse(text: &str) -> Result<Vec<Command>, String> {
    if text.trim_start().starts_with('[') {
        return serde_json::from_str(text).map_err(|e| format!("not a list of commands: {e}"));
    }
    let file: File = serde_json::from_str(text).map_err(|e| format!("not a journal: {e}"))?;
    if file.cavendish_journal != JOURNAL_VERSION {
        return Err(format!(
            "a journal of version {}; this build reads version {JOURNAL_VERSION}",
            file.cavendish_journal
        ));
    }
    Ok(file.commands)
}

/// Reads a journal file. At most [`MAX_JOURNAL_FILE_BYTES`] and one byte are ever read, whatever
/// the file's size: a path given by an agent may point at anything.
///
/// # Errors
/// When the file cannot be read, is too large, or is not a journal.
pub fn read(path: &str) -> Result<Vec<Command>, String> {
    let failed = |e: std::io::Error| format!("{path}: {e}");
    let file = std::fs::File::open(path).map_err(failed)?;
    let length = file.metadata().map_err(failed)?.len();
    if length > MAX_JOURNAL_FILE_BYTES {
        return Err(format!(
            "{path}: the file is larger than {MAX_JOURNAL_FILE_BYTES} bytes, the most a journal may be; it was \
             not read"
        ));
    }
    // The size the file says it has, and one byte, reserved once; no more is read even if the
    // file has grown since.
    let mut bytes = Vec::with_capacity(length as usize + 1);
    std::io::Read::read_to_end(&mut std::io::Read::take(file, length + 1), &mut bytes).map_err(failed)?;
    let text = String::from_utf8(bytes).map_err(|e| format!("{path}: not text: {e}"))?;
    parse(&text).map_err(|why| format!("{path}: {why}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_journal_file_is_the_journal() {
        let commands = vec![
            Command::load(cav_sim::dev::comb()),
            Command::set("bodies[0].mass", serde_json::json!(0.1 + 0.2)),
            Command::Seek(1.0 / 3.0),
            Command::Undo,
        ];
        let text = write(&commands);
        assert_eq!(parse(&text).unwrap(), commands);
        assert!(text.starts_with("{\"cavendish_journal\":1,"), "{}", &text[..40]);
        // A bare list, as `journal` prints it, is a journal too.
        assert_eq!(parse(&serde_json::to_string(&commands).unwrap()).unwrap(), commands);
        // Through a file.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/journals");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a session.json");
        std::fs::write(&path, &text).unwrap();
        assert_eq!(read(path.to_str().unwrap()).unwrap(), commands);
        // Control: what is not a journal is refused with the reason, and so is another version.
        assert!(parse("{\"commands\": []}").unwrap_err().contains("not a journal"));
        assert!(
            parse(&text.replacen("\"cavendish_journal\":1", "\"cavendish_journal\":2", 1))
                .unwrap_err()
                .contains("version 2")
        );
        assert!(parse("[{\"Explode\": 1}]").unwrap_err().contains("not a list of commands"));
        assert!(read(dir.join("missing.json").to_str().unwrap()).is_err());
    }
}
