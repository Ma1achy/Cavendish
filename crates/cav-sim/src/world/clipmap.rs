//! The clipmap: which bricks of the world a camera draws, at which level of detail, and the
//! resident part of the world a renderer is handed ([`WorldFrame`]).
//!
//! **Levels.** Level `k` is made of bricks of 32 cells of `2^k · pitch` (level 0 is the world's
//! chunks). Around a camera, level `k` covers a cube of `2n` bricks a side, `Box_k`; the boxes nest
//! exactly, because each is centred on a snapped point: `c_0 = 2⌊cam/2⌋` (in level-0 bricks) and
//! `c_k = 2⌊c_{k−1}/4⌋`. Then `Box_{k−1}`'s footprint in level-`k` bricks,
//! `[c_{k−1}/2 − n/2, c_{k−1}/2 + n/2)`, is whole bricks and lies inside `Box_k`, and level `k`
//! draws `Box_k` without it ([`Level::contains`]). So every point within the coarsest box is drawn
//! by exactly one level (`clipmap_levels_nest_and_cover_once`). Level 0 is drawn as meshes, the
//! others ray-marched.
//!
//! **How many.** `n` comes from the screen-space error (`plan/REBUILD.md` §5): a cell at the inner
//! edge of its level, about `16 n` cells away, subtends `1/(16 n θ)` pixels for a pixel of `θ`
//! radians; so `n = ⌈1/(16 θ ε)⌉` for `ε` pixels, rounded up to even and held to `2..=MAX_RING`.
//! `θ` is the nominal pixel ([`NOMINAL_PIXEL`]: the field of view over 1080 rows), the same for the
//! engine that streams and the renderer that draws, so the two agree on what is wanted. Levels are
//! added until the coarsest box holds the world, at most [`MAX_LEVELS`]: S14 extends the same
//! rings outwards to the planet.

use std::collections::BTreeMap;
use std::sync::Arc;

use cav_core::Vec3;
use cav_core::voxel::Aabb;
use cav_core::voxel::world::{Brick, ChunkCells, ChunkKey, WorldGrid};

/// The most levels a clipmap has: level 11's bricks are 2048 chunks across.
pub const MAX_LEVELS: u8 = 12;
/// The widest a level may be, in bricks either side of its centre.
pub const MAX_RING: i64 = 16;
/// The nominal pixel, radians: the vertical field of view over 1080 rows.
pub const NOMINAL_PIXEL: f64 = crate::FOV_Y_DEGREES * std::f64::consts::PI / 180.0 / 1080.0;

/// The half-width `n` of every level, in bricks, for a screen-space error of `error_px` pixels.
#[must_use]
pub fn ring(error_px: f64) -> i64 {
    let n = (1.0 / (16.0 * NOMINAL_PIXEL * error_px.max(1e-3))).ceil();
    let n = n.clamp(2.0, MAX_RING as f64) as i64;
    n + n % 2
}

/// One level of a clipmap around a camera.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    /// Its level: bricks of `2^level` chunks.
    pub level: u8,
    /// Its box, in its own bricks: `lo` inclusive, `hi` exclusive.
    pub lo: [i64; 3],
    /// See `lo`.
    pub hi: [i64; 3],
    /// The next finer level's box in this level's bricks, which this level does not draw; none at
    /// level 0.
    pub hole: Option<([i64; 3], [i64; 3])>,
}

impl Level {
    /// Whether this level draws brick `key` (of its own level).
    #[must_use]
    pub fn contains(&self, key: ChunkKey) -> bool {
        let k = [i64::from(key.x), i64::from(key.y), i64::from(key.z)];
        let inside = |lo: [i64; 3], hi: [i64; 3]| (0..3).all(|a| k[a] >= lo[a] && k[a] < hi[a]);
        inside(self.lo, self.hi) && !self.hole.is_some_and(|(lo, hi)| inside(lo, hi))
    }

