//! The world's octree held to its chunks' cubes, its carried bounds to the leaves' true absolute
//! moments, its bits to the thread count and its size to its caps (`docs/slices/S5.md` §3, O1–O3,
//! X3).

use super::*;
use crate::world::{LodSpec, MaterialSpec, PieceKind, PieceSpec, StreamSpec, WorldReadSpec, WorldSpec};
use cav_core::Vec3;
use cav_core::voxel::summary::{ABSOLUTE_ORDERS, cubes_absolute};

/// A small terrain world: 96 × 80 m of rolling ground at 0.45 m (chunks of 14.4 m) with a building
/// on it: about a hundred chunks and three levels. Its pitch and origin are not dyadic, so the
/// moments' sums and shifts round (on `world-box`'s lattice every product is exact in `f64`).
fn terrain_world() -> WorldSpec {
    WorldSpec {
        pitch: 0.45,
        supersample: 2,
        origin: [0.137, -0.291, 0.053],
        materials: vec![
            MaterialSpec::plain("ground", 1800.0, "#708054"),
            MaterialSpec::plain("concrete", 2400.0, "#b2b0a8"),
        ],
        pieces: vec![
            PieceSpec {
                label: "building".into(),
                material: 1,
                kind: PieceKind::Box { centre: [16.0, 8.5, 4.0], size: [12.0, 9.0, 10.0] },
            },
            PieceSpec {
                label: "ground".into(),
                material: 0,
                kind: PieceKind::Terrain {
                    lo: [-40.0, -40.0],
                    hi: [56.0, 40.0],
                    bottom: -3.0,
                    level: 0.0,
                    relief: 1.0,
                    wavelength: 24.0,
                    seed: 3,
                },
            },
        ],
        stream: StreamSpec::default(),
        read: WorldReadSpec::default(),
        lod: LodSpec::default(),
    }
}

/// The two worlds of O1–O3: `world-box` and the small terrain world.
fn worlds() -> Vec<(&'static str, World)> {
    let boxed = crate::dev::world_box().world.expect("world-box has a world");
    vec![("world-box", World::new(&boxed)), ("terrain", World::new(&terrain_world()))]
}

fn build(world: &World) -> Octree {
    Octree::build(world, &|| false).unwrap().unwrap()
}

/// Every chunk's cubes, by the chunk's node index.
fn leaves(tree: &Octree, world: &World) -> Vec<Vec<Cube>> {
    tree.level(0).map(|id| chunk_cubes(world, tree.chunk(id).unwrap()).unwrap()).collect()
}

/// The cubes of every chunk under node `id`.
fn under(tree: &Octree, id: usize, leaves: &[Vec<Cube>]) -> Vec<Cube> {
    match tree.chunk(id) {
        Some(_) => leaves[id].clone(),
        None => tree.children(id).iter().flat_map(|&c| under(tree, c as usize, leaves)).collect(),
    }
}

/// The worst difference of two summaries' raw moments, each order relative to `A_0 side^k`.
fn raw_error(a: &Summary, b: &Summary, side: f64) -> f64 {
    let scale = a.absolute[0].max(b.absolute[0]);
    let mut worst = (a.mass - b.mass).abs() / scale;
    let orders: [(&[f64], &[f64], i32); 3] =
        [(&a.first, &b.first, 1), (&a.second, &b.second, 2), (&a.third, &b.third, 3)];
    for (x, y, k) in orders {
        for (p, q) in x.iter().zip(y) {
            worst = worst.max((p - q).abs() / (scale * side.powi(k)));
        }
    }
    worst
}

