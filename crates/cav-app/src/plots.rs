//! The plots a pane can show in place of the scene: every tower's signal against time, and the
//! periodogram of a tower's signal with each oscillating body's comb drawn over it.
//!
//! Drawn with egui's painter: lines and text, no plotting crate. The mapping from data to the
//! pane is [`Axis`], plain arithmetic that the tests hold to; what was drawn is recorded in the
//! shell's layout ([`Drawn`]) so that a test can ask where a point went without reading pixels.

use cav_engine::{ChannelName, PaneContent, Query, Reply, SignalState};
use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, pos2, vec2};

use crate::shell::{Shell, phase_text};
use crate::theme;

/// One axis of a plot: a range of values laid along a range of points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axis {
    /// The value at the start of the axis and at its end.
    pub lo: f64,
    pub hi: f64,
    /// Whether the axis is logarithmic. Both ends are then positive.
    pub log: bool,
}

impl Axis {
    /// A linear axis that holds every value with a twelfth of the span to spare at each end; a
    /// flat series gets a span of its own size (or one, at zero) so that it is not a division by
    /// nothing.
    #[must_use]
    pub fn linear(values: impl IntoIterator<Item = f64>) -> Self {
        let (lo, hi) = values
            .into_iter()
            .filter(|v| v.is_finite())
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)));
        if lo > hi {
            return Self { lo: -1.0, hi: 1.0, log: false };
        }
        let span = if hi > lo { hi - lo } else { lo.abs().max(1.0) };
        Self { lo: lo - span / 12.0, hi: hi + span / 12.0, log: false }
    }

    /// A logarithmic axis from the decade below the largest value down `decades` decades, or as
    /// far as the smallest positive value if that is nearer.
    #[must_use]
    pub fn log(values: impl IntoIterator<Item = f64>, decades: f64) -> Self {
        let (lo, hi) = values
            .into_iter()
            .filter(|v| v.is_finite() && *v > 0.0)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)));
        if lo > hi {
            return Self { lo: 1e-9, hi: 1.0, log: true };
        }
        let top = 10f64.powf(hi.log10().ceil());
        Self { lo: lo.max(top / 10f64.powf(decades)) * 0.7, hi: top, log: true }
    }

    /// How far along the axis a value is: 0 at `lo`, 1 at `hi`; outside that when it is off the axis.
    #[must_use]
    pub fn fraction(&self, value: f64) -> f64 {
        if self.log {
            (value.max(f64::MIN_POSITIVE).log10() - self.lo.log10()) / (self.hi.log10() - self.lo.log10())
        } else {
            (value - self.lo) / (self.hi - self.lo)
        }
    }
}

/// Where a data point goes in `area`: x from left to right, y from the bottom up.
#[must_use]
pub fn place(area: Rect, x: &Axis, y: &Axis, point: (f64, f64)) -> Pos2 {
    pos2(
        area.left() + area.width() * x.fraction(point.0) as f32,
        area.bottom() - area.height() * y.fraction(point.1) as f32,
    )
}

/// What a plot drew, for tests and for the layout: which pane, what kind, the area inside the
/// axes, the axes, and the marks.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
    pub pane: usize,
    pub content: PaneContent,
    pub area: Rect,
    pub x: Axis,
    pub y: Axis,
    /// How many data points were joined, over all the series.
    pub points: usize,
    /// The playhead's x, for a signal plot.
    pub playhead: Option<f32>,
    /// Each comb stem: the body's label, the harmonic, where it stands, and whether it is folded.
    pub stems: Vec<(String, u32, f32, bool)>,
    /// What the plot says when it has nothing to draw.
    pub note: Option<String>,
}

/// The most points a series is drawn with; a longer one is thinned to its extremes in each
/// stretch, so that a peak is never lost between two drawn points.
pub const MAX_POINTS: usize = 2000;

/// A series thinned to at most about `MAX_POINTS` points, keeping the smallest and the largest
/// of every stretch.
#[must_use]
pub fn thinned(x: &[f64], y: &[f64]) -> Vec<(f64, f64)> {
    let n = x.len().min(y.len());
    if n <= MAX_POINTS {
        return x.iter().zip(y).map(|(a, b)| (*a, *b)).collect();
    }
    let stretch = (2 * n).div_ceil(MAX_POINTS);
    let mut out = Vec::with_capacity(MAX_POINTS + 2);
    for start in (0..n).step_by(stretch) {
        let end = (start + stretch).min(n);
        let lowest = (start..end).min_by(|&a, &b| y[a].total_cmp(&y[b])).expect("a stretch");
        let highest = (start..end).max_by(|&a, &b| y[a].total_cmp(&y[b])).expect("a stretch");
        for k in [lowest.min(highest), lowest.max(highest)] {
            out.push((x[k], y[k]));
        }
    }
    out
}

