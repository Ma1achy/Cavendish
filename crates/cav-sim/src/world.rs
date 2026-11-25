//! The world volume of a scene: static matter (terrain, buildings, slabs) as pieces of one world,
//! voxelised into chunks ([`cav_core::voxel::world`]) and read by the instrument like a body.
//!
//! **What a world is** ([`WorldSpec`]): a pitch (the finest cell), its materials (density and
//! colour), and its pieces, each a solid of one material, in order of precedence where they
//! overlap. Chunks of 32³ cells are made from the pieces when they are wanted (by the read here,
//! by the engine's streaming for the picture) and are never part of the spec.
//!
//! **How it is read** ([`contribute`], source `World`). The world is static, so each tower's
//! reading is made once and holds for every shot. For each tower: the chunks within
//! `read.radius` of its atoms' path, nearest first, at most [`MAX_READ_CHUNKS`] in all. A chunk is
//! read through its mass moments (the octupole expansion about its centre of mass) when the bound
//! on what that leaves out is within its share of `read.tolerance` radians; otherwise leaf by
//! leaf, each leaf a uniform cube with the exact kernel. The bound (`docs/slices/S4.md` §2): the
//! remainder after the octupole has a gradient at most `√2 M q⁴(5 − 4q)/((1 − q)² R²)` with
//! `q = a/R`, `a` the chunk's radius about its centre of mass and `R` the distance to the
//! interferometer's arms; times the arms' separation it bounds the remainder's share of
//! `V(upper) − V(lower)`. That bound is itself read through the flight ([`BoundSource`]) and added
//! to the read's error estimate, so the error the read reports covers the approximation. Matter
//! beyond the radius or the cap is not read, and the read says so: reading a whole 10¹⁰-cell
//! world is S5's bounded walk.
//!
//! Frames and units: the world's frame (x east, y north, z up), metres, kilograms, kg/m³, radians.

use cav_core::Vec3;
use cav_core::forward::{Audited, PHASE_TOLERANCE, Reader, Source};
use cav_core::voxel::kernel::{MAX_KERNEL_WORK, cube_difference};
use cav_core::voxel::shape::Cuboid;
use cav_core::voxel::tree::Moments;
use cav_core::voxel::world::{
    Brick, Chunk, ChunkKey, MAX_MATERIALS, Piece, Solid, Terrain, WorldGrid, brick_of_pieces, voxelise_chunk,
};
use cav_core::voxel::{Aabb, MAX_SUPERSAMPLE};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::spec::{SpecError, limits};

/// The most pieces a world may have.
pub const MAX_PIECES: usize = 256;
/// The most chunks one read of a world makes and reads, over all its towers, nearest first.
pub const MAX_READ_CHUNKS: usize = 4096;
/// The most chunk keys a read looks at to find the ones within its radius (the boxes around the
/// towers, clipped to the world): what bounds the search itself.
pub const MAX_READ_SCAN: u64 = 4_000_000;
/// The widest read radius, metres.
pub const MAX_READ_RADIUS: f64 = 1.0e4;
/// The finest pitch a world may have, metres: a chunk is then 3.2 cm, and a world a kilometre
/// across is 3×10⁷ chunks along a side, inside the keys' range.
pub const MIN_PITCH: f64 = 1.0e-3;

/// A scene's world: static matter in one volume.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorldSpec {
    /// The side of a finest cell, metres; a chunk is 32 of them.
    #[schemars(range(min = 1.0e-3, max = 1.0e2))]
    #[schemars(extend("x-unit" = "m"))]
    pub pitch: f64,
    /// Sample points along each side of a finest cell, 1 to 6: how finely a cell a surface cuts
    /// is weighed. The default is 2.
    #[serde(default = "default_supersample")]
    #[schemars(range(min = 1, max = 6))]
    #[schemars(extend("x-unit" = "1"))]
    pub supersample: u8,
    /// The corner of chunk `[0, 0, 0]`, `[x, y, z]` metres: where the chunk lattice starts.
    #[serde(default)]
    #[schemars(extend("x-unit" = "m"))]
    pub origin: [f64; 3],
    /// The materials, 1 to 255; a piece names one by its index.
    pub materials: Vec<MaterialSpec>,
    /// The pieces, 0 to 256, in order of precedence: where two overlap, the earlier holds the
    /// matter. A finest cell two materials share takes the one with more of its sample points
    /// (an error of at most one cell layer at an interface).
    #[serde(default)]
    pub pieces: Vec<PieceSpec>,
    /// How much of the world is kept resident for the picture.
    #[serde(default)]
    pub stream: StreamSpec,
    /// How the instrument reads the world.
    #[serde(default)]
    pub read: WorldReadSpec,
    /// How the picture coarsens with distance.
    #[serde(default)]
    pub lod: LodSpec,
}

