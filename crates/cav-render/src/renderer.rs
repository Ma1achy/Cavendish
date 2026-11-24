//! The wgpu passes: one function draws a view of a scene into a texture it is given. The app's
//! panes, `cav render` and MCP `render_view` all call it; none of them needs a window.

use bytemuck::{Pod, Zeroable};
use cav_gpu::Gpu;
use cav_gpu::wgpu::{self, util::DeviceExt};
use cav_sim::DrawStyle;

use crate::camera::Camera;
use crate::scene::{Drawables, Line, Vertex};

/// The colour format of every pane: sRGB, so shaders work in linear light.
pub const COLOUR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
/// The same bytes without the sRGB decoding on read; see [`PaneTarget::encoded_view`].
const ENCODED_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Samples per pixel: lines and edges are antialiased by multisampling.
const SAMPLES: u32 = 4;

/// What to draw. The type is `cav-sim`'s, so that the engine can make one without knowing the renderer.
pub use cav_sim::SceneView as Frame;

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    eye_lit: [f32; 4],
    viewport_ground: [f32; 4],
    phase: [f32; 4],
    ground_surface: [f32; 4],
    ground_line: [f32; 4],
    background: [f32; 4],
}

/// The textures one pane is drawn into: the picture itself, and the multisampled colour and depth
/// behind it. Make one per pane and remake it when the pane's size changes.
#[derive(Debug)]
pub struct PaneTarget {
    size: [u32; 2],
    colour: wgpu::Texture,
    colour_view: wgpu::TextureView,
    encoded_view: wgpu::TextureView,
    multisampled: wgpu::TextureView,
    depth: wgpu::TextureView,
}

impl PaneTarget {
    /// Textures for a pane of `size` pixels, `[width, height]` (each at least 1).
    #[must_use]
    pub fn new(gpu: &Gpu, size: [u32; 2]) -> Self {
        let size = [size[0].max(1), size[1].max(1)];
        let texture = |label, format, samples, usage, view_formats: &[wgpu::TextureFormat]| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats,
            })
        };
        use wgpu::TextureUsages as U;
        let colour = texture(
            "pane",
            COLOUR_FORMAT,
            1,
            U::RENDER_ATTACHMENT | U::TEXTURE_BINDING | U::COPY_SRC,
            &[ENCODED_FORMAT],
        );
        let encoded = wgpu::TextureViewDescriptor { format: Some(ENCODED_FORMAT), ..Default::default() };
        let view = wgpu::TextureViewDescriptor::default();
        Self {
            size,
            colour_view: colour.create_view(&view),
            encoded_view: colour.create_view(&encoded),
            multisampled: texture("pane, multisampled", COLOUR_FORMAT, SAMPLES, U::RENDER_ATTACHMENT, &[])
                .create_view(&view),
            depth: texture("pane, depth", DEPTH_FORMAT, SAMPLES, U::RENDER_ATTACHMENT, &[])
                .create_view(&view),
            colour,
        }
    }

    /// The pane's size in pixels, `[width, height]`.
    #[must_use]
    pub const fn size(&self) -> [u32; 2] {
        self.size
    }

    /// The finished picture, for showing in a UI.
    #[must_use]
    pub const fn view(&self) -> &wgpu::TextureView {
        &self.colour_view
    }

    /// The finished picture with its bytes read as they are stored (sRGB-encoded), not decoded to
    /// linear light. This is the view a UI wants: egui treats texture values as already encoded,
    /// so giving it the decoding view would darken the picture.
    #[must_use]
    pub const fn encoded_view(&self) -> &wgpu::TextureView {
        &self.encoded_view
    }

    /// The finished picture's texture, for reading back.
    #[must_use]
    pub const fn texture(&self) -> &wgpu::Texture {
        &self.colour
    }
}

/// The pipelines. Make one per device and keep it.
#[derive(Debug)]
pub struct Renderer {
    globals_layout: wgpu::BindGroupLayout,
    mesh: wgpu::RenderPipeline,
    mesh_xray: wgpu::RenderPipeline,
    line: wgpu::RenderPipeline,
    line_xray: wgpu::RenderPipeline,
    ground: wgpu::RenderPipeline,
}

