//! The figure exporter in a browser page: no pixels. A page cannot wait for the GPU to hand a
//! picture back, so PNG figures and the renderer's view are refused with the reason; SVG and PDF
//! figures, and views of voxel bodies (drawn as vectors), are made as natively.

use cav_engine::Engine;

use crate::headless::Painter;

/// What the exporter keeps between figures: in a page, nothing.
#[derive(Default)]
pub struct Studio {
    _nothing: (),
}

/// Why there are no pixels here.
const NO_PIXELS: &str = "a picture made on the GPU cannot be read back in a browser page: choose SVG or PDF, \
                         or make PNG figures and views of the 3-D scene with the native application or `cav`";

impl Studio {
    /// Refused: see [`NO_PIXELS`].
    #[allow(clippy::unused_self)]
    pub(super) fn painter(&mut self) -> Result<&mut Painter, String> {
        Err(NO_PIXELS.into())
    }

    /// Refused: see [`NO_PIXELS`].
    #[allow(clippy::unused_self)]
    pub(super) fn view(
        &mut self,
        _engine: &Engine,
        _pane: usize,
        _size: [u32; 2],
    ) -> Result<Vec<u8>, String> {
        Err(NO_PIXELS.into())
    }
}
