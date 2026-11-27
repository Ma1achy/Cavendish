//! The window's icons (the tool palette's, the scene tree's, a viewport header's and its navigation
//! gizmo's), drawn with the painter: the application's fonts have no pictographs (S18's first
//! pictures drew `▾` and `⟲` as boxes), and a few strokes need no image files.
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

/// A polyline through points of the 20 × 20 design grid.
fn path(painter: &Painter, rect: Rect, points: &[(f32, f32)], stroke: Stroke) {
    painter.add(Shape::line(points.iter().map(|&(x, y)| at(rect, x, y)).collect(), stroke));
}

/// A filled triangle through three points of the design grid.
fn triangle(painter: &Painter, rect: Rect, points: [(f32, f32); 3], colour: Color32) {
    let points = points.iter().map(|&(x, y)| at(rect, x, y)).collect();
    painter.add(Shape::convex_polygon(points, colour, Stroke::NONE));
}

/// A stroke that keeps its weight as an icon is drawn smaller than the design's 20 points.
fn line(rect: Rect, colour: Color32) -> Stroke {
    Stroke::new((1.5 * rect.width() / 18.0).clamp(1.0, 1.6), colour)
}

/// What a viewport shows, for its header's first control: a cube for the scene, a wave for the
/// signal, stems for the periodogram, fountains for the flights, an uneven comb for the cadence.
pub fn content(painter: &Painter, rect: Rect, content: cav_sim::PaneContent, colour: Color32) {
    use cav_sim::PaneContent;
    let stroke = line(rect, colour);
    match content {
        PaneContent::Scene => {
            let hexagon = [(10.0, 2.0), (17.0, 6.0), (17.0, 14.0), (10.0, 18.0), (3.0, 14.0), (3.0, 6.0)];
            painter.add(Shape::closed_line(hexagon.iter().map(|&(x, y)| at(rect, x, y)).collect(), stroke));
            path(painter, rect, &[(3.0, 6.0), (10.0, 10.0), (17.0, 6.0)], stroke);
            path(painter, rect, &[(10.0, 10.0), (10.0, 18.0)], stroke);
        }
        PaneContent::Signal => {
            let wave: Vec<(f32, f32)> = (0..=16)
                .map(|k| {
                    let x = 2.0 + k as f32;
                    (x, 10.0 - 5.5 * (std::f32::consts::TAU * k as f32 / 11.0).sin())
                })
                .collect();
            path(painter, rect, &wave, stroke);
        }
        PaneContent::Periodogram => {
            path(painter, rect, &[(2.0, 17.0), (18.0, 17.0)], stroke);
            for (x, height) in [(5.0, 6.0), (9.0, 14.0), (13.0, 8.0), (16.0, 4.0)] {
                path(painter, rect, &[(x, 17.0), (x, 17.0 - height)], stroke);
            }
        }
        PaneContent::Flight => {
            path(painter, rect, &[(2.0, 17.0), (18.0, 17.0)], stroke);
            for (from, to, top) in [(2.5, 10.5, 3.0), (9.5, 17.5, 7.0)] {
                let arc: Vec<(f32, f32)> = (0..=10)
                    .map(|k| {
                        let u = k as f32 / 10.0;
                        (from + (to - from) * u, 17.0 - (17.0 - top) * 4.0 * u * (1.0 - u))
                    })
                    .collect();
                path(painter, rect, &arc, stroke);
            }
        }
        PaneContent::Cadence => {
            path(painter, rect, &[(2.0, 15.0), (18.0, 15.0)], stroke);
            for x in [4.0, 7.0, 10.0, 16.0] {
                path(painter, rect, &[(x, 15.0), (x, 5.0)], stroke);
            }
        }
    }
}

/// A camera: the header's camera menu.
pub fn camera(painter: &Painter, rect: Rect, colour: Color32) {
    let stroke = line(rect, colour);
    painter.rect_stroke(
        Rect::from_min_max(at(rect, 2.0, 6.0), at(rect, 13.0, 15.0)),
        1.5,
        stroke,
        egui::StrokeKind::Middle,
    );
    let lens = [(13.0, 10.5), (18.0, 6.5), (18.0, 14.5)];
    painter.add(Shape::closed_line(lens.iter().map(|&(x, y)| at(rect, x, y)).collect(), stroke));
}

