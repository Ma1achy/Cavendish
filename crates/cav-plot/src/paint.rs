//! A [`Drawing`] painted by egui: in a pane of the application, or into a picture with no window.

use egui::{Align2, Color32, FontFamily, FontId, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::drawing::{Anchor, Drawing, Face, Item, Measure};

fn font(face: Face, size: f32) -> FontId {
    FontId::new(
        size,
        match face {
            Face::Sans => FontFamily::Proportional,
            Face::Mono => FontFamily::Monospace,
        },
    )
}

/// The measure of text as egui lays it out: the application's.
pub struct EguiMeasure<'a>(pub &'a egui::Painter);

impl Measure for EguiMeasure<'_> {
    fn size(&self, text: &str, face: Face, size: f32) -> [f32; 2] {
        let took = self.0.layout_no_wrap(text.to_owned(), font(face, size), Color32::WHITE).size();
        [took.x, took.y]
    }
}

fn align(fraction: f32) -> egui::Align {
    if fraction <= 0.25 {
        egui::Align::Min
    } else if fraction < 0.75 {
        egui::Align::Center
    } else {
        egui::Align::Max
    }
}

/// Paints `drawing` with its top left corner at `origin`, one point of the drawing to one of
/// egui's. A picture ([`Item::Image`]) is left out: egui shows pictures through textures, and
/// the application has the scene's own.
pub fn paint(painter: &egui::Painter, origin: Pos2, drawing: &Drawing) {
    let at = |point: [f32; 2]| origin + vec2(point[0], point[1]);
    let whole = painter.clone();
    let mut painter = whole.clone();
    for item in &drawing.items {
        match item {
            Item::Line { points, width, colour, dash } => {
                let points: Vec<Pos2> = points.iter().map(|point| at(*point)).collect();
                let stroke = Stroke::new(*width, colour.egui());
                if let ([from, to], true) = (points.as_slice(), dash.len() < 2) {
                    // A straight rule is a segment, which egui sets on whole pixels.
                    painter.line_segment([*from, *to], stroke);
                } else if dash.len() < 2 {
                    painter.add(egui::Shape::line(points, stroke));
                } else {
                    let (dashes, gaps): (Vec<f32>, Vec<f32>) =
                        dash.chunks(2).map(|pair| (pair[0], pair.get(1).copied().unwrap_or(pair[0]))).unzip();
                    painter
                        .extend(egui::Shape::dashed_line_with_offset(&points, stroke, &dashes, &gaps, 0.0));
                }
            }
            Item::Rect { min, max, fill, stroke } => {
                let rect = Rect::from_min_max(at(*min), at(*max));
                if let Some(fill) = fill {
                    painter.rect_filled(rect, 0.0, fill.egui());
                }
                if let Some((width, colour)) = stroke {
                    painter.rect_stroke(rect, 0.0, Stroke::new(*width, colour.egui()), StrokeKind::Middle);
                }
            }
            Item::Circle { centre, radius, fill } => {
                painter.circle_filled(at(*centre), *radius, fill.egui());
            }
            Item::Diamond { centre, radius, fill, stroke } => {
                let (c, r) = (at(*centre), *radius);
                let corners =
                    vec![pos2(c.x, c.y - r), pos2(c.x + r, c.y), pos2(c.x, c.y + r), pos2(c.x - r, c.y)];
                painter.add(egui::Shape::convex_polygon(
                    corners,
                    fill.egui(),
                    Stroke::new(stroke.0, stroke.1.egui()),
                ));
            }
            Item::Text { at: position, anchor: Anchor(x, y), text, face, size, colour } => {
                painter.text(
                    at(*position),
                    Align2([align(*x), align(*y)]),
                    text,
                    font(*face, *size),
                    colour.egui(),
                );
            }
            Item::Image { .. } => {}
            Item::Clip { min, max } => {
                let rect = Rect::from_min_max(at(*min), at(*max));
                painter = whole.with_clip_rect(rect.intersect(whole.clip_rect()));
            }
            Item::Unclip => painter = whole.clone(),
        }
    }
}
