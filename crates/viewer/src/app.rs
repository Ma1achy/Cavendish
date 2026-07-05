//! The eframe App — thin orchestration over the tested modules. It renders the 3D scene (shaded voxel
//! cubes + wireframe gizmos) to an offscreen colour+depth texture (the same [`render`](crate::render)
//! path the headless test drives) and shows it in egui; text labels are projected and painted on top.
//! Camera orbit/zoom, the scene composition, and the panels/scrubber read the pure modules below —
//! the App itself holds no asserted behaviour (coherence and fails-soft live under it).

use eframe::egui;

use crate::camera::{self, Camera};
use crate::data::run_guarded;
use crate::params::{build_scenario, ScenarioParams};
use crate::render::{SceneData, SceneRenderer};
use crate::{gizmo, panels, scene, scrub};
use state::StateBundle;

/// The offscreen colour+depth target the 3D scene renders into; the colour is shown as an egui image.
struct Offscreen {
    colour: wgpu::TextureView,
    depth: wgpu::TextureView,
    id: egui::TextureId,
    size: [u32; 2],
}

/// Which overlays are drawn — each a toggle in the scenario panel.
struct Toggles {
    voxels: bool,
    body_wire: bool,
    detectors: bool,
    spin: bool,
    axes: bool,
    field: bool,
}

impl Default for Toggles {
    fn default() -> Self {
        Toggles {
            voxels: true,
            body_wire: true,
            detectors: true,
            spin: true,
            axes: true,
            field: false,
        }
    }
}

