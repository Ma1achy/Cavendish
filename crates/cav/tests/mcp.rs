//! `cav mcp` spoken to as an agent's client would: JSON-RPC over the process's stdin and stdout.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

struct Client {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Client {
    /// Starts the server and completes the MCP handshake.
    fn start() -> (Self, Value) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_cav"))
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("cav mcp starts");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut client = Self { child, stdin, stdout, next_id: 1 };
        let hello = client.request(
            "initialize",
            json!({ "protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": { "name": "cav-test", "version": "0" } }),
        );
        client.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        (client, hello)
    }

    fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().expect("the session is open");
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    }

    /// Sends a request and returns the whole response with the matching id.
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).unwrap();
            assert!(read > 0, "the server closed its output while `{method}` was waiting");
            // Standard output is the protocol: every line must be a JSON-RPC message.
            let message: Value =
                serde_json::from_str(&line).unwrap_or_else(|e| panic!("not JSON on stdout: {e}: {line}"));
            assert_eq!(message["jsonrpc"], "2.0", "{line}");
            if message["id"] == id {
                return message;
            }
        }
    }

    /// Calls a tool; returns `(is_error, content blocks)`.
    fn call(&mut self, tool: &str, arguments: Value) -> (bool, Vec<Value>) {
        let response = self.request("tools/call", json!({ "name": tool, "arguments": arguments }));
        let result = &response["result"];
        assert!(result.is_object(), "`{tool}` got no result: {response}");
        (result["isError"] == true, result["content"].as_array().cloned().unwrap_or_default())
    }

    /// Calls a tool that answers with text and must succeed.
    fn text(&mut self, tool: &str, arguments: Value) -> String {
        let (is_error, content) = self.call(tool, arguments);
        assert!(!is_error, "`{tool}` failed: {content:?}");
        content[0]["text"].as_str().expect("a text block").to_owned()
    }

    /// Closes the session and waits for the server to exit cleanly.
    fn finish(mut self) {
        drop(self.stdin.take());
        let status = self.child.wait().unwrap();
        assert!(status.success(), "cav mcp exited with {status}");
    }
}

#[test]
fn mcp_answers_scene_schema() {
    let (mut client, hello) = Client::start();
    assert_eq!(hello["result"]["serverInfo"]["name"], "cavendish");
    assert!(hello["result"]["capabilities"]["tools"].is_object(), "{hello}");

    let schema: Value =
        serde_json::from_str(&client.text("scene_schema", json!({}))).expect("the schema is JSON");
    assert_eq!(schema, cav_sim::SceneSpec::schema(), "the tool answers with the spec's own schema");
    assert_eq!(schema["properties"]["duration"]["description"], "Length of the timeline, seconds.");
    // Control: the comparison is against the real schema, which is not empty and not another type's.
    assert_eq!(schema["title"], "SceneSpec");
    assert!(schema["properties"].as_object().unwrap().len() >= 7);
    client.finish();
}

#[test]
fn mcp_lists_its_tools_with_their_arguments() {
    let (mut client, _) = Client::start();
    let listed = client.request("tools/list", json!({}));
    let tools = listed["result"]["tools"].as_array().expect("a list of tools");
    let mut names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    names.sort_unstable();
    assert_eq!(names, ["command", "log", "render_view", "scene_get", "scene_load", "scene_schema"]);
    for tool in tools {
        assert!(tool["description"].as_str().is_some_and(|d| d.len() > 20), "{tool}");
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
    }
    // Arguments are documented from the doc comments.
    let load = tools.iter().find(|t| t["name"] == "scene_load").unwrap();
    assert!(
        load["inputSchema"]["properties"]["scene"]["description"]
            .as_str()
            .unwrap()
            .contains("library scene name")
    );
    assert_eq!(load["inputSchema"]["required"], json!(["scene"]));
    client.finish();
}

