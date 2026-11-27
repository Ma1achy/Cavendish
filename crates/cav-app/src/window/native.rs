//! The native application's entry (the `cav-app` binary, `src/main.rs`): the Cavendish window,
//! or (with `--screenshot`) the same application drawn to a PNG with no window at all. One side
//! of the `window` seam; a page has no window and no command line (`window/web.rs`).

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use crate::app::App;
use crate::headless::Headless;
use crate::{GpuPanes, Shell, theme};

const USAGE: &str = "\
cav-app [options]

  (no options)                 open the window
  --exec \"<commands>\"          run console commands first (separated by `;`), e.g. \"load void; panes 4\"
  --keep-going                 with --exec: carry on past a refused command instead of stopping
  --screenshot <file.png>      draw the application to a PNG with no window, then exit
  --size 1440x900              the size of the screenshot, or of the window
  --timings                    with --screenshot: also print frame times
  --measure                    print the workspace's budgets (docs/slices/S20.md §6) measured with
                               no window and no GPU, then exit
  --minimised                  open the window minimised
  --help                       this text
";

struct Options {
    exec: Option<String>,
    screenshot: Option<String>,
    size: [u32; 2],
    timings: bool,
    measure: bool,
    minimised: bool,
    keep_going: bool,
}

fn options() -> Result<Option<Options>, String> {
    let mut options = Options {
        exec: None,
        screenshot: None,
        size: [1440, 900],
        timings: false,
        measure: false,
        minimised: false,
        keep_going: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("`{name}` needs a value"));
        match arg.as_str() {
            "--help" | "-h" => return Ok(None),
            "--exec" => options.exec = Some(value("--exec")?),
            "--screenshot" => options.screenshot = Some(value("--screenshot")?),
            "--size" => {
                let text = value("--size")?;
                let parsed = text.split_once('x').and_then(|(w, h)| Some([w.parse().ok()?, h.parse().ok()?]));
                options.size = parsed
                    .filter(|s: &[u32; 2]| s.iter().all(|side| (64..=8192).contains(side)))
                    .ok_or_else(|| {
                        format!("`{text}` is not a size; write it as 1440x900 (each side 64 to 8192)")
                    })?;
            }
            "--timings" => options.timings = true,
            "--measure" => options.measure = true,
            "--minimised" => options.minimised = true,
            "--keep-going" => options.keep_going = true,
            other => return Err(format!("`{other}` is not an option\n\n{USAGE}")),
        }
    }
    Ok(Some(options))
}

/// Runs the application as its command line asks (`--help` for the options).
pub fn main() -> ExitCode {
    let outcome = match options() {
        Ok(None) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Ok(Some(options)) => run(&options),
        Err(why) => Err(why),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("cav-app: {why}");
            ExitCode::FAILURE
        }
    }
}

fn run(options: &Options) -> Result<(), String> {
    let mut shell = Shell::new();
    if let Some(script) = &options.exec {
        // The engine's own splitting: quotes keep a `;` together, a `#` line is a comment.
        for line in cav_engine::console::statements(script) {
            match shell.run(line) {
                Ok(_) => {}
                // The refusal is in the log of the engine either way; here it also reaches the terminal.
                Err(why) if options.keep_going => eprintln!("cav-app: --exec: `{line}` was refused: {why}"),
                Err(why) => return Err(format!("--exec: `{line}`: {why}")),
            }
        }
    }
    if options.measure {
        return measure(options.size);
    }
    match &options.screenshot {
        Some(path) => screenshot(&mut shell, options, path),
        None => window(shell, options),
    }
}

