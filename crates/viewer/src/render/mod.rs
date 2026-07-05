//! The 3D scene renderer: instanced **shaded cubes** for the voxels and depth-independent **lines**
//! for wireframe overlays (detector cages, the body's oriented box, the spin-axis arrow, the world
//! axes). Written against **borrowed** `&Device`/`&Queue` — never owning them and never naming
//! `compute::Gpu` — so the same renderer runs against eframe's window device (live) and compute's
//! shared device (the headless test). "One graphics stack" is honoured by the single `wgpu 22` in the
//! graph, not by injecting one device.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::camera::Camera;

/// One voxel cube: a world-frame centre, a half-extent, and a colour. Drawn as a solid, depth-tested,
/// Lambert-shaded box so the cloud reads as a body rather than a sprite haze.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Cube {
    pub center: [f32; 3],
    pub half: f32,
    pub colour: [f32; 3],
    pub _pad: f32,
}

/// One vertex of a wireframe line segment (`LineList`: vertices come in pairs).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LineVert {
    pub pos: [f32; 3],
    pub colour: [f32; 3],
}

/// A text label anchored at a world point — **not** drawn by wgpu; the App projects `at` through the
/// camera and paints the text with egui (crisp glyphs without a font atlas here).
#[derive(Clone, Debug)]
pub struct Label {
    pub at: [f32; 3],
    pub text: String,
    pub colour: [f32; 3],
}

/// The drawables for one frame — assembled on the CPU from the bundle and gizmos, uploaded per frame.
#[derive(Clone, Debug, Default)]
pub struct SceneData {
    pub cubes: Vec<Cube>,
    pub lines: Vec<LineVert>,
    pub labels: Vec<Label>,
}

/// The 12 edges of a box, as index pairs into an 8-corner array ordered `i = x + 2y + 4z` (each of
/// x/y/z either the min, bit 0, or the max, bit 1). Edges join corners differing in exactly one bit.
pub const BOX_EDGES: [(usize, usize); 12] = [
    (0, 1),
    (2, 3),
    (4, 5),
    (6, 7), // along x
    (0, 2),
    (1, 3),
    (4, 6),
    (5, 7), // along y
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7), // along z
];

impl SceneData {
    pub fn new() -> Self {
        SceneData::default()
    }

    /// Add one voxel cube (half-extent in world units).
    pub fn push_cube(&mut self, center: [f64; 3], half: f32, colour: [f32; 3]) {
        self.cubes.push(Cube {
            center: [center[0] as f32, center[1] as f32, center[2] as f32],
            half,
            colour,
            _pad: 0.0,
        });
    }

    /// Add a cube for each cloud vertex — the voxel body.
    pub fn push_cubes(&mut self, centres: &[[f64; 3]], half: f32, colour: [f32; 3]) {
        for &c in centres {
            self.push_cube(c, half, colour);
        }
    }

    /// Add one line segment `a → b`.
    pub fn push_line(&mut self, a: [f64; 3], b: [f64; 3], colour: [f32; 3]) {
        let v = |p: [f64; 3]| LineVert {
            pos: [p[0] as f32, p[1] as f32, p[2] as f32],
            colour,
        };
        self.lines.push(v(a));
        self.lines.push(v(b));
    }

    /// Add the 12 edges of a box from its 8 corners (order `i = x + 2y + 4z`, see [`BOX_EDGES`]).
    pub fn push_box_wireframe(&mut self, corners: &[[f64; 3]; 8], colour: [f32; 3]) {
        for (a, b) in BOX_EDGES {
            self.push_line(corners[a], corners[b], colour);
        }
    }

