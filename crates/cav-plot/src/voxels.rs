//! Voxel bodies on paper and on screen: the legend of the field that colours them, made from what
//! is drawn, and a vector drawing of a view of them.
//!
//! The legend ([`legend`]) is a display list like any plot's: the application paints it in a
//! corner of a scene pane, and a figure writes it into SVG or PDF. Its ends are the
//! [`VoxelFrame`]'s range, which is the range of the values drawn (never one set beforehand),
//! and its bar is the field's own colour map ([`cav_render::colour`]).
//!
//! The vector view ([`view`]) draws a pane's voxel bodies as a technical drawing would: an
//! orthographic projection along the pane's direction, each cube's faces that look towards the
//! viewer (at most three) as filled polygons, far to near (painter's order; no other hiding),
//! each face a little darker the more it turns from the light, the towers as lines, and the
//! legend. It draws at most the cubes the frame holds ([`cav_sim::voxel_view::MAX_DRAWN_CUBES`]).

use cav_core::Vec3;
use cav_sim::voxel_view::{LeafField, VoxelFrame};
use cav_sim::{SceneSpec, ViewSpec};

use crate::drawing::{Anchor, Colour, Drawing, Face, Item, Measure};
use crate::style::Style;

/// Segments of a legend's colour bar.
pub const BAR_SEGMENTS: usize = 32;

/// Whether `style` is drawn on a dark surface.
#[must_use]
pub fn is_dark(style: &Style) -> bool {
    let [r, g, b] = style.background.0.map(u32::from);
    r + g + b < 3 * 128
}

/// A number as a legend writes it: a level as it is (a whole number, or a half between two at
/// the middle of an odd range, never rounded to a level it is not); otherwise three figures, or a
/// power of ten when it is very large or very small.
#[must_use]
pub fn number(value: f64, field: LeafField) -> String {
    if field == LeafField::Level {
        return format!("{value}");
    }
    if value == 0.0 {
        return "0".into();
    }
    let size = value.abs();
    if !(1e-2..1e4).contains(&size) {
        let text = format!("{value:.2e}");
        // `1.20e-5` as `1.2e-5`, `1.00e4` as `1e4`.
        let (mantissa, exponent) = text.split_once('e').unwrap_or((&text, "0"));
        let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
        return format!("{mantissa}e{exponent}");
    }
    let decimals = (2 - size.log10().floor() as i32).max(0) as usize;
    let text = format!("{value:.decimals$}");
    if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.').to_owned() } else { text }
}

/// The field's name as the legend's heading: the field and its unit, and for `phase` the tower.
#[must_use]
pub fn heading(frame: &VoxelFrame, scene: &SceneSpec) -> String {
    let field = frame.colouring.field;
    match field {
        LeafField::Phase => {
            let tower = scene
                .array
                .towers
                .get(frame.colouring.tower)
                .map_or_else(|| format!("tower {}", frame.colouring.tower + 1), |t| t.label.clone());
            format!("phase at {tower}, {}", field.unit())
        }
        // A level is its own unit: "level, level" would say it twice.
        LeafField::Level => field.name().to_owned(),
        _ => format!("{}, {}", field.name(), field.unit()),
    }
}

/// The values a legend writes under its bar: the range's two ends and its middle (zero, for a
/// diverging field).
#[must_use]
pub fn ticks(frame: &VoxelFrame) -> Option<[f64; 3]> {
    let (lo, hi) = frame.range?;
    Some([lo, 0.5 * (lo + hi), hi])
}

/// The colour of `value` on the frame's map, as the renderer colours a cube of it.
#[must_use]
pub fn colour_of(frame: &VoxelFrame, value: f64, dark: bool) -> Colour {
    let range = frame.range.unwrap_or((0.0, 1.0));
    Colour(cav_render::colour::of_value(value, range, frame.colouring.field.diverging(), dark))
}