const fn default_supersample() -> u8 {
    2
}

/// One material of a world.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialSpec {
    /// Its name, as legends and the paper drawing show it. At most 200 characters.
    pub label: String,
    /// Kilograms per cubic metre, 0 to 1e5 (0 for matter that is drawn but weighs nothing).
    #[schemars(range(min = 0.0, max = 1.0e5))]
    #[schemars(extend("x-unit" = "kg/m^3"))]
    pub density: f64,
    /// Its colour in the picture, sRGB, as `#rrggbb`.
    pub colour: String,
}

/// One piece of a world.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PieceSpec {
    /// Its name. At most 200 characters.
    pub label: String,
    /// Its material: an index into `world.materials`.
    pub material: usize,
    /// What solid it is.
    pub kind: PieceKind,
}

/// The solids a piece can be.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum PieceKind {
    /// A box with its faces along the axes: a slab, a building, a road.
    Box {
        /// Its centre, `[x, y, z]` metres.
        #[schemars(extend("x-unit" = "m"))]
        centre: [f64; 3],
        /// Its size along x, y and z, metres; each positive.
        #[schemars(extend("x-unit" = "m"))]
        size: [f64; 3],
    },
    /// Rolling terrain over a rectangle: from `bottom` up to a surface `level` ± `relief`, hills
    /// of three octaves from `wavelength` down, their phases from `seed`.
    Terrain {
        /// The south-west corner, `[x, y]` metres.
        #[schemars(extend("x-unit" = "m"))]
        lo: [f64; 2],
        /// The north-east corner, `[x, y]` metres.
        #[schemars(extend("x-unit" = "m"))]
        hi: [f64; 2],
        /// The underside, metres, up.
        #[schemars(extend("x-unit" = "m"))]
        bottom: f64,
        /// The mean surface, metres, up.
        #[schemars(extend("x-unit" = "m"))]
        level: f64,
        /// How far the surface rises and falls about `level`, metres (0 for flat).
        #[schemars(extend("x-unit" = "m"))]
        relief: f64,
        /// The longest wavelength of the hills, metres.
        #[schemars(extend("x-unit" = "m"))]
        wavelength: f64,
        /// The hills' seed.
        seed: u32,
    },
}

/// What the engine keeps of the world for the picture.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StreamSpec {
    /// Every chunk within this distance of a tower's atoms' path stays resident whatever the
    /// camera does: the instrument always has its neighbourhood. Metres, 0 to 1e3; default 8.
    #[schemars(range(min = 0.0, max = 1.0e3))]
    #[schemars(extend("x-unit" = "m"))]
    pub tower_radius: f64,
}

impl Default for StreamSpec {
    fn default() -> Self {
        Self { tower_radius: 8.0 }
    }
}

/// How the instrument reads the world.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorldReadSpec {
    /// Chunks within this distance of a tower's atoms' path are read; farther matter is not (the
    /// read says so). Metres, 0 to 1e4; default 100.
    #[schemars(range(min = 0.0, max = 1.0e4))]
    #[schemars(extend("x-unit" = "m"))]
    pub radius: f64,
    /// The most the reading through chunks' moments may be off, in all, radians: a chunk is
    /// read through its moments only when its bound fits its share. Default 1e-10 (the phase
    /// tolerance); larger lets more chunks go by their moments, for a read that is cheaper and
    /// as far off as said.
    #[schemars(range(min = 0.0, max = 1.0))]
    #[schemars(extend("x-unit" = "rad"))]
    pub tolerance: f64,
}

impl Default for WorldReadSpec {
    fn default() -> Self {
        Self { radius: 100.0, tolerance: PHASE_TOLERANCE }
    }
}

