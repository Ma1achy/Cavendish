//! Painting with no window: an egui context of our own, laid out frame by frame and painted by
//! egui's wgpu renderer into a texture that is read back.
//!
//! The application's screenshots, the pictures of plot panes and PNG figures are all made here.
//! Nothing in this module opens a window or takes the focus.
//!
//! Not for a browser page: it reads pixels back, which a page cannot wait for. It compiles there
//! (the figure exporter names its types) and refuses at its first read-back; egui's renderer is
//! not `Send` on the web, hence the allowance below: the lint fires only there (natively the
//! renderer is `Send`), and it is harmless with one thread. Unconditional, so that no `cfg` on
//! the target stands outside a seam.

#![allow(clippy::arc_with_non_send_sync)]

use std::sync::Arc;

use cav_gpu::Gpu;
use cav_gpu::wgpu;
use egui::mutex::RwLock;

/// The time step between headless frames, seconds.
pub const FRAME_DT: f64 = 1.0 / 60.0;

/// The format the UI is painted into. Not sRGB: egui blends in gamma space and writes final bytes.
const UI_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The input of one frame: a window of `size` points at one pixel per point, at `time` seconds,
/// with `events`.
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
    // Focused, as a window in front is, but for a frame that says the window lost its focus.
    let focused = !events.iter().any(|event| matches!(event, egui::Event::WindowFocused(false)));
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(size[0] as f32, size[1] as f32),
        )),
        time: Some(time),
        predicted_dt: FRAME_DT as f32,
        focused,
        events,
        ..Default::default()
    }
}

/// An egui context with the two faces installed and nothing else changed.
#[must_use]
pub fn context() -> egui::Context {
    let ctx = egui::Context::default();
    ctx.set_fonts(crate::fonts::definitions());
    ctx
}

/// A device, an egui context and egui's renderer, with no window.
pub struct Painter {
    /// The device everything is painted on.
    pub gpu: Gpu,
    /// The context frames are run in.
    pub ctx: egui::Context,
    /// egui's renderer; shared, so that a client can register textures of its own with it.
    pub egui: Arc<RwLock<egui_wgpu::Renderer>>,
    /// The time of the last frame, seconds; it advances by [`FRAME_DT`] a frame.
    pub clock: f64,
}

impl Painter {
    /// Creates the device and the renderer, to paint frames of `ctx`.
    ///
    /// # Errors
    /// When there is no adapter or no device.
    pub fn new(ctx: egui::Context) -> Result<Self, String> {
        let gpu = Gpu::new()?;
        let options = egui_wgpu::RendererOptions { msaa_samples: 1, ..Default::default() };
        let egui = Arc::new(RwLock::new(egui_wgpu::Renderer::new(&gpu.device, UI_FORMAT, options)));
        Ok(Self { gpu, ctx, egui, clock: 0.0 })
    }

    /// Hands a frame's font and image changes to the renderer. Every frame's must reach it, not
    /// only those of the frame that is painted.
    pub fn take_textures(&self, frame: &mut egui::FullOutput) {
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

    /// Paints a frame's shapes over `clear` into a texture of `size` pixels and reads it back as
    /// RGBA8, rows top to bottom.
    ///
    /// # Errors
    /// When the read-back fails.
    pub fn paint(
        &self,
        output: egui::FullOutput,
        size: [u32; 2],
        clear: egui::Color32,
    ) -> Result<Vec<u8>, String> {
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
            let [r, g, b, _] = clear.to_normalized_gamma_f32();
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
        let rgba = self.paint(output.expect("two frames"), size, crate::theme::BACKGROUND)?;
        cav_render::encode_png(&rgba, size)
    }

    /// A drawing as RGBA8 of `pixels` wide and high, rows top to bottom: the whole drawing fills
    /// the picture, at as many pixels per point as that takes, over `clear`.
    ///
    /// # Errors
    /// When the read-back fails.
    pub fn drawing_rgba(
        &mut self,
        drawing: &crate::drawing::Drawing,
        pixels: [u32; 2],
        clear: egui::Color32,
    ) -> Result<Vec<u8>, String> {
        let pixels = [pixels[0].max(1), pixels[1].max(1)];
        let per_point = pixels[0] as f32 / drawing.size[0];
        self.ctx.set_pixels_per_point(per_point);
        let mut output = None;
        // The scale takes hold at the start of a frame and the text is laid out in the next:
        // three frames, the last one kept.
        for _ in 0..3 {
            self.clock += FRAME_DT;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(drawing.size[0], drawing.size[1]),
                )),
                time: Some(self.clock),
                predicted_dt: FRAME_DT as f32,
                focused: true,
                ..Default::default()
            };
            let mut frame = self.ctx.run_ui(input, |ui| {
                let painter = ui.painter_at(egui::Rect::EVERYTHING);
                crate::paint::paint(&painter, egui::Pos2::ZERO, drawing);
            });
            self.take_textures(&mut frame);
            output = Some(frame);
        }
        // The context goes back to a pixel a point for whatever is painted next. A scale takes
        // hold at the start of a frame, so an empty frame is run for it: left pending, it undid
        // the next picture's scale whenever that was this one's again (two PNG figures of one
        // size from one engine: the second was refused).
        self.ctx.set_pixels_per_point(1.0);
        self.clock += FRAME_DT;
        let mut settled = self.ctx.run_ui(input(pixels, self.clock, Vec::new()), |_| {});
        self.take_textures(&mut settled);
        let output = output.expect("three frames");
        if (output.pixels_per_point - per_point).abs() > 1e-3 * per_point {
            return Err(format!(
                "the picture was laid out at {} pixels per point, not the {per_point} asked for",
                output.pixels_per_point
            ));
        }
        self.paint(output, pixels, clear)
    }

    /// As [`Self::drawing_rgba`], encoded as a PNG.
    ///
    /// # Errors
    /// When the read-back or the encoding fails.
    pub fn drawing_png(
        &mut self,
        drawing: &crate::drawing::Drawing,
        pixels: [u32; 2],
        clear: egui::Color32,
    ) -> Result<Vec<u8>, String> {
        let rgba = self.drawing_rgba(drawing, pixels, clear)?;
        cav_render::encode_png(&rgba, [pixels[0].max(1), pixels[1].max(1)])
    }
}