    /// Whether this level draws brick `key` (of its own level) for the seams' cross-fade
    /// ([`seam_band`]): a brick of its hole (the next finer level's box) in the hole's outer
    /// layers, where the band over which the two levels are dithered may lie, whatever the camera's
    /// place in its snapped brick. None of level 0 (it has no hole), none where the rings are too
    /// narrow for a band.
    #[must_use]
    pub fn blends(&self, key: ChunkKey, n: i64) -> bool {
        let (Some((lo, hi)), Some(band)) = (self.hole, seam_band(n)) else { return false };
        let k = [i64::from(key.x), i64::from(key.y), i64::from(key.z)];
        let inside = |lo: [i64; 3], hi: [i64; 3]| (0..3).all(|a| k[a] >= lo[a] && k[a] < hi[a]);
        if !inside(lo, hi) {
            return false;
        }
        // In this level's bricks from the hole's middle: the band lies at least `near` from it.
        let near = (0.5 * band.0 - 0.5 * SEAM_SNAP).floor() as i64;
        (0..3).any(|a| {
            let mid = (lo[a] + hi[a]) / 2;
            k[a] - mid >= near || k[a] + 1 - mid <= -near
        })
    }

    /// Whether brick `key` of this level, one it draws for the seams ([`Self::blends`]), reaches the
    /// band seen from `centre` (the camera's continuous place, metres, the world's frame) on `grid`:
    /// whether any of it lies at least the band's start from `centre` on some axis, in the finer
    /// level's bricks. A hole brick wholly nearer would be dithered away everywhere: a renderer
    /// need not march it (and must not spend its far field's budget on it).
    #[must_use]
    pub fn reaches_band(&self, grid: &WorldGrid, key: ChunkKey, centre: Vec3, n: i64) -> bool {
        let Some((d0, _)) = seam_band(n) else { return false };
        if self.level == 0 {
            return false;
        }
        let reach = d0 * grid.brick_side(self.level - 1);
        let b = grid.brick_box(self.level, key);
        let (lo, hi, c) = (b.lo.to_array(), b.hi.to_array(), centre.to_array());
        (0..3).any(|a| lo[a] < c[a] - reach || hi[a] > c[a] + reach)
    }

    /// The keys of `range` (inclusive, as [`WorldGrid::bricks_of`] gives) that this level draws,
    /// and with `blend` those it draws for the seams' cross-fade ([`Self::blends`]).
    pub fn keys_with(
        &self,
        range: (ChunkKey, ChunkKey),
        n: i64,
        blend: bool,
    ) -> impl Iterator<Item = ChunkKey> + '_ {
        let (a, b) = range;
        let (lo, hi) = (self.lo, self.hi);
        let lo = [lo[0].max(a.x.into()), lo[1].max(a.y.into()), lo[2].max(a.z.into())];
        let hi =
            [hi[0].min(i64::from(b.x) + 1), hi[1].min(i64::from(b.y) + 1), hi[2].min(i64::from(b.z) + 1)];
        (lo[2]..hi[2]).flat_map(move |z| {
            (lo[1]..hi[1]).flat_map(move |y| {
                (lo[0]..hi[0])
                    .map(move |x| ChunkKey::new(x as i32, y as i32, z as i32))
                    .filter(move |key| self.contains(*key) || (blend && self.blends(*key, n)))
            })
        })
    }

    /// The keys of `range` (inclusive, as [`WorldGrid::bricks_of`] gives) that this level draws.
    pub fn keys(&self, range: (ChunkKey, ChunkKey)) -> impl Iterator<Item = ChunkKey> + '_ {
        let (a, b) = range;
        let lo = [self.lo[0].max(a.x.into()), self.lo[1].max(a.y.into()), self.lo[2].max(a.z.into())];
        let hi = [
            self.hi[0].min(i64::from(b.x) + 1),
            self.hi[1].min(i64::from(b.y) + 1),
            self.hi[2].min(i64::from(b.z) + 1),
        ];
        (lo[2]..hi[2]).flat_map(move |z| {
            (lo[1]..hi[1]).flat_map(move |y| {
                (lo[0]..hi[0])
                    .map(move |x| ChunkKey::new(x as i32, y as i32, z as i32))
                    .filter(move |key| self.contains(*key))
            })
        })
    }
}

/// How the seams between a pane's rings of detail are drawn.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub enum Seams {
    /// Cross-faded by a 4×4 ordered dither over a band inside each finer ring's edge, weighted by
    /// the camera's continuous place ([`seam_band`]): the default.
    #[default]
    Dither,
    /// Each ring cut at its snapped edge (S4's picture): a step, and a pop when the ring snaps.
    Off,
}

impl Seams {
    /// Every one, in the order menus list them.
    pub const ALL: [Self; 2] = [Self::Dither, Self::Off];

