//! The region-file cache in a page: there is none (no file system). Chunks are made again when
//! they are wanted again; the folder a page is given is ignored.

use std::path::PathBuf;

use cav_core::voxel::world::{ChunkCells, ChunkKey, WorldGrid};

/// No cache.
pub struct Cache;

impl Cache {
    /// No cache, whatever the folder.
    #[must_use]
    pub fn new(_folder: PathBuf) -> Self {
        Self
    }

    /// Nothing is ever there.
    pub fn load(&mut self, _world: u64, _grid: &WorldGrid, _key: ChunkKey) -> Option<Option<ChunkCells>> {
        None
    }

    /// Nothing is written.
    pub fn store(&mut self, _world: u64, _grid: &WorldGrid, _key: ChunkKey, _cells: Option<&ChunkCells>) {}

    /// Nothing to say.
    pub fn note(&mut self) -> Option<String> {
        None
    }
}
