//! Streaming the world volume (S4): which chunks and bricks are resident, made from the world's
//! pieces or read from the region-file cache, around every scene pane's camera **and** around
//! every tower (the instrument always has its neighbourhood: `plan/REBUILD.md` §5.3).
//!
//! What is wanted is worked out when the scene or a view changes ([`Streaming::refresh`]): for
//! each pane that shows the scene, its camera's clipmap (`cav_sim::world::clipmap`), level 0 as
//! chunks and the levels above as bricks; for each tower, every chunk within
//! `world.stream.tower_radius` of its atoms' path. Residents no longer wanted are dropped; the
//! rest wait in a queue, nearest first. Each [`Streaming::step`] (once a frame, from
//! `Engine::poll`) makes at most a stated number of them within a stated time, so the frame is
//! never held for long and the work is the same in a page, which has one thread.
//!
//! **Bounded** (`docs/slices/S4.md` §3.1, rows W5 and W6): at most [`MAX_RESIDENT`] chunks and
//! bricks and [`MAX_RESIDENT_BYTES`] of them; the queue is what is wanted and not resident, so at
//! most [`MAX_RESIDENT`]; the wanted set is cut to the nearest [`MAX_RESIDENT`] and the cut is
//! said; at most [`MAX_BUILDS_PER_POLL`] made a step.
//!
//! **The cache** (`world/cache_native.rs`): natively, when a folder is set
//! ([`Streaming::set_cache`]), a chunk made is written to its region file
//! (`<folder>/<world hash>/r.<x>.<y>.<z>.cavr`, the format of `cav_core::voxel::region`) and a
//! chunk wanted is read from there first. In a page there is no cache (`world/cache_web.rs`):
//! chunks are made again when wanted again.

use std::collections::{BTreeMap, HashSet, VecDeque};
use std::sync::Arc;

use cav_core::Vec3;
use cav_core::voxel::world::{ChunkCells, ChunkKey};
use cav_sim::world::World;
use cav_sim::world::clipmap::{WorldFrame, level_count, levels, lod_centre, ring};
use cav_sim::{PaneContent, SceneSpec, ViewSpec};

#[cfg(not(target_arch = "wasm32"))]
#[path = "world/cache_native.rs"]
mod cache;
#[cfg(target_arch = "wasm32")]
#[path = "world/cache_web.rs"]
mod cache;

pub use cache::Cache;

/// The most chunks and bricks resident together.
pub const MAX_RESIDENT: usize = 24_576;
/// The most bytes of resident chunks and bricks.
pub const MAX_RESIDENT_BYTES: usize = 512 << 20;
/// The most chunks and bricks made in one step.
pub const MAX_BUILDS_PER_POLL: usize = 64;
/// The longest one step spends making chunks and bricks, milliseconds (it stops after the one
/// that passes it).
pub const STEP_MILLISECONDS: f64 = 6.0;
/// The most keys known to be empty that are remembered (past it they are forgotten and made
/// again if wanted).
pub const MAX_KNOWN_EMPTY: usize = 1 << 20;

/// One thing to make: a chunk (level 0) or a brick of a level above.
type Want = (u8, ChunkKey);

/// The world's resident part and its queue.
#[derive(Default)]
pub struct Streaming {
    /// The world being streamed: its hash, and its pieces made solids.
    world: Option<(u64, World)>,
    frame: WorldFrame,
    known_empty: HashSet<Want>,
    queue: VecDeque<Want>,
    bytes: usize,
    cache: Option<Cache>,
    /// What streaming has to say once: a cut, a refusal.
    notes: Vec<String>,
    /// How many chunks and bricks have been made, and how many read from the cache.
    pub made: u64,
    /// See `made`.
    pub cached: u64,
}

impl std::fmt::Debug for Streaming {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Streaming")
            .field("chunks", &self.frame.chunks.len())
            .field("bricks", &self.frame.bricks.len())
            .field("queue", &self.queue.len())
            .field("bytes", &self.bytes)
            .finish_non_exhaustive()
    }
}

