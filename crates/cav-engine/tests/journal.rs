//! The journal as a record with a size, and replays that write: where a replay's exports land,
//! what the journal is counted in, the way on when it is full, and answers that are cut.
//! (`docs/slices/S2.md` §7.12: the findings of S2's review.)

use std::path::{Path, PathBuf};
use std::time::Duration;

use cav_engine::{
    Command, Engine, ExportKind, Exports, Figure, FigureFormat, FigureKind, HISTORY_BYTES, JOURNAL_BYTES,
    Level, LogFilter, Query, Reply, Subsystem, Target, ViewCommand,
};
use cav_sim::dev;
use serde_json::json;

const LONG: Duration = Duration::from_secs(120);

/// An empty directory of this test's own, under the build's.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/journal").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Every file under `dir`, by its path within it, with its bytes.
fn files_of(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
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

fn save(to: &str) -> Command {
    Command::Export { what: ExportKind::Scene, to: to.to_owned() }
}

#[test]
fn exports_of_a_replay_keep_their_folders_and_never_share_a_file() {
    // A journal that saves the scene seven times, the scene different each time, to paths that
    // all end in the same file name. (S2's review: the replay kept the file name only, so `bA`
    // and `sub/bA` were one file, silently, and "exports written" counted both.)
    let paths = [
        "b.ron",                     // 1: plain
        "sub/b.ron",                 // 2: in a folder: kept
        "/somewhere/else/b.ron",     // 3: absolute: its name alone, which 1 has
        "..\\..\\b.ron",             // 4: climbs: its name alone
        "B.RON",                     // 5: the same file as 1 where case does not count
        "sub/b.ron",                 // 6: the path of 2 again: the same place, written again
        "C:\\Users\\someone\\b.ron", // 7: another platform's absolute path
    ];
    let mut journal = vec![Command::load(dev::point())];
    for (n, path) in paths.iter().enumerate() {
        journal.push(Command::set("duration", json!(10.0 + n as f64)));
        journal.push(save(path));
    }
    let root = scratch("landing");
    let into = root.join("replayed");
    let replayed = Engine::replay_into(&journal, &into).unwrap();

    // Six files, each the scene as it stood at its own save.
    let files = files_of(&into);
    let duration = |name: &str| {
        let text = String::from_utf8(files[name].clone()).unwrap();
        cav_sim::SceneSpec::from_ron(&text).unwrap().duration
    };
    assert_eq!(
        files.keys().map(String::as_str).collect::<Vec<_>>(),
        ["B (4).RON", "b (2).ron", "b (3).ron", "b (5).ron", "b.ron", "sub/b.ron"]
    );
    assert_eq!(duration("b.ron"), 10.0);
    assert_eq!(duration("sub/b.ron"), 15.0, "the path saved twice holds the later scene");
    assert_eq!(duration("b (2).ron"), 12.0);
    assert_eq!(duration("b (3).ron"), 13.0);
    assert_eq!(duration("B (4).RON"), 14.0);
    assert_eq!(duration("b (5).ron"), 16.0);
    // Nothing was written anywhere but inside the directory: not above it, not at the root.
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1, "something was written beside the directory");
    assert!(!root.join("b.ron").exists() && !root.join("../b.ron").exists());
    assert!(!Path::new("/somewhere/else/b.ron").exists());
    // The log says which were moved aside, and for what.
    let moved =
        replayed.log().select(&LogFilter { text: Some("has the plain name".into()), ..LogFilter::default() });
    assert_eq!(moved.len(), 4, "{moved:?}");
    assert!(moved[0].message.contains("as `b (2).ron`"), "{}", moved[0].message);

    // Control: by the file name alone, which was the rule, all seven are one file.
    let names: std::collections::BTreeSet<String> =
        paths.iter().map(|path| path.rsplit(['/', '\\']).next().unwrap().to_lowercase()).collect();
    assert_eq!(names.len(), 1);
    // Control: a session of one's own writes where it says, and nothing is moved aside.
    let own = root.join("replayed").join("own.ron");
    let mut session = Engine::new();
    for _ in 0..2 {
        session.apply(save(own.to_str().unwrap())).unwrap();
    }
    assert!(own.exists() && !into.join("own (2).ron").exists());
    // A path that names no file is refused, and the replay says which command.
    let nothing = Engine::replay_into(&[save("../")], &into).unwrap_err();
    assert!(nothing.contains("command 0") && nothing.contains("names no file"), "{nothing}");
}

