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
        ("array.towers[0].instrument", json!({"cycle": 1.0}), "array.towers[0].instrument.cycle"),
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
    let query = Query::Signal { tower: 2, channel: ChannelName::Lower, span: None, compared: false };
    let Reply::Signal(reply) = engine.query(&query).unwrap() else { panic!("a signal") };
    assert_eq!((reply.state.as_str(), reply.tower), ("refined", 2));
    assert_eq!(bits(&reply.values), bits(cold.channel(1, cav_core::forward::Channel::Lower)));
    assert_eq!(reply.times, cold.times(1));
    assert!(reply.estimate <= 1e-10);
    // The log says a signal arrived.
    let said = engine.log().select(&LogFilter {
        text: Some("signal: 5 tower(s), 30 shot(s) in all".into()),
        ..LogFilter::default()
    });
    assert_eq!(said.len(), 1);
    // The void's signal is all zeros and is there at once: nothing to compute.
    let mut void = loaded(dev::void());
    void.poll();
    assert_eq!(void.signal().0, SignalState::Refined);
    // Control: a tower that is not there is refused, and a scene with too many shots says why.
    assert!(
        engine
            .query(&Query::Signal {
                tower: 6,
                channel: ChannelName::Gradiometer,
                span: None,
                compared: false
            })
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
    let query = Query::Signal { tower: 1, channel: ChannelName::Gradiometer, span: None, compared: false };
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
        Query::Signal { tower: 3, channel: ChannelName::Upper, span: None, compared: false },
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
    // (The pane that is not there was @5 while a figure held four; S20's mosaic holds twelve.)
    for line in ["span 3 1 @2", "span 2 2 @2", "span nan 4 @2", "span 0 1 @13"] {
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
    let query =
        Query::Signal { tower: 1, channel: ChannelName::Lower, span: Some([0.0, 1.0]), compared: false };
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
    // A series that never arrives while this test wants it not to: its worker is held at its
    // gate, so nothing here depends on how long a series takes or how busy the machine is. (This
    // was a 200,000-shot series and a limit of 20 s on the clock; the read in parallel made the
    // series quick and a busy machine made the readings slow, and the test failed for neither
    // reason it was written for.)
    let mut engine = Engine::new();
    engine.hold_series(true);
    engine.apply(Command::load(dev::pass())).unwrap();
    engine.set_patience(LONG);
    // The reading is asked for with a long patience and comes back refined although the series
    // cannot: it waited for the reading worker and for nothing else. (It once waited for the
    // series: with the series held, that fault would wait out the whole patience and then say
    // `stale`, and the assertions below would fail.)
    let said = engine.run_line("reading").unwrap();
    assert!(said.contains("· refined ·"), "{said}");
    let away = engine.run_line("reading 2.5").unwrap();
    assert!(away.contains("a shot read out at 2.5 s · refined"), "{away}");
    assert_ne!(engine.signal().0, SignalState::Refined, "the series is held: it cannot have arrived");
    // The typed form of the same: `ask` waits for what the query needs and asks the worker for a
    // time that is not the playhead's, which `query` alone never does.
    let Reply::Reading(direct) = engine.query(&Query::Reading(Some(7.5))).unwrap() else { panic!() };
    assert_ne!(direct.state, "refined", "nobody had asked for 7.5 s");
    let Reply::Reading(asked) = engine.ask(&Query::Reading(Some(7.5))).unwrap() else { panic!() };
    assert_eq!((asked.state.as_str(), asked.time), ("refined", 7.5));
    // Control: a signal is what does wait for the series; with no patience it says where it
    // stands, and once the series is let go it arrives.
    engine.set_patience(Duration::ZERO);
    let signal = engine.run_line("signal 1 0 1").unwrap_or_else(|why| why);
    assert!(signal.contains("computing") || signal.contains("stale"), "{signal}");
    engine.hold_series(false);
    assert!(engine.settle(LONG), "the series, let go, arrives");
    assert_eq!(engine.signal().0, SignalState::Refined);
}

#[test]
fn an_unsettled_reading_says_so() {
    // The words: an estimate is "within" only when the read settled.
    assert_eq!(cav_engine::estimate_text(0.0, true, None), "exact");
    assert_eq!(cav_engine::estimate_text(3e-11, true, None), "within 3e-11 rad");
    assert_eq!(cav_engine::estimate_text(0.1, false, None), "did not settle, estimate 1e-1 rad");
    assert_eq!(cav_engine::estimate_text(f64::INFINITY, false, None), "did not settle");
    // A read of part of the matter (the world within 2 m of each tower) says so, whatever the
    // quadrature did, and never "within" a number of radians.
    let part = cav_engine::estimate_text(1e-8, false, Some(2.0));
    assert_eq!(
        part,
        "not settled: of the matter within 2 m only (quadrature of what was read within 1e-8 rad)"
    );
    assert_eq!(cav_engine::estimate_text(1e-8, true, Some(2.0)), part);
    assert!(cav_engine::estimate_text(1e-8, false, Some(0.03)).contains("within 0.03 m only"));
    // A world that was not read at all (past the octree's caps, S5) says so in those words.
    assert_eq!(cav_engine::estimate_text(0.0, false, Some(0.0)), "not settled: the world is not read");
    // S5's badge: the world's ledger, its largest cut, and the words for a walk that stopped.
    let tower = |ledger: f64, settled: bool| read::TowerLedger {
        ledger,
        accepted: 400,
        opened: 12,
        per_level: vec![300, 100],
        blocks: 88,
        blocks_per_level: vec![8, 30, 50, 0],
        leaves: 9000,
        degree: 16,
        far: 480,
        near: 8,
        audit: 1e-5,
        settled,
    };
    let settled = read::WorldLedger { tolerance: 1e-3, towers: vec![tower(3e-4, true), tower(1e-4, true)] };
    // Within the ledger and the series' audit together, and the cut's nodes and blocks.
    assert_eq!(cav_engine::world_text(&settled), "world within 3e-4 rad (ledger and series, 500 nodes)");
    let stopped = read::WorldLedger { tolerance: 1e-3, towers: vec![tower(3e-3, false), tower(1e-4, true)] };
    assert_eq!(
        cav_engine::world_text(&stopped),
        "world not settled: ledger 3e-3 rad, 2e-3 over τ (500 nodes)"
    );
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
    let Reply::Signal(series) = engine
        .ask(&Query::Signal { tower: 1, channel: ChannelName::Lower, span: None, compared: false })
        .unwrap()
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

#[test]
fn non_finite_numbers_survive_a_reply() {
    // Over a half-space each interferometer's own phase is infinite. The reply that carries it
    // must survive JSON, which is how it reaches an agent (S1: it did not).
    let mut engine = loaded(dev::infinite_floor());
    assert!(engine.settle(LONG));
    for query in [
        Query::Signal { tower: 2, channel: ChannelName::Lower, span: None, compared: false },
        Query::Reading(None),
    ] {
        let reply = engine.query(&query).unwrap();
        let text = serde_json::to_string(&reply).unwrap();
        assert!(text.contains("\"inf\"") && !text.contains("null,null"), "{text}");
        assert_eq!(serde_json::from_str::<Reply>(&text).unwrap(), reply, "{text}");
        let value = serde_json::to_value(&reply).unwrap();
        assert_eq!(serde_json::from_value::<Reply>(value).unwrap(), reply);
    }
    let Reply::Signal(lower) = engine
        .query(&Query::Signal { tower: 2, channel: ChannelName::Lower, span: None, compared: false })
        .unwrap()
    else {
        panic!("a signal")
    };
    assert!(lower.values.iter().all(|v| *v == f64::INFINITY) && lower.values.len() == 6);
    // The gradiometer over the same floor is finite, and exactly nothing.
    let Reply::Signal(null) = engine
        .query(&Query::Signal { tower: 2, channel: ChannelName::Gradiometer, span: None, compared: false })
        .unwrap()
    else {
        panic!("a signal")
    };
    assert!(null.values.iter().all(|v| v.to_bits() == 0));
    // Control: the plain derive would have written `null` and failed to read it back.
    assert_eq!(serde_json::to_string(&f64::INFINITY).unwrap(), "null");
    assert!(serde_json::from_str::<f64>("null").is_err());
}

/// A session of every kind of edit, each a command a panel, the console or an agent could issue.
fn edits() -> Vec<Command> {
    let body = |label: &str, x: f64| json!({"label": label, "mass": 2.0, "motion": {"Static": {"position": [x, 1.0, 2.0]}}});
    let drag = |x: f64, continues: bool| Command::Set {
        path: "bodies[0].motion.Static.position".into(),
        value: json!([x, 0.0, 2.5]),
        continues,
    };
    vec![
        Command::set("bodies[0].mass", json!(0.1 + 0.2)),
        Command::Insert { path: "bodies".into(), index: None, value: body("second", 3.0) },
        Command::Insert { path: "bodies".into(), index: Some(0), value: body("first", -3.0) },
        Command::set("bodies[2].radius", json!(1.0 / 3.0)),
        Command::Remove { path: "bodies[1]".into() },
        Command::set("room", json!(null)),
        Command::set("room", json!({"origin": [-5.0, -5.0, 0.0], "size": [10.0, 10.0, 3.0]})),
        drag(2.0, false),
        drag(2.25, true),
        drag(2.5, true),
        Command::set(
            "bodies[0].motion",
            json!({"Linear": {"centre": [3, 0, 1], "amplitude": [0, 0, 0.5], "frequency": 0.05, "phase": 0}}),
        ),
        Command::set("array.towers[3].position[1]", json!(4.5)),
        Command::Insert {
            path: "array.towers".into(),
            index: None,
            value: json!({"label": "tower 6", "position": [6, 0, 0]}),
        },
        Command::load(dev::superposition()),
        Command::set("name", json!("renamed")),
        Command::Remove { path: "bodies[0]".into() },
        Command::set("duration", json!(55.0)),
    ]
}

#[test]
fn edits_undo_and_redo_to_the_bit() {
    let mut engine = loaded(dev::point());
    // The scene after each edit. (The drag's three steps are one edit: one entry.)
    let mut scenes = vec![engine.spec().to_ron()];
    for command in edits() {
        let continues = matches!(command, Command::Set { continues: true, .. });
        engine.apply(command).unwrap();
        if continues {
            scenes.pop();
        }
        scenes.push(engine.spec().to_ron());
    }
    let steps = scenes.len() - 1;
    assert_eq!(steps, 15);
    // Undone one at a time, the scene is each earlier scene in turn, as text (so that -0.0 and
    // 0.0, which compare equal, would show).
    for back in (0..steps).rev() {
        engine.apply(Command::Undo).unwrap();
        assert_eq!(engine.spec().to_ron(), scenes[back], "undo to step {back}");
    }
    assert_eq!(engine.spec(), &dev::point());
    // The load itself is still there to undo; then nothing is.
    engine.apply(Command::Undo).unwrap();
    assert_eq!(engine.spec(), &SceneSpec::room());
    assert_eq!(engine.apply(Command::Undo), Err("nothing to undo".into()));
    engine.apply(Command::Redo).unwrap();
    // Redone, each later scene in turn, and at the end the last.
    for (forward, scene) in scenes.iter().enumerate().skip(1) {
        engine.apply(Command::Redo).unwrap();
        assert_eq!(&engine.spec().to_ron(), scene, "redo to step {forward}");
    }
    assert_eq!(engine.apply(Command::Redo), Err("nothing to redo".into()));
    // The whole session, undos and redos and all, replays to the same state.
    assert_eq!(Engine::replay(engine.journal()).unwrap().digest(), engine.digest());
    // The history holds edits, not scenes: sixteen steps on small scenes are a few kilobytes,
    // most of it the two loads (a scene each way).
    let (history, _, _) = engine.held();
    assert!(history < 24_000, "{history} bytes of history");
    // Control: the scenes do differ from step to step, so matching each is a claim about each.
    for pair in scenes.windows(2) {
        assert_ne!(pair[0], pair[1]);
    }
    // And an edit between an undo and its redo forgets the redo.
    engine.apply(Command::Undo).unwrap();
    set(&mut engine, "name", json!("forked")).unwrap();
    assert_eq!(engine.apply(Command::Redo), Err("nothing to redo".into()));
}

#[test]
fn insert_and_remove_are_each_others_inverse() {
    let mut engine = loaded(dev::superposition());
    let before = engine.spec().clone();
    let labels = |engine: &Engine| engine.spec().bodies.iter().map(|b| b.label.clone()).collect::<Vec<_>>();
    let (a, b) = (before.bodies[0].label.clone(), before.bodies[1].label.clone());
    let third = json!({"label": "third", "mass": 2.0, "motion": {"Static": {"position": [1.0, 2.0, 3.0]}}});
    engine.apply(Command::Insert { path: "bodies".into(), index: Some(1), value: third.clone() }).unwrap();
    assert_eq!(labels(&engine), [a.clone(), "third".to_owned(), b.clone()]);
    engine.apply(Command::Remove { path: "bodies[1]".into() }).unwrap();
    assert_eq!(engine.spec(), &before);
    // Undo puts the removed item back where it was; undo again takes the inserted one out.
    engine.apply(Command::Undo).unwrap();
    assert_eq!(engine.spec().bodies[1].label, "third");
    engine.apply(Command::Undo).unwrap();
    assert_eq!(engine.spec(), &before);
    // The console's words for the same, and the log's line says what the edit invalidates.
    engine.run_line(&format!("add bodies 0 {third}")).unwrap();
    assert_eq!(engine.spec().bodies[0].label, "third");
    assert_eq!(
        engine.journal().last(),
        Some(&Command::Insert { path: "bodies".into(), index: Some(0), value: third })
    );
    let said = engine.log().iter().next_back().unwrap().message.clone();
    // The log's line is the line that issues the command again.
    let mut again = loaded(dev::superposition());
    again.run_line(&said).unwrap();
    assert_eq!(again.spec(), engine.spec());
    engine.run_line("remove bodies[0]").unwrap();
    assert_eq!(engine.spec(), &before);
    engine.run_line(r#"add array.towers {"label": "tower 6", "position": [6, 0, 0]}"#).unwrap();
    assert_eq!(engine.spec().array.towers.len(), 6);
    // A selection that the removal takes away is dropped.
    engine.run_script("select tower 6; remove array.towers[5]").unwrap();
    assert_eq!(engine.selection(), None);
    // Control: refusals change nothing and name what is wrong.
    let digest = engine.digest();
    for (line, why) in [
        ("remove bodies[7]", "has 2 items"),
        ("remove bodies", "ends in an index"),
        ("add bodies 9 {}", "at [0] to [2]"),
        ("add bodies not-json", "is not JSON"),
        ("add name 3", "not a list"),
    ] {
        let refused = engine.run_line(line).unwrap_err();
        assert!(refused.contains(why), "`{line}`: {refused}");
    }
    assert_eq!(engine.digest(), digest);
}

#[test]
fn a_replay_never_writes_where_the_journal_says() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/replay exports");
    let (first, second) = (root.join("where the session wrote"), root.join("where the replay writes"));
    for dir in [&first, &second] {
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(dir).unwrap();
    }
    let scene = first.join("my scene.ron");
    let session = first.join("session.json");
    let mut engine = loaded(dev::point());
    for command in edits().into_iter().take(6) {
        engine.apply(command).unwrap();
    }
    engine.run_line(&format!("save \"{}\"", scene.display())).unwrap();
    engine.run_line("set duration 30").unwrap();
    engine.run_line(&format!("journal save \"{}\"", session.display())).unwrap();
    // The saved scene is the scene as it stood at the save, and loads.
    let saved = std::fs::read(&scene).unwrap();
    let mut at_save = engine.spec().clone();
    at_save.duration = dev::point().duration;
    assert_eq!(cav_sim::load(scene.to_str().unwrap()).unwrap(), at_save);
    // The journal on disk is the session up to the command that wrote it.
    let journal = cav_engine::journal::read(session.to_str().unwrap()).unwrap();
    assert_eq!(journal.as_slice(), &engine.journal()[..engine.journal().len() - 1]);
    // A plain replay writes nothing at all: the files are gone and stay gone.
    std::fs::remove_file(&scene).unwrap();
    std::fs::remove_file(&session).unwrap();
    let replayed = Engine::replay(engine.journal()).unwrap();
    assert_eq!(replayed.digest(), engine.digest());
    assert!(!scene.exists() && !session.exists(), "a replay wrote where the journal said");
    // A replay with somewhere to write puts each export there under its file name, and the
    // bytes are the bytes the session wrote.
    let again = Engine::replay_into(&journal, &second).unwrap();
    assert_eq!(again.spec(), engine.spec());
    assert!(!scene.exists(), "still nothing where the journal said");
    assert_eq!(std::fs::read(second.join("my scene.ron")).unwrap(), saved);
    // A journal that names a path outside is held to the directory all the same.
    let outside =
        vec![Command::Export { what: cav_engine::ExportKind::Scene, to: "../../../escaped.ron".into() }];
    Engine::replay_into(&outside, &second).unwrap();
    assert!(second.join("escaped.ron").exists());
    assert!(!second.join("../../../escaped.ron").exists());
    // Control: a session of one's own does write where it says, and a path that cannot be
    // written is a refusal that leaves no entry in the journal.
    let mut own = loaded(dev::void());
    let entries = own.journal().len();
    let nowhere = first.join("no such folder").join("x.ron");
    assert!(own.run_line(&format!("save \"{}\"", nowhere.display())).unwrap_err().contains("not written"));
    assert_eq!(own.journal().len(), entries);
    own.run_line(&format!("save \"{}\"", scene.display())).unwrap();
    assert_eq!(cav_sim::load(scene.to_str().unwrap()).unwrap(), dev::void());
}

cav_validate::case! {
    test: validation_compare_equals_the_other_preset,
    name: "presets.compare_equals_other_preset",
    subsystem: "presets",
    slice: "S2",
    reference: Exact,
    against: "the comb read on AION-10 and, by `compare aice`, on AICE 140 m as well: the comparison's signal against the same scene loaded with the AICE instrument; values of any channel that differ in any bit",
    unit: Count,
    tolerance: 0.0,
    measure: {
        let mut scene = dev::comb();
        scene.duration = 200.0;
        let mut engine = loaded(scene.clone());
        engine.run_line("compare aice").unwrap();
        assert!(engine.settle(LONG));
        let compared = engine.compared_signal().1.expect("the comparison's signal").clone();
        // The same scene, loaded with the instrument set to AICE, read cold.
        let mut other = scene.clone();
        other.array.instrument = cav_sim::presets::aice();
        let cold = read::signal(&other).unwrap();
        let differing = |a: &[f64], b: &[f64]| {
            if a.len() == b.len() { a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count() } else { usize::MAX }
        };
        let count = differing(&compared.lower, &cold.lower)
            + differing(&compared.upper, &cold.upper)
            + differing(&compared.gradiometer, &cold.gradiometer);
        assert_eq!(compared.times(0), cold.times(0), "AICE's own shots: one every 5 s, read out 5 s after launch");
        assert_eq!(compared.times(0).len(), 40);
        // Control: the comparison is another instrument's reading, not the scene's own again:
        // the scene's own signal is still AION-10's, with its own shots.
        let own = engine.signal().1.unwrap().clone();
        assert_eq!(own.as_ref(), &read::signal(&scene).unwrap());
        assert_eq!(own.times(0).len(), 100);
        // (Its first shot is another shot altogether: launched together, read out at 5 s, not 1.46 s.)
        assert_ne!(compared.gradiometer[0].to_bits(), own.gradiometer[0].to_bits());
        assert_ne!(compared.times(0)[0], own.times(0)[0]);
        cav_validate::Measured::Error(count as f64)
    },
}

#[test]
fn a_preset_is_one_undoable_edit() {
    let mut engine = loaded(dev::comb());
    engine.set_patience(LONG);
    let before = engine.spec().clone();
    let journal = engine.journal().len();
    // A preset is a set of the instrument: one command, one undo, replayed like any other.
    engine.run_line("preset aice").unwrap();
    assert_eq!(engine.spec().array.instrument, cav_sim::presets::aice());
    assert_eq!(engine.journal().len(), journal + 1);
    let Some(Command::Set { path, .. }) = engine.journal().last() else { panic!("a set") };
    assert_eq!(path, "array.instrument");
    // Everything else in the scene is as it was.
    let mut expected = before.clone();
    expected.array.instrument = cav_sim::presets::aice();
    assert_eq!(engine.spec(), &expected);
    engine.run_line("undo").unwrap();
    assert_eq!(engine.spec(), &before);
    engine.run_line("redo").unwrap();
    assert_eq!(Engine::replay(engine.journal()).unwrap().digest(), engine.digest());
    // The launch rule as a command: on AION-10's instrument it replaces George's 3.86 m/s.
    engine.run_line("preset aion10").unwrap();
    engine.run_line("launch symmetric").unwrap();
    let launch = engine.spec().array.instrument.launch_velocity;
    assert!((launch - (9.81 * 0.73 - 6.5779 / 2.0)).abs() < 1e-3, "{launch}");
    // Comparing is not an edit: the scene and its history are untouched; the answers are both
    // there, each for its own instrument, and `compare off` drops the comparison.
    engine.run_line("preset aion10").unwrap();
    let scene = engine.spec().clone();
    let (undo, _) = engine.history();
    engine.run_line("compare aice").unwrap();
    assert_eq!((engine.spec(), engine.history().0), (&scene, undo));
    let own = engine.run_line("periodogram 1").unwrap();
    let other = engine.run_line("periodogram 1 compared").unwrap();
    assert!(
        own.contains("Nyquist frequency, 0.25 Hz") && other.contains("Nyquist frequency, 0.1 Hz"),
        "{own}\n{other}"
    );
    assert!(engine.run_line("stats").unwrap().contains("\"compare\": \"AICE 140 m\""));
    assert_eq!(Engine::replay(engine.journal()).unwrap().digest(), engine.digest());
    engine.run_line("compare off").unwrap();
    assert!(engine.comparison().is_none());
    // Control: refusals. A preset that is not one; a comparison asked for with none set; a scene
    // that cannot be read on the other instrument (a floor above where its towers stand is
    // fine, a body count is fine: here, a cycle shorter than AICE's flight is not the scene's).
    assert!(engine.run_line("preset moon").unwrap_err().contains("not a preset (aion10, aice)"));
    assert!(engine.run_line("signal 1 compared").unwrap_err().contains("nothing is being compared"));
    assert!(engine.run_line("compare moon").is_err());
}

/// The shots tower `tower` took and what it read, from the engine's own answer.
fn taken_series(engine: &mut Engine, tower: usize) -> (Vec<f64>, Vec<f64>) {
    let query = Query::Signal { tower, channel: ChannelName::Gradiometer, span: None, compared: false };
    let Reply::Signal(signal) = engine.ask(&query).unwrap() else { panic!("a signal") };
    assert_eq!(signal.state, "refined");
    let kept = signal.times.iter().zip(&signal.values).zip(&signal.taken).filter(|(_, taken)| **taken);
    kept.map(|((time, value), _)| (*time, *value)).unzip()
}

cav_validate::case! {
    test: validation_periodogram_on_uneven_shots,
    name: "cadence.periodogram_on_uneven_shots",
    subsystem: "cadence",
    slice: "S2",
    reference: Exact,
    against: "the cadence-lab scene (each cycle up to 0.3 s late, 2% of shots lost): the engine's periodogram of tower 2, by non-uniform FFT, against the exact Lomb–Scargle of the same shots; largest difference as a fraction of the peak",
    unit: Relative,
    tolerance: 1e-9,
    measure: {
        let mut scene = dev::cadence_lab();
        scene.duration = 600.0;
        let mut engine = loaded(scene);
        engine.set_patience(LONG);
        let (times, values) = taken_series(&mut engine, 2);
        let asked = Query::Periodogram { tower: 2, channel: ChannelName::Gradiometer, compared: false };
        let Reply::Periodogram(ours) = engine.ask(&asked).unwrap() else { panic!("a periodogram") };
        assert_eq!(ours.method, "Lomb–Scargle (non-uniform FFT)");
        assert_eq!(ours.shots, times.len());
        assert!(times.len() < 280 && times.len() > 250, "{} shots taken of about 279", times.len());
        // The mean cycle is 2.15 s, so the Nyquist frequency of the mean rate is 1/4.3 Hz.
        assert!((ours.nyquist - 1.0 / 4.3).abs() < 1e-12);
        let exact = cav_core::spectrum::lomb_scargle(&times, &values, &ours.frequencies);
        let peak = exact.iter().fold(0.0f64, |m, s| m.max(*s));
        let worst = exact.iter().zip(&ours.power).fold(0.0f64, |m, (a, b)| m.max((a - b).abs()));
        // The oscillator's line is the tallest thing in it, at 0.05 Hz, to a bin.
        let top = (0..ours.power.len()).max_by(|&a, &b| ours.power[a].total_cmp(&ours.power[b])).unwrap();
        let bin = ours.frequencies[1] - ours.frequencies[0];
        assert!((ours.frequencies[top] - 0.05).abs() <= bin, "the peak is at {} Hz", ours.frequencies[top]);
        // Control: the classical periodogram of the same values, as if they were evenly spaced,
        // is another spectrum: the method matters on these shots.
        let naive = cav_core::spectrum::periodogram(&values, 2.15, cav_core::spectrum::Classical::SCRIPTS);
        let top_naive = naive.peak().unwrap();
        assert!((top_naive.2 / peak - 1.0).abs() > 1e-3, "{} against {peak}", top_naive.2);
        cav_validate::Measured::Error(worst / peak)
    },
}

#[test]
fn an_even_cadence_keeps_the_classical_periodogram() {
    let mut scene = dev::comb();
    scene.duration = 400.0;
    let mut engine = loaded(scene.clone());
    engine.set_patience(LONG);
    let asked = Query::Periodogram { tower: 1, channel: ChannelName::Gradiometer, compared: false };
    let Reply::Periodogram(ours) = engine.ask(&asked).unwrap() else { panic!("a periodogram") };
    assert_eq!((ours.method.as_str(), ours.shots), ("classical periodogram (FFT)", 200));
    // To the bit what S1 computed: the classical periodogram of the tower's series.
    let cold = read::signal(&scene).unwrap();
    let spectrum = cav_core::spectrum::periodogram(
        cold.channel(0, cav_core::forward::Channel::Gradiometer),
        2.0,
        cav_core::spectrum::Classical::SCRIPTS,
    );
    assert_eq!(bits(&ours.power), bits(&spectrum.power));
    assert_eq!(bits(&ours.frequencies), bits(&spectrum.frequencies));
    // A seed alone changes nothing: the schedule is still the plain one.
    set(&mut engine, "array.schedule.seed", json!(77)).unwrap();
    let Reply::Periodogram(seeded) = engine.ask(&asked).unwrap() else { panic!("a periodogram") };
    assert_eq!(bits(&seeded.power), bits(&ours.power));
    // Control: one lost shot in a hundred, and it is Lomb–Scargle on the shots that are left.
    set(&mut engine, "array.schedule.loss", json!(0.05)).unwrap();
    let Reply::Periodogram(lossy) = engine.ask(&asked).unwrap() else { panic!("a periodogram") };
    assert_eq!(lossy.method, "Lomb–Scargle (non-uniform FFT)");
    assert!(lossy.shots < 200 && lossy.shots > 170, "{}", lossy.shots);
    // The signal says which shots were not taken, and prints them as such.
    let text = engine.run_line("signal 1").unwrap();
    assert_eq!(text.lines().filter(|line| line.ends_with("not taken")).count(), 200 - lossy.shots);
}

#[test]
fn the_cadence_answer_is_the_closed_forms() {
    let mut engine = loaded(dev::cadence_lab());
    // The engine's answer is the prediction, and the console prints it.
    let Reply::Cadence(answer) = engine.query(&Query::Cadence).unwrap() else { panic!("a prediction") };
    assert_eq!(answer, cav_sim::schedule::predict(engine.spec()).unwrap());
    assert_eq!(answer.mean_cycle, 2.15);
    let said = engine.run_line("cadence").unwrap();
    assert!(said.starts_with("# mean cycle 2.15 s · Nyquist 0.232558 Hz"), "{said}");
    assert!(said.contains("cycle jitter (a random walk)") && said.contains("as if alone"), "{said}");
    // Editing the schedule changes the answer at once: nothing is simulated to give it.
    engine.hold_workers(true);
    engine.run_line("set array.schedule.jitter null").unwrap();
    engine.run_line("set array.schedule.alternate 2.5").unwrap();
    let said = engine.run_line("cadence").unwrap();
    assert!(said.contains("0.444444  0.9397  alternation, sideband 2"), "{said}");
    // The window of the same schedule, one realisation: its largest lines are where the forms
    // put them (2/4.5 Hz first: the tallest sideband).
    let window = engine.run_line("window 1 to 1").unwrap();
    let first = window.lines().nth(1).unwrap();
    assert!(first.starts_with("0.44"), "{window}");
    let Reply::Window(reply) = engine.query(&Query::Window { tower: 1, to: Some(1.0) }).unwrap() else {
        panic!()
    };
    assert_eq!((reply.frequencies.len(), reply.window[0]), (cav_engine::engine::WINDOW_POINTS, 1.0));
    // Control: refusals.
    assert!(engine.run_line("window 9").unwrap_err().contains("no tower 9"));
    assert!(engine.run_line("window 1 to -1").is_err());
}

/// Every file under `dir`, by name, with its bytes.
fn files_of(dir: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).unwrap_or_else(|e| panic!("{}: {e}", at.display())) {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let name = path.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(name, std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

/// How many files of two directories differ, counting a file that only one of them has.
fn files_differing(a: &std::path::Path, b: &std::path::Path) -> usize {
    let (a, b) = (files_of(a), files_of(b));
    let names: std::collections::BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    names.into_iter().filter(|name| a.get(*name) != b.get(*name)).count()
}

/// The session of the exit test: a scene loaded, edited every way there is (a drag among them),
/// its towers staggered, read on a second instrument for a while, its cadence jittered and
/// thinned, undone and redone, and exported twice, once before the last edits and once after.
/// (Figures are not in it: this crate cannot draw. `cav-plot`'s
/// `slow_a_session_with_figures_and_a_bundle_replays_to_the_same_files` has them.)
fn exit_session(engine: &mut Engine, out: &std::path::Path, settle_each: bool) {
    let step = |engine: &mut Engine, line: &str| {
        engine.run_line(line).unwrap_or_else(|why| panic!("`{line}`: {why}"));
        if settle_each {
            assert!(engine.settle(LONG));
        }
    };
    step(engine, "load comb");
    step(engine, "set duration 120");
    step(engine, "mass 25 at 3 -2 1.5");
    step(
        engine,
        r#"set bodies[1].motion {"Pass": {"from": [-6, 3, 1], "to": [6, 3, 1], "start": 10, "speed": 1.5}}"#,
    );
    // A drag: forty steps, each superseding the work begun for the one before.
    for n in 0..40 {
        let x = 3.0 + 0.01 * f64::from(n);
        engine
            .apply(Command::Set {
                path: "bodies[0].motion.Linear.centre".into(),
                value: json!([x, 0.0, 1.0]),
                continues: n > 0,
            })
            .unwrap();
    }
    if settle_each {
        assert!(engine.settle(LONG));
    }
    step(engine, "set array.towers[2].timing.offset 0.292");
    step(engine, "set array.towers[4].timing.offset -0.5");
    // A comparison, on for the first bundle (whose manifest says so) and off for the second.
    step(engine, "compare aice");
    step(engine, &format!("export bundle \"{}\"", out.join("first").display()));
    step(engine, "compare off");
    step(engine, r#"set array.schedule.jitter {"model": "Walk", "kind": "Positive", "delta": 0.3}"#);
    step(engine, "set array.schedule.loss 0.05");
    step(engine, "set array.schedule.seed 7");
    step(engine, "remove bodies[1]");
    step(engine, "undo");
    step(engine, "undo");
    step(engine, "redo");
    step(engine, "preset aice");
    step(engine, "undo");
    step(engine, &format!("save \"{}\"", out.join("scene at the end.ron").display()));
    step(engine, &format!("export bundle \"{}\"", out.join("second").display()));
}

cav_validate::case! {
    test: validation_slow_replay_writes_identical_bundles,
    name: "engine.replay_bundles",
    subsystem: "engine",
    slice: "S2",
    reference: Exact,
    against: "a session (every kind of edit, a drag, staggered towers, a comparison switched on and off, a jittered and thinned cadence, undo and redo) that exports two bundles and a scene, against the same session replayed four ways: from its journal in memory, from its journal saved to disk and read back, with the workers settled after every command, and with them never waited for; files of any replay that differ in any byte (each manifest's provenance, the build's revision and dirty flag, among them)",
    unit: Count,
    tolerance: 0.0,
    measure: {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/exit test");
        let _ = std::fs::remove_dir_all(&root);
        let dirs: Vec<std::path::PathBuf> = ["session", "replayed", "from disk", "settled", "rushed"].iter().map(|name| root.join(name)).collect();
        for dir in &dirs {
            std::fs::create_dir_all(dir).unwrap();
        }
        // The session itself: commands back to back, the signal waited for only by the exports.
        let mut engine = Engine::new();
        engine.set_patience(LONG);
        exit_session(&mut engine, &dirs[0], false);
        let journal = engine.journal().to_vec();
        // 1: replayed from the journal in memory.
        let mut replayed = Engine::new();
        replayed.set_patience(LONG);
        replayed.set_exports(cav_engine::Exports::Into(dirs[1].clone()));
        for command in &journal {
            replayed.apply(command.clone()).unwrap();
        }
        assert_eq!(replayed.digest(), engine.digest());
        // 2: the journal written to disk as a file, read back, and replayed.
        let file = root.join("session.json");
        std::fs::write(&file, cav_engine::journal::write(&journal)).unwrap();
        let read = cav_engine::journal::read(file.to_str().unwrap()).unwrap();
        assert_eq!(read, journal);
        let mut from_disk = Engine::new();
        from_disk.set_patience(LONG);
        from_disk.set_exports(cav_engine::Exports::Into(dirs[2].clone()));
        for command in &read {
            from_disk.apply(command.clone()).unwrap();
        }
        // 3 and 4: the session run again by its own lines, once with the workers let finish
        // after every command (nothing is ever superseded), once as the first time (work for
        // superseded scenes is dropped part-way: latest wins).
        let mut settled = Engine::new();
        settled.set_patience(LONG);
        exit_session(&mut settled, &dirs[3], true);
        let mut rushed = Engine::new();
        rushed.set_patience(LONG);
        exit_session(&mut rushed, &dirs[4], false);
        // The replays put their exports under file names alone, so the layouts are the same:
        // two bundles of eighteen files and a scene.
        let files = files_of(&dirs[0]);
        assert_eq!(files.len(), 37, "{:?}", files.keys().collect::<Vec<_>>());
        assert!(files.contains_key("first/signal.npy") && files.contains_key("second/manifest.json"));
        let differing: usize = dirs[1..].iter().map(|dir| files_differing(&dirs[0], dir)).sum();
        // The provenance in particular: each manifest's bytes are the session's in every replay,
        // and what they say of the engine is the build's, which no replay can change. (Within
        // one process this could not have caught a revision asked of git when the file is
        // written: the answer was asked once and kept. `cav`'s
        // `slow_cli_provenance_is_the_builds_whatever_the_tree_or_the_directory` runs the binary
        // twice with the tree changed in between, and does.)
        for dir in &dirs[1..] {
            let replay = files_of(dir);
            for manifest in ["first/manifest.json", "second/manifest.json"] {
                assert!(replay[manifest] == files[manifest], "{manifest} of {}", dir.display());
                let said: serde_json::Value = serde_json::from_slice(&replay[manifest]).unwrap();
                assert_eq!(said["numerics"]["engine"], cav_engine::build().json());
            }
        }
        // The two bundles are different bundles: the session did something between them.
        assert_ne!(files["first/signal.npy"], files["second/signal.npy"]);
        assert_ne!(files["first/valid.npy"], files["second/valid.npy"]);
        // Control: a journal with one command dropped, and one with one bit of one number
        // changed, write bundles that differ.
        let control = |edit: &dyn Fn(&mut Vec<Command>), name: &str| {
            let dir = root.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            let mut changed = journal.clone();
            edit(&mut changed);
            let mut other = Engine::new();
            other.set_patience(LONG);
            other.set_exports(cav_engine::Exports::Into(dir.clone()));
            for command in changed {
                other.apply(command).unwrap();
            }
            files_differing(&dirs[0], &dir)
        };
        let mass = journal.iter().position(|c| matches!(c, Command::Insert { .. })).expect("the mass");
        assert!(control(&|journal| { journal.remove(mass + 1); }, "one dropped") >= 2);
        let nudged = control(
            &|journal| {
                let at = journal.iter().position(|c| matches!(c, Command::Set { path, .. } if path == "array.towers[2].timing.offset")).unwrap();
                journal[at] = Command::set("array.towers[2].timing.offset", json!(f64::from_bits(0.292f64.to_bits() + 1)));
            },
            "one bit",
        );
        assert!(nudged >= 2, "one unit in the last place of one offset changed {nudged} files");
        cav_validate::Measured::Error(differing as f64)
    },
}

#[test]
fn a_bundle_waits_for_a_refined_signal() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/bundle waits");
    let _ = std::fs::remove_dir_all(&root);
    let mut engine = loaded(dev::pass());
    assert!(engine.settle(LONG));
    // The scene changes and the workers are held: the signal on show is the old scene's. A
    // bundle asked for now is refused, and nothing is written, however long the patience.
    engine.hold_workers(true);
    set(&mut engine, "bodies[0].mass", json!(20.0)).unwrap();
    engine.set_patience(Duration::from_millis(50));
    let journal = engine.journal().len();
    let line = format!("export bundle \"{}\"", root.join("stale").display());
    let why = engine.run_line(&line).unwrap_err();
    assert!(why.contains("only from a refined signal"), "{why}");
    assert!(!root.join("stale").exists());
    assert_eq!(engine.journal().len(), journal, "a refused export is not in the journal");
    // Let the workers go: the same line waits for the signal and writes the new scene's bundle.
    engine.hold_workers(false);
    engine.set_patience(LONG);
    engine.run_line(&line).unwrap();
    let written = std::fs::read(root.join("stale").join("signal.npy")).unwrap();
    let cold = read::signal(engine.spec()).unwrap();
    let (_, values) = cav_core::npy::parse(&written).unwrap().into_f64().unwrap();
    assert_eq!(
        values[..5].iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        (0..5)
            .map(|d| cold.channel(d, cav_core::forward::Channel::Gradiometer)[0].to_bits())
            .collect::<Vec<_>>()
    );
    assert_eq!(engine.journal().len(), journal + 1);
    assert!(engine.log().iter().next_back().unwrap().message.contains("(18 files)"));
    // The manifest names the engine's revision: the build's, a commit or `unknown`.
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("stale").join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["numerics"]["engine"], cav_engine::build().json());
    let revision = manifest["numerics"]["engine"]["revision"].as_str().unwrap();
    assert!(
        revision == "unknown" || (revision.len() == 40 && revision.chars().all(|c| c.is_ascii_hexdigit())),
        "{revision}"
    );
    // A scene that cannot have a signal has no bundle, and says why.
    set(&mut engine, "array.instrument.half_sequence", json!(0.001)).unwrap();
    set(&mut engine, "array.instrument.cycle", json!(0.0021)).unwrap();
    set(&mut engine, "duration", json!(1.0e6)).unwrap();
    assert!(engine.run_line(&line).unwrap_err().contains("no signal to bundle"));
}

#[test]
fn a_replay_waits_for_the_signal_an_export_needs() {
    // A session that wrote a bundle of a long series: the journal has the export straight after
    // the edit, as `cav exec` records it. Replaying it must wait for the signal as the session
    // did (found by the S2 demo: `cav replay` refused its own journal after a tenth of a second).
    let scratch = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("replay-waits");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    let mut session = Engine::new();
    session.set_exports(cav_engine::Exports::Into(scratch.join("first")));
    std::fs::create_dir_all(scratch.join("first")).unwrap();
    session.set_patience(std::time::Duration::from_secs(600));
    for line in ["load comb", "set duration 4000", "export bundle out"] {
        session.run_line(line).unwrap();
    }
    let journal = session.journal().to_vec();

    // Control: with no patience the replay's export finds the signal still being computed (ten
    // thousand reads cannot have finished in the instant since the edit, on any machine: a busy
    // one is slower still), and writes nothing.
    let refused = Engine::replay_into_within(&journal, &scratch.join("none"), std::time::Duration::ZERO);
    let Err(why) = refused else { panic!("an export with no signal was not refused") };
    assert!(why.contains("command 2") && why.contains("still being computed"), "{why}");
    assert!(!scratch.join("none").join("out").join("signal.npy").exists());

    // The replay proper waits, and writes the same bytes as the session.
    std::fs::create_dir_all(scratch.join("again")).unwrap();
    Engine::replay_into(&journal, &scratch.join("again")).unwrap();
    for file in ["signal.npy", "detector_time.npy", "manifest.json"] {
        let first = std::fs::read(scratch.join("first").join("out").join(file)).unwrap();
        let again = std::fs::read(scratch.join("again").join("out").join(file)).unwrap();
        assert!(!first.is_empty() && first == again, "{file} differs");
    }
    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn a_kept_window_is_the_window_of_the_scene_as_it_is() {
    // The engine keeps the last spectral window between frames. After any edit that changes a
    // tower's shots, the answer must be the edited scene's, not the one kept.
    let mut engine = Engine::new();
    engine.run_line("load cadence-lab").unwrap();
    let ask = |engine: &Engine, tower: usize, to: Option<f64>| {
        let Ok(Reply::Window(reply)) = engine.query(&Query::Window { tower, to }) else {
            panic!("no window")
        };
        reply
    };
    let fresh = |engine: &Engine, tower: usize, to: f64| {
        cav_sim::schedule::window(engine.spec(), tower - 1, to, cav_engine::engine::WINDOW_POINTS).unwrap()
    };
    let first = ask(&engine, 1, Some(0.5));
    assert_eq!(first, ask(&engine, 1, Some(0.5)), "asked twice, the same answer");
    let mut differing = 0;
    for line in [
        "set array.schedule.loss 0.3",
        "set array.schedule.seed 7",
        r#"set array.schedule.jitter {"model":"Grid","kind":"Symmetric","delta":0.1}"#,
        "set array.schedule.jitter null",
        r#"set array.schedule.gaps {"on":3,"period":7}"#,
        "set array.schedule.alternate 3.5",
        "set array.instrument.cycle 2.5",
        "set duration 900",
        "set array.towers[0].timing.offset 0.4",
        "undo",
        "redo",
    ] {
        let before = ask(&engine, 1, Some(0.5));
        engine.run_line(line).unwrap();
        let after = ask(&engine, 1, Some(0.5));
        let (frequencies, window) = fresh(&engine, 1, 0.5);
        assert!(after.frequencies == frequencies && after.window == window, "stale after `{line}`");
        differing += usize::from(after != before);
    }
    // The edits did change the window (all but the offset, which moves every shot of the tower
    // alike), so a kept answer served stale would have been seen.
    assert!(differing >= 9, "only {differing} of the edits changed the window");
    // Another tower and another span are other windows.
    let other = ask(&engine, 2, Some(0.5));
    let (frequencies, window) = fresh(&engine, 2, 0.5);
    assert!(other.tower == 2 && other.frequencies == frequencies && other.window == window);
    assert_ne!(ask(&engine, 2, Some(0.25)).frequencies, other.frequencies);
    // What does not touch the shots keeps the answer: the same numbers again.
    let before = ask(&engine, 2, Some(0.5));
    engine.run_line("seek 3").unwrap();
    engine.run_line("set bodies[0].mass 3").unwrap();
    assert_eq!(before, ask(&engine, 2, Some(0.5)));
}

#[test]
fn a_scene_with_no_towers_answers_every_query() {
    let mut engine = loaded(dev::no_towers());
    assert!(engine.settle(Duration::from_secs(60)));
    let (state, signal) = engine.signal();
    assert_eq!(state, SignalState::Refined);
    assert_eq!(signal.unwrap().towers, 0);
    // A tower's query says there is none.
    let asked = Query::Signal { tower: 1, channel: ChannelName::Gradiometer, span: None, compared: false };
    assert!(engine.query(&asked).unwrap_err().contains("the scene has no towers"));
    let periodogram = Query::Periodogram { tower: 1, channel: ChannelName::Gradiometer, compared: false };
    assert!(engine.query(&periodogram).unwrap_err().contains("no towers"));
    assert!(engine.query(&Query::Window { tower: 1, to: None }).unwrap_err().contains("no towers"));
    assert!(engine.apply(Command::Select(Some(Target::Tower(1)))).unwrap_err().contains("no towers"));
    // The array's answers are there, and empty where they are a tower's.
    let Ok(Reply::Cadence(cadence)) = engine.query(&Query::Cadence) else { panic!("a cadence answer") };
    assert!(cadence.kept.is_empty() && cadence.shots > 0);
    assert!(engine.query(&Query::Stats).is_ok());
    // The console's commands answer, and a comparison is made of nothing.
    assert!(engine.run_line("signal").unwrap_err().contains("no towers"));
    assert!(engine.run_line("ls array.towers").is_ok());
    engine.apply(Command::Compare(Some(Box::new(cav_sim::presets::aice())))).unwrap();
    assert!(engine.settle(Duration::from_secs(60)));
    // Control: a tower put back is read.
    engine.run_line("add array.towers {\"label\": \"t\", \"position\": [0.0, 3.0, 0.0]}").unwrap();
    assert!(engine.settle(Duration::from_secs(60)));
    assert!(engine.query(&asked).is_ok());
}

#[test]
fn slow_a_truncated_world_read_reaches_every_reply() {
    // S4's second review, J4: the truncation and `settled` as the API and MCP serve them, in the
    // signal, the reading under a playhead and the periodogram, and in their JSON; and in
    // `read::reading` itself. Since S5 the world is read whole, so the only world left out is one
    // past the octree's caps: stress-1e10's kilometre, not read at all (truncated at 0 m).
    let part = dev::stress_1e10();
    let replies = |scene: SceneSpec| {
        let mut engine = loaded(scene);
        engine.set_patience(LONG);
        assert!(engine.settle(LONG));
        let Reply::Signal(signal) = engine
            .ask(&Query::Signal { tower: 1, channel: ChannelName::Gradiometer, span: None, compared: false })
            .unwrap()
        else {
            panic!("a signal")
        };
        let Reply::Reading(reading) = engine.ask(&Query::Reading(Some(1.0))).unwrap() else {
            panic!("a reading")
        };
        let Reply::Periodogram(periodogram) = engine
            .ask(&Query::Periodogram { tower: 1, channel: ChannelName::Gradiometer, compared: false })
            .unwrap()
        else {
            panic!("a periodogram")
        };
        let texts = [
            serde_json::to_string(&signal).unwrap(),
            serde_json::to_string(&reading).unwrap(),
            serde_json::to_string(&periodogram).unwrap(),
        ];
        (
            (signal.truncated, signal.settled),
            (reading.truncated, reading.settled),
            periodogram.truncated,
            texts,
        )
    };
    let (signal, reading, periodogram, texts) = replies(part.clone());
    assert_eq!((signal, reading, periodogram), ((Some(0.0), false), (Some(0.0), false), Some(0.0)));
    assert!(texts.iter().all(|t| t.contains("\"truncated\":0.0")), "{texts:?}");
    assert_eq!(read::reading(&part, 1.0, &|| false).unwrap().truncated, Some(0.0));
    // Control: the whole box is read whole: no truncation, settled, and no key in the JSON.
    let whole = dev::world_box();
    let (signal, reading, periodogram, texts) = replies(whole.clone());
    assert_eq!((signal, reading, periodogram), ((None, true), (None, true), None));
    assert!(texts.iter().all(|t| !t.contains("truncated")), "{texts:?}");
    assert_eq!(read::reading(&whole, 1.0, &|| false).unwrap().truncated, None);
}

#[test]
fn a_signal_s_news_is_tagged_as_the_forward_stage_s() {
    // Readers that look for what a command did skip the forward stage's news by its tag
    // (`Subsystem::Forward`), so every line written when a signal lands must carry it: the
    // timing, the signal's own line and its warnings. Control: the edit's own line does not.
    let mut engine = loaded(dev::point());
    set(&mut engine, "bodies[0].mass", json!(250.0)).unwrap();
    assert!(engine.settle(LONG), "the signal arrives");
    let log = engine.log();
    let news: Vec<_> = log
        .iter()
        .filter(|e| e.message.starts_with("signal: ") || e.message.starts_with("the signal was whole"))
        .collect();
    assert!(news.len() >= 2, "{log:?}");
    assert!(news.iter().all(|e| e.subsystem == cav_engine::Subsystem::Forward), "{news:?}");
    assert!(
        log.iter().any(|e| e.subsystem != cav_engine::Subsystem::Forward && e.message.contains("mass")),
        "the edit is not the forward stage's: {log:?}"
    );
}