/// How the picture coarsens with distance (the clipmap's rings).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LodSpec {
    /// The largest a cell may look at the inner edge of its ring, pixels, 0.5 to 64; default 4.
    /// Smaller is finer and costs more bricks.
    #[schemars(range(min = 0.5, max = 64.0))]
    #[schemars(extend("x-unit" = "px"))]
    pub error_px: f64,
}

impl Default for LodSpec {
    fn default() -> Self {
        Self { error_px: 4.0 }
    }
}

impl WorldSpec {
    /// Checks the world's numbers and counts, naming the first that is wrong by its path.
    ///
    /// # Errors
    /// When a number is out of its range or a count past its cap (`docs/slices/S4.md` §3.1, W1).
    pub fn validate(&self) -> Result<(), SpecError> {
        let fail = |why: String| Err(SpecError(why));
        if !(self.pitch.is_finite() && (MIN_PITCH..=100.0).contains(&self.pitch)) {
            return fail(format!("world.pitch: {} m must be from {MIN_PITCH} to 100", self.pitch));
        }
        if !(1..=MAX_SUPERSAMPLE).contains(&self.supersample) {
            return fail(format!(
                "world.supersample: {} must be from 1 to {MAX_SUPERSAMPLE}",
                self.supersample
            ));
        }
        crate::spec::finite("world.origin", &self.origin)?;
        if self.materials.is_empty() || self.materials.len() > MAX_MATERIALS {
            return fail(format!(
                "world.materials: {} materials; a world has 1 to {MAX_MATERIALS}",
                self.materials.len()
            ));
        }
        for (n, m) in self.materials.iter().enumerate() {
            crate::spec::short(&format!("world.materials[{n}].label"), &m.label, limits::LABEL)?;
            crate::spec::within(&format!("world.materials[{n}].density"), m.density, (0.0, limits::DENSITY))?;
            if parse_colour(&m.colour).is_none() {
                return fail(format!(
                    "world.materials[{n}].colour: `{}` is not a colour `#rrggbb`",
                    m.colour
                ));
            }
        }
        if self.pieces.len() > MAX_PIECES {
            return fail(format!("world.pieces: {} pieces is more than {MAX_PIECES}", self.pieces.len()));
        }
        for (n, p) in self.pieces.iter().enumerate() {
            let at = format!("world.pieces[{n}]");
            crate::spec::short(&format!("{at}.label"), &p.label, limits::LABEL)?;
            if p.material >= self.materials.len() {
                return fail(format!(
                    "{at}.material: {} is not one of the world's {} materials",
                    p.material,
                    self.materials.len()
                ));
            }
            match &p.kind {
                PieceKind::Box { centre, size } => {
                    crate::spec::finite(&format!("{at}.kind.Box.centre"), centre)?;
                    for (axis, s) in size.iter().enumerate() {
                        crate::spec::positive(
                            &format!("{at}.kind.Box.size[{axis}]"),
                            *s,
                            2.0 * limits::POSITION,
                        )?;
                    }
                }
                PieceKind::Terrain { lo, hi, bottom, level, relief, wavelength, .. } => {
                    let k = format!("{at}.kind.Terrain");
                    crate::spec::finite(&format!("{k}.lo"), lo)?;
                    crate::spec::finite(&format!("{k}.hi"), hi)?;
                    crate::spec::finite(&format!("{k}.bottom"), &[*bottom, *level])?;
                    if !(lo[0] < hi[0] && lo[1] < hi[1]) {
                        return fail(format!("{k}: lo {lo:?} must be south-west of hi {hi:?}"));
                    }
                    crate::spec::within(&format!("{k}.relief"), *relief, (0.0, 1.0e4))?;
                    crate::spec::positive(&format!("{k}.wavelength"), *wavelength, 2.0 * limits::POSITION)?;
                    if *bottom >= *level - *relief {
                        return fail(format!(
                            "{k}.bottom: {bottom} m must be below the lowest surface, {} m",
                            level - relief
                        ));
                    }
                }
            }
        }
        crate::spec::within("world.stream.tower_radius", self.stream.tower_radius, (0.0, 1.0e3))?;
        crate::spec::within("world.read.radius", self.read.radius, (0.0, MAX_READ_RADIUS))?;
        crate::spec::within("world.read.tolerance", self.read.tolerance, (0.0, 1.0))?;
        crate::spec::within("world.lod.error_px", self.lod.error_px, (0.5, 64.0))?;
        Ok(())
    }

