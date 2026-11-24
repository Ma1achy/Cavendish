# Progress

The builder updates this after every slice. The owner reads it at checkpoints.

| Slice | State | Tag | Notes |
|---|---|---|---|
| S0 the shell | done: TASK-S0-01 to 14 merged; reviewed twice, nothing important remains | `s0-shell` | brief and results: `docs/slices/S0.md`; screenshots: `docs/slices/S0/`; validation: `docs/validation/report.md`; research: `docs/research/s0-app-stack-and-build-speed.md` |
| S1 a point mass | done: TASK-S1-01 to 19 merged; reviewed twice, nothing important remains | `s1-point-mass` | brief, results and the record of each first measurement: `docs/slices/S1.md` (§7, §7.1 to §7.4); screenshots: `docs/slices/S1/`; validation: `docs/validation/report.md` |
| S2 scenes as files | built: TASK-S2-01 to 12 merged; awaiting review | | brief and results: `docs/slices/S2.md` (§7); screenshots, transcripts and sample figures: `docs/slices/S2/`; validation: `docs/validation/report.md`; research: `docs/research/s2-replay-spec-paths-cadence.md`; for contributors: `docs/ARCHITECTURE.md`, `CONTRIBUTING.md`, `docs/extending/` |

## S2, scenes as files: where it stands (23 Nov 2026) — built, not yet reviewed or tagged

**Done.** A scene is a file and every field has a path (`get`, `set`, `ls`, `schema`, from the
console, `cav` and MCP, with inline specs); every f64 survives RON, JSON and `.npy` to the bit. One
edit path with inverse undo and redo, editors made from the schema, journals as files, `cav replay`.
Each tower on its own clock: offsets, a lane per tower that can be dragged, flight diagrams on
absolute time. Presets AION-10 and AICE, your own from a file, and `compare` to read the same scene
on another instrument. The cadence designer: jitter, alternation, gaps, windows and loss, with the
closed forms' predictions beside one realisation, and the NUFFT Lomb–Scargle behind `periodogram`
on uneven shots. `cav run scene.ron --out dir/` writes the bundle. Figures: `cav-plot`, SVG, PDF
and PNG in five presets with a colour-blind-safe palette, each with its data and provenance, from
the console, `cav figure` and MCP; a Python helper that redraws one. For other people:
`docs/ARCHITECTURE.md`, `CONTRIBUTING.md`, six recipes in `docs/extending/`, every public item of
every library crate documented, the dependency table held by a test. PRs #64 to #77 and
TASK-S2-11's.

**Numbers** (`docs/slices/S2.md` §7): `quick` 325 tests, 8.8 s warm (it was 13.6 s with all of S2
in; 16.9 s straight after an edit of `cav-sim`); `gpu` 29 in 9.3 s; `oracle` 41, pyright strict
and 8 Python tests; 82 of 82 validation cases (18 new, none dropped, four tolerances tightened
from 5e-7 to 1e-8). The exit test: a session replayed four ways writes the same 23 files, 0
differ; 1, 2, 3 and 6 reading threads, 0 bits differ. A 10,000-shot series: 0.4 s on six threads, 1.5 s on one (S1: 2.5 s).

**Decided** (the full list is `docs/slices/S2.md` §7.9).
1. **`cav-plot`** is a new crate: plots are display lists, and `cav` no longer links the window.
2. **A replay never writes where its journal says**: exports go into the replayer's directory, or
   nowhere.
3. **Dependencies are optimised in dev, and `cav-sim`, `cav-engine`, `cav-plot` are at
   `opt-level = 1`**: that, and nothing cut, is what brought `quick` back under its gate. Building
   from nothing takes about three minutes longer, once.
4. **The build's revision is asked of git at run time**, not compiled in.
5. **Text in SVG and PDF is glyph outlines** from the two Ubuntu faces, and the PDF writer is
   in-house: no new dependency but `skrifa`, which egui already builds.