/// A figure's export: a command whose text is far larger than what it holds beside its path.
fn figure(n: usize) -> Command {
    Command::Export {
        what: ExportKind::Figure(Box::new(Figure {
            tower: Some(1 + n % 5),
            span: Some([0.25, 7.5]),
            size_mm: Some([85.0, 60.0]),
            ..Figure::new(FigureKind::Periodogram, FigureFormat::Svg)
        })),
        to: format!("figure-{n}.svg"),
    }
}

#[test]
fn a_journal_at_its_budget_is_a_file_that_is_read_back() {
    // The journal is counted in the bytes of the file it would be written as, so a journal that
    // is whole is a file `journal::read` accepts. (S2's review: it was counted by what each
    // command holds in memory, and a whole journal could be written larger than it is read.)
    const BUDGET: usize = 6000;
    let commands = |n: usize| -> Command {
        match n % 4 {
            0 => figure(n),
            1 => Command::set("duration", json!(20.0 + n as f64)),
            2 => Command::Select(Some(Target::Tower(1 + n % 5))),
            _ => Command::View(ViewCommand::Orbit { pane: 0, yaw: n as f64, pitch: 0.1 + 0.2 }),
        }
    };
    // Commands until one does not fit.
    let mut engine = Engine::with_budgets(HISTORY_BYTES, BUDGET);
    engine.set_exports(Exports::Skipped);
    engine.hold_workers(true);
    let mut applied = 0;
    while engine.held().2 {
        engine.apply(commands(applied)).unwrap();
        applied += 1;
    }
    let journal = engine.journal().to_vec();
    assert_eq!(journal.len(), applied - 1, "every command but the one that did not fit");
    assert!(journal.len() > 40, "{}", journal.len());
    // The file is within the budget, is exactly what was counted (less the comma after the last
    // command, which the count includes: the count is one byte more than the file, never less),
    // and the command that did not fit would have taken the count over.
    let file = cav_engine::journal::write(&journal);
    assert!(file.len() <= BUDGET, "a whole journal of {} bytes against a budget of {BUDGET}", file.len());
    assert_eq!(file.len(), engine.held().1 - 1);
    let next = serde_json::to_string(&commands(applied - 1)).unwrap().len();
    assert!(engine.held().1 + next + 1 > BUDGET);
    // An engine that applied exactly these is whole, saves its journal, and the file reads back.
    let mut whole = Engine::with_budgets(HISTORY_BYTES, BUDGET);
    whole.set_exports(Exports::Skipped);
    whole.hold_workers(true);
    for command in &journal {
        whole.apply(command.clone()).unwrap();
    }
    assert!(whole.held().2);
    whole.set_exports(Exports::AsNamed);
    let path = scratch("budget").join("at the budget.json");
    whole.run_line(&format!("journal save \"{}\"", path.display())).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().len(), file.len() as u64);
    assert_eq!(cav_engine::journal::read(path.to_str().unwrap()).unwrap(), journal);
    // At the real size the budget is the most a journal file is read at.
    assert_eq!(cav_engine::journal::MAX_JOURNAL_FILE_BYTES, JOURNAL_BYTES as u64);

    // Control: the old count of a figure's export was the command's size in memory and the
    // length of its path. Its text is larger, so as many of them as that count lets into the
    // budget are a file larger than the budget: one that would have been offered as whole.
    let old = std::mem::size_of::<Command>() + "figure-0.svg".len();
    let text = serde_json::to_string(&figure(0)).unwrap().len();
    assert!(text > old, "{text} bytes of text against {old} counted");
    let fitted: Vec<Command> = (0..BUDGET / old).map(|_| figure(0)).collect();
    assert!(cav_engine::journal::write(&fitted).len() > BUDGET);
}

