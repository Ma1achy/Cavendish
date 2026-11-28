//! The world's infinite base (`world.base`, `docs/slices/S5.md` §2 "Infinite bases", TASK-S5-13;
//! `docs/research/s5-world-octree.md` §4): a flat slab or a layered planet under the pieces, read
//! in closed form, with the pieces read as **departures** from it.
//!
//! **The three bases** ([`BaseSpec`]). `Patch` is the finite world of S4 (the pieces and nothing
//! else; the default, never written to a file). `Plane` is a uniform slab that extends to the
//! horizon, its top at `surface`: above it the field is uniform and its gradient zero (S1's
//! [`Slab`]). `Sphere` is a planet whose centre is at `(0, 0, −radius)` (so its north pole is the
//! world's origin), made of uniform concentric layers: outside them each is a point mass at the
//! centre (Newton's shell theorem), so the base reads as one point mass of their sum ([`Planet`]).
//!
//! **Departures** ([`Overlap`], [`merge`]). A piece is matter put where the base's matter was not,
//! or put in place of it: inside the base a piece of density `ρ` changes the matter by
//! `ρ − ρ_base`. So a chunk's matter is its pieces' cells (as for a patch) plus a second
//! voxelisation, of the **overlap** of the pieces with the base, at minus the base's density, the
//! two added cell by cell into one list of signed leaves. One code path for both bases: the
//! overlap is the pieces' union intersected with the base's [`BaseSolid`], weighed by the same
//! sample points as the pieces, so a cell a curved surface cuts is weighed as the pieces' cells
//! are. A building on the plane adds its mass; its foundation below the surface adds
//! `ρ_building − ρ_slab`; a ditch (a piece of a material of no density, "air") carves `−ρ_slab`.
//!
//! **What must not be done.** A piece of a planet may not reach into its layers below the top one
//! (refused in words: the overlap is taken at the top layer's density only). A tower must stand
//! on or above the base's surface ([`Base::stands_above`]): the closed forms hold only there.
//! The base's matter is never voxelised and never in the octree; the walk, its ledger and the
//! local expansion see only the departures, whose masses are signed (every bound of
//! [`cav_core::voxel::summary::Summary`] is of `|m|`).
//!
//! Frames and units: the world's frame (x east, y north, z up), metres, kilograms, kg/m³.

use cav_core::Vec3;
use cav_core::forward::{Mass, Slab, Source};
use cav_core::voxel::morton::{self, span};
use cav_core::voxel::world::{CHUNK_DEPTH, Chunk, Solid, WorldGrid};
use cav_core::voxel::{Aabb, Cover};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Cube;
use crate::spec::{SpecError, limits};

/// The most layers a planet may have.
pub const MAX_LAYERS: usize = 64;
/// The largest planet, metres: the Earth's radius is 6.4×10⁶ m.
pub const MAX_PLANET_RADIUS: f64 = 1.0e7;
/// The thickest plane, metres.
pub const MAX_PLANE_THICKNESS: f64 = 1.0e5;

/// What lies under a world's pieces (`world.base`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum BaseSpec {
    /// Nothing: the world is its pieces (a finite patch). The default.
    #[default]
    Patch,
    /// A uniform slab that extends to the horizon, its top at `surface`: uniform `g`, no
    /// gradient. The pieces are departures from it.
    Plane {
        /// The height of its top, metres, up. Every tower stands on or above it.
        #[schemars(extend("x-unit" = "m"))]
        surface: f64,
        /// Its density, kg/m³.
        #[schemars(range(min = 0.0, max = 1.0e5))]
        #[schemars(extend("x-unit" = "kg/m3"))]
        density: f64,
        /// How deep it goes below `surface`, metres.
        #[schemars(range(min = 0.0, max = 1.0e5))]
        #[schemars(extend("x-unit" = "m"))]
        thickness: f64,
    },
    /// A layered planet whose centre is at `(0, 0, −radius)`: its surface passes through the
    /// world's origin, the north pole. The pieces are departures from it, none reaching below
    /// its top layer.
    Sphere {
        /// Its radius, metres, at most 1e7.
        #[schemars(range(min = 0.0, max = 1.0e7))]
        #[schemars(extend("x-unit" = "m"))]
        radius: f64,
        /// Its layers from the surface down, 1 to 64; their thicknesses add to at most `radius`
        /// (any rest at the centre is empty).
        layers: Vec<LayerSpec>,
    },
}

