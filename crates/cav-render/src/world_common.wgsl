// Cavendish S4: what the far field's ray-march and its parity kernel share; the module that
// includes it declares `bricks: array<u32>`, the brick words.

const BRICK_WORDS: u32 = 1042u;
const MASK_AT: u32 = 1024u;
const MATERIALS_AT: u32 = 1026u;
// The most cells and sub-blocks one ray visits: 10 sub-blocks, 22 cells in each, and a margin.
const MAX_MARCH_STEPS: u32 = 256u;

fn axis_normal(face: u32) -> vec3<f32> {
    // 0 −x, 1 +x, 2 −y, 3 +y, 4 −z, 5 +z
    let axis = face / 2u;
    let sign = select(-1.0, 1.0, face % 2u == 1u);
    var n = vec3<f32>(0.0);
    n[axis] = sign;
    return n;
}

struct Hit {
    // 1 when the ray met an occupied cell.
    hit: u32,
    // Where, in cells from the ray's start.
    t: f32,
    cell: vec3<i32>,
    // The face it came in through (as `axis_normal`).
    face: u32,
    steps: u32,
};

fn cell_set(base: u32, c: vec3<i32>) -> bool {
    let i = u32(c.x + 32 * c.y + 1024 * c.z);
    return ((bricks[base + i / 32u] >> (i % 32u)) & 1u) == 1u;
}

fn sub_index(s: vec3<i32>) -> u32 {
    return u32(s.x + 4 * s.y + 16 * s.z);
}

fn sub_set(base: u32, s: vec3<i32>) -> bool {
    let i = sub_index(s);
    return ((bricks[base + MASK_AT + i / 32u] >> (i % 32u)) & 1u) == 1u;
}

fn sub_material(base: u32, s: vec3<i32>) -> u32 {
    let i = sub_index(s);
    return (bricks[base + MATERIALS_AT + i / 4u] >> (8u * (i % 4u))) & 255u;
}

fn face_of(axis: u32, step: i32) -> u32 {
    // Entering a cell by stepping +x means coming in through its −x face.
    return axis * 2u + select(1u, 0u, step > 0);
}

// The smallest component's axis.
fn least(v: vec3<f32>) -> u32 {
    if v.x <= v.y && v.x <= v.z {
        return 0u;
    }
    if v.y <= v.z {
        return 1u;
    }
    return 2u;
}

// Marches the brick at `base` from `start` (cells, inside [0, 32]³) along `dir` (cells per unit
// t, not zero), entering through `entry_face`: sub-block by sub-block over the 4³ mask, cell by
// cell inside an occupied sub-block. The CPU twin is `world::march`, the same steps in f32.
fn march(base: u32, start: vec3<f32>, dir: vec3<f32>, entry_face: u32) -> Hit {
    var out = Hit(0u, 0.0, vec3<i32>(0), entry_face, 0u);
    let step = vec3<i32>(select(vec3<i32>(-1), vec3<i32>(1), dir > vec3<f32>(0.0)));
    let inv = vec3<f32>(1.0) / dir;
    let up = dir > vec3<f32>(0.0);
    let p0 = clamp(start, vec3<f32>(0.0), vec3<f32>(31.9999));
    var s = vec3<i32>(floor(p0 / 8.0));
    var t = 0.0;
    var face = entry_face;
    var coarse_next = (vec3<f32>(s * 8 + select(vec3<i32>(0), vec3<i32>(8), up)) - start) * inv;
    let coarse_delta = abs(8.0 * inv);
    for (var n = 0u; n < MAX_MARCH_STEPS; n = n + 1u) {
        out.steps = out.steps + 1u;
        if sub_set(base, s) {
            // Cell by cell inside this sub-block, from where the ray is now.
            let here = clamp(start + dir * t, vec3<f32>(s * 8), vec3<f32>(s * 8) + vec3<f32>(7.9999));
            var c = vec3<i32>(floor(here));
            var fine_next = (vec3<f32>(c + select(vec3<i32>(0), vec3<i32>(1), up)) - start) * inv;
            let fine_delta = abs(inv);
            var tf = t;
            var ff = face;
            for (var m = 0u; m < 24u; m = m + 1u) {
                out.steps = out.steps + 1u;
                if cell_set(base, c) {
                    out.hit = 1u;
                    out.t = tf;
                    out.cell = c;
                    out.face = ff;
                    return out;
                }
                let a = least(fine_next);
                tf = fine_next[a];
                fine_next[a] = fine_next[a] + fine_delta[a];
                c[a] = c[a] + step[a];
                ff = face_of(a, step[a]);
                if c[a] < s[a] * 8 || c[a] >= s[a] * 8 + 8 {
                    break;
                }
            }
        }
        let a = least(coarse_next);
        t = coarse_next[a];
        coarse_next[a] = coarse_next[a] + coarse_delta[a];
        s[a] = s[a] + step[a];
        face = face_of(a, step[a]);
        if s[a] < 0 || s[a] >= 4 {
            return out;
        }
    }
    return out;
}

