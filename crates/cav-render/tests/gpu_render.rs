//! GPU tier: the renderer draws what the camera maths says, on the local adapter.
//! Every test here has `gpu_` in its name, which is how `cargo xtask gpu` finds it.

use cav_core::Vec3;
use cav_gpu::Gpu;
use cav_render::{BACKGROUND_SRGB, Camera, Frame, PAPER_SRGB, Renderer, tower_srgb};
use cav_sim::{DrawStyle, NamedView, Projection, SceneSpec, ViewSpec, dev};

const SIZE: [u32; 2] = [800, 600];

struct Picture {
    rgba: Vec<u8>,
}

impl Picture {
    fn at(&self, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * SIZE[0] + x) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]]
    }

    /// Whether any pixel within `reach` of `(x, y)` is within `tolerance` of `colour` per channel.
    fn has_near(&self, at: [f64; 2], reach: i64, colour: [u8; 3], tolerance: u8) -> bool {
        let (cx, cy) = (at[0].round() as i64, at[1].round() as i64);
        (-reach..=reach).any(|dy| {
            (-reach..=reach).any(|dx| {
                let (x, y) = (cx + dx, cy + dy);
                x >= 0
                    && y >= 0
                    && x < i64::from(SIZE[0])
                    && y < i64::from(SIZE[1])
                    && close(self.at(x as u32, y as u32), colour, tolerance)
            })
        })
    }

    /// The first and last rows that hold a pixel of `colour`.
    fn rows_of(&self, colour: [u8; 3], tolerance: u8) -> Option<(u32, u32)> {
        let rows: Vec<u32> =
            (0..SIZE[1]).filter(|&y| (0..SIZE[0]).any(|x| close(self.at(x, y), colour, tolerance))).collect();
        Some((*rows.first()?, *rows.last()?))
    }

    /// The fraction of pixels in rows `from..to` that are not the background.
    fn drawn_fraction(&self, rows: std::ops::Range<u32>, background: [u8; 3]) -> f64 {
        let total = f64::from(rows.len() as u32 * SIZE[0]);
        let drawn = rows
            .flat_map(|y| (0..SIZE[0]).map(move |x| (x, y)))
            .filter(|&(x, y)| !close(self.at(x, y), background, 1))
            .count();
        drawn as f64 / total
    }
}

fn close(a: [u8; 3], b: [u8; 3], tolerance: u8) -> bool {
    (0..3).all(|i| a[i].abs_diff(b[i]) <= tolerance)
}

fn setup() -> (Gpu, Renderer) {
    let gpu = Gpu::new().expect("the GPU tier needs an adapter");
    let renderer = Renderer::new(&gpu);
    (gpu, renderer)
}

fn render(gpu: &Gpu, renderer: &Renderer, scene: &SceneSpec, view: &ViewSpec) -> Picture {
    let frame = Frame { scene, view, highlight: None, time: 0.0 };
    Picture { rgba: renderer.render_rgba(gpu, frame, SIZE).expect("render") }
}

fn camera(view: &ViewSpec) -> Camera {
    Camera::new(view, f64::from(SIZE[0]) / f64::from(SIZE[1]))
}

/// A point on tower `n`'s baseline, `height` metres up.
fn on_tower(scene: &SceneSpec, n: usize, height: f64) -> Vec3 {
    Vec3::from_array(scene.array.towers[n].position) + Vec3::Z * height
}

