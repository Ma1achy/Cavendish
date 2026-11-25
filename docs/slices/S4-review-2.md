# S4 big scenes: second review (25 Nov 2026)

Reviewed: `main` at `df96192` (the fixes, PRs #143 to #151, TASK-S4-12 to 20) against the first
review (`docs/slices/S4-review.md`), `docs/slices/S4.md` (§7.6, §7.7), `plan/REBUILD.md` and
CLAUDE.md's quality bar. The diff is `9e5d04c..df96192`.

Method: I read the fixes and their tests. I ran `cargo xtask quick` (536 of 536 passed; 35.1 s,
over the 30 s budget on this run, with the machine also serving two read-only helpers) and
`cargo xtask strays` (nothing). I ran no app, no GPU tier and no demo: the demo reads the full
`stress-1e10` in its bench, which this review may not do; the screenshots in `docs/slices/S4/` are
judged as committed. Two helpers reviewed parts read-only: (a) I2, I3 and I12 end to end; (b) the
renderer, streaming and cache (I4 to I9). I checked their important findings against the code.

**Verdict:** most fixes are real and their tests bite. The slice is not yet ready for its tag. Five
findings are important:
- three are faults in the fixes themselves (the stray guard, I10; the worker lane, I11; the
  mesh cap, I7);
- two leave a fix short of what it claims (I2, I12).

All five are small fixes.

## Important

### J1. `strays --stop` can still stop a process nobody it knows started (I10)
**Where:** `scripts/strays.ps1:177-181` (an ended parent), `xtask/src/main.rs:91-92` and
`xtask/src/strays.rs:26` (every run records itself).

**(a) Pid reuse through an ended parent.** When a candidate's parent has ended, it counts as
xtask's if the parent's id is recorded and the child started at or after the record's window
*opened*. There is no upper limit on that start time. A run that is killed part-way leaves its
record for a week (`RECORD_LIFE_S`), and agents' tool timeouts kill `cargo xtask` runs. Windows
reuses ids quickly.

Scenario:
- a killed `cargo xtask demo` leaves `xtask-5120.txt`;
- two days later the owner's shell, now pid 5120, starts `python -` in the background and exits;
- after 30 minutes the next agent's `strays --stop` (which the Stop hook tells every agent to
  run) kills the owner's script.

The live-parent path is sound (`Test-Recorded` holds the parent to its window, and parents must be
older than their children). The self-test has no arm for an ended parent, so neither the rule nor
its hole is tested.

**(b) The owner's own `cargo xtask` runs are recorded too.** `Record::start` runs for every
invocation, whoever typed it. The owner's `cargo xtask web serve` (no `--exit-after`), or a long
demo, becomes "xtask's" and stoppable once it is 30 minutes old. Meanwhile
`scripts/strays-hook.ps1:27` tells agents that `--stop` stops "only the STRAY ones an agent
started". S17's second review (item 3) asked for exactly that rule; what was built is "xtask
started".

**Fix:**
- An ended parent counts only if the child started inside the parent's record window (the child
  of a crashed xtask was started while that xtask ran, so the window must have an end). Either
  have xtask refresh its record's end time every minute while it runs, or record each child it
  spawns (`{pid, start}`), as the first review asked.
- Write the record only when an agent runs xtask: Claude Code sets `CLAUDECODE=1` in its shells.
  Alternatively, mark agent runs in the record and let `--stop` act only on those.
- Add self-test arms:
  - an orphan whose recorded parent's window ended before the orphan started: not stopped;
  - a record without the agent mark: not stopped.

### J2. The page's fallback now runs a panicking job on the page's thread, and respawns never recover (I11)
**Where:** `crates/cav-app/src/lane.rs:153-165` and `:193-200`; `Cargo.toml:103`
(`[profile.web] panic = "abort"`).

**(a) A panic becomes a page crash.** The fallback used to fire only when no worker had ever
loaded. It now fires whenever no worker is ready *now* and a job has waited 15 s. That happens
during respawn back-off (the third wait is 16 s) and for good once both slots are dead.

