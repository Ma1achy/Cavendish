//! Editing by path, and the forward stage: the signal is computed off the caller's thread, kept
//! source by source, and never waited for by a command or a query.

use std::time::Duration;

use cav_engine::{ChannelName, Command, Engine, Level, LogFilter, Query, Reply, SignalState, Stage, Target};
use cav_sim::read;
use cav_sim::{SceneSpec, dev};
use serde_json::json;

const LONG: Duration = Duration::from_secs(60);

fn loaded(scene: SceneSpec) -> Engine {
    let mut engine = Engine::new();
    engine.apply(Command::load(scene)).unwrap();
    engine
}

fn set(engine: &mut Engine, path: &str, value: serde_json::Value) -> Result<(), String> {
    engine.apply(Command::set(path, value))
}

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|v| v.to_bits()).collect()
}

#[test]
fn set_edits_any_path_and_undo_restores_it() {
    let mut engine = loaded(dev::point());
    let before = engine.spec().clone();
    set(&mut engine, "bodies[0].mass", json!(250.0)).unwrap();
    set(&mut engine, "bodies[0].motion.Static.position", json!([1.0, -2.0, 0.5])).unwrap();
    set(&mut engine, "array.instrument.launch_floor", json!(false)).unwrap();
    set(&mut engine, "array.towers[4].position[1]", json!(-6.5)).unwrap();
    set(&mut engine, "name", json!("mine")).unwrap();
    let spec = engine.spec();
    assert_eq!((spec.bodies[0].mass, spec.name.as_str()), (250.0, "mine"));
    assert_eq!(spec.bodies[0].motion, cav_sim::Motion::Static { position: [1.0, -2.0, 0.5] });
    assert!(!spec.array.instrument.launch_floor);
    assert_eq!(spec.array.towers[4].position, [0.0, -6.5, 0.0]);
    // A whole part of the scene can be set at once: here, the bodies.
    set(&mut engine, "bodies", json!([])).unwrap();
    assert!(engine.spec().bodies.is_empty());
    // Each set is an edit: six undos give back the scene as it was, to the bit.
    for _ in 0..6 {
        engine.apply(Command::Undo).unwrap();
    }
    assert_eq!(engine.spec(), &before);
    assert_eq!(engine.apply(Command::Undo).map_err(|_| ()), Ok(()), "the load itself is still there to undo");
    // The session replays from its journal, sets and all.
    let replayed = Engine::replay(engine.journal()).unwrap();
    assert_eq!(replayed.digest(), engine.digest());
    // Control: a set that changes nothing visible still changes the state it says it changes.
    let mut other = loaded(dev::point());
    set(&mut other, "bodies[0].mass", json!(100.000_000_000_000_01)).unwrap();
    assert_ne!(other.digest(), loaded(dev::point()).digest());
}

#[test]
fn a_refused_set_changes_nothing() {
    let mut engine = loaded(dev::point());
    engine.apply(Command::Select(Some(Target::Body(1)))).unwrap();
    let refused = [
        ("bodies[0].mass", json!(-1.0), "bodies[0].mass"),
        ("bodies[0].mass", json!("heavy"), "cannot be set"),
        ("bodies[3].mass", json!(1.0), "has 1 items"),
        ("bodies[0].weight", json!(1.0), "has no field `weight`"),
        ("array.instrument.cycle", json!(1.0), "array.instrument.cycle"),
        ("duration", json!(null), "cannot be set"),
        ("array.towers", json!([]), "at least one tower"),
    ];
    for (path, value, expected) in refused {
        let before = (engine.digest(), engine.journal().len(), engine.generation());
        let why = set(&mut engine, path, value.clone()).unwrap_err();
        assert!(why.contains(expected), "set {path} {value}: `{why}` should mention `{expected}`");
        assert_eq!(before, (engine.digest(), engine.journal().len(), engine.generation()), "{path}");
        assert_eq!(engine.log().iter().next_back().unwrap().level, Level::Error);
    }
    // Control: the same paths take values that make a scene.
    set(&mut engine, "bodies[0].mass", json!(1.0)).unwrap();
    set(&mut engine, "array.instrument.cycle", json!(3.0)).unwrap();
    // A selection that the edit removes is dropped, not left pointing at nothing.
    set(&mut engine, "bodies", json!([])).unwrap();
    assert_eq!(engine.selection(), None);
}

