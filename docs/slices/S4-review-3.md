# S4 big scenes: third review (25 Nov 2026)

Reviewed: `main` at `cf1337d` (the fixes for the second review, PRs #153 to #156, TASK-S4-21 to
24) against `docs/slices/S4-review-2.md` (J1 to J5) and `docs/slices/S4.md` §7.8. The diff is
`ac7cace..cf1337d`. Focused: J1 to J5, whether each test has a control arm that fails when the
property is broken, what the fixes touched nearby, the GPU test's new precondition and
`BUNDLE_VERSION` 2.

Method: I read the code and the tests. I ran nothing but `cargo xtask strays`: no tier, no app, no
GPU test (the oracle tier is being run separately).

**Verdict:** J1, J3 and J4 are fixed, and their tests bite. Two findings are important:
- K1: J2's fix ends the page fallback that J2 asked to keep: a page whose workers never load
  computes nothing after about 21 s.
- K2: J5's code is right, but its test does not test the place where J5 broke.

Both are small fixes.

## Important

### K1. A page whose workers never load stops computing after about 21 s (J2's fix)
**Where:** `crates/cav-app/src/lane.rs:219-233` (the "every slot empty" branch), with
`:181-189` (a slot goes dead after `MAX_RESPAWNS` failures in a row, with or without a job).

J2 asked to keep the page-runs-it fallback for "no worker has loaded". The module doc
(`lane.rs:14-17`) still promises it: "the workers did not load: offline, a refused script ... the
page runs waiting jobs itself". But the new branch answers every waiting job with nothing as soon
as all slots are `dead`. It does this whatever `died_in_job` says. A slot whose script never loads
goes dead like any other.

Scenario:
- `worker.js` fails to load: a browser without module workers, a CSP that refuses it, or
  `file://`;
- each slot fails with "Script error", holding no job;
- respawns come at 1, 4 and 16 s, and the fourth failure makes the slot dead;
- about 21 s in, both slots are dead;
- from then on, every job is answered with nothing, and the page says "Reload the page", which
  fails the same way.

Before #154, the page ran these jobs itself, slowly. Now at most one job runs there: the first
one, once it has waited `READY_WITHIN_MS` (15 s). After that, no scene in the session gets
numbers.

The tests miss this. `the_page_runs_jobs_when_no_worker_has_loaded_and_takes_them_back` fails each
slot once, so nothing goes dead. `a_worker_that_answers_starts_its_count_again_and_empty_slots_answer_nothing`
kills every slot with "Script error" and no job (`died_in_job` is false), and asserts the
answer-nothing outcome. So that test now writes the regression down as the intended behaviour.

**Fix:**
- Take the answer-nothing branch only when `died_in_job`. Otherwise fall through to the page
  fallback, which `!died_in_job` already gates.
- Test: fail both slots `MAX_RESPAWNS + 1` times holding no job. The page must still return
  `RunHere`. Control: the same with one failure holding a job, which is answered with nothing.
- Change the last part of the answers test to match.

### K2. J5's test checks the helpers, not the outliner that had the bug
**Where:** `crates/cav-app/tests/outliner.rs:76-95`, `crates/cav-app/src/panels.rs:579`.

J5's bug was at the call site: the outliner computed `silent` as `bodies.1 == 0`. The fix moved
the rule into `panels::silent` and `panels::tower_detail`, and the call site now uses them
(`panels.rs:579`). The code is right.

The test calls the two helpers directly, through `panels_for_tests`. Revert `panels.rs:579` to
`let silent = bodies.1 == 0;` and `a_world_scene_s_towers_show_what_they_read` still passes. The
tower rows in `world-box` and `town` would read "+0.0 +0.0" again, which is exactly J5's symptom,
and every tier would stay green.

J5 asked to extend the rendered-outliner test (`the_world_is_in_the_outliner_and_can_be_selected`,
which runs headless frames) so that it looks for a tower's reading.

**Fix:**
- Assert the tower row's words from a rendered frame. For example, record each outline row's
  detail beside its rect in `shell.layout().outline`, or read the frame's text shapes.
- Check `world-box` (read radius 1): a tower row contains "rad" and ◐.
- Control: `point` with its body removed shows the tower's x and y.
- The helper-level asserts can stay.

## Checked and fixed

