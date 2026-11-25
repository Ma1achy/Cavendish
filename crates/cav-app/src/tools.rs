//! The editor's tools (`docs/slices/S18.md` §1, "Move things in the view"): the strip's data
//! (`ui/tools.ron`), the tool in hand, and the geometry the Move tool's gizmo and the Place tool
//! need, as pure functions.
//!
//! A tool is UI state, like the draw style: choosing one changes no scene and is never
//! journalled. What a tool *does* is a command: the gizmo, the transform session and the
//! inspector's field all end in the same `Command::Set` of the body's anchor
//! (`bodies[i].motion.<anchor>`), a run of them one journal entry and one undo; Place is an
//! `add <starter> at x y z`.
//!
//! Frames and units: world positions are metres in the world's frame (x east, y north, z up);
//! screen positions are points within a pane, from its top-left corner.

use std::sync::OnceLock;

use cav_sim::ViewSpec;
use serde::Deserialize;

/// The built-in tool strip, as compiled in.
pub const TOOLS_RON: &str = include_str!("../ui/tools.ron");

/// What a tool does: a closed set the code implements.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum Behaviour {
    /// Click selects; a drag of a body moves it in the view's plane (the app's first tool).
    Select,
    /// A translate gizmo on the selected body.
    Translate,
    /// A click on the ground adds the chosen starter there.
    Place,
}

/// One tool of the strip.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ToolData {
    /// Its id: `tool <id>` chooses it.
    pub id: String,
    /// What a tooltip and the palette call it.
    pub label: String,
    /// What its button says.
    pub short: String,
    /// What it does.
    pub behaviour: Behaviour,
    /// One line for its tooltip.
    pub about: String,
}

/// The strip.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Strip {
    /// The Move tool's snap step and the transform session's step, metres.
    pub snap_step: f64,
    /// The tools, top to bottom.
    pub tools: Vec<ToolData>,
}

impl Strip {
    /// Reads and checks the strip: ids unique, a positive snap step, each tool chosen by an
    /// action of the table (`tool <id>`).
    ///
    /// # Errors
    /// The first problem.
    pub fn parse(text: &str) -> Result<Self, String> {
        let strip: Self = ron::from_str(text).map_err(|e| format!("ui/tools.ron: {e}"))?;
        if !(strip.snap_step.is_finite() && strip.snap_step > 0.0) {
            return Err(format!("ui/tools.ron: the snap step must be positive, not {}", strip.snap_step));
        }
        let mut ids = std::collections::BTreeSet::new();
        for tool in &strip.tools {
            if !ids.insert(tool.id.as_str()) {
                return Err(format!("ui/tools.ron: the tool `{}` is listed twice", tool.id));
            }
            let line = format!("tool {}", tool.id);
            if !crate::actions::table().actions.iter().any(|a| a.line == line) {
                return Err(format!("ui/tools.ron: no action of ui/actions.ron runs `{line}`"));
            }
        }
        Ok(strip)
    }

    /// The tool called `id`.
    #[must_use]
    pub fn tool(&self, id: &str) -> Option<&ToolData> {
        self.tools.iter().find(|t| t.id == id)
    }
}

/// The strip compiled in, read once.
///
/// # Panics
/// When the built-in file does not parse or check: `the_tool_strip_is_valid` fails first.
pub fn strip() -> &'static Strip {
    static STRIP: OnceLock<Strip> = OnceLock::new();
    STRIP.get_or_init(|| Strip::parse(TOOLS_RON).expect("ui/tools.ron is valid (a test holds it)"))
}

/// The handle of the gizmo under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// An arrow: 0 x, 1 y, 2 z.
    Axis(usize),
    /// The square at the middle: free movement in the view's plane.
    Centre,
}

/// The translate gizmo as drawn: where its middle and the tip of each arrow are, points within
/// the pane, and how long each arrow is in the world, metres. An arrow seen end on (under
/// [`MIN_ARROW`] points long) is `None`: it cannot be dragged along.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gizmo {
    /// The body's position on screen.
    pub centre: egui::Pos2,
    /// Each arrow's tip on screen: x, y, z.
    pub tips: [Option<egui::Pos2>; 3],
    /// Each arrow's length in the world, metres.
    pub metres: f64,
}

