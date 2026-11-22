//! A read's memory and work are bounded whatever the length of the flight (S1 review, important
//! finding 2: a valid scene with T = 700 s once made one reading allocate over 2 GB).
//!
//! Memory is measured with a counting allocator, not inferred from time. nextest runs each test
//! in a process of its own, so nothing else allocates while one measures.

use cav_core::Vec3;
use cav_core::forward::{Instrument, MAX_NODES, Mass, PHASE_TOLERANCE, Reader};
use cav_memtest::{Counting, peak_during};

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// AION-10 with a half-sequence of `t` seconds, and the floor on or off.
fn instrument(t: f64, floor: bool) -> Instrument {
    Instrument { half_sequence: t, floor, ..Instrument::aion10() }
}

#[test]
fn memory_per_read_is_flat_in_the_length_of_the_flight() {
    // A mass that moves, 5 m from the tower: a smooth source, read until it settles.
    let source = Mass::point(10.0, |t: f64| Vec3::new(5.0, 0.0, 1.0 + (0.3 * t).cos()));
    let mut peaks = Vec::new();
    for floor in [true, false] {
        for t in [0.73, 30.0, 1000.0] {
            let reader = Reader::new(instrument(t, floor));
            let (audited, peak) =
                peak_during(|| reader.read_to(Vec3::ZERO, &source, 3.0 * t, PHASE_TOLERANCE));
            assert!(audited.reading.lower.is_finite());
            eprintln!(
                "T = {t} s, floor {floor}: {} nodes, {} halvings, settled {}, {peak} bytes live at most",
                audited.nodes, audited.halvings, audited.settled
            );
            // With the floor off the work does grow with T; the memory must not.
            if !floor && t > 100.0 {
                assert!(audited.nodes > 100_000, "the long flight is a long sum: {} nodes", audited.nodes);
            }
            assert!(peak <= 64 * 1024, "T = {t} s, floor {floor}: {peak} bytes live at once");
            peaks.push(peak);
        }
    }
    let (least, most) = (*peaks.iter().min().unwrap(), *peaks.iter().max().unwrap());
    assert!(most <= 2 * least + 16 * 1024, "from {least} to {most} bytes: not flat in T");
    // Control: the allocator does see a table that grows with the flight. The same instrument's
    // nodes gathered into one, at T = 320 s with the floor off, are over a megabyte.
    let long = instrument(320.0, false);
    let (table, peak) = peak_during(|| long.flight(&[]));
    assert!(table.nodes.len() > 30_000);
    assert!(peak > 1024 * 1024, "the gathered table took {peak} bytes");
}

#[test]
fn the_largest_valid_flight_is_read_within_its_budget() {
    // The longest half-sequence a scene may have, the floor off so that the whole 2000 s is
    // integrated, and a point mass on the atoms' own path, whose integral never settles: the
    // most work a read can be asked for.
    let t = cav_sim::spec::limits::HALF_SEQUENCE_MAX;
    let reader = Reader::new(instrument(t, false));
    let on_path = Mass::point(1.0, |_| Vec3::new(0.0, 0.0, 1.0));
    let (audited, peak) = peak_during(|| reader.read_to(Vec3::ZERO, &on_path, 3.0 * t, PHASE_TOLERANCE));
    eprintln!(
        "T = {t} s, on the path: {} nodes, {} halvings, estimate {:?}, {peak} bytes",
        audited.nodes, audited.halvings, audited.estimate
    );
    // The budget is a count of nodes, known before each rule is run; it is not a time.
    assert!(audited.nodes <= MAX_NODES, "{} nodes", audited.nodes);
    assert!(audited.nodes > MAX_NODES / 4, "it used what it was allowed: {} nodes", audited.nodes);
    assert!(!audited.settled, "it did not settle, and says so");
    assert!(audited.estimate.is_some());
    assert!(peak <= 64 * 1024, "{peak} bytes live at once");
    // The same through a scene: the reading says it did not settle, with its estimate.
    let mut scene = cav_sim::dev::void();
    scene.array.towers.truncate(1);
    scene.array.instrument.half_sequence = t;
    scene.array.instrument.cycle = 3.0 * t;
    scene.array.instrument.launch_floor = false;
    scene.bodies.push(cav_sim::BodySpec::point("speck", 1.0, [0.0, 0.0, 1.0]));
    assert_eq!(scene.validate(), Ok(()));
    let reading = cav_sim::read::reading(&scene, 3.0 * t, &|| false).unwrap();
    assert!(reading.warnings.iter().any(|w| w.contains("did not settle")), "{:?}", reading.warnings);
    // One second more than the limit is not a scene.
    scene.array.instrument.half_sequence = t + 1.0;
    assert!(scene.validate().unwrap_err().0.starts_with("array.instrument.half_sequence"));
    // Control: an AION-10 read of a mass 5 m away costs a few hundred nodes and settles.
    let near = Reader::new(Instrument::aion10());
    let easy =
        near.read_to(Vec3::ZERO, &Mass::point(1.0, |_| Vec3::new(5.0, 0.0, 1.0)), 3.0, PHASE_TOLERANCE);
    assert!(easy.settled && easy.nodes < 1000, "{} nodes", easy.nodes);
}
