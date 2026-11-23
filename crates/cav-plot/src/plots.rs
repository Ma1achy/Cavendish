//! The plots a pane can show in place of the scene, and a figure can hold: every tower's signal
//! against time, the periodogram of a tower's signal with each oscillating body's comb drawn
//! over it, the cadence's spectral window, and the atoms' flights on absolute time.
//!
//! Each is built once, as a [`Drawing`] in points, from the engine, a [`Subject`] (which plot,
//! of what), a [`Style`] and a [`Measure`] of text. The mapping from data to the plot is
//! [`Axis`], plain arithmetic that the tests hold to; what was drawn is recorded beside the
//! drawing ([`Drawn`]) so that a test can ask where a point went without reading pixels.

use cav_engine::{ChannelName, Engine, PaneContent, Query, Reply};
use egui::{Pos2, Rect, pos2, vec2};
use serde_json::json;

use crate::drawing::{Anchor, Colour, Drawing, Face, Item, Measure};
use crate::style::{SHOT_NOISE_FLOORS, Style, ULDM_AMPLITUDE, ULDM_FREQUENCY};

/// One axis of a plot: a range of values laid along a range of points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axis {
    /// The value at the start of the axis.
    pub lo: f64,
    /// The value at its end.
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
/// axes, the axes, and the marks. Positions are points, in the coordinates the plot was painted
/// in (the window's, in the application).
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
    /// The pane, from 0.
    pub pane: usize,
    /// Which plot it is.
    pub content: PaneContent,
    /// The area inside the axes, points.
    pub area: Rect,
    /// The horizontal axis.
    pub x: Axis,
    /// The vertical axis.
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
    /// Whether this is the scene as read on the instrument being compared, drawn under the
    /// scene's own.
    pub compared: bool,
    /// For a flight diagram: each shot drawn, as the tower (from 0) and the x of its launch and
    /// of its readout.
    /// of its readout, and whether the tower takes it (a shot not taken is drawn dashed).
    pub flights: Vec<(usize, f32, f32, bool)>,
}

impl Drawn {
    /// The same record for a plot painted with its top left corner at `origin`.
    #[must_use]
    pub fn moved(mut self, origin: Pos2) -> Self {
        let by = origin.to_vec2();
        self.area = self.area.translate(by);
        self.playhead = self.playhead.map(|x| x + by.x);
        self.stems.iter_mut().for_each(|stem| stem.2 += by.x);
        self.labels.iter_mut().for_each(|(_, rect)| *rect = rect.translate(by));
        self.flights.iter_mut().for_each(|flight| (flight.1, flight.2) = (flight.1 + by.x, flight.2 + by.x));
        self
    }
}

/// What a plot is of: the words and the marks a figure's provenance records beside its data.
#[derive(Clone, Debug, PartialEq)]
pub struct Described {
    /// The horizontal axis: its name and its unit.
    pub x: (String, String),
    /// The vertical axis: its name and its unit (the data's; the marks may be written in a
    /// multiple of it, which `marks` then says).
    pub y: (String, String),
    /// Each series drawn: its name and its colour.
    pub series: Vec<(String, Colour)>,
    /// What else is drawn that is not a series, as JSON: comb stems, predicted lines, reference
    /// levels, the benchmark mark, each saying what it is.
    pub marks: serde_json::Value,
}

/// A plot, built: the picture, what it drew and what it is of.
#[derive(Clone, Debug, PartialEq)]
pub struct Plot {
    /// The picture, in points from its own top left corner.
    pub drawing: Drawing,
    /// Where things went, in the picture's own coordinates.
    pub drawn: Drawn,
    /// The words for a figure's provenance.
    pub described: Described,
}

/// Which plot to build, and of what.
#[derive(Clone, Debug, PartialEq)]
pub struct Subject {
    /// The pane it is for, from 0: recorded in [`Drawn`], nothing else.
    pub pane: usize,
    /// Which plot. `Scene` is not a plot.
    pub content: PaneContent,
    /// The tower, from 1: the one whose periodogram or window is drawn (the first if `None`);
    /// for a signal, the only tower drawn (every tower if `None`).
    pub tower: Option<usize>,
    /// The channel of a signal or a periodogram.
    pub channel: ChannelName,
    /// The stretch of the horizontal axis shown: seconds, or hertz; all of it if `None`.
    pub span: Option<[f64; 2]>,
    /// The scene as read on the instrument being compared, in place of its own.
    pub compared: bool,
}

impl Subject {
    /// What pane `pane` of `engine` shows, as the application draws it: the selected tower's
    /// periodogram, every tower's signal, the pane's own span.
    ///
    /// # Panics
    /// When the engine has no such pane.
    #[must_use]
    pub fn of_pane(engine: &Engine, pane: usize, compared: bool) -> Self {
        let content = engine.content(pane);
        let selected = match engine.selection() {
            Some(cav_engine::Target::Tower(n)) => Some(n),
            _ => None,
        };
        Self {
            pane,
            content,
            tower: if content == PaneContent::Signal { None } else { selected },
            channel: ChannelName::Gradiometer,
            span: engine.frame(pane).expect("the pane exists").span,
            compared,
        }
    }
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
    let stretch = stretch_within(x, lo, hi);
    (&x[stretch.clone()], &y[stretch])
}

/// Which of the rising `x` are inside `[lo, hi]`, with one either side.
fn stretch_within(x: &[f64], lo: f64, hi: f64) -> std::ops::Range<usize> {
    let first = x.partition_point(|v| *v < lo).saturating_sub(1);
    let last = (x.partition_point(|v| *v <= hi) + 1).min(x.len());
    if first >= last { 0..0 } else { first..last }
}

/// A tower's series as the lines it is drawn with: one for each run of shots taken, so that a
/// shot not taken (whose slot holds a zero that is no measurement) is on no line and the line
/// is broken there. `x`, `y` and `taken` are the shots shown, in order.
///
/// With no more than [`MAX_POINTS`] shots every taken shot is a point of its run. With more,
/// the series is thinned as [`thinned`] does it, to the smallest and the largest taken value of
/// each stretch, and the line is broken wherever a whole stretch holds no shot taken: a gap
/// narrower than a stretch (under a point wide) is not drawn. At most about [`MAX_POINTS`]
/// points in all, and no more runs than points.
#[must_use]
pub fn runs(x: &[f64], y: &[f64], taken: &[bool]) -> Vec<Vec<(f64, f64)>> {
    let n = x.len().min(y.len()).min(taken.len());
    let stretch = if n <= MAX_POINTS { 1 } else { (2 * n).div_ceil(MAX_POINTS) };
    let mut out = Vec::new();
    let mut run: Vec<(f64, f64)> = Vec::new();
    for start in (0..n).step_by(stretch) {
        let end = (start + stretch).min(n);
        let lowest = (start..end).filter(|&k| taken[k]).min_by(|&a, &b| y[a].total_cmp(&y[b]));
        let highest = (start..end).filter(|&k| taken[k]).max_by(|&a, &b| y[a].total_cmp(&y[b]));
        match lowest.zip(highest) {
            None if !run.is_empty() => out.push(std::mem::take(&mut run)),
            None => {}
            Some((lowest, highest)) if lowest == highest => run.push((x[lowest], y[lowest])),
            Some((lowest, highest)) => {
                for k in [lowest.min(highest), lowest.max(highest)] {
                    run.push((x[k], y[k]));
                }
            }
        }
    }
    if !run.is_empty() {
        out.push(run);
    }
    out
}