6. **One schedule for the whole array**, loss drawn per tower; per-tower instruments and jitter
   wait for S15.
7. **`cycle ≥ 2T`** (it was `>`): the AICE preset's cycle is exactly a shot.
8. **AICE launches at 21.236 m/s**, the symmetric rule with ⁸⁷Sr, not the plan's 21.27. George
   has confirmed it (23 Nov 2026): the speed is derived by his scripts' rule, which is this one.

**George's answers (23 Nov 2026), TASK-S2-18** (`docs/slices/S2.md` §7.13; each answer is under
"For George" below). The tower is 10.1 m, and a test holds the arms below it. AICE's launch is
derived, never a typed number, and the kick number is a field of the instrument
(`array.instrument.kicks`, default 1000). The default layout is called a default. Nothing that is
read moved: 82 of 82 earlier validation records to the last digit, but one of the picture
(`render.gpu_perspective_ratio`, 0.26% to 0.23% against 3%: the towers drawn are taller). Two new
cases, 84 in all. `quick` 369 tests, `gpu` 30, `oracle` 43. **Two things for the next checkpoint:**
the moving anchor's "~7 mrad" is not half its peak to peak (8.34 mrad; half its largest phase is
6.83), and AICE's arms reach 108.9 m in its 100 m tower; both are back with George.

**Calls worth a second look (S2).**
1. **The palette passes by a hair**: least CIEDE2000 12.17 against 12, the first four colours 8.3 L*
   apart against 8. The colours were searched for under the criterion, which was written first.
2. **`quick`'s margin is thin** (8.8 s against 10; 9 to 12 s from the pre-push hook on a busy
   machine), and it was bought with optimisation levels. S3 will need a real answer: the memory
   tests and the figure tests are the slow ones.
3. **Two faults were found by the demo, not by the tests**: `cav replay` refused its own journal
   (a replay waited a tenth of a second for a bundle's signal) and could not draw figures. Both
   are fixed with tests; the exit test's scenes had been too quick to show the first.
4. **The dense-signal criterion was restated** from "every tower" to "every distinct series": two
   towers of the comb read the same to the bit, and no drawing order shows both.
5. **The recipes were not followed by a second person.** They were written from the code, and a
   test holds every name in them to the code; the brief asked for each to be followed once.
6. **The fonts in figures**: the Ubuntu Font Licence's text says documents made with the font are
   not affected; that is a reading, not advice. It matters when a figure is published.
7. **`serde_json`'s `preserve_order`** is on for the whole workspace.
8. **The Python tests have their own runner** rather than pytest, which is not installed.
9. **A figure's revision is the commit it was made at**, so the demo's sample figures change with
   every run of the demo.

**Open.**
- Nobody has looked at the real window (every picture is headless); DX12; SVG in a browser.
- A harmonic's label can cross a neighbouring stem in the narrowest figure preset.
- Figures of the compared instrument; a vector drawing of the 3-D view.
- `apply(Set)` at 1000 bodies was not measured; at the largest valid scene a set is tens of
  milliseconds.
- Carried again, in `plan/IDEAS.md`: Tab scopes, completion and a command palette, the clipboard,
  MCP resources, saved layouts.

**Next.** A fresh review of S2 against its brief; fixes as further tasks; then the tag, and S3.

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
   window ending one step short of 2T, and two other cases (`fig2_10m` c and d) passed 3e-4 only by
   cancellation (this said seven; corrected after the second review). The bound is withdrawn. Now:
   the scripts' rule taken to its limit is the engine (3e-10 on AICE, 1e-7 on AION-10; since S2's
   TASK-S2-10, 5e-9 at dt/128 against 1e-8), and the engine differs from each fixture by what that
   rule's error predicts, to within 0.3% of the prediction, two-sided. The second is the first
   rescaled, not an independent check (`docs/slices/S1.md` §7.2, §7.5, `acknowledged.txt`).
