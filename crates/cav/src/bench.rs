//! `cav bench flythrough`: a scripted camera over `stress-1e10` (REBUILD §9.1 `flythrough`), or
//! over another scene with a world (`--scene town`, the loop scaled to its world),
//! drawn offscreen frame by frame as a window draws it (the world streamed a step a frame, its
//! meshes made a few a frame), with a frame-time trace. What this card holds is measured here:
//! the owner's ruling makes 60 fps and 10¹⁰ cells targets for a faster GPU, recorded, not gated.
//!
//! Each frame: the camera moves (three view commands), the engine polls (which streams), the
//! renderer draws one pane and the GPU is waited for, so a frame's time is CPU and GPU together.
//! The trace's columns are written in its header.

use std::fmt::Write as _;
use std::time::Instant;

use cav_engine::{Command, Engine, ViewCommand};
use cav_gpu::{Gpu, wgpu};
use cav_render::{PaneTarget, Renderer};

/// Where the camera is at `s` from 0 to 1 along the flythrough: its target (on the ground) and
/// its yaw, pitch (degrees) and distance (metres). A loop out from the array over the town and
/// back, low over the ground, looking along the way it goes; `scale` shrinks the loop for a smaller
/// world (1 for `stress-1e10`'s kilometre).
#[must_use]
pub fn path(s: f64, scale: f64) -> ([f64; 3], f64, f64, f64) {
    let angle = std::f64::consts::TAU * s;
    // An ellipse 480 m by 800 m through the array's neighbourhood at s = 0 (scaled).
    let (a, b) = (240.0 * scale, 400.0 * scale);
    let x = a * (angle.cos() - 1.0) + 20.0 * scale;
    let y = b * angle.sin();
    // Facing along the path: the camera behind the target, so its yaw points back along it.
    let (dx, dy) = (-a * angle.sin(), b * angle.cos());
    let heading = dy.atan2(dx).to_degrees();
    ([x, y, 0.0], heading + 180.0, 14.0, 24.0)
}

/// The loop's length, metres, at `scale`: Ramanujan's perimeter of the path's ellipse.
#[must_use]
pub fn loop_length(scale: f64) -> f64 {
    let (a, b) = (240.0 * scale, 400.0 * scale);
    std::f64::consts::PI * (3.0 * (a + b) - ((3.0 * a + b) * (a + 3.0 * b)).sqrt())
}

/// The trace's summary: the mean, the median, the 99th percentile and the worst frame, ms.
fn summary(times: &[f64]) -> [f64; 4] {
    let mut sorted = times.to_vec();
    sorted.sort_by(f64::total_cmp);
    let at = |q: f64| sorted[((sorted.len() - 1) as f64 * q).round() as usize];
    [times.iter().sum::<f64>() / times.len() as f64, at(0.5), at(0.99), sorted[sorted.len() - 1]]
}

