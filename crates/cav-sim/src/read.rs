//! From a scene to what its towers read: the shots, each source's contribution, and their sum.
//!
//! The phase is a sum over sources, so each source's contribution is computed on its own
//! ([`contribution`]) and the signal is their sum in a fixed order: the floor first, then the
//! bodies as the scene lists them. The engine keeps the contributions apart, so that moving one
//! body recomputes that body alone, and the sum is the same bits however it was arrived at.
//!
//! Every shot is read until it stops changing (`Reader::read_to`), and the largest of the
//! estimates that leaves is the signal's own error estimate, in radians.

use cav_core::Vec3;
use cav_core::forward::{Channel, Mass, PHASE_TOLERANCE, Reader, Sign, Slab};

use crate::{FloorKind, SceneSpec};

/// The most shots a signal is computed for. A scene with more is refused, with the count.
pub const MAX_SHOTS: usize = 2_000_000;

/// The most values (towers × shots) one channel of a signal holds. A source's contribution is
/// three such arrays of `f64`, 96 MiB at this cap, and so is the signal; a scene that asks for
/// more is refused with the numbers before anything is reserved. The schema allows a thousand
/// towers and two million shots: together they would be 48 GB a source.
pub const MAX_SIGNAL_VALUES: usize = 4_000_000;

/// Whether a signal of `towers` towers and `shots` shots is one the engine makes.
///
/// # Errors
/// When it holds more than [`MAX_SIGNAL_VALUES`] values, with the counts.
pub fn fits(towers: usize, shots: usize) -> Result<(), String> {
    match towers.checked_mul(shots) {
        Some(values) if values <= MAX_SIGNAL_VALUES => Ok(()),
        _ => Err(format!(
            "{towers} tower(s) × {shots} shot(s) is more than the {MAX_SIGNAL_VALUES} values a signal holds;              shorten the timeline, lengthen the cycle or use fewer towers"
        )),
    }
}

/// One thing in a scene that has mass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SourceId {
    /// The floor.
    Floor,
    /// Body `n` of the scene, from 0.
    Body(usize),
}

/// The sources of a scene, in the order their contributions are summed.
#[must_use]
pub fn sources(spec: &SceneSpec) -> Vec<SourceId> {
    spec.floor.iter().map(|_| SourceId::Floor).chain((0..spec.bodies.len()).map(SourceId::Body)).collect()
}

/// The shots of a scene: when each tower reads each of its shots out, and which it takes at all.
///
/// **Each tower has its own shots.** A tower flies its own instrument
/// ([`crate::ArraySpec::instrument_of`]): its own cycle and its own 2T. So the towers need not
/// have as many shots as each other, and what is read is ragged, stored flat: tower `d`'s shots
/// are entries `starts[d]..starts[d + 1]` of `clock` and `valid` (CSR). [`Self::count`],
/// [`Self::times`], [`Self::readout`] and the rest take a tower and a shot of that tower, and
/// every caller goes through them.
///
/// `clock` is when the array's clock reads each shot out: the launch the array's schedule gives
/// at the tower's cycle, and the tower's 2T. Tower `d` launches its shot `k` at
/// `clock[starts[d] + k] + offsets[d] − 2T_d` and reads it out at `clock[…] + offsets[d]`. A shot
/// a tower does not take (a planned gap, a lost shot) keeps its place and is marked not valid.
/// Towers that fly the array's instrument have the same clock, shot for shot and bit for bit.
///
/// This is the one source of shot times and of which shots are taken: the read, the timeline's
/// lanes, the flight diagram, the signal plot and every caption that counts shots go by it.
#[derive(Clone, Debug, PartialEq)]
pub struct Shots {
    /// When the array's clock reads each shot out, seconds, tower after tower (see `starts`):
    /// the launch plus the tower's 2T. They rise within a tower.
    pub clock: Vec<f64>,
    /// Where each tower's shots begin in `clock` and `valid`, and, last, where the last tower's
    /// end: one more entry than there are towers, the first 0.
    pub starts: Vec<usize>,
    /// How much later than its clock each tower reads out, seconds, in the towers' order.
    pub offsets: Vec<f64>,
    /// Whether each tower takes each of its shots, laid out as `clock`.
    pub valid: Vec<bool>,
    /// How long each tower's shot is in flight, seconds: its 2T, from launch to readout.
    pub flights: Vec<f64>,
    /// What a person should know about the schedule: a tower whose offset puts shots outside
    /// the timeline. The signal read from these shots carries them.
    pub warnings: Vec<Warning>,
}

impl Default for Shots {
    /// No towers and no shots.
    fn default() -> Self {
        Self {
            clock: Vec::new(),
            starts: vec![0],
            offsets: Vec::new(),
            valid: Vec::new(),
            flights: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

/// One shot of one tower, as it is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flown {
    /// Which shot of the tower's schedule it is, from 0.
    pub k: usize,
    /// When the tower launches it, seconds.
    pub launch: f64,
    /// When the tower reads it out, seconds: the launch and the tower's 2T.
    pub readout: f64,
    /// Whether the tower takes it.
    pub taken: bool,
}

impl Shots {
    /// Shots that every tower reads out at `times` (readout times, seconds on the scene's clock,
    /// in the order given), all of them taken: the towers of `spec` with no offsets, whatever the
    /// scene's own schedule and offsets are. For reading a scene at times of one's choosing; not
    /// the scene's own shots, which are [`shots`].
    #[must_use]
    pub fn together(spec: &SceneSpec, times: Vec<f64>) -> Self {
        let towers = spec.array.towers.len();
        let count = times.len();
        let mut clock = Vec::with_capacity(towers * count);
        for _ in 0..towers {
            clock.extend_from_slice(&times);
        }
        Self {
            valid: vec![true; towers * count],
            starts: (0..=towers).map(|d| d * count).collect(),
            offsets: vec![0.0; towers],
            clock,
            flights: (0..towers).map(|d| 2.0 * spec.array.instrument_of(d).half_sequence).collect(),
            warnings: Vec::new(),
        }
    }

    /// How many towers there are.
    #[must_use]
    pub fn towers(&self) -> usize {
        self.offsets.len()
    }

    /// How many shots tower `tower` (from 0) has, taken or not.
    #[must_use]
    pub fn count(&self, tower: usize) -> usize {
        self.starts[tower + 1] - self.starts[tower]
    }

    /// How many shots there are over every tower: the length of `clock`.
    #[must_use]
    pub fn total(&self) -> usize {
        self.clock.len()
    }

    /// The most shots any one tower has: the rows of the padded view of a bundle. Zero with no
    /// towers.
    #[must_use]
    pub fn most(&self) -> usize {
        (0..self.towers()).map(|d| self.count(d)).max().unwrap_or(0)
    }

    /// Tower `tower`'s shots read out on the array's clock (before its offset), seconds.
    #[must_use]
    pub fn times(&self, tower: usize) -> &[f64] {
        &self.clock[self.starts[tower]..self.starts[tower + 1]]
    }

    /// Tower `tower`'s 2T, seconds.
    #[must_use]
    pub fn flight(&self, tower: usize) -> f64 {
        self.flights[tower]
    }

    /// When tower `tower` (from 0) reads its shot `k` out, seconds. With no offset this is its
    /// clock's time to the bit: adding 0.0 changes nothing.
    #[must_use]
    pub fn readout(&self, tower: usize, k: usize) -> f64 {
        self.clock[self.starts[tower] + k] + self.offsets[tower]
    }

    /// Whether tower `tower` takes its shot `k`.
    #[must_use]
    pub fn taken(&self, tower: usize, k: usize) -> bool {
        self.valid[self.starts[tower] + k]
    }

    /// Whether tower `tower` takes each of its shots, in shot order.
    #[must_use]
    pub fn taken_all(&self, tower: usize) -> &[bool] {
        &self.valid[self.starts[tower]..self.starts[tower + 1]]
    }

    /// When tower `tower` (from 0) launches its shot `k`, seconds: its readout less its 2T.
    #[must_use]
    pub fn launch(&self, tower: usize, k: usize) -> f64 {
        self.readout(tower, k) - self.flights[tower]
    }

    /// Shot `k` of tower `tower` (from 0), as it is drawn.
    #[must_use]
    pub fn flown(&self, tower: usize, k: usize) -> Flown {
        let readout = self.readout(tower, k);
        Flown { k, launch: readout - self.flights[tower], readout, taken: self.taken(tower, k) }
    }

    /// The shots of tower `tower` (from 0) any part of whose flight is inside `[from, to]`
    /// seconds: launched at or before `to` and read out at or after `from`. One stretch, since
    /// the times rise; found by bisection, so it costs the same for six shots and two million.
    #[must_use]
    pub fn touching(&self, tower: usize, [from, to]: [f64; 2]) -> std::ops::Range<usize> {
        let (offset, flight) = (self.offsets[tower], self.flights[tower]);
        let times = self.times(tower);
        let first = times.partition_point(|time| time + offset < from);
        let end = times.partition_point(|time| time + offset - flight <= to);
        first..end.max(first)
    }
}

/// When the array's clock launches each shot, seconds, by the scene's schedule at the array's
/// own cycle: [`launches_at`] with `spec.array.instrument.cycle`.
///
/// # Errors
/// As [`launches_at`].
pub fn launches(spec: &SceneSpec) -> Result<Vec<f64>, String> {
    let most = most_launches(spec, spec.array.instrument.cycle)?;
    fits(spec.array.towers.len().max(1), most)?;
    Ok(launches_at(spec, spec.array.instrument.cycle, most))
}

/// The most shots the schedule can launch on the timeline at a cycle of `cycle` seconds: one
/// every shortest cycle the schedule allows. (For the plain schedule this is the count itself.)
///
/// # Errors
/// When it is more than [`MAX_SHOTS`].
pub fn most_launches(spec: &SceneSpec, cycle: f64) -> Result<usize, String> {
    let schedule = &spec.array.schedule;
    let most = (spec.duration / schedule.shortest_cycle(cycle).max(f64::MIN_POSITIVE)).ceil();
    if !(most.is_finite() && most <= MAX_SHOTS as f64) {
        return Err(format!(
            "the timeline holds {most} shots ({} s at one every {} s); a signal is computed for at most \
             {MAX_SHOTS}",
            spec.duration,
            schedule.shortest_cycle(cycle)
        ));
    }
    Ok(most as usize)
}

/// When the array's clock launches each shot of a tower whose cycle is `cycle` seconds, by the
/// scene's schedule: one every cycle from time zero, or as the alternation and the jitter have
/// it, while the shot's place on the clock is before the end of the timeline. With the plain
/// schedule these are `k·cycle` exactly, as S1 had them. At most `most` of them
/// ([`most_launches`], which the caller has counted against its budget). The jitter is keyed by
/// the seed and the shot alone, so towers of one cycle launch together.
///
/// **No launch is before time zero.** The clock starts at zero and nothing is launched before
/// it starts. Jitter that can only delay never does that; symmetric jitter about a grid would
/// (shot 0 drawn early), so there the grid stands `delta` after zero: shot `k` is launched at
/// `k·cycle + delta + delta·(2u − 1)`, in `[k·cycle, k·cycle + 2·delta)`. A shift of every shot
/// alike changes no height of the spectral window. A shot's launch may be later than the end of
/// the timeline by its jitter, and its readout by 2T more: it is the timeline's shot because its
/// place on the grid is.
#[must_use]
pub fn launches_at(spec: &SceneSpec, cycle: f64, most: usize) -> Vec<f64> {
    use crate::{JitterKind, JitterModel};
    use cav_core::cadence::{STREAM_JITTER, unit};
    let schedule = &spec.array.schedule;
    // The launch of shot k on a clock that keeps time: k·cycle, or the two cycle times in turn.
    // A product, not a running sum, so the thousandth shot is as exact as the first.
    let grid = |k: usize| match schedule.alternate {
        None => k as f64 * cycle,
        Some(other) => (k / 2) as f64 * (cycle + other) + (k % 2) as f64 * cycle,
    };
    let draw = |k: usize, kind: JitterKind| {
        let u = unit(schedule.seed, STREAM_JITTER, 0, k as u64);
        if kind == JitterKind::Symmetric { 2.0 * u - 1.0 } else { u }
    };
    let mut times = Vec::with_capacity(most);
    match schedule.jitter {
        Some(jitter) if jitter.model == JitterModel::Walk => {
            // Each cycle is drawn afresh and the launches are their running sum: a late shot
            // delays every later one. Summed in shot order, so the bits are the same anywhere.
            let mut now = 0.0;
            for k in 0..most {
                if now >= spec.duration {
                    break;
                }
                times.push(now);
                let own = if k % 2 == 1 { schedule.alternate.unwrap_or(cycle) } else { cycle };
                now += own + jitter.delta * draw(k, jitter.kind);
            }
        }
        jitter => {
            for k in 0..most {
                let late = jitter.map_or(0.0, |j| j.delta * draw(k, j.kind));
                // A shot is one of the timeline's if its place on the clock is before the end.
                if grid(k) >= spec.duration {
                    break;
                }
                // The symmetric grid stands `delta` late (see above): the unshifted time first,
                // as `cav_core::cadence::grid_times` has it, and then the shift.
                let shift = jitter.filter(|j| j.kind == JitterKind::Symmetric).map_or(0.0, |j| j.delta);
                times.push(match (late == 0.0, shift == 0.0) {
                    (true, true) => grid(k),
                    (_, true) => grid(k) + late,
                    _ => (grid(k) + late) + shift,
                });
            }
        }
    }
    times
}

/// When the array's clock reads each shot of the array's own instrument out, seconds: its
/// launch ([`launches`]) and 2T. A tower with an instrument of its own has its own
/// ([`Shots::times`]).
///
/// # Errors
/// As [`launches`].
pub fn readouts(spec: &SceneSpec) -> Result<Vec<f64>, String> {
    let flight = 2.0 * spec.array.instrument.half_sequence;
    Ok(launches(spec)?.into_iter().map(|launch| launch + flight).collect())
}

/// The scene's shots: each tower's clock (the array's schedule at the tower's own cycle, read
/// out at its own 2T), each tower's offset from it, and which shots each tower takes: those inside
/// the planned pattern and the daily window, less the ones lost (each tower loses its own, keyed
/// by seed, tower and shot).
///
/// **A tower's offset moves its shots and nothing else.** Every shot stays as it was, taken or
/// not, and is read at the tower's own time, either side of the timeline's ends (an offset may
/// be negative: that tower's clock is ahead of the array's). When an offset puts a launch before
/// time zero, or at or after the end of the timeline a shot the array's clock launches before
/// it, the shots carry a warning about that tower, and so does the signal read from them.
///
/// # Errors
/// When a tower could have more than [`MAX_SHOTS`] shots, or the towers together more than
/// [`MAX_SIGNAL_VALUES`]. Every tower's count is taken from the shortest time between launches
/// the schedule allows at its cycle, and their sum checked, before anything is reserved.
pub fn shots(spec: &SceneSpec) -> Result<Shots, String> {
    use cav_core::cadence::{STREAM_LOSS, unit};
    let schedule = &spec.array.schedule;
    let instruments = spec.array.instruments();
    // Each tower's most, and their sum, before anything is made.
    let mut most = Vec::with_capacity(instruments.len());
    for instrument in &instruments {
        most.push(most_launches(spec, instrument.cycle)?);
    }
    let budget = most.iter().try_fold(0usize, |sum, n| sum.checked_add(*n)).unwrap_or(usize::MAX);
    if budget > MAX_SIGNAL_VALUES {
        return Err(format!(
            "{} tower(s) with up to {budget} shot(s) between them is more than the {MAX_SIGNAL_VALUES} values a \
             signal holds; shorten the timeline, lengthen the cycle or use fewer towers",
            instruments.len()
        ));
    }
    let offsets: Vec<f64> = spec.array.towers.iter().map(|tower| tower.timing.offset).collect();
    let (mut clock, mut valid) = (Vec::with_capacity(budget), Vec::with_capacity(budget));
    let mut starts = Vec::with_capacity(instruments.len() + 1);
    starts.push(0);
    let mut flights = Vec::with_capacity(instruments.len());
    let mut warnings = Vec::new();
    // Towers of one cycle launch together: the launches are made once a cycle, by its bits.
    let mut made: Vec<(u64, Vec<f64>)> = Vec::new();
    for (tower, instrument) in instruments.iter().enumerate() {
        let key = instrument.cycle.to_bits();
        if !made.iter().any(|(cycle, _)| *cycle == key) {
            made.push((key, launches_at(spec, instrument.cycle, most[tower])));
        }
        let launches = &made.iter().find(|(cycle, _)| *cycle == key).expect("made above").1;
        let flight = 2.0 * instrument.half_sequence;
        // What the plan itself leaves out, the same for every tower of this cycle.
        let planned = |k: usize| {
            let in_pattern = schedule.gaps.is_none_or(|gaps| (k % gaps.period as usize) < gaps.on as usize);
            let in_window = schedule.window.is_none_or(|window| {
                let phase = launches[k].rem_euclid(window.period) - window.start;
                // The window may run past the end of the period into the next.
                (0.0..window.length).contains(&phase) || phase + window.period < window.length
            });
            in_pattern && in_window
        };
        valid.extend((0..launches.len()).map(|k| {
            planned(k)
                && (schedule.loss == 0.0
                    || unit(schedule.seed, STREAM_LOSS, tower as u64, k as u64) >= schedule.loss)
        }));
        clock.extend(launches.iter().map(|launch| launch + flight));
        starts.push(clock.len());
        flights.push(flight);
        let offset = offsets[tower];
        if offset != 0.0 {
            let label = &spec.array.towers[tower].label;
            // The launches rise, so those an offset puts outside are a stretch at one end.
            let early = launches.partition_point(|launch| launch + offset < 0.0);
            if early > 0 {
                warnings.push(Warning::about(
                    tower,
                    format!(
                        "{label}'s clock is offset by {offset} s: {early} of its {} shot(s) are launched before \
                         the timeline begins, the first at {} s",
                        launches.len(),
                        launches[0] + offset
                    ),
                ));
            }
            let inside = launches.partition_point(|launch| *launch < spec.duration);
            let late = inside - launches[..inside].partition_point(|launch| launch + offset < spec.duration);
            if late > 0 {
                warnings.push(Warning::about(
                    tower,
                    format!(
                        "{label}'s clock is offset by {offset} s: {late} of its {} shot(s) are launched at or after \
                         the end of the timeline ({} s), the last at {} s",
                        launches.len(),
                        spec.duration,
                        launches[inside - 1] + offset
                    ),
                ));
            }
        }
    }
    Ok(Shots { clock, starts, offsets, valid, flights, warnings })
}

/// Something a person should know about a reading, and the tower it is about.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Warning {
    /// The tower it concerns, from 0 in the scene's order; `None` when it concerns them all.
    pub tower: Option<usize>,
    /// The warning, in words.
    pub text: String,
}

impl Warning {
    /// A warning about every tower.
    #[must_use]
    pub fn general(text: impl Into<String>) -> Self {
        Self { tower: None, text: text.into() }
    }

