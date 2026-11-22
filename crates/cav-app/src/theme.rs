//! The look: egui dark with the mock-up's palette (`docs/viewer-mockup/scene-viewer.html`).

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};

pub const BACKGROUND: Color32 = Color32::from_rgb(0x0a, 0x0a, 0x0a);
pub const PANEL: Color32 = Color32::from_rgb(0x1b, 0x1b, 0x1b);
pub const FOOTER: Color32 = Color32::from_rgb(0x14, 0x14, 0x14);
pub const RULE: Color32 = Color32::from_rgb(0x2a, 0x2a, 0x2a);
pub const WIDGET: Color32 = Color32::from_rgb(0x3c, 0x3c, 0x3c);
pub const WIDGET_EDGE: Color32 = Color32::from_rgb(0x4a, 0x4a, 0x4a);
pub const HOVER: Color32 = Color32::from_rgb(0x46, 0x46, 0x46);
pub const SELECTION: Color32 = Color32::from_rgb(0x00, 0x5c, 0x80);
pub const SELECTION_EDGE: Color32 = Color32::from_rgb(0x1a, 0x7c, 0xa3);
pub const LINK: Color32 = Color32::from_rgb(0x5a, 0xb4, 0xff);
pub const WARN: Color32 = Color32::from_rgb(0xe0, 0xc0, 0x70);
pub const ERROR: Color32 = Color32::from_rgb(0xe0, 0x70, 0x5a);
pub const TEXT: Color32 = Color32::from_rgb(0xa8, 0xa8, 0xa8);
pub const STRONG: Color32 = Color32::from_rgb(0xdc, 0xdc, 0xdc);
pub const WEAK: Color32 = Color32::from_rgb(0x6f, 0x6f, 0x6f);
pub const GOLD: Color32 = Color32::from_rgb(0xe7, 0xc4, 0x6a);

/// A tower's colour, as the renderer draws it.
#[must_use]
pub fn tower(n: usize) -> Color32 {
    let [r, g, b] = cav_render::tower_srgb(n);
    Color32::from_rgb(r, g, b)
}

/// Ubuntu Mono Regular, unmodified: the monospace face. Its licence is [`MONO_FONT_LICENCE`];
/// where the file came from is in `assets/fonts/SOURCE.md`.
pub const MONO_FONT: &[u8] = include_bytes!("../assets/fonts/UbuntuMono-Regular.ttf");

/// The Ubuntu Font Licence 1.0, whole: built into the application with the font it covers, so the
/// licence goes wherever the binary does. It covers the font only.
pub const MONO_FONT_LICENCE: &str = include_str!("../assets/fonts/UFL.txt");

/// The name the monospace face is registered under.
pub const MONO_FONT_NAME: &str = "Ubuntu Mono";

/// The fonts: egui's bundled Ubuntu Light for text and Ubuntu Mono for numbers, paths and the
/// console, with egui's Hack behind it for the signs Ubuntu Mono lacks. No system fonts, so a
/// screenshot is the same on every machine.
#[must_use]
pub fn fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert(MONO_FONT_NAME.to_owned(), std::sync::Arc::new(egui::FontData::from_static(MONO_FONT)));
    fonts.families.entry(FontFamily::Monospace).or_default().insert(0, MONO_FONT_NAME.to_owned());
    fonts
}

/// Installs the fonts and their sizes, the colours and the spacing.
pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(15.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(12.0, FontFamily::Monospace)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(7.0, 2.0);
        style.spacing.interact_size.y = 20.0;

        let visuals = &mut style.visuals;
        visuals.dark_mode = true;
        visuals.panel_fill = PANEL;
        visuals.window_fill = PANEL;
        visuals.extreme_bg_color = BACKGROUND;
        visuals.faint_bg_color = Color32::from_rgb(0x24, 0x24, 0x24);
        visuals.window_stroke = Stroke::new(1.0, WIDGET);
        visuals.selection.bg_fill = SELECTION;
        visuals.selection.stroke = Stroke::new(1.0, STRONG);
        visuals.hyperlink_color = LINK;
        visuals.warn_fg_color = WARN;
        visuals.error_fg_color = ERROR;
        let radius = CornerRadius::same(2);
        visuals.window_corner_radius = CornerRadius::same(4);
        visuals.menu_corner_radius = CornerRadius::same(3);
        let widgets = &mut visuals.widgets;
        widgets.noninteractive.bg_fill = PANEL;
        widgets.noninteractive.bg_stroke = Stroke::new(1.0, RULE);
        widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        widgets.noninteractive.corner_radius = radius;
        widgets.inactive.bg_fill = WIDGET;
        widgets.inactive.weak_bg_fill = WIDGET;
        widgets.inactive.bg_stroke = Stroke::new(1.0, WIDGET_EDGE);
        widgets.inactive.fg_stroke = Stroke::new(1.0, STRONG);
        widgets.inactive.corner_radius = radius;
        widgets.hovered.bg_fill = HOVER;
        widgets.hovered.weak_bg_fill = HOVER;
        widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x6a, 0x6a, 0x6a));
        widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
        widgets.hovered.corner_radius = radius;
        widgets.active.bg_fill = SELECTION;
        widgets.active.weak_bg_fill = SELECTION;
        widgets.active.bg_stroke = Stroke::new(1.0, SELECTION_EDGE);
        widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
        widgets.active.corner_radius = radius;
        widgets.open.bg_fill = HOVER;
        widgets.open.weak_bg_fill = HOVER;
        widgets.open.corner_radius = radius;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The width of `text` in the monospace face at 12 points, in a context set up by `setup`.
    fn width(setup: impl Fn(&egui::Context), text: &str) -> f32 {
        let ctx = egui::Context::default();
        setup(&ctx);
        let mut wide = 0.0;
        // Fonts are installed at the start of a frame.
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                wide = ui.painter().layout_no_wrap(text.to_owned(), FontId::monospace(12.0), TEXT).size().x;
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
            let wide = width(apply, text);
            assert!((wide - 72.0).abs() < 0.75, "`{text}` is {wide} points wide");
        }
        // Control: egui's own monospace face (Hack, 0.6 em) is wider, so the test can tell them apart.
        let hack = width(|_| {}, "MMMMMMMMMMMM");
        assert!(hack > 80.0, "egui's default monospace is {hack} points wide");
        // The face comes first in its family; Hack stays behind it for signs it lacks.
        let fonts = fonts();
        let family = &fonts.families[&FontFamily::Monospace];
        assert_eq!(family[0], MONO_FONT_NAME);
        assert!(family.len() >= 2 && family.iter().any(|name| name == "Hack"), "{family:?}");
        // The file is a TrueType font, and the licence built in beside it is the Ubuntu Font Licence 1.0.
        assert_eq!(&MONO_FONT[..4], &[0, 1, 0, 0]);
        assert!(MONO_FONT_LICENCE.contains("UBUNTU FONT LICENCE Version 1.0"));
        assert!(MONO_FONT_LICENCE.len() > 4000, "the whole licence, not a line of it");
    }
}
