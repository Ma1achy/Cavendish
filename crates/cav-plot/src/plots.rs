//! The plots a pane can show in place of the scene, and a figure can hold: every tower's signal
//! against time, the periodogram of a tower's signal with each oscillating body's comb drawn
//! over it, the cadence's spectral window, and the atoms' flights on absolute time.
//!
//! Each is built once, as a [`Drawing`] in points, from the engine, a [`Subject`] (which plot,
//! of what), a [`Style`] and a [`Measure`] of text. The mapping from data to the plot is
//! [`Axis`], plain arithmetic that the tests hold to; what was drawn is recorded beside the
//! drawing ([`Drawn`]) so that a test can ask where a point went without reading pixels.

use cav_engine::{ChannelName, Engine, PaneContent, Query, Reply};
use egui::{Pos2, Rect, Vec2, pos2, vec2};
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

    /// The short marks between the labelled ones (`ticks`, from [`Self::ticks`]) of a
    /// logarithmic axis on which a decade takes `decade` points, at the layout's scale `u` (1 in
    /// the application; [`Style::unit`]). At 2 to 9 times each power of ten when a decade is at
    /// least 16·u high (and at every decade not labelled); at the decades that are not labelled when only every second or third
    /// is, and a decade is at least 3·u high; otherwise none. None on a linear axis, which is
    /// marked often enough by its numbers.
    #[must_use]
    pub fn minor_ticks(&self, ticks: &[f64], decade: f32, u: f32) -> Vec<f64> {
        if !self.log || !(decade.is_finite() && decade > 0.0) {
            return Vec::new();
        }
        let (bottom, top) = (self.lo.log10().round() as i32, self.hi.log10().round() as i32);
        let labelled = |d: i32| ticks.iter().any(|t| (t.log10() - f64::from(d)).abs() < 1e-9);
        if decade >= 16.0 * u {
            // Every decade not labelled, and 2 to 9 times each.
            let between = (bottom..top).flat_map(|d| (2..=9).map(move |m| f64::from(m) * 10f64.powi(d)));
            let mut marks: Vec<f64> =
                (bottom..=top).filter(|d| !labelled(*d)).map(|d| 10f64.powi(d)).chain(between).collect();
            marks.sort_by(f64::total_cmp);
            marks
        } else if decade >= 3.0 * u {
            (bottom..=top).filter(|d| !labelled(*d)).map(|d| 10f64.powi(d)).collect()
        } else {
            Vec::new()
        }
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
    /// For a flight diagram: each shot drawn, as the tower (from 0), the x of its launch and of
    /// its readout, and whether the tower takes it (a shot not taken is drawn dashed).
    pub flights: Vec<(usize, f32, f32, bool)>,
    /// The values of the labelled marks of the horizontal and the vertical axis, in the data's
    /// unit (a signal's in radians, whatever multiple its numbers are written in).
    pub ticks: [Vec<f64>; 2],
    /// What is written at each of `ticks`: a decade of a logarithmic axis as `10^-5` (drawn as
    /// 10 with its exponent raised).
    pub tick_labels: [Vec<String>; 2],
    /// The values of the short unlabelled marks of each axis ([`Axis::minor_ticks`]).
    pub minor_ticks: [Vec<f64>; 2],
    /// The label of each axis as written: the caption under the horizontal axis, and the
    /// vertical axis's name and unit written up the page beside its numbers. `None` where
    /// there is none.
    pub axis_labels: [Option<String>; 2],
    /// Whether the plot has a title (the first of `labels` when it does).
    pub titled: bool,
    /// What a label by a mark must keep clear of: each stem's line, each dot and each diamond,
    /// as the rectangle it covers, points.
    pub marks: Vec<Rect>,
    /// Each label written by a mark: its text, its rectangle, and which of `marks` are its own
    /// (the only ones it may touch).
    pub marked: Vec<(String, Rect, Vec<usize>)>,
    /// For a flight diagram: each row's height scale, as its x and the y of its two ends (the
    /// tower's base, then the top of the scale), points.
    pub scale_bars: Vec<[f32; 3]>,
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
        self.marks.iter_mut().for_each(|rect| *rect = rect.translate(by));
        self.marked.iter_mut().for_each(|(_, rect, _)| *rect = rect.translate(by));
        self.scale_bars.iter_mut().for_each(|bar| *bar = [bar[0] + by.x, bar[1] + by.y, bar[2] + by.y]);
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
    /// For a signal: the tower (from 0) each series is, in the order of `series`. Empty for
    /// every other plot.
    pub towers: Vec<usize>,
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
    /// The most series a signal plot draws: [`MAX_SERIES`], except in a test that measures what
    /// the cap saves.
    pub most_series: usize,
    /// Whether it has a title at its top. Without one the axes take the room; the drawing's
    /// own name (an SVG's or a PDF's title, not drawn) is kept either way.
    pub title: bool,
}

impl Subject {
    /// Plot `content` for pane `pane` (from 0): the first tower's or every tower's, the
    /// gradiometer channel, the whole axis, the scene's own instrument.
    #[must_use]
    pub const fn new(pane: usize, content: PaneContent) -> Self {
        Self {
            pane,
            content,
            tower: None,
            channel: ChannelName::Gradiometer,
            span: None,
            compared: false,
            most_series: MAX_SERIES,
            title: true,
        }
    }

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
            most_series: MAX_SERIES,
            title: true,
        }
    }
}

/// The most series a signal plot draws. A scene may hold a thousand towers; twelve lines are
/// already more than a reader tells apart, and the data file beside a figure holds them all.
pub const MAX_SERIES: usize = 12;

