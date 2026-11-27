//! The camera's arithmetic for the editor (`docs/slices/S18.md` §1, "Move the camera better"):
//! framing the selection, zooming toward the pointer, and the first-person camera
//! (`docs/slices/S20.md` §2: an eye that looks by raw pointer motion and moves by held keys, eased
//! with the frame's time). Each answer is one
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

/// The first-person camera's settings as compiled in (`ui/camera.ron`).
pub const CAMERA_RON: &str = include_str!("../ui/camera.ron");

/// The longest frame the first-person camera integrates, seconds: a frame longer than this (the
/// window was hidden, a breakpoint) moves the camera as one of this length, so a hitch never
/// throws it across the scene.
pub const MAX_DT: f64 = 0.1;

/// How far from straight ahead the first-person camera may look up or down, degrees: short of
/// the poles, where yaw would stop meaning a heading.
pub const PITCH_LIMIT: f64 = 89.0;

/// The least and greatest flying speed the wheel reaches, metres per second.
pub const FLY_SPEED_RANGE: (f64, f64) = (0.05, 1000.0);

/// The settings of the first-person camera (`ui/camera.ron`; `camera-set <setting> <value>` in
/// the console changes one for the session). Window state: never part of a scene.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// How far a look turns per count of raw pointer motion, degrees per count.
    pub sensitivity: f64,
    /// The flying speed at first, metres per second (the wheel changes the session's).
    pub fly_speed: f64,
    /// What sprinting multiplies the speed by (Ctrl held, or a double tap of W), dimensionless.
    pub sprint: f64,
    /// The time constant with which the velocity eases toward the wanted one, seconds.
    pub easing: f64,
    /// Two presses of a key within this long are a double tap, seconds.
    pub double_tap: f64,
    /// How long the speed is shown in the canvas after the wheel changed it, seconds.
    pub speed_shown: f64,
}

/// One setting of [`Settings`] as the console names it: its name, its unit, the least and
/// greatest value it takes, and what it is.
pub const SETTINGS: [(&str, &str, f64, f64, &str); 6] = [
    ("sensitivity", "°/count", 0.001, 10.0, "how far a look turns per count of mouse motion"),
    ("fly-speed", "m/s", FLY_SPEED_RANGE.0, FLY_SPEED_RANGE.1, "the flying speed at first"),
    ("sprint", "×", 1.0, 100.0, "what sprinting multiplies the speed by"),
    ("easing", "s", 0.0, 2.0, "the time constant of the velocity's easing"),
    ("double-tap", "s", 0.05, 1.0, "how close two presses are to be a double tap"),
    ("speed-shown", "s", 0.0, 10.0, "how long the speed is shown after the wheel changed it"),
];

impl Default for Settings {
    fn default() -> Self {
        Self::from_data()
    }
}

impl Settings {
    /// The settings of `ui/camera.ron`.
    ///
    /// # Panics
    /// If the compiled-in file does not parse (a test reads it).
    #[must_use]
    pub fn from_data() -> Self {
        ron::from_str(CAMERA_RON).expect("ui/camera.ron parses")
    }

    fn field(&mut self, name: &str) -> Option<&mut f64> {
        Some(match name {
            "sensitivity" => &mut self.sensitivity,
            "fly-speed" => &mut self.fly_speed,
            "sprint" => &mut self.sprint,
            "easing" => &mut self.easing,
            "double-tap" => &mut self.double_tap,
            "speed-shown" => &mut self.speed_shown,
            _ => return None,
        })
    }

    /// The value of the setting called `name` (one of [`SETTINGS`]), in its unit.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<f64> {
        self.clone().field(name).map(|value| *value)
    }

    /// Sets the setting called `name` to `value`, in its unit.
    ///
    /// # Errors
    /// An unknown name, or a value outside the setting's range; nothing is changed.
    pub fn set(&mut self, name: &str, value: f64) -> Result<(), String> {
        let Some(&(_, unit, lo, hi, _)) = SETTINGS.iter().find(|s| s.0 == name) else {
            let names: Vec<&str> = SETTINGS.iter().map(|s| s.0).collect();
            return Err(format!("camera-set <{}> <value>", names.join("|")));
        };
        if !(lo..=hi).contains(&value) {
            return Err(format!("{name} is from {lo} to {hi} {unit}"));
        }
        *self.field(name).expect("every listed setting is a field") = value;
        Ok(())
    }
}