/// The raw moments (mass to third, and `Σ|m|`) of `cubes` about `centre`, each component summed
/// with Neumaier's compensation: the reference O1 holds the octree to. Summed plainly, a node of
/// 10⁵ cubes is itself off by 2.9e-13 of `m side^k` (measured: §7.2), more than the octree.
fn direct_sum(centre: Vec3, cubes: &[Cube]) -> Summary {
    let parts: Vec<Summary> = cubes.iter().map(|&c| Summary::of_cubes(centre, [c])).collect();
    let field = |get: &dyn Fn(&Summary) -> f64| -> f64 {
        let (mut sum, mut carry) = (0.0f64, 0.0f64);
        for p in &parts {
            let x = get(p);
            let t = sum + x;
            carry += if sum.abs() >= x.abs() { (sum - t) + x } else { (x - t) + sum };
            sum = t;
        }
        sum + carry
    };
    let mut out = Summary::empty(centre);
    out.mass = field(&|p| p.mass);
    out.first = std::array::from_fn(|i| field(&|p| p.first[i]));
    out.second = std::array::from_fn(|i| field(&|p| p.second[i]));
    out.third = std::array::from_fn(|i| field(&|p| p.third[i]));
    out.absolute[0] = field(&|p| p.absolute[0]);
    out
}

/// The worst raw-moment error of any node of `tree` against the direct sum over its chunks'
/// cubes about the node's centre.
fn worst_raw(tree: &Octree, leaves: &[Vec<Cube>]) -> f64 {
    (0..tree.len())
        .map(|id| {
            let direct = direct_sum(tree.summary(id).centre, &under(tree, id, leaves));
            raw_error(tree.summary(id), &direct, tree.boxed(id).size().x)
        })
        .fold(0.0, f64::max)
}

cav_validate::case! {
    test: validation_the_octree_s_moments_are_its_chunks,
    name: "world.the_octree_s_moments_are_its_chunks",
    subsystem: "world",
    slice: "S5",
    reference: Exact,
    against: "every node's mass, first, second and third raw moments about its centre, carried up the octree, against the direct sum over its chunks' cubes about the same centre (world-box and a small terrain world), each order relative to the node's mass times its side to that power",
    unit: Relative,
    tolerance: 1e-13,
    measure: {
        let mut worst = 0.0f64;
        for (name, world) in worlds() {
            let tree = build(&world);
            let leaves = leaves(&tree, &world);
            if name == "terrain" {
                assert!(tree.levels() >= 3, "{:?}", tree.per_level());
            }
            // The node's centre is its box's, never its centre of mass.
            for id in 0..tree.len() {
                assert_eq!(tree.summary(id).centre, tree.boxed(id).centre());
            }
            worst = worst.max(worst_raw(&tree, &leaves));
            // Control: one chunk's third moments zeroed, carried up, are seen at once.
            let mut chunks: Vec<(ChunkKey, Summary)> =
                tree.level(0).map(|id| (tree.chunk(id).unwrap(), *tree.summary(id))).collect();
            chunks[0].1.third = [0.0; 10];
            let broken = Octree::from_chunks(world.grid, chunks).unwrap();
            assert!(worst_raw(&broken, &leaves) > 1e-6, "{name}: the control passed");
        }
        cav_validate::Measured::Error(worst)
    },
}

/// `x`'s median.
fn median(mut x: Vec<f64>) -> f64 {
    x.sort_by(f64::total_cmp);
    x[x.len() / 2]
}