#[test]
fn a_journal_that_has_stopped_can_be_started_again() {
    const BUDGET: usize = 40_000;
    let mut engine = Engine::with_budgets(HISTORY_BYTES, BUDGET);
    engine.set_patience(LONG);
    // A session with something of every kind of state in it.
    for line in [
        "load comb",
        "panes 2",
        "view top @2",
        "show periodogram @2",
        "orbit 33 21",
        "zoom 17.5",
        "compare aice",
        "set bodies[0].mass 12.5",
        "select tower 3",
    ] {
        engine.run_line(line).unwrap_or_else(|why| panic!("`{line}`: {why}"));
    }
    // Then seeks and long edits by turns until the journal is full. A seek is a few bytes and an
    // edit three thousand, so it is an edit that does not fit and a seek that is recorded last.
    let long = "x".repeat(3000);
    let mut n = 0;
    while engine.held().2 {
        engine.apply(Command::Seek(1.0 + f64::from(n))).unwrap();
        engine.apply(Command::set("description", json!(format!("{long} {n}")))).unwrap();
        n += 1;
    }
    let frozen = engine.journal().to_vec();
    assert!(matches!(frozen.last(), Some(Command::Seek(_))), "{:?}", frozen.last().map(|_| "not a seek"));
    assert!(engine.held().1 <= BUDGET);

    // The record has stopped; the session has not. What is asked of the record is refused,
    // and each refusal names the way on.
    let why = engine.run_line("journal").unwrap_err();
    assert!(why.contains("not the whole session") && why.contains("`journal restart`"), "{why}");
    let path = scratch("restart").join("session.json");
    let save_line = format!("journal save \"{}\"", path.display());
    let why = engine.run_line(&save_line).unwrap_err();
    assert!(why.contains("stopped recording") && why.contains("`journal restart`"), "{why}");
    assert!(!path.exists());
    let said = engine.log().select(&LogFilter {
        min_level: Some(Level::Error),
        text: Some("records no further commands".into()),
        ..LogFilter::default()
    });
    assert!(said.len() == 1 && said[0].message.contains("`journal restart`"), "{said:?}");

    // Commands after the stop are compared with the command before them, not with the journal's
    // last entry, which no longer moves. A seek now is not a step of the scrub that the journal
    // happens to end with: the journal is left exactly as it stopped.
    engine.apply(Command::Seek(2.5)).unwrap();
    engine.apply(Command::Seek(3.5)).unwrap();
    assert_eq!(engine.journal(), frozen.as_slice(), "the stopped journal was rewritten");
    // And a drag after the stop is still one step of undo: its steps continue one another.
    let steps = |engine: &Engine| {
        let Ok(Reply::Stats(stats)) = engine.query(&Query::Stats) else { panic!("stats") };
        stats.undo
    };
    let before = steps(&engine);
    for (n, mass) in [20.0, 21.0, 22.0].into_iter().enumerate() {
        engine
            .apply(Command::Set { path: "bodies[0].mass".into(), value: json!(mass), continues: n > 0 })
            .unwrap();
    }
    assert_eq!(steps(&engine), before + 1, "a drag after the journal stopped is one undo step");
    engine.apply(Command::Seek(4.25)).unwrap();

    // The way on. The state is kept: scene, comparison, panes, what each shows, cameras,
    // playhead, selection. The undo history starts again.
    let state = |engine: &Engine| {
        let Ok(Reply::Stats(stats)) = engine.query(&Query::Stats) else { panic!("stats") };
        (
            engine.spec().clone(),
            (stats.compare, stats.panes, stats.active_pane, stats.time, stats.selection),
            (0..4).map(|pane| (engine.view(pane).clone(), engine.content(pane))).collect::<Vec<_>>(),
        )
    };
    let kept = state(&engine);
    let answer = engine.run_line("journal restart").unwrap();
    assert!(answer.starts_with("the journal starts again"), "{answer}");
    assert_eq!(state(&engine), kept);
    assert_eq!(kept.1, (Some("AICE 100 m".to_owned()), 2, 0, 4.25, Some(Target::Tower(3))));
    assert_eq!(engine.history(), (true, false));
    assert_eq!(steps(&engine), 1);
    // The journal is whole again and is the session: it replays to this state, from memory and
    // from the file it can now be saved as; and it goes on recording.
    assert!(engine.held().2 && engine.held().1 < BUDGET);
    let Ok(Reply::Journal(journal)) = engine.query(&Query::Journal) else { panic!("a journal") };
    assert_eq!(Engine::replay(&journal).unwrap().digest(), engine.digest());
    engine.run_line("set bodies[0].mass 30").unwrap();
    engine.run_line(&save_line).unwrap();
    let read = cav_engine::journal::read(path.to_str().unwrap()).unwrap();
    assert_eq!(read.len(), journal.len() + 1);
    assert_eq!(Engine::replay(&read).unwrap().digest(), engine.digest());
    // The signal is the scene's again once the workers have run: a bundle can be written.
    assert!(engine.settle(LONG));

    // Control: a replay of the journal as it stopped is not this session (which is why it was
    // not offered), and with no room for the scene itself a restart is refused and changes
    // nothing.
    assert_ne!(Engine::replay(&frozen).unwrap().digest(), engine.digest());
    let mut cramped = Engine::with_budgets(HISTORY_BYTES, 300);
    cramped.run_line("load comb").unwrap();
    cramped.run_line("seek 2").unwrap();
    let (digest, whole) = (cramped.digest(), cramped.held().2);
    let why = cramped.run_line("journal restart").unwrap_err();
    assert!(!whole && why.contains("larger than the journal's"), "{why}");
    assert_eq!((cramped.digest(), cramped.held().2), (digest, false));
}

