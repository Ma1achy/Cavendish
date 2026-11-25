//! The application in a browser page: the platform seam of `cav-app` on `wasm32-unknown-unknown`
//! (`platform/native.rs` is the native side; `bin/cav-web.rs` calls [`start`]).
//!
//! The page (`crates/cav-app/web/index.html`) has a canvas `#cavendish` and a `#message` element,
//! and loads this module only when the browser has a WebGPU adapter. Here: eframe's web runner on
//! that canvas, the engine's device taken from eframe's (a page cannot make its own without
//! waiting), the address's `#exec=` commands, and the files the engine "writes" handed to the
//! browser as downloads. What a page cannot do at all (threads, files by path, folders, pixels
//! read back) is refused where it is asked for, by the crates that would do it
//! (`cav_engine::files`, `cav_engine::forward`, `cav_gpu`).
//!
//! A panic after start-up would abort the module (`panic = "abort"`) and leave the last frame on
//! the canvas, still-looking; the panic hook puts the message on the page instead.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;

use eframe::wasm_bindgen::JsCast;
use eframe::wasm_bindgen::prelude::Closure;
use eframe::web_sys;
use web_time::{Duration, Instant};

use crate::address::{download_name, exec_commands};
use crate::app::App;
use crate::{GpuPanes, Shell, theme};

/// The id of the canvas the page gives the application.
pub const CANVAS_ID: &str = "cavendish";

/// The id of the button that holds the downloads an address's commands made.
const HELD_ID: &str = "held-downloads";

/// How long an address's command that needs the whole signal (a figure of the signal or the
/// periodogram) waits for it before it is run anyway (and refused, with the reason, if the
/// signal is still not whole).
const EXEC_WAIT: Duration = Duration::from_secs(120);

/// The address's commands not yet run, and when the one at the front began to wait.
struct Exec {
    commands: VecDeque<String>,
    waiting_since: Option<Instant>,
}

thread_local! {
    static EXEC: RefCell<Exec> = const { RefCell::new(Exec { commands: VecDeque::new(), waiting_since: None }) };
    /// Files an address's commands made, held until the person clicks for them.
    static HELD: RefCell<Vec<cav_engine::files::Download>> = const { RefCell::new(Vec::new()) };
    /// Files dropped on the page whose bytes the browser has given, waiting for the next frame.
    static DROPPED: RefCell<Vec<(String, Option<Vec<u8>>)>> = const { RefCell::new(Vec::new()) };
}

/// The files dropped on the page whose bytes have been read, each as its name and its bytes. A
/// browser reads a dropped file only asynchronously: a file dropped this frame is asked for here
/// and handed over a frame or two later.
#[must_use]
pub fn dropped(ctx: &egui::Context) -> Vec<(String, Option<Vec<u8>>)> {
    for file in ctx.input(|i| i.raw.dropped_files.clone()) {
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let name = file.path().to_string_lossy().into_owned();
            // The size first, from the browser's handle (S19's review, minor 5): a file larger
            // than the engine keeps is refused before its bytes are read into the page.
            if let Some(size) = file.web_file().map(|f| f.size())
                && size > cav_engine::MAX_DROPPED_BYTES as f64
            {
                let why = format!(
                    "{name}: {} MiB is more than the {} MiB a dropped file may be",
                    (size / f64::from(1u32 << 20)).round(),
                    cav_engine::MAX_DROPPED_BYTES >> 20
                );
                log::warn!("{why}");
                note(&why);
                return;
            }
            match file.bytes_async().await {
                Ok(bytes) => DROPPED.with(|d| d.borrow_mut().push((name, Some(bytes)))),
                Err(why) => log::warn!("{name} could not be read: {why}"),
            }
            ctx.request_repaint();
        });
    }
    DROPPED.with(|d| std::mem::take(&mut *d.borrow_mut()))
}

/// Starts the application on the page's canvas. Returns at once; the application runs from the
/// browser's frames. If it cannot start, the reason replaces the canvas on the page.
pub fn start() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    if serve_if_worker() {
        return;
    }
    match Remote::start() {
        Ok(remote) => cav_engine::forward::wire::install(std::rc::Rc::new(remote)),
        // Without workers the jobs run on the page's thread, a source a frame: slow, not wrong.
        Err(why) => log::warn!("the forward stage's workers did not start ({why}); its jobs run on the page"),
    }
    wasm_bindgen_futures::spawn_local(async {
        if let Err(why) = run().await {
            log::error!("Cavendish could not start: {why}");
            say(&format!("Cavendish could not start in this browser: {why}"));
        }
    });
}

