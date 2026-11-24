//! Everything sized by a spec, a view or a command value has a bound, and the bound is measured
//! (`docs/slices/S2.md` §3.1). The class of fault: a value a person or an agent may legitimately
//! write decides how much memory something takes. S0 had a spec that hung the renderer; S1 a read
//! that allocated 2 GB, then a signal that could reserve 48 GB.
//!
//! Memory is measured with a counting allocator, not inferred. nextest runs each test in a
//! process of its own, so nothing else allocates while one measures; the engine's own worker
//! threads are counted, which is the point.

use std::time::Duration;

use cav_engine::{
    Command, Engine, FORWARD_STORE_BYTES, HISTORY_BYTES, JOURNAL_BYTES, Level, LogFilter, Query,
};
use cav_memtest::{Counting, live, peak_during};
use cav_sim::read::{self, MAX_SIGNAL_VALUES};
use cav_sim::{BodySpec, FloorKind, FloorSpec, SceneSpec, dev};
use serde_json::json;

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const MIB: usize = 1 << 20;
const LONG: Duration = Duration::from_secs(120);

/// The void with `towers` towers, a slab under them, and `shots` shots: a scene whose signal is
/// as large as asked and costs one read a tower, so that its memory can be measured in a moment.
fn slab_scene(towers: usize, shots: usize, density: f64) -> SceneSpec {
    let mut scene = dev::void();
    scene.array.towers =
        (0..towers).map(|n| cav_sim::TowerSpec::at(&format!("t{n}"), [n as f64, 0.0, 0.0])).collect();
    scene.array.instrument.half_sequence = 0.2;
    scene.array.instrument.cycle = 0.5;
    scene.duration = 0.5 * shots as f64;
    scene.floor = Some(FloorSpec { height: -1.0, kind: FloorKind::Slab { thickness: 1.0 }, density });
    assert_eq!(scene.validate(), Ok(()));
    scene
}

#[test]
fn a_scene_too_large_for_a_signal_is_refused_before_anything_is_allocated() {
    // The scene of the finding: a thousand towers and two million shots, 48 GB a source.
    let huge = slab_scene(1000, 2_000_000 - 1, 2400.0);
    let (refused, peak) = peak_during(|| read::readouts(&huge));
    let why = refused.unwrap_err();
    assert!(why.contains("1000 tower(s) × 1999999 shot(s)") && why.contains("4000000 values"), "{why}");
    assert!(peak < 64 << 10, "refusing took {peak} bytes");
    // Times that are the caller's own are counted too, before the towers multiply them.
    let wide = slab_scene(1000, 6, 2400.0);
    let (refused, peak) = peak_during(|| read::signal_at(&wide, vec![1.0; 5000]).map(|_| ()));
    assert!(refused.unwrap_err().contains("1000 tower(s) × 5000 shot(s)"));
    assert!(peak < 64 << 10, "refusing took {peak} bytes");
    // The engine says the same and keeps nothing.
    let mut engine = Engine::new();
    let (_, peak) = peak_during(|| engine.apply(Command::load(huge.clone())).unwrap());
    let asked = Query::Signal {
        tower: 1,
        channel: cav_engine::ChannelName::Gradiometer,
        span: None,
        compared: false,
    };
    assert!(engine.query(&asked).unwrap_err().contains("4000000 values"));
    assert_eq!(engine.forward_bytes(), 0);
    // (The scene itself, a thousand towers, is cloned a few times on the way in: megabytes.)
    assert!(peak < 16 * MIB, "loading the refused scene took {peak} bytes");
    // Control: the count is the bound and not a blanket refusal. At the cap itself the times are
    // made, and they are what the allocator sees: 2,000,000 shots, 16 MB.
    let at_cap = slab_scene(2, MAX_SIGNAL_VALUES / 2, 2400.0);
    let (times, peak) = peak_during(|| read::readouts(&at_cap).unwrap());
    assert_eq!(times.len() * 2, MAX_SIGNAL_VALUES);
    assert!(peak >= 16_000_000, "the times alone are 16 MB: {peak}");
    let over = slab_scene(3, MAX_SIGNAL_VALUES / 2, 2400.0);
    assert!(read::readouts(&over).is_err(), "one tower more is refused");
}

