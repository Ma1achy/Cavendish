//! The world streamed and drawn as a camera flies over `town`: what the camera drew stays drawn
//! (TASK-S4-26, the owner's report of chunks going black and coming back on the live page).
//!
//! The streaming (`cav_engine::world::Streaming`) and the renderer's GPU copies
//! (`cav_render::world::WorldGpu`) are driven frame by frame as the application drives them, with
//! the streaming's work a frame fixed (a count, not a time) so that the run is the same on every
//! machine: a native frame's worth and a page's (one thread, a few a frame).

use std::collections::HashSet;

use cav_core::voxel::world::ChunkKey;
use cav_engine::world::Streaming;
use cav_gpu::Gpu;
use cav_render::world::{WorldGpu, frustum_planes, visible};
use cav_render::{Camera, Renderer};
use cav_sim::world::clipmap::{MAX_LEVELS, ancestor};
use cav_sim::{DrawStyle, NamedView, PaneContent, Projection, SceneSpec, ViewSpec};

type Item = (u8, ChunkKey);

/// The camera at `s` (0 to 1 a lap) along a circle of 60 m round the array, low, looking along
/// the way it goes.
fn camera_at(template: &ViewSpec, s: f64) -> ViewSpec {
    let angle = std::f64::consts::TAU * s;
    let mut view = template.clone();
    view.target = [60.0 * angle.cos(), 60.0 * angle.sin(), 0.0];
    view.yaw = (angle + std::f64::consts::FRAC_PI_2).to_degrees() + 180.0;
    view.pitch = 14.0;
    view.distance = 24.0;
    view
}

/// What one flight found: frames; items drawn in a frame and gone the next while their box was
/// still in view, with nothing drawn over it and the world still holding matter there
/// (`vanished`: the defect); such items where the world, at the level now wanted, holds none
/// (`phantoms`: a coarse brick's overreach, rightly gone); the frames whose far field met its
/// budget (`cut`: not counted); the frames with any vanished; and the most items drawn at once.
#[derive(Debug, Default)]
struct Flight {
    frames: usize,
    vanished: usize,
    phantoms: usize,
    cut: usize,
    frames_with: usize,
    most_drawn: usize,
}

