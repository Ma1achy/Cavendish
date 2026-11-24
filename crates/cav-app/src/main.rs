//! `cav-app`: the Cavendish window, or (with `--screenshot`) the same application drawn to a PNG
//! with no window at all.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use cav_app::headless::Headless;
use cav_app::{GpuPanes, Shell, theme};

const USAGE: &str = "\
cav-app [options]

  (no options)                 open the window
  --exec \"<commands>\"          run console commands first (separated by `;`), e.g. \"load void; panes 4\"
  --keep-going                 with --exec: carry on past a refused command instead of stopping
  --screenshot <file.png>      draw the application to a PNG with no window, then exit
  --size 1440x900              the size of the screenshot, or of the window
  --timings                    with --screenshot: also print frame times
  --minimised                  open the window minimised
  --help                       this text
";

struct Options {
    exec: Option<String>,
    screenshot: Option<String>,
    size: [u32; 2],
    timings: bool,
    minimised: bool,
    keep_going: bool,
}

fn options() -> Result<Option<Options>, String> {
    let mut options = Options {
        exec: None,
        screenshot: None,
        size: [1440, 900],
        timings: false,
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
            "--minimised" => options.minimised = true,
            "--keep-going" => options.keep_going = true,
            other => return Err(format!("`{other}` is not an option\n\n{USAGE}")),
        }
    }
    Ok(Some(options))
}

fn main() -> ExitCode {
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
    match &options.screenshot {
        Some(path) => screenshot(&mut shell, options, path),
        None => window(shell, options),
    }
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
    let png = headless.screenshot(shell, options.size)?;
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

struct App {
    shell: Shell,
    panes: GpuPanes,
    last_frame: Instant,
    /// Minimise the window on its first frame (`--minimised`).
    minimise: bool,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if std::mem::take(&mut self.minimise) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        let now = Instant::now();
        self.shell.frame_ms = Some(now.duration_since(self.last_frame).as_secs_f32() * 1e3);
        self.last_frame = now;
        self.shell.ui(ui, &mut self.panes);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::BACKGROUND.to_normalized_gamma_f32()
    }
}

fn window(mut shell: Shell, options: &Options) -> Result<(), String> {
    // The device is ours, made by cav-gpu and handed to eframe, so the panes and the kernels share it.
    let gpu = cav_gpu::Gpu::new()?;
    shell.adapter = Some(gpu.adapter_report().to_string());
    let viewport = egui::ViewportBuilder::default()
        .with_title("Cavendish")
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
            Ok(Box::new(App { shell, panes, last_frame: Instant::now(), minimise }))
        }),
    )
    .map_err(|e| format!("the window could not be opened: {e}"))
}
