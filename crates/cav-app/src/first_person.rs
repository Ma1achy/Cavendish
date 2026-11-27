//! The first-person camera in the window (`docs/slices/S20.md` §2, the owner's ruling of 25 Nov
//! 2026: "make it work like a fps game camera"): what a fly session holds between frames, the
//! keys it takes, its frame, and the capture of the pointer. The arithmetic is
//! [`crate::camera::FirstPerson`]; this is the window's side of it.
//!
//! **Entering and leaving.** Holding the right button over a scene viewport flies it until the
//! button is let go ([`Shell::mouse_fly`]); `fly` (Shift+F, the header's camera menu) flies the
//! active viewport until `fly` again, Esc or Enter (a [`crate::keyboard::Session::Fly`]). The
//! window losing its focus ends either. The camera stays where it was flown.
//!
//! **While flying** the pointer is captured and hidden ([`crate::platform::capture`]); the mouse
//! looks by its raw motion ([`crate::platform::motion`]); W A S D move along the view's heading
//! and across it, Space rises and Shift sinks (E and Q too), Ctrl held or a double tap of W
//! sprints, the wheel changes the speed, which is then shown in the canvas for a moment. Every
//! key is the flight's: no tool, scope, selection or transport shortcut fires (W is not Move,
//! Space does not play, Ctrl+S does not save). Only the actions of [`WHILE_FLYING`] keep their
//! keys. Each frame issues at most one [`cav_engine::ViewCommand::Camera`]: journalled as any camera move is
//! (a run of them is one journal entry), never an edit, never undone.

use cav_engine::Command;
use egui::Key;

use crate::camera::{FLY_SPEED_RANGE, FirstPerson, Push};
use crate::keyboard::Press;
use crate::shell::Shell;

/// The actions whose keys still work while flying: `fly` itself (Shift+F: stop), the window
/// filling the screen (F11) and hiding the panels (F3). Everything else waits for the flight to
/// end: a key held to move must never also save, undo, play or take a tool.
pub const WHILE_FLYING: [&str; 3] = ["fly", "fullscreen", "gui"];

/// How far an arrow key turns the look while flying, degrees a step.
pub const LOOK_STEP: f64 = 1.0;

/// What a fly session holds between frames. Window state: none of it is in the scene, the
/// journal or a saved file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Flight {
    /// The viewport (pane, from 0) a session started by `fly` flies.
    pub pane: usize,
    /// The eye's velocity, metres per second in the world's frame: what eases.
    pub velocity: [f64; 3],
    /// Whether a double tap of W has the camera sprinting, until W is let go.
    pub sprinting: bool,
    /// When W was last pressed, seconds of the window's clock.
    last_forward: Option<f64>,
    /// Until when the speed is shown in the canvas, seconds of the window's clock.
    pub speed_shown_until: f64,
    /// Whether the pointer is captured and hidden now (what was last asked of the platform).
    pub captured: bool,
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
    /// The viewport (pane, from 0) being flown, if one is.
    #[must_use]
    pub fn fly_pane(&self) -> Option<usize> {
        self.mouse_fly.or_else(|| self.flying().then_some(self.flight.pane))
    }

    /// Ends the flight, however it was started; the camera stays where it is. The pointer is
    /// given back at the end of the frame ([`Self::fly_capture`]).
    pub fn end_fly(&mut self) {
        self.mouse_fly = None;
        if self.flying() {
            self.keyboard.session = None;
            self.keyboard.mode = crate::keyboard::Mode::Navigation;
        }
    }

    /// Starts a fly session on the active viewport (`fly`): the canvas takes the keys, in
    /// interaction mode.
    ///
    /// # Errors
    /// When the active viewport shows no scene: there is no camera to fly.
    pub(crate) fn start_fly(&mut self) -> Result<(), String> {
        let pane = self.engine.active_pane();
        if self.engine.content(pane) != cav_engine::PaneContent::Scene {
            return Err("fly: the active viewport shows no scene".to_owned());
        }
        self.flight.pane = pane;
        self.keyboard.focus = vec!["viewport".into(), "viewport/canvas".into()];
        self.keyboard.mode = crate::keyboard::Mode::Interaction;
        self.keyboard.session = Some(crate::keyboard::Session::Fly);
        self.keyboard.nav_on |= self.by_keyboard;
        Ok(())
    }

    /// The frame's key presses while flying (`presses`: each with whether it is a held key's
    /// repeat), at `now` seconds of the window's clock. Esc or Enter end the flight; a second
    /// press of W within the double tap's time sprints; the arrows look; a key of an action of
    /// [`WHILE_FLYING`] runs it; every other press does nothing (the held keys are read by
    /// [`Self::fly_frame`]).
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
            match press.key {
                Key::Escape | Key::Enter => {
                    self.end_fly();
                    return;
                }
                Key::W => {
                    let tapped =
                        self.flight.last_forward.is_some_and(|before| now - before <= self.camera.double_tap);
                    self.flight.sprinting |= tapped;
                    self.flight.last_forward = Some(now);
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

    /// One frame of a flight: the mouse's raw motion looks, the held keys set the velocity
    /// wanted, the velocity eases toward it and moves the eye for the frame's time
    /// ([`FirstPerson`]), and the wheel changes the speed. At most one camera command.
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
        let held = ctx.input(|i| {
            let down = |key: Key| f64::from(u8::from(i.key_down(key)));
            let rise = (down(Key::Space) + down(Key::E)).min(1.0);
            let sink = (f64::from(u8::from(i.modifiers.shift)) + down(Key::Q)).min(1.0);
            Held {
                push: Push {
                    forward: down(Key::W) - down(Key::S),
                    right: down(Key::D) - down(Key::A),
                    up: rise - sink,
                },
                forward_down: i.key_down(Key::W),
                sprint: i.modifiers.ctrl || i.modifiers.command,
                dt: f64::from(i.stable_dt),
                scroll: f64::from(i.smooth_scroll_delta.y),
                now: i.time,
                pointer: [f64::from(i.pointer.delta().x), f64::from(i.pointer.delta().y)],
            }
        });
        // The wheel is the flight's wherever the pointer rests: nothing else scrolls or zooms.
        if held.scroll != 0.0 {
            ctx.input_mut(|i| {
                i.smooth_scroll_delta = egui::Vec2::ZERO;
            });
            self.fly_speed =
                (self.fly_speed * (held.scroll * 0.002).exp()).clamp(FLY_SPEED_RANGE.0, FLY_SPEED_RANGE.1);
            self.flight.speed_shown_until = held.now + self.camera.speed_shown;
        }
        if !held.forward_down {
            self.flight.sprinting = false;
        }
        let sprint = if held.sprint || self.flight.sprinting { self.camera.sprint } else { 1.0 };
        // Raw motion where the platform has it (a captured pointer does not move); else the
        // pointer's own.
        let motion = crate::platform::motion(ctx).unwrap_or(held.pointer);
        let view = self.engine.view(pane).clone();
        let before = FirstPerson::of(&view, self.flight.velocity);
        let mut camera = before;
        camera.look(motion, self.camera.sensitivity);
        camera.advance(camera.wanted(held.push, self.fly_speed * sprint), held.dt, self.camera.easing);
        self.flight.velocity = camera.velocity;
        if (camera.eye, camera.yaw, camera.pitch) != (before.eye, before.yaw, before.pitch) {
            self.apply(Command::View(camera.command(&view, pane)));
        }
        if camera.velocity != [0.0; 3] || held.push != Push::default() {
            ctx.request_repaint();
        } else if held.now < self.flight.speed_shown_until {
            // The read-out goes when its time is up.
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                self.flight.speed_shown_until - held.now,
            ));
        }
    }

    /// Asks the platform for the pointer when a flight has begun and gives it back when it has
    /// ended: once per change, at the end of the frame that changed it.
    pub(crate) fn fly_capture(&mut self, ctx: &egui::Context) {
        let flying = self.any_fly();
        if flying == self.flight.captured {
            return;
        }
        crate::platform::capture(ctx, flying);
        self.flight = Flight {
            captured: flying,
            pane: self.flight.pane,
            speed_shown_until: if flying { self.flight.speed_shown_until } else { 0.0 },
            ..Flight::default()
        };
        if flying {
            // Nothing is being typed into while the keys fly.
            if let Some(id) = ctx.memory(egui::Memory::focused) {
                ctx.memory_mut(|m| m.surrender_focus(id));
            }
        }
        ctx.request_repaint();
    }

    /// The speed as the canvas of pane `pane` shows it at `now` seconds of the window's clock:
    /// for a moment after the wheel changed it in a flight of that pane.
    #[must_use]
    pub fn fly_speed_shown(&self, pane: usize, now: f64) -> Option<String> {
        (self.fly_pane() == Some(pane) && now < self.flight.speed_shown_until)
            .then(|| speed_text(self.fly_speed))
    }
}

