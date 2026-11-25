//! The world volume on the GPU (S4, `plan/REBUILD.md` §5): the near field as greedy-meshed
//! chunks, culled by a compute pass and drawn with indirect draws; the far field as the clipmap's
//! bricks, each ray-marched inside its box. What is drawn is what the camera's clipmap wants of
//! what the engine has made resident ([`WorldFrame`]); this module keeps the GPU copies.
//!
//! **Kernels and their CPU twins.** The culling test is [`visible`], the march is [`march`]: the
//! same steps in `f32` as `world_cull.wgsl` and `world_common.wgsl`, held to the GPU by
//! `gpu_world_culling_matches_the_cpu` and `gpu_brick_march_matches_the_cpu`.
//!
//! **In a page.** Everything here is core WebGPU: storage buffers read in the vertex and fragment
//! stages, one `draw_indexed_indirect` a chunk (no multi-draw, no `first_instance`, neither of which
//! is core), a dynamic-free bind group, `frag_depth`. The largest binding is the brick store,
//! [`MAX_BRICK_SLOTS`] × [`BRICK_WORDS`] × 4 bytes = 34 MB, under WebGPU's 128 MiB default.
//!
//! **Bounded** (`docs/slices/S4.md` §3.1, rows W7 to W9): at most [`MAX_CHUNK_SLOTS`] meshes and
//! [`MAX_WORLD_VERTICES`] vertices on the GPU, [`MAX_BRICK_SLOTS`] bricks, [`MAX_MARCH_STEPS`] steps
//! a ray, [`MAX_MESHES_PER_DRAW`] meshes made a frame in a window (all of them for a picture taken
//! once). What does not fit is not drawn, and [`WorldGpu::notes`] says so.

use std::collections::HashMap;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use cav_core::maths::Mat4;
use cav_core::voxel::world::{BRICK_SIDE, Brick, CHUNK_SIDE, ChunkCells, ChunkKey, cell_index};
use cav_gpu::Gpu;
use cav_gpu::wgpu::{self, util::DeviceExt};
use cav_sim::ViewSpec;
use cav_sim::world::clipmap::{WorldFrame, levels, lod_centre};

use crate::camera::Camera;

/// The most chunk meshes on the GPU at once.
pub const MAX_CHUNK_SLOTS: usize = 8192;
/// The most bricks on the GPU at once.
pub const MAX_BRICK_SLOTS: usize = 8192;
/// The most vertices of chunk meshes on the GPU at once (12 bytes each).
pub const MAX_WORLD_VERTICES: usize = 12 << 20;
/// The most chunk meshes made in one frame of a window (a picture taken once makes them all).
pub const MAX_MESHES_PER_DRAW: usize = 96;
/// The most sub-blocks and cells one ray visits: 10 sub-blocks and 22 cells in each, and a margin.
pub const MAX_MARCH_STEPS: u32 = 256;
/// The words of one brick on the GPU: 32 768 occupancy bits, the 64-bit sub-block mask, and 64
/// sub-block materials four to a word.
pub const BRICK_WORDS: usize = 1042;

/// One vertex of a chunk's mesh: its corner in cells from the chunk's origin and the chunk's
/// slot (for its origin), then the face it is on and its world material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
#[repr(C)]
pub struct ChunkVertex {
    /// `[x, y, z, slot]`: cells from the chunk's corner (0 to 32), and the chunk's slot.
    pub cell: [u16; 4],
    /// `[face, material, 0, 0]`: the face (0 −x, 1 +x, 2 −y, 3 +y, 4 −z, 5 +z), the world material.
    pub attributes: [u8; 4],
}

/// A chunk's mesh: its vertices, and its triangles as indices into them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mesh {
    /// The vertices.
    pub vertices: Vec<ChunkVertex>,
    /// Three a triangle.
    pub indices: Vec<u32>,
}

/// The unit step along axis `axis` (0 x, 1 y, 2 z).
const fn along(axis: usize) -> [i32; 3] {
    let mut v = [0; 3];
    v[axis] = 1;
    v
}