#[test]
fn the_forward_store_stays_within_its_budget() {
    // The largest signal there is, from one source: a contribution of 96 MiB, summed into a
    // signal of 96 MiB with its 16 MiB of times. Stated in the brief: under 0.6 GiB in all for
    // any scene; for this one, the two and their times.
    let largest = slab_scene(2, MAX_SIGNAL_VALUES / 2, 2400.0);
    let mut engine = Engine::new();
    let before = live();
    let (whole, peak) = peak_during(|| {
        engine.apply(Command::load(largest.clone())).unwrap();
        engine.settle(LONG)
    });
    assert!(whole, "the signal arrived");
    let signal = MAX_SIGNAL_VALUES * 24 + MAX_SIGNAL_VALUES / 2 * 8;
    eprintln!("the largest signal: peak {} MiB, kept {} MiB", peak / MIB, engine.forward_bytes() / MIB);
    assert!(engine.forward_bytes() <= FORWARD_STORE_BYTES);
    assert!(engine.forward_bytes() >= MAX_SIGNAL_VALUES * 24, "the contribution is kept, and counted");
    // The contribution, the signal, the times twice more (the plan's and the worker's), and a
    // few megabytes of scene.
    assert!(peak <= 2 * signal + 48 * MIB, "{} MiB at once", peak / MIB);
    assert!(live() - before <= 2 * signal + 16 * MIB, "{} MiB held", (live() - before) / MIB);

    // Earlier scenes' contributions are kept while there is room and dropped, oldest first, when
    // there is not: ten floors of 48 MiB each are 480 MiB made and never more than 256 MiB kept.
    let mut engine = Engine::new();
    let densities: Vec<f64> = (0..10).map(|n| 2000.0 + 100.0 * f64::from(n)).collect();
    engine.apply(Command::load(slab_scene(2, 1_000_000, densities[0]))).unwrap();
    assert!(engine.settle(LONG));
    let one = engine.forward_bytes();
    assert!((48_000_000..49 * MIB).contains(&one), "{one}");
    let (_, peak) = peak_during(|| {
        for density in &densities[1..] {
            engine.apply(Command::set("floor.density", json!(density))).unwrap();
            assert!(engine.settle(LONG));
            assert!(engine.forward_bytes() <= FORWARD_STORE_BYTES, "{} bytes kept", engine.forward_bytes());
        }
    });
    assert_eq!(engine.contributions_finished(), 10);
    assert_eq!(engine.forward_bytes(), (FORWARD_STORE_BYTES / one) * one, "as many as fit, and no more");
    // Kept, the old signal still shown and the new one being summed, and one in the worker's hands.
    assert!(peak <= FORWARD_STORE_BYTES + 4 * one, "{} MiB at once", peak / MIB);
    // Control: the store does keep. The floor before last is still here: nothing is computed.
    engine.apply(Command::set("floor.density", json!(densities[8]))).unwrap();
    engine.poll();
    assert_eq!(engine.signal().0, cav_engine::SignalState::Refined);
    assert_eq!(engine.contributions_finished(), 10);
    // And the first was dropped for room: going back to it is work again.
    engine.apply(Command::set("floor.density", json!(densities[0]))).unwrap();
    assert!(engine.settle(LONG));
    assert_eq!(engine.contributions_finished(), 11);

    // A scene whose own contributions would not fit is refused with the numbers, and nothing of
    // it is computed: a hundred bodies, five towers, a hundred thousand shots, 1.1 GiB.
    let mut many = dev::void();
    many.array.instrument.half_sequence = 0.002;
    many.array.instrument.cycle = 0.005;
    many.duration = 500.0;
    many.bodies =
        (0..100).map(|n| BodySpec::point(&format!("m{n}"), 1.0, [3.0, f64::from(n), 1.0])).collect();
    assert_eq!(many.validate(), Ok(()));
    let mut engine = Engine::new();
    let (_, peak) = peak_during(|| engine.apply(Command::load(many)).unwrap());
    let asked = Query::Signal {
        tower: 1,
        channel: cav_engine::ChannelName::Gradiometer,
        span: None,
        compared: false,
    };
    let why = engine.query(&asked).unwrap_err();
    assert!(why.contains("100 source(s) × 5 tower(s) × 100000 shot(s)") && why.contains("256 MiB"), "{why}");
    assert_eq!((engine.contributions_computed(), engine.forward_bytes()), (0, 0));
    assert!(peak < 8 * MIB, "{peak} bytes to refuse it");
}