#[test]
fn mcp_tools_drive_the_engine() {
    let (mut client, _) = Client::start();
    // The session starts on the default scene.
    assert_eq!(client.text("scene_get", json!({ "path": "name" })), "\"room\"");

    let stats: Value =
        serde_json::from_str(&client.text("scene_load", json!({ "scene": "infinite-floor" }))).unwrap();
    assert_eq!(
        (stats["scene"].as_str(), stats["floor"].as_bool(), stats["room"].as_bool()),
        (Some("infinite-floor"), Some(true), Some(false))
    );
    let floor: Value = serde_json::from_str(&client.text("scene_get", json!({ "path": "floor" }))).unwrap();
    assert_eq!(floor, json!({ "height": 0.0, "kind": "HalfSpace", "density": 2400.0 }));
    let whole: Value = serde_json::from_str(&client.text("scene_get", json!({}))).unwrap();
    assert_eq!(whole["array"]["towers"].as_array().unwrap().len(), 5);

    // Commands are the console's; the state persists between calls, as one session.
    assert_eq!(
        client.text("command", json!({ "line": "panes 2; view top @2; select tower 3; seek 1.25" })),
        "panes 2\nview top @2\nselect tower 3\nseek 1.25"
    );
    let stats: Value = serde_json::from_str(&client.text("command", json!({ "line": "stats" }))).unwrap();
    assert_eq!(
        (stats["panes"].as_u64(), stats["time"].as_f64(), stats["journal"].as_u64()),
        (Some(2), Some(1.25), Some(5))
    );
    // The same session through the library reaches the same state: MCP is not a second engine.
    let mut engine = cav_engine::Engine::new();
    engine.run_script("load infinite-floor; panes 2; view top @2; select tower 3; seek 1.25").unwrap();
    assert_eq!(stats["digest"], engine.digest());

    // Refusals come back as tool errors that say why, change nothing, and are in the log.
    let (is_error, content) = client.call("command", json!({ "line": "select tower 9" }));
    assert!(is_error && content[0]["text"].as_str().unwrap().contains("there is no tower 9"), "{content:?}");
    let (is_error, content) = client.call("scene_load", json!({ "scene": "nowhere" }));
    assert!(is_error && content[0]["text"].as_str().unwrap().contains("not a library scene"), "{content:?}");
    let (is_error, content) = client.call("scene_get", json!({ "path": "array.tower" }));
    assert!(is_error && content[0]["text"].as_str().unwrap().contains("has no field `tower`"), "{content:?}");
    let after: Value = serde_json::from_str(&client.text("command", json!({ "line": "stats" }))).unwrap();
    assert_eq!(after["digest"], stats["digest"], "the refused calls changed nothing");
    let errors = client.text("log", json!({ "level": "error" }));
    assert_eq!(
        errors.lines().count(),
        2,
        "the two refused commands are logged (the failed query is not a command): {errors}"
    );
    assert!(errors.contains("there is no tower 9"));
    let (is_error, _) = client.call("log", json!({ "level": "loud" }));
    assert!(is_error);

    // A tool that does not exist is a protocol error, not a silent success.
    let unknown = client.request("tools/call", json!({ "name": "frobnicate", "arguments": {} }));
    assert!(unknown["error"].is_object() || unknown["result"]["isError"] == true, "{unknown}");
    client.finish();
}

#[test]
fn gpu_mcp_render_view_returns_a_png() {
    use base64::Engine as _;
    let (mut client, _) = Client::start();
    client.text("command", json!({ "line": "style flat; view top; proj ortho" }));
    let picture = |client: &mut Client, arguments: Value| {
        let (is_error, content) = client.call("render_view", arguments);
        assert!(!is_error, "{content:?}");
        assert_eq!(content[0]["type"], "image");
        assert_eq!(content[0]["mimeType"], "image/png");
        let bytes =
            base64::engine::general_purpose::STANDARD.decode(content[0]["data"].as_str().unwrap()).unwrap();
        cav_render::decode_png(&bytes).expect("a PNG")
    };
    let (rgba, size) = picture(&mut client, json!({ "width": 400, "height": 300 }));
    assert_eq!(size, [400, 300]);
    // From above, orthographic: tower 1 stands at the centre of the picture, in its own colour.
    let centre = ((150 * 400 + 200) * 4) as usize;
    let found = (0..3).all(|c| rgba[centre + c].abs_diff(cav_render::tower_srgb(0)[c]) <= 40);
    assert!(found, "the centre pixel is {:?}", &rgba[centre..centre + 3]);
    assert_eq!(&rgba[..3], &cav_render::BACKGROUND_SRGB, "the corner is background");

    // The picture follows the engine: load the void and the room's floor is gone from the same pixel.
    let floor_pixel = ((100 * 400 + 150) * 4) as usize;
    assert_ne!(
        &rgba[floor_pixel..floor_pixel + 3],
        &cav_render::BACKGROUND_SRGB,
        "the room's floor is drawn"
    );
    client.text("scene_load", json!({ "scene": "void" }));
    let (rgba, _) = picture(&mut client, json!({ "width": 400, "height": 300 }));
    assert_eq!(
        &rgba[floor_pixel..floor_pixel + 3],
        &cav_render::BACKGROUND_SRGB,
        "the void has no floor there"
    );

    // Defaults, and refusals.
    let (_, size) = picture(&mut client, json!({}));
    assert_eq!(size, [960, 600]);
    let (is_error, content) = client.call("render_view", json!({ "width": 0, "height": 10 }));
    assert!(
        is_error && content[0]["text"].as_str().unwrap().contains("each side is 1 to 4096"),
        "{content:?}"
    );
    let (is_error, _) = client.call("render_view", json!({ "pane": 9 }));
    assert!(is_error);
    let log = client.text("log", json!({ "text": "render_view" }));
    assert!(log.contains("400 × 300 px") && log.contains("no pane 9"), "{log}");
    client.finish();
}