/// How long an arrow is drawn, points, when it lies across the view.
pub const ARROW: f32 = 70.0;
/// The shortest an arrow may look and still be dragged along, points.
pub const MIN_ARROW: f32 = 12.0;
/// How near the pointer must be to an arrow or the square to take it, points.
pub const REACH: f32 = 7.0;

/// Where a point of the world (metres) is drawn in a pane of `size` points with `view`.
fn on_screen(view: &ViewSpec, size: egui::Vec2, at: [f64; 3]) -> Option<egui::Pos2> {
    let camera = cav_render::Camera::new(view, f64::from(size.x / size.y));
    let scale = 1024.0;
    let whole = [(f64::from(size.x) * scale) as u32, (f64::from(size.y) * scale) as u32];
    let [x, y] = camera.pixel(cav_core::Vec3::from_array(at), whole)?;
    Some(egui::pos2((x / scale) as f32, (y / scale) as f32))
}

/// The gizmo for a body at `at` (metres) in a pane of `size` points, or `None` when the body is
/// behind the camera.
#[must_use]
pub fn gizmo(view: &ViewSpec, size: egui::Vec2, at: [f64; 3]) -> Option<Gizmo> {
    let centre = on_screen(view, size, at)?;
    let camera = cav_render::Camera::new(view, f64::from(size.x / size.y));
    // Metres per point at the body's depth.
    let at_target = 2.0 * camera.half_height / f64::from(size.y);
    let depth = (cav_core::Vec3::from_array(at) - camera.origin - camera.eye).dot(camera.forward);
    let per_point = match view.projection {
        cav_sim::Projection::Orthographic => at_target,
        cav_sim::Projection::Perspective => at_target * depth.max(1e-6) / view.distance,
    };
    let metres = per_point * f64::from(ARROW);
    let tip = |axis: usize| {
        let mut end = at;
        end[axis] += metres;
        on_screen(view, size, end).filter(|p| p.distance(centre) >= MIN_ARROW)
    };
    Some(Gizmo { centre, tips: [tip(0), tip(1), tip(2)], metres })
}

/// The distance from `p` to the segment from `a` to `b`, points.
fn to_segment(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// The handle under `p`, if one is within [`REACH`]: the square first, then the nearest arrow.
#[must_use]
pub fn hit(gizmo: &Gizmo, p: egui::Pos2) -> Option<Handle> {
    if (p - gizmo.centre).abs().max_elem() <= REACH {
        return Some(Handle::Centre);
    }
    (0..3)
        .filter_map(|axis| gizmo.tips[axis].map(|tip| (axis, to_segment(p, gizmo.centre, tip))))
        .filter(|(_, d)| *d <= REACH)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(axis, _)| Handle::Axis(axis))
}

/// How far a drag of `delta` points moves the body along `axis`, metres: the drag's share along
/// the arrow on screen, scaled by the arrow's metres per point.
#[must_use]
pub fn along(gizmo: &Gizmo, axis: usize, delta: egui::Vec2) -> f64 {
    let Some(tip) = gizmo.tips[axis] else { return 0.0 };
    let arrow = tip - gizmo.centre;
    let length = arrow.length();
    f64::from(delta.dot(arrow / length)) * gizmo.metres / f64::from(length)
}

/// `value` to the nearest multiple of `step`, or itself with no step.
#[must_use]
pub fn snapped(value: f64, step: Option<f64>) -> f64 {
    match step {
        Some(step) if step > 0.0 => (value / step).round() * step,
        _ => value,
    }
}

