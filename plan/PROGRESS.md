# Progress

The builder updates this after every slice. The owner reads it at checkpoints.

| Slice | State | Tag | Notes |
|---|---|---|---|
| S0 the shell | built (TASK-S0-01 to 11 merged); awaiting the reviewer | | brief and results: `docs/slices/S0.md`; screenshots: `docs/slices/S0/`; validation: `docs/validation/report.md`; research: `docs/research/s0-app-stack-and-build-speed.md` |

## S0, the shell: where it stands (21 Nov 2026)

**Done.** The workspace (`cav-core`, `cav-sim`, `cav-engine`, `cav-gpu`, `cav-render`, `cav-app`, `cav`,
`cav-validate`, `xtask`). Open the app (`cargo run -p cav-app`): the room, five towers, menu bar,
outliner, properties, per-tower timeline, console, footer; perspective and orthographic, seven named
views and Home, 1, 2 or 4 panes, five draw styles. `cav doctor`, `cav render`, `cav scenes`, `cav schema`,
`cav exec`; `cav mcp` with `scene_schema` and five more tools; `scenes/dev/void.ron` and
`infinite-floor.ron`; `cargo xtask quick | gpu | oracle | demo S0 | validate --report`. The engine's
command/query API is the one path all of them drive, with a journal that replays.

**Numbers** (detail in `docs/slices/S0.md` §7): `quick` 97 tests in 4.6 s warm (gate: 10 s); `gpu` 14
tests in 5.5 s; cold build 51 s with the Defender exclusion; warm rebuild 2.4 s; 10 of 10 validation
cases pass.

**Open.**
- Nobody has looked at the real window: it was started minimised for five seconds and closed. Everything
  seen is from the headless renderer, which runs the same UI code.
- The oracle tier is a stub until S1.
- The rules of the validation report are not in `CLAUDE.md` or the agent definitions (question below).

**Next.** A fresh reviewer reads the slice; its findings become further task PRs; the tag follows.
Then S1: the forward model, the oracle crate, a point mass you can drag. S1's brief should start from
George's answers below and add the first physics rows to the validation report (the void's channels
exactly zero, the infinite floor's null, the v1 anchors).

## Questions for the owner
- **21 Nov 2026 — Ubuntu Mono.** The mock-up uses Ubuntu Mono; egui bundles Ubuntu Light but not the
  mono face. Adding `UbuntuMono-Regular.ttf` and its licence text (Ubuntu Font Licence 1.0) to this public
  repository is a licence matter, so S0 uses egui's bundled Hack. Say yes and the font goes in.
- **21 Nov 2026 — the validation report's rules in `CLAUDE.md` and the agent definitions.** The request
  to add them came to the S0 builder through the coordinating agent, and a message from an agent cannot
  authorise edits to `CLAUDE.md` or `.claude/agents/`, so the builder left them alone. The rules are in
  force in `docs/validation/README.md`. If you want them in those files, these are the lines:
  - `CLAUDE.md`, a new section "Validation report": "`cargo xtask validate --report` runs every
    validation case and writes `docs/validation/` (how: `docs/validation/README.md`). Never drop a case to
    make the report green. A tolerance changes only with a line in the slice notes saying why. Every slice
    ends by regenerating the report and committing it; the reviewer checks it."
  - `.claude/agents/builder.md`, a step: "Regenerate the validation report (`cargo xtask validate
    --report`) at the end of the slice and commit `docs/validation/`; new physics gets new cases
    (`cav_validate::case!`) beside its tests."
  - `.claude/agents/reviewer.md`, a check: "Read `docs/validation/report.md` against the previous
    `report.json` in git: no dropped cases, no tolerance changed without a line in the slice notes, no
    missing cases."

## For George
Answered 21 Nov 2026 (through the owner). The defaults below replace v1's conventions from S1 on.
1. **Sign:** lower minus upper (δφ₁ − δφ₂). Both conventions are in use and it should not matter.
2. **Timing:** the atom and the source are taken at the same time; the reference scripts' one-step
   offset (arm at (j+1)·dt against source at j·dt) is not intended.
3. **Atom:** ⁸⁷Sr, mass ≈ 1.443×10⁻²⁵ kg (S1 takes the value from a cited source), not the
   1.46×10⁻²⁵ kg (⁸⁸Sr) used so far.
4. **Launch:** AION-10's launch speed is 3.86 m/s; the atoms come back to the launch height and pause
   there until the next sequence starts.
5. **Floor:** the arms never go below launch. There is a floor, coded as the Python scripts code the
   sequence; copy that.

Still open (asked in S0; defaults in use until answered):
- The ground layout of a multi-tower array and the room around it. S0 draws v2's layout: five towers,
  one at the centre and four on a cross with 4 m arms, standing on the floor (z = 0) of a
  10 × 10 × 3 m room, each 10 m tall so they rise through the ceiling. Is there a real or intended
  layout, and where is the tower base relative to the floor people walk on?

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
  - `quick` is 4.6 s warm but 7 to 10 s on the run straight after an edit that recompiles `cav-app`
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
