//! The timeline: one lane per tower, each shot a bar from launch to readout, and the playhead.
//!
//! The bars are the scene's shots as the engine keeps them (`cav_sim::read::Shots`, the one
//! source of shot times): each tower's bars stand at its own times, the array's clock and the
//! tower's offset, and a shot the tower does not take (a planned gap, the daily window, a lost
//! shot) is the bar's outline only. A lane dragged sideways with Shift held sets the tower's
//! offset (M13-R14); a plain drag scrubs.
//!
//! Everything here is counted by whole index with a cap. A scene may ask for a million seconds of
//! two-millisecond shots; the lanes then say how many there are instead of drawing them.

use cav_sim::read::{Flown, Shots};
use egui::{Align2, Color32, FontId, Rect, Sense, Shape, Stroke, StrokeKind, pos2, vec2};

use crate::theme;

/// The most shots a lane draws one by one.
pub const MAX_SHOTS_DRAWN: usize = 400;
/// The most marks on the time ruler.
pub const MAX_RULER_MARKS: u32 = 13;

/// Whether a lane has too many shots to draw one by one: it is then one band.
#[must_use]
pub fn decimated(shots: &Shots) -> bool {
    shots.times.len() > MAX_SHOTS_DRAWN
}

/// The shots of tower `tower` (from 0) its lane draws one by one, in order: every shot of the
/// schedule, if there are no more than [`MAX_SHOTS_DRAWN`]; none otherwise. Nothing is listed:
/// each is read from `shots` as it is drawn.
pub fn drawn(shots: &Shots, tower: usize) -> impl Iterator<Item = Flown> + '_ {
    let count = if decimated(shots) { 0 } else { shots.times.len() };
    (0..count).map(move |k| shots.flown(tower, k))
}

/// One bar a lane drew.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    /// The tower, from 0.
    pub tower: usize,
    /// Which shot of the schedule, from 0.
    pub k: usize,
    /// From the tower's launch to its readout, before it is cut to the lane.
    pub rect: Rect,
    /// Whether the bar is filled: the tower takes the shot. An outline otherwise.
    pub filled: bool,
}

/// The shape of a bar: filled with `colour` for a shot taken, its outline alone for one that is
/// not. A shot in flight at the playhead is at full strength; the rest are dimmed.
pub fn bar_shape(rect: Rect, colour: Color32, taken: bool, flying: bool) -> Shape {
    let colour = if flying { colour } else { colour.gamma_multiply(0.45) };
    if taken {
        Shape::rect_filled(rect, 1.0, colour)
    } else {
        Shape::rect_stroke(rect, 1.0, Stroke::new(1.0, colour), StrokeKind::Inside)
    }
}

/// What the timeline's heading says about the shots: how long one is, how many the schedule
/// plans and how many each tower takes (`taken` is the least and the most over the towers), or
/// why there are none. `shots` is the engine's answer (`Engine::shots`).
#[must_use]
pub fn caption(shots: Result<&Shots, &str>, mean_cycle: f64, taken: Option<(usize, usize)>) -> String {
    let shots = match shots {
        Ok(shots) => shots,
        Err(why) => return format!("one lane per tower · no shots are drawn: {why}"),
    };
    let planned = shots.times.len();
    let count = match taken {
        Some((fewest, most)) if most < planned => {
            if fewest == most {
                format!("{planned} shots planned, {most} taken a tower")
            } else {
                format!("{planned} shots planned, {fewest} to {most} taken a tower")
            }
        }
        _ => format!("{planned} shots a tower"),
    };
    if decimated(shots) {
        // Said in words, so that a lane drawn as one band is not mistaken for one long shot.
        format!(
            "one lane per tower · {count}, too many to draw one by one (one every {mean_cycle} s on average) · drag to scrub"
        )
    } else {
        let hollow = if matches!(taken, Some((fewest, _)) if fewest < planned) {
            " (an outline: not taken)"
        } else {
            ""
        };
        format!(
            "one lane per tower · each bar is a shot, launch to readout (2T = {:.2} s) · {count}{hollow} · drag to scrub · Shift-drag a lane to move its clock",
            shots.flight
        )
    }
}

