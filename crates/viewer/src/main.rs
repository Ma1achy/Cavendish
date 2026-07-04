//! The viewer binary: an eframe window on the wgpu backend (the same `wgpu 22` as compute).

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "Cavendish viewer",
        options,
        Box::new(|cc| Ok(Box::new(viewer::app::App::new(cc)?))),
    )
}
