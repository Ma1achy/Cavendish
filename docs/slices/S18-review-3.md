# S18 the editor: third review (25 Nov 2026)

Reviewed: `main` at `b8c165a`, after the fixes to the second review (`docs/slices/S18-review-2.md`):
PRs #178 to #182, TASK-S18-19 to 22, with the notes in `docs/slices/S18.md` §7.8. This review was
narrow. I read the diff `2229107..b8c165a`, the new and changed tests, `docs/design/keyboard.md`
§7 and §8, and the pictures in `docs/slices/S18/` (gizmo, place, history, wizard, palette,
instrument, context, sheet), looking at each one as an image. I ran the new tests one by one with
`cargo xtask test <filter>`, and all pass: `a_run_of_sets_is_one_step`, `legend` (8 tests),
`a_click_in_a_window_ends_…`, `home_in_the_middle_…`, `framed_voxel`, `gizmo_drag_ends`. I did not
write probes this round, so every scenario below is reasoned from the code and none is measured.
`docs/validation/` is unchanged since `s4-big-scenes`. This slice adds no physics.

**Verdict: nothing important remains.** I1 and I2 are fixed as the second review asked, and every
new test fails with its fix reverted (reasoned below). The cav-plot test change is a real defect
fix. What is left is minor. The largest minor item is that the session's "a click keeps it" rule
covers only windows and drags in the view. Clicks on panels, menus and the canvas leave the
session open (minor 1).

## I1: Home ends the run. Fixed.
`engine.rs:524-527`: `edit` is now true for any `Command::View(v)` with `v.is_edit()`, so
`self.run` becomes `None` after a Home. The run's next `Set { continues: true }` no longer matches
`run`, so it starts a step of its own instead of rewriting Home's step. Undo and redo of the Home
step put the camera back and forward again (`:1016`, `:1025`).
- `engine::tests::a_run_of_sets_is_one_step_whatever_comes_between`: with the fix reverted, the
  second set rewrites Home's step and the depth is 3, so `assert_eq!(depth, 4)` fails. Its control
  is an orbit in place of Home, which gives depth 2. That control would also fail if the fix went
  too far and ended the run on every view command. Both arms bite.
- `home_in_the_middle_of_a_drag_keeps_its_own_step` (cav-app): with the fix reverted, the first
  Ctrl+Z undoes a step whose undo is `Edit::View`, which puts the camera back, so `camera_kept`
  fails. The test also checks that Home really fired: without it, `after_undo == start` and the
  `assert_ne` fails. Its control (no Home, one undo back to the start) bites the other way.

## I2: the orphaned session. Fixed for the paths named.
`shell.rs:557-561`, `orphaned_session`, runs twice a frame: after `session_keys` and after the
windows are drawn. It ends the shell's session, keeping the move, whenever the keyboard is no
longer in `Session::Transform`. The order inside a frame is sound. Esc's `Release` is produced in
`keys()` and taken by `session_keys` before the first `orphaned_session` call, so Esc still
cancels and is not turned into "kept". A mouse-opened window (`open_window`) ends the session at
the second call, the one after `dialogs::draw`. A primary drag in the view ends it at
`panels.rs:2003-2006`, before the gizmo's drag is set up.
- `a_click_in_a_window_ends_a_transform_session_keeping_its_move`: the click is a real pointer
  event, so it goes through `window_node_at` and `focus_on`. With `orphaned_session` removed,
  `session.is_none()` fails. The control (the same click with no session open) moves nothing and
  leaves the depth unchanged.
- `a_gizmo_drag_ends_a_transform_session_first`: with `panels.rs:2005` removed, the session stays
  open, because a gizmo drag does not touch `keyboard.session`, so `orphaned_session` would not
  catch it. `session.is_none()` then fails. Bites.

**Against `keyboard.md` §8 as now written.** The code matches the letter of §8: "anything that
moves the focus out of a session … or a drag in the view ends the session, keeping its move".
Under §8's own first rule, a click on a panel or the canvas does not move the focus, so those
clicks rightly do not end the session. Two points still disagree, and they are minor 1 and 2.

## The two cav-plot expectations ("2.5 2.5 2.5" became "all 2.5"): a real fix
The old expectations were not a tolerance or an oracle. They froze the exact output that the
second review's UX 5 called a defect: a bar of one colour labelled with the same number three
times. The legend now draws one swatch and "all N" when every leaf has the same value
(`voxels.rs:106`). Both changed tests keep a branch for a range that is not flat. The new
`a_legend_of_one_value_is_one_swatch` has a working control: two values give a bar of
`BAR_SEGMENTS` segments and four texts. The change is recorded under "Changes to tests, with
reasons" in §7.8, so it does not break the CLAUDE.md rule against changing a check to make it
pass. The tests use `lo == hi` where the code uses a 1e-12 relative test. That mismatch is
harmless: a near-flat range would make the test fail, not pass falsely. See minor 5 for a small
loss of coverage.