    /// The world's grid.
    #[must_use]
    pub fn grid(&self) -> WorldGrid {
        WorldGrid { origin: Vec3::from_array(self.origin), pitch: self.pitch, supersample: self.supersample }
    }

    /// A hash of everything that decides the world's chunks (pitch, sampling, origin, materials'
    /// order and the pieces), FNV-1a over their JSON: what a region file is keyed by. Not of the
    /// colours, the streaming, the read or the level of detail, which change no chunk.
    #[must_use]
    pub fn hash(&self) -> u64 {
        let densities: Vec<u64> = self.materials.iter().map(|m| m.density.to_bits()).collect();
        let text =
            serde_json::to_string(&(self.pitch, self.supersample, self.origin, densities, &self.pieces))
                .unwrap_or_default();
        text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
    }
}

/// `#rrggbb` as `[r, g, b]`, or `None` when it is not one.
#[must_use]
pub fn parse_colour(text: &str) -> Option<[u8; 3]> {
    let hex = text.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let at = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([at(0)?, at(2)?, at(4)?])
}

/// A world made ready to voxelise: its solids, densities and grid.
pub struct World {
    /// The world's grid.
    pub grid: WorldGrid,
    /// Each material's density, kg/m³, by index.
    pub densities: Vec<f64>,
    /// Each material's colour, sRGB.
    pub colours: Vec<[u8; 3]>,
    solids: Vec<(Box<dyn Solid>, u8)>,
}

impl World {
    /// The world of `spec` (which [`WorldSpec::validate`] has passed).
    #[must_use]
    pub fn new(spec: &WorldSpec) -> Self {
        let solids = spec
            .pieces
            .iter()
            .map(|p| {
                let solid: Box<dyn Solid> = match &p.kind {
                    PieceKind::Box { centre, size } => Box::new(Cuboid {
                        centre: Vec3::from_array(*centre),
                        half: Vec3::from_array(*size) * 0.5,
                    }),
                    PieceKind::Terrain { lo, hi, bottom, level, relief, wavelength, seed } => {
                        Box::new(Terrain {
                            lo: *lo,
                            hi: *hi,
                            bottom: *bottom,
                            level: *level,
                            relief: *relief,
                            wavelength: *wavelength,
                            seed: u64::from(*seed),
                        })
                    }
                };
                (solid, p.material.min(MAX_MATERIALS - 1) as u8)
            })
            .collect();
        Self {
            grid: spec.grid(),
            densities: spec.materials.iter().map(|m| m.density).collect(),
            colours: spec.materials.iter().map(|m| parse_colour(&m.colour).unwrap_or([128; 3])).collect(),
            solids,
        }
    }

    /// The pieces, in order, for the voxeliser.
    #[must_use]
    pub fn pieces(&self) -> Vec<Piece<'_>> {
        self.solids
            .iter()
            .map(|(solid, material)| Piece { solid: solid.as_ref(), material: *material })
            .collect()
    }

    /// Chunk `key`, made from the pieces: `None` when it holds nothing.
    ///
    /// # Errors
    /// When voxelising it passes its caps ([`cav_core::voxel::world::MAX_CHUNK_WORK`]).
    pub fn chunk(&self, key: ChunkKey) -> Result<Option<Chunk>, String> {
        voxelise_chunk(&self.pieces(), &self.grid, key).map_err(|e| e.0)
    }

    /// Brick `key` of level `level` (1 and up) of the far field, straight from the pieces.
    #[must_use]
    pub fn brick(&self, level: u8, key: ChunkKey) -> Option<Brick> {
        brick_of_pieces(&self.pieces(), &self.grid, level, key)
    }

    /// Whether any piece may have matter in `boxed`: some piece's `cover` of it is not `Outside`
    /// (conservative: `true` may be wrong, `false` never is). What streaming asks before it queues
    /// a chunk or a brick, so that the air above the ground is never made.
    #[must_use]
    pub fn reaches(&self, boxed: &Aabb) -> bool {
        self.solids
            .iter()
            .any(|(s, _)| s.bounds().overlaps(boxed) && s.cover(boxed) != cav_core::voxel::Cover::Outside)
    }

    /// The box holding every piece, or `None` for a world with none.
    #[must_use]
    pub fn extent(&self) -> Option<Aabb> {
        self.solids.iter().map(|(s, _)| s.bounds()).reduce(|a, b| a.union(&b))
    }

    /// About how many finest cells the world's matter fills: each piece's volume over `pitch³`,
    /// overlaps counted twice (a terrain's volume taken at its mean surface). For saying how big a
    /// world is, never for a mass.
    #[must_use]
    pub fn cells(&self, spec: &WorldSpec) -> f64 {
        let volume: f64 = spec
            .pieces
            .iter()
            .map(|p| match &p.kind {
                PieceKind::Box { size, .. } => size[0] * size[1] * size[2],
                PieceKind::Terrain { lo, hi, bottom, level, .. } => {
                    (hi[0] - lo[0]) * (hi[1] - lo[1]) * (level - bottom)
                }
            })
            .sum();
        volume / spec.pitch.powi(3)
    }
}