/// The workspace's budgets (`docs/slices/S20.md` §6), measured with no window and no GPU and
/// printed: a frame of the window's own drawing with six viewports, a walker's step against
/// `town`, and the cutting of a path of 10⁴ periods. Recorded, not gated.
fn measure(size: [u32; 2]) -> Result<(), String> {
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    let median = |mut times: Vec<f64>| {
        times.sort_by(f64::total_cmp);
        times[times.len() / 2]
    };
    // A frame with six viewports of mixed types and no GPU surface: the layout, the headers,
    // the panels and the plots, all but the scene's picture.
    for scene in ["point", "town"] {
        let mut shell = Shell::new();
        for line in [
            &format!("load {scene}"),
            "mosaic \"ABC;DEF\"",
            "show signal @2",
            "show periodogram @3",
            "show flight @5",
            "show cadence @6",
        ] {
            shell.run(line)?;
        }
        if !shell.engine.settle(std::time::Duration::from_secs(SETTLE_S)) {
            return Err(format!("{scene}: the signal was not whole after {SETTLE_S} s"));
        }
        // (And its world resident: a frame that is still streaming also makes chunks, up to
        // the engine's 6 ms a poll, which is the streaming's cost and not the window's drawing.)
        shell.engine.settle_world(std::time::Duration::from_secs(60));
        let ctx = crate::headless::context();
        let mut clock = 0.0;
        let mut frame = |shell: &mut Shell| {
            let started = Instant::now();
            crate::headless::run_frames(
                &ctx,
                shell,
                &mut crate::NoSurface,
                size,
                &mut clock,
                vec![Vec::new()],
            );
            started.elapsed().as_secs_f64() * 1e3
        };
        for _ in 0..10 {
            frame(&mut shell);
        }
        let times: Vec<f64> = (0..120).map(|_| frame(&mut shell)).collect();
        println!(
            "a frame of the window's own drawing, six viewports (2 scene, 4 plots), no GPU surface, `{scene}`, {} × {}: median {:.2} ms over {} frames ({profile}; budget 4 ms)",
            size[0],
            size[1],
            median(times.clone()),
            times.len()
        );
    }
    // A walker's step against `town`: over the ground to the first building's wall, then along
    // it; the scene's occupancy is made anew each step, as a frame makes it.
    {
        use crate::walk::{Env, Walker, Wish};
        let mut shell = Shell::new();
        shell.run("load town")?;
        let settings = shell.camera.walk.clone();
        let env = Env { gravity: shell.engine.spec().array.instrument.gravity, easing: shell.camera.easing };
        let ground = shell.ground();
        let wall = shell.engine.spec().world.as_ref().and_then(|world| {
            world.pieces.iter().find_map(|piece| match &piece.kind {
                cav_sim::world::PieceKind::Box { centre, size } => {
                    Some([centre[0] - 0.5 * size[0] - 20.0, centre[1], ground + 2.0])
                }
                cav_sim::world::PieceKind::Terrain { .. } => None,
            })
        });
        let eye = wall.ok_or("`town` has no box to walk at")?;
        let mut walker = Walker { flying: false, ..Walker::at_eye(eye, &settings) };
        let wish = Wish { along: [settings.speed, 0.2], ..Wish::default() };
        let steps = 20_000_u32;
        let started = Instant::now();
        for _ in 0..steps {
            let world = crate::occupancy::SceneOccupancy::new(&shell.engine, ground);
            walker.step(1.0 / 60.0, &wish, &world, &settings, env);
        }
        println!(
            "a walker's step against `town` (the occupancy made each step): {:.2} µs each over {steps} ({profile}; budget 50 µs)",
            started.elapsed().as_secs_f64() / f64::from(steps) * 1e6
        );
    }
    // The path of a body that goes round 10⁴ times in the timeline: a circle (one period is
    // drawn) and a motion that never repeats (it meets its cap of pieces and is flagged).
    {
        use cav_render::path;
        use cav_sim::Motion;
        let ring = Motion::Circular {
            centre: [0.0; 3],
            u: [2.0, 0.0, 0.0],
            v: [0.0, 2.0, 0.0],
            frequency: 1000.0,
            phase: 0.0,
        };
        let buzz = Motion::Sphere {
            centre: [0.0; 3],
            radius: 1.5,
            tilt: 60.0,
            tilt_frequency: 1000.0,
            spin_frequency: 0.37,
        };
        for (name, motion) in [("a 1000 Hz circle over 10 s", ring), ("a forced sphere at its cap", buzz)] {
            let body = cav_sim::BodySpec::moving("b", 1.0, motion);
            let build = || path::path(&body, 9.81, 10.0, path::PATH_PIECES);
            let drawn = build();
            let times: Vec<f64> = (0..201)
                .map(|_| {
                    let started = Instant::now();
                    std::hint::black_box(build());
                    started.elapsed().as_secs_f64() * 1e6
                })
                .collect();
            println!(
                "the path of 10^4 periods, {name}: {} pieces{} in a median {:.1} µs over {} builds ({profile}; budget 2000 µs)",
                drawn.pieces(),
                if drawn.capped { " (capped, flagged decimated)" } else { "" },
                median(times.clone()),
                times.len()
            );
        }
    }
    Ok(())
}

/// How long a screenshot waits for the scene's numbers, seconds: the scaffold's 48 members take
/// about 100 s on the build PC in a debug build, and 120 s was missed under load.
const SETTLE_S: u64 = 600;