/// Flies `frames` frames at 15 m/s (a frame a sixtieth of a second), the streaming making at
/// most `builds` a frame, and counts what vanished.
fn fly(
    gpu: &Gpu,
    renderer: &Renderer,
    spec: &SceneSpec,
    frames: usize,
    builds: usize,
    stand_ins: bool,
) -> Flight {
    let size = [640u32, 360];
    let aspect = f64::from(size[0]) / f64::from(size[1]);
    let template = ViewSpec::framing(spec, NamedView::Top, Projection::Perspective, DrawStyle::Shaded);
    let mut streaming = Streaming::default();
    let mut world = WorldGpu::default();
    world.without_stand_ins = !stand_ins;
    let (pipelines, _) = renderer.world_parts();
    let contents = [PaneContent::Scene];
    // The first view made whole and drawn whole, as a load does.
    let first = camera_at(&template, 0.0);
    streaming.refresh(spec, std::slice::from_ref(&first), &contents);
    while streaming.pending() > 0 {
        streaming.step(usize::MAX, f64::INFINITY);
    }
    let frame = streaming.frame().expect("town has a world");
    world.prepare(gpu, pipelines, frame, &first, &Camera::new(&first, aspect), size, true);
    let mut last: Vec<Item> = world.drawn.clone();
    let mut previous = first.clone();
    let mut flight = Flight::default();
    let lap = std::f64::consts::TAU * 60.0;
    for f in 1..=frames {
        let view = camera_at(&template, f as f64 * 15.0 / 60.0 / lap);
        // As the engine does: a refresh a view command, so a pan then an orbit (the pan alone
        // drops some of what the orbit wants again, which is made again).
        let mut panned = previous.clone();
        panned.target = view.target;
        streaming.refresh(spec, std::slice::from_ref(&panned), &contents);
        streaming.refresh(spec, std::slice::from_ref(&view), &contents);
        previous = view.clone();
        streaming.step(builds, f64::INFINITY);
        let frame = streaming.frame().expect("town has a world");
        let camera = Camera::new(&view, aspect);
        world.prepare(gpu, pipelines, frame, &view, &camera, size, false);
        let grid = frame.grid.expect("a grid");
        let planes = frustum_planes(&camera.view_proj).map(|p| p.map(|v| v as f32));
        let seen = |(level, key): Item| {
            let b = grid.brick_box(level, key);
            let lo = b.lo - camera.origin;
            visible(&planes, [lo.x as f32, lo.y as f32, lo.z as f32], grid.brick_side(level) as f32)
        };
        let drawn = world.drawn.clone();
        flight.frames += 1;
        flight.most_drawn = flight.most_drawn.max(drawn.len());
        let now: HashSet<Item> = drawn.iter().copied().collect();
        // Everything drawn now and every brick holding something drawn now.
        let mut under: HashSet<Item> = HashSet::new();
        for &(level, key) in &drawn {
            for to in level..MAX_LEVELS {
                under.insert((to, ancestor(level, key, to)));
            }
        }
        // What the world has in view now, at the levels now wanted: resident or waiting to be
        // made; and every brick holding one. (A coarse box in view may hold finer ones out of it:
        // what is out of view is not lost from the picture.)
        let held = frame.chunks.keys().map(|k| (0, *k)).chain(frame.bricks.keys().copied());
        let matter: HashSet<Item> =
            held.chain(frame.waiting.iter().copied()).filter(|item| seen(*item)).collect();
        let mut matter_under: HashSet<Item> = HashSet::new();
        for &(level, key) in &matter {
            for to in level..MAX_LEVELS {
                if !matter_under.insert((to, ancestor(level, key, to))) {
                    break;
                }
            }
        }
        let (mut gone, mut phantoms) = (0, 0);
        for &(level, key) in &last {
            // Whether an item of `set` is this one or holds it.
            let over = |set: &HashSet<Item>| {
                (level..MAX_LEVELS).any(|to| set.contains(&(to, ancestor(level, key, to))))
            };
            if !seen((level, key)) || under.contains(&(level, key)) || over(&now) {
                continue;
            }
            // A coarse brick's matter that the finer level has not (a brick's cell is filled when
            // a piece overlaps it at all, a chunk's when its centre is inside; `plan/IDEAS.md`):
            // nothing there to draw now, which is right.
            if matter_under.contains(&(level, key)) || over(&matter) {
                gone += 1;
            } else {
                phantoms += 1;
            }
        }
        // A frame whose far field met its budget leaves its farthest bricks out, and says so
        // (`MAX_BRICK_OVERDRAW`): a bound, not this defect, so such frames are counted apart.
        if world.notes.iter().any(|n| n.starts_with("far field cut")) {
            flight.cut += 1;
            last = drawn;
            continue;
        }
        flight.vanished += gone;
        flight.phantoms += phantoms;
        flight.frames_with += usize::from(gone > 0);
        last = drawn;
    }
    flight
}

#[test]
fn gpu_a_moving_camera_never_loses_what_it_drew() {
    let gpu = Gpu::new().expect("a GPU");
    let renderer = Renderer::new(&gpu);
    let mut spec = cav_sim::load("town").expect("town");
    // A coarser error than town's 4 px, so that each level is 8 bricks either side, not 16: the
    // same snaps and handovers with far fewer items, so that the test is quick in the tier's
    // build. (Town's own ring was measured too, in release: TASK-S4-26 in `docs/slices/S4.md`.)
    spec.world.as_mut().expect("town has a world").lod.error_px = 16.0;
    // 160 frames at 15 m/s: 40 m round the array, six times across a level-0 snap (6.4 m).
    let frames = 160;
    // The page's rate of building (one thread, a few a frame), at which a hole lasts many
    // frames: what the owner saw. (At a native frame's worth, 256, a hole lasts a frame or two;
    // measured in release over town's own ring, TASK-S4-26 in `docs/slices/S4.md`.)
    let builds = 6;
    let started = std::time::Instant::now();
    let fixed = fly(&gpu, &renderer, &spec, frames, builds, true);
    println!("with stand-ins: {fixed:?} in {:.1} s", started.elapsed().as_secs_f64());
    assert_eq!(fixed.frames, frames);
    assert!(fixed.cut < frames / 2, "most frames are counted: {fixed:?}");
    assert!(fixed.most_drawn > 100, "the flight draws the town: {fixed:?}");
    assert_eq!(fixed.vanished, 0, "something drawn went with nothing over it: {fixed:?}");
    // Control: the handover as it was (each level's own items only) loses what it drew, in
    // bursts at the clipmap's snaps: at several of them.
    let control = fly(&gpu, &renderer, &spec, frames, builds, false);
    println!("without: {control:?}");
    assert!(control.frames_with >= 3, "{control:?}");
}
