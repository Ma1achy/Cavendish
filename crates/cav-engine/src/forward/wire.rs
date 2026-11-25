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

use cav_sim::SceneSpec;
use cav_sim::read::{self, Contribution, Phases, ReadingAt, SourceId, Warning};

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
const NOTHING: u8 = 0;
/// The world's index in a request (the floor's is `u64::MAX`, a body's its number).
const WORLD: u64 = u64::MAX - 1;

/// The request for `source`'s contribution to the signal of `spec`, at the shots
/// `cav_sim::read::shots(spec)` gives (the ones the engine schedules).
#[must_use]
pub fn contribution_request(spec: &SceneSpec, source: SourceId) -> Vec<u8> {
    let mut out = vec![CONTRIBUTION];
    let index = match source {
        SourceId::Floor => u64::MAX,
        SourceId::World => WORLD,
        SourceId::Body(n) => n as u64,
    };
    out.extend(index.to_le_bytes());
    out.extend(serde_json::to_vec(spec).expect("a scene is JSON"));
    out
}

/// The request for what every tower of `spec` reads from a shot read out at `time`.
#[must_use]
pub fn reading_request(spec: &SceneSpec, time: f64) -> Vec<u8> {
    let mut out = vec![READING];
    out.extend(time.to_bits().to_le_bytes());
    out.extend(serde_json::to_vec(spec).expect("a scene is JSON"));
    out
}

/// What a job's runner does: the request's work, done here, and its answer as bytes. A request
/// that cannot be read, or a scene whose shots are refused, answers nothing (the engine never
/// sends either: it is the same module at both ends).
#[must_use]
pub fn serve(request: &[u8]) -> Vec<u8> {
    let never = || false;
    let Some((&kind, rest)) = request.split_first() else { return vec![NOTHING] };
    let Some((head, json)) = rest.split_first_chunk::<8>() else { return vec![NOTHING] };
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
                    out.warnings(&contribution.warnings);
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
            warnings: reader.warnings()?,
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

    fn float(&mut self, value: f64) {
        self.0.extend(value.to_bits().to_le_bytes());
    }

    fn floats(&mut self, values: &[f64]) {
        self.count(values.len());
        for &value in values {
            self.float(value);
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
        same_bits(&[cav_sim::dev::point(), cav_sim::dev::comb(), cav_sim::dev::pass()]);
    }

    /// The voxel sphere through the wire: run by `cargo xtask web`, not `quick`.
    #[test]
    #[ignore = "8 to 9 s: run by `cargo xtask web`"]
    fn a_voxel_job_through_the_wire_is_the_same_bits() {
        same_bits(&[cav_sim::dev::sphere()]);
    }

    /// Every source of every scene, and the reading at 1 s, sent through the wire and read back:
    /// the same bits as made directly.
    fn same_bits(specs: &[SceneSpec]) {
        let never = || false;
        for spec in specs {
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
                assert_eq!((direct.settled, &direct.warnings), (sent.settled, &sent.warnings));
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
            assert_eq!((direct.settled, &direct.warnings), (sent.settled, &sent.warnings));
        }
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
