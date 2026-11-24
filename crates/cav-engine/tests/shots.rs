//! The scene's shots and the cadence prediction as the engine keeps them between edits: always
//! those of the scene as it is.

use cav_engine::{Engine, Query, Reply};

fn predicted(engine: &Engine) -> cav_sim::schedule::Prediction {
    let Ok(Reply::Cadence(prediction)) = engine.query(&Query::Cadence) else { panic!("no prediction") };
    prediction
}

#[test]
fn a_kept_prediction_is_the_prediction_of_the_scene_as_it_is() {
    // The engine keeps its last cadence answer between frames, and the shots it made at the
    // last edit. After any edit both must be the edited scene's, not the ones kept.
    let mut engine = Engine::new();
    engine.run_line("load cadence-lab").unwrap();
    assert_eq!(predicted(&engine), predicted(&engine), "asked twice, the same answer");
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
        "remove array.towers[4]",
        "undo",
        "redo",
    ] {
        let before = predicted(&engine);
        engine.run_line(line).unwrap();
        let after = predicted(&engine);
        assert_eq!(after, cav_sim::schedule::predict(engine.spec()).unwrap(), "stale after `{line}`");
        assert_eq!(
            engine.shots().unwrap(),
            &cav_sim::read::shots(engine.spec()).unwrap(),
            "the engine's shots after `{line}`"
        );
        differing += usize::from(after != before);
    }
    // The edits did change the prediction, so a kept answer served stale would have been seen.
    assert!(differing >= 8, "only {differing} of the edits changed the prediction");
    // What does not touch the shots keeps the answer: an offset moves shots and takes none.
    let before = predicted(&engine);
    engine.run_line("set array.towers[0].timing.offset 0.4").unwrap();
    engine.run_line("seek 3").unwrap();
    assert_eq!(predicted(&engine), before);
    assert_eq!(engine.shots().unwrap().offsets[0], 0.4, "but the shots do have the offset");
    // A window asked with no span ends at 2.5 times the mean rate, with no prediction made for it.
    let Ok(Reply::Window(window)) = engine.query(&Query::Window { tower: 1, to: None }) else {
        panic!("no window")
    };
    assert!((window.frequencies.last().unwrap() - 2.5 / before.mean_cycle).abs() < 1e-12);
    // Control: a scene with too many shots has none, and says why; nothing stale is offered.
    engine.run_line("set duration 1e6").unwrap();
    engine.run_line("set array.instrument.half_sequence 0.001").unwrap();
    engine.run_line("set array.instrument.cycle 0.0021").unwrap();
    assert!(engine.shots().unwrap_err().contains("at most 2000000"));
    assert!(engine.query(&Query::Cadence).is_err());
}
