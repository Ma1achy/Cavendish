//! Bodies made of voxels, held to closed forms at sizes too slow for `quick`, and to George's
//! scripts (`VALIDATION-AND-RESEARCH.md` §2.3; the S3 brief, §3.3, R5, R9, R12).

use cav_core::Vec3;
use cav_core::voxel::shape::{Cuboid, Cylinder, Ellipsoid, Platonic, Shape, Sphere};
use cav_core::voxel::{Aabb, Grid, voxelise};

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

/// One shape's volume and inertia errors at `cells` finest cells across its longest side, k = 4,
/// its grid shifted by `shift` (metres): `[volume, inertia]` (relative; the inertia's largest
/// element error over the largest principal moment), and the envelopes S3.md §8.1 derives for
/// them, `√3·A·δ/V` and `2·M·r²_max` times that over `I_max` (A the shape's surface; δ = h/k).
fn errors(name: &str, shape: &dyn Shape, cells: f64, shift: Vec3) -> ([f64; 2], [f64; 2]) {
    let b = shape.bounds();
    let size = b.size();
    let pitch = size.x.max(size.y).max(size.z) / cells;
    let pad = Vec3::new(pitch, pitch, pitch);
    let grid = Grid::around(Aabb::new(b.lo - pad + shift, b.hi + pad + shift), pitch, 4).unwrap();
    let tree = voxelise(shape, &grid, 1.0, 0).unwrap();
    let voxels = tree.samples as f64 / f64::from(grid.samples_per_cell()) * pitch.powi(3);
    let volume = (voxels / shape.volume() - 1.0).abs();
    let got = tree.body().inertia();
    let s = shape.second_moments().map(|v| v / shape.volume());
    let want =
        cav_core::voxel::Moments { mass: 1.0, centre: Vec3::ZERO, second: s, third: [0.0; 10] }.inertia();
    let largest = want[0].max(want[1]).max(want[2]);
    let inertia = (0..6).map(|i| (got[i] - want[i]).abs() / largest).fold(0f64, f64::max);
    let shell = 3f64.sqrt() * surface(name) * (pitch / 4.0) / shape.volume();
    let r_max = 0.5 * size.length();
    ([volume, inertia], [shell, 2.0 * r_max * r_max * shell / largest])
}

/// The surface area of a test shape by its name, square metres: closed forms (the ellipsoid by
/// Thomsen's formula, within 1.1% of the true area, taken 1.1% up so that it bounds it).
fn surface(name: &str) -> f64 {
    use std::f64::consts::PI;
    let edge: f64 = 0.6;
    match name {
        "sphere" => 4.0 * PI * 0.25,
        "shell" => 4.0 * PI * (0.25 + 0.09),
        "cuboid" => 2.0 * (1.0 * 0.6 + 0.6 * 0.4 + 0.4 * 1.0),
        "ellipsoid" => {
            let (a, b, c, p) = (0.5f64, 0.35f64, 0.25f64, 1.6075);
            let mean = ((a * b).powf(p) + (b * c).powf(p) + (c * a).powf(p)) / 3.0;
            1.011 * 4.0 * PI * mean.powf(1.0 / p)
        }
        "cylinder" => 2.0 * PI * 0.3 * (0.3 + 1.0),
        "Tetrahedron" => 3f64.sqrt() * edge * edge,
        "Cube" => 6.0 * edge * edge,
        "Octahedron" => 2.0 * 3f64.sqrt() * edge * edge,
        "Dodecahedron" => 3.0 * (25.0 + 10.0 * 5f64.sqrt()).sqrt() * edge * edge,
        "Icosahedron" => 5.0 * 3f64.sqrt() * edge * edge,
        other => panic!("no area for {other}"),
    }
}

/// A small deterministic generator of sub-cell shifts (SplitMix64), in [0, 1).
fn numbers(seed: u64) -> impl FnMut() -> f64 {
    let mut state = seed;
    move || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[test]
fn volume_and_inertia_converge_with_the_pitch() {
    // S3.md §8.1 (finding 5 of S3's review), written before this was run. The faceted shapes
    // (flat faces at an arbitrary lattice phase: the cuboid, the cylinder's ends, the Platonic
    // solids) are held to their first-order envelopes at every size; the curved ones converge at
    // order 1.5 at least, root mean square over eight random sub-cell placements.
    for (name, shape) in solids() {
        let curved = name == "sphere" || name == "ellipsoid";
        let size = shape.bounds().size();
        let mut rms = Vec::new();
        for cells in [32.0, 64.0, 128.0] {
            let pitch = size.x.max(size.y).max(size.z) / cells;
            let mut next = numbers(cells as u64);
            let (mut squares, mut worst) = ([0f64; 2], [0f64; 2]);
            for _ in 0..8 {
                let shift = Vec3::new(next(), next(), next()) * pitch;
                let (e, envelope) = errors(&name, shape.as_ref(), cells, shift);
                for i in 0..2 {
                    squares[i] += e[i] * e[i] / 8.0;
                    worst[i] = worst[i].max(e[i] / envelope[i]);
                }
            }
            let root = squares.map(f64::sqrt);
            eprintln!(
                "{name:>12} {cells:>4} across: RMS volume {:.3e}, inertia {:.3e}; worst over its envelope {:.3}, {:.3}",
                root[0], root[1], worst[0], worst[1]
            );
            assert!(worst[0] <= 1.0 && worst[1] <= 1.0, "{name} at {cells}: outside its envelope {worst:?}");
            rms.push(root);
        }
        if curved {
            for i in 0..2 {
                let order = (rms[0][i] / rms[2][i]).ln() / 4f64.ln();
                eprintln!("{name:>12}: order 32 → 128 {} {order:.2}", ["volume", "inertia"][i]);
                assert!(
                    rms[2][i] * 8.0 <= rms[0][i],
                    "{name}: {} converges at order {order:.2}, under 1.5",
                    ["volume", "inertia"][i]
                );
            }
        }
    }
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
