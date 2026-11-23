//! The two faces everything is written in, and their licence.
//!
//! Ubuntu Light is the proportional face (egui bundles it); Ubuntu Mono is the monospace face
//! (bundled here, `assets/fonts/`). Both are under the Ubuntu Font Licence 1.0, built into the
//! binary whole as [`MONO_FONT_LICENCE`]. No system font is ever used, so a picture is the same
//! on every machine.

use egui::FontFamily;

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

#[cfg(test)]
mod tests {
    use super::*;
    use egui::FontId;

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