#[test]
fn gpu_towers_appear_where_the_camera_projects_them() {
    let (gpu, renderer) = setup();
    let scene = SceneSpec::room();
    let framed = |named, projection| ViewSpec::framing(&scene, named, projection, DrawStyle::Flat);
    let views = [
        ("iso", framed(NamedView::Iso, Projection::Perspective)),
        ("iso, orthographic", framed(NamedView::Iso, Projection::Orthographic)),
        ("top", framed(NamedView::Top, Projection::Orthographic)),
        (
            "low from the south-east",
            ViewSpec { yaw: -60.0, pitch: 10.0, ..framed(NamedView::Iso, Projection::Perspective) },
        ),
    ];
    for (name, view) in views {
        let picture = render(&gpu, &renderer, &scene, &view);
        let camera = camera(&view);
        for n in 0..5 {
            // 7.3 m up: above the room, between ticks, so only the tower's own colour is there.
            let pixel = camera.pixel(on_tower(&scene, n, 7.3), SIZE).expect("in front of the camera");
            // Within 2 px, as a distance: not a 5 × 5 box, whose corners are 2.8 px away.
            let off = picture.distance_to(pixel, 8, tower_srgb(n), 8);
            assert!(off <= 2.0, "{name}: tower {} is {off:.2} px from {pixel:?}", n + 1);
            // Control arms: another tower's colour is not there, and 25 px to the side is empty,
            // so a renderer that put the towers elsewhere (or mirrored them) fails.
            assert!(
                !picture.has_near(pixel, 2, tower_srgb((n + 1) % 5), 8),
                "{name}: wrong tower at {pixel:?}"
            );
            let beside = [pixel[0] + 25.0, pixel[1] + 12.0];
            assert!(!picture.has_near(beside, 2, tower_srgb(n), 8), "{name}: tower {} smeared", n + 1);
        }
    }
}

#[test]
fn gpu_void_has_towers_and_no_room() {
    let (gpu, renderer) = setup();
    let room = SceneSpec::room();
    let void = dev::void();
    // The same camera for both, so the pictures differ only by the room.
    let view = ViewSpec::framing(&room, NamedView::Iso, Projection::Perspective, DrawStyle::Flat);
    let with_room = render(&gpu, &renderer, &room, &view);
    let without = render(&gpu, &renderer, &void, &view);
    let camera = camera(&view);
    // The middle of a ceiling edge of the room: an edge line with nothing else near it.
    let edge = camera.pixel(Vec3::new(5.0, 2.5, 3.0), SIZE).unwrap();
    let edge_colour = [0x5a, 0x5a, 0x5a];
    assert!(with_room.has_near(edge, 2, edge_colour, 10), "the room's edge is drawn at {edge:?}");
    assert!(!without.has_near(edge, 4, edge_colour, 60), "the void has no room edge");
    assert!(without.has_near(edge, 0, BACKGROUND_SRGB, 1), "the void is background there");
    // The towers are in both.
    for n in 0..5 {
        let pixel = camera.pixel(on_tower(&room, n, 7.3), SIZE).unwrap();
        assert!(
            with_room.has_near(pixel, 2, tower_srgb(n), 8) && without.has_near(pixel, 2, tower_srgb(n), 8)
        );
    }
    // The void's renderer copes with an empty world: nothing but towers and background above them.
    assert_eq!(without.drawn_fraction(0..40, BACKGROUND_SRGB), 0.0);
}

#[test]
fn gpu_render_is_deterministic() {
    let (gpu, renderer) = setup();
    let scene = dev::infinite_floor();
    let view = ViewSpec::home(&scene);
    let first = render(&gpu, &renderer, &scene, &view);
    let second = render(&gpu, &renderer, &scene, &view);
    assert!(first.rgba == second.rgba, "two renders of the same frame differ");
    // A second renderer on the same device gives the same picture: nothing depends on its history.
    let again = render(&gpu, &Renderer::new(&gpu), &scene, &view);
    assert!(first.rgba == again.rgba);
    // Control: the comparison sees a camera moved by a hundredth of a degree.
    let nudged = ViewSpec { yaw: view.yaw + 0.01, ..view };
    assert!(first.rgba != render(&gpu, &renderer, &scene, &nudged).rgba);
}

