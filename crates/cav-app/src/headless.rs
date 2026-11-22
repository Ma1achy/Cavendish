//! The whole application drawn with no window: the same [`Shell::ui`] the window runs, laid out
//! by an egui context of our own and painted by egui's wgpu renderer into a texture.
//!
//! This is how screenshots and the UI's GPU tests are made. Nothing here opens a window or takes
//! the focus.

use std::sync::Arc;

use cav_gpu::Gpu;
use cav_gpu::wgpu;
use egui::mutex::RwLock;

use crate::panes::{GpuPanes, PaneSurface};
use crate::shell::Shell;
use crate::theme;

/// The time step between headless frames, seconds.
const FRAME_DT: f64 = 1.0 / 60.0;

/// An egui context styled as the application's, for running frames with no window.
#[must_use]
pub fn context() -> egui::Context {
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    ctx
}

/// The input of one frame: a window of `size` points at one pixel per point, at `time`, with `events`.
#[must_use]
pub fn input(size: [u32; 2], time: f64, mut events: Vec<egui::Event>) -> egui::RawInput {
    // A window reports the held modifiers before the keys they modify; here they are whatever the
    // frame's key and pointer events say, and none otherwise.
    let modifiers = events
        .iter()
        .filter_map(|event| match event {
            egui::Event::Key { modifiers, pressed: true, .. }
            | egui::Event::PointerButton { modifiers, .. } => Some(*modifiers),
            _ => None,
        })
        .next_back()
        .unwrap_or_default();
    events.insert(0, egui::Event::ModifiersChanged(modifiers));
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(size[0] as f32, size[1] as f32),
        )),
        time: Some(time),
        predicted_dt: FRAME_DT as f32,
        focused: true,
        events,
        ..Default::default()
    }
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
    gpu: Gpu,
    ctx: egui::Context,
    egui: Arc<RwLock<egui_wgpu::Renderer>>,
    panes: GpuPanes,
    clock: f64,
}

/// The format the UI is painted into. Not sRGB: egui blends in gamma space and writes final bytes.
const UI_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

impl Headless {
    /// Creates the device and the renderers.
    ///
    /// # Errors
    /// When there is no adapter or no device.
    pub fn new() -> Result<Self, String> {
        let gpu = Gpu::new()?;
        let options = egui_wgpu::RendererOptions { msaa_samples: 1, ..Default::default() };
        let egui = Arc::new(RwLock::new(egui_wgpu::Renderer::new(&gpu.device, UI_FORMAT, options)));
        let panes = GpuPanes::new(gpu.clone(), Arc::clone(&egui));
        Ok(Self { gpu, ctx: context(), egui, panes, clock: 0.0 })
    }

    /// The adapter in use, for the About box and the timings.
    #[must_use]
    pub fn adapter(&self) -> String {
        self.gpu.adapter_report().to_string()
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
            self.clock += FRAME_DT;
            let mut frame =
                self.ctx.run_ui(input(size, self.clock, events), |ui| shell.ui(ui, &mut self.panes));
            // Every frame's texture changes must reach the renderer, not only the last frame's.
            self.take_textures(&mut frame);
            output = Some(frame);
        }
        let output = output.ok_or("no frames to render")?;
        self.paint(output, size)
    }

    /// One plot pane of `engine`, filling a picture of `size` pixels, as a PNG: what
    /// [`crate::plots::paint`] draws in the application, with no application round it.
    ///
    /// # Errors
    /// When the pane shows the scene (which `cav-render` draws, not this), or the read-back or
    /// the encoding fails.
    pub fn plot_png(
        &mut self,
        engine: &cav_engine::Engine,
        pane: usize,
        size: [u32; 2],
    ) -> Result<Vec<u8>, String> {
        let size = [size[0].max(1), size[1].max(1)];
        let mut drew = false;
        let mut output = None;
        // Two frames: the first lays the text out, the second is the one kept.
        for _ in 0..2 {
            self.clock += FRAME_DT;
            let mut frame = self.ctx.run_ui(input(size, self.clock, Vec::new()), |ui| {
                let rect = ui.ctx().content_rect();
                drew = !crate::plots::paint(engine, &ui.painter_at(rect), pane, rect).is_empty();
            });
            self.take_textures(&mut frame);
            output = Some(frame);
        }
        if !drew {
            return Err(format!("pane {} shows the scene, not a plot", pane + 1));
        }
        let rgba = self.paint(output.expect("two frames"), size)?;
        cav_render::encode_png(&rgba, size)
    }

    /// Hands a frame's font and image changes to the renderer.
    fn take_textures(&self, frame: &mut egui::FullOutput) {
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        let mut egui = self.egui.write();
        for (id, deltas) in &frame.textures_delta.set {
            for delta in deltas {
                egui.update_texture(device, queue, *id, delta);
            }
        }
        for id in &frame.textures_delta.free {
            egui.free_texture(id);
        }
        frame.textures_delta.clear();
    }

    /// Paints a frame's shapes into a texture of `size` pixels and reads it back as RGBA8.
    fn paint(&self, output: egui::FullOutput, size: [u32; 2]) -> Result<Vec<u8>, String> {
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        let jobs = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        let screen =
            egui_wgpu::ScreenDescriptor { size_in_pixels: size, pixels_per_point: output.pixels_per_point };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ui"),
            size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: UI_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("ui") });
        let mut egui = self.egui.write();
        let extra = egui.update_buffers(device, queue, &mut encoder, &jobs, &screen);
        {
            let [r, g, b, _] = theme::BACKGROUND.to_normalized_gamma_f32();
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: r.into(),
                            g: g.into(),
                            b: b.into(),
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            egui.render(&mut pass.forget_lifetime(), &jobs, &screen);
        }
        drop(egui);
        queue.submit(extra.into_iter().chain([encoder.finish()]));
        self.gpu.read_rgba8(&texture)
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