    /// A warning about tower `tower` (from 0).
    #[must_use]
    pub fn about(tower: usize, text: impl Into<String>) -> Self {
        Self { tower: Some(tower), text: text.into() }
    }

    /// Whether the warning is about tower `tower` (from 0), or about all of them.
    #[must_use]
    pub fn concerns(&self, tower: usize) -> bool {
        self.tower.is_none_or(|own| own == tower)
    }
}

impl std::ops::Deref for Warning {
    type Target = str;
    fn deref(&self) -> &str {
        &self.text
    }
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

/// One tower's three phases from one shot, radians. Interferometer 1 is the lower one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Phases {
    /// δφ₁, the lower interferometer alone.
    pub lower: f64,
    /// δφ₂, the upper interferometer alone.
    pub upper: f64,
    /// ΔΦ, the gradiometer, with the instrument's sign (lower minus upper by default).
    pub gradiometer: f64,
}

/// What one source adds to every tower's every shot.
#[derive(Clone, Debug, PartialEq)]
pub struct Contribution {
    /// δφ₁ of each shot of each tower, radians: tower by tower, `towers × shots` values.
    pub lower: Vec<f64>,
    /// δφ₂ likewise.
    pub upper: Vec<f64>,
    /// ΔΦ likewise, with the instrument's sign.
    pub gradiometer: Vec<f64>,
    /// The largest quadrature estimate of any shot, radians.
    pub estimate: f64,
    /// The same for each tower on its own, radians, in the towers' order.
    pub estimates: Vec<f64>,
    /// Whether every shot's read settled to the tolerance. When false, `estimate` is not a bound
    /// that was met but how far the worst read was from settling.
    pub settled: bool,
    /// What a person should know about this source's readings.
    pub warnings: Vec<Warning>,
}

fn base(position: [f64; 3]) -> Vec3 {
    Vec3::from_array(position)
}

/// The body as the forward model takes it.
fn mass(spec: &SceneSpec, n: usize) -> Mass<impl Fn(f64) -> Vec3 + Sync + '_> {
    let body = &spec.bodies[n];
    let gravity = spec.array.instrument.gravity;
    Mass {
        mass: body.mass,
        radius: body.radius,
        position: move |t| Vec3::from_array(body.motion.position(t, gravity)),
        exists: body.exists(),
        breaks: body.motion.breaks(),
    }
}

/// The key an instrument's reader is shared by: every field of the forward model's
/// [`cav_core::forward::Instrument`] by its bits (with −0 taken as +0), and its enums by
/// variant. Two towers whose instruments give the same key read alike, bit for bit, and share one
/// [`Reader`].
#[must_use]
pub fn instrument_key(instrument: &cav_core::forward::Instrument) -> Vec<u64> {
    use cav_core::forward::Quadrature;
    let bits = |x: f64| if x == 0.0 { 0 } else { x.to_bits() };
    let mut key = vec![
        bits(instrument.ifo_separation),
        bits(instrument.half_sequence),
        bits(instrument.launch_velocity),
        bits(instrument.gravity),
        bits(instrument.atom_mass),
        bits(instrument.hbar),
        bits(instrument.big_g),
        bits(instrument.wavelength),
        bits(instrument.kicks),
        u64::from(instrument.floor),
        match instrument.sign {
            Sign::LowerMinusUpper => 0,
            Sign::UpperMinusLower => 1,
        },
    ];
    match instrument.quadrature {
        Quadrature::Gauss { order, panel } => key.extend([0, order as u64, bits(panel)]),
        Quadrature::Simpson { step } => key.extend([1, bits(step)]),
        Quadrature::ScriptTrapezoid { step, nodes, lag } => {
            key.extend([2, bits(step), nodes as u64, lag as u64])
        }
    }
    key
}

/// A reader for each distinct instrument of an array, and which one each tower uses: towers that
/// fly the same instrument ([`instrument_key`]) share one, by `Arc`, across threads. At most one
/// a tower, each a few kilobytes (its Gauss rule).
#[derive(Clone, Debug)]
pub struct Readers {
    /// One reader for each distinct instrument, in the order of the first tower that flies it.
    pub readers: Vec<std::sync::Arc<Reader>>,
    /// For each tower, in the towers' order, the index of its reader in `readers`.
    pub of_tower: Vec<usize>,
}

impl Readers {
    /// The readers of `spec`'s towers.
    #[must_use]
    pub fn new(spec: &SceneSpec) -> Self {
        let mut keys: Vec<Vec<u64>> = Vec::new();
        let mut readers = Vec::new();
        let mut of_tower = Vec::with_capacity(spec.array.towers.len());
        for tower in 0..spec.array.towers.len() {
            let instrument = spec.array.instrument_of(tower).instrument();
            let key = instrument_key(&instrument);
            let index = keys.iter().position(|k| *k == key).unwrap_or_else(|| {
                keys.push(key);
                readers.push(std::sync::Arc::new(Reader::new(instrument)));
                readers.len() - 1
            });
            of_tower.push(index);
        }
        Self { readers, of_tower }
    }

    /// The reader of tower `tower` (from 0).
    #[must_use]
    pub fn of(&self, tower: usize) -> &Reader {
        &self.readers[self.of_tower[tower]]
    }
}

/// The contribution of `source` to `shots`: each tower's every shot, read out at that tower's own
/// time. A shot a tower does not take contributes exactly zero. `cancelled` is asked between
/// shots; when it says yes the work stops and nothing is returned.
///
/// # Panics
/// When towers × shots is more than [`MAX_SIGNAL_VALUES`]: [`readouts`] and [`signal_at`] refuse
/// such a scene first, so reaching here with one is a fault in the caller, and nothing has been
/// reserved when it is found.
#[must_use]
pub fn contribution(
    spec: &SceneSpec,
    source: SourceId,
    shots: &Shots,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Option<Contribution> {
    contribution_on(threads(), spec, source, shots, cancelled)
}

/// How many shots of one tower a thread takes at a time. A constant: the cut does not depend on
/// how many threads there are, and nothing is summed across pieces, so neither does any value.
const PIECE: usize = 64;

/// One piece of a contribution: where a tower's consecutive shots from `start` are written.
struct Piece<'a> {
    at: usize,
    start: usize,
    lower: &'a mut [f64],
    upper: &'a mut [f64],
    gradiometer: &'a mut [f64],
}

/// What reading one piece found, besides the values: the largest estimate, the first shot in
/// which the body crossed the atoms' path, and the first that did not settle, with its estimate.
struct Found {
    estimate: f64,
    inside: Option<usize>,
    unsettled: Option<(usize, f64)>,
}

/// How many threads a contribution is read on: `CAV_THREADS` if it is set to a number from 1 to
/// 64, else one fewer than the machine's cores (the window keeps one), at least 1 and at most 8.
/// The number changes how long a read takes and nothing else (`engine.thread_count`).
#[must_use]
pub fn threads() -> usize {
    let asked = std::env::var("CAV_THREADS").ok().and_then(|text| text.trim().parse::<usize>().ok());
    match asked {
        Some(count) if (1..=64).contains(&count) => count,
        _ => {
            std::thread::available_parallelism().map_or(1, |cores| cores.get().saturating_sub(1)).clamp(1, 8)
        }
    }
}

