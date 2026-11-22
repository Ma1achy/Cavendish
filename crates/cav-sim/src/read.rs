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

/// When each shot is read out, seconds: a shot is launched every cycle from time zero while the
/// launch is before the end of the timeline, and read out 2T later (the timeline's lanes draw the
/// same shots).
///
/// # Errors
/// When there are more than [`MAX_SHOTS`] of them, or more values than [`MAX_SIGNAL_VALUES`] over
/// the scene's towers. Nothing is reserved before both counts have passed.
pub fn readouts(spec: &SceneSpec) -> Result<Vec<f64>, String> {
    let instrument = &spec.array.instrument;
    let count = (spec.duration / instrument.cycle).ceil();
    if !(count.is_finite() && count <= MAX_SHOTS as f64) {
        return Err(format!(
            "the timeline holds {count} shots ({} s at one every {} s); a signal is computed for at most \
             {MAX_SHOTS}",
            spec.duration, instrument.cycle
        ));
    }
    fits(spec.array.towers.len(), count as usize)?;
    let flight = 2.0 * instrument.half_sequence;
    // k·cycle, not a running sum, so the thousandth shot is as exact as the first.
    Ok((0..count as usize).map(|k| k as f64 * instrument.cycle + flight).collect())
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

/// The contribution of `source` to the shots read out at `readouts`. `cancelled` is asked between
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
    readouts: &[f64],
    cancelled: &dyn Fn() -> bool,
) -> Option<Contribution> {
    let instrument = spec.array.instrument.instrument();
    let reader = Reader::new(instrument);
    let towers = &spec.array.towers;
    if let Err(why) = fits(towers.len(), readouts.len()) {
        panic!("a contribution was asked for a signal that is refused: {why}");
    }
    let count = towers.len() * readouts.len();
    let mut out = Contribution {
        lower: Vec::with_capacity(count),
        upper: Vec::with_capacity(count),
        gradiometer: Vec::with_capacity(count),
        estimate: 0.0,
        settled: true,
        warnings: Vec::new(),
    };
    let signed = |lower: f64, upper: f64| match instrument.sign {
        Sign::LowerMinusUpper => lower - upper,
        Sign::UpperMinusLower => upper - lower,
    };
    match source {
        SourceId::Floor => {
            let floor = spec.floor.as_ref()?;
            // The slab's field is uniform above it, so one reading serves every shot; it is read
            // for each tower all the same, so that the null below is what the forward model gives
            // for that tower and not a zero written here.
            for tower in towers {
                let (lower, upper, difference) = match floor.kind {
                    FloorKind::Slab { thickness } => {
                        let slab = Slab { density: floor.density, thickness };
                        let reading = reader.read(base(tower.position), &slab, 0.0);
                        (reading.lower, reading.upper, signed(reading.lower, reading.upper))
                    }
                    FloorKind::HalfSpace => {
                        // Each interferometer's own phase grows without bound with the depth of
                        // matter; their difference does not depend on the depth at all, and is
                        // read from a slab one metre thick.
                        let unit = reader.read(
                            base(tower.position),
                            &Slab { density: floor.density, thickness: 1.0 },
                            0.0,
                        );
                        (f64::INFINITY, f64::INFINITY, signed(unit.lower, unit.upper))
                    }
                };
                out.lower.extend(std::iter::repeat_n(lower, readouts.len()));
                out.upper.extend(std::iter::repeat_n(upper, readouts.len()));
                out.gradiometer.extend(std::iter::repeat_n(difference, readouts.len()));
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
            for (at, tower) in towers.iter().enumerate() {
                let (mut inside, mut unsettled) = (None, None);
                for (k, &readout) in readouts.iter().enumerate() {
                    if cancelled() {
                        return None;
                    }
                    // The body is watched as it is read: the crossing check costs no node of its own.
                    let watched = Watched::new(&body);
                    let audited = reader.read_to(base(tower.position), &watched, readout, PHASE_TOLERANCE);
                    let (lower, upper) = (audited.reading.lower, audited.reading.upper);
                    let estimate = audited.estimate.unwrap_or(0.0);
                    // Whatever stopped the audit short of its tolerance (the halvings, the work
                    // budget, a reading that is not a number) is said, never passed off as refined.
                    if !audited.settled {
                        unsettled.get_or_insert((k, estimate));
                        out.settled = false;
                    }
                    if inside.is_none() && watched.crossed() {
                        inside = Some(k);
                    }
                    out.estimate = out.estimate.max(if estimate.is_nan() { f64::INFINITY } else { estimate });
                    out.lower.push(lower);
                    out.upper.push(upper);
                    out.gradiometer.push(signed(lower, upper));
                }
                if let Some(k) = inside {
                    out.warnings.push(Warning::about(
                        at,
                        format!(
                            "`{label}` passes through the atoms' path of {} during shot {} (read out at {} s)",
                            tower.label,
                            k + 1,
                            readouts[k]
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
#[derive(Clone, Debug, PartialEq)]
pub struct Signal {
    /// When each shot is read out, seconds.
    pub times: Vec<f64>,
    /// How many towers there are.
    pub towers: usize,
    /// δφ₁ of each shot of each tower, radians: tower by tower, `towers × shots` values.
    pub lower: Vec<f64>,
    /// δφ₂ likewise.
    pub upper: Vec<f64>,
    /// ΔΦ likewise, with the instrument's sign.
    pub gradiometer: Vec<f64>,
    /// The largest quadrature estimate of any source's any shot, radians.
    pub estimate: f64,
    /// Whether every read of every source settled to the tolerance.
    pub settled: bool,
    /// What a person should know about these readings.
    pub warnings: Vec<Warning>,
}

impl Signal {
    /// A signal of zeros for the scene's towers and shots: what the void reads, and what the sum
    /// starts from.
    #[must_use]
    pub fn silent(towers: usize, times: Vec<f64>) -> Self {
        let zeros = vec![0.0; towers * times.len()];
        Self {
            times,
            towers,
            lower: zeros.clone(),
            upper: zeros.clone(),
            gradiometer: zeros,
            estimate: 0.0,
            settled: true,
            warnings: Vec::new(),
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
        self.settled &= contribution.settled;
        self.warnings.extend(contribution.warnings.iter().cloned());
    }

    /// One channel of one tower (from 0): a value for each shot.
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
        let shots = self.times.len();
        &all[tower * shots..(tower + 1) * shots]
    }
}

/// The scene's signal, computed here and now: every source's contribution, summed in the order
/// of [`sources`].
///
/// # Errors
/// When the timeline holds too many shots.
pub fn signal(spec: &SceneSpec) -> Result<Signal, String> {
    signal_at(spec, readouts(spec)?)
}

/// The signal for shots read out at the given times, which need not be the scene's own.
///
/// # Errors
/// When towers × shots is more than [`MAX_SIGNAL_VALUES`].
pub fn signal_at(spec: &SceneSpec, times: Vec<f64>) -> Result<Signal, String> {
    fits(spec.array.towers.len(), times.len())?;
    let mut total = Signal::silent(spec.array.towers.len(), times);
    for source in sources(spec) {
        let part = contribution(spec, source, &total.times, &|| false).expect("not cancelled");
        total.add(&part);
    }
    Ok(total)
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
pub fn reading(spec: &SceneSpec, time: f64, cancelled: &dyn Fn() -> bool) -> Option<ReadingAt> {
    let mut total = Signal::silent(spec.array.towers.len(), vec![time]);
    for source in sources(spec) {
        total.add(&contribution(spec, source, &total.times, cancelled)?);
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
                assert_eq!(signal.times.len() * signal.towers, 30, "five towers, six shots");
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
            assert_eq!(read.times.len(), 30);
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
                let partial = fine.read(base(scene.array.towers[tower].position), &mass(&scene, 0), read.times[2]).lower;
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

    #[test]
    fn a_warning_marks_its_own_tower_only() {
        // Twelve towers in a row, and a point mass on the axis of the twelfth. Its label, "tower
        // 12", begins with the first tower's: a mark found by looking for the label in the words
        // of the warning (as the outliner once did) lights tower 1 as well.
        let mut scene = dev::point();
        scene.array.towers = (0..12)
            .map(|n| crate::TowerSpec {
                label: format!("tower {}", n + 1),
                position: [f64::from(n) * 3.0, 0.0, 0.0],
            })
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
        let live = reading(&scene, read.times[3], &|| false).unwrap().towers;
        for (tower, values) in live.iter().enumerate() {
            assert_eq!(values.gradiometer.to_bits(), read.channel(tower, Channel::Gradiometer)[3].to_bits());
            assert_eq!(values.lower.to_bits(), read.channel(tower, Channel::Lower)[3].to_bits());
        }
        // Control: at another time it is another value.
        assert_ne!(
            reading(&scene, read.times[3] + 0.3, &|| false).unwrap().towers[0].gradiometer,
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
        let asked = std::cell::Cell::new(0);
        let cancel_after_ten = || {
            asked.set(asked.get() + 1);
            asked.get() > 10
        };
        assert_eq!(contribution(&scene, SourceId::Body(0), &read.times, &cancel_after_ten), None);
        assert!(contribution(&scene, SourceId::Body(0), &read.times, &|| false).is_some());
        assert_eq!(sources(&dev::infinite_floor()), [SourceId::Floor]);
        assert_eq!(sources(&dev::superposition()), [SourceId::Body(0), SourceId::Body(1)]);
    }
}
