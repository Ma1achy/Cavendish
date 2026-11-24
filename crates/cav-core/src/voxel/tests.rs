//! The voxeliser against its oracle, the shapes against their closed forms, the merge against no
//! merge: the S3 brief's R3 to R6 (`docs/slices/S3.md` §3.3 has each criterion as written first).

use super::kernel::{TreeBody, cube_potential};
use super::mesh::{Mesh, box_mesh, torus_mesh};
use super::shape::{Cuboid, Cylinder, Ellipsoid, Platonic, Polyhedron, Sphere};
use super::*;
use crate::Vec3;
use crate::forward::{Instrument, Reader};

/// A small deterministic generator for test placements (SplitMix64).
struct Numbers(u64);

impl Numbers {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A polyhedron as a closed mesh: each face's fan.
fn mesh_of(p: &Polyhedron) -> Mesh {
    let triangles = p
        .faces
        .iter()
        .flat_map(|f| (1..f.len() - 1).map(move |t| [f[0] as u32, f[t] as u32, f[t + 1] as u32]))
        .collect();
    Mesh::new(p.vertices.clone(), triangles).unwrap()
}

/// Every kind of shape, each about a metre across, centred at `at`.
fn shapes(at: Vec3) -> Vec<(String, Box<dyn Shape>)> {
    let mut out: Vec<(String, Box<dyn Shape>)> = vec![
        ("sphere".into(), Box::new(Sphere { centre: at, radius: 0.5 })),
        ("cuboid".into(), Box::new(Cuboid { centre: at, half: Vec3::new(0.5, 0.3, 0.2) })),
        ("ellipsoid".into(), Box::new(Ellipsoid { centre: at, radii: Vec3::new(0.5, 0.35, 0.25) })),
    ];
    for axis in 0..3 {
        out.push((
            format!("cylinder {axis}"),
            Box::new(Cylinder { centre: at, radius: 0.3, half_length: 0.5, axis }),
        ));
    }
    for solid in Platonic::ALL {
        out.push((format!("{solid:?}"), Box::new(solid.solid(0.6).moved(at))));
    }
    let mut numbers = Numbers(7);
    let points: Vec<Vec3> = (0..12)
        .map(|_| Vec3::new(numbers.next() - 0.5, numbers.next() - 0.5, numbers.next() - 0.5) + at)
        .collect();
    out.push(("random hull".into(), Box::new(Polyhedron::hull(&points).unwrap())));
    let (v, t) = box_mesh(at - Vec3::new(0.5, 0.3, 0.2), at + Vec3::new(0.5, 0.3, 0.2));
    out.push(("box mesh".into(), Box::new(Mesh::new(v, t).unwrap())));
    let (v, t) = torus_mesh(0.35, 0.15, 24, 12);
    let v = v.into_iter().map(|p| p + at).collect();
    out.push(("torus mesh".into(), Box::new(Mesh::new(v, t).unwrap())));
    out.push(("icosahedron mesh".into(), Box::new(mesh_of(&Platonic::Icosahedron.solid(0.6).moved(at)))));
    out
}

/// A shape whose `cover` lies: it says `Inside` for any box whose centre is inside.
struct Liar<'a>(&'a dyn Shape);

impl Shape for Liar<'_> {
    fn bounds(&self) -> Aabb {
        self.0.bounds()
    }
    fn contains(&self, p: Vec3) -> bool {
        self.0.contains(p)
    }
    fn cover(&self, b: &Aabb) -> Cover {
        match self.0.cover(b) {
            Cover::Mixed if self.0.contains(b.centre()) => Cover::Inside,
            other => other,
        }
    }
    fn volume(&self) -> f64 {
        self.0.volume()
    }
    fn centroid(&self) -> Vec3 {
        self.0.centroid()
    }
    fn second_moments(&self) -> [f64; 6] {
        self.0.second_moments()
    }
}

/// Points of a box at which a cover's answer is checked: its corners, its centre, random points.
fn points_of(b: &Aabb, numbers: &mut Numbers) -> Vec<Vec3> {
    let mut out: Vec<Vec3> = (0..8).map(|i| b.corner(i)).collect();
    out.push(b.centre());
    let s = b.size();
    out.extend(
        (0..24).map(|_| b.lo + Vec3::new(s.x * numbers.next(), s.y * numbers.next(), s.z * numbers.next())),
    );
    out
}

