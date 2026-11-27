//! A body's drawn path (S20): the curve its motion traces, cut into straight pieces by its
//! geometry, not by the clock.
//!
//! Until S20 a path was 96 pieces across the whole timeline, so a quick circle got a handful of
//! pieces a turn and was drawn as a square (or a star). Now:
//!
//! - the path is drawn only while the body exists (from `appears` to `vanishes`, inside the
//!   timeline), over at most [`PATH_SPAN`] seconds;
//! - a motion that repeats ([`Motion::period`]) is drawn over one period: its whole closed curve,
//!   however many times the timeline goes round it;
//! - it is cut where the motion has a kink ([`Motion::breaks`]), and each smooth stretch starts
//!   as four pieces to each period of the motion's quickest part ([`Motion::fastest`]);
//! - each piece is halved until it is flat: its chord's sag (how far the path's point at the
//!   middle of the piece's time, and at its quarters, is from the chord) is at most
//!   [`SAG_FRACTION`] of the radius of the circle through the chord's ends and that point, or
//!   [`SAG_FLOOR`] metres if that is more.
//!
//! **Bound.** The drawn polyline is within 1 % of the path's local radius of curvature (as
//! estimated from each chord), or 1 cm, of the true path; every vertex is on the path
//! (`position(t)` for a time `t` that is kept with it). **Unless** the path would need more
//! pieces than it is allowed ([`allowance`]: [`PATH_PIECES`] a body, [`SCENE_PATH_PIECES`] a
//! scene) or more than [`MAX_DEPTH`] halvings: then it is drawn as finely as that allows, evenly,
//! and flagged ([`Path::capped`]), which [`crate::decimation`] reports. It is never silently
//! coarse.

use cav_core::Vec3;
use cav_sim::{BodySpec, Motion, SceneSpec};

/// The most straight pieces one body's path is drawn in.
pub const PATH_PIECES: usize = 4096;

/// The most straight pieces the paths of a whole scene are drawn in: shared evenly between the
/// bodies that have paths (a thousand bodies get 131 each).
pub const SCENE_PATH_PIECES: usize = 131_072;

/// The longest stretch of the timeline a path that does not repeat is drawn over, seconds.
pub const PATH_SPAN: f64 = 1.0e4;

/// The largest sag of a chord, as a fraction of the path's local radius of curvature.
pub const SAG_FRACTION: f64 = 0.01;

/// The sag that is always allowed, metres: a path is not cut finer than a centimetre matters.
pub const SAG_FLOOR: f64 = 0.01;

/// How many pieces each period of the motion's quickest part starts as, before any is halved.
pub const SEEDS_PER_PERIOD: f64 = 4.0;

/// The most times a starting piece is halved.
pub const MAX_DEPTH: u32 = 8;

/// A point of a drawn path: when the body is there, seconds on the scene's clock, and where,
/// metres in the world's frame (x east, y north, z up). It is `position(t)` exactly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Knot {
    /// The time, seconds.
    pub t: f64,
    /// The body's position then, metres.
    pub at: Vec3,
}

/// A body's path as it is drawn: one polyline, its knots in order of time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    /// The knots. Each neighbouring pair is one straight piece; fewer than two is no path.
    pub knots: Vec<Knot>,
    /// Whether the path met its cap of pieces (or of halvings) before every piece was flat: it
    /// is then coarser than the bound in this module's notes.
    pub capped: bool,
}

impl Path {
    /// How many straight pieces it has.
    #[must_use]
    pub fn pieces(&self) -> usize {
        self.knots.len().saturating_sub(1)
    }
}

/// Whether a body has a path to draw: its motion goes somewhere (it repeats, or it has a pass's
/// start and stop). A body at rest has none.
#[must_use]
pub fn has_path(body: &BodySpec, gravity: f64) -> bool {
    body.motion.frequency(gravity).is_some() || !body.motion.breaks().is_empty()
}

/// How many pieces each body's path may have in `scene`: [`PATH_PIECES`], or an even share of
/// [`SCENE_PATH_PIECES`] when the scene has so many paths that that is less.
#[must_use]
pub fn allowance(scene: &SceneSpec) -> usize {
    let gravity = scene.array.instrument.gravity;
    let paths = scene.bodies.iter().filter(|b| has_path(b, gravity)).count().max(1);
    PATH_PIECES.min(SCENE_PATH_PIECES / paths)
}

