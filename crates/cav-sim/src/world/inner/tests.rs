//! A chunk's tree held to its leaves (`docs/slices/S5.md` §7.9).

use super::*;
use crate::world::walk::tests::terrain;

/// Every leaf of `tree` under `parts` once each, in order, and the nodes' summaries' masses.
fn cover(tree: &ChunkTree, parts: &[Part], out: &mut Vec<u32>) {
    for part in parts {
        match part {
            Part::Leaves(r) => out.extend(r.clone()),
            Part::Node(n) => {
                let node = tree.node(*n);
                let mut under = Vec::new();
                cover(tree, tree.children(*n), &mut under);
                assert_eq!(under, node.leaves.clone().collect::<Vec<_>>(), "a node's parts are its leaves");
                out.extend(under);
            }
        }
    }
}

#[test]
fn a_chunk_s_tree_is_its_leaves() {
    // Every leaf under the chunk once, in Morton order; each node's summary is its leaves'
    // (mass to 1e-12), its leaves within its block, and every node has at least MIN_INNER_LEAVES.
    let case = terrain();
    let keys: Vec<ChunkKey> = case.tree.level(0).map(|id| case.tree.chunk(id).unwrap()).collect();
    let trees = crate::world::open(&case.world, &keys, &|| false).unwrap().unwrap();
    let mut nodes = 0;
    for tree in &trees {
        let mut all = Vec::new();
        cover(tree, tree.parts(), &mut all);
        assert_eq!(all, (0..tree.cubes.len() as u32).collect::<Vec<_>>(), "{:?}", tree.key);
        for n in 0..tree.len() as u32 {
            let node = tree.node(n);
            let cubes = &tree.cubes[node.leaves.start as usize..node.leaves.end as usize];
            assert!(cubes.len() >= MIN_INNER_LEAVES);
            let mass: f64 = cubes.iter().map(|c| c.1).sum();
            assert!((node.summary.mass - mass).abs() <= 1e-12 * mass.abs());
            for &(centre, _, side) in cubes {
                let inside =
                    |v: f64, lo: f64, hi: f64| v - 0.5 * side >= lo - 1e-9 && v + 0.5 * side <= hi + 1e-9;
                assert!(
                    inside(centre.x, node.boxed.lo.x, node.boxed.hi.x)
                        && inside(centre.y, node.boxed.lo.y, node.boxed.hi.y)
                        && inside(centre.z, node.boxed.lo.z, node.boxed.hi.z)
                );
            }
            assert_eq!(tree.find(node.key), Some(n));
            nodes += 1;
        }
    }
    assert!(nodes > 100, "the terrain's chunks have blocks: {nodes}");
    // Control: a tree that dropped a run would not cover its leaves.
    let tree = trees.iter().find(|t| t.parts().len() > 1).unwrap();
    let mut all = Vec::new();
    cover(tree, &tree.parts()[1..], &mut all);
    assert_ne!(all.len(), tree.cubes.len());
}
