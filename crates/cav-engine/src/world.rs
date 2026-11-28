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
//! **Bounded** (`docs/slices/S4.md` §3.1, rows W5 and W6; [`Caps`]): at most [`MAX_RESIDENT`] chunks and
//! bricks and [`MAX_RESIDENT_BYTES`] of them; the queue is what is wanted and not resident, so at
//! most [`MAX_RESIDENT`]; the wanted set is cut to the nearest [`MAX_RESIDENT`] and the cut is
//! said; at most [`MAX_BUILDS_PER_POLL`] made a step.
//!
//! **The cache** (`world/cache_native.rs`): natively, when a folder is set
//! ([`Streaming::set_cache`]), a chunk made is written to its region file
//! (`<folder>/v<format>-x<voxeliser>/<world hash>/r.<x>.<y>.<z>.cavr`, the format of `cav_core::voxel::region`) and a
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
/// The most chunks and bricks made in one step (in batches as wide as the machine's threads;
/// one at a time in a page).
pub const MAX_BUILDS_PER_POLL: usize = 256;
/// The longest one step means to spend making chunks and bricks, milliseconds: it starts no
/// batch once they have gone, nor one that the longest batch so far says would pass them (at
/// least one batch a step, so a batch longer than this still passes it).
pub const STEP_MILLISECONDS: f64 = 6.0;
/// The most keys known to be empty that are remembered (past it they are forgotten and made
/// again if wanted).
pub const MAX_KNOWN_EMPTY: usize = 1 << 20;

/// One thing to make: a chunk (level 0) or a brick of a level above.
type Want = (u8, ChunkKey);

/// What making one thing gave.
enum Made {
    /// A chunk read from the cache (`None`: made empty before).
    Cached(Option<ChunkCells>),
    /// A chunk made now.
    Chunk(Option<ChunkCells>),
    /// A brick made now.
    Brick(Option<cav_core::voxel::world::Brick>),
    /// A chunk that could not be made, and why.
    Refused(String),
}

/// The caps a [`Streaming`] holds itself to: [`MAX_RESIDENT`] and [`MAX_RESIDENT_BYTES`] unless
/// a test asks for smaller ([`Streaming::with_caps`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caps {
    /// The most chunks and bricks resident together.
    pub resident: usize,
    /// The most bytes of resident chunks and bricks.
    pub bytes: usize,
}

impl Default for Caps {
    fn default() -> Self {
        Self { resident: MAX_RESIDENT, bytes: MAX_RESIDENT_BYTES }
    }
}

/// The world's resident part and its queue.
#[derive(Default)]
pub struct Streaming {
    /// What it holds itself to.
    caps: Caps,
    /// The world being streamed: its hash, and its pieces made solids.
    world: Option<(u64, World)>,
    frame: WorldFrame,
    known_empty: HashSet<Want>,
    queue: VecDeque<Want>,
    bytes: usize,
    cache: Option<Cache>,
    /// What streaming has to say once: a cut, a refusal.
    notes: Vec<String>,
    /// What the last refresh was asked for (the world, the cameras' snapped level boxes, the
    /// towers): a refresh asked the same again changes nothing and is skipped.
    asked: Vec<u64>,
    /// Whether the bytes cap stopped the making since the last refresh (said once).
    full: bool,
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
    /// A streaming held to `caps`, each at most its constant ([`MAX_RESIDENT`],
    /// [`MAX_RESIDENT_BYTES`]): for tests that reach the caps with a small scene. The engine's
    /// own is [`Streaming::default`]. Must not be used to raise a cap.
    #[must_use]
    pub fn with_caps(caps: Caps) -> Self {
        let caps =
            Caps { resident: caps.resident.min(MAX_RESIDENT), bytes: caps.bytes.min(MAX_RESIDENT_BYTES) };
        Self { caps, ..Self::default() }
    }