/// The stretch of time a body's path is drawn over, `(from, to)` seconds: its lifetime within
/// the timeline of `duration` seconds, at most [`PATH_SPAN`] long, and one period of a motion
/// that repeats in less. `None` when the body is never there within the timeline.
#[must_use]
pub fn span(body: &BodySpec, gravity: f64, duration: f64) -> Option<(f64, f64)> {
    let (appears, vanishes) = body.exists();
    let from = appears.max(0.0);
    let mut to = vanishes.min(duration).min(from + PATH_SPAN);
    if let Some(period) = body.motion.period(gravity)
        && period < to - from
    {
        to = from + period;
    }
    (to > from).then_some((from, to))
}

/// How far `p` is from the segment `a`–`b`, metres.
fn off_chord(a: Vec3, b: Vec3, p: Vec3) -> f64 {
    let chord = b - a;
    let length_squared = chord.dot(chord);
    if length_squared <= 0.0 {
        return (p - a).length();
    }
    let along = ((p - a).dot(chord) / length_squared).clamp(0.0, 1.0);
    (p - (a + chord * along)).length()
}

/// Whether the piece `a`–`b` is flat enough given `p`, a point of the path between them: `p`'s
/// distance from the chord is within [`SAG_FLOOR`], or within [`SAG_FRACTION`] of the radius of
/// the circle that has this chord and this sag.
#[must_use]
pub fn flat(a: Vec3, b: Vec3, p: Vec3) -> bool {
    let sag = off_chord(a, b, p);
    if sag <= SAG_FLOOR {
        return true;
    }
    let chord = (b - a).length();
    let radius = (0.25 * chord * chord + sag * sag) / (2.0 * sag);
    sag <= SAG_FRACTION * radius
}

/// One piece while the path is being cut: its ends, the path's points at the middle and the
/// quarters of its time once they have been looked at, and whether it has been found flat.
#[derive(Clone, Copy)]
struct Piece {
    a: Knot,
    b: Knot,
    middle: Option<Knot>,
    quarters: Option<[Knot; 2]>,
    flat: bool,
}