/// The chunk's exposed faces as rectangles, merged greedily per slice and material: a face of an
/// occupied cell is exposed when the cell beyond it is empty, looked up in the neighbouring chunk
/// across the chunk's side (`neighbours`, in face order −x, +x, −y, +y, −z, +z; a missing one is
/// air). `materials` is the chunk's cells as [`ChunkCells::materials`] gives them (material + 1,
/// 0 empty); every vertex carries `slot`.
#[must_use]
pub fn greedy_mesh(materials: &[u8], neighbours: [Option<&[u8]>; 6], slot: u16) -> Mesh {
    let n = CHUNK_SIDE as i32;
    let at = |c: [i32; 3]| -> u8 {
        let outside = (0..3).find(|&a| c[a] < 0 || c[a] >= n);
        match outside {
            None => materials[cell_index(c.map(|v| v as u32))],
            Some(a) => {
                let face = 2 * a + usize::from(c[a] >= n);
                let mut d = c;
                d[a] = d[a].rem_euclid(n);
                neighbours[face].map_or(0, |cells| cells[cell_index(d.map(|v| v as u32))])
            }
        }
    };
    let mut mesh = Mesh::default();
    let mut mask = vec![0u8; (n * n) as usize];
    for face in 0..6usize {
        let axis = face / 2;
        let positive = face % 2 == 1;
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        let step = along(axis);
        for slice in 0..n {
            // Which faces of this slice are exposed, by material.
            for b in 0..n {
                for a in 0..n {
                    let mut c = [0; 3];
                    c[axis] = slice;
                    c[u] = a;
                    c[v] = b;
                    let here = at(c);
                    let sign = if positive { 1 } else { -1 };
                    let beyond = [c[0] + sign * step[0], c[1] + sign * step[1], c[2] + sign * step[2]];
                    mask[(a + n * b) as usize] = if here != 0 && at(beyond) == 0 { here } else { 0 };
                }
            }
            // Greedy rectangles: widest first along u, then as tall as the row allows along v.
            for b in 0..n {
                let mut a = 0;
                while a < n {
                    let m = mask[(a + n * b) as usize];
                    if m == 0 {
                        a += 1;
                        continue;
                    }
                    let mut width = 1;
                    while a + width < n && mask[(a + width + n * b) as usize] == m {
                        width += 1;
                    }
                    let mut height = 1;
                    'grow: while b + height < n {
                        for k in 0..width {
                            if mask[(a + k + n * (b + height)) as usize] != m {
                                break 'grow;
                            }
                        }
                        height += 1;
                    }
                    for h in 0..height {
                        for k in 0..width {
                            mask[(a + k + n * (b + h)) as usize] = 0;
                        }
                    }
                    let plane = slice + i32::from(positive);
                    let corner = |du: i32, dv: i32| {
                        let mut p = [0u16; 4];
                        p[axis] = plane as u16;
                        p[u] = (a + du) as u16;
                        p[v] = (b + dv) as u16;
                        p[3] = slot;
                        ChunkVertex { cell: p, attributes: [face as u8, m - 1, 0, 0] }
                    };
                    let first = mesh.vertices.len() as u32;
                    mesh.vertices.extend([
                        corner(0, 0),
                        corner(width, 0),
                        corner(width, height),
                        corner(0, height),
                    ]);
                    mesh.indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
                    a += width;
                }
            }
        }
    }
    mesh
}

/// The frustum's six planes, `[a, b, c, d]` with `a x + b y + c z + d ≥ 0` inside, in the frame
/// `view_proj` takes (offsets from the view's target), each normalised; from the matrix's rows
/// (Gribb and Hartmann), for wgpu's clip depth from 0 to 1.
#[must_use]
pub fn frustum_planes(view_proj: &Mat4) -> [[f64; 4]; 6] {
    let row = |r: usize| [view_proj.m[0][r], view_proj.m[1][r], view_proj.m[2][r], view_proj.m[3][r]];
    let (r0, r1, r2, r3) = (row(0), row(1), row(2), row(3));
    let add = |a: [f64; 4], b: [f64; 4], s: f64| {
        [a[0] + s * b[0], a[1] + s * b[1], a[2] + s * b[2], a[3] + s * b[3]]
    };
    let planes =
        [add(r3, r0, 1.0), add(r3, r0, -1.0), add(r3, r1, 1.0), add(r3, r1, -1.0), r2, add(r3, r2, -1.0)];
    planes.map(|p| {
        let length = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt().max(1e-300);
        p.map(|v| v / length)
    })
}

/// Whether the box from `lo` of side `side` (metres from the target) is on the inner side of every
/// plane: its corner farthest along each plane's normal is tested. Conservative (a box near a
/// frustum's corner may be kept). The CPU twin of `world_cull.wgsl`'s `box_visible`, in `f32`.
#[must_use]
pub fn visible(planes: &[[f32; 4]; 6], lo: [f32; 3], side: f32) -> bool {
    planes.iter().all(|p| {
        let far = [0, 1, 2].map(|a| lo[a] + if p[a] > 0.0 { side } else { 0.0 });
        p[0] * far[0] + p[1] * far[1] + p[2] * far[2] + p[3] >= 0.0
    })
}