    /// Add an arrow `from → to`: a shaft plus a small four-segment head at the tip.
    pub fn push_arrow(&mut self, from: [f64; 3], to: [f64; 3], colour: [f32; 3]) {
        self.push_line(from, to, colour);
        let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if len <= 0.0 {
            return;
        }
        let dir = [d[0] / len, d[1] / len, d[2] / len];
        // A perpendicular via the least-aligned world axis, and a second one by the cross product.
        let up = if dir[2].abs() < 0.9 {
            [0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let p1 = normalise(cross(dir, up));
        let p2 = normalise(cross(dir, p1));
        let h = 0.18 * len; // head length
        let w = 0.09 * len; // head half-width
        let base = [to[0] - dir[0] * h, to[1] - dir[1] * h, to[2] - dir[2] * h];
        for perp in [p1, p2] {
            for s in [-1.0, 1.0] {
                let corner = [
                    base[0] + perp[0] * w * s,
                    base[1] + perp[1] * w * s,
                    base[2] + perp[2] * w * s,
                ];
                self.push_line(to, corner, colour);
            }
        }
    }

    /// Anchor a text label at a world point.
    pub fn push_label(&mut self, at: [f64; 3], text: impl Into<String>, colour: [f32; 3]) {
        self.labels.push(Label {
            at: [at[0] as f32, at[1] as f32, at[2] as f32],
            text: text.into(),
            colour,
        });
    }
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalise(a: [f64; 3]) -> [f64; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if l > 0.0 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        a
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniform {
    view_proj: [[f32; 4]; 4],
}

/// One vertex of the unit cube mesh: position in `[-1, 1]³` and its face normal.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CubeVert {
    pos: [f32; 3],
    normal: [f32; 3],
}

/// The 36 vertices (6 faces × 2 triangles) of a unit cube spanning `[-1, 1]³`, with per-face normals,
/// wound counter-clockwise seen from outside (so back-face culling keeps the outer faces).
fn unit_cube() -> Vec<CubeVert> {
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]), // +x
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]), // −x
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]), // +y
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]), // −y
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), // +z
        ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]), // −z
    ];
    let mut verts = Vec::with_capacity(36);
    for (normal, u, v) in faces {
        // The face centre is `normal`; its four corners are `normal ± u ± v`.
        let corner = |su: f32, sv: f32| CubeVert {
            pos: [
                normal[0] + u[0] * su + v[0] * sv,
                normal[1] + u[1] * su + v[1] * sv,
                normal[2] + u[2] * su + v[2] * sv,
            ],
            normal,
        };
        let (a, b, c, d) = (
            corner(-1.0, -1.0),
            corner(1.0, -1.0),
            corner(1.0, 1.0),
            corner(-1.0, 1.0),
        );
        verts.extend([a, b, c, a, c, d]);
    }
    verts
}

const SHADER: &str = r#"
struct Camera { view_proj: mat4x4<f32> };
@group(0) @binding(0) var<uniform> camera: Camera;

// ── Voxel cubes: instanced, Lambert-shaded, depth-tested. ──
struct CubeOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) colour: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

@vertex
fn vs_cube(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>,
           @location(2) center: vec3<f32>, @location(3) half: f32,
           @location(4) colour: vec3<f32>) -> CubeOut {
    var out: CubeOut;
    out.clip = camera.view_proj * vec4<f32>(center + pos * half, 1.0);
    out.colour = colour;
    out.normal = normal;
    return out;
}

@fragment
fn fs_cube(in: CubeOut) -> @location(0) vec4<f32> {
    let light = normalize(vec3<f32>(0.4, 0.7, 0.6));
    let d = max(dot(normalize(in.normal), light), 0.0);
    return vec4<f32>(in.colour * (0.3 + 0.7 * d), 1.0);
}

// ── Wireframe lines: depth-independent (drawn over the cubes). ──
struct LineOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) colour: vec3<f32>,
};

@vertex
fn vs_line(@location(0) pos: vec3<f32>, @location(1) colour: vec3<f32>) -> LineOut {
    var out: LineOut;
    out.clip = camera.view_proj * vec4<f32>(pos, 1.0);
    out.colour = colour;
    return out;
}

@fragment
fn fs_line(in: LineOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.colour, 1.0);
}
"#;

/// The scene's clear colour — a dark slate, distinct from any drawn geometry (so the headless render
/// test can tell "rasterised geometry" from "merely cleared").
pub const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.02,
    g: 0.02,
    b: 0.05,
    a: 1.0,
};

/// The depth format the offscreen target must match.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// The 3D renderer: the cube and line pipelines, the camera uniform, and the shared cube mesh — all
/// bound to a borrowed device.
pub struct SceneRenderer {
    cube_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    cube_mesh: wgpu::Buffer,
    cube_verts: u32,
}

impl SceneRenderer {
    /// Build both pipelines for a colour target of `format` (and the fixed depth format) on a borrowed
    /// device.
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

