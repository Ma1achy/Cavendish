//! Bodies of several materials: solids laid on one grid in order, each adding matter of its
//! material, painting its material into the matter already there, or erasing it (S19,
//! `docs/slices/S19.md` §2).
//!
//! **What a sample point holds.** The grid's sample points are the voxeliser's ([`Grid::sample`]).
//! At each, the layers are applied in order, starting from nothing: an [`Mode::Add`] layer that
//! holds the point puts its material there; a [`Mode::Paint`] layer puts its material there only
//! if there is matter already; an [`Mode::Erase`] layer takes the matter away. What is left is
//! the point's material, or none.
//!
//! **What a cell weighs.** A finest cell's mass is `Σ_m n_m · u_m`: `n_m` its sample points of
//! material `m`, `u_m = ρ_m · v` the mass of one sample point of it (`v = (h/k)³`). So a cell two
//! materials share keeps both masses, as the exact-skin note asks of a cell crossed by two
//! materials (`docs/research/exact-skin-and-partial-fill.md` §5: "moments add"); the leaf's
//! `material` is then the one with most points (the lowest index on a tie), for colour only, and
//! its mass is kept apart in [`VoxTree::mixed`]. A cell of one material weighs
//! `count · u_m` as any leaf does.
//!
//! **The oracle.** [`raster_layers`] asks every sample point of every cell; the top-down walk asks
//! [`Region::cover`] about whole blocks first. Covers are conservative for the very sample points,
//! so the two agree cell for cell, bit for bit (`tests::the_walk_equals_the_raster`).
//!
//! Frames and units: the body's frame, metres; densities kg/m³; masses kg.

use super::morton::{self, span};
use super::shape::{Cover, Shape};
use super::tree::{Leaf, MixedCell, VoxTree};
use super::{Aabb, Budget, Grid, MAX_RASTER_CELLS, VoxelError};
use crate::Vec3;

/// The most layers one body may have.
pub const MAX_LAYERS: usize = 256;
/// The most materials one body may use (a leaf's material is a byte).
pub const MAX_BODY_MATERIALS: usize = 255;
/// The most points one brush stroke may have.
pub const MAX_STROKE_POINTS: usize = 1024;

/// What can be laid as a layer: a region of space with a conservative cover. Every [`Shape`] is
/// one through [`Whole`]; a brush [`Stroke`] is one with no closed-form volume.
pub trait Region: Sync + Send {
    /// A box holding the whole region, metres, the body's frame.
    fn bounds(&self) -> Aabb;
    /// Whether `point` (metres, the body's frame) is inside, its surface counting as inside.
    fn contains(&self, point: Vec3) -> bool;
    /// What the region says about every point of `boxed`: as [`Shape::cover`]'s contract, never
    /// `Inside` or `Outside` unless true of every point of the box.
    fn cover(&self, boxed: &Aabb) -> Cover;
}

/// A [`Shape`] as a [`Region`].
pub struct Whole<'a>(pub &'a dyn Shape);

impl Region for Whole<'_> {
    fn bounds(&self) -> Aabb {
        self.0.bounds()
    }
    fn contains(&self, point: Vec3) -> bool {
        self.0.contains(point)
    }
    fn cover(&self, boxed: &Aabb) -> Cover {
        self.0.cover(boxed)
    }
}

/// A region moved by `by` metres: a part laid away from the body's centre.
pub struct Moved<R: Region> {
    /// The region, about its own origin.
    pub inner: R,
    /// Where its origin goes, metres, the body's frame.
    pub by: Vec3,
}

impl<R: Region> Region for Moved<R> {
    fn bounds(&self) -> Aabb {
        let b = self.inner.bounds();
        Aabb::new(b.lo + self.by, b.hi + self.by)
    }
    fn contains(&self, point: Vec3) -> bool {
        self.inner.contains(point - self.by)
    }
    fn cover(&self, boxed: &Aabb) -> Cover {
        // Rounding is monotone, so every sample point `p` of the box, shifted as `contains` shifts
        // it (`p − by`), lies in the box shifted corner by corner: the inner cover holds of them.
        self.inner.cover(&Aabb::new(boxed.lo - self.by, boxed.hi - self.by))
    }
}