/// One layer of a planet: a uniform shell.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayerSpec {
    /// How thick it is, metres, from the layer above it (or the surface) down.
    #[schemars(extend("x-unit" = "m"))]
    pub thickness: f64,
    /// Its density, kg/m³.
    #[schemars(range(min = 0.0, max = 1.0e5))]
    #[schemars(extend("x-unit" = "kg/m3"))]
    pub density: f64,
}

impl BaseSpec {
    /// Whether it is the default, `Patch` (so that a file that does not say it is unchanged).
    #[must_use]
    pub fn is_patch(&self) -> bool {
        *self == Self::Patch
    }

    /// Checks its numbers, and that no piece (by its box, `pieces`, labelled) reaches below a
    /// planet's top layer.
    ///
    /// # Errors
    /// The first number out of its range, or the first piece that reaches too deep, in words.
    pub fn validate(&self, pieces: &[(String, Aabb)]) -> Result<(), SpecError> {
        match self {
            Self::Patch => Ok(()),
            Self::Plane { surface, density, thickness } => {
                crate::spec::finite("world.base.Plane.surface", &[*surface])?;
                crate::spec::positive("world.base.Plane.density", *density, limits::DENSITY)?;
                crate::spec::positive("world.base.Plane.thickness", *thickness, MAX_PLANE_THICKNESS)
            }
            Self::Sphere { radius, layers } => {
                crate::spec::positive("world.base.Sphere.radius", *radius, MAX_PLANET_RADIUS)?;
                if layers.is_empty() || layers.len() > MAX_LAYERS {
                    return Err(SpecError(format!(
                        "world.base.Sphere.layers: {} layers; a planet has 1 to {MAX_LAYERS}",
                        layers.len()
                    )));
                }
                let mut depth = 0.0;
                for (n, layer) in layers.iter().enumerate() {
                    let at = format!("world.base.Sphere.layers[{n}]");
                    crate::spec::positive(&format!("{at}.thickness"), layer.thickness, MAX_PLANET_RADIUS)?;
                    crate::spec::positive(&format!("{at}.density"), layer.density, limits::DENSITY)?;
                    depth += layer.thickness;
                }
                if depth > *radius * (1.0 + 4.0 * f64::EPSILON) {
                    return Err(SpecError(format!(
                        "world.base.Sphere.layers: their thicknesses add to {depth} m, more than the radius, {radius} m"
                    )));
                }
                let inner = radius - layers[0].thickness;
                let centre = Vec3::new(0.0, 0.0, -radius);
                for (n, (label, boxed)) in pieces.iter().enumerate() {
                    // The nearest point of the piece's box to the centre.
                    let near = Vec3::new(
                        centre.x.clamp(boxed.lo.x, boxed.hi.x),
                        centre.y.clamp(boxed.lo.y, boxed.hi.y),
                        centre.z.clamp(boxed.lo.z, boxed.hi.z),
                    );
                    let from = (near - centre).length();
                    if from < inner {
                        return Err(SpecError(format!(
                            "world.pieces[{n}] (`{label}`): its box comes within {from:.3} m of the planet's centre, \
                             below its top layer ({inner:.3} m to {radius} m from the centre); a piece is a departure \
                             from the top layer and may not reach below it"
                        )));
                    }
                }
                Ok(())
            }
        }
    }
}

/// A base made ready to read: its solid, the density its overlap is taken at, and its closed form.
#[derive(Clone, Debug, PartialEq)]
pub enum Base {
    /// The infinite slab from `surface − thickness` to `surface`.
    Plane {
        /// Its top, metres, up.
        surface: f64,
        /// kg/m³.
        density: f64,
        /// Metres.
        thickness: f64,
    },
    /// A planet of radius `radius` centred at `(0, 0, −radius)`.
    Sphere {
        /// Metres.
        radius: f64,
        /// The top layer's density, kg/m³: what a departure is taken against.
        top: f64,
        /// The layers' mass in all, kg, by the shell theorem a point at the centre.
        mass: f64,
        /// Each layer's mass, kg, from the surface down.
        layers: Vec<f64>,
    },
}

