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
        Behaviour::Probe => probe(painter, rect, colour),
    }
}

/// A probe's needle into a point, with the point ringed: read what is there.
fn probe(painter: &Painter, rect: Rect, colour: Color32) {
    let stroke = Stroke::new(1.8, colour);
    painter.line_segment([at(rect, 17.0, 3.0), at(rect, 8.0, 12.0)], stroke);
    painter.rect_filled(Rect::from_two_pos(at(rect, 14.5, 2.0), at(rect, 18.5, 6.0)), 1.0, colour);
    painter.circle_stroke(at(rect, 6.0, 14.0), rect.width() * 0.16, stroke);
    painter.circle_filled(at(rect, 6.0, 14.0), rect.width() * 0.05, colour);
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

/// A small picture of what a starter adds: a dot for a point mass, a disc for a sphere, a square
/// for a cube, a drum for a cylinder, a triangle for a Platonic solid or any other shape; a moving
/// body (an oscillator, a pendulum) has a swing drawn under it. `None` (no body) is a cross.
pub fn starter(painter: &Painter, rect: Rect, body: Option<&cav_sim::BodySpec>, colour: Color32) {
    let stroke = Stroke::new(1.4, colour);
    let Some(body) = body else {
        painter.line_segment([at(rect, 5.0, 5.0), at(rect, 15.0, 15.0)], stroke);
        painter.line_segment([at(rect, 15.0, 5.0), at(rect, 5.0, 15.0)], stroke);
        return;
    };
    let moving = !matches!(body.motion, cav_sim::Motion::Static { .. });
    let (y, size) = if moving { (8.0, 0.75) } else { (10.0, 1.0) };
    let c = at(rect, 10.0, y);
    let r = rect.width() * 0.32 * size;
    match body.shape.as_ref().map(|s| &s.kind) {
        None => {
            painter.circle_filled(c, rect.width() * 0.12, colour);
        }
        Some(
            cav_sim::ShapeKind::Sphere { .. }
            | cav_sim::ShapeKind::Shell { .. }
            | cav_sim::ShapeKind::Ellipsoid { .. },
        ) => {
            painter.circle_stroke(c, r, stroke);
        }
        Some(cav_sim::ShapeKind::Cuboid { .. }) => {
            painter.rect_stroke(
                Rect::from_center_size(c, egui::vec2(2.0 * r, 2.0 * r) * 0.85),
                0.0,
                stroke,
                egui::StrokeKind::Middle,
            );
        }
        Some(cav_sim::ShapeKind::Cylinder { .. }) => {
            painter.rect_stroke(
                Rect::from_center_size(c, egui::vec2(1.4 * r, 2.0 * r)),
                3.0,
                stroke,
                egui::StrokeKind::Middle,
            );
        }
        Some(_) => {
            let points = vec![
                c + egui::vec2(0.0, -r),
                c + egui::vec2(r * 0.9, r * 0.7),
                c + egui::vec2(-r * 0.9, r * 0.7),
            ];
            painter.add(Shape::closed_line(points, stroke));
        }
    }
    if moving {
        painter.line_segment([at(rect, 3.0, 18.0), at(rect, 17.0, 18.0)], Stroke::new(1.0, colour));
        painter.line_segment([at(rect, 3.0, 18.0), at(rect, 5.0, 16.5)], Stroke::new(1.0, colour));
        painter.line_segment([at(rect, 17.0, 18.0), at(rect, 15.0, 16.5)], Stroke::new(1.0, colour));
    }
}

/// An eye, open (shown) or struck through (hidden): the scene tree's visibility.
pub fn eye(painter: &Painter, rect: Rect, open: bool, colour: Color32) {
    let stroke = Stroke::new(1.4, colour);
    let left = at(rect, 3.0, 10.0);
    let right = at(rect, 17.0, 10.0);
    let top: Vec<Pos2> = (0..=8)
        .map(|k| {
            let t = k as f32 / 8.0;
            let x = 3.0 + 14.0 * t;
            at(rect, x, 10.0 - 5.0 * (std::f32::consts::PI * t).sin())
        })
        .collect();
    let bottom: Vec<Pos2> = (0..=8)
        .map(|k| {
            let t = k as f32 / 8.0;
            let x = 3.0 + 14.0 * t;
            at(rect, x, 10.0 + 5.0 * (std::f32::consts::PI * t).sin())
        })
        .collect();
    painter.add(Shape::line(top, stroke));
    painter.add(Shape::line(bottom, stroke));
    painter.circle_filled(at(rect, 10.0, 10.0), rect.width() * 0.1, colour);
    let _ = (left, right);
    if !open {
        painter.line_segment([at(rect, 3.0, 17.0), at(rect, 17.0, 3.0)], stroke);
    }
}

/// A padlock, shut (locked) or open.
pub fn lock(painter: &Painter, rect: Rect, shut: bool, colour: Color32) {
    let stroke = Stroke::new(1.4, colour);
    let body = Rect::from_min_max(at(rect, 5.0, 9.0), at(rect, 15.0, 17.0));
    if shut {
        painter.rect_filled(body, 1.0, colour);
    } else {
        painter.rect_stroke(body, 1.0, stroke, egui::StrokeKind::Middle);
    }
    let lift = if shut { 0.0 } else { 3.0 };
    let shackle: Vec<Pos2> = (0..=8)
        .map(|k| {
            let a = std::f32::consts::PI * k as f32 / 8.0;
            at(rect, 10.0 - 3.5 * a.cos(), 9.0 - lift - 4.0 * a.sin())
        })
        .collect();
    painter.add(Shape::line(shackle, stroke));
}