/// The least distance from `p` to the vertical segment from `base` up `height` metres.
fn to_segment(p: Vec3, base: Vec3, height: f64) -> f64 {
    let dz = if p.z < base.z {
        base.z - p.z
    } else if p.z > base.z + height {
        p.z - base.z - height
    } else {
        0.0
    };
    (p.x - base.x).hypot(p.y - base.y).hypot(dz)
}

/// The least distance from the box `b` to the vertical segment from `base` up `height` metres.
fn box_to_segment(b: &Aabb, base: Vec3, height: f64) -> f64 {
    let gap = |v: f64, lo: f64, hi: f64| (lo - v).max(v - hi).max(0.0);
    let dx = gap(base.x, b.lo.x, b.hi.x);
    let dy = gap(base.y, b.lo.y, b.hi.y);
    let dz = (b.lo.z - (base.z + height)).max(base.z - b.hi.z).max(0.0);
    dx.hypot(dy).hypot(dz)
}

/// A chunk read through its moments: mass, centre of mass, second and third central moments,
/// its radius `a` about the centre (everything of it is within `a`) and a bound on its fourth
/// moment, which together bound what the octupole expansion leaves out.
#[derive(Clone, Copy, Debug)]
pub struct Multipole {
    /// The moments, in the world's frame.
    pub moments: Moments,
    /// The radius about the centre of mass holding all the chunk's matter, metres.
    pub radius: f64,
    /// `Σ m s⁴` over the chunk's matter, `s` the distance from the centre of mass, bounded from
    /// above: each leaf's mass at its farthest point, `|c − centre| + side·√3/2`. kg m⁴.
    pub fourth: f64,
}

/// `√((l+1)(2l+1))` for `l` from 4: the most the gradient of `P_l(cos γ)/R^{l+1}` can be, in
/// units of `1/R^{l+2}` (from `|P_l| ≤ 1` and `(1 − x²)P_l'² ≤ l(l+1)`).
fn gradient_factor(l: u32) -> f64 {
    f64::from((l + 1) * (2 * l + 1)).sqrt()
}

impl Multipole {
    /// The chunk's multipole: its moments, and its radius and fourth moment from its leaves.
    #[must_use]
    pub fn of(chunk: &Chunk, grid: &WorldGrid, densities: &[f64]) -> Self {
        let moments = chunk.moments(grid, densities);
        let (mut radius, mut fourth) = (0f64, 0f64);
        for (centre, mass, side) in chunk.cubes(grid, densities) {
            let far = (centre - moments.centre).length() + 0.5 * side * 3f64.sqrt();
            radius = radius.max(far);
            fourth += mass * far.powi(4);
        }
        Self { moments, radius, fourth }
    }

