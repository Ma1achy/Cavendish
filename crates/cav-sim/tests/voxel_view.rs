//! What a view draws of voxel bodies: cubes, overlays, the legend's range (the S3 brief, R10, R11).

use std::sync::Arc;

use cav_sim::voxel_view::{
    Colouring, LeafField, MAX_DRAWN_CUBES, MAX_TREE_BOXES, Overlay, TreeOverlay, voxel_frame,
};
use cav_sim::{BodySpec, Motion, ShapeKind, ShapeSpec, dev};

fn trees(scene: &cav_sim::SceneSpec) -> Vec<Option<Arc<cav_core::voxel::VoxTree>>> {
    scene.bodies.iter().map(|b| b.shape.as_ref().map(|s| Arc::new(s.tree(b.mass).unwrap()))).collect()
}

#[test]
fn a_legend_is_made_from_what_is_drawn() {
    let scene = dev::platonic_row();
    let trees = trees(&scene);
    // The phase is each leaf read on its own: the unit cube's few leaves show the same rule at a
    // fraction of the cost of the five solids'.
    let cube = dev::unit_cube();
    let cube_trees = self::trees(&cube);
    for field in LeafField::ALL {
        let (scene, trees) = if field == LeafField::Phase { (&cube, &cube_trees) } else { (&scene, &trees) };
        let frame = voxel_frame(scene, trees, Colouring { field, tower: 0 }, Overlay::default(), 0.0);
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
    // Tower 2 (index 1) carries its own instrument, T = 0.9 s against the array's 0.73: the leaves
    // coloured for it must add up to what it reads, not to what the array's instrument would.
    let mut scene = dev::unit_cube();
    assert!(scene.array.towers.len() >= 2);
    scene.array.towers[1].instrument =
        Some(cav_sim::InstrumentOverride { half_sequence: Some(0.9), ..Default::default() });
    let trees = trees(&scene);
    let shots = cav_sim::read::Shots::together(&scene, vec![3.0]);
    let whole =
        cav_sim::read::contribution(&scene, cav_sim::read::SourceId::Body(0), &shots, &|| false).unwrap();
    let mut sums = Vec::new();
    for tower in [0, 1] {
        let frame = voxel_frame(
            &scene,
            &trees,
            Colouring { field: LeafField::Phase, tower },
            Overlay::default(),
            3.0,
        );
        assert!(frame.notes.is_empty(), "{:?}", frame.notes);
        let sum: f64 = frame.bodies[0].cubes.iter().map(|c| c.value).sum();
        let reads = whole.gradiometer[shots.starts[tower]];
        assert!((sum - reads).abs() <= 1e-9 * reads.abs(), "tower {}: {sum} against {reads}", tower + 1);
        sums.push(sum);
    }
    // Control: the override matters here; the array's instrument at tower 2 reads otherwise.
    let mut plain = scene.clone();
    plain.array.towers[1].instrument = None;
    let frame =
        voxel_frame(&plain, &trees, Colouring { field: LeafField::Phase, tower: 1 }, Overlay::default(), 3.0);
    let other: f64 = frame.bodies[0].cubes.iter().map(|c| c.value).sum();
    assert!((other - sums[1]).abs() > 1e-3 * sums[1].abs(), "{other} against {}", sums[1]);
}

/// The unit cube's scene with its cube made a ball of many leaves (tens of thousands, so that a
/// few hundred voxel bodies are enough to crowd it out of the kernel's work).
fn crowdable() -> cav_sim::SceneSpec {
    let mut scene = dev::unit_cube();
    scene.bodies[0].shape =
        Some(ShapeSpec { kind: ShapeKind::Sphere { radius: 0.5 }, pitch: 1.0 / 64.0, supersample: 1 });
    scene
}

/// [`crowdable`] with `copies` more of its ball placed far along x: every one a voxel body, so
/// each has `1/(copies + 1)` of one read's kernel work.
fn with_copies(copies: usize) -> cav_sim::SceneSpec {
    let mut scene = crowdable();
    let cube = scene.bodies[0].clone();
    for k in 0..copies {
        let mut copy = cube.clone();
        copy.label = format!("copy {}", k + 1);
        copy.motion = Motion::Static { position: [100.0 + 2.0 * k as f64, 0.0, 2.5] };
        scene.bodies.push(copy);
    }
    scene
}

#[test]
fn the_kernel_s_work_is_shared_by_the_scene_s_voxel_bodies() {
    use cav_core::voxel::kernel::MAX_KERNEL_WORK;
    use cav_sim::read::{SourceId, kernel_share};
    let alone = crowdable();
    let leaves = trees(&alone)[0].as_ref().unwrap().leaves.len() as u64;
    let nodes = (0..alone.array.towers.len())
        .map(|t| alone.array.instrument_of(t).instrument().node_count(&[]))
        .max()
        .unwrap();
    // The read's first rule costs 4 cube evaluations a node for each leaf; colouring by phase,
    // whose read of a leaf may refine to three times the nodes, at most 12.
    let first = 4 * nodes * leaves;
    assert!(3 * first <= MAX_KERNEL_WORK, "the cube alone must fit: {first}");
    // Enough voxel bodies that the cube's share is less than its first rule.
    let copies = usize::try_from(MAX_KERNEL_WORK / first).unwrap();
    let crowded = with_copies(copies);
    assert!(
        crowded.bodies.len() <= cav_sim::spec::limits::BODIES,
        "{} bodies, {leaves} leaves",
        crowded.bodies.len()
    );
    assert!(kernel_share(&crowded) < first && kernel_share(&alone) >= 3 * first);
    let shots = |scene| cav_sim::read::Shots::together(scene, vec![3.0]);
    // The read refuses the cube and says why; the view does not colour it by phase and says why.
    let read = cav_sim::read::contribution(&crowded, SourceId::Body(0), &shots(&crowded), &|| false).unwrap();
    assert!(
        read.warnings.iter().any(|w| w.text.contains("`cube` is not read") && w.text.contains("shared by")),
        "{:?}",
        read.warnings
    );
    assert!(!read.settled && read.gradiometer.iter().all(|&g| g == 0.0));
    let phase = Colouring { field: LeafField::Phase, tower: 0 };
    let mut few = trees(&alone);
    few.resize(crowded.bodies.len(), None);
    let frame = voxel_frame(&crowded, &few, phase, Overlay::default(), 3.0);
    assert!(frame.notes.iter().any(|n| n.contains("`cube` is not coloured by phase")), "{:?}", frame.notes);
    // Control: the cube alone is read and coloured, and says nothing.
    let read = cav_sim::read::contribution(&alone, SourceId::Body(0), &shots(&alone), &|| false).unwrap();
    assert!(read.warnings.is_empty() && read.gradiometer.iter().all(|&g| g != 0.0), "{:?}", read.warnings);
    let frame = voxel_frame(&alone, &trees(&alone), phase, Overlay::default(), 3.0);
    assert!(frame.notes.is_empty() && frame.range.is_some_and(|(_, hi)| hi > 0.0), "{:?}", frame.notes);
}

#[test]
fn a_scene_with_no_towers_says_why_it_has_no_distances() {
    let mut scene = dev::unit_cube();
    let trees = trees(&scene);
    let colouring = Colouring { field: LeafField::Distance, tower: 0 };
    // Control: with towers, distances are drawn and nothing is said.
    let frame = voxel_frame(&scene, &trees, colouring, Overlay::default(), 0.0);
    assert!(frame.notes.is_empty() && frame.range.is_some(), "{:?}", frame.notes);
    scene.array.towers.clear();
    let frame = voxel_frame(&scene, &trees, colouring, Overlay::default(), 0.0);
    assert!(frame.range.is_none());
    assert!(frame.notes.iter().any(|n| n.contains("no towers")), "{:?}", frame.notes);
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
    let leaves = voxel_frame(
        &scene,
        &trees,
        Colouring::default(),
        Overlay { leaves: true, ..Overlay::default() },
        0.0,
    );
    assert_eq!(leaves.boxes.len(), tree.leaves.len());
    // Each box is its leaf's level.
    let levels: Vec<u8> = tree.leaves.iter().map(|l| l.level).collect();
    assert_eq!(leaves.boxes.iter().map(|b| b.level).collect::<Vec<_>>(), levels);
    let cut = voxel_frame(
        &scene,
        &trees,
        Colouring::default(),
        Overlay { cut: Some(2), ..Overlay::default() },
        0.0,
    );
    assert_eq!(cut.boxes.len(), tree.cut(2).len());
    assert!(cut.boxes.len() < leaves.boxes.len());
    // Control: no overlay, no boxes.
    assert!(voxel_frame(&scene, &trees, Colouring::default(), Overlay::default(), 0.0).boxes.is_empty());
}

fn tree_frame(
    scene: &cav_sim::SceneSpec,
    trees: &[Option<Arc<cav_core::voxel::VoxTree>>],
    tree: TreeOverlay,
    field: LeafField,
) -> cav_sim::voxel_view::VoxelFrame {
    voxel_frame(
        scene,
        trees,
        Colouring { field, tower: 0 },
        Overlay { tree: Some(tree), ..Overlay::default() },
        0.0,
    )
}

#[test]
fn the_tree_overlay_draws_every_node_by_its_depth() {
    let scene = dev::sphere();
    let trees = trees(&scene);
    let tree = trees[0].as_ref().unwrap();
    let frame = tree_frame(&scene, &trees, TreeOverlay::default(), LeafField::Level);
    // Every internal node and every leaf, once, at its depth from the root.
    assert_eq!(frame.tree.len(), tree.nodes.len() + tree.leaves.len());
    let mut want: Vec<u8> = tree.nodes.iter().map(|n| tree.grid.depth - n.level).collect();
    want.extend(tree.leaves.iter().map(|l| tree.grid.depth - l.level));
    want.sort_unstable();
    assert_eq!(frame.tree.iter().map(|b| b.depth).collect::<Vec<_>>(), want);
    assert_eq!(frame.tree[0].depth, 0, "the root first");
    assert_eq!(frame.depths, Some((0, *want.last().unwrap())));
    // Each box is its block: side 2^level pitches.
    for b in &frame.tree {
        let side = tree.side(tree.grid.depth - b.depth);
        assert!(((b.hi.x - b.lo.x) - side).abs() < 1e-12);
    }
    // A range of depths draws those and only those, and the legend runs over them.
    let some = tree_frame(
        &scene,
        &trees,
        TreeOverlay { from: 1, to: Some(2), ..TreeOverlay::default() },
        LeafField::Level,
    );
    assert!(some.tree.iter().all(|b| (1..=2).contains(&b.depth)));
    assert_eq!(some.tree.len(), want.iter().filter(|d| (1..=2).contains(*d)).count());
    assert_eq!(some.depths, Some((1, 2)));
    assert!(some.tree_capped_at.is_none() && some.notes.is_empty(), "{:?}", some.notes);
    // Alone, the tree hides the cubes; control: not alone, they are drawn.
    assert!(!frame.bodies[0].cubes.is_empty());
    let alone =
        tree_frame(&scene, &trees, TreeOverlay { alone: true, ..TreeOverlay::default() }, LeafField::Level);
    assert!(alone.bodies[0].cubes.is_empty() && alone.tree.len() == frame.tree.len());
    // Control: no tree overlay, no nodes; a range with nothing in it says so.
    assert!(voxel_frame(&scene, &trees, Colouring::default(), Overlay::default(), 0.0).tree.is_empty());
    let none =
        tree_frame(&scene, &trees, TreeOverlay { from: 30, ..TreeOverlay::default() }, LeafField::Level);
    assert!(none.tree.is_empty() && none.notes.iter().any(|n| n.contains("no node")), "{:?}", none.notes);
}

#[test]
fn leaves_coloured_by_depth_agree_with_the_tree() {
    let scene = dev::sphere();
    let trees = trees(&scene);
    let frame = tree_frame(&scene, &trees, TreeOverlay::default(), LeafField::Depth);
    let (a, b) = frame.depths.unwrap();
    assert_eq!(frame.range, Some((f64::from(a), f64::from(b))), "one map for leaves and nodes");
    // A leaf's cube and its tree box: one depth.
    let tree = trees[0].as_ref().unwrap();
    for (cube, leaf) in frame.bodies[0].cubes.iter().zip(&tree.leaves) {
        assert_eq!(cube.value, f64::from(tree.grid.depth - leaf.level));
    }
    // Control: with the tree's range cut to its root, the leaves' range is still their own and
    // the map takes in both.
    let root =
        tree_frame(&scene, &trees, TreeOverlay { to: Some(0), ..TreeOverlay::default() }, LeafField::Depth);
    let leaf_lo = frame.bodies[0].cubes.iter().map(|c| c.value).fold(f64::INFINITY, f64::min);
    assert_eq!(root.range, Some((0.0, frame.range.unwrap().1)));
    assert!(leaf_lo > 0.0);
}

#[test]
fn opacity_is_monotone_in_depth() {
    use cav_sim::voxel_view::{FAINTEST, Fade, opacity};
    let range = (1, 6);
    for d in 1..6 {
        assert!(opacity(d, range, Fade::Deep) < opacity(d + 1, range, Fade::Deep));
        assert!(opacity(d, range, Fade::Shallow) > opacity(d + 1, range, Fade::Shallow));
        assert_eq!(opacity(d, range, Fade::Off), 1.0);
    }
    assert_eq!((opacity(1, range, Fade::Deep), opacity(6, range, Fade::Deep)), (FAINTEST, 1.0));
    assert_eq!((opacity(1, range, Fade::Shallow), opacity(6, range, Fade::Shallow)), (1.0, FAINTEST));
    // One depth drawn is solid, whatever the fade.
    assert_eq!(opacity(3, (3, 3), Fade::Deep), 1.0);
    assert_eq!(opacity(3, (3, 3), Fade::Shallow), 1.0);
}

#[test]
fn the_tree_overlay_stops_at_whole_depths_within_its_cap() {
    let mut scene = dev::void();
    let fine = ShapeSpec { kind: ShapeKind::Sphere { radius: 1.0 }, pitch: 1.0 / 64.0, supersample: 1 };
    scene.bodies.push(BodySpec::shaped("ball", 100.0, fine, Motion::Static { position: [2.0, 0.0, 2.0] }));
    let trees = trees(&scene);
    let tree = trees[0].as_ref().unwrap();
    let all = tree.nodes.len() + tree.leaves.len();
    assert!(all > MAX_TREE_BOXES, "{all}");
    let frame = tree_frame(&scene, &trees, TreeOverlay::default(), LeafField::Level);
    let capped = frame.tree_capped_at.expect("capped");
    assert!(frame.tree.len() <= MAX_TREE_BOXES);
    // Whole depths, the shallowest: every block to `capped`, none deeper, and one more depth
    // would not have fitted.
    let upto = |d: u8| {
        tree.nodes.iter().filter(|n| tree.grid.depth - n.level <= d).count()
            + tree.leaves.iter().filter(|l| tree.grid.depth - l.level <= d).count()
    };
    assert_eq!(frame.tree.len(), upto(capped));
    assert!(upto(capped + 1) > MAX_TREE_BOXES);
    assert!(frame.notes.iter().any(|n| n.contains("drawn to depth")), "{:?}", frame.notes);
    // A range that starts at a depth past the cap on its own: nothing is drawn (no part of the
    // depth chosen by the blocks' order), and the frame names the depth that would fit.
    let at = |d: u8| upto(d) - d.checked_sub(1).map_or(0, upto);
    let past = (0..=tree.grid.depth).find(|&d| at(d) > MAX_TREE_BOXES).expect("a depth past the cap");
    assert!(at(past - 1) <= MAX_TREE_BOXES);
    let deep =
        tree_frame(&scene, &trees, TreeOverlay { from: past, ..TreeOverlay::default() }, LeafField::Level);
    assert!(deep.tree.is_empty() && deep.depths.is_none() && deep.tree_capped_at.is_none());
    let want = format!("depth {past} alone has {} tree nodes", at(past));
    assert!(deep.notes.iter().any(|n| n.contains(&want)), "{:?}", deep.notes);
    assert!(deep.notes.iter().any(|n| n.contains(&format!("depth {} and shallower fit", past - 1))));
    // Control: the range starting one depth shallower draws that depth whole and stops there.
    let one_up = tree_frame(
        &scene,
        &trees,
        TreeOverlay { from: past - 1, ..TreeOverlay::default() },
        LeafField::Level,
    );
    assert_eq!(one_up.tree.len(), at(past - 1));
    assert_eq!((one_up.tree_capped_at, one_up.depths), (Some(past - 1), Some((past - 1, past - 1))));
    // Control: the small sphere is drawn whole and says nothing.
    let small = dev::sphere();
    let f = tree_frame(&small, &self::trees(&small), TreeOverlay::default(), LeafField::Level);
    assert!(f.tree_capped_at.is_none() && f.notes.is_empty());
}

fn cut(
    scene: &cav_sim::SceneSpec,
    trees: &[Option<Arc<cav_core::voxel::VoxTree>>],
    overlay: Overlay,
    offset: f64,
    eye: cav_core::Vec3,
) -> cav_sim::voxel_view::VoxelFrame {
    use cav_sim::voxel_view::{Cutting, Section, voxel_frame_cut};
    let cutting = Cutting { section: Section { axis: cav_sim::Axis::Z, offset }, eye };
    voxel_frame_cut(scene, trees, Colouring::default(), overlay, Some(cutting), 0.0)
}

/// Whether some cell of `frame`'s first body covers the point `(x, y)` of its plane.
fn covers(frame: &cav_sim::voxel_view::VoxelFrame, x: f64, y: f64) -> bool {
    frame.bodies[0].cells.iter().any(|c| c.lo.x <= x && x <= c.hi.x && c.lo.y <= y && y <= c.hi.y)
}

#[test]
fn a_section_shows_a_hollow_body_empty_inside_and_a_solid_one_full() {
    let top = cav_core::Vec3::Z;
    let hollow = dev::hollow_sphere();
    let solid = dev::sphere();
    let (hollow_trees, solid_trees) = (trees(&hollow), trees(&solid));
    // Through the middle (the bodies' centres are at (2, 0, 2.5)), seen from above.
    let through = |scene, trees: &[_], dz: f64| cut(scene, trees, Overlay::default(), 2.5 + dz, top);
    let ring = through(&hollow, &hollow_trees, 0.01);
    assert!(ring.bodies[0].cubes.is_empty(), "a plan view draws the section alone");
    assert!(ring.bodies[0].cells.iter().all(|c| c.lo.z == 2.51 && c.hi.z == 2.51));
    assert!(!covers(&ring, 2.0, 0.0), "the cavity is empty");
    assert!(covers(&ring, 2.0 + 0.42, 0.0), "the wall is material");
    // Control: the solid sphere is full at its middle, and the shell is full above its cavity.
    assert!(covers(&through(&solid, &solid_trees, 0.01), 2.0, 0.0));
    assert!(covers(&through(&hollow, &hollow_trees, 0.42), 2.0, 0.0));
    // A plane that misses the body draws nothing of it.
    assert!(through(&solid, &solid_trees, 0.7).bodies[0].cells.is_empty());
}

#[test]
fn the_quadtree_of_a_section_is_the_octree_cut_by_the_plane() {
    let scene = dev::sphere();
    let trees = trees(&scene);
    let tree = trees[0].as_ref().unwrap();
    let tree_overlay = Overlay { tree: Some(TreeOverlay::default()), ..Overlay::default() };
    for dz in [-0.3, 0.01, 0.2] {
        let offset = 2.5 + dz;
        let frame = cut(&scene, &trees, tree_overlay, offset, cav_core::Vec3::Z);
        // The blocks the plane passes through: low face in, high face out.
        let side = |level: u8| tree.side(level);
        let crossed = |key: u32, level: u8| {
            let z = 2.5 + tree.block_centre(key, level).z;
            z - 0.5 * side(level) <= offset && offset < z + 0.5 * side(level)
        };
        let want = tree.nodes.iter().filter(|n| crossed(n.key, n.level)).count()
            + tree.leaves.iter().filter(|l| crossed(l.key, l.level)).count();
        assert_eq!(frame.tree.len(), want, "{dz}");
        // Squares in the plane (a fiftieth of a pitch towards the camera), one per block.
        assert!(
            frame.tree.iter().all(|b| b.lo.z == b.hi.z && (b.lo.z - offset - 0.02 * side(0)).abs() < 1e-12)
        );
        assert!(frame.tree.iter().all(|b| (b.hi.x - b.lo.x - side(tree.grid.depth - b.depth)).abs() < 1e-12));
        // Control: uncut, the tree is every block.
        assert!(want < tree.nodes.len() + tree.leaves.len());
    }
}

#[test]
fn a_cut_away_keeps_the_half_away_from_the_camera() {
    let scene = dev::sphere();
    let trees = trees(&scene);
    let iso = cav_sim::voxel_view::eye_direction(-45.0, 35.0);
    let below = cut(&scene, &trees, Overlay::default(), 2.51, iso);
    let b = &below.bodies[0];
    assert!(!b.cubes.is_empty() && !b.cells.is_empty());
    assert!(b.cubes.iter().all(|c| c.centre.z + 0.5 * c.side <= 2.51));
    assert!(b.cells.iter().all(|c| c.hi.z == 2.51 && c.lo.z < 2.51));
    // From below, the other half.
    let from_below = cav_sim::voxel_view::eye_direction(-45.0, -35.0);
    let above = cut(&scene, &trees, Overlay::default(), 2.51, from_below);
    assert!(above.bodies[0].cubes.iter().all(|c| c.centre.z - 0.5 * c.side >= 2.51));
    assert!(above.bodies[0].cells.iter().all(|c| c.lo.z == 2.51 && c.hi.z > 2.51));
    // Control: uncut, the cubes reach both sides.
    let whole = voxel_frame(&scene, &trees, Colouring::default(), Overlay::default(), 0.0);
    assert!(whole.bodies[0].cubes.iter().any(|c| c.centre.z > 2.5) && whole.bodies[0].cells.is_empty());
    // The span a section slider runs over: the body's box along z.
    assert_eq!(cav_sim::voxel_view::section_span(&scene, cav_sim::Axis::Z, 0.0), Some((2.0, 3.0)));
}
