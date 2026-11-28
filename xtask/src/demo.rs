//! `cargo xtask demo <slice>`: the slice's demo, run with no window, with its screenshots and
//! transcripts saved to `docs/slices/<slice>/`.
//!
//! Every step checks what it produced (a PNG is a PNG of the right size, a transcript says what it
//! should); a demo that silently wrote nothing would be worse than no demo.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::{cargo, step};

/// Runs `slice`'s demo; `web`: with its page in headless Chrome, where it has one (S4: without,
/// the page's screenshots already there are kept, and no local server is started).
pub fn run(root: &Path, slice: &str, web: bool) -> Result<(), String> {
    match slice {
        "S0" | "s0" => s0(root),
        "S1" | "s1" => s1(root),
        "S2" | "s2" => s2(root),
        "S3" | "s3" => s3(root),
        "S4" | "s4" => s4(root, web),
        "S5" | "s5" => s5(root),
        "S17" | "s17" => s17(root),
        "S18" | "s18" => s18(root),
        "S19" | "s19" => s19(root),
        "S20" | "s20" => s20(root),
        other => Err(format!(
            "no demo for slice `{other}`: the slices with demos are: S0, S1, S2, S3, S4, S5, S17, S18, S19, S20"
        )),
    }
}

/// The built binary called `name`.
fn binary(root: &Path, name: &str) -> PathBuf {
    root.join("target").join("debug").join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

/// Runs a program, returning its standard output; a failure carries its standard error.
fn capture(program: &Path, args: &[&str]) -> Result<String, String> {
    let output =
        crate::gentle(program).args(args).output().map_err(|e| format!("{}: {e}", program.display()))?;
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

/// The directory's regular files, by name, with their bytes.
fn files_of(dir: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten() {
        if entry.path().is_file() {
            let bytes =
                std::fs::read(entry.path()).map_err(|e| format!("{}: {e}", entry.path().display()))?;
            out.push((entry.file_name().to_string_lossy().into_owned(), bytes));
        }
    }
    out.sort();
    Ok(out)
}

fn s2(root: &Path) -> Result<(), String> {
    let out = root.join("docs").join("slices").join("S2");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    step("build cav and cav-app", cargo(root, &["build", "--quiet", "-p", "cav", "-p", "cav-app"]))?;
    let cav = binary(root, "cav");
    let app = binary(root, "cav-app");
    let path = |name: &str| out.join(name).to_string_lossy().into_owned();
    let mut made: Vec<String> = Vec::new();
    // Everything that is not kept is written here, never where a journal or a scene says.
    let scratch = root.join("target").join("demo-s2");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
    let temp = |name: &str| scratch.join(name).to_string_lossy().replace('\\', "/");

    // 1 to 4. The application: editors made from the schema, a lane per tower, the flights on
    // absolute time, two instruments on one scene, and the cadence designer.
    let lab = "load cadence-lab; set array.schedule.jitter null; panes 2; show cadence @2;";
    let shots: [(&str, String); 8] = [
        ("editors.png", "load comb; select body 1".to_owned()),
        ("editors-tower.png", "load staggered; select tower 2".to_owned()),
        ("lanes.png", "load staggered; set duration 30; panes 2; show signal @2; seek 12".to_owned()),
        ("flight.png", "load staggered; set duration 30; panes 2; show flight @2; seek 12".to_owned()),
        ("preset-compare.png", "load comb; compare aice; panes 2; show periodogram @2".to_owned()),
        (
            "cadence-jitter.png",
            format!(r#"{lab} set array.schedule.jitter {{"model":"Walk","kind":"Symmetric","delta":0.2}}"#),
        ),
        ("cadence-alternating.png", format!("{lab} set array.schedule.alternate 3.0")),
        (
            "cadence-gaps-loss.png",
            format!(r#"{lab} set array.schedule.gaps {{"on":4,"period":10}}; set array.schedule.loss 0.2"#),
        ),
    ];
    for (file, script) in &shots {
        let target = path(file);
        capture(&app, &["--screenshot", &target, "--size", "1440x900", "--exec", script])?;
        expect_png(&out, file, [1440, 900], &mut made)?;
    }

    // A screenshot is the same picture every time it is taken: nothing a run's speed decides is
    // drawn (the footer once said how many milliseconds the signal took, and every demo run
    // changed every picture). One of each kind of pane is taken again and compared.
    for (file, script) in [&shots[0], &shots[2], &shots[5]] {
        let again = temp("again.png");
        capture(&app, &["--screenshot", &again, "--size", "1440x900", "--exec", script])?;
        let (first, second) = (std::fs::read(out.join(file)), std::fs::read(&again));
        if first.is_err() || first.ok() != second.ok() {
            return Err(format!("{file} is not the same picture when it is taken again"));
        }
    }

    // 5. Scenes as files, from the command line; then what the console answers.
    let mine = temp("mine.ron");
    let mut session = String::new();
    let said = |args: &[&str], expected: &[&str]| -> Result<String, String> {
        let answer = capture(&cav, args)?;
        for text in expected {
            if !answer.contains(text) {
                return Err(format!("`cav {}` does not say `{text}`:\n{answer}", args.join(" ")));
            }
        }
        let shown: Vec<String> = args
            .iter()
            .map(|arg| if arg.contains(' ') { format!("\"{arg}\"") } else { (*arg).to_owned() })
            .collect();
        Ok(format!(
            "$ cav {}
{}
",
            shown.join(" "),
            abridged(answer.trim_end(), 2400)
        ))
    };
    session.push_str(&said(
        &["ls", "staggered", "array.towers[1]"],
        &["timing", "position", "x east, y north, z up"],
    )?);
    session.push_str(&said(
        &["set", "staggered", "array.towers[1].timing.offset", "0.5", "--out", &mine],
        &["saved"],
    )?);
    session.push_str(&said(&["get", &mine, "array.towers[1].timing"], &["\"offset\": 0.5"])?);
    match said(&["set", &mine, "bodies[0].mass", "-1"], &[]) {
        Ok(_) => return Err("a negative mass was accepted from the command line".to_owned()),
        Err(refusal) if refusal.contains("mass") => {
            // What `cav` itself said, without this machine's path to the binary.
            let said = refusal.split_once("): ").map_or(refusal.as_str(), |(_, said)| said);
            session.push_str(&format!("$ cav set {mine} bodies[0].mass -1\n{}\n", said.trim_end()));
        }
        Err(other) => return Err(format!("the refusal does not name the field: {other}")),
    }
    session.push_str(&said(&["schema", "array.schedule.loss"], &["probability"])?);
    let saved = temp("saved.ron");
    let script = format!(
        "load cadence-lab; cadence; window 1 to 0.5; periodogram 1; set array.schedule.loss 0.3; undo; \
         redo; periodogram 1; preset aice; launch symmetric; undo; undo; save {saved}; load {saved}; stats"
    );
    session.push_str(&said(&["exec", &script], &["mean cycle", "Lomb–Scargle", "saved"])?);
    session = session.replace(&scratch.to_string_lossy().replace('\\', "/"), "target/demo-s2");
    write(&out, "session.txt", &session, &mut made)?;

    // 5b. An agent: an inline spec, paths, a set, a refusal that says where.
    let mut mcp =
        Mcp::start(&cav, "An MCP session with `cav mcp` over stdio, as `cargo xtask demo S2` ran it.")?;
    mcp.hello()?;
    let tools = mcp.exchange("tools/list", "{}", "what the server offers")?;
    for tool in ["scene_set", "scene_ls", "scene_schema", "scene_load", "scene_get"] {
        if !tools.contains(&format!("\"name\":\"{tool}\"")) {
            return Err(format!("the tool list lacks `{tool}`"));
        }
    }
    let spec = capture(&cav, &["get", "staggered"])?;
    let spec: String = spec.split_whitespace().collect::<Vec<_>>().join(" ");
    mcp.call("scene_load", &format!(r#"{{"spec":{spec}}}"#), "a scene given inline, as JSON, not by name")?;
    let listed = mcp.call("scene_ls", r#"{"path":"array.towers[0].timing"}"#, "what is inside a path")?;
    if !listed.contains("offset") {
        return Err(format!("scene_ls did not list the timing: {}", abridged(&listed, 300)));
    }
    mcp.call(
        "scene_set",
        r#"{"path":"array.towers[0].timing.offset","value":0.25}"#,
        "set a field by its path",
    )?;
    let read = mcp.call("scene_get", r#"{"path":"array.towers[0].timing.offset"}"#, "and read it back")?;
    if !read.contains("0.25") {
        return Err(format!("scene_get did not answer with the value set: {}", abridged(&read, 300)));
    }
    let refused = mcp.call(
        "scene_set",
        r#"{"path":"array.towers[0].timing.offset","value":1e9}"#,
        "a value out of range is refused, saying the field and the range",
    )?;
    if !refused.contains("offset") || !refused.contains("isError\":true") {
        return Err(format!("the refusal did not say where: {}", abridged(&refused, 400)));
    }
    mcp.call(
        "command",
        r#"{"line":"undo; signal 1 0 6"}"#,
        "console lines work as before: undo, and the signal",
    )?;
    write(&out, "mcp-session.txt", &mcp.finish()?, &mut made)?;

    // 6. A bundle from a scene file, and a session replayed: the same bytes.
    let bundle = temp("bundle");
    let ran = capture(&cav, &["run", "staggered", "--out", &bundle])?;
    let mut listing = format!(
        "$ cav run staggered --out target/demo-s2/bundle\n{}\n",
        ran.replace(&bundle, "target/demo-s2/bundle")
    );
    let files = files_of(Path::new(&bundle))?;
    for (name, bytes) in &files {
        listing.push_str(&format!("  {name:<24} {:>9} bytes\n", bytes.len()));
    }
    let manifest =
        files.iter().find(|(name, _)| name == "manifest.json").ok_or("the bundle has no manifest")?;
    for expected in ["\"numerics\"", "\"error_estimate", "\"scene\""] {
        if !String::from_utf8_lossy(&manifest.1).contains(expected) {
            return Err(format!("the manifest does not have {expected}"));
        }
    }
    std::fs::create_dir_all(out.join("bundle")).map_err(|e| format!("bundle: {e}"))?;
    std::fs::write(out.join("bundle").join("manifest.json"), &manifest.1)
        .map_err(|e| format!("manifest: {e}"))?;
    made.push("bundle/manifest.json".to_owned());
    std::fs::write(out.join("bundle").join("listing.txt"), &listing).map_err(|e| format!("listing: {e}"))?;
    made.push("bundle/listing.txt".to_owned());

    let (first, journal, again) = (temp("first"), temp("session.json"), temp("again"));
    let script = format!(
        "load staggered; set duration 600; set bodies[0].mass 2; preset aice; undo; \
         set array.schedule.loss 0.1; export bundle {first}; journal save {journal}"
    );
    capture(&cav, &["exec", &script])?;
    let replayed = capture(&cav, &["replay", &journal, "--out", &again])?;
    let (one, two) = (files_of(Path::new(&first))?, files_of(&Path::new(&again).join("first"))?);
    let differing = one.iter().zip(&two).filter(|(a, b)| a != b).count() + one.len().abs_diff(two.len());
    if one.len() < 10 || differing != 0 {
        return Err(format!(
            "the replay wrote {} file(s) against {}, {differing} differing",
            two.len(),
            one.len()
        ));
    }
    let bytes: usize = one.iter().map(|(_, bytes)| bytes.len()).sum();
    let replay = format!(
        "$ cav exec \"{}\"\n$ cav replay target/demo-s2/session.json --out target/demo-s2/again\n{}\n\
         {} files, {bytes} bytes: {differing} differ between the session's bundle and the replay's.\n",
        script.replace(&scratch.to_string_lossy().replace('\\', "/"), "target/demo-s2"),
        replayed.trim_end().replace(&scratch.to_string_lossy().replace('\\', "/"), "target/demo-s2"),
        one.len()
    );
    write(&out, "replay.txt", &replay, &mut made)?;

    // 7. Figures: each in a preset's style and size, with its data and its provenance beside it;
    // and one redrawn from that data alone by the Python helper.
    let figures = out.join("figures");
    let _ = std::fs::remove_dir_all(&figures);
    std::fs::create_dir_all(&figures).map_err(|e| format!("{}: {e}", figures.display()))?;
    let mut said = String::new();
    for (scene, what, preset, file) in [
        ("comb", "periodogram", "paper-column", "periodogram-paper-column.svg"),
        ("comb", "periodogram", "paper-column", "periodogram-paper-column.pdf"),
        ("comb", "periodogram", "paper-column", "periodogram-paper-column.png"),
        ("pass", "signal", "slide", "signal-slide.svg"),
        ("cadence-lab", "cadence", "paper-full", "cadence-paper-full.pdf"),
        ("staggered", "flight", "paper-full", "flight-paper-full.svg"),
    ] {
        // Run from the figures' directory, so that nothing a figure records names this machine.
        let output = crate::gentle(&cav)
            .args(["figure", scene, what, "--preset", preset, "--out", file])
            .current_dir(&figures)
            .output()
            .map_err(|e| format!("cav figure: {e}"))?;
        if !output.status.success() {
            return Err(format!("cav figure {scene} {what}: {}", String::from_utf8_lossy(&output.stderr)));
        }
        said.push_str(&format!(
            "$ cav figure {scene} {what} --preset {preset} --out {file}\n{}",
            String::from_utf8_lossy(&output.stdout)
        ));
        let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
        for name in [file.to_owned(), format!("{stem}.csv"), format!("{stem}.json")] {
            let bytes = std::fs::read(figures.join(&name)).map_err(|e| format!("figures/{name}: {e}"))?;
            let is_what_it_says = match name.rsplit_once('.').map_or("", |(_, ext)| ext) {
                "svg" => bytes.starts_with(b"<?xml") || bytes.starts_with(b"<svg"),
                "pdf" => bytes.starts_with(b"%PDF-1.") && bytes.ends_with(b"%%EOF\n"),
                "png" => png_size_of(&bytes).is_some(),
                "json" => bytes.starts_with(b"{") && String::from_utf8_lossy(&bytes).contains("\"revision\""),
                _ => !bytes.is_empty(),
            };
            if !is_what_it_says {
                return Err(format!("figures/{name} is not what its name says"));
            }
            let listed = format!("figures/{name}");
            if !made.contains(&listed) {
                made.push(listed);
            }
        }
    }
    // The periodogram was written in three formats: its one record lists all three.
    let record = std::fs::read_to_string(figures.join("periodogram-paper-column.json"))
        .map_err(|e| format!("figures/periodogram-paper-column.json: {e}"))?;
    for format in ["svg", "pdf", "png"] {
        if !record.contains(&format!("\"file\": \"periodogram-paper-column.{format}\"")) {
            return Err(format!("figures/periodogram-paper-column.json does not list the .{format}"));
        }
    }
    let python = std::env::var(crate::python::INTERPRETER).unwrap_or_else(|_| "python".to_owned());
    if let Some(lacking) = crate::python::missing(&python) {
        return Err(format!("the figure cannot be redrawn in Python: {lacking}"));
    }
    let code = "import sys; from cavendish_figures import load, redraw; \
                print(redraw(load(sys.argv[1]).unwrap(), sys.argv[2]).unwrap().name)";
    let from = figures.join("periodogram-paper-column.json").to_string_lossy().into_owned();
    let to = figures.join("redrawn.png").to_string_lossy().into_owned();
    let output = crate::gentle(&python)
        .args(["-c", code, &from, &to])
        .current_dir(root.join("python"))
        .output()
        .map_err(|e| format!("{python}: {e}"))?;
    if !output.status.success() || png_size(&figures.join("redrawn.png")).is_err() {
        return Err(format!("the Python redraw failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    said.push_str(
        "$ python -c \"from cavendish_figures import load, redraw; \
         redraw(load('periodogram-paper-column.json').unwrap(), 'redrawn.png').unwrap()\"\nredrawn.png\n",
    );
    made.push("figures/redrawn.png".to_owned());
    std::fs::write(figures.join("made.txt"), &said).map_err(|e| format!("figures/made.txt: {e}"))?;
    made.push("figures/made.txt".to_owned());

    // 8. Timings: a 10,000-shot series on one thread and on six, and the same bytes from both.
    let mut timings =
        String::from("Debug build (cav-core optimised), this machine; recorded, not gated.\n\n");
    let mut signals = Vec::new();
    for threads in ["1", "6"] {
        let target = temp(&format!("threads-{threads}"));
        let output = crate::gentle(&cav)
            .args(["run", "comb", "--exec", "set duration 20000", "--out", &target])
            .env("CAV_THREADS", threads)
            .output()
            .map_err(|e| format!("cav run: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "cav run on {threads} thread(s): {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let line = String::from_utf8_lossy(&output.stdout).replace(&target, "bundle");
        timings.push_str(&format!("CAV_THREADS={threads}: {}", line.trim_start()));
        signals.push(
            std::fs::read(Path::new(&target).join("signal.npy")).map_err(|e| format!("signal.npy: {e}"))?,
        );
    }
    if signals[0].is_empty() || signals[0] != signals[1] {
        return Err("the signal differs between one thread and six".to_owned());
    }
    timings.push_str("signal.npy: the same bytes on one thread and on six.\n\n");
    let shot = root.join("target").join("demo-timing.png").to_string_lossy().into_owned();
    for (what, script) in [
        (
            "staggered (five towers, each with its own offset, 1000 shots)",
            "load staggered; panes 2; show signal @2",
        ),
        ("cadence-lab with the designer open", "load cadence-lab; panes 2; show cadence @2"),
    ] {
        let lines =
            capture(&app, &["--screenshot", &shot, "--size", "1440x900", "--timings", "--exec", script])?;
        if !lines.contains("drag step") || lines.contains("NOT FINISHED") {
            return Err(format!("the timings of {what} are incomplete:\n{lines}"));
        }
        timings.push_str(&format!("## {what}\n"));
        timings.extend(lines.lines().skip(1).map(|line| format!("{line}\n")));
        timings.push('\n');
    }
    write(&out, "timings.txt", &timings, &mut made)?;

    eprintln!("xtask: demo S2 wrote {} files to {}:", made.len(), out.display());
    for file in &made {
        eprintln!("  {file}");
    }
    Ok(())
}

/// S3: voxel bodies drawn as trees, coloured by fields, with overlays; George's scaffold; a mixed
/// array and an array of none; a voxel view as a figure; the voxeliser's speed.
fn s3(root: &Path) -> Result<(), String> {
    let out = root.join("docs").join("slices").join("S3");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    step("build cav and cav-app", cargo(root, &["build", "--quiet", "-p", "cav", "-p", "cav-app"]))?;
    let (cav, app) = (binary(root, "cav"), binary(root, "cav-app"));
    let path = |name: &str| out.join(name).to_string_lossy().into_owned();
    let mut made: Vec<String> = Vec::new();
    let scratch = root.join("target").join("demo-s3");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
    let temp = |name: &str| scratch.join(name).to_string_lossy().replace('\\', "/");

    // 1 to 5. The application, headless.
    let close = |pane: u32| format!("view iso @{pane}; pan 0 0 2.5 @{pane}; zoom 9 @{pane}");
    let shots: [(&str, String); 7] = [
        ("shapes.png", format!("load platonic-row; {}; colour level", close(1))),
        (
            "colour-by-field.png",
            format!(
                "load platonic-row; panes 4; {}; {}; {}; {}; colour distance @1; colour mass @2; colour fill @3; \
                 colour phase 1 @4",
                close(1),
                close(2),
                close(3),
                close(4)
            ),
        ),
        (
            "overlay-leaves.png",
            "load sphere; pan 2 0 2.5; zoom 2.5; colour height; overlay leaves".to_owned(),
        ),
        ("overlay-cut.png", "load sphere; pan 2 0 2.5; zoom 2.5; colour mass; overlay cut 2".to_owned()),
        ("scaffold.png", "load scaffold; pan 0 0 5; zoom 16; colour height".to_owned()),
        ("mixed-array.png", "load mixed-array; panes 2; show flight @2; seek 20".to_owned()),
        ("zero-towers.png", "load no-towers; panes 2; show signal @2".to_owned()),
    ];
    for (file, script) in &shots {
        capture(&app, &["--screenshot", &path(file), "--size", "1440x900", "--exec", script])?;
        expect_png(&out, file, [1440, 900], &mut made)?;
    }
    // The same picture when it is taken again.
    let again = temp("again.png");
    capture(&app, &["--screenshot", &again, "--size", "1440x900", "--exec", &shots[1].1])?;
    if std::fs::read(out.join(shots[1].0)).ok() != std::fs::read(&again).ok() {
        return Err(format!("{} is not the same picture when it is taken again", shots[1].0));
    }

    // 6. A voxel view as a figure, and the cylinder against the point mass.
    let figures = out.join("figures");
    let _ = std::fs::remove_dir_all(&figures);
    std::fs::create_dir_all(&figures).map_err(|e| format!("{}: {e}", figures.display()))?;
    let mut said = String::new();
    for file in ["platonic-row.svg", "platonic-row.pdf", "platonic-row.png"] {
        let exec = "view top; proj ortho; colour height";
        let output = crate::gentle(&cav)
            .args(["figure", "platonic-row", "view", "--preset", "paper-full", "--exec", exec, "--out", file])
            .current_dir(&figures)
            .output()
            .map_err(|e| format!("cav figure: {e}"))?;
        if !output.status.success() {
            return Err(format!("cav figure platonic-row view: {}", String::from_utf8_lossy(&output.stderr)));
        }
        said.push_str(&format!(
            "$ cav figure platonic-row view --preset paper-full --exec \"{exec}\" --out {file}\n{}",
            String::from_utf8_lossy(&output.stdout)
        ));
        made.push(format!("figures/{file}"));
    }
    std::fs::write(figures.join("made.txt"), &said).map_err(|e| format!("figures/made.txt: {e}"))?;
    made.push("figures/made.txt".to_owned());

    // 7. A session: shapes and colours from the console; the mixed array's ragged bundle.
    let mut session = String::new();
    // Each line of the brief's §4 in turn: a body made of voxels added, listed, coloured and
    // overlaid; a pitch too fine for one tree refused (alone, so that nothing after it is lost);
    // a tower given its own instrument, and what each tower reads.
    let scripts = [
        r#"load unit-cube; add bodies {"label": "ball", "mass": 50, "shape": {"kind": {"Sphere": {"radius": 0.4}}, "pitch": 0.1, "supersample": 2}, "motion": {"Static": {"position": [-2, 0, 1.5]}}}; ls bodies[1].shape; ls bodies[1].shape.kind; colour phase 1; overlay leaves cut 1; stats"#,
        "load unit-cube; set bodies[0].shape.pitch 1e-4",
        r#"load mixed-array; set array.towers[0].instrument {"half_sequence": 0.6, "cycle": 1.5}; get array.towers[0].instrument; reading 1"#,
    ];
    for (n, script) in scripts.iter().enumerate() {
        // The second is refused by design: its refusal is the transcript.
        let answer = if n == 1 {
            match capture(&cav, &["exec", script]) {
                Ok(said) => return Err(format!("a pitch of 1e-4 m was not refused:\n{said}")),
                Err(refused) => refused,
            }
        } else {
            capture(&cav, &["exec", script])?
        };
        let wanted = ["radius", "pitch", "T = 0.9 s"][n];
        if !answer.contains(wanted) {
            return Err(format!("the session's `{script}` does not say `{wanted}`:\n{answer}"));
        }
        session.push_str(&format!("$ cav exec '{script}'\n{}\n\n", abridged(answer.trim_end(), 3000)));
    }
    let bundle = temp("mixed");
    let ran = capture(&cav, &["run", "mixed-array", "--out", &bundle])?;
    session.push_str(&format!(
        "$ cav run mixed-array --out target/demo-s3/mixed\n{}\n",
        ran.replace(&bundle, "target/demo-s3/mixed")
    ));
    for (name, bytes) in files_of(Path::new(&bundle))? {
        session.push_str(&format!("  {name:<24} {:>9} bytes\n", bytes.len()));
    }
    write(
        &out,
        "session.txt",
        &session.replace(&scratch.to_string_lossy().replace('\\', "/"), "target/demo-s3"),
        &mut made,
    )?;

    // 8. Timings: the voxeliser on spheres, optimised as the hot crate is; frame times with leaves.
    let mut timings = String::from("This machine; recorded, not gated.\n\n");
    let output = crate::gentle("cargo")
        .args(["run", "--quiet", "--release", "-p", "cav-core", "--example", "voxel_timing"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("voxel_timing: {e}"))?;
    if !output.status.success() {
        return Err(format!("voxel_timing: {}", String::from_utf8_lossy(&output.stderr)));
    }
    timings.push_str("## The voxeliser (release build)\n");
    timings.push_str(&String::from_utf8_lossy(&output.stdout));
    let shot = root.join("target").join("demo-timing.png").to_string_lossy().into_owned();
    for (what, script) in [
        ("platonic-row: five solids drawn as their leaves", "load platonic-row; colour height"),
        ("sphere: a voxel sphere at r/16 beside the analytic one", "load sphere; colour height"),
    ] {
        let lines =
            capture(&app, &["--screenshot", &shot, "--size", "1440x900", "--timings", "--exec", script])?;
        timings.push_str(&format!("\n## {what}\n"));
        timings.extend(lines.lines().skip(1).map(|line| format!("{line}\n")));
    }
    write(&out, "timings.txt", &timings, &mut made)?;

    eprintln!("xtask: demo S3 wrote {} files to {}:", made.len(), out.display());
    for file in &made {
        eprintln!("  {file}");
    }
    Ok(())
}

/// S4, big scenes (`docs/slices/S4.md` §4): the world near and far, its LOD bands, the box read
/// as a body, the flythrough measured, the paper drawings, and the world in a page.
fn s4(root: &Path, web: bool) -> Result<(), String> {
    let out = root.join("docs").join("slices").join("S4");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    step("build cav and cav-app", cargo(root, &["build", "--quiet", "-p", "cav", "-p", "cav-app"]))?;
    step(
        "build cav and cav-app (release, for the pictures and the measurements)",
        cargo(root, &["build", "--quiet", "--release", "-p", "cav", "-p", "cav-app"]),
    )?;
    let release = |name: &str| {
        root.join("target").join("release").join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
    };
    let (cav, app, fast) = (binary(root, "cav"), release("cav-app"), release("cav"));
    let path = |name: &str| out.join(name).to_string_lossy().into_owned();
    let mut made: Vec<String> = Vec::new();

    // 1. The application, headless: the world near and far, the bands, the box.
    // Pictures of `town` (stress-1e10 scaled down): a full stress-1e10 picture on this card held
    // the owner's desktop (TASK-S4-11); the full scene is measured below in bounded runs.
    let shots: [(&str, &str); 6] = [
        ("world-near.png", "load town; view iso; pan 0 0 0; zoom 45"),
        ("world-lod.png", "load town; view iso; pan 0 0 0; zoom 45; overlay bands"),
        ("world-far.png", "load town; pan 10 -10 2; orbit 225 7; zoom 14"),
        ("world-far-lod.png", "load town; pan 10 -10 2; orbit 225 7; zoom 14; overlay bands"),
        ("world-box.png", "load world-box; view iso; pan 6 5 1; zoom 26"),
        // Looking west along an axis, orthographic: the far field's rays have components that
        // are exactly zero (S4's review, I9).
        ("world-east-ortho.png", "load town; view east; proj ortho; pan 0 0 2; zoom 30"),
    ];
    for (file, script) in &shots {
        capture_within(
            &app,
            &["--screenshot", &path(file), "--size", "1440x900", "--exec", script],
            120,
            None,
        )?;
        expect_png(&out, file, [1440, 900], &mut made)?;
    }

    // 2. What the towers read of the box, and of the town's ground (each step within a limit,
    // as every step here is: S4's review, minor 6).
    let mut session = String::new();
    for script in ["load world-box; reading 1", "load town; reading 1; log"] {
        let answer = capture_within(&cav, &["exec", script], 300, None)?;
        session.push_str(&format!("$ cav exec \"{script}\"\n{answer}\n"));
    }
    write(&out, "readings.txt", &session, &mut made)?;

    // 3. The flythrough, measured: cold, from the region cache, and at 1440p.
    let cache = root.join("target").join("demo-s4-cache");
    let _ = std::fs::remove_dir_all(&cache);
    let cache = cache.to_string_lossy().into_owned();
    let mut bench = String::new();
    // Bounded: 300 frames at most, stopping at any frame over 250 ms; below normal priority.
    let runs = [
        ("fly-cold.csv", "1280x720", "300"),
        ("fly-warm.csv", "1280x720", "300"),
        ("fly-1440.csv", "2560x1440", "150"),
    ];
    for (file, size, frames) in runs {
        let target = path(file);
        let args = [
            "bench",
            "flythrough",
            "--frames",
            frames,
            "--size",
            size,
            "--cache",
            &cache,
            "--max-frame-ms",
            "250",
            "--out",
            &target,
        ];
        // Each run stops itself at a slow frame; this stops one that hangs.
        let said = capture_within(&fast, &args, 600, None)?;
        bench.push_str(&format!(
            "$ cav bench flythrough --frames {frames} --size {size} --cache <cache> --max-frame-ms 250 --out {file}\n{said}\n"
        ));
        made.push(file.to_owned());
    }
    write(&out, "flythrough.txt", &bench, &mut made)?;

    // 4. The paper drawings, as SVG: of `town` since the review (the full stress-1e10 is read
    // only by the bounded bench, the owner's rule after the freeze), cut at y = 74 m through its
    // building at x = 20 m.
    for (scene, file, extra) in [
        ("town", "paper-plan.svg", None),
        ("town", "paper-section.svg", Some("y,74")),
        ("world-box", "paper-world-box.svg", None),
    ] {
        let mut args = vec!["figure", scene, "paper", "--out", file];
        if let Some(cut) = extra {
            args.extend(["--section", cut]);
        }
        capture_within(&cav, &args, 300, Some(&out))?;
        made.push(file.to_owned());
    }

    // 5. The world in a page: the box, and the town (which streams on the page's one thread).
    if !web {
        eprintln!("xtask: demo S4: the page left out (--no-web); web-*.png and web.txt are as they were");
        eprintln!("xtask: demo S4 wrote {} files to {}:", made.len(), out.display());
        for file in &made {
            eprintln!("  {file}");
        }
        return Ok(());
    }
    crate::web::build(root)?;
    let chrome = crate::web::chrome()?;
    let server = crate::web::serve(&root.join(crate::web::DIST), 0)?;
    let mut log = String::new();
    for (file, script, seconds) in [
        ("web-world-box.png", "load world-box; view iso; pan 6 5 1; zoom 26", 15u32),
        ("web-town.png", "load town; view iso; pan 0 0 0; zoom 45", 60),
    ] {
        let url = format!("http://127.0.0.1:{}/#exec={}", server.port, script.replace(' ', "%20"));
        let target = out.join(file).to_string_lossy().into_owned();
        let chrome = chrome.to_string_lossy().into_owned();
        let seconds_text = seconds.to_string();
        // The page's own limit is `seconds`; this one stops a browser that hangs past it.
        let mut said = capture_within(
            Path::new("node"),
            &["scripts/web_screenshot.mjs", &chrome, &url, &target, &seconds_text],
            u64::from(seconds) + 90,
            Some(root),
        )
        .map_err(|why| format!("{file}: the page failed: {why}"))?;
        for home in ["USERPROFILE", "HOME"].iter().filter_map(|name| std::env::var(name).ok()) {
            said = said.replace(&home, "~");
        }
        log.push_str(&format!(
            "$ node scripts/web_screenshot.mjs <chrome> \"{url}\" {file} {seconds}\n{said}\n"
        ));
        expect_png(&out, file, [1440, 900], &mut made)?;
    }
    drop(server);
    write(&out, "web.txt", &log, &mut made)?;

    eprintln!("xtask: demo S4 wrote {} files to {}:", made.len(), out.display());
    for file in &made {
        eprintln!("  {file}");
    }
    Ok(())
}

/// S5 (`docs/slices/S5.md` §4), items 2 and 3 so far (TASK-S5-13; the rest is TASK-S5-23's):
/// the infinite plane with a building and a ditch, its readings; the layered planet with hills.
fn s5(root: &Path) -> Result<(), String> {
    let out = root.join("docs").join("slices").join("S5");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    step("build cav and cav-app", cargo(root, &["build", "--quiet", "-p", "cav"]))?;
    step(
        "build cav-app (release, for the pictures)",
        cargo(root, &["build", "--quiet", "--release", "-p", "cav-app"]),
    )?;
    let app = root.join("target").join("release").join(format!("cav-app{}", std::env::consts::EXE_SUFFIX));
    let cav = binary(root, "cav");
    let path = |name: &str| out.join(name).to_string_lossy().into_owned();
    let mut made: Vec<String> = Vec::new();
    let shots: [(&str, &str); 2] = [
        ("plane.png", "load plane; view iso; pan 0 3 0; zoom 30"),
        ("planet.png", "load planet; pan 30 0 0; orbit 200 12; zoom 4"),
    ];
    for (file, script) in &shots {
        capture_within(
            &app,
            &["--screenshot", &path(file), "--size", "1440x900", "--exec", script],
            120,
            None,
        )?;
        expect_png(&out, file, [1440, 900], &mut made)?;
    }
    let mut session = String::new();
    for script in ["load plane; reading 1; reading 2; reading 4", "load planet; reading 1; reading 2"] {
        let answer = capture_within(&cav, &["exec", script], 300, None)?;
        session.push_str(&format!("$ cav exec \"{script}\"\n{answer}\n"));
    }
    write(&out, "bases.txt", &session, &mut made)?;
    eprintln!("xtask: demo S5 wrote {} files to {}:", made.len(), out.display());
    for file in &made {
        eprintln!("  {file}");
    }
    Ok(())
}

/// S18, the editor (`docs/slices/S18.md` §5): the window as a first-time user meets it, its
/// menus as the palette, the tools, the keyboard's ring, the windows and the layouts; and the
/// end-to-end task as console lines. Release builds, each run within a wall-clock limit;
/// scenes `void`, `point`, `room`, `comb` and `town` only.
fn s18(root: &Path) -> Result<(), String> {
    let out = root.join("docs").join("slices").join("S18");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    step(
        "build cav and cav-app (release)",
        cargo(root, &["build", "--quiet", "--release", "-p", "cav", "-p", "cav-app"]),
    )?;
    let release = |name: &str| {
        root.join("target").join("release").join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
    };
    let (cav, app) = (release("cav"), release("cav-app"));
    let path = |name: &str| out.join(name).to_string_lossy().into_owned();
    let mut made: Vec<String> = Vec::new();
    let shots: [(&str, &str); 13] = [
        ("editor-empty.png", "load void"),
        ("editor-add-menu.png", "load void; palette add"),
        ("editor-palette.png", "load point; palette sph"),
        ("editor-gizmo.png", "load point; add sphere at 2 2 0.25; select body 2; tool move; frame"),
        ("editor-keyboard.png", "load point; select body 1; focus field/bodies[0].mass"),
        ("editor-sheet.png", "load point; keys"),
        (
            "editor-history.png",
            "load point; set bodies[0].mass 5; set duration 20; add cube at 2 -2 0.25; \
             set bodies[1].motion.Static.position [2.5, -2, 0.25]; history",
        ),
        ("editor-context.png", "load point; select body 1; context"),
        ("editor-wizard.png", "load room; wizard"),
        ("editor-instrument.png", "load point; layout instrument"),
        ("editor-analysis.png", "load comb; layout analysis"),
        ("editor-place.png", "load void; tool place; pointer 0.55 0.7"),
        ("editor-maximised.png", "load town; view iso; pan 0 0 0; zoom 45; maximise"),
    ];
    for (file, script) in &shots {
        capture_within(
            &app,
            &["--screenshot", &path(file), "--size", "1440x900", "--exec", script],
            180,
            None,
        )?;
        expect_png(&out, file, [1440, 900], &mut made)?;
    }
    // The editor's budgets of §6, measured in the release build on `point` with no window.
    let timings = capture_within(
        &app,
        &["--screenshot", &path("timings.png"), "--size", "1440x900", "--exec", "load point", "--timings"],
        300,
        None,
    )?;
    let _ = std::fs::remove_file(out.join("timings.png"));
    // The first line names the throwaway picture's path on this machine: not kept.
    let timings: String = timings
        .lines()
        .filter(|l| !l.contains("timings.png"))
        .map(|l| {
            format!(
                "{l}
"
            )
        })
        .collect();
    write(&out, "timings.txt", &timings, &mut made)?;
    // The end-to-end task as console lines (what the editor's clicks issue), with tower 1's phase.
    let script = "load room; add sphere at 2 0 0.25; set bodies[0].motion.Static.position [2, 2, 0.25]; \
                  journal; reading";
    let answer = capture_within(&cav, &["exec", script], 300, None)?;
    write(&out, "session.txt", &format!("$ cav exec \"{script}\"\n{answer}"), &mut made)?;
    eprintln!("xtask: demo S18 wrote {} files to {}:", made.len(), out.display());
    for file in &made {
        eprintln!("  {file}");
    }
    Ok(())
}

/// S19, the editor's second half (`docs/slices/S19.md` §5): the side panels redesigned
/// (`docs/design/panels.md`), materials, a probe, an imported mesh. Release builds, each run
/// within a wall-clock limit; scenes `point`, `world-box` and `town` only.
fn s19(root: &Path) -> Result<(), String> {
    let out = root.join("docs").join("slices").join("S19");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    step(
        "build cav and cav-app (release)",
        cargo(root, &["build", "--quiet", "--release", "-p", "cav", "-p", "cav-app"]),
    )?;
    let app = root.join("target").join("release").join(format!("cav-app{}", std::env::consts::EXE_SUFFIX));
    let path = |name: &str| out.join(name).to_string_lossy().into_owned();
    let mut made: Vec<String> = Vec::new();
    // A steel box with a lead part beside it, as `point`'s second body.
    let materials = "add bodies {\"label\":\"block\",\"mass\":1.0,\"shape\":{\"kind\":{\"Cuboid\":{\"size\":[0.6,0.6,0.6]}},\"pitch\":0.05,\"supersample\":2,\"material\":\"steel\",\"parts\":[{\"mode\":\"Add\",\"material\":\"lead\",\"at\":[0.45,0,0],\"kind\":{\"Solid\":{\"shape\":{\"Sphere\":{\"radius\":0.3}}}}},{\"mode\":\"Paint\",\"material\":\"copper\",\"at\":[0,0,0.3],\"kind\":{\"Stroke\":{\"radius\":0.1,\"points\":[[-0.3,-0.3,0],[0.3,0.3,0]]}}}]},\"motion\":{\"Static\":{\"position\":[2,2,0.5]}}}";
    let shots: Vec<(&str, String)> = vec![
        ("panels-body.png", "load point; add sphere at 2 2 0.25; select body 2; tool move; frame".to_owned()),
        ("panels-scene.png", "load point; layout instrument".to_owned()),
        ("panels-tower.png", "load town; select tower 2".to_owned()),
        ("panels-readings.png", "load point; tab readings".to_owned()),
        ("materials.png", format!("load point; {materials}; select body 2; colour material; frame")),
        (
            "probe.png",
            "load point; add sphere at 2 2 0.25; select body 2; frame; tool probe; probe-at 0.5 0.5"
                .to_owned(),
        ),
        // After the review: the Probe on a world scene names what it does not count (I3), and a
        // body of materials' parts as the inspector lists them (I5).
        ("probe-town.png", "load town; tool probe; probe-at 0.5 0.6".to_owned()),
    ];
    for (file, script) in &shots {
        capture_within(
            &app,
            &["--screenshot", &path(file), "--size", "1440x900", "--exec", script],
            180,
            None,
        )?;
        expect_png(&out, file, [1440, 900], &mut made)?;
    }
    // The whole of a body of materials' inspector, its parts listed: a taller window.
    let file = "inspector-parts.png";
    let script = format!("load point; {materials}; select body 2; colour material; frame");
    capture_within(
        &app,
        &["--screenshot", &path(file), "--size", "1440x1500", "--exec", &script],
        180,
        None,
    )?;
    expect_png(&out, file, [1440, 1500], &mut made)?;
    eprintln!("xtask: demo S19 wrote {} files to {}:", made.len(), out.display());
    for file in &made {
        eprintln!("  {file}");
    }
    Ok(())
}

/// S20, the workspace (`docs/slices/S20.md` §5): the dense panels (the same selections and tabs
/// as S19's `panels-*.png`, to stand beside them), the mosaic, a viewport's header, the tool
/// palette, the transport, folders, the first-person camera and a fast body's path; the
/// session's console lines and §6's timings. Release builds, each run within a wall-clock limit
/// and stopped by its own handle; scenes `point` and `town` only.
fn s20(root: &Path) -> Result<(), String> {
    let out = root.join("docs").join("slices").join("S20");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    step(
        "build cav and cav-app (release)",
        cargo(root, &["build", "--quiet", "--release", "-p", "cav", "-p", "cav-app"]),
    )?;
    let release = |name: &str| {
        root.join("target").join("release").join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
    };
    let (cav, app) = (release("cav"), release("cav-app"));
    let path = |name: &str| out.join(name).to_string_lossy().into_owned();
    let mut made: Vec<String> = Vec::new();
    let circle = "set bodies[1].motion {\"Circular\": {\"centre\": [0, 0, 2], \"u\": [2, 0, 0], \
                  \"v\": [0, 2, 0], \"frequency\": 5, \"phase\": 0}}";
    // A second mass going round a point a metre from the centre tower once in ten seconds, so
    // that the plots of the mosaic's pictures have something to show (`point` alone is at rest,
    // and a circle about the tower itself is at one distance from it).
    let slow = circle.replace("\"frequency\": 5", "\"frequency\": 0.1").replace("[0, 0, 2]", "[1, 0, 2]");
    // Along the west wall of a building of `town`, on foot: the walker falls to the ground,
    // walks east, and the eye is then put beside the wall looking along it.
    let on_foot = "load town; eye 29 -47.5 2 0 0; walk; walk-for 2; walk-for 3 w";
    let shots: Vec<(&str, String)> = vec![
        // 1. The panels, as S19's `panels-*.png` were taken.
        ("dense-body.png", "load point; add sphere at 2 2 0.25; select body 2; tool move; frame".to_owned()),
        ("dense-scene.png", "load point; layout instrument".to_owned()),
        ("dense-tower.png", "load town; select tower 2".to_owned()),
        ("dense-readings.png", "load point; tab readings".to_owned()),
        // 2. The mosaic: four view types; six viewports; one popped out.
        (
            "mosaic.png",
            format!(
                "load point; mass 5 at 2 0 2; {slow}; mosaic \"AAB;CDB\"; show flight @2; show signal @3; show periodogram @4"
            ),
        ),
        (
            "mosaic-six.png",
            "load town; mosaic \"ABC;DEF\"; pane 1; view iso; pan 0 0 0; zoom 45; show signal @2; \
             show periodogram @3; pane 4; view top; pan 0 0 0; zoom 25; show flight @5; show cadence @6; pane 1"
                .to_owned(),
        ),
        (
            "mosaic-float.png",
            format!(
                "load point; mass 5 at 2 0 2; {slow}; mosaic \"AB;CD\"; show signal @2; show periodogram @3; show flight @4; viewport pop-out 4"
            ),
        ),
        // 3. A viewport's header, its Overlays popover open, and the navigation gizmo.
        ("header.png", "load point; add sphere at 2 2 0.25; frame; viewport overlays".to_owned()),
        // 4. The palette at the right edge, Place's flyout open, the options row under the header.
        ("palette.png", "load point; tool place; tool-palette right; flyout place".to_owned()),
        // 5. The transport, the end menu open.
        ("transport.png", "load point; seek 3.2; end menu".to_owned()),
        // 6. Folders: nested, one folded, two bodies hidden.
        (
            "folders.png",
            "load point; mass 5 at 1 1 1; mass 6 at -1 1 1; mass 7 at 0 2 1; add sphere at 2 -2 1; \
             add cube at -2 -2 1; folder new rig; folder new clamps in rig; folder new weights; \
             folder new north in array; folder move body 2 body 3 clamps; folder move body 4 rig; \
             folder move body 5 body 6 weights; folder move tower 2 tower 3 north; hide sphere; hide cube; \
             tree-fold clamps; select body 4"
                .to_owned(),
        ),
        // 7. On foot in `town` along a wall, the walking speed's read-out showing (the speed in use);
        // and through it, noclip (flying: the flying speed's).
        ("fly.png", format!("{on_foot}; eye 34.7 -47.5 1.32 100 -8; camera-set walk-speed 4.3")),
        ("noclip.png", format!("{on_foot}; noclip; walk-for 0.5 w; camera-set fly-speed 4")),
        // 8. A 5 Hz circle, drawn round.
        ("path.png", format!("load point; mass 5 at 2 0 2; {circle}; orbit -60 50; select body 2")),
    ];
    let mut window_lines = String::new();
    for (file, script) in &shots {
        capture_within(
            &app,
            &["--screenshot", &path(file), "--size", "1440x900", "--exec", script],
            180,
            None,
        )?;
        expect_png(&out, file, [1440, 900], &mut made)?;
        window_lines.push_str(&format!("{file}:\n  $ cav-app --screenshot {file} --exec \"{script}\"\n"));
    }

    // 9. The session of §1 as console lines. `cav exec` runs the engine's commands (the mosaic,
    // the folders, the duration: what replays from a journal); the window's own (`step`, `end`,
    // `speed`, `walk`, `eye`, `walk-for`, `noclip`, `tool`, `tool-palette`, `flyout`,
    // `viewport`) have no meaning without a window and were run by `cav-app --exec`, as the
    // lines under each picture's name say.
    let script = "load point; mosaic \"AAB;CDB\"; show flight @2; show signal @3; show periodogram @4; \
                  split right @1; swap 1 2; close @2; mosaic even; mass 5 at 1 1 1; folder new rig; \
                  folder new clamps in rig; folder move body 2 clamps; folder rename clamps grips; \
                  set duration 20; seek 3.2; stats; undo; undo; undo; undo; undo; journal; reading";
    let answer = capture_within(&cav, &["exec", script], 300, None)?;
    let session = format!(
        "$ cav exec \"{script}\"\n{answer}\n\
         # The window's own commands (not the engine's: `cav exec` has no window to act on), as run\n\
         # for each picture by the headless application:\n{window_lines}"
    );
    write(&out, "session.txt", &session, &mut made)?;

    // §6's numbers, in the release build, with no window and no GPU.
    let timings = capture_within(&app, &["--measure"], 600, None)?;
    write(&out, "timings.txt", &timings, &mut made)?;
    eprintln!("xtask: demo S20 wrote {} files to {}:", made.len(), out.display());
    for file in &made {
        eprintln!("  {file}");
    }
    Ok(())
}

/// [`capture`] with a wall-clock limit, in `dir` when given: past `seconds` the child is stopped
/// by its own handle and the step fails, so a picture that would hold the machine cannot
/// (TASK-S4-11).
fn capture_within(program: &Path, args: &[&str], seconds: u64, dir: Option<&Path>) -> Result<String, String> {
    let mut command = crate::gentle(program);
    command.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    let mut child = command.spawn().map_err(|e| format!("{}: {e}", program.display()))?;
    let started = std::time::Instant::now();
    // Both pipes drained on threads of their own while the child runs: a child that writes more
    // than a pipe holds would otherwise block, and be stopped at the limit for that alone (S4's
    // review, minor 5). Each thread ends when its pipe closes, at the child's end or its kill.
    let drain = |pipe: Option<Box<dyn std::io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut bytes);
            }
            bytes
        })
    };
    let out = drain(child.stdout.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>));
    let err = drain(child.stderr.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>));
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            let stdout = out.join().unwrap_or_default();
            let stderr = err.join().unwrap_or_default();
            if !status.success() {
                return Err(format!(
                    "`{} {}` failed ({status}): {}",
                    program.display(),
                    args.join(" "),
                    String::from_utf8_lossy(&stderr)
                ));
            }
            return Ok(String::from_utf8_lossy(&stdout).replace("\r\n", "\n"));
        }
        if started.elapsed().as_secs() >= seconds {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "`{} {}` took over {seconds} s and was stopped",
                program.display(),
                args.join(" ")
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
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

/// A client of `cav mcp` over stdio, keeping a transcript of what was said.
struct Mcp {
    child: std::process::Child,
    stdin: Option<std::process::ChildStdin>,
    stdout: BufReader<std::process::ChildStdout>,
    id: u32,
    transcript: String,
}

impl Mcp {
    fn start(cav: &Path, heading: &str) -> Result<Self, String> {
        let mut child = crate::gentle(cav)
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cav mcp: {e}"))?;
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().expect("stdout is piped"));
        Ok(Self { child, stdin, stdout, id: 0, transcript: format!("{heading}\n\n") })
    }

    /// One request and its answer, both added to the transcript under `note`.
    fn exchange(&mut self, method: &str, params: &str, note: &str) -> Result<String, String> {
        self.id += 1;
        let id = self.id;
        let request = format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#);
        let stdin = self.stdin.as_mut().expect("the session is open");
        writeln!(stdin, "{request}").and_then(|()| stdin.flush()).map_err(|e| format!("cav mcp: {e}"))?;
        let marker = format!("\"id\":{id}");
        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).map_err(|e| format!("cav mcp: {e}"))?;
            if read == 0 {
                return Err(format!("cav mcp closed its output during `{method}`"));
            }
            if line.contains(&marker) {
                self.transcript
                    .push_str(&format!("# {note}\n→ {request}\n← {}\n\n", abridged(line.trim(), 700)));
                if method == "initialize" {
                    // The client confirms the handshake; a notification has no answer.
                    let confirmed = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
                    writeln!(stdin, "{confirmed}")
                        .and_then(|()| stdin.flush())
                        .map_err(|e| format!("cav mcp: {e}"))?;
                    self.transcript.push_str(&format!("→ {confirmed}\n\n"));
                }
                return Ok(line);
            }
        }
    }

    /// A tool call.
    fn call(&mut self, tool: &str, arguments: &str, note: &str) -> Result<String, String> {
        self.exchange("tools/call", &format!(r#"{{"name":"{tool}","arguments":{arguments}}}"#), note)
    }

    fn hello(&mut self) -> Result<(), String> {
        let hello = self.exchange(
            "initialize",
            r#"{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"xtask-demo","version":"0"}}"#,
            "the handshake",
        )?;
        if hello.contains("cavendish") {
            Ok(())
        } else {
            Err(format!("the server did not introduce itself: {hello}"))
        }
    }

    /// Closes the session (the server exits when its input ends) and gives the transcript.
    fn finish(mut self) -> Result<String, String> {
        drop(self.stdin.take());
        let status = self.child.wait().map_err(|e| format!("cav mcp: {e}"))?;
        if status.success() { Ok(self.transcript) } else { Err(format!("cav mcp exited with {status}")) }
    }
}

/// Speaks MCP to `cav mcp` as an agent would: the handshake, the tool list, the schema, a scene, a
/// refusal, and a picture. Returns the transcript and the PNG that `render_view` sent.
fn mcp_session(cav: &Path) -> Result<(String, Vec<u8>), String> {
    let mut mcp =
        Mcp::start(cav, "An MCP session with `cav mcp` over stdio, as `cargo xtask demo S0` ran it.")?;
    let mut picture = None;
    mcp.hello()?;
    let mut exchange = |method: &str, params: &str, note: &str| mcp.exchange(method, params, note);

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

    let transcript = mcp.finish()?;
    let picture = picture.ok_or("render_view sent no picture")?;
    Ok((transcript, picture))
}

/// The scene `name` as `export scene` writes it natively, by `cav exec`.
fn cav_text(root: &Path, name: &str) -> Result<String, String> {
    step("build cav", cargo(root, &["build", "--quiet", "-p", "cav"]))?;
    let file = root.join("target").join("demo-s17").join(format!("{name}-native.ron"));
    let target = file.to_string_lossy().replace('\\', "/");
    capture(&binary(root, "cav"), &["exec", &format!("load {name}; save {target}")])?;
    std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))
}

/// Whether `bytes` look like a whole JSON object: `{` first and `}` last, past white space.
fn serde_json_like(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim();
    text.starts_with('{') && text.ends_with('}')
}

/// S17: the application as a web page. The page is built (`cargo xtask web build`), served from
/// this process, and photographed in headless Chrome with WebGPU (`scripts/web_screenshot.mjs`,
/// which starts and closes its own browser: no window opens). Each picture is taken after a wait,
/// and fails if the page's console has an error or a panic.
fn s17(root: &Path) -> Result<(), String> {
    let out = root.join("docs").join("slices").join("S17");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    crate::web::build(root)?;
    let chrome = crate::web::chrome()?;
    let server = crate::web::serve(&root.join(crate::web::DIST), 0)?;
    let mut made: Vec<String> = Vec::new();
    let mut log = String::new();
    let shots: [(&str, &str, u32); 4] = [
        ("web-room.png", "", 8),
        ("web-comb.png", "load comb; panes 4; show signal @2; show periodogram @3; show cadence @4", 20),
        ("web-sphere.png", "load sphere; pan 2 0 2.5; zoom 2.5; colour mass; overlay cut 2", 12),
        (
            "web-platonic.png",
            "load platonic-row; panes 2; view iso @1; pan 0 0 2.5 @1; zoom 9 @1; colour level @1; view top @2; colour height @2",
            12,
        ),
    ];
    for (file, script, seconds) in shots {
        let fragment =
            if script.is_empty() { String::new() } else { format!("#exec={}", script.replace(' ', "%20")) };
        let url = format!("http://127.0.0.1:{}/{fragment}", server.port);
        let target = out.join(file);
        let output = crate::gentle("node")
            .current_dir(root)
            .arg("scripts/web_screenshot.mjs")
            .arg(&chrome)
            .arg(&url)
            .arg(&target)
            .arg(seconds.to_string())
            .output()
            .map_err(|e| format!("node: {e}"))?;
        // The log names files under the builder's home (the registry's sources): not for the record.
        let mut said = String::from_utf8_lossy(&output.stdout).into_owned();
        for home in ["USERPROFILE", "HOME"].iter().filter_map(|name| std::env::var(name).ok()) {
            said = said.replace(&home, "~");
        }
        log.push_str(&format!(
            "$ node scripts/web_screenshot.mjs <chrome> \"{url}\" {file} {seconds}
{said}
"
        ));
        if !output.status.success() {
            return Err(format!(
                "{file}: the page failed: {said}{}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        expect_png(&out, file, [1440, 900], &mut made)?;
    }
    // Downloads. A link's commands may make files, but the page holds them behind a button until
    // the person clicks: opened with no click, nothing arrives (the control).
    let downloads = root.join("target").join("demo-s17").join("downloads");
    let page = |url: &str, click: bool, seconds: u32| -> Result<String, String> {
        let _ = std::fs::remove_dir_all(&downloads);
        std::fs::create_dir_all(&downloads).map_err(|e| format!("{}: {e}", downloads.display()))?;
        let mut node = crate::gentle("node");
        node.current_dir(root).env("CAV_DOWNLOADS", &downloads).arg("scripts/web_screenshot.mjs");
        if click {
            node.env("CAV_CLICK", "held-downloads");
        }
        let output = node
            .arg(&chrome)
            .arg(url)
            .arg(root.join("target").join("demo-s17").join("page.png"))
            .arg(seconds.to_string())
            .output()
            .map_err(|e| format!("node: {e}"))?;
        let said = String::from_utf8_lossy(&output.stdout).into_owned();
        if output.status.success() { Ok(said) } else { Err(format!("the download page failed: {said}")) }
    };
    std::fs::create_dir_all(root.join("target").join("demo-s17")).map_err(|e| e.to_string())?;
    let url = format!("http://127.0.0.1:{}/#exec=load%20comb;%20save%20comb.ron", server.port);
    let said = page(&url, false, 6)?;
    if downloads.join("comb.ron").exists() {
        return Err("a link's `save comb.ron` was downloaded without a click".into());
    }
    log.push_str(&format!(
        "$ (page, no click) {url}\n{said}nothing downloaded: the file waits behind the button\n\n"
    ));
    // With a click: a scene and two figures (picture, data, record). `pass` moves in a straight
    // line, so its signal is the same bits in wasm as natively and its figure's files must be the
    // same bytes; comb's periodogram goes through `sin` (THROUGH_LIBM), so its files are only
    // checked to have arrived whole.
    let script =
        "load pass; save pass.ron; figure signal to pass.svg; load comb; figure periodogram to comb.svg";
    let url = format!("http://127.0.0.1:{}/#exec={}", server.port, script.replace(' ', "%20"));
    let said = page(&url, true, 20)?;
    let native_dir = root.join("target").join("demo-s17").join("native-figure");
    let _ = std::fs::remove_dir_all(&native_dir);
    std::fs::create_dir_all(&native_dir).map_err(|e| format!("{}: {e}", native_dir.display()))?;
    step("build cav", cargo(root, &["build", "--quiet", "-p", "cav"]))?;
    let target = native_dir.join("pass.svg").to_string_lossy().replace('\\', "/");
    capture(&binary(root, "cav"), &["exec", &format!("load pass; figure signal to {target}")])?;
    let mut compared = Vec::new();
    let mut files = 0;
    let natives = std::fs::read_dir(&native_dir).map_err(|e| format!("{}: {e}", native_dir.display()))?;
    for entry in natives {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let web = std::fs::read(downloads.join(&name))
            .map_err(|e| format!("{name}: the page's figure did not arrive as a download: {e}"))?;
        let native = std::fs::read(&path).map_err(|e| format!("{name} (native): {e}"))?;
        // The record names the platform that made the numbers: the one field that must differ.
        let platform = |bytes: &[u8]| -> (String, String) {
            let text = String::from_utf8_lossy(bytes).into_owned();
            let named =
                text.lines().find_map(|line| line.trim().strip_prefix("\"platform\": ")).map(str::to_owned);
            let named = named.unwrap_or_default();
            (text.replace(&named, "\"…\","), named)
        };
        let (web, native) = if name.ends_with(".json") {
            let ((web, on_web), (native, natively)) = (platform(&web), platform(&native));
            if on_web == natively {
                return Err(format!("{name}: the page's record names the native platform ({natively})"));
            }
            let bare = |named: &str| named.trim_end_matches(',').trim_matches('"').to_owned();
            compared.push(format!(
                "{name}: platform {} against {}, the rest the same bytes",
                bare(&on_web),
                bare(&natively)
            ));
            (web.into_bytes(), native.into_bytes())
        } else {
            (web, native)
        };
        if web != native {
            return Err(format!("{name}: the downloaded figure differs from the native one"));
        }
        compared.push(format!("{name} {} bytes", web.len()));
        files += 1;
    }
    if files != 3 {
        return Err(format!("the native figure made {files} files, not a picture, its data and its record"));
    }
    for name in ["comb.svg", "comb.csv", "comb.json"] {
        let bytes =
            std::fs::read(downloads.join(name)).map_err(|e| format!("{name}: not downloaded: {e}"))?;
        let whole = match name {
            "comb.svg" => {
                (bytes.starts_with(b"<?xml") || bytes.starts_with(b"<svg"))
                    && bytes.trim_ascii_end().ends_with(b"</svg>")
            }
            "comb.json" => serde_json_like(&bytes),
            _ => !bytes.is_empty(),
        };
        if !whole {
            return Err(format!("{name}: downloaded but not whole"));
        }
        compared.push(format!("{name} {} bytes (arrived whole)", bytes.len()));
    }
    let downloaded = std::fs::read_to_string(downloads.join("pass.ron"))
        .map_err(|e| format!("the page's `save pass.ron` did not arrive as a download: {e}"))?;
    let native = cav_text(root, "pass")?;
    // ron ends lines with the platform's own ending (CRLF on Windows, LF in a page and on Linux):
    // the same scene, compared line by line.
    if downloaded.lines().ne(native.lines()) {
        return Err("the downloaded pass.ron differs from the scene natively saved".into());
    }
    log.push_str(&format!(
        "$ (page, clicking the button) {url}
{said}downloaded: {}; pass.ron {} bytes, line for line the native `save`

",
        compared.join(", "),
        downloaded.len()
    ));

    // A browser without WebGPU, and one with WebGPU but no adapter: the page's message, and the
    // module never fetched.
    let url = format!("http://127.0.0.1:{}/", server.port);
    for (file, mode, what) in [
        ("web-no-webgpu.png", "no-webgpu", "without WebGPU"),
        ("web-no-adapter.png", "no-adapter", "with WebGPU but no adapter"),
    ] {
        let output = crate::gentle("node")
            .current_dir(root)
            .args(["scripts/web_screenshot.mjs"])
            .arg(&chrome)
            .arg(&url)
            .arg(out.join(file))
            .args(["4", "1440x900", mode])
            .output()
            .map_err(|e| format!("node: {e}"))?;
        let said = String::from_utf8_lossy(&output.stdout).into_owned();
        log.push_str(&format!(
            "$ node scripts/web_screenshot.mjs <chrome> \"{url}\" {file} 4 1440x900 {mode}
{said}
"
        ));
        if !output.status.success() {
            return Err(format!("{file}: the page {what} did not say so: {said}"));
        }
        expect_png(&out, file, [1440, 900], &mut made)?;
    }
    drop(server);
    std::fs::write(out.join("web-console.txt"), &log).map_err(|e| format!("web-console.txt: {e}"))?;
    eprintln!("xtask: demo S17: {} in {}", made.join(", "), out.display());
    Ok(())
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
