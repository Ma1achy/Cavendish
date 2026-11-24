//! The wgpu device, created once and shared: by the renderer now, by the kernels from S5.
//!
//! Vulkan is the default backend on the build PC, with DX12 as the fallback; set `WGPU_BACKEND=dx12`
//! to choose it (research note §1). Creating a device costs a few tenths of a second, so a process
//! makes one [`Gpu`] and keeps it.
//!
//! ## What this crate is for
//! One wgpu device per process and what is needed to get pixels back from it: [`Gpu`], the
//! adapter reports `cav doctor` prints, and [`Gpu::read_rgba8`]. It re-exports `wgpu` so that every
//! crate uses the one pinned version.
//!
//! ## What it may depend on
//! Of the workspace, `cav-core` only (today it needs none of it).
//! It must not hold anything that knows what is being drawn or computed for a scene: no scene
//! spec, no renderer, no engine, no windows.

#![warn(missing_docs)]

pub use wgpu;

/// The instance, adapter, device and queue, kept together. Cloning is cheap: wgpu's handles are
/// reference-counted.
#[derive(Clone, Debug)]
pub struct Gpu {
    /// wgpu's entry point: the backends that were enabled (Vulkan and DX12, or what
    /// `WGPU_BACKEND` names).
    pub instance: wgpu::Instance,
    /// The physical adapter the device was made on: the most capable one found.
    pub adapter: wgpu::Adapter,
    /// The device: what buffers, textures and pipelines are made with.
    pub device: wgpu::Device,
    /// The device's queue: what commands and buffer writes are submitted to.
    pub queue: wgpu::Queue,
}

/// What `cav doctor` prints about one adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterReport {
    /// The adapter's name as its driver gives it (the graphics card's model).
    pub name: String,
    /// The backend it is reached through, as wgpu names it: `Vulkan`, `Dx12`.
    pub backend: String,
    /// What sort of adapter it is, as wgpu names it: `DiscreteGpu`, `IntegratedGpu`, `Cpu`.
    pub kind: String,
    /// The driver's name and version; empty when the backend reports neither.
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

// The platform seam: how a device is made, how errors are caught and how pixels come back
// differ between a process (which may block) and a browser page (which may not). One module each.
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

impl Gpu {
    /// Wraps a device made elsewhere: in the browser, eframe's (a page cannot block on making
    /// its own, and has one already). The four handles must belong together: the adapter the
    /// device was made on, from that instance, with the device's own queue.
    #[must_use]
    pub const fn from_parts(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Self {
        Self { instance, adapter, device, queue }
    }

    /// The adapter the device was created on.
    #[must_use]
    pub fn adapter_report(&self) -> AdapterReport {
        AdapterReport::of(&self.adapter.get_info())
    }
}
