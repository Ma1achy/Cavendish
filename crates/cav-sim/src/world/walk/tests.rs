//! The walk held to its partition, its ledger, its order and its caps (`docs/slices/S5.md` §3,
//! W1–W5, X4).

use super::*;
use crate::SceneSpec;
use crate::read::{self, SourceId};
use crate::world::inner::{ChunkTree, InnerKey};
use crate::world::tests::{aim, all_leaves, leaf_exact, scene, walked};
use crate::world::{
    Cube, LodSpec, MaterialSpec, PieceKind, PieceSpec, StreamSpec, World, WorldReadSpec, WorldSpec,
};
use cav_core::forward::Reading;

/// The small terrain world of the octree's tests: 96 × 80 m of rolling ground at 0.45 m (chunks
/// of 14.4 m) with a building, 84 chunks in three levels, its pitch and origin not dyadic.
pub(crate) fn terrain_world() -> WorldSpec {
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

/// A scene, its world and its octree.
pub(crate) struct Case {
    pub(crate) name: &'static str,
    pub(crate) spec: SceneSpec,
    pub(crate) world: World,
    pub(crate) tree: Octree,
    /// Tower bases to read from, metres.
    pub(crate) bases: Vec<Vec3>,
}

pub(crate) fn case(name: &'static str, spec: SceneSpec, bases: Vec<[f64; 3]>) -> Case {
    let world = World::new(spec.world.as_ref().expect("a world"));
    let tree = Octree::build(&world, &|| false).unwrap().unwrap();
    Case { name, spec, world, tree, bases: bases.into_iter().map(Vec3::from_array).collect() }
}

/// `world-box` (a box over four chunks, one level) at eight bases: in the open, beside a wall, on
/// the roof, under it, and from 40 to 200 m off, where the walk accepts its chunks.
pub(crate) fn world_box() -> Case {
    let bases = vec![
        [0.0, 0.0, 0.0],
        [5.5, 7.0, 0.0],
        [8.0, 7.0, 2.75],
        [8.0, 7.0, -12.0],
        [40.0, -20.0, 5.0],
        [80.0, 7.0, 0.0],
        [8.0, 120.0, 0.0],
        [-150.0, 0.0, 5.0],
    ];
    case("world-box", crate::dev::world_box(), bases)
}

/// The terrain world (84 chunks, three levels) at eight bases: on the ground in the open, beside
/// the building's wall, on its roof, and from 100 to 400 m off and above, where the walk accepts
/// nodes of every level (its chunks are 14.4 m: nearer, every bound is far above any tolerance
/// and everything is opened).
pub(crate) fn terrain() -> Case {
    let bases = vec![
        [0.0, 0.0, 1.2],
        [9.6, 8.5, 1.2],
        [16.0, 8.5, 9.5],
        [120.0, 0.0, 1.2],
        [200.0, 100.0, 1.2],
        [-150.0, -150.0, 1.2],
        [8.0, 0.0, 300.0],
        [400.0, 0.0, 0.0],
    ];
    case("terrain", scene(terrain_world()), bases)
}

/// How many cut entries cover each chunk of `tree` (by its index among the chunks), the mass the
/// cut holds, and its leaves.
fn coverage(tree: &Octree, accepted: &[NodeKey], opened: &[ChunkKey]) -> (Vec<usize>, f64, u64) {
    fn mark(tree: &Octree, id: usize, count: &mut [usize]) {
        if tree.chunk(id).is_some() {
            count[id] += 1;
        }
        for &child in tree.children(id) {
            mark(tree, child as usize, count);
        }
    }
    let mut count = vec![0usize; tree.level(0).len()];
    let (mut mass, mut leaves) = (0f64, 0u64);
    let ids = accepted.iter().copied().chain(opened.iter().map(|&k| NodeKey::chunk(k)));
    for key in ids {
        let id = tree.find(key).expect("a node of the tree");
        mark(tree, id, &mut count);
        mass += tree.summary(id).mass;
        leaves += tree.node(id).leaves;
    }
    (count, mass, leaves)
}

/// The octree's own mass and leaves: its roots'.
fn whole(tree: &Octree) -> (f64, u64) {
    tree.roots().fold((0.0, 0), |(m, l), id| (m + tree.summary(id).mass, l + tree.node(id).leaves))
}

/// Whether a cut covers every chunk once, its mass the tree's to 1e-12 and its leaves exactly.
fn is_partition(tree: &Octree, accepted: &[NodeKey], opened: &[ChunkKey]) -> bool {
    let (count, mass, leaves) = coverage(tree, accepted, opened);
    let (total, all) = whole(tree);
    count.iter().all(|&c| c == 1) && (mass - total).abs() <= 1e-12 * total.abs() && leaves == all
}

#[test]
fn the_cut_is_a_partition() {
    let mut parent = None;
    for case in [world_box(), terrain()] {
        for (b, &base) in case.bases.iter().enumerate() {
            for tolerance in [1e-1, 1e-4, 1e-8] {
                for bubble in [0.0, 5.0] {
                    let mut aim = aim(&case.spec, 0, base, tolerance, Rule::Octupole);
                    aim.bubble = bubble;
                    let cut = walk(&case.tree, &aim);
                    assert!(cut.settled, "{} base {b} at {tolerance}: {:?}", case.name, cut.stopped);
                    assert!(
                        is_partition(&case.tree, &cut.accepted, &cut.opened),
                        "{} base {b} at {tolerance}, bubble {bubble}",
                        case.name
                    );
                    assert!(cut.ledger <= tolerance);
                    assert_eq!(cut.per_level.iter().sum::<usize>(), cut.accepted.len());
                    // Every chunk within the bubble is opened.
                    for id in case.tree.level(0) {
                        if box_to_segment(&case.tree.boxed(id), base, aim.height) <= bubble {
                            assert!(
                                cut.opened.contains(&case.tree.chunk(id).unwrap()),
                                "{} base {b}",
                                case.name
                            );
                        }
                    }
                    if parent.is_none()
                        && let Some(&key) = cut.accepted.iter().find(|k| k.level > 0)
                    {
                        parent = Some((case.name, cut.clone(), key));
                    }
                }
            }
        }
    }
    // Control: a node and its child both kept double-counts a chunk and its mass.
    let (name, cut, key) = parent.expect("some cut accepts a node above the chunks");
    let case = if name == "terrain" { terrain() } else { world_box() };
    let id = case.tree.find(key).unwrap();
    let child = case.tree.node(case.tree.children(id)[0] as usize).key;
    let mut doubled = cut.accepted.clone();
    doubled.push(child);
    let (count, mass, _) = coverage(&case.tree, &doubled, &cut.opened);
    assert!(count.contains(&2));
    let total = whole(&case.tree).0;
    assert!((mass - total).abs() > 1e-12 * total.abs());
    assert!(!is_partition(&case.tree, &doubled, &cut.opened));
}

#[test]
fn the_cut_grows_as_the_tolerance_falls() {
    // The walk at a smaller tolerance is the same pops carried further, and a pop never makes the
    // cut smaller: its entries do not fall as the tolerance does, and its ledger stays within it.
    for case in [world_box(), terrain()] {
        for &base in &case.bases {
            let sizes: Vec<usize> = (1..=10)
                .map(|k| {
                    let tolerance = 10f64.powi(-k);
                    let cut = walk(&case.tree, &aim(&case.spec, 0, base, tolerance, Rule::Octupole));
                    assert!(cut.settled && cut.ledger <= tolerance);
                    cut.nodes()
                })
                .collect();
            assert!(sizes.windows(2).all(|w| w[0] <= w[1]), "{}: {sizes:?}", case.name);
        }
    }
    // Control: the tolerance changes the cut (the property is not true of a walk that ignores it).
    let case = terrain();
    let at = |tolerance| walk(&case.tree, &aim(&case.spec, 0, case.bases[4], tolerance, Rule::Octupole));
    let (loose, tight) = (at(1e-1), at(1e-10));
    assert!(
        loose.nodes() < tight.nodes(),
        "{} {:?} {} / {} {:?} {}",
        loose.nodes(),
        loose.per_level,
        loose.opened.len(),
        tight.nodes(),
        tight.per_level,
        tight.opened.len()
    );
}

#[test]
fn the_walk_is_bounded() {
    // Past a cap the walk stops where it is: not settled, its cut still a partition, its ledger
    // over the tolerance, and why said in words that name the cap.
    let case = terrain();
    // A base and a tolerance at which the walk both opens chunks and accepts nodes.
    let (base, tolerance, free) = case
        .bases
        .iter()
        .flat_map(|&base| (2..=9).map(move |k| (base, 10f64.powi(-k))))
        .map(|(base, tolerance)| {
            (base, tolerance, walk(&case.tree, &aim(&case.spec, 0, base, tolerance, Rule::Octupole)))
        })
        .find(|(_, _, cut)| cut.leaves > 1000 && cut.accepted.len() > 10)
        .expect("a walk that opens and accepts");
    assert!(free.settled && free.stopped.is_none());
    let mut work = aim(&case.spec, 0, base, tolerance, Rule::Octupole);
    work.max_work = free.leaves / 2;
    let mut nodes = aim(&case.spec, 0, base, tolerance, Rule::Octupole);
    nodes.max_nodes = free.accepted.len() / 2;
    for (capped, words) in [(work, "work"), (nodes, "nodes")] {
        let cut = walk(&case.tree, &capped);
        assert!(!cut.settled, "{words}");
        let why = cut.stopped.as_deref().unwrap();
        assert!(why.contains(words), "{why}");
        assert!(cut.ledger > tolerance, "{words}: {}", cut.ledger);
        assert!(is_partition(&case.tree, &cut.accepted, &cut.opened), "{words}");
        assert!(cut.accepted.len() <= capped.max_nodes);
        assert!(cut.leaves + 4 * cut.accepted.len() as u64 <= capped.max_work);
    }
    // The cap is never past MAX_CUT_NODES, whatever is asked.
    let mut more = aim(&case.spec, 0, base, tolerance, Rule::Octupole);
    more.max_nodes = usize::MAX;
    assert_eq!(walk(&case.tree, &more), free);
    // And the read says it: a walk stopped by the work cap is not settled, and its words say by
    // how much its ledger is over.
    let mut spec = case.spec.clone();
    spec.world.as_mut().unwrap().read.tolerance = 1e-12;
    spec.world.as_mut().unwrap().read.radius = 1e4;
    spec.bodies = (0..20_000)
        .map(|k| {
            let mut body = crate::BodySpec::point("weight", 1.0, [500.0 + f64::from(k), 0.0, 0.0]);
            body.shape = Some(crate::ShapeSpec::new(crate::ShapeKind::Cuboid { size: [0.1; 3] }, 0.1, 1));
            body
        })
        .collect();
    let shots = read::Shots::together(&spec, vec![0.0]);
    let c = read::contribution(&spec, SourceId::World, &shots, &|| false).unwrap();
    assert!(!c.settled, "{:?}", c.warnings);
    let said = c.warnings.iter().find(|w| w.text.contains("the walk stopped")).expect("said");
    assert!(said.text.contains("over world.read.tolerance"), "{said}");
    let world = c.world.unwrap();
    assert!(!world.settled() && world.towers.iter().all(|t| !t.settled));
}

/// The worst `error / ledger` of a walk's read against the leaf-exact one over both
/// interferometers and the gradiometer, each ledger with a rounding allowance of 1e-13 of the
/// phase it is of (the two reads add the same leaves in another order).
fn over(exact: Reading, got: Reading, ledger: [f64; 2]) -> f64 {
    let arm = |e: f64, g: f64, l: f64| (g - e).abs() / (l + 1e-13 * e.abs());
    let gradiometer = ((got.lower - got.upper) - (exact.lower - exact.upper)).abs()
        / (ledger[0] + ledger[1] + 1e-13 * (exact.lower.abs() + exact.upper.abs()));
    arm(exact.lower, got.lower, ledger[0]).max(arm(exact.upper, got.upper, ledger[1])).max(gradiometer)
}

#[test]
fn the_walk_needs_its_bound() {
    // W4: the walk is right only with its bound. With the octupole dropped and the gradient's
    // factor (l + 1) taken as 1, the read leaves out more than its ledger says, at some base and
    // tolerance; with the read's rule it never does, at the same ones. On world-box's bases from
    // 40 m off, where its chunks are accepted (the leaf-exact read of the terrain world takes
    // seconds; the town's levels are held by `validation_slow_the_walk_is_within_its_ledger`).
    let (mut broken, mut sound) = (0f64, 0f64);
    for case in [world_box()] {
        let leaves = all_leaves(&case.world, &case.tree);
        let readers = read::Readers::new(&case.spec);
        for &base in &case.bases[4..] {
            let exact = leaf_exact(readers.of(0), base, &leaves);
            for tolerance in [1e-3, 1e-6] {
                let (got, ledger, _) =
                    walked(&case.spec, &case.world, &case.tree, 0, base, tolerance, Rule::Octupole);
                sound = sound.max(over(exact, got, ledger));
                let (got, ledger, _) =
                    walked(&case.spec, &case.world, &case.tree, 0, base, tolerance, Rule::Broken);
                broken = broken.max(over(exact, got, ledger));
            }
        }
    }
    assert!(sound <= 1.0, "{sound}");
    assert!(broken > 1.0, "the broken rule stayed within its ledger: {broken}");
}

#[test]
fn a_node_too_near_is_never_accepted() {
    // Whatever the tolerance, a node whose reach is at least Q_MAX of its least distance to the
    // atoms' path is opened, never accepted (Salmon & Warren's footnote 3).
    let case = terrain();
    let mut near = 0;
    for &base in &case.bases {
        let probe = aim(&case.spec, 0, base, 1.0, Rule::Octupole);
        let cut = walk(&case.tree, &probe);
        for key in &cut.accepted {
            let id = case.tree.find(*key).unwrap();
            assert!(case.tree.summary(id).reach < Q_MAX * node_distance(&case.tree, id, &probe), "{key:?}");
        }
        near += (0..case.tree.len()).filter(|&id| node_bound(&case.tree, id, &probe).is_infinite()).count();
    }
    // Control: some nodes are that near some base, so the rule had work to do.
    assert!(near > 0);
}

/// W2 and W3 on `world-box` and `town`, measured once: for each, the worst over its bases and
/// the tolerances 1e-2, 1e-4, 1e-6 of the error over the ledger, of the ledger over the
/// tolerance, and of the ledger over the error where the error is above 1e-12 of the phase.
struct Ledgers {
    error_over_ledger: f64,
    ledger_over_tolerance: f64,
    ledger_over_error: f64,
}

/// The town's eight bases: the yard by the array, beside a wall, between two buildings, on a
/// roof, on the road, by a building's corner, at the world's edge and beyond it.
pub(crate) const TOWN_BASES: [[f64; 3]; 8] = [
    [0.0, 0.0, 0.0],
    [-59.5, -55.0, 0.0],
    [75.0, 10.0, 0.0],
    [42.0, -47.5, 10.5],
    [0.0, 55.5, 0.0],
    [-55.5, 19.5, 0.0],
    [95.0, 95.0, 0.0],
    [130.0, 0.0, 0.0],
];

/// The chunks under node `id`, pushed to `out`.
pub(crate) fn chunks_under(tree: &Octree, id: usize, out: &mut Vec<ChunkKey>) {
    match tree.chunk(id) {
        Some(key) => out.push(key),
        None => tree.children(id).iter().for_each(|&c| chunks_under(tree, c as usize, out)),
    }
}

/// W2 and W3, measured once a process: on `world-box` and `town` at eight bases each and the
/// tolerances 1e-2, 1e-4, 1e-6. The walk's read differs from the leaf-exact read of every chunk
/// only in its accepted nodes (the opened chunks are the same leaves in both), so the error is
/// formed there: the accepted nodes by their expansion against their own leaves, which is the
/// same difference without the rounding of two whole-world sums. Both are read with one coarse
/// Gauss rule (order 3 on 0.5 s panels) in place of the instrument's: the bound holds at every
/// instant, so it holds through any rule whose weights are positive, and the town's leaf-exact
/// reads then fit the slow tier. The phase W3's floor is taken of is the leaf-exact read of the
/// whole world through the same rule.
fn ledgers() -> &'static Ledgers {
    static FOUND: std::sync::OnceLock<Ledgers> = std::sync::OnceLock::new();
    FOUND.get_or_init(|| {
        let town = case("town", crate::dev::town(), TOWN_BASES.to_vec());
        measure_ledgers(vec![world_box(), town])
    })
}

