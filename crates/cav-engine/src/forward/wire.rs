//! The forward stage's two jobs as bytes, so that they can be run somewhere that shares no memory
//! with the engine: in a browser page, a Web Worker running the same module (the page's thread
//! must never wait on a voxel body's seconds of work, `plan/REBUILD.md` §5.1).
//!
//! A job is a request ([`contribution_request`], [`reading_request`]); whoever runs it calls
//! [`serve`], which is the very code a native worker thread runs (`cav_sim::read::contribution`
//! and `cav_sim::read::reading` on the scene as sent), and the engine reads the answer back with
//! [`answer`]. The scene travels as JSON (exact: the workspace builds `serde_json` with
//! `float_roundtrip`, and a scene's numbers are finite); the answer's numbers travel as their
//! bits. So a job run through here gives the same bits as one run in place
//! (`tests::a_job_through_the_wire_is_the_same_bits`).
//!
//! Where the jobs go is a [`Remote`], installed by the application ([`install`]); it is asked
//! only where the forward stage has no threads (`forward/inline.rs`). Natively nothing installs
//! one and nothing asks.
//!
//! Must not be used to store or exchange numbers between versions: the layout is this build's
//! own, and both ends are always the same module.

use std::cell::RefCell;
use std::rc::Rc;

use cav_core::voxel::world::{ChunkKey, WorldGrid};
use cav_sim::SceneSpec;
use cav_sim::read::{self, Contribution, Phases, ReadingAt, SourceId, TowerLedger, Warning, WorldLedger};
use cav_sim::world::inner::InnerKey;
use cav_sim::world::lod_view::{ReadView, TowerCut};
use cav_sim::world::octree::NodeKey;

/// Somewhere jobs run off the caller's thread: in a page, the application's Web Workers.
pub trait Remote {
    /// Hands over a job's request; answers a ticket to ask for its answer by. Never waits.
    fn send(&self, request: Vec<u8>) -> u64;
    /// The answer to `ticket` (what [`serve`] returned), once it has come; it is given once.
    fn take(&self, ticket: u64) -> Option<Vec<u8>>;
}

thread_local! {
    static REMOTE: RefCell<Option<Rc<dyn Remote>>> = const { RefCell::new(None) };
}

/// Sends this thread's forward jobs to `remote` from now on (where the forward stage has no
/// threads: in a page). Installed once, before or after engines are made.
pub fn install(remote: Rc<dyn Remote>) {
    REMOTE.with_borrow_mut(|installed| *installed = Some(remote));
}

/// The remote installed on this thread, if any.
#[must_use]
pub fn remote() -> Option<Rc<dyn Remote>> {
    REMOTE.with_borrow(Clone::clone)
}

const CONTRIBUTION: u8 = 1;
const READING: u8 = 2;
/// Held meshes for the worker to keep, then a job (S19's review, I4).
const ASSETS: u8 = 3;
const NOTHING: u8 = 0;
/// The world's index in a request (the floor's is `u64::MAX`, a body's its number).
const WORLD: u64 = u64::MAX - 1;
/// The length of a held mesh's hash ([`cav_sim::assets::hash`]).
const HASH: usize = 16;

/// A job's request: its kind, its head (a source's index, or a time's bits), the hashes of the
/// held meshes its scene names (at most 255: a byte of count), then the scene.
fn request(kind: u8, head: u64, spec: &SceneSpec) -> Vec<u8> {
    let mut out = vec![kind];
    out.extend(head.to_le_bytes());
    let named = cav_sim::assets::named(spec);
    out.push(u8::try_from(named.len()).unwrap_or(u8::MAX));
    for hash in named.iter().take(usize::from(u8::MAX)) {
        out.extend(hash.as_bytes());
    }
    out.extend(serde_json::to_vec(spec).expect("a scene is JSON"));
    out
}

/// The request for `source`'s contribution to the signal of `spec`, at the shots
/// `cav_sim::read::shots(spec)` gives (the ones the engine schedules).
#[must_use]
pub fn contribution_request(spec: &SceneSpec, source: SourceId) -> Vec<u8> {
    let index = match source {
        SourceId::Floor => u64::MAX,
        SourceId::World => WORLD,
        SourceId::Body(n) => n as u64,
    };
    request(CONTRIBUTION, index, spec)
}