/// How many (box, point) pairs break the contract of `cover` over `boxes` random boxes.
fn cover_breaches(shape: &dyn Shape, boxes: usize, numbers: &mut Numbers) -> usize {
    let bounds = shape.bounds().grown(0.2);
    let mut breaches = 0;
    for _ in 0..boxes {
        let size = bounds.size();
        let lo =
            bounds.lo + Vec3::new(size.x * numbers.next(), size.y * numbers.next(), size.z * numbers.next());
        let side = 0.4 * numbers.next().powi(2);
        let b = Aabb::new(lo, lo + Vec3::new(side, side * numbers.next(), side));
        let want = match shape.cover(&b) {
            Cover::Inside => true,
            Cover::Outside => false,
            Cover::Mixed => continue,
        };
        breaches += points_of(&b, numbers).into_iter().filter(|p| shape.contains(*p) != want).count();
    }
    breaches
}

#[test]
fn cover_is_conservative_for_every_shape() {
    let mut numbers = Numbers(11);
    for (name, shape) in shapes(Vec3::new(0.1, -0.2, 0.05)) {
        assert_eq!(cover_breaches(shape.as_ref(), 3000, &mut numbers), 0, "{name}");
    }
    // Control: a cover that says `Inside` too readily is caught.
    let sphere = Sphere { centre: Vec3::ZERO, radius: 0.5 };
    assert!(cover_breaches(&Liar(&sphere), 3000, &mut numbers) > 0);
}

/// Cells whose count differs between the tree expanded and the raster.
fn differing(tree: &VoxTree, shape: &dyn Shape) -> usize {
    let expanded = Raster::of_tree(tree).unwrap();
    let oracle = raster(shape, &tree.grid).unwrap();
    expanded.counts.iter().zip(&oracle.counts).filter(|(a, b)| a != b).count()
}

/// Every shape, at three pitches and three supersamplings, at random placements: the cells that
/// differ between the tree and the raster, in all.
fn tree_against_raster() -> usize {
    let mut numbers = Numbers(3);
    let mut total = 0;
    for (cells, k) in [(8.0, 1), (13.0, 2), (21.0, 4)] {
        let at = Vec3::new(numbers.next(), numbers.next(), numbers.next()) * 0.3;
        for (_, shape) in shapes(at) {
            let pitch = 1.05 / cells;
            let grid = Grid::around(shape.bounds(), pitch, k).unwrap();
            let tree = voxelise(shape.as_ref(), &grid, 1.0, 0).unwrap();
            total += differing(&tree, shape.as_ref());
        }
    }
    total
}

cav_validate::case! {
    test: validation_the_tree_is_the_raster,
    name: "voxel.tree_equals_raster",
    subsystem: "voxel",
    slice: "S3",
    reference: Exact,
    against: "the raster voxeliser over every cell: 17 shapes (primitives, Platonic solids, a random hull, three meshes) at 8, 13 and 21 cells across with k = 1, 2, 4, placed at random",
    unit: Count,
    tolerance: 0.0,
    measure: cav_validate::Measured::Error(tree_against_raster() as f64),
}

#[test]
fn the_tree_is_the_raster_and_the_controls_fail() {
    let sphere = Sphere { centre: Vec3::ZERO, radius: 0.5 };
    let grid = Grid::around(sphere.bounds(), 1.0 / 16.0, 2).unwrap();
    let tree = voxelise(&sphere, &grid, 1.0, 0).unwrap();
    assert_eq!(differing(&tree, &sphere), 0);
    // The merge happened: fewer leaves than cells with something in them.
    let full = Raster::of_tree(&tree).unwrap().counts.iter().filter(|&&c| c > 0).count();
    assert!(tree.leaves.len() < full, "{} leaves for {full} cells", tree.leaves.len());
    // Control: one count changed in a leaf.
    let mut changed = tree.clone();
    let partial = changed.leaves.iter().position(|l| l.count > 1 && l.count < 8).unwrap();
    changed.leaves[partial].count -= 1;
    assert_eq!(differing(&changed, &sphere), 1);
    // Control: a shape whose cover lies.
    let liar = Liar(&sphere);
    let lied = voxelise(&liar, &grid, 1.0, 0).unwrap();
    assert!(differing(&lied, &sphere) > 0);
}