pub struct App {
    render_state: eframe::egui_wgpu::RenderState,
    scene: SceneRenderer,
    offscreen: Offscreen,
    camera: Camera,
    // Orbit state driven by pointer drag / scroll.
    azimuth: f32,
    elevation: f32,
    radius: f32,
    target: [f32; 3],
    params: ScenarioParams,
    bundle: Option<StateBundle>,
    ell: usize,
    playing: bool,
    toggles: Toggles,
    bundle_path: String,
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
            azimuth: 0.9,
            elevation: 0.5,
            radius: 12.0,
            target: [0.0, 0.0, 0.0],
            params: ScenarioParams::default(),
            bundle: None,
            ell: 0,
            playing: false,
            toggles: Toggles::default(),
            bundle_path: String::new(),
            toast: None,
        })
    }

    /// Adopt a new bundle: reset the scrubber and frame the camera on the whole scene.
    fn adopt(&mut self, bundle: StateBundle) {
        let (target, radius) = scene::frame_scene(&bundle);
        self.target = target;
        self.radius = radius;
        self.ell = 0;
        self.toast = None;
        self.bundle = Some(bundle);
    }

    /// Compose the frame's drawables from the enabled toggles: voxel cubes, then the wireframe gizmos.
    fn build_scene(&self) -> SceneData {
        let Some(b) = &self.bundle else {
            return SceneData::new();
        };
        let mut scene = if self.toggles.voxels {
            scene::scene_at(b, self.ell)
        } else {
            SceneData::new()
        };
        if self.toggles.field {
            scene::push_field_slice(&mut scene, b, self.ell);
        }
        if self.toggles.detectors {
            gizmo::push_detector_cages(&mut scene, &b.detector_placement, 0.15);
        }
        let spin_len = (self.radius as f64 * 0.15).max(0.5);
        for s in 0..b.source_cloud.len() {
            if self.toggles.body_wire {
                gizmo::push_body_wireframe(&mut scene, b, s, self.ell);
            }
            if self.toggles.spin {
                gizmo::push_spin_axis(&mut scene, b, s, self.ell, spin_len);
            }
        }
        if self.toggles.axes {
            let len = (self.radius as f64 * 0.55).max(1.0);
            gizmo::push_axes(&mut scene, len, nice_step(len));
        }
        scene
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
            ui.add(egui::Slider::new(&mut self.params.mass, 10.0..=5000.0).text("mass"));
            ui.add(egui::Slider::new(&mut self.params.distance, 1.0..=20.0).text("distance"));
            ui.add(egui::Slider::new(&mut self.params.omega0[1], 0.0..=2.0).text("ω₀·y"));
            ui.horizontal(|ui| {
                if ui.button("Run").clicked() {
                    match run_guarded(&build_scenario(&self.params)) {
                        Ok(b) => self.adopt(b),
                        Err(e) => self.toast = Some(e),
                    }
                }
                if ui.button("Frame").clicked() {
                    if let Some(b) = &self.bundle {
                        let (t, r) = scene::frame_scene(b);
                        self.target = t;
                        self.radius = r;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.bundle_path);
                if ui.button("Load").clicked() {
                    match state::load_bundle(&self.bundle_path) {
                        Ok(b) => self.adopt(b),
                        Err(e) => self.toast = Some(format!("load failed: {e}")),
                    }
                }
            });
            if let Some(msg) = &self.toast {
                ui.colored_label(egui::Color32::LIGHT_RED, msg);
            }
            ui.separator();
            ui.label("Show");
            ui.checkbox(&mut self.toggles.voxels, "voxels");
            ui.checkbox(&mut self.toggles.body_wire, "object wireframe");
            ui.checkbox(&mut self.toggles.detectors, "detectors");
            ui.checkbox(&mut self.toggles.spin, "spin axis");
            ui.checkbox(&mut self.toggles.axes, "world axes");
            ui.checkbox(&mut self.toggles.field, "field slice");
            ui.weak("drag to orbit · scroll to zoom");
            ui.separator();
            ui.heading("Periodogram");
            match &self.bundle {
                Some(b) => panels::periodogram_panel(ui, b),
                None => {
                    ui.weak("no run yet");
                }
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

        egui::TopBottomPanel::bottom("signal")
            .resizable(true)
            .default_height(180.0)
            .show(ctx, |ui| match &self.bundle {
                Some(b) => panels::signal_panel(ui, b, self.ell),
                None => {
                    ui.weak("no run — Run or Load a bundle");
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let avail = ui.available_size();
            let (rect, resp) = ui.allocate_exact_size(avail, egui::Sense::click_and_drag());

            // Orbit on drag, zoom on scroll (while hovered).
            if resp.dragged() {
                let d = resp.drag_delta();
                self.azimuth -= d.x * 0.008;
                self.elevation = (self.elevation + d.y * 0.008).clamp(-1.5, 1.5);
            }
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if resp.hovered() && scroll != 0.0 {
                self.radius = (self.radius * (1.0 - scroll * 0.0015)).clamp(1.0, 500.0);
            }

            let size = [rect.width().max(1.0) as u32, rect.height().max(1.0) as u32];
            if size != self.offscreen.size {
                self.offscreen = make_offscreen(&self.render_state, size);
            }
            self.camera.aspect = rect.width() / rect.height().max(1.0);
            self.camera
                .set_orbit(self.target, self.azimuth, self.elevation, self.radius);

            let scene = self.build_scene();
            self.scene.render(
                &self.render_state.device,
                &self.render_state.queue,
                &self.offscreen.colour,
                &self.offscreen.depth,
                &self.camera,
                &scene,
            );

            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter()
                .image(self.offscreen.id, rect, uv, egui::Color32::WHITE);

            // Project and paint the world-anchored text labels over the image.
            let vp = self.camera.view_proj();
            let bounds = [rect.min.x, rect.min.y, rect.width(), rect.height()];
            for label in &scene.labels {
                if let Some(px) = camera::project(&vp, label.at, bounds) {
                    ui.painter().text(
                        egui::pos2(px[0], px[1]),
                        egui::Align2::CENTER_CENTER,
                        &label.text,
                        egui::FontId::monospace(12.0),
                        col32(label.colour),
                    );
                }
            }
        });
    }
}

fn col32(c: [f32; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
    )
}

/// A tick spacing that divides `len` into ~5 rounded steps (1/2/5 × 10ⁿ).
fn nice_step(len: f64) -> f64 {
    let raw = (len / 5.0).max(1e-6);
    let mag = 10f64.powf(raw.log10().floor());
    let n = raw / mag;
    mag * if n < 1.5 {
        1.0
    } else if n < 3.5 {
        2.0
    } else if n < 7.5 {
        5.0
    } else {
        10.0
    }
}

/// Create the offscreen colour+depth target and register the colour with egui for display.
fn make_offscreen(render_state: &eframe::egui_wgpu::RenderState, size: [u32; 2]) -> Offscreen {
    let device = &render_state.device;
    let extent = wgpu::Extent3d {
        width: size[0],
        height: size[1],
        depth_or_array_layers: 1,
    };
    let colour_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("viewer.offscreen.colour"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OFFSCREEN_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let depth_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("viewer.offscreen.depth"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: crate::render::DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let colour = colour_tex.create_view(&wgpu::TextureViewDescriptor::default());
    let depth = depth_tex.create_view(&wgpu::TextureViewDescriptor::default());
    let id = render_state.renderer.write().register_native_texture(
        &render_state.device,
        &colour,
        wgpu::FilterMode::Linear,
    );
    Offscreen {
        colour,
        depth,
        id,
        size,
    }
}