/// [`contribution`] on a stated number of threads.
///
/// # Panics
/// As [`contribution`].
#[must_use]
pub fn contribution_on(
    threads: usize,
    spec: &SceneSpec,
    source: SourceId,
    shots: &Shots,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Option<Contribution> {
    let readers = Readers::new(spec);
    let towers = &spec.array.towers;
    assert_eq!(shots.towers(), towers.len(), "shots of another array");
    let count = shots.total();
    if count > MAX_SIGNAL_VALUES {
        panic!("a contribution was asked for a signal that is refused: {count} values");
    }
    let mut out = Contribution {
        lower: Vec::with_capacity(count),
        upper: Vec::with_capacity(count),
        gradiometer: Vec::with_capacity(count),
        estimate: 0.0,
        estimates: vec![0.0; towers.len()],
        settled: true,
        warnings: Vec::new(),
    };
    // Each tower's own sign: the gradiometer is differenced as its instrument says.
    let signed = |tower: usize, lower: f64, upper: f64| match readers.of(tower).instrument().sign {
        Sign::LowerMinusUpper => lower - upper,
        Sign::UpperMinusLower => upper - lower,
    };
    match source {
        SourceId::Floor => {
            let floor = spec.floor.as_ref()?;
            // The slab's field is uniform above it, so one reading serves every shot; it is read
            // for each tower all the same, so that the null below is what the forward model gives
            // for that tower and not a zero written here.
            for (at, tower) in towers.iter().enumerate() {
                let (lower, upper, difference) = match floor.kind {
                    FloorKind::Slab { thickness } => {
                        let slab = Slab { density: floor.density, thickness };
                        let reading = readers.of(at).read(base(tower.position), &slab, 0.0);
                        (reading.lower, reading.upper, signed(at, reading.lower, reading.upper))
                    }
                    FloorKind::HalfSpace => {
                        // Each interferometer's own phase grows without bound with the depth of
                        // matter; their difference does not depend on the depth at all, and is
                        // read from a slab one metre thick.
                        let unit = readers.of(at).read(
                            base(tower.position),
                            &Slab { density: floor.density, thickness: 1.0 },
                            0.0,
                        );
                        (f64::INFINITY, f64::INFINITY, signed(at, unit.lower, unit.upper))
                    }
                };
                // A shot that is not taken reads nothing; the others all read the same.
                let taken = |value: f64| {
                    (0..shots.count(at)).map(move |k| if shots.taken(at, k) { value } else { 0.0 })
                };
                out.lower.extend(taken(lower));
                out.upper.extend(taken(upper));
                out.gradiometer.extend(taken(difference));
            }
            if floor.kind == FloorKind::HalfSpace {
                out.warnings.push(Warning::general(
                    "the floor is a half-space: its uniform field is unbounded, so each interferometer's \
                     own phase is infinite; it is the same for both and the gradiometer reads none of it",
                ));
            }
        }
        SourceId::Body(n) => {
            let body = mass(spec, n);
            let label = &spec.bodies[n].label;
            // Every value has its place before any is computed: the three channels are cut into
            // pieces of one tower's consecutive shots, and each piece is filled by whichever
            // thread takes it. A shot's value depends on the scene and its own readout time and
            // on nothing another thread does, so the bits are the same at any thread count.
            for channel in [&mut out.lower, &mut out.upper, &mut out.gradiometer] {
                channel.resize(count, 0.0);
            }
            let mut pieces = Vec::new();
            let (mut lower_rest, mut upper_rest, mut gradiometer_rest) =
                (&mut out.lower[..], &mut out.upper[..], &mut out.gradiometer[..]);
            for at in 0..towers.len() {
                // Tower by tower, each its own number of shots (CSR, as the shots are laid out).
                let own = shots.count(at);
                let (lower, rest) = std::mem::take(&mut lower_rest).split_at_mut(own);
                lower_rest = rest;
                let (upper, rest) = std::mem::take(&mut upper_rest).split_at_mut(own);
                upper_rest = rest;
                let (gradiometer, rest) = std::mem::take(&mut gradiometer_rest).split_at_mut(own);
                gradiometer_rest = rest;
                let cut =
                    lower.chunks_mut(PIECE).zip(upper.chunks_mut(PIECE)).zip(gradiometer.chunks_mut(PIECE));
                for (piece, ((lower, upper), gradiometer)) in cut.enumerate() {
                    pieces.push(Piece { at, start: piece * PIECE, lower, upper, gradiometer });
                }
            }
            // One piece: the shots of one tower from `start`, read in order.
            let read_piece = |piece: Piece<'_>| -> Option<Found> {
                let tower = &towers[piece.at];
                let mut found = Found { estimate: 0.0, inside: None, unsettled: None };
                for step in 0..piece.lower.len() {
                    let k = piece.start + step;
                    if cancelled() {
                        return None;
                    }
                    if !shots.taken(piece.at, k) {
                        continue;
                    }
                    // The tower's own readout: the array's, and the tower's offset.
                    let readout = shots.readout(piece.at, k);
                    // The body is watched as it is read: the crossing check costs no node of its own.
                    let watched = Watched::new(&body);
                    let audited = readers.of(piece.at).read_to(
                        base(tower.position),
                        &watched,
                        readout,
                        PHASE_TOLERANCE,
                    );
                    let (lower, upper) = (audited.reading.lower, audited.reading.upper);
                    let estimate = audited.estimate.unwrap_or(0.0);
                    // Whatever stopped the audit short of its tolerance (the halvings, the work
                    // budget, a reading that is not a number) is said, never passed off as refined.
                    if !audited.settled {
                        found.unsettled.get_or_insert((k, estimate));
                    }
                    if found.inside.is_none() && watched.crossed() {
                        found.inside = Some(k);
                    }
                    found.estimate =
                        found.estimate.max(if estimate.is_nan() { f64::INFINITY } else { estimate });
                    piece.lower[step] = lower;
                    piece.upper[step] = upper;
                    piece.gradiometer[step] = signed(piece.at, lower, upper);
                }
                Some(found)
            };
            // The pieces are handed out from one queue; what each found comes back with its
            // number, and is put together in that order whoever finished first.
            let workers = threads.min(pieces.len()).max(1);
            let meta: Vec<usize> = pieces.iter().map(|piece| piece.at).collect();
            let queue = std::sync::Mutex::new(pieces.into_iter().enumerate());
            let results = std::sync::Mutex::new(Vec::with_capacity(meta.len()));
            let stopped = std::sync::atomic::AtomicBool::new(false);
            let work = || {
                loop {
                    let next = queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner).next();
                    let Some((index, piece)) = next else { break };
                    if stopped.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    match read_piece(piece) {
                        Some(found) => {
                            results
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .push((index, found));
                        }
                        None => {
                            stopped.store(true, std::sync::atomic::Ordering::Relaxed);
                            break;
                        }
                    }
                }
            };
            if workers == 1 {
                work();
            } else {
                std::thread::scope(|scope| {
                    for _ in 1..workers {
                        scope.spawn(work);
                    }
                    work();
                });
            }
            if stopped.load(std::sync::atomic::Ordering::Relaxed) {
                return None;
            }
            let mut results = results.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner);
            results.sort_by_key(|(index, _)| *index);
            // Tower by tower, piece by piece in shot order: the first shot that crossed, the
            // first that did not settle, the largest estimate.
            for (at, tower) in towers.iter().enumerate() {
                let (mut inside, mut unsettled) = (None, None);
                for (_, found) in results.iter().filter(|(index, _)| meta[*index] == at) {
                    out.estimate = out.estimate.max(found.estimate);
                    out.estimates[at] = out.estimates[at].max(found.estimate);
                    inside = inside.or(found.inside);
                    unsettled = unsettled.or(found.unsettled);
                }
                if unsettled.is_some() {
                    out.settled = false;
                }
                if let Some(k) = inside {
                    out.warnings.push(Warning::about(
                        at,
                        format!(
                            "`{label}` passes through the atoms' path of {} during shot {} (read out at {} s)",
                            tower.label,
                            k + 1,
                            shots.readout(at, k)
                        ),
                    ));
                }
                if let Some((k, estimate)) = unsettled {
                    let point = if spec.bodies[n].radius == 0.0 {
                        " (a point mass on the atoms' path has no finite phase)"
                    } else {
                        ""
                    };
                    out.warnings.push(Warning::about(
                        at,
                        format!(
                            "`{label}` at {}: shot {} did not settle; its quadrature estimate is \
                             {estimate:.1e} rad{point}",
                            tower.label,
                            k + 1
                        ),
                    ));
                }
            }
        }
    }
    Some(out)
}

/// A body watched while it is read: it is the body as a [`Source`], and it notes whether its
/// surface (or, for a point, a millimetre about it) came between the two arms of an
/// interferometer at any node the read evaluated.
///
/// The check rides on the read's own nodes, so it is inside the read's budget of work
/// ([`cav_core::forward::MAX_NODES`]) by construction. (In S1 it was a second pass over the
/// flight at a quarter of the spacing: up to 480,000 nodes that no budget counted.)
struct Watched<'a, P: Fn(f64) -> Vec3 + Sync> {
    body: &'a Mass<P>,
    /// How near the atoms' path counts as on it, metres.
    reach: f64,
    crossed: std::sync::atomic::AtomicBool,
}

impl<'a, P: Fn(f64) -> Vec3 + Sync> Watched<'a, P> {
    fn new(body: &'a Mass<P>) -> Self {
        Self { body, reach: body.radius.max(1e-3), crossed: std::sync::atomic::AtomicBool::new(false) }
    }

    /// Whether the body came between an interferometer's arms during the reads so far.
    fn crossed(&self) -> bool {
        self.crossed.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl<P: Fn(f64) -> Vec3 + Sync> cav_core::forward::Source for Watched<'_, P> {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, time: f64) -> [f64; 2] {
        if time >= self.body.exists.0 && time < self.body.exists.1 && !self.crossed() {
            let offset = (self.body.position)(time) - tower;
            let reach = self.reach;
            // Between the two arms of an interferometer, give or take the body's reach.
            if offset.x.hypot(offset.y) <= reach
                && heights
                    .iter()
                    .any(|height| offset.z >= height + lower - reach && offset.z <= height + upper + reach)
            {
                self.crossed.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        self.body.difference(tower, heights, lower, upper, time)
    }

    fn breaks(&self, from: f64, to: f64, out: &mut Vec<f64>) {
        self.body.breaks(from, to, out);
    }
}

/// What every tower reads over the timeline.
///
/// Laid out as its [`Shots`] are: tower after tower, each its own number of shots (CSR), tower
/// `d`'s values at `starts[d]..starts[d + 1]` of every per-shot array. [`Self::channel`],
/// [`Self::times`], [`Self::readouts`] and [`Self::taken`] give one tower's.
#[derive(Clone, Debug, PartialEq)]
pub struct Signal {
    /// When the array's clock reads each shot out, seconds, tower after tower. A tower with an
    /// offset reads out that much later: [`Self::readout`].
    pub clock: Vec<f64>,
    /// Where each tower's shots begin in every per-shot array, and where the last tower's end:
    /// one more entry than there are towers, the first 0.
    pub starts: Vec<usize>,
    /// How many towers there are.
    pub towers: usize,
    /// How much later than its clock each tower reads out, seconds, in the towers' order.
    pub offsets: Vec<f64>,
    /// Each tower's 2T, seconds, in the towers' order.
    pub flights: Vec<f64>,
    /// Whether each tower took each of its shots, laid out as `clock`. A shot not taken holds
    /// zeros, which are not measurements.
    pub valid: Vec<bool>,
    /// δφ₁ of each shot of each tower, radians, laid out as `clock`.
    pub lower: Vec<f64>,
    /// δφ₂ likewise.
    pub upper: Vec<f64>,
    /// ΔΦ likewise, with each tower's sign.
    pub gradiometer: Vec<f64>,
    /// The largest quadrature estimate of any source's any shot, radians.
    pub estimate: f64,
    /// The same for each tower on its own, radians, in the towers' order: the largest estimate
    /// of any one source's any shot at that tower. (The estimates of several sources are not
    /// added: each is a bound on its own read, and the largest says how far to trust the tower.)
    pub estimates: Vec<f64>,
    /// Whether every read of every source settled to the tolerance.
    pub settled: bool,
    /// What a person should know about these readings.
    pub warnings: Vec<Warning>,
}

impl Signal {
    /// A signal of zeros for the scene's towers and shots: what the void reads, and what the sum
    /// starts from.
    #[must_use]
    pub fn silent(shots: &Shots) -> Self {
        let towers = shots.towers();
        let zeros = vec![0.0; shots.total()];
        Self {
            clock: shots.clock.clone(),
            starts: shots.starts.clone(),
            towers,
            offsets: shots.offsets.clone(),
            flights: shots.flights.clone(),
            valid: shots.valid.clone(),
            lower: zeros.clone(),
            upper: zeros.clone(),
            gradiometer: zeros,
            estimate: 0.0,
            estimates: vec![0.0; towers],
            settled: true,
            warnings: shots.warnings.clone(),
        }
    }

    /// Adds a source's contribution. The order of the additions is the order of the calls.
    ///
    /// # Panics
    /// When the contribution is for another number of towers or shots.
    pub fn add(&mut self, contribution: &Contribution) {
        assert_eq!(contribution.gradiometer.len(), self.gradiometer.len(), "another scene's contribution");
        for (sum, part) in [
            (&mut self.lower, &contribution.lower),
            (&mut self.upper, &contribution.upper),
            (&mut self.gradiometer, &contribution.gradiometer),
        ] {
            sum.iter_mut().zip(part).for_each(|(s, p)| *s += p);
        }
        self.estimate = self.estimate.max(contribution.estimate);
        for (own, part) in self.estimates.iter_mut().zip(&contribution.estimates) {
            *own = own.max(*part);
        }
        self.settled &= contribution.settled;
        self.warnings.extend(contribution.warnings.iter().cloned());
    }

    /// How many shots tower `tower` (from 0) has, taken or not.
    #[must_use]
    pub fn count(&self, tower: usize) -> usize {
        self.starts[tower + 1] - self.starts[tower]
    }

    /// The most shots any one tower has; zero with no towers.
    #[must_use]
    pub fn most(&self) -> usize {
        (0..self.towers).map(|d| self.count(d)).max().unwrap_or(0)
    }

    /// Tower `tower`'s shots on the array's clock (before its offset), seconds.
    #[must_use]
    pub fn times(&self, tower: usize) -> &[f64] {
        &self.clock[self.starts[tower]..self.starts[tower + 1]]
    }

    /// When tower `tower` (from 0) read its shot `k` out, seconds.
    #[must_use]
    pub fn readout(&self, tower: usize, k: usize) -> f64 {
        self.clock[self.starts[tower] + k] + self.offsets[tower]
    }

    /// Tower `tower`'s own readout times, seconds, one for each of its shots.
    #[must_use]
    pub fn readouts(&self, tower: usize) -> Vec<f64> {
        let offset = self.offsets[tower];
        self.times(tower).iter().map(|time| time + offset).collect()
    }

    /// Whether tower `tower` took each of its shots.
    ///
    /// # Panics
    /// When there is no such tower.
    #[must_use]
    pub fn taken(&self, tower: usize) -> &[bool] {
        &self.valid[self.starts[tower]..self.starts[tower + 1]]
    }

    /// One channel of one tower (from 0): a value for each of its shots.
    ///
    /// # Panics
    /// When there is no such tower.
    #[must_use]
    pub fn channel(&self, tower: usize, channel: Channel) -> &[f64] {
        assert!(tower < self.towers, "there is no tower {}", tower + 1);
        let all = match channel {
            Channel::Gradiometer => &self.gradiometer,
            Channel::Lower => &self.lower,
            Channel::Upper => &self.upper,
        };
        &all[self.starts[tower]..self.starts[tower + 1]]
    }
}
/// The scene's signal, computed here and now: every source's contribution, summed in the order
/// of [`sources`].
///
/// # Errors
/// When the timeline holds too many shots.
pub fn signal(spec: &SceneSpec) -> Result<Signal, String> {
    signal_of(spec, &shots(spec)?)
}

/// The signal for the given shots: every source's contribution, summed in the order of
/// [`sources`].
///
/// # Errors
/// When the towers' shots together are more than [`MAX_SIGNAL_VALUES`].
pub fn signal_of(spec: &SceneSpec, shots: &Shots) -> Result<Signal, String> {
    fits(1, shots.total())?;
    let mut total = Signal::silent(shots);
    for source in sources(spec) {
        let part = contribution(spec, source, shots, &|| false).expect("not cancelled");
        total.add(&part);
    }
    Ok(total)
}

/// The signal for shots that every tower reads out together at the given times, which need not
/// be the scene's own (the towers' offsets are not applied).
///
/// # Errors
/// When towers × shots is more than [`MAX_SIGNAL_VALUES`].
pub fn signal_at(spec: &SceneSpec, times: Vec<f64>) -> Result<Signal, String> {
    fits(spec.array.towers.len(), times.len())?;
    signal_of(spec, &Shots::together(spec, times))
}

/// What every tower would read from one shot.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadingAt {
    /// When the shot is read out, seconds.
    pub time: f64,
    /// Each tower's phases, in the towers' order.
    pub towers: Vec<Phases>,
    /// The largest quadrature estimate of any source at any tower, radians.
    pub estimate: f64,
    /// Whether every read settled to the tolerance.
    pub settled: bool,
    /// What a person should know about these numbers: a reading that did not settle, a body in
    /// the atoms' path, an unbounded floor.
    pub warnings: Vec<Warning>,
}

/// What each tower would read from a shot read out at `time`: the live reading under the
/// playhead, with the audit's estimate and its warnings. `cancelled` is asked as it goes; when it
/// says yes the work stops and nothing is returned.
#[must_use]
pub fn reading(spec: &SceneSpec, time: f64, cancelled: &(dyn Fn() -> bool + Sync)) -> Option<ReadingAt> {
    let shot = Shots::together(spec, vec![time]);
    let mut total = Signal::silent(&shot);
    for source in sources(spec) {
        total.add(&contribution(spec, source, &shot, cancelled)?);
    }
    let towers = (0..total.towers)
        .map(|t| Phases { lower: total.lower[t], upper: total.upper[t], gradiometer: total.gradiometer[t] })
        .collect();
    Some(ReadingAt {
        time,
        towers,
        estimate: total.estimate,
        settled: total.settled,
        warnings: total.warnings,
    })
}

/// A scene with only the listed bodies kept, for reading sources apart.
#[must_use]
pub fn only(spec: &SceneSpec, bodies: &[usize]) -> SceneSpec {
    let mut scene = spec.clone();
    scene.bodies = bodies.iter().map(|&n| spec.bodies[n].clone()).collect();
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BodySpec, FloorSpec, dev};
    use cav_core::forward::Source;
    use cav_validate::Measured;

