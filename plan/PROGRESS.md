# Progress

The builder updates this after every slice. The owner reads it at checkpoints.

| Slice | State | Tag | Notes |
|---|---|---|---|
| S0 the shell | done: TASK-S0-01 to 14 merged; reviewed twice, nothing important remains | `s0-shell` | brief and results: `docs/slices/S0.md`; screenshots: `docs/slices/S0/`; validation: `docs/validation/report.md`; research: `docs/research/s0-app-stack-and-build-speed.md` |
| S1 a point mass | done: TASK-S1-01 to 19 merged; reviewed twice, nothing important remains | `s1-point-mass` | brief, results and the record of each first measurement: `docs/slices/S1.md` (§7, §7.1 to §7.4); screenshots: `docs/slices/S1/`; validation: `docs/validation/report.md` |
| S2 scenes as files | building; research: `docs/research/s2-replay-spec-paths-cadence.md` | | |

## S1, a point mass: where it stands (22 Nov 2026) — done, tagged `s1-point-mass`

**Done.** The forward model in `cav-core` with George's defaults; the oracle crate (v1 at `v1-final`
compiled from source, George's scripts as fixtures) and `cargo xtask oracle`; bodies, the oscillator
library and thirteen dev scenes; `Set`, `Run`, `Signal`, `Reading`, `Periodogram`, the forward worker
and per-source caching; in the app, a mass you can drag with every tower's reading beside it, a pass
you can play, the signal plot and the periodogram with each body's comb and its folds; the spectra
in `cav-core`; the §2.1 tests; the nulls. `cargo xtask demo S1`; the validation report regenerated
with `--report --against s0-shell`.

**Numbers** (`docs/slices/S1.md` §7): `quick` 208 tests, 4.0 to 4.8 s warm (6.7 to 12.2 s straight
after a rebuild: still over the 10 s gate then); `gpu` 25 in 7.2 s; `oracle` 41 in 7.4 s; 64 of 64
validation cases. The v1 tie: 1e-15 of the peak against 1e-10. Scripts as published: 1e-12 against
1e-9. The scripts' rule taken to its limit is the engine: 3e-10 (AICE) and 1e-7 (AION-10), against
1e-9 and 5e-7. One read allocates about 100 bytes whatever T is and stops at 4,000,000 nodes.

**After the review (22 Nov 2026).** PRs #54 to #60 and this one:
- TASK-S1-15: the moved tolerance is gone; script parity is a limit and a two-sided prediction.
- TASK-S1-16: a read's memory and work are bounded; readings are made off the caller's thread; T is
  bounded in the schema (1000 s).
- TASK-S1-17: undo of separate edits; values with strings from the console and MCP; controls and
  labels that can fail; `quick` no longer builds doctests.
- TASK-S1-18: the frame says what a pane shows; `render_view` and `cav render` draw plot panes; the
  plots have marks, zoom (`span`), and labels that fit.
- TASK-S1-14: the NUFFT Lomb–Scargle, 3e-15 of the peak against the exact one.
- TASK-S1-12: Ubuntu Mono is the monospace face, its licence built in; screenshots made again.
- TASK-S1-19: the validation report, the demo and the brief's results regenerated.

**Not done.** `docs/ARCHITECTURE.md` and the recipes in `docs/extending/` (a duty that CLAUDE.md
gained during the review): neither exists yet, and public items are documented but not yet all with
units and frames. It is a task of its own, before or with S2. A 10,000-shot series takes 2.5 s and
a frame with it plotted 27 ms, a reading of four bodies 1.2 ms: all over the brief's own budgets
(recorded, not gated). The NUFFT Lomb–Scargle is in `cav-core` but no command uses it. Nobody has
looked at the real window.

**Calls worth a second look (S1).**
1. **One tolerance was changed after measuring, and the review undid it.** The builder set
   `oracle.script_same_time_100m_oscillators` to 1.5e-3 after it measured 1.07e-3 against 3e-4, and
   gave the trapezoid as the cause. The review measured otherwise: 8.1e-4 of it is the scripts'
   window ending one step short of 2T, and seven other cases passed 3e-4 only by cancellation. The
   bound is withdrawn. Now: the scripts' rule taken to its limit is the engine (3e-10 on AICE, 1e-7
   on AION-10), and the engine differs from each fixture by what that rule's error predicts, to
   within 0.3% of the prediction, two-sided (`docs/slices/S1.md` §7.2, `acknowledged.txt`).
