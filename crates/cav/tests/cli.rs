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
    assert_eq!(names.len(), 16);
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
    let file = include_str!("../../cav-app/assets/fonts/UFL.txt").replace("\r\n", "\n");
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