impl Base {
    /// The base of `spec`, or `None` for a patch.
    #[must_use]
    pub fn of(spec: &BaseSpec) -> Option<Self> {
        match spec {
            BaseSpec::Patch => None,
            BaseSpec::Plane { surface, density, thickness } => {
                Some(Self::Plane { surface: *surface, density: *density, thickness: *thickness })
            }
            BaseSpec::Sphere { radius, layers } => {
                let mut outer = *radius;
                let masses: Vec<f64> = layers
                    .iter()
                    .map(|layer| {
                        let inner = (outer - layer.thickness).max(0.0);
                        // (4/3)π (r₁³ − r₀³) without the cancellation of two large cubes.
                        let shell = (outer - inner) * (outer * outer + outer * inner + inner * inner);
                        outer = inner;
                        4.0 / 3.0 * std::f64::consts::PI * layer.density * shell
                    })
                    .collect();
                Some(Self::Sphere {
                    radius: *radius,
                    top: layers.first().map_or(0.0, |l| l.density),
                    mass: masses.iter().sum(),
                    layers: masses,
                })
            }
        }
    }

    /// The density, kg/m³, the overlap of the pieces with it is taken at (with a minus sign).
    #[must_use]
    pub const fn density(&self) -> f64 {
        match self {
            Self::Plane { density, .. } => *density,
            Self::Sphere { top, .. } => *top,
        }
    }

    /// Its solid: what the overlap is the pieces' intersection with.
    #[must_use]
    pub const fn solid(&self) -> BaseSolid {
        match self {
            Self::Plane { surface, thickness, .. } => {
                BaseSolid::Slab { bottom: *surface - *thickness, top: *surface }
            }
            Self::Sphere { radius, .. } => BaseSolid::Ball { radius: *radius },
        }
    }

    /// Whether `point` (a tower's base) is on or above the surface, where the closed form holds.
    #[must_use]
    pub fn stands_above(&self, point: Vec3) -> bool {
        match self {
            Self::Plane { surface, .. } => point.z >= *surface,
            Self::Sphere { radius, .. } => ball_excess(point, *radius) >= 0.0,
        }
    }

    /// Where its surface is, in words, for a refusal.
    #[must_use]
    pub fn surface_words(&self) -> String {
        match self {
            Self::Plane { surface, .. } => format!("the plane's surface at z = {surface} m"),
            Self::Sphere { radius, .. } => {
                format!("the planet's surface ({radius} m from (0, 0, −{radius}))")
            }
        }
    }

    /// Its closed form as a source.
    #[must_use]
    pub fn source(&self) -> BaseSource {
        match self {
            Self::Plane { density, thickness, .. } => {
                BaseSource::Plane(Slab { density: *density, thickness: *thickness })
            }
            Self::Sphere { radius, mass, .. } => {
                BaseSource::Sphere(Planet { centre: Vec3::new(0.0, 0.0, -radius), mass: *mass })
            }
        }
    }
}

/// `|p − c|² − R²` for the ball of radius `R` centred at `c = (0, 0, −R)`, formed as
/// `x² + y² + z(z + 2R)`: no cancellation of two numbers of the size `R²` near the surface, where
/// pieces and towers are. Negative inside, zero on the surface.
#[must_use]
pub fn ball_excess(p: Vec3, radius: f64) -> f64 {
    p.x * p.x + p.y * p.y + p.z * (p.z + 2.0 * radius)
}

/// A base's matter as a solid, for the overlap of the pieces with it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BaseSolid {
    /// `bottom ≤ z ≤ top`, everywhere across.
    Slab {
        /// Metres, up.
        bottom: f64,
        /// Metres, up.
        top: f64,
    },
    /// The ball of radius `radius` centred at `(0, 0, −radius)`.
    Ball {
        /// Metres.
        radius: f64,
    },
}

impl Solid for BaseSolid {
    fn bounds(&self) -> Aabb {
        let far = f64::MAX / 4.0;
        match *self {
            Self::Slab { bottom, top } => Aabb::new(Vec3::new(-far, -far, bottom), Vec3::new(far, far, top)),
            Self::Ball { radius } => {
                Aabb::new(Vec3::new(-radius, -radius, -2.0 * radius), Vec3::new(radius, radius, 0.0))
            }
        }
    }

    fn contains(&self, point: Vec3) -> bool {
        match *self {
            Self::Slab { bottom, top } => point.z >= bottom && point.z <= top,
            Self::Ball { radius } => ball_excess(point, radius) <= 0.0,
        }
    }

