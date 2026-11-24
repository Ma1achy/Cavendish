# Adding a command, with its console, CLI and MCP faces

**Example to read:** `Command::Compare` in `crates/cav-engine/src/command.rs`, how
`engine.rs` prepares and performs it, and the console's `compare` in `console.rs`. For something
that only reads, `Query::Cadence`.

A command changes the engine's state and is journalled; a query reads and changes nothing. Decide
which yours is first. There is one path in: whatever a person can do from the window, the
console, the command line or an agent must be the same `Command`.

## The command

1. **The data.** Add a variant to `enum Command` (or `Query`, with its reply in `Reply`) in
   `crates/cav-engine/src/command.rs`. Plain serialisable data, every field documented with its
   unit. It will be written into journals: a number in it must be finite, and anything large
   should be boxed.
2. **Whether it can be applied.** In `Engine::prepare` (`engine.rs`): return why not, with the
   values at fault, or what it will do. Nothing may change here: a refusal leaves the engine
   exactly as it was, and the tests check that by digest.
3. **Doing it.** In `Engine::perform`. If it changes the scene it must go through `Prepared::Edit`
   with the step that undoes and redoes it (`history::Edit`), so that undo works; if it changes
   what the towers read, call `self.schedule(false)`.
4. **Its place in the journal.** `history::weigh` counts it as the bytes of its JSON in a
   journal file, so there is nothing to add there; `Command::overwrites` says whether a run of it
   collapses into its last (a scrub, a drag). If your command holds state the engine must
   reproduce on replay, add that state to `State` (what `digest` covers) and to
   `Engine::standing`, the commands `journal restart` opens a new journal with.
5. **Its level in the log**: the match at the top of `Engine::apply`.

## The console

6. In `crates/cav-engine/src/console.rs`: an entry in `COMMANDS` (usage, what it does, an example
   that runs on a fresh engine), a line in `Engine::parse`, and the inverse in `describe`: the
   log shows a command as the line that would issue it again.

## The command line and MCP

7. `cav exec "<your line>"` already works. A subcommand of its own goes in
   `crates/cav/src/main.rs` (`USAGE` and `run`), as a short session of console lines: see
   `run_scene`.
8. An MCP tool goes in `crates/cav/src/mcp.rs`: an argument struct with doc comments (they are
   the tool's schema), a `#[tool]` function whose body runs inside `answered("<name>", …)` (so
   that a panic is an answer, not a hung client), and a word in the server's instructions.

## The window

9. A menu entry in `crates/cav-app/src/menus.rs` or a button via `button_row` in `panels.rs`:
   both run a console line, so there is nothing else to wire.

## Tests that then hold it

- `console::tests::every_documented_command_parses`: runs your example.
- `console::tests::console_lines_parse_to_the_same_commands`: add your line and its command; it
  also checks that `describe` gives the line back.
- `tests::commands_and_queries_round_trip_through_json` (`lib.rs`): add your variant.
- `engine::tests::session_replays_from_its_journal` and `engine.replay_bundles`
  (`crates/cav-engine/tests/forward.rs`): add your command to a session; the replay must come to
  the same digest and the same bytes.
- `engine::tests::a_refused_command_changes_nothing_and_logs_an_error`: add a refusal.
- `cav`: `mcp_lists_its_tools_with_their_arguments` and `every_tool_runs_where_a_panic_is_an_answer`
  count the tools: update the count with your tool.
- `cav-app`: `every_menu_item_is_a_console_command`.

## Things that go wrong

- A command that does its work in `prepare`, or that can fail in `perform`: a refusal must leave
  nothing behind.
- A command whose effect depends on the time of day, the files on disk or the order of a
  `HashMap`: the journal will not replay.
- Writing to a path from the journal on replay. Exports go where the replayer says
  (`Exports::Into`: inside its directory, a relative path with its folders, any other under its
  file name, two paths never on one file), never where the journal says.
- Asking the machine something when a file is written (git, the clock, the host's name). What a
  file says of the build is fixed when the engine is compiled (`cav_engine::build`).
- Waiting for the signal inside `apply`. Only an export waits, and only for as long as the
  client's patience.