/// Puts a panic's message on the page (the canvas would otherwise freeze on its last frame), then
/// does what the hook before it did (eframe's: the browser's console).
fn install_panic_hook() {
    let before = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        say(&format!(
            "Cavendish stopped: {info}. Reload the page to start again; the browser's console has the details."
        ));
        before(info);
    }));
}

async fn run() -> Result<(), String> {
    let document = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
    let canvas = document
        .get_element_by_id(CANVAS_ID)
        .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        .ok_or_else(|| format!("the page has no canvas `#{CANVAS_ID}`"))?;
    let runner = eframe::WebRunner::new();
    install_panic_hook();
    runner
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|creation| {
                theme::apply(&creation.egui_ctx);
                let state =
                    creation.wgpu_render_state.as_ref().ok_or("eframe did not start its WebGPU renderer")?;
                let gpu = cav_gpu::Gpu::from_parts(
                    state.instance.clone(),
                    state.adapter.clone(),
                    state.device.clone(),
                    state.queue.clone(),
                );
                let mut shell = Shell::new();
                shell.adapter = Some(gpu.adapter_report().to_string());
                exec_from_address();
                let panes = GpuPanes::new(gpu, Arc::clone(&state.renderer));
                Ok(Box::new(App::new(shell, panes, false)))
            }),
        )
        .await
        .map_err(|e| format!("{e:?}"))?;
    Ok(())
}

/// The page's address may carry console commands, as the native `--exec` does:
/// `…/Cavendish/#exec=load comb; panes 2` (spaces may be written `%20`), within the caps of
/// [`crate::address`]. They are queued here and run from the first frame on ([`run_exec`]); a
/// refused one is in the console's log, and the rest still run.
fn exec_from_address() {
    let Some(hash) = web_sys::window().and_then(|w| w.location().hash().ok()) else { return };
    match exec_commands(&hash) {
        Ok(Some(commands)) => EXEC.with_borrow_mut(|exec| exec.commands = commands.into()),
        Ok(None) => {}
        Err(why) => log::warn!("#exec: {why}"),
    }
}

/// Runs the address's commands that are ready, in order, and says whether any ran. A figure of
/// the signal or the periodogram waits (at most [`EXEC_WAIT`]) for the signal to be whole, as a
/// person would before asking for one; everything after it waits behind it.
fn run_exec(shell: &mut Shell) -> bool {
    let mut ran = false;
    loop {
        let Some(line) = EXEC.with_borrow(|exec| exec.commands.front().cloned()) else { return ran };
        if needs_whole_signal(shell, &line) && shell.engine.signal().0 == cav_engine::SignalState::Stale {
            let since = EXEC.with_borrow_mut(|exec| *exec.waiting_since.get_or_insert_with(Instant::now));
            if since.elapsed() < EXEC_WAIT {
                return ran;
            }
        }
        EXEC.with_borrow_mut(|exec| {
            exec.commands.pop_front();
            exec.waiting_since = None;
        });
        let started = Instant::now();
        match shell.run(&line) {
            Ok(_) => log::info!("#exec: `{line}` in {:.0} ms", started.elapsed().as_secs_f64() * 1e3),
            Err(why) => log::warn!("#exec: `{line}` was refused: {why}"),
        }
        ran = true;
    }
}

/// Whether `line` is a figure that is written only from a whole signal.
fn needs_whole_signal(shell: &Shell, line: &str) -> bool {
    matches!(
        shell.engine.parse(line),
        Ok(cav_engine::Request::Do(cav_engine::Command::Export { what: cav_engine::ExportKind::Figure(figure), .. }))
            if figure.needs_signal()
    )
}

/// Puts `text` on the page in place of the canvas.
fn say(text: &str) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    if let Some(canvas) = document.get_element_by_id(CANVAS_ID) {
        canvas.remove();
    }
    if let Some(message) = document.get_element_by_id("message") {
        message.set_text_content(Some(text));
    } else if let Some(body) = document.body() {
        // The loading message has gone after the first frame: a new one.
        if let Ok(message) = document.create_element("div") {
            message.set_id("message");
            message.set_text_content(Some(text));
            let _ = body.append_child(&message);
        }
    }
}

/// Puts `text` on the page above the canvas, which keeps running: for a fault the application
/// survives (a forward worker that failed).
fn note(text: &str) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    let note = document.get_element_by_id("note").or_else(|| {
        let note = document.create_element("div").ok()?;
        note.set_id("note");
        let _ = note.set_attribute("role", "alert");
        let _ = document.body()?.prepend_with_node_1(&note);
        Some(note)
    });
    if let Some(note) = note {
        note.set_text_content(Some(text));
    }
}