/// The measurement behind [`ledgers`], over any `cases`.
fn measure_ledgers(cases: Vec<Case>) -> Ledgers {
    {
        use cav_core::forward::{Quadrature, Reader};
        let mut out = Ledgers { error_over_ledger: 0.0, ledger_over_tolerance: 0.0, ledger_over_error: 0.0 };
        println!(
            "world base tolerance: accepted per level, opened, ledger (rad), gradiometer error (rad), ledger/error"
        );
        for case in cases {
            let readers = read::Readers::new(&case.spec);
            let coarse = Quadrature::Gauss { order: 3, panel: 0.5 };
            let reader = Reader::new(cav_core::forward::Instrument {
                quadrature: coarse,
                ..*readers.of(0).instrument()
            });
            let keys: Vec<ChunkKey> = case.tree.level(0).map(|id| case.tree.chunk(id).unwrap()).collect();
            let made = crate::world::open(&case.world, &keys, &|| false).unwrap().unwrap();
            let all: Vec<Cube> = made.iter().flat_map(|t| t.cubes.iter().copied()).collect();
            let height = case.spec.array.instrument_of(0).tower_height;
            for (b, &base) in case.bases.iter().enumerate() {
                let whole = leaf_exact(&reader, base, &all);
                for tolerance in [1e-2, 1e-4, 1e-6] {
                    let probe = Aim {
                        base,
                        height,
                        per: crate::world::radians_per_kg_m(&reader, base, 0.0),
                        tolerance,
                        bubble: 0.0,
                        max_work: u64::MAX,
                        max_nodes: MAX_CUT_NODES,
                        rule: Rule::Octupole,
                    };
                    // The read's walk: into the trees of the chunks it opens (every chunk's is made).
                    let cut = walk_with(&case.tree, &probe, &made);
                    assert!(cut.settled, "{} base {b}: {:?}", case.name, cut.stopped);
                    let far = far_field(&case.tree, &made, &cut, Rule::Octupole);
                    let mut under = Vec::new();
                    for key in &cut.accepted {
                        chunks_under(&case.tree, case.tree.find(*key).unwrap(), &mut under);
                    }
                    let tree_of = |k: &ChunkKey| &made[made.binary_search_by_key(k, |t| t.key).unwrap()];
                    let mut leaves: Vec<Cube> =
                        under.iter().flat_map(|k| tree_of(k).cubes.iter().copied()).collect();
                    for key in &cut.inner {
                        let t = tree_of(&key.chunk);
                        let r = &t.node(t.find(*key).unwrap()).leaves;
                        leaves.extend_from_slice(&t.cubes[r.start as usize..r.end as usize]);
                    }
                    let exact = leaf_exact(&reader, base, &leaves);
                    let got = reader.read(base, &far, 0.0);
                    let bound =
                        reader.read(base, &LedgerSource { nodes: &far.nodes, rule: Rule::Octupole }, 0.0);
                    let ledger = [bound.lower.abs(), bound.upper.abs()];
                    // Each with an allowance of 1e-13 of the accepted part's phase for the order
                    // its leaves are added in.
                    let arm = |e: f64, g: f64, l: f64| (g - e).abs() / (l + 1e-13 * e.abs());
                    let gradiometer = ((got.lower - got.upper) - (exact.lower - exact.upper)).abs();
                    out.error_over_ledger = out
                        .error_over_ledger
                        .max(arm(exact.lower, got.lower, ledger[0]))
                        .max(arm(exact.upper, got.upper, ledger[1]))
                        .max(
                            gradiometer
                                / (ledger[0] + ledger[1] + 1e-13 * (exact.lower.abs() + exact.upper.abs())),
                        );
                    out.ledger_over_tolerance =
                        out.ledger_over_tolerance.max((ledger[0] + ledger[1]) / tolerance);
                    // W3, per read at its worst channel (the owner's ruling of 28 Nov 2026, option (a),
                    // after measuring: `docs/slices/S5.md` §7.3). The ledger bounds both
                    // interferometers, so its looseness is judged against the one it must cover
                    // hardest: the larger ledger over the larger error of the read. A ratio taken
                    // channel by channel is unbounded at any zero of a signed error (the yard at
                    // 1e-2, lower interferometer: 2,493, its ~2,600 node errors cancelling), and is
                    // still printed below.
                    let errors = [(got.lower - exact.lower).abs(), (got.upper - exact.upper).abs()];
                    let worst = errors[0].max(errors[1]);
                    if worst > 1e-12 * whole.lower.abs().max(whole.upper.abs()) {
                        out.ledger_over_error = out.ledger_over_error.max(ledger[0].max(ledger[1]) / worst);
                    }
                    println!(
                        "{} {b} {tolerance:.0e}: {:?}, {}, {:.2e}, {gradiometer:.2e}, {:.0}; arms {:.0} {:.0}",
                        case.name,
                        cut.per_level,
                        cut.opened.len(),
                        ledger[0] + ledger[1],
                        (ledger[0] + ledger[1]) / gradiometer,
                        ledger[0] / (got.lower - exact.lower).abs(),
                        ledger[1] / (got.upper - exact.upper).abs()
                    );
                }
            }
        }
        println!(
            "worst: error/ledger {:.3}, ledger/tolerance {:.3}, ledger/error {:.0}",
            out.error_over_ledger, out.ledger_over_tolerance, out.ledger_over_error
        );
        out
    }
}

