//! The cut cells against the shapes' closed forms (C1), their caps (C5), and the clippers against
//! each other (`docs/slices/S5.md` §3 has each criterion as written first).

use super::convex::box_raw;
use super::*;
use crate::polyhedral::Faceted;
use crate::voxel::mesh::{Mesh, box_mesh, torus_mesh};
use crate::voxel::shape::{Cuboid, Cylinder, Ellipsoid, Platonic, Polyhedron, Shell, Sphere};
use crate::voxel::tree::Moments;

/// A rotation by `angle` radians about the unit axis `axis` (Rodrigues).
fn turn(axis: Vec3, angle: f64, p: Vec3) -> Vec3 {
    let k = axis.normalised();
    let (s, c) = angle.sin_cos();
    p * c + k.cross(p) * s + k * (k.dot(p) * (1.0 - c))
}

/// The cube of side `side` turned about a general axis, as a hull.
pub(crate) fn turned_cube(side: f64) -> Polyhedron {
    let h = 0.5 * side;
    let corners: Vec<Vec3> = (0..8)
        .map(|i| {
            let p = Vec3::new(
                if i & 1 == 0 { -h } else { h },
                if i & 2 == 0 { -h } else { h },
                if i & 4 == 0 { -h } else { h },
            );
            turn(Vec3::new(0.3, -0.5, 0.81), 0.7, p)
        })
        .collect();
    Polyhedron::hull(&corners).unwrap()
}

