//! The first-person camera in the window (`docs/slices/S20.md` §2, the owner's rulings of 25 Nov
//! 2026: "make it work like a fps game camera", and a physical camera "like a creative-mode
//! game"): what a session holds between frames, the keys it takes, its frame, and the capture of
//! the pointer. The arithmetic is [`crate::camera::FirstPerson`] (the look, the keys' wish) and
//! [`crate::walk::Walker`] (the box that moves through the scene's matter,
//! [`crate::occupancy::SceneOccupancy`]); this is the window's side of them.
//!
//! **The camera's modes.** Orbit (the default: a drag turns the camera about what it looks at),
//! and two first-person ones, *fly* and *walk*, which are one session with a switch.
//!
//! **Entering and leaving.** Holding the right button over a scene viewport flies it until the
//! button is let go ([`Shell::mouse_fly`]); `fly` (Shift+F) and `walk` (Shift+G), also in the
//! header's camera menu, start a session on the active viewport that lasts until the same word
//! again, Esc or Enter (a [`crate::keyboard::Session::Fly`]). The window losing its focus ends
//! either. The camera stays where it was taken. One session at a time, on the viewport it
//! started in.
//!
//! **While it is on** the pointer is captured and hidden ([`crate::platform::capture`]) and the
//! mouse looks by its raw motion ([`crate::platform::motion`]). W A S D move along the view's
//! heading and across it; Ctrl held or a double tap of W sprints; the wheel changes the flying
//! speed, which is then shown in the canvas for a moment. *Flying*, Space rises and Shift sinks
//! (E and Q too), and flying down onto the ground is walking again. *Walking*, Space jumps and
//! Shift sneaks (slower, and not off an edge). A double tap of Space switches between the two.
//! Every key is the session's: no tool, scope, selection or transport shortcut fires (W is not
//! Move, Space does not play, Ctrl+S does not save). Only the actions of [`WHILE_FLYING`] keep
//! their keys.
//!
//! **The camera is physical**: a box the size of a person ([`crate::walk::Settings`]) that the
//! scene's matter stops, walking or flying, unless `noclip` (N) is on ([`Shell::noclip`]: off by
//! default, marked in the canvas while on; it implies flying). It has no mass: it reads the
//! scene to collide and writes only the view. Each frame issues at most one
//! [`cav_engine::ViewCommand::Camera`]: journalled as any camera move is (a run of them is one
//! journal entry), never an edit, never undone.
//!
//! **The near plane.** The view's near plane is a hundredth of its distance to the target, so
//! from an orbit's distance a wall at arm's length would be cut away. A session therefore holds
//! the target [`EYE_DISTANCE`] in front of the eye (the eye where it was) and puts the orbit's
//! distance back when it ends. An orthographic view is made perspective for it.

use cav_engine::{Command, ViewCommand};
use cav_sim::{Projection, ViewSpec};
use egui::Key;

use crate::camera::{FLY_SPEED_RANGE, FirstPerson, PITCH_LIMIT, Push};
use crate::keyboard::Press;
use crate::occupancy::SceneOccupancy;
use crate::shell::Shell;
use crate::walk::{Env, Walker, Wish};

/// The actions whose keys still work in a first-person session: `fly` and `walk` themselves
/// (Shift+F, Shift+G: switch or stop), `noclip` (N), the window filling the screen (F11) and
/// hiding the panels (F3). Everything else waits for the session to end: a key held to move
/// must never also save, undo, play or take a tool.
pub const WHILE_FLYING: [&str; 5] = ["fly", "walk", "noclip", "fullscreen", "gui"];

/// How far an arrow key turns the look in a session, degrees a step.
pub const LOOK_STEP: f64 = 1.0;

/// How far in front of the eye a session holds the view's target, metres: the near plane is
/// then 2 cm from the eye, well inside the camera's box (half of 0.6 m to its faces), so a wall
/// the box touches is never cut away.
pub const EYE_DISTANCE: f64 = 1.0;

/// The frame's time `walk-for` steps by, seconds.
pub const SCRIPT_DT: f64 = 1.0 / 60.0;