/// Which way the first-person camera is pushed by the held keys: each −1, 0 or 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Push {
    /// +1 forward (W), −1 back (S), along the view's horizontal heading.
    pub forward: f64,
    /// +1 right (D), −1 left (A), horizontal.
    pub right: f64,
    /// +1 up (Space), −1 down (Shift), along the world's z.
    pub up: f64,
}

/// A velocity component eased toward a wanted one for `dt` seconds with time constant `easing`
/// seconds, integrated exactly: `v(t) = w + (v₀ − w)·e^(−t/τ)`, so the answer is the same
/// whatever frames the time is cut into (while `wanted` holds). Returns the new velocity (m/s)
/// and the distance moved (m). `easing` 0 is no easing: the wanted velocity at once.
#[must_use]
pub fn ease(velocity: f64, wanted: f64, dt: f64, easing: f64) -> (f64, f64) {
    if easing <= 0.0 {
        return (wanted, wanted * dt);
    }
    let gap = velocity - wanted;
    let decay = (-dt / easing).exp();
    (wanted + gap * decay, wanted * dt + gap * easing * (1.0 - decay))
}

/// The camera as a first-person game has it: an eye that looks and moves, where the view
/// ([`ViewSpec`]) has a target, two angles and a distance. Made from the view each frame
/// ([`Self::of`]) and written back as one [`ViewCommand::Camera`] ([`Self::command`]); only the
/// velocity is kept between frames (the window's state). It has no roll: the picture's right is
/// always horizontal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FirstPerson {
    /// Where the eye is, metres in the world's frame (x east, y north, z up).
    pub eye: [f64; 3],
    /// The view's yaw, degrees, as [`ViewSpec::yaw`] has it: the bearing of the eye seen from
    /// what it looks at, so the camera looks along yaw + 180°.
    pub yaw: f64,
    /// How far above the horizontal the camera looks, degrees (+ up; the negative of
    /// [`ViewSpec::pitch`]), within ±[`PITCH_LIMIT`].
    pub pitch: f64,
    /// The eye's velocity, metres per second in the world's frame.
    pub velocity: [f64; 3],
}

impl FirstPerson {
    /// The first-person camera at `view`'s eye, looking as it looks, moving at `velocity` (m/s).
    #[must_use]
    pub fn of(view: &ViewSpec, velocity: [f64; 3]) -> Self {
        Self { eye: eye(view), yaw: view.yaw, pitch: -view.pitch, velocity }
    }

    /// The command that puts pane `pane`'s camera here: `view`'s distance kept, its target
    /// placed that far along the line of sight.
    #[must_use]
    pub fn command(&self, view: &ViewSpec, pane: usize) -> ViewCommand {
        let turned = ViewSpec { yaw: self.yaw, pitch: -self.pitch, ..view.clone() };
        let (to_eye, _, _) = turned.basis();
        let target = add(self.eye, scaled(to_eye, -view.distance));
        ViewCommand::Camera { pane, target, yaw: turned.yaw, pitch: turned.pitch, distance: view.distance }
    }

    /// The horizontal unit vectors the keys move along: the heading (the line of sight laid
    /// flat, whatever the pitch) and the picture's right. World frame, dimensionless.
    #[must_use]
    pub fn heading(&self) -> ([f64; 3], [f64; 3]) {
        let yaw = self.yaw.to_radians();
        ([-yaw.cos(), -yaw.sin(), 0.0], [-yaw.sin(), yaw.cos(), 0.0])
    }