    /// `−V/G` at offset `d` from the centre of mass, kg/m, to the octupole, without the monopole
    /// (which the caller forms differentially): `Σ S_ab (3 d_a d_b − δ_ab R²)/(2R⁵) +
    /// [5 Σ T_abc d_a d_b d_c − 3R² Σ T_aac d_c]/(2R⁷)`.
    fn higher(&self, d: Vec3) -> f64 {
        let m = &self.moments;
        let d = [d.x, d.y, d.z];
        let r2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        let r = r2.sqrt();
        let mut quad = 0.0;
        let mut cube = 0.0;
        let mut trace = 0.0;
        for a in 0..3 {
            for b in 0..3 {
                let delta = if a == b { r2 } else { 0.0 };
                quad += m.second_at(a, b) * (3.0 * d[a] * d[b] - delta);
                for c in 0..3 {
                    cube += m.third_at(a, b, c) * d[a] * d[b] * d[c];
                }
                trace += m.third_at(a, a, b) * d[b];
            }
        }
        quad / (2.0 * r2 * r2 * r) + (5.0 * cube - 3.0 * r2 * trace) / (2.0 * r2 * r2 * r2 * r)
    }

    /// `(V(upper) − V(lower))/G`, kg/m, at two points on a vertical line: `across` the horizontal
    /// offset from the centre of mass, `below` and `above` the heights over it, `gap = below −
    /// above` as the caller knows it exactly (as [`cube_difference`] takes them).
    #[must_use]
    pub fn difference(&self, across: [f64; 2], below: f64, above: f64, gap: f64) -> f64 {
        let horizontal = across[0] * across[0] + across[1] * across[1];
        let (b, a) = ((horizontal + below * below).sqrt(), (horizontal + above * above).sqrt());
        let monopole = -self.moments.mass * gap * (below + above) / (a * b * (a + b));
        let du = Vec3::new(across[0], across[1], above);
        let dl = Vec3::new(across[0], across[1], below);
        monopole - (self.higher(du) - self.higher(dl))
    }

    /// The most the remainder after the octupole can change `V/G` between two points `separation`
    /// metres apart on a line all of whose points are at least `distance` from the centre of
    /// mass, kg/m. The remainder's gradient is at most `Σ_{l≥4} √((l+1)(2l+1)) Σ m s^l / R^{l+2}`
    /// and `Σ m s^l ≤ fourth · a^{l−4}`, so it is at most
    /// `fourth / R⁶ · Σ_{l≥4} √((l+1)(2l+1)) q^{l−4}`, `q = a/R`; times the separation. Infinite
    /// when `q ≥ 1`.
    #[must_use]
    pub fn bound(&self, distance: f64, separation: f64) -> f64 {
        let q = self.radius / distance;
        if q >= 1.0 {
            return f64::INFINITY;
        }
        // The series to where its terms are below 1e-17 of its first (at most a few hundred).
        let (mut sum, mut power, mut l) = (0.0, 1.0, 4u32);
        while power > 1e-17 && l < 4000 {
            sum += gradient_factor(l) * power;
            power *= q;
            l += 1;
        }
        separation.abs() * self.fourth * sum / distance.powi(6)
    }
}

/// The world as one tower reads it: some chunks leaf by leaf, others through their moments.
pub struct WorldSource {
    /// Leaves as `(centre, m, the world's frame; mass, kg; side, m)`.
    pub cubes: Vec<(Vec3, f64, f64)>,
    /// Chunks read through their moments.
    pub far: Vec<Multipole>,
}

impl Source for WorldSource {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, _time: f64) -> [f64; 2] {
        heights.map(|height| {
            let mut sum = 0.0;
            for &(centre, mass, side) in &self.cubes {
                let at = centre - tower;
                let (below, above) = (height + lower - at.z, height + upper - at.z);
                sum += cube_difference(mass, side, [-at.x, -at.y], below, above, lower - upper);
            }
            for far in &self.far {
                let at = far.moments.centre - tower;
                let (below, above) = (height + lower - at.z, height + upper - at.z);
                sum += far.difference([-at.x, -at.y], below, above, lower - upper);
            }
            sum
        })
    }
}

/// The bound on what reading [`WorldSource::far`] through moments leaves out: at each instant,
/// for each interferometer, the sum of each chunk's [`Multipole::bound`] between the two arm
/// points. Read through the flight like any source; the quadrature's weights are positive, so its
/// reading bounds the error of the far chunks' reading, in radians.
pub struct BoundSource<'a> {
    /// The chunks read through their moments.
    pub far: &'a [Multipole],
}

