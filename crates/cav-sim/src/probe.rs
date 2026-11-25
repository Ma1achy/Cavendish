//! The Probe tool's reading of a point (S19, `docs/slices/S19.md` §2): the gravity of the scene's
//! matter there, the finest voxel cell under it, that cell's share of each tower's phase now, and
//! the distance to each tower. A query: nothing in it is an edit.
//!
//! **The field** `g` is the attraction at the point, m/s², the world's frame, of
//! - every body that exists now: a point mass (or a sphere of its radius, from inside as from out)
//!   in closed form; a voxel body leaf by leaf, each leaf a uniform cube — within [`NEAR_SIDES`]
//!   of its side by the central difference of the exact cube potential
//!   ([`cav_core::voxel::kernel::cube_potential`]), farther as a point of its mass (what that
//!   leaves out is the cube's l = 4 term, under 1e-4 of the leaf's share at [`NEAR_SIDES`]
//!   sides). Every voxel body is counted, whatever the view's byte budget kept: a tree the caller
//!   does not give is made here;
//! - a slab floor, in closed form (`2πGρ` times the matter above less the matter below).
//!
//! What it does not count it names in [`Probe::not_counted`]: a half-space floor (its uniform
//! field is unbounded; the towers read none of it), the world volume (its field at a point comes
//! with S5's octree), a body that cannot be voxelised. The room is not matter.
//!
//! **The cell** is the finest cell (side = the body's pitch) holding the point, however the tree
//! merged it into a larger leaf: its mass is its own (a merged leaf's cells are alike; a cell two
//! materials share weighs what [`VoxTree::mixed`] says), and its **share of a tower's phase** is
//! that cell read alone through the tower's flight as a one-cell body
//! ([`crate::voxel_view::leaf_phase`], as the view's Phase colouring reads a leaf). The leaf it
//! lies in is reported beside it ([`CellProbe::block`]).

use std::sync::Arc;

use cav_core::Vec3;
use cav_core::voxel::kernel::cube_potential;
use cav_core::voxel::{Leaf, VoxTree, morton};

use crate::{FloorKind, SceneSpec};

/// The gravitational constant, m³ kg⁻¹ s⁻².
const G: f64 = cav_core::constants::BIG_G;
/// Within this many of its sides a leaf's field is the exact cube's; beyond, a point's.
pub const NEAR_SIDES: f64 = 8.0;

/// What the probe reads at a point.
#[derive(Clone, Debug, PartialEq)]
pub struct Probe {
    /// The point, metres, the world's frame.
    pub point: [f64; 3],
    /// The gravity there of what is counted (the bodies and a slab floor), m/s², the world's
    /// frame (x east, y north, z up).
    pub g: [f64; 3],
    /// The floor's part of `g`, m/s², when a slab floor is counted.
    pub floor: Option<[f64; 3]>,
    /// The finest voxel cell the point is in or on, if any.
    pub cell: Option<CellProbe>,
    /// Each tower, in the scene's order.
    pub towers: Vec<TowerProbe>,
    /// What has matter but is not in `g`, a phrase each (empty when everything is counted).
    pub not_counted: Vec<String>,
}

/// The finest voxel cell under the probe.
#[derive(Clone, Debug, PartialEq)]
pub struct CellProbe {
    /// The body, from 0.
    pub body: usize,
    /// The cell as a leaf of level 0: its Morton key in the body's grid, its count and material.
    pub leaf: Leaf,
    /// Its material's label, or none for a uniform body.
    pub material: Option<String>,
    /// Its mass, kilograms.
    pub mass: f64,
    /// Its side, metres: the body's pitch.
    pub side: f64,
    /// The tree's leaf it lies in: its index and its side, metres (the cell's own side when the
    /// leaf is the cell).
    pub block: (usize, f64),
}

/// The probe's reading at one tower.
#[derive(Clone, Debug, PartialEq)]
pub struct TowerProbe {
    /// The tower's label.
    pub label: String,
    /// From the point to the tower's axis, horizontally, metres.
    pub distance: f64,
    /// The cell's share of the tower's ΔΦ now, radians (none without a cell, or when the read
    /// was refused: then `why` says so).
    pub phase: Option<f64>,
    /// Why there is no phase, when there is a cell.
    pub why: Option<String>,
}