/// The most shots of a series that are each given a dot, and a hollow square at the foot of
/// the axis when not taken: beyond that the marks would run together.
pub const MAX_SHOTS_MARKED: usize = 200;

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

/// The most stems that are labelled in one periodogram, shared among the bodies: with one body
/// every harmonic says which it is; with many, each body's first few do and the legend names
/// the rest by colour.
pub const MAX_STEM_LABELS: u32 = 8;

const fn p(point: Pos2) -> [f32; 2] {
    [point.x, point.y]
}

/// The plot being drawn: where it is, and what it has written.
struct Sheet<'a> {
    style: &'a Style,
    measure: &'a dyn Measure,
    drawing: Drawing,
    rect: Rect,
    area: Rect,
    /// The layout's scale: 1 in the application.
    u: f32,
    labels: Vec<(String, Rect)>,
    /// How many of the labels are the title and the axes' own, which a stem's label may sit on
    /// (a mark's number is outside the axes; the title is above them).
    fixed: usize,
}

impl<'a> Sheet<'a> {
    /// A sheet of `size` points with a title at the top (the first of `titles` that fits) and
    /// `legend` lines under it, above the axes. `left` is what is written to the left of the
    /// axes: where the style does not fix the margin there, it is as wide as the widest needs.
    fn new(
        style: &'a Style,
        measure: &'a dyn Measure,
        size: [f32; 2],
        titles: &[String],
        legend: usize,
        left: &[String],
    ) -> Self {
        let u = style.unit();
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(size[0], size[1]));
        let fits = |text: &String| measure.size(text, Face::Sans, style.title_size)[0] <= size[0] - 16.0 * u;
        let title = titles.iter().find(|text| fits(text)).or(titles.last()).cloned().unwrap_or_default();
        let widest =
            left.iter().map(|text| measure.size(text, Face::Mono, style.tick_size)[0]).fold(0.0f32, f32::max);
        let margin = style.left.unwrap_or(widest + 11.0 * u);
        let top = (38.0 + 13.0 * legend as f32) * u;
        let area = Rect::from_min_max(pos2(margin, top), rect.max - vec2(16.0 * u, 34.0 * u));
        let mut drawing = Drawing::new(size, title.clone());
        drawing.items.push(Item::Rect {
            min: p(rect.min),
            max: p(rect.max),
            fill: Some(style.background),
            stroke: None,
        });
        drawing.items.push(Item::Rect {
            min: p(area.min),
            max: p(area.max),
            fill: None,
            stroke: Some((style.rule_width, style.rule)),
        });
        let mut sheet = Self { style, measure, drawing, rect, area, u, labels: Vec::new(), fixed: 0 };
        sheet.text(pos2(8.0 * u, 5.0 * u), Anchor::LEFT_TOP, title, Face::Sans, style.title_size, style.ink);
        sheet
    }

    /// The rectangle a text would take.
    fn took(&self, at: Pos2, anchor: Anchor, text: &str, face: Face, size: f32) -> Rect {
        let [w, h] = self.measure.size(text, face, size);
        let [x, y] = anchor.corner(p(at), [w, h]);
        Rect::from_min_size(pos2(x, y), vec2(w, h))
    }

    /// Writes text and remembers where it went.
    fn text(
        &mut self,
        at: Pos2,
        anchor: Anchor,
        text: impl Into<String>,
        face: Face,
        size: f32,
        colour: Colour,
    ) -> Rect {
        let text = text.into();
        let took = self.took(at, anchor, &text, face, size);
        self.drawing.items.push(Item::Text { at: p(at), anchor, text: text.clone(), face, size, colour });
        self.labels.push((text, took));
        took
    }

    fn small(&mut self, at: Pos2, anchor: Anchor, text: impl Into<String>) {
        self.text(at, anchor, text, Face::Mono, self.style.tick_size, self.style.weak);
    }

    /// The unit of the vertical axis, above its numbers.
    fn unit(&mut self, text: &str) {
        let at = self.area.left_top() + vec2(-5.0, -19.0) * self.u;
        self.small(at, Anchor::RIGHT_TOP, text);
    }

    /// The caption under the horizontal axis: the first of `forms` that fits the width of the
    /// axes, or the last if none does.
    fn caption(&mut self, forms: &[String]) {
        let fits =
            |text: &String| self.measure.size(text, Face::Mono, self.style.tick_size)[0] <= self.area.width();
        let text = forms.iter().find(|text| fits(text)).or(forms.last()).cloned().unwrap_or_default();
        self.small(pos2(self.area.center().x, self.area.bottom() + 18.0 * self.u), Anchor::CENTRE_TOP, text);
    }

    /// Writes a mark's label by its top, where it covers no label already written: to the right
    /// of the mark (to the left near the right edge), then higher, a line at a time. A label
    /// with nowhere to go is left out; its mark still stands. True when written.
    fn stem_label(&mut self, top: Pos2, text: &str, colour: Colour) -> bool {
        let (u, font) = (self.u, self.style.label_size);
        let [w, h] = self.measure.size(text, Face::Sans, font);
        let left = top.x + 3.0 * u + w > self.area.right();
        for lift in 0..4 {
            let bottom = (top.y - 3.0 * u - 11.0 * u * lift as f32).max(self.area.top() + h + 1.0);
            let min = pos2(if left { top.x - 3.0 * u - w } else { top.x + 3.0 * u }, bottom - h);
            let took = Rect::from_min_size(min, vec2(w, h));
            // With a little air round it: labels that touch read as one.
            let air = took.expand2(vec2(4.0 * u, u));
            if self.labels[self.fixed..].iter().all(|(_, other)| !other.intersects(air)) {
                self.text(min, Anchor::LEFT_TOP, text, Face::Sans, font, colour);
                return true;
            }
        }
        false
    }

    fn nothing(&mut self, why: &str) {
        let at = self.rect.center();
        self.text(at, Anchor::CENTRE, why, Face::Sans, self.style.title_size, self.style.weak);
    }

    /// A straight line of the rules' width.
    fn rule(&mut self, from: Pos2, to: Pos2, colour: Colour, dash: &[f32]) {
        self.drawing.items.push(Item::Line {
            points: vec![p(from), p(to)],
            width: self.style.rule_width,
            colour,
            dash: dash.to_vec(),
        });
    }

    /// The marks of the horizontal axis: a faint line up the plot and a number under it.
    fn x_marks(&mut self, x: &Axis, text: impl Fn(f64) -> String) {
        let area = self.area;
        for value in x.ticks(6) {
            let at = area.left() + area.width() * x.fraction(value) as f32;
            self.rule(pos2(at, area.top()), pos2(at, area.bottom()), self.style.rule, &[]);
            self.small(pos2(at, area.bottom() + 4.0 * self.u), Anchor::CENTRE_TOP, text(value));
        }
    }

    /// The marks of both axes: a faint line across the plot and a number outside it.
    fn axes(&mut self, x: &Axis, y: &Axis, x_text: impl Fn(f64) -> String, y_text: impl Fn(f64) -> String) {
        let area = self.area;
        self.x_marks(x, x_text);
        for value in y.ticks(6) {
            let at = area.bottom() - area.height() * y.fraction(value) as f32;
            self.rule(pos2(area.left(), at), pos2(area.right(), at), self.style.rule, &[]);
            self.small(pos2(area.left() - 5.0 * self.u, at), Anchor::RIGHT_CENTRE, y_text(value));
        }
    }

    /// From here to [`Self::unclip`], nothing is drawn further than `margin` outside the axes.
    fn clip(&mut self, margin: f32) {
        let area = self.area.expand(margin);
        self.drawing.items.push(Item::Clip { min: p(area.min), max: p(area.max) });
    }

    fn unclip(&mut self) {
        self.drawing.items.push(Item::Unclip);
    }

    fn dot(&mut self, centre: Pos2, colour: Colour) {
        self.drawing.items.push(Item::Circle { centre: p(centre), radius: self.style.dot, fill: colour });
    }

    /// A series' line, kept inside the axes.
    fn line(&mut self, points: &[Pos2], width: f32, colour: Colour, dash: &[f32]) {
        self.clip(1.0);
        self.drawing.items.push(Item::Line {
            points: points.iter().map(|point| p(*point)).collect(),
            width,
            colour,
            dash: dash.to_vec(),
        });
        self.unclip();
    }

    /// One entry of a key on legend line `line` (from 0), starting at `x`: a sample of the
    /// mark's line, then its name. Returns where the next entry starts, or `None` (and writes
    /// nothing) when the entry would run past the axes' right edge.
    fn key(&mut self, x: f32, line: usize, name: &str, colour: Colour, dash: &[f32]) -> Option<f32> {
        let (u, size) = (self.u, self.style.text_size);
        let top = (24.0 + 13.0 * line as f32) * u;
        let [w, h] = self.measure.size(name, Face::Sans, size);
        let end = x + 18.0 * u + w;
        if end > self.area.right() {
            return None;
        }
        let middle = top + h / 2.0;
        self.drawing.items.push(Item::Line {
            points: vec![[x, middle], [x + 14.0 * u, middle]],
            width: self.style.line,
            colour,
            dash: dash.to_vec(),
        });
        let ink = self.style.naming(colour);
        self.text(pos2(x + 18.0 * u, top), Anchor::LEFT_TOP, name, Face::Sans, size, ink);
        Some(end + 12.0 * u)
    }

    fn finish(self, mut drawn: Drawn, described: Described) -> Plot {
        drawn.area = self.area;
        drawn.labels = self.labels;
        Plot { drawing: self.drawing, drawn, described }
    }
}