#[test]
fn gpu_orthographic_and_perspective_differ_as_expected() {
    let (gpu, renderer) = setup();
    let scene = dev::void();
    // From east-south-east, level with the middle of the towers: no tower hides another, and tower 2
    // (at x = +4) is nearer than tower 3 (at x = −4).
    let mut base = ViewSpec::framing(&scene, NamedView::South, Projection::Orthographic, DrawStyle::Flat);
    (base.yaw, base.pitch) = (-30.0, 0.0);
    base.frame(&scene);
    let height_of = |picture: &Picture, n: usize| {
        let (top, bottom) =
            picture.rows_of(tower_srgb(n), 8).unwrap_or_else(|| panic!("tower {} is not drawn", n + 1));
        f64::from(bottom - top)
    };
    // Orthographic: every tower is the same height on screen, whatever its distance.
    let ortho = render(&gpu, &renderer, &scene, &base);
    let heights: Vec<f64> = (0..5).map(|n| height_of(&ortho, n)).collect();
    for h in &heights {
        assert!((h - heights[0]).abs() <= 2.0, "orthographic heights differ: {heights:?}");
    }
    // Perspective: the nearer tower is taller by the ratio of the distances, as the CPU camera predicts.
    let mut view = ViewSpec { projection: Projection::Perspective, ..base };
    view.frame(&scene);
    let persp = render(&gpu, &renderer, &scene, &view);
    let camera = camera(&view);
    let predicted = |n: usize| {
        let (foot, head) = (
            camera.pixel(on_tower(&scene, n, 0.0), SIZE).unwrap(),
            camera.pixel(on_tower(&scene, n, 10.0), SIZE).unwrap(),
        );
        foot[1] - head[1]
    };
    let (near, far) = (1, 2); // tower 2 at x = +4, tower 3 at x = −4
    let measured_ratio = height_of(&persp, near) / height_of(&persp, far);
    let predicted_ratio = predicted(near) / predicted(far);
    assert!(predicted_ratio > 1.15, "the test's geometry separates the towers: {predicted_ratio}");
    assert!(
        (measured_ratio / predicted_ratio - 1.0).abs() < 0.03,
        "perspective ratio {measured_ratio:.4}, predicted {predicted_ratio:.4}"
    );
    // Control: the orthographic picture does not show that ratio.
    assert!((height_of(&ortho, near) / height_of(&ortho, far) - 1.0).abs() < 0.02);
}

#[test]
fn gpu_every_style_is_distinct_and_paper_is_cream() {
    let (gpu, renderer) = setup();
    let scene = SceneSpec::room();
    let pictures: Vec<(DrawStyle, Picture)> = DrawStyle::ALL
        .into_iter()
        .map(|style| {
            let view = ViewSpec::framing(&scene, NamedView::Iso, Projection::Perspective, style);
            (style, render(&gpu, &renderer, &scene, &view))
        })
        .collect();
    for (i, (a, picture_a)) in pictures.iter().enumerate() {
        for (b, picture_b) in &pictures[i + 1..] {
            assert!(picture_a.rgba != picture_b.rgba, "{a:?} and {b:?} draw the same picture");
        }
    }
    let of = |style: DrawStyle| &pictures.iter().find(|(s, _)| *s == style).unwrap().1;
    // The corner of the picture is background: cream paper, near-black otherwise.
    assert!(close(of(DrawStyle::Paper).at(2, 2), PAPER_SRGB, 1), "{:?}", of(DrawStyle::Paper).at(2, 2));
    for style in [DrawStyle::Shaded, DrawStyle::Flat, DrawStyle::XRay, DrawStyle::Wireframe] {
        assert!(close(of(style).at(2, 2), BACKGROUND_SRGB, 1), "{style:?}: {:?}", of(style).at(2, 2));
    }
    // Wireframe has no filled surfaces, so it draws fewer pixels than flat.
    let all = 0..SIZE[1];
    let (wire, flat) = (
        of(DrawStyle::Wireframe).drawn_fraction(all.clone(), BACKGROUND_SRGB),
        of(DrawStyle::Flat).drawn_fraction(all.clone(), BACKGROUND_SRGB),
    );
    assert!(wire > 0.005 && wire < 0.6 * flat, "wireframe {wire:.4}, flat {flat:.4}");
    // Paper is ink on cream: nothing on it is lighter than the paper, and something is dark.
    let paper = of(DrawStyle::Paper);
    let mut darkest = 255u8;
    for y in 0..SIZE[1] {
        for x in 0..SIZE[0] {
            let [r, g, b] = paper.at(x, y);
            assert!(r <= PAPER_SRGB[0] + 1 && g <= PAPER_SRGB[1] + 1 && b <= PAPER_SRGB[2] + 1);
            darkest = darkest.min(g);
        }
    }
    assert!(darkest < 0x60, "paper has ink: darkest green channel {darkest}");
    // Shaded differs from flat here; what shading must do is `gpu_shading_follows_the_light`.
    assert!(of(DrawStyle::Shaded).rgba != of(DrawStyle::Flat).rgba);
}