    const CHANNELS: [Channel; 3] = Channel::ALL;

    fn peak(values: &[f64]) -> f64 {
        values.iter().fold(0.0f64, |m, v| m.max(v.abs()))
    }

    /// The largest difference between two series, as a fraction of the second's peak.
    fn worst_of_peak(a: &[f64], b: &[f64]) -> f64 {
        assert_eq!(a.len(), b.len());
        a.iter().zip(b).fold(0.0f64, |m, (x, y)| m.max((x - y).abs())) / peak(b)
    }

    #[test]
    fn shots_are_the_timeline_s_shots() {
        let spec = SceneSpec::room();
        let times = readouts(&spec).unwrap();
        assert_eq!(times, [1.46, 3.46, 5.46, 7.46, 9.46, 11.46], "launched at 0, 2 … 10 s, read 2T later");
        // A timeline that ends inside a cycle still launches that shot.
        let longer = SceneSpec { duration: 12.5, ..SceneSpec::room() };
        assert_eq!(readouts(&longer).unwrap().len(), 7);
        // Control: too many shots are refused with the count, not attempted.
        let mut huge = SceneSpec { duration: 1.0e6, ..SceneSpec::room() };
        huge.array.instrument.half_sequence = 0.001;
        huge.array.instrument.cycle = 0.0021;
        assert_eq!(huge.validate(), Ok(()));
        assert!(readouts(&huge).unwrap_err().contains("at most 2000000"));
    }

    cav_validate::case! {
        test: validation_void_reads_exactly_zero,
        name: "nulls.void",
        subsystem: "nulls",
        slice: "S1",
        reference: Exact,
        against: "the void: the array and nothing else; values of any channel of any tower's any shot that are not exactly zero",
        unit: Count,
        tolerance: 0.0,
        measure: {
            let count = |spec: &SceneSpec| {
                let signal = signal(spec).unwrap();
                assert_eq!(signal.times(0).len() * signal.towers, 30, "five towers, six shots");
                let all = signal.lower.iter().chain(&signal.upper).chain(&signal.gradiometer);
                all.filter(|v| v.to_bits() != 0).count()
            };
            // Control: one kilogram anywhere, and the count is not zero.
            let mut touched = dev::void();
            touched.bodies.push(BodySpec::point("speck", 1.0, [50.0, 50.0, 50.0]));
            assert_eq!(count(&touched), 90, "every value of every channel feels a speck 87 m away");
            Measured::Error(count(&dev::void()) as f64)
        },
    }

    cav_validate::case! {
        test: validation_infinite_floor_null,
        name: "nulls.infinite_floor",
        subsystem: "nulls",
        slice: "S1",
        reference: ClosedForm,
        against: "an infinite uniform slab (and the half-space) gives uniform g: the forward model's reading of the slab at each of five towers at different heights, lower minus upper; values over five towers and six shots that are not exactly zero",
        unit: Count,
        tolerance: 0.0,
        measure: {
            let nonzero = |spec: &SceneSpec| {
                signal(spec).unwrap().gradiometer.iter().filter(|v| v.to_bits() != 0).count()
            };
            // The half-space of the dev scene, and a slab under towers standing at different heights.
            let mut slab = dev::infinite_floor();
            slab.floor = Some(FloorSpec { height: -0.3, kind: FloorKind::Slab { thickness: 1.0 }, density: 2400.0 });
            for (n, tower) in slab.array.towers.iter_mut().enumerate() {
                tower.position[2] = 0.37 * n as f64;
            }
            assert_eq!(slab.validate(), Ok(()));
            let read = signal(&slab).unwrap();
            // Each interferometer does feel the slab: k_eff·2πGρh·T², about 4.8e3 rad, the same for both.
            assert!((read.channel(3, Channel::Lower)[2] / 4.8e3 - 1.0).abs() < 0.02, "{}", read.lower[0]);
            assert_eq!(read.lower, read.upper);
            // The half-space's own phase is unbounded, and says so; its difference is still nothing.
            let half = signal(&dev::infinite_floor()).unwrap();
            assert!(half.lower.iter().all(|v| *v == f64::INFINITY) && half.warnings[0].contains("half-space"));
            // Control: the null is computed, and a floor that is not uniform in depth for the two
            // interferometers would show. Here: the same reading with one interferometer's part doubled.
            let reader = Reader::new(slab.array.instrument.instrument());
            let reading = reader.read(Vec3::ZERO, &Slab { density: 2400.0, thickness: 1.0 }, 0.0);
            assert_eq!((reading.lower - reading.upper).to_bits(), 0f64.to_bits());
            assert!((2.0 * reading.lower - reading.upper).abs() > 1e3);
            // Control: a body beside the slab is read through it; the null is the slab's, not a stuck zero.
            let mut with_body = slab.clone();
            with_body.bodies.push(BodySpec::point("mass", 10.0, [3.0, 0.0, 1.0]));
            assert_eq!(nonzero(&with_body), 30);
            Measured::Error((nonzero(&slab) + nonzero(&dev::infinite_floor())) as f64)
        },
    }

    cav_validate::case! {
        test: validation_superposition_is_exact,
        name: "scenes.superposition",
        subsystem: "scenes",
        slice: "S1",
        reference: Exact,
        against: "superposition: A alone, B alone, A + B; values of any channel, tower and shot where the reading of both is not the sum of the two, to the bit",
        unit: Count,
        tolerance: 0.0,
        measure: {
            let scene = dev::superposition();
            let (a, b, both) = (
                signal(&only(&scene, &[0])).unwrap(),
                signal(&only(&scene, &[1])).unwrap(),
                signal(&scene).unwrap(),
            );
            let mut differing = 0;
            for tower in 0..both.towers {
                for channel in CHANNELS {
                    let (x, y, sum) = (a.channel(tower, channel), b.channel(tower, channel), both.channel(tower, channel));
                    differing += (0..sum.len()).filter(|&k| (x[k] + y[k]).to_bits() != sum[k].to_bits()).count();
                }
            }
            // Control: the two parts are different things of comparable size, so a sum that dropped one would show.
            assert!(peak(a.channel(0, Channel::Gradiometer)) > 1e-4 && peak(b.channel(0, Channel::Gradiometer)) > 1e-4);
            assert_ne!(a.gradiometer, both.gradiometer);
            Measured::Error(differing as f64)
        },
    }

    cav_validate::case! {
        test: validation_mirror_pair_swaps,
        name: "scenes.mirror_pair",
        subsystem: "scenes",
        slice: "S1",
        reference: Exact,
        against: "mirror-pair: towers 2 and 3 swap their readings of the two bodies, and towers 1, 4 and 5 read the same from each; values that differ, to the bit",
        unit: Count,
        tolerance: 0.0,
        measure: {
            let scene = dev::mirror_pair();
            let (east, west) = (signal(&only(&scene, &[0])).unwrap(), signal(&only(&scene, &[1])).unwrap());
            let differing = |a: &[f64], b: &[f64]| a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
            let mut wrong = 0;
            for channel in CHANNELS {
                // Towers 2 (x = +4) and 3 (x = −4) are each other's mirror image.
                wrong += differing(east.channel(1, channel), west.channel(2, channel));
                wrong += differing(east.channel(2, channel), west.channel(1, channel));
                for on_plane in [0, 3, 4] {
                    wrong += differing(east.channel(on_plane, channel), west.channel(on_plane, channel));
                }
            }
            // Control: the swap is a swap. Tower 2 does not read the east body as tower 3 does.
            assert!(worst_of_peak(east.channel(1, Channel::Gradiometer), east.channel(2, Channel::Gradiometer)) > 0.1);
            Measured::Error(wrong as f64)
        },
    }

    cav_validate::case! {
        test: validation_ring_orbit_is_constant,
        name: "scenes.ring_orbit",
        subsystem: "scenes",
        slice: "S1",
        reference: ClosedForm,
        against: "ring-orbit: a mass circling the centre tower at fixed radius and height reads the same every shot; largest departure from the mean as a fraction of it",
        unit: Relative,
        tolerance: 1e-12,
        measure: {
            let read = signal(&dev::ring_orbit()).unwrap();
            let ripple = |tower: usize| {
                let series = read.channel(tower, Channel::Gradiometer);
                let mean = series.iter().sum::<f64>() / series.len() as f64;
                series.iter().fold(0.0f64, |m, v| m.max((v - mean).abs())) / mean.abs()
            };
            assert_eq!(read.times(0).len(), 30);
            // Control: an outer tower sees the mass come and go.
            assert!(ripple(1) > 0.5, "tower 2's reading ripples by {}", ripple(1));
            Measured::Error(ripple(0))
        },
    }

    /// The forward body's series at tower `tower` against the backward body's, reversed.
    fn reversal_error(scene: &SceneSpec, tower: usize) -> f64 {
        let (there, back) = (signal(&only(scene, &[0])).unwrap(), signal(&only(scene, &[1])).unwrap());
        let mut reversed: Vec<f64> = back.channel(tower, Channel::Gradiometer).to_vec();
        reversed.reverse();
        worst_of_peak(&reversed, there.channel(tower, Channel::Gradiometer))
    }

    cav_validate::case! {
        test: validation_time_reversal_mirrors,
        name: "scenes.time_reversal",
        subsystem: "scenes",
        slice: "S1",
        reference: ClosedForm,
        against: "time-reversal: with the launch that closes the arms at 2T (u0 = gT - v_kick/2), a pass played backwards reads the forward pass's series in reverse; largest difference over five towers as a fraction of the peak",
        unit: Relative,
        tolerance: 1e-12,
        measure: {
            let scene = dev::time_reversal();
            // Control, and a number worth having: AION-10's 3.86 m/s with the floor is not symmetric.
            let mut aion = scene.clone();
            aion.array.instrument.launch_velocity = 3.86;
            let asymmetry = reversal_error(&aion, 0);
            eprintln!("time-reversal: with u0 = 3.86 m/s and the floor the reversed series differs by {asymmetry:e} of the peak");
            assert!(asymmetry > 1e-6, "{asymmetry:e}");
            let worst = (0..5).fold(0.0f64, |worst, tower| worst.max(reversal_error(&scene, tower)));
            Measured::Error(worst)
        },
    }

