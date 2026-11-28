//! The rings held to their table (`docs/slices/S5.md` §3, R1).

use super::*;
use crate::read;
use crate::world::tests::aim;
use crate::world::walk::Rule;
use crate::world::walk::tests::{Case, TOWN_BASES, case, terrain, world_box};
use cav_core::forward::{Quadrature, Reader};

/// Every chunk's tree of `case`, sorted.
fn trees_of(case: &Case) -> Vec<ChunkTree> {
    let keys: Vec<ChunkKey> = case.tree.level(0).map(|id| case.tree.chunk(id).unwrap()).collect();
    crate::world::open(&case.world, &keys, &|| false).unwrap().unwrap()
}

#[test]
fn the_rings_cut_is_a_partition() {
    // At radii measured on the case itself, the rings' cut covers every leaf once and its mass
    // is the world's; a node is accepted only at its radius or farther.
    for case in [world_box(), terrain()] {
        let trees = trees_of(&case);
        let aims: Vec<Aim> =
            case.bases.iter().map(|&b| aim(&case.spec, 0, b, 1e-4, Rule::Octupole)).collect();
        let radii = measured(&case.tree, &trees, &aims, 1e-4);
        assert!(radii.sides.iter().any(|r| r.is_finite()), "{}: some level is accepted", case.name);
        let total: f64 = case.tree.roots().map(|id| case.tree.summary(id).mass).sum();
        let chunk = case.tree.grid().chunk_side();
        for probe in &aims {
            let cut = rings_with(&case.tree, probe, &trees, &radii);
            let mut mass: f64 =
                cut.accepted.iter().map(|k| case.tree.summary(case.tree.find(*k).unwrap()).mass).sum();
            for key in &cut.inner {
                let t = &trees[trees.binary_search_by_key(&key.chunk, |t| t.key).unwrap()];
                mass += t.node(t.find(*key).unwrap()).summary.mass;
            }
            for (key, r) in &cut.runs {
                let t = &trees[trees.binary_search_by_key(key, |t| t.key).unwrap()];
                mass += t.cubes[r.start as usize..r.end as usize].iter().map(|c| c.1).sum::<f64>();
            }
            assert!((mass - total).abs() <= 1e-12 * total.abs(), "{}", case.name);
            for key in &cut.accepted {
                let s = case.tree.summary(case.tree.find(*key).unwrap());
                let level = octree_level(*key);
                assert!(
                    to_segment(s.centre, probe.base, probe.height) >= radii.of(level) * side(chunk, level)
                );
            }
        }
        // Control: with no radius anywhere nothing is accepted and every leaf is read.
        let none = Radii { tolerance: 1e-4, sides: [f64::INFINITY; LEVELS] };
        let cut = rings_with(&case.tree, &aims[0], &trees, &none);
        assert!(cut.accepted.is_empty() && cut.inner.is_empty());
        assert_eq!(cut.leaves, trees.iter().map(|t| t.cubes.len() as u64).sum::<u64>());
    }
}

/// R1: for each stored row, on the town and world-box at their eight bases, the worst error of
/// the rings' cut at the row's radii times `factor` (1 as stored, ½ for the control) over the row's
/// tolerance, through Gauss order 3 on 0.5 s panels (as W2). The two factors are measured in their
/// own tests, so that each fits the slow tier's watchdog.
fn r1(factor: f64) -> f64 {
    let town = case("town", crate::dev::town(), TOWN_BASES.to_vec());
    let mut worst = 0f64;
    println!("world base tolerance: error at {factor} of the stored radii (rad); ledger");
    for case in [world_box(), town] {
        let trees = trees_of(&case);
        let readers = read::Readers::new(&case.spec);
        let coarse = Quadrature::Gauss { order: 3, panel: 0.5 };
        let reader =
            Reader::new(cav_core::forward::Instrument { quadrature: coarse, ..*readers.of(0).instrument() });
        for (b, &base) in case.bases.iter().enumerate() {
            let mut probe = aim(&case.spec, 0, base, 1.0, Rule::Octupole);
            probe.per = crate::world::radians_per_kg_m(&reader, base, 0.0);
            for row in &RINGS {
                let cut = rings_with(&case.tree, &probe, &trees, &row.scaled(factor));
                let error = cut_error(&case.tree, &trees, &cut, &reader, base);
                worst = worst.max(error / row.tolerance);
                println!("{} {b} {:.0e}: {error:.2e}; {:.2e}", case.name, row.tolerance, cut.ledger);
            }
        }
    }
    println!("worst error/τ at {factor} of the radii: {worst:.3e}");
    worst
}