fn screenshot(shell: &mut Shell, options: &Options, path: &str) -> Result<(), String> {
    let started = Instant::now();
    let mut headless = Headless::new()?;
    shell.adapter = Some(headless.adapter());
    // A picture of the application is a picture of finished numbers: wait for the signal, which the
    // window itself never does.
    // A picture of numbers still being made is not taken: it would differ from run to run (S3's
    // review: the demo's scaffold was drawn with "signal · stale").
    if !shell.engine.settle(std::time::Duration::from_secs(SETTLE_S)) {
        return Err(format!(
            "{path}: the signal was not whole after {SETTLE_S} s, so no picture was taken; shorten the scene"
        ));
    }
    // And the world's chunks and bricks the panes and towers want, made (natively a minute at most).
    if !shell.engine.settle_world(std::time::Duration::from_secs(60)) {
        return Err(format!("{path}: the world was not resident after 60 s, so no picture was taken"));
    }
    let mut png = headless.screenshot(shell, options.size)?;
    // The frames themselves may have moved a camera (a `walk` session under way settles onto
    // the ground) and so changed what the world is wanted for: the picture is taken again until
    // nothing is waiting to be made, so that it is of a whole world (TASK-S20-14).
    let mut retaken = 0;
    while shell.engine.streaming().pending() > 0 && retaken < 8 {
        if !shell.engine.settle_world(std::time::Duration::from_secs(60)) {
            return Err(format!("{path}: the world was not resident after 60 s, so no picture was taken"));
        }
        png = headless.screenshot(shell, options.size)?;
        retaken += 1;
    }
    if retaken > 0 {
        println!("{path}: taken again {retaken} time(s): the world was still streaming in");
    }
    std::fs::write(path, &png).map_err(|e| format!("writing {path}: {e}"))?;
    println!(
        "{path}: {} × {} px, {} bytes, {:.2} s",
        options.size[0],
        options.size[1],
        png.len(),
        started.elapsed().as_secs_f64()
    );
    if options.timings {
        timings(shell, &mut headless, options.size)?;
    }
    Ok(())
}