    cav_validate::case! {
        test: validation_switch_on_step_response,
        name: "scenes.switch_on",
        subsystem: "scenes",
        slice: "S1",
        reference: Exact,
        against: "switch-on: the shot in flight when the mass appears reads the always-there mass's integral from the switch to the readout (12 nodes on 2 ms panels); shots before read nothing, shots after read all of it; largest difference as a fraction of the full reading",
        unit: Relative,
        tolerance: 1e-10,
        measure: {
            let scene = dev::switch_on();
            let read = signal(&scene).unwrap();
            let mut always = scene.clone();
            always.bodies[0].appears = None;
            let full = signal(&always).unwrap();
            // The reference for the straddling shot: the same mass, a far finer rule, the flight cut at the switch.
            let instrument = scene.array.instrument.instrument();
            let fine = Reader::new(cav_core::forward::Instrument {
                quadrature: cav_core::forward::Quadrature::Gauss { order: 12, panel: 0.002 },
                ..instrument
            });
            let mut worst = 0.0f64;
            for tower in 0..read.towers {
                let (ours, whole) = (read.channel(tower, Channel::Lower), full.channel(tower, Channel::Lower));
                assert_eq!(&ours[..2], [0.0, 0.0], "launched at 0 and 2 s: over before the mass appears");
                let partial = fine.read(base(scene.array.towers[tower].position), &mass(&scene, 0), read.times(0)[2]).lower;
                worst = worst.max((ours[2] - partial).abs() / whole[2].abs());
                for k in 3..6 {
                    worst = worst.max((ours[k] - whole[k]).abs() / whole[k].abs());
                }
                // Control: the straddling shot is a part of the whole, neither nothing nor all of it.
                let share = ours[2] / whole[2];
                eprintln!("switch-on: tower {} reads {share:.4} of the whole in the shot in flight", tower + 1);
                assert!(share.abs() > 1e-3 && (share - 1.0).abs() > 1e-3, "{share}");
            }
            Measured::Error(worst)
        },
    }

    cav_validate::case! {
        test: validation_flash_impulse_response,
        name: "scenes.flash",
        subsystem: "scenes",
        slice: "S1",
        reference: ClosedForm,
        against: "flash: a mass that exists for 1 ms reads, in the one shot in flight, the integrand at the middle of that millisecond times its length; largest difference over five towers as a fraction of that",
        unit: Relative,
        tolerance: 1e-5,
        measure: {
            let scene = dev::flash();
            let read = signal(&scene).unwrap();
            let instrument = scene.array.instrument.instrument();
            let [lower, upper] = instrument.arms();
            let (appears, vanishes) = scene.bodies[0].exists();
            let middle = 0.5 * (appears + vanishes);
            let tau = middle - 4.0;
            let mut worst = 0.0f64;
            for tower in 0..read.towers {
                let ours = read.channel(tower, Channel::Lower);
                // Only the shot launched at 4 s sees it.
                assert!(ours.iter().enumerate().all(|(k, v)| (k == 2) == (*v != 0.0)), "{ours:?}");
                let mut always = mass(&scene, 0);
                always.exists = (f64::NEG_INFINITY, f64::INFINITY);
                let integrand = always.difference(
                    base(scene.array.towers[tower].position),
                    [0.0, instrument.ifo_separation],
                    lower.rise(tau),
                    upper.rise(tau),
                    middle,
                )[0];
                let expected = instrument.phase_scale() * integrand * (vanishes - appears);
                worst = worst.max((ours[2] / expected - 1.0).abs());
            }
            // Control: twice as long a flash is twice the reading, not the same one.
            let mut longer = scene.clone();
            longer.bodies[0].vanishes = Some(4.502);
            let ratio = signal(&longer).unwrap().channel(0, Channel::Lower)[2] / read.channel(0, Channel::Lower)[2];
            // (Not exactly twice: the integrand changes by 0.7% over the extra millisecond.)
            assert!((ratio - 2.0).abs() < 0.02, "{ratio}");
            Measured::Error(worst)
        },
    }

    cav_validate::case! {
        test: validation_precision_far_scene,
        name: "scenes.precision_far",
        subsystem: "scenes",
        slice: "S1",
        reference: Exact,
        against: "precision-far: the scene 10 km from the origin against the same scene at the origin, all five towers; largest difference as a fraction of each tower's peak",
        unit: Relative,
        tolerance: 1e-11,
        measure: {
            let (far, near) = (signal(&dev::precision_far()).unwrap(), signal(&dev::precision_far_at([0.0; 3])).unwrap());
            let worst = (0..5).fold(0.0f64, |worst, tower| {
                worst.max(worst_of_peak(far.channel(tower, Channel::Gradiometer), near.channel(tower, Channel::Gradiometer)))
            });
            // Control: a millimetre's shift of the towers alone is seen.
            let mut nudged = dev::precision_far();
            nudged.array.towers.iter_mut().for_each(|t| t.position[0] += 1e-3);
            let moved = signal(&nudged).unwrap();
            assert!(worst_of_peak(moved.channel(1, Channel::Gradiometer), near.channel(1, Channel::Gradiometer)) > 1e-5);
            Measured::Error(worst)
        },
    }

    #[test]
    fn a_sphere_through_the_tower_is_finite_and_warned() {
        let scene = dev::through_the_tower();
        let read = signal(&scene).unwrap();
        assert!(read.lower.iter().chain(&read.upper).all(|v| v.is_finite()), "every reading is finite");
        // The warning names the body and each tower whose atoms it crosses: 3, 1 and 2.
        for tower in ["tower 1", "tower 2", "tower 3"] {
            let named =
                read.warnings.iter().any(|w| w.contains("`ball` passes through") && w.contains(tower));
            assert!(named, "no warning for {tower}: {:?}", read.warnings);
        }
        assert!(!read.warnings.iter().any(|w| w.contains("tower 4") || w.contains("tower 5")));
        // Control: a point mass on the atoms' own path is not hidden behind a finite number. It is
        // warned as unsettled, and the signal's estimate is far above the tolerance.
        let mut point = dev::point();
        point.bodies[0].motion = crate::Motion::Static { position: [0.0, 0.0, 1.0] };
        let singular = signal(&point).unwrap();
        let unsettled =
            singular.warnings.iter().any(|w| w.contains("did not settle") && w.contains("tower 1"));
        assert!(unsettled, "{:?}", singular.warnings);
        assert!(singular.estimate > 1e3 * PHASE_TOLERANCE);
        // And a ball that keeps clear is read without a word.
        let mut clear = scene;
        if let crate::Motion::Pass { from, to, .. } = &mut clear.bodies[0].motion {
            (from[1], to[1]) = (2.0, 2.0);
        }
        let quiet = signal(&clear).unwrap();
        assert!(quiet.warnings.is_empty(), "{:?}", quiet.warnings);
        assert!(quiet.estimate <= PHASE_TOLERANCE);
    }

    /// How many values of two series differ in any bit.
    fn bits_differing(a: &[f64], b: &[f64]) -> usize {
        assert_eq!(a.len(), b.len());
        a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count()
    }

    cav_validate::case! {
        test: validation_zero_offsets_are_the_old_schedule,
        name: "timing.zero_offsets",
        subsystem: "timing",
        slice: "S2",
        reference: Exact,
        against: "the comb's signal against what the engine at the tag s1-point-mass printed (a fixture: 5000 readout times and phases); and the comb and the pass with every tower's offset zero, against each shot read as S1 read it (the audited read at k·cycle + 2T, sources summed in the scene's order); values of any channel that differ in any bit",
        unit: Count,
        tolerance: 0.0,
        measure: {
            let mut differing = 0;
            for scene in [dev::comb(), dev::pass(), dev::superposition()] {
                let ours = signal(&scene).unwrap();
                let instrument = scene.array.instrument.instrument();
                let reader = Reader::new(instrument);
                let (cycle, flight) = (scene.array.instrument.cycle, instrument.flight_time());
                let shots = ours.times(0).len();
                assert!(ours.offsets.iter().all(|offset| *offset == 0.0) && ours.valid.iter().all(|taken| *taken));
                for (tower, placed) in scene.array.towers.iter().enumerate() {
                    let (mut lower, mut upper, mut gradiometer) = (vec![0.0; shots], vec![0.0; shots], vec![0.0; shots]);
                    for n in 0..scene.bodies.len() {
                        let body = mass(&scene, n);
                        for k in 0..shots {
                            // S1's own arithmetic for the readout: no offset in it at all.
                            let readout = k as f64 * cycle + flight;
                            let read = reader.read_to(base(placed.position), &body, readout, PHASE_TOLERANCE).reading;
                            lower[k] += read.lower;
                            upper[k] += read.upper;
                            gradiometer[k] += read.lower - read.upper;
                        }
                    }
                    differing += bits_differing(ours.channel(tower, Channel::Lower), &lower);
                    differing += bits_differing(ours.channel(tower, Channel::Upper), &upper);
                    differing += bits_differing(ours.channel(tower, Channel::Gradiometer), &gradiometer);
                    assert_eq!(ours.readouts(tower), ours.times(0), "with no offset a tower's times are the array's");
                }
            }
            // And against what S1 itself computed, not S1's arithmetic done by today's reader
            // (S2's review, finding 23): the comb's signal as the engine at the tag
            // `s1-point-mass` printed it, every readout time and every phase, to the bit.
            let frozen = include_str!("../tests/fixtures/s1-comb-signal.txt");
            let ours = signal(&dev::comb()).unwrap();
            let mut compared = 0;
            for line in frozen.lines().filter(|line| !line.starts_with('#')) {
                let mut words = line.split_whitespace();
                let tower: usize = words.next().unwrap().parse().unwrap();
                let (time, phase): (f64, f64) =
                    (words.next().unwrap().parse().unwrap(), words.next().unwrap().parse().unwrap());
                let k = compared % ours.times(0).len();
                let here = ours.channel(tower - 1, Channel::Gradiometer)[k];
                differing += usize::from(ours.times(0)[k].to_bits() != time.to_bits());
                differing += usize::from(here.to_bits() != phase.to_bits());
                compared += 1;
            }
            assert_eq!(compared, 5 * ours.times(0).len(), "the fixture has every shot of every tower");
            // Control: the fixture is not today's numbers by construction: one bit of one of its
            // phases changed is one difference.
            let first = frozen.lines().find(|line| !line.starts_with('#')).unwrap();
            let phase: f64 = first.split_whitespace().nth(2).unwrap().parse().unwrap();
            let nudged_phase = f64::from_bits(phase.to_bits() ^ 1);
            assert_ne!(nudged_phase.to_bits(), ours.channel(0, Channel::Gradiometer)[0].to_bits());
            // Control: an offset of a millisecond on one tower changes that tower's bits.
            let mut nudged = dev::comb();
            nudged.array.towers[2].timing.offset = 1e-3;
            let (moved, still) = (signal(&nudged).unwrap(), signal(&dev::comb()).unwrap());
            assert!(bits_differing(moved.channel(2, Channel::Gradiometer), still.channel(2, Channel::Gradiometer)) > 100);
            Measured::Error(differing as f64)
        },
    }

    cav_validate::case! {
        test: validation_offset_shifts_one_tower_only,
        name: "timing.offset_shifts_one_tower",
        subsystem: "timing",
        slice: "S2",
        reference: Exact,
        against: "the comb with tower 3 fired 0.292 s late: that tower reads what the synchronous scene reads at t + 0.292 s; largest difference as a fraction of the peak (the other towers must be unchanged to the bit)",
        unit: Relative,
        tolerance: 1e-12,
        measure: {
            let delta = 0.292;
            let sync = dev::comb();
            let mut late = sync.clone();
            late.array.towers[2].timing.offset = delta;
            let (ours, together) = (signal(&late).unwrap(), signal(&sync).unwrap());
            // The synchronous scene read at the shifted times.
            let shifted = signal_at(&sync, together.times(0).iter().map(|t| t + delta).collect()).unwrap();
            for tower in [0, 1, 3, 4] {
                for channel in CHANNELS {
                    assert_eq!(bits_differing(ours.channel(tower, channel), together.channel(tower, channel)), 0, "tower {}", tower + 1);
                }
            }
            assert_eq!(ours.readout(2, 7), together.times(0)[7] + delta);
            assert_eq!(ours.readout(1, 7), together.times(0)[7]);
            // Control: the offset matters. Read at the unshifted times the tower is far from it.
            let unshifted = worst_of_peak(ours.channel(2, Channel::Gradiometer), together.channel(2, Channel::Gradiometer));
            assert!(unshifted > 1e-3, "{unshifted:e}");
            Measured::Error(worst_of_peak(ours.channel(2, Channel::Gradiometer), shifted.channel(2, Channel::Gradiometer)))
        },
    }