/// `|Σm − M|/M` for every shape at 32 cells across.
fn mass_error() -> f64 {
    let mut worst = 0f64;
    for (_, shape) in shapes(Vec3::ZERO) {
        let grid = Grid::around(shape.bounds(), 1.05 / 32.0, 4).unwrap();
        let tree = voxelise(shape.as_ref(), &grid, 37.5, 0).unwrap();
        let sum: f64 = tree.leaves.iter().map(|l| tree.leaf_mass(l)).sum();
        worst = worst.max((sum - 37.5).abs() / 37.5);
    }
    worst
}

cav_validate::case! {
    test: validation_the_mass_is_the_spec_s,
    name: "voxel.mass_is_the_spec",
    subsystem: "voxel",
    slice: "S3",
    reference: Exact,
    against: "the spec's mass, against the Morton-order sum of the leaves' masses, 17 shapes at 32 cells across",
    unit: Relative,
    tolerance: 1e-12,
    measure: cav_validate::Measured::Error(mass_error()),
}

/// The voxelised volume's and inertia's errors against the closed forms, worst over the
/// primitives and Platonic solids, at `cells` finest cells across the longest side with `k = 4`.
pub(crate) fn volume_and_inertia_errors(cells: f64) -> (f64, f64) {
    let (mut volume, mut inertia) = (0f64, 0f64);
    for (name, shape) in shapes(Vec3::ZERO) {
        if name.contains("mesh") || name.contains("hull") {
            continue;
        }
        let size = shape.bounds().size();
        let pitch = size.x.max(size.y).max(size.z) / cells;
        let grid = Grid::around(shape.bounds(), pitch, 4).unwrap();
        let mass = 3.0;
        let tree = voxelise(shape.as_ref(), &grid, mass, 0).unwrap();
        let k3 = f64::from(grid.samples_per_cell());
        let voxels = tree.samples as f64 / k3 * pitch.powi(3);
        volume = volume.max((voxels / shape.volume() - 1.0).abs());
        let got = tree.body().inertia();
        let s = shape.second_moments().map(|v| v * mass / shape.volume());
        let want = super::Moments { mass, centre: Vec3::ZERO, second: s, third: [0.0; 10] }.inertia();
        let largest = want[0].max(want[1]).max(want[2]);
        for i in 0..6 {
            inertia = inertia.max((got[i] - want[i]).abs() / largest);
        }
    }
    (volume, inertia)
}

cav_validate::case! {
    test: validation_volume_and_inertia_match_the_analytic,
    name: "voxel.volume_and_inertia",
    subsystem: "voxel",
    slice: "S3",
    reference: ClosedForm,
    against: "each primitive's and Platonic solid's closed-form volume (tolerance 2e-3) and inertia tensor (5e-3 of the largest principal moment, the error shown as a fraction of that tolerance), at 64 cells across, k = 4",
    unit: Relative,
    tolerance: 2e-3,
    measure: {
        let (volume, inertia) = volume_and_inertia_errors(64.0);
        cav_validate::Measured::Error(volume.max(inertia * 2e-3 / 5e-3))
    },
}

/// The potential at 24 points and one shot's three channels for a tower 2 m away, merged against
/// unmerged, as fractions of the largest |V| and of the larger single interferometer's phase.
fn merge_differences(shape: &dyn Shape, cells: f64) -> (f64, f64) {
    let size = shape.bounds().size();
    let grid = Grid::around(shape.bounds(), size.x.max(size.y).max(size.z) / cells, 2).unwrap();
    let merged = voxelise(shape, &grid, 50.0, 0).unwrap();
    let flat = VoxTree::build(grid, super::unmerged(&merged), 50.0).unwrap();
    assert!(merged.leaves.len() < flat.leaves.len());
    let potential = |tree: &VoxTree, p: Vec3| -> f64 {
        tree.leaves
            .iter()
            .map(|l| {
                cube_potential(tree.leaf_mass(l), tree.side(l.level), p - tree.block_centre(l.key, l.level))
            })
            .sum()
    };
    let mut worst_v = 0f64;
    let mut numbers = Numbers(5);
    for k in 0..24 {
        let n = Vec3::new(numbers.next() - 0.5, numbers.next() - 0.5, numbers.next() - 0.5).normalised();
        let r = 0.6 * (10f64).powf(k as f64 / 23.0 * 1.3);
        let (a, b) = (potential(&merged, n * r), potential(&flat, n * r));
        worst_v = worst_v.max((a - b).abs() / a.abs().max(b.abs()));
    }
    let reader = Reader::new(Instrument::aion10());
    let tower = Vec3::new(-2.0, 0.5, -1.0);
    let read = |tree: &VoxTree| reader.read(tower, &TreeBody::new(tree, |_| Vec3::ZERO), 10.0);
    let (a, b) = (read(&merged), read(&flat));
    let scale = a.lower.abs().max(a.upper.abs());
    let worst_phase = (a.lower - b.lower)
        .abs()
        .max((a.upper - b.upper).abs())
        .max(((a.lower - a.upper) - (b.lower - b.upper)).abs());
    (worst_v, worst_phase / scale)
}

