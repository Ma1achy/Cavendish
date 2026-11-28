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
//! once), each held to a [`GpuCaps`]. What does not fit is not drawn (the nearest are), and
//! [`WorldGpu::notes`] says so: the application shows each note as a badge in the scene view's
//! header and logs it once. At a mesh cap nothing more is meshed until what is wanted changes.
//! The far field's work a frame is [`MAX_BRICK_OVERDRAW`] times the pane's pixels, spent on the
//! bricks in the camera's frustum, nearest first ([`choose_bricks`]).
//!
//! **The handover** (TASK-S4-26). When the camera moves, part of the world changes level: chunks
//! leave level 0 for a brick of level 1, bricks give way to finer ones. The streaming drops what is
//! no longer wanted at once and makes the rest over later frames (a few a frame in a page), and the
//! meshes are made a few a frame, so the region would be a hole until they are: the owner saw
//! chunks go black and come back. So a GPU copy whose source the streaming let go is kept,
//! retired, and drawn in place of what is wanted there but not ready
//! ([`cav_sim::world::clipmap::stand_ins`]); it goes when it has stood in for nothing for
//! [`RETIRED_PREPARES`] prepares, or first when its room is needed. Held by
//! `gpu_a_moving_camera_never_loses_what_it_drew` (control: [`WorldGpu::without_stand_ins`]).

use std::collections::HashMap;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use cav_core::maths::Mat4;
use cav_core::voxel::world::{BRICK_SIDE, Brick, CHUNK_SIDE, ChunkCells, ChunkKey, cell_index};
use cav_gpu::Gpu;
use cav_gpu::wgpu::{self, util::DeviceExt};
use cav_sim::ViewSpec;
use cav_sim::world::clipmap::{WorldFrame, levels, lod_centre, stand_ins};

use crate::camera::Camera;

/// The most chunk meshes on the GPU at once.
pub const MAX_CHUNK_SLOTS: usize = 8192;
/// The most bricks on the GPU at once.
pub const MAX_BRICK_SLOTS: usize = 8192;
/// The most vertices of chunk meshes on the GPU at once (12 bytes each).
pub const MAX_WORLD_VERTICES: usize = 12 << 20;
/// How many prepares a retired GPU copy (of a chunk or brick the streaming has let go) is kept
/// while it stands in for nothing ([`stand_ins`]): a few frames of every pane sharing it, so
/// that one pane's frame does not let go what another's is drawing.
pub const RETIRED_PREPARES: u32 = 8;
/// The most chunk meshes made in one frame of a window (a picture taken once makes them all).
pub const MAX_MESHES_PER_DRAW: usize = 96;
/// The longest a frame of a window spends making chunk meshes, milliseconds: it makes no new
/// batch past it (eight meshes a thread a batch; at least one batch). A mesh is about 0.08 ms on
/// one thread of the build PC since TASK-S4-27, so natively [`MAX_MESHES_PER_DRAW`] binds first;
/// in a page, which has one thread, this does (`docs/slices/S4.md` §7.10).
pub const MESH_MILLISECONDS: f64 = 4.0;
/// The most sub-blocks and cells one ray visits: 10 sub-blocks and 22 cells in each, and a margin.
pub const MAX_MARCH_STEPS: u32 = 256;
/// The words of one brick on the GPU: 32 768 occupancy bits, the 64-bit sub-block mask, and 64
/// sub-block materials four to a word.
pub const BRICK_WORDS: usize = 1042;

/// The most pixels of bricks' boxes one frame marches, over the pane's own pixels. Every pixel of
/// a box runs the march (it writes its own depth, so the GPU cannot skip hidden ones), so this is
/// what bounds one submission's far-field work: a frame that would march more draws its nearest
/// visible bricks (those in the camera's frustum: bricks behind it are never counted) and leaves
/// the farthest out, saying so. Chosen from the flythrough over `stress-1e10` (`docs/slices/S4.md`
/// §7.4) so that a typical frame stays well under the ~50 ms after which Windows' desktop stops
/// answering a busy GPU. Since the frustum fix it no longer binds in that flythrough (10× the
/// pane marched on average): the median frame is 30 ms, but 1% of frames pass 59–64 ms (worst 68
/// ms on the build PC's GTX 1660 SUPER), and nothing but the bench's 250 ms alarm bounds them. A
/// cap of 6 saved 1–2 ms, and the draw's time follows the pixels this estimate counts only weakly
/// (correlation 0.13–0.17 over the flythrough's frames): where the time goes is not yet measured
/// (`plan/IDEAS.md`, early depth for the bricks). The bench's `--max-frame-ms 250` is not this limit: it is the alarm that stops a
/// run whose frames have gone far past it, before the desktop freezes (TDR is at 2 s).
///
/// The budget is a multiple of the pane's own pixels, so a 1440p pane marches up to four times
/// what a 720p one does; per pixel the work is the same, which is what keeps the picture alike at
/// every size, and the frame's time grows with the pane as any fill-bound pass does.
pub const MAX_BRICK_OVERDRAW: f64 = 12.0;

/// About how many pixels a brick of side `side` metres at `distance` metres from the eye covers
/// in a pane `height` pixels high: its side over the pane's height at that distance, squared, and
/// twice that for the box's visible faces. In an orthographic view the distance does not matter.
#[must_use]
pub fn brick_pixels(side: f64, distance: f64, camera: &Camera, view: &ViewSpec, height: u32) -> f64 {
    let across = match view.projection {
        cav_sim::Projection::Perspective => {
            let slope = camera.half_height / camera.eye.length().max(1e-9);
            2.0 * slope * distance.max(side)
        }
        cav_sim::Projection::Orthographic => 2.0 * camera.half_height,
    };
    let span = side / across.max(1e-9) * f64::from(height);
    2.0 * span * span
}

/// How many of `areas` (pixels, nearest first) fit `budget` pixels together: the longest prefix
/// whose sum is within it (none when the first alone is over).
#[must_use]
pub fn within_budget(areas: &[f64], budget: f64) -> usize {
    let mut sum = 0.0;
    areas
        .iter()
        .take_while(|a| {
            sum += **a;
            sum <= budget
        })
        .count()
}

/// A brick the far field might march this frame: its distance from the eye (metres), the pixels
/// its box covers ([`brick_pixels`]) and whether its box is in the camera's frustum ([`visible`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrickCandidate {
    /// From the eye to the brick's centre, metres.
    pub distance: f64,
    /// About how many pixels its box covers.
    pub pixels: f64,
    /// Whether its box is in the camera's frustum.
    pub visible: bool,
}

