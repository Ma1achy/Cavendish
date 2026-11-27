//! The physical camera (`docs/slices/S20.md` §2, "The walker"; the owner's ruling of 25 Nov
//! 2026): a player-sized box that walks, jumps, steps up, slides along walls and flies, against
//! whatever is solid. Pure arithmetic: nothing here knows the engine, the scene or the window.
//! What is solid is asked of an [`Occupancy`]; the scene's is `crate::occupancy`.
//!
//! Frames and units: metres in the world's frame (x east, y north, z up), seconds, metres per
//! second. The box has its faces along the axes and never turns.
//!
//! **How a step moves.** The velocity is brought up to date for the frame's time (the horizontal
//! part eases toward the wanted one, [`crate::camera::ease`]; walking, the vertical part falls
//! under gravity, integrated exactly; flying, it eases too). The move is then made one axis at a
//! time, up-and-down first, each axis *swept*: the solid boxes in the whole region the move
//! covers are asked for once, and the move along an axis is cut short at the nearest face in its
//! way. So the box ends touching what stopped it (within [`EPS`]), never inside it, at any
//! speed: nothing is skipped between frames. A move longer than [`MAX_SWEEP`] is made in parts,
//! which bounds the region asked for and changes nothing else.
//!
//! **Choices recorded.**
//! - *Starting inside matter* (walk mode entered with the eye in a wall, noclip turned off in
//!   one, a body moved onto the camera): the box is lifted to the nearest free place straight
//!   above ([`Walker::lift`]). Out of a building that is its roof; out of the ground, the
//!   surface.
//! - *Stepping up*: walking on the ground into something no higher than one voxel of what it
//!   stands on or of what is in the way (the larger, at most [`Settings::step_cap`]) climbs it.
//! - *Sneaking* walks slower and does not walk off an edge more than one step high.
//! - *Landing while flying down* (Shift held) is walking again, as a creative-mode game has it.
//! - *Noclip* is no collision at all, and so no ground to walk on: it implies flying.

use serde::Deserialize;

use crate::camera::{MAX_DT, ease};

/// How close two faces are to count as touching, metres. A box rests *on* a face, to rounding;
/// tests of "inside" allow this much.
pub const EPS: f64 = 1e-9;

/// The longest move swept in one part, metres: longer moves are made as several, so that the
/// region whose solids are asked for stays small. (At 1000 m/s and a 0.1 s frame: 50 parts.)
pub const MAX_SWEEP: f64 = 2.0;

/// The greatest speed of a fall, metres per second.
pub const TERMINAL: f64 = 100.0;

/// A box with its faces along the axes, metres in the world's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    /// The corner with the least x, y and z.
    pub min: [f64; 3],
    /// The corner with the greatest.
    pub max: [f64; 3],
}

impl Aabb {
    /// Whether the two boxes share more than a face: they overlap by more than [`EPS`] on every
    /// axis.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        (0..3).all(|a| self.min[a] < other.max[a] - EPS && self.max[a] > other.min[a] + EPS)
    }

    /// The box moved by `by` metres.
    #[must_use]
    pub fn moved(&self, by: [f64; 3]) -> Self {
        Self {
            min: [self.min[0] + by[0], self.min[1] + by[1], self.min[2] + by[2]],
            max: [self.max[0] + by[0], self.max[1] + by[1], self.max[2] + by[2]],
        }
    }

    /// The box moved `by` metres along `axis` (0 x, 1 y, 2 z).
    #[must_use]
    pub fn along(&self, axis: usize, by: f64) -> Self {
        let mut step = [0.0; 3];
        step[axis] = by;
        self.moved(step)
    }

    /// The least box holding both.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        Self {
            min: [0, 1, 2].map(|a| self.min[a].min(other.min[a])),
            max: [0, 1, 2].map(|a| self.max[a].max(other.max[a])),
        }
    }

    /// The box grown by `by[a]` metres on both sides of each axis.
    #[must_use]
    pub fn grown(&self, by: [f64; 3]) -> Self {
        Self { min: [0, 1, 2].map(|a| self.min[a] - by[a]), max: [0, 1, 2].map(|a| self.max[a] + by[a]) }
    }
}

