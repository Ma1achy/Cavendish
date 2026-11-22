//! `cav`: Cavendish without a window. Every subcommand drives the same engine API as the app.

mod args;
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
  replay <journal.json> [--out <dir>]
                                 run a saved session again (`journal save <file>` writes one); what it
                                 exported is written into --out under each file's own name, or not at all
  mcp                            serve the engine to an agent: an MCP server on stdin and stdout
  licences                       the licences of what is built into the binaries (the monospace font)
  help                           this text

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
        "mcp" => {
            Args::parse(rest, &[])?.finish(0)?;
            mcp::serve()
        }
        "licences" | "licenses" => {
            Args::parse(rest, &[])?.finish(0)?;
            println!(
                "{} is built into cav and cav-app, unmodified, under the Ubuntu Font Licence 1.0:\n",
                cav_app::theme::MONO_FONT_NAME
            );
            println!("{}", cav_app::theme::MONO_FONT_LICENCE);
            Ok(())
        }
        other => Err(format!("`{other}` is not a command\n\n{USAGE}")),
    }
}

/// `cav run`: load, any edits asked for, wait for the signal, write the bundle. Each step is the
/// console command a person would type, so the run is a short session like any other.
fn run_scene(rest: &[String]) -> Result<(), String> {
    let mut args = Args::parse(rest, &["out", "preset", "exec", "patience"])?;
    let out = args.option("out").ok_or("run needs --out <directory> for the bundle")?;
    let preset = args.option("preset");
    let exec = args.option("exec");
    let patience = match args.option("patience") {
        None => 600.0,
        Some(text) => text
            .parse::<f64>()
            .ok()
            .filter(|seconds| seconds.is_finite() && *seconds >= 0.0 && *seconds <= 86_400.0)
            .ok_or_else(|| format!("`{text}` is not a patience: seconds, from 0 to 86400"))?,
    };
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
        "{out}: scene `{}`, {} tower(s) × {} shot(s), quadrature {}, {:.2} s",
        session.engine.spec().name,
        signal.towers,
        signal.times.len(),
        cav_engine::estimate_text(signal.estimate, signal.settled),
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

/// `cav replay`: a journal run again on a fresh engine. Exports go into `--out` by file name; a
/// journal is never allowed to write where it says.
fn replay(rest: &[String]) -> Result<(), String> {
    let mut args = Args::parse(rest, &["out"])?;
    let out = args.option("out");
    let [file]: [String; 1] = args.finish(1)?.try_into().map_err(|_| "replay takes one journal file")?;
    let journal = cav_engine::journal::read(&file)?;
    let mut engine = match &out {
        Some(directory) => {
            std::fs::create_dir_all(directory).map_err(|e| format!("{directory}: {e}"))?;
            cav_engine::Engine::replay_into(&journal, std::path::Path::new(directory))?
        }
        None => cav_engine::Engine::replay(&journal)?,
    };
    engine.settle(std::time::Duration::from_secs(600));
    let exports =
        journal.iter().filter(|command| matches!(command, cav_engine::Command::Export { .. })).count();
    println!(
        "replayed {} command(s) · scene `{}` · state {} · {exports} export(s) {}",
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
    let value = serde_json::from_str(&value).unwrap_or(serde_json::Value::String(value));
    let command = cav_engine::Command::set(&path, value);
    let described = cav_engine::describe(&command, 0);
    session.engine.apply(command)?;
    session.engine.spec().save(&target).map_err(|e| e.0)?;
    println!("{described}\nsaved {target}");
    Ok(())
}

/// `cav render`: every option becomes a console command, so a render is a short engine session.
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
