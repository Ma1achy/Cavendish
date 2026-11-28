//! What the views draw of the world's levels (`docs/slices/S5.md` §3, L1 to L3; §7.14, §7.15).

use cav_core::Vec3;
use cav_core::voxel::world::{ChunkKey, WorldGrid};
use cav_sim::world::lod_view::{LodView, MAX_OVERLAY_NODES, ReadView, WorldOverlay, lod_frame, octree_boxes};
use cav_sim::world::octree::NodeKey;

/// An octree over a block of `nx × ny × nz` chunks: every chunk, and above them every node holding
/// one, level by level to a single root.
fn block_octree(nx: i32, ny: i32, nz: i32) -> ReadView {
    let mut nodes: Vec<NodeKey> = Vec::new();
    let mut level: Vec<ChunkKey> = Vec::new();
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                level.push(ChunkKey::new(x, y, z));
            }
        }
    }
    let mut l = 0u8;
    loop {
        level.sort_unstable();
        level.dedup();
        nodes.extend(level.iter().map(|&key| NodeKey { level: l, key }));
        if level.len() == 1 {
            break;
        }
        level = level
            .iter()
            .map(|k| ChunkKey::new(k.x.div_euclid(2), k.y.div_euclid(2), k.z.div_euclid(2)))
            .collect();
        l += 1;
    }
    ReadView {
        grid: Some(WorldGrid { origin: Vec3::ZERO, pitch: 0.1, supersample: 2 }),
        nodes,
        towers: Vec::new(),
    }
}

#[test]
fn the_octree_overlay_is_bounded() {
    // L3 (criterion in `docs/slices/S5.md` §7.14, written before measuring): `overlay octree` draws
    // the world's octree by level, a legend of exactly the levels drawn, at most 20,000 nodes:
    // past it whole levels are left out, the finest first, and the frame says which.
    let big = block_octree(60, 60, 6); // 21,600 chunks: level 0 alone is past the cap
    let frame = lod_frame(&LodView::Camera, WorldOverlay::Octree, Some(&big));
    let per_level = |l: u8| big.nodes.iter().filter(|n| n.level == l).count();
    assert!(per_level(0) > MAX_OVERLAY_NODES);
    assert!(frame.boxes.len() <= MAX_OVERLAY_NODES);
    assert!(frame.boxes.iter().all(|b| b.level >= 1), "level 0 is left out whole");
    let top = big.nodes.iter().map(|n| n.level).max().unwrap();
    assert_eq!(frame.boxes.len(), big.nodes.len() - per_level(0), "every coarser level is drawn");
    assert_eq!(frame.levels, (1..=top as i8).collect::<Vec<_>>(), "the legend is the levels drawn");
    assert!(frame.notes.iter().any(|n| n.contains("level 0 left out")), "{:?}", frame.notes);
    eprintln!("L3: {} nodes, {} drawn at levels {:?}", big.nodes.len(), frame.boxes.len(), frame.levels);
    // Each box is its node's.
    let grid = big.grid.unwrap();
    for (b, n) in frame.boxes.iter().zip(big.nodes.iter().filter(|n| n.level >= 1)) {
        assert_eq!(b.boxed, grid.brick_box(n.level, n.key));
    }
    // Control: an octree within the cap is drawn whole and nothing is said.
    let small = block_octree(30, 30, 6);
    let frame = lod_frame(&LodView::Camera, WorldOverlay::Octree, Some(&small));
    assert_eq!(frame.boxes.len(), small.nodes.len());
    assert!(frame.notes.is_empty() && frame.levels[0] == 0);
    eprintln!("L3 control: {} nodes, all drawn", frame.boxes.len());
    // Two levels past the cap together, each within it: the finer goes, the coarser stays.
    let mut notes = Vec::new();
    let two = block_octree(40, 40, 12); // 19,200 chunks and 2,400 at level 1
    let boxes = octree_boxes(&two, &mut notes);
    assert!(boxes.len() <= MAX_OVERLAY_NODES && boxes.iter().all(|b| b.level >= 1), "{notes:?}");
    // Before the world is read there is no octree: said, nothing drawn.
    let none = lod_frame(&LodView::Camera, WorldOverlay::Octree, None);
    assert!(none.boxes.is_empty() && none.notes.iter().any(|n| n.contains("world's read")));
}

#[test]
fn slow_the_read_gives_the_views_its_octree_and_cuts() {
    // The world's contribution carries what the views draw: the octree's every node, level by
    // level, and each tower's cut with each accepted node's bound, summing to its ledger. Control:
    // a body's contribution carries none.
    let spec = cav_sim::dev::world_box();
    let shots = cav_sim::read::Shots::together(&spec, vec![1.0]);
    let world =
        cav_sim::read::contribution(&spec, cav_sim::read::SourceId::World, &shots, &|| false).unwrap();
    let view = world.view.as_ref().expect("a world read carries its view");
    let octree = cav_sim::world::octree::Octree::build(
        &cav_sim::world::World::new(spec.world.as_ref().unwrap()),
        &|| false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(view.nodes.len(), octree.len());
    assert!(view.nodes.windows(2).all(|w| w[0].level <= w[1].level));
    let ledger = world.world.as_ref().unwrap();
    assert_eq!(view.towers.len(), spec.array.towers.len());
    for (cut, tower) in view.towers.iter().zip(&ledger.towers) {
        assert_eq!(
            (cut.accepted.len(), cut.inner.len(), cut.opened.len()),
            (tower.accepted, tower.blocks, tower.opened)
        );
        let sum: f64 = cut.accepted.iter().map(|(_, b)| b).chain(cut.inner.iter().map(|(_, b)| b)).sum();
        assert!((sum - cut.ledger).abs() <= 1e-12 * cut.ledger.max(1e-300), "{sum} against {}", cut.ledger);
    }
    let point = cav_sim::dev::point();
    let shots = cav_sim::read::Shots::together(&point, vec![1.0]);
    let body =
        cav_sim::read::contribution(&point, cav_sim::read::SourceId::Body(0), &shots, &|| false).unwrap();
    assert!(body.view.is_none());
}
