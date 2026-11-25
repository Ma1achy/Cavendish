//! The world's GPU kernels against their CPU twins, and a world drawn near and far
//! (`docs/slices/S4.md` R10, R11, R15). The GPU tier: `cargo xtask gpu`.

use std::sync::Arc;

use cav_core::Vec3;
use cav_core::voxel::world::{Brick, ChunkCells, ChunkKey};
use cav_gpu::Gpu;
use cav_gpu::wgpu::{self, util::DeviceExt};
use cav_render::world::{
    BRICK_WORDS, GpuCaps, MAX_MARCH_STEPS, WorldGpu, brick_words, frustum_planes, march, march_source,
    visible,
};
use cav_render::{Camera, Frame, Renderer};
use cav_sim::world::clipmap::{WorldFrame, level_count, levels, lod_centre, ring};
use cav_sim::world::{MaterialSpec, PieceKind, PieceSpec, World, WorldSpec};
use cav_sim::{DrawStyle, NamedView, PaneContent, Projection, SceneSpec, ViewSpec};

fn unit(seed: u64, n: u64) -> f64 {
    let mut z = seed.wrapping_add(n.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
}

fn gpu() -> Gpu {
    Gpu::new().expect("the GPU tier needs an adapter")
}

fn view(target: [f64; 3], yaw: f64, pitch: f64, distance: f64) -> ViewSpec {
    let mut v =
        ViewSpec::framing(&SceneSpec::room(), NamedView::Iso, Projection::Perspective, DrawStyle::Shaded);
    v.target = target;
    v.yaw = yaw;
    v.pitch = pitch;
    v.distance = distance;
    v
}

#[test]
fn gpu_world_culling_matches_the_cpu() {
    let gpu = gpu();
    let renderer = Renderer::new(&gpu);
    let grid = cav_core::voxel::world::WorldGrid { origin: Vec3::ZERO, pitch: 0.25, supersample: 1 };
    let cells = Arc::new(ChunkCells {
        key: ChunkKey::new(0, 0, 0),
        leaves: vec![cav_core::voxel::Leaf { key: 0, level: 0, count: 1, material: 0 }],
        palette: vec![0],
    });
    let (pipelines, store) = renderer.world_parts();
    let mut compared = 0;
    let (mut seen, mut hidden) = (0, 0);
    // Control: the twin with one plane turned round disagrees with the GPU, so the comparison
    // can fail.
    let mut disagree = 0;
    for camera_n in 0..20u64 {
        let v = view(
            [40.0 * (unit(camera_n, 0) - 0.5), 40.0 * (unit(camera_n, 1) - 0.5), 2.0],
            360.0 * unit(camera_n, 2),
            -10.0 + 60.0 * unit(camera_n, 3),
            5.0 + 20.0 * unit(camera_n, 4),
        );
        let camera = Camera::new(&v, 1.6);
        // About a hundred chunks a camera within its level-0 box: 2000 boxes in all.
        let centre = lod_centre(&v);
        let level = levels(&grid, centre, 16, 1)[0];
        let mut frame = WorldFrame {
            grid: Some(grid),
            colours: vec![[200, 100, 50]],
            ring: 16,
            levels: 1,
            world: 7 + camera_n,
            ..WorldFrame::default()
        };
        for i in 0..100u64 {
            let k = [0, 1, 2].map(|a| level.lo[a] + (unit(camera_n * 1000 + i, 5 + a as u64) * 32.0) as i64);
            let key = ChunkKey::new(k[0] as i32, k[1] as i32, k[2] as i32);
            frame.chunks.insert(key, Arc::new(ChunkCells { key, ..(*cells).clone() }));
        }
        let mut world = store.lock().unwrap();
        let draw =
            world.prepare(&gpu, pipelines, &frame, &v, &camera, [1600, 1000], true).expect("a frame to draw");
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        world.cull(pipelines, &mut encoder, &draw);
        gpu.queue.submit([encoder.finish()]);
        let (indirect, chunks) = world.indirect(&draw);
        let bytes = gpu.read_buffer(indirect.expect("draws")).unwrap();
        let words: Vec<u32> = bytemuck::cast_slice(&bytes).to_vec();
        let planes = frustum_planes(&camera.view_proj).map(|p| p.map(|x| x as f32));
        for (i, key) in chunks.iter().enumerate() {
            let lo = grid.chunk_box(*key).lo - camera.origin;
            let cpu = visible(&planes, [lo.x as f32, lo.y as f32, lo.z as f32], grid.chunk_side() as f32);
            let on_gpu = words[i * 5 + 1] == 1;
            assert_eq!(on_gpu, cpu, "camera {camera_n}, chunk {key:?}");
            let mut turned = planes;
            turned[0] = turned[0].map(|v| -v);
            let wrong = visible(&turned, [lo.x as f32, lo.y as f32, lo.z as f32], grid.chunk_side() as f32);
            disagree += usize::from(wrong != on_gpu);
            assert_eq!(words[i * 5], 36, "one cell: six faces, six indices each");
            compared += 1;
            if cpu { seen += 1 } else { hidden += 1 }
        }
    }
    eprintln!(
        "{compared} boxes compared: {seen} seen, {hidden} culled; a turned plane disagrees on {disagree}"
    );
    assert!(compared >= 1900 && seen > 100 && hidden > 100, "{compared} {seen} {hidden}");
    assert!(disagree > 100, "{disagree}");
}

#[test]
fn gpu_brick_march_matches_the_cpu() {
    let gpu = gpu();
    // Eight bricks of random cells at different fills.
    let bricks: Vec<Vec<u32>> = (0..8u64)
        .map(|b| {
            let mut brick = Brick::empty(1, ChunkKey::new(0, 0, 0));
            let fill = [0.0005, 0.002, 0.01, 0.03, 0.1, 0.3, 0.001, 0.05][b as usize];
            for i in 0..32 * 32 * 32u64 {
                if unit(100 + b, i) < fill {
                    brick.set([(i % 32) as u32, (i / 32 % 32) as u32, (i / 1024) as u32]);
                }
            }
            brick_words(&brick)
        })
        .collect();
    // Rays from every face, inward, at random angles.
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    #[repr(C)]
    struct Ray {
        start: [f32; 4],
        dir: [f32; 4],
        info: [u32; 4],
    }
    let mut rays = Vec::new();
    for r in 0..4096u64 {
        let face = (r % 6) as usize;
        let axis = face / 2;
        let positive = face % 2 == 1;
        let mut start = [(32.0 * unit(r, 1)) as f32, (32.0 * unit(r, 2)) as f32, (32.0 * unit(r, 3)) as f32];
        start[axis] = if positive { 32.0 } else { 0.0 };
        let mut dir = [(unit(r, 4) - 0.5) as f32, (unit(r, 5) - 0.5) as f32, (unit(r, 6) - 0.5) as f32];
        dir[axis] = (0.3 + unit(r, 7) as f32) * if positive { -1.0 } else { 1.0 };
        // Entering through the face it starts on: stepping into −x from +x comes in through +x.
        let entry = (axis * 2) as u32 + u32::from(positive);
        rays.push(Ray {
            start: [start[0], start[1], start[2], 0.0],
            dir: [dir[0], dir[1], dir[2], 0.0],
            info: [(r % 8) as u32, entry, 0, 0],
        });
    }
    // Rays along an axis or in an axis plane (an orthographic named view: components exactly
    // zero, the review's I9), half of them starting on a sub-block's plane or a cell's.
    let along = rays.len();
    for r in 0..1024u64 {
        let face = (r % 6) as usize;
        let axis = face / 2;
        let positive = face % 2 == 1;
        let on_plane =
            |v: f64| if r % 2 == 0 { (8.0 * (v * 4.0).floor()).min(24.0) } else { (32.0 * v).floor() };
        let mut start =
            [on_plane(unit(r, 11)) as f32, on_plane(unit(r, 12)) as f32, on_plane(unit(r, 13)) as f32];
        if r % 4 >= 2 {
            start = start.map(|v| v + 0.5);
        }
        start[axis] = if positive { 32.0 } else { 0.0 };
        let mut dir = [0.0f32; 3];
        dir[axis] = if positive { -1.0 } else { 1.0 };
        // A quarter in a plane: one other component not zero.
        if r % 8 >= 6 {
            dir[(axis + 1) % 3] = (unit(r, 14) - 0.5) as f32;
        }
        let entry = (axis * 2) as u32 + u32::from(positive);
        rays.push(Ray {
            start: [start[0], start[1], start[2], 0.0],
            dir: [dir[0], dir[1], dir[2], 0.0],
            info: [(r % 8) as u32, entry, 0, 0],
        });
    }
    let device = &gpu.device;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("march"),
        source: wgpu::ShaderSource::Wgsl(march_source().into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("march"),
        layout: None,
        module: &module,
        entry_point: Some("cs_march"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    let init = |contents: &[u8], usage| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents, usage })
    };
    let all: Vec<u32> = bricks.concat();
    assert_eq!(all.len(), 8 * BRICK_WORDS);
    let ray_buffer = init(bytemuck::cast_slice(&rays), wgpu::BufferUsages::STORAGE);
    let brick_buffer = init(bytemuck::cast_slice(&all), wgpu::BufferUsages::STORAGE);
    let found = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (rays.len() * 32) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: ray_buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: found.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: brick_buffer.as_entire_binding() },
        ],
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups((rays.len() as u32).div_ceil(64), 1, 1);
    }
    gpu.queue.submit([encoder.finish()]);
    let bytes = gpu.read_buffer(&found).unwrap();
    let words: Vec<u32> = bytemuck::cast_slice(&bytes).to_vec();
    let (mut hits, mut worst_t, mut most_steps, mut different) = (0, 0f32, 0, Vec::new());
    let mut axis_hits = 0;
    for (i, ray) in rays.iter().enumerate() {
        let w = &words[i * 8..i * 8 + 8];
        let cpu = march(
            &bricks[ray.info[0] as usize],
            [ray.start[0], ray.start[1], ray.start[2]],
            [ray.dir[0], ray.dir[1], ray.dir[2]],
            ray.info[1],
        );
        let gpu_hit = w[0] == 1;
        let gpu_cell = [w[4] as i32, w[5] as i32, w[6] as i32];
        most_steps = most_steps.max(w[3]).max(cpu.steps);
        if gpu_hit != cpu.hit || (cpu.hit && (gpu_cell != cpu.cell || w[2] != cpu.face)) {
            different.push((i, cpu, gpu_hit, gpu_cell, f32::from_bits(w[1])));
            continue;
        }
        if cpu.hit {
            hits += 1;
            axis_hits += usize::from(i >= along);
            worst_t = worst_t.max((f32::from_bits(w[1]) - cpu.t).abs());
        }
        // Every number the GPU gave back is finite.
        assert!(f32::from_bits(w[1]).is_finite(), "ray {i}: t {}", f32::from_bits(w[1]));
    }
    eprintln!(
        "{} rays ({} along an axis or in its plane): {hits} hits ({axis_hits} of those); t within {worst_t:e} \
         cells; at most {most_steps} steps; {} differ",
        rays.len(),
        rays.len() - along,
        different.len()
    );
    assert!(different.is_empty(), "{:?}", &different[..different.len().min(5)]);
    // Within 1e-4 of the brick's side (32 cells), and within the step cap.
    assert!(worst_t <= 1e-4 * 32.0, "{worst_t}");
    assert!(most_steps <= MAX_MARCH_STEPS);
    assert!(hits > 1000 && hits < 5000, "{hits}");
    assert!(axis_hits > 100 && axis_hits < 1024, "{axis_hits}");
}