#[test]
#[ignore = "a held-out check of W3 as restated, run once in release (S5 §7.3.1: its reads and expected outcome written before it was run)"]
fn held_out_w3() {
    let town = vec![
        [20.0, -20.0, 0.0],
        [-30.0, 40.0, 0.0],
        [60.0, 60.0, 0.0],
        [-100.0, -20.0, 0.0],
        [10.0, 90.0, 0.0],
    ];
    let boxed = vec![[3.0, -4.0, 0.0], [12.0, 2.0, 1.0], [25.0, 25.0, 0.0], [-60.0, 30.0, 0.0]];
    let ground = vec![[5.0, -6.0, 1.2], [30.0, 20.0, 1.2], [-60.0, 40.0, 1.2], [90.0, -80.0, 1.2]];
    let found = measure_ledgers(vec![
        case("town", crate::dev::town(), town),
        case("world-box", crate::dev::world_box(), boxed),
        case("terrain", scene(terrain_world()), ground),
    ]);
    // Reported whatever it is; the criteria as written before the run.
    println!(
        "held out: W2 error/ledger {:.3} (<= 1), ledger/tolerance {:.3} (<= 1); W3 ledger/error {:.0} (<= 1000)",
        found.error_over_ledger, found.ledger_over_tolerance, found.ledger_over_error
    );
    assert!(
        found.error_over_ledger <= 1.0
            && found.ledger_over_tolerance <= 1.0
            && found.ledger_over_error <= 1000.0
    );
}