/// Where the ray through `pointer` (points within a pane of `size`) meets the horizontal plane
/// z = `ground`, metres; the point on the target's plane when the ray does not come down to it.
#[must_use]
pub fn on_ground(view: &ViewSpec, size: [f64; 2], pointer: [f64; 2], ground: f64) -> [f64; 3] {
    let p = crate::camera::under_pointer(view, size, pointer);
    let (to_eye, _, _) = view.basis();
    let (origin, direction) = match view.projection {
        cav_sim::Projection::Perspective => {
            let eye = crate::camera::eye(view);
            (eye, [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]])
        }
        // Parallel rays: from the plane of the eye, along the view.
        cav_sim::Projection::Orthographic => {
            let o = [
                p[0] + to_eye[0] * view.distance,
                p[1] + to_eye[1] * view.distance,
                p[2] + to_eye[2] * view.distance,
            ];
            (o, [-to_eye[0], -to_eye[1], -to_eye[2]])
        }
    };
    if direction[2] >= -1e-9 {
        return p;
    }
    let t = (ground - origin[2]) / direction[2];
    [origin[0] + t * direction[0], origin[1] + t * direction[1], ground]
}

/// A transform session (the keyboard's Move: `docs/slices/S18.md`): where the body started, how
/// far it has been moved along each axis, an axis lock, the distance being typed and the axis it
/// is for, and whether an edit has been made (so Esc knows to take it back).
#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    /// The body, from 0.
    pub body: usize,
    /// Its anchor at the start, metres.
    pub start: [f64; 3],
    /// The offset so far, metres, per axis.
    pub offset: [f64; 3],
    /// The axis locked to, if any (X, Y, Z).
    pub lock: Option<usize>,
    /// What has been typed, and for which axis.
    pub typed: String,
    /// The axis the typed distance is for.
    pub typed_axis: usize,
    /// Whether the session has issued an edit.
    pub edited: bool,
}

impl Session {
    /// A session on body `body` whose anchor is `start`.
    #[must_use]
    pub fn new(body: usize, start: [f64; 3]) -> Self {
        Self { body, start, offset: [0.0; 3], lock: None, typed: String::new(), typed_axis: 0, edited: false }
    }

    /// The anchor now: the start plus the offset.
    #[must_use]
    pub fn anchor(&self) -> [f64; 3] {
        [self.start[0] + self.offset[0], self.start[1] + self.offset[1], self.start[2] + self.offset[2]]
    }

    /// An arrow: `steps` (already ×10 or ×0.1) of `step` metres along `axis`, or along the locked
    /// axis if there is one.
    pub fn nudge(&mut self, axis: usize, steps: f64, step: f64) {
        let axis = self.lock.unwrap_or(axis);
        self.offset[axis] += steps * step;
        self.typed.clear();
    }

    /// A character of the typed distance (`\u{8}` takes one off); the typed axis's offset
    /// follows the number as it is typed (an unfinished one, `-` or `.`, counts as nothing).
    pub fn type_char(&mut self, c: char) {
        if c == '\u{8}' {
            self.typed.pop();
        } else {
            self.typed.push(c);
        }
        let axis = self.lock.unwrap_or(self.typed_axis);
        self.offset[axis] = self.typed.parse::<f64>().unwrap_or(0.0);
    }

    /// Tab: the next axis's distance is typed.
    pub fn next_axis(&mut self) {
        self.typed_axis = (self.lock.unwrap_or(self.typed_axis) + 1) % 3;
        self.lock = None;
        self.typed.clear();
    }

