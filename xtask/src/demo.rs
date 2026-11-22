//! `cargo xtask demo <slice>`: the slice's demo, run with no window, with its screenshots and
//! transcripts saved to `docs/slices/<slice>/`.
//!
//! Every step checks what it produced (a PNG is a PNG of the right size, a transcript says what it
//! should); a demo that silently wrote nothing would be worse than no demo.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::{cargo, step};

pub fn run(root: &Path, slice: &str) -> Result<(), String> {
    match slice {
        "S0" | "s0" => s0(root),
        "S1" | "s1" => s1(root),
        other => Err(format!("no demo for slice `{other}`: the slices with demos are: S0, S1")),
    }
}

/// The built binary called `name`.
fn binary(root: &Path, name: &str) -> PathBuf {
    root.join("target").join("debug").join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

/// Runs a program, returning its standard output; a failure carries its standard error.
fn capture(program: &Path, args: &[&str]) -> Result<String, String> {
    let output =
        Command::new(program).args(args).output().map_err(|e| format!("{}: {e}", program.display()))?;
    if !output.status.success() {
        return Err(format!(
            "`{} {}` failed ({}): {}",
            program.display(),
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"))
}

/// The width and height of a PNG file, from its header.
fn png_size(path: &Path) -> Result<[u32; 2], String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    png_size_of(&bytes).ok_or_else(|| format!("{} is not a PNG", path.display()))
}

/// The width and height in a PNG's header, or `None` if the bytes are not a PNG.
pub fn png_size_of(bytes: &[u8]) -> Option<[u32; 2]> {
    let header = bytes.get(..24)?;
    if &header[..8] != b"\x89PNG\r\n\x1a\n" || &header[12..16] != b"IHDR" {
        return None;
    }
    let field = |at: usize| u32::from_be_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]]);
    Some([field(16), field(20)])
}

/// Decodes standard base64 (with or without padding); `None` on any other character.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// The value of the first `"key":"…"` in a line of JSON, without unescaping. Enough for base64
/// and for names; xtask has no JSON parser because it has no dependencies.
pub fn json_string<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let start = json.find(&format!("\"{key}\":\""))? + key.len() + 4;
    let end = start + json[start..].find('"')?;
    Some(&json[start..end])
}

/// Text cut to `limit` characters, saying how much was left out.
fn abridged(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let head: String = text.chars().take(limit).collect();
    format!("{head}… [{} more characters]", text.chars().count() - limit)
}