cav_validate::case! {
    test: validation_slow_the_walk_is_within_its_ledger,
    name: "world.walk_within_its_ledger",
    subsystem: "world",
    slice: "S5",
    reference: Exact,
    against: "the whole world read by the walk (accepted nodes by their octupole, opened chunks leaf by leaf) against every chunk read leaf by leaf, on world-box and town at eight tower bases each and tolerances 1e-2, 1e-4, 1e-6 rad: the worst error over the reported ledger, each interferometer and the gradiometer (control: the_walk_needs_its_bound)",
    unit: Relative,
    tolerance: 1.0,
    measure: cav_validate::Measured::Error(ledgers().error_over_ledger),
}

cav_validate::case! {
    test: validation_slow_the_walk_s_ledger_is_within_its_tolerance,
    name: "world.walk_ledger_within_tolerance",
    subsystem: "world",
    slice: "S5",
    reference: Exact,
    against: "the same reads: the reported ledger (both interferometers' bounds read through the flight) over the tolerance asked, the worst",
    unit: Relative,
    tolerance: 1.0,
    measure: cav_validate::Measured::Error(ledgers().ledger_over_tolerance),
}

cav_validate::case! {
    test: validation_slow_the_walk_s_ledger_is_not_vacuous,
    name: "world.walk_ledger_is_not_vacuous",
    subsystem: "world",
    slice: "S5",
    reference: Exact,
    against: "the same reads: at each read its worst channel, the larger interferometer's ledger over the larger interferometer's error (the owner's ruling of 28 Nov 2026, restated after measuring: a channel's own ratio is unbounded at a zero of its signed error), the loosest where that error is above 1e-12 of the phase",
    unit: Relative,
    tolerance: 1000.0,
    measure: cav_validate::Measured::Error(ledgers().ledger_over_error),
}