/// Which of `towers` towers (from 0) a signal plot draws when it may draw `most`: all of them
/// if they are no more than that; otherwise the first `most`, except that the selected tower
/// (`selected`, from 0), if it is not among those, takes the last place. In rising order.
#[must_use]
pub fn towers_drawn(towers: usize, selected: Option<usize>, most: usize) -> Vec<usize> {
    let mut drawn: Vec<usize> = (0..towers.min(most)).collect();
    if let (Some(selected), Some(last)) = (selected.filter(|n| (most..towers).contains(n)), drawn.last_mut())
    {
        *last = selected;
    }
    drawn
}

/// Towers (from 0) as a person reads them, numbered from 1, runs written as ranges:
/// `1–11 and 250`.
#[must_use]
pub fn numbered(towers: &[usize]) -> String {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for tower in towers {
        match runs.last_mut() {
            Some((_, last)) if *last + 1 == *tower => *last = *tower,
            _ => runs.push((*tower, *tower)),
        }
    }
    let said: Vec<String> = runs
        .iter()
        .map(|(first, last)| match last - first {
            0 => format!("{}", first + 1),
            1 => format!("{}, {}", first + 1, last + 1),
            _ => format!("{}–{}", first + 1, last + 1),
        })
        .collect();
    match said.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => said.concat(),
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

/// How many shots each stretch of a tower's envelope holds when `shots` shots are drawn across
/// `width` points: `None` when there are no more shots than points (each shot is drawn, as
/// [`runs`] gives them); otherwise enough that a stretch is at least two points wide, and never
/// fewer than four shots.
#[must_use]
pub fn envelope_stretch(shots: usize, width: f32) -> Option<usize> {
    let width = f64::from(width.max(1.0));
    (shots as f64 > width).then(|| ((2.0 * shots as f64 / width).ceil() as usize).max(4))
}

/// A line of data points, `(x, y)`, in the data's units.
pub type Line = Vec<(f64, f64)>;

/// A tower's series too dense to be drawn shot by shot, as its envelope: for each stretch of
/// `stretch` shots (in order), the least and the greatest value taken in it, at the middle of
/// the times of its first and last shot taken. Each item is one unbroken run, as its upper line
/// and its lower line, of the same length; a stretch in which no shot was taken breaks the run
/// (a shot not taken holds a zero that is no measurement, and is on neither line).
#[must_use]
pub fn envelope(x: &[f64], y: &[f64], taken: &[bool], stretch: usize) -> Vec<(Line, Line)> {
    let n = x.len().min(y.len()).min(taken.len());
    let mut out = Vec::new();
    let (mut upper, mut lower) = (Vec::new(), Vec::new());
    for start in (0..n).step_by(stretch.max(1)) {
        let mut kept = (start..(start + stretch.max(1)).min(n)).filter(|&k| taken[k]);
        let Some(first) = kept.next() else {
            if !upper.is_empty() {
                out.push((std::mem::take(&mut upper), std::mem::take(&mut lower)));
            }
            continue;
        };
        let (mut least, mut most, mut last) = (y[first], y[first], first);
        for k in kept {
            (least, most, last) = (least.min(y[k]), most.max(y[k]), k);
        }
        let at = f64::midpoint(x[first], x[last]);
        upper.push((at, most));
        lower.push((at, least));
    }
    if !upper.is_empty() {
        out.push((upper, lower));
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

/// What goes round a plot's axes: a title at the top (the first of `titles` that fits), unless
/// it is left out; `legend` lines under it; the numbers (`left`) and the name of the vertical
/// axis (`y_label`, written up the page) to the left.
struct Frame<'b> {
    titles: &'b [String],
    titled: bool,
    legend: usize,
    left: &'b [String],
    y_label: Option<&'b str>,
}

/// A periodogram's label asked for: its order (harmonic, then body), its mark's top, its text,
/// its colour, its own marks, and the comb and stem the record gives it to.
type Asked = ((u32, usize), Pos2, String, Colour, Vec<usize>, Option<(usize, usize)>);

/// A label waiting to be placed by its mark once every mark is drawn.
struct Waiting {
    top: Pos2,
    text: String,
    colour: Colour,
    own: Vec<usize>,
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
    /// Where the first legend line's top is.
    legend_top: f32,
    titled: bool,
    marks: Vec<Rect>,
    waiting: Vec<Waiting>,
    marked: Vec<(String, Rect, Vec<usize>)>,
    ticks: [Vec<f64>; 2],
    tick_labels: [Vec<String>; 2],
    minor: [Vec<f64>; 2],
    axis_labels: [Option<String>; 2],
}

/// The text a log axis writes at a decade, as the record gives it: `10^-5`.
fn power_text(value: f64) -> String {
    format!("10^{}", value.log10().round() as i32)
}

/// An exponent as it is written raised: with a minus sign, not a hyphen.
fn exponent(value: f64) -> String {
    let n = value.log10().round() as i32;
    if n < 0 { format!("\u{2212}{}", -n) } else { n.to_string() }
}

impl<'a> Sheet<'a> {
    /// A sheet of `size` points framed as `frame` says. Where the style does not fix the margin
    /// at the left, it is as wide as the numbers and the axis's name need.
    fn new(style: &'a Style, measure: &'a dyn Measure, size: [f32; 2], frame: &Frame<'_>) -> Self {
        let u = style.unit();
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(size[0], size[1]));
        let widest = frame
            .left
            .iter()
            .map(|text| measure.size(text, Face::Mono, style.tick_size)[0])
            .fold(0.0f32, f32::max);
        let named =
            frame.y_label.map_or(0.0, |_| measure.size("M", Face::Sans, style.text_size)[1] + 4.0 * u);
        let needed = widest + 11.0 * u + named;
        let margin = style.left.map_or(needed, |fixed| fixed.max(needed));
        let legend_top = if frame.titled { 24.0 * u } else { 7.0 * u };
        let top = legend_top + (13.0 * frame.legend as f32 + 14.0) * u;
        let area = Rect::from_min_max(pos2(margin, top), rect.max - vec2(16.0 * u, 34.0 * u));
        let mut sheet = Self {
            style,
            measure,
            drawing: Drawing::new(size, frame.titles.first().cloned().unwrap_or_default()),
            rect,
            area,
            u,
            labels: Vec::new(),
            fixed: 0,
            legend_top,
            titled: frame.titled,
            marks: Vec::new(),
            waiting: Vec::new(),
            marked: Vec::new(),
            ticks: [Vec::new(), Vec::new()],
            tick_labels: [Vec::new(), Vec::new()],
            minor: [Vec::new(), Vec::new()],
            axis_labels: [None, None],
        };
        let title = sheet.fit(frame.titles, Face::Sans, style.title_size, size[0] - 16.0 * u);
        // The drawing is named by its title whether or not the title is drawn.
        sheet.drawing.title.clone_from(&title);
        sheet.drawing.items.push(Item::Rect {
            min: p(rect.min),
            max: p(rect.max),
            fill: Some(style.background),
            stroke: None,
        });
        sheet.drawing.items.push(Item::Rect {
            min: p(area.min),
            max: p(area.max),
            fill: None,
            stroke: Some((style.rule_width, style.rule)),
        });
        if frame.titled {
            sheet.text(
                pos2(8.0 * u, 5.0 * u),
                Anchor::LEFT_TOP,
                title,
                Face::Sans,
                style.title_size,
                style.ink,
            );
        }
        if let Some(name) = frame.y_label {
            // Up the page, centred on the axis, its right side 6·u left of the widest number.
            let at = pos2(margin - 11.0 * u - widest, area.center().y);
            let name = sheet.fit(&[name.to_owned()], Face::Sans, style.text_size, area.height());
            sheet.turned(at, Anchor(0.5, 1.0), &name, style.text_size, style.weak);
            sheet.axis_labels[1] = Some(name);
        }
        sheet
    }

    /// Leaves `room` points between the axes and the page's right edge (if that is more than
    /// there is already): before anything is drawn on the axes.
    fn narrow_right(&mut self, room: f32) {
        let right = (self.rect.right() - room).min(self.area.right());
        self.area.max.x = right.max(self.area.left() + 1.0);
        if let Some(Item::Rect { max, .. }) = self.drawing.items.get_mut(1) {
            max[0] = self.area.max.x;
        }
    }

    /// The top of legend line `line` (from 0).
    fn legend(&self, line: usize) -> f32 {
        self.legend_top + 13.0 * line as f32 * self.u
    }

    /// The first of `forms` that is no wider than `width` in `face` at `size`; if none is, the
    /// last cut short with an ellipsis until it is (an ellipsis alone at worst).
    fn fit(&self, forms: &[String], face: Face, size: f32, width: f32) -> String {
        let wide = |text: &str| self.measure.size(text, face, size)[0];
        if let Some(form) = forms.iter().find(|form| wide(form) <= width) {
            return form.clone();
        }
        let mut cut: Vec<char> = forms.last().map(|form| form.chars().collect()).unwrap_or_default();
        while !cut.is_empty() {
            cut.pop();
            let text: String = cut.iter().collect::<String>().trim_end().to_owned() + "…";
            if wide(&text) <= width {
                return text;
            }
        }
        "…".to_owned()
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
        self.drawing.items.push(Item::Text {
            at: p(at),
            anchor,
            text: text.clone(),
            face,
            size,
            colour,
            turned: false,
        });
        self.labels.push((text, took));
        took
    }

    /// Writes text up the page, turned a quarter turn about `at`, and remembers its rectangle.
    fn turned(&mut self, at: Pos2, anchor: Anchor, text: &str, size: f32, colour: Colour) -> Rect {
        let measured = self.measure.size(text, Face::Sans, size);
        let (min, max) = crate::drawing::text_box(p(at), anchor, measured, true);
        let took = Rect::from_min_max(pos2(min[0], min[1]), pos2(max[0], max[1]));
        self.drawing.items.push(Item::Text {
            at: p(at),
            anchor,
            text: text.to_owned(),
            face: Face::Sans,
            size,
            colour,
            turned: true,
        });
        self.labels.push((text.to_owned(), took));
        took
    }

    fn small(&mut self, at: Pos2, anchor: Anchor, text: impl Into<String>) {
        self.text(at, anchor, text, Face::Mono, self.style.tick_size, self.style.weak);
    }

    /// The caption under the horizontal axis, which is its label: the first of `forms` that
    /// fits the width of the axes, or the last cut short.
    fn caption(&mut self, forms: &[String]) {
        let text = self.fit(forms, Face::Mono, self.style.tick_size, self.area.width());
        self.axis_labels[0] = Some(text.clone());
        self.small(pos2(self.area.center().x, self.area.bottom() + 18.0 * self.u), Anchor::CENTRE_TOP, text);
    }

    /// Records a mark a label must keep clear of, as the rectangle it covers (inside the
    /// axes); returns its index, for a label that is its own.
    fn mark(&mut self, covers: Rect) -> usize {
        let inside = covers.intersect(self.area.expand(3.0 * self.u));
        self.marks.push(if inside.is_positive() { inside } else { covers });
        self.marks.len() - 1
    }

    /// A stem from `top` down to the axis, with a dot on top, as two marks: returns them.
    fn stem(&mut self, top: Pos2, colour: Colour, dash: &[f32]) -> Vec<usize> {
        self.clip(3.0);
        self.rule(pos2(top.x, self.area.bottom()), top, colour, dash);
        self.dot(top, colour);
        self.unclip();
        let half = (self.style.rule_width / 2.0).max(0.25);
        let line = Rect::from_min_max(pos2(top.x - half, top.y), pos2(top.x + half, self.area.bottom()));
        let dot = Rect::from_center_size(top, Vec2::splat(2.0 * self.style.dot));
        vec![self.mark(line), self.mark(dot)]
    }

    /// Asks for a label by a mark whose top is `top`; it is placed by [`Self::place_labels`],
    /// once every mark is drawn. `own` are the marks it may touch. Returns its number.
    fn label_by(&mut self, top: Pos2, text: &str, colour: Colour, own: Vec<usize>) -> usize {
        self.waiting.push(Waiting { top, text: text.to_owned(), colour, own });
        self.waiting.len() - 1
    }

    /// Places every label asked for, in the order asked: each by its mark's top, where its
    /// rectangle, with a little air round it, meets no label already written, no mark but its
    /// own, and stays inside the axes: to the right of the mark, then to its left, then a line
    /// higher, up to three lines. A label with nowhere to go is left out; its mark still
    /// stands. Returns, for each label asked for, whether it was written.
    fn place_labels(&mut self) -> Vec<bool> {
        let (u, font) = (self.u, self.style.label_size);
        let waiting = std::mem::take(&mut self.waiting);
        let mut written = Vec::with_capacity(waiting.len());
        for label in waiting {
            let [w, h] = self.measure.size(&label.text, Face::Sans, font);
            let mut placed = None;
            'tries: for lift in 0..4 {
                let bottom = (label.top.y - 3.0 * u - 11.0 * u * lift as f32).max(self.area.top() + h + 1.0);
                for x in [label.top.x + 3.0 * u, label.top.x - 3.0 * u - w] {
                    let took = Rect::from_min_size(pos2(x, bottom - h), vec2(w, h));
                    if !self.area.contains_rect(took) {
                        continue;
                    }
                    // With a little air round it: labels that touch read as one.
                    let air = took.expand2(vec2(4.0 * u, u));
                    let clear_of_labels =
                        self.labels[self.fixed..].iter().all(|(_, other)| !other.intersects(air));
                    let clear_of_marks = self
                        .marks
                        .iter()
                        .enumerate()
                        .all(|(n, mark)| label.own.contains(&n) || !mark.intersects(air));
                    if clear_of_labels && clear_of_marks {
                        placed = Some(took);
                        break 'tries;
                    }
                }
            }
            if let Some(took) = placed {
                self.text(took.min, Anchor::LEFT_TOP, label.text.clone(), Face::Sans, font, label.colour);
                self.marked.push((label.text, took, label.own));
            }
            written.push(placed.is_some());
        }
        written
    }

    fn nothing(&mut self, why: &str) {
        let at = self.rect.center();
        let why =
            self.fit(&[why.to_owned()], Face::Sans, self.style.title_size, self.rect.width() - 16.0 * self.u);
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
            self.ticks[0].push(value);
            self.tick_labels[0].push(text(value));
        }
    }

    /// The marks of both axes: a faint line across the plot and a number outside it. A
    /// logarithmic vertical axis writes each decade as a power of ten, and has short marks
    /// between ([`Axis::minor_ticks`]); `y_text` is not used for it.
    fn axes(&mut self, x: &Axis, y: &Axis, x_text: impl Fn(f64) -> String, y_text: impl Fn(f64) -> String) {
        let (area, u) = (self.area, self.u);
        self.x_marks(x, x_text);
        let ticks = y.ticks(6);
        for value in &ticks {
            let at = area.bottom() - area.height() * y.fraction(*value) as f32;
            self.rule(pos2(area.left(), at), pos2(area.right(), at), self.style.rule, &[]);
            let right = pos2(area.left() - 5.0 * u, at);
            if y.log {
                self.power(right, *value);
                self.tick_labels[1].push(power_text(*value));
            } else {
                self.small(right, Anchor::RIGHT_CENTRE, y_text(*value));
                self.tick_labels[1].push(y_text(*value));
            }
        }
        if y.log {
            let decade = area.height() / (y.hi.log10() - y.lo.log10()) as f32;
            let minor = y.minor_ticks(&ticks, decade, u);
            for value in &minor {
                let at = area.bottom() - area.height() * y.fraction(*value) as f32;
                self.rule(pos2(area.left(), at), pos2(area.left() + 3.0 * u, at), self.style.weak, &[]);
            }
            self.minor[1] = minor;
        }
        self.ticks[1] = ticks;
    }

    /// A decade written as a power of ten, its right side and middle at `right`: `10`, and the
    /// exponent smaller and raised (the faces have no raised minus, so the exponent is a second
    /// piece of text, not a glyph).
    fn power(&mut self, right: Pos2, value: f64) {
        let (size, small) = (self.style.tick_size, self.style.tick_size * 0.72);
        let raised = exponent(value);
        let wide = self.measure.size(&raised, Face::Mono, small)[0];
        // A hair apart, so that the two boxes do not touch.
        let gap = 0.05 * small;
        let base = pos2(right.x - wide - gap, right.y);
        self.text(base, Anchor::RIGHT_CENTRE, "10", Face::Mono, size, self.style.weak);
        let at = pos2(base.x + gap, right.y + 0.05 * size);
        self.text(at, Anchor::LEFT_BOTTOM, raised, Face::Mono, small, self.style.weak);
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
        let top = self.legend(line);
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

    /// A row of key entries on legend line `line`, from the axes' left edge: `(name, colour,
    /// dash)` each. All of them if they fit; otherwise as many as fit together with a note,
    /// "and N more" (or "+N" where that is all there is room for), so that a series never
    /// leaves the key without a word. Returns how many were named.
    fn key_row(&mut self, line: usize, entries: &[(String, Colour, Vec<f32>)]) -> usize {
        let (u, size) = (self.u, self.style.text_size);
        let (left, right) = (self.area.left(), self.area.right());
        let wide = |text: &str| self.measure.size(text, Face::Sans, size)[0];
        // Where each entry would end, and where the next would start.
        let mut ends = Vec::with_capacity(entries.len());
        let mut at = left;
        for (name, ..) in entries {
            let end = at + 18.0 * u + wide(name);
            ends.push(end);
            at = end + 12.0 * u;
        }
        let all_fit = ends.last().is_none_or(|end| *end <= right);
        let named = if all_fit {
            entries.len()
        } else {
            // The most entries that leave room for the note of the rest.
            let note_room = |named: usize| {
                let start = if named == 0 { left } else { ends[named - 1] + 12.0 * u };
                let rest = entries.len() - named;
                [format!("and {rest} more"), format!("+{rest}")]
                    .into_iter()
                    .any(|note| start + wide(&note) <= right)
            };
            (0..entries.len())
                .rev()
                .find(|named| (*named == 0 || ends[named - 1] <= right) && note_room(*named))
                .unwrap_or(0)
        };
        let mut x = left;
        for (name, colour, dash) in &entries[..named] {
            x = self.key(x, line, name, *colour, dash).unwrap_or(x);
        }
        if named < entries.len() {
            let rest = entries.len() - named;
            let forms = [format!("and {rest} more"), format!("+{rest}")];
            let note = self.fit(&forms, Face::Sans, size, (right - x).max(0.0));
            let top = pos2(x, self.legend(line));
            let weak = self.style.weak;
            self.text(top, Anchor::LEFT_TOP, note, Face::Sans, size, weak);
        }
        named
    }

    /// One entry of a key on legend line `line`, starting at `x`: a diamond like the ULDM
    /// benchmark's, then `name`. Nothing is written, and false returned, when it would run past
    /// the axes' right edge.
    fn key_mark(&mut self, x: f32, line: usize, name: &str, style: &Style) -> bool {
        let (u, size) = (self.u, style.text_size);
        let [w, h] = self.measure.size(name, Face::Sans, size);
        if x + 14.0 * u + w > self.area.right() {
            return false;
        }
        let top = self.legend(line);
        let radius = style.dot * 1.6;
        self.drawing.items.push(Item::Diamond {
            centre: [x + 5.0 * u, top + h / 2.0],
            radius,
            fill: style.ink,
            stroke: (style.rule_width, style.ink),
        });
        let ink = style.naming(style.ink);
        self.text(pos2(x + 14.0 * u, top), Anchor::LEFT_TOP, name, Face::Sans, size, ink);
        true
    }

    fn finish(mut self, mut drawn: Drawn, described: Described) -> Plot {
        if !self.waiting.is_empty() {
            self.place_labels();
        }
        drawn.area = self.area;
        drawn.labels = self.labels;
        drawn.ticks = self.ticks;
        drawn.tick_labels = self.tick_labels;
        drawn.minor_ticks = self.minor;
        drawn.axis_labels = self.axis_labels;
        drawn.titled = self.titled;
        drawn.marks = self.marks;
        drawn.marked = self.marked;
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

/// What a logarithmic axis writes at a decade, as one piece of text, for measuring a margin.
fn decade(value: f64) -> String {
    format!("10{}", exponent(value))
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
        ticks: [Vec::new(), Vec::new()],
        tick_labels: [Vec::new(), Vec::new()],
        minor_ticks: [Vec::new(), Vec::new()],
        axis_labels: [None, None],
        titled: subject.title,
        marks: Vec::new(),
        marked: Vec::new(),
        scale_bars: Vec::new(),
    }
}

fn described(x: (&str, &str), y: (&str, &str)) -> Described {
    Described {
        x: (x.0.to_owned(), x.1.to_owned()),
        y: (y.0.to_owned(), y.1.to_owned()),
        series: Vec::new(),
        towers: Vec::new(),
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
        let frame = Frame { titles: &titles, titled: subject.title, legend: 0, left: &[], y_label: None };
        let mut sheet = Sheet::new(style, measure, size, &frame);
        sheet.nothing(why);
        drawn.note = Some(why.to_owned());
        sheet.finish(drawn, described)
    };
    let Some(signal) = held else {
        return empty("computing the signal", drawn, described);
    };
    let channel = subject.channel.channel();
    let towers = signal.towers.min(spec.array.towers.len());
    // With more towers than a plot draws, the selected one is among those drawn.
    let selected = match engine.selection() {
        Some(cav_engine::Target::Tower(n)) => n.checked_sub(1),
        _ => None,
    };
    let which: Vec<usize> = match subject.tower {
        None => towers_drawn(towers, selected, subject.most_series),
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
    let left: Vec<String> = y.ticks(6).into_iter().map(y_text).collect();
    // A key of the towers' colours, on a line of its own, when there is more than one.
    let keyed = which.len() > 1;
    let y_label = format!("{name}, {unit}");
    let frame = Frame {
        titles: &titles,
        titled: subject.title,
        legend: usize::from(keyed),
        left: &left,
        y_label: Some(&y_label),
    };
    let mut sheet = Sheet::new(style, measure, size, &frame);
    sheet.axes(&x, &y, plain, y_text);
    let area = sheet.area;
    // The towers the key names: all of them, or as many as fit with a note of how many it
    // leaves out, which is always written. No series leaves the key without a word.
    let mut keyed_towers: Vec<usize> = Vec::new();
    if keyed {
        let entries: Vec<(String, Colour, Vec<f32>)> = which
            .iter()
            .map(|tower| {
                let (colour, dash) = style.series(*tower);
                (spec.array.towers[*tower].label.clone(), colour, dash.to_vec())
            })
            .collect();
        let named = sheet.key_row(0, &entries);
        keyed_towers.extend_from_slice(&which[..named]);
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
        // More shots than points across: each shot joined to the next would fill the plot
        // from one side of the band to the other, and the towers over it would hide it. The
        // tower is drawn as its envelope, two lines, so that what is under it shows between.
        if let Some(stretch) = envelope_stretch(times.len(), area.width()) {
            let width = style.line * 0.8;
            sheet.clip(1.0);
            for (upper, lower) in envelope(times, values, taken, stretch) {
                let at =
                    |line: &[(f64, f64)]| line.iter().map(|q| p(place(area, &x, &y, *q))).collect::<Vec<_>>();
                let (upper, lower) = (at(&upper), at(&lower));
                drawn.points += upper.len() + lower.len();
                // A run of one stretch is a stroke from its least to its greatest.
                let lines =
                    if upper.len() == 1 { vec![vec![lower[0], upper[0]]] } else { vec![upper, lower] };
                for points in lines {
                    // The envelope keeps the series' dash: past the fifth hue a dash is all that
                    // tells two series apart, and the key shows it.
                    sheet.drawing.items.push(Item::Line { points, width, colour, dash: dash.to_vec() });
                }
            }
            sheet.unclip();
            continue;
        }
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
                dash: dash.to_vec(),
            });
        }
        sheet.unclip();
    }
    // What is drawn is said in the towers' own order, whatever order they were drawn in.
    for tower in &which {
        described.series.push((spec.array.towers[*tower].label.clone(), style.series(*tower).0));
    }
    described.towers.clone_from(&which);
    let time = engine.time();
    if !style.house && (from..=to).contains(&time) {
        let at = place(area, &x, &y, (time, y.lo)).x;
        sheet.rule(pos2(at, area.top()), pos2(at, area.bottom()), style.playhead, &[]);
        drawn.playhead = Some(at);
    }
    // How many shots there are and how many each tower drawn took, over the whole signal.
    // Each tower has its own shots (a tower of its own instrument may have another number).
    let own: Vec<usize> = which.iter().map(|t| signal.count(*t)).collect();
    let shots = own.iter().copied().max().unwrap_or(0);
    let taken: Vec<usize> =
        which.iter().map(|t| signal.taken(*t).iter().filter(|taken| **taken).count()).collect();
    let (fewest, most) =
        (taken.iter().min().copied().unwrap_or(shots), taken.iter().max().copied().unwrap_or(shots));
    let even = own.iter().all(|n| *n == shots);
    let count = match (even, fewest == shots, which.len() == 1 || fewest == most) {
        (true, true, _) => format!("{shots} shots"),
        (true, _, true) => format!("{most} of {shots} shots taken"),
        (true, ..) => format!("{shots} shots, {fewest} to {most} taken a tower"),
        (false, ..) => format!(
            "{} to {shots} shots a tower, {fewest} to {most} taken",
            own.iter().copied().min().unwrap_or(0)
        ),
    };
    // When not every tower is drawn the plot says so, at every size.
    let left_out = (subject.tower.is_none() && which.len() < towers)
        .then(|| format!("{} of {towers} towers drawn", which.len()));
    described.marks = json!({
        "vertical_axis_written_in": unit,
        "shots": shots,
        "shots_taken": taken,
        "towers": {
            "in_the_scene": towers,
            "drawn": which.iter().map(|tower| tower + 1).collect::<Vec<_>>(),
            "named_in_the_key": keyed.then(|| keyed_towers.iter().map(|tower| tower + 1).collect::<Vec<_>>()),
            "note": left_out.as_ref().map(|said| format!(
                "{said}: {} (a plot draws at most {} series; the selected tower is always among them); \
                 the data file has every tower",
                numbered(&which),
                subject.most_series
            )),
        },
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
    if let Some(said) = left_out {
        let which = numbered(&which);
        let file = if style.house { "; the data file has them all" } else { "" };
        forms = vec![
            format!("readout time, s · {count} · {said}: {which}{file}"),
            format!("readout time, s · {said}: {which}{file}"),
            format!("time, s · {said}: {which}"),
            format!("time, s · {said}"),
            said,
        ];
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
    let frame = Frame {
        titles: &titles,
        titled: subject.title,
        legend: 2,
        left: &left,
        y_label: Some("|W|, relative to the tone"),
    };
    let mut sheet = Sheet::new(style, measure, size, &frame);
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
        let own = sheet.stem(top, colour, &[]);
        sheet.label_by(top, &format!("{:.3}", line.height), style.naming(colour), own);
        drawn.stems.push((line.what.clone(), k as u32 + 1, top.x, false));
        lines.push(json!({ "what": line.what, "frequency_hz": line.frequency, "height": line.height }));
    }
    let written = sheet.place_labels();
    for (line, written) in lines.iter_mut().zip(written) {
        line["label"] = if written {
            json!(format!("{:.3}", line["height"].as_f64().unwrap_or_default()))
        } else {
            json!(null)
        };
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
    let (cycle, nyquist) = (plain(predicted.mean_cycle), plain(predicted.nyquist));
    let shots = match (fewest == predicted.shots, fewest == most) {
        (true, _) => format!("{} shots", predicted.shots),
        (_, true) => format!("{} shots planned, {most} taken a tower", predicted.shots),
        _ => format!("{} shots planned, {fewest} to {most} taken a tower", predicted.shots),
    };
    let factor = predicted.noise_factor.map_or_else(
        || "a tower takes no shot".to_owned(),
        |factor| format!("smallest detectable tone × {factor:.3}"),
    );
    // Each legend line is the first of its forms that fits over the axes (the last cut short).
    let said = [
        format!("mean cycle {cycle} s · Nyquist {nyquist} Hz · {shots} · {factor}"),
        format!("mean cycle {cycle} s · Nyquist {nyquist} Hz · {shots}"),
        format!("mean cycle {cycle} s · {shots}"),
    ];
    let key = if style.house {
        [
            "grey: this seed's window · stems: the lines the closed forms predict · dashed: the floor they predict",
            "grey: this seed's window · stems: predicted lines · dashed: predicted floor",
            "grey: window · stems: predicted",
        ]
    } else {
        [
            "grey: this seed's window · stems: the lines the closed forms predict, at their predicted heights",
            "grey: this seed's window · stems: the predicted lines",
            "grey: window · stems: predicted",
        ]
    }
    .map(str::to_owned);
    let line = sheet.fit(&said, Face::Sans, style.text_size, area.width());
    sheet.text(
        pos2(area.left(), sheet.legend(0)),
        Anchor::LEFT_TOP,
        line,
        Face::Sans,
        style.text_size,
        style.text,
    );
    let line = sheet.fit(&key, Face::Sans, style.text_size, area.width());
    let ink = style.naming(colour);
    sheet.text(pos2(area.left(), sheet.legend(1)), Anchor::LEFT_TOP, line, Face::Sans, style.text_size, ink);
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
    let long = format!("Flight · the atoms' arms, height against absolute time · {}", spec.name);
    let titles = vec![long, format!("Flight · the atoms' arms · {}", spec.name), "Flight".to_owned()];
    let towers = spec.array.towers.len().min(MAX_FLIGHT_ROWS);
    let left: Vec<String> = spec.array.towers.iter().take(towers).map(|t| t.label.clone()).collect();
    let frame = Frame { titles: &titles, titled: subject.title, legend: 0, left: &left, y_label: None };
    let mut sheet = Sheet::new(style, measure, size, &frame);
    // Each tower flies its own instrument: its own arms and its own 2T. With no tower drawn, the
    // array's stands for the scale.
    let own: Vec<(cav_sim::InstrumentSpec, cav_core::forward::Instrument)> = (0..towers)
        .map(|n| {
            let resolved = spec.array.instrument_of(n);
            let instrument = resolved.instrument();
            (resolved, instrument)
        })
        .collect();
    let array = (spec.array.instrument.clone(), spec.array.instrument.instrument());
    let reach = |(resolved, instrument): &(cav_sim::InstrumentSpec, cav_core::forward::Instrument)| {
        resolved.tower_height.max(arm_heights(instrument, instrument.half_sequence)[3])
    };
    // Each row's height scale stands at the right of the rows, its top named in metres: one scale
    // for every row, the tallest tower's (or arms').
    let top = if own.is_empty() { reach(&array) } else { own.iter().map(reach).fold(0.0, f64::max) };
    let flights: Vec<f64> = own.iter().map(|(_, instrument)| instrument.flight_time()).collect();
    let (shortest, longest) =
        flights.iter().fold((f64::INFINITY, 0.0f64), |(a, b), f| (a.min(*f), b.max(*f)));
    let flight_text = if flights.is_empty() {
        format!("2T = {} s", plain(array.1.flight_time()))
    } else if shortest < longest {
        format!("2T = {} to {} s", plain(shortest), plain(longest))
    } else {
        format!("2T = {} s", plain(longest))
    };
    let top_name = format!("{} m", plain(top));
    let named = measure.size(&top_name, Face::Mono, style.tick_size)[0];
    sheet.narrow_right(12.0 * sheet.u + named);
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
        // The row's scale: a bar from the tower's base (0) to the top of the axis, as the arms
        // are placed in the row; the first row names its two ends.
        let bar = area.right() + 5.0 * sheet.u;
        let (base, summit) = (
            inner.bottom() - inner.height() * y.fraction(0.0) as f32,
            inner.bottom() - inner.height() * y.fraction(top) as f32,
        );
        sheet.drawing.items.push(Item::Line {
            points: vec![
                [bar - 1.5 * sheet.u, base],
                [bar, base],
                [bar, summit],
                [bar - 1.5 * sheet.u, summit],
            ],
            width: style.rule_width,
            colour: style.weak,
            dash: Vec::new(),
        });
        drawn.scale_bars.push([bar, base, summit]);
        if n == 0 {
            let right = bar + 3.0 * sheet.u;
            sheet.small(pos2(right, summit), Anchor::LEFT_CENTRE, top_name.clone());
            sheet.small(pos2(right, base), Anchor::LEFT_CENTRE, "0");
        }
        sheet.small(
            pos2(area.left() - 5.0 * sheet.u, row.center().y),
            Anchor::RIGHT_CENTRE,
            tower.label.clone(),
        );
        let (count, flown) = flights_drawn(shots, n, [from, to]);
        let instrument = &own[n].1;
        let flight_time = instrument.flight_time();
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
                        let height = arm_heights(instrument, tau)[arm];
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
                for height in arm_heights(instrument, time - launch) {
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
    let mut said = format!("absolute time, s · each row a tower, 0 to {} m · {flight_text}", plain(top));
    if not_taken > 0 {
        said.push_str(&format!(" · dashed: {not_taken} shot(s) not taken"));
    }
    if more > 0 || left_out > 0 {
        said.push_str(&format!(" · not drawn: {more} tower(s), {left_out} shot(s); zoom in"));
        drawn.note = Some(format!("{more} tower(s) and {left_out} shot(s) are not drawn"));
    }
    described.marks = json!({
        "row_height_m": top,
        "flight_times_s": flights,
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
    // The tower's own instrument: the array's (or the compared one), with the tower's override.
    let base = engine.comparison().filter(|_| compared).unwrap_or(&spec.array.instrument);
    let own = spec.array.towers.get(tower.wrapping_sub(1)).and_then(|t| t.instrument.as_ref());
    let cycle = own.map_or(base.cycle, |own| own.applied_to(base).cycle);
    // Half the mean shot rate: the schedule's, which for the plain schedule is the cycle's.
    let nyquist = 0.5 / spec.array.schedule.mean_cycle(cycle);
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
        let frame = Frame {
            titles: &titles,
            titled: subject.title,
            legend: combs + usize::from(style.house),
            left: &[],
            y_label: None,
        };
        let mut sheet = Sheet::new(style, measure, size, &frame);
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
    let left: Vec<String> = y.ticks(6).into_iter().map(decade).collect();
    // The house style's key has a line more: the floors are named there, where no stem can
    // cross the words, and told apart by the axis they lie on.
    let frame = Frame {
        titles: &titles,
        titled: subject.title,
        legend: combs + usize::from(style.house) + usize::from(uldm.is_some()),
        left: &left,
        y_label: Some("√S, rad/√Hz"),
    };
    let mut sheet = Sheet::new(style, measure, size, &frame);
    sheet.axes(&x, &y, plain, decade);
    let (area, u) = (sheet.area, sheet.u);
    for floor in floors.iter().filter(|floor| **floor >= y.lo) {
        let level = place(area, &x, &y, (from, *floor)).y;
        sheet.rule(pos2(area.left(), level), pos2(area.right(), level), style.reference, &[]);
    }
    if style.house {
        // In µrad/√Hz the floors are plain numbers (the faces have no raised minus).
        let named: Vec<String> = floors.iter().map(|floor| plain(floor * 1e6)).collect();
        let text = format!("shot-noise floors, {} µrad/√Hz", named.join(" and "));
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
    // Labels wait until every mark is drawn: (its order, its mark's top, the text, the colour,
    // its own marks, and where the record says it: the comb and the stem, or the benchmark).
    let mut asked: Vec<Asked> = Vec::new();
    for (n, comb) in spectrum.combs.iter().enumerate() {
        let colour = style.comb(n);
        // Each label's place, text, own marks and first stem.
        let mut named: Vec<(Pos2, String, Vec<usize>, usize, u32)> = Vec::new();
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
            let dash = if harmonic.folded { vec![4.0 * u, 3.0 * u] } else { Vec::new() };
            let own = sheet.stem(top, colour, &dash);
            drawn.stems.push((comb.label.clone(), harmonic.n, top.x, harmonic.folded));
            stems.push(json!({
                "n": harmonic.n,
                "frequency_hz": harmonic.frequency,
                "appears_at_hz": harmonic.appears_at,
                "folded": harmonic.folded,
                "height": amplitude[bin],
                "label": null,
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
            // onto 4f₀, say) share one label rather than write over each other; it may touch
            // every stem it names.
            match named.iter_mut().find(|(at, ..)| (at.x - top.x).abs() < 3.0 * u) {
                Some((_, names, mine, _, _)) => {
                    names.push_str(" · ");
                    names.push_str(&text);
                    mine.extend(own);
                }
                None => named.push((top, text, own, stems.len() - 1, harmonic.n)),
            }
        }
        for (top, text, own, stem, harmonic) in named {
            asked.push(((harmonic, n), top, text, style.naming(colour), own, Some((n, stem))));
        }
        marks.push(json!({ "body": comb.label, "fundamental_hz": comb.fundamental, "colour": colour.css(), "stems": stems }));
        // The legend has a line of its own above the axes, where no peak can be under it.
        if n < style.combs.len() {
            let text = format!("{} · f₀ = {:.4} Hz", comb.label, comb.fundamental);
            if style.house {
                sheet.key(area.left(), n, &text, colour, &[]);
            } else {
                let at = pos2(area.left(), sheet.legend(n));
                let text = sheet.fit(&[text], Face::Sans, style.text_size, area.width());
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
        let radius = style.dot * 1.6;
        sheet.clip(3.0);
        sheet.drawing.items.push(Item::Diamond {
            centre: p(centre),
            radius,
            fill: style.ink,
            stroke: (style.rule_width, style.ink),
        });
        sheet.unclip();
        sheet.mark(Rect::from_center_size(centre, Vec2::splat(2.0 * radius)));
        // Named in the key, on a line of its own beside a diamond like its own: by the mark
        // there is seldom room, for the comb's stems stand round it (S2.md §7.14).
        let keyed = sheet.key_mark(area.left(), combs + 1, "ULDM benchmark", style);
        reference["uldm_benchmark"] = json!({
            "frequency_hz": ULDM_FREQUENCY,
            "amplitude_rad": ULDM_AMPLITUDE,
            "height_rad_per_sqrt_hz": height,
            "run_length_s": spectrum.shots as f64 / (2.0 * spectrum.nyquist),
            "note": "a reference mark from the plan, not a simulated signal: a scalar of mass 2e-16 eV (f = m c²/h) at 1e-2 µrad, drawn at the height a tone of that amplitude has in this periodogram, a·√T_obs/2, over this run's length",
            "label": null,
            "named_in_the_key": keyed,
        });
    }
    // The labels, in order: every body's f₀, then every 2f₀, and so on,
    // so that each body is named before any body's higher harmonics take the room.
    asked.sort_by_key(|asked| asked.0);
    for (_, top, text, colour, own, _) in &asked {
        sheet.label_by(*top, text, *colour, own.clone());
    }
    for ((_, _, text, _, _, at), written) in asked.into_iter().zip(sheet.place_labels()) {
        let said = if written { json!(text) } else { json!(null) };
        match at {
            Some((comb, stem)) => marks[comb]["stems"][stem]["label"] = said,
            None => reference["uldm_benchmark"]["label"] = said,
        }
    }
    described.marks = json!({
        "combs": marks,
        "reference": reference,
        "method": spectrum.method,
        "shots": spectrum.shots,
        "nyquist_hz": spectrum.nyquist,
        "folded": "a dashed stem is a harmonic above Nyquist, standing where it folds to",
    });
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
