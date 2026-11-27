# S20 review: the workspace

Fresh review of S20 on `main` at f204fc1 (PR #230), against `docs/slices/S20.md` (§7 as built),
`docs/design/workspace.md`, `panels.md`, `keyboard.md`, `plan/REBUILD.md` and CLAUDE.md's quality
bar. Diff read against `s19-editor-second-half`. Every picture in `docs/slices/S20/` was looked at.

Run here, in a fresh worktree (cold build), below normal priority:

- `cargo xtask quick`: **704 tests pass**, fmt and clippy clean, "at the floor". The 347 s is a
  cold build of a new worktree (226 s of it the tests compiling and running cold), not a warm run.
- `cargo xtask gpu`: **42 of 42 pass**.
- `docs/validation/`: byte-identical to the tag (116 of 116 there and here); no case dropped, no
  tolerance moved, none added. Agreed: S20 changes no number a case measures.

## Verdict

The slice does what the brief promises. The mosaic, the header, the gizmo, the floating palette,
the transport, end behaviour, folders, the path and the physical camera are all there, and the
tests are mostly well made: most have real control arms, and many were seen failing by mutation.
Three things should be fixed before tagging. All three are about the first-person camera as the
owner will meet it, and each is a small change.

## Important (fix before tagging)

1. **In the default scene the room traps the camera, behind walls and a roof that are drawn as a
   wireframe.** `crates/cav-app/src/occupancy.rs:270-283` makes the room's six faces solid shells
   from both sides. `scenes/dev/point.ron:73-76` has a room 10 × 10 × 3 m, drawn as an outline
   the eye sees through, and every orbit view of `point` starts outside it (all the `dense-*`
   pictures). What happens:
   - Hold the right button and fly toward the towers: the camera stops in mid-air at an
     invisible plane, `outer = wall + SHELL`. W2 asserts exactly this
     (`s20.rs:2813-2821`, "the wall's outside stops it").
   - Shift+G (walk) from the default view falls onto the room's **ceiling** at z = 3.1 m and
     stands on it.
   - The only way in is to know about N (noclip).

   The owner asked for a Minecraft creative camera. In Minecraft the walls you collide with are
   walls you can see. The brief's "flying collides" is right for walls that look solid, wrong for
   an outline. *Fix (suggested):* the room collides only with a camera whose box is inside the
   room's volume (it keeps you in the hall, it does not keep you out). Or leave the room out of
   the occupancy while it is drawn as an outline. Either way, change W2's last arm to the new
   rule, with its control.

2. **Walking, the wheel changes and shows a speed that does not apply.**
   - `first_person.rs:343-348`: the wheel scales `fly_speed` and shows it.
   - `first_person.rs:389-399`: walking moves at `settings.walk.speed × pace` (4.3 m/s) and never
     reads `fly_speed`.
   - So `fly.png`, "on foot in town", shows **4.0 m/s** while the camera walks at 4.3 m/s.
     Scrolling while walking moves that number and nothing else.

   The owner's words were "wheel sets speed and shows it briefly". *Fix:* either the wheel scales
   the walking speed too and the read-out shows the speed actually in use, or walking ignores the
   wheel and shows nothing. Add a C3 arm that walks, scrolls and checks that the distance covered
   matches the number shown.

3. **On the web the app teaches Ctrl as sprint, and Ctrl+W closes the tab.**
   - `first_person.rs:336` sprints on `ctrl || command` on every platform.
   - The `?` sheet (`sheet.rs:67`, "Ctrl · W twice") and the `fly` action's tooltip
     (`ui/actions.ron:91`, "Ctrl or W twice sprints") say so in the page too.
   - Browsers do not let a page cancel Ctrl+W or Cmd+W, even under pointer lock.
   - There is no `beforeunload` guard (none in `crates/cav-app/src`).

   So on the public Pages build, a user who follows the sheet (Ctrl held, then W) loses the tab
   and any unsaved scene. Ctrl+W is not *needed* on the web, since the double tap of W works, but
   nothing stops it from being used. *Fix:* on `wasm32`, Ctrl is not sprint. The sheet and the
   tooltip then say "W twice" there (the platform seam already exists). Better still, also add a
   `beforeunload` prompt while the scene has unsaved edits. Record it against TASK-S19-11.

## Minor (one line each)

4. Home's undo targets a pane by index (`engine.rs:1118-1130`, stored at `:1330`). After a close
   renumbers the viewports, Ctrl+Z gives another viewport the camera, or writes to a hidden slot
   and appears to do nothing. The fix is cheap now that labels exist (store the label). Fix it
   rather than leave it in IDEAS.
5. C1's control arm (`camera.rs:585-589`) is arithmetic on constants. It cannot fail whatever the
   code does. The property is still held by the main assertion (30 and 144 fps agree to 1e-9),
   so this is a decorative control, not a hole.
6. §7.6 quotes a frame time of 1.3 ms, a walker step of 2.2 µs and a capped sphere of 1.24 ms.
   The committed `S20/timings.txt` says 2.12 / 2.51 ms, 3.04 µs and **1868.9 µs** (93 % of the
   2 ms budget). Make the record match the file.
7. `mosaic-float.png` does not show a float: the popped-out viewport opens exactly over its own
   slot, so it looks docked. A pop-out should open offset and smaller than its slot, so that it
   reads as floating and the "dock" button stays visible.
8. The first-person near plane of 2 cm with far plane 5000 m on forward-Z `Depth32Float`
   (`cav-render/src/camera.rs:52-61`, `renderer.rs:16`) gives about 3 to 7 cm of depth
   resolution at 100 to 150 m. That may be the speckle at town's horizon in `fly.png`.
   Reverse-Z would remove it; put it in IDEAS.
9. A right-drag look in an orthographic view leaves it perspective after the session
   (`first_person.rs:422-425`). The ruling is acceptable, but a Top view in a mosaic is lost to
   one accidental right-drag. Consider restoring the projection when the session only looked.
10. A split takes the first free letter (`mosaic.rs:446-455`). After closing `A`, a split makes a
    new `A`, which becomes viewport 1 and renumbers the rest while standing on the right. This is
    consistent with §7.8, but surprising; prefer the next letter after the highest in use.
11. `transport.png`: at 3.20 s the second shot's bar is lit, but the field says `shot 1`
    (readouts so far). One of the two should change so they agree.
12. `crates/cav-app/src/lib.rs:8-15` ("What this crate is for") does not mention the mosaic's
    window side, the header, the palette, the transport or the physical camera. ARCHITECTURE
    covers them; the crate's own header should name them in a line.
13. Plot titles ("Flight · the atoms' arms, height against absolute time · point") are sentences
    in a viewport. They are borderline under "no prose in the chrome"; consider moving the gloss
    to the tooltip.

## Checked and sound

- **The flaky-test fix** (`s20.rs:1498-1505`, `last_said`) keeps the property. It skips only the
  forward worker's `signal: ` notes, the one asynchronous line, which `engine.rs:1775` writes.
  It still requires the refusal's own words, and the scene's unchanged state is asserted
  separately. A missing refusal still fails: the line before does not contain the expected
  words. No other S20 test reads the last log line after frames.
- **Walker collision** (`walk.rs`) uses swept axis-by-axis clipping. W1 checks "never inside
  matter" on every frame, with two controls (it falls for ever with nothing solid; it passes the
  wall with noclip). `nothing_is_skipped_at_any_speed` stops a 1000 m/s camera at a 1 cm wall,
  with a no-sweep control. W2 runs against the real `town`, a voxel sphere and the room, each
  with a noclip control. W4 is bit-identical with a 70 kg control.
