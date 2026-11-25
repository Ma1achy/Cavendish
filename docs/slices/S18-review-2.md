# S18 the editor: second review (25 Nov 2026)

Reviewed: `main` at `b44f723`, after the fixes to the first review (`docs/slices/S18-review.md`):
PRs #169 to #176, TASK-S18-12 to 18, with the notes in `docs/slices/S18.md` §7.7. I read the diff
`323c34e..b44f723`, the tests, the docs and all fourteen pictures in `docs/slices/S18/`, looking
at each one as an image. I measured what looked wrong with four throwaway probes in
`crates/cav-app/tests/`, driven like `tests/editor.rs`. I ran them with `cargo xtask test probe_`
in my own worktree and did not commit them. The numbers below come from those probes. I also
ran `cargo xtask strays`, which was clean. I did not rerun `quick`, `gpu`, `oracle`, `validate`
or the demo. `docs/validation/` has not changed since `s4-big-scenes`: `git diff s4-big-scenes
b44f723 -- docs/validation/` is empty. So no case was dropped and no tolerance moved. This slice
adds no physics.

**Verdict:** the first review's findings are fixed, and the fixes' tests bite (checked one by one
below). Two important defects remain, and both are in the new code:
- I1: R2's run outlives a Home, which is an edit, and merges into Home's step.
- I2: the new rule for clicks on a window's controls (§8) orphans a transform session.

The pictures now read as an editor. What still confuses a newcomer is smaller and listed
under UX.

## Important

### I1. A run of `Set`s continues *through* a Home, and rewrites Home's undo step
**Where:** `crates/cav-engine/src/engine.rs:524-531`. `run` is cleared only when `prepared` is
`Edit` or `Restore`. `ViewCommand::Home` prepares `Plain` (`:770`), yet it pushes a history step of
its own (`perform_view`, `:1161-1166`). So after a Home, `run` still names the drag's path. The
drag's next `Set { continues: true }` then takes the `continued` branch, which rewrites the
**last** step: Home's.

**Measured** (engine, `point`, camera orbited to yaw 33 first): set x+1, Home, set x+2
(continues). The result is three steps. The last has `undo = Edit::View`, `redo = Edit::Put`. The
first undo puts the camera back and leaves the body at x+2. The second undo takes x to 0. After
two redos the body is at x+2, but the camera is at yaw 33, not Home's −45: the redo of "Home" no
longer homes. **In the app:** I pressed the `Home` key halfway through a gizmo drag of the x
arrow (a global, routed while the mouse is held). The same mixed step appeared (`undo=view
redo=put`), and one Ctrl+Z put the camera back and left the body 2 m out. The history list then
words the Home row as a move.

Before R2's change this was safe, because a `Set` continued only the journal's adjacent entry.
The engine already knows that Home is an edit (`ViewCommand::is_edit`, `command.rs:447`). Today
only `layouts.rs:57` uses it.

**Fix:** end the run on any edit, Home included. For example,
`let edit = matches!(prepared, Prepared::Edit(..) | Prepared::Restore(_)) || matches!(&command,
Command::View(v) if v.is_edit());`. Or, more robustly, end it whenever the history's depth or
last step changed under anything but this run. Add a case to
`a_run_of_sets_is_one_step_whatever_comes_between`: set, Home, set with continues gives three
steps, the last being two `Put`s. Its control is the same sequence with an orbit for the Home,
which gives one step.

### I2. A transform session is orphaned by a click on a window's control or by a window opened with the mouse
**Where:** `crates/cav-app/src/keyboard.rs:550-556` (`focus_on`) and `:563-572` (`open_window`) set
`keyboard.session = None`. Neither ends the shell's `tools::Session` (`shell.rs:1240-1243` calls
`focus_on` for the new §8 rule; `window_opened`, `shell.rs:1077`, calls `open_window`).
`end_session` is reached only from `Effect::Release` and from `Effect::Action`
(`shell.rs:1299`).

**Measured** (`point` with a second mass, the history window open beside the viewport): Enter on
the selection, then → moves body 1 by 0.25 m. A click on the history window's last row (which
changes no scene) leaves `shell.session` as `Some`, `keyboard.session` as `None`, and focus at
`history/done/3`. Then →, Esc and Esc leave the session open and the body at +0.25 m. Esc no
longer cancels, and the options row still says "moving by (+0.250, …) · Enter keeps · Esc
cancels" (`panels.rs:1559-1572`). The next shortcut of any scope (F, Delete, Q, …) arrives as an
`Effect::Action`, and `effect` then cancels the forgotten session first. The body jumps back to
where the session began. The person never asked for that, and they took the move for kept. With
`history` opened from the Edit menu by the mouse the orphan is the same.

