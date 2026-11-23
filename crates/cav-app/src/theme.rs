//! The look: egui dark with the mock-up's palette (`docs/viewer-mockup/scene-viewer.html`).

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};

pub use cav_plot::fonts::{MONO_FONT, MONO_FONT_LICENCE, MONO_FONT_NAME, definitions as fonts};
pub use cav_plot::theme::{BACKGROUND, LINK, PLAYHEAD, RULE, STRONG, TEXT, WEAK, tower};

/// The background of a panel. sRGB.
pub const PANEL: Color32 = Color32::from_rgb(0x1b, 0x1b, 0x1b);
/// The background of the status line. sRGB.
pub const FOOTER: Color32 = Color32::from_rgb(0x14, 0x14, 0x14);
/// The fill of a button or a field at rest. sRGB.
pub const WIDGET: Color32 = Color32::from_rgb(0x3c, 0x3c, 0x3c);
/// The outline of a button or a field. sRGB.
pub const WIDGET_EDGE: Color32 = Color32::from_rgb(0x4a, 0x4a, 0x4a);
/// The fill of a button or a field under the pointer. sRGB.
pub const HOVER: Color32 = Color32::from_rgb(0x46, 0x46, 0x46);
/// The fill of what is selected or switched on. sRGB.
pub const SELECTION: Color32 = Color32::from_rgb(0x00, 0x5c, 0x80);
/// The outline of what is selected. sRGB.
pub const SELECTION_EDGE: Color32 = Color32::from_rgb(0x1a, 0x7c, 0xa3);
/// A warning's text. sRGB.
pub const WARN: Color32 = Color32::from_rgb(0xe0, 0xc0, 0x70);
/// An error's text. sRGB.
pub const ERROR: Color32 = Color32::from_rgb(0xe0, 0x70, 0x5a);
/// The scene's name and other marks of the present state. sRGB.
pub const GOLD: Color32 = Color32::from_rgb(0xe7, 0xc4, 0x6a);

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