impl Streaming {
    /// Reads and writes chunks through the region files under `folder` from now on (natively;
    /// in a page, nothing). `None` for no cache.
    pub fn set_cache(&mut self, folder: Option<std::path::PathBuf>) {
        self.cache = folder.map(Cache::new);
    }

    /// The resident world, when the scene has one.
    #[must_use]
    pub fn frame(&self) -> Option<&WorldFrame> {
        self.world.as_ref().map(|_| &self.frame)
    }

    /// What streaming has said since it was last asked; taken once.
    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }

    /// How many are waiting to be made.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// Bytes resident.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Works out what `spec`'s panes (`views`, those whose `contents` are the scene) and towers
    /// want, drops what is resident and no longer wanted, and queues the rest nearest first.
    pub fn refresh(&mut self, spec: &SceneSpec, views: &[ViewSpec], contents: &[PaneContent]) {
        let Some(world_spec) = &spec.world else {
            *self = Self { cache: self.cache.take(), ..Self::default() };
            return;
        };
        let hash = world_spec.hash();
        if self.world.as_ref().is_none_or(|(h, _)| *h != hash) {
            let cache = self.cache.take();
            *self = Self { cache, ..Self::default() };
            self.world = Some((hash, World::new(world_spec)));
        }
        let Some((_, world)) = &self.world else { return };
        let Some(extent) = world.extent() else {
            self.frame = WorldFrame { grid: Some(world.grid), world: hash, ..WorldFrame::default() };
            return;
        };
        let grid = world.grid;
        let n = ring(world_spec.lod.error_px);
        // Every camera, and the levels enough for the farthest of them to see the whole world.
        let centres: Vec<Vec3> = views
            .iter()
            .zip(contents)
            .filter(|(_, c)| **c == PaneContent::Scene)
            .map(|(v, _)| lod_centre(v))
            .collect();
        let count = centres.iter().map(|c| level_count(&grid, &extent, *c, n)).max().unwrap_or(1);
        let mut wanted: BTreeMap<Want, f64> = BTreeMap::new();
        let mut want = |what: Want, distance: f64| {
            let entry = wanted.entry(what).or_insert(distance);
            *entry = entry.min(distance);
        };
        for centre in &centres {
            for level in levels(&grid, *centre, n, count) {
                let range = grid.bricks_of(level.level, &extent);
                for key in level.keys(range) {
                    let distance = (grid.brick_box(level.level, key).centre() - *centre).length();
                    want((level.level, key), distance);
                }
            }
        }
        let radius = world_spec.stream.tower_radius;
        for (n, tower) in spec.array.towers.iter().enumerate() {
            let base = Vec3::from_array(tower.position);
            let top = base + Vec3::new(0.0, 0.0, spec.array.instrument_of(n).tower_height);
            let reach = cav_core::voxel::Aabb::new(base, top).grown(radius);
            if !reach.overlaps(&extent) {
                continue;
            }
            let (lo, hi) = grid.bricks_of(0, &reach);
            // A tower's neighbourhood is a few metres: bounded by its radius over the chunk side.
            for x in lo.x..=hi.x {
                for y in lo.y..=hi.y {
                    for z in lo.z..=hi.z {
                        let key = ChunkKey::new(x, y, z);
                        let b = grid.chunk_box(key);
                        if b.overlaps(&extent) {
                            // Wanted first of all: the instrument's neighbourhood.
                            want((0, key), 0.0);
                        }
                    }
                }
            }
        }
        let mut wanted: Vec<(Want, f64)> = wanted.into_iter().collect();
        wanted.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        if wanted.len() > MAX_RESIDENT {
            self.notes.push(format!(
                "the world's cameras and towers want {} chunks and bricks; the nearest {MAX_RESIDENT} are kept",
                wanted.len()
            ));
            wanted.truncate(MAX_RESIDENT);
        }
        let keep: HashSet<Want> = wanted.iter().map(|(w, _)| *w).collect();
        // Drop what is no longer wanted.
        let mut freed = 0usize;
        self.frame.chunks.retain(|key, cells| {
            let stays = keep.contains(&(0, *key));
            if !stays {
                freed += cells.bytes();
            }
            stays
        });
        self.frame.bricks.retain(|want, _| {
            let stays = keep.contains(want);
            if !stays {
                freed += BRICK_BYTES;
            }
            stays
        });
        self.bytes -= freed.min(self.bytes);
        let resident = |w: &Want| {
            if w.0 == 0 { self.frame.chunks.contains_key(&w.1) } else { self.frame.bricks.contains_key(w) }
        };
        self.queue = wanted
            .iter()
            .map(|(w, _)| *w)
            .filter(|w| !resident(w) && !self.known_empty.contains(w))
            .collect();
        self.frame.grid = Some(grid);
        self.frame.colours.clone_from(&world.colours);
        self.frame.ring = n;
        self.frame.levels = count;
        self.frame.extent = Some(extent);
        self.frame.world = hash;
        self.frame.pending = self.queue.len();
    }

    /// Makes what is queued, nearest first: at most `most` of them, stopping after the one that
    /// takes the step past `milliseconds`. True when something new became resident.
    pub fn step(&mut self, most: usize, milliseconds: f64) -> bool {
        // Taken out while it is used, so that the rest of `self` can change; put back below.
        let Some((hash, world)) = self.world.take() else { return false };
        let changed = self.make(hash, &world, most, milliseconds);
        self.world = Some((hash, world));
        changed
    }

    fn make(&mut self, hash: u64, world: &World, most: usize, milliseconds: f64) -> bool {
        let started = web_time::Instant::now();
        let mut changed = false;
        for _ in 0..most.min(MAX_BUILDS_PER_POLL) {
            if started.elapsed().as_secs_f64() * 1e3 > milliseconds {
                break;
            }
            let Some(next) = self.queue.pop_front() else { break };
            if self.bytes > MAX_RESIDENT_BYTES {
                self.notes.push(format!(
                    "the world's resident chunks and bricks reached {MAX_RESIDENT_BYTES} bytes; no more are made"
                ));
                self.queue.clear();
                break;
            }
            let (level, key) = next;
            if level == 0 {
                let cached = self.cache.as_mut().and_then(|c| c.load(hash, &world.grid, key));
                let cells = match cached {
                    Some(cells) => {
                        self.cached += 1;
                        cells
                    }
                    None => match world.chunk(key) {
                        Ok(chunk) => {
                            self.made += 1;
                            let cells = chunk.as_ref().map(ChunkCells::of);
                            if let Some(cache) = self.cache.as_mut() {
                                cache.store(hash, &world.grid, key, cells.as_ref());
                                self.notes.extend(cache.note());
                            }
                            cells
                        }
                        Err(why) => {
                            self.notes.push(format!("chunk {:?} is not drawn: {why}", key.to_array()));
                            None
                        }
                    },
                };
                match cells {
                    Some(cells) => {
                        self.bytes += cells.bytes();
                        self.frame.chunks.insert(key, Arc::new(cells));
                        changed = true;
                    }
                    None => self.forget_empty(next),
                }
            } else {
                self.made += 1;
                match world.brick(level, key) {
                    Some(brick) => {
                        self.bytes += BRICK_BYTES;
                        self.frame.bricks.insert(next, Arc::new(brick));
                        changed = true;
                    }
                    None => self.forget_empty(next),
                }
            }
        }
        self.frame.pending = self.queue.len();
        changed
    }

    fn forget_empty(&mut self, what: Want) {
        if self.known_empty.len() >= MAX_KNOWN_EMPTY {
            self.known_empty.clear();
        }
        self.known_empty.insert(what);
    }

    /// Sets whether the picture colours the clipmap's levels (the LOD bands) in place of materials.
    pub fn set_bands(&mut self, on: bool) {
        self.frame.bands = on;
    }
}