        let cube_verts_data = unit_cube();
        let cube_mesh = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("viewer.scene.cube"),
            contents: bytemuck::cast_slice(&cube_verts_data),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let cube_vert_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CubeVert>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
        };
        let cube_inst_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Cube>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![2 => Float32x3, 3 => Float32, 4 => Float32x3],
        };
        let line_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<LineVert>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
        };

        let target = wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        };

        // Cubes: depth test + write, back-face culling for solidity.
        let cube_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("viewer.scene.cube"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_cube",
                buffers: &[cube_vert_layout, cube_inst_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_cube",
                targets: &[Some(target.clone())],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Lines: depth test disabled (always drawn), so wireframe shows through the cubes.
        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("viewer.scene.line"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_line",
                buffers: &[line_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_line",
                targets: &[Some(target)],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        SceneRenderer {
            cube_pipeline,
            line_pipeline,
            bind_group,
            uniform,
            cube_mesh,
            cube_verts: cube_verts_data.len() as u32,
        }
    }

    /// Clear `colour`/`depth` and draw `scene` under `camera`: shaded cubes first, then wireframe lines
    /// over them. An empty scene draws only the clear — the degenerate case renders, it does not panic.
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        colour: &wgpu::TextureView,
        depth: &wgpu::TextureView,
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
        let cubes = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("viewer.scene.cubes"),
            contents: bytemuck::cast_slice(&scene.cubes),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let lines = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("viewer.scene.lines"),
            contents: bytemuck::cast_slice(&scene.lines),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("viewer.scene"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewer.scene.pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: colour,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(CLEAR),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            if !scene.cubes.is_empty() {
                pass.set_pipeline(&self.cube_pipeline);
                pass.set_vertex_buffer(0, self.cube_mesh.slice(..));
                pass.set_vertex_buffer(1, cubes.slice(..));
                pass.draw(0..self.cube_verts, 0..scene.cubes.len() as u32);
            }
            if !scene.lines.is_empty() {
                pass.set_pipeline(&self.line_pipeline);
                pass.set_vertex_buffer(0, lines.slice(..));
                pass.draw(0..scene.lines.len() as u32, 0..1);
            }
        }
        queue.submit(std::iter::once(encoder.finish()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compute::Gpu;

    #[test]
    fn box_wireframe_is_twelve_edges() {
        let mut scene = SceneData::new();
        let corners =
            std::array::from_fn(|i| [(i & 1) as f64, ((i >> 1) & 1) as f64, (i >> 2) as f64]);
        scene.push_box_wireframe(&corners, [1.0, 1.0, 1.0]);
        assert_eq!(scene.lines.len(), 24, "12 edges = 24 line vertices");
    }

    #[test]
    #[ignore = "requires a GPU device (run in the gpu CI job / locally on Metal)"]
    fn renders_headless() {
        // One frame renders to an offscreen colour+depth target under lavapipe (the gpu CI job) without
        // panic, and ACTUALLY rasterises geometry: at least one pixel differs from the cleared corner.
        // Sharing compute's device proves the borrowed-device renderer runs on the one graphics stack.
        let gpu = Gpu::new().expect("acquire device");
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let n = 256u32;
        let colour = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("headless.colour"),
            size: wgpu::Extent3d {
                width: n,
                height: n,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("headless.depth"),
            size: wgpu::Extent3d {
                width: n,
                height: n,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let colour_view = colour.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());

        let renderer = SceneRenderer::new(&gpu.device, format);
        let mut scene = SceneData::new();
        scene.push_cubes(&[[0.0, 0.0, 0.0], [0.4, 0.0, 0.0]], 0.2, [1.0, 1.0, 1.0]);
        scene.push_arrow([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        renderer.render(
            &gpu.device,
            &gpu.queue,
            &colour_view,
            &depth_view,
            &Camera::default(),
            &scene,
        );

        // Read the colour texture back (256-byte row alignment, like compute's buffer readback).
        let bpp = 4u32;
        let unpadded = n * bpp;
        let padded = unpadded.div_ceil(256) * 256;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headless.readback"),
            size: (padded * n) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &colour,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(n),
                },
            },
            wgpu::Extent3d {
                width: n,
                height: n,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit(std::iter::once(encoder.finish()));

        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        gpu.device.poll(wgpu::Maintain::Wait);
        let data = slice.get_mapped_range();

        let corner = [data[0], data[1], data[2], data[3]];
        let mut differs = false;
        for y in 0..n as usize {
            let row = &data[y * padded as usize..y * padded as usize + unpadded as usize];
            if row.chunks_exact(4).any(|px| px != corner) {
                differs = true;
                break;
            }
        }
        assert!(differs, "frame is uniform — geometry did not rasterise");
    }
}
