//! The whole application drawn with no window: the same [`Shell::ui`] the window runs, laid out
//! by an egui context of our own and painted by `cav-plot`'s windowless painter into a texture.
//!
//! This is how screenshots and the UI's GPU tests are made. Nothing here opens a window or takes
//! the focus.

use std::sync::Arc;

pub use cav_plot::headless::input;
use cav_plot::headless::{FRAME_DT, Painter};

use crate::panes::{GpuPanes, PaneSurface};
use crate::shell::Shell;
use crate::theme;

/// An egui context styled as the application's, for running frames with no window.
#[must_use]
pub fn context() -> egui::Context {
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    ctx
}

/// Runs frames of the shell: one per entry of `frames`, each with that entry's events. Returns the
/// last frame's output. Time advances by a sixtieth of a second per frame from `*clock`.
pub fn run_frames(
    ctx: &egui::Context,
    shell: &mut Shell,
    surface: &mut dyn PaneSurface,
    size: [u32; 2],
    clock: &mut f64,
    frames: Vec<Vec<egui::Event>>,
) -> egui::FullOutput {
    let mut last = None;
    for events in frames {
        *clock += FRAME_DT;
        let mut output = ctx.run_ui(input(size, *clock, events), |ui| shell.ui(ui, surface));
        // Nothing is painted here, so the font atlas updates have nowhere to go.
        output.textures_delta.clear();
        last = Some(output);
    }
    last.expect("at least one frame")
}

/// The application's renderer with no window.
pub struct Headless {
    painter: Painter,
    panes: GpuPanes,
}

impl Headless {
    /// Creates the device and the renderers.
    ///
    /// # Errors
    /// When there is no adapter or no device.
    pub fn new() -> Result<Self, String> {
        let painter = Painter::new(context())?;
        let panes = GpuPanes::new(painter.gpu.clone(), Arc::clone(&painter.egui));
        Ok(Self { painter, panes })
    }

    /// The adapter in use, for the About box and the timings.
    #[must_use]
    pub fn adapter(&self) -> String {
        self.painter.gpu.adapter_report().to_string()
    }

    /// How many times a pane has been drawn since the start.
    #[must_use]
    pub const fn pane_draws(&self) -> u64 {
        self.panes.draws
    }

    /// Runs `frames` frames of input (each a list of events) and returns the last one as RGBA8
    /// pixels, `size[0] × size[1] × 4` bytes, rows top to bottom.
    ///
    /// # Errors
    /// When the read-back fails.
    pub fn render(
        &mut self,
        shell: &mut Shell,
        size: [u32; 2],
        frames: Vec<Vec<egui::Event>>,
    ) -> Result<Vec<u8>, String> {
        let size = [size[0].max(1), size[1].max(1)];
        let mut output = None;
        for events in frames {
            self.painter.clock += FRAME_DT;
            let mut frame = self
                .painter
                .ctx
                .run_ui(input(size, self.painter.clock, events), |ui| shell.ui(ui, &mut self.panes));
            // Every frame's texture changes must reach the renderer, not only the last frame's.
            self.painter.take_textures(&mut frame);
            output = Some(frame);
        }
        let output = output.ok_or("no frames to render")?;
        self.painter.paint(output, size, theme::BACKGROUND)
    }

    /// One plot pane of `engine` as a PNG: see [`Painter::plot_png`].
    ///
    /// # Errors
    /// As [`Painter::plot_png`].
    pub fn plot_png(
        &mut self,
        engine: &cav_engine::Engine,
        pane: usize,
        size: [u32; 2],
    ) -> Result<Vec<u8>, String> {
        self.painter.plot_png(engine, pane, size)
    }

    /// A screenshot: a dozen frames so everything settles, the last one encoded as a PNG.
    ///
    /// # Errors
    /// When the read-back or the encoding fails.
    pub fn screenshot(&mut self, shell: &mut Shell, size: [u32; 2]) -> Result<Vec<u8>, String> {
        // Enough frames for panels to settle and for a window's fade-in to finish.
        let rgba = self.render(shell, size, vec![Vec::new(); 12])?;
        cav_render::encode_png(&rgba, [size[0].max(1), size[1].max(1)])
    }
}