/// One solid box, and what it is made of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solid {
    /// Where it is, metres in the world's frame.
    pub bounds: Aabb,
    /// The side of a voxel of the matter it is part of, metres: what "a step of one voxel" is
    /// for it. Zero for matter not made of voxels (a floor, a wall).
    pub voxel: f64,
}

/// What is solid: the walker's only knowledge of the scene.
pub trait Occupancy {
    /// Adds to `out` every solid box that overlaps `region` (metres, the world's frame). Boxes
    /// may reach outside it, and may overlap one another; one that does not touch the region
    /// may be added too (it costs time, not correctness). All matter in the region must be
    /// covered.
    fn solids(&self, region: &Aabb, out: &mut Vec<Solid>);
}

/// Nothing solid anywhere.
pub struct Void;

impl Occupancy for Void {
    fn solids(&self, _region: &Aabb, _out: &mut Vec<Solid>) {}
}

/// The walker's settings (`ui/camera.ron`, under `walk`; `camera-set` changes one).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// The box's side along x and along y, metres.
    pub width: f64,
    /// The box's height, metres.
    pub height: f64,
    /// How far above its feet the eye is, metres; below `height`.
    pub eye_height: f64,
    /// The walking speed, metres per second.
    pub speed: f64,
    /// What sprinting multiplies the walking speed by.
    pub sprint: f64,
    /// What sneaking multiplies the walking speed by.
    pub sneak: f64,
    /// How high a jump lifts the feet, metres.
    pub jump_height: f64,
    /// The highest step climbed, metres, whatever the voxels' size.
    pub step_cap: f64,
}

/// What the keys ask of the walker for one step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wish {
    /// The horizontal velocity wanted, `[x, y]` metres per second (sprint and sneak already in
    /// it).
    pub along: [f64; 2],
    /// Flying: the vertical velocity wanted, metres per second, up positive. Not read walking.
    pub up: f64,
    /// Walking: jump, if on the ground.
    pub jump: bool,
    /// Walking: sneak (do not walk off an edge).
    pub sneak: bool,
}

/// What a step needs of its surroundings besides the matter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Env {
    /// The acceleration of gravity, metres per second squared, down positive (the scene's
    /// `array.instrument.gravity`).
    pub gravity: f64,
    /// The time constant with which the velocity eases toward the wanted one, seconds.
    pub easing: f64,
}

/// The physical camera: a box that stands on its feet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Walker {
    /// The middle of the box's underside, metres in the world's frame.
    pub feet: [f64; 3],
    /// Its velocity, metres per second.
    pub velocity: [f64; 3],
    /// Whether the last step ended standing on something.
    pub on_ground: bool,
    /// Whether it flies (no gravity; Space and Shift go up and down) rather than walks.
    pub flying: bool,
    /// Whether nothing stops it (which implies flying).
    pub noclip: bool,
    /// The voxel size of what it last stood on, metres (0 for none).
    pub stood_on: f64,
}

impl Default for Walker {
    /// At the origin, at rest, flying, colliding.
    fn default() -> Self {
        Self {
            feet: [0.0; 3],
            velocity: [0.0; 3],
            on_ground: false,
            flying: true,
            noclip: false,
            stood_on: 0.0,
        }
    }
}

fn horizontal(v: [f64; 3]) -> f64 {
    v[0].hypot(v[1])
}

/// How far of `d` metres along `axis` the box `b` can move before a face of `solids` is in its
/// way, and which solid stopped it (if one did).
fn clip(b: &Aabb, axis: usize, d: f64, solids: &[Solid]) -> (f64, Option<usize>) {
    if d == 0.0 {
        return (0.0, None);
    }
    let (o1, o2) = ((axis + 1) % 3, (axis + 2) % 3);
    let (mut best, mut by) = (d, None);
    for (n, solid) in solids.iter().enumerate() {
        let x = &solid.bounds;
        let beside = |o: usize| b.min[o] < x.max[o] - EPS && b.max[o] > x.min[o] + EPS;
        if !(beside(o1) && beside(o2)) {
            continue;
        }
        if d > 0.0 && x.min[axis] >= b.max[axis] - EPS {
            let gap = (x.min[axis] - b.max[axis]).max(0.0);
            if gap < best {
                (best, by) = (gap, Some(n));
            }
        } else if d < 0.0 && x.max[axis] <= b.min[axis] + EPS {
            let gap = (x.max[axis] - b.min[axis]).min(0.0);
            if gap > best {
                (best, by) = (gap, Some(n));
            }
        }
    }
    (best, by)
}