/// Frame times with no window, each including the read-back that waits for the GPU:
/// an unchanged frame (panes reused), and a frame in which the camera moved (panes redrawn).
fn timings(shell: &mut Shell, headless: &mut Headless, size: [u32; 2]) -> Result<(), String> {
    const FRAMES: u32 = 60;
    let median = |mut times: Vec<f64>| {
        times.sort_by(f64::total_cmp);
        times[times.len() / 2]
    };
    let mut still = Vec::new();
    for _ in 0..FRAMES {
        let started = Instant::now();
        headless.render(shell, size, vec![Vec::new()])?;
        still.push(started.elapsed().as_secs_f64() * 1e3);
    }
    // The editor's budgets (docs/slices/S18.md §6): a key routed over the tree the frame
    // registered, the palette's filter over every entry, a frame with the palette open.
    let tree = shell.focus_tree().clone();
    let shortcuts = crate::actions::table().shortcuts();
    let mut routes = Vec::new();
    for n in 0..2000 {
        let key = if n % 2 == 0 { egui::Key::Tab } else { egui::Key::ArrowDown };
        let started = Instant::now();
        let routed = shell.keyboard.route(&tree, &shortcuts, crate::keyboard::Press::plain(key));
        routes.push(started.elapsed().as_secs_f64() * 1e6);
        std::hint::black_box(routed);
    }
    shell.keyboard = crate::keyboard::Keyboard::default();
    let entries = crate::palette::entries(crate::actions::table(), shell);
    let mut filters = Vec::new();
    for query in ["sph", "add", "tower 3", "x", "zzz"].iter().cycle().take(200) {
        let started = Instant::now();
        std::hint::black_box(crate::palette::filter(&entries, query, usize::MAX));
        filters.push(started.elapsed().as_secs_f64() * 1e3);
    }
    shell.run("palette")?;
    let mut with_palette = Vec::new();
    for _ in 0..FRAMES {
        let started = Instant::now();
        headless.render(shell, size, vec![Vec::new()])?;
        with_palette.push(started.elapsed().as_secs_f64() * 1e3);
    }
    shell.palette.open = false;
    shell.keyboard = crate::keyboard::Keyboard::default();
    println!(
        "a key routed (Tab, ↓) over the frame's {} scopes and elements: median {:.2} µs over {} (budget 50 µs)",
        tree.len(),
        median(routes.clone()),
        routes.len()
    );
    println!(
        "the palette's filter over all {} entries: median {:.3} ms over {} queries (budget 1 ms)",
        entries.len(),
        median(filters.clone()),
        filters.len()
    );
    println!(
        "frame with the palette open: median {:.2} ms (without: {:.2} ms; budget within 2 ms of it)",
        median(with_palette),
        median(still.clone())
    );
    let draws_before = headless.pane_draws();
    let mut moving = Vec::new();
    for step in 0..FRAMES {
        shell.run(&format!("orbit {} 30", -45.0 + f64::from(step)))?;
        let started = Instant::now();
        headless.render(shell, size, vec![Vec::new()])?;
        moving.push(started.elapsed().as_secs_f64() * 1e3);
    }
    let redraws = headless.pane_draws() - draws_before;
    println!("adapter: {}", headless.adapter());
    println!(
        "frame, nothing changed (UI only, panes reused): median {:.2} ms over {FRAMES} frames",
        median(still)
    );
    println!(
        "frame, camera moving ({} pane redraw(s) per frame): median {:.2} ms over {FRAMES} frames",
        redraws / u64::from(FRAMES),
        median(moving)
    );
    println!(
        "each time includes copying the {} × {} picture back to the CPU, which a window does not do",
        size[0], size[1]
    );
    // The forward model, on the scene as loaded: the reading under the playhead, and the series.
    let spec = shell.engine.spec().clone();
    let (towers, bodies) = (spec.array.towers.len(), spec.bodies.len());
    let started = Instant::now();
    const READINGS: u32 = 200;
    for step in 0..READINGS {
        // The read itself, as the engine's worker runs it; asking the engine would time only the
        // handing over of a reading the worker has already made.
        let read = cav_sim::read::reading(&spec, f64::from(step) * 0.01, &|| false);
        std::hint::black_box(read);
    }
    println!(
        "reading, {towers} towers × {bodies} bodies, the audited read itself (the engine runs it off the caller's thread): {:.3} ms each over {READINGS}",
        started.elapsed().as_secs_f64() * 1e3 / f64::from(READINGS)
    );
    let started = Instant::now();
    shell.run("run")?;
    let whole = shell.engine.settle(std::time::Duration::from_secs(300));
    let shots = shell.engine.signal().1.map_or(0, |signal| signal.most());
    println!(
        "signal from nothing, {shots} shots × {towers} towers × {bodies} bodies, on the worker: {:.1} ms{}",
        started.elapsed().as_secs_f64() * 1e3,
        if whole { "" } else { " (NOT FINISHED in 300 s)" }
    );
    // A drag: a set of the first body's anchor, then the reading and a frame, as the window does it.
    if let Some(body) = spec.bodies.first() {
        let (anchor, field) = body.motion.anchor();
        let mut drag = Vec::new();
        for step in 0..FRAMES {
            let started = Instant::now();
            let moved = [anchor[0] + 0.01 * f64::from(step), anchor[1], anchor[2]];
            shell.run(&format!("set bodies[0].motion.{field} [{}, {}, {}]", moved[0], moved[1], moved[2]))?;
            shell.reading();
            headless.render(shell, size, vec![Vec::new()])?;
            drag.push(started.elapsed().as_secs_f64() * 1e3);
        }
        println!(
            "drag step (set, readings, one frame with its read-back): median {:.2} ms over {FRAMES} steps",
            median(drag)
        );
    }
    Ok(())
}

fn window(mut shell: Shell, options: &Options) -> Result<(), String> {
    // The device is ours, made by cav-gpu and handed to eframe, so the panes and the kernels share it.
    let gpu = cav_gpu::Gpu::new()?;
    shell.adapter = Some(gpu.adapter_report().to_string());
    let viewport = egui::ViewportBuilder::default()
        .with_title("Cavendish")
        .with_icon(
            eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icon-256.png"))
                .expect("the bundled window icon is a valid PNG"),
        )
        .with_inner_size([options.size[0] as f32, options.size[1] as f32])
        .with_min_inner_size([900.0, 600.0])
        // A minimised start must not take the focus from whatever the owner is doing.
        .with_active(!options.minimised);
    let mut native = eframe::NativeOptions { viewport, ..Default::default() };
    native.wgpu_options.wgpu_setup = egui_wgpu::WgpuSetup::Existing(egui_wgpu::WgpuSetupExisting {
        instance: gpu.instance.clone(),
        adapter: gpu.adapter.clone(),
        device: gpu.device.clone(),
        queue: gpu.queue.clone(),
    });
    let minimise = options.minimised;
    eframe::run_native(
        "Cavendish",
        native,
        Box::new(move |creation| {
            theme::apply(&creation.egui_ctx);
            let state =
                creation.wgpu_render_state.as_ref().ok_or("eframe did not start its wgpu renderer")?;
            let panes = GpuPanes::new(gpu, Arc::clone(&state.renderer));
            Ok(Box::new(App::new(shell, panes, minimise)))
        }),
    )
    .map_err(|e| format!("the window could not be opened: {e}"))
}