A gizmo drag of the same body while a session is open is a third way in. The session's next
arrow recomputes the anchor from its own `start` (`shell.rs:497-500`), so it throws the drag's
move away.

**Fix:** one owner for "a session is open". After `keys()` each frame, if `self.session.is_some()`
and `self.keyboard.session != Some(Session::Transform)`, call `end_session(false)` (or `true`;
decide and write it into `keyboard.md` §8). Do the same when a primary drag starts on a pane.
Test: session, →, a click on a window control. The session is then over, the body at its start,
and the depth unchanged. Control: the same click without a session moves nothing.

## The first review's findings: fixed, and do the tests bite?

For each test I asked what it would see with the fix reverted.
- **R1:** `delete_inside_a_transform_session_…`: with `session_key` returning `None` again,
  Delete removes a body (`len == 2` fails). `a_session_whose_body_has_gone_moves_nothing`:
  without `holds`, → moves "other" (fails). `a_global_key_ends_a_session_first_…`: without the
  cancel, `session.is_none()` fails. Without the `undo` early return, the duration edit is undone
  (fails). All bite. Labels are unique by `validate` (`spec.rs:910`), so `holds` is sound. The
  gap is I2.
- **R2:** `a_drag_or_a_session_while_playing_is_one_undo` and `a_held_session_while_playing_…`
  give several steps with the engine change taken out. They bite. The journal stays replayable
  bit for bit: `run` is derived only from applied commands, refused commands are not journalled,
  and `restart_journal` starts a fresh engine (with `run` = `None`) on both sides. Two separate
  user edits cannot merge through the app, because every interaction's first step sends
  `continues: false`, and only after an applied step (`GizmoDrag::edited`, `editing`,
  `drag_under_way`). Undo and redo end the run (`Restore`). Home does not (I1).
- **R3:** each `depth()` assertion has its edit-shaped control (`set duration`, a layout with an
  edit refused, Home seen by C3's count). They bite.
- **R4:** parses the item's own line. Fixed.
- **R5:** K3's application half bites on the wizard radio (focus path asserted). K9's half bites
  (the tool would become `select`). C5 through the frame: ratios of exactly 10 and 0.1 bite.
  K4 is walked maximised and with a selection. `TASK_EVENTS` = 12.
  `every_starter_has_an_editor_for_every_field`'s "control" (nothing selected gives no editor)
  only shows the measure works; the loop is what bites. That is acceptable.
- **R6:** the sheet is in the window at 1440×900 and 1024×640, and Page Down and ↑ scroll it, with
  the navigation-mode control. Bites. The picture shows the whole sheet.
- **U1:** folding, names and the reset's enabled state bite. Folded sections stay stops: the
  section id is registered as a `Scope` first and re-kinded to a `Button` when folded
  (`editors.rs:234-243`, `panels.rs:70-82`; `FocusTree::add` keeps the place). So Tab and arrow
  order do not change with the fold. Gap: "the reset is not *drawn* on an unchanged field" is not
  held. `editor.rs:1302` checks `enabled`, so reverting `add_visible` to `add` would pass.
- **U2, U4, U6, U7, minors 1, 3–7, 9–11, 15:** each test has a control that fails on revert.
  Minor 9's K7 control is now a driven `Repeat::with_clock(0.0, …)` giving 27.
- **U3:** `the_right_button_flies_and_holds_the_letters` bites (W would choose Move), and its
  control is W after release.
- **U5:** C3 measures the silhouette at FILL ± 0.02 in a wide pane, a narrow one and ortho, and
  the gizmo's size on screen is held near and far. Bites.

## Minor (one line each)

1. History rows name bodies by today's index: after a removal shifts the list, a step of
   `bodies[1]` names whatever body is now second (`dialogs.rs:127-129`). The test even freezes
   "Moved body 2" and "Set mass's mass to 5 kg" (`editor.rs:1467-1469`). Name from the step's
   own scene, or keep the label in the step.
2. U1's "reset drawn only on a changed field" has no assertion. Check the reset's rect is absent
   from the layout, or that it is not visible, when unchanged (`editor.rs:1302`).
3. During the right-button fly only letters are held back (`shell.rs:1257-1261`): `1`–`9`
   select towers, Delete deletes and Space plays mid-flight. Hold back every non-global key, as
   a session does.
4. `mouse_fly` ends only when `secondary_down()` reads false (`shell.rs:1231`). A release that
   lands outside the page on the web leaves letters swallowed until the next right click. Also
   end it on focus loss or pointer leave.
5. `window_node_at` picks the smallest node under the pointer across *all* windows
   (`keyboard.rs:177-194`). Where two windows overlap it can focus a control in the one
   underneath. Prefer the topmost window, by egui's layer order.
6. The `?` sheet still lists "S · Snap to the step" under **Tools** (`editor-sheet.png`), while U2
   made snap "not a tool". The test checks only the label prefix (`editor.rs:1393`).
7. A click on a window's empty area focuses the window root (`focus = ["history"]`), which is
   not a section. Clamp to the window's first section.

## UX judgement (as a first-time user, from the pictures)

It now reads as an editor. The tool strip has icons with their keys and a separate snap switch,
as Godot and Blender do. The inspector says "Tower height", "Separation Δr" and "Sign of ΔΦ". The
scene summary has folded Cadence and Instrument sections, like Unity's component headers. The
breadcrumb reads "Inspector › Body › Mass". The toast says "Added sphere · Undo". Maximised has a
visible "Restore Ctrl+Space". The whole `?` sheet fits. The first review's U1 to U7 are met.
What still confuses, ranked:

1. **Two "Radius" fields on a shaped body** (`editor-gizmo.png`): the body's "Radius 0 m" above
   and "Shape › Radius 0.25 m" below. A newcomer cannot tell which one counts. Hide the body's
   radius when it has a shape, or call it "Point radius".
2. **Place in the void has nothing to aim at** (`editor-place.png`). It says "click the ground
   (z = 0 m)", but no ground or grid is drawn and no ghost follows the pointer. Draw a ground
   grid and a preview while Place is in hand.
3. **After F the selection's box runs off the pane** (`editor-gizmo.png`: the corners of the
   blue cage are cut at top and bottom). The sphere sits at 0.6, but the highlight reads as
   "framed too tight". Frame on the drawn cage, or draw a tighter highlight.
