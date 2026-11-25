# S18 the editor: first review (25 Nov 2026)

Reviewed: `main` at `246456c` (PRs #160 to #167; TASK-S18-01 to 04 and 07 to 11) against
`docs/slices/S18.md`, `docs/design/keyboard.md` (the requirement, rule by rule),
`docs/research/editor-ux.md`, the owner's editor rulings in `plan/PROGRESS.md`, and
`plan/REBUILD.md` principle 11 and §6. The diff is `s4-big-scenes..246456c`.

Method: I read the code, the tests and the twelve pictures in `docs/slices/S18/` (each looked at
as an image). Then I measured what looked wrong: five throwaway probes in
`crates/cav-app/tests/` that drive the shell with the same input events as `tests/editor.rs`. I
ran them with `cargo xtask test probe_` in my own worktree and did not commit them. The numbers
below are theirs. I also ran `cargo xtask demo S18 --no-web` and `cargo xtask strays`. I did not
rerun `quick`, `gpu`, `oracle` or `validate`. `docs/validation/report.json` is the same file at
`s4-big-scenes` and at `246456c`, so no case was dropped and no tolerance moved. The slice adds no
physics, so that is expected.

**Verdict:** the keyboard model is well built. It is pure logic, tested as a state machine, and
most of the spec's rules hold. The GUI-as-data split is clean and easy to follow, and the
recipe matches the code. Six findings are important. Two are defects in the "one undoable
command" promise that I measured (R1, R2). Two are test arms that cannot fail (R3, R4). One is an
overclaim in §7.2 (R5). One is the `?` sheet, which is clipped and cannot be scrolled from the
keyboard (R6). The pictures show something that is starting to feel like an editor. The
inspector still reads as a developer's parameter dump (U1).

## Important

### R1. Delete or Ctrl+D inside a transform session acts; Esc then moves *another* body
**Where:** `crates/cav-app/src/keyboard.rs:706-708`. The session claims only letters; every other
key returns `None` and falls through to the scope shortcuts. Also `crates/cav-app/src/shell.rs:461-471`:
Esc's cancel moves `bodies[session.body]`, an index stored when the session began.

**Measured:** in `point` with a second mass added at (3, 3, 1), I selected body 1, pressed Enter
on `viewport/selection`, then →, then Delete, then Esc. The result was one body, now at
(2, 0, 2.5). That is the *deleted* body's start. The surviving mass was moved from (3, 3, 1) to
where the other one began. It is undoable, but nothing unseen should happen. The keyboard spec's
§1 promise is that arrows "move" or "adjust" and nothing else. Ctrl+D, which the letter rule
skips because Ctrl is held, and every other non-letter key of the viewport's scope get through the
same way.

**Fix:** a transform session owns every key that is not a global and not its own (return
`Some(vec![])` for the rest, as it already does for letters). Or end the session, cancelled,
before any other action runs. At Release, check that the body at `session.body` is still the
one the session began on (its label, or the selection) and do nothing if not. Add a test with a
control arm: Delete in a session removes nothing, and the same Delete outside one does.

### R2. A drag or a session while the scene plays is many undo steps; a cancelled one leaves them
**Where:** `crates/cav-engine/src/engine.rs:500-512`. A run is coalesced only when the
*previous journal entry* is a `Set` of the same path. `crates/cav-app/src/shell.rs:1273-1281`:
while playing, `advance` issues a `Seek` every frame. Space is a global (`ui/actions.ron:119`),
and the session does not take it.

**Measured** (`point`, body 1 selected; depths are `engine.steps().0.len()`):
- One gizmo drag of the x arrow, 2 m, over 3 frames: 1 → **4** steps while playing, 1 → 2
  paused.
- A transform session of three → presses, then Esc: 1 → **5** steps while playing, with the
  body back at its start. So each Ctrl+Z now moves it somewhere. K10's "Esc leaves nothing to
  undo" fails.

This breaks the slice's exit criterion ("every edit by viewport or panel is one undoable
command") in an ordinary use: editing with the scene playing. The old Select-tool drag has the
same hole; the gizmo and the session inherit it.

**Fix:** the simplest is to pause playback when a drag or a session starts, as the scrubber
already does (`panels.rs:455`, `:474`). The sturdier fix is to coalesce on the run itself rather
than on journal adjacency: a run id carried by `continues`, or the editor remembering the undo
depth at the run's start and cancelling with `undo` back to it. Add a test: a drag while
playing is one undo, with the paused drag as its control.

### R3. Four "no new undo step" assertions cannot fail
**Where:** `crates/cav-app/tests/editor.rs:210` (K10: "nothing new can be undone"), `:593`
(W2: "a layout's lines are view commands, never edits"), `:629` (C3: "framing is no edit"),
`:723` (C5: fly "none of it was an edit").

`Engine::history()` is `(bool, bool)`, and each test has already loaded a scene. So it reads
`(true, false)` before and after, whether or not an extra step was pushed. For example, a
`Home` added to the Analysis layout's lines would not fail W2. Neither would a fly that pushed a
step per frame. K10 is the end-to-end guard on the history change of §7.3.4, and its "no step
left" half is not held. The unit test in `history.rs` does hold the mechanism, and R2 shows the
end-to-end path breaking.

**Fix:** compare `engine.steps().0.len()` (or `History::depth`), with a control arm. A session
ended with Enter adds exactly one step; a `layout` whose lines include `set duration 5` fails.

### R4. `every_menu_item_is_an_action_that_runs` has a tautology where its check was
**Where:** `crates/cav-app/tests/shell.rs:228`.
`assert!(Shell::is_app_command(&line) || shell.engine.parse("select none").is_ok(), …)`. Before
S18 this was `engine.parse(line)`. It now parses a fixed line and is always true. The run
above it still catches a line that fails, so nothing is lost today. But CLAUDE.md forbids an
assertion that cannot fail, and this one reads as if it guards something.

**Fix:** `shell.engine.parse(&line).is_ok()`.

### R5. §7.2 says every row of §4 has its test; several do not, or are held more loosely than written
- K3's app-level half, `editor::a_canvas_click_does_not_move_the_keyboard_focus`: no such
  test. `shell.rs:1132-1135` does turn navigation off on any press, but nothing drives a canvas
  click and asserts that the focus path is kept.
- K9's app-level half, `editor::typing_q_in_a_field_does_not_change_the_tool`: no such test.
  E4 types Delete into the console, which is close but not the stated test.
- E6's second half, `editor::every_starter_has_an_editor_for_every_field`: no such test.
- K4 "…in every layout, maximised…": the Tab walk is skipped when maximised
  (`editor.rs:574`, `if state != "maximised"`). It is also never walked "with a selection and
  without".
- T1 "one undo per step": not asserted. Its budget is `TASK_EVENTS = 20` (`editor.rs:369`),
  but §7.2 says 12 is "now the test's budget".
- T2 "each by input events only": the Save and Open tests put the path into the window by
  assignment (`editor.rs:488`, `:495`), not by typing it.
- C5 "Shift ×10, Alt ×0.1": `camera.rs:185-186` passes `FLY_SPEED * 10.0` directly. The
  frame's use of the modifiers (`shell.rs:509`) is not exercised.
- §6's budgets (a key routed in 50 µs or less, the palette's filter in 1 ms or less) are not
  measured, but §7.4 names only the palette-open frame as unmeasured.

