//! L1 and L2 (`docs/slices/S5.md` §3, §7.15): a tower's LOD view is its cut; the union of
//! towers is a minimal common refinement.

use std::collections::BTreeSet;

use cav_core::Vec3;
use cav_core::voxel::world::ChunkKey;

use super::super::octree::Octree;
use super::super::tests::aim;
use super::super::walk::tests::{Case, terrain, world_box};
use super::super::walk::{self, Rule};
use super::*;

/// Each tower's cut (as the read keeps it) at `bases` of `case`, at `tolerance`, walked into the
/// chunks' trees.
fn cuts(case: &Case, bases: &[Vec3], tolerance: f64) -> Vec<TowerCut> {
    let mut trees = Vec::new();
    bases
        .iter()
        .map(|&base| {
            let a = aim(&case.spec, 0, base, tolerance, Rule::Octupole);
            let cut = walk::walk_into(&case.tree, &a, &case.world, &mut trees, &|| false).unwrap().unwrap();
            TowerCut::of(&cut)
        })
        .collect()
}

/// The world's chunks (the octree's level 0).
fn chunks(tree: &Octree) -> Vec<ChunkKey> {
    tree.level(0).map(|id| tree.node(id).key.key).collect()
}

/// How many of `entries` hold each chunk.
fn cover(entries: &[Entry], chunks: &[ChunkKey]) -> Vec<usize> {
    let set: BTreeSet<Entry> = entries.iter().copied().collect();
    chunks
        .iter()
        .map(|&c| {
            (0..=super::super::octree::MAX_LEVEL).filter(|&l| set.contains(&ancestor((0, c), l))).count()
        })
        .collect()
}

/// Whether entry `a` lies inside entry `b` (or is it).
fn inside(a: Entry, b: Entry) -> bool {
    a.0 <= b.0 && ancestor(a, b.0) == b
}

fn read_of(case: &Case, towers: Vec<TowerCut>) -> ReadView {
    ReadView {
        grid: Some(*case.tree.grid()),
        nodes: (0..case.tree.levels())
            .flat_map(|l| case.tree.level(l))
            .map(|id| case.tree.node(id).key)
            .collect(),
        towers,
    }
}

#[test]
fn a_tower_s_lod_view_is_its_cut() {
    // L1 (criterion in §7.15, written before measuring): the draw list of `lod-view tower n` is
    // the keys of tower n's cut (its accepted nodes and the chunks it opened), no more and no
    // fewer, on world-box and the terrain at each of their eight bases; its blocks below the chunk
    // are the cut's blocks. Control: another tower's list is not it (where their cuts differ).
    let mut differ = 0;
    for case in [world_box(), terrain()] {
        let towers = cuts(&case, &case.bases, 1e-2);
        let read = read_of(&case, towers.clone());
        for (n, cut) in towers.iter().enumerate() {
            let frame = lod_frame(&LodView::Tower(n), WorldOverlay::Level, Some(&read));
            let list = frame.list.expect("a tower's view has a list");
            let mut keys: Vec<Entry> = cut.accepted.iter().map(|(k, _)| (k.level, k.key)).collect();
            keys.extend(cut.opened.iter().map(|k| (0, *k)));
            keys.sort_unstable();
            assert_eq!(list, keys, "{} tower {n}", case.name);
            // Its blocks below the chunk, outlined at their levels by `overlay level`.
            let blocks = frame.boxes.iter().filter(|b| b.level < 0).count();
            assert_eq!(blocks, cut.inner.len().min(MAX_OVERLAY_NODES), "{} tower {n}", case.name);
            if n > 0 {
                let other = lod_frame(&LodView::Tower(n - 1), WorldOverlay::None, Some(&read)).list.unwrap();
                differ += usize::from(other != list);
            }
        }
    }
    assert!(differ > 0, "the control: some neighbouring towers' views differ");
}

