//! How fast the voxeliser is and what the merge saves: spheres from 32 to 256 voxels across,
//! `k = 4`. `cargo xtask demo S3` runs it and keeps what it prints (recorded, not gated).

use cav_core::Vec3;
use cav_core::voxel::shape::{Shape, Sphere};
use cav_core::voxel::{Grid, voxelise};

fn main() {
    println!("sphere, k = 4: voxels across | cells with something in them | leaves | merge yield | nodes | seconds | cells a second");
    for across in [32u32, 64, 128, 256] {
        let sphere = Sphere { centre: Vec3::ZERO, radius: 0.5 };
        let grid = Grid::around(sphere.bounds(), 1.0 / f64::from(across), 4).expect("a grid");
        let started = std::time::Instant::now();
        let tree = voxelise(&sphere, &grid, 1.0, 0).expect("a tree");
        let seconds = started.elapsed().as_secs_f64();
        let cells: u64 = tree.leaves.iter().map(|l| u64::from(l.cells())).sum();
        println!(
            "{across:>4} | {cells:>10} | {:>8} | {:>6.1} | {:>7} | {seconds:>7.3} | {:>10.3e}",
            tree.leaves.len(),
            cells as f64 / tree.leaves.len() as f64,
            tree.nodes.len(),
            f64::from(across).powi(3) / seconds
        );
    }
}
