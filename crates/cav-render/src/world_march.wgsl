// Cavendish S4: the march for its parity test, one thread a ray (`gpu_brick_march_matches_the_cpu`).

@group(0) @binding(2) var<storage, read> bricks: array<u32>;

struct Ray {
    start: vec4<f32>,
    dir: vec4<f32>,
    // x: the brick's slot; y: the entry face.
    info: vec4<u32>,
};

struct Found {
    hit: u32,
    t: f32,
    face: u32,
    steps: u32,
    cell: vec4<i32>,
};

@group(0) @binding(0) var<storage, read> rays: array<Ray>;
@group(0) @binding(1) var<storage, read_write> found: array<Found>;

@compute @workgroup_size(64)
fn cs_march(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= arrayLength(&rays) {
        return;
    }
    let r = rays[i];
    let h = march(r.info.x * BRICK_WORDS, r.start.xyz, r.dir.xyz, r.info.y);
    found[i] = Found(h.hit, h.t, h.face, h.steps, vec4<i32>(h.cell, 0));
}