    /// The caps it holds itself to.
    #[must_use]
    pub fn caps(&self) -> Caps {
        self.caps
    }

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
        self.refresh_with(spec, views, contents, &[]);
    }

    /// [`Self::refresh`], with the draw list of each pane that draws the world by a tower's or
    /// towers' cut (`lists[pane]`, `cav_sim::world::lod_view`): those chunks and bricks are wanted
    /// too, in the order of their distance from the pane's camera in their own level's sides.
    pub fn refresh_with(
        &mut self,
        spec: &SceneSpec,
        views: &[ViewSpec],
        contents: &[PaneContent],
        lists: &[Option<&[(u8, ChunkKey)]>],
    ) {
        let Some(world_spec) = &spec.world else {
            *self = Self { cache: self.cache.take(), caps: self.caps, ..Self::default() };
            return;
        };
        let hash = world_spec.hash();
        if self.world.as_ref().is_none_or(|(h, _)| *h != hash) {
            let cache = self.cache.take();
            *self = Self { cache, caps: self.caps, ..Self::default() };
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
        // The same boxes and towers as last time: nothing to do (a camera moving inside its
        // chunk, or a command that is not about the world).
        let mut asked = vec![hash, n as u64, u64::from(count)];
        for centre in &centres {
            for level in levels(&grid, *centre, n, count) {
                asked.extend(level.lo.iter().map(|v| *v as u64));
            }
        }
        for (n, tower) in spec.array.towers.iter().enumerate() {
            asked.extend(tower.position.map(f64::to_bits));
            asked.push(spec.array.instrument_of(n).tower_height.to_bits());
        }
        asked.push(world_spec.stream.tower_radius.to_bits());
        for list in lists.iter().flatten() {
            asked.push(list.len() as u64);
            asked.extend(list.iter().map(|(l, k)| {
                (u64::from(*l) << 56)
                    ^ ((k.x as u64 & 0xf_ffff) << 36)
                    ^ ((k.y as u64 & 0xf_ffff) << 16)
                    ^ (k.z as u64 & 0xffff)
            }));
        }
        if asked == self.asked {
            return;
        }
        self.asked = asked;
        let mut wanted: BTreeMap<Want, f64> = BTreeMap::new();
        let mut want = |what: Want, distance: f64| {
            let entry = wanted.entry(what).or_insert(distance);
            *entry = entry.min(distance);
        };
        // Each in the order it matters to the picture: its distance in its own level's bricks
        // (a coarse brick far away is as urgent as a chunk near by), and only where some piece
        // may have matter.
        for centre in &centres {
            for level in levels(&grid, *centre, n, count) {
                let range = grid.bricks_of(level.level, &extent);
                let side = grid.brick_side(level.level);
                for key in level.keys(range) {
                    let boxed = grid.brick_box(level.level, key);
                    if world.reaches(&boxed) {
                        want((level.level, key), (boxed.centre() - *centre).length() / side);
                    }
                }
            }
        }
        // The panes that draw a tower's cut: its entries (chunks at level 0, bricks above).
        for (pane, list) in lists.iter().enumerate() {
            let (Some(list), Some(view)) = (list, views.get(pane)) else { continue };
            let centre = lod_centre(view);
            for &(level, key) in *list {
                let boxed = grid.brick_box(level, key);
                if world.reaches(&boxed) {
                    want((level, key), (boxed.centre() - centre).length() / grid.brick_side(level));
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
                        if world.reaches(&grid.chunk_box(key)) {
                            // Wanted first of all: the instrument's neighbourhood.
                            want((0, key), 0.0);
                        }
                    }
                }
            }
        }
        let mut wanted: Vec<(Want, f64)> = wanted.into_iter().collect();
        wanted.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        let most = self.caps.resident;
        if wanted.len() > most {
            // The towers' chunks are wanted at distance 0, so they are the last to go.
            self.notes.push(format!(
                "the world's cameras and towers want {} chunks and bricks; the {most} that matter most are kept",
                wanted.len()
            ));
            wanted.truncate(most);
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
        self.full = false;
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
        self.frame.waiting = self.queue.iter().copied().collect();
    }

    /// Makes what is queued, nearest first: at most `most` of them, in batches as wide as the
    /// read's threads, starting no batch once `milliseconds` have gone or the longest batch so far
    /// would take the step past them (at least one batch). True when something new became
    /// resident.
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
        let mut left = most.min(MAX_BUILDS_PER_POLL);
        // Batches as wide as the read's threads (one in a page): each thread makes its share of
        // a batch, and the batch is put in the queue's order, so what is resident does not depend
        // on the thread count.
        let threads = cav_sim::read::threads();
        // The longest batch so far this step: a batch that would likely pass the step's time is
        // not started (TASK-S4-27: the step had run a whole batch, 3 to 6 ms natively, past it).
        let mut longest = 0.0f64;
        while left > 0 && !self.queue.is_empty() {
            let spent = started.elapsed().as_secs_f64() * 1e3;
            if spent > milliseconds || (longest > 0.0 && spent + longest > milliseconds) {
                break;
            }
            let batch_started = web_time::Instant::now();
            let width = threads.min(left);
            let batch: Vec<Want> = (0..width).map_while(|_| self.queue.pop_front()).collect();
            for want in &batch {
                self.frame.waiting.remove(want);
            }
            left -= batch.len();
            // The cache first, on this thread (it holds open files); what it has not, made below.
            let mut found: Vec<Option<Made>> = batch
                .iter()
                .map(|&(level, key)| {
                    (level == 0)
                        .then(|| self.cache.as_mut().and_then(|c| c.load(hash, &world.grid, key)))
                        .flatten()
                        .map(Made::Cached)
                })
                .collect();
            let make_one = |&(level, key): &Want| -> Made {
                if level == 0 {
                    match world.chunk(key) {
                        Ok(chunk) => Made::Chunk(chunk.as_ref().map(ChunkCells::of)),
                        Err(why) => Made::Refused(format!("chunk {:?} is not drawn: {why}", key.to_array())),
                    }
                } else {
                    Made::Brick(world.brick(level, key))
                }
            };
            let missing: Vec<usize> = (0..batch.len()).filter(|&i| found[i].is_none()).collect();
            if missing.len() > 1 {
                let results: Vec<(usize, Made)> = std::thread::scope(|scope| {
                    let handles: Vec<_> = missing
                        .iter()
                        .map(|&i| {
                            let want = batch[i];
                            let make_one = &make_one;
                            scope.spawn(move || (i, make_one(&want)))
                        })
                        .collect();
                    handles.into_iter().filter_map(|h| h.join().ok()).collect()
                });
                for (i, made) in results {
                    found[i] = Some(made);
                }
            } else if let Some(&i) = missing.first() {
                found[i] = Some(make_one(&batch[i]));
            }
            for (want, made) in batch.into_iter().zip(found) {
                let (level, key) = want;
                match made {
                    Some(Made::Cached(cells)) => {
                        self.cached += 1;
                        changed |= self.keep_chunk(want, cells);
                    }
                    Some(Made::Chunk(cells)) => {
                        self.made += 1;
                        if let Some(cache) = self.cache.as_mut() {
                            cache.store(hash, &world.grid, key, cells.as_ref());
                            self.notes.extend(cache.note());
                        }
                        changed |= self.keep_chunk(want, cells);
                    }
                    Some(Made::Brick(brick)) => {
                        self.made += 1;
                        match brick {
                            Some(brick) => {
                                if self.fits(BRICK_BYTES) {
                                    self.bytes += BRICK_BYTES;
                                    self.frame.bricks.insert((level, key), Arc::new(brick));
                                    changed = true;
                                }
                            }
                            None => self.forget_empty(want),
                        }
                    }
                    Some(Made::Refused(why)) => self.notes.push(why),
                    None => {}
                }
            }
            longest = longest.max(batch_started.elapsed().as_secs_f64() * 1e3);
        }
        self.frame.pending = self.queue.len();
        changed
    }

    /// Whether `more` bytes fit under the bytes cap. When they do not, nothing more is made until
    /// the next refresh (the queue is emptied) and that is said once: what is resident stays
    /// within the cap, a batch included.
    fn fits(&mut self, more: usize) -> bool {
        if self.bytes + more <= self.caps.bytes {
            return true;
        }
        if !self.full {
            self.notes.push(format!(
                "the world's resident chunks and bricks reached their {} bytes; no more are made until the view moves",
                self.caps.bytes
            ));
        }
        self.full = true;
        self.queue.clear();
        self.frame.waiting.clear();
        false
    }

    /// Keeps a chunk made or read (or remembers it empty). True when it became resident.
    fn keep_chunk(&mut self, want: Want, cells: Option<ChunkCells>) -> bool {
        match cells {
            Some(cells) if !self.fits(cells.bytes()) => false,
            Some(cells) => {
                self.bytes += cells.bytes();
                self.frame.chunks.insert(want.1, Arc::new(cells));
                true
            }
            None => {
                self.forget_empty(want);
                false
            }
        }
    }

    fn forget_empty(&mut self, what: Want) {
        if self.known_empty.len() >= MAX_KNOWN_EMPTY {
            self.known_empty.clear();
        }
        self.known_empty.insert(what);
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
    fn slow_streaming_keeps_the_towers_and_the_camera() {
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

    #[test]
    fn slow_the_frame_says_what_is_waiting() {
        // A renderer draws stand-ins over what the streaming has still to make (TASK-S4-26), so
        // the frame carries the queue: what is wanted and neither resident nor known empty.
        let mut spec = cav_sim::dev::stress_1e10();
        spec.world.as_mut().unwrap().lod.error_px = 32.0;
        let mut streaming = Streaming::default();
        streaming.refresh(&spec, &[view_above([300.0, 0.0, 0.0], 4.0)], &[PaneContent::Scene]);
        let queued: HashSet<Want> = streaming.queue.iter().copied().collect();
        let frame = streaming.frame().unwrap();
        assert!(queued.len() > 10 && frame.waiting == queued, "{} queued", queued.len());
        // As things are made they leave it; none of it is resident.
        streaming.step(7, f64::INFINITY);
        let frame = streaming.frame().unwrap();
        assert_eq!(frame.waiting.len(), queued.len() - 7);
        assert!(frame.waiting.iter().all(|w| {
            if w.0 == 0 { !frame.chunks.contains_key(&w.1) } else { !frame.bricks.contains_key(w) }
        }));
        // Control: made whole, nothing waits.
        while streaming.pending() > 0 {
            streaming.step(MAX_BUILDS_PER_POLL, f64::INFINITY);
        }
        assert!(streaming.frame().unwrap().waiting.is_empty());
    }

    /// Streams `spec` for a camera 300 m east with `caps` until nothing is queued; returns it.
    fn streamed(spec: &SceneSpec, caps: Caps) -> Streaming {
        let mut streaming = Streaming::with_caps(caps);
        streaming.refresh(spec, &[view_above([300.0, 0.0, 0.0], 4.0)], &[PaneContent::Scene]);
        for _ in 0..10_000 {
            if streaming.pending() == 0 {
                break;
            }
            streaming.step(MAX_BUILDS_PER_POLL, f64::INFINITY);
        }
        assert_eq!(streaming.pending(), 0);
        streaming
    }

    /// The chunks with matter within 8 m of some tower's atoms: what must stay resident.
    fn tower_chunks(spec: &SceneSpec) -> Vec<ChunkKey> {
        let world = World::new(spec.world.as_ref().unwrap());
        let mut keys = std::collections::BTreeSet::new();
        for tower in &spec.array.towers {
            let base = Vec3::from_array(tower.position);
            let reach = cav_core::voxel::Aabb::new(base, base + Vec3::new(0.0, 0.0, 10.1)).grown(8.0);
            let (lo, hi) = world.grid.bricks_of(0, &reach);
            for x in lo.x..=hi.x {
                for y in lo.y..=hi.y {
                    for z in lo.z..=hi.z {
                        let key = ChunkKey::new(x, y, z);
                        if world.chunk(key).unwrap().is_some() {
                            keys.insert(key);
                        }
                    }
                }
            }
        }
        keys.into_iter().collect()
    }

    #[test]
    fn slow_streaming_holds_its_caps() {
        let mut spec = cav_sim::dev::stress_1e10();
        spec.world.as_mut().unwrap().lod.error_px = 32.0;
        let towers = tower_chunks(&spec);
        // Builds a step: asked for any number, a step makes at most MAX_BUILDS_PER_POLL.
        let mut streaming = Streaming::default();
        streaming.refresh(&spec, &[view_above([300.0, 0.0, 0.0], 4.0)], &[PaneContent::Scene]);
        let queued = streaming.pending();
        assert!(queued > MAX_BUILDS_PER_POLL, "{queued}: more than a step's worth is wanted");
        streaming.step(usize::MAX, f64::INFINITY);
        let first = streaming.made + streaming.cached;
        assert!(first == MAX_BUILDS_PER_POLL as u64, "{first}");
        assert_eq!(streaming.pending(), queued - MAX_BUILDS_PER_POLL);
        let whole = streamed(&spec, Caps::default());
        let wants = whole.frame().map_or(0, |f| f.chunks.len() + f.bricks.len());
        // The resident cap: a cap below what the scene wants keeps the nearest, the towers'
        // chunks first, and says so.
        let cap = towers.len() + 40;
        assert!(wants > cap + 100, "{wants} wanted");
        let mut capped = streamed(&spec, Caps { resident: cap, bytes: MAX_RESIDENT_BYTES });
        let frame = capped.frame().unwrap();
        assert!(frame.chunks.len() + frame.bricks.len() <= cap);
        assert!(towers.iter().all(|k| frame.chunks.contains_key(k)), "every tower chunk is kept");
        assert!(
            capped.take_notes().iter().any(|n| n.contains(&format!("the {cap} that matter most are kept")))
        );
        // Control: with the default cap nothing is cut and nothing is said.
        let mut uncut = streamed(&spec, Caps::default());
        assert!(!uncut.take_notes().iter().any(|n| n.contains("that matter most")));
        // The bytes cap: held after every batch, not only before it (a batch is as wide as the
        // machine's threads), and said once.
        let bytes = whole.bytes() / 5;
        let mut small = streamed(&spec, Caps { resident: MAX_RESIDENT, bytes });
        assert!(small.bytes() <= bytes && small.bytes() > bytes / 2, "{} of {bytes}", small.bytes());
        let notes = small.take_notes();
        assert_eq!(notes.iter().filter(|n| n.contains("no more are made")).count(), 1, "{notes:?}");
        // Control: the default holds everything this scene wants, past the small cap.
        assert!(whole.bytes() > bytes);
        // A cap is never raised past its constant.
        let asked = Caps { resident: usize::MAX, bytes: usize::MAX };
        assert_eq!(Streaming::with_caps(asked).caps(), Caps::default());
    }
}