impl Walker {
    /// A walker whose eye is at `eye` (metres, the world's frame), at rest.
    #[must_use]
    pub fn at_eye(eye: [f64; 3], settings: &Settings) -> Self {
        Self { feet: [eye[0], eye[1], eye[2] - settings.eye_height], ..Self::default() }
    }

    /// Where its eye is, metres in the world's frame.
    #[must_use]
    pub fn eye(&self, settings: &Settings) -> [f64; 3] {
        [self.feet[0], self.feet[1], self.feet[2] + settings.eye_height]
    }

    /// Its box.
    #[must_use]
    pub fn bounds(&self, settings: &Settings) -> Aabb {
        let half = 0.5 * settings.width;
        Aabb {
            min: [self.feet[0] - half, self.feet[1] - half, self.feet[2]],
            max: [self.feet[0] + half, self.feet[1] + half, self.feet[2] + settings.height],
        }
    }

    /// Whether its box is inside matter of `world` (by more than [`EPS`]).
    #[must_use]
    pub fn inside(&self, world: &dyn Occupancy, settings: &Settings) -> bool {
        let (b, mut solids) = (self.bounds(settings), Vec::new());
        world.solids(&b, &mut solids);
        solids.iter().any(|s| s.bounds.overlaps(&b))
    }

    /// Lifts the box straight up until it is in no matter of `world`: onto the highest top of
    /// what it is in, again and again (at most 256 times; a box still inside matter then stays
    /// where that left it). Its fall is stopped.
    pub fn lift(&mut self, world: &dyn Occupancy, settings: &Settings, solids: &mut Vec<Solid>) {
        for _ in 0..256 {
            let b = self.bounds(settings);
            solids.clear();
            world.solids(&b, solids);
            let top = solids
                .iter()
                .filter(|s| s.bounds.overlaps(&b))
                .map(|s| s.bounds.max[2])
                .fold(f64::NEG_INFINITY, f64::max);
            if top == f64::NEG_INFINITY {
                break;
            }
            self.feet[2] = top;
            self.velocity[2] = 0.0;
        }
    }

    /// One step of `dt` seconds (at most [`MAX_DT`] of them): the velocity brought toward what
    /// `wish` asks, and the box moved through `world` (see the module's notes). With
    /// [`Self::noclip`] nothing stops it.
    pub fn step(&mut self, dt: f64, wish: &Wish, world: &dyn Occupancy, settings: &Settings, env: Env) {
        let dt = dt.clamp(0.0, MAX_DT);
        self.flying |= self.noclip;
        let mut d = [0.0; 3];
        for axis in 0..2 {
            (self.velocity[axis], d[axis]) = ease(self.velocity[axis], wish.along[axis], dt, env.easing);
        }
        if self.flying {
            (self.velocity[2], d[2]) = ease(self.velocity[2], wish.up, dt, env.easing);
        } else {
            let gravity = env.gravity.max(0.0);
            if wish.jump && self.on_ground {
                self.velocity[2] = (2.0 * gravity * settings.jump_height).sqrt();
                self.on_ground = false;
            }
            // Constant acceleration, integrated exactly: the mean of the speeds times the time.
            let before = self.velocity[2];
            self.velocity[2] = (before - gravity * dt).max(-TERMINAL);
            d[2] = 0.5 * (before + self.velocity[2]) * dt;
        }
        if wish.along == [0.0; 2] && horizontal(self.velocity) < 1e-4 {
            self.velocity[0] = 0.0;
            self.velocity[1] = 0.0;
        }
        if self.flying && wish.up == 0.0 && self.velocity[2].abs() < 1e-4 {
            self.velocity[2] = 0.0;
        }
        if self.noclip {
            for axis in 0..3 {
                self.feet[axis] += d[axis];
            }
            self.on_ground = false;
            return;
        }
        let length = d[0].hypot(d[1]).hypot(d[2]);
        let parts = (length / MAX_SWEEP).ceil().max(1.0);
        let part = d.map(|x| x / parts);
        let mut solids = Vec::new();
        for _ in 0..parts as usize {
            self.sweep(part, wish, world, settings, &mut solids);
        }
    }