/// Builds the plot `subject` names, `size` points wide and high: `None` when it names the
/// scene, which is not a plot.
#[must_use]
pub fn build(
    engine: &Engine,
    subject: &Subject,
    style: &Style,
    measure: &dyn Measure,
    size: [f32; 2],
) -> Option<Plot> {
    match subject.content {
        PaneContent::Scene => None,
        PaneContent::Signal => Some(signal(engine, subject, style, measure, size)),
        PaneContent::Periodogram => Some(periodogram(engine, subject, style, measure, size)),
        PaneContent::Flight => Some(flight(engine, subject, style, measure, size)),
        PaneContent::Cadence => Some(cadence(engine, subject, style, measure, size)),
    }
}

/// The plots pane `pane` of `engine` shows in a pane of `size` points, in the application's
/// style: none when the pane shows the scene. While the scene is being compared on another
/// instrument, a signal or a periodogram is drawn twice, the scene's own above and the
/// comparison's below, each on its own axes: the second value of each pair is how far down
/// the pane the plot starts, points.
#[must_use]
pub fn of_pane(
    engine: &Engine,
    pane: usize,
    style: &Style,
    measure: &dyn Measure,
    size: [f32; 2],
) -> Vec<(Plot, f32)> {
    let content = engine.content(pane);
    let twice =
        engine.comparison().is_some() && matches!(content, PaneContent::Signal | PaneContent::Periodogram);
    let one = |compared: bool, size: [f32; 2]| {
        build(engine, &Subject::of_pane(engine, pane, compared), style, measure, size)
    };
    if twice {
        let half = [size[0], size[1] * 0.5];
        one(false, half)
            .map(|plot| (plot, 0.0))
            .into_iter()
            .chain(one(true, half).map(|p| (p, half[1])))
            .collect()
    } else {
        one(false, size).map(|plot| (plot, 0.0)).into_iter().collect()
    }
}

/// Paints the plot pane `pane` of `engine` shows into `rect`, and says what was drawn: nothing
/// when the pane shows the scene. The application's panes and the pictures `cav render` and the
/// MCP server make are both this function.
#[must_use]
pub fn paint(engine: &Engine, painter: &egui::Painter, pane: usize, rect: Rect) -> Vec<Drawn> {
    let style = Style::screen();
    let measure = crate::paint::EguiMeasure(painter);
    of_pane(engine, pane, &style, &measure, [rect.width(), rect.height()])
        .into_iter()
        .map(|(plot, down)| {
            let origin = rect.min + vec2(0.0, down);
            crate::paint::paint(painter, origin, &plot.drawing);
            plot.drawn.moved(origin)
        })
        .collect()
}

/// What the comparison's plots say they are: `as AICE 100 m`.
fn compared_as(engine: &Engine) -> String {
    let name = engine.comparison().and_then(cav_sim::presets::name_of).unwrap_or("another instrument");
    format!(" · as {name}")
}

/// A number of seconds or hertz as an axis shows it: as short as says it.
fn plain(value: f64) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-0" { "0".to_owned() } else { text.to_owned() }
}

/// A decade as a logarithmic axis shows it: `1e-5`.
fn decade(value: f64) -> String {
    format!("1e{}", value.log10().round() as i32)
}

/// What a channel is called in a title.
const fn channel_name(channel: ChannelName) -> &'static str {
    match channel {
        ChannelName::Gradiometer => "ΔΦ",
        ChannelName::Lower => "δφ₁",
        ChannelName::Upper => "δφ₂",
    }
}

fn blank(subject: &Subject, x: Axis, y: Axis) -> Drawn {
    Drawn {
        pane: subject.pane,
        content: subject.content,
        area: Rect::NOTHING,
        x,
        y,
        points: 0,
        playhead: None,
        stems: Vec::new(),
        labels: Vec::new(),
        note: None,
        compared: subject.compared,
        flights: Vec::new(),
    }
}

fn described(x: (&str, &str), y: (&str, &str)) -> Described {
    Described {
        x: (x.0.to_owned(), x.1.to_owned()),
        y: (y.0.to_owned(), y.1.to_owned()),
        series: Vec::new(),
        marks: json!({}),
    }
}

