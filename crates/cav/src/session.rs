//! An engine with a renderer beside it, for the CLI and the MCP server.
//!
//! The device is created on first use and kept: creating one costs a few tenths of a second, and
//! commands that draw nothing (`scenes`, `schema`, `exec`) should not need a GPU at all.

use cav_engine::Engine;
use cav_gpu::Gpu;
use cav_render::Renderer;

/// The longest a picture of a plot waits for the signal it shows, seconds of wall time. The dev
/// scenes' signals take well under a second; a picture drawn before the signal is whole says
/// "computing the signal" (or "stale" in its title) rather than showing nothing.
const PLOT_PATIENCE: std::time::Duration = std::time::Duration::from_secs(10);

/// One engine and, once something has been drawn, its device and pipelines.
pub struct Session {
    pub engine: Engine,
    gpu: Option<(Gpu, Renderer)>,
    /// The painter with no window, for panes that show plots.
    plots: Option<cav_plot::headless::Painter>,
}

impl Session {
    /// A fresh engine; no device yet.
    pub fn new() -> Self {
        let mut engine = Engine::new();
        // A client that prints numbers waits for them: a line that asks for a signal gets the
        // current scene's, or, after a minute, the last whole one marked stale.
        engine.set_patience(std::time::Duration::from_secs(60));
        // The engine does not draw: figures are cav-plot's.
        cav_plot::figure::install(&mut engine);
        Self { engine, gpu: None, plots: None }
    }

    /// The device and the renderer, created the first time they are asked for.
    ///
    /// # Errors
    /// When no adapter is found or the device cannot be created.
    pub fn gpu(&mut self) -> Result<&(Gpu, Renderer), String> {
        if self.gpu.is_none() {
            let gpu = Gpu::new()?;
            let renderer = Renderer::new(&gpu);
            self.gpu = Some((gpu, renderer));
        }
        Ok(self.gpu.as_ref().expect("just created"))
    }

    /// Loads a library scene or a scene file, as the command the engine journals. The name or
    /// path is taken whole, spaces and all: it never passes through the console's words.
    ///
    /// # Errors
    /// When the scene cannot be found or read, or the engine refuses it.
    pub fn load(&mut self, scene: &str) -> Result<(), String> {
        match cav_sim::load(scene) {
            Ok(spec) => self.engine.apply(cav_engine::Command::load(spec)),
            Err(why) => {
                // A scene that cannot be read is a refusal like any other: it goes in the log.
                self.engine.note(cav_engine::Level::Error, cav_engine::Subsystem::Engine, why.0.clone());
                Err(why.0)
            }
        }
    }

    /// Draws what a pane shows as a PNG: its view of the engine's scene with the selection
    /// highlighted, or its plot.
    ///
    /// # Errors
    /// When there is no GPU, no such pane, or the read-back fails.
    pub fn render_png(&mut self, pane: usize, size: [u32; 2]) -> Result<Vec<u8>, String> {
        // The engine says what the pane shows; refusing a pane that does not exist comes first,
        // before a device is made for nothing.
        let frame = self.engine.frame(pane)?;
        if frame.content != cav_engine::PaneContent::Scene {
            // A plot is drawn by the code the application draws it with, once the signal it
            // shows is the scene's (or as it stands, saying so, if that takes longer than the wait).
            self.engine.settle(PLOT_PATIENCE);
            if self.plots.is_none() {
                self.plots = Some(cav_plot::headless::Painter::new(cav_plot::headless::context())?);
            }
            return self.plots.as_mut().expect("just created").plot_png(&self.engine, pane, size);
        }
        self.gpu()?;
        let (gpu, renderer) = self.gpu.as_ref().expect("just created");
        renderer.render_png(gpu, self.engine.frame(pane)?, size)
    }
}