4. **The history list** (`editor-history.png`): "The scene as it was" and "Opened “point”"
   one above the other look like the same thing twice. The window also covers part of the tool
   strip (x ≈ 266–297), and "Set mass's mass to 5 kg" reads oddly. Say "Before opening", keep
   the window inside the outliner, and write "Set mass to 5 kg on “mass”".
5. **The level legend "3 3 3"** (`editor-history.png`) looks like a bug when every body has one
   level. Hide the legend, or show a single swatch.
6. **The palette pads "sph" with noise**: after the five real hits come "View › Show in the
   pane › the scene / signal / flights …" (`editor-palette.png`). Cut matches below a score.
7. **The wizard's "A first body" is fifteen radios in a column** (`editor-wizard.png`). A
   drop-down or a grid of icons would fit the window and scan faster.
8. **The inspector in the Instrument layout is clipped at the bottom** with no sign it scrolls
   (`editor-instrument.png`, the hint cut mid-line). "1.44316e-25 kg" is raw; "Loss 0" and
   "Seed 0" have no hint. Show a scroll shadow, and atom mass in u (or Da).
9. **"Opened “void” · Undo"** on every load invites undoing an open. Keep the toast for edits a
   person made.

## Docs and plan

- `docs/ARCHITECTURE.md`'s editor paragraph matches the code (`paths::Entry::label`,
  `dialogs::step_words`, the engine's private `run`, the session's key rule). It should gain I1's
  rule once fixed.
- `docs/extending/a-tool-or-panel.md`: I followed "a tool" against the code. Every name it cites
  exists (`ui/tools.ron` without `short`, the label check in `Strip::parse`, `icons::tool` by
  `behaviour`, `figure`, `tool_options_row`), and the layout recipe's `unfold` and its
  view-commands-only check match `layouts.rs`.
- `docs/design/keyboard.md` §7 records the session's key rule and Tab as the next axis. §8 needs a
  line on what a click into a window does to an open session (I2).
- `plan/IDEAS.md`, "From S18's first review", holds the items left: minor 2 (Home drops the redo),
  minor 8 in part, minor 14 (browser keys), the wizard's unsaved mark after undo, a key for the
  context menu, and the name column.
- `plan/REBUILD.md`'s S18 row now says what was built and what moved to S19.
- Validation: the report is unchanged since `s4-big-scenes` (113 of 113). Nothing was dropped and
  no tolerance moved.