#[test]
fn gpu_infinite_floor_reaches_the_horizon() {
    let (gpu, renderer) = setup();
    let floor = dev::infinite_floor();
    let void = dev::void();
    // Low over the ground, looking north-west: the horizon crosses the picture.
    let view = ViewSpec {
        pitch: 8.0,
        ..ViewSpec::framing(&floor, NamedView::Iso, Projection::Perspective, DrawStyle::Flat)
    };
    let camera = camera(&view);
    // The horizon is where a ground point very far ahead projects to.
    let ahead = Vec3::new(camera.forward.x, camera.forward.y, 0.0).normalised();
    let far_point = camera.origin + ahead * 1.0e6 - Vec3::Z * camera.origin.z;
    let horizon = camera.pixel(far_point, SIZE).unwrap()[1].round() as u32;
    assert!(horizon > 60 && horizon < SIZE[1] - 200, "the horizon is in the picture: row {horizon}");

    let with_floor = render(&gpu, &renderer, &floor, &view);
    let without = render(&gpu, &renderer, &void, &view);
    // Below the horizon the floor fills the picture from edge to edge...
    let low = (horizon + 80)..SIZE[1];
    assert!(with_floor.drawn_fraction(low.clone(), BACKGROUND_SRGB) > 0.98);
    // ...including the picture's bottom corners, which a finite quad around the array would miss.
    for x in [0, SIZE[0] - 1] {
        assert!(
            !close(with_floor.at(x, SIZE[1] - 1), BACKGROUND_SRGB, 1),
            "the floor does not reach x = {x}"
        );
    }
    // Above the horizon there is no floor: the sky is background apart from the towers.
    let sky = 0..horizon.saturating_sub(40);
    assert!(with_floor.drawn_fraction(sky.clone(), BACKGROUND_SRGB) < 0.02);
    // Control: the void, from the same camera, has nothing below the horizon but the towers.
    assert!(without.drawn_fraction(low, BACKGROUND_SRGB) < 0.02);
    // The grid has lines: the floor is not one flat colour.
    let row = SIZE[1] - 30;
    let distinct: std::collections::HashSet<[u8; 3]> = (0..SIZE[0]).map(|x| with_floor.at(x, row)).collect();
    assert!(distinct.len() > 3, "the floor shows a grid: {} colours on a row", distinct.len());
}

impl Picture {
    /// The distance in pixels from `at` to the nearest pixel within `tolerance` of `colour`,
    /// looking no further than `reach` pixels; infinite if there is none.
    fn distance_to(&self, at: [f64; 2], reach: i64, colour: [u8; 3], tolerance: u8) -> f64 {
        let (cx, cy) = (at[0].round() as i64, at[1].round() as i64);
        let mut nearest = f64::INFINITY;
        for y in (cy - reach).max(0)..=(cy + reach).min(i64::from(SIZE[1]) - 1) {
            for x in (cx - reach).max(0)..=(cx + reach).min(i64::from(SIZE[0]) - 1) {
                if close(self.at(x as u32, y as u32), colour, tolerance) {
                    // Pixel centres are at half-integers.
                    let (dx, dy) = (x as f64 + 0.5 - at[0], y as f64 + 0.5 - at[1]);
                    nearest = nearest.min(dx.hypot(dy));
                }
            }
        }
        nearest
    }
}