impl Renderer {
    /// Compiles the shaders and builds the pipelines.
    #[must_use]
    pub fn new(gpu: &Gpu) -> Self {
        let device = &gpu.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cavendish"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders.wgsl").into()),
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });
        let mesh_buffer = wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4],
        };
        let line_buffer = wgpu::VertexBufferLayout {
            array_stride: size_of::<Line>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
        };
        // X-ray: additive and see-through, with no depth test. Everything else: opaque with depth.
        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        };
        let pipeline = |label: &str,
                        vertex: &str,
                        fragment: &str,
                        buffer: Option<&wgpu::VertexBufferLayout>,
                        xray: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vertex),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[buffer.cloned()],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(!xray),
                    depth_compare: Some(if xray {
                        wgpu::CompareFunction::Always
                    } else {
                        wgpu::CompareFunction::LessEqual
                    }),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState { count: SAMPLES, ..Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOUR_FORMAT,
                        blend: Some(if xray { additive } else { wgpu::BlendState::REPLACE }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            mesh: pipeline("mesh", "vs_mesh", "fs_mesh", Some(&mesh_buffer), false),
            mesh_xray: pipeline("mesh, x-ray", "vs_mesh", "fs_mesh", Some(&mesh_buffer), true),
            line: pipeline("line", "vs_line", "fs_line", Some(&line_buffer), false),
            line_xray: pipeline("line, x-ray", "vs_line", "fs_line", Some(&line_buffer), true),
            ground: pipeline("ground", "vs_ground", "fs_ground", None, false),
            globals_layout,
        }
    }

    /// Draws `frame` into `target`. Returns when the commands are submitted, not when the GPU has
    /// finished; reading the texture back waits for it.
    ///
    /// # Errors
    /// When the scene needs a buffer larger than the device allows, or the GPU refuses the work.
    /// Nothing is drawn then; the target keeps its last picture.
    pub fn draw(&self, gpu: &Gpu, target: &PaneTarget, frame: Frame<'_>) -> Result<(), String> {
        let [width, height] = target.size;
        let camera = Camera::new(frame.view, f64::from(width) / f64::from(height));
        let things = crate::scene::drawables_with(
            frame.scene,
            frame.view.style,
            frame.highlight,
            camera.origin,
            frame.time,
            frame.voxels,
        );
        check_sizes(&things, gpu.device.limits().max_buffer_size)?;
        // A validation error inside wgpu would otherwise be a panic; here it is an answer.
        gpu.checked(|| self.draw_things(gpu, target, &camera, &things, frame.view.style))
    }

    fn draw_things(
        &self,
        gpu: &Gpu,
        target: &PaneTarget,
        camera: &Camera,
        things: &Drawables,
        style: DrawStyle,
    ) {
        let [width, height] = target.size;
        let xray = style == DrawStyle::XRay;
        let lit = style == DrawStyle::Shaded;
        // The ground fades out over a range that grows with the camera's distance.
        let range = (frame_range(camera)).min(camera.far * 0.9) as f32;
        let ground = things.ground;
        let [r, g, b] = things.background;
        let eye = camera.eye.to_f32();
        let globals = Globals {
            view_proj: camera.view_proj.to_f32(),
            eye_lit: [eye[0], eye[1], eye[2], if lit { 1.0 } else { 0.0 }],
            viewport_ground: [width as f32, height as f32, ground.map_or(0.0, |g| g.height), range],
            phase: ground.map_or([0.0; 4], |g| [g.phase[0], g.phase[1], 0.0, 0.0]),
            ground_surface: ground.map_or([0.0; 4], |g| g.surface),
            ground_line: ground.map_or([0.0; 4], |g| g.line),
            background: [r, g, b, 1.0],
        };
        let device = &gpu.device;
        let buffer = |label, contents: &[u8], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        let globals_buffer = buffer("globals", bytemuck::bytes_of(&globals), wgpu::BufferUsages::UNIFORM);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &self.globals_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals_buffer.as_entire_binding() }],
        });
        let triangles = (!things.triangles.is_empty()).then(|| {
            buffer("triangles", bytemuck::cast_slice(&things.triangles), wgpu::BufferUsages::VERTEX)
        });
        let lines = (!things.lines.is_empty())
            .then(|| buffer("lines", bytemuck::cast_slice(&things.lines), wgpu::BufferUsages::VERTEX));

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pane") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.multisampled,
                    depth_slice: None,
                    resolve_target: Some(&target.colour_view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: r.into(),
                            g: g.into(),
                            b: b.into(),
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &target.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, &bind_group, &[]);
            if ground.is_some() {
                pass.set_pipeline(&self.ground);
                pass.draw(0..6, 0..1);
            }
            if let Some(triangles) = &triangles {
                pass.set_pipeline(if xray { &self.mesh_xray } else { &self.mesh });
                pass.set_vertex_buffer(0, triangles.slice(..));
                pass.draw(0..things.triangles.len() as u32, 0..1);
            }
            if let Some(lines) = &lines {
                pass.set_pipeline(if xray { &self.line_xray } else { &self.line });
                pass.set_vertex_buffer(0, lines.slice(..));
                pass.draw(0..6, 0..things.lines.len() as u32);
            }
        }
        gpu.queue.submit([encoder.finish()]);
    }

    /// Draws `frame` at `size` pixels (`[width, height]`) and reads the picture back:
    /// `width × height × 4` bytes, RGBA8, sRGB-encoded, rows top to bottom.
    ///
    /// # Errors
    /// When the read-back fails.
    pub fn render_rgba(&self, gpu: &Gpu, frame: Frame<'_>, size: [u32; 2]) -> Result<Vec<u8>, String> {
        let target = PaneTarget::new(gpu, size);
        self.draw(gpu, &target, frame)?;
        gpu.read_rgba8(target.texture())
    }

    /// Draws `frame` at `size` pixels (`[width, height]`) and encodes it as a PNG.
    ///
    /// # Errors
    /// When the read-back or the encoding fails.
    pub fn render_png(&self, gpu: &Gpu, frame: Frame<'_>, size: [u32; 2]) -> Result<Vec<u8>, String> {
        let size = [size[0].max(1), size[1].max(1)];
        encode_png(&self.render_rgba(gpu, frame, size)?, size)
    }
}

