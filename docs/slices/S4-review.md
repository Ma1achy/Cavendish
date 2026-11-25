# S4 big scenes: first review (25 Nov 2026)

Reviewed: `main` at `d902275` (PRs #128 to #133, #137, #138, #141; TASK-S4-01 to 11) against
`docs/slices/S4.md`, `plan/REBUILD.md` (§3.4, §5, §6, §9 row S4), `plan/VALIDATION-AND-RESEARCH.md`
and CLAUDE.md's quality bar. The diff is `s17-in-the-browser..d902275`.

Method: I read the code and the tests. I ran no build or app; the only command was `cargo xtask strays`,
which found nothing. Two helpers reviewed parts of the slice read-only: (a) the renderer, streaming and
region cache; (b) the worker lane, the stray guard and xtask's processes. I checked their main findings
against the code (I1–I9 by reading). Finding I9 is the one I did not reproduce, and it says so.

**Verdict:** the slice is not ready for its tag. Eleven findings are important: two are physics
honesty, three are missing or narrowed tests, four are renderer or cache bugs, two are the stray guard
and the worker lane. Most fixes are small.

## Important

### I1. R7's criterion was narrowed after measuring, and the measured value fails it
**Where:** `crates/cav-sim/src/world/tests.rs:263-293`, and the brief at `docs/slices/S4.md:120,226`.

**What the brief says.** The criterion was written in #128 (`56c0fb6`), before anything was measured:
- "error ≤ bound everywhere tried; bound ≤ 1000 × error";
- "control: the octupole left out breaks the bound at q ≤ 0.1".

**What the test does instead:**
- (a) It measures how loose the bound is only for `q >= 0.05`. Line 277 says "as the brief says", but the brief gives no such range.
- (b) It moves the control from q ≤ 0.1 to q = 0.002 (line 286). At q = 0.1, leaving out the octupole does not break the bound, because the bound is too loose there.
- (c) At q = 0.002, the one point excluded from the measurement, the bound is 1054 times the error. That is over the 1000 that was written down.

§7.3 explains this as "the range the criterion was written for". That range was not in the brief before the test existed: the whole test arrived in one commit (`a496fc6`).

**Why it matters:**
- This is the case CLAUDE.md forbids: a criterion moved to pass.
- Small q is exactly where chunks are read through their moments (the far chunks). So the bound is loosest where it is used, and the stated control does not work in the range where the bound is applied.

**Fix:**
- Measure looseness over every q tried, so the report shows 1054 and fails against 1000.
- Then do one of two things:
  - tighten the bound, for example by summing `Σ m s^l` for each l from the leaves, which `Multipole::of` already visits, instead of `fourth · a^(l−4)`, and by using the l = 4 term's own factor;
  - or change the tolerance with a line in the slice notes saying why.
- Restate the control as what it really guards ("the octupole left out breaks the bound for q ≤ 0.002") and add a control that works at q around 0.1. A sign error in `third` is one candidate.

### I2. The big scenes' truncated read is reported as settled, with an error of 1e-8 rad
**Where:**
- `crates/cav-sim/src/read.rs` `world_contribution` (`estimate = audited.estimate + bound`; `settled` follows the quadrature only);
- `scenes/dev/town.ron:7`;
- `docs/slices/S4/readings.txt`.

**What the user sees.** `stress-1e10` and `town` read only the matter within 2 m of each tower. The reading of `town` is about 1.6e3 rad a tower. Every place it appears calls it settled and accurate to 1e-8 rad:
- the header: "refined · quadrature within 1e-8 rad";
- the pane badge: "signal · within 1e-8 rad";
- the Run line: "refined · within 1e-8 rad".

**Why that is wrong.** The matter left out changes each interferometer by order one, relative. A 2 m disc seen from 5 m up gives about 7% of an infinite slab's field, and the 8 buildings are left out entirely.

**What is already good.** The warning is honest and appears in the status bar and in the log.

**Where it still fails.** Anything that reads the numbers gets `settled = true` and a tiny estimate: the API, MCP, a bundle, Gradar. That data would be wrong while carrying a stamp of accuracy.

**Fix:**
- When `read_keys` says matter was left out, mark the contribution unsettled, or carry a `truncated` flag through `Contribution`, the wire and the bundle.
- Have the header and badge say "of the matter within 2 m".
- Add the 2 m line to `town`'s description: `stress-1e10` has it, `town` does not. `town`'s description also has a run of spaces: "about          5x10^7".

### I3. R8 is untested apart from "static"
**Where:** `crates/cav-engine/src/forward/wire.rs:280-293`, `crates/cav-engine/tests/web_equivalence.rs`, `crates/cav-sim/src/world/tests.rs:324`.

**The wire.** R8 asks for "the same bits through the wire", but no scene with a world goes through it:
- `a_job_through_the_wire_is_the_same_bits` runs `point`, `comb` and `pass`;
- the ignored arm runs `sphere`;
- `web_equivalence` has no world.

The web build reads worlds in its workers through `WORLD = u64::MAX − 1`. A bug in encoding or decoding that index, or in `valid`, would pass every tier.

**Superposition.** The criterion is "world + body = sum to 1e-15", but `the_world_is_a_source_like_any_other` never sums anything. The bundle arm is also missing.

**Fix:**
- Add `world_box()` (small: 4 chunks) to the quick wire test.
- Add a superposition assertion against `read::reading`.
- Round-trip a bundle of `world-box`.

### I4. Bound tests named in §3.1 do not exist, and R14 never reaches its caps
**Missing tests:**
- `slow_streaming_memory_is_bounded` (W5, memtest);
- `meshes_past_the_vertex_cap_are_not_drawn` (W7).

§7.5 quietly changes W7 to "said as notes when reached", and §7.6 has no departure line for it. Those notes are never shown anyway: see I5.

**R14** (`crates/cav-engine/src/world.rs:407`) does not test the caps:
- `step(5, …)` tests the `most` argument, not `MAX_BUILDS_PER_POLL`;
- no case wants more than `MAX_RESIDENT`, so the farthest-first truncation, with towers kept, is never run;
- no case reaches `MAX_RESIDENT_BYTES`.

§3.1 also names `MAX_RESIDENT_CHUNKS`; the code calls it `MAX_RESIDENT`.

**Fix:** write the two tests, each with a control: a tiny cap passed as a parameter, a scene that wants more than the cap, and `step(usize::MAX)` making at most 256. Alternatively, amend §3.1 with a departure line.

### I5. The renderer's "not drawn" notes never reach the user
**Where:** `crates/cav-render/src/world.rs:726,732,774,819` write `WorldGpu::notes`. Their only reader, `Renderer::world_counts` (`renderer.rs:303`), is called by:
- `bench.rs:104`, which discards them (`_`);
- `gpu_world.rs:323`, which also discards them;
- never by `cav-app`.

**What drops silently:**
- the vertex cap;
- the slot cap;
- the brick-slot cap;
- the overdraw cap. §7.6 says this one binds every frame of the bounded flythrough.

**Why it matters.** REBUILD §5: "visual-only shortcuts are fine; the viewer labels them". The module's own doc also promises the notes.

**Fix:** carry them to the pane's status as the engine's `take_notes` is carried (`engine.rs:1358`). Show them as a badge when the far field is cut.

### I6. The far field's overdraw budget is spent on bricks behind the camera
**Where:** `crates/cav-render/src/world.rs:646-650`, `801-817`.

**The bug.** `far` is filtered by the level boxes only, which surround the camera. `order` sorts every brick by distance, and `within_budget` then keeps the nearest prefix, with no frustum test first. Bricks behind the camera are often the nearest, so they use up `MAX_BRICK_OVERDRAW`, and visible far terrain is dropped.

**Consequences:**
- §7.4's frame times and `marched` are measured with this waste;
- the "farthest bricks left out" are partly the visible horizon.

**Fix:** filter with `visible(&frustum_planes(..), lo, side)` before the budget; the planes are already made at `:874`. Add a test in which bricks behind the camera cannot push out the ones in front.

### I7. At the vertex or slot cap, every frame re-meshes up to 96 chunks and throws them away
**Where:** `crates/cav-render/src/world.rs:719-735`.

**The bug.** When the cap is reached, the loop `break`s. The remaining chunks are neither stored nor marked in `empty`, so `todo` holds them again next frame and greedy-meshes them again, on threads, every frame for as long as the cap holds.

**Fix:** remember that the cap was reached, keyed to the resident set, and stop meshing until that set changes or a slot is freed.

### I8. The region cache can serve stale chunks, and it truncates files it cannot read
**Stale chunks** (`crates/cav-engine/src/world/cache_native.rs:37-41`, with `WorldSpec::hash`, `cav-sim/src/world.rs:307`). The folder and the header hash cover the spec only, and `VERSION` covers the byte format only. Two kinds of change are invisible to the key:
- a change to how chunks are voxelised, such as terrain classification or `cover`;
- a change to the precedence rule.

After either, a warm cache returns the old chunks with no warning. The flythrough "from a warm cache" and the page are exactly where nobody would notice.

**Truncation** (`:100-107`). `write` overwrites any region whose header reads as `None`. That includes:
- a file of another `VERSION`;
- a file another process is still creating;
- a file that is momentarily unreadable.

So two builds sharing one cache folder wipe each other's regions.

**Fix:**
- Put a `VOXELISER_VERSION` constant and `VERSION` into the folder name, and bump the constant with any change to voxelising.
- Create region files with `create_new`.
- When a region file exists but cannot be read, stop caching that region instead of rewriting it.

### I9. The ray-march may break on rays parallel to an axis (reported by the helper; not reproduced)
**Where:** `world_render.wgsl:125-133`, `world_common.wgsl:71-85`.

**The problem.** In an orthographic named view (E, W, N, S, top), `dir` has components that are exactly zero. The march takes `1/dir`. WGSL lets an implementation assume there are no infinities, so `(s·8 − start)·inf` can turn into NaN when `start` lies on a sub-block plane. The CPU twin's doc assumes "no component zero", and R11 only tries rays with no zero component (`gpu_world.rs:131`).

Named orthographic views are a core feature (§6) and are what the demo uses.

**Fix:**
- Clamp each `|dir|` component to at least 1e-6, keeping its sign, in both the shader and the twin.
- Add rays with zero components to `gpu_brick_march_matches_the_cpu`.
- Take a `look E` orthographic picture of `town`.

### I10. The stray guard's `--stop` still acts on processes it cannot attribute
**Where:** `scripts/strays.ps1:103-104, 118-134, 190, 197`. S17's second review (item 3) asked that `--stop` touch only processes an agent started.

**`'agent'` rule.** `python -` and headless browsers with a scratchpad profile are stopped on their command line alone. The owner piping a script into `python -` for over 30 minutes would be killed.

**Ancestry rule.** It accepts *any* `claude.exe` or `node … claude` ancestor:
- that includes the owner's own interactive session and another agent's live work;
- CPU over 600 s counts as stray, and a release `cav` bench on 10 cores passes that in about a minute, so one session's `--stop` can kill another session's run mid-way;
- `node … claude` matches any path containing `.claude\worktrees\`.

**Misses real leftovers.** A process started in the background is orphaned when its shell exits, so `Test-AgentAncestor` returns false (`:122`) and it can never be stopped.

**No test.** `strays.test.ps1` has no arm for the ancestry rule (no fake parent, and no control without one). Its header says "four" processes and then "these three" (`:12`).

Given the owner's standing rule (never kill a process another did not start), this is important.

**Fix:**
- xtask records `{pid, start time}` of everything it spawns, under `target/strays/`.
- `--stop` acts only on processes listed there, judged by wall-clock age.
- Everything else becomes report-only.
- Add a test arm for a listed process and one for an unlisted one.

### I11. A failing Web Worker is respawned without limit, and the page fallback never comes back
**Where:** `crates/cav-app/src/platform/web.rs:437-460, 485`.

**Respawn loop.** `failed` terminates the worker and spawns a new one at once, with no limit. If `worker.js` cannot load (offline after the page loaded, a 404, a MIME error), the new worker fails too. Each round:
- leaks two `forget()`-ed closures;
- logs an error;
- rewrites the note on the page.

This repeats for as long as the page is open.

**Fallback.** `any_ready` never goes back to false. If both workers die after once being ready and their replacements never become ready, jobs queue forever and the page never runs them itself.

**The message shown.** The page shows the trap ("RuntimeError: unreachable"), not the panic's message: the worker installs no panic hook, so the S17 item 2 wording ("shows the message") is only half met.

**Fix:**
- Cap respawns for each slot (for example 3, with back-off).
- Base the fallback on "no worker ready now and a job waiting longer than N s".
- Have the worker post its panic message before it traps.
- Lift the lane's state into a plain struct with a clock and a post closure, so the native tests can cover it. §7.6 notes that no test forces a failure; this makes one cheap.

### I12. The GUI says a scene with a world has no sources and no surroundings
**Where:** `world-near.png`, `world-box.png`, `web-town.png` (all three show it), drawn by the outliner (`cav-app/src/panels.rs`).

**What it says.** "Sources: none · `load point` has one" and "Surroundings: nothing: no room, no floor, no air", in `town` and `world-box`.

**Why that is wrong.** The world is a source (`SourceId::World`) and is the scene's surroundings. A newcomer reads the panel as saying the scene is empty, and cannot find or select the world, or its read radius, anywhere in the GUI.

**Fix:** add a "World" row (pitch, pieces, materials, read radius and tolerance, chunks resident) under Surroundings or Sources.

## Minor
1. R6's control is vacuous. `off(readings[0], readings[0]) > 0.05` (`world/tests.rs:214`) tests the helper function, not any code. A real control: read the same series with `read.radius` fixed at the smallest slab, so it stops converging.
2. R10 has no control arm. A CPU-only control exists (`culling_keeps_what_the_camera_sees`); add one with mutated planes.
3. R13 checks only parent (0,0,0). Add a parent at (−1,−1,−1), where `div_euclid` matters.
4. `QUICK_BUDGET_S` is 20 s (`xtask/src/tiers.rs:19`), but §7.2 records quick at 18.6–21.9 s warm, so `--budget` fails at times. `xtask/src/main.rs:4,36` still say 10 s.
5. `capture_within` (`xtask/src/demo.rs`) pipes stdout and stderr but only polls `try_wait`. If cav-app writes more than a pipe holds, it blocks, the 120 s stop kills it, and the demo fails falsely. Drain the pipes on threads.
6. Some demo steps have no wall-clock limit: the flythrough benches and `load town; reading 1` go through plain `capture`.
7. `cav bench flythrough` prints the requested `{frames}`, not `times.len()`, so a run stopped by `--max-frame-ms` still claims 300 frames (`crates/cav/src/bench.rs`).
8. `MAX_BRICK_OVERDRAW`'s doc (`cav-render/src/world.rs:47-53`) puts the desktop's limit at about 50 ms, while the bench stops at 250 ms; nothing explains the gap. The budget also grows with the pane's pixels, so a frame at 1440p may march several times what one at 720p does.
9. While no worker is ready, the page fallback can run more than one job a frame (`pump` is called on every take and send), against its doc. A voxel job is 8–9 s on the page's thread.
10. `quick` runs workspace clippy and oracle clippy together (`tiers.rs:33-48`), so up to twice cores − 2 jobs; rayon test binaries ignore the cap.
11. `world-lod.png` has two flat colours and no legend or badge naming the overlay. §6 asks for "legends generated from what is drawn".
12. The demo's `world-box` step promised "the chunked box and the voxel body side by side" (§4.3), but `world-box.png` shows only the world box. `world-near.png` shows no far field, although §4.1 says "near field and rings".
13. Meshes are not rebuilt when a neighbour chunk arrives later, so buried boundary faces stay and cost vertices (`world.rs:655,684`).
14. Meshes stay on the GPU after a chunk leaves level 0, while it stays resident.
15. With the eye inside a brick, `entry_face` comes from the negative `tmin` and the first cell is shaded with the wrong face (`world_render.wgsl:146`).
16. `WorldGpu` is shared by every pane, so `notes`, `incomplete` and `marched` describe only the last pane prepared.
17. The resident-bytes cap is checked before a batch (`cav-engine/src/world.rs:283`), so one batch (threads × a chunk) can pass `MAX_RESIDENT_BYTES`.
18. The read's error estimate does not include the voxelisation error: the big scenes use one sample a cell, as S3's bodies do. One line saying so beside `world.supersample` would help.

## Checked and sound
- **The kernel's NaN fix.** It leaves every other value unchanged:
  - outside `u < 0 && u + r < 1e-6 r` the bits are the old ones;
  - inside, the change is below `~1e-16 r²`.
  The test is a real guard. At `d = (0.2+ε, 0.2+ε, 1)` with side 0.4, q = 0.33, so the kernel takes the prism. The two corners at z = −0.8 and z = −1.2 each give `c · ln 0 = −∞`, and their signed sum is `∞ − ∞ = NaN` without the fix. The 107 earlier cases in `report.json` are identical to the bit.
- **Validation report.** 113 of 113: 107 unchanged, 6 new, none dropped or missing, no tolerance moved. I1 is about how one new case was written, not about the report.
- **Physics of the read.**
  - The bound's derivation checks out: `|∇(r^−(l+1) P_l)| ≤ √((l+1)(2l+1)) / R^(l+2)`, and `Σ m s^l ≤ fourth · a^(l−4)`.
  - The formulas for the monopole difference, the quadrupole and the octupole are right; a cube's own second and third moments cancel in the traceless contraction.
  - The bound is read through the flight and added to the estimate.
  - R4 has a real control (a chunk left out), and so does R5 (a prism one cell longer).
- **Clipmap nesting** (R12). Sound, including negative centres; the control without snapping double-covers.
- **Region reader.** Every count is checked before any allocation.
- **Precision (§3.4).** f64 on the CPU; on the GPU, f32 relative to the camera.
- **xtask processes.** Every process goes through `gentle` at below-normal priority, and the job count floors at 1.
- **Docs.**
  - `docs/ARCHITECTURE.md`'s crate table and where-things-live rows match the code; no new dependencies, and the crate boundaries hold.
  - I followed `docs/extending/a-world-piece.md` against the code. Every file, function and test it names exists (`terrain_cover_is_conservative`, `a_chunk_is_the_raster_of_its_pieces`, `World::cells`, `every_leaf_has_a_unit_or_is_dimensionless_and_a_stage`), and its steps match `World::new` and `WorldSpec::validate`.
- **Strays.** `cargo xtask strays` at the end of this review: no process matches.
