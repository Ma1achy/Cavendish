//! The plots a pane can show in place of the scene: every tower's signal against time, and the
//! periodogram of a tower's signal with each oscillating body's comb drawn over it.
//!
//! Drawn with egui's painter: lines and text, no plotting crate. The mapping from data to the
//! pane is [`Axis`], plain arithmetic that the tests hold to; what was drawn is recorded in the
//! shell's layout ([`Drawn`]) so that a test can ask where a point went without reading pixels.

use cav_engine::{ChannelName, Engine, PaneContent, Query, Reply};
use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, pos2, vec2};

use crate::shell::Shell;
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

    /// A logarithmic axis whose ends are whole decades: from the decade above the largest value
    /// down to the decade below the smallest positive one, but no more than `decades` in all.
    #[must_use]
    pub fn log(values: impl IntoIterator<Item = f64>, decades: f64) -> Self {
        let (lo, hi) = values
            .into_iter()
            .filter(|v| v.is_finite() && *v > 0.0)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)));
        if lo > hi {
            return Self { lo: 1e-9, hi: 1.0, log: true };
        }
        let top = hi.log10().ceil();
        let bottom = lo.log10().floor().max(top - decades).min(top - 1.0);
        Self { lo: 10f64.powf(bottom), hi: 10f64.powf(top), log: true }
    }

    /// Where the marks of the axis go: at most about `most` of them. On a linear axis they are
    /// multiples of 1, 2 or 5 times a power of ten; on a logarithmic one, decades (every second
    /// or third when there are many).
    #[must_use]
    pub fn ticks(&self, most: usize) -> Vec<f64> {
        let most = most.max(2) as f64;
        if self.log {
            let (bottom, top) = (self.lo.log10().round() as i32, self.hi.log10().round() as i32);
            let every = (f64::from(top - bottom) / most).ceil().max(1.0) as i32;
            return (bottom..=top).filter(|d| (top - d) % every == 0).map(|d| 10f64.powi(d)).collect();
        }
        let span = self.hi - self.lo;
        if !(span.is_finite() && span > 0.0) {
            return Vec::new();
        }
        let rough = span / most;
        let decade = 10f64.powf(rough.log10().floor());
        let step =
            [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * decade).find(|s| *s >= rough).unwrap_or(decade);
        let first = (self.lo / step).ceil() as i64;
        let last = (self.hi / step).floor() as i64;
        // A count, never a float added to itself; and never more than a few dozen.
        (first..=last.min(first + 40)).map(|k| k as f64 * step).collect()
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
    /// The playhead's x, for a signal plot, when it is inside the span shown.
    pub playhead: Option<f32>,
    /// Each comb stem: the body's label, the harmonic, where it stands, and whether it is folded.
    pub stems: Vec<(String, u32, f32, bool)>,
    /// Every piece of text the plot wrote, with the rectangle it took.
    pub labels: Vec<(String, Rect)>,
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

/// The part of a series inside `[lo, hi]` on its x, with one point either side so that the line
/// runs to the edges.
fn within<'a>(x: &'a [f64], y: &'a [f64], lo: f64, hi: f64) -> (&'a [f64], &'a [f64]) {
    let first = x.partition_point(|v| *v < lo).saturating_sub(1);
    let last = (x.partition_point(|v| *v <= hi) + 1).min(x.len());
    if first >= last { (&[], &[]) } else { (&x[first..last], &y[first..last]) }
}

/// The span a wheel movement of `scroll` points leaves, zooming about `about` on an axis whose
/// whole extent is `[0, whole]`: the point under the pointer stays where it is, the span never
/// leaves the axis, and a span that has grown back to all of it is `None`.
#[must_use]
pub fn zoomed([lo, hi]: [f64; 2], about: f64, scroll: f32, whole: f64) -> Option<[f64; 2]> {
    let factor = f64::from(-scroll * 0.002).exp();
    let (lo, hi) = ((about - (about - lo) * factor).max(0.0), (about + (hi - about) * factor).min(whole));
    // No narrower than a millionth of the axis: past that the numbers run out before the picture does.
    if hi - lo < 1e-6 * whole {
        return Some([(about - 5e-7 * whole).max(0.0), (about + 5e-7 * whole).min(whole)]);
    }
    (lo > 0.0 || hi < whole).then_some([lo, hi])
}

