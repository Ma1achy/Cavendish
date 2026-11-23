//! The timeline: one lane per tower, each shot a bar from launch to readout, and the playhead.
//!
//! Each tower's bars stand at its own times: the array's clock and the tower's offset. A lane
//! dragged sideways with Shift held sets that offset (M13-R14); a plain drag scrubs.
//!
//! Everything here is counted by whole index with a cap. A scene may ask for a million seconds of
//! two-millisecond shots; the lanes then say how many there are instead of drawing them.

use cav_sim::InstrumentSpec;
use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, pos2, vec2};

use crate::theme;

/// One shot: the atoms are launched at `launch` and read out at `readout`, seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub launch: f64,
    pub readout: f64,
}

/// The most shots a lane draws one by one.
pub const MAX_SHOTS_DRAWN: u64 = 400;
/// The most marks on the time ruler.
pub const MAX_RULER_MARKS: u32 = 13;

/// The shots of one tower over a timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct Shots {
    /// How many shots are launched before the end.
    pub total: u64,
    /// Each of them, if there are no more than [`MAX_SHOTS_DRAWN`]; otherwise empty, and the lane
    /// is drawn as one band.
    pub shown: Vec<Shot>,
}

impl Shots {
    /// Whether there are too many shots to draw one by one.
    #[must_use]
    pub fn decimated(&self) -> bool {
        self.total > self.shown.len() as u64
    }
}

/// The shots of one tower over a timeline of `duration` seconds: one per cycle from time zero, each
/// lasting 2T. A shot counts if it is launched before the end; its readout may fall after it.
#[must_use]
pub fn shots(instrument: &InstrumentSpec, duration: f64) -> Shots {
    let flight = 2.0 * instrument.half_sequence;
    // The number of k ≥ 0 with k·cycle < duration. Not a loop: the count can be in the billions.
    let count = (duration / instrument.cycle).ceil();
    let total = if count.is_finite() && count > 0.0 { count.min(1.0e15) as u64 } else { 0 };
    let shown = if total <= MAX_SHOTS_DRAWN {
        (0..total)
            .map(|k| {
                // k·cycle, not a running sum, so the hundredth shot is as exact as the first.
                let launch = k as f64 * instrument.cycle;
                Shot { launch, readout: launch + flight }
            })
            .collect()
    } else {
        Vec::new()
    };
    Shots { total, shown }
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

/// Draws the lanes into the space `ui` has left and reports scrubbing and clicks.
pub fn lanes(
    ui: &mut egui::Ui,
    labels: &[String],
    all_shots: &Shots,
    offsets: &[f64],
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
        let offset = offsets.get(n).copied().unwrap_or(0.0);
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
        if all_shots.decimated() {
            // Too many shots to tell apart: one band, and the panel's caption says how many.
            painter.rect_filled(lane, 1.0, colour.gamma_multiply(0.3));
        }
        for (k, shot) in all_shots.shown.iter().enumerate() {
            // The tower's own times: the array's clock and the tower's offset.
            let (launch, readout) = (shot.launch + offset, shot.readout + offset);
            let bar = Rect::from_min_max(
                pos2(x_of(launch).max(plot.left()), lane.top()),
                pos2(x_of(readout).min(plot.right()), lane.bottom()),
            );
            if k == 0 {
                response.bars.push((
                    n,
                    Rect::from_min_max(pos2(x_of(launch), lane.top()), pos2(x_of(readout), lane.bottom())),
                ));
            }
            if bar.width() <= 0.0 {
                continue;
            }
            // A shot in flight at the playhead is drawn at full strength; the rest are dimmed.
            let flying = time >= launch && time <= readout;
            painter.rect_filled(bar, 1.0, if flying { colour } else { colour.gamma_multiply(0.45) });
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

    #[test]
    fn shots_are_one_cycle_apart_and_last_two_t() {
        let instrument = InstrumentSpec::default();
        let all = shots(&instrument, 12.0);
        // A 2 s cycle over 12 s: launches at 0, 2, …, 10.
        assert_eq!((all.total, all.shown.len(), all.decimated()), (6, 6, false));
        for (k, shot) in all.shown.iter().enumerate() {
            assert_eq!(shot.launch, 2.0 * k as f64);
            assert_eq!(shot.readout, shot.launch + 1.46, "a shot lasts 2T = 1.46 s");
        }
        // No drift: the 334th launch is exactly 333 cycles after the first.
        let slow = InstrumentSpec { cycle: 0.1 + 0.2, half_sequence: 0.1, ..instrument.clone() };
        let many = shots(&slow, 100.0);
        assert_eq!(many.shown[333].launch, 333.0 * (0.1 + 0.2));
        // A shot launched just before the end is listed; one that would launch at the end is not.
        assert_eq!(shots(&instrument, 10.5).total, 6);
        assert_eq!(shots(&instrument, 10.0).total, 5);
        // Control: another cycle gives other shots, so the lanes do follow the instrument.
        let faster = InstrumentSpec { cycle: 1.5, ..instrument };
        assert_eq!(shots(&faster, 12.0).total, 8);
        assert_eq!(shots(&faster, 12.0).shown[1].launch, 1.5);
    }

    #[test]
    fn too_many_shots_are_counted_not_listed() {
        // The longest timeline of the shortest shots a spec allows: half a billion of them.
        let instrument = InstrumentSpec { cycle: 2.1e-3, half_sequence: 1.0e-3, ..InstrumentSpec::default() };
        let started = std::time::Instant::now();
        let all = shots(&instrument, 1.0e6);
        assert_eq!(all.total, 476_190_477);
        assert!(all.shown.is_empty() && all.decimated(), "they are counted, not listed");
        assert!(started.elapsed().as_millis() < 100, "and counting is arithmetic, not a loop");
        // The cap is exact: MAX_SHOTS_DRAWN shots are drawn, one more is not.
        let unit = InstrumentSpec { cycle: 1.0, half_sequence: 0.25, ..InstrumentSpec::default() };
        let at_cap = shots(&unit, MAX_SHOTS_DRAWN as f64);
        assert_eq!((at_cap.shown.len() as u64, at_cap.decimated()), (MAX_SHOTS_DRAWN, false));
        let over = shots(&unit, MAX_SHOTS_DRAWN as f64 + 1.0);
        assert_eq!((over.total, over.shown.len(), over.decimated()), (MAX_SHOTS_DRAWN + 1, 0, true));
        // Numbers that are not numbers give no shots rather than a hang or a panic.
        assert_eq!(shots(&unit, f64::NAN).total, 0);
        assert_eq!(shots(&InstrumentSpec { cycle: 0.0, ..unit }, 10.0).total, 0);
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