/// The spacing of the ruler's marks for a timeline of `duration` seconds: the smallest of 1, 2 or 5
/// times a power of ten that needs no more than [`MAX_RULER_MARKS`] marks.
#[must_use]
pub fn ruler_step(duration: f64) -> f64 {
    if !(duration.is_finite() && duration > 0.0) {
        return 1.0;
    }
    let most = f64::from(MAX_RULER_MARKS - 1);
    let mut decade = 10f64.powf((duration / most).log10().floor());
    // At most a few steps: the first candidate is within a factor of ten of the answer.
    for _ in 0..4 {
        for factor in [1.0, 2.0, 5.0] {
            if duration / (factor * decade) <= most {
                return factor * decade;
            }
        }
        decade *= 10.0;
    }
    duration
}

/// Height of one tower's lane, points.
const LANE: f32 = 15.0;
/// Height of the time ruler, points.
const RULER: f32 = 16.0;
/// Width of the lane labels, points.
const LABELS: f32 = 76.0;
/// The most lanes shown without scrolling.
pub const MAX_LANES_SHOWN: usize = 8;

/// What the timeline was asked to do this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TimelineResponse {
    /// The time the pointer scrubbed to, if it did.
    pub seek: Option<f64>,
    /// The tower (from 0) whose lane label was clicked, if one was.
    pub select: Option<usize>,
    /// A lane dragged sideways with Shift held: the tower (from 0) and how many seconds later
    /// its shots should now be than they were at the last frame.
    pub offset: Option<(usize, f64)>,
    /// Whether a drag of a lane ended this frame.
    pub offset_ended: bool,
    /// Where the first shot of each lane that was drawn stands: the tower (from 0) and its bar.
    pub bars: Vec<(usize, Rect)>,
    /// Every bar drawn: at most [`MAX_SHOTS_DRAWN`] for each lane in view.
    pub shots: Vec<Bar>,
    /// The ruler's numbers, with the rectangle each took.
    pub ruler: Vec<(String, Rect)>,
    /// The rectangle everything is drawn inside: what is outside it is cut off.
    pub rect: Option<Rect>,
}

/// The height the lanes need for `towers` towers, points.
#[must_use]
pub fn height(towers: usize) -> f32 {
    RULER + LANE * towers as f32 + 6.0
}