/// What a resident brick weighs: its bits, mask and materials.
const BRICK_BYTES: usize = cav_core::voxel::world::Brick::WORDS * 4 + 8 + 64 + 64;

#[cfg(test)]
mod tests {
    use super::*;
    use cav_sim::{DrawStyle, NamedView, Projection};

    fn view_above(target: [f64; 3], distance: f64) -> ViewSpec {
        let mut view =
            ViewSpec::framing(&SceneSpec::room(), NamedView::Top, Projection::Perspective, DrawStyle::Shaded);
        view.target = target;
        view.distance = distance;
        view
    }

    #[test]
    fn streaming_keeps_the_towers_and_the_camera() {
        let mut spec = cav_sim::dev::stress_1e10();
        spec.world.as_mut().unwrap().lod.error_px = 32.0;
        let mut streaming = Streaming::default();
        let contents = [PaneContent::Scene];
        // The camera far from the towers: 300 m east, 4 m up.
        let far = view_above([300.0, 0.0, 0.0], 4.0);
        streaming.refresh(&spec, std::slice::from_ref(&far), &contents);
        let queued = streaming.pending();
        assert!(queued > 0 && queued <= MAX_RESIDENT);
        // A step makes at most its count, however much is queued.
        assert!(streaming.step(5, f64::INFINITY));
        assert_eq!(streaming.made + streaming.cached, 5);
        while streaming.pending() > 0 {
            streaming.step(MAX_BUILDS_PER_POLL, f64::INFINITY);
        }
        let frame = streaming.frame().unwrap();
        // Every chunk of every tower's neighbourhood with matter is resident, though the camera is
        // 300 m away.
        let world = World::new(spec.world.as_ref().unwrap());
        let grid = world.grid;
        let mut tower_chunks = std::collections::BTreeSet::new();
        for tower in &spec.array.towers {
            let base = Vec3::from_array(tower.position);
            let reach = cav_core::voxel::Aabb::new(base, base + Vec3::new(0.0, 0.0, 10.1)).grown(8.0);
            let (lo, hi) = grid.bricks_of(0, &reach);
            for x in lo.x..=hi.x {
                for y in lo.y..=hi.y {
                    for z in lo.z..=hi.z {
                        let key = ChunkKey::new(x, y, z);
                        if world.chunk(key).unwrap().is_some() {
                            tower_chunks.insert(key);
                            assert!(frame.chunks.contains_key(&key), "tower chunk {key:?}");
                        }
                    }
                }
            }
        }
        assert!(tower_chunks.len() > 10, "{}", tower_chunks.len());
        // And the camera's own neighbourhood at level 0, and coarse bricks beyond.
        let near_camera = frame.chunks.keys().filter(|k| grid.chunk_box(**k).centre().x > 250.0).count();
        assert!(near_camera > 10 && !frame.bricks.is_empty(), "{near_camera}");
        assert!(frame.chunks.len() + frame.bricks.len() <= MAX_RESIDENT);
        assert!(streaming.bytes() <= MAX_RESIDENT_BYTES);
        // Moving the camera drops what it no longer wants, but never a tower's chunks.
        let moved = view_above([-300.0, 200.0, 0.0], 4.0);
        streaming.refresh(&spec, &[moved], &contents);
        let frame = streaming.frame().unwrap();
        assert_eq!(frame.chunks.keys().filter(|k| grid.chunk_box(**k).centre().x > 250.0).count(), 0);
        assert!(tower_chunks.iter().all(|k| frame.chunks.contains_key(k)));
        // Control: with no tower and the camera away, none of the towers' chunks would be wanted.
        let mut lonely = spec.clone();
        lonely.array.towers.clear();
        let mut other = Streaming::default();
        other.refresh(&lonely, std::slice::from_ref(&far), &contents);
        while other.pending() > 0 {
            other.step(MAX_BUILDS_PER_POLL, f64::INFINITY);
        }
        assert!(!other.frame().unwrap().chunks.keys().any(|k| grid.chunk_box(*k).centre().x.abs() < 20.0));
        // A scene without a world has no frame.
        let mut none = Streaming::default();
        none.refresh(&SceneSpec::room(), &[far], &contents);
        assert!(none.frame().is_none());
    }
}
