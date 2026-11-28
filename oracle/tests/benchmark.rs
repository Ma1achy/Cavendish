//! The paper's benchmark read by Cavendish (`docs/papers/polyhedral-gravity`, Table "phase error at
//! h = 1/64 m" and Figure "against pitch"; `docs/slices/S5.md` §3's C2 extended to the paper's five
//! bodies, the section that records TASK-S5-18).
//!
//! The five bodies are the paper's (their triangles from `data/esa_fixtures.json`, in
//! `fixtures/paper/polyhedral/<body>/triangles.npy`, ρ = 2700 kg/m³): the 0.8 m cube with its faces
//! on lattice planes, the same cube turned, an icosahedron, a 1 m × 1 m × 5 cm plate turned, and a
//! 1280-face icosphere of radius 0.5 m. Each is voxelised by `cav_core` as a mesh at h = 1/8, 1/16,
//! 1/32 and 1/64 m on four lattices (the origin at a fixed sub-cell offset of the body's frame), its
//! boundary cells read five ways (0/1, the sampled count, `Fill`, `Centroid`, `Second`), and each
//! read through AION-10's flight (`cav_core::forward::Reader`, the read `cav run` makes) against the
//! same flight's read of the polyhedral closed form. The towers stand beside the body: the lower
//! launch point level with the body's centre, 1.5 m and 10 m from it horizontally, along the
//! benchmark's three directions made horizontal (`scripts/conv_level.py`).
//!
//! Frames: metres, the body's frame is the world's (the body at the origin, x east, y north, z up).

use cav_core::Vec3;
use cav_core::forward::{DEFAULT_QUADRATURE, Instrument, Quadrature, Reader, Reading, Sign, Source};
use cav_core::polyhedral::Faceted;
use cav_core::voxel::cut::{Representation, Weight, voxelise_cut};
use cav_core::voxel::kernel::TreeBody;
use cav_core::voxel::mesh::Mesh;
use cav_core::voxel::shape::Shape;
use cav_core::voxel::{Budget, Grid, VoxTree, voxelise};
use oracle::fixture;

/// The paper's five bodies, in its table's order.
const BODIES: [&str; 5] = ["cube_aligned", "cube", "icosahedron", "plate", "sphere"];
/// Their density, kg/m³.
const DENSITY: f64 = 2700.0;
/// The pitches: 1/n m.
const CELLS_PER_METRE: [u32; 4] = [8, 16, 32, 64];
/// The towers' distances from the body's centre, metres, horizontally.
const DISTANCES: [f64; 2] = [1.5, 10.0];
/// The research benchmark's three directions (`skin_bench.DIRECTIONS`), before they are made
/// horizontal.
const DIRECTIONS: [[f64; 3]; 3] = [[1.0, 0.2, -0.3], [-0.4, 1.0, 0.1], [0.3, -0.5, -1.0]];
/// The lattices: each origin at these fractions of a cell along x, y and z (the body's frame),
/// the same at every pitch. Chosen so that the aligned cube's faces are at least 0.015 of a cell
/// from the sample planes of k = 4 and k = 1 at every pitch: a face on one is counted inside or out
/// by rounding, which is not what is measured (`docs/slices/S5.md` §7.6).
const PLACEMENTS: [[f64; 3]; 4] =
    [[0.85, 0.39, 0.15], [0.96, 0.39, 0.85], [0.15, 0.21, 0.25], [0.16, 0.62, 0.41]];

/// The representations read, with the key the paper's scripts give each (`realread.py`'s `REPS`).
const REPS: [(&str, Rep); 5] = [
    ("zero_one_raw", Rep::ZeroOne),
    ("ss4", Rep::Sampled),
    ("frac", Rep::Cut(Representation::Fill)),
    ("frac_cen", Rep::Cut(Representation::Centroid)),
    ("frac_cen_q", Rep::Cut(Representation::Second)),
];

#[derive(Clone, Copy)]
enum Rep {
    /// One sample at each cell's centre (k = 1), each cell inside weighing ρh³: the count as made,
    /// not scaled to the body's mass.
    ZeroOne,
    /// Cavendish's sampled count before S5: k = 4, scaled to the body's exact mass.
    Sampled,
    /// A cut-cell representation of S5.
    Cut(Representation),
}