#[test]
fn a_long_journal_and_a_long_log_are_cut_and_say_so() {
    use cav_engine::console::MAX_ANSWER_BYTES;
    let mut engine = Engine::new();
    engine.hold_workers(true);
    // Control first: a short journal is printed whole, as a list `journal::parse` reads.
    engine.run_script("load void; seek 2; select tower 1").unwrap();
    let short = engine.run_line("journal").unwrap();
    assert_eq!(cav_engine::journal::parse(&short).unwrap(), engine.journal());
    assert!(!short.contains('#'));
    // Four hundred edits of four thousand characters: a journal of 1.6 MB.
    let long = "y".repeat(3990);
    for n in 0..400 {
        engine.apply(Command::set("description", json!(format!("{long} {n}")))).unwrap();
    }
    let whole = cav_engine::journal::write(engine.journal());
    assert!(whole.len() > 3 * MAX_ANSWER_BYTES / 2, "{}", whole.len());
    let answer = engine.run_line("journal").unwrap();
    assert!(answer.len() <= MAX_ANSWER_BYTES + 200, "{} bytes", answer.len());
    let (list, note) = answer.split_once('\n').expect("a note after the list");
    let printed: Vec<Command> = serde_json::from_str(list).unwrap();
    assert_eq!(printed.as_slice(), &engine.journal()[..printed.len()], "the first commands, in order");
    let left = engine.journal().len() - printed.len();
    assert!(left > 100 && printed.len() > 200, "{} printed, {left} left out", printed.len());
    assert!(
        note.starts_with(&format!("# … {left} more commands of {} are not printed", engine.journal().len()))
            && note.contains("`journal save <file>`"),
        "{note}"
    );
    // What is cut cannot pass for the journal; saved, the journal is whole.
    assert!(cav_engine::journal::parse(&answer).is_err());
    let path = scratch("cut").join("whole.json");
    engine.run_line(&format!("journal save \"{}\"", path.display())).unwrap();
    assert_eq!(cav_engine::journal::read(path.to_str().unwrap()).unwrap().len(), engine.journal().len() - 1);

    // The log: forty entries of sixty thousand bytes. The newest that fit are printed, under a
    // line that says how many earlier ones were not.
    engine.clear_log();
    for n in 0..40 {
        engine.note(Level::Info, Subsystem::App, format!("entry {n:02} {}", "z".repeat(60_000)));
    }
    let answer = engine.run_line("log 40").unwrap();
    assert!(answer.len() <= MAX_ANSWER_BYTES + 200, "{} bytes", answer.len());
    let lines: Vec<&str> = answer.lines().collect();
    let shown = lines.len() - 1;
    assert!((10..40).contains(&shown), "{shown} entries shown");
    assert_eq!(
        lines[0].split(" (").next().unwrap(),
        format!("# … {} earlier entries of 40 are not printed", 40 - shown)
    );
    assert!(lines[1].contains(&format!("entry {:02} ", 40 - shown)) && lines[shown].contains("entry 39 "));
    // Control: a few entries are printed whole, with no such line.
    let few = engine.run_line("log 5").unwrap();
    assert_eq!(few.lines().count(), 5);
    assert!(!few.contains('#') && few.lines().next().unwrap().contains("entry 35 "));
}