#[test]
fn slow_the_whole_town_is_read() {
    // W5: the town, whole, through the forward model: not truncated, settled, and every chunk is
    // under the tower's cut (a partition). One tower, the array's centre: each tower reads the
    // whole world, and the town's five take minutes in a debug build (`docs/slices/S5.md` §7.3).
    let mut spec = crate::dev::town();
    let centre =
        spec.array.towers.iter().position(|t| t.position == [0.0, 0.0, 0.0]).expect("a centre tower");
    spec.array.towers = vec![spec.array.towers[centre].clone()];
    let shots = read::Shots::together(&spec, vec![0.0]);
    let c = read::contribution(&spec, SourceId::World, &shots, &|| false).unwrap();
    assert_eq!(c.truncated, None);
    assert!(c.settled, "{:?}", c.warnings);
    let world = c.world.expect("the town's ledger");
    assert!(world.settled() && world.worst() <= spec.world.as_ref().unwrap().read.tolerance);
    let w = World::new(spec.world.as_ref().unwrap());
    let tree = Octree::build(&w, &|| false).unwrap().unwrap();
    assert_eq!(tree.level(0).len(), 5350, "the town's chunks");
    for (n, tower) in spec.array.towers.iter().enumerate() {
        let base = Vec3::from_array(tower.position);
        // As `read_world` aims it: the walk takes what the local expansion leaves of the tolerance.
        let tolerance =
            (1.0 - crate::world::local::LOCAL_SHARE) * spec.world.as_ref().unwrap().read.tolerance;
        let mut probe = aim(&spec, n, base, tolerance, Rule::Octupole);
        probe.bubble = spec.world.as_ref().unwrap().read.radius;
        // The read's walk, into the trees of the chunks it opens.
        let cut = walk_into(&tree, &probe, &w, &mut Vec::new(), &|| false).unwrap().unwrap();
        assert!(cut.settled && cut.whole.is_empty(), "{}", tower.label);
        assert!(is_partition(&tree, &cut.accepted, &cut.opened), "{}", tower.label);
        assert_eq!(cut.accepted.len(), world.towers[n].accepted);
        assert_eq!(cut.opened.len(), world.towers[n].opened);
    }
    // Control: a town past the octree's caps is not read, and says so.
    let mut fine = spec.clone();
    fine.world.as_mut().unwrap().pitch = 0.01;
    let c = read::contribution(&fine, SourceId::World, &shots, &|| false).unwrap();
    assert_eq!(c.truncated, Some(0.0));
    assert!(!c.settled && c.world.is_none());
}

