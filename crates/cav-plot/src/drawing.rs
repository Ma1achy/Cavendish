//! A picture as a list of things to draw, in points.
//!
//! A plot is built once into a [`Drawing`]; the application paints it with egui
//! ([`crate::paint`]), and a figure is the same list written as SVG ([`crate::svg`]), as PDF
//! ([`crate::pdf`]) or painted to a PNG. Coordinates are points (1/72 inch) from the top left
//! corner, x to the right and y **down**. Colours are opaque sRGB bytes: a tint is a mix with
//! the background, worked out when the list is built, so no format needs transparency.

/// An opaque sRGB colour: red, green, blue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Colour(pub [u8; 3]);

impl Colour {
    /// The colour of a hex number `0xRRGGBB`.
    #[must_use]
    pub const fn hex(rgb: u32) -> Self {
        Self([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8])
    }

    /// The colour as `#rrggbb`.
    #[must_use]
    pub fn css(self) -> String {
        let [r, g, b] = self.0;
        format!("#{r:02x}{g:02x}{b:02x}")
    }

    /// This colour mixed with `other`: `part` of this and the rest of the other, in sRGB bytes.
    #[must_use]
    pub fn mixed(self, other: Self, part: f32) -> Self {
        let mix = |a: u8, b: u8| (f32::from(a) * part + f32::from(b) * (1.0 - part)).round() as u8;
        Self([mix(self.0[0], other.0[0]), mix(self.0[1], other.0[1]), mix(self.0[2], other.0[2])])
    }

    /// The colour for egui.
    #[must_use]
    pub const fn egui(self) -> egui::Color32 {
        egui::Color32::from_rgb(self.0[0], self.0[1], self.0[2])
    }
}

/// The face a text is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    /// Ubuntu Light: titles, legends, labels.
    Sans,
    /// Ubuntu Mono: numbers, units, captions.
    Mono,
}

/// Which point of a text's box stands at its position: fractions of the box's width and height,
/// `(0, 0)` the top left corner, `(1, 1)` the bottom right.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor(pub f32, pub f32);

impl Anchor {
    /// The top left corner.
    pub const LEFT_TOP: Self = Self(0.0, 0.0);
    /// The middle of the left edge.
    pub const LEFT_CENTRE: Self = Self(0.0, 0.5);
    /// The bottom left corner.
    pub const LEFT_BOTTOM: Self = Self(0.0, 1.0);
    /// The middle of the top edge.
    pub const CENTRE_TOP: Self = Self(0.5, 0.0);
    /// The middle.
    pub const CENTRE: Self = Self(0.5, 0.5);
    /// The top right corner.
    pub const RIGHT_TOP: Self = Self(1.0, 0.0);
    /// The middle of the right edge.
    pub const RIGHT_CENTRE: Self = Self(1.0, 0.5);
    /// The bottom right corner.
    pub const RIGHT_BOTTOM: Self = Self(1.0, 1.0);

    /// The top left corner of a box of `size` anchored at `at`.
    #[must_use]
    pub fn corner(self, at: [f32; 2], size: [f32; 2]) -> [f32; 2] {
        [at[0] - self.0 * size[0], at[1] - self.1 * size[1]]
    }
}

/// Where a point of a text's box goes on the page. The box, `size` wide and high, is laid out
/// with its `anchor` at `at`; `point` is measured from the box's top left corner. A text that is
/// `turned` is then turned a quarter turn anticlockwise about `at`, so that it reads up the page.
#[must_use]
pub fn text_point(at: [f32; 2], anchor: Anchor, size: [f32; 2], turned: bool, point: [f32; 2]) -> [f32; 2] {
    let [left, top] = anchor.corner(at, size);
    let (x, y) = (left + point[0], top + point[1]);
    if turned { [at[0] + (y - at[1]), at[1] - (x - at[0])] } else { [x, y] }
}

/// The rectangle a text takes on the page, as its top left and bottom right corners: for a
/// turned text, as tall as the text is long.
#[must_use]
pub fn text_box(at: [f32; 2], anchor: Anchor, size: [f32; 2], turned: bool) -> ([f32; 2], [f32; 2]) {
    let (a, b) =
        (text_point(at, anchor, size, turned, [0.0, 0.0]), text_point(at, anchor, size, turned, size));
    ([a[0].min(b[0]), a[1].min(b[1])], [a[0].max(b[0]), a[1].max(b[1])])
}

