//! `cav doctor`: what the machine has, whether a device can be made, and whether it draws correctly.

use std::time::Instant;

use cav_core::Vec3;
use cav_gpu::Gpu;
use cav_render::{BACKGROUND_SRGB, Camera, Frame, Renderer, tower_srgb};
use cav_sim::{DrawStyle, NamedView, Projection, SceneSpec, ViewSpec};

/// The size of the self-test picture, pixels.
const SIZE: [u32; 2] = [320, 240];

pub fn run() -> Result<(), String> {
    println!("cavendish {} · {} {}", cav_engine::VERSION, std::env::consts::OS, std::env::consts::ARCH);
    let started = Instant::now();
    let gpu = Gpu::new()?;
    let device_time = started.elapsed();
    let chosen = gpu.adapter_report();
    println!("adapters:");
    for adapter in gpu.all_adapters() {
        println!("  {} {adapter}", if adapter == chosen { "*" } else { " " });
    }
    println!(
        "device: `{}` on {}, created in {:.2} s",
        chosen.name,
        chosen.backend,
        device_time.as_secs_f64()
    );

    let started = Instant::now();
    let renderer = Renderer::new(&gpu);
    let (scene, view) = self_test_frame();
    let frame = Frame {
        scene: &scene,
        view: &view,
        highlight: None,
        time: 0.0,
        content: cav_sim::PaneContent::Scene,
        span: None,
        voxels: None,
    };
    let rgba = renderer.render_rgba(&gpu, frame, SIZE)?;
    match check_picture(&rgba, &scene, &view) {
        Ok(()) => println!(
            "render self-test: ok · {} × {} px, 5 towers found where the camera projects them, background exact, {:.2} s",
            SIZE[0],
            SIZE[1],
            started.elapsed().as_secs_f64()
        ),
        Err(why) => return Err(format!("render self-test FAILED: {why}")),
    }
    println!(
        "precision self-test: none yet. S0 runs no gravity kernel; the GPU kernels and their f64 twins arrive with S5."
    );
    let names: Vec<&str> = cav_sim::LIBRARY.iter().map(|s| s.name).collect();
    println!("scenes: {}", names.join(", "));
    Ok(())
}

/// What the self-test draws: the default scene, isometric, flat colours.
fn self_test_frame() -> (SceneSpec, ViewSpec) {
    let scene = SceneSpec::room();
    let view = ViewSpec::framing(&scene, NamedView::Iso, Projection::Perspective, DrawStyle::Flat);
    (scene, view)
}

/// Checks a self-test picture: each tower's colour where the CPU camera projects a point on it,
/// and the exact background colour in the corners.
fn check_picture(rgba: &[u8], scene: &SceneSpec, view: &ViewSpec) -> Result<(), String> {
    let at = |x: u32, y: u32| {
        let i = ((y * SIZE[0] + x) * 4) as usize;
        [rgba[i], rgba[i + 1], rgba[i + 2]]
    };
    let close = |a: [u8; 3], b: [u8; 3], tolerance: u8| (0..3).all(|i| a[i].abs_diff(b[i]) <= tolerance);
    if rgba.len() != (SIZE[0] * SIZE[1] * 4) as usize {
        return Err(format!("the picture is {} bytes, not {} × {} × 4", rgba.len(), SIZE[0], SIZE[1]));
    }
    for (x, y) in [(1, 1), (SIZE[0] - 2, 1), (1, SIZE[1] - 2), (SIZE[0] - 2, SIZE[1] - 2)] {
        if !close(at(x, y), BACKGROUND_SRGB, 1) {
            return Err(format!(
                "the corner ({x}, {y}) is {:?}, not the background {BACKGROUND_SRGB:?}",
                at(x, y)
            ));
        }
    }
    let camera = Camera::new(view, f64::from(SIZE[0]) / f64::from(SIZE[1]));
    for (n, tower) in scene.array.towers.iter().enumerate() {
        // 7.3 m up: above the room and between the metre ticks.
        let point = Vec3::from_array(tower.position) + Vec3::Z * 7.3;
        let [px, py] = camera.pixel(point, SIZE).ok_or("a tower is behind the camera")?;
        let (cx, cy) = (px.round() as i64, py.round() as i64);
        let found = (-2..=2i64).any(|dy| {
            (-2..=2i64).any(|dx| {
                let (x, y) = (cx + dx, cy + dy);
                x >= 0
                    && y >= 0
                    && x < i64::from(SIZE[0])
                    && y < i64::from(SIZE[1])
                    && close(at(x as u32, y as u32), tower_srgb(n), 40)
            })
        });
        if !found {
            return Err(format!("tower {} is not drawn at ({px:.1}, {py:.1})", n + 1));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture of the background with a 3 × 3 patch of each tower's colour at `shift` pixels to
    /// the right of where the camera projects it.
    fn synthetic(shift: f64) -> Vec<u8> {
        let (scene, view) = self_test_frame();
        let camera = Camera::new(&view, f64::from(SIZE[0]) / f64::from(SIZE[1]));
        let mut rgba: Vec<u8> = (0..SIZE[0] * SIZE[1])
            .flat_map(|_| [BACKGROUND_SRGB[0], BACKGROUND_SRGB[1], BACKGROUND_SRGB[2], 255])
            .collect();
        for (n, tower) in scene.array.towers.iter().enumerate() {
            let [px, py] = camera.pixel(Vec3::from_array(tower.position) + Vec3::Z * 7.3, SIZE).unwrap();
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (x, y) = ((px + shift).round() as i64 + dx, py.round() as i64 + dy);
                    let i = ((y * i64::from(SIZE[0]) + x) * 4) as usize;
                    rgba[i..i + 3].copy_from_slice(&tower_srgb(n));
                }
            }
        }
        rgba
    }

    #[test]
    fn the_self_test_accepts_a_right_picture_and_refuses_wrong_ones() {
        let (scene, view) = self_test_frame();
        assert_eq!(check_picture(&synthetic(0.0), &scene, &view), Ok(()));
        // Towers drawn ten pixels off: refused.
        let why = check_picture(&synthetic(10.0), &scene, &view).unwrap_err();
        assert!(why.contains("tower 1 is not drawn"), "{why}");
        // Nothing drawn: refused.
        let blank: Vec<u8> = (0..SIZE[0] * SIZE[1]).flat_map(|_| [10, 10, 10, 255]).collect();
        assert!(check_picture(&blank, &scene, &view).unwrap_err().contains("tower 1"));
        // The wrong background: refused.
        let mut grey = synthetic(0.0);
        for pixel in grey.as_chunks_mut::<4>().0 {
            if pixel[..3] == BACKGROUND_SRGB {
                pixel[..3].copy_from_slice(&[40, 40, 40]);
            }
        }
        assert!(check_picture(&grey, &scene, &view).unwrap_err().contains("not the background"));
        // A truncated picture: refused.
        assert!(check_picture(&blank[..100], &scene, &view).is_err());
    }
}