/// The worst of [`merge_differences`] over the primitives and Platonic solids, read in parallel.
fn merge_error() -> f64 {
    let all: Vec<(String, Box<dyn Shape>)> =
        shapes(Vec3::ZERO).into_iter().filter(|(n, _)| !n.contains("mesh") && !n.contains("hull")).collect();
    std::thread::scope(|scope| {
        let handles: Vec<_> =
            all.iter().map(|(_, s)| scope.spawn(move || merge_differences(s.as_ref(), 32.0))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).fold(0f64, |w, (v, p)| w.max(v).max(p))
    })
}

cav_validate::case! {
    test: validation_merged_equals_unmerged,
    name: "voxel.merged_equals_unmerged",
    subsystem: "voxel",
    slice: "S3",
    reference: Exact,
    against: "the same body as unmerged finest cubes: potentials at 24 points from 0.6 to 12 m and one AION-10 shot 2 m away, each primitive and Platonic solid at 32 cells across",
    unit: Relative,
    tolerance: 1e-12,
    measure: cav_validate::Measured::Error(merge_error()),
}

#[test]
fn a_merge_of_unequal_children_is_seen() {
    let sphere = Sphere { centre: Vec3::ZERO, radius: 0.5 };
    let grid = Grid::around(sphere.bounds(), 1.0 / 16.0, 2).unwrap();
    let tree = voxelise(&sphere, &grid, 50.0, 0).unwrap();
    // As if eight children whose counts differed by one had been merged: one merged leaf's
    // count is off by one.
    let mut wrong = tree.leaves.clone();
    let merged = wrong.iter().position(|l| l.level >= 1 && l.count > 1).unwrap();
    wrong[merged].count -= 1;
    let wrong = VoxTree::build(grid, wrong, 50.0).unwrap();
    let reader = Reader::new(Instrument::aion10());
    let tower = Vec3::new(-2.0, 0.5, -1.0);
    let a = reader.read(tower, &TreeBody::new(&tree, |_| Vec3::ZERO), 10.0);
    let b = reader.read(tower, &TreeBody::new(&wrong, |_| Vec3::ZERO), 10.0);
    assert!((a.lower - b.lower).abs() > 1e-9 * a.lower.abs(), "{a:?} {b:?}");
}

#[test]
fn a_cube_mesh_voxelises_as_the_cuboid() {
    let (lo, hi) = (Vec3::new(-0.5, -0.3, -0.2), Vec3::new(0.5, 0.3, 0.2));
    let cuboid = Cuboid { centre: Vec3::ZERO, half: hi };
    let (v, t) = box_mesh(lo, hi);
    let mesh = Mesh::new(v, t).unwrap();
    for (pitch, k) in [(0.1, 1), (0.07, 3), (0.033, 4)] {
        let grid = Grid::around(cuboid.bounds(), pitch, k).unwrap();
        let a = voxelise(&cuboid, &grid, 1.0, 0).unwrap();
        let b = voxelise(&mesh, &grid, 1.0, 0).unwrap();
        assert_eq!(a, b, "pitch {pitch}");
    }
}

#[test]
fn refusals_name_the_number_at_fault() {
    let sphere = Sphere { centre: Vec3::ZERO, radius: 0.5 };
    assert!(Grid::around(sphere.bounds(), 1.0 / 2000.0, 1).unwrap_err().0.contains("pitch"));
    assert!(Grid::around(sphere.bounds(), 0.01, 7).unwrap_err().0.contains("supersample"));
    assert!(Grid::around(sphere.bounds(), -1.0, 2).is_err());
    // A pitch so coarse that no sample point is inside.
    let tiny = Sphere { centre: Vec3::new(0.3, 0.3, 0.3), radius: 0.01 };
    let grid = Grid { origin: Vec3::ZERO, pitch: 1.0, depth: 0, cells: [1, 1, 1], supersample: 1 };
    assert!(voxelise(&tiny, &grid, 1.0, 0).unwrap_err().0.contains("finer pitch"));
}