/// The field at `d` metres from a uniform cube's centre, m/s²: `−G ∇V`.
fn cube_field(mass: f64, side: f64, d: Vec3) -> Vec3 {
    let r = d.length();
    if r > NEAR_SIDES * side {
        return d * (-G * mass / (r * r * r));
    }
    // A step small beside the side (its error ~ step²) and large beside rounding (~ ε/step).
    let h = 1e-5 * side;
    let mut out = [0.0; 3];
    for (axis, slot) in out.iter_mut().enumerate() {
        let mut e = [0.0; 3];
        e[axis] = h;
        let e = Vec3::from_array(e);
        let up = cube_potential(mass, side, d + e);
        let down = cube_potential(mass, side, d - e);
        *slot = -G * (up - down) / (2.0 * h);
    }
    Vec3::from_array(out)
}

/// The finest cell of `leaf` that holds `p` (metres, the body's frame, inside or on the leaf's
/// box): a leaf of level 0 with the leaf's count and material.
fn finest_cell(tree: &VoxTree, leaf: &Leaf, p: Vec3) -> Leaf {
    if leaf.level == 0 {
        return *leaf;
    }
    let corner = morton::decode(leaf.key);
    let last = (1u32 << leaf.level) - 1;
    let o = tree.grid.origin;
    let index = |x: f64, origin: f64, start: u32| {
        let i = ((x - origin) / tree.grid.pitch).floor() - f64::from(start);
        // On the leaf's far face the point belongs to its last cell.
        start + (i.max(0.0) as u32).min(last)
    };
    let cell = [index(p.x, o.x, corner[0]), index(p.y, o.y, corner[1]), index(p.z, o.z, corner[2])];
    Leaf { key: morton::encode(cell), level: 0, count: leaf.count, material: leaf.material }
}

/// The field of the floor at height `z`, m/s² upwards, if it is a slab (a half-space's is
/// unbounded and is not counted).
fn floor_field(scene: &SceneSpec, z: f64) -> Option<f64> {
    let floor = scene.floor.as_ref()?;
    let FloorKind::Slab { thickness } = floor.kind else { return None };
    let (top, bottom) = (floor.height, floor.height - thickness);
    let z = z.clamp(bottom, top);
    // The matter below the point pulls it down, the matter above pulls it up.
    Some(-2.0 * std::f64::consts::PI * G * floor.density * ((z - bottom) - (top - z)))
}

