//! The world volume of a scene: static matter (terrain, buildings, slabs) as pieces of one world,
//! voxelised into chunks ([`cav_core::voxel::world`]) and read by the instrument like a body.
//!
//! **What a world is** ([`WorldSpec`]): a pitch (the finest cell), its materials (density and
//! colour), and its pieces, each a solid of one material, in order of precedence where they
//! overlap. Chunks of 32³ cells are made from the pieces when they are wanted (by the read here,
//! by the engine's streaming for the picture) and are never part of the spec.
//!
//! **How it is read** ([`read_world`], source `World`; `docs/slices/S5.md` §2). The world is
//! static, so each tower's reading is made once and holds for every shot. The whole world is
//! read: its chunks merged upward into one octree of summaries ([`octree`], made anew for each
//! read: this crate keeps nothing between calls), and each tower's walk ([`walk`]) picks the cut
//! it reads by phase error on one global ledger: the node whose bound is largest is opened first,
//! until the bounds of the nodes it accepts sum to `read.tolerance` radians. An accepted node is
//! read through its expansion to the octupole about its centre; an opened chunk is opened into
//! its own tree's blocks ([`inner`]), each a node like any other, and what is left of it is read
//! leaf by leaf, each leaf a uniform cube with the exact kernel; every chunk within `read.radius` (the bubble)
//! of the atoms' path is opened. The bound (S4's, `docs/slices/S4.md` §2): the remainder after the
//! octupole has a gradient at most `Σ_{l≥4} (l+1) A_l / R^{l+2}` ([`Summary::bound`]); the accepted
//! nodes' bounds are read through the flight ([`walk::LedgerSource`]) and the ledger so made is
//! added to the read's error estimate, so the error the read reports covers the approximation. A
//! walk stopped by its caps says so and by how much its ledger is over (it is not settled); a
//! world past the octree's caps is not read, and says why.
//!
//! Frames and units: the world's frame (x east, y north, z up), metres, kilograms, kg/m³, radians.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Mutex, PoisonError};

use cav_core::Vec3;
use cav_core::forward::{Audited, PHASE_TOLERANCE, Reader, Source};
use cav_core::voxel::kernel::{MAX_KERNEL_WORK, cube_difference};
use cav_core::voxel::shape::Cuboid;
#[cfg(doc)]
use cav_core::voxel::summary::Summary;
use cav_core::voxel::world::{
    Brick, Chunk, ChunkKey, MAX_MATERIALS, Piece, Solid, Terrain, WorldGrid, brick_of_pieces, voxelise_chunk,
};
use cav_core::voxel::{Aabb, MAX_SUPERSAMPLE};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::spec::{SpecError, limits};
use octree::Octree;

/// The most pieces a world may have.
pub const MAX_PIECES: usize = 256;
/// The widest bubble (`world.read.radius`), metres. A bubble that opens more than the read's work
/// allows stops the walk, which says so ([`walk`]).
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
    /// is weighed. The default is 2. The read's error estimate does not include this
    /// voxelisation error (the matter as cells against the pieces as solids): it covers the
    /// quadrature and the moments' bound for the matter as voxelised, as for a voxel body.
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

/// One material of a world: the scenes' one kind of material ([`crate::materials`]), indexed by
/// the world's pieces.
pub use crate::materials::MaterialSpec;

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
    /// The bubble: every chunk within this distance of a tower's atoms' path is read leaf by leaf,
    /// whatever its bound. The whole world is read either way; farther matter goes by the walk.
    /// Metres, 0 to 1e4; default 0 (only what the path passes through or touches).
    #[schemars(range(min = 0.0, max = 1.0e4))]
    #[schemars(extend("x-unit" = "m"))]
    pub radius: f64,
    /// The world's error budget, radians: the walk accepts nodes through their expansion until
    /// the bounds on what they leave out (the ledger) sum to this, and the quadrature is audited
    /// to it when it is looser than the phase tolerance. Default 1e-10 (the phase tolerance);
    /// larger is a cheaper read, as far off as said.
    #[schemars(range(min = 0.0, max = 1.0))]
    #[schemars(extend("x-unit" = "rad"))]
    pub tolerance: f64,
}

impl Default for WorldReadSpec {
    fn default() -> Self {
        Self { radius: 0.0, tolerance: PHASE_TOLERANCE }
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
            m.validate(&format!("world.materials[{n}]"))?;
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
        let text = self.chunk_text();
        text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
    }

