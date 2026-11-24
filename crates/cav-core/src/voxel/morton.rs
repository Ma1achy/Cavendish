//! Morton (Z-order) keys: three cell indices of ten bits each, interleaved into one `u32`.
//!
//! Bit `3b` of a key is bit `b` of `x`, bit `3b + 1` is bit `b` of `y`, bit `3b + 2` is bit `b`
//! of `z`. Sorting by key visits cells in Z order, so the cells of any aligned block of side
//! `2^ℓ` are one run of keys: the block's key with its low `3ℓ` bits cleared, and the next
//! `8^ℓ` keys. Thirty bits is what a WGSL shader can hold without 64-bit integers (the research
//! note `docs/research/s3-sparse-voxel-structures.md` §6), so one tree is at most 1024 cells
//! across.

/// Bits of each index in a key.
pub const BITS: u32 = 10;
/// The most cells across one tree: `2^BITS`.
pub const SIDE: u32 = 1 << BITS;

/// Spreads the low ten bits of `v` to every third bit: `abc` becomes `a00b00c`.
const fn spread(v: u32) -> u32 {
    let mut x = v & 0x3ff;
    x = (x | (x << 16)) & 0x0300_00ff;
    x = (x | (x << 8)) & 0x0300_f00f;
    x = (x | (x << 4)) & 0x030c_30c3;
    x = (x | (x << 2)) & 0x0924_9249;
    x
}

/// Gathers every third bit of `v` (from bit 0) into the low ten bits: the inverse of `spread`.
const fn gather(v: u32) -> u32 {
    let mut x = v & 0x0924_9249;
    x = (x | (x >> 2)) & 0x030c_30c3;
    x = (x | (x >> 4)) & 0x0300_f00f;
    x = (x | (x >> 8)) & 0x0300_00ff;
    x = (x | (x >> 16)) & 0x3ff;
    x
}

/// The key of the cell `[x, y, z]` (cell indices, each below [`SIDE`]; higher bits are ignored).
#[must_use]
pub const fn encode(cell: [u32; 3]) -> u32 {
    spread(cell[0]) | (spread(cell[1]) << 1) | (spread(cell[2]) << 2)
}

/// The cell `[x, y, z]` of a key.
#[must_use]
pub const fn decode(key: u32) -> [u32; 3] {
    [gather(key), gather(key >> 1), gather(key >> 2)]
}

/// How many finest cells (keys) an aligned block of level `level` spans: `8^level`.
#[must_use]
pub const fn span(level: u8) -> u32 {
    1 << (3 * level as u32)
}

/// The key of the block of level `level` that holds `key`: its low `3·level` bits cleared.
#[must_use]
pub const fn block(key: u32, level: u8) -> u32 {
    if level as u32 >= BITS { 0 } else { key & !(span(level) - 1) }
}

/// The key of child `octant` (0 to 7, the bits being x, y, z) of the block of level `level` (at
/// least 1) whose key is `key`.
#[must_use]
pub const fn child(key: u32, level: u8, octant: u32) -> u32 {
    key + octant * span(level - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morton_keys_round_trip_and_sort_in_z_order() {
        // Every index at the edges and a spread of others, on every axis.
        let mut values: Vec<u32> = vec![0, 1, 2, 3, 511, 512, 1022, 1023];
        values.extend((0..64).map(|k| (k * 16_381) % SIDE));
        for &x in &values {
            for &y in &values {
                for &z in &values {
                    assert_eq!(decode(encode([x, y, z])), [x, y, z]);
                }
            }
        }
        // Keys are a permutation of 0..2^30: the largest cell has the largest key.
        assert_eq!(encode([SIDE - 1; 3]), (1 << 30) - 1);
        assert_eq!(encode([1, 0, 0]), 1);
        assert_eq!(encode([0, 1, 0]), 2);
        assert_eq!(encode([0, 0, 1]), 4);
        // Z order: the eight cells of a 2×2×2 block are consecutive keys, and a block's cells all
        // come before the next block's.
        for level in 1..=3u8 {
            let side = 1u32 << level;
            let mut keys: Vec<u32> = Vec::new();
            for x in 0..side {
                for y in 0..side {
                    for z in 0..side {
                        keys.push(encode([x + side, y, z]));
                    }
                }
            }
            keys.sort_unstable();
            let first = encode([side, 0, 0]);
            assert_eq!(first, block(first, level));
            assert_eq!(keys, (first..first + span(level)).collect::<Vec<_>>());
        }
        // Children of a block are its eight sub-blocks in octant order.
        let parent = encode([4, 8, 12]);
        for octant in 0..8 {
            let [x, y, z] = decode(child(parent, 2, octant));
            assert_eq!(
                [x, y, z],
                [4 + (octant & 1) * 2, 8 + ((octant >> 1) & 1) * 2, 12 + (octant >> 2) * 2]
            );
        }
        // Control: a key off by one is another cell, and a swapped pair of indices is too.
        assert_ne!(decode(encode([3, 5, 7]) + 1), [3, 5, 7]);
        assert_ne!(encode([5, 3, 7]), encode([3, 5, 7]));
    }
}