/// A world of hills at 50 cm, 240 m across, its frame made resident for `v` as the engine's
/// streaming would make it.
fn hills_frame(v: &ViewSpec) -> WorldFrame {
    let spec = WorldSpec {
        pitch: 0.5,
        supersample: 1,
        origin: [0.0; 3],
        materials: vec![MaterialSpec::plain("grass", 1800.0, "#40c040")],
        pieces: vec![PieceSpec {
            label: "hills".into(),
            material: 0,
            kind: PieceKind::Terrain {
                lo: [-120.0, -120.0],
                hi: [120.0, 120.0],
                bottom: -3.0,
                level: -1.0,
                relief: 1.5,
                wavelength: 60.0,
                seed: 3,
            },
        }],
        stream: Default::default(),
        read: Default::default(),
        lod: cav_sim::world::LodSpec { error_px: 32.0 },
    };
    let world = World::new(&spec);
    let grid = world.grid;
    let extent = world.extent().unwrap();
    let n = ring(spec.lod.error_px);
    let centre = lod_centre(v);
    let count = level_count(&grid, &extent, centre, n);
    let mut frame = WorldFrame {
        grid: Some(grid),
        colours: world.colours.clone(),
        ring: n,
        levels: count,
        extent: Some(extent),
        world: spec.hash(),
        ..WorldFrame::default()
    };
    for level in levels(&grid, centre, n, count) {
        for key in level.keys(grid.bricks_of(level.level, &extent)) {
            if level.level == 0 {
                if let Some(chunk) = world.chunk(key).unwrap() {
                    frame.chunks.insert(key, Arc::new(ChunkCells::of(&chunk)));
                }
            } else if let Some(brick) = world.brick(level.level, key) {
                frame.bricks.insert((level.level, key), Arc::new(brick));
            }
        }
    }
    frame
}