Scenario: a job that panics deterministically (a bug for one scene) kills slot 0. An edit asks
again, and it kills slot 1. Repeat a few times: both slots are dead, or both are backing off for
16 s. The next time that job is asked for, the page runs it itself. On wasm a panic aborts, so the
whole page traps and the user loses the session. A worker failure, which was contained, becomes
fatal, and this is the very failure mode S17's item 2 was about.

**(b) The respawn count never resets.** `respawns` only grows. In a long session, three
unrelated failures for each slot (a transient load error, or one panicking job per edit) leave
both slots dead for good. Every later voxel job then runs on the page's thread, 8 to 9 s each,
one after another.

**Fix:**
- Never run here a job whose request already failed a worker: keep a set of the failed requests'
  hashes, answer those with nothing, and say so.
- Keep the page-runs-it fallback for "no worker has loaded", as before, or for workers that never
  became ready, not for workers that died in a job.
- Reset `respawns` (and `dead`) when a worker in the slot answers a job.
- Test, in the lane's native tests: the same request fails both workers until neither is ready;
  the lane must never return `RunHere` for it. Control: a different request is run here.

### J3. At a mesh cap, a moving camera re-meshes every frame, and a re-mesh can drop a drawn chunk (I7)
**Where:** `crates/cav-render/src/world.rs:741` (the order), `:746-754` (`fingerprint`) and
`:884-914`.

**(a) The fingerprint depends on order.** It hashes `todo` in the order of the sort at `:741`,
which is nearest the eye first. At the vertex or slot cap, orbiting or flying reorders the waiting
chunks almost every frame. Each reorder changes the fingerprint, so the frame meshes up to 96
chunks (`MAX_MESHES_PER_DRAW`) and throws them away. That is I7's symptom, now confined to the
interactive case, which is where it matters.

The test (`gpu_meshes_past_the_vertex_cap_are_not_drawn`) re-prepares the same view. Its "moved
camera" uses a new frame, so every pointer changes anyway.

**(b) A stale mesh is dropped, or never replaced.**
- The old mesh is removed before the new one is checked against the cap (`:884-887`). If a
  neighbour has left, the new mesh is larger. If it does not fit, its slot is freed (`:908`), and
  a chunk that was drawn vanishes.
- Missing chunks sort before stale ones, and the loop `break`s at the first that does not fit. So
  at a cap no stale chunk is ever re-meshed, and faces against a departed neighbour stay culled
  as holes.

**Fix:**
- Make the fingerprint order-free: sort the `(stale, key, ptr)` entries by key, or sum per-entry
  hashes.
- If a re-mesh does not fit, keep the old mesh.
- At a cap, `continue` past a missing chunk that does not fit rather than `break`.
- Tests:
  - the same frame with the eye nudged makes no mesh;
  - a capped version of `gpu_a_neighbour_arriving_buries_faces`, in which no drawn chunk
    disappears.