    fn cover(&self, boxed: &Aabb) -> Cover {
        match *self {
            Self::Slab { bottom, top } => {
                if boxed.lo.z >= bottom && boxed.hi.z <= top {
                    Cover::Inside
                } else if boxed.hi.z < bottom || boxed.lo.z > top {
                    Cover::Outside
                } else {
                    Cover::Mixed
                }
            }
            Self::Ball { radius } => {
                // The excess is convex: its largest over the box is at a corner, its least at the
                // point of the box nearest the centre.
                let farthest = (0..8).map(|i| ball_excess(boxed.corner(i), radius)).fold(f64::MIN, f64::max);
                let near = Vec3::new(
                    0f64.clamp(boxed.lo.x, boxed.hi.x),
                    0f64.clamp(boxed.lo.y, boxed.hi.y),
                    (-radius).clamp(boxed.lo.z, boxed.hi.z),
                );
                // A margin of rounding on each side keeps the answer conservative.
                let slack = 8.0 * f64::EPSILON * (radius * radius + near.dot(near));
                if farthest < -slack {
                    Cover::Inside
                } else if ball_excess(near, radius) > slack {
                    Cover::Outside
                } else {
                    Cover::Mixed
                }
            }
        }
    }
}

/// The pieces' union intersected with a base: the matter a departure takes the base's density
/// from. Conservative as every [`Solid`] is.
pub struct Overlap<'a> {
    /// The pieces' solids, the world's frame.
    pub pieces: Vec<&'a dyn Solid>,
    /// The base's solid.
    pub base: BaseSolid,
}

impl Solid for Overlap<'_> {
    fn bounds(&self) -> Aabb {
        let base = self.base.bounds();
        let union = self.pieces.iter().map(|s| s.bounds()).reduce(|a, b| a.union(&b));
        union.map_or(Aabb::new(Vec3::ZERO, Vec3::ZERO), |u| {
            // The two boxes' intersection; where they miss, a flat box on the nearer face.
            let lo = [u.lo.x.max(base.lo.x), u.lo.y.max(base.lo.y), u.lo.z.max(base.lo.z)];
            let hi = [u.hi.x.min(base.hi.x), u.hi.y.min(base.hi.y), u.hi.z.min(base.hi.z)];
            Aabb::new(Vec3::from_array(lo), Vec3::new(hi[0].max(lo[0]), hi[1].max(lo[1]), hi[2].max(lo[2])))
        })
    }

    fn contains(&self, point: Vec3) -> bool {
        self.base.contains(point) && self.pieces.iter().any(|s| s.contains(point))
    }

    fn cover(&self, boxed: &Aabb) -> Cover {
        let base = self.base.cover(boxed);
        if base == Cover::Outside {
            return Cover::Outside;
        }
        let mut any = false;
        for piece in &self.pieces {
            if !piece.bounds().overlaps(boxed) {
                continue;
            }
            match piece.cover(boxed) {
                Cover::Outside => {}
                Cover::Inside if base == Cover::Inside => return Cover::Inside,
                _ => any = true,
            }
        }
        if any { Cover::Mixed } else { Cover::Outside }
    }
}

/// One leaf of a chunk as the read takes it: the Morton key of its first finest cell, its level,
/// and its matter as a uniform cube (signed mass).
pub type ReadLeaf = (u32, u8, Cube);

/// The leaves of chunk `pieces` (at the materials' `densities`) and of the chunk `overlap` (the
/// overlap of the same pieces with the base, every sample of it at `−base` kg/m³), added cell by
/// cell into one list of signed leaves in Morton order on `grid`: where a leaf of one covers
/// several of the other it is split into their blocks (its mass shared by volume: exact, a
/// power of two). A block whose two masses cancel exactly (a piece of the base's own density
/// under its surface) is left out.
///
/// # Panics
/// When a palette entry has no density in `densities`.
#[must_use]
pub fn merge(
    grid: &WorldGrid,
    pieces: &Chunk,
    overlap: Option<&Chunk>,
    densities: &[f64],
    base: f64,
) -> Vec<ReadLeaf> {
    let sample = grid.sample_volume();
    // (key, level, samples per finest cell, density)
    let items = |chunk: &Chunk, density: &dyn Fn(u8) -> f64| -> Vec<(u32, u8, u8, f64)> {
        chunk.tree.leaves.iter().map(|l| (l.key, l.level, l.count, density(chunk.material(l)))).collect()
    };
    let a = items(pieces, &|m| densities[usize::from(m)]);
    let b = overlap.map(|o| items(o, &|_| -base)).unwrap_or_default();
    let tree_grid = grid.chunk_grid(pieces.key);
    let geometry = Geometry { origin: tree_grid.origin, pitch: grid.pitch };
    let mut out = Vec::with_capacity(a.len() + 2 * b.len());
    let top = CHUNK_DEPTH;
    walk(&geometry, 0, top, Side::of(&a, 0, top), Side::of(&b, 0, top), sample, &mut out);
    out
}