/// The largest absolute coordinate of a box.
fn reach(b: &Aabb) -> f64 {
    [b.lo.x, b.lo.y, b.lo.z, b.hi.x, b.hi.y, b.hi.z].iter().fold(0f64, |m, v| m.max(v.abs()))
}

/// A brush stroke: the union of balls of `radius` metres centred on every point of the segments
/// joining `points` in order (a capsule chain). One point is one ball.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    /// The path, metres, the body's frame: 1 to [`MAX_STROKE_POINTS`] points.
    pub points: Vec<Vec3>,
    /// Metres; positive.
    pub radius: f64,
}

impl Stroke {
    /// The segments of the path; a path of one point is one segment of no length.
    fn segments(&self) -> impl Iterator<Item = (Vec3, Vec3)> + '_ {
        let n = self.points.len();
        (0..n.saturating_sub(1).max(1)).map(move |i| (self.points[i], self.points[(i + 1).min(n - 1)]))
    }
}

/// The distance from `p` to the segment `a`–`b`, metres.
fn to_segment(p: Vec3, a: Vec3, b: Vec3) -> f64 {
    let ab = b - a;
    let along = ab.dot(ab);
    let t = if along > 0.0 { ((p - a).dot(ab) / along).clamp(0.0, 1.0) } else { 0.0 };
    (p - (a + ab * t)).length()
}

impl Region for Stroke {
    fn bounds(&self) -> Aabb {
        let r = Vec3::new(self.radius, self.radius, self.radius);
        let first = Aabb::new(self.points[0] - r, self.points[0] + r);
        self.points.iter().fold(first, |b, &p| b.union(&Aabb::new(p - r, p + r)))
    }

    fn contains(&self, point: Vec3) -> bool {
        self.segments().any(|(a, b)| to_segment(point, a, b) <= self.radius)
    }

    fn cover(&self, b: &Aabb) -> Cover {
        let slack = 1e-12 * (self.radius + reach(b));
        let corners: Vec<Vec3> = (0..8).map(|i| b.corner(i)).collect();
        // Each capsule is convex: a box whose eight corners it holds is inside it.
        if self.segments().any(|(p, q)| corners.iter().all(|&c| to_segment(c, p, q) <= self.radius - slack)) {
            return Cover::Inside;
        }
        // Every point of the box is within half its diagonal of its centre.
        let half = 0.5 * (b.hi - b.lo).length();
        let centre = b.centre();
        if self.segments().all(|(p, q)| to_segment(centre, p, q) - half > self.radius + slack) {
            return Cover::Outside;
        }
        Cover::Mixed
    }
}

/// What a layer does where it lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Puts matter of its material at every point it holds.
    Add,
    /// Turns the matter at the points it holds to its material; adds none.
    Paint,
    /// Takes away the matter at the points it holds.
    Erase,
}

/// One layer of a body: a region, a material (an index into the body's palette) and a mode.
pub struct Layer<'a> {
    /// Where it lies, the body's frame.
    pub region: &'a dyn Region,
    /// Its material: an index into the densities given with the layers.
    pub material: u8,
    /// What it does.
    pub mode: Mode,
}

/// The material at `point` after every layer, or none.
fn material_at(layers: &[Layer], point: Vec3) -> Option<u8> {
    let mut held = None;
    for layer in layers {
        if layer.region.contains(point) {
            held = match layer.mode {
                Mode::Add => Some(layer.material),
                Mode::Paint => held.map(|_| layer.material),
                Mode::Erase => None,
            };
        }
    }
    held
}