#[test]
fn the_union_of_towers_is_a_common_refinement() {
    // L2 (criterion in §7.15, written before measuring): for towers at several bases of the
    // terrain and of world-box, the view of all of them (`lod-view towers …`) is a partition of
    // the world's chunks (each under exactly one entry), finer than or equal to each tower's cut
    // (each entry inside one of every cut's entries), and minimal (every entry is in some cut);
    // its blocks stand where every tower that opened their chunk reads that block or coarser.
    // Control: the naive union of the cuts covers some chunk twice.
    let mut naive_double = 0;
    for case in [world_box(), terrain()] {
        let towers = cuts(&case, &case.bases, 1e-2);
        let read = read_of(&case, towers.clone());
        let all: Vec<usize> = (0..towers.len()).collect();
        let frame = lod_frame(&LodView::Towers(all.clone()), WorldOverlay::Tower, Some(&read));
        let list = frame.list.expect("a list");
        let chunks = chunks(&case.tree);
        assert!(cover(&list, &chunks).iter().all(|&c| c == 1), "{}: a partition", case.name);
        for cut in &towers {
            let own = entries(cut);
            assert!(
                list.iter().all(|&e| own.iter().any(|&o| inside(e, o))),
                "{}: finer than each cut",
                case.name
            );
        }
        assert!(list.iter().all(|e| towers.iter().any(|c| entries(c).contains(e))), "{}: minimal", case.name);
        let refs: Vec<&TowerCut> = towers.iter().collect();
        for (b, _) in union_inner(&refs) {
            for cut in &towers {
                if cut.opened.contains(&b.chunk) {
                    let holds = cut.inner.iter().any(|(k, _)| {
                        k.chunk == b.chunk
                            && k.level >= b.level
                            && (b.key >> (3 * u32::from(k.level))) == (k.key >> (3 * u32::from(k.level)))
                    });
                    assert!(holds, "{}: a block some tower reads finer stands", case.name);
                }
            }
        }
        // The control: every tower's entries together.
        let naive: Vec<Entry> =
            towers.iter().flat_map(entries).collect::<BTreeSet<_>>().into_iter().collect();
        naive_double += cover(&naive, &chunks).iter().filter(|&&c| c > 1).count();
        eprintln!(
            "L2 {}: {} towers, union {} entries ({} blocks), naive {} entries",
            case.name,
            towers.len(),
            list.len(),
            frame.boxes.iter().filter(|b| b.level < 0).count(),
            naive.len()
        );
    }
    assert!(naive_double > 0, "the naive union double-covers some chunk");
}

#[test]
fn a_draw_list_past_its_cap_is_merged_and_said() {
    // X6: a view's draw list holds at most MAX_DRAW_LIST entries; past it the finest level is
    // drawn by its parents (still a partition), said.
    let mut cut = TowerCut::default();
    for x in 0..130 {
        for y in 0..130 {
            cut.opened.push(ChunkKey::new(x, y, 0));
        }
    }
    let grid = cav_core::voxel::world::WorldGrid { origin: Vec3::ZERO, pitch: 0.1, supersample: 2 };
    let read = ReadView { grid: Some(grid), nodes: Vec::new(), towers: vec![cut.clone()] };
    let frame = lod_frame(&LodView::Tower(0), WorldOverlay::None, Some(&read));
    let list = frame.list.unwrap();
    assert!(list.len() <= MAX_DRAW_LIST && list.iter().all(|e| e.0 == 1), "{}", list.len());
    assert_eq!(cover(&list, &cut.opened).iter().filter(|&&c| c != 1).count(), 0);
    assert!(frame.notes.iter().any(|n| n.contains("16900 entries")), "{:?}", frame.notes);
    // Control: within the cap nothing is merged.
    cut.opened.truncate(MAX_DRAW_LIST);
    let read = ReadView { grid: Some(grid), nodes: Vec::new(), towers: vec![cut] };
    let frame = lod_frame(&LodView::Tower(0), WorldOverlay::None, Some(&read));
    assert!(frame.notes.is_empty() && frame.list.unwrap().len() == MAX_DRAW_LIST);
}
