//! What a page's address may ask for, and what may leave a page as a file: the rules, on every
//! platform, so that they are tested natively (`cargo xtask quick`); the page itself is
//! [`crate::platform`] on `wasm32`.
//!
//! The page is public, so a link is a stranger's input: `…/Cavendish/#exec=load comb; panes 2`
//! runs at most [`MAX_EXEC_COMMANDS`] console commands from at most [`MAX_FRAGMENT`] bytes of
//! address, and a file such a link makes is not downloaded until the person clicks for it (the
//! page holds it behind a button). Every download, a link's or the person's own, is named by a
//! plain file name with one of [`DOWNLOAD_EXTENSIONS`]; any other is refused.

/// The longest `#exec=` fragment a page runs, in bytes as the address carries them.
pub const MAX_FRAGMENT: usize = 4096;

/// The most console commands a page runs from its address.
pub const MAX_EXEC_COMMANDS: usize = 32;

/// The longest name a download is given, in bytes.
pub const MAX_DOWNLOAD_NAME: usize = 120;

/// The kinds of file a page hands the browser: a scene (`.ron`), a journal or a figure's record
/// (`.json`), a figure (`.svg`, `.pdf`) and its data (`.csv`, `.npy`).
pub const DOWNLOAD_EXTENSIONS: [&str; 6] = ["ron", "json", "svg", "pdf", "csv", "npy"];

/// The console commands of an address's fragment (`#exec=…`, still percent-encoded), in order.
/// `Ok(None)` when the fragment asks for none.
///
/// # Errors
/// A fragment longer than [`MAX_FRAGMENT`], not valid percent-encoded UTF-8, or of more than
/// [`MAX_EXEC_COMMANDS`] commands: none of it is run.
pub fn exec_commands(fragment: &str) -> Result<Option<Vec<String>>, String> {
    let Some(script) = fragment.strip_prefix("#exec=") else { return Ok(None) };
    if fragment.len() > MAX_FRAGMENT {
        return Err(format!(
            "the address's #exec is {} bytes, more than the {MAX_FRAGMENT} a page runs; none of it was run",
            fragment.len()
        ));
    }
    let script = percent_decode(script)?;
    let commands: Vec<String> =
        cav_engine::console::statements(&script).into_iter().map(str::to_owned).collect();
    if commands.len() > MAX_EXEC_COMMANDS {
        return Err(format!(
            "the address's #exec has {} commands, more than the {MAX_EXEC_COMMANDS} a page runs; none was run",
            commands.len()
        ));
    }
    Ok(Some(commands))
}

/// `%XX` decoded, as `decodeURIComponent` does.
fn percent_decode(text: &str) -> Result<String, String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = bytes.get(at + 1..at + 3).and_then(|h| std::str::from_utf8(h).ok());
            let byte = hex.and_then(|h| u8::from_str_radix(h, 16).ok());
            out.push(byte.ok_or_else(|| format!("`{}` is not percent-encoded", &text[at..]))?);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "the address's #exec is not UTF-8".to_owned())
}

/// The name a file is downloaded under, from the name the engine gave it (already its path's
/// last part): letters, digits, `-`, `_` and `.` kept, anything else made `_`, no leading dot,
/// at most [`MAX_DOWNLOAD_NAME`] bytes, and an extension of [`DOWNLOAD_EXTENSIONS`]. `None`
/// for a name with another extension, or none: such a file is not handed to the browser.
#[must_use]
pub fn download_name(name: &str) -> Option<String> {
    let name = name.rsplit(['/', '\\']).next()?;
    let (stem, extension) = name.rsplit_once('.')?;
    let extension = extension.to_ascii_lowercase();
    if !DOWNLOAD_EXTENSIONS.contains(&extension.as_str()) {
        return None;
    }
    let stem: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' })
        .collect();
    let stem = stem.trim_start_matches('.');
    let stem = if stem.is_empty() { "cavendish" } else { stem };
    let room = MAX_DOWNLOAD_NAME - extension.len() - 1;
    Some(format!("{}.{extension}", &stem[..stem.len().min(room)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_runs_a_bounded_script() {
        assert_eq!(exec_commands("").unwrap(), None);
        assert_eq!(exec_commands("#top").unwrap(), None);
        let commands = exec_commands("#exec=load%20comb;%20panes 2").unwrap().unwrap();
        assert_eq!(commands, ["load comb", "panes 2"]);
        // At the caps: allowed; one past: refused whole.
        let most = vec!["panes 1"; MAX_EXEC_COMMANDS].join(";");
        assert_eq!(exec_commands(&format!("#exec={most}")).unwrap().unwrap().len(), MAX_EXEC_COMMANDS);
        assert!(exec_commands(&format!("#exec={most};panes 2")).is_err());
        let long = format!("#exec=load comb;{}", " ".repeat(MAX_FRAGMENT));
        assert!(exec_commands(&long).is_err());
        assert!(exec_commands("#exec=load%2").is_err(), "a broken escape");
        assert!(exec_commands("#exec=%ff").is_err(), "not UTF-8");
    }

    #[test]
    fn a_download_has_a_plain_name_and_a_known_kind() {
        assert_eq!(download_name("comb.svg").as_deref(), Some("comb.svg"));
        assert_eq!(download_name("Fig 1 (final).PDF").as_deref(), Some("Fig_1__final_.pdf"));
        assert_eq!(download_name("../../evil.json").as_deref(), Some("evil.json"));
        assert_eq!(download_name(r"..\..\x.csv").as_deref(), Some("x.csv"));
        assert_eq!(download_name(".hidden.ron").as_deref(), Some("hidden.ron"));
        assert_eq!(
            download_name(&format!("{}.npy", "a".repeat(500))).map(|n| n.len()),
            Some(MAX_DOWNLOAD_NAME)
        );
        // Control arms: the kinds a page must not hand out, and a name with no kind.
        for refused in ["evil.html", "../../evil.html", "run.exe", "x.svg.js", "noextension", "x."] {
            assert_eq!(download_name(refused), None, "{refused}");
        }
    }
}
