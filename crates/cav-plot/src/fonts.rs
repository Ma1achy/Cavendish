//! The two faces everything is written in, and their licence.
//!
//! Ubuntu Light is the proportional face (egui bundles it); Ubuntu Mono is the monospace face
//! (bundled here, `assets/fonts/`). Both are under the Ubuntu Font Licence 1.0, built into the
//! binary whole as [`MONO_FONT_LICENCE`]. No system font is ever used, so a picture is the same
//! on every machine.

use egui::FontFamily;

pub use crate::drawing::Face;

/// Ubuntu Mono Regular, unmodified: the monospace face. Its licence is [`MONO_FONT_LICENCE`];
/// where the file came from is in `assets/fonts/SOURCE.md`.
pub const MONO_FONT: &[u8] = include_bytes!("../assets/fonts/UbuntuMono-Regular.ttf");

/// The Ubuntu Font Licence 1.0, whole: built into the binaries with the fonts it covers, so the
/// licence goes wherever they do. It covers the fonts only.
pub const MONO_FONT_LICENCE: &str = include_str!("../assets/fonts/UFL.txt");

/// The name the monospace face is registered under.
pub const MONO_FONT_NAME: &str = "Ubuntu Mono";

/// The name egui registers its proportional face under.
pub const SANS_FONT_NAME: &str = "Ubuntu-Light";

/// The fonts: egui's bundled Ubuntu Light for text and Ubuntu Mono for numbers, paths and the
/// console, with egui's Hack behind it for the signs Ubuntu Mono lacks.
#[must_use]
pub fn definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert(MONO_FONT_NAME.to_owned(), std::sync::Arc::new(egui::FontData::from_static(MONO_FONT)));
    fonts.families.entry(FontFamily::Monospace).or_default().insert(0, MONO_FONT_NAME.to_owned());
    fonts
}

/// The bytes of a face's font file.
#[must_use]
pub fn bytes(face: Face) -> &'static [u8] {
    static SANS: std::sync::OnceLock<std::sync::Arc<egui::FontData>> = std::sync::OnceLock::new();
    match face {
        Face::Mono => MONO_FONT,
        Face::Sans => SANS
            .get_or_init(|| {
                egui::FontDefinitions::default()
                    .font_data
                    .get(SANS_FONT_NAME)
                    .cloned()
                    .expect("egui is built with its default fonts, Ubuntu Light among them")
            })
            .bytes(),
    }
}

fn font(face: Face) -> skrifa::FontRef<'static> {
    skrifa::FontRef::new(bytes(face)).expect("the built-in fonts are fonts")
}

const fn other(face: Face) -> Face {
    match face {
        Face::Sans => Face::Mono,
        Face::Mono => Face::Sans,
    }
}

/// The face a character is drawn from when text is written in `face`, as outlines: `face`
/// itself if it has the character, else the other of the two. `None` when neither has it; the
/// face's "not defined" box is drawn then.
#[must_use]
pub fn face_of(c: char, face: Face) -> Option<Face> {
    use skrifa::MetadataProvider as _;
    [face, other(face)].into_iter().find(|f| font(*f).charmap().map(c).is_some())
}

/// One piece of an outline. Points are in points (1/72 inch), x to the right, y down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    /// Start a new contour here.
    Move([f32; 2]),
    /// A straight line to here.
    Line([f32; 2]),
    /// A cubic Bézier curve: two control points, then the end.
    Cubic([f32; 2], [f32; 2], [f32; 2]),
    /// Close the contour.
    Close,
}

/// A line of text as outlines, in a box of its own: the origin is the box's top left corner.
#[derive(Clone, Debug, PartialEq)]
pub struct Outline {
    /// The contours of every glyph, to be filled by the non-zero rule.
    pub segments: Vec<Segment>,
    /// The box's width and height, points: the sum of the advances, and the face's line height.
    pub size: [f32; 2],
}

/// What a glyph is drawn into: design units in, points out, y turned to point down.
struct Pen {
    segments: Vec<Segment>,
    scale: f32,
    origin: [f32; 2],
    at: [f32; 2],
}