/// A valid scene that is as heavy as a scene gets in text: a thousand bodies and a thousand
/// towers, every label as long as a label may be.
fn heavy_scene(tag: usize) -> SceneSpec {
    let mut scene = dev::void();
    let pad = "x".repeat(cav_sim::spec::limits::LABEL - 12);
    scene.array.towers =
        (0..1000).map(|n| cav_sim::TowerSpec::at(&format!("t{n:05}{pad}"), [0.0; 3])).collect();
    scene.bodies =
        (0..1000).map(|n| BodySpec::point(&format!("b{n:05}{pad}"), 1.0, [3.0, n as f64, 1.0])).collect();
    scene.name = format!("heavy-{tag}");
    assert_eq!(scene.validate(), Ok(()));
    scene
}

#[test]
fn the_undo_history_is_bounded_by_bytes() {
    // The mechanism is the engine's own; the size is a sixteenth of the real one, so that filling
    // it three times over is tens of megabytes of scenes and not five hundred.
    const BUDGET: usize = HISTORY_BYTES / 16;
    let mut engine = Engine::with_budgets(BUDGET, JOURNAL_BYTES);
    // Held: nothing is computed, so what grows here is the engine's own record and nothing else.
    engine.hold_workers(true);
    // The heaviest edit there is: a load, which keeps a whole scene each way.
    let scene = heavy_scene(0);
    let each = 2 * serde_json::to_vec(&scene).unwrap().len();
    assert!(each > 800_000, "a load of a heavy scene is {each} bytes of history");
    let loads = 3 * BUDGET / each;
    let before = live();
    let (_, peak) = peak_during(|| {
        for n in 0..loads {
            engine.apply(Command::load(heavy_scene(n))).unwrap();
        }
    });
    let (history, _, _) = engine.held();
    let Ok(cav_engine::Reply::Stats(stats)) = engine.query(&Query::Stats) else { panic!("stats") };
    eprintln!(
        "{loads} loads of {each} bytes: history {} KiB, {} steps kept; live +{} MiB, peak +{} MiB",
        history >> 10,
        stats.undo,
        (live() - before) / MIB,
        peak / MIB
    );
    assert!(history <= BUDGET, "{history} bytes of history");
    assert!(stats.undo < loads && stats.undo >= BUDGET / each - 1, "{} steps", stats.undo);
    // What the process holds is the history's count give or take the difference between a scene
    // in memory and its JSON (within a factor of two), and the journal, which keeps every load.
    let journal = engine.held().1;
    assert!(live() - before <= 2 * (BUDGET + journal) + 8 * MIB, "{} MiB held", (live() - before) / MIB);
    // The steps that are kept are the newest, and they undo.
    engine.apply(Command::Undo).unwrap();
    assert_eq!(engine.spec().name, format!("heavy-{}", loads - 2));
    // Control: without the budget these loads are three times it.
    assert!(loads * each >= 3 * BUDGET - each);
    // An ordinary edit is not a scene: a few dozen sets on the same heavy scene add a few hundred
    // bytes each to the history, however large the scene they edit, and all of them undo.
    let mut small = Engine::new();
    small.hold_workers(true);
    small.apply(Command::load(heavy_scene(0))).unwrap();
    let held = small.held().0;
    for n in 0..25 {
        small.apply(Command::set("bodies[7].mass", json!(2.0 + f64::from(n)))).unwrap();
    }
    let grew = small.held().0 - held;
    assert!(grew <= 25 * 400, "25 sets hold {grew} bytes of history");
    let Ok(cav_engine::Reply::Stats(stats)) = small.query(&Query::Stats) else { panic!("stats") };
    assert_eq!(stats.undo, 26);
}