**Fix:** write the missing tests (each is a few lines with the existing `Rig`). Set
`TASK_EVENTS` to the 12 measured. Then correct §7.2 to say what is held.

### R6. The `?` sheet is clipped at 1440×900, and the keyboard cannot reach what is cut off
**Where:** `crates/cav-app/src/panels.rs:1808-1853`; picture `docs/slices/S18/editor-sheet.png`.

The window is `resizable(false)` with an unwrapped two-column grid. At the default window size
it grows past the right edge: the text runs to x = 1433 and the right column's descriptions
touch the frame. Its bottom rows (Typing, Tab, and below them the transform session's keys) are
cut off. The whole list is registered as one `Kind::ReadOnly` node (`:1853`), so Enter does
nothing and the arrows cannot scroll it. The spec's §6 asks of lists that "the WHOLE list is
reachable". So the sheet that should teach the keyboard cannot be read to its end with the
keyboard.

**Fix:** cap the width at `content_rect().width() - 32` and wrap the description column.
Register the list as a scrollable node: Enter starts interaction, then ↑ ↓ and Page Up/Down
scroll it, as the console's log does in the spec's §9 example.

## UX judgement

Measured against Unity, Godot or Blender as a first-time user would meet them. These are
opinions, ranked.

- **U1 (important). The inspector still reads as a developer's panel.** It shows snake_case
  field names (`tower_height`, `ifo_separation`, `half_sequence`, `launch_velocity`,
  `atom_mass`), a raw enum (`LowerMinusUpper`), and a faint `‹` reset glyph after *every* field,
  changed or not (every picture). It also shows the instrument's twelve fields whenever nothing
  is selected. The fix: take labels from the schema's titles ("Tower height", "Arm separation",
  "Gradiometer sign: lower − upper"); draw the reset only on a changed field (keeping the
  disabled node in the keyboard order, which the spec needs, without painting it); collapse
  Instrument and Cadence by default when nothing is selected.
