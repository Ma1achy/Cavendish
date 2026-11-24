//! The `cav` binary, run as a person or a script would run it. Tests with `gpu_` in their names
//! need the local adapter and belong to `cargo xtask gpu`.

use std::path::PathBuf;
use std::process::{Command, Output};

fn cav(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cav")).args(args).output().expect("cav runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A fresh file path under the test's temporary directory.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    path
}

#[test]
fn cli_lists_the_dev_scenes() {
    let out = cav(&["scenes"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    let names: Vec<&str> = text.lines().filter_map(|l| l.split_whitespace().next()).collect();
    assert_eq!(names[..5], ["room", "void", "infinite-floor", "point", "pass"]);
    // Every line is a name, a gap and a path: the longest name (17 letters) once ran into its path.
    assert_eq!(names.len(), 20);
    assert!(names.contains(&"through-the-tower") && text.contains("through-the-tower  scenes/dev/"));
    assert!(text.contains("scenes/dev/void.ron") && text.contains("scenes/dev/infinite-floor.ron"));
}

#[test]
fn cli_schema_is_the_spec_schema() {
    let out = cav(&["schema"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let printed: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("the schema is JSON");
    assert_eq!(printed, cav_sim::SceneSpec::schema());
    // Control: it is the schema, not just any JSON.
    assert_eq!(printed["title"], "SceneSpec");
    assert_ne!(printed, serde_json::json!({}));
}

#[test]
fn cli_exec_runs_commands_through_the_engine() {
    let out = cav(&["exec", "load void; panes 2; view top @2; select tower 4; seek 3.5; stats"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    // Each command is echoed as the engine describes it; the last answer is the state as JSON.
    assert!(text.starts_with("load void\npanes 2\nview top @2\nselect tower 4\nseek 3.5\n"), "{text}");
    let stats: serde_json::Value = serde_json::from_str(text.split_once("seek 3.5\n").unwrap().1).unwrap();
    assert_eq!(stats["scene"], "void");
    assert_eq!(stats["room"], false);
    assert_eq!(stats["panes"], 2);
    assert_eq!(stats["time"], 3.5);
    assert_eq!(stats["selection"], serde_json::json!({ "Tower": 4 }));
    assert_eq!(stats["journal"], 5);

    // The same session through the library gives the same digest: the CLI is not a second engine.
    let mut engine = cav_engine::Engine::new();
    engine.run_script("load void; panes 2; view top @2; select tower 4; seek 3.5").unwrap();
    assert_eq!(stats["digest"], engine.digest());
    // Control: a different session has a different digest.
    let other = cav(&["exec", "load void; panes 2; view top @2; select tower 5; seek 3.5; stats"]);
    assert!(!stdout(&other).contains(&engine.digest()));
}

#[test]
fn cli_exec_stops_at_a_refused_command_and_fails() {
    let out = cav(&["exec", "panes 2; select tower 9; panes 4"]);
    assert!(!out.status.success());
    assert_eq!(stdout(&out), "panes 2\n", "what ran before the failure is shown");
    let why = stderr(&out);
    assert!(why.contains("`select tower 9`") && why.contains("there is no tower 9"), "{why}");
}

#[test]
fn cli_exec_splits_as_the_console_does() {
    // A scene file whose path holds a space and a `;`: one quoted word, not two commands.
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("exec; my scenes");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("far void.ron");
    let mut scene = cav_sim::dev::void();
    scene.name = "far-void".into();
    std::fs::write(&path, scene.to_ron()).unwrap();
    // The comment line holds a lone quote and a `;`: neither reaches the lines after it.
    let script = format!("# a 4\" pane; not a command\nload \"{}\"; panes 2; stats", path.display());
    let out = cav(&["exec", &script]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    let stats: serde_json::Value = serde_json::from_str(text.split_once("panes 2\n").unwrap().1).unwrap();
    assert_eq!(stats["scene"], "far-void");
    assert_eq!(stats["panes"], 2);
    // Control: without the quotes the `;` cuts the path in two, and the load is refused.
    let bare = format!("load {}; panes 2", path.display());
    let out = cav(&["exec", &bare]);
    assert!(!out.status.success());
    assert_eq!(stdout(&out), "", "nothing ran");
}

#[test]
fn cli_refuses_an_unknown_scene() {
    // No GPU is needed to find out that the scene does not exist: it fails before a device is made.
    let path = scratch("never-written.png");
    let out = cav(&["render", "no-such-scene", "--out", path.to_str().unwrap()]);
    assert!(!out.status.success());
    let why = stderr(&out);
    assert!(why.contains("no-such-scene") && why.contains("room, void, infinite-floor, point"), "{why}");
    assert!(!path.exists(), "nothing is written on failure");
}

#[test]
fn cli_refuses_bad_usage() {
    for (args, expected) in [
        (vec!["frobnicate"], "`frobnicate` is not a command"),
        (vec!["render", "room"], "render needs --out"),
        (vec!["render", "room", "--out", "x.png", "--size", "big"], "is not a size"),
        (vec!["render", "room", "--out", "x.png", "--colour", "red"], "`--colour` is not an option here"),
        (vec!["render", "room", "--out", "x.png", "--view", "up"], "`up` is not a view"),
        (vec!["scenes", "extra"], "expected 0 arguments"),
    ] {
        let out = cav(&args);
        assert!(!out.status.success(), "{args:?} should fail");
        assert!(stderr(&out).contains(expected), "{args:?}: {}", stderr(&out));
    }
    // Control: help succeeds and names every subcommand.
    let help = cav(&["help"]);
    assert!(help.status.success());
    for word in ["doctor", "render", "scenes", "schema", "exec"] {
        assert!(stdout(&help).contains(word));
    }
}

#[test]
fn gpu_doctor_reports_the_adapter_and_self_tests() {
    let out = cav(&["doctor"]);
    let text = stdout(&out);
    assert!(out.status.success(), "{text}\n{}", stderr(&out));
    assert!(text.contains("adapters:\n"), "{text}");
    // Exactly one adapter is marked as the one in use, and the device line names a backend.
    assert_eq!(text.lines().filter(|l| l.starts_with("  * ")).count(), 1, "{text}");
    assert!(
        text.lines().any(|l| l.starts_with("device: `") && (l.contains("Vulkan") || l.contains("Dx12"))),
        "{text}"
    );
    assert!(text.contains("render self-test: ok"), "{text}");
    // Honest about what it does not test.
    assert!(text.contains("precision self-test: none yet"), "{text}");
}

#[test]
fn gpu_render_writes_a_png_of_the_asked_size() {
    let path = scratch("room-top.png");
    let out = cav(&[
        "render",
        "room",
        "--view",
        "top",
        "--proj",
        "ortho",
        "--style",
        "paper",
        "--size",
        "640x400",
        "--out",
        path.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("640 × 400 px"), "{}", stdout(&out));
    let (rgba, size) = cav_render::decode_png(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(size, [640, 400]);
    // Paper: the corner is cream.
    assert_eq!(&rgba[..3], &cav_render::PAPER_SRGB);

    // The options are engine commands: the same picture comes from --exec.
    let other = scratch("room-top-exec.png");
    let out = cav(&[
        "render",
        "room",
        "--size",
        "640x400",
        "--exec",
        "view top; proj ortho; style paper",
        "--out",
        other.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        std::fs::read(&other).unwrap() == std::fs::read(&path).unwrap(),
        "--view/--proj/--style and --exec differ"
    );
    // Control: a different view is a different picture.
    let side = scratch("room-east.png");
    let out = cav(&[
        "render",
        "room",
        "--view",
        "east",
        "--proj",
        "ortho",
        "--style",
        "paper",
        "--size",
        "640x400",
        "--out",
        side.to_str().unwrap(),
    ]);
    assert!(out.status.success());
    assert!(std::fs::read(&side).unwrap() != std::fs::read(&path).unwrap());
    // A scene file works as well as a library name, and a selection is highlighted.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenes/dev/void.ron");
    let void = scratch("void.png");
    let out =
        cav(&["render", root.to_str().unwrap(), "--exec", "select tower 2", "--out", void.to_str().unwrap()]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("scene `void`") && stdout(&out).contains("1280 × 720 px"));
}

/// A copy of the void scene in a folder whose name has spaces; returns its path.
fn scene_in_a_folder_with_spaces(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("my scenes");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.ron"));
    let mut scene = cav_sim::dev::void();
    scene.name = name.to_owned();
    std::fs::write(&path, scene.to_ron()).unwrap();
    path
}

#[test]
fn gpu_render_loads_a_scene_whose_path_has_spaces() {
    let scene = scene_in_a_folder_with_spaces("far void");
    assert!(scene.to_str().unwrap().contains(' '), "the control: the path does have a space");
    let out = scratch("far void.png");
    let done = cav(&["render", scene.to_str().unwrap(), "--size", "320x200", "--out", out.to_str().unwrap()]);
    assert!(done.status.success(), "{}", stderr(&done));
    assert!(stdout(&done).contains("scene `far void`"), "{}", stdout(&done));
    assert_eq!(cav_render::decode_png(&std::fs::read(&out).unwrap()).unwrap().1, [320, 200]);
}

#[test]
fn gpu_named_views_fill_the_picture() {
    // `cav render --view top` used to keep the isometric distance: the room took half the height.
    let out = scratch("plan-flat.png");
    let done = cav(&[
        "render",
        "room",
        "--view",
        "top",
        "--proj",
        "ortho",
        "--style",
        "flat",
        "--size",
        "600x600",
        "--out",
        out.to_str().unwrap(),
    ]);
    assert!(done.status.success(), "{}", stderr(&done));
    let (rgba, _) = cav_render::decode_png(&std::fs::read(&out).unwrap()).unwrap();
    // The rows that hold anything but background: the room's plan, 10 m of a framed 10 m.
    let drawn: Vec<usize> = (0..600)
        .filter(|y| (0..600).any(|x| rgba[(y * 600 + x) * 4..][..3] != cav_render::BACKGROUND_SRGB))
        .collect();
    let height = drawn.last().unwrap() - drawn.first().unwrap() + 1;
    // Framing leaves a margin of 1/1.12 and assumes a picture 1.25 times as wide as high: in a
    // square picture the plan takes 1/(1.12 × 1.25) = 71% of the height. It was 46%.
    let share = height as f64 / 600.0;
    assert!((share - 1.0 / (1.12 * 1.25)).abs() < 0.02, "the plan takes {share:.2} of the picture's height");
}

#[test]
fn cli_prints_the_font_licence() {
    let out = cav(&["licences"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    // The licence itself, whole, as it is in the repository beside the font.
    let file = include_str!("../../cav-plot/assets/fonts/UFL.txt").replace("\r\n", "\n");
    assert!(text.contains("Ubuntu Mono") && text.contains("Ubuntu Font Licence 1.0"), "{text}");
    assert!(text.contains(file.trim()), "the licence text is not printed whole");
    assert!(file.len() > 3000, "the file is the licence, not a stub");
    // The American spelling finds it too; `help` lists it.
    assert_eq!(stdout(&cav(&["licenses"])), text);
    assert!(stdout(&cav(&["help"])).contains("licences"));
    // Control: a word that is not a command is refused.
    assert!(!cav(&["license-plate"]).status.success());
}

#[test]
fn cli_writes_the_fonts_and_their_licence() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli fonts").join("faces");
    let _ = std::fs::remove_dir_all(&dir);
    let out = cav(&["fonts", "--out", dir.to_str().unwrap()]);
    assert!(out.status.success(), "{}", stderr(&out));
    // The folder is made, and each file is the binary's own bytes, named where it was written.
    let files: [(&str, &[u8]); 3] = [
        ("Ubuntu-Light.ttf", cav_plot::fonts::bytes(cav_plot::fonts::Face::Sans)),
        ("UbuntuMono-Regular.ttf", cav_plot::fonts::MONO_FONT),
        ("UFL.txt", cav_plot::fonts::MONO_FONT_LICENCE.as_bytes()),
    ];
    for (name, bytes) in files {
        let path = dir.join(name);
        assert!(std::fs::read(&path).unwrap() == bytes, "{name} is not the binary's own");
        assert!(stdout(&out).contains(&path.display().to_string()), "{}", stdout(&out));
    }
    assert!(cav_plot::fonts::bytes(cav_plot::fonts::Face::Sans) != cav_plot::fonts::MONO_FONT, "two faces");
    assert!(stdout(&cav(&["help"])).contains("fonts --out <dir>"));
    // Control: without --out nothing is written, and the refusal says what is needed.
    let refused = cav(&["fonts"]);
    assert!(!refused.status.success());
    assert!(stderr(&refused).contains("fonts needs --out"), "{}", stderr(&refused));
}

#[test]
fn cli_figure_without_a_title() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli no title");
    std::fs::create_dir_all(&dir).unwrap();
    let title = |name: &str, extra: &[&str]| -> serde_json::Value {
        let out = dir.join(format!("{name}.svg"));
        let mut args = vec!["figure", "comb", "periodogram", "--out", out.to_str().unwrap()];
        args.extend_from_slice(extra);
        let done = cav(&args);
        assert!(done.status.success(), "{}", stderr(&done));
        let record: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(out.with_extension("json")).unwrap()).unwrap();
        record["files"][0]["title"].clone()
    };
    assert_eq!(title("untitled", &["--no-title"]), serde_json::Value::Null);
    // Control: without the flag the picture has its title.
    assert!(title("titled", &[]).is_string());
    // A flag takes no value.
    let refused = cav(&["figure", "comb", "periodogram", "--out", "x.svg", "--no-title=yes"]);
    assert!(!refused.status.success() && stderr(&refused).contains("takes no value"), "{}", stderr(&refused));
}

#[test]
fn cli_get_set_ls_edit_a_file() {
    // A scene file of our own: the comb, saved by the library's own writer.
    let file = scratch("edited comb.ron");
    let scene = cav_sim::load("comb").unwrap();
    scene.save(file.to_str().unwrap()).unwrap();
    let path = file.to_str().unwrap();
    // get: the whole scene, or the value at a path, as JSON.
    let mass = cav(&["get", path, "bodies[0].mass"]);
    assert!(mass.status.success(), "{}", stderr(&mass));
    assert_eq!(stdout(&mass).trim(), serde_json::to_string(&scene.bodies[0].mass).unwrap());
    let whole: serde_json::Value = serde_json::from_str(&stdout(&cav(&["get", path]))).unwrap();
    assert_eq!(whole, scene.to_value());
    // ls: the children with their units and kinds, in the order they are declared.
    let listed = stdout(&cav(&["ls", path, "bodies[0]"]));
    let names: Vec<&str> = listed.lines().filter_map(|line| line.split_whitespace().next()).collect();
    assert_eq!(names, ["label", "mass", "radius", "motion", "appears", "vanishes"], "{listed}");
    assert!(listed.contains("mass  =  ") && listed.contains(" kg  (number"), "{listed}");
    assert!(stdout(&cav(&["ls", "room"])).lines().count() == 8, "a library scene lists too");
    // set: the file is the edited scene, and nothing else in it has changed.
    let set = cav(&["set", path, "bodies[0].mass", "42.5"]);
    assert!(set.status.success(), "{}", stderr(&set));
    assert!(stdout(&set).starts_with("set bodies[0].mass 42.5\nsaved "), "{}", stdout(&set));
    let mut expected = scene.clone();
    expected.bodies[0].mass = 42.5;
    assert_eq!(cav_sim::load(path).unwrap(), expected);
    // A bare word is text; JSON is JSON; --out leaves the source alone.
    let other = scratch("renamed.ron");
    let renamed = cav(&["set", path, "name", "another", "--out", other.to_str().unwrap()]);
    assert!(renamed.status.success(), "{}", stderr(&renamed));
    assert_eq!(cav_sim::load(other.to_str().unwrap()).unwrap().name, "another");
    assert_eq!(cav_sim::load(path).unwrap(), expected, "the source is untouched");
    let room = cav(&["set", path, "room", r#"{"origin": [-5, -5, 0], "size": [10, 10, 3]}"#]);
    assert!(room.status.success(), "{}", stderr(&room));
    assert!(cav_sim::load(path).unwrap().room.is_some());
    // The schema at a path, with no scene at all.
    let schema: serde_json::Value =
        serde_json::from_str(&stdout(&cav(&["schema", "bodies[0].mass"]))).unwrap();
    assert_eq!(
        (&schema["x-unit"], &schema["type"]),
        (&serde_json::json!("kg"), &serde_json::json!("number"))
    );
    // Control: refusals change no file and say why. A value out of range, a path that is not
    // there, a library scene with nowhere to go.
    let before = std::fs::read(&file).unwrap();
    for (args, why) in [
        (vec!["set", path, "bodies[0].mass", "-1"], "bodies[0].mass"),
        (vec!["set", path, "bodies[0].weight", "1"], "has no field `weight`"),
        (vec!["set", "comb", "bodies[0].mass", "1"], "--out"),
        (vec!["get", path, "bodies[7]"], "has 1 items"),
        (vec!["ls", "no-such-scene"], "not a library scene"),
    ] {
        let out = cav(&args);
        assert!(!out.status.success(), "{args:?} was accepted");
        assert!(stderr(&out).contains(why), "{args:?}: {}", stderr(&out));
    }
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn cli_replays_a_saved_session() {
    let journal = scratch("session to replay.json");
    let scene = scratch("replayed scene.ron");
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("replay out");
    let _ = std::fs::remove_dir_all(&out);
    // A session through `cav exec`: edits, a save, the journal saved.
    let script = format!(
        "load comb; set bodies[0].mass 42.5; add array.towers {{\"label\": \"tower 6\", \"position\": [6, 0, 0]}}; \
         save \"{}\"; set duration 300; journal save \"{}\"; stats",
        scene.display(),
        journal.display()
    );
    let session = cav(&["exec", &script]);
    assert!(session.status.success(), "{}", stderr(&session));
    let text = stdout(&session);
    let stats: serde_json::Value = serde_json::from_str(&text[text.rfind("\n{").unwrap()..]).unwrap();
    let saved = std::fs::read(&scene).unwrap();
    std::fs::remove_file(&scene).unwrap();
    // Replayed with somewhere to write: the same scene file, byte for byte, under its own name.
    let replayed = cav(&["replay", journal.to_str().unwrap(), "--out", out.to_str().unwrap()]);
    assert!(replayed.status.success(), "{}", stderr(&replayed));
    let said = stdout(&replayed);
    assert!(said.starts_with("replayed 5 command(s) · scene `comb`"), "{said}");
    assert!(said.contains("1 export(s) written into"), "{said}");
    assert_eq!(std::fs::read(out.join("replayed scene.ron")).unwrap(), saved);
    assert!(!scene.exists(), "the replay wrote where the journal said");
    // The session went one command further (the journal's own save), which changes no state:
    // the digests agree.
    assert!(said.contains(stats["digest"].as_str().unwrap()), "{said} against {}", stats["digest"]);
    // Without --out nothing is written at all.
    let plain = cav(&["replay", journal.to_str().unwrap()]);
    assert!(stdout(&plain).contains("not written (no --out)") && !scene.exists());
    // Control: what is not a journal is refused.
    let wrong = cav(&["replay", out.join("replayed scene.ron").to_str().unwrap()]);
    assert!(!wrong.status.success() && stderr(&wrong).contains("not a journal"), "{}", stderr(&wrong));
}

#[test]
fn cli_replay_with_nowhere_to_write_computes_no_series() {
    use cav_engine::{Command as Edit, Engine, ExportKind, SignalState};
    // A journal of a long series (two hundred thousand shots of five towers) that exported a
    // bundle. Replayed with no --out nothing is written, so nothing needs the series: the
    // replay says what state it reached and that the signal was not waited for. (S2's review:
    // it ended with a wait of up to ten minutes for a signal nobody had asked for.)
    let journal = vec![
        Edit::load(cav_sim::load("comb").unwrap()),
        Edit::set("duration", serde_json::json!(400_000.0)),
        Edit::Export { what: ExportKind::Bundle, to: "bundle".into() },
        Edit::Seek(12.5),
    ];
    let file = scratch("a long series.json");
    std::fs::write(&file, cav_engine::journal::write(&journal)).unwrap();
    let done = cav(&["replay", file.to_str().unwrap()]);
    assert!(done.status.success(), "{}", stderr(&done));
    let said = stdout(&done);
    // The state it reached is the state of the journal, which does not depend on the signal.
    let expected = Engine::replay(&journal).unwrap().digest();
    assert!(
        said.starts_with(&format!("replayed 4 command(s) · scene `comb` · state {expected} · ")),
        "{said}"
    );
    assert!(said.contains("1 export(s) not written (no --out)"), "{said}");
    // What is printed says the series was not computed: the signal is still the stale one. Had
    // the replay waited, as it did, this would read `signal refined`.
    assert!(said.trim_end().ends_with("signal stale, not waited for: nothing written needed it"), "{said}");

    // The digest does not depend on the signal: the same state with the workers held (no signal)
    // and after they have run (a refined one) has the same digest. Control: the signal's
    // standing did change between the two, and a short journal replayed with somewhere to
    // write does end with a refined signal, since its export waited for one.
    let short = [journal[0].clone(), Edit::set("duration", serde_json::json!(60.0)), journal[2].clone()];
    let mut engine = Engine::new();
    engine.hold_workers(true);
    for command in &short[..2] {
        engine.apply(command.clone()).unwrap();
    }
    let (held, digest) = (engine.signal().0, engine.digest());
    engine.hold_workers(false);
    assert!(engine.settle(std::time::Duration::from_secs(120)));
    assert_eq!((held, engine.signal().0), (SignalState::Stale, SignalState::Refined));
    assert_eq!(engine.digest(), digest);
    let file = scratch("a short series.json");
    std::fs::write(&file, cav_engine::journal::write(&short)).unwrap();
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("replay short out");
    let _ = std::fs::remove_dir_all(&out);
    let done = cav(&["replay", file.to_str().unwrap(), "--out", out.to_str().unwrap()]);
    assert!(stdout(&done).trim_end().ends_with("signal refined"), "{}{}", stdout(&done), stderr(&done));
    assert!(out.join("bundle").join("manifest.json").exists());
}

#[test]
fn cli_run_writes_a_bundle_twice_the_same() {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli run");
    let _ = std::fs::remove_dir_all(&root);
    let (a, b) = (root.join("a"), root.join("b"));
    let run = |out: &PathBuf| {
        let done = cav(&[
            "run",
            "staggered",
            "--out",
            out.to_str().unwrap(),
            "--exec",
            "set duration 60; set array.schedule.loss 0.1",
        ]);
        assert!(done.status.success(), "{}", stderr(&done));
        stdout(&done)
    };
    let said = run(&a);
    assert!(said.contains("scene `staggered`, 5 tower(s), 150 shot(s) in all, quadrature within"), "{said}");
    run(&b);
    // The same command twice: the same eighteen files, byte for byte.
    let names = |dir: &PathBuf| {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    assert_eq!(names(&a).len(), 18);
    assert_eq!(names(&a), names(&b));
    for name in names(&a) {
        assert_eq!(std::fs::read(a.join(&name)).unwrap(), std::fs::read(b.join(&name)).unwrap(), "{name}");
    }
    // The bundle's own scene, run again, is the bundle again: it carries what regenerates it.
    let c = root.join("c");
    let again = cav(&["run", a.join("scene.ron").to_str().unwrap(), "--out", c.to_str().unwrap()]);
    assert!(again.status.success(), "{}", stderr(&again));
    for name in ["signal.npy", "detector_time.npy", "valid.npy", "signal_per_ifo.npy", "error_estimate.npy"] {
        assert_eq!(std::fs::read(a.join(name)).unwrap(), std::fs::read(c.join(name)).unwrap(), "{name}");
    }
    // A preset is an edit before the run: another instrument, another bundle.
    let d = root.join("d");
    let aice =
        cav(&["run", "comb", "--preset", "aice", "--out", d.to_str().unwrap(), "--exec", "set duration 100"]);
    assert!(aice.status.success(), "{}", stderr(&aice));
    assert!(stdout(&aice).contains("5 tower(s), 100 shot(s) in all"), "{}", stdout(&aice));
    // Control: refusals. No --out; a scene that is not one; a patience that is not a number.
    assert!(stderr(&cav(&["run", "comb"])).contains("--out"));
    assert!(!cav(&["run", "nowhere", "--out", d.to_str().unwrap()]).status.success());
    assert!(
        stderr(&cav(&["run", "comb", "--out", d.to_str().unwrap(), "--patience", "soon"]))
            .contains("not a patience")
    );
}

/// A file in the repository that git does not know, removed when this is dropped: also when the
/// test fails.
struct Untracked(PathBuf);

impl Untracked {
    fn new(path: PathBuf) -> Self {
        std::fs::write(&path, "made by a test of `cav`; safe to delete\n").unwrap();
        Self(path)
    }
}

impl Drop for Untracked {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Every file under `dir`, by its path within it, with its bytes.
fn files_under(dir: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
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

#[test]
fn cli_provenance_is_the_builds_whatever_the_tree_or_the_directory() {
    // The revision and the dirty flag in a manifest and in a figure's .json are the build's.
    // The same binary, run before and after the working tree changes and from another
    // directory, writes the same bytes. (S2's review, I3: they were asked of git at run time,
    // so one untracked file between two runs changed thirteen files of a replay. Inside one
    // process the old code asked once and kept the answer, so only separate processes show it.)
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli provenance");
    let _ = std::fs::remove_dir_all(&root);
    // A PATH with nothing on it: a process given it has no git to ask.
    let nowhere = root.join("a PATH with nothing on it");
    std::fs::create_dir_all(&nowhere).unwrap();
    let write = |name: &str, from: Option<&std::path::Path>, path: Option<&std::path::Path>| {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let (bundle, figure) = (dir.join("bundle"), dir.join("p.svg"));
        for args in [
            vec!["run", "staggered", "--out", bundle.to_str().unwrap(), "--exec", "set duration 60"],
            vec![
                "figure",
                "comb",
                "periodogram",
                "--out",
                figure.to_str().unwrap(),
                "--exec",
                "set duration 200",
            ],
        ] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_cav"));
            command.args(&args);
            if let Some(directory) = from {
                command.current_dir(directory);
            }
            if let Some(path) = path {
                command.env("PATH", path);
            }
            let done = command.output().expect("cav runs");
            assert!(done.status.success(), "{args:?}: {}", stderr(&done));
        }
        files_under(&dir)
    };
    // Whether a process with this PATH (its own, if none is given) can run git: asked through
    // the platform's shell, which looks a program up as any process would.
    let can_run_git = |path: Option<&std::path::Path>| {
        let mut shell = if cfg!(windows) { Command::new("cmd") } else { Command::new("/bin/sh") };
        shell.args([if cfg!(windows) { "/C" } else { "-c" }, "git --version"]);
        if let Some(path) = path {
            shell.env("PATH", path);
        }
        shell.output().is_ok_and(|done| done.status.success())
    };
    // What git says of the tree, asked without letting it rewrite its index.
    let status = || {
        Command::new("git")
            .args(["--no-optional-locks", "status", "--porcelain"])
            .current_dir(&repository)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| output.stdout)
    };

    let before = status();
    let first = write("first", None, None);
    // A file git does not know, in the repository's root, under a name no ignore rule covers.
    let made =
        Untracked::new(repository.join(format!("untracked-by-a-test-of-cav-{}.txt", std::process::id())));
    let between = status();
    let second = write("second", None, None);
    // From another directory, outside the repository, with the tree still changed.
    let third = write("third", Some(&std::env::temp_dir()), None);
    drop(made);
    // And with no git to ask: whatever the tree is like, a binary that asked git when it wrote
    // would now write `unknown`.
    let fourth = write("fourth", None, Some(&nowhere));

    // A bundle of eighteen files and a figure of three, each time; every byte the same.
    assert_eq!(first.len(), 21, "{:?}", first.keys().collect::<Vec<_>>());
    for (name, files) in [
        ("after an untracked file appeared", &second),
        ("from another directory", &third),
        ("with no git on the PATH", &fourth),
    ] {
        assert_eq!(files.keys().collect::<Vec<_>>(), first.keys().collect::<Vec<_>>(), "{name}");
        for (file, bytes) in files {
            assert!(*bytes == first[file], "{file} differs {name}");
        }
    }
    // The provenance is there, in both, is the same block, and says it is the build's.
    let manifest: serde_json::Value = serde_json::from_slice(&first["bundle/manifest.json"]).unwrap();
    let figure: serde_json::Value = serde_json::from_slice(&first["p.json"]).unwrap();
    let engine = &manifest["numerics"]["engine"];
    assert_eq!(engine, &figure["engine"]);
    assert_eq!(engine, &cav_engine::build().json(), "the binary's build is this test's");
    assert!(engine["built_from"].as_str().unwrap().contains("fixed at compile time"), "{engine}");
    assert!(engine["revision"].is_string() && (engine["dirty"].is_boolean() || engine["dirty"] == "unknown"));

    // Controls: the things that were changed did change, so a binary that asked git when it
    // wrote had something else to be told. What git says of the tree is not what it was (from a
    // tree with nothing uncommitted, `dirty` went from false to true: the review's case; from
    // one already dirty it stays true, which is why the fourth run is here as well). And a
    // process given the empty PATH cannot run git, where one with the ordinary PATH can. Where
    // there is no git or no repository there is nothing to compare, and that is said.
    match (before, between) {
        (Some(before), Some(between)) => {
            assert_ne!(before, between, "the untracked file did not change `git status`");
            assert!(String::from_utf8_lossy(&between).contains("untracked-by-a-test-of-cav-"));
        }
        _ => eprintln!("git or the repository is not there: the control (git status changed) was not run"),
    }
    if can_run_git(None) {
        assert!(!can_run_git(Some(&nowhere)), "git was still found with nothing on the PATH");
    } else {
        eprintln!("git is not installed: the control (no git on an empty PATH) was not run");
    }
    assert!(!repository.join(format!("untracked-by-a-test-of-cav-{}.txt", std::process::id())).exists());
}

#[test]
fn gpu_cli_figure_takes_a_picture_wider_than_mcp_does() {
    // A16: a figure's picture is at most 8192 pixels a side at the command line, 4096 over MCP
    // (`mcp_figure_writes_a_figure_and_says_what_it_wrote` has the refusal of 4097 there).
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli figure wide");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("wide.png");
    // 1041.4 mm at 100 pixels per inch is 4100 pixels.
    let wide = |dpi: &str| {
        let out = out.to_str().unwrap();
        let size = "1041.4x60";
        cav(&["figure", "comb", "signal", "--out", out, "--size", size, "--dpi", dpi, "--preset", "slide"])
    };
    let done = wide("100");
    assert!(done.status.success(), "{}", stderr(&done));
    let (_, size) = cav_render::decode_png(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!(size, [4100, 236]);
    // Control: past 8192 it is refused here too, with the numbers, and the file is left as it was.
    let before = std::fs::read(&out).unwrap();
    let done = wide("200");
    assert!(!done.status.success());
    assert!(stderr(&done).contains("8200 × 472 pixels; each side is 1 to 8192"), "{}", stderr(&done));
    assert_eq!(std::fs::read(&out).unwrap(), before);
}

#[test]
fn cli_figure_writes_a_figure_with_its_data_twice_the_same() {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli figure");
    let _ = std::fs::remove_dir_all(&root);
    let (a, b) = (root.join("a"), root.join("b"));
    let run = |dir: &PathBuf, file: &str, more: &[&str]| {
        std::fs::create_dir_all(dir).unwrap();
        let out = dir.join(file);
        let mut args = vec!["figure", "comb", "periodogram", "--out", out.to_str().unwrap()];
        args.extend_from_slice(more);
        let done = cav(&args);
        assert!(done.status.success(), "{}", stderr(&done));
        stdout(&done)
    };
    let options = ["--tower", "2", "--preset", "paper-full", "--exec", "set duration 200"];
    let said = run(&a, "p.svg", &options);
    assert!(said.contains("periodogram of scene `comb`, preset paper-full"), "{said}");
    assert!(said.contains("p.csv") && said.contains("p.json"), "{said}");
    run(&b, "p.svg", &options);
    run(&a, "q.pdf", &["--size", "120x70", "--span", "0,0.2"]);
    run(&b, "q.pdf", &["--size", "120x70", "--span", "0,0.2"]);
    // The same commands twice: the same six files, byte for byte.
    let mut names: Vec<String> = std::fs::read_dir(&a)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["p.csv", "p.json", "p.svg", "q.csv", "q.json", "q.pdf"]);
    for name in &names {
        assert_eq!(std::fs::read(a.join(name)).unwrap(), std::fs::read(b.join(name)).unwrap(), "{name}");
    }
    // The files are what they say: an SVG of the preset's width, a PDF of the size asked for.
    let svg = std::fs::read_to_string(a.join("p.svg")).unwrap();
    assert!(svg.contains("width=\"178mm\" height=\"90mm\""), "{}", &svg[..200]);
    let pdf = std::fs::read(a.join("q.pdf")).unwrap();
    assert_eq!(cav_plot::pdf::check(&pdf).unwrap().media_box, [0.0, 0.0, 340.157, 198.425]);
    // The provenance carries the line that made it, with the edits a library name does not say.
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(a.join("p.json")).unwrap()).unwrap();
    assert_eq!(json["files"][0]["command"]["console"], "figure periodogram tower 2 as paper-full to p.svg");
    assert_eq!(json["scene"]["spec"]["duration"], 200.0);
    // Control: refusals, each with its reason, and nothing written.
    let c = root.join("c");
    std::fs::create_dir_all(&c).unwrap();
    let out = c.join("x.svg");
    let out = out.to_str().unwrap();
    for (args, why) in [
        (vec!["figure", "comb", "periodogram"], "--out"),
        (vec!["figure", "comb", "periodogram", "--out", "x.jpg"], "names no format"),
        (vec!["figure", "comb", "spectrum", "--out", out], "is not something a figure shows"),
        (vec!["figure", "comb", "periodogram", "--out", out, "--preset", "a4"], "is not a figure preset"),
        (vec!["figure", "comb", "periodogram", "--out", out, "--tower", "9"], "there is no tower 9"),
        (vec!["figure", "comb", "periodogram", "--out", out, "--size", "wide"], "is not a size"),
        (vec!["figure", "nowhere", "periodogram", "--out", out], "not a library scene"),
    ] {
        let done = cav(&args);
        assert!(!done.status.success() && stderr(&done).contains(why), "{args:?}: {}", stderr(&done));
    }
    assert_eq!(std::fs::read_dir(&c).unwrap().count(), 0);
    assert!(stdout(&cav(&["help"])).contains("figure <scene> <signal|periodogram|cadence|flight|view>"));
}

#[test]
fn cli_replay_writes_the_bundle_and_the_figure_a_session_exported() {
    // A session at the command line exports a bundle and a figure and saves its journal; the
    // replay writes both again, into its own directory, the same bytes. (Found by the S2 demo:
    // the replay refused the bundle after a tenth of a second, and could not draw.)
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli-replay-exports");
    let _ = std::fs::remove_dir_all(&root);
    let (first, again) = (root.join("first"), root.join("again"));
    std::fs::create_dir_all(&first).unwrap();
    let at = |name: &str| first.join(name).to_string_lossy().replace('\\', "/");
    let script = format!(
        "load comb; set duration 4000; export bundle {}; figure periodogram tower 2 as paper-column to {}; \
         journal save {}",
        at("bundle"),
        at("fig.svg"),
        at("session.json")
    );
    let done = cav(&["exec", &script]);
    assert!(done.status.success(), "{}", stderr(&done));
    let journal = at("session.json");
    let done = cav(&["replay", &journal, "--out", again.to_str().unwrap()]);
    assert!(done.status.success(), "{}", stderr(&done));
    assert!(stdout(&done).contains("2 export(s) written into"), "{}", stdout(&done));
    for name in ["bundle/signal.npy", "bundle/manifest.json", "fig.svg", "fig.csv", "fig.json"] {
        let (a, b) = (std::fs::read(first.join(name)).unwrap(), std::fs::read(again.join(name)).unwrap());
        assert!(!a.is_empty() && a == b, "{name} differs between the session and its replay");
    }
    // Control: with no patience the bundle's signal is not there yet, the replay says which
    // command it stopped at, and nothing of it is written. Without --out nothing is written at all.
    let none = root.join("none");
    let done = cav(&["replay", &journal, "--out", none.to_str().unwrap(), "--patience", "0"]);
    assert!(!done.status.success());
    assert!(
        stderr(&done).contains("command 2") && stderr(&done).contains("still being computed"),
        "{}",
        stderr(&done)
    );
    assert!(!none.join("bundle").join("signal.npy").exists() && !none.join("fig.svg").exists());
    let done = cav(&["replay", &journal]);
    assert!(done.status.success() && stdout(&done).contains("not written (no --out)"), "{}", stdout(&done));
}

#[test]
fn cli_runs_a_scene_with_no_towers_and_a_mixed_array() {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli no towers");
    let _ = std::fs::remove_dir_all(&root);
    // No towers: a bundle of empty arrays, the tracks still there.
    let none = root.join("none");
    let done = cav(&["run", "no-towers", "--out", none.to_str().unwrap()]);
    assert!(done.status.success(), "{}", stderr(&done));
    assert!(stdout(&done).contains("0 tower(s), 0 shot(s) in all"), "{}", stdout(&done));
    let manifest = std::fs::read_to_string(none.join("manifest.json")).unwrap();
    assert!(manifest.contains("\"towers\": 0"), "{manifest}");
    // A mixed array: each tower its own shots, the bundle ragged.
    let mixed = root.join("mixed");
    let done = cav(&["run", "mixed-array", "--out", mixed.to_str().unwrap(), "--exec", "set duration 20"]);
    assert!(done.status.success(), "{}", stderr(&done));
    let manifest = std::fs::read_to_string(mixed.join("manifest.json")).unwrap();
    assert!(manifest.contains("\"shots_per_tower\": [\n    10,\n    4,\n    10\n  ]"), "{manifest}");
    // `get`, `set` and `ls` reach a tower's own instrument.
    let file = scratch("mixed.ron");
    let spec = cav(&["get", "mixed-array", "array.towers[1].instrument.half_sequence"]);
    assert_eq!(stdout(&spec).trim(), "2.5", "{}", stderr(&spec));
    std::fs::copy(mixed.join("scene.ron"), &file).unwrap();
    let set = cav(&["set", file.to_str().unwrap(), "array.towers[0].instrument", "{\"half_sequence\": 0.9}"]);
    assert!(set.status.success(), "{}", stderr(&set));
    let got = cav(&["get", file.to_str().unwrap(), "array.towers[0].instrument.half_sequence"]);
    assert_eq!(stdout(&got).trim(), "0.9");
    // Control: an override that cannot fly is refused, naming the tower.
    let bad = cav(&["set", file.to_str().unwrap(), "array.towers[0].instrument.cycle", "1"]);
    assert!(
        !bad.status.success() && stderr(&bad).contains("array.towers[0].instrument.cycle"),
        "{}",
        stderr(&bad)
    );
}