/// What a first-person session holds between frames. Window state: none of it is in the scene,
/// the journal or a saved file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Flight {
    /// The viewport (pane, from 0) the session is on.
    pub pane: usize,
    /// The camera's box: its velocity, whether it flies or walks, whether it stands. Its place
    /// is taken from the view each frame.
    pub walker: Walker,
    /// Whether a double tap of W has the camera sprinting, until W is let go.
    pub sprinting: bool,
    /// When W was last pressed, seconds of the window's clock.
    last_forward: Option<f64>,
    /// When Space was last pressed (and not yet half of a double tap), seconds.
    last_space: Option<f64>,
    /// Until when the speed is shown in the canvas, seconds of the window's clock.
    pub speed_shown_until: f64,
    /// The speed is to be shown from the next frame (a command changed it, with no clock).
    show_speed: bool,
    /// The view's distance to its target before the session, metres: put back when it ends.
    distance: Option<f64>,
    /// Shift is still down from the chord that started the session (Shift+F, Shift+G): it is
    /// not "down" or "sneak" until it has been let go once.
    shift_from_chord: bool,
    /// Whether the pointer is captured and hidden now (what was last asked of the platform).
    pub captured: bool,
}

/// What one frame's input holds for a session.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Held {
    /// The keys held: W/S, D/A, and Space or E against Shift or Q.
    pub push: Push,
    /// Whether W is held (a double tap's sprint lasts until it is not).
    pub forward_down: bool,
    /// Whether Ctrl is held: sprint.
    pub sprint: bool,
    /// The frame's time, seconds.
    pub dt: f64,
}

impl Held {
    /// The keys named in `words` held for a frame of [`SCRIPT_DT`]: `w`, `a`, `s`, `d`, `space`,
    /// `shift`, `ctrl` (`walk-for`'s).
    ///
    /// # Errors
    /// A word that is none of them.
    pub fn of_words<'a>(words: impl Iterator<Item = &'a str>) -> Result<Self, String> {
        let mut held = Self { dt: SCRIPT_DT, ..Self::default() };
        for word in words {
            match word.to_lowercase().as_str() {
                "w" => held.push.forward += 1.0,
                "s" => held.push.forward -= 1.0,
                "d" => held.push.right += 1.0,
                "a" => held.push.right -= 1.0,
                "space" => held.push.up += 1.0,
                "shift" => held.push.up -= 1.0,
                "ctrl" => held.sprint = true,
                other => return Err(format!("`{other}` is no key of w, a, s, d, space, shift, ctrl")),
            }
        }
        held.forward_down = held.push.forward > 0.0;
        Ok(held)
    }
}

/// A speed as the canvas shows it: `4.0 m/s`, with the digits its size needs.
#[must_use]
pub fn speed_text(speed: f64) -> String {
    if speed < 1.0 {
        format!("{speed:.2} m/s")
    } else if speed < 100.0 {
        format!("{speed:.1} m/s")
    } else {
        format!("{speed:.0} m/s")
    }
}

impl Shell {
    /// The viewport (pane, from 0) a first-person session is on, if one is.
    #[must_use]
    pub fn fly_pane(&self) -> Option<usize> {
        self.mouse_fly.or_else(|| self.flying().then_some(self.flight.pane))
    }

    /// Whether a first-person session is on and walking (not flying).
    #[must_use]
    pub fn walking(&self) -> bool {
        self.any_fly() && !self.flight.walker.flying
    }

    /// Ends the session, however it was started; the eye stays where it is and the view's
    /// distance to its target is put back. The pointer is given back at the end of the frame
    /// ([`Self::fly_capture`]).
    pub fn end_fly(&mut self) {
        if !self.any_fly() {
            return;
        }
        self.mouse_fly = None;
        if self.flying() {
            self.keyboard.session = None;
            self.keyboard.mode = crate::keyboard::Mode::Navigation;
        }
        let pane = self.flight.pane;
        if let Some(distance) = self.flight.distance.take()
            && pane < self.engine.panes()
        {
            let view = self.engine.view(pane).clone();
            let far = ViewSpec { distance, ..view.clone() };
            self.apply(Command::View(FirstPerson::of(&view, [0.0; 3]).command(&far, pane)));
        }
        self.flight = Flight { captured: self.flight.captured, pane, ..Flight::default() };
    }

    /// Begins a session on `pane`, flying or walking, the camera's box at rest. Nothing of the
    /// view changes until the camera first looks or moves ([`Self::fly_step`]): a click of the
    /// right button, which opens the viewport's menu, is a session of no frames and leaves the
    /// view exactly as it was.
    pub(crate) fn enter_first_person(&mut self, pane: usize, flying: bool) {
        self.flight = Flight {
            pane,
            walker: Walker { flying: flying || self.noclip, ..Walker::default() },
            captured: self.flight.captured,
            shift_from_chord: true,
            ..Flight::default()
        };
    }