/// What every sample point of a box holds, when the covers decide it: `Some(None)` empty,
/// `Some(Some(m))` all material `m`, `None` undecided.
fn block_material(layers: &[Layer], boxed: &Aabb) -> Option<Option<u8>> {
    let mut held: Option<Option<u8>> = Some(None);
    for layer in layers {
        let cover = layer.region.cover(boxed);
        let wanted = match layer.mode {
            Mode::Add => Some(Some(layer.material)),
            Mode::Paint => held.map(|h| h.map(|_| layer.material)),
            Mode::Erase => Some(None),
        };
        held = match cover {
            Cover::Outside => held,
            Cover::Inside => match layer.mode {
                Mode::Paint => match held {
                    // Painting over an undecided block: its matter (wherever it is) turns to the
                    // layer's material, the empty points stay empty; still undecided.
                    None => None,
                    known => known.map(|h| h.map(|_| layer.material)),
                },
                _ => wanted,
            },
            // Some points change and some do not: decided only if both give the same.
            Cover::Mixed => {
                if held.is_some() && held == wanted {
                    held
                } else {
                    None
                }
            }
        };
    }
    held
}

/// The sample points of `cell` of each material, as a list of (material, count), the lowest
/// material first.
fn tally(layers: &[Layer], grid: &Grid, cell: [u32; 3]) -> Vec<(u8, u8)> {
    let k = u32::from(grid.supersample);
    let mut counts: Vec<(u8, u8)> = Vec::new();
    for c in 0..k {
        for b in 0..k {
            for a in 0..k {
                if let Some(m) = material_at(layers, grid.sample(cell, [a, b, c])) {
                    match counts.iter_mut().find(|(n, _)| *n == m) {
                        Some((_, count)) => *count += 1,
                        None => counts.push((m, 1)),
                    }
                }
            }
        }
    }
    counts.sort_unstable();
    counts
}

/// The leaf of a cell from its tally, and its mass when it is shared by more than one material.
fn cell_leaf(key: u32, counts: &[(u8, u8)], units: &[f64]) -> Option<(Leaf, Option<MixedCell>)> {
    let total: u8 = counts.iter().map(|(_, n)| n).sum();
    if total == 0 {
        return None;
    }
    // The material with most points; the lowest on a tie (the list is sorted by material).
    let dominant = counts.iter().fold(counts[0], |best, &c| if c.1 > best.1 { c } else { best }).0;
    let leaf = Leaf { key, level: 0, count: total, material: dominant };
    let mixed = (counts.len() > 1).then(|| MixedCell {
        key,
        mass: counts.iter().map(|&(m, n)| f64::from(n) * units[usize::from(m)]).sum(),
    });
    Some((leaf, mixed))
}

/// The mass of one sample point of each material: `ρ_m · (h/k)³`, kilograms.
#[must_use]
pub fn units_of(grid: &Grid, densities: &[f64]) -> Vec<f64> {
    let side = grid.pitch / f64::from(grid.supersample);
    let v = side * side * side;
    densities.iter().map(|rho| rho * v).collect()
}

fn check(layers: &[Layer], densities: &[f64]) -> Result<(), VoxelError> {
    if layers.is_empty() || layers.len() > MAX_LAYERS {
        return Err(VoxelError(format!("{} layers; a body has 1 to {MAX_LAYERS}", layers.len())));
    }
    if densities.is_empty() || densities.len() > MAX_BODY_MATERIALS {
        return Err(VoxelError(format!(
            "{} materials; a body has 1 to {MAX_BODY_MATERIALS}",
            densities.len()
        )));
    }
    if let Some(rho) = densities.iter().find(|rho| !(rho.is_finite() && **rho >= 0.0)) {
        return Err(VoxelError(format!("a density of {rho} kg/m³: each is 0 or more")));
    }
    if let Some(layer) = layers.iter().find(|l| usize::from(l.material) >= densities.len()) {
        return Err(VoxelError(format!("a layer names material {} of {}", layer.material, densities.len())));
    }
    Ok(())
}

/// The layers on `grid` as a tree: each sample point's material by the layers in order, each
/// material's points weighing `densities[m] × (h/k)³` kilograms.
///
/// # Errors
/// As [`super::voxelise`]: the work, leaf and byte caps of `budget`; when no sample point holds
/// matter; when there are no layers or more than [`MAX_LAYERS`], or a layer's material has no
/// density.
pub fn voxelise_layers(
    layers: &[Layer],
    grid: &Grid,
    densities: &[f64],
    budget: Budget,
) -> Result<VoxTree, VoxelError> {
    check(layers, densities)?;
    let units = units_of(grid, densities);
    let mut walk =
        Walk { layers, grid, units: &units, leaves: Vec::new(), mixed: Vec::new(), work: 0, budget };
    walk.visit(0, grid.depth)?;
    VoxTree::build_layered(*grid, walk.leaves, units.clone(), walk.mixed, budget)
}