/// The titles a plot may have, longest first; the house style never says where the signal
/// stands (a figure is made only from a refined one).
fn titles(style: &Style, with_state: String, without: String, short: String) -> Vec<String> {
    if style.house { vec![without, short] } else { vec![with_state, without, short] }
}

fn signal(engine: &Engine, subject: &Subject, style: &Style, measure: &dyn Measure, size: [f32; 2]) -> Plot {
    let spec = engine.spec();
    let (state, held) = if subject.compared { engine.compared_signal() } else { engine.signal() };
    let also = if subject.compared { compared_as(engine) } else { String::new() };
    let name = channel_name(subject.channel);
    let of = match subject.tower {
        None => "every tower".to_owned(),
        Some(n) => {
            spec.array.towers.get(n.wrapping_sub(1)).map_or_else(|| format!("tower {n}"), |t| t.label.clone())
        }
    };
    let titles = titles(
        style,
        format!("Signal · {name} of {of} · {}{also} · {}", spec.name, state.name()),
        format!("Signal · {name} of {of} · {}{also}", spec.name),
        format!("Signal · {name}"),
    );
    let [from, to] = subject.span.unwrap_or([0.0, spec.duration]);
    let x = Axis { lo: from, hi: to, log: false };
    let mut drawn = blank(subject, x, Axis { lo: -1.0, hi: 1.0, log: false });
    let mut described = described(("readout time", "s"), (name, "rad"));
    let empty = |why: &str, mut drawn: Drawn, described: Described| {
        let mut sheet = Sheet::new(style, measure, size, &titles, 0, &[]);
        sheet.nothing(why);
        drawn.note = Some(why.to_owned());
        sheet.finish(drawn, described)
    };
    let Some(signal) = held else {
        return empty("computing the signal", drawn, described);
    };
    let channel = subject.channel.channel();
    let towers = signal.towers.min(spec.array.towers.len());
    let which: Vec<usize> = match subject.tower {
        None => (0..towers).collect(),
        Some(n) if n >= 1 && n <= towers => vec![n - 1],
        Some(n) => return empty(&format!("there is no tower {n}"), drawn, described),
    };
    // Each tower against its own readout times, with which of its shots it took.
    let own: Vec<Vec<f64>> = which.iter().map(|t| signal.readouts(*t)).collect();
    let shown: Vec<(&[f64], &[f64], &[bool])> = which
        .iter()
        .zip(&own)
        .map(|(t, times)| {
            let stretch = stretch_within(times, from, to);
            (
                &times[stretch.clone()],
                &signal.channel(*t, channel)[stretch.clone()],
                &signal.taken(*t)[stretch],
            )
        })
        .collect();
    // The axis is fitted to the shots taken: the zero in a slot not taken is not a measurement.
    let measured = |(_, values, taken): &(&[f64], &[f64], &[bool])| {
        values
            .iter()
            .zip(taken.iter())
            .filter(|(_, taken)| **taken)
            .map(|(value, _)| *value)
            .collect::<Vec<_>>()
    };
    let y = Axis::linear(shown.iter().flat_map(measured));
    drawn.y = y;
    // The unit of the vertical axis is chosen once, from the larger end, so the marks agree.
    let largest = y.lo.abs().max(y.hi.abs());
    let (scale, unit) = match largest {
        s if s >= 1.0 => (1.0, "rad"),
        s if s >= 1e-3 => (1e3, "mrad"),
        s if s >= 1e-6 => (1e6, "µrad"),
        _ => (1e9, "nrad"),
    };
    let y_text = |value: f64| plain(value * scale);
    let mut left: Vec<String> = y.ticks(6).into_iter().map(y_text).collect();
    left.push(unit.to_owned());
    // A key of the towers' colours, on a line of its own, when there is more than one.
    let keyed = which.len() > 1;
    let mut sheet = Sheet::new(style, measure, size, &titles, usize::from(keyed), &left);
    sheet.axes(&x, &y, plain, y_text);
    let area = sheet.area;
    if keyed {
        let mut at = area.left();
        for (n, tower) in which.iter().enumerate() {
            let (colour, dash) = style.series(*tower);
            match sheet.key(at, 0, &spec.array.towers[*tower].label, colour, dash) {
                Some(next) => at = next,
                None => {
                    let more = format!("and {} more", which.len() - n);
                    let top = pos2(at, 24.0 * sheet.u);
                    if at + measure.size(&more, Face::Sans, style.text_size)[0] <= area.right() {
                        sheet.text(top, Anchor::LEFT_TOP, more, Face::Sans, style.text_size, style.weak);
                    }
                    break;
                }
            }
        }
    }
    sheet.fixed = sheet.labels.len();
    // The towers that swing furthest are drawn first, so that over all of a long run, where
    // each series is a band, no band lies wholly under another.
    let extents: Vec<f64> = shown
        .iter()
        .map(|series| {
            let (lo, hi) = measured(series)
                .into_iter()
                .filter(|v| v.is_finite())
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)));
            if hi >= lo { hi - lo } else { 0.0 }
        })
        .collect();
    let mut order: Vec<usize> = (0..which.len()).collect();
    order.sort_by(|a, b| extents[*b].total_cmp(&extents[*a]).then(a.cmp(b)));
    for n in order {
        let (tower, (times, values, taken)) = (which[n], shown[n]);
        let (colour, dash) = style.series(tower);
        // One line for each run of shots taken: a shot not taken is on none of them.
        let lines: Vec<Vec<Pos2>> = runs(times, values, taken)
            .iter()
            .map(|run| run.iter().map(|point| place(area, &x, &y, *point)).collect())
            .collect();
        let count: usize = lines.iter().map(Vec::len).sum();
        drawn.points += count;
        // Few shots are drawn as dots as well: six points joined by lines hide that they are
        // six. A shot not taken has no dot: a small hollow square at the foot of the axis, at
        // the time it would have been read out, says one was planned.
        let marked = times.len() <= MAX_SHOTS_MARKED;
        sheet.clip(4.0 * sheet.u);
        for line in &lines {
            // A run of one shot has no line to show it, so it has a dot whatever the count.
            if marked || line.len() == 1 {
                for point in line {
                    sheet.dot(*point, colour);
                }
            }
        }
        if marked {
            let half = style.dot * 1.2;
            for (time, _) in
                times.iter().zip(taken).filter(|(time, taken)| !**taken && (from..=to).contains(*time))
            {
                let foot = place(area, &x, &y, (*time, y.lo));
                sheet.drawing.items.push(Item::Rect {
                    min: [foot.x - half, foot.y - half],
                    max: [foot.x + half, foot.y + half],
                    fill: None,
                    stroke: Some((style.rule_width, colour)),
                });
            }
        }
        sheet.unclip();
        // More than a point to a point of width: the line is a band, and a thinner one shows more.
        let dense = count as f32 > area.width();
        let width = if dense { style.line * 0.6 } else { style.line };
        sheet.clip(1.0);
        for line in lines.iter().filter(|line| line.len() > 1) {
            sheet.drawing.items.push(Item::Line {
                points: line.iter().map(|point| p(*point)).collect(),
                width,
                colour,
                dash: if dense { Vec::new() } else { dash.to_vec() },
            });
        }
        sheet.unclip();
        described.series.push((spec.array.towers[tower].label.clone(), colour));
    }
    described.series.sort_by_key(|(label, _)| spec.array.towers.iter().position(|t| &t.label == label));
    let time = engine.time();
    if !style.house && (from..=to).contains(&time) {
        let at = place(area, &x, &y, (time, y.lo)).x;
        sheet.rule(pos2(at, area.top()), pos2(at, area.bottom()), style.playhead, &[]);
        drawn.playhead = Some(at);
    }
    sheet.unit(unit);
    // How many shots there are and how many each tower drawn took, over the whole signal.
    let shots = signal.times.len();
    let taken: Vec<usize> =
        which.iter().map(|t| signal.taken(*t).iter().filter(|taken| **taken).count()).collect();
    let (fewest, most) =
        (taken.iter().min().copied().unwrap_or(shots), taken.iter().max().copied().unwrap_or(shots));
    let count = match (fewest == shots, which.len() == 1 || fewest == most) {
        (true, _) => format!("{shots} shots"),
        (_, true) => format!("{most} of {shots} shots taken"),
        _ => format!("{shots} shots, {fewest} to {most} taken a tower"),
    };
    described.marks = json!({
        "vertical_axis_written_in": unit,
        "shots": shots,
        "shots_taken": taken,
        "not_taken": "a shot not taken is on no line (the line is broken there) and has no dot; with few \
                      shots shown a hollow square at the foot of the axis marks its time. Its slot in the \
                      data file holds 0 and its flag is 0: it is not a measurement",
    });
    let hint =
        if subject.span.is_some() { "scroll to zoom, double-click for all of it" } else { "scroll to zoom" };
    let mut forms =
        vec![format!("readout time, s · {count}"), "readout time, s".to_owned(), "time, s".to_owned()];
    if !style.house {
        forms.insert(0, format!("readout time, s · {count} · {hint}"));
    }
    sheet.caption(&forms);
    sheet.finish(drawn, described)
}

