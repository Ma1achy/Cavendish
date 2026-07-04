//! The 3D scene renderer: instanced billboard quads for the cloud, the array markers, and the spin
//! axis. Written against **borrowed** `&Device`/`&Queue` — never owning them and never naming
//! `compute::Gpu` — so the same renderer runs against eframe's window device (live) and compute's
//! shared device (the headless test). "One graphics stack" is honoured by the single `wgpu 22` in the
//! graph, not by injecting one device.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::camera::Camera;

/// One drawn billboard: a world position, a screen-ish size, and a colour.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Instance {
    pub pos: [f32; 3],
    pub size: f32,
    pub colour: [f32; 3],
    pub _pad: f32,
}

impl Instance {
    fn new(pos: [f64; 3], size: f32, colour: [f32; 3]) -> Self {
        Instance {
            pos: [pos[0] as f32, pos[1] as f32, pos[2] as f32],
            size,
            colour,
            _pad: 0.0,
        }
    }
}

/// The instances to draw this frame — assembled on the CPU from the bundle, uploaded per frame.
#[derive(Clone, Debug, Default)]
pub struct SceneData {
    pub instances: Vec<Instance>,
}

impl SceneData {
    pub fn new() -> Self {
        SceneData::default()
    }

    /// Add the cloud's world-frame vertices as small billboards.
    pub fn push_points(&mut self, verts: &[[f64; 3]], size: f32, colour: [f32; 3]) {
        self.instances
            .extend(verts.iter().map(|&v| Instance::new(v, size, colour)));
    }

    /// Add one marker (a detector, or the tip of the spin axis).
    pub fn push_marker(&mut self, pos: [f64; 3], size: f32, colour: [f32; 3]) {
        self.instances.push(Instance::new(pos, size, colour));
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniform {
    view_proj: [[f32; 4]; 4],
}

/// A unit quad (two triangles) in the billboard's local `[-1, 1]²` frame.
const QUAD: [[f32; 2]; 6] = [
    [-1.0, -1.0],
    [1.0, -1.0],
    [1.0, 1.0],
    [-1.0, -1.0],
    [1.0, 1.0],
    [-1.0, 1.0],
];

const SHADER: &str = r#"
struct Camera { view_proj: mat4x4<f32> };
@group(0) @binding(0) var<uniform> camera: Camera;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) colour: vec3<f32>,
};

@vertex
fn vs_main(@location(0) corner: vec2<f32>,
           @location(1) pos: vec3<f32>,
           @location(2) size: f32,
           @location(3) colour: vec3<f32>) -> VsOut {
    var out: VsOut;
    let c = camera.view_proj * vec4<f32>(pos, 1.0);
    // Offset the quad corner in clip space, scaled by w so the billboard keeps a roughly constant size.
    out.clip = vec4<f32>(c.xy + corner * size * c.w, c.z, c.w);
    out.colour = colour;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.colour, 1.0);
}
"#;

/// The scene's clear colour — a dark slate, distinct from any drawn billboard (so the headless render
/// test can tell "rasterised geometry" from "merely cleared").
pub const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.02,
    g: 0.02,
    b: 0.05,
    a: 1.0,
};

/// The 3D renderer: a pipeline, a camera uniform, and the shared quad — all bound to a borrowed device.
pub struct SceneRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    quad: wgpu::Buffer,
}

impl SceneRenderer {
    /// Build the pipeline for a colour target of `format` on a borrowed device.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viewer.scene.shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("viewer.scene.bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewer.scene.uniform"),
            size: std::mem::size_of::<Uniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewer.scene.bg"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("viewer.scene.layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let quad = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("viewer.scene.quad"),
            contents: bytemuck::cast_slice(&QUAD),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let corner_layout = wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x2],
        };
        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![1 => Float32x3, 2 => Float32, 3 => Float32x3],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("viewer.scene.pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[corner_layout, instance_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        SceneRenderer {
            pipeline,
            bind_group,
            uniform,
            quad,
        }
    }

    /// Clear `view` to [`CLEAR`] and draw `scene` under `camera`. An empty scene draws only the clear —
    /// the degenerate case renders, it does not panic.
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        camera: &Camera,
        scene: &SceneData,
    ) {
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::cast_slice(&[Uniform {
                view_proj: camera.view_proj(),
            }]),
        );
        let instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("viewer.scene.instances"),
            contents: bytemuck::cast_slice(&scene.instances),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("viewer.scene"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewer.scene.pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(CLEAR),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if !scene.instances.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.quad.slice(..));
                pass.set_vertex_buffer(1, instances.slice(..));
                pass.draw(0..QUAD.len() as u32, 0..scene.instances.len() as u32);
            }
        }
        queue.submit(std::iter::once(encoder.finish()));
    }
}