/// What the page needs after each frame the application has drawn: the page's own message
/// ("Loading Cavendish…") goes after the first, the address's commands that are ready run, and
/// the files the frame "wrote" are downloaded, or held behind a button if a link's commands
/// made them.
pub fn after_frame(shell: &mut Shell, ctx: &egui::Context) {
    thread_local! {
        static DRAWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    if !DRAWN.replace(true)
        && let Some(message) =
            web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id("message"))
    {
        message.remove();
    }
    say_when_whole(shell);
    let from_address = run_exec(shell);
    // A command run here, after the frame was drawn, may have changed what the next one shows
    // (a load), and one left waiting must be looked at again: another frame, whatever egui
    // thought it needed.
    if from_address || EXEC.with_borrow(|exec| !exec.commands.is_empty()) {
        ctx.request_repaint();
    }
    let downloads = cav_engine::files::take_downloads();
    if downloads.is_empty() {
        return;
    }
    if from_address {
        HELD.with_borrow_mut(|held| held.extend(downloads));
        show_held();
    } else {
        for download in &downloads {
            offer(download);
        }
    }
}

/// Says in the browser's console how long a signal took to be whole, from the frame it went
/// stale to the frame it was whole: what a person waits, measured where they wait.
fn say_when_whole(shell: &Shell) {
    thread_local! {
        static STALE_SINCE: std::cell::Cell<Option<Instant>> = const { std::cell::Cell::new(None) };
    }
    let state = shell.engine.signal().0;
    match (state, STALE_SINCE.get()) {
        (cav_engine::SignalState::Stale, None) => STALE_SINCE.set(Some(Instant::now())),
        (cav_engine::SignalState::Stale, Some(_)) => {}
        (_, Some(since)) => {
            STALE_SINCE.set(None);
            if state == cav_engine::SignalState::Refined {
                log::info!(
                    "the signal of `{}` was whole {:.0} ms after it went stale",
                    shell.engine.spec().name,
                    since.elapsed().as_secs_f64() * 1e3
                );
            }
        }
        (_, None) => {}
    }
}

/// Shows (or updates) the button that downloads what a link's commands made: a page may not put
/// files in a visitor's downloads unless they click for them.
fn show_held() {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    let names: Vec<String> = HELD.with_borrow(|held| held.iter().map(|d| d.name.clone()).collect());
    let button = match document.get_element_by_id(HELD_ID) {
        Some(button) => button,
        None => {
            let Ok(button) = document.create_element("button") else { return };
            button.set_id(HELD_ID);
            let _ = button.set_attribute(
                "style",
                "position:absolute;top:36px;left:50%;transform:translateX(-50%);z-index:10;padding:6px 14px;\
                 font:14px system-ui,sans-serif;background:#2b3a55;color:#e8ecf4;border:1px solid #8ab4f8;\
                 border-radius:4px;cursor:pointer",
            );
            let clicked = Closure::<dyn FnMut()>::new(|| {
                for download in HELD.with_borrow_mut(std::mem::take) {
                    offer(&download);
                }
                if let Some(button) =
                    web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id(HELD_ID))
                {
                    button.remove();
                }
            });
            if let Some(element) = button.dyn_ref::<web_sys::HtmlElement>() {
                element.set_onclick(Some(clicked.as_ref().unchecked_ref()));
            }
            // The button lives as long as the page: so does its handler.
            clicked.forget();
            if let Some(body) = document.body() {
                let _ = body.append_child(&button);
            }
            button
        }
    };
    let files = if names.len() == 1 { "file" } else { "files" };
    button.set_text_content(Some(&format!(
        "Download {} {files} this link made: {}",
        names.len(),
        names.join(", ")
    )));
}

/// Hands one file to the browser as a download, under its [`download_name`]. A file of a kind a
/// page does not hand out, or one the browser will not take, is said in the browser's console;
/// it is not kept.
fn offer(download: &cav_engine::files::Download) {
    let Some(name) = download_name(&download.name) else {
        log::error!(
            "{}: not downloaded: a page hands out only .{} files",
            download.name,
            crate::address::DOWNLOAD_EXTENSIONS.join(", .")
        );
        return;
    };
    if let Err(why) = hand_over(&name, &download.bytes) {
        log::error!("{name}: the browser did not take the download: {why:?}");
    }
}

