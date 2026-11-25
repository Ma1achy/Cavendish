//! The device in a browser page: never made here, only handed over ([`Gpu::from_parts`], from
//! eframe's), because a page cannot block on a promise. What would block on native is refused
//! or done without waiting, and each says so.

use crate::{AdapterReport, Gpu};

impl Gpu {
    /// A page cannot make a device of its own without waiting on the browser: it is handed
    /// eframe's with [`Gpu::from_parts`].
    ///
    /// # Errors
    /// Always, with that explanation.
    pub fn new() -> Result<Self, String> {
        Err("in the browser the device is the page's own, handed over with `Gpu::from_parts`; none is made here".into())
    }

    /// Runs `work`. A page cannot wait for an error scope, so a validation error raised while it
    /// ran is not turned into an `Err` here: the browser reports it in its console (wgpu installs
    /// no handler of its own on the web, and nor does eframe), and the page goes on.
    ///
    /// # Errors
    /// Never, on the web.
    pub fn checked<T>(&self, work: impl FnOnce() -> T) -> Result<T, String> {
        Ok(work())
    }

    /// The one adapter the page was given: a page cannot list the others.
    #[must_use]
    pub fn all_adapters(&self) -> Vec<AdapterReport> {
        vec![self.adapter_report()]
    }

    /// A page cannot wait for the GPU, so pixels are not read back; figures with pixels are
    /// made by the native application or `cav`.
    ///
    /// # Errors
    /// Always, with that explanation.
    pub fn read_rgba8(&self, _texture: &wgpu::Texture) -> Result<Vec<u8>, String> {
        Err("reading pixels back waits for the GPU, which a browser page cannot do: PNG figures and screenshots \
             are made by the native application or `cav`"
            .into())
    }

    /// A page cannot wait for the GPU, so buffers are not read back either.
    ///
    /// # Errors
    /// Always, with that explanation.
    pub fn read_buffer(&self, _buffer: &wgpu::Buffer) -> Result<Vec<u8>, String> {
        Err("reading a buffer back waits for the GPU, which a browser page cannot do".into())
    }
}
