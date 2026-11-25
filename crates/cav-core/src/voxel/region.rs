//! Chunks as bytes, and region files: 8³ chunks to a file, as Minecraft keeps its world
//! (`plan/REBUILD.md` §5.3), so that a world made once is read back rather than voxelised again.
//!
//! Pure bytes: nothing here touches a disk (the engine's cache does, natively only). A region
//! file is a header and the chunks' bytes after it:
//!
//! ```text
//! "CAVR" · version u32 · world hash u64 · region key i32×3 · 512 × (offset u32, length u32)
//! chunk:  key i32×3 · supersample u8 · palette length u8 · palette · leaf count u32 · leaves (key u32, level u8, count u8, material u8)
//! ```
//!
//! little-endian throughout. A slot of length 0 holds nothing (not yet made); an empty chunk is a
//! chunk record with no palette and no leaves, so "made, and empty" is remembered too. A region
//! is append-only: a slot is written once, its bytes at the end, its entry in the header.
//!
//! **Everything read is checked before anything is reserved**: the magic, the version, the world's
//! hash (a file of another world or another build of the format is never read), every count
//! against its cap ([`MAX_CHUNK_BYTES`], [`MAX_REGION_BYTES`]) and against the bytes there, and
//! the leaves by [`VoxTree::build`]'s own checks (aligned, inside, not overlapping, counts in
//! range). Must not be used to exchange chunks between versions: the version is the format's,
//! and a new one is a new number.

use super::morton;
use super::tree::{Leaf, VoxTree};
use super::world::{CHUNK_CELLS, Chunk, ChunkKey, MAX_MATERIALS, WorldGrid};
use super::{Budget, VoxelError};

/// Chunks along each side of a region.
pub const REGION_SIDE: i32 = 8;
/// Chunks in a region.
pub const SLOTS: usize = (REGION_SIDE * REGION_SIDE * REGION_SIDE) as usize;
/// The format's version.
pub const VERSION: u32 = 1;
const MAGIC: [u8; 4] = *b"CAVR";
/// Bytes of a region's header: magic, version, hash, key, then the slots.
pub const HEADER_BYTES: usize = 4 + 4 + 8 + 12 + SLOTS * 8;
/// Bytes of one leaf on disk.
const LEAF_BYTES: usize = 7;
/// The most bytes one chunk's record can be: key, supersample, palette, leaves.
pub const MAX_CHUNK_BYTES: usize = 12 + 1 + 1 + MAX_MATERIALS + 4 + CHUNK_CELLS * LEAF_BYTES;
/// The most bytes a region file can be: its header and every slot at its largest.
pub const MAX_REGION_BYTES: usize = HEADER_BYTES + SLOTS * MAX_CHUNK_BYTES;

/// The region holding chunk `key`, and the chunk's slot in it.
#[must_use]
pub fn region_of(key: ChunkKey) -> (ChunkKey, usize) {
    let region = ChunkKey::new(
        key.x.div_euclid(REGION_SIDE),
        key.y.div_euclid(REGION_SIDE),
        key.z.div_euclid(REGION_SIDE),
    );
    let [x, y, z] = [key.x, key.y, key.z].map(|c| c.rem_euclid(REGION_SIDE) as usize);
    (region, x + REGION_SIDE as usize * (y + REGION_SIDE as usize * z))
}

/// Where slot `slot`'s entry is in the header, in bytes from the file's start.
#[must_use]
pub const fn slot_position(slot: usize) -> u64 {
    (28 + slot * 8) as u64
}

/// Chunk `key`'s record: `chunk`'s leaves and palette, or an empty record for `None` (made, and
/// holding nothing).
#[must_use]
pub fn encode_chunk(key: ChunkKey, supersample: u8, chunk: Option<&Chunk>) -> Vec<u8> {
    let leaves = chunk.map_or(&[][..], |c| &c.tree.leaves[..]);
    let palette = chunk.map_or(&[][..], |c| &c.palette[..]);
    let mut out = Vec::with_capacity(12 + 2 + palette.len() + 4 + leaves.len() * LEAF_BYTES);
    for c in key.to_array() {
        out.extend(c.to_le_bytes());
    }
    out.push(supersample);
    out.push(palette.len() as u8);
    out.extend(palette);
    out.extend((leaves.len() as u32).to_le_bytes());
    for leaf in leaves {
        out.extend(leaf.key.to_le_bytes());
        out.extend([leaf.level, leaf.count, leaf.material]);
    }
    out
}

