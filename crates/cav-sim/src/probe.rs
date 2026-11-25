//! The Probe tool's reading of a point (S19, `docs/slices/S19.md` §2): the gravity of the scene's
//! bodies there, the voxel cell under it, that cell's share of each tower's phase now, and the
//! distance to each tower. A query: nothing in it is an edit.
//!
//! **The field** `g` is every body's attraction at the point, m/s², the world's frame: a point
//! mass (or a sphere of its radius, from inside as from out) in closed form; a voxel body leaf by
//! leaf, each leaf a uniform cube — within [`NEAR_SIDES`] of its side by the central difference of
//! the exact cube potential ([`cav_core::voxel::kernel::cube_potential`]), farther as a point of
//! its mass (what that leaves out is the cube's l = 4 term, under 1e-4 of the leaf's share at
//! [`NEAR_SIDES`] sides). The room, the floor and the world volume are not counted: the reading
//! says "bodies".
//!
//! **The cell's share of a tower's phase** is the leaf read alone through the tower's flight,
//! exactly as the view's Phase colouring reads it ([`crate::voxel_view::leaf_phase`]).

use std::sync::Arc;

use cav_core::Vec3;
use cav_core::voxel::VoxTree;
use cav_core::voxel::kernel::cube_potential;

use crate::SceneSpec;

/// The gravitational constant, m³ kg⁻¹ s⁻².
const G: f64 = cav_core::constants::BIG_G;
/// Within this many of its sides a leaf's field is the exact cube's; beyond, a point's.
pub const NEAR_SIDES: f64 = 8.0;

/// What the probe reads at a point.
#[derive(Clone, Debug, PartialEq)]
pub struct Probe {
    /// The point, metres, the world's frame.
    pub point: [f64; 3],
    /// The bodies' gravity there, m/s², the world's frame (x east, y north, z up).
    pub g: [f64; 3],
    /// The voxel cell the point is in or on, if any.
    pub cell: Option<CellProbe>,
    /// Each tower, in the scene's order.
    pub towers: Vec<TowerProbe>,
}

/// The voxel cell under the probe.
#[derive(Clone, Debug, PartialEq)]
pub struct CellProbe {
    /// The body, from 0.
    pub body: usize,
    /// The leaf, its index in the body's tree.
    pub leaf: usize,
    /// Its material's label, or none for a uniform body.
    pub material: Option<String>,
    /// Its mass, kilograms.
    pub mass: f64,
    /// Its side, metres.
    pub side: f64,
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

/// The probe's reading at `point` (metres, the world's frame) of `scene` at `time`, with
/// `trees[n]` body `n`'s tree (None for a body not made of voxels). The cell is the leaf whose box
/// holds the point, or is within a millionth of its side of it (a point picked on a face).
#[must_use]
pub fn probe(scene: &SceneSpec, trees: &[Option<Arc<VoxTree>>], time: f64, point: [f64; 3]) -> Probe {
    let p = Vec3::from_array(point);
    let gravity = scene.array.instrument.gravity;
    let mut g = Vec3::ZERO;
    let mut cell = None;
    for (n, body) in scene.bodies.iter().enumerate() {
        let (from, until) = body.exists();
        if time < from || time >= until {
            continue;
        }
        let at = Vec3::from_array(body.motion.position(time, gravity));
        match trees.get(n).and_then(Option::as_ref) {
            Some(tree) => {
                for (l, leaf) in tree.leaves.iter().enumerate() {
                    let centre = at + tree.block_centre(leaf.key, leaf.level);
                    let side = tree.side(leaf.level);
                    g = g + cube_field(tree.leaf_mass(leaf), side, p - centre);
                    let off = p - centre;
                    let reach = 0.5 * side * (1.0 + 1e-6);
                    if cell.is_none() && off.x.abs() <= reach && off.y.abs() <= reach && off.z.abs() <= reach
                    {
                        let palette = body.shape.as_ref().map(|s| s.palette()).unwrap_or_default();
                        cell = Some(CellProbe {
                            body: n,
                            leaf: l,
                            material: palette.get(usize::from(leaf.material)).map(|s| (*s).to_owned()),
                            mass: tree.leaf_mass(leaf),
                            side,
                        });
                    }
                }
            }
            None if body.shape.is_none() => {
                let d = p - at;
                let r = d.length();
                if r > 0.0 {
                    // A uniform sphere: a point outside itself; inside, the mass within r.
                    let inside = if r < body.radius { (r / body.radius).powi(3) } else { 1.0 };
                    g = g + d * (-G * body.mass * inside / (r * r * r));
                }
            }
            // A voxel body whose tree was not made (the budget): not counted.
            None => {}
        }
    }
    let towers = scene
        .array
        .towers
        .iter()
        .enumerate()
        .map(|(t, tower)| {
            let distance = (point[0] - tower.position[0]).hypot(point[1] - tower.position[1]);
            let read = cell.as_ref().map(|c| {
                let tree = trees[c.body].as_ref().expect("a cell is of a tree");
                let at = Vec3::from_array(scene.bodies[c.body].motion.position(time, gravity));
                crate::voxel_view::leaf_phase(scene, tree, &tree.leaves[c.leaf], at, t, time)
            });
            let (phase, why) = match read {
                Some(Ok(phase)) => (Some(phase), None),
                Some(Err(why)) => (None, Some(why)),
                None => (None, None),
            };
            TowerProbe { label: tower.label.clone(), distance, phase, why }
        })
        .collect();
    Probe { point, g: g.to_array(), cell, towers }
}

impl Probe {
    /// The reading in words, a line a fact, as the console prints it.
    #[must_use]
    pub fn words(&self, scene: &SceneSpec) -> String {
        let [x, y, z] = self.point;
        let g = Vec3::from_array(self.g);
        let mut out = format!(
            "at ({x:.4}, {y:.4}, {z:.4}) m\ng from the bodies: {:.6e} m/s² ({:.4e}, {:.4e}, {:.4e})\n",
            g.length(),
            self.g[0],
            self.g[1],
            self.g[2]
        );
        match &self.cell {
            Some(c) => out.push_str(&format!(
                "cell of `{}`: {}, {:.6e} kg, {} m a side\n",
                scene.bodies[c.body].label,
                c.material.as_deref().unwrap_or("uniform"),
                c.mass,
                c.side
            )),
            None => out.push_str("no voxel cell here\n"),
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
    use crate::{BodySpec, Motion, ShapeKind, ShapeSpec, TowerSpec};

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
        let surface = probe(&scene, &trees, 0.0, [centre[0] + 0.49, centre[1], centre[2]]);
        let cell = surface.cell.clone().expect("a cell under the surface");
        let tree = trees[0].as_ref().unwrap();
        let shares = crate::voxel_view::leaf_phases(&scene, tree, Vec3::from_array(centre), 0, 0.0).unwrap();
        assert_eq!(surface.towers[0].phase.map(f64::to_bits), Some(shares[cell.leaf].to_bits()));
        assert_ne!(shares[cell.leaf].to_bits(), shares[(cell.leaf + 1) % shares.len()].to_bits());
        assert!((surface.towers[0].distance - (2.49f64).hypot(1.0)).abs() < 1e-12);
        assert!(probe(&scene, &trees, 0.0, [0.0, 0.0, 9.0]).cell.is_none(), "no cell in the air");
    }

    const BIG_G_TEST: f64 = cav_core::constants::BIG_G;
}