    /// X, Y or Z: lock to that axis, or unlock it.
    pub fn toggle_lock(&mut self, axis: usize) {
        self.lock = if self.lock == Some(axis) { None } else { Some(axis) };
        self.typed_axis = axis;
        self.typed.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tool_strip_is_valid() {
        let strip = Strip::parse(TOOLS_RON).unwrap();
        assert_eq!(
            strip.tools.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["select", "move", "place"]
        );
        assert_eq!(strip.snap_step, 0.25);
        // Control: a tool no action chooses, and a twice-listed one, are refused.
        assert!(
            Strip::parse(&TOOLS_RON.replace("id: \"place\"", "id: \"paint\""))
                .unwrap_err()
                .contains("tool paint")
        );
        assert!(Strip::parse(&TOOLS_RON.replace("id: \"place\"", "id: \"move\"")).is_err());
        assert!(Strip::parse(&TOOLS_RON.replace("snap_step: 0.25", "snap_step: 0.0")).is_err());
    }

    #[test]
    fn a_drag_along_an_arrow_is_that_many_metres() {
        let view = ViewSpec { target: [0.0, 0.0, 1.0], ..ViewSpec::home(&cav_sim::SceneSpec::room()) };
        let size = egui::vec2(900.0, 600.0);
        let g = gizmo(&view, size, [1.0, 0.5, 1.0]).expect("in view");
        let tip = g.tips[0].expect("x lies across the iso view");
        // Dragging the whole arrow's length along it is the arrow's metres.
        // (Screen positions are f32 points: a few parts in 10⁷.)
        let metres = along(&g, 0, tip - g.centre);
        assert!((metres - g.metres).abs() < 1e-6 * g.metres, "{metres} {}", g.metres);
        // Across it, nothing.
        let across = egui::vec2(-(tip - g.centre).y, (tip - g.centre).x);
        assert!(along(&g, 0, across).abs() < 1e-6 * g.metres);
        // The handles are found where they are drawn; control: far away, none.
        assert_eq!(hit(&g, g.centre + egui::vec2(2.0, 1.0)), Some(Handle::Centre));
        assert_eq!(hit(&g, g.centre + (tip - g.centre) * 0.8), Some(Handle::Axis(0)));
        assert_eq!(hit(&g, g.centre + egui::vec2(300.0, 300.0)), None);
        assert_eq!(snapped(1.13, Some(0.25)), 1.25);
        assert_eq!(snapped(1.13, None), 1.13);
    }

    #[test]
    fn the_ground_under_the_pointer_is_on_the_ray() {
        let view = ViewSpec { target: [1.0, 2.0, 1.5], ..ViewSpec::home(&cav_sim::SceneSpec::room()) };
        let (size, pointer) = ([800.0, 600.0], [300.0, 420.0]);
        let ground = on_ground(&view, size, pointer, 0.0);
        assert_eq!(ground[2], 0.0);
        // It is drawn where the pointer is.
        let camera = cav_render::Camera::new(&view, size[0] / size[1]);
        let [x, y] = camera.pixel(cav_core::Vec3::from_array(ground), [8000, 6000]).unwrap();
        assert!((x / 10.0 - pointer[0]).abs() < 0.01 && (y / 10.0 - pointer[1]).abs() < 0.01, "{x} {y}");
        // Orthographic too.
        let ortho = ViewSpec { projection: cav_sim::Projection::Orthographic, ..view };
        let ground = on_ground(&ortho, size, pointer, 0.0);
        let camera = cav_render::Camera::new(&ortho, size[0] / size[1]);
        let [x, y] = camera.pixel(cav_core::Vec3::from_array(ground), [8000, 6000]).unwrap();
        assert!((x / 10.0 - pointer[0]).abs() < 0.01 && (y / 10.0 - pointer[1]).abs() < 0.01, "{x} {y}");
    }

    #[test]
    fn a_session_moves_types_and_locks() {
        let mut s = Session::new(0, [1.0, 2.0, 3.0]);
        s.nudge(0, 1.0, 0.25);
        s.nudge(0, 10.0, 0.25);
        assert_eq!(s.anchor(), [3.75, 2.0, 3.0]);
        s.toggle_lock(2);
        s.nudge(0, 1.0, 0.25);
        assert_eq!(s.anchor()[2], 3.25, "with Z locked, any arrow moves along z");
        s.toggle_lock(2);
        s.type_char('2');
        assert_eq!(s.anchor()[2], 5.0, "typing sets the typed axis's offset");
        s.next_axis();
        s.type_char('-');
        s.type_char('1');
        assert_eq!(s.anchor(), [0.0, 2.0, 5.0], "a typed distance replaces that axis's offset");
        s.type_char('\u{8}');
        assert_eq!(s.anchor()[0], 1.0, "a lone `-` is nothing");
    }
}