cav_validate::case! {
    test: validation_gpu_towers_at_projected_pixels,
    name: "render.gpu_projection",
    subsystem: "render",
    slice: "S0",
    reference: ClosedForm,
    against: "the f64 camera's projection of a point on each tower; distance to the centre of the nearest pixel of that tower's colour, worst over five towers and four views. The measure resolves no finer than half a pixel's diagonal, 0.71 px",
    unit: Pixels,
    tolerance: 2.0,
    measure: {
        let (gpu, renderer) = setup();
        let scene = SceneSpec::room();
        let framed = |named, projection| ViewSpec::framing(&scene, named, projection, DrawStyle::Flat);
        let views = [
            framed(NamedView::Iso, Projection::Perspective),
            framed(NamedView::Iso, Projection::Orthographic),
            framed(NamedView::Top, Projection::Orthographic),
            ViewSpec { yaw: -60.0, pitch: 10.0, ..framed(NamedView::Iso, Projection::Perspective) },
        ];
        let mut worst = 0f64;
        for view in &views {
            let picture = render(&gpu, &renderer, &scene, view);
            let camera = camera(view);
            for n in 0..5 {
                let pixel = camera.pixel(on_tower(&scene, n, 7.3), SIZE).expect("in front of the camera");
                worst = worst.max(picture.distance_to(pixel, 12, tower_srgb(n), 8));
            }
        }
        cav_validate::Measured::Error(worst)
    },
}

cav_validate::case! {
    test: validation_gpu_render_is_deterministic,
    name: "render.gpu_determinism",
    subsystem: "render",
    slice: "S0",
    reference: Exact,
    against: "the same frame drawn twice, and by a second renderer on the same device; bytes that differ",
    unit: Count,
    tolerance: 0.0,
    measure: {
        let (gpu, renderer) = setup();
        let scene = dev::infinite_floor();
        let view = ViewSpec::home(&scene);
        let first = render(&gpu, &renderer, &scene, &view);
        let second = render(&gpu, &renderer, &scene, &view);
        let third = render(&gpu, &Renderer::new(&gpu), &scene, &view);
        let differing = |other: &Picture| first.rgba.iter().zip(&other.rgba).filter(|(a, b)| a != b).count();
        cav_validate::Measured::Error((differing(&second) + differing(&third)) as f64)
    },
}

cav_validate::case! {
    test: validation_gpu_perspective_ratio,
    name: "render.gpu_perspective_ratio",
    subsystem: "render",
    slice: "S0",
    reference: ClosedForm,
    against: "the ratio of the on-screen heights of a near and a far tower, as the f64 camera predicts it",
    unit: Relative,
    tolerance: 0.03,
    measure: {
        let (gpu, renderer) = setup();
        let scene = dev::void();
        let mut view = ViewSpec::framing(&scene, NamedView::South, Projection::Perspective, DrawStyle::Flat);
        (view.yaw, view.pitch) = (-30.0, 0.0);
        view.frame(&scene);
        let picture = render(&gpu, &renderer, &scene, &view);
        let camera = camera(&view);
        let measured = |n: usize| {
            let (top, bottom) = picture.rows_of(tower_srgb(n), 8).expect("the tower is drawn");
            f64::from(bottom - top)
        };
        let predicted = |n: usize| {
            let foot = camera.pixel(on_tower(&scene, n, 0.0), SIZE).unwrap();
            let head = camera.pixel(on_tower(&scene, n, 10.0), SIZE).unwrap();
            foot[1] - head[1]
        };
        let (near, far) = (1, 2);
        cav_validate::Measured::Error(((measured(near) / measured(far)) / (predicted(near) / predicted(far)) - 1.0).abs())
    },
}

/// A scene with every range of the spec at its limit: a thousand towers a kilometre tall in a
/// ten-kilometre room, a thousand kilometres from the origin.
fn largest_scene() -> SceneSpec {
    use cav_sim::spec::limits;
    let mut scene = SceneSpec::room();
    scene.array.instrument.tower_height = limits::TOWER_HEIGHT;
    scene.array.instrument.ifo_separation = 500.0;
    let corner = limits::POSITION - limits::ROOM_SIZE;
    scene.array.towers = (0..limits::TOWERS)
        .map(|n| cav_sim::TowerSpec {
            label: format!("t{n}"),
            position: [corner + 5.0 * n as f64, corner + 100.0, corner],
        })
        .collect();
    let room = scene.room.as_mut().unwrap();
    (room.origin, room.size) = ([corner; 3], [limits::ROOM_SIZE; 3]);
    scene.floor =
        Some(cav_sim::FloorSpec { height: corner, kind: cav_sim::FloorKind::HalfSpace, density: 2400.0 });
    scene.validate().expect("the largest scene is a valid one");
    scene
}