    #[test]
    fn staggered_array_end_to_end() {
        let scene = dev::staggered();
        let read = signal(&scene).unwrap();
        // Five towers, each a fifth of a flight after the one before, on their own times.
        let step = 2.0 * 0.73 / 5.0;
        for tower in 0..5 {
            assert_eq!(read.offsets[tower], tower as f64 * step);
            assert_eq!(read.readouts(tower)[3], read.times(0)[3] + tower as f64 * step);
        }
        assert!(read.settled && read.warnings.is_empty());
        // The sum over sources is still exact, tower by tower.
        let alone = signal(&only(&scene, &[0])).unwrap();
        assert_eq!(bits_differing(&alone.gradiometer, &read.gradiometer), 0);
        // Each tower's periodogram, on its own (evenly spaced) times, has the oscillator's line
        // on the same bin: an offset moves a tower's phase, not its frequencies.
        let frequency = scene.bodies[0].motion.frequency(scene.array.instrument.gravity).unwrap();
        for tower in 0..5 {
            let spectrum = cav_core::spectrum::periodogram(
                read.channel(tower, Channel::Gradiometer),
                scene.array.instrument.cycle,
                cav_core::spectrum::Classical::SCRIPTS,
            );
            let (bin, _, _) = spectrum.peak().unwrap();
            assert_eq!(Some(bin), spectrum.bin_of(frequency), "tower {}", tower + 1);
        }
        // Control: the towers do not read the same thing at the same shot: they are at different
        // places and fire at different times.
        assert!(
            bits_differing(read.channel(3, Channel::Gradiometer), read.channel(4, Channel::Gradiometer))
                > 100
        );
        // A shot that is not taken reads exactly nothing and says so.
        let mut shots = shots(&scene).unwrap();
        let count = shots.times(0).len();
        shots.valid[2 * count + 5] = false;
        let gapped = signal_of(&scene, &shots).unwrap();
        assert_eq!(gapped.channel(2, Channel::Gradiometer)[5].to_bits(), 0);
        assert!(!gapped.taken(2)[5] && gapped.taken(2)[4] && gapped.taken(1)[5]);
        assert_eq!(gapped.channel(2, Channel::Gradiometer)[4], read.channel(2, Channel::Gradiometer)[4]);
    }

    /// The comb with a schedule, short enough to look at shot by shot.
    fn scheduled(edit: impl FnOnce(&mut crate::ScheduleSpec)) -> SceneSpec {
        let mut scene = dev::comb();
        scene.duration = 400.0;
        edit(&mut scene.array.schedule);
        assert_eq!(scene.validate(), Ok(()));
        scene
    }

    #[test]
    fn schedules_draw_what_they_say() {
        use crate::{GapSpec, JitterKind, JitterModel, JitterSpec, WindowSpec};
        use cav_core::cadence;
        let jitter = |model, kind, delta| Some(JitterSpec { model, kind, delta });
        // The plain schedule: k·cycle, to the bit, all taken.
        let plain = launches(&scheduled(|_| {})).unwrap();
        assert_eq!(plain, cadence::uniform_times(200, 2.0));
        // Cycle jitter is the reference scripts' random walk, and is the generator the closed
        // forms were checked against, shot for shot.
        for (kind, symmetric) in [(JitterKind::Positive, false), (JitterKind::Symmetric, true)] {
            let scene = scheduled(|s| (s.jitter, s.seed) = (jitter(JitterModel::Walk, kind, 0.3), 9));
            let walk = launches(&scene).unwrap();
            assert_eq!(walk[..], cadence::walk_times(walk.len(), 2.0, 0.3, symmetric, 9)[..]);
            assert!(
                walk.windows(2)
                    .all(|w| w[1] - w[0] >= if symmetric { 1.7 } else { 2.0 } && w[1] - w[0] <= 2.3)
            );
            assert!(*walk.last().unwrap() < 400.0, "every launch is before the end");
            let grid =
                launches(&scheduled(|s| (s.jitter, s.seed) = (jitter(JitterModel::Grid, kind, 0.2), 9)))
                    .unwrap();
            // The generator's times; the symmetric grid stands δ after zero (finding 22 of S2's
            // review: no shot is launched before the clock starts), to the bit.
            let shift = if symmetric { 0.2 } else { 0.0 };
            let expected: Vec<f64> = cadence::grid_times(200, 2.0, 0.2, symmetric, 9)
                .into_iter()
                .map(|time| if symmetric { time + shift } else { time })
                .collect();
            assert_eq!(grid, expected);
        }
        // Alternation: the two cycle times in turn.
        let alternating = launches(&scheduled(|s| s.alternate = Some(2.5))).unwrap();
        assert_eq!(alternating[..5], [0.0, 2.0, 4.5, 6.5, 9.0]);
        assert_eq!(alternating, cadence::alternating_times(alternating.len(), 2.0, 2.5));
        // Planned gaps and the window say which shots are fired; the times are still all there.
        let gapped = shots(&scheduled(|s| s.gaps = Some(GapSpec { on: 3, period: 5 }))).unwrap();
        assert_eq!(gapped.times(0).len(), 200);
        for tower in 0..5 {
            assert!((0..200).all(|k| gapped.taken(tower, k) == (k % 5 < 3)));
        }
        let windowed =
            shots(&scheduled(|s| s.window = Some(WindowSpec { start: 30.0, length: 20.0, period: 100.0 })))
                .unwrap();
        for k in 0..200 {
            let launch = k as f64 * 2.0;
            assert_eq!(
                windowed.taken(0, k),
                (30.0..50.0).contains(&(launch % 100.0)),
                "shot {k} at {launch} s"
            );
        }
        // A window that runs past the end of its period wraps into the next.
        let late =
            shots(&scheduled(|s| s.window = Some(WindowSpec { start: 90.0, length: 20.0, period: 100.0 })))
                .unwrap();
        assert!(
            late.taken(0, 2) && late.taken(0, 47) && !late.taken(0, 20),
            "4 s and 94 s are in, 40 s is out"
        );
        // Loss: about the stated fraction, each tower its own shots.
        let lossy = shots(&scheduled(|s| (s.loss, s.seed) = (0.2, 3))).unwrap();
        let lost: Vec<usize> =
            (0..5).map(|tower| (0..200).filter(|&k| !lossy.taken(tower, k)).count()).collect();
        assert!(lost.iter().all(|n| (20..=60).contains(n)), "{lost:?} of 200 at p = 0.2");
        assert!((0..200).any(|k| lossy.taken(0, k) != lossy.taken(1, k)), "towers lose their own shots");
        // Control: no loss, nothing lost; and the schedule that is plain says so.
        assert!(shots(&scheduled(|_| {})).unwrap().valid.iter().all(|taken| *taken));
        assert!(scheduled(|s| s.seed = 5).array.schedule.is_uniform());
        assert!(!scheduled(|s| s.loss = 0.1).array.schedule.is_uniform());
    }

    #[test]
    fn jitter_is_keyed_not_ordered() {
        use crate::{JitterKind, JitterModel, JitterSpec};
        let scene = scheduled(|s| {
            s.jitter =
                Some(JitterSpec { model: JitterModel::Grid, kind: JitterKind::Symmetric, delta: 0.25 });
            (s.loss, s.seed) = (0.1, 42);
        });
        let whole = shots(&scene).unwrap();
        // The same shots, asked for again, on four threads at once, and as part of a timeline
        // twice as long: each shot's time and fate depend on the seed, the tower and the shot's
        // own number, and on nothing that was drawn before it.
        assert_eq!(shots(&scene).unwrap(), whole);
        let threads: Vec<Shots> = std::thread::scope(|scope| {
            let jobs: Vec<_> = (0..4).map(|_| scope.spawn(|| shots(&scene).unwrap())).collect();
            jobs.into_iter().map(|job| job.join().unwrap()).collect()
        });
        assert!(threads.iter().all(|other| *other == whole));
        let mut longer = scene.clone();
        longer.duration = 800.0;
        let more = shots(&longer).unwrap();
        assert_eq!(more.times(0)[..200], whole.times(0)[..]);
        for tower in 0..5 {
            assert_eq!(more.taken_all(tower)[..200], whole.taken_all(tower)[..]);
        }
        // Fewer towers: the towers that remain keep their shots.
        let mut fewer = scene.clone();
        fewer.array.towers.truncate(2);
        assert_eq!(shots(&fewer).unwrap().taken_all(1), whole.taken_all(1));
        // Control: another seed is another schedule.
        let mut other = scene.clone();
        other.array.schedule.seed = 43;
        let reseeded = shots(&other).unwrap();
        assert_ne!(reseeded.times(0), whole.times(0));
        assert_ne!(reseeded.valid, whole.valid);
    }

    #[test]
    fn a_schedule_that_cannot_fly_is_refused() {
        use crate::{GapSpec, JitterKind, JitterModel, JitterSpec, WindowSpec};
        type Break = fn(&mut crate::ScheduleSpec);
        let walk = |kind, delta| Some(JitterSpec { model: JitterModel::Walk, kind, delta });
        let cases: [(&str, Break); 10] = [
            // The comb's cycle is 2 s and a shot 1.46 s: a cycle may not be drawn shorter.
            ("array.schedule.jitter.delta", |s| {
                s.jitter =
                    Some(JitterSpec { model: JitterModel::Walk, kind: JitterKind::Symmetric, delta: 0.6 })
            }),
            ("array.schedule.jitter.delta", |s| {
                s.jitter =
                    Some(JitterSpec { model: JitterModel::Grid, kind: JitterKind::Symmetric, delta: 0.3 })
            }),
            ("array.schedule.jitter.delta", |s| {
                s.jitter =
                    Some(JitterSpec { model: JitterModel::Grid, kind: JitterKind::Positive, delta: f64::NAN })
            }),
            ("array.schedule.alternate", |s| s.alternate = Some(1.4)),
            ("array.schedule.alternate", |s| s.alternate = Some(-2.0)),
            ("array.schedule.gaps", |s| s.gaps = Some(GapSpec { on: 0, period: 5 })),
            ("array.schedule.gaps", |s| s.gaps = Some(GapSpec { on: 6, period: 5 })),
            ("array.schedule.window.length", |s| {
                s.window = Some(WindowSpec { start: 0.0, length: 200.0, period: 100.0 })
            }),
            ("array.schedule.loss", |s| s.loss = 1.0),
            ("array.schedule.seed", |s| s.seed = (1 << 53) + 2),
        ];
        for (field, break_it) in cases {
            let mut scene = dev::comb();
            break_it(&mut scene.array.schedule);
            let why = scene.validate().expect_err(field).0;
            assert!(why.starts_with(field), "`{why}` should name `{field}`");
        }
        // At the edge each is accepted: a cycle drawn down to exactly a shot, late-only jitter of
        // any size the cycle allows, the largest seed.
        let edge = scheduled(|s| {
            s.jitter = walk(JitterKind::Symmetric, 2.0 - 1.46);
            s.seed = 1 << 53;
            s.loss = 0.99;
        });
        assert!(shots(&edge).is_ok());
        assert!(shots(&scheduled(|s| s.jitter = walk(JitterKind::Positive, 500.0))).is_ok());
        // Too many shots are refused by count before anything is made, jitter or not: the count
        // is taken at the shortest cycle the schedule allows.
        let mut long = scheduled(|s| s.jitter = walk(JitterKind::Positive, 1.0));
        long.array.instrument.half_sequence = 0.001;
        long.array.instrument.cycle = 0.0021;
        long.duration = 1.0e6;
        assert_eq!(long.validate(), Ok(()));
        assert!(launches(&long).unwrap_err().contains("at most 2000000"));
        // Control: the schema describes the schedule, with units.
        assert_eq!(crate::paths::schema_at("array.schedule.jitter.delta").unwrap()["x-unit"], "s");
        assert_eq!(crate::paths::entry(&dev::cadence_lab(), "array.schedule.loss").unwrap().value, 0.02);
    }

    cav_validate::case! {
        test: validation_threads_do_not_change_a_bit,
        name: "engine.thread_count",
        subsystem: "engine",
        slice: "S2",
        reference: Exact,
        against: "the signals of three scenes (staggered towers on a jittered, thinned cadence; a sphere through the towers, with its warnings; two bodies) read on 1, 2, 3 and 6 threads; values, estimates, warnings and flags that differ in any way from the one-thread read",
        unit: Count,
        tolerance: 0.0,
        measure: {
            let mut uneven = dev::staggered();
            uneven.duration = 700.0;
            uneven.array.schedule.loss = 0.1;
            uneven.array.schedule.jitter = Some(crate::JitterSpec {
                model: crate::JitterModel::Walk,
                kind: crate::JitterKind::Positive,
                delta: 0.3,
            });
            let scenes = [uneven, dev::through_the_tower(), dev::superposition()];
            let on = |threads: usize, scene: &SceneSpec| {
                let shots = shots(scene).unwrap();
                let mut total = Signal::silent(&shots);
                for source in sources(scene) {
                    total.add(&contribution_on(threads, scene, source, &shots, &|| false).unwrap());
                }
                total
            };
            let mut differing = 0;
            for scene in &scenes {
                let one = on(1, scene);
                // More pieces than threads, and more threads than towers: the queue is shared.
                assert!(one.times(0).len() > 2 * PIECE || scene.name != "staggered", "{} shots", one.times(0).len());
                for threads in [2, 3, 6] {
                    let many = on(threads, scene);
                    differing += bits_differing(&one.lower, &many.lower)
                        + bits_differing(&one.upper, &many.upper)
                        + bits_differing(&one.gradiometer, &many.gradiometer)
                        + bits_differing(&one.estimates, &many.estimates)
                        + usize::from(one.warnings != many.warnings)
                        + usize::from(one.settled != many.settled || one.valid != many.valid);
                }
                // The read that the engine makes (however many threads this machine gives it) is
                // the same again.
                assert_eq!(signal(scene).unwrap(), one, "{}", scene.name);
            }
            // The warnings are there to be compared, in tower order.
            let warned = on(6, &scenes[1]);
            assert!(warned.warnings.len() >= 3 && warned.warnings.windows(2).all(|w| w[0].tower <= w[1].tower));
            // Control: one bit of one value is one difference, so the count can fail.
            let mut nudged = on(1, &scenes[2]);
            nudged.upper[7] = f64::from_bits(nudged.upper[7].to_bits() ^ 1);
            assert_eq!(bits_differing(&nudged.upper, &on(2, &scenes[2]).upper), 1);
            // And a read that is cancelled part-way gives nothing, on any number of threads.
            let shots = shots(&scenes[0]).unwrap();
            let asked = std::sync::atomic::AtomicUsize::new(0);
            let cancel = || asked.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 200;
            assert_eq!(contribution_on(6, &scenes[0], SourceId::Body(0), &shots, &cancel), None);
            Measured::Error(differing as f64)
        },
    }

