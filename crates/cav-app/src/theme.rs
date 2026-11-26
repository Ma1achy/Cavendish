//! The look: egui dark with the mock-up's palette (`docs/viewer-mockup/scene-viewer.html`).

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};

pub use cav_plot::fonts::{MONO_FONT, MONO_FONT_LICENCE, MONO_FONT_NAME, definitions as fonts};
pub use cav_plot::theme::{BACKGROUND, LINK, PLAYHEAD, RULE, STRONG, TEXT, tower};

/// Secondary text in the window: labels, hints, units. sRGB. Lighter than the figures' own weak
/// grey (`cav_plot::theme::WEAK`, #6f6f6f), which on a card read at 3.2:1: the design asks at
/// least 4.5:1 of every text against its ground (S19, `docs/design/panels.md`); this is 4.7:1
/// on a card and 5.1:1 on a panel ([`contrast`]).
pub const WEAK: Color32 = Color32::from_rgb(0x8c, 0x8c, 0x8c);

/// Secondary text on a button's or a field's ground ([`WIDGET`], [`HOVER`]): the tool strip's key
/// letters, a field's hint, a material's density in its drop-down. [`WEAK`] there read at 3.3:1
/// (S19's review, I6); this is at least 4.5:1 on both ([`text_pairs`]).
pub const WEAK_ON_WIDGET: Color32 = Color32::from_rgb(0xbd, 0xbd, 0xbd);

/// The side of anything a pointer aims at in the side panels, points: a row, a box, a tick, a
/// drop-down, an eye, a lock, a ⋮, a `+` (`docs/design/workspace.md` §0: desktop rows 20 pt; the
/// owner's ruling of 25 Nov 2026 withdrew `panels.md`'s 24 pt on desktop). Not the touch rule
/// (44 pt), which is the phone and tablet layout's.
pub const HIT: f32 = 20.0;

/// The side of an icon button, points: the tool strip's, a viewport header's, the transport's
/// (the design's 20 to 22). Larger than a row ([`HIT`]) so that icons read at a glance.
pub const ICON: f32 = 22.0;

/// The spacing grid, points: margins inside a panel and a card, and the gap between items and
/// between cards, are this or a whole multiple of it.
pub const GRID: f32 = 4.0;

/// The gap between two rows of a card, points: half a step of [`GRID`], so that rows of [`HIT`]
/// stand at a pitch of 22 pt (the design asks for no more than 24).
pub const ROW_GAP: f32 = 2.0;

/// The height of a card's header and of a panel's tab, points.
pub const HEADER: f32 = 22.0;

/// The size of body and button text, points.
pub const BODY_PT: f32 = 12.0;
/// The size of small text (units, hints, captions), points.
pub const SMALL_PT: f32 = 11.0;
/// The size of monospace text (numbers), points.
pub const MONO_PT: f32 = 12.0;

/// A font for numbers: monospace at [`MONO_PT`], so that digits stand in columns.
#[must_use]
pub fn mono() -> FontId {
    FontId::monospace(MONO_PT)
}

/// A font for names and words: proportional at [`BODY_PT`].
#[must_use]
pub fn body() -> FontId {
    FontId::proportional(BODY_PT)
}

/// The side panels' own spacing on `ui` and what is laid out in it after: every control
/// [`HIT`] high and at least as wide (egui's least width of a box is 40), items [`GRID`] apart
/// along a row and rows `row_gap` apart ([`ROW_GAP`] in the inspector, 0 in the tree, whose rows
/// touch).
pub fn dense(ui: &mut egui::Ui, row_gap: f32) {
    let spacing = ui.spacing_mut();
    spacing.interact_size = egui::vec2(HIT, HIT);
    spacing.item_spacing = egui::vec2(GRID, row_gap);
}

/// Every (text, ground) pair the window's panels draw, by name: what the contrast test holds to
/// 4.5:1 (`docs/design/panels.md`). A new colour of text, or a text on a new ground, is added
/// here where it is first drawn.
#[must_use]
pub fn text_pairs() -> Vec<(&'static str, Color32, &'static str, Color32)> {
    let card = crate::inspector::CARD;
    let mut pairs = Vec::new();
    for (ground_name, ground) in [("panel", PANEL), ("card", card), ("status line", FOOTER)] {
        for (text_name, text) in [
            ("text", TEXT),
            ("weak", WEAK),
            ("strong", STRONG),
            ("gold", GOLD),
            ("warn", WARN),
            ("error", ERROR),
        ] {
            pairs.push((text_name, text, ground_name, ground));
        }
    }
    for (ground_name, ground) in [("button", WIDGET), ("button under the pointer", HOVER)] {
        // A button's own text is [`STRONG`] (white under the pointer): egui's widget colours.
        for (text_name, text) in [("strong", STRONG), ("weak on a button", WEAK_ON_WIDGET)] {
            pairs.push((text_name, text, ground_name, ground));
        }
    }
    // A selected row's name, and a field's text and hint on the field's own ground.
    pairs.push(("strong", STRONG, "selection", SELECTION));
    for (text_name, text) in [("text", TEXT), ("strong", STRONG), ("hint", WEAK_ON_WIDGET)] {
        pairs.push((text_name, text, "field", BACKGROUND));
    }
    pairs
}

/// The contrast ratio of two sRGB colours, WCAG 2's (1 to 21): `(L₁ + 0.05) / (L₂ + 0.05)` of
/// their relative luminances, the lighter over the darker.
#[must_use]
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let luminance = |c: Color32| {
        let channel = |v: u8| {
            let v = f32::from(v) / 255.0;
            if v <= 0.040_45 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
    };
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

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
            (TextStyle::Small, FontId::new(SMALL_PT, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(BODY_PT, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(BODY_PT, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(MONO_PT, FontFamily::Monospace)),
        ]
        .into();
        // A number that is dragged or typed is set in the monospace face (the design's §0).
        style.drag_value_text_style = TextStyle::Monospace;
        style.spacing.item_spacing = egui::vec2(GRID, GRID);
        style.spacing.button_padding = egui::vec2(6.0, 2.0);
        style.spacing.interact_size.y = HIT;

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
        // egui's own weak text (a field's hint) is drawn on a field's or a button's ground.
        visuals.weak_text_color = Some(WEAK_ON_WIDGET);
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

/// Adds `widget` in a box of `size` points, at the box's right and centred down it: a column of
/// numbers whose last digits stand under one another.
pub fn fixed_right(ui: &mut egui::Ui, size: egui::Vec2, widget: impl egui::Widget) -> egui::Response {
    ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.set_min_size(size);
        ui.set_max_width(size.x);
        ui.add(widget)
    })
    .inner
}

/// Adds `widget` in a box of `size` points, at the box's left and centred down it: a column of
/// names that reads down its left edge, where `Ui::add_sized` would centre each in its box.
pub fn fixed_left(ui: &mut egui::Ui, size: egui::Vec2, widget: impl egui::Widget) -> egui::Response {
    ui.allocate_ui_with_layout(size, egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_min_size(size);
        ui.set_max_width(size.x);
        ui.add(widget)
    })
    .inner
}