/// The raster oracle of [`voxelise_layers`]: every finest cell of the used region, asked point by
/// point. For tests and small grids.
///
/// # Errors
/// When the used region has more than [`MAX_RASTER_CELLS`] cells, or the layers are refused.
pub fn raster_layers(
    layers: &[Layer],
    grid: &Grid,
    densities: &[f64],
) -> Result<(Vec<Leaf>, Vec<MixedCell>), VoxelError> {
    check(layers, densities)?;
    let total = grid.cells.iter().map(|&c| u64::from(c)).product::<u64>();
    if total > MAX_RASTER_CELLS {
        return Err(VoxelError(format!(
            "{total} cells is more than the raster oracle holds, {MAX_RASTER_CELLS}"
        )));
    }
    let units = units_of(grid, densities);
    let mut leaves = Vec::new();
    let mut mixed = Vec::new();
    for z in 0..grid.cells[2] {
        for y in 0..grid.cells[1] {
            for x in 0..grid.cells[0] {
                let key = morton::encode([x, y, z]);
                if let Some((leaf, shared)) = cell_leaf(key, &tally(layers, grid, [x, y, z]), &units) {
                    leaves.push(leaf);
                    mixed.extend(shared);
                }
            }
        }
    }
    leaves.sort_unstable_by_key(|l| l.key);
    mixed.sort_unstable_by_key(|m| m.key);
    Ok((leaves, mixed))
}

struct Walk<'a> {
    layers: &'a [Layer<'a>],
    grid: &'a Grid,
    units: &'a [f64],
    leaves: Vec<Leaf>,
    mixed: Vec<MixedCell>,
    work: u64,
    budget: Budget,
}

impl Walk<'_> {
    fn spend(&mut self, work: u64) -> Result<(), VoxelError> {
        self.work += work;
        if self.work > self.budget.work {
            return Err(VoxelError(format!(
                "voxelising would take more than {} box and sample tests: use a coarser pitch, less \
                 supersampling or fewer parts",
                self.budget.work
            )));
        }
        Ok(())
    }

    fn emit(&mut self, leaf: Leaf) -> Result<(), VoxelError> {
        if self.leaves.len() >= self.budget.leaves {
            return Err(VoxelError(format!(
                "the tree would have more than {} leaves: use a coarser pitch",
                self.budget.leaves
            )));
        }
        let bytes = (self.leaves.len() + 1) * size_of::<Leaf>() + self.mixed.len() * size_of::<MixedCell>();
        if bytes > self.budget.bytes {
            return Err(VoxelError(format!(
                "the tree's leaves alone would pass the {} bytes allowed it: use a coarser pitch",
                self.budget.bytes
            )));
        }
        self.leaves.push(leaf);
        Ok(())
    }

    fn visit(&mut self, key: u32, level: u8) -> Result<(), VoxelError> {
        let corner = morton::decode(key);
        if !self.grid.touches_used(corner) {
            return Ok(());
        }
        let side = 1u32 << level;
        self.spend(self.layers.len() as u64)?;
        let full = self.grid.samples_per_cell() as u8;
        match block_material(self.layers, &self.grid.sample_box(corner, side)) {
            Some(None) => Ok(()),
            Some(Some(material)) => self.emit(Leaf { key, level, count: full, material }),
            None if level == 0 => {
                self.spend(u64::from(self.grid.samples_per_cell()) * self.layers.len() as u64)?;
                if let Some((leaf, shared)) =
                    cell_leaf(key, &tally(self.layers, self.grid, corner), self.units)
                {
                    self.emit(leaf)?;
                    self.mixed.extend(shared);
                }
                Ok(())
            }
            None => {
                let before = self.leaves.len();
                let mixed_before = self.mixed.len();
                for octant in 0..8 {
                    self.visit(morton::child(key, level, octant), level - 1)?;
                }
                // The lossless merge, as the uniform voxeliser's; never of a shared cell, whose
                // mass is not its count's.
                let new = &self.leaves[before..];
                if new.len() == 8
                    && self.mixed.len() == mixed_before
                    && new.iter().all(|l| {
                        l.level == level - 1 && l.count == new[0].count && l.material == new[0].material
                    })
                {
                    let merged = Leaf { key, level, count: new[0].count, material: new[0].material };
                    self.leaves.truncate(before);
                    self.leaves.push(merged);
                }
                Ok(())
            }
        }
    }
}

