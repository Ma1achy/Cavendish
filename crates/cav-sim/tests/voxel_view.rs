//! What a view draws of voxel bodies: cubes, overlays, the legend's range (the S3 brief, R10, R11).

use std::sync::Arc;

use cav_sim::voxel_view::{Colouring, LeafField, MAX_DRAWN_CUBES, Overlay, voxel_frame};
use cav_sim::{BodySpec, Motion, ShapeKind, ShapeSpec, dev};

fn trees(scene: &cav_sim::SceneSpec) -> Vec<Option<Arc<cav_core::voxel::VoxTree>>> {
    scene.bodies.iter().map(|b| b.shape.as_ref().map(|s| Arc::new(s.tree(b.mass).unwrap()))).collect()
}

#[test]
fn a_legend_is_made_from_what_is_drawn() {
    let scene = dev::platonic_row();
    let trees = trees(&scene);
    for field in LeafField::ALL {
        let frame = voxel_frame(&scene, &trees, Colouring { field, tower: 0 }, Overlay::default(), 0.0);
        assert!(frame.notes.is_empty(), "{field:?}: {:?}", frame.notes);
        let values: Vec<f64> = frame.bodies.iter().flat_map(|b| b.cubes.iter().map(|c| c.value)).collect();
        let (least, most) =
            values.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(*v), b.max(*v)));
        let range = frame.range.unwrap();
        if field.diverging() {
            let m = least.abs().max(most.abs());
            assert_eq!(range, (-m, m), "{field:?}");
            // Phase: the leaves' shares add up to the body's reading, sign and all.
            assert!(m > 0.0);
        } else {
            assert_eq!(range, (least, most), "{field:?}: the legend's ends are the drawn extremes");
        }
    }
    // Every leaf is drawn as itself: the five bodies' leaves, no more.
    let frame = voxel_frame(&scene, &trees, Colouring::default(), Overlay::default(), 0.0);
    let drawn: usize = frame.bodies.iter().map(|b| b.cubes.len()).sum();
    let leaves: usize = trees.iter().flatten().map(|t| t.leaves.len()).sum();
    assert_eq!(drawn, leaves);
    // Control: a legend fixed beforehand (0 to 1 for `fill`) is not the drawn range here: the
    // solids' interiors are full and their boundaries partly so, never empty.
    let fill =
        voxel_frame(&scene, &trees, Colouring { field: LeafField::Fill, tower: 0 }, Overlay::default(), 0.0);
    let (low, high) = fill.range.unwrap();
    assert!(low > 0.0 && (high - 1.0).abs() < 1e-12, "{low} {high}");
}

#[test]
fn the_phases_of_the_leaves_add_up_to_the_body_s() {
    let scene = dev::unit_cube();
    let trees = trees(&scene);
    let frame =
        voxel_frame(&scene, &trees, Colouring { field: LeafField::Phase, tower: 0 }, Overlay::default(), 3.0);
    let sum: f64 = frame.bodies[0].cubes.iter().map(|c| c.value).sum();
    let shots = cav_sim::read::Shots::together(&scene, vec![3.0]);
    let whole =
        cav_sim::read::contribution(&scene, cav_sim::read::SourceId::Body(0), &shots, &|| false).unwrap();
    assert!(
        (sum - whole.gradiometer[0]).abs() <= 1e-9 * whole.gradiometer[0].abs(),
        "{sum} against {}",
        whole.gradiometer[0]
    );
}

#[test]
fn too_many_leaves_are_drawn_a_level_up_and_said() {
    let mut scene = dev::void();
    let fine = ShapeSpec { kind: ShapeKind::Sphere { radius: 1.0 }, pitch: 1.0 / 64.0, supersample: 1 };
    scene.bodies.push(BodySpec::shaped("ball", 100.0, fine, Motion::Static { position: [2.0, 0.0, 2.0] }));
    let trees = trees(&scene);
    let leaves = trees[0].as_ref().unwrap().leaves.len();
    assert!(leaves > MAX_DRAWN_CUBES, "{leaves}");
    let frame = voxel_frame(&scene, &trees, Colouring::default(), Overlay::default(), 0.0);
    let drawn = frame.bodies[0].cubes.len();
    assert!(drawn <= MAX_DRAWN_CUBES && frame.bodies[0].cut > 0, "{drawn} at {}", frame.bodies[0].cut);
    assert!(frame.notes.iter().any(|n| n.contains("cut at level")), "{:?}", frame.notes);
    // The cut drawn still holds every leaf's mass.
    let mass: f64 =
        voxel_frame(&scene, &trees, Colouring { field: LeafField::Mass, tower: 0 }, Overlay::default(), 0.0)
            .bodies[0]
            .cubes
            .iter()
            .map(|c| c.value)
            .sum();
    assert!((mass - 100.0).abs() < 1e-10, "{mass}");
    // Control: a small body is drawn leaf by leaf and says nothing.
    let small = dev::unit_cube();
    assert!(
        voxel_frame(&small, &self::trees(&small), Colouring::default(), Overlay::default(), 0.0)
            .notes
            .is_empty()
    );
}

#[test]
fn overlays_give_a_box_for_each_leaf_and_each_entry_of_the_cut() {
    let scene = dev::sphere();
    let trees = trees(&scene);
    let tree = trees[0].as_ref().unwrap();
    let leaves = voxel_frame(&scene, &trees, Colouring::default(), Overlay { leaves: true, cut: None }, 0.0);
    assert_eq!(leaves.boxes.len(), tree.leaves.len());
    // Each box is its leaf's level.
    let levels: Vec<u8> = tree.leaves.iter().map(|l| l.level).collect();
    assert_eq!(leaves.boxes.iter().map(|b| b.level).collect::<Vec<_>>(), levels);
    let cut = voxel_frame(&scene, &trees, Colouring::default(), Overlay { leaves: false, cut: Some(2) }, 0.0);
    assert_eq!(cut.boxes.len(), tree.cut(2).len());
    assert!(cut.boxes.len() < leaves.boxes.len());
    // Control: no overlay, no boxes.
    assert!(voxel_frame(&scene, &trees, Colouring::default(), Overlay::default(), 0.0).boxes.is_empty());
}
