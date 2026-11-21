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
    assert_eq!(names, ["room", "void", "infinite-floor"]);
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
fn cli_refuses_an_unknown_scene() {
    // No GPU is needed to find out that the scene does not exist: it fails before a device is made.
    let path = scratch("never-written.png");
    let out = cav(&["render", "no-such-scene", "--out", path.to_str().unwrap()]);
    assert!(!out.status.success());
    let why = stderr(&out);
    assert!(why.contains("no-such-scene") && why.contains("room, void, infinite-floor"), "{why}");
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
