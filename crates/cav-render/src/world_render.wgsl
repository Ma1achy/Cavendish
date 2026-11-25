// Cavendish S4: the world volume. Near: greedy-meshed chunks, culled on the GPU and drawn
// indirectly. Far: bricks of the clipmap ray-marched in their boxes. Every position is an offset
// from the view's target, metres (REBUILD §3.4): chunk and brick origins are formed in f64 on the
// CPU and rounded here.

struct World {
    view_proj: mat4x4<f32>,
    // The camera's eye relative to the target (xyz); w: 1 for an orthographic view.
    eye: vec4<f32>,
    // The direction the camera looks (xyz).
    forward: vec4<f32>,
    // x: the finest cell, metres; y: 1 to colour by level (the LOD bands); z: 1 when lit.
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> w: World;
// Each material's colour, linear RGB, by world index (alpha unused).
@group(0) @binding(1) var<uniform> palette: array<vec4<f32>, 256>;
// Each chunk slot's origin relative to the target (xyz), metres.
@group(0) @binding(2) var<storage, read> origins: array<vec4<f32>>;
// The bricks: BRICK_WORDS words each (32 768 occupancy bits, the 4³ sub-block mask, the 64
// sub-block materials packed four to a word).
@group(0) @binding(3) var<storage, read> bricks: array<u32>;


// A colour for level k of the clipmap (the LOD bands): a categorical set, so that neighbouring
// levels differ in hue (Okabe and Ito's, in linear light, repeating after eight).
fn band(level: u32) -> vec3<f32> {
    var colours = array<vec3<f32>, 8>(
        vec3<f32>(0.91, 0.62, 0.0),
        vec3<f32>(0.0, 0.29, 0.69),
        vec3<f32>(0.0, 0.42, 0.25),
        vec3<f32>(0.79, 0.15, 0.0),
        vec3<f32>(0.47, 0.13, 0.36),
        vec3<f32>(0.08, 0.53, 0.84),
        vec3<f32>(0.83, 0.79, 0.04),
        vec3<f32>(0.62, 0.25, 0.09));
    return colours[level % 8u];
}

fn shade(base: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    if w.params.z < 0.5 {
        return base;
    }
    let key = max(dot(normal, normalize(vec3<f32>(-0.45, -0.55, 0.70))), 0.0);
    return base * (0.38 + 0.62 * key);
}

// ---- near field: meshed chunks ----------------------------------------------------------------

struct MeshOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) @interpolate(flat) face: u32,
    @location(1) @interpolate(flat) material: u32,
};

@vertex
fn vs_chunk(@location(0) cell: vec4<u32>, @location(1) attributes: vec4<u32>) -> MeshOut {
    let origin = origins[cell.w].xyz;
    let position = origin + vec3<f32>(cell.xyz) * w.params.x;
    var out: MeshOut;
    out.clip = w.view_proj * vec4<f32>(position, 1.0);
    out.face = attributes.x;
    out.material = attributes.y;
    return out;
}

@fragment
fn fs_chunk(in: MeshOut) -> @location(0) vec4<f32> {
    var base = palette[in.material].rgb;
    if w.params.y > 0.5 {
        base = band(0u);
    }
    return vec4<f32>(shade(base, axis_normal(in.face)), 1.0);
}

struct BrickOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) local: vec3<f32>,
    @location(1) @interpolate(flat) origin: vec3<f32>,
    @location(2) @interpolate(flat) params: vec4<f32>,
    @location(3) @interpolate(flat) slot: u32,
};

// The 36 corners of a cube's twelve triangles, as bits of the corner index (x, y, z).
fn corner(i: u32) -> vec3<f32> {
    var faces = array<u32, 36>(
        0u, 2u, 1u, 1u, 2u, 3u, 4u, 5u, 6u, 5u, 7u, 6u,
        0u, 1u, 4u, 1u, 5u, 4u, 2u, 6u, 3u, 3u, 6u, 7u,
        0u, 4u, 2u, 2u, 4u, 6u, 1u, 3u, 5u, 3u, 7u, 5u);
    let c = faces[i];
    return vec3<f32>(f32(c & 1u), f32((c >> 1u) & 1u), f32((c >> 2u) & 1u));
}

@vertex
fn vs_brick(
    @builtin(vertex_index) vertex: u32,
    @location(0) origin: vec4<f32>,
    @location(1) info: vec4<u32>,
) -> BrickOut {
    // origin.xyz: the brick's corner relative to the target; origin.w: its cell, metres.
    // info.x: its slot; info.y: its level.
    let side = origin.w * 32.0;
    let local = origin.xyz + corner(vertex) * side;
    var out: BrickOut;
    out.clip = w.view_proj * vec4<f32>(local, 1.0);
    out.local = local;
    out.origin = origin.xyz;
    out.params = vec4<f32>(origin.w, f32(info.y), 0.0, 0.0);
    out.slot = info.x;
    return out;
}

struct FragOut {
    @location(0) colour: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fs_brick(in: BrickOut) -> FragOut {
    let cell = in.params.x;
    let side = cell * 32.0;
    // The ray: from the eye (perspective) or along the view through this point (orthographic).
    var ray_from = w.eye.xyz;
    var dir = nonzero(normalize(in.local - w.eye.xyz));
    if w.eye.w > 0.5 {
        // Orthographic: a named view looks along an axis, so two components are zero.
        dir = nonzero(w.forward.xyz);
        ray_from = in.local - dir * (4.0 * side + length(in.local - in.origin));
    }
    // Into the brick's box: [origin, origin + side]³.
    let inv = vec3<f32>(1.0) / dir;
    let t0 = (in.origin - ray_from) * inv;
    let t1 = (in.origin + vec3<f32>(side) - ray_from) * inv;
    let tmin = min(t0, t1);
    let tmax = max(t0, t1);
    var enter = max(max(tmin.x, tmin.y), tmin.z);
    let leave = min(min(tmax.x, tmax.y), tmax.z);
    var entry_face = 0u;
    if tmin.x >= tmin.y && tmin.x >= tmin.z {
        entry_face = face_of(0u, select(-1, 1, dir.x > 0.0));
    } else if tmin.y >= tmin.z {
        entry_face = face_of(1u, select(-1, 1, dir.y > 0.0));
    } else {
        entry_face = face_of(2u, select(-1, 1, dir.z > 0.0));
    }
    enter = max(enter, 0.0);
    if enter > leave {
        discard;
    }
    let start = (ray_from + dir * enter - in.origin) / cell;
    let base = in.slot * BRICK_WORDS;
    let hit = march(base, start, dir / cell, entry_face);
    if hit.hit == 0u {
        discard;
    }
    let at = ray_from + dir * (enter + hit.t);
    let clip = w.view_proj * vec4<f32>(at, 1.0);
    var base_colour = palette[sub_material(base, hit.cell / 8)].rgb;
    if w.params.y > 0.5 {
        base_colour = band(u32(in.params.y));
    }
    var out: FragOut;
    out.colour = vec4<f32>(shade(base_colour, axis_normal(hit.face)), 1.0);
    out.depth = clamp(clip.z / clip.w, 0.0, 1.0);
    return out;
}