/// The probe's reading at `point` (metres, the world's frame) of `scene` at `time`, with
/// `trees[n]` body `n`'s tree when the caller has it made (None for a body not made of voxels,
/// or one the caller left out: that one is made here). The cell is the finest cell of the first
/// body whose leaf's box holds the point, or is within a millionth of its side of it (a point
/// picked on a face).
#[must_use]
pub fn probe(scene: &SceneSpec, trees: &[Option<Arc<VoxTree>>], time: f64, point: [f64; 3]) -> Probe {
    let p = Vec3::from_array(point);
    let gravity = scene.array.instrument.gravity;
    let mut g = Vec3::ZERO;
    let mut cell: Option<(CellProbe, Arc<VoxTree>)> = None;
    let mut not_counted = Vec::new();
    for (n, body) in scene.bodies.iter().enumerate() {
        let (from, until) = body.exists();
        if time < from || time >= until {
            continue;
        }
        let at = Vec3::from_array(body.motion.position(time, gravity));
        let Some(shape) = &body.shape else {
            let d = p - at;
            let r = d.length();
            if r > 0.0 {
                // A uniform sphere: a point outside itself; inside, the mass within r.
                let inside = if r < body.radius { (r / body.radius).powi(3) } else { 1.0 };
                g = g + d * (-G * body.mass * inside / (r * r * r));
            }
            continue;
        };
        let tree = match trees.get(n).and_then(Option::as_ref) {
            Some(tree) => Arc::clone(tree),
            None => match shape.tree(body.mass, &scene.materials) {
                Ok(tree) => Arc::new(tree),
                Err(why) => {
                    not_counted.push(format!("`{}` ({why})", body.label));
                    continue;
                }
            },
        };
        for (l, leaf) in tree.leaves.iter().enumerate() {
            let centre = at + tree.block_centre(leaf.key, leaf.level);
            let side = tree.side(leaf.level);
            g = g + cube_field(tree.leaf_mass(leaf), side, p - centre);
            let off = p - centre;
            let reach = 0.5 * side * (1.0 + 1e-6);
            if cell.is_none() && off.x.abs() <= reach && off.y.abs() <= reach && off.z.abs() <= reach {
                let finest = finest_cell(&tree, leaf, p - at);
                let palette = shape.palette();
                let found = CellProbe {
                    body: n,
                    leaf: finest,
                    material: palette.get(usize::from(leaf.material)).map(|s| (*s).to_owned()),
                    mass: tree.leaf_mass(&finest),
                    side: tree.grid.pitch,
                    block: (l, side),
                };
                cell = Some((found, Arc::clone(&tree)));
            }
        }
    }
    let floor = floor_field(scene, point[2]).map(|gz| [0.0, 0.0, gz]);
    if let Some(f) = floor {
        g = g + Vec3::from_array(f);
    }
    if scene.floor.as_ref().is_some_and(|f| f.kind == FloorKind::HalfSpace) {
        not_counted
            .push("the floor (a half-space: its uniform field is unbounded, and no tower reads it)".into());
    }
    if scene.world.is_some() {
        not_counted.push("the world (its field at a point comes with S5)".into());
    }
    let towers = scene
        .array
        .towers
        .iter()
        .enumerate()
        .map(|(t, tower)| {
            let distance = (point[0] - tower.position[0]).hypot(point[1] - tower.position[1]);
            let read = cell.as_ref().map(|(c, tree)| {
                let at = Vec3::from_array(scene.bodies[c.body].motion.position(time, gravity));
                crate::voxel_view::leaf_phase(scene, tree, &c.leaf, at, t, time)
            });
            let (phase, why) = match read {
                Some(Ok(phase)) => (Some(phase), None),
                Some(Err(why)) => (None, Some(why)),
                None => (None, None),
            };
            TowerProbe { label: tower.label.clone(), distance, phase, why }
        })
        .collect();
    Probe { point, g: g.to_array(), floor, cell: cell.map(|(c, _)| c), towers, not_counted }
}