    /// Looks round by raw pointer motion: `counts` to the right and down (as a mouse reports
    /// them), `sensitivity` degrees a count. Right turns right, up looks up; the pitch stops at
    /// ±[`PITCH_LIMIT`]; the eye does not move.
    pub fn look(&mut self, counts: [f64; 2], sensitivity: f64) {
        self.yaw -= counts[0] * sensitivity;
        self.pitch = (self.pitch - counts[1] * sensitivity).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// The velocity the keys ask for at `speed` metres per second: W A S D along the heading and
    /// across it (two held together are no faster than one), Space and Shift along the world's
    /// z. As a creative-mode game flies: looking up or down does not change where W goes.
    #[must_use]
    pub fn wanted(&self, push: Push, speed: f64) -> [f64; 3] {
        let (forward, right) = self.heading();
        let scale = speed / push.forward.hypot(push.right).max(1.0);
        [
            (forward[0] * push.forward + right[0] * push.right) * scale,
            (forward[1] * push.forward + right[1] * push.right) * scale,
            push.up * speed,
        ]
    }

    /// Moves for `dt` seconds (at most [`MAX_DT`] of them), the velocity easing toward `wanted`
    /// (m/s) with time constant `easing` seconds: [`ease`] on each axis. With nothing wanted, a
    /// velocity under 0.1 mm/s is rest.
    pub fn advance(&mut self, wanted: [f64; 3], dt: f64, easing: f64) {
        let dt = dt.clamp(0.0, MAX_DT);
        for axis in 0..3 {
            let (velocity, moved) = ease(self.velocity[axis], wanted[axis], dt, easing);
            self.velocity[axis] = velocity;
            self.eye[axis] += moved;
        }
        if wanted == [0.0; 3] && self.velocity.iter().all(|v| v.abs() < 1e-4) {
            self.velocity = [0.0; 3];
        }
    }
}

/// Looking round from where the eye is by angles: turned `right` degrees to the right and `up`
/// degrees up, the eye kept (the arrows while flying).
#[must_use]
pub fn look(view: &ViewSpec, pane: usize, right: f64, up: f64) -> ViewCommand {
    let mut camera = FirstPerson::of(view, [0.0; 3]);
    camera.look([right, -up], 1.0);
    camera.command(view, pane)
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

    fn start() -> ViewSpec {
        ViewSpec {
            target: [1.0, 2.0, 3.0],
            yaw: 30.0,
            pitch: -20.0,
            distance: 5.0,
            ..ViewSpec::home(&SceneSpec::room())
        }
    }

    fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }

    fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    /// Flies from `start()` with `push` held for `seconds` at `fps` frames a second, each frame
    /// made from the view and written back as its one command, as the window does.
    fn flown(push: Push, speed: f64, seconds: f64, fps: f64, easing: f64) -> (ViewSpec, [f64; 3]) {
        let (mut view, mut velocity) = (start(), [0.0; 3]);
        let frames = (seconds * fps).round() as usize;
        for _ in 0..frames {
            let mut camera = FirstPerson::of(&view, velocity);
            camera.advance(camera.wanted(push, speed), 1.0 / fps, easing);
            velocity = camera.velocity;
            view = apply(&view, &camera.command(&view, 0));
        }
        (view, velocity)
    }

    /// How far along the starting heading `view`'s eye is from `start()`'s, metres.
    fn along_heading(view: &ViewSpec) -> f64 {
        dot(sub(eye(view), eye(&start())), FirstPerson::of(&start(), [0.0; 3]).heading().0)
    }

