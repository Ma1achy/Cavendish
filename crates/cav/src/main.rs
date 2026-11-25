//! `cav`: Cavendish without a window. Every subcommand drives the same engine API as the app.

mod args;
mod bench;
mod doctor;
mod mcp;
mod session;

use std::process::ExitCode;

use args::Args;
use session::Session;

const USAGE: &str = "\
cav <command>

  doctor                         the GPU adapters, the device, and a render self-test
  render <scene> --out <file.png> [--view iso|top|bottom|north|south|east|west]
         [--proj persp|ortho] [--style shaded|flat|xray|wireframe|paper]
         [--size 1280x720] [--exec \"<commands>\"]
                                 draw a scene to a PNG, with no window
  bench flythrough [--scene stress-1e10] [--frames 600] [--size 1280x720] [--speed 15] [--cache <dir>]
                   [--out <trace.csv>] [--max-frame-ms 250]
                                 fly a scripted camera over a world (stress-1e10, or e.g. town, the loop
                                 scaled to it) offscreen and print (or write) the frame-time trace
  scenes                         list the built-in scene library
  schema [path]                  print the JSON Schema of the scene spec, or of the field at a path
  get <scene> [path]             print a scene, or the value at a path of it, as JSON
  ls <scene> [path]              list what is inside a path: each field's value, unit, kind and range
  set <scene> <path> <value> [--out <file.ron>]
                                 set the value at a path (JSON, or a bare word for text) and save the
                                 scene: over the file it came from, or to --out (needed for a library scene)
  exec \"<commands>\"              run console commands (separated by `;`) and print what each answers;
                                 `cav exec help` lists them
  run <scene> --out <dir> [--preset aion10|aice|file.ron] [--exec \"<commands>\"] [--patience <seconds>]
                                 compute the scene's signal and write its bundle: .npy arrays, each tower's
                                 readout times, the numerics and the error estimate (see the manifest)
  figure <scene> <signal|periodogram|cadence|flight|view|paper> --out <file.svg|.pdf|.png>
         [--preset paper-column|paper-full|slide|poster|screen] [--tower <n>]
         [--channel gradiometer|lower|upper] [--span <from>,<to>] [--size <W>x<H>] [--dpi <n>]
         [--section <x|y>,<m>] [--no-title] [--exec \"<commands>\"] [--patience <seconds>]
                                 write a figure in a preset's style and size (size in mm; dpi for a PNG),
                                 with its data (.csv or .npy) and a .json of axes, units and provenance;
                                 --no-title leaves the title out; paper is a technical drawing of
                                 the scene, a plan or (--section) a vertical cut at x or y = m metres
  replay <journal.json> [--out <dir>] [--patience <seconds>]
                                 run a saved session again (`journal save <file>` writes one); what it
                                 exported is written inside --out (a relative path with its folders, any
                                 other under its file name), or not at all; an export waits for its signal,
                                 an hour unless --patience says otherwise; nothing else is waited for
  mcp                            serve the engine to an agent: an MCP server on stdin and stdout
  licences                       the licences of what is built into the binaries (the two fonts)
  fonts --out <dir>              write the two faces built into the binaries and their licence, for
                                 redrawing a figure in them (python: redraw(figure, out, fonts = dir))
  help                          this text

<scene> is a library name (see `cav scenes`) or the path of a .ron file.
";

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    match run(&raw) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("cav: {why}");
            ExitCode::FAILURE
        }
    }
}