impl Source for BoundSource<'_> {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, _time: f64) -> [f64; 2] {
        heights.map(|height| {
            let (z0, z1) = (tower.z + height + lower.min(upper), (upper - lower).abs());
            let base = Vec3::new(tower.x, tower.y, z0);
            self.far.iter().map(|far| far.bound(to_segment(far.moments.centre, base, z1), z1)).sum()
        })
    }
}

/// A source that reads `1 kg/m` at every instant: what turns kg/m into radians for a reader.
struct Unit;

impl Source for Unit {
    fn difference(&self, _: Vec3, _: [f64; 2], _: f64, _: f64, _: f64) -> [f64; 2] {
        [1.0; 2]
    }
}

/// The world's chunks a read takes, nearest first: within `read.radius` of some tower's atoms'
/// path, at most [`MAX_READ_CHUNKS`]. Returns the keys and what was left out, in words.
///
/// # Errors
/// When the search itself would look at more than [`MAX_READ_SCAN`] keys.
pub fn read_keys(spec: &crate::SceneSpec, world: &World) -> Result<(Vec<ChunkKey>, Vec<String>), String> {
    let Some(w) = &spec.world else { return Ok((Vec::new(), Vec::new())) };
    let Some(extent) = world.extent() else { return Ok((Vec::new(), Vec::new())) };
    let radius = w.read.radius;
    let mut found: std::collections::BTreeMap<ChunkKey, f64> = std::collections::BTreeMap::new();
    let mut scanned = 0u64;
    for n in 0..spec.array.towers.len() {
        let base = Vec3::from_array(spec.array.towers[n].position);
        let height = spec.array.instrument_of(n).tower_height;
        let reach = Aabb::new(base, base + Vec3::new(0.0, 0.0, height)).grown(radius);
        if !reach.overlaps(&extent) {
            continue;
        }
        let clipped = Aabb::new(
            Vec3::new(reach.lo.x.max(extent.lo.x), reach.lo.y.max(extent.lo.y), reach.lo.z.max(extent.lo.z)),
            Vec3::new(reach.hi.x.min(extent.hi.x), reach.hi.y.min(extent.hi.y), reach.hi.z.min(extent.hi.z)),
        );
        let (lo, hi) = world.grid.bricks_of(0, &clipped);
        let span = |a: i32, b: i32| u64::from((i64::from(b) - i64::from(a) + 1).max(0) as u32);
        scanned += span(lo.x, hi.x) * span(lo.y, hi.y) * span(lo.z, hi.z);
        if scanned > MAX_READ_SCAN {
            return Err(format!(
                "the world's chunks within {radius} m of the towers are more than {MAX_READ_SCAN} to look at; \
                 use a smaller world.read.radius or a coarser pitch"
            ));
        }
        for x in lo.x..=hi.x {
            for y in lo.y..=hi.y {
                for z in lo.z..=hi.z {
                    let key = ChunkKey::new(x, y, z);
                    let distance = box_to_segment(&world.grid.chunk_box(key), base, height);
                    if distance <= radius {
                        let entry = found.entry(key).or_insert(distance);
                        *entry = entry.min(distance);
                    }
                }
            }
        }
    }
    let mut keys: Vec<(ChunkKey, f64)> = found.into_iter().collect();
    keys.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    let mut said = Vec::new();
    if keys.len() > MAX_READ_CHUNKS {
        said.push(format!(
            "the world has {} chunks within {radius} m of the towers; the nearest {MAX_READ_CHUNKS} (to {:.1} m) are \
             read and the rest are not",
            keys.len(),
            keys[MAX_READ_CHUNKS - 1].1
        ));
        keys.truncate(MAX_READ_CHUNKS);
    }
    let beyond = spec.array.towers.iter().enumerate().any(|(n, t)| {
        let base = Vec3::from_array(t.position);
        let h = spec.array.instrument_of(n).tower_height;
        (0..8).any(|c| to_segment(extent.corner(c), base, h) > radius)
    });
    if beyond {
        said.push(format!(
            "world matter farther than {radius} m from a tower's atoms is not read (S5 reads the rest)"
        ));
    }
    Ok((keys.into_iter().map(|(k, _)| k).collect(), said))
}