impl Pen {
    fn point(&self, x: f32, y: f32) -> [f32; 2] {
        [self.origin[0] + x * self.scale, self.origin[1] - y * self.scale]
    }
}

impl skrifa::outline::OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.at = self.point(x, y);
        self.segments.push(Segment::Move(self.at));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.at = self.point(x, y);
        self.segments.push(Segment::Line(self.at));
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        // A quadratic segment is the cubic whose controls are two thirds of the way from each
        // end to the quadratic's control: the same curve, exactly.
        let (from, control, to) = (self.at, self.point(cx, cy), self.point(x, y));
        let towards = |end: [f32; 2]| {
            [end[0] + (control[0] - end[0]) * (2.0 / 3.0), end[1] + (control[1] - end[1]) * (2.0 / 3.0)]
        };
        self.segments.push(Segment::Cubic(towards(from), towards(to), to));
        self.at = to;
    }

    fn curve_to(&mut self, ax: f32, ay: f32, bx: f32, by: f32, x: f32, y: f32) {
        self.at = self.point(x, y);
        self.segments.push(Segment::Cubic(self.point(ax, ay), self.point(bx, by), self.at));
    }

    fn close(&mut self) {
        self.segments.push(Segment::Close);
    }
}

/// Lays `text` out in `face` at font size `size` points: each character's advance from the
/// font file, no kerning, no hinting. With `draw` the glyphs' contours are read as well.
fn lay_out(text: &str, face: Face, size: f32, draw: bool) -> Outline {
    use skrifa::MetadataProvider as _;
    use skrifa::instance::{LocationRef, Size};
    let fonts = [font(Face::Sans), font(Face::Mono)];
    let of = |f: Face| &fonts[usize::from(f == Face::Mono)];
    // The line's height and baseline are the face's own, whichever face a character comes from.
    let metrics = of(face).metrics(Size::unscaled(), LocationRef::default());
    let line = size / f32::from(metrics.units_per_em);
    let (ascent, height) =
        (metrics.ascent * line, (metrics.ascent - metrics.descent + metrics.leading) * line);
    let mut pen = Pen { segments: Vec::new(), scale: 1.0, origin: [0.0, ascent], at: [0.0, 0.0] };
    for c in text.chars() {
        let from = face_of(c, face).unwrap_or(face);
        let font = of(from);
        let glyph = font.charmap().map(c).unwrap_or(skrifa::GlyphId::NOTDEF);
        let scale = size / f32::from(font.metrics(Size::unscaled(), LocationRef::default()).units_per_em);
        pen.scale = scale;
        if draw && let Some(outline) = font.outline_glyphs().get(glyph) {
            let settings = skrifa::outline::DrawSettings::unhinted(Size::unscaled(), LocationRef::default());
            // A glyph that cannot be drawn is left out; its advance still stands.
            let _ = outline.draw(settings, &mut pen);
        }
        let advance = font
            .glyph_metrics(Size::unscaled(), LocationRef::default())
            .advance_width(glyph)
            .unwrap_or_default();
        pen.origin[0] += advance * scale;
    }
    Outline { segments: pen.segments, size: [pen.origin[0], height] }
}

/// `text` in `face` at font size `size` points, as outlines.
#[must_use]
pub fn outline(text: &str, face: Face, size: f32) -> Outline {
    lay_out(text, face, size, true)
}

/// The measure of text written as outlines: what [`outline`] takes, from the font files.
#[derive(Clone, Copy, Debug, Default)]
pub struct Outlines;