fn frame(painter: &egui::Painter, rect: Rect, title: &str) -> Rect {
    painter.rect_filled(rect, 0.0, theme::BACKGROUND);
    painter.text(
        rect.left_top() + vec2(8.0, 5.0),
        Align2::LEFT_TOP,
        title,
        FontId::proportional(12.0),
        theme::STRONG,
    );
    let area = Rect::from_min_max(rect.min + vec2(104.0, 26.0), rect.max - vec2(14.0, 24.0));
    painter.rect_stroke(area, 0.0, Stroke::new(1.0, theme::RULE), egui::StrokeKind::Outside);
    area
}

fn nothing(painter: &egui::Painter, rect: Rect, why: &str) {
    painter.text(rect.center(), Align2::CENTER_CENTER, why, FontId::proportional(12.0), theme::WEAK);
}

fn label(painter: &egui::Painter, at: Pos2, anchor: Align2, text: impl Into<String>) {
    painter.text(at, anchor, text.into(), FontId::monospace(10.5), theme::WEAK);
}

fn polyline(painter: &egui::Painter, area: Rect, points: Vec<Pos2>, colour: Color32) {
    let clip = painter.with_clip_rect(area.expand(1.0));
    clip.add(egui::Shape::line(points, Stroke::new(1.3, colour)));
}

/// Draws what pane `pane` shows, if it is a plot, into `rect`, and records what was drawn.
pub fn draw(shell: &mut Shell, ui: &egui::Ui, pane: usize, rect: Rect) {
    let content = shell.engine.content(pane);
    let painter = ui.painter_at(rect);
    let drawn = match content {
        PaneContent::Scene => return,
        PaneContent::Signal => signal(shell, &painter, pane, rect),
        PaneContent::Periodogram => periodogram(shell, &painter, pane, rect),
    };
    shell.layout.plots.push(drawn);
}

fn state_word(shell: &Shell) -> &'static str {
    match shell.engine.signal().0 {
        SignalState::Refined => "refined",
        SignalState::Stale => "stale",
        SignalState::None => "none",
    }
}

fn signal(shell: &Shell, painter: &egui::Painter, pane: usize, rect: Rect) -> Drawn {
    let spec = shell.engine.spec();
    let title = format!("Signal · ΔΦ of every tower · {} · {}", spec.name, state_word(shell));
    let area = frame(painter, rect, &title);
    let x = Axis { lo: 0.0, hi: spec.duration, log: false };
    let mut drawn = Drawn {
        pane,
        content: PaneContent::Signal,
        area,
        x,
        y: Axis { lo: -1.0, hi: 1.0, log: false },
        points: 0,
        playhead: None,
        stems: Vec::new(),
        note: None,
    };
    let Some(signal) = shell.engine.signal().1 else {
        let why = "computing the signal";
        nothing(painter, rect, why);
        drawn.note = Some(why.into());
        return drawn;
    };
    let channel = ChannelName::Gradiometer.channel();
    let towers = signal.towers.min(spec.array.towers.len());
    let y = Axis::linear((0..towers).flat_map(|t| signal.channel(t, channel).iter().copied()));
    drawn.y = y;
    for tower in 0..towers {
        let series = thinned(&signal.times, signal.channel(tower, channel));
        drawn.points += series.len();
        let points: Vec<Pos2> = series.iter().map(|p| place(area, &x, &y, *p)).collect();
        // Few shots are drawn as dots as well: six points joined by lines hide that they are six.
        if points.len() <= 200 {
            for point in &points {
                painter.circle_filled(*point, 2.5, theme::tower(tower));
            }
        }
        polyline(painter, area, points, theme::tower(tower));
    }
    // The playhead, and the axes' ends in words.
    let at = place(area, &x, &y, (shell.engine.time(), y.lo)).x;
    painter.vline(at, area.y_range(), Stroke::new(1.0, theme::LINK));
    drawn.playhead = Some(at);
    label(painter, area.left_bottom() + vec2(0.0, 4.0), Align2::LEFT_TOP, "0 s");
    label(
        painter,
        area.right_bottom() + vec2(0.0, 4.0),
        Align2::RIGHT_TOP,
        format!("{} s · readout time", spec.duration),
    );
    label(painter, area.left_top() + vec2(-4.0, 0.0), Align2::RIGHT_TOP, phase_text(y.hi));
    label(painter, area.left_bottom() + vec2(-4.0, 0.0), Align2::RIGHT_BOTTOM, phase_text(y.lo));
    if y.lo < 0.0 && y.hi > 0.0 {
        let zero = place(area, &x, &y, (0.0, 0.0)).y;
        painter.hline(area.x_range(), zero, Stroke::new(1.0, theme::RULE));
    }
    drawn
}

