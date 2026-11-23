//! How a plot looks: every colour, font size and line width in one value, so that the
//! application's dark theme and the presets for papers, slides and posters are the same drawing
//! code with different styles.
//!
//! The light presets' series colours ([`PAPER_SERIES`]) were chosen by search under the `dataviz`
//! skill's method and pass its validator on white, adjacent and all pairs: OKLCH lightness in the
//! band, chroma above the floor, every pair at least 12 apart (OKLab × 100, and CIEDE2000) in
//! normal vision and under simulated protanopia, deuteranopia and tritanopia, 15 apart in normal
//! vision, and 3:1 against white. The first four are at least 8 L* apart, and every series has
//! a dash of its own, so that a figure printed in black and white still tells them apart
//! (`the_palette_survives_colour_blindness_and_print`).

use crate::drawing::Colour;

/// Millimetres in a point (1/72 inch).
pub const MM_PER_POINT: f64 = 25.4 / 72.0;

/// The series colours of the light presets, in order: blue, orange, red, mauve, green, orchid,
/// wine, purple. Assigned in order and never cycled: a ninth series takes the eighth's dash
/// partner (see [`Style::series`]).
pub const PAPER_SERIES: [Colour; 8] = [
    Colour::hex(0x0b_0dff),
    Colour::hex(0xe7_6f11),
    Colour::hex(0xc6_1604),
    Colour::hex(0x8a_6aae),
    Colour::hex(0x28_a682),
    Colour::hex(0xcb_64fe),
    Colour::hex(0x91_0c59),
    Colour::hex(0x6d_1ba7),
];

/// The dash of each series in the light presets, points at a line width of one (scaled by the
/// preset's line width): solid, then seven different rhythms.
pub const PAPER_DASHES: [&[f32]; 8] = [
    &[],
    &[6.0, 2.5],
    &[1.5, 2.0],
    &[7.0, 2.0, 1.5, 2.0],
    &[3.5, 3.5],
    &[10.0, 2.5],
    &[7.0, 2.0, 1.5, 2.0, 1.5, 2.0],
    &[3.0, 1.5],
];

/// The colours of the combs in the dark theme, in the order of the bodies: none of them a
/// tower's colour, and none the grey of the spectrum itself.
pub const COMB_COLOURS: [Colour; 4] =
    [Colour::hex(0x5a_b4ff), Colour::hex(0xd0_8ae0), Colour::hex(0x7f_e0b0), Colour::hex(0xf0_a0a0)];

/// The two shot-noise floors of the plan (VALIDATION §1), rad/√Hz: the baseline and the stretch.
pub const SHOT_NOISE_FLOORS: [f64; 2] = [1e-4, 1e-5];

/// The ULDM benchmark of the plan (VALIDATION §1): a scalar of mass 2×10⁻¹⁶ eV, whose field
/// oscillates at `m c²/h`, hertz.
pub const ULDM_FREQUENCY: f64 = 2e-16 * 1.602_176_634e-19 / 6.626_070_15e-34;

/// The amplitude of the benchmark's signal, radians: 10⁻² µrad.
pub const ULDM_AMPLITUDE: f64 = 1e-8;

/// Everything about how a plot looks. Sizes are points (1/72 inch).
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    /// The preset's name.
    pub name: &'static str,
    /// The width and height a figure in this style has unless told otherwise, millimetres.
    pub size_mm: [f64; 2],
    /// The resolution a PNG figure has unless told otherwise, pixels per inch.
    pub dpi: f64,
    /// Whether this is the paper's house style: floors and the ULDM mark on a periodogram, text
    /// in ink beside coloured marks, no hints about the mouse.
    pub house: bool,
    /// Behind everything.
    pub background: Colour,
    /// Titles.
    pub ink: Colour,
    /// Legends and notes.
    pub text: Colour,
    /// The numbers on the axes, units and captions.
    pub weak: Colour,
    /// The frame and the grid.
    pub rule: Colour,
    /// A spectrum's own line.
    pub spectrum: Colour,
    /// The playhead: one colour in every plot, and no series'.
    pub playhead: Colour,
    /// Reference levels: the shot-noise floors, the predicted floor.
    pub reference: Colour,
    /// The colour of each series (a tower's signal, a tower's flights).
    pub series: Vec<Colour>,
    /// The dash of each series, at this style's line width; empty for solid.
    pub dashes: Vec<Vec<f32>>,
    /// The colour of each body's comb.
    pub combs: Vec<Colour>,
    /// The font size of the title.
    pub title_size: f32,
    /// The font size of legends and notes.
    pub text_size: f32,
    /// The font size of the numbers on the axes and of captions.
    pub tick_size: f32,
    /// The font size of the labels by the stems.
    pub label_size: f32,
    /// The width of a series' line.
    pub line: f32,
    /// The width of the frame, the grid, stems and reference levels.
    pub rule_width: f32,
    /// The radius of a dot on a point or a stem.
    pub dot: f32,
    /// The room left of the axes, or `None` to take what the widest label needs.
    pub left: Option<f32>,
}

