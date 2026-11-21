# Progress

The builder updates this after every slice. The owner reads it at checkpoints.

| Slice | State | Tag | Notes |
|---|---|---|---|
| S0 the shell | building, task by task (`plan/tasks.yaml`) | | research: `docs/research/s0-app-stack-and-build-speed.md` |

## Questions for the owner
(none open)

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
- **Defender exclusion:** `C:\src\Cavendish	arget` is excluded from real-time scanning (added by the
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
