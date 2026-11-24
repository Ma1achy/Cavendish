//! The application in a browser page: the platform seam of `cav-app` (compiled for
//! `wasm32-unknown-unknown` only; `bin/cav-web.rs` calls [`start`]).
//!
//! The page (`crates/cav-app/web/index.html`) has a canvas `#cavendish` and a `#message` element,
//! and loads this module only when the browser has WebGPU. Here: eframe's web runner on that
//! canvas, the engine's device taken from eframe's (a page cannot make its own without waiting),
//! and the files the engine "writes" handed to the browser as downloads. What a page cannot do at
//! all (threads, files by path, folders, pixels read back) is refused where it is asked for, by
//! the crates that would do it (`cav_engine::files`, `cav_engine::forward`, `cav_gpu`).

use std::sync::Arc;

use eframe::wasm_bindgen::JsCast;
use eframe::web_sys;

use crate::app::App;
use crate::{GpuPanes, Shell, theme};

/// The id of the canvas the page gives the application.
pub const CANVAS_ID: &str = "cavendish";

/// Starts the application on the page's canvas. Returns at once; the application runs from the
/// browser's frames. If it cannot start, the reason replaces the canvas on the page.
pub fn start() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        if let Err(why) = run().await {
            log::error!("Cavendish could not start: {why}");
            say(&format!("Cavendish could not start in this browser: {why}"));
        }
    });
}

async fn run() -> Result<(), String> {
    let document = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
    let canvas = document
        .get_element_by_id(CANVAS_ID)
        .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        .ok_or_else(|| format!("the page has no canvas `#{CANVAS_ID}`"))?;
    let runner = eframe::WebRunner::new();
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
                exec_from_address(&mut shell);
                let panes = GpuPanes::new(gpu, Arc::clone(&state.renderer));
                Ok(Box::new(App::new(shell, panes, false)))
            }),
        )
        .await
        .map_err(|e| format!("{e:?}"))?;
    Ok(())
}

/// The page's address may carry console commands, as the native `--exec` does:
/// `…/Cavendish/#exec=load comb; panes 2` (spaces may be written `%20`). They are run before the
/// first frame; a refused one is in the console's log, and the rest still run.
fn exec_from_address(shell: &mut Shell) {
    let Some(hash) = web_sys::window().and_then(|w| w.location().hash().ok()) else { return };
    let Some(script) = hash.strip_prefix("#exec=") else { return };
    let script = js_sys::decode_uri_component(script).map_or_else(|_| script.to_owned(), String::from);
    for line in cav_engine::console::statements(&script) {
        let started = web_time::Instant::now();
        match shell.run(line) {
            Ok(_) => log::info!("#exec: `{line}` in {:.0} ms", started.elapsed().as_secs_f64() * 1e3),
            Err(why) => log::warn!("#exec: `{line}` was refused: {why}"),
        }
    }
}

/// Puts `text` on the page in place of the canvas.
fn say(text: &str) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    if let Some(canvas) = document.get_element_by_id(CANVAS_ID) {
        canvas.remove();
    }
    if let Some(message) = document.get_element_by_id("message") {
        message.set_text_content(Some(text));
    }
}

/// What the page needs after each frame the application has drawn: the page's own message
/// ("Loading Cavendish…") goes after the first, and the files the frame "wrote" are downloaded.
pub fn after_frame() {
    thread_local! {
        static DRAWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    if !DRAWN.replace(true)
        && let Some(message) =
            web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id("message"))
    {
        message.remove();
    }
    offer_downloads();
}

/// Hands every file the engine has "written" since the last frame to the browser as a download.
/// A file the browser will not take is said in the browser's console; it is not kept.
fn offer_downloads() {
    for download in cav_engine::files::take_downloads() {
        if let Err(why) = offer(&download) {
            log::error!("{}: the browser did not take the download: {why:?}", download.name);
        }
    }
}

fn offer(download: &cav_engine::files::Download) -> Result<(), eframe::wasm_bindgen::JsValue> {
    let document = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
    let bytes = js_sys::Uint8Array::from(download.bytes.as_slice());
    let parts = js_sys::Array::of1(&bytes);
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/octet-stream");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    let link: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
    link.set_href(&url);
    link.set_download(&download.name);
    link.click();
    web_sys::Url::revoke_object_url(&url)?;
    Ok(())
}