/// The cadence designer's pane: the spectral window `|W|` of one tower's shots (one
/// realisation, for the scene's seed) on a logarithmic scale, with a stem at each line the
/// closed forms predict, as tall as they predict it, and the predicted floor as a level.
fn cadence(engine: &Engine, subject: &Subject, style: &Style, measure: &dyn Measure, size: [f32; 2]) -> Plot {
    let spec = engine.spec();
    let tower = subject.tower.unwrap_or(1);
    let label_of = spec.array.towers.get(tower.wrapping_sub(1)).map_or("tower", |t| t.label.as_str());
    let long = format!("Cadence · spectral window |W| of {label_of}'s shots · {}", spec.name);
    let titles = vec![long, format!("Cadence · |W| of {label_of}'s shots"), "Cadence · |W|".to_owned()];
    // Four decades, from the peak down: a floor of 1e-4 in amplitude is 1e-8 in power.
    let y = Axis { lo: 1e-4, hi: 1.0, log: true };
    let left: Vec<String> = y.ticks(6).into_iter().map(decade).collect();
    let mut sheet = Sheet::new(style, measure, size, &titles, 2, &left);
    let mut drawn = blank(subject, Axis { lo: 0.0, hi: 1.0, log: false }, y);
    let mut described = described(("frequency offset from a tone", "Hz"), ("|W| relative to the tone", "1"));
    let predicted = match engine.query(&Query::Cadence) {
        Ok(Reply::Cadence(predicted)) => predicted,
        other => {
            let why = other.err().unwrap_or_else(|| "no prediction".to_owned());
            sheet.nothing(&why);
            drawn.note = Some(why);
            return sheet.finish(drawn, described);
        }
    };
    let whole = 2.5 / predicted.mean_cycle;
    let [from, to] = subject.span.unwrap_or([0.0, whole]);
    let x = Axis { lo: from, hi: to, log: false };
    let Ok(Reply::Window(window)) = engine.query(&Query::Window { tower, to: Some(to) }) else {
        sheet.nothing("no window");
        drawn.note = Some("no window".to_owned());
        return sheet.finish(drawn, described);
    };
    drawn.x = x;
    sheet.axes(&x, &y, plain, decade);
    let area = sheet.area;
    let (frequencies, shown) = within(&window.frequencies, &window.window, from, to);
    let series = thinned(frequencies, shown);
    drawn.points = series.len();
    let points: Vec<Pos2> = series.iter().map(|q| place(area, &x, &y, (q.0, q.1.max(y.lo)))).collect();
    sheet.line(&points, style.line, style.spectrum, &[]);
    described.series.push((format!("|W| of {label_of}'s shots, this seed"), style.spectrum));
    sheet.fixed = sheet.labels.len();
    // The floor the closed forms predict, as a level across the plot.
    if let Some(floor) = predicted.floor.filter(|floor| *floor > 0.0) {
        let level = place(area, &x, &y, (from, floor.sqrt().max(y.lo))).y;
        sheet.clip(3.0);
        let dash = [5.0 * sheet.u, 4.0 * sheet.u];
        sheet.rule(pos2(area.left(), level), pos2(area.right(), level), style.reference, &dash);
        sheet.unclip();
        // In a figure the window's own line runs through where the words would be: there the
        // legend names the level.
        if !style.house {
            let at = pos2(area.right() - 4.0 * sheet.u, level - 2.0 * sheet.u);
            sheet.small(at, Anchor::RIGHT_BOTTOM, "predicted floor");
        }
    }
    // The predicted lines: a stem to the predicted height, and a dot there.
    let colour = style.comb(0);
    let mut lines = Vec::new();
    for (k, line) in
        predicted.lines.iter().enumerate().filter(|(_, line)| (from..=to).contains(&line.frequency))
    {
        let top = place(area, &x, &y, (line.frequency, line.height.max(y.lo)));
        sheet.clip(3.0);
        sheet.rule(top, pos2(top.x, area.bottom()), colour, &[]);
        sheet.dot(top, colour);
        sheet.unclip();
        sheet.stem_label(top, &format!("{:.3}", line.height), style.naming(colour));
        drawn.stems.push((line.what.clone(), k as u32 + 1, top.x, false));
        lines.push(json!({ "what": line.what, "frequency_hz": line.frequency, "height": line.height }));
    }
    described.marks = json!({
        "predicted_lines": lines,
        "predicted_floor": predicted.floor.map(f64::sqrt),
        "mean_cycle_s": predicted.mean_cycle,
        "nyquist_hz": predicted.nyquist,
        "note": "stems and the dashed level are the closed forms' predictions (cav_sim::schedule), not data",
    });
    let fewest = predicted.kept.iter().min().copied().unwrap_or(0);
    let most = predicted.kept.iter().max().copied().unwrap_or(0);
    let u = sheet.u;
    sheet.text(
        pos2(area.left(), 24.0 * u),
        Anchor::LEFT_TOP,
        format!(
            "mean cycle {} s · Nyquist {} Hz · {} · {}",
            plain(predicted.mean_cycle),
            plain(predicted.nyquist),
            match (fewest == predicted.shots, fewest == most) {
                (true, _) => format!("{} shots", predicted.shots),
                (_, true) => format!("{} shots planned, {most} taken a tower", predicted.shots),
                _ => format!("{} shots planned, {fewest} to {most} taken a tower", predicted.shots),
            },
            predicted.noise_factor.map_or_else(
                || "a tower takes no shot".to_owned(),
                |factor| format!("smallest detectable tone × {factor:.3}")
            )
        ),
        Face::Sans,
        style.text_size,
        style.text,
    );
    sheet.text(
        pos2(area.left(), 37.0 * u),
        Anchor::LEFT_TOP,
        if style.house {
            "grey: this seed's window · blue: the lines the closed forms predict · dashed: the floor they predict"
        } else {
            "grey: this seed's window · blue: the lines the closed forms predict, at their predicted heights"
        },
        Face::Sans,
        style.text_size,
        style.naming(colour),
    );
    let mut forms = vec![
        "frequency offset from a tone, Hz · |W| relative to the tone".to_owned(),
        "frequency offset, Hz".to_owned(),
    ];
    if !style.house {
        forms.insert(
            0,
            "frequency offset from a tone, Hz · |W| relative to the tone · scroll to zoom".to_owned(),
        );
    }
    sheet.caption(&forms);
    sheet.finish(drawn, described)
}