/// The request for what every tower of `spec` reads from a shot read out at `time`.
#[must_use]
pub fn reading_request(spec: &SceneSpec, time: f64) -> Vec<u8> {
    request(READING, time.to_bits(), spec)
}

/// The hashes of the held meshes a request's scene names, read from its head (not its scene):
/// what a worker must hold before it runs the job. None for bytes that are not a request.
#[must_use]
pub fn needs(request: &[u8]) -> Vec<String> {
    let Some(&count) = request.get(9) else { return Vec::new() };
    let hashes = request.get(10..10 + HASH * usize::from(count)).unwrap_or_default();
    hashes
        .as_chunks::<HASH>()
        .0
        .iter()
        .filter_map(|h| std::str::from_utf8(h).ok())
        .map(str::to_owned)
        .collect()
}

/// `request` with the held meshes `assets` (each a hash, the file's name and its bytes) put
/// before it, for a worker that does not hold them yet: it keeps them, then runs the job. A page
/// sends each mesh to each worker once (`cav-app`'s lane says which it lacks).
#[must_use]
pub fn with_assets(request: &[u8], assets: &[(String, String, std::sync::Arc<[u8]>)]) -> Vec<u8> {
    let mut out = vec![ASSETS];
    out.extend((assets.len() as u64).to_le_bytes());
    for (_, name, bytes) in assets {
        out.extend((name.len() as u64).to_le_bytes());
        out.extend(name.as_bytes());
        out.extend((bytes.len() as u64).to_le_bytes());
        out.extend(bytes.iter());
    }
    out.extend(request);
    out
}

/// What a job's runner does: the request's work, done here, and its answer as bytes. A request
/// that cannot be read, or a scene whose shots are refused, answers nothing (the engine never
/// sends either: it is the same module at both ends). Held meshes sent before the job
/// ([`with_assets`]) are kept first; one that cannot be kept leaves its bodies unread, with the
/// voxeliser's words as their warning.
#[must_use]
pub fn serve(request: &[u8]) -> Vec<u8> {
    let never = || false;
    let Some((&kind, rest)) = request.split_first() else { return vec![NOTHING] };
    if kind == ASSETS {
        let mut reader = Reader { bytes: rest, at: 0 };
        let Ok(count) = reader.count() else { return vec![NOTHING] };
        for _ in 0..count {
            let name =
                reader.count().and_then(|n| reader.take(n).map(|b| String::from_utf8_lossy(b).into_owned()));
            let Ok(name) = name else { return vec![NOTHING] };
            let Ok(bytes) = reader.count().and_then(|n| reader.take(n)) else { return vec![NOTHING] };
            // One that cannot be kept (past the session's cap) is not held: the job then reads
            // its bodies as unreadable, and its warning says why in the mesh's words.
            let _ = cav_sim::assets::hold(&name, bytes);
        }
        return serve(&rest[reader.at..]);
    }
    let Some((head, rest)) = rest.split_first_chunk::<8>() else { return vec![NOTHING] };
    let Some((&count, rest)) = rest.split_first() else { return vec![NOTHING] };
    let Some(json) = rest.get(HASH * usize::from(count)..) else { return vec![NOTHING] };
    let Ok(spec) = serde_json::from_slice::<SceneSpec>(json) else { return vec![NOTHING] };
    let head = u64::from_le_bytes(*head);
    let mut out = Writer::default();
    match kind {
        CONTRIBUTION => {
            let source = match head {
                u64::MAX => SourceId::Floor,
                WORLD => SourceId::World,
                n => SourceId::Body(n as usize),
            };
            let valid = match source {
                SourceId::Floor => spec.floor.is_some(),
                SourceId::World => spec.world.is_some(),
                SourceId::Body(n) => n < spec.bodies.len(),
            };
            let made = read::shots(&spec).ok().filter(|_| valid);
            match made.and_then(|shots| read::contribution(&spec, source, &shots, &never)) {
                Some(contribution) => {
                    out.byte(CONTRIBUTION);
                    out.floats(&contribution.lower);
                    out.floats(&contribution.upper);
                    out.floats(&contribution.gradiometer);
                    out.float(contribution.estimate);
                    out.floats(&contribution.estimates);
                    out.byte(u8::from(contribution.settled));
                    out.maybe(contribution.truncated);
                    out.world(contribution.world.as_ref());
                    out.warnings(&contribution.warnings);
                    out.view(contribution.view.as_deref());
                }
                None => out.byte(NOTHING),
            }
        }
        READING => match read::reading(&spec, f64::from_bits(head), &never) {
            Some(reading) => {
                out.byte(READING);
                out.float(reading.time);
                let phases: Vec<f64> =
                    reading.towers.iter().flat_map(|p| [p.lower, p.upper, p.gradiometer]).collect();
                out.floats(&phases);
                out.float(reading.estimate);
                out.byte(u8::from(reading.settled));
                out.maybe(reading.truncated);
                out.world(reading.world.as_ref());
                out.warnings(&reading.warnings);
            }
            None => out.byte(NOTHING),
        },
        _ => out.byte(NOTHING),
    }
    out.0
}