2. **The published anchor sizes** ("~7 mrad", "~2 mrad peak to peak") are not met by v1 itself: it
   gives 13.66 mrad and 4.34 mrad. George's `1D_oscillator` gives 4.06 mrad peak to peak, so "~2 mrad"
   is the oscillator's amplitude. The engine is held to v1 and the scripts, not to the round numbers.
   *Since 23 Nov 2026:* George says the sizes are amplitudes, half the peak to peak. The
   oscillator is now held to that (2.15 mrad against 2, within 10%:
   `oracle.published_anchor_oscillation_amplitude`). The pass is not: its series changes sign, so
   half its peak to peak is 8.34 mrad, 19% from 7, while half its largest phase is 6.83. The 10%
   was written first and was not moved; the question is back with George.
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

**Open.**
- **The moving anchor's "~7 mrad (Fig 12)" is not half its peak to peak in the engine or in v1.**
  You said the published sizes are amplitudes, half the peak to peak. For the oscillator that
  holds: 2.15 mrad at the anchor's readouts, 2.03 in your `1D_oscillator`. For the pass (10 kg at
  D = 10 m rising at 2.5 m/s from 10 m below the base) the series changes sign as the mass goes
  by: +3.02 mrad before, −13.65 mrad at the peak (lower minus upper), so the peak to peak is
  16.67 mrad and half of it 8.34 mrad, 19% over 7. Half the largest |ΔΦ| is 6.83 mrad, 2.5%
  under. v1 gives the same (8.34 and 6.83). Is Fig 12's "~7" half the largest excursion from
  zero, or is the figure of another pass (another window, speed or distance)? Default: the
  oscillator's amplitude is a validation case (`oracle.published_anchor_oscillation_amplitude`,
  10%); the pass is held to v1 only, and a test asserts both numbers
  (`the_published_anchor_sizes_are_what_the_references_give`). Detail: `docs/slices/S2.md` §7.13.
- **AICE's arms do not fit in a 100 m tower either.** With 70 m between the interferometers,
  T = 2.5 s, the derived launch (21.236 m/s) and n = 1000, the upper interferometer's upper arm
  reaches 108.88 m above the base at the π pulse (38.88 m above its launch point). `fig2_final.py`
  has `L_base = 140.0`. Should the preset's tower be 140 m, or the upper interferometer start
  lower? Default: the plan's 100 m; nothing that is read depends on the height, and the flight
  diagram's axis already stretches to the arms.
- **Your `fig2_*` and `100m_oscillators` scripts integrate to 2T less one step**
  (`arange(N_fly)`; `fig2_final.py` does too). That is the main term of their difference from the
  converged integral: 8.1e-4 of the peak on `100m_oscillators` (the trapezoid itself is 2.6e-4),
  2.7e-4 to 3.2e-4 on `fig2_10m`. One more node (integrating to 2T) cuts the error on
  `100m_oscillators` about fourfold. `1D_oscillator` already covers the whole flight. Nothing to
  answer: for you to know.

**Answered 23 Nov 2026** (through the owner; TASK-S2-18, `docs/slices/S2.md` §7.13).
1. **Which AICE is the preset?** Interferometers 70 m apart, a 5 s cycle, T = 2.5 s: fixed. The
   launch speed is not a constant: it is computed as `fig2_final.py` computes it, so that the
   atoms finish at the height they started. *Done:* the preset keeps those three and derives the
   launch when it is built (`presets::aice`); the "unconfirmed" label is gone.
2. **May shots overlap?** No, for now; multiplexing may come later. *Held by*
   `a_shot_is_two_t_long_and_shots_do_not_overlap` (`cav-sim`), and `validate` refuses a cycle
   shorter than 2T.