fn run(raw: &[String]) -> Result<(), String> {
    let Some((command, rest)) = raw.split_first() else {
        print!("{USAGE}");
        return Ok(());
    };
    match command.as_str() {
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
            Ok(())
        }
        "doctor" => {
            Args::parse(rest, &[])?.finish(0)?;
            doctor::run()
        }
        "scenes" => {
            Args::parse(rest, &[])?.finish(0)?;
            println!("{}", Session::new().engine.run_line("scenes")?);
            Ok(())
        }
        "schema" => {
            let words = Args::parse(rest, &[])?.finish_up_to(1)?;
            let query = cav_engine::Query::Schema(words.first().cloned());
            println!("{}", cav_engine::render(&Session::new().engine.query(&query)?));
            Ok(())
        }
        "get" | "ls" => {
            let words = Args::parse(rest, &[])?.finish_up_to(2)?;
            let Some(scene) = words.first() else {
                return Err(format!("`cav {command}` needs a scene: a library name or a .ron file"));
            };
            let mut session = Session::new();
            session.load(scene)?;
            let path = words.get(1).cloned();
            let query =
                if command == "get" { cav_engine::Query::Spec(path) } else { cav_engine::Query::Ls(path) };
            println!("{}", cav_engine::render(&session.engine.query(&query)?));
            Ok(())
        }
        "set" => set(rest),
        "replay" => replay(rest),
        "run" => run_scene(rest),
        "figure" => figure(rest),
        "exec" => {
            let args = Args::parse(rest, &[])?;
            let [script]: [String; 1] = args.finish(1)?.try_into().map_err(|_| "exec takes one argument")?;
            let mut session = Session::new();
            // Print what ran before reporting a failure, so the output shows how far the script got.
            let mut failure = None;
            // The engine's own splitting: quotes keep a `;` together, a `#` line is a comment.
            for line in cav_engine::console::statements(&script) {
                match session.engine.run_line(line) {
                    Ok(answer) => println!("{answer}"),
                    Err(why) => {
                        failure = Some(format!("`{line}`: {why}"));
                        break;
                    }
                }
            }
            failure.map_or(Ok(()), Err)
        }
        "render" => render(rest),
        "bench" => bench(rest),
        "mcp" => {
            Args::parse(rest, &[])?.finish(0)?;
            mcp::serve()
        }
        "licences" | "licenses" => {
            Args::parse(rest, &[])?.finish(0)?;
            println!(
                "{} (from the file in this repository) and Ubuntu Light (the copy egui carries) are built into \
                 cav and cav-app, unmodified, under the Ubuntu Font Licence 1.0. A figure written as SVG or \
                 PDF holds its text as outlines of their glyphs: a document created using the fonts, which \
                 the licence's clause 4 leaves under no obligation.\n",
                cav_plot::fonts::MONO_FONT_NAME
            );
            println!("{}", cav_plot::fonts::MONO_FONT_LICENCE);
            Ok(())
        }
        "fonts" => fonts(rest),
        other => Err(format!("`{other}` is not a command\n\n{USAGE}")),
    }
}

/// `cav fonts`: the two faces the binaries carry, unmodified, and their licence beside them, so
/// that a figure can be redrawn in them (`cavendish_figures.redraw(figure, out, fonts = dir)`).
fn fonts(rest: &[String]) -> Result<(), String> {
    use cav_plot::fonts::{Face, MONO_FONT, MONO_FONT_LICENCE, bytes};
    let mut args = Args::parse(rest, &["out"])?;
    let out = args.option("out").ok_or("fonts needs --out <directory> for the font files")?;
    args.finish(0)?;
    let dir = std::path::Path::new(&out);
    std::fs::create_dir_all(dir).map_err(|e| format!("{out}: {e}"))?;
    let files: [(&str, &[u8]); 3] = [
        ("Ubuntu-Light.ttf", bytes(Face::Sans)),
        ("UbuntuMono-Regular.ttf", MONO_FONT),
        ("UFL.txt", MONO_FONT_LICENCE.as_bytes()),
    ];
    for (name, content) in files {
        let path = dir.join(name);
        std::fs::write(&path, content).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("{}", path.display());
    }
    Ok(())
}