2. **The published anchor sizes** ("~7 mrad", "~2 mrad peak to peak") are not met by v1 itself: it
   gives 13.66 mrad and 4.34 mrad. George's `1D_oscillator` gives 4.06 mrad peak to peak, so "~2 mrad"
   is the oscillator's amplitude. The engine is held to v1 and the scripts, not to the round numbers.
3. **The engine reads until it stops changing** (`Reader::read_to`): the fixed rule that is good to
   3e-15 on the anchors is off by 3.9e-6 rad for a pass 0.5 m from a tower.
4. **v1 stays a git dependency of the oracle crate** (2.1 s to build from nothing), and its anchor
   outputs are frozen as well so that `quick` checks the tie on every commit.
5. **`cav` now depends on `cav-app`** (the plan's crate table says `cav-engine` and `cav-render`),
   so that an agent's picture of a plot pane is drawn by the application's plot code and not by a
   second copy. The cost is that the CLI links egui.
6. `Command::Load` now boxes its scene (a scene with bodies is large); the JSON form is unchanged.
7. **A load forgets its predecessor's signal** rather than showing it stale under the new scene's
   name; an edit keeps the old signal, marked stale, until the new one is whole.
8. **The in-house FFT** stays; rustfft measured 3 to 30 times faster and is in `plan/IDEAS.md` for S11.
9. **`quick` has little room after a rebuild.** Warm it is 4 to 5 s (documentation examples moved
   to `oracle`); straight after a rebuild of `cav-core` or `cav-app` it was 11 to 12 s. Nothing was
   changed to hide it.
10. **The comb test with noise asserts three harmonics, not four:** at the fixtures' 2e4 s the fourth
    is under the scripts' shot noise.

11. **The read is slower for being bounded:** 10,000 shots went from 1.8 s to 2.5 s when the read
    stopped keeping tables.
12. **The NUFFT's two predictions were too cautious** (rounding floor and the kernel's starting
    error); the criteria were left as written rather than tightened to the measurements.

**Next.** A second look at S1 by the reviewer; then the tag, `docs/ARCHITECTURE.md` and
`docs/extending/`, and S2.

## Carried into S2 from S1's second review (22 Nov 2026)
The second review found nothing important; S1 is tagged `s1-point-mass` at `05f76ca`. Its minor
findings are S2's first task, the first one first because it is the class of fault that was
important twice (a valid scene that can take the machine down):
1. **A signal's memory scales with towers × shots and is not capped** (`crates/cav-sim/src/read.rs:99-103`
   reserves `towers × shots` three times per source; the schema allows 1000 towers and 2,000,000 shots,
   48 GB). Found by reading, not run. Bound it by construction and test it with the counting allocator.
2. **`reading` waits for an unrelated series** (`crates/cav-engine/src/forward.rs:342-367`,
   `console.rs:508-520`): 60 s for a reading that was ready in milliseconds behind a long signal.
3. **An unsettled reading is headed "refined · quadrature within 1e-1 rad"** (`engine.rs:26-34`): say
   "did not settle", not "within".
4. **Script parity, said truthfully.** The two-sided "gap" case is the "limit" case rescaled, not an
   independent check (`oracle/tests/scripts.rs:212-250`); say so in `S1.md` §3.1 and §7. "Seven cases
   passed 3e-4 only by cancellation" is wrong: it was two (`fig2_10m` c and d); correct
   `acknowledged.txt:12-13` and its `fig2_10m_b` line, `scripts.rs:15-18`, `S1.md` line 194 and §7,
   and call 1 below.
5. **The AION-10 limit tolerance (5e-7) is the reference's residual, not the engine's**: the script
   rule at dt/128 reaches 1e-9 to 4e-9, so hold those four series to 1e-8 there (about 16 s, oracle
   tier). An engine regression of 3e-7 of peak passes today.