fn hand_over(name: &str, bytes: &[u8]) -> Result<(), eframe::wasm_bindgen::JsValue> {
    let document = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
    let bytes = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::of1(&bytes);
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/octet-stream");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    let link: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
    link.set_href(&url);
    link.set_download(name);
    link.click();
    // The browser reads the file after this returns: its address is let go a minute later, not
    // now (let go at once, a second or third file of one click can arrive as an error page).
    let later = Closure::once_into_js(move || {
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    web_sys::window()
        .ok_or("no window")?
        .set_timeout_with_callback_and_timeout_and_arguments_0(later.unchecked_ref(), 60_000)?;
    Ok(())
}

/// The forward stage's Web Workers (`web/worker.js`, each running this same module): the
/// [`cav_engine::forward::wire::Remote`] the page installs, so that a voxel body's seconds of
/// work never run on the page's thread. What happens when is [`crate::lane::Lane`]'s (its rules
/// and tests are there): a failed worker is said with its panic's message and started again a
/// bounded number of times, and the page runs waiting jobs itself, one a frame, while no worker
/// is ready (S17's second review, item 2; S4's review, I11). Here each of its actions becomes a
/// call on a `Worker`.
struct PageWorkers {
    lane: crate::lane::Lane,
    workers: Vec<Option<web_sys::Worker>>,
}

/// The page's clock, milliseconds.
fn now_ms() -> f64 {
    web_sys::window().and_then(|w| w.performance()).map_or(0.0, |p| p.now())
}

/// The handle the workers' message handlers and the engine both hold.
#[derive(Clone)]
struct Remote(std::rc::Rc<RefCell<PageWorkers>>);

impl Remote {
    /// Starts the workers, handing each the module the page compiled (`globalThis.cavendishWasm`,
    /// set by `index.html`) so that it is not fetched and compiled again.
    fn start() -> Result<Self, String> {
        let (lane, first) = crate::lane::Lane::new();
        let remote = Self(std::rc::Rc::new(RefCell::new(PageWorkers {
            lane,
            workers: vec![None; crate::lane::WORKERS],
        })));
        // The first workers must start, or there is no lane: the page then runs the jobs itself.
        for action in first {
            if let crate::lane::Action::Spawn(index) = action {
                let worker = remote.spawn(index)?;
                remote.0.borrow_mut().workers[index] = Some(worker);
            }
        }
        Ok(remote)
    }

    /// Does what the lane asks, in order. Never called with the pool borrowed.
    fn apply(&self, actions: Vec<crate::lane::Action>) {
        use crate::lane::Action;
        for action in actions {
            match action {
                Action::Spawn(index) => match self.spawn(index) {
                    Ok(worker) => self.0.borrow_mut().workers[index] = Some(worker),
                    Err(why) => {
                        log::error!("the forward stage's worker {index} could not be started again: {why}")
                    }
                },
                Action::Terminate(index) => {
                    if let Some(worker) = self.0.borrow_mut().workers[index].take() {
                        worker.terminate();
                    }
                }
                Action::Post { slot, ticket, request, assets } => {
                    let message = js_sys::Object::new();
                    // The held meshes this worker has not been sent yet go first, once each
                    // (S19's review, I4): the page's store has their bytes.
                    let held: Vec<(String, String, std::sync::Arc<[u8]>)> = assets
                        .into_iter()
                        .filter_map(|hash| {
                            cav_sim::assets::bytes(&hash).map(|(name, bytes)| (hash, name, bytes))
                        })
                        .collect();
                    let sent = if held.is_empty() {
                        request.clone()
                    } else {
                        cav_engine::forward::wire::with_assets(&request, &held)
                    };
                    let bytes = js_sys::Uint8Array::from(sent.as_slice());
                    let _ = js_sys::Reflect::set(&message, &"ticket".into(), &(ticket as f64).into());
                    let _ = js_sys::Reflect::set(&message, &"request".into(), &bytes);
                    let transfer = js_sys::Array::of1(&bytes.buffer());
                    let posted = self.0.borrow().workers[slot]
                        .as_ref()
                        .map(|w| w.post_message_with_transfer(&message, &transfer));
                    if !matches!(posted, Some(Ok(()))) {
                        log::error!("a job could not be handed to the forward stage's worker {slot}");
                        let more = self.0.borrow_mut().lane.not_posted(slot, ticket, request, now_ms());
                        self.apply(more);
                    }
                }
                Action::RunHere { ticket, request } => {
                    let answer = cav_engine::forward::wire::serve(&request);
                    self.0.borrow_mut().lane.ran_here(ticket, answer, now_ms());
                }
                Action::Say(text) => {
                    log::error!("{text}");
                    note(&text);
                }
            }
        }
    }

    /// A new worker for slot `index`, sent the module; it says `ready` when it runs.
    fn spawn(&self, index: usize) -> Result<web_sys::Worker, String> {
        let module = js_sys::Reflect::get(&js_sys::global(), &"cavendishWasm".into()).unwrap_or_default();
        let options = web_sys::WorkerOptions::new();
        options.set_type(web_sys::WorkerType::Module);
        let worker =
            web_sys::Worker::new_with_options("./worker.js", &options).map_err(|e| format!("{e:?}"))?;
        // Two closures a worker, kept for its life: bounded, as the lane starts a slot again at
        // most `MAX_RESPAWNS` times.
        let handle = self.clone();
        let on_message =
            Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
                handle.arrived(index, &event.data());
            });
        worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();
        let handle = self.clone();
        let on_error = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            let message = js_sys::Reflect::get(&event, &"message".into())
                .ok()
                .and_then(|m| m.as_string())
                .unwrap_or_else(|| "no message".to_owned());
            let actions = handle.0.borrow_mut().lane.failed(index, &message, now_ms());
            handle.apply(actions);
        });
        worker.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_error.forget();
        let start = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&start, &"module".into(), &module);
        worker.post_message(&start).map_err(|e| format!("{e:?}"))?;
        Ok(worker)
    }

    /// A worker's message: `{ready}` once it runs, `{panic}` just before it traps, and
    /// `{ticket, answer}` for each job.
    fn arrived(&self, index: usize, data: &eframe::wasm_bindgen::JsValue) {
        let field = |name: &str| js_sys::Reflect::get(data, &name.into()).unwrap_or_default();
        {
            let mut pool = self.0.borrow_mut();
            if field("ready").is_truthy() {
                pool.lane.ready(index);
            } else if let Some(message) = field("panic").as_string() {
                pool.lane.panicked(index, message);
            } else if let Some(ticket) = field("ticket").as_f64() {
                let answer = js_sys::Uint8Array::new(&field("answer")).to_vec();
                pool.lane.answered(index, ticket as u64, answer);
            }
        }
        self.pump();
    }

    /// Asks the lane what to do now, and does it.
    fn pump(&self) {
        let actions = self.0.borrow_mut().lane.pump(now_ms());
        self.apply(actions);
    }
}