#[test]
fn the_journal_is_bounded_and_says_when_it_is_no_longer_whole() {
    // As above: the engine's own mechanism at a sixteenth of the real sizes.
    const JOURNAL: usize = JOURNAL_BYTES / 16;
    const HISTORY: usize = HISTORY_BYTES / 16;
    let mut engine = Engine::with_budgets(HISTORY, JOURNAL);
    engine.hold_workers(true);
    let scene = heavy_scene(0);
    let each = serde_json::to_vec(&scene).unwrap().len();
    let loads = JOURNAL / each + 6;
    let before = live();
    let (_, peak) = peak_during(|| {
        for n in 0..loads {
            let mut next = scene.clone();
            next.name = format!("heavy-{n}");
            engine.apply(Command::load(next)).unwrap();
        }
    });
    let (history, journal, whole) = engine.held();
    eprintln!(
        "{loads} loads of {each} bytes: journal {} MiB in {} commands, whole {whole}; live +{} MiB, peak +{} MiB",
        journal / MIB,
        engine.journal().len(),
        (live() - before) / MIB,
        peak / MIB
    );
    assert!(!whole, "the journal stopped");
    assert!(journal <= JOURNAL && history <= HISTORY);
    assert!(engine.journal().len() < loads);
    // It says so: once in the log, in the stats, and by refusing to pass for a session.
    let said = engine.log().select(&LogFilter {
        min_level: Some(Level::Error),
        text: Some("records no further commands".into()),
        ..LogFilter::default()
    });
    assert_eq!(said.len(), 1, "{said:?}");
    let Ok(cav_engine::Reply::Stats(stats)) = engine.query(&Query::Stats) else { panic!("stats") };
    assert!(!stats.journal_whole);
    assert!(engine.query(&Query::Journal).unwrap_err().contains("not the whole session"));
    // The commands themselves were all applied: the engine is on the last scene loaded.
    assert_eq!(engine.spec().name, format!("heavy-{}", loads - 1));
    assert!(live() - before <= 2 * (JOURNAL + HISTORY) + 16 * MIB);
    // Control: a short session is whole, is offered, and replays; and at the real sizes the same
    // loads that overflowed here are all recorded.
    let mut roomy = Engine::new();
    roomy.hold_workers(true);
    for _ in 0..3 {
        roomy.apply(Command::load(scene.clone())).unwrap();
    }
    assert!(roomy.held().2 && roomy.journal().len() == 3);
    let mut short = Engine::new();
    short.apply(Command::load(dev::point())).unwrap();
    short.apply(Command::Seek(2.0)).unwrap();
    assert!(short.held().2);
    let Ok(cav_engine::Reply::Journal(journal)) = short.query(&Query::Journal) else { panic!("a journal") };
    assert_eq!(Engine::replay(&journal).unwrap().digest(), short.digest());
}

#[test]
fn a_long_message_does_not_grow_the_log() {
    let mut engine = Engine::new();
    let long = "a line of an answer\n".repeat(2_500_000);
    assert_eq!(long.len(), 50_000_000);
    let before = live();
    for _ in 0..40 {
        engine.note(Level::Info, cav_engine::Subsystem::App, long.as_str());
    }
    let grew = live() - before;
    assert!(grew <= 40 * (cav_engine::log::MAX_MESSAGE_BYTES + 256), "forty long messages hold {grew} bytes");
    let last = engine.log().iter().next_back().unwrap().message.clone();
    assert!(last.ends_with("of 50000000 bytes kept"), "{}", &last[last.len() - 80..]);
    assert!(last.starts_with("a line of an answer\n"));
    // A cut never splits a character.
    engine.note(Level::Info, cav_engine::Subsystem::App, "δφ₁ ".repeat(40_000));
    let cut = engine.log().iter().next_back().unwrap().message.clone();
    assert!(cut.contains("cut here") && cut.starts_with("δφ₁ δφ₁"));
    // Control: a message of ordinary length is kept whole, to the byte.
    engine.note(Level::Info, cav_engine::Subsystem::App, "a short message");
    assert_eq!(engine.log().iter().next_back().unwrap().message, "a short message");
    let at_cap = "y".repeat(cav_engine::log::MAX_MESSAGE_BYTES);
    engine.note(Level::Info, cav_engine::Subsystem::App, at_cap.as_str());
    assert_eq!(engine.log().iter().next_back().unwrap().message, at_cap);
}