/// The path of `motion` from `from` to `to` seconds (a stretch such as [`span`] gives), in at
/// most `allowance` pieces. `gravity` is the site's, metres per second squared. See this
/// module's notes for the rule and its bound.
#[must_use]
pub fn path_of(motion: &Motion, gravity: f64, (from, to): (f64, f64), allowance: usize) -> Path {
    let allowance = allowance.max(1);
    let knot = |t: f64| Knot { t, at: Vec3::from_array(motion.position(t, gravity)) };
    if to <= from || !from.is_finite() || !to.is_finite() {
        return Path::default();
    }
    // The smooth stretches: cut at the motion's kinks.
    let mut cuts = vec![from];
    cuts.extend(motion.breaks().into_iter().filter(|t| *t > from && *t < to));
    cuts.push(to);
    // Four pieces to each period of the quickest part, to begin with.
    let rate = motion.fastest(gravity).max(0.0) * SEEDS_PER_PERIOD;
    let seeds: Vec<f64> = cuts.windows(2).map(|w| ((w[1] - w[0]) * rate).ceil().max(1.0)).collect();
    let wanted: f64 = seeds.iter().sum();
    let mut capped = false;
    let mut pieces: Vec<Piece> = Vec::new();
    for (stretch, &seeds) in cuts.windows(2).zip(&seeds) {
        // Too many to begin with: an even share of what is allowed. (They are then looked at
        // as any piece is: a path that is flat at that, a tiny one, is whole; any other is
        // flagged, since nothing is left to halve it with.)
        let count = if wanted > allowance as f64 {
            let share = (stretch[1] - stretch[0]) / (to - from);
            ((allowance as f64 * share).floor() as usize).max(1)
        } else {
            seeds as usize
        };
        let at = |k: usize| stretch[0] + (stretch[1] - stretch[0]) * (k as f64 / count as f64);
        let mut a = knot(stretch[0]);
        for k in 1..=count {
            // The stretch's own end exactly, not a product that rounds beside it.
            let b = knot(if k == count { stretch[1] } else { at(k) });
            pieces.push(Piece { a, b, middle: None, quarters: None, flat: false });
            a = b;
        }
    }
    // Halve what is not flat, a level at a time, so that a path that meets its cap is evenly
    // fine along its length.
    for depth in 0..=MAX_DEPTH {
        let mut bent = 0;
        for piece in pieces.iter_mut().filter(|p| !p.flat) {
            let (from, to) = (piece.a.t, piece.b.t);
            let middle = *piece.middle.get_or_insert_with(|| knot(0.5 * (from + to)));
            let quarters = *piece
                .quarters
                .get_or_insert_with(|| [knot(0.5 * (from + middle.t)), knot(0.5 * (middle.t + to))]);
            piece.flat =
                [middle, quarters[0], quarters[1]].iter().all(|p| flat(piece.a.at, piece.b.at, p.at));
            bent += usize::from(!piece.flat);
        }
        if bent == 0 {
            break;
        }
        if depth == MAX_DEPTH || pieces.len() + bent > allowance {
            capped = true;
            break;
        }
        let mut halved = Vec::with_capacity(pieces.len() + bent);
        for piece in pieces {
            match (piece.flat, piece.middle, piece.quarters) {
                (false, Some(middle), Some([first, second])) => {
                    let half =
                        |a, b, middle| Piece { a, b, middle: Some(middle), quarters: None, flat: false };
                    halved.push(half(piece.a, middle, first));
                    halved.push(half(middle, piece.b, second));
                }
                _ => halved.push(piece),
            }
        }
        pieces = halved;
    }
    // A stretch on which the body does not move (a pass before it sets off) is no line.
    let mut knots: Vec<Knot> = Vec::with_capacity(pieces.len() + 1);
    for piece in &pieces {
        if piece.a.at == piece.b.at && piece.flat {
            continue;
        }
        if knots.last().map(|k| k.t) != Some(piece.a.t) {
            // (A gap can only be a stretch at rest, so the path is still one polyline.)
            knots.push(piece.a);
        }
        knots.push(piece.b);
    }
    Path { knots, capped }
}

/// The path of body `body` of a scene with this `gravity` (metres per second squared) and
/// `duration` (seconds), in at most `allowance` pieces ([`allowance`]). Empty for a body with no
/// path, or one that is never there.
#[must_use]
pub fn path(body: &BodySpec, gravity: f64, duration: f64, allowance: usize) -> Path {
    if !has_path(body, gravity) {
        return Path::default();
    }
    match span(body, gravity, duration) {
        Some(stretch) => path_of(&body.motion, gravity, stretch, allowance),
        None => Path::default(),
    }
}

