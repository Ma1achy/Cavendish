//! The wgpu device context and low-level dispatch helpers, isolated from the forward-model logic.

use wgpu::util::DeviceExt;

use crate::ComputeError;

/// The wgpu device context — created once, shared across dispatches (and, later, the viewer).
pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_name: String,
}

impl Gpu {
    /// Acquire a compute-capable wgpu device (Metal / Vulkan / DX12). Falls back to a software adapter
    /// (lavapipe under CI) when no hardware adapter is present.
    pub fn new() -> Result<Self, ComputeError> {
        let instance = wgpu::Instance::default();
        let request = |fallback: bool| {
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: fallback,
                compatible_surface: None,
            }))
        };
        let adapter = request(false)
            .or_else(|| request(true))
            .ok_or_else(|| ComputeError::DeviceUnavailable("no adapter".into()))?;
        let adapter_name = adapter.get_info().name;
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("cavendish-compute"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|e| ComputeError::DeviceUnavailable(e.to_string()))?;
        Ok(Gpu {
            device,
            queue,
            adapter_name,
        })
    }

    /// Read a storage buffer back to the host as `f32`.
    fn read_f32(&self, buffer: &wgpu::Buffer, len: usize) -> Vec<f32> {
        let size = (len * std::mem::size_of::<f32>()) as u64;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        enc.copy_buffer_to_buffer(buffer, 0, &readback, 0, size);
        self.queue.submit([enc.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::Maintain::Wait);
        let out: Vec<f32> = {
            let data = slice.get_mapped_range();
            bytemuck::cast_slice(&data).to_vec()
        };
        readback.unmap();
        out
    }

    /// Run one forward kernel: bind `cloud` (vec4 = xyz+mass), a packed `params` f32 array, and an
    /// `out` f32 array; dispatch `entry` once; read `out` back. The building block for kernel parity.
    pub fn run_kernel(
        &self,
        wgsl: &str,
        entry: &str,
        cloud: &[[f32; 4]],
        params: &[f32],
        out_len: usize,
    ) -> Vec<f32> {
        // buffer_init rejects empty contents; pad an unused cloud/params with one dummy element.
        let cloud_data: Vec<[f32; 4]> = if cloud.is_empty() {
            vec![[0.0; 4]]
        } else {
            cloud.to_vec()
        };
        let params_data: Vec<f32> = if params.is_empty() {
            vec![0.0]
        } else {
            params.to_vec()
        };
        let cloud_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("cloud"),
                contents: bytemuck::cast_slice(&cloud_data),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let params_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("params"),
                contents: bytemuck::cast_slice(&params_data),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let out_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("out"),
            size: (out_len * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let storage = |read_only: bool| wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let bgl = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("forward"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        ..storage(true)
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        ..storage(true)
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        ..storage(false)
                    },
                ],
            });
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&bgl],
                push_constant_ranges: &[],
            });
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("forward"),
                source: wgpu::ShaderSource::Wgsl(wgsl.into()),
            });
        let pipeline = self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&layout),
                module: &shader,
                entry_point: entry,
                compilation_options: Default::default(),
                cache: None,
            });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cloud_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: out_buf.as_entire_binding(),
                },
            ],
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        self.queue.submit([enc.finish()]);
        self.read_f32(&out_buf, out_len)
    }

    /// Smoke helper: double a buffer of `f32` on-device (validates the whole dispatch/read-back path).
    pub fn run_double(&self, input: &[f32]) -> Vec<f32> {
        let storage = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("io"),
                contents: bytemuck::cast_slice(input),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            });
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("double"),
                source: wgpu::ShaderSource::Wgsl(
                    r#"
@group(0) @binding(0) var<storage, read_write> data: array<f32>;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i < arrayLength(&data)) { data[i] = data[i] * 2.0; }
}
"#
                    .into(),
                ),
            });
        let pipeline = self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("double"),
                layout: None,
                module: &shader,
                entry_point: "main",
                compilation_options: Default::default(),
                cache: None,
            });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: storage.as_entire_binding(),
            }],
        });
        let groups = input.len().div_ceil(64) as u32;
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(groups, 1, 1);
        }
        self.queue.submit([enc.finish()]);
        self.read_f32(&storage, input.len())
    }
}