- **U2. The tool strip is text, abbreviated and inconsistent.** It says "Sel Q", "Move W",
  "Add A". The Place tool is "Add" on the strip (`ui/tools.ron:29`, `short: "Add"`) and "Place"
  in its options row, its tooltip and the `?` sheet. Snap is a toggle drawn exactly like a tool,
  so in `editor-place.png` two buttons are lit. Use icons, or full words with the key as a
  smaller hint. Draw snap as a toggle apart from the tools, and call Place "Place" everywhere.
- **U3. Unity and Unreal users will hold the right button and press W A S D.** §1 promises this,
  but it was not built (`panels.rs:1717`: a right-drag pans). It is also not in §3's list for
  S19. Today W picks Move, A picks Place, and S toggles snap, so the user's next left click
  *adds a body*. Build it, or move it to S19 in §3 and record why.
- **U4. The breadcrumb says "Inspector › mass › mass"** (`editor-keyboard.png`, held by
  `task_keyboard_only`). The section is named after the body's label. The spec's own example is
  "Inspector › Body › mass". Name the section by what it is ("Body", "Shape", "Motion").
- **U5. The gizmo is small and gets lost.** Its 70-point arrows sit inside a framed voxel body of
  the same blue (`editor-gizmo.png`). After F the sphere fills about 0.9 of the pane's height,
  because C3 measures the centre-to-edge projection and not the perspective silhouette. Draw the
  gizmo with a dark outline on top, and frame on the silhouette.
- **U6. The history list shows derived JSON, not "the line that made it":** `add bodies 1
  {"label":"cube","mass":100.0,…` where the person chose Add › Cube (`editor-history.png`). Say
  what happened ("Added cube", "Moved sphere 2 m north", "Set duration 20 s") and keep the
  console line on hover. The window also covers the figure's top-left; open it over the
  outliner.
- **U7. Maximised, there is no visible way back.** The badge strip goes too
  (`editor-maximised.png`). A small "restore (Ctrl+Space)" in the corner beside Home would do.
- **What works:** the empty state (`editor-empty.png`: guidance and three buttons in the tree,
  and a hint line for right-click, A, F2 and Delete) is right. So are the palette, the layout
  tabs in the top bar, the unsaved `*` beside the scene name, and the Analysis and Instrument
  layouts, which look like a scientist's workspace. With U1 and U2 done it would feel like an
  editor rather than a dev panel.

## Minor (one line each)

1. A no-op `set` drops the redo stack. `history.rs:169` calls `forget_redo` before parking a
   step that changes nothing. Measured: set 13, undo, set back to 12 → redo gone. Forget redo
   only when a step is pushed (`continued` already does).
2. Home after an undo drops the redo stack (measured). This follows from the owner's rule (§7.3.6),
   but it is surprising: undo a delete, press Home to look, and the delete cannot be redone. Worth
   the owner's second look; a separate one-level camera undo would avoid it.
3. Tab in a transform session is "next axis" (`keyboard.rs:691`). The spec's §3 says Tab always
   goes to the next big scope, and the recorded exception (§7) is Esc and Enter only. Record it
   in `keyboard.md` or use another key.
4. Enter on `viewport/selection` with a tower selected starts a session with no body
   (`shell.rs:434-439`). The arrows do nothing until Esc, and the ring says blue.
5. `viewport/selection` is registered only when the active pane is a scene
   (`panels.rs:1631`). In the Analysis layout (pane 1 a plot) the viewport's stops change.
   Order should not depend on state.
6. `knows_scope` checks only the root of a sub-scope id (`actions.rs:182-185`). A misspelt
   `tree/sorces` passes `the_data_files_are_valid` and the key never fires.
7. `layouts.ron` validation accepts any command, edits included (`layouts.rs:46-52`). The
   Instrument layout runs `select none` (`ui/layouts.ron:29`), so switching layout loses the
   selection.
8. `History::said` serialises every Insert's whole value each frame the history window is open
   (`history.rs` `Edit::line`, `dialogs.rs:93`). A big voxel or mesh body makes that
   megabytes a frame. Cache it per generation.
9. K7's "control" computes a closed form and does not drive a broken `Repeat`
   (`keyboard.rs:1209-1212`). K1's stated control (a router that turns the ring on first) is
   absent. K8 never puts a scope shortcut against a mode key (an arrow or Enter) for the
   (4)/(5) pair.
