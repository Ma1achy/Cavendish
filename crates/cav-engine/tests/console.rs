//! Console lines as a client sends them: what one line is, and what its value is.

use cav_engine::{Command, Engine};

#[test]
fn a_quoted_value_with_an_escaped_quote_is_one_statement() {
    // Found by S2's review (finding 15): over MCP's `command`,
    // `set name "a\"; get duration; set name \"b"` ran as three statements and journalled two
    // wrong sets. It is one statement: a name with a quote and a semicolon in it.
    let mut engine = Engine::new();
    let before = engine.journal().len();
    let line = r#"set name "a\"; get duration; set name \"b""#;
    engine.run_script(line).unwrap();
    assert_eq!(engine.spec().name, r#"a"; get duration; set name "b"#);
    assert_eq!(engine.journal().len(), before + 1, "one command, not three");
    let Some(Command::Set { path, value, .. }) = engine.journal().last() else { panic!("a set") };
    assert_eq!((path.as_str(), value.as_str()), ("name", Some(r#"a"; get duration; set name "b"#)));
    // The log shows it as a line that issues the same command again.
    let mut again = Engine::new();
    again.run_script(&cav_engine::describe(engine.journal().last().unwrap(), 0)).unwrap();
    assert_eq!(again.spec().name, engine.spec().name);
    // Control: without the escapes it is three statements, and the journal has two sets.
    let mut plain = Engine::new();
    plain.run_script(r#"set name "a"; get duration; set name "b""#).unwrap();
    assert_eq!((plain.spec().name.as_str(), plain.journal().len()), ("b", before + 2));
}

#[test]
fn text_that_begins_as_json_and_is_not_is_refused() {
    // Finding 16: `set name "open` set the name to `"open`, quote included.
    let mut engine = Engine::new();
    let before = (engine.spec().clone(), engine.journal().len());
    for line in [r#"set name "open"#, "set bodies [1, 2", r#"set room {"size": }"#] {
        let why = engine.run_line(line).unwrap_err();
        assert!(why.contains("is not JSON"), "{line}: {why}");
    }
    assert_eq!((engine.spec().clone(), engine.journal().len()), before, "a refusal changes nothing");
    // Control: a bare word is still text, and a quoted one is the same text.
    engine.run_line("set name mine").unwrap();
    assert_eq!(engine.spec().name, "mine");
    engine.run_line(r#"set name "yours""#).unwrap();
    assert_eq!(engine.spec().name, "yours");
}

#[test]
fn import_adds_a_mesh_body_in_one_step() {
    // S19 I1 and I3: a mesh file dropped on the window (its bytes kept by the engine), imported
    // fitted to 2 m, of lead, placed: one body, one step, which one undo takes out; the journal
    // holds the body, so a replay needs no file. Control: a file that is not a mesh is refused in
    // words and changes nothing; so is a name nothing was dropped under and no file has.
    use cav_core::Vec3;
    use cav_core::voxel::mesh::{box_mesh, to_stl};
    let (v, t) = box_mesh(Vec3::new(-0.5, -0.25, -0.1), Vec3::new(0.5, 0.25, 0.1));
    let mut engine = Engine::new();
    engine.run_line("load void").unwrap();
    engine.keep_dropped("crate.stl", to_stl(&v, &t)).unwrap();
    let depth = engine.depth().0;
    engine.run_line("import crate.stl fit 2 material lead at 1 2 3").unwrap();
    let body = engine.spec().bodies.last().unwrap().clone();
    assert_eq!(body.label, "crate");
    assert_eq!(body.motion.anchor().0, [1.0, 2.0, 3.0]);
    let shape = body.shape.as_ref().unwrap();
    assert!((shape.kind.bounds().unwrap().size().x - 2.0).abs() < 1e-12);
    assert_eq!(shape.material.as_deref(), Some("lead"));
    assert_eq!(engine.depth().0, depth + 1, "one step");
    let replayed = Engine::replay(engine.journal()).unwrap();
    assert_eq!(replayed.spec(), engine.spec(), "a replay needs no file");
    engine.run_line("undo").unwrap();
    assert!(engine.spec().bodies.is_empty());
    engine.keep_dropped("notes.txt", b"not a mesh".to_vec()).unwrap();
    let why = engine.run_line("import notes.txt").unwrap_err();
    assert!(why.contains(".obj or an .stl"), "{why}");
    let why = engine.run_line("import nothing-dropped.stl").unwrap_err();
    assert!(why.contains("not a file dropped on the window"), "{why}");
    let why = engine.run_line("import crate.stl fit -1").unwrap_err();
    assert!(why.contains("not a positive number"), "{why}");
    assert!(engine.spec().bodies.is_empty() && engine.depth().0 == depth, "nothing changed");
}