    /// `fly` (`flying`) or `walk`: with no session on, starts one on the active viewport, the
    /// canvas taking the keys in interaction mode; in a session of the other kind, switches to
    /// this one; in a session of this kind, ends it.
    ///
    /// # Errors
    /// When the active viewport shows no scene: there is no camera to take.
    pub(crate) fn first_person(&mut self, flying: bool) -> Result<(), String> {
        if self.any_fly() {
            if self.flight.walker.flying == flying {
                self.end_fly();
            } else {
                self.set_flying(flying);
            }
            return Ok(());
        }
        let pane = self.engine.active_pane();
        if self.engine.content(pane) != cav_engine::PaneContent::Scene {
            let word = if flying { "fly" } else { "walk" };
            return Err(format!("{word}: the active viewport shows no scene"));
        }
        self.keyboard.focus = vec!["viewport".into(), "viewport/canvas".into()];
        self.keyboard.mode = crate::keyboard::Mode::Interaction;
        self.keyboard.session = Some(crate::keyboard::Session::Fly);
        self.keyboard.nav_on |= self.by_keyboard;
        if !flying {
            // Walking is on the ground: nothing to walk on with no collision.
            self.noclip = false;
        }
        self.enter_first_person(pane, flying);
        Ok(())
    }

    /// Switches the session between flying and walking. Walking turns noclip off (with no
    /// collision there is no ground); the fall or the climb under way is stopped.
    pub(crate) fn set_flying(&mut self, flying: bool) {
        self.flight.walker.flying = flying;
        self.flight.walker.velocity[2] = 0.0;
        if !flying {
            self.noclip = false;
        }
    }

    /// `noclip`: whether the scene's matter stops the camera. On, a walking camera flies.
    pub(crate) fn toggle_noclip(&mut self) {
        self.noclip = !self.noclip;
        if self.noclip {
            self.flight.walker.flying = true;
        }
    }

    /// The frame's key presses in a session (`presses`: each with whether it is a held key's
    /// repeat), at `now` seconds of the window's clock. Esc or Enter end it; a second press of W
    /// within the double tap's time sprints; a second press of Space switches flying and
    /// walking; the arrows look; a key of an action of [`WHILE_FLYING`] runs it; every other
    /// press does nothing (the held keys are read by [`Self::fly_frame`]).
    pub(crate) fn fly_keys(&mut self, presses: &[(Press, bool)], now: f64) {
        let shortcuts = crate::actions::table().shortcuts();
        for &(press, repeat) in presses {
            let pane = match self.fly_pane() {
                Some(pane) if pane < self.engine.panes() => pane,
                _ => return,
            };
            let arrow = match press.key {
                Key::ArrowLeft => Some((-LOOK_STEP, 0.0)),
                Key::ArrowRight => Some((LOOK_STEP, 0.0)),
                Key::ArrowUp => Some((0.0, LOOK_STEP)),
                Key::ArrowDown => Some((0.0, -LOOK_STEP)),
                _ => None,
            };
            if let Some((right, up)) = arrow {
                let command = crate::camera::look(self.engine.view(pane), pane, right, up);
                self.apply(Command::View(command));
                continue;
            }
            if repeat {
                continue;
            }
            let within = self.camera.double_tap;
            let tapped = |last: Option<f64>| last.is_some_and(|before| now - before <= within);
            match press.key {
                Key::Escape | Key::Enter => {
                    self.end_fly();
                    return;
                }
                Key::W => {
                    self.flight.sprinting |= tapped(self.flight.last_forward);
                    self.flight.last_forward = Some(now);
                }
                Key::Space if !press.modifiers.ctrl && !press.modifiers.command => {
                    if tapped(self.flight.last_space) {
                        // With noclip on there is nothing to walk on: it stays flying.
                        let flying = !self.flight.walker.flying || self.noclip;
                        self.flight.walker.flying = flying;
                        self.flight.walker.velocity[2] = 0.0;
                        self.flight.last_space = None;
                    } else {
                        self.flight.last_space = Some(now);
                    }
                }
                _ => {
                    let action = shortcuts
                        .iter()
                        .find(|s| WHILE_FLYING.contains(&s.action.as_str()) && s.chord.exactly(press))
                        .map(|s| s.action.clone());
                    if let Some(id) = action {
                        self.by_keyboard = true;
                        let _ = self.run_action(&id);
                        self.by_keyboard = false;
                    }
                }
            }
        }
    }