/// A brick as the GPU holds it: [`BRICK_WORDS`] words.
#[must_use]
pub fn brick_words(brick: &Brick) -> Vec<u32> {
    let mut out = Vec::with_capacity(BRICK_WORDS);
    out.extend_from_slice(&brick.bits);
    out.push(brick.mask as u32);
    out.push((brick.mask >> 32) as u32);
    for four in brick.materials.chunks(4) {
        out.push(u32::from_le_bytes([four[0], four[1], four[2], four[3]]));
    }
    out
}

/// What a ray found in a brick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarchHit {
    /// Whether it met an occupied cell.
    pub hit: bool,
    /// Where, in units of `dir` from `start`.
    pub t: f32,
    /// The cell it met, `[x, y, z]` from 0 to 31.
    pub cell: [i32; 3],
    /// The face it came in through (0 −x, 1 +x, 2 −y, 3 +y, 4 −z, 5 +z).
    pub face: u32,
    /// Sub-blocks and cells visited.
    pub steps: u32,
}

fn face_of(axis: usize, step: i32) -> u32 {
    axis as u32 * 2 + u32::from(step <= 0)
}

fn least(v: [f32; 3]) -> usize {
    if v[0] <= v[1] && v[0] <= v[2] {
        0
    } else if v[1] <= v[2] {
        1
    } else {
        2
    }
}

fn floor_i(v: f32) -> i32 {
    v.floor() as i32
}

/// Marches `words` (one brick, [`brick_words`]) from `start` (cells, inside `[0, 32]³`) along
/// `dir` (cells per unit of t; no component zero), entering through `entry_face`: sub-block by
/// sub-block over the 4³ mask, cell by cell inside an occupied one, at most [`MAX_MARCH_STEPS`]
/// visits. The CPU twin of `march` in `world_common.wgsl`, step for step in `f32`.
#[must_use]
pub fn march(words: &[u32], start: [f32; 3], dir: [f32; 3], entry_face: u32) -> MarchHit {
    let cell_set = |c: [i32; 3]| {
        let i = (c[0] + 32 * c[1] + 1024 * c[2]) as usize;
        (words[i / 32] >> (i % 32)) & 1 == 1
    };
    let sub_set = |s: [i32; 3]| {
        let i = (s[0] + 4 * s[1] + 16 * s[2]) as usize;
        (words[1024 + i / 32] >> (i % 32)) & 1 == 1
    };
    let mut out = MarchHit { hit: false, t: 0.0, cell: [0; 3], face: entry_face, steps: 0 };
    let step = dir.map(|d| if d > 0.0 { 1 } else { -1 });
    let inv = dir.map(|d| 1.0 / d);
    let p0 = start.map(|v| v.clamp(0.0, 31.9999));
    let mut s = p0.map(|v| floor_i(v / 8.0));
    let mut t = 0.0f32;
    let mut face = entry_face;
    let mut coarse_next =
        [0, 1, 2].map(|a| ((s[a] * 8 + if dir[a] > 0.0 { 8 } else { 0 }) as f32 - start[a]) * inv[a]);
    let coarse_delta = inv.map(|v| (8.0 * v).abs());
    for _ in 0..MAX_MARCH_STEPS {
        out.steps += 1;
        if sub_set(s) {
            let here = [0, 1, 2]
                .map(|a| (start[a] + dir[a] * t).clamp((s[a] * 8) as f32, (s[a] * 8) as f32 + 7.9999));
            let mut c = here.map(floor_i);
            let mut fine_next =
                [0, 1, 2].map(|a| ((c[a] + i32::from(dir[a] > 0.0)) as f32 - start[a]) * inv[a]);
            let fine_delta = inv.map(f32::abs);
            let (mut tf, mut ff) = (t, face);
            for _ in 0..24 {
                out.steps += 1;
                if cell_set(c) {
                    return MarchHit { hit: true, t: tf, cell: c, face: ff, steps: out.steps };
                }
                let a = least(fine_next);
                tf = fine_next[a];
                fine_next[a] += fine_delta[a];
                c[a] += step[a];
                ff = face_of(a, step[a]);
                if c[a] < s[a] * 8 || c[a] >= s[a] * 8 + 8 {
                    break;
                }
            }
        }
        let a = least(coarse_next);
        t = coarse_next[a];
        coarse_next[a] += coarse_delta[a];
        s[a] += step[a];
        face = face_of(a, step[a]);
        if s[a] < 0 || s[a] >= 4 {
            return out;
        }
    }
    out
}

/// A brick drawn this frame: its corner from the target and its cell (metres), its slot and level.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
struct BrickInstance {
    origin: [f32; 4],
    info: [u32; 4],
}

/// What the world shaders read each frame.
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct WorldUniform {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    forward: [f32; 4],
    params: [f32; 4],
}

