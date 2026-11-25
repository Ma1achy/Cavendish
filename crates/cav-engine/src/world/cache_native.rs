//! The region-file cache, natively: chunks written to and read from files under a folder, one
//! file a region of 8³ chunks (`cav_core::voxel::region`). The files are under
//! `<folder>/v<format>-x<voxeliser>/<world hash>/`: the region format's [`VERSION`], the
//! voxeliser's [`VOXELISER_VERSION`] and the world's spec, so a changed world, file format or way
//! of making chunks never reads another's chunks. Append-only: a chunk's record is written once at
//! the end of its region's file and its slot set in the header.
//!
//! A region file is made with `create_new` (never over one that is there). A file that is there
//! but cannot be read as a region of this world (another version, half written by another
//! process, damaged) is left alone: its chunks are made again and not cached, and that is said
//! once. One that cannot be written stops the cache for the session. Two processes writing the
//! same region at once are not coordinated; a record a reader cannot decode is passed over. Must
//! not be used for anything but chunks: the engine's own state is never kept here.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use cav_core::voxel::region::{
    HEADER_BYTES, Header, MAX_REGION_BYTES, VERSION, decode_chunk, encode_cells, region_of, slot_position,
};
use cav_core::voxel::world::{ChunkCells, ChunkKey, VOXELISER_VERSION, WorldGrid};

/// The most region headers held at once.
const MAX_HEADERS: usize = 256;

/// The most regions remembered as there but unreadable (past it they are forgotten, and looked
/// at again).
const MAX_PASSED_OVER: usize = 4096;

/// Region files under a folder.
pub struct Cache {
    folder: PathBuf,
    /// The voxeliser's version the folder is keyed by ([`VOXELISER_VERSION`]; another only in
    /// tests).
    voxeliser: u32,
    headers: HashMap<(u64, ChunkKey), Header>,
    /// Regions whose file is there but cannot be read: never written over, never read.
    passed_over: std::collections::HashSet<(u64, ChunkKey)>,
    stopped: bool,
    note: Option<String>,
}

impl Cache {
    /// The cache under `folder` (made when the first chunk is written).
    #[must_use]
    pub fn new(folder: PathBuf) -> Self {
        Self::of_voxeliser(folder, VOXELISER_VERSION)
    }

    /// The cache under `folder` as a voxeliser of version `voxeliser` keys it: for tests that a
    /// changed voxeliser reads nothing cached by the old one.
    fn of_voxeliser(folder: PathBuf, voxeliser: u32) -> Self {
        Self {
            folder,
            voxeliser,
            headers: HashMap::new(),
            passed_over: std::collections::HashSet::new(),
            stopped: false,
            note: None,
        }
    }

    fn path(&self, world: u64, region: ChunkKey) -> PathBuf {
        self.folder
            .join(format!("v{VERSION}-x{}", self.voxeliser))
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

    /// What the cache has to say (that it stopped, or passed over a file, and why), once.
    pub fn note(&mut self) -> Option<String> {
        self.note.take()
    }

    /// Remembers that the region's file is there and cannot be read: it is neither read nor
    /// written again (until forgotten), and that is said for the first such file.
    fn pass_over(&mut self, world: u64, region: ChunkKey, path: &std::path::Path) {
        if self.passed_over.len() >= MAX_PASSED_OVER {
            self.passed_over.clear();
        }
        if self.passed_over.is_empty() && self.note.is_none() {
            self.note = Some(format!(
                "the world's chunk cache leaves {} alone (it is not a region this build can read); its chunks are made \
                 and not cached",
                path.display()
            ));
        }
        self.passed_over.insert((world, region));
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
        if self.passed_over.contains(&(world, region)) {
            return Ok(());
        }
        if self.header(world, region).is_none() {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            // Never over a file that is there: one this cache cannot read is another's (another
            // version, or one being written) and is left alone.
            let made = std::fs::OpenOptions::new().write(true).create_new(true).open(&path);
            let mut file = match made {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    self.pass_over(world, region, &path);
                    return Ok(());
                }
                Err(e) => return Err(e),
            };
            let header = Header::new(world, region);
            file.write_all(&header.encode())?;
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

    #[test]
    fn another_voxeliser_reads_nothing_cached() {
        // The review's I8: a chunk cached by one way of making chunks is not served to another.
        let folder =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-cache/voxeliser");
        let _ = std::fs::remove_dir_all(&folder);
        let spec = cav_sim::dev::world_box().world.unwrap();
        let world = World::new(&spec);
        let hash = spec.hash();
        let key = ChunkKey::new(0, 0, 0);
        let cells = world.chunk(key).unwrap().as_ref().map(ChunkCells::of);
        Cache::new(folder.clone()).store(hash, &world.grid, key, cells.as_ref());
        // Control: the same voxeliser reads it back.
        assert_eq!(Cache::new(folder.clone()).load(hash, &world.grid, key), Some(cells));
        let mut next = Cache::of_voxeliser(folder.clone(), VOXELISER_VERSION + 1);
        assert_eq!(next.load(hash, &world.grid, key), None);
        assert!(next.path(hash, key).to_string_lossy().contains(&format!("x{}", VOXELISER_VERSION + 1)));
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_region_it_cannot_read_is_left_alone() {
        // The review's I8: a file at a region's path that is not a region this build reads
        // (another version, one being written) is never written over.
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-cache/foreign");
        let _ = std::fs::remove_dir_all(&folder);
        let spec = cav_sim::dev::world_box().world.unwrap();
        let world = World::new(&spec);
        let hash = spec.hash();
        let (key, other) = (ChunkKey::new(0, 0, 0), ChunkKey::new(1, 0, 0));
        let mut cache = Cache::new(folder.clone());
        let path = cache.path(hash, key);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // A region of another format version: the magic, then version 99.
        let foreign: Vec<u8> = [&b"CAVR"[..], &99u32.to_le_bytes(), &[7u8; 64]].concat();
        std::fs::write(&path, &foreign).unwrap();
        for k in [key, other] {
            let cells = world.chunk(k).unwrap().as_ref().map(ChunkCells::of);
            cache.store(hash, &world.grid, k, cells.as_ref());
            assert_eq!(cache.load(hash, &world.grid, k), None);
        }
        assert_eq!(std::fs::read(&path).unwrap(), foreign, "the file is as it was");
        assert!(cache.note().is_some_and(|n| n.contains("leaves") && n.contains("alone")));
        assert!(!cache.stopped, "the rest of the cache goes on");
        // Control: with the file gone, the region is made and its chunks cached.
        std::fs::remove_file(&path).unwrap();
        let mut fresh = Cache::new(folder.clone());
        let cells = world.chunk(key).unwrap().as_ref().map(ChunkCells::of);
        fresh.store(hash, &world.grid, key, cells.as_ref());
        assert_eq!(fresh.load(hash, &world.grid, key), Some(cells));
        assert_ne!(std::fs::read(&path).unwrap(), foreign);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