3. **AICE's launch speed** is that rule, not 21.27 as a number. *Checked:* `fig2_final.py` has
   `u0 = g * T_fly / 2 - v_kick / 2` with `T_fly = 2 * T`, which is the engine's `g·T − v_kick/2`:
   the same rule. With the script's constants (m = 1.46×10⁻²⁵ kg, ħ = 1.055×10⁻³⁴ J s) it is
   21.27267 m/s, the plan's 21.27; with ⁸⁷Sr and the exact ħ it is 21.23605 m/s, the preset's.
   Case `oracle.script_launch_rule`: 0 against 1e-12. Each arm, not only their mean, is back at
   its launch height at 2T (to 1e-9 m); at 21.27 m/s with ⁸⁷Sr both end 17 cm above it.
4. **AION-10's T = 0.73 s:** yes.
5. **AICE's far-field geometry** (70 m, the same kick number as AION-10): yes, though AICE would
   support larger LMT kick numbers, so n is a parameter. *Done:* the kick number is a field of
   every instrument (`array.instrument.kicks`, default 1000); `presets::aice_with_kicks(n)` is
   the preset at another n with its launch derived, and in a scene
   `set array.instrument.kicks <n>` then `launch symmetric`.
6. **When a shot is late, are the later ones late too?** Yes: lateness accumulates (the random
   walk), and it is only ever in the pause between shots; the sequence itself never varies, 2T
   for every shot. *Held by* `a_shot_is_two_t_long_and_shots_do_not_overlap` (`cav-sim`); the
   default schedule was already the walk, late only.
7. **Do the towers share one clock and one laser?** Yes. As built.
8. **The ground layout.** There is no real layout: this is a prospective study. S0's cross of
   five towers stays as the default, *labelled as a default* where it is defined, in the default
   scene's description and in the scene files.
9. **The tower is 10.1 m**, to hold the ⁸⁷Sr upper arm at 10.006 m. *Done:* the default and the
   AION-10 preset; `the_default_arms_top_out_below_the_tower` holds the arms' reach (from the
   forward model's own arms) below the tower, with 10 m as its control. Nothing that is read
   changed: 82 of 82 validation records are as they were to the last digit but one of the
   picture.
10. **The published anchor sizes are amplitudes** (half the peak to peak): yes. *Done* for the
    oscillator; for the pass see the open question above.
11. **Forced motion on a sphere:** confirmed, the engine's parametrisation matches the upgrade
    report's. Closed.
12. **The site's g = 9.81 m/s²:** yes.

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

## Owner's ruling (23 Nov 2026): mixed towers come forward
Towers of different heights and sequences in one array move from S15 to S3 (TASK-S3-08), with a
scene of zero towers allowed. The research is done (`docs/research/s15-instruments-anywhere.md` §3 and
the second note beside it). S15 keeps mixing across axes and gravity environments.

## Owner's ruling (22 Nov 2026): theoretical and speculative sources
Added to S16 (`REBUILD.md` §9.3, TASK-S16-06; three validation rows in §2.6): hypothetical sources,
each labelled as such with the paper it comes from — exotic gravitational waves (primordial and
intermediate-mass black holes, cosmic strings, an early-universe background), dark-matter fields
and transients, a Yukawa fifth force on every mass in the scene, an equivalence-principle
violation, a dark compact object passing the Earth.

## Owner's ruling (22 Nov 2026): gravitational waves from cosmic events
Added to S16 (`REBUILD.md` §9.3, TASK-S16-05; validation rows in `VALIDATION-AND-RESEARCH.md` §2.6): a
library of sources (inspirals, a stochastic background, bursts) with sky position and polarisation,
read through the baseline's antenna pattern as the Earth turns, and replay of catalogued events.

## Owner's ruling (22 Nov 2026): air and water are fluids
The owner wants water adaptive like the air, and air and water under one heading. In the plan:
S8 "Fluids: air", S9 "Fluids: the adaptive grid", S10 "Fluids: water" (`REBUILD.md` §9 and §9.1a) —
one solver and one adaptive octree for any fluid; water adds the free surface, refined at the
surface and by tower response; every adaptive-grid property is proved for water as for air
(TASK-S10-05, TASK-S10-06). The spec, commands and viewer say `fluid` with a material.

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
