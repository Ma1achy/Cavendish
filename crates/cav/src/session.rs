//! An engine with a renderer beside it, for the CLI and the MCP server.
//!
//! The device is created on first use and kept: creating one costs a few tenths of a second, and
//! commands that draw nothing (`scenes`, `schema`, `exec`) should not need a GPU at all.

use cav_engine::Engine;
use cav_gpu::Gpu;
use cav_render::Renderer;

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

    /// Loads a library scene or a scene file, as the command the engine journals. The name or
    /// path is taken whole, spaces and all: it never passes through the console's words.
    ///
    /// # Errors
    /// When the scene cannot be found or read, or the engine refuses it.
    pub fn load(&mut self, scene: &str) -> Result<(), String> {
        match cav_sim::load(scene) {
            Ok(spec) => self.engine.apply(cav_engine::Command::Load(spec)),
            Err(why) => {
                // A scene that cannot be read is a refusal like any other: it goes in the log.
                self.engine.note(cav_engine::Level::Error, cav_engine::Subsystem::Engine, why.0.clone());
                Err(why.0)
            }
        }
    }

    /// Draws a pane's view of the engine's scene, with the selection highlighted, as a PNG.
    ///
    /// # Errors
    /// When there is no GPU, no such pane, or the read-back fails.
    pub fn render_png(&mut self, pane: usize, size: [u32; 2]) -> Result<Vec<u8>, String> {
        // The engine says what the pane shows; refusing a pane that does not exist comes first,
        // before a device is made for nothing.
        self.engine.frame(pane)?;
        self.gpu()?;
        let (gpu, renderer) = self.gpu.as_ref().expect("just created");
        renderer.render_png(gpu, self.engine.frame(pane)?, size)
    }
}