6. **Viewer**: the timeline's last tick label is clipped ("200" for 2000 s); a run of spaces in the
   About box (`panels.rs:1020`); the full-span signal is a solid block with one tower over the rest;
   no key on a signal plot from `render_view`; the playhead's colour differs between plots; the
   crowded oscillator-zoo periodogram; "Home" over a plot's title line.
7. **The NUFFT zero-denominator guard is untested** (`spectrum.rs:392`); the outliner's warning mark
   matches by substring (`panels.rs:432`: "tower 1" is marked for "tower 12").
8. **`cav` depends on `cav-app`** so the CLI links eframe and winit: move `plots.rs`, the windowless
   painter and the font set-up into a small `cav-plot` crate both depend on.
9. **Smaller**: `crosses()` visits up to 480,000 nodes outside the read's budget; latest-wins can
   starve the live reading during a drag; `Query::Reading(Some(t))` off the playhead never refines
   without `ask_reading`; a drag whose first step is refused can merge into an earlier `set`; string
   states compared as `== "stale"` beside the `SignalState` enum; `ReadingAt.towers` is a positional
   array; undocumented units and frames (`Source::difference`, `lomb_scargle_fast`, `Flight.nodes`);
   a wall-time assert in `cav-render/tests/gpu_render.rs:439`; `cav` has no command that prints the
   font's licence.
Still unchecked after two slices: the real window (every picture is headless), DX12.

## S0, the shell: where it stands (21 Nov 2026)

**Done.** The workspace (`cav-core`, `cav-sim`, `cav-engine`, `cav-gpu`, `cav-render`, `cav-app`, `cav`,
`cav-validate`, `xtask`). Open the app (`cargo run -p cav-app`): the room, five towers, menu bar,
outliner, properties, per-tower timeline, console, footer; perspective and orthographic, seven named
views and Home, 1, 2 or 4 panes, five draw styles. `cav doctor`, `cav render`, `cav scenes`, `cav schema`,
`cav exec`; `cav mcp` with `scene_schema` and five more tools; `scenes/dev/void.ron` and
`infinite-floor.ron`; `cargo xtask quick | gpu | oracle | demo S0 | validate --report`. The engine's
command/query API is the one path all of them drive, with a journal that replays.

**Numbers** (detail in `docs/slices/S0.md` §7): `quick` 114 tests in 4.7 s warm (gate: 10 s); `gpu` 22
tests in 6.4 s; cold build 51 s with the Defender exclusion; warm rebuild 2.4 s; 11 of 11 validation
cases pass.

**The review** (21 Nov 2026) found one important fault (a valid spec could hang the renderer and an MCP
client) and nine minor ones. All ten are fixed with tests; `docs/slices/S0.md` §7, "After the review".

**Open.**
- Nobody has looked at the real window: it was started minimised for five seconds and closed. Everything
  seen is from the headless renderer, which runs the same UI code.
- The oracle tier is a stub until S1.
- The rules of the validation report are not in `CLAUDE.md` or the agent definitions (question below).

**The second review** (21 Nov 2026, of the fixes) found nothing important; S0 is tagged `s0-shell` at
`00ca950`. Its six minor findings are carried into S1 as its first task:
1. `validate --against <revision>` passes when the revision does not resolve (a mistyped tag switches
   the guard off silently): an explicit `--against` that cannot be read must be an error.
2. A failing `validate --report` still rewrites `report.json`, so committing that file with a dropped
   case or loosened tolerance makes the default HEAD comparison pass from then on. Write the report
   only after the comparison holds, and run the slice-end check `--against <last slice's tag>`
   (from S1: `--against s0-shell`).
3. `cargo xtask hooks` overwrites a foreign hook that is not valid UTF-8.
4. `scene_schema`, `scene_get` and `log` are not under `catch_unwind` in `cav mcp`.
5. The schema's ranges are not exact: inclusive `minimum: 0` where `validate` refuses 0;
   `Slab.thickness` capped with no range; cross-field rules (`cycle > 2T`, separation < height) unstated.
6. `cav exec` and `cav-app --exec` split on `;` and newlines without regard for quotes; a comment line
   holding one `"` swallows the lines after it.
Also noted: `scene_load` reads a file of any size; DX12 was not re-run after the fixes.

