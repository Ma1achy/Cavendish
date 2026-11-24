//! How a plot looks: every colour, font size and line width in one value, so that the
//! application's dark theme and the presets for papers, slides and posters are the same drawing
//! code with different styles.
//!
//! The light presets' series colours ([`PAPER_SERIES`]) are five hues, told apart in colour and
//! by a dash of their own ([`PAPER_DASHES`]); a sixth series and after take the hues again with
//! dashes no earlier series has. Five, not eight, because no eight colours were found that stay
//! at least 12 apart in CIEDE2000 under every simulation of colour-blindness the test holds them
//! to (S2.md §7.14), and five do, with room (S2's second review, S3.md §3.2 item 1): normal vision
//! and seven simulations of protanopia, deuteranopia and tritanopia (those of Machado, Oliveira
//! and Fernandes 2009, and of Viénot, Brettel and Mollon 1999 and 1997); the `dataviz` skill's
//! validator passes every check on white. The first four are at least 8 L* apart, so that a
//! figure printed in black and white still tells them apart
//! (`the_palette_survives_colour_blindness_and_print`).
use crate::drawing::Colour;

/// Millimetres in a point (1/72 inch).
pub const MM_PER_POINT: f64 = 25.4 / 72.0;

/// The series colours of the light presets, in order: blue, crimson, teal, orange, purple. Their
/// lightness rises 33, 42, 52, 60 L* through the first four, so that they stay apart in grey;
/// the fifth is dark again. Searched under the test's criterion with blue and orange kept from
/// the reviewer's five (`#0b0dff`, `#e76f11`) and every hue at least 40° from the others: the
/// reviewer's five are 17.49 apart at worst under every simulation but three of them are within
/// 2 L* of each other, so they fail the grey-print rule. Assigned in order and never cycled
/// alone: past the fifth they come round with a new dash ([`Style::series`]).
pub const PAPER_SERIES: [Colour; 5] = [
    Colour::hex(0x0b_0dff),
    Colour::hex(0xc0_1850),
    Colour::hex(0x10_86a6),
    Colour::hex(0xe7_6f11),
    Colour::hex(0x70_3088),
];
/// The dash of each series in the light presets, points at a line width of one (scaled by the
/// preset's line width): solid, then seven different rhythms. Series 6 to 8 share a hue with
/// series 1 to 3 and are told apart from them by these alone, so every drawing of a series keeps
/// its dash: the line, the envelope of a dense series and the key.
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

    /// The colour and the dash of series `n` (from 0). The colours come round every
    /// `series.len()` and the dashes every `dashes.len()`: in the light presets (five colours,
    /// eight dashes) the first eight series have eight different dashes and no two of the first
    /// forty look the same.
    #[must_use]
    pub fn series(&self, n: usize) -> (Colour, &[f32]) {
        let colours = self.series.len().max(1);
        let dashes = self.dashes.len().max(1);
        let dash = n % dashes;
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

#[cfg(test)]
mod tests {
    use super::*;

    type Lab = [f64; 3];
    type Matrix = [[f64; 3]; 3];

    /// The transforms of Machado, Oliveira and Fernandes, "A physiologically-based model for
    /// simulation of color vision deficiency", IEEE TVCG 15(6), 2009, at severity 1.0
    /// (dichromacy), applied to linear RGB: protanopia, deuteranopia, tritanopia. The same
    /// matrices as the `dataviz` skill's validator.
    const MACHADO: [(&str, Matrix); 3] = [
        (
            "protanopia",
            [
                [0.152_286, 1.052_583, -0.204_868],
                [0.114_503, 0.786_281, 0.099_216],
                [-0.003_882, -0.048_116, 1.051_998],
            ],
        ),
        (
            "deuteranopia",
            [
                [0.367_322, 0.860_646, -0.227_968],
                [0.280_085, 0.672_501, 0.047_413],
                [-0.011_820, 0.042_940, 0.968_881],
            ],
        ),
        (
            "tritanopia",
            [
                [1.255_528, -0.076_749, -0.178_779],
                [-0.078_411, 0.930_809, 0.147_602],
                [0.004_733, 0.691_367, 0.303_900],
            ],
        ),
    ];

    /// sRGB bytes as linear RGB (IEC 61966-2-1).
    fn linear(colour: Colour) -> [f64; 3] {
        colour.0.map(|byte| {
            let c = f64::from(byte) / 255.0;
            if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        })
    }

    /// Linear sRGB as CIE L*a*b* (D65).
    fn lab([r, g, b]: [f64; 3]) -> Lab {
        let x = 0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b;
        let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b;
        let z = 0.019_333_9 * r + 0.119_192_0 * g + 0.950_304_1 * b;
        let f = |t: f64| if t > 216.0 / 24389.0 { t.cbrt() } else { (24389.0 / 27.0 * t + 16.0) / 116.0 };
        let (fx, fy, fz) = (f(x / 0.950_47), f(y), f(z / 1.088_83));
        [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
    }

    /// Linear sRGB as OKLab × 100 (Ottosson 2020): the `dataviz` skill's own measure.
    fn oklab([r, g, b]: [f64; 3]) -> Lab {
        let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
        let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
        let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
        [
            100.0 * (0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s),
            100.0 * (1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s),
            100.0 * (0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s),
        ]
    }

    fn euclid(a: Lab, b: Lab) -> f64 {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    }

    /// CIEDE2000 (Sharma, Wu and Dalal, "The CIEDE2000 color-difference formula", Color Research
    /// and Application 30(1), 2005), with unit weights.
    fn ciede2000([l1, a1, b1]: Lab, [l2, a2, b2]: Lab) -> f64 {
        let seventh = |c: f64| (c.powi(7) / (c.powi(7) + 25f64.powi(7))).sqrt();
        let mean_c = (a1.hypot(b1) + a2.hypot(b2)) / 2.0;
        let g = 0.5 * (1.0 - seventh(mean_c));
        let (a1, a2) = ((1.0 + g) * a1, (1.0 + g) * a2);
        let (c1, c2) = (a1.hypot(b1), a2.hypot(b2));
        let hue =
            |a: f64, b: f64, c: f64| if c == 0.0 { 0.0 } else { b.atan2(a).to_degrees().rem_euclid(360.0) };
        let (h1, h2) = (hue(a1, b1, c1), hue(a2, b2, c2));
        let (dl, dc) = (l2 - l1, c2 - c1);
        let turn = if c1 * c2 == 0.0 {
            0.0
        } else if (h2 - h1).abs() <= 180.0 {
            h2 - h1
        } else if h2 - h1 > 180.0 {
            h2 - h1 - 360.0
        } else {
            h2 - h1 + 360.0
        };
        let dh = 2.0 * (c1 * c2).sqrt() * (turn / 2.0).to_radians().sin();
        let (l, c) = (f64::midpoint(l1, l2), f64::midpoint(c1, c2));
        let h = if c1 * c2 == 0.0 {
            h1 + h2
        } else if (h1 - h2).abs() <= 180.0 {
            f64::midpoint(h1, h2)
        } else if h1 + h2 < 360.0 {
            (h1 + h2 + 360.0) / 2.0
        } else {
            (h1 + h2 - 360.0) / 2.0
        };
        let t = 1.0 - 0.17 * (h - 30.0).to_radians().cos()
            + 0.24 * (2.0 * h).to_radians().cos()
            + 0.32 * (3.0 * h + 6.0).to_radians().cos()
            - 0.20 * (4.0 * h - 63.0).to_radians().cos();
        let swing = 30.0 * (-((h - 275.0) / 25.0).powi(2)).exp();
        let rotation = -(2.0 * swing).to_radians().sin() * 2.0 * seventh(c);
        let sl = 1.0 + 0.015 * (l - 50.0).powi(2) / (20.0 + (l - 50.0).powi(2)).sqrt();
        let (sc, sh) = (1.0 + 0.045 * c, 1.0 + 0.015 * c * t);
        ((dl / sl).powi(2) + (dc / sc).powi(2) + (dh / sh).powi(2) + rotation * (dc / sc) * (dh / sh)).sqrt()
    }

    /// Viénot, Brettel and Mollon, "Digital video colourmaps for checking the legibility of
    /// displays by dichromats", Color Research and Application 24(4), 1999: linear RGB to LMS,
    /// and the cone a protanope and a deuteranope lack written from the other two. The same
    /// numbers as `scripts/palette_search.py`, which searched the palette under them.
    const VIENOT_LMS: Matrix =
        [[17.8824, 43.5161, 4.11935], [3.45565, 27.1554, 3.86714], [0.029_956_6, 0.184_309, 1.46709]];
    const VIENOT_RULES: [(&str, Matrix); 2] = [
        ("Viénot protanopia", [[0.0, 2.02344, -2.52581], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
        ("Viénot deuteranopia", [[1.0, 0.0, 0.0], [0.494_207, 0.0, 1.24827], [0.0, 0.0, 1.0]]),
    ];

    /// Brettel, Viénot and Mollon, "Computerized simulation of color appearance for dichromats",
    /// JOSA A 14(10), 1997: tritanopia as two half-planes, written as two matrices on linear RGB
    /// and the normal of the plane between them (the values of libDaltonLens, N. Burrus).
    const BRETTEL_TRITAN: (Matrix, Matrix, [f64; 3]) = (
        [[1.01277, 0.13548, -0.14826], [-0.01243, 0.86812, 0.14431], [0.07589, -0.80500, 0.72911]],
        [[0.93678, 0.18979, -0.12657], [0.06154, 0.81526, 0.12320], [-0.37562, 1.12767, 0.24796]],
        [0.03901, -0.02788, -0.01113],
    );

    fn product(a: &Matrix, b: &Matrix) -> Matrix {
        std::array::from_fn(|r| std::array::from_fn(|c| (0..3).map(|k| a[r][k] * b[k][c]).sum()))
    }

    fn inverse(m: &Matrix) -> Matrix {
        let cofactor = |r: usize, c: usize| {
            let (r1, r2, c1, c2) = ((r + 1) % 3, (r + 2) % 3, (c + 1) % 3, (c + 2) % 3);
            m[r1][c1] * m[r2][c2] - m[r1][c2] * m[r2][c1]
        };
        let det: f64 = (0..3).map(|c| m[0][c] * cofactor(0, c)).sum();
        std::array::from_fn(|r| std::array::from_fn(|c| cofactor(c, r) / det))
    }

    fn apply(m: &Matrix, rgb: [f64; 3]) -> [f64; 3] {
        [0, 1, 2].map(|row| (m[row][0] * rgb[0] + m[row][1] * rgb[1] + m[row][2] * rgb[2]).clamp(0.0, 1.0))
    }

    /// A condition of sight: its name and what it does to linear RGB.
    type Vision = (&'static str, Box<dyn Fn([f64; 3]) -> [f64; 3]>);

    /// The seven conditions a palette is held under: normal vision; Machado's protanopia,
    /// deuteranopia and tritanopia; Viénot's protanopia and deuteranopia; Brettel's tritanopia.
    /// Each is a name and what it does to linear RGB.
    fn visions() -> Vec<Vision> {
        let mut out: Vec<Vision> = vec![("normal vision", Box::new(|rgb| rgb))];
        for (name, m) in MACHADO {
            out.push((name, Box::new(move |rgb| apply(&m, rgb))));
        }
        for (name, rule) in VIENOT_RULES {
            let m = product(&inverse(&VIENOT_LMS), &product(&rule, &VIENOT_LMS));
            out.push((name, Box::new(move |rgb| apply(&m, rgb))));
        }
        out.push((
            "Brettel tritanopia",
            Box::new(|rgb| {
                let (first, second, normal) = BRETTEL_TRITAN;
                let side = normal[0] * rgb[0] + normal[1] * rgb[1] + normal[2] * rgb[2];
                apply(if side >= 0.0 { &first } else { &second }, rgb)
            }),
        ));
        out
    }

    /// The least difference between any two of `colours`, by `measure`, under each of the seven
    /// conditions of [`visions`]: the vision's name, the difference and the pair (from 1).
    fn least(
        colours: &[Colour],
        space: fn([f64; 3]) -> Lab,
        measure: fn(Lab, Lab) -> f64,
    ) -> Vec<(&'static str, f64, (usize, usize))> {
        visions()
            .iter()
            .map(|(name, vision)| {
                let seen: Vec<Lab> = colours.iter().map(|c| space(vision(linear(*c)))).collect();
                let mut worst = (f64::INFINITY, (0, 0));
                for a in 0..seen.len() {
                    for b in a + 1..seen.len() {
                        let apart = measure(seen[a], seen[b]);
                        if apart < worst.0 {
                            worst = (apart, (a + 1, b + 1));
                        }
                    }
                }
                (*name, worst.0, worst.1)
            })
            .collect()
    }

    #[test]
    fn the_palette_survives_colour_blindness_and_print() {
        // The measure itself, against published check pairs of Sharma et al. (their table 1,
        // pairs 1, 17 and 25), before it is trusted with ours.
        for (a, b, expected) in [
            ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
            ([50.0, 2.5, 0.0], [73.0, 25.0, -18.0], 27.1492),
            ([60.2574, -34.0099, 36.2677], [60.4626, -34.1751, 39.4387], 1.2644),
        ] {
            assert!((ciede2000(a, b) - expected).abs() < 1e-4, "{}", ciede2000(a, b));
        }
        // The simulation itself: Viénot's protanopia sees the old palette's blue and purple
        // 10.37 apart, as the review measured (finding 28), to the hundredth.
        let old = [Colour::hex(0x0b_0dff), Colour::hex(0x6d_1ba7)];
        let vienot = least(&old, lab, ciede2000);
        assert_eq!(vienot[4].0, "Viénot protanopia");
        assert!((vienot[4].1 - 10.37).abs() < 0.005, "{vienot:?}");
        // The criterion (S2.md §3.3, held as first written: S3.md §3.2 item 1): every pair of the
        // hues at least 12 apart in CIEDE2000, in normal vision and under the seven simulated
        // dichromacies of Machado and of Viénot and Brettel; in the `dataviz` skill's own measure
        // (OKLab × 100) at least its floors, 15 in normal vision and 8 under Machado's three.
        let floors = |measure: &str, vision: &str| match (measure, vision) {
            ("CIEDE2000", _) => Some(12.0),
            (_, "normal vision") => Some(15.0),
            // Machado's three are named by the deficiency alone.
            (_, "protanopia" | "deuteranopia" | "tritanopia") => Some(8.0),
            _ => None,
        };
        let table = |colours: &[Colour]| {
            let mut rows = Vec::new();
            for (measure, table) in [
                ("CIEDE2000", least(colours, lab, ciede2000)),
                ("OKLab × 100", least(colours, oklab, euclid)),
            ] {
                for (vision, apart, pair) in table {
                    rows.push((measure, vision, apart, pair));
                }
            }
            rows
        };
        for (measure, vision, apart, pair) in table(&PAPER_SERIES) {
            println!("{measure:>12} · {vision:<20} least ΔE {apart:6.2} (series {} and {})", pair.0, pair.1);
            if let Some(floor) = floors(measure, vision) {
                assert!(apart >= floor, "{measure}, {vision}: series {pair:?} are {apart:.2} apart");
            }
        }
        // Controls: the eight-colour palettes this replaces fail the same rule: S2's first under
        // Viénot's protanopia (10.37), and the one S2's review left in place (11.41 at worst).
        for before in [
            [0x0b_0dff, 0xe7_6f11, 0xc6_1604, 0x8a_6aae, 0x28_a682, 0xcb_64fe, 0x91_0c59, 0x6d_1ba7],
            [0x90_0c60, 0x2c_84b2, 0x84_6000, 0xa8_78fc, 0x3a_469e, 0x82_00f2, 0xca_4076, 0xc0_8400],
        ] {
            let failed: Vec<_> = table(&before.map(Colour::hex))
                .into_iter()
                .filter(|(measure, vision, apart, _)| {
                    *measure == "CIEDE2000" && floors(measure, vision).is_some_and(|floor| *apart < floor)
                })
                .collect();
            assert!(!failed.is_empty(), "{before:x?} passes, and should not");
        } // The first four survive grey print: at least 8 L* between any two.
        let lightness: Vec<f64> = PAPER_SERIES.iter().map(|c| lab(linear(*c))[0]).collect();
        println!("L*: {:?}", lightness.iter().map(|l| (l * 10.0).round() / 10.0).collect::<Vec<_>>());
        for a in 0..4 {
            for b in a + 1..4 {
                let apart = (lightness[a] - lightness[b]).abs();
                assert!(apart >= 8.0, "series {} and {} are {apart:.1} L* apart", a + 1, b + 1);
            }
        }
        // And each of the first eight series has a dash of its own, for when there is no colour
        // at all, and a hue and a dash together that no other of the first forty has; each
        // mark is at least 3:1 against the white it is drawn on (WCAG's relative luminance).
        let paper = Style::preset("paper-column").unwrap();
        for a in 0..40 {
            for b in a + 1..40 {
                if b < 8 {
                    assert_ne!(
                        paper.series(a).1,
                        paper.series(b).1,
                        "series {} and {} share a dash",
                        a + 1,
                        b + 1
                    );
                }
                assert_ne!(paper.series(a), paper.series(b), "series {} and {} look the same", a + 1, b + 1);
            }
        }
        for colour in PAPER_SERIES {
            let [r, g, b] = linear(colour);
            let contrast = 1.05 / (0.2126 * r + 0.7152 * g + 0.0722 * b + 0.05);
            assert!(contrast >= 3.0, "{colour:?} is {contrast:.2}:1 on white");
        }
        // A sixth series is not the first again: the hue comes round with another dash.
        assert_ne!(paper.series(5), paper.series(0));
        assert_eq!(paper.series(5).0, paper.series(0).0); // Control: a plain red and green (matplotlib's tab:red and tab:green) are far apart to
        // normal eyes and fail under deuteranopia; and two greys 5 L* apart fail the print rule.
        let naive = least(&[Colour::hex(0xd6_2728), Colour::hex(0x2c_a02c)], lab, ciede2000);
        assert!(naive[0].1 > 50.0, "{naive:?}");
        assert!(
            naive.iter().any(|(vision, apart, _)| *vision == "deuteranopia" && *apart < 12.0),
            "{naive:?}"
        );
        let greys = [lab(linear(Colour::hex(0x70_7070)))[0], lab(linear(Colour::hex(0x7c_7c7c)))[0]];
        assert!((greys[0] - greys[1]).abs() < 8.0);
    }

    /// L* of the colour map `map` at `samples + 1` evenly spaced points, under each vision.
    fn lightness_along(
        map: &dyn Fn(f64) -> [u8; 3],
        from: f64,
        to: f64,
        samples: usize,
    ) -> Vec<(&'static str, Vec<f64>)> {
        visions()
            .iter()
            .map(|(name, vision)| {
                let l = (0..=samples)
                    .map(|k| {
                        let t = from + (to - from) * k as f64 / samples as f64;
                        lab(vision(linear(Colour(map(t)))))[0]
                    })
                    .collect();
                (*name, l)
            })
            .collect()
    }

    /// Whether `values` rise strictly from first to last.
    fn rising(values: &[f64]) -> bool {
        values.windows(2).all(|w| w[1] > w[0])
    }

    #[test]
    fn the_colour_maps_survive_colour_blindness_and_print() {
        use cav_render::colour::{diverging, sequential};
        // The criteria (S3.md R10, written before this was run): under all seven conditions of
        // `visions`, (1) the sequential map's L* is strictly monotone from one end to the other,
        // on paper and on the dark surface, so its order survives grey print and colour-blind
        // sight; (2) each arm of the diverging map is strictly monotone in L* away from its
        // middle, on paper and on the dark surface; (3) its two ends, −1 and +1, are at least 12
        // apart in CIEDE2000, so the sign is not lost.
        for dark in [false, true] {
            for (vision, l) in lightness_along(&|t| sequential(t, dark), 0.0, 1.0, 48) {
                let ok = if dark { rising(&l) } else { rising(&l.iter().rev().copied().collect::<Vec<_>>()) };
                assert!(ok, "sequential, dark {dark}, {vision}: L* {l:?}");
            }
            for (from, to) in [(0.0, 1.0), (0.0, -1.0)] {
                for (vision, l) in lightness_along(&|t| diverging(t, dark), from, to, 24) {
                    let ok =
                        if dark { rising(&l) } else { rising(&l.iter().rev().copied().collect::<Vec<_>>()) };
                    assert!(ok, "diverging arm to {to}, dark {dark}, {vision}: L* {l:?}");
                }
            }
            for (vision, see) in visions() {
                let ends = [diverging(-1.0, dark), diverging(1.0, dark)].map(|c| lab(see(linear(Colour(c)))));
                let apart = ciede2000(ends[0], ends[1]);
                println!("diverging ends, dark {dark}, {vision}: {apart:.2}");
                assert!(apart >= 12.0, "diverging ends, dark {dark}, {vision}: {apart:.2}");
            }
        }
        // Control: a rainbow (red, green, blue) is not monotone in L*, and a map whose ends are
        // red and green loses its sign under deuteranopia.
        let rainbow = |t: f64| {
            let stops = [[0xd6u8, 0x27, 0x28], [0x2c, 0xa0, 0x2c], [0x1f, 0x77, 0xb4]];
            stops[((t * 2.0).round() as usize).min(2)]
        };
        assert!(
            lightness_along(&rainbow, 0.0, 1.0, 2)
                .iter()
                .any(|(_, l)| !rising(l) && !rising(&l.iter().rev().copied().collect::<Vec<_>>()))
        );
        let red_green = least(&[Colour::hex(0xd6_2728), Colour::hex(0x2c_a02c)], lab, ciede2000);
        assert!(red_green.iter().any(|(_, apart, _)| *apart < 12.0));
    }

    #[test]
    fn presets_are_the_stated_sizes() {
        let widths = [("paper-column", 85.0), ("paper-full", 178.0), ("slide", 254.0), ("poster", 400.0)];
        for (name, width) in widths {
            let style = Style::preset(name).unwrap();
            assert_eq!((style.name, style.size_mm[0]), (name, width));
            assert!(style.house && style.background == Colour::hex(0xff_ffff));
            assert!(style.size_mm[1] > 0.4 * width && style.size_mm[1] < width);
        }
        assert_eq!(Style::preset("slide").unwrap().size_mm, [254.0, 143.0]);
        assert!(PRESETS.iter().all(|name| Style::preset(name).is_ok()));
        // The screen preset is the application's theme, to the colour.
        let screen = Style::preset("screen").unwrap();
        assert!(!screen.house && screen.series.len() == 5 && screen.left == Some(86.0));
        assert_eq!(screen.series[1], Colour(cav_render::tower_srgb(1)));
        assert_eq!(screen.series(6).0, Colour(cav_render::tower_srgb(6)), "past the last, as the renderer");
        // The benchmark's frequency: 2e-16 eV over Planck's constant.
        assert!((ULDM_FREQUENCY - 0.048_359_8).abs() < 1e-6, "{ULDM_FREQUENCY}");
        // Control: a name that is no preset is refused, with the names that are.
        let why = Style::preset("paper").unwrap_err();
        assert!(why.contains("`paper` is not a figure preset") && why.contains("paper-column"), "{why}");
    }
}