/// The colours of the combs, in the order of the bodies: none of them a tower's colour, and none
/// the grey of the spectrum itself.
pub const COMB_COLOURS: [Color32; 4] = [
    Color32::from_rgb(0x5a, 0xb4, 0xff),
    Color32::from_rgb(0xd0, 0x8a, 0xe0),
    Color32::from_rgb(0x7f, 0xe0, 0xb0),
    Color32::from_rgb(0xf0, 0xa0, 0xa0),
];

/// The colour of the spectrum's own line: the grey of the paper's periodograms.
pub const SPECTRUM: Color32 = Color32::from_rgb(0x9a, 0xa5, 0xab);

/// The plot being drawn: where it is, and what it has written.
struct Sheet<'a> {
    painter: &'a egui::Painter,
    rect: Rect,
    area: Rect,
    labels: Vec<(String, Rect)>,
}

impl<'a> Sheet<'a> {
    /// A sheet with its title at the top and `legend` lines under it, above the axes.
    fn new(painter: &'a egui::Painter, rect: Rect, title: &str, legend: usize) -> Self {
        painter.rect_filled(rect, 0.0, theme::BACKGROUND);
        let top = 26.0 + 13.0 * legend as f32;
        let area = Rect::from_min_max(rect.min + vec2(86.0, top), rect.max - vec2(16.0, 34.0));
        painter.rect_stroke(area, 0.0, Stroke::new(1.0, theme::RULE), egui::StrokeKind::Outside);
        let mut sheet = Self { painter, rect, area, labels: Vec::new() };
        sheet.text(
            rect.left_top() + vec2(8.0, 5.0),
            Align2::LEFT_TOP,
            title,
            FontId::proportional(12.0),
            theme::STRONG,
        );
        sheet
    }

    /// Writes text and remembers where it went.
    fn text(&mut self, at: Pos2, anchor: Align2, text: impl Into<String>, font: FontId, colour: Color32) {
        let text = text.into();
        let took = self.painter.text(at, anchor, &text, font, colour);
        self.labels.push((text, took));
    }

    fn small(&mut self, at: Pos2, anchor: Align2, text: impl Into<String>) {
        self.text(at, anchor, text, FontId::monospace(10.5), theme::WEAK);
    }

    fn nothing(&mut self, why: &str) {
        self.text(self.rect.center(), Align2::CENTER_CENTER, why, FontId::proportional(12.0), theme::WEAK);
    }

    /// The marks of both axes: a faint line across the plot and a number outside it.
    fn axes(&mut self, x: &Axis, y: &Axis, x_text: impl Fn(f64) -> String, y_text: impl Fn(f64) -> String) {
        let area = self.area;
        for value in x.ticks(6) {
            let at = area.left() + area.width() * x.fraction(value) as f32;
            self.painter.vline(at, area.y_range(), Stroke::new(1.0, theme::RULE));
            self.small(pos2(at, area.bottom() + 4.0), Align2::CENTER_TOP, x_text(value));
        }
        for value in y.ticks(6) {
            let at = area.bottom() - area.height() * y.fraction(value) as f32;
            self.painter.hline(area.x_range(), at, Stroke::new(1.0, theme::RULE));
            self.small(pos2(area.left() - 5.0, at), Align2::RIGHT_CENTER, y_text(value));
        }
    }

    fn line(&self, points: Vec<Pos2>, colour: Color32) {
        let clip = self.painter.with_clip_rect(self.area.expand(1.0));
        clip.add(egui::Shape::line(points, Stroke::new(1.3, colour)));
    }
}

/// Draws what pane `pane` shows, if it is a plot, into `rect`, and records what was drawn.
pub fn draw(shell: &mut Shell, ui: &egui::Ui, pane: usize, rect: Rect) {
    if let Some(drawn) = paint(&shell.engine, &ui.painter_at(rect), pane, rect) {
        shell.layout.plots.push(drawn);
    }
}

