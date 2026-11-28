//! What a view's draw mode draws of bodies with a shape (`docs/slices/S5.md` §3, D1, and §7.12).

use std::sync::Arc;

use cav_core::Vec3;
use cav_core::voxel::VoxTree;
use cav_core::voxel::tree::CutEntry;
use cav_sim::body_draw::{BodyDraw, MAX_DRAWN_TRIANGLES, Surface, enclosed_volume, surface};
use cav_sim::voxel_view::{Colouring, LeafField, Look, Overlay, voxel_frame_drawn};
use cav_sim::{Representation, SceneSpec, ShapeSpec, dev};

fn trees(scene: &SceneSpec) -> Vec<Option<Arc<VoxTree>>> {
    scene
        .bodies
        .iter()
        .map(|b| b.shape.as_ref().map(|s| Arc::new(s.tree(b.mass, &scene.materials).unwrap())))
        .collect()
}

fn surfaces(scene: &SceneSpec) -> Vec<Option<Arc<Surface>>> {
    scene.bodies.iter().map(|b| b.shape.as_ref().map(|s| Arc::new(surface(&s.kind).unwrap()))).collect()
}

fn look(draw: BodyDraw) -> Look {
    Look {
        colouring: Colouring { field: LeafField::Height, tower: 0 },
        overlay: Overlay::default(),
        cutting: None,
        draw,
    }
}

/// Each leaf's exact fill: its piece's share of its cell when it is cut, 1 when it is full.
fn exact_fill(tree: &VoxTree, leaf: usize) -> f64 {
    let l = &tree.leaves[leaf];
    tree.cut_of(l).map_or(1.0, |c| c.piece.volume / tree.grid.pitch.powi(3))
}

