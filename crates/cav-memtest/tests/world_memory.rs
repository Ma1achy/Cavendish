//! The world volume's memory is bounded by its caps (`docs/slices/S4.md` §3.1, rows W2 to W4):
//! a chunk by its cells, a region's record by its stated bytes, and a record that claims more
//! than it holds is refused before anything is reserved for it. Measured with the counting
//! allocator; nextest runs each test in a process of its own.

use cav_core::Vec3;
use cav_core::voxel::region::{MAX_CHUNK_BYTES, decode_chunk, encode_chunk};
use cav_core::voxel::world::{CHUNK_CELLS, ChunkKey, Piece, Terrain, WorldGrid, voxelise_chunk};
use cav_memtest::{Counting, peak_during};

#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[test]
fn reading_a_region_is_bounded() {
    let grid = WorldGrid { origin: Vec3::ZERO, pitch: 0.05, supersample: 3 };
    // The worst a chunk can be for leaves: a surface that cuts every cell (rough hills at the
    // pitch), so that little merges.
    let rough = Terrain {
        lo: [0.0, 0.0],
        hi: [2.0, 2.0],
        bottom: 0.0,
        level: 0.8,
        relief: 0.75,
        wavelength: 0.4,
        seed: 2,
    };
    let pieces = [Piece { solid: &rough, material: 0 }];
    let key = ChunkKey::new(0, 0, 0);
    let (chunk, made) = peak_during(|| voxelise_chunk(&pieces, &grid, key).unwrap().unwrap());
    let leaves = chunk.tree.leaves.len();
    assert!(leaves > 4000 && leaves <= CHUNK_CELLS, "{leaves}");
    let record = encode_chunk(key, grid.supersample, Some(&chunk));
    assert!(record.len() <= MAX_CHUNK_BYTES);
    // Reading it back holds the leaves once and the tree: a few times the record, well under a
    // megabyte past it.
    let (back, peak) = peak_during(|| decode_chunk(&record, &grid, key).unwrap());
    eprintln!(
        "{leaves} leaves; making it peaked at {made} bytes, reading its {} bytes at {peak}",
        record.len()
    );
    assert_eq!(back.as_ref(), Some(&chunk));
    assert!(peak <= 8 * record.len() + (1 << 20), "{peak} bytes for a record of {}", record.len());
    // Making a chunk is bounded by its cells, not by the world around it.
    assert!(made <= 64 << 20, "{made}");
    // Control: a record that claims the most leaves a chunk can have but holds a few is refused
    // having reserved nothing for them.
    let mut lying = record[..18 + chunk.palette.len()].to_vec();
    lying[14 + chunk.palette.len()..18 + chunk.palette.len()]
        .copy_from_slice(&(CHUNK_CELLS as u32).to_le_bytes());
    lying.extend(&record[18 + chunk.palette.len()..18 + chunk.palette.len() + 70]);
    let (refused, peak) = peak_during(|| decode_chunk(&lying, &grid, key));
    assert!(refused.is_err());
    assert!(peak < 4096, "{peak} bytes reserved for a refused record");
}
