//! GPU tier: each body draw mode draws its own picture (`docs/slices/S5.md` §3, D1; §7.12).
//! Every test here has `gpu_` in its name, which is how `cargo xtask gpu` finds it.

use std::sync::Arc;

use cav_core::voxel::cut::piece_surfaces;
use cav_gpu::Gpu;
use cav_render::{Frame, Renderer};
use cav_sim::body_draw::{BodyDraw, MAX_DRAWN_TRIANGLES, surface};
use cav_sim::voxel_view::{Extras, Look, VoxelFrame, voxel_frame_drawn};
use cav_sim::{DrawStyle, NamedView, Projection, SceneSpec, ViewSpec, dev};

const SIZE: [u32; 2] = [640, 480];

/// The scene's frame of its voxel bodies in `draw`, as the engine makes it.
fn frame(scene: &SceneSpec, view: &ViewSpec) -> VoxelFrame {
    let trees: Vec<_> = scene
        .bodies
        .iter()
        .map(|b| b.shape.as_ref().map(|s| Arc::new(s.tree(b.mass, &scene.materials).unwrap())))
        .collect();
    let surfaces: Vec<Extras> = scene
        .bodies
        .iter()
        .zip(&trees)
        .map(|(b, tree)| {
            let (Some(shape), Some(tree)) = (&b.shape, tree) else { return Extras::default() };
            let all: Vec<usize> = (0..tree.cuts.len()).collect();
            let solid = shape.kind.solid().unwrap();
            Extras {
                surface: Some(Arc::new(surface(&shape.kind).unwrap())),
                pieces: piece_surfaces(solid.as_ref(), tree, &all, MAX_DRAWN_TRIANGLES).ok().map(Arc::new),
            }
        })
        .collect();
    let look = Look { colouring: view.colour, overlay: view.overlay, cutting: None, draw: view.body_draw };
    voxel_frame_drawn(scene, &trees, &surfaces, look, 0.0)
}

fn picture(gpu: &Gpu, renderer: &Renderer, scene: &SceneSpec, view: &ViewSpec) -> (Vec<u8>, f64) {
    let voxels = frame(scene, view);
    let frame = Frame {
        scene,
        view,
        highlight: None,
        time: 0.0,
        content: cav_sim::PaneContent::Scene,
        span: None,
        voxels: Some(&voxels),
        world: None,
    };
    let started = std::time::Instant::now();
    let rgba = renderer.render_rgba(gpu, frame, SIZE).expect("render");
    (rgba, started.elapsed().as_secs_f64() * 1e3)
}

/// The fraction of pixels where two pictures differ by more than `tolerance` in some channel.
fn differ(a: &[u8], b: &[u8], tolerance: u8) -> f64 {
    let n = a.len() / 4;
    let count = (0..n).filter(|i| (0..3).any(|c| a[4 * i + c].abs_diff(b[4 * i + c]) > tolerance)).count();
    count as f64 / n as f64
}

#[test]
fn gpu_each_draw_mode_draws() {
    // D1's pixels (criterion written before measuring, §7.12): on `cut-cells`, framed on the two
    // bodies, each mode's picture differs from the voxels' in at least 1 % of its pixels, and
    // each differs from every other mode's; a mode drawn twice is the same picture (the control
    // that the difference is the mode's, not noise).
    let gpu = Gpu::new().expect("the GPU tier needs an adapter");
    let renderer = Renderer::new(&gpu);
    let scene = dev::cut_cells();
    let mut view = ViewSpec::framing(&scene, NamedView::Iso, Projection::Perspective, DrawStyle::Shaded);
    view.target = [2.0, 0.05, 2.5];
    view.distance = 4.0;
    let mut pictures = Vec::new();
    for draw in BodyDraw::ALL {
        let mode = ViewSpec { body_draw: draw, ..view.clone() };
        let (rgba, first) = picture(&gpu, &renderer, &scene, &mode);
        let (again, ms) = picture(&gpu, &renderer, &scene, &mode);
        assert_eq!(differ(&rgba, &again, 0), 0.0, "{}: the same picture twice", draw.name());
        eprintln!("{}: {first:.1} ms, then {ms:.1} ms ({}x{})", draw.name(), SIZE[0], SIZE[1]);
        pictures.push((draw, rgba));
    }
    for (i, (a, pa)) in pictures.iter().enumerate() {
        for (b, pb) in &pictures[i + 1..] {
            let d = differ(pa, pb, 8);
            eprintln!("{} against {}: {:.2} % of pixels differ", a.name(), b.name(), 100.0 * d);
            assert!(d >= 0.01, "{} and {} differ in {:.3} % of pixels", a.name(), b.name(), 100.0 * d);
        }
    }
}