## `CAGE_FILL`: sound
`camera.rs:116-161`. `frame_caged` backs the camera off until the cage's farthest projected
corner, measured as a radius from the projected centre, spans at most 0.9 of the pane's shorter
side. Because the centre is the target, it projects to the middle of the pane, so a span of 0.9
or less really does mean the box is inside the pane, with a margin. `cage_of` uses the shape's
reach on every axis. For a sphere that is the drawn box (the cubes' bounding box,
`cav-render/src/scene.rs:570-586`), and for any other shape it is a box that contains the drawn
one. The geometry puts the body at about 0.52 of the side, inside C3's 0.4-0.8 band, and the test
asserts that band. The control (the plain `frame` puts the cage past 0.9) bites if the shell goes
back to `frame`. `editor-gizmo.png` now shows the whole cage inside the pane. 0.9 is recorded as
a new framing bound under "Decided" in §7.8.

## The UX items in the pictures
- UX 1: "Point radius 0 m" and "Shape › Radius 0.25 m" are now told apart (`editor-gizmo.png`).
- UX 2: Place draws a ground grid (`editor-place.png`). The ghost is not in the picture
  (minor 7).
- UX 3: the cage is inside the pane.
- UX 4: the rows read "Before opening: “room”", "Opened “point”", "Set mass to 5 kg on “mass”"
  and "Moved cube to (2.5, -2, 0.25) m", and the window stays within the outliner's width.
- UX 5: the legend reads "level / all 3".
- UX 6: "sph" gives the five word matches. A sixth "command" row is still there (the console's
  `add <starter>` syntax), which is fine.
- UX 7: the first body is chosen from tiles, three to a row, and the wizard now fits.
- UX 8: the atom mass reads "86.9089 u" (Rb-87, correct). "more below: scroll" sits at the foot,
  though it is faint. "Loss 0" and "Seed 0" still have no hint (minor 9).
- UX 9: loading a scene raises no Undo toast (`editor-place.png` footer).
- The `?` sheet now titles the section "Tool strip", with snap listed in it as a switch.

## Minor (one line each)

1. The session outlives mouse actions outside windows. Per the code, nothing ends it: an outliner
   row click (`panels.rs:677`), a canvas click that selects (`panels.rs:2049-2054`), and a menu or
   context-menu item (`menus.rs:104`, `panels.rs:721`, which call `run_action` directly and not
   through `Effect::Action`, so they do not cancel first the way a global key does). Scenario:
   Enter, →, then click "other" in the outliner. The gizmo is now on "other", but → still moves
   body 1, and Delete and F are eaten by the session. Edit › Undo from the menu undoes the
   session's step while the options row still says "moving by". Nothing is lost, because Esc
   still cancels as the options row says. But §8 cites Blender, where any click outside the move
   confirms it. Fix: in `keys()`, end the session (kept) on any primary press outside the
   viewport's options row, and test it with an outliner click followed by → (control: no session).
2. `keyboard.md` §8 says "Esc is the only cancel", but §7 (lines 139-141) and `shell.rs:1333-1342`
   cancel the session on any global key, Ctrl+Z included. Say: "of the mouse, nothing cancels;
   of keys, Esc and any global key (Ctrl+Z being that cancel) do".
3. `shell.rs:1268`: the mouse fly now ends on `hover_pos().is_none()`. On native Windows, winit
   reports the cursor leaving even while a button is held. So a wide look turn that reaches the
   window's edge would end the fly mid-drag, and the held W would then be the Move tool. It is
   not restarted when the pointer comes back, since that needs a fresh press (`panels.rs:1949`).
   Unverified on hardware. End on pointer-gone only when `secondary_down()` is also false, or
   grab the cursor while flying.
4. A Home that changes nothing (the camera already at home, `engine.rs:1164`) pushes no step but
   still ends the run. A drag with such a Home in it then becomes two undo steps with nothing
   between them. Count Home as an edit only when its view changed.
5. `a_legend_is_made_from_what_is_drawn` now checks the bar segment by segment only for Phase,
   which is diverging. No sequential field is checked over a range that is not flat. (The old
   Height case was flat, so it never truly tested this either.) Add Level or Height with a widened
   range.
6. The history's first row is drawn as "Before opening: “room”" (`dialogs.rs:265`) but its
   focus-tree name is "the start" (`dialogs.rs:304`, asserted at `editor.rs:1511`). A screen
   reader says something other than what is shown, and the visible wording is not asserted
   anywhere. Name the node after its text.
7. `editor-place.png` shows the grid but no ghost, because the demo has no pointer. UX 2's point
   is the ghost. Move the pointer into the pane in the demo step.
8. The wizard's tiles reuse icons: Cube and Box share one, the four polyhedra share a triangle,
   and Oscillating mass, Pendulum and Walker share one (`editor-wizard.png`). Suggested for
   `plan/IDEAS.md`.
9. In the Instrument layout, "Loss 0" and "Seed 0" still have no unit or hint, and "more below:
   scroll" is low contrast over the faded rows (`editor-instrument.png`).

## Docs
`docs/ARCHITECTURE.md` gains both rules: Home ends the run, and a session left by the keyboard
ends kept. They match the code. The rows of `docs/slices/S18.md` §7.8 match the tests named.
Validation: the report is unchanged since `s4-big-scenes`, so nothing was dropped and no tolerance
moved.