#[test]
fn carried_absolute_moments_bound_the_leaves() {
    // At every node and every order, the carried A_l is no smaller than its leaves' true
    // Σ|m|s^l about the node's centre (even orders exact; odd ones by Cauchy–Schwarz, which is
    // never below the truth). Orders 2 and 4 of a chunk are the truth itself, so they are allowed
    // their rounding (1e-12).
    for (name, world) in worlds() {
        let tree = build(&world);
        let leaves = leaves(&tree, &world);
        let mut unshifted_fails = false;
        let mut ratios: Vec<Vec<[f64; 3]>> = vec![Vec::new(); tree.levels()];
        for (level, kept) in ratios.iter_mut().enumerate() {
            for id in tree.level(level) {
                let s = tree.summary(id);
                let truth = cubes_absolute(s.centre, under(&tree, id, &leaves));
                for l in 0..ABSOLUTE_ORDERS {
                    assert!(
                        s.absolute[l] >= truth[l] * (1.0 - 1e-12),
                        "{name}: node {:?} A_{l} carried {:e} below its leaves' {:e}",
                        tree.node(id).key,
                        s.absolute[l],
                        truth[l]
                    );
                }
                kept.push([4, 8, 16].map(|l| s.absolute[l] / truth[l]));
                // Control: children's A_l added without being moved to the parent's centre fall
                // below the truth somewhere.
                if level > 0 {
                    let summed = |l: usize| -> f64 {
                        tree.children(id).iter().map(|&c| tree.summary(c as usize).absolute[l]).sum()
                    };
                    unshifted_fails |= (1..ABSOLUTE_ORDERS).any(|l| summed(l) < truth[l]);
                }
            }
        }
        if tree.levels() > 1 {
            assert!(unshifted_fails, "{name}: the control passed");
        }
        for (level, r) in ratios.iter().enumerate() {
            let column = |i: usize| r.iter().map(|x| x[i]).collect::<Vec<_>>();
            let worst = |i: usize| column(i).into_iter().fold(0.0, f64::max);
            println!(
                "{name} level {level} ({} nodes): carried/true A_4 median {:.3} worst {:.3}; A_8 {:.3} / {:.3}; \
                 A_16 {:.3} / {:.3}",
                r.len(),
                median(column(0)),
                worst(0),
                median(column(1)),
                worst(1),
                median(column(2)),
                worst(2)
            );
        }
    }
}

/// Every bit of the octree: its keys, masks, children and every number of every summary.
fn bits(tree: &Octree) -> Vec<u64> {
    let mut out = Vec::new();
    for id in 0..tree.len() {
        let n = tree.node(id);
        let s = &n.summary;
        out.extend([u64::from(n.key.level), u64::from(n.mask), u64::from(n.first_child)]);
        out.extend(n.key.key.to_array().map(|k| u64::from(k as u32)));
        out.extend(s.centre.to_array().map(f64::to_bits));
        out.push(s.mass.to_bits());
        out.extend(s.first.iter().chain(&s.second).chain(&s.third).chain(&s.absolute).map(|v| v.to_bits()));
        out.push(s.reach.to_bits());
        out.extend(tree.children(id).iter().map(|&c| u64::from(c)));
    }
    out
}

#[test]
fn the_octree_is_the_same_on_any_thread_count() {
    let world = World::new(&terrain_world());
    let one = Octree::build_on(1, &world, &|| false).unwrap().unwrap();
    let reference = bits(&one);
    for threads in [2, 6] {
        let other = Octree::build_on(threads, &world, &|| false).unwrap().unwrap();
        assert_eq!(bits(&other), reference, "{threads} threads");
    }
    // Control: one bit of one number is seen.
    let mut nudged = one.clone();
    nudged.nodes[3].summary.third[4] = f64::from_bits(nudged.nodes[3].summary.third[4].to_bits() ^ 1);
    assert_ne!(bits(&nudged), reference);
    // A build cancelled part-way gives nothing, on any number of threads.
    for threads in [1, 6] {
        let asked = AtomicUsize::new(0);
        let cancel = || asked.fetch_add(1, Ordering::Relaxed) >= 20;
        assert_eq!(Octree::build_on(threads, &world, &cancel).unwrap(), None);
    }
}