/// The labels of the bodies whose paths meet their cap in `scene` and are drawn coarser than
/// this module's bound. Cheap for an ordinary scene: a path whose starting pieces, halved
/// [`MAX_DEPTH`] times, still fit its allowance cannot meet it, and is not cut to find out.
#[must_use]
pub fn capped(scene: &SceneSpec) -> Vec<String> {
    let gravity = scene.array.instrument.gravity;
    let allowance = allowance(scene);
    let mut labels = Vec::new();
    for body in scene.bodies.iter().filter(|b| has_path(b, gravity)) {
        let Some((from, to)) = span(body, gravity, scene.duration) else { continue };
        let stretches = 1.0 + body.motion.breaks().len() as f64;
        let seeds = (to - from) * body.motion.fastest(gravity).max(0.0) * SEEDS_PER_PERIOD + stretches;
        if seeds * f64::from(1_u32 << MAX_DEPTH) <= allowance as f64 {
            continue;
        }
        if path_of(&body.motion, gravity, (from, to), allowance).capped {
            labels.push(body.label.clone());
        }
    }
    labels
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    const G: f64 = 9.81;

    fn circle(radius: f64, frequency: f64) -> Motion {
        Motion::Circular {
            centre: [1.0, -2.0, 3.0],
            u: [radius, 0.0, 0.0],
            v: [0.0, radius, 0.0],
            frequency,
            phase: 0.0,
        }
    }

    /// The largest distance of the true path from the chords of `path`, metres, by looking at
    /// 64 points of each piece.
    fn worst_sag(motion: &Motion, path: &Path) -> f64 {
        let mut worst: f64 = 0.0;
        for pair in path.knots.windows(2) {
            for k in 1..64 {
                let t = pair[0].t + (pair[1].t - pair[0].t) * f64::from(k) / 64.0;
                let p = Vec3::from_array(motion.position(t, G));
                worst = worst.max(off_chord(pair[0].at, pair[1].at, p));
            }
        }
        worst
    }

    #[test]
    fn every_kind_of_motion_is_drawn_within_the_bound_or_flagged() {
        // Each kind, quick enough that the old rule (96 pieces a timeline) was far off: the
        // path stays within the bound of this module's notes, measured at 64 points a piece
        // against the true path, or it says it met its cap.
        let motions = [
            Motion::Linear { centre: [0.0; 3], amplitude: [0.0, 0.0, 1.5], frequency: 7.0, phase: 30.0 },
            circle(0.4, 13.0),
            Motion::Pendulum { pivot: [0.0, 0.0, 4.0], length: 2.0, major: 0.8, minor: 0.3, azimuth: 20.0 },
            Motion::Sphere {
                centre: [0.0, 0.0, 3.0],
                radius: 1.5,
                tilt: 60.0,
                tilt_frequency: 0.7,
                spin_frequency: 0.11,
            },
            Motion::Modes {
                centre: [0.0; 3],
                amplitude: [0.6, 0.2, 0.0],
                modes: vec![
                    cav_sim::ModeSpec { frequency: 2.3, scale: 1.0, phase: 0.0 },
                    cav_sim::ModeSpec { frequency: 12.7, scale: 0.5, phase: 90.0 },
                ],
            },
            Motion::Pass { from: [-8.0, 3.0, 1.0], to: [8.0, 3.0, 1.0], start: 2.0, speed: 2.0 },
        ];
        for motion in &motions {
            let path = path_of(motion, G, (0.0, 12.0), PATH_PIECES);
            assert!(
                path.pieces() >= 1 && path.pieces() <= PATH_PIECES,
                "{}: {}",
                motion.kind(),
                path.pieces()
            );
            assert!(!path.capped, "{}: {} pieces", motion.kind(), path.pieces());
            for knot in &path.knots {
                assert_eq!(knot.at, Vec3::from_array(motion.position(knot.t, G)), "a vertex off the path");
            }
            // The extent of the path, for a scale: no chord strays by more than 2 % of it (the
            // local radius is no larger than that for these curves) or a centimetre and a half.
            let extent = path
                .knots
                .iter()
                .flat_map(|a| path.knots.iter().map(move |b| (a.at - b.at).length()))
                .fold(0.0, f64::max);
            let sag = worst_sag(motion, &path);
            assert!(
                sag <= (0.02 * extent).max(1.5 * SAG_FLOOR),
                "{}: sag {sag} m of {extent} m",
                motion.kind()
            );
        }
        // The pass is one line: from where it sets off to where it stops, and nothing while it waits.
        let pass = path_of(&motions[5], G, (0.0, 12.0), PATH_PIECES);
        assert_eq!(pass.knots.iter().map(|k| k.t).collect::<Vec<_>>(), [2.0, 10.0]);
        // Control: a path that needs more pieces than it may have says so, and stays within them.
        let quick = path_of(&motions[4], G, (0.0, 12.0), 64);
        assert!(quick.capped && quick.pieces() <= 64, "{} pieces", quick.pieces());
        assert!(worst_sag(&motions[4], &quick) > 0.05, "control: capped, it is coarse");
        // And a circle is closed: it ends where it began.
        let ring = path_of(&circle(0.4, 13.0), G, (0.0, 1.0 / 13.0), PATH_PIECES);
        assert!((ring.knots[0].at - ring.knots[ring.pieces()].at).length() < 1e-12);
        // (Sixteen chords: at this radius the centimetre is the larger allowance.)
        let round: f64 = ring.knots.windows(2).map(|w| (w[1].at - w[0].at).length()).sum();
        assert!(round < TAU * 0.4 && TAU * 0.4 - round < 0.02, "{round} m round");
    }
}