/// Writes a scene file under the test's temporary directory and returns its path with forward
/// slashes (and no spaces: the console's `load` takes one word).
fn scene_file(name: &str, spec: &cav_sim::SceneSpec) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, spec.to_ron()).unwrap();
    path.to_string_lossy().replace('\\', "/")
}

#[test]
fn mcp_refuses_scenes_outside_the_ranges_and_keeps_answering() {
    let (mut client, _) = Client::start();
    // The three specs of the S0 review that hung the renderer or exhausted memory.
    let mut far = cav_sim::SceneSpec::room();
    far.room.as_mut().unwrap().origin[0] = 1e17;
    let mut wide = cav_sim::SceneSpec::room();
    wide.room.as_mut().unwrap().size[0] = 1e10;
    let mut tall = cav_sim::SceneSpec::room();
    tall.array.instrument.tower_height = 3e6;
    for (name, spec, field) in [
        ("far.ron", far, "room.origin"),
        ("wide.ron", wide, "room.size[0]"),
        ("tall.ron", tall, "array.instrument.tower_height"),
    ] {
        let (is_error, content) = client.call("scene_load", json!({ "scene": scene_file(name, &spec) }));
        let why = content[0]["text"].as_str().unwrap();
        assert!(is_error && why.contains(field), "{name}: {why}");
    }
    // The scene is still the default one, and the server still answers.
    assert_eq!(client.text("scene_get", json!({ "path": "name" })), "\"room\"");
    // Control: a file inside the ranges loads.
    let mut tallest = cav_sim::SceneSpec::room();
    tallest.name = "tallest".into();
    tallest.array.instrument.tower_height = 1000.0;
    let stats: Value = serde_json::from_str(
        &client.text("scene_load", json!({ "scene": scene_file("tallest.ron", &tallest) })),
    )
    .unwrap();
    assert_eq!(stats["scene"], "tallest");
    client.finish();
}

#[test]
fn gpu_mcp_render_view_answers_for_the_largest_scene() {
    use cav_sim::spec::limits;
    let (mut client, _) = Client::start();
    let mut scene = cav_sim::SceneSpec::room();
    scene.name = "largest".into();
    scene.array.instrument.tower_height = limits::TOWER_HEIGHT;
    scene.array.instrument.ifo_separation = 500.0;
    scene.array.towers = (0..limits::TOWERS)
        .map(|n| cav_sim::TowerSpec { label: format!("t{n}"), position: [5.0 * n as f64, 100.0, 0.0] })
        .collect();
    scene.room.as_mut().unwrap().size = [limits::ROOM_SIZE; 3];
    client.text("scene_load", json!({ "scene": scene_file("largest.ron", &scene) }));
    // Before the ranges and the caps, this request was never answered. Now it is a picture.
    let (is_error, content) = client.call("render_view", json!({ "width": 320, "height": 200 }));
    assert!(!is_error, "{content:?}");
    assert_eq!(content[0]["type"], "image");
    // And the next request is answered too.
    let stats: Value = serde_json::from_str(&client.text("command", json!({ "line": "stats" }))).unwrap();
    assert_eq!(stats["towers"], 1000);
    client.finish();
}