/// Reads chunk `key`'s record on `grid`: the chunk, or `None` for one made empty.
///
/// # Errors
/// When the bytes are not a whole record of `key` at `grid`'s supersampling, a count passes its
/// cap or the bytes there, the palette is not ascending world materials, or the leaves are not a
/// tree ([`VoxTree::build`]'s checks), with what was wrong.
pub fn decode_chunk(bytes: &[u8], grid: &WorldGrid, key: ChunkKey) -> Result<Option<Chunk>, VoxelError> {
    let bad = |what: &str| VoxelError(format!("chunk {:?}: {what}", key.to_array()));
    if bytes.len() > MAX_CHUNK_BYTES {
        return Err(bad(&format!("{} bytes is more than a chunk can be, {MAX_CHUNK_BYTES}", bytes.len())));
    }
    let mut cursor = Cursor { bytes, at: 0 };
    let mut take = |n: usize| cursor.take(n).ok_or_else(|| bad("the record is cut short"));
    let mut ints = [0i32; 3];
    for c in &mut ints {
        *c = i32::from_le_bytes(take(4)?.try_into().unwrap_or_default());
    }
    if ints != key.to_array() {
        return Err(bad(&format!("the record is of chunk {ints:?}")));
    }
    let k = take(1)?[0];
    if k != grid.supersample {
        return Err(bad(&format!("sampled {k} times a side, not the world's {}", grid.supersample)));
    }
    let palette_len = usize::from(take(1)?[0]);
    let palette = take(palette_len)?.to_vec();
    if palette.windows(2).any(|w| w[0] >= w[1]) || palette.iter().any(|&m| usize::from(m) >= MAX_MATERIALS) {
        return Err(bad("the palette is not ascending world materials"));
    }
    let count = u32::from_le_bytes(take(4)?.try_into().unwrap_or_default()) as usize;
    if count > CHUNK_CELLS {
        return Err(bad(&format!("{count} leaves is more than a chunk's {CHUNK_CELLS} cells")));
    }
    // Palette and leaves follow the fixed 18 bytes of key, supersample, lengths and count.
    let rest = bytes.len() - (18 + palette_len);
    if rest != count * LEAF_BYTES {
        return Err(bad(&format!("{count} leaves need {} bytes; {rest} are there", count * LEAF_BYTES)));
    }
    if count == 0 {
        return if palette.is_empty() { Ok(None) } else { Err(bad("a palette with no leaves")) };
    }
    let mut leaves = Vec::with_capacity(count);
    for _ in 0..count {
        let raw = take(LEAF_BYTES)?;
        let leaf = Leaf {
            key: u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]),
            level: raw[4],
            count: raw[5],
            material: raw[6],
        };
        if usize::from(leaf.material) >= palette.len() {
            return Err(bad(&format!("a leaf names palette entry {} of {}", leaf.material, palette.len())));
        }
        if leaf.key >= morton::span(super::world::CHUNK_DEPTH) {
            return Err(bad("a leaf lies outside the chunk"));
        }
        leaves.push(leaf);
    }
    let samples: u64 = leaves.iter().map(Leaf::samples).sum();
    let tree = VoxTree::build_within(grid.chunk_grid(key), leaves, samples as f64, Budget::DEFAULT)
        .map_err(|e| bad(&e.0))?;
    if tree.leaves.len() != count {
        return Err(bad("the leaves are not a tree"));
    }
    Ok(Some(Chunk { key, tree, palette }))
}

/// Bytes read from the front.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let part = self.bytes.get(self.at..self.at + n)?;
        self.at += n;
        Some(part)
    }
}

/// A region's header: which world and region it is, and where each slot's record is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// The hash of the world it belongs to (whatever the world's spec hashes to).
    pub world: u64,
    /// The region's key, in units of [`REGION_SIDE`] chunks.
    pub region: ChunkKey,
    /// Each slot's `(offset, length)` in bytes from the file's start; length 0 for none.
    pub slots: Vec<(u32, u32)>,
}

impl Header {
    /// The header of an empty region.
    #[must_use]
    pub fn new(world: u64, region: ChunkKey) -> Self {
        Self { world, region, slots: vec![(0, 0); SLOTS] }
    }

