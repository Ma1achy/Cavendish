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