/// An answer, read back.
#[derive(Debug, PartialEq)]
pub enum Answer {
    /// A source's contribution.
    Contribution(Contribution),
    /// A reading.
    Reading(ReadingAt),
    /// Nothing: the job could not be done (see [`serve`]).
    Nothing,
}

/// Reads an answer [`serve`] wrote.
///
/// # Errors
/// Bytes that are not an answer (cut short, or of another layout).
pub fn answer(bytes: &[u8]) -> Result<Answer, String> {
    let mut reader = Reader { bytes, at: 0 };
    let answer = match reader.byte()? {
        CONTRIBUTION => Answer::Contribution(Contribution {
            lower: reader.floats()?,
            upper: reader.floats()?,
            gradiometer: reader.floats()?,
            estimate: reader.float()?,
            estimates: reader.floats()?,
            settled: reader.byte()? != 0,
            truncated: reader.maybe()?,
            world: reader.world()?,
            warnings: reader.warnings()?,
            view: reader.view()?.map(std::sync::Arc::new),
        }),
        READING => {
            let time = reader.float()?;
            let phases = reader.floats()?;
            if phases.len() % 3 != 0 {
                return Err("a reading's phases are not in threes".into());
            }
            Answer::Reading(ReadingAt {
                time,
                towers: phases
                    .chunks(3)
                    .map(|p| Phases { lower: p[0], upper: p[1], gradiometer: p[2] })
                    .collect(),
                estimate: reader.float()?,
                settled: reader.byte()? != 0,
                truncated: reader.maybe()?,
                world: reader.world()?,
                warnings: reader.warnings()?,
            })
        }
        NOTHING => Answer::Nothing,
        other => return Err(format!("an answer of kind {other}")),
    };
    if reader.at != bytes.len() {
        return Err("an answer with bytes left over".into());
    }
    Ok(answer)
}

#[derive(Default)]
struct Writer(Vec<u8>);

impl Writer {
    fn byte(&mut self, byte: u8) {
        self.0.push(byte);
    }

    fn count(&mut self, count: usize) {
        self.0.extend((count as u64).to_le_bytes());
    }

    /// A number that may be absent: a byte (0 or 1), then the number when it is there.
    fn maybe(&mut self, value: Option<f64>) {
        self.byte(u8::from(value.is_some()));
        if let Some(value) = value {
            self.float(value);
        }
    }

    fn float(&mut self, value: f64) {
        self.0.extend(value.to_bits().to_le_bytes());
    }

    fn floats(&mut self, values: &[f64]) {
        self.count(values.len());
        for &value in values {
            self.float(value);
        }
    }

    /// The world's ledger, when there is one: a byte (0 or 1), then the tolerance and each
    /// tower's ledger, counts, nodes per level, blocks, blocks per level, leaves, its local
    /// expansion's degree, far and near nodes and audit, and whether its walk settled.
    fn world(&mut self, world: Option<&WorldLedger>) {
        self.byte(u8::from(world.is_some()));
        let Some(world) = world else { return };
        self.float(world.tolerance);
        self.count(world.towers.len());
        for tower in &world.towers {
            self.float(tower.ledger);
            self.count(tower.accepted);
            self.count(tower.opened);
            self.count(tower.per_level.len());
            for &n in &tower.per_level {
                self.count(n);
            }
            self.count(tower.blocks);
            self.count(tower.blocks_per_level.len());
            for &n in &tower.blocks_per_level {
                self.count(n);
            }
            self.count(usize::try_from(tower.leaves).unwrap_or(usize::MAX));
            self.count(tower.degree);
            self.count(tower.far);
            self.count(tower.near);
            self.float(tower.audit);
            self.byte(u8::from(tower.settled));
        }
    }