impl cav_engine::forward::wire::Remote for Remote {
    fn send(&self, request: Vec<u8>) -> u64 {
        let ticket = self.0.borrow_mut().lane.send(request, now_ms());
        self.pump();
        ticket
    }

    fn take(&self, ticket: u64) -> Option<Vec<u8>> {
        // Asked every frame: workers started again, and the page's own run of a job, start here.
        self.pump();
        self.0.borrow_mut().lane.take(ticket)
    }
}

/// In a forward stage's Web Worker (`web/worker.js` started the module): answers each job with
/// [`cav_engine::forward::wire::serve`], after saying it is ready. True when this is such a
/// worker, which then starts nothing else.
fn serve_if_worker() -> bool {
    let Ok(scope) = js_sys::global().dyn_into::<web_sys::DedicatedWorkerGlobalScope>() else { return false };
    let replies = scope.clone();
    let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
        let data = event.data();
        let field = |name: &str| js_sys::Reflect::get(&data, &name.into()).unwrap_or_default();
        let request = js_sys::Uint8Array::new(&field("request")).to_vec();
        let answer = js_sys::Uint8Array::from(cav_engine::forward::wire::serve(&request).as_slice());
        let message = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&message, &"ticket".into(), &field("ticket"));
        let _ = js_sys::Reflect::set(&message, &"answer".into(), &answer);
        let transfer = js_sys::Array::of1(&answer.buffer());
        if let Err(why) = replies.post_message_with_transfer(&message, &transfer) {
            log::error!("the forward stage's worker could not answer: {why:?}");
        }
    });
    scope.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    on_message.forget();
    // A panic in a job traps the module (`panic = "abort"`), and the page would see only
    // "unreachable": the message is posted first, so that the page can say what went wrong.
    let panics = scope.clone();
    std::panic::set_hook(Box::new(move |info| {
        let message = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&message, &"panic".into(), &info.to_string().into());
        let _ = panics.post_message(&message);
        log::error!("the forward stage's worker panicked: {info}");
    }));
    let ready = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&ready, &"ready".into(), &true.into());
    let _ = scope.post_message(&ready);
    true
}