/// The most towers a flight diagram gives a row to, and the most shots it draws in a row.
pub const MAX_FLIGHT_ROWS: usize = 8;
/// See [`MAX_FLIGHT_ROWS`].
pub const MAX_FLIGHT_SHOTS: usize = 64;

/// How high each of a tower's four arm paths is above the tower's base, metres, `tau` seconds
/// after launch: the lower interferometer's lower and upper arm, then the upper
/// interferometer's. The forward model's own arms; nothing is drawn that it does not fly.
#[must_use]
pub fn arm_heights(instrument: &cav_core::forward::Instrument, tau: f64) -> [f64; 4] {
    let [lower, upper] = instrument.arms();
    let (low, high) = (lower.rise(tau), upper.rise(tau));
    [low, high, instrument.ifo_separation + low, instrument.ifo_separation + high]
}

/// The shots of one tower a flight diagram draws over `[from, to]` seconds: how many shots
/// touch the span, and the launch time of each that is drawn (at most [`MAX_FLIGHT_SHOTS`]).
///
/// The shots are the scene's own (`Engine::shots`): the schedule's times and the tower's
/// offset, each with whether the tower takes it. Nothing here works a time out.
#[must_use]
pub fn flights_drawn(
    shots: &cav_sim::read::Shots,
    tower: usize,
    span: [f64; 2],
) -> (usize, Vec<cav_sim::read::Flown>) {
    let touching = shots.touching(tower, span);
    (touching.len(), touching.take(MAX_FLIGHT_SHOTS).map(|k| shots.flown(tower, k)).collect())
}

/// The span a flight diagram shows: its own, or, with none, two cycles either side of the
/// playhead.
#[must_use]
pub fn flight_span(engine: &Engine, span: Option<[f64; 2]>) -> [f64; 2] {
    let spec = engine.spec();
    let (cycle, flight_time) =
        (spec.array.instrument.cycle, spec.array.instrument.instrument().flight_time());
    let time = engine.time();
    span.unwrap_or([(time - 2.0 * cycle).max(-flight_time), time + 2.0 * cycle])
}

/// The atoms' flights on absolute time: a row for each tower, each shot a pair of arcs for each
/// interferometer from its launch to its readout, at the tower's own times.
fn flight(engine: &Engine, subject: &Subject, style: &Style, measure: &dyn Measure, size: [f32; 2]) -> Plot {
    let spec = engine.spec();
    let instrument = spec.array.instrument.instrument();
    let flight_time = instrument.flight_time();
    let long = format!("Flight · the atoms' arms, height against absolute time · {}", spec.name);
    let titles = vec![long, format!("Flight · the atoms' arms · {}", spec.name), "Flight".to_owned()];
    let towers = spec.array.towers.len().min(MAX_FLIGHT_ROWS);
    let left: Vec<String> = spec.array.towers.iter().take(towers).map(|t| t.label.clone()).collect();
    let mut sheet = Sheet::new(style, measure, size, &titles, 0, &left);
    // The scene's shots, as the engine keeps them; a scene whose signal is refused has none.
    let shots = match engine.shots() {
        Ok(shots) if shots.towers() >= towers => shots,
        other => {
            let why = other.err().unwrap_or("the shots are being made").to_owned();
            let mut drawn =
                blank(subject, Axis { lo: 0.0, hi: 1.0, log: false }, Axis { lo: 0.0, hi: 1.0, log: false });
            sheet.nothing(&why);
            drawn.note = Some(why);
            let described = described(("absolute time", "s"), ("height above each tower's base", "m"));
            return sheet.finish(drawn, described);
        }
    };
    let time = engine.time();
    let [from, to] = flight_span(engine, subject.span);
    let x = Axis { lo: from, hi: to, log: false };
    let top = spec.array.instrument.tower_height.max(arm_heights(&instrument, instrument.half_sequence)[3]);
    let y = Axis { lo: 0.0, hi: top, log: false };
    let mut drawn = blank(subject, x, y);
    let mut described = described(("absolute time", "s"), ("height above each tower's base", "m"));
    let area = sheet.area;
    sheet.x_marks(&x, plain);
    let row_height = area.height() / towers.max(1) as f32;
    let (mut left_out, mut not_taken) = (0usize, 0usize);
    for (n, tower) in spec.array.towers.iter().take(towers).enumerate() {
        let row = Rect::from_min_size(
            pos2(area.left(), area.top() + row_height * n as f32),
            vec2(area.width(), row_height),
        );
        let inner = row.shrink2(vec2(0.0, 3.0 * sheet.u));
        sheet.rule(row.left_bottom(), row.right_bottom(), style.rule, &[]);
        sheet.small(
            pos2(area.left() - 5.0 * sheet.u, row.center().y),
            Anchor::RIGHT_CENTRE,
            tower.label.clone(),
        );
        let (count, flown) = flights_drawn(shots, n, [from, to]);
        left_out += count - flown.len();
        let (colour, _) = style.series(n);
        described.series.push((tower.label.clone(), colour));
        sheet.drawing.items.push(Item::Clip { min: p(area.min), max: p(area.max) });
        for shot in flown {
            let launch = shot.launch;
            not_taken += usize::from(!shot.taken);
            let at = |seconds: f64| inner.left() + inner.width() * x.fraction(seconds) as f32;
            drawn.flights.push((n, at(launch), at(shot.readout), shot.taken));
            // Each of the four arm paths as a line through 33 points of the flight.
            const STEPS: usize = 32;
            for arm in 0..4 {
                let points: Vec<[f32; 2]> = (0..=STEPS)
                    .map(|step| {
                        let tau = flight_time * step as f64 / STEPS as f64;
                        let height = arm_heights(&instrument, tau)[arm];
                        [at(launch + tau), inner.bottom() - inner.height() * y.fraction(height) as f32]
                    })
                    .collect();
                drawn.points += points.len();
                // The upper interferometer's arms are fainter than the lower's. A shot the
                // tower does not take is where it was planned, dashed and fainter still: no
                // atoms flew it.
                let shade = if arm < 2 { colour } else { colour.mixed(style.background, 0.7) };
                let (shade, dash) = if shot.taken {
                    (shade, Vec::new())
                } else {
                    (shade.mixed(style.background, 0.55), vec![2.5 * sheet.u, 2.0 * sheet.u])
                };
                sheet.drawing.items.push(Item::Line {
                    points,
                    width: style.line * (1.2 / 1.3),
                    colour: shade,
                    dash,
                });
            }
            // Where the clouds are at the playhead, if this shot is in flight.
            if !style.house && shot.taken && (launch..=launch + flight_time).contains(&time) {
                for height in arm_heights(&instrument, time - launch) {
                    let point = pos2(at(time), inner.bottom() - inner.height() * y.fraction(height) as f32);
                    sheet.dot(point, style.ink);
                }
            }
        }
        sheet.unclip();
    }
    if !style.house && (from..=to).contains(&time) {
        let at = area.left() + area.width() * x.fraction(time) as f32;
        sheet.rule(pos2(at, area.top()), pos2(at, area.bottom()), style.playhead, &[]);
        drawn.playhead = Some(at);
    }
    let more = spec.array.towers.len() - towers;
    let mut said =
        format!("absolute time, s · each row a tower, 0 to {} m · 2T = {} s", plain(top), plain(flight_time));
    if not_taken > 0 {
        said.push_str(&format!(" · dashed: {not_taken} shot(s) not taken"));
    }
    if more > 0 || left_out > 0 {
        said.push_str(&format!(" · not drawn: {more} tower(s), {left_out} shot(s); zoom in"));
        drawn.note = Some(format!("{more} tower(s) and {left_out} shot(s) are not drawn"));
    }
    described.marks = json!({
        "row_height_m": top,
        "flight_time_s": flight_time,
        "arms": "each shot: the lower interferometer's two arms, and the upper's (fainter), as the forward model flies them",
        "not_taken": { "shots": not_taken, "drawn": "dashed and faint, where the schedule planned them" },
        "not_drawn": { "towers": more, "shots": left_out },
    });
    let short = if not_taken > 0 {
        format!("absolute time, s · dashed: {not_taken} shot(s) not taken")
    } else {
        "absolute time, s".to_owned()
    };
    sheet.caption(&[said, short, "absolute time, s".to_owned()]);
    sheet.finish(drawn, described)
}