fn s0(root: &Path) -> Result<(), String> {
    let out = root.join("docs").join("slices").join("S0");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    step("build cav and cav-app", cargo(root, &["build", "--quiet", "-p", "cav", "-p", "cav-app"]))?;
    let cav = binary(root, "cav");
    let app = binary(root, "cav-app");
    let path = |name: &str| out.join(name).to_string_lossy().into_owned();
    let mut made: Vec<String> = Vec::new();

    // 1. The doctor.
    let doctor = capture(&cav, &["doctor"])?;
    if !doctor.contains("render self-test: ok") {
        return Err(format!("cav doctor did not pass its self-test:\n{doctor}"));
    }
    write(&out, "doctor.txt", &doctor, &mut made)?;

    // 2. Scenes drawn by the CLI: views, projections and styles.
    let renders: [(&str, &str, &[&str]); 7] = [
        ("render-room-iso.png", "room", &[]),
        ("render-room-plan-paper.png", "room", &["--view", "top", "--proj", "ortho", "--style", "paper"]),
        (
            "render-room-south-wireframe.png",
            "room",
            &["--view", "south", "--proj", "ortho", "--style", "wireframe"],
        ),
        ("render-room-xray.png", "room", &["--style", "xray", "--exec", "orbit -70 20; select tower 1"]),
        ("render-void-iso.png", "void", &["--style", "flat"]),
        ("render-infinite-floor.png", "infinite-floor", &["--exec", "orbit -62 9; zoom 30"]),
        ("render-infinite-floor-plan.png", "infinite-floor", &["--view", "top", "--proj", "ortho"]),
    ];
    for (file, scene, options) in renders {
        let target = path(file);
        let mut args = vec!["render", scene, "--size", "1280x720", "--out", &target];
        args.extend_from_slice(options);
        capture(&cav, &args)?;
        expect_png(&out, file, [1280, 720], &mut made)?;
    }

    // 3. The whole application, with no window.
    let shots: [(&str, &str, &str); 8] = [
        ("app-default.png", "1440x900", "select tower 2"),
        ("app-four-panes.png", "1440x900", "panes 4; select tower 3; seek 4.6"),
        ("app-paper-plan.png", "1440x900", "style paper; view top; proj ortho; select room"),
        ("app-console.png", "1440x900", "view north; select tower 9; get array.instrument"),
        ("app-infinite-floor.png", "1440x900", "load infinite-floor; orbit -62 9; zoom 30; select floor"),
        ("app-void-two-panes.png", "1440x900", "load void; panes 2; style xray @2"),
        ("app-f3.png", "1440x900", "gui"),
        ("app-small-window.png", "1024x700", "panes 2; keys"),
    ];
    for (file, size, script) in shots {
        // One script has a command the engine refuses, on purpose: it shows the console opening
        // on an error. --keep-going lets the script carry on past it.
        let target = path(file);
        capture(&app, &["--screenshot", &target, "--size", size, "--exec", script, "--keep-going"])?;
        let (w, h) = size.split_once('x').expect("sizes are WxH");
        expect_png(&out, file, [w.parse().expect("a width"), h.parse().expect("a height")], &mut made)?;
    }

    // 4. An agent's session with the MCP server.
    let (transcript, picture) = mcp_session(&cav)?;
    write(&out, "mcp-session.txt", &transcript, &mut made)?;
    std::fs::write(out.join("mcp-render-view.png"), &picture)
        .map_err(|e| format!("mcp-render-view.png: {e}"))?;
    expect_png(&out, "mcp-render-view.png", [800, 500], &mut made)?;

    // 5. Frame times.
    let scratch = root.join("target").join("demo-timing.png");
    let timings =
        capture(&app, &["--screenshot", &scratch.to_string_lossy(), "--size", "1440x900", "--timings"])?;
    let timings: String = timings.lines().skip(1).map(|line| format!("{line}\n")).collect();
    if !timings.contains("median") {
        return Err(format!("no frame times were measured:\n{timings}"));
    }
    write(&out, "timings.txt", &timings, &mut made)?;

    eprintln!("xtask: demo S0 wrote {} files to {}:", made.len(), out.display());
    for file in &made {
        eprintln!("  {file}");
    }
    Ok(())
}