/// A draw style, for the header's segmented group: a lit ball (shaded), a plain disc (flat), two
/// squares seen through one another (x-ray), a globe of lines (wireframe), a sheet (paper).
pub fn shading(painter: &Painter, rect: Rect, style: cav_sim::DrawStyle, colour: Color32) {
    use cav_sim::DrawStyle;
    let stroke = line(rect, colour);
    let centre = at(rect, 10.0, 10.0);
    let radius = rect.width() * 0.36;
    match style {
        DrawStyle::Shaded => {
            painter.circle(centre, radius, colour.gamma_multiply(0.35), stroke);
            painter.circle_filled(at(rect, 7.6, 7.6), rect.width() * 0.11, colour);
        }
        DrawStyle::Flat => {
            painter.circle_filled(centre, radius, colour);
        }
        DrawStyle::XRay => {
            let back = Rect::from_min_max(at(rect, 3.0, 3.0), at(rect, 13.0, 13.0));
            let front = Rect::from_min_max(at(rect, 7.0, 7.0), at(rect, 17.0, 17.0));
            painter.rect(back, 1.0, colour.gamma_multiply(0.3), stroke, egui::StrokeKind::Middle);
            painter.rect_stroke(front, 1.0, stroke, egui::StrokeKind::Middle);
        }
        DrawStyle::Wireframe => {
            painter.circle_stroke(centre, radius, stroke);
            path(painter, rect, &[(2.8, 10.0), (17.2, 10.0)], stroke);
            let meridian: Vec<Pos2> = (0..=16)
                .map(|k| {
                    let a = std::f32::consts::TAU * k as f32 / 16.0;
                    centre + vec2(0.42 * radius * a.cos(), radius * a.sin())
                })
                .collect();
            painter.add(Shape::line(meridian, stroke));
        }
        DrawStyle::Paper => {
            painter.rect_stroke(
                Rect::from_min_max(at(rect, 5.0, 3.0), at(rect, 15.0, 17.0)),
                0.0,
                stroke,
                egui::StrokeKind::Middle,
            );
            path(painter, rect, &[(7.5, 8.0), (12.5, 8.0)], stroke);
            path(painter, rect, &[(7.5, 12.0), (12.5, 12.0)], stroke);
        }
    }
}

/// Two discs, one over the other: what is drawn over the scene (the header's Overlays).
pub fn overlays(painter: &Painter, rect: Rect, colour: Color32) {
    let stroke = line(rect, colour);
    let radius = rect.width() * 0.27;
    painter.circle(at(rect, 8.0, 8.0), radius, colour.gamma_multiply(0.35), stroke);
    painter.circle_stroke(at(rect, 12.0, 12.0), radius, stroke);
}

/// An arrow from end to end between two stops: show the whole of a plot's axis.
pub fn show_all(painter: &Painter, rect: Rect, colour: Color32) {
    let stroke = line(rect, colour);
    path(painter, rect, &[(3.0, 5.0), (3.0, 15.0)], stroke);
    path(painter, rect, &[(17.0, 5.0), (17.0, 15.0)], stroke);
    path(painter, rect, &[(6.0, 10.0), (14.0, 10.0)], stroke);
    triangle(painter, rect, [(4.5, 10.0), (8.0, 7.0), (8.0, 13.0)], colour);
    triangle(painter, rect, [(15.5, 10.0), (12.0, 7.0), (12.0, 13.0)], colour);
}

/// A window filling its frame (maximise), or, with `restore`, two windows one behind the other.
pub fn maximise(painter: &Painter, rect: Rect, restore: bool, colour: Color32) {
    let stroke = line(rect, colour);
    if restore {
        let front = Rect::from_min_max(at(rect, 3.0, 7.0), at(rect, 13.0, 17.0));
        painter.rect_stroke(front, 0.0, stroke, egui::StrokeKind::Middle);
        path(painter, rect, &[(7.0, 7.0), (7.0, 3.0), (17.0, 3.0), (17.0, 13.0), (13.0, 13.0)], stroke);
    } else {
        let frame = Rect::from_min_max(at(rect, 3.0, 4.0), at(rect, 17.0, 16.0));
        painter.rect_stroke(frame, 0.0, stroke, egui::StrokeKind::Middle);
        painter.rect_filled(Rect::from_min_max(at(rect, 3.0, 4.0), at(rect, 17.0, 7.0)), 0.0, colour);
    }
}