#[test]
fn a_drag_is_one_journal_entry_and_replays() {
    let mut engine = loaded(dev::point());
    let before = engine.spec().clone();
    let journal_before = engine.journal().len();
    // A drag: its first step is an edit, and each later step says that it continues it.
    for step in 0..40 {
        let x = 2.0 + 0.05 * f64::from(step);
        let path = "bodies[0].motion.Static.position".to_owned();
        engine.apply(Command::Set { path, value: json!([x, 0.0, 2.5]), continues: step > 0 }).unwrap();
    }
    assert_eq!(engine.journal().len(), journal_before + 1, "forty steps of one drag are one entry");
    assert_eq!(engine.generation(), 41, "and forty changes");
    assert_eq!(Engine::replay(engine.journal()).unwrap().digest(), engine.digest());
    // One undo takes the whole drag back.
    engine.apply(Command::Undo).unwrap();
    assert_eq!(engine.spec(), &before);
    // Control: sets that are issued separately are separate edits, even of one path. (They once
    // merged: `set bodies[0].mass 20; set bodies[0].mass 30; undo` gave 100, not 20.)
    set(&mut engine, "bodies[0].mass", json!(20.0)).unwrap();
    set(&mut engine, "bodies[0].mass", json!(30.0)).unwrap();
    engine.apply(Command::Undo).unwrap();
    assert_eq!(engine.spec().bodies[0].mass, 20.0);
    engine.apply(Command::Undo).unwrap();
    assert_eq!(engine.spec().bodies[0].mass, 100.0);
    // And two masses added one after the other are two edits: undo takes back the second only.
    engine.run_script("mass 10 at 3 0 1; mass 5 at -3 0 1; undo").unwrap();
    assert_eq!(engine.spec().bodies.len(), 2);
    engine.run_line("undo").unwrap();
    assert_eq!(engine.spec(), &before);
    let journal_before = engine.journal().len() - 2;
    // Sets of different paths are different edits too.
    set(&mut engine, "bodies[0].mass", json!(5.0)).unwrap();
    set(&mut engine, "bodies[0].radius", json!(0.1)).unwrap();
    assert_eq!(engine.journal().len(), journal_before + 4, "the drag, the undo, and two sets");
    engine.apply(Command::Undo).unwrap();
    assert_eq!(engine.spec().bodies[0].mass, 5.0, "the second set alone was undone");
}

#[test]
fn the_signal_is_computed_on_the_worker_and_arrives() {
    let mut engine = loaded(dev::pass());
    assert!(engine.settle(LONG), "the signal arrives");
    let (state, signal) = engine.signal();
    assert_eq!(state, SignalState::Refined);
    let cold = read::signal(&dev::pass()).unwrap();
    assert_eq!(signal.unwrap().as_ref(), &cold, "what arrives is what a run here and now computes");
    // The query gives one tower's channel, and says it is the current scene's.
    let query = Query::Signal { tower: 2, channel: ChannelName::Lower, span: None };
    let Reply::Signal(reply) = engine.query(&query).unwrap() else { panic!("a signal") };
    assert_eq!((reply.state.as_str(), reply.tower), ("refined", 2));
    assert_eq!(bits(&reply.values), bits(cold.channel(1, cav_core::forward::Channel::Lower)));
    assert_eq!(reply.times, cold.times);
    assert!(reply.estimate <= 1e-10);
    // The log says a signal arrived.
    let said = engine
        .log()
        .select(&LogFilter { text: Some("signal: 5 tower(s) × 6 shot(s)".into()), ..LogFilter::default() });
    assert_eq!(said.len(), 1);
    // The void's signal is all zeros and is there at once: nothing to compute.
    let mut void = loaded(dev::void());
    void.poll();
    assert_eq!(void.signal().0, SignalState::Refined);
    // Control: a tower that is not there is refused, and a scene with too many shots says why.
    assert!(
        engine
            .query(&Query::Signal { tower: 6, channel: ChannelName::Gradiometer, span: None })
            .unwrap_err()
            .contains("no tower 6")
    );
    set(&mut engine, "array.instrument.half_sequence", json!(0.001)).unwrap();
    set(&mut engine, "array.instrument.cycle", json!(0.0021)).unwrap();
    set(&mut engine, "duration", json!(1.0e6)).unwrap();
    assert!(engine.query(&query).unwrap_err().contains("at most 2000000"));
    assert_eq!(engine.signal().0, SignalState::None);
}