#[test]
fn gpu_the_largest_valid_scene_renders() {
    let (gpu, renderer) = setup();
    let scene = largest_scene();
    let started = std::time::Instant::now();
    for (named, projection) in
        [(NamedView::Iso, Projection::Perspective), (NamedView::Top, Projection::Orthographic)]
    {
        let view = ViewSpec::framing(&scene, named, projection, DrawStyle::Shaded);
        let frame =
            Frame { scene: &scene, view: &view, highlight: Some(cav_sim::Target::Tower(500)), time: 0.0 };
        let rgba = renderer.render_rgba(&gpu, frame, SIZE).expect("the largest valid scene must draw");
        let picture = Picture { rgba };
        // Something was drawn, and it is a picture rather than one flat colour. (The scene has an
        // infinite floor, so from above there may be no background left at all.)
        assert!(picture.drawn_fraction(0..SIZE[1], BACKGROUND_SRGB) > 0.01, "{named:?}: nothing is drawn");
        let colours: std::collections::HashSet<[u8; 3]> = (0..SIZE[1])
            .flat_map(|y| (0..SIZE[0]).map(move |x| (x, y)))
            .map(|(x, y)| picture.at(x, y))
            .collect();
        assert!(colours.len() > 8, "{named:?}: only {} colours", colours.len());
    }
    // Bounded work: seconds, not minutes. (nextest's watchdog kills a test that hangs.)
    assert!(started.elapsed().as_secs() < 20, "it took {:?}", started.elapsed());
    // Control: the same code path refuses a scene outside the ranges before anything is built.
    let mut beyond = scene;
    beyond.array.instrument.tower_height = 3.0e6;
    assert!(beyond.validate().is_err());
}

