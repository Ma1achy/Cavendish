//! Bodies made of voxels, held to closed forms at sizes too slow for `quick`, and to George's
//! scripts (`VALIDATION-AND-RESEARCH.md` §2.3; the S3 brief, §3.3, R5, R9, R12).

use cav_core::Vec3;
use cav_core::voxel::shape::{Cuboid, Cylinder, Ellipsoid, Platonic, Shape, Sphere};
use cav_core::voxel::{Grid, voxelise};

/// The primitives and the Platonic solids, about a metre across, centred on the origin.
fn solids() -> Vec<(String, Box<dyn Shape>)> {
    let mut out: Vec<(String, Box<dyn Shape>)> = vec![
        ("sphere".into(), Box::new(Sphere { centre: Vec3::ZERO, radius: 0.5 })),
        ("cuboid".into(), Box::new(Cuboid { centre: Vec3::ZERO, half: Vec3::new(0.5, 0.3, 0.2) })),
        ("ellipsoid".into(), Box::new(Ellipsoid { centre: Vec3::ZERO, radii: Vec3::new(0.5, 0.35, 0.25) })),
        (
            "cylinder".into(),
            Box::new(Cylinder { centre: Vec3::ZERO, radius: 0.3, half_length: 0.5, axis: 2 }),
        ),
    ];
    for solid in Platonic::ALL {
        out.push((format!("{solid:?}"), Box::new(solid.solid(0.6))));
    }
    out
}

/// The worst volume error and inertia error (as a fraction of the largest principal moment) over
/// the solids, at `cells` across the longest side, k = 4.
fn errors(cells: f64) -> (f64, f64) {
    let (mut volume, mut inertia) = (0f64, 0f64);
    for (_name, shape) in solids() {
        let size = shape.bounds().size();
        let pitch = size.x.max(size.y).max(size.z) / cells;
        let grid = Grid::around(shape.bounds(), pitch, 4).unwrap();
        let tree = voxelise(shape.as_ref(), &grid, 1.0, 0).unwrap();
        let voxels = tree.samples as f64 / f64::from(grid.samples_per_cell()) * pitch.powi(3);
        volume = volume.max((voxels / shape.volume() - 1.0).abs());
        if std::env::var_os("CAV_SHOW").is_some() {
            eprintln!("  {cells} {}: volume {:.3e}", _name, (voxels / shape.volume() - 1.0).abs());
        }
        let got = tree.body().inertia();
        let s = shape.second_moments().map(|v| v / shape.volume());
        let want =
            cav_core::voxel::Moments { mass: 1.0, centre: Vec3::ZERO, second: s, third: [0.0; 10] }.inertia();
        let largest = want[0].max(want[1]).max(want[2]);
        for i in 0..6 {
            inertia = inertia.max((got[i] - want[i]).abs() / largest);
        }
    }
    (volume, inertia)
}

#[test]
fn volume_and_inertia_converge_with_the_pitch() {
    let points: Vec<(f64, (f64, f64))> = [32.0, 64.0, 128.0].into_iter().map(|c| (c, errors(c))).collect();
    for (cells, (v, i)) in &points {
        eprintln!("{cells} across: volume {v:.3e}, inertia {i:.3e}");
    }
    let (coarse, fine) = (points[0].1, points[2].1);
    // The brief's criterion (S3.md §3.3, R5): at least three times smaller at 128 than at 32.
    assert!(fine.0 * 3.0 <= coarse.0, "volume: {coarse:?} to {fine:?}");
    assert!(fine.1 * 3.0 <= coarse.1, "inertia: {coarse:?} to {fine:?}");
    let order = |a: f64, b: f64| (a / b).ln() / 4f64.ln();
    eprintln!(
        "measured order 32 → 128: volume {:.2}, inertia {:.2}",
        order(coarse.0, fine.0),
        order(coarse.1, fine.1)
    );
}

/// The worst relative difference over one shot's three channels between a voxel sphere at
/// `pitch` (r = 0.5 m, 100 kg, k = 4) and the analytic sphere, a tower 1.6 m off its axis.
fn sphere_against_point(pitch: f64) -> f64 {
    use cav_core::forward::{Instrument, Mass, PHASE_TOLERANCE, Reader};
    use cav_core::voxel::kernel::TreeBody;
    let at = Vec3::new(1.4, 0.6, 2.2);
    let shape = Sphere { centre: Vec3::ZERO, radius: 0.5 };
    let grid = Grid::around(shape.bounds(), pitch, 4).unwrap();
    let tree = voxelise(&shape, &grid, 100.0, 0).unwrap();
    let reader = Reader::new(Instrument::aion10());
    let voxels = reader.read_to(Vec3::ZERO, &TreeBody::new(&tree, |_| at), 1.46, PHASE_TOLERANCE).reading;
    let exact = reader.read_to(Vec3::ZERO, &Mass::point(100.0, |_| at), 1.46, PHASE_TOLERANCE).reading;
    let relative = |a: f64, b: f64| (a - b).abs() / b.abs();
    relative(voxels.lower, exact.lower)
        .max(relative(voxels.upper, exact.upper))
        .max(relative(voxels.lower - voxels.upper, exact.lower - exact.upper))
}

#[test]
fn a_finer_voxel_sphere_is_nearer_the_sphere() {
    let (coarse, fine) = (sphere_against_point(0.5 / 16.0), sphere_against_point(0.5 / 32.0));
    eprintln!("r/16: {coarse:.3e}, r/32: {fine:.3e}");
    // S3.md §3.3, R9: within 1e-3 at r/32.
    assert!(fine <= 1e-3, "{fine}");
}