    #[test]
    fn the_thread_count_is_stated_and_bounded() {
        // What the environment asks for is taken when it is a count; anything else is the default.
        let default = threads();
        assert!((1..=64).contains(&default));
        let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
        if std::env::var_os("CAV_THREADS").is_none() {
            assert_eq!(default, cores.saturating_sub(1).clamp(1, 8));
        }
        // More threads than pieces is as many as there are pieces: nothing is spawned for nothing.
        let scene = dev::point();
        let shots = shots(&scene).unwrap();
        let few = contribution_on(64, &scene, SourceId::Body(0), &shots, &|| false).unwrap();
        assert_eq!(Some(few), contribution_on(1, &scene, SourceId::Body(0), &shots, &|| false));
    }

    #[test]
    fn error_estimates_are_per_tower() {
        // A sphere that passes through tower 1's atoms and well clear of tower 5: the read at
        // tower 1 does not settle and says how far it was off; tower 5's is good to the tolerance.
        let scene = dev::through_the_tower();
        let read = signal(&scene).unwrap();
        assert_eq!(read.estimates.len(), 5);
        assert!(read.estimates[0] > PHASE_TOLERANCE && !read.settled, "{:e}", read.estimates[0]);
        assert!(read.estimates[4] <= PHASE_TOLERANCE, "{:e}", read.estimates[4]);
        // The signal's one number is the largest of them.
        let largest = read.estimates.iter().fold(0.0f64, |m, e| m.max(*e));
        assert_eq!(read.estimate, largest);
        // With two sources a tower's estimate is the larger of the two, not their sum.
        let both = signal(&dev::superposition()).unwrap();
        let (a, b) = (
            signal(&only(&dev::superposition(), &[0])).unwrap(),
            signal(&only(&dev::superposition(), &[1])).unwrap(),
        );
        for tower in 0..5 {
            assert_eq!(both.estimates[tower], a.estimates[tower].max(b.estimates[tower]));
        }
        // Control: a scene with nothing in it has nothing to estimate.
        assert_eq!(signal(&dev::void()).unwrap().estimates, [0.0; 5]);
    }

    #[test]
    fn a_warning_marks_its_own_tower_only() {
        // Twelve towers in a row, and a point mass on the axis of the twelfth. Its label, "tower
        // 12", begins with the first tower's: a mark found by looking for the label in the words
        // of the warning (as the outliner once did) lights tower 1 as well.
        let mut scene = dev::point();
        scene.array.towers = (0..12)
            .map(|n| crate::TowerSpec::at(&format!("tower {}", n + 1), [f64::from(n) * 3.0, 0.0, 0.0]))
            .collect();
        scene.bodies[0].motion = crate::Motion::Static { position: [33.0, 0.0, 1.0] };
        let read = reading(&scene, 3.0, &|| false).unwrap();
        assert!(!read.warnings.is_empty());
        assert!(read.warnings.iter().all(|w| w.tower == Some(11)), "{:?}", read.warnings);
        let marked: Vec<usize> =
            (0..12).filter(|&n| read.warnings.iter().any(|w| w.tower == Some(n))).collect();
        assert_eq!(marked, [11]);
        // Control: the words alone cannot tell the two apart.
        let by_words: Vec<usize> = (0..12)
            .filter(|&n| read.warnings.iter().any(|w| w.contains(&scene.array.towers[n].label)))
            .collect();
        assert_eq!(by_words, [0, 11], "matching by substring marks tower 1 too");
    }

    #[test]
    fn the_crossing_check_is_inside_the_reads_budget() {
        use std::sync::atomic::{AtomicU64, Ordering};
        // A long flight with the floor off and a ball on the atoms' own path: the most work a
        // read is asked for, and a crossing to find.
        let instrument = cav_core::forward::Instrument {
            half_sequence: 30.0,
            floor: false,
            ..cav_core::forward::Instrument::aion10()
        };
        let reader = Reader::new(instrument);
        let asked = AtomicU64::new(0);
        let ball = Mass {
            mass: 1.0,
            radius: 0.2,
            position: |_| {
                asked.fetch_add(1, Ordering::Relaxed);
                Vec3::new(0.0, 0.0, 1.0)
            },
            exists: (f64::NEG_INFINITY, f64::INFINITY),
            breaks: Vec::new(),
        };
        let watched = Watched::new(&ball);
        let audited = reader.read_to(Vec3::ZERO, &watched, 90.0, PHASE_TOLERANCE);
        assert!(watched.crossed(), "a ball on the path is seen");
        // The body was asked where it is at the read's own nodes and nowhere else: once by the
        // watch until it has seen a crossing, once by the kernel. No second pass over the flight.
        let asked = asked.load(Ordering::Relaxed);
        assert!(audited.nodes <= cav_core::forward::MAX_NODES);
        assert!(
            asked <= 2 * audited.nodes && asked >= audited.nodes / 2,
            "{asked} for {} nodes",
            audited.nodes
        );
        // Control: a ball that keeps clear is not seen, and the watch changes no reading.
        let clear = Mass { mass: 1.0, radius: 0.2, ..Mass::point(1.0, |_| Vec3::new(3.0, 0.0, 1.0)) };
        let watched = Watched::new(&clear);
        let audited = reader.read_to(Vec3::ZERO, &watched, 90.0, PHASE_TOLERANCE);
        assert!(!watched.crossed());
        assert_eq!(audited, reader.read_to(Vec3::ZERO, &clear, 90.0, PHASE_TOLERANCE));
    }

    #[test]
    fn the_reading_is_the_series_at_a_readout_and_contributions_can_be_cancelled() {
        let scene = dev::pass();
        let read = signal(&scene).unwrap();
        // The live reading at a shot's readout is that shot's value, to the bit.
        let live = reading(&scene, read.times(0)[3], &|| false).unwrap().towers;
        for (tower, values) in live.iter().enumerate() {
            assert_eq!(values.gradiometer.to_bits(), read.channel(tower, Channel::Gradiometer)[3].to_bits());
            assert_eq!(values.lower.to_bits(), read.channel(tower, Channel::Lower)[3].to_bits());
        }
        // Control: at another time it is another value.
        assert_ne!(
            reading(&scene, read.times(0)[3] + 0.3, &|| false).unwrap().towers[0].gradiometer,
            live[0].gradiometer
        );
        // The reading carries what the audit found. A point mass on a tower's axis does not
        // settle, and the number beside that tower says so.
        let mut point = dev::point();
        point.bodies[0].motion = crate::Motion::Static { position: [0.0, 0.0, 1.0] };
        let unsettled = reading(&point, 3.0, &|| false).unwrap();
        assert!(unsettled.estimate > 1e3 * PHASE_TOLERANCE, "{:e}", unsettled.estimate);
        assert!(unsettled.warnings.iter().any(|w| w.contains("did not settle") && w.contains("tower 1")));
        assert!(!unsettled.settled, "and it says so as a fact, not only in words");
        // A warning names its tower by number: tower 1 is the one on whose axis the mass sits,
        // and a tower whose label merely starts the same way is not it.
        assert!(unsettled.warnings.iter().all(|w| w.tower == Some(0)), "{:?}", unsettled.warnings);
        assert!(unsettled.warnings[0].concerns(0) && !unsettled.warnings[0].concerns(1));
        assert!(Warning::general("the floor").concerns(3));
        let settled = reading(&dev::point(), 3.0, &|| false).unwrap();
        assert!(settled.warnings.is_empty() && settled.settled && settled.estimate <= PHASE_TOLERANCE);
        // And it can be cancelled like any other read.
        assert_eq!(reading(&scene, 3.0, &|| true), None);
        // Cancelled work returns nothing at all: no half-filled contribution.
        let asked = std::sync::atomic::AtomicUsize::new(0);
        let cancel_after_ten = || asked.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 10;
        assert_eq!(contribution(&scene, SourceId::Body(0), &shots(&scene).unwrap(), &cancel_after_ten), None);
        assert!(contribution(&scene, SourceId::Body(0), &shots(&scene).unwrap(), &|| false).is_some());
        assert_eq!(sources(&dev::infinite_floor()), [SourceId::Floor]);
        assert_eq!(sources(&dev::superposition()), [SourceId::Body(0), SourceId::Body(1)]);
    }

    #[test]
    fn no_shot_is_launched_before_the_clock_starts() {
        use crate::{JitterKind, JitterModel, JitterSpec};
        use cav_core::cadence;
        // Symmetric jitter about a grid, a quarter of a second either way, on 200 seeds: shot 0
        // would be drawn early for about half of them.
        let (mut early, delta) = (0, 0.25);
        for seed in 0..200 {
            let scene = scheduled(|s| {
                s.jitter = Some(JitterSpec { model: JitterModel::Grid, kind: JitterKind::Symmetric, delta });
                s.seed = seed;
            });
            let ours = launches(&scene).unwrap();
            let least = ours.iter().fold(f64::INFINITY, |m, t| m.min(*t));
            assert!(least >= 0.0, "seed {seed}: a shot is launched at {least} s");
            // The generator the closed forms were held to, and the grid standing δ late.
            let unshifted = cadence::grid_times(ours.len(), 2.0, delta, true, seed);
            assert!(ours.iter().zip(&unshifted).all(|(a, b)| a.to_bits() == (b + delta).to_bits()));
            // Control: without the shift the same schedule launches shot 0 before zero.
            early += usize::from(unshifted[0] < 0.0);
        }
        assert!((60..=140).contains(&early), "{early} of 200 seeds would launch shot 0 before zero");
        // Jitter that only delays, a walk and the plain schedule begin at zero or after as they did.
        for (model, kind) in [
            (JitterModel::Grid, JitterKind::Positive),
            (JitterModel::Walk, JitterKind::Positive),
            (JitterModel::Walk, JitterKind::Symmetric),
        ] {
            let scene = scheduled(|s| s.jitter = Some(JitterSpec { model, kind, delta }));
            assert!(launches(&scene).unwrap()[0] >= 0.0);
        }
    }

    /// The least time from one launch to the next, and the largest departure of a shot's length
    /// (readout less launch) from 2T, over the array's clock.
    fn spacing_and_length(scene: &SceneSpec) -> (f64, f64) {
        let (launched, all) = (launches(scene).unwrap(), shots(scene).unwrap());
        let flight = 2.0 * scene.array.instrument.half_sequence;
        assert_eq!((all.flight(0), all.times(0).len()), (flight, launched.len()));
        let least = launched.windows(2).fold(f64::INFINITY, |m, w| m.min(w[1] - w[0]));
        let off = launched.iter().zip(all.times(0)).fold(0.0f64, |m, (l, r)| m.max((r - l - flight).abs()));
        (least, off)
    }

    #[test]
    fn a_shot_is_two_t_long_and_shots_do_not_overlap() {
        // George: lateness is only ever in the pause between shots; the sequence itself never
        // varies (2T for every shot), and shots do not overlap. So whatever the schedule, the
        // array's launches are at least 2T apart and every readout is its launch and 2T; gaps,
        // the window and loss say which shots are taken and move none.
        use crate::{GapSpec, JitterKind, JitterModel, JitterSpec, WindowSpec};
        type Edit = fn(&mut crate::ScheduleSpec);
        fn jitter(model: JitterModel, kind: JitterKind, delta: f64) -> Option<JitterSpec> {
            Some(JitterSpec { model, kind, delta })
        }
        let schedules: [(&str, Edit); 9] = [
            ("plain", |_| {}),
            ("walk, late only", |s| s.jitter = jitter(JitterModel::Walk, JitterKind::Positive, 0.5)),
            ("walk, either way", |s| s.jitter = jitter(JitterModel::Walk, JitterKind::Symmetric, 0.5)),
            ("grid, late only", |s| s.jitter = jitter(JitterModel::Grid, JitterKind::Positive, 0.5)),
            ("grid, either way", |s| s.jitter = jitter(JitterModel::Grid, JitterKind::Symmetric, 0.25)),
            ("alternating", |s| s.alternate = Some(1.5)),
            ("gaps", |s| s.gaps = Some(GapSpec { on: 3, period: 5 })),
            ("window", |s| s.window = Some(WindowSpec { start: 30.0, length: 20.0, period: 100.0 })),
            ("loss", |s| s.loss = 0.3),
        ];
        let flight = 1.46;
        let plain = shots(&scheduled(|_| {})).unwrap();
        for (name, edit) in schedules {
            for seed in [0, 1, 7] {
                let scene = scheduled(|s| {
                    edit(s);
                    s.seed = seed;
                });
                let (least, off) = spacing_and_length(&scene);
                assert!(
                    least >= flight - 1e-12,
                    "{name}, seed {seed}: launches {least} s apart, a shot is {flight} s"
                );
                assert!(off <= 1e-12, "{name}, seed {seed}: a shot's length is {off} s from 2T");
                let all = shots(&scene).unwrap();
                // Every tower's shot is 2T long as well, whatever its offset would be.
                assert!((0..all.towers()).all(|d| all.readout(d, 3) - all.launch(d, 3) == flight));
                if ["gaps", "window", "loss"].contains(&name) {
                    assert_eq!(all.times(0), plain.times(0), "{name} moves no shot");
                    assert!(all.valid.iter().any(|taken| !taken), "{name} does leave shots out");
                }
            }
        }
        // The library's uneven scene is the random walk, late only: lateness accumulates.
        let lab = dev::cadence_lab().array.schedule.jitter.unwrap();
        assert_eq!((lab.model, lab.kind), (JitterModel::Walk, JitterKind::Positive));
        // Control: a jitter the validation refuses (a cycle may be drawn shorter than a shot)
        // does overlap, and the same measure sees it.
        let mut overlapping = dev::comb();
        overlapping.duration = 400.0;
        overlapping.array.schedule.jitter = jitter(JitterModel::Walk, JitterKind::Symmetric, 0.9);
        assert!(overlapping.validate().is_err());
        let (least, _) = spacing_and_length(&overlapping);
        assert!(least < flight, "the control's closest launches are {least} s apart");
    }

