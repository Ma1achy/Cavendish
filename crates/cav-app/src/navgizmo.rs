//! The navigation gizmo (`docs/design/workspace.md` §2; Blender's): an axis triad in the top
//! right corner of a scene viewport's canvas, drawn from the view's own basis, with three small
//! buttons under it.
//!
//! - A click on an axis end looks along that axis from that side (`view top @2`: the named views
//!   are the six ends); a click on the end already faced looks from the other side.
//! - A drag on the triad orbits, exactly as a drag in the view does (the same `Orbit` command,
//!   0.4° a point).
//! - Under it: zoom (drag up and down, or click to step), pan (drag) and the projection
//!   (perspective or orthographic).
//!
//! Lengths are points; an axis is 0, 1 or 2 for x (east), y (north) and z (up), in the world's
//! frame. Nothing here is state: the triad is the view's, drawn again each frame.

use cav_engine::{Command, ViewCommand};
use cav_sim::{NamedView, ViewSpec};
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, pos2, vec2};

use crate::icons;
use crate::panels::Asked;
use crate::shell::Shell;

/// The triad's side in a viewport with room for it, points.
pub const SIDE: f32 = 64.0;
/// The least side it is drawn at, points: in a smaller viewport there is no gizmo.
pub const LEAST_SIDE: f32 = 40.0;
/// The most of a canvas's shorter side the triad takes: a quarter.
pub const SHARE: f32 = 0.25;
/// How far it stands from the canvas's top and right edges, points.
pub const MARGIN: f32 = 8.0;
/// The side of each of the three buttons under it, points.
pub const BUTTON: f32 = 20.0;
/// How much a point of drag on the zoom button changes the camera's distance: its natural
/// logarithm (so 69 points halve or double it).
pub const ZOOM_PER_POINT: f64 = 0.01;

/// The axes' colours, x, y, z: red, green, blue, as the Move tool's arrows and the inspector's
/// vectors. sRGB.
pub const AXIS_COLOURS: [Color32; 3] = [
    Color32::from_rgb(0xff, 0x5a, 0x5a),
    Color32::from_rgb(0x6c, 0xe8, 0x6c),
    Color32::from_rgb(0x6c, 0xb4, 0xff),
];

/// One end of an axis of the triad.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct End {
    /// The axis: 0, 1, 2 for x, y, z.
    pub axis: usize,
    /// Whether it is the positive end.
    pub positive: bool,
    /// Where its middle is drawn, points.
    pub at: Pos2,
    /// How far it is from the viewer, from −1 (the end nearest the eye) to 1 (the farthest):
    /// the far ends are drawn dimmer, and under the near ones.
    pub depth: f32,
    /// The radius of its disc, points.
    pub radius: f32,
}

impl End {
    /// Its name: `+x`, `-z`.
    #[must_use]
    pub fn name(&self) -> String {
        format!("{}{}", if self.positive { '+' } else { '-' }, ["x", "y", "z"][self.axis])
    }
}

/// Where the triad stands in a canvas: its square in the top right corner, [`SIDE`] across or a
/// quarter of the canvas's shorter side if that is less; `None` when that is under
/// [`LEAST_SIDE`] (the viewport is too small for one).
#[must_use]
pub fn place(canvas: Rect) -> Option<Rect> {
    let side = SIDE.min(SHARE * canvas.width().min(canvas.height()));
    (side >= LEAST_SIDE).then(|| {
        Rect::from_min_size(pos2(canvas.right() - MARGIN - side, canvas.top() + MARGIN), vec2(side, side))
    })
}

/// The six ends of the triad of `view` drawn in the square `rect`: each axis's unit vector as
/// the view sees it (its components along the picture's right and up; no perspective, as a
/// compass has none), scaled to the square.
#[must_use]
pub fn ends(view: &ViewSpec, rect: Rect) -> [End; 6] {
    let camera = cav_render::Camera::new(view, 1.0);
    let (right, up, forward) = (camera.right.to_array(), camera.up.to_array(), camera.forward.to_array());
    let near = 0.14 * rect.width();
    let far = 0.10 * rect.width();
    let reach = 0.5 * rect.width() - near - 1.0;
    std::array::from_fn(|k| {
        let (axis, positive) = (k / 2, k % 2 == 0);
        let sign = if positive { 1.0 } else { -1.0 };
        let across = (sign * right[axis]) as f32;
        let upward = (sign * up[axis]) as f32;
        End {
            axis,
            positive,
            at: rect.center() + vec2(across, -upward) * reach,
            depth: (sign * forward[axis]) as f32,
            radius: if positive { near } else { far },
        }
    })
}

/// The named view that looks at the scene from the `positive` (or the negative) end of `axis`:
/// +z is the top view, −z the bottom, ±x east and west, ±y north and south.
#[must_use]
pub const fn looks_from(axis: usize, positive: bool) -> NamedView {
    match (axis, positive) {
        (0, true) => NamedView::East,
        (0, false) => NamedView::West,
        (1, true) => NamedView::North,
        (1, false) => NamedView::South,
        (_, true) => NamedView::Top,
        (_, false) => NamedView::Bottom,
    }
}

