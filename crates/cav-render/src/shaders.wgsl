// Cavendish S0 shaders: lit triangles, thick lines and the infinite ground grid.
// Every position is an offset from the view's target, in metres (REBUILD §3.4).

struct Globals {
    view_proj: mat4x4<f32>,
    // Camera position relative to the target (xyz); w: 1 when surfaces are lit.
    eye_lit: vec4<f32>,
    // Viewport size in pixels (xy); ground height (z); the far fade distance (w).
    viewport_ground: vec4<f32>,
    // The target's x and y modulo the grid period (xy).
    phase: vec4<f32>,
    ground_surface: vec4<f32>,
    ground_line: vec4<f32>,
    background: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;

// ---- triangles ----------------------------------------------------------------------------

struct MeshOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) colour: vec4<f32>,
    @location(2) offset: vec3<f32>,
};

@vertex
fn vs_mesh(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) colour: vec4<f32>) -> MeshOut {
    var out: MeshOut;
    out.clip = g.view_proj * vec4<f32>(position, 1.0);
    out.normal = normal;
    out.colour = colour;
    out.offset = position;
    return out;
}

@fragment
fn fs_mesh(in: MeshOut) -> @location(0) vec4<f32> {
    if g.eye_lit.w < 0.5 {
        return in.colour;
    }
    // A key light from the south-west and above, a weak fill from the camera, and ambient.
    let n = normalize(in.normal);
    let key = max(dot(n, normalize(vec3<f32>(-0.45, -0.55, 0.70))), 0.0);
    let fill = max(dot(n, normalize(g.eye_lit.xyz - in.offset)), 0.0);
    let light = 0.32 + 0.58 * key + 0.22 * fill;
    return vec4<f32>(in.colour.rgb * light, in.colour.a);
}

// ---- thick lines ---------------------------------------------------------------------------
// Each instance is a segment; six vertices make a quad facing the screen, `width` pixels wide.

struct LineOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) colour: vec4<f32>,
};

@vertex
fn vs_line(
    @builtin(vertex_index) index: u32,
    @location(0) a_width: vec4<f32>,
    @location(1) b_pad: vec4<f32>,
    @location(2) colour: vec4<f32>,
) -> LineOut {
    var out: LineOut;
    out.colour = colour;
    var a = g.view_proj * vec4<f32>(a_width.xyz, 1.0);
    var b = g.view_proj * vec4<f32>(b_pad.xyz, 1.0);
    // Clip the segment to the near plane (z = 0 in clip space) before dividing by w.
    if a.z < 0.0 && b.z < 0.0 {
        out.clip = vec4<f32>(0.0, 0.0, -1.0, 1.0);
        return out;
    }
    if a.z < 0.0 {
        a = mix(a, b, a.z / (a.z - b.z));
    } else if b.z < 0.0 {
        b = mix(b, a, b.z / (b.z - a.z));
    }
    let viewport = g.viewport_ground.xy;
    let pa = a.xy / a.w * viewport;
    let pb = b.xy / b.w * viewport;
    var along = pb - pa;
    let span = length(along);
    if span > 1e-6 {
        along = along / span;
    } else {
        along = vec2<f32>(1.0, 0.0);
    }
    let across = vec2<f32>(-along.y, along.x);
    // Vertices 0..5: (end, side) = (a,-) (a,+) (b,-) / (b,-) (a,+) (b,+).
    var ends = array<f32, 6>(0.0, 0.0, 1.0, 1.0, 0.0, 1.0);
    var sides = array<f32, 6>(-1.0, 1.0, -1.0, -1.0, 1.0, 1.0);
    let at_b = ends[index];
    var clip = a;
    if at_b > 0.5 {
        clip = b;
    }
    // Half the width across, and half a pixel of cap along, in pixels; back to clip space.
    let shift = across * sides[index] * 0.5 * a_width.w + along * (at_b * 2.0 - 1.0) * 0.5;
    out.clip = vec4<f32>(clip.xy + shift / viewport * 2.0 * clip.w, clip.z, clip.w);
    return out;
}

@fragment
fn fs_line(in: LineOut) -> @location(0) vec4<f32> {
    return in.colour;
}

// ---- the infinite ground -------------------------------------------------------------------
// One big quad at the ground's height; the grid is computed per pixel from the offset, so it has
// no vertices to run out of and no large coordinates.

struct GroundOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) offset: vec2<f32>,
};

@vertex
fn vs_ground(@builtin(vertex_index) index: u32) -> GroundOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let extent = g.viewport_ground.w * 1.05;
    // Centred under the camera, so the fade is a circle about where you stand.
    let xy = g.eye_lit.xy + corners[index] * extent;
    var out: GroundOut;
    out.clip = g.view_proj * vec4<f32>(xy, g.viewport_ground.z, 1.0);
    out.offset = xy;
    return out;
}

// The strength of a grid of the given period at this pixel, 0 to 1: one-pixel lines, fading out
// when the cells become smaller than a few pixels.
fn grid(coordinate: vec2<f32>, period: f32) -> f32 {
    let c = coordinate / period;
    let width = fwidth(c);
    let apart = abs(fract(c - 0.5) - 0.5) / max(width, vec2<f32>(1e-6));
    let stroke = 1.0 - min(min(apart.x, apart.y), 1.0);
    let density = max(width.x, width.y);
    return stroke * (1.0 - smoothstep(0.08, 0.35, density));
}

@fragment
fn fs_ground(in: GroundOut) -> @location(0) vec4<f32> {
    let coordinate = in.offset + g.phase.xy;
    let fine = grid(coordinate, 1.0) * 0.55;
    let coarse = grid(coordinate, 10.0) * 0.8;
    let coarsest = grid(coordinate, 100.0);
    let strength = max(max(fine, coarse), coarsest);
    let colour = mix(g.ground_surface.rgb, g.ground_line.rgb, strength);
    // Fade into the background towards the horizon.
    let range = length(in.offset - g.eye_lit.xy) / g.viewport_ground.w;
    let fade = 1.0 - smoothstep(0.25, 1.0, range);
    return vec4<f32>(mix(g.background.rgb, colour, fade), 1.0);
}