/// What the cull shader reads.
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct CullUniform {
    planes: [[f32; 4]; 6],
    sizes: [f32; 4],
}

/// One chunk's mesh on the GPU.
struct GpuMesh {
    slot: u32,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
    vertex_count: usize,
    source: Arc<ChunkCells>,
}

/// The pipelines of the world: made once per device with the renderer's.
#[derive(Debug)]
pub struct WorldPipelines {
    layout: wgpu::BindGroupLayout,
    chunk: wgpu::RenderPipeline,
    brick: wgpu::RenderPipeline,
    cull_layout: wgpu::BindGroupLayout,
    cull: wgpu::ComputePipeline,
}

/// The render module's source: its own part and the march it shares with the parity kernel.
#[must_use]
pub fn render_source() -> String {
    format!("{}\n{}", include_str!("world_render.wgsl"), include_str!("world_common.wgsl"))
}

/// The parity kernel's source: the march in a compute shader.
#[must_use]
pub fn march_source() -> String {
    format!("{}\n{}", include_str!("world_march.wgsl"), include_str!("world_common.wgsl"))
}

impl WorldPipelines {
    /// Compiles the world's shaders for targets of `format`, `samples` samples and depth
    /// `depth`.
    #[must_use]
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat, samples: u32, depth: wgpu::TextureFormat) -> Self {
        let device = &gpu.device;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("world"),
            source: wgpu::ShaderSource::Wgsl(render_source().into()),
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty,
            count: None,
        };
        let uniform = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let storage = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("world"),
            entries: &[entry(0, uniform), entry(1, uniform), entry(2, storage), entry(3, storage)],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("world"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |label: &str,
                        vs: &str,
                        fs: &str,
                        buffer: wgpu::VertexBufferLayout<'_>,
                        cull: Option<wgpu::Face>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(vs),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(buffer)],
                },
                primitive: wgpu::PrimitiveState { cull_mode: cull, ..wgpu::PrimitiveState::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: depth,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let chunk = pipeline(
            "world chunks",
            "vs_chunk",
            "fs_chunk",
            wgpu::VertexBufferLayout {
                array_stride: size_of::<ChunkVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Uint16x4, 1 => Uint8x4],
            },
            None,
        );
        // Back faces: the march starts where the ray enters the box, or at the eye inside it.
        let brick = pipeline(
            "world bricks",
            "vs_brick",
            "fs_brick",
            wgpu::VertexBufferLayout {
                array_stride: size_of::<BrickInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Uint32x4],
            },
            Some(wgpu::Face::Front),
        );
        let cull_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("world cull"),
            source: wgpu::ShaderSource::Wgsl(include_str!("world_cull.wgsl").into()),
        });
        let compute = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty,
            count: None,
        };
        let cull_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("world cull"),
            entries: &[
                compute(0, uniform),
                compute(1, storage),
                compute(2, storage),
                compute(
                    3,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
            ],
        });
        let cull = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("world cull"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("world cull"),
                bind_group_layouts: &[Some(&cull_layout)],
                immediate_size: 0,
            })),
            module: &cull_module,
            entry_point: Some("cs_cull"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        Self { layout, chunk, brick, cull_layout, cull }
    }
}

/// The world's GPU copies: chunk meshes and the brick store, keyed by what they were made from.
#[derive(Default)]
pub struct WorldGpu {
    world: u64,
    meshes: HashMap<ChunkKey, GpuMesh>,
    free_meshes: Vec<u32>,
    vertices: usize,
    bricks: HashMap<(u8, ChunkKey), (u32, Arc<Brick>)>,
    free_bricks: Vec<u32>,
    store: Option<wgpu::Buffer>,
    /// What could not be drawn this frame, and why (at most a line each).
    pub notes: Vec<String>,
}

impl std::fmt::Debug for WorldGpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorldGpu")
            .field("meshes", &self.meshes.len())
            .field("vertices", &self.vertices)
            .field("bricks", &self.bricks.len())
            .finish_non_exhaustive()
    }
}

/// One frame's world: what to cull and draw, and the buffers that say it.
pub struct WorldDraw {
    bind_group: wgpu::BindGroup,
    cull_group: Option<wgpu::BindGroup>,
    indirect: Option<wgpu::Buffer>,
    /// The chunks drawn, in the order of their indirect draws: their slots.
    chunks: Vec<ChunkKey>,
    instances: Option<wgpu::Buffer>,
    bricks: u32,
}