/// Prepares `frame` for `v` on `world` (a picture taken once: every mesh wanted is made).
fn prepare(gpu: &Gpu, renderer: &Renderer, world: &mut WorldGpu, frame: &WorldFrame, v: &ViewSpec) {
    let (pipelines, _) = renderer.world_parts();
    let camera = Camera::new(v, 480.0 / 270.0);
    world.prepare(gpu, pipelines, frame, v, &camera, [480, 270], true).expect("a frame to draw");
}

#[test]
fn gpu_meshes_past_the_vertex_cap_are_not_drawn() {
    // W7: the near field's caps reached with small ones; the nearest meshes are kept, the cap is
    // said, and a frame at the cap makes no meshes (the review's I5, I7).
    let gpu = gpu();
    let renderer = Renderer::new(&gpu);
    let v = view([0.0, 0.0, 2.0], 200.0, 8.0, 6.0);
    let frame = hills_frame(&v);
    let mut whole = WorldGpu::default();
    prepare(&gpu, &renderer, &mut whole, &frame, &v);
    let [meshes, bricks, vertices] = whole.counts();
    assert!(whole.notes.iter().all(|n| !n.contains("not drawn (the nearest are)")), "{:?}", whole.notes);
    // More of each than the small caps below, so each binds: the brick cap below is 7, and more
    // than 7 bricks are wanted (only those in the frustum are uploaded since S4's second review,
    // minor 15; this view holds 16).
    assert!(meshes > 20 && vertices > 10_000 && bricks > 7, "{meshes} {vertices} {bricks}");
    // The vertex cap: a third of what this view wants.
    let cap = vertices / 3;
    let mut capped = WorldGpu::with_caps(GpuCaps { vertices: cap, ..GpuCaps::default() });
    prepare(&gpu, &renderer, &mut capped, &frame, &v);
    let [held, _, held_vertices] = capped.counts();
    assert!(held_vertices <= cap && held < meshes && held > 0, "{held_vertices} of {cap}");
    assert!(capped.notes.iter().any(|n| n.contains(&format!("meshes past {cap} vertices are not drawn"))));
    // The next frame at the cap meshes nothing, and says the cap again.
    let made = capped.meshed;
    prepare(&gpu, &renderer, &mut capped, &frame, &v);
    assert_eq!(capped.meshed, made, "no meshes are made and thrown away at the cap");
    assert!(capped.notes.iter().any(|n| n.contains("vertices are not drawn")));
    // Nor a frame with the eye nudged round, which lists the waiting chunks in another order
    // (S4's second review, J3).
    let mut nudged = v.clone();
    nudged.yaw += 7.0;
    nudged.distance += 0.3;
    let camera = |view: &ViewSpec| Camera::new(view, 480.0 / 270.0);
    let before = capped.waiting(&frame, &v, &camera(&v));
    let after = capped.waiting(&frame, &nudged, &camera(&nudged));
    let sorted = |mut keys: Vec<ChunkKey>| {
        keys.sort_unstable();
        keys
    };
    // (The third review's minor 5: the nudge does reorder the same waiting chunks.)
    assert!(before != after && sorted(before.clone()) == sorted(after.clone()), "{} waiting", before.len());
    prepare(&gpu, &renderer, &mut capped, &frame, &nudged);
    assert_eq!(capped.meshed, made, "a camera moving at the cap makes no meshes");
    // Control: the capped frame made more meshes than it kept (which the next frame would make
    // and throw away again, but for the fingerprint), and a camera moved starts it again.
    assert!(made > held as u64, "{made} made, {held} kept");
    let moved = view([30.0, 10.0, 2.0], 120.0, 8.0, 6.0);
    prepare(&gpu, &renderer, &mut capped, &hills_frame(&moved), &moved);
    assert!(capped.meshed > made);
    // The slot cap: at most that many meshes.
    let mut few = WorldGpu::with_caps(GpuCaps { meshes: 5, ..GpuCaps::default() });
    prepare(&gpu, &renderer, &mut few, &frame, &v);
    assert!(few.counts()[0] <= 5 && few.notes.iter().any(|n| n.contains("chunks past 5 are not drawn")));
    // The brick cap: at most that many bricks, the nearest.
    let mut store = WorldGpu::with_caps(GpuCaps { bricks: 7, ..GpuCaps::default() });
    prepare(&gpu, &renderer, &mut store, &frame, &v);
    assert!(store.counts()[1] == 7 && store.notes.iter().any(|n| n.contains("bricks past 7 are not drawn")));
    // A cap is never raised past its constant.
    let asked = GpuCaps { meshes: usize::MAX, vertices: usize::MAX, bricks: usize::MAX };
    let mut most = WorldGpu::with_caps(asked);
    prepare(&gpu, &renderer, &mut most, &frame, &v);
    assert_eq!(most.counts(), whole.counts());
}