    fn key(&mut self, level: u8, key: ChunkKey) {
        self.byte(level);
        for v in key.to_array() {
            self.0.extend(v.to_le_bytes());
        }
    }

    /// What the views draw of the world's read, when there is one: a byte (0 or 1), then the grid
    /// (origin, pitch, supersample), the octree's nodes (level, key), and each tower's cut (its
    /// ledger; accepted nodes with their bounds; blocks with theirs; opened chunks).
    fn view(&mut self, view: Option<&ReadView>) {
        self.byte(u8::from(view.is_some()));
        let Some(view) = view else { return };
        self.byte(u8::from(view.grid.is_some()));
        if let Some(grid) = view.grid {
            for v in grid.origin.to_array() {
                self.float(v);
            }
            self.float(grid.pitch);
            self.byte(grid.supersample);
        }
        self.count(view.nodes.len());
        for n in &view.nodes {
            self.key(n.level, n.key);
        }
        self.count(view.towers.len());
        for t in &view.towers {
            self.float(t.ledger);
            self.count(t.accepted.len());
            for (k, b) in &t.accepted {
                self.key(k.level, k.key);
                self.float(*b);
            }
            self.count(t.inner.len());
            for (k, b) in &t.inner {
                self.key(k.level, k.chunk);
                self.0.extend(k.key.to_le_bytes());
                self.float(*b);
            }
            self.count(t.opened.len());
            for k in &t.opened {
                self.key(0, *k);
            }
        }
    }