    /// One swept move of `d` metres (no longer than [`MAX_SWEEP`]), axis by axis.
    fn sweep(
        &mut self,
        d: [f64; 3],
        wish: &Wish,
        world: &dyn Occupancy,
        settings: &Settings,
        solids: &mut Vec<Solid>,
    ) {
        let start = self.bounds(settings);
        // Everything the move could touch: the box here and there, a step's height above (to
        // climb) and below (to know an edge), and a hair round.
        let reach = settings.step_cap + 0.1;
        let region = start.union(&start.moved(d)).grown([1e-6, 1e-6, reach]);
        solids.clear();
        world.solids(&region, solids);
        if solids.iter().any(|s| s.bounds.overlaps(&start)) {
            // Inside matter: out of it first, upward; the move waits for the next step.
            self.lift(world, settings, solids);
            self.on_ground = false;
            return;
        }
        let mut b = start;
        // Up and down first, so that a landing is known before the walk along the ground.
        let (dz, under) = clip(&b, 2, d[2], solids);
        b = b.along(2, dz);
        if dz != d[2] {
            self.velocity[2] = 0.0;
        }
        let landed = d[2] < 0.0 && dz != d[2];
        if landed {
            self.stood_on = under.map_or(0.0, |n| solids[n].voxel);
            if self.flying && wish.up < 0.0 {
                self.flying = false;
            }
        }
        if !self.flying || d[2] != 0.0 {
            self.on_ground = landed;
        }
        let walking = !self.flying && self.on_ground;
        let (mut dx, mut dy) = (d[0], d[1]);
        if walking && wish.sneak {
            (dx, dy) = Self::held_back(&b, dx, dy, self.stood_on.min(settings.step_cap), solids);
        }
        let ground = b;
        let (mx, by_x) = clip(&b, 0, dx, solids);
        b = b.along(0, mx);
        let (my, by_y) = clip(&b, 1, dy, solids);
        b = b.along(1, my);
        let (mut mx, mut my) = (mx, my);
        if walking && (mx != dx || my != dy) {
            // In the way: one voxel of what is stood on, or of what is in the way, is climbed.
            let in_the_way = [by_x, by_y].into_iter().flatten().map(|n| solids[n].voxel).fold(0.0, f64::max);
            let height = self.stood_on.max(in_the_way).min(settings.step_cap);
            if height > 0.0 {
                let (up, _) = clip(&ground, 2, height, solids);
                let mut c = ground.along(2, up);
                let (ux, _) = clip(&c, 0, dx, solids);
                c = c.along(0, ux);
                let (uy, _) = clip(&c, 1, dy, solids);
                c = c.along(1, uy);
                let (down, onto) = clip(&c, 2, -up, solids);
                c = c.along(2, down);
                if ux.hypot(uy) > mx.hypot(my) + EPS {
                    (b, mx, my) = (c, ux, uy);
                    if let Some(n) = onto {
                        self.stood_on = solids[n].voxel;
                    }
                }
            }
        }
        if mx != dx {
            self.velocity[0] = 0.0;
        }
        if my != dy {
            self.velocity[1] = 0.0;
        }
        let half = 0.5 * settings.width;
        self.feet = [b.min[0] + half, b.min[1] + half, b.min[2]];
    }