    #[test]
    fn an_offset_that_puts_shots_outside_the_timeline_is_said() {
        let mut plain = dev::comb();
        plain.duration = 12.0;
        let mut scene = plain.clone();
        scene.array.towers[1].timing.offset = -0.5;
        scene.array.towers[3].timing.offset = 3.0;
        assert_eq!(scene.validate(), Ok(()));
        let all = shots(&scene).unwrap();
        assert_eq!(all.warnings.len(), 2, "{:?}", all.warnings);
        let (early, late) = (&all.warnings[0], &all.warnings[1]);
        assert_eq!((early.tower, late.tower), (Some(1), Some(3)));
        assert!(
            early.contains("1 of its 6 shot(s) are launched before the timeline begins, the first at -0.5 s"),
            "{early}"
        );
        assert!(
            late.contains(
                "1 of its 6 shot(s) are launched at or after the end of the timeline (12 s), the last at 13 s"
            ),
            "{late}"
        );
        // The offset moves the shots and nothing else: all are still taken, the signal says the
        // same two things, and every other tower reads the bits it read.
        assert!(all.valid.iter().all(|taken| *taken));
        let (moved, still) = (signal(&scene).unwrap(), signal(&plain).unwrap());
        assert_eq!(moved.warnings, all.warnings);
        for tower in [0, 2, 4] {
            for channel in CHANNELS {
                assert_eq!(moved.channel(tower, channel), still.channel(tower, channel));
            }
        }
        assert_ne!(moved.channel(1, Channel::Gradiometer), still.channel(1, Channel::Gradiometer));
        // Control: no offsets, and offsets that keep every launch inside, say nothing.
        assert!(shots(&plain).unwrap().warnings.is_empty() && still.warnings.is_empty());
        assert!(shots(&dev::staggered()).unwrap().warnings.is_empty());
    }

    #[test]
    fn shots_touching_a_span_are_one_stretch() {
        use crate::{JitterKind, JitterModel, JitterSpec};
        let mut scene = scheduled(|s| {
            s.jitter = Some(JitterSpec { model: JitterModel::Walk, kind: JitterKind::Positive, delta: 0.5 });
            s.seed = 4;
        });
        scene.array.towers[2].timing.offset = 0.7;
        let all = shots(&scene).unwrap();
        let spans =
            [[10.0, 20.0], [-5.0, 3.0], [300.0, 320.0], [395.0, 500.0], [1000.0, 1100.0], [50.0, 50.0]];
        for tower in [0, 2] {
            for [from, to] in spans {
                let found: Vec<usize> = (0..all.times(0).len())
                    .filter(|&k| all.launch(tower, k) <= to && all.readout(tower, k) >= from)
                    .collect();
                let stretch: Vec<usize> = all.touching(tower, [from, to]).collect();
                assert_eq!(stretch, found, "tower {tower}, {from} to {to} s");
                for k in stretch {
                    let shot = all.flown(tower, k);
                    assert_eq!(
                        (shot.k, shot.readout, shot.taken),
                        (k, all.readout(tower, k), all.taken(tower, k))
                    );
                    assert_eq!(shot.launch, shot.readout - 1.46);
                }
            }
        }
        assert!(all.touching(0, [1000.0, 1100.0]).is_empty() && !all.touching(0, [10.0, 20.0]).is_empty());
        // Control: these are the schedule's shots, not the plain one's: by 300 s the walk is
        // tens of seconds behind an even clock, and other shots touch the span.
        let even = shots(&scheduled(|_| {})).unwrap();
        assert_ne!(all.touching(0, [300.0, 320.0]), even.touching(0, [300.0, 320.0]));
        // And the tower with the offset has its own.
        assert_ne!(all.touching(2, [50.0, 50.0]), all.touching(0, [50.0, 50.0]));
    }

    /// Four towers that do not share an instrument (AION-10; AICE, late by 0.3 s; AION-10 with
    /// T = 0.9 s; AION-10 with Δr = 4 m and 800 kicks, whose shots are tower 1's to the bit) and
    /// two moving bodies.
    fn unequal() -> SceneSpec {
        use crate::{InstrumentOverride, InstrumentSpec, TowerSpec};
        let mut scene = dev::mixed_array();
        scene.duration = 40.0;
        let base = scene.array.instrument.clone();
        let short = InstrumentSpec { ifo_separation: 4.0, kicks: 800.0, ..base.clone() };
        scene.array.towers[1].timing.offset = 0.3;
        scene.array.towers.push(TowerSpec {
            instrument: Some(InstrumentOverride::between(&base, &short)),
            ..TowerSpec::at("short", [-5.0, 0.0, 0.0])
        });
        scene.bodies.push(BodySpec::moving(
            "walker",
            70.0,
            crate::Motion::Pass { from: [-8.0, 3.0, 1.0], to: [8.0, 3.0, 1.0], start: 2.0, speed: 0.5 },
        ));
        assert_eq!(scene.validate(), Ok(()));
        scene
    }

    /// Tower `tower` of `scene` alone: a one-tower scene of that tower's resolved instrument,
    /// position and offset.
    fn alone(scene: &SceneSpec, tower: usize) -> SceneSpec {
        let mut one = scene.clone();
        one.array.instrument = scene.array.instrument_of(tower);
        one.array.towers = vec![crate::TowerSpec { instrument: None, ..scene.array.towers[tower].clone() }];
        one
    }

    /// The signal of `scene` read on `threads` threads.
    fn read_on(threads: usize, scene: &SceneSpec) -> Signal {
        let shots = shots(scene).unwrap();
        let mut total = Signal::silent(&shots);
        for source in sources(scene) {
            total.add(&contribution_on(threads, scene, source, &shots, &|| false).unwrap());
        }
        total
    }

    /// How many values (each channel and each readout time of every shot) of each tower of
    /// `mixed` differ in any bit from what `singles[d]` reads as its one tower.
    fn against_alone(mixed: &Signal, singles: &[Signal]) -> Vec<usize> {
        (0..mixed.towers)
            .map(|d| {
                let single = &singles[d];
                let mut wrong = usize::from(mixed.count(d) != single.count(0));
                if wrong == 0 {
                    for channel in CHANNELS {
                        wrong += bits_differing(mixed.channel(d, channel), single.channel(0, channel));
                    }
                    wrong += bits_differing(&mixed.readouts(d), &single.readouts(0));
                    wrong += mixed.taken(d).iter().zip(single.taken(0)).filter(|(a, b)| a != b).count();
                }
                wrong
            })
            .collect()
    }

    cav_validate::case! {
        test: validation_unequal_towers_read_as_each_alone,
        name: "array.unequal_towers_read_as_each_alone",
        subsystem: "array",
        slice: "S3",
        reference: Exact,
        against: "four towers of four instruments (AION-10; AICE 0.3 s late; T = 0.9 s; Δr = 4 m and 800 kicks) and two moving bodies: each tower's every shot, every channel and readout time, against a one-tower scene of that tower's resolved instrument, position and offset, at 1 and 6 threads; values that differ in any bit",
        unit: Count,
        tolerance: 0.0,
        measure: {
            let scene = unequal();
            let singles: Vec<Signal> = (0..scene.array.towers.len()).map(|d| read_on(1, &alone(&scene, d))).collect();
            let counts: Vec<usize> = (0..4).map(|d| singles[d].count(0)).collect();
            assert!(counts[1] < counts[0] && counts[2] == counts[0], "ragged: {counts:?}");
            let mut differing = 0;
            for threads in [1, 6] {
                differing += against_alone(&read_on(threads, &scene), &singles).iter().sum::<usize>();
            }
            // One reader for each distinct instrument, shared by the towers that fly it.
            assert_eq!(Readers::new(&scene).readers.len(), 4);
            Measured::Error(differing as f64)
        },
    }

    #[test]
    fn unequal_towers_controls() {
        use crate::InstrumentOverride;
        let scene = unequal();
        let whole = read_on(1, &scene);
        // (a) One tower's T changed by one ulp changes that tower's bits and no other's.
        let mut nudged = scene.clone();
        let t = scene.array.instrument_of(2).half_sequence;
        nudged.array.towers[2].instrument.as_mut().unwrap().half_sequence =
            Some(f64::from_bits(t.to_bits() + 1));
        let moved = read_on(1, &nudged);
        for d in 0..4 {
            let changed = bits_differing(
                moved.channel(d, Channel::Gradiometer),
                whole.channel(d, Channel::Gradiometer),
            );
            assert_eq!(changed > 0, d == 2, "tower {}: {changed} values changed", d + 1);
        }
        // (b) An override equal to the default is no override: the same bits, one reader.
        let mut plain = dev::comb();
        plain.duration = 20.0;
        let mut same = plain.clone();
        same.array.towers[3].instrument = Some(InstrumentOverride {
            half_sequence: Some(plain.array.instrument.half_sequence),
            ..InstrumentOverride::default()
        });
        assert_eq!(same.validate(), Ok(()));
        assert_eq!(bits_differing(&read_on(1, &same).gradiometer, &read_on(1, &plain).gradiometer), 0);
        assert_eq!(Readers::new(&same).readers.len(), 1);
        assert!(same.array.is_uniform() && !scene.array.is_uniform());
        // (c) Swapping two towers' instruments changes what they read, and each then reads as
        // it would alone with its new instrument; swapping whole towers swaps their readings.
        let mut swapped = scene.clone();
        let (a, b) = (swapped.array.towers[0].instrument.clone(), swapped.array.towers[3].instrument.clone());
        (swapped.array.towers[0].instrument, swapped.array.towers[3].instrument) = (b, a);
        let read = read_on(1, &swapped);
        assert!(
            bits_differing(read.channel(0, Channel::Gradiometer), whole.channel(0, Channel::Gradiometer)) > 0
        );
        let singles: Vec<Signal> = (0..4).map(|d| read_on(1, &alone(&swapped, d))).collect();
        assert_eq!(against_alone(&read, &singles), [0, 0, 0, 0]);
        let mut exchanged = scene.clone();
        exchanged.array.towers.swap(1, 2);
        let read = read_on(1, &exchanged);
        for channel in CHANNELS {
            assert_eq!(bits_differing(read.channel(1, channel), whole.channel(2, channel)), 0);
            assert_eq!(bits_differing(read.channel(2, channel), whole.channel(1, channel)), 0);
        }
        // (d) A cache of readings keyed on the readout time alone would give tower 4 (whose shots
        // are tower 1's to the bit, on another instrument) tower 1's numbers: the comparison
        // with each tower alone sees it.
        let shots = shots(&scene).unwrap();
        assert_eq!(bits_differing(shots.times(3), shots.times(0)), 0, "the two towers' shots coincide");
        let mut cached = whole.clone();
        let mut cache: std::collections::HashMap<u64, f64> = std::collections::HashMap::new();
        for d in [0, 3] {
            for k in 0..cached.count(d) {
                let key = cached.readout(d, k).to_bits();
                let at = cached.starts[d] + k;
                let value = *cache.entry(key).or_insert(whole.gradiometer[at]);
                cached.gradiometer[at] = value;
            }
        }
        let singles: Vec<Signal> = (0..4).map(|d| read_on(1, &alone(&scene, d))).collect();
        let wrong = against_alone(&cached, &singles);
        assert!(wrong[3] > 0 && wrong[0] == 0, "{wrong:?}");
    }

    #[test]
    fn a_scene_with_no_towers_is_a_scene() {
        let scene = dev::no_towers();
        assert_eq!(scene.validate(), Ok(()));
        let all = shots(&scene).unwrap();
        assert_eq!((all.towers(), all.total(), all.most(), all.starts.clone()), (0, 0, 0, vec![0]));
        let read = signal(&scene).unwrap();
        assert!(read.gradiometer.is_empty() && read.estimates.is_empty() && read.settled);
        assert_eq!(read.most(), 0);
        let live = reading(&scene, 3.0, &|| false).unwrap();
        assert!(live.towers.is_empty());
        assert!(Readers::new(&scene).readers.is_empty());
        // The schedule still says what it would be, and the box of the scene is finite.
        assert!(!readouts(&scene).unwrap().is_empty());
        let (lo, hi) = scene.bounds();
        assert!(lo.iter().chain(&hi).all(|v| v.is_finite()));
        // Control: one tower put back reads the mass.
        let mut one = scene.clone();
        one.array.towers.push(crate::TowerSpec::at("tower 1", [0.0, 3.0, 0.0]));
        assert!(signal(&one).unwrap().gradiometer.iter().any(|v| *v != 0.0));
    }

    #[test]
    fn a_ragged_array_is_bounded_by_its_sum() {
        // Three towers of 1.5 million shots each: each is within MAX_SHOTS, together they are past
        // the signal's cap, and they are refused by count before anything is made.
        let mut scene = dev::no_towers();
        scene.bodies.clear();
        scene.duration = 1.5e6;
        scene.array.instrument.half_sequence = 0.1;
        scene.array.instrument.cycle = 1.0;
        for (n, label) in ["a", "b", "c"].into_iter().enumerate() {
            scene.array.towers.push(crate::TowerSpec::at(label, [n as f64, 0.0, 0.0]));
        }
        let why = shots(&scene).unwrap_err();
        assert!(why.contains("4500000 shot(s) between them") && why.contains("4000000 values"), "{why}");
        // Control: a timeline a thousand times shorter is made.
        scene.duration = 1.5e3;
        assert_eq!(shots(&scene).unwrap().total(), 4500);
    }
}