    #[test]
    fn fly_moves_by_the_integrated_speed() {
        // S18 C5, rewritten for the owner's ruling of 25 Nov 2026 (TASK-S20-10: "make it work
        // like an FPS game camera"): W goes along the view's *horizontal* heading whatever the
        // pitch, Space and Shift are up and down (E and Q were), the velocity eases (so the
        // distance is the eased integral, not speed × time), and Shift ×10 / Alt ×0.1 are gone
        // for flying (sprint is a factor on the speed). With no easing the old arithmetic holds:
        // N steps move the eye by speed × N × dt, to 1e-9 m.
        let settings = Settings::from_data();
        let speed = settings.fly_speed;
        let (heading, right) = FirstPerson::of(&start(), [0.0; 3]).heading();
        let moved = |push: Push, speed: f64| sub(eye(&flown(push, speed, 2.0, 60.0, 0.0).0), eye(&start()));
        let forward = moved(Push { forward: 1.0, ..Push::default() }, speed);
        assert!((dot(forward, heading) - speed * 2.0).abs() < 1e-9, "{forward:?}");
        assert!(dot(forward, right).abs() < 1e-9 && forward[2].abs() < 1e-9, "no drift across or up");
        // Sprinting is exactly its factor.
        let fast = moved(Push { forward: 1.0, ..Push::default() }, speed * settings.sprint);
        assert!((dot(fast, heading) / (speed * 2.0) - settings.sprint).abs() < 1e-9);
        // Space rises along the world's z whatever the pitch; D goes right; W and D together are
        // no faster than one of them.
        let up = moved(Push { up: 1.0, ..Push::default() }, speed);
        assert!((up[2] - speed * 2.0).abs() < 1e-9 && up[0].abs() < 1e-9 && up[1].abs() < 1e-9);
        let side = moved(Push { right: 1.0, ..Push::default() }, speed);
        assert!((dot(side, right) - speed * 2.0).abs() < 1e-9);
        let both = moved(Push { forward: 1.0, right: 1.0, up: 0.0 }, speed);
        assert!((dot(both, both).sqrt() - speed * 2.0).abs() < 1e-9, "the diagonal is no faster");
        // The angles and the distance are kept.
        let (view, _) = flown(Push { forward: 1.0, ..Push::default() }, speed, 2.0, 60.0, 0.0);
        assert!((view.yaw - 30.0).abs() < 1e-12 && (view.pitch + 20.0).abs() < 1e-12);
        assert!((view.distance - 5.0).abs() < 1e-12);
        // Control: no push, no move.
        assert_eq!(moved(Push::default(), speed), [0.0; 3]);
    }

    #[test]
    fn first_person_fly_is_frame_rate_independent() {
        // C1. Criterion (written before measuring): 2 s of W at 30 and at 144 frames a second
        // end within 1 % of each other and of speed × (t − τ(1 − e^(−t/τ))); after the key is let
        // go the speed is under 1 mm/s within 1 s. Control: steps of a fixed size a frame (no
        // frame time) differ by the frame rates' ratio.
        let settings = Settings::from_data();
        assert!((settings.easing - 0.08).abs() < 1e-12, "the brief's time constant");
        let (speed, tau, t) = (settings.fly_speed, settings.easing, 2.0);
        let w = Push { forward: 1.0, ..Push::default() };
        let run = |fps: f64| {
            let (view, velocity) = flown(w, speed, t, fps, tau);
            (along_heading(&view), velocity)
        };
        let (slow, _) = run(30.0);
        let (fast, at_speed) = run(144.0);
        let exact = speed * (t - tau * (1.0 - (-t / tau).exp()));
        assert!((slow / fast - 1.0).abs() < 0.01, "30 and 144 frames a second: {slow} {fast}");
        assert!(
            (slow / exact - 1.0).abs() < 0.01 && (fast / exact - 1.0).abs() < 0.01,
            "{slow} {fast} {exact}"
        );
        // (The integration is exact, so they agree far inside the criterion.)
        assert!((slow - fast).abs() < 1e-9 && (fast - exact).abs() < 1e-9, "{slow} {fast} {exact}");
        // It eases in: after one time constant it has moved speed·τ/e, not speed·τ.
        let early = along_heading(&flown(w, speed, tau, 1000.0, tau).0);
        assert!((early / (speed * tau * (-1.0_f64).exp()) - 1.0).abs() < 0.01, "{early}");
        // Let go: under 1 mm/s within a second, at either frame rate, and then exactly at rest.
        for fps in [30.0, 144.0] {
            let mut camera = FirstPerson::of(&start(), at_speed);
            for _ in 0..(fps as usize) {
                camera.advance([0.0; 3], 1.0 / fps, tau);
            }
            let left = dot(camera.velocity, camera.velocity).sqrt();
            assert!(left < 1e-3, "{left} m/s after 1 s at {fps} frames a second");
            assert_eq!(camera.velocity, [0.0; 3], "and it comes to rest");
        }
        // A frame longer than MAX_DT moves as one of MAX_DT.
        let mut long = FirstPerson::of(&start(), [0.0; 3]);
        let mut capped = long;
        long.advance(long.wanted(w, speed), 5.0, tau);
        capped.advance(capped.wanted(w, speed), MAX_DT, tau);
        assert_eq!(long, capped);
        // Control: a fixed step a frame, as a camera that ignores the frame's time takes, ends
        // 144/30 times as far at 144 frames a second.
        let fixed = |fps: f64| (t * fps).round() * speed * (1.0 / 60.0);
        assert!((fixed(144.0) / fixed(30.0) - 144.0 / 30.0).abs() < 1e-9);
        assert!((fixed(144.0) / fixed(30.0) - 1.0).abs() > 0.01, "the control fails the criterion");
    }