#[test]
fn a_query_never_waits_for_the_worker() {
    // A scene with a whole signal and a whole reading: six shots of the pass.
    let mut engine = loaded(dev::pass());
    assert!(engine.settle(LONG));
    let query = Query::Signal { tower: 1, channel: ChannelName::Gradiometer, span: None };
    let finished = engine.contributions_finished();
    // The workers are held: from here on nothing can be computed anywhere but on this thread, and
    // a call that waited for a result would never return.
    engine.hold_workers(true);
    set(&mut engine, "bodies[0].mass", json!(20.0)).unwrap();
    engine.apply(Command::Seek(3.0)).unwrap();
    let Reply::Signal(signal) = engine.query(&query).unwrap() else { panic!("a signal") };
    let Reply::Reading(reading) = engine.query(&Query::Reading(None)).unwrap() else { panic!("a reading") };
    engine.query(&Query::Stats).unwrap();
    engine.poll();
    // Every call came back, and with what there was: the last whole signal and reading, marked
    // stale, and nothing new.
    assert_eq!((signal.state.as_str(), signal.times.len()), ("stale", 6));
    assert_eq!((reading.state.as_str(), reading.towers.len(), reading.time), ("stale", 5, 0.0));
    assert_eq!(engine.signal().0, SignalState::Stale);
    assert_eq!(engine.contributions_finished(), finished, "nothing was computed on this thread");
    // A load is another scene altogether: its predecessor's results are not shown under its name.
    engine.apply(Command::load(dev::superposition())).unwrap();
    let Reply::Signal(fresh) = engine.query(&query).unwrap() else { panic!("a signal") };
    let Reply::Reading(none) = engine.query(&Query::Reading(None)).unwrap() else { panic!("a reading") };
    assert_eq!((fresh.state.as_str(), fresh.times.len()), ("computing", 0));
    assert_eq!((none.state.as_str(), none.towers.len()), ("computing", 0));
    // Control: let the workers go and the same queries have the scene's own numbers.
    engine.hold_workers(false);
    assert!(engine.settle(LONG));
    let Reply::Signal(whole) = engine.query(&query).unwrap() else { panic!("a signal") };
    let Reply::Reading(read) = engine.query(&Query::Reading(None)).unwrap() else { panic!("a reading") };
    assert_eq!((whole.state.as_str(), whole.times.len()), ("refined", 20));
    assert_eq!((read.state.as_str(), read.towers.len(), read.time), ("refined", 5, 0.0));
}

#[test]
fn the_latest_edit_wins_and_stale_work_is_dropped() {
    let mut engine = loaded(dev::void());
    assert!(engine.settle(LONG));
    // Forty edits arrive while the workers are held, as a drag does between two of their jobs.
    engine.hold_workers(true);
    engine.apply(Command::load(dev::point())).unwrap();
    for step in 0..40 {
        let x = 2.0 + 0.05 * f64::from(step);
        set(&mut engine, "bodies[0].motion.Static.position", json!([x, 0.0, 2.5])).unwrap();
    }
    assert_eq!(engine.contributions_computed(), 41, "each edit asked for the body again");
    engine.hold_workers(false);
    assert!(engine.settle(LONG));
    // Only the last was computed: the forty before it were dropped unstarted.
    assert_eq!(engine.contributions_finished(), 1);
    let (state, signal) = engine.signal();
    assert_eq!(state, SignalState::Refined);
    assert_eq!(signal.unwrap().as_ref(), &read::signal(engine.spec()).unwrap(), "and it is the last scene's");
    // No signal of a superseded scene was ever announced.
    let arrivals = engine.log().select(&LogFilter { text: Some("signal:".into()), ..LogFilter::default() });
    assert_eq!(arrivals.len(), 1, "the last scene and no other: {arrivals:?}");
    // The reading too is the last one asked for.
    let Reply::Reading(reading) = engine.query(&Query::Reading(None)).unwrap() else { panic!("a reading") };
    let cold = read::reading(engine.spec(), 0.0, &|| false).unwrap();
    assert_eq!(reading.state, "refined");
    assert_eq!(reading.towers[1].gradiometer.to_bits(), cold.towers[1].gradiometer.to_bits());
    // (That a job already under way is abandoned between shots is `contribution`'s own test, in
    // cav-sim: `the_reading_is_the_series_at_a_readout_and_contributions_can_be_cancelled`.)
}