    /// One frame of a session, from the window's input: the mouse's raw motion looks, the held
    /// keys say where to go ([`Self::fly_step`]), and the wheel changes the flying speed.
    pub(crate) fn fly_frame(&mut self, ctx: &egui::Context) {
        let Some(pane) = self.fly_pane() else { return };
        if pane >= self.engine.panes()
            || self.engine.content(pane) != cav_engine::PaneContent::Scene
            || (self.flight.captured && crate::platform::capture_lost())
        {
            // The viewport went (a layout with fewer), or the browser took the pointer back.
            self.end_fly();
            return;
        }
        // Shift held over from the chord that began the session is not yet a key of it.
        self.flight.shift_from_chord &= ctx.input(|i| i.modifiers.shift);
        let chord = self.flight.shift_from_chord;
        let (held, scroll, now, pointer) = ctx.input(|i| {
            let down = |key: Key| f64::from(u8::from(i.key_down(key)));
            let rise = (down(Key::Space) + down(Key::E)).min(1.0);
            let shift = i.modifiers.shift && !chord;
            let sink = (f64::from(u8::from(shift)) + down(Key::Q)).min(1.0);
            let held = Held {
                push: Push {
                    forward: down(Key::W) - down(Key::S),
                    right: down(Key::D) - down(Key::A),
                    up: rise - sink,
                },
                forward_down: i.key_down(Key::W),
                sprint: i.modifiers.ctrl || i.modifiers.command,
                dt: f64::from(i.stable_dt),
            };
            let pointer = [f64::from(i.pointer.delta().x), f64::from(i.pointer.delta().y)];
            (held, f64::from(i.smooth_scroll_delta.y), i.time, pointer)
        });
        // The wheel is the session's wherever the pointer rests: nothing else scrolls or zooms.
        if scroll != 0.0 {
            ctx.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
            self.fly_speed =
                (self.fly_speed * (scroll * 0.002).exp()).clamp(FLY_SPEED_RANGE.0, FLY_SPEED_RANGE.1);
            self.flight.show_speed = true;
        }
        if std::mem::take(&mut self.flight.show_speed) {
            self.flight.speed_shown_until = now + self.camera.speed_shown;
        }
        // Raw motion where the platform has it (a captured pointer does not move); else the
        // pointer's own.
        let motion = crate::platform::motion(ctx).unwrap_or(pointer);
        if self.fly_step(pane, held, motion) {
            ctx.request_repaint();
        } else if now < self.flight.speed_shown_until {
            // The read-out goes when its time is up.
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                self.flight.speed_shown_until - now,
            ));
        }
    }

    /// One step of a session on `pane`: a look by `motion` (raw counts right and down), then
    /// the camera's box moved for `held.dt` seconds as the held keys ask, through the scene's
    /// matter ([`Walker::step`] against [`SceneOccupancy`]). At most one camera command. Returns
    /// whether the camera is still moving (another frame is wanted).
    pub(crate) fn fly_step(&mut self, pane: usize, held: Held, motion: [f64; 2]) -> bool {
        if !held.forward_down {
            self.flight.sprinting = false;
        }
        let sprinting = held.sprint || self.flight.sprinting;
        let settings = self.camera.clone();
        let mut view = self.engine.view(pane).clone();
        let mut walker = self.flight.walker;
        walker.noclip = self.noclip;
        walker.flying |= self.noclip;
        let mut camera = FirstPerson::of(&view, walker.velocity);
        let before = (camera.eye, camera.yaw, camera.pitch);
        if motion != [0.0; 2] {
            camera.look(motion, settings.sensitivity);
        }
        let wish = if walker.flying {
            let speed = self.fly_speed * if sprinting { settings.sprint } else { 1.0 };
            let wanted = camera.wanted(held.push, speed);
            Wish { along: [wanted[0], wanted[1]], up: wanted[2], ..Wish::default() }
        } else {
            // Walking, Space jumps and Shift sneaks.
            let sneak = held.push.up < 0.0;
            let pace = if sneak {
                settings.walk.sneak
            } else if sprinting {
                settings.walk.sprint
            } else {
                1.0
            };
            let wanted = camera.wanted(Push { up: 0.0, ..held.push }, settings.walk.speed * pace);
            Wish { along: [wanted[0], wanted[1]], up: 0.0, jump: held.push.up > 0.0, sneak }
        };
        walker.feet = [camera.eye[0], camera.eye[1], camera.eye[2] - settings.walk.eye_height];
        let env = Env { gravity: self.engine.spec().array.instrument.gravity, easing: settings.easing };
        // Hovering with nothing asked, nothing moves: the box is not even looked at (so a
        // session of no keys never lifts a camera that an orbit left inside matter).
        let idle = walker.flying && walker.velocity == [0.0; 3] && wish == Wish::default();
        if !idle {
            // The scene is read here, and only read.
            let world = SceneOccupancy::new(&self.engine, self.ground());
            walker.step(held.dt, &wish, &world, &settings.walk, env);
        }
        self.flight.walker = walker;
        // The eye follows the box where the box moved (more than rounding: the eye's place is
        // read from the view each frame, and its trip to the feet and back is not exact).
        let eye = walker.eye(&settings.walk);
        if !idle && (0..3).any(|a| (eye[a] - camera.eye[a]).abs() > 1e-12) {
            camera.eye = eye;
        }
        if (camera.eye, camera.yaw, camera.pitch) != before {
            if self.flight.distance.is_none() {
                // The session's first move: a perspective view, its target a metre before the
                // eye (the near plane; see the module's notes), the eye where it is.
                if view.projection == Projection::Orthographic {
                    let projection = Projection::Perspective;
                    self.apply(Command::View(ViewCommand::Project { pane, projection }));
                }
                self.flight.distance = Some(view.distance);
                view.distance = EYE_DISTANCE;
                camera.pitch = camera.pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT);
            }
            self.apply(Command::View(camera.command(&view, pane)));
        }
        walker.velocity != [0.0; 3] || held.push != Push::default() || !(walker.flying || walker.on_ground)
    }

    /// `walk-for <seconds> [keys]`: steps the session under way for `seconds` at 60 frames a
    /// second with the keys of `held` down, as a person holding them would: for scripts and
    /// pictures made with no window.
    ///
    /// # Errors
    /// When no session is on.
    pub(crate) fn walk_for(&mut self, seconds: f64, held: Held) -> Result<(), String> {
        let frames = (seconds / SCRIPT_DT).round() as usize;
        for _ in 0..frames {
            let Some(pane) = self.fly_pane().filter(|pane| *pane < self.engine.panes()) else {
                return Err("walk-for: no session is on (`walk` or `fly` starts one)".to_owned());
            };
            self.fly_step(pane, held, [0.0; 2]);
        }
        Ok(())
    }

    /// Shows the flying speed in the canvas for its moment, from the next frame (a command
    /// changed it).
    pub(crate) fn show_fly_speed(&mut self) {
        self.flight.show_speed = true;
    }

    /// Asks the platform for the pointer when a session has begun and gives it back when it
    /// has ended: once per change, at the end of the frame that changed it.
    pub(crate) fn fly_capture(&mut self, ctx: &egui::Context) {
        let flying = self.any_fly();
        if flying == self.flight.captured {
            return;
        }
        crate::platform::capture(ctx, flying);
        self.flight.captured = flying;
        if flying {
            // Nothing is being typed into while the keys fly.
            if let Some(id) = ctx.memory(egui::Memory::focused) {
                ctx.memory_mut(|m| m.surrender_focus(id));
            }
        }
        ctx.request_repaint();
    }

    /// The speed as the canvas of pane `pane` shows it at `now` seconds of the window's clock:
    /// for a moment after the wheel changed it in a session on that pane.
    #[must_use]
    pub fn fly_speed_shown(&self, pane: usize, now: f64) -> Option<String> {
        (self.fly_pane() == Some(pane) && (now < self.flight.speed_shown_until || self.flight.show_speed))
            .then(|| speed_text(self.fly_speed))
    }
}