impl Probe {
    /// What `g` is of, in words: "the bodies", "the bodies and the floor".
    #[must_use]
    pub fn counted(&self) -> &'static str {
        if self.floor.is_some() { "the bodies and the floor" } else { "the bodies" }
    }

    /// The reading in words, a line a fact, as the console prints it.
    #[must_use]
    pub fn words(&self, scene: &SceneSpec) -> String {
        let [x, y, z] = self.point;
        let g = Vec3::from_array(self.g);
        let mut out = format!(
            "at ({x:.4}, {y:.4}, {z:.4}) m\ng from {}: {:.6e} m/s² ({:.4e}, {:.4e}, {:.4e})\n",
            self.counted(),
            g.length(),
            self.g[0],
            self.g[1],
            self.g[2]
        );
        if !self.not_counted.is_empty() {
            out.push_str(&format!("not counted: {}\n", self.not_counted.join("; ")));
        }
        match &self.cell {
            Some(c) => {
                let block = if c.block.1 > c.side {
                    let n = (c.block.1 / c.side).round();
                    format!(", in a block of {n}³ alike")
                } else {
                    String::new()
                };
                out.push_str(&format!(
                    "cell of `{}`: {}, {:.6e} kg, {} m a side{block}\n",
                    scene.bodies[c.body].label,
                    c.material.as_deref().unwrap_or("uniform"),
                    c.mass,
                    c.side
                ));
            }
            None => out.push_str("no body here\n"),
        }
        for t in &self.towers {
            let share = match (&t.phase, &t.why) {
                (Some(phase), _) => format!(", the cell's share of ΔΦ {phase:.6e} rad"),
                (None, Some(why)) => format!(", the cell's share not read: {why}"),
                (None, None) => String::new(),
            };
            out.push_str(&format!("{}: {:.4} m from its axis{share}\n", t.label, t.distance));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BodySpec, FloorSpec, Motion, ShapeKind, ShapeSpec, TowerSpec};

    /// One tower at the origin and a voxel ball of 100 kg, r = 0.5 m, at `at`.
    fn ball_at(at: [f64; 3]) -> (SceneSpec, Vec<Option<Arc<VoxTree>>>) {
        let mut scene = crate::dev::void();
        scene.array.towers = vec![TowerSpec::at("tower 1", [0.0, 0.0, 0.0])];
        let shape = ShapeSpec::new(ShapeKind::Sphere { radius: 0.5 }, 1.0 / 64.0, 2);
        scene.bodies = vec![BodySpec::shaped("ball", 100.0, shape, Motion::Static { position: at })];
        let tree = scene.bodies[0].shape.as_ref().unwrap().tree(100.0, &[]).unwrap();
        (scene, vec![Some(Arc::new(tree))])
    }

    #[test]
    fn probe_reads_the_field_and_the_cell() {
        // S19 T1: beside a voxel ball the field is GM/d² towards its centre, to 1e-3; at twice the
        // distance a quarter of it (the control). A point on its surface is in a cell, and the
        // cell's share of tower 1's phase is the Phase colouring's number for that leaf, bit for
        // bit; the next leaf's is another number.
        let centre = [2.0, 1.0, 3.0];
        let (scene, trees) = ball_at(centre);
        let field = |d: f64| probe(&scene, &trees, 0.0, [centre[0] + d, centre[1], centre[2]]).g;
        let expected = BIG_G_TEST * 100.0 / (1.0 * 1.0);
        let near = field(1.0);
        assert!((near[0] + expected).abs() < 1e-3 * expected, "{near:?} against {expected:e}");
        assert!(near[1].abs() < 1e-3 * expected && near[2].abs() < 1e-3 * expected);
        let far = field(2.0);
        assert!((far[0] / near[0] - 0.25).abs() < 1e-3, "{} ", far[0] / near[0]);
        // A cell the surface cuts (a leaf of one cell, partly filled), probed at its centre.
        let tree = trees[0].as_ref().unwrap();
        let full = 2u8.pow(3);
        let leaf = tree.leaves.iter().position(|l| l.level == 0 && l.count < full).expect("a cut cell");
        let p = Vec3::from_array(centre) + tree.block_centre(tree.leaves[leaf].key, 0);
        let surface = probe(&scene, &trees, 0.0, p.to_array());
        let cell = surface.cell.clone().expect("a cell under the surface");
        assert_eq!(cell.block.0, leaf);
        let shares = crate::voxel_view::leaf_phases(&scene, tree, Vec3::from_array(centre), 0, 0.0).unwrap();
        assert_eq!(surface.towers[0].phase.map(f64::to_bits), Some(shares[leaf].to_bits()));
        assert_ne!(shares[leaf].to_bits(), shares[(leaf + 1) % shares.len()].to_bits());
        assert!((surface.towers[0].distance - p.x.hypot(p.y)).abs() < 1e-12);
        let air = probe(&scene, &trees, 0.0, [0.0, 0.0, 9.0]);
        assert!(air.cell.is_none(), "no cell in the air");
        assert!(air.words(&scene).contains("no body here"));
    }

    #[test]
    fn the_cell_is_the_finest_cell_however_the_tree_merged() {
        // S19 review I2: at the middle of a 0.4 m steel box (pitch 0.05 m) the tree has merged the
        // cells into large blocks, but the probe reports the one cell of 0.05 m holding the point:
        // its mass ρh³, and its share of each tower's phase that cell's read alone. Control: the
        // leaf holding the point is larger than a cell, and reading the whole leaf gives another
        // share.
        let mut scene = crate::dev::void();
        scene.array.towers = vec![TowerSpec::at("tower 1", [0.0, 0.0, 0.0])];
        let mut shape = ShapeSpec::new(ShapeKind::Cuboid { size: [0.4; 3] }, 0.05, 2);
        shape.material = Some("steel".into());
        scene.bodies =
            vec![BodySpec::shaped("box", 1.0, shape, Motion::Static { position: [2.0, 2.0, 1.0] })];
        let tree = Arc::new(scene.bodies[0].shape.as_ref().unwrap().tree(1.0, &[]).unwrap());
        let trees = vec![Some(Arc::clone(&tree))];
        let inside = probe(&scene, &trees, 0.0, [2.01, 2.02, 1.03]);
        let cell = inside.cell.clone().expect("a cell inside the box");
        assert_eq!(cell.side, 0.05, "the cell's side is the pitch");
        assert_eq!(cell.leaf.level, 0);
        let rho = crate::materials::find(&[], "steel").unwrap().density;
        assert!((cell.mass - rho * 0.05f64.powi(3)).abs() < 1e-12 * cell.mass, "{} kg", cell.mass);
        // The cell holds the point.
        let c = Vec3::new(2.0, 2.0, 1.0) + tree.block_centre(cell.leaf.key, 0);
        let off = Vec3::new(2.01, 2.02, 1.03) - c;
        assert!(off.x.abs() <= 0.025 && off.y.abs() <= 0.025 && off.z.abs() <= 0.025, "{off:?}");
        let merged = tree.leaves[cell.block.0];
        assert!(
            merged.level > 0 && cell.block.1 > cell.side,
            "the control: the leaf there is a merged block"
        );
        let at = Vec3::new(2.0, 2.0, 1.0);
        let one = crate::voxel_view::leaf_phase(&scene, &tree, &cell.leaf, at, 0, 0.0).unwrap();
        let whole = crate::voxel_view::leaf_phase(&scene, &tree, &merged, at, 0, 0.0).unwrap();
        assert_eq!(inside.towers[0].phase.map(f64::to_bits), Some(one.to_bits()));
        assert!((whole / one).abs() > 2.0, "the block's share is not the cell's: {whole:e} against {one:e}");
        assert!(inside.words(&scene).contains("in a block of"), "{}", inside.words(&scene));
    }

    #[test]
    fn the_probe_counts_every_body_and_the_floor_and_names_the_rest() {
        // S19 review I3: a voxel body the caller has no tree for (the view's budget left it out)
        // is still counted, the same to rounding as with its tree; a slab floor adds −2πGρt
        // above it; a half-space floor and the world are named as not counted. Control: with the
        // body left out the field differs.
        let centre = [2.0, 1.0, 3.0];
        let (mut scene, trees) = ball_at(centre);
        let point = [3.0, 1.0, 3.0];
        let with = probe(&scene, &trees, 0.0, point);
        let without_tree = probe(&scene, &[None], 0.0, point);
        assert!(
            (without_tree.g[0] - with.g[0]).abs() <= 1e-12 * with.g[0].abs(),
            "{:?} {:?}",
            without_tree.g,
            with.g
        );
        assert!(with.not_counted.is_empty() && with.counted() == "the bodies");
        let mut empty = scene.clone();
        empty.bodies.clear();
        assert_ne!(probe(&empty, &[], 0.0, point).g, with.g, "the control: the ball is counted");
        // A slab floor, 0.5 m thick, under the point.
        scene.floor =
            Some(FloorSpec { height: 0.0, kind: FloorKind::Slab { thickness: 0.5 }, density: 2400.0 });
        let floored = probe(&scene, &trees, 0.0, point);
        let slab = -2.0 * std::f64::consts::PI * G * 2400.0 * 0.5;
        assert!((floored.floor.unwrap()[2] - slab).abs() < 1e-15, "{:?}", floored.floor);
        assert!((floored.g[2] - with.g[2] - slab).abs() < 1e-15);
        assert_eq!(floored.counted(), "the bodies and the floor");
        scene.floor = Some(FloorSpec { height: 0.0, kind: FloorKind::HalfSpace, density: 2400.0 });
        let half = probe(&scene, &trees, 0.0, point);
        assert!(half.floor.is_none() && half.not_counted.iter().any(|w| w.starts_with("the floor")));
        assert!(half.words(&scene).contains("not counted: the floor"));
    }

    const BIG_G_TEST: f64 = cav_core::constants::BIG_G;
}