/// Paints the plot pane `pane` of `engine` shows into `rect`, and says what was drawn; `None`
/// when the pane shows the scene. The application's panes and the pictures `cav render` and the
/// MCP server make are both this function.
#[must_use]
pub fn paint(engine: &Engine, painter: &egui::Painter, pane: usize, rect: Rect) -> Option<Drawn> {
    match engine.content(pane) {
        PaneContent::Scene => None,
        PaneContent::Signal => Some(signal(engine, painter, pane, rect)),
        PaneContent::Periodogram => Some(periodogram(engine, painter, pane, rect)),
    }
}

/// A number of seconds or hertz as an axis shows it: as short as says it.
fn plain(value: f64) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-0" { "0".to_owned() } else { text.to_owned() }
}

fn signal(engine: &Engine, painter: &egui::Painter, pane: usize, rect: Rect) -> Drawn {
    let spec = engine.spec();
    let frame = engine.frame(pane).expect("the pane exists");
    let state = engine.signal().0.name();
    let title = format!("Signal · ΔΦ of every tower · {} · {state}", spec.name);
    let mut sheet = Sheet::new(painter, rect, &title, 0);
    let [from, to] = frame.span.unwrap_or([0.0, spec.duration]);
    let x = Axis { lo: from, hi: to, log: false };
    let mut drawn = Drawn {
        pane,
        content: PaneContent::Signal,
        area: sheet.area,
        x,
        y: Axis { lo: -1.0, hi: 1.0, log: false },
        points: 0,
        playhead: None,
        stems: Vec::new(),
        labels: Vec::new(),
        note: None,
    };
    let Some(signal) = engine.signal().1 else {
        let why = "computing the signal";
        sheet.nothing(why);
        drawn.note = Some(why.into());
        drawn.labels = sheet.labels;
        return drawn;
    };
    let channel = ChannelName::Gradiometer.channel();
    let towers = signal.towers.min(spec.array.towers.len());
    let shown: Vec<(&[f64], &[f64])> =
        (0..towers).map(|t| within(&signal.times, signal.channel(t, channel), from, to)).collect();
    let y = Axis::linear(shown.iter().flat_map(|(_, values)| values.iter().copied()));
    drawn.y = y;
    // The unit of the vertical axis is chosen once, from the larger end, so the marks agree.
    let size = y.lo.abs().max(y.hi.abs());
    let (scale, unit) = match size {
        s if s >= 1.0 => (1.0, "rad"),
        s if s >= 1e-3 => (1e3, "mrad"),
        s if s >= 1e-6 => (1e6, "µrad"),
        _ => (1e9, "nrad"),
    };
    sheet.axes(&x, &y, plain, |value| plain(value * scale));
    let area = sheet.area;
    for (tower, (times, values)) in shown.iter().enumerate() {
        let series = thinned(times, values);
        drawn.points += series.len();
        let points: Vec<Pos2> = series.iter().map(|p| place(area, &x, &y, *p)).collect();
        // Few shots are drawn as dots as well: six points joined by lines hide that they are six.
        if points.len() <= 200 {
            let clip = painter.with_clip_rect(area.expand(3.0));
            for point in &points {
                clip.circle_filled(*point, 2.5, theme::tower(tower));
            }
        }
        sheet.line(points, theme::tower(tower));
    }
    let time = engine.time();
    if (from..=to).contains(&time) {
        let at = place(area, &x, &y, (time, y.lo)).x;
        painter.vline(at, area.y_range(), Stroke::new(1.0, theme::LINK));
        drawn.playhead = Some(at);
    }
    sheet.small(pos2(area.left() - 5.0, area.bottom() + 18.0), Align2::RIGHT_TOP, unit);
    let whole = if frame.span.is_some() {
        " · scroll to zoom, double-click for all of it"
    } else {
        " · scroll to zoom"
    };
    sheet.small(
        pos2(area.center().x, area.bottom() + 18.0),
        Align2::CENTER_TOP,
        format!("readout time, s · {} shots{whole}", signal.times.len()),
    );
    drawn.labels = sheet.labels;
    drawn
}

