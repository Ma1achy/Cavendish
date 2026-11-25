//! The camera's arithmetic for the editor (`docs/slices/S18.md` §1, "Move the camera better"):
//! framing the selection, zooming toward the pointer, flying and looking. Each answer is one
//! [`ViewCommand::Camera`], so a whole move is one command through the engine (never an edit:
//! the owner's rule, Home apart).
//!
//! Frames and units: positions are metres in the world's frame (x east, y north, z up); angles
//! are degrees, as [`cav_sim::ViewSpec`] has them; pointer positions are points within a pane,
//! from its top-left corner.

use cav_engine::ViewCommand;
use cav_sim::{SceneSpec, Target, ViewSpec};

/// How much of the pane's shorter side a framed thing's bounding sphere spans: 0.6, inside the
/// brief's C3 band of 40% to 80%.
pub const FILL: f64 = 0.6;

/// How fast a fly session moves at first, metres per second (Shift ×10, Alt ×0.1; the wheel
/// halves or doubles it).
pub const FLY_SPEED: f64 = 2.0;

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scaled(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// Half the picture's height at the target's depth, per metre of distance, in a picture `aspect`
/// times as wide as high: how big the view is for a camera this far away (the same for both
/// projections, which keep the target's scale). The camera widens its field in a narrow picture,
/// so the aspect is the pane's own (S18's review, U5: it was 1, which framed a wide pane's body
/// about 1.5 times too large).
fn half_height_per_metre(view: &ViewSpec, aspect: f64) -> f64 {
    let unit = ViewSpec { distance: 1.0, ..view.clone() };
    cav_render::Camera::new(&unit, aspect).half_height
}

/// Where the camera eye is, metres in the world's frame.
#[must_use]
pub fn eye(view: &ViewSpec) -> [f64; 3] {
    let (to_eye, _, _) = view.basis();
    add(view.target, scaled(to_eye, view.distance))
}

/// The centre and radius (metres) of a sphere holding `target` at time `t` seconds: a body by its
/// shape ([`cav_sim::starters::reach`]), a tower by its height, the room by its box; the floor,
/// the world and nothing (the whole scene) by the scene's bounds.
#[must_use]
pub fn bounds_of(spec: &SceneSpec, target: Option<Target>, t: f64) -> ([f64; 3], f64) {
    let whole = || {
        let (lo, hi) = spec.bounds();
        let centre = scaled(add(lo, hi), 0.5);
        let half = [0.5 * (hi[0] - lo[0]), 0.5 * (hi[1] - lo[1]), 0.5 * (hi[2] - lo[2])];
        (centre, (half[0] * half[0] + half[1] * half[1] + half[2] * half[2]).sqrt().max(0.5))
    };
    match target {
        Some(Target::Body(n)) if n >= 1 && n <= spec.bodies.len() => {
            let body = &spec.bodies[n - 1];
            (body.motion.position(t, spec.array.instrument.gravity), cav_sim::starters::reach(body))
        }
        Some(Target::Tower(n)) if n >= 1 && n <= spec.array.towers.len() => {
            let height = spec.array.instrument_of(n - 1).tower_height;
            let [x, y, z] = spec.array.towers[n - 1].position;
            ([x, y, z + 0.5 * height], 0.5 * height)
        }
        Some(Target::Room) if spec.room.is_some() => {
            let room = spec.room.as_ref().expect("checked");
            let s = room.size;
            (add(room.origin, scaled(s, 0.5)), 0.5 * (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt())
        }
        _ => whole(),
    }
}

/// The camera that frames a sphere (`centre`, `radius` metres) in a pane `aspect` times as wide
/// as high, from the present direction: the sphere's outline on screen spans [`FILL`] of the
/// pane's shorter side, leaving a margin round it. In perspective the outline of a sphere is
/// wider than its radius at its centre's depth (its silhouette is where the eye's rays graze it:
/// a half-angle of asin(r/d)), so the distance is solved for the silhouette.
#[must_use]
pub fn frame(view: &ViewSpec, pane: usize, centre: [f64; 3], radius: f64, aspect: f64) -> ViewCommand {
    // The slope (half the shorter side over the distance) the outline must fill a share of.
    let fit = half_height_per_metre(view, aspect) * aspect.min(1.0);
    let k = FILL * fit;
    let distance = match view.projection {
        // tan(asin(r/d)) = k  ⇒  d = r √(1 + k²) / k.
        cav_sim::Projection::Perspective => radius * (1.0 + k * k).sqrt() / k,
        cav_sim::Projection::Orthographic => radius / k,
    };
    ViewCommand::Camera { pane, target: centre, yaw: view.yaw, pitch: view.pitch, distance }
}

/// How much of the pane's shorter side a framed voxel body's selection cage may span at most:
/// the cage (the box round its drawn cubes) stays inside the pane, with a margin.
pub const CAGE_FILL: f64 = 0.9;

/// The half-size of the box the view draws round a selected voxel body (`target` a body with a
/// shape), metres from its centre on each axis; `None` for anything else. Conservative: the
/// shape's reach on every axis, which is the drawn box for a sphere and holds any other shape.
#[must_use]
pub fn cage_of(spec: &SceneSpec, target: Option<Target>) -> Option<f64> {
    match target {
        Some(Target::Body(n)) => {
            spec.bodies.get(n.checked_sub(1)?).filter(|b| b.shape.is_some()).map(cav_sim::starters::reach)
        }
        _ => None,
    }
}

/// [`frame`], and then farther back if need be so that a selection cage of half-size `cage`
/// (metres, about `centre`) spans no more than [`CAGE_FILL`] of the pane's shorter side: its
/// corners, seen in perspective, reach farther out than the body inside (S18's second review,
/// UX 3: after F the cage ran off the pane).
#[must_use]
pub fn frame_caged(
    view: &ViewSpec,
    pane: usize,
    centre: [f64; 3],
    radius: f64,
    cage: Option<f64>,
    aspect: f64,
) -> ViewCommand {
    let command = frame(view, pane, centre, radius, aspect);
    let Some(half) = cage else { return command };
    let ViewCommand::Camera { mut distance, .. } = command else { return command };
    let span = |distance: f64| {
        let framed = ViewSpec { target: centre, distance, ..view.clone() };
        cage_span(&framed, centre, half, aspect)
    };
    // The span falls as the camera backs off; a few steps of the ratio settle it within a hair.
    for _ in 0..40 {
        let now = span(distance);
        if now <= CAGE_FILL {
            break;
        }
        distance *= (now / CAGE_FILL).max(1.0001);
    }
    ViewCommand::Camera { pane, target: centre, yaw: view.yaw, pitch: view.pitch, distance }
}

/// How much of the shorter side the cube of half-size `half` about `centre` spans in `view`, in a
/// pane `aspect` times as wide as high: twice its farthest corner from its centre's picture.
#[must_use]
pub fn cage_span(view: &ViewSpec, centre: [f64; 3], half: f64, aspect: f64) -> f64 {
    let camera = cav_render::Camera::new(view, aspect);
    let (w, h) = if aspect >= 1.0 { (4096.0 * aspect, 4096.0) } else { (4096.0, 4096.0 / aspect) };
    let whole = [w as u32, h as u32];
    let at = |p: [f64; 3]| camera.pixel(cav_core::Vec3::from_array(p), whole);
    let Some([x0, y0]) = at(centre) else { return f64::INFINITY };
    let mut reach: f64 = 0.0;
    for corner in 0..8 {
        let sign = |bit: usize| if corner & bit == 0 { -half } else { half };
        let p = [centre[0] + sign(1), centre[1] + sign(2), centre[2] + sign(4)];
        match at(p) {
            Some([x, y]) => reach = reach.max((x - x0).hypot(y - y0)),
            None => return f64::INFINITY,
        }
    }
    2.0 * reach / w.min(h)
}

/// Which way an arrow key moves the selection in a transform session, seen in `view`: `arrow` 0
/// for ← →, 1 for ↑ ↓, 2 for Page Up / Down. ← → go along the horizontal world axis nearest the
/// screen's right; ↑ ↓ along the one nearest the view's direction across the ground (away from
/// the eye is up the screen); Page Up / Down along z. Returns the world axis (0 x, 1 y, 2 z) and
/// the sign that makes the key's "more" (→, ↑, Page Up) go that way. S18's review, minor 11: the
/// arrows went along x and y whatever the view, so from the east → moved the body left.
#[must_use]
pub fn arrow_axis(view: &ViewSpec, arrow: usize) -> (usize, f64) {
    let (to_eye, right, _) = view.basis();
    let along = match arrow {
        0 => [right[0], right[1]],
        1 => [-to_eye[0], -to_eye[1]],
        _ => return (2, 1.0),
    };
    // Straight down (a plan view) the view's direction has no ground part: its up is the screen's.
    let along = if along[0].hypot(along[1]) < 1e-6 {
        let (_, _, up) = view.basis();
        [up[0], up[1]]
    } else {
        along
    };
    if along[0].abs() >= along[1].abs() { (0, along[0].signum()) } else { (1, along[1].signum()) }
}

/// The point on the plane through the target, square to the view, under `pointer` (points from
/// the pane's top-left corner) in a pane of `size` points. Metres, the world's frame.
#[must_use]
pub fn under_pointer(view: &ViewSpec, size: [f64; 2], pointer: [f64; 2]) -> [f64; 3] {
    let camera = cav_render::Camera::new(view, size[0] / size[1]);
    // Metres per point at the target's depth.
    let scale = 2.0 * camera.half_height / size[1];
    let right = (pointer[0] - 0.5 * size[0]) * scale;
    let up = (0.5 * size[1] - pointer[1]) * scale;
    let at = camera.origin + camera.right * right + camera.up * up;
    at.to_array()
}

/// The camera after zooming by `factor` (below 1 is in) about the point under `pointer`: the
/// whole camera is scaled about that point, so that point stays where it is on screen, in both
/// projections.
#[must_use]
pub fn zoom_about(
    view: &ViewSpec,
    pane: usize,
    size: [f64; 2],
    pointer: [f64; 2],
    factor: f64,
) -> ViewCommand {
    let p = under_pointer(view, size, pointer);
    let target =
        add(p, scaled([view.target[0] - p[0], view.target[1] - p[1], view.target[2] - p[2]], factor));
    ViewCommand::Camera { pane, target, yaw: view.yaw, pitch: view.pitch, distance: view.distance * factor }
}

/// Which way a fly session is being pushed: each −1, 0 or 1 (S/W along the view, A/D across it,
/// Q/E along the world's up).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Push {
    /// +1 forward (W), −1 back (S).
    pub forward: f64,
    /// +1 right (D), −1 left (A).
    pub right: f64,
    /// +1 up (E), −1 down (Q), along the world's z.
    pub up: f64,
}

/// One step of a fly session: the camera moved by `push` at `speed` metres per second for `dt`
/// seconds, its angles and distance kept (so the eye moves as the target does).
#[must_use]
pub fn fly(view: &ViewSpec, pane: usize, push: Push, speed: f64, dt: f64) -> ViewCommand {
    let (to_eye, right, _) = view.basis();
    let forward = scaled(to_eye, -1.0);
    let step = add(add(scaled(forward, push.forward), scaled(right, push.right)), [0.0, 0.0, push.up]);
    let target = add(view.target, scaled(step, speed * dt));
    ViewCommand::Camera { pane, target, yaw: view.yaw, pitch: view.pitch, distance: view.distance }
}

/// Looking round from where the eye is: the angles turned by `yaw` and `pitch` degrees, and the
/// target moved so that the eye stays put (fly's look, where an orbit turns about the target).
#[must_use]
pub fn look(view: &ViewSpec, pane: usize, yaw: f64, pitch: f64) -> ViewCommand {
    let eye = eye(view);
    let turned =
        ViewSpec { yaw: view.yaw + yaw, pitch: (view.pitch + pitch).clamp(-89.0, 89.0), ..view.clone() };
    let (to_eye, _, _) = turned.basis();
    let target = add(eye, scaled(to_eye, -view.distance));
    ViewCommand::Camera { pane, target, yaw: turned.yaw, pitch: turned.pitch, distance: view.distance }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(view: &ViewSpec, command: &ViewCommand) -> ViewSpec {
        match *command {
            ViewCommand::Camera { target, yaw, pitch, distance, .. } => {
                ViewSpec { target, yaw, pitch, distance, ..view.clone() }
            }
            _ => panic!("{command:?}"),
        }
    }

    #[test]
    fn fly_moves_by_the_integrated_speed() {
        // S18 C5: N steps at a fixed push move the eye by speed × N × dt along the view, to 1e-9 m.
        let start = ViewSpec {
            target: [1.0, 2.0, 3.0],
            yaw: 30.0,
            pitch: -20.0,
            distance: 5.0,
            ..ViewSpec::home(&SceneSpec::room())
        };
        let (to_eye, right, _) = start.basis();
        let (dt, frames) = (1.0 / 60.0, 120);
        let fly_for = |push: Push, speed: f64| {
            let mut view = start.clone();
            for _ in 0..frames {
                view = apply(&view, &fly(&view, 0, push, speed, dt));
            }
            [eye(&view)[0] - eye(&start)[0], eye(&view)[1] - eye(&start)[1], eye(&view)[2] - eye(&start)[2]]
        };
        let along = |d: [f64; 3], v: [f64; 3]| d[0] * v[0] + d[1] * v[1] + d[2] * v[2];
        let forward = fly_for(Push { forward: 1.0, ..Push::default() }, FLY_SPEED);
        let expected = FLY_SPEED * f64::from(frames) * dt;
        assert!((along(forward, scaled(to_eye, -1.0)) - expected).abs() < 1e-9, "{forward:?}");
        assert!(along(forward, right).abs() < 1e-9, "no drift across");
        // Shift ×10 and Alt ×0.1 are exactly that.
        let fast = fly_for(Push { forward: 1.0, ..Push::default() }, FLY_SPEED * 10.0);
        let slow = fly_for(Push { forward: 1.0, ..Push::default() }, FLY_SPEED * 0.1);
        assert!((along(fast, scaled(to_eye, -1.0)) / expected - 10.0).abs() < 1e-9);
        assert!((along(slow, scaled(to_eye, -1.0)) / expected - 0.1).abs() < 1e-9);
        // E rises along the world's z whatever the pitch; D goes right.
        let up = fly_for(Push { up: 1.0, ..Push::default() }, FLY_SPEED);
        assert!((up[2] - expected).abs() < 1e-9 && up[0].abs() < 1e-12 && up[1].abs() < 1e-12);
        let side = fly_for(Push { right: 1.0, ..Push::default() }, FLY_SPEED);
        assert!((along(side, right) - expected).abs() < 1e-9);
        // Control: no push, no move.
        assert_eq!(fly_for(Push::default(), FLY_SPEED), [0.0; 3]);
    }

    #[test]
    fn looking_keeps_the_eye_and_zooming_keeps_the_pointed_point() {
        let view = ViewSpec {
            target: [0.0, 0.0, 1.0],
            yaw: -60.0,
            pitch: 25.0,
            distance: 12.0,
            ..ViewSpec::home(&SceneSpec::room())
        };
        let looked = apply(&view, &look(&view, 0, 15.0, -5.0));
        let (a, b) = (eye(&view), eye(&looked));
        assert!(a.iter().zip(b).all(|(p, q)| (p - q).abs() < 1e-12), "the eye stays: {a:?} {b:?}");
        assert!((looked.yaw - view.yaw - 15.0).abs() < 1e-12);
        // Zooming about a point leaves it under the same pixel.
        let (size, pointer) = ([800.0, 600.0], [620.0, 140.0]);
        let before = under_pointer(&view, size, pointer);
        let zoomed = apply(&view, &zoom_about(&view, 0, size, pointer, 0.5));
        let camera = cav_render::Camera::new(&zoomed, size[0] / size[1]);
        let whole = [8000_u32, 6000];
        let [x, y] = camera.pixel(cav_core::Vec3::from_array(before), whole).expect("in front");
        assert!((x / 10.0 - pointer[0]).abs() < 0.01 && (y / 10.0 - pointer[1]).abs() < 0.01, "{x} {y}");
        assert!((zoomed.distance - 6.0).abs() < 1e-12);
    }
}