/// The legend of `frame` (of `scene`), in `style`: a heading, a colour bar of
/// [`BAR_SEGMENTS`] segments from the range's least to its greatest, and the numbers of
/// [`ticks`] under it, on a panel of the style's background. `None` when nothing is drawn.
#[must_use]
pub fn legend(
    frame: &VoxelFrame,
    scene: &SceneSpec,
    style: &Style,
    measure: &dyn Measure,
) -> Option<Drawing> {
    let (lo, hi) = frame.range?;
    let field = frame.colouring.field;
    let dark = is_dark(style);
    let size = style.text_size;
    let pad = 0.5 * size;
    let title = heading(frame, scene);
    let [title_w, title_h] = measure.size(&title, Face::Sans, size);
    let labels = ticks(frame)?.map(|v| number(v, field));
    let widths: Vec<f32> = labels.iter().map(|l| measure.size(l, Face::Mono, size)[0]).collect();
    // The ends' labels run inwards from the ends and the middle's is centred: each half of the
    // bar holds an end's label, half the middle's, and a gap of a letter's width.
    let half = widths[0].max(widths[2]) + 0.5 * widths[1] + size;
    let bar_w = (8.0 * size).max(title_w).max(2.0 * half);
    let bar_h = 0.7 * size;
    let label_h = measure.size("0", Face::Mono, size)[1];
    let width = bar_w + 2.0 * pad;
    let height = title_h + bar_h + label_h + 4.0 * pad;
    let mut drawing = Drawing::new([width, height], format!("legend · {title}"));
    drawing.items.push(Item::Rect {
        min: [0.0, 0.0],
        max: [width, height],
        fill: Some(style.background),
        stroke: Some((style.rule_width, style.rule)),
    });
    drawing.items.push(Item::Text {
        at: [pad, pad],
        anchor: Anchor::LEFT_TOP,
        text: title,
        face: Face::Sans,
        size,
        colour: style.text,
        turned: false,
    });
    let top = 2.0 * pad + title_h;
    let step = bar_w / BAR_SEGMENTS as f32;
    for k in 0..BAR_SEGMENTS {
        // Each segment is the colour of the value at its middle.
        let t = (k as f64 + 0.5) / BAR_SEGMENTS as f64;
        let value = lo + (hi - lo) * t;
        let x = pad + step * k as f32;
        drawing.items.push(Item::Rect {
            min: [x, top],
            max: [x + step, top + bar_h],
            fill: Some(colour_of(frame, value, dark)),
            stroke: None,
        });
    }
    drawing.items.push(Item::Rect {
        min: [pad, top],
        max: [pad + bar_w, top + bar_h],
        fill: None,
        stroke: Some((style.rule_width, style.rule)),
    });
    let below = top + bar_h + pad;
    for (n, (label, anchor)) in
        labels.into_iter().zip([Anchor::LEFT_TOP, Anchor::CENTRE_TOP, Anchor::RIGHT_TOP]).enumerate()
    {
        let x = pad + bar_w * n as f32 / 2.0;
        drawing.items.push(Item::Line {
            points: vec![[x, top + bar_h], [x, top + bar_h + 0.4 * pad]],
            width: style.rule_width,
            colour: style.rule,
            dash: Vec::new(),
        });
        drawing.items.push(Item::Text {
            at: [x, below],
            anchor,
            text: label,
            face: Face::Mono,
            size,
            colour: style.text,
            turned: false,
        });
    }
    Some(drawing)
}

/// What a vector view drew, for its record.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
    /// Faces drawn (polygons).
    pub faces: usize,
    /// Cubes those faces are of.
    pub cubes: usize,
    /// Whether the pane's own projection was perspective (the drawing is orthographic always).
    pub was_perspective: bool,
}