/// Refuses geometry that would not fit in one buffer of `max_buffer_size` bytes.
fn check_sizes(things: &Drawables, max_buffer_size: u64) -> Result<(), String> {
    let buffers = [
        ("lines", things.lines.len() as u64 * size_of::<Line>() as u64),
        ("triangles", things.triangles.len() as u64 * size_of::<Vertex>() as u64),
    ];
    for (what, bytes) in buffers {
        if bytes > max_buffer_size {
            return Err(format!(
                "the scene's {what} need {bytes} bytes in one buffer; this device allows {max_buffer_size}"
            ));
        }
    }
    Ok(())
}

/// How far the ground grid reaches before it has faded into the background, metres.
fn frame_range(camera: &Camera) -> f64 {
    (camera.eye.length() * 25.0).max(300.0)
}

/// Encodes RGBA8 pixels (rows top to bottom) of a picture `size` = `[width, height]` pixels as
/// a PNG.
///
/// # Errors
/// When `rgba` is not `size[0] × size[1] × 4` bytes, or the encoder fails.
pub fn encode_png(rgba: &[u8], size: [u32; 2]) -> Result<Vec<u8>, String> {
    if rgba.len() != (size[0] as usize) * (size[1] as usize) * 4 {
        return Err(format!("{} bytes is not a {} × {} RGBA picture", rgba.len(), size[0], size[1]));
    }
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, size[0], size[1]);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| format!("PNG header: {e}"))?;
    writer.write_image_data(rgba).map_err(|e| format!("PNG data: {e}"))?;
    writer.finish().map_err(|e| format!("PNG end: {e}"))?;
    Ok(out)
}

/// Decodes a PNG to `(pixels, size)`: RGBA8 bytes, rows top to bottom, and `[width, height]` in
/// pixels. For tests and for checking what was written.
///
/// # Errors
/// When the bytes are not a PNG of 8-bit RGBA.
pub fn decode_png(bytes: &[u8]) -> Result<(Vec<u8>, [u32; 2]), String> {
    let mut reader =
        png::Decoder::new(std::io::Cursor::new(bytes)).read_info().map_err(|e| format!("not a PNG: {e}"))?;
    let size = reader.output_buffer_size().ok_or("the PNG is too large")?;
    let mut pixels = vec![0; size];
    let info = reader.next_frame(&mut pixels).map_err(|e| format!("PNG data: {e}"))?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(format!("expected 8-bit RGBA, found {:?} {:?}", info.color_type, info.bit_depth));
    }
    pixels.truncate(info.buffer_size());
    Ok((pixels, [info.width, info.height]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_too_big_for_a_buffer_is_refused_before_it_is_made() {
        let line = Line { a: [0.0; 3], width: 1.0, b: [1.0; 3], _pad: 0.0, colour: [1.0; 4] };
        let things = Drawables { lines: vec![line; 100], ..Drawables::default() };
        // 100 lines of 48 bytes fit in 4800 bytes and not in 4799.
        assert_eq!(check_sizes(&things, 4800), Ok(()));
        let why = check_sizes(&things, 4799).unwrap_err();
        assert!(why.contains("lines need 4800 bytes") && why.contains("allows 4799"), "{why}");
    }

    #[test]
    fn png_round_trips_pixels() {
        let size = [5, 3];
        let rgba: Vec<u8> = (0..5 * 3 * 4).map(|i| (i * 7 % 256) as u8).collect();
        let png = encode_png(&rgba, size).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(decode_png(&png).unwrap(), (rgba.clone(), size));
        // Control: a changed pixel is a changed picture, and a wrong size is refused.
        let mut other = rgba.clone();
        other[13] ^= 1;
        assert_ne!(decode_png(&encode_png(&other, size).unwrap()).unwrap().0, rgba);
        assert!(encode_png(&rgba, [5, 4]).is_err());
        assert!(decode_png(b"not a png").is_err());
    }
}