    /// The word commands use.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dither => "dither",
            Self::Off => "off",
        }
    }

    /// The one a word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == word)
    }

    /// Whether it is the default.
    #[must_use]
    pub fn is_dither(&self) -> bool {
        *self == Self::Dither
    }
}

/// How far, in a level's own bricks, the camera may be from the middle of that level's snapped box:
/// each box is centred on a point snapped to two bricks of the level below and then two of its
/// own, so the camera is within three of its bricks of it, on one side ([`levels`]).
pub const SEAM_SNAP: f64 = 3.0;

/// The band over which a level and the next coarser one are cross-faded at the finer level's outer
/// edge (`docs/slices/S5.md` §2 "Seams", `docs/research/s5-world-octree.md` §7b): `(d₀, w)` in the
/// finer level's bricks, measured as the largest of the three axes' distances from the camera's
/// continuous place. The width is a tenth of the ring's half-width `n` and at least two coarse cells
/// (an eighth of a finer brick); the band ends [`SEAM_SNAP`] bricks inside the snapped edge, so that a
/// snap, which moves the edge, happens where the coarser level is already all that is drawn. `None`
/// when the ring is too narrow for a band clear of the next finer level's own (`n` below about 12).
#[must_use]
pub fn seam_band(n: i64) -> Option<(f64, f64)> {
    let n = n as f64;
    let w = (0.1 * n).max(0.125);
    let d0 = n - SEAM_SNAP - w;
    // The next finer level's box reaches at most n/2 + SEAM_SNAP/2 of these bricks from the camera.
    (d0 > 0.5 * n + 0.5 * SEAM_SNAP).then_some((d0, w))
}

/// Where a view's camera is, for its clipmap: the eye in perspective, the target in an
/// orthographic view (whose scale does not change with depth). Metres, the world's frame. The
/// engine streams around it and the renderer draws around it: the same point for both.
#[must_use]
pub fn lod_centre(view: &crate::ViewSpec) -> Vec3 {
    let target = Vec3::from_array(view.target);
    match view.projection {
        crate::Projection::Perspective => target + Vec3::from_array(view.basis().0) * view.distance,
        crate::Projection::Orthographic => target,
    }
}

/// The levels around `centre` (metres, the world's frame), `count` of them, each `n` bricks either
/// side (`n` even, from [`ring`]).
#[must_use]
pub fn levels(grid: &WorldGrid, centre: Vec3, n: i64, count: u8) -> Vec<Level> {
    let side = grid.chunk_side();
    let at = |v: f64, o: f64| ((v - o) / side).floor().clamp(-1e15, 1e15) as i64;
    let cam = [at(centre.x, grid.origin.x), at(centre.y, grid.origin.y), at(centre.z, grid.origin.z)];
    let mut c = cam.map(|v| 2 * v.div_euclid(2));
    let mut out = Vec::with_capacity(usize::from(count));
    for level in 0..count.min(MAX_LEVELS) {
        let hole = (level > 0).then(|| {
            let previous = out.last().map_or(c, |l: &Level| l.lo.map(|v| v + n)).map(|v| v / 2);
            (previous.map(|v| v - n / 2), previous.map(|v| v + n / 2))
        });
        if level > 0 {
            c = c.map(|v| 2 * v.div_euclid(4));
        }
        out.push(Level { level, lo: c.map(|v| v - n), hi: c.map(|v| v + n), hole });
    }
    out
}

/// How many levels reach from `centre` over the whole of `extent`, at least 1, at most
/// [`MAX_LEVELS`].
#[must_use]
pub fn level_count(grid: &WorldGrid, extent: &Aabb, centre: Vec3, n: i64) -> u8 {
    for count in 1..=MAX_LEVELS {
        let last = levels(grid, centre, n, count)[usize::from(count - 1)];
        let (lo, hi) = grid.bricks_of(last.level, extent);
        let covers = [lo.x, lo.y, lo.z].iter().zip(last.lo).all(|(a, b)| i64::from(*a) >= b)
            && [hi.x, hi.y, hi.z].iter().zip(last.hi).all(|(a, b)| i64::from(*a) < b);
        if covers {
            return count;
        }
    }
    MAX_LEVELS
}

/// The brick of level `to` (at least `level`) that holds brick `key` of level `level` (a chunk at
/// level 0): each level's bricks are twice the side of the one below, on the same origin.
#[must_use]
pub fn ancestor(level: u8, key: ChunkKey, to: u8) -> ChunkKey {
    let s = 1i32 << (to.saturating_sub(level)).min(30);
    ChunkKey::new(key.x.div_euclid(s), key.y.div_euclid(s), key.z.div_euclid(s))
}