    #[test]
    fn looking_clamps_pitch_and_never_rolls() {
        // C2. Criterion: a look of 10 000 counts up leaves the pitch at +89°, the camera's right
        // vector horizontal to 1e-12, the eye unmoved to 1e-9 m. Control: a look of 100 counts
        // (10° at the default) is not clamped, and turns by exactly counts × sensitivity.
        let settings = Settings::from_data();
        let view = start();
        let mut camera = FirstPerson::of(&view, [0.0; 3]);
        camera.look([0.0, -10_000.0], settings.sensitivity);
        assert_eq!(camera.pitch, PITCH_LIMIT);
        let looked = apply(&view, &camera.command(&view, 0));
        assert_eq!(looked.pitch, -89.0, "the view's pitch is the eye's elevation over the target");
        let (to_eye, right, up) = looked.basis();
        assert!(right[2].abs() < 1e-12, "no roll: the picture's right is horizontal ({right:?})");
        assert!(up[2] > 0.0, "and its up is not below the horizon");
        assert!(to_eye[2] < 0.0, "it looks up: the target is above the eye");
        let (a, b) = (eye(&view), eye(&looked));
        assert!(a.iter().zip(b).all(|(p, q)| (p - q).abs() < 1e-9), "the eye stays: {a:?} {b:?}");
        // Down as far the other way; a wild mixed look never rolls or passes the limit.
        camera.look([123_456.0, 50_000.0], settings.sensitivity);
        assert_eq!(camera.pitch, -PITCH_LIMIT);
        let wild = apply(&view, &camera.command(&view, 0));
        assert!(wild.basis().1[2].abs() < 1e-12 && wild.pitch == 89.0);
        assert!(eye(&wild).iter().zip(a).all(|(p, q)| (p - q).abs() < 1e-9));
        // Control: a small look is not clamped. Right turns right (the heading swings toward
        // the picture's right), up looks up.
        let mut small = FirstPerson::of(&view, [0.0; 3]);
        let (heading, right) = small.heading();
        small.look([100.0, -100.0], settings.sensitivity);
        assert!((small.pitch - (20.0 + 100.0 * settings.sensitivity)).abs() < 1e-12);
        assert!((small.yaw - (30.0 - 100.0 * settings.sensitivity)).abs() < 1e-12);
        assert!(dot(small.heading().0, right) > 0.0 && dot(small.heading().0, heading) > 0.9);
    }

    #[test]
    fn the_camera_s_settings_are_data() {
        // `ui/camera.ron` parses, every setting the console names is a field within its own
        // range, and a value outside the range or an unknown name is refused, nothing changed.
        let mut settings = Settings::from_data();
        for (name, _, lo, hi, _) in SETTINGS {
            let value = settings.get(name).unwrap_or_else(|| panic!("{name} is no field"));
            assert!((lo..=hi).contains(&value), "{name} = {value}");
        }
        let before = settings.clone();
        assert!(settings.set("sensitivity", 0.0).unwrap_err().contains("from 0.001"));
        assert!(settings.set("roll", 1.0).unwrap_err().contains("camera-set <sensitivity|"));
        assert_eq!(settings, before);
        settings.set("sensitivity", 0.25).unwrap();
        assert!((settings.sensitivity - 0.25).abs() < 1e-15);
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
        // (TASK-S20-10: `look` takes degrees to the right and up, as a first-person camera
        // turns; it took a yaw and a pitch of the orbit's, whose up was the other way.)
        let looked = apply(&view, &look(&view, 0, -15.0, 5.0));
        let (a, b) = (eye(&view), eye(&looked));
        assert!(a.iter().zip(b).all(|(p, q)| (p - q).abs() < 1e-12), "the eye stays: {a:?} {b:?}");
        assert!((looked.yaw - view.yaw - 15.0).abs() < 1e-12);
        assert!((looked.pitch - view.pitch + 5.0).abs() < 1e-12, "up lowers the eye's elevation");
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