- **Path sag**: D1 measures the exact chord sag of a 2 m circle at 0.01, 1 and 50 Hz against 1 %
  of the radius. Its control is the S19 rule kept in the test (29 % at 50 Hz). Vertices lie on
  the path to 1e-9 m, and the lifetime is exact. `path.rs`'s general test is looser (2 % of the
  extent) but covers every kind of motion, with a capped control.
- **Folder serialisation**: F1 pins an FNV hash of every library scene's RON and JSON
  (`spec.rs`, `library_bytes_hash`). It asserts that "folder" never appears in an unused scene
  and that the read is bit-identical with folders. Controls: a mass change is seen; an unknown
  folder is refused in words; `skip_serializing_if` off was seen failing. The hash's provenance
  (taken before the fields existed) is the builder's word. It is not re-derived here.
- **Keyboard model in flight**: `shell.rs:2101-2111` routes every press to `fly_keys` and strips
  key and text events before any widget. `WHILE_FLYING` is five harmless actions, and a test
  checks that none of them is a chord of a key held to move. C3 covers W, A, Q, 2, Delete,
  Space, Ctrl+S, Ctrl+Z, Ctrl+Space and the backtick, with a not-flying control.
- **Docs**: the ARCHITECTURE sections and both new recipes name things that exist (checked by
  hand, and by `recipes::the_recipes_name_things_that_exist`). Crate dependencies are as
  `cav-app`'s header and `the_crates_depend_as_the_plan_says` say.