    /// The JSON of everything that decides the world's chunks: what [`Self::hash`] hashes.
    fn chunk_text(&self) -> String {
        let densities: Vec<u64> = self.materials.iter().map(|m| m.density.to_bits()).collect();
        serde_json::to_string(&(self.pitch, self.supersample, self.origin, densities, &self.pieces))
            .unwrap_or_default()
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
pub(crate) fn to_segment(p: Vec3, base: Vec3, height: f64) -> f64 {
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
pub(crate) fn box_to_segment(b: &Aabb, base: Vec3, height: f64) -> f64 {
    let gap = |v: f64, lo: f64, hi: f64| (lo - v).max(v - hi).max(0.0);
    let dx = gap(base.x, b.lo.x, b.hi.x);
    let dy = gap(base.y, b.lo.y, b.hi.y);
    let dz = (b.lo.z - (base.z + height)).max(base.z - b.hi.z).max(0.0);
    dx.hypot(dy).hypot(dz)
}

/// A leaf as a uniform cube: `(centre, m, the world's frame; mass, kg; side, m)`.
pub type Cube = (Vec3, f64, f64);

/// The world as one tower reads it: the opened chunks leaf by leaf, and the far field of its cut.
pub struct WorldSource {
    /// The opened chunks' leaves.
    pub cubes: Vec<Cube>,
    /// The accepted nodes: near ones directly, the rest through the tower's local expansion
    /// ([`local::Local`]).
    pub far: local::Local,
}

/// Leaves a [`WorldSource`] sums on one thread: past this, its leaves are summed in blocks of
/// this many on [`crate::read::threads`] threads and the blocks' sums added in order, so the bits
/// do not depend on the number of threads.
pub const LEAF_BLOCK: usize = 1 << 12;

/// `(V(upper) − V(lower))/G` of `cubes` at the line through `tower`, kg/m, in order.
fn leaves_at(cubes: &[Cube], tower: Vec3, height: f64, lower: f64, upper: f64) -> f64 {
    let mut sum = 0.0;
    for &(centre, mass, side) in cubes {
        let at = centre - tower;
        let (below, above) = (height + lower - at.z, height + upper - at.z);
        sum += cube_difference(mass, side, [-at.x, -at.y], below, above, lower - upper);
    }
    sum
}

impl Source for WorldSource {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, _time: f64) -> [f64; 2] {
        heights.map(|height| {
            let leaves = if self.cubes.len() <= LEAF_BLOCK {
                leaves_at(&self.cubes, tower, height, lower, upper)
            } else {
                let blocks: Vec<&[Cube]> = self.cubes.chunks(LEAF_BLOCK).collect();
                let next = AtomicUsize::new(0);
                let sums: Vec<Mutex<f64>> = blocks.iter().map(|_| Mutex::new(0.0)).collect();
                let work = || {
                    loop {
                        let at = next.fetch_add(1, AtomicOrdering::Relaxed);
                        let Some(block) = blocks.get(at) else { break };
                        *sums[at].lock().unwrap_or_else(PoisonError::into_inner) =
                            leaves_at(block, tower, height, lower, upper);
                    }
                };
                let workers = crate::read::threads().clamp(1, blocks.len());
                std::thread::scope(|scope| {
                    for _ in 1..workers {
                        scope.spawn(work);
                    }
                    work();
                });
                sums.into_iter().map(|s| s.into_inner().unwrap_or_else(PoisonError::into_inner)).sum()
            };
            leaves + self.far.at(tower, height, lower, upper)
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

/// Radians the flight of `reader` at `base` makes of 1 kg/m held at every instant, the two
/// interferometers' added: the walk's [`walk::Aim::per`].
#[must_use]
pub fn radians_per_kg_m(reader: &Reader, base: Vec3, readout: f64) -> f64 {
    let per = reader.read(base, &Unit, readout);
    per.lower.abs() + per.upper.abs()
}

/// The chunks `keys` (sorted, each once) made from the pieces, each as its leaves and its own
/// tree's blocks ([`inner::ChunkTree`]), on [`crate::read::threads`] threads, in key order.
/// `Ok(None)` when cancelled.
///
/// # Errors
/// When a chunk cannot be voxelised: the first such by key, whatever the threads.
pub fn open(
    world: &World,
    keys: &[ChunkKey],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Option<Vec<inner::ChunkTree>>, String> {
    type Made = (usize, Result<inner::ChunkTree, String>);
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let was_cancelled = AtomicBool::new(false);
    let found: Mutex<Vec<Made>> = Mutex::new(Vec::with_capacity(keys.len()));
    let work = || {
        let mut mine: Vec<Made> = Vec::new();
        while !stop.load(AtomicOrdering::Relaxed) {
            let at = next.fetch_add(1, AtomicOrdering::Relaxed);
            if at >= keys.len() {
                break;
            }
            if cancelled() {
                was_cancelled.store(true, AtomicOrdering::Relaxed);
                stop.store(true, AtomicOrdering::Relaxed);
                break;
            }
            let key = keys[at];
            let made = world.chunk(key).map(|chunk| {
                chunk
                    .map_or_else(|| inner::ChunkTree::empty(key), |chunk| inner::ChunkTree::of(world, &chunk))
            });
            if made.is_err() {
                stop.store(true, AtomicOrdering::Relaxed);
            }
            mine.push((at, made));
        }
        found.lock().unwrap_or_else(PoisonError::into_inner).extend(mine);
    };
    let workers = crate::read::threads().clamp(1, keys.len().max(1));
    std::thread::scope(|scope| {
        for _ in 1..workers {
            scope.spawn(work);
        }
        work();
    });
    if was_cancelled.load(AtomicOrdering::Relaxed) {
        return Ok(None);
    }
    let mut found = found.into_inner().unwrap_or_else(PoisonError::into_inner);
    found.sort_by_key(|(at, _)| *at);
    let mut out = Vec::with_capacity(found.len());
    for (at, made) in found {
        out.push(made.map_err(|why| format!("chunk {:?}: {why}", keys[at].to_array()))?);
    }
    Ok(Some(out))
}

/// What reading the world at one tower found.
pub struct TowerRead {
    /// The reading, with the quadrature's audit.
    pub audited: Audited,
    /// The ledger for each interferometer, radians: the accepted nodes' bounds read through the
    /// flight ([`walk::LedgerSource`]); 0 when the cut accepted nothing.
    pub ledger: [f64; 2],
    /// The tower's cut.
    pub cut: walk::Cut,
    /// What the local expansion of its far field found: its degree, its nodes, its audit
    /// (radians, added to the read's estimate beside the ledger).
    pub local: local::LocalAudit,
}

/// The most work one flight node of `reader` may do for one point pair, from the read's share of
/// the kernel's work (`share` cube evaluations, [`work_share`]): each node evaluates every leaf
/// and every accepted node (four leaves' worth) at two points for each of two interferometers.
#[must_use]
pub fn max_work(reader: &Reader, share: u64) -> u64 {
    share.checked_div(4 * reader.instrument().node_count(&[])).unwrap_or(u64::MAX)
}

/// Reads the world at the tower `aim` describes (its base, height and flight's scale), with
/// `reader`, at `readout`: the leaves `cut` reads one by one (from `trees`, which hold at least
/// its opened chunks', sorted by key) with the exact kernel, and its accepted nodes by `aim.rule`, the
/// far ones through the tower's local expansion ([`local::Local::gather`], its audit within
/// [`local::LOCAL_SHARE`] of `tolerance`); the quadrature audited to the larger of the phase
/// tolerance and `tolerance` (radians, the world's), within `share` cube evaluations in all.
///
/// # Errors
/// When even the first rule of the read would pass `share`, with the numbers; or a chunk the cut
/// opened is not in `opened`.
#[allow(clippy::too_many_arguments)]
pub fn read_tower(
    octree: &Octree,
    trees: &[inner::ChunkTree],
    reader: &Reader,
    aim: &walk::Aim,
    readout: f64,
    tolerance: f64,
    cut: walk::Cut,
    share: u64,
) -> Result<TowerRead, String> {
    let mut cubes = Vec::with_capacity(usize::try_from(cut.leaves).unwrap_or(0));
    let tree = |key: &ChunkKey| {
        trees
            .binary_search_by_key(key, |t| t.key)
            .map(|at| &trees[at])
            .map_err(|_| format!("chunk {:?} was opened but not made", key.to_array()))
    };
    for key in &cut.whole {
        cubes.extend_from_slice(&tree(key)?.cubes);
    }
    for (key, run) in &cut.runs {
        cubes.extend_from_slice(&tree(key)?.cubes[run.start as usize..run.end as usize]);
    }
    let rule = aim.rule;
    let nodes = walk::far_field(octree, trees, &cut, rule).nodes;
    let along = local::Along {
        base: aim.base,
        height: aim.height,
        per: aim.per,
        share: local::LOCAL_SHARE * tolerance,
        most: local::MAX_LOCAL_DEGREE,
        forced: None,
    };
    let source = WorldSource { cubes, far: local::Local::gather(nodes, rule, &along) };
    // Each flight node evaluates every cube, and the far field (an accepted node read directly
    // counted as four cubes, the series by its terms), at two points for each of two
    // interferometers.
    let per_node = 4 * (source.cubes.len() as u64 + source.far.work());
    let first = reader.instrument().node_count(&[]);
    if per_node > 0 && first * per_node > share {
        return Err(format!(
            "{} cubes and {} nodes by their moments × {first} flight nodes is more than the {share} evaluations \
             one read of the world may make; use a smaller world.read.radius, a coarser pitch or a larger \
             world.read.tolerance",
            source.cubes.len(),
            source.far.near.nodes.len() + source.far.far.nodes.len()
        ));
    }
    let max_nodes = share.checked_div(per_node).unwrap_or(u64::MAX);
    // The quadrature is audited to the world's own budget when it is looser than the phase
    // tolerance: a scene that asks for its world within 1e-3 rad does not pay for 1e-10.
    let audited =
        reader.read_to_within(aim.base, &source, readout, PHASE_TOLERANCE.max(tolerance), max_nodes);
    // The ledger: the accepted nodes' bounds read through the flight, the near and the far ones
    // (each reading is of positive bounds through positive weights, so the two add).
    let mut ledger = [0.0; 2];
    for nodes in [&source.far.near.nodes, &source.far.far.nodes] {
        if !nodes.is_empty() {
            let b = reader.read(aim.base, &walk::LedgerSource { nodes, rule }, readout);
            ledger[0] += b.lower.abs();
            ledger[1] += b.upper.abs();
        }
    }
    Ok(TowerRead { audited, ledger, cut, local: source.far.found })
}

/// The whole world read at every tower of `spec`: the octree ([`Octree::build`]), each tower's walk
/// into the trees of the chunks it opens ([`walk::walk_into`]; a chunk any tower opened is made
/// once), then each tower read ([`read_tower`]) at `readouts[n]`. `Ok(None)` when cancelled.
///
/// # Errors
/// When the octree is refused (its caps), a chunk cannot be voxelised, or a tower's read would
/// pass the kernel's share, with why.
pub fn read_world(
    spec: &crate::SceneSpec,
    readers: &crate::read::Readers,
    readouts: &[f64],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Option<Vec<TowerRead>>, String> {
    let Some(world_spec) = &spec.world else { return Ok(Some(Vec::new())) };
    let world = World::new(world_spec);
    let Some(octree) = Octree::build(&world, cancelled)? else { return Ok(None) };
    let share = work_share(spec);
    let mut cuts = Vec::with_capacity(spec.array.towers.len());
    let mut aims = Vec::with_capacity(spec.array.towers.len());
    let mut trees = Vec::new();
    for (at, tower) in spec.array.towers.iter().enumerate() {
        if cancelled() {
            return Ok(None);
        }
        let reader = readers.of(at);
        let base = Vec3::from_array(tower.position);
        let aim = walk::Aim {
            base,
            height: spec.array.instrument_of(at).tower_height,
            per: radians_per_kg_m(reader, base, readouts[at]),
            // The walk's ledger takes what the local expansion's audit leaves of the tolerance.
            tolerance: (1.0 - local::LOCAL_SHARE) * world_spec.read.tolerance,
            bubble: world_spec.read.radius,
            max_work: max_work(reader, share),
            max_nodes: walk::MAX_CUT_NODES,
            rule: walk::Rule::Octupole,
        };
        let Some(cut) = walk::walk_into(&octree, &aim, &world, &mut trees, cancelled)? else {
            return Ok(None);
        };
        cuts.push(cut);
        aims.push(aim);
    }
    let mut out = Vec::with_capacity(cuts.len());
    for (at, (aim, cut)) in aims.iter().zip(cuts).enumerate() {
        if cancelled() {
            return Ok(None);
        }
        let read = read_tower(
            &octree,
            &trees,
            readers.of(at),
            aim,
            readouts[at],
            world_spec.read.tolerance,
            cut,
            share,
        )?;
        out.push(read);
    }
    Ok(Some(out))
}

/// The share of [`MAX_KERNEL_WORK`] the world's read may use: the world counts as one voxel body
/// in [`crate::read::kernel_share`].
#[must_use]
pub fn work_share(spec: &crate::SceneSpec) -> u64 {
    crate::read::kernel_share(spec).min(MAX_KERNEL_WORK)
}

pub mod clipmap;
pub mod inner;
pub mod local;
pub mod octree;
pub mod walk;

#[cfg(test)]
mod tests;