impl crate::drawing::Measure for Outlines {
    fn size(&self, text: &str, face: Face, size: f32) -> [f32; 2] {
        lay_out(text, face, size, false).size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::Measure as _;
    use egui::FontId;

    #[test]
    fn text_becomes_outlines_from_the_two_faces() {
        // Ubuntu Mono's advance is half an em whatever the character; the proportional face's is not.
        assert!((Outlines.size("0.1234567890", Face::Mono, 12.0)[0] - 72.0).abs() < 1e-3);
        let (narrow, wide) =
            (Outlines.size("iiii", Face::Sans, 12.0), Outlines.size("MMMM", Face::Sans, 12.0));
        assert!(wide[0] > 2.0 * narrow[0], "{narrow:?} {wide:?}");
        assert_eq!(narrow[1], wide[1], "the height is the line's");
        // An outline is closed contours inside its box (with a little room for overshoot).
        let drawn = outline("Ag", Face::Sans, 10.0);
        let moves = drawn.segments.iter().filter(|s| matches!(s, Segment::Move(_))).count();
        let closes = drawn.segments.iter().filter(|s| matches!(s, Segment::Close)).count();
        assert!(moves >= 3 && moves == closes, "{moves} contours, {closes} closed");
        for segment in &drawn.segments {
            let ends = match segment {
                Segment::Move(p) | Segment::Line(p) | Segment::Cubic(_, _, p) => *p,
                Segment::Close => continue,
            };
            assert!((-0.5..=drawn.size[0] + 0.5).contains(&ends[0]), "{ends:?}");
            assert!((-0.5..=drawn.size[1] + 0.5).contains(&ends[1]), "{ends:?}");
        }
        // Twice the size is twice the outline.
        assert!((outline("Ag", Face::Sans, 20.0).size[0] / drawn.size[0] - 2.0).abs() < 1e-5);
        // Every character the plots write is in one of the two faces; these come from the
        // other face than the one asked for.
        let written =
            "ΔΦ√S·µ₀δφ₁₂×|–-+=,.:()/'\"0123456789 abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
        for face in [Face::Sans, Face::Mono] {
            let absent: Vec<char> = written.chars().filter(|c| face_of(*c, face).is_none()).collect();
            assert!(absent.is_empty(), "{face:?} cannot write {absent:?}");
        }
        // Control: a character neither face has is found to be missing, and takes a box's room.
        assert_eq!(face_of('\u{4e2d}', Face::Sans), None);
        assert!(Outlines.size("\u{4e2d}", Face::Sans, 12.0)[0] > 0.0);
        let borrowed =
            |face: Face| -> String { written.chars().filter(|c| face_of(*c, face) != Some(face)).collect() };
        println!("from the other face: Sans {:?}, Mono {:?}", borrowed(Face::Sans), borrowed(Face::Mono));
    }

    /// The width of `text` in the monospace face at 12 points, in a context set up by `setup`.
    fn width(setup: impl Fn(&egui::Context), text: &str) -> f32 {
        let ctx = egui::Context::default();
        setup(&ctx);
        let mut wide = 0.0;
        // Fonts are installed at the start of a frame.
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                wide = ui
                    .painter()
                    .layout_no_wrap(text.to_owned(), FontId::monospace(12.0), egui::Color32::WHITE)
                    .size()
                    .x;
            });
            // Nothing is painted, so the font atlas has nowhere to go.
            output.textures_delta.clear();
        }
        wide
    }

    #[test]
    fn the_monospace_face_is_ubuntu_mono_with_its_licence() {
        // Ubuntu Mono's advance is half an em: twelve characters at 12 points are 72 points wide,
        // whatever the characters.
        for text in ["iiiiiiiiiiii", "MMMMMMMMMMMM", "0.1234567890"] {
            let wide = width(|ctx| ctx.set_fonts(definitions()), text);
            assert!((wide - 72.0).abs() < 0.75, "`{text}` is {wide} points wide");
        }
        // Control: egui's own monospace face (Hack, 0.6 em) is wider, so the test can tell them apart.
        let hack = width(|_| {}, "MMMMMMMMMMMM");
        assert!(hack > 80.0, "egui's default monospace is {hack} points wide");
        // The face comes first in its family; Hack stays behind it for signs it lacks.
        let fonts = definitions();
        let family = &fonts.families[&FontFamily::Monospace];
        assert_eq!(family[0], MONO_FONT_NAME);
        assert!(family.len() >= 2 && family.iter().any(|name| name == "Hack"), "{family:?}");
        // The file is a TrueType font, and the licence built in beside it is the Ubuntu Font Licence 1.0.
        assert_eq!(&MONO_FONT[..4], &[0, 1, 0, 0]);
        assert!(MONO_FONT_LICENCE.contains("UBUNTU FONT LICENCE Version 1.0"));
        assert!(MONO_FONT_LICENCE.len() > 4000, "the whole licence, not a line of it");
    }
}