#[test]
fn the_octree_s_shape_is_its_keys() {
    let world = World::new(&terrain_world());
    let tree = build(&world);
    assert!(tree.roots().len() <= MAX_ROOTS && !tree.roots().is_empty());
    let mut mass = 0.0;
    for id in tree.level(0) {
        mass += tree.summary(id).mass;
        assert!(tree.children(id).is_empty());
        assert_eq!(tree.find(tree.node(id).key), Some(id));
    }
    for level in 1..tree.levels() {
        for id in tree.level(level) {
            let key = tree.node(id).key;
            assert_eq!(tree.chunk(id), None);
            assert_eq!(tree.find(key), Some(id));
            // Children in octant order, each the child of this key in its octant, one level down.
            let octants: Vec<u8> =
                tree.children(id).iter().map(|&c| tree.node(c as usize).key.octant()).collect();
            assert!(octants.windows(2).all(|w| w[0] < w[1]) && !octants.is_empty());
            for &c in tree.children(id) {
                let child = tree.node(c as usize).key;
                assert_eq!(child.parent(), key);
                assert_eq!(key.child(child.octant()), child);
                assert!(tree.boxed(id).overlaps(&tree.boxed(c as usize)));
            }
            assert_eq!(tree.node(id).mask.count_ones() as usize, tree.children(id).len());
        }
    }
    // Every chunk is under exactly one root, and the roots hold all the mass.
    let roots_mass: f64 = tree.roots().map(|r| tree.summary(r).mass).sum();
    assert!((roots_mass - mass).abs() <= 1e-12 * mass, "{roots_mass} against {mass}");
    // Chunk keys agree with S4's ancestors at every level.
    let top = tree.levels() - 1;
    for id in tree.level(0) {
        let key = tree.chunk(id).unwrap();
        let root = NodeKey { level: top as u8, key: super::super::clipmap::ancestor(0, key, top as u8) };
        assert!(tree.find(root).is_some_and(|r| tree.roots().contains(&r)));
    }
    // An empty world is an empty octree.
    let mut empty = terrain_world();
    empty.pieces.clear();
    let none = build(&World::new(&empty));
    assert!(none.is_empty() && none.levels() == 0 && none.roots().is_empty());
}

#[test]
fn the_octree_is_bounded() {
    // A thin plate at the finest pitch has more chunks than a whole-world read takes: refused by
    // the search, naming the number, before any chunk is made.
    let mut plate = terrain_world();
    plate.pitch = 1.0e-3;
    plate.pieces = vec![PieceSpec {
        label: "plate".into(),
        material: 1,
        kind: PieceKind::Box { centre: [0.0, 0.0, 0.0], size: [20.0, 20.0, 0.1] },
    }];
    let made = AtomicUsize::new(0);
    let counting = || {
        made.fetch_add(1, Ordering::Relaxed);
        false
    };
    let why = Octree::build(&World::new(&plate), &counting).unwrap_err();
    assert!(why.contains(&MAX_READ_CHUNKS_WHOLE.to_string()), "{why}");
    // Only the search's levels asked: no chunk was made.
    assert!(made.load(Ordering::Relaxed) < 32);
    // Chunks spread so that none shares a parent make more nodes than the cap: refused before
    // anything is reserved, naming the number.
    let grid = World::new(&terrain_world()).grid;
    let spread: Vec<(ChunkKey, Summary)> = (0..MAX_WORLD_NODES as i32 / 4)
        .map(|i| {
            let key = ChunkKey::new(64 * i, 0, 0);
            let centre = grid.chunk_box(key).centre();
            (key, Summary::of_cubes(centre, [(centre, 1.0, 0.5)]))
        })
        .collect();
    let why = Octree::from_chunks(grid, spread).unwrap_err();
    assert!(why.contains(&MAX_WORLD_NODES.to_string()), "{why}");
    // A key given twice is refused.
    let key = ChunkKey::new(0, 0, 0);
    let one = Summary::of_cubes(Vec3::ZERO, [(Vec3::ZERO, 1.0, 0.5)]);
    assert!(Octree::from_chunks(grid, vec![(key, one), (key, one)]).is_err());
}