/// The end under `point`: of those whose disc (and two points round it) holds the point, the
/// one nearest the viewer. `None` beside every end.
#[must_use]
pub fn hit(ends: &[End; 6], point: Pos2) -> Option<End> {
    ends.iter()
        .filter(|end| end.at.distance(point) <= end.radius + 2.0)
        .min_by(|a, b| a.depth.total_cmp(&b.depth))
        .copied()
}

/// Draws pane `pane`'s gizmo in the corner of its `canvas` and takes its input; nothing when
/// the canvas is too small ([`place`]). What it asks goes into `asked`, as the view's own
/// pointer work does. `active` is whether the pane is the active one (a use of the gizmo makes
/// it so).
pub(crate) fn draw(
    shell: &mut Shell,
    ui: &egui::Ui,
    asked: &mut Asked,
    pane: usize,
    canvas: Rect,
    active: bool,
) {
    let Some(rect) = place(canvas) else { return };
    let n = pane + 1;
    let view = shell.engine.view(pane).clone();
    let mut used = false;
    let response = ui
        .interact(rect, egui::Id::unique(("navigation gizmo", pane)), Sense::click_and_drag())
        .on_hover_text("click an axis end to look from it (again: from the other side) · drag to orbit");
    let painter = ui.painter_at(canvas);
    let all = ends(&view, rect);
    let under = response.hover_pos().and_then(|point| hit(&all, point));
    if response.hovered() || response.dragged() {
        painter.circle_filled(rect.center(), 0.5 * rect.width(), Color32::from_white_alpha(22));
    }
    // The far ends first, so that the near ones are drawn over them.
    let mut order = all;
    order.sort_by(|a, b| b.depth.total_cmp(&a.depth));
    let halo = Color32::from_black_alpha(150);
    for end in order {
        let dim = if end.depth > 0.05 { 0.55 } else { 1.0 };
        let colour = AXIS_COLOURS[end.axis].gamma_multiply(dim);
        if end.positive {
            painter.line_segment([rect.center(), end.at], Stroke::new(3.5, halo));
            painter.line_segment([rect.center(), end.at], Stroke::new(2.0, colour));
            painter.circle(end.at, end.radius, colour, Stroke::new(1.0, halo));
            painter.text(
                end.at,
                Align2::CENTER_CENTER,
                ["X", "Y", "Z"][end.axis],
                FontId::proportional((end.radius * 1.35).clamp(9.0, 12.0)),
                Color32::BLACK,
            );
        } else {
            painter.circle(end.at, end.radius, colour.gamma_multiply(0.45), Stroke::new(1.5, colour));
        }
        if under.is_some_and(|hit| hit.axis == end.axis && hit.positive == end.positive) {
            painter.circle_stroke(end.at, end.radius + 1.0, Stroke::new(1.5, Color32::WHITE));
        }
        let disc = Rect::from_center_size(end.at, vec2(2.0 * end.radius, 2.0 * end.radius));
        shell.layout.buttons.push((format!("gizmo {n} {}", end.name()), disc));
    }
    shell.layout.buttons.push((format!("gizmo {n}"), rect));
    if response.clicked()
        && let Some(point) = response.interact_pointer_pos()
        && let Some(end) = hit(&all, point)
    {
        // Already looking from that end: the other side (Blender's).
        let named = looks_from(end.axis, end.positive);
        let named = if view.named() == Some(named) { looks_from(end.axis, !end.positive) } else { named };
        asked.lines.push(format!("view {} @{n}", named.name()));
        used = true;
    }
    if response.dragged() {
        // The view's own orbit: the same command, the same 0.4° a point.
        asked.drags.push((pane, response.drag_delta(), false, canvas.size()));
        used = true;
    }

    // The three buttons under the triad, their row's right edge under its right edge.
    let gap = 2.0;
    let left = rect.right() - 3.0 * BUTTON - 2.0 * gap;
    let button = |k: usize| {
        Rect::from_min_size(pos2(left + k as f32 * (BUTTON + gap), rect.bottom() + gap), vec2(BUTTON, BUTTON))
    };
    let zoom = crate::header::extra_control(
        shell,
        ui,
        pane,
        "gizmo/zoom",
        button(0),
        "Zoom\ndrag up to go nearer, down to go farther · click steps in, right-click out (the wheel zooms too)",
        None,
        Sense::click_and_drag(),
        icons::zoom,
    );
    shell.layout.buttons.push((format!("gizmo {n} zoom"), zoom.rect));
    let factor = if zoom.dragged() {
        (f64::from(zoom.drag_delta().y) * ZOOM_PER_POINT).exp()
    } else if zoom.clicked() {
        2f64.powf(-crate::panels::ZOOM_STEP)
    } else if zoom.secondary_clicked() {
        2f64.powf(crate::panels::ZOOM_STEP)
    } else {
        1.0
    };
    if factor != 1.0 {
        let distance = (view.distance * factor).clamp(0.05, 1.0e5);
        asked.commands.push(Command::View(ViewCommand::Zoom { pane, distance }));
        used = true;
    }
    let pan = crate::header::extra_control(
        shell,
        ui,
        pane,
        "gizmo/pan",
        button(1),
        "Pan\ndrag to slide the view (a middle-drag in the view does too)",
        None,
        Sense::drag(),
        icons::pan,
    );
    shell.layout.buttons.push((format!("gizmo {n} pan"), pan.rect));
    if pan.dragged() {
        asked.drags.push((pane, pan.drag_delta(), true, canvas.size()));
        used = true;
    }
    let projection = view.projection;
    let toggle = crate::header::extra_control(
        shell,
        ui,
        pane,
        "gizmo/projection",
        button(2),
        &crate::header::tooltip("proj-toggle"),
        Some("proj-toggle"),
        Sense::click(),
        |painter, rect, colour| icons::projection(painter, rect, projection, colour),
    );
    shell.layout.buttons.push((format!("proj toggle @{n}"), toggle.rect));
    if toggle.clicked() {
        asked.lines.push(format!("proj toggle @{n}"));
        used = true;
    }
    if used && !active {
        asked.commands.insert(0, Command::View(ViewCommand::Activate(pane)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_triad_is_the_view_s_basis() {
        // From the east (looking west) north is to the right and up is up: +y at the right of the
        // square, +z at its top, +x at its middle and nearest the viewer.
        let rect = Rect::from_min_size(pos2(100.0, 50.0), vec2(SIDE, SIDE));
        let scene = cav_sim::load("point").unwrap();
        let mut view = ViewSpec::framing(
            &scene,
            NamedView::Iso,
            cav_sim::Projection::Perspective,
            cav_sim::DrawStyle::Shaded,
        );
        (view.yaw, view.pitch) = NamedView::East.angles();
        let all = ends(&view, rect);
        let end = |axis: usize, positive: bool| {
            *all.iter().find(|e| e.axis == axis && e.positive == positive).unwrap()
        };
        assert!(
            end(1, true).at.x > rect.center().x + 15.0 && (end(1, true).at.y - rect.center().y).abs() < 0.01
        );
        assert!(
            end(2, true).at.y < rect.center().y - 15.0 && (end(2, true).at.x - rect.center().x).abs() < 0.01
        );
        assert!(end(0, true).at.distance(rect.center()) < 0.01);
        assert!((end(0, true).depth + 1.0).abs() < 1e-6 && (end(0, false).depth - 1.0).abs() < 1e-6);
        // Every end is inside the square, whatever the view.
        for (yaw, pitch) in [(-45.0, 35.0), (10.0, -80.0), (200.0, 0.0), (-90.0, 90.0)] {
            (view.yaw, view.pitch) = (yaw, pitch);
            for end in ends(&view, rect) {
                let disc = Rect::from_center_size(end.at, vec2(2.0 * end.radius, 2.0 * end.radius));
                assert!(rect.contains_rect(disc), "{} at {yaw}, {pitch}: {disc:?}", end.name());
            }
        }
        // The end under a point is the one nearest the viewer; beside every end there is none.
        (view.yaw, view.pitch) = NamedView::Top.angles();
        let all = ends(&view, rect);
        let middle = hit(&all, rect.center()).expect("both ends of z are at the middle");
        assert_eq!((middle.axis, middle.positive), (2, true), "looking down, +z is the nearer");
        assert!(hit(&all, rect.left_top()).is_none(), "control: the square's corner holds no end");
        // The six ends are the six named views that look along an axis.
        for axis in 0..3 {
            for positive in [true, false] {
                (view.yaw, view.pitch) = looks_from(axis, positive).angles();
                let facing =
                    ends(&view, rect).into_iter().min_by(|a, b| a.depth.total_cmp(&b.depth)).unwrap();
                assert_eq!((facing.axis, facing.positive), (axis, positive));
            }
        }
    }

    #[test]
    fn the_gizmo_is_never_more_than_a_quarter_of_a_small_viewport() {
        let canvas = |w: f32, h: f32| Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h));
        assert_eq!(place(canvas(800.0, 600.0)).unwrap().width(), SIDE);
        let small = place(canvas(400.0, 200.0)).unwrap();
        assert_eq!(small.width(), 50.0);
        assert!((small.right() - (400.0 - MARGIN)).abs() < 1e-3 && (small.top() - MARGIN).abs() < 1e-3);
        assert!(place(canvas(400.0, 159.0)).is_none(), "too small: no gizmo");
        assert!(place(canvas(120.0, 400.0)).is_none());
    }
}
