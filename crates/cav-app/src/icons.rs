//! The tool strip's icons, drawn with the painter: the application's fonts have no pictographs
//! (S18's first pictures drew `▾` and `⟲` as boxes), and a few strokes need no image files.
//!
//! Each icon fills the square `rect` it is given (points), in `colour`; none reads state.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, vec2};

use crate::tools::Behaviour;

/// A point of `rect` from coordinates in a 20 × 20 design grid (0 to 20 across and down).
fn at(rect: Rect, x: f32, y: f32) -> Pos2 {
    rect.min + vec2(x, y) * (rect.width() / 20.0)
}

/// The icon of a tool, by what it does: Select an arrow pointer, Move four arrows, Place a body
/// dropped onto the ground.
pub fn tool(painter: &Painter, rect: Rect, behaviour: Behaviour, colour: Color32) {
    match behaviour {
        Behaviour::Select => pointer(painter, rect, colour),
        Behaviour::Translate => four_arrows(painter, rect, colour),
        Behaviour::Place => place(painter, rect, colour),
    }
}

/// An arrow pointer, as every system draws one.
fn pointer(painter: &Painter, rect: Rect, colour: Color32) {
    let points =
        [(5.0, 2.0), (5.0, 17.0), (8.6, 13.4), (11.2, 19.0), (13.6, 17.9), (11.0, 12.4), (16.0, 12.4)];
    let points: Vec<Pos2> = points.iter().map(|&(x, y)| at(rect, x, y)).collect();
    painter.add(Shape::convex_polygon(points[..3].to_vec(), colour, Stroke::NONE));
    painter.add(Shape::closed_line(points, Stroke::new(1.6, colour)));
}

/// Four arrows out from the middle: move in any direction.
fn four_arrows(painter: &Painter, rect: Rect, colour: Color32) {
    let stroke = Stroke::new(1.8, colour);
    painter.line_segment([at(rect, 2.5, 10.0), at(rect, 17.5, 10.0)], stroke);
    painter.line_segment([at(rect, 10.0, 2.5), at(rect, 10.0, 17.5)], stroke);
    let head = |tip: (f32, f32), back: (f32, f32), side: (f32, f32)| {
        let points = vec![
            at(rect, tip.0, tip.1),
            at(rect, back.0 + side.0, back.1 + side.1),
            at(rect, back.0 - side.0, back.1 - side.1),
        ];
        painter.add(Shape::convex_polygon(points, colour, Stroke::NONE));
    };
    head((1.0, 10.0), (5.0, 10.0), (0.0, 3.0));
    head((19.0, 10.0), (15.0, 10.0), (0.0, 3.0));
    head((10.0, 1.0), (10.0, 5.0), (3.0, 0.0));
    head((10.0, 19.0), (10.0, 15.0), (3.0, 0.0));
}

/// A body coming down onto the ground, with a plus: put a new thing where you click.
fn place(painter: &Painter, rect: Rect, colour: Color32) {
    let stroke = Stroke::new(1.6, colour);
    // The ground, in perspective.
    painter.add(Shape::closed_line(
        vec![at(rect, 1.0, 16.0), at(rect, 13.0, 16.0), at(rect, 19.0, 19.5), at(rect, 7.0, 19.5)],
        stroke,
    ));
    // The body, on it.
    painter.circle_filled(at(rect, 10.0, 11.0), rect.width() * 0.18, colour);
    // The plus.
    painter.line_segment([at(rect, 15.5, 1.0), at(rect, 15.5, 8.0)], stroke);
    painter.line_segment([at(rect, 12.0, 4.5), at(rect, 19.0, 4.5)], stroke);
}

/// A grid of dots with one on a corner of it: moves go to the grid's points.
pub fn snap(painter: &Painter, rect: Rect, colour: Color32) {
    for i in 0..3 {
        for j in 0..3 {
            let p = at(rect, 3.0 + 7.0 * i as f32, 3.0 + 7.0 * j as f32);
            painter.circle_filled(p, 1.3, colour);
        }
    }
    painter.rect_stroke(
        Rect::from_min_max(at(rect, 3.0, 3.0), at(rect, 10.0, 10.0)),
        0.0,
        Stroke::new(1.4, colour),
        egui::StrokeKind::Middle,
    );
    painter.circle_filled(at(rect, 10.0, 10.0), 2.6, colour);
}