### J4. The truncation is untested above `cav-sim`, and figure exports drop it (I2)
**Where:** `crates/cav-engine/src/engine.rs:1195`, `:1361` (`SignalReply`/`ReadingReply`);
`crates/cav-sim/src/read.rs` (`reading`'s `ReadingAt`); `crates/cav-plot/src/figure.rs:409-433`
(`provenance`).

**Untested.** No test asserts `truncated` or `settled` on the engine's replies, which are what the
API and MCP serve. `same_bits` in the wire test compares the direct read with the one through the
wire, so if `reading` dropped the flag, both would be `None` and the test would still pass.
`an_unsettled_reading_says_so` tests only `estimate_text`.

Scenario: a refactor writes `truncated: None` in either reply. Every tier stays green, and API
and MCP clients see `town` as settled again. That is I2's original bug.

**Figure exports.** `export figure signal …` of `town` writes CSV and JSON whose provenance has the
scene and the command but not `settled`, `truncated` or the warnings. I2's rule was "anything that
reads the numbers".

**Fix:**
- An engine test: `world-box` with `read.radius = 1`, settled; `Signal` and `Reading(Some(1.0))`
  each give `(Some(1.0), false)`, and their JSON has `"truncated"`. Control: the whole box gives
  `None` and `true`, and no key.
- Assert `read::reading(..).truncated` in the wire test, and `sent.truncated.is_none() &&
  sent.settled` for the whole box. The comment at `wire.rs:324-326` claims this check, but it was
  dropped.
- Put `settled`, `truncated` and `warnings` in the figure's provenance.

### J5. The outliner still says a world scene reads nothing (I12)
**Where:** `crates/cav-app/src/panels.rs:548`: `let silent = bodies.1 == 0;`.

**What the user sees.** The new world row says the world is read. But in `town` and `world-box`,
the Array section shows each tower's x and y instead of what it reads. The "N reading(s) warned
of" line (`:562`), which would carry the truncation warning, is hidden too. `world-near.png` shows
this: the tower rows read "+0.0 +0.0".

The truncation warnings are general, with `tower: None`, so no tower row would get its ⚠ even
with the fix.

**Fix:**
- `silent = bodies.1 == 0 && world.is_none() && floor.is_none()`.
- Mark the tower rows when `reply.truncated.is_some()`.
- Extend `the_world_is_in_the_outliner_and_can_be_selected` to look for a tower's reading. Its
  control: `point` without bodies.

## The flythrough, ~19 to ~31 ms (the I6 fix)

Honest and recorded: §7.4 keeps the first table, adds the second with its reason, `flythrough.txt`
now says "300 frames of the 300 asked", and PROGRESS lists the 99th percentile over 50 ms as open.
The mechanism is plausible (before the fix the nearest-first budget was filled by bricks behind the
camera, so much of the visible far field was never drawn: the first table measured a thinner
picture). Two gaps:
- The claim that "the cost is in the nearest visible bricks" is a story, not a measurement. A cap of
  6 saving only 1–2 ms of the 12 ms added is the one number that tests it, and it fits several
  stories (the estimate `brick_pixels` overcounting near bricks, or the cost being elsewhere in
  the draw). CLAUDE.md asks for a measurement: the draw with bricks off, or the per-frame `marched`
  column against `draw_ms` from the CSVs already written (a correlation would settle it).
- The tracking is stale: `plan/IDEAS.md` "Early depth for the ray-marched bricks" still says the
  99th percentile is "50–57 ms"; it is now 59–64 ms. `MAX_BRICK_OVERDRAW`'s doc says the cap was
  chosen so a typical frame stays well under ~50 ms "after which Windows' desktop stops answering";
  with the cap no longer binding, 1% of frames are past that and nothing bounds them but the
  bench's 250 ms alarm. Say so in the doc and IDEAS (minor 1).

## I1, looked at hard

- **The bound is right.** `|∇(P_l(cos γ)/R^{l+1})|² = ((l+1)² P_l² + (1−x²) P_l'²)/R^{2l+4}`, and
  with `P_l² + (1−x²)P_l'²/(l(l+1)) ≤ 1` that is at most `(l+1)²`: the factor is exact on the axis
  and `the_legendre_gradient_bound_holds` checks it with a control. The cube's `∫ s⁴ dm =
  m(d⁴ + 5d²h²/6 + 19h⁴/240)` is right and tested numerically. The tail from l = 17,
  `powers[12]·a^{l−16}/R^{l+2}`, is indexed correctly (`distance.powi(BOUND_ORDERS + 5)` is R¹⁸).
- **Measured over every q?** Over every q in `MOMENT_QS` (0.45 to 0.001), where the error is above
  1e-13 of the reading. But `checked >= 2 * MOMENT_QS.len() - 2` lets two (q, arm) pairs drop out
  without saying which, and §7.3 gives values down to 0.002 only. If q = 0.001 is the one under
  rounding (it likely is: the error falls as q⁴), "over every q tried, 0.45 to 0.001" in the
  report's `against` text is one q too wide (minor 2).
- **Do the controls bite near q ≈ 0.1?** Only the quadrupole left out does (3× at 0.1), which is a
  gross fault. The octupole is guarded from 0.05 (flipped) and 0.02 (left out). So between 0.05 and
  0.45 an octupole fault of up to about a quarter of the bound would pass R7. That is acceptable
  only because R7's main assertion (error ≤ bound) is what keeps the reported estimate honest: an
  octupole fault under the bound is still covered by the estimate.
- **Is §7.6 honest?** Mostly. The tolerances (1 and 1000) did not move, the control's change has
  its line, and it is scoped to "any bound from `Σ m s^l` and `|P_l| ≤ 1`"; IDEAS says how to get
  the written control back (the hexadecapole). One inconsistency: §7.6 says at q = 0.1 the
  octupole is "about 30 times" the remainder and 0.27 of the bound, but §7.3 measures the bound at
  220× the error at q = 0.1, so 0.27 of the bound is about 60× the error. Both cannot be the same
  measurement (minor 3). The rule CLAUDE.md states (never move a criterion to pass) was kept in
  substance: the looseness is now measured where it was hidden, and it passes on a tighter bound,
  not a wider range.

## Fixed and checked (the first review's important findings)

| Finding | Verdict |
|---|---|
| I1 | fixed; see above |
| I2 | fixed in the read, the wire (flag byte; `None` and `Some(0)` distinct), the bundle manifest, the replies (`serde(default)`), and every text through `estimate_text`; but untested above `cav-sim` (J4), and figure exports drop it (J4) |
| I3 | fixed: world-box through the wire whole and in part, in `web_equivalence`, in a bundle; superposition with a control (world left out). The 1e-15 is by construction (same `Signal::add` order): it guards that the world is summed, not superposition as physics |
| I4 | fixed: the resident and bytes caps reached with small caps, each with a control (the old check-before-batch code fails `<= bytes` even on one thread); `step(usize::MAX)` makes 256; memtest under 1 MiB with an uncapped control |
| I5 | fixed by reading: `GpuPanes::notes` → `shell.render_notes` → header badge and log; no test (minor 5) |
| I6 | fixed: `choose_bricks` filters by the frustum (a p-vertex test, so partly visible bricks stay) before the budget; its control fails as it should |
| I7 | half fixed: a still view is not re-meshed; a moving one is (J3) |
| I8 | fixed: `VOXELISER_VERSION` and the format in the folder; `create_new`; unreadable regions left alone. Nothing in the fixes changes voxelising, so version 1 is right |
| I9 | fixed: `nonzero` in the shader and the twin; `1/dir ≤ 1e6`, finite in f32 at kilometre scale; `a_ray_along_an_axis_marches` is the real control (minor 7) |
| I10 | the record and the reuse check for a live parent hold; the dead-parent path does not (J1) |
| I11 | the lane is a clear, tested state machine; two rules it introduced are wrong (J2) |
| I12 | the world row and `select world` work with a control; the Array section still treats the scene as silent (J5) |

## Validation report

`docs/validation/report.json` against `s17-in-the-browser`: 113 cases, 107 unchanged to the bit
(measured, tolerance, unit, reference, status), 6 new world cases, none dropped or missing, all
pass. Against the report before the fixes (`9e5d04c`) only the two R7 rows changed (656 → 457 and
0.0106 → 0.0228), with their `against` text; no tolerance moved. Sound.

## Docs

`docs/extending/a-world-piece.md` gained step 6 (bump `VOXELISER_VERSION`), which names a constant
that exists where it says; `docs/ARCHITECTURE.md`'s rows for the world, the cache and the drawing
still match. `cav_app::lane` is not in ARCHITECTURE's where-things-live table (minor 4). The
recipe test (`the_recipes_name_things_that_exist`) passes.

## Minor
1. **The flythrough's tracking.** `plan/IDEAS.md` (early depth) still gives the 99th percentile as
   50–57 ms; it is now 59–64 ms. `MAX_BRICK_OVERDRAW`'s doc
   (`cav-render/src/world.rs:55-60`) should say that the cap no longer binds and that 1% of frames
   pass the ~50 ms it aims at. The claim that the cost is "in the nearest visible bricks" needs a
   measurement: `draw_ms` against `marched` from the CSVs already written, or a run with bricks
   off.
2. **R7's loosest bound** (`cav-sim/src/world/tests.rs`, `checked >= 2 * MOMENT_QS.len() - 2`)
   lets two (q, arm) pairs drop out under rounding without naming them. Assert which ones (likely
   q = 0.001), or narrow the `against` text to the qs actually measured.
3. **§7.6's R7 line** says the octupole at q = 0.1 is "about 30 times" the remainder. §7.3's 220×
   looseness at 0.1 makes 0.27 of the bound about 60×. Give the measured number.
4. **`docs/ARCHITECTURE.md`** has no row for `cav_app::lane` (the worker lane's rules), nor for the
   stray records under `target/strays/`.
5. **I5's badges have no test.** The log keys a note's kind by the text before ':', so the vertex
   and slot caps, both "near field: …", log only the first. Give each kind a fixed tag, and add a
   shell test with a fake `PaneSurface`.
6. **I2's text:**
   - the header badge reads "not settled: of the matter within 2 m only, estimate 1e-8 rad", and
     "estimate 1e-8 rad" next to a reading that is off by order one invites misreading. Say
     "quadrature of what was read within 1e-8 rad";
   - `short_metres` prints a radius under 5 cm as "0 m" (`engine.rs:57`);
   - the world's properties print `{within}` unrounded (`panels.rs:834-840`);
   - "1 pieces" (`panels.rs:466`).
7. **I9's GPU arm** checks only that the CPU and GPU agree, so both could walk the same wrong
   path. Assert that the axis rays hit their row's first occupied cell, as the CPU test does.
8. **I2's edges:**
   - a refused world read leaves `truncated` as `None`, against the manifest's "null when every
     source's matter was read" (`read.rs:944-951`);
   - `BUNDLE_VERSION` stayed 1, so a world bundle written before #144 is indistinguishable from a
     whole read;
   - `PeriodogramReply` carries no flag;
   - the manifest's `"sources"` counts bodies only.
9. **I8's leftovers:**
   - a crash between `create_new` and the header leaves a short file, and its region is passed over
     in every later session (`cache_native.rs:153,163`). Write the header to a temporary file and
     hard-link it into place, or replace a short file over a minute old; the note should say
     "delete it to cache again";
   - `load` re-opens passed-over regions;
   - the old `<hash>/` folders are orphaned.
10. **Minor 17's transient.** A batch (threads × a chunk, about 400 KB each) is made before
    `fits` is checked, and that is not said anywhere. The memtest's fixed `cap + 2 MiB` margin
    does not scale with threads. Once full, each refresh remakes one batch and throws it away.
    The bytes cap keeps what came first, not what is nearest.
11. **The lane:** a panic message that arrives after the error event sticks to the slot and labels
    its next failure; clear it on `Spawn`/`ready`. A post that fails every time logs once a
    frame.
12. **The page's pictures** (`web-town.png`, `web-world-box.png`) predate the fixes: they still show
    "Sources: none" and the old badges, so R17's evidence is of the code before I2 and I12.
    Retake them with `cargo xtask demo S4` (its own server) at the next checkpoint.
13. **`quick`** ran in 35.1 s here, against the 30 s budget, with two read-only helpers on the
    machine. Re-measure it when the machine is idle.
14. **`world-east-ortho.png`:** dark notches at the tower bases, where the conservative coarse
    bricks stand above the fine meshes (§7.6 "surfaces thicken"). This is visual only, but it is
    the most visible seam in the demo, and it deserves a line in IDEAS.
15. **I6's slot cap:** bricks are uploaded nearest first without the frustum (`world.rs:962-980`),
    so at the brick-slot cap, bricks behind the camera hold slots, and with no eviction "(the
    nearest are)" is untrue after a move.

The first review's minors 11, 12, 14, 15 and 16 are in `plan/IDEAS.md` as §7.7 says. Concurrent
region writers are there too.

## Strays
`cargo xtask strays` at the end: no process matches. `target/strays/` holds no records.