/// Which of `held` to draw in place of `unready`: the handover between levels (the bug of chunks
/// vanishing while the camera moves, TASK-S4-26).
///
/// When the camera moves, a piece of the world changes level: a chunk leaves level 0 and its
/// region is drawn next by a brick of level 1, or the other way round. Until what is wanted now
/// is ready to draw (made by the streaming, meshed or uploaded by the renderer), the region would
/// be a hole. So what was drawn there before stands in: an item of `held` (by `(level, key)`,
/// what a renderer still has a copy of but does not draw this frame) is returned when its box
/// overlaps the box of an item of `unready` (what the camera's clipmap draws there this frame but
/// cannot yet): a coarser one holding it, or finer ones inside it. Nothing else of `held` is
/// returned, so a stand-in goes the frame its region is ready. Order: `held`'s.
#[must_use]
pub fn stand_ins(
    unready: impl IntoIterator<Item = (u8, ChunkKey)>,
    held: impl IntoIterator<Item = (u8, ChunkKey)>,
) -> Vec<(u8, ChunkKey)> {
    use std::collections::HashSet;
    // Every unready item and every brick that holds one: a held item in this set has something
    // unready inside it (or is itself unready).
    let mut under: HashSet<(u8, ChunkKey)> = HashSet::new();
    let mut unready_set: HashSet<(u8, ChunkKey)> = HashSet::new();
    for (level, key) in unready {
        unready_set.insert((level, key));
        for to in level..MAX_LEVELS {
            if !under.insert((to, ancestor(level, key, to))) {
                break; // in already, and so are the bricks that hold it
            }
        }
    }
    if unready_set.is_empty() {
        return Vec::new();
    }
    held.into_iter()
        .filter(|&(level, key)| {
            under.contains(&(level, key))
                || (level + 1..MAX_LEVELS).any(|to| unready_set.contains(&(to, ancestor(level, key, to))))
        })
        .collect()
}