fn periodogram(engine: &Engine, painter: &egui::Painter, pane: usize, rect: Rect) -> Drawn {
    let tower = match engine.selection() {
        Some(cav_engine::Target::Tower(n)) => n,
        _ => 1,
    };
    let query = Query::Periodogram { tower, channel: ChannelName::Gradiometer };
    let spec = engine.spec();
    let frame = engine.frame(pane).expect("the pane exists");
    let label_of = spec.array.towers.get(tower - 1).map_or("tower", |t| t.label.as_str());
    let state = engine.signal().0.name();
    let title = format!("Periodogram · √S of ΔΦ at {label_of} · {} · {state}", spec.name);
    let nyquist = 0.5 / spec.array.instrument.cycle;
    let reply = engine.query(&query);
    let combs = match &reply {
        Ok(Reply::Periodogram(spectrum)) => spectrum.combs.len().min(COMB_COLOURS.len()),
        _ => 0,
    };
    let mut sheet = Sheet::new(painter, rect, &title, combs);
    let [from, to] = frame.span.unwrap_or([0.0, nyquist]);
    let x = Axis { lo: from, hi: to, log: false };
    let mut drawn = Drawn {
        pane,
        content: PaneContent::Periodogram,
        area: sheet.area,
        x,
        y: Axis { lo: 1e-9, hi: 1.0, log: true },
        points: 0,
        playhead: None,
        stems: Vec::new(),
        labels: Vec::new(),
        note: None,
    };
    let spectrum = match reply {
        Ok(Reply::Periodogram(spectrum)) if spectrum.power.len() >= 2 => spectrum,
        other => {
            let why = match other {
                Err(why) => why,
                _ => "too few shots for a periodogram".to_owned(),
            };
            sheet.nothing(&why);
            drawn.note = Some(why);
            drawn.labels = sheet.labels;
            return drawn;
        }
    };
    let amplitude: Vec<f64> = spectrum.power.iter().map(|p| p.sqrt()).collect();
    if !amplitude.iter().any(|a| *a > 0.0) {
        let why = "the signal is constant: nothing to resolve";
        sheet.nothing(why);
        drawn.note = Some(why.into());
        drawn.labels = sheet.labels;
        return drawn;
    }
    let (frequencies, shown) = within(&spectrum.frequencies, &amplitude, from, to);
    let y = Axis::log(shown.iter().copied(), 9.0);
    drawn.y = y;
    sheet.axes(&x, &y, plain, |value| format!("1e{}", value.log10().round() as i32));
    let area = sheet.area;
    let series = thinned(frequencies, shown);
    drawn.points = series.len();
    let points: Vec<Pos2> = series.iter().map(|p| place(area, &x, &y, (p.0, p.1.max(y.lo)))).collect();
    sheet.line(points, SPECTRUM);
    // The combs: a stem at each harmonic, at the height the spectrum has there. A harmonic above
    // Nyquist stands where it folds to, dashed, and says so.
    let clip = painter.with_clip_rect(area.expand(3.0));
    for (n, comb) in spectrum.combs.iter().enumerate() {
        let colour = COMB_COLOURS[n % COMB_COLOURS.len()];
        let mut named: Vec<(Pos2, String)> = Vec::new();
        for harmonic in comb.harmonics.iter().filter(|h| (from..=to).contains(&h.appears_at)) {
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
                clip.extend(egui::Shape::dashed_line(&[pos2(top.x, area.bottom()), top], stroke, 4.0, 3.0));
            } else {
                clip.vline(top.x, top.y..=area.bottom(), stroke);
            }
            clip.circle_filled(top, 2.5, colour);
            let text = match (harmonic.n, harmonic.folded) {
                (1, false) => "f₀".to_owned(),
                (1, true) => "f₀ folded".to_owned(),
                (n, false) => format!("{n}f₀"),
                (n, true) => format!("{n}f₀ folded"),
            };
            // Harmonics that stand in the same place (5f₀ at Nyquist's edge and 6f₀ folded back
            // onto 4f₀, say) share one label rather than write over each other.
            match named.iter_mut().find(|(at, _): &&mut (Pos2, String)| (at.x - top.x).abs() < 3.0) {
                Some((_, names)) => {
                    names.push_str(" · ");
                    names.push_str(&text);
                }
                None => named.push((top, text)),
            }
            drawn.stems.push((comb.label.clone(), harmonic.n, top.x, harmonic.folded));
        }
        for (top, text) in named {
            // To the right of its stem, or to the left when the right edge is too near for the
            // words; never above the top of the plot.
            let width = 5.5 * text.chars().count() as f32 + 8.0;
            let (anchor, offset) = if top.x + width > area.right() {
                (Align2::RIGHT_BOTTOM, vec2(-3.0, -3.0))
            } else {
                (Align2::LEFT_BOTTOM, vec2(3.0, -3.0))
            };
            let at = pos2(top.x, top.y.max(area.top() + 13.0)) + offset;
            sheet.text(at, anchor, text, FontId::proportional(10.0), colour);
        }
        // The legend has a line of its own above the axes, where no peak can be under it.
        if n < COMB_COLOURS.len() {
            sheet.text(
                pos2(area.left(), rect.top() + 24.0 + 13.0 * n as f32),
                Align2::LEFT_TOP,
                format!("{} · f₀ = {:.4} Hz", comb.label, comb.fundamental),
                FontId::proportional(10.5),
                colour,
            );
        }
    }
    sheet.small(pos2(area.left() - 5.0, area.bottom() + 18.0), Align2::RIGHT_TOP, "rad/√Hz");
    sheet.small(
        pos2(area.center().x, area.bottom() + 18.0),
        Align2::CENTER_TOP,
        format!("frequency, Hz · Nyquist {} Hz · scroll to zoom", plain(nyquist)),
    );
    drawn.labels = sheet.labels;
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
        // A logarithmic axis ends on whole decades, and is no longer than it is asked to be.
        let log = Axis::log([3e-4, 2e-9, 0.0, 7e-13], 6.0);
        assert!((log.hi / 1e-3 - 1.0).abs() < 1e-12 && (log.lo / 1e-9 - 1.0).abs() < 1e-12, "{log:?}");
        let short = Axis::log([3e-4, 2e-5], 9.0);
        assert!((short.hi / 1e-3 - 1.0).abs() < 1e-12 && (short.lo / 1e-5 - 1.0).abs() < 1e-12, "{short:?}");
        // Its marks are the decades between; with many, every second or third, the top among them.
        let marks = |axis: &Axis| axis.ticks(6).iter().map(|v| v.log10().round() as i32).collect::<Vec<_>>();
        assert_eq!(marks(&log), [-9, -8, -7, -6, -5, -4, -3]);
        assert_eq!(marks(&Axis { lo: 1e-12, hi: 1e-3, log: true }), [-11, -9, -7, -5, -3]);
        // A linear axis is marked at multiples of 1, 2 or 5 times a power of ten, inside its ends.
        let close =
            |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
        assert!(close(&Axis { lo: 0.0, hi: 0.25, log: false }.ticks(6), &[0.0, 0.05, 0.1, 0.15, 0.2, 0.25]));
        assert!(close(
            &Axis { lo: 0.0, hi: 2000.0, log: false }.ticks(6),
            &[0.0, 500.0, 1000.0, 1500.0, 2000.0]
        ));
        assert!(close(&Axis { lo: -0.0113, hi: 0.0042, log: false }.ticks(6), &[-0.01, -0.005, 0.0]));
        // Zooming keeps the point under the pointer where it is and stays on the axis.
        let near = zoomed([0.0, 2000.0], 500.0, 350.0, 2000.0).unwrap();
        assert!(((500.0 - near[0]) / (near[1] - 500.0) - 500.0 / 1500.0).abs() < 1e-12, "{near:?}");
        assert!(near[0] > 0.0 && near[1] < 2000.0 && near[1] - near[0] < 2000.0);
        assert_eq!(zoomed(near, 500.0, -5000.0, 2000.0), None, "zoomed all the way out is all of it");
        let deep = (0..200).fold([0.0, 2000.0], |span, _| zoomed(span, 777.0, 900.0, 2000.0).unwrap());
        assert!(
            deep[1] - deep[0] >= 1e-6 * 2000.0 * 0.99 && deep[0] <= 777.0 && deep[1] >= 777.0,
            "{deep:?}"
        );
        // Control: an axis with no span has no marks, and one a million marks wide has a few.
        assert!(Axis { lo: 1.0, hi: 1.0, log: false }.ticks(6).is_empty());
        assert!(Axis { lo: 0.0, hi: 1e6, log: false }.ticks(6).len() <= 7);
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