impl Style {
    /// The scale of the layout: 1 at the application's text size, and in proportion to the
    /// axes' numbers otherwise, so that margins grow with the text.
    #[must_use]
    pub fn unit(&self) -> f32 {
        self.tick_size / 10.5
    }

    /// The colour and the dash of series `n` (from 0). Past the last colour the colours come
    /// round again with the next dash, so that no two of the first 56 series look the same.
    #[must_use]
    pub fn series(&self, n: usize) -> (Colour, &[f32]) {
        let colours = self.series.len().max(1);
        let dash = (n + n / colours) % self.dashes.len().max(1);
        (
            self.series.get(n % colours).copied().unwrap_or(self.ink),
            self.dashes.get(dash).map_or(&[], Vec::as_slice),
        )
    }

    /// The colour of comb `n` (from 0).
    #[must_use]
    pub fn comb(&self, n: usize) -> Colour {
        self.combs.get(n % self.combs.len().max(1)).copied().unwrap_or(self.ink)
    }

    /// The colour of a text that names a mark of `colour`: the mark's own in the dark theme, ink
    /// in the house style (where a swatch beside the text carries the colour).
    #[must_use]
    pub const fn naming(&self, colour: Colour) -> Colour {
        if self.house { self.text } else { colour }
    }

    /// The application's dark theme: what the panes show.
    #[must_use]
    pub fn screen() -> Self {
        let colour = |c: egui::Color32| Colour([c.r(), c.g(), c.b()]);
        Self {
            name: "screen",
            size_mm: [960.0 * MM_PER_POINT, 600.0 * MM_PER_POINT],
            dpi: 72.0,
            house: false,
            background: colour(crate::theme::BACKGROUND),
            ink: colour(crate::theme::STRONG),
            text: colour(crate::theme::TEXT),
            weak: colour(crate::theme::WEAK),
            rule: colour(crate::theme::RULE),
            spectrum: Colour::hex(0x9a_a5ab),
            playhead: colour(crate::theme::PLAYHEAD),
            reference: colour(crate::theme::WEAK),
            // The renderer's tower colours, as far as they go before they come round again.
            series: (0..64)
                .map(cav_render::tower_srgb)
                .enumerate()
                .take_while(|(n, colour)| *n == 0 || *colour != cav_render::tower_srgb(0))
                .map(|(_, colour)| Colour(colour))
                .collect(),
            dashes: vec![Vec::new()],
            combs: COMB_COLOURS.to_vec(),
            title_size: 12.0,
            text_size: 10.5,
            tick_size: 10.5,
            label_size: 10.0,
            line: 1.3,
            rule_width: 1.0,
            dot: 2.5,
            left: Some(86.0),
        }
    }

    /// A light style: white, near-black ink, grey rules, the palette and its dashes. `tick` is
    /// the font size of the axes' numbers and `line` the width of a series' line.
    fn light(name: &'static str, size_mm: [f64; 2], dpi: f64, tick: f32, line: f32) -> Self {
        Self {
            name,
            size_mm,
            dpi,
            house: true,
            background: Colour::hex(0xff_ffff),
            ink: Colour::hex(0x11_1111),
            text: Colour::hex(0x22_2222),
            weak: Colour::hex(0x55_5555),
            rule: Colour::hex(0xcf_cfcf),
            spectrum: Colour::hex(0x6e_6e6e),
            playhead: Colour::hex(0x11_1111),
            reference: Colour::hex(0x8c_8c8c),
            series: PAPER_SERIES.to_vec(),
            dashes: PAPER_DASHES.iter().map(|dash| dash.iter().map(|d| d * line).collect()).collect(),
            combs: PAPER_SERIES.to_vec(),
            title_size: tick * 1.15,
            text_size: tick,
            tick_size: tick,
            label_size: tick * 0.95,
            line,
            rule_width: line * 0.6,
            dot: line * 1.9,
            left: None,
        }
    }

    /// The preset called `name`, or why not: `paper-column` (85 mm wide), `paper-full`
    /// (178 mm), `slide` (254 × 143 mm), `poster` (400 mm wide) or `screen`.
    ///
    /// # Errors
    /// A name that is not a preset's, with the names that are.
    pub fn preset(name: &str) -> Result<Self, String> {
        match name {
            "paper-column" => Ok(Self::light("paper-column", [85.0, 60.0], 600.0, 7.0, 0.8)),
            "paper-full" => Ok(Self::light("paper-full", [178.0, 90.0], 300.0, 8.0, 0.9)),
            "slide" => Ok(Self::light("slide", [254.0, 143.0], 150.0, 15.0, 1.8)),
            "poster" => Ok(Self::light("poster", [400.0, 250.0], 150.0, 24.0, 3.0)),
            "screen" => Ok(Self::screen()),
            other => Err(format!("`{other}` is not a figure preset; the presets are {}", PRESETS.join(", "))),
        }
    }
}

/// The names of the presets.
pub const PRESETS: [&str; 5] = ["paper-column", "paper-full", "slide", "poster", "screen"];
