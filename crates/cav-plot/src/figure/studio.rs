//! The figure exporter's pixels on a native process: its own device and renderers.

use cav_engine::Engine;

use crate::headless::Painter;

/// What an exporter keeps between figures: the device and the renderers, made the first time a
/// figure needs pixels.
#[derive(Default)]
pub struct Studio {
    painter: Option<Painter>,
    renderer: Option<cav_render::Renderer>,
}

impl Studio {
    pub(super) fn painter(&mut self) -> Result<&mut Painter, String> {
        if self.painter.is_none() {
            self.painter = Some(Painter::new(crate::headless::context())?);
        }
        Ok(self.painter.as_mut().expect("just made"))
    }

    /// The scene as pane `pane` of `engine` sees it, `size` pixels, as RGBA.
    pub(super) fn view(&mut self, engine: &Engine, pane: usize, size: [u32; 2]) -> Result<Vec<u8>, String> {
        let gpu = self.painter()?.gpu.clone();
        let renderer = self.renderer.get_or_insert_with(|| cav_render::Renderer::new(&gpu));
        renderer.render_rgba(&gpu, engine.frame(pane)?, size)
    }
}
