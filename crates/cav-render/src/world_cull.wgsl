// Cavendish S4: the near field's culling, one thread a chunk; its CPU twin is `world::visible`.

struct Cull {
    // The frustum's six planes (xyz normal pointing in, w offset) in the target's frame.
    planes: array<vec4<f32>, 6>,
    // x: the chunk's side, metres; y: how many draws.
    sizes: vec4<f32>,
};

struct Draw {
    // x: the chunk's slot; y: its index count.
    slot: u32,
    count: u32,
};

struct Indirect {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
};

@group(0) @binding(0) var<uniform> cull: Cull;
@group(0) @binding(1) var<storage, read> draws: array<Draw>;
@group(0) @binding(2) var<storage, read> cull_origins: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> indirect: array<Indirect>;

// Whether the box from `lo` of side `side` is on the inner side of every plane (the corner
// farthest along each plane's normal tested): the CPU twin is `world::visible`.
fn box_visible(lo: vec3<f32>, side: f32) -> bool {
    for (var p = 0u; p < 6u; p = p + 1u) {
        let plane = cull.planes[p];
        let far = lo + select(vec3<f32>(0.0), vec3<f32>(side), plane.xyz > vec3<f32>(0.0));
        if dot(plane.xyz, far) + plane.w < 0.0 {
            return false;
        }
    }
    return true;
}

@compute @workgroup_size(64)
fn cs_cull(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= u32(cull.sizes.y) {
        return;
    }
    let d = draws[i];
    let seen = box_visible(cull_origins[d.slot].xyz, cull.sizes.x);
    indirect[i] = Indirect(d.count, select(0u, 1u, seen), 0u, 0, 0u);
}