**Next.** S1: the forward model, the oracle crate, a point mass you can drag. S1's brief should start from
George's answers below and add the first physics rows to the validation report (the void's channels
exactly zero, the infinite floor's null, the v1 anchors).

## Questions for the owner
None open.

Answered 22 Nov 2026 (as relayed to the S1 builder by the coordinating agent):
- **Ubuntu Mono: yes.** `UbuntuMono-Regular.ttf` from the official Ubuntu font family release goes in,
  with the Ubuntu Font Licence 1.0 beside it and a line on where the file came from and its version; it
  becomes the app's monospace face and the affected screenshots are regenerated. It is TASK-S1-12. **Done:**
  the coordinator added the file, `UFL.txt` and `SOURCE.md` on the owner's direct yes; the builder
  wired it in and built the licence into the binary (PR #60).
- **The validation report's rules** are in `CLAUDE.md` and the builder and reviewer definitions: the
  owner's PR #41. The builder's step is `cargo xtask validate --report --against <previous slice's
  tag>`; for S1 that is `s0-shell`.

## For George
- **AICE's geometry, for the far-field estimates** (`docs/research/s14-wider-world.md`): the note assumes
  the 100 m instrument has interferometers 70 m apart and the same laser kick number as AION-10. Right?
Answered 21 Nov 2026 (through the owner). The defaults below replace v1's conventions from S1 on.
1. **Sign:** lower minus upper (δφ₁ − δφ₂). Both conventions are in use and it should not matter.
   *Which interferometer is 1:* the lower one. The reference scripts launch IFO 1 from z = 0 and IFO 2
   from z = δr (`docs/research/s1-periodograms-and-script-fixtures.md`, the note on the scripts'
   geometry) and compute lower minus upper; v1 computes δφ₂ − δφ₁, upper minus lower
   (`reference/design/instrument.md` §4, which numbers the interferometers but does not say which is
   lower: S1 should confirm it in v1's code at `v1-final` when it sets the oracle's conventions).
   `plan/REBUILD.md` §3.4 still says "the read is upper-minus-lower": that sentence is about v1's
   convention and about differencing without cancellation, which holds for either sign. The plan's text
   is left as it is; George's sign is the engine's from S1.
2. **Timing:** the atom and the source are taken at the same time; the reference scripts' one-step
   offset (arm at (j+1)·dt against source at j·dt) is not intended.
3. **Atom:** ⁸⁷Sr, mass ≈ 1.443×10⁻²⁵ kg (S1 takes the value from a cited source), not the
   1.46×10⁻²⁵ kg (⁸⁸Sr) used so far.
4. **Launch:** AION-10's launch speed is 3.86 m/s; the atoms come back to the launch height and pause
   there until the next sequence starts.
5. **Floor:** the arms never go below launch. There is a floor, coded as the Python scripts code the
   sequence; copy that.

Asked in S1 (defaults in use until answered):
- **What do the published anchor sizes measure?** "10 kg at 10 m passing at 2.5 m/s, ~7 mrad (Fig 12)"
  and "1 kg at 5 m oscillating, ~2 mrad (Fig 15)". Your `1D_oscillator` gives −3.86 to +0.20 mrad,
  4.06 mrad peak to peak; v1 and the engine give 13.7 mrad as the peak of the pass. Are the published
  figures amplitudes (half the peak to peak)? Default: nothing is held to the two round numbers.
- **Forced motion on a sphere** (upgrade report §2.5.1) was not available to read. The engine's
  `Sphere` motion is its own guess: a body at a fixed distance from a centre, at an angle
  `tilt·cos(2π f t)` from straight down, in a vertical plane turning at a second frequency. What is
  the report's parametrisation?
- **The site's g.** 9.81 m/s², as in the scripts, is the default.
- **The `fig2_*` and `100m_oscillators` scripts integrate to 2T less one step** (`arange(N_fly)`).
  That is the main term of their difference from the converged integral: 8.1e-4 of the peak on
  `100m_oscillators` (the trapezoid itself is 2.6e-4), 2.7e-4 to 3.2e-4 on `fig2_10m`. One more node
  (integrating to 2T) cuts the error on `100m_oscillators` about fourfold. `1D_oscillator` already
  covers the whole flight.

- **With ⁸⁷Sr the upper arm of the upper interferometer tops out at 10.006 m in the 10 m tower**
  (the kick is 6.578 m/s with ⁸⁷Sr's mass and the exact ħ, 6.505 with the scripts' constants; the arm
  is 5.006 m above its launch point at T). Is the launch point of the upper interferometer really at
  5 m, or the tower taller than 10 m? Default: as it is; nothing in the engine depends on the tower's
  height but the picture.

Still open (asked in S0; defaults in use until answered):
- The ground layout of a multi-tower array and the room around it. S0 draws v2's layout: five towers,
  one at the centre and four on a cross with 4 m arms, standing on the floor (z = 0) of a
  10 × 10 × 3 m room, each 10 m tall so they rise through the ceiling. Is there a real or intended
  layout, and where is the tower base relative to the floor people walk on?

## Owner's ruling (22 Nov 2026): built for other people, and figures
The owner wants the project easy for others to extend and work with (clean architecture, documented
and easy-to-follow code) and a good way to make plots and figures from it. Written in as principle
10 of `REBUILD.md` §2, a line in `CLAUDE.md`'s quality bar, and duties in the builder's and
reviewer's definitions; figure export is a must-have of §6. Two tasks in S2: TASK-S2-07 (figure
export) and TASK-S2-08 (`docs/ARCHITECTURE.md`, `CONTRIBUTING.md`, recipes in `docs/extending/`, a
lint for undocumented public items). Every later slice keeps them true.