#[test]
#[ignore = "a measurement for docs/slices/S5.md §7.3: run in release, below normal priority"]
fn town_s_walk_measured() {
    // The town's five towers as the forward model reads them (its own bubble, the read's work
    // share), at 1e-2, 1e-4 and 1e-6 rad: nodes per level, chunks opened, the walk's time and
    // the read's (the chunks made, the audited read, the ledger), for tower 1; then the whole
    // contribution at the town's own settings.
    let spec = crate::dev::town();
    let world_spec = spec.world.as_ref().unwrap();
    let w = World::new(world_spec);
    let start = std::time::Instant::now();
    let tree = Octree::build(&w, &|| false).unwrap().unwrap();
    println!("octree: {:?} nodes per level, {:.2} s", tree.per_level(), start.elapsed().as_secs_f64());
    let readers = read::Readers::new(&spec);
    let share = crate::world::work_share(&spec);
    for tolerance in [1e-2, 1e-4, 1e-6] {
        // Tower 1 alone: each read of the town takes seconds to minutes.
        for (n, tower) in spec.array.towers.iter().enumerate().take(1) {
            let base = Vec3::from_array(tower.position);
            let reader = readers.of(n);
            // As `read_world` aims it: the walk takes what the local expansion leaves of τ.
            let mut probe =
                aim(&spec, n, base, (1.0 - crate::world::local::LOCAL_SHARE) * tolerance, Rule::Octupole);
            probe.bubble = world_spec.read.radius;
            probe.max_work = crate::world::max_work(reader, share);
            let chunked = walk(&tree, &probe).leaves;
            let start = std::time::Instant::now();
            let mut trees = Vec::new();
            let cut = walk_into(&tree, &probe, &w, &mut trees, &|| false).unwrap().unwrap();
            let walked = start.elapsed().as_secs_f64();
            let (per_level, inner, opened, leaves, settled) =
                (cut.per_level.clone(), cut.inner_per_level, cut.opened.len(), cut.leaves, cut.settled);
            let start = std::time::Instant::now();
            let read =
                crate::world::read_tower(&tree, &trees, reader, &probe, 0.0, tolerance, cut, share).unwrap();
            let reading = start.elapsed().as_secs_f64();
            println!(
                "{tolerance:.0e} {}: {per_level:?} accepted, blocks {inner:?} (levels 1 to 4), {opened} chunks opened, \
                 {leaves} leaves read (the chunk walk's {chunked}), settled {settled}; walk and chunks made {:.0} ms, \
                 read {:.0} ms ({} nodes, settled {}); ledger {:.2e}, estimate {:.2e}; local {:?}",
                tower.label,
                1e3 * walked,
                1e3 * reading,
                read.audited.nodes,
                read.audited.settled,
                read.ledger[0] + read.ledger[1],
                read.audited.estimate.unwrap_or(0.0),
                read.local
            );
        }
    }
    // What the bubble costs: tower 1 at 1e-2 with no bubble.
    let base = Vec3::from_array(spec.array.towers[0].position);
    let mut probe = aim(&spec, 0, base, (1.0 - crate::world::local::LOCAL_SHARE) * 1e-2, Rule::Octupole);
    probe.max_work = crate::world::max_work(readers.of(0), share);
    let mut trees = Vec::new();
    let cut = walk_into(&tree, &probe, &w, &mut trees, &|| false).unwrap().unwrap();
    let leaves = cut.leaves;
    let start = std::time::Instant::now();
    let read = crate::world::read_tower(&tree, &trees, readers.of(0), &probe, 0.0, 1e-2, cut, share).unwrap();
    println!(
        "1e-2 tower 1 with no bubble: {leaves} leaves read, read {:.0} ms ({} nodes, settled {})",
        1e3 * start.elapsed().as_secs_f64(),
        read.audited.nodes,
        read.audited.settled
    );
    let shots = read::Shots::together(&spec, vec![0.0]);
    let start = std::time::Instant::now();
    let c = read::contribution(&spec, SourceId::World, &shots, &|| false).unwrap();
    println!(
        "the town's world contribution, whole: {:.2} s, settled {}",
        start.elapsed().as_secs_f64(),
        c.settled
    );
}