/// While a `fly` or `walk` session is on (the pointer is captured, wherever it rests): a sheet
/// over the whole window that takes every click, so that a press of a mouse button acts on
/// nothing under the unseen pointer. (A flight by the held right button needs none: the canvas
/// holds the pointer until the button is let go.)
pub(crate) fn shield(shell: &Shell, ctx: &egui::Context) {
    if !shell.flying() {
        return;
    }
    egui::Area::new(egui::Id::unique("first-person shield"))
        .order(egui::Order::Foreground)
        .fixed_pos(ctx.content_rect().min)
        .show(ctx, |ui| {
            ui.allocate_response(ctx.content_rect().size(), egui::Sense::click_and_drag());
        });
}

/// The session's marks in the canvas `rect` of pane `pane`, small at the middle of its foot:
/// the speed, for a moment after it changed, and `noclip` while that is on.
pub(crate) fn read_out(shell: &mut Shell, ui: &egui::Ui, pane: usize, rect: egui::Rect) {
    if shell.fly_pane() != Some(pane) {
        return;
    }
    let now = ui.input(|i| i.time);
    let painter = ui.painter_at(rect);
    let pad = egui::vec2(crate::theme::GRID * 1.5, crate::theme::GRID * 0.5);
    let marks = [
        ("fly speed", shell.fly_speed_shown(pane, now), egui::Color32::WHITE),
        ("noclip mark", shell.noclip.then(|| "noclip".to_owned()), crate::theme::WARN),
    ];
    let galleys: Vec<_> = marks
        .into_iter()
        .filter_map(|(name, text, colour)| {
            Some((name, painter.layout_no_wrap(text?, crate::theme::mono(), colour)))
        })
        .collect();
    let gap = crate::theme::GRID;
    let width: f32 = galleys.iter().map(|(_, g)| g.size().x + 2.0 * pad.x + gap).sum::<f32>() - gap;
    let mut left = rect.center().x - 0.5 * width;
    for (name, galley) in galleys {
        let size = galley.size() + 2.0 * pad;
        let plate = egui::Rect::from_min_size(
            egui::pos2(left, rect.bottom() - size.y - 3.0 * crate::theme::GRID),
            size,
        );
        painter.rect_filled(plate, 3.0, crate::theme::FOOTER.gamma_multiply(0.85));
        painter.galley(plate.min + pad, galley, egui::Color32::WHITE);
        shell.layout.buttons.push((name.to_owned(), plate));
        left += size.x + gap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_speed_reads_with_the_digits_its_size_needs() {
        assert_eq!(speed_text(4.0), "4.0 m/s");
        assert_eq!(speed_text(0.05), "0.05 m/s");
        assert_eq!(speed_text(12.34), "12.3 m/s");
        assert_eq!(speed_text(1000.0), "1000 m/s");
    }

    #[test]
    fn only_harmless_actions_keep_their_keys_in_flight() {
        // Every action allowed in flight exists and has a key, and none of those keys is one the
        // session holds to move (W A S D, Space, E, Q; Shift and Ctrl are held with them, so a
        // chord of Shift or Ctrl with one of those letters would fire by accident). None of them
        // is a tool, an edit, the transport or a file (the control: `play`, `undo`, `save` and
        // the Move tool are not among them).
        let table = crate::actions::table();
        let moving = [Key::W, Key::A, Key::S, Key::D, Key::Space, Key::E, Key::Q];
        for id in WHILE_FLYING {
            let action = table.action(id).unwrap_or_else(|| panic!("no action `{id}`"));
            assert!(!action.keys.is_empty(), "{id} has no key");
            for key in &action.keys {
                let chord = crate::keyboard::Chord::parse(key).unwrap();
                for held in moving {
                    for modifiers in [egui::Modifiers::NONE, egui::Modifiers::SHIFT, egui::Modifiers::CTRL] {
                        assert!(
                            !chord.exactly(Press::with(held, modifiers)),
                            "{id}'s {key} is a key held to move"
                        );
                    }
                }
            }
        }
        for id in ["play", "undo", "save", "tool-move", "delete"] {
            assert!(!WHILE_FLYING.contains(&id));
        }
    }

    #[test]
    fn walk_for_s_keys_are_the_held_keys() {
        let held = Held::of_words("w d shift ctrl".split(' ')).unwrap();
        assert_eq!(held.push, Push { forward: 1.0, right: 1.0, up: -1.0 });
        assert!(held.forward_down && held.sprint && (held.dt - SCRIPT_DT).abs() < 1e-15);
        assert!(Held::of_words(["x"].into_iter()).unwrap_err().contains("`x` is no key"));
    }
}