cav_validate::case! {
    test: validation_cached_sum_equals_cold_run,
    name: "engine.cached_sum_equals_cold_run",
    subsystem: "engine",
    slice: "S1",
    reference: Exact,
    against: "superposition after a drag of one body and back, summed from kept contributions, against a cold run of the same scene; values of any channel that differ, to the bit",
    unit: Count,
    tolerance: 0.0,
    measure: {
        let mut engine = loaded(dev::superposition());
        assert!(engine.settle(LONG));
        // Drag the second body away and to a new place; the first body's contribution is reused.
        for x in [-2.5, -3.0, -3.5] {
            set(&mut engine, "bodies[1].motion.Static.position", json!([x, 3.0, 4.0])).unwrap();
        }
        assert!(engine.settle(LONG));
        let cold = read::signal(engine.spec()).unwrap();
        let kept = engine.signal().1.unwrap();
        let differing = |a: &[f64], b: &[f64]| a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
        let count = differing(&kept.lower, &cold.lower)
            + differing(&kept.upper, &cold.upper)
            + differing(&kept.gradiometer, &cold.gradiometer);
        // Control: the drag did change the signal, so equality is not of two untouched things.
        assert_ne!(cold, read::signal(&dev::superposition()).unwrap());
        cav_validate::Measured::Error(count as f64)
    },
}

#[test]
fn dragging_one_body_recomputes_that_body_only() {
    let mut engine = loaded(dev::superposition());
    assert!(engine.settle(LONG));
    let after_load = engine.contributions_computed();
    assert_eq!(after_load, 2, "two bodies, two contributions");
    set(&mut engine, "bodies[1].motion.Static.position", json!([-2.5, 3.0, 4.0])).unwrap();
    assert!(engine.settle(LONG));
    assert_eq!(engine.contributions_computed(), after_load + 1, "the moved body, and not the other");
    // Back where it was: that contribution is still kept, and nothing is computed at all.
    set(&mut engine, "bodies[1].motion.Static.position", json!([-2.0, 3.0, 4.0])).unwrap();
    engine.poll();
    assert_eq!(engine.signal().0, SignalState::Refined, "whole at once, from what was kept");
    assert_eq!(engine.contributions_computed(), after_load + 1);
    // `run` drops what was kept and computes both again.
    engine.apply(Command::Run { from: Stage::Forward }).unwrap();
    assert!(engine.settle(LONG));
    assert_eq!(engine.contributions_computed(), after_load + 3);
    // Control: a change to the instrument is a change to every source.
    set(&mut engine, "array.instrument.launch_velocity", json!(3.9)).unwrap();
    assert!(engine.settle(LONG));
    assert_eq!(engine.contributions_computed(), after_load + 5);
}