/// A chunk full of one material: six faces of four vertices when alone.
fn full(key: ChunkKey) -> Arc<ChunkCells> {
    Arc::new(ChunkCells {
        key,
        leaves: vec![cav_core::voxel::Leaf { key: 0, level: 5, count: 1, material: 0 }],
        palette: vec![0],
    })
}

/// A frame of full chunks at `keys`, on a grid of 25 cm (chunks of 8 m), seen from `view`'s
/// camera (within its level 0).
fn full_frame(keys: &[[i32; 3]]) -> WorldFrame {
    let grid = cav_core::voxel::world::WorldGrid { origin: Vec3::ZERO, pitch: 0.25, supersample: 1 };
    let mut frame = WorldFrame {
        grid: Some(grid),
        colours: vec![[200, 100, 50]],
        ring: 16,
        levels: 1,
        world: 99,
        ..WorldFrame::default()
    };
    for k in keys {
        let key = ChunkKey::new(k[0], k[1], k[2]);
        frame.chunks.insert(key, full(key));
    }
    frame
}

#[test]
fn gpu_at_a_mesh_cap_no_drawn_chunk_disappears() {
    // S4's second review, J3. Chunks A (0,0,0) and B (1,0,0) side by side: 20 vertices each.
    let gpu = gpu();
    let renderer = Renderer::new(&gpu);
    let v = view([4.0, 4.0, 4.0], 30.0, 20.0, 30.0);
    // (a) B leaves and N (far from A) arrives at a cap of 44: N (missing, first) takes 24, and
    // A's new mesh (24, a face no longer buried) does not fit: A keeps its old mesh, still drawn.
    // (The same frame changed in place: A's cells stay the same, as streaming keeps them.)
    let moved = |frame: &mut WorldFrame| {
        frame.chunks.remove(&ChunkKey::new(1, 0, 0));
        frame.chunks.insert(ChunkKey::new(0, 3, 0), full(ChunkKey::new(0, 3, 0)));
    };
    for (cap, after) in [(44, [2, 0, 44]), (48, [2, 0, 48])] {
        let mut world = WorldGpu::with_caps(GpuCaps { vertices: cap, ..GpuCaps::default() });
        let mut frame = full_frame(&[[0, 0, 0], [1, 0, 0]]);
        prepare(&gpu, &renderer, &mut world, &frame, &v);
        assert_eq!(world.counts(), [2, 0, 40]);
        moved(&mut frame);
        prepare(&gpu, &renderer, &mut world, &frame, &v);
        // At 44, A and N drawn, A with its old mesh; control, at 48, A meshed again with its
        // sixth face.
        assert_eq!(world.counts(), after, "cap {cap}");
    }
    // (b) A alone (24) at a cap of 30; B arrives. B (missing, first) needs 20 and does not fit;
    // A's new mesh (20: its face against B buried) does, and is made: the loop goes on past B.
    let mut tight = WorldGpu::with_caps(GpuCaps { vertices: 30, ..GpuCaps::default() });
    let mut frame = full_frame(&[[0, 0, 0]]);
    prepare(&gpu, &renderer, &mut tight, &frame, &v);
    assert_eq!(tight.counts(), [1, 0, 24]);
    frame.chunks.insert(ChunkKey::new(1, 0, 0), full(ChunkKey::new(1, 0, 0)));
    prepare(&gpu, &renderer, &mut tight, &frame, &v);
    assert_eq!(tight.counts(), [1, 0, 20], "B left out, A meshed again past it");
}