#[test]
fn a_bundle_under_a_comparison_is_the_scenes_own_and_says_so() {
    let root = scratch("compared bundle");
    let mut engine = Engine::new();
    engine.set_patience(LONG);
    engine.run_script("load comb; set duration 60; compare aice").unwrap();
    let export = |engine: &mut Engine, name: &str| {
        let dir = root.join(name);
        engine.run_line(&format!("export bundle \"{}\"", dir.display())).unwrap();
        let said = engine.log().iter().next_back().unwrap().message.clone();
        (files_of(&dir), said)
    };
    // With the comparison on: the manifest and the log line both say whose reading this is.
    let (compared, said) = export(&mut engine, "compared");
    assert!(
        said.contains("of the scene's own instrument: the comparison with AICE 100 m is not in it"),
        "{said}"
    );
    let manifest: serde_json::Value = serde_json::from_slice(&compared["manifest.json"]).unwrap();
    let note = manifest["comparison"].as_str().expect("a note on the comparison");
    assert!(note.contains("the scene's own instruments alone") && note.contains("on AICE 100 m"), "{note}");
    // The numbers are the scene's own instrument's: AION-10's, not AICE's.
    assert_eq!(
        manifest["spec"]["array"]["instrument"],
        serde_json::to_value(&engine.spec().array.instrument).unwrap()
    );
    let (_, own) = engine.signal();
    let (_, other) = engine.compared_signal();
    let first = |bytes: &[u8]| cav_core::npy::parse(bytes).unwrap().into_f64().unwrap().1[0];
    let gradiometer = cav_core::forward::Channel::Gradiometer;
    assert_eq!(first(&compared["signal.npy"]).to_bits(), own.unwrap().channel(0, gradiometer)[0].to_bits());
    assert_ne!(first(&compared["signal.npy"]).to_bits(), other.unwrap().channel(0, gradiometer)[0].to_bits());
    // Control: with the comparison off the same scene's bundle has every array the same, a
    // manifest that says `null`, and a log line that says nothing of a comparison.
    engine.run_line("compare off").unwrap();
    let (plain, said) = export(&mut engine, "plain");
    assert!(said.ends_with("(18 files)"), "{said}");
    let manifest: serde_json::Value = serde_json::from_slice(&plain["manifest.json"]).unwrap();
    assert!(manifest["comparison"].is_null());
    for (name, bytes) in &plain {
        assert_eq!(*bytes == compared[name], name != "manifest.json", "{name}");
    }
}