/// S1, a point mass: a drag, a pass, the signal, the periodogram with its comb and its folds,
/// what the console answers, and the timings. No window is opened.
fn s1(root: &Path) -> Result<(), String> {
    let out = root.join("docs").join("slices").join("S1");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    step("build cav and cav-app", cargo(root, &["build", "--quiet", "-p", "cav", "-p", "cav-app"]))?;
    let cav = binary(root, "cav");
    let app = binary(root, "cav-app");
    let path = |name: &str| out.join(name).to_string_lossy().into_owned();
    let mut made: Vec<String> = Vec::new();

    // 1 to 4. The application. A drag is shown as three of the places it passes through, each made
    // by the `set` a drag issues (the drag itself, by pointer, is `dragging_a_body_moves_it_under_the_pointer`).
    let drag = "load point; select body 1; set bodies[0].motion.Static.position";
    let pass = "load pass; panes 2; show signal @2; select tower 2; seek";
    let shots: [(&str, String); 11] = [
        ("drag-1.png", format!("{drag} [2, 0, 2.5]")),
        ("drag-2.png", format!("{drag} [2.9, 0.9, 2.5]")),
        ("drag-3.png", format!("{drag} [3.6, 1.6, 2.5]")),
        ("pass-1.png", format!("{pass} 1.46")),
        ("pass-2.png", format!("{pass} 5.46")),
        ("pass-3.png", format!("{pass} 9.46")),
        ("signal.png", "load oscillator-zoo; panes 2; show signal @2; seek 600".to_owned()),
        // The same plot zoomed to its first two minutes: the shots themselves.
        (
            "signal-zoom.png",
            "load oscillator-zoo; panes 2; show signal @2; span 0 120 @2; seek 60".to_owned(),
        ),
        ("periodogram-comb.png", "load comb; panes 2; show periodogram @2".to_owned()),
        ("periodogram-folded.png", "load folded-comb; panes 2; show periodogram @2".to_owned()),
        (
            "periodogram-zoo.png",
            "load oscillator-zoo; panes 2; show periodogram @2; select tower 1".to_owned(),
        ),
    ];
    for (file, script) in &shots {
        let target = path(file);
        capture(&app, &["--screenshot", &target, "--size", "1440x900", "--exec", script])?;
        expect_png(&out, file, [1440, 900], &mut made)?;
    }

    // 4b. A plot pane as an agent sees it: `cav render` (and MCP `render_view`) draw the pane's plot.
    let target = path("render-periodogram.png");
    capture(
        &cav,
        &["render", "folded-comb", "--size", "900x520", "--exec", "show periodogram", "--out", &target],
    )?;
    expect_png(&out, "render-periodogram.png", [900, 520], &mut made)?;

    // 5. What the console answers.
    let script = "load point; reading 3; set bodies[0].motion.Static.position [3, 0, 2.5]; reading 3; \
                  load pass; signal 2; load comb; periodogram 1; load folded-comb; periodogram 1";
    let session = capture(&cav, &["exec", script])?;
    for expected in ["tower 5", "· refined · 6 shots", "2f₀ at 0.100000 Hz", "f₀ at 0.172500 Hz (folded)"]
    {
        if !session.contains(expected) {
            return Err(format!("the session does not say `{expected}`:\n{session}"));
        }
    }
    write(&out, "session.txt", &format!("$ cav exec \"{script}\"\n{session}"), &mut made)?;

    // 6. Timings: frames, the reading, the series, a drag step; and a series of 10,000 shots.
    let scratch = root.join("target").join("demo-timing.png").to_string_lossy().into_owned();
    let mut timings = String::new();
    for (what, script) in [
        ("point (one body at rest, 6 shots)", "load point; panes 2; show signal @2"),
        ("oscillator-zoo (four bodies, 1000 shots)", "load oscillator-zoo; panes 2; show periodogram @2"),
        (
            "comb stretched to 10,000 shots (one body)",
            "load comb; set duration 20000; panes 2; show signal @2",
        ),
    ] {
        let lines =
            capture(&app, &["--screenshot", &scratch, "--size", "1440x900", "--timings", "--exec", script])?;
        if !lines.contains("drag step") || lines.contains("NOT FINISHED") {
            return Err(format!("the timings of {what} are incomplete:\n{lines}"));
        }
        timings.push_str(&format!("## {what}\n"));
        timings.extend(lines.lines().skip(1).map(|line| format!("{line}\n")));
        timings.push('\n');
    }
    write(&out, "timings.txt", &timings, &mut made)?;

    eprintln!("xtask: demo S1 wrote {} files to {}:", made.len(), out.display());
    for file in &made {
        eprintln!("  {file}");
    }
    Ok(())
}

fn write(dir: &Path, name: &str, text: &str, made: &mut Vec<String>) -> Result<(), String> {
    std::fs::write(dir.join(name), text).map_err(|e| format!("{name}: {e}"))?;
    made.push(name.to_owned());
    Ok(())
}

fn expect_png(dir: &Path, name: &str, size: [u32; 2], made: &mut Vec<String>) -> Result<(), String> {
    let found = png_size(&dir.join(name))?;
    if found != size {
        return Err(format!("{name} is {} × {}, not {} × {}", found[0], found[1], size[0], size[1]));
    }
    made.push(name.to_owned());
    Ok(())
}