/// The body's triangles and the same as a mesh (corners joined by their bits).
fn body(name: &str) -> (Faceted, Mesh) {
    let flat = fixture(&format!("paper/polyhedral/{name}/triangles.npy"));
    let soup: Vec<[Vec3; 3]> =
        flat.chunks(9).map(|t| [0, 3, 6].map(|k| Vec3::new(t[k], t[k + 1], t[k + 2]))).collect();
    let mut vertices: Vec<Vec3> = Vec::new();
    let mut index = std::collections::HashMap::new();
    let triangles: Vec<[u32; 3]> = soup
        .iter()
        .map(|t| {
            t.map(|v| {
                *index.entry(v.to_array().map(f64::to_bits)).or_insert_with(|| {
                    vertices.push(v);
                    (vertices.len() - 1) as u32
                })
            })
        })
        .collect();
    (Faceted::new(&soup).unwrap(), Mesh::new(vertices, triangles).unwrap())
}

/// The towers' bases (each the lower launch point), distance-major: `[1.5 m × 3, 10 m × 3]`.
fn towers() -> Vec<Vec3> {
    let flat: Vec<Vec3> = DIRECTIONS
        .iter()
        .map(|d| {
            let h = Vec3::new(d[0], d[1], 0.0);
            h * (1.0 / h.length())
        })
        .collect();
    DISTANCES.iter().flat_map(|d| flat.iter().map(move |u| *u * *d)).collect()
}

/// A uniform polyhedron as a source, by the closed form: the reference read.
struct Exact<'a>(&'a Faceted);

impl Source for Exact<'_> {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, _time: f64) -> [f64; 2] {
        let at = Vec3::ZERO - tower;
        heights.map(|h| {
            self.0.difference(DENSITY, [-at.x, -at.y], h + lower - at.z, h + upper - at.z, lower - upper)
        })
    }
}

/// The lattice of `1/n` m about `shape` with its origin at `placement` cells of the body's frame
/// (`origin = h·(⌊o/h⌋ + placement)`), two cells more on every side, sampled `k` times a cell.
fn lattice(shape: &dyn Shape, n: u32, placement: [f64; 3], k: u8) -> Grid {
    let h = 1.0 / f64::from(n);
    let mut grid = Grid::around(shape.bounds().grown(2.0 * h), h, k).unwrap();
    let o = grid.origin;
    let snap = |x: f64, f: f64| ((x / h).floor() + f) * h;
    grid.origin = Vec3::new(snap(o.x, placement[0]), snap(o.y, placement[1]), snap(o.z, placement[2]));
    grid
}

fn gradiometer(reading: Reading) -> f64 {
    reading.gradiometer(Sign::LowerMinusUpper)
}

/// One body's benchmark: per tower, the exact gradiometer phase (rad, by the engine's own rule),
/// and per representation, pitch and tower, the RMS over the placements of
/// `phase(representation) − phase(exact)` (rad, the same flight rule for both).
struct Measured {
    signal: Vec<f64>,
    /// `[representation][pitch][tower]`, the error at each placement.
    errors: Vec<Vec<Vec<Vec<f64>>>>,
}