#[test]
fn the_reading_follows_a_drag_and_the_console_speaks_of_it() {
    let mut engine = loaded(dev::point());
    engine.set_patience(LONG);
    let reading = |engine: &mut Engine| {
        assert!(engine.settle(LONG));
        let Reply::Reading(reply) = engine.query(&Query::Reading(None)).unwrap() else { panic!("readings") };
        assert_eq!(reply.state, "refined");
        reply
    };
    let before = reading(&mut engine).towers;
    assert_eq!(before.len(), 5);
    assert_eq!(before[0].label, "tower 1");
    assert_eq!(before[0].gradiometer, before[0].lower - before[0].upper, "lower minus upper");
    // A static mass reads the same at every readout: the live reading is the series' value.
    let series = engine.signal().1.unwrap().channel(0, cav_core::forward::Channel::Gradiometer).to_vec();
    assert_eq!(bits(&series), bits(&[before[0].gradiometer; 6]));
    // Drag the mass a metre nearer tower 2: tower 2 reads more.
    set(&mut engine, "bodies[0].motion.Static.position", json!([3.0, 0.0, 2.5])).unwrap();
    let after = reading(&mut engine);
    assert!(after.towers[1].gradiometer.abs() > 2.0 * before[1].gradiometer.abs());
    assert_ne!(after.towers[0].gradiometer, before[0].gradiometer);
    assert!(after.warnings.is_empty() && after.estimate <= 1e-10);
    // The reading carries what the audit found. On a tower's axis a point mass does not settle,
    // and the number beside the tower says so, as the signal does.
    set(&mut engine, "bodies[0].motion.Static.position", json!([0.0, 0.0, 1.0])).unwrap();
    let unsettled = reading(&mut engine);
    assert!(unsettled.estimate > 1e-7, "{:e}", unsettled.estimate);
    assert!(unsettled.warnings.iter().any(|w| w.contains("did not settle") && w.contains("tower 1")));
    let said = engine.run_line("reading 3").unwrap();
    assert!(said.contains("# warning:") && said.contains("did not settle"), "{said}");
    assert!(said.contains("a shot read out at 3 s · refined"), "{said}");
    set(&mut engine, "bodies[0].motion.Static.position", json!([3.0, 0.0, 2.5])).unwrap();
    // The console: the same things as text.
    let table = engine.run_line("reading 3").unwrap();
    assert!(table.lines().count() == 7 && table.contains("tower 5") && !table.contains("warning"), "{table}");
    // After a reading away from the playhead, the reading held is the playhead's again.
    assert!(engine.settle(LONG));
    let Reply::Reading(back) = engine.query(&Query::Reading(None)).unwrap() else { panic!("readings") };
    assert_eq!((back.state.as_str(), back.time), ("refined", 0.0));
    let signal = engine.run_line("signal 2").unwrap();
    assert!(signal.starts_with("# tower 2 · gradiometer · refined · 6 shots"), "{signal}");
    assert_eq!(signal.lines().count(), 7);
    assert!(signal.lines().nth(1).unwrap().starts_with("1.46  "), "{signal}");
    engine.run_line("mass 10 at 3 0 1").unwrap();
    engine.run_line("mass 5 at -3 0 1").unwrap();
    let labels: Vec<&str> = engine.spec().bodies.iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, ["mass", "mass 1", "mass 2"]);
    engine.run_line("set bodies[2].mass 7.5").unwrap();
    assert_eq!(engine.spec().bodies[2].mass, 7.5);
    engine.run_line("select body 3").unwrap();
    assert_eq!(engine.log().iter().next_back().unwrap().message, "select body 3");
    // Control: a channel that is not one is refused with the usage.
    assert!(engine.run_line("signal 2 middle").unwrap_err().contains("usage: signal"));
}

#[test]
fn a_body_through_the_tower_is_warned_of_in_the_log() {
    let mut engine = loaded(dev::through_the_tower());
    assert!(engine.settle(LONG));
    let warnings = engine.log().select(&LogFilter { min_level: Some(Level::Warn), ..LogFilter::default() });
    // For each of the three towers it crosses: that it is in the atoms' path, and that the shot's
    // read did not settle to the tolerance (the sphere's potential has a corner at its surface;
    // the reading is finite, and its estimate says how good it is).
    let count = |text: &str| warnings.iter().filter(|w| w.message.contains(text)).count();
    assert_eq!(warnings.len(), 6, "{warnings:?}");
    assert_eq!(count("`ball` passes through the atoms' path"), 3);
    assert_eq!(count("did not settle"), 3);
    // Control: the pass that keeps clear of every tower warns of nothing.
    let mut clear = loaded(dev::pass());
    assert!(clear.settle(LONG));
    assert!(
        clear.log().select(&LogFilter { min_level: Some(Level::Warn), ..LogFilter::default() }).is_empty()
    );
}