## Owner's ruling (22 Nov 2026): beyond the scene
The owner asked for the world beyond the voxel scene (weather, clouds, the Moon and tides,
earthquakes, tsunamis, real planet data, Earth's rotation), for towers of different heights, a
gradiometer on a horizontal axis, other gravity environments, one in orbit, and said to plan the
ideas that followed (signal injection, two sites, replaying a real day, site selection, a twin,
ground water, what is underground, a systematics budget). Added to the plan: **S14, the wider
world**, **S15, instruments anywhere**, **S16, the science case** (`REBUILD.md` §9 and §9.3;
validation in `VALIDATION-AND-RESEARCH.md` §2.6; TASK-S14-01 to 09, S15-01 to 05, S16-01 to 04). Each
starts with a research note that sizes every effect in radians; what a note shows to be invisible
is built as a null test and no further. The remaining ideas are in `plan/IDEAS.md`. Datasets are
cached outside the repo; each licence is checked and reported to the owner before use.

## Owner's ruling (22 Nov 2026): heat everywhere
The owner asked for full thermal physics, in air and in water, and for a way to validate it. Added to
the plan: slice **S13, heat everywhere** (`REBUILD.md` §9 and §9.2) — sources, conduction in solids,
conjugate transfer to air and water, radiation between surfaces, water's real equation of state,
thermal expansion, and the read of all three (air, water, displaced solids). Its validation suite is
`VALIDATION-AND-RESEARCH.md` §2.5: conservation and null cases, closed forms, published heat-transfer
benchmarks and cases through the instrument. Tasks TASK-S13-01 to 09. It runs after S10 and before
S11. Out of scope until asked for: phase change, combustion, humidity, participating air. The
research note `docs/research/s13-heat.md` (methods, reference numbers and their sources) comes first.

## Owner's rulings (21 Nov 2026)
- **Tools on the build PC:** anything the build needs may be installed. Done so far: rustup with
  stable `x86_64-pc-windows-msvc` (rustc 1.100.0), `cargo-nextest`.
- **GitHub:** `gh` for all GitHub work (git pushes use its login via `gh auth setup-git`); plain git
  for local commits; never force-push.
- **This PC is for building and testing correctness only** (Windows 10 Pro, GTX 1660 SUPER, 6 GB).
  The design and the targets are not shrunk to fit it. Every slice works here at whatever scale the
  card manages; the measured numbers go in the slice notes; the performance targets (60 fps, 10¹⁰
  cells, latency budgets) are marked "to be measured on a faster GPU later" and do not fail a slice.
  Correctness, oracle and GPU-parity tests must pass here.
- `origin/build-machine` was already in `main` (merge `176bc81`); nothing further to merge.
- **Defender exclusion:** `C:\src\Cavendish\target` is excluded from real-time scanning (added by the
  owner). S0 re-measures cold and warm build times with it; both sets are in `docs/slices/S0.md`.
- **Fixtures from the reference scripts:** George has given permission to use his scripts
  (`github.com/Thranduil02/atom-interferometry`); fixtures made by running them may be committed,
  from S1 on.
- **The two S1 plan changes are accepted** (detail: `docs/research/s1-periodograms-and-script-fixtures.md`):
  1. `point_oscillator_phase_matches_script ≤1e-9` holds for a test-only quadrature that copies the
     scripts' sampling; the engine's own forward model gets a separate measured tolerance and stays
     tied to v1 at ≤1e-10.
  2. No GPU periodogram in S1: f64 on the CPU (classical, exact Lomb–Scargle, NUFFT Lomb–Scargle); the
     GPU version waits for float-float in S5.

## Decisions worth a second look
- **S0 (reasons in `docs/slices/S0.md` §7, "Departures"):**
  - the engine is a plain value with no worker thread, and does not depend on `cav-gpu` yet;
  - `ViewSpec` and `Target` live in `cav-sim`, so the renderer draws views and selections without
    depending on the engine; rendering is done by the client (`cav-app`, `cav`), not by an engine query;
  - playback is the app's clock issuing `Seek` commands, which the journal keeps as one entry;
  - panes redraw on any change of engine state, not only of their own view;
  - no `clap`, no `anyhow`, no docking crate, no clipboard; a tenth crate, `cav-validate`;
  - paper style is ink on cream only until S4;
  - the validation plots use one axis, error over tolerance, for cases of every unit;
  - a named view re-frames the scene (it used to keep the last distance), and a framed view stays
    framed when the projection changes; a camera the user has moved is left alone;
  - the spec's ranges (`cav_sim::spec::limits`): ±1e6 m, rooms to 1e4 m, towers to 1 km and 1000 of
    them, T from 1 ms, cycle to 1e4 s, timeline to 1e6 s. Wide, but they are limits: say if a real
    scene needs more;
  - `quick` is 4.7 s warm but 7 to 10 s on the run straight after an edit that recompiles `cav-app`
    (10.0 s was seen once): the 10 s gate holds warm, with little to spare as tests are added. S1 should
    watch it and, if it goes over, say so with numbers before changing anything.
- **S1: the engine's defaults become George's** (lower − upper, ⁸⁷Sr, u₀ = 3.86 m/s, a floor at the
  launch height with the pause). They differ from v1 in sign, in mass (1.2%) and in the floor. So:
  - the v1 oracle tie (≤1e-10) is run with the instrument set to v1's conventions (floor off, v1's
    mass, sign flipped). The floor and the mass are instrument parameters, not a second code path;
  - the floor's behaviour is tested against the scripts;
  - because the one-step offset is unintended, S1 keeps two script fixtures: the scripts as published
    (matched by the test-only script-copying quadrature, ≤1e-9) and a copy with only the offset
    corrected to same-time, against which the engine's own model gets its measured tolerance;
  - **for the owner:** the sign changes relative to v1 bundles, which matters to Gradar's existing data.
- **S0's "builds in under a minute cold" is recorded, not gated.** Measured on this PC: 60–102 s
  cold (median about 75 s) for an egui + wgpu + rmcp skeleton, front-end bound; warm rebuild 4.5 s.
  Following the owner's ruling on this PC's role, the slice records cold and warm times and gates
  only on `quick` under 10 s.
- **rust-lld is left off for now**, against REBUILD §8: measured gain 0.3 s per link, lost in the
  noise. Kept as a commented option to re-measure when the binary grows.
- **Tasks merge as they pass `quick`; the review is per slice.** Each task has its own PR
  (`task/<TASK-id>`) merged with a merge commit so later tasks build on `main`; a fresh reviewer
  reads the whole slice at the end, its findings are fixed in further task PRs (new ids), and the
  slice is tagged only when no important finding remains.
- **Cloud sessions** are started by the loop in a minimised console window (`claude --cloud` needs a
  console); their work comes back as a pushed branch. The first one: `research/s3-sparse-voxel-structures`.