/// Which of `candidates` to march within `budget` pixels: the visible ones only, nearest first,
/// the longest prefix that fits ([`within_budget`]). Returns their indices in that order, and how
/// many visible ones were left out. A brick outside the frustum is never drawn, so it never takes
/// a visible one's place in the budget.
#[must_use]
pub fn choose_bricks(candidates: &[BrickCandidate], budget: f64) -> (Vec<usize>, usize) {
    let mut order: Vec<usize> = (0..candidates.len()).filter(|&i| candidates[i].visible).collect();
    order.sort_by(|&a, &b| candidates[a].distance.total_cmp(&candidates[b].distance).then(a.cmp(&b)));
    let areas: Vec<f64> = order.iter().map(|&i| candidates[i].pixels).collect();
    let keep = within_budget(&areas, budget);
    let left = order.len() - keep;
    order.truncate(keep);
    (order, left)
}

/// The caps a [`WorldGpu`] holds itself to: the constants unless a test asks for smaller
/// ([`WorldGpu::with_caps`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpuCaps {
    /// The most chunk meshes on the GPU ([`MAX_CHUNK_SLOTS`]).
    pub meshes: usize,
    /// The most mesh vertices on the GPU ([`MAX_WORLD_VERTICES`]).
    pub vertices: usize,
    /// The most bricks on the GPU ([`MAX_BRICK_SLOTS`]).
    pub bricks: usize,
}

impl Default for GpuCaps {
    fn default() -> Self {
        Self { meshes: MAX_CHUNK_SLOTS, vertices: MAX_WORLD_VERTICES, bricks: MAX_BRICK_SLOTS }
    }
}

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

/// The unit step along axis `axis` (0 x, 1 y, 2 z): the reference mesher's.
#[cfg(test)]
const fn along(axis: usize) -> [i32; 3] {
    let mut v = [0; 3];
    v[axis] = 1;
    v
}