#[test]
fn files_are_read_up_to_their_cap() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/file caps");
    std::fs::create_dir_all(&dir).unwrap();
    // Twenty megabytes at a scene's path: a path given by an agent may point at anything.
    let big = dir.join("not-a-scene.ron");
    std::fs::write(&big, vec![b'x'; 20 * MIB]).unwrap();
    let (refused, peak) = peak_during(|| cav_sim::load(big.to_str().unwrap()));
    let why = refused.unwrap_err().0;
    assert!(why.contains("larger than 1048576 bytes") && why.contains("not read"), "{why}");
    // The cap and one byte are read, and no more is ever held.
    assert!(peak <= MIB + (64 << 10), "{peak} bytes to refuse a 20 MiB file");
    // Control: the allocator sees a whole read of the same file.
    let (bytes, whole) = peak_during(|| std::fs::read(&big).unwrap());
    assert!(bytes.len() == 20 * MIB && whole >= 20 * MIB);
    drop(bytes);
    std::fs::remove_file(&big).unwrap();

    // Each of the four things read from outside (§3.1, A12), at its cap and one byte over. At
    // the cap the file is read (and is then found not to be what it should be: it is padding);
    // one byte over it is refused for its size, and no more than the cap and a byte is held.
    let path = |name: &str| dir.join(name).to_str().unwrap().to_owned();
    let padding = |bytes: u64| vec![b' '; bytes as usize];

    // A scene file: 1 MiB.
    let cap = cav_sim::library::MAX_SCENE_FILE_BYTES;
    std::fs::write(path("at the cap.ron"), padding(cap)).unwrap();
    std::fs::write(path("over the cap.ron"), padding(cap + 1)).unwrap();
    let (at, read) = peak_during(|| cav_sim::load(&path("at the cap.ron")));
    let (over, held) = peak_during(|| cav_sim::load(&path("over the cap.ron")));
    let (at, over) = (at.unwrap_err().0, over.unwrap_err().0);
    assert!(!at.contains("larger than") && read >= cap as usize, "{at}: {read} bytes");
    assert!(
        over.contains("larger than 1048576 bytes") && held <= cap as usize + (64 << 10),
        "{over}: {held}"
    );

    // A preset file: 64 KiB.
    let cap = cav_sim::presets::MAX_PRESET_FILE_BYTES;
    std::fs::write(path("at the cap.preset.ron"), padding(cap)).unwrap();
    std::fs::write(path("over the cap.preset.ron"), padding(cap + 1)).unwrap();
    let (at, read) = peak_during(|| cav_sim::presets::load(&path("at the cap.preset.ron")));
    let (over, held) = peak_during(|| cav_sim::presets::load(&path("over the cap.preset.ron")));
    let (at, over) = (at.unwrap_err().0, over.unwrap_err().0);
    assert!(at.contains("not an instrument") && read >= cap as usize, "{at}: {read} bytes");
    assert!(over.contains("larger than 65536 bytes") && held <= cap as usize + (16 << 10), "{over}: {held}");

    // An inline spec, as an agent sends one: 1 MiB of text. At the cap it is a scene (the void,
    // padded with spaces); a byte over, it is refused before anything is parsed.
    let cap = cav_sim::library::MAX_SCENE_FILE_BYTES as usize;
    let mut text = serde_json::to_string(&dev::void()).unwrap();
    text.push_str(&" ".repeat(cap - text.len()));
    let (at, parsed) = peak_during(|| cav_sim::inline(&text));
    assert_eq!(at.unwrap(), dev::void());
    text.push(' ');
    let (over, held) = peak_during(|| cav_sim::inline(&text));
    let over = over.unwrap_err().0;
    assert!(over.contains("larger than 1048576 bytes") && held <= 4096, "{over}: {held} bytes");
    assert!(parsed > held, "parsing a scene held {parsed} bytes, refusing one {held}");
    drop(text);

    // A journal file: 64 MiB, the size at which a journal stops recording. Over the cap its
    // length alone refuses it and nothing of it is read.
    let cap = cav_engine::journal::MAX_JOURNAL_FILE_BYTES;
    assert_eq!(cap, JOURNAL_BYTES as u64);
    let journal = path("a journal.json");
    std::fs::write(&journal, padding(cap)).unwrap();
    let (at, read) = peak_during(|| cav_engine::journal::read(&journal));
    let at = at.unwrap_err();
    assert!(at.contains("not a journal") && !at.contains("larger than"), "{at}");
    assert!(read >= cap as usize && read <= cap as usize + MIB, "{read} bytes to read a journal at its cap");
    std::fs::write(&journal, padding(cap + 1)).unwrap();
    let (over, held) = peak_during(|| cav_engine::journal::read(&journal));
    let over = over.unwrap_err();
    assert!(over.contains("larger than 67108864 bytes") && over.contains("not read"), "{over}");
    assert!(held <= 64 << 10, "{held} bytes to refuse a journal one byte over its cap");
    // The files are large: they do not stay.
    std::fs::remove_file(&journal).unwrap();
}