#[test]
fn the_new_commands_and_queries_round_trip_through_json() {
    let commands = [
        Command::set("bodies[0].motion.Static.position", json!([0.1, -2.0, 3.0])),
        Command::Set { path: "bodies[0].mass".into(), value: json!(2.5), continues: true },
        Command::Run { from: Stage::Forward },
        Command::Select(Some(Target::Body(2))),
        Command::load(dev::oscillator_zoo()),
    ];
    for command in &commands {
        let text = serde_json::to_string(command).unwrap();
        assert_eq!(&serde_json::from_str::<Command>(&text).unwrap(), command, "{text}");
    }
    let mut engine = loaded(dev::pass());
    assert!(engine.settle(LONG));
    for query in [
        Query::Signal { tower: 3, channel: ChannelName::Upper, span: None },
        Query::Reading(None),
        Query::Reading(Some(2.5)),
    ] {
        let text = serde_json::to_string(&query).unwrap();
        assert_eq!(serde_json::from_str::<Query>(&text).unwrap(), query, "{text}");
        let reply = engine.query(&query).unwrap();
        assert_eq!(serde_json::from_str::<Reply>(&serde_json::to_string(&reply).unwrap()).unwrap(), reply);
    }
    // Control: the wire form names the command.
    assert_eq!(serde_json::to_string(&commands[2]).unwrap(), r#"{"Run":{"from":"Forward"}}"#);
    // A journal written before `continues` existed still reads: an absent flag is an edit of its own.
    let old: Command = serde_json::from_str(r#"{"Set":{"path":"duration","value":20.0}}"#).unwrap();
    assert_eq!(old, Command::set("duration", json!(20.0)));
}

#[test]
fn a_pane_says_what_it_shows_and_which_stretch_of_it() {
    use cav_engine::{PaneContent, ViewCommand};
    let mut engine = loaded(dev::comb());
    engine.run_script("panes 2; show signal @2; span 100 300 @2").unwrap();
    // The frame carries what the pane shows, so nothing that draws a pane has to ask elsewhere.
    let frame = engine.frame(1).unwrap();
    assert_eq!((frame.content, frame.span), (PaneContent::Signal, Some([100.0, 300.0])));
    let scene = engine.frame(0).unwrap();
    assert_eq!((scene.content, scene.span), (PaneContent::Scene, None));
    // A span is a view command: it replays, it is not an edit of the scene, and it round-trips.
    let before = engine.spec().clone();
    let replayed = Engine::replay(engine.journal()).unwrap();
    assert_eq!(replayed.frame(1).unwrap().span, Some([100.0, 300.0]));
    assert_eq!(replayed.digest(), engine.digest());
    assert_eq!(engine.spec(), &before);
    let command = Command::View(ViewCommand::Span { pane: 1, span: Some([0.5, 2.0]) });
    let text = serde_json::to_string(&command).unwrap();
    assert_eq!(serde_json::from_str::<Command>(&text).unwrap(), command, "{text}");
    // Another plot has another axis, so the span does not follow the pane to it; `span all`
    // shows the whole axis.
    engine.run_line("show periodogram @2").unwrap();
    assert_eq!(engine.frame(1).unwrap().span, None);
    engine.run_script("span 0.1 0.2 @2; span all @2").unwrap();
    assert_eq!(engine.frame(1).unwrap().span, None);
    // Control: a span that runs backwards, is empty or is not a number is refused and nothing
    // changes; so is one for a pane that is not there.
    engine.run_line("span 0.1 0.2 @2").unwrap();
    for line in ["span 3 1 @2", "span 2 2 @2", "span nan 4 @2", "span 0 1 @5"] {
        assert!(engine.run_line(line).is_err(), "`{line}` was accepted");
        assert_eq!(engine.frame(1).unwrap().span, Some([0.1, 0.2]));
    }
}

#[test]
fn a_value_with_strings_in_it_can_be_set_from_the_console() {
    // An agent adds an oscillator with two lines: a mass, then its motion as JSON. The console's
    // quotes used to take the JSON's, so anything but a number or a list could only be loaded.
    let mut engine = loaded(dev::void());
    engine.run_line("mass 1 at 3 0 1").unwrap();
    let motion =
        r#"{"Linear": {"centre": [3, 0, 1], "amplitude": [0, 0, 0.5], "frequency": 0.05, "phase": 0}}"#;
    engine.run_line(&format!("set bodies[0].motion {motion}")).unwrap();
    assert_eq!(engine.spec().bodies[0].motion, dev::comb().bodies[0].motion);
    engine.run_line(r#"set bodies[0].label "the oscillator; number one""#).unwrap();
    assert_eq!(engine.spec().bodies[0].label, "the oscillator; number one");
    // In a script too: a `;` inside the JSON's quotes does not end the line.
    engine.run_script(r#"set name "two; words"; seek 2"#).unwrap();
    assert_eq!((engine.spec().name.as_str(), engine.time()), ("two; words", 2.0));
    // A bare word is still a string, and a number a number.
    engine.run_line("set name comb").unwrap();
    engine.run_line("set duration 30").unwrap();
    assert_eq!((engine.spec().name.as_str(), engine.spec().duration), ("comb", 30.0));
    // The log's line for a set issues it again.
    let line = engine.log().iter().rev().find(|e| e.message.starts_with("set bodies[0].label")).unwrap();
    let mut again = loaded(dev::void());
    again.run_line("mass 1 at 3 0 1").unwrap();
    again.run_line(&line.message).unwrap();
    assert_eq!(again.spec().bodies[0].label, "the oscillator; number one");
    // Control: JSON that is not a motion is refused, and says what was wrong.
    let why = engine.run_line(r#"set bodies[0].motion {"Sideways": {}}"#).unwrap_err();
    assert!(why.contains("cannot be set"), "{why}");
    assert!(engine.run_line("set duration").unwrap_err().contains("usage: set"));
}

#[test]
fn a_long_answer_is_cut_and_says_so() {
    // Twelve thousand shots over a slab: one read a tower, so the signal is there at once.
    let mut scene = dev::infinite_floor();
    scene.array.towers.truncate(1);
    scene.array.instrument.half_sequence = 0.2;
    scene.array.instrument.cycle = 0.5;
    scene.duration = 6000.0;
    let mut engine = loaded(scene);
    engine.set_patience(LONG);
    let all = engine.run_line("signal 1").unwrap();
    let lines: Vec<&str> = all.lines().collect();
    assert!(lines[0].contains("12000 shots"), "{}", lines[0]);
    let printed = lines.iter().filter(|line| !line.starts_with('#')).count();
    assert_eq!(printed, cav_engine::console::MAX_LINES);
    assert!(lines.last().unwrap().contains("2000 more shots are not printed"), "{}", lines.last().unwrap());
    // A stretch of the timeline is asked for by its ends, and is those shots: launched every
    // 0.5 s and read out 0.4 s later, so 100.4 to 102.4 s is five of them.
    let stretch = engine.run_line("signal 1 gradiometer 100.4 102.4").unwrap();
    let shots: Vec<&str> = stretch.lines().filter(|line| !line.starts_with('#')).collect();
    assert_eq!(shots.len(), 5, "{stretch}");
    assert!(shots[0].starts_with("100.4 ") && shots[4].starts_with("102.4 "), "{stretch}");
    let query = Query::Signal { tower: 1, channel: ChannelName::Lower, span: Some([0.0, 1.0]) };
    let Reply::Signal(reply) = engine.query(&query).unwrap() else { panic!("a signal") };
    assert_eq!(reply.times, [0.4, 0.9]);
    // Control: a short answer is whole and says nothing of a cut; a span that runs backwards is refused.
    let short = engine.run_line("signal 1 0 50").unwrap();
    assert!(
        !short.contains("not printed") && short.lines().count() == 1 + 1 + 100,
        "{}",
        short.lines().count()
    );
    assert!(engine.run_line("signal 1 5 2").unwrap_err().contains("from one time up to a later one"));
    // Two numbers are a tower and then nothing else: `signal 1 2` is not a span.
    assert!(engine.run_line("signal 1 2").is_err() || engine.spec().array.towers.len() >= 2);
}

#[test]
fn a_reading_does_not_wait_for_the_series() {
    // A series that takes minutes: 200,000 shots of a pass, five towers.
    let mut scene = dev::pass();
    scene.array.instrument.half_sequence = 0.002;
    scene.array.instrument.cycle = 0.005;
    scene.duration = 1000.0;
    let mut engine = loaded(scene);
    engine.set_patience(LONG);
    // The reading is asked for with a minute's patience and is back long before the series is:
    // it waited for the reading worker and for nothing else. (It once waited the whole minute.)
    let started = std::time::Instant::now();
    let said = engine.run_line("reading").unwrap();
    assert!(said.contains("· refined ·"), "{said}");
    let away = engine.run_line("reading 2.5").unwrap();
    assert!(away.contains("a shot read out at 2.5 s · refined"), "{away}");
    assert!(started.elapsed() < Duration::from_secs(20), "two readings took {:?}", started.elapsed());
    assert_eq!(engine.signal().0, SignalState::Stale, "the series is still under way");
    // The typed form of the same: `ask` waits for what the query needs and asks the worker for a
    // time that is not the playhead's, which `query` alone never does.
    let Reply::Reading(direct) = engine.query(&Query::Reading(Some(7.5))).unwrap() else { panic!() };
    assert_ne!(direct.state, "refined", "nobody had asked for 7.5 s");
    let Reply::Reading(asked) = engine.ask(&Query::Reading(Some(7.5))).unwrap() else { panic!() };
    assert_eq!((asked.state.as_str(), asked.time), ("refined", 7.5));
    // Control: a signal is what does wait for the series; with no patience it says where it stands.
    engine.set_patience(Duration::ZERO);
    let signal = engine.run_line("signal 1 0 1").unwrap_or_else(|why| why);
    assert!(signal.contains("computing") || signal.contains("stale"), "{signal}");
}

#[test]
fn an_unsettled_reading_says_so() {
    // The words: an estimate is "within" only when the read settled.
    assert_eq!(cav_engine::estimate_text(0.0, true), "exact");
    assert_eq!(cav_engine::estimate_text(3e-11, true), "within 3e-11 rad");
    assert_eq!(cav_engine::estimate_text(0.1, false), "did not settle, estimate 1e-1 rad");
    assert_eq!(cav_engine::estimate_text(f64::INFINITY, false), "did not settle");
    // A point mass on a tower's axis does not settle. The reading that comes back is the one
    // asked for (`refined`), and its heading must not pass the estimate off as a bound.
    let mut engine = loaded(dev::point());
    engine.set_patience(LONG);
    set(&mut engine, "bodies[0].motion.Static.position", json!([0.0, 0.0, 1.0])).unwrap();
    let Reply::Reading(reply) = engine.ask(&Query::Reading(None)).unwrap() else { panic!("a reading") };
    assert!(reply.state == "refined" && !reply.settled && reply.estimate > 1e-7);
    let said = engine.run_line("reading").unwrap();
    let heading = said.lines().next().unwrap();
    assert!(heading.contains("did not settle, estimate") && !heading.contains("within"), "{heading}");
    let signal = engine.run_line("signal 1").unwrap();
    assert!(signal.lines().next().unwrap().contains("did not settle"), "{signal}");
    let Reply::Signal(series) =
        engine.ask(&Query::Signal { tower: 1, channel: ChannelName::Lower, span: None }).unwrap()
    else {
        panic!("a signal")
    };
    assert!(!series.settled);
    // The warning names its tower by number, from 0.
    assert!(reply.warnings.iter().all(|w| w.tower == Some(0)) && !reply.warnings.is_empty());
    // Control: the mass moved off the axis settles, and says "within".
    set(&mut engine, "bodies[0].motion.Static.position", json!([3.0, 0.0, 2.5])).unwrap();
    let said = engine.run_line("reading").unwrap();
    assert!(said.lines().next().unwrap().contains("quadrature within"), "{said}");
    let Reply::Reading(reply) = engine.ask(&Query::Reading(None)).unwrap() else { panic!("a reading") };
    assert!(reply.settled && reply.warnings.is_empty());
}

#[test]
fn a_refused_first_step_is_not_a_continuation() {
    let mut engine = loaded(dev::point());
    let path = "bodies[0].motion.Static.position".to_owned();
    // An edit of the body's place, on its own.
    set(&mut engine, &path, json!([2.0, 0.0, 2.5])).unwrap();
    let journal = engine.journal().len();
    // A drag begins with a step that is refused (out of range), and goes on with steps that say
    // they continue. There is nothing for them to continue: the first one applied is an edit of
    // its own, and must not be folded into the set above.
    let step =
        |x: f64, continues: bool| Command::Set { path: path.clone(), value: json!([x, 0.0, 2.5]), continues };
    assert!(engine.apply(step(1.0e9, false)).is_err());
    engine.apply(step(2.5, true)).unwrap();
    engine.apply(step(3.0, true)).unwrap();
    assert_eq!(engine.journal().len(), journal + 1, "the drag is one new entry, not none");
    // One undo takes back the drag and leaves the earlier edit standing.
    engine.apply(Command::Undo).unwrap();
    assert_eq!(engine.spec().bodies[0].motion, cav_sim::Motion::Static { position: [2.0, 0.0, 2.5] });
    assert_eq!(Engine::replay(engine.journal()).unwrap().digest(), engine.digest());
    // Control: with no refusal between them, steps that continue are one entry with the edit
    // they continue, as a drag is.
    let mut smooth = loaded(dev::point());
    smooth.apply(step(2.0, false)).unwrap();
    let journal = smooth.journal().len();
    smooth.apply(step(2.5, true)).unwrap();
    assert_eq!(smooth.journal().len(), journal);
}