/// Speaks MCP to `cav mcp` as an agent would: the handshake, the tool list, the schema, a scene, a
/// refusal, and a picture. Returns the transcript and the PNG that `render_view` sent.
fn mcp_session(cav: &Path) -> Result<(String, Vec<u8>), String> {
    let mut child = Command::new(cav)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cav mcp: {e}"))?;
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout is piped"));
    let mut transcript =
        String::from("An MCP session with `cav mcp` over stdio, as `cargo xtask demo S0` ran it.\n\n");
    let mut id = 0u32;
    let mut picture = None;
    let mut exchange = |method: &str, params: &str, note: &str| -> Result<String, String> {
        id += 1;
        let request = format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#);
        writeln!(stdin, "{request}").and_then(|()| stdin.flush()).map_err(|e| format!("cav mcp: {e}"))?;
        let marker = format!("\"id\":{id}");
        loop {
            let mut line = String::new();
            let read = stdout.read_line(&mut line).map_err(|e| format!("cav mcp: {e}"))?;
            if read == 0 {
                return Err(format!("cav mcp closed its output during `{method}`"));
            }
            if line.contains(&marker) {
                transcript.push_str(&format!("# {note}\n→ {request}\n← {}\n\n", abridged(line.trim(), 700)));
                if method == "initialize" {
                    // The client confirms the handshake; a notification has no answer.
                    let confirmed = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
                    writeln!(stdin, "{confirmed}")
                        .and_then(|()| stdin.flush())
                        .map_err(|e| format!("cav mcp: {e}"))?;
                    transcript.push_str(&format!("→ {confirmed}\n\n"));
                }
                return Ok(line);
            }
        }
    };

    let hello = exchange(
        "initialize",
        r#"{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"xtask-demo","version":"0"}}"#,
        "the handshake",
    )?;
    if !hello.contains("cavendish") {
        return Err(format!("the server did not introduce itself: {hello}"));
    }
    let tools = exchange("tools/list", "{}", "what the server offers")?;
    for tool in ["scene_schema", "scene_load", "scene_get", "command", "log", "render_view"] {
        if !tools.contains(&format!("\"name\":\"{tool}\"")) {
            return Err(format!("the tool list lacks `{tool}`: {tools}"));
        }
    }
    let schema =
        exchange("tools/call", r#"{"name":"scene_schema","arguments":{}}"#, "the scene spec's JSON Schema")?;
    if !schema.contains("SceneSpec") || !schema.contains("Length of the timeline, seconds.") {
        return Err(format!(
            "scene_schema did not answer with the spec's schema: {}",
            abridged(&schema, 300)
        ));
    }
    exchange(
        "tools/call",
        r#"{"name":"scene_load","arguments":{"scene":"infinite-floor"}}"#,
        "load a dev scene",
    )?;
    exchange("tools/call", r#"{"name":"scene_get","arguments":{"path":"floor"}}"#, "read part of it")?;
    exchange(
        "tools/call",
        r#"{"name":"command","arguments":{"line":"orbit -62 9; zoom 30; select tower 4"}}"#,
        "move the camera and select a tower, with console commands",
    )?;
    let refused = exchange(
        "tools/call",
        r#"{"name":"command","arguments":{"line":"select tower 9"}}"#,
        "a command the engine refuses comes back as an error that says why",
    )?;
    if !refused.contains("there is no tower 9") {
        return Err(format!("the refusal did not say why: {refused}"));
    }
    let rendered = exchange(
        "tools/call",
        r#"{"name":"render_view","arguments":{"width":800,"height":500}}"#,
        "look at it: the picture comes back as a base64 PNG (saved as mcp-render-view.png)",
    )?;
    if let Some(data) = json_string(&rendered, "data") {
        picture = base64_decode(data);
    }
    exchange("tools/call", r#"{"name":"log","arguments":{"last":6}}"#, "what the engine did, and refused")?;

    drop(exchange);
    drop(stdin);
    let status = child.wait().map_err(|e| format!("cav mcp: {e}"))?;
    if !status.success() {
        return Err(format!("cav mcp exited with {status}"));
    }
    let picture = picture.ok_or("render_view sent no picture")?;
    Ok((transcript, picture))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_decodes_and_refuses_what_is_not_base64() {
        assert_eq!(base64_decode("aGVsbG8=").as_deref(), Some(b"hello".as_slice()));
        assert_eq!(base64_decode("aGVsbG8").as_deref(), Some(b"hello".as_slice()), "padding is optional");
        assert_eq!(base64_decode("iVBORw0KGgo=").as_deref(), Some(b"\x89PNG\r\n\x1a\n".as_slice()));
        assert_eq!(base64_decode("").as_deref(), Some(b"".as_slice()));
        assert_eq!(base64_decode("aGVs bG8="), None, "a space is not base64");
        assert_eq!(base64_decode("aGVsbG8*"), None);
    }

    #[test]
    fn png_headers_give_the_size_and_other_bytes_do_not() {
        let mut png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR".to_vec();
        png.extend_from_slice(&1280u32.to_be_bytes());
        png.extend_from_slice(&720u32.to_be_bytes());
        assert_eq!(png_size_of(&png), Some([1280, 720]));
        // Control: a truncated file, another format and an empty file are not PNGs.
        assert_eq!(png_size_of(&png[..20]), None);
        assert_eq!(png_size_of(b"GIF89a, not a PNG, with padding to 24"), None);
        assert_eq!(png_size_of(b""), None);
    }

    #[test]
    fn json_strings_are_found_by_key() {
        let line = r#"{"result":{"content":[{"type":"image","data":"QUJD","mimeType":"image/png"}]}}"#;
        assert_eq!(json_string(line, "data"), Some("QUJD"));
        assert_eq!(json_string(line, "mimeType"), Some("image/png"));
        assert_eq!(json_string(line, "text"), None);
        assert_eq!(abridged("abcdef", 3), "abc… [3 more characters]");
        assert_eq!(abridged("abc", 3), "abc");
    }
}