#[test]
fn a_periodogram_and_a_window_are_bounded_in_frequencies() {
    use cav_sim::schedule::{MAX_FREQUENCIES, window};
    // A window asked for at more frequencies than there are numbers: it is made at the cap. A
    // thousand shots at a million frequencies goes by the non-uniform FFT, whose grid is twice
    // the frequencies: 32 MiB of grid, the transform's own working copy, and the answer.
    let scene = dev::comb();
    let (answer, peak) = peak_during(|| window(&scene, 0, 1.0, usize::MAX).unwrap());
    assert_eq!(answer.0.len(), MAX_FREQUENCIES);
    eprintln!("a window at {MAX_FREQUENCIES} frequencies: {} MiB at once", peak / MIB);
    assert!(peak <= 192 * MIB, "{} MiB at once", peak / MIB);
    // Its work is bounded with it: the direct sum is used only while shots × frequencies is
    // small, so a long run at many frequencies is never a product of the two.
    let (short, small) = peak_during(|| window(&scene, 0, 1.0, 1000).unwrap());
    assert_eq!(short.0.len(), 1000);
    assert!(small < MIB, "a thousand frequencies took {small} bytes");
    // Control: the capped answer is the window itself, only finer. 1024 frequencies are every
    // 1025th of the capped set (1,048,575 = 1023 × 1025), so the two can be compared at every one
    // of them, not only at zero where a window is 1 by definition (S2's review, finding 8).
    let step = (MAX_FREQUENCIES - 1) / 1023;
    assert_eq!(step * 1023, MAX_FREQUENCIES - 1);
    let coarse = window(&scene, 0, 1.0, 1024).unwrap();
    let mut worst = 0.0f64;
    for (k, (frequency, height)) in coarse.0.iter().zip(&coarse.1).enumerate() {
        assert!((answer.0[k * step] - frequency).abs() < 1e-9, "frequency {k}");
        worst = worst.max((answer.1[k * step] - height).abs());
    }
    assert!(worst < 1e-9, "the capped window differs from the coarse one by {worst}");
    // And the comparison is of a window with lines in it, and can fail: a neighbouring
    // frequency of the fine set is another height somewhere.
    let spread =
        coarse.1.iter().fold(0.0f64, |m, h| m.max(*h)) - coarse.1.iter().fold(1.0f64, |m, h| m.min(*h));
    assert!(spread > 0.9, "the window has no line in it: {spread}");
    assert!(
        coarse
            .1
            .iter()
            .enumerate()
            .any(|(k, h)| (answer.1[(k * step + step / 2).min(MAX_FREQUENCIES - 1)] - h).abs() > 1e-3)
    );
}

#[test]
fn a_bundle_is_written_without_a_second_copy() {
    // A signal of two towers and a million shots over a slab: 48 MB of channels and 8 MB of
    // times. Writing its bundle goes value by value through a buffer: what is held while it is
    // written is the buffers and the manifest, not another copy of anything.
    let scene = slab_scene(2, 1_000_000, 2400.0);
    let signal = read::signal(&scene).unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/bundle memory");
    let _ = std::fs::remove_dir_all(&dir);
    let build = cav_sim::bundle::Build { version: "test".into(), revision: "none".into(), dirty: None };
    let (files, peak) = peak_during(|| cav_sim::bundle::write(&dir, &scene, &signal, &build, None).unwrap());
    assert_eq!(files.len(), 11);
    eprintln!("a bundle of 2 × 1,000,000: {} KiB held at once while writing", peak >> 10);
    assert!(peak <= 2 * MIB, "{peak} bytes at once to write a 74 MB bundle");
    let written: u64 = files.iter().map(|file| std::fs::metadata(file).unwrap().len()).sum();
    assert!(written > 70_000_000, "{written} bytes on disk");
    // Control: the allocator sees an array that is gathered before it is written.
    let (_, gathered) = peak_during(|| cav_core::npy::encode_f64(&[1_000_000, 2], &signal.gradiometer));
    assert!(gathered >= 16_000_000, "{gathered}");
    let _ = std::fs::remove_dir_all(&dir);
}