/// `cav run`: load, any edits asked for, wait for the signal, write the bundle. Each step is the
/// console command a person would type, so the run is a short session like any other.
fn run_scene(rest: &[String]) -> Result<(), String> {
    let mut args = Args::parse(rest, &["out", "preset", "exec", "patience"])?;
    let out = args.option("out").ok_or("run needs --out <directory> for the bundle")?;
    let preset = args.option("preset");
    let exec = args.option("exec");
    let patience = patience_of(args.option("patience"))?;
    let [scene]: [String; 1] = args.finish(1)?.try_into().map_err(|_| "run takes one scene")?;
    let started = std::time::Instant::now();
    let mut session = Session::new();
    session.engine.set_patience(std::time::Duration::from_secs_f64(patience));
    session.load(&scene)?;
    if let Some(preset) = preset {
        session.engine.run_line(&format!("preset {preset}"))?;
    }
    if let Some(script) = exec {
        session.engine.run_script(&script)?;
    }
    session
        .engine
        .apply(cav_engine::Command::Export { what: cav_engine::ExportKind::Bundle, to: out.clone() })?;
    let (_, signal) = session.engine.signal();
    let signal = signal.ok_or("the bundle was written from no signal")?;
    println!(
        "{out}: scene `{}`, {} tower(s), {} shot(s) in all, quadrature {}, {:.2} s",
        session.engine.spec().name,
        signal.towers,
        signal.clock.len(),
        cav_engine::estimate_text(signal.estimate, signal.settled, signal.truncated),
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

/// A patience as `--patience` gives it: seconds, from 0 to a day; ten minutes if not given.
fn patience_of(given: Option<String>) -> Result<f64, String> {
    match given {
        None => Ok(600.0),
        Some(text) => text
            .parse::<f64>()
            .ok()
            .filter(|seconds| seconds.is_finite() && *seconds >= 0.0 && *seconds <= 86_400.0)
            .ok_or_else(|| format!("`{text}` is not a patience: seconds, from 0 to 86400")),
    }
}

/// `cav figure`: load, any edits asked for, and one export of a figure: the command the
/// console's `figure` line issues.
fn figure(rest: &[String]) -> Result<(), String> {
    use cav_engine::{ChannelName, Figure, FigureFormat, FigureKind};
    let options = ["out", "preset", "tower", "channel", "span", "size", "dpi", "section", "exec", "patience"];
    let mut args = Args::parse_with_flags(rest, &options, &["no-title"])?;
    let titled = !args.flag("no-title");
    let out = args.option("out").ok_or("figure needs --out <file.svg|.pdf|.png>")?;
    let format = FigureFormat::of_file(&out)
        .ok_or_else(|| format!("`{out}` does not end in .svg, .pdf or .png, so it names no format"))?;
    let preset = args.option("preset");
    let tower = args
        .option("tower")
        .map(|text| text.parse::<usize>().map_err(|_| format!("`{text}` is not a tower number")))
        .transpose()?;
    let channel = args
        .option("channel")
        .map(|text| {
            ChannelName::parse(&text)
                .ok_or_else(|| format!("`{text}` is not a channel: gradiometer, lower or upper"))
        })
        .transpose()?;
    let pair = |text: &str, between: &[char], what: &str| -> Result<[f64; 2], String> {
        text.split_once(between)
            .and_then(|(a, b)| Some([a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?]))
            .ok_or_else(|| format!("`{text}` is not {what}"))
    };
    let span =
        args.option("span").map(|text| pair(&text, &[','], "a span; write it as 0,200")).transpose()?;
    let size_mm = args
        .option("size")
        .map(|text| pair(&text, &['x', '×'], "a size; write it as 85x60 (millimetres)"))
        .transpose()?;
    let dpi = args
        .option("dpi")
        .map(|text| text.parse::<f64>().map_err(|_| format!("`{text}` is not a resolution: pixels per inch")))
        .transpose()?;
    let section = args
        .option("section")
        .map(|text| {
            cav_engine::PaperView::parse(&text)
                .filter(|view| *view != cav_engine::PaperView::Plan)
                .ok_or_else(|| format!("`{text}` is not a section; write it as x,12 or y,-40 (metres)"))
        })
        .transpose()?;
    let exec = args.option("exec");
    let patience = patience_of(args.option("patience"))?;
    let [scene, what]: [String; 2] =
        args.finish(2)?.try_into().map_err(|_| "figure takes a scene and what to draw")?;
    let what = FigureKind::parse(&what).ok_or_else(|| {
        format!(
            "`{what}` is not something a figure shows: signal, periodogram, cadence, flight, view or paper"
        )
    })?;
    let started = std::time::Instant::now();
    let mut session = Session::new();
    session.engine.set_patience(std::time::Duration::from_secs_f64(patience));
    session.load(&scene)?;
    if let Some(script) = exec {
        session.engine.run_script(&script)?;
    }
    let mut figure = Figure::new(what, format);
    figure.preset = preset.unwrap_or(figure.preset);
    // A periodogram and a window are one tower's: the first unless another is named.
    figure.tower = tower.or(matches!(what, FigureKind::Periodogram | FigureKind::Cadence).then_some(1));
    figure.channel = channel.unwrap_or_default();
    (figure.span, figure.size_mm, figure.dpi) = (span, size_mm, dpi);
    figure.title = titled;
    figure.pane = if what == FigureKind::View { session.engine.active_pane() } else { 0 };
    if what == FigureKind::Paper {
        figure.paper = Some(section.unwrap_or_default());
    } else if section.is_some() {
        return Err("--section is for a paper drawing".into());
    }
    let preset = figure.preset.clone();
    session.engine.apply(cav_engine::Command::Export {
        what: cav_engine::ExportKind::Figure(Box::new(figure)),
        to: out.clone(),
    })?;
    let path = std::path::Path::new(&out);
    let beside: Vec<String> = ["csv", "npy", "json"]
        .iter()
        .map(|extension| path.with_extension(extension))
        .filter(|file| file.exists())
        .map(|file| file.display().to_string())
        .collect();
    println!(
        "{out}: {} of scene `{}`, preset {preset}, {:.2} s\nbeside it: {}",
        what.name(),
        session.engine.spec().name,
        started.elapsed().as_secs_f64(),
        beside.join(", ")
    );
    Ok(())
}

/// `cav replay`: a journal run again on a fresh engine. Exports go inside `--out`
/// (`cav_engine::Exports::Into` has the rule); a journal is never allowed to write where it says.
fn replay(rest: &[String]) -> Result<(), String> {
    let mut args = Args::parse(rest, &["out", "patience"])?;
    let out = args.option("out");
    let patience = match args.option("patience") {
        None => cav_engine::REPLAY_PATIENCE,
        Some(text) => text
            .parse::<f64>()
            .ok()
            .filter(|seconds| (0.0..=86400.0).contains(seconds))
            .map(std::time::Duration::from_secs_f64)
            .ok_or_else(|| format!("`{text}` is not a patience: seconds, from 0 to 86400"))?,
    };
    let [file]: [String; 1] = args.finish(1)?.try_into().map_err(|_| "replay takes one journal file")?;
    let journal = cav_engine::journal::read(&file)?;
    // The journal is applied to an engine that can draw, so that the figures a session exported
    // are written again too (`Engine::replay_into` makes an engine of its own, which cannot).
    let mut engine = cav_engine::Engine::new();
    cav_plot::figure::install(&mut engine);
    match &out {
        Some(directory) => {
            std::fs::create_dir_all(directory).map_err(|e| format!("{directory}: {e}"))?;
            engine.set_exports(cav_engine::Exports::Into(std::path::PathBuf::from(directory)));
            engine.set_patience(patience);
        }
        None => engine.set_exports(cav_engine::Exports::Skipped),
    }
    for (n, command) in journal.iter().enumerate() {
        engine.apply(command.clone()).map_err(|why| format!("replay: command {n}: {why}"))?;
    }
    // Nothing is waited for here. Each export waited for the signal it was written from, and
    // the state's digest is of the scene, the playhead, the views and the history, none of
    // which is the signal: a replay that writes nothing computes no series to say what it
    // says. Where the signal stands as the replay ends is said, since it is what was skipped.
    let exports =
        journal.iter().filter(|command| matches!(command, cav_engine::Command::Export { .. })).count();
    let signal = match engine.signal().0 {
        cav_engine::SignalState::Stale => "signal stale, not waited for: nothing written needed it",
        cav_engine::SignalState::Refined => "signal refined",
        cav_engine::SignalState::None => "signal none",
    };
    println!(
        "replayed {} command(s) · scene `{}` · state {} · {exports} export(s) {} · {signal}",
        journal.len(),
        engine.spec().name,
        engine.digest(),
        out.map_or_else(
            || "not written (no --out)".to_owned(),
            |directory| format!("written into {directory}")
        )
    );
    Ok(())
}

/// `cav set`: load, one `Set` through the engine (the same command a panel or an agent issues,
/// with the same checks), save.
fn set(rest: &[String]) -> Result<(), String> {
    let mut args = Args::parse(rest, &["out"])?;
    let out = args.option("out");
    let [scene, path, value]: [String; 3] =
        args.finish(3)?.try_into().map_err(|_| "set takes a scene, a path and a value")?;
    let is_library = cav_sim::LIBRARY.iter().any(|known| known.name == scene);
    let Some(target) = out.or_else(|| (!is_library).then(|| scene.clone())) else {
        return Err(format!(
            "`{scene}` is a library scene, built into the binary: say where the edited scene goes with --out <file.ron>"
        ));
    };
    let mut session = Session::new();
    session.load(&scene)?;
    // The value as the console takes it: JSON, or a bare word for text.
    let value = cav_engine::value_of(&value)?;
    let command = cav_engine::Command::set(&path, value);
    let described = cav_engine::describe(&command, 0);
    session.engine.apply(command)?;
    session.engine.spec().save(&target).map_err(|e| e.0)?;
    println!("{described}\nsaved {target}");
    Ok(())
}

/// `cav render`: every option becomes a console command, so a render is a short engine session.
/// `cav bench flythrough`: see `bench.rs`.
fn bench(rest: &[String]) -> Result<(), String> {
    let mut args = Args::parse(rest, &["scene", "frames", "size", "cache", "out", "speed", "max-frame-ms"])?;
    let scene = args.option("scene").unwrap_or_else(|| "stress-1e10".to_owned());
    let frames = args
        .option("frames")
        .map_or(Ok(600), |f| f.parse::<usize>().map_err(|_| format!("--frames {f} is not a count")))?;
    if !(1..=100_000).contains(&frames) {
        return Err(format!("--frames {frames}: from 1 to 100000"));
    }
    let size = args.option("size").map_or(Ok([1280, 720]), |s| parse_size(&s))?;
    let cache = args.option("cache").map(std::path::PathBuf::from);
    let speed = args
        .option("speed")
        .map_or(Ok(15.0), |v| v.parse::<f64>().map_err(|_| format!("--speed {v} is not a number")))?;
    if !(speed > 0.0 && speed <= 1000.0) {
        return Err(format!("--speed {speed}: metres a second, more than 0 and at most 1000"));
    }
    let max_frame_ms = args
        .option("max-frame-ms")
        .map_or(Ok(250.0), |v| v.parse::<f64>().map_err(|_| format!("--max-frame-ms {v} is not a number")))?;
    let out = args.option("out");
    let [what] = args.finish(1)?.try_into().map_err(|_| "bench takes what to run: flythrough")?;
    if what != "flythrough" {
        return Err(format!("`{what}` is not a bench; there is `flythrough`"));
    }
    let trace = bench::flythrough(&scene, frames, size, speed, cache, max_frame_ms)?;
    match out {
        Some(path) => {
            std::fs::write(&path, &trace).map_err(|e| format!("writing {path}: {e}"))?;
            print!(
                "{}",
                trace
                    .lines()
                    .filter(|l| l.starts_with('#'))
                    .map(|l| format!(
                        "{l}
"
                    ))
                    .collect::<String>()
            );
        }
        None => print!("{trace}"),
    }
    Ok(())
}

fn render(rest: &[String]) -> Result<(), String> {
    let mut args = Args::parse(rest, &["out", "view", "proj", "style", "size", "exec"])?;
    let out = args.option("out").ok_or("render needs --out <file.png>")?;
    let size = args.option("size").map_or(Ok([1280, 720]), |s| parse_size(&s))?;
    let mut script = Vec::new();
    for (option, command) in [("view", "view"), ("proj", "proj"), ("style", "style")] {
        if let Some(value) = args.option(option) {
            script.push(format!("{command} {value}"));
        }
    }
    if let Some(extra) = args.option("exec") {
        script.push(extra);
    }
    let [scene] = args.finish(1)?.try_into().map_err(|_| "render takes one scene")?;
    let started = std::time::Instant::now();
    let mut session = Session::new();
    session.load(&scene)?;
    session.engine.run_script(&script.join("; "))?;
    let pane = session.engine.active_pane();
    let png = session.render_png(pane, size)?;
    std::fs::write(&out, &png).map_err(|e| format!("writing {out}: {e}"))?;
    for note in cav_render::decimation(session.engine.spec()) {
        println!("note: {note} (the scene is too large to mark every metre)");
    }
    // What the pane's view of the voxel bodies left out to stay within its budgets, and why.
    if let Some(voxels) = session.engine.frame(pane)?.voxels {
        for note in &voxels.notes {
            println!("note: {note}");
        }
    }
    println!(
        "{out}: {} × {} px, {} bytes, scene `{}`, {:.2} s",
        size[0],
        size[1],
        png.len(),
        session.engine.spec().name,
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

/// `1280x720` → `[1280, 720]`; each side from 1 to 8192.
fn parse_size(text: &str) -> Result<[u32; 2], String> {
    let wrong = || format!("`{text}` is not a size; write it as 1280x720 (each side 1 to 8192)");
    let (w, h) = text.split_once(['x', '×']).ok_or_else(wrong)?;
    let side = |s: &str| s.parse::<u32>().ok().filter(|n| (1..=8192).contains(n)).ok_or_else(wrong);
    Ok([side(w)?, side(h)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_parse_and_bad_ones_are_refused() {
        assert_eq!(parse_size("1280x720"), Ok([1280, 720]));
        assert_eq!(parse_size("1×8192"), Ok([1, 8192]));
        for bad in ["1280", "0x720", "1280x", "axb", "1280x720x3", "9000x10", "-5x5"] {
            let why = parse_size(bad).unwrap_err();
            assert!(why.contains("1280x720"), "`{bad}`: {why}");
        }
    }
}