/// What one frame's input holds for a flight.
struct Held {
    push: Push,
    forward_down: bool,
    sprint: bool,
    /// The frame's time, seconds.
    dt: f64,
    /// The wheel's movement this frame, points.
    scroll: f64,
    /// The window's clock, seconds.
    now: f64,
    /// How far the pointer itself moved this frame, points.
    pointer: [f64; 2],
}

/// While a `fly` session is on (the pointer is captured, wherever it rests): a sheet over the
/// whole window that takes every click, so that a press of a mouse button acts on nothing
/// under the unseen pointer. (A flight by the held right button needs none: the canvas holds
/// the pointer until the button is let go.)
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

/// The flight's read-out in the canvas `rect` of pane `pane`: the speed, for a moment after the
/// wheel changed it, small at the middle of the canvas's foot.
pub(crate) fn read_out(shell: &mut Shell, ui: &egui::Ui, pane: usize, rect: egui::Rect) {
    let now = ui.input(|i| i.time);
    let Some(text) = shell.fly_speed_shown(pane, now) else { return };
    let painter = ui.painter_at(rect);
    let galley = painter.layout_no_wrap(text, crate::theme::mono(), egui::Color32::WHITE);
    let pad = egui::vec2(crate::theme::GRID * 1.5, crate::theme::GRID * 0.5);
    let size = galley.size() + 2.0 * pad;
    let at = egui::pos2(rect.center().x - 0.5 * size.x, rect.bottom() - size.y - 3.0 * crate::theme::GRID);
    let plate = egui::Rect::from_min_size(at, size);
    painter.rect_filled(plate, 3.0, crate::theme::FOOTER.gamma_multiply(0.85));
    painter.galley(plate.min + pad, galley, egui::Color32::WHITE);
    shell.layout.buttons.push(("fly speed".to_owned(), plate));
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
        // Every action allowed in flight exists and has a key; none of them is a tool, an edit,
        // the transport or a file (the control: `play`, `undo`, `save` and the Move tool are
        // not among them).
        let table = crate::actions::table();
        for id in WHILE_FLYING {
            let action = table.action(id).unwrap_or_else(|| panic!("no action `{id}`"));
            assert!(!action.keys.is_empty(), "{id} has no key");
        }
        for id in ["play", "undo", "save", "tool-move", "delete"] {
            assert!(!WHILE_FLYING.contains(&id));
        }
    }
}