/// Flies `frames` frames (stopping at the first that takes over `max_frame_ms`) at `size` pixels and `speed` metres a second (taking a frame as a
/// sixtieth of a second), the world cached under `cache` when given, and returns the trace as
/// CSV with the summary in its last lines.
///
/// # Errors
/// When there is no GPU or the scene cannot be loaded.
pub fn flythrough(
    scene: &str,
    frames: usize,
    size: [u32; 2],
    speed: f64,
    cache: Option<std::path::PathBuf>,
    max_frame_ms: f64,
) -> Result<String, String> {
    let mut engine = Engine::new();
    engine.set_world_cache(cache);
    let spec = cav_sim::load(scene).map_err(|e| e.0)?;
    let world = spec.world.as_ref().ok_or_else(|| format!("{scene} has no world to fly over"))?;
    let built = cav_sim::world::World::new(world);
    let cells = built.cells(world);
    // The loop fits a kilometre; a smaller world's is scaled to its own half-width.
    let scale = built.extent().map_or(1.0, |e| {
        let half = 0.5 * (e.hi.x - e.lo.x).min(e.hi.y - e.lo.y);
        (half / 500.0).clamp(0.01, 1.0)
    });
    engine.apply(Command::load(spec)).map_err(|e| e.to_string())?;
    let gpu = Gpu::new()?;
    let renderer = Renderer::new(&gpu);
    let target = PaneTarget::new(&gpu, size);
    let wait = |gpu: &Gpu| {
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).map(|_| ()).map_err(|e| e.to_string())
    };
    // The start: everything the first camera and the towers want, made at once (a load).
    let place = |engine: &mut Engine, s: f64| -> Result<(), String> {
        let (target, yaw, pitch, distance) = path(s, scale);
        for command in [
            ViewCommand::Pan { pane: 0, target },
            ViewCommand::Orbit { pane: 0, yaw, pitch },
            ViewCommand::Zoom { pane: 0, distance },
        ] {
            engine.apply(Command::View(command)).map_err(|e| e.to_string())?;
        }
        Ok(())
    };
    place(&mut engine, 0.0)?;
    let started = Instant::now();
    let whole = engine.settle_world(std::time::Duration::from_secs(600));
    let load = started.elapsed().as_secs_f64();
    let made = (engine.streaming().made, engine.streaming().cached);
    let mut out = String::from(
        "frame,ms,poll_ms,draw_ms,mesh_ms,meshed,made,chunks,bricks,pending,meshes,bricks_on_gpu,vertices,resident_mb,marched\n",
    );
    let mut times = Vec::with_capacity(frames);
    let (mut meshed_before, mut made_before) = (0u64, made.0 + made.1);
    // What the renderer left out, by kind (the words before the colon), and in how many frames.
    let mut cuts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for frame in 0..frames {
        let s = frame as f64 * speed / 60.0 / loop_length(scale);
        let began = Instant::now();
        place(&mut engine, s)?;
        engine.poll();
        let polled = began.elapsed().as_secs_f64() * 1e3;
        let view = engine.frame(0)?;
        renderer.draw(&gpu, &target, view)?;
        wait(&gpu)?;
        let ms = began.elapsed().as_secs_f64() * 1e3;
        let world = engine.frame(0)?.world.cloned().unwrap_or_default();
        let ([meshes, bricks_on_gpu, vertices], notes) = renderer.world_counts();
        let (meshed, mesh_ms) = renderer.world_meshing();
        let new_meshes = meshed - meshed_before;
        meshed_before = meshed;
        let streamed = engine.streaming().made + engine.streaming().cached;
        let new_made = streamed - made_before;
        made_before = streamed;
        for note in &notes {
            *cuts.entry(note.split(':').next().unwrap_or_default().to_owned()).or_default() += 1;
        }
        let _ = writeln!(
            out,
            "{frame},{ms:.3},{polled:.3},{:.3},{mesh_ms:.3},{new_meshes},{new_made},{},{},{},{meshes},{bricks_on_gpu},{vertices},{:.1},{:.2}",
            ms - polled,
            world.chunks.len(),
            world.bricks.len(),
            world.pending,
            engine.streaming().bytes() as f64 / 1048576.0,
            renderer.world_marched()
        );
        times.push(ms);
        // A frame this long would hold the owner's desktop (Windows stops answering a busy GPU
        // long before its 2 s reset): stop here, cleanly, and say so.
        if ms > max_frame_ms {
            let _ = writeln!(
                out,
                "# stopped at frame {frame}: {ms:.0} ms is over the {max_frame_ms} ms a frame may take"
            );
            break;
        }
    }
    let [mean, median, p99, worst] = summary(&times);
    let _ = writeln!(
        out,
        "# {scene}: about {cells:.2e} finest cells; the loop {:.0} m; {}x{} px at {speed} m/s; first view resident in {load:.2} s ({} made, {} from the cache{}); \
         {} frames of the {frames} asked: mean {mean:.1} ms ({:.0} fps), median {median:.1} ms, 99th percentile {p99:.1} ms, worst {worst:.1} ms",
        loop_length(scale),
        size[0],
        size[1],
        made.0,
        made.1,
        if whole { "" } else { ", NOT WHOLE in 600 s" },
        times.len(),
        1000.0 / mean
    );
    for (kind, count) in &cuts {
        let _ = writeln!(out, "# not drawn in {count} of {} frames: {kind}", times.len());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flythrough_path_loops_and_looks_ahead() {
        let (start, ..) = path(0.0, 1.0);
        let (end, ..) = path(1.0, 1.0);
        assert!((start[0] - end[0]).abs() < 1e-6 && (start[1] - end[1]).abs() < 1e-6);
        // It starts by the array and goes out over the town, within the kilometre.
        assert!(start[0].abs() < 30.0 && start[1].abs() < 30.0);
        let (far, ..) = path(0.5, 1.0);
        assert!(far[0] < -400.0 && far[0] > -500.0);
        for i in 0..100 {
            let (t, yaw, pitch, distance) = path(f64::from(i) / 100.0, 1.0);
            assert!(t[0].abs() <= 500.0 && t[1].abs() <= 500.0, "{t:?}");
            assert!(pitch > 0.0 && distance > 0.0 && yaw.is_finite());
            // The camera is behind the target: the next target is ahead, away from the eye.
            let (next, ..) = path(f64::from(i) / 100.0 + 0.001, 1.0);
            let eye = [t[0] + yaw.to_radians().cos(), t[1] + yaw.to_radians().sin()];
            let ahead = (next[0] - t[0]) * (t[0] - eye[0]) + (next[1] - t[1]) * (t[1] - eye[1]);
            assert!(ahead > 0.0, "at {i} the camera looks back");
        }
        // Scaled for town's 200 m: within ±100 m, and its length scales with it.
        for i in 0..100 {
            let (t, ..) = path(f64::from(i) / 100.0, 0.2);
            assert!(t[0].abs() <= 100.0 && t[1].abs() <= 100.0, "{t:?}");
        }
        assert!((loop_length(0.2) - 0.2 * loop_length(1.0)).abs() < 1e-9);
        assert_eq!(summary(&[1.0, 2.0, 3.0, 10.0]), [4.0, 3.0, 10.0, 10.0]);
    }
}