/// Draws the lanes into the space `ui` has left and reports scrubbing and clicks. `shots` are
/// the scene's, or `None` when it has none to draw (a scene whose signal is refused): the lanes
/// are then bands.
pub fn lanes(
    ui: &mut egui::Ui,
    labels: &[String],
    shots: Option<&Shots>,
    duration: f64,
    time: f64,
    selected: Option<usize>,
) -> TimelineResponse {
    let mut response = TimelineResponse::default();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height(labels.len())), Sense::hover());
    let plot =
        Rect::from_min_max(pos2(rect.left() + LABELS, rect.top()), pos2(rect.right() - 18.0, rect.bottom()));
    let painter = ui.painter_at(rect);
    let x_of = |t: f64| plot.left() + (t / duration) as f32 * plot.width();
    let mono = FontId::monospace(10.5);

    // The ruler: marks at whole multiples of a round step, counted and capped.
    let step = ruler_step(duration);
    for k in 0..=MAX_RULER_MARKS {
        let at = f64::from(k) * step;
        if at > duration {
            break;
        }
        let x = x_of(at);
        painter.line_segment(
            [pos2(x, plot.top() + 9.0), pos2(x, plot.top() + RULER)],
            Stroke::new(1.0, theme::WIDGET_EDGE),
        );
        // A number stands to the right of its mark; one that would run off the end of the
        // timeline (the last, at the end of the run) stands to the left of it instead.
        let text = format!("{at}");
        let wide = painter.layout_no_wrap(text.clone(), mono.clone(), theme::WEAK).size().x;
        let (at, anchor) = if x + 2.0 + wide > rect.right() {
            (pos2(x - 2.0, plot.top()), Align2::RIGHT_TOP)
        } else {
            (pos2(x + 2.0, plot.top()), Align2::LEFT_TOP)
        };
        let took = painter.text(at, anchor, &text, mono.clone(), theme::WEAK);
        response.ruler.push((text, took));
    }
    response.rect = Some(rect);
    painter.text(pos2(rect.left() + 4.0, plot.top()), Align2::LEFT_TOP, "seconds", mono.clone(), theme::WEAK);

    // Only the lanes in view are drawn: an array of a thousand towers costs what eight do.
    let seen = ui.clip_rect();
    let shift = ui.input(|i| i.modifiers.shift);
    for (n, label) in labels.iter().enumerate() {
        let top = plot.top() + RULER + LANE * n as f32;
        if top + LANE < seen.top() || top > seen.bottom() {
            continue;
        }
        let lane = Rect::from_min_max(pos2(plot.left(), top + 2.0), pos2(plot.right(), top + LANE - 2.0));
        let colour = theme::tower(n);
        let is_selected = selected == Some(n);
        // The label is a button: clicking it selects the tower.
        let label_rect = Rect::from_min_max(pos2(rect.left(), top), pos2(plot.left() - 4.0, top + LANE));
        let label_response = ui.interact(label_rect, egui::Id::unique(("lane", n)), Sense::click());
        if label_response.clicked() {
            response.select = Some(n);
        }
        if is_selected || label_response.hovered() {
            painter.rect_filled(label_rect, 2.0, if is_selected { theme::SELECTION } else { theme::HOVER });
        }
        painter.rect_filled(
            Rect::from_min_size(pos2(rect.left() + 5.0, top + 4.0), vec2(7.0, 7.0)),
            1.0,
            colour,
        );
        let text = if is_selected { theme::STRONG } else { theme::TEXT };
        painter.text(
            pos2(rect.left() + 17.0, top + LANE * 0.5),
            Align2::LEFT_CENTER,
            label,
            mono.clone(),
            text,
        );
        painter.rect_filled(lane, 1.0, Color32::from_rgb(0x15, 0x15, 0x15));
        // A tower the shots do not know (a frame between an edit and its shots) draws nothing.
        let Some(shots) = shots.filter(|shots| n < shots.towers()) else {
            painter.rect_filled(lane, 1.0, colour.gamma_multiply(0.15));
            continue;
        };
        if decimated(shots) {
            // Too many shots to tell apart: one band, and the panel's caption says how many.
            painter.rect_filled(lane, 1.0, colour.gamma_multiply(0.3));
        }
        // The tower's own times: the array's clock and the tower's offset.
        for shot in drawn(shots, n) {
            let whole = Rect::from_min_max(
                pos2(x_of(shot.launch), lane.top()),
                pos2(x_of(shot.readout), lane.bottom()),
            );
            if shot.k == 0 {
                response.bars.push((n, whole));
            }
            response.shots.push(Bar { tower: n, k: shot.k, rect: whole, filled: shot.taken });
            let bar = whole.intersect(lane);
            if bar.width() <= 0.0 {
                continue;
            }
            let flying = time >= shot.launch && time <= shot.readout;
            painter.add(bar_shape(bar, colour, shot.taken, flying && shot.taken));
        }
    }

    // Scrub: drag or click anywhere on the lanes or the ruler.
    let scrub = ui.interact(plot, egui::Id::unique("timeline scrub"), Sense::click_and_drag());
    // Shift and a sideways drag on a lane moves that tower's clock: so many points, so many
    // seconds, at the timeline's own scale. The lane is the one the drag began on.
    if shift
        && scrub.dragged()
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
    {
        let row = ((origin.y - plot.top() - RULER) / LANE).floor();
        if row >= 0.0 && (row as usize) < labels.len() {
            response.offset = Some((row as usize, f64::from(scrub.drag_delta().x / plot.width()) * duration));
        }
    }
    if scrub.drag_stopped() {
        response.offset_ended = true;
    }
    if !shift
        && (scrub.dragged() || scrub.clicked())
        && let Some(pointer) = scrub.interact_pointer_pos()
    {
        let t = f64::from((pointer.x - plot.left()) / plot.width()) * duration;
        response.seek = Some(t.clamp(0.0, duration));
    }
    // The playhead crosses every lane.
    let x = x_of(time);
    painter.line_segment(
        [pos2(x, plot.top() + 2.0), pos2(x, plot.bottom() - 2.0)],
        Stroke::new(1.5, theme::PLAYHEAD),
    );
    painter.circle_filled(pos2(x, plot.top() + 3.0), 3.0, theme::PLAYHEAD);
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use cav_sim::{GapSpec, InstrumentSpec, SceneSpec};

    fn scene(instrument: InstrumentSpec, duration: f64) -> SceneSpec {
        let mut scene = SceneSpec::room();
        (scene.array.instrument, scene.duration) = (instrument, duration);
        scene
    }

    fn shots(instrument: InstrumentSpec, duration: f64) -> Shots {
        cav_sim::read::shots(&scene(instrument, duration)).unwrap()
    }

    #[test]
    fn shots_are_one_cycle_apart_and_last_two_t() {
        let instrument = InstrumentSpec::default();
        let all = shots(instrument.clone(), 12.0);
        // A 2 s cycle over 12 s: launches at 0, 2, …, 10.
        assert_eq!((all.times.len(), drawn(&all, 0).count(), decimated(&all)), (6, 6, false));
        for (k, shot) in drawn(&all, 0).enumerate() {
            assert!((shot.launch - 2.0 * k as f64).abs() < 1e-12);
            assert_eq!(shot.readout, 2.0 * k as f64 + 1.46, "a shot lasts 2T = 1.46 s");
            assert!(shot.taken && shot.k == k);
        }
        // No drift: the 334th readout is exactly 333 cycles and 2T after zero.
        let slow = InstrumentSpec { cycle: 0.1 + 0.2, half_sequence: 0.1, ..instrument.clone() };
        let many = shots(slow, 100.0);
        assert_eq!(drawn(&many, 0).nth(333).unwrap().readout, 333.0 * (0.1 + 0.2) + 0.2);
        // A shot launched just before the end is listed; one that would launch at the end is not.
        assert_eq!(shots(instrument.clone(), 10.5).times.len(), 6);
        assert_eq!(shots(instrument.clone(), 10.0).times.len(), 5);
        // Control: another cycle gives other shots, so the lanes do follow the instrument.
        let faster = shots(InstrumentSpec { cycle: 1.5, ..instrument }, 12.0);
        assert_eq!(faster.times.len(), 8);
        assert!((drawn(&faster, 0).nth(1).unwrap().launch - 1.5).abs() < 1e-12);
    }

    #[test]
    fn too_many_shots_are_counted_not_listed() {
        // The longest timeline of the shortest shots a spec allows: half a billion of them.
        // They are counted by arithmetic and refused with the count: nothing is listed, so
        // there is nothing for a lane to draw and the caption says why.
        let instrument = InstrumentSpec { cycle: 2.1e-3, half_sequence: 1.0e-3, ..InstrumentSpec::default() };
        let refused = cav_sim::read::shots(&scene(instrument, 1.0e6)).unwrap_err();
        assert!(refused.contains("476190477 shots"), "{refused}");
        let said = caption(Err(&refused), 2.1e-3, None);
        assert!(said.contains("no shots are drawn") && said.contains("476190477"), "{said}");
        // The cap is exact: MAX_SHOTS_DRAWN shots are drawn, one more is not.
        let unit = InstrumentSpec { cycle: 1.0, half_sequence: 0.25, ..InstrumentSpec::default() };
        let at_cap = shots(unit.clone(), MAX_SHOTS_DRAWN as f64);
        assert_eq!((drawn(&at_cap, 0).count(), decimated(&at_cap)), (MAX_SHOTS_DRAWN, false));
        let over = shots(unit, MAX_SHOTS_DRAWN as f64 + 1.0);
        assert_eq!(
            (over.times.len(), drawn(&over, 0).count(), decimated(&over)),
            (MAX_SHOTS_DRAWN + 1, 0, true)
        );
        assert!(caption(Ok(&over), 1.0, Some((401, 401))).contains("401 shots a tower, too many to draw"));
        // No shots at all draw nothing rather than hang or panic.
        assert_eq!(drawn(&Shots::default(), 0).count(), 0);
    }

    #[test]
    fn a_bar_is_filled_only_when_the_shot_is_taken() {
        let rect = Rect::from_min_size(pos2(10.0, 5.0), vec2(30.0, 11.0));
        let colour = Color32::from_rgb(200, 120, 40);
        let Shape::Rect(hollow) = bar_shape(rect, colour, false, false) else { panic!("a rectangle") };
        assert_eq!(hollow.fill, Color32::TRANSPARENT, "a shot not taken has no fill");
        assert!(hollow.stroke.width > 0.0 && hollow.stroke.color != Color32::TRANSPARENT);
        assert_eq!(hollow.rect, rect);
        // Control: a shot taken is filled and has no outline.
        let Shape::Rect(solid) = bar_shape(rect, colour, true, true) else { panic!("a rectangle") };
        assert_eq!((solid.fill, solid.stroke.width), (colour, 0.0));
    }

    #[test]
    fn the_caption_counts_what_the_schedule_takes() {
        let mut gapped = scene(InstrumentSpec::default(), 60.0);
        gapped.array.schedule.gaps = Some(GapSpec { on: 4, period: 10 });
        let all = cav_sim::read::shots(&gapped).unwrap();
        let taken = all.taken_all(0).iter().filter(|taken| **taken).count();
        assert_eq!((all.times.len(), taken), (30, 12));
        let said = caption(Ok(&all), 2.0, Some((taken, taken)));
        assert!(said.contains("30 shots planned, 12 taken a tower (an outline: not taken)"), "{said}");
        assert!(caption(Ok(&all), 2.0, Some((9, 12))).contains("30 shots planned, 9 to 12 taken a tower"));
        // Control: a schedule that takes every shot says only how many there are.
        let plain = shots(InstrumentSpec::default(), 60.0);
        let said = caption(Ok(&plain), 2.0, Some((30, 30)));
        assert!(said.contains("30 shots a tower ·") && !said.contains("taken"), "{said}");
    }

    #[test]
    fn the_ruler_has_a_round_step_and_few_marks() {
        for (duration, step) in
            [(12.0, 1.0), (13.0, 2.0), (60.0, 5.0), (600.0, 50.0), (1.0e6, 1.0e5), (0.5, 0.05)]
        {
            assert!(
                (ruler_step(duration) / step - 1.0).abs() < 1e-12,
                "a timeline of {duration} s: {}",
                ruler_step(duration)
            );
        }
        for duration in [1.0e-3, 0.7, 12.0, 97.0, 86_400.0, 1.0e6, 1.0e300] {
            let marks = (duration / ruler_step(duration)).floor() + 1.0;
            assert!((2.0..=f64::from(MAX_RULER_MARKS)).contains(&marks), "{duration} s has {marks} marks");
        }
        // Control: a step of one second on a day-long timeline would be 86 401 marks.
        assert!(86_400.0 / 1.0 > f64::from(MAX_RULER_MARKS));
        assert_eq!(ruler_step(f64::NAN), 1.0);
        assert_eq!(ruler_step(0.0), 1.0);
    }
}
