//! The timeline: one lane per tower, each shot a bar from launch to readout, and the playhead.
//!
//! In S0 every tower fires together on the instrument's cycle; per-tower offsets, jitter and
//! masked shots arrive with S2 (M13-R14), which is why the shots come from one function.

use cav_sim::InstrumentSpec;
use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, pos2, vec2};

use crate::theme;

/// One shot: the atoms are launched at `launch` and read out at `readout`, seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub launch: f64,
    pub readout: f64,
}

/// The shots of one tower over a timeline of `duration` seconds: one per cycle from time zero, each
/// lasting 2T. A shot is listed if it is launched before the end; its readout may fall after it.
#[must_use]
pub fn shots(instrument: &InstrumentSpec, duration: f64) -> Vec<Shot> {
    let flight = 2.0 * instrument.half_sequence;
    let mut out = Vec::new();
    let mut k = 0u32;
    loop {
        // k·cycle, not a running sum, so the hundredth shot is as exact as the first.
        let launch = f64::from(k) * instrument.cycle;
        if launch >= duration {
            return out;
        }
        out.push(Shot { launch, readout: launch + flight });
        k += 1;
    }
}

/// Height of one tower's lane, points.
const LANE: f32 = 15.0;
/// Height of the time ruler, points.
const RULER: f32 = 16.0;
/// Width of the lane labels, points.
const LABELS: f32 = 76.0;

/// What the timeline was asked to do this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TimelineResponse {
    /// The time the pointer scrubbed to, if it did.
    pub seek: Option<f64>,
    /// The tower (from 0) whose lane label was clicked, if one was.
    pub select: Option<usize>,
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
    instrument: &InstrumentSpec,
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

    // The ruler: a tick every second, a label every two.
    for second in 0..=duration.floor() as u32 {
        let x = x_of(f64::from(second));
        let major = second % 2 == 0;
        painter.line_segment(
            [pos2(x, plot.top() + if major { 9.0 } else { 12.0 }), pos2(x, plot.top() + RULER)],
            Stroke::new(1.0, theme::WIDGET_EDGE),
        );
        if major {
            painter.text(
                pos2(x + 2.0, plot.top()),
                Align2::LEFT_TOP,
                format!("{second}"),
                mono.clone(),
                theme::WEAK,
            );
        }
    }
    painter.text(pos2(rect.left() + 4.0, plot.top()), Align2::LEFT_TOP, "seconds", mono.clone(), theme::WEAK);

    let all_shots = shots(instrument, duration);
    for (n, label) in labels.iter().enumerate() {
        let top = plot.top() + RULER + LANE * n as f32;
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
        for shot in &all_shots {
            let bar = Rect::from_min_max(
                pos2(x_of(shot.launch), lane.top()),
                pos2(x_of(shot.readout).min(plot.right()), lane.bottom()),
            );
            // A shot in flight at the playhead is drawn at full strength; the rest are dimmed.
            let flying = time >= shot.launch && time <= shot.readout;
            painter.rect_filled(bar, 1.0, if flying { colour } else { colour.gamma_multiply(0.45) });
        }
    }

    // Scrub: drag or click anywhere on the lanes or the ruler.
    let scrub = ui.interact(plot, egui::Id::unique("timeline scrub"), Sense::click_and_drag());
    if (scrub.dragged() || scrub.clicked())
        && let Some(pointer) = scrub.interact_pointer_pos()
    {
        let t = f64::from((pointer.x - plot.left()) / plot.width()) * duration;
        response.seek = Some(t.clamp(0.0, duration));
    }
    // The playhead crosses every lane.
    let x = x_of(time);
    painter.line_segment(
        [pos2(x, plot.top() + 2.0), pos2(x, plot.bottom() - 2.0)],
        Stroke::new(1.5, theme::STRONG),
    );
    painter.circle_filled(pos2(x, plot.top() + 3.0), 3.0, theme::STRONG);
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
        assert_eq!(all.len(), 6);
        for (k, shot) in all.iter().enumerate() {
            assert_eq!(shot.launch, 2.0 * k as f64);
            assert_eq!(shot.readout, shot.launch + 1.46, "a shot lasts 2T = 1.46 s");
        }
        // No drift: the thousandth launch is exactly 999 cycles after the first.
        let slow = InstrumentSpec { cycle: 0.1 + 0.2, half_sequence: 0.1, ..instrument.clone() };
        let many = shots(&slow, 400.0);
        assert_eq!(many[999].launch, 999.0 * (0.1 + 0.2));
        // A shot launched just before the end is listed; one that would launch at the end is not.
        assert_eq!(shots(&instrument, 10.5).len(), 6);
        assert_eq!(shots(&instrument, 10.0).len(), 5);
        // Control: another cycle gives other shots, so the lanes do follow the instrument.
        let faster = InstrumentSpec { cycle: 1.5, ..instrument };
        assert_eq!(shots(&faster, 12.0).len(), 8);
        assert_eq!(shots(&faster, 12.0)[1].launch, 1.5);
    }
}