- **UX against the owner's wants**: dense panels, a full-width Description box, a floating
  palette with flyout, real transport buttons, folders, a subplot_mosaic-style mosaic and the
  gizmo are all as asked, in the pictures. The game camera's keys are as asked, except findings
  1 to 3.

## The builder's calls

| Call | Judgement |
|---|---|
| Run on as a scene edit | Acceptable for now (the brief asked for it). It marks the scene unsaved and adds an undo step just by playing, and each extension in `town` costs about 12 s of world read. TASK-S22-01's live mode should replace it. |
| Flying collides by default; the room has no door | Colliding by default: yes. The room as a two-sided shell: no. See finding 1. |
| Target 1 m ahead, near plane 2 cm | Sound: the near plane is well inside the 0.3 m half-box, and the distance is restored at the end. Depth precision is a minor cost (finding 8). |
| Orthographic turned to perspective | Acceptable; consider restoring it (finding 9). |
| Default fly speed 4 m/s | Fine for 10 to 200 m scenes, and the wheel covers the rest. |
| 22 pt row pitch, off the 4 pt grid | Acceptable. N1's ≤ 24 pt holds and the tree is at 20 pt. Note it in `workspace.md` §0 so the design and the code agree. |
| Home's undo naming a pane by number | Not acceptable to leave: it is a wrong-target undo with a cheap fix (finding 4). |

## gpu tier

`cargo xtask gpu`: **42 of 42 pass** in 12.7 s, at the floor.

## Re-check of the fixes (TASK-S20-15, PR #233)

Fresh re-check on `main` at bea2c5d, in a new worktree. I read the fixes against `S20.md` §7.5 and
§7.11. `cargo xtask quick` gives **712 of 712**, at the floor (a cold build, 326 s), and `cargo
xtask gpu` gives **42 of 42**. W2 (`slow_walk_…`, the slow tier) passes. `docs/validation/` is
byte-identical to `s19-editor-second-half`.

To mutate, I reverted each fix by hand, ran its tests, then restored it:

| Finding | Mutation | Result |
|---|---|---|
| 1 | room solid always | the three tests fail: `the_room_holds…` ("outside, east of it"), `walking_from_outside…`, W2 |
| 2 | the wheel and read-out always the flying speed | `the_wheel_while_walking…` fails |
| 3 | `sprint_held` ignores the platform | `ctrl_sprints_natively…` fails |
| 4 | Home's undo by number again (B → slot 1) | `homes_undo_follows…` fails ("B now viewport 1") |
| 7 | the float ignores the dock button | `a_pop_out_floats…` fails |
| 7 | the float as big as its slot (×1.0 for ×0.5) | **passes**: see below |
| 9 | the projection not put back | `an_orthographic_view…` fails |
| 10 | the first free letter again | `split_close_and_swap…` fails |
| 11 | `shown` = `read` | `a_place_is_among…` and `each_transport_button…` fail |

**Changed expectations (§7.5).** None is a weakening.
- W2's last arm now asserts the new rule (walking back in passes the wall) with a control (once
  inside, the wall holds).
- The mosaic's split letter (B → D) and the shot field (1 → 2 at 3.2 s) follow the behaviour each
  finding asked for, and each has its control.
- The place test's move from 4.0 s to 4.2 s only takes it off the third shot's launch instant. At
  both times `read`, `earlier` and `later` are the same.

**Pictures.**
- `mosaic-float.png`: the float is half its slot, offset, and the dock button is visible under it.
- `transport.png`: shot 2 beside the second lit bar.
- `fly.png`: 4.3 m/s while walking.
- `mosaic.png` and `mosaic-six.png`: the terse titles.

**Left (minor, none blocks the tag):**
- Minor 7's "smaller than its slot" is not held by a test. The 0.45-of-figure cap makes 1 × 1 and
  2 × 2 slots smaller anyway, and the size guard skips the 3 × 2 cells (286 pt < 400). One more
  arm with a 2 × 1 or 3 × 1 slot, or a guard of 2 × `MIN_FLOAT` on the *figure*, would hold it.
- Finding 3's test holds `sprint_held` and the texts but not the call in `Shell::fly_frame`.
  Reading `i.modifiers.ctrl` there directly would still pass. The `beforeunload` guard is untried
  in a browser, as recorded.
- In `mosaic-float.png` the floated Flight plot is cramped (its x label is cut to "absolute
  time,…"). This is cosmetic.

The S20 review's leftover note (the 22 pt inspector pitch, off the 4 pt grid) is now in
`docs/design/workspace.md` §0.

**Verdict: ready to tag.**