#[test]
#[ignore = "a measurement for docs/slices/S5.md §7.2: run in release with --ignored --nocapture"]
fn town_s_carried_bounds() {
    // The town's carried A_l against its leaves' true ones at every level, streamed chunk by
    // chunk: each chunk's exact even powers are added to every ancestor's, about its centre; the
    // odd orders by Cauchy–Schwarz on the totals.
    let spec = crate::dev::town();
    let world = World::new(spec.world.as_ref().expect("town has a world"));
    let tree = build(&world);
    let mut truth = vec![[0.0f64; ABSOLUTE_ORDERS]; tree.len()];
    let mut leaves = 0usize;
    for id in tree.level(0) {
        let key = tree.chunk(id).unwrap();
        let cubes = chunk_cubes(&world, key).unwrap();
        leaves += cubes.len();
        for level in 0..tree.levels() {
            let up =
                NodeKey { level: level as u8, key: super::super::clipmap::ancestor(0, key, level as u8) };
            let at = tree.find(up).unwrap();
            let even = cubes_absolute(tree.summary(at).centre, cubes.iter().copied());
            for l in (0..ABSOLUTE_ORDERS).step_by(2) {
                truth[at][l] += even[l];
            }
        }
    }
    let mut worst_low = f64::INFINITY;
    for level in 0..tree.levels() {
        let mut ratios: Vec<[f64; 3]> = Vec::new();
        for id in tree.level(level) {
            let t = &mut truth[id];
            for l in (1..ABSOLUTE_ORDERS).step_by(2) {
                t[l] = (t[l - 1] * t[l + 1]).sqrt();
            }
            let s = tree.summary(id);
            for l in 0..ABSOLUTE_ORDERS {
                worst_low = worst_low.min(s.absolute[l] / t[l]);
            }
            ratios.push([4, 8, 16].map(|l| s.absolute[l] / t[l]));
        }
        let column = |i: usize| ratios.iter().map(|x| x[i]).collect::<Vec<_>>();
        let worst = |i: usize| column(i).into_iter().fold(0.0, f64::max);
        println!(
            "town level {level} ({} nodes): carried/true A_4 median {:.3} worst {:.3}; A_8 {:.3} / {:.3}; A_16 {:.3} / {:.3}",
            ratios.len(),
            median(column(0)),
            worst(0),
            median(column(1)),
            worst(1),
            median(column(2)),
            worst(2)
        );
    }
    println!("town: {leaves} leaves; least carried/true over every node and order {worst_low:.6}");
    assert!(worst_low >= 1.0 - 1e-12);
}

#[test]
fn slow_the_town_s_octree() {
    // The town of the demo, whole: its chunks, nodes per level, build time and bytes, for
    // `docs/slices/S5.md` §7.2 (run it in release to measure). Its roots hold every chunk's mass.
    let spec = crate::dev::town();
    let world = World::new(spec.world.as_ref().expect("town has a world"));
    let start = std::time::Instant::now();
    let keys = chunk_keys(&world, &|| false).unwrap().unwrap();
    let searched = start.elapsed();
    let tree = build(&world);
    let built = start.elapsed() - searched;
    let mass: f64 = tree.level(0).map(|id| tree.summary(id).mass).sum();
    let roots: f64 = tree.roots().map(|r| tree.summary(r).mass).sum();
    assert!((roots - mass).abs() <= 1e-12 * mass);
    assert!(tree.len() <= MAX_WORLD_NODES && keys.len() <= MAX_READ_CHUNKS_WHOLE);
    println!(
        "town: {} chunk keys searched in {:.3} s; {} chunks with matter; nodes per level {:?} ({} in all); built in \
         {:.2} s on {} threads; {} bytes ({} a node); mass {mass:.6e} kg",
        keys.len(),
        searched.as_secs_f64(),
        tree.level(0).len(),
        tree.per_level(),
        tree.len(),
        built.as_secs_f64(),
        crate::read::threads(),
        tree.bytes(),
        Octree::NODE_BYTES,
    );
}