/// The height the ULDM benchmark's tone has in a periodogram of `shots` shots whose Nyquist
/// frequency is `nyquist` hertz, rad/√Hz: `a·√T_obs/2` (the convention of
/// `cav_core::spectrum`), with `T_obs` the run's length, `shots` mean cycles.
#[must_use]
pub fn uldm_height(shots: usize, nyquist: f64) -> f64 {
    let run = shots as f64 / (2.0 * nyquist);
    ULDM_AMPLITUDE * run.sqrt() / 2.0
}

fn periodogram(
    engine: &Engine,
    subject: &Subject,
    style: &Style,
    measure: &dyn Measure,
    size: [f32; 2],
) -> Plot {
    let compared = subject.compared;
    let tower = subject.tower.unwrap_or(1);
    let query = Query::Periodogram { tower, channel: subject.channel, compared };
    let spec = engine.spec();
    let label_of = spec.array.towers.get(tower.wrapping_sub(1)).map_or("tower", |t| t.label.as_str());
    let state = if compared { engine.compared_signal().0.name() } else { engine.signal().0.name() };
    let also = if compared { compared_as(engine) } else { String::new() };
    let name = channel_name(subject.channel);
    let titles = titles(
        style,
        format!("Periodogram · √S of {name} at {label_of} · {}{also} · {state}", spec.name),
        format!("Periodogram · √S of {name} at {label_of} · {}{also}", spec.name),
        format!("Periodogram · {label_of}"),
    );
    let instrument = engine.comparison().filter(|_| compared).unwrap_or(&spec.array.instrument);
    // Half the mean shot rate: the schedule's, which for the plain schedule is the cycle's.
    let nyquist = 0.5 / spec.array.schedule.mean_cycle(instrument.cycle);
    let reply = engine.query(&query);
    let combs = match &reply {
        Ok(Reply::Periodogram(spectrum)) => spectrum.combs.len().min(style.combs.len()),
        _ => 0,
    };
    let [from, to] = subject.span.unwrap_or([0.0, nyquist]);
    let x = Axis { lo: from, hi: to, log: false };
    let mut drawn = blank(subject, x, Axis { lo: 1e-9, hi: 1.0, log: true });
    let mut described = described(("frequency", "Hz"), ("√S", "rad/√Hz"));
    let empty = |why: &str, mut drawn: Drawn, described: Described| {
        let mut sheet = Sheet::new(style, measure, size, &titles, combs + usize::from(style.house), &[]);
        sheet.nothing(why);
        drawn.note = Some(why.to_owned());
        sheet.finish(drawn, described)
    };
    let spectrum = match reply {
        Ok(Reply::Periodogram(spectrum)) if spectrum.power.len() >= 2 => spectrum,
        Err(why) => return empty(&why, drawn, described),
        _ => return empty("too few shots for a periodogram", drawn, described),
    };
    let amplitude: Vec<f64> = spectrum.power.iter().map(|power| power.sqrt()).collect();
    if !amplitude.iter().any(|a| *a > 0.0) {
        return empty("the signal is constant: nothing to resolve", drawn, described);
    }
    let (frequencies, shown) = within(&spectrum.frequencies, &amplitude, from, to);
    // The house style's reference marks are on the axis with the data: the two shot-noise
    // floors always, the ULDM benchmark when its frequency is in the span.
    let uldm = (style.house && (from..=to).contains(&ULDM_FREQUENCY))
        .then(|| uldm_height(spectrum.shots, spectrum.nyquist));
    let floors: &[f64] = if style.house { &SHOT_NOISE_FLOORS } else { &[] };
    let y = Axis::log(shown.iter().copied().chain(floors.iter().copied()).chain(uldm), 9.0);
    drawn.y = y;
    let mut left: Vec<String> = y.ticks(6).into_iter().map(decade).collect();
    left.push("rad/√Hz".to_owned());
    // The house style's key has a line more: the floors are named there, where no stem can
    // cross the words, and told apart by the axis they lie on.
    let mut sheet = Sheet::new(style, measure, size, &titles, combs + usize::from(style.house), &left);
    sheet.axes(&x, &y, plain, decade);
    let (area, u) = (sheet.area, sheet.u);
    for floor in floors.iter().filter(|floor| **floor >= y.lo) {
        let level = place(area, &x, &y, (from, *floor)).y;
        sheet.rule(pos2(area.left(), level), pos2(area.right(), level), style.reference, &[]);
    }
    if style.house {
        let named: Vec<String> = floors.iter().map(|floor| decade(*floor)).collect();
        let text = format!("shot-noise floors, {} rad/√Hz", named.join(" and "));
        sheet.key(area.left(), combs, &text, style.reference, &[]);
    }
    sheet.fixed = sheet.labels.len();
    let series = thinned(frequencies, shown);
    drawn.points = series.len();
    let points: Vec<Pos2> = series.iter().map(|q| place(area, &x, &y, (q.0, q.1.max(y.lo)))).collect();
    sheet.line(&points, style.line, style.spectrum, &[]);
    described.series.push((format!("√S of {name} at {label_of}"), style.spectrum));
    // The combs: a stem at each harmonic, at the height the spectrum has there. A harmonic above
    // Nyquist stands where it folds to, dashed, and says so.
    let labelled = (MAX_STEM_LABELS / spectrum.combs.len().max(1) as u32).max(1);
    let mut marks = Vec::new();
    for (n, comb) in spectrum.combs.iter().enumerate() {
        let colour = style.comb(n);
        let mut named: Vec<(Pos2, String)> = Vec::new();
        let mut stems = Vec::new();
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
            sheet.clip(3.0);
            let dash = if harmonic.folded { vec![4.0 * u, 3.0 * u] } else { Vec::new() };
            sheet.rule(pos2(top.x, area.bottom()), top, colour, &dash);
            sheet.dot(top, colour);
            sheet.unclip();
            drawn.stems.push((comb.label.clone(), harmonic.n, top.x, harmonic.folded));
            stems.push(json!({
                "n": harmonic.n,
                "frequency_hz": harmonic.frequency,
                "appears_at_hz": harmonic.appears_at,
                "folded": harmonic.folded,
                "height": amplitude[bin],
            }));
            // With many bodies only each one's first harmonics say which they are; the rest
            // are told by their colour, which the legend names.
            if harmonic.n > labelled {
                continue;
            }
            let text = match (harmonic.n, harmonic.folded) {
                (1, false) => "f₀".to_owned(),
                (1, true) => "f₀ folded".to_owned(),
                (n, false) => format!("{n}f₀"),
                (n, true) => format!("{n}f₀ folded"),
            };
            // Harmonics that stand in the same place (5f₀ at Nyquist's edge and 6f₀ folded back
            // onto 4f₀, say) share one label rather than write over each other.
            match named.iter_mut().find(|(at, _): &&mut (Pos2, String)| (at.x - top.x).abs() < 3.0 * u) {
                Some((_, names)) => {
                    names.push_str(" · ");
                    names.push_str(&text);
                }
                None => named.push((top, text)),
            }
        }
        for (top, text) in named {
            sheet.stem_label(top, &text, style.naming(colour));
        }
        marks.push(json!({ "body": comb.label, "fundamental_hz": comb.fundamental, "colour": colour.css(), "stems": stems }));
        // The legend has a line of its own above the axes, where no peak can be under it.
        if n < style.combs.len() {
            let text = format!("{} · f₀ = {:.4} Hz", comb.label, comb.fundamental);
            if style.house {
                sheet.key(area.left(), n, &text, colour, &[]);
            } else {
                let at = pos2(area.left(), (24.0 + 13.0 * n as f32) * u);
                sheet.text(at, Anchor::LEFT_TOP, text, Face::Sans, style.text_size, colour);
            }
        }
    }
    let mut reference = json!({});
    if style.house {
        reference = json!({
            "shot_noise_floors_rad_per_sqrt_hz": SHOT_NOISE_FLOORS,
            "shot_noise_note": "the plan's two reference levels (baseline and stretch), drawn as grey lines: not computed from this scene",
        });
    }
    if let Some(height) = uldm {
        let centre = place(area, &x, &y, (ULDM_FREQUENCY, height));
        sheet.clip(3.0);
        sheet.drawing.items.push(Item::Diamond {
            centre: p(centre),
            radius: style.dot * 1.6,
            fill: style.ink,
            stroke: (style.rule_width, style.ink),
        });
        sheet.unclip();
        sheet.stem_label(centre, "ULDM benchmark", style.text);
        reference["uldm_benchmark"] = json!({
            "frequency_hz": ULDM_FREQUENCY,
            "amplitude_rad": ULDM_AMPLITUDE,
            "height_rad_per_sqrt_hz": height,
            "run_length_s": spectrum.shots as f64 / (2.0 * spectrum.nyquist),
            "note": "a reference mark from the plan, not a simulated signal: a scalar of mass 2e-16 eV (f = m c²/h) at 1e-2 µrad, drawn at the height a tone of that amplitude has in this periodogram, a·√T_obs/2, over this run's length",
        });
    }
    described.marks = json!({
        "combs": marks,
        "reference": reference,
        "method": spectrum.method,
        "shots": spectrum.shots,
        "nyquist_hz": spectrum.nyquist,
        "folded": "a dashed stem is a harmonic above Nyquist, standing where it folds to",
    });
    sheet.unit("rad/√Hz");
    let mut forms =
        vec![format!("frequency, Hz · Nyquist {} Hz", plain(nyquist)), "frequency, Hz".to_owned()];
    if style.house {
        forms.insert(0, format!("frequency, Hz · Nyquist {} Hz · dashed stems are folded", plain(nyquist)));
    } else {
        forms.insert(0, format!("frequency, Hz · Nyquist {} Hz · scroll to zoom", plain(nyquist)));
    }
    sheet.caption(&forms);
    sheet.finish(drawn, described)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flight_arms_are_the_forward_models() {
        use cav_core::forward::Instrument;
        for instrument in [Instrument::aion10(), Instrument::aice()] {
            let [lower, upper] = instrument.arms();
            for step in 0..=20 {
                let tau = instrument.flight_time() * f64::from(step) / 20.0;
                let [a, b, c, d] = arm_heights(&instrument, tau);
                assert_eq!([a, b], [lower.rise(tau), upper.rise(tau)]);
                assert_eq!([c, d], [instrument.ifo_separation + a, instrument.ifo_separation + b]);
            }
            // The arms leave together, are furthest apart at the π pulse (by the kick times T)
            // and never go below the launch point.
            let [a, b, _, _] = arm_heights(&instrument, 0.0);
            assert_eq!([a, b], [0.0, 0.0]);
            let [a, b, _, _] = arm_heights(&instrument, instrument.half_sequence);
            assert!(
                (b - a - instrument.kick_velocity() * instrument.half_sequence).abs() < 1e-9,
                "{}",
                b - a
            );
            assert!(
                (0..=200)
                    .all(|step| arm_heights(&instrument, instrument.flight_time() * f64::from(step) / 200.0)
                        [0]
                        >= 0.0)
            );
        }
        // Control: the two instruments fly differently, so the diagram is the instrument's.
        let half = 0.5;
        assert!(
            arm_heights(&Instrument::aice(), half)[1] > 3.0 * arm_heights(&Instrument::aion10(), half)[1]
        );
    }

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
