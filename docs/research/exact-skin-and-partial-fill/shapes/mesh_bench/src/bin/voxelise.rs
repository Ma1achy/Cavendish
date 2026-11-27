//! Cavendish's own mesh import and voxeliser (`cav_core::voxel`), timed on one mesh at one pitch.
//!
//!     voxelise <mesh.stl | triangles.f64> <pitch m> [supersample]
//!
//! An `.stl` goes through `Mesh::read` (the importer's path: the 32 MiB byte cap, then the triangle
//! cap and the closed / no-crossing checks); a `.f64` (9 little-endian f64 per triangle, from
//! `fullres.py dump`) is welded by exact coordinates and given to `Mesh::new`, which applies the
//! triangle and vertex caps and the same checks without the byte cap. Then `voxelise` at the pitch
//! (supersample 2 unless given, as a body's default) with the engine's default budget. Prints the
//! times, the counts and the refusal if there is one. Peak memory is measured from outside.

use std::collections::HashMap;
use std::time::Instant;

use cav_core::voxel::mesh::Mesh;
use cav_core::voxel::{voxelise, Grid, Shape};
use cav_core::Vec3;

/// The process's peak working set so far, MiB (Windows; 0 elsewhere).
fn peak_mib() -> f64 {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct Counters {
            cb: u32,
            page_faults: u32,
            peak_working_set: usize,
            rest: [usize; 7],
        }
        unsafe extern "system" {
            fn GetCurrentProcess() -> isize;
            fn K32GetProcessMemoryInfo(process: isize, counters: *mut Counters, cb: u32) -> i32;
        }
        let mut c = Counters { cb: size_of::<Counters>() as u32, page_faults: 0, peak_working_set: 0, rest: [0; 7] };
        // SAFETY: `c` is a PROCESS_MEMORY_COUNTERS of the size given; the handle is this process's.
        let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &raw mut c, c.cb) };
        if ok != 0 {
            return c.peak_working_set as f64 / (1 << 20) as f64;
        }
    }
    0.0
}

fn from_f64(path: &str) -> Result<Mesh, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let f: Vec<f64> = bytes.chunks_exact(8).map(|b| f64::from_le_bytes(b.try_into().unwrap())).collect();
    let mut index: HashMap<[u64; 3], u32> = HashMap::new();
    let mut vertices = Vec::new();
    let mut triangles = Vec::with_capacity(f.len() / 9);
    for t in f.chunks_exact(9) {
        let mut tri = [0u32; 3];
        for (k, c) in t.chunks_exact(3).enumerate() {
            let key = [c[0].to_bits(), c[1].to_bits(), c[2].to_bits()];
            tri[k] = *index.entry(key).or_insert_with(|| {
                vertices.push(Vec3::new(c[0], c[1], c[2]));
                (vertices.len() - 1) as u32
            });
        }
        triangles.push(tri);
    }
    Mesh::new(vertices, triangles).map_err(|e| e.0)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];
    let pitch: f64 = args[2].parse().expect("a pitch in metres");
    let supersample: u8 = args.get(3).map_or(2, |s| s.parse().expect("a supersample"));
    let t0 = Instant::now();
    let mesh = if path.ends_with(".f64") {
        from_f64(path)
    } else {
        Mesh::read(std::path::Path::new(path), 1.0).map_err(|e| e.0)
    };
    let read_s = t0.elapsed().as_secs_f64();
    let mesh = match mesh {
        Ok(m) => m.centred_on_box(),
        Err(why) => {
            println!("refused_at_import {read_s:.3} s: {why}");
            return;
        }
    };
    println!(
        "import_s {read_s:.3} triangles {} vertices {} peak_after_import_MiB {:.1}",
        mesh.triangles.len(),
        mesh.vertices.len(),
        peak_mib()
    );
    let t1 = Instant::now();
    let grid = match Grid::around(mesh.bounds(), pitch, supersample) {
        Ok(g) => g,
        Err(why) => {
            println!("refused_grid: {}", why.0);
            return;
        }
    };
    match voxelise(&mesh, &grid, 1.0, 0) {
        Ok(tree) => {
            let s = t1.elapsed().as_secs_f64();
            let full = u32::from(supersample).pow(3) as u8;
            let partial = tree.leaves.iter().filter(|l| l.count < full).count();
            let finest = tree.leaves.iter().filter(|l| l.level == 0).count();
            println!(
                "voxelise_s {s:.3} pitch {pitch} leaves {} finest_leaves {finest} partial_leaves {partial} nodes {} \
                 tree_bytes {} samples {} peak_MiB {:.1}",
                tree.leaves.len(),
                tree.nodes.len(),
                tree.bytes(),
                tree.samples,
                peak_mib()
            );
        }
        Err(why) => {
            println!("refused_voxelise {:.3} s peak_MiB {:.1}: {}", t1.elapsed().as_secs_f64(), peak_mib(), why.0);
        }
    }
}