/// R1 at the stored radii, once a process.
fn stored() -> f64 {
    static FOUND: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *FOUND.get_or_init(|| r1(1.0))
}

#[test]
fn slow_the_rings_miss_their_tolerance_at_half_the_radii() {
    // Control: at half the stored radii the rings miss their tolerance somewhere.
    let halved = r1(0.5);
    assert!(halved > 1.0, "halved radii stayed within τ: {halved}");
}

cav_validate::case! {
    test: validation_slow_the_rings_reproduce_their_tolerance,
    name: "world.rings_reproduce_their_tolerance",
    subsystem: "world",
    slice: "S5",
    reference: Exact,
    against: "the rings tier at the stored radii (RINGS, measured on town's eight bases) against the leaf-exact read of the nodes it accepts, on world-box and town at eight bases each, for each stored tolerance (1e-2, 1e-4, 1e-6 rad): the worst error over the tolerance (control: the radii halved exceed it somewhere)",
    unit: Relative,
    tolerance: 1.0,
    measure: cav_validate::Measured::Error(stored()),
}

/// Whether `rows` come largest tolerance first and no level's radius falls as the tolerance does.
fn monotone(rows: &[Radii]) -> bool {
    rows.windows(2)
        .all(|p| p[0].tolerance > p[1].tolerance && p[0].sides.iter().zip(&p[1].sides).all(|(a, b)| b >= a))
}

#[test]
fn the_stored_radii_are_monotone_in_the_tolerance() {
    // R1: as the tolerance falls, no level's radius falls.
    assert!(monotone(&RINGS), "{RINGS:?}");
    // Control: the rows out of order are not.
    let mut wrong = RINGS;
    wrong.swap(0, 2);
    assert!(!monotone(&wrong));
    // A read finds its row: the largest tolerance at most its own.
    assert_eq!(row(5e-3).map(|r| r.tolerance), Some(1e-4));
    assert_eq!(row(1e-2).map(|r| r.tolerance), Some(1e-2));
    assert!(row(1e-7).is_none());
}

#[test]
#[ignore = "a measurement for docs/slices/S5.md §7.11: run in release, below normal priority"]
fn town_s_radii_measured() {
    // The stored table's source: the ledger walk's nearest acceptance per level at town's eight
    // bases (bubble 0, no work cap) at 1e-2, 1e-4, 1e-6 rad.
    let town = case("town", crate::dev::town(), TOWN_BASES.to_vec());
    let trees = trees_of(&town);
    let aims: Vec<Aim> = town.bases.iter().map(|&b| aim(&town.spec, 0, b, 1.0, Rule::Octupole)).collect();
    for tolerance in [1e-2, 1e-4, 1e-6] {
        let radii = measured(&town.tree, &trees, &aims, tolerance);
        println!("{tolerance:.0e}: {:?}", radii.sides);
    }
}

#[test]
fn a_world_read_by_rings_reports_its_ledger() {
    // `world.read.walk: Rings` reads the world through the stored table's row for its tolerance,
    // with each tower's ledger reported as the ledger walk's is; past the table it is refused in
    // words that say why.
    use crate::read::SourceId;
    let mut spec = crate::dev::world_box();
    let read = &mut spec.world.as_mut().unwrap().read;
    read.walk = crate::world::WalkKind::Rings;
    read.tolerance = 1e-4;
    let shots = read::Shots::together(&spec, vec![0.0]);
    let c = read::contribution(&spec, SourceId::World, &shots, &|| false).unwrap();
    let ledger = c.world.as_ref().expect("a ledger");
    assert!(ledger.settled() && c.truncated.is_none(), "{:?}", c.warnings);
    assert!(c.gradiometer.iter().any(|v| *v != 0.0));
    // Control: the same world by the ledger walk reads within the same tolerance of it.
    let mut by_ledger = spec.clone();
    by_ledger.world.as_mut().unwrap().read.walk = crate::world::WalkKind::Ledger;
    let l = read::contribution(&by_ledger, SourceId::World, &shots, &|| false).unwrap();
    for (a, b) in c.gradiometer.iter().zip(&l.gradiometer) {
        assert!((a - b).abs() <= 2e-4, "{a} {b}");
    }
    // Below the table's smallest tolerance the rings are refused, said.
    spec.world.as_mut().unwrap().read.tolerance = 1e-8;
    let c = read::contribution(&spec, SourceId::World, &shots, &|| false).unwrap();
    assert!(
        c.warnings.iter().any(|w| w.text.contains("Rings has radii for 1e-2, 1e-4 and 1e-6")),
        "{:?}",
        c.warnings
    );
    assert_eq!(c.truncated, Some(0.0));
}