/// sRGB to linear light, for the palette.
fn linear(c: u8) -> f32 {
    let v = f32::from(c) / 255.0;
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

impl WorldGpu {
    /// Chunk meshes and bricks held, and vertices of the meshes.
    #[must_use]
    pub fn counts(&self) -> [usize; 3] {
        [self.meshes.len(), self.bricks.len(), self.vertices]
    }

    /// Brings the GPU copies up to `frame` for the camera of `view` (`camera` is its renderer
    /// camera) and makes this frame's buffers. `complete`: make every mesh wanted now (a picture
    /// taken once), else at most [`MAX_MESHES_PER_DRAW`].
    pub fn prepare(
        &mut self,
        gpu: &Gpu,
        pipelines: &WorldPipelines,
        frame: &WorldFrame,
        view: &ViewSpec,
        camera: &Camera,
        complete: bool,
    ) -> Option<WorldDraw> {
        let grid = frame.grid?;
        self.notes.clear();
        if self.world != frame.world {
            *self = Self { world: frame.world, store: self.store.take(), ..Self::default() };
        }
        if self.free_meshes.is_empty() && self.meshes.is_empty() {
            self.free_meshes = (0..MAX_CHUNK_SLOTS as u32).rev().collect();
            self.free_bricks = (0..MAX_BRICK_SLOTS as u32).rev().collect();
        }
        let device = &gpu.device;
        let store = self.store.get_or_insert_with(|| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("world bricks"),
                size: (MAX_BRICK_SLOTS * BRICK_WORDS * 4) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        // What this camera's clipmap draws of what is resident.
        let levels = levels(&grid, lod_centre(view), frame.ring, frame.levels.max(1));
        let near: Vec<(ChunkKey, &Arc<ChunkCells>)> =
            frame.chunks.iter().filter(|(k, _)| levels[0].contains(**k)).map(|(k, c)| (*k, c)).collect();
        let far: Vec<(&(u8, ChunkKey), &Arc<Brick>)> = frame
            .bricks
            .iter()
            .filter(|((level, key), _)| levels.get(usize::from(*level)).is_some_and(|l| l.contains(*key)))
            .collect();
        // Drop the GPU copies of what is no longer resident (or has been made again).
        let gone: Vec<ChunkKey> = self
            .meshes
            .iter()
            .filter(|(k, m)| frame.chunks.get(*k).is_none_or(|c| !Arc::ptr_eq(c, &m.source)))
            .map(|(k, _)| *k)
            .collect();
        for key in gone {
            if let Some(mesh) = self.meshes.remove(&key) {
                self.vertices -= mesh.vertex_count;
                self.free_meshes.push(mesh.slot);
            }
        }
        let gone: Vec<(u8, ChunkKey)> = self
            .bricks
            .iter()
            .filter(|(k, (_, b))| frame.bricks.get(*k).is_none_or(|c| !Arc::ptr_eq(c, b)))
            .map(|(k, _)| *k)
            .collect();
        for key in gone {
            if let Some((slot, _)) = self.bricks.remove(&key) {
                self.free_bricks.push(slot);
            }
        }
        // Make the meshes wanted and missing, nearest first by the order of the map (bounded).
        let mut made = 0;
        let mut skipped = 0;
        for (key, cells) in &near {
            if self.meshes.contains_key(key) {
                continue;
            }
            if !complete && made >= MAX_MESHES_PER_DRAW {
                skipped += 1;
                continue;
            }
            let Some(slot) = self.free_meshes.pop() else {
                self.notes
                    .push(format!("more than {MAX_CHUNK_SLOTS} chunks are near; the rest are not drawn"));
                break;
            };
            let materials = cells.materials();
            let neighbour = |d: [i32; 3]| {
                frame
                    .chunks
                    .get(&ChunkKey::new(key.x + d[0], key.y + d[1], key.z + d[2]))
                    .map(|c| c.materials())
            };
            let around = [[-1, 0, 0], [1, 0, 0], [0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1]].map(neighbour);
            let mesh = greedy_mesh(&materials, around.each_ref().map(|n| n.as_deref()), slot as u16);
            if self.vertices + mesh.vertices.len() > MAX_WORLD_VERTICES {
                self.free_meshes.push(slot);
                self.notes.push(format!(
                    "the near field's meshes reached {MAX_WORLD_VERTICES} vertices; the rest are not drawn"
                ));
                break;
            }
            made += 1;
            if mesh.indices.is_empty() {
                self.free_meshes.push(slot);
                continue;
            }
            let buffer = |label, contents: &[u8], usage| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents,
                    usage,
                })
            };
            self.vertices += mesh.vertices.len();
            self.meshes.insert(
                *key,
                GpuMesh {
                    slot,
                    vertices: buffer(
                        "chunk vertices",
                        bytemuck::cast_slice(&mesh.vertices),
                        wgpu::BufferUsages::VERTEX,
                    ),
                    indices: buffer(
                        "chunk indices",
                        bytemuck::cast_slice(&mesh.indices),
                        wgpu::BufferUsages::INDEX,
                    ),
                    count: mesh.indices.len() as u32,
                    vertex_count: mesh.vertices.len(),
                    source: Arc::clone(cells),
                },
            );
        }
        let _ = skipped;
        // Upload the bricks wanted and missing.
        for (key, brick) in &far {
            if self.bricks.contains_key(*key) {
                continue;
            }
            let Some(slot) = self.free_bricks.pop() else {
                self.notes
                    .push(format!("more than {MAX_BRICK_SLOTS} bricks are wanted; the rest are not drawn"));
                break;
            };
            gpu.queue.write_buffer(
                store,
                (slot as usize * BRICK_WORDS * 4) as u64,
                bytemuck::cast_slice(&brick_words(brick)),
            );
            self.bricks.insert(**key, (slot, Arc::clone(brick)));
        }
        // This frame's buffers: the chunks' origins by slot, the draws, the brick instances.
        let origin = camera.origin;
        let mut origins = vec![[0f32; 4]; MAX_CHUNK_SLOTS];
        let mut draws: Vec<[u32; 2]> = Vec::new();
        let mut chunks = Vec::new();
        for (key, _) in &near {
            if let Some(mesh) = self.meshes.get(key) {
                let lo = grid.chunk_box(*key).lo - origin;
                origins[mesh.slot as usize] = [lo.x as f32, lo.y as f32, lo.z as f32, 0.0];
                draws.push([mesh.slot, mesh.count]);
                chunks.push(*key);
            }
        }
        let mut instances: Vec<BrickInstance> = Vec::new();
        for (key, _) in &far {
            if let Some((slot, _)) = self.bricks.get(*key) {
                let b = grid.brick_box(key.0, key.1);
                let lo = b.lo - origin;
                let cell = b.size().x / f64::from(BRICK_SIDE);
                instances.push(BrickInstance {
                    origin: [lo.x as f32, lo.y as f32, lo.z as f32, cell as f32],
                    info: [*slot, u32::from(key.0), 0, 0],
                });
            }
        }
        let eye = camera.eye.to_f32();
        let forward = camera.forward.to_f32();
        let ortho = f32::from(u8::from(view.projection == cav_sim::Projection::Orthographic));
        let lit = f32::from(u8::from(view.style == cav_sim::DrawStyle::Shaded));
        let uniform = WorldUniform {
            view_proj: camera.view_proj.to_f32(),
            eye: [eye[0], eye[1], eye[2], ortho],
            forward: [forward[0], forward[1], forward[2], 0.0],
            params: [grid.pitch as f32, f32::from(u8::from(frame.bands)), lit, 0.0],
        };
        let mut palette = [[0f32; 4]; 256];
        for (i, c) in frame.colours.iter().enumerate().take(256) {
            palette[i] = [linear(c[0]), linear(c[1]), linear(c[2]), 1.0];
        }
        let init = |label, contents: &[u8], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        let uniform_buffer = init("world", bytemuck::bytes_of(&uniform), wgpu::BufferUsages::UNIFORM);
        let palette_buffer =
            init("world palette", bytemuck::cast_slice(&palette), wgpu::BufferUsages::UNIFORM);
        let origins_buffer =
            init("world origins", bytemuck::cast_slice(&origins), wgpu::BufferUsages::STORAGE);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("world"),
            layout: &pipelines.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: palette_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: origins_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: store.as_entire_binding() },
            ],
        });
        let (cull_group, indirect) = if draws.is_empty() {
            (None, None)
        } else {
            let planes = frustum_planes(&camera.view_proj).map(|p| p.map(|v| v as f32));
            let cull =
                CullUniform { planes, sizes: [grid.chunk_side() as f32, draws.len() as f32, 0.0, 0.0] };
            let cull_buffer = init("world cull", bytemuck::bytes_of(&cull), wgpu::BufferUsages::UNIFORM);
            let draws_buffer = init("world draws", bytemuck::cast_slice(&draws), wgpu::BufferUsages::STORAGE);
            let indirect = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("world indirect"),
                size: (draws.len() * 20) as u64,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::INDIRECT
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("world cull"),
                layout: &pipelines.cull_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: cull_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: draws_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: origins_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: indirect.as_entire_binding() },
                ],
            });
            (Some(group), Some(indirect))
        };
        let instances_buffer = (!instances.is_empty()).then(|| {
            init("world bricks drawn", bytemuck::cast_slice(&instances), wgpu::BufferUsages::VERTEX)
        });
        Some(WorldDraw {
            bind_group,
            cull_group,
            indirect,
            chunks,
            instances: instances_buffer,
            bricks: instances.len() as u32,
        })
    }

    /// The cull pass: one thread a drawn chunk, writing its indirect draw.
    pub fn cull(&self, pipelines: &WorldPipelines, encoder: &mut wgpu::CommandEncoder, draw: &WorldDraw) {
        if let Some(group) = &draw.cull_group {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("world cull"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipelines.cull);
            pass.set_bind_group(0, group, &[]);
            pass.dispatch_workgroups((draw.chunks.len() as u32).div_ceil(64), 1, 1);
        }
    }

    /// Draws the frame's chunks (each by its indirect draw) and bricks into `pass`.
    pub fn draw<'a>(
        &'a self,
        pipelines: &'a WorldPipelines,
        pass: &mut wgpu::RenderPass<'a>,
        draw: &'a WorldDraw,
    ) {
        pass.set_bind_group(0, &draw.bind_group, &[]);
        if let Some(indirect) = &draw.indirect {
            pass.set_pipeline(&pipelines.chunk);
            for (i, key) in draw.chunks.iter().enumerate() {
                let Some(mesh) = self.meshes.get(key) else { continue };
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed_indirect(indirect, (i * 20) as u64);
            }
        }
        if let Some(instances) = &draw.instances {
            pass.set_pipeline(&pipelines.brick);
            pass.set_vertex_buffer(0, instances.slice(..));
            pass.draw(0..36, 0..draw.bricks);
        }
    }

    /// For tests: the indirect draws' buffer of `draw` and its chunks, in order.
    #[must_use]
    pub fn indirect<'a>(&self, draw: &'a WorldDraw) -> (Option<&'a wgpu::Buffer>, &'a [ChunkKey]) {
        (draw.indirect.as_ref(), &draw.chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(seed: u64, n: u64) -> f64 {
        let mut z = seed.wrapping_add(n.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A chunk of random cells (half full, three materials), material + 1 per cell.
    fn random_cells(seed: u64, fill: f64) -> Vec<u8> {
        (0..32 * 32 * 32u64)
            .map(|i| if unit(seed, i) < fill { 1 + (unit(seed + 1, i) * 3.0) as u8 } else { 0 })
            .collect()
    }

    #[test]
    fn a_greedy_mesh_covers_the_exposed_faces() {
        let cells = random_cells(3, 0.5);
        let east = random_cells(4, 0.7);
        let neighbours = [None, Some(&east[..]), None, None, None, None];
        let mesh = greedy_mesh(&cells, neighbours, 9);
        // The exposed faces, counted cell by cell: per face direction and material.
        let mut want = HashMap::new();
        let occupied = |c: [i32; 3]| -> bool {
            if c.iter().all(|v| (0..32).contains(v)) {
                cells[cell_index(c.map(|v| v as u32))] != 0
            } else if c[0] == 32 && (0..32).contains(&c[1]) && (0..32).contains(&c[2]) {
                east[cell_index([0, c[1] as u32, c[2] as u32])] != 0
            } else {
                false
            }
        };
        let mut buried = 0;
        for z in 0..32 {
            for y in 0..32 {
                for x in 0..32 {
                    let m = cells[cell_index([x as u32, y as u32, z as u32])];
                    if m == 0 {
                        continue;
                    }
                    for face in 0..6usize {
                        let mut c = [x, y, z];
                        c[face / 2] += if face % 2 == 1 { 1 } else { -1 };
                        if occupied(c) {
                            buried += 1;
                        } else {
                            *want.entry((face as u8, m - 1)).or_insert(0u32) += 1;
                        }
                    }
                }
            }
        }
        let mut got = HashMap::new();
        for quad in mesh.vertices.chunks(4) {
            let [a, _, c, _] = [quad[0], quad[1], quad[2], quad[3]];
            let face = a.attributes[0];
            let axis = usize::from(face / 2);
            let area: u32 =
                (0..3).filter(|&k| k != axis).map(|k| u32::from(c.cell[k].abs_diff(a.cell[k]))).product();
            assert!(quad.iter().all(|v| v.cell[3] == 9 && v.attributes == a.attributes));
            *got.entry((face, a.attributes[1])).or_insert(0u32) += area;
        }
        assert_eq!(got, want);
        // It merged: far fewer quads than faces.
        let faces: u32 = want.values().sum();
        assert!((mesh.vertices.len() / 4) < faces as usize);
        assert_eq!(mesh.indices.len(), mesh.vertices.len() / 4 * 6);
        // Control: counting the buried faces too would not match.
        assert!(buried > 0);
        let mut with_buried = want.clone();
        *with_buried.get_mut(&(1, 0)).unwrap() += 1;
        assert_ne!(got, with_buried);
        // A full chunk with full neighbours has no faces; alone it has its six sides, one quad each.
        let full = vec![1u8; 32 * 32 * 32];
        assert!(greedy_mesh(&full, [Some(&full[..]); 6], 0).indices.is_empty());
        assert_eq!(greedy_mesh(&full, [None; 6], 0).vertices.len(), 6 * 4);
    }

    #[test]
    fn the_march_meets_the_first_occupied_cell() {
        // A brick with one cell set, and a ray at it from outside along x.
        let mut brick = Brick::empty(1, ChunkKey::new(0, 0, 0));
        brick.set([20, 5, 9]);
        let words = brick_words(&brick);
        let hit = march(&words, [0.0, 5.5, 9.5], [1.0, 1e-7, 1e-7], 0);
        assert!(hit.hit && hit.cell == [20, 5, 9] && hit.face == 0, "{hit:?}");
        assert!((hit.t - 20.0).abs() < 1e-3, "{}", hit.t);
        // From the other side, it comes in through +x.
        let back = march(&words, [31.9, 5.5, 9.5], [-1.0, 1e-7, 1e-7], 1);
        assert!(back.hit && back.face == 1 && (back.t - 10.9).abs() < 1e-3, "{back:?}");
        // A ray beside it misses, within its steps.
        let miss = march(&words, [0.0, 6.5, 9.5], [1.0, 0.013, 0.007], 0);
        assert!(!miss.hit && miss.steps <= MAX_MARCH_STEPS);
        // Every ray through a full brick stops at once; through a random one, at the first cell a
        // dense walk of the same line meets.
        let mut random = Brick::empty(1, ChunkKey::new(0, 0, 0));
        for i in 0..32 * 32 * 32u64 {
            if unit(11, i) < 0.02 {
                random.set([(i % 32) as u32, (i / 32 % 32) as u32, (i / 1024) as u32]);
            }
        }
        let words = brick_words(&random);
        let mut agree = 0;
        for r in 0..200u64 {
            let start = [0.0f32, (32.0 * unit(r, 1)) as f32, (32.0 * unit(r, 2)) as f32];
            let dir = [1.0f32, (unit(r, 3) - 0.5) as f32, (unit(r, 4) - 0.5) as f32];
            let hit = march(&words, start, dir, 0);
            assert!(hit.steps <= 230, "{} steps", hit.steps);
            // The dense walk: small steps along the line, the first occupied cell.
            let mut first = None;
            for k in 0..64_000 {
                let t = k as f32 * 0.0005;
                let p = [start[0] + dir[0] * t, start[1] + dir[1] * t, start[2] + dir[2] * t];
                if p.iter().any(|v| *v < 0.0 || *v >= 32.0) {
                    break;
                }
                let c = p.map(|v| v.floor() as u32);
                if random.get(c) {
                    first = Some(c.map(|v| v as i32));
                    break;
                }
            }
            assert_eq!(hit.hit, first.is_some(), "ray {r}");
            if let Some(c) = first {
                assert_eq!(hit.cell, c, "ray {r}");
                agree += 1;
            }
        }
        assert!(agree > 40, "{agree}");
    }

    #[test]
    fn culling_keeps_what_the_camera_sees() {
        let mut view = ViewSpec::framing(
            &cav_sim::SceneSpec::room(),
            cav_sim::NamedView::Iso,
            cav_sim::Projection::Perspective,
            cav_sim::DrawStyle::Shaded,
        );
        view.distance = 30.0;
        let camera = Camera::new(&view, 1.5);
        let planes = frustum_planes(&camera.view_proj).map(|p| p.map(|v| v as f32));
        // A box at the target is seen; one far behind the camera is not.
        assert!(visible(&planes, [-1.0, -1.0, -1.0], 2.0));
        let behind = camera.eye * 3.0;
        assert!(!visible(&planes, [behind.x as f32, behind.y as f32, behind.z as f32], 1.0));
        // Every box whose centre projects inside the picture is kept (conservative).
        for i in 0..2000u64 {
            let c = cav_core::Vec3::new(
                80.0 * (unit(i, 0) - 0.5),
                80.0 * (unit(i, 1) - 0.5),
                40.0 * (unit(i, 2) - 0.5),
            );
            let clip = camera.view_proj.transform(c);
            let inside = clip[3] > 0.0
                && (0..2).all(|k| (clip[k] / clip[3]).abs() < 0.99)
                && (0.0..1.0).contains(&(clip[2] / clip[3]));
            if inside {
                let lo = [c.x as f32 - 0.5, c.y as f32 - 0.5, c.z as f32 - 0.5];
                assert!(visible(&planes, lo, 1.0), "box at {c:?}");
            }
        }
    }
}
