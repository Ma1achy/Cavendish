//! The wgpu device, created once and shared: by the renderer now, by the kernels from S5.
//!
//! Vulkan is the default backend on the build PC, with DX12 as the fallback; set `WGPU_BACKEND=dx12`
//! to choose it (research note §1). Creating a device costs a few tenths of a second, so a process
//! makes one [`Gpu`] and keeps it.

pub use wgpu;

/// The instance, adapter, device and queue, kept together. Cloning is cheap: wgpu's handles are
/// reference-counted.
#[derive(Clone, Debug)]
pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

/// What `cav doctor` prints about one adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterReport {
    pub name: String,
    pub backend: String,
    pub kind: String,
    pub driver: String,
}

impl AdapterReport {
    fn of(info: &wgpu::AdapterInfo) -> Self {
        Self {
            name: info.name.clone(),
            backend: format!("{:?}", info.backend),
            kind: format!("{:?}", info.device_type),
            driver: format!("{} {}", info.driver, info.driver_info).trim().to_owned(),
        }
    }
}

impl std::fmt::Display for AdapterReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} · {} · {}", self.name, self.backend, self.kind)?;
        if !self.driver.is_empty() {
            write!(f, " · driver {}", self.driver)?;
        }
        Ok(())
    }
}

impl Gpu {
    /// Creates the device on the most capable adapter.
    ///
    /// # Errors
    /// When no adapter is found or the device cannot be created; the message says which.
    pub fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::VULKAN | wgpu::Backends::DX12),
            flags: wgpu::InstanceFlags::from_build_config().with_env(),
            backend_options: wgpu::BackendOptions::from_env_or_default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            display: None,
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU adapter (Vulkan or DX12) was found: {e}"))?;
        let (device, queue) = pollster::block_on(
            adapter
                .request_device(&wgpu::DeviceDescriptor { label: Some("cavendish"), ..Default::default() }),
        )
        .map_err(|e| format!("the adapter `{}` refused a device: {e}", adapter.get_info().name))?;
        Ok(Self { instance, adapter, device, queue })
    }

    /// Runs `work` and turns a validation error raised by wgpu while it ran into an `Err`, where
    /// it would otherwise reach wgpu's default handler and panic.
    ///
    /// # Errors
    /// What the GPU refused.
    pub fn checked<T>(&self, work: impl FnOnce() -> T) -> Result<T, String> {
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let done = work();
        match pollster::block_on(scope.pop()) {
            None => Ok(done),
            Some(error) => Err(format!("the GPU refused the work: {error}")),
        }
    }

    /// The adapter the device was created on.
    #[must_use]
    pub fn adapter_report(&self) -> AdapterReport {
        AdapterReport::of(&self.adapter.get_info())
    }

    /// Every adapter the instance can see, on every enabled backend.
    #[must_use]
    pub fn all_adapters(&self) -> Vec<AdapterReport> {
        pollster::block_on(self.instance.enumerate_adapters(wgpu::Backends::all()))
            .iter()
            .map(|a| AdapterReport::of(&a.get_info()))
            .collect()
    }

    /// Copies an RGBA8 texture back to the CPU: `width × height × 4` bytes, rows top to bottom.
    /// The texture must have been created with `COPY_SRC`.
    ///
    /// # Errors
    /// When the device is lost or the buffer cannot be mapped.
    pub fn read_rgba8(&self, texture: &wgpu::Texture) -> Result<Vec<u8>, String> {
        let (width, height) = (texture.width(), texture.height());
        let row_bytes = width * 4;
        let padded = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("read-back"),
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: None,
                },
            },
            texture.size(),
        );
        self.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| format!("waiting for the GPU: {e}"))?;
        receiver
            .recv()
            .map_err(|e| format!("the read-back never completed: {e}"))?
            .map_err(|e| format!("mapping the read-back buffer: {e}"))?;
        let data = slice.get_mapped_range().map_err(|e| format!("reading the mapped buffer: {e}"))?;
        let mut rgba = Vec::with_capacity((row_bytes * height) as usize);
        for row in 0..height {
            let start = (row * padded) as usize;
            rgba.extend_from_slice(&data[start..start + row_bytes as usize]);
        }
        Ok(rgba)
    }
}