#[test]
fn draw_modes_draw_what_they_say() {
    // D1 (criteria in `docs/slices/S5.md` §7.12, written before measuring).
    let scene = dev::cut_cells();
    let trees = trees(&scene);
    let surfaces = surfaces(&scene);
    assert!(trees.iter().flatten().all(|t| !t.cuts.is_empty()), "both bodies are read by cut cells");

    // Fill: whatever the view's colouring, each drawn leaf's value is its exact fill, within
    // 1e-12; the legend's field is the fill's.
    let fill = voxel_frame_drawn(&scene, &trees, &surfaces, look(BodyDraw::Fill), 0.0);
    assert_eq!(fill.colouring.field, LeafField::Fill);
    let mut worst = 0f64;
    let mut partial = 0;
    for (body, tree) in fill.bodies.iter().zip(trees.iter().flatten()) {
        assert_eq!(body.cut, 0, "every leaf drawn as itself");
        let cut = tree.cut(0);
        assert_eq!(body.cubes.len(), cut.len());
        for (cube, entry) in body.cubes.iter().zip(cut) {
            let CutEntry::Leaf(l) = entry else { panic!("a level-0 cut is of leaves") };
            let want = exact_fill(tree, l);
            worst = worst.max((cube.value - want).abs());
            partial += usize::from(want < 1.0);
        }
        assert!(body.surface.is_none(), "fill draws no surface");
    }
    eprintln!("fill: worst {worst:e} over {partial} boundary cells");
    assert!(worst <= 1e-12, "fill off the exact by {worst:e}");
    assert!(partial > 100, "{partial} boundary cells");
    // Control: drawn as voxels, the view's own field colours them (heights, not fills).
    let voxels = voxel_frame_drawn(&scene, &trees, &surfaces, look(BodyDraw::Voxels), 0.0);
    assert_eq!(voxels.colouring.field, LeafField::Height);
    assert!(voxels.bodies[0].cubes.iter().any(|c| (c.value - 1.0).abs() > 0.5));

    // A body read Sampled and shaded by the fill of its tree clipped for the picture: the exact
    // fill again. Control: its own sampled tree's fill (the count of k³ points) is off the exact by
    // more than 1e-3 in some cell, so the clipping is what makes it exact.
    let mut sampled = scene.clone();
    sampled.bodies.truncate(1);
    sampled.bodies[0].shape.as_mut().unwrap().representation = Representation::Sampled;
    let own = Arc::new(sampled.bodies[0].shape.as_ref().unwrap().tree(200.0, &[]).unwrap());
    assert!(own.cuts.is_empty());
    let clipped = Arc::new(
        ShapeSpec { representation: Representation::Fill, ..sampled.bodies[0].shape.clone().unwrap() }
            .tree(200.0, &[])
            .unwrap(),
    );
    let frame = voxel_frame_drawn(&sampled, &[Some(Arc::clone(&clipped))], &[], look(BodyDraw::Fill), 0.0);
    for (cube, entry) in frame.bodies[0].cubes.iter().zip(clipped.cut(0)) {
        let CutEntry::Leaf(l) = entry else { panic!() };
        assert!((cube.value - exact_fill(&clipped, l)).abs() <= 1e-12);
    }
    let rough = voxel_frame_drawn(&sampled, &[Some(Arc::clone(&own))], &[], look(BodyDraw::Fill), 0.0);
    let exact_at = |centre: Vec3| {
        frame.bodies[0].cubes.iter().find(|c| (c.centre - centre).length() < 1e-9).map(|c| c.value)
    };
    let off = rough.bodies[0]
        .cubes
        .iter()
        .filter_map(|c| exact_at(c.centre).map(|e| (c.value - e).abs()))
        .fold(0.0, f64::max);
    eprintln!("the sampled fill is off the exact by up to {off:e}");
    assert!(off > 1e-3, "the sampled fill is within {off:e} of the exact everywhere");

    // Mesh: each body's own triangles, at the body's position, enclosing its volume; the voxels
    // are still made (for the overlays and the selection) but the frame says not to draw them.
    for draw in [BodyDraw::Mesh, BodyDraw::MeshOverVoxels] {
        let frame = voxel_frame_drawn(&scene, &trees, &surfaces, look(draw), 0.0);
        assert_eq!(frame.draw, draw);
        for (n, body) in frame.bodies.iter().enumerate() {
            let drawn = body.surface.as_ref().expect("a surface");
            let kind = &scene.bodies[n].shape.as_ref().unwrap().kind;
            let want = match kind {
                cav_sim::ShapeKind::Triangles { triangles, .. } => triangles.len(),
                _ => kind.faceted().unwrap().faces().len(),
            };
            assert_eq!(drawn.triangles.len(), want, "{}", scene.bodies[n].label);
            assert!(!drawn.tessellated);
            let volume = kind.solid().unwrap().volume();
            assert!(((enclosed_volume(&drawn.triangles) - volume) / volume).abs() < 1e-12);
            let position = Vec3::from_array(scene.bodies[n].motion.position(0.0, 9.81));
            assert!((body.at - position).length() < 1e-15);
        }
        assert!(frame.notes.is_empty(), "{:?}", frame.notes);
    }
    assert!(!BodyDraw::Mesh.draws_voxels() && BodyDraw::MeshOverVoxels.draws_voxels());
    // Control: drawn as voxels or by fill, no surface.
    assert!(voxels.bodies.iter().all(|b| b.surface.is_none()));
}

#[test]
fn surfaces_past_the_view_s_cap_are_said() {
    // X7: the surfaces drawn together stay within MAX_DRAWN_TRIANGLES; the body that would pass it
    // is drawn without its surface, and the frame says so with the numbers.
    let scene = dev::cut_cells();
    let trees = trees(&scene);
    let half = MAX_DRAWN_TRIANGLES / 2 + 1;
    let big = Arc::new(Surface { triangles: vec![[Vec3::ZERO, Vec3::X, Vec3::Y]; half], tessellated: false });
    let frame = voxel_frame_drawn(
        &scene,
        &trees,
        &[Some(Arc::clone(&big)), Some(Arc::clone(&big))],
        look(BodyDraw::Mesh),
        0.0,
    );
    assert!(frame.bodies[0].surface.is_some() && frame.bodies[1].surface.is_none());
    let said = frame.notes.iter().find(|n| n.contains("torus")).expect("said");
    assert!(said.contains(&half.to_string()) && said.contains(&MAX_DRAWN_TRIANGLES.to_string()), "{said}");
    // Control: within the cap, both are drawn and nothing is said.
    let small =
        Arc::new(Surface { triangles: vec![[Vec3::ZERO, Vec3::X, Vec3::Y]; half - 2], tessellated: false });
    let frame = voxel_frame_drawn(
        &scene,
        &trees,
        &[Some(Arc::clone(&small)), Some(small)],
        look(BodyDraw::Mesh),
        0.0,
    );
    assert!(frame.bodies.iter().all(|b| b.surface.is_some()) && frame.notes.is_empty());
}
