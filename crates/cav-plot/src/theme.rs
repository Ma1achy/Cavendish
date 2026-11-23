//! The colours of the application's dark theme that the plots share with it.

use egui::Color32;

/// Behind everything: the plots' and the panes' background.
pub const BACKGROUND: Color32 = Color32::from_rgb(0x0a, 0x0a, 0x0a);
/// Frames, grid lines and separators.
pub const RULE: Color32 = Color32::from_rgb(0x2a, 0x2a, 0x2a);
/// Links, and the first comb.
pub const LINK: Color32 = Color32::from_rgb(0x5a, 0xb4, 0xff);
/// Ordinary text.
pub const TEXT: Color32 = Color32::from_rgb(0xa8, 0xa8, 0xa8);
/// Titles and what is selected.
pub const STRONG: Color32 = Color32::from_rgb(0xdc, 0xdc, 0xdc);
/// Captions and the numbers on axes.
pub const WEAK: Color32 = Color32::from_rgb(0x6f, 0x6f, 0x6f);
/// The playhead, in the timeline and in every plot: one colour, and no series'.
pub const PLAYHEAD: Color32 = STRONG;

/// A tower's colour, as the renderer draws it.
#[must_use]
pub fn tower(n: usize) -> Color32 {
    let [r, g, b] = cav_render::tower_srgb(n);
    Color32::from_rgb(r, g, b)
}