/// What reading the world at one tower found.
pub struct TowerRead {
    /// The reading, with the quadrature's audit.
    pub audited: Audited,
    /// The bound on the moments' approximation for each interferometer, radians; 0 when no chunk
    /// went by its moments.
    pub bound: [f64; 2],
    /// Chunks read leaf by leaf, and through their moments.
    pub counts: [usize; 2],
}

/// A leaf as a uniform cube: `(centre, m, the world's frame; mass, kg; side, m)`.
pub type Cube = (Vec3, f64, f64);

/// The world's chunks made for a read, each with its leaves as cubes and its multipole.
pub struct Made {
    /// For each chunk with matter: its cubes and its multipole.
    pub chunks: Vec<(Vec<Cube>, Multipole)>,
}

/// Makes the chunks of `keys` (empty ones dropped). `cancelled` is asked between chunks.
///
/// # Errors
/// When a chunk cannot be voxelised, with why.
pub fn make(
    world: &World,
    keys: &[ChunkKey],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Option<Made>, String> {
    let mut chunks = Vec::new();
    for key in keys {
        if cancelled() {
            return Ok(None);
        }
        if let Some(chunk) = world.chunk(*key)? {
            let cubes = chunk.cubes(&world.grid, &world.densities);
            let multipole = Multipole::of(&chunk, &world.grid, &world.densities);
            if multipole.moments.mass > 0.0 {
                chunks.push((cubes, multipole));
            }
        }
    }
    Ok(Some(Made { chunks }))
}

/// Reads the world's `made` chunks at the tower whose base is `base` and whose height is
/// `height`, with `reader`, at `readout`: each chunk through its moments when its bound fits a
/// `tolerance / chunks` share (radians), else leaf by leaf; within `max_work` cube evaluations.
///
/// # Errors
/// When even the first rule of the read would pass `max_work`, with the numbers.
pub fn read_tower(
    made: &Made,
    reader: &Reader,
    base: Vec3,
    height: f64,
    readout: f64,
    tolerance: f64,
    max_work: u64,
) -> Result<TowerRead, String> {
    // Radians a kilogram per metre held through the whole flight is: the scale of a bound.
    let per = reader.read(base, &Unit, readout);
    let scale = per.lower.abs().max(per.upper.abs());
    let share = tolerance / made.chunks.len().max(1) as f64;
    let mut source = WorldSource { cubes: Vec::new(), far: Vec::new() };
    for (cubes, multipole) in &made.chunks {
        let distance = to_segment(multipole.moments.centre, base, height);
        // The arms are never farther apart than the tower is tall.
        let worst = multipole.bound(distance, height) * scale;
        if worst <= share && multipole.radius < 0.5 * distance {
            source.far.push(*multipole);
        } else {
            source.cubes.extend_from_slice(cubes);
        }
    }
    let counts = [made.chunks.len() - source.far.len(), source.far.len()];
    // Each flight node evaluates every cube, or a multipole (counted as four cubes), at two
    // points for each of two interferometers.
    let per_node = 4 * (source.cubes.len() as u64 + 4 * source.far.len() as u64);
    let first = reader.instrument().node_count(&[]);
    if per_node > 0 && first * per_node > max_work {
        return Err(format!(
            "{} cubes and {} chunks by their moments × {first} flight nodes is more than the {max_work} \
             evaluations one read of the world may make; use a smaller world.read.radius, a coarser pitch or a \
             larger world.read.tolerance",
            source.cubes.len(),
            source.far.len()
        ));
    }
    let max_nodes = max_work.checked_div(per_node).unwrap_or(u64::MAX);
    let audited = reader.read_to_within(base, &source, readout, PHASE_TOLERANCE, max_nodes);
    let bound = if source.far.is_empty() {
        [0.0; 2]
    } else {
        let b = reader.read(base, &BoundSource { far: &source.far }, readout);
        [b.lower.abs(), b.upper.abs()]
    };
    Ok(TowerRead { audited, bound, counts })
}

/// The share of [`MAX_KERNEL_WORK`] the world's read may use: the world counts as one voxel body
/// in [`crate::read::kernel_share`].
#[must_use]
pub fn work_share(spec: &crate::SceneSpec) -> u64 {
    crate::read::kernel_share(spec).min(MAX_KERNEL_WORK)
}

pub mod clipmap;

#[cfg(test)]
mod tests;