/// One thing to draw. Lengths and positions are points; a `dash` is the lengths of the dashes
/// and gaps in turn (`[4, 3]`: four on, three off), empty for a solid line.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// A line through `points`, `width` wide.
    Line {
        /// The points joined, in order.
        points: Vec<[f32; 2]>,
        /// The line's width.
        width: f32,
        /// Its colour.
        colour: Colour,
        /// Dashes and gaps in turn; empty for solid.
        dash: Vec<f32>,
    },
    /// A rectangle from `min` (top left) to `max` (bottom right), filled, outlined or both.
    Rect {
        /// The top left corner.
        min: [f32; 2],
        /// The bottom right corner.
        max: [f32; 2],
        /// What it is filled with, if it is.
        fill: Option<Colour>,
        /// The width and colour of its outline, centred on its edge, if it has one.
        stroke: Option<(f32, Colour)>,
    },
    /// A filled disc.
    Circle {
        /// Its centre.
        centre: [f32; 2],
        /// Its radius.
        radius: f32,
        /// Its colour.
        fill: Colour,
    },
    /// A diamond: a square on its corner, `radius` from the centre to each corner.
    Diamond {
        /// Its centre.
        centre: [f32; 2],
        /// From the centre to a corner.
        radius: f32,
        /// What it is filled with.
        fill: Colour,
        /// The width and colour of its outline.
        stroke: (f32, Colour),
    },
    /// A filled polygon: its corners in order, closed back to the first. Convex for the egui
    /// painter (a face of a cube seen in projection always is).
    Polygon {
        /// The corners, in order round it.
        points: Vec<[f32; 2]>,
        /// What it is filled with.
        fill: Colour,
    },
    /// One line of text.
    Text {
        /// Where the anchor point of the text's box stands.
        at: [f32; 2],
        /// Which point of the box that is.
        anchor: Anchor,
        /// The text.
        text: String,
        /// The face.
        face: Face,
        /// The font size.
        size: f32,
        /// The colour.
        colour: Colour,
        /// Whether it is written up the page: turned a quarter turn anticlockwise about `at`
        /// ([`text_point`]). A vertical axis's label is.
        turned: bool,
    },
    /// A picture: `width × height` pixels of RGB, rows top to bottom, filling the rectangle.
    /// Written into SVG and PDF; the egui painter leaves it out (the application shows the
    /// scene through its own surface).
    Image {
        /// The top left corner.
        min: [f32; 2],
        /// The bottom right corner.
        max: [f32; 2],
        /// The picture's width, pixels.
        width: u32,
        /// The picture's height, pixels.
        height: u32,
        /// `width × height × 3` bytes.
        rgb: std::sync::Arc<Vec<u8>>,
    },
    /// From here on, nothing is drawn outside this rectangle, until [`Item::Unclip`]. Clips do
    /// not nest.
    Clip {
        /// The top left corner.
        min: [f32; 2],
        /// The bottom right corner.
        max: [f32; 2],
    },
    /// The end of a [`Item::Clip`].
    Unclip,
}

/// A picture: its size in points and what is drawn, in order (later items over earlier ones).
#[derive(Clone, Debug, PartialEq)]
pub struct Drawing {
    /// Width and height, points.
    pub size: [f32; 2],
    /// What the picture is, in a few words: the title of an SVG or a PDF.
    pub title: String,
    /// The items, first drawn first.
    pub items: Vec<Item>,
}

impl Drawing {
    /// An empty picture of `size` points.
    #[must_use]
    pub fn new(size: [f32; 2], title: impl Into<String>) -> Self {
        Self { size, title: title.into(), items: Vec::new() }
    }

    /// Another drawing's items, moved by `offset`, after this one's.
    pub fn append(&mut self, other: &Self, offset: [f32; 2]) {
        let moved = |p: [f32; 2]| [p[0] + offset[0], p[1] + offset[1]];
        for item in &other.items {
            let mut item = item.clone();
            match &mut item {
                Item::Line { points, .. } | Item::Polygon { points, .. } => {
                    points.iter_mut().for_each(|p| *p = moved(*p));
                }
                Item::Rect { min, max, .. } | Item::Image { min, max, .. } | Item::Clip { min, max } => {
                    (*min, *max) = (moved(*min), moved(*max));
                }
                Item::Circle { centre, .. } | Item::Diamond { centre, .. } => *centre = moved(*centre),
                Item::Text { at, .. } => *at = moved(*at),
                Item::Unclip => {}
            }
            self.items.push(item);
        }
    }

    /// Every text in the picture, in order.
    #[must_use]
    pub fn texts(&self) -> Vec<&str> {
        self.items
            .iter()
            .filter_map(|item| match item {
                Item::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }
}

/// How big a line of text is.
pub trait Measure {
    /// The width and height, points, of `text` in `face` at font size `size`. The height is
    /// the face's line height, whatever the text.
    fn size(&self, text: &str, face: Face, size: f32) -> [f32; 2];
}