/// With `full` false (the case), only what the criteria use: the towers at 1.5 m, and not 0/1.
fn measure(name: &str, full: bool) -> Measured {
    let (exact, mesh) = body(name);
    let mass = DENSITY * exact.volume();
    let mut towers = towers();
    if !full {
        towers.truncate(3);
    }
    // The comparison's rule is C2's (Gauss order 6 on panels of at most 0.4 s): the same nodes for
    // the representation and the reference, so its own error cancels in their difference. The
    // signal is read by the engine's rule.
    let reader = Reader::new(Instrument {
        quadrature: Quadrature::Gauss { order: 6, panel: 0.4 },
        ..Instrument::aion10()
    });
    let engine = Reader::new(Instrument { quadrature: DEFAULT_QUADRATURE, ..Instrument::aion10() });
    let signal = towers.iter().map(|t| gradiometer(engine.read(*t, &Exact(&exact), 10.0))).collect();
    let reference: Vec<f64> =
        towers.iter().map(|t| gradiometer(reader.read(*t, &Exact(&exact), 10.0))).collect();
    let jobs: Vec<(usize, usize)> =
        (0..CELLS_PER_METRE.len()).flat_map(|i| (0..PLACEMENTS.len()).map(move |j| (i, j))).collect();
    let threads = std::thread::available_parallelism().map_or(2, |n| n.get().saturating_sub(2).max(1));
    let results: Vec<(usize, usize, Vec<Vec<f64>>)> = std::thread::scope(|scope| {
        let chunks: Vec<_> = jobs.chunks(jobs.len().div_ceil(threads)).collect();
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                let (mesh, reader, towers, reference) = (&mesh, &reader, &towers, &reference);
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|&(i, j)| {
                            let n = CELLS_PER_METRE[i];
                            (i, j, one_lattice(mesh, mass, n, PLACEMENTS[j], reader, towers, reference, full))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let mut errors =
        vec![vec![vec![vec![0.0; PLACEMENTS.len()]; towers.len()]; CELLS_PER_METRE.len()]; REPS.len()];
    for (i, j, per_rep) in results {
        for (r, per_tower) in per_rep.into_iter().enumerate() {
            for (t, e) in per_tower.into_iter().enumerate() {
                errors[r][i][t][j] = e;
            }
        }
    }
    Measured { signal, errors }
}

/// Every representation's error at every tower on one lattice, `[representation][tower]`, rad
/// (0/1's zero when `zero_one` is false).
#[allow(clippy::too_many_arguments, reason = "one lattice's inputs, each used once")]
fn one_lattice(
    mesh: &Mesh,
    mass: f64,
    n: u32,
    placement: [f64; 3],
    reader: &Reader,
    towers: &[Vec3],
    reference: &[f64],
    zero_one: bool,
) -> Vec<Vec<f64>> {
    let unbounded = Budget { work: u64::MAX, ..Budget::DEFAULT };
    // 0/1 (k = 1) only when asked: it is recorded, not gated.
    let one = lattice(mesh, n, placement, 1);
    let zero_one = zero_one.then(|| voxelise(mesh, &one, mass, 0).unwrap());
    let raw = zero_one.as_ref().map_or(0.0, |t| DENSITY * one.pitch.powi(3) * t.samples as f64 / mass);
    let sampled = voxelise(mesh, &lattice(mesh, n, placement, 4), mass, 0).unwrap();
    let cut =
        voxelise_cut(mesh, &lattice(mesh, n, placement, 4), Weight::Density(DENSITY), 0, unbounded).unwrap();
    let read = |tree: &VoxTree, representation: Representation, t: Vec3| {
        gradiometer(reader.read(t, &TreeBody::read_as(tree, representation, |_| Vec3::ZERO), 10.0))
    };
    REPS.iter()
        .map(|(_, rep)| {
            towers
                .iter()
                .zip(reference)
                .map(|(t, want)| {
                    let got = match rep {
                        Rep::ZeroOne => match &zero_one {
                            Some(tree) => raw * read(tree, Representation::Sampled, *t),
                            None => *want,
                        },
                        Rep::Sampled => read(&sampled, Representation::Sampled, *t),
                        Rep::Cut(r) => read(&cut, *r, *t),
                    };
                    got - want
                })
                .collect()
        })
        .collect()
}

/// The RMS over placements of the worst of the three directions at distance `d` (0 or 1), for one
/// representation and pitch: the paper's error (`conv_real.py`'s `table`).
fn rms_worst(per_tower: &[Vec<f64>], d: usize) -> f64 {
    let placements = per_tower[0].len();
    let squares: f64 = (0..placements)
        .map(|j| (3 * d..3 * d + 3).map(|t| per_tower[t][j].abs()).fold(0.0, f64::max).powi(2))
        .sum();
    (squares / placements as f64).sqrt()
}

/// The least-squares slope of `ln error` on `ln h`.
fn order(errors: &[f64]) -> f64 {
    let points: Vec<(f64, f64)> =
        CELLS_PER_METRE.iter().zip(errors).map(|(n, e)| ((1.0 / f64::from(*n)).ln(), e.ln())).collect();
    let k = points.len() as f64;
    let (mx, my) = points.iter().fold((0.0, 0.0), |(a, b), (x, y)| (a + x / k, b + y / k));
    let sxy: f64 = points.iter().map(|(x, y)| (x - mx) * (y - my)).sum();
    let sxx: f64 = points.iter().map(|(x, _)| (x - mx) * (x - mx)).sum();
    sxy / sxx
}

/// The criteria (written before measuring, `docs/slices/S5.md`, TASK-S5-18), at the towers 1.5 m
/// from the centre: on every body at h = 1/64 m, `Second` < `Centroid` < the sampled count; on the
/// aligned cube the sampled count's order 1 ± 0.3, `Fill`'s 2 ± 0.3, `Centroid`'s ≥ 2.7 and
/// `Second`'s ≥ 3.1 (C2's); on the other four `Centroid`'s and `Second`'s ≥ 2.7. Returns each
/// failure in words.
fn failures(name: &str, m: &Measured) -> Vec<String> {
    let mut out = Vec::new();
    let errs = |r: usize| -> Vec<f64> { m.errors[r].iter().map(|p| rms_worst(p, 0)).collect() };
    let [sampled, fill, centroid, second] = [1, 2, 3, 4].map(errs);
    let last = CELLS_PER_METRE.len() - 1;
    if !(second[last] < centroid[last] && centroid[last] < sampled[last]) {
        out.push(format!(
            "{name} at 1/64 m: Second {:.2e}, Centroid {:.2e}, sampled {:.2e}",
            second[last], centroid[last], sampled[last]
        ));
    }
    let bands: Vec<(&str, &[f64], f64, f64)> = if name == "cube_aligned" {
        vec![
            ("sampled", &sampled, 0.7, 1.3),
            ("Fill", &fill, 1.7, 2.3),
            ("Centroid", &centroid, 2.7, f64::INFINITY),
            ("Second", &second, 3.1, f64::INFINITY),
        ]
    } else {
        vec![("Centroid", &centroid, 2.7, f64::INFINITY), ("Second", &second, 2.7, f64::INFINITY)]
    };
    for (label, e, lo, hi) in bands {
        let p = order(e);
        if !(p >= lo && p <= hi) {
            out.push(format!("{name} {label}: order {p:.2} not in [{lo}, {hi}]"));
        }
    }
    out
}

fn all_bodies(full: bool) -> Vec<(&'static str, Measured)> {
    BODIES.iter().map(|b| (*b, measure(b, full))).collect()
}

fn failure_count(all: &[(&str, Measured)]) -> f64 {
    let mut count = 0;
    for (name, m) in all {
        for f in failures(name, m) {
            eprintln!("benchmark fails: {f}");
            count += 1;
        }
    }
    count as f64
}

cav_validate::case! {
    test: validation_the_benchmark_bodies_converge,
    name: "cut.benchmark_bodies",
    subsystem: "voxel",
    slice: "S5",
    reference: ClosedForm,
    against: "the paper's five bodies (docs/papers/polyhedral-gravity: aligned and turned 0.8 m cube, icosahedron, turned plate, icosphere; rho 2700) at h = 1/8 to 1/64 m on four lattices, read through AION-10's flight with the tower's lower launch point level with the centre 1.5 m away in three directions, against the polyhedral closed form's read: the count of criteria that fail (at 1/64 m Second < Centroid < sampled on each; aligned cube sampled order 1 +- 0.3, Fill 2 +- 0.3, Centroid >= 2.7, Second >= 3.1; the other four Centroid and Second >= 2.7)",
    unit: Count,
    tolerance: 0.0,
    measure: cav_validate::Measured::Error(failure_count(&all_bodies(false))),
}

/// For the paper (`make.py conv`): every body's errors in `conv_real.py`'s schema, written to
/// `docs/papers/polyhedral-gravity/data/cavendish/conv.json` (the flight read under "real").
#[test]
#[ignore = "writes the paper's data: cargo test --manifest-path oracle/Cargo.toml --test benchmark -- --ignored"]
fn the_benchmark_for_the_paper() {
    let started = std::time::Instant::now();
    let all = all_bodies(true);
    let list = |v: &[f64]| v.iter().map(|x| format!("{x:e}")).collect::<Vec<_>>().join(", ");
    let ns = CELLS_PER_METRE.iter().map(u32::to_string).collect::<Vec<_>>().join(", ");
    let mut json = String::from("{\n");
    for (b, (name, m)) in all.iter().enumerate() {
        json += &format!("\"{name}\": {{\"seeds\": {}", PLACEMENTS.len());
        for (d, dist) in DISTANCES.iter().enumerate() {
            let dirs = &m.signal[3 * d..3 * d + 3];
            let sig = dirs.iter().fold(0.0f64, |a, x| a.max(x.abs()));
            json += &format!(
                ", \"{dist:?}\": {{\"real\": {{\"signal\": {sig:e}, \"signal_by_direction\": [{}], \"reps\": {{",
                list(dirs)
            );
            for (r, (key, _)) in REPS.iter().enumerate() {
                let errs: Vec<f64> = m.errors[r].iter().map(|p| rms_worst(p, d)).collect();
                println!("{name} {dist} m {key}: order {:.2}, errors {}", order(&errs), list(&errs));
                json += &format!(
                    "{}\"{key}\": {{\"n\": [{ns}], \"err\": [{}], \"order\": {:e}}}",
                    if r == 0 { "" } else { ", " },
                    list(&errs),
                    order(&errs)
                );
            }
            json += "}}}";
        }
        json += &format!("}}{}\n", if b + 1 < all.len() { "," } else { "" });
        for f in failures(name, m) {
            println!("fails: {f}");
        }
    }
    json += "}\n";
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../docs/papers/polyhedral-gravity/data/cavendish");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("conv.json"), json).unwrap();
    println!("the benchmark took {:.1} s", started.elapsed().as_secs_f64());
}
