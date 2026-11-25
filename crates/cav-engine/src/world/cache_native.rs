//! The region-file cache, natively: chunks written to and read from files under a folder, one
//! file a region of 8³ chunks (`cav_core::voxel::region`). A world's files are in a folder named
//! by its hash, so a changed world never reads another's chunks. Append-only: a chunk's record is
//! written once at the end of its region's file and its slot set in the header.
//!
//! A file that cannot be read or is not a region of this world is passed over (the chunk is made
//! again); one that cannot be written stops the cache for the session. Must not be used for
//! anything but chunks: the engine's own state is never kept here.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use cav_core::voxel::region::{
    HEADER_BYTES, Header, MAX_REGION_BYTES, decode_chunk, encode_cells, region_of, slot_position,
};
use cav_core::voxel::world::{ChunkCells, ChunkKey, WorldGrid};

/// The most region headers held at once.
const MAX_HEADERS: usize = 256;

/// Region files under a folder.
pub struct Cache {
    folder: PathBuf,
    headers: HashMap<(u64, ChunkKey), Header>,
    stopped: bool,
    note: Option<String>,
}

impl Cache {
    /// The cache under `folder` (made when the first chunk is written).
    #[must_use]
    pub fn new(folder: PathBuf) -> Self {
        Self { folder, headers: HashMap::new(), stopped: false, note: None }
    }

    fn path(&self, world: u64, region: ChunkKey) -> PathBuf {
        self.folder
            .join(format!("{world:016x}"))
            .join(format!("r.{}.{}.{}.cavr", region.x, region.y, region.z))
    }

    /// The region's header, read from its file if it is not held.
    fn header(&mut self, world: u64, region: ChunkKey) -> Option<&Header> {
        if !self.headers.contains_key(&(world, region)) {
            let mut file = std::fs::File::open(self.path(world, region)).ok()?;
            let length = file.metadata().ok()?.len();
            let mut head = vec![0u8; HEADER_BYTES];
            file.read_exact(&mut head).ok()?;
            let header = Header::decode(&head, world, region, length).ok()?;
            if self.headers.len() >= MAX_HEADERS {
                self.headers.clear();
            }
            self.headers.insert((world, region), header);
        }
        self.headers.get(&(world, region))
    }

    /// Chunk `key` of world `world` if its record is in the cache: `Some(None)` for a chunk made
    /// empty, `None` when it is not there (or cannot be read).
    pub fn load(&mut self, world: u64, grid: &WorldGrid, key: ChunkKey) -> Option<Option<ChunkCells>> {
        let (region, slot) = region_of(key);
        let (offset, length) = self.header(world, region)?.slots[slot];
        if length == 0 {
            return None;
        }
        let mut file = std::fs::File::open(self.path(world, region)).ok()?;
        file.seek(SeekFrom::Start(u64::from(offset))).ok()?;
        let mut record = vec![0u8; length as usize];
        file.read_exact(&mut record).ok()?;
        let chunk = decode_chunk(&record, grid, key).ok()?;
        Some(chunk.as_ref().map(ChunkCells::of))
    }

    /// Writes chunk `key`'s record (`None` for one made empty) if its slot is free.
    pub fn store(&mut self, world: u64, grid: &WorldGrid, key: ChunkKey, cells: Option<&ChunkCells>) {
        if self.stopped {
            return;
        }
        if let Err(why) = self.write(world, grid, key, cells) {
            self.note = Some(format!("the world's chunk cache under {} stops: {why}", self.folder.display()));
            self.stopped = true;
        }
    }

    /// What the cache has to say (that it stopped, and why), once.
    pub fn note(&mut self) -> Option<String> {
        self.note.take()
    }

    fn write(
        &mut self,
        world: u64,
        grid: &WorldGrid,
        key: ChunkKey,
        cells: Option<&ChunkCells>,
    ) -> std::io::Result<()> {
        let (region, slot) = region_of(key);
        let path = self.path(world, region);
        if self.header(world, region).is_none() {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let header = Header::new(world, region);
            std::fs::write(&path, header.encode())?;
            self.headers.insert((world, region), header);
        }
        let Some(header) = self.headers.get_mut(&(world, region)) else { return Ok(()) };
        if header.slots[slot].1 != 0 {
            return Ok(());
        }
        let record = encode_cells(key, grid.supersample, cells);
        let mut file = std::fs::OpenOptions::new().read(true).write(true).open(&path)?;
        let end = file.seek(SeekFrom::End(0))?;
        if end as usize + record.len() > MAX_REGION_BYTES {
            return Ok(());
        }
        file.write_all(&record)?;
        let entry = (end as u32, record.len() as u32);
        file.seek(SeekFrom::Start(slot_position(slot)))?;
        file.write_all(&[entry.0.to_le_bytes(), entry.1.to_le_bytes()].concat())?;
        header.slots[slot] = entry;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cav_sim::world::World;

    #[test]
    fn a_chunk_comes_back_from_the_cache_to_the_bit() {
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-cache/region");
        let _ = std::fs::remove_dir_all(&folder);
        let spec = cav_sim::dev::world_box().world.unwrap();
        let world = World::new(&spec);
        let hash = spec.hash();
        let mut cache = Cache::new(folder.clone());
        let keys =
            [ChunkKey::new(0, 0, 0), ChunkKey::new(1, 0, 0), ChunkKey::new(0, 1, 0), ChunkKey::new(5, 5, 5)];
        for key in keys {
            assert_eq!(cache.load(hash, &world.grid, key), None, "nothing there yet");
            let cells = world.chunk(key).unwrap().as_ref().map(ChunkCells::of);
            cache.store(hash, &world.grid, key, cells.as_ref());
        }
        // A new cache over the same folder reads every one back, empty ones as empty.
        let mut again = Cache::new(folder.clone());
        for key in keys {
            let made = world.chunk(key).unwrap().as_ref().map(ChunkCells::of);
            assert_eq!(again.load(hash, &world.grid, key), Some(made), "{key:?}");
        }
        // Control: another world's hash finds nothing; a damaged file is passed over.
        assert_eq!(again.load(hash ^ 1, &world.grid, keys[0]), None);
        let file = again.path(hash, ChunkKey::new(0, 0, 0));
        let mut bytes = std::fs::read(&file).unwrap();
        bytes[0] = b'X';
        std::fs::write(&file, bytes).unwrap();
        assert_eq!(Cache::new(folder.clone()).load(hash, &world.grid, keys[0]), None);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