    fn warnings(&mut self, warnings: &[Warning]) {
        self.count(warnings.len());
        for warning in warnings {
            self.count(warning.tower.map_or(0, |tower| tower + 1));
            self.count(warning.text.len());
            self.0.extend(warning.text.as_bytes());
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8], String> {
        let end = self.at.checked_add(count).filter(|&end| end <= self.bytes.len());
        let end = end.ok_or("an answer cut short")?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn count(&mut self) -> Result<usize, String> {
        let bytes: [u8; 8] = self.take(8)?.try_into().map_err(|_| "a count")?;
        usize::try_from(u64::from_le_bytes(bytes)).map_err(|_| "a count larger than memory".to_owned())
    }

    fn maybe(&mut self) -> Result<Option<f64>, String> {
        match self.byte()? {
            0 => Ok(None),
            1 => self.float().map(Some),
            other => Err(format!("a flag of {other}")),
        }
    }

    fn float(&mut self) -> Result<f64, String> {
        let bytes: [u8; 8] = self.take(8)?.try_into().map_err(|_| "a number")?;
        Ok(f64::from_bits(u64::from_le_bytes(bytes)))
    }

    fn floats(&mut self) -> Result<Vec<f64>, String> {
        let count = self.count()?;
        // Bounded by what is there: a count larger than the bytes left is refused before any
        // memory is asked for.
        let bytes = self.take(count.checked_mul(8).ok_or("a count larger than memory")?)?;
        Ok(bytes.as_chunks::<8>().0.iter().map(|b| f64::from_bits(u64::from_le_bytes(*b))).collect())
    }

    fn world(&mut self) -> Result<Option<WorldLedger>, String> {
        match self.byte()? {
            0 => return Ok(None),
            1 => {}
            other => return Err(format!("a flag of {other}")),
        }
        let tolerance = self.float()?;
        let count = self.count()?;
        // Each tower takes at least 89 bytes: a count larger than what is left is refused before
        // anything is reserved.
        if count > (self.bytes.len() - self.at) / 89 {
            return Err("an answer cut short".into());
        }
        let mut towers = Vec::with_capacity(count);
        for _ in 0..count {
            let ledger = self.float()?;
            let accepted = self.count()?;
            let opened = self.count()?;
            let levels = self.count()?;
            if levels > (self.bytes.len() - self.at) / 8 {
                return Err("an answer cut short".into());
            }
            let per_level = (0..levels).map(|_| self.count()).collect::<Result<Vec<_>, _>>()?;
            let blocks = self.count()?;
            let sizes = self.count()?;
            if sizes > (self.bytes.len() - self.at) / 8 {
                return Err("an answer cut short".into());
            }
            let blocks_per_level = (0..sizes).map(|_| self.count()).collect::<Result<Vec<_>, _>>()?;
            let leaves = self.count()? as u64;
            let (degree, far, near) = (self.count()?, self.count()?, self.count()?);
            let audit = self.float()?;
            let settled = self.byte()? != 0;
            towers.push(TowerLedger {
                ledger,
                accepted,
                opened,
                per_level,
                blocks,
                blocks_per_level,
                leaves,
                degree,
                far,
                near,
                audit,
                settled,
            });
        }
        Ok(Some(WorldLedger { tolerance, towers }))
    }

    fn key(&mut self) -> Result<(u8, ChunkKey), String> {
        let level = self.byte()?;
        let mut v = [0i32; 3];
        for a in &mut v {
            *a = i32::from_le_bytes(self.take(4)?.try_into().map_err(|_| "a key")?);
        }
        Ok((level, ChunkKey::new(v[0], v[1], v[2])))
    }

    /// A count of items of at least `each` bytes, refused when larger than what is left.
    fn items(&mut self, each: usize) -> Result<usize, String> {
        let count = self.count()?;
        if count > (self.bytes.len() - self.at) / each {
            return Err("an answer cut short".into());
        }
        Ok(count)
    }

    fn view(&mut self) -> Result<Option<ReadView>, String> {
        match self.byte()? {
            0 => return Ok(None),
            1 => {}
            other => return Err(format!("a flag of {other}")),
        }
        let grid = match self.byte()? {
            0 => None,
            _ => {
                let origin = cav_core::Vec3::new(self.float()?, self.float()?, self.float()?);
                Some(WorldGrid { origin, pitch: self.float()?, supersample: self.byte()? })
            }
        };
        let nodes = (0..self.items(13)?)
            .map(|_| self.key().map(|(level, key)| NodeKey { level, key }))
            .collect::<Result<Vec<_>, _>>()?;
        let mut towers = Vec::new();
        for _ in 0..self.items(32)? {
            let ledger = self.float()?;
            let accepted = (0..self.items(21)?)
                .map(|_| Ok((self.key().map(|(level, key)| NodeKey { level, key })?, self.float()?)))
                .collect::<Result<Vec<_>, String>>()?;
            let inner = (0..self.items(25)?)
                .map(|_| {
                    let (level, chunk) = self.key()?;
                    let key = u32::from_le_bytes(self.take(4)?.try_into().map_err(|_| "a key")?);
                    Ok((InnerKey { chunk, level, key }, self.float()?))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let opened =
                (0..self.items(13)?).map(|_| self.key().map(|(_, k)| k)).collect::<Result<Vec<_>, _>>()?;
            towers.push(TowerCut { accepted, inner, opened, ledger });
        }
        Ok(Some(ReadView { grid, nodes, towers }))
    }

    fn warnings(&mut self) -> Result<Vec<Warning>, String> {
        let count = self.count()?;
        let mut warnings = Vec::new();
        for _ in 0..count {
            let tower = self.count()?.checked_sub(1);
            let length = self.count()?;
            let text = std::str::from_utf8(self.take(length)?).map_err(|_| "a warning that is not UTF-8")?;
            warnings.push(Warning { tower, text: text.to_owned() });
        }
        Ok(warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(values: &[f64]) -> Vec<u64> {
        values.iter().map(|v| v.to_bits()).collect()
    }

    #[test]
    fn a_job_through_the_wire_is_the_same_bits() {
        // A point, an oscillator and a floor (the dev scenes with each kind). The voxel sphere is
        // the same code path at 8 to 9 s, so it runs in the web tier
        // (`a_voxel_job_through_the_wire_is_the_same_bits`); the point alone catches a broken
        // encoding in milliseconds (S17's second review, item 1).
        let _ = same_bits(&[cav_sim::dev::point(), cav_sim::dev::comb(), cav_sim::dev::pass()]);
    }

    #[test]
    fn an_exact_body_through_the_wire_is_the_same_bits() {
        // S5 (TASK-S5-06, A3): bodies read Exact, no voxels in the read, through the wire as any
        // source; the scene travels as JSON with `representation` in it.
        let scene = cav_sim::dev::exact_solids();
        assert!(contribution_request(&scene, SourceId::Body(0)).windows(5).any(|w| w == b"Exact"));
        let _ = same_bits(&[scene]);
    }

    #[test]
    fn slow_a_world_job_through_the_wire_is_the_same_bits() {
        // The world goes through the wire by its own index (`WORLD`): `world-box` (four chunks),
        // read whole with its ledger, and stress-1e10, past the octree's caps and not read, whose
        // truncation must come back.
        let whole = cav_sim::dev::world_box();
        let part = cav_sim::dev::stress_1e10();
        let flags = same_bits(&[whole, part]);
        // What came back says it, whatever the direct read said (S4's second review, J4): the
        // part is truncated at 0 m and not settled, in the contribution and the reading.
        assert_eq!(flags[1], (Some((false, Some(0.0))), (false, Some(0.0))));
        // Control: the whole box comes back whole and settled.
        assert_eq!(flags[0], (Some((true, None)), (true, None)));
        // A world asked of a scene without one is nothing, not a world of zeros.
        let none = answer(&serve(&contribution_request(&cav_sim::dev::point(), SourceId::World))).unwrap();
        assert_eq!(none, Answer::Nothing);
    }

    /// The voxel sphere through the wire: run by `cargo xtask web`, not `quick`.
    #[test]
    #[ignore = "8 to 9 s: run by `cargo xtask web`"]
    fn a_voxel_job_through_the_wire_is_the_same_bits() {
        let _ = same_bits(&[cav_sim::dev::sphere()]);
    }

    /// Every source of every scene, and the reading at 1 s, sent through the wire and read back:
    /// the same bits as made directly.
    /// `(settled, truncated)` as an answer came back with them.
    type Flags = (bool, Option<f64>);

    /// Returns, for each scene, the flags of its sent world contribution (when it has a world)
    /// and of its sent reading.
    fn same_bits(specs: &[SceneSpec]) -> Vec<(Option<Flags>, Flags)> {
        let never = || false;
        let mut flags = Vec::new();
        for spec in specs {
            let mut world = None;
            let shots = read::shots(spec).unwrap();
            for source in read::sources(spec) {
                let direct = read::contribution(spec, source, &shots, &never).unwrap();
                let Answer::Contribution(sent) = answer(&serve(&contribution_request(spec, source))).unwrap()
                else {
                    panic!("{}: {source:?}: no contribution came back", spec.name)
                };
                for (a, b) in [
                    (&direct.lower, &sent.lower),
                    (&direct.upper, &sent.upper),
                    (&direct.gradiometer, &sent.gradiometer),
                    (&direct.estimates, &sent.estimates),
                ] {
                    assert_eq!(bits(a), bits(b), "{}: {source:?}", spec.name);
                }
                assert_eq!(direct.estimate.to_bits(), sent.estimate.to_bits());
                assert_eq!(
                    (direct.settled, direct.truncated, &direct.warnings, &direct.world),
                    (sent.settled, sent.truncated, &sent.warnings, &sent.world)
                );
                if source == SourceId::World {
                    assert_eq!(sent.world.is_some(), sent.truncated.is_none(), "{}", spec.name);
                    world = Some((sent.settled, sent.truncated));
                }
            }
            let direct = read::reading(spec, 1.0, &never).unwrap();
            let Answer::Reading(sent) = answer(&serve(&reading_request(spec, 1.0))).unwrap() else {
                panic!("{}: no reading came back", spec.name)
            };
            let phases = |r: &ReadingAt| -> Vec<u64> {
                r.towers.iter().flat_map(|p| [p.lower, p.upper, p.gradiometer]).map(f64::to_bits).collect()
            };
            assert_eq!(phases(&direct), phases(&sent), "{}", spec.name);
            assert_eq!(
                (direct.time.to_bits(), direct.estimate.to_bits()),
                (sent.time.to_bits(), sent.estimate.to_bits())
            );
            assert_eq!(
                (direct.settled, direct.truncated, &direct.warnings, &direct.world),
                (sent.settled, sent.truncated, &sent.warnings, &sent.world)
            );
            flags.push((world, (sent.settled, sent.truncated)));
        }
        flags
    }

    #[test]
    fn a_held_mesh_travels_with_the_job_that_needs_it() {
        // S19's review, I4, and its second review, J1(a): a worker that starts with an empty
        // store reads a body of a held mesh only from the bytes sent before its job. The mesh is
        // NOT held first. Control: the plain request, served before the mesh has come, leaves the
        // body unread and says so by the mesh's hash; with `serve` no longer keeping what
        // `with_assets` sent, the second answer would be that one again.
        use cav_core::Vec3;
        use cav_core::voxel::mesh::{box_mesh, to_stl};
        // Bytes no other test of this crate holds (the store is one static a process).
        let (v, t) = box_mesh(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.5, 0.25, 0.8125));
        let bytes = to_stl(&v, &t);
        let hash = cav_sim::assets::hash(&bytes);
        assert!(cav_sim::assets::held(&hash).is_none(), "the store starts without it");
        let mut spec = cav_sim::dev::point();
        let mut shape = cav_sim::ShapeSpec::new(
            cav_sim::ShapeKind::Mesh { file: "wired.stl".into(), scale: 1.0, hash: Some(hash.clone()) },
            0.125,
            2,
        );
        shape.material = Some("steel".into());
        spec.bodies.push(cav_sim::BodySpec::shaped(
            "wired",
            1.0,
            shape,
            cav_sim::Motion::Static { position: [2.0, 1.0, 1.0] },
        ));
        let request = contribution_request(&spec, SourceId::Body(1));
        assert_eq!(needs(&request), std::slice::from_ref(&hash));
        assert!(needs(&contribution_request(&cav_sim::dev::point(), SourceId::Body(0))).is_empty());
        // The control: the job alone, the mesh not there. The body is not read, in words.
        let unread = serve(&request);
        let Ok(Answer::Contribution(nothing)) = answer(&unread) else { panic!("an answer, unread") };
        assert!(nothing.warnings.iter().any(|w| w.text.contains(&hash)), "{:?}", nothing.warnings);
        assert!(nothing.gradiometer.iter().all(|phase| *phase == 0.0), "an unread body reads as nothing");
        assert!(cav_sim::assets::held(&hash).is_none(), "and serving the job held nothing");
        // An assets message cut short answers nothing and keeps nothing.
        let sent = with_assets(
            &request,
            &[(hash.clone(), "wired.stl".into(), std::sync::Arc::from(bytes.as_slice()))],
        );
        assert_eq!(answer(&serve(&sent[..20])), Ok(Answer::Nothing), "cut short");
        assert!(cav_sim::assets::held(&hash).is_none());
        // The mesh sent before the job: kept, and the body read.
        let read = serve(&sent);
        assert!(cav_sim::assets::held(&hash).is_some(), "the worker keeps what it was sent");
        let Ok(Answer::Contribution(body)) = answer(&read) else { panic!("an answer, read") };
        assert!(body.warnings.is_empty(), "{:?}", body.warnings);
        assert!(body.gradiometer.iter().any(|phase| *phase != 0.0), "the towers read the body");
        assert_ne!(read, unread);
        // From then on the plain request gives the same bits, and they are the bits of the read
        // made in place.
        assert_eq!(serve(&request), read);
        assert_eq!(spec.validate(), Ok(()));
        let _ = same_bits(std::slice::from_ref(&spec));
    }

    #[test]
    fn a_broken_message_is_refused_not_trusted() {
        let spec = cav_sim::dev::point();
        let good = serve(&contribution_request(&spec, SourceId::Body(0)));
        assert!(matches!(answer(&good), Ok(Answer::Contribution(_))));
        // Control arms: cut short, a byte too many, a count far past the bytes there.
        assert!(answer(&good[..good.len() - 1]).is_err());
        assert!(answer(&[good.as_slice(), &[0]].concat()).is_err());
        let mut huge = vec![CONTRIBUTION];
        huge.extend(u64::MAX.to_le_bytes());
        assert!(answer(&huge).is_err());
        // A request for a body the scene does not have, or that is not a request, answers nothing.
        assert_eq!(answer(&serve(&contribution_request(&spec, SourceId::Body(7)))), Ok(Answer::Nothing));
        assert_eq!(answer(&serve(b"\x01nonsense")), Ok(Answer::Nothing));
        // A warning keeps its tower, and "all towers".
        let mut out = Writer::default();
        out.warnings(&[Warning::about(2, "x"), Warning::general("y")]);
        let mut reader = Reader { bytes: &out.0, at: 0 };
        assert_eq!(reader.warnings().unwrap(), [Warning::about(2, "x"), Warning::general("y")]);
    }
}