    /// As bytes, [`HEADER_BYTES`] of them.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_BYTES);
        out.extend(MAGIC);
        out.extend(VERSION.to_le_bytes());
        out.extend(self.world.to_le_bytes());
        for c in self.region.to_array() {
            out.extend(c.to_le_bytes());
        }
        for (offset, length) in &self.slots {
            out.extend(offset.to_le_bytes());
            out.extend(length.to_le_bytes());
        }
        out
    }

    /// Reads a header of world `world`'s region `region` from the first [`HEADER_BYTES`] of
    /// `bytes`; `file_len` is the whole file's length, which every slot must lie within.
    ///
    /// # Errors
    /// When the magic, the version, the world, the region or a slot is wrong, with which.
    pub fn decode(bytes: &[u8], world: u64, region: ChunkKey, file_len: u64) -> Result<Self, VoxelError> {
        let bad = |what: &str| VoxelError(format!("region {:?}: {what}", region.to_array()));
        if file_len > MAX_REGION_BYTES as u64 {
            return Err(bad(&format!("{file_len} bytes is more than a region can be, {MAX_REGION_BYTES}")));
        }
        let head = bytes.get(..HEADER_BYTES).ok_or_else(|| bad("the header is cut short"))?;
        if head[..4] != MAGIC {
            return Err(bad("not a region file"));
        }
        let word = |at: usize| u32::from_le_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
        if word(4) != VERSION {
            return Err(bad(&format!("version {} is not this build's {VERSION}", word(4))));
        }
        let hash = u64::from(word(8)) | u64::from(word(12)) << 32;
        if hash != world {
            return Err(bad("the file is of another world"));
        }
        let key = [16, 20, 24].map(|at| word(at) as i32);
        if key != region.to_array() {
            return Err(bad(&format!("the file is of region {key:?}")));
        }
        let mut slots = Vec::with_capacity(SLOTS);
        for slot in 0..SLOTS {
            let at = slot_position(slot) as usize;
            let (offset, length) = (word(at), word(at + 4));
            if length > 0
                && (u64::from(offset) < HEADER_BYTES as u64
                    || u64::from(offset) + u64::from(length) > file_len
                    || length as usize > MAX_CHUNK_BYTES)
            {
                return Err(bad(&format!("slot {slot} points outside the file")));
            }
            slots.push((offset, length));
        }
        Ok(Self { world, region, slots })
    }
}

/// A whole region in memory: its header and its bytes. For tests and for building a file in one
/// go; the engine's cache reads and appends slot by slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    /// Its header.
    pub header: Header,
    /// The whole file, header first.
    pub bytes: Vec<u8>,
}

impl Region {
    /// An empty region of world `world`.
    #[must_use]
    pub fn new(world: u64, region: ChunkKey) -> Self {
        let header = Header::new(world, region);
        let bytes = header.encode();
        Self { header, bytes }
    }

    /// Writes `record` (from [`encode_chunk`]) into slot `slot`, at the end.
    ///
    /// # Errors
    /// When the slot is written already, the record is larger than a chunk can be, or the file
    /// would pass [`MAX_REGION_BYTES`].
    pub fn put(&mut self, slot: usize, record: &[u8]) -> Result<(), VoxelError> {
        if self.header.slots[slot].1 != 0 {
            return Err(VoxelError(format!("slot {slot} is written already")));
        }
        if record.is_empty()
            || record.len() > MAX_CHUNK_BYTES
            || self.bytes.len() + record.len() > MAX_REGION_BYTES
        {
            return Err(VoxelError(format!("a record of {} bytes does not fit", record.len())));
        }
        self.header.slots[slot] = (self.bytes.len() as u32, record.len() as u32);
        self.bytes.extend(record);
        let at = slot_position(slot) as usize;
        self.bytes[at..at + 8].copy_from_slice(
            &[&self.header.slots[slot].0.to_le_bytes()[..], &(record.len() as u32).to_le_bytes()].concat(),
        );
        Ok(())
    }

    /// Reads a whole region file of world `world`'s region `region`.
    ///
    /// # Errors
    /// As [`Header::decode`].
    pub fn decode(bytes: Vec<u8>, world: u64, region: ChunkKey) -> Result<Self, VoxelError> {
        let header = Header::decode(&bytes, world, region, bytes.len() as u64)?;
        Ok(Self { header, bytes })
    }