10. A scene made by the wizard counts as saved (`engine.rs:968`: a load sets the baseline), so a
    never-saved new scene has no `*`.
11. The session's arrows move along world x and y whatever the view, so seen from the east, →
    moves the body left on screen.
12. Demo pictures differ from §5: there is no `editor-context.png` (the tree's right-click menu);
    the palette has no "sph" typed; the history picture has no toast; `editor-add-menu.png` is
    `editor-add.png`.
13. Docs: the brief's §2 names `ui/scopes.ron`, but the scopes are in `actions.ron`. §7.1 lists
    "tiers" for TASK-S18-11, though tiers moved to S19. `REBUILD.md`'s S18 row still lists
    docking, pop-out, materials, mesh import and tiers. §3's S19 list omits the browser keys
    that `IDEAS.md` sends there.
14. The web build ships the sheet's Ctrl+N, Ctrl+1–3 and Ctrl+Shift+P, which browsers keep for
    themselves (recorded for S19 in `IDEAS.md`). Until then, the sheet could mark them "not in
    the page".
15. E8's row in §4 says "clicking the first row of three undoes three". The test (rightly)
    expects the first row to undo two, and "history 0" to undo three.

## The questions asked

- **Is every edit by viewport or panel one undoable command?** Yes when nothing else is
  journalled during the run: the gizmo, the session, the field, Place, Delete, Ctrl+D, rename,
  the wizard, reset and the history list. No while the scene plays, or with Delete inside a
  session (R1, R2).
- **The keyboard state machine as logic with control arms:** yes, with the gaps in minor 9.
  Rules checked against the spec: navigation starts off, and a key acts then shows the ring.
  Enter with navigation off presses nothing (`keyboard.rs:581`). A click hides the ring and keeps
  the focus. Tab's order comes from data and is never skipped. Text fields eat all keys but Esc,
  Enter and Tab. Windows are sections and give the focus back. The breadcrumb works. DAS/ARR
  runs on the frame clock. Shift ×10 / Alt ×0.1 come from one function, with modifier hygiene
  for globals. The priority order is typing, drop-down, globals, scopes inside-out, modes. Two
  deviations: Tab in a session (minor 3), and the session not owning non-letter keys (R1).
- **Do the scripted tasks drive the UI?** T1 and T3 do, by input events only (T3 sends no
  pointer event and walks Tab, Enter and the arrows). Several others set `keyboard.focus` or a
  window's path field directly (R5).
- **The no-op-dropping history change:** the mechanism is right and its unit test bites. It
  cannot renew the step before it (I traced `edited`/`continued`/`undone` through drags that
  pass their start). It does drop the redo stack on a no-op (minor 1). End to end it fails while
  playing (R2), and the test that should show that cannot (R3).
- **Esc in a transform session:** cancels and leaves no step when nothing intervenes (K10
  holds). It fails as R1 and R2 describe.
- **Home undoable, clearing redo:** as built and as ruled. See minor 2.
- **`BODY_VISITS` 48 → 24:** honest. At `s4-big-scenes` the instrument and cadence were drawn
  under *every* selection (`panels.rs` at the tag, after the match), so the body's 48 visits
  included 24 that the scene's and the towers' inspectors also make. They still do, with
  `SCENE_VISITS` and `TOWER_VISITS` unchanged. Fewer fields are on screen; the check is not
  weaker.
- **History rows' memory:** deriving each row from its `Edit` keeps the byte bound true without
  a new count. The cost moved to the frame (minor 8).
- **Layouts and the action table as data:** yes, parsed with `deny_unknown_fields` and checked,
  with controls, and errors name the line for actions. Gaps in minor 6 and 7.
- **The web:** nothing is done for browser-reserved keys. This is recorded for S19
  (minor 14).
- **Docs:** `ARCHITECTURE.md`'s new section and table rows match the code. I followed
  `docs/extending/a-tool-or-panel.md` for an action and a window against the code: every name it
  cites exists (`Shell::register`, `window_opened`, `Effect::Close` in `Shell::effect`,
  `WINDOWS`, `tool_options_row`, `figure`), and the steps are complete. The crate boundaries are
  as the table says: `ron` was added to `cav-app` with its reason, and the starters are in
  `cav-sim`.
- **The demo:** `cargo xtask demo S18 --no-web` runs. After a 172 s release build it wrote 13
  files in about 22 s, and every picture and `session.txt` came out byte-identical to the
  committed ones (git saw no change). The demo is reproducible.