#[test]
fn gpu_a_neighbour_arriving_buries_faces() {
    // Minor 13: two full chunks side by side are ten faces, five each; the first, meshed alone,
    // had six, and is meshed again when its neighbour arrives.
    let gpu = gpu();
    let renderer = Renderer::new(&gpu);
    let v = view([4.0, 4.0, 4.0], 30.0, 20.0, 30.0);
    let mut frame = full_frame(&[[0, 0, 0]]);
    let mut world = WorldGpu::default();
    prepare(&gpu, &renderer, &mut world, &frame, &v);
    assert_eq!(world.counts()[2], 6 * 4, "one full chunk: six faces of four vertices");
    frame.chunks.insert(ChunkKey::new(1, 0, 0), full(ChunkKey::new(1, 0, 0)));
    prepare(&gpu, &renderer, &mut world, &frame, &v);
    assert_eq!(world.counts(), [2, 0, 10 * 4], "the face between them is buried on both sides");
    // And when the neighbour goes, the face is drawn again.
    frame.chunks.remove(&ChunkKey::new(1, 0, 0));
    prepare(&gpu, &renderer, &mut world, &frame, &v);
    assert_eq!(world.counts(), [1, 0, 6 * 4]);
}

#[test]
fn gpu_a_world_is_drawn_near_and_far() {
    let gpu = gpu();
    let renderer = Renderer::new(&gpu);
    let mut scene = SceneSpec::room();
    scene.room = None;
    scene.array.towers.clear();
    // Low over the hills, looking along them to the horizon.
    let v = view([0.0, 0.0, 2.0], 200.0, 8.0, 6.0);
    let size = [480, 270];
    let draw = |world: Option<&WorldFrame>, bands: bool| {
        let mut v = v.clone();
        v.overlay.bands = bands;
        let frame = Frame {
            scene: &scene,
            view: &v,
            highlight: None,
            time: 0.0,
            content: PaneContent::Scene,
            span: None,
            voxels: None,
            world,
        };
        renderer.render_rgba(&gpu, frame, size).unwrap()
    };
    // With the LOD bands: level 0 (meshes) orange, level 1 blue, level 2 green (ray-marched).
    let banded = hills_frame(&v);
    assert!(!banded.chunks.is_empty() && !banded.bricks.is_empty());
    let pixels = draw(Some(&banded), true);
    let plain = draw(Some(&banded), false);
    std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/gpu-world")).unwrap();
    for (name, p) in [("hills.png", &plain), ("hills-bands.png", &pixels)] {
        let png = cav_render::encode_png(p, size).unwrap();
        std::fs::write(format!("{}/../../target/gpu-world/{name}", env!("CARGO_MANIFEST_DIR")), png).unwrap();
    }
    let count = |pixels: &[u8], rows: std::ops::Range<u32>, test: &dyn Fn([u8; 3]) -> bool| {
        rows.flat_map(|y| (0..size[0]).map(move |x| ((y * size[0] + x) * 4) as usize))
            .filter(|&i| test([pixels[i], pixels[i + 1], pixels[i + 2]]))
            .count()
    };
    let orange = |c: [u8; 3]| {
        let [r, g, b] = c.map(i32::from);
        r > g + 20 && g > b + 40
    };
    let coarser = |c: [u8; 3]| {
        let [r, g, b] = c.map(i32::from);
        b > r + 40 || (g > r + 30 && g > b + 10)
    };
    let (near, far) = (count(&pixels, 200..270, &orange), count(&pixels, 0..200, &coarser));
    let (meshes, bricks, vertices) = {
        let (c, _) = renderer.world_counts();
        (c[0], c[1], c[2])
    };
    eprintln!(
        "near (meshes) {near} px, far (bricks) {far} px; {meshes} meshes of {vertices} vertices, {bricks} bricks"
    );
    assert!(near > 2000, "the near field is drawn by meshes: {near}");
    assert!(far > 500, "the far field is drawn by bricks: {far}");
    // In its own colours, the ground is green near and far.
    let green = |c: [u8; 3]| {
        let [r, g, b] = c.map(i32::from);
        g > r + 30 && g > b + 30
    };
    assert!(count(&plain, 200..270, &green) > 2000 && count(&plain, 0..200, &green) > 500);
    // Control: no world, no green.
    let none = draw(None, false);
    assert_eq!(count(&none, 0..270, &green), 0);
}