    /// Slot `slot`'s record, if it was written.
    #[must_use]
    pub fn get(&self, slot: usize) -> Option<&[u8]> {
        let (offset, length) = self.header.slots[slot];
        (length > 0).then(|| &self.bytes[offset as usize..(offset + length) as usize])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vec3;
    use crate::voxel::shape::Cuboid;
    use crate::voxel::world::{Piece, Terrain, voxelise_chunk};

    fn world() -> (WorldGrid, Terrain, Cuboid) {
        let grid = WorldGrid { origin: Vec3::new(-3.0, -3.0, -1.0), pitch: 0.1, supersample: 2 };
        let terrain = Terrain {
            lo: [-3.0, -3.0],
            hi: [6.0, 6.0],
            bottom: -0.8,
            level: 0.4,
            relief: 0.5,
            wavelength: 4.0,
            seed: 3,
        };
        let house = Cuboid { centre: Vec3::new(1.0, 0.5, 1.0), half: Vec3::new(0.7, 0.4, 0.9) };
        (grid, terrain, house)
    }

    #[test]
    fn a_region_round_trips_to_the_bit() {
        let (grid, terrain, house) = world();
        let pieces = [Piece { solid: &house, material: 3 }, Piece { solid: &terrain, material: 1 }];
        let mut region = Region::new(0xFEED, ChunkKey::new(0, 0, 0));
        let mut kept = Vec::new();
        for (x, y, z) in [(0, 0, 0), (1, 0, 0), (0, 1, 0), (1, 1, 0), (0, 0, 1), (2, 2, 2)] {
            let key = ChunkKey::new(x, y, z);
            let chunk = voxelise_chunk(&pieces, &grid, key).unwrap();
            let (_, slot) = region_of(key);
            region.put(slot, &encode_chunk(key, grid.supersample, chunk.as_ref())).unwrap();
            kept.push((key, chunk));
        }
        assert!(
            kept.iter().any(|(_, c)| c.is_none()) && kept.iter().filter(|(_, c)| c.is_some()).count() >= 4
        );
        let read = Region::decode(region.bytes.clone(), 0xFEED, ChunkKey::new(0, 0, 0)).unwrap();
        assert_eq!(read, region);
        for (key, chunk) in &kept {
            let (_, slot) = region_of(*key);
            assert_eq!(&decode_chunk(read.get(slot).unwrap(), &grid, *key).unwrap(), chunk, "{key:?}");
        }
        // A slot never written is absent; a slot is written once.
        assert!(read.get(region_of(ChunkKey::new(7, 7, 7)).1).is_none());
        assert!(region.put(0, &[1]).is_err());
        // Control: a changed byte in a leaf is a changed chunk (or refused), never the same.
        let (key, chunk) = &kept[0];
        let mut record = encode_chunk(*key, grid.supersample, chunk.as_ref());
        let last = record.len() - 2;
        record[last] ^= 1;
        assert_ne!(decode_chunk(&record, &grid, *key).ok(), Some(chunk.clone()));
        // Region keys and slots: negative chunks fall in negative regions.
        assert_eq!(region_of(ChunkKey::new(-1, 8, 15)), (ChunkKey::new(-1, 1, 1), 7 + 8 * 8 * 7));
    }

    #[test]
    fn a_broken_region_is_refused() {
        let (grid, terrain, _) = world();
        let pieces = [Piece { solid: &terrain, material: 0 }];
        let key = ChunkKey::new(0, 0, 0);
        let chunk = voxelise_chunk(&pieces, &grid, key).unwrap();
        let record = encode_chunk(key, grid.supersample, chunk.as_ref());
        let mut region = Region::new(9, key);
        region.put(0, &record).unwrap();
        let bytes = region.bytes.clone();
        // The good one reads.
        assert!(Region::decode(bytes.clone(), 9, key).is_ok());
        assert!(decode_chunk(&record, &grid, key).unwrap().is_some());
        // Controls, each refused: another world, another region, another version, not a region,
        // cut short, a slot past the end; a record of another chunk, cut short, with a leaf count
        // past the bytes, a palette out of order, another supersampling, overlapping leaves.
        assert!(Region::decode(bytes.clone(), 10, key).is_err());
        assert!(Region::decode(bytes.clone(), 9, ChunkKey::new(1, 0, 0)).is_err());
        let mut v = bytes.clone();
        v[4] = 2;
        assert!(Region::decode(v, 9, key).is_err());
        let mut v = bytes.clone();
        v[0] = b'X';
        assert!(Region::decode(v, 9, key).is_err());
        assert!(Region::decode(bytes[..HEADER_BYTES - 1].to_vec(), 9, key).is_err());
        assert!(Region::decode(bytes[..bytes.len() - 1].to_vec(), 9, key).is_err());
        assert!(decode_chunk(&record, &grid, ChunkKey::new(0, 0, 1)).is_err());
        assert!(decode_chunk(&record[..record.len() - 1], &grid, key).is_err());
        let mut v = record.clone();
        v[14 + 1..14 + 5].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode_chunk(&v, &grid, key).is_err());
        let two = encode_chunk(key, grid.supersample, None);
        let mut v = two.clone();
        v[13] = 2;
        v.splice(14..14, [3, 1]);
        assert!(decode_chunk(&v, &grid, key).is_err(), "palette out of order");
        let other = WorldGrid { supersample: 3, ..grid };
        assert!(decode_chunk(&record, &other, key).is_err());
        let c = chunk.unwrap();
        let mut overlapping = c.clone();
        let mut extra = c.tree.leaves[0];
        extra.level = 0;
        overlapping.tree.leaves.push(extra);
        let bad = encode_chunk(key, grid.supersample, Some(&overlapping));
        assert!(decode_chunk(&bad, &grid, key).is_err());
        // An empty chunk is remembered as empty.
        assert_eq!(decode_chunk(&two, &grid, key).unwrap(), None);
        // The caps are what a full chunk needs.
        const { assert!(MAX_CHUNK_BYTES >= 12 + 2 + 255 + 4 + 32_768 * 7 && MAX_REGION_BYTES < 128 << 20) };
    }
}