/// An sRGB byte as linear light, 0 to 1.
fn linear(byte: u8) -> f64 {
    let c = f64::from(byte) / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// The void with only the towers at the given ground positions, in that order.
fn towers_at(positions: &[[f64; 2]]) -> SceneSpec {
    let mut scene = dev::void();
    scene.array.towers = positions
        .iter()
        .enumerate()
        .map(|(n, p)| cav_sim::TowerSpec { label: format!("tower {}", n + 1), position: [p[0], p[1], 0.0] })
        .collect();
    scene
}

#[test]
fn gpu_xray_adds_what_is_behind_and_flat_hides_it() {
    let (gpu, renderer) = setup();
    // Seen from the south, one tower stands straight behind the other.
    let (near, far) = ([0.0, -4.0], [0.0, 4.0]);
    let both = towers_at(&[near, far]);
    let pixel_in = |scene: &SceneSpec, style: DrawStyle| {
        // The same camera for every scene: framed on the pair.
        let view = ViewSpec::framing(&both, NamedView::South, Projection::Orthographic, style);
        let at = camera(&view).pixel(on_tower(&both, 0, 7.3), SIZE).unwrap();
        render(&gpu, &renderer, scene, &view).at(at[0].round() as u32, at[1].round() as u32)
    };
    // Each order of drawing: the near tower first, and the far one first. With a depth test,
    // which of the two is hidden depends on the order; without one, neither is.
    for (first, second) in [(near, far), (far, near)] {
        let pair = towers_at(&[first, second]);
        let alone_first = towers_at(&[first]);
        let together = pixel_in(&pair, DrawStyle::XRay);
        let a = pixel_in(&alone_first, DrawStyle::XRay);
        // The second tower alone is drawn in the first tower's colour (colours go by index), so
        // take its light from the pair's second colour by drawing it second behind nothing:
        // a pair whose first tower is far off to the side.
        let aside = towers_at(&[[30.0, 0.0], second]);
        let b = pixel_in(&aside, DrawStyle::XRay);
        let background = linear(BACKGROUND_SRGB[0]);
        for channel in 0..3 {
            // Additive, depth off: the light of both towers, less the background counted twice.
            let expected = linear(a[channel]) + linear(b[channel]) - background;
            if expected < 0.95 {
                let found = linear(together[channel]);
                assert!(
                    (found - expected).abs() < 0.02,
                    "x-ray, channel {channel}: {found:.3} is not the sum {expected:.3} of the two towers"
                );
                assert!(
                    found > linear(a[channel]) + 0.005 && found > linear(b[channel]) + 0.005,
                    "and it is more than either"
                );
            }
        }
        // Control: with depth (flat), the same pixel is the nearer tower alone, whichever was drawn first.
        let nearer_index = if first == near { 0 } else { 1 };
        let flat = pixel_in(&pair, DrawStyle::Flat);
        assert!(
            close(flat, tower_srgb(nearer_index), 8),
            "flat: {flat:?} should be the nearer tower's colour"
        );
    }
}

#[test]
fn gpu_shading_follows_the_light() {
    let (gpu, renderer) = setup();
    let scene = dev::void();
    // Close on the upper launch-point sphere of tower 1, from the south: it fills a fifth of the picture.
    let sphere = on_tower(&scene, 0, scene.array.instrument.ifo_separation);
    let view = |style| ViewSpec {
        target: sphere.to_array(),
        yaw: -90.0,
        pitch: 0.0,
        distance: 1.0,
        projection: Projection::Perspective,
        style,
    };
    let camera = camera(&view(DrawStyle::Shaded));
    // Two points on the sphere's face, half its radius out along each axis: up and to the west (towards the key
    // light, which comes from the south-west and above), and down and to the east (away from it).
    let lit = camera.pixel(sphere + Vec3::new(-0.08, 0.0, 0.08), SIZE).unwrap();
    let unlit = camera.pixel(sphere + Vec3::new(0.08, 0.0, -0.08), SIZE).unwrap();
    assert!((lit[0] - unlit[0]).abs() > 40.0, "the two samples are well apart on screen");
    let sample = |picture: &Picture, at: [f64; 2]| picture.at(at[0].round() as u32, at[1].round() as u32);
    let shaded = render(&gpu, &renderer, &scene, &view(DrawStyle::Shaded));
    let flat = render(&gpu, &renderer, &scene, &view(DrawStyle::Flat));
    let (bright, dark) = (sample(&shaded, lit), sample(&shaded, unlit));
    // The side facing the light is clearly brighter; a constant "lighting" would make them equal.
    assert!(
        i32::from(bright[1]) - i32::from(dark[1]) > 30,
        "shaded: towards the light {bright:?}, away from it {dark:?}"
    );
    // Both are the tower's hue: red over green over blue, as its colour is.
    for pixel in [bright, dark] {
        assert!(pixel[0] > pixel[1] && pixel[1] > pixel[2], "{pixel:?} is not the tower's colour, shaded");
    }
    // Control: flat is unlit. The same two points are the same colour, the tower's own.
    let (a, b) = (sample(&flat, lit), sample(&flat, unlit));
    assert!(close(a, b, 1) && close(a, tower_srgb(0), 2), "flat: {a:?} and {b:?}");
}

cav_validate::case! {
    test: validation_gpu_picture_is_the_same_10_km_from_the_origin,
    name: "render.gpu_local_frame_at_10_km",
    subsystem: "render",
    slice: "S0",
    reference: Exact,
    against: "end to end on the GPU: the infinite-floor scene moved by (10000, -7000, 300) m, camera and all, against the same scene at the origin; bytes of the picture that differ",
    unit: Count,
    tolerance: 0.0,
    measure: {
        let (gpu, renderer) = setup();
        let shift = [10_000.0, -7_000.0, 300.0];
        let at_origin = dev::infinite_floor();
        let mut far = at_origin.clone();
        for tower in &mut far.array.towers {
            for axis in 0..3 {
                tower.position[axis] += shift[axis];
            }
        }
        far.floor.as_mut().unwrap().height += shift[2];
        let view = ViewSpec { pitch: 12.0, ..ViewSpec::home(&at_origin) };
        let mut moved = view.clone();
        for axis in 0..3 {
            moved.target[axis] += shift[axis];
        }
        let a = render(&gpu, &renderer, &at_origin, &view);
        let b = render(&gpu, &renderer, &far, &moved);
        cav_validate::Measured::Error(a.rgba.iter().zip(&b.rgba).filter(|(x, y)| x != y).count() as f64)
    },
}