/// A frame with an arrow leaving it (pop out), or, with `dock`, coming back into it.
pub fn pop_out(painter: &Painter, rect: Rect, dock: bool, colour: Color32) {
    let stroke = line(rect, colour);
    path(painter, rect, &[(9.0, 6.0), (3.0, 6.0), (3.0, 17.0), (14.0, 17.0), (14.0, 11.0)], stroke);
    path(painter, rect, &[(9.0, 11.0), (17.0, 3.0)], stroke);
    if dock {
        triangle(painter, rect, [(8.0, 12.0), (8.5, 7.0), (13.0, 11.5)], colour);
    } else {
        triangle(painter, rect, [(18.0, 2.0), (17.5, 7.0), (13.0, 2.5)], colour);
    }
}

/// A cross: close.
pub fn close(painter: &Painter, rect: Rect, colour: Color32) {
    let stroke = line(rect, colour);
    path(painter, rect, &[(5.0, 5.0), (15.0, 15.0)], stroke);
    path(painter, rect, &[(15.0, 5.0), (5.0, 15.0)], stroke);
}

/// A small triangle pointing down, `size` points wide, centred on `centre`: a drop-down's mark.
pub fn chevron(painter: &Painter, centre: Pos2, size: f32, colour: Color32) {
    let (half, drop) = (0.5 * size, 0.3 * size);
    let points = vec![centre + vec2(-half, -drop), centre + vec2(half, -drop), centre + vec2(0.0, drop)];
    painter.add(Shape::convex_polygon(points, colour, Stroke::NONE));
}

/// A magnifying glass with a plus: zoom.
pub fn zoom(painter: &Painter, rect: Rect, colour: Color32) {
    let stroke = line(rect, colour);
    painter.circle_stroke(at(rect, 8.5, 8.5), rect.width() * 0.28, stroke);
    path(painter, rect, &[(12.7, 12.7), (17.5, 17.5)], Stroke::new(stroke.width + 0.6, colour));
    path(painter, rect, &[(6.0, 8.5), (11.0, 8.5)], stroke);
    path(painter, rect, &[(8.5, 6.0), (8.5, 11.0)], stroke);
}

/// Four arrowheads round a point: slide the view (pan).
pub fn pan(painter: &Painter, rect: Rect, colour: Color32) {
    painter.circle_filled(at(rect, 10.0, 10.0), rect.width() * 0.09, colour);
    triangle(painter, rect, [(10.0, 1.5), (6.5, 6.0), (13.5, 6.0)], colour);
    triangle(painter, rect, [(10.0, 18.5), (6.5, 14.0), (13.5, 14.0)], colour);
    triangle(painter, rect, [(1.5, 10.0), (6.0, 6.5), (6.0, 13.5)], colour);
    triangle(painter, rect, [(18.5, 10.0), (14.0, 6.5), (14.0, 13.5)], colour);
}

/// A grid drawn as the projection shows it: lines that meet far away (perspective) or stay
/// parallel (orthographic).
pub fn projection(painter: &Painter, rect: Rect, projection: cav_sim::Projection, colour: Color32) {
    let stroke = line(rect, colour);
    let top = match projection {
        cav_sim::Projection::Perspective => (7.0, 13.0),
        cav_sim::Projection::Orthographic => (3.0, 17.0),
    };
    let outline = [(top.0, 4.0), (top.1, 4.0), (17.0, 16.0), (3.0, 16.0)];
    painter.add(Shape::closed_line(outline.iter().map(|&(x, y)| at(rect, x, y)).collect(), stroke));
    path(painter, rect, &[(0.5 * (top.0 + top.1), 4.0), (10.0, 16.0)], stroke);
    path(painter, rect, &[(0.5 * (top.0 + 3.0), 10.0), (0.5 * (top.1 + 17.0), 10.0)], stroke);
}