#[test]
#[ignore = "a measurement for docs/slices/S5.md §7.9: run in release, below normal priority"]
fn town_s_blocks_tightened() {
    // The town's tower 1 walked into every chunk's tree, the blocks' summaries made four ways
    // (about their block's centre or their leaves' box's; far-corner or exact even absolute
    // moments): the leaves read one by one and the blocks accepted, at 1e-2, 1e-4 and 1e-6 rad.
    use crate::world::inner::Tightness;
    let spec = crate::dev::town();
    let world_spec = spec.world.as_ref().unwrap();
    let w = World::new(world_spec);
    let tree = Octree::build(&w, &|| false).unwrap().unwrap();
    let keys: Vec<ChunkKey> = tree.level(0).map(|id| tree.chunk(id).unwrap()).collect();
    let chunks: Vec<_> = keys.iter().map(|&k| w.chunk(k).unwrap().unwrap()).collect();
    let readers = read::Readers::new(&spec);
    let share = crate::world::work_share(&spec);
    let base = Vec3::from_array(spec.array.towers[0].position);
    for (centres, even) in [(false, false), (true, false), (false, true), (true, true)] {
        let tight = Tightness { centres, even };
        let trees: Vec<ChunkTree> = chunks.iter().map(|c| ChunkTree::of_with(&w, c, tight)).collect();
        for tolerance in [1e-2, 1e-4, 1e-6] {
            let mut probe =
                aim(&spec, 0, base, (1.0 - crate::world::local::LOCAL_SHARE) * tolerance, Rule::Octupole);
            probe.bubble = world_spec.read.radius;
            probe.max_work = crate::world::max_work(readers.of(0), share);
            let cut = walk_with(&tree, &probe, &trees);
            println!(
                "centres {centres}, even {even}, {tolerance:.0e}: {} leaves, blocks {:?}, chunks accepted {}, settled {}",
                cut.leaves,
                cut.inner_per_level,
                cut.accepted.len(),
                cut.settled
            );
        }
    }
}