/// The chunk's exposed faces as rectangles, merged greedily per slice and material: a face of an
/// occupied cell is exposed when the cell beyond it is empty, looked up in the neighbouring chunk
/// across the chunk's side (`neighbours`, in face order −x, +x, −y, +y, −z, +z; a missing one is
/// air; only its layer against this chunk is read, so [`ChunkCells::materials_layer`] will do).
/// `materials` is the chunk's cells as [`ChunkCells::materials`] gives them (material + 1, 0
/// empty); every vertex carries `slot`.
///
/// Which faces are exposed is found 34 cells at a time, as bits (TASK-S4-27): a row of the padded
/// chunk along x is one `u64`, and a row's exposed faces are its occupied bits without the
/// occupied bits beyond them. Only the slices and rows with an exposed face are then merged, cell
/// by cell, in the same order as before, so the rectangles are the same, in the same order, as
/// the cell-by-cell mesher's (held by `the_bit_mesher_is_the_cell_mesher`).
#[must_use]
pub fn greedy_mesh(materials: &[u8], neighbours: [Option<&[u8]>; 6], slot: u16) -> Mesh {
    let n = CHUNK_SIDE as i32;
    let nu = CHUNK_SIDE as usize;
    // The chunk with a one-cell rim from its neighbours (34³, x fastest), so that every look-up
    // below is one index.
    const P: i32 = CHUNK_SIDE as i32 + 2;
    const PU: usize = P as usize;
    let mut padded = vec![0u8; PU * PU * PU];
    let pad = |c: [i32; 3]| ((c[0] + 1) + P * ((c[1] + 1) + P * (c[2] + 1))) as usize;
    for z in 0..n {
        for y in 0..n {
            let row = cell_index([0, y as u32, z as u32]);
            let at = pad([0, y, z]);
            padded[at..at + nu].copy_from_slice(&materials[row..row + nu]);
        }
    }
    for (face, cells) in neighbours.iter().enumerate() {
        let Some(cells) = cells else { continue };
        let axis = face / 2;
        let (inside, outside) = if face % 2 == 1 { (0, n) } else { (n - 1, -1) };
        for b in 0..n {
            for a in 0..n {
                let (mut from, mut to) = ([0; 3], [0; 3]);
                from[axis] = inside;
                to[axis] = outside;
                from[(axis + 1) % 3] = a;
                to[(axis + 1) % 3] = a;
                from[(axis + 2) % 3] = b;
                to[(axis + 2) % 3] = b;
                padded[pad(to)] = cells[cell_index(from.map(|v| v as u32))];
            }
        }
    }
    // Occupancy as bits: `occupied[y + P z]` has bit `x` set when padded cell (x, y, z) holds
    // matter (padded coordinates, 0 to 33: chunk cell c is padded c + 1).
    let mut occupied = vec![0u64; PU * PU];
    for (at, bits) in occupied.iter_mut().enumerate() {
        let cells = &padded[at * PU..at * PU + PU];
        *bits = cells.iter().enumerate().fold(0u64, |acc, (x, &m)| acc | (u64::from(m != 0) << x));
    }
    let row = |y: usize, z: usize| occupied[y + PU * z];
    // The chunk's own cells in a row: padded bits 1 to 32.
    const INSIDE: u64 = 0xFFFF_FFFF << 1;
    let mut mesh = Mesh::default();
    let mut mask = vec![0u8; nu * nu];
    let mut exposed = vec![0u64; PU * PU];
    for face in 0..6usize {
        let axis = face / 2;
        let positive = face % 2 == 1;
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        // This face's exposed cells, as rows along x (padded rows 1 to 32 only).
        let mut any = 0u64;
        for z in 1..=nu {
            for y in 1..=nu {
                let here = row(y, z);
                let beyond = match (axis, positive) {
                    (0, true) => here >> 1,
                    (0, false) => here << 1,
                    (1, true) => row(y + 1, z),
                    (1, false) => row(y - 1, z),
                    (_, true) => row(y, z + 1),
                    (_, false) => row(y, z - 1),
                };
                let bits = here & !beyond & INSIDE;
                exposed[y + PU * z] = bits;
                any |= bits;
            }
        }
        if any == 0 {
            continue;
        }
        // Chunk cell (x, y, z): whether its face is exposed, and its material.
        let bit = |x: usize, y: usize, z: usize| (exposed[(y + 1) + PU * (z + 1)] >> (x + 1)) & 1 == 1;
        let material = |x: usize, y: usize, z: usize| padded[(x + 1) + PU * ((y + 1) + PU * (z + 1))];
        for slice in 0..nu {
            // The slice's mask (a along u, b along v), and which of its rows b hold a face.
            let mut rows = [false; CHUNK_SIDE as usize];
            match axis {
                // u = y, v = z: the slice is x = slice.
                0 => {
                    if (any >> (slice + 1)) & 1 == 0 {
                        continue;
                    }
                    for b in 0..nu {
                        for a in 0..nu {
                            if bit(slice, a, b) {
                                mask[a + nu * b] = material(slice, a, b);
                                rows[b] = true;
                            }
                        }
                    }
                }
                // u = z, v = x: the slice is y = slice; a row of bits runs along x, which is v.
                1 => {
                    for a in 0..nu {
                        let mut bits = exposed[(slice + 1) + PU * (a + 1)] >> 1;
                        while bits != 0 {
                            let b = bits.trailing_zeros() as usize;
                            bits &= bits - 1;
                            mask[a + nu * b] = material(b, slice, a);
                            rows[b] = true;
                        }
                    }
                }
                // u = x, v = y: the slice is z = slice; a row of bits runs along x, which is u.
                _ => {
                    for (b, filled) in rows.iter_mut().enumerate() {
                        let mut bits = exposed[(b + 1) + PU * (slice + 1)] >> 1;
                        *filled = bits != 0;
                        while bits != 0 {
                            let a = bits.trailing_zeros() as usize;
                            bits &= bits - 1;
                            mask[a + nu * b] = material(a, b, slice);
                        }
                    }
                }
            }
            if !rows.contains(&true) {
                continue;
            }
            // Greedy rectangles: widest first along u, then as tall as the row allows along v.
            // Every set cell of the mask is taken, so the mask is all zero again after.
            let slice = slice as i32;
            for b in 0..n {
                if !rows[b as usize] {
                    continue;
                }
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

/// [`greedy_mesh`] as it was before TASK-S4-27, cell by cell: the reference its test holds it to.
/// The chunk's exposed faces as rectangles, merged greedily per slice and material: a face of an
/// occupied cell is exposed when the cell beyond it is empty, looked up in the neighbouring chunk
/// across the chunk's side (`neighbours`, in face order −x, +x, −y, +y, −z, +z; a missing one is
/// air). `materials` is the chunk's cells as [`ChunkCells::materials`] gives them (material + 1,
/// 0 empty); every vertex carries `slot`.
#[cfg(test)]
#[must_use]
fn greedy_mesh_by_cells(materials: &[u8], neighbours: [Option<&[u8]>; 6], slot: u16) -> Mesh {
    let n = CHUNK_SIDE as i32;
    // The chunk with a one-cell rim from its neighbours (34³, x fastest), so that every look-up
    // below is one index: what made this 10 to 20 times faster than a look-up that asked which
    // chunk a cell is in.
    const P: i32 = CHUNK_SIDE as i32 + 2;
    let mut padded = vec![0u8; (P * P * P) as usize];
    let pad = |c: [i32; 3]| ((c[0] + 1) + P * ((c[1] + 1) + P * (c[2] + 1))) as usize;
    for z in 0..n {
        for y in 0..n {
            let row = cell_index([0, y as u32, z as u32]);
            let at = pad([0, y, z]);
            padded[at..at + n as usize].copy_from_slice(&materials[row..row + n as usize]);
        }
    }
    for (face, cells) in neighbours.iter().enumerate() {
        let Some(cells) = cells else { continue };
        let axis = face / 2;
        let (inside, outside) = if face % 2 == 1 { (0, n) } else { (n - 1, -1) };
        for b in 0..n {
            for a in 0..n {
                let (mut from, mut to) = ([0; 3], [0; 3]);
                from[axis] = inside;
                to[axis] = outside;
                from[(axis + 1) % 3] = a;
                to[(axis + 1) % 3] = a;
                from[(axis + 2) % 3] = b;
                to[(axis + 2) % 3] = b;
                padded[pad(to)] = cells[cell_index(from.map(|v| v as u32))];
            }
        }
    }
    let at = |c: [i32; 3]| -> u8 { padded[pad(c)] };
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

/// The least a ray direction's component may be, in magnitude: a zero one (a ray along an
/// axis, as in an orthographic named view) is taken as this, its sign positive. Twin of
/// `MIN_DIR` in `world_common.wgsl`.
pub const MIN_DIR: f32 = 1e-6;

/// `v` with each component at least [`MIN_DIR`] from zero, its sign kept (zero as positive).
#[must_use]
pub fn nonzero(v: [f32; 3]) -> [f32; 3] {
    v.map(|c| if c >= 0.0 { c.max(MIN_DIR) } else { c.min(-MIN_DIR) })
}

/// Marches `words` (one brick, [`brick_words`]) from `start` (cells, inside `[0, 32]³`) along
/// `dir` (cells per unit of t; a zero component is taken as [`MIN_DIR`], see [`nonzero`]),
/// entering through `entry_face`: sub-block by sub-block over the 4³ mask, cell by cell inside an
/// occupied one, at most [`MAX_MARCH_STEPS`] visits. The CPU twin of `march` in
/// `world_common.wgsl`, step for step in `f32`.
#[must_use]
pub fn march(words: &[u32], start: [f32; 3], dir: [f32; 3], entry_face: u32) -> MarchHit {
    let dir = nonzero(dir);
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
    /// Which of its six neighbours were resident when it was made (a missing one is air): a
    /// mesh is made again when that changes, so faces a neighbour buries are not drawn.
    around: [bool; 6],
}

/// The six neighbours of a chunk, in face order (−x, +x, −y, +y, −z, +z).
const AROUND: [[i32; 3]; 6] = [[-1, 0, 0], [1, 0, 0], [0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1]];

/// Which of `key`'s neighbours are resident in `frame`, in face order.
fn around(frame: &WorldFrame, key: ChunkKey) -> [bool; 6] {
    AROUND.map(|d| frame.chunks.contains_key(&ChunkKey::new(key.x + d[0], key.y + d[1], key.z + d[2])))
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
///
/// One is shared by every pane of a renderer: [`Self::notes`], [`Self::incomplete`] and
/// [`Self::marched`] describe the last pane prepared.
#[derive(Default)]
pub struct WorldGpu {
    caps: GpuCaps,
    world: u64,
    meshes: HashMap<ChunkKey, GpuMesh>,
    /// Chunks whose mesh is empty (buried whole), by what they were made from and which
    /// neighbours were resident then.
    empty: HashMap<ChunkKey, (Arc<ChunkCells>, [bool; 6])>,
    free_meshes: Vec<u32>,
    /// When a mesh cap stopped the meshing: a fingerprint of what was wanted then (the chunks
    /// to mesh, the vertices and slots in use) and the note said. While the same is wanted,
    /// nothing is meshed again (the meshes would be thrown away again); a freed slot, fewer
    /// vertices or another set of chunks starts it again.
    full: Option<(u64, Vec<String>)>,
    /// How many chunk meshes have been made (for tests: a frame at a cap makes none).
    pub meshed: u64,
    /// How long the last prepare spent making meshes and their buffers, milliseconds (for the
    /// flythrough's trace).
    pub mesh_ms: f64,
    vertices: usize,
    bricks: HashMap<(u8, ChunkKey), (u32, Arc<Brick>)>,
    free_bricks: Vec<u32>,
    store: Option<wgpu::Buffer>,
    /// What could not be drawn this frame, and why (at most a line each).
    pub notes: Vec<String>,
    /// Whether meshes wanted this frame were left for later frames ([`MAX_MESHES_PER_DRAW`]).
    pub incomplete: bool,
    /// The bricks' boxes marched last frame, in pixels over the pane's pixels (at most
    /// [`MAX_BRICK_OVERDRAW`]).
    pub marched: f64,
    /// What the last frame drew of the world, by `(level, key)`: the near chunks with a mesh (or
    /// meshed empty, buried whole: ready, with nothing to show), then the bricks marched. The
    /// chunks are drawn whether or not they are in the frustum (the GPU culls them); the bricks
    /// only when they are. For tests of what a moving camera keeps on screen; not for drawing.
    pub drawn: Vec<(u8, ChunkKey)>,
    /// GPU copies of what the streaming no longer holds, by `(level, key)` (they stay in
    /// `meshes` and `bricks`), and for how many prepares each has not stood in for anything
    /// ([`stand_ins`]).
    retired: HashMap<(u8, ChunkKey), u32>,
    /// True to draw no stand-ins: the handover as it was before TASK-S4-26, for the control arm
    /// of the test that holds it (`gpu_a_moving_camera_never_loses_what_it_drew`). Never set it
    /// otherwise.
    pub without_stand_ins: bool,
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
    /// A world's GPU copies held to `caps`, each at most its constant: for tests that reach the
    /// caps with a small world. The renderer's own is [`WorldGpu::default`]. Must not be used to
    /// raise a cap.
    #[must_use]
    pub fn with_caps(caps: GpuCaps) -> Self {
        let most = GpuCaps::default();
        let caps = GpuCaps {
            meshes: caps.meshes.min(most.meshes),
            vertices: caps.vertices.min(most.vertices),
            bricks: caps.bricks.min(most.bricks),
        };
        Self { caps, ..Self::default() }
    }

    /// Lets chunk `key`'s mesh go: its slot and vertices given back.
    fn drop_mesh(&mut self, key: ChunkKey) {
        if let Some(mesh) = self.meshes.remove(&key) {
            self.vertices -= mesh.vertex_count;
            self.free_meshes.push(mesh.slot);
        }
        self.retired.remove(&(0, key));
    }

    /// Lets brick `key` go: its slot given back.
    fn drop_brick(&mut self, key: (u8, ChunkKey)) {
        if let Some((slot, _)) = self.bricks.remove(&key) {
            self.free_bricks.push(slot);
        }
        self.retired.remove(&key);
    }

    /// Lets the retired mesh (`level` 0) or brick (`level` above 0) that has stood idle longest
    /// go, to make room. False when none is retired.
    fn retire_one(&mut self, level: u8) -> bool {
        let pick = self
            .retired
            .iter()
            .filter(|((l, _), _)| (*l == 0) == (level == 0))
            .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
            .map(|(k, _)| *k);
        match pick {
            Some((0, key)) => self.drop_mesh(key),
            Some(item) => self.drop_brick(item),
            None => return false,
        }
        true
    }

    /// Chunk meshes and bricks held, and vertices of the meshes.
    #[must_use]
    pub fn counts(&self) -> [usize; 3] {
        [self.meshes.len(), self.bricks.len(), self.vertices]
    }

    /// The near chunks whose meshes are to be made: `(stale, distance from the eye, key, cells)`,
    /// the missing first, then those whose neighbours changed since their mesh was made or that
    /// the streaming has made again (each drawn with its old mesh until then), each nearest first.
    fn to_mesh<'f>(
        &self,
        near: &[(ChunkKey, &'f Arc<ChunkCells>)],
        frame: &WorldFrame,
        eye: cav_core::Vec3,
    ) -> Vec<(bool, f64, ChunkKey, &'f Arc<ChunkCells>)> {
        let Some(grid) = frame.grid else { return Vec::new() };
        let mut todo: Vec<(bool, f64, ChunkKey, &'f Arc<ChunkCells>)> = near
            .iter()
            .filter_map(|&(key, cells)| {
                let now = around(frame, key);
                let stale = match (self.meshes.get(&key), self.empty.get(&key)) {
                    (Some(mesh), _) => mesh.around != now || !Arc::ptr_eq(&mesh.source, cells),
                    (None, Some((made, then))) if Arc::ptr_eq(made, cells) => *then != now,
                    _ => false,
                };
                let missing = !self.meshes.contains_key(&key)
                    && self.empty.get(&key).is_none_or(|(made, _)| !Arc::ptr_eq(made, cells));
                (missing || stale).then(|| (stale, (grid.chunk_box(key).centre() - eye).length(), key, cells))
            })
            .collect();
        todo.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
        todo
    }

    /// The chunks of `frame` waiting to be meshed for the camera of `view` (`camera` is its
    /// renderer camera), in the order the next frame would mesh them: for tests that the order a
    /// moving camera changes is not what stops the meshing at a cap.
    #[must_use]
    pub fn waiting(&self, frame: &WorldFrame, view: &ViewSpec, camera: &Camera) -> Vec<ChunkKey> {
        let Some(grid) = frame.grid else { return Vec::new() };
        let levels = levels(&grid, lod_centre(view), frame.ring, frame.levels.max(1));
        let near: Vec<(ChunkKey, &Arc<ChunkCells>)> =
            frame.chunks.iter().filter(|(k, _)| levels[0].contains(**k)).map(|(k, c)| (*k, c)).collect();
        self.to_mesh(&near, frame, camera.origin + camera.eye).into_iter().map(|(_, _, key, _)| key).collect()
    }

    /// What identifies a frame's meshing at a cap: the chunks to mesh and what is in use.
    /// Free of the order the chunks are listed in (nearest the eye first, which changes as the
    /// camera moves: S4's second review, J3), so a camera moving at a cap does not mesh again.
    fn fingerprint(&self, todo: &[(bool, f64, ChunkKey, &Arc<ChunkCells>)]) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let mut entries: Vec<(ChunkKey, bool, usize)> =
            todo.iter().map(|(stale, _, key, cells)| (*key, *stale, Arc::as_ptr(cells) as usize)).collect();
        entries.sort_unstable();
        for entry in &entries {
            entry.hash(&mut hasher);
        }
        (self.vertices, self.free_meshes.len()).hash(&mut hasher);
        hasher.finish()
    }

    #[allow(clippy::too_many_arguments)] // each is one thing the frame is made from
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
        pixels: [u32; 2],
        complete: bool,
    ) -> Option<WorldDraw> {
        self.prepare_list(gpu, pipelines, frame, view, camera, pixels, complete, None)
    }

    #[allow(clippy::too_many_arguments)] // each is one thing the frame is made from
    /// [`Self::prepare`] drawing `list` (a LOD view's draw list, `(level, key)`: a chunk's mesh at
    /// level 0, a brick above; `cav_sim::world::lod_view`) in place of the camera's clipmap when
    /// it is given. An entry neither resident nor waiting to be made is drawn by the nearest
    /// coarser brick that is resident, and [`Self::notes`] counts them.
    pub fn prepare_list(
        &mut self,
        gpu: &Gpu,
        pipelines: &WorldPipelines,
        frame: &WorldFrame,
        view: &ViewSpec,
        camera: &Camera,
        pixels: [u32; 2],
        complete: bool,
        list: Option<&[(u8, ChunkKey)]>,
    ) -> Option<WorldDraw> {
        let grid = frame.grid?;
        self.notes.clear();
        if self.world != frame.world {
            *self = Self {
                world: frame.world,
                store: self.store.take(),
                caps: self.caps,
                without_stand_ins: self.without_stand_ins,
                ..Self::default()
            };
        }
        if self.free_meshes.is_empty() && self.meshes.is_empty() {
            self.free_meshes = (0..self.caps.meshes as u32).rev().collect();
        }
        if self.free_bricks.is_empty() && self.bricks.is_empty() {
            self.free_bricks = (0..self.caps.bricks as u32).rev().collect();
        }
        let device = &gpu.device;
        // A handle (wgpu's buffers are shared references), so `self` stays free to borrow.
        let store = self
            .store
            .get_or_insert_with(|| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("world bricks"),
                    size: (MAX_BRICK_SLOTS * BRICK_WORDS * 4) as u64,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .clone();
        let store = &store;
        // What this camera's clipmap draws of what is resident, or what the LOD view's list names
        // (with, for an entry not resident and not on its way, its nearest resident ancestor).
        let levels = levels(&grid, lod_centre(view), frame.ring, frame.levels.max(1));
        let wanted: Option<std::collections::HashSet<(u8, ChunkKey)>> = list.map(|list| {
            let mut set: std::collections::HashSet<(u8, ChunkKey)> = list.iter().copied().collect();
            let mut by_ancestor = 0usize;
            for &(level, key) in list {
                let resident = if level == 0 {
                    frame.chunks.contains_key(&key)
                } else {
                    frame.bricks.contains_key(&(level, key))
                };
                if resident || frame.waiting.contains(&(level, key)) {
                    continue;
                }
                let found = (level + 1..cav_sim::world::clipmap::MAX_LEVELS)
                    .map(|to| (to, cav_sim::world::clipmap::ancestor(level, key, to)))
                    .find(|item| frame.bricks.contains_key(item));
                if let Some(item) = found {
                    set.insert(item);
                    by_ancestor += 1;
                }
            }
            if by_ancestor > 0 {
                self.notes.push(format!(
                    "LOD view: {by_ancestor} nodes not resident are drawn by a coarser brick that is"
                ));
            }
            set
        });
        let selected = |level: u8, key: ChunkKey| match &wanted {
            Some(set) => set.contains(&(level, key)),
            None => levels.get(usize::from(level)).is_some_and(|l| l.contains(key)),
        };
        let near: Vec<(ChunkKey, &Arc<ChunkCells>)> =
            frame.chunks.iter().filter(|(k, _)| selected(0, **k)).map(|(k, c)| (*k, c)).collect();
        // Nearest the camera first, so that a full brick store keeps the nearest.
        let eye_at = camera.origin + camera.eye;
        let mut far: Vec<(&(u8, ChunkKey), &Arc<Brick>)> =
            frame.bricks.iter().filter(|((level, key), _)| *level > 0 && selected(*level, *key)).collect();
        let distance = |k: &(u8, ChunkKey)| (grid.brick_box(k.0, k.1).centre() - eye_at).length();
        far.sort_by(|a, b| distance(a.0).total_cmp(&distance(b.0)).then(a.0.cmp(b.0)));
        // What the streaming no longer holds is kept, retired: drawn only in place of what is not
        // yet ready where it was ([`stand_ins`]), and let go when it has not been needed for
        // RETIRED_PREPARES prepares or its room is. What it has made again keeps its old copy,
        // drawn, until the new one is made (a mesh below; a brick is written over in its slot).
        for key in self.meshes.keys() {
            if frame.chunks.contains_key(key) {
                self.retired.remove(&(0, *key));
            } else {
                self.retired.entry((0, *key)).or_insert(0);
            }
        }
        self.empty.retain(|k, (c, _)| frame.chunks.get(k).is_some_and(|now| Arc::ptr_eq(now, c)));
        for key in self.bricks.keys() {
            if frame.bricks.contains_key(key) {
                self.retired.remove(key);
            } else {
                self.retired.entry(*key).or_insert(0);
            }
        }
        // Make the meshes wanted: the missing first, then those whose neighbours have come or
        // gone since they were made (kept drawn until their new mesh is made), nearest the camera
        // first (at most MAX_MESHES_PER_DRAW a frame unless complete), on as many threads as the
        // read uses (one in a page); the buffers are made here in the same order, so slots do not
        // depend on the thread count.
        let eye = camera.origin + camera.eye;
        let mut todo = self.to_mesh(&near, frame, eye);
        // At a cap with the same wanted as when it was reached: nothing is meshed (it would be
        // thrown away again), and the cap is said again.
        let fingerprint = self.fingerprint(&todo);
        match &self.full {
            Some((at, notes)) if *at == fingerprint => {
                // Every cap that bound, not only the last (S4's third review, minor 7).
                self.notes.extend(notes.iter().cloned());
                todo.clear();
            }
            _ => self.full = None,
        }
        let mut skipped = if complete { 0 } else { todo.len().saturating_sub(MAX_MESHES_PER_DRAW) };
        todo.truncate(todo.len() - skipped);
        let mesh_of = |key: ChunkKey, cells: &ChunkCells| {
            // Of each neighbour only the layer against this chunk (its first along the face's
            // axis on a + face, its last on a − face).
            let neighbour = |face: usize| {
                let d = AROUND[face];
                let layer = if face % 2 == 1 { 0 } else { CHUNK_SIDE - 1 };
                frame
                    .chunks
                    .get(&ChunkKey::new(key.x + d[0], key.y + d[1], key.z + d[2]))
                    .map(|c| c.materials_layer(face / 2, layer))
            };
            let cells_around: [Option<Vec<u8>>; 6] = std::array::from_fn(neighbour);
            greedy_mesh(&cells.materials(), cells_around.each_ref().map(|n| n.as_deref()), 0)
        };
        // In batches (eight a thread), until MESH_MILLISECONDS have gone (at least one batch; all
        // of them for a picture taken once): what keeps a page's frame, which has one thread,
        // from meshing for tens of milliseconds (TASK-S4-27).
        let meshing = web_time::Instant::now();
        let threads = cav_sim::read::threads().max(1);
        let mesh_batch = |part: &[(bool, f64, ChunkKey, &Arc<ChunkCells>)]| -> Vec<Mesh> {
            let threads = threads.min(part.len()).max(1);
            if threads == 1 {
                return part.iter().map(|(_, _, key, cells)| mesh_of(*key, cells)).collect();
            }
            let per = part.len().div_ceil(threads);
            std::thread::scope(|scope| {
                let handles: Vec<_> = part
                    .chunks(per)
                    .map(|share| {
                        let mesh_of = &mesh_of;
                        scope.spawn(move || {
                            share.iter().map(|(_, _, key, cells)| mesh_of(*key, cells)).collect::<Vec<_>>()
                        })
                    })
                    .collect();
                handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
            })
        };
        let mut meshes: Vec<Mesh> = Vec::with_capacity(todo.len());
        for part in todo.chunks(threads * 8) {
            if !complete && !meshes.is_empty() && meshing.elapsed().as_secs_f64() * 1e3 > MESH_MILLISECONDS {
                break;
            }
            meshes.extend(mesh_batch(part));
        }
        skipped += todo.len() - meshes.len();
        todo.truncate(meshes.len());
        self.meshed += meshes.len() as u64;
        for ((_, _, key, cells), mut mesh) in todo.iter().zip(meshes) {
            let now = around(frame, *key);
            // A mesh made again replaces the old one: its slot and vertices are given back first.
            let old = self.meshes.remove(key);
            if let Some(old) = &old {
                self.vertices -= old.vertex_count;
            }
            if mesh.indices.is_empty() {
                // Buried whole: nothing to draw, and nothing to make again while it stays.
                if let Some(old) = old {
                    self.free_meshes.push(old.slot);
                }
                self.empty.insert(*key, (Arc::clone(cells), now));
                continue;
            }
            self.empty.remove(key);
            // Room made from retired meshes first: they stand in only until what replaces them
            // is drawn, which this mesh is part of.
            while (self.vertices + mesh.vertices.len() > self.caps.vertices
                || (old.is_none() && self.free_meshes.is_empty()))
                && self.retire_one(0)
            {}
            let reached = if self.vertices + mesh.vertices.len() > self.caps.vertices {
                Some(format!(
                    "near field vertices: meshes past {} vertices are not drawn (the nearest are)",
                    self.caps.vertices
                ))
            } else if old.is_none() && self.free_meshes.is_empty() {
                Some(format!(
                    "near field chunks: chunks past {} are not drawn (the nearest are)",
                    self.caps.meshes
                ))
            } else {
                None
            };
            if let Some(note) = reached {
                // A chunk already drawn keeps its old mesh (a hole where a neighbour left is
                // better than the chunk gone); one not yet drawn waits. The rest are still tried:
                // a smaller mesh further down may fit (S4's second review, J3).
                if let Some(old) = old {
                    self.vertices += old.vertex_count;
                    self.meshes.insert(*key, old);
                }
                if !self.notes.contains(&note) {
                    self.notes.push(note.clone());
                }
                // Remembered against what is wanted after this frame's meshes are kept.
                match &mut self.full {
                    Some((_, notes)) if !notes.contains(&note) => notes.push(note),
                    Some(_) => {}
                    None => self.full = Some((0, vec![note])),
                }
                continue;
            }
            let slot = match old {
                Some(old) => old.slot,
                None => self.free_meshes.pop().expect("a free slot, checked above"),
            };
            for vertex in &mut mesh.vertices {
                vertex.cell[3] = slot as u16;
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
                    around: now,
                },
            );
        }
        self.mesh_ms = meshing.elapsed().as_secs_f64() * 1e3;
        // The fingerprint of what will be wanted next frame if nothing changes: what this frame
        // left (the same chunks as would be listed then), so that the next frame skips them.
        if self.full.is_some() {
            let left = self.to_mesh(&near, frame, eye);
            let at = self.fingerprint(&left);
            if let Some((then, _)) = &mut self.full {
                *then = at;
            }
        }
        self.incomplete = skipped > 0 && self.full.is_none();
        // Upload the bricks wanted, in the frustum and missing, nearest first. At the slot cap a
        // brick held but outside the frustum gives its slot up, so that "the nearest are" drawn
        // stays true after the camera turns (S4's second review, minor 15).
        let planes = frustum_planes(&camera.view_proj).map(|p| p.map(|v| v as f32));
        let seen = |key: &(u8, ChunkKey)| {
            let b = grid.brick_box(key.0, key.1);
            let lo = b.lo - camera.origin;
            visible(&planes, [lo.x as f32, lo.y as f32, lo.z as f32], grid.brick_side(key.0) as f32)
        };
        let mut unseen: Vec<(u8, ChunkKey)> = self.bricks.keys().filter(|k| !seen(k)).copied().collect();
        for (key, brick) in &far {
            if let Some((slot, held)) = self.bricks.get_mut(*key) {
                // Made again: written over in its slot, this frame (the old one drawn till now).
                if !Arc::ptr_eq(held, brick) {
                    *held = Arc::clone(brick);
                    gpu.queue.write_buffer(
                        store,
                        (*slot as usize * BRICK_WORDS * 4) as u64,
                        bytemuck::cast_slice(&brick_words(brick)),
                    );
                }
                continue;
            }
            if !seen(key) {
                continue;
            }
            if self.free_bricks.is_empty() {
                self.retire_one(1);
            }
            if self.free_bricks.is_empty()
                && let Some(out) = unseen.pop()
            {
                self.drop_brick(out);
            }
            let Some(slot) = self.free_bricks.pop() else {
                self.notes.push(format!(
                    "far field: bricks past {} are not drawn (the nearest are)",
                    self.caps.bricks
                ));
                break;
            };
            gpu.queue.write_buffer(
                store,
                (slot as usize * BRICK_WORDS * 4) as u64,
                bytemuck::cast_slice(&brick_words(brick)),
            );
            self.bricks.insert(**key, (slot, Arc::clone(brick)));
        }
        // The handover (TASK-S4-26): what this camera's clipmap draws but cannot yet (waiting in
        // the streaming's queue, resident but not yet meshed, or a brick in view not uploaded),
        // and what is held but not drawn by it (retired, or resident for another camera or a
        // tower). Of the held, those over something not ready are drawn in its place this frame.
        let ready = |key: &ChunkKey, cells: &Arc<ChunkCells>| {
            self.meshes.contains_key(key)
                || self.empty.get(key).is_some_and(|(made, _)| Arc::ptr_eq(made, cells))
        };
        let selected = |(level, key): (u8, ChunkKey)| selected(level, key);
        let unready: Vec<(u8, ChunkKey)> = frame
            .waiting
            .iter()
            .copied()
            .filter(|item| selected(*item))
            .chain(near.iter().filter(|(k, c)| !ready(k, c)).map(|(k, _)| (0, *k)))
            .chain(far.iter().filter(|(k, _)| !self.bricks.contains_key(*k) && seen(k)).map(|(k, _)| **k))
            .collect();
        let near_keys: std::collections::HashSet<ChunkKey> = near.iter().map(|(k, _)| *k).collect();
        let held = self
            .meshes
            .keys()
            .filter(|k| !near_keys.contains(k))
            .map(|k| (0, *k))
            // A retired brick the camera selects again stands in for itself while it is made again.
            .chain(self.bricks.keys().filter(|k| !selected(**k) || self.retired.contains_key(*k)).copied());
        let standing = if self.without_stand_ins { Vec::new() } else { stand_ins(unready, held) };
        for idle in self.retired.values_mut() {
            *idle = idle.saturating_add(1);
        }
        for item in &standing {
            if let Some(idle) = self.retired.get_mut(item) {
                *idle = 0;
            }
        }
        let stale: Vec<(u8, ChunkKey)> =
            self.retired.iter().filter(|(_, idle)| **idle > RETIRED_PREPARES).map(|(k, _)| *k).collect();
        for item in stale {
            if item.0 == 0 {
                self.drop_mesh(item.1);
            } else {
                self.drop_brick(item);
            }
        }
        // This frame's buffers: the chunks' origins by slot, the draws, the brick instances.
        let origin = camera.origin;
        let mut origins = vec![[0f32; 4]; MAX_CHUNK_SLOTS];
        let mut draws: Vec<[u32; 2]> = Vec::new();
        let mut chunks = Vec::new();
        self.drawn.clear();
        let standing_chunks = standing.iter().filter(|(level, _)| *level == 0).map(|(_, k)| *k);
        for key in near.iter().map(|(k, _)| *k).chain(standing_chunks) {
            if let Some(mesh) = self.meshes.get(&key) {
                let lo = grid.chunk_box(key).lo - origin;
                origins[mesh.slot as usize] = [lo.x as f32, lo.y as f32, lo.z as f32, 0.0];
                draws.push([mesh.slot, mesh.count]);
                chunks.push(key);
                self.drawn.push((0, key));
            } else if self.empty.contains_key(&key) && near_keys.contains(&key) {
                self.drawn.push((0, key));
            }
        }
        // The bricks to march: those in the camera's frustum, nearest first, as many as the
        // frame's budget of pixels allows (MAX_BRICK_OVERDRAW times the pane): what bounds one
        // submission's far-field work. Bricks behind the camera are never counted against it.
        let eye_world = camera.origin + camera.eye;
        let held: Vec<((u8, ChunkKey), u32)> = far
            .iter()
            .map(|(key, _)| **key)
            .chain(standing.iter().filter(|(level, _)| *level > 0).copied())
            .filter_map(|key| self.bricks.get(&key).map(|(slot, _)| (key, *slot)))
            .collect();
        let candidates: Vec<BrickCandidate> = held
            .iter()
            .map(|(key, _)| {
                let b = grid.brick_box(key.0, key.1);
                let side = grid.brick_side(key.0);
                let distance = (b.centre() - eye_world).length();
                let lo = b.lo - origin;
                BrickCandidate {
                    distance,
                    pixels: brick_pixels(side, distance, camera, view, pixels[1]),
                    visible: visible(&planes, [lo.x as f32, lo.y as f32, lo.z as f32], side as f32),
                }
            })
            .collect();
        let pane = (f64::from(pixels[0]) * f64::from(pixels[1])).max(1.0);
        let (chosen, left) = choose_bricks(&candidates, MAX_BRICK_OVERDRAW * pane);
        if left > 0 {
            self.notes.push(format!(
                "far field cut: its {left} farthest visible bricks are not drawn (a frame marches at most \
                 {MAX_BRICK_OVERDRAW} times the pane's pixels)"
            ));
        }
        self.marched = chosen.iter().map(|&i| candidates[i].pixels).sum::<f64>() / pane;
        let order: Vec<((u8, ChunkKey), u32)> = chosen.iter().map(|&i| held[i]).collect();
        self.drawn.extend(order.iter().map(|(key, _)| *key));
        let mut instances: Vec<BrickInstance> = Vec::new();
        for (key, slot) in &order {
            let b = grid.brick_box(key.0, key.1);
            let lo = b.lo - origin;
            let cell = b.size().x / f64::from(BRICK_SIDE);
            instances.push(BrickInstance {
                origin: [lo.x as f32, lo.y as f32, lo.z as f32, cell as f32],
                info: [*slot, u32::from(key.0), 0, 0],
            });
        }
        let eye = camera.eye.to_f32();
        let forward = camera.forward.to_f32();
        let ortho = f32::from(u8::from(view.projection == cav_sim::Projection::Orthographic));
        let lit = f32::from(u8::from(view.style == cav_sim::DrawStyle::Shaded));
        // The world tinted by level: `overlay bands`, or `overlay level` (S5).
        let by_level =
            view.overlay.bands || view.overlay.world == cav_sim::world::lod_view::WorldOverlay::Level;
        let uniform = WorldUniform {
            view_proj: camera.view_proj.to_f32(),
            eye: [eye[0], eye[1], eye[2], ortho],
            forward: [forward[0], forward[1], forward[2], 0.0],
            params: [grid.pitch as f32, f32::from(u8::from(by_level)), lit, 0.0],
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
    fn the_bit_mesher_is_the_cell_mesher() {
        // The same rectangles in the same order (TASK-S4-27): random chunks at several fills, with
        // and without neighbours; a ground (a half-space with steps); full and empty.
        // A chunk's cells and its six neighbours'.
        type Case = (Vec<u8>, [Option<Vec<u8>>; 6]);
        let mut cases: Vec<Case> = Vec::new();
        for (seed, fill) in [(1, 0.05), (2, 0.5), (3, 0.95)] {
            let around =
                std::array::from_fn(|f| (f % 3 != 0).then(|| random_cells(seed * 10 + f as u64, fill)));
            cases.push((random_cells(seed, fill), around));
            cases.push((random_cells(seed + 100, fill), std::array::from_fn(|_| None)));
        }
        let ground: Vec<u8> = (0..32 * 32 * 32usize)
            .map(|i| {
                let [x, y, z] = [i % 32, (i / 32) % 32, i / 1024];
                u8::from(z < 10 + (x / 5 + y / 7) % 4) * (1 + u8::from(x > 20))
            })
            .collect();
        cases.push((ground.clone(), std::array::from_fn(|f| (f != 5).then(|| ground.clone()))));
        cases.push((vec![1; 32 * 32 * 32], std::array::from_fn(|_| Some(vec![1; 32 * 32 * 32]))));
        cases.push((vec![2; 32 * 32 * 32], std::array::from_fn(|_| None)));
        cases.push((vec![0; 32 * 32 * 32], std::array::from_fn(|_| None)));
        let mesh = |cells: &[u8], around: &[Option<Vec<u8>>; 6], by_cells: bool| {
            let around = around.each_ref().map(|n| n.as_deref());
            if by_cells { greedy_mesh_by_cells(cells, around, 7) } else { greedy_mesh(cells, around, 7) }
        };
        let mut quads = 0;
        for (i, (cells, around)) in cases.iter().enumerate() {
            let want = mesh(cells, around, true);
            assert_eq!(mesh(cells, around, false), want, "case {i}");
            quads += want.vertices.len() / 4;
        }
        assert!(quads > 10_000, "the cases have faces to merge: {quads}");
        // Control: a chunk one cell different meshes differently, so equal meshes are not a given.
        let (cells, around) = &cases[1];
        let mut changed = cells.clone();
        changed[cell_index([16, 16, 16])] = if changed[cell_index([16, 16, 16])] == 0 { 1 } else { 0 };
        assert_ne!(mesh(&changed, around, false), mesh(cells, around, true));
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
    fn the_far_field_work_a_frame_is_capped() {
        let view = ViewSpec::framing(
            &cav_sim::SceneSpec::room(),
            cav_sim::NamedView::Iso,
            cav_sim::Projection::Perspective,
            cav_sim::DrawStyle::Shaded,
        );
        let camera = Camera::new(&view, 1.6);
        // A brick filling the view, and one under a pixel, by the estimate.
        let near = brick_pixels(10.0, 1.0, &camera, &view, 900);
        let tiny = brick_pixels(0.01, 1000.0, &camera, &view, 900);
        assert!(near > 900.0 * 900.0 && tiny < 1.0, "{near} {tiny}");
        // Many large bricks: only as many as the budget holds, nearest first; the sum within it.
        let budget = MAX_BRICK_OVERDRAW * 1440.0 * 900.0;
        let areas = vec![near; 1000];
        let keep = within_budget(&areas, budget);
        assert!(keep < 1000 && areas[..keep].iter().sum::<f64>() <= budget);
        assert!(areas[..=keep].iter().sum::<f64>() > budget, "one more would pass it");
        // Control: small bricks all fit.
        assert_eq!(within_budget(&vec![tiny; 1000], budget), 1000);
        assert_eq!(within_budget(&[budget * 2.0, 1.0], budget), 0);
    }

    #[test]
    fn bricks_behind_the_camera_never_take_the_budget() {
        // The review's I6: a row of bricks behind the eye (the nearest of all) and a row in
        // front; the budget holds the front row and no more.
        let mut view = ViewSpec::framing(
            &cav_sim::SceneSpec::room(),
            cav_sim::NamedView::Iso,
            cav_sim::Projection::Perspective,
            cav_sim::DrawStyle::Shaded,
        );
        view.distance = 30.0;
        let camera = Camera::new(&view, 1.6);
        let planes = frustum_planes(&camera.view_proj).map(|p| p.map(|v| v as f32));
        let eye = camera.eye;
        let toward = (-eye).normalised();
        let side = 4.0;
        let mut candidates = Vec::new();
        for k in 0..20 {
            // Behind: from 8 m to 27 m behind the eye; in front: from 10 m to 29 m ahead of it.
            for (ahead, from) in [(false, -8.0), (true, 10.0)] {
                let centre = eye + toward * (from + if ahead { 1.0 } else { -1.0 } * f64::from(k));
                let lo = centre - cav_core::Vec3::new(0.5 * side, 0.5 * side, 0.5 * side);
                let distance = (centre - eye).length();
                candidates.push(BrickCandidate {
                    distance,
                    pixels: brick_pixels(side, distance, &camera, &view, 900),
                    visible: visible(&planes, [lo.x as f32, lo.y as f32, lo.z as f32], side as f32),
                });
            }
        }
        let front: Vec<usize> = (0..candidates.len()).filter(|&i| i % 2 == 1).collect();
        assert!(front.iter().all(|&i| candidates[i].visible), "the front row is seen");
        assert!((0..candidates.len()).step_by(2).all(|i| !candidates[i].visible), "the back row is not");
        let budget: f64 = front.iter().map(|&i| candidates[i].pixels).sum::<f64>() * 1.0001;
        let (chosen, left) = choose_bricks(&candidates, budget);
        assert_eq!(left, 0);
        let mut sorted = chosen.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, front, "every brick in front is marched, and none behind");
        assert!(chosen.windows(2).all(|w| candidates[w[0]].distance <= candidates[w[1]].distance));
        // Control: counted without the frustum (as the first version did), the bricks behind
        // take the budget and push out some in front.
        let blind: Vec<BrickCandidate> =
            candidates.iter().map(|c| BrickCandidate { visible: true, ..*c }).collect();
        let (chosen, left) = choose_bricks(&blind, budget);
        assert!(left > 0 && front.iter().any(|i| !chosen.contains(i)));
    }

    #[test]
    fn a_ray_along_an_axis_marches() {
        // The review's I9: rays with components exactly zero (an orthographic named view), from a
        // start on a sub-block's plane, meet the first occupied cell of their row.
        let mut brick = Brick::empty(1, ChunkKey::new(0, 0, 0));
        brick.set([20, 8, 16]);
        brick.set([8, 24, 3]);
        let words = brick_words(&brick);
        // Along +x at y = 8, z = 16 exactly: both on sub-block planes.
        let hit = march(&words, [0.0, 8.0, 16.0], [1.0, 0.0, 0.0], 0);
        assert!(hit.hit && hit.cell == [20, 8, 16] && (hit.t - 20.0).abs() < 1e-3, "{hit:?}");
        // Down −z through (8.5, 24.5): starts on the top face.
        let down = march(&words, [8.5, 24.5, 32.0], [0.0, 0.0, -1.0], 5);
        assert!(down.hit && down.cell == [8, 24, 3] && (down.t - 28.0).abs() < 1e-3, "{down:?}");
        // Along −y with x and z zero-component, missing everything.
        let miss = march(&words, [8.0, 32.0, 8.0], [0.0, -1.0, 0.0], 3);
        assert!(!miss.hit && miss.steps <= MAX_MARCH_STEPS, "{miss:?}");
        assert!(nonzero([0.0, -0.0, 1.0]).iter().all(|c| c.abs() >= MIN_DIR && c.is_finite()));
        assert_eq!(nonzero([0.5, -0.25, -1e-9]), [0.5, -0.25, -MIN_DIR]);
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