- **J1, the strays guard:**
  - `Record::start` writes nothing unless `CLAUDECODE` is set (`xtask/src/main.rs:94`,
    `xtask/src/strays.rs:39`).
  - The heartbeat moves `alive until` to two beats ahead, every 30 s, under the same lock that
    `Drop` takes, so a removed record is never written again.
  - Through an ended parent, the child must have started within `[from, alive until]`
    (`scripts/strays.ps1:186-191`). Old three-number records are read as alive until their
    latest start, which is conservative.
  - The self-test arm has a real control. One orphan of an ended PowerShell is tested against two
    record folders that differ only in `alive until`. With the window closed before the orphan
    started, it is reported and not stopped; while the window was open, it is stopped.
  - The Rust test's control fails if `agent` is ignored.
- **J3, the mesh cap:**
  - The fingerprint sorts the entries by key (`world.rs:752-760`).
  - A re-mesh that does not fit puts the old mesh and its vertices back (`:922-925`), and the loop
    `continue`s.
  - `gpu_at_a_mesh_cap_no_drawn_chunk_disappears` has real arms. At cap 44, the old code drops A
    (`[1,0,24]`), and the control at 48 re-meshes it. In (b), `break` would leave A at 24 vertices,
    and the test wants 20.
- **J4, the truncation:**
  - `a_truncated_world_read_reaches_every_reply` checks the signal, reading and periodogram replies
    and their JSON, plus `read::reading`. Writing `truncated: None` in any of them fails it.
  - The control is the whole box (no key, settled).
  - The wire test now asserts the flags that came back, not just their agreement with the direct
    read.
  - Figure records carry `signal` {settled, truncated, warnings}, with a whole-box control.
  - `SignalReply`, `ReadingReply` and `PeriodogramReply` are the only replies with numbers that
    carry the flag, and all three are covered.
- **GPU precondition (16 bricks > 11):** honest, and stricter than needed. The brick cap in the
  test is 7, and `bricks > 7 + 4` keeps a margin so the cap binds. Minor 4 below covers the
  wording.
- **`BUNDLE_VERSION` 2:**
  - Nothing in the repository reads bundles (no reader in Rust or `python/`), so there is no old
    path to break.
  - The bump marks the change in meaning of `sources`, and `bodies` carries the old meaning.
  - `docs/slices/S2/bundle/manifest.json` stays a v1 record, which is right.
  - Minor 3 covers the module table.

## Minor

1. `lane.rs:205-225`: a request a worker failed on is still posted to the next worker, so asking
   for it again kills slot after slot. Answer a request in `poisoned` with nothing at `pump`, in
   the posting loop as well as at `RunHere`.
2. `lane.rs:259-266`: `not_posted` counts as a failure in a row but does not poison the request.
   One request that can never be posted retries at the queue's front until both slots are dead.
   Drop it after one failed post, or poison it.
3. `crates/cav-sim/src/bundle.rs:26-27`: the module table's `S` in `source_position.npy` and
   `source_mass.npy` is now the manifest's `bodies`, not its `sources`. Say so in the table, and
   add a line on v1 versus v2.
4. `crates/cav-render/tests/gpu_world.rs:340` and `docs/slices/S4.md` §7.8: "16 > 11" reads as a
   cap of 11, but the cap is 7. Write `bricks > 7` with the margin explained, or name the margin.
5. `gpu_world.rs:353-359`: the nudged-eye arm does not show that the nudge reorders the waiting
   chunks, so it might pass with the old fingerprint. Assert the order changed (`to_mesh` order
   before and after), or nudge until it does.
6. `world.rs:918-931` with test (b) at `gpu_world.rs:436-442`: at the cap, A is re-meshed with its
   face toward B buried while B is not drawn, so A is see-through on that side. Mesh against the
   chunks that are drawn, or keep A's old mesh when the neighbour that buries it is not drawn.
7. `world.rs:930`: `self.full` keeps only the last note. When both the vertex cap and the slot cap
   bind, a frame skipped by the fingerprint says one of them. Keep both.
8. `xtask/src/main.rs:94`: the owner's own interactive Claude Code session also sets
   `CLAUDECODE`, so an `xtask` run through it is recorded as an agent's. Say so in the strays doc.

## Strays

`cargo xtask strays`: no process matches an agent-leftover signature.
