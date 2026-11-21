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
  schema                         print the JSON Schema of the scene spec
  exec \"<commands>\"              run console commands (separated by `;`) and print what each answers;
                                 `cav exec help` lists them
  mcp                            serve the engine to an agent: an MCP server on stdin and stdout
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
            Args::parse(rest, &[])?.finish(0)?;
            println!("{}", Session::new().engine.run_line("schema")?);
            Ok(())
        }
        "exec" => {
            let args = Args::parse(rest, &[])?;
            let [script]: [String; 1] = args.finish(1)?.try_into().map_err(|_| "exec takes one argument")?;
            let mut session = Session::new();
            // Print what ran before reporting a failure, so the output shows how far the script got.
            let mut failure = None;
            for line in script.split([';', '\n']).map(str::trim).filter(|l| !l.is_empty()) {
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
        other => Err(format!("`{other}` is not a command\n\n{USAGE}")),
    }
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
    session.engine.run_line(&format!("load {scene}"))?;
    session.engine.run_script(&script.join("; "))?;
    let pane = session.engine.active_pane();
    let png = session.render_png(pane, size)?;
    std::fs::write(&out, &png).map_err(|e| format!("writing {out}: {e}"))?;
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