    /// Sneaking: how much of a walk of `(dx, dy)` metres keeps something under the box `b`
    /// within a step of `step` metres below it. Each part is shortened 5 cm at a time, as far as
    /// nothing, until what is left ends over ground.
    fn held_back(b: &Aabb, dx: f64, dy: f64, step: f64, solids: &[Solid]) -> (f64, f64) {
        let drop = step.max(0.05) + 2.0 * EPS;
        let held = |x: f64, y: f64| {
            let lowered = b.moved([x, y, -drop]);
            solids.iter().any(|s| s.bounds.overlaps(&lowered))
        };
        let shorter = |v: f64| if v.abs() <= 0.05 { 0.0 } else { v - 0.05 * v.signum() };
        let (mut dx, mut dy) = (dx, dy);
        while dx != 0.0 && !held(dx, 0.0) {
            dx = shorter(dx);
        }
        while dy != 0.0 && !held(0.0, dy) {
            dy = shorter(dy);
        }
        while dx != 0.0 && dy != 0.0 && !held(dx, dy) {
            (dx, dy) = (shorter(dx), shorter(dy));
        }
        (dx, dy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A handful of boxes.
    struct Blocks(Vec<Solid>);

    impl Occupancy for Blocks {
        fn solids(&self, region: &Aabb, out: &mut Vec<Solid>) {
            out.extend(self.0.iter().filter(|s| s.bounds.grown([EPS; 3]).overlaps(region)));
        }
    }

    const VOXEL: f64 = 0.25;

    fn solid(min: [f64; 3], max: [f64; 3]) -> Solid {
        Solid { bounds: Aabb { min, max }, voxel: VOXEL }
    }

    /// A floor whose top is z = 0, 200 m square.
    fn floor() -> Solid {
        solid([-100.0, -100.0, -1.0], [100.0, 100.0, 0.0])
    }

    fn settings() -> Settings {
        crate::camera::Settings::from_data().walk
    }

    const ENV: Env = Env { gravity: 9.81, easing: 0.08 };
    const DT: f64 = 1.0 / 60.0;

    fn standing(at: [f64; 3]) -> Walker {
        Walker { feet: at, flying: false, on_ground: true, ..Walker::default() }
    }

    /// Steps `walker` for `seconds` with `wish`, checking after every frame that it is in no
    /// matter. Returns the highest its feet were.
    fn run(walker: &mut Walker, world: &dyn Occupancy, wish: Wish, seconds: f64, dt: f64) -> f64 {
        let s = settings();
        let mut highest = walker.feet[2];
        for frame in 0..(seconds / dt).round() as usize {
            walker.step(dt, &wish, world, &s, ENV);
            assert!(walker.noclip || !walker.inside(world, &s), "inside matter at frame {frame}: {walker:?}");
            highest = highest.max(walker.feet[2]);
        }
        highest
    }

    #[test]
    fn the_walker_obeys_its_box() {
        // W1. Criteria (written before measuring): on a floor it rests with its eye at eye height
        // to 1e-9 m; walking into a wall it stops with its box 0 to 1 mm clear and slides along
        // it at the tangential speed; a step of one voxel is climbed, one of two is not; a jump
        // peaks at the stated height within 2 %; it is inside matter at no frame. Controls: with
        // nothing solid it falls for ever; with noclip it passes the wall.
        let s = settings();
        assert_eq!((s.width, s.height, s.eye_height), (0.6, 1.8, 1.62), "the brief's player");
        assert_eq!((s.speed, s.sprint, s.jump_height, s.step_cap), (4.3, 1.3, 1.25, 0.6));
        let still = Wish::default();

        // Dropped from half a metre, it rests on the floor, its eye at eye height.
        let world = Blocks(vec![floor()]);
        let mut walker = Walker { feet: [0.0, 0.0, 0.5], flying: false, ..Walker::default() };
        run(&mut walker, &world, still, 2.0, DT);
        assert!(walker.on_ground && walker.velocity == [0.0; 3], "{walker:?}");
        assert!((walker.eye(&s)[2] - s.eye_height).abs() < 1e-9, "the eye rests at {}", walker.eye(&s)[2]);
        // Control: with nothing solid it falls for ever (to the terminal speed, and on).
        let mut falling = Walker { feet: [0.0, 0.0, 0.5], flying: false, ..Walker::default() };
        run(&mut falling, &Void, still, 30.0, DT);
        assert!(!falling.on_ground && falling.feet[2] < -2000.0, "{falling:?}");
        assert!((falling.velocity[2] + TERMINAL).abs() < 1e-9);

        // A wall across x = 3: walking north-east into it stops the box at its face and slides
        // north at the wanted northward speed.
        let wall = solid([3.0, -100.0, 0.0], [4.0, 100.0, 5.0]);
        let world = Blocks(vec![floor(), wall]);
        let wish = Wish { along: [s.speed, 1.5], ..Wish::default() };
        let mut walker = standing([0.0, 0.0, 0.0]);
        run(&mut walker, &world, wish, 3.0, DT);
        let clear = 3.0 - walker.bounds(&s).max[0];
        assert!((-EPS..=1e-3).contains(&clear), "the box is {clear} m from the wall");
        assert!(
            walker.velocity[0] == 0.0 && (walker.velocity[1] - 1.5).abs() < 1e-9,
            "{:?}",
            walker.velocity
        );
        let y = walker.feet[1];
        run(&mut walker, &world, wish, 1.0, DT);
        assert!((walker.feet[1] - y - 1.5).abs() < 1e-9, "it slides at the tangential speed");
        assert!(walker.on_ground && walker.feet[2].abs() < 1e-9);
        // Control: with noclip the same walk passes through the wall.
        let mut ghost = Walker { noclip: true, ..standing([0.0, 0.0, 0.0]) };
        run(&mut ghost, &world, wish, 3.0, DT);
        assert!(ghost.feet[0] > 5.0 && ghost.flying, "noclip passes, flying: {ghost:?}");

        // A step of one voxel is climbed; of two, not.
        let east = Wish { along: [s.speed, 0.0], ..Wish::default() };
        for (voxels, climbed) in [(1.0, true), (2.0, false)] {
            let step = solid([2.0, -100.0, 0.0], [100.0, 100.0, voxels * VOXEL]);
            let world = Blocks(vec![floor(), step]);
            let mut walker = standing([0.0, 0.0, 0.0]);
            run(&mut walker, &world, east, 2.0, DT);
            if climbed {
                assert!(walker.feet[0] > 4.0 && (walker.feet[2] - VOXEL).abs() < 1e-9, "{walker:?}");
            } else {
                let clear = 2.0 - walker.bounds(&s).max[0];
                assert!((-EPS..=1e-3).contains(&clear) && walker.feet[2].abs() < 1e-9, "{walker:?}");
            }
        }
        // (A step higher than the cap is not climbed, whatever the voxel's size.)
        let tall = Solid { voxel: 1.0, ..solid([2.0, -100.0, 0.0], [100.0, 100.0, 1.0]) };
        let mut walker = standing([0.0, 0.0, 0.0]);
        run(&mut walker, &Blocks(vec![Solid { voxel: 1.0, ..floor() }, tall]), east, 2.0, DT);
        assert!(walker.feet[0] < 2.0 && walker.feet[2].abs() < 1e-9, "{walker:?}");

        // A jump peaks at the stated height, and only from the ground.
        let world = Blocks(vec![floor()]);
        let jump = Wish { jump: true, ..Wish::default() };
        let mut walker = standing([0.0, 0.0, 0.0]);
        walker.step(DT, &jump, &world, &s, ENV);
        let peak = run(&mut walker, &world, still, 2.0, DT);
        assert!((peak / s.jump_height - 1.0).abs() < 0.02, "the jump peaked at {peak} m");
        assert!(walker.on_ground && walker.feet[2].abs() < 1e-9, "and came down");
        let mut airborne = Walker { feet: [0.0, 0.0, 3.0], flying: false, ..Walker::default() };
        airborne.step(DT, &jump, &world, &s, ENV);
        assert!(airborne.velocity[2] < 0.0, "no jump from the air");
    }

    #[test]
    fn nothing_is_skipped_at_any_speed() {
        // The sweep's bound: flying at 1000 m/s, in frames of 0.1 s (100 m a frame), a wall 1 cm
        // thick still stops the box at its face. Control: a walker that only tests where it
        // ends (the same move with nothing between) is past the wall.
        let s = settings();
        let wall = solid([500.0, -100.0, -100.0], [500.01, 100.0, 100.0]);
        let world = Blocks(vec![wall]);
        let wish = Wish { along: [1000.0, 0.0], ..Wish::default() };
        let mut walker = Walker { feet: [0.0, 0.0, 0.0], ..Walker::default() };
        run(&mut walker, &world, wish, 2.0, MAX_DT);
        let clear = 500.0 - walker.bounds(&s).max[0];
        assert!((-EPS..=1e-3).contains(&clear), "stopped {clear} m from the wall");
        let mut naive = Walker { noclip: true, ..Walker::default() };
        let mut found = false;
        for _ in 0..20 {
            naive.step(MAX_DT, &wish, &world, &s, ENV);
            found |= naive.inside(&world, &s);
        }
        assert!(!found && naive.feet[0] > 501.0, "control: testing only where a frame ends skips the wall");
    }

    #[test]
    fn flying_collides_lands_and_sneaking_keeps_the_edge() {
        let s = settings();
        // Flying: no gravity; Space and Shift go up and down; a ceiling and a wall stop it.
        let ceiling = solid([-100.0, -100.0, 5.0], [100.0, 100.0, 6.0]);
        let wall = solid([3.0, -100.0, 0.0], [4.0, 100.0, 5.0]);
        let world = Blocks(vec![floor(), ceiling, wall]);
        let mut walker = Walker { feet: [0.0, 0.0, 2.0], ..Walker::default() };
        run(&mut walker, &world, Wish::default(), 1.0, DT);
        assert!((walker.feet[2] - 2.0).abs() < 1e-12 && walker.flying, "it hovers");
        run(&mut walker, &world, Wish { up: 4.0, along: [4.0, 0.0], ..Wish::default() }, 2.0, DT);
        assert!((5.0 - walker.bounds(&s).max[2]).abs() < 1e-9, "the ceiling stops it");
        assert!((3.0 - walker.bounds(&s).max[0]).abs() < 1e-9, "and the wall");
        // Flying down onto the ground is walking again.
        run(&mut walker, &world, Wish { up: -4.0, ..Wish::default() }, 2.0, DT);
        assert!(!walker.flying && walker.on_ground && walker.feet[2].abs() < 1e-9, "{walker:?}");
        // Control: drifting onto the ground without asking to go down does not land.
        let mut hovering = Walker { feet: [0.0, 0.0, 0.0], ..Walker::default() };
        run(&mut hovering, &world, Wish { along: [-1.0, 0.0], ..Wish::default() }, 1.0, DT);
        assert!(hovering.flying);

        // Sneaking does not walk off the edge of a platform 2 m up; walking does.
        let platform = solid([-1.0, -1.0, 0.0], [2.0, 1.0, 2.0]);
        let world = Blocks(vec![floor(), platform]);
        let east = [s.speed * s.sneak, 0.0];
        let mut sneaking = standing([0.0, 0.0, 2.0]);
        run(&mut sneaking, &world, Wish { along: east, sneak: true, ..Wish::default() }, 6.0, DT);
        assert!((sneaking.feet[2] - 2.0).abs() < 1e-9 && sneaking.on_ground, "{sneaking:?}");
        assert!(
            sneaking.feet[0] > 1.9 && sneaking.bounds(&s).min[0] < 2.0,
            "at the edge: {:?}",
            sneaking.feet
        );
        let mut walking = standing([0.0, 0.0, 2.0]);
        run(&mut walking, &world, Wish { along: east, ..Wish::default() }, 6.0, DT);
        assert!(walking.feet[2].abs() < 1e-9 && walking.feet[0] > 2.3, "control: walking falls off");

        // Starting inside matter, it is lifted to the nearest free place above: the top.
        let block = solid([-2.0, -2.0, 0.0], [2.0, 2.0, 7.0]);
        let world = Blocks(vec![floor(), block]);
        let mut buried = standing([0.0, 0.0, 1.0]);
        assert!(buried.inside(&world, &s));
        buried.step(DT, &Wish::default(), &world, &s, ENV);
        assert!(!buried.inside(&world, &s) && (buried.feet[2] - 7.0).abs() < 1e-9, "{buried:?}");
    }
}
