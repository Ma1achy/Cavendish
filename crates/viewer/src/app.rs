//! The eframe App — thin orchestration over the tested modules. It renders the 3D scene to an offscreen
//! texture (the same [`render`](crate::render) path the headless test drives) and shows it in egui; the
//! panels and scrubber read the pure [`scrub`](crate::scrub)/[`scene`](crate::scene) logic. The App
//! itself holds no asserted behaviour — coherence and fails-soft live below it.

use eframe::egui;
use egui::load::SizedTexture;

use crate::camera::Camera;
use crate::render::SceneRenderer;
use crate::scene::scene_at;
use crate::scrub;
use state::StateBundle;

/// The offscreen colour target the 3D scene renders into, then displayed as an egui image.
struct Offscreen {
    view: wgpu::TextureView,
    id: egui::TextureId,
    size: [u32; 2],
}

pub struct App {
    render_state: eframe::egui_wgpu::RenderState,
    scene: SceneRenderer,
    offscreen: Offscreen,
    camera: Camera,
    bundle: Option<StateBundle>,
    ell: usize,
    playing: bool,
    /// A soft-failure message shown to the user (never a crash).
    toast: Option<String>,
}

const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let render_state = cc
            .wgpu_render_state
            .clone()
            .ok_or("viewer requires the wgpu backend")?;
        let scene = SceneRenderer::new(&render_state.device, OFFSCREEN_FORMAT);
        let offscreen = make_offscreen(&render_state, [960, 720]);
        Ok(App {
            render_state,
            scene,
            offscreen,
            camera: Camera::default(),
            bundle: None,
            ell: 0,
            playing: false,
            toast: None,
        })
    }

    /// Re-render the 3D scene into the offscreen texture (recreating it on resize).
    fn draw_scene(&mut self, size: [u32; 2]) {
        if size != self.offscreen.size && size[0] > 0 && size[1] > 0 {
            self.offscreen = make_offscreen(&self.render_state, size);
        }
        self.camera.aspect = self.offscreen.size[0] as f32 / self.offscreen.size[1].max(1) as f32;
        let scene = match &self.bundle {
            Some(b) => scene_at(b, self.ell),
            None => crate::render::SceneData::new(),
        };
        self.scene.render(
            &self.render_state.device,
            &self.render_state.queue,
            &self.offscreen.view,
            &self.camera,
            &scene,
        );
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let times_len = self.bundle.as_ref().map_or(0, |b| b.time.len());
        if self.playing && times_len > 0 {
            self.ell = (self.ell + 1) % times_len;
            ctx.request_repaint();
        }

        egui::SidePanel::right("scenario").show(ctx, |ui| {
            ui.heading("Scenario");
            ui.label("Run / Load land in later commits.");
            if let Some(msg) = &self.toast {
                ui.colored_label(egui::Color32::LIGHT_RED, msg);
            }
        });

        egui::TopBottomPanel::bottom("scrubber").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_enabled_ui(times_len > 0, |ui| {
                    if ui.button(if self.playing { "⏸" } else { "▶" }).clicked() {
                        self.playing = !self.playing;
                    }
                    let mut ell = self.ell;
                    if times_len > 0
                        && ui
                            .add(egui::Slider::new(&mut ell, 0..=times_len - 1).text("ℓ"))
                            .changed()
                    {
                        self.ell = scrub::clamp_index(ell, times_len);
                    }
                });
                if let Some(b) = &self.bundle {
                    if let Some(t) = scrub::time_at(&b.time, self.ell) {
                        ui.label(format!("t = {t:.3}  ({}/{})", self.ell, times_len));
                    }
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let avail = ui.available_size();
            let size = [avail.x.max(1.0) as u32, avail.y.max(1.0) as u32];
            self.draw_scene(size);
            ui.image(SizedTexture::new(self.offscreen.id, avail));
        });
    }
}

/// Create an offscreen colour target and register it with egui for display.
fn make_offscreen(render_state: &eframe::egui_wgpu::RenderState, size: [u32; 2]) -> Offscreen {
    let texture = render_state
        .device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("viewer.offscreen"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let id = render_state.renderer.write().register_native_texture(
        &render_state.device,
        &view,
        wgpu::FilterMode::Linear,
    );
    Offscreen { view, id, size }
}