fn periodogram(shell: &Shell, painter: &egui::Painter, pane: usize, rect: Rect) -> Drawn {
    let tower = shell.selected_tower().map_or(1, |n| n + 1);
    let query = Query::Periodogram { tower, channel: ChannelName::Gradiometer };
    let spec = shell.engine.spec();
    let label_of = spec.array.towers.get(tower - 1).map_or("tower", |t| t.label.as_str());
    let title = format!("Periodogram · √S of ΔΦ at {label_of} · {} · {}", spec.name, state_word(shell));
    let area = frame(painter, rect, &title);
    let nyquist = 0.5 / spec.array.instrument.cycle;
    let x = Axis { lo: 0.0, hi: nyquist, log: false };
    let mut drawn = Drawn {
        pane,
        content: PaneContent::Periodogram,
        area,
        x,
        y: Axis { lo: 1e-9, hi: 1.0, log: true },
        points: 0,
        playhead: None,
        stems: Vec::new(),
        note: None,
    };
    let spectrum = match shell.engine.query(&query) {
        Ok(Reply::Periodogram(spectrum)) if spectrum.power.len() >= 2 => spectrum,
        other => {
            let why = match other {
                Err(why) => why,
                _ => "too few shots for a periodogram".to_owned(),
            };
            nothing(painter, rect, &why);
            drawn.note = Some(why);
            return drawn;
        }
    };
    let amplitude: Vec<f64> = spectrum.power.iter().map(|p| p.sqrt()).collect();
    if !amplitude.iter().any(|a| *a > 0.0) {
        let why = "the signal is constant: nothing to resolve";
        nothing(painter, rect, why);
        drawn.note = Some(why.into());
        return drawn;
    }
    let y = Axis::log(amplitude.iter().copied(), 9.0);
    drawn.y = y;
    let series = thinned(&spectrum.frequencies, &amplitude);
    drawn.points = series.len();
    let points: Vec<Pos2> = series.iter().map(|p| place(area, &x, &y, (p.0, p.1.max(y.lo)))).collect();
    polyline(painter, area, points, theme::tower(tower - 1));
    // The combs: a stem at each harmonic, at the height the spectrum has there. A harmonic above
    // Nyquist stands where it folds to, dashed, and says so.
    let palette = [theme::GOLD, theme::LINK, theme::ERROR, theme::STRONG];
    for (n, comb) in spectrum.combs.iter().enumerate() {
        let colour = palette[n % palette.len()];
        for harmonic in &comb.harmonics {
            let bin = spectrum
                .frequencies
                .iter()
                .enumerate()
                .min_by(|a, b| {
                    (a.1 - harmonic.appears_at).abs().total_cmp(&(b.1 - harmonic.appears_at).abs())
                })
                .map_or(0, |(k, _)| k);
            let top = place(area, &x, &y, (harmonic.appears_at, amplitude[bin].max(y.lo)));
            let stroke = Stroke::new(1.0, colour);
            if harmonic.folded {
                painter.extend(egui::Shape::dashed_line(
                    &[pos2(top.x, area.bottom()), top],
                    stroke,
                    4.0,
                    3.0,
                ));
            } else {
                painter.vline(top.x, top.y..=area.bottom(), stroke);
            }
            painter.circle_filled(top, 2.5, colour);
            let text = match (harmonic.n, harmonic.folded) {
                (1, false) => "f₀".to_owned(),
                (1, true) => "f₀ folded".to_owned(),
                (n, false) => format!("{n}f₀"),
                (n, true) => format!("{n}f₀ folded"),
            };
            painter.text(
                top + vec2(3.0, -3.0),
                Align2::LEFT_BOTTOM,
                text,
                FontId::proportional(10.0),
                colour,
            );
            drawn.stems.push((comb.label.clone(), harmonic.n, top.x, harmonic.folded));
        }
        painter.text(
            area.right_top() + vec2(-6.0, 6.0 + 13.0 * n as f32),
            Align2::RIGHT_TOP,
            format!("{} · f₀ = {:.4} Hz", comb.label, comb.fundamental),
            FontId::proportional(10.5),
            colour,
        );
    }
    label(painter, area.left_bottom() + vec2(0.0, 4.0), Align2::LEFT_TOP, "0 Hz");
    label(painter, area.right_bottom() + vec2(0.0, 4.0), Align2::RIGHT_TOP, format!("f_N = {nyquist} Hz"));
    label(painter, area.left_top() + vec2(-4.0, 0.0), Align2::RIGHT_TOP, format!("{:.0e}", y.hi));
    label(painter, area.left_bottom() + vec2(-4.0, 0.0), Align2::RIGHT_BOTTOM, format!("{:.0e}", y.lo));
    label(painter, area.left_center() + vec2(-4.0, 0.0), Align2::RIGHT_CENTER, "rad/√Hz");
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plots_map_data_to_pixels() {
        let area = Rect::from_min_max(pos2(100.0, 50.0), pos2(500.0, 250.0));
        let x = Axis { lo: 0.0, hi: 12.0, log: false };
        let y = Axis { lo: -2.0, hi: 2.0, log: false };
        // The corners and the middle, to the point.
        assert_eq!(place(area, &x, &y, (0.0, -2.0)), pos2(100.0, 250.0));
        assert_eq!(place(area, &x, &y, (12.0, 2.0)), pos2(500.0, 50.0));
        assert_eq!(place(area, &x, &y, (6.0, 0.0)), pos2(300.0, 150.0));
        assert_eq!(place(area, &x, &y, (3.0, 1.0)), pos2(200.0, 100.0));
        // A logarithmic axis puts each decade the same distance apart.
        let log = Axis { lo: 1e-6, hi: 1e-2, log: true };
        let at = |v: f64| place(area, &x, &log, (0.0, v)).y;
        assert!((at(1e-6) - 250.0).abs() < 1e-3 && (at(1e-2) - 50.0).abs() < 1e-3);
        assert!((at(1e-4) - 150.0).abs() < 1e-3 && (at(1e-5) - 200.0).abs() < 1e-3);
        // Control: on a linear axis the same values are nowhere near evenly spaced.
        let linear = Axis { lo: 1e-6, hi: 1e-2, log: false };
        assert!((place(area, &x, &linear, (0.0, 1e-4)).y - 150.0).abs() > 90.0);
    }

    #[test]
    fn axes_hold_their_data_and_series_are_thinned_without_losing_peaks() {
        let axis = Axis::linear([3.0, -1.0, 2.0]);
        assert!(axis.lo < -1.0 && axis.hi > 3.0 && (axis.hi - axis.lo - 4.0 * 14.0 / 12.0).abs() < 1e-12);
        assert!((0.0..=1.0).contains(&axis.fraction(3.0)) && (0.0..=1.0).contains(&axis.fraction(-1.0)));
        // A flat series, an empty one and one that is not a number still give an axis with a span.
        for values in [vec![5.0, 5.0], vec![], vec![f64::NAN], vec![0.0]] {
            let axis = Axis::linear(values);
            assert!(axis.hi > axis.lo && axis.lo.is_finite() && axis.hi.is_finite());
        }
        let log = Axis::log([3e-4, 2e-9, 0.0, 7e-13], 6.0);
        assert_eq!(log.hi, 1e-3);
        assert!(log.lo < 1e-9 && log.lo > 1e-10, "six decades down from the top, not twelve: {}", log.lo);
        // A long series is thinned, and its one tall bin survives.
        let n = 100_000;
        let x: Vec<f64> = (0..n).map(f64::from).collect();
        let mut y = vec![1.0; n as usize];
        y[54_321] = 1000.0;
        y[77_777] = -5.0;
        let thin = thinned(&x, &y);
        assert!(thin.len() <= MAX_POINTS + 2, "{}", thin.len());
        assert!(thin.contains(&(54_321.0, 1000.0)) && thin.contains(&(77_777.0, -5.0)));
        // Control: a short series is left as it is.
        assert_eq!(thinned(&x[..10], &y[..10]).len(), 10);
    }
}