/// How many cut entries cover each leaf of each opened chunk of `cut` (walked into `trees`), and
/// the mass the cut's blocks and runs hold.
fn leaf_coverage(trees: &[ChunkTree], cut: &Cut, extra: &[InnerKey]) -> (Vec<Vec<u8>>, f64) {
    let at = |k: &ChunkKey| trees.binary_search_by_key(k, |t| t.key).unwrap();
    let mut count: Vec<Vec<u8>> = cut.opened.iter().map(|k| vec![0u8; trees[at(k)].cubes.len()]).collect();
    let slot = |k: &ChunkKey| cut.opened.binary_search(k).unwrap();
    let mut mass = 0.0;
    for key in cut.inner.iter().chain(extra) {
        let t = &trees[at(&key.chunk)];
        let node = t.node(t.find(*key).unwrap());
        mass += node.summary.mass;
        for i in node.leaves.clone() {
            count[slot(&key.chunk)][i as usize] += 1;
        }
    }
    for (key, run) in &cut.runs {
        let t = &trees[at(key)];
        for i in run.clone() {
            mass += t.cubes[i as usize].1;
            count[slot(key)][i as usize] += 1;
        }
    }
    (count, mass)
}

#[test]
fn the_cut_into_the_chunks_trees_is_a_partition() {
    // W1 below the chunk: walked into every chunk's tree, every chunk not opened is under exactly
    // one accepted node, every leaf of every opened chunk under exactly one accepted block or run,
    // and the cut's mass is the world's to 1e-12.
    let mut doubled = None;
    for case in [world_box(), terrain()] {
        let keys: Vec<ChunkKey> = case.tree.level(0).map(|id| case.tree.chunk(id).unwrap()).collect();
        let trees = crate::world::open(&case.world, &keys, &|| false).unwrap().unwrap();
        let total = whole(&case.tree).0;
        for (b, &base) in case.bases.iter().enumerate() {
            for tolerance in [1e-1, 1e-4, 1e-8] {
                for bubble in [0.0, 5.0] {
                    let mut probe = aim(&case.spec, 0, base, tolerance, Rule::Octupole);
                    probe.bubble = bubble;
                    let cut = walk_with(&case.tree, &probe, &trees);
                    assert!(cut.settled && cut.whole.is_empty(), "{} base {b}: {:?}", case.name, cut.stopped);
                    assert!(cut.ledger <= tolerance);
                    // The chunks: each under one accepted node or opened, once.
                    let (count, chunk_mass, _) = coverage(&case.tree, &cut.accepted, &cut.opened);
                    assert!(count.iter().all(|&c| c == 1), "{} base {b}", case.name);
                    let opened_mass: f64 = cut
                        .opened
                        .iter()
                        .map(|k| case.tree.summary(case.tree.find(NodeKey::chunk(*k)).unwrap()).mass)
                        .sum();
                    let (leaves, mass) = leaf_coverage(&trees, &cut, &[]);
                    assert!(
                        leaves.iter().flatten().all(|&c| c == 1),
                        "{} base {b} at {tolerance}",
                        case.name
                    );
                    let cut_mass = chunk_mass - opened_mass + mass;
                    assert!((cut_mass - total).abs() <= 1e-12 * total.abs(), "{} base {b}", case.name);
                    let read: u64 = cut.runs.iter().map(|(_, r)| u64::from(r.end - r.start)).sum();
                    assert_eq!(read, cut.leaves);
                    assert_eq!(cut.inner_per_level.iter().sum::<usize>(), cut.inner.len());
                    if doubled.is_none()
                        && let Some(&key) = cut.inner.iter().find(|k| k.level < 4)
                    {
                        // A block's parent block, if it has one with a node.
                        let t = &trees[trees.binary_search_by_key(&key.chunk, |t| t.key).unwrap()];
                        let span = 1u32 << (3 * (u32::from(key.level) + 1));
                        let parent =
                            InnerKey { chunk: key.chunk, level: key.level + 1, key: key.key / span * span };
                        if t.find(parent).is_some() {
                            doubled = Some((case.name, trees.clone(), cut.clone(), parent));
                        }
                    }
                }
            }
        }
    }
    // Control: a block and its parent both kept covers the block's leaves twice.
    let (name, trees, cut, parent) = doubled.expect("some cut accepts a block whose parent is a node");
    let (leaves, _) = leaf_coverage(&trees, &cut, &[parent]);
    assert!(leaves.iter().flatten().any(|&c| c == 2), "{name}");
}