/// What a renderer is handed of the world: the resident chunks and bricks with matter, and what
/// it needs to draw them. Made and kept by the engine's streaming; the renderer draws what of it
/// its camera's clipmap wants and holds its own GPU copies keyed by `(level, key)`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorldFrame {
    /// The world's grid.
    pub grid: Option<WorldGrid>,
    /// Each material's colour, sRGB, by world index.
    pub colours: Vec<[u8; 3]>,
    /// The half-width of every level, in bricks ([`ring`]).
    pub ring: i64,
    /// The levels a camera uses: enough to reach over the world from anywhere in it.
    pub levels: u8,
    /// The world's box, for drawing it and for streaming.
    pub extent: Option<Aabb>,
    /// Resident level-0 chunks with matter.
    pub chunks: BTreeMap<ChunkKey, Arc<ChunkCells>>,
    /// Resident bricks of levels 1 and up with matter, by `(level, key)`.
    pub bricks: BTreeMap<(u8, ChunkKey), Arc<Brick>>,
    /// The world's hash ([`super::WorldSpec::hash`]): a renderer's copies of another world are stale.
    pub world: u64,
    /// How many chunks and bricks are still to be made for what the cameras want.
    pub pending: usize,
    /// Which, by `(level, key)` (level 0 for a chunk): the streaming's queue. A renderer draws
    /// what it drew before in their place until they are made ([`stand_ins`]); a wanted item
    /// neither resident nor here has no matter (or was cut by a cap) and needs no stand-in.
    pub waiting: std::collections::HashSet<(u8, ChunkKey)>,
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

    /// How many of `levels` draw the level-0 brick `cell` (each level asked about the brick of its
    /// own level that holds it).
    fn drawn_by(levels: &[Level], cell: [i64; 3]) -> usize {
        levels
            .iter()
            .filter(|l| {
                let k = cell.map(|v| v.div_euclid(1 << l.level));
                l.contains(ChunkKey::new(k[0] as i32, k[1] as i32, k[2] as i32))
            })
            .count()
    }

    #[test]
    fn clipmap_levels_nest_and_cover_once() {
        let grid = WorldGrid { origin: Vec3::new(3.0, -7.0, 0.5), pitch: 0.05, supersample: 2 };
        let side = grid.chunk_side();
        for n in [2, 4, 8, 16] {
            for i in 0..250u64 {
                let centre = Vec3::new(
                    2000.0 * (unit(i, 0) - 0.5),
                    2000.0 * (unit(i, 1) - 0.5),
                    100.0 * (unit(i, 2) - 0.5),
                );
                let levels = levels(&grid, centre, n, 6);
                let last = levels[5];
                // Every level-0 brick in the coarsest box, sampled, is drawn by exactly one level.
                for s in 0..200u64 {
                    let span = |a: usize| (last.hi[a] - last.lo[a]) << 5;
                    let cell = [0, 1, 2].map(|a| {
                        (last.lo[a] << 5) + (unit(i * 1000 + s, 3 + a as u64) * span(a) as f64) as i64
                    });
                    assert_eq!(drawn_by(&levels, cell), 1, "n {n}, camera {i}, cell {cell:?}");
                }
                // The camera's own brick is level 0's.
                let own = [centre.x - grid.origin.x, centre.y - grid.origin.y, centre.z - grid.origin.z]
                    .map(|v| (v / side).floor() as i64);
                assert!(levels[0].contains(ChunkKey::new(own[0] as i32, own[1] as i32, own[2] as i32)));
            }
        }
        // Control: levels centred each on its own camera brick, not snapped, leave cells drawn by
        // none or by two.
        let centre = Vec3::new(37.3 * side, -11.6 * side, 2.2 * side);
        let n = 4;
        let unsnapped: Vec<Level> = (0..4u8)
            .map(|level| {
                let c = [centre.x - grid.origin.x, centre.y - grid.origin.y, centre.z - grid.origin.z]
                    .map(|v| (v / (side * f64::from(1u32 << level))).floor() as i64);
                let hole = (level > 0).then(|| {
                    let f = [centre.x - grid.origin.x, centre.y - grid.origin.y, centre.z - grid.origin.z]
                        .map(|v| (v / (side * f64::from(1u32 << (level - 1)))).floor() as i64);
                    (f.map(|v| (v - n).div_euclid(2)), f.map(|v| (v + n).div_euclid(2)))
                });
                Level { level, lo: c.map(|v| v - n), hi: c.map(|v| v + n), hole }
            })
            .collect();
        let last = unsnapped[3];
        let mut wrong = 0;
        for x in (last.lo[0] << 3)..(last.hi[0] << 3) {
            for y in (last.lo[1] << 3)..(last.hi[1] << 3) {
                if drawn_by(&unsnapped, [x, y, last.lo[2] * 8 + 30]) != 1 {
                    wrong += 1;
                }
            }
        }
        assert!(wrong > 0, "unsnapped levels happened to nest");
    }

    #[test]
    fn what_was_drawn_stands_in_until_what_replaces_it_is_ready() {
        let k = |x, y, z| ChunkKey::new(x, y, z);
        // A chunk not yet ready: the level-1 brick holding it stands in, a neighbouring one not.
        let held = [(1, k(1, 1, 0)), (1, k(2, 2, 0)), (2, k(0, 0, 0)), (0, k(2, 3, 0))];
        assert_eq!(stand_ins([(0, k(2, 3, 1))], held), vec![(1, k(1, 1, 0)), (2, k(0, 0, 0))]);
        // A brick not yet ready: the chunks inside it stand in, those beside it not.
        let chunks = [(0, k(3, 3, 3)), (0, k(4, 0, 0)), (0, k(0, 0, 0)), (1, k(1, 1, 1)), (3, k(0, 0, 0))];
        assert_eq!(
            stand_ins([(2, k(0, 0, 0))], chunks),
            vec![(0, k(3, 3, 3)), (0, k(0, 0, 0)), (1, k(1, 1, 1)), (3, k(0, 0, 0))]
        );
        // Below zero the bricks hold the chunks below them (floor, not truncation): chunk −1 is in
        // brick −1, not 0.
        assert_eq!(ancestor(0, k(-1, -1, -3), 1), k(-1, -1, -2));
        assert_eq!(
            stand_ins([(0, k(-1, -1, 0))], [(1, k(-1, -1, 0)), (1, k(0, 0, 0))]),
            vec![(1, k(-1, -1, 0))]
        );
        // Control: with everything ready, nothing stands in.
        assert!(stand_ins([], held).is_empty());
        // A held item that is itself unready (made again) is its own stand-in.
        assert_eq!(stand_ins([(1, k(2, 2, 0))], [(1, k(2, 2, 0))]), vec![(1, k(2, 2, 0))]);
    }

    #[test]
    fn the_seams_band_is_drawn_by_both_levels_wherever_the_camera_is() {
        // SE1's geometry (`docs/slices/S5.md` §7.16): wherever the camera is in its snapped brick,
        // every point of the band over which level k fades into level k − 1 (measured from the
        // camera's continuous place) is drawn by level k − 1 and by a brick level k draws for the
        // seams, and the band ends inside level k − 1's box (so a snap moves nothing drawn there).
        // Control: level k's own bricks (without the seams' layers) leave part of the band bare.
        assert_eq!(seam_band(16), Some((11.4, 1.6)));
        assert_eq!(seam_band(8), None, "too narrow a ring has no band");
        let grid = WorldGrid { origin: Vec3::new(1.0, -2.0, 0.0), pitch: 0.1, supersample: 2 };
        let n = 16;
        let (d0, w) = seam_band(n).unwrap();
        let (mut bare, mut checked) = (0, 0);
        for i in 0..60u64 {
            let centre = Vec3::new(500.0 * (unit(i, 0) - 0.5), 500.0 * (unit(i, 1) - 0.5), 20.0 * unit(i, 2));
            let ls = levels(&grid, centre, n, 4);
            for k in 1..4usize {
                let (finer, coarse) = (ls[k - 1], ls[k]);
                let fside = grid.brick_side(finer.level);
                for s in 0..80u64 {
                    // A point of the band: on a random face of the cube of half-side d (in finer
                    // bricks) about the camera, d within the band.
                    let d = (d0 + w * unit(i * 1000 + s, 9)) * fside;
                    let mut p = [0, 1, 2].map(|a| (2.0 * unit(i * 1000 + s, 10 + a as u64) - 1.0) * d);
                    let axis = (unit(i * 1000 + s, 13) * 3.0) as usize % 3;
                    p[axis] = if unit(i * 1000 + s, 14) < 0.5 { -d } else { d };
                    let at = centre + Vec3::from_array(p);
                    let key_of = |level: u8| {
                        let side = grid.brick_side(level);
                        let r = at - grid.origin;
                        ChunkKey::new(
                            (r.x / side).floor() as i32,
                            (r.y / side).floor() as i32,
                            (r.z / side).floor() as i32,
                        )
                    };
                    assert!(finer.contains(key_of(finer.level)), "the band is inside the finer ring");
                    let ck = key_of(coarse.level);
                    assert!(coarse.contains(ck) || coarse.blends(ck, n), "the coarser level draws the band");
                    if !coarse.contains(ck) {
                        assert!(coarse.reaches_band(&grid, ck, centre, n));
                    }
                    bare += usize::from(!coarse.contains(ck));
                    checked += 1;
                }
            }
        }
        assert!(bare > checked / 10, "control: the coarser level's own bricks miss {bare} of {checked}");
    }

    #[test]
    fn the_ring_follows_the_screen_space_error() {
        // n = ⌈1/(16 θ ε)⌉, even, from 2 to 16.
        assert_eq!(ring(64.0), 2);
        assert_eq!(ring(0.5), MAX_RING);
        let n = ring(8.0);
        let expected = (1.0 / (16.0 * NOMINAL_PIXEL * 8.0)).ceil() as i64;
        assert_eq!(n, expected + expected % 2);
        // A finer error needs at least as wide a ring.
        assert!(ring(4.0) >= ring(8.0) && ring(8.0) >= ring(16.0));
        // The levels reach over a kilometre from its middle, and not with fewer.
        let grid = WorldGrid { origin: Vec3::ZERO, pitch: 0.05, supersample: 2 };
        let extent = Aabb::new(Vec3::new(-500.0, -500.0, -2.0), Vec3::new(500.0, 500.0, 25.0));
        let count = level_count(&grid, &extent, Vec3::new(0.0, 0.0, 5.0), 16);
        let last = levels(&grid, Vec3::new(0.0, 0.0, 5.0), 16, count)[usize::from(count) - 1];
        let reach = grid.brick_side(last.level) * 16.0;
        assert!(reach >= 500.0, "{count} levels reach {reach}");
        // Control: one level fewer does not reach.
        assert!(grid.brick_side(last.level - 1) * 16.0 < 500.0, "{count} levels: one fewer would do");
    }
}
