//! An engine with a renderer beside it, for the CLI and the MCP server.
//!
//! The device is created on first use and kept: creating one costs a few tenths of a second, and
//! commands that draw nothing (`scenes`, `schema`, `exec`) should not need a GPU at all.

use cav_engine::Engine;
use cav_gpu::Gpu;
use cav_render::{Frame, Renderer};

/// One engine and, once something has been drawn, its device and pipelines.
pub struct Session {
    pub engine: Engine,
    gpu: Option<(Gpu, Renderer)>,
}

impl Session {
    /// A fresh engine; no device yet.
    pub fn new() -> Self {
        Self { engine: Engine::new(), gpu: None }
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

    /// Draws a pane's view of the engine's scene, with the selection highlighted, as a PNG.
    ///
    /// # Errors
    /// When there is no GPU, no such pane, or the read-back fails.
    pub fn render_png(&mut self, pane: usize, size: [u32; 2]) -> Result<Vec<u8>, String> {
        if pane >= cav_engine::MAX_PANES {
            return Err(format!("there is no pane {}; panes are 1 to {}", pane + 1, cav_engine::MAX_PANES));
        }
        self.gpu()?;
        let (gpu, renderer) = self.gpu.as_ref().expect("just created");
        let frame = Frame {
            scene: self.engine.spec(),
            view: self.engine.view(pane),
            highlight: self.engine.selection(),
        };
        renderer.render_png(gpu, frame, size)
    }
}