/// A transport button's picture, as a media player draws them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    /// A bar and a triangle pointing at it, leftward: to the start.
    Start,
    /// Two triangles leftward: a step back.
    StepBack,
    /// A triangle rightward: play.
    Play,
    /// Two bars: pause.
    Pause,
    /// Two triangles rightward: a step forward.
    StepForward,
    /// A triangle rightward and a bar: to the end.
    End,
}

/// A transport button's picture ([`Transport`]).
pub fn transport(painter: &Painter, rect: Rect, glyph: Transport, colour: Color32) {
    let bar = |x: f32| {
        painter.rect_filled(Rect::from_min_max(at(rect, x, 4.0), at(rect, x + 2.4, 16.0)), 0.5, colour);
    };
    // A triangle from its flat side at `from` to its point at `to`, 12 high.
    let wedge = |from: f32, to: f32| triangle(painter, rect, [(from, 4.0), (from, 16.0), (to, 10.0)], colour);
    match glyph {
        Transport::Start => {
            bar(3.0);
            wedge(17.0, 6.5);
        }
        Transport::StepBack => {
            wedge(10.0, 2.0);
            wedge(18.0, 10.0);
        }
        Transport::Play => wedge(5.0, 17.0),
        Transport::Pause => {
            bar(5.0);
            bar(12.6);
        }
        Transport::StepForward => {
            wedge(2.0, 10.0);
            wedge(10.0, 18.0);
        }
        Transport::End => {
            wedge(3.0, 13.5);
            bar(14.6);
        }
    }
}

/// What playing does at the end of the timeline ([`crate::transport::AtEnd`]): an arrow that
/// comes round again (loop), an arrow up to a bar (stop), an arrow on from a bar (run on).
pub fn at_end(painter: &Painter, rect: Rect, at_end: crate::transport::AtEnd, colour: Color32) {
    use crate::transport::AtEnd;
    let stroke = line(rect, colour);
    match at_end {
        AtEnd::Loop => {
            // Most of a circle, clockwise from the upper right round to just left of the top,
            // and an arrowhead there pointing on round it.
            let (centre, radius) = (at(rect, 10.0, 10.5), rect.width() * 0.32);
            let arc: Vec<Pos2> = (0..=20)
                .map(|k| {
                    let a = (-45.0 + 300.0 * k as f32 / 20.0).to_radians();
                    centre + vec2(radius * a.cos(), radius * a.sin())
                })
                .collect();
            painter.add(Shape::line(arc, stroke));
            triangle(painter, rect, [(11.8, 3.4), (7.6, 1.4), (9.1, 7.2)], colour);
        }
        AtEnd::Stop => {
            // An arrow up to a bar: the end is where it stops.
            path(painter, rect, &[(2.5, 10.0), (11.0, 10.0)], stroke);
            triangle(painter, rect, [(14.0, 10.0), (9.5, 6.5), (9.5, 13.5)], colour);
            painter.rect_filled(Rect::from_min_max(at(rect, 15.0, 4.0), at(rect, 17.2, 16.0)), 0.5, colour);
        }
        AtEnd::RunOn => {
            // A bar and an arrow on from it: past the end.
            painter.rect_filled(Rect::from_min_max(at(rect, 2.8, 4.0), at(rect, 5.0, 16.0)), 0.5, colour);
            path(painter, rect, &[(7.0, 10.0), (14.0, 10.0)], stroke);
            triangle(painter, rect, [(18.0, 10.0), (13.0, 6.5), (13.0, 13.5)], colour);
        }
    }
}

/// A magnifying glass: search.
pub fn search(painter: &Painter, rect: Rect, colour: Color32) {
    let stroke = line(rect, colour);
    painter.circle_stroke(at(rect, 8.5, 8.5), rect.width() * 0.28, stroke);
    path(painter, rect, &[(12.7, 12.7), (17.5, 17.5)], Stroke::new(stroke.width + 0.6, colour));
}

/// A plus: add one.
pub fn plus(painter: &Painter, rect: Rect, colour: Color32) {
    let stroke = line(rect, colour);
    path(painter, rect, &[(4.0, 10.0), (16.0, 10.0)], stroke);
    path(painter, rect, &[(10.0, 4.0), (10.0, 16.0)], stroke);
}