/// The leaves split back to finest cells (with the same shared cells): the same body, unmerged.
#[must_use]
pub fn unmerged_leaves(leaves: &[Leaf]) -> Vec<Leaf> {
    let mut out = Vec::new();
    for leaf in leaves {
        for i in 0..span(leaf.level) {
            out.push(Leaf { key: leaf.key + i, level: 0, ..*leaf });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::shape::{Cuboid, Sphere};

    fn grid(lo: [f64; 3], hi: [f64; 3], pitch: f64, k: u8) -> Grid {
        Grid::around(Aabb::new(Vec3::from_array(lo), Vec3::from_array(hi)), pitch, k).unwrap()
    }

    #[test]
    fn the_walk_equals_the_raster() {
        // A steel ball, a lead box added across it, a copper stroke painted over both, and a ball
        // erased: the tree expanded equals the raster cell for cell, bit for bit, shared cells
        // and their masses included. Control: the same layers in another order are another body.
        let ball = Sphere { centre: Vec3::ZERO, radius: 0.3 };
        let block = Cuboid { centre: Vec3::new(0.25, 0.0, 0.0), half: Vec3::new(0.12, 0.2, 0.17) };
        let stroke =
            Stroke { points: vec![Vec3::new(-0.3, -0.2, 0.1), Vec3::new(0.3, 0.1, 0.15)], radius: 0.07 };
        let hole = Moved { inner: Whole(&ball), by: Vec3::new(0.0, 0.0, -0.32) };
        let (ball_r, block_r) = (Whole(&ball), Whole(&block));
        let layers = [
            Layer { region: &ball_r, material: 0, mode: Mode::Add },
            Layer { region: &block_r, material: 1, mode: Mode::Add },
            Layer { region: &stroke, material: 2, mode: Mode::Paint },
            Layer { region: &hole, material: 0, mode: Mode::Erase },
        ];
        let densities = [7850.0, 11340.0, 8960.0];
        let g = grid([-0.31, -0.31, -0.31], [0.38, 0.31, 0.31], 0.02, 3);
        let tree = voxelise_layers(&layers, &g, &densities, Budget::DEFAULT).unwrap();
        let (leaves, mixed) = raster_layers(&layers, &g, &densities).unwrap();
        let mut expanded = unmerged_leaves(&tree.leaves);
        expanded.sort_unstable_by_key(|l| l.key);
        assert_eq!(expanded, leaves, "cell for cell");
        assert_eq!(tree.mixed, mixed, "the shared cells and their masses");
        assert!(mixed.len() > 50, "the body has cells two materials share: {}", mixed.len());
        assert!(tree.leaves.len() < leaves.len(), "and merged leaves");
        let painted = tree.leaves.iter().filter(|l| l.material == 2).count();
        assert!(painted > 0, "the stroke painted");
        // Control: the stroke first paints nothing (nothing is there yet), so the body differs.
        let reordered = [
            Layer { region: &stroke, material: 2, mode: Mode::Paint },
            Layer { region: &ball_r, material: 0, mode: Mode::Add },
            Layer { region: &block_r, material: 1, mode: Mode::Add },
            Layer { region: &hole, material: 0, mode: Mode::Erase },
        ];
        let other = voxelise_layers(&reordered, &g, &densities, Budget::DEFAULT).unwrap();
        assert!(other.leaves.iter().all(|l| l.material != 2));
        assert_ne!(other.body().mass, tree.body().mass);
    }

    #[test]
    fn a_shared_cell_keeps_both_masses() {
        // S19 M4: a cell half steel, half lead weighs n_s·u_s + n_l·u_l, to rounding. Control: the
        // world's majority rule (every point at the dominant material's density) differs.
        let steel = Cuboid { centre: Vec3::new(-0.5, 0.0, 0.0), half: Vec3::new(0.5, 0.5, 0.5) };
        let lead = Cuboid { centre: Vec3::new(0.5, 0.0, 0.0), half: Vec3::new(0.5, 0.5, 0.5) };
        let (s, l) = (Whole(&steel), Whole(&lead));
        let layers = [
            Layer { region: &s, material: 0, mode: Mode::Add },
            Layer { region: &l, material: 1, mode: Mode::Add },
        ];
        let densities = [7850.0, 11340.0];
        // One cell of pitch 0.5 straddles x = 0 (the grid is centred on the box, so its cells do).
        let g = grid([-0.25, -0.25, -0.25], [0.25, 0.25, 0.25], 0.5, 4);
        let tree = voxelise_layers(&layers, &g, &densities, Budget::DEFAULT).unwrap();
        assert_eq!(tree.leaves.len(), 1);
        let units = units_of(&g, &densities);
        let expected = 32.0 * units[0] + 32.0 * units[1];
        let mass = tree.leaf_mass(&tree.leaves[0]);
        assert!((mass - expected).abs() <= 1e-15 * expected, "{mass} against {expected}");
        assert!((mass - 0.125 * 0.5 * (7850.0 + 11340.0)).abs() < 1e-12 * mass, "ρ̄·V");
        let majority = 64.0 * units[usize::from(tree.leaves[0].material)];
        assert!((majority - mass).abs() > 1e-3 * mass, "control: {majority} against {mass}");
    }

    #[test]
    fn a_body_of_one_material_weighs_its_density_times_its_samples() {
        // A box whose faces lie on cell faces: every sample inside, mass ρ·a·b·c to rounding, and
        // the tree is one merged block where it can be. Control: an erased corner weighs less by
        // the erased samples exactly.
        let block = Cuboid { centre: Vec3::ZERO, half: Vec3::new(0.2, 0.1, 0.05) };
        let r = Whole(&block);
        let layers = [Layer { region: &r, material: 0, mode: Mode::Add }];
        let g = grid([-0.2, -0.1, -0.05], [0.2, 0.1, 0.05], 0.025, 2);
        let tree = voxelise_layers(&layers, &g, &[2700.0], Budget::DEFAULT).unwrap();
        let mass = tree.body().mass;
        assert!((mass - 2700.0 * 0.4 * 0.2 * 0.1).abs() < 1e-12 * mass, "{mass}");
        assert!(tree.mixed.is_empty());
        let corner = Cuboid { centre: Vec3::new(0.2, 0.1, 0.05), half: Vec3::new(0.05, 0.05, 0.05) };
        let c = Whole(&corner);
        let erased = [
            Layer { region: &r, material: 0, mode: Mode::Add },
            Layer { region: &c, material: 0, mode: Mode::Erase },
        ];
        let less = voxelise_layers(&erased, &g, &[2700.0], Budget::DEFAULT).unwrap().body().mass;
        assert!((mass - less - 2700.0 * 0.05 * 0.05 * 0.05).abs() < 1e-12 * mass, "{mass} - {less}");
    }

    #[test]
    fn refusals_say_why() {
        let ball = Sphere { centre: Vec3::ZERO, radius: 0.3 };
        let r = Whole(&ball);
        let g = grid([-0.3; 3], [0.3; 3], 0.05, 2);
        let layers = [Layer { region: &r, material: 1, mode: Mode::Add }];
        let why = voxelise_layers(&layers, &g, &[1000.0], Budget::DEFAULT).unwrap_err().0;
        assert!(why.contains("material 1 of 1"), "{why}");
        assert!(voxelise_layers(&[], &g, &[1000.0], Budget::DEFAULT).is_err());
        let erased = [
            Layer { region: &r, material: 0, mode: Mode::Add },
            Layer { region: &r, material: 0, mode: Mode::Erase },
        ];
        assert!(voxelise_layers(&erased, &g, &[1000.0], Budget::DEFAULT).is_err(), "nothing left");
    }
}