/// A pane's view of `scene` as a vector drawing of `points` (width, height): the voxel bodies
/// of `frame` as their faces, the towers as lines, the legend at the bottom right, the title at
/// the top left when `title` is given. Orthographic along the view's direction, whatever its
/// projection.
#[must_use]
pub fn view(
    scene: &SceneSpec,
    view: &ViewSpec,
    frame: &VoxelFrame,
    style: &Style,
    points: [f32; 2],
    title: Option<&str>,
    measure: &dyn Measure,
) -> (Drawing, Drawn) {
    let (to_eye, right, up) = view.basis();
    let (to_eye, right, up) = (Vec3::from_array(to_eye), Vec3::from_array(right), Vec3::from_array(up));
    let target = Vec3::from_array(view.target);
    // In the picture's plane, metres: across, up, and towards the eye.
    let plane = |p: Vec3| {
        let d = p - target;
        (d.dot(right), d.dot(up), d.dot(to_eye))
    };
    // Faces: the three of each cube that look towards the eye, with their depth and shade.
    let normals = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
    let light = (to_eye + up * 0.6 + right * 0.3).normalised();
    let dark = is_dark(style);
    let mut faces: Vec<(f64, usize, [Vec3; 4], Colour)> = Vec::new();
    for body in &frame.bodies {
        for cube in &body.cubes {
            let base = colour_of(frame, cube.value, dark);
            let h = 0.5 * cube.side;
            for normal in normals {
                if normal.dot(to_eye) <= 1e-12 {
                    continue;
                }
                // The face's four corners, in order round it.
                let (a, b) = if normal.x != 0.0 {
                    (Vec3::Y, Vec3::Z)
                } else if normal.y != 0.0 {
                    (Vec3::Z, Vec3::X)
                } else {
                    (Vec3::X, Vec3::Y)
                };
                let c = cube.centre + normal * h;
                let corners = [c - a * h - b * h, c + a * h - b * h, c + a * h + b * h, c - a * h + b * h];
                let shade = 0.65 + 0.35 * normal.dot(light).max(0.0);
                let colour = base.mixed(Colour([0, 0, 0]), shade as f32);
                faces.push((plane(c).2, faces.len(), corners, colour));
            }
        }
    }
    // Towers: each a segment from its base to its top.
    let towers: Vec<(Vec3, Vec3)> = (0..scene.array.towers.len())
        .map(|n| {
            let base = Vec3::from_array(scene.array.towers[n].position);
            (base, base + Vec3::Z * scene.array.instrument_of(n).tower_height)
        })
        .collect();
    // The scale that holds every corner and every tower, with a margin, centred.
    let mut extent = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    let mut include = |p: Vec3| {
        let (x, y, _) = plane(p);
        extent = (extent.0.min(x), extent.1.max(x), extent.2.min(y), extent.3.max(y));
    };
    for (_, _, corners, _) in &faces {
        corners.iter().for_each(|c| include(*c));
    }
    for (a, b) in &towers {
        include(*a);
        include(*b);
    }
    if !extent.0.is_finite() {
        extent = (-1.0, 1.0, -1.0, 1.0);
    }
    let top_room = if title.is_some() { 2.0 * style.title_size } else { style.text_size };
    let (width, height) = (f64::from(points[0]), f64::from(points[1]) - f64::from(top_room));
    let span_x = (extent.1 - extent.0).max(1e-9);
    let span_y = (extent.3 - extent.2).max(1e-9);
    let scale = 0.9 * (width / span_x).min(height / span_y);
    let (mid_x, mid_y) = (0.5 * (extent.0 + extent.1), 0.5 * (extent.2 + extent.3));
    let to_page = |p: Vec3| {
        let (x, y, _) = plane(p);
        [
            (0.5 * width + (x - mid_x) * scale) as f32,
            (f64::from(top_room) + 0.5 * height - (y - mid_y) * scale) as f32,
        ]
    };
    let mut drawing =
        Drawing::new(points, title.map_or_else(|| format!("View · {}", scene.name), str::to_owned));
    drawing.items.push(Item::Rect {
        min: [0.0, 0.0],
        max: points,
        fill: Some(style.background),
        stroke: None,
    });
    if let Some(title) = title {
        drawing.items.push(Item::Text {
            at: [style.text_size, 0.5 * style.title_size],
            anchor: Anchor::LEFT_TOP,
            text: title.to_owned(),
            face: Face::Sans,
            size: style.title_size,
            colour: style.text,
            turned: false,
        });
    }
    // Far to near: faces and towers in one order, by depth towards the eye (ties by the order
    // they were made in, so the bytes are the same every time).
    enum Thing {
        Face(usize),
        Tower(usize),
    }
    let mut order: Vec<(f64, usize, Thing)> =
        faces.iter().enumerate().map(|(n, f)| (f.0, f.1, Thing::Face(n))).collect();
    for (n, (a, b)) in towers.iter().enumerate() {
        let depth = 0.5 * (plane(*a).2 + plane(*b).2);
        order.push((depth, faces.len() + n, Thing::Tower(n)));
    }
    order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    for (_, _, thing) in &order {
        match thing {
            Thing::Face(n) => {
                let (_, _, corners, colour) = &faces[*n];
                drawing.items.push(Item::Polygon {
                    points: corners.iter().map(|c| to_page(*c)).collect(),
                    fill: *colour,
                });
            }
            Thing::Tower(n) => {
                let (a, b) = towers[*n];
                drawing.items.push(Item::Line {
                    points: vec![to_page(a), to_page(b)],
                    width: 2.0 * style.line,
                    colour: style.ink,
                    dash: Vec::new(),
                });
            }
        }
    }
    if let Some(key) = legend(frame, scene, style, measure) {
        let margin = style.text_size;
        let at = [points[0] - key.size[0] - margin, points[1] - key.size[1] - margin];
        drawing.append(&key, at);
    }
    let cubes = frame.bodies.iter().map(|b| b.cubes.len()).sum();
    let drawn = Drawn {
        faces: faces.len(),
        cubes,
        was_perspective: view.projection == cav_sim::Projection::Perspective,
    };
    (drawing, drawn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_written_short() {
        assert_eq!(number(3.0, LeafField::Level), "3");
        assert_eq!(number(0.0, LeafField::Mass), "0");
        assert_eq!(number(2.5, LeafField::Height), "2.5");
        assert_eq!(number(123.456, LeafField::Mass), "123");
        assert_eq!(number(0.0123, LeafField::Fill), "0.0123");
        assert_eq!(number(-1.2e-5, LeafField::Phase), "-1.2e-5");
        assert_eq!(number(1.0e5, LeafField::Mass), "1e5");
    }
}
