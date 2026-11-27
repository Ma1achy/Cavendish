//! Cavendish's own kernels on the paper-benchmark fixtures, and their cost.
//!
//! The Python scripts (`../bench_*.py`) write a plain-text fixture and read back what this
//! program prints, so the numbers in `docs/research/paper-benchmarks.md` come from the engine's
//! code (`cav_core::polyhedral::Faceted`, `cav_core::voxel::kernel`), not from a re-implementation.
//!
//! Fixture format (whitespace-separated, metres):
//! ```text
//! T <n>      then n lines of 9 numbers: a triangle's corners, anticlockwise seen from outside
//! C <m>      then m lines of 4 numbers: a cube's centre and side (density 1)
//! P <k>      then k lines of 3 numbers: a field point
//! ```
//! Modes:
//! - `field <fixture> <out>`: per point, `U g_x g_y g_z` for the triangles at density 1 with no
//!   G (W&S's `U = ∫dV/r`, positive, and `g = ∇U`, pointing at the mass);
//! - `cubes <fixture> <out>`: per point, `V_kernel V_prism`: the sum over the cubes of
//!   `cube_potential` (the engine's cube kernel: prism near, halves, then the series) and of
//!   `prism_potential` alone (Nagy's closed form), both `V/G = −∫dV/r`, kg/m at density 1;
//! - `time <fixture> <repeats>`: ns per point per triangle for `Faceted::field`, and ns per point
//!   per cube for `cube_potential` and `prism_potential`, one thread.

use std::time::Instant;

use cav_core::polyhedral::Faceted;
use cav_core::voxel::kernel::{cube_potential, prism_potential};
use cav_core::Vec3;

struct Fixture {
    triangles: Vec<[Vec3; 3]>,
    cubes: Vec<(Vec3, f64)>,
    points: Vec<Vec3>,
}

fn read(path: &str) -> Fixture {
    let text = std::fs::read_to_string(path).expect("fixture readable");
    let mut words = text.split_whitespace();
    let mut fx = Fixture { triangles: Vec::new(), cubes: Vec::new(), points: Vec::new() };
    let num = |w: &mut std::str::SplitWhitespace<'_>| -> f64 {
        w.next().expect("a number").parse().expect("a number")
    };
    while let Some(tag) = words.next() {
        let n: usize = words.next().expect("a count").parse().expect("a count");
        for _ in 0..n {
            match tag {
                "T" => {
                    let v: Vec<f64> = (0..9).map(|_| num(&mut words)).collect();
                    fx.triangles.push([
                        Vec3::new(v[0], v[1], v[2]),
                        Vec3::new(v[3], v[4], v[5]),
                        Vec3::new(v[6], v[7], v[8]),
                    ]);
                }
                "C" => {
                    let v: Vec<f64> = (0..4).map(|_| num(&mut words)).collect();
                    fx.cubes.push((Vec3::new(v[0], v[1], v[2]), v[3]));
                }
                "P" => {
                    let v: Vec<f64> = (0..3).map(|_| num(&mut words)).collect();
                    fx.points.push(Vec3::new(v[0], v[1], v[2]));
                }
                other => panic!("unknown section {other}"),
            }
        }
    }
    fx
}

fn cubes_at(cubes: &[(Vec3, f64)], p: Vec3) -> (f64, f64) {
    let (mut kernel, mut prism) = (0.0, 0.0);
    for &(c, s) in cubes {
        kernel += cube_potential(s * s * s, s, p - c);
        let half = Vec3::new(0.5 * s, 0.5 * s, 0.5 * s);
        prism += prism_potential(1.0, c - half - p, c + half - p);
    }
    (kernel, prism)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fx = read(&args[2]);
    match args[1].as_str() {
        "field" => {
            let solid = Faceted::new(&fx.triangles).expect("a closed solid");
            let mut out = String::new();
            for &p in &fx.points {
                let f = solid.field(1.0, p);
                let g = f.attraction;
                out += &format!("{:e} {:e} {:e} {:e}\n", -f.potential, g.x, g.y, g.z);
            }
            std::fs::write(&args[3], out).expect("output writable");
        }
        "cubes" => {
            let mut out = String::new();
            for &p in &fx.points {
                let (k, q) = cubes_at(&fx.cubes, p);
                out += &format!("{k:e} {q:e}\n");
            }
            std::fs::write(&args[3], out).expect("output writable");
        }
        "time" => {
            let repeats: usize = args[3].parse().expect("repeats");
            let mut sink = 0.0;
            if !fx.triangles.is_empty() {
                let solid = Faceted::new(&fx.triangles).expect("a closed solid");
                let start = Instant::now();
                for _ in 0..repeats {
                    for &p in &fx.points {
                        sink += solid.field(1.0, p).attraction.z;
                    }
                }
                let ns = start.elapsed().as_secs_f64() * 1e9;
                let per = ns / (repeats * fx.points.len() * fx.triangles.len()) as f64;
                println!("faceted_ns_per_point_per_triangle {per:.3}");
            }
            if !fx.cubes.is_empty() {
                let start = Instant::now();
                for _ in 0..repeats {
                    for &p in &fx.points {
                        for &(c, s) in &fx.cubes {
                            sink += cube_potential(s * s * s, s, p - c);
                        }
                    }
                }
                let ns = start.elapsed().as_secs_f64() * 1e9;
                let per = ns / (repeats * fx.points.len() * fx.cubes.len()) as f64;
                println!("cube_kernel_ns_per_point_per_cube {per:.3}");
                let start = Instant::now();
                for _ in 0..repeats {
                    for &p in &fx.points {
                        for &(c, s) in &fx.cubes {
                            let half = Vec3::new(0.5 * s, 0.5 * s, 0.5 * s);
                            sink += prism_potential(1.0, c - half - p, c + half - p);
                        }
                    }
                }
                let ns = start.elapsed().as_secs_f64() * 1e9;
                let per = ns / (repeats * fx.points.len() * fx.cubes.len()) as f64;
                println!("prism_ns_per_point_per_cube {per:.3}");
            }
            println!("sink {sink:e}");
        }
        other => panic!("unknown mode {other}"),
    }
}