/// Where a chunk's finest cells are.
struct Geometry {
    origin: Vec3,
    pitch: f64,
}

impl Geometry {
    /// The centre of the block of `level` whose first finest cell is `key`.
    fn centre(&self, key: u32, level: u8) -> Vec3 {
        let [x, y, z] = morton::decode(key);
        let half = 0.5 * f64::from(1u32 << level);
        let (o, h) = (self.origin, self.pitch);
        Vec3::new(
            o.x + h * (f64::from(x) + half),
            o.y + h * (f64::from(y) + half),
            o.z + h * (f64::from(z) + half),
        )
    }
}

/// What one of the two lists holds of a block.
#[derive(Clone, Copy)]
enum Side<'a> {
    /// Nothing.
    None,
    /// One leaf that covers the whole block: `(samples per finest cell, density)`.
    Cover(u8, f64),
    /// Several leaves inside it.
    Leaves(&'a [(u32, u8, u8, f64)]),
}

impl<'a> Side<'a> {
    /// The leaves of `list` inside the block of `level` whose first cell is `first`.
    fn of(list: &'a [(u32, u8, u8, f64)], first: u32, level: u8) -> Self {
        let end = u64::from(first) + u64::from(span(level));
        let lo = list.partition_point(|l| l.0 < first);
        let hi = list.partition_point(|l| u64::from(l.0) < end);
        Self::of_slice(&list[lo..hi], level)
    }

    fn of_slice(slice: &'a [(u32, u8, u8, f64)], level: u8) -> Self {
        match slice {
            [] => Self::None,
            [one] if one.1 >= level => Self::Cover(one.2, one.3),
            _ => Self::Leaves(slice),
        }
    }

    /// What it holds of the block of `level` whose first cell is `first`, inside its own block.
    fn down(self, first: u32, level: u8) -> Self {
        match self {
            Self::Leaves(list) => Self::of(list, first, level),
            other => other,
        }
    }

    /// The mass it holds of a block of `level`, kg: `count × 8^level` samples at its density.
    fn mass(self, level: u8, sample: f64) -> f64 {
        match self {
            Self::Cover(count, density) => {
                (u64::from(count) * u64::from(span(level))) as f64 * sample * density
            }
            _ => 0.0,
        }
    }
}

/// The block of `level` whose first cell is `first`, from both lists, pushed to `out`.
fn walk(
    geometry: &Geometry,
    first: u32,
    level: u8,
    a: Side<'_>,
    b: Side<'_>,
    sample: f64,
    out: &mut Vec<ReadLeaf>,
) {
    match (a, b) {
        (Side::None, Side::None) => {}
        (Side::Leaves(_), _) | (_, Side::Leaves(_)) => {
            let step = span(level - 1);
            for octant in 0..8u32 {
                let lo = first + octant * step;
                walk(geometry, lo, level - 1, a.down(lo, level - 1), b.down(lo, level - 1), sample, out);
            }
        }
        _ => {
            let mass = match b {
                Side::None => a.mass(level, sample),
                _ => a.mass(level, sample) + b.mass(level, sample),
            };
            if mass != 0.0 {
                out.push((
                    first,
                    level,
                    (geometry.centre(first, level), mass, geometry.pitch * f64::from(1u32 << level)),
                ));
            }
        }
    }
}

/// A base's closed form, as a source of the flight integral.
pub enum BaseSource {
    /// The plane: S1's slab.
    Plane(Slab),
    /// The planet: its layers as one point mass at its centre.
    Sphere(Planet),
}

impl Source for BaseSource {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, time: f64) -> [f64; 2] {
        match self {
            Self::Plane(slab) => slab.difference(tower, heights, lower, upper, time),
            Self::Sphere(planet) => planet.difference(tower, heights, lower, upper, time),
        }
    }
}

/// A layered planet seen from outside: a point mass of its layers' sum at its centre (Newton's
/// shell theorem), read without cancellation ([`Mass`]'s form). Not for a point inside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Planet {
    /// Its centre, metres, the world's frame.
    pub centre: Vec3,
    /// Its mass, kg.
    pub mass: f64,
}

impl Source for Planet {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, time: f64) -> [f64; 2] {
        let centre = self.centre;
        Mass::point(self.mass, move |_| centre).difference(tower, heights, lower, upper, time)
    }
}

#[cfg(test)]
mod tests;