/// The shapes of C1, each with its closed-form moments at unit density (volume, centroid, second
/// and third central moments) and its size.
pub(crate) fn c1_shapes() -> Vec<(&'static str, Box<dyn Shape>, Moments)> {
    let analytic = |s: &dyn Shape| Moments {
        mass: s.volume(),
        centre: s.centroid(),
        second: s.second_moments(),
        third: [0.0; 10],
    };
    let mut out: Vec<(&'static str, Box<dyn Shape>, Moments)> = Vec::new();
    let cube = Cuboid { centre: Vec3::new(0.013, -0.021, 0.007), half: Vec3::new(0.4, 0.4, 0.4) };
    out.push(("aligned cube", Box::new(cube), analytic(&cube)));
    let turned = turned_cube(0.8);
    let m = Faceted::from_polyhedron(&turned).unwrap().moments(1.0);
    out.push(("turned cube", Box::new(turned), m));
    let ico = Platonic::Icosahedron.solid(0.6);
    let m = Faceted::from_polyhedron(&ico).unwrap().moments(1.0);
    out.push(("icosahedron", Box::new(ico), m));
    let sphere = Sphere { centre: Vec3::new(0.011, 0.0, -0.017), radius: 0.5 };
    out.push(("sphere", Box::new(sphere), analytic(&sphere)));
    let ellipsoid = Ellipsoid { centre: Vec3::ZERO, radii: Vec3::new(0.5, 0.35, 0.25) };
    out.push(("ellipsoid", Box::new(ellipsoid), analytic(&ellipsoid)));
    for axis in 0..3 {
        let cylinder = Cylinder { centre: Vec3::new(0.0, 0.009, 0.0), radius: 0.3, half_length: 0.45, axis };
        out.push((["cylinder x", "cylinder y", "cylinder z"][axis], Box::new(cylinder), analytic(&cylinder)));
    }
    let shell = Shell { centre: Vec3::ZERO, outer: 0.5, inner: 0.3 };
    out.push(("shell", Box::new(shell), analytic(&shell)));
    let (v, t) = torus_mesh(0.35, 0.15, 24, 12);
    let torus = Mesh::new(v, t).unwrap();
    let m = Faceted::from_mesh(&torus).unwrap().moments(1.0);
    out.push(("torus mesh", Box::new(torus), m));
    out
}

/// The worst difference of `got` from `want`, each order relative to its own scale (volume;
/// volume × size; × size²; × size³).
fn moments_error(got: &Moments, want: &Moments, size: f64) -> f64 {
    let v = want.mass;
    let mut worst = (got.mass - v).abs() / v;
    let d = got.centre - want.centre;
    worst = worst.max(d.x.abs().max(d.y.abs()).max(d.z.abs()) / size);
    for i in 0..6 {
        worst = worst.max((got.second[i] - want.second[i]).abs() / (v * size * size));
    }
    for i in 0..10 {
        worst = worst.max((got.third[i] - want.third[i]).abs() / (v * size.powi(3)));
    }
    worst
}

/// The pitch of C1: not a divisor of any size, so no face lies on the lattice's planes.
const C1_PITCH: f64 = 0.8 / 16.37;

fn c1_tree(shape: &dyn Shape) -> VoxTree {
    let grid = Grid::around(shape.bounds(), C1_PITCH, 4).unwrap();
    voxelise_cut(shape, &grid, Weight::Density(1.0), 0, Budget::DEFAULT).unwrap()
}

/// C1's measurement: the worst, over the shapes, of the tree's body moments against the closed
/// forms; and the least the control (one piece of median volume dropped) misses by.
fn c1_errors() -> (f64, f64) {
    let shapes = c1_shapes();
    let results: Vec<(f64, f64)> = std::thread::scope(|scope| {
        let handles: Vec<_> = shapes
            .iter()
            .map(|(name, shape, want)| {
                scope.spawn(move || {
                    let tree = c1_tree(shape.as_ref());
                    assert!(!tree.cuts.is_empty(), "{name}: no cut cells");
                    let size =
                        shape.bounds().size().x.max(shape.bounds().size().y).max(shape.bounds().size().z);
                    let error = moments_error(&tree.body(), want, size);
                    // Control: the piece of median volume dropped, with its leaf.
                    let mut by_volume: Vec<&CutCell> = tree.cuts.iter().collect();
                    by_volume.sort_by(|a, b| a.piece.volume.total_cmp(&b.piece.volume));
                    let dropped = by_volume[by_volume.len() / 2].key;
                    let leaves =
                        tree.leaves.iter().copied().filter(|l| l.key != dropped || l.level != 0).collect();
                    let cuts = tree.cuts.iter().copied().filter(|c| c.key != dropped).collect();
                    let broken = VoxTree::build_cut(tree.grid, leaves, cuts, 1.0, Budget::DEFAULT).unwrap();
                    let control = moments_error(&broken.body(), want, size);
                    eprintln!("C1 {name}: {error:.3e} (control {control:.3e}, {} cuts)", tree.cuts.len());
                    (error, control)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let worst = results.iter().fold(0f64, |w, r| w.max(r.0));
    let least_control = results.iter().fold(f64::INFINITY, |w, r| w.min(r.1));
    (worst, least_control)
}

cav_validate::case! {
    test: validation_cut_cells_sum_to_the_shape,
    name: "cut.cells_sum_to_the_shape",
    subsystem: "voxel",
    slice: "S5",
    reference: ClosedForm,
    against: "the closed-form volume, centroid, second and third central moments of an aligned and a turned cube, an icosahedron, a sphere, an ellipsoid, a cylinder along each axis, a shell and a closed torus mesh (the polyhedra's and the mesh's by the divergence theorem), each voxelised at 0.8/16.37 m with exact cut cells: interior cubes plus pieces",
    unit: Relative,
    tolerance: 1e-12,
    measure: cav_validate::Measured::Error({
        let (worst, control) = c1_errors();
        // Control: one piece dropped, on every shape, misses by more than the tolerance.
        assert!(control > 1e-12, "the control missed by only {control:e}");
        worst
    }),
}

#[test]
fn a_box_clipped_by_a_plane_is_its_closed_form() {
    // The unit cube about the origin cut by x + y + z ≤ 0.3: the part of it below the plane.
    // By symmetry and the volume of the corner cut away, against a fine sum of thin slabs.
    let mut piece = Convex::from_box(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5));
    let n = Vec3::new(1.0, 1.0, 1.0);
    piece.clip(n, 0.3, 1e-15);
    let raw = piece.raw();
    // The part above is the corner tetrahedron with legs 1.2 − ... : x' = 0.5 − x etc. sum ≤ 1.2,
    // a simplex of side 1.2 (the cube's corners at sum 1.5 lie past it, the edges' ends at 0.5
    // cut it: legs 1.2 > 1, so the corner cut is the simplex less three simplices of side 0.2).
    let simplex = |a: f64| a.powi(3) / 6.0;
    let above = simplex(1.2) - 3.0 * simplex(0.2);
    assert!((raw.volume - (1.0 - above)).abs() < 1e-14, "{}", raw.volume);
    // Each face loop has three corners or more and the cap is one face more than the box's six.
    assert_eq!(piece.faces.len(), 7);
    // A plane through a face (the cube's top) changes nothing; one past it leaves nothing.
    let mut same = Convex::from_box(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5));
    same.clip(Vec3::Z, 0.5, 1e-15);
    assert_eq!(same.raw(), box_raw(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5)));
    same.clip(Vec3::Z, -0.5, 1e-15);
    assert!(same.is_empty());
    // Control: a plane flipped keeps the other part.
    let mut other = Convex::from_box(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5));
    other.clip(-n, -0.3, 1e-15);
    assert!((other.raw().volume - above).abs() < 1e-14);
}

#[test]
fn a_box_mesh_s_pieces_are_the_cuboid_s() {
    // The prism decomposition of a box mesh against two boxes' closed-form intersection, cell by
    // cell, at a pitch whose lattice the box's faces do not lie on and at one they do.
    let cuboid = Cuboid { centre: Vec3::new(0.01, 0.02, -0.03), half: Vec3::new(0.5, 0.3, 0.2) };
    let (v, t) = box_mesh(cuboid.centre - cuboid.half, cuboid.centre + cuboid.half);
    let mesh = Mesh::new(v, t).unwrap();
    for pitch in [0.07, 0.1] {
        let grid = Grid::around(cuboid.bounds(), pitch, 2).unwrap();
        let a = voxelise_cut(&cuboid, &grid, Weight::Density(1.0), 0, Budget::DEFAULT).unwrap();
        let b = voxelise_cut(&mesh, &grid, Weight::Density(1.0), 0, Budget::DEFAULT).unwrap();
        assert_eq!(a.leaves, b.leaves, "pitch {pitch}");
        let mut worst = 0f64;
        for (x, y) in a.cuts.iter().zip(&b.cuts) {
            assert_eq!(x.key, y.key);
            worst = worst.max((x.piece.volume - y.piece.volume).abs() / pitch.powi(3));
            worst = worst.max((x.piece.centroid - y.piece.centroid).length() / pitch);
        }
        assert!(worst < 1e-13, "pitch {pitch}: {worst:e}");
        if pitch == 0.1 {
            // The faces on the lattice's planes: every cell is whole, no cut at all.
            assert!(a.cuts.is_empty() && b.cuts.is_empty(), "{} {}", a.cuts.len(), b.cuts.len());
        }
    }
}

#[test]
fn sampled_is_unchanged_and_cut_trees_read_their_pieces() {
    // A tree of sampled counts has no cuts and reads by every representation as S3's does.
    let sphere = Sphere { centre: Vec3::ZERO, radius: 0.4 };
    let grid = Grid::around(sphere.bounds(), 0.09, 3).unwrap();
    let sampled = crate::voxel::voxelise(&sphere, &grid, 100.0, 0).unwrap();
    assert!(sampled.cuts.is_empty() && sampled.density == 0.0);
    let cut = voxelise_cut(&sphere, &grid, Weight::Mass(100.0), 0, Budget::DEFAULT).unwrap();
    assert!((cut.body().mass - 100.0).abs() < 1e-12 * 100.0);
    let leaf = cut.leaves.iter().find(|l| cut.cut_of(l).is_some()).unwrap();
    let piece = cut.cut_of(leaf).unwrap().piece;
    assert!((cut.leaf_mass(leaf) - cut.density * piece.volume).abs() < 1e-15 * cut.leaf_mass(leaf));
    assert_eq!(cut.leaf_moments(leaf).centre, piece.centroid);
    // Control: a cut tree's leaf mass is not its quantised count's.
    let by_count = f64::from(leaf.count) * cut.unit;
    assert!((by_count - cut.leaf_mass(leaf)).abs() > 1e-6 * by_count);
    // Its bytes count the cuts.
    let without = VoxTree { cuts: Vec::new(), ..cut.clone() };
    assert_eq!(cut.bytes() - without.bytes(), cut.cuts.len() * size_of::<CutCell>());
}

#[test]
fn cut_cells_are_bounded() {
    // C5: the clipping is counted in the work cap, the cuts in the tree's bytes, each refused
    // before it is done or reserved.
    let ico = Platonic::Icosahedron.solid(0.6);
    let grid = Grid::around(ico.bounds(), 0.6 / 12.3, 4).unwrap();
    let made = voxelise_cut(&ico, &grid, Weight::Density(1.0), 0, Budget::DEFAULT).unwrap();
    let cuts = made.cuts.len();
    // The work: the clipping alone passes these caps, so each is refused, in words.
    let quick =
        voxelise_cut(&ico, &grid, Weight::Density(1.0), 0, Budget { work: 1 << 40, ..Budget::DEFAULT });
    assert!(quick.is_ok());
    let clipping = cuts as u64 * CLIP_WORK_PER_PLANE * ico.planes.len() as u64;
    for work in [clipping / 2, clipping] {
        let refused = voxelise_cut(&ico, &grid, Weight::Density(1.0), 0, Budget { work, ..Budget::DEFAULT });
        assert!(refused.unwrap_err().0.contains("clipping"), "work {work}");
    }
    // The bytes: the leaves alone fit, the leaves and the cuts do not.
    let leaves = made.leaves.len() * size_of::<Leaf>();
    let all = leaves + cuts * size_of::<CutCell>();
    let refused = voxelise_cut(&ico, &grid, Weight::Density(1.0), 0, Budget::with_bytes(all - 1));
    assert!(refused.unwrap_err().0.contains("cut cells"));
    // Control: with room for them and the nodes, it is made, and its bytes are what was counted.
    // (The nodes are counted before they are made with one child entry more than a tree has:
    // its root is no one's child.)
    let room = made.bytes() + size_of::<u32>();
    let fits = voxelise_cut(&ico, &grid, Weight::Density(1.0), 0, Budget::with_bytes(room)).unwrap();
    assert_eq!(fits, made);
    // The tree's own check counts the cuts too: built from the same parts with a byte short.
    let short = VoxTree::build_cut(
        made.grid,
        made.leaves.clone(),
        made.cuts.clone(),
        1.0,
        Budget::with_bytes(room - 1),
    );
    assert!(short.is_err());
    // A shape with no clipper is refused in words.
    struct Bare(Sphere);
    impl Shape for Bare {
        fn bounds(&self) -> Aabb {
            self.0.bounds()
        }
        fn contains(&self, p: Vec3) -> bool {
            self.0.contains(p)
        }
        fn cover(&self, b: &Aabb) -> Cover {
            self.0.cover(b)
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
    let bare = Bare(Sphere { centre: Vec3::ZERO, radius: 0.3 });
    let grid = Grid::around(bare.bounds(), 0.1, 2).unwrap();
    assert!(
        voxelise_cut(&bare, &grid, Weight::Mass(1.0), 0, Budget::DEFAULT).unwrap_err().0.contains("clipper")
    );
}

/// A uniform polyhedron as a source: the closed form, the reference `Exact` read of C2.
struct ExactSource(Faceted, f64);

impl crate::forward::Source for ExactSource {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, _time: f64) -> [f64; 2] {
        let at = Vec3::ZERO - tower;
        heights.map(|h| {
            self.0.difference(self.1, [-at.x, -at.y], h + lower - at.z, h + upper - at.z, lower - upper)
        })
    }
}

/// The representations that read a tree alone (C2's ladder): all but `Pieces`.
const LADDER: [Representation; 4] =
    [Representation::Sampled, Representation::Fill, Representation::Centroid, Representation::Second];

/// The density of C2's bodies, kg/m³.
const C2_DENSITY: f64 = 2700.0;
/// The cells across each body's box at C2's pitches, less the 0.37 that keeps every face off the
/// lattice's planes at the same place in its cell at every pitch: 0.8 m over 8.37 to 32.37.
const C2_CELLS: [u32; 5] = [8, 11, 16, 23, 32];

/// The tower's base: 1.5 m beyond the surface along (1, 0.2, −0.3) from the body's centre, so
/// that the arms rise past the body ([X] §3.1's direction).
fn c2_tower(shape: &dyn Shape) -> Vec3 {
    let n = Vec3::new(1.0, 0.2, -0.3).normalised();
    let (mut inside, mut outside) = (0.0, 2.0);
    for _ in 0..80 {
        let mid = 0.5 * (inside + outside);
        if shape.contains(n * mid) { inside = mid } else { outside = mid }
    }
    n * (outside + 1.5)
}

/// The worst channel's phase difference, radians.
fn worst_channel(a: crate::forward::Reading, b: crate::forward::Reading) -> f64 {
    (a.lower - b.lower)
        .abs()
        .max((a.upper - b.upper).abs())
        .max(((a.lower - a.upper) - (b.lower - b.upper)).abs())
}

/// The least-squares slope of `ln error` against `ln h`.
fn order(points: &[(f64, f64)]) -> f64 {
    let n = points.len() as f64;
    let (x, y): (Vec<f64>, Vec<f64>) = points.iter().map(|(h, e)| (h.ln(), e.ln())).unzip();
    let (mx, my) = (x.iter().sum::<f64>() / n, y.iter().sum::<f64>() / n);
    let sxy: f64 = x.iter().zip(&y).map(|(a, b)| (a - mx) * (b - my)).sum();
    let sxx: f64 = x.iter().map(|a| (a - mx) * (a - mx)).sum();
    sxy / sxx
}

/// C2's lattice placements: each the grid's origin moved by these fractions of a cell along x,
/// y and z, so that every face sits at its own place in its cells ([X] §3's four random sub-cell
/// placements). The same fractions at every pitch, and the size over the pitch a whole number
/// and 0.37, keep each face at the same place in its cell as the pitch changes. Each puts the
/// aligned cube's faces at least 0.05 of a cell from the planes of the `k = 4` sample points: a
/// face on such a plane (the first table had one, at 0.625 of a cell) is counted inside or out
/// by rounding, which changes with the pitch and is not the order being measured.
const C2_PLACEMENTS: [[f64; 3]; 4] =
    [[0.13, 0.37, 0.88], [0.62, 0.11, 0.25], [0.38, 0.87, 0.51], [0.86, 0.49, 0.74]];

/// The grid of pitch `h` about `shape` with its origin moved by `placement` cells: a cell more
/// on every side, so the shape stays inside.
fn c2_grid(shape: &dyn Shape, h: f64, placement: [f64; 3]) -> Grid {
    let mut grid = Grid::around(shape.bounds().grown(h), h, 4).unwrap();
    grid.origin = grid.origin + (Vec3::from_array(placement) - Vec3::new(0.5, 0.5, 0.5)) * h;
    grid
}

/// C2's measurement for one body: per representation, `(h, error)` at each pitch, the error the
/// root mean square over [`C2_PLACEMENTS`] of the worst channel's phase error, radians.
pub(crate) fn c2_errors(shape: &dyn Shape, exact: &Faceted) -> Vec<(Representation, Vec<(f64, f64)>)> {
    use crate::forward::{Instrument, Quadrature, Reader};
    use crate::voxel::kernel::TreeBody;
    let reader = Reader::new(Instrument {
        quadrature: Quadrature::Gauss { order: 6, panel: 0.4 },
        ..Instrument::aion10()
    });
    let tower = c2_tower(shape);
    let reference = reader.read(tower, &ExactSource(exact.clone(), C2_DENSITY), 10.0);
    let mass = C2_DENSITY * exact.volume();
    let mut out: Vec<(Representation, Vec<(f64, f64)>)> = LADDER.iter().map(|r| (*r, Vec::new())).collect();
    for cells in C2_CELLS {
        let h = 0.8 / (f64::from(cells) + 0.37);
        let mut squares = [0.0f64; 4];
        for placement in C2_PLACEMENTS {
            let grid = c2_grid(shape, h, placement);
            let sampled = crate::voxel::voxelise(shape, &grid, mass, 0).unwrap();
            let cut = voxelise_cut(shape, &grid, Weight::Density(C2_DENSITY), 0, Budget::DEFAULT).unwrap();
            for (i, representation) in LADDER.iter().enumerate() {
                let tree = if *representation == Representation::Sampled { &sampled } else { &cut };
                let read =
                    reader.read(tower, &TreeBody::read_as(tree, *representation, |_| Vec3::ZERO), 10.0);
                squares[i] += worst_channel(read, reference).powi(2);
            }
        }
        for (i, (_, points)) in out.iter_mut().enumerate() {
            points.push((h, (squares[i] / C2_PLACEMENTS.len() as f64).sqrt()));
        }
    }
    out
}

/// C2's bodies: the 0.8 m aligned cube, the same cube turned, an icosahedron about as wide.
fn c2_bodies() -> Vec<(&'static str, Box<dyn Shape>, Faceted)> {
    let cube = Cuboid { centre: Vec3::ZERO, half: Vec3::new(0.4, 0.4, 0.4) };
    let turned = turned_cube(0.8);
    let ico = Platonic::Icosahedron.solid(0.42);
    vec![
        ("aligned cube", Box::new(cube), Faceted::from_cuboid(&cube).unwrap()),
        ("turned cube", Box::new(turned.clone()), Faceted::from_polyhedron(&turned).unwrap()),
        ("icosahedron", Box::new(ico.clone()), Faceted::from_polyhedron(&ico).unwrap()),
    ]
}

/// C2: every body's orders and finest errors, printed, and each criterion of the brief checked:
/// the number of criteria that fail (each named on the error stream).
fn c2_failures() -> f64 {
    let bodies = c2_bodies();
    let all: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = bodies
            .iter()
            .map(|(name, shape, exact)| scope.spawn(move || (*name, c2_errors(shape.as_ref(), exact))))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut failures = Vec::new();
    for (name, rows) in &all {
        let mut finest = Vec::new();
        for (representation, points) in rows {
            let p = order(points);
            let errors: Vec<String> = points.iter().map(|(h, e)| format!("{h:.4}: {e:.2e}")).collect();
            eprintln!("C2 {name} {representation:?}: order {p:.2} ({})", errors.join(", "));
            let (lo, hi) = match (*name, representation) {
                ("aligned cube", Representation::Sampled) => (0.7, 1.3),
                ("aligned cube", Representation::Fill) => (1.7, 2.3),
                ("aligned cube", Representation::Centroid) => (2.7, f64::INFINITY),
                ("aligned cube", Representation::Second) => (3.1, f64::INFINITY),
                (_, Representation::Centroid | Representation::Second) => (2.7, f64::INFINITY),
                _ => (f64::NEG_INFINITY, f64::INFINITY),
            };
            if !(p >= lo && p <= hi) {
                failures.push(format!("{name} {representation:?}: order {p:.2} not in [{lo}, {hi}]"));
            }
            finest.push(points.last().unwrap().1);
        }
        // At the finest pitch: Second < Centroid < Sampled.
        let [sampled, _, centroid, second] = [finest[0], finest[1], finest[2], finest[3]];
        if !(second < centroid && centroid < sampled) {
            failures.push(format!(
                "{name} at the finest: Second {second:e}, Centroid {centroid:e}, Sampled {sampled:e}"
            ));
        }
    }
    for failure in &failures {
        eprintln!("C2 fails: {failure}");
    }
    failures.len() as f64
}

cav_validate::case! {
    test: validation_slow_cut_cell_orders,
    name: "cut.cell_orders",
    subsystem: "voxel",
    slice: "S5",
    reference: ClosedForm,
    against: "the polyhedral closed form's AION-10 read of a 0.8 m aligned cube, the same cube turned and an icosahedron, 1.5 m beyond the surface along (1, 0.2, -0.3), against each representation at 8.37 to 32.37 cells across, RMS over four sub-cell placements: the count of C2's criteria that fail (aligned: Sampled order 1 +- 0.3, Fill 2 +- 0.3, Centroid >= 2.7, Second >= 3.1; turned cube and icosahedron: Centroid and Second >= 2.7; at the finest pitch Second < Centroid < Sampled on each)",
    unit: Count,
    tolerance: 0.0,
    measure: cav_validate::Measured::Error(c2_failures()),
}

/// The clipper's time per boundary cell (`docs/slices/S5.md` §5: over about 10⁶ cells; [X] §7
/// estimated 1e-6 to 1e-5 s): each shape voxelised with cut cells and with samples (`k = 4`), one
/// thread, the time over the cut cells. Release: `cargo test -p cav-core --release --lib
/// clip_time_per_cell -- --ignored --nocapture`.
#[test]
#[ignore = "a timing: run by hand in release"]
fn clip_time_per_cell() {
    let (v, t) = torus_mesh(0.35, 0.15, 128, 64);
    let shapes: Vec<(&str, Box<dyn Shape>, f64)> = vec![
        ("sphere", Box::new(Sphere { centre: Vec3::ZERO, radius: 0.5 }), 1.0 / 500.0),
        ("icosahedron", Box::new(Platonic::Icosahedron.solid(0.6)), 1.0 / 420.0),
        ("turned cube", Box::new(turned_cube(0.8)), 1.0 / 330.0),
        ("torus mesh (16,384 triangles)", Box::new(Mesh::new(v, t).unwrap()), 1.0 / 600.0),
    ];
    for (name, shape, h) in shapes {
        let grid = Grid::around(shape.bounds(), h, 4).unwrap();
        let start = std::time::Instant::now();
        let unbounded = Budget { work: u64::MAX, ..Budget::DEFAULT };
        let cut = voxelise_cut(shape.as_ref(), &grid, Weight::Density(1.0), 0, unbounded).unwrap();
        let cut_time = start.elapsed().as_secs_f64();
        let start = std::time::Instant::now();
        let sampled = crate::voxel::voxelise(shape.as_ref(), &grid, 1.0, 0).unwrap();
        let sampled_time = start.elapsed().as_secs_f64();
        let n = cut.cuts.len() as f64;
        eprintln!(
            "{name}: h {h:.5} m, {} cut cells, cut {cut_time:.2} s ({:.2} us a cut cell), sampled {sampled_time:.2} s \
             ({} leaves), cut tree {} leaves, {:.1} MB",
            cut.cuts.len(),
            cut_time / n * 1e6,
            sampled.leaves.len(),
            cut.leaves.len(),
            cut.bytes() as f64 / 1e6
        );
    }
}

/// How many points the analytic clipper's rule needs: each crossed cell of a ball, at several
/// pitches, by rules of 6 to 24 points against 48, the worst difference of any moment relative to
/// its cell's scale (`h³`, `h⁴`, …). Run by hand: `-- --ignored --nocapture`.
#[test]
#[ignore = "a measurement: run by hand"]
fn analytic_rule_points() {
    let reference = super::analytic::cosine_rule(48);
    let ball = Sphere { centre: Vec3::ZERO, radius: 0.5 };
    for points in [6, 8, 10, 12, 16, 20, 24] {
        let rule = super::analytic::cosine_rule(points);
        let mut worst = 0f64;
        for h in [0.5 / 4.3, 0.5 / 17.7, 0.5 / 63.1] {
            let grid = Grid::around(ball.bounds(), h, 1).unwrap();
            for z in 0..grid.cells[2] {
                for y in 0..grid.cells[1] {
                    for x in 0..grid.cells[0] {
                        let b = grid.block_box([x, y, z], 1);
                        if ball.cover(&b) != Cover::Mixed {
                            continue;
                        }
                        let c = b.centre();
                        let (lo, hi) = (b.lo, b.hi);
                        let a = super::analytic::ball_raw_by(0.5, lo, hi, &rule);
                        let r = super::analytic::ball_raw_by(0.5, lo, hi, &reference);
                        let _ = c;
                        let mut e = (a.volume - r.volume).abs() / h.powi(3);
                        for i in 0..3 {
                            e = e.max((a.first[i] - r.first[i]).abs() / h.powi(4));
                        }
                        for i in 0..6 {
                            e = e.max((a.second[i] - r.second[i]).abs() / h.powi(5));
                        }
                        for i in 0..10 {
                            e = e.max((a.third[i] - r.third[i]).abs() / h.powi(6));
                        }
                        worst = worst.max(e);
                    }
                }
            }
        }
        eprintln!("{points} points: worst {worst:.2e}");
    }
}

/// How C3's geometry is read ([`c3_read`]).
#[derive(Clone, Copy, Debug)]
pub(crate) enum C3Read {
    /// `Pieces`: exact within this many cells of an arm point, `Second` beyond, with or without
    /// the octupole.
    Pieces(f64, bool),
    /// `Second`.
    Second,
}

/// C3's geometry: an icosahedron (edge 0.6 m) at 1/16 m on a lattice placed off the body's box,
/// the tower's line half a cell from its surface (the base below the solid, so the arms rise past
/// it), one AION-10 shot through Gauss order 6. The worst of the lower, upper and gradiometer
/// channels' errors against the polyhedral closed form of the whole solid, relative to each.
pub(crate) fn c3_read(how: C3Read) -> f64 {
    c3_read_with(how, |_, _| {})
}

/// C3's geometries (C3a, C3b: `docs/slices/S5.md` §7.20).
#[derive(Clone, Copy, Debug)]
pub(crate) enum C3Body {
    /// C3's own: an icosahedron (edge 0.6 m), the tower's line half a cell from its surface.
    Icosahedron,
    /// A torus mesh about the z axis (major radius 0.35 m, tube 0.15 m, 24 × 12 quads: not
    /// convex, its pieces signed prisms), the tower's line up through its hole 3 cm off its axis.
    Torus,
}

/// One of C3's geometries at 1/16 m on a lattice placed off the body's box, ready to read: the
/// reader (one AION-10 shot through Gauss order 6), the tower's base (below the solid, so that
/// the arms rise past it), the closed form of the whole solid as a source, the tree and the cut
/// cells' solids.
pub(crate) struct C3 {
    reader: crate::forward::Reader,
    tower: Vec3,
    exact: ExactSource,
    tree: VoxTree,
    solids: Vec<PieceSolid>,
}

pub(crate) fn c3(body: C3Body) -> C3 {
    use crate::forward::{Instrument, Quadrature, Reader};
    let reader = Reader::new(Instrument {
        quadrature: Quadrature::Gauss { order: 6, panel: 0.4 },
        ..Instrument::aion10()
    });
    let h = 1.0 / 16.0;
    let (shape, exact, line): (Box<dyn Shape>, Faceted, Vec3) = match body {
        C3Body::Icosahedron => {
            let ico = Platonic::Icosahedron.solid(0.6);
            let exact = Faceted::from_polyhedron(&ico).unwrap();
            let n = Vec3::new(1.0, 0.23, 0.0).normalised();
            let (mut inside, mut outside) = (0.0, 2.0);
            for _ in 0..80 {
                let mid = 0.5 * (inside + outside);
                if ico.contains(n * mid) { inside = mid } else { outside = mid }
            }
            (Box::new(ico), exact, n * (outside + 0.5 * h))
        }
        C3Body::Torus => {
            let (v, t) = torus_mesh(0.35, 0.15, 24, 12);
            let torus = Mesh::new(v, t).unwrap();
            let exact = Faceted::from_mesh(&torus).unwrap();
            (Box::new(torus), exact, Vec3::new(0.03, 0.01, 0.0))
        }
    };
    let grid = c2_grid(shape.as_ref(), h, [0.31, 0.62, 0.17]);
    let tree = voxelise_cut(shape.as_ref(), &grid, Weight::Density(C2_DENSITY), 0, Budget::DEFAULT).unwrap();
    let solids = piece_solids(shape.as_ref(), &tree).unwrap();
    C3 { reader, tower: Vec3::new(line.x, line.y, -0.6), exact: ExactSource(exact, C2_DENSITY), tree, solids }
}

/// The worst of the lower, upper and gradiometer channels' relative errors of `r` against
/// `reference`.
fn c3_channels(r: crate::forward::Reading, reference: crate::forward::Reading) -> f64 {
    let g = |x: crate::forward::Reading| x.lower - x.upper;
    ((r.lower - reference.lower) / reference.lower)
        .abs()
        .max(((r.upper - reference.upper) / reference.upper).abs())
        .max(((g(r) - g(reference)) / g(reference)).abs())
}

/// [`c3_read`] with the cut cells' solids changed by `change` first, given the tower's base (for
/// C3a's control).
fn c3_read_with(how: C3Read, change: impl FnOnce(&mut Vec<PieceSolid>, Vec3)) -> f64 {
    use crate::voxel::kernel::TreeBody;
    let C3 { reader, tower, exact, tree, mut solids } = c3(C3Body::Icosahedron);
    change(&mut solids, tower);
    let reference = reader.read(tower, &exact, 10.0);
    let origin: fn(f64) -> Vec3 = |_| Vec3::ZERO;
    let body = match how {
        C3Read::Pieces(cells, octupole) => {
            TreeBody::with_pieces_within(&tree, solids, cells, octupole, origin)
        }
        C3Read::Second => TreeBody::read_as(&tree, Representation::Second, origin),
    };
    let e = c3_channels(reader.read(tower, &body, 10.0), reference);
    eprintln!("C3 geometry, {how:?}: {e:.3e} relative ({} cut cells)", tree.cuts.len());
    e
}

cav_validate::case! {
    test: validation_pieces_everywhere_are_the_body,
    name: "cut.pieces_everywhere_are_the_body",
    subsystem: "voxel",
    slice: "S5",
    reference: ClosedForm,
    against: "C3a (the owner's ruling of 28 Nov 2026, docs/slices/S5.md 7.20): the polyhedral closed form of an icosahedron (edge 0.6 m), the tower's line half a cell from its surface, one AION-10 shot through Gauss order 6: voxelised at 1/16 m with every cut cell read as its exact piece by the polyhedral kernel and the interior by the cube kernel, the worst of the lower, upper and gradiometer channels, relative",
    unit: Relative,
    tolerance: 1e-11,
    measure: cav_validate::Measured::Error({
        let exact = c3_read(C3Read::Pieces(f64::INFINITY, false));
        // Controls: without the exact pieces it is far off; and with one piece (the cut cell
        // nearest the tower's line) moved by a micrometre it fails, so the measure sees one piece.
        let second = c3_read(C3Read::Second);
        assert!(second > 1e-11, "Second alone is {second:e}");
        let moved = c3_read_with(C3Read::Pieces(f64::INFINITY, false), |solids, tower| {
            let line = |s: &PieceSolid| (s.centre.x - tower.x).hypot(s.centre.y - tower.y);
            let nearest = (0..solids.len()).min_by(|&a, &b| line(&solids[a]).total_cmp(&line(&solids[b]))).unwrap();
            solids[nearest].centre = solids[nearest].centre + Vec3::new(1e-6, 0.0, 0.0);
        });
        assert!(moved > 1e-11, "one piece moved by 1 um reads {moved:e}");
        exact
    }),
}

/// What C3b measures of one geometry: the production `Pieces` read against the closed form of
/// the whole solid and against its own ledger ([`crate::voxel::kernel::TreeBody::ledger`]).
#[derive(Clone, Copy, Debug)]
pub(crate) struct C3b {
    /// The worst, over every arm point the read evaluates, of the body's error over its ledger
    /// plus the reference's rounding allowance ([`C3B_ROUNDING`] of the exact difference).
    pub body: f64,
    /// The worst of each interferometer's phase error over its ledger read through the flight,
    /// and of the gradiometer's over the two ledgers added.
    pub phase: f64,
    /// The worst, over every cut cell read through its moments and every arm point, of the
    /// cell's error over its own ledger term plus [`piece_rounding`].
    pub cells: f64,
    /// The control: the same for the one cell whose error stands farthest above its rounding
    /// allowance, with its term left out of the ledger (must exceed 1).
    pub left_out: f64,
    /// The relative error of the read, the worst channel (C3 as first written measured this).
    pub relative: f64,
    /// The cut cells.
    pub cut_cells: usize,
}

/// The rounding allowed the closed form of the whole solid in C3b's body check, relative to its
/// difference (C3a measures the read's at 3.3e-15).
pub(crate) const C3B_ROUNDING: f64 = 1e-12;

/// How far from a far cell, in cell diagonals, C3b checks its error against its own term.
const C3B_CELL_REACH: f64 = 10.0;

/// P8's rounding law's constant (`oracle/tests/polyhedral.rs`, `docs/slices/S5.md` §7.19): the
/// closed form's potential is within `c ε (R/s)²` of itself.
const ROUNDING_LAW: f64 = 2000.0;

/// The rounding allowed one piece's closed form at an arm point pair, kg/m: each of its solids'
/// two potentials within P8's law, `2 c ε (R/s)² ρ|v|/R'`, with `R` the nearer point's distance
/// from the cell's centre plus the solid's half-size, `R'` less it (so `|V| ≤ ρ|v|/R'`), `s` the
/// solid's size. (The first allowance, `100 ε (R/s)² (R/gap)` of the piece's difference, scaled
/// with the difference, which a piece level with the point makes far smaller than its
/// potentials: §7.20.)
fn piece_rounding(solid: &PieceSolid, density: f64, distance: f64) -> f64 {
    solid
        .parts
        .iter()
        .map(|(_, part)| {
            let size = part.size();
            let (far, near) = (distance + 0.5 * size, (distance - 0.5 * size).max(1e-300));
            2.0 * ROUNDING_LAW * f64::EPSILON * (far / size).powi(2) * density * part.volume().abs() / near
        })
        .sum()
}

/// A source that, at every arm point pair the reader asks for, reads the `Pieces` body, its
/// ledger and the exact solid, and keeps the worst ratios C3b holds below 1.
struct C3bCheck<'a, P: Fn(f64) -> Vec3 + Sync> {
    body: &'a crate::voxel::kernel::TreeBody<P>,
    exact: &'a ExactSource,
    /// The cut cells' solids, in the tree's order, and their density (kg/m³).
    solids: &'a [PieceSolid],
    density: f64,
    /// A cell's diagonal, metres.
    diagonal: f64,
    /// `[body, cells]` worst ratios, and each cell's worst error over its rounding allowance.
    worst: std::sync::Mutex<([f64; 2], Vec<f64>)>,
}

impl<P: Fn(f64) -> Vec3 + Sync> crate::forward::Source for C3bCheck<'_, P> {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, time: f64) -> [f64; 2] {
        let read = self.body.difference(tower, heights, lower, upper, time);
        let ledger = self
            .body
            .ledger()
            .expect("a Pieces body has a ledger")
            .difference(tower, heights, lower, upper, time);
        let exact = self.exact.difference(tower, heights, lower, upper, time);
        let mut worst = self.worst.lock().unwrap();
        for i in 0..2 {
            let allowed = ledger[i] + C3B_ROUNDING * exact[i].abs();
            let excess = (read[i] - exact[i]).abs();
            if excess > 0.0 {
                worst.0[0] = worst.0[0].max(excess / allowed);
            }
            for k in 0..self.body.pieces() {
                let (by_moments, piece, bound) =
                    self.body.piece_terms(k, tower, heights[i], lower, upper, time);
                let Some(bound) = bound else { continue };
                let error = (by_moments - piece).abs();
                let at = self.body.piece_distance(k, tower, heights[i], lower, upper, time);
                // Within C3B_CELL_REACH cell diagonals only: beyond, the piece's closed form
                // rounds above its term, and the body's check covers it (`docs/slices/S5.md` §7.20).
                if at > C3B_CELL_REACH * self.diagonal {
                    continue;
                }
                let rounding = piece_rounding(&self.solids[k], self.density, at);
                if error > 0.0 {
                    worst.0[1] = worst.0[1].max(error / (bound + rounding));
                    worst.1[k] = worst.1[k].max(error / rounding);
                }
            }
        }
        read
    }
}

/// C3b on one geometry (`docs/slices/S5.md` §7.20): the production `Pieces` read (exact pieces
/// within `PIECE_WITHIN` cells of an arm, `Second` and the octupole beyond) against the closed
/// form of the whole solid and against its ledger, at every arm point the read evaluates and in
/// each phase; each far cell against its own term; and the control with one cell's term left out.
pub(crate) fn c3b(geometry: C3Body) -> C3b {
    use crate::voxel::kernel::TreeBody;
    let C3 { reader, tower, exact, tree, solids } = c3(geometry);
    let cut_cells = tree.cuts.len();
    let body = TreeBody::with_pieces(&tree, solids.clone(), |_| Vec3::ZERO);
    let check = C3bCheck {
        body: &body,
        exact: &exact,
        solids: &solids,
        density: tree.density,
        diagonal: tree.side(0) * 3f64.sqrt(),
        worst: std::sync::Mutex::new(([0.0; 2], vec![0.0; body.pieces()])),
    };
    let read = reader.read(tower, &check, 10.0);
    let reference = reader.read(tower, &exact, 10.0);
    let ledger = reader.read(tower, &body.ledger().unwrap(), 10.0);
    let (lower, upper) = (ledger.lower.abs(), ledger.upper.abs());
    let phase = ((read.lower - reference.lower).abs() / lower)
        .max((read.upper - reference.upper).abs() / upper)
        .max(((read.lower - read.upper) - (reference.lower - reference.upper)).abs() / (lower + upper));
    let (worst, per_cell) = check.worst.into_inner().unwrap();
    // The control: the cell whose error stands farthest above its rounding allowance, its term
    // left out of the ledger. Its error over its allowance alone is what the check then sees.
    let (k, left_out) =
        per_cell.iter().copied().enumerate().fold((0, 0.0), |a, (k, r)| if r > a.1 { (k, r) } else { a });
    // The body's ledger with that cell left out still covers the body's error or not: recorded.
    let without = reader.read(tower, &body.ledger().unwrap().leaving_out(k), 10.0);
    eprintln!(
        "C3b {geometry:?}: {cut_cells} cut cells; body/ledger {:.3e}, phase/ledger {phase:.3e}, cell/term {:.3e}; \
         control cell {k}: error/rounding {left_out:.3e}; ledger lower {lower:.3e} upper {upper:.3e} rad \
         (without cell {k}: {:.3e}, {:.3e}); errors lower {:.3e} upper {:.3e} rad",
        worst[0],
        worst[1],
        without.lower.abs(),
        without.upper.abs(),
        (read.lower - reference.lower).abs(),
        (read.upper - reference.upper).abs(),
    );
    C3b {
        body: worst[0],
        phase,
        cells: worst[1],
        left_out,
        relative: c3_channels(read, reference),
        cut_cells,
    }
}

cav_validate::case! {
    test: validation_pieces_are_the_body,
    name: "cut.pieces_are_the_body",
    subsystem: "voxel",
    slice: "S5",
    reference: ClosedForm,
    against: "C3b (C3 restated by the owner's ruling of 28 Nov 2026, docs/slices/S5.md 7.20; as first written, within 1e-11 relative, it measured 1.34e-6): the engine's Pieces read (exact pieces within PIECE_WITHIN = 3 cells of an arm point, Second and the octupole beyond) of an icosahedron (edge 0.6 m, the tower's line half a cell from its surface) and of a torus mesh (the line up its hole), at 1/16 m, one AION-10 shot through Gauss order 6, against the polyhedral closed form of the whole solid: the largest of its error over its own ledger (TreeBody::ledger, the far cells' left-out orders, plus 1e-12 of the reference for its rounding) at every arm point the read evaluates, in each interferometer's phase and the gradiometer's, and of each far cell's error over its own term (plus its closed form's rounding) at every arm point. Control: the cell farthest above its rounding allowance, its term left out, fails",
    unit: Relative,
    tolerance: 1.0,
    measure: cav_validate::Measured::Error({
        let mut worst = 0f64;
        for geometry in [C3Body::Icosahedron, C3Body::Torus] {
            let m = c3b(geometry);
            assert!(m.left_out > 1.0, "{geometry:?}: a cell's term left out is not missed ({:e})", m.left_out);
            println!("C3b {geometry:?}: {} cut cells, {:.2e} relative (recorded)", m.cut_cells, m.relative);
            worst = worst.max(m.body).max(m.phase).max(m.cells);
        }
        worst
    }),
}

/// How far `Pieces` must reach, and what the octupole beyond buys (TASK-S5-10, §7.7; the
/// question TASK-S5-26 puts to the owner). Run by hand: `-- --ignored --nocapture`.
#[test]
#[ignore = "a measurement: run by hand"]
fn pieces_by_reach() {
    // `Second` (the `Centroid` cube within three cells) against the bare quadrupole everywhere,
    // which is `Pieces` with no reach and no octupole.
    c3_read(C3Read::Second);
    for octupole in [false, true] {
        for cells in [0.0, 3.0, 6.0, 12.0, 24.0, 48.0, f64::INFINITY] {
            c3_read(C3Read::Pieces(cells, octupole));
        }
    }
}

#[test]
fn pieces_are_the_cells_pieces() {
    // Each cell's solids weigh what its piece does, for a polyhedron and a mesh; a curved solid
    // has none, in words; the cap is counted first.
    let ico = Platonic::Icosahedron.solid(0.6);
    let (v, t) = torus_mesh(0.35, 0.15, 24, 12);
    let torus = Mesh::new(v, t).unwrap();
    for shape in [&ico as &dyn Shape, &torus] {
        let grid = Grid::around(shape.bounds(), 0.6 / 9.3, 2).unwrap();
        let tree = voxelise_cut(shape, &grid, Weight::Density(1.0), 0, Budget::DEFAULT).unwrap();
        let solids = piece_solids(shape, &tree).unwrap();
        let mut worst = 0f64;
        for (cut, solid) in tree.cuts.iter().zip(&solids) {
            let v: f64 = solid.parts.iter().map(|(sign, f)| sign * f.volume()).sum();
            worst = worst.max((v - cut.piece.volume).abs() / grid.pitch.powi(3));
        }
        assert!(worst < 1e-13, "{worst:e}");
    }
    let sphere = Sphere { centre: Vec3::ZERO, radius: 0.3 };
    let grid = Grid::around(sphere.bounds(), 0.1, 2).unwrap();
    let tree = voxelise_cut(&sphere, &grid, Weight::Density(1.0), 0, Budget::DEFAULT).unwrap();
    assert!(piece_solids(&sphere, &tree).unwrap_err().0.contains("polyhedron or a mesh"));
}

/// 26 directions from a fixed generator (SplitMix64), uniform over the sphere, each kept only when
/// a tower's line from `n·(radius + 1.5)` upward misses the ball by 0.25 m or more: the analytic
/// reference (a point outside the ball) does not hold on a line through it ([X] §3.1's "26 random
/// tower directions"; the 26 to a cube's faces, edges and corners were the first choice, and the
/// six beneath the ball put the arms through it).
fn twenty_six(radius: f64) -> Vec<Vec3> {
    let mut state = 2626u64;
    let mut next = || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut out = Vec::new();
    while out.len() < 26 {
        let (u, v) = (2.0 * next() - 1.0, 2.0 * std::f64::consts::PI * next());
        let s = (1.0 - u * u).sqrt();
        let n = Vec3::new(s * v.cos(), s * v.sin(), u);
        let base = n * (radius + 1.5);
        let across = (base.x * base.x + base.y * base.y).sqrt();
        if across >= radius + 0.25 || base.z >= radius + 0.25 {
            out.push(n);
        }
    }
    out
}

/// C6's measurement: for each size/h of 8, 16 and 32, the RMS over 8 random lattice placements and
/// 26 directions of `Centroid`'s and `Second`'s gradiometer phase error against the analytic
/// sphere (a point mass outside itself), the tower's base 1.5 m beyond the surface, radians.
/// `centred`: on the engine's own grid (centred on the ball, the near-symmetric placement) in
/// place of the eight.
fn c6_rms(centred: bool) -> Vec<(f64, f64, f64)> {
    use crate::forward::{Instrument, Mass, Quadrature, Reader};
    use crate::voxel::kernel::TreeBody;
    let reader = Reader::new(Instrument {
        quadrature: Quadrature::Gauss { order: 6, panel: 0.4 },
        ..Instrument::aion10()
    });
    let radius = 0.5;
    let sphere = Sphere { centre: Vec3::ZERO, radius };
    let mass = C2_DENSITY * sphere.volume();
    let directions = twenty_six(radius);
    let towers: Vec<Vec3> = directions.iter().map(|n| *n * (radius + 1.5)).collect();
    let point = Mass::point(mass, |_| Vec3::ZERO);
    let exact: Vec<_> = towers.iter().map(|t| reader.read(*t, &point, 10.0)).collect();
    // Eight placements from a fixed generator (SplitMix64), each a sub-cell offset along x, y, z.
    let mut state = 26u64;
    let mut next = || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    };
    let placements: Vec<[f64; 3]> = (0..8).map(|_| [next(), next(), next()]).collect();
    [8u32, 16, 32]
        .into_iter()
        .map(|cells| {
            let h = 2.0 * radius / f64::from(cells);
            let mut sums = [0.0f64; 2];
            let mut n = 0.0;
            let grids: Vec<Grid> = if centred {
                vec![Grid::around(sphere.bounds(), h, 4).unwrap()]
            } else {
                placements.iter().map(|p| c2_grid(&sphere, h, *p)).collect()
            };
            // One thread a grid, joined in order, so the sums are the same bits however they ran.
            let parts: Vec<([f64; 2], f64)> = std::thread::scope(|scope| {
                let handles: Vec<_> = grids
                    .iter()
                    .map(|grid| {
                        let (reader, towers, exact, sphere) = (&reader, &towers, &exact, &sphere);
                        scope.spawn(move || {
                            let tree =
                                voxelise_cut(sphere, grid, Weight::Density(C2_DENSITY), 0, Budget::DEFAULT)
                                    .unwrap();
                            let mut sums = [0.0f64; 2];
                            for (tower, exact) in towers.iter().zip(exact) {
                                for (i, representation) in
                                    [Representation::Centroid, Representation::Second].iter().enumerate()
                                {
                                    let body = TreeBody::read_as(&tree, *representation, |_| Vec3::ZERO);
                                    let read = reader.read(*tower, &body, 10.0);
                                    let e = (read.lower - read.upper) - (exact.lower - exact.upper);
                                    sums[i] += e * e;
                                }
                            }
                            (sums, towers.len() as f64)
                        })
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            for (part, count) in parts {
                sums[0] += part[0];
                sums[1] += part[1];
                n += count;
            }
            (h, (sums[0] / n).sqrt(), (sums[1] / n).sqrt())
        })
        .collect()
}

/// C6, the referee's S1 (recorded, not gated): did the paper's sphere anomaly (`Second` no better
/// than `Centroid` on a sphere, [X] §3.1) come from the near-symmetric placement? Written before
/// measuring: if `Second` beats `Centroid` at two of the three pitches or more, it did.
#[test]
fn slow_the_sphere_anomaly_with_random_placements() {
    for (h, centroid, second) in c6_rms(true) {
        eprintln!("C6 centred grid, h {h:.4} m: Centroid RMS {centroid:.3e} rad, Second {second:.3e} rad");
    }
    let rows = c6_rms(false);
    let mut wins = 0;
    for (h, centroid, second) in &rows {
        eprintln!("C6 h {h:.4} m: Centroid RMS {centroid:.3e} rad, Second {second:.3e} rad");
        wins += usize::from(second < centroid);
    }
    eprintln!(
        "C6: Second beats Centroid at {wins} of 3 pitches: {}",
        if wins >= 2 {
            "the anomaly was the near-symmetric placement"
        } else {
            "the anomaly stands with random placements"
        }
    );
    // Recorded, not gated: only that each number is a number.
    assert!(rows.iter().all(|(_, a, b)| a.is_finite() && b.is_finite()));
}

/// The volume and first moments of a closed surface's triangles (anticlockwise seen from
/// outside), by the divergence theorem: `Σ a·(b × c)/6` and `Σ (a + b + c) a·(b × c)/24`.
fn surface_moments(triangles: &[[Vec3; 3]]) -> (f64, Vec3) {
    triangles.iter().fold((0.0, Vec3::ZERO), |(v, m), [a, b, c]| {
        let six = a.dot(b.cross(*c));
        (v + six / 6.0, m + (*a + *b + *c) * (six / 24.0))
    })
}

/// The worst, over the cut cells of `shape` at `pitch`, of a piece surface's volume and first
/// moment against the piece's (each over a cell's volume, the moment also over its side), with the
/// surfaces made by `make`.
fn piece_surface_error(shape: &dyn Shape, pitch: f64, origin: Option<Vec3>) -> (f64, usize) {
    let grid = match origin {
        None => Grid::around(shape.bounds(), pitch, 2).unwrap(),
        Some(lo) => {
            let mut g = Grid::around(shape.bounds(), pitch, 2).unwrap();
            g.origin = lo;
            g
        }
    };
    let tree = voxelise_cut(shape, &grid, Weight::Density(1.0), 0, Budget::DEFAULT).unwrap();
    let all: Vec<usize> = (0..tree.cuts.len()).collect();
    let surfaces = piece_surfaces(shape, &tree, &all, usize::MAX).unwrap();
    let cell = pitch.powi(3);
    let mut worst = 0f64;
    for (cut, s) in tree.cuts.iter().zip(&surfaces) {
        assert_eq!(cut.key, s.key);
        let (v, m) = surface_moments(&s.triangles);
        let centroid = s.centre + m * (1.0 / v);
        worst = worst
            .max((v - cut.piece.volume).abs() / cell)
            .max((centroid - cut.piece.centroid).length() * cut.piece.volume / (cell * pitch));
    }
    (worst, tree.cuts.len())
}

#[test]
fn a_piece_s_surface_is_its_piece() {
    // D1's pieces (criterion in `docs/slices/S5.md` §7.13, written before measuring): each cut
    // cell's drawn surface encloses its piece: volume and first moment within 1e-12 of a cell's,
    // for an icosahedron (one convex piece a cell), a torus mesh (the mesh clipped and its caps)
    // and a box mesh whose faces lie on the lattice's planes (faces on the cells' walls).
    let ico = Platonic::Icosahedron.solid(0.6);
    let (v, t) = torus_mesh(0.35, 0.15, 24, 12);
    let torus = Mesh::new(v, t).unwrap();
    let (v, t) = box_mesh(Vec3::new(-0.3, -0.2, -0.15), Vec3::new(0.3, 0.2, 0.15));
    let block = Mesh::new(v, t).unwrap();
    for (name, shape, origin) in [
        ("icosahedron", &ico as &dyn Shape, None),
        ("torus", &torus, None),
        // A grid on which the box's faces at x = ±0.3 and z = ±0.15 lie on cell walls and those at
        // y = ±0.2 cut through cells.
        ("aligned box", &block, Some(Vec3::new(-0.35, -0.375, -0.2))),
    ] {
        let (worst, cells) = piece_surface_error(shape, 0.05, origin);
        eprintln!("{name}: {cells} cut cells, worst {worst:.2e} of a cell");
        assert!(cells > 20, "{name}: {cells}");
        assert!(worst <= 1e-12, "{name}: {worst:e}");
    }
    // Control: a mesh piece's surface without its caps is not closed: its volume is off.
    let grid = Grid::around(torus.bounds(), 0.05, 2).unwrap();
    let tree = voxelise_cut(&torus, &grid, Weight::Density(1.0), 0, Budget::DEFAULT).unwrap();
    let clipper = Clip::prepare(&torus, &grid).unwrap();
    let mut off = 0f64;
    for cut in &tree.cuts {
        let corner = morton::decode(cut.key);
        let b = grid.block_box(corner, 1);
        let all = clipper.surface(corner, &b).unwrap();
        // The caps are the triangles lying in a wall of the cell.
        let h = 0.5 * grid.pitch;
        let in_wall = |t: &[Vec3; 3]| {
            (0..3).any(|a| [-h, h].iter().any(|w| t.iter().all(|p| (p.to_array()[a] - w).abs() < 1e-12)))
        };
        let skin: Vec<[Vec3; 3]> = all.into_iter().filter(|t| !in_wall(t)).collect();
        off = off.max((surface_moments(&skin).0 - cut.piece.volume).abs() / grid.pitch.powi(3));
    }
    assert!(off > 1e-2, "without its caps a piece is off by {off:e} only");
    // A curved solid's pieces are not polyhedra: refused in words. The cap is counted first.
    let sphere = Sphere { centre: Vec3::ZERO, radius: 0.3 };
    let grid = Grid::around(sphere.bounds(), 0.1, 2).unwrap();
    let tree = voxelise_cut(&sphere, &grid, Weight::Density(1.0), 0, Budget::DEFAULT).unwrap();
    assert!(piece_surfaces(&sphere, &tree, &[0], usize::MAX).unwrap_err().0.contains("not polyhedra"));
    let grid = Grid::around(ico.bounds(), 0.05, 2).unwrap();
    let tree = voxelise_cut(&ico, &grid, Weight::Density(1.0), 0, Budget::DEFAULT).unwrap();
    let all: Vec<usize> = (0..tree.cuts.len()).collect();
    let refused = piece_surfaces(&ico, &tree, &all, 30).unwrap_err().0;
    assert!(refused.contains("more than 30 triangles"), "{refused}");
}
