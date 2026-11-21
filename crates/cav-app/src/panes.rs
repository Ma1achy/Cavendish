//! Where a pane's picture comes from. The shell asks a [`PaneSurface`] for a texture; the real one
//! draws with `cav-render` into a wgpu texture that egui shows as an image (panes are textures, not
//! paint callbacks: research note §1), and a null one lets the shell's layout and input be tested
//! with no GPU.

use std::sync::Arc;

use cav_gpu::Gpu;
use cav_render::{Frame, PaneTarget, Renderer};
use egui::mutex::RwLock;

/// Something that can show a view of the scene as an egui texture.
pub trait PaneSurface {
    /// The texture for pane `pane` at `size` pixels showing `frame`, or `None` if there is no GPU.
    /// `generation` changes whenever the engine's state does: the same `(generation, size)` means
    /// the same picture, so it need not be drawn again.
    fn texture(
        &mut self,
        pane: usize,
        size: [u32; 2],
        generation: u64,
        frame: Frame<'_>,
    ) -> Option<egui::TextureId>;
}

/// No GPU: the shell draws an empty pane. For tests of layout and input.
#[derive(Debug, Default)]
pub struct NoSurface;

impl PaneSurface for NoSurface {
    fn texture(&mut self, _: usize, _: [u32; 2], _: u64, _: Frame<'_>) -> Option<egui::TextureId> {
        None
    }
}

struct Slot {
    target: PaneTarget,
    id: egui::TextureId,
    generation: u64,
}

/// Panes drawn by `cav-render` and registered with egui's wgpu renderer.
pub struct GpuPanes {
    gpu: Gpu,
    renderer: Renderer,
    egui: Arc<RwLock<egui_wgpu::Renderer>>,
    slots: [Option<Slot>; cav_engine::MAX_PANES],
    /// How many times a pane has been drawn: tells a test or the profiler that an unchanged scene
    /// costs nothing.
    pub draws: u64,
}

impl GpuPanes {
    /// Panes on `gpu`, shown through `egui`'s renderer (eframe's, or a headless one).
    #[must_use]
    pub fn new(gpu: Gpu, egui: Arc<RwLock<egui_wgpu::Renderer>>) -> Self {
        let renderer = Renderer::new(&gpu);
        Self { gpu, renderer, egui, slots: [const { None }; cav_engine::MAX_PANES], draws: 0 }
    }
}

impl PaneSurface for GpuPanes {
    fn texture(
        &mut self,
        pane: usize,
        size: [u32; 2],
        generation: u64,
        frame: Frame<'_>,
    ) -> Option<egui::TextureId> {
        let size = [size[0].max(1), size[1].max(1)];
        let slot = self.slots.get_mut(pane)?;
        let resized = slot.as_ref().is_none_or(|s| s.target.size() != size);
        if resized {
            let target = PaneTarget::new(&self.gpu, size);
            // The egui renderer is locked only for the registration, never across the UI.
            let mut egui = self.egui.write();
            let id = match slot.take() {
                Some(old) => {
                    egui.update_egui_texture_from_wgpu_texture(
                        &self.gpu.device,
                        target.encoded_view(),
                        wgpu_filter(),
                        old.id,
                    );
                    old.id
                }
                None => egui.register_native_texture(&self.gpu.device, target.encoded_view(), wgpu_filter()),
            };
            // One less than the wanted generation, so the new texture is drawn below.
            *slot = Some(Slot { target, id, generation: generation.wrapping_sub(1) });
        }
        let slot = slot.as_mut().expect("just filled");
        if slot.generation != generation {
            self.renderer.draw(&self.gpu, &slot.target, frame);
            slot.generation = generation;
            self.draws += 1;
        }
        Some(slot.id)
    }
}

/// The pane's texture is the pane's size in pixels, so the filter only matters while resizing.
const fn wgpu_filter() -> cav_gpu::wgpu::FilterMode {
    cav_gpu::wgpu::FilterMode::Linear
}
